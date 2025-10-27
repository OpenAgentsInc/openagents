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
                        info!(
                            "msg" = "OPENAGENTS_BOOTSTRAP disabled — skipping convex function push"
                        );
                    }
                }
                Err(e) => {
                    error!(
                        ?e,
                        "failed to ensure local Convex is running; skipping bootstrap"
                    );
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
    });

    // Start readers for stdout/stderr → broadcast + console
    crate::ws::start_stream_forwarders(child, state.clone()).await?;

    // Live-reload + sync (optional toggle via OPENAGENTS_CONVEX_SYNC=0 to disable)
    let sync_enabled = std::env::var("OPENAGENTS_CONVEX_SYNC")
        .ok()
        .map(|v| v != "0")
        .unwrap_or(true);
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
        // Watch Codex sessions for external runs and mirror to Convex (best-effort)
        tokio::spawn(watch_sessions_and_tail(state.clone()));
        // Background: enqueue historical threads/messages to the mirror spool on startup
        tokio::spawn(crate::watchers::enqueue_historical_on_start(state.clone()));
    } else {
        info!("msg" = "OPENAGENTS_CONVEX_SYNC=0 — FS→Convex sync disabled");
    }

    // HTTP submit endpoint for app → bridge turn submission
    let app = Router::new()
        .route("/ws", get(crate::ws::ws_handler))
        .with_state(state);

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
