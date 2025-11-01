//! OpenAgents bridge binary entrypoint.
//!
//! What this process does now:
//! - Persists to Tinyvex (in-process SQLite) for fast local sync.
//! - Spawns the Codex CLI with JSONL output enabled and tails stdout/stderr.
//! - Exposes a WebSocket at `/ws` used primarily for control messages
//!   (e.g., `run.submit`, status probes, projects/skills syncing). For
//!   backwards‑compatibility and mobile usage, the raw JSONL lines are still
//!   broadcast to connected clients.
//! - Normalizes Codex JSONL into structured mutations and writes them into
//!   Tinyvex (threads, messages). Mobile can continue using the broadcast
//!   feed; Desktop reads via Tinyvex WS controls.

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use axum::{Router, routing::get};
use clap::Parser;
use tokio::sync::Mutex;
use tracing::{info, warn};
use tracing_subscriber::prelude::*;

mod codex_runner;
mod tinyvex_write;
// watchers removed with Convex

mod controls;
mod history;
mod projects;
mod skills;
mod state;
mod util;
mod ws;
mod types;
mod provider_claude;
mod claude_runner;
pub mod watchers;
// spool/mirror removed — we write directly to Convex

#[derive(Parser, Debug, Clone)]
#[command(
    name = "oa-bridge",
    about = "OpenAgents WebSocket bridge (spawns agent CLIs, persists to Tinyvex)",
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

    // Convex options removed; Tinyvex is in-process.

    /// Optional WebSocket token required for `/ws` upgrades. When set, incoming
    /// clients must provide a matching token via `Authorization: Bearer` or
    /// `?token=` query parameter. If unset here, we also attempt to load it from
    /// `~/.openagents/bridge.json` (key: `token`).
    #[arg(long, env = "OPENAGENTS_BRIDGE_TOKEN")]
    ws_token: Option<String>,

    /// Path to the Claude CLI (falls back to $CLAUDE_BIN or `claude` in PATH)
    #[arg(long, env = "CLAUDE_BIN")]
    claude_bin: Option<PathBuf>,

    /// Optional Claude CLI args (default: `code --json`)
    #[arg(long, env = "CLAUDE_ARGS")]
    claude_args: Option<String>,
}

use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let mut opts = Opts::parse();

    // If no token provided via env/flag, try to read from ~/.openagents/bridge.json
    if opts.ws_token.is_none() {
        match crate::util::read_bridge_token_from_home() {
            Ok(Some(tok)) => {
                opts.ws_token = Some(tok);
            }
            Ok(None) => {
                // No configured token — generate one and persist to ~/.openagents/bridge.json
                let tok = crate::util::generate_bridge_token();
                if let Err(e) = crate::util::write_bridge_token_to_home(&tok) {
                    warn!(?e, "failed to persist generated bridge token; in-memory only");
                }
                opts.ws_token = Some(tok);
                info!("msg" = "generated bridge token (persisted to ~/.openagents/bridge.json)");
            }
            Err(e) => {
                warn!(?e, "failed to read bridge token from config; generating new token");
                let tok = crate::util::generate_bridge_token();
                if let Err(e2) = crate::util::write_bridge_token_to_home(&tok) {
                    warn!(?e2, "failed to persist generated bridge token; in-memory only");
                }
                opts.ws_token = Some(tok);
                info!("msg" = "generated bridge token (persisted to ~/.openagents/bridge.json)");
            }
        }
    }

    // Tinyvex replaces Convex; no external backend management here.

    let (mut child, tx) = codex_runner::spawn_codex(&opts).await?;
    // Initialize Tinyvex database
    let db = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".openagents/tinyvex/data.sqlite3"))
        .unwrap_or_else(|_| std::path::PathBuf::from("tinyvex.sqlite3"));
    if let Some(parent) = db.parent() { let _ = std::fs::create_dir_all(parent); }
    let tinyvex = match tinyvex::Tinyvex::open(&db) {
        Ok(tvx) => std::sync::Arc::new(tvx),
        Err(e) => { return Err(anyhow::anyhow!("tinyvex init failed: {e}")); }
    };
    let state = Arc::new(AppState {
        tx,
        child_stdin: Mutex::new(Some(child.stdin.take().context("child stdin missing")?)),
        child_pid: Mutex::new(Some(child.pid)),
        opts: opts.clone(),
        last_thread_id: Mutex::new(None),
        history: Mutex::new(Vec::new()),
        current_thread_doc: Mutex::new(None),
        stream_track: Mutex::new(std::collections::HashMap::new()),
        pending_user_text: Mutex::new(std::collections::HashMap::new()),
        bridge_ready: std::sync::atomic::AtomicBool::new(true),
        tinyvex: tinyvex.clone(),
        tinyvex_writer: std::sync::Arc::new(tinyvex::Writer::new(tinyvex.clone())),
        sync_enabled: std::sync::atomic::AtomicBool::new(true),
        sync_two_way: std::sync::atomic::AtomicBool::new(false),
        sync_last_read_ms: Mutex::new(0),
        sync_cmd_tx: Mutex::new(None),
        sync_cmd_tx_claude: Mutex::new(None),
        sessions_by_client_doc: Mutex::new(std::collections::HashMap::new()),
        client_doc_by_session: Mutex::new(std::collections::HashMap::new()),
    });


    // Start readers for stdout/stderr → broadcast + console
    crate::ws::start_stream_forwarders(child, state.clone()).await?;

    // Start Codex sessions watcher (inbound sync) by default
    {
        let tx_cmd = crate::watchers::spawn_codex_watcher(state.clone());
        *state.sync_cmd_tx.lock().await = Some(tx_cmd);
        let tx_cmd2 = crate::watchers::spawn_claude_watcher(state.clone());
        *state.sync_cmd_tx_claude.lock().await = Some(tx_cmd2);
    }

    // Watchers removed with Convex; Tinyvex writes occur on JSONL events only.

    // HTTP submit endpoint for app → bridge turn submission
    let app = Router::new()
        .route("/ws", get(crate::ws::ws_handler))
        .with_state(state);

    let bind_addr = opts.bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("binding" = %bind_addr, "msg" = "oa-bridge listening (route: /ws)");
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    // Default to info but quiet down noisy dependencies unless overridden by RUST_LOG
    let default_filter = "info,tungstenite=warn";
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter.into()))
        .with(fmt::layer())
        .try_init();
}
