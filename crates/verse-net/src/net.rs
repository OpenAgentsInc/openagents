//! Bounded relay connections on a cancellable background runtime.
//! Dropping a link signals cancellation without waiting on the native UI thread.
//!
//! A browser build's [`Link`] is the browser's own WebSocket, driven from
//! its callbacks on the page's thread; both links share the protocol state
//! in `wire`.
use nostr::domain::Event;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use std::collections::VecDeque;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(not(target_arch = "wasm32"))]
mod relay;
mod wire;
#[cfg(target_arch = "wasm32")]
pub use browser::Link;
#[cfg(not(target_arch = "wasm32"))]
pub use relay::Link;
#[cfg(test)]
use relay::*;
#[cfg(test)]
use wire::*;

const QUEUE: usize = 64;
/// Messages the relay may leave for the session between two frames. A world
/// subscription's history (up to 500 state events) and the crowd's pose
/// frames arrive in one burst; a smaller inbox dropped the connection.
const INBOX: usize = 4096;
const MAX_WIRE: usize = 64 * 1024;
/// A command from the game to the relay.
#[derive(Clone, Debug)]
pub enum Out {
    /// Publish a signed event.
    Publish(Event),
    /// Open or replace a subscription. Unfinished queries are restored after
    /// reconnect or authentication. One-shot queries close after EOSE.
    Subscribe {
        /// Subscription id.
        id: String,
        /// NIP-01 filters.
        filters: Vec<Value>,
        /// Whether to keep the subscription after EOSE.
        live: bool,
    },
    /// Close a subscription.
    Close(String),
    /// Answer a NIP-42 challenge with a signed kind `22242` event.
    Auth(Event),
}

/// A message from the relay to the game.
#[derive(Clone, Debug)]
pub enum In {
    /// The socket is open.
    Connected,
    /// The socket closed or could not open.
    Disconnected(String),
    /// An event for a subscription.
    Event {
        /// Subscription id.
        sub: String,
        /// The event, not yet validated.
        event: Box<Event>,
    },
    /// End of stored events for a subscription.
    Eose(String),
    /// The relay's verdict on a published event.
    Ok {
        /// Event id.
        id: String,
        /// Accepted or not.
        accepted: bool,
        /// The relay's reason.
        message: String,
    },
    /// The relay closed a subscription.
    Closed(String, String),
    /// A human-readable relay notice.
    Notice(String),
    /// A NIP-42 challenge.
    Auth(String),
}

