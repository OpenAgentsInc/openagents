//! Mock Nostr Relay for Integration Testing
//!
//! Provides an in-memory Nostr relay implementation for testing Nostr client
//! functionality without requiring a real relay connection.
//!
//! # Example
//!
//! ```no_run
//! use testing::MockRelay;
//!
//! #[tokio::test]
//! async fn test_publish_event() {
//!     let relay = MockRelay::start().await;
//!
//!     // Connect your client to relay.url()
//!     // Publish events and verify they're stored
//!
//!     relay.shutdown().await;
//! }
//! ```

use anyhow::Result;
use nostr::Event;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter as WarpFilter;

/// Mock Nostr relay for testing
pub struct MockRelay {
    /// URL of the mock relay (ws://127.0.0.1:PORT)
    url: String,
    /// Shared state between WebSocket handlers
    state: Arc<RelayState>,
    /// Server task handle
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Default)]
struct RelayState {
    /// All events stored in the relay (id -> event)
    events: RwLock<HashMap<String, Event>>,
    /// Active subscriptions (subscription_id -> filter JSON)
    subscriptions: RwLock<HashMap<String, serde_json::Value>>,
}

impl MockRelay {
    /// Start a new mock relay on a random available port
    pub async fn start() -> Self {
        Self::start_on_port(0).await
    }

    /// Start a new mock relay on a specific port (0 = random)
    pub async fn start_on_port(port: u16) -> Self {
        let state = Arc::new(RelayState::default());
        let state_clone = state.clone();
        let state_filter = warp::any().map(move || state_clone.clone());

        // WebSocket route
        let ws_route = warp::path::end().and(warp::ws()).and(state_filter).map(
            |ws: warp::ws::Ws, state: Arc<RelayState>| {
                ws.on_upgrade(move |socket| handle_connection(socket, state))
            },
        );

        // For port 0, we need a unique test port
        let actual_port = if port == 0 {
            // Use a random high port for testing
            use std::sync::atomic::{AtomicU16, Ordering};
            static PORT_COUNTER: AtomicU16 = AtomicU16::new(9000);
            PORT_COUNTER.fetch_add(1, Ordering::SeqCst)
        } else {
            port
        };

        // Bind to port
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], actual_port).into();
        let server = warp::serve(ws_route);

        // Try to bind to the address
        let server_handle = tokio::spawn(async move {
            server.run(addr).await;
        });

        let url = format!("ws://127.0.0.1:{}", actual_port);

        // Give server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        Self {
            url,
            state: state.clone(),
            server_handle: Some(server_handle),
        }
    }

    /// Get the WebSocket URL for this relay
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get all events stored in the relay
    pub async fn get_events(&self) -> Vec<Event> {
        self.state.events.read().await.values().cloned().collect()
    }

    /// Get events by kind
    pub async fn get_events_by_kind(&self, kind: u16) -> Vec<Event> {
        self.state
            .events
            .read()
            .await
            .values()
            .filter(|e| e.kind == kind)
            .cloned()
            .collect()
    }

    /// Get events by author pubkey
    pub async fn get_events_by_author(&self, pubkey: &str) -> Vec<Event> {
        self.state
            .events
            .read()
            .await
            .values()
            .filter(|e| e.pubkey == pubkey)
            .cloned()
            .collect()
    }

    /// Get event by ID
    pub async fn get_event(&self, id: &str) -> Option<Event> {
        self.state.events.read().await.get(id).cloned()
    }

    /// Clear all events
    pub async fn clear_events(&self) {
        self.state.events.write().await.clear();
    }

    /// Get count of stored events
    pub async fn event_count(&self) -> usize {
        self.state.events.read().await.len()
    }

    /// Store an event directly (for testing)
    ///
    /// This bypasses WebSocket publishing and stores the event directly.
    /// Useful for test setup.
    pub async fn store_event(&self, event: Event) {
        self.state
            .events
            .write()
            .await
            .insert(event.id.clone(), event);
    }

    /// Shutdown the relay gracefully
    ///
    /// Aborts the server task and waits for cleanup.
    pub async fn shutdown(mut self) {
        if let Some(handle) = self.server_handle.take() {
            // Abort the server task
            handle.abort();

            // Wait for the task to finish (will return immediately with abort error)
            let _ = handle.await;
        }
    }
}

async fn handle_connection(ws: warp::ws::WebSocket, state: Arc<RelayState>) {
    use futures_util::{SinkExt, StreamExt};

    let (mut tx, mut rx) = ws.split();

    while let Some(result) = rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str()
                    && let Ok(response) = handle_message(text, &state).await
                    && let Some(resp_msg) = response
                {
                    let _ = tx.send(warp::ws::Message::text(resp_msg)).await;
                }
            }
            Err(_) => break,
        }
    }
}

