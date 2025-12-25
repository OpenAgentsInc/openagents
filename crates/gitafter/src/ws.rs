//! Broadcast support for real-time Nostr event streaming.

use tokio::sync::broadcast;
use tracing::warn;

/// Broadcasts messages to all subscribers.
pub struct WsBroadcaster {
    tx: broadcast::Sender<String>,
}

impl WsBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Broadcast a message to all connected listeners.
    #[allow(dead_code)]
    pub fn broadcast(&self, msg: &str) -> usize {
        match self.tx.send(msg.to_string()) {
            Ok(receiver_count) => receiver_count,
            Err(e) => {
                // No active receivers isn't an error.
                warn!("Broadcast send failed (no receivers): {}", e);
                0
            }
        }
    }

    /// Subscribe to broadcasts.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}
