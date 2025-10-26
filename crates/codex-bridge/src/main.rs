use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

mod bootstrap;
mod convex_write;
mod codex_runner;
use crate::watchers::{
    sync_projects_to_convex, sync_skills_to_convex, watch_projects_and_sync, watch_sessions_and_tail,
    watch_skills_and_broadcast,
};

mod history;
mod projects;
mod skills;
mod controls;
mod state;
mod util;
mod ws;
mod watchers;
// spool/mirror removed — we write directly to Convex

#[derive(Parser, Debug, Clone)]
#[command(
    name = "codex-bridge",
    about = "WebSocket bridge to Codex CLI",
    version
)]
struct Opts {
    /// Bind address for the WebSocket server (e.g., 0.0.0.0:8787)
    #[arg(long, env = "CODEX_BRIDGE_BIND", default_value = "0.0.0.0:8787")]
    bind: String,

    /// Path to the codex binary (falls back to $CODEX_BIN or `codex` in PATH)
    #[arg(long, env = "CODEX_BIN")]
    codex_bin: Option<PathBuf>,

    /// Optional JSON exec args; if empty defaults to: exec --json
    #[arg(long, env = "CODEX_ARGS")]
    codex_args: Option<String>,

    /// Additional args after `--` are forwarded to codex
    #[arg(trailing_var_arg = true)]
    extra: Vec<String>,

    /// Bootstrap developer dependencies (Bun) and run Convex deploy/dev once.
    /// Enabled by default for CLI (cargo bridge). Tauri disables via --bootstrap=false.
    #[arg(long, env = "OPENAGENTS_BOOTSTRAP", default_value_t = true)]
    bootstrap: bool,

    /// Path to the Convex local backend binary (defaults to ~/.openagents/bin/local_backend)
    #[arg(long, env = "OPENAGENTS_CONVEX_BIN")]
    convex_bin: Option<PathBuf>,

    /// Port to bind Convex on (loopback)
    #[arg(long, env = "OPENAGENTS_CONVEX_PORT", default_value_t = 7788)]
    convex_port: u16,

    /// SQLite DB path for Convex (defaults to ~/.openagents/convex/data.sqlite3)
    #[arg(long, env = "OPENAGENTS_CONVEX_DB")]
    convex_db: Option<PathBuf>,

    /// Interface to bind Convex on (e.g., 0.0.0.0 for remote access, 127.0.0.1 for loopback only)
    #[arg(long, env = "OPENAGENTS_CONVEX_INTERFACE", default_value = "0.0.0.0")]
    convex_interface: String,

    /// Manage local Convex lifecycle: start/monitor/bootstrap. Enabled by default for CLI.
    /// Tauri disables with --manage-convex=false because it runs its own sidecar on 3210.
    #[arg(long, env = "OPENAGENTS_MANAGE_CONVEX", default_value_t = true)]
    manage_convex: bool,
}

const MAX_HISTORY_LINES: usize = 2000;

