//! Shared application state for the WebSocket bridge.
//!
//! The `AppState` struct holds all mutable, cross-task state needed by the
//! bridge: broadcast channels for UI clients, child process I/O handles,
//! last-seen thread id for resume support, a bounded history buffer for
//! late-joining WS clients, and per-thread streaming aggregation state.
//!
//! Splitting this into its own module allows the WS server, codex runner,
//! and Tinyvex write helpers to operate over a common type without circular
//! dependencies or forcing everything into a monolithic main.rs.

use std::collections::HashMap;

use tokio::sync::{Mutex, broadcast};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Opts;

/// Tracks streaming item aggregation for a single (thread, kind) pair.
/// Used to upsert deltas and finalize once a full item text is known.
#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub item_id: String,
    pub last_text: String,
    pub seq: u64,
}

/// Global bridge state shared across WS handlers and background tasks.
pub struct AppState {
    pub tx: broadcast::Sender<String>,
    #[allow(dead_code)]
    pub child_stdin: Mutex<Option<tokio::process::ChildStdin>>, // drop after first write to signal EOF
    pub child_pid: Mutex<Option<u32>>,
    pub opts: Opts,
    // Track last seen session id so we can resume on subsequent prompts
    pub last_thread_id: Mutex<Option<String>>,
    // Replay buffer for new websocket clients
    pub history: Mutex<Vec<String>>,
    // Current thread doc id being processed (for mapping thread.started -> client threadId)
    pub current_thread_doc: Mutex<Option<String>>,
    // Streaming message trackers (per thread, per kind). Key: "<threadId>|assistant" or "<threadId>|reason".
    pub stream_track: Mutex<HashMap<String, StreamEntry>>,
    // Pending user message text keyed by client thread doc id (to emit ACP once session id is known)
    pub pending_user_text: Mutex<HashMap<String, String>>,
    // Legacy: formerly Convex readiness; always true in Tinyvex-only mode
    #[allow(dead_code)]
    pub bridge_ready: AtomicBool,
    // Tinyvex database (mandatory)
    pub tinyvex: std::sync::Arc<tinyvex::Tinyvex>,
    // Map client thread doc id -> last known session id (for per-thread resume)
    pub sessions_by_client_doc: Mutex<HashMap<String, String>>,
}

impl AppState {
    #[allow(dead_code)]
    pub fn is_bridge_ready(&self) -> bool {
        self.bridge_ready.load(Ordering::Relaxed)
    }
}
