//! One proven route to a host: a direct channel, or relay fallback.
//!
//! Both routes carry the same operations with the same admission. A NIP-HOST
//! call sends the exact signed request of the direct artifact binding, and
//! the host answers with its signed reply either way. A NIP-TERM request is
//! a channel message on the direct route and a sealed `3188` artifact on the
//! relay. Terminal frames arrive through [`Link::next_frame`] on both.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_access::protocol::{Operation, Outcome};
use coder_pty::wire::{FRAME, Frame, RESULT, TerminalResult, Value as TermValue};
use coder_reach::channel::{ClientConfig, connect};
use nostr_transport::Connection;
use serde_json::json;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::Device;
use crate::message::{Assembler, TermRequest, ToDevice, ToHost, fragments};
use crate::{Error, Result, unix_time};

/// How long one operation waits for its answer.
const CALL_TIMEOUT: Duration = Duration::from_secs(12);
/// How long a relay-carried terminal request stays valid.
const TERMINAL_LIFETIME: u64 = 60;

/// Which route a link uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// A direct channel to `host:port` over TCP, or to a `ws` or `wss` URL.
    Direct(String),
    /// Relay fallback through this relay.
    Relay(String),
}

type Waiters = Arc<Mutex<HashMap<String, oneshot::Sender<ToDevice>>>>;

struct Direct {
    outbound: mpsc::Sender<ToHost>,
    waiters: Waiters,
    closed: watch::Receiver<Option<Option<String>>>,
    tasks: Vec<JoinHandle<()>>,
}

/// One proven route. Dropping it closes a direct channel and stops relay
/// frame subscriptions.
pub struct Link {
    device: Arc<Device>,
    route: Route,
    /// The host generation this route was proven against, when known. A
    /// relay route follows fresh presence ([`Link::refresh_generation`]).
    generation: Mutex<Option<u64>>,
    direct: Option<Direct>,
    frames_in: mpsc::UnboundedSender<Frame>,
    frames: tokio::sync::Mutex<mpsc::UnboundedReceiver<Frame>>,
    subscriptions: Mutex<Vec<JoinHandle<()>>>,
    /// The newest renewed grant envelope the host sent on this channel,
    /// not yet taken ([`Link::take_renewal`]).
    renewal: Arc<Mutex<Option<nostr::domain::Event>>>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("route", &self.route)
            .finish_non_exhaustive()
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Link {
    /// Close the route now, even while other holders keep this link: drop a
    /// direct channel's socket and stop relay frame subscriptions.
    pub fn shutdown(&self) {
        if let Some(direct) = &self.direct {
            for task in &direct.tasks {
                task.abort();
            }
        }
        for task in std::mem::take(&mut *lock(&self.subscriptions)) {
            task.abort();
        }
    }

    /// Open a direct channel over `stream` to `address`, expecting the host
    /// generation from fresh presence. The stream is a TCP connection or a
    /// [`WebSocketStream`](super::WebSocketStream).
    ///
    /// # Errors
    /// Returns the handshake's refusal. One whose detail is
    /// `coder_reach::channel::UNAUTHENTICATED` came before the host proved
    /// its key and says nothing about this device's grant.
    pub async fn direct<S>(
        device: Arc<Device>,
        stream: S,
        address: String,
        generation: u64,
        timeout: Duration,
    ) -> Result<Self>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let config = ClientConfig {
            device: device.secret,
            host: device.host().to_owned(),
            grant: device.grant().to_owned(),
            epoch: device.epoch(),
            generation,
            timeout,
        };
        let channel = connect(stream, &config, unix_time()?).await?;
        let (mut reader, mut writer) = channel.into_split();
        let (frames_in, frames) = mpsc::unbounded_channel();
        let waiters: Waiters = Arc::default();
        let (closed_tx, closed) = watch::channel(None);
        let (outbound, mut queue) = mpsc::channel::<ToHost>(256);

        let writer_task = tokio::spawn(async move {
            while let Some(message) = queue.recv().await {
                let Ok(parts) = fragments(&message.encode()) else {
                    continue;
                };
                for part in parts {
                    if writer.send(&part).await.is_err() {
                        return;
                    }
                }
            }
            let _ = writer.close().await;
        });
        let reader_waiters = waiters.clone();
        let reader_frames = frames_in.clone();
        let renewal: Arc<Mutex<Option<nostr::domain::Event>>> = Arc::default();
        let reader_renewal = renewal.clone();
        let reader_task = tokio::spawn(async move {
            let mut assembler = Assembler::default();
            let mut code = None;
            while let Ok(Some(payload)) = reader.recv().await {
                let Ok(Some(message)) = assembler.push(&payload) else {
                    continue;
                };
                match ToDevice::decode(&message) {
                    Ok(ToDevice::Frame(frame)) => {
                        let _ = reader_frames.send(frame);
                    }
                    Ok(ToDevice::Closing(sent)) => code = Some(sent),
                    // Kept for the device to check and store; the channel
                    // itself follows the renewed grant on the host's side.
                    Ok(ToDevice::Renewal(event)) => *lock(&reader_renewal) = Some(event),
                    Ok(answer) => {
                        let key = match &answer {
                            ToDevice::Answer(event) => {
                                event.tag_values("h").next().map(str::to_owned)
                            }
                            ToDevice::Result(result) => Some(result.request.clone()),
                            ToDevice::Pong(nonce) => Some(format!("ping:{nonce}")),
                            _ => None,
                        };
                        let waiter = key.and_then(|key| lock(&reader_waiters).remove(&key));
                        if let Some(waiter) = waiter {
                            let _ = waiter.send(answer);
                        }
                    }
                    Err(_) => {}
                }
            }
            // Pending calls see their senders dropped and report the close.
            lock(&reader_waiters).clear();
            let _ = closed_tx.send(Some(code));
        });
        Ok(Self {
            device,
            route: Route::Direct(address),
            generation: Mutex::new(Some(generation)),
            direct: Some(Direct {
                outbound,
                waiters,
                closed,
                tasks: vec![writer_task, reader_task],
            }),
            frames_in,
            frames: tokio::sync::Mutex::new(frames),
            subscriptions: Mutex::default(),
            renewal,
        })
    }

