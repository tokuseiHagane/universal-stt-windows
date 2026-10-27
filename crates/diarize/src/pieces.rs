use std::ops::Range;

/// Pieces of `window` frames laid end to end, the last one short.
pub fn cut(frames: usize, window: usize) -> Vec<Range<usize>> {
    if window == 0 {
        return Vec::new();
    }
    if frames <= window {
        return std::iter::once(0..frames).collect();
    }
    (0..frames)
        .step_by(window)
        .map(|first| first..(first + window).min(frames))
        .collect()
}

pub fn window_for(frames: usize, windows: &[usize]) -> usize {
    windows
        .iter()
        .copied()
        .find(|&w| w >= frames)
        .or_else(|| windows.last().copied())
        .unwrap_or(frames)
}
