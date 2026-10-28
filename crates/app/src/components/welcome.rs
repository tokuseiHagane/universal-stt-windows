use super::icons::FolderIcon;
use crate::service::{ImportInput, Service, parse_import_url};
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
    pub on_start_import: EventHandler<(ImportInput, PathBuf)>,
    pub on_open_project: EventHandler<PathBuf>,
    pub on_demo: EventHandler<MediaKind>,
}

#[component]
pub fn Welcome(props: WelcomeProps) -> Element {
    let service = use_context::<Arc<dyn Service>>();
    let text = strings(props.lang);
    let recent = use_context::<Signal<Vec<ustt_core::RecentEntry>>>()();
    let mut dragging = use_signal(|| false);
    let mut url_draft = use_signal(String::new);
    let mut url_error = use_signal(|| None::<String>);

    let start = move |input: ImportInput| {
        #[cfg(not(target_arch = "wasm32"))]
        let dir = match &input {
            ImportInput::File(path) => crate::service::native::project_dir_for(
                path,
                &crate::service::native::documents_dir(),
            ),
            ImportInput::Url(_) => crate::service::native::project_dir_for(
                std::path::Path::new("URL import"),
                &crate::service::native::documents_dir(),
            ),
        };
        #[cfg(target_arch = "wasm32")]
        let dir = PathBuf::from("demo.ustt");
        props.on_start_import.call((input, dir));
    };
    let start_url = move |raw: String| match parse_import_url(&raw) {
        Ok(url) => {
            url_error.set(None);
            start(ImportInput::Url(url));
        }
        Err(_) => url_error.set(Some(text.welcome_url_invalid.to_string())),
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
                        let transfer = evt.data_transfer();
                        let files: Vec<String> = transfer
                            .files()
                            .into_iter()
                            .map(|file| file.path().to_string_lossy().into_owned())
                            .collect();
                        if let Some(path) = pick_dropped(&files) {
                            start(ImportInput::File(path));
                            return;
                        }
                        if let Some(raw) = transfer
                            .get_as_text()
                            .or_else(|| transfer.get_data("text/uri-list"))
                        {
                            start_url(raw);
                        }
                    },
                    div { class: "drop-icon", FolderIcon {} }
                    strong { "{text.welcome_drop}" }
                    span { class: "hint", "MP3 · WAV · FLAC · MP4 · MKV · MOV · WebM" }
                    button {
                        class: "btn primary",
                        onclick: move |_| {
                            if let Some(path) = service.pick_media() {
                                start(ImportInput::File(path));
                            }
                        },
                        "{text.welcome_browse}"
                    }
                }
                form {
                    class: "url-import",
                    onsubmit: move |evt| {
                        evt.prevent_default();
                        start_url(url_draft());
                    },
                    input {
                        class: "url-input",
                        "data-testid": "url-input",
                        r#type: "url",
                        placeholder: "{text.welcome_url_placeholder}",
                        aria_label: "{text.welcome_url_placeholder}",
                        value: url_draft(),
                        oninput: move |evt| url_draft.set(evt.value()),
                    }
                    button { class: "btn", r#type: "submit", "data-testid": "url-submit", "{text.welcome_url_import}" }
                }
                if let Some(message) = url_error() {
                    div { class: "url-error", role: "alert", "data-testid": "url-error", "{message}" }
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