    /// A relay fallback route. The caller proved the relay first.
    #[must_use]
    pub fn relay(device: Arc<Device>, relay: String) -> Self {
        let (frames_in, frames) = mpsc::unbounded_channel();
        Self {
            device,
            route: Route::Relay(relay),
            generation: Mutex::new(None),
            direct: None,
            frames_in,
            frames: tokio::sync::Mutex::new(frames),
            subscriptions: Mutex::default(),
            renewal: Arc::default(),
        }
    }

    /// The renewed grant envelope the host sent on this channel, once. The
    /// device checks it with `coder_access::Access::renewed` before it
    /// stores it; a relay route never carries one.
    #[must_use]
    pub fn take_renewal(&self) -> Option<nostr::domain::Event> {
        lock(&self.renewal).take()
    }

    /// A relay fallback route to a host whose fresh presence named
    /// `generation`.
    #[must_use]
    pub fn relay_at(device: Arc<Device>, relay: String, generation: u64) -> Self {
        let link = Self::relay(device, relay);
        link.note_generation(generation);
        link
    }

    /// The host generation this route was proven against: the handshake's
    /// on a direct channel, and the presence the connector read before it
    /// fell back to the relay. A NIP-TERM terminal reference names the
    /// terminal generation derived from it.
    #[must_use]
    pub fn generation(&self) -> Option<u64> {
        *lock(&self.generation)
    }

