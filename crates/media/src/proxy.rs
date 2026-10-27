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

fn encoder_names(tools: &Tools) -> Result<Vec<String>, MediaError> {
    let output = Command::new(&tools.ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()?;
    if !output.status.success() {
        return Err(MediaError::Decode(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let list = String::from_utf8_lossy(&output.stdout);
    Ok(list
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(str::to_string)
        .collect())
}

pub fn select_video_encoder(tools: &Tools) -> Result<&'static str, MediaError> {
    if let Ok(forced) = std::env::var("USTT_VIDEO_ENCODER") {
        return match forced.as_str() {
            "h264_mf" => Ok("h264_mf"),
            "libvpx-vp9" => Ok("libvpx-vp9"),
            other => Err(MediaError::Decode(format!(
                "unsupported USTT_VIDEO_ENCODER={other}"
            ))),
        };
    }
    let names = encoder_names(tools)?;
    for candidate in ["h264_mf", "libvpx-vp9"] {
        if names.iter().any(|name| name == candidate) {
            return Ok(candidate);
        }
    }
    Err(MediaError::Decode(
        "no usable H.264/VP9 encoder in ffmpeg".into(),
    ))
}

/// `h264_mf` is advertised by Windows FFmpeg builds but Media Foundation is
/// often missing (Wine/Proton, stripped Windows editions), so keep VP9 as a
/// runtime fallback.
pub fn video_encoder_candidates(preferred: &'static str, vp9_available: bool) -> Vec<&'static str> {
    if preferred == "h264_mf" && vp9_available {
        vec!["h264_mf", "libvpx-vp9"]
    } else {
        vec![preferred]
    }
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
            let names = encoder_names(tools)?;
            let preferred = select_video_encoder(tools)?;
            let vp9 = names.iter().any(|name| name == "libvpx-vp9");
            let mut last_error = None;
            for encoder in video_encoder_candidates(preferred, vp9) {
                let path = out_dir.join(if encoder == "libvpx-vp9" {
                    format!("{stem}.webm")
                } else {
                    format!("{stem}.mp4")
                });
                match encode(tools, source, &path, Some(encoder)) {
                    Ok(()) => {
                        return finish(tools, path);
                    }
                    Err(error) => {
                        let _ = std::fs::remove_file(&path);
                        last_error = Some(error);
                    }
                }
            }
            return Err(last_error.unwrap_or(MediaError::Decode("no usable video encoder".into())));
        }
    };
    encode(tools, source, &path, encoder)?;
    finish(tools, path)
}

fn encode(
    tools: &Tools,
    source: &Path,
    path: &Path,
    encoder: Option<&str>,
) -> Result<(), MediaError> {
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
            cmd.args(["-c:a", "libopus", "-b:a", "96k"]);
        }
        Some(other) => {
            cmd.args(["-c:v", other, "-b:v", "2M"]);
            cmd.args(["-c:a", "aac", "-b:a", "128k"]);
        }
    }
    cmd.arg(path);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(MediaError::Decode(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

fn finish(tools: &Tools, path: PathBuf) -> Result<ProxyInfo, MediaError> {
    let info = probe(tools, &path)?;
    let codec = info
        .video
        .as_ref()
        .map(|v| v.codec.clone())
        .or_else(|| info.audio.as_ref().map(|a| a.codec.clone()))
        .ok_or_else(|| MediaError::Decode("proxy has neither audio nor video".into()))?;
    Ok(ProxyInfo { path, codec })
}
