mod progress;
mod tools;

pub use progress::DownloadProgress;
pub use tools::DownloadTools;

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::borrow::Borrow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("download tools: {0}")]
    Tools(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("download cancelled")]
    Cancelled,
    #[error("unsupported URL: {0}")]
    Unsupported(String),
    #[error("authentication required: {0}")]
    Authentication(String),
    #[error("DRM-protected media: {0}")]
    Drm(String),
    #[error("download failed: {0}")]
    Failed(String),
    #[error("download I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct DownloadedMedia {
    pub path: PathBuf,
    pub title: String,
    pub temp: TempDir,
}

pub fn download<F, C>(
    tools: &DownloadTools,
    url: &str,
    cancel: C,
    on_progress: F,
) -> Result<DownloadedMedia, DownloadError>
where
    F: FnMut(DownloadProgress),
    C: Borrow<AtomicBool>,
{
    validate_url(url)?;
    let temp = tempfile::tempdir()?;
    run(tools, url, temp, cancel, on_progress)
}

pub fn download_in_dir<F, C>(
    tools: &DownloadTools,
    url: &str,
    parent: &Path,
    cancel: C,
    on_progress: F,
) -> Result<DownloadedMedia, DownloadError>
where
    F: FnMut(DownloadProgress),
    C: Borrow<AtomicBool>,
{
    validate_url(url)?;
    let temp = tempfile::tempdir_in(parent)?;
    run(tools, url, temp, cancel, on_progress)
}

fn validate_url(url: &str) -> Result<(), DownloadError> {
    let lower = url.trim().to_ascii_lowercase();
    if (lower.starts_with("https://") && lower.len() > 8)
        || (lower.starts_with("http://") && lower.len() > 7)
    {
        Ok(())
    } else {
        Err(DownloadError::InvalidUrl(url.to_string()))
    }
}

fn run<F, C>(
    tools: &DownloadTools,
    url: &str,
    temp: TempDir,
    cancel: C,
    mut on_progress: F,
) -> Result<DownloadedMedia, DownloadError>
where
    F: FnMut(DownloadProgress),
    C: Borrow<AtomicBool>,
{
    let mut command = ustt_media::process::command(&tools.yt_dlp);
    let mut runtime = OsString::from("deno:");
    runtime.push(&tools.deno);
    command
        .arg("--no-playlist")
        .arg("--newline")
        .arg("--no-simulate")
        .arg("--js-runtimes")
        .arg(runtime)
        .arg("--progress-template")
        .arg("download:USTT_PROGRESS|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.percent)s")
        .arg("--print")
        .arg("title:USTT_TITLE|%(title)s")
        .arg("--print")
        .arg("after_move:USTT_FILE|%(filepath)s")
        .arg("--paths")
        .arg(temp.path())
        .arg("--output")
        .arg("%(id)s.%(ext)s")
        .arg("--format")
        .arg("bv*[height<=1080]+ba/b[height<=1080]/b")
        .arg("--")
        .arg(url.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(ffmpeg) = &tools.ffmpeg {
        command.arg("--ffmpeg-location").arg(ffmpeg);
    }

    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let (tx, rx) = mpsc::channel();
    let out_thread = spawn_reader(stdout, tx.clone(), false);
    let err_thread = spawn_reader(stderr, tx, true);
    let mut output = Output::default();

    let mut cancelled = false;
    let status = loop {
        while let Ok(line) = rx.try_recv() {
            output.consume(line, &mut on_progress);
        }
        if cancel.borrow().load(Ordering::Relaxed) {
            let _ = child.kill();
            cancelled = true;
            break child.wait()?;
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(25));
    };
    if cancelled {
        drop(out_thread);
        drop(err_thread);
    } else {
        let _ = out_thread.join();
        let _ = err_thread.join();
    }
    while let Ok(line) = rx.try_recv() {
        output.consume(line, &mut on_progress);
    }

    if cancelled || cancel.borrow().load(Ordering::Relaxed) {
        return Err(DownloadError::Cancelled);
    }
    if !status.success() {
        return Err(classify_failure(&output.stderr, status));
    }
    let reported = output
        .file
        .ok_or_else(|| DownloadError::Failed("yt-dlp did not report an output file".to_string()))?;
    let path = if reported.is_absolute() {
        reported
    } else {
        temp.path().join(reported)
    };
    if !path.is_file() {
        return Err(DownloadError::Failed(format!(
            "downloaded file does not exist: {}",
            path.display()
        )));
    }
    let canonical = path.canonicalize()?;
    let root = temp.path().canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(DownloadError::Failed(format!(
            "downloaded file is outside the temporary directory: {}",
            path.display()
        )));
    }
    let title = output
        .title
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    Ok(DownloadedMedia { path, title, temp })
}

#[derive(Default)]
struct Output {
    title: Option<String>,
    file: Option<PathBuf>,
    stderr: String,
}

impl Output {
    fn consume<F>(&mut self, line: Line, on_progress: &mut F)
    where
        F: FnMut(DownloadProgress),
    {
        if line.is_stderr {
            if !self.stderr.is_empty() {
                self.stderr.push('\n');
            }
            self.stderr.push_str(&line.text);
            if self.stderr.len() > 32_768 {
                let mut start = self.stderr.len() - 32_768;
                while !self.stderr.is_char_boundary(start) {
                    start += 1;
                }
                self.stderr.drain(..start);
            }
        }
        match progress::parse_line(&line.text) {
            progress::OutputLine::Progress(value) => on_progress(value),
            progress::OutputLine::Title(value) => self.title = Some(value),
            progress::OutputLine::File(value) => self.file = Some(value),
            progress::OutputLine::Other => {}
        }
    }
}

struct Line {
    text: String,
    is_stderr: bool,
}

fn spawn_reader(
    reader: impl Read + Send + 'static,
    tx: Sender<Line>,
    is_stderr: bool,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(text) = line else { break };
            if tx.send(Line { text, is_stderr }).is_err() {
                break;
            }
        }
    })
}

fn classify_failure(stderr: &str, status: ExitStatus) -> DownloadError {
    let message = if stderr.trim().is_empty() {
        format!("yt-dlp exited with {status}")
    } else {
        stderr.trim().to_string()
    };
    let lower = message.to_ascii_lowercase();
    if lower.contains("unsupported url") || lower.contains("no suitable info extractor") {
        DownloadError::Unsupported(message)
    } else if lower.contains("drm") {
        DownloadError::Drm(message)
    } else if lower.contains("sign in")
        || lower.contains("login")
        || lower.contains("cookies")
        || lower.contains("account")
    {
        DownloadError::Authentication(message)
    } else {
        DownloadError::Failed(message)
    }
}
