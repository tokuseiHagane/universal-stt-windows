use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use ustt_core::ProjectStore;
use ustt_pipeline::backend::Backend;
use ustt_pipeline::import::{ImportRequest, ImportSource, run_import};
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
        input: ImportSource::File(fixture("two_speakers.wav")),
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
        input: ImportSource::File(fixture("two_speakers.wav")),
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

#[cfg(unix)]
#[test]
#[ignore = "needs models, libparakeet, FFmpeg and fixture"]
fn imports_url_through_the_same_pipeline_and_records_origin() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let tools_dir = tmp.path().join("tools");
    std::fs::create_dir(&tools_dir).unwrap();
    let fixture = fixture("two_speakers.wav");
    let script = tools_dir.join("yt-dlp");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--paths\" ]; then shift; out=\"$1\"; fi\n  shift\ndone\ncp '{}' \"$out/id.wav\"\nprintf 'USTT_TITLE|Remote clip\\n'\nprintf 'USTT_FILE|%s\\n' \"$out/id.wav\"\n",
            fixture.display()
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();
    std::fs::write(tools_dir.join("deno"), "").unwrap();
    unsafe { std::env::set_var("USTT_DOWNLOAD_DIR", &tools_dir) };

    let url = "https://example.test/two_speakers.wav";
    let req = ImportRequest {
        input: ImportSource::Url(url.into()),
        project_dir: tmp.path().join("remote.ustt"),
        models: resolve_models(&models_dir(), Backend::Cpu).unwrap(),
        backend: Backend::Cpu,
        ffmpeg_dir: None,
        make_proxy: false,
    };
    let mut phases = Vec::new();
    let project = run_import(&req, &AtomicBool::new(false), &mut |progress| {
        phases.push(progress.phase);
    })
    .unwrap();
    assert_eq!(project.title, "Remote clip");
    assert_eq!(project.source.origin_url.as_deref(), Some(url));
    assert!(phases.contains(&ustt_pipeline::progress::Phase::Download));
    assert!(!project.segments.is_empty());
}
