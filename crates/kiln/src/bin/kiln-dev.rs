//! `kiln-dev [--port N] [--dir DIR] [--runner KEY]`: serve the Kiln page and
//! its assets on `127.0.0.1` with the cross-origin isolation headers (COOP
//! and COEP). `/config.json` names the `kiln-runner` key the page sends
//! remote `cargo` runs to, and the NIP-34 repository address patches go to.
//!
//! Standard library only. `scripts/build-kiln.sh` builds `DIR`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    serve::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod serve {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Component, Path, PathBuf};

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

    fn respond(stream: &mut TcpStream, root: &Path, config: &str) -> std::io::Result<()> {
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
        let mut file = PathBuf::from(root);
        let relative = Path::new(path.trim_start_matches('/'));
        let safe = relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if safe {
            file.push(relative);
        }
        if file.is_dir() {
            file.push("index.html");
        }
        let read = if path == "/config.json" {
            Ok(config.as_bytes().to_vec())
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
        eprintln!("{method} {path} {status}");
        Ok(())
    }

    pub fn main() {
        let mut port = 8787_u16;
        let mut dir = std::env::var("KILN_WEB").unwrap_or_else(|_| "dist".into());
        let mut runner = String::new();
        let mut repo = String::new();
        let mut relay = "wss://relay.openagents.com".to_string();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
                "--dir" => dir = args.next().unwrap_or(dir),
                "--runner" => runner = args.next().unwrap_or_default(),
                "--repo" => repo = args.next().unwrap_or_default(),
                "--relay" => relay = args.next().unwrap_or(relay),
                _ => {
                    eprintln!(
                        "usage: kiln-dev [--port N] [--dir DIR] [--runner KEY] [--repo 30617:KEY:ID]"
                    );
                    std::process::exit(2);
                }
            }
        }
        let root = PathBuf::from(&dir);
        if !root.join("index.html").is_file() {
            eprintln!("kiln-dev: {dir} has no index.html; run scripts/build-kiln.sh first");
            std::process::exit(1);
        }
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| {
            eprintln!("kiln-dev: 127.0.0.1:{port}: {e}");
            std::process::exit(1);
        });
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        println!("Kiln is at http://127.0.0.1:{port}/ (serving {dir})");
        // The page reads the runner's key from here; it is public.
        let config = format!(
            "{{\"runner\": \"{}\", \"repo\": \"{}\", \"relay\": \"{}\"}}",
            runner.replace('"', ""),
            repo.replace('"', ""),
            relay.replace('"', "")
        );
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            let config = config.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let _ = respond(&mut stream, &root, &config);
            });
        }
    }
}
