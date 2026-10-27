use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::Path;
use std::process::Command;
use ustt_media::{Tools, decode_wav16k, read_wav_mono16k};

fn write_stereo_48k(path: &Path, seconds: f32) {
    let spec = WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).unwrap();
    let total = (48_000.0 * seconds) as u32;
    for i in 0..total {
        let t = i as f32 / 48_000.0;
        let v = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
        let s = (v * 32767.0) as i16;
        writer.write_sample(s).unwrap();
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
}

#[test]
fn decode_resamples_48k_stereo() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("s.wav");
    write_stereo_48k(&src, 1.0);
    let dst = tmp.path().join("out.wav");
    decode_wav16k(&Tools::locate(None).unwrap(), &src, &dst).unwrap();
    let s = read_wav_mono16k(&dst).unwrap();
    assert!((s.len() as i64 - 16_000).abs() <= 1);
    assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
}

#[test]
fn decode_rejects_missing_input() {
    let tmp = tempfile::tempdir().unwrap();
    let dst = tmp.path().join("out.wav");
    let result = decode_wav16k(
        &Tools::locate(None).unwrap(),
        &tmp.path().join("missing.wav"),
        &dst,
    );
    assert!(result.is_err());
}

#[allow(dead_code)]
fn ffmpeg_exists() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
