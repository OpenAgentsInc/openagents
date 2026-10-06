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
//! # Side effects
//!
//! With [`Config::emulator`], each terminal runs one authoritative
//! emulator ([`crate::emulator`]) that parses every output byte once. The
//! host serves the effects feature: an attachment that names it receives
//! bells, title and directory changes, and clipboard writes as effect
//! frames, live and never replayed, and a clipboard write reaches only the
//! interacting attachments of the principal who typed last. The host
//! writes the program's query replies to the terminal itself whenever no
//! interacting attachment predates the feature; an older client that
//! answers queries keeps answering them, so a reply is never sent twice.
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
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::emulator::{self, Effects, Emulator, HistoryRead};
use crate::ext::{BlockPageRead, Effect, Features, History, Join, RecordsFrame};
use crate::ring::Ring;
use crate::wire::{
    self, Attach, Body, Cause, Close, Detach, Detached, Exit, Frame, Input, Launch, Mode, Open,
    Reason, Refusal, Resize, Signal, Size, Status, TerminalRef, Value,
};

mod cmdline;
#[cfg(unix)]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;
#[cfg(not(any(unix, windows)))]
#[path = "unsupported.rs"]
mod sys;

/// How long a terminal's group has to exit after a hang-up before it is
/// killed, and how long the reader drains output after the child exits.
#[cfg(unix)]
pub const GRACE: Duration = supervise::GRACE;
/// How long a terminal's tree has to exit after a hang-up before it is
/// killed, and how long the reader drains output after the child exits:
/// `supervise::GRACE`, which this build does not link.
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

    /// Whether this sink carries record streams, which an attach by
    /// snapshot and a history read need.
    fn carries_records(&self) -> bool {
        false
    }

    /// Delivers one part of a record stream, in the same order as frames.
    fn deliver_records(&mut self, frame: &RecordsFrame) -> Result<(), SinkError> {
        let _ = frame;
        Err(SinkError::Closed)
    }
}

/// What a [`DeliverySink`] delivers: a frame or a part of a record stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delivery {
    Frame(Frame),
    Records(RecordsFrame),
}

/// A sink over a bounded channel that also carries record streams, in
/// order with frames.
#[derive(Debug)]
pub struct DeliverySink(SyncSender<Delivery>);

impl DeliverySink {
    fn send(&self, delivery: Delivery) -> Result<(), SinkError> {
        match self.0.try_send(delivery) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(SinkError::Full),
            Err(TrySendError::Disconnected(_)) => Err(SinkError::Closed),
        }
    }
}

impl FrameSink for DeliverySink {
    fn deliver(&mut self, frame: &Frame) -> Result<(), SinkError> {
        self.send(Delivery::Frame(frame.clone()))
    }

    fn carries_records(&self) -> bool {
        true
    }

    fn deliver_records(&mut self, frame: &RecordsFrame) -> Result<(), SinkError> {
        self.send(Delivery::Records(frame.clone()))
    }
}

/// A delivery sink holding at most `bound` deliveries, and its receiver.
#[must_use]
pub fn deliveries(bound: usize) -> (DeliverySink, Receiver<Delivery>) {
    let (sender, receiver) = std::sync::mpsc::sync_channel(bound);
    (DeliverySink(sender), receiver)
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
    /// Makes each terminal's authoritative emulator. When set, the host
    /// owns query replies and serves the effects feature.
    pub emulator: Option<emulator::Factory>,
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
            .field("emulator", &self.emulator.is_some())
            .finish_non_exhaustive()
    }
}

