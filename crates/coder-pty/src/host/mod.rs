//! The host half: terminals on real PTYs, their replay rings, and the
//! attachments that read them.
//!
//! # Process ownership
//!
//! A terminal's child leads a session of its own, so its process group is
//! the terminal's tree, the same ownership `crates/supervise` gives a job.
//! Close, idle expiry, and host shutdown end that tree the same way: the
//! group gets `SIGHUP` and `SIGTERM` (an interactive shell ignores
//! `SIGTERM` but not a hang-up), up to `GRACE` to exit, then `SIGKILL`,
//! and the direct child is reaped before the terminal reports its exit.
//! Ending waits, within `GRACE`, until the group holds no process, since
//! a killed descendant lingers as a zombie until init or a subreaper reaps
//! it.
//! A child that exits on its own takes whatever is left in its group with
//! it. A descendant that calls `setsid` leaves the group and escapes this,
//! as it escapes the supervisor.
//!
//! A prepared command can be wrapped before it is spawned — the
//! `crates/coder-boundary` write boundary, for example — through
//! [`Wrap`]. The host then sets the working directory, clears the
//! environment, and applies the allowlisted variables on the wrapped
//! command.
//!
//! # Output
//!
//! One reader thread per terminal reads the PTY into frames of at most
//! [`Config::frame_max`] bytes, appends them to the terminal's
//! [`Ring`], and pumps every attachment. Delivery never blocks the reader:
//! a [`FrameSink`] that is full keeps its place and is pumped again on the
//! next output or tick, and an attachment that falls further behind than
//! the ring reaches receives a gap frame. Each attachment has a byte
//! budget per second, the smaller of the client's request and
//! [`Config::rate_max`].
//!
//! # Authority
//!
//! [`Rights`] answers whether a principal holds `terminal` or `observe`.
//! The host asks on every operation, and [`Host::tick`] asks again for
//! every attachment, ending those whose right was revoked.
//!
//! # Lifetime
//!
//! A terminal survives its clients: detaching or dropping a transport does
//! not end it. It ends when a client closes it, when no client has been
//! attached or typed for [`Config::idle`], or when the host shuts down.
//! A terminal reference carries the host generation, and a reference from
//! an earlier generation is refused as `lost`: a restarted host never
//! resumes something else under an old name.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::ring::Ring;
use crate::wire::{
    self, Attach, Body, Cause, Close, Detach, Detached, Exit, Frame, Input, Launch, Mode, Open,
    Reason, Refusal, Resize, Signal, Size, Status, TerminalRef, Value,
};

#[cfg(unix)]
mod sys;
#[cfg(not(unix))]
#[path = "unsupported.rs"]
mod sys;

/// How long a terminal's group has to exit after a hang-up before it is
/// killed, and how long the reader drains output after the child exits.
#[cfg(unix)]
pub const GRACE: Duration = supervise::GRACE;
/// How long a terminal's group has to exit after a hang-up before it is
/// killed, and how long the reader drains output after the child exits.
#[cfg(not(unix))]
pub const GRACE: Duration = Duration::from_millis(250);

/// How long one reader wait lasts before it checks the child.
const POLL: Duration = Duration::from_millis(25);
/// How often an ended terminal's group is checked for leftover processes.
const SETTLE_POLL: Duration = Duration::from_millis(5);
/// How often the background ticker pumps and expires terminals.
const TICK: Duration = Duration::from_millis(100);
/// How many applied requests the host remembers to answer exact retries.
const RECENT_MAX: usize = 1024;
/// How many ended terminals the host remembers to answer `closed`.
const ENDED_MAX: usize = 256;

/// A right a NIP-HOST grant confers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
    /// Operate terminals: open, attach, type, resize, signal, and close.
    Terminal,
    /// Observe the host. Reads terminal output only where
    /// [`Config::observers_read`] allows it.
    Observe,
}

/// Whether a principal holds a right now. The resident host backs this
/// with its NIP-HOST grants; the host asks on every operation.
pub trait Rights: Send + Sync {
    fn holds(&self, principal: &str, right: Right) -> bool;
}

