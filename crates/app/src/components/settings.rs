use crate::service::Service;
use dioxus::prelude::*;
use std::path::Path;
use std::sync::Arc;
use ustt_core::{BackendPreference, Settings};
use ustt_ui_model::{Lang, strings};

fn backend_hint() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let detection = ustt_pipeline::backend::detect(BackendPreference::Auto);
        match detection.gpu_name {
            Some(gpu) => format!("{:?} · {gpu}", detection.selected),
            None => format!("{:?}", detection.selected),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        "CPU".to_string()
    }
}

pub fn load_project_error(path: &Path) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        match ustt_core::ProjectStore::open(path) {
            Ok(_) => String::new(),
            Err(err) => format!("project.json: {err}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        format!("project.json: {} is missing", path.display())
    }
}

fn models_available(settings: &Settings) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(dir) = &settings.models_dir else {
            return false;
        };
        let backend = ustt_pipeline::backend::detect(settings.backend).selected;
        ustt_pipeline::models::resolve_models(dir, backend).is_ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = settings;
        false
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SettingsProps {
    pub lang: Lang,
    pub on_close: EventHandler<()>,
    pub on_saved: EventHandler<Settings>,
}

#[component]
pub fn SettingsScreen(props: SettingsProps) -> Element {
    let service = use_context::<Arc<dyn Service>>();
    let text = strings(props.lang);
    let mut draft = use_signal(|| service.settings());
    let mut error = use_signal(|| None::<String>);
    let mut lang_code = use_signal(|| draft.read().language.clone().unwrap_or_default());
    let current = draft();

    let models_ok = models_available(&current);

    rsx! {
        div { class: "scrim", onclick: move |_| props.on_close.call(()),
            div {
                class: "modal",
                "data-testid": "settings",
                onclick: move |evt| evt.stop_propagation(),
                h2 { "{text.settings}" }
                if let Some(message) = error() {
                    div { class: "banner", span { "{message}" } }
                }
                div { class: "settings-grid",
                    label { "{text.language}" }
                    select {
                        value: "{lang_code}",
                        onchange: move |evt| {
                            let value = evt.value();
                            lang_code.set(value.clone());
                            let mut next = draft();
                            next.language = (!value.is_empty()).then_some(value);
                            draft.set(next);
                        },
                        option { value: "", "Auto" }
                        option { value: "ru", "Русский" }
                        option { value: "en", "English" }
                    }
                    label { "{text.theme}" }
                    select {
                        value: "{draft.read().theme}",
                        onchange: move |evt| {
                            let mut next = draft();
                            next.theme = evt.value();
                            draft.set(next);
                        },
                        option { value: "dark", "{text.theme_dark}" }
                        option { value: "light", "{text.theme_light}" }
                        option { value: "system", "{text.theme_system}" }
                    }
                    label { "{text.backend}" }
                    select {
                        value: "{draft.read().backend:?}",
                        onchange: move |evt| {
                            let mut next = draft();
                            next.backend = match evt.value().to_lowercase().as_str() {
                                "cpu" => BackendPreference::Cpu,
                                "vulkan" => BackendPreference::Vulkan,
                                "cuda" => BackendPreference::Cuda,
                                _ => BackendPreference::Auto,
                            };
                            draft.set(next);
                        },
                        option { value: "Auto", "{text.backend_auto}" }
                        option { value: "Cpu", "CPU" }
                        option { value: "Vulkan", "Vulkan" }
                        option { value: "Cuda", "CUDA" }
                    }
                    label { "{text.threads}" }
                    input {
                        r#type: "number",
                        min: "0",
                        value: "{draft.read().threads}",
                        oninput: move |evt| {
                            let mut next = draft();
                            next.threads = evt.value().parse().unwrap_or(0);
                            draft.set(next);
                        },
                    }
                    label { "{text.autosave}" }
                    input {
                        r#type: "number",
                        min: "1",
                        value: "{draft.read().autosave_secs}",
                        oninput: move |evt| {
                            let mut next = draft();
                            next.autosave_secs = evt.value().parse().unwrap_or(5);
                            draft.set(next);
                        },
                    }
                    label { "{text.ffmpeg_dir}" }
                    input {
                        value: "{draft.read().ffmpeg_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default()}",
                        onchange: move |evt| {
                            let mut next = draft();
                            let value = evt.value();
                            next.ffmpeg_dir = (!value.is_empty()).then(|| value.into());
                            draft.set(next);
                        },
                    }
                    label { "{text.models_dir}" }
                    input {
                        value: "{draft.read().models_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default()}",
                        onchange: move |evt| {
                            let mut next = draft();
                            let value = evt.value();
                            next.models_dir = (!value.is_empty()).then(|| value.into());
                            draft.set(next);
                        },
                    }
                    label { "" }
                    span { class: "meta", "data-testid": "models-status", if models_ok { "{text.models_ok}" } else { "{text.models_missing}" } }
                    label { "" }
                    span { class: "meta", "data-testid": "backend-hint", "{backend_hint()}" }
                }
                div { class: "modal-actions",
                    button { class: "btn ghost", onclick: move |_| props.on_close.call(()), "{text.cancel}" }
                    button {
                        class: "btn primary",
                        "data-testid": "settings-save",
                        onclick: move |_| {
                            let next = draft();
                            match service.save_settings(&next) {
                                Ok(()) => {
                                    props.on_saved.call(next);
                                    props.on_close.call(());
                                }
                                Err(message) => error.set(Some(message)),
                            }
                        },
                        "{text.apply}"
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
    fn missing_project_shows_error() {
        let err = load_project_error(Path::new("/definitely/missing.ustt"));
        assert!(
            err.contains("project.json") || err.contains("missing") || err.contains("не найден")
        );
    }
}
