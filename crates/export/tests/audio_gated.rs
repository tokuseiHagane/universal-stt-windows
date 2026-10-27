use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use ustt_core::{AudioFormat, MediaKind, Project, Segment, SourceRef, Speaker};
use ustt_export::audio::{plan_segments, plan_speakers};
use ustt_export::extract;
use ustt_media::Tools;
use uuid::Uuid;

fn sine(path: &Path, seconds: f32) {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec).unwrap();
    for i in 0..(16_000.0 * seconds) as u32 {
        let v = ((i as f32 / 16_000.0) * 440.0 * std::f32::consts::TAU).sin() * 0.3;
        w.write_sample((v * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn project() -> Project {
    let mut p = Project::new(
        "Demo",
        SourceRef {
            path: PathBuf::from("/tmp/demo.wav"),
            size_bytes: 1,
            modified_unix: 0,
            fingerprint: "x".into(),
        },
        MediaKind::Audio,
        2.0,
    );
    let a = Uuid::from_u128(1);
    p.speakers = vec![Speaker {
        id: a,
        name: "Alice".into(),
        color: 0,
    }];
    p.segments = vec![Segment {
        id: Uuid::from_u128(10),
        start: 0.2,
        end: 0.8,
        text: "x".into(),
        speaker_id: Some(a),
        words: Vec::new(),
    }];
    p
}

#[test]
#[ignore = "needs ffmpeg"]
fn extracts_speaker_track_and_cancels() {
    let tmp = tempfile::tempdir().unwrap();
    let wav = tmp.path().join("a.wav");
    sine(&wav, 2.0);
    let tools = Tools::locate(None).unwrap();
    let p = project();
    let plan = plan_speakers(&p, None);
    let files = extract(
        &tools,
        &wav,
        &plan,
        "demo",
        AudioFormat::Wav,
        tmp.path(),
        &AtomicBool::new(false),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].is_file());

    let ids: Vec<_> = p.segments.iter().map(|s| s.id).collect();
    let plan = plan_segments(&p, &ids);
    let err = extract(
        &tools,
        &wav,
        &plan,
        "demo",
        AudioFormat::Wav,
        tmp.path(),
        &AtomicBool::new(true),
        &mut |_, _| {},
    )
    .unwrap_err();
    assert!(matches!(err, ustt_export::ExportError::Cancelled));

    // Two speakers: cancel from the progress callback after the first file.
    let mut p2 = project();
    let b = Uuid::from_u128(2);
    p2.speakers.push(Speaker {
        id: b,
        name: "Bob".into(),
        color: 1,
    });
    p2.segments.push(Segment {
        id: Uuid::from_u128(11),
        start: 1.0,
        end: 1.8,
        text: "y".into(),
        speaker_id: Some(b),
        words: Vec::new(),
    });
    let plan = plan_speakers(&p2, None);
    assert_eq!(plan.tracks.len(), 2);
    let cancel = AtomicBool::new(false);
    let out = tmp.path().join("cancel-out");
    std::fs::create_dir_all(&out).unwrap();
    let err = extract(
        &tools,
        &wav,
        &plan,
        "cancel",
        AudioFormat::Wav,
        &out,
        &cancel,
        &mut |fraction, _| {
            if fraction < 1.0 {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
    )
    .unwrap_err();
    assert!(matches!(err, ustt_export::ExportError::Cancelled));
    assert!(!std::fs::read_dir(&out).unwrap().any(|e| {
        e.map(|e| e.path().extension().and_then(|x| x.to_str()) == Some("wav"))
            .unwrap_or(false)
    }));
}