use crate::state::{AppState, StreamEntry};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let opts = Opts::parse();

    // Start Convex in the background so the WebSocket server can come up immediately.
    if opts.manage_convex {
        let opts_clone = opts.clone();
        tokio::spawn(async move {
            match bootstrap::ensure_convex_running(&opts_clone).await {
                Ok(()) => {
                    // Only attempt bootstrap when the backend is healthy
                    if opts_clone.bootstrap {
                        if let Err(e) = bootstrap::bootstrap_convex(&opts_clone).await {
                            error!(?e, "convex bootstrap failed");
                        }
                    } else {
                        info!("msg" = "OPENAGENTS_BOOTSTRAP disabled — skipping convex function push");
                    }
                }
                Err(e) => {
                    error!(?e, "failed to ensure local Convex is running; skipping bootstrap");
                }
            }
        });
    } else {
        info!("msg" = "OPENAGENTS_MANAGE_CONVEX=0 — bridge will not manage Convex; expecting an external process");
    }
    // Optional destructive clear of all Convex data before ingest (opt-in), only if we manage Convex
    if opts.manage_convex {
        if std::env::var("OPENAGENTS_CONVEX_CLEAR").ok().as_deref() == Some("1") {
            if let Err(e) = crate::util::run_convex_clear_all(opts.convex_port).await {
                error!(?e, "convex clearAll failed");
            }
        }
    }

    let (mut child, tx) = codex_runner::spawn_codex(&opts).await?;
    let state = Arc::new(AppState {
        tx,
        child_stdin: Mutex::new(Some(child.stdin.take().context("child stdin missing")?)),
        child_pid: Mutex::new(Some(child.pid)),
        opts: opts.clone(),
        last_thread_id: Mutex::new(None),
        history: Mutex::new(Vec::new()),
        history_cache: Mutex::new(crate::history::HistoryCache::new(400, std::time::Duration::from_secs(6))),
        current_convex_thread: Mutex::new(None),
        stream_track: Mutex::new(std::collections::HashMap::new()),
    });

    // Start readers for stdout/stderr → broadcast + console
    crate::ws::start_stream_forwarders(child, state.clone()).await?;

    // Live-reload + sync (optional toggle via OPENAGENTS_CONVEX_SYNC=0 to disable)
    let sync_enabled = std::env::var("OPENAGENTS_CONVEX_SYNC").ok().map(|v| v != "0").unwrap_or(true);
    if sync_enabled {
        tokio::spawn(watch_skills_and_broadcast(state.clone()));
        tokio::spawn(watch_projects_and_sync(state.clone()));
        // Initial sync of Projects and Skills (filesystem → Convex)
        {
            let st = state.clone();
            tokio::spawn(async move {
                if let Err(e) = sync_projects_to_convex(st.clone()).await { warn!(?e, "initial projects sync failed"); }
                if let Err(e) = sync_skills_to_convex(st.clone()).await { warn!(?e, "initial skills sync failed"); }
            });
        }
        // Watch Codex sessions for external runs and mirror to Convex (best-effort)
        tokio::spawn(watch_sessions_and_tail(state.clone()));
    } else {
        info!("msg" = "OPENAGENTS_CONVEX_SYNC=0 — FS→Convex sync disabled");
    }
    // Background: enqueue historical threads/messages to the mirror spool on startup
    tokio::spawn(crate::watchers::enqueue_historical_on_start(state.clone()));

    // HTTP submit endpoint for app → bridge turn submission
    let app = Router::new().route("/ws", get(crate::ws::ws_handler)).with_state(state);

    let bind_addr = opts.bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("binding" = %bind_addr, "msg" = "codex-bridge listening (route: /ws)");
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    // Default to info but quiet down noisy dependencies unless overridden by RUST_LOG
    let default_filter = "info,convex=warn,convex::base_client=warn,tungstenite=warn";
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter.into()))
        .with(fmt::layer())
        .try_init();
}

// legacy convex bin/db defaults now live in bootstrap

// legacy helper removed (use bootstrap/codex_runner detect_repo_root internally)

async fn convex_health(url: &str) -> Result<bool> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build()?;
    let resp = client.get(format!("{}/instance_version", url)).send().await;
    match resp {
        Ok(r) if r.status().is_success() => Ok(true),
        _ => Ok(false),
    }
}

