use crate::DownloadError;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DownloadTools {
    pub yt_dlp: PathBuf,
    pub deno: PathBuf,
    pub ffmpeg: Option<PathBuf>,
}

impl DownloadTools {
    pub fn new(yt_dlp: PathBuf, deno: PathBuf) -> Self {
        let ffmpeg = yt_dlp
            .parent()
            .map(|dir| {
                if cfg!(windows) {
                    dir.join("ffmpeg.exe")
                } else {
                    dir.join("ffmpeg")
                }
            })
            .filter(|path| path.is_file());
        Self {
            yt_dlp,
            deno,
            ffmpeg,
        }
    }

    pub fn locate(dir: Option<&Path>) -> Result<Self, DownloadError> {
        let dir = match dir {
            Some(path) => path.to_path_buf(),
            None if std::env::var_os("USTT_DOWNLOAD_DIR").is_some() => {
                PathBuf::from(std::env::var_os("USTT_DOWNLOAD_DIR").unwrap())
            }
            None => std::env::current_exe()
                .map_err(DownloadError::Io)?
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        };
        let yt_dlp = dir.join(if cfg!(windows) {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        });
        let deno = dir.join(if cfg!(windows) { "deno.exe" } else { "deno" });
        let mut missing = Vec::new();
        if !yt_dlp.is_file() {
            missing.push("yt-dlp");
        }
        if !deno.is_file() {
            missing.push("deno");
        }
        if !missing.is_empty() {
            return Err(DownloadError::Tools(format!(
                "{} not found in {}",
                missing.join(" and "),
                dir.display()
            )));
        }
        Ok(Self::new(yt_dlp, deno))
    }
}
