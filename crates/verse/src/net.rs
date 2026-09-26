//! Bounded relay connections on a cancellable background runtime.
//! Dropping a link signals cancellation without waiting on the native UI thread.
use futures_util::{SinkExt, StreamExt};
use nostr::domain::Event;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;
use tokio::sync::{mpsc as async_mpsc, watch};
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
const QUEUE: usize = 64;
const MAX_WIRE: usize = 64 * 1024;
const CONNECT_WAIT: Duration = Duration::from_secs(5);
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

/// The game's handle on a bounded, cancellable link thread.
pub struct Link {
    tx: async_mpsc::Sender<Out>,
    rx: Receiver<In>,
    cancel: watch::Sender<bool>,
    done: Receiver<()>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// The exact relay URL used for NIP-42.
    pub url: String,
}
impl Link {
    #[cfg(test)]
    pub(crate) fn idle() -> Self {
        let (tx, _) = async_mpsc::channel(QUEUE);
        let (_, rx) = mpsc::sync_channel(QUEUE);
        let (cancel, _) = watch::channel(false);
        let (_, done) = mpsc::sync_channel(1);
        Self {
            tx,
            rx,
            cancel,
            done,
            thread: None,
            url: "ws://127.0.0.1:1".into(),
        }
    }
    /// Start a relay worker. No connection, DNS lookup, or read blocks this caller.
    #[must_use]
    pub fn start(url: &str) -> Self {
        let (tx, mut out) = async_mpsc::channel(QUEUE);
        let (inbox, rx) = mpsc::sync_channel(QUEUE);
        let (cancel, mut cancellation) = watch::channel(false);
        let (complete, done) = mpsc::sync_channel(1);
        let thread_url = url.to_owned();
        let thread = std::thread::Builder::new()
            .name("verse-relay".into())
            .spawn(move || {
                if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    runtime.block_on(async {
                        tokio::select! {
                            biased;
                            _ = cancellation.changed() => {},
                            _ = run(&thread_url, &mut out, &inbox) => {},
                        }
                    });
                }
                let _ = complete.send(());
            })
            .expect("the relay thread starts");
        Self {
            tx,
            rx,
            cancel,
            done,
            thread: Some(thread),
            url: url.into(),
        }
    }
    /// Queue a bounded command. False means backpressure or a stopped worker.
    pub fn send(&self, out: Out) -> bool {
        self.send_batch(vec![out])
    }
    /// Reserve queue capacity for every operation before queuing any of them.
    /// This is local backpressure handling, not a delivery acknowledgment.
    pub fn send_batch(&self, commands: Vec<Out>) -> bool {
        if commands.is_empty()
            || commands.len() > QUEUE
            || commands.iter().any(|out| {
                let size = match out {
                    Out::Publish(e) | Out::Auth(e) => {
                        serde_json::to_vec(e).map_or(usize::MAX, |b| b.len())
                    }
                    Out::Subscribe { id, filters, .. } => {
                        if id.len() > 128 || filters.len() > 16 {
                            usize::MAX
                        } else {
                            req(id, filters).to_string().len()
                        }
                    }
                    Out::Close(id) => id.len(),
                };
                size > MAX_WIRE
            })
        {
            return false;
        }
        let Ok(permits) = self.tx.try_reserve_many(commands.len()) else {
            return false;
        };
        for (permit, command) in permits.zip(commands) {
            permit.send(command);
        }
        true
    }
    /// Drain at most one queue's worth of messages, keeping frame work bounded.
    #[must_use]
    pub fn drain(&self) -> Vec<In> {
        self.rx.try_iter().take(QUEUE).collect()
    }
    /// Cancel the worker. The optional wait is capped at 100 ms; UI callers use zero.
    pub fn shutdown(&mut self, wait: Duration) -> bool {
        let _ = self.cancel.send(true);
        let finished = self.done.try_recv().is_ok()
            || (!wait.is_zero()
                && self
                    .done
                    .recv_timeout(wait.min(Duration::from_millis(100)))
                    .is_ok())
            || self.thread.as_ref().is_some_and(|t| t.is_finished());
        if finished && let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        finished
    }
}
impl Drop for Link {
    fn drop(&mut self) {
        self.shutdown(Duration::ZERO);
    }
}
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type Subscriptions = BTreeMap<String, (Vec<Value>, bool)>;
fn connector() -> Result<Connector, String> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| "TLS protocol configuration unavailable")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Connector::Rustls(std::sync::Arc::new(config)))
}
async fn run(url: &str, out: &mut async_mpsc::Receiver<Out>, inbox: &SyncSender<In>) {
    let mut subscriptions = Subscriptions::new();
    let mut backlog = VecDeque::new();
    let mut backoff = Duration::from_millis(500);
    loop {
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_WIRE))
            .max_frame_size(Some(MAX_WIRE));
        let Ok(tls) = connector() else {
            return;
        };
        let opened = tokio::time::timeout(
            CONNECT_WAIT,
            connect_async_tls_with_config(url, Some(config), false, Some(tls)),
        )
        .await;
        if let Ok(Ok((mut socket, _))) = opened {
            if inbox.try_send(In::Connected).is_err() {
                return;
            }
            let result = connected(&mut socket, out, inbox, &mut subscriptions, &mut backlog).await;
            if result.is_ok() {
                return;
            }
        }
        if inbox
            .try_send(In::Disconnected(
                "Relay connection unavailable; retrying.".into(),
            ))
            .is_err()
        {
            return;
        }
        let until = tokio::time::sleep(backoff);
        tokio::pin!(until);
        loop {
            tokio::select! {
                _ = &mut until => break,
                command = out.recv() => {
                    let Some(command) = command else { return; };
                    if !retain_offline(command, &mut subscriptions, &mut backlog) {
                        let _ = inbox.try_send(In::Notice("Relay queue is full; an unsent operation was refused.".into()));
                    }
                }
            }
        }
        backoff = (backoff * 2).min(Duration::from_secs(8));
    }
}
async fn restore(socket: &mut Socket, subscriptions: &Subscriptions) -> Result<(), ()> {
    for (id, (filters, _)) in subscriptions {
        write(socket, &req(id, filters)).await?;
    }
    Ok(())
}
async fn connected(
    socket: &mut Socket,
    out: &mut async_mpsc::Receiver<Out>,
    inbox: &SyncSender<In>,
    subscriptions: &mut Subscriptions,
    backlog: &mut VecDeque<Event>,
) -> Result<(), ()> {
    restore(socket, subscriptions).await?;
    while let Some(event) = backlog.pop_front() {
        write(socket, &json!(["EVENT", event])).await?;
    }
    let mut auth_id = None;
    loop {
        tokio::select! {
            command = out.recv() => {
                let Some(command) = command else { return Ok(()); };
                match command {
                    Out::Publish(event) => write(socket, &json!(["EVENT", event])).await?,
                    Out::Auth(event) => { auth_id=Some(event.id.clone()); write(socket, &json!(["AUTH", event])).await?; },
                    Out::Subscribe {id, filters, live} => {
                        if subscriptions.len() >= QUEUE && !subscriptions.contains_key(&id) {
                            inbox.try_send(In::Closed(id,"restricted: subscription bound".into())).map_err(|_| ())?;
                        } else {
                            write(socket, &req(&id,&filters)).await?;
                            subscriptions.insert(id,(filters,live));
                        }
                    },
                    Out::Close(id) => { subscriptions.remove(&id); write(socket, &json!(["CLOSE",id])).await?; }
                }
            },
            frame = socket.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    let Some(message) = parse(&text) else { return Err(()); };
                    if matches!(&message, In::Ok {id,accepted:true,..} if auth_id.as_ref()==Some(id)) {
                        auth_id=None;
                        restore(socket, subscriptions).await?;
                    }
                    if let In::Eose(id) = &message
                        && subscriptions.get(id).is_some_and(|(_,live)| !live) {
                        subscriptions.remove(id);
                        write(socket,&json!(["CLOSE",id])).await?;
                    }
                    inbox.try_send(message).map_err(|_| ())?;
                },
                Some(Ok(Message::Ping(bytes))) => { socket.send(Message::Pong(bytes)).await.map_err(|_| ())?; },
                Some(Ok(Message::Pong(_))) => {},
                _ => return Err(()),
            }
        }
    }
}
fn retain_offline(
    command: Out,
    subscriptions: &mut Subscriptions,
    backlog: &mut VecDeque<Event>,
) -> bool {
    match command {
        Out::Subscribe { id, filters, live } => {
            if subscriptions.len() >= QUEUE && !subscriptions.contains_key(&id) {
                return false;
            }
            subscriptions.insert(id, (filters, live));
        }
        Out::Close(id) => {
            subscriptions.remove(&id);
        }
        Out::Publish(event) if !(20_000..30_000).contains(&event.kind) => {
            backlog.retain(|old| old.id != event.id && !same_address(old, &event));
            if backlog.len() >= QUEUE {
                return false;
            }
            backlog.push_back(event);
        }
        // Motion and authentication from a previous connection must not replay.
        _ => {}
    }
    true
}
fn same_address(a: &Event, b: &Event) -> bool {
    if a.kind != b.kind || a.pubkey != b.pubkey {
        return false;
    }
    match a.kind {
        0 | 3 | 10_000..20_000 => true,
        30_000..40_000 => a.tag_values("d").next() == b.tag_values("d").next(),
        _ => false,
    }
}
fn req(id: &str, filters: &[Value]) -> Value {
    let mut message = vec![json!("REQ"), json!(id)];
    message.extend(filters.iter().cloned());
    Value::Array(message)
}
async fn write(socket: &mut Socket, value: &Value) -> Result<(), ()> {
    let text = value.to_string();
    if text.len() > MAX_WIRE {
        return Err(());
    }
    tokio::time::timeout(CONNECT_WAIT, socket.send(Message::text(text)))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
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
