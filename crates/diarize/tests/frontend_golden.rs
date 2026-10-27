use serde::Deserialize;
use std::path::PathBuf;
use ustt_diarize::{Frontend, FrontendGeometry};

#[derive(Deserialize)]
struct Golden {
    frames: usize,
    means: Vec<f32>,
    spots: Vec<Spot>,
}

#[derive(Deserialize, Debug)]
struct Spot {
    mel: usize,
    frame: usize,
    value: f32,
}

fn geometry() -> FrontendGeometry {
    FrontendGeometry {
        sample_rate: 16_000,
        frame: 400,
        shift: 160,
        n_fft: 512,
        mels: 23,
        preemphasis: 0.97,
        scale: 32_768.0,
        log_floor: 2.0f32.powi(-23),
    }
}

fn reference_signal(seconds: f64, sample_rate: u32) -> Vec<f32> {
    let total = (seconds * sample_rate as f64) as usize;
    (0..total)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            let tau = std::f64::consts::TAU;
            (0.5 * (tau * 440.0 * t).sin()
                + 0.25 * (tau * 1234.5 * t).sin()
                + 0.05 * (tau * 97.0 * t).sin()) as f32
        })
        .collect()
}

#[test]
fn frontend_matches_reference() {
    let g: Golden =
        serde_json::from_str(include_str!("../../../tests/fixtures/who_frontend.json")).unwrap();
    let raw = std::env::var("WHO_MEL_BANKS")
        .expect("run with WHO_MEL_BANKS=models/who/who_mel_banks.f32");
    let path = if PathBuf::from(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(raw)
    };
    let banks = std::fs::read(path).unwrap();
    let f = Frontend::new(geometry(), &banks).unwrap();
    let signal = reference_signal(30.0, 16_000);
    let (means, frames) = f.means(&signal);
    assert_eq!(frames, g.frames);
    for (a, b) in means.iter().zip(&g.means) {
        assert!((a - b).abs() < 1e-4, "{a} vs {b}");
    }
    let feats = f.features(&signal, 0..frames, &means);
    for s in &g.spots {
        let got = feats[s.frame * 23 + s.mel];
        assert!((got - s.value).abs() < 1e-3, "spot {s:?}: {got}");
    }
}
