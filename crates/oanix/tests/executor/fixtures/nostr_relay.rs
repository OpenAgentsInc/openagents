//! Mock Nostr relay implementing NIP-01 protocol for testing

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// Mock Nostr relay implementing NIP-01 protocol
///
/// Handles:
/// - `["EVENT", event]` - Store event, respond with `["OK", id, true, ""]`
/// - `["REQ", sub_id, ...filters]` - Store subscription, respond with `["EOSE", sub_id]`
/// - `["CLOSE", sub_id]` - Remove subscription
pub struct NostrMockRelay {
    addr: SocketAddr,
    shutdown_tx: broadcast::Sender<()>,
    handle: JoinHandle<()>,
    /// Events received from clients
    events: Arc<RwLock<HashMap<String, Value>>>,
    /// Active subscriptions: sub_id -> filters
    subscriptions: Arc<RwLock<HashMap<String, Vec<Value>>>>,
    /// Messages to push to subscribers when they connect
    pending_events: Arc<RwLock<Vec<Value>>>,
}

impl NostrMockRelay {
    /// Start a new mock relay on a random port
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let mut shutdown_rx = shutdown_tx.subscribe();

        let events = Arc::new(RwLock::new(HashMap::new()));
        let subscriptions = Arc::new(RwLock::new(HashMap::new()));
        let pending_events = Arc::new(RwLock::new(Vec::new()));

