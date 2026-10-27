use dioxus::prelude::*;
use ustt_ui_model::{Lang, strings};

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
    let lang = use_signal(|| Lang::from_system(system_locale().as_deref()));
    let theme = use_signal(|| "dark".to_string());
    let screen = use_signal(|| Screen::Welcome);
    let text = strings(lang());

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
                match screen() {
                    Screen::Welcome => rsx! { section { class: "placeholder", "{text.welcome_title}" } },
                    Screen::Processing => rsx! { section { class: "placeholder", "{text.processing_title}" } },
                    Screen::Project => rsx! { section { class: "placeholder", "{text.transcript}" } },
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
