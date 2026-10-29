pub mod ffi;
pub mod json;
mod path;
pub mod stitch;
pub mod vad;

pub use ffi::{AsrEngine, ParakeetLib, VadEngine};
pub use json::AsrJson;
pub use stitch::stitch;
pub use vad::{TimeRange, chunk_ranges};

use std::path::Path;

pub fn transcribe_chunks(
    engine: &mut AsrEngine,
    vad: &mut VadEngine,
    wav: &Path,
    max_len_s: f64,
    progress: &mut dyn FnMut(f32),
) -> Result<AsrJson, AsrError> {
    transcribe_chunks_cancellable(engine, vad, wav, max_len_s, None, progress)
}

pub fn transcribe_chunks_cancellable(
    engine: &mut AsrEngine,
    vad: &mut VadEngine,
    wav: &Path,
    max_len_s: f64,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: &mut dyn FnMut(f32),
) -> Result<AsrJson, AsrError> {
    let total_samples =
        ustt_media::wav_mono16k_samples(wav).map_err(|e| AsrError::Model(e.to_string()))?;
    let duration = total_samples as f64 / 16_000.0;
    let ranges = vad.speech_ranges(wav)?;
    let chunks = chunk_ranges(&ranges, max_len_s, duration);
    if chunks.is_empty() {
        progress(1.0);
        return Ok(AsrJson {
            text: String::new(),
            language: None,
            words: Vec::new(),
        });
    }

    let total: f64 = chunks.iter().map(|r| (r.end - r.start).max(0.0)).sum();
    let mut done = 0.0;
    let mut out = Vec::with_capacity(chunks.len());
    for range in chunks {
        if cancel.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
            return Err(AsrError::Cancelled);
        }
        let first = (range.start * 16_000.0).floor().max(0.0) as usize;
        let last = ((range.end * 16_000.0).ceil() as usize).min(total_samples);
        let count = last.saturating_sub(first.min(last));
        let slice = ustt_media::read_wav_mono16k_range(wav, first, count)
            .map_err(|e| AsrError::Model(e.to_string()))?;
        let json = engine.transcribe_pcm(&slice)?;
        out.push((range, json));
        done += (range.end - range.start).max(0.0);
        progress(if total > 0.0 {
            (done / total) as f32
        } else {
            1.0
        });
    }
    Ok(stitch(&out))
}

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("parakeet library: {0}")]
    Lib(String),
    #[error("parakeet model: {0}")]
    Model(String),
    #[error("invalid asr json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cancelled")]
    Cancelled,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
