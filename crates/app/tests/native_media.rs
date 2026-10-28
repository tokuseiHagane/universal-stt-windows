#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::sync::Mutex;
use ustt_app::service::native::NativeService;
use ustt_app::service::{MediaAsset, Service, media};
use ustt_core::{MediaKind, ProjectStore, SourceRef};

static MEDIA_TEST: Mutex<()> = Mutex::new(());

fn source(name: &str) -> SourceRef {
    SourceRef {
        path: PathBuf::from(format!("{name}.mp4")),
        size_bytes: 1,
        modified_unix: 0,
        fingerprint: name.into(),
        origin_url: None,
    }
}

#[test]
fn native_media_url_uses_http_and_switches_project_roots() {
    let _guard = MEDIA_TEST.lock().unwrap();
    media::start_shared().unwrap();
    let service = NativeService::open();
    let temp = tempfile::tempdir().unwrap();

    let first_dir = temp.path().join("first.ustt");
    let second_dir = temp.path().join("second.ustt");
    let (first_store, _) =
        ProjectStore::create(&first_dir, "First", source("first"), MediaKind::Video, 3.0).unwrap();
    ProjectStore::create(
        &second_dir,
        "Second",
        source("second"),
        MediaKind::Video,
        3.0,
    )
    .unwrap();
    std::fs::write(first_store.media_dir().join("first.webm"), b"first").unwrap();
    std::fs::write(second_dir.join("media/second.webm"), b"second").unwrap();

    let (first_handle, _) = service.open(&first_dir).unwrap();
    let url = service.media_url(&first_handle, MediaAsset::Proxy).unwrap();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    assert!(url.ends_with("/media/proxy"), "{url}");
    assert!(!url.starts_with("ustt://"));

    let (second_handle, _) = service.open(&second_dir).unwrap();
    let second_url = service
        .media_url(&second_handle, MediaAsset::Proxy)
        .unwrap();
    assert!(second_url.starts_with("http://127.0.0.1:"));
    let (resolved, _) = media::shared()
        .lock()
        .unwrap()
        .resolve("/media/proxy")
        .unwrap();
    assert_eq!(
        resolved,
        second_dir.join("media/second.webm").canonicalize().unwrap()
    );
}
