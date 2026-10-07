//! Relay plumbing over `nostr-transport`: NIP-42 connections with a fixed
//! lifetime, frame parsing, publication that waits for `OK`, and one-shot
//! queries that read to `EOSE`.

use std::time::Duration;

use nostr::domain::Event;
use nostr_transport::Connection;
use serde_json::{Value, json};

use crate::identity::Identity;

/// How long one connection lives; `nostr-transport` allows at most 120 s.
pub const LIFETIME: Duration = Duration::from_secs(115);

/// One relay message the pylon cares about.
#[derive(Debug)]
pub enum Frame {
    Event {
        sub: String,
        event: Box<Event>,
    },
    Eose(String),
    Ok {
        id: String,
        accepted: bool,
        message: String,
    },
    Closed {
        sub: String,
        message: String,
    },
    Notice(String),
    Other,
}

impl Frame {
    /// Parse one relay message.
    #[must_use]
    pub fn parse(value: Value) -> Self {
        let Some(items) = value.as_array() else {
            return Self::Other;
        };
        let text = |i: usize| {
            items
                .get(i)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        match items.first().and_then(Value::as_str) {
            Some("EVENT") if items.len() == 3 => {
                match serde_json::from_value::<Event>(items[2].clone()) {
                    Ok(event) => Self::Event {
                        sub: text(1),
                        event: Box::new(event),
                    },
                    Err(_) => Self::Other,
                }
            }
            Some("EOSE") => Self::Eose(text(1)),
            Some("OK") => Self::Ok {
                id: text(1),
                accepted: items.get(2).and_then(Value::as_bool).unwrap_or(false),
                message: text(3),
            },
            Some("CLOSED") => Self::Closed {
                sub: text(1),
                message: text(2),
            },
            Some("NOTICE") => Self::Notice(text(1)),
            _ => Self::Other,
        }
    }
}

/// Connect to `url` and answer its NIP-42 challenge as `identity`.
///
/// # Errors
///
/// When the relay cannot be reached or does not acknowledge authentication.
pub async fn connect(
    url: &str,
    identity: &Identity,
    lifetime: Duration,
) -> Result<Connection, String> {
    Connection::connect(url, identity.secret(), lifetime)
        .await
        .map(|c| c.with_frame_budget(4096))
        .map_err(|e| format!("relay {url}: {e}"))
}

/// Publish `event` and wait for the relay's `OK`.
///
/// # Errors
///
/// When the relay refuses the event or the connection fails first.
pub async fn publish(conn: &mut Connection, event: &Event) -> Result<(), String> {
    conn.send(json!(["EVENT", event])).await?;
    loop {
        if let Frame::Ok {
            id,
            accepted,
            message,
        } = Frame::parse(conn.next().await?)
            && id == event.id
        {
            return if accepted {
                Ok(())
            } else {
                Err(format!("relay refused kind {}: {message}", event.kind))
            };
        }
    }
}

/// Run one query and return the stored events up to `EOSE`.
///
/// # Errors
///
/// When the relay closes the query or the connection fails.
pub async fn query(
    conn: &mut Connection,
    sub: &str,
    filters: &[Value],
) -> Result<Vec<Event>, String> {
    let mut request = vec![json!("REQ"), json!(sub)];
    request.extend(filters.iter().cloned());
    conn.send(Value::Array(request)).await?;
    let mut events = Vec::new();
    loop {
        match Frame::parse(conn.next().await?) {
            Frame::Event { sub: s, event } if s == sub => events.push(*event),
            Frame::Eose(s) if s == sub => break,
            Frame::Closed { sub: s, message } if s == sub => {
                return Err(format!("relay closed the query: {message}"));
            }
            _ => {}
        }
    }
    conn.send(json!(["CLOSE", sub])).await?;
    Ok(events)
}
