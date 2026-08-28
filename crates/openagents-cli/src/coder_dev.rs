//! Start the local Rust coder API for `--dev`.
//!
//! Phoenix stays production. `--dev` talks to `openagents-coder-api` on this
//! machine, using the same provider env names Phoenix reads
//! (`AI_GATEWAY_API_KEY`, `OPENROUTER_API_KEY`).

use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::sleep;

/// Preferred bind when nothing else is on 4000. If Phoenix already owns
/// 4000, the API binds 4010 instead of fighting it.
const PREFERRED: u16 = 4000;
const FALLBACK: u16 = 4010;

pub struct DevApi {
    pub origin: String,
}

impl DevApi {
    pub fn api_v1(&self) -> String {
        format!("{}/api/v1", self.origin)
    }
}

/// Ensure a local coder API is listening. Starts one if needed.
pub async fn ensure_running() -> Result<DevApi, Box<dyn std::error::Error>> {
    if let Some(origin) = already_ours(PREFERRED).await {
        eprintln!("coder-api already running at {origin}");
        return Ok(DevApi { origin });
    }
    let port = if port_occupied(PREFERRED).await {
        if let Some(origin) = already_ours(FALLBACK).await {
            eprintln!("coder-api already running at {origin}");
            return Ok(DevApi { origin });
        }
        eprintln!(
            "port {PREFERRED} is in use (Phoenix or another server); binding coder-api to {FALLBACK}"
        );
        FALLBACK
    } else {
        PREFERRED
    };
    start(port).await
}

async fn start(port: u16) -> Result<DevApi, Box<dyn std::error::Error>> {
    let bind = format!("127.0.0.1:{port}");
    let origin = format!("http://{bind}");
    let bin = api_bin()?;
    let log_path = log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    eprintln!("starting coder-api from {}", bin.display());
    eprintln!("waiting for {origin}/health");
    eprintln!("server log: {}", log_path.display());
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let mut command = Command::new(&bin);
    command
        .arg("--bind")
        .arg(&bind)
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    let _child = command.spawn()?;
    for attempt in 1..=80 {
        if already_ours(port).await.is_some() {
            eprintln!("coder-api ready at {origin}");
            return Ok(DevApi { origin });
        }
        if attempt % 10 == 0 {
            eprintln!("still waiting for coder-api ({} seconds)", attempt / 2);
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(format!(
        "coder-api did not become ready at {origin}/health. \
         Build it with `cargo build -p openagents-coder-api`, or set \
         OPENAGENTS_CODER_API_BIN. Provider keys: AI_GATEWAY_API_KEY \
         (Phoenix's Vercel gateway name) and OPENROUTER_API_KEY."
    )
    .into())
}

fn api_bin() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = env::var("OPENAGENTS_CODER_API_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("OPENAGENTS_CODER_API_BIN is not a file: {}", path.display()).into());
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("openagents-coder-api");
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let debug = workspace.join("target/debug/openagents-coder-api");
    if debug.is_file() {
        return Ok(debug);
    }
    Err(
        "openagents-coder-api is not built. Run `cargo build -p openagents-coder-api` \
         from the workspace, or set OPENAGENTS_CODER_API_BIN."
            .into(),
    )
}

fn log_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".openagents")
        .join("coder-api")
        .join("dev.log")
}

async fn already_ours(port: u16) -> Option<String> {
    let origin = format!("http://127.0.0.1:{port}");
    match probe(port).await {
        Probe::Ours => Some(origin),
        _ => None,
    }
}

async fn port_occupied(port: u16) -> bool {
    !matches!(probe(port).await, Probe::Unreachable)
}

#[derive(Debug)]
enum Probe {
    Ours,
    Other,
    Unreachable,
}

async fn probe(port: u16) -> Probe {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await else {
        return Probe::Unreachable;
    };
    let request =
        format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).await.is_err() {
        return Probe::Unreachable;
    }
    let mut buf = [0u8; 1024];
    let Ok(n) = stream.read(&mut buf).await else {
        return Probe::Unreachable;
    };
    let head = std::str::from_utf8(&buf[..n]).unwrap_or("");
    if head.contains("openagents-coder-api") {
        Probe::Ours
    } else if head.starts_with("HTTP/") {
        Probe::Other
    } else {
        Probe::Unreachable
    }
}
