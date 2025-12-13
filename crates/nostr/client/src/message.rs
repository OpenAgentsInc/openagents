//! Nostr relay message types.
//!
//! This module implements the relay protocol messages as specified in NIP-01:
//! - Client to Relay: EVENT, REQ, CLOSE
//! - Relay to Client: EVENT, OK, EOSE, CLOSED, NOTICE

use nostr::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur when parsing relay messages.
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("invalid message format: {0}")]
    InvalidFormat(String),

    #[error("unknown message type: {0}")]
    UnknownType(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("missing field: {0}")]
    MissingField(String),
}

/// Messages sent from client to relay.
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// Publish an event: ["EVENT", <event JSON>]
    Event(Event),

    /// Subscribe to events: ["REQ", <subscription_id>, <filter1>, <filter2>, ...]
    Req {
        subscription_id: String,
        filters: Vec<Filter>,
    },

    /// Close a subscription: ["CLOSE", <subscription_id>]
    Close { subscription_id: String },

    /// Authentication (NIP-42): ["AUTH", <event JSON>]
    Auth(Event),
}

impl ClientMessage {
    /// Serialize to JSON array for sending to relay.
    pub fn to_json(&self) -> Result<String, MessageError> {
        let value = match self {
            ClientMessage::Event(event) => {
                serde_json::json!(["EVENT", event])
            }
            ClientMessage::Req {
                subscription_id,
                filters,
            } => {
                let mut arr: Vec<Value> = vec![
                    Value::String("REQ".to_string()),
                    Value::String(subscription_id.clone()),
                ];
                for filter in filters {
                    arr.push(serde_json::to_value(filter)?);
                }
                Value::Array(arr)
            }
            ClientMessage::Close { subscription_id } => {
                serde_json::json!(["CLOSE", subscription_id])
            }
            ClientMessage::Auth(event) => {
                serde_json::json!(["AUTH", event])
            }
        };
        Ok(value.to_string())
    }
}

/// Messages sent from relay to client.
#[derive(Debug, Clone)]
pub enum RelayMessage {
    /// Event matching a subscription: ["EVENT", <subscription_id>, <event JSON>]
    Event {
        subscription_id: String,
        event: Event,
    },

    /// Command result: ["OK", <event_id>, <true|false>, <message>]
    Ok {
        event_id: String,
        success: bool,
        message: String,
    },

    /// End of stored events: ["EOSE", <subscription_id>]
    Eose { subscription_id: String },

    /// Subscription closed by relay: ["CLOSED", <subscription_id>, <message>]
    Closed {
        subscription_id: String,
        message: String,
    },

    /// Human-readable notice: ["NOTICE", <message>]
    Notice { message: String },

    /// Authentication challenge (NIP-42): ["AUTH", <challenge>]
    Auth { challenge: String },

    /// Count response (NIP-45): ["COUNT", <subscription_id>, {"count": <n>}]
    Count { subscription_id: String, count: u64 },
}

