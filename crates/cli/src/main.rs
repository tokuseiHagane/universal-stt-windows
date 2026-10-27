use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use ustt_core::BackendPreference;
use ustt_core::{AudioFormat, ProjectStore};
use ustt_export::{
    TextOutput, extract, lines, plan_segments, plan_speakers, render_docx, render_pdf, render_text,
};
use ustt_media::{Tools, probe};
use ustt_pipeline::backend::{detect, detect_with};
use ustt_pipeline::import::{ImportRequest, run_import};
use ustt_pipeline::models::{resolve_models, verify_models};
use ustt_pipeline::progress::Progress;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        usage();
        return Ok(());
    }
    match args[0].as_str() {
        "probe" => {
            let file = args.get(1).ok_or("probe needs a file")?;
            let tools = Tools::locate(None).map_err(|e| e.to_string())?;
            let info = probe(&tools, std::path::Path::new(file)).map_err(|e| e.to_string())?;
            let value = serde_json::json!({
                "duration": info.duration,
                "kind": if info.video.is_some() { "video" } else { "audio" },
                "video": info.video,
                "audio": info.audio,
            });
            println!("{value}");
            Ok(())
        }
        "doctor" => {
            let models = flag(&args, "--models").unwrap_or_else(|| "models".into());
            let detection = detect(BackendPreference::Auto);
            let checks = verify_models(std::path::Path::new(&models));
            let value = serde_json::json!({
                "backend": format!("{:?}", detection.selected).to_lowercase(),
                "cuda": detection.cuda_available,
                "vulkan": detection.vulkan_available,
                "hip": detection.hip_present,
                "gpu": detection.gpu_name,
                "models": checks,
            });
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Ok(())
        }
        "process" => process(&args),
        "export" => export(&args),
        other => Err(format!("unknown command {other}\n\n{}", usage_text())),
    }
}

fn process(args: &[String]) -> Result<(), String> {
    let input = args.get(1).ok_or("process needs a file")?;
    let project = flag(args, "--project").ok_or("--project is required")?;
    let models = flag(args, "--models").ok_or("--models is required")?;
    let backend_arg = flag(args, "--backend")
        .or_else(|| std::env::var("USTT_BACKEND").ok())
        .unwrap_or_else(|| "auto".into());
    let pref = match backend_arg.as_str() {
        "auto" => BackendPreference::Auto,
        "cpu" => BackendPreference::Cpu,
        "vulkan" => BackendPreference::Vulkan,
        "cuda" => BackendPreference::Cuda,
        other => return Err(format!("unknown backend {other}")),
    };
    let detection = detect_with(pref, Some(backend_arg.as_str()));
    let models = resolve_models(std::path::Path::new(&models), detection.selected)
        .map_err(|e| e.to_string())?;
    let req = ImportRequest {
        input: PathBuf::from(input),
        project_dir: PathBuf::from(&project),
        models,
        backend: detection.selected,
        make_proxy: !args.iter().any(|a| a == "--no-proxy"),
    };
    let cancel = AtomicBool::new(false);
    let result = run_import(&req, &cancel, &mut |progress| {
        println!("{}", progress_json(&progress));
    })
    .map_err(|e| e.to_string())?;
    let _ = result;
    println!(
        "{}",
        serde_json::json!({"status": "done", "project": project})
    );
    Ok(())
}

