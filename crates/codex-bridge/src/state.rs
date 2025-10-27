//! Shared application state for the WebSocket bridge.
//!
//! The `AppState` struct holds all mutable, cross-task state needed by the
//! bridge: broadcast channels for UI clients, child process I/O handles,
//! last-seen thread id for resume support, a bounded history buffer for
//! late-joining WS clients, and per-thread streaming aggregation state.
//!
//! Splitting this into its own module allows the WS server, codex runner,
//! and Convex write helpers to operate over a common type without circular
//! dependencies or forcing everything into a monolithic main.rs.

use std::collections::HashMap;

use tokio::sync::{Mutex, broadcast};

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
    pub child_stdin: Mutex<Option<tokio::process::ChildStdin>>, // drop after first write to signal EOF
    pub child_pid: Mutex<Option<u32>>,
    pub opts: Opts,
    // Track last seen session id so we can resume on subsequent prompts
    pub last_thread_id: Mutex<Option<String>>,
    // Replay buffer for new websocket clients
    pub history: Mutex<Vec<String>>,
    // Current Convex thread doc id being processed (for mapping thread.started -> Convex threadId)
    pub current_convex_thread: Mutex<Option<String>>,
    // Streaming message trackers (per thread, per kind). Key: "<threadId>|assistant" or "<threadId>|reason".
    pub stream_track: Mutex<HashMap<String, StreamEntry>>,
}
