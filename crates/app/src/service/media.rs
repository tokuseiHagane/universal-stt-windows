use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
pub struct MediaRegistry {
    root: Option<PathBuf>,
    entries: HashMap<String, PathBuf>,
}

impl MediaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, token: &str, path: PathBuf) -> bool {
        match self.root.clone() {
            Some(root) => self.insert_with_root(token, path, root),
            None => {
                self.entries.insert(token.to_string(), path);
                true
            }
        }
    }

    pub fn insert_with_root(&mut self, token: &str, path: PathBuf, root: PathBuf) -> bool {
        let root = normalize(&root);
        let path = normalize(&path);
        let inside = path.starts_with(&root)
            && !path.components().any(|c| matches!(c, Component::ParentDir));
        if !inside {
            return false;
        }
        self.root = Some(root);
        self.entries.insert(token.to_string(), path);
        true
    }

    pub fn url(token: &str) -> String {
        format!("ustt://media/{token}")
    }

    pub fn resolve(&self, uri: &str) -> Option<(PathBuf, String)> {
        let token = uri.strip_prefix("ustt://media/")?;
        let path = self.entries.get(token)?;
        let root = self.root.as_ref()?;
        if !path.starts_with(root) {
            return None;
        }
        Some((path.clone(), mime(path)))
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

fn normalize(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn mime(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "wav" => "audio/wav",
        "m4a" | "aac" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "ogg" | "opus" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
    .to_string()
}

static REGISTRY: OnceLock<Mutex<MediaRegistry>> = OnceLock::new();

pub fn shared() -> &'static Mutex<MediaRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(MediaRegistry::new()))
}
