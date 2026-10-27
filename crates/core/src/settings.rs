use crate::model::ExportDefaults;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendPreference {
    #[default]
    Auto,
    Cpu,
    Vulkan,
    Cuda,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: Option<String>,
    pub theme: String,
    pub backend: BackendPreference,
    pub threads: u32,
    pub autosave_secs: u32,
    pub models_dir: Option<PathBuf>,
    pub ffmpeg_dir: Option<PathBuf>,
    pub export: ExportDefaults,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            theme: "system".into(),
            backend: BackendPreference::Auto,
            threads: 0,
            autosave_secs: 5,
            models_dir: None,
            ffmpeg_dir: None,
            export: ExportDefaults::default(),
        }
    }
}

impl Settings {
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::paths::write_atomic(path.as_ref(), &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let mut s = Settings::default();
        assert_eq!(s.backend, BackendPreference::Auto);
        assert_eq!(s.autosave_secs, 5);
        s.language = Some("ru".into());
        s.save(&file).unwrap();
        assert_eq!(
            Settings::load(&file).unwrap().language.as_deref(),
            Some("ru")
        );
    }
}
