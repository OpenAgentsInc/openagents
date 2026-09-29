//! One signed-in relay connection for the basic Coder's jobs.
//!
//! Opening a connection, answering the relay's NIP-42 challenge, and placing
//! a subscription cost about 300 ms, half of what a reply's first line waits
//! for. So while the Coder tab shows, the phone keeps one authenticated
//! connection to the chat worker's relay with one standing subscription for
//! the worker's answers to this device (`27000` feedback and `26900`
//! results, `#p` this device), and each job only publishes its request.
//!
//! The conversation kinds are ephemeral: the relay delivers them only to a
//! subscription open at the time. A connection lives at most 120 s
//! ([`nostr_transport::Connection`]), so the link opens the next one, with
//! its subscription, before the current one ends, and both deliver while
//! they overlap. Every answer is routed to the job whose request it names,
//! once: an event both connections deliver reaches the job one time. The
//! job still checks each answer ([`crate::basic_coder::Reading`]); the link
//! only routes frames.
//!
//! [`Link::warm`] keeps the connection open, as when the tab shows or the
//! app comes to the foreground; [`Link::rest`] closes it once no job is
//! waiting, as when the app goes to the background.

use nostr::domain::Event;
use nostr::kinds::{CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_RESULT};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::{Notify, mpsc};
use tokio::time::Instant;

/// How long one connection lives: the transport's most.
const LIFETIME: Duration = Duration::from_secs(120);
/// How old the newest connection is when the link opens the next, so the
/// two overlap for the rest of the older one's life.
const ROTATE: Duration = Duration::from_secs(70);
/// How often the link looks at its connections while it is kept open.
const TICK: Duration = Duration::from_secs(5);
/// How long the link waits after a connection failed to open.
const RETRY: Duration = Duration::from_secs(2);
/// The most frames one connection reads.
const FRAMES: usize = 4_096;
/// How many delivered event IDs the link remembers, to route each once.
const SEEN: usize = 1_024;

struct Socket {
    id: u64,
    out: mpsc::UnboundedSender<Value>,
    ready: bool,
    born: Instant,
}

#[derive(Default)]
struct State {
    /// Keep a connection open, as while the Coder tab shows.
    wanted: bool,
    /// The connections, oldest first.
    sockets: Vec<Socket>,
    next: u64,
    /// Each waiting job's request ID and where its frames go.
    jobs: HashMap<String, mpsc::UnboundedSender<Value>>,
    seen: VecDeque<String>,
    seen_set: HashSet<String>,
    /// A task keeps the connection fresh.
    kept: bool,
    /// How many connections the link has opened, for tests.
    opened: usize,
}

struct Inner {
    url: String,
    secret: SecretKey,
    me: String,
    state: Mutex<State>,
    changed: Notify,
}

/// The basic Coder's kept relay connection. Cloning shares it.
#[derive(Clone)]
pub(crate) struct Link {
    inner: Arc<Inner>,
}

/// A job's registration: its frames arrive on `frames`, and dropping it
/// forgets the job, as when the person stops a reply.
pub(crate) struct Job {
    link: Link,
    request: String,
    pub(crate) frames: mpsc::UnboundedReceiver<Value>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.link.forget(&self.request);
    }
}

