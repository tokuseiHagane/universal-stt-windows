use super::icons::FolderIcon;
use crate::service::Service;
use dioxus::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use ustt_core::MediaKind;
use ustt_ui_model::{Lang, date_label, duration_label, strings};

const EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "opus", "m4a", "aac", "wma", "mp4", "mkv", "mov", "avi", "webm",
    "m4v", "mpeg", "mpg", "ts",
];

pub fn pick_dropped(files: &[String]) -> Option<PathBuf> {
    files
        .iter()
        .map(PathBuf::from)
        .find(|path| is_supported(path))
}

fn is_supported(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[derive(Props, Clone, PartialEq)]
pub struct WelcomeProps {
    pub lang: Lang,
    pub on_start_import: EventHandler<(PathBuf, PathBuf)>,
    pub on_open_project: EventHandler<PathBuf>,
    pub on_demo: EventHandler<MediaKind>,
}

#[component]
pub fn Welcome(props: WelcomeProps) -> Element {
    let service = use_context::<Arc<dyn Service>>();
    let text = strings(props.lang);
    let recent = service.recent();
    let mut dragging = use_signal(|| false);

    let mut start = move |path: PathBuf| {
        #[cfg(not(target_arch = "wasm32"))]
        let dir = crate::service::native::project_dir_for(
            &path,
            &crate::service::native::documents_dir(),
        );
        #[cfg(target_arch = "wasm32")]
        let dir = PathBuf::from("demo.ustt");
        props.on_start_import.call((path, dir));
    };

    rsx! {
        section { class: "welcome",
            div { class: "welcome-main",
                h1 { "{text.welcome_title}" }
                p { "{text.welcome_subtitle}" }
                div {
                    class: if dragging() { "dropzone drag" } else { "dropzone" },
                    "data-testid": "dropzone",
                    ondragover: move |evt| {
                        evt.prevent_default();
                        dragging.set(true);
                    },
                    ondragleave: move |_| dragging.set(false),
                    ondrop: move |evt| {
                        evt.prevent_default();
                        dragging.set(false);
                        let files: Vec<String> = evt
                            .data_transfer()
                            .files()
                            .into_iter()
                            .map(|file| file.path().to_string_lossy().into_owned())
                            .collect();
                        if let Some(path) = pick_dropped(&files) {
                            start(path);
                        }
                    },
                    div { class: "drop-icon", FolderIcon {} }
                    strong { "{text.welcome_drop}" }
                    span { class: "hint", "MP3 · WAV · FLAC · MP4 · MKV · MOV · WebM" }
                    button {
                        class: "btn primary",
                        onclick: move |_| {
                            if let Some(path) = service.pick_media() {
                                start(path);
                            }
                        },
                        "{text.welcome_browse}"
                    }
                }
                if service.demo_available() {
                    div { class: "demo-actions",
                        button {
                            class: "btn ghost small",
                            "data-testid": "demo-audio",
                            onclick: move |_| props.on_demo.call(MediaKind::Audio),
                            "{text.welcome_demo} · audio"
                        }
                        button {
                            class: "btn ghost small",
                            "data-testid": "demo-video",
                            onclick: move |_| props.on_demo.call(MediaKind::Video),
                            "{text.welcome_demo} · video"
                        }
                    }
                }
            }
            aside { class: "recent",
                h2 { "{text.welcome_recent}" }
                if recent.is_empty() {
                    div { class: "empty", "{text.welcome_empty}" }
                } else {
                    div { class: "recent-list",
                        for entry in recent {
                            button {
                                class: "recent-item",
                                key: "{entry.path.display()}",
                                onclick: move |_| props.on_open_project.call(entry.path.clone()),
                                span { "{entry.title}" }
                                span { class: "meta",
                                    "{date_label(&entry.opened_at, props.lang)} · {duration_label(entry.duration)}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_accepts_the_first_supported_file() {
        let files = vec![
            "/tmp/a.txt".to_string(),
            "/tmp/b.MP4".to_string(),
            "/tmp/c.wav".to_string(),
        ];
        assert_eq!(pick_dropped(&files), Some(PathBuf::from("/tmp/b.MP4")));
        assert_eq!(pick_dropped(&["/tmp/a.txt".into()]), None);
    }
}
