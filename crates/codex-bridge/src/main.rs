#[cfg(unix)]
use std::convert::TryInto;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use axum::extract::ws::{Message, WebSocket};
use axum::{Router, extract::State, extract::WebSocketUpgrade, response::IntoResponse, routing::get};
use clap::Parser;
use futures::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, broadcast},
};
use tracing::{error, info, warn};
use chrono::TimeZone;
use tracing_subscriber::prelude::*;
use std::time::Duration;

mod history;
mod projects;
mod skills;
mod mirror;

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

    /// Start a local Convex (SQLite) backend on loopback before serving WS
    #[arg(long, env = "OPENAGENTS_WITH_CONVEX", default_value_t = false)]
    with_convex: bool,

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
}

const MAX_HISTORY_LINES: usize = 2000;

struct AppState {
    tx: broadcast::Sender<String>,
    child_stdin: Mutex<Option<tokio::process::ChildStdin>>, // drop after first write to signal EOF
    child_pid: Mutex<Option<u32>>,
    opts: Opts,
    // Track last seen session id so we can resume on subsequent prompts
    last_thread_id: Mutex<Option<String>>,
    // Replay buffer for new websocket clients
    history: Mutex<Vec<String>>,
    history_cache: Mutex<crate::history::HistoryCache>,
    mirror: mirror::ConvexMirror,
    // Current Convex thread doc id being processed (for mapping thread.started -> Convex threadId)
    current_convex_thread: Mutex<Option<String>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let opts = Opts::parse();

    // Optional: supervise/start local Convex backend
    if opts.with_convex {
        if let Err(e) = ensure_convex_running(&opts).await {
            error!(?e, "failed to ensure local Convex is running");
        }
        // Optional destructive clear of all Convex data before ingest
        if std::env::var("OPENAGENTS_CONVEX_CLEAR").ok().as_deref() == Some("1") {
            if let Err(e) = run_convex_clear_all(opts.convex_port).await {
                error!(?e, "convex clearAll failed");
            }
        }
    }

    let (mut child, tx) = spawn_codex(&opts).await?;
    let state = Arc::new(AppState {
        tx,
        child_stdin: Mutex::new(Some(child.stdin.take().context("child stdin missing")?)),
        child_pid: Mutex::new(Some(child.pid)),
        opts: opts.clone(),
        last_thread_id: Mutex::new(None),
        history: Mutex::new(Vec::new()),
        history_cache: Mutex::new(crate::history::HistoryCache::new(400, std::time::Duration::from_secs(6))),
        mirror: mirror::ConvexMirror::new(mirror::default_mirror_dir()),
        current_convex_thread: Mutex::new(None),
    });

    // Start background ingester loop to drain mirror spool into Convex
    if opts.with_convex {
        let s = state.clone();
        tokio::spawn(run_convex_ingester(s, opts.convex_port));
    }

    // Start readers for stdout/stderr → broadcast + console
    start_stream_forwarders(child, state.clone()).await?;

    // Live-reload: watch ~/.openagents/skills and broadcast updates
    tokio::spawn(watch_skills_and_broadcast(state.clone()));
    // Background: enqueue historical threads/messages to the mirror spool on startup
    tokio::spawn(enqueue_historical_on_start(state.clone()));

    // HTTP submit endpoint for app → bridge turn submission

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    info!("binding" = %opts.bind, "msg" = "codex-bridge listening (route: /ws)");
    let listener = tokio::net::TcpListener::bind(&opts.bind).await?;
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

fn default_convex_bin() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".openagents/bin/local_backend");
    }
    PathBuf::from("local_backend")
}

fn default_convex_db() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".openagents/convex/data.sqlite3");
    }
    PathBuf::from("data.sqlite3")
}

async fn convex_health(url: &str) -> Result<bool> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build()?;
    let resp = client.get(format!("{}/instance_version", url)).send().await;
    match resp {
        Ok(r) if r.status().is_success() => Ok(true),
        _ => Ok(false),
    }
}

