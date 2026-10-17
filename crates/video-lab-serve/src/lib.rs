//! Video Lab's loopback service: the 1.0 routes on 127.0.0.1:8792 that the
//! local web chat (`full-local.sh --lab`), Coder 1.0.0 and `openagents lab`
//! call, over the filesystem store under `VIDEO_LAB_ROOT`.
//!
//! This service answers the store's reads itself: `/`, `/player`,
//! `/healthz`, `/api/tools`, `/api/docs`, and `/media/*` with range reads and
//! the no-training headers. The tools, renders, notes and the timeline
//! (`/api/doc`, `/api/card`, `/api/timeline`, `/api/timeline/asset`,
//! `/api/note`, `/api/tool/{name}`) still run in `scripts/film/lab.py`,
//! which this service starts as its engine on a private loopback port and
//! forwards those requests to unchanged. `lab.py serve` starts this service
//! when it is built, so every client keeps its address.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

/// The service's settings.
#[derive(Clone, Debug)]
pub struct Service {
    /// The store (`VIDEO_LAB_ROOT`, default `~/work/film-out/lab`).
    pub root: PathBuf,
    /// The project folder under the root (`VIDEO_LAB_PROJECT`, default `films`).
    pub project: String,
    /// The player page (`scripts/film/lab_player.html`).
    pub player: PathBuf,
    /// The engine's address (`127.0.0.1:PORT`), when it runs.
    pub engine: Option<String>,
}

/// What a reply sends.
pub enum Body {
    Bytes(Vec<u8>),
    /// `len` bytes of an open file from where it is positioned.
    File(File, u64),
}

/// A reply: status, headers and body.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Reply {
    fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("Access-Control-Allow-Origin".into(), "*".into()),
                ("Cache-Control".into(), "no-store".into()),
            ],
            body: Body::Bytes(value.to_string().into_bytes()),
        }
    }

    fn status(status: u16) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), "text/plain; charset=utf-8".into())],
            body: Body::Bytes(Vec::new()),
        }
    }

    /// The body as bytes (a file body is read to its end).
    pub fn bytes(self) -> Vec<u8> {
        match self.body {
            Body::Bytes(b) => b,
            Body::File(f, len) => {
                let mut out = Vec::new();
                let _ = f.take(len).read_to_end(&mut out);
                out
            }
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A request, as the service needs it.
pub struct Request<'a> {
    pub method: &'a str,
    /// The path with its query, as sent.
    pub url: &'a str,
    pub range: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub body: &'a [u8],
}

/// `%xx` escapes decoded, as Python's `unquote`.
fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn media_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("json") => "application/json",
        Some("wav") => "audio/x-wav",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("flac") => "audio/flac",
        Some("html") => "text/html",
        Some("txt" | "md") => "text/plain",
        _ => "application/octet-stream",
    }
}

impl Service {
    /// Answers one request.
    pub fn handle(&self, rq: &Request<'_>) -> Reply {
        let path = rq.url.split('?').next().unwrap_or_default();
        match (rq.method, path) {
            ("OPTIONS", _) => Reply {
                status: 204,
                headers: vec![
                    ("Access-Control-Allow-Origin".into(), "*".into()),
                    ("Access-Control-Allow-Methods".into(), "GET, POST".into()),
                    ("Access-Control-Allow-Headers".into(), "Content-Type".into()),
                ],
                body: Body::Bytes(Vec::new()),
            },
            ("GET", "/" | "/player") => match std::fs::read(&self.player) {
                Ok(page) => Reply {
                    status: 200,
                    headers: vec![("Content-Type".into(), "text/html; charset=utf-8".into())],
                    body: Body::Bytes(page),
                },
                Err(_) => Reply::status(404),
            },
            ("GET", "/healthz") => Reply::json(200, &json!({"ok": true})),
            ("GET", "/api/tools") => {
                let tools: Value = serde_json::from_str(video_lab::TOOLS).unwrap_or(json!([]));
                Reply::json(200, &json!({ "tools": tools }))
            }
            ("GET", "/api/docs") => Reply::json(200, &json!({"documents": self.documents()})),
            ("GET", p) if p.starts_with("/media/") => self.media(&p["/media/".len()..], rq.range),
            ("GET" | "POST", _) => self.forward(rq),
            _ => Reply::status(405),
        }
    }

