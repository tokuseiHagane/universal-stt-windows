use std::sync::atomic::AtomicBool;
use ustt_pipeline::backend::Backend;
use ustt_pipeline::import::{ImportRequest, ImportSource, PipelineError, run_import};
use ustt_pipeline::models::ModelPaths;
use ustt_pipeline::progress::{
    PHASE_WEIGHTS, Phase, Progress, estimate_eta, phase_start, with_overall_eta, with_phase_eta,
};

#[test]
fn phase_fractions_are_monotonic() {
    assert_eq!(phase_start(Phase::Probe), 0.0);
    assert!((phase_start(Phase::Transcribe) - 0.35).abs() < 1e-9);
    assert_eq!(phase_start(Phase::Done), 1.0);
    assert!(PHASE_WEIGHTS.iter().sum::<f32>() > 0.999);
}

#[test]
fn eta_extrapolates_from_processed_audio() {
    let eta = estimate_eta(60.0, 600.0, 6.0);
    assert!((eta - 54.0).abs() < 1e-6);
}

#[test]
fn every_active_phase_gets_an_overall_eta() {
    let progress = with_overall_eta(Progress::new(Phase::Diarize, 0.5, ""), 20.0);
    assert!((progress.fraction - 0.25).abs() < 1e-6);
    assert!((progress.eta_seconds.unwrap() - 60.0).abs() < 1e-6);
    assert_eq!(
        with_overall_eta(Progress::new(Phase::Probe, 0.0, ""), 20.0).eta_seconds,
        None
    );
}

#[test]
fn phase_eta_uses_local_fraction_and_preserves_overall_eta() {
    let progress = with_phase_eta(Progress::new(Phase::Diarize, 0.25, "").with_eta(60.0), 10.0);
    assert!((progress.phase_fraction - 0.25).abs() < 1e-6);
    assert!((progress.phase_eta_seconds.unwrap() - 30.0).abs() < 1e-6);
    assert_eq!(progress.eta_seconds, Some(60.0));
    assert_eq!(
        with_phase_eta(Progress::new(Phase::Probe, 0.0, ""), 10.0).phase_eta_seconds,
        None
    );
    assert_eq!(
        with_phase_eta(Progress::new(Phase::Done, 1.0, ""), 10.0).phase_eta_seconds,
        None
    );
}

#[test]
fn remote_progress_reserves_download_share() {
    let download = Progress::new_scaled(Phase::Download, 0.5, "downloading", 0.25);
    assert!((download.fraction - 0.125).abs() < 1e-6);
    let probe = Progress::new_scaled(Phase::Probe, 0.5, "probing", 0.25);
    assert!((probe.fraction - 0.2575).abs() < 1e-6);
    assert_eq!(Progress::new_scaled(Phase::Done, 1.0, "done", 0.25).fraction, 1.0);
    assert_eq!(
        Progress::new_scaled(Phase::Diarize, 0.5, "diarizing", 0.0).fraction,
        Progress::new(Phase::Diarize, 0.5, "diarizing").fraction
    );
}

#[test]
fn import_source_accepts_http_urls_and_local_paths_only() {
    assert_eq!(
        ImportSource::parse("  https://example.test/clip.mp4  ").unwrap(),
        ImportSource::Url("https://example.test/clip.mp4".into())
    );
    assert_eq!(
        ImportSource::parse("recording.wav").unwrap(),
        ImportSource::File("recording.wav".into())
    );
    assert!(ImportSource::parse("ftp://example.test/clip.mp4").is_err());
    assert!(ImportSource::parse("https://").is_err());
}

#[test]
fn cancel_leaves_no_partial_project() {
    let tmp = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(true);
    let req = ImportRequest {
        input: ImportSource::File(tmp.path().join("missing.mp4")),
        project_dir: tmp.path().join("t.ustt"),
        models: ModelPaths {
            who_dir: tmp.path().join("who"),
            asr_model: tmp.path().join("m.gguf"),
            silero_vad: tmp.path().join("v.gguf"),
        },
        backend: Backend::Cpu,
        ffmpeg_dir: None,
        make_proxy: false,
    };
    let err = run_import(&req, &cancel, &mut |_| {}).unwrap_err();
    assert!(matches!(err, PipelineError::Cancelled));
    assert!(!req.project_dir.exists() || !req.project_dir.join("project.json").exists());
}
