use super::speakers::SpeakerBar;
use super::transcript::Transcript;
use crate::service::{MediaAsset, Service, StoreHandle};
use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use std::sync::Arc;
use ustt_core::{History, Project};
use ustt_ui_model::{Lang, Playback, strings};

pub struct ProjectSession {
    pub store: StoreHandle,
    pub history: History<Project>,
    pub saved_at: Option<DateTime<Utc>>,
    pub audio_url: Option<String>,
    pub proxy_url: Option<String>,
}

impl ProjectSession {
    pub fn new(store: StoreHandle, project: Project, service: &dyn Service) -> Self {
        let audio_url = service.media_url(&store, MediaAsset::Audio);
        let proxy_url = service.media_url(&store, MediaAsset::Proxy);
        Self {
            store,
            history: History::new(project, 100),
            saved_at: None,
            audio_url,
            proxy_url,
        }
    }

    pub fn project(&self) -> &Project {
        self.history.current()
    }

    pub fn commit(&mut self, next: Project) {
        self.history.commit(next);
    }

    pub fn save(&mut self, service: &dyn Service) -> Result<(), String> {
        let mut current = self.history.current().clone();
        service.save(&self.store, &mut current)?;
        self.history.mark_saved();
        self.saved_at = Some(Utc::now());
        Ok(())
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

    let rename = {
        let session = session;
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
        let session = session;
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
        let playback = playback;
        move |seconds: f64| {
            let mut player = playback;
            player.write().seek(seconds);
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

    rsx! {
        section { class: "project", "data-testid": "project",
            div { class: "project-toolbar",
                div { class: "project-title", "{project.title}" }
                div { class: "topbar-actions",
                    span {
                        class: "save-state",
                        "data-testid": "save-state",
                        if dirty { "{text.unsaved}" } else { "{text.saved}" }
                    }
                    button { class: "icon-button", title: text.save, disabled: !dirty, onclick: save_now, "💾" }
                    button { class: "icon-button", title: text.undo, disabled: !can_undo, onclick: undo, "↶" }
                    button { class: "icon-button", title: text.redo, disabled: !can_redo, onclick: redo, "↷" }
                }
            }
            SpeakerBar {
                lang: props.lang,
                project: project.clone(),
                selected: selected_speaker,
                on_rename: rename,
            }
            div { class: "project-body audio",
                Transcript {
                    lang: props.lang,
                    project,
                    selected: selected_speaker,
                    on_seek: seek,
                    on_edit: edit_text,
                }
            }
            div { class: "bottom",
                div { class: "timeline-placeholder" }
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
    fn active_segment_follows_playhead() {
        let project = sample();
        assert_eq!(active_segment(&project, 0.5), Some(0));
        assert_eq!(active_segment(&project, 100.0), None);
    }
}
