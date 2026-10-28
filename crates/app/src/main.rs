mod app;
mod components;
pub mod service;

#[cfg(any(windows, test))]
use std::path::{Path, PathBuf};

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
        #[cfg(windows)]
        prepare_webview();
        let config = Config::new()
            .with_window(
                WindowBuilder::new()
                    .with_title("Universal STT")
                    .with_inner_size(LogicalSize::new(1280.0, 800.0))
                    .with_min_inner_size(LogicalSize::new(1024.0, 700.0)),
            )
            .with_background_color((18, 20, 24, 255));
        // Diagnostic escape hatch: Wine/WebView2 experiments need Chromium flags.
        let config = match std::env::var("USTT_WEBVIEW_ARGS") {
            Ok(extra) if !extra.trim().is_empty() => config.with_windows_browser_args(extra),
            _ => config,
        };
        if let Err(err) = service::media::start_shared() {
            eprintln!("media server: {err}");
        }
        dioxus::LaunchBuilder::desktop()
            .with_cfg(config)
            .launch(app::App);
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        dioxus::launch(app::App);
    }
}

#[cfg(any(windows, test))]
fn portable_webview_data(exe_dir: &Path) -> PathBuf {
    exe_dir.join("WebView2Data")
}

/// Uses the WebView2 fixed-version runtime shipped in the portable ZIP, so the
/// user does not have to install the Evergreen runtime. Windows 10 requires
/// AppContainer ACLs for unpackaged Fixed Version apps (WebView2 >= 120).
#[cfg(windows)]
fn prepare_webview() {
    use ustt_media::process::command;
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return;
    };
    let dir = exe_dir.join("WebView2Runtime");
    if !dir.join("msedgewebview2.exe").is_file() {
        return;
    }
    if std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_none() {
        // SAFETY: called once at startup before the WebView2 environment exists.
        unsafe { std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", &dir) };
    }
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        // Avoid sharing a corrupt or incompatible cache with an installed WebView2.
        unsafe { std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", portable_webview_data(&exe_dir)) };
    }
    for sid in ["S-1-15-2-2", "S-1-15-2-1"] {
        let _ = command("icacls")
            .arg(&dir)
            .arg("/grant")
            .arg(format!("*{sid}:(OI)(CI)(RX)"))
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview_cache_is_portable_next_to_the_executable() {
        assert_eq!(
            portable_webview_data(Path::new("/opt/UniversalSTT")),
            PathBuf::from("/opt/UniversalSTT/WebView2Data")
        );
    }
}
