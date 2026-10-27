use super::player;
use super::speakers::SpeakerBar;
use super::timeline::Timeline;
use super::transcript::Transcript;
use crate::service::{MediaAsset, Service, StoreHandle};
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use std::sync::Arc;
use ustt_core::{History, MediaKind, Project, slugify};
use ustt_ui_model::{Lang, Playback, WaveData, strings};

pub struct ProjectSession {
    pub store: StoreHandle,
    pub history: History<Project>,
    pub saved_at: Option<DateTime<Utc>>,
    pub audio_url: Option<String>,
    pub proxy_url: Option<String>,
    pub wave: WaveData,
}

impl ProjectSession {
    pub fn new(store: StoreHandle, project: Project, service: &dyn Service) -> Self {
        let audio_url = service.media_url(&store, MediaAsset::Audio);
        let proxy_url = service.media_url(&store, MediaAsset::Proxy);
        let wave = service.waveform(&store).unwrap_or_default();
        Self {
            store,
            history: History::new(project, 100),
            saved_at: None,
            audio_url,
            proxy_url,
            wave,
        }
    }

    pub fn save(&mut self, service: &dyn Service) -> Result<(), String> {
        let mut current = self.history.current().clone();
        service.save(&self.store, &mut current)?;
        self.history.mark_saved();
        self.saved_at = Some(Utc::now());
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectGrid {
    pub transcript: &'static str,
    pub preview: &'static str,
}

pub fn project_grid(kind: MediaKind) -> ProjectGrid {
    match kind {
        MediaKind::Video => ProjectGrid {
            transcript: "minmax(0,1fr)",
            preview: "minmax(0,1fr)",
        },
        MediaKind::Audio => ProjectGrid {
            transcript: "minmax(0,1fr)",
            preview: "0px",
        },
    }
}

pub fn commit_edit(history: &mut History<Project>, edit: impl FnOnce(&mut Project) -> bool) {
    let mut next = history.current().clone();
    if edit(&mut next) {
        history.commit(next);
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ProjectViewProps {
    pub lang: Lang,
}

#[component]
pub fn ProjectView(props: ProjectViewProps) -> Element {
    let service = use_context::<Arc<dyn Service>>();
    let session = use_context::<Signal<Option<ProjectSession>>>();
    let playback = use_context::<Signal<Playback>>();
    let text = strings(props.lang);

    let Some(project) = session.read().as_ref().map(|s| s.history.current().clone()) else {
        return rsx! { section { class: "placeholder", "{text.project_missing}" } };
    };

    let selected_speaker = use_signal(|| None::<uuid::Uuid>);
    let mut project_error = use_signal(|| None::<String>);
    use_effect(move || player::bind(playback));

    let rename = {
        move |(id, name): (uuid::Uuid, String)| {
            let mut session = session;
            if let Some(state) = session.write().as_mut() {
                commit_edit(&mut state.history, |project| {
                    ustt_ui_model::rename_speaker(project, id, &name)
                });
            }
        }
    };
    let edit_text = {
        move |(id, value): (uuid::Uuid, String)| {
            let mut session = session;
            if let Some(state) = session.write().as_mut() {
                commit_edit(&mut state.history, |project| {
                    ustt_ui_model::set_segment_text(project, id, &value)
                });
            }
        }
    };
    let seek = {
        move |seconds: f64| {
            let mut state = playback;
            state.write().seek(seconds);
            player::seek(seconds);
        }
    };

    let save_now = {
        let service = service.clone();
        move |_| {
            let mut session = session;
            if let Some(state) = session.write().as_mut() {
                let _ = state.save(service.as_ref());
            }
        }
    };
    let save_as = {
        let service = service.clone();
        move |_| {
            let mut session = session;
            let Some(parent) = service.pick_project() else {
                return;
            };
            if let Some(state) = session.write().as_mut() {
                let title = state.history.current().title.clone();
                let target = parent.join(format!("{}.ustt", slugify(&title)));
                let mut copy = state.history.current().clone();
                match service.save_as(&state.store, &mut copy, target) {
                    Ok(store) => {
                        state.audio_url = service.media_url(&store, MediaAsset::Audio);
                        state.proxy_url = service.media_url(&store, MediaAsset::Proxy);
                        state.wave = service.waveform(&store).unwrap_or_default();
                        state.store = store;
                    }
                    Err(message) => project_error.set(Some(message)),
                }
            }
        }
    };
    let switch_project = {
        let service = service.clone();
        move |evt: Event<FormData>| {
            let mut session = session;
            let path = std::path::PathBuf::from(evt.value());
            if path.as_os_str().is_empty() {
                return;
            }
            if let Some(state) = session.write().as_mut() {
                let _ = state.save(service.as_ref());
            }
            match service.open(&path) {
                Ok((store, project)) => {
                    let mut player = playback;
                    player.write().set_duration(project.duration);
                    *session.write() = Some(ProjectSession::new(store, project, service.as_ref()));
                }
                Err(message) => project_error.set(Some(message)),
            }
        }
    };
    let undo = move |_| {
        let mut session = session;
        if let Some(state) = session.write().as_mut() {
            state.history.undo();
        }
    };
    let redo = move |_| {
        let mut session = session;
        if let Some(state) = session.write().as_mut() {
            state.history.redo();
        }
    };

    let recent = service.recent();
    let dirty = session
        .read()
        .as_ref()
        .map(|s| s.history.is_dirty())
        .unwrap_or(false);
    let can_undo = session
        .read()
        .as_ref()
        .map(|s| s.history.can_undo())
        .unwrap_or(false);
    let can_redo = session
        .read()
        .as_ref()
        .map(|s| s.history.can_redo())
        .unwrap_or(false);
    let audio_url = session
        .read()
        .as_ref()
        .and_then(|s| s.audio_url.clone())
        .unwrap_or_default();
    let wave = session
        .read()
        .as_ref()
        .map(|s| s.wave.clone())
        .unwrap_or_default();
    let proxy_url = session
        .read()
        .as_ref()
        .and_then(|s| s.proxy_url.clone())
        .unwrap_or_default();
    let grid = project_grid(project.kind);

    rsx! {
        section { class: "project", "data-testid": "project",
            if let Some(message) = project_error() {
                div { class: "banner",
                    span { "{message}" }
                    button { class: "btn ghost small", onclick: move |_| project_error.set(None), "{text.dismiss}" }
                }
            }
            div { class: "project-toolbar",
                div { class: "project-title", "{project.title}" }
                div { class: "topbar-actions",
                    span {
                        class: "save-state",
                        "data-testid": "save-state",
                        if dirty { "{text.unsaved}" } else { "{text.saved}" }
                    }
                    button { class: "icon-button", title: text.save, "data-testid": "save-button", disabled: !dirty, onclick: save_now, "💾" }
                    button { class: "icon-button", title: text.undo, disabled: !can_undo, onclick: undo, "↶" }
                    button { class: "icon-button", title: text.redo, disabled: !can_redo, onclick: redo, "↷" }
                    button { class: "btn ghost small", onclick: save_as, "{text.save_as}" }
                    if !recent.is_empty() {
                        select {
                            class: "project-switch",
                            "data-testid": "project-switch",
                            value: "",
                            onchange: switch_project,
                            option { value: "", "{text.switching_project}" }
                            for entry in recent {
                                option { value: "{entry.path.display()}", "{entry.title}" }
                            }
                        }
                    }
                }
            }
            SpeakerBar {
                lang: props.lang,
                project: project.clone(),
                selected: selected_speaker,
                on_rename: rename,
            }
            div {
                class: if project.kind == MediaKind::Video { "project-body video" } else { "project-body audio" },
                style: "grid-template-columns: {grid.transcript} {grid.preview};",
                Transcript {
                    lang: props.lang,
                    project: project.clone(),
                    selected: selected_speaker,
                    on_seek: seek,
                    on_edit: edit_text,
                }
                if project.kind == MediaKind::Video {
                    div { class: "preview",
                        if proxy_url.is_empty() {
                            div { class: "missing", "{text.proxy_missing}" }
                        } else {
                            video {
                                id: "player",
                                src: "{proxy_url}",
                                preload: "metadata",
                                playsinline: true,
                            }
                        }
                    }
                }
            }
            if project.kind == MediaKind::Audio {
                audio {
                    id: "player",
                    src: "{audio_url}",
                    preload: "metadata",
                    style: "display:none;",
                }
            }
            Timeline {
                lang: props.lang,
                project: project.clone(),
                wave,
                on_seek: seek,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ustt_core::{MediaKind, Segment, SourceRef, Speaker};
    use ustt_ui_model::{active_segment, rename_speaker};
    use uuid::Uuid;

    fn sample() -> Project {
        let mut project = Project::new(
            "Demo",
            SourceRef {
                path: "/tmp/demo.wav".into(),
                size_bytes: 1,
                modified_unix: 0,
                fingerprint: "x".into(),
            },
            MediaKind::Audio,
            10.0,
        );
        project.speakers = vec![Speaker {
            id: Uuid::from_u128(1),
            name: "Speaker 1".into(),
            color: 0,
        }];
        project.segments = vec![Segment {
            id: Uuid::from_u128(10),
            start: 0.0,
            end: 2.0,
            text: "hello".into(),
            speaker_id: Some(Uuid::from_u128(1)),
            words: Vec::new(),
        }];
        project
    }

    #[test]
    fn edit_then_save_then_undo_marks_dirty() {
        let mut history = History::new(sample(), 100);
        commit_edit(&mut history, |p| {
            rename_speaker(p, Uuid::from_u128(1), "Alice")
        });
        assert!(history.is_dirty());
        history.mark_saved();
        assert!(!history.is_dirty());
        assert!(history.undo());
        assert_eq!(history.current().speakers[0].name, "Speaker 1");
        assert!(history.can_redo());
        assert!(history.is_dirty());
    }

    #[test]
    fn video_layout_is_half_and_half() {
        let grid = project_grid(MediaKind::Video);
        assert_eq!(grid.transcript, "minmax(0,1fr)");
        assert_eq!(grid.preview, "minmax(0,1fr)");
        assert_eq!(project_grid(MediaKind::Audio).preview, "0px");
    }

    #[test]
    fn active_segment_follows_playhead() {
        let project = sample();
        assert_eq!(active_segment(&project, 0.5), Some(0));
        assert_eq!(active_segment(&project, 100.0), None);
    }
}
