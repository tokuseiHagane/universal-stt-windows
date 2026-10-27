use std::path::{Path, PathBuf};
use std::process::Command;
use ustt_media::{MediaError, Tools, probe};

fn make_mp4(dir: &Path) -> PathBuf {
    let out = dir.join("a.mp4");
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
            "2",
            "-c:v",
            "mpeg4",
            "-c:a",
            "aac",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap()
        .success();
    assert!(ok);
    out
}

#[test]
fn probe_reports_video_and_audio() {
    let tmp = tempfile::tempdir().unwrap();
    let tools = Tools::locate(None).unwrap();
    let info = probe(&tools, &make_mp4(tmp.path())).unwrap();
    assert!((info.duration - 2.0).abs() < 0.2);
    assert!(info.video.is_some());
    assert_eq!(info.audio.unwrap().channels, 1);
}

#[test]
fn probe_rejects_truncated_file() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("bad.mp4");
    std::fs::write(&f, b"not a video at all").unwrap();
    assert!(matches!(
        probe(&Tools::locate(None).unwrap(), &f),
        Err(MediaError::Probe(_))
    ));
}

#[test]
fn locate_honours_explicit_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    for name in ["ffmpeg", "ffprobe"] {
        let p = tmp.path().join(format!("{name}{exe}"));
        std::fs::write(&p, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let tools = Tools::locate(Some(tmp.path())).unwrap();
    assert_eq!(tools.ffmpeg, tmp.path().join(format!("ffmpeg{exe}")));
}
