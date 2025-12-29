//! Single relay connection management
//!
//! Provides async connection to a Nostr relay with automatic reconnection,
//! message handling, health monitoring, and automatic event queueing for
//! offline support.

use crate::error::{ClientError, Result};
use crate::queue::MessageQueue;
use crate::recovery::{CircuitBreaker, ExponentialBackoff, HealthMetrics};
use crate::subscription::Subscription;
use futures::{SinkExt, StreamExt};
use nostr::Event;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// Enable message queue for offline support
    pub enable_queue: bool,
    /// Queue retry poll interval
    pub queue_poll_interval: Duration,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            max_reconnect_attempts: 0, // Infinite
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
            ping_interval: Duration::from_secs(30),
            enable_queue: true,
            queue_poll_interval: Duration::from_secs(5),
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
    /// Active subscriptions (subscription_id -> Subscription)
    subscriptions: Arc<Mutex<HashMap<String, Subscription>>>,
    /// Message queue for offline support
    queue: Option<Arc<MessageQueue>>,
    /// Queue processing task handle
    queue_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Receive loop task handle
    recv_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Circuit breaker for fault tolerance
    circuit_breaker: Arc<CircuitBreaker>,
    /// Exponential backoff for reconnection
    backoff: Arc<Mutex<ExponentialBackoff>>,
    /// Health metrics
    health_metrics: Arc<RwLock<HealthMetrics>>,
    /// Connection start time (for uptime tracking)
    connected_at: Arc<RwLock<Option<Instant>>>,
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

        // Create message queue if enabled
        let queue = if config.enable_queue {
            Some(Arc::new(MessageQueue::new().map_err(|e| {
                ClientError::Internal(format!("Failed to create message queue: {}", e))
            })?))
        } else {
            None
        };

        // Create circuit breaker with relay-appropriate thresholds
        let circuit_breaker = Arc::new(CircuitBreaker::new(
            5,                              // 5 failures before opening
            2,                              // 2 successes to close when half-open
            Duration::from_secs(30),        // 30s timeout before half-open
        ));

        // Create exponential backoff matching relay config
        let backoff = Arc::new(Mutex::new(ExponentialBackoff::new(
            config.reconnect_delay,
            config.max_reconnect_delay,
            config.max_reconnect_attempts,
        )));

        // Create health metrics
        let health_metrics = Arc::new(RwLock::new(HealthMetrics::new(url.as_str())));

        Ok(Self {
            url,
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            ws: Arc::new(Mutex::new(None)),
            _rx: Arc::new(Mutex::new(msg_rx)),
            _tx: out_tx,
            pending_confirmations: Arc::new(Mutex::new(HashMap::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            queue,
            queue_task: Arc::new(Mutex::new(None)),
            recv_task: Arc::new(Mutex::new(None)),
            circuit_breaker,
            backoff,
            health_metrics,
            connected_at: Arc::new(RwLock::new(None)),
        })
    }

    /// Connect to the relay
    pub async fn connect(&self) -> Result<()> {
        // Check circuit breaker
        if !self.circuit_breaker.is_allowed().await {
            let error_msg = "Circuit breaker is open - too many failures".to_string();
            let mut metrics = self.health_metrics.write().await;
            metrics.last_error = Some(error_msg.clone());
            return Err(ClientError::CircuitOpen(error_msg));
        }

        let mut state = self.state.write().await;
        if *state != ConnectionState::Disconnected {
            return Err(ClientError::AlreadyConnected);
        }
        *state = ConnectionState::Connecting;
        drop(state);

        info!("Connecting to relay: {}", self.url);

        let connect_start = Instant::now();
        let ws_stream = match timeout(
            self.config.connect_timeout,
            connect_async(self.url.as_str()),
        )
        .await
        {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(e)) => {
                *self.state.write().await = ConnectionState::Disconnected;

                // Record failure in circuit breaker and metrics
                self.circuit_breaker.record_failure().await;
                let mut metrics = self.health_metrics.write().await;
                metrics.failed_messages += 1;
                metrics.last_error = Some(e.to_string());
                metrics.circuit_state = self.circuit_breaker.state().await;
                metrics.backoff_attempt = self.backoff.lock().await.attempt();

                return Err(ClientError::WebSocket(e.to_string()));
            }
            Err(_) => {
                *self.state.write().await = ConnectionState::Disconnected;

                // Record timeout failure
                self.circuit_breaker.record_failure().await;
                let mut metrics = self.health_metrics.write().await;
                metrics.failed_messages += 1;
                metrics.last_error = Some("Connection timeout".to_string());
                metrics.circuit_state = self.circuit_breaker.state().await;
                metrics.backoff_attempt = self.backoff.lock().await.attempt();

                return Err(ClientError::Timeout(format!(
                    "Connection timeout after {:?}",
                    self.config.connect_timeout
                )));
            }
        };

        *self.ws.lock().await = Some(ws_stream);
        *self.state.write().await = ConnectionState::Connected;
        *self.connected_at.write().await = Some(Instant::now());

        // Record successful connection
        self.circuit_breaker.record_success().await;
        self.backoff.lock().await.reset();

        let mut metrics = self.health_metrics.write().await;
        metrics.successful_messages += 1;
        metrics.circuit_state = self.circuit_breaker.state().await;
        metrics.backoff_attempt = 0;
        metrics.last_error = None;

        info!("Connected to relay: {} (took {:?})", self.url, connect_start.elapsed());

        // Process any queued messages
        if let Err(e) = self.process_queue().await {
            warn!("Error processing queue after connect: {}", e);
        }

        // Start queue processing task
        self.start_queue_task();

        // Start background receive loop
        self.start_recv_loop().await;

        Ok(())
    }

    /// Start background receive loop to process incoming messages
    async fn start_recv_loop(&self) {
        let ws = Arc::clone(&self.ws);
        let state = Arc::clone(&self.state);
        let pending_confirmations = Arc::clone(&self.pending_confirmations);
        let subscriptions = Arc::clone(&self.subscriptions);
        let url = self.url.to_string();

        let handle = tokio::spawn(async move {
            loop {
                // Check if still connected
                if *state.read().await != ConnectionState::Connected {
                    break;
                }

                // Try to receive a message
                let msg = {
                    let mut ws_guard = ws.lock().await;
                    if let Some(stream) = ws_guard.as_mut() {
                        match tokio::time::timeout(
                            Duration::from_millis(100),
                            stream.next(),
                        ).await {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                Some(text)
                            }
                            Ok(Some(Ok(Message::Ping(data)))) => {
                                // Respond to ping
                                let _ = stream.send(Message::Pong(data)).await;
                                None
                            }
                            Ok(Some(Ok(Message::Close(_)))) => {
                                info!("Relay {} closed connection", url);
                                break;
                            }
                            Ok(Some(Err(e))) => {
                                warn!("WebSocket error from {}: {}", url, e);
                                break;
                            }
                            Ok(Some(Ok(_))) => None, // Ignore other message types
                            Ok(None) => {
                                // Stream ended
                                break;
                            }
                            Err(_) => None, // Timeout - continue loop
                        }
                    } else {
                        break;
                    }
                };

                // Process received message outside of lock
                if let Some(text) = msg
                    && let Ok(Some(relay_msg)) = Self::parse_relay_message(&text)
                {
                        // Handle OK messages for pending confirmations
                        if let RelayMessage::Ok(event_id, accepted, message) = &relay_msg {
                            let mut confirmations = pending_confirmations.lock().await;
                            if let Some(tx) = confirmations.remove(event_id) {
                                let confirmation = PublishConfirmation {
                                    event_id: event_id.clone(),
                                    accepted: *accepted,
                                    message: message.clone(),
                                };
                                let _ = tx.send(confirmation);
                            }
                        }

                        // Route EVENT messages to subscriptions
                        if let RelayMessage::Event(sub_id, event) = &relay_msg {
                            let mut should_remove = false;
                            {
                                let subs = subscriptions.lock().await;
                                if let Some(subscription) = subs.get(sub_id) {
                                    if let Err(e) = subscription.handle_event(event.clone()) {
                                        let err_str = e.to_string();
                                        // Only warn once if channel closed, then remove subscription
                                        if err_str.contains("channel closed") {
                                            debug!("Subscription {} channel closed, removing", sub_id);
                                            should_remove = true;
                                        } else {
                                            warn!("Error handling event for subscription {}: {}", sub_id, e);
                                        }
                                    }
                                }
                            }
                            if should_remove {
                                subscriptions.lock().await.remove(sub_id);
                            }
                        }

                        // Handle EOSE messages for subscriptions
                        if let RelayMessage::Eose(sub_id) = &relay_msg {
                            let subs = subscriptions.lock().await;
                            if let Some(subscription) = subs.get(sub_id) {
                                subscription.mark_eose();
                            }
                        }
                }
            }
        });

        *self.recv_task.lock().await = Some(handle);
    }

    /// Stop background receive loop
    async fn stop_recv_loop(&self) {
        if let Some(handle) = self.recv_task.lock().await.take() {
            handle.abort();
        }
    }

    /// Disconnect from the relay
    pub async fn disconnect(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state == ConnectionState::Disconnected {
            return Ok(());
        }

        info!("Disconnecting from relay: {}", self.url);

        // Stop receive loop first
        self.stop_recv_loop().await;

        // Stop queue processing task
        self.stop_queue_task().await;

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

            // If not connected and queue is enabled, queue the event for retry
            if matches!(e, ClientError::NotConnected)
                && let Some(ref queue) = self.queue
            {
                queue.enqueue(event, self.url.as_str()).map_err(|queue_err| {
                    warn!("Failed to queue event for retry: {}", queue_err);
                    queue_err
                })?;
                info!("Event queued for retry: {}", event_id);
                // Return success since event is queued
                return Ok(PublishConfirmation {
                    event_id,
                    accepted: true,
                    message: "Queued for retry".to_string(),
                });
            }

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

    /// Subscribe with a callback for handling received events
    pub async fn subscribe_with_callback(
        &self,
        subscription_id: &str,
        filters: &[Value],
        callback: crate::subscription::EventCallback,
    ) -> Result<()> {
        // Create subscription
        let subscription = Subscription::with_callback(
            subscription_id.to_string(),
            filters.to_vec(),
            callback,
        );

        // Store subscription
        {
            let mut subs = self.subscriptions.lock().await;
            subs.insert(subscription_id.to_string(), subscription);
        }

        // Send REQ message to relay
        self.subscribe(subscription_id, filters).await
    }

    /// Subscribe and receive events through a channel
    pub async fn subscribe_with_channel(
        &self,
        subscription_id: &str,
        filters: &[Value],
    ) -> Result<mpsc::Receiver<Event>> {
        // Create subscription with channel
        let (subscription, rx) = Subscription::with_channel(
            subscription_id.to_string(),
            filters.to_vec(),
        );

        // Store subscription
        {
            let mut subs = self.subscriptions.lock().await;
            subs.insert(subscription_id.to_string(), subscription);
        }

        // Send REQ message to relay
        self.subscribe(subscription_id, filters).await?;

        Ok(rx)
    }

    /// Unsubscribe from a subscription
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        // Remove subscription
        {
            let mut subs = self.subscriptions.lock().await;
            subs.remove(subscription_id);
        }

        // Send CLOSE message to relay
        self.close_subscription(subscription_id).await
    }

    /// Get a subscription by ID
    pub async fn get_subscription(&self, subscription_id: &str) -> Option<Subscription> {
        let subs = self.subscriptions.lock().await;
        subs.get(subscription_id).cloned()
    }

    /// Get all active subscription IDs
    pub async fn active_subscriptions(&self) -> Vec<String> {
        let subs = self.subscriptions.lock().await;
        subs.keys().cloned().collect()
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
            match stream.send(Message::Text(msg_text)).await {
                Ok(()) => {
                    // Record successful send
                    let mut metrics = self.health_metrics.write().await;
                    metrics.successful_messages += 1;
                    drop(metrics);

                    self.circuit_breaker.record_success().await;
                    Ok(())
                }
                Err(e) => {
                    // Record failure
                    let mut metrics = self.health_metrics.write().await;
                    metrics.failed_messages += 1;
                    metrics.last_error = Some(e.to_string());
                    drop(metrics);

                    self.circuit_breaker.record_failure().await;
                    Err(ClientError::WebSocket(e.to_string()))
                }
            }
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

                    // Route EVENT messages to subscriptions
                    if let Some(RelayMessage::Event(sub_id, event)) = &msg {
                        let mut should_remove = false;
                        {
                            let subs = self.subscriptions.lock().await;
                            if let Some(subscription) = subs.get(sub_id) {
                                if let Err(e) = subscription.handle_event(event.clone()) {
                                    let err_str = e.to_string();
                                    if err_str.contains("channel closed") {
                                        debug!("Subscription {} channel closed, removing", sub_id);
                                        should_remove = true;
                                    } else {
                                        warn!("Error handling event for subscription {}: {}", sub_id, e);
                                    }
                                }
                            }
                        }
                        if should_remove {
                            self.subscriptions.lock().await.remove(sub_id);
                        }
                    }

                    // Handle EOSE messages for subscriptions
                    if let Some(RelayMessage::Eose(sub_id)) = &msg {
                        let subs = self.subscriptions.lock().await;
                        if let Some(subscription) = subs.get(sub_id) {
                            subscription.mark_eose();
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
                    let sub_id = arr[1]
                        .as_str()
                        .ok_or_else(|| ClientError::Protocol("EVENT subscription_id must be a string".into()))?
                        .to_string();
                    let event: Event = serde_json::from_value(arr[2].clone())?;
                    Ok(Some(RelayMessage::Event(sub_id, event)))
                } else {
                    Err(ClientError::Protocol("EVENT message requires at least 3 elements".into()))
                }
            }
            "OK" => {
                if arr.len() >= 4 {
                    let event_id = arr[1]
                        .as_str()
                        .ok_or_else(|| ClientError::Protocol("OK event_id must be a string".into()))?
                        .to_string();
                    let success = arr[2]
                        .as_bool()
                        .ok_or_else(|| ClientError::Protocol("OK accepted field must be a boolean".into()))?;
                    let message = arr[3]
                        .as_str()
                        .ok_or_else(|| ClientError::Protocol("OK message must be a string".into()))?
                        .to_string();
                    Ok(Some(RelayMessage::Ok(event_id, success, message)))
                } else {
                    Err(ClientError::Protocol("OK message requires at least 4 elements".into()))
                }
            }
            "EOSE" => {
                if arr.len() >= 2 {
                    let sub_id = arr[1]
                        .as_str()
                        .ok_or_else(|| ClientError::Protocol("EOSE subscription_id must be a string".into()))?
                        .to_string();
                    Ok(Some(RelayMessage::Eose(sub_id)))
                } else {
                    Err(ClientError::Protocol("EOSE message requires at least 2 elements".into()))
                }
            }
            "NOTICE" => {
                if arr.len() >= 2 {
                    let message = arr[1]
                        .as_str()
                        .ok_or_else(|| ClientError::Protocol("NOTICE message must be a string".into()))?
                        .to_string();
                    Ok(Some(RelayMessage::Notice(message)))
                } else {
                    Err(ClientError::Protocol("NOTICE message requires at least 2 elements".into()))
                }
            }
            "AUTH" => {
                if arr.len() >= 2 {
                    let challenge = arr[1]
                        .as_str()
                        .ok_or_else(|| ClientError::Protocol("AUTH challenge must be a string".into()))?
                        .to_string();
                    Ok(Some(RelayMessage::Auth(challenge)))
                } else {
                    Err(ClientError::Protocol("AUTH message requires at least 2 elements".into()))
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

    /// Get health metrics for this connection
    pub async fn health(&self) -> HealthMetrics {
        let mut metrics = self.health_metrics.read().await.clone();

        // Update uptime if connected
        if let Some(connected_at) = *self.connected_at.read().await {
            metrics.uptime = Some(connected_at.elapsed());
        }

        metrics
    }

    /// Check if relay connection is healthy
    pub async fn is_healthy(&self) -> bool {
        self.health_metrics.read().await.is_healthy()
    }

    /// Get circuit breaker state
    pub async fn circuit_state(&self) -> crate::recovery::CircuitState {
        self.circuit_breaker.state().await
    }

    /// Reset circuit breaker (use with caution)
    pub async fn reset_circuit(&self) {
        self.circuit_breaker.reset().await;
        let mut metrics = self.health_metrics.write().await;
        metrics.circuit_state = crate::recovery::CircuitState::Closed;
        metrics.last_error = None;
    }

    /// Attempt reconnection with exponential backoff
    ///
    /// Returns the delay that was waited before attempting reconnection,
    /// or None if max attempts exhausted.
    pub async fn reconnect_with_backoff(&self) -> Result<Option<Duration>> {
        // Check if already connected
        if self.is_connected().await {
            return Ok(Some(Duration::ZERO));
        }

        // Get next backoff delay
        let delay = {
            let mut backoff = self.backoff.lock().await;
            backoff.next_delay()
        };

        let Some(wait_duration) = delay else {
            warn!("Max reconnection attempts exhausted for {}", self.url);
            return Ok(None);
        };

        info!("Waiting {:?} before reconnecting to {}", wait_duration, self.url);
        tokio::time::sleep(wait_duration).await;

        // Attempt connection
        match self.connect().await {
            Ok(()) => {
                info!("Successfully reconnected to {}", self.url);
                Ok(Some(wait_duration))
            }
            Err(e) => {
                warn!("Reconnection attempt failed for {}: {}", self.url, e);
                Err(e)
            }
        }
    }

    /// Process queued messages and retry sending
    async fn process_queue(&self) -> Result<()> {
        let Some(ref queue) = self.queue else {
            return Ok(());
        };

        while let Some(queued_msg) = queue.dequeue()? {
            debug!("Processing queued message: {}", queued_msg.event_id);

            // Parse event from JSON
            let event: Event = match serde_json::from_str(&queued_msg.event_json) {
                Ok(e) => e,
                Err(parse_err) => {
                    warn!("Failed to parse queued event JSON: {}", parse_err);
                    queue.mark_failed(
                        queued_msg.id,
                        &format!("JSON parse error: {}", parse_err),
                    )?;
                    continue;
                }
            };

            // Try to send the event
            match self.send_event(&event).await {
                Ok(_) => {
                    info!("Successfully sent queued event: {}", event.id);
                    queue.mark_sent(queued_msg.id)?;
                }
                Err(e) => {
                    warn!(
                        "Failed to send queued event {}: {}",
                        event.id, e
                    );
                    queue.mark_failed(queued_msg.id, &e.to_string())?;
                }
            }
        }

        Ok(())
    }

    /// Start background task to process queue periodically
    fn start_queue_task(&self) {
        let Some(ref _queue) = self.queue else {
            return;
        };

        let url = self.url.clone();
        let state = Arc::clone(&self.state);
        let queue = Arc::clone(self.queue.as_ref().unwrap());
        let poll_interval = self.config.queue_poll_interval;
        let ws = Arc::clone(&self.ws);
        let health_metrics = Arc::clone(&self.health_metrics);
        let circuit_breaker = self.circuit_breaker.clone();

        // Spawn background task
        let task = tokio::spawn(async move {
            info!("Queue processing task started for {}", url);

            loop {
                // Check if still connected
                let current_state = *state.read().await;
                if current_state != ConnectionState::Connected {
                    debug!("Queue task paused - not connected");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                // Process queue
                while let Ok(Some(queued_msg)) = queue.dequeue() {
                    debug!("Processing queued message: {}", queued_msg.event_id);

                    // Parse event from JSON
                    let event: Event = match serde_json::from_str(&queued_msg.event_json) {
                        Ok(e) => e,
                        Err(parse_err) => {
                            warn!("Failed to parse queued event JSON: {}", parse_err);
                            if let Err(e) = queue.mark_failed(
                                queued_msg.id,
                                &format!("JSON parse error: {}", parse_err),
                            ) {
                                warn!("Failed to mark queued message as failed: {}", e);
                            }
                            continue;
                        }
                    };

                    // Check connection state again before sending
                    if *state.read().await != ConnectionState::Connected {
                        debug!("Connection lost during queue processing");
                        break;
                    }

                    // Send the event
                    let msg = json!(["EVENT", event]);
                    let msg_text = match serde_json::to_string(&msg) {
                        Ok(text) => text,
                        Err(e) => {
                            warn!("Failed to serialize event: {}", e);
                            if let Err(e) = queue.mark_failed(
                                queued_msg.id,
                                &format!("Serialization error: {}", e),
                            ) {
                                warn!("Failed to mark queued message as failed: {}", e);
                            }
                            continue;
                        }
                    };

                    debug!("Sending queued event to {}: {}", url, msg_text);

                    // Send through WebSocket
                    let mut ws_lock = ws.lock().await;
                    if let Some(stream) = ws_lock.as_mut() {
                        match stream.send(Message::Text(msg_text)).await {
                            Ok(()) => {
                                info!("Successfully sent queued event: {}", event.id);

                                // Record successful send
                                let mut metrics = health_metrics.write().await;
                                metrics.successful_messages += 1;
                                drop(metrics);

                                circuit_breaker.record_success().await;

                                // Mark as sent in queue
                                if let Err(e) = queue.mark_sent(queued_msg.id) {
                                    warn!("Failed to mark message as sent: {}", e);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to send queued event {}: {}", event.id, e);

                                // Record failure
                                let mut metrics = health_metrics.write().await;
                                metrics.failed_messages += 1;
                                metrics.last_error = Some(e.to_string());
                                drop(metrics);

                                circuit_breaker.record_failure().await;

                                // Mark as failed in queue (will retry if under limit)
                                if let Err(e) = queue.mark_failed(queued_msg.id, &e.to_string()) {
                                    warn!("Failed to mark message as failed: {}", e);
                                }
                            }
                        }
                    } else {
                        debug!("WebSocket not available, will retry later");
                        break;
                    }
                    drop(ws_lock);
                }

                // Sleep before next poll
                tokio::time::sleep(poll_interval).await;
            }
        });

        // Store task handle
        let queue_task = Arc::clone(&self.queue_task);
        tokio::spawn(async move {
            *queue_task.lock().await = Some(task);
        });
    }

    /// Stop queue processing task
    async fn stop_queue_task(&self) {
        let mut task = self.queue_task.lock().await;
        if let Some(handle) = task.take() {
            handle.abort();
            debug!("Queue processing task stopped");
        }
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

    #[tokio::test]
    async fn test_subscription_tracking() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();

        // Initially no subscriptions
        assert_eq!(relay.active_subscriptions().await.len(), 0);

        // Add a subscription manually
        {
            let subscription = Subscription::new(
                "test-sub".to_string(),
                vec![serde_json::json!({"kinds": [1]})],
            );
            let mut subs = relay.subscriptions.lock().await;
            subs.insert("test-sub".to_string(), subscription);
        }

        // Check it was added
        assert_eq!(relay.active_subscriptions().await.len(), 1);
        assert!(relay.get_subscription("test-sub").await.is_some());

        // Check subscription details
        let sub = relay.get_subscription("test-sub").await.unwrap();
        assert_eq!(sub.id(), "test-sub");
        assert_eq!(sub.filters().len(), 1);
    }

    #[tokio::test]
    async fn test_subscription_eose_handling() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();

        // Add a subscription
        {
            let subscription = Subscription::new(
                "test-sub".to_string(),
                vec![serde_json::json!({"kinds": [1]})],
            );
            let mut subs = relay.subscriptions.lock().await;
            subs.insert("test-sub".to_string(), subscription);
        }

        // Get subscription and check EOSE not received
        let sub = relay.get_subscription("test-sub").await.unwrap();
        assert!(!sub.has_eose());

        // Simulate EOSE message handling (what recv() does)
        {
            let subs = relay.subscriptions.lock().await;
            if let Some(subscription) = subs.get("test-sub") {
                subscription.mark_eose();
            }
        }

        // Check EOSE was marked
        let sub = relay.get_subscription("test-sub").await.unwrap();
        assert!(sub.has_eose());
    }

    #[tokio::test]
    async fn test_active_subscriptions() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();

        // Add multiple subscriptions
        {
            let sub1 = Subscription::new(
                "sub1".to_string(),
                vec![serde_json::json!({"kinds": [1]})],
            );
            let sub2 = Subscription::new(
                "sub2".to_string(),
                vec![serde_json::json!({"kinds": [3]})],
            );
            let mut subs = relay.subscriptions.lock().await;
            subs.insert("sub1".to_string(), sub1);
            subs.insert("sub2".to_string(), sub2);
        }

        // Check active subscriptions
        let active = relay.active_subscriptions().await;
        assert_eq!(active.len(), 2);
        assert!(active.contains(&"sub1".to_string()));
        assert!(active.contains(&"sub2".to_string()));
    }

    #[test]
    fn test_parse_eose_for_subscription() {
        let text = r#"["EOSE","my-subscription"]"#;
        let msg = RelayConnection::parse_relay_message(text).unwrap();
        assert!(msg.is_some());
        match msg.unwrap() {
            RelayMessage::Eose(sub_id) => {
                assert_eq!(sub_id, "my-subscription");
            }
            _ => panic!("Expected EOSE message"),
        }
    }

    #[tokio::test]
    async fn test_health_metrics_initialization() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();

        let health = relay.health().await;
        assert!(health.url.starts_with("wss://relay.example.com"));
        assert_eq!(health.successful_messages, 0);
        assert_eq!(health.failed_messages, 0);
        assert_eq!(health.success_rate(), 1.0);
        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn test_circuit_breaker_integration() {
        use crate::recovery::CircuitState;

        let relay = RelayConnection::new("wss://relay.example.com").unwrap();

        // Circuit should start closed
        assert_eq!(relay.circuit_state().await, CircuitState::Closed);

        // Simulate failures by directly accessing circuit breaker
        for _ in 0..5 {
            relay.circuit_breaker.record_failure().await;
        }

        // Circuit should now be open
        assert_eq!(relay.circuit_state().await, CircuitState::Open);

        // Reset circuit
        relay.reset_circuit().await;
        assert_eq!(relay.circuit_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_is_healthy_after_failures() {
        let relay = RelayConnection::new("wss://relay.example.com").unwrap();

        // Initially healthy
        assert!(relay.is_healthy().await);

        // Simulate failures
        {
            let mut metrics = relay.health_metrics.write().await;
            metrics.failed_messages = 5;
            metrics.successful_messages = 5;
        }

        // Success rate now 50%, should not be healthy (threshold is 80%)
        assert!(!relay.is_healthy().await);
    }
}
