use ustt_core::{Segment, Turn, Word};
use uuid::Uuid;

pub const MAX_GAP_S: f64 = 0.6;
pub const MAX_SEGMENT_S: f64 = 15.0;

pub fn build_segments(
    words: &[Word],
    turns: &[Turn],
    speaker_ids: &[Uuid],
    duration: f64,
) -> Vec<Segment> {
    let duration = duration.max(words.last().map(|w| w.end).unwrap_or(0.0));
    let mut out = Vec::new();
    let mut current: Option<Vec<&Word>> = None;
    let mut start = 0.0;

    for word in words {
        let mut fresh = current.is_none();
        if let Some(acc) = current.as_ref() {
            let last = acc.last().unwrap();
            let gap = word.start - last.end;
            let punctuation = last.text.ends_with(['.', '!', '?', '…']);
            fresh = gap > MAX_GAP_S || punctuation || word.end - start > MAX_SEGMENT_S;
        }
        if fresh {
            if let Some(acc) = current.take() {
                out.push(make_segment(&acc, turns, speaker_ids, duration));
            }
            current = Some(vec![word]);
            start = word.start;
        } else if let Some(acc) = current.as_mut() {
            acc.push(word);
        }
    }
    if let Some(acc) = current {
        out.push(make_segment(&acc, turns, speaker_ids, duration));
    }
    out
}

fn make_segment(words: &[&Word], turns: &[Turn], speaker_ids: &[Uuid], duration: f64) -> Segment {
    let start = words.first().map(|w| w.start).unwrap_or(0.0);
    let end = words
        .last()
        .map(|w| w.end)
        .unwrap_or(start)
        .min(duration)
        .max(start);
    Segment {
        id: Uuid::new_v4(),
        start,
        end,
        text: words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        speaker_id: assign_speaker(start, end, turns, speaker_ids),
        words: words.iter().map(|w| (*w).clone()).collect(),
    }
}

pub fn assign_speaker(start: f64, end: f64, turns: &[Turn], speaker_ids: &[Uuid]) -> Option<Uuid> {
    let mut best = 0.0;
    let mut best_index = None;
    for turn in turns {
        let overlap = end.min(turn.end) - start.max(turn.start);
        // A hair of float noise must not beat the earlier turn on an exact tie.
        if overlap > best + 1e-9 {
            best = overlap;
            best_index = Some(turn.speaker_index as usize);
        }
    }
    best_index.and_then(|index| speaker_ids.get(index).copied())
}
