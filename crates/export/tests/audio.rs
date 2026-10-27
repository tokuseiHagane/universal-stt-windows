use std::path::{Path, PathBuf};
use ustt_core::{AudioFormat, MediaKind, Project, Segment, SourceRef, Speaker};
use ustt_export::audio::{build_filter_script, plan_segments, plan_speakers};
use ustt_export::{AudioPlan, ExportError, extract};
use ustt_media::Tools;
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
        segment(10, 0.0, 2.0, Some(alice)),
        segment(11, 2.1, 4.0, Some(alice)),
        segment(12, 5.0, 6.0, Some(bob)),
        segment(13, 7.0, 8.0, None),
    ];
    project
}

fn segment(id: u128, start: f64, end: f64, speaker: Option<Uuid>) -> Segment {
    Segment {
        id: Uuid::from_u128(id),
        start,
        end,
        text: "x".into(),
        speaker_id: speaker,
        words: Vec::new(),
    }
}

#[test]
fn plan_skips_unassigned() {
    let plan = plan_speakers(&sample_project(), None);
    assert_eq!(plan.tracks.len(), 2);
    assert!(plan.tracks.iter().all(|t| !t.ranges.is_empty()));
}

#[test]
fn plan_merges_adjacent_ranges() {
    let p = sample_project();
    let plan = plan_speakers(&p, Some(&[p.speakers[0].id]));
    assert_eq!(plan.tracks[0].ranges, vec![(0.0, 4.0)]);
}

#[test]
fn plan_writes_filter_script_for_many_ranges() {
    let mut p = sample_project();
    p.segments = (0..2_000)
        .map(|i| segment(i, i as f64, i as f64 + 0.3, Some(p.speakers[0].id)))
        .collect();
    let ids: Vec<_> = p.segments.iter().map(|s| s.id).collect();
    let plan = plan_segments(&p, &ids);
    let script = build_filter_script(&plan.tracks[0], Path::new("/tmp/a.wav"));
    assert!(script.contains("concat=n=2000"));
    assert!(script.len() > 10_000);
}

#[test]
fn missing_wav_is_an_error() {
    let tools = Tools {
        ffmpeg: PathBuf::from("ffmpeg"),
        ffprobe: PathBuf::from("ffprobe"),
    };
    let plan = plan_segments(&sample_project(), &[Uuid::from_u128(10)]);
    let err = extract(
        &tools,
        Path::new("/definitely/missing.wav"),
        &plan,
        "demo",
        AudioFormat::Wav,
        Path::new("/tmp"),
        &std::sync::atomic::AtomicBool::new(false),
        &mut |_, _| {},
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::MissingMedia));
}

#[test]
fn empty_plan_is_rejected() {
    let tools = Tools {
        ffmpeg: PathBuf::from("ffmpeg"),
        ffprobe: PathBuf::from("ffprobe"),
    };
    let tmp = tempfile::tempdir().unwrap();
    let wav = tmp.path().join("a.wav");
    std::fs::write(&wav, b"x").unwrap();
    let err = extract(
        &tools,
        &wav,
        &AudioPlan::default(),
        "demo",
        AudioFormat::Wav,
        tmp.path(),
        &std::sync::atomic::AtomicBool::new(false),
        &mut |_, _| {},
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::Audio(_)));
}

#[test]
fn track_names_are_unique_across_duplicate_speakers() {
    let a = ustt_export::audio::track_filename("demo-2026-10-28", 0, "Alice", "wav");
    let b = ustt_export::audio::track_filename("demo-2026-10-28", 1, "alice", "wav");
    assert_ne!(a, b);
    assert_eq!(a, "demo-2026-10-28-01-alice.wav");
}
