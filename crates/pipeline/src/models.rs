use crate::backend::{Backend, asr_model_file};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPaths {
    pub who_dir: PathBuf,
    pub asr_model: PathBuf,
    pub silero_vad: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model file {name} is missing at {path}")]
    Missing { name: String, path: PathBuf },
}

const WHO_FILES: &[&str] = &[
    "encoder_6000.onnx",
    "encoder_30000.onnx",
    "queries_6000.onnx",
    "queries_30000.onnx",
    "voice_3256.onnx",
    "who_embed.f16",
    "who_mel_banks.f32",
    "who_voice_banks.f32",
    "who_meta.json",
];

const DEFAULT_FILES: &[&str] = &[
    "redux-packed.gguf",
    "redux-q8_0.gguf",
    "silero-vad-f16.gguf",
];

pub fn resolve_models(dir: &Path, backend: Backend) -> Result<ModelPaths, ModelError> {
    let who_dir = dir.join("who");
    for name in WHO_FILES {
        let path = who_dir.join(name);
        if !path.is_file() {
            return Err(ModelError::Missing {
                name: (*name).to_string(),
                path,
            });
        }
    }
    let asr_name = asr_model_file(backend);
    let asr_model = dir.join(asr_name);
    if !asr_model.is_file() {
        return Err(ModelError::Missing {
            name: asr_name.to_string(),
            path: asr_model,
        });
    }
    let silero_vad = dir.join("silero-vad-f16.gguf");
    if !silero_vad.is_file() {
        return Err(ModelError::Missing {
            name: "silero-vad-f16.gguf".into(),
            path: silero_vad,
        });
    }
    Ok(ModelPaths {
        who_dir,
        asr_model,
        silero_vad,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModelCheck {
    pub name: String,
    pub ok: bool,
    pub expected_sha256: Option<String>,
    pub actual_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    files: Vec<ModelFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelFile {
    pub name: String,
    pub sha256: String,
    #[serde(default)]
    pub bytes: u64,
}

pub fn verify_models(dir: &Path) -> Vec<ModelCheck> {
    let manifest_path = dir.join("manifest.json");
    let files = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => match serde_json::from_str::<Manifest>(&text) {
            Ok(manifest) => manifest.files,
            Err(e) => {
                return vec![ModelCheck {
                    name: "manifest.json".into(),
                    ok: false,
                    expected_sha256: None,
                    actual_sha256: Some(format!("invalid: {e}")),
                }];
            }
        },
        Err(_) => DEFAULT_FILES
            .iter()
            .copied()
            .chain(WHO_FILES.iter().copied())
            .map(|name| ModelFile {
                name: name.to_string(),
                sha256: String::new(),
                bytes: 0,
            })
            .collect(),
    };

    files
        .into_iter()
        .map(|file| {
            let path = dir.join(&file.name);
            let actual = sha256_file(&path).ok();
            let ok = !file.sha256.is_empty() && actual.as_deref() == Some(file.sha256.as_str());
            ModelCheck {
                name: file.name,
                ok,
                expected_sha256: (!file.sha256.is_empty()).then_some(file.sha256),
                actual_sha256: actual,
            }
        })
        .collect()
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