/// Why a sink did not take a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkError {
    /// The transport is backed up. The attachment keeps its place and the
    /// host tries again later.
    Full,
    /// The transport is gone. The host ends the attachment.
    Closed,
}

/// Where one attachment's frames go: a NIP-REACH direct channel, or
/// private `3188` artifacts over an admitted relay.
///
/// `deliver` runs while the host holds the terminal's state, so it must
/// return promptly and must not call back into the host. Queue the frame
/// for the transport and return [`SinkError::Full`] rather than wait.
pub trait FrameSink: Send {
    fn deliver(&mut self, frame: &Frame) -> Result<(), SinkError>;
}

/// A sink over a bounded channel, for tests and in-process transports.
#[derive(Debug)]
pub struct ChannelSink(SyncSender<Frame>);

impl FrameSink for ChannelSink {
    fn deliver(&mut self, frame: &Frame) -> Result<(), SinkError> {
        match self.0.try_send(frame.clone()) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(SinkError::Full),
            Err(TrySendError::Disconnected(_)) => Err(SinkError::Closed),
        }
    }
}

/// A channel sink holding at most `bound` frames, and its receiver.
#[must_use]
pub fn channel(bound: usize) -> (ChannelSink, Receiver<Frame>) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(bound);
    (ChannelSink(sender), receiver)
}

/// Turns a program and its arguments into the command the host spawns,
/// such as a `coder-boundary` wrapper around it. The wrapper must stay
/// valid for as long as the host runs terminals through it.
pub trait Wrap: Send + Sync {
    fn command(&self, program: &Path, args: &[OsString]) -> Result<Command, String>;
}

/// What a host allows and how much it keeps.
#[derive(Clone)]
pub struct Config {
    /// This host run's generation, a common ID. A restart takes a new one.
    pub generation: String,
    /// Admitted workspaces: common ID to root directory.
    pub workspaces: BTreeMap<String, PathBuf>,
    /// The program a `shell` launch runs, by absolute path.
    pub shell: PathBuf,
    pub shell_args: Vec<String>,
    /// Variables every terminal gets, such as `PATH`, `HOME`, and `TERM`.
    pub base_env: Vec<(String, String)>,
    /// Variable names an open request may set.
    pub env_allow: BTreeSet<String>,
    /// Whether a principal with only `observe` may attach to read output.
    pub observers_read: bool,
    /// Output bytes each terminal's ring retains.
    pub ring_bytes: usize,
    /// Frames each terminal's ring retains.
    pub ring_frames: usize,
    /// Output bytes per frame, at most [`wire::FRAME_MAX`].
    pub frame_max: usize,
    /// The most output bytes per second one attachment receives.
    pub rate_max: u64,
    /// How long a terminal lives with no attachment and no input.
    pub idle: Duration,
    /// The most terminals whose process is still running. Ended terminals
    /// stay for replay until the idle period passes and do not count.
    pub terminals_max: usize,
    /// Attachments per terminal.
    pub attachments_max: usize,
    pub wrap: Option<Arc<dyn Wrap>>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("generation", &self.generation)
            .field("workspaces", &self.workspaces)
            .field("shell", &self.shell)
            .field("env_allow", &self.env_allow)
            .field("observers_read", &self.observers_read)
            .field("ring_bytes", &self.ring_bytes)
            .field("idle", &self.idle)
            .field("wrap", &self.wrap.is_some())
            .finish_non_exhaustive()
    }
}

impl Config {
    /// Defaults with a fresh generation: `/bin/sh`, `TERM=xterm-256color`
    /// and this process's `PATH`, `HOME`, and `LANG`, a 1 MiB ring, 256 KiB
    /// per second per attachment, and 30 minutes of idle life.
    #[must_use]
    pub fn new() -> Self {
        let mut base_env = vec![("TERM".to_string(), "xterm-256color".to_string())];
        for name in ["PATH", "HOME", "LANG", "USER", "LOGNAME"] {
            if let Ok(value) = std::env::var(name) {
                base_env.push((name.to_string(), value));
            }
        }
        Config {
            generation: sys::random_id(),
            workspaces: BTreeMap::new(),
            shell: PathBuf::from("/bin/sh"),
            shell_args: Vec::new(),
            base_env,
            env_allow: BTreeSet::new(),
            observers_read: false,
            ring_bytes: 1024 * 1024,
            ring_frames: 4096,
            frame_max: wire::FRAME_MAX,
            rate_max: 256 * 1024,
            idle: Duration::from_secs(30 * 60),
            terminals_max: 16,
            attachments_max: 8,
            wrap: None,
        }
    }

