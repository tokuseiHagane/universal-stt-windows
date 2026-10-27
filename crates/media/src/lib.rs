pub mod probe;
pub mod tools;

pub use probe::{AudioStream, MediaInfo, VideoStream, probe};
pub use tools::Tools;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("ffmpeg tools not found: {0}")]
    Tools(String),
    #[error("ffprobe failed: {0}")]
    Probe(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
