use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use ustt_core::ProjectStore;
use ustt_pipeline::backend::Backend;
use ustt_pipeline::import::{ImportRequest, run_import};
use ustt_pipeline::models::resolve_models;

fn models_dir() -> PathBuf {
    let raw = std::env::var("USTT_MODELS_DIR").unwrap_or_else(|_| "models".into());
    if PathBuf::from(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(raw)
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
#[ignore = "needs models, libparakeet and fixture"]
fn imports_two_speaker_clip_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let req = ImportRequest {
        input: fixture("two_speakers.wav"),
        project_dir: tmp.path().join("t.ustt"),
        models: resolve_models(&models_dir(), Backend::Cpu).unwrap(),
        backend: Backend::Cpu,
        ffmpeg_dir: None,
        make_proxy: true,
    };
    let mut last = 0.0;
    let project = run_import(&req, &AtomicBool::new(false), &mut |p| {
        assert!(
            p.fraction >= last,
            "progress went backwards: {} after {}",
            p.fraction,
            last
        );
        last = p.fraction;
    })
    .unwrap();
    assert_eq!(last, 1.0);
    assert_eq!(project.speakers.len(), 2);
    assert!(!project.segments.is_empty());
    assert!(
        project
            .segments
            .iter()
            .all(|s| s.end <= project.duration + 1e-6)
    );
    let (store, _) = ProjectStore::open(&req.project_dir).unwrap();
    assert!(store.media_dir().join("audio16k.wav").is_file());
    assert!(store.media_dir().join("waveform.bin").is_file());
    assert!(
        std::fs::read_dir(store.media_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name.ends_with(".m4a") || name.ends_with(".mp4") || name.ends_with(".webm")
            })
    );
}

#[test]
#[ignore = "needs models, libparakeet and fixture"]
fn cancel_mid_run_removes_partial_project() {
    let tmp = tempfile::tempdir().unwrap();
    let req = ImportRequest {
        input: fixture("two_speakers.wav"),
        project_dir: tmp.path().join("c.ustt"),
        models: resolve_models(&models_dir(), Backend::Cpu).unwrap(),
        backend: Backend::Cpu,
        ffmpeg_dir: None,
        make_proxy: false,
    };
    let cancel = AtomicBool::new(false);
    let err = run_import(&req, &cancel, &mut |p| {
        if p.phase == ustt_pipeline::progress::Phase::Transcribe {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .unwrap_err();
    assert!(matches!(
        err,
        ustt_pipeline::import::PipelineError::Cancelled
    ));
    assert!(!req.project_dir.exists());
    let partial = std::path::PathBuf::from(format!("{}.partial", req.project_dir.display()));
    assert!(!partial.exists());
}
