use super::{
    ExportEvent, ExportJob, ExportRequest, ImportEvent, ImportHandle, MediaAsset, Service,
    StoreHandle,
};
use dioxus::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use ustt_core::{
    MediaKind, Project, RecentEntry, Segment, Settings, SourceRef, Speaker, Turn, Word,
};
use ustt_ui_model::{PhaseView, ProgressView, WaveData, WavePoint};
use uuid::Uuid;

const DEMO_AUDIO: Asset = asset!("/assets/demo/audio.wav");
const DEMO_VIDEO: Asset = asset!("/assets/demo/video.mp4");

pub struct MockService;

impl MockService {
    pub fn new() -> Self {
        Self
    }

    fn project(kind: MediaKind) -> Project {
        let epoch = chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap();
        let mut project = Project {
            schema: Project::SCHEMA,
            id: Uuid::from_u128(1000),
            title: if kind == MediaKind::Video {
                "Demo video".into()
            } else {
                "Demo interview".into()
            },
            source: SourceRef {
                path: PathBuf::from("demo"),
                size_bytes: 0,
                modified_unix: 0,
                fingerprint: "demo".into(),
                origin_url: None,
            },
            kind,
            duration: 3.0,
            language: None,
            speakers: Vec::new(),
            segments: Vec::new(),
            turns: Vec::new(),
            created_at: epoch,
            modified_at: epoch,
            export_defaults: Default::default(),
        };
        project.speakers = vec![
            Speaker {
                id: Uuid::from_u128(101),
                name: "Speaker 1".into(),
                color: 0,
            },
            Speaker {
                id: Uuid::from_u128(102),
                name: "Speaker 2".into(),
                color: 1,
            },
        ];
        let lines = [
            (0.0, 0.9, "Hello, can you hear me?", 101),
            (1.0, 1.8, "Yes, loud and clear.", 102),
            (1.9, 2.5, "Let us start with the first question.", 101),
            (2.5, 3.0, "Sounds good to me.", 102),
        ];
        project.segments = lines
            .iter()
            .enumerate()
            .map(|(i, (start, end, text, speaker))| Segment {
                id: Uuid::from_u128(200 + i as u128),
                start: *start,
                end: *end,
                text: (*text).into(),
                speaker_id: Some(Uuid::from_u128(*speaker)),
                words: vec![Word {
                    start: *start,
                    end: *end,
                    text: (*text).into(),
                    confidence: Some(0.9),
                }],
            })
            .collect();
        project.language = Some("en".into());
        #[cfg(target_arch = "wasm32")]
        if web_sys::window()
            .and_then(|window| window.location().search().ok())
            .is_some_and(|query| query.contains("long=1"))
        {
            project.segments = (0..60)
                .map(|i| {
                    let start = i as f64 * project.duration / 60.0;
                    let end = (i + 1) as f64 * project.duration / 60.0;
                    Segment {
                        id: Uuid::from_u128(200 + i),
                        start,
                        end,
                        text: format!("Long transcript row {}", i + 1),
                        speaker_id: Some(Uuid::from_u128(101 + i % 2)),
                        words: Vec::new(),
                    }
                })
                .collect();
        }
        project.turns = project
            .segments
            .iter()
            .map(|segment| Turn {
                start: segment.start,
                end: segment.end,
                speaker_index: if segment.speaker_id == Some(Uuid::from_u128(101)) {
                    0
                } else {
                    1
                },
            })
            .collect();
        project
    }
}

impl Service for MockService {
    fn settings(&self) -> Settings {
        Settings::default()
    }

    fn save_settings(&self, _settings: &Settings) -> Result<(), String> {
        Ok(())
    }

    fn recent(&self) -> Vec<RecentEntry> {
        Vec::new()
    }

    fn push_recent(&self, _entry: RecentEntry) {}

    fn pick_media(&self) -> Option<PathBuf> {
        None
    }

    fn pick_project(&self) -> Option<PathBuf> {
        None
    }

