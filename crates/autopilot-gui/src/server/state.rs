//! Shared application state

use serde::Serialize;
use tokio::sync::broadcast;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Broadcast channel for WebSocket messages
    pub ws_tx: broadcast::Sender<WSBroadcast>,
}

/// WebSocket broadcast message
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WSBroadcast {
    #[serde(rename = "apm_updated")]
    APMUpdated {
        avg_apm: f64,
        session_apm: Option<f64>,
    },
    #[serde(rename = "session_started")]
    SessionStarted {
        session_id: String,
    },
    #[serde(rename = "session_completed")]
    SessionCompleted {
        session_id: String,
    },
}

impl AppState {
    /// Create new application state
    pub fn new() -> Self {
        let (ws_tx, _) = broadcast::channel(100);
        Self { ws_tx }
    }

    /// Send APM update to all connected clients
    pub fn broadcast_apm_update(&self, avg_apm: f64, session_apm: Option<f64>) {
        let _ = self.ws_tx.send(WSBroadcast::APMUpdated {
            avg_apm,
            session_apm,
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
