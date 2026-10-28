use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use ustt_core::{MediaKind, Project, RecentEntry, Settings};
use ustt_ui_model::{ProgressView, WaveData};

pub mod media;
#[cfg(target_arch = "wasm32")]
pub mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportInput {
    File(PathBuf),
    Url(String),
}

pub fn parse_import_url(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return Err("enter one HTTP(S) URL".into());
    }
    let parsed = url::Url::parse(value).map_err(|_| "enter one valid HTTP(S) URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("only HTTP(S) URLs are supported".into());
    }
    Ok(parsed.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaAsset {
    Audio,
    Proxy,
    Waveform,
}

#[derive(Debug)]
pub enum ImportEvent {
    Progress(ProgressView),
    Finished(Box<Project>),
    Failed(String),
    Cancelled,
}

pub struct ImportHandle {
    pub rx: std::sync::mpsc::Receiver<ImportEvent>,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub format: ustt_export::TextOutput,
    pub speakers: Option<Vec<uuid::Uuid>>,
    pub timestamps: bool,
    pub audio: ustt_export::AudioMode,
    pub audio_format: ustt_core::AudioFormat,
    pub segments: Option<Vec<uuid::Uuid>>,
    pub out_dir: PathBuf,
}

#[derive(Debug)]
pub enum ExportEvent {
    Progress { fraction: f32, detail: String },
    Done(Vec<PathBuf>),
    Download { filename: String, bytes: Vec<u8> },
    Failed(String),
    Cancelled,
}

pub struct ExportJob {
    pub rx: std::sync::mpsc::Receiver<ExportEvent>,
    pub cancel: Arc<AtomicBool>,
}

pub fn validate_export(request: &ExportRequest) -> Result<(), String> {
    if matches!(request.audio, ustt_export::AudioMode::Segments)
        && request.segments.as_ref().is_none_or(|ids| ids.is_empty())
    {
        return Err("select at least one segment for audio export".into());
    }
    if request.out_dir.as_os_str().is_empty() {
        return Err("choose a destination folder".into());
    }
    Ok(())
}

pub fn export_filename(title: &str, date: &str, format: ustt_export::TextOutput) -> String {
    let slug = ustt_core::slugify(title);
    let slug = if slug == "project" {
        "transcript".to_string()
    } else {
        slug
    };
    format!("{slug}-{date}.{}", format.extension())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreHandle {
    pub dir: PathBuf,
}

impl StoreHandle {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn media_dir(&self) -> PathBuf {
        self.dir.join("media")
    }
}

pub trait Service: Send + Sync {
    fn settings(&self) -> Settings;
    fn save_settings(&self, settings: &Settings) -> Result<(), String>;
    fn recent(&self) -> Vec<RecentEntry>;
    fn push_recent(&self, entry: RecentEntry);
    fn pick_media(&self) -> Option<PathBuf>;
    fn pick_project(&self) -> Option<PathBuf>;
    fn import(&self, input: ImportInput, project_dir: PathBuf) -> ImportHandle;
    fn open(&self, dir: &Path) -> Result<(StoreHandle, Project), String>;
    fn save(&self, store: &StoreHandle, project: &mut Project) -> Result<(), String>;
    fn save_as(
        &self,
        store: &StoreHandle,
        project: &mut Project,
        dir: PathBuf,
    ) -> Result<StoreHandle, String>;
    fn media_url(&self, store: &StoreHandle, asset: MediaAsset) -> Option<String>;
    fn waveform(&self, store: &StoreHandle) -> Option<WaveData>;
    fn project_root(&self, store: &StoreHandle) -> Option<PathBuf>;
    fn demo_available(&self) -> bool;
    fn demo(&self, kind: MediaKind) -> Option<(StoreHandle, Project)>;
    fn export(&self, store: &StoreHandle, project: Project, request: ExportRequest) -> ExportJob;
    fn default_export_dir(&self) -> Option<PathBuf>;
}

pub fn open_service() -> Arc<dyn Service> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        Arc::new(native::NativeService::open())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Arc::new(mock::MockService::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(segments: Option<Vec<uuid::Uuid>>) -> ExportRequest {
        ExportRequest {
            format: ustt_export::TextOutput::Vtt,
            speakers: None,
            timestamps: true,
            audio: ustt_export::AudioMode::Segments,
            audio_format: ustt_core::AudioFormat::Wav,
            segments,
            out_dir: PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn import_url_validation_accepts_only_one_http_url() {
        assert_eq!(
            parse_import_url("  https://example.test/clip.mp4  ").unwrap(),
            "https://example.test/clip.mp4"
        );
        assert_eq!(
            parse_import_url("http://localhost:8080/a.wav").unwrap(),
            "http://localhost:8080/a.wav"
        );
        for invalid in [
            "",
            "ftp://example.test/a.wav",
            "javascript:alert(1)",
            "https://example.test/a https://example.test/b",
            "https://",
        ] {
            assert!(parse_import_url(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn export_request_rejects_empty_selection() {
        assert!(
            validate_export(&request(None))
                .unwrap_err()
                .contains("segment")
        );
        assert!(validate_export(&request(Some(Vec::new()))).is_err());
        assert!(validate_export(&request(Some(vec![uuid::Uuid::nil()]))).is_ok());
    }

    #[test]
    fn export_filename_is_slugged() {
        assert_eq!(
            export_filename("My Talk", "2026-10-28", ustt_export::TextOutput::Vtt),
            "my-talk-2026-10-28.vtt"
        );
        assert_eq!(
            export_filename("!!!", "2026-10-28", ustt_export::TextOutput::Txt),
            "transcript-2026-10-28.txt"
        );
    }
}
