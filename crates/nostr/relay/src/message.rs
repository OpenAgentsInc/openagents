//! NIP-01 relay protocol messages.
//!
//! This module implements the Nostr relay protocol:
//!
//! **Client → Relay:**
//! - `EVENT`: Publish an event
//! - `REQ`: Subscribe to events matching filters
//! - `CLOSE`: Close a subscription
//!
//! **Relay → Client:**
//! - `EVENT`: Event matching a subscription
//! - `OK`: Command result (success/failure)
//! - `EOSE`: End of stored events
//! - `CLOSED`: Subscription closed by relay
//! - `NOTICE`: Human-readable message

use crate::Filter;
use nostr::Event;
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur when parsing messages.
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("invalid JSON: {0}")]
    InvalidJson(String),

    #[error("invalid message format: {0}")]
    InvalidFormat(String),

    #[error("unknown message type: {0}")]
    UnknownType(String),

    #[error("missing field: {0}")]
    MissingField(String),
}

/// Messages sent from client to relay.
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// Publish an event: `["EVENT", <event JSON>]`
    Event(Event),

    /// Subscribe to events: `["REQ", <subscription_id>, <filter1>, <filter2>, ...]`
    Req {
        subscription_id: String,
        filters: Vec<Filter>,
    },

    /// Close a subscription: `["CLOSE", <subscription_id>]`
    Close { subscription_id: String },

    /// Authentication (NIP-42): `["AUTH", <event JSON>]`
    Auth(Event),
}

impl ClientMessage {
    /// Parse a JSON message from the client.
    pub fn from_json(json: &str) -> Result<Self, MessageError> {
        let arr: Vec<Value> =
            serde_json::from_str(json).map_err(|e| MessageError::InvalidJson(e.to_string()))?;

        if arr.is_empty() {
            return Err(MessageError::InvalidFormat("empty array".to_string()));
        }

        let msg_type = arr[0]
            .as_str()
            .ok_or_else(|| MessageError::InvalidFormat("first element not a string".to_string()))?;

        match msg_type {
            "EVENT" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("event".to_string()));
                }
                let event: Event = serde_json::from_value(arr[1].clone())
                    .map_err(|e| MessageError::InvalidFormat(format!("invalid event: {}", e)))?;
                Ok(ClientMessage::Event(event))
            }
            "REQ" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("subscription_id".to_string()));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("subscription_id not a string".to_string())
                    })?
                    .to_string();

                let mut filters = Vec::new();
                for filter_val in arr.iter().skip(2) {
                    let filter: Filter =
                        serde_json::from_value(filter_val.clone()).map_err(|e| {
                            MessageError::InvalidFormat(format!("invalid filter: {}", e))
                        })?;
                    filters.push(filter);
                }

                Ok(ClientMessage::Req {
                    subscription_id,
                    filters,
                })
            }
            "CLOSE" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("subscription_id".to_string()));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("subscription_id not a string".to_string())
                    })?
                    .to_string();
                Ok(ClientMessage::Close { subscription_id })
            }
            "AUTH" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("auth event".to_string()));
                }
                let event: Event = serde_json::from_value(arr[1].clone()).map_err(|e| {
                    MessageError::InvalidFormat(format!("invalid auth event: {}", e))
                })?;
                Ok(ClientMessage::Auth(event))
            }
            _ => Err(MessageError::UnknownType(msg_type.to_string())),
        }
    }
}

/// Messages sent from relay to client.
#[derive(Debug, Clone)]
pub enum RelayMessage {
    /// Event matching a subscription: `["EVENT", <subscription_id>, <event JSON>]`
    Event {
        subscription_id: String,
        event: Event,
    },

    /// Command result: `["OK", <event_id>, <true|false>, <message>]`
    Ok {
        event_id: String,
        success: bool,
        message: String,
    },

    /// End of stored events: `["EOSE", <subscription_id>]`
    Eose { subscription_id: String },

    /// Subscription closed by relay: `["CLOSED", <subscription_id>, <message>]`
    Closed {
        subscription_id: String,
        message: String,
    },

