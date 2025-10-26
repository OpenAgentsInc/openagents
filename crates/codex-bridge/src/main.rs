//! codex-bridge binary entrypoint.
//!
//! What this process does now:
//! - Optionally manages a self‑hosted Convex backend (SQLite) for persistence
//!   when running standalone (CLI). Desktop (Tauri) typically runs its own
//!   sidecar and disables bridge‑side management via flags.
//! - Spawns the Codex CLI with JSONL output enabled and tails stdout/stderr.
//! - Exposes a WebSocket at `/ws` used primarily for control messages
//!   (e.g., `run.submit`, status probes, projects/skills syncing). For
//!   backwards‑compatibility and mobile usage, the raw JSONL lines are still
//!   broadcast to connected clients.
//! - Normalizes Codex JSONL into structured mutations and writes them into
//!   Convex (threads, messages, tool rows). Mobile can continue using the
//!   broadcast feed; Desktop consumes the Convex data model.
//! - Filesystem watchers (projects/skills/sessions) mirror local changes into
//!   Convex. The heavy lifting lives in submodules to keep this entry thin.

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;
// Imports required by legacy helpers that are slated for removal; kept to
// avoid compile errors while tests run against other modules.
use std::time::Duration;
use std::path::Path;
use std::process::{Stdio};
use tokio::process::Command as TokioCommand;
use crate::bootstrap::{default_convex_bin, default_convex_db};

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

use crate::state::AppState;

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

// (pruned legacy) health helpers now live in crate::bootstrap

// (pruned legacy) convex lifecycle now lives in crate::bootstrap

// (pruned legacy) kill_listeners_on_port moved to bootstrap

// (pruned legacy) bootstrap lives in crate::bootstrap

// (pruned legacy)

// (pruned legacy)

// (pruned legacy)

// (pruned legacy)

// (pruned legacy)

// (pruned legacy)

// (pruned legacy)

// ws handlers moved to ws.rs


// ChildWithIo is provided by codex_runner module

// codex spawn helpers moved to codex_runner.rs

/* legacy copy removed; use crate::ws::start_stream_forwarders */
/*
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
*/

    // streaming helpers moved to convex_write.rs

// watchers moved to watchers.rs

// Ingest/backfill helpers moved to watchers.rs and util.rs

// Control parser moved to controls.rs

// interrupt + send_interrupt_signal live in ws.rs

// util + ws helpers now live in their respective modules

// Filesystem → Convex sync now lives in watchers.rs

// Project-scoped skills sync moved to watchers.rs