    fn import(&self, _input: PathBuf, _project_dir: PathBuf) -> ImportHandle {
        let (tx, rx) = mpsc::channel();
        #[cfg(target_arch = "wasm32")]
        if web_sys::window()
            .and_then(|window| window.location().search().ok())
            .is_some_and(|query| query.contains("processing=1"))
        {
            let mut progress = ProgressView::new(PhaseView::Diarize, 0.25);
            progress.fraction = 0.25;
            progress.phase_eta_seconds = Some(30.0);
            progress.eta_seconds = Some(75.0);
            progress.detail = "diarizing".into();
            let _ = tx.send(ImportEvent::Progress(progress));
            return ImportHandle {
                rx,
                cancel: Arc::new(AtomicBool::new(false)),
            };
        }
        for (phase, fraction) in [
            (PhaseView::Probe, 0.2),
            (PhaseView::Decode, 0.5),
            (PhaseView::Diarize, 0.7),
            (PhaseView::Transcribe, 0.9),
        ] {
            let _ = tx.send(ImportEvent::Progress(ProgressView::new(phase, fraction)));
        }
        let _ = tx.send(ImportEvent::Finished(Box::new(Self::project(
            MediaKind::Audio,
        ))));
        ImportHandle {
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn open(&self, _dir: &Path) -> Result<(StoreHandle, Project), String> {
        Err("The web demo cannot open local projects".into())
    }

    fn save(&self, _store: &StoreHandle, _project: &mut Project) -> Result<(), String> {
        Ok(())
    }

    fn save_as(
        &self,
        store: &StoreHandle,
        _project: &mut Project,
        _dir: PathBuf,
    ) -> Result<StoreHandle, String> {
        Ok(store.clone())
    }

    fn media_url(&self, store: &StoreHandle, asset: MediaAsset) -> Option<String> {
        match (store.dir.to_string_lossy().as_ref(), asset) {
            ("demo-video", MediaAsset::Proxy) => Some(DEMO_VIDEO.to_string()),
            ("demo-video", MediaAsset::Audio) => Some(DEMO_VIDEO.to_string()),
            (_, MediaAsset::Audio | MediaAsset::Proxy) => Some(DEMO_AUDIO.to_string()),
            (_, MediaAsset::Waveform) => None,
        }
    }

    fn waveform(&self, _store: &StoreHandle) -> Option<WaveData> {
        let peaks = (0..150)
            .map(|i| {
                let base = ((i as f32) * 0.31).sin().abs() * 0.7 + 0.08;
                WavePoint {
                    min: -base,
                    max: base,
                }
            })
            .collect();
        Some(WaveData {
            bucket_ms: 20,
            peaks,
        })
    }

    fn project_root(&self, _store: &StoreHandle) -> Option<PathBuf> {
        None
    }

    fn demo_available(&self) -> bool {
        true
    }

    fn demo(&self, kind: MediaKind) -> Option<(StoreHandle, Project)> {
        let handle = match kind {
            MediaKind::Audio => StoreHandle::new("demo-audio"),
            MediaKind::Video => StoreHandle::new("demo-video"),
        };
        Some((handle, Self::project(kind)))
    }

    fn export(&self, _store: &StoreHandle, project: Project, request: ExportRequest) -> ExportJob {
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let result = (|| {
            if !matches!(request.audio, ustt_export::AudioMode::None) {
                return Err("audio export needs the desktop build".to_string());
            }
            if matches!(
                request.format,
                ustt_export::TextOutput::Docx | ustt_export::TextOutput::Pdf
            ) {
                return Err("DOCX/PDF export needs the desktop build".to_string());
            }
            let ls = ustt_export::lines(&project, request.speakers.as_deref());
            if ls.is_empty() {
                return Err("nothing selected for export".to_string());
            }
            Ok(ustt_export::render_text(&ls, request.format, request.timestamps).into_bytes())
        })();
        match result {
            Ok(bytes) => {
                let name = super::export_filename(
                    &project.title,
                    &chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    request.format,
                );
                let _ = tx.send(ExportEvent::Download {
                    filename: name,
                    bytes,
                });
            }
            Err(message) => {
                let _ = tx.send(ExportEvent::Failed(message));
            }
        }
        ExportJob { rx, cancel }
    }

    fn default_export_dir(&self) -> Option<PathBuf> {
        None
    }
}