    /// Admits a workspace root under a common ID.
    #[must_use]
    pub fn workspace(mut self, id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        self.workspaces.insert(id.into(), root.into());
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::new()
    }
}

/// A byte budget refilled at a fixed rate.
#[derive(Debug)]
struct Bucket {
    rate: u64,
    capacity: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    fn new(rate: u64, frame_max: usize, now: Instant) -> Self {
        // A whole frame always fits eventually, even under a small rate.
        let capacity = rate.max(frame_max as u64) as f64;
        Bucket {
            rate,
            capacity,
            tokens: capacity,
            last: now,
        }
    }

    fn take(&mut self, cost: usize, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.rate as f64).min(self.capacity);
        if self.tokens >= cost as f64 {
            self.tokens -= cost as f64;
            true
        } else {
            false
        }
    }

    fn refund(&mut self, cost: usize) {
        self.tokens = (self.tokens + cost as f64).min(self.capacity);
    }
}

struct Attachment {
    principal: String,
    mode: Mode,
    sink: Box<dyn FrameSink>,
    /// Every sequenced frame through this one was delivered or reported
    /// missing.
    sent: u64,
    bucket: Bucket,
}

struct State {
    ring: Ring,
    attachments: BTreeMap<String, Attachment>,
    size: Size,
    ended: Option<Exit>,
    /// Why the host is ending the terminal, when it is.
    closing: Option<Cause>,
    /// The last attach, detach, or input.
    activity: Instant,
}

struct Terminal {
    reference: TerminalRef,
    process: Arc<sys::Process>,
    state: Mutex<State>,
    reader: Mutex<Option<JoinHandle<()>>>,
}

impl Terminal {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn ended(&self) -> bool {
        self.state().ended.is_some()
    }

    /// Delivers what each attachment can take now.
    fn pump(&self, state: &mut State, now: Instant) {
        let State {
            ring, attachments, ..
        } = state;
        let mut finished = Vec::new();
        for (id, attachment) in attachments.iter_mut() {
            if pump_one(&self.reference, id, attachment, ring, now) {
                finished.push(id.clone());
            }
        }
        for id in finished {
            attachments.remove(&id);
        }
    }