impl RelayMessage {
    /// Parse a JSON message from the relay.
    pub fn from_json(json: &str) -> Result<Self, MessageError> {
        let arr: Vec<Value> =
            serde_json::from_str(json).map_err(|e| MessageError::InvalidFormat(e.to_string()))?;

        if arr.is_empty() {
            return Err(MessageError::InvalidFormat("empty array".to_string()));
        }

        let msg_type = arr[0]
            .as_str()
            .ok_or_else(|| MessageError::InvalidFormat("first element not a string".to_string()))?;

        match msg_type {
            "EVENT" => {
                if arr.len() < 3 {
                    return Err(MessageError::MissingField(
                        "event or subscription_id".to_string(),
                    ));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("subscription_id not a string".to_string())
                    })?
                    .to_string();
                let event: Event = serde_json::from_value(arr[2].clone())?;
                Ok(RelayMessage::Event {
                    subscription_id,
                    event,
                })
            }
            "OK" => {
                if arr.len() < 4 {
                    return Err(MessageError::MissingField("OK fields".to_string()));
                }
                let event_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("event_id not a string".to_string())
                    })?
                    .to_string();
                let success = arr[2].as_bool().ok_or_else(|| {
                    MessageError::InvalidFormat("success not a boolean".to_string())
                })?;
                let message = arr[3].as_str().unwrap_or("").to_string();
                Ok(RelayMessage::Ok {
                    event_id,
                    success,
                    message,
                })
            }
            "EOSE" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("subscription_id".to_string()));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("subscription_id not a string".to_string())
                    })?
                    .to_string();
                Ok(RelayMessage::Eose { subscription_id })
            }
            "CLOSED" => {
                if arr.len() < 3 {
                    return Err(MessageError::MissingField("CLOSED fields".to_string()));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("subscription_id not a string".to_string())
                    })?
                    .to_string();
                let message = arr[2].as_str().unwrap_or("").to_string();
                Ok(RelayMessage::Closed {
                    subscription_id,
                    message,
                })
            }
            "NOTICE" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("message".to_string()));
                }
                let message = arr[1]
                    .as_str()
                    .ok_or_else(|| MessageError::InvalidFormat("message not a string".to_string()))?
                    .to_string();
                Ok(RelayMessage::Notice { message })
            }
            "AUTH" => {
                if arr.len() < 2 {
                    return Err(MessageError::MissingField("challenge".to_string()));
                }
                let challenge = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("challenge not a string".to_string())
                    })?
                    .to_string();
                Ok(RelayMessage::Auth { challenge })
            }
            "COUNT" => {
                if arr.len() < 3 {
                    return Err(MessageError::MissingField("COUNT fields".to_string()));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| {
                        MessageError::InvalidFormat("subscription_id not a string".to_string())
                    })?
                    .to_string();
                let count_obj = arr[2].as_object().ok_or_else(|| {
                    MessageError::InvalidFormat("count not an object".to_string())
                })?;
                let count = count_obj
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| MessageError::MissingField("count value".to_string()))?;
                Ok(RelayMessage::Count {
                    subscription_id,
                    count,
                })
            }
            _ => Err(MessageError::UnknownType(msg_type.to_string())),
        }
    }
}

/// Filter for subscription requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Event IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,

    /// Authors (pubkeys)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    /// Event kinds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,

    /// Events since timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,

    /// Events until timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,

    /// Maximum number of events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,

    /// Generic tag queries (e.g., #e, #p)
    /// The key is the tag letter (without #), value is list of values
    #[serde(flatten, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub tags: std::collections::HashMap<String, Vec<String>>,
}

impl Filter {
    /// Create a new empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by event IDs.
    pub fn ids(mut self, ids: Vec<String>) -> Self {
        self.ids = Some(ids);
        self
    }

    /// Filter by authors.
    pub fn authors(mut self, authors: Vec<String>) -> Self {
        self.authors = Some(authors);
        self
    }

