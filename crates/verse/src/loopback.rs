//! A minimal NIP-01 relay on a loopback socket for presence tests and
//! simulated players (`openagents verse walkers --loopback`). It is a
//! fixture, not the production relay: no authentication, rate limits, or
//! persistence. It verifies signatures, forwards ephemeral events, keeps the
//! latest addressable event per address, answers `REQ` with stored matches and
//! `EOSE`, and records every accepted event so a test can inspect exactly what
//! each client published, and every message each connection sent, in order.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::Event;
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, oneshot};
use tokio_tungstenite::tungstenite::Message;

type Stored = Arc<Mutex<BTreeMap<(String, u16, String), Event>>>;
/// Every client message by connection (numbered from 0 in accept order):
/// its NIP-01 verb, `REQ`, `EVENT`, `AUTH`, `CLOSE`, or anything else sent.
type Sent = Arc<Mutex<Vec<(usize, String)>>>;

/// A running loopback relay. Dropping it stops the relay.
pub struct LoopbackRelay {
    /// The `ws://127.0.0.1:<port>` address.
    pub url: String,
    published: Arc<Mutex<Vec<Event>>>,
    sent: Sent,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl LoopbackRelay {
    /// Starts the relay on its own thread and returns once it listens.
    #[must_use]
    pub fn start() -> Self {
        let (address_tx, address_rx) = std::sync::mpsc::channel();
        let (stop, stop_rx) = oneshot::channel::<()>();
        let published = Arc::new(Mutex::new(Vec::new()));
        let log = published.clone();
        let sent: Sent = Arc::default();
        let verbs = sent.clone();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                address_tx
                    .send(format!("ws://{}", listener.local_addr().unwrap()))
                    .unwrap();
                let stored: Stored = Arc::default();
                let (sender, _) = broadcast::channel::<Event>(1024);
                let accept = async {
                    let mut connection = 0;
                    while let Ok((stream, _)) = listener.accept().await {
                        let stored = stored.clone();
                        let sender = sender.clone();
                        let log = log.clone();
                        let verbs = verbs.clone();
                        tokio::spawn(async move {
                            let _ = serve(stream, stored, sender, log, (connection, verbs)).await;
                        });
                        connection += 1;
                    }
                };
                tokio::select! {
                    _ = stop_rx => {},
                    () = accept => {},
                }
            });
        });
        Self {
            url: address_rx.recv().unwrap(),
            published,
            sent,
            stop: Some(stop),
            thread: Some(thread),
        }
    }

    /// Every event the relay accepted so far, in arrival order.
    #[must_use]
    pub fn published(&self) -> Vec<Event> {
        self.published.lock().unwrap().clone()
    }

    /// The verb of every message connection `connection` sent, in order.
    #[must_use]
    pub fn sent_by(&self, connection: usize) -> Vec<String> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|(from, _)| *from == connection)
            .map(|(_, verb)| verb.clone())
            .collect()
    }

    /// Accepted events of `kind` signed by `pubkey`.
    #[must_use]
    pub fn published_by(&self, pubkey: &str, kind: u16) -> Vec<Event> {
        self.published()
            .into_iter()
            .filter(|event| event.pubkey == pubkey && event.kind == kind)
            .collect()
    }
}

impl Drop for LoopbackRelay {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn matches(event: &Event, filter: &Value) -> bool {
    let listed = |key: &str, value: Value| {
        filter[key]
            .as_array()
            .is_none_or(|items| items.contains(&value))
    };
    listed("ids", json!(event.id))
        && listed("authors", json!(event.pubkey))
        && listed("kinds", json!(event.kind))
        && ["w", "c", "d", "p", "h", "t", "z", "e"].iter().all(|key| {
            filter[format!("#{key}")].as_array().is_none_or(|wanted| {
                event
                    .tag_values(key)
                    .any(|value| wanted.contains(&json!(value)))
            })
        })
}

async fn serve(
    stream: TcpStream,
    stored: Stored,
    sender: broadcast::Sender<Event>,
    log: Arc<Mutex<Vec<Event>>>,
    (connection, sent): (usize, Sent),
) -> Result<(), String> {
    let mut socket = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|e| e.to_string())?;
    let mut subscriptions = BTreeMap::<String, Vec<Value>>::new();
    let mut live = sender.subscribe();
    loop {
        tokio::select! {
            received = live.recv() => {
                let Ok(event) = received else { continue };
                for (id, filters) in &subscriptions {
                    if filters.iter().any(|filter| matches(&event, filter)) {
                        let text = json!(["EVENT", id, event]).to_string();
                        socket.send(Message::text(text)).await.map_err(|e| e.to_string())?;
                    }
                }
            }
            message = socket.next() => {
                let Some(Ok(message)) = message else { return Ok(()) };
                let Ok(text) = message.to_text() else { continue };
                let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(text) else { continue };
                let verb = parts.first().and_then(Value::as_str).unwrap_or("?").to_owned();
                sent.lock().unwrap().push((connection, verb));
                match parts.first().and_then(Value::as_str) {
                    Some("REQ") => {
                        let Some(id) = parts.get(1).and_then(Value::as_str) else { continue };
                        let filters: Vec<Value> = parts[2..].to_vec();
                        let found: Vec<Event> = stored
                            .lock()
                            .unwrap()
                            .values()
                            .filter(|event| filters.iter().any(|filter| matches(event, filter)))
                            .cloned()
                            .collect();
                        for event in found {
                            let text = json!(["EVENT", id, event]).to_string();
                            socket.send(Message::text(text)).await.map_err(|e| e.to_string())?;
                        }
                        let text = json!(["EOSE", id]).to_string();
                        socket.send(Message::text(text)).await.map_err(|e| e.to_string())?;
                        subscriptions.insert(id.to_owned(), filters);
                    }
                    Some("CLOSE") => {
                        if let Some(id) = parts.get(1).and_then(Value::as_str) {
                            subscriptions.remove(id);
                        }
                    }
                    Some("EVENT") => {
                        let Some(event) = parts
                            .get(1)
                            .and_then(|value| serde_json::from_value::<Event>(value.clone()).ok())
                        else {
                            continue;
                        };
                        let accepted = event.validate_crypto().is_ok();
                        let text = json!(["OK", event.id, accepted, if accepted { "" } else { "invalid: signature" }]).to_string();
                        socket.send(Message::text(text)).await.map_err(|e| e.to_string())?;
                        if !accepted {
                            continue;
                        }
                        if !(20_000..30_000).contains(&event.kind) {
                            let d = event.tag_values("d").next().unwrap_or_default().to_owned();
                            let key = if (30_000..40_000).contains(&event.kind) {
                                (event.pubkey.clone(), event.kind, d)
                            } else {
                                (event.pubkey.clone(), event.kind, event.id.clone())
                            };
                            stored.lock().unwrap().insert(key, event.clone());
                        }
                        log.lock().unwrap().push(event.clone());
                        let _ = sender.send(event);
                    }
                    _ => {}
                }
            }
        }
    }
}