    /// Waits for the reader to finish, which happens after the child is
    /// reaped and its exit is recorded.
    fn join(&self) {
        let handle = self
            .reader
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

/// Pumps one attachment. Returns whether it is finished: its transport
/// closed, or it received the terminal's exit.
fn pump_one(
    reference: &TerminalRef,
    id: &str,
    attachment: &mut Attachment,
    ring: &Ring,
    now: Instant,
) -> bool {
    loop {
        if let Some(missed) = ring.missed(attachment.sent) {
            let body = Body::Gap {
                from: missed.from,
                to: missed.to,
                bytes: missed.bytes,
            };
            match attachment
                .sink
                .deliver(&Frame::new(reference.clone(), id, body))
            {
                Ok(()) => attachment.sent = missed.to,
                Err(SinkError::Full) => return false,
                Err(SinkError::Closed) => return true,
            }
            continue;
        }
        let next = attachment.sent + 1;
        let Some(body) = ring.get(next) else {
            return false;
        };
        let cost = match body {
            Body::Output { data, .. } => data.len(),
            _ => 0,
        };
        if !attachment.bucket.take(cost, now) {
            return false;
        }
        match attachment
            .sink
            .deliver(&Frame::new(reference.clone(), id, body.clone()))
        {
            Ok(()) => {
                attachment.sent = next;
                if matches!(body, Body::Exit { .. }) {
                    return true;
                }
            }
            Err(SinkError::Full) => {
                attachment.bucket.refund(cost);
                return false;
            }
            Err(SinkError::Closed) => return true,
        }
    }
}

/// One applied request, kept to answer an exact retry.
struct Applied {
    request: String,
    body: String,
    value: Value,
}

struct Inner {
    config: Config,
    rights: Arc<dyn Rights>,
    terminals: Mutex<BTreeMap<String, Arc<Terminal>>>,
    ended: Mutex<VecDeque<String>>,
    recent: Mutex<VecDeque<Applied>>,
    shutting: AtomicBool,
}

/// A host's terminals.
///
/// Dropping the host shuts it down: every terminal's process group ends.
pub struct Host {
    inner: Arc<Inner>,
    ticker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("generation", &self.inner.config.generation)
            .finish_non_exhaustive()
    }
}

/// The result of an operation, as [`wire::TerminalResult::from_outcome`]
/// takes it.
pub type Outcome = Result<(Status, Value), Refusal>;

impl Host {
    /// A host with `config`, asking `rights` for every operation. It starts
    /// a background ticker that pumps rate-limited attachments and expires
    /// idle terminals.
    #[must_use]
    pub fn new(config: Config, rights: Arc<dyn Rights>) -> Self {
        let inner = Arc::new(Inner {
            config,
            rights,
            terminals: Mutex::new(BTreeMap::new()),
            ended: Mutex::new(VecDeque::new()),
            recent: Mutex::new(VecDeque::new()),
            shutting: AtomicBool::new(false),
        });
        let weak = Arc::downgrade(&inner);
        let ticker = std::thread::Builder::new()
            .name("coder-pty-ticker".into())
            .spawn(move || ticker(&weak))
            .ok();
        Host { inner, ticker }
    }

    /// This host run's generation.
    #[must_use]
    pub fn generation(&self) -> &str {
        &self.inner.config.generation
    }

    /// Opens a terminal.
    pub fn open(&self, principal: &str, request: &Open) -> Outcome {
        request.check()?;
        self.inner.require(principal, Right::Terminal)?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let value = self.inner.open(request)?;
        self.inner
            .remember(&key(principal, &request.request), body, value.clone());
        Ok((Status::Accepted, value))
    }

