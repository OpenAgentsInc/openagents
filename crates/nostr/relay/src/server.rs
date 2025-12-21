//! WebSocket server for handling client connections
//!
//! The relay server accepts WebSocket connections and handles Nostr protocol messages:
//! - EVENT: Store events
//! - REQ: Subscribe to events matching filters
//! - CLOSE: Close a subscription
//!
//! Messages are JSON arrays following the Nostr protocol specification.

use crate::db::Database;
use crate::error::{RelayError, Result};
use crate::subscription::{Filter, SubscriptionManager};
use futures::{SinkExt, StreamExt};
use nostr::Event;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Relay server configuration
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Bind address for the WebSocket server
    pub bind_addr: SocketAddr,
    /// Maximum message size in bytes
    pub max_message_size: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:7000".parse().unwrap(),
            max_message_size: 512 * 1024, // 512 KB
        }
    }
}

/// Nostr relay server
pub struct RelayServer {
    config: RelayConfig,
    db: Arc<Database>,
}

impl RelayServer {
    /// Create a new relay server
    pub fn new(config: RelayConfig, db: Database) -> Self {
        Self {
            config,
            db: Arc::new(db),
        }
    }

    /// Start the relay server
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        info!("Relay server listening on {}", self.config.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New connection from {}", addr);
                    let db = Arc::clone(&self.db);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr, db).await {
                            error!("Error handling connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }
}

/// Handle a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    db: Arc<Database>,
) -> Result<()> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| RelayError::WebSocket(e.to_string()))?;

    info!("WebSocket connection established: {}", addr);

    let (mut write, mut read) = ws_stream.split();
    let mut subscriptions = SubscriptionManager::new();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received message from {}: {}", addr, text);

                // Parse the Nostr message
                match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        let responses = handle_nostr_message(&value, &db, &mut subscriptions).await;
                        for response in responses {
                            let response_text = serde_json::to_string(&response)?;
                            if let Err(e) = write.send(Message::Text(response_text)).await {
                                error!("Failed to send response to {}: {}", addr, e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Invalid JSON from {}: {}", addr, e);
                        let notice = json!(["NOTICE", format!("Invalid JSON: {}", e)]);
                        let _ = write.send(Message::Text(notice.to_string())).await;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("Client {} disconnected", addr);
                break;
            }
            Ok(Message::Ping(data)) => {
                debug!("Ping from {}", addr);
                let _ = write.send(Message::Pong(data)).await;
            }
            Err(e) => {
                error!("WebSocket error from {}: {}", addr, e);
                break;
            }
            _ => {}
        }
    }

    info!("Connection closed: {}", addr);
    Ok(())
}

