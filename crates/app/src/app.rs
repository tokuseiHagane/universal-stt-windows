use crate::components::player;
use crate::components::processing::Processing;
use crate::components::project::{ProjectSession, ProjectView};
use crate::components::settings::{SettingsScreen, load_project_error};
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
    let mut settings = use_signal(|| service.settings());
    let mut lang = use_signal(|| {
        Lang::from_preference(
            settings.read().language.as_deref(),
            system_locale().as_deref(),
        )
    });
    let mut theme = use_signal(|| settings.read().theme.clone());
    let mut screen = use_signal(|| Screen::Welcome);
    let mut pending = use_signal(|| None::<ImportHandle>);
    let mut pending_dir = use_signal(|| None::<std::path::PathBuf>);
    let mut pending_name = use_signal(String::new);
    let mut progress = use_signal(|| None::<ProgressView>);
    let mut error = use_signal(|| None::<String>);
    let mut settings_open = use_signal(|| false);
    let mut recent = use_signal(|| service.recent());
    use_context_provider(|| recent);
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
                                Ok((store, project)) => {
                                    let mut player = playback;
                                    player.write().set_duration(project.duration);
                                    session.set(Some(ProjectSession::new(
                                        store,
                                        project,
                                        service.as_ref(),
                                    )));
                                    screen.set(Screen::Project);
                                }
                                Err(_) => error.set(Some(load_project_error(&dir))),
                            }
                            recent.set(service.recent());
                        }
                        pending.set(None);
                        progress.set(None);
                    }
                    Some(ImportEvent::Failed(message)) => {
                        pending.set(None);
                        progress.set(None);
                        processing_error.set(Some(message));
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
                    let snapshot = {
                        let guard = session.read();
                        guard.as_ref().and_then(|state| state.snapshot_for_save())
                    };
                    if let Some((store, snapshot)) = snapshot {
                        let mut to_save = snapshot.clone();
                        match service.save(&store, &mut to_save) {
                            Ok(()) => {
                                if let Some(state) = session.write().as_mut() {
                                    state.mark_saved_if(&snapshot);
                                }
                            }
                            Err(message) => error.set(Some(message)),
                        }
                    }
                }
            }
        });
    }

    let key_service = service.clone();
    use_effect(move || {
        let service = key_service.clone();
        spawn(async move {
            const KEYS: &str = r#"
(async () => {
  if (window.__usttKeys) document.removeEventListener("keydown", window.__usttKeys);
  window.__usttKeys = (event) => {
    const tag = (event.target && event.target.tagName) || "";
    if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
    const key = event.key;
    if (key === " " || key.startsWith("Arrow") || key === "Home" || key === "End") {
      event.preventDefault();
    }
    dioxus.send({
      key,
      ctrl: event.ctrlKey || event.metaKey,
      shift: event.shiftKey,
    });
  };
  document.addEventListener("keydown", window.__usttKeys);
})();
"#;
            let mut eval = document::eval(KEYS);
            while let Ok(key) = eval.recv::<KeyMsg>().await {
                if screen() != Screen::Project {
                    continue;
                }
                match (key.ctrl, key.key.as_str()) {
                    (true, "s") | (true, "S") => {
                        if let Some(state) = session.write().as_mut()
                            && state.history.is_dirty()
                            && let Err(message) = state.save(service.as_ref())
                        {
                            error.set(Some(message));
                        }
                    }
                    (true, "z") | (true, "Z") => {
                        let mut state = session.write();
                        if let Some(state) = state.as_mut() {
                            if key.shift {
                                state.history.redo();
                            } else {
                                state.history.undo();
                            }
                        }
                    }
                    (true, "y") | (true, "Y") => {
                        if let Some(state) = session.write().as_mut() {
                            state.history.redo();
                        }
                    }
                    (false, " ") => {
                        if playback.read().playing {
                            player::pause();
                        } else {
                            player::play();
                        }
                    }
                    (false, "ArrowLeft") => {
                        let step = if key.shift { -30.0 } else { -5.0 };
                        let mut state = playback;
                        state.write().step(step);
                        player::seek(state.read().position);
                    }
                    (false, "ArrowRight") => {
                        let step = if key.shift { 30.0 } else { 5.0 };
                        let mut state = playback;
                        state.write().step(step);
                        player::seek(state.read().position);
                    }
                    (false, "Home") => {
                        let mut state = playback;
                        state.write().seek(0.0);
                        player::seek(0.0);
                    }
                    (false, "End") => {
                        let mut state = playback;
                        let duration = state.read().duration;
                        state.write().seek(duration);
                        player::seek(duration);
                    }
                    _ => {}
                }
            }
        });
    });

    use_effect(move || {
        let theme = serde_json::to_string(&theme()).unwrap_or_else(|_| "\"dark\"".into());
        let code = format!(
            "document.documentElement.dataset.theme = ({theme} === 'system') ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark') : {theme};"
        );
        spawn(async move {
            let _ = document::eval(&code).await;
        });
    });

    rsx! {
        document::Title { "{text.app_name}" }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Script { src: "/timeline.js" }
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
                        "data-testid": "open-settings",
                        onclick: move |_| settings_open.set(true),
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
                                        Err(_) => error.set(Some(load_project_error(&dir))),
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
            if settings_open() {
                SettingsScreen {
                    lang: lang(),
                    on_close: move |_| settings_open.set(false),
                    on_saved: move |next: ustt_core::Settings| {
                        theme.set(next.theme.clone());
                        lang.set(Lang::from_preference(
                            next.language.as_deref(),
                            system_locale().as_deref(),
                        ));
                        settings.set(next);
                    },
                }
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct KeyMsg {
    key: String,
    ctrl: bool,
    shift: bool,
}

fn system_locale() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.navigator().language())
            .or_else(|| Some("en".into()))
    }
    #[cfg(windows)]
    {
        windows_locale().or_else(|| std::env::var("LANG").ok())
    }
    #[cfg(all(not(windows), not(target_arch = "wasm32")))]
    {
        std::env::var("LANG").ok()
    }
}

#[cfg(windows)]
fn windows_locale() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buffer = [0u16; 85];
    let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    (length > 1).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
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
