#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use ustt_app::service::media::{MediaServer, shared};

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn request(port: u16, method: &str, range: Option<&str>, body_limit: usize) -> HttpResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut request =
        format!("GET /media/sample HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    request.replace_range(..3, method);
    if let Some(range) = range {
        request.push_str(&format!("Range: {range}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').unwrap();
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    let length = headers
        .get("content-length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let mut body = Vec::new();
    reader
        .take(length.min(body_limit as u64))
        .read_to_end(&mut body)
        .unwrap();
    HttpResponse {
        status,
        headers,
        body,
    }
}

fn fixture() -> (tempfile::TempDir, MediaServer, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let bytes: Vec<u8> = (0..1_100_000).map(|i| (i % 251) as u8).collect();
    let path = dir.path().join("large.webm");
    std::fs::write(&path, &bytes).unwrap();
    {
        let mut registry = shared().lock().unwrap();
        registry.clear();
        assert!(registry.insert_with_root("sample", path, dir.path().to_path_buf()));
    }
    let server = MediaServer::start().unwrap();
    (dir, server, bytes)
}

#[test]
fn bounded_range_streams_exact_bytes() {
    let _lock = TEST_LOCK.lock().unwrap();
    let (_dir, server, bytes) = fixture();
    let response = request(server.port(), "GET", Some("bytes=100-199"), 200);
    assert_eq!(response.status, 206);
    assert_eq!(response.body, bytes[100..200]);
    assert_eq!(
        response.headers["content-range"],
        format!("bytes 100-199/{}", bytes.len())
    );
    assert_eq!(response.headers["accept-ranges"], "bytes");
    assert!(!response.headers.contains_key("access-control-allow-origin"));
}

#[test]
fn open_range_has_length_and_can_be_read_incrementally() {
    let _lock = TEST_LOCK.lock().unwrap();
    let (_dir, server, bytes) = fixture();
    let response = request(server.port(), "GET", Some("bytes=0-"), 128);
    assert_eq!(response.status, 206);
    assert_eq!(response.headers["content-length"], bytes.len().to_string());
    assert_eq!(response.body, bytes[..128]);
}

#[test]
fn suffix_head_missing_and_unsatisfiable_ranges_are_handled() {
    let _lock = TEST_LOCK.lock().unwrap();
    let (_dir, server, bytes) = fixture();
    let suffix = request(server.port(), "GET", Some("bytes=-128"), 256);
    assert_eq!(suffix.status, 206);
    assert_eq!(suffix.body, bytes[bytes.len() - 128..]);

    let head = request(server.port(), "HEAD", None, 16);
    assert_eq!(head.status, 200);
    assert_eq!(head.headers["content-length"], bytes.len().to_string());
    assert!(head.body.is_empty());

    let full = request(server.port(), "GET", None, 16);
    assert_eq!(full.status, 200);
    drop(full);

    let eof = request(
        server.port(),
        "GET",
        Some(&format!("bytes={}-", bytes.len())),
        16,
    );
    assert_eq!(eof.status, 416);
    assert_eq!(
        eof.headers["content-range"],
        format!("bytes */{}", bytes.len())
    );
}

#[test]
fn missing_token_is_404() {
    let _lock = TEST_LOCK.lock().unwrap();
    let (_dir, server, _bytes) = fixture();
    let mut stream = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
    stream
        .write_all(b"GET /media/missing HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).unwrap();
    assert!(status.contains("404"));
}
