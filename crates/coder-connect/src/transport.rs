//! Finite NIP-42 connections. Signed application replies, not relay ACKs, carry data.
use crate::{Error, ErrorCode, Result, client::Pending, protocol::*, unix_time};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn transport(_: String) -> Error {
    Error::new(
        ErrorCode::Transport,
        "authenticated relay exchange is unavailable",
    )
}
fn event(value: serde_json::Value) -> Result<Event> {
    let event: Event = serde_json::from_value(value)
        .map_err(|_| Error::new(ErrorCode::Malformed, "relay returned an invalid event"))?;
    nostr::private_artifact::admit(&event).map_err(|_| {
        Error::new(
            ErrorCode::Forbidden,
            "relay returned an invalid private declaration",
        )
    })?;
    Ok(event)
}

pub async fn exchange(
    relay: &str,
    secret: &SecretKey,
    pending: &Pending,
    host: &str,
    policy: RelayPolicy,
) -> Result<Event> {
    tokio::time::timeout(Duration::from_secs(8), async {
        let mut session = Session::connect(relay, secret, policy).await?;
        session.exchange(pending, host, &pubkey(secret)).await
    })
    .await
    .map_err(|_| transport(String::new()))?
}

/// A serial client socket with a fixed lifetime and aggregate frame budget.
/// The caller must drop it after any interrupted or unsuccessful exchange.
pub struct Session {
    socket: Connection,
    started: std::time::Instant,
    exchanges: usize,
}
impl Session {
    pub async fn connect(relay: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<Self> {
        policy.validate(relay)?;
        let started = std::time::Instant::now();
        let socket = Connection::connect(relay, secret, Duration::from_secs(90))
            .await
            .map_err(transport)?;
        Ok(Self {
            socket,
            started,
            exchanges: 0,
        })
    }
    pub fn reusable(&self) -> bool {
        self.exchanges < 24 && self.started.elapsed() < Duration::from_secs(75)
    }
    pub async fn exchange(&mut self, pending: &Pending, host: &str, own: &str) -> Result<Event> {
        self.exchange_event(
            &pending.event,
            &pending.request.request,
            (pending.request.issued_at, pending.request.expires_at),
            host,
            own,
        )
        .await
    }
    pub async fn exchange_event(
        &mut self,
        event: &Event,
        subscription: &str,
        lifetime: (u64, u64),
        host: &str,
        own: &str,
    ) -> Result<Event> {
        self.socket.send(json!(["REQ",subscription,{"kinds":[3188],"authors":[host],"#p":[own],"#h":[subscription],"limit":0}])).await.map_err(transport)?;
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == subscription {
                return Err(transport(String::new()));
            }
            if frame[0] == "EVENT" && frame[1] == subscription {
                // No read has been published on this subscription yet. A
                // replayed old reply cannot establish current host admission.
                return Err(Error::new(
                    ErrorCode::Forbidden,
                    "reply arrived before this exchange was admitted",
                ));
            }
            if frame[0] == "EOSE" && frame[1] == subscription {
                break;
            }
        }
        fresh(lifetime.0, lifetime.1, unix_time()?)?;
        self.socket
            .send(json!(["EVENT", event]))
            .await
            .map_err(transport)?;
        let mut acknowledged = false;
        let mut response = None;
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == subscription {
                return Err(transport(String::new()));
            }
            if frame[0] == "OK" && frame[1] == event.id {
                if frame[2] != true {
                    return Err(transport(String::new()));
                }
                acknowledged = true;
            }
            if frame[0] == "EVENT" && frame[1] == subscription {
                let received = check_reply(frame[2].clone(), host, own, subscription)?;
                if response
                    .as_ref()
                    .is_some_and(|old: &Event| old.id != received.id)
                {
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "relay supplied conflicting replies",
                    ));
                }
                response = Some(received);
            }
            if acknowledged && let Some(reply) = response {
                self.socket
                    .send(json!(["CLOSE", subscription]))
                    .await
                    .map_err(transport)?;
                self.exchanges += 1;
                return Ok(reply);
            }
        }
    }
}
/// A client's standing relay connection: one authenticated socket with one
/// reply subscription for its whole lease, carrying many requests at once.
/// Each request is published at once, with no per-read subscribe, EOSE, or
/// close; a reply is matched to its request by mailbox and accepted only
/// after the request was published. A socket failure fails every request
/// in flight, and the caller opens a new link.
pub struct Link {
    outbound: tokio::sync::mpsc::UnboundedSender<serde_json::Value>,
    waiting: Arc<Mutex<HashMap<String, Waiter>>>,
    closed: Arc<AtomicBool>,
    started: std::time::Instant,
    task: tokio::task::JoinHandle<()>,
}
struct Waiter {
    event: String,
    acknowledged: bool,
    reply: Option<Event>,
    done: Option<tokio::sync::oneshot::Sender<Result<Event>>>,
}
impl Waiter {
    fn finish(&mut self, result: Result<Event>) {
        if let Some(done) = self.done.take() {
            let _ = done.send(result);
        }
    }
}
/// A link's socket lease, the longest the transport allows.
const LINK_LEASE: Duration = Duration::from_secs(120);
/// A link takes new requests only this long, so each finishes within the
/// lease under its own eight-second deadline.
const LINK_REUSE: Duration = Duration::from_secs(105);
impl Link {
    /// Connect, authenticate, and open the reply subscription for replies
    /// from `host` to `own`, waiting for its end of stored events.
    pub async fn connect(
        relay: &str,
        secret: &SecretKey,
        policy: RelayPolicy,
        host: &str,
    ) -> Result<Self> {
        policy.validate(relay)?;
        let own = pubkey(secret);
        let mut socket = Connection::connect(relay, secret, LINK_LEASE)
            .await
            .map_err(transport)?
            .with_frame_budget(4096);
        let subscription = random_id();
        socket
            .send(
                json!(["REQ",subscription,{"kinds":[3188],"authors":[host],"#p":[own],"limit":0}]),
            )
            .await
            .map_err(transport)?;
        loop {
            let frame = socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == subscription.as_str() {
                return Err(transport(String::new()));
            }
            if frame[0] == "EOSE" && frame[1] == subscription.as_str() {
                break;
            }
            // A stored reply names no request of this link: it is ignored.
        }
        let (outbound, queue) = tokio::sync::mpsc::unbounded_channel();
        let waiting: Arc<Mutex<HashMap<String, Waiter>>> = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(run(
            socket,
            queue,
            waiting.clone(),
            closed.clone(),
            (subscription, host.to_owned(), own),
        ));
        Ok(Self {
            outbound,
            waiting,
            closed,
            started: std::time::Instant::now(),
            task,
        })
    }
    /// Whether a new request may use this link.
    pub fn reusable(&self) -> bool {
        !self.closed.load(Ordering::Acquire) && self.started.elapsed() < LINK_REUSE
    }
    /// Publish `pending` and wait for its signed reply. The caller bounds
    /// the wait; dropping the future forgets the request, not the link.
    pub async fn exchange(&self, pending: &Pending) -> Result<Event> {
        fresh(
            pending.request.issued_at,
            pending.request.expires_at,
            unix_time()?,
        )?;
        let mailbox = pending.request.request.clone();
        let (done, result) = tokio::sync::oneshot::channel();
        {
            let mut waiting = lock(&self.waiting);
            if self.closed.load(Ordering::Acquire) || waiting.contains_key(&mailbox) {
                return Err(transport(String::new()));
            }
            waiting.insert(
                mailbox.clone(),
                Waiter {
                    event: pending.event.id.clone(),
                    acknowledged: false,
                    reply: None,
                    done: Some(done),
                },
            );
        }
        struct Forget<'a>(&'a Mutex<HashMap<String, Waiter>>, String);
        impl Drop for Forget<'_> {
            fn drop(&mut self) {
                lock(self.0).remove(&self.1);
            }
        }
        let _forget = Forget(&self.waiting, mailbox);
        if self.outbound.send(json!(["EVENT", pending.event])).is_err() {
            return Err(transport(String::new()));
        }
        result.await.map_err(|_| transport(String::new()))?
    }
}
impl Drop for Link {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn run(
    mut socket: Connection,
    mut queue: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    waiting: Arc<Mutex<HashMap<String, Waiter>>>,
    closed: Arc<AtomicBool>,
    (subscription, host, own): (String, String, String),
) {
    loop {
        let frame = tokio::select! {
            biased;
            outbound = queue.recv() => {
                let Some(value) = outbound else { break };
                if socket.send(value).await.is_err() {
                    break;
                }
                continue;
            }
            frame = socket.next() => match frame {
                Ok(frame) => frame,
                Err(_) => break,
            },
        };
        if frame[0] == "CLOSED" && frame[1] == subscription.as_str() {
            break;
        }
        let mut waiting = lock(&waiting);
        if frame[0] == "OK" {
            let Some(waiter) = waiting.values_mut().find(|w| frame[1] == w.event.as_str()) else {
                continue;
            };
            if frame[2] != true {
                waiter.finish(Err(transport(String::new())));
                continue;
            }
            waiter.acknowledged = true;
        } else if frame[0] == "EVENT" && frame[1] == subscription.as_str() {
            let mailbox = frame[2]["tags"]
                .as_array()
                .and_then(|tags| {
                    tags.iter()
                        .find(|t| t[0] == "h")
                        .and_then(|t| t[1].as_str())
                })
                .unwrap_or_default()
                .to_owned();
            // Only a request this link published and waits on takes a reply.
            let Some(waiter) = waiting.get_mut(&mailbox) else {
                continue;
            };
            match check_reply(frame[2].clone(), &host, &own, &mailbox) {
                Ok(received) => {
                    if waiter
                        .reply
                        .as_ref()
                        .is_some_and(|old| old.id != received.id)
                    {
                        waiter.finish(Err(Error::new(
                            ErrorCode::Conflict,
                            "relay supplied conflicting replies",
                        )));
                        continue;
                    }
                    waiter.reply = Some(received);
                }
                Err(error) => {
                    waiter.finish(Err(error));
                    continue;
                }
            }
        } else {
            continue;
        }
        for waiter in waiting.values_mut() {
            if waiter.acknowledged
                && let Some(reply) = waiter.reply.take()
            {
                waiter.finish(Ok(reply));
            }
        }
    }
    closed.store(true, Ordering::Release);
    // Every waiting request fails at once rather than at its deadline.
    for waiter in lock(&waiting).values_mut() {
        waiter.finish(Err(transport(String::new())));
    }
}
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn check_reply(
    value: serde_json::Value,
    host: &str,
    recipient: &str,
    mailbox: &str,
) -> Result<Event> {
    let event = event(value)?;
    if event.pubkey != host
        || event.tag_values("p").collect::<Vec<_>>() != [recipient]
        || event.tag_values("h").collect::<Vec<_>>() != [mailbox]
    {
        return Err(Error::new(
            ErrorCode::Forbidden,
            "relay reply identity differs",
        ));
    }
    Ok(event)
}

/// A finite host subscription, renewed by the CLI with bounded backoff.
pub struct Receiver {
    socket: Connection,
    host: String,
}
impl Receiver {
    pub async fn connect(relay: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<Self> {
        policy.validate(relay)?;
        let host = pubkey(secret);
        // The longest lease a connection may hold, and a frame budget for
        // hundreds of reads in it: the serve loop renews it at once.
        let mut socket = Connection::connect(relay, secret, Duration::from_secs(120))
            .await
            .map_err(transport)?
            .with_frame_budget(4096);
        socket.send(json!(["REQ","history-input",{"kinds":[3188],"#p":[host],"since":unix_time()?.saturating_sub(60),"limit":128}])).await.map_err(transport)?;
        Ok(Self { socket, host })
    }
    pub async fn next_request(&mut self) -> Result<Event> {
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" {
                return Err(transport(String::new()));
            }
            if frame[0] == "EVENT" && frame[1] == "history-input" {
                let event = event(frame[2].clone())?;
                if event.tag_values("p").collect::<Vec<_>>() != [self.host.as_str()] {
                    return Err(Error::new(
                        ErrorCode::Forbidden,
                        "request targets another host",
                    ));
                }
                return Ok(event);
            }
        }
    }
    pub async fn publish(&mut self, event: &Event) -> Result<()> {
        if event.pubkey != self.host {
            return Err(Error::new(
                ErrorCode::Forbidden,
                "reply signer differs from host",
            ));
        }
        self.socket
            .send(json!(["EVENT", event]))
            .await
            .map_err(transport)?;
        // ACKs and interleaved requests stay in the socket and are read by the
        // next iteration. The client independently needs the signed reply.
        Ok(())
    }
}
