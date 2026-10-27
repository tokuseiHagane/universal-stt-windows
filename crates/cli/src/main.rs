use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use ustt_core::BackendPreference;
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
        other => Err(format!("unknown command {other}\n\n{}", usage_text())),
    }
}

fn process(args: &[String]) -> Result<(), String> {
    let input = args.get(1).ok_or("process needs a file")?;
    let project = flag(args, "--project").ok_or("--project is required")?;
    let models = flag(args, "--models").ok_or("--models is required")?;
    let backend_arg = flag(args, "--backend").unwrap_or_else(|| "auto".into());
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
     ustt-cli process <file> --project <dir> --models <dir> [--backend auto|cpu|vulkan|cuda] [--no-proxy]\n"
}