async fn ensure_convex_running(opts: &Opts) -> Result<()> {
    info!(port = opts.convex_port, interface = %opts.convex_interface, "convex.ensure: begin");
    let bin = opts.convex_bin.clone().unwrap_or_else(default_convex_bin);
    if !bin.exists() {
        // Try to fetch/install the local backend binary via Convex CLI (bunx convex dev).
        if let Err(e) = ensure_local_backend_present().await {
            warn!(?e, path=%bin.display(), "convex local_backend missing and auto-install failed");
        }
    }
    let db = opts.convex_db.clone().unwrap_or_else(default_convex_db);
    let port = opts.convex_port;
    let interface = opts.convex_interface.clone();
    let base = format!("http://127.0.0.1:{}", port);
    let pre_healthy = convex_health(&base).await.unwrap_or(false);
    if pre_healthy {
        // If a previous instance is already running but the desired interface is not loopback,
        // attempt a best-effort restart so mobile devices can reach it.
        if opts.convex_interface.trim() != "127.0.0.1" {
            info!(url=%base, desired_interface=%opts.convex_interface, "convex healthy on loopback; restarting on desired interface");
            if let Err(e) = kill_listeners_on_port(port).await {
                warn!(?e, port, "failed killing existing convex on port; will try spawn anyway");
            }
            // Small delay to allow the port to free up
            tokio::time::sleep(Duration::from_millis(300)).await;
        } else {
            info!(url=%base, "convex.ensure: already healthy");
            return Ok(());
        }
    }
    // Spawn process
    std::fs::create_dir_all(db.parent().unwrap_or_else(|| Path::new(".")))
        .ok();
    let mut cmd = Command::new(&bin);
    cmd.arg(&db)
        .arg("--db").arg("sqlite")
        .arg("--interface").arg(&interface)
        .arg("--port").arg(port.to_string())
        .arg("--local-storage").arg(
            std::env::var("HOME").map(|h| format!("{}/.openagents/convex/storage", h)).unwrap_or_else(|_| "convex_local_storage".to_string())
        )
        .arg("--disable-beacon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    info!(bin=%bin.display(), db=%db.display(), port, interface=%interface, "convex.ensure: starting local backend");
    let mut child = cmd.spawn().context("spawn convex local_backend")?;
    // Wait up to ~20s for health (first-run migrations can be slow)
    let mut ok = false;
    for i in 0..40 {
        if convex_health(&base).await.unwrap_or(false) { ok = true; break; }
        if i % 2 == 0 { info!(attempt=i+1, url=%base, "convex.ensure: waiting for health"); }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if ok {
        info!(url=%base, pid=?child.id(), "convex.ensure: healthy after start");
        // Detach, let OS clean up on exit; user can stop process manually if needed
        Ok(())
    } else {
        // If failed to become healthy, try to kill child and report
        let _ = child.kill().await;
        error!(url=%base, "convex.ensure: failed to report healthy in time");
        anyhow::bail!("convex health probe failed")
    }
}

#[cfg(unix)]
async fn kill_listeners_on_port(port: u16) -> Result<()> {
    use std::process::Command as StdCommand;
    // Find listener PIDs with lsof (macOS/Linux)
    let output = StdCommand::new("lsof")
        .args(["-i", &format!(":{}", port), "-sTCP:LISTEN", "-t"])
        .output();
    let out = match output {
        Ok(o) => o,
        Err(e) => return Err(anyhow::Error::from(e).context("lsof not available to kill listeners")),
    };
    if !out.status.success() { return Ok(()); }
    let pids = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect::<Vec<_>>();
    for pid in pids {
        let _ = StdCommand::new("kill").args(["-TERM", &pid.to_string()]).status();
    }
    Ok(())
}

#[cfg(not(unix))]
async fn kill_listeners_on_port(_port: u16) -> Result<()> { Ok(()) }

async fn bootstrap_convex(opts: &Opts) -> Result<()> {
    // 1) Ensure .env.local exists with URL + ADMIN KEY for local dev
    ensure_env_local(opts.convex_port)?;
    // 2) Ensure Bun is installed or install it to ~/.bun/bin
    let bun_bin = ensure_bun_installed().await?;
    // 3) bun install (idempotent)
    let root = crate::util::detect_repo_root(None);
    info!(dir=%root.display(), msg="running bun install");
    let status = Command::new(&bun_bin)
        .arg("install")
        .current_dir(&root)
        .status()
        .await
        .context("bun install failed to spawn")?;
    if !status.success() { anyhow::bail!("bun install failed"); }
    // 4) bun run convex:dev:once (uses scripts/convex-cli.sh + .env.local)
    info!(port = opts.convex_port, msg="deploying Convex functions (dev:once)");
    let status = Command::new(&bun_bin)
        .args(["run", "convex:dev:once"]) 
        .current_dir(&root)
        .status()
        .await
        .context("bun run convex:dev:once failed to spawn")?;
    if !status.success() { anyhow::bail!("convex dev:once failed"); }
    Ok(())
}

fn ensure_env_local(port: u16) -> Result<()> {
    use std::io::Write;
    let root = crate::util::detect_repo_root(None);
    let path = root.join(".env.local");
    if path.exists() { return Ok(()); }
    let url = format!("http://127.0.0.1:{}", port);
    // Default local admin key. This matches our self-hosted dev defaults.
    let admin = "carnitas|017c5405aba48afe1d1681528424e4528026e69e3b99e400ef23f2f3741a11db225497db09";
    let mut f = std::fs::File::create(&path).context("create .env.local")?;
    writeln!(f, "CONVEX_SELF_HOSTED_URL={}", url).ok();
    writeln!(f, "CONVEX_URL={}", url).ok();
    writeln!(f, "CONVEX_ADMIN_KEY={}", admin).ok();
    writeln!(f, "CONVEX_SELF_HOSTED_ADMIN_KEY={}", admin).ok();
    info!(path=%path.display(), msg="wrote default .env.local for Convex dev");
    Ok(())
}

async fn ensure_bun_installed() -> Result<PathBuf> {
    // If bun is on PATH, use it; otherwise install to ~/.bun/bin/bun
    let from_path = which::which("bun").ok();
    if let Some(p) = from_path { return Ok(p); }
    // Try ~/.bun/bin/bun
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let bun_bin = PathBuf::from(&home).join(".bun/bin/bun");
    if bun_bin.exists() { return Ok(bun_bin); }
    info!("msg" = "bun not found; installing via bun.sh");
    // Install Bun non-interactively
    let install_cmd = format!("curl -fsSL https://bun.sh/install | bash");
    #[cfg(unix)]
    {
        let status = Command::new("bash")
            .arg("-lc")
            .arg(&install_cmd)
            .status()
            .await
            .context("failed running bun installer")?;
        if !status.success() {
            anyhow::bail!("bun installer failed");
        }
    }
    // Verify
    if bun_bin.exists() { Ok(bun_bin) } else { anyhow::bail!("bun not found after install") }
}

async fn ensure_local_backend_present() -> Result<()> {
    // Ensure bun is present (we invoke convex CLI via bunx)
    let bun_bin = ensure_bun_installed().await?;
    // Ask convex CLI to fetch/upgrade local backend binary into its cache
    // We use --once and --skip-push to avoid long-running watchers or pushing functions.
    let root = crate::util::detect_repo_root(None);
    info!(dir=%root.display(), msg="ensuring Convex local backend binary via convex CLI");
    let status = Command::new(&bun_bin)
        .args(["x", "convex", "dev", "--once", "--skip-push", "--local-force-upgrade"])
        .current_dir(&root)
        .status()
        .await
        .context("bunx convex dev bootstrap failed to spawn")?;
    if !status.success() {
        anyhow::bail!("bunx convex dev bootstrap failed (exit)");
    }
    // Copy the cached binary to ~/.openagents/bin/local_backend for our supervisor to use
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    // Default convex cache directory (~/.cache/convex/binaries/<version>/convex-local-backend)
    let cache_root = PathBuf::from(&home).join(".cache/convex/binaries");
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&cache_root) {
        for e in entries.flatten() {
            let p = e.path().join(if cfg!(windows) { "convex-local-backend.exe" } else { "convex-local-backend" });
            if p.exists() {
                if let Ok(meta) = std::fs::metadata(&p) {
                    if let Ok(modt) = meta.modified() { candidates.push((modt, p)); }
                }
            }
        }
    }
    if candidates.is_empty() {
        anyhow::bail!("convex CLI did not provision a local backend binary in cache");
    }
    candidates.sort_by_key(|(t, _)| *t);
    let src = candidates.last().unwrap().1.clone();
    let dest = default_convex_bin();
    if let Some(parent) = dest.parent() { let _ = std::fs::create_dir_all(parent); }
    std::fs::copy(&src, &dest).context("copy convex local_backend to ~/.openagents/bin")?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(mut perms) = std::fs::metadata(&dest).map(|m| m.permissions()) {
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&dest, perms);
        }
    }
    info!(from=%src.display(), to=%dest.display(), msg="installed Convex local backend binary");
    Ok(())
}