impl Config {
    /// Defaults with a fresh generation: `/bin/sh`, `TERM=xterm-256color`
    /// and this process's `PATH`, `HOME`, and `LANG`, a 1 MiB ring, 256 KiB
    /// per second per attachment, and 30 minutes of idle life. On Windows
    /// the shell is `%ComSpec%` (`cmd.exe`), and the variables are the ones
    /// a Windows program expects to find ([`WINDOWS_ENV`]).
    #[must_use]
    pub fn new() -> Self {
        let mut base_env = vec![("TERM".to_string(), "xterm-256color".to_string())];
        let inherited: &[&str] = if cfg!(windows) {
            &WINDOWS_ENV
        } else {
            &["PATH", "HOME", "LANG", "USER", "LOGNAME"]
        };
        for name in inherited {
            if let Ok(value) = std::env::var(name) {
                base_env.push(((*name).to_string(), value));
            }
        }
        Config {
            generation: sys::random_id(),
            workspaces: BTreeMap::new(),
            shell: default_shell(),
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
            emulator: None,
        }
    }

    /// Admits a workspace root under a common ID.
    #[must_use]
    pub fn workspace(mut self, id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        self.workspaces.insert(id.into(), root.into());
        self
    }

    /// The extension features a host with this configuration serves.
    #[must_use]
    pub fn features(&self) -> Features {
        Features {
            effects: self.emulator.is_some(),
            snapshot: self
                .emulator
                .as_ref()
                .is_some_and(|emulators| emulators.snapshots()),
            blocks: self
                .emulator
                .as_ref()
                .is_some_and(|emulators| emulators.blocks()),
            ..Features::NONE
        }
    }
}

/// The variables a terminal on Windows gets from the host's own
/// environment: without `SystemRoot` many programs fail to start, and
/// without `PATH` and `PATHEXT` a shell finds nothing.
pub const WINDOWS_ENV: [&str; 16] = [
    "SystemRoot",
    "SystemDrive",
    "windir",
    "ComSpec",
    "PATH",
    "PATHEXT",
    "USERPROFILE",
    "USERNAME",
    "USERDOMAIN",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "TEMP",
    "TMP",
];

/// The shell a `shell` launch runs: `/bin/sh`, or on Windows `%ComSpec%`,
/// falling back to `cmd.exe` under `%SystemRoot%`.
fn default_shell() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| {
                let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
                PathBuf::from(root).join("System32").join("cmd.exe")
            })
    } else {
        PathBuf::from("/bin/sh")
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
    /// Whether the attachment named the effects feature.
    effects: bool,
    /// Whether it joined by snapshot. Such an attachment that falls behind
    /// the ring receives a fresh snapshot instead of a gap.
    joined: bool,
    /// Parts of record streams not yet delivered. Every part goes before
    /// the next sequenced frame, so a snapshot's `READY` always precedes
    /// the frames after its `through`.
    parts: VecDeque<RecordsFrame>,
    /// Effects waiting for the output they follow, at most one of each
    /// kind: a later title replaces an earlier one, and bells add up.
    pending: VecDeque<(u64, Effect)>,
}

impl Attachment {
    fn queue(&mut self, after: u64, effect: Effect) {
        let same =
            |queued: &Effect| std::mem::discriminant(queued) == std::mem::discriminant(&effect);
        if let Some(index) = self.pending.iter().position(|(_, queued)| same(queued)) {
            let (_, old) = self.pending.remove(index).expect("found");
            let effect = match (old, effect) {
                (Effect::Bell { count: old }, Effect::Bell { count }) => Effect::Bell {
                    count: old.saturating_add(count),
                },
                (_, effect) => effect,
            };
            self.pending.push_back((after, effect));
        } else {
            self.pending.push_back((after, effect));
        }
    }
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
    /// The authoritative emulator, when the host runs one.
    emulator: Option<Box<dyn Emulator>>,
    /// The title and directory the emulator last reported.
    title: String,
    directory: Option<String>,
    /// The principal that typed last, whose interacting attachments
    /// receive clipboard writes.
    typist: Option<String>,
}

impl State {
    /// Whether the host answers the program's queries: it runs an emulator
    /// and no interacting attachment predates the effects feature.
    fn answers(&self) -> bool {
        self.emulator.is_some()
            && self
                .attachments
                .values()
                .all(|attachment| attachment.mode != Mode::Interact || attachment.effects)
    }