#[derive(Debug, PartialEq)]
struct ExportArgs {
    project: PathBuf,
    out: PathBuf,
    format: TextOutput,
    speakers: Option<Vec<uuid::Uuid>>,
    timestamps: bool,
    audio: AudioModeArg,
    segments: Option<Vec<uuid::Uuid>>,
    audio_format: AudioFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioModeArg {
    None,
    Speakers,
    Segments,
}

fn parse_export(args: &[String]) -> Result<ExportArgs, String> {
    let project = args.first().ok_or("export needs a project directory")?;
    if project.starts_with("--") {
        return Err("export needs a project directory".into());
    }
    let out = flag(args, "--out").ok_or("--out is required")?;
    let format = flag(args, "--format")
        .as_deref()
        .map(|value| TextOutput::parse(value).ok_or_else(|| format!("unknown format {value}")))
        .transpose()?
        .unwrap_or(TextOutput::Docx);
    let speakers = parse_ids(flag(args, "--speakers"))?;
    let segments = parse_ids(flag(args, "--segments"))?;
    let audio = match flag(args, "--audio").as_deref().unwrap_or("none") {
        "none" => AudioModeArg::None,
        "speakers" => AudioModeArg::Speakers,
        "segments" => AudioModeArg::Segments,
        other => return Err(format!("unknown audio mode {other}")),
    };
    let audio_format = match flag(args, "--audio-format").as_deref().unwrap_or("wav") {
        "wav" => AudioFormat::Wav,
        "m4a" => AudioFormat::M4a,
        other => return Err(format!("unknown audio format {other}")),
    };
    if audio == AudioModeArg::Segments && segments.as_ref().is_none_or(|ids| ids.is_empty()) {
        return Err("--segments is required for audio=segments".into());
    }
    Ok(ExportArgs {
        project: PathBuf::from(project),
        out: PathBuf::from(out),
        format,
        speakers,
        timestamps: !args.iter().any(|a| a == "--no-timestamps"),
        audio,
        segments,
        audio_format,
    })
}

fn parse_ids(raw: Option<String>) -> Result<Option<Vec<uuid::Uuid>>, String> {
    let Some(raw) = raw else { return Ok(None) };
    let mut ids = Vec::new();
    for value in raw.split(',').filter(|v| !v.is_empty()) {
        ids.push(uuid::Uuid::parse_str(value).map_err(|e| format!("bad id {value}: {e}"))?);
    }
    Ok(Some(ids))
}

fn export(args: &[String]) -> Result<(), String> {
    let parsed = parse_export(&args[1..])?;
    let (store, project) = ProjectStore::open(&parsed.project).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&parsed.out).map_err(|e| e.to_string())?;
    let selected = parsed.speakers.as_deref();
    let export_lines = lines(&project, selected);
    let stem = format!(
        "{}-{}",
        ustt_core::slugify(&project.title),
        chrono::Utc::now().format("%Y-%m-%d")
    );
    let mut files = Vec::new();
    let text_bytes = match parsed.format {
        TextOutput::Docx => Some(
            render_docx(&project, &export_lines, parsed.timestamps).map_err(|e| e.to_string())?,
        ),
        TextOutput::Pdf => Some(
            render_pdf(&project, &export_lines, parsed.timestamps).map_err(|e| e.to_string())?,
        ),
        other => Some(render_text(&export_lines, other, parsed.timestamps).into_bytes()),
    };
    if let Some(bytes) = text_bytes {
        let path = parsed
            .out
            .join(format!("{stem}.{}", parsed.format.extension()));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        files.push(path);
    }
    if parsed.audio != AudioModeArg::None {
        let plan = match parsed.audio {
            AudioModeArg::Speakers => plan_speakers(&project, selected),
            AudioModeArg::Segments => {
                let ids = parsed
                    .segments
                    .ok_or("--segments is required for audio=segments")?;
                plan_segments(&project, &ids)
            }
            AudioModeArg::None => unreachable!(),
        };
        let tools = Tools::locate(None).map_err(|e| e.to_string())?;
        let wav = store.media_dir().join("audio16k.wav");
        let mut extracted = match extract(
            &tools,
            &wav,
            &plan,
            &stem,
            parsed.audio_format,
            &parsed.out,
            &AtomicBool::new(false),
            &mut |_, _| {},
        ) {
            Ok(files) => files,
            Err(err) => {
                for file in &files {
                    let _ = std::fs::remove_file(file);
                }
                return Err(err.to_string());
            }
        };
        files.append(&mut extracted);
    }
    println!("{}", serde_json::json!({ "files": files }));
    Ok(())
}

fn progress_json(progress: &Progress) -> String {
    serde_json::json!({
        "phase": format!("{:?}", progress.phase).to_lowercase(),
        "fraction": progress.fraction,
        "eta_seconds": progress.eta_seconds,
        "detail": progress.detail,
    })
    .to_string()
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn usage() {
    print!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Universal STT CLI\n\n\
     ustt-cli probe <file>\n\
     ustt-cli doctor [--models <dir>]\n\
     ustt-cli process <file> --project <dir> --models <dir> [--backend auto|cpu|vulkan|cuda] [--no-proxy]\n\
     ustt-cli export <project.ustt> --out <dir> [--format docx|pdf|srt|vtt|txt|md] [--speakers <uuid,...>] [--no-timestamps]\n\
       [--audio none|speakers|segments] [--segments <uuid,...>] [--audio-format wav|m4a]\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_args_parse_minimal() {
        let args = vec![
            "p.ustt".to_string(),
            "--out".into(),
            "/tmp".into(),
            "--format".into(),
            "srt".into(),
        ];
        let parsed = parse_export(&args).unwrap();
        assert_eq!(parsed.format, TextOutput::Srt);
        assert!(parsed.timestamps);
        assert_eq!(parsed.audio, AudioModeArg::None);
    }

    #[test]
    fn export_args_require_out() {
        let args = vec!["p.ustt".to_string()];
        assert!(parse_export(&args).unwrap_err().contains("--out"));
    }

    #[test]
    fn export_args_parse_speakers_and_audio() {
        let id = uuid::Uuid::nil();
        let args = vec![
            "p.ustt".into(),
            "--out".into(),
            "/tmp".into(),
            "--speakers".into(),
            id.to_string(),
            "--audio".into(),
            "segments".into(),
            "--segments".into(),
            id.to_string(),
            "--no-timestamps".into(),
            "--audio-format".into(),
            "m4a".into(),
        ];
        let parsed = parse_export(&args).unwrap();
        assert_eq!(parsed.speakers, Some(vec![id]));
        assert_eq!(parsed.segments, Some(vec![id]));
        assert!(!parsed.timestamps);
        assert_eq!(parsed.audio_format, AudioFormat::M4a);
    }
}
