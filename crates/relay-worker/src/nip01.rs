//! NIP-01: Basic Protocol
//!
//! Client-to-relay and relay-to-client message types.

use serde::{Deserialize, Serialize};

use crate::subscription::Filter;

/// Client-to-relay message
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// ["EVENT", <event>]
    Event { event: nostr::Event },
    /// ["REQ", <subscription_id>, <filter>...]
    Req {
        subscription_id: String,
        filters: Vec<Filter>,
    },
    /// ["CLOSE", <subscription_id>]
    Close { subscription_id: String },
    /// ["AUTH", <event>]
    Auth { event: nostr::Event },
}

impl<'de> Deserialize<'de> for ClientMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let arr: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
        if arr.is_empty() {
            return Err(D::Error::custom("empty message"));
        }

        let cmd = arr[0].as_str().ok_or_else(|| D::Error::custom("first element must be string"))?;

        match cmd {
            "EVENT" => {
                if arr.len() < 2 {
                    return Err(D::Error::custom("EVENT requires event"));
                }
                let event: nostr::Event = serde_json::from_value(arr[1].clone())
                    .map_err(|e| D::Error::custom(format!("invalid event: {}", e)))?;
                Ok(ClientMessage::Event { event })
            }
            "REQ" => {
                if arr.len() < 3 {
                    return Err(D::Error::custom("REQ requires subscription_id and at least one filter"));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| D::Error::custom("subscription_id must be string"))?
                    .to_string();
                let mut filters = Vec::new();
                for filter_val in &arr[2..] {
                    let filter: Filter = serde_json::from_value(filter_val.clone())
                        .map_err(|e| D::Error::custom(format!("invalid filter: {}", e)))?;
                    filters.push(filter);
                }
                Ok(ClientMessage::Req {
                    subscription_id,
                    filters,
                })
            }
            "CLOSE" => {
                if arr.len() < 2 {
                    return Err(D::Error::custom("CLOSE requires subscription_id"));
                }
                let subscription_id = arr[1]
                    .as_str()
                    .ok_or_else(|| D::Error::custom("subscription_id must be string"))?
                    .to_string();
                Ok(ClientMessage::Close { subscription_id })
            }
            "AUTH" => {
                if arr.len() < 2 {
                    return Err(D::Error::custom("AUTH requires event"));
                }
                let event: nostr::Event = serde_json::from_value(arr[1].clone())
                    .map_err(|e| D::Error::custom(format!("invalid auth event: {}", e)))?;
                Ok(ClientMessage::Auth { event })
            }
            _ => Err(D::Error::custom(format!("unknown command: {}", cmd))),
        }
    }
}

/// Relay-to-client message
#[derive(Debug, Clone)]
pub enum RelayMessage {
    /// ["EVENT", <subscription_id>, <event>]
    Event {
        subscription_id: String,
        event: nostr::Event,
    },
    /// ["OK", <event_id>, <accepted>, <message>]
    Ok {
        event_id: String,
        accepted: bool,
        message: String,
    },
    /// ["EOSE", <subscription_id>]
    Eose { subscription_id: String },
    /// ["CLOSED", <subscription_id>, <message>]
    Closed {
        subscription_id: String,
        message: String,
    },
    /// ["NOTICE", <message>]
    Notice { message: String },
    /// ["AUTH", <challenge>]
    Auth { challenge: String },
}

impl Serialize for RelayMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        match self {
            RelayMessage::Event {
                subscription_id,
                event,
            } => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element("EVENT")?;
                seq.serialize_element(subscription_id)?;
                seq.serialize_element(event)?;
                seq.end()
            }
            RelayMessage::Ok {
                event_id,
                accepted,
                message,
            } => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element("OK")?;
                seq.serialize_element(event_id)?;
                seq.serialize_element(accepted)?;
                seq.serialize_element(message)?;
                seq.end()
            }
            RelayMessage::Eose { subscription_id } => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("EOSE")?;
                seq.serialize_element(subscription_id)?;
                seq.end()
            }
            RelayMessage::Closed {
                subscription_id,
                message,
            } => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element("CLOSED")?;
                seq.serialize_element(subscription_id)?;
                seq.serialize_element(message)?;
                seq.end()
            }
            RelayMessage::Notice { message } => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("NOTICE")?;
                seq.serialize_element(message)?;
                seq.end()
            }
            RelayMessage::Auth { challenge } => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("AUTH")?;
                seq.serialize_element(challenge)?;
                seq.end()
            }
        }
    }
}