impl Link {
    pub(crate) fn new(url: &str, secret: SecretKey) -> Self {
        Self {
            inner: Arc::new(Inner {
                url: url.to_owned(),
                secret,
                me: crate::account::public(&secret).0,
                state: Mutex::new(State::default()),
                changed: Notify::new(),
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        crate::basic_coder::lock(&self.inner.state)
    }

    /// How many connections the link has opened.
    #[cfg(test)]
    pub(crate) fn opened(&self) -> usize {
        self.state().opened
    }

    /// Keep a connection open, and open it now if none is.
    pub(crate) fn warm(&self, runtime: &Handle) {
        let mut state = self.state();
        state.wanted = true;
        if !state.kept {
            state.kept = true;
            drop(state);
            let link = self.clone();
            runtime.spawn(async move { link.keep().await });
        }
    }

    /// Stop keeping a connection: close it once no job waits on it.
    pub(crate) fn rest(&self) {
        let mut state = self.state();
        state.wanted = false;
        if state.jobs.is_empty() {
            state.sockets.clear();
        }
        drop(state);
        self.inner.changed.notify_waiters();
    }

    /// Keep the newest connection fresh while the link is wanted or a job
    /// waits: open one when none is open, and the next before it ages out.
    async fn keep(&self) {
        loop {
            let changed = self.inner.changed.notified();
            {
                let mut state = self.state();
                if !state.wanted && state.jobs.is_empty() {
                    state.sockets.clear();
                    state.kept = false;
                    return;
                }
                let fresh = state
                    .sockets
                    .last()
                    .is_some_and(|socket| !socket.ready || socket.born.elapsed() < ROTATE);
                if !fresh {
                    self.open(&mut state);
                }
            }
            let _ = tokio::time::timeout(TICK, changed).await;
        }
    }

    /// Open a connection in the background; it joins the link once its
    /// subscription is placed.
    fn open(&self, state: &mut State) {
        let (out, rx) = mpsc::unbounded_channel();
        state.next += 1;
        state.opened += 1;
        let id = state.next;
        state.sockets.push(Socket {
            id,
            out,
            ready: false,
            born: Instant::now(),
        });
        let link = self.clone();
        tokio::spawn(async move {
            let _ = link.run(id, rx).await;
            let ready = link
                .state()
                .sockets
                .iter()
                .any(|socket| socket.id == id && socket.ready);
            if !ready {
                // A connection that never opened waits before the next try.
                tokio::time::sleep(RETRY).await;
            }
            link.state().sockets.retain(|socket| socket.id != id);
            link.inner.changed.notify_waiters();
        });
    }

    async fn run(&self, id: u64, mut out: mpsc::UnboundedReceiver<Value>) -> Result<(), String> {
        let mut connection =
            nostr_transport::Connection::connect(&self.inner.url, &self.inner.secret, LIFETIME)
                .await?
                .with_frame_budget(FRAMES);
        let subscription = format!("coder-{id}");
        connection
            .send(json!(["REQ", subscription, {
                "kinds": [CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_RESULT],
                "#p": [self.inner.me],
            }]))
            .await?;
        // The worker's answers reach only subscriptions open before a
        // request: the connection takes jobs once this one is placed.
        loop {
            let frame = connection.next().await?;
            match frame[0].as_str() {
                Some("EOSE") if frame[1] == subscription.as_str() => break,
                Some("CLOSED") if frame[1] == subscription.as_str() => {
                    return Err(frame[2].as_str().unwrap_or("closed").into());
                }
                _ => {}
            }
        }
        {
            let mut state = self.state();
            if let Some(socket) = state.sockets.iter_mut().find(|socket| socket.id == id) {
                socket.ready = true;
            }
            // The two newest connections overlap; an older one closes.
            let ready = state.sockets.iter().filter(|socket| socket.ready).count();
            if ready > 2 {
                let mut extra = ready - 2;
                state.sockets.retain(|socket| {
                    let close = socket.ready && extra > 0;
                    extra -= usize::from(close);
                    !close
                });
            }
        }
        self.inner.changed.notify_waiters();
        loop {
            tokio::select! {
                frame = connection.next() => {
                    let frame = frame?;
                    if frame[0] == "CLOSED" && frame[1] == subscription.as_str() {
                        return Err(frame[2].as_str().unwrap_or("closed").into());
                    }
                    self.route(&subscription, frame);
                }
                message = out.recv() => match message {
                    Some(message) => connection.send(message).await?,
                    None => {
                        let _ = connection.send(json!(["CLOSE", subscription])).await;
                        let _ = connection.close().await;
                        return Ok(());
                    }
                },
            }
        }
    }

    /// Send a relay frame to the job it concerns: an answer event to the
    /// job whose request it tags, once, and the relay's `OK` to the job
    /// whose request it acknowledges.
    fn route(&self, subscription: &str, frame: Value) {
        let mut state = self.state();
        match frame[0].as_str() {
            Some("EVENT") if frame[1] == subscription => {
                let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                    return;
                };
                if state.seen_set.contains(&event.id) {
                    return;
                }
                let Some(job) = event
                    .tag_values("e")
                    .find_map(|request| state.jobs.get(request).cloned())
                else {
                    return;
                };
                state.seen.push_back(event.id.clone());
                state.seen_set.insert(event.id.clone());
                while state.seen.len() > SEEN {
                    if let Some(old) = state.seen.pop_front() {
                        state.seen_set.remove(&old);
                    }
                }
                let _ = job.send(frame);
            }
            Some("OK") => {
                if let Some(job) = frame[1].as_str().and_then(|id| state.jobs.get(id)) {
                    let _ = job.send(frame.clone());
                }
            }
            _ => {}
        }
    }

    /// Publish `request` on a connection whose subscription is placed,
    /// opening one when none is, within `wait`. Its answers arrive on the
    /// returned job.
    pub(crate) async fn publish(&self, request: &Event, wait: Duration) -> Result<Job, String> {
        let (sender, frames) = mpsc::unbounded_channel();
        self.state().jobs.insert(request.id.clone(), sender);
        let job = Job {
            link: self.clone(),
            request: request.id.clone(),
            frames,
        };
        let deadline = Instant::now() + wait;
        loop {
            let changed = self.inner.changed.notified();
            {
                let mut state = self.state();
                while let Some(at) = state.sockets.iter().rposition(|socket| socket.ready) {
                    if state.sockets[at]
                        .out
                        .send(json!(["EVENT", request]))
                        .is_ok()
                    {
                        return Ok(job);
                    }
                    // That connection just ended.
                    state.sockets.remove(at);
                }
                if state.sockets.is_empty() {
                    self.open(&mut state);
                }
                // A job keeps the connection fresh until it ends.
                if !state.kept {
                    state.kept = true;
                    let link = self.clone();
                    tokio::spawn(async move { link.keep().await });
                }
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Err("the relay could not be reached".into());
            }
        }
    }

    fn forget(&self, request: &str) {
        let mut state = self.state();
        state.jobs.remove(request);
        if !state.wanted && state.jobs.is_empty() {
            state.sockets.clear();
        }
        drop(state);
        self.inner.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use crate::basic_coder::{Door, Relay, Reply, Turn, lock};
    use crate::router::Context;
    use futures_util::{SinkExt, StreamExt};
    use nostr::domain::{Event, RelaySigner, Tag};
    use nostr::kinds::{CJ_CONVERSATION_REQUEST, CJ_CONVERSATION_RESULT};
    use nostr::nip44;
    use secp256k1::SecretKey;
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio_tungstenite::tungstenite::Message;

    fn public(secret: &SecretKey) -> secp256k1::XOnlyPublicKey {
        secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), secret)
            .x_only_public_key()
            .0
    }

    /// A relay with the chat worker behind it: it asks for NIP-42, answers
    /// a subscription with `EOSE`, and answers each conversation request
    /// with the worker's signed result, "you said: <task>", on every
    /// connection subscribed for the requester. Returns its URL, how many
    /// connections it accepted, and each request's payload.
    async fn relay(worker: SecretKey) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<Value>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let accepted = Arc::new(AtomicUsize::new(0));
        let payloads = Arc::new(Mutex::new(vec![]));
        let (count, seen) = (accepted.clone(), payloads.clone());
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                count.fetch_add(1, Ordering::SeqCst);
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    let send = |value: Value| Message::Text(value.to_string().into());
                    socket
                        .send(send(json!(["AUTH", "challenge"])))
                        .await
                        .unwrap();
                    let mut subscription = String::new();
                    while let Some(Ok(Message::Text(text))) = socket.next().await {
                        let frame: Value = serde_json::from_str(&text).unwrap();
                        match frame[0].as_str() {
                            Some("AUTH") => {
                                let id = frame[1]["id"].clone();
                                socket
                                    .send(send(json!(["OK", id, true, ""])))
                                    .await
                                    .unwrap();
                            }
                            Some("REQ") => {
                                subscription = frame[1].as_str().unwrap().to_owned();
                                socket
                                    .send(send(json!(["EOSE", subscription])))
                                    .await
                                    .unwrap();
                            }
                            Some("EVENT") => {
                                let request: Event =
                                    serde_json::from_value(frame[1].clone()).unwrap();
                                assert_eq!(request.kind, CJ_CONVERSATION_REQUEST);
                                socket
                                    .send(send(json!(["OK", request.id, true, ""])))
                                    .await
                                    .unwrap();
                                let from = secp256k1::XOnlyPublicKey::from_byte_array(
                                    (0..32)
                                        .map(|at| {
                                            u8::from_str_radix(
                                                &request.pubkey[at * 2..at * 2 + 2],
                                                16,
                                            )
                                            .unwrap()
                                        })
                                        .collect::<Vec<u8>>()
                                        .try_into()
                                        .unwrap(),
                                )
                                .unwrap();
                                let key = nip44::conversation_key(&worker, &from);
                                let payload: Value = serde_json::from_str(
                                    &nip44::decrypt(&request.content, &key).unwrap(),
                                )
                                .unwrap();
                                let body = json!({"v": 2, "type": "result",
                                    "text": format!("you said: {}", payload["task"].as_str().unwrap()),
                                    "model": "m"});
                                seen.lock().unwrap().push(payload);
                                let answer = RelaySigner::from_secret_hex(
                                    &worker.display_secret().to_string(),
                                )
                                .unwrap()
                                .sign(
                                    request.created_at,
                                    CJ_CONVERSATION_RESULT,
                                    vec![
                                        Tag::new(vec!["e".into(), request.id.clone()]),
                                        Tag::new(vec!["p".into(), request.pubkey.clone()]),
                                    ],
                                    nip44::encrypt(&body.to_string(), &key, [9; 32]).unwrap(),
                                );
                                socket
                                    .send(send(json!(["EVENT", subscription, answer])))
                                    .await
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                });
            }
        });
        (url, accepted, payloads)
    }

    /// Two messages in a row travel over the one connection the tab opened,
    /// signed in once, and each gets its own answer; the job asks for the
    /// worker's first response.
    #[tokio::test(flavor = "multi_thread")]
    async fn messages_reuse_the_kept_connection() {
        let worker = SecretKey::from_byte_array([0x42; 32]).unwrap();
        let (url, accepted, payloads) = relay(worker).await;
        let me = SecretKey::from_byte_array([0x43; 32]).unwrap();
        let worker_hex: String = public(&worker)
            .serialize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let door = Relay::new(&url, &worker_hex, me).unwrap();
        door.warm(&tokio::runtime::Handle::current());
        for text in ["first", "second"] {
            let reply = Arc::new(Mutex::new(Reply::default()));
            door.ask(vec![Turn::user(text)], Context::default(), reply.clone())
                .await;
            let reply = lock(&reply).clone();
            assert!(reply.failure.is_none(), "{:?}", reply.failure);
            assert_eq!(reply.text, format!("you said: {text}"));
        }
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
        assert_eq!(door.link().opened(), 1);
        assert!(
            payloads
                .lock()
                .unwrap()
                .iter()
                .all(|payload| payload["opener"] == true && payload["router"] == "chat-router-v1")
        );
        // In the background the connection closes; the next message opens
        // one again.
        door.rest();
        let reply = Arc::new(Mutex::new(Reply::default()));
        door.ask(vec![Turn::user("third")], Context::default(), reply.clone())
            .await;
        assert_eq!(lock(&reply).text, "you said: third");
        assert_eq!(accepted.load(Ordering::SeqCst), 2);
    }
}
