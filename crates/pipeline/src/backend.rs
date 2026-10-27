use std::path::{Path, PathBuf};
use ustt_core::BackendPreference;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    Vulkan,
    Cuda,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub selected: Backend,
    pub cuda_available: bool,
    pub vulkan_available: bool,
    pub hip_present: bool,
    pub gpu_name: Option<String>,
}

pub fn parakeet_lib(backend: Backend) -> &'static str {
    match backend {
        Backend::Cpu => "parakeet_cpu.dll",
        Backend::Vulkan => "parakeet_vulkan.dll",
        Backend::Cuda => "parakeet_cuda.dll",
    }
}

pub fn asr_model_file(backend: Backend) -> &'static str {
    match backend {
        Backend::Cpu => "redux-packed.gguf",
        _ => "redux-q8_0.gguf",
    }
}

pub fn detect(pref: BackendPreference) -> Detection {
    detect_with(pref, std::env::var("USTT_BACKEND").ok().as_deref())
}

pub fn detect_with(pref: BackendPreference, env_override: Option<&str>) -> Detection {
    let vulkan = vulkan_probe();
    let (vulkan_available, gpu_name) = vulkan.unwrap_or((false, None));
    let cuda_available = cuda_driver_present() && library_present(parakeet_lib(Backend::Cuda));
    let hip_present = library_present("amdhip64.dll") || library_present("libamdhip64.so");

    let forced = env_override.and_then(parse_backend);

    // GPU backends ship as Windows DLLs; a dev build on Linux runs the CPU path.
    #[cfg(not(windows))]
    let selected = {
        let _ = pref;
        forced.unwrap_or(Backend::Cpu)
    };

    #[cfg(windows)]
    let selected = if let Some(forced) = forced {
        forced
    } else {
        match pref {
            BackendPreference::Cpu => Backend::Cpu,
            BackendPreference::Cuda if cuda_available => Backend::Cuda,
            BackendPreference::Vulkan if vulkan_available => Backend::Vulkan,
            BackendPreference::Cuda | BackendPreference::Vulkan => {
                if cuda_available {
                    Backend::Cuda
                } else if vulkan_available {
                    Backend::Vulkan
                } else {
                    Backend::Cpu
                }
            }
            BackendPreference::Auto => {
                if cuda_available {
                    Backend::Cuda
                } else if vulkan_available {
                    Backend::Vulkan
                } else {
                    Backend::Cpu
                }
            }
        }
    };

    Detection {
        selected,
        cuda_available,
        vulkan_available,
        hip_present,
        gpu_name,
    }
}

fn parse_backend(value: &str) -> Option<Backend> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Some(Backend::Cpu),
        "vulkan" => Some(Backend::Vulkan),
        "cuda" => Some(Backend::Cuda),
        _ => None,
    }
}

fn library_present(name: &str) -> bool {
    for dir in candidate_dirs() {
        if dir.join(name).is_file() {
            return true;
        }
    }
    false
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("USTT_LIB_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        dirs.push(parent.to_path_buf());
    }
    dirs.push(PathBuf::from("."));
    dirs.push(PathBuf::from("lib"));
    if let Ok(cwd) = std::env::current_dir() {
        let mut cursor = Some(cwd.as_path());
        for _ in 0..5 {
            let Some(dir) = cursor else { break };
            dirs.push(dir.join("lib"));
            cursor = dir.parent();
        }
    }
    dirs
}

fn cuda_driver_present() -> bool {
    let candidates: &[&str] = if cfg!(windows) {
        &["nvcuda.dll"]
    } else {
        &["libcuda.so.1", "libcuda.so"]
    };
    candidates
        .iter()
        .any(|name| unsafe { libloading::Library::new(*name) }.is_ok())
}

fn vulkan_probe() -> Option<(bool, Option<String>)> {
    unsafe {
        let entry = ash::Entry::load().ok()?;
        let app = ash::vk::ApplicationInfo::default()
            .application_name(c"Universal STT")
            .api_version(ash::vk::make_api_version(0, 1, 1, 0));
        let create = ash::vk::InstanceCreateInfo::default().application_info(&app);
        let instance = entry.create_instance(&create, None).ok()?;
        let devices = instance.enumerate_physical_devices().ok()?;
        let mut name = None;
        let mut usable = false;
        for device in devices {
            let props = instance.get_physical_device_properties(device);
            let families = instance.get_physical_device_queue_family_properties(device);
            let has_graphics = families
                .iter()
                .any(|f| f.queue_flags.contains(ash::vk::QueueFlags::GRAPHICS));
            let has_compute = families
                .iter()
                .any(|f| f.queue_flags.contains(ash::vk::QueueFlags::COMPUTE));
            if has_graphics && has_compute {
                usable = true;
                if name.is_none() {
                    name = props
                        .device_name_as_c_str()
                        .ok()
                        .map(|c| c.to_string_lossy().into_owned());
                }
            }
        }
        instance.destroy_instance(None);
        Some((usable, name))
    }
}

/// Where the parakeet shared library would live for `backend` on this platform.
pub fn parakeet_lib_path(backend: Backend, explicit: Option<&Path>) -> Option<PathBuf> {
    let _ = backend;
    #[cfg(windows)]
    let platform: &[&str] = &[parakeet_lib(backend)];
    #[cfg(not(windows))]
    let platform: &[&str] = &[
        "libparakeet.so",
        "parakeet_cpu.so",
        "parakeet_vulkan.so",
        "parakeet_cuda.so",
    ];
    if let Some(dir) = explicit {
        for name in platform {
            let path = dir.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    for dir in candidate_dirs() {
        for name in platform {
            let path = dir.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}
