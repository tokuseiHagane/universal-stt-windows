mod app;
mod components;
pub mod service;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
        #[cfg(windows)]
        prepare_webview();
        let config = Config::new()
            .with_asynchronous_custom_protocol("ustt", |_id, request, responder| {
                let uri = request.uri().to_string();
                let method = request.method().as_str().to_string();
                let range = request
                    .headers()
                    .get("range")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                responder.respond(service::media::respond(&uri, &method, range.as_deref()));
            })
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

/// Uses the WebView2 fixed-version runtime shipped in the portable ZIP, so the
/// user does not have to install the Evergreen runtime. Windows 10 requires
/// AppContainer ACLs for unpackaged Fixed Version apps (WebView2 >= 120).
#[cfg(windows)]
fn prepare_webview() {
    use std::process::Command;
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("WebView2Runtime")))
    else {
        return;
    };
    if !dir.join("msedgewebview2.exe").is_file() {
        return;
    }
    if std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_none() {
        // SAFETY: called once at startup before the WebView2 environment exists.
        unsafe { std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", &dir) };
    }
    for sid in ["S-1-15-2-2", "S-1-15-2-1"] {
        let _ = Command::new("icacls")
            .arg(&dir)
            .arg("/grant")
            .arg(format!("*{sid}:(OI)(CI)(RX)"))
            .status();
    }
}
