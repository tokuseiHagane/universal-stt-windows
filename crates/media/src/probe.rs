use crate::MediaError;
use crate::tools::Tools;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VideoStream {
    pub codec: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioStream {
    pub codec: String,
    pub channels: u32,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaInfo {
    pub duration: f64,
    pub video: Option<VideoStream>,
    pub audio: Option<AudioStream>,
}

#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    format: Option<Format>,
    #[serde(default)]
    streams: Vec<Stream>,
}

#[derive(Debug, Deserialize)]
struct Format {
    #[serde(default)]
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Stream {
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    channels: Option<u32>,
    #[serde(default)]
    sample_rate: Option<String>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    disposition: Option<Disposition>,
}

#[derive(Debug, Deserialize)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

pub fn probe(tools: &Tools, path: &Path) -> Result<MediaInfo, MediaError> {
    let output = Command::new(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()?;

    if !output.status.success() {
        return Err(MediaError::Probe(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let parsed: ProbeOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| MediaError::Probe(format!("invalid ffprobe json: {e}")))?;

    let video = parsed.streams.iter().find_map(|s| {
        let is_video = s.codec_type.as_deref() == Some("video");
        let is_picture = s.disposition.as_ref().is_some_and(|d| d.attached_pic != 0);
        let width = s.width.unwrap_or(0);
        let height = s.height.unwrap_or(0);
        (is_video && !is_picture && width > 0 && height > 0).then(|| VideoStream {
            codec: s.codec_name.clone().unwrap_or_default(),
            width,
            height,
        })
    });

    let audio = parsed.streams.iter().find_map(|s| {
        (s.codec_type.as_deref() == Some("audio")).then(|| AudioStream {
            codec: s.codec_name.clone().unwrap_or_default(),
            channels: s.channels.unwrap_or(0),
            sample_rate: s
                .sample_rate
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        })
    });

    let duration = parsed
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| {
            parsed
                .streams
                .iter()
                .filter_map(|s| s.duration.as_deref().and_then(|v| v.parse::<f64>().ok()))
                .reduce(f64::max)
        })
        .unwrap_or(0.0);

    Ok(MediaInfo {
        duration,
        video,
        audio,
    })
}
