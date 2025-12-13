//! WebSocket connection to a single Nostr relay.
//!
//! This module handles connecting to a relay, sending messages, and receiving events.

use crate::message::{ClientMessage, Filter, MessageError, RelayMessage};
use async_tungstenite::tungstenite::Message as WsMessage;
use async_tungstenite::{WebSocketReceiver, WebSocketSender};
use futures::StreamExt;
use nostr::Event;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};
use url::Url;

/// Errors that can occur during relay connection.
#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("failed to connect: {0}")]
    Connect(String),

    #[error("connection closed")]
    Closed,

    #[error("send error: {0}")]
    Send(String),

    #[error("receive error: {0}")]
    Receive(String),

    #[error("message error: {0}")]
    Message(#[from] MessageError),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("not connected")]
    NotConnected,
}

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connecting
    Connecting,
    /// Connected and ready
    Connected,
    /// Connection failed
    Failed,
}

/// A subscription to events from a relay.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Subscription ID
    pub id: String,
    /// Filters for this subscription
    pub filters: Vec<Filter>,
    /// Whether EOSE has been received
    pub eose_received: bool,
}

/// Configuration for a relay connection.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Relay URL
    pub url: String,
    /// Reconnect on disconnect
    pub auto_reconnect: bool,
    /// Reconnect delay in milliseconds
    pub reconnect_delay_ms: u64,
    /// Maximum reconnect attempts (0 = unlimited)
    pub max_reconnect_attempts: u32,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            auto_reconnect: true,
            reconnect_delay_ms: 1000,
            max_reconnect_attempts: 0,
        }
    }
}

impl ConnectionConfig {
    /// Create a new config with the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set auto-reconnect behavior.
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Set reconnect delay.
    pub fn reconnect_delay(mut self, ms: u64) -> Self {
        self.reconnect_delay_ms = ms;
        self
    }
}

/// Type alias for the WebSocket sender.
type WsSender = WebSocketSender<async_tungstenite::tokio::ConnectStream>;

/// Type alias for the WebSocket receiver.
type WsReceiver = WebSocketReceiver<async_tungstenite::tokio::ConnectStream>;

/// A connection to a single Nostr relay.
pub struct RelayConnection {
    /// Configuration
    config: ConnectionConfig,
    /// Current connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Active subscriptions
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    /// Channel for sending messages to the relay
    outgoing_tx: Option<mpsc::Sender<ClientMessage>>,
    /// Broadcast channel for received events
    events_tx: broadcast::Sender<RelayMessage>,
    /// WebSocket sender (for direct send)
    ws_sender: Arc<Mutex<Option<WsSender>>>,
}

impl RelayConnection {
    /// Create a new relay connection with the given config.
    pub fn new(config: ConnectionConfig) -> Self {
        let (events_tx, _) = broadcast::channel(1000);
        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            outgoing_tx: None,
            events_tx,
            ws_sender: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new connection from a URL string.
    pub fn from_url(url: impl Into<String>) -> Self {
        Self::new(ConnectionConfig::new(url))
    }

    /// Get the relay URL.
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Get the current connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        *self.state.read().await == ConnectionState::Connected
    }

    /// Subscribe to relay messages.
    pub fn subscribe_messages(&self) -> broadcast::Receiver<RelayMessage> {
        self.events_tx.subscribe()
    }

    /// Connect to the relay.
    pub async fn connect(&mut self) -> Result<(), ConnectionError> {
        // Parse and validate URL
        let url = Url::parse(&self.config.url)
            .map_err(|e| ConnectionError::InvalidUrl(e.to_string()))?;

        // Update state
        *self.state.write().await = ConnectionState::Connecting;
        info!("Connecting to relay: {}", url);

        // Connect via WebSocket
        let (ws_stream, _response) = async_tungstenite::tokio::connect_async(url.as_str())
            .await
            .map_err(|e| {
                error!("Failed to connect to {}: {}", url, e);
                ConnectionError::Connect(e.to_string())
            })?;

        info!("Connected to relay: {}", self.config.url);

        // Split the stream
        let (ws_sender, ws_receiver) = ws_stream.split();

        // Store the sender for direct sends
        *self.ws_sender.lock().await = Some(ws_sender);

        // Create channel for outgoing messages
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<ClientMessage>(100);
        self.outgoing_tx = Some(outgoing_tx);

        // Update state
        *self.state.write().await = ConnectionState::Connected;

        // Spawn tasks for sending and receiving
        self.spawn_receiver(ws_receiver);
        self.spawn_sender(outgoing_rx);

        Ok(())
    }