    /// Human-readable notice: `["NOTICE", <message>]`
    Notice { message: String },

    /// Authentication challenge (NIP-42): `["AUTH", <challenge>]`
    Auth { challenge: String },

    /// Count response (NIP-45): `["COUNT", <subscription_id>, {"count": <n>}]`
    Count { subscription_id: String, count: u64 },
}

impl RelayMessage {
    /// Convert to JSON string for sending to client.
    pub fn to_json(&self) -> String {
        match self {
            RelayMessage::Event {
                subscription_id,
                event,
            } => serde_json::json!(["EVENT", subscription_id, event]).to_string(),

            RelayMessage::Ok {
                event_id,
                success,
                message,
            } => serde_json::json!(["OK", event_id, success, message]).to_string(),

            RelayMessage::Eose { subscription_id } => {
                serde_json::json!(["EOSE", subscription_id]).to_string()
            }

            RelayMessage::Closed {
                subscription_id,
                message,
            } => serde_json::json!(["CLOSED", subscription_id, message]).to_string(),

            RelayMessage::Notice { message } => serde_json::json!(["NOTICE", message]).to_string(),

            RelayMessage::Auth { challenge } => serde_json::json!(["AUTH", challenge]).to_string(),

            RelayMessage::Count {
                subscription_id,
                count,
            } => serde_json::json!(["COUNT", subscription_id, {"count": count}]).to_string(),
        }
    }

    /// Create an EVENT message.
    pub fn event(subscription_id: impl Into<String>, event: Event) -> Self {
        RelayMessage::Event {
            subscription_id: subscription_id.into(),
            event,
        }
    }

    /// Create an OK success response.
    pub fn ok_success(event_id: impl Into<String>) -> Self {
        RelayMessage::Ok {
            event_id: event_id.into(),
            success: true,
            message: String::new(),
        }
    }

    /// Create an OK failure response.
    pub fn ok_failure(event_id: impl Into<String>, reason: impl Into<String>) -> Self {
        RelayMessage::Ok {
            event_id: event_id.into(),
            success: false,
            message: reason.into(),
        }
    }

    /// Create an OK response for duplicate event.
    pub fn ok_duplicate(event_id: impl Into<String>) -> Self {
        RelayMessage::Ok {
            event_id: event_id.into(),
            success: true,
            message: "duplicate:".to_string(),
        }
    }

    /// Create an EOSE message.
    pub fn eose(subscription_id: impl Into<String>) -> Self {
        RelayMessage::Eose {
            subscription_id: subscription_id.into(),
        }
    }

    /// Create a CLOSED message.
    pub fn closed(subscription_id: impl Into<String>, message: impl Into<String>) -> Self {
        RelayMessage::Closed {
            subscription_id: subscription_id.into(),
            message: message.into(),
        }
    }

    /// Create a NOTICE message.
    pub fn notice(message: impl Into<String>) -> Self {
        RelayMessage::Notice {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_req() {
        let json = r#"["REQ", "sub1", {"kinds": [1], "limit": 10}]"#;
        let msg = ClientMessage::from_json(json).unwrap();

        match msg {
            ClientMessage::Req {
                subscription_id,
                filters,
            } => {
                assert_eq!(subscription_id, "sub1");
                assert_eq!(filters.len(), 1);
                assert_eq!(filters[0].kinds, Some(vec![1]));
                assert_eq!(filters[0].limit, Some(10));
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_parse_close() {
        let json = r#"["CLOSE", "sub1"]"#;
        let msg = ClientMessage::from_json(json).unwrap();

        match msg {
            ClientMessage::Close { subscription_id } => {
                assert_eq!(subscription_id, "sub1");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_json() {
        let msg = RelayMessage::ok_success("event123");
        let json = msg.to_json();
        assert!(json.contains("OK"));
        assert!(json.contains("event123"));
        assert!(json.contains("true"));

        let msg = RelayMessage::eose("sub1");
        let json = msg.to_json();
        assert_eq!(json, r#"["EOSE","sub1"]"#);
    }
}
