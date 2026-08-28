//! Start the local Pro inference door for `--dev`.
//!
//! Phoenix stays production. `--dev` talks to `pro` on this machine
//! (`gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`).

use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::sleep;

/// Preferred bind. Phoenix and coder-api already fight over 4000/4010.
const PREFERRED: u16 = 4100;
const FALLBACK: u16 = 4101;

pub struct DevApi {
    pub origin: String,
}

impl DevApi {
    pub fn api_v1(&self) -> String {
        format!("{}/api/v1", self.origin)
    }
}

/// Ensure a local Pro door is listening. Starts one if needed.
pub async fn ensure_running() -> Result<DevApi, Box<dyn std::error::Error>> {
    if let Some(origin) = already_ours(PREFERRED).await {
        eprintln!("pro already running at {origin}");
        return Ok(DevApi { origin });
    }
    let port = if port_occupied(PREFERRED).await {
        if let Some(origin) = already_ours(FALLBACK).await {
            eprintln!("pro already running at {origin}");
            return Ok(DevApi { origin });
        }
        eprintln!("port {PREFERRED} is in use; binding pro to {FALLBACK}");
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
    eprintln!("starting pro from {}", bin.display());
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
            eprintln!("pro ready at {origin}");
            return Ok(DevApi { origin });
        }
        if attempt % 10 == 0 {
            eprintln!("still waiting for pro ({} seconds)", attempt / 2);
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(format!(
        "pro did not become ready at {origin}/health. \
         Build it with `cargo build -p pro-gateway` in the Pro repository, or set \
         OPENAGENTS_PRO_BIN. Upstream key: PRO_UPSTREAM_KEY."
    )
    .into())
}

fn api_bin() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = env::var("OPENAGENTS_PRO_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("OPENAGENTS_PRO_BIN is not a file: {}", path.display()).into());
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("pro");
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }
    if let Ok(path) = which("pro") {
        return Ok(path);
    }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    for rel in [
        ".openagents/bin/pro",
        "work/pro/target/debug/pro",
        "work/pro/target/release/pro",
    ] {
        let candidate = PathBuf::from(&home).join(rel);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(
        "pro is not built. Run `cargo build -p pro-gateway` in the Pro repository, \
         or set OPENAGENTS_PRO_BIN."
            .into(),
    )
}

fn which(name: &str) -> Result<PathBuf, ()> {
    let path = env::var_os("PATH").ok_or(())?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(())
}

fn log_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".openagents")
        .join("pro")
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

pub fn health_is_ours(head: &str) -> bool {
    head.contains("\"service\":\"pro\"") || head.contains("\"service\": \"pro\"")
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
    if health_is_ours(head) {
        Probe::Ours
    } else if head.starts_with("HTTP/") {
        Probe::Other
    } else {
        Probe::Unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_json_is_recognised() {
        assert!(health_is_ours(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\":\"pro\",\"upstream\":true}"
        ));
        assert!(!health_is_ours(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\":\"openagents-coder-api\"}"
        ));
    }
}