    /// Each document's `doc.json` with its number of versions, by folder name.
    pub fn documents(&self) -> Vec<Value> {
        let base = self.root.join(&self.project);
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&base)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        dirs.sort();
        dirs.into_iter()
            .filter_map(|d| {
                let mut meta: Value =
                    serde_json::from_slice(&std::fs::read(d.join("doc.json")).ok()?).ok()?;
                let versions = std::fs::read_dir(&d)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        name.strip_prefix('v')
                            .is_some_and(|n| n.parse::<u64>().is_ok())
                            && e.path().join("version.json").is_file()
                    })
                    .count();
                meta["versions"] = json!(versions);
                Some(meta)
            })
            .collect()
    }

    /// A file under the store, with range reads (seeking in the player).
    fn media(&self, rel: &str, range: Option<&str>) -> Reply {
        let Ok(root) = self.root.canonicalize() else {
            return Reply::status(404);
        };
        let Ok(path) = root.join(unquote(rel)).canonicalize() else {
            return Reply::status(404);
        };
        if !path.starts_with(&root) || path == root || !path.is_file() {
            return Reply::status(404);
        }
        let Ok(mut file) = File::open(&path) else {
            return Reply::status(404);
        };
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let (mut start, mut end) = (0u64, size.saturating_sub(1));
        if let Some(spec) = range.and_then(|r| r.trim().strip_prefix("bytes=")) {
            let (a, b) = spec.split_once('-').unwrap_or((spec, ""));
            match (a.trim().parse::<u64>(), b.trim().parse::<u64>()) {
                (Ok(a), Ok(b)) => (start, end) = (a, b),
                (Ok(a), Err(_)) => start = a,
                (Err(_), Ok(b)) => start = size.saturating_sub(b),
                _ => {}
            }
            end = end.min(size.saturating_sub(1));
        }
        if size == 0 || start > end {
            return Reply {
                status: 416,
                headers: vec![("Content-Range".into(), format!("bytes */{size}"))],
                body: Body::Bytes(Vec::new()),
            };
        }
        if file.seek(SeekFrom::Start(start)).is_err() {
            return Reply::status(404);
        }
        let mut headers = vec![
            ("Content-Type".into(), media_type(&path).into()),
            ("Accept-Ranges".into(), "bytes".into()),
            ("Access-Control-Allow-Origin".into(), "*".into()),
            ("Cache-Control".into(), "private, max-age=3600".into()),
        ];
        let jpg = path.extension().is_some_and(|e| e == "jpg");
        let marked = path.with_extension("no-training").exists()
            || (jpg
                && path.parent().is_some_and(|dir| {
                    std::fs::read_dir(dir)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .any(|e| e.path().extension().is_some_and(|x| x == "no-training"))
                }));
        if marked {
            headers.push(("X-Robots-Tag".into(), "noai, noimageai".into()));
            headers.push(("X-OpenAgents-No-Training".into(), "1".into()));
        }
        if range.is_some() {
            headers.push((
                "Content-Range".into(),
                format!("bytes {start}-{end}/{size}"),
            ));
        }
        Reply {
            status: if range.is_some() { 206 } else { 200 },
            headers,
            body: Body::File(file, end - start + 1),
        }
    }

    /// Sends the request to the engine and returns its reply as it came.
    fn forward(&self, rq: &Request<'_>) -> Reply {
        let Some(engine) = &self.engine else {
            return Reply::json(
                503,
                &json!({"error": "Video Lab is starting. Try again in a moment."}),
            );
        };
        match forward(engine, rq) {
            Ok(reply) => reply,
            Err(_) => Reply::json(
                502,
                &json!({"error": "Video Lab stopped. Start it again with scripts/film/lab.py serve."}),
            ),
        }
    }
}

