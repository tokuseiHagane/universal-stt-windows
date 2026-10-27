use super::{
    ExportEvent, ExportJob, ExportRequest, ImportEvent, ImportHandle, MediaAsset, Service,
    StoreHandle, media,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use ustt_core::{MediaKind, Project, ProjectStore, RecentEntry, RecentStore, Settings, slugify};
use ustt_export::{AudioMode, extract, plan_segments, plan_speakers};
use ustt_pipeline::backend::detect;
use ustt_pipeline::import::{ImportRequest, PipelineError, run_import};
use ustt_pipeline::models::resolve_models;
use ustt_ui_model::{PhaseView, ProgressView, WaveData, WavePoint};

#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error("{0}")]
    Message(String),
}

pub struct NativeService {
    settings_path: PathBuf,
    settings: RwLock<Settings>,
    recent: Mutex<RecentStore>,
}

impl NativeService {
    pub fn open() -> Self {
        let config = ustt_core::paths::config_dir_from(None);
        let _ = std::fs::create_dir_all(&config);
        let settings_path = config.join("settings.json");
        let recent_path = config.join("recent.json");
        let settings = Settings::load(&settings_path).unwrap_or_default();
        let recent = RecentStore::new(recent_path.clone())
            .load()
            .unwrap_or_else(|_| RecentStore::new(recent_path.clone()));
        Self {
            settings_path,
            settings: RwLock::new(settings),
            recent: Mutex::new(recent),
        }
    }

    fn models_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("USTT_MODELS_DIR") {
            return PathBuf::from(dir);
        }
        let settings = self.settings.read().unwrap();
        if let Some(dir) = &settings.models_dir {
            return dir.clone();
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let candidate = dir.join("models");
            if candidate.is_dir() {
                return candidate;
            }
        }
        PathBuf::from("models")
    }
}

