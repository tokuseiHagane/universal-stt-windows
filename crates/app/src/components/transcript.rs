use dioxus::prelude::*;
use ustt_core::Project;
use ustt_ui_model::{Lang, Playback, ROW_HEIGHT, active_segment, strings, timecode, virtual_rows};
use uuid::Uuid;

#[derive(Props, Clone, PartialEq)]
pub struct TranscriptProps {
    pub lang: Lang,
    pub project: Project,
    pub selected: Signal<Option<Uuid>>,
    pub on_seek: EventHandler<f64>,
    pub on_edit: EventHandler<(Uuid, String)>,
}

#[component]
pub fn Transcript(props: TranscriptProps) -> Element {
    let text = strings(props.lang);
    let playback = use_context::<Signal<Playback>>();
    let mut scroll_top = use_signal(|| 0.0f64);
    let mut viewport = use_signal(|| 520.0f64);
    let mut autoscroll = use_signal(|| true);
    let mut editing = use_signal(|| None::<Uuid>);
    let mut draft = use_signal(String::new);

    let position = playback.read().position;
    let playing = playback.read().playing;
    let active = active_segment(&props.project, position);
    let rows = virtual_rows(
        scroll_top(),
        viewport(),
        ROW_HEIGHT,
        props.project.segments.len(),
    );

    use_effect(move || {
        if autoscroll()
            && playing
            && let Some(index) = active
        {
            document::eval(&format!(
                "window.usttTimeline && window.usttTimeline.scrollToRow('segment-{index}');"
            ));
        }
    });

    let selected_id = (props.selected)();
    let items: Vec<(usize, ustt_core::Segment, String, f32, bool, f64)> = (rows.first..rows.last)
        .filter_map(|index| {
            let segment = props.project.segments.get(index)?.clone();
            let color = segment
                .speaker_id
                .and_then(|id| props.project.speakers.iter().find(|s| s.id == id))
                .map(|s| format!("var(--speaker-{})", s.color % 8))
                .unwrap_or_else(|| "var(--muted)".into());
            let dim = if selected_id.is_some_and(|id| segment.speaker_id != Some(id)) {
                0.45
            } else {
                1.0
            };
            let top = index as f64 * ROW_HEIGHT + rows.offset_px;
            Some((index, segment, color, dim, active == Some(index), top))
        })
        .collect();

    rsx! {
        div { class: "transcript-panel",
            div { class: "transcript-head",
                span { "{text.transcript}" }
                if !autoscroll() {
                    button {
                        class: "btn ghost small",
                        onclick: move |_| autoscroll.set(true),
                        "{text.current_segment}"
                    }
                }
            }
            div {
                class: "transcript-scroll",
                "data-testid": "transcript",
                onmounted: move |evt| {
                    spawn(async move {
                        if let Ok(rect) = evt.get_client_rect().await {
                            viewport.set(rect.height());
                        }
                    });
                },
                onscroll: move |evt| {
                    scroll_top.set(evt.scroll_top());
                },
                onwheel: move |_| autoscroll.set(false),
                div { style: "position: relative; height: {rows.total_px}px;",
                    for (index, segment, color, dim, is_active, top) in items {
                        {
                            let segment_id = segment.id;
                            let start = segment.start;
                            let row_text = segment.text.clone();
                            let editing_this = editing() == Some(segment_id);
                            rsx! {
                                div {
                                    key: "{segment_id}",
                                    id: "segment-{index}",
                                    class: if is_active { "transcript-row active" } else { "transcript-row" },
                                    "data-testid": "segment-row",
                                    style: "position: absolute; top: {top}px; left: 0; right: 0; opacity: {dim};",
                                    onclick: move |_| {
                                        autoscroll.set(true);
                                        props.on_seek.call(start);
                                    },
                                    ondoubleclick: move |evt| {
                                        evt.stop_propagation();
                                        draft.set(row_text.clone());
                                        editing.set(Some(segment_id));
                                    },
                                    span { class: "time", "{timecode(start)}" }
                                    span { class: "dot", style: "background: {color}" }
                                    if editing_this {
                                        textarea {
                                            "data-testid": "segment-text",
                                            value: "{draft}",
                                            autofocus: true,
                                            oninput: move |evt| draft.set(evt.value()),
                                            onclick: move |evt| evt.stop_propagation(),
                                            onblur: move |_| {
                                                let value = draft();
                                                props.on_edit.call((segment_id, value));
                                                editing.set(None);
                                            },
                                            onkeydown: move |evt| {
                                                if evt.key() == Key::Enter && !evt.modifiers().shift() {
                                                    evt.prevent_default();
                                                    let value = draft();
                                                    props.on_edit.call((segment_id, value));
                                                    editing.set(None);
                                                }
                                                if evt.key() == Key::Escape {
                                                    editing.set(None);
                                                }
                                            },
                                        }
                                    } else {
                                        div { class: "text", "data-testid": "segment-text", "{row_text}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