/// Parses one relay message.
#[must_use]
pub fn parse(text: &str) -> Option<In> {
    let value = nostr::contracts::parse_strict_bounded(text.as_bytes(), MAX_WIRE).ok()?;
    let parts = value.as_array()?;
    let str_at = |i: usize| parts.get(i).and_then(Value::as_str).map(str::to_owned);
    match parts.first()?.as_str()? {
        "EVENT" => {
            let event: Event = serde_json::from_value(parts.get(2)?.clone()).ok()?;
            Some(In::Event {
                sub: str_at(1)?,
                event: Box::new(event),
            })
        }
        "EOSE" => Some(In::Eose(str_at(1)?)),
        "OK" => Some(In::Ok {
            id: str_at(1)?,
            accepted: parts.get(2)?.as_bool()?,
            message: str_at(3).unwrap_or_default(),
        }),
        "CLOSED" => Some(In::Closed(str_at(1)?, str_at(2).unwrap_or_default())),
        "NOTICE" => Some(In::Notice(str_at(1)?)),
        "AUTH" => Some(In::Auth(str_at(1)?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::Message;

    #[test]
    fn relay_messages_parse() {
        assert!(matches!(parse(r#"["EOSE","s1"]"#), Some(In::Eose(s)) if s == "s1"));
        assert!(matches!(
            parse(r#"["OK","ab",false,"rate-limited: slow down"]"#),
            Some(In::Ok { accepted: false, message, .. }) if message.starts_with("rate-limited:")
        ));
        assert!(matches!(parse(r#"["NOTICE","hi"]"#), Some(In::Notice(_))));
        assert!(matches!(parse(r#"["AUTH","abc"]"#), Some(In::Auth(c)) if c == "abc"));
        assert!(parse("not json").is_none());
        assert!(parse(r#"["EVENT","s1",{"bad":1}]"#).is_none());
    }

    #[test]
    fn a_req_carries_every_filter() {
        let v = req("s", &[json!({"kinds":[1]}), json!({"kinds":[2]})]);
        assert_eq!(v.as_array().map(Vec::len), Some(4));
    }

    #[test]
    fn offline_queue_keeps_ordinary_events_and_replaces_only_replaceable_addresses() {
        let signer = nostr::domain::RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
        let mut subscriptions = Subscriptions::new();
        let mut backlog = VecDeque::new();
        for content in ["first", "second"] {
            let line = signer.sign(1, 9, vec![], content.into());
            assert!(retain_offline(
                Out::Publish(line),
                &mut subscriptions,
                &mut backlog
            ));
        }
        assert_eq!(backlog.len(), 2);
        for content in ["old", "new"] {
            let state = signer.sign(
                1,
                33301,
                vec![nostr::domain::Tag::new(vec![
                    "d".into(),
                    "world/avatar".into(),
                ])],
                content.into(),
            );
            assert!(retain_offline(
                Out::Publish(state),
                &mut subscriptions,
                &mut backlog
            ));
        }
        assert_eq!(backlog.len(), 3);
        assert_eq!(backlog.back().unwrap().content, "new");
        let frame = signer.sign(1, 23300, vec![], "pose".into());
        assert!(retain_offline(
            Out::Publish(frame),
            &mut subscriptions,
            &mut backlog
        ));
        assert_eq!(backlog.len(), 3);
    }

    #[test]
    fn relay_parsing_and_queues_are_bounded() {
        assert!(parse(&format!("[\"NOTICE\",\"{}\"]", "x".repeat(MAX_WIRE))).is_none());
        assert!(connector().is_ok());
        let mut subscriptions = Subscriptions::new();
        let mut backlog = VecDeque::new();
        for i in 0..QUEUE {
            assert!(retain_offline(
                Out::Subscribe {
                    id: i.to_string(),
                    filters: vec![],
                    live: true
                },
                &mut subscriptions,
                &mut backlog
            ));
        }
        assert!(!retain_offline(
            Out::Subscribe {
                id: "overflow".into(),
                filters: vec![],
                live: true
            },
            &mut subscriptions,
            &mut backlog
        ));
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_unfinished_websocket_handshake() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut link = Link::start(&format!("ws://{}", listener.local_addr().unwrap()));
        let (_blocked_handshake, _) =
            tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap();
        assert!(link.shutdown(Duration::from_millis(100)));
    }

    #[tokio::test]
    async fn authentication_ack_restores_live_and_unfinished_one_shot_subscriptions() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut link = Link::start(&format!("ws://{}", listener.local_addr().unwrap()));
        let signer = nostr::domain::RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
        let auth = signer.sign(1, 22242, vec![], String::new());
        link.send(Out::Subscribe {
            id: "world".into(),
            filters: vec![json!({"kinds":[33301]})],
            live: true,
        });
        link.send(Out::Subscribe {
            id: "spawn".into(),
            filters: vec![json!({"kinds":[33301]})],
            live: false,
        });
        link.send(Out::Auth(auth.clone()));
        tokio::time::timeout(Duration::from_secs(3), async {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            for _ in 0..3 {
                socket.next().await.unwrap().unwrap();
            }
            socket
                .send(Message::text(json!(["OK", auth.id, true, ""]).to_string()))
                .await
                .unwrap();
            let mut ids = Vec::new();
            for _ in 0..2 {
                let frame = socket.next().await.unwrap().unwrap();
                let v: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
                assert_eq!(v[0], "REQ");
                ids.push(v[1].as_str().unwrap().to_owned());
            }
            ids.sort();
            assert_eq!(ids, ["spawn", "world"]);
        })
        .await
        .unwrap();
        assert!(link.shutdown(Duration::from_millis(100)));
    }
}
