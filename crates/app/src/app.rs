use crate::components::processing::Processing;
use crate::components::project::{ProjectSession, ProjectView};
use crate::components::welcome::Welcome;
use crate::service::{ImportEvent, ImportHandle};
use dioxus::prelude::*;
use ustt_core::{MediaKind, RecentEntry};
use ustt_ui_model::{Lang, Playback, ProgressView, strings};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Processing,
    Project,
}

impl Screen {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::Processing => "processing",
            Self::Project => "project",
        }
    }
}

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn App() -> Element {
    let service = use_context_provider(crate::service::open_service);
    let lang = use_signal(|| Lang::from_system(system_locale().as_deref()));
    let theme = use_signal(|| "dark".to_string());
    let mut screen = use_signal(|| Screen::Welcome);
    let mut pending = use_signal(|| None::<ImportHandle>);
    let mut pending_dir = use_signal(|| None::<std::path::PathBuf>);
    let mut pending_name = use_signal(String::new);
    let mut progress = use_signal(|| None::<ProgressView>);
    let mut error = use_signal(|| None::<String>);
    let mut processing_error = use_signal(|| None::<String>);
    let mut session = use_signal(|| None::<ProjectSession>);
    let playback = use_signal(Playback::default);
    use_context_provider(|| session);
    use_context_provider(|| playback);
    let worker_service = service.clone();
    let text = strings(lang());

    use_future(move || {
        let service = worker_service.clone();
        async move {
            loop {
                futures_timer::Delay::new(std::time::Duration::from_millis(80)).await;
                let event = {
                    let guard = pending.read();
                    guard.as_ref().and_then(|handle| handle.rx.try_recv().ok())
                };
                match event {
                    Some(ImportEvent::Progress(next)) => {
                        progress.set(Some(next));
                    }
                    Some(ImportEvent::Finished(project)) => {
                        if let Some(dir) = pending_dir() {
                            service.push_recent(RecentEntry::new(
                                dir.clone(),
                                project.title.clone(),
                                project.duration,
                                project.kind,
                            ));
                            match service.open(&dir) {
                                Ok(_) => screen.set(Screen::Project),
                                Err(message) => error.set(Some(message)),
                            }
                        }
                        pending.set(None);
                        progress.set(None);
                    }
                    Some(ImportEvent::Failed(message)) => {
                        error.set(Some(message.clone()));
                    }
                    Some(ImportEvent::Cancelled) => {
                        pending.set(None);
                        progress.set(None);
                        screen.set(Screen::Welcome);
                    }
                    None => {}
                }
            }
        }
    });

    {
        let service = service.clone();
        use_future(move || {
            let service = service.clone();
            async move {
                loop {
                    let secs = service.settings().autosave_secs.clamp(1, 3600) as u64;
                    futures_timer::Delay::new(std::time::Duration::from_secs(secs)).await;
                    if let Some(state) = session.write().as_mut()
                        && state.history.is_dirty()
                    {
                        let _ = state.save(service.as_ref());
                    }
                }
            }
        });
    }

    use_effect(move || {
        let theme = theme();
        document::eval(&format!(
            "document.documentElement.dataset.theme = '{theme}';"
        ));
    });

    rsx! {
        document::Title { "{text.app_name}" }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        div { class: "app",
            header { class: "topbar",
                div { class: "brand",
                    span { class: "brand-mark", "U" }
                    span { class: "brand-name", "{text.app_name}" }
                }
                div { class: "topbar-actions",
                    button {
                        class: "icon-button",
                        title: text.settings,
                        "⚙"
                    }
                }
            }
            main { class: "screen", "data-screen": screen().as_str(),
                if let Some(message) = error() {
                    div { class: "banner",
                        span { "{message}" }
                        button { class: "btn ghost small", onclick: move |_| error.set(None), "{text.dismiss}" }
                    }
                }
                match screen() {
                    Screen::Welcome => rsx! {
                        Welcome {
                            lang: lang(),
                            on_start_import: {
                                let service = service.clone();
                                move |(input, dir): (std::path::PathBuf, std::path::PathBuf)| {
                                pending_name.set(
                                    input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                                );
                                pending_dir.set(Some(dir.clone()));
                                progress.set(None);
                                pending.set(Some(service.import(input, dir)));
                                screen.set(Screen::Processing);
                                }
                            },
                            on_open_project: {
                                let service = service.clone();
                                move |dir: std::path::PathBuf| {
                                    match service.open(&dir) {
                                        Ok((store, project)) => {
                                            let mut player = playback;
                                            player.write().set_duration(project.duration);
                                            session.set(Some(ProjectSession::new(store, project, service.as_ref())));
                                            screen.set(Screen::Project);
                                        }
                                        Err(message) => error.set(Some(message)),
                                    }
                                }
                            },
                            on_demo: {
                                let service = service.clone();
                                move |kind: MediaKind| {
                                    match service.demo(kind) {
                                        Some((store, project)) => {
                                            let mut player = playback;
                                            player.write().set_duration(project.duration);
                                            session.set(Some(ProjectSession::new(store, project, service.as_ref())));
                                            screen.set(Screen::Project);
                                        }
                                        None => error.set(Some(text.error.to_string())),
                                    }
                                }
                            },
                        }
                    },
                    Screen::Processing => {
                        let cancel = move |_| {
                            if let Some(handle) = pending.read().as_ref() {
                                handle
                                    .cancel
                                    .store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        };
                        rsx! {
                            Processing {
                                lang: lang(),
                                file_name: pending_name(),
                                progress: progress(),
                                error: processing_error(),
                                on_cancel: cancel,
                                on_back: move |_| {
                                    pending.set(None);
                                    processing_error.set(None);
                                    progress.set(None);
                                    screen.set(Screen::Welcome);
                                },
                            }
                        }
                    },
                    Screen::Project => rsx! { ProjectView { lang: lang() } },
                }
            }
        }
    }
}

fn system_locale() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.navigator().language())
            .or_else(|| Some("en".into()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("LANG").ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_names_are_stable() {
        assert_eq!(Screen::Welcome.as_str(), "welcome");
        assert_eq!(Screen::Processing.as_str(), "processing");
        assert_eq!(Screen::Project.as_str(), "project");
    }
}