fn list_sqlite_tables(db_path: &PathBuf) -> Result<Vec<String>> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in iter { out.push(r?); }
    Ok(out)
}

fn create_demo_table(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS oa_demo (id INTEGER PRIMARY KEY, k TEXT, v TEXT)",
        rusqlite::params![],
    )?;
    Ok(())
}

fn create_threads_table(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS threads (\
            id TEXT PRIMARY KEY,\
            rollout_path TEXT NOT NULL,\
            title TEXT,\
            resume_id TEXT,\
            project_id TEXT,\
            source TEXT,\
            created_at INTEGER,\
            updated_at INTEGER\
        )",
        rusqlite::params![],
    )?;
    Ok(())
}

fn insert_demo_thread(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let id = format!("demo-{}", now);
    conn.execute(
        "INSERT OR REPLACE INTO threads (id, rollout_path, title, resume_id, project_id, source, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, "", "Demo Thread", rusqlite::types::Null, rusqlite::types::Null, "demo", now, now],
    )?;
    Ok(())
}

// ws handlers moved to ws.rs


// ChildWithIo is provided by codex_runner module

// codex spawn helpers moved to codex_runner.rs

async fn start_stream_forwarders(mut child: ChildWithIo, state: Arc<AppState>) -> Result<()> {
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;

    let tx_out = state.tx.clone();
    let state_for_stdout = state.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // Drop noisy CLI lines we do not want to surface
            let low = line.trim().to_ascii_lowercase();
            if low.contains("reading prompt from stdin") || low == "no prompt provided via stdin." {
                continue;
            }
            // Transient exec errors from the CLI that are immediately retried (e.g., after path correction)
            // are confusing in the UI; suppress them from the broadcast/log stream.
            if low.contains("codex_core::exec: exec error") {
                continue;
            }
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            println!("{}", log);
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stdout", raw = %line);
            }
            // Try to capture thread id for resume
            if let Ok(v) = serde_json::from_str::<JsonValue>(&line) {
                let t = v
                    .get("type")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        v.get("msg")
                            .and_then(|m| m.get("type"))
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                    });
                if matches!(t.as_deref(), Some("thread.started")) {
                    let id = v.get("thread_id").and_then(|x| x.as_str()).or_else(|| {
                        v.get("msg")
                            .and_then(|m| m.get("thread_id"))
                            .and_then(|x| x.as_str())
                    });
                    if let Some(val) = id {
                        let _ = state_for_stdout
                            .last_thread_id
                            .lock()
                            .await
                            .insert(val.to_string());
                        info!(thread_id=%val, msg="captured thread id for resume");
                        // Direct: upsert thread in Convex (map Codex CLI id -> Convex thread doc id when known)
                        let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                        {
                            use convex::{ConvexClient, Value};
                            use std::collections::BTreeMap;
                            let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                            if let Ok(mut client) = ConvexClient::new(&url).await {
                                let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                if let Some(ctid) = convex_tid_opt.as_deref() { args.insert("threadId".into(), Value::from(ctid)); }
                                args.insert("resumeId".into(), Value::from(val));
                                args.insert("title".into(), Value::from("Thread"));
                                args.insert("createdAt".into(), Value::from(now_ms() as f64));
                                args.insert("updatedAt".into(), Value::from(now_ms() as f64));
                                let _ = client.mutation("threads:upsertFromStream", args).await;
                            }
                        }
                    }
                }
                if matches!(t.as_deref(), Some("turn.completed")) {
                    let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                    let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                    if !target_tid.is_empty() {
                        finalize_streaming_for_thread(&state_for_stdout, &target_tid).await;
                    }
                }
                // Mirror agent/user messages when present in newer shapes
                if t.as_deref() == Some("response_item") {
                    if let Some(payload) = v.get("payload") {
                        if payload.get("type").and_then(|x| x.as_str()) == Some("message") {
                            let role = payload.get("role").and_then(|x| x.as_str()).unwrap_or("");
                            let mut txt = String::new();
                            if let Some(arr) = payload.get("content").and_then(|x| x.as_array()) {
                                for part in arr { if let Some(t) = part.get("text").and_then(|x| x.as_str()) { if !txt.is_empty() { txt.push('\n'); } txt.push_str(t); } }
                            }
                            if !txt.trim().is_empty() {
                                let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                                let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                                if !target_tid.is_empty() {
                                    if role == "assistant" {
                                        stream_upsert_or_append(&state_for_stdout, &target_tid, "assistant", &txt).await;
                                    }
                                }
                            }
                        } else if payload.get("type").and_then(|x| x.as_str()) == Some("reasoning") {
                            // Aggregate summary text if available
                            let mut txt = String::new();
                            if let Some(arr) = payload.get("summary").and_then(|x| x.as_array()) {
                                for part in arr { if let Some(t) = part.get("text").and_then(|x| x.as_str()) { if !txt.is_empty() { txt.push('\n'); } txt.push_str(t); } }
                            }
                            if !txt.trim().is_empty() {
                                let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                                let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                                if !target_tid.is_empty() { stream_upsert_or_append(&state_for_stdout, &target_tid, "reason", &txt).await; }
                            }
                        }
                    }
                }
                // Item lifecycle: command executions and others
                if let Some(ty) = t.as_deref() {
                    if ty.starts_with("item.") {
                        if let Some(payload) = v.get("item").or_else(|| v.get("payload").and_then(|p| p.get("item"))) {
                            let kind = payload.get("type").and_then(|x| x.as_str()).unwrap_or("");
                            // Special-case agent_message and reasoning to store as first-class messages
                            if kind == "agent_message" {
                                let text = payload.get("text").and_then(|x| x.as_str()).unwrap_or("");
                                if !text.trim().is_empty() {
                                    let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                                    let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                                    if !target_tid.is_empty() {
                                        if !try_finalize_stream_kind(&state_for_stdout, &target_tid, "assistant", text).await {
                                            use convex::{ConvexClient, Value};
                                            use std::collections::BTreeMap;
                                            let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                                            if let Ok(mut client) = ConvexClient::new(&url).await {
                                                let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                                args.insert("threadId".into(), Value::from(target_tid.clone()));
                                                args.insert("role".into(), Value::from("assistant"));
                                                args.insert("kind".into(), Value::from("message"));
                                                args.insert("text".into(), Value::from(text));
                                                args.insert("ts".into(), Value::from(now_ms() as f64));
                                                let _ = client.mutation("messages:create", args).await;
                                            }
                                        }
                                    }
                                }
                            } else if kind == "reasoning" {
                                let mut text_owned: String = payload.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                if text_owned.trim().is_empty() {
                                    if let Some(arr) = payload.get("summary").and_then(|x| x.as_array()) {
                                        for part in arr { if let Some(t) = part.get("text").and_then(|x| x.as_str()) { if !text_owned.is_empty() { text_owned.push('\n'); } text_owned.push_str(t); } }
                                    }
                                }
                                let text = text_owned;
                                if !text.trim().is_empty() {
                                    let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                                    let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                                    if !target_tid.is_empty() {
                                        if !try_finalize_stream_kind(&state_for_stdout, &target_tid, "reason", &text).await {
                                            use convex::{ConvexClient, Value};
                                            use std::collections::BTreeMap;
                                            let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                                            if let Ok(mut client) = ConvexClient::new(&url).await {
                                                let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                                args.insert("threadId".into(), Value::from(target_tid.clone()));
                                                args.insert("kind".into(), Value::from("reason"));
                                                args.insert("text".into(), Value::from(text));
                                                args.insert("ts".into(), Value::from(now_ms() as f64));
                                                let _ = client.mutation("messages:create", args).await;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Tool/exec/search/todo buckets
                                let map_kind = match kind {
                                    "command_execution" => Some("cmd"),
                                    "file_change" => Some("file"),
                                    "web_search" => Some("search"),
                                    "mcp_tool_call" => Some("mcp"),
                                    "todo_list" => Some("todo"),
                                    _ => None,
                                };
                                if let Some(k) = map_kind {
                                    let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                                    let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                                    if !target_tid.is_empty() {
                                        let payload_str = payload.to_string();
                                        use convex::{ConvexClient, Value};
                                        use std::collections::BTreeMap;
                                        let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                                        if let Ok(mut client) = ConvexClient::new(&url).await {
                                            let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                            args.insert("threadId".into(), Value::from(target_tid.clone()));
                                            args.insert("kind".into(), Value::from(k));
                                            args.insert("text".into(), Value::from(payload_str));
                                            args.insert("ts".into(), Value::from(now_ms() as f64));
                                            match client.mutation("messages:create", args).await {
                                                Ok(_) => info!(thread_id=%target_tid, kind=%k, msg="convex message:create ok"),
                                                Err(e) => warn!(?e, thread_id=%target_tid, kind=%k, msg="convex message:create failed"),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // no-op for agent_message in always-resume mode
            }
            {
                let mut history = state_for_stdout.history.lock().await;
                history.push(line.clone());
                if history.len() > MAX_HISTORY_LINES {
                    let drop = history.len() - MAX_HISTORY_LINES;
                    history.drain(0..drop);
                }
            }
            let _ = tx_out.send(line);
        }
        info!("msg" = "stdout stream ended");
    });

    let tx_err = state.tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // Drop noisy CLI lines we do not want to surface
            let low = line.trim().to_ascii_lowercase();
            if low.contains("reading prompt from stdin") || low == "no prompt provided via stdin." {
                continue;
            }
            if low.contains("codex_core::exec: exec error") {
                continue;
            }
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            eprintln!("{}", log);
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stderr", raw = %line);
            }
            {
                let mut history = state.history.lock().await;
                history.push(line.clone());
                if history.len() > MAX_HISTORY_LINES {
                    let drop = history.len() - MAX_HISTORY_LINES;
                    history.drain(0..drop);
                }
            }
            let _ = tx_err.send(line);
        }
        info!("msg" = "stderr stream ended");
    });

    Ok(())
}

    // streaming helpers moved to convex_write.rs

/* moved to watchers.rs
async fn watch_skills_and_broadcast(state: Arc<AppState>) {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    let user_dir = crate::skills::skills_dir();
    let registry_dirs = crate::skills::registry_skills_dirs();
    let mut watched: Vec<std::path::PathBuf> = Vec::new();
    if let Err(e) = std::fs::create_dir_all(&user_dir) { error!(?e, "skills mkdir failed"); }
    if user_dir.is_dir() { watched.push(user_dir.clone()); }
    for d in registry_dirs { if d.is_dir() { watched.push(d); } }

    if watched.is_empty() { return; }
    let (txev, rcev) = channel();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let _ = txev.send(res);
    }) {
        Ok(w) => w,
        Err(e) => { error!(?e, "skills watcher create failed"); return; }
    };
    for d in &watched {
        if let Err(e) = watcher.watch(d, RecursiveMode::Recursive) {
            error!(dir=%d.display(), ?e, "skills watcher watch failed");
        } else {
            info!(dir=%d.display(), msg="skills watcher started");
        }
    }
    // Blocking loop; debounced
    loop {
        match rcev.recv() {
            Ok(_evt) => {
                // Drain quick bursts
                let _ = rcev.try_recv(); let _ = rcev.try_recv();
                match crate::skills::list_skills() {
                    Ok(items) => {
                        // Broadcast for legacy clients
                        let line = serde_json::json!({"type":"bridge.skills","items": items}).to_string();
                        let _ = state.tx.send(line);
                        // Also mirror to Convex (user + registry scopes)
                        if let Err(e) = sync_skills_to_convex(state.clone()).await { warn!(?e, "skills convex sync failed on change"); }
                    }
                    Err(e) => { error!(?e, "skills list failed on change"); }
                }
            }
            Err(_disconnected) => break,
        }
    }
}

// Ingest/backfill helpers moved to watchers.rs and util.rs

// Control parser moved to controls.rs

// interrupt + send_interrupt_signal live in ws.rs

// util + ws helpers now live in their respective modules

// Filesystem → Convex sync now lives in watchers.rs

// Project-scoped skills sync moved to watchers.rs

