//! Single relay connection management.

use crate::error::{ClientError, Result};
use crate::subscription::Subscription;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use nostr::Event;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::timeout;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing::{debug, warn};
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsWriter = SplitSink<WsStream, Message>;

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

/// Relay message received from a relay.
#[derive(Debug, Clone)]
pub enum RelayMessage {
    Event(String, Event),
    Ok(String, bool, String),
    Eose(String),
    Notice(String),
    Auth(String),
}

/// Publish confirmation from a relay.
#[derive(Debug, Clone)]
pub struct PublishConfirmation {
    pub relay_url: String,
    pub event_id: String,
    pub accepted: bool,
    pub message: String,
}

/// Relay connection configuration.
#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub connect_timeout: Duration,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
        }
    }
}

/// Relay connection.
pub struct RelayConnection {
    url: Url,
    config: RelayConfig,
    state: Arc<RwLock<ConnectionState>>,
    writer: Arc<Mutex<Option<WsWriter>>>,
    incoming_tx: mpsc::UnboundedSender<RelayMessage>,
    incoming_rx: Arc<Mutex<mpsc::UnboundedReceiver<RelayMessage>>>,
    subscriptions: Arc<Mutex<HashMap<String, Subscription>>>,
    recv_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl RelayConnection {
    /// Create a new relay connection with default config.
    pub fn new(url: &str) -> Result<Self> {
        Self::with_config(url, RelayConfig::default())
    }

    /// Create a new relay connection with custom config.
    pub fn with_config(url: &str, config: RelayConfig) -> Result<Self> {
        let parsed_url = Url::parse(url)?;
        if parsed_url.scheme() != "ws" && parsed_url.scheme() != "wss" {
            return Err(ClientError::InvalidUrl(format!(
                "URL must use ws:// or wss:// scheme, got: {}",
                parsed_url.scheme()
            )));
        }

        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();

        Ok(Self {
            url: parsed_url,
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            writer: Arc::new(Mutex::new(None)),
            incoming_tx,
            incoming_rx: Arc::new(Mutex::new(incoming_rx)),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            recv_task: Arc::new(Mutex::new(None)),
        })
    }

    /// Relay URL as string.
    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    /// Current connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Connect to relay and start background receive loop.
    pub async fn connect(&self) -> Result<()> {
        let mut state_guard = self.state.write().await;
        if *state_guard == ConnectionState::Connected {
            return Err(ClientError::AlreadyConnected);
        }
        *state_guard = ConnectionState::Connecting;
        drop(state_guard);

        let connect_result = timeout(
            self.config.connect_timeout,
            connect_async(self.url.as_str()),
        )
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "connection timeout after {:?}",
                self.config.connect_timeout
            ))
        })?
        .map_err(|error| ClientError::WebSocket(error.to_string()))?;

        let (stream, _response) = connect_result;
        let (writer, mut reader) = stream.split();
        *self.writer.lock().await = Some(writer);
        *self.state.write().await = ConnectionState::Connected;

        let incoming_tx = self.incoming_tx.clone();
        let subscriptions = Arc::clone(&self.subscriptions);
        let state = Arc::clone(&self.state);
        let relay_url = self.url.to_string();

        let task = tokio::spawn(async move {
            while let Some(frame) = reader.next().await {
                match frame {
                    Ok(Message::Text(text)) => match parse_relay_message(text.as_str()) {
                        Ok(Some(RelayMessage::Event(subscription_id, event))) => {
                            let subscription =
                                { subscriptions.lock().await.get(&subscription_id).cloned() };
                            if let Some(subscription) = subscription
                                && let Err(error) = subscription.handle_event(event.clone())
                            {
                                warn!("subscription callback error on {}: {}", relay_url, error);
                            }
                            if incoming_tx
                                .send(RelayMessage::Event(subscription_id, event))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Some(RelayMessage::Eose(subscription_id))) => {
                            if let Some(subscription) =
                                subscriptions.lock().await.get(&subscription_id).cloned()
                            {
                                subscription.mark_eose();
                            }
                            if incoming_tx
                                .send(RelayMessage::Eose(subscription_id))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Some(message)) => {
                            if incoming_tx.send(message).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            warn!("protocol parse error on {}: {}", relay_url, error);
                            if incoming_tx
                                .send(RelayMessage::Notice(format!("parse error: {}", error)))
                                .is_err()
                            {
                                break;
                            }
                        }
                    },
                    Ok(Message::Ping(payload)) => {
                        debug!("received ping from {} ({} bytes)", relay_url, payload.len());
                    }
                    Ok(Message::Pong(_)) => {}
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Binary(_)) => {}
                    Ok(Message::Frame(_)) => {}
                    Err(error) => {
                        warn!("websocket read error on {}: {}", relay_url, error);
                        break;
                    }
                }
            }

            *state.write().await = ConnectionState::Disconnected;
        });

        *self.recv_task.lock().await = Some(task);
        Ok(())
    }

    /// Disconnect from relay and stop background tasks.
    pub async fn disconnect(&self) -> Result<()> {
        if let Some(mut writer) = self.writer.lock().await.take() {
            writer
                .send(Message::Close(None))
                .await
                .map_err(|error| ClientError::WebSocket(error.to_string()))?;
        }

        if let Some(task) = self.recv_task.lock().await.take() {
            task.abort();
        }

        *self.state.write().await = ConnectionState::Disconnected;
        Ok(())
    }

    /// Publish event to relay.
    pub async fn publish(&self, event: &Event) -> Result<PublishConfirmation> {
        self.send_json(&json!(["EVENT", event])).await?;
        Ok(PublishConfirmation {
            relay_url: self.url.to_string(),
            event_id: event.id.clone(),
            accepted: true,
            message: "queued".to_string(),
        })
    }

    /// Register and send subscription request.
    pub async fn subscribe(&self, subscription: Subscription) -> Result<()> {
        self.send_json(&json!(["REQ", subscription.id, subscription.filters]))
            .await?;
        self.subscriptions
            .lock()
            .await
            .insert(subscription.id.clone(), subscription);
        Ok(())
    }

    /// Subscribe with raw filters.
    pub async fn subscribe_filters(
        &self,
        subscription_id: impl Into<String>,
        filters: Vec<Value>,
    ) -> Result<()> {
        self.subscribe(Subscription::new(subscription_id.into(), filters))
            .await
    }

    /// Close subscription on relay.
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        self.send_json(&json!(["CLOSE", subscription_id])).await?;
        self.subscriptions.lock().await.remove(subscription_id);
        Ok(())
    }

    /// Receive next message from relay.
    pub async fn recv(&self) -> Result<Option<RelayMessage>> {
        Ok(self.incoming_rx.lock().await.recv().await)
    }

    async fn send_json(&self, value: &Value) -> Result<()> {
        if self.state().await != ConnectionState::Connected {
            return Err(ClientError::NotConnected);
        }
        let text = serde_json::to_string(value)?;
        self.send_text(text).await
    }

    async fn send_text(&self, text: String) -> Result<()> {
        let mut writer_guard = self.writer.lock().await;
        let writer = writer_guard.as_mut().ok_or(ClientError::NotConnected)?;
        writer
            .send(Message::Text(text.into()))
            .await
            .map_err(|error| ClientError::WebSocket(error.to_string()))
    }
}

