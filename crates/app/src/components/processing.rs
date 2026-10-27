use dioxus::prelude::*;
use ustt_ui_model::{Lang, PhaseView, ProgressView, strings};

pub fn phase_label(phase: PhaseView, lang: Lang) -> &'static str {
    let text = strings(lang);
    match phase {
        PhaseView::Probe => text.phase_probe,
        PhaseView::Decode => text.phase_decode,
        PhaseView::Proxy => text.phase_proxy,
        PhaseView::Diarize => text.phase_diarize,
        PhaseView::Transcribe => text.phase_transcribe,
        PhaseView::Merge => text.phase_merge,
        PhaseView::Done => text.phase_done,
    }
}

fn eta_value(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    if total >= 60 {
        format!("{}:{:02}", total / 60, total % 60)
    } else {
        format!("{total} s")
    }
}

pub fn eta_text(eta: Option<f64>, lang: Lang) -> String {
    match eta {
        Some(seconds) => format!("{}: {}", strings(lang).eta_prefix, eta_value(seconds)),
        None => format!("{}: —", strings(lang).eta_prefix),
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ProcessingProps {
    pub lang: Lang,
    pub file_name: String,
    pub progress: Option<ProgressView>,
    pub error: Option<String>,
    pub on_cancel: EventHandler<()>,
    pub on_back: EventHandler<()>,
}

#[component]
pub fn Processing(props: ProcessingProps) -> Element {
    let text = strings(props.lang);
    let percent = props
        .progress
        .as_ref()
        .map(|p| (p.fraction * 100.0).round().clamp(0.0, 100.0))
        .unwrap_or(0.0);
    let phase = props
        .progress
        .as_ref()
        .map(|p| phase_label(p.phase, props.lang))
        .unwrap_or(text.processing_title);

    rsx! {
        section { class: "processing", "data-testid": "processing",
            div { class: "processing-card",
                h2 { "{text.processing_title}" }
                div { class: "processing-file", "{props.file_name}" }
                if let Some(message) = props.error.clone() {
                    div { class: "banner",
                        span { "{message}" }
                    }
                    div { class: "modal-actions",
                        button { class: "btn", onclick: move |_| props.on_back.call(()), "{text.processing_back}" }
                    }
                } else {
                    div { class: "progress-meta",
                        span { "{phase}" }
                        span { "{percent:.0}%" }
                    }
                    div { class: "progress-track",
                        div { class: "progress-fill", style: "width: {percent}%" }
                    }
                    div { class: "progress-meta",
                        span { "{props.progress.as_ref().map(|p| p.detail.clone()).unwrap_or_default()}" }
                        span { "{eta_text(props.progress.as_ref().and_then(|p| p.eta_seconds), props.lang)}" }
                    }
                    div { class: "modal-actions",
                        button {
                            class: "btn danger",
                            "data-testid": "cancel-import",
                            onclick: move |_| props.on_cancel.call(()),
                            "{text.processing_cancel}"
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
    fn phase_label_covers_every_phase() {
        for phase in [
            PhaseView::Probe,
            PhaseView::Decode,
            PhaseView::Proxy,
            PhaseView::Diarize,
            PhaseView::Transcribe,
            PhaseView::Merge,
            PhaseView::Done,
        ] {
            assert!(!phase_label(phase, Lang::Ru).is_empty());
            assert!(!phase_label(phase, Lang::En).is_empty());
        }
    }

    #[test]
    fn eta_text_is_optional() {
        assert_eq!(eta_text(None, Lang::Ru), "Осталось: —");
        assert!(eta_text(Some(61.0), Lang::Ru).contains("1"));
    }
}
