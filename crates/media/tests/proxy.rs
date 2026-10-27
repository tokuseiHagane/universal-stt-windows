use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::Path;
use std::process::Command;
use ustt_media::{MediaKind, Tools, make_proxy, select_video_encoder};

fn write_sine_wav(path: &Path, seconds: f32) {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec).unwrap();
    for i in 0..(16_000.0 * seconds) as u32 {
        let t = i as f32 / 16_000.0;
        let v = (t * 440.0 * std::f32::consts::TAU).sin() * 0.4;
        w.write_sample((v * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn make_testsrc_mp4(path: &Path) {
    let ok = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=25",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "1",
            "-c:v",
            "mpeg4",
            "-c:a",
            "aac",
            path.to_str().unwrap(),
        ])
        .status()
        .unwrap()
        .success();
    assert!(ok);
}

#[test]
fn audio_proxy_is_m4a_aac() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("a.wav");
    write_sine_wav(&src, 1.0);
    let info = make_proxy(
        &Tools::locate(None).unwrap(),
        &src,
        tmp.path(),
        "proxy",
        MediaKind::Audio,
    )
    .unwrap();
    assert_eq!(info.codec, "aac");
    assert_eq!(info.path.extension().unwrap(), "m4a");
    assert!(info.path.is_file());
}

#[test]
fn video_proxy_uses_an_available_encoder() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("v.mp4");
    make_testsrc_mp4(&src);
    let info = make_proxy(
        &Tools::locate(None).unwrap(),
        &src,
        tmp.path(),
        "proxy",
        MediaKind::Video,
    )
    .unwrap();
    assert!(["h264", "vp9"].contains(&info.codec.as_str()));
    assert!(info.path.is_file());
}

#[test]
fn select_video_encoder_reports_vp9_on_linux() {
    let enc = select_video_encoder(&Tools::locate(None).unwrap()).unwrap();
    assert!(matches!(enc, "h264_mf" | "libx264" | "libvpx-vp9"));
}
