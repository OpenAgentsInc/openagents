//! A loopback NIP-42 relay for the benchmark. It is not the production
//! relay and claims nothing about its latency.
//!
//! It follows the fixture in `crates/coder-control/src/tests/relay.rs`, but
//! honors a filter's `since` and `limit` as NIP-01 says. That fixture
//! replays every stored event on each subscription, so a host whose
//! subscription renews (every 90 seconds) answers every earlier request
//! again, and those replays would compete with the reads being timed.

use futures_util::{SinkExt, StreamExt};
use nostr::domain::Event;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, broadcast};
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};

type Events = Arc<Mutex<Vec<Event>>>;

fn visible(event: &Event, principal: &str) -> bool {
    event.pubkey == principal || event.tag_values("p").any(|p| p == principal)
}

fn matches(event: &Event, filter: &Value) -> bool {
    let member = |key: &str, value: Value| {
        filter[key]
            .as_array()
            .is_none_or(|items| items.contains(&value))
    };
    member("ids", json!(event.id))
        && member("authors", json!(event.pubkey))
        && member("kinds", json!(event.kind))
        && filter["since"]
            .as_u64()
            .is_none_or(|since| event.created_at >= since)
        && ["p", "e", "h"].iter().all(|key| {
            filter[format!("#{key}")]
                .as_array()
                .is_none_or(|wanted| event.tag_values(key).any(|v| wanted.contains(&json!(v))))
        })
}

/// Start the relay on a loopback port; returns its `ws://` URL.
pub async fn start() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let url = format!("ws://{}", listener.local_addr().expect("an address"));
    let events: Events = Arc::default();
    let address = url.clone();
    let (sender, _) = broadcast::channel(4096);
    let handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (url, events, sender) = (address.clone(), events.clone(), sender.clone());
            tokio::spawn(async move {
                let _ = serve(stream, &url, events, sender).await;
            });
        }
    });
    (url, handle)
}

async fn send(
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    value: Value,
) -> Result<(), String> {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(|e| e.to_string())
}

async fn serve(
    stream: TcpStream,
    url: &str,
    events: Events,
    sender: broadcast::Sender<Event>,
) -> Result<(), String> {
    let _ = stream.set_nodelay(true);
    let mut socket = accept_async_with_config(
        stream,
        Some(WebSocketConfig::default().max_message_size(Some(1024 * 1024))),
    )
    .await
    .map_err(|e| e.to_string())?;
    let challenge = secp256k1::rand::random::<u128>().to_string();
    send(&mut socket, json!(["AUTH", challenge])).await?;
    let mut principal = None::<String>;
    let mut subscriptions = BTreeMap::<String, Value>::new();
    let mut broadcast = sender.subscribe();
    loop {
        tokio::select! {
            received = broadcast.recv() => {
                let event = match received {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                };
                if principal.as_deref().is_some_and(|p| visible(&event, p)) {
                    let ids: Vec<String> = subscriptions
                        .iter()
                        .filter(|(_, filter)| matches(&event, filter))
                        .map(|(id, _)| id.clone())
                        .collect();
                    for id in ids {
                        send(&mut socket, json!(["EVENT", id, event])).await?;
                    }
                }
            }
            frame = socket.next() => {
                let text = match frame {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => return Err(error.to_string()),
                };
                let value = nostr::contracts::parse_strict_bounded(text.as_bytes(), 1024 * 1024)
                    .map_err(|e| e.to_string())?;
                let response = match value[0].as_str() {
                    Some("AUTH") => {
                        let e: Event = serde_json::from_value(value[1].clone()).map_err(|e| e.to_string())?;
                        let now = coder_connect::unix_time().unwrap_or_default();
                        let good = e.kind == 22242
                            && e.validate_crypto().is_ok()
                            && e.tag_values("relay").collect::<Vec<_>>() == [url]
                            && e.tag_values("challenge").collect::<Vec<_>>() == [challenge.as_str()]
                            && e.created_at.abs_diff(now) <= 60;
                        if good {
                            principal = Some(e.pubkey.clone());
                        }
                        json!(["OK", e.id, good, if good { "" } else { "auth-required: invalid proof" }])
                    }
                    Some("EVENT") => {
                        let e: Event = serde_json::from_value(value[1].clone()).map_err(|e| e.to_string())?;
                        // Private artifacts are stored; NIP-CJ jobs and their
                        // answers are ephemeral and only relayed.
                        let stored = e.kind == 3188;
                        let good = principal.as_ref() == Some(&e.pubkey)
                            && e.validate_crypto().is_ok()
                            && (stored && nostr::private_artifact::admit(&e).is_ok()
                                || matches!(e.kind, 25900 | 25920 | 26900 | 26920 | 27000 | 27020));
                        if good {
                            if stored {
                                events.lock().await.push(e.clone());
                            }
                            let _ = sender.send(e.clone());
                        }
                        json!(["OK", e.id, good, if good { "" } else { "restricted: event refused" }])
                    }
                    Some("REQ") => {
                        let id = value[1].as_str().ok_or("subscription id")?.to_string();
                        let Some(reader) = principal.clone() else {
                            send(&mut socket, json!(["CLOSED", id, "auth-required: authenticate"])).await?;
                            continue;
                        };
                        let filter = value[2].clone();
                        subscriptions.insert(id.clone(), filter.clone());
                        let limit = filter["limit"].as_u64().unwrap_or(500) as usize;
                        if limit > 0 {
                            // Newest first, as NIP-01 says, then sent oldest first.
                            let stored = events.lock().await;
                            let mut found: Vec<&Event> = stored
                                .iter()
                                .rev()
                                .filter(|e| visible(e, &reader) && matches(e, &filter))
                                .take(limit)
                                .collect();
                            found.reverse();
                            let found: Vec<Event> = found.into_iter().cloned().collect();
                            drop(stored);
                            for event in found {
                                send(&mut socket, json!(["EVENT", id, event])).await?;
                            }
                        }
                        json!(["EOSE", id])
                    }
                    Some("CLOSE") => {
                        subscriptions.remove(value[1].as_str().ok_or("subscription id")?);
                        continue;
                    }
                    _ => continue,
                };
                send(&mut socket, response).await?;
            }
        }
    }
}