    /// Read the host's generation again from fresh presence, on a relay
    /// route, and return the generation this link now names.
    ///
    /// A host that restarts closes a direct channel, so the supervisor
    /// proves a new one against the new generation. A relay route never
    /// closes: without this, it keeps naming the generation it was proven
    /// against, and every terminal opened through it after a restart is
    /// refused as `lost`. A direct channel keeps its handshake's generation.
    ///
    /// # Errors
    /// Reports an unreachable relay or presence that is missing or stale.
    pub async fn refresh_generation(&self) -> Result<Option<u64>> {
        let Route::Relay(relay) = &self.route else {
            return Ok(self.generation());
        };
        let reach = super::fetch_reach(&self.device, relay).await?;
        self.note_generation(reach.presence.presence.generation);
        Ok(self.generation())
    }

    /// Follow `generation`, from fresh presence, on a relay route. A
    /// generation never goes backward; a direct channel keeps its own.
    pub fn note_generation(&self, generation: u64) {
        if self.direct.is_some() {
            return;
        }
        let mut held = lock(&self.generation);
        if held.is_none_or(|held| generation > held) {
            *held = Some(generation);
        }
    }

    /// The route this link uses.
    #[must_use]
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// The device this link acts for.
    #[must_use]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    /// Whether a direct channel has closed, and the host's code if it sent
    /// one. A relay link never reports closed.
    #[must_use]
    pub fn closed(&self) -> Option<Option<String>> {
        self.direct.as_ref().and_then(|d| d.closed.borrow().clone())
    }

    /// Wait until a direct channel closes; a relay link waits forever.
    pub async fn wait_closed(&self) -> Option<String> {
        let Some(direct) = &self.direct else {
            return std::future::pending().await;
        };
        let mut closed = direct.closed.clone();
        loop {
            if let Some(code) = closed.borrow_and_update().clone() {
                return code;
            }
            if closed.changed().await.is_err() {
                return None;
            }
        }
    }

    /// Check the route answers: a ping over a direct channel, an
    /// authenticated relay connection otherwise.
    ///
    /// # Errors
    /// Reports a closed channel or an unreachable relay.
    pub async fn ping(&self) -> Result<()> {
        match &self.route {
            Route::Direct(_) => {
                let nonce = coder_reach::new_id()[..16].to_owned();
                match self
                    .exchange(format!("ping:{nonce}"), ToHost::Ping(nonce))
                    .await?
                {
                    ToDevice::Pong(_) => Ok(()),
                    _ => Err(Error::Closed(None)),
                }
            }
            Route::Relay(relay) => {
                let socket =
                    Connection::connect(relay, &self.device.secret, Duration::from_secs(5))
                        .await
                        .map_err(Error::Transport)?;
                let _ = socket.close().await;
                Ok(())
            }
        }
    }

    /// Send one NIP-HOST operation and verify the host's signed reply.
    ///
    /// # Errors
    /// A host refusal is `Error::Access` with its code and any missing right.
    pub async fn call(&self, op: Operation) -> Result<Outcome> {
        let pending = self.device.client.prepare(op, unix_time()?)?;
        match &self.route {
            Route::Relay(_) => Ok(self.device.client.send(&pending).await?),
            Route::Direct(_) => {
                let answer = self
                    .exchange(
                        pending.request.request.clone(),
                        ToHost::Call(pending.event.clone()),
                    )
                    .await?;
                let ToDevice::Answer(reply) = answer else {
                    return Err(Error::Closed(None));
                };
                Ok(self
                    .device
                    .client
                    .verify_reply(&pending, &reply, unix_time()?)?)
            }
        }
    }

    /// Send one NIP-TERM request and return its result. A relay attach also
    /// starts delivering that attachment's frames to [`Link::next_frame`].
    ///
    /// # Errors
    /// Reports transport failures. A NIP-TERM refusal is a result with
    /// status `refused`, not an error.
    pub async fn terminal(&self, request: TermRequest) -> Result<TerminalResult> {
        let id = request.request().to_owned();
        let result = match &self.route {
            Route::Direct(_) => match self.exchange(id, ToHost::Terminal(request)).await? {
                ToDevice::Result(result) => result,
                _ => return Err(Error::Closed(None)),
            },
            Route::Relay(relay) => self.relay_terminal(relay, &request).await?,
        };
        if let (Route::Relay(relay), Some(TermValue::Attached { attachment, .. })) =
            (&self.route, &result.value)
        {
            self.subscribe(relay.clone(), attachment.clone());
        }
        Ok(result)
    }

