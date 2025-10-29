//! Local Convex backend lifecycle helpers for the bridge.
//!
//! This module provides utilities to locate defaults (binary path, DB path),
//! probe health, start the local backend in supervised mode, and run a one‑shot
//! function deploy. It is used both by the CLI bridge and by the Tauri sidecar.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::Opts;

/// Default path where we install or expect the local Convex backend binary.
pub fn default_convex_bin() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".openagents/bin/local_backend");
    }
    PathBuf::from("local_backend")
}

/// Default sqlite DB location for the local backend when not provided.
pub fn default_convex_db() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".openagents/convex/data.sqlite3");
    }
    PathBuf::from("data.sqlite3")
}

/// Quick health probe against the local backend instance.
/// If `OPENAGENTS_CONVEX_INSTANCE` is set (or defaults to "openagents"),
/// prefer checking `/instance_name` for equality to detect early readiness.
pub async fn convex_health(url: &str) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let expected = std::env::var("OPENAGENTS_CONVEX_INSTANCE").ok();
    if let Some(inst) = expected.as_deref() {
        if let Ok(r) = client.get(format!("{}/instance_name", url)).send().await {
            if r.status().is_success() {
                if let Ok(name) = r.text().await { if name.trim() == inst { return Ok(true); } }
            }
        }
    }
    // Primary endpoint
    if let Ok(r) = client.get(format!("{}/instance_version", url)).send().await {
        if r.status().is_success() {
            return Ok(true);
        }
    }
    // Fallback endpoint used by some builds
    if let Ok(r) = client.get(format!("{}/health_check", url)).send().await {
        if r.status().is_success() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Clone, Copy)]
enum StateMode { Convex, OpenAgents, Ephemeral }

fn determine_state_mode() -> StateMode {
    match std::env::var("OPENAGENTS_CONVEX_STATE").ok().as_deref() {
        Some("convex") => StateMode::Convex,
        Some("ephemeral") => StateMode::Ephemeral,
        Some("openagents") => StateMode::OpenAgents,
        _ => StateMode::OpenAgents,
    }
}

fn resolve_paths_for_state(mode: StateMode) -> (PathBuf, PathBuf) {
    match mode {
        StateMode::OpenAgents => {
            let db = default_convex_db();
            let storage = std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".openagents/convex/storage"))
                .unwrap_or_else(|_| PathBuf::from("convex_local_storage"));
            (db, storage)
        }
        StateMode::Convex => {
            let base = std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".convex/convex-backend-state/openagents-dev"))
                .unwrap_or_else(|_| PathBuf::from("convex_state"));
            let db = base.join("convex_local_backend.sqlite3");
            let storage = base.join("convex_local_storage");
            (db, storage)
        }
        StateMode::Ephemeral => {
            let base = std::env::temp_dir().join(format!("oa-convex-{}", std::process::id()));
            // Use a stable directory per-process
            let _ = std::fs::create_dir_all(&base);
            let db = base.join("convex_ephemeral.sqlite3");
            let storage = base.join("storage");
            (db, storage)
        }
    }
}

