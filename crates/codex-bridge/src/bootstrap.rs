//! Local Convex backend lifecycle helpers for the bridge.
//!
//! This module provides utilities to locate defaults (binary path, DB path),
//! probe health, start the local backend in supervised mode, and run a one‑shot
//! function deploy. It is used both by the CLI bridge and by the Tauri sidecar.

use std::path::{Path, PathBuf};
use std::time::Duration;

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
pub async fn convex_health(url: &str) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let resp = client.get(format!("{}/instance_version", url)).send().await;
    Ok(matches!(resp, Ok(r) if r.status().is_success()))
}

/// Start (or restart) the local backend as needed and wait until healthy.
pub async fn ensure_convex_running(opts: &Opts) -> Result<()> {
    info!(port = opts.convex_port, interface = %opts.convex_interface, "convex.ensure: begin");
    let bin = opts.convex_bin.clone().unwrap_or_else(default_convex_bin);
    if !bin.exists() {
        if let Err(e) = ensure_local_backend_present().await {
            warn!(?e, path=%bin.display(), "convex local_backend missing and auto-install failed");
        }
    }
    let db = opts.convex_db.clone().unwrap_or_else(default_convex_db);
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
        .arg(
            std::env::var("HOME")
                .map(|h| format!("{}/.openagents/convex/storage", h))
                .unwrap_or_else(|_| "convex_local_storage".to_string()),
        )
        .arg("--disable-beacon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    info!(bin=%bin.display(), db=%db.display(), port, site_proxy_port, interface=%interface, "convex.ensure: starting local backend");
    let mut child = cmd.spawn().context("spawn convex local_backend")?;
    let mut ok = false;
    for i in 0..40 {
        if convex_health(&base).await.unwrap_or(false) {
            ok = true;
            break;
        }
        if i % 2 == 0 {
            info!(attempt=i+1, url=%base, "convex.ensure: waiting for health");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if ok {
        info!(url=%base, pid=?child.id(), "convex.ensure: healthy after start");
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
