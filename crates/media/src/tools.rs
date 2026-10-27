use crate::MediaError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Tools {
    pub fn locate(explicit_dir: Option<&Path>) -> Result<Self, MediaError> {
        if let Some(dir) = explicit_dir {
            return Self::in_dir(dir).ok_or_else(|| missing(dir));
        }

        if let Some(dir) = std::env::var_os("USTT_FFMPEG_DIR") {
            let dir = PathBuf::from(dir);
            if let Some(tools) = Self::in_dir(&dir) {
                return Ok(tools);
            }
        }

        if let Ok(exe) = std::env::current_exe()
            && let Some(parent) = exe.parent()
            && let Some(tools) = Self::in_dir(parent)
        {
            return Ok(tools);
        }

        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                if let Some(tools) = Self::in_dir(&dir) {
                    return Ok(tools);
                }
            }
        }

        Err(MediaError::Tools(
            "ffmpeg/ffprobe not found in explicit dir, USTT_FFMPEG_DIR, app dir, or PATH".into(),
        ))
    }

    fn in_dir(dir: &Path) -> Option<Self> {
        let (ffmpeg_name, ffprobe_name) = exe_names();
        let ffmpeg = dir.join(ffmpeg_name);
        let ffprobe = dir.join(ffprobe_name);
        (ffmpeg.is_file() && ffprobe.is_file()).then_some(Self { ffmpeg, ffprobe })
    }
}

fn missing(dir: &Path) -> MediaError {
    MediaError::Tools(format!("ffmpeg/ffprobe not found in {}", dir.display()))
}

#[cfg(windows)]
fn exe_names() -> (&'static str, &'static str) {
    ("ffmpeg.exe", "ffprobe.exe")
}

#[cfg(not(windows))]
fn exe_names() -> (&'static str, &'static str) {
    ("ffmpeg", "ffprobe")
}
