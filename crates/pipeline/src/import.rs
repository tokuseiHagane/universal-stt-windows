use crate::backend::{Backend, parakeet_lib_path};
use crate::merge::build_segments;
use crate::models::{ModelError, ModelPaths};
use crate::progress::{Phase, Progress, estimate_eta, with_overall_eta, with_phase_eta};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use ustt_asr::{AsrEngine, AsrError, ParakeetLib, VadEngine, transcribe_chunks_cancellable};
use ustt_core::{MediaKind, Project, SourceRef, speakers_from_diarization};
use ustt_core::{ProjectStore, StoreError};
use ustt_diarize::{DiarizeError, WhoModel};
use ustt_download::{DownloadError, DownloadTools, DownloadedMedia, download as download_url};
use ustt_media::{MediaError, Tools};
use ustt_media::{
    decode_wav16k, make_proxy, probe, read_wav_mono16k_range, wav_mono16k_samples,
    waveform_from_wav,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    File(PathBuf),
    Url(String),
}

impl ImportSource {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.starts_with("http://") || value.starts_with("https://") {
            let parsed = url::Url::parse(value).map_err(|e| format!("invalid URL: {e}"))?;
            if parsed.host_str().is_none() {
                return Err("URL must include a host".into());
            }
            Ok(Self::Url(parsed.into()))
        } else if value.contains("://") {
            Err("only HTTP(S) URLs are supported".into())
        } else if value.is_empty() {
            Err("input path or URL is empty".into())
        } else {
            Ok(Self::File(PathBuf::from(value)))
        }
    }
}

pub struct ImportRequest {
    pub input: ImportSource,
    pub project_dir: PathBuf,
    pub models: ModelPaths,
    pub backend: Backend,
    pub ffmpeg_dir: Option<PathBuf>,
    pub make_proxy: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Media(#[from] MediaError),
    #[error(transparent)]
    Diarize(#[from] DiarizeError),
    #[error(transparent)]
    Asr(#[from] AsrError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("project already exists: {0}")]
    Exists(PathBuf),
    #[error("worker thread panicked")]
    Worker,
}

struct PartialGuard {
    dir: PathBuf,
    keep: bool,
}

impl Drop for PartialGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

pub fn run_import(
    req: &ImportRequest,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Project, PipelineError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }

    if req.project_dir.exists() {
        return Err(PipelineError::Exists(req.project_dir.clone()));
    }
    let started = Instant::now();
    let mut last_fraction = 0.0f32;
    let mut current_phase = None;
    let mut phase_started = Instant::now();
    let mut emit = |mut progress: Progress| {
        if current_phase != Some(progress.phase) {
            current_phase = Some(progress.phase);
            phase_started = Instant::now();
        }
        progress.fraction = progress.fraction.max(last_fraction);
        last_fraction = progress.fraction;
        let progress = with_phase_eta(progress, phase_started.elapsed().as_secs_f64());
        on_progress(with_overall_eta(progress, started.elapsed().as_secs_f64()));
    };
    let partial = partial_dir(&req.project_dir);
    let _ = std::fs::remove_dir_all(&partial);
    let mut guard = PartialGuard {
        dir: partial.clone(),
        keep: false,
    };

    let download_weight = if matches!(req.input, ImportSource::Url(_)) {
        crate::progress::DOWNLOAD_WEIGHT
    } else {
        0.0
    };
    let (input, origin_url, remote_title, _download_guard): (
        PathBuf,
        Option<String>,
        Option<String>,
        Option<DownloadedMedia>,
    ) = match &req.input {
        ImportSource::File(path) => (path.clone(), None, None, None),
        ImportSource::Url(url) => {
            emit(Progress::new_scaled(
                Phase::Download,
                0.0,
                "starting download",
                download_weight,
            ));
            let tools = DownloadTools::locate(None)?;
            let downloaded = download_url(
                &tools,
                url,
                cancel,
                &mut |progress: ustt_download::DownloadProgress| {
                    let detail = match progress.total_bytes {
                        Some(total) => {
                            format!("downloaded {} / {} bytes", progress.downloaded_bytes, total)
                        }
                        None => format!("downloaded {} bytes", progress.downloaded_bytes),
                    };
                    emit(Progress::new_scaled(
                        Phase::Download,
                        progress.fraction.unwrap_or(0.0),
                        detail,
                        download_weight,
                    ));
                },
            )
            .map_err(|error| match error {
                DownloadError::Cancelled => PipelineError::Cancelled,
                other => other.into(),
            })?;
            (
                downloaded.path.clone(),
                Some(url.clone()),
                Some(downloaded.title.clone()),
                Some(downloaded),
            )
        }
    };

    let tools = Tools::locate(req.ffmpeg_dir.as_deref())?;
    emit(Progress::new_scaled(
        Phase::Probe,
        0.5,
        "probing",
        download_weight,
    ));
    let info = probe(&tools, &input)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }
    let kind = if info.video.is_some() {
        MediaKind::Video
    } else {
        MediaKind::Audio
    };
    let title = remote_title
        .filter(|title| !title.trim().is_empty())
        .or_else(|| input.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Recording".into());
    let mut source = SourceRef::new(&input)?;
    source.origin_url = origin_url;
    let (store, mut project) = ProjectStore::create(&partial, &title, source, kind, info.duration)?;
    let media_dir = store.media_dir();
    emit(Progress::new_scaled(
        Phase::Probe,
        1.0,
        "probed",
        download_weight,
    ));

    let wav = media_dir.join("audio16k.wav");
    emit(Progress::new_scaled(
        Phase::Decode,
        0.2,
        "decoding",
        download_weight,
    ));
    decode_wav16k(&tools, &input, &wav)?;
    let waveform = waveform_from_wav(&wav, 20)?;
    std::fs::write(media_dir.join("waveform.bin"), waveform.to_bytes())?;
    let total_samples = wav_mono16k_samples(&wav)?;
    let duration = info.duration.max(total_samples as f64 / 16_000.0);
    project.duration = duration;
    emit(Progress::new_scaled(
        Phase::Decode,
        1.0,
        "decoded",
        download_weight,
    ));
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }

    let diarization = std::thread::scope(|scope| {
        let proxy_handle = req.make_proxy.then(|| {
            let tools = &tools;
            let input = &input;
            let media_dir = &media_dir;
            let stem = ustt_core::slugify(&title);
            scope.spawn(move || make_proxy(tools, input, media_dir, &stem, kind))
        });
        emit(Progress::new_scaled(
            Phase::Proxy,
            0.1,
            "preparing playback",
            download_weight,
        ));
        let mut who = WhoModel::load(&req.models.who_dir)?;
        let diarization = who
            .diarize_stream(
                total_samples,
                &mut |start, count| {
                    read_wav_mono16k_range(&wav, start, count)
                        .map_err(|e| DiarizeError::Model(e.to_string()))
                },
                Some(cancel),
                &mut |fraction| {
                    emit(Progress::new_scaled(
                        Phase::Diarize,
                        fraction,
                        "diarizing",
                        download_weight,
                    ))
                },
            )
            .map_err(|e| match e {
                DiarizeError::Cancelled => PipelineError::Cancelled,
                other => other.into(),
            })?;
        let mut proxy = None;
        if let Some(handle) = proxy_handle {
            match handle.join() {
                Ok(Ok(info)) => proxy = Some(info),
                Ok(Err(error)) => {
                    // A proxy is optional: the transcript must still be usable.
                    emit(Progress::new_scaled(
                        Phase::Proxy,
                        1.0,
                        format!("proxy unavailable: {error}"),
                        download_weight,
                    ));
                }
                Err(_) => {
                    emit(Progress::new_scaled(
                        Phase::Proxy,
                        1.0,
                        "proxy worker failed",
                        download_weight,
                    ));
                }
            }
        }
        if let Some(proxy) = proxy {
            emit(Progress::new_scaled(
                Phase::Proxy,
                1.0,
                format!("proxy: {}", proxy.codec),
                download_weight,
            ));
        }
        Ok::<_, PipelineError>(diarization)
    })?;
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }

    let lib_dir = std::env::var_os("USTT_LIB_DIR").map(PathBuf::from);
    let lib_path = parakeet_lib_path(req.backend, lib_dir.as_deref())
        .ok_or_else(|| PipelineError::Asr(AsrError::Lib("parakeet library not found".into())))?;
    let lib = std::sync::Arc::new(ParakeetLib::open(&lib_path)?);
    let mut asr = AsrEngine::load(lib.clone(), &req.models.asr_model)?;
    let mut vad = VadEngine::load(lib, &req.models.silero_vad)?;
    let transcript = transcribe_chunks_cancellable(
        &mut asr,
        &mut vad,
        &wav,
        30.0,
        Some(cancel),
        &mut |fraction| {
            let elapsed = started.elapsed().as_secs_f64();
            let processed = duration * 0.15 + duration * 0.6 * fraction as f64;
            emit(
                Progress::new_scaled(Phase::Transcribe, fraction, "transcribing", download_weight)
                    .with_eta(estimate_eta(processed, duration, elapsed)),
            );
        },
    )
    .map_err(|e| match e {
        AsrError::Cancelled => PipelineError::Cancelled,
        other => other.into(),
    })?;
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }

    emit(Progress::new_scaled(
        Phase::Merge,
        0.3,
        "merging",
        download_weight,
    ));
    project.speakers = speakers_from_diarization(&diarization);
    let ids: Vec<_> = project.speakers.iter().map(|s| s.id).collect();
    project.segments = build_segments(&transcript.words, &diarization.turns, &ids, duration);
    project.turns = diarization.turns.clone();
    project.duration = duration;
    project.language = transcript.language.clone();
    store.save(&mut project)?;
    emit(Progress::new_scaled(
        Phase::Merge,
        1.0,
        "merged",
        download_weight,
    ));

    if req.project_dir.exists() {
        return Err(PipelineError::Exists(req.project_dir.clone()));
    }
    std::fs::rename(&partial, &req.project_dir)?;
    guard.keep = true;
    emit(Progress::new_scaled(
        Phase::Done,
        1.0,
        "done",
        download_weight,
    ));
    Ok(project)
}

fn partial_dir(project_dir: &Path) -> PathBuf {
    let mut name = project_dir.as_os_str().to_os_string();
    name.push(".partial");
    PathBuf::from(name)
}
