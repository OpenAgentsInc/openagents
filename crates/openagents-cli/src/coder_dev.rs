//! Start the local Coder inference door for `--dev`.
//!
//! Phoenix stays production. `--dev` talks to `openagents-coder-api` on this
//! machine, which serves the `flash` and `free` lanes (catalog, threads,
//! grants, proxy, credit).

use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::sleep;

/// Preferred bind. Phoenix already uses 4000 and the old `pro` listener
/// sometimes occupies 4100/4101, so we prefer 4100 and fall back to 4101.
const PREFERRED: u16 = 4100;
const FALLBACK: u16 = 4101;

/// The contract a door serves.
///
/// `openagents-coder-api` serves the thread/grant/proxy hop; `nitro` serves
/// Open Responses on `/responses` and has no threads to open. The turn has to
/// know which one it is talking to before it sends anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorSpec {
    Threads,
    OpenResponses,
}

/// The environment name that carries [`DoorSpec`] to the session that opens
/// after `--dev` resolves the door, alongside the origin it already sets.
pub const SPEC_ENV: &str = "OPENAGENTS_DOOR_SPEC";

impl DoorSpec {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Threads => "threads",
            Self::OpenResponses => "open-responses",
        }
    }
}

/// Whether the door this process talks to serves Open Responses.
pub fn door_speaks_openresponses() -> bool {
    env::var(SPEC_ENV).map(|value| value.trim() == DoorSpec::OpenResponses.as_str()) == Ok(true)
}

pub struct DevApi {
    pub origin: String,
    pub spec: DoorSpec,
}

impl DevApi {
    pub fn api_v1(&self) -> String {
        format!("{}/api/v1", self.origin)
    }
}

/// Ensure a local Coder inference door is listening. Starts one if needed.
pub async fn ensure_running() -> Result<DevApi, Box<dyn std::error::Error>> {
    if let Some(door) = already_ours(PREFERRED).await {
        eprintln!(
            "{} door already running at {}",
            door.spec.as_str(),
            door.origin
        );
        return Ok(door);
    }
    let port = if port_occupied(PREFERRED).await {
        if let Some(door) = already_ours(FALLBACK).await {
            eprintln!(
                "{} door already running at {}",
                door.spec.as_str(),
                door.origin
            );
            return Ok(door);
        }
        eprintln!("port {PREFERRED} is in use; binding coder-api to {FALLBACK}");
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
        if let Some(door) = already_ours(port).await {
            eprintln!("coder-api ready at {origin}");
            return Ok(door);
        }
        if attempt % 10 == 0 {
            eprintln!("still waiting for coder-api ({} seconds)", attempt / 2);
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(format!(
        "coder-api did not become ready at {origin}/health. \
         Build it with `cargo build -p openagents-coder-api`, or set \
         OPENAGENTS_CODER_API_BIN."
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
    if let Ok(path) = which("openagents-coder-api") {
        return Ok(path);
    }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    for rel in [
        ".openagents/bin/openagents-coder-api",
        "work/openagents/target/debug/openagents-coder-api",
        "work/openagents/target/release/openagents-coder-api",
    ] {
        let candidate = PathBuf::from(&home).join(rel);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(
        "openagents-coder-api is not built. Run `cargo build -p openagents-coder-api`, \
         or set OPENAGENTS_CODER_API_BIN."
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
        .join("coder-api")
        .join("dev.log")
}

async fn already_ours(port: u16) -> Option<DevApi> {
    let origin = format!("http://127.0.0.1:{port}");
    match probe(port).await {
        Probe::Ours(spec) => Some(DevApi { origin, spec }),
        _ => None,
    }
}

async fn port_occupied(port: u16) -> bool {
    !matches!(probe(port).await, Probe::Unreachable)
}

#[derive(Debug)]
enum Probe {
    Ours(DoorSpec),
    Other,
    Unreachable,
}

/// The contract the door at this `/health` response serves.
pub fn health_spec(head: &str) -> DoorSpec {
    if head.contains("\"spec\":\"open-responses\"") || head.contains("\"spec\": \"open-responses\"")
    {
        DoorSpec::OpenResponses
    } else {
        DoorSpec::Threads
    }
}

/// Whether a `/health` response comes from a door this CLI can drive.
///
/// The names are the doors that serve the lanes: `openagents-coder-api`,
/// the older `pro` listener, and `nitro`, which serves the Open Responses
/// contract `--dev` speaks directly. A door that answers `/health` with
/// another service name is someone else's process on the port.
pub fn health_is_ours(head: &str) -> bool {
    ["openagents-coder-api", "pro", "nitro"]
        .iter()
        .any(|service| {
            head.contains(&format!("\"service\":\"{service}\""))
                || head.contains(&format!("\"service\": \"{service}\""))
        })
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
        Probe::Ours(health_spec(head))
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
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\":\"openagents-coder-api\",\"upstream\":true}"
        ));
        assert!(health_is_ours(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\": \"openagents-coder-api\"}"
        ));
        assert!(health_is_ours(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\":\"pro\"}"
        ));
        assert!(health_is_ours(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\":\"nitro\",\"spec\":\"open-responses\"}"
        ));
        assert!(!health_is_ours(
            "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true,\"service\":\"phoenix\"}"
        ));
    }

    #[test]
    fn the_door_spec_comes_from_health() {
        assert_eq!(
            health_spec(
                "HTTP/1.1 200 OK\r\n\r\n{\"service\":\"nitro\",\"spec\":\"open-responses\"}"
            ),
            DoorSpec::OpenResponses
        );
        assert_eq!(
            health_spec("HTTP/1.1 200 OK\r\n\r\n{\"service\":\"openagents-coder-api\"}"),
            DoorSpec::Threads
        );
    }
}
