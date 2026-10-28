use dioxus::prelude::*;
use ustt_core::Project;
use ustt_ui_model::{Lang, speaker_time, strings, timecode};
use uuid::Uuid;

#[derive(Props, Clone, PartialEq)]
pub struct SpeakerBarProps {
    pub lang: Lang,
    pub project: Project,
    pub selected: Signal<Option<Uuid>>,
    pub on_rename: EventHandler<(Uuid, String)>,
}

#[component]
pub fn SpeakerBar(props: SpeakerBarProps) -> Element {
    let text = strings(props.lang);
    rsx! {
        div { class: "speaker-bar", title: text.speaker_filter_hint,
            span { class: "section-label", "{text.speakers}" }
            for speaker in props.project.speakers.iter().cloned() {
                {
                    let id = speaker.id;
                    let color = format!("var(--speaker-{})", speaker.color % 8);
                    let selected = props.selected;
                    let active = selected() == Some(id);
                    let filtering = selected().is_some();
                    rsx! {
                        div {
                            class: if !filtering || active { "chip" } else { "chip dim" },
                            "data-testid": "speaker-chip",
                            onclick: move |_| {
                                let mut selected = selected;
                                let current = selected();
                                selected.set(if current == Some(id) { None } else { Some(id) });
                            },
                            span { class: "dot", style: "background: {color}" }
                            input {
                                "data-testid": "speaker-name-input",
                                value: "{speaker.name}",
                                onclick: move |evt| evt.stop_propagation(),
                                onchange: move |evt| {
                                    props.on_rename.call((id, evt.value()));
                                },
                                onkeydown: move |evt| {
                                    if evt.key() == Key::Enter {
                                        evt.prevent_default();
                                        document::eval("document.activeElement && document.activeElement.blur()");
                                    }
                                },
                            }
                            span { class: "time", "{timecode(speaker_time(&props.project, id))}" }
                        }
                    }
                }
            }
        }
    }
}
