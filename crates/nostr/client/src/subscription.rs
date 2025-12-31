//! Subscription management for receiving filtered events from relays

use crate::error::{ClientError, Result};
use nostr::Event;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Callback type for handling received events
pub type EventCallback = Arc<dyn Fn(Event) -> Result<()> + Send + Sync>;

/// A subscription to filtered events from a relay
#[derive(Clone)]
pub struct Subscription {
    /// Subscription ID
    pub id: String,
    /// Filters for this subscription
    pub filters: Vec<Value>,
    /// Whether EOSE (End of Stored Events) has been received
    eose_received: Arc<std::sync::atomic::AtomicBool>,
    /// Event callback
    callback: Option<EventCallback>,
    /// Event channel sender (alternative to callback)
    event_tx: Option<mpsc::Sender<Event>>,
}

impl Subscription {
    /// Create a new subscription with filters
    pub fn new(id: String, filters: Vec<Value>) -> Self {
        Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: None,
            event_tx: None,
        }
    }

    /// Create a subscription with a callback for received events
    pub fn with_callback(id: String, filters: Vec<Value>, callback: EventCallback) -> Self {
        Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: Some(callback),
            event_tx: None,
        }
    }

    /// Create a subscription with a channel for received events
    ///
    /// Uses a bounded channel with a buffer of 1000 events to provide backpressure.
    /// If the consumer is too slow and the buffer fills, the oldest events will be dropped.
    pub fn with_channel(id: String, filters: Vec<Value>) -> (Self, mpsc::Receiver<Event>) {
        // Use bounded channel to prevent unbounded memory growth
        // Buffer size of 1000 events is a reasonable default for most use cases
        let (tx, rx) = mpsc::channel(1000);
        let sub = Self {
            id,
            filters,
            eose_received: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            callback: None,
            event_tx: Some(tx),
        };
        (sub, rx)
    }

    /// Handle a received event for this subscription
    pub fn handle_event(&self, event: Event) -> Result<()> {
        // Call callback if present
        if let Some(callback) = &self.callback {
            callback(event.clone())?;
        }

        // Send to channel if present
        if let Some(tx) = &self.event_tx {
            // Use try_send to avoid blocking when buffer is full
            // This provides backpressure - if consumer is slow, we drop events
            tx.try_send(event).map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => {
                    ClientError::Subscription("Event channel full - consumer too slow".to_string())
                }
                mpsc::error::TrySendError::Closed(_) => {
                    ClientError::Subscription("Event channel closed".to_string())
                }
            })?;
        }

        Ok(())
    }

    /// Mark EOSE as received
    pub fn mark_eose(&self) {
        self.eose_received
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if EOSE has been received
    pub fn has_eose(&self) -> bool {
        self.eose_received
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get subscription ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get filters
    pub fn filters(&self) -> &[Value] {
        &self.filters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event() -> Event {
        // Create a minimal valid event for testing
        Event {
            id: "test123".to_string(),
            pubkey: "pubkey123".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test content".to_string(),
            sig: "sig123".to_string(),
        }
    }

    #[test]
    fn test_subscription_creation() {
        let filters = vec![serde_json::json!({"kinds": [1]})];
        let sub = Subscription::new("test-sub".to_string(), filters.clone());
        assert_eq!(sub.id(), "test-sub");
        assert_eq!(sub.filters().len(), 1);
        assert!(!sub.has_eose());
    }

    #[test]
    fn test_subscription_eose() {
        let filters = vec![serde_json::json!({"kinds": [1]})];
        let sub = Subscription::new("test-sub".to_string(), filters);
        assert!(!sub.has_eose());
        sub.mark_eose();
        assert!(sub.has_eose());
    }

    #[tokio::test]
    async fn test_subscription_with_channel() {
        let filters = vec![serde_json::json!({"kinds": [1]})];
        let (sub, mut rx) = Subscription::with_channel("test-sub".to_string(), filters);

        // Create a test event
        let event = create_test_event();

        // Handle event
        sub.handle_event(event.clone()).unwrap();

        // Receive event from channel
        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "test content");
    }

    #[test]
    fn test_subscription_with_callback() {
        let filters = vec![serde_json::json!({"kinds": [1]})];
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let callback: EventCallback = Arc::new(move |event: Event| {
            received_clone.lock().unwrap().push(event.content.clone());
            Ok(())
        });

        let sub = Subscription::with_callback("test-sub".to_string(), filters, callback);

        // Create a test event
        let event = create_test_event();

        // Handle event
        sub.handle_event(event).unwrap();

        // Check callback was called
        let events = received.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "test content");
    }

    #[test]
    fn test_subscription_clone() {
        let filters = vec![serde_json::json!({"kinds": [1]})];
        let sub1 = Subscription::new("test-sub".to_string(), filters);
        let sub2 = sub1.clone();

        assert_eq!(sub1.id(), sub2.id());
        assert_eq!(sub1.filters().len(), sub2.filters().len());

        // EOSE should be shared
        sub1.mark_eose();
        assert!(sub2.has_eose());
    }
}
