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
        let token = uri.strip_prefix("/media/")?;
        let path = normalize(self.entries.get(token)?);
        let root = normalize(self.root.as_ref()?);
        if !path.starts_with(&root) || path.components().any(|c| matches!(c, Component::ParentDir))
        {
            return None;
        }
        Some((path.clone(), mime(&path)))
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
#[cfg(not(target_arch = "wasm32"))]
static SERVER: OnceLock<Result<MediaServer, String>> = OnceLock::new();

pub fn shared() -> &'static Mutex<MediaRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(MediaRegistry::new()))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start_shared() -> Result<&'static MediaServer, String> {
    match SERVER.get_or_init(|| MediaServer::start().map_err(|err| err.to_string())) {
        Ok(server) => Ok(server),
        Err(err) => Err(err.clone()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn shared_url(token: &str) -> Option<String> {
    let server = start_shared().ok()?;
    let path = format!("/media/{token}");
    let registry = shared().lock().ok()?;
    registry.resolve(&path).map(|_| server.url(token))
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct RangeError;

#[cfg(not(target_arch = "wasm32"))]
pub struct MediaServer {
    server: std::sync::Arc<tiny_http::Server>,
    port: u16,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    accept: Option<std::thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl MediaServer {
    pub fn start() -> std::io::Result<Self> {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let server = tiny_http::Server::http("127.0.0.1:0")
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let port = server
            .server_addr()
            .to_string()
            .parse::<std::net::SocketAddr>()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .port();
        let server = Arc::new(server);
        let running = Arc::new(AtomicBool::new(true));
        let accept = {
            let server = server.clone();
            let running = running.clone();
            std::thread::spawn(move || {
                while running.load(std::sync::atomic::Ordering::Relaxed) {
                    let Ok(request) = server.recv() else { break };
                    std::thread::spawn(move || serve_request(request));
                }
            })
        };
        Ok(Self {
            server,
            port,
            running,
            accept: Some(accept),
        })
    }

    pub fn url(&self, token: &str) -> String {
        format!("http://127.0.0.1:{}/media/{token}", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for MediaServer {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.server.unblock();
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn response_header(name: &str, value: impl AsRef<[u8]>) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_ref()).unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
fn empty_response(
    status: u16,
    total: Option<u64>,
) -> tiny_http::Response<Box<dyn std::io::Read + Send>> {
    let mut response = tiny_http::Response::new(
        tiny_http::StatusCode(status),
        Vec::new(),
        Box::new(std::io::empty()) as Box<dyn std::io::Read + Send>,
        Some(0),
        None,
    );
    if let Some(total) = total {
        response =
            response.with_header(response_header("Content-Range", format!("bytes */{total}")));
    }
    response
}

#[cfg(not(target_arch = "wasm32"))]
fn serve_request(request: tiny_http::Request) {
    use std::io::{Read, Seek, SeekFrom};

    let method = request.method().as_str().to_string();
    if method != "GET" && method != "HEAD" {
        let _ = request.respond(empty_response(405, None));
        return;
    }
    let path = request.url().split('?').next().unwrap_or("").to_string();
    let Some(_token) = path.strip_prefix("/media/") else {
        let _ = request.respond(empty_response(404, None));
        return;
    };
    let resolved = shared().lock().unwrap().resolve(&path);
    let Some((path, mime)) = resolved else {
        let _ = request.respond(empty_response(404, None));
        return;
    };
    let Ok(total) = std::fs::metadata(&path).map(|meta| meta.len()) else {
        let _ = request.respond(empty_response(404, None));
        return;
    };
    let range = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Range"))
        .map(|header| header.value.as_str().to_string());
    let (status, start, end) = match parse_range(range.as_deref(), total) {
        Ok(Some((start, end))) => (206, start, end),
        Ok(None) => (200, 0, total.saturating_sub(1)),
        Err(_) => {
            let _ = request.respond(empty_response(416, Some(total)));
            return;
        }
    };
    let length = if total == 0 { 0 } else { end - start + 1 };
    let reader: Box<dyn Read + Send> = if method == "HEAD" || length == 0 {
        Box::new(std::io::empty())
    } else {
        let Ok(mut file) = std::fs::File::open(&path) else {
            let _ = request.respond(empty_response(404, None));
            return;
        };
        if file.seek(SeekFrom::Start(start)).is_err() {
            let _ = request.respond(empty_response(404, None));
            return;
        }
        Box::new(file.take(length))
    };
    let mut response = tiny_http::Response::new(
        tiny_http::StatusCode(status),
        Vec::new(),
        reader,
        Some(usize::try_from(length).unwrap_or(usize::MAX)),
        None,
    )
    .with_chunked_threshold(usize::MAX)
    .with_header(response_header("Content-Type", mime))
    .with_header(response_header("Accept-Ranges", "bytes"))
    .with_header(response_header("Cache-Control", "no-store"));
    if status == 206 {
        response = response.with_header(response_header(
            "Content-Range",
            format!("bytes {start}-{end}/{total}"),
        ));
    }
    let _ = request.respond(response);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn parse_range(value: Option<&str>, total: u64) -> Result<Option<(u64, u64)>, RangeError> {
    let Some(value) = value else { return Ok(None) };
    if total == 0 {
        return Err(RangeError);
    }
    let spec = value.trim().strip_prefix("bytes=").ok_or(RangeError)?;
    let (start, end) = spec.split_once('-').ok_or(RangeError)?;
    let (start, end) = if start.is_empty() {
        let suffix: u64 = end.parse().map_err(|_| RangeError)?;
        if suffix == 0 {
            return Err(RangeError);
        }
        (total.saturating_sub(suffix), total - 1)
    } else {
        let start: u64 = start.parse().map_err(|_| RangeError)?;
        let end = if end.is_empty() {
            total - 1
        } else {
            end.parse::<u64>().map_err(|_| RangeError)?.min(total - 1)
        };
        (start, end)
    };
    if start > end || start >= total {
        return Err(RangeError);
    }
    Ok(Some((start, end)))
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
        assert!(registry.resolve("/media/audio").is_some());
        assert!(!registry.insert_with_root(
            "evil",
            dir.path().join("../secret"),
            dir.path().to_path_buf()
        ));
        assert!(registry.resolve("/media/evil").is_none());
    }
}