async fn ensure_convex_running(opts: &Opts) -> Result<()> {
    let bin = opts.convex_bin.clone().unwrap_or_else(default_convex_bin);
    let db = opts.convex_db.clone().unwrap_or_else(default_convex_db);
    let port = opts.convex_port;
    let interface = opts.convex_interface.clone();
    let base = format!("http://127.0.0.1:{}", port);
    if convex_health(&base).await.unwrap_or(false) {
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
            info!(url=%base, "convex healthy (already running)");
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
    info!(bin=%bin.display(), db=%db.display(), port, interface=%interface, "starting local Convex backend");
    let mut child = cmd.spawn().context("spawn convex local_backend")?;
    // Wait up to ~10s for health
    let mut ok = false;
    for _ in 0..20 {
        if convex_health(&base).await.unwrap_or(false) { ok = true; break; }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if ok {
        info!(url=%base, pid=?child.id(), "convex healthy after start");
        // Detach, let OS clean up on exit; user can stop process manually if needed
        Ok(())
    } else {
        // If failed to become healthy, try to kill child and report
        let _ = child.kill().await;
        error!(url=%base, "convex failed to report healthy in time");
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

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    info!("msg" = "websocket connected");

    // Broadcast reader → socket
    let mut rx = state.tx.subscribe();
    let (mut sink, mut stream) = socket.split();
    let history = { state.history.lock().await.clone() };
    let mut sink_task = tokio::spawn(async move {
        for line in history {
            if sink.send(Message::Text(line.into())).await.is_err() {
                return;
            }
        }
        loop {
            match rx.recv().await {
                Ok(line) => {
                    if sink.send(Message::Text(line.into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Socket → child stdin
    let stdin_state = state.clone();
    let mut read_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(t) => {
                    if let Some(cmd) = parse_control_command(&t) {
                        info!(?cmd, "ws control command");
                        match cmd {
                            ControlCommand::Interrupt => {
                                if let Err(e) = interrupt_running_child(&stdin_state).await {
                                    error!(?e, "failed to interrupt codex child");
                                }
                            }
                            // History/Thread controls removed — Convex-only UI now
                            ControlCommand::Projects => {
                                match crate::projects::list_projects() {
                                    Ok(items) => {
                                        let line = serde_json::json!({"type":"bridge.projects","items": items}).to_string();
                                        let _ = stdin_state.tx.send(line);
                                    }
                                    Err(e) => { error!(?e, "projects list failed via ws"); }
                                }
                            }
                            ControlCommand::Skills => {
                                match crate::skills::list_skills() {
                                    Ok(items) => {
                                        let line = serde_json::json!({"type":"bridge.skills","items": items}).to_string();
                                        let _ = stdin_state.tx.send(line);
                                    }
                                    Err(e) => { error!(?e, "skills list failed via ws"); }
                                }
                            }
                            ControlCommand::ConvexStatus => {
                                let url = format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let db = stdin_state.opts.convex_db.clone().unwrap_or_else(default_convex_db);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = if healthy { list_sqlite_tables(&db).unwrap_or_default() } else { Vec::new() };
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexCreateDemo => {
                                let db = stdin_state.opts.convex_db.clone().unwrap_or_else(default_convex_db);
                                let _ = create_demo_table(&db);
                                let url = format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = list_sqlite_tables(&db).unwrap_or_default();
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexCreateThreads => {
                                let db = stdin_state.opts.convex_db.clone().unwrap_or_else(default_convex_db);
                                let _ = create_threads_table(&db);
                                let url = format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = list_sqlite_tables(&db).unwrap_or_default();
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexCreateDemoThread => {
                                let db = stdin_state.opts.convex_db.clone().unwrap_or_else(default_convex_db);
                                let _ = create_threads_table(&db);
                                let _ = insert_demo_thread(&db);
                                let url = format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = list_sqlite_tables(&db).unwrap_or_default();
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexBackfill => {
                                let base = std::env::var("CODEXD_HISTORY_DIR").ok().unwrap_or_else(|| {
                                    std::env::var("HOME").map(|h| format!("{}/.codex/sessions", h)).unwrap_or_else(|_| ".".into())
                                });
                                let limit = 400usize;
                                match stdin_state.history_cache.lock().await.get(std::path::Path::new(&base), limit, None) {
                                    Ok(items) => {
                                        for h in items.clone() {
                                            if let Some(path) = crate::history::resolve_session_path(std::path::Path::new(&base), Some(&h.id), Some(&h.path)) {
                                                if let Ok(th) = crate::history::parse_thread(std::path::Path::new(&path)) {
                                                    let resume_id = th.resume_id.clone().unwrap_or(h.id.clone());
                                                    let title = th.title.clone();
                                                    // Compute strict started_ms as in enqueue_single_thread
                                                    let started_ms = th.started_ts.map(|t| t * 1000)
                                                        .or_else(|| crate::history::derive_started_ts_from_path(std::path::Path::new(&path)).map(|t| t * 1000))
                                                        .unwrap_or_else(|| {
                                                            let today = chrono::Local::now().date_naive();
                                                            let ndt = chrono::NaiveDateTime::new(today, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                                                            if let Some(dt) = chrono::Local.from_local_datetime(&ndt).single() { (dt.timestamp() as u64) * 1000 } else { 0 }
                                                        });
                                let _ = stdin_state.mirror.append(&crate::mirror::MirrorEvent::ThreadUpsert { thread_id: &resume_id, title: Some(&title), project_id: None, created_at: Some(started_ms), updated_at: Some(started_ms), source_path: Some(&path), resume_id: Some(&resume_id), convex_thread_id: None }).await;
                                                    for it in th.items {
                                                        if it.kind == "message" {
                                                            let role = it.role.as_deref().unwrap_or("assistant");
                                                            let text = it.text;
                                                            let _ = stdin_state.mirror.append(&crate::mirror::MirrorEvent::MessageCreate { thread_id: &resume_id, role, text: &text, ts: it.ts * 1000 }).await;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        let _ = stdin_state.tx.send(serde_json::json!({"type":"bridge.convex_backfill","status":"enqueued","count": items.len()}).to_string());
                                    }
                                    Err(e) => { error!(?e, "backfill scan failed"); }
                                }
                            }
                            ControlCommand::RunSubmit { thread_doc_id, text, project_id, resume_id } => {
                                // Map to project working dir
                                let desired_cd = project_id.as_ref().and_then(|pid| {
                                    match crate::projects::list_projects() { Ok(list) => list.into_iter().find(|p| p.id == *pid).map(|p| p.working_dir), Err(_) => None }
                                });
                                { *stdin_state.current_convex_thread.lock().await = Some(thread_doc_id.clone()); }
                                // Ensure child running with desired dir/resume
                                match spawn_codex_child_only_with_dir(
                                    &stdin_state.opts,
                                    desired_cd.clone().map(|s| std::path::PathBuf::from(s)),
                                    resume_id.as_deref(),
                                ).await {
                                    Ok(mut child) => {
                                        if let Some(stdin) = child.stdin.take() { *stdin_state.child_stdin.lock().await = Some(stdin); }
                                        if let Err(e) = start_stream_forwarders(child, stdin_state.clone()).await { error!(?e, "run.submit: forwarders failed"); }
                                        // Write config + text
                                        let mut cfg = serde_json::json!({ "sandbox": "danger-full-access", "approval": "never" });
                                        if let Some(cd) = desired_cd.as_deref() { cfg["cd"] = serde_json::Value::String(cd.to_string()); }
                                        if let Some(pid) = project_id.as_deref() { cfg["project"] = serde_json::json!({ "id": pid }); }
                                        let payload = format!("{}\n{}\n", cfg.to_string(), text);
                                        if let Some(mut stdin) = stdin_state.child_stdin.lock().await.take() {
                                            if let Err(e) = stdin.write_all(payload.as_bytes()).await { error!(?e, "run.submit: write failed"); }
                                            let _ = stdin.flush().await; drop(stdin);
                                        }
                                    }
                                    Err(e) => { error!(?e, "run.submit: spawn failed"); }
                                }
                            }
                            ControlCommand::ProjectSave { project } => {
                                match crate::projects::save_project(&project) {
                                    Ok(_) => {
                                        if let Ok(items) = crate::projects::list_projects() {
                                            let line = serde_json::json!({"type":"bridge.projects","items": items}).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                    }
                                    Err(e) => { error!(?e, "project save failed via ws"); }
                                }
                            }
                            ControlCommand::ProjectDelete { id } => {
                                match crate::projects::delete_project(&id) {
                                    Ok(_) => {
                                        if let Ok(items) = crate::projects::list_projects() {
                                            let line = serde_json::json!({"type":"bridge.projects","items": items}).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                    }
                                    Err(e) => { error!(?e, "project delete failed via ws"); }
                                }
                            }
                        }
                        continue;
                    }
                    let preview = if t.len() > 180 {
                        format!("{}…", &t[..180].replace('\n', "\\n"))
                    } else {
                        t.replace('\n', "\\n")
                    };
                    info!(
                        "msg" = "ws text received",
                        size = t.len(),
                        preview = preview
                    );
                    let desired_cd = extract_cd_from_ws_payload(&t);
                    let desired_resume = extract_resume_from_ws_payload(&t); // "last" | session id | "new"/"none" (start fresh)
                    info!(?desired_cd, ?desired_resume, msg = "parsed ws preface");
                    // Ensure we have a live codex stdin; respawn if needed
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn || desired_cd.is_some() {
                        // Decide on resume: prefer explicit resume id from preface; otherwise use last captured thread id
                        let resume_id = { stdin_state.last_thread_id.lock().await.clone() };
                        let resume_arg: Option<String> = match desired_resume.as_deref() {
                            Some("new") | Some("none") => None,
                            Some("last") => resume_id.clone(),
                            Some(s) if !s.is_empty() => Some(s.to_string()),
                            _ => resume_id.clone(),
                        };
                        // Defer closing previous stdin until we know the new child spawned.
                        match spawn_codex_child_only_with_dir(
                            &stdin_state.opts,
                            desired_cd.clone(),
                            resume_arg.as_deref(),
                        )
                        .await
                        {
                            Ok(mut child) => {
                                // Close previous stdin only after we have a new child
                                if desired_cd.is_some() {
                                    let mut g = stdin_state.child_stdin.lock().await;
                                    let _ = g.take();
                                }
                                {
                                    let mut pid_lock = stdin_state.child_pid.lock().await;
                                    *pid_lock = Some(child.pid);
                                }
                                if let Some(stdin) = child.stdin.take() {
                                    *stdin_state.child_stdin.lock().await = Some(stdin);
                                } else {
                                    error!("respawned codex missing stdin");
                                }
                                // start forwarding for new child
                                if let Err(e) =
                                    start_stream_forwarders(child, stdin_state.clone()).await
                                {
                                    error!(?e, "failed starting forwarders for respawned codex");
                                }
                            }
                            Err(e) => {
                                error!(?e, "failed to respawn codex");
                            }
                        }
                    }

                    let mut guard = stdin_state.child_stdin.lock().await;
                    if let Some(mut stdin) = guard.take() {
                        let mut data = t.to_string();
                        // Always-resume mode: no injection fallback
                        if !data.ends_with('\n') {
                            data.push('\n');
                        }
                        let write_preview = if data.len() > 160 {
                            format!("{}…", &data[..160].replace('\n', "\\n"))
                        } else {
                            data.replace('\n', "\\n")
                        };
                        info!(
                            "msg" = "writing to child stdin",
                            bytes = write_preview.len(),
                            preview = write_preview
                        );
                        if let Err(e) = stdin.write_all(data.as_bytes()).await {
                            error!(?e, "failed to write to codex stdin");
                            break;
                        }
                        let _ = stdin.flush().await;
                        drop(stdin); // close to send EOF
                    } else {
                        error!("stdin already closed; ignoring input");
                    }
                }
                Message::Binary(b) => {
                    info!("msg" = "ws binary received", size = b.len());
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn {
                        let resume_id = { stdin_state.last_thread_id.lock().await.clone() };
                        match spawn_codex_child_only_with_dir(
                            &stdin_state.opts,
                            None,
                            resume_id.as_deref(),
                        )
                        .await
                        {
                            Ok(mut child) => {
                                {
                                    let mut pid_lock = stdin_state.child_pid.lock().await;
                                    *pid_lock = Some(child.pid);
                                }
                                if let Some(stdin) = child.stdin.take() {
                                    *stdin_state.child_stdin.lock().await = Some(stdin);
                                }
                                if let Err(e) =
                                    start_stream_forwarders(child, stdin_state.clone()).await
                                {
                                    error!(?e, "failed starting forwarders for respawned codex");
                                }
                            }
                            Err(e) => {
                                error!(?e, "failed to respawn codex");
                            }
                        }
                    }

                    let mut guard = stdin_state.child_stdin.lock().await;
                    if let Some(mut stdin) = guard.take() {
                        if let Err(e) = stdin.write_all(&b).await {
                            error!(?e, "failed to write binary to codex stdin");
                            break;
                        }
                        let _ = stdin.flush().await;
                        drop(stdin);
                    } else {
                        error!("stdin already closed; ignoring binary");
                    }
                }
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => {}
            }
        }
    });

    // Await either task end
    tokio::select! {
        _ = (&mut sink_task) => { read_task.abort(); },
        _ = (&mut read_task) => { sink_task.abort(); },
    }
    info!("msg" = "websocket disconnected");
}

struct ChildWithIo {
    pid: u32,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
}

async fn spawn_codex(opts: &Opts) -> Result<(ChildWithIo, broadcast::Sender<String>)> {
    let (bin, args) = build_bin_and_args(opts)?; // initial spawn: never add resume here
    let workdir = detect_repo_root(None);
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "spawning codex"
    );
    let mut command = Command::new(&bin);
    command
        .current_dir(&workdir)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            // Put the child in its own process group so we can signal the group
            let res = libc::setpgid(0, 0);
            if res != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("failed to spawn codex")?;

    let pid = child.id().context("child pid missing")?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (tx, _rx) = broadcast::channel::<String>(1024);

    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => info!(?status, "codex exited"),
            Err(e) => error!(?e, "codex wait failed"),
        }
    });

    Ok((
        ChildWithIo {
            pid,
            stdin,
            stdout,
            stderr,
        },
        tx,
    ))
}

// Note: single-purpose respawns are handled by spawn_codex_child_only_with_dir.

async fn spawn_codex_child_only_with_dir(
    opts: &Opts,
    workdir_override: Option<PathBuf>,
    resume_id: Option<&str>,
) -> Result<ChildWithIo> {
    let (bin, mut args) = build_bin_and_args(opts)?;
    // Attach resume args when requested; automatically fall back if the CLI
    // doesn't support exec resume on this machine.
    if let Some(rid) = resume_id {
        let supports = cli_supports_resume(&bin);
        if supports {
            if rid == "last" {
                info!(msg = "enabling resume --last");
                args.push("resume".into());
                args.push("--last".into());
                // No positional dash: exec reads from stdin when no prompt arg is provided
            } else {
                info!(resume = rid, msg = "enabling resume by id");
                args.push("resume".into());
                args.push(rid.into());
                // No positional dash: exec reads from stdin when no prompt arg is provided
            }
        } else {
            info!(
                requested = rid,
                msg = "exec resume not supported by codex binary; spawning without resume"
            );
        }
    }
    let workdir = workdir_override.unwrap_or_else(|| detect_repo_root(None));
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "respawn codex for new prompt"
    );
    let mut command = Command::new(&bin);
    command
        .current_dir(&workdir)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            let res = libc::setpgid(0, 0);
            if res != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("failed to spawn codex")?;
    let pid = child.id().context("child pid missing")?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => info!(?status, "codex exited"),
            Err(e) => error!(?e, "codex wait failed"),
        }
    });
    Ok(ChildWithIo {
        pid,
        stdin,
        stdout,
        stderr,
    })
}

fn build_bin_and_args(opts: &Opts) -> Result<(PathBuf, Vec<String>)> {
    let bin = match &opts.codex_bin {
        Some(p) => p.clone(),
        None => which::which("codex").unwrap_or_else(|_| PathBuf::from("codex")),
    };

    let mut args: Vec<String> = if let Some(args_str) = &opts.codex_args {
        shlex::split(args_str).ok_or_else(|| anyhow!("failed to parse CODEX_ARGS"))?
    } else {
        vec!["exec".into(), "--json".into()]
    };
    // Do not attach resume here; we add it per-message when respawning after a
    // prior thread id is known.
    if !opts.extra.is_empty() {
        args.extend(opts.extra.clone());
    }

    fn contains_flag(args: &[String], short: &str, long: &str) -> bool {
        args.iter().any(|a| {
            a == short
                || a == long
                || a.starts_with(&format!("{short}="))
                || a.starts_with(&format!("{long}="))
        })
    }
    fn contains_substring(args: &[String], needle: &str) -> bool {
        args.iter().any(|a| a.contains(needle))
    }

    let mut pre_flags: Vec<String> = Vec::new();
    if !contains_flag(&args, "-m", "--model") {
        pre_flags.push("-m".into());
        pre_flags.push("gpt-5".into());
    }
    if !contains_substring(&args, "model_reasoning_effort=") {
        pre_flags.push("-c".into());
        pre_flags.push("model_reasoning_effort=\"high\"".into());
    }
    if !args
        .iter()
        .any(|a| a == "--dangerously-bypass-approvals-and-sandbox")
    {
        pre_flags.push("--dangerously-bypass-approvals-and-sandbox".into());
    }
    // Ensure explicit sandbox + approvals flags so the CLI reports the correct state
    if !contains_flag(&args, "-s", "--sandbox") {
        pre_flags.push("-s".into());
        pre_flags.push("danger-full-access".into());
    }
    // Do not add explicit approvals when using bypass; the bypass implies no approvals
    // Strongly hint full disk access and override default preset via config
    if !contains_substring(&args, "sandbox_permissions=") {
        pre_flags.push("-c".into());
        pre_flags.push(r#"sandbox_permissions=["disk-full-access"]"#.into());
    }
    if !contains_substring(&args, "sandbox_mode=") {
        pre_flags.push("-c".into());
        pre_flags.push(r#"sandbox_mode="danger-full-access""#.into());
    }
    if !contains_substring(&args, "approval_policy=") {
        pre_flags.push("-c".into());
        pre_flags.push(r#"approval_policy="never""#.into());
    }

    if !pre_flags.is_empty() {
        let mut updated = pre_flags;
        updated.extend(args);
        args = updated;
    }

    Ok((bin, args))
}

fn cli_supports_resume(bin: &PathBuf) -> bool {
    // Strict detection: only treat as supported if `codex exec resume --help` succeeds.
    match std::process::Command::new(bin)
        .args(["exec", "resume", "--help"]) // exists only on resume-capable builds
        .output()
    {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

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
                        // Mirror: enqueue thread upsert, mapped to current Convex thread if known
                        let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                        let _ = state_for_stdout.mirror.append(&crate::mirror::MirrorEvent::ThreadUpsert {
                            thread_id: convex_tid_opt.as_deref().unwrap_or(val),
                            title: Some("Thread"),
                            project_id: None,
                            created_at: Some(now_ms()),
                            updated_at: Some(now_ms()),
                            source_path: None,
                            resume_id: Some(val),
                            convex_thread_id: convex_tid_opt.as_deref(),
                        }).await;
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
                                if let Some(tid) = state_for_stdout.last_thread_id.lock().await.clone() {
                                    let role_s = if role == "assistant" { "assistant" } else if role == "user" { "user" } else { "" };
                                    if !role_s.is_empty() {
                                        let _ = state_for_stdout.mirror.append(&crate::mirror::MirrorEvent::MessageCreate { thread_id: &tid, role: role_s, text: &txt, ts: now_ms() }).await;
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
                                if let Some(tid) = state_for_stdout.last_thread_id.lock().await.clone() {
                                    let _ = state_for_stdout.mirror.append(&crate::mirror::MirrorEvent::JsonlItem { thread_id: &tid, kind: "reason", ts: now_ms(), text: Some(&txt), data: None }).await;
                                }
                            }
                        }
                    }
                }
                // Item lifecycle: command executions and others
                if let Some(ty) = t.as_deref() {
                    if ty.starts_with("item.") {
                        if let Some(payload) = v.get("item").or_else(|| v.get("payload").and_then(|p| p.get("item"))) {
                            let kind = payload.get("type").and_then(|x| x.as_str()).unwrap_or("");
                            let map_kind = match kind {
                                "command_execution" => Some("cmd"),
                                "file_change" => Some("file"),
                                "web_search" => Some("search"),
                                "mcp_tool_call" => Some("mcp"),
                                "todo_list" => Some("todo"),
                                _ => None,
                            };
                            if let Some(k) = map_kind {
                                if let Some(tid) = state_for_stdout.last_thread_id.lock().await.clone() {
                                    // Store payload as JSON string in text to avoid type issues; UI can parse
                                    let payload_str = payload.to_string();
                                    let _ = state_for_stdout.mirror.append(&crate::mirror::MirrorEvent::JsonlItem { thread_id: &tid, kind: k, ts: now_ms(), text: Some(&payload_str), data: None }).await;
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

fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }

fn summarize_exec_delta_for_log(line: &str) -> Option<String> {
    // Try to parse line as JSON and compact large delta arrays for logging only
    let mut root: JsonValue = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return None,
    };

    // Check top-level and nested msg without overlapping borrows
    let is_top = root.get("type").and_then(|t| t.as_str()) == Some("exec_command_output_delta");
    let mut_target: Option<&mut JsonValue> = if is_top {
        Some(&mut root)
    } else {
        let msg_opt = {
            if let Some(obj) = root.as_object_mut() {
                obj.get_mut("msg")
            } else {
                None
            }
        };
        if let Some(m) = msg_opt {
            if m.get("type").and_then(|t| t.as_str()) == Some("exec_command_output_delta") {
                Some(m)
            } else {
                None
            }
        } else {
            None
        }
    };
    let tgt = match mut_target {
        Some(t) => t,
        None => return None,
    };

    // Replace large fields
    if let Some(arr) = tgt.get_mut("chunk").and_then(|v| v.as_array_mut()) {
        let len = arr.len();
        *tgt.get_mut("chunk").unwrap() = JsonValue::String(format!("[{} elements]", len));
    }
    if let Some(arr) = tgt.get_mut("chunk_bytes").and_then(|v| v.as_array_mut()) {
        let len = arr.len();
        *tgt.get_mut("chunk_bytes").unwrap() = JsonValue::String(format!("[{} elements]", len));
    }

    Some(match serde_json::to_string(&root) {
        Ok(s) => s,
        Err(_) => return None,
    })
}

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
                        let line = serde_json::json!({"type":"bridge.skills","items": items}).to_string();
                        let _ = state.tx.send(line);
                    }
                    Err(e) => { error!(?e, "skills list failed on change"); }
                }
            }
            Err(_disconnected) => break,
        }
    }
}

async fn run_convex_ingester(state: Arc<AppState>, port: u16) {
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", port);
    loop {
        let spool = crate::mirror::default_mirror_dir().join("spool.jsonl");
        let run_now = match std::fs::metadata(&spool) { Ok(m) => m.len() > 0, Err(_) => false };
        if run_now {
            let lines = std::fs::read_to_string(&spool).unwrap_or_default();
            if lines.trim().is_empty() {
                tokio::time::sleep(Duration::from_secs(6)).await;
                continue;
            }
            let mut client = match ConvexClient::new(&url).await {
                Ok(c) => c,
                Err(e) => { warn!(?e, "convex client init failed"); tokio::time::sleep(Duration::from_secs(6)).await; continue; }
            };
            let mut failed: Vec<String> = Vec::new();
            let mut ok_count: usize = 0;
            for line in lines.split('\n').filter(|l| !l.trim().is_empty()) {
                let parsed: serde_json::Value = match serde_json::from_str(line) { Ok(v)=>v, Err(_)=> { failed.push(line.to_string()); continue } };
                let t = parsed.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if t == "thread_upsert" {
                    let tid = parsed.get("thread_id").and_then(|x| x.as_str()).unwrap_or("");
                    if tid.is_empty() { continue }
                    let mut args: BTreeMap<String, Value> = BTreeMap::new();
                    // Prefer explicit convex_thread_id as threadId; else fall back to thread_id
                    let convex_tid = parsed.get("convex_thread_id").and_then(|x| x.as_str()).unwrap_or("");
                    if !convex_tid.is_empty() { args.insert("threadId".into(), Value::from(convex_tid)); }
                    else { args.insert("threadId".into(), Value::from(tid)); }
                    if let Some(resume) = parsed.get("resume_id").and_then(|x| x.as_str()) { args.insert("resumeId".into(), Value::from(resume)); }
                    if let Some(title) = parsed.get("title").and_then(|x| x.as_str()) { args.insert("title".into(), Value::from(title)); }
                    if let Some(pid) = parsed.get("project_id").and_then(|x| x.as_str()) { args.insert("projectId".into(), Value::from(pid)); }
                    let mut ca = parsed.get("created_at").and_then(|x| x.as_u64());
                    let mut ua = parsed.get("updated_at").and_then(|x| x.as_u64());
                    if (ca.is_none() || ca == Some(0)) || (ua.is_none() || ua == Some(0)) {
                        if let Some(sp) = parsed.get("source_path").and_then(|x| x.as_str()) {
                            if let Some(derived) = crate::history::derive_started_ts_from_path(std::path::Path::new(sp)) { ca = Some(derived * 1000); ua = ca; }
                        }
                        if ca.is_none() || ca == Some(0) { warn!(thread_id=%tid, msg="ingester: created_at missing; deriving fallback"); }
                    }
                    if let Some(v) = ca { args.insert("createdAt".into(), Value::from(v as f64)); }
                    if let Some(v) = ua { args.insert("updatedAt".into(), Value::from(v as f64)); }
                    if let Err(e) = client.mutation("threads:upsertFromStream", args).await {
                        warn!(?e, "convex upsert failed"); failed.push(line.to_string());
                    } else { ok_count += 1; }
                } else if t == "message_create" {
                    let tid = parsed.get("thread_id").and_then(|x| x.as_str()).unwrap_or("");
                    let role = parsed.get("role").and_then(|x| x.as_str()).unwrap_or("");
                    let text = parsed.get("text").and_then(|x| x.as_str()).unwrap_or("");
                    let ts = parsed.get("ts").and_then(|x| x.as_u64()).unwrap_or(0);
                    if tid.is_empty() || role.is_empty() || text.is_empty() { continue }
                    let mut args: BTreeMap<String, Value> = BTreeMap::new();
                    args.insert("threadId".into(), Value::from(tid));
                    args.insert("role".into(), Value::from(role));
                    args.insert("kind".into(), Value::from("message"));
                    args.insert("text".into(), Value::from(text));
                    args.insert("ts".into(), Value::from(ts as f64));
                    if let Err(e) = client.mutation("messages:create", args).await {
                        warn!(?e, "convex message failed"); failed.push(line.to_string());
                    } else { ok_count += 1; }
                } else if t == "jsonl_item" {
                    let tid = parsed.get("thread_id").and_then(|x| x.as_str()).unwrap_or("");
                    if tid.is_empty() { continue }
                    let kind = parsed.get("kind").and_then(|x| x.as_str()).unwrap_or("item");
                    let ts = parsed.get("ts").and_then(|x| x.as_u64()).unwrap_or(0);
                    let text = parsed.get("text").and_then(|x| x.as_str());
                    let mut args: BTreeMap<String, Value> = BTreeMap::new();
                    args.insert("threadId".into(), Value::from(tid));
                    args.insert("kind".into(), Value::from(kind));
                    if let Some(t) = text { args.insert("text".into(), Value::from(t)); }
                    args.insert("ts".into(), Value::from(ts as f64));
                    if let Err(e) = client.mutation("messages:create", args).await {
                        warn!(?e, "convex jsonl item failed"); failed.push(line.to_string());
                    } else { ok_count += 1; }
                }
            }
            // Rewrite spool with failures only
            if let Err(e) = std::fs::write(&spool, if failed.is_empty() { String::new() } else { failed.join("\n") + "\n" }) {
                warn!(?e, "rewrite spool failed");
            }
            info!(ok = ok_count, failed = failed.len(), msg = "convex ingester batch done");
            // Broadcast progress to UI (optional)
            let remaining = failed.len();
            let line = serde_json::json!({
                "type": "bridge.ingest_progress",
                "ok_batch": ok_count,
                "remaining": remaining,
            }).to_string();
            let _ = state.tx.send(line);
        }
        tokio::time::sleep(Duration::from_secs(if run_now { 2 } else { 6 })).await;
    }
}

async fn run_convex_clear_all(port: u16) -> anyhow::Result<()> {
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", port);
    let mut client = ConvexClient::new(&url).await?;
    let args: BTreeMap<String, Value> = BTreeMap::new();
    match client.mutation("admin:clearAll", args).await {
        Ok(res) => { info!(?res, msg="convex clearAll done"); }
        Err(e) => { error!(?e, "convex clearAll error"); }
    }
    Ok(())
}

fn state_broadcast_send(line: String) -> Result<(), ()> {
    // Access to a global is not available; we can't reach AppState here without refactoring.
    // For now, print the JSON for consumers reading stdout; UI already ignores non-JSON lines.
    println!("{}", line);
    Ok(())
}

fn sessions_base_dir() -> String {
    std::env::var("CODEXD_HISTORY_DIR").ok().unwrap_or_else(|| {
        std::env::var("HOME").map(|h| format!("{}/.codex/sessions", h)).unwrap_or_else(|_| ".".into())
    })
}

async fn enqueue_historical_on_start(state: Arc<AppState>) {
    let base = sessions_base_dir();
    let base_path_owned = std::path::PathBuf::from(&base);
    let base_path = base_path_owned.as_path();
    // Initial quick batch (10 newest)
    let initial = match crate::history::scan_history(base_path, 10) {
        Ok(v) => v,
        Err(e) => { warn!(?e, "initial history scan failed"); Vec::new() }
    };
    let mut ok = 0usize;
    for h in &initial {
        if let Err(e) = enqueue_single_thread(&state, h).await { warn!(?e, id=%h.id, "enqueue thread failed") } else { ok += 1; }
    }
    info!(count = ok, base=%base, msg="initial enqueue to spool");

    // Continue with larger batch in the background
    let state2 = state.clone();
    let base_path2 = base_path_owned.clone();
    tokio::spawn(async move {
        let rest = match crate::history::scan_history(base_path2.as_path(), 2000) {
            Ok(mut all) => { if all.len() > 10 { all.drain(0..10); all } else { Vec::new() } },
            Err(_e) => Vec::new(),
        };
        let mut cnt = 0usize;
        for h in rest {
            let _ = enqueue_single_thread(&state2, &h).await.map(|_| { cnt += 1; });
            // Yield frequently so we don't block the runtime
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        if cnt > 0 { info!(count = cnt, msg = "enqueue remaining history to spool"); }
    });
}


async fn enqueue_single_thread(state: &Arc<AppState>, h: &crate::history::HistoryItem) -> anyhow::Result<()> {
    let path = std::path::Path::new(&h.path);
    let th = crate::history::parse_thread(path)?;
    let resume_id = th.resume_id.clone().unwrap_or(h.id.clone());
    let title = th.title.clone();
    // Use thread.started ts if present; else derive from filename/path; else midnight today
    let started_ms = th.started_ts.map(|t| t * 1000)
        .or_else(|| crate::history::derive_started_ts_from_path(path).map(|t| t * 1000))
        .unwrap_or_else(|| {
            // Midnight today (local), and log a warning with details for diagnostics
            let today = chrono::Local::now().date_naive();
            let ndt = chrono::NaiveDateTime::new(today, chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
            if let Some(dt) = chrono::Local.from_local_datetime(&ndt).single() {
                warn!(id=%resume_id, path=%h.path, msg="fallback to midnight today (no derivable start time)");
                (dt.timestamp() as u64) * 1000
            } else {
                warn!(id=%resume_id, path=%h.path, msg="failed to compute midnight; using 0");
                0
            }
        });
    // If started_ms is suspiciously close to now (likely import-time), log the full context
    let now_ms = now_ms();
    if started_ms > now_ms.saturating_sub(2 * 60 * 1000) {
        warn!(id=%resume_id, path=%h.path, title=%title, started_ms, now_ms, msg="suspicious start time close to now; check derive logic");
    }
    state.mirror.append(&crate::mirror::MirrorEvent::ThreadUpsert {
        thread_id: &resume_id,
        title: Some(&title),
        project_id: None,
        created_at: Some(started_ms),
        updated_at: Some(started_ms),
        source_path: Some(&h.path),
        resume_id: Some(&resume_id),
        convex_thread_id: None,
    }).await?;
    for it in th.items {
        if it.kind == "message" {
            let role = it.role.as_deref().unwrap_or("assistant");
            let text = it.text;
            state.mirror.append(&crate::mirror::MirrorEvent::MessageCreate {
                thread_id: &resume_id,
                role,
                text: &text,
                ts: it.ts * 1000,
            }).await?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum ControlCommand {
    Interrupt,
    Projects,
    Skills,
    ProjectSave { project: crate::projects::Project },
    ProjectDelete { id: String },
    ConvexStatus,
    ConvexCreateDemo,
    ConvexCreateThreads,
    ConvexCreateDemoThread,
    ConvexBackfill,
    RunSubmit { thread_doc_id: String, text: String, project_id: Option<String>, resume_id: Option<String> },
}

fn parse_control_command(payload: &str) -> Option<ControlCommand> {
    let mut lines = payload.lines();
    let first = lines.next()?.trim();
    if lines.next().is_some() {
        return None;
    }
    if !first.starts_with('{') {
        return None;
    }
    let v: JsonValue = serde_json::from_str(first).ok()?;
    let control = v.get("control").and_then(|c| c.as_str())?;
    match control {
        "interrupt" => Some(ControlCommand::Interrupt),
        "projects" => Some(ControlCommand::Projects),
        "skills" => Some(ControlCommand::Skills),
        "convex.status" => Some(ControlCommand::ConvexStatus),
        "convex.create_demo" => Some(ControlCommand::ConvexCreateDemo),
        "convex.create_threads" => Some(ControlCommand::ConvexCreateThreads),
        "convex.create_demo_thread" => Some(ControlCommand::ConvexCreateDemoThread),
        "convex.backfill" => Some(ControlCommand::ConvexBackfill),
        "project.save" => {
            let proj: crate::projects::Project = serde_json::from_value(v.get("project")?.clone()).ok()?;
            Some(ControlCommand::ProjectSave { project: proj })
        }
        "project.delete" => {
            let id = v.get("id").and_then(|x| x.as_str())?.to_string();
            Some(ControlCommand::ProjectDelete { id })
        }
        "run.submit" => {
            let tdoc = v.get("threadDocId").and_then(|x| x.as_str())?.to_string();
            let text = v.get("text").and_then(|x| x.as_str())?.to_string();
            let project_id = v.get("projectId").and_then(|x| x.as_str()).map(|s| s.to_string());
            let resume_id = v.get("resumeId").and_then(|x| x.as_str()).map(|s| s.to_string());
            Some(ControlCommand::RunSubmit { thread_doc_id: tdoc, text, project_id, resume_id })
        }
        _ => None,
    }
}

async fn interrupt_running_child(state: &Arc<AppState>) -> Result<()> {
    let pid_opt = { state.child_pid.lock().await.clone() };
    match pid_opt {
        Some(pid) => match send_interrupt_signal(pid) {
            Ok(_) => {
                info!(pid, "sent interrupt signal to codex child");
                Ok(())
            }
            Err(e) => Err(e.context("failed to send interrupt signal to codex child")),
        },
        None => {
            info!("msg" = "no child pid recorded when interrupt requested");
            Ok(())
        }
    }
}

#[cfg(unix)]
fn send_interrupt_signal(pid: u32) -> Result<()> {
    use std::io::ErrorKind;
    let pid_i32: i32 = pid.try_into().context("pid out of range for SIGINT")?;
    let target = -pid_i32;
    let res = unsafe { libc::kill(target, libc::SIGINT) };
    if res == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == ErrorKind::NotFound {
        return Ok(());
    }
    Err(anyhow::Error::from(err).context("libc::kill(SIGINT) failed"))
}

#[cfg(windows)]
fn send_interrupt_signal(pid: u32) -> Result<()> {
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .status()
        .context("failed to spawn taskkill for interrupt")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("taskkill exited with status {status:?}"))
    }
}

/// Detect the repository root directory so Codex runs from the right place.
/// Heuristics:
/// - Prefer the nearest ancestor that contains both `expo/` and `crates/` directories.
/// - If not found, fall back to the process current_dir.
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
            // reached filesystem root
            return original;
        }
    }
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(stripped) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") { return PathBuf::from(home).join(stripped); }
    } else if p == "~" {
        if let Ok(home) = std::env::var("HOME") { return PathBuf::from(home); }
    }
    PathBuf::from(p)
}

fn extract_cd_from_ws_payload(payload: &str) -> Option<PathBuf> {
    let first_line = payload.lines().next()?.trim();
    if !first_line.starts_with('{') {
        return None;
    }
    let v: JsonValue = serde_json::from_str(first_line).ok()?;
    let cd = v.get("cd").and_then(|s| s.as_str())?;
    if cd.is_empty() {
        return None;
    }
    Some(expand_home(cd))
}

fn extract_resume_from_ws_payload(payload: &str) -> Option<String> {
    let first_line = payload.lines().next()?.trim();
    if !first_line.starts_with('{') {
        return None;
    }
    let v: JsonValue = serde_json::from_str(first_line).ok()?;
    match v.get("resume") {
        Some(JsonValue::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}
