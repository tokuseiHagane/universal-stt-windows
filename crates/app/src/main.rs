mod app;
mod components;
mod service;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
        let config = Config::new()
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
