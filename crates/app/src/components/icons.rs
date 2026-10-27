use dioxus::prelude::*;

#[component]
pub fn PlayIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "18", height: "18", path { d: "M8 5v14l11-7z", fill: "currentColor" } } }
}

#[component]
pub fn PauseIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "18", height: "18", path { d: "M6 5h4v14H6zM14 5h4v14h-4z", fill: "currentColor" } } }
}

#[component]
pub fn SaveIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "18", height: "18", path { d: "M5 3h11l3 3v15H5zM8 3v6h8V3M8 14h8v7H8z", fill: "none", stroke: "currentColor", stroke_width: "1.6" } } }
}

#[component]
pub fn UndoIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "18", height: "18", path { d: "M9 7 4 12l5 5M4 12h9a6 6 0 0 1 6 6", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round" } } }
}

#[component]
pub fn RedoIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "18", height: "18", path { d: "m15 7 5 5-5 5M20 12h-9a6 6 0 0 0-6 6", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round" } } }
}

#[component]
pub fn SettingsIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "18", height: "18", path { d: "M12 15.5A3.5 3.5 0 1 0 12 8.5a3.5 3.5 0 0 0 0 7zM4 12l-2 1 2 1m16-2 2 1-2 1M12 4l1-2 1 2m-2 16 1 2 1-2", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linecap: "round" } } }
}

#[component]
pub fn FolderIcon() -> Element {
    rsx! { svg { view_box: "0 0 24 24", width: "28", height: "28", path { d: "M3 6h6l2 2h10v10H3z", fill: "none", stroke: "currentColor", stroke_width: "1.5" } } }
}