impl Service for NativeService {
    fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        settings
            .save(&self.settings_path)
            .map_err(|e| NativeError::Message(format!("settings: {e}")).to_string())?;
        *self.settings.write().unwrap() = settings.clone();
        Ok(())
    }

    fn recent(&self) -> Vec<RecentEntry> {
        let mut recent = self.recent.lock().unwrap();
        recent.prune_missing();
        recent.list().to_vec()
    }

    fn push_recent(&self, entry: RecentEntry) {
        let mut recent = self.recent.lock().unwrap();
        recent.push(entry);
        let _ = recent.save();
    }

    fn pick_media(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(
                "Audio and video",
                &[
                    "mp3", "wav", "flac", "ogg", "opus", "m4a", "aac", "wma", "mp4", "mkv", "mov",
                    "avi", "webm", "m4v", "mpeg", "mpg", "ts",
                ],
            )
            .pick_file()
    }

    fn pick_project(&self) -> Option<PathBuf> {
        rfd::FileDialog::new().pick_folder()
    }

    fn import(&self, input: PathBuf, project_dir: PathBuf) -> ImportHandle {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let models_dir = self.models_dir();
        let backend = detect(self.settings.read().unwrap().backend);
        std::thread::spawn(move || {
            let models = match resolve_models(&models_dir, backend.selected) {
                Ok(paths) => paths,
                Err(err) => {
                    let _ = tx.send(ImportEvent::Failed(err.to_string()));
                    return;
                }
            };
            let request = ImportRequest {
                input,
                project_dir,
                models,
                backend: backend.selected,
                make_proxy: true,
            };
            let sender = tx.clone();
            let outcome = run_import(&request, &flag, &mut |progress| {
                let _ = sender.send(ImportEvent::Progress(ProgressView {
                    phase: match progress.phase {
                        ustt_pipeline::progress::Phase::Probe => PhaseView::Probe,
                        ustt_pipeline::progress::Phase::Decode => PhaseView::Decode,
                        ustt_pipeline::progress::Phase::Proxy => PhaseView::Proxy,
                        ustt_pipeline::progress::Phase::Diarize => PhaseView::Diarize,
                        ustt_pipeline::progress::Phase::Transcribe => PhaseView::Transcribe,
                        ustt_pipeline::progress::Phase::Merge => PhaseView::Merge,
                        ustt_pipeline::progress::Phase::Done => PhaseView::Done,
                    },
                    fraction: progress.fraction,
                    eta_seconds: progress.eta_seconds,
                    detail: progress.detail,
                }));
            });
            match outcome {
                Ok(project) => {
                    let _ = tx.send(ImportEvent::Finished(Box::new(project)));
                }
                Err(PipelineError::Cancelled) => {
                    let _ = tx.send(ImportEvent::Cancelled);
                }
                Err(err) => {
                    let _ = tx.send(ImportEvent::Failed(err.to_string()));
                }
            }
        });
        ImportHandle { rx, cancel }
    }

    fn open(&self, dir: &Path) -> Result<(StoreHandle, Project), String> {
        let (store, project) =
            ProjectStore::open(dir).map_err(|e| NativeError::Message(e.to_string()).to_string())?;
        let handle = StoreHandle::new(store.dir());
        let root = store.dir();
        let mut registry = media::shared().lock().unwrap();
        registry.clear();
        if let Ok(root) = root.canonicalize() {
            let audio = root.join("media").join("audio16k.wav");
            if audio.is_file() {
                registry.insert_with_root("audio", audio, root.clone());
            }
            let wave = root.join("media").join("waveform.bin");
            if wave.is_file() {
                registry.insert_with_root("waveform", wave, root.clone());
            }
            if let Ok(entries) = std::fs::read_dir(root.join("media")) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if matches!(ext, "mp4" | "m4a" | "webm" | "aac") {
                        registry.insert_with_root("proxy", path, root.clone());
                        break;
                    }
                }
            }
        }
        Ok((handle, project))
    }

    fn save(&self, store: &StoreHandle, project: &mut Project) -> Result<(), String> {
        let (store, _) = ProjectStore::open(&store.dir)
            .map_err(|e| NativeError::Message(e.to_string()).to_string())?;
        store
            .save(project)
            .map_err(|e| NativeError::Message(e.to_string()).to_string())
    }

    fn save_as(
        &self,
        store: &StoreHandle,
        project: &mut Project,
        dir: PathBuf,
    ) -> Result<StoreHandle, String> {
        if dir.exists() && dir.join("project.json").exists() {
            return Err(format!("{} already exists", dir.display()));
        }
        std::fs::create_dir_all(dir.join("media")).map_err(|e| e.to_string())?;
        if let Ok(entries) = std::fs::read_dir(store.media_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let target = dir.join("media").join(entry.file_name());
                    std::fs::copy(&path, target).map_err(|e| e.to_string())?;
                }
            }
        }
        let (target, _) = ProjectStore::open(&dir).map_err(|e| e.to_string())?;
        target.save(project).map_err(|e| e.to_string())?;
        Ok(StoreHandle::new(dir))
    }

    fn media_url(&self, _store: &StoreHandle, asset: MediaAsset) -> Option<String> {
        let token = match asset {
            MediaAsset::Audio => "audio",
            MediaAsset::Proxy => "proxy",
            MediaAsset::Waveform => "waveform",
        };
        let registry = media::shared().lock().unwrap();
        registry
            .resolve(&MediaRegistryUrl::of(token))
            .map(|_| MediaRegistryUrl::of(token))
    }

    fn waveform(&self, store: &StoreHandle) -> Option<WaveData> {
        let bytes = std::fs::read(store.media_dir().join("waveform.bin")).ok()?;
        let wave = ustt_media::Waveform::from_bytes(&bytes).ok()?;
        Some(WaveData {
            bucket_ms: wave.bucket_ms,
            peaks: wave
                .peaks
                .into_iter()
                .map(|p| WavePoint {
                    min: p.min,
                    max: p.max,
                })
                .collect(),
        })
    }

    fn project_root(&self, store: &StoreHandle) -> Option<PathBuf> {
        Some(store.dir.clone())
    }

    fn demo_available(&self) -> bool {
        false
    }

    fn demo(&self, _kind: MediaKind) -> Option<(StoreHandle, Project)> {
        None
    }

    fn export(&self, store: &StoreHandle, project: Project, request: ExportRequest) -> ExportJob {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let store = store.clone();
        std::thread::spawn(move || {
            if let Err(message) = super::validate_export(&request) {
                let _ = tx.send(ExportEvent::Failed(message));
                return;
            }
            let date = chrono::Local::now().format("%Y-%m-%d").to_string();
            let filename = super::export_filename(&project.title, &date, request.format);
            let stem = filename
                .rsplit_once('.')
                .map(|(stem, _)| stem.to_string())
                .unwrap_or(filename);
            let mut files = Vec::new();
            let selected = request.speakers.as_deref();
            let lines = ustt_export::lines(&project, selected);
            let text = match request.format {
                ustt_export::TextOutput::Docx => {
                    ustt_export::render_docx(&project, &lines, request.timestamps)
                }
                ustt_export::TextOutput::Pdf => {
                    ustt_export::render_pdf(&project, &lines, request.timestamps)
                }
                other => {
                    Ok(ustt_export::render_text(&lines, other, request.timestamps).into_bytes())
                }
            };
            let bytes = match text {
                Ok(bytes) => bytes,
                Err(err) => {
                    let _ = tx.send(ExportEvent::Failed(err.to_string()));
                    return;
                }
            };
            if let Err(err) = std::fs::create_dir_all(&request.out_dir) {
                let _ = tx.send(ExportEvent::Failed(err.to_string()));
                return;
            }
            let path = request
                .out_dir
                .join(format!("{stem}.{}", request.format.extension()));
            if let Err(err) = std::fs::write(&path, bytes) {
                let _ = tx.send(ExportEvent::Failed(err.to_string()));
                return;
            }
            files.push(path);
            if !matches!(request.audio, AudioMode::None) {
                let plan = match request.audio {
                    AudioMode::Speakers => plan_speakers(&project, selected),
                    AudioMode::Segments => {
                        let ids = request.segments.clone().unwrap_or_default();
                        plan_segments(&project, &ids)
                    }
                    AudioMode::None => unreachable!(),
                };
                let tools = match ustt_media::Tools::locate(None) {
                    Ok(tools) => tools,
                    Err(err) => {
                        let _ = tx.send(ExportEvent::Failed(err.to_string()));
                        return;
                    }
                };
                let wav = store.media_dir().join("audio16k.wav");
                let sender = tx.clone();
                let outcome = extract(
                    &tools,
                    &wav,
                    &plan,
                    request.audio_format,
                    &request.out_dir,
                    &flag,
                    &mut |fraction, detail| {
                        let _ = sender.send(ExportEvent::Progress {
                            fraction,
                            detail: detail.to_string(),
                        });
                    },
                );
                match outcome {
                    Ok(mut audio) => files.append(&mut audio),
                    Err(ustt_export::ExportError::Cancelled) => {
                        let _ = tx.send(ExportEvent::Cancelled);
                        return;
                    }
                    Err(err) => {
                        let _ = tx.send(ExportEvent::Failed(err.to_string()));
                        return;
                    }
                }
            }
            let _ = tx.send(ExportEvent::Done(files));
        });
        ExportJob { rx, cancel }
    }

    fn default_export_dir(&self) -> Option<PathBuf> {
        let dir = documents_dir().join("Universal STT").join("Exports");
        let _ = std::fs::create_dir_all(&dir);
        Some(dir)
    }
}

struct MediaRegistryUrl;

impl MediaRegistryUrl {
    fn of(token: &str) -> String {
        format!("ustt://media/{token}")
    }
}

pub fn project_dir_for(input: &Path, documents: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Recording".into());
    let slug = slugify(&stem);
    let date = chrono::Local::now().format("%Y-%m-%d");
    let base = documents.join("Universal STT");
    let mut candidate = base.join(format!("{slug}-{date}.ustt"));
    let mut n = 2;
    while candidate.exists() {
        candidate = base.join(format!("{slug}-{date}-{n}.ustt"));
        n += 1;
    }
    candidate
}

pub fn documents_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| ustt_core::paths::config_dir_from(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_is_slugged_and_dated() {
        let dir = project_dir_for(Path::new("/tmp/My Talk (final).mp4"), Path::new("/docs"));
        let name = dir.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("my-talk-final-"));
        assert!(name.ends_with(".ustt"));
        assert_eq!(dir.parent().unwrap(), Path::new("/docs/Universal STT"));
    }

    #[test]
    fn native_errors_survive_the_trait() {
        let err = NativeError::Message("missing ffmpeg".into());
        assert_eq!(err.to_string(), "missing ffmpeg");
    }
}
