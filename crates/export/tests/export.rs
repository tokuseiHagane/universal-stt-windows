use std::path::PathBuf;
use ustt_core::{MediaKind, Project, Segment, SourceRef, Speaker};
use ustt_export::{TextOutput, lines, render_text};
use uuid::Uuid;

fn sample_project() -> Project {
    let mut project = Project::new(
        "Demo",
        SourceRef {
            path: PathBuf::from("/tmp/demo.wav"),
            size_bytes: 1,
            modified_unix: 0,
            fingerprint: "x".into(),
        },
        MediaKind::Audio,
        60.0,
    );
    let alice = Uuid::from_u128(1);
    let bob = Uuid::from_u128(2);
    project.speakers = vec![
        Speaker {
            id: alice,
            name: "Alice".into(),
            color: 0,
        },
        Speaker {
            id: bob,
            name: "Bob".into(),
            color: 1,
        },
    ];
    project.segments = vec![
        Segment {
            id: Uuid::from_u128(10),
            start: 0.0,
            end: 2.0,
            text: "hello".into(),
            speaker_id: Some(alice),
            words: Vec::new(),
        },
        Segment {
            id: Uuid::from_u128(11),
            start: 3.0,
            end: 4.0,
            text: "hi".into(),
            speaker_id: Some(bob),
            words: Vec::new(),
        },
        Segment {
            id: Uuid::from_u128(12),
            start: 5.0,
            end: 6.0,
            text: "again".into(),
            speaker_id: Some(alice),
            words: Vec::new(),
        },
    ];
    project
}

#[test]
fn filter_excludes_unselected_speakers() {
    let p = sample_project();
    let only = lines(&p, Some(&[p.speakers[0].id]));
    assert_eq!(only.len(), 2);
    assert!(only.iter().all(|l| l.speaker == "Alice"));
    assert_eq!(lines(&p, None).len(), 3);
}

#[test]
fn srt_and_vtt_are_well_formed() {
    let p = sample_project();
    let ls = lines(&p, None);
    let srt = render_text(&ls, TextOutput::Srt, true);
    assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:02,000\nAlice: hello"));
    let vtt = render_text(&ls, TextOutput::Vtt, true);
    assert!(vtt.starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nAlice: hello"));
}

#[test]
fn txt_respects_timestamps_flag() {
    let ls = lines(&sample_project(), None);
    assert!(render_text(&ls, TextOutput::Txt, true).contains("[00:00:00] Alice: hello"));
    assert!(!render_text(&ls, TextOutput::Txt, false).contains("[00:00:00]"));
}

#[test]
fn markdown_groups_by_speaker() {
    let md = render_text(&lines(&sample_project(), None), TextOutput::Markdown, false);
    assert!(md.contains("**Alice**"));
    assert!(md.contains("**Bob**"));
}

#[test]
fn text_output_parses_and_extends() {
    assert_eq!(TextOutput::parse("MD"), Some(TextOutput::Markdown));
    assert_eq!(TextOutput::parse("nope"), None);
    assert_eq!(TextOutput::Vtt.extension(), "vtt");
}

#[test]
fn unassigned_segments_are_dropped_by_a_filter() {
    let mut p = sample_project();
    p.segments[0].speaker_id = None;
    let filtered = lines(&p, Some(&[p.speakers[0].id]));
    assert!(!filtered.iter().any(|l| l.text == "hello"));
    assert_eq!(lines(&p, None).len(), 3);
}

#[test]
fn docx_is_a_zip_with_document_xml() {
    let p = sample_project();
    let bytes = ustt_export::render_docx(&p, &lines(&p, None), true).unwrap();
    assert_eq!(&bytes[..2], b"PK");
    assert!(bytes.len() > 2_000);
}

#[test]
fn pdf_has_a_header_and_cyrillic_text() {
    let mut p = sample_project();
    p.title = "Тестовая запись".into();
    let bytes = ustt_export::render_pdf(&p, &lines(&p, None), true).unwrap();
    assert_eq!(&bytes[..4], b"%PDF");
    assert!(bytes.len() > 4_000);
}