    /// Spawn the receiver task.
    fn spawn_receiver(&self, mut receiver: WsReceiver) {
        let events_tx = self.events_tx.clone();
        let state = self.state.clone();
        let subscriptions = self.subscriptions.clone();
        let url = self.config.url.clone();

        tokio::spawn(async move {
            while let Some(msg_result) = receiver.next().await {
                match msg_result {
                    Ok(WsMessage::Text(text)) => {
                        match RelayMessage::from_json(&text) {
                            Ok(relay_msg) => {
                                // Handle EOSE specially to update subscription state
                                if let RelayMessage::Eose { ref subscription_id } = relay_msg {
                                    let mut subs = subscriptions.write().await;
                                    if let Some(sub) = subs.get_mut(subscription_id) {
                                        sub.eose_received = true;
                                        debug!(
                                            "EOSE received for subscription {} on {}",
                                            subscription_id, url
                                        );
                                    }
                                }

                                // Broadcast to subscribers
                                let _ = events_tx.send(relay_msg);
                            }
                            Err(e) => {
                                warn!("Failed to parse relay message from {}: {}", url, e);
                            }
                        }
                    }
                    Ok(WsMessage::Close(_)) => {
                        info!("Relay {} closed connection", url);
                        break;
                    }
                    Ok(WsMessage::Ping(data)) => {
                        debug!("Received ping from {}", url);
                        // Pong is handled automatically by tungstenite
                        let _ = data; // silence unused warning
                    }
                    Ok(_) => {
                        // Ignore binary and other messages
                    }
                    Err(e) => {
                        error!("Error receiving from {}: {}", url, e);
                        break;
                    }
                }
            }

            // Connection closed
            *state.write().await = ConnectionState::Disconnected;
            info!("Receiver task ended for {}", url);
        });
    }

    /// Spawn the sender task.
    fn spawn_sender(&self, mut outgoing_rx: mpsc::Receiver<ClientMessage>) {
        let ws_sender = self.ws_sender.clone();
        let url = self.config.url.clone();

        tokio::spawn(async move {
            while let Some(msg) = outgoing_rx.recv().await {
                let json = match msg.to_json() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                        continue;
                    }
                };

                let mut sender_guard = ws_sender.lock().await;
                if let Some(sender) = sender_guard.as_mut() {
                    if let Err(e) = sender.send(WsMessage::Text(json.into())).await {
                        error!("Failed to send to {}: {}", url, e);
                        break;
                    }
                } else {
                    warn!("No WebSocket sender available for {}", url);
                    break;
                }
            }
            debug!("Sender task ended for {}", url);
        });
    }

    /// Send a message to the relay.
    pub async fn send(&self, msg: ClientMessage) -> Result<(), ConnectionError> {
        if let Some(tx) = &self.outgoing_tx {
            tx.send(msg)
                .await
                .map_err(|e| ConnectionError::Send(e.to_string()))?;
            Ok(())
        } else {
            Err(ConnectionError::NotConnected)
        }
    }

    /// Publish an event to the relay.
    pub async fn publish(&self, event: Event) -> Result<(), ConnectionError> {
        debug!("Publishing event {} to {}", event.id, self.config.url);
        self.send(ClientMessage::Event(event)).await
    }

    /// Subscribe to events matching the given filters.
    pub async fn subscribe(
        &self,
        subscription_id: impl Into<String>,
        filters: Vec<Filter>,
    ) -> Result<(), ConnectionError> {
        let id = subscription_id.into();
        debug!(
            "Subscribing {} with {} filters on {}",
            id,
            filters.len(),
            self.config.url
        );

        // Store subscription
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(
                id.clone(),
                Subscription {
                    id: id.clone(),
                    filters: filters.clone(),
                    eose_received: false,
                },
            );
        }

        // Send REQ message
        self.send(ClientMessage::Req {
            subscription_id: id,
            filters,
        })
        .await
    }

    /// Close a subscription.
    pub async fn unsubscribe(&self, subscription_id: impl Into<String>) -> Result<(), ConnectionError> {
        let id = subscription_id.into();
        debug!("Unsubscribing {} on {}", id, self.config.url);

        // Remove subscription
        {
            let mut subs = self.subscriptions.write().await;
            subs.remove(&id);
        }

        // Send CLOSE message
        self.send(ClientMessage::Close {
            subscription_id: id,
        })
        .await
    }

    /// Get all active subscriptions.
    pub async fn subscriptions(&self) -> Vec<Subscription> {
        self.subscriptions.read().await.values().cloned().collect()
    }

    /// Disconnect from the relay.
    pub async fn disconnect(&mut self) {
        info!("Disconnecting from {}", self.config.url);

        // Close WebSocket
        if let Some(sender) = self.ws_sender.lock().await.as_mut() {
            let _ = sender.close(None).await;
        }
        *self.ws_sender.lock().await = None;

        // Clear outgoing channel
        self.outgoing_tx = None;

        // Update state
        *self.state.write().await = ConnectionState::Disconnected;

        // Clear subscriptions
        self.subscriptions.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config() {
        let config = ConnectionConfig::new("wss://relay.example.com")
            .auto_reconnect(true)
            .reconnect_delay(2000);

        assert_eq!(config.url, "wss://relay.example.com");
        assert!(config.auto_reconnect);
        assert_eq!(config.reconnect_delay_ms, 2000);
    }

    #[test]
    fn test_connection_state_default() {
        let conn = RelayConnection::from_url("wss://relay.example.com");
        // Can't test async state without tokio runtime, but we can verify construction
        assert_eq!(conn.url(), "wss://relay.example.com");
    }

    #[tokio::test]
    async fn test_connection_initial_state() {
        let conn = RelayConnection::from_url("wss://relay.example.com");
        assert_eq!(conn.state().await, ConnectionState::Disconnected);
        assert!(!conn.is_connected().await);
    }

    #[tokio::test]
    async fn test_subscriptions_empty() {
        let conn = RelayConnection::from_url("wss://relay.example.com");
        let subs = conn.subscriptions().await;
        assert!(subs.is_empty());
    }

    // Note: Integration tests requiring actual relay connection would be in separate test file
}
