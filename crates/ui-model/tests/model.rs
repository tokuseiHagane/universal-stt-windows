use chrono::Utc;
use ustt_core::{MediaKind, Project, Segment, SourceRef, Speaker, Turn};
use ustt_ui_model::{
    Lang, Playback, TimelineLayout, active_segment, best_segment_for_turn, date_label,
    rename_speaker, set_segment_text, speaker_time, strings, timecode, virtual_rows,
};
use uuid::Uuid;

fn sample_project() -> Project {
    let mut project = Project::new(
        "Demo",
        SourceRef {
            path: "/tmp/demo.wav".into(),
            size_bytes: 10,
            modified_unix: 0,
            fingerprint: "x".into(),
            origin_url: None,
        },
        MediaKind::Audio,
        10.0,
    );
    project.speakers = vec![
        Speaker {
            id: Uuid::from_u128(1),
            name: "Speaker 1".into(),
            color: 0,
        },
        Speaker {
            id: Uuid::from_u128(2),
            name: "Speaker 2".into(),
            color: 1,
        },
    ];
    project.segments = vec![
        Segment {
            id: Uuid::from_u128(10),
            start: 0.0,
            end: 2.0,
            text: "hello".into(),
            speaker_id: Some(Uuid::from_u128(1)),
            words: Vec::new(),
        },
        Segment {
            id: Uuid::from_u128(11),
            start: 3.0,
            end: 5.0,
            text: "world".into(),
            speaker_id: Some(Uuid::from_u128(2)),
            words: Vec::new(),
        },
    ];
    project
}

#[test]
fn timeline_maps_both_ways_and_clamps() {
    let t = TimelineLayout::new(10.0, 100.0, 120.0);
    assert!((t.x(5.0) - 50.0).abs() < 1e-9);
    assert!((t.time(50.0) - 5.0).abs() < 1e-9);
    assert_eq!(t.time(-10.0), 0.0);
    assert_eq!(t.time(10_000.0), 120.0);
}

#[test]
fn zoom_keeps_the_cursor_time_under_the_cursor() {
    let mut t = TimelineLayout::new(10.0, 30.0, 120.0);
    let before = t.time(400.0);
    t.zoom_at(400.0, 2.0);
    assert!((t.time(400.0) - before).abs() < 1e-6);
    assert!(t.px_per_sec <= 2000.0);
}

#[test]
fn virtualized_rows_stay_bounded() {
    let v = virtual_rows(512.0, 480.0, 64.0, 10_000);
    assert_eq!(v.first, 8);
    assert_eq!(v.last, 16);
    assert_eq!(v.total_px, 640_000.0);
}

#[test]
fn seek_before_metadata_is_clamped() {
    let mut p = Playback::default();
    p.seek(12.0);
    assert_eq!(p.position, 0.0);
    p.set_duration(60.0);
    p.seek(12.0);
    assert_eq!(p.position, 12.0);
    p.seek(999.0);
    assert_eq!(p.position, 60.0);
}

#[test]
fn both_languages_fill_every_string() {
    for lang in [Lang::Ru, Lang::En] {
        let s = strings(lang);
        assert!(!s.app_name.is_empty());
        assert!(!s.welcome_drop.is_empty());
        assert!(!s.processing_cancel.is_empty());
    }
    assert_ne!(
        strings(Lang::Ru).welcome_drop,
        strings(Lang::En).welcome_drop
    );
    assert_eq!(Lang::from_system(Some("ru-RU")), Lang::Ru);
    assert_eq!(Lang::from_system(Some("en-US")), Lang::En);
    assert_eq!(Lang::from_preference(Some("ru"), Some("en-US")), Lang::Ru);
    assert_eq!(Lang::from_preference(None, Some("ru-RU")), Lang::Ru);
    assert_eq!(Lang::from_preference(Some("auto"), Some("en-US")), Lang::En);
}

#[test]
fn edit_helpers_report_change() {
    let mut p = sample_project();
    let speaker = p.speakers[0].id;
    let segment = p.segments[0].id;
    let expected = p.segments[0].end - p.segments[0].start;
    assert!(rename_speaker(&mut p, speaker, "Alice"));
    assert!(!rename_speaker(&mut p, speaker, "Alice"));
    assert!(set_segment_text(&mut p, segment, "new"));
    assert_eq!(speaker_time(&p, speaker), expected);
}

#[test]
fn active_segment_and_formatting() {
    let p = sample_project();
    assert_eq!(active_segment(&p, 0.5), Some(0));
    assert_eq!(active_segment(&p, 100.0), None);
    assert_eq!(timecode(3661.4), "01:01:01");
    assert_eq!(date_label(&Utc::now(), Lang::Ru).len(), 10);
}

#[test]
fn best_segment_for_turn_prefers_max_overlap() {
    let project = sample_project();
    let turn = Turn {
        start: 1.5,
        end: 4.5,
        speaker_index: 0,
    };
    assert_eq!(best_segment_for_turn(&project.segments, &turn), Some(1));
}

#[test]
fn best_segment_for_turn_breaks_tie_by_earlier_segment() {
    let project = sample_project();
    let turn = Turn {
        start: 1.0,
        end: 4.0,
        speaker_index: 0,
    };
    assert_eq!(best_segment_for_turn(&project.segments, &turn), Some(0));
}

#[test]
fn best_segment_for_turn_rejects_no_overlap() {
    let project = sample_project();
    let turn = Turn {
        start: 5.1,
        end: 6.0,
        speaker_index: 0,
    };
    assert_eq!(best_segment_for_turn(&project.segments, &turn), None);
}