/// Handle a Nostr protocol message, returns multiple responses
async fn handle_nostr_message(
    msg: &Value,
    db: &Database,
    subscriptions: &mut SubscriptionManager,
) -> Vec<Value> {
    let mut responses = Vec::new();

    let msg_array = match msg.as_array() {
        Some(arr) => arr,
        None => {
            responses.push(json!(["NOTICE", "Message must be an array"]));
            return responses;
        }
    };

    if msg_array.is_empty() {
        responses.push(json!(["NOTICE", "Empty message"]));
        return responses;
    }

    let msg_type = match msg_array[0].as_str() {
        Some(t) => t,
        None => {
            responses.push(json!(["NOTICE", "Invalid message type"]));
            return responses;
        }
    };

    match msg_type {
        "EVENT" => {
            // ["EVENT", <event JSON>]
            if msg_array.len() < 2 {
                responses.push(json!(["NOTICE", "EVENT message missing event data"]));
                return responses;
            }

            match serde_json::from_value::<Event>(msg_array[1].clone()) {
                Ok(event) => {
                    // Validate event signature
                    #[cfg(feature = "full")]
                    {
                        if let Err(e) = nostr::verify_event(&event) {
                            responses.push(json!(["OK", event.id, false, format!("invalid: {}", e)]));
                            return responses;
                        }
                    }

                    // Store the event
                    match db.store_event(&event) {
                        Ok(_) => {
                            debug!("Stored event: {}", event.id);
                            responses.push(json!(["OK", event.id, true, ""]));
                        }
                        Err(e) => {
                            error!("Failed to store event {}: {}", event.id, e);
                            responses.push(json!(["OK", event.id, false, format!("error: {}", e)]));
                        }
                    }
                }
                Err(e) => {
                    warn!("Invalid event: {}", e);
                    responses.push(json!(["NOTICE", format!("Invalid event: {}", e)]));
                }
            }
        }
        "REQ" => {
            // ["REQ", <subscription_id>, <filters JSON>...]
            if msg_array.len() < 2 {
                responses.push(json!(["NOTICE", "REQ message missing subscription ID"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NOTICE", "Invalid subscription ID"]));
                    return responses;
                }
            };

            // Parse filters
            let mut filters = Vec::new();
            for i in 2..msg_array.len() {
                match serde_json::from_value::<Filter>(msg_array[i].clone()) {
                    Ok(filter) => {
                        if let Err(e) = filter.validate() {
                            responses.push(json!(["NOTICE", format!("Invalid filter: {}", e)]));
                            return responses;
                        }
                        filters.push(filter);
                    }
                    Err(e) => {
                        responses.push(json!(["NOTICE", format!("Failed to parse filter: {}", e)]));
                        return responses;
                    }
                }
            }

            if filters.is_empty() {
                responses.push(json!(["NOTICE", "REQ message missing filters"]));
                return responses;
            }

            debug!("Subscription requested: {} with {} filters", sub_id, filters.len());

            // Store subscription
            let subscription = crate::subscription::Subscription::new(sub_id.to_string(), filters.clone());
            subscriptions.add(subscription);

            // Query and send matching events for each filter
            for filter in &filters {
                match db.query_events(filter) {
                    Ok(events) => {
                        debug!("Found {} matching events for subscription {}", events.len(), sub_id);
                        for event in events {
                            // Send EVENT message: ["EVENT", <subscription_id>, <event JSON>]
                            responses.push(json!(["EVENT", sub_id, event]));
                        }
                    }
                    Err(e) => {
                        error!("Failed to query events for subscription {}: {}", sub_id, e);
                        responses.push(json!(["NOTICE", format!("Error querying events: {}", e)]));
                    }
                }
            }

            // Send EOSE (End of Stored Events)
            responses.push(json!(["EOSE", sub_id]));
        }
        "CLOSE" => {
            // ["CLOSE", <subscription_id>]
            if msg_array.len() < 2 {
                responses.push(json!(["NOTICE", "CLOSE message missing subscription ID"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NOTICE", "Invalid subscription ID"]));
                    return responses;
                }
            };

            if subscriptions.remove(sub_id) {
                debug!("Subscription closed: {}", sub_id);
            } else {
                debug!("Attempted to close non-existent subscription: {}", sub_id);
            }
            // No response needed for CLOSE
        }
        _ => {
            warn!("Unknown message type: {}", msg_type);
            responses.push(json!(["NOTICE", format!("Unknown message type: {}", msg_type)]));
        }
    }

    responses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, DatabaseConfig};

    #[tokio::test]
    async fn test_handle_event_message() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let config = DatabaseConfig {
            path: db_path,
            ..Default::default()
        };
        let db = Database::new(config).unwrap();
        let mut subs = SubscriptionManager::new();

        // Create a test event (simplified - in real use would use nostr crate)
        let event_json = json!({
            "id": "test_id_123",
            "pubkey": "test_pubkey",
            "created_at": 1234567890,
            "kind": 1,
            "tags": [],
            "content": "Hello, world!",
            "sig": "test_sig"
        });

        let msg = json!(["EVENT", event_json]);
        let responses = handle_nostr_message(&msg, &db, &mut subs).await;

        assert!(!responses.is_empty());
        let resp = &responses[0];
        assert_eq!(resp[0], "OK");
    }

    #[tokio::test]
    async fn test_handle_req_message() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let config = DatabaseConfig {
            path: db_path,
            ..Default::default()
        };
        let db = Database::new(config).unwrap();
        let mut subs = SubscriptionManager::new();

        let msg = json!(["REQ", "sub_123", {"kinds": [1]}]);
        let responses = handle_nostr_message(&msg, &db, &mut subs).await;

        assert!(!responses.is_empty());
        // Last response should be EOSE
        let last_resp = responses.last().unwrap();
        assert_eq!(last_resp[0], "EOSE");
        assert_eq!(last_resp[1], "sub_123");

        // Subscription should be tracked
        assert_eq!(subs.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_close_message() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let config = DatabaseConfig {
            path: db_path,
            ..Default::default()
        };
        let db = Database::new(config).unwrap();
        let mut subs = SubscriptionManager::new();

        // First create a subscription
        let msg1 = json!(["REQ", "sub_123", {"kinds": [1]}]);
        handle_nostr_message(&msg1, &db, &mut subs).await;
        assert_eq!(subs.len(), 1);

        // Now close it
        let msg2 = json!(["CLOSE", "sub_123"]);
        let responses = handle_nostr_message(&msg2, &db, &mut subs).await;

        assert!(responses.is_empty()); // CLOSE doesn't send responses
        assert_eq!(subs.len(), 0); // Subscription removed
    }

    #[tokio::test]
    async fn test_handle_invalid_message() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let config = DatabaseConfig {
            path: db_path,
            ..Default::default()
        };
        let db = Database::new(config).unwrap();
        let mut subs = SubscriptionManager::new();

        let msg = json!(["UNKNOWN", "data"]);
        let responses = handle_nostr_message(&msg, &db, &mut subs).await;

        assert!(!responses.is_empty());
        let resp = &responses[0];
        assert_eq!(resp[0], "NOTICE");
    }
}
