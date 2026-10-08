//! Admitted browser host sessions. A lost dispatch is never queued or replayed.
use coder_access::{RelayPolicy, Right, protocol::Access};
use coder_host_wire::{Assembler, TermRequest, ToDevice, ToHost, fragments};
use coder_pty::{
    ext::RecordsFrame,
    wire::{Frame, RESULT, TerminalResult},
};
use coder_reach::{
    Refusal, artifact,
    channel::{Channel, ClientConfig},
};
use nostr::domain::Event;
use secp256k1::SecretKey;
use std::{collections::VecDeque, time::Duration};
use tokio::io::{AsyncRead, AsyncWrite};

pub mod pairing;
pub mod workbench;

#[cfg(target_arch = "wasm32")]
pub mod browser;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ready,
    Disconnected,
    Unknown,
    Revoked,
    Stale,
    Behind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotAdmitted,
    Stale,
    Malformed,
    Limit,
    Unknown,
    Disconnected,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug)]
pub enum Incoming {
    Frame(Frame),
    Records(RecordsFrame),
}

/// Explicit host admission, held only in page memory. Connectivity supplies no rights.
pub struct Admission {
    secret: SecretKey,
    access: Access,
    policy: RelayPolicy,
    generation: u64,
    state: State,
    features: coder_pty::ext::Features,
}
impl Admission {
    pub fn new(
        secret: SecretKey,
        access: Access,
        policy: RelayPolicy,
        generation: u64,
        now: u64,
    ) -> Result<Self> {
        access
            .verify(&secret, now, policy)
            .map_err(|_| Error::NotAdmitted)?;
        Ok(Self {
            secret,
            access,
            policy,
            generation,
            state: State::Ready,
            features: coder_pty::ext::Features::NONE,
        })
    }
    pub fn negotiate<S: AsRef<str>>(&mut self, capabilities: &[S]) {
        self.features = coder_pty::ext::Features::advertised(capabilities);
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn host(&self) -> &str {
        &self.access.grant.host
    }
    /// The public key the host granted. The secret never leaves this admission.
    pub fn device(&self) -> &str {
        &self.access.grant.device
    }
    pub fn expires_at(&self) -> u64 {
        self.access.grant.expires_at
    }
    pub fn features(&self) -> coder_pty::ext::Features {
        self.features
    }
    /// Check page-memory authority even when the transport is idle.
    pub fn current(&self, now: u64) -> bool {
        now < self.access.grant.expires_at
            && matches!(self.state, State::Ready | State::Unknown | State::Behind)
            && self.access.verify(&self.secret, now, self.policy).is_ok()
            && self.access.grant.rights.contains(Right::Terminal)
    }
    pub fn can_observe(&self, now: u64) -> bool {
        self.current(now) && self.access.grant.rights.contains(Right::Observe)
    }
    /// Prepare an owned bounded read before releasing the mount's state borrow.
    pub fn prepare_thread(
        &self,
        thread: &str,
        before: Option<u64>,
        now: u64,
    ) -> Result<ThreadRead> {
        if !self.can_observe(now) {
            return Err(Error::NotAdmitted);
        }
        let client = coder_access::client::Client::device_at(
            self.access.clone(),
            self.secret,
            self.policy,
            now,
        )
        .map_err(|_| Error::NotAdmitted)?;
        let pending = client
            .prepare(
                coder_access::protocol::Operation::ReadThread {
                    thread: thread.into(),
                    before,
                },
                now,
            )
            .map_err(|_| Error::Malformed)?;
        let admission = Self::new(
            self.secret,
            self.access.clone(),
            self.policy,
            self.generation,
            now,
        )?;
        Ok(ThreadRead {
            admission,
            client,
            pending,
        })
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn disconnect(&mut self) {
        self.state = State::Disconnected;
    }
    pub fn revoke(&mut self) {
        self.state = State::Revoked;
    }
    pub fn check(&self, request: &TermRequest, now: u64) -> Result<()> {
        if self.state != State::Ready {
            return Err(Error::Disconnected);
        }
        self.access
            .verify(&self.secret, now, self.policy)
            .map_err(|_| Error::NotAdmitted)?;
        self.access
            .grant
            .rights
            .require(Right::Terminal)
            .map_err(|_| Error::NotAdmitted)?;
        let checked = match request {
            TermRequest::Open(r) => r.check(),
            TermRequest::Attach(r) => r.check_with(self.features),
            TermRequest::Detach(r) => r.check(),
            TermRequest::Input(r) => r.check_with(self.features),
            TermRequest::Resize(r) => r.check_with(self.features),
            TermRequest::Signal(r) => r.check_with(self.features),
            TermRequest::Close(r) => r.check(),
            TermRequest::History(r) => r.check_with(self.features),
            TermRequest::BlockPage(r) => r.check_with(self.features),
            TermRequest::Seat(r) => r.check_with(self.features),
            TermRequest::Proposal(r) => r.check(self.features),
            TermRequest::Share(r) => r.check_with(self.features),
            TermRequest::Unshare(r) => r.check_with(self.features),
            TermRequest::Handoff(r) => r.check_with(self.features),
            TermRequest::SharePause(r) => r.check_with(self.features),
            TermRequest::Viewers(r) => {
                if !self.features.shares {
                    return Err(Error::NotAdmitted);
                }
                r.check()
            }
            TermRequest::SessionRead(r) => r.check_with(self.features),
            TermRequest::SessionWrite(r) => r.check_with(self.features),
            TermRequest::SessionList(r) => r.check_with(self.features),
            TermRequest::SessionRemove(r) => r.check_with(self.features),
            TermRequest::EngineStatus(r) => r.check(),
        };
        checked.map_err(|_| Error::Malformed)?;
        let value = request.to_value();
        let expected = terminal_generation(self.host(), self.generation);
        if value
            .get("terminal")
            .and_then(|t| t.get("generation"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|g| g != expected)
        {
            return Err(Error::Stale);
        }
        // Reparse the exact wire bytes; unsupported versions and malformed IDs fail locally.
        let bytes = ToHost::Terminal(request.clone()).encode();
        if bytes.len() > 64 * 1024 {
            return Err(Error::Limit);
        }
        ToHost::decode(&bytes).map_err(|_| Error::Malformed)?;
        Ok(())
    }
    fn result(&mut self, result: &TerminalResult) {
        use coder_pty::wire::Reason;
        self.state = match result.reason {
            Some(Reason::Revoked | Reason::NotAdmitted) => State::Revoked,
            Some(Reason::Lost | Reason::Stale) => State::Stale,
            _ => State::Ready,
        };
    }
    fn config(&self) -> ClientConfig {
        ClientConfig {
            device: self.secret,
            host: self.host().into(),
            grant: self.access.grant.grant.clone(),
            epoch: self.access.grant.epoch,
            generation: self.generation,
            timeout: Duration::from_secs(10),
        }
    }
}
/// The identical generation derivation used by the resident host.
pub fn terminal_generation(host: &str, generation: u64) -> String {
    use nostr::contracts;
    let mut bytes = b"openagents.host-terminal-generation.v1\0".to_vec();
    bytes.extend_from_slice(host.as_bytes());
    bytes.extend_from_slice(&generation.to_be_bytes());
    contracts::digest_bytes(&bytes)
        .trim_start_matches("sha256:")
        .to_owned()
}

/// A proven direct channel. Failed sends leave Unknown and require a fresh read/attachment.
pub struct Direct<S> {
    pub admission: Admission,
    channel: Channel<S>,
    assembler: Assembler,
    incoming: VecDeque<Incoming>,
    bytes: usize,
}
impl<S: AsyncRead + AsyncWrite + Unpin> Direct<S> {
    pub async fn connect(stream: S, mut admission: Admission, now: u64) -> Result<Self> {
        let channel = coder_reach::channel::connect(stream, &admission.config(), now)
            .await
            .map_err(|e| {
                admission.state = if e.code == Refusal::Stale {
                    State::Stale
                } else {
                    State::Disconnected
                };
                Error::Disconnected
            })?;
        Ok(Self {
            admission,
            channel,
            assembler: Assembler::default(),
            incoming: VecDeque::new(),
            bytes: 0,
        })
    }
    pub fn take_incoming(&mut self) -> Option<Incoming> {
        let item = self.incoming.pop_front()?;
        self.bytes = self.bytes.saturating_sub(incoming_size(&item));
        Some(item)
    }
    pub async fn request(&mut self, request: TermRequest, now: u64) -> Result<TerminalResult> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            tokio::time::timeout(Duration::from_secs(12), self.dispatch(request, now))
                .await
                .map_err(|_| Error::Unknown)?
        }
        #[cfg(target_arch = "wasm32")]
        {
            use futures_util::future::{Either, select};
            match select(
                Box::pin(self.dispatch(request, now)),
                Box::pin(gloo_timers::future::TimeoutFuture::new(12_000)),
            )
            .await
            {
                Either::Left((result, _)) => result,
                Either::Right(_) => Err(Error::Unknown),
            }
        }
    }
    async fn dispatch(&mut self, request: TermRequest, now: u64) -> Result<TerminalResult> {
        self.admission.check(&request, now)?;
        let bytes = ToHost::Terminal(request.clone()).encode();
        // Mark before the first write; cancellation cannot make an uncertain operation ready again.
        self.admission.state = State::Unknown;
        for part in fragments(&bytes).map_err(|_| Error::Limit)? {
            self.channel.send(&part).await.map_err(|_| Error::Unknown)?;
        }
        for _ in 0..128 {
            let part = self
                .channel
                .recv()
                .await
                .map_err(|_| Error::Unknown)?
                .ok_or(Error::Unknown)?;
            let Some(bytes) = self.assembler.push(&part).map_err(|_| Error::Malformed)? else {
                continue;
            };
            match ToDevice::decode(&bytes).map_err(|_| Error::Malformed)? {
                ToDevice::Result(result) if result.request == request.request() => {
                    self.admission.result(&result);
                    return Ok(result);
                }
                ToDevice::Frame(frame) => self.retain(Incoming::Frame(frame))?,
                ToDevice::Records(records) => self.retain(Incoming::Records(records))?,
                ToDevice::Renewal(event) => {
                    self.admission.access = self
                        .admission
                        .access
                        .renewed(event, &self.admission.secret, now, self.admission.policy)
                        .map_err(|_| Error::NotAdmitted)?;
                }
                ToDevice::Closing(_) => {
                    self.admission.revoke();
                    return Err(Error::NotAdmitted);
                }
                _ => return Err(Error::Malformed),
            }
        }
        Err(Error::Limit)
    }
    /// Receives output while idle; gaps remain explicit in the portable terminal state.
    pub async fn next(&mut self) -> Result<Incoming> {
        if let Some(item) = self.take_incoming() {
            return Ok(item);
        }
        if self.admission.state != State::Ready {
            return Err(Error::Disconnected);
        }
        for _ in 0..128 {
            let part = match self.channel.recv().await {
                Ok(Some(p)) => p,
                _ => {
                    self.admission.disconnect();
                    return Err(Error::Disconnected);
                }
            };
            let Some(bytes) = self.assembler.push(&part).map_err(|_| Error::Malformed)? else {
                continue;
            };
            match ToDevice::decode(&bytes).map_err(|_| Error::Malformed)? {
                ToDevice::Frame(f) => {
                    let item = Incoming::Frame(f);
                    self.check_incoming(&item)?;
                    return Ok(item);
                }
                ToDevice::Records(r) => {
                    let item = Incoming::Records(r);
                    self.check_incoming(&item)?;
                    return Ok(item);
                }
                ToDevice::Closing(_) => {
                    self.admission.revoke();
                    return Err(Error::NotAdmitted);
                }
                _ => return Err(Error::Malformed),
            }
        }
        Err(Error::Limit)
    }
    fn check_incoming(&self, item: &Incoming) -> Result<()> {
        let generation = match item {
            Incoming::Frame(f) => &f.terminal.generation,
            Incoming::Records(r) => &r.terminal.generation,
        };
        if *generation != terminal_generation(self.admission.host(), self.admission.generation) {
            return Err(Error::Stale);
        }
        Ok(())
    }
    fn retain(&mut self, item: Incoming) -> Result<()> {
        self.check_incoming(&item)?;
        self.bytes += incoming_size(&item);
        if self.incoming.len() >= 64 || self.bytes > 256 * 1024 {
            self.admission.state = State::Behind;
            self.incoming.clear();
            self.bytes = 0;
            return Err(Error::Limit);
        }
        self.incoming.push_back(item);
        Ok(())
    }
}
fn incoming_size(item: &Incoming) -> usize {
    match item {
        Incoming::Frame(f) => serde_json::to_vec(f).map_or(usize::MAX, |v| v.len()),
        Incoming::Records(r) => serde_json::to_vec(r).map_or(usize::MAX, |v| v.len()),
    }
}

/// Relay carriage provides signed artifacts, never a grant or an input queue.
#[allow(async_fn_in_trait)]
pub trait Relay {
    /// Drain a complete event already received while waiting for a command reply.
    fn take_event(&mut self) -> Result<Option<Event>> {
        Ok(None)
    }
    async fn subscribe(&mut self, _host: &str, _recipient: &str, _attachment: &str) -> Result<()> {
        Err(Error::Disconnected)
    }
    async fn next(&mut self) -> Result<Event> {
        Err(Error::Disconnected)
    }
    async fn exchange(
        &mut self,
        event: &Event,
        host: &str,
        recipient: &str,
        now: u64,
    ) -> Result<Event>;
}
pub struct Relayed<R> {
    pub admission: Admission,
    relay: R,
    attachment: Option<String>,
}
impl<R: Relay> Relayed<R> {
    pub fn new(admission: Admission, relay: R) -> Self {
        Self {
            admission,
            relay,
            attachment: None,
        }
    }
    pub async fn request(&mut self, request: TermRequest, now: u64) -> Result<TerminalResult> {
        self.admission.check(&request, now)?;
        let event = artifact::seal(
            &request.to_value(),
            request.schema(),
            &self.admission.secret,
            self.admission.host(),
            request.request(),
            now,
            now.saturating_add(60),
        )
        .map_err(|_| Error::Malformed)?;
        self.admission.state = State::Unknown;
        let reply = self
            .relay
            .exchange(
                &event,
                self.admission.host(),
                &self.admission.access.grant.device,
                now,
            )
            .await?;
        let (result, seal): (TerminalResult, _) = artifact::open(
            &reply,
            &self.admission.secret,
            self.admission.host(),
            &self.admission.access.grant.device,
            RESULT,
        )
        .map_err(|_| Error::Malformed)?;
        if result.request != request.request()
            || seal.issued_at < now
            || seal.issued_at > now.saturating_add(60)
            || seal.retain_until <= seal.issued_at
            || reply.tag_values("h").collect::<Vec<_>>() != [request.request()]
        {
            return Err(Error::Malformed);
        }
        if let Some(coder_pty::wire::Value::Attached { attachment, .. }) = &result.value {
            self.relay
                .subscribe(
                    self.admission.host(),
                    &self.admission.access.grant.device,
                    attachment,
                )
                .await?;
            self.attachment = Some(attachment.clone());
        }
        self.admission.result(&result);
        Ok(result)
    }
    pub fn take_incoming(&mut self, now: u64) -> Result<Option<Incoming>> {
        if !self.admission.current(now) {
            self.admission.revoke();
            return Err(Error::NotAdmitted);
        }
        self.relay
            .take_event()?
            .map(|event| self.decode_incoming(event, now))
            .transpose()
    }
    pub async fn next(&mut self, now: u64) -> Result<Incoming> {
        if let Some(incoming) = self.take_incoming(now)? {
            return Ok(incoming);
        }
        if self.admission.state != State::Ready {
            return Err(Error::Disconnected);
        }
        let event = self.relay.next().await?;
        self.decode_incoming(event, now)
    }
    fn decode_incoming(&self, event: Event, now: u64) -> Result<Incoming> {
        let mailbox = self.attachment.as_ref().ok_or(Error::Disconnected)?;
        if event.tag_values("h").collect::<Vec<_>>() != [mailbox.as_str()] {
            return Err(Error::Malformed);
        }
        let frame: std::result::Result<(Frame, _), _> = artifact::open(
            &event,
            &self.admission.secret,
            self.admission.host(),
            &self.admission.access.grant.device,
            coder_pty::wire::FRAME,
        );
        let incoming = if let Ok((frame, seal)) = frame {
            if now >= seal.retain_until {
                return Err(Error::Stale);
            }
            frame.check().map_err(|_| Error::Malformed)?;
            if frame.attachment != *mailbox
                || frame.terminal.generation
                    != terminal_generation(self.admission.host(), self.admission.generation)
            {
                return Err(Error::Stale);
            }
            Incoming::Frame(frame)
        } else {
            let (records, seal): (RecordsFrame, _) = artifact::open(
                &event,
                &self.admission.secret,
                self.admission.host(),
                &self.admission.access.grant.device,
                coder_pty::ext::RECORDS,
            )
            .map_err(|_| Error::Malformed)?;
            if now >= seal.retain_until {
                return Err(Error::Stale);
            }
            records.check().map_err(|_| Error::Malformed)?;
            if records.attachment != *mailbox
                || records.terminal.generation
                    != terminal_generation(self.admission.host(), self.admission.generation)
            {
                return Err(Error::Stale);
            }
            Incoming::Records(records)
        };
        Ok(incoming)
    }
}

#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
pub use coder_reach::browser as reach_socket;
pub use coder_reach::new_id as new_request_id;

impl Drop for Admission {
    fn drop(&mut self) {
        self.secret.non_secure_erase();
    }
}

/// An opaque original thread read. Dropping it erases its page-memory key copies.
pub struct ThreadRead {
    pub(crate) admission: Admission,
    pub(crate) client: coder_access::client::Client,
    pub(crate) pending: coder_access::client::Pending,
}

impl ThreadRead {
    fn verify(&self, event: &Event, now: u64) -> Result<coder_access::thread::ThreadPage> {
        if !self.admission.can_observe(now) {
            return Err(Error::NotAdmitted);
        }
        let coder_access::protocol::Outcome::Thread { thread } = self
            .client
            .verify_reply(&self.pending, event, now)
            .map_err(|_| Error::NotAdmitted)?
        else {
            return Err(Error::Malformed);
        };
        let coder_access::protocol::Operation::ReadThread { before, .. } = &self.pending.request.op
        else {
            return Err(Error::Malformed);
        };
        if thread.start.saturating_add(thread.turns.len() as u64)
            != before.unwrap_or(thread.total).min(thread.total)
        {
            return Err(Error::Stale);
        }
        Ok(*thread)
    }
}