    /// Filter by kinds.
    pub fn kinds(mut self, kinds: Vec<u16>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    /// Filter by events since timestamp.
    pub fn since(mut self, timestamp: u64) -> Self {
        self.since = Some(timestamp);
        self
    }

    /// Filter by events until timestamp.
    pub fn until(mut self, timestamp: u64) -> Self {
        self.until = Some(timestamp);
        self
    }

    /// Limit number of results.
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Add a tag filter. The key should be the tag letter (e.g., "e", "p").
    pub fn tag(mut self, key: impl Into<String>, values: Vec<String>) -> Self {
        self.tags.insert(format!("#{}", key.into()), values);
        self
    }

    /// Filter by #e (event reference) tags.
    pub fn event_refs(self, event_ids: Vec<String>) -> Self {
        self.tag("e", event_ids)
    }

    /// Filter by #p (pubkey reference) tags.
    pub fn pubkey_refs(self, pubkeys: Vec<String>) -> Self {
        self.tag("p", pubkeys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_event() {
        let event = Event {
            id: "abc123".to_string(),
            pubkey: "pubkey123".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello".to_string(),
            sig: "sig123".to_string(),
        };

        let msg = ClientMessage::Event(event);
        let json = msg.to_json().unwrap();

        assert!(json.contains("EVENT"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_client_message_req() {
        let filter = Filter::new().kinds(vec![1]).limit(10);

        let msg = ClientMessage::Req {
            subscription_id: "sub1".to_string(),
            filters: vec![filter],
        };

        let json = msg.to_json().unwrap();
        assert!(json.contains("REQ"));
        assert!(json.contains("sub1"));
        assert!(json.contains("kinds"));
    }

    #[test]
    fn test_client_message_close() {
        let msg = ClientMessage::Close {
            subscription_id: "sub1".to_string(),
        };

        let json = msg.to_json().unwrap();
        assert_eq!(json, r#"["CLOSE","sub1"]"#);
    }

    #[test]
    fn test_relay_message_event() {
        let json = r#"["EVENT","sub1",{"id":"abc","pubkey":"pk","created_at":123,"kind":1,"tags":[],"content":"Hello","sig":"sig"}]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Event {
                subscription_id,
                event,
            } => {
                assert_eq!(subscription_id, "sub1");
                assert_eq!(event.id, "abc");
                assert_eq!(event.content, "Hello");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_ok_success() {
        let json = r#"["OK","event123",true,""]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Ok {
                event_id,
                success,
                message,
            } => {
                assert_eq!(event_id, "event123");
                assert!(success);
                assert_eq!(message, "");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_ok_failure() {
        let json = r#"["OK","event123",false,"duplicate: already have this event"]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Ok {
                event_id,
                success,
                message,
            } => {
                assert_eq!(event_id, "event123");
                assert!(!success);
                assert!(message.contains("duplicate"));
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_eose() {
        let json = r#"["EOSE","sub1"]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Eose { subscription_id } => {
                assert_eq!(subscription_id, "sub1");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_closed() {
        let json = r#"["CLOSED","sub1","error: too many subscriptions"]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Closed {
                subscription_id,
                message,
            } => {
                assert_eq!(subscription_id, "sub1");
                assert!(message.contains("too many subscriptions"));
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_notice() {
        let json = r#"["NOTICE","rate limited"]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Notice { message } => {
                assert_eq!(message, "rate limited");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_auth() {
        let json = r#"["AUTH","challenge123"]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Auth { challenge } => {
                assert_eq!(challenge, "challenge123");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_relay_message_count() {
        let json = r#"["COUNT","sub1",{"count":42}]"#;
        let msg = RelayMessage::from_json(json).unwrap();

        match msg {
            RelayMessage::Count {
                subscription_id,
                count,
            } => {
                assert_eq!(subscription_id, "sub1");
                assert_eq!(count, 42);
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_filter_builder() {
        let filter = Filter::new()
            .kinds(vec![1, 4])
            .authors(vec!["author1".to_string()])
            .since(1000)
            .until(2000)
            .limit(100)
            .event_refs(vec!["event1".to_string()]);

        assert_eq!(filter.kinds, Some(vec![1, 4]));
        assert_eq!(filter.authors, Some(vec!["author1".to_string()]));
        assert_eq!(filter.since, Some(1000));
        assert_eq!(filter.until, Some(2000));
        assert_eq!(filter.limit, Some(100));
        assert!(filter.tags.contains_key("#e"));
    }

    #[test]
    fn test_filter_serialization() {
        let filter = Filter::new().kinds(vec![1]).limit(10);

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"kinds\":[1]"));
        assert!(json.contains("\"limit\":10"));
        // Should not include None fields
        assert!(!json.contains("authors"));
    }

    #[test]
    fn test_invalid_message() {
        let result = RelayMessage::from_json("not valid json");
        assert!(result.is_err());

        let result = RelayMessage::from_json("[]");
        assert!(result.is_err());

        let result = RelayMessage::from_json(r#"["UNKNOWN"]"#);
        assert!(result.is_err());
    }
}
