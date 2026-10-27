use crate::service::{ExportEvent, ExportJob, ExportRequest, Service};
use dioxus::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use ustt_core::{AudioFormat, Project};
use ustt_export::{AudioMode, TextOutput};
use ustt_ui_model::{Lang, strings};
use uuid::Uuid;

#[derive(Props, Clone, PartialEq)]
pub struct ExportModalProps {
    pub lang: Lang,
    pub project: Project,
    pub store: crate::service::StoreHandle,
    pub selected_segments: Signal<HashSet<Uuid>>,
    pub on_close: EventHandler<()>,
}

#[component]
pub fn ExportModal(props: ExportModalProps) -> Element {
    let service = use_context::<Arc<dyn Service>>();
    let text = strings(props.lang);
    let mut format = use_signal(|| TextOutput::Vtt);
    let mut speakers = use_signal(|| {
        props
            .project
            .speakers
            .iter()
            .map(|s| s.id)
            .collect::<HashSet<_>>()
    });
    let mut timestamps = use_signal(|| true);
    let mut audio = use_signal(|| AudioMode::None);
    let mut audio_format = use_signal(|| AudioFormat::Wav);
    let mut out_dir = use_signal(|| service.default_export_dir());
    let mut job = use_signal(|| None::<ExportJob>);
    let mut progress = use_signal(|| None::<f32>);
    let mut detail = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut done = use_signal(Vec::<String>::new);

    let run_project = props.project.clone();
    let run_selected = props.selected_segments;
    let run = {
        let service = service.clone();
        move |_| {
            error.set(None);
            done.set(Vec::new());
            let selected = run_selected();
            let all_speakers = run_project
                .speakers
                .iter()
                .all(|s| speakers().contains(&s.id));
            let request = ExportRequest {
                format: format(),
                speakers: if all_speakers {
                    None
                } else {
                    Some(speakers().into_iter().collect())
                },
                timestamps: timestamps(),
                audio: audio(),
                audio_format: audio_format(),
                segments: (audio() == AudioMode::Segments).then(|| selected.into_iter().collect()),
                out_dir: out_dir().unwrap_or_else(|| PathBuf::from(".")),
            };
            let job_value = service.export(&props.store, run_project.clone(), request);
            job.set(Some(job_value));
            progress.set(Some(0.0));
            spawn(async move {
                loop {
                    futures_timer::Delay::new(std::time::Duration::from_millis(80)).await;
                    let event = {
                        let guard = job.read();
                        guard.as_ref().and_then(|j| j.rx.try_recv().ok())
                    };
                    match event {
                        Some(ExportEvent::Progress {
                            fraction,
                            detail: line,
                        }) => {
                            progress.set(Some(fraction));
                            detail.set(line);
                        }
                        Some(ExportEvent::Done(files)) => {
                            progress.set(Some(1.0));
                            done.set(files.iter().map(|p| p.display().to_string()).collect());
                            job.set(None);
                            break;
                        }
                        Some(ExportEvent::Download { filename, bytes }) => {
                            download(&filename, &bytes);
                            done.set(vec![filename]);
                            job.set(None);
                            break;
                        }
                        Some(ExportEvent::Failed(message)) => {
                            error.set(Some(message));
                            job.set(None);
                            progress.set(None);
                            break;
                        }
                        Some(ExportEvent::Cancelled) => {
                            error.set(Some("export cancelled".into()));
                            job.set(None);
                            progress.set(None);
                            break;
                        }
                        None => {}
                    }
                }
            });
        }
    };

    let choose_dir = {
        let service = service.clone();
        move |_| {
            if let Some(dir) = service.pick_project() {
                out_dir.set(Some(dir));
            }
        }
    };

    let busy = job.read().is_some();
    let segment_count = (props.selected_segments)().len();

    rsx! {
        div { class: "scrim", onclick: move |_| props.on_close.call(()),
            div {
                class: "modal export-modal",
                "data-testid": "export-modal",
                onclick: move |evt| evt.stop_propagation(),
                h2 { "{text.export}" }
                label { class: "field",
                    span { "{text.export_format}" }
                    select {
                        "data-testid": "export-format",
                        value: "{format():?}",
                        onchange: move |evt| {
                            if let Some(value) = TextOutput::parse(&evt.value()) {
                                format.set(value);
                            }
                        },
                        for value in [TextOutput::Docx, TextOutput::Pdf, TextOutput::Srt, TextOutput::Vtt, TextOutput::Txt, TextOutput::Markdown] {
                            option { value: "{value:?}", selected: format() == value, "{value:?}" }
                        }
                    }
                }
                div { class: "field",
                    span { "{text.speakers}" }
                    div { class: "chip-row",
                        for speaker in props.project.speakers.clone() {
                            label { class: "check",
                                input {
                                    r#type: "checkbox",
                                    "data-testid": "export-speaker",
                                    checked: speakers().contains(&speaker.id),
                                    onchange: move |_| {
                                        let mut next = speakers();
                                        if !next.remove(&speaker.id) {
                                            next.insert(speaker.id);
                                        }
                                        speakers.set(next);
                                    },
                                }
                                span { class: "dot", style: "background: var(--speaker-{speaker.color % 8})" }
                                "{speaker.name}"
                            }
                        }
                    }
                }
                label { class: "check",
                    input {
                        r#type: "checkbox",
                        checked: timestamps(),
                        onchange: move |_| timestamps.toggle(),
                    }
                    "{text.export_timestamps}"
                }
                label { class: "field",
                    span { "{text.export_audio}" }
                    select {
                        value: "{audio():?}",
                        onchange: move |evt| {
                            audio.set(match evt.value().as_str() {
                                "Speakers" => AudioMode::Speakers,
                                "Segments" => AudioMode::Segments,
                                _ => AudioMode::None,
                            });
                        },
                        option { value: "None", "{text.export_audio_none}" }
                        option { value: "Speakers", "{text.export_audio_speakers}" }
                        option { value: "Segments", "{text.export_audio_segments} ({segment_count})" }
                    }
                }
                label { class: "field",
                    span { "{text.export_audio_format}" }
                    select {
                        value: "{audio_format():?}",
                        onchange: move |evt| {
                            audio_format.set(if evt.value() == "M4a" { AudioFormat::M4a } else { AudioFormat::Wav });
                        },
                        option { value: "Wav", "WAV" }
                        option { value: "M4a", "M4A" }
                    }
                }
                div { class: "field",
                    span { "{text.export_folder}" }
                    div { class: "row",
                        code { "{out_dir().map(|p| p.display().to_string()).unwrap_or_else(|| text.export_download_folder.into())}" }
                        button { class: "btn ghost small", onclick: choose_dir, "{text.choose_folder}" }
                    }
                }
                if let Some(fraction) = progress() {
                    div { class: "progress", "data-testid": "export-progress",
                        div { class: "bar", style: "width: {fraction * 100.0}%" }
                    }
                    p { class: "muted", "{detail}" }
                }
                if let Some(message) = error() {
                    p { class: "error", "{message}" }
                }
                for file in done() {
                    p { class: "ok", "{file}" }
                }
                div { class: "modal-actions",
                    button { class: "btn ghost", onclick: move |_| props.on_close.call(()), "{text.cancel}" }
                    if busy {
                        button {
                            class: "btn danger",
                            onclick: move |_| {
                                if let Some(j) = job.read().as_ref() {
                                    j.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                                }
                            },
                            "{text.cancel}"
                        }
                    } else {
                        button {
                            class: "btn primary",
                            "data-testid": "export-submit",
                            onclick: run,
                            "{text.export}"
                        }
                    }
                }
            }
        }
    }
}

fn download(filename: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let payload = serde_json::to_string(text.as_ref()).unwrap_or_else(|_| "\"\"".into());
    let name = serde_json::to_string(filename).unwrap_or_else(|_| "\"export.txt\"".into());
    let script = format!(
        "(() => {{const a=document.createElement('a');const b=new Blob([{payload}],{{type:'text/plain'}});a.href=URL.createObjectURL(b);a.download={name};a.click();}})();"
    );
    spawn(async move {
        let _ = document::eval(&script).await;
    });
}