async fn handle_message(text: &str, state: &Arc<RelayState>) -> Result<Option<String>> {
    let msg: serde_json::Value = serde_json::from_str(text)?;

    let msg_type = msg.get(0).and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "EVENT" => {
            // Client publishing an event
            if let Some(event_value) = msg.get(1) {
                let event: Event = serde_json::from_value(event_value.clone())?;

                // Verify event signature
                if let Ok(false) | Err(_) = nostr::verify_event(&event) {
                    return Ok(Some(
                        json!([
                            "OK",
                            event.id,
                            false,
                            "invalid: signature verification failed"
                        ])
                        .to_string(),
                    ));
                }

                // Store event
                state
                    .events
                    .write()
                    .await
                    .insert(event.id.clone(), event.clone());

                // Send OK response
                return Ok(Some(json!(["OK", event.id, true, ""]).to_string()));
            }
        }
        "REQ" => {
            // Client subscribing
            if let Some(sub_id) = msg.get(1).and_then(|v| v.as_str()) {
                // Store filters as JSON (we'll do basic matching)
                let filters = msg.as_array().map(|arr| &arr[2..]).unwrap_or(&[]);

                // Store subscription
                state
                    .subscriptions
                    .write()
                    .await
                    .insert(sub_id.to_string(), json!(filters));

                // Send EOSE (in real impl, would send matching events first)
                return Ok(Some(json!(["EOSE", sub_id]).to_string()));
            }
        }
        "CLOSE" => {
            // Client closing subscription
            if let Some(sub_id) = msg.get(1).and_then(|v| v.as_str()) {
                state.subscriptions.write().await.remove(sub_id);
            }
        }
        _ => {}
    }

    Ok(None)
}

// Helper function for future filter matching implementation
// For now, subscriptions just trigger EOSE
#[allow(dead_code)]
fn event_matches_kind(event: &Event, kind: u16) -> bool {
    event.kind == kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{EventTemplate, KIND_SHORT_TEXT_NOTE, finalize_event, generate_secret_key};

    #[tokio::test]
    async fn test_relay_starts_and_stops() {
        let relay = MockRelay::start().await;
        assert!(relay.url().starts_with("ws://127.0.0.1:"));
        relay.shutdown().await;
    }

    #[tokio::test]
    async fn test_relay_stores_events() {
        let relay = MockRelay::start().await;

        // Create test event
        let sk = generate_secret_key();
        let template = EventTemplate {
            kind: KIND_SHORT_TEXT_NOTE,
            tags: vec![],
            content: "test message".to_string(),
            created_at: 1234567890,
        };
        let event = finalize_event(&template, &sk).unwrap();

        // Store using relay helper
        relay.store_event(event.clone()).await;

        // Verify
        assert_eq!(relay.event_count().await, 1);
        assert_eq!(relay.get_event(&event.id).await, Some(event.clone()));

        relay.shutdown().await;
    }

    #[tokio::test]
    async fn test_get_events_by_kind() {
        let relay = MockRelay::start().await;

        let sk = generate_secret_key();

        // Create events of different kinds
        let template1 = EventTemplate {
            kind: 1,
            tags: vec![],
            content: "note 1".to_string(),
            created_at: 1234567890,
        };
        let template2 = EventTemplate {
            kind: 1,
            tags: vec![],
            content: "note 2".to_string(),
            created_at: 1234567891,
        };
        let template3 = EventTemplate {
            kind: 3,
            tags: vec![],
            content: "contacts".to_string(),
            created_at: 1234567892,
        };

        let event1 = finalize_event(&template1, &sk).unwrap();
        let event2 = finalize_event(&template2, &sk).unwrap();
        let event3 = finalize_event(&template3, &sk).unwrap();

        relay
            .state
            .events
            .write()
            .await
            .insert(event1.id.clone(), event1.clone());
        relay
            .state
            .events
            .write()
            .await
            .insert(event2.id.clone(), event2.clone());
        relay
            .state
            .events
            .write()
            .await
            .insert(event3.id.clone(), event3.clone());

        let kind1_events = relay.get_events_by_kind(1).await;
        assert_eq!(kind1_events.len(), 2);

        let kind3_events = relay.get_events_by_kind(3).await;
        assert_eq!(kind3_events.len(), 1);

        relay.shutdown().await;
    }
}
