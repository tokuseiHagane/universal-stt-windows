use std::sync::atomic::AtomicBool;
use ustt_pipeline::backend::Backend;
use ustt_pipeline::import::{ImportRequest, PipelineError, run_import};
use ustt_pipeline::models::ModelPaths;
use ustt_pipeline::progress::{PHASE_WEIGHTS, Phase, estimate_eta, phase_start};

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
fn cancel_leaves_no_partial_project() {
    let tmp = tempfile::tempdir().unwrap();
    let cancel = AtomicBool::new(true);
    let req = ImportRequest {
        input: tmp.path().join("missing.mp4"),
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
