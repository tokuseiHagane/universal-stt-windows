pub mod decode;
pub mod probe;
pub mod tools;
pub mod wav;
pub mod waveform;

pub use decode::decode_wav16k;
pub use probe::{AudioStream, MediaInfo, VideoStream, probe};
pub use tools::Tools;
pub use wav::{read_wav_mono16k, write_wav_mono16k};
pub use waveform::{Peak, Waveform, WaveformBuilder, waveform_from_samples, waveform_from_wav};

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("ffmpeg tools not found: {0}")]
    Tools(String),
    #[error("ffprobe failed: {0}")]
    Probe(String),
    #[error("decode failed: {0}")]
    Decode(String),
    #[error("invalid waveform: {0}")]
    Waveform(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
