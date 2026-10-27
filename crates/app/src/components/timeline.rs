use super::player;
use dioxus::prelude::*;
use ustt_core::Project;
use ustt_ui_model::{Lang, Playback, TimelineLayout, WaveData, strings, timecode};

pub fn timeline_click_to_time(x: f64, layout: &TimelineLayout) -> f64 {
    layout.time(x)
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
}

#[component]
pub fn Timeline(props: TimelineProps) -> Element {
    let playback = use_context::<Signal<Playback>>();
    let mut layout =
        use_signal(|| TimelineLayout::new(20.0, 900.0, props.project.duration.max(1.0)));
    let mut width = use_signal(|| 900.0f64);
    let text = strings(props.lang);
    let duration = props.project.duration.max(0.001);

    let scene = {
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
        let turns: Vec<serde_json::Value> = props
            .project
            .turns
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
        })
        .to_string()
    };

    use_effect(move || {
        let scene = scene.clone();
        document::eval(&format!(
            "window.usttTimeline && window.usttTimeline.draw(document.getElementById('timeline'), {scene});"
        ));
    });

    use_effect(move || {
        let position = playback.read().position;
        let fraction = (position / duration).clamp(0.0, 1.0);
        document::eval(&format!(
            "window.usttTimeline && window.usttTimeline.setPlayhead('playhead', {fraction});"
        ));
    });

    let mut seek_abs = move |seconds: f64| {
        let mut player = playback;
        player.write().seek(seconds);
        player::seek(seconds);
        props.on_seek.call(seconds);
    };

    let state = *playback.read();
    rsx! {
        div { class: "bottom",
            div { class: "timeline-wrap",
                canvas {
                    id: "timeline",
                    "data-testid": "timeline",
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
                        let x = evt.element_coordinates().x;
                        let seconds = timeline_click_to_time(x, &layout());
                        seek_abs(seconds);
                    },
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
