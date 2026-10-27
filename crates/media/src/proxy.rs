use crate::MediaError;
use crate::probe::probe;
use crate::tools::Tools;
use std::path::{Path, PathBuf};
use std::process::Command;
use ustt_core::MediaKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyInfo {
    pub path: PathBuf,
    pub codec: String,
}

pub fn select_video_encoder(tools: &Tools) -> Result<&'static str, MediaError> {
    let output = Command::new(&tools.ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()?;
    if !output.status.success() {
        return Err(MediaError::Decode(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let list = String::from_utf8_lossy(&output.stdout);
    for candidate in ["h264_mf", "libx264", "libvpx-vp9"] {
        if list.contains(candidate) {
            return Ok(candidate);
        }
    }
    Err(MediaError::Decode(
        "no usable H.264/VP9 encoder in ffmpeg".into(),
    ))
}

pub fn make_proxy(
    tools: &Tools,
    source: &Path,
    out_dir: &Path,
    stem: &str,
    kind: MediaKind,
) -> Result<ProxyInfo, MediaError> {
    std::fs::create_dir_all(out_dir)?;
    let (path, encoder) = match kind {
        MediaKind::Audio => (out_dir.join(format!("{stem}.m4a")), None),
        MediaKind::Video => {
            let encoder = select_video_encoder(tools)?;
            let name = if encoder == "libvpx-vp9" {
                format!("{stem}.webm")
            } else {
                format!("{stem}.mp4")
            };
            (out_dir.join(name), Some(encoder))
        }
    };

    let mut cmd = Command::new(&tools.ffmpeg);
    cmd.args(["-y", "-i"]).arg(source);
    match encoder {
        None => {
            cmd.args(["-vn", "-c:a", "aac", "-b:a", "128k"]);
        }
        Some("libvpx-vp9") => {
            cmd.args([
                "-c:v",
                "libvpx-vp9",
                "-b:v",
                "2M",
                "-deadline",
                "realtime",
                "-cpu-used",
                "8",
            ]);
            cmd.args(["-c:a", "aac", "-b:a", "128k"]);
        }
        Some("libx264") => {
            cmd.args(["-c:v", "libx264", "-b:v", "2M", "-preset", "veryfast"]);
            cmd.args(["-c:a", "aac", "-b:a", "128k"]);
        }
        Some(other) => {
            cmd.args(["-c:v", other, "-b:v", "2M"]);
            cmd.args(["-c:a", "aac", "-b:a", "128k"]);
        }
    }
    cmd.arg(&path);

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(MediaError::Decode(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let info = probe(tools, &path)?;
    let codec = info
        .video
        .as_ref()
        .map(|v| v.codec.clone())
        .or_else(|| info.audio.as_ref().map(|a| a.codec.clone()))
        .ok_or_else(|| MediaError::Decode("proxy has neither audio nor video".into()))?;

    Ok(ProxyInfo { path, codec })
}