/// Parse relay protocol JSON text message into typed relay message.
pub fn parse_relay_message(text: &str) -> Result<Option<RelayMessage>> {
    let value: Value = serde_json::from_str(text)?;
    let array = value
        .as_array()
        .ok_or_else(|| ClientError::Protocol("expected JSON array relay message".to_string()))?;
    if array.is_empty() {
        return Ok(None);
    }

    let kind = array[0]
        .as_str()
        .ok_or_else(|| ClientError::Protocol("missing relay message kind".to_string()))?;

    match kind {
        "EVENT" => {
            if array.len() < 3 {
                return Err(ClientError::Protocol("invalid EVENT message".to_string()));
            }
            let subscription_id = array[1]
                .as_str()
                .ok_or_else(|| ClientError::Protocol("invalid EVENT subscription id".to_string()))?
                .to_string();
            let event: Event = serde_json::from_value(array[2].clone()).map_err(|error| {
                ClientError::Protocol(format!("invalid EVENT payload: {}", error))
            })?;
            Ok(Some(RelayMessage::Event(subscription_id, event)))
        }
        "OK" => {
            if array.len() < 4 {
                return Err(ClientError::Protocol("invalid OK message".to_string()));
            }
            let event_id = array[1]
                .as_str()
                .ok_or_else(|| ClientError::Protocol("invalid OK event id".to_string()))?
                .to_string();
            let accepted = array[2]
                .as_bool()
                .ok_or_else(|| ClientError::Protocol("invalid OK accepted flag".to_string()))?;
            let message = array[3]
                .as_str()
                .ok_or_else(|| ClientError::Protocol("invalid OK message text".to_string()))?
                .to_string();
            Ok(Some(RelayMessage::Ok(event_id, accepted, message)))
        }
        "EOSE" => {
            if array.len() < 2 {
                return Err(ClientError::Protocol("invalid EOSE message".to_string()));
            }
            let subscription_id = array[1]
                .as_str()
                .ok_or_else(|| ClientError::Protocol("invalid EOSE subscription id".to_string()))?
                .to_string();
            Ok(Some(RelayMessage::Eose(subscription_id)))
        }
        "NOTICE" => {
            if array.len() < 2 {
                return Err(ClientError::Protocol("invalid NOTICE message".to_string()));
            }
            let message = array[1]
                .as_str()
                .ok_or_else(|| ClientError::Protocol("invalid NOTICE message text".to_string()))?
                .to_string();
            Ok(Some(RelayMessage::Notice(message)))
        }
        "AUTH" => {
            if array.len() < 2 {
                return Err(ClientError::Protocol("invalid AUTH message".to_string()));
            }
            let challenge = array[1]
                .as_str()
                .ok_or_else(|| ClientError::Protocol("invalid AUTH challenge".to_string()))?
                .to_string();
            Ok(Some(RelayMessage::Auth(challenge)))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> Event {
        Event {
            id: "id".to_string(),
            pubkey: "pubkey".to_string(),
            created_at: 1,
            kind: 1,
            tags: vec![],
            content: "hello".to_string(),
            sig: "sig".to_string(),
        }
    }

    fn assert_relay_message(actual: RelayMessage, expected: RelayMessage) {
        assert_eq!(
            std::mem::discriminant(&actual),
            std::mem::discriminant(&expected),
            "mismatched relay message variants: actual={actual:?}, expected={expected:?}"
        );

        match (actual, expected) {
            (
                RelayMessage::Event(actual_sub, actual_event),
                RelayMessage::Event(expected_sub, expected_event),
            ) => {
                assert_eq!(actual_sub, expected_sub);
                assert_eq!(actual_event.id, expected_event.id);
                assert_eq!(actual_event.pubkey, expected_event.pubkey);
                assert_eq!(actual_event.created_at, expected_event.created_at);
                assert_eq!(actual_event.kind, expected_event.kind);
                assert_eq!(actual_event.tags, expected_event.tags);
                assert_eq!(actual_event.content, expected_event.content);
                assert_eq!(actual_event.sig, expected_event.sig);
            }
            (
                RelayMessage::Ok(actual_id, actual_accepted, actual_message),
                RelayMessage::Ok(expected_id, expected_accepted, expected_message),
            ) => {
                assert_eq!(actual_id, expected_id);
                assert_eq!(actual_accepted, expected_accepted);
                assert_eq!(actual_message, expected_message);
            }
            (RelayMessage::Eose(actual_sub), RelayMessage::Eose(expected_sub)) => {
                assert_eq!(actual_sub, expected_sub);
            }
            (RelayMessage::Notice(actual_message), RelayMessage::Notice(expected_message)) => {
                assert_eq!(actual_message, expected_message);
            }
            (RelayMessage::Auth(actual_challenge), RelayMessage::Auth(expected_challenge)) => {
                assert_eq!(actual_challenge, expected_challenge);
            }
            _ => {}
        }
    }

    fn encode_inbound_message(message: &RelayMessage) -> Value {
        match message {
            RelayMessage::Event(subscription_id, event) => {
                json!(["EVENT", subscription_id, event])
            }
            RelayMessage::Ok(event_id, accepted, message_text) => {
                json!(["OK", event_id, accepted, message_text])
            }
            RelayMessage::Eose(subscription_id) => json!(["EOSE", subscription_id]),
            RelayMessage::Notice(message_text) => json!(["NOTICE", message_text]),
            RelayMessage::Auth(challenge) => json!(["AUTH", challenge]),
        }
    }

    #[test]
    fn parse_known_message_kinds() -> Result<()> {
        let expected_messages = vec![
            RelayMessage::Event("sub".to_string(), sample_event()),
            RelayMessage::Ok("event-id".to_string(), true, "accepted".to_string()),
            RelayMessage::Eose("sub".to_string()),
            RelayMessage::Notice("relay notice".to_string()),
            RelayMessage::Auth("challenge-token".to_string()),
        ];

        for expected in expected_messages {
            let text = serde_json::to_string(&encode_inbound_message(&expected))?;
            let parsed = parse_relay_message(&text)?;
            let actual = parsed.ok_or_else(|| {
                ClientError::Internal("expected parsed relay message, got None".to_string())
            })?;
            assert_relay_message(actual, expected);
        }

        Ok(())
    }

    #[test]
    fn parse_unknown_message_kind_returns_none() -> Result<()> {
        let parsed = parse_relay_message(r#"["UNKNOWN","data"]"#)?;
        assert!(parsed.is_none());
        Ok(())
    }

    #[test]
    fn parse_malformed_structures() {
        struct Case {
            name: &'static str,
            input: &'static str,
            expected_error_fragment: &'static str,
        }

        let cases = vec![
            Case {
                name: "non-array payload",
                input: r#"{"kind":"EVENT"}"#,
                expected_error_fragment: "expected JSON array relay message",
            },
            Case {
                name: "kind is not string",
                input: "[123]",
                expected_error_fragment: "missing relay message kind",
            },
            Case {
                name: "event too short",
                input: r#"["EVENT","sub"]"#,
                expected_error_fragment: "invalid EVENT message",
            },
            Case {
                name: "ok too short",
                input: r#"["OK","event-id",true]"#,
                expected_error_fragment: "invalid OK message",
            },
            Case {
                name: "eose too short",
                input: r#"["EOSE"]"#,
                expected_error_fragment: "invalid EOSE message",
            },
            Case {
                name: "notice too short",
                input: r#"["NOTICE"]"#,
                expected_error_fragment: "invalid NOTICE message",
            },
            Case {
                name: "auth too short",
                input: r#"["AUTH"]"#,
                expected_error_fragment: "invalid AUTH message",
            },
        ];

        let empty = parse_relay_message("[]");
        assert!(
            matches!(empty, Ok(None)),
            "empty array should parse to None"
        );

        for case in cases {
            let result = parse_relay_message(case.input);
            assert!(result.is_err(), "{}: expected an error", case.name);

            if let Err(error) = result {
                let rendered = error.to_string();
                assert!(
                    rendered.contains(case.expected_error_fragment),
                    "{}: expected error fragment '{}' in '{}'",
                    case.name,
                    case.expected_error_fragment,
                    rendered
                );
            }
        }
    }

    #[test]
    fn parse_malformed_field_types() {
        struct Case {
            name: &'static str,
            input: &'static str,
            expected_error_fragment: &'static str,
        }

        let cases = vec![
            Case {
                name: "event subscription id type",
                input: r#"["EVENT",123,{"id":"id"}]"#,
                expected_error_fragment: "invalid EVENT subscription id",
            },
            Case {
                name: "event payload shape",
                input: r#"["EVENT","sub",{"id":"id"}]"#,
                expected_error_fragment: "invalid EVENT payload",
            },
            Case {
                name: "ok event id type",
                input: r#"["OK",123,true,"accepted"]"#,
                expected_error_fragment: "invalid OK event id",
            },
            Case {
                name: "ok accepted type",
                input: r#"["OK","event-id","yes","accepted"]"#,
                expected_error_fragment: "invalid OK accepted flag",
            },
            Case {
                name: "ok message text type",
                input: r#"["OK","event-id",true,42]"#,
                expected_error_fragment: "invalid OK message text",
            },
            Case {
                name: "eose subscription id type",
                input: r#"["EOSE",42]"#,
                expected_error_fragment: "invalid EOSE subscription id",
            },
            Case {
                name: "notice text type",
                input: r#"["NOTICE",{"text":"msg"}]"#,
                expected_error_fragment: "invalid NOTICE message text",
            },
            Case {
                name: "auth challenge type",
                input: r#"["AUTH",{"challenge":"token"}]"#,
                expected_error_fragment: "invalid AUTH challenge",
            },
        ];

        for case in cases {
            let result = parse_relay_message(case.input);
            assert!(result.is_err(), "{}: expected an error", case.name);

            if let Err(error) = result {
                let rendered = error.to_string();
                assert!(
                    rendered.contains(case.expected_error_fragment),
                    "{}: expected error fragment '{}' in '{}'",
                    case.name,
                    case.expected_error_fragment,
                    rendered
                );
            }
        }
    }

    #[test]
    fn inbound_roundtrip_preserves_messages() -> Result<()> {
        let messages = vec![
            RelayMessage::Event("sub".to_string(), sample_event()),
            RelayMessage::Ok("event-id".to_string(), false, "rejected".to_string()),
            RelayMessage::Eose("sub-2".to_string()),
            RelayMessage::Notice("maintenance".to_string()),
            RelayMessage::Auth("challenge-2".to_string()),
        ];

        for expected in messages {
            let encoded = encode_inbound_message(&expected);
            let text = serde_json::to_string(&encoded)?;
            let parsed = parse_relay_message(&text)?;
            let actual = parsed.ok_or_else(|| {
                ClientError::Internal("expected parsed relay message, got None".to_string())
            })?;
            assert_relay_message(actual, expected);
        }

        Ok(())
    }
}
