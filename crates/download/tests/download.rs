use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ustt_download::{DownloadError, DownloadTools, download, download_in_dir};

#[test]
fn locate_reports_missing_tools() {
    let dir = tempfile::tempdir().unwrap();
    let err = DownloadTools::locate(Some(dir.path())).unwrap_err();
    assert!(matches!(err, DownloadError::Tools(_)));
    assert!(err.to_string().contains("yt-dlp"));
    assert!(err.to_string().contains("deno"));
}

#[test]
fn rejects_non_http_urls_before_spawning() {
    let dir = tempfile::tempdir().unwrap();
    let tools = DownloadTools::new(dir.path().join("yt-dlp"), dir.path().join("deno"));
    let err = download(
        &tools,
        "file:///tmp/a.wav",
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, DownloadError::InvalidUrl(_)));
}

#[cfg(unix)]
mod fake_tool {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    pub struct Fake {
        pub tools: DownloadTools,
        pub tool_dir: PathBuf,
        _dir: tempfile::TempDir,
    }

    pub fn install(script: &str) -> Fake {
        let dir = tempfile::tempdir().unwrap();
        let yt = dir.path().join("yt-dlp");
        let deno = dir.path().join("deno");
        fs::write(&yt, script).unwrap();
        fs::write(&deno, "").unwrap();
        let mut permissions = fs::metadata(&yt).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&yt, permissions).unwrap();
        Fake {
            tools: DownloadTools::new(yt, deno),
            tool_dir: dir.path().to_path_buf(),
            _dir: dir,
        }
    }

    pub fn success_script() -> String {
        r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/args.log"
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--paths" ]; then out="$2"; shift 2; else shift; fi
done
printf 'USTT_TITLE|Demo video\n'
printf 'USTT_PROGRESS|50|100|100|50.0\n'
printf 'USTT_PROGRESS|100|100|100|100.0\n'
printf 'media' > "$out/demo.webm"
printf 'USTT_FILE|%s/demo.webm\n' "$out"
"#
        .to_string()
    }

    #[test]
    fn success_parses_progress_title_path_and_arguments() {
        let fake = install(&success_script());
        let parent = tempfile::tempdir().unwrap();
        let mut updates = Vec::new();
        let media = download_in_dir(
            &fake.tools,
            "https://example.com/video",
            parent.path(),
            Arc::new(AtomicBool::new(false)),
            |p| updates.push(p),
        )
        .unwrap();

        assert_eq!(media.title, "Demo video");
        assert_eq!(fs::read(&media.path).unwrap(), b"media");
        assert!(media.path.starts_with(media.temp.path()));
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].downloaded_bytes, 50);
        assert_eq!(updates[0].total_bytes, Some(100));
        assert_eq!(updates[0].fraction, Some(0.5));

        let args = fs::read_to_string(fake.tool_dir.join("args.log")).unwrap();
        assert!(args.contains("--no-playlist"));
        assert!(args.contains("--newline"));
        assert!(args.contains("--js-runtimes"));
        assert!(args.contains(&format!("deno:{}", fake.tools.deno.display())));
        assert!(args.contains("bv*[height<=1080]+ba/b[height<=1080]/b"));
    }

    #[test]
    fn non_ascii_temp_path_is_supported() {
        let fake = install(&success_script());
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("Загрузки");
        fs::create_dir(&parent).unwrap();
        let media = download_in_dir(
            &fake.tools,
            "https://example.com/video",
            &parent,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap();
        assert!(media.path.exists());
        assert!(media.path.to_string_lossy().contains("Загрузки"));
    }

    #[test]
    fn cancellation_kills_tool_and_removes_partial_directory() {
        let fake = install(
            r#"#!/bin/sh
printf 'USTT_PROGRESS|1|100|100|1.0\n'
sleep 30
"#,
        );
        let parent = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let err = download_in_dir(
            &fake.tools,
            "https://example.com/video",
            parent.path(),
            cancel,
            move |_| flag.store(true, Ordering::SeqCst),
        )
        .unwrap_err();
        assert!(matches!(err, DownloadError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }

    #[test]
    fn classifies_tool_errors() {
        for (message, expected) in [
            ("ERROR: Unsupported URL: https://example.com", "unsupported"),
            ("ERROR: Sign in to confirm your account", "login"),
            ("ERROR: This video is DRM protected", "drm"),
            ("ERROR: network unreachable", "failed"),
        ] {
            let fake = install(&format!(
                "#!/bin/sh\nprintf '%s\\n' '{message}' >&2\nexit 2\n"
            ));
            let parent = tempfile::tempdir().unwrap();
            let err = download_in_dir(
                &fake.tools,
                "https://example.com/video",
                parent.path(),
                Arc::new(AtomicBool::new(false)),
                |_| {},
            )
            .unwrap_err();
            let matched = matches!(
                (expected, err),
                ("unsupported", DownloadError::Unsupported(_))
                    | ("login", DownloadError::Authentication(_))
                    | ("drm", DownloadError::Drm(_))
                    | ("failed", DownloadError::Failed(_))
            );
            assert!(matched, "unexpected error for {message}");
            assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn long_non_ascii_error_does_not_panic() {
        let fake = install(
            "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 1500 ]; do printf 'Ошибка загрузки видео\\n' >&2; i=$((i+1)); done\nexit 2\n",
        );
        let parent = tempfile::tempdir().unwrap();
        let err = download_in_dir(
            &fake.tools,
            "https://example.com/video",
            parent.path(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, DownloadError::Failed(_)));
    }

    #[test]
    fn missing_final_file_is_an_error() {
        let fake = install("#!/bin/sh\nprintf 'USTT_TITLE|No file\\n'\n");
        let parent = tempfile::tempdir().unwrap();
        let err = download_in_dir(
            &fake.tools,
            "https://example.com/video",
            parent.path(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, DownloadError::Failed(_)));
    }

    #[test]
    fn waits_for_progress_without_fixed_delays() {
        let fake = install(&success_script());
        let parent = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        download_in_dir(
            &fake.tools,
            "https://example.com/video",
            parent.path(),
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
