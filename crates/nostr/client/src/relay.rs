//! Single relay connection management
//!
//! Provides async connection to a Nostr relay with automatic reconnection,
//! message handling, and health monitoring.

use crate::error::{ClientError, Result};
use futures::{SinkExt, StreamExt};
use nostr::Event;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, info, warn};
use url::Url;

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Disconnected
    Disconnected,
    /// Currently connecting
    Connecting,
    /// Connected and ready
    Connected,
    /// Reconnecting after disconnect
    Reconnecting,
}

/// Relay message received from the relay
#[derive(Debug, Clone)]
pub enum RelayMessage {
    /// EVENT message: ["EVENT", subscription_id, event]
    Event(String, Event),
    /// OK message: ["OK", event_id, success, message]
    Ok(String, bool, String),
    /// EOSE message: ["EOSE", subscription_id]
    Eose(String),
    /// NOTICE message: ["NOTICE", message]
    Notice(String),
    /// AUTH message: ["AUTH", challenge]
    Auth(String),
}

/// Confirmation result for event publishing
#[derive(Debug, Clone)]
pub struct PublishConfirmation {
    /// Event ID that was published
    pub event_id: String,
    /// Whether the relay accepted the event
    pub accepted: bool,
    /// Message from the relay (empty if accepted, error message if rejected)
    pub message: String,
}

type ConfirmationSender = oneshot::Sender<PublishConfirmation>;

/// Relay connection configuration
#[derive(Debug, Clone)]
pub struct RelayConfig {
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Reconnection attempts (0 = infinite)
    pub max_reconnect_attempts: u32,
    /// Initial reconnection delay
    pub reconnect_delay: Duration,
    /// Maximum reconnection delay
    pub max_reconnect_delay: Duration,
    /// Ping interval for health checks
    pub ping_interval: Duration,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            max_reconnect_attempts: 0, // Infinite
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
            ping_interval: Duration::from_secs(30),
        }
    }
}

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Relay connection
pub struct RelayConnection {
    /// Relay URL
    url: Url,
    /// Configuration
    config: RelayConfig,
    /// Connection state
    state: Arc<RwLock<ConnectionState>>,
    /// WebSocket stream
    ws: Arc<Mutex<Option<WsStream>>>,
    /// Incoming message channel (for future background task)
    _rx: Arc<Mutex<mpsc::UnboundedReceiver<RelayMessage>>>,
    /// Outgoing message channel (for future background task)
    _tx: mpsc::UnboundedSender<Value>,
    /// Pending event confirmations (event_id -> oneshot sender)
    pending_confirmations: Arc<Mutex<HashMap<String, ConfirmationSender>>>,
}

impl RelayConnection {
    /// Create a new relay connection (does not connect yet)
    pub fn new(url: &str) -> Result<Self> {
        Self::with_config(url, RelayConfig::default())
    }

