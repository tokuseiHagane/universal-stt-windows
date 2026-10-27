use dioxus::prelude::*;
use serde::Deserialize;
use ustt_ui_model::Playback;

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerTick {
    pub position: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub rate: f64,
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub muted: bool,
}

const BRIDGE: &str = r#"
(async () => {
  window.__usttBridgeGen = (window.__usttBridgeGen || 0) + 1;
  const gen = window.__usttBridgeGen;
  const el = () => document.getElementById("player");
  while (window.__usttBridgeGen === gen) {
    const media = el();
    if (media) {
      dioxus.send({
        position: media.currentTime || 0,
        duration: Number.isFinite(media.duration) ? media.duration : 0,
        playing: !media.paused && !media.ended,
        rate: media.playbackRate || 1,
        volume: media.volume,
        muted: media.muted,
      });
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
})();
"#;

pub fn bind(playback: Signal<Playback>) {
    let mut playback = playback;
    spawn(async move {
        let mut eval = document::eval(BRIDGE);
        while let Ok(tick) = eval.recv::<PlayerTick>().await {
            let mut state = playback.write();
            state.position = tick.position;
            if tick.duration > 0.0 {
                state.duration = tick.duration;
            }
            state.playing = tick.playing;
            if tick.rate > 0.0 {
                state.rate = tick.rate;
            }
            state.volume = tick.volume;
            state.muted = tick.muted;
        }
    });
}

pub fn eval_player(script: &str) {
    let code = format!(
        "(() => {{ const el = document.getElementById('player'); if (!el) return; {script} }})();"
    );
    spawn(async move {
        let _ = document::eval(&code).await;
    });
}

#[cfg(target_arch = "wasm32")]
fn with_media(apply: impl FnOnce(&web_sys::HtmlMediaElement)) {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(element) = document.get_element_by_id("player") else {
        return;
    };
    if let Ok(media) = element.dyn_into::<web_sys::HtmlMediaElement>() {
        apply(&media);
    }
}

pub fn seek(seconds: f64) {
    #[cfg(target_arch = "wasm32")]
    {
        with_media(|media| media.set_current_time(seconds));
    }
    #[cfg(not(target_arch = "wasm32"))]
    eval_player(&format!(
        "if (el.readyState > 0) {{ el.currentTime = {seconds}; }} else {{ el.addEventListener('loadedmetadata', () => {{ el.currentTime = {seconds}; }}, {{ once: true }}); }}"
    ));
}

pub fn play() {
    #[cfg(target_arch = "wasm32")]
    {
        with_media(|media| {
            let _ = media.play();
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    eval_player("el.play().catch(() => {});");
}

pub fn pause() {
    #[cfg(target_arch = "wasm32")]
    {
        with_media(|media| {
            let _ = media.pause();
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    eval_player("el.pause();");
}

pub fn set_rate(rate: f64) {
    #[cfg(target_arch = "wasm32")]
    {
        with_media(|media| media.set_playback_rate(rate));
    }
    #[cfg(not(target_arch = "wasm32"))]
    eval_player(&format!("el.playbackRate = {rate};"));
}

pub fn set_volume(volume: f64) {
    #[cfg(target_arch = "wasm32")]
    {
        with_media(|media| media.set_volume(volume));
    }
    #[cfg(not(target_arch = "wasm32"))]
    eval_player(&format!("el.volume = {volume};"));
}

pub fn set_muted(muted: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        with_media(|media| media.set_muted(muted));
    }
    #[cfg(not(target_arch = "wasm32"))]
    eval_player(&format!("el.muted = {muted};"));
}
