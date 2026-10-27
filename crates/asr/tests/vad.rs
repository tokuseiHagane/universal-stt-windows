use ustt_asr::TimeRange;
use ustt_asr::chunk_ranges;

#[test]
fn empty_vad_falls_back_to_fixed_windows() {
    let r = chunk_ranges(&[], 30.0, 65.0);
    assert_eq!(r.len(), 3);
    assert!((r[2].end - 65.0).abs() < 1e-9);
    assert!(r.windows(2).all(|w| w[0].end <= w[1].start + 1e-9));
}

#[test]
fn long_speech_is_split_at_30s() {
    let r = chunk_ranges(
        &[TimeRange {
            start: 0.0,
            end: 75.0,
        }],
        30.0,
        75.0,
    );
    assert_eq!(
        r,
        vec![
            TimeRange {
                start: 0.0,
                end: 30.0
            },
            TimeRange {
                start: 30.0,
                end: 60.0
            },
            TimeRange {
                start: 60.0,
                end: 75.0
            },
        ]
    );
}

#[test]
#[ignore = "needs models"]
fn silence_yields_no_segments() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use ustt_asr::{AsrEngine, ParakeetLib, VadEngine, transcribe_chunks};

    let models = std::env::var("USTT_MODELS_DIR").unwrap_or_else(|_| "models".into());
    let models = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(models);
    let lib_path = std::env::var("PARAKEET_LIB").unwrap_or_else(|_| "lib/libparakeet.so".into());
    let lib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(lib_path);
    let lib = Arc::new(ParakeetLib::open(&lib_path).unwrap());
    let mut asr = AsrEngine::load(lib.clone(), &models.join("redux-packed.gguf")).unwrap();
    let mut vad = VadEngine::load(lib, &models.join("silero-vad-f16.gguf")).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("silence.wav");
    ustt_media::write_wav_mono16k(&wav, &vec![0.0; 16_000 * 5]).unwrap();
    let out = transcribe_chunks(&mut asr, &mut vad, &wav, 30.0, &mut |_| {}).unwrap();
    assert!(out.text.trim().is_empty(), "got {:?}", out.text);
    assert!(out.words.is_empty());
}

#[test]
#[ignore = "needs models"]
fn transcribes_speech_with_words() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use ustt_asr::{AsrEngine, ParakeetLib, VadEngine, transcribe_chunks};

    let models = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models");
    let lib_path = std::env::var("PARAKEET_LIB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../lib/libparakeet.so")
        });
    let lib = Arc::new(ParakeetLib::open(&lib_path).unwrap());
    let mut asr = AsrEngine::load(lib.clone(), &models.join("redux-packed.gguf")).unwrap();
    let mut vad = VadEngine::load(lib, &models.join("silero-vad-f16.gguf")).unwrap();

    let wav =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/two_speakers.wav");
    let mut last = 0.0f32;
    let out = transcribe_chunks(&mut asr, &mut vad, &wav, 10.0, &mut |p| last = p).unwrap();
    assert!(last >= 0.999);
    assert!(!out.words.is_empty(), "no words: {:?}", out.text);
    assert!(!out.text.trim().is_empty());
    assert!(
        out.words
            .windows(2)
            .all(|w| w[0].start <= w[1].start + 1e-6)
    );
}