    /// Attaches `sink` to a terminal and replays what it retains after
    /// `request.after`, reporting a gap for anything it discarded.
    pub fn attach(&self, principal: &str, request: &Attach, sink: Box<dyn FrameSink>) -> Outcome {
        request.check()?;
        let admitted = match request.mode {
            Mode::Interact => self.inner.rights.holds(principal, Right::Terminal),
            Mode::Observe => self.inner.may_read(principal),
        };
        if !admitted {
            return Err(Refusal::new(
                Reason::NotAdmitted,
                "this device may not attach to terminals",
            ));
        }
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.find(&request.terminal)?;
        let now = Instant::now();
        let mut state = terminal.state();
        if request.after > state.ring.head() {
            return Err(Refusal::new(
                Reason::Malformed,
                "after names a frame the terminal has not produced",
            ));
        }
        if state.attachments.len() >= self.inner.config.attachments_max {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the terminal has its most attachments",
            ));
        }
        let id = sys::random_id();
        let rate = request.rate.min(self.inner.config.rate_max);
        state.attachments.insert(
            id.clone(),
            Attachment {
                principal: principal.to_string(),
                mode: request.mode,
                sink,
                sent: request.after,
                bucket: Bucket::new(rate, self.inner.config.frame_max, now),
            },
        );
        state.activity = now;
        let value = Value::Attached {
            attachment: id,
            head: state.ring.head(),
            size: state.size,
            running: state.ended.is_none(),
        };
        terminal.pump(&mut state, now);
        drop(state);
        self.inner
            .remember(&key(principal, &request.request), body, value.clone());
        Ok((Status::Accepted, value))
    }

    /// Ends one attachment. The terminal keeps running.
    pub fn detach(&self, principal: &str, request: &Detach) -> Outcome {
        request.check()?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.find(&request.terminal)?;
        let mut state = terminal.state();
        let owned = state
            .attachments
            .get(&request.attachment)
            .is_some_and(|attachment| attachment.principal == principal);
        if !owned {
            return Err(Refusal::new(
                Reason::Unavailable,
                "no such attachment for this device",
            ));
        }
        if let Some(mut attachment) = state.attachments.remove(&request.attachment) {
            let body = Body::Detached {
                reason: Detached::Requested,
            };
            let _ = attachment.sink.deliver(&Frame::new(
                request.terminal.clone(),
                &request.attachment,
                body,
            ));
        }
        state.activity = Instant::now();
        drop(state);
        self.inner
            .remember(&key(principal, &request.request), body, Value::Done);
        Ok((Status::Accepted, Value::Done))
    }

    /// Types into a terminal. Returns how many bytes the terminal took;
    /// fewer than sent means it stopped reading input for a second.
    pub fn input(&self, principal: &str, request: &Input) -> Outcome {
        request.check()?;
        self.inner.require(principal, Right::Terminal)?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.running(&request.terminal)?;
        let written = terminal.process.write(&request.data).map_err(|error| {
            Refusal::new(
                Reason::Unavailable,
                format!("the terminal refused input: {error}"),
            )
        })?;
        if written == 0 {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the terminal is not reading input",
            ));
        }
        terminal.state().activity = Instant::now();
        let value = Value::Written {
            bytes: written as u64,
        };
        self.inner
            .remember(&key(principal, &request.request), body, value.clone());
        Ok((Status::Accepted, value))
    }

    /// Changes a terminal's size.
    pub fn resize(&self, principal: &str, request: &Resize) -> Outcome {
        request.check()?;
        self.inner.require(principal, Right::Terminal)?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.running(&request.terminal)?;
        terminal.process.resize(request.size).map_err(|error| {
            Refusal::new(
                Reason::Unavailable,
                format!("the terminal refused the size: {error}"),
            )
        })?;
        terminal.state().size = request.size;
        self.inner
            .remember(&key(principal, &request.request), body, Value::Done);
        Ok((Status::Accepted, Value::Done))
    }

    /// Signals a terminal's foreground process group.
    pub fn signal(&self, principal: &str, request: &Signal) -> Outcome {
        request.check()?;
        self.inner.require(principal, Right::Terminal)?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.running(&request.terminal)?;
        terminal
            .process
            .signal_foreground(request.signal)
            .map_err(|error| {
                Refusal::new(
                    Reason::Unavailable,
                    format!("the signal was not delivered: {error}"),
                )
            })?;
        self.inner
            .remember(&key(principal, &request.request), body, Value::Done);
        Ok((Status::Accepted, Value::Done))
    }

    /// Ends a terminal and its process group, and waits until the child is
    /// reaped and the exit is recorded.
    pub fn close(&self, principal: &str, request: &Close) -> Outcome {
        request.check()?;
        self.inner.require(principal, Right::Terminal)?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.find(&request.terminal)?;
        end(&[terminal], Cause::Closed);
        self.inner
            .remember(&key(principal, &request.request), body, Value::Done);
        Ok((Status::Accepted, Value::Done))
    }

    /// Pumps rate-limited attachments, ends attachments whose right was
    /// revoked, and expires terminals idle as of `now`. The background
    /// ticker calls this with the current time; a test can pass a later one.
    pub fn tick(&self, now: Instant) {
        self.inner.tick(now);
    }

    /// Ends every terminal's process group, delivers each exit it can, and
    /// refuses further opens. Returns once every child is reaped.
    pub fn shutdown(&self) {
        self.inner.shutting.store(true, Ordering::SeqCst);
        let terminals: Vec<Arc<Terminal>> = {
            let mut map = self.inner.terminals();
            std::mem::take(&mut *map).into_values().collect()
        };
        end(&terminals, Cause::HostShutdown);
    }

    /// The process group a terminal runs in. A host-local diagnostic for
    /// evidence and tests; it never appears on the wire.
    #[must_use]
    pub fn process_group(&self, terminal: &TerminalRef) -> Option<i32> {
        let terminal = self.inner.find(terminal).ok()?;
        Some(terminal.process.group())
    }

    /// A terminal's newest sequence number and whether its process is
    /// still running. A host-local diagnostic; clients learn both from
    /// attach.
    pub fn head(&self, terminal: &TerminalRef) -> Result<(u64, bool), Refusal> {
        let terminal = self.inner.find(terminal)?;
        let state = terminal.state();
        Ok((state.ring.head(), state.ended.is_none()))
    }

    /// How many terminals the host holds, running or ended.
    #[must_use]
    pub fn terminals(&self) -> usize {
        self.inner.terminals().len()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(ticker) = self.ticker.take() {
            let _ = ticker.join();
        }
    }
}