    /// Hands the effects of output through `seq` to the attachments that
    /// take them, and answers the replies the host writes.
    fn effects(&mut self, seq: u64, effects: Effects) -> Vec<u8> {
        let mut out = Vec::new();
        if effects.bells > 0 {
            out.push(Effect::Bell {
                count: effects.bells,
            });
        }
        if let Some(title) = effects.title {
            self.title.clone_from(&title);
            out.push(Effect::Title { title });
        }
        if let Some(dir) = effects.directory {
            self.directory = Some(dir.clone());
            out.push(Effect::Directory { dir });
        }
        let clipboard = effects
            .clipboard
            .map(|text| Effect::Clipboard { text })
            .filter(|effect| effect.check().is_ok());
        for attachment in self.attachments.values_mut().filter(|a| a.effects) {
            for effect in out.iter().filter(|effect| effect.check().is_ok()) {
                attachment.queue(seq, effect.clone());
            }
            if let Some(clipboard) = &clipboard
                && attachment.mode == Mode::Interact
                && self.typist.as_deref() == Some(attachment.principal.as_str())
            {
                attachment.queue(seq, clipboard.clone());
            }
        }
        if self.answers() {
            effects.replies
        } else {
            Vec::new()
        }
    }
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
            ring,
            attachments,
            emulator,
            ended,
            ..
        } = state;
        let mut finished = Vec::new();
        for (id, attachment) in attachments.iter_mut() {
            let mut source = Source {
                reference: &self.reference,
                ring,
                emulator: emulator.as_deref_mut(),
                exit: *ended,
            };
            if pump_one(&mut source, id, attachment, now) {
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

/// What a terminal's attachments are pumped from.
struct Source<'a> {
    reference: &'a TerminalRef,
    ring: &'a Ring,
    emulator: Option<&'a mut (dyn Emulator + 'static)>,
    /// The process's exit, once the ring holds it.
    exit: Option<Exit>,
}

impl Source<'_> {
    /// A snapshot stream of the terminal now, through the ring's head, cut
    /// into parts for attachment `id`.
    fn snapshot(&mut self, id: &str) -> Option<Result<Vec<RecordsFrame>, Refusal>> {
        let through = self.ring.head();
        let records = self
            .emulator
            .as_mut()?
            .snapshot(self.reference, through, self.exit)?;
        Some(records.map(|records| parts(self.reference, id, &records)))
    }
}

/// A record stream cut into parts for one attachment, under a new stream
/// ID.
fn parts(reference: &TerminalRef, id: &str, records: &[crate::ext::Record]) -> Vec<RecordsFrame> {
    let bytes = crate::ext::encode_stream(records);
    crate::ext::frames(
        reference,
        id,
        &sys::random_id(),
        &bytes,
        crate::ext::PART_MAX,
    )
}

