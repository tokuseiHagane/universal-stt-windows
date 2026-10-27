use crate::model::MediaKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_RECENT: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    pub title: String,
    pub opened_at: DateTime<Utc>,
    pub duration: f64,
    pub kind: MediaKind,
}

impl RecentEntry {
    pub fn new(path: PathBuf, title: impl Into<String>, duration: f64, kind: MediaKind) -> Self {
        Self {
            path,
            title: title.into(),
            opened_at: Utc::now(),
            duration,
            kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentStore {
    #[serde(skip)]
    file: PathBuf,
    #[serde(default)]
    entries: Vec<RecentEntry>,
}

impl RecentStore {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            entries: Vec::new(),
        }
    }

    pub fn load(mut self) -> std::io::Result<Self> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => {
                let mut parsed: Self = serde_json::from_str(&text)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                parsed.file = self.file;
                self = parsed;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(self)
    }

    pub fn push(&mut self, entry: RecentEntry) {
        self.entries.retain(|e| e.path != entry.path);
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_RECENT);
        let _ = self.save();
    }

    pub fn list(&self) -> &[RecentEntry] {
        &self.entries
    }

    pub fn prune_missing(&mut self) {
        self.entries
            .retain(|e| e.path.join("project.json").is_file());
        let _ = self.save();
    }

    pub fn save(&self) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(&self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::paths::write_atomic(&self.file, &bytes)
    }

    pub fn file(&self) -> &Path {
        &self.file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_caps_at_twelve_and_prunes_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store_file = dir.path().join("recent.json");
        std::fs::create_dir_all(dir.path().join("p1.ustt")).unwrap();
        std::fs::write(dir.path().join("p1.ustt").join("project.json"), b"{}").unwrap();
        let mut r = RecentStore::new(store_file.clone());
        for i in 0..14 {
            r.push(RecentEntry::new(
                dir.path().join(format!("x{i}.ustt")),
                "x",
                1.0,
                MediaKind::Audio,
            ));
        }
        r.push(RecentEntry::new(
            dir.path().join("p1.ustt"),
            "p1",
            10.0,
            MediaKind::Audio,
        ));
        assert_eq!(r.list().len(), 12);
        r.prune_missing();
        assert_eq!(r.list().len(), 1);
        assert_eq!(RecentStore::new(store_file).load().unwrap().list().len(), 1);
    }
}
