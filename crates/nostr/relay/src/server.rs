//! WebSocket server for handling client connections
//!
//! The relay server accepts WebSocket connections and handles Nostr protocol messages:
//! - EVENT: Store events
//! - REQ: Subscribe to events matching filters
//! - CLOSE: Close a subscription
//!
//! Messages are JSON arrays following the Nostr protocol specification.

use crate::broadcast::{BroadcastEvent, create_broadcast_channel};
use crate::db::Database;
use crate::error::{RelayError, Result};
use crate::rate_limit::{RateLimiter, RateLimitConfig};
use crate::subscription::{Filter, SubscriptionManager};
use futures::{SinkExt, StreamExt};
use nostr::Event;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Relay server configuration
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Bind address for the WebSocket server
    pub bind_addr: SocketAddr,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:7000".parse().unwrap(),
            max_message_size: 512 * 1024, // 512 KB
            rate_limit: RateLimitConfig::default(),
        }
    }
}

/// Nostr relay server
pub struct RelayServer {
    config: RelayConfig,
    db: Arc<Database>,
    broadcast_tx: broadcast::Sender<BroadcastEvent>,
    rate_limiter: Arc<RateLimiter>,
}

impl RelayServer {
    /// Create a new relay server
    pub fn new(config: RelayConfig, db: Database) -> Self {
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit.clone()));
        Self {
            config,
            db: Arc::new(db),
            broadcast_tx,
            rate_limiter,
        }
    }

    /// Start the relay server
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        info!("Relay server listening on {}", self.config.bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let ip = addr.ip();

                    // Check if IP is banned
                    if self.rate_limiter.is_banned(ip).await {
                        warn!("Rejected connection from banned IP: {}", ip);
                        continue;
                    }

                    // Check connection limit
                    if !self.rate_limiter.check_connection_allowed(ip).await {
                        warn!("Connection limit exceeded for IP: {}", ip);
                        continue;
                    }

                    debug!("New connection from {}", addr);
                    self.rate_limiter.register_connection(ip).await;

                    let db = Arc::clone(&self.db);
                    let broadcast_tx = self.broadcast_tx.clone();
                    let rate_limiter = Arc::clone(&self.rate_limiter);

                    tokio::spawn(async move {
                        let result = handle_connection(stream, addr, db, broadcast_tx, rate_limiter.clone()).await;

                        // Unregister connection
                        rate_limiter.unregister_connection(ip).await;

                        if let Err(e) = result {
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
    broadcast_tx: broadcast::Sender<BroadcastEvent>,
    rate_limiter: Arc<RateLimiter>,
) -> Result<()> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| RelayError::WebSocket(e.to_string()))?;

    info!("WebSocket connection established: {}", addr);

    let (mut write, mut read) = ws_stream.split();
    let mut subscriptions = SubscriptionManager::new();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Spawn task to handle broadcasts
    let subscriptions_clone = Arc::new(tokio::sync::Mutex::new(SubscriptionManager::new()));
    let subscriptions_for_broadcast = Arc::clone(&subscriptions_clone);
    let (broadcast_event_tx, mut broadcast_event_rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(broadcast_event) => {
                    let subs = subscriptions_for_broadcast.lock().await;
                    let matching_sub_ids = subs.matches_any(&broadcast_event.event);

                    for sub_id in matching_sub_ids {
                        let event_msg = json!(["EVENT", sub_id, broadcast_event.event]);
                        let _ = broadcast_event_tx.send(event_msg);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Broadcast receiver lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Broadcast channel closed");
                    break;
                }
            }
        }
    });

    loop {
        tokio::select! {
            // Handle incoming WebSocket messages
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("Received message from {}: {}", addr, text);

                        // Parse the Nostr message
                        match serde_json::from_str::<Value>(&text) {
                            Ok(value) => {
                                let responses = handle_nostr_message(&value, &db, &mut subscriptions, &broadcast_tx, &rate_limiter).await;

                                // Update the shared subscriptions
                                *subscriptions_clone.lock().await = subscriptions.clone();

                                for response in responses {
                                    let response_text = serde_json::to_string(&response)?;
                                    if let Err(e) = write.send(Message::Text(response_text)).await {
                                        error!("Failed to send response to {}: {}", addr, e);
                                        return Ok(());
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
                    Some(Ok(Message::Close(_))) => {
                        info!("Client {} disconnected", addr);
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        debug!("Ping from {}", addr);
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error from {}: {}", addr, e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }

            // Handle broadcast events
            event_msg = broadcast_event_rx.recv() => {
                if let Some(msg) = event_msg {
                    let response_text = serde_json::to_string(&msg)?;
                    if let Err(e) = write.send(Message::Text(response_text)).await {
                        error!("Failed to send broadcast event to {}: {}", addr, e);
                        break;
                    }
                }
            }
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
    broadcast_tx: &broadcast::Sender<BroadcastEvent>,
    rate_limiter: &RateLimiter,
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

            // Check event rate limit
            if !rate_limiter.check_event_allowed() {
                responses.push(json!(["NOTICE", "rate limit: slow down"]));
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

                            // Broadcast the event to all subscribers
                            let broadcast_event = BroadcastEvent {
                                event: event.clone(),
                            };
                            if let Err(e) = broadcast_tx.send(broadcast_event) {
                                warn!("Failed to broadcast event: {}", e);
                            }
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

            // Check subscription limit
            if !rate_limiter.check_subscription_allowed(subscriptions.len()) {
                responses.push(json!(["NOTICE", format!(
                    "rate limit: max {} subscriptions per connection",
                    rate_limiter.max_subscriptions()
                )]));
                return responses;
            }

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
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());

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
        let responses = handle_nostr_message(&msg, &db, &mut subs, &broadcast_tx, &rate_limiter).await;

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
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());

        let msg = json!(["REQ", "sub_123", {"kinds": [1]}]);
        let responses = handle_nostr_message(&msg, &db, &mut subs, &broadcast_tx, &rate_limiter).await;

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
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());

        // First create a subscription
        let msg1 = json!(["REQ", "sub_123", {"kinds": [1]}]);
        handle_nostr_message(&msg1, &db, &mut subs, &broadcast_tx, &rate_limiter).await;
        assert_eq!(subs.len(), 1);

        // Now close it
        let msg2 = json!(["CLOSE", "sub_123"]);
        let responses = handle_nostr_message(&msg2, &db, &mut subs, &broadcast_tx, &rate_limiter).await;

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
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());

        let msg = json!(["UNKNOWN", "data"]);
        let responses = handle_nostr_message(&msg, &db, &mut subs, &broadcast_tx, &rate_limiter).await;

        assert!(!responses.is_empty());
        let resp = &responses[0];
        assert_eq!(resp[0], "NOTICE");
    }
}
