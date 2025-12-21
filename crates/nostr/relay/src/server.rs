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

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received message from {}: {}", addr, text);

                // Parse the Nostr message
                match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        if let Some(response) = handle_nostr_message(&value, &db).await {
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

/// Handle a Nostr protocol message
async fn handle_nostr_message(msg: &Value, db: &Database) -> Option<Value> {
    let msg_array = msg.as_array()?;
    if msg_array.is_empty() {
        return Some(json!(["NOTICE", "Empty message"]));
    }

    let msg_type = msg_array[0].as_str()?;

    match msg_type {
        "EVENT" => {
            // ["EVENT", <event JSON>]
            if msg_array.len() < 2 {
                return Some(json!(["NOTICE", "EVENT message missing event data"]));
            }

            match serde_json::from_value::<Event>(msg_array[1].clone()) {
                Ok(event) => {
                    // Validate event signature
                    #[cfg(feature = "full")]
                    {
                        if let Err(e) = nostr::verify_event(&event) {
                            return Some(json!(["OK", event.id, false, format!("invalid: {}", e)]));
                        }
                    }

                    // Store the event
                    match db.store_event(&event) {
                        Ok(_) => {
                            debug!("Stored event: {}", event.id);
                            Some(json!(["OK", event.id, true, ""]))
                        }
                        Err(e) => {
                            error!("Failed to store event {}: {}", event.id, e);
                            Some(json!(["OK", event.id, false, format!("error: {}", e)]))
                        }
                    }
                }
                Err(e) => {
                    warn!("Invalid event: {}", e);
                    Some(json!(["NOTICE", format!("Invalid event: {}", e)]))
                }
            }
        }
        "REQ" => {
            // ["REQ", <subscription_id>, <filters JSON>...]
            // For now, just acknowledge the subscription
            // Full implementation would manage subscriptions and send matching events
            if msg_array.len() < 2 {
                return Some(json!(["NOTICE", "REQ message missing subscription ID"]));
            }

            let sub_id = msg_array[1].as_str()?;
            debug!("Subscription requested: {}", sub_id);

            // Send EOSE (End of Stored Events) immediately for now
            Some(json!(["EOSE", sub_id]))
        }
        "CLOSE" => {
            // ["CLOSE", <subscription_id>]
            if msg_array.len() < 2 {
                return Some(json!(["NOTICE", "CLOSE message missing subscription ID"]));
            }

            let sub_id = msg_array[1].as_str()?;
            debug!("Subscription closed: {}", sub_id);
            None // No response needed for CLOSE
        }
        _ => {
            warn!("Unknown message type: {}", msg_type);
            Some(json!(["NOTICE", format!("Unknown message type: {}", msg_type)]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, DatabaseConfig};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_handle_event_message() {
        let config = DatabaseConfig {
            path: PathBuf::from(":memory:"),
            ..Default::default()
        };
        let db = Database::new(config).unwrap();

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
        let response = handle_nostr_message(&msg, &db).await;

        assert!(response.is_some());
        let resp = response.unwrap();
        assert_eq!(resp[0], "OK");
    }

    #[tokio::test]
    async fn test_handle_req_message() {
        let config = DatabaseConfig {
            path: PathBuf::from(":memory:"),
            ..Default::default()
        };
        let db = Database::new(config).unwrap();

        let msg = json!(["REQ", "sub_123", {"kinds": [1]}]);
        let response = handle_nostr_message(&msg, &db).await;

        assert!(response.is_some());
        let resp = response.unwrap();
        assert_eq!(resp[0], "EOSE");
        assert_eq!(resp[1], "sub_123");
    }

    #[tokio::test]
    async fn test_handle_close_message() {
        let config = DatabaseConfig {
            path: PathBuf::from(":memory:"),
            ..Default::default()
        };
        let db = Database::new(config).unwrap();

        let msg = json!(["CLOSE", "sub_123"]);
        let response = handle_nostr_message(&msg, &db).await;

        assert!(response.is_none()); // CLOSE doesn't send a response
    }

    #[tokio::test]
    async fn test_handle_invalid_message() {
        let config = DatabaseConfig {
            path: PathBuf::from(":memory:"),
            ..Default::default()
        };
        let db = Database::new(config).unwrap();

        let msg = json!(["UNKNOWN", "data"]);
        let response = handle_nostr_message(&msg, &db).await;

        assert!(response.is_some());
        let resp = response.unwrap();
        assert_eq!(resp[0], "NOTICE");
    }
}