    /// The next terminal frame from any attachment on this link.
    pub async fn next_frame(&self, timeout: Duration) -> Option<Frame> {
        tokio::time::timeout(timeout, self.frames.lock().await.recv())
            .await
            .ok()
            .flatten()
    }

    async fn exchange(&self, key: String, message: ToHost) -> Result<ToDevice> {
        let direct = self.direct.as_ref().ok_or(Error::Closed(None))?;
        if let Some(code) = direct.closed.borrow().clone() {
            return Err(Error::Closed(code));
        }
        let (sender, receiver) = oneshot::channel();
        lock(&direct.waiters).insert(key.clone(), sender);
        if direct.outbound.send(message).await.is_err() {
            lock(&direct.waiters).remove(&key);
            return Err(Error::Closed(direct.closed.borrow().clone().flatten()));
        }
        match tokio::time::timeout(CALL_TIMEOUT, receiver).await {
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(_)) => Err(Error::Closed(direct.closed.borrow().clone().flatten())),
            Err(_) => {
                lock(&direct.waiters).remove(&key);
                Err(Error::Transport("the host did not answer in time".into()))
            }
        }
    }

    async fn relay_terminal(&self, relay: &str, request: &TermRequest) -> Result<TerminalResult> {
        let device = &self.device;
        let now = unix_time()?;
        let id = request.request();
        let event = coder_reach::artifact::seal(
            &request.to_value(),
            request.schema(),
            &device.secret,
            device.host(),
            id,
            now,
            now + TERMINAL_LIFETIME,
        )?;
        let reply = tokio::time::timeout(CALL_TIMEOUT, async {
            let mut session =
                coder_connect::transport::Session::connect(relay, &device.secret, device.policy)
                    .await
                    .map_err(|e| Error::Transport(e.message))?;
            session
                .exchange_event(
                    &event,
                    id,
                    (now, now + TERMINAL_LIFETIME),
                    device.host(),
                    &device.key(),
                )
                .await
                .map_err(|e| Error::Transport(e.message))
        })
        .await
        .map_err(|_| Error::Transport("the host did not answer in time".into()))??;
        let (result, _): (TerminalResult, _) = coder_reach::artifact::open(
            &reply,
            &device.secret,
            device.host(),
            &device.key(),
            RESULT,
        )?;
        if result.request != id {
            return Err(Error::Transport(
                "the result answers another request".into(),
            ));
        }
        Ok(result)
    }

    /// Deliver one attachment's relay-carried frames. Reconnects replay
    /// retained frames; the client state ignores the duplicates.
    fn subscribe(&self, relay: String, attachment: String) {
        let device = self.device.clone();
        let frames = self.frames_in.clone();
        let task = tokio::spawn(async move {
            let host = device.host().to_owned();
            let me = device.key();
            loop {
                let Ok(mut socket) =
                    Connection::connect(&relay, &device.secret, Duration::from_secs(110)).await
                else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                };
                let filter =
                    json!({"kinds": [3188], "authors": [host], "#p": [me], "#h": [attachment]});
                if socket
                    .send(json!(["REQ", attachment, filter]))
                    .await
                    .is_err()
                {
                    continue;
                }
                while let Ok(frame) = socket.next().await {
                    if frame[0] != "EVENT" {
                        continue;
                    }
                    let Ok(event) = serde_json::from_value(frame[2].clone()) else {
                        continue;
                    };
                    let opened: std::result::Result<(Frame, _), _> =
                        coder_reach::artifact::open(&event, &device.secret, &host, &me, FRAME);
                    if let Ok((frame, _)) = opened
                        && frame.check().is_ok()
                        && frame.attachment == attachment
                        && frames.send(frame).is_err()
                    {
                        return;
                    }
                }
            }
        });
        lock(&self.subscriptions).push(task);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
