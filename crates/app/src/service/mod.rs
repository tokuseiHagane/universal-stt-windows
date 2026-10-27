use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use ustt_core::{MediaKind, Project, RecentEntry, Settings};
use ustt_ui_model::ProgressView;

pub mod media;
#[cfg(target_arch = "wasm32")]
pub mod mock;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;

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
    fn import(&self, input: PathBuf, project_dir: PathBuf) -> ImportHandle;
    fn open(&self, dir: &Path) -> Result<(StoreHandle, Project), String>;
    fn save(&self, store: &StoreHandle, project: &mut Project) -> Result<(), String>;
    fn media_url(&self, store: &StoreHandle, asset: MediaAsset) -> Option<String>;
    fn project_root(&self, store: &StoreHandle) -> Option<PathBuf>;
    fn demo_available(&self) -> bool;
    fn demo(&self, kind: MediaKind) -> Option<(StoreHandle, Project)>;
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
