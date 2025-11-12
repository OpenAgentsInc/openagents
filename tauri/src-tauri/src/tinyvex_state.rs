//! Shared application state for the tinyvex WebSocket server.
//!
//! Simplified version for Tauri integration - focuses on core tinyvex
//! functionality without the full oa-bridge complexity.

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Global state shared across WebSocket handlers and the tinyvex system.
pub struct TinyvexState {
    /// Broadcast channel for pushing updates to all connected WebSocket clients
    pub tx: broadcast::Sender<String>,

    /// Replay buffer for late-joining WebSocket clients (last N messages)
    pub history: Mutex<Vec<String>>,

    /// Tinyvex database instance
    pub tinyvex: Arc<tinyvex::Tinyvex>,

    /// Tinyvex writer for converting ACP updates to DB writes + notifications
    pub tinyvex_writer: Arc<tinyvex::Writer>,
}

impl TinyvexState {
    pub fn new(tinyvex: Arc<tinyvex::Tinyvex>, tinyvex_writer: Arc<tinyvex::Writer>) -> Self {
        let (tx, _rx) = broadcast::channel(1000);
        Self {
            tx,
            history: Mutex::new(Vec::new()),
            tinyvex,
            tinyvex_writer,
        }
    }

    /// Add a message to history buffer (keep last 100 messages for replay)
    pub async fn add_to_history(&self, msg: String) {
        let mut history = self.history.lock().await;
        history.push(msg);
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// Get history for replay to new clients
    pub async fn get_history(&self) -> Vec<String> {
        self.history.lock().await.clone()
    }

    /// Broadcast a message to all connected clients and add to history
    pub async fn broadcast(&self, msg: String) {
        self.add_to_history(msg.clone()).await;
        let _ = self.tx.send(msg);
    }
}