async fn tcp_listen_probe(port: u16) -> bool {
    use tokio::time::{timeout, Duration as TokioDuration};
    let addr = format!("127.0.0.1:{}", port);
    match timeout(TokioDuration::from_millis(300), tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

/// Start (or restart) the local backend as needed and wait until healthy.
pub async fn ensure_convex_running(opts: &Opts) -> Result<()> {
    info!(port = opts.convex_port, interface = %opts.convex_interface, "convex.ensure: begin");
    let t0 = Instant::now();
    let bin = opts.convex_bin.clone().unwrap_or_else(default_convex_bin);
    if !bin.exists() {
        if let Err(e) = ensure_local_backend_present().await {
            warn!(?e, path=%bin.display(), "convex local_backend missing and auto-install failed");
        }
    }
    // Resolve state layout and paths
    let state_mode = determine_state_mode();
    let (db, storage_dir) = match opts.convex_db.clone() {
        Some(p) => {
            let storage = std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".openagents/convex/storage"))
                .unwrap_or_else(|_| PathBuf::from("convex_local_storage"));
            (p, storage)
        }
        None => resolve_paths_for_state(state_mode),
    };
    let port = opts.convex_port;
    let site_proxy_port = port.saturating_add(1);
    let interface = opts.convex_interface.clone();
    let base = format!("http://127.0.0.1:{}", port);
    let pre_healthy = convex_health(&base).await.unwrap_or(false);
    if pre_healthy {
        if opts.convex_interface.trim() != "127.0.0.1" {
            info!(url=%base, desired_interface=%opts.convex_interface, "convex healthy on loopback; restarting on desired interface");
            if let Err(e) = kill_listeners_on_port(port).await {
                warn!(
                    ?e,
                    port, "failed killing existing convex on port; will try spawn anyway"
                );
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        } else {
            info!(url=%base, "convex.ensure: already healthy");
            return Ok(());
        }
    }
    std::fs::create_dir_all(db.parent().unwrap_or_else(|| Path::new("."))).ok();
    // Validate binary presence and perms
    match std::fs::metadata(&bin) {
        Ok(meta) => {
            if !meta.is_file() {
                warn!(path=%bin.display(), "convex local_backend is not a regular file");
            }
        }
        Err(e) => {
            warn!(?e, path=%bin.display(), "convex local_backend metadata unavailable");
        }
    }

    let mut cmd = Command::new(&bin);
    cmd.arg(&db)
        .arg("--db")
        .arg("sqlite")
        .arg("--interface")
        .arg(&interface)
        .arg("--port")
        .arg(port.to_string())
        .arg("--site-proxy-port")
        .arg(site_proxy_port.to_string())
        .arg("--local-storage")
        .arg(storage_dir.display().to_string())
        .arg("--disable-beacon");
    // Provide an instance name so we can use /instance_name for early readiness
    let instance_name = std::env::var("OPENAGENTS_CONVEX_INSTANCE").unwrap_or_else(|_| "openagents".to_string());
    cmd.arg("--instance-name").arg(&instance_name);
    let debug_backend = std::env::var("OPENAGENTS_CONVEX_DEBUG").ok().as_deref() == Some("1");
    if debug_backend {
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
    } else {
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    info!(bin=%bin.display(), db=%db.display(), port, site_proxy_port, interface=%interface, "convex.ensure: starting local backend");
    let mut child = cmd.spawn().context("spawn convex local_backend")?;
    // Adaptive timeout: longer window on first-run (no marker present)
    let init_marker = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".openagents/convex/.initialized"))
        .unwrap_or_else(|| PathBuf::from(".initialized"));
    let is_first_run = !init_marker.exists();
    // Allow more time on subsequent runs as well; large DBs may take longer to become ready
    let max_iters: u32 = if is_first_run { 240 } else { 120 }; // 120s vs 60s

    let mut ok = false;
    for i in 0..max_iters {
        // If the child crashed, log status and abort early
        match child.try_wait() {
            Ok(Some(status)) => {
                warn!(?status, "convex local_backend exited prematurely");
                break;
            }
            Ok(None) => {}
            Err(e) => {
                warn!(?e, "convex local_backend try_wait failed");
            }
        }
        // TCP listener first, then HTTP readiness (prefer /instance_name when instance is set)
        if tcp_listen_probe(port).await {
            if convex_health(&base).await.unwrap_or(false) { ok = true; break; }
        }
        if i % 10 == 0 {
            info!(attempt=i+1, url=%base, first_run=is_first_run, "convex.ensure: waiting for health");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if ok {
        let ready_ms = t0.elapsed().as_millis() as u128;
        info!(url=%base, pid=?child.id(), ready_ms, "convex.ensure: healthy after start");
        // Persist initialized marker
        if let Some(parent) = init_marker.parent() { let _ = std::fs::create_dir_all(parent); }
        let _ = std::fs::write(&init_marker, b"ok");
        Ok(())
    } else {
        let _ = child.kill().await;
        error!(url=%base, "convex.ensure: failed to report healthy in time");
        anyhow::bail!("convex health probe failed")
    }
}

#[cfg(unix)]
/// Best‑effort helper to terminate any listeners on `port` (Unix only).
pub async fn kill_listeners_on_port(port: u16) -> Result<()> {
    use std::process::Command as StdCommand;
    let output = StdCommand::new("lsof")
        .args(["-i", &format!(":{}", port), "-sTCP:LISTEN", "-t"])
        .output();
    let out = match output {
        Ok(o) => o,
        Err(e) => {
            return Err(anyhow::Error::from(e).context("lsof not available to kill listeners"));
        }
    };
    if !out.status.success() {
        return Ok(());
    }
    let pids = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect::<Vec<_>>();
    for pid in pids {
        let _ = StdCommand::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    Ok(())
}

/// Idempotent “dev:once” deploy of Convex functions when the backend is healthy.
pub async fn bootstrap_convex(opts: &Opts) -> Result<()> {
    use std::process::Stdio;
    let port = opts.convex_port;
    let url = format!("http://127.0.0.1:{}", port);
    if !convex_health(&url).await.unwrap_or(false) {
        warn!(%url, "convex bootstrap skipped; backend not healthy");
        return Ok(());
    }
    let admin = std::env::var("CONVEX_ADMIN_KEY")
        .ok()
        .or_else(|| std::env::var("CONVEX_SELF_HOSTED_ADMIN_KEY").ok())
        .unwrap_or_else(|| {
            "carnitas|017c5405aba48afe1d1681528424e4528026e69e3b99e400ef23f2f3741a11db225497db09"
                .to_string()
        });
    let root = detect_repo_root(None);
    let mut cmd = std::process::Command::new("bun");
    cmd.args(["run", "convex:dev:once"])
        .current_dir(&root)
        .env("CONVEX_URL", &url)
        .env("CONVEX_SELF_HOSTED_URL", &url)
        .env("CONVEX_ADMIN_KEY", &admin)
        .env("CONVEX_SELF_HOSTED_ADMIN_KEY", &admin)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    match cmd.spawn() {
        Ok(child) => info!(pid=?child.id(), "convex dev:once spawned"),
        Err(e) => warn!(?e, "convex dev:once spawn failed"),
    }
    Ok(())
}

/// Heuristic repo root detector to run `bun run` from the correct directory.
fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &Path) -> bool {
        p.join("expo").is_dir() && p.join("crates").is_dir()
    }
    let mut cur =
        start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop {
        if is_repo_root(&cur) {
            return cur;
        }
        if !cur.pop() {
            return original;
        }
    }
}

// Attempts to run `bunx convex dev` once to install local backend if missing. Best effort.
/// Fire‑and‑forget attempt to provision the local backend via `bunx convex dev`.
async fn ensure_local_backend_present() -> Result<()> {
    use std::process::Stdio;
    let mut cmd = std::process::Command::new("bunx");
    cmd.env("CI", "1");
    cmd.args([
        "convex",
        "dev",
        "--configure",
        "--dev-deployment",
        "local",
        "--once",
        "--skip-push",
        "--local-force-upgrade",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    let _ = cmd.spawn();
    Ok(())
}
