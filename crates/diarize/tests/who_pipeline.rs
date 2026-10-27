use std::path::PathBuf;
use ustt_diarize::cluster;
use ustt_diarize::decision::{Decision, Rule, Stretch};
use ustt_diarize::embedding::Embedding;
use ustt_diarize::model::WhoModel;
use ustt_diarize::pieces;

#[test]
fn decision_applies_mask_rule_and_pooling() {
    let d = Decision {
        presence_threshold: 0.8,
        mask_threshold: 0.5,
        rule: Rule::Mask,
        pool: 1,
    };
    let s = d.stretches(&[0.9, 0.1], &[0.0, 0.6, 0.7, 0.0, 0.9, 0.0], 3);
    assert_eq!(
        s,
        vec![Stretch {
            query: 0,
            first: 1,
            end: 3
        }]
    );
}

#[test]
fn pieces_are_300s_end_to_end() {
    assert_eq!(pieces::cut(30_000, 30_000), vec![0..30_000]);
    assert_eq!(pieces::cut(30_001, 30_000), vec![0..30_000, 30_000..30_001]);
}

#[test]
fn average_linkage_merges_close_vectors() {
    let labels =
        cluster::average_linkage(&[vec![1.0, 0.0], vec![0.99, 0.01], vec![0.0, 1.0]], 0.65);
    assert_eq!(labels[0], labels[1]);
    assert_ne!(labels[0], labels[2]);
}

#[test]
fn embedding_sidecar_parses_header() {
    let bytes = embedding_sidecar(2, 4, &[0u16; 10]);
    let e = Embedding::from_sidecar(&bytes).unwrap();
    assert_eq!((e.out, e.columns), (2, 2));
}

fn embedding_sidecar(out: u32, inner: u32, values: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&out.to_le_bytes());
    bytes.extend_from_slice(&inner.to_le_bytes());
    for &value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn models_dir() -> PathBuf {
    let raw = std::env::var("USTT_MODELS_DIR").unwrap_or_else(|_| "models".into());
    if PathBuf::from(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(raw)
    }
}

fn harmonic_signal(seconds: f64, sample_rate: u32) -> Vec<f32> {
    let total = (seconds * sample_rate as f64) as usize;
    (0..total)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            let tau = std::f64::consts::TAU;
            (0.5 * (tau * 220.0 * t).sin() + 0.2 * (tau * 660.0 * t).sin()) as f32
        })
        .collect()
}

#[test]
#[ignore = "needs models"]
fn diarize_700s_streams_multiple_pieces() {
    let mut model = WhoModel::load(models_dir().join("who")).unwrap();
    let samples = harmonic_signal(700.0, 16_000);
    let mut last = 0.0f32;
    let mut piece_ticks = 0;
    let result = model
        .diarize(&samples, 16_000, &mut |p| {
            last = p;
            if p <= 0.6001 {
                piece_ticks += 1;
            }
        })
        .unwrap();
    assert!(last >= 0.999);
    assert!(
        piece_ticks >= 3,
        "expected several pieces, saw {piece_ticks}"
    );
    assert!((result.duration - 700.0).abs() < 1.0);
    for turn in &result.turns {
        assert!(turn.start >= 0.0 && turn.end <= 701.0 && turn.end > turn.start);
    }
}

#[test]
#[ignore = "needs models and a two-speaker recording"]
fn diarize_two_speakers_fixture() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/two_speakers.wav");
    if !fixture.is_file() {
        eprintln!("skipping: {} is absent", fixture.display());
        return;
    }
    let mut model = WhoModel::load(models_dir().join("who")).unwrap();
    let samples = ustt_media::read_wav_mono16k(&fixture).unwrap();
    let result = model.diarize(&samples, 16_000, &mut |_| {}).unwrap();
    assert_eq!(result.speaker_count, 2);
    assert!(result.turns.windows(2).all(|w| w[0].start <= w[1].start));
}