impl Inner {
    fn terminals(&self) -> MutexGuard<'_, BTreeMap<String, Arc<Terminal>>> {
        self.terminals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn require(&self, principal: &str, right: Right) -> Result<(), Refusal> {
        if self.rights.holds(principal, right) {
            Ok(())
        } else {
            Err(Refusal::new(
                Reason::NotAdmitted,
                "this device lacks the terminal right",
            ))
        }
    }

    fn may_read(&self, principal: &str) -> bool {
        self.rights.holds(principal, Right::Terminal)
            || (self.config.observers_read && self.rights.holds(principal, Right::Observe))
    }

    /// The answer to an exact retry, a conflict for a reused request ID,
    /// or `None` for a new request.
    fn retry(&self, request: &str, body: &str) -> Option<Outcome> {
        let recent = self.recent.lock().unwrap_or_else(PoisonError::into_inner);
        let applied = recent.iter().find(|applied| applied.request == request)?;
        Some(if applied.body == body {
            Ok((Status::Duplicate, applied.value.clone()))
        } else {
            Err(Refusal::new(
                Reason::IdempotencyConflict,
                "this request ID was used with different content",
            ))
        })
    }

    fn remember(&self, request: &str, body: String, value: Value) {
        let mut recent = self.recent.lock().unwrap_or_else(PoisonError::into_inner);
        recent.push_back(Applied {
            request: request.to_string(),
            body,
            value,
        });
        if recent.len() > RECENT_MAX {
            recent.pop_front();
        }
    }

    fn find(&self, reference: &TerminalRef) -> Result<Arc<Terminal>, Refusal> {
        if reference.generation != self.config.generation {
            return Err(Refusal::new(
                Reason::Lost,
                "the host restarted; the terminal did not survive it",
            ));
        }
        if let Some(terminal) = self.terminals().get(&reference.terminal) {
            return Ok(terminal.clone());
        }
        let ended = self.ended.lock().unwrap_or_else(PoisonError::into_inner);
        if ended.contains(&reference.terminal) {
            Err(Refusal::new(
                Reason::Closed,
                "the terminal ended and is no longer retained",
            ))
        } else {
            Err(Refusal::new(Reason::Unavailable, "no such terminal"))
        }
    }

    /// A terminal whose process is still running.
    fn running(&self, reference: &TerminalRef) -> Result<Arc<Terminal>, Refusal> {
        let terminal = self.find(reference)?;
        if terminal.ended() {
            return Err(Refusal::new(
                Reason::Closed,
                "the terminal's process has ended",
            ));
        }
        Ok(terminal)
    }

