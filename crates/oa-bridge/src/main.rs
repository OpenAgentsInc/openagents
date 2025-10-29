//! OpenAgents bridge binary entrypoint.
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
use axum::{Router, routing::get};
use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

mod bootstrap;
mod codex_runner;
mod convex_write;
use crate::watchers::{
    sync_projects_to_convex, sync_skills_to_convex, watch_projects_and_sync,
    watch_sessions_and_tail, watch_skills_and_broadcast,
};

mod controls;
mod history;
mod projects;
mod skills;
mod state;
mod util;
mod watchers;
mod ws;
mod acp_codex;
// spool/mirror removed — we write directly to Convex

#[derive(Parser, Debug, Clone)]
#[command(
    name = "oa-bridge",
    about = "OpenAgents WebSocket bridge (spawns agent CLIs, persists to Convex)",
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

    /// Optional WebSocket token required for `/ws` upgrades. When set, incoming
    /// clients must provide a matching token via `Authorization: Bearer` or
    /// `?token=` query parameter. If unset here, we also attempt to load it from
    /// `~/.openagents/bridge.json` (key: `token`).
    #[arg(long, env = "OPENAGENTS_BRIDGE_TOKEN")]
    ws_token: Option<String>,

    /// Use Codex ACP adapter instead of Codex JSONL for Codex integration.
    /// When enabled, the bridge spawns `codex-acp` and communicates over ACP,
    /// mirroring SessionUpdates into Convex.
    #[arg(long, env = "OPENAGENTS_USE_CODEX_ACP", default_value_t = false)]
    use_codex_acp: bool,

    /// Path to the `codex-acp` executable. If unset, resolves from PATH.
    #[arg(long, env = "OPENAGENTS_CODEX_ACP_EXECUTABLE")]
    codex_acp_bin: Option<PathBuf>,
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

    // Start Convex in the background so the WebSocket server can come up immediately.
    if opts.manage_convex {
        let opts_clone = opts.clone();
        tokio::spawn(async move {
            match bootstrap::ensure_convex_running(&opts_clone).await {
                Ok(()) => {
                    info!("msg" = "convex.ensure: healthy (initial)\n");
                }
                Err(e) => {
                    error!(?e, "failed to ensure local Convex is running; proceeding in degraded mode");
                }
            }
        });
    } else {
        info!(
            "msg" = "OPENAGENTS_MANAGE_CONVEX=0 — bridge will not manage Convex; expecting an external process"
        );
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
        current_convex_thread: Mutex::new(None),
        stream_track: Mutex::new(std::collections::HashMap::new()),
        convex_ready: std::sync::atomic::AtomicBool::new(false),
    });

    // Background health watcher: mark convex_ready when healthy and run bootstrap once
    if opts.manage_convex {
        let st = state.clone();
        let opts_for_bootstrap = opts.clone();
        tokio::spawn(async move {
            let url = format!("http://127.0.0.1:{}", st.opts.convex_port);
            let mut tries: u32 = 0;
            loop {
                tries = tries.saturating_add(1);
                if crate::bootstrap::convex_health(&url).await.unwrap_or(false) {
                    st.convex_ready.store(true, std::sync::atomic::Ordering::Relaxed);
                    info!(url=%url, "convex health watcher: ready");
                    if opts_for_bootstrap.bootstrap {
                        if let Err(e) = crate::bootstrap::bootstrap_convex(&opts_for_bootstrap).await {
                            error!(?e, "convex bootstrap failed");
                        }
                    }
                    break;
                }
                if tries % 10 == 0 { info!(attempt=tries, url=%url, "convex health watcher: waiting"); }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        });
    }

    // Start readers for stdout/stderr → broadcast + console
    crate::ws::start_stream_forwarders(child, state.clone()).await?;

    // Live-reload + sync (optional toggle via OPENAGENTS_CONVEX_SYNC=0 to disable)
    // Disable FS→Convex syncing by default to avoid any background churn before the user opts in.
    // Enable explicitly with OPENAGENTS_CONVEX_SYNC=1
    let sync_enabled = std::env::var("OPENAGENTS_CONVEX_SYNC")
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "on"))
        .unwrap_or(false);
    if sync_enabled {
        tokio::spawn(watch_skills_and_broadcast(state.clone()));
        tokio::spawn(watch_projects_and_sync(state.clone()));
        // Initial sync of Projects and Skills (filesystem → Convex)
        {
            let st = state.clone();
            tokio::spawn(async move {
                if let Err(e) = sync_projects_to_convex(st.clone()).await {
                    warn!(?e, "initial projects sync failed");
                }
                if let Err(e) = sync_skills_to_convex(st.clone()).await {
                    warn!(?e, "initial skills sync failed");
                }
            });
        }
        // History mirroring (Codex sessions → Convex) — disabled by default.
        // Enable by setting OPENAGENTS_HISTORY=1.
        let history_enabled = std::env::var("OPENAGENTS_HISTORY")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "on"))
            .unwrap_or(false);
        if history_enabled {
            tokio::spawn(watch_sessions_and_tail(state.clone()));
            tokio::spawn(crate::watchers::enqueue_historical_on_start(state.clone()));
        } else {
            info!("msg" = "OPENAGENTS_HISTORY=0 — history watcher/backfill disabled");
        }
    } else {
        info!("msg" = "OPENAGENTS_CONVEX_SYNC=0 — FS→Convex sync disabled");
    }

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
    let default_filter = "info,convex=warn,convex::base_client=warn,tungstenite=warn";
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter.into()))
        .with(fmt::layer())
        .try_init();
}
