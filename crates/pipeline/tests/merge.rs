use ustt_core::{Turn, Word};
use ustt_pipeline::merge::{assign_speaker, build_segments};
use uuid::Uuid;

fn words_from(items: &[(f64, f64, &str)]) -> Vec<Word> {
    items
        .iter()
        .map(|&(start, end, text)| Word {
            start,
            end,
            text: text.into(),
            confidence: None,
        })
        .collect()
}

#[test]
fn splits_on_gap_punctuation_and_length() {
    let words = words_from(&[
        (0.0, 0.2, "hi"),
        (0.3, 0.5, "there."),
        (2.0, 2.2, "next"),
        (16.0, 16.2, "late"),
    ]);
    let segs = build_segments(&words, &[], &[], 30.0);
    assert_eq!(segs.len(), 3);
    assert_eq!(segs[0].text, "hi there.");
    assert_eq!(segs[0].start, 0.0);
    assert!((segs[2].start - 16.0).abs() < 1e-9);
}

#[test]
fn overlapping_turns_pick_max_overlap() {
    let turns = vec![
        Turn {
            start: 0.0,
            end: 2.0,
            speaker_index: 0,
        },
        Turn {
            start: 1.0,
            end: 4.0,
            speaker_index: 1,
        },
    ];
    let ids = vec![Uuid::from_u128(1), Uuid::from_u128(2)];
    assert_eq!(assign_speaker(1.5, 3.0, &turns, &ids), Some(ids[1]));
    assert_eq!(assign_speaker(0.1, 0.5, &turns, &ids), Some(ids[0]));
}

#[test]
fn no_overlap_leaves_speaker_none() {
    let turns = vec![Turn {
        start: 0.0,
        end: 1.0,
        speaker_index: 0,
    }];
    assert_eq!(
        assign_speaker(5.0, 6.0, &turns, &[Uuid::from_u128(1)]),
        None
    );
}

#[test]
fn equal_overlap_picks_earliest_turn() {
    let turns = vec![
        Turn {
            start: 0.0,
            end: 1.0,
            speaker_index: 0,
        },
        Turn {
            start: 1.0,
            end: 2.0,
            speaker_index: 1,
        },
    ];
    let ids = vec![Uuid::from_u128(5), Uuid::from_u128(7)];
    assert_eq!(assign_speaker(0.9, 1.1, &turns, &ids), Some(ids[0]));
}
