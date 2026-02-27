//! Subscription management for receiving filtered events.

use crate::error::{ClientError, Result};
use nostr::Event;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Callback type for handling received events.
pub type EventCallback = Arc<dyn Fn(Event) -> Result<()> + Send + Sync>;

/// A subscription to filtered events from a relay.
#[derive(Clone)]
pub struct Subscription {
    /// Subscription ID.
    pub id: String,
    /// Filters for this subscription.
    pub filters: Vec<Value>,
    eose_received: Arc<std::sync::atomic::AtomicBool>,
    callback: Option<EventCallback>,
    event_tx: Option<mpsc::Sender<Event>>,
}

impl Subscription {
    /// Create a new subscription with filters.
    pub fn new(id: String, filters: Vec<Value>) -> Self {
        Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: None,
            event_tx: None,
        }
    }

    /// Create a subscription with callback-based event handling.
    pub fn with_callback(id: String, filters: Vec<Value>, callback: EventCallback) -> Self {
        Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: Some(callback),
            event_tx: None,
        }
    }

    /// Create a subscription that receives events on a bounded channel.
    pub fn with_channel(id: String, filters: Vec<Value>) -> (Self, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel(1000);
        let subscription = Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: None,
            event_tx: Some(tx),
        };
        (subscription, rx)
    }

    /// Handle a received event.
    pub fn handle_event(&self, event: Event) -> Result<()> {
        if let Some(callback) = &self.callback {
            callback(event.clone())?;
        }

        if let Some(tx) = &self.event_tx {
            tx.try_send(event).map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    ClientError::Subscription("event channel full - consumer too slow".to_string())
                }
                mpsc::error::TrySendError::Closed(_) => {
                    ClientError::Subscription("event channel closed".to_string())
                }
            })?;
        }

        Ok(())
    }

    /// Mark EOSE as received.
    pub fn mark_eose(&self) {
        self.eose_received
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if EOSE has been received.
    pub fn has_eose(&self) -> bool {
        self.eose_received
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