    /// Create a new relay connection with custom config
    pub fn with_config(url: &str, config: RelayConfig) -> Result<Self> {
        let url = Url::parse(url)?;

        // Validate WebSocket URL
        if url.scheme() != "ws" && url.scheme() != "wss" {
            return Err(ClientError::InvalidUrl(format!(
                "URL must use ws:// or wss:// scheme, got: {}",
                url.scheme()
            )));
        }

        let (_msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (out_tx, _) = mpsc::unbounded_channel();

        Ok(Self {
            url,
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            ws: Arc::new(Mutex::new(None)),
            _rx: Arc::new(Mutex::new(msg_rx)),
            _tx: out_tx,
            pending_confirmations: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Connect to the relay
    pub async fn connect(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state != ConnectionState::Disconnected {
            return Err(ClientError::AlreadyConnected);
        }
        *state = ConnectionState::Connecting;
        drop(state);

        info!("Connecting to relay: {}", self.url);

        let ws_stream = match timeout(
            self.config.connect_timeout,
            connect_async(self.url.as_str()),
        )
        .await
        {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(e)) => {
                *self.state.write().await = ConnectionState::Disconnected;
                return Err(ClientError::WebSocket(e.to_string()));
            }
            Err(_) => {
                *self.state.write().await = ConnectionState::Disconnected;
                return Err(ClientError::Timeout(format!(
                    "Connection timeout after {:?}",
                    self.config.connect_timeout
                )));
            }
        };

        *self.ws.lock().await = Some(ws_stream);
        *self.state.write().await = ConnectionState::Connected;

        info!("Connected to relay: {}", self.url);
        Ok(())
    }

    /// Disconnect from the relay
    pub async fn disconnect(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state == ConnectionState::Disconnected {
            return Ok(());
        }

        info!("Disconnecting from relay: {}", self.url);

        let mut ws = self.ws.lock().await;
        if let Some(mut stream) = ws.take() {
            let _ = stream.close(None).await;
        }

        *state = ConnectionState::Disconnected;
        info!("Disconnected from relay: {}", self.url);
        Ok(())
    }

    /// Get current connection state
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        *self.state.read().await == ConnectionState::Connected
    }

    /// Send an event to the relay
    pub async fn send_event(&self, event: &Event) -> Result<()> {
        let msg = json!(["EVENT", event]);
        self.send_message(&msg).await
    }

    /// Publish an event and wait for confirmation from the relay
    ///
    /// This method sends the event and waits for an OK message response.
    /// Returns PublishConfirmation indicating whether the relay accepted the event.
    pub async fn publish_event(
        &self,
        event: &Event,
        confirmation_timeout: Duration,
    ) -> Result<PublishConfirmation> {
        // Create oneshot channel for confirmation
        let (tx, rx) = oneshot::channel();

        // Get event ID (already hex-encoded string)
        let event_id = event.id.clone();

        // Register pending confirmation
        {
            let mut confirmations = self.pending_confirmations.lock().await;
            confirmations.insert(event_id.clone(), tx);
        }

        // Send the event
        if let Err(e) = self.send_event(event).await {
            // Remove pending confirmation on send error
            let mut confirmations = self.pending_confirmations.lock().await;
            confirmations.remove(&event_id);
            return Err(e);
        }

        // Wait for confirmation with timeout
        match timeout(confirmation_timeout, rx).await {
            Ok(Ok(confirmation)) => Ok(confirmation),
            Ok(Err(_)) => {
                // Channel was dropped (shouldn't happen)
                Err(ClientError::PublishFailed(
                    "Confirmation channel closed".to_string(),
                ))
            }
            Err(_) => {
                // Timeout - remove pending confirmation
                let mut confirmations = self.pending_confirmations.lock().await;
                confirmations.remove(&event_id);
                Err(ClientError::Timeout(format!(
                    "Event confirmation timeout after {:?}",
                    confirmation_timeout
                )))
            }
        }
    }

    /// Subscribe to events matching filters
    pub async fn subscribe(&self, subscription_id: &str, filters: &[Value]) -> Result<()> {
        let mut msg = vec![json!("REQ"), json!(subscription_id)];
        msg.extend(filters.iter().cloned());
        self.send_message(&Value::Array(msg)).await
    }

    /// Close a subscription
    pub async fn close_subscription(&self, subscription_id: &str) -> Result<()> {
        let msg = json!(["CLOSE", subscription_id]);
        self.send_message(&msg).await
    }

    /// Send a raw JSON message to the relay
    pub async fn send_message(&self, msg: &Value) -> Result<()> {
        if !self.is_connected().await {
            return Err(ClientError::NotConnected);
        }

        let msg_text = serde_json::to_string(msg)?;
        debug!("Sending to {}: {}", self.url, msg_text);

        let mut ws = self.ws.lock().await;
        if let Some(stream) = ws.as_mut() {
            stream
                .send(Message::Text(msg_text))
                .await
                .map_err(|e| ClientError::WebSocket(e.to_string()))?;
            Ok(())
        } else {
            Err(ClientError::NotConnected)
        }
    }

    /// Receive the next message from the relay
    pub async fn recv(&self) -> Result<Option<RelayMessage>> {
        if !self.is_connected().await {
            return Err(ClientError::NotConnected);
        }

        let mut ws = self.ws.lock().await;
        if let Some(stream) = ws.as_mut() {
            match stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    debug!("Received from {}: {}", self.url, text);
                    let msg = Self::parse_relay_message(&text)?;

                    // Handle OK messages for pending confirmations
                    if let Some(RelayMessage::Ok(event_id, accepted, message)) = &msg {
                        let mut confirmations = self.pending_confirmations.lock().await;
                        if let Some(tx) = confirmations.remove(event_id) {
                            let confirmation = PublishConfirmation {
                                event_id: event_id.clone(),
                                accepted: *accepted,
                                message: message.clone(),
                            };
                            let _ = tx.send(confirmation); // Ignore error if receiver dropped
                        }
                    }

                    Ok(msg)
                }
                Some(Ok(Message::Close(_))) => {
                    info!("Relay closed connection: {}", self.url);
                    Ok(None)
                }
                Some(Ok(Message::Ping(data))) => {
                    stream
                        .send(Message::Pong(data))
                        .await
                        .map_err(|e| ClientError::WebSocket(e.to_string()))?;
                    Ok(None)
                }
                Some(Ok(_)) => Ok(None), // Ignore other message types
                Some(Err(e)) => Err(ClientError::WebSocket(e.to_string())),
                None => Ok(None),
            }
        } else {
            Err(ClientError::NotConnected)
        }
    }

