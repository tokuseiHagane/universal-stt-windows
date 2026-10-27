mod app;
mod components;
pub mod service;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
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
        dioxus::LaunchBuilder::desktop()
            .with_cfg(config)
            .launch(app::App);
    }
    #[cfg(target_arch = "wasm32")]
    dioxus::launch(app::App);
}
