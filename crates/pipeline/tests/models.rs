use ustt_pipeline::backend::Backend;
use ustt_pipeline::models::{ModelError, resolve_models, verify_models};

#[test]
fn resolve_models_reports_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let err = resolve_models(tmp.path(), Backend::Cpu).unwrap_err();
    assert!(matches!(err, ModelError::Missing { .. }));
}

#[test]
fn verify_models_detects_bad_hash() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("redux-packed.gguf"), b"x").unwrap();
    let checks = verify_models(tmp.path());
    assert!(
        checks
            .iter()
            .any(|c| !c.ok && c.name == "redux-packed.gguf")
    );
}
