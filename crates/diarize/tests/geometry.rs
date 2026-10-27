use ustt_diarize::voices;

#[test]
fn windows_stop_when_margin_leaves_no_progress() {
    assert!(voices::windows(1_000, 100, 50, 8).is_empty());
    assert!(voices::windows(1_000, 100, 80, 8).is_empty());
}

#[test]
fn windows_cover_the_recording() {
    let windows = voices::windows(3_256, 3_256, 128, 8);
    assert!(!windows.is_empty());
    assert_eq!(windows[0].start, 0);
}
