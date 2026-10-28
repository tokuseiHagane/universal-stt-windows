use super::player;
use dioxus::prelude::*;
use ustt_core::{Project, Turn};
use ustt_ui_model::{
    Lang, Playback, TimelineLayout, WaveData, best_segment_for_turn, strings, timecode,
};

pub const TIMELINE_WAVE_HEIGHT: f64 = 92.0;
pub const SPEAKER_LANE_HEIGHT: f64 = 28.0;

pub fn timeline_click_to_time(x: f64, layout: &TimelineLayout) -> f64 {
    layout.time(x)
}

pub fn timeline_turn_at(x: f64, layout: &TimelineLayout, turns: &[Turn]) -> Option<usize> {
    turns.iter().position(|turn| {
        x >= layout.x(turn.start).min(layout.x(turn.end))
            && x <= layout.x(turn.start).max(layout.x(turn.end))
    })
}

pub fn canvas_peaks(peaks: &[ustt_ui_model::WavePoint], width_px: usize) -> Vec<(f32, f32)> {
    if width_px == 0 || peaks.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(width_px);
    for column in 0..width_px {
        let start = column * peaks.len() / width_px;
        let end = ((column + 1) * peaks.len() / width_px).max(start + 1);
        let slice = &peaks[start.min(peaks.len() - 1)..end.min(peaks.len())];
        let mut min = 0.0f32;
        let mut max = 0.0f32;
        for point in slice {
            min = min.min(point.min);
            max = max.max(point.max);
        }
        out.push((min, max));
    }
    out
}

#[derive(Props, Clone, PartialEq)]
pub struct TimelineProps {
    pub lang: Lang,
    pub project: Project,
    pub wave: WaveData,
    pub on_seek: EventHandler<f64>,
    pub on_turn: EventHandler<usize>,
}