    /// Parse a relay message
    fn parse_relay_message(text: &str) -> Result<Option<RelayMessage>> {
        let value: Value = serde_json::from_str(text)?;

        let arr = match value.as_array() {
            Some(a) => a,
            None => return Ok(None),
        };

        if arr.is_empty() {
            return Ok(None);
        }

        let msg_type = match arr[0].as_str() {
            Some(t) => t,
            None => return Ok(None),
        };

        match msg_type {
            "EVENT" => {
                if arr.len() >= 3 {
                    let sub_id = arr[1].as_str().unwrap_or("").to_string();
                    let event: Event = serde_json::from_value(arr[2].clone())?;
                    Ok(Some(RelayMessage::Event(sub_id, event)))
                } else {
                    Ok(None)
                }
            }
            "OK" => {
                if arr.len() >= 4 {
                    let event_id = arr[1].as_str().unwrap_or("").to_string();
                    let success = arr[2].as_bool().unwrap_or(false);
                    let message = arr[3].as_str().unwrap_or("").to_string();
                    Ok(Some(RelayMessage::Ok(event_id, success, message)))
                } else {
                    Ok(None)
                }
            }
            "EOSE" => {
                if arr.len() >= 2 {
                    let sub_id = arr[1].as_str().unwrap_or("").to_string();
                    Ok(Some(RelayMessage::Eose(sub_id)))
                } else {
                    Ok(None)
                }
            }
            "NOTICE" => {
                if arr.len() >= 2 {
                    let message = arr[1].as_str().unwrap_or("").to_string();
                    Ok(Some(RelayMessage::Notice(message)))
                } else {
                    Ok(None)
                }
            }
            "AUTH" => {
                if arr.len() >= 2 {
                    let challenge = arr[1].as_str().unwrap_or("").to_string();
                    Ok(Some(RelayMessage::Auth(challenge)))
                } else {
                    Ok(None)
                }
            }
            _ => {
                warn!("Unknown message type from relay: {}", msg_type);
                Ok(None)
            }
        }
    }

    /// Get relay URL
    pub fn url(&self) -> &Url {
        &self.url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_connection_creation() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();
        assert_eq!(relay.url().scheme(), "wss");
        assert_eq!(relay.url().host_str(), Some("relay.example.com"));
    }

    #[test]
    fn test_invalid_url_scheme() {
        let result = RelayConnection::new("https://relay.example.com");
        assert!(result.is_err());
        match result {
            Err(ClientError::InvalidUrl(_)) => {}
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    #[test]
    fn test_parse_event_message() {
        let text = r#"["EVENT","sub1",{"id":"abc","pubkey":"def","created_at":123,"kind":1,"tags":[],"content":"hello","sig":"xyz"}]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Event(sub_id, _) => assert_eq!(sub_id, "sub1"),
            _ => panic!("Expected Event message"),
        }
    }

    #[test]
    fn test_parse_ok_message() {
        let text = r#"["OK","event123",true,""]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Ok(id, success, _) => {
                assert_eq!(id, "event123");
                assert!(success);
            }
            _ => panic!("Expected OK message"),
        }
    }

    #[test]
    fn test_parse_eose_message() {
        let text = r#"["EOSE","sub1"]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Eose(sub_id) => assert_eq!(sub_id, "sub1"),
            _ => panic!("Expected EOSE message"),
        }
    }

    #[test]
    fn test_parse_notice_message() {
        let text = r#"["NOTICE","Something went wrong"]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Notice(msg) => assert_eq!(msg, "Something went wrong"),
            _ => panic!("Expected NOTICE message"),
        }
    }

    #[tokio::test]
    async fn test_connection_state() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();
        assert_eq!(relay.state().await, ConnectionState::Disconnected);
        assert!(!relay.is_connected().await);
    }

    #[test]
    fn test_publish_confirmation_creation() {
        let confirmation = PublishConfirmation {
            event_id: "abc123".to_string(),
            accepted: true,
            message: "".to_string(),
        };
        assert_eq!(confirmation.event_id, "abc123");
        assert!(confirmation.accepted);
        assert_eq!(confirmation.message, "");
    }

    #[test]
    fn test_publish_confirmation_rejected() {
        let confirmation = PublishConfirmation {
            event_id: "abc123".to_string(),
            accepted: false,
            message: "duplicate: already have this event".to_string(),
        };
        assert_eq!(confirmation.event_id, "abc123");
        assert!(!confirmation.accepted);
        assert_eq!(confirmation.message, "duplicate: already have this event");
    }

    #[test]
    fn test_parse_ok_message_accepted() {
        let text = r#"["OK","5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36",true,""]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Ok(id, success, message) => {
                assert_eq!(id, "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36");
                assert!(success);
                assert_eq!(message, "");
            }
            _ => panic!("Expected OK message"),
        }
    }

    #[test]
    fn test_parse_ok_message_rejected() {
        let text = r#"["OK","5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36",false,"duplicate: already have this event"]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Ok(id, success, message) => {
                assert_eq!(id, "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36");
                assert!(!success);
                assert_eq!(message, "duplicate: already have this event");
            }
            _ => panic!("Expected OK message"),
        }
    }

    #[tokio::test]
    async fn test_pending_confirmations_tracking() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();
        let (tx, _rx) = oneshot::channel();

        // Add pending confirmation
        {
            let mut confirmations = relay.pending_confirmations.lock().await;
            confirmations.insert("event123".to_string(), tx);
            assert_eq!(confirmations.len(), 1);
        }

        // Check it exists
        {
            let confirmations = relay.pending_confirmations.lock().await;
            assert!(confirmations.contains_key("event123"));
        }

        // Remove it
        {
            let mut confirmations = relay.pending_confirmations.lock().await;
            confirmations.remove("event123");
            assert_eq!(confirmations.len(), 0);
        }
    }
}