    fn open(self: &Arc<Self>, request: &Open) -> Result<Value, Refusal> {
        if self.shutting.load(Ordering::SeqCst) {
            return Err(Refusal::new(
                Reason::Unavailable,
                "the host is shutting down",
            ));
        }
        let running = self
            .terminals()
            .values()
            .filter(|terminal| !terminal.ended())
            .count();
        if running >= self.config.terminals_max {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the host has its most terminals",
            ));
        }
        let dir = self.directory(&request.workspace, &request.dir)?;
        for var in &request.env {
            if !self.config.env_allow.contains(&var.name) {
                return Err(Refusal::new(
                    Reason::NotAdmitted,
                    format!("the host does not admit the variable {}", var.name),
                ));
            }
        }
        let (program, args): (PathBuf, Vec<OsString>) = match &request.launch {
            Launch::Shell => (
                self.config.shell.clone(),
                self.config.shell_args.iter().map(OsString::from).collect(),
            ),
            Launch::Command { program, args } => (
                PathBuf::from(program),
                args.iter().map(OsString::from).collect(),
            ),
        };
        let mut command = match &self.config.wrap {
            Some(wrap) => wrap.command(&program, &args).map_err(|why| {
                Refusal::new(
                    Reason::Unavailable,
                    format!("the command could not be wrapped: {why}"),
                )
            })?,
            None => {
                let mut command = Command::new(&program);
                command.args(&args);
                command
            }
        };
        command.current_dir(&dir).env_clear();
        for (name, value) in &self.config.base_env {
            command.env(name, value);
        }
        for var in &request.env {
            command.env(&var.name, &var.value);
        }
        let process = sys::spawn(command, request.size).map_err(|error| {
            Refusal::new(
                Reason::Unavailable,
                format!("the terminal did not start: {error}"),
            )
        })?;
        let reference = TerminalRef {
            generation: self.config.generation.clone(),
            terminal: sys::random_id(),
        };
        let terminal = Arc::new(Terminal {
            reference: reference.clone(),
            process: Arc::new(process),
            state: Mutex::new(State {
                ring: Ring::new(
                    self.config.ring_bytes.max(self.config.frame_max),
                    self.config.ring_frames,
                ),
                attachments: BTreeMap::new(),
                size: request.size,
                ended: None,
                closing: None,
                activity: Instant::now(),
            }),
            reader: Mutex::new(None),
        });
        let frame_max = self.config.frame_max.clamp(1, wire::FRAME_MAX);
        let reading = terminal.clone();
        let handle = std::thread::Builder::new()
            .name("coder-pty-reader".into())
            .spawn(move || read(&reading, frame_max));
        match handle {
            Ok(handle) => {
                *terminal
                    .reader
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(handle)
            }
            Err(error) => {
                end(&[terminal], Cause::Closed);
                return Err(Refusal::new(
                    Reason::Unavailable,
                    format!("no reader thread: {error}"),
                ));
            }
        }
        self.terminals()
            .insert(reference.terminal.clone(), terminal);
        Ok(Value::Opened {
            terminal: reference,
            size: request.size,
        })
    }

    /// Resolves a working directory inside an admitted workspace root,
    /// following symbolic links before checking containment.
    fn directory(&self, workspace: &str, dir: &str) -> Result<PathBuf, Refusal> {
        let root = self.config.workspaces.get(workspace).ok_or_else(|| {
            Refusal::new(
                Reason::NotAdmitted,
                "the workspace is not admitted on this host",
            )
        })?;
        let root = root.canonicalize().map_err(|_| {
            Refusal::new(Reason::Unavailable, "the workspace root is not available")
        })?;
        let resolved = root.join(dir).canonicalize().map_err(|_| {
            Refusal::new(Reason::Unavailable, "the working directory does not exist")
        })?;
        if !resolved.starts_with(&root) {
            return Err(Refusal::new(
                Reason::NotAdmitted,
                "the working directory leaves the workspace",
            ));
        }
        if !resolved.is_dir() {
            return Err(Refusal::new(
                Reason::Unavailable,
                "the working directory is not a directory",
            ));
        }
        Ok(resolved)
    }

    fn tick(&self, now: Instant) {
        let terminals: Vec<Arc<Terminal>> = self.terminals().values().cloned().collect();
        let mut expire = Vec::new();
        let mut remove = Vec::new();
        for terminal in terminals {
            let mut state = terminal.state();
            let revoked: Vec<String> = state
                .attachments
                .iter()
                .filter(|(_, attachment)| match attachment.mode {
                    Mode::Interact => !self.rights.holds(&attachment.principal, Right::Terminal),
                    Mode::Observe => !self.may_read(&attachment.principal),
                })
                .map(|(id, _)| id.clone())
                .collect();
            for id in revoked {
                if let Some(mut attachment) = state.attachments.remove(&id) {
                    let body = Body::Detached {
                        reason: Detached::Revoked,
                    };
                    let _ =
                        attachment
                            .sink
                            .deliver(&Frame::new(terminal.reference.clone(), &id, body));
                }
            }
            terminal.pump(&mut state, now);
            let idle = state.attachments.is_empty()
                && now.saturating_duration_since(state.activity) >= self.config.idle;
            if idle && state.ended.is_some() {
                remove.push(terminal.reference.terminal.clone());
            } else if idle && state.closing.is_none() {
                drop(state);
                expire.push(terminal);
            }
        }
        if !expire.is_empty() {
            end(&expire, Cause::IdleExpired);
            remove.extend(
                expire
                    .iter()
                    .map(|terminal| terminal.reference.terminal.clone()),
            );
        }
        if !remove.is_empty() {
            let mut map = self.terminals();
            let mut ended = self.ended.lock().unwrap_or_else(PoisonError::into_inner);
            for id in remove {
                map.remove(&id);
                ended.push_back(id);
                if ended.len() > ENDED_MAX {
                    ended.pop_front();
                }
            }
        }
    }
}

