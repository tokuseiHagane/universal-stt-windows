use crate::model::{MediaKind, Project, SourceRef};
use crate::paths::write_atomic;
use chrono::Utc;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("project schema {0} is not supported")]
    Schema(u32),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid project json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct ProjectStore {
    dir: PathBuf,
}

impl ProjectStore {
    pub fn create(
        dir: impl AsRef<Path>,
        title: &str,
        source: SourceRef,
        kind: MediaKind,
        duration: f64,
    ) -> Result<(Self, Project), StoreError> {
        let store = Self {
            dir: dir.as_ref().to_path_buf(),
        };
        std::fs::create_dir_all(store.media_dir())?;
        let mut project = Project::new(title, source, kind, duration);
        store.save(&mut project)?;
        Ok((store, project))
    }

    pub fn open(dir: impl AsRef<Path>) -> Result<(Self, Project), StoreError> {
        let store = Self {
            dir: dir.as_ref().to_path_buf(),
        };
        let text = std::fs::read_to_string(store.project_file())?;
        let project: Project = serde_json::from_str(&text)?;
        if project.schema != Project::SCHEMA {
            return Err(StoreError::Schema(project.schema));
        }
        Ok((store, project))
    }

    pub fn save(&self, project: &mut Project) -> Result<(), StoreError> {
        project.modified_at = Utc::now();
        let bytes = serde_json::to_vec_pretty(project)?;
        std::fs::create_dir_all(&self.dir)?;
        write_atomic(&self.project_file(), &bytes)?;
        Ok(())
    }

    pub fn media_dir(&self) -> PathBuf {
        self.dir.join("media")
    }

    pub fn project_file(&self) -> PathBuf {
        self.dir.join("project.json")
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Lowercases the title and collapses every run of non-alphanumeric characters
/// into a single dash. Unicode letters are kept, so Cyrillic survives.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in title.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("project");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MediaKind, SourceRef};
    use std::path::PathBuf;

    fn sample_source() -> SourceRef {
        SourceRef {
            path: PathBuf::from("clip.wav"),
            size_bytes: 10,
            modified_unix: 0,
            fingerprint: "abc".into(),
        }
    }

    #[test]
    fn create_save_open_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("talk.ustt");
        let (store, mut p) =
            ProjectStore::create(&dir, "Talk", sample_source(), MediaKind::Audio, 3.0).unwrap();
        assert!(store.media_dir().is_dir());
        p.title = "Renamed".into();
        store.save(&mut p).unwrap();
        let (_, back) = ProjectStore::open(&dir).unwrap();
        assert_eq!(back.title, "Renamed");
        assert!(back.modified_at >= back.created_at);
    }

    #[test]
    fn save_is_atomic_and_leaves_no_tmp() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("a.ustt");
        let (store, mut p) =
            ProjectStore::create(&dir, "A", sample_source(), MediaKind::Audio, 1.0).unwrap();
        store.save(&mut p).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn open_rejects_wrong_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bad.ustt");
        let (store, mut p) =
            ProjectStore::create(&dir, "Bad", sample_source(), MediaKind::Audio, 1.0).unwrap();
        store.save(&mut p).unwrap();
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("project.json")).unwrap())
                .unwrap();
        v["schema"] = 99.into();
        std::fs::write(dir.join("project.json"), v.to_string()).unwrap();
        assert!(matches!(
            ProjectStore::open(&dir),
            Err(StoreError::Schema(99))
        ));
    }

    #[test]
    fn slugify_keeps_cyrillic_and_strips_separators() {
        assert_eq!(slugify("  Интервью: Иван / Петя  "), "интервью-иван-петя");
    }
}
