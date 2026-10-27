use crate::AsrError;
use crate::ffi::VadEngine;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Deserialize)]
struct VadDoc {
    #[serde(default)]
    segments: Vec<VadSegment>,
}

#[derive(Debug, Deserialize)]
struct VadSegment {
    start: f64,
    end: f64,
}

impl VadEngine {
    pub fn speech_ranges(&mut self, wav: &Path) -> Result<Vec<TimeRange>, AsrError> {
        let text = self.raw_segments_json(wav)?;
        let doc: VadDoc = serde_json::from_str(&text)?;
        Ok(doc
            .segments
            .into_iter()
            .filter(|s| s.end > s.start)
            .map(|s| TimeRange {
                start: s.start,
                end: s.end,
            })
            .collect())
    }
}

/// Splits long speech ranges at `max_len_s`; with no VAD output, cuts the whole
/// recording into equal windows.
pub fn chunk_ranges(ranges: &[TimeRange], max_len_s: f64, duration: f64) -> Vec<TimeRange> {
    if ranges.is_empty() {
        if duration <= 0.0 || max_len_s <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut start = 0.0;
        while start < duration {
            let end = (start + max_len_s).min(duration);
            out.push(TimeRange { start, end });
            start = end;
        }
        return out;
    }
    let mut out = Vec::new();
    for range in ranges {
        let mut start = range.start;
        while start < range.end {
            let end = (start + max_len_s).min(range.end);
            out.push(TimeRange { start, end });
            start = end;
        }
    }
    out
}
