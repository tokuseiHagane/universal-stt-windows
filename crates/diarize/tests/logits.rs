use serde::Deserialize;
use std::path::PathBuf;
use ustt_diarize::meta::Meta;
use ustt_diarize::session::Sessions;
use ustt_diarize::{Frontend, compute_logits, concat_mask_chunks};

const MINIMAL_META: &str = r#"{
  "schema": 2,
  "model": {"architecture": "eend_m2f", "dim": 256, "queries": 50},
  "audio": {"sample_rate": 16000},
  "frontend": {"frame_samples": 400, "shift_samples": 160, "n_fft": 512,
               "n_mels": 23, "preemphasis": 0.97, "scale": 32768.0,
               "log_floor": 1.1920928955078125e-07},
  "frames": {"downsample": 10},
  "windows": [{
    "seconds": 60.0, "frames": 6000, "low": 600, "chunks": [6000],
    "encoder": {"inputs": ["mel"], "outputs": ["memory", "frames"]},
    "queries": {"inputs": ["memory", "frames_0"],
                "outputs": ["presence", "queries", "masks_0"]}
  }],
  "voice": {"mel_frames": 3256, "mel_frames_per_column": 8}
}"#;

#[test]
fn mask_chunks_concatenate_without_padding() {
    let masks = concat_mask_chunks(&[vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0]], &[4, 2], 5, 1);
    assert_eq!(masks, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn meta_rejects_window_without_chunks() {
    let mut meta: Meta = serde_json::from_str(MINIMAL_META).unwrap();
    meta.windows[0].chunks = vec![];
    assert!(meta.validate().is_err());
}

#[derive(Deserialize)]
struct LogitsGolden {
    frames: usize,
    presence: Vec<f32>,
    mask_row_means: Vec<f32>,
    mask_spots: Vec<Spot>,
}

#[derive(Deserialize)]
struct Spot {
    query: usize,
    frame: usize,
    value: f32,
}

fn models_dir() -> PathBuf {
    let raw = std::env::var("WHO_MODEL_DIR").unwrap_or_else(|_| "models/who".into());
    if PathBuf::from(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(raw)
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
#[ignore = "needs models and onnxruntime"]
fn logits_match_onnx_reference() {
    let golden: LogitsGolden =
        serde_json::from_str(include_str!("../../../tests/fixtures/who_logits.json")).unwrap();
    let dir = models_dir();
    let meta = Meta::load(&dir).unwrap();
    meta.validate().unwrap();
    let banks = std::fs::read(dir.join(&meta.frontend.banks)).unwrap();
    let frontend = Frontend::new(meta.geometry(), &banks).unwrap();
    let mut sessions = Sessions::load(&dir, &meta).unwrap();

    let samples = reference_signal(100.0, meta.audio.sample_rate);
    let (means, frames) = frontend.means(&samples);
    assert_eq!(frames, golden.frames);
    let logits =
        compute_logits(&mut sessions, &meta, &frontend, &samples, 0..frames, &means).unwrap();

    for (i, (a, b)) in logits.presence.iter().zip(&golden.presence).enumerate() {
        assert!((a - b).abs() < 1e-3, "presence[{i}]: {a} vs {b}");
    }

    let queries = meta.model.queries;
    for q in 0..queries {
        let row = &logits.masks[q * frames..(q + 1) * frames];
        let mean = row.iter().sum::<f32>() / frames as f32;
        assert!(
            (mean - golden.mask_row_means[q]).abs() < 1e-2,
            "mask mean[{q}]: {mean} vs {}",
            golden.mask_row_means[q]
        );
    }

    for spot in &golden.mask_spots {
        let got = logits.masks[spot.query * frames + spot.frame];
        assert!(
            (got - spot.value).abs() < 5e-2,
            "spot q{} f{}: {got} vs {}",
            spot.query,
            spot.frame,
            spot.value
        );
    }
}
