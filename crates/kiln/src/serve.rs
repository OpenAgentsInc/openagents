//! The static server `kiln-dev` runs, and `kiln-runner` runs for its smoke
//! test: files under one directory on `127.0.0.1`, with the cross-origin
//! isolation headers (COOP and COEP), a `/config.json` of the caller's, and
//! an optional overlay that answers a path before the directory does (a
//! livehost manifest, in `kiln-dev --live-*`).
//!
//! Standard library only.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// Answers a relative path from somewhere other than the directory.
pub type Overlay = Arc<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

/// What to serve.
#[derive(Clone)]
pub struct Server {
    pub root: PathBuf,
    /// The body of `/config.json`.
    pub config: String,
    pub overlay: Option<Overlay>,
    /// Log each request to standard error.
    pub log: bool,
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, server: &Server) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" || header == "\n" {
            break;
        }
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let relative = path.trim_start_matches('/');
    let safe = Path::new(relative)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    let mut file = server.root.join(relative);
    let name = if safe && (relative.is_empty() || file.is_dir()) {
        file.push("index.html");
        if relative.is_empty() {
            "index.html".to_string()
        } else {
            format!("{}/index.html", relative.trim_end_matches('/'))
        }
    } else {
        relative.to_string()
    };
    let read = if path == "/config.json" {
        Ok(server.config.as_bytes().to_vec())
    } else if let Some(bytes) = server
        .overlay
        .as_ref()
        .filter(|_| safe)
        .and_then(|overlay| overlay(&name))
    {
        Ok(bytes)
    } else {
        std::fs::read(&file)
    };
    let (status, body, kind) = match (method, safe, read) {
        ("GET" | "HEAD", true, Ok(body)) => ("200 OK", body, content_type(&file)),
        ("GET" | "HEAD", _, _) => ("404 Not Found", b"not found\n".to_vec(), "text/plain"),
        _ => (
            "405 Method Not Allowed",
            b"method not allowed\n".to_vec(),
            "text/plain",
        ),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Cross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\n\
         Cross-Origin-Resource-Policy: same-origin\r\n\
         Cache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    if method != "HEAD" {
        stream.write_all(&body)?;
    }
    if server.log {
        eprintln!("{method} {path} {status}");
    }
    Ok(())
}

/// Serve on `listener` until the process ends, a thread per connection.
pub fn run(listener: &TcpListener, server: &Server) {
    for stream in listener.incoming().flatten() {
        let server = server.clone();
        std::thread::spawn(move || {
            let mut stream = stream;
            let _ = respond(&mut stream, &server);
        });
    }
}

/// Serve on a free port in a background thread; returns the port.
///
/// # Errors
///
/// When no port can be bound.
pub fn spawn(server: Server) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(move || run(&listener, &server));
    Ok(port)
}
