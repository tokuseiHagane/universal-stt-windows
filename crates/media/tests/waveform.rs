use hound::{SampleFormat, WavSpec, WavWriter};
use ustt_media::{Waveform, waveform_from_samples, waveform_from_wav};

#[test]
fn waveform_buckets_are_20ms() {
    let samples: Vec<f32> = (0..16_000)
        .map(|i| if i < 8_000 { -0.5 } else { 0.75 })
        .collect();
    let w = waveform_from_samples(&samples, 16_000, 20);
    assert_eq!(w.peaks.len(), 50);
    assert!((w.peaks[0].min + 0.5).abs() < 1e-6);
    assert!((w.peaks[49].max - 0.75).abs() < 1e-6);
}

#[test]
fn waveform_from_wav_1h_is_streaming() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("long.wav");
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(&f, spec).unwrap();
    for i in 0..16_000u32 * 3600 {
        w.write_sample(((i % 200) as i16) - 100).unwrap();
    }
    w.finalize().unwrap();
    let wf = waveform_from_wav(&f, 20).unwrap();
    assert_eq!(wf.peaks.len(), 180_000);
}

#[test]
fn waveform_bytes_roundtrip() {
    let w = waveform_from_samples(&[0.0, 1.0, -1.0, 0.5], 16_000, 20);
    assert_eq!(Waveform::from_bytes(&w.to_bytes()).unwrap(), w);
}

#[test]
fn truncated_header_is_an_error() {
    let mut bytes = b"USTTW1".to_vec();
    bytes.extend_from_slice(&[0u8; 10]);
    assert!(Waveform::from_bytes(&bytes).is_err());
}
