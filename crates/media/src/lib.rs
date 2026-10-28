pub mod decode;
pub mod probe;
pub mod process;
pub mod proxy;
pub mod tools;
pub mod wav;
pub mod waveform;

pub use decode::decode_wav16k;
pub use probe::{AudioStream, MediaInfo, VideoStream, probe};
pub use proxy::{ProxyInfo, make_proxy, select_video_encoder};
pub use tools::Tools;
pub use ustt_core::MediaKind;
pub use wav::{read_wav_mono16k, read_wav_mono16k_range, wav_mono16k_samples, write_wav_mono16k};
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
