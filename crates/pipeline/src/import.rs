use crate::backend::{Backend, parakeet_lib_path};
use crate::merge::build_segments;
use crate::models::{ModelError, ModelPaths};
use crate::progress::{Phase, Progress, estimate_eta};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use ustt_asr::{AsrEngine, AsrError, ParakeetLib, VadEngine, transcribe_chunks_cancellable};
use ustt_core::{MediaKind, Project, SourceRef, speakers_from_diarization};
use ustt_core::{ProjectStore, StoreError};
use ustt_diarize::{DiarizeError, WhoModel};
use ustt_media::{MediaError, Tools};
use ustt_media::{
    decode_wav16k, make_proxy, probe, read_wav_mono16k_range, wav_mono16k_samples,
    waveform_from_wav,
};

pub struct ImportRequest {
    pub input: PathBuf,
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
    let mut emit = |mut progress: Progress| {
        progress.fraction = progress.fraction.max(last_fraction);
        last_fraction = progress.fraction;
        on_progress(progress);
    };
    let partial = partial_dir(&req.project_dir);
    let _ = std::fs::remove_dir_all(&partial);
    let mut guard = PartialGuard {
        dir: partial.clone(),
        keep: false,
    };

    let tools = Tools::locate(req.ffmpeg_dir.as_deref())?;
    emit(Progress::new(Phase::Probe, 0.5, "probing"));
    let info = probe(&tools, &req.input)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }
    let kind = if info.video.is_some() {
        MediaKind::Video
    } else {
        MediaKind::Audio
    };
    let title = req
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Recording".into());
    let source = SourceRef::new(&req.input)?;
    let (store, mut project) = ProjectStore::create(&partial, &title, source, kind, info.duration)?;
    let media_dir = store.media_dir();
    emit(Progress::new(Phase::Probe, 1.0, "probed"));

    let wav = media_dir.join("audio16k.wav");
    emit(Progress::new(Phase::Decode, 0.2, "decoding"));
    decode_wav16k(&tools, &req.input, &wav)?;
    let waveform = waveform_from_wav(&wav, 20)?;
    std::fs::write(media_dir.join("waveform.bin"), waveform.to_bytes())?;
    let total_samples = wav_mono16k_samples(&wav)?;
    let duration = info.duration.max(total_samples as f64 / 16_000.0);
    project.duration = duration;
    emit(Progress::new(Phase::Decode, 1.0, "decoded"));
    if cancel.load(Ordering::Relaxed) {
        return Err(PipelineError::Cancelled);
    }

    let diarization = std::thread::scope(|scope| {
        let proxy_handle = req.make_proxy.then(|| {
            let tools = &tools;
            let input = &req.input;
            let media_dir = &media_dir;
            let stem = ustt_core::slugify(&title);
            scope.spawn(move || make_proxy(tools, input, media_dir, &stem, kind))
        });
        emit(Progress::new(Phase::Proxy, 0.1, "preparing playback"));
        let mut who = WhoModel::load(&req.models.who_dir)?;
        let diarization = who
            .diarize_stream(
                total_samples,
                &mut |start, count| {
                    read_wav_mono16k_range(&wav, start, count)
                        .map_err(|e| DiarizeError::Model(e.to_string()))
                },
                Some(cancel),
                &mut |fraction| emit(Progress::new(Phase::Diarize, fraction, "diarizing")),
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
                    emit(Progress::new(
                        Phase::Proxy,
                        1.0,
                        format!("proxy unavailable: {error}"),
                    ));
                }
                Err(_) => {
                    emit(Progress::new(Phase::Proxy, 1.0, "proxy worker failed"));
                }
            }
        }
        if let Some(proxy) = proxy {
            emit(Progress::new(
                Phase::Proxy,
                1.0,
                format!("proxy: {}", proxy.codec),
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
                Progress::new(Phase::Transcribe, fraction, "transcribing")
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

    emit(Progress::new(Phase::Merge, 0.3, "merging"));
    project.speakers = speakers_from_diarization(&diarization);
    let ids: Vec<_> = project.speakers.iter().map(|s| s.id).collect();
    project.segments = build_segments(&transcript.words, &diarization.turns, &ids, duration);
    project.turns = diarization.turns.clone();
    project.duration = duration;
    project.language = transcript.language.clone();
    store.save(&mut project)?;
    emit(Progress::new(Phase::Merge, 1.0, "merged"));

    if req.project_dir.exists() {
        return Err(PipelineError::Exists(req.project_dir.clone()));
    }
    std::fs::rename(&partial, &req.project_dir)?;
    guard.keep = true;
    emit(Progress::new(Phase::Done, 1.0, "done"));
    Ok(project)
}

fn partial_dir(project_dir: &Path) -> PathBuf {
    let mut name = project_dir.as_os_str().to_os_string();
    name.push(".partial");
    PathBuf::from(name)
}
