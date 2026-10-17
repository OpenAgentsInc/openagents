//! `video-lab-serve [--port 8792] --lab PATH [--python PATH]`: Video Lab's
//! loopback service. It starts `PATH` (`scripts/film/lab.py`) as its engine
//! on a free loopback port, waits for it, then serves 127.0.0.1:PORT
//! (`video_lab_serve`). It stops when the engine stops, and the engine
//! stops when this service does. `lab.py serve` runs it when it is built.

use std::io::Read as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use video_lab_serve::{Body, Request, Service};

const USAGE: &str =
    "Usage: video-lab-serve [--port 8792] --lab scripts/film/lab.py [--python python3]";

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(p),
    }
}

fn run() -> Result<(), String> {
    let mut port: u16 = std::env::var("VIDEO_LAB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8792);
    let mut lab: Option<PathBuf> = None;
    let mut python = PathBuf::from("python3");
    let mut args = std::env::args().skip(1);
    while let Some(word) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{word} needs a value"));
        match word.as_str() {
            "--port" => port = value()?.parse().map_err(|_| "--port needs a number")?,
            "--lab" => lab = Some(PathBuf::from(value()?)),
            "--python" => python = PathBuf::from(value()?),
            _ => return Err(USAGE.into()),
        }
    }
    let lab = lab.ok_or(USAGE)?;
    let root =
        expand(&std::env::var("VIDEO_LAB_ROOT").unwrap_or_else(|_| "~/work/film-out/lab".into()));
    let project = std::env::var("VIDEO_LAB_PROJECT").unwrap_or_else(|_| "films".into());
    let player = lab.with_file_name("lab_player.html");
    let public = format!("127.0.0.1:{port}");
    // Hold the public port first, so a second copy fails before it starts an engine.
    let held = TcpListener::bind(&public)
        .map_err(|e| format!("Video Lab can't listen on {public} ({e})."))?;
    let engine_port = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map_err(|e| e.to_string())?
        .port();
    let engine = format!("127.0.0.1:{engine_port}");
    let mut child = Command::new(&python)
        .arg(&lab)
        .args(["engine", "--port", &engine_port.to_string()])
        .env("VIDEO_LAB_URL", format!("http://{public}"))
        .env("VIDEO_LAB_PORT", port.to_string())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Video Lab couldn't start its engine ({e})."))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("Video Lab's engine stopped ({status})."));
        }
        let up = video_lab_serve::forward(
            &engine,
            &Request {
                method: "GET",
                url: "/healthz",
                range: None,
                content_type: None,
                body: b"",
            },
        )
        .is_ok_and(|r| r.status == 200);
        if up {
            break;
        }
        if started.elapsed() > Duration::from_secs(600) {
            let _ = child.kill();
            return Err("Video Lab's engine didn't start.".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::spawn(move || {
        let status = child.wait();
        eprintln!("Video Lab's engine stopped ({status:?}).");
        std::process::exit(1);
    });
    let service = std::sync::Arc::new(Service {
        root,
        project,
        player,
        engine: Some(engine),
    });
    let server = tiny_http::Server::from_listener(held, None).map_err(|e| e.to_string())?;
    println!(
        "Video Lab on http://{public} (documents in {})",
        service.root.join(&service.project).display()
    );
    for mut rq in server.incoming_requests() {
        let service = service.clone();
        std::thread::spawn(move || {
            let local = rq.remote_addr().is_none_or(|a| a.ip().is_loopback());
            if !local {
                let _ = rq.respond(tiny_http::Response::empty(403));
                return;
            }
            let header = |name: &'static str| {
                rq.headers()
                    .iter()
                    .find(|h| h.field.equiv(name))
                    .map(|h| h.value.as_str().to_owned())
            };
            let (range, content_type) = (header("Range"), header("Content-Type"));
            let method = rq.method().as_str().to_owned();
            let url = rq.url().to_owned();
            let mut body = Vec::new();
            let _ = rq.as_reader().take(64 * 1024 * 1024).read_to_end(&mut body);
            let reply = service.handle(&Request {
                method: &method,
                url: &url,
                range: range.as_deref(),
                content_type: content_type.as_deref(),
                body: &body,
            });
            let headers: Vec<tiny_http::Header> = reply
                .headers
                .iter()
                .filter_map(|(k, v)| tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()).ok())
                .collect();
            let status = tiny_http::StatusCode(reply.status);
            let _ = match reply.body {
                Body::Bytes(b) => {
                    let len = b.len();
                    rq.respond(tiny_http::Response::new(
                        status,
                        headers,
                        std::io::Cursor::new(b),
                        Some(len),
                        None,
                    ))
                }
                Body::File(f, len) => rq.respond(tiny_http::Response::new(
                    status,
                    headers,
                    f.take(len),
                    Some(len as usize),
                    None,
                )),
            };
        });
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            std::process::ExitCode::FAILURE
        }
    }
}