        let events_clone = Arc::clone(&events);
        let subs_clone = Arc::clone(&subscriptions);
        let pending_clone = Arc::clone(&pending_events);

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    result = listener.accept() => {
                        if let Ok((stream, _)) = result {
                            let events = Arc::clone(&events_clone);
                            let subs = Arc::clone(&subs_clone);
                            let pending = Arc::clone(&pending_clone);
                            tokio::spawn(Self::handle_connection(stream, events, subs, pending));
                        }
                    }
                }
            }
        });

        Self {
            addr,
            shutdown_tx,
            handle,
            events,
            subscriptions,
            pending_events,
        }
    }

    /// Handle a single WebSocket connection (NIP-01 protocol)
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        events: Arc<RwLock<HashMap<String, Value>>>,
        subscriptions: Arc<RwLock<HashMap<String, Vec<Value>>>>,
        pending_events: Arc<RwLock<Vec<Value>>>,
    ) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(_) => return,
        };

        let (mut write, mut read) = ws_stream.split();

        // Track this connection's subscriptions
        let mut local_subs: Vec<String> = Vec::new();

        while let Some(result) = read.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    if let Some(responses) = Self::process_message(
                        &text,
                        &events,
                        &subscriptions,
                        &pending_events,
                        &mut local_subs,
                    )
                    .await
                    {
                        for response in responses {
                            let _ = write.send(Message::Text(response.into())).await;
                        }
                    }
                }
                Ok(Message::Binary(data)) => {
                    // Also handle binary messages (WsConnector sends as binary)
                    if let Ok(text) = String::from_utf8(data.to_vec()) {
                        if let Some(responses) = Self::process_message(
                            &text,
                            &events,
                            &subscriptions,
                            &pending_events,
                            &mut local_subs,
                        )
                        .await
                        {
                            for response in responses {
                                let _ = write.send(Message::Text(response.into())).await;
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = write.send(Message::Close(None)).await;
                    break;
                }
                Ok(Message::Ping(data)) => {
                    let _ = write.send(Message::Pong(data)).await;
                }
                _ => {}
            }
        }

        // Clean up subscriptions for this connection
        let mut subs = subscriptions.write().await;
        for sub_id in local_subs {
            subs.remove(&sub_id);
        }
    }

    /// Process a NIP-01 message and return responses
    async fn process_message(
        message: &str,
        events: &Arc<RwLock<HashMap<String, Value>>>,
        subscriptions: &Arc<RwLock<HashMap<String, Vec<Value>>>>,
        pending_events: &Arc<RwLock<Vec<Value>>>,
        local_subs: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        let value: Value = serde_json::from_str(message).ok()?;
        let array = value.as_array()?;

        if array.is_empty() {
            return None;
        }

        let msg_type = array[0].as_str()?;
        let mut responses = Vec::new();

        match msg_type {
            "EVENT" => {
                // ["EVENT", event]
                if let Some(event) = array.get(1) {
                    let event_id = event["id"].as_str().unwrap_or("unknown").to_string();

                    // Store event
                    events.write().await.insert(event_id.clone(), event.clone());

                    // Send OK response
                    responses.push(json!(["OK", event_id, true, ""]).to_string());
                }
            }
            "REQ" => {
                // ["REQ", sub_id, filter1, filter2, ...]
                if let Some(sub_id) = array.get(1).and_then(|v| v.as_str()) {
                    let filters: Vec<Value> = array[2..].to_vec();
                    subscriptions
                        .write()
                        .await
                        .insert(sub_id.to_string(), filters);
                    local_subs.push(sub_id.to_string());

                    // Send any pending events that match this subscription
                    let pending = pending_events.read().await;
                    for event in pending.iter() {
                        responses.push(json!(["EVENT", sub_id, event]).to_string());
                    }

                    // Send EOSE (end of stored events)
                    responses.push(json!(["EOSE", sub_id]).to_string());
                }
            }
            "CLOSE" => {
                // ["CLOSE", sub_id]
                if let Some(sub_id) = array.get(1).and_then(|v| v.as_str()) {
                    subscriptions.write().await.remove(sub_id);
                    local_subs.retain(|s| s != sub_id);

                    // Send CLOSED response
                    responses.push(json!(["CLOSED", sub_id, ""]).to_string());
                }
            }
            _ => {
                // Unknown message type, ignore
            }
        }

        if responses.is_empty() {
            None
        } else {
            Some(responses)
        }
    }

    /// Get the WebSocket URL (ws://...)
    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    /// Get all events received by the relay
    pub async fn received_events(&self) -> Vec<Value> {
        self.events.read().await.values().cloned().collect()
    }

    /// Get a specific event by ID
    pub async fn get_event(&self, id: &str) -> Option<Value> {
        self.events.read().await.get(id).cloned()
    }

    /// Get active subscriptions
    pub async fn active_subscriptions(&self) -> Vec<String> {
        self.subscriptions.read().await.keys().cloned().collect()
    }

    /// Inject an event into the relay (simulates receiving from another client)
    /// This event will be sent to matching subscribers
    pub async fn inject_event(&self, event: Value) {
        self.pending_events.write().await.push(event);
    }

    /// Clear all received events
    pub async fn clear_events(&self) {
        self.events.write().await.clear();
    }

    /// Shutdown the relay
    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.handle.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;

    #[tokio::test]
    async fn test_nostr_mock_relay_starts() {
        let relay = NostrMockRelay::start().await;
        assert!(relay.url().starts_with("ws://"));
        relay.shutdown().await;
    }

    #[tokio::test]
    async fn test_nostr_mock_relay_handles_event() {
        let relay = NostrMockRelay::start().await;

        let (ws_stream, _) = connect_async(relay.url()).await.unwrap();
        let (mut write, mut read) = ws_stream.split();

        // Send an EVENT
        let event = json!({
            "id": "abc123",
            "pubkey": "def456",
            "created_at": 1700000000,
            "kind": 1,
            "tags": [],
            "content": "Hello Nostr!",
            "sig": "sig123"
        });
        let msg = json!(["EVENT", event]);
        write
            .send(Message::Text(msg.to_string().into()))
            .await
            .unwrap();

        // Wait for OK response
        if let Some(Ok(Message::Text(text))) = read.next().await {
            let response: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(response[0], "OK");
            assert_eq!(response[1], "abc123");
            assert_eq!(response[2], true);
        } else {
            panic!("Expected OK response");
        }

        // Verify event was stored
        assert!(relay.get_event("abc123").await.is_some());

        relay.shutdown().await;
    }

    #[tokio::test]
    async fn test_nostr_mock_relay_handles_req() {
        let relay = NostrMockRelay::start().await;

        let (ws_stream, _) = connect_async(relay.url()).await.unwrap();
        let (mut write, mut read) = ws_stream.split();

        // Send a REQ
        let filter = json!({"kinds": [1]});
        let msg = json!(["REQ", "sub1", filter]);
        write
            .send(Message::Text(msg.to_string().into()))
            .await
            .unwrap();

        // Wait for EOSE response
        if let Some(Ok(Message::Text(text))) = read.next().await {
            let response: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(response[0], "EOSE");
            assert_eq!(response[1], "sub1");
        } else {
            panic!("Expected EOSE response");
        }

        // Verify subscription was stored
        let subs = relay.active_subscriptions().await;
        assert!(subs.contains(&"sub1".to_string()));

        relay.shutdown().await;
    }
}