/// Pumps one attachment. Returns whether it is finished: its transport
/// closed, or it received the terminal's exit.
fn pump_one(source: &mut Source<'_>, id: &str, attachment: &mut Attachment, now: Instant) -> bool {
    let reference = source.reference;
    let ring = source.ring;
    loop {
        if let Some(index) = attachment
            .pending
            .iter()
            .position(|(after, _)| *after <= attachment.sent)
        {
            let (after, effect) = attachment.pending[index].clone();
            let body = Body::Effect { after, effect };
            match attachment
                .sink
                .deliver(&Frame::new(reference.clone(), id, body))
            {
                Ok(()) => {
                    attachment.pending.remove(index);
                }
                Err(SinkError::Full) => return false,
                Err(SinkError::Closed) => return true,
            }
            continue;
        }
        if let Some(part) = attachment.parts.front() {
            let cost = part.data.len();
            if !attachment.bucket.take(cost, now) {
                return false;
            }
            match attachment.sink.deliver_records(part) {
                Ok(()) => {
                    attachment.parts.pop_front();
                }
                Err(SinkError::Full) => {
                    attachment.bucket.refund(cost);
                    return false;
                }
                Err(SinkError::Closed) => return true,
            }
            continue;
        }
        // A snapshot of an ended terminal carried its exit: nothing follows.
        if source.exit.is_some() && attachment.sent >= ring.head() && attachment.pending.is_empty()
        {
            return true;
        }
        if attachment.joined
            && ring.missed(attachment.sent).is_some()
            && let Some(Ok(parts)) = source.snapshot(id)
        {
            // Behind the ring: a fresh snapshot replaces what was lost.
            attachment.parts.extend(parts);
            attachment.sent = ring.head();
            continue;
        }
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

    /// The extension features this host serves.
    #[must_use]
    pub fn features(&self) -> Features {
        self.inner.config.features()
    }

    /// Attaches `sink` to a terminal and replays what it retains after
    /// `request.after`, reporting a gap for anything it discarded; or, by
    /// snapshot, sends a snapshot of the terminal's parsed state and then
    /// the frames after it. An attachment that names the effects feature
    /// first receives the terminal's current title and directory.
    pub fn attach(&self, principal: &str, request: &Attach, sink: Box<dyn FrameSink>) -> Outcome {
        request.check_with(self.features())?;
        let joined = request.join == Some(Join::Snapshot);
        if joined && !sink.carries_records() {
            return Err(Refusal::new(
                Reason::UnsupportedFeature,
                "this transport carries no record streams",
            ));
        }
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
        let mut attachment = Attachment {
            principal: principal.to_string(),
            mode: request.mode,
            sink,
            sent: request.after,
            bucket: Bucket::new(rate, self.inner.config.frame_max, now),
            effects: request.effects(),
            joined,
            parts: VecDeque::new(),
            pending: VecDeque::new(),
        };
        if joined {
            let State {
                ring,
                emulator,
                ended,
                ..
            } = &mut *state;
            let head = ring.head();
            let mut source = Source {
                reference: &terminal.reference,
                ring,
                emulator: emulator.as_deref_mut(),
                exit: *ended,
            };
            match source.snapshot(&id) {
                Some(Ok(parts)) => {
                    attachment.parts.extend(parts);
                    attachment.sent = head;
                }
                Some(Err(refusal)) => return Err(refusal),
                None => {
                    return Err(Refusal::new(
                        Reason::UnsupportedFeature,
                        "this host writes no snapshots",
                    ));
                }
            }
        }
        if attachment.effects {
            if !state.title.is_empty() {
                attachment.queue(
                    0,
                    Effect::Title {
                        title: state.title.clone(),
                    },
                );
            }
            if let Some(dir) = &state.directory {
                attachment.queue(0, Effect::Directory { dir: dir.clone() });
            }
        }
        state.attachments.insert(id.clone(), attachment);
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

    /// Reads a page of a terminal's block journal. It needs the
    /// `terminal` right, or `observe` under the observer policy. A block's
    /// `retained` says whether the replay buffer still holds all its
    /// output.
    pub fn block_page(&self, principal: &str, request: &BlockPageRead) -> Outcome {
        request.check_with(self.features())?;
        if !self.inner.may_read(principal) {
            return Err(Refusal::new(
                Reason::NotAdmitted,
                "this device may not read terminals",
            ));
        }
        let terminal = self.inner.find(&request.terminal)?;
        let state = terminal.state();
        let mut page = match state
            .emulator
            .as_ref()
            .and_then(|emulator| emulator.blocks(request.before, request.limit))
        {
            Some(page) => page?,
            None => {
                return Err(Refusal::new(
                    Reason::UnsupportedFeature,
                    "this host keeps no block journal",
                ));
            }
        };
        let first = state.ring.first();
        let head = state.ring.head();
        for block in &mut page.blocks {
            block.retained = block.output.is_some_and(|output| {
                first.is_some_and(|first| first <= output.from) && output.to <= head
            });
        }
        drop(state);
        Ok((Status::Accepted, Value::Blocks { page }))
    }

    /// Reads older history rows into a record stream on the principal's
    /// own attachment, and answers the stream's ID.
    pub fn history(&self, principal: &str, request: &History) -> Outcome {
        request.check_with(self.features())?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        let terminal = self.inner.find(&request.terminal)?;
        let now = Instant::now();
        let mut state = terminal.state();
        let mode = match state.attachments.get(&request.attachment) {
            Some(attachment) if attachment.principal == principal => attachment.mode,
            _ => {
                return Err(Refusal::new(
                    Reason::NotAdmitted,
                    "the attachment is not this device's",
                ));
            }
        };
        let admitted = match mode {
            Mode::Interact => self.inner.rights.holds(principal, Right::Terminal),
            Mode::Observe => self.inner.may_read(principal),
        };
        let carries = state
            .attachments
            .get(&request.attachment)
            .is_some_and(|attachment| attachment.sink.carries_records());
        if !admitted {
            return Err(Refusal::new(
                Reason::NotAdmitted,
                "this device may not read this terminal",
            ));
        }
        if !carries {
            return Err(Refusal::new(
                Reason::UnsupportedFeature,
                "this transport carries no record streams",
            ));
        }
        let read = HistoryRead {
            through: state.ring.head(),
            exit: state.ended,
            epoch: request.epoch,
            before: request.before,
            rows: request.rows,
        };
        let records = match state
            .emulator
            .as_ref()
            .and_then(|emulator| emulator.history(&terminal.reference, &read))
        {
            Some(records) => records?,
            None => {
                return Err(Refusal::new(
                    Reason::UnsupportedFeature,
                    "this host writes no history streams",
                ));
            }
        };
        let parts = parts(&terminal.reference, &request.attachment, &records);
        let stream = parts
            .first()
            .map(|part| part.stream.clone())
            .unwrap_or_default();
        if let Some(attachment) = state.attachments.get_mut(&request.attachment) {
            attachment.parts.extend(parts);
        }
        terminal.pump(&mut state, now);
        drop(state);
        let value = Value::Stream { stream };
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
        let mut state = terminal.state();
        state.activity = Instant::now();
        state.typist = Some(principal.to_string());
        drop(state);
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
        let mut state = terminal.state();
        state.size = request.size;
        if let Some(emulator) = state.emulator.as_mut() {
            emulator.resize(request.size);
        }
        drop(state);
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
            Launch::Command { program, args } => {
                // The wire admits either form of absolute path; this host
                // runs only its own, so a name is never searched for.
                if !Path::new(program).is_absolute() {
                    return Err(Refusal::malformed(
                        "a command's program must be an absolute path on this host",
                    ));
                }
                (
                    PathBuf::from(program),
                    args.iter().map(OsString::from).collect(),
                )
            }
        };
        let mut command = match &self.config.wrap {
            Some(wrap) => wrap.command(&program, &args).map_err(|why| {
                Refusal::new(
                    Reason::Unavailable,
                    format!("the command could not be wrapped: {why}"),
                )
            })?,
            // On macOS the terminal stays out of the places the system
            // guards with a privacy prompt, which nobody at the Mac would
            // be there to answer (`coder_boundary::privacy`).
            None => {
                let mut command = coder_boundary::privacy::command(&program, &[dir.as_path()]);
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
                emulator: self
                    .config
                    .emulator
                    .as_ref()
                    .map(|emulators| emulators.make(request.size)),
                title: String::new(),
                directory: None,
                typist: None,
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
        // The wire refuses `/` and `..`; on Windows `\\`, a drive, and a
        // `..` between backslashes are this host's own ways to leave.
        if Path::new(dir)
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(Refusal::malformed("dir must not leave the workspace root"));
        }
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
                    let seq = state.ring.push_output(buffer[..n].to_vec());
                    let effects = state
                        .emulator
                        .as_mut()
                        .map(|emulator| emulator.output(&buffer[..n], seq));
                    let replies = match effects {
                        Some(effects) => state.effects(seq, effects),
                        None => Vec::new(),
                    };
                    terminal.pump(&mut state, now);
                    drop(state);
                    // The program waits for these; the terminal reads its
                    // input promptly, and what it does not take is dropped
                    // as a terminal would drop it.
                    if !replies.is_empty() {
                        let _ = terminal.process.write(&replies);
                    }
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
