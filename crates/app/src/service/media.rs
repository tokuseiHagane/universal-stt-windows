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

#[cfg(not(target_arch = "wasm32"))]
pub fn respond(
    uri: &str,
    method: &str,
    range: Option<&str>,
) -> dioxus::desktop::wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    use dioxus::desktop::wry::http::{Response, StatusCode};
    use std::borrow::Cow;

    let empty = |status: StatusCode| {
        Response::builder()
            .status(status)
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Borrowed(&b""[..]))
            .unwrap()
    };

    let Some((path, mime)) = shared().lock().unwrap().resolve(uri) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let total = bytes.len();
    let (status, start, end) = match range.and_then(|value| parse_range(value, total)) {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
        None => (StatusCode::OK, 0, total.saturating_sub(1)),
    };
    let body = if total == 0 {
        Vec::new()
    } else {
        bytes[start..=end].to_vec()
    };
    let mut builder = Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", "*")
        .header("Content-Length", body.len().to_string());
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header("Content-Range", format!("bytes {start}-{end}/{total}"));
    }
    if method.eq_ignore_ascii_case("HEAD") {
        return builder.body(Cow::Borrowed(&b""[..])).unwrap();
    }
    builder.body(Cow::Owned(body)).unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_range(value: &str, total: usize) -> Option<(usize, usize)> {
    if total == 0 {
        return None;
    }
    let spec = value.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let (start, end) = if start.is_empty() {
        let suffix: usize = end.parse().ok()?;
        (total.saturating_sub(suffix), total - 1)
    } else {
        let start: usize = start.parse().ok()?;
        let end = if end.is_empty() {
            total - 1
        } else {
            end.parse::<usize>().ok()?.min(total - 1)
        };
        (start, end)
    };
    (start <= end && start < total).then_some((start, end))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn media_registry_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = MediaRegistry::new();
        let inside = dir.path().join("a.wav");
        std::fs::write(&inside, b"x").unwrap();
        assert!(registry.insert_with_root("audio", inside, dir.path().to_path_buf()));
        assert!(registry.resolve("ustt://media/audio").is_some());
        assert!(!registry.insert_with_root(
            "evil",
            dir.path().join("../secret"),
            dir.path().to_path_buf()
        ));
        assert!(registry.resolve("ustt://media/evil").is_none());
    }

    #[test]
    fn range_request_returns_206() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("range.bin");
        std::fs::write(&file, (0u8..100).collect::<Vec<_>>()).unwrap();
        {
            let mut registry = shared().lock().unwrap();
            registry.clear();
            assert!(registry.insert_with_root("range", file, dir.path().to_path_buf()));
        }
        let response = respond("ustt://media/range", "GET", Some("bytes=10-19"));
        assert_eq!(response.status(), 206);
        assert_eq!(response.body().len(), 10);
        assert_eq!(
            response.headers().get("content-range").unwrap(),
            "bytes 10-19/100"
        );
    }
}
