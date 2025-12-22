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
use crate::metrics::RelayMetrics;
use crate::negentropy::{NegentropySessionManager, SessionId};
use crate::rate_limit::{RateLimiter, RateLimitConfig};
use crate::relay_info::RelayInformation;
use crate::subscription::{Filter, SubscriptionManager};
use crate::validation;
use futures::{SinkExt, StreamExt};
use nostr::Event;
use nostr::nip77::{NegentropyMessage, Record};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use warp::Filter as WarpFilter;

/// Relay server configuration
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Bind address for the WebSocket server
    pub bind_addr: SocketAddr,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Relay information (NIP-11)
    pub relay_info: RelayInformation,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:7000".parse().unwrap(),
            max_message_size: 512 * 1024, // 512 KB
            rate_limit: RateLimitConfig::default(),
            relay_info: RelayInformation::default(),
        }
    }
}

/// Nostr relay server
pub struct RelayServer {
    config: RelayConfig,
    db: Arc<Database>,
    broadcast_tx: broadcast::Sender<BroadcastEvent>,
    rate_limiter: Arc<RateLimiter>,
    metrics: Arc<RelayMetrics>,
}

impl RelayServer {
    /// Create a new relay server
    pub fn new(config: RelayConfig, db: Database) -> Self {
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit.clone()));
        let metrics = Arc::new(RelayMetrics::new());
        Self {
            config,
            db: Arc::new(db),
            broadcast_tx,
            rate_limiter,
            metrics,
        }
    }

    /// Get metrics
    pub fn metrics(&self) -> Arc<RelayMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Start the relay server (WebSocket only, NIP-11 HTTP endpoint to be added separately)
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
                        self.metrics.connection_blocked_banned();
                        continue;
                    }

                    // Check connection limit
                    if !self.rate_limiter.check_connection_allowed(ip).await {
                        warn!("Connection limit exceeded for IP: {}", ip);
                        self.metrics.connection_blocked_rate_limit();
                        continue;
                    }

                    debug!("New connection from {}", addr);
                    self.rate_limiter.register_connection(ip).await;
                    self.metrics.connection_opened();

                    let db = Arc::clone(&self.db);
                    let broadcast_tx = self.broadcast_tx.clone();
                    let rate_limiter = Arc::clone(&self.rate_limiter);
                    let metrics = Arc::clone(&self.metrics);

                    tokio::spawn(async move {
                        let result = handle_connection(stream, addr, db, broadcast_tx, rate_limiter.clone(), metrics.clone()).await;

                        // Unregister connection
                        rate_limiter.unregister_connection(ip).await;
                        metrics.connection_closed();

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

    /// Get the relay information document (NIP-11)
    pub fn relay_info(&self) -> RelayInformation {
        self.config.relay_info.clone()
    }

    /// Start HTTP server for NIP-11 relay information endpoint
    ///
    /// This should be run concurrently with start() to provide NIP-11 support.
    /// Serves relay information at the given address with Accept: application/nostr+json header.
    pub async fn start_info_server(&self, http_addr: SocketAddr) -> Result<()> {
        let relay_info = self.config.relay_info.clone();

        // NIP-11: Relay information endpoint
        let info_route = warp::path::end()
            .and(warp::header::exact("accept", "application/nostr+json"))
            .and(warp::any().map(move || relay_info.clone()))
            .map(|info: RelayInformation| {
                warp::reply::json(&info)
            })
            .with(warp::reply::with::header(
                "Access-Control-Allow-Origin",
                "*",
            ))
            .with(warp::reply::with::header(
                "Access-Control-Allow-Headers",
                "*",
            ))
            .with(warp::reply::with::header(
                "Access-Control-Allow-Methods",
                "GET, POST, OPTIONS",
            ));

        info!("NIP-11 relay info server listening on http://{}", http_addr);
        info!("Query with: curl -H 'Accept: application/nostr+json' http://{}", http_addr);

        warp::serve(info_route).run(http_addr).await;
        Ok(())
    }
}

/// Handle a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    db: Arc<Database>,
    broadcast_tx: broadcast::Sender<BroadcastEvent>,
    rate_limiter: Arc<RateLimiter>,
    metrics: Arc<RelayMetrics>,
) -> Result<()> {
    let ws_stream = accept_async(stream)
        .await
        .map_err(|e| RelayError::WebSocket(e.to_string()))?;

    info!("WebSocket connection established: {}", addr);

    let (mut write, mut read) = ws_stream.split();
    let mut subscriptions = SubscriptionManager::new();
    let mut negentropy_sessions = NegentropySessionManager::new();
    let connection_id = addr.to_string();
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
                        metrics.bytes_in(text.len() as u64);

                        // Parse the Nostr message
                        match serde_json::from_str::<Value>(&text) {
                            Ok(value) => {
                                let responses = handle_nostr_message(&value, &db, &mut subscriptions, &mut negentropy_sessions, &connection_id, &broadcast_tx, &rate_limiter, &metrics).await;

                                // Update the shared subscriptions
                                *subscriptions_clone.lock().await = subscriptions.clone();

                                for response in responses {
                                    let response_text = serde_json::to_string(&response)?;
                                    metrics.bytes_out(response_text.len() as u64);
                                    if let Err(e) = write.send(Message::Text(response_text)).await {
                                        error!("Failed to send response to {}: {}", addr, e);
                                        return Ok(());
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Invalid JSON from {}: {}", addr, e);
                                let notice = json!(["NOTICE", format!("Invalid JSON: {}", e)]);
                                let notice_text = notice.to_string();
                                metrics.bytes_out(notice_text.len() as u64);
                                let _ = write.send(Message::Text(notice_text)).await;
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
    negentropy_sessions: &mut NegentropySessionManager,
    connection_id: &str,
    broadcast_tx: &broadcast::Sender<BroadcastEvent>,
    rate_limiter: &RateLimiter,
    metrics: &RelayMetrics,
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
                responses.push(json!(["NOTICE", "invalid: EVENT message must have 2 elements"]));
                return responses;
            }

            metrics.event_received();

            // Check event rate limit
            if !rate_limiter.check_event_allowed() {
                metrics.event_rejected_rate_limit();
                responses.push(json!(["NOTICE", "rate-limited: slow down there chief"]));
                return responses;
            }

            // Parse event
            let event: Event = match serde_json::from_value(msg_array[1].clone()) {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to parse event: {}", e);
                    metrics.event_rejected_validation();
                    responses.push(json!(["NOTICE", format!("invalid: failed to parse event: {}", e)]));
                    return responses;
                }
            };

            // Validate event structure and cryptography
            if let Err(e) = validation::validate_event(&event) {
                metrics.event_rejected_signature();
                responses.push(json!(["OK", event.id.clone(), false, e.to_string()]));
                return responses;
            }

            // Store the event
            metrics.db_query();
            match db.store_event(&event) {
                Ok(_) => {
                    debug!("Stored event: {}", event.id);
                    metrics.event_stored();
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
                    metrics.db_error();
                    metrics.event_rejected_validation();
                    responses.push(json!(["OK", event.id, false, format!("error: {}", e)]));
                }
            }
        }
        "REQ" => {
            // ["REQ", <subscription_id>, <filters JSON>...]
            if msg_array.len() < 3 {
                responses.push(json!(["NOTICE", "invalid: REQ message must have at least 3 elements"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NOTICE", "invalid: subscription ID must be string"]));
                    return responses;
                }
            };

            // Validate subscription ID
            if let Err(e) = validation::validate_subscription_id(sub_id) {
                responses.push(json!(["NOTICE", e.to_string()]));
                return responses;
            }

            // Parse and validate filters
            let mut filters = Vec::new();
            for filter_value in msg_array.iter().skip(2) {
                let filter: Filter = match serde_json::from_value(filter_value.clone()) {
                    Ok(f) => f,
                    Err(e) => {
                        responses.push(json!(["CLOSED", sub_id, format!("invalid: failed to parse filter: {}", e)]));
                        return responses;
                    }
                };

                if let Err(e) = validation::validate_filter(&filter) {
                    responses.push(json!(["CLOSED", sub_id, e.to_string()]));
                    return responses;
                }

                filters.push(filter);
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
            metrics.subscription_opened();

            // Query and send matching events for each filter
            for filter in &filters {
                metrics.db_query();
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
                        metrics.db_error();
                        responses.push(json!(["NOTICE", format!("Error querying events: {}", e)]));
                    }
                }
            }

            // Send EOSE (End of Stored Events)
            responses.push(json!(["EOSE", sub_id]));
        }
        "CLOSE" => {
            // ["CLOSE", <subscription_id>]
            if msg_array.len() != 2 {
                responses.push(json!(["NOTICE", "invalid: CLOSE message must have 2 elements"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NOTICE", "invalid: subscription ID must be string"]));
                    return responses;
                }
            };

            // Validate subscription ID
            if let Err(e) = validation::validate_subscription_id(sub_id) {
                responses.push(json!(["NOTICE", e.to_string()]));
                return responses;
            }

            if subscriptions.remove(sub_id) {
                debug!("Subscription closed: {}", sub_id);
                metrics.subscription_closed();
            } else {
                debug!("Attempted to close non-existent subscription: {}", sub_id);
            }
            // No response needed for CLOSE
        }
        "NEG-OPEN" => {
            // ["NEG-OPEN", <subscription_id>, <filter>, <initial_message>]
            if msg_array.len() != 4 {
                responses.push(json!(["NEG-ERR", "", "invalid: NEG-OPEN must have 4 elements"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NEG-ERR", "", "invalid: subscription ID must be string"]));
                    return responses;
                }
            };

            // Validate subscription ID
            if let Err(e) = validation::validate_subscription_id(sub_id) {
                responses.push(json!(["NEG-ERR", sub_id, e.to_string()]));
                return responses;
            }

            // Parse filter
            let filter: Filter = match serde_json::from_value(msg_array[2].clone()) {
                Ok(f) => f,
                Err(e) => {
                    responses.push(json!(["NEG-ERR", sub_id, format!("invalid: failed to parse filter: {}", e)]));
                    return responses;
                }
            };

            if let Err(e) = validation::validate_filter(&filter) {
                responses.push(json!(["NEG-ERR", sub_id, e.to_string()]));
                return responses;
            }

            // Decode initial message
            let initial_message_hex = match msg_array[3].as_str() {
                Some(hex) => hex,
                None => {
                    responses.push(json!(["NEG-ERR", sub_id, "invalid: initial message must be hex string"]));
                    return responses;
                }
            };

            let client_message = match NegentropyMessage::decode_hex(initial_message_hex) {
                Ok(msg) => msg,
                Err(e) => {
                    responses.push(json!(["NEG-ERR", sub_id, format!("invalid: failed to decode message: {}", e)]));
                    return responses;
                }
            };

            // Query events matching filter
            metrics.db_query();
            let events = match db.query_events(&filter) {
                Ok(events) => events,
                Err(e) => {
                    responses.push(json!(["NEG-ERR", sub_id, format!("error: failed to query events: {}", e)]));
                    metrics.db_error();
                    return responses;
                }
            };

            // Convert events to records
            let records: Vec<Record> = events
                .iter()
                .map(|event| {
                    let id_bytes = hex::decode(&event.id).unwrap_or_else(|_| vec![0u8; 32]);
                    let mut id = [0u8; 32];
                    id.copy_from_slice(&id_bytes[..32.min(id_bytes.len())]);
                    Record::new(event.created_at, id)
                })
                .collect();

            debug!("NEG-OPEN: {} events for subscription {}", records.len(), sub_id);

            // Create session
            let session_id = SessionId::new(connection_id.to_string(), sub_id.to_string());
            negentropy_sessions.create_session(session_id.clone(), records);

            // Process initial message
            if let Some(session) = negentropy_sessions.get_session_mut(&session_id) {
                match session.state.process_message(&client_message) {
                    Ok(response_message) => {
                        match response_message.encode_hex() {
                            Ok(hex) => {
                                responses.push(json!(["NEG-MSG", sub_id, hex]));
                            }
                            Err(e) => {
                                responses.push(json!(["NEG-ERR", sub_id, format!("error: failed to encode response: {}", e)]));
                                negentropy_sessions.remove_session(&session_id);
                            }
                        }
                    }
                    Err(e) => {
                        responses.push(json!(["NEG-ERR", sub_id, format!("error: {}", e)]));
                        negentropy_sessions.remove_session(&session_id);
                    }
                }
            }
        }
        "NEG-MSG" => {
            // ["NEG-MSG", <subscription_id>, <message>]
            if msg_array.len() != 3 {
                responses.push(json!(["NEG-ERR", "", "invalid: NEG-MSG must have 3 elements"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NEG-ERR", "", "invalid: subscription ID must be string"]));
                    return responses;
                }
            };

            // Decode message
            let message_hex = match msg_array[2].as_str() {
                Some(hex) => hex,
                None => {
                    responses.push(json!(["NEG-ERR", sub_id, "invalid: message must be hex string"]));
                    return responses;
                }
            };

            let client_message = match NegentropyMessage::decode_hex(message_hex) {
                Ok(msg) => msg,
                Err(e) => {
                    responses.push(json!(["NEG-ERR", sub_id, format!("invalid: failed to decode message: {}", e)]));
                    return responses;
                }
            };

            // Get session
            let session_id = SessionId::new(connection_id.to_string(), sub_id.to_string());
            if let Some(session) = negentropy_sessions.get_session_mut(&session_id) {
                // Check if reconciliation is complete
                let is_complete = session.state.is_complete(&client_message);

                // Process message
                match session.state.process_message(&client_message) {
                    Ok(response_message) => {
                        if is_complete {
                            // Reconciliation complete - clean up session
                            debug!("NEG-MSG: Reconciliation complete for {}", sub_id);
                            negentropy_sessions.remove_session(&session_id);
                            // Send empty message to signal completion
                            responses.push(json!(["NEG-MSG", sub_id, ""]));
                        } else {
                            // Send next round
                            match response_message.encode_hex() {
                                Ok(hex) => {
                                    responses.push(json!(["NEG-MSG", sub_id, hex]));
                                }
                                Err(e) => {
                                    responses.push(json!(["NEG-ERR", sub_id, format!("error: failed to encode response: {}", e)]));
                                    negentropy_sessions.remove_session(&session_id);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        responses.push(json!(["NEG-ERR", sub_id, format!("error: {}", e)]));
                        negentropy_sessions.remove_session(&session_id);
                    }
                }
            } else {
                responses.push(json!(["NEG-ERR", sub_id, "error: no active session"]));
            }
        }
        "NEG-CLOSE" => {
            // ["NEG-CLOSE", <subscription_id>]
            if msg_array.len() != 2 {
                responses.push(json!(["NOTICE", "invalid: NEG-CLOSE must have 2 elements"]));
                return responses;
            }

            let sub_id = match msg_array[1].as_str() {
                Some(id) => id,
                None => {
                    responses.push(json!(["NOTICE", "invalid: subscription ID must be string"]));
                    return responses;
                }
            };

            // Remove session
            let session_id = SessionId::new(connection_id.to_string(), sub_id.to_string());
            if negentropy_sessions.remove_session(&session_id).is_some() {
                debug!("NEG-CLOSE: Session closed for {}", sub_id);
            } else {
                debug!("NEG-CLOSE: No active session for {}", sub_id);
            }
            // No response needed for NEG-CLOSE
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
        let mut neg_sessions = NegentropySessionManager::new();
        let conn_id = "test-conn";
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
        let metrics = RelayMetrics::new();
        let responses = handle_nostr_message(&msg, &db, &mut subs, &mut neg_sessions, conn_id, &broadcast_tx, &rate_limiter, &metrics).await;

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
        let mut neg_sessions = NegentropySessionManager::new();
        let conn_id = "test-conn";
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());
        let metrics = RelayMetrics::new();

        let msg = json!(["REQ", "sub_123", {"kinds": [1]}]);
        let responses = handle_nostr_message(&msg, &db, &mut subs, &mut neg_sessions, conn_id, &broadcast_tx, &rate_limiter, &metrics).await;

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
        let mut neg_sessions = NegentropySessionManager::new();
        let conn_id = "test-conn";
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());
        let metrics = RelayMetrics::new();

        // First create a subscription
        let msg1 = json!(["REQ", "sub_123", {"kinds": [1]}]);
        handle_nostr_message(&msg1, &db, &mut subs, &mut neg_sessions, conn_id, &broadcast_tx, &rate_limiter, &metrics).await;
        assert_eq!(subs.len(), 1);

        // Now close it
        let msg2 = json!(["CLOSE", "sub_123"]);
        let responses = handle_nostr_message(&msg2, &db, &mut subs, &mut neg_sessions, conn_id, &broadcast_tx, &rate_limiter, &metrics).await;

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
        let mut neg_sessions = NegentropySessionManager::new();
        let conn_id = "test-conn";
        let (broadcast_tx, _) = create_broadcast_channel();
        let rate_limiter = RateLimiter::new(RateLimitConfig::default());
        let metrics = RelayMetrics::new();

        let msg = json!(["UNKNOWN", "data"]);
        let responses = handle_nostr_message(&msg, &db, &mut subs, &mut neg_sessions, conn_id, &broadcast_tx, &rate_limiter, &metrics).await;

        assert!(!responses.is_empty());
        let resp = &responses[0];
        assert_eq!(resp[0], "NOTICE");
    }
}