/// One HTTP/1.0 exchange with the engine at `addr`.
///
/// # Errors
/// The engine can't be reached or its reply doesn't read.
pub fn forward(addr: &str, rq: &Request<'_>) -> std::io::Result<Reply> {
    let mut stream = TcpStream::connect(addr)?;
    // A render the tool waits for can take minutes (lab.py's own limit is 600 s).
    stream.set_read_timeout(Some(Duration::from_secs(900)))?;
    let mut head = format!(
        "{} {} HTTP/1.0\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        rq.method,
        rq.url,
        rq.body.len()
    );
    if let Some(ct) = rq.content_type {
        head += &format!("Content-Type: {ct}\r\n");
    }
    if let Some(r) = rq.range {
        head += &format!("Range: {r}\r\n");
    }
    head += "\r\n";
    stream.write_all(head.as_bytes())?;
    stream.write_all(rq.body)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "not an HTTP reply");
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(bad)?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(bad)?;
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(k, _)| {
            ![
                "content-length",
                "connection",
                "server",
                "date",
                "transfer-encoding",
            ]
            .contains(&k.to_ascii_lowercase().as_str())
        })
        .collect();
    Ok(Reply {
        status,
        headers,
        body: Body::Bytes(raw[split + 4..].to_vec()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    struct Store(PathBuf);

    impl Drop for Store {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store(name: &str) -> (Store, Service) {
        let dir =
            std::env::temp_dir().join(format!("video-lab-serve-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let doc = dir.join("films/episode-1");
        std::fs::create_dir_all(doc.join("v1")).unwrap();
        std::fs::create_dir_all(doc.join("v2")).unwrap();
        std::fs::create_dir_all(doc.join("renders")).unwrap();
        std::fs::write(
            doc.join("doc.json"),
            r#"{"id": "episode-1", "kind": "cut.v1", "title": "Episode 1", "created": 1.5}"#,
        )
        .unwrap();
        std::fs::write(doc.join("v1/version.json"), "{}").unwrap();
        std::fs::write(doc.join("v2/version.json"), "{}").unwrap();
        std::fs::write(doc.join("renders/abc-preview.mp4"), b"0123456789").unwrap();
        std::fs::write(
            doc.join("renders/abc-preview.no-training"),
            "Not for training.\n",
        )
        .unwrap();
        std::fs::write(dir.join("player.html"), "<html>player</html>").unwrap();
        std::fs::write(dir.join("secret.txt"), "outside the films").unwrap();
        let service = Service {
            root: dir.join("films").parent().unwrap().to_path_buf(),
            project: "films".into(),
            player: dir.join("player.html"),
            engine: None,
        };
        (Store(dir), service)
    }

    fn get<'a>(url: &'a str, range: Option<&'a str>) -> Request<'a> {
        Request {
            method: "GET",
            url,
            range,
            content_type: None,
            body: b"",
        }
    }

    #[test]
    fn the_store_reads_are_answered_here() {
        let (_s, service) = store("reads");
        let r = service.handle(&get("/healthz", None));
        assert_eq!(r.status, 200);
        assert_eq!(r.bytes(), br#"{"ok":true}"#);
        let r = service.handle(&get("/api/docs", None));
        let docs: Value = serde_json::from_slice(&r.bytes()).unwrap();
        assert_eq!(docs["documents"][0]["id"], "episode-1");
        assert_eq!(docs["documents"][0]["versions"], 2);
        // The document's own keys keep their order.
        let keys: Vec<&String> = docs["documents"][0].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["id", "kind", "title", "created", "versions"]);
        let tools: Value =
            serde_json::from_slice(&service.handle(&get("/api/tools", None)).bytes()).unwrap();
        assert!(
            tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "lab_open")
        );
        let page = service.handle(&get("/player?doc=episode-1&v=2", None));
        assert_eq!(
            page.header("content-type"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(page.bytes(), b"<html>player</html>");
        assert_eq!(
            service
                .handle(&Request {
                    method: "OPTIONS",
                    ..get("/api/tool/lab_open", None)
                })
                .status,
            204
        );
    }

    #[test]
    fn media_seeks_and_says_it_is_not_for_training() {
        let (_s, service) = store("media");
        let url = "/media/films/episode-1/renders/abc-preview.mp4";
        let whole = service.handle(&get(url, None));
        assert_eq!(whole.status, 200);
        assert_eq!(whole.header("content-type"), Some("video/mp4"));
        assert_eq!(whole.header("x-openagents-no-training"), Some("1"));
        assert_eq!(whole.bytes(), b"0123456789");
        let part = service.handle(&get(url, Some("bytes=2-5")));
        assert_eq!(part.status, 206);
        assert_eq!(part.header("content-range"), Some("bytes 2-5/10"));
        assert_eq!(part.bytes(), b"2345");
        let tail = service.handle(&get(url, Some("bytes=-3")));
        assert_eq!(tail.bytes(), b"789");
        let open = service.handle(&get(url, Some("bytes=8-")));
        assert_eq!(open.bytes(), b"89");
        // Escaped names, and nothing outside the store.
        let escaped = service.handle(&get(
            "/media/films/episode%2D1/renders/abc-preview.mp4",
            None,
        ));
        assert_eq!(escaped.status, 200);
        assert_eq!(
            service.handle(&get("/media/../secret.txt", None)).status,
            404
        );
        assert_eq!(
            service
                .handle(&get("/media/films/%2E%2E/%2E%2E/etc/passwd", None))
                .status,
            404
        );
        assert_eq!(
            service
                .handle(&get("/media/films/missing.mp4", None))
                .status,
            404
        );
    }

    #[test]
    fn tools_go_to_the_engine_and_come_back_unchanged() {
        let (_s, mut service) = store("forward");
        let r = service.handle(&Request {
            method: "POST",
            url: "/api/tool/lab_open",
            range: None,
            content_type: Some("application/json"),
            body: br#"{"document": "episode-1"}"#,
        });
        assert_eq!(r.status, 503);
        // A stand-in engine that echoes the request line and body.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let engine = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            while !got.windows(4).any(|w| w == b"\r\n\r\n") || !got.ends_with(b"}") {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            let text = String::from_utf8_lossy(&got).into_owned();
            let body = json!({"line": text.lines().next(), "body": text.rsplit("\r\n\r\n").next()})
                .to_string();
            write!(
                s,
                "HTTP/1.0 200 OK\r\nServer: VideoLab/1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        service.engine = Some(addr);
        let r = service.handle(&Request {
            method: "POST",
            url: "/api/tool/lab_open?x=1",
            range: None,
            content_type: Some("application/json"),
            body: br#"{"document": "episode-1"}"#,
        });
        engine.join().unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.header("access-control-allow-origin"), Some("*"));
        assert_eq!(r.header("server"), None);
        let echoed: Value = serde_json::from_slice(&r.bytes()).unwrap();
        assert_eq!(echoed["line"], "POST /api/tool/lab_open?x=1 HTTP/1.0");
        assert_eq!(echoed["body"], r#"{"document": "episode-1"}"#);
    }
}