/// Ends terminals together: every group gets its hang-up at once, then
/// one grace period, then `SIGKILL`, then each reader records the exit.
fn end(terminals: &[Arc<Terminal>], cause: Cause) {
    let mut live = Vec::new();
    for terminal in terminals {
        let mut state = terminal.state();
        if state.ended.is_none() {
            state.closing.get_or_insert(cause);
            drop(state);
            terminal.process.hang_up();
            live.push(terminal.clone());
        }
    }
    let deadline = Instant::now() + GRACE;
    while Instant::now() < deadline && live.iter().any(|terminal| !terminal.ended()) {
        std::thread::sleep(POLL);
    }
    for terminal in &live {
        if !terminal.ended() {
            terminal.process.kill();
        }
    }
    for terminal in terminals {
        terminal.join();
    }
    // A killed descendant stays in the group as a zombie until its new
    // parent, init or a subreaper, reaps it. On Linux that happens after
    // the direct child is reaped, so emptiness is waited for, within the
    // grace period, rather than read once.
    let settle = Instant::now() + GRACE;
    for terminal in &live {
        while terminal.process.group_running() && Instant::now() < settle {
            std::thread::sleep(SETTLE_POLL);
        }
    }
}

/// A terminal's reader: output into the ring, the child's exit after the
/// output it wrote, then done.
fn read(terminal: &Terminal, frame_max: usize) {
    let mut buffer = vec![0u8; frame_max];
    let mut exited: Option<(sys::Status, Instant)> = None;
    let mut eof = false;
    loop {
        if !eof {
            match terminal.process.read(&mut buffer, POLL) {
                sys::Read::Data(n) => {
                    let now = Instant::now();
                    let mut state = terminal.state();
                    state.ring.push_output(buffer[..n].to_vec());
                    terminal.pump(&mut state, now);
                }
                sys::Read::Timeout => {}
                sys::Read::Eof => eof = true,
            }
        } else {
            std::thread::sleep(POLL);
        }
        if exited.is_none()
            && let Some(status) = terminal.process.try_wait()
        {
            // A descendant still in the group outlived the terminal's
            // process; end it, as the supervisor does for a job.
            terminal.process.kill();
            exited = Some((status, Instant::now()));
        }
        if let Some((status, at)) = exited
            && (eof || at.elapsed() >= GRACE)
        {
            let now = Instant::now();
            let mut state = terminal.state();
            let exit = Exit {
                cause: state.closing.unwrap_or(Cause::Exited),
                code: status.code,
                signal: status.signal,
            };
            state.ring.push_exit(exit);
            state.ended = Some(exit);
            state.activity = now;
            terminal.pump(&mut state, now);
            return;
        }
    }
}

fn ticker(inner: &Weak<Inner>) {
    loop {
        std::thread::sleep(TICK);
        let Some(inner) = inner.upgrade() else {
            return;
        };
        if inner.shutting.load(Ordering::SeqCst) {
            return;
        }
        inner.tick(Instant::now());
    }
}

/// Where the host remembers a request: its ID under the device that sent
/// it, so one device's request ID never answers for another's.
fn key(principal: &str, request: &str) -> String {
    format!("{principal} {request}")
}

/// The exact request content an idempotent retry must repeat.
fn identity<T: serde::Serialize>(principal: &str, request: &T) -> String {
    format!(
        "{principal} {}",
        serde_json::to_string(request).unwrap_or_default()
    )
}
