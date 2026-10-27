use crate::linker::LabelledTurn;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceWindow {
    pub start: usize,
    pub keep_low: usize,
    pub keep_high: usize,
    pub place_low: usize,
}

/// Windows over `mel_frames` voice-filterbank frames: `width` each, overlapping
/// by `margin` a side, the first at the recording's start and the last ending at
/// its end; a shorter recording is one padded window.
pub fn windows(
    mel_frames: usize,
    width: usize,
    margin: usize,
    per_column: usize,
) -> Vec<VoiceWindow> {
    if per_column == 0 {
        return Vec::new();
    }
    let columns = mel_frames / per_column;
    if mel_frames <= width {
        return vec![VoiceWindow {
            start: 0,
            keep_low: 0,
            keep_high: columns,
            place_low: 0,
        }];
    }
    let mut out = Vec::new();
    let mut kept = 0;
    let mut start = 0;
    while kept < columns {
        let last = start + width >= mel_frames;
        if last {
            start = mel_frames - width;
        }
        let end = if last {
            columns
        } else {
            (start + width - margin) / per_column
        };
        out.push(VoiceWindow {
            start,
            keep_low: kept - start / per_column,
            keep_high: end - start / per_column,
            place_low: kept,
        });
        kept = end;
        start += width - 2 * margin;
    }
    out
}

/// A lane's solo frames at 10 ms, placed at its piece's offset, as a mask over
/// the recording's columns: a column is the lane's when at least half its frames are.
pub fn column_mask(solo: &[f32], offset: usize, columns: usize, per_column: usize) -> Vec<bool> {
    let mut out = vec![false; columns];
    if per_column == 0 {
        return out;
    }
    let first = (offset / per_column).min(columns);
    let last = (offset + solo.len()).div_ceil(per_column).min(columns);
    for (column, slot) in out.iter_mut().enumerate().take(last).skip(first) {
        let mut on = 0;
        for f in 0..per_column {
            let t = column * per_column + f;
            if t >= offset && t - offset < solo.len() && solo[t - offset] > 0.0 {
                on += 1;
            }
        }
        *slot = 2 * on >= per_column;
    }
    out
}

/// Of the voiced lanes, the one nearest `turns` in time: the smallest gap
/// between a turn of each, an overlap counting as no gap, the most overlap
/// deciding among those. Returns the lane index.
pub fn nearest(turns: &[LabelledTurn], among: &[Vec<LabelledTurn>], lanes: &[usize]) -> usize {
    let mut best = lanes[0];
    let mut best_gap = f64::INFINITY;
    let mut best_overlap = f64::NEG_INFINITY;
    for (candidate, &lane) in among.iter().zip(lanes) {
        let mut gap = f64::INFINITY;
        let mut overlap = 0.0;
        for a in turns {
            for b in candidate {
                let shared = a.end.min(b.end) - a.start.max(b.start);
                gap = gap.min((-shared).max(0.0));
                overlap += shared.max(0.0);
            }
        }
        if gap < best_gap || (gap == best_gap && overlap > best_overlap) {
            best = lane;
            best_gap = gap;
            best_overlap = overlap;
        }
    }
    best
}

/// The same partition numbered in order of first appearance.
pub fn first_appearance(labels: &[i64]) -> Vec<usize> {
    let mut numbered: HashMap<i64, usize> = HashMap::new();
    labels
        .iter()
        .map(|&label| {
            let next = numbered.len();
            *numbered.entry(label).or_insert(next)
        })
        .collect()
}