#[component]
pub fn Timeline(props: TimelineProps) -> Element {
    let playback = use_context::<Signal<Playback>>();
    let mut layout =
        use_signal(|| TimelineLayout::new(20.0, 900.0, props.project.duration.max(1.0)));
    let mut width = use_signal(|| 900.0f64);
    let mut lane_tooltip = use_signal(String::new);
    let text = strings(props.lang);
    let duration = props.project.duration.max(0.001);
    let scene_turns = props.project.turns.clone();
    let click_turns = props.project.turns.clone();
    let click_segments = props.project.segments.clone();
    let hover_turns = props.project.turns.clone();
    let hover_speakers = props.project.speakers.clone();

    let scene = use_memo(move || {
        let layout = layout();
        let width = width();
        let peaks = canvas_peaks(&props.wave.peaks, width.max(1.0) as usize);
        let wave: Vec<serde_json::Value> = peaks
            .iter()
            .enumerate()
            .map(|(i, (min, max))| {
                serde_json::json!({
                    "x": i as f64 / width.max(1.0),
                    "min": min,
                    "max": max,
                })
            })
            .collect();
        let turns: Vec<serde_json::Value> = scene_turns
            .iter()
            .map(|turn| {
                serde_json::json!({
                    "x0": (layout.x(turn.start) / width.max(1.0)).clamp(0.0, 1.0),
                    "x1": (layout.x(turn.end) / width.max(1.0)).clamp(0.0, 1.0),
                    "color": ustt_core::color_for_index(turn.speaker_index as usize),
                    "active": false,
                })
            })
            .collect();
        serde_json::json!({
            "peaks": wave,
            "turns": turns,
            "barWidth": 1.5,
            "speakerLaneHeight": SPEAKER_LANE_HEIGHT,
        })
        .to_string()
    });

    use_effect(move || {
        let duration = props.project.duration.max(0.001);
        let w = width();
        layout.set(TimelineLayout::new(w / duration, w, duration));
    });

    use_effect(move || {
        let scene = scene();
        let code = format!(
            "window.usttTimeline && window.usttTimeline.draw(document.getElementById('timeline'), {scene});"
        );
        spawn(async move {
            let _ = document::eval(&code).await;
        });
    });

    use_effect(move || {
        let position = playback.read().position;
        let fraction = (position / duration).clamp(0.0, 1.0);
        let code = format!(
            "window.usttTimeline && window.usttTimeline.setPlayhead('playhead', {fraction});"
        );
        spawn(async move {
            let _ = document::eval(&code).await;
        });
    });

    let seek_abs = move |seconds: f64| {
        props.on_seek.call(seconds);
    };

    let state = *playback.read();
    rsx! {
        div { class: "bottom",
            div { class: "timeline-wrap",
                canvas {
                    id: "timeline",
                    "data-testid": "timeline",
                    title: "{lane_tooltip}",
                    onmounted: move |evt| {
                        spawn(async move {
                            if let Ok(rect) = evt.get_client_rect().await {
                                width.set(rect.width());
                                let px = rect.width() / duration;
                                layout.set(TimelineLayout::new(px, rect.width(), duration));
                            }
                        });
                    },
                    onclick: move |evt| {
                        let point = evt.element_coordinates();
                        if point.y >= TIMELINE_WAVE_HEIGHT {
                            if let Some(turn_index) = timeline_turn_at(point.x, &layout(), &click_turns) {
                                let turn = click_turns[turn_index];
                                seek_abs(turn.start);
                                if let Some(segment) = best_segment_for_turn(&click_segments, &turn) {
                                    props.on_turn.call(segment);
                                }
                            }
                            return;
                        }
                        let seconds = timeline_click_to_time(point.x, &layout());
                        seek_abs(seconds);
                    },
                    onmousemove: move |evt| {
                        let point = evt.element_coordinates();
                        if point.y < TIMELINE_WAVE_HEIGHT {
                            lane_tooltip.set(String::new());
                            return;
                        }
                        let tooltip = timeline_turn_at(point.x, &layout(), &hover_turns)
                            .and_then(|index| hover_turns.get(index))
                            .map(|turn| {
                                let name = hover_speakers.get(turn.speaker_index as usize)
                                    .map(|speaker| speaker.name.clone())
                                    .unwrap_or_else(|| format!("#{}", turn.speaker_index + 1));
                                format!("{name} · {}–{}", timecode(turn.start), timecode(turn.end))
                            });
                        lane_tooltip.set(tooltip.unwrap_or_default());
                    },
                    onmouseleave: move |_| lane_tooltip.set(String::new()),
                    onwheel: move |evt| {
                        evt.prevent_default();
                        let delta = evt.delta().strip_units().y;
                        let mut next = layout();
                        if evt.modifiers().shift() {
                            next.scroll_by(if delta < 0.0 { -2.0 } else { 2.0 });
                        } else {
                            next.zoom_at(evt.element_coordinates().x, if delta < 0.0 { 1.2 } else { 0.8 });
                        }
                        layout.set(next);
                    },
                }
                div { class: "playhead", id: "playhead", "data-testid": "playhead" }
            }
            div { class: "transport",
                button {
                    class: "icon-button",
                    "data-testid": "play-toggle",
                    title: if state.playing { text.pause } else { text.play },
                    onclick: move |_| {
                        if state.playing { player::pause(); } else { player::play(); }
                    },
                    if state.playing { "⏸" } else { "▶" }
                }
                span { class: "time", "{timecode(state.position)} / {timecode(state.duration)}" }
                span { class: "spacer" }
                span { class: "time", "{text.rate}" }
                select {
                    "data-testid": "rate",
                    value: "{state.rate}",
                    onchange: move |evt| {
                        let rate: f64 = evt.value().parse().unwrap_or(1.0);
                        player::set_rate(rate);
                    },
                    option { value: "0.75", "0.75×" }
                    option { value: "1", "1×" }
                    option { value: "1.25", "1.25×" }
                    option { value: "1.5", "1.5×" }
                    option { value: "2", "2×" }
                }
                input {
                    class: "volume",
                    r#type: "range",
                    min: "0",
                    max: "1",
                    step: "0.01",
                    value: "{state.volume}",
                    oninput: move |evt| {
                        let volume: f64 = evt.value().parse().unwrap_or(1.0);
                        player::set_volume(volume);
                    },
                }
                button {
                    class: "icon-button",
                    title: text.mute,
                    onclick: move |_| player::set_muted(!state.muted),
                    if state.muted { "🔇" } else { "🔊" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_click_maps_to_time() {
        let layout = TimelineLayout::new(10.0, 600.0, 60.0);
        assert!((timeline_click_to_time(150.0, &layout) - 15.0).abs() < 1e-9);
    }

    #[test]
    fn timeline_payload_is_decimated() {
        let peaks: Vec<ustt_ui_model::WavePoint> = (0..100_000)
            .map(|i| ustt_ui_model::WavePoint {
                min: -0.5,
                max: 0.5 + i as f32 * 0.0,
            })
            .collect();
        assert_eq!(canvas_peaks(&peaks, 800).len(), 800);
    }
}
