//! One terminal on a linked host, driven over the host's current link.
//!
//! [`Session::start`] spawns a task that asks the host to open a shell with
//! NIP-HOST `terminal.open`, and [`Session::attach`] one that attaches to a
//! terminal the host already runs, such as the one a screen showed before
//! the app went to the background. Either attaches with NIP-TERM in
//! `interact` mode, or `observe` mode when the model only watches,
//! and applies the attachment's frames in sequence order to the model's
//! emulator. Input, resize, and close travel as NIP-TERM requests on the same
//! link. The task never queues input for later: while the link is down,
//! typed bytes are refused on the screen rather than held.
//!
//! When the link drops or the supervisor replaces it, the task attaches
//! again with `after` set to the last frame it applied, so the host replays
//! what the screen missed or reports a gap. A frame that arrives ahead of
//! the next expected one waits in [`Ordered`]; if the missing frame does
//! not arrive soon, the task reattaches to repair it. A host that restarted
//! answers `lost`, which ends the session, except for a terminal the host
//! opened just now: then the link named an older generation (a relay route
//! outlives a host restart), so the task reads the host's generation from
//! fresh presence and attaches again.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use coder_access::protocol::{Operation, Outcome};
use coder_access::{Code, Error as AccessError};
use coder_host::Error as HostError;
use coder_host::client::Incoming;
use coder_host::client::{Link, Ordered, Route};
use coder_host::mailbox::terminal_generation;
use coder_host::message::TermRequest;
use coder_host::pty::client::{Applied, TerminalState};
use coder_host::pty::ext::{
    BlockPageRead, BlockState, Join, Member, MemberState, RecordsFrame, Seat, SessionList,
    SessionRead, SessionRecord,
};
use coder_host::pty::wire::{
    Attach, Body, Cause, Close, Detach, Detached, Exit, Frame, Input, Mode, Reason, Resize, Size,
    Status, TerminalRef, TerminalResult, Value,
};
use coder_host::reach::new_id;
use coder_vt::{StreamEvent, Streams};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::model::{
    BlockRow, Blocks, Model, Phase, SCROLLBACK, Saved, SavedEntry, SavedMember, Typing,
};
use crate::controller::describe;

/// The current link to a host, as the Computers service's supervisor holds
/// it. It answers a transport error while the host is not connected.
pub type Links = Arc<dyn Fn() -> Result<Arc<Link>, AccessError> + Send + Sync>;

/// Output bytes per second asked for over a direct channel.
const DIRECT_RATE: u64 = 64 * 1024;
/// Output bytes per second asked for over the relay.
const RELAY_RATE: u64 = 16 * 1024;
/// The most input bytes one request carries.
const INPUT_CHUNK: usize = 4096;
/// How long a frame may wait for an earlier one before the task reattaches.
const HOLD_LIMIT: Duration = Duration::from_secs(2);
/// How often the task checks whether the supervisor replaced the link.
const LINK_CHECK: Duration = Duration::from_secs(2);
/// How long to wait before trying an unreachable host again.
const RETRY: Duration = Duration::from_secs(1);
/// How many times a session reads presence for a restarted host's new
/// generation before it reports its new terminal lost.
const GENERATION_TRIES: u32 = 10;

/// One private owner sharing operation. Mutations are never queued for reconnect.
#[derive(Clone, Debug)]
pub enum SharingCommand {
    Read,
    Issue {
        grantee: String,
        mode: coder_host::pty::share::ShareMode,
        expires_at: u64,
    },
    Pause(bool),
    Revoke(Option<String>),
    Handoff {
        agent: String,
        thread: String,
        run: String,
    },
}

enum Command {
    Sharing {
        action: SharingCommand,
        answer: std::sync::mpsc::Sender<Result<Value, String>>,
    },
    LeaveWait(std::sync::mpsc::Sender<()>),
    OwnerProposal {
        action: coder_host::pty::proposal::Action,
        answer: std::sync::mpsc::Sender<Result<coder_host::pty::proposal::Page, String>>,
    },
    Reconcile,
    Proposals,
    DecideProposal {
        thread: String,
        proposal: String,
        revision: u64,
        approve: bool,
    },
    Bytes(Vec<u8>),
    Resize(u16, u16),
    /// Take the typist role.
    Take,
    /// Read a page of the block journal older than this block, or the
    /// newest page.
    Blocks(Option<u64>),
    /// List the host's saved sessions, or read the one named.
    Saved(Option<String>),
    Close,
    Leave,
}

/// The most blocks one page shows.
pub const BLOCK_PAGE: u16 = 8;

/// Which terminal a session drives.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    /// A new shell the host opens.
    Open,
    /// A terminal the host already runs.
    Attach(TerminalRef),
}

/// A running terminal session. Dropping it detaches.
pub struct Session {
    model: Arc<Mutex<Model>>,
    commands: mpsc::UnboundedSender<Command>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl Session {
    /// Open a terminal on the host `model` names, over the links `links`
    /// returns, on `runtime`.
    #[must_use]
    pub fn start(runtime: &Handle, links: Links, model: Model) -> Self {
        Self::spawn(runtime, links, model, Target::Open)
    }

    /// Attach to the terminal `reference` names, which the host already
    /// runs, without opening another. A host that restarted since answers
    /// `lost`, and the screen says so rather than opening a new shell.
    #[must_use]
    pub fn attach(runtime: &Handle, links: Links, model: Model, reference: TerminalRef) -> Self {
        Self::spawn(runtime, links, model, Target::Attach(reference))
    }

    fn spawn(runtime: &Handle, links: Links, model: Model, target: Target) -> Self {
        let model = Arc::new(Mutex::new(model));
        let (commands, receiver) = mpsc::unbounded_channel();
        runtime.spawn(run(links, model.clone(), receiver, target));
        Session { model, commands }
    }

    /// Show the newest page of the terminal's block journal, or the page
    /// older than block `before`.
    pub fn blocks(&self, before: Option<u64>) {
        {
            let mut model = self.model();
            if model.phase != Phase::Attached {
                model.blocks = Blocks::Unavailable("Not connected.".into());
                model.touch();
                return;
            }
            model.blocks = Blocks::Reading;
            model.touch();
        }
        let _ = self.commands.send(Command::Blocks(before));
    }

    /// Reads the bounded proposal page from the terminal owner.
    /// Sends one exact owner proposal operation without retaining it for reconnect.
    pub fn owner_proposal(
        &self,
        action: coder_host::pty::proposal::Action,
    ) -> std::sync::mpsc::Receiver<Result<coder_host::pty::proposal::Page, String>> {
        let (answer, receive) = std::sync::mpsc::channel();
        let model = self.model();
        if model.phase != Phase::Attached || model.watch {
            let _ = answer.send(Err("Terminal input is unavailable.".into()));
        } else {
            let _ = self
                .commands
                .send(Command::OwnerProposal { action, answer });
        }
        receive
    }

    /// Sends one sharing operation and confirms only the host's exact reply.
    pub fn owner_sharing(
        &self,
        action: SharingCommand,
    ) -> std::sync::mpsc::Receiver<Result<Value, String>> {
        let (answer, receive) = std::sync::mpsc::channel();
        if self.model().phase != Phase::Attached {
            let _ = answer.send(Err("The terminal is offline. Nothing was sent.".into()));
        } else {
            let _ = self.commands.send(Command::Sharing { action, answer });
        }
        receive
    }

    /// Reattaches for an authoritative snapshot without replaying input.
    pub fn reconcile(&self) {
        let _ = self.commands.send(Command::Reconcile);
    }

    pub fn proposals(&self) {
        let _ = self.commands.send(Command::Proposals);
    }
    /// A decision is live-only and names exactly the displayed revision.
    pub fn decide_proposal(&self, thread: String, proposal: String, revision: u64, approve: bool) {
        let model = self.model();
        if model.phase != Phase::Attached || model.watch {
            return;
        }
        if !model.proposals.as_ref().is_some_and(|page| {
            page.entries.iter().any(|entry| {
                entry.proposal.thread == thread
                    && entry.proposal.id == proposal
                    && entry.proposal.revision == revision
                    && matches!(
                        entry.state,
                        coder_host::pty::proposal::State::Pending
                            | coder_host::pty::proposal::State::Warned { .. }
                    )
            })
        }) {
            return;
        }
        drop(model);
        let _ = self.commands.send(Command::DecideProposal {
            thread,
            proposal,
            revision,
            approve,
        });
    }

    /// Show the host's saved sessions, or the members of the one named.
    pub fn saved(&self, session: Option<String>) {
        {
            let mut model = self.model();
            if model.phase != Phase::Attached {
                model.saved = Saved::Unavailable("Not connected.".into());
                model.touch();
                return;
            }
            model.saved = Saved::Reading;
            model.touch();
        }
        let _ = self.commands.send(Command::Saved(session));
    }

    /// Hide the saved sessions.
    pub fn hide_saved(&self) {
        let mut model = self.model();
        if model.saved != Saved::Hidden {
            model.saved = Saved::Hidden;
            model.touch();
        }
    }

    /// Hide the block list.
    pub fn hide_blocks(&self) {
        let mut model = self.model();
        if model.blocks != Blocks::Hidden {
            model.blocks = Blocks::Hidden;
            model.touch();
        }
    }

    /// The screen's state.
    pub fn model(&self) -> MutexGuard<'_, Model> {
        lock(&self.model)
    }

    /// Send typed bytes. Refused on the screen, not queued, unless the
    /// session is attached.
    pub fn send(&self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        {
            let mut model = self.model();
            if model.watch {
                model.notice = Some("You're watching this terminal; typing isn't sent.".into());
                model.touch();
                return;
            }
            if model.phase != Phase::Attached {
                model.notice = Some("Not connected. What you typed wasn't sent.".into());
                model.touch();
                return;
            }
        }
        let _ = self.commands.send(Command::Bytes(bytes));
    }

    /// Resize the grid and tell the host.
    pub fn resize(&self, rows: u16, cols: u16) {
        let mut model = self.model();
        if model.watch {
            // A watcher draws at the terminal's size and never sets it.
            model.view = super::model::clamp(rows, cols);
            model.touch();
            return;
        }
        let changed = model.resize(rows, cols);
        drop(model);
        if let Some((rows, cols)) = changed {
            let _ = self.commands.send(Command::Resize(rows, cols));
        }
    }

    /// Take the typist role from another device, so this screen types.
    pub fn take(&self) {
        let _ = self.commands.send(Command::Take);
    }

    /// End the shell on the host.
    pub fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }

    /// Detach. The shell keeps running on the host.
    /// Detaches before a mount drops its runtime, with a bounded wait and no process close.
    pub fn leave_wait(&self, timeout: Duration) {
        if self.model().phase != Phase::Attached {
            self.leave();
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = self.commands.send(Command::LeaveWait(sender));
        let _ = receiver.recv_timeout(timeout);
    }

    pub fn leave(&self) {
        let _ = self.commands.send(Command::Leave);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.leave();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Why opening or attaching stopped.
enum Stop {
    /// The person left.
    Left,
    /// The session ended in this phase.
    Ended(Phase),
}

fn route(link: &Link) -> String {
    match link.route() {
        Route::Direct(_) => "directly".into(),
        Route::Relay(_) => "through the relay".into(),
    }
}

fn rate(link: &Link) -> u64 {
    match link.route() {
        Route::Direct(_) => DIRECT_RATE,
        Route::Relay(_) => RELAY_RATE,
    }
}

/// The phase a NIP-HOST refusal of `terminal.open` ends in, or `None` to
/// try again.
fn open_refusal(error: &HostError) -> Option<Phase> {
    match error {
        HostError::Access(error) => match error.code {
            Code::Transport => None,
            Code::Unsupported => Some(Phase::Refused(
                "The computer's host serves no workspace, so it can't open a terminal. Add one with `host serve --workspace LABEL=PATH`.".into(),
            )),
            Code::Unavailable => Some(Phase::Refused(
                "The computer can't open a terminal now. Its host's workspace directory is missing or unusable; check the host log.".into(),
            )),
            _ => Some(Phase::Refused(describe(error))),
        },
        HostError::Reach(_) | HostError::Config(_) => Some(Phase::Refused(
            "The computer couldn't open a terminal.".into(),
        )),
        _ => None,
    }
}

/// The phase a terminal whose process ended shows.
fn exited_phase(exit: Exit) -> Phase {
    Phase::Exited {
        code: exit.code,
        signal: exit.signal,
        cause: match exit.cause {
            Cause::Exited => "exited",
            Cause::Closed => "closed",
            Cause::IdleExpired => "idle",
            Cause::HostShutdown => "shutdown",
        },
    }
}

/// The most frames held for a snapshot's `READY` before the session
/// attaches again.
const HELD_BEFORE_READY: usize = 4096;

/// The NIP-TERM features an attach asks for, most first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Features {
    /// Join by snapshot, with effects and the typist role.
    Typist,
    /// Join by snapshot, with effects.
    Snapshot,
    /// Replay, with effects: the host answers queries.
    Effects,
    /// The base profile: this device answers queries.
    Base,
}

impl Features {
    fn fewer(self) -> Self {
        match self {
            Features::Typist => Features::Snapshot,
            Features::Snapshot => Features::Effects,
            Features::Effects | Features::Base => Features::Base,
        }
    }

    fn snapshot(self) -> bool {
        matches!(self, Features::Typist | Features::Snapshot)
    }
}

/// How this session's input, resizes, and takes name it: by its
/// attachment, once it attached with the typist feature.
#[derive(Clone, Debug, Default)]
struct Speaker(Option<String>);

impl Speaker {
    fn input(&self, reference: &TerminalRef, data: &[u8]) -> TermRequest {
        let input = Input::new(new_id(), reference.clone(), data);
        TermRequest::Input(match &self.0 {
            Some(attachment) => input.from_attachment(attachment),
            None => input,
        })
    }

    fn resize(&self, reference: &TerminalRef, rows: u16, cols: u16) -> TermRequest {
        let resize = Resize::new(new_id(), reference.clone(), Size::new(rows, cols));
        TermRequest::Resize(match &self.0 {
            Some(attachment) => resize.from_attachment(attachment),
            None => resize,
        })
    }
}

/// What one attachment negotiated and, joining by snapshot, how far the
/// join got.
struct Joining {
    /// The host answers queries (the effects feature).
    host_answers: bool,
    /// How requests name this attachment.
    speaker: Speaker,
    /// The record streams of an attachment that joined by snapshot.
    streams: Option<Streams>,
    /// Whether sequenced frames apply now: always on replay, and after the
    /// snapshot's `READY` on a join.
    ready: bool,
    /// Frames that arrived before `READY`.
    held: Vec<Frame>,
}

impl Joining {
    fn new(level: Features, reference: &TerminalRef, speaker: Speaker) -> Self {
        let snapshot = level.snapshot();
        Joining {
            host_answers: level != Features::Base,
            speaker,
            streams: snapshot.then(|| Streams::new(reference.clone(), SCROLLBACK)),
            ready: !snapshot,
            held: Vec::new(),
        }
    }

    /// Applies one part of a record stream: a snapshot's `READY` replaces
    /// the model's screen and answers its `through`, and history pages add
    /// older rows. A broken snapshot before the first `READY` is an error:
    /// the session attaches again.
    fn records(
        &mut self,
        model: &Arc<Mutex<Model>>,
        part: &RecordsFrame,
        exited: &mut bool,
    ) -> Result<Option<u64>, ()> {
        let Some(streams) = self.streams.as_mut() else {
            return Ok(None);
        };
        let events = match streams.push(part) {
            Ok(events) => events,
            // After a READY the screen stands; only that stream is lost.
            Err(_) if self.ready => return Ok(None),
            Err(_) => return Err(()),
        };
        let mut model = lock(model);
        let mut through = None;
        if let Some(tap) = &model.projection {
            tap.send(super::model::Projection::Records(part.clone()));
        }
        for event in events {
            match event {
                StreamEvent::Ready {
                    terminal,
                    through: at,
                    exit,
                } => {
                    let (rows, cols) = model.size();
                    let mut vt = *terminal;
                    vt.resize(usize::from(rows), usize::from(cols));
                    model.vt = vt;
                    model.touch();
                    if let Some(exit) = exit {
                        *exited = true;
                        model.set_phase(exited_phase(exit));
                    }
                    self.ready = true;
                    through = Some(at);
                }
                StreamEvent::History { epoch, page } => {
                    if model.vt.attach_history(epoch, &page).is_ok() {
                        model.touch();
                    }
                }
                StreamEvent::Finished { .. } => {}
            }
        }
        Ok(through)
    }
}

/// Whether an attach that named the effects feature was refused for naming
/// it, by a host that predates the feature: attach again without it.
fn refuses_feature(result: &TerminalResult) -> bool {
    result.status == Status::Refused
        && matches!(
            result.reason,
            Some(Reason::UnsupportedFeature | Reason::UnsupportedVersion)
        )
}

/// The phase a refused attach ends in.
fn attach_refusal(reason: Option<Reason>) -> Phase {
    match reason {
        Some(Reason::Lost) => Phase::Lost,
        Some(Reason::Closed | Reason::Unavailable) => Phase::Closed,
        Some(Reason::NotAdmitted | Reason::Revoked) => Phase::Refused(
            "The computer refused: This device doesn't have the \"Open terminals\" right on this computer."
                .into(),
        ),
        _ => Phase::Refused("The computer refused to attach this terminal.".into()),
    }
}

/// Wait for a usable link, or for the person to leave.
async fn wait_link(
    links: &Links,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    first: bool,
) -> Result<Arc<Link>, Stop> {
    let mut delay = !first;
    loop {
        if delay {
            tokio::select! {
                command = commands.recv() => match command {
                    None | Some(Command::Leave) => return Err(Stop::Left),
                    // Nothing is queued while disconnected.
                    Some(_) => continue,
                },
                () = tokio::time::sleep(RETRY) => {}
            }
        }
        delay = true;
        match links() {
            Ok(link) if link.closed().is_none() => return Ok(link),
            Ok(_) => {}
            Err(error) if matches!(error.code, Code::Transport | Code::Unavailable) => {}
            Err(error) => return Err(Stop::Ended(Phase::Refused(describe(&error)))),
        }
    }
}

/// Wait for `work`, or stop when the person leaves. Other commands that
/// arrive meanwhile are dropped: nothing typed is queued while the session
/// is not attached, and the model already holds the latest size.
async fn until_left<T>(
    commands: &mut mpsc::UnboundedReceiver<Command>,
    work: impl Future<Output = T>,
) -> Result<T, Stop> {
    tokio::pin!(work);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                None | Some(Command::Leave) => return Err(Stop::Left),
                Some(_) => {}
            },
            done = &mut work => return Ok(done),
        }
    }
}

/// Ask the host to open a shell. Returns the link, the terminal, and the
/// size it opened at.
async fn open(
    links: &Links,
    model: &Arc<Mutex<Model>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<(Arc<Link>, TerminalRef, (u16, u16)), Stop> {
    let mut first = true;
    loop {
        lock(model).set_phase(Phase::Connecting);
        let link = wait_link(links, commands, first).await?;
        first = false;
        let Some(generation) = link.generation() else {
            continue;
        };
        let (rows, cols) = {
            let mut model = lock(model);
            model.set_phase(Phase::Opening);
            model.size()
        };
        let answer =
            until_left(commands, link.call(Operation::OpenTerminal { cols, rows })).await?;
        match answer {
            Ok(Outcome::Dispatched { receipt }) => {
                let reference = TerminalRef {
                    generation: terminal_generation(link.device().host(), generation),
                    terminal: receipt.reference,
                };
                return Ok((link, reference, (rows, cols)));
            }
            Ok(_) => {
                return Err(Stop::Ended(Phase::Refused(
                    "The computer answered with something other than a terminal.".into(),
                )));
            }
            Err(error) => {
                if let Some(phase) = open_refusal(&error) {
                    return Err(Stop::Ended(phase));
                }
            }
        }
    }
}

async fn run(
    links: Links,
    model: Arc<Mutex<Model>>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    target: Target,
) {
    let ended = match drive(&links, &model, &mut commands, target).await {
        Stop::Left => Phase::Left,
        Stop::Ended(phase) => phase,
    };
    let mut model = lock(&model);
    // An exit the host reported stays on the screen after leaving.
    if !(matches!(model.phase, Phase::Exited { .. }) && ended == Phase::Left) {
        model.set_phase(ended);
    }
}

/// What the attached loop decided.
enum Next {
    /// Attach again after the last applied frame, on this link or a new one.
    Reattach {
        new_link: bool,
    },
    Stop(Stop),
}

async fn drive(
    links: &Links,
    model: &Arc<Mutex<Model>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    target: Target,
) -> Stop {
    let opened = match target {
        Target::Open => open(links, model, commands).await,
        Target::Attach(reference) => {
            lock(model).set_phase(Phase::Connecting);
            // The terminal's size is the host's; the first attach says it.
            wait_link(links, commands, true)
                .await
                .map(|link| (link, reference, (0, 0)))
        }
    };
    let (mut link, mut reference, mut host_size) = match opened {
        Ok(opened) => opened,
        Err(stop) => return stop,
    };
    let watch = lock(model).watch;
    // The client state tracks sequence numbers and gaps; the emulator in
    // the model draws the output.
    let mut ordered = Ordered::new(TerminalState::new(reference.clone(), 1, 1));
    let mut exited = false;
    let mut first = true;
    // Whether an attach has succeeded, and whether a `lost` refusal of the
    // first one sent the session to fresh presence for the generation.
    let mut attached_once = false;
    let mut rechecked = false;
    // The features to ask for: a join by snapshot with effects, effects
    // alone, or the base profile. An older host refuses a feature, and the
    // session attaches again asking for less.
    let mut level = Features::Typist;
    loop {
        if !first {
            lock(model).set_phase(Phase::Reconnecting);
        }
        // A join by snapshot starts from the host's state, not a sequence
        // number; the others resume after the last applied frame.
        let after = if level.snapshot() {
            0
        } else {
            ordered.state().resume_after()
        };
        let mode = if watch { Mode::Observe } else { Mode::Interact };
        let mut attach = Attach::new(new_id(), reference.clone(), mode, after, rate(&link));
        if level.snapshot() {
            attach = attach.joining(Join::Snapshot);
        }
        if level != Features::Base {
            attach = attach.with_effects();
        }
        if level == Features::Typist {
            attach = attach.with_typist();
        }
        let attach = TermRequest::Attach(attach);
        let attached = match until_left(commands, link.terminal(attach)).await {
            Ok(attached) => attached,
            Err(stop) => return stop,
        };
        let attachment = match attached {
            Ok(TerminalResult {
                status: Status::Accepted | Status::Duplicate,
                value:
                    Some(Value::Attached {
                        attachment, size, ..
                    }),
                ..
            }) => {
                if host_size == (0, 0) {
                    host_size = (size.rows, size.cols);
                }
                attachment
            }
            Ok(result) if level != Features::Base && refuses_feature(&result) => {
                level = level.fewer();
                continue;
            }
            Ok(result) if result.status == Status::Refused => {
                // A terminal that already reported its exit stays exited.
                if exited && result.reason == Some(Reason::Closed) {
                    return Stop::Ended(lock(model).phase.clone());
                }
                if !attached_once {
                    if result.reason == Some(Reason::Lost) && !rechecked {
                        // The host opened this terminal just now, on its
                        // current generation, so the link named an older
                        // one: a relay route that outlived a host restart.
                        // Follow fresh presence and attach again.
                        rechecked = true;
                        match until_left(commands, newer_reference(&link, &reference)).await {
                            Err(stop) => return stop,
                            Ok(Some(fresh)) => {
                                reference = fresh;
                                ordered = Ordered::new(TerminalState::new(reference.clone(), 1, 1));
                                continue;
                            }
                            Ok(None) => {}
                        }
                    }
                    // The host restarted again between opening and
                    // attaching: the terminal did not survive it.
                    if rechecked
                        && matches!(result.reason, Some(Reason::Closed | Reason::Unavailable))
                    {
                        return Stop::Ended(Phase::Lost);
                    }
                }
                return Stop::Ended(attach_refusal(result.reason));
            }
            _ => {
                link = match wait_link(links, commands, false).await {
                    Ok(link) => link,
                    Err(stop) => return stop,
                };
                first = false;
                continue;
            }
        };
        attached_once = true;
        {
            let mut model = lock(model);
            model.route = Some(route(&link));
            model.reference = Some((reference.generation.clone(), reference.terminal.clone()));
            if watch {
                // A watcher draws at the host's size.
                model.seat(Typing::Elsewhere, host_size);
            }
            if !exited {
                model.set_phase(Phase::Attached);
            }
            model.touch();
        }
        let speaker = Speaker((level == Features::Typist && !watch).then(|| attachment.clone()));
        // The screen may have changed size while opening or detached. A
        // host with another typist refuses it, and this screen follows.
        let size = lock(model).size();
        if size != host_size && !exited && !watch {
            let _ = request(&link, speaker.resize(&reference, size.0, size.1)).await;
            host_size = size;
        }
        first = false;
        let next = attached_loop(
            links,
            model,
            commands,
            &link,
            &reference,
            &attachment,
            &mut ordered,
            &mut exited,
            &mut host_size,
            &mut Joining::new(level, &reference, speaker),
        )
        .await;
        match next {
            Next::Stop(stop) => {
                if matches!(stop, Stop::Left) {
                    detach(&link, &reference, &attachment).await;
                }
                return stop;
            }
            Next::Reattach { new_link } => {
                if !exited {
                    lock(model).set_phase(Phase::Reconnecting);
                }
                let old = link.clone();
                let old_attachment = attachment.clone();
                let old_reference = reference.clone();
                // End the old attachment if its route still works.
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(3),
                        detach(&old, &old_reference, &old_attachment),
                    )
                    .await;
                });
                if new_link {
                    link = match wait_link(links, commands, false).await {
                        Ok(link) => link,
                        Err(stop) => return stop,
                    };
                }
            }
        }
    }
}

/// The reference to `reference`'s terminal on the host generation fresh
/// presence names, once it differs from the one `reference` names; `None`
/// when it does not change soon, or the link is a direct channel, whose
/// handshake proved its generation.
async fn newer_reference(link: &Link, reference: &TerminalRef) -> Option<TerminalRef> {
    if !matches!(link.route(), Route::Relay(_)) {
        return None;
    }
    for attempt in 0..GENERATION_TRIES {
        if attempt > 0 {
            tokio::time::sleep(RETRY).await;
        }
        if let Ok(Some(generation)) = link.refresh_generation().await {
            let generation = terminal_generation(link.device().host(), generation);
            if generation != reference.generation {
                return Some(TerminalRef {
                    generation,
                    terminal: reference.terminal.clone(),
                });
            }
        }
    }
    None
}

async fn request(link: &Link, request: TermRequest) -> Result<TerminalResult, HostError> {
    link.terminal(request).await
}

async fn detach(link: &Link, reference: &TerminalRef, attachment: &str) {
    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        link.terminal(TermRequest::Detach(Detach::new(
            new_id(),
            reference.clone(),
            attachment,
        ))),
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn attached_loop(
    links: &Links,
    model: &Arc<Mutex<Model>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    link: &Arc<Link>,
    reference: &TerminalRef,
    attachment: &str,
    ordered: &mut Ordered,
    exited: &mut bool,
    host_size: &mut (u16, u16),
    joining: &mut Joining,
) -> Next {
    let mut held_since: Option<Instant> = None;
    let mut checked = Instant::now();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let command = command.unwrap_or(Command::Leave);
                if let Some(next) =
                    handle(command, model, link, reference, *exited, host_size, &joining.speaker).await
                {
                    return next;
                }
            }
            incoming = link.next_incoming(Duration::from_millis(250)) => {
                let mut frames = Vec::new();
                match incoming {
                    Some(Incoming::Frame(frame))
                        if frame.terminal == *reference && frame.attachment == attachment =>
                    {
                        if joining.ready {
                            frames.push(frame);
                        } else if joining.held.len() < HELD_BEFORE_READY {
                            // Over a relay a frame can overtake the snapshot.
                            joining.held.push(frame);
                        } else {
                            return Next::Reattach { new_link: false };
                        }
                    }
                    Some(Incoming::Records(part))
                        if part.terminal == *reference && part.attachment == attachment =>
                    {
                        match joining.records(model, &part, exited) {
                            Ok(Some(through)) => {
                                // The snapshot holds everything through
                                // `through`; frames after it follow.
                                *ordered = Ordered::new(
                                    TerminalState::new(reference.clone(), 1, 1)
                                        .starting_after(through),
                                );
                                frames = std::mem::take(&mut joining.held);
                            }
                            Ok(None) => {}
                            Err(()) => return Next::Reattach { new_link: false },
                        }
                    }
                    _ => {}
                }
                for frame in frames {
                    let (replies, next) = apply(model, ordered, frame, exited);
                    // With the effects feature the host's emulator answers
                    // queries; answering here too would answer twice.
                    if !replies.is_empty() && !*exited && !joining.host_answers {
                        send_input(link, reference, model, replies, &joining.speaker).await;
                    }
                    if let Some(next) = next {
                        return next;
                    }
                }
                // This screen may set the size again: it took the role, or
                // nobody holds it.
                let pending = lock(model).pending_resize.take();
                if let Some((rows, cols)) = pending
                    && !*exited
                    && *host_size != (rows, cols)
                {
                    let _ = request(link, joining.speaker.resize(reference, rows, cols)).await;
                    *host_size = (rows, cols);
                }
            }
        }
        if link.closed().is_some() {
            return Next::Reattach { new_link: true };
        }
        // A frame waits for an earlier one that did not arrive: repair.
        if ordered.held() > 0 {
            let since = *held_since.get_or_insert_with(Instant::now);
            if since.elapsed() > HOLD_LIMIT {
                return Next::Reattach { new_link: false };
            }
        } else {
            held_since = None;
        }
        // The supervisor may have replaced the route, for example with a
        // direct channel after the relay.
        if checked.elapsed() > LINK_CHECK {
            checked = Instant::now();
            match links() {
                Ok(current)
                    if !Arc::ptr_eq(&current, link) && current.closed().is_none() && !*exited =>
                {
                    return Next::Reattach { new_link: true };
                }
                Err(error) if error.code == Code::Transport && !*exited => {
                    return Next::Reattach { new_link: true };
                }
                Err(error) if !*exited => {
                    return Next::Stop(Stop::Ended(Phase::Refused(describe(&error))));
                }
                _ => {}
            }
        }
    }
}

/// Apply one frame and every held frame it releases. Returns reply bytes
/// the emulator queued and what to do next, if anything.
fn apply(
    model: &Arc<Mutex<Model>>,
    ordered: &mut Ordered,
    frame: Frame,
    exited: &mut bool,
) -> (Vec<u8>, Option<Next>) {
    let mut model = lock(model);
    let mut next = None;
    for (applied, frame) in ordered.push_frames(frame) {
        match applied {
            Applied::Output { .. } => {
                if let Body::Output { data, .. } = &frame.body {
                    model.output(data);
                }
            }
            Applied::Gap { bytes, .. } => model.gap(bytes),
            Applied::Exit(exit) => {
                *exited = true;
                model.set_phase(exited_phase(exit));
            }
            Applied::Detached(Detached::Revoked) => {
                next = Some(Next::Stop(Stop::Ended(Phase::Refused(
                    "This computer revoked this device's terminal access.".into(),
                ))));
            }
            Applied::Detached(Detached::Transport) | Applied::Behind { .. } => {
                next.get_or_insert(Next::Reattach { new_link: false });
            }
            Applied::Typist { typist, size } => {
                let typing = match typist {
                    None => Typing::Free,
                    Some(id) if id == frame.attachment => Typing::Mine,
                    Some(_) => Typing::Elsewhere,
                };
                model.seat(typing, (size.rows, size.cols));
            }
            // The owner's own attachments are never under a share.
            Applied::Detached(Detached::Requested)
            | Applied::Duplicate
            | Applied::Refused(_)
            | Applied::Effect(_)
            | Applied::Paused(_) => {}
        }
    }
    let replies = model.vt.take_replies();
    (replies, next)
}

/// Run one command. Returns what to do next when the command ends the
/// attachment.
async fn handle(
    command: Command,
    model: &Arc<Mutex<Model>>,
    link: &Link,
    reference: &TerminalRef,
    exited: bool,
    host_size: &mut (u16, u16),
    speaker: &Speaker,
) -> Option<Next> {
    match command {
        Command::LeaveWait(answer) => {
            if let Some(attachment) = &speaker.0 {
                detach(link, reference, attachment).await;
            }
            let _ = answer.send(());
            Some(Next::Stop(Stop::Left))
        }
        Command::Sharing { action, answer } => {
            use coder_host::pty::share::{SharePause, ShareRequest, Unshare, ViewersRead};
            let operation = match action {
                SharingCommand::Handoff { agent, thread, run } => {
                    let Some(attachment) = &speaker.0 else {
                        let _ = answer.send(Err("This attachment cannot hand off typing.".into()));
                        return None;
                    };
                    TermRequest::Handoff(coder_host::pty::ext::Handoff::new(
                        new_id(),
                        reference.clone(),
                        attachment.clone(),
                        agent,
                        thread,
                        run,
                    ))
                }
                SharingCommand::Read => {
                    TermRequest::Viewers(ViewersRead::new(new_id(), reference.clone()))
                }
                SharingCommand::Issue {
                    grantee,
                    mode,
                    expires_at,
                } => TermRequest::Share(ShareRequest::new(
                    new_id(),
                    reference.clone(),
                    grantee,
                    mode,
                    expires_at,
                )),
                SharingCommand::Pause(paused) => {
                    TermRequest::SharePause(SharePause::new(new_id(), reference.clone(), paused))
                }
                SharingCommand::Revoke(Some(share)) => {
                    TermRequest::Unshare(Unshare::one(new_id(), reference.clone(), share))
                }
                SharingCommand::Revoke(None) => {
                    TermRequest::Unshare(Unshare::all(new_id(), reference.clone()))
                }
            };
            let result = match request(link, operation).await {
                Ok(result) if result.status != Status::Refused => result
                    .value
                    .ok_or_else(|| "The host's sharing reply had no value.".into()),
                Ok(result) => Err(format!("The host refused sharing: {:?}.", result.reason)),
                Err(_) => Err("The sharing change is unknown. Refresh before acting again.".into()),
            };
            let _ = answer.send(result);
            None
        }
        Command::OwnerProposal { mut action, answer } => {
            use coder_host::pty::proposal::{Action, Request};
            if exited || lock(model).watch {
                let _ = answer.send(Err("Terminal input is unavailable.".into()));
                return None;
            }
            if let Action::Decide { attachment, .. } = &mut action {
                let Some(current) = &speaker.0 else {
                    let _ = answer.send(Err("This attachment has no proposal authority.".into()));
                    return None;
                };
                attachment.clone_from(current);
            }
            let result = request(
                link,
                TermRequest::Proposal(Request::new(new_id(), reference.clone(), action)),
            )
            .await;
            let result = match result {
                Ok(TerminalResult {
                    value: Some(Value::Proposals { page }),
                    ..
                }) => {
                    lock(model).proposals = Some(page.clone());
                    Ok(page)
                }
                Ok(result) => Err(format!(
                    "The host refused the proposal: {:?}.",
                    result.reason
                )),
                Err(_) => {
                    Err("Proposal disposition is unknown. Reconcile before acting again.".into())
                }
            };
            let _ = answer.send(result);
            None
        }
        Command::Leave => Some(Next::Stop(Stop::Left)),
        Command::Reconcile => Some(Next::Reattach { new_link: false }),
        Command::Bytes(bytes) => {
            if exited {
                return None;
            }
            send_input(link, reference, model, bytes, speaker).await
        }
        Command::Resize(rows, cols) => {
            if !exited && *host_size != (rows, cols) {
                let resized = request(link, speaker.resize(reference, rows, cols)).await;
                if matches!(resized, Ok(ref result) if result.status != Status::Refused) {
                    if let Some(tap) = &lock(model).projection {
                        tap.send(super::model::Projection::Size(rows, cols));
                    }
                    *host_size = (rows, cols);
                }
            }
            None
        }
        Command::Take => {
            let Some(attachment) = &speaker.0 else {
                let mut model = lock(model);
                model.notice =
                    Some("This computer's host can't hand over typing; update it.".into());
                model.touch();
                return None;
            };
            if exited {
                return None;
            }
            let take = Seat::take(new_id(), reference.clone(), attachment);
            let taken = request(link, TermRequest::Seat(take)).await;
            let mut model = lock(model);
            if matches!(taken, Ok(ref result) if result.status != Status::Refused) {
                model.notice = None;
            } else {
                model.notice = Some("The computer didn't hand over typing. Try again.".into());
            }
            model.touch();
            // The typist frame that follows sets the role and the size.
            None
        }
        Command::Proposals | Command::DecideProposal { .. } => {
            use coder_host::pty::proposal::{Action, Request};
            if exited {
                return None;
            }
            let action = match command {
                Command::DecideProposal {
                    thread,
                    proposal,
                    revision,
                    approve,
                } => {
                    if lock(model).watch {
                        return None;
                    }
                    Action::Decide {
                        thread,
                        proposal,
                        revision,
                        approve,
                        attachment: match &speaker.0 {
                            Some(attachment) => attachment.clone(),
                            None => {
                                return None;
                            }
                        },
                    }
                }
                _ => Action::Read { limit: 8 },
            };
            let answer = request(
                link,
                TermRequest::Proposal(Request::new(new_id(), reference.clone(), action)),
            )
            .await;
            let mut model = lock(model);
            match answer {
                Ok(TerminalResult {
                    value: Some(Value::Proposals { page }),
                    ..
                }) => {
                    model.proposals = Some(page);
                }
                Ok(result) => {
                    model.proposals = None;
                    model.notice = Some(
                        match result.reason {
                            Some(Reason::Stale) => {
                                "The proposal or shell context changed. Read proposals again."
                            }
                            Some(Reason::NotTypist) => {
                                "Another device types here. Choose Type here first."
                            }
                            Some(Reason::NotAdmitted | Reason::Revoked) => {
                                "This device may not decide proposals on this terminal."
                            }
                            Some(Reason::UnsupportedFeature | Reason::UnsupportedVersion) => {
                                "This computer does not support proposal controls. Update its host."
                            }
                            _ => "The computer refused this proposal request.",
                        }
                        .into(),
                    );
                }
                Err(_) => {
                    model.proposals = None;
                    model.notice = Some(
                        "Proposal disposition is unknown. Read proposals before deciding again."
                            .into(),
                    );
                }
            }
            model.touch();
            None
        }
        Command::Blocks(before) => {
            let read = BlockPageRead::new(new_id(), reference.clone(), before, BLOCK_PAGE);
            let answer = request(link, TermRequest::BlockPage(read)).await;
            if let Ok(TerminalResult {
                value: Some(Value::Blocks { page }),
                ..
            }) = &answer
            {
                if let Some(tap) = &lock(model).projection {
                    tap.send(super::model::Projection::Blocks(page.clone()));
                }
            }
            let blocks = blocks_from(answer);
            let mut model = lock(model);
            // The person may have hidden the list meanwhile.
            if model.blocks == Blocks::Reading {
                model.blocks = blocks;
                model.touch();
            }
            None
        }
        Command::Saved(session) => {
            let read = match session {
                None => TermRequest::SessionList(SessionList::new(new_id())),
                Some(session) => TermRequest::SessionRead(SessionRead::new(new_id(), session)),
            };
            let saved = saved_from(request(link, read).await);
            let mut model = lock(model);
            if model.saved == Saved::Reading {
                model.saved = saved;
                model.touch();
            }
            None
        }
        Command::Close => {
            if exited {
                return None;
            }
            let closed = request(
                link,
                TermRequest::Close(Close::new(new_id(), reference.clone())),
            )
            .await;
            if !matches!(closed, Ok(ref result) if result.status != Status::Refused) {
                let mut model = lock(model);
                model.notice = Some("The computer didn't close the terminal. Try again.".into());
                model.touch();
            }
            // The exit frame follows on the attachment.
            None
        }
    }
}

/// The saved-session view a session list or read answer shows.
fn saved_from(answer: Result<TerminalResult, HostError>) -> Saved {
    match answer {
        Ok(TerminalResult {
            value: Some(Value::Sessions { sessions }),
            ..
        }) => Saved::List(
            sessions
                .into_iter()
                .map(|entry| SavedEntry {
                    session: entry.session,
                    name: entry.name,
                    members: entry.members,
                })
                .collect(),
        ),
        Ok(TerminalResult {
            value: Some(Value::Session { record }),
            ..
        }) => saved_record(record),
        Ok(result) => Saved::Unavailable(match result.reason {
            Some(Reason::UnsupportedFeature | Reason::UnsupportedVersion) => {
                "This computer's host keeps no saved sessions; update it.".into()
            }
            Some(Reason::NotAdmitted | Reason::Revoked) => {
                "This device may not read this computer's saved sessions.".into()
            }
            Some(Reason::Unavailable) => "The computer can't read its saved sessions now.".into(),
            _ => "The computer didn't send its saved sessions.".into(),
        }),
        Err(_) => Saved::Unavailable("Couldn't reach the computer.".into()),
    }
}

/// One session's members as the screen shows them. A resource member is
/// a workbench reference the host stores unresolved: a thread shows as a
/// link, anything else as its kind and ID.
pub(crate) fn saved_record(record: SessionRecord) -> Saved {
    let members = record
        .members
        .into_iter()
        .map(|member| match member {
            Member::Terminal {
                member,
                terminal,
                state,
            } => SavedMember::Terminal {
                member,
                generation: terminal.generation,
                terminal: terminal.terminal,
                state: match state {
                    Some(MemberState::Live) => "live",
                    Some(MemberState::Closed) => "closed",
                    Some(MemberState::Lost) => "lost",
                    None => "unknown",
                },
            },
            Member::Resource { member, resource } => {
                let kind = resource["kind"].as_str().unwrap_or("resource").to_owned();
                let id = resource["id"]
                    .as_str()
                    .or_else(|| resource[kind.as_str()].as_str())
                    .unwrap_or("")
                    .to_owned();
                if kind == "thread" && !id.is_empty() {
                    SavedMember::Thread { member, thread: id }
                } else {
                    SavedMember::Other { member, kind, id }
                }
            }
        })
        .collect();
    Saved::Open {
        session: record.session.unwrap_or_default(),
        name: record.name,
        members,
    }
}

/// The block list a block-page answer shows.
fn blocks_from(answer: Result<TerminalResult, HostError>) -> Blocks {
    match answer {
        Ok(TerminalResult {
            value: Some(Value::Blocks { page }),
            ..
        }) => Blocks::Page {
            more: page.more,
            rows: page
                .blocks
                .into_iter()
                .map(|block| BlockRow {
                    number: block.block,
                    command: block.command,
                    dir: block.dir,
                    outcome: match (block.state, block.status) {
                        (BlockState::Running, _) => "running".into(),
                        (BlockState::Abandoned, _) => "abandoned".into(),
                        (BlockState::Finished, Some(0)) => "ok".into(),
                        (BlockState::Finished, Some(code)) => format!("exit {code}"),
                        (BlockState::Finished, None) => "done".into(),
                    },
                })
                .collect(),
        },
        Ok(result) => Blocks::Unavailable(match result.reason {
            Some(Reason::UnsupportedFeature | Reason::UnsupportedVersion) => {
                "This computer's host keeps no command list for terminals.".into()
            }
            Some(Reason::ContentUnavailable) => "Older commands left the host's list.".into(),
            Some(Reason::NotAdmitted | Reason::Revoked) => {
                "This device may not read this terminal's commands.".into()
            }
            _ => "The computer didn't send the command list.".into(),
        }),
        Err(_) => Blocks::Unavailable("Couldn't reach the computer.".into()),
    }
}

async fn send_input(
    link: &Link,
    reference: &TerminalRef,
    model: &Arc<Mutex<Model>>,
    bytes: Vec<u8>,
    speaker: &Speaker,
) -> Option<Next> {
    for chunk in bytes.chunks(INPUT_CHUNK) {
        let sent = request(link, speaker.input(reference, chunk)).await;
        match sent {
            Ok(result) if result.status == Status::Refused => {
                let mut model = lock(model);
                if result.reason == Some(Reason::NotTypist) {
                    model.typing = Typing::Elsewhere;
                }
                model.notice = Some(match result.reason {
                    Some(Reason::NotTypist) => {
                        "Another device is typing in this terminal. Tap Type here to take over."
                            .into()
                    }
                    Some(Reason::NotAdmitted | Reason::Revoked) => {
                        "The computer refused your input: this device lacks the terminal right."
                            .into()
                    }
                    Some(Reason::Closed) => "The shell has ended; input wasn't sent.".into(),
                    _ => "The computer refused your input.".into(),
                });
                model.touch();
                return None;
            }
            Ok(_) => {}
            Err(_) => {
                let mut model = lock(model);
                model.notice =
                    Some("Couldn't reach the computer. Your input may not have arrived.".into());
                model.touch();
                return Some(Next::Reattach { new_link: true });
            }
        }
    }
    let mut model = lock(model);
    if model.notice.take().is_some() {
        model.touch();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_host::pty::wire::Exit;

    fn reference() -> TerminalRef {
        TerminalRef {
            generation: "a".repeat(64),
            terminal: "b".repeat(64),
        }
    }

    fn frame(body: Body) -> Frame {
        Frame::new(reference(), "c".repeat(64), body)
    }

    fn output(seq: u64, text: &str) -> Frame {
        frame(Body::Output {
            seq,
            data: text.as_bytes().to_vec(),
        })
    }

    fn setup() -> (Arc<Mutex<Model>>, Ordered, bool) {
        let mut model = Model::new("h", "Mac", 6, 50);
        model.phase = Phase::Attached;
        (
            Arc::new(Mutex::new(model)),
            Ordered::new(TerminalState::new(reference(), 1, 1)),
            false,
        )
    }

    #[test]
    fn output_gaps_and_exit_reach_the_model_in_order() {
        let (model, mut ordered, mut exited) = setup();
        let (_, next) = apply(&model, &mut ordered, output(1, "$ make\r\n"), &mut exited);
        assert!(next.is_none());
        // A frame ahead of the next one waits; nothing is drawn yet.
        apply(&model, &mut ordered, output(5, "tail\r\n"), &mut exited);
        assert_eq!(ordered.held(), 1);
        assert!(!lock(&model).vt.text().contains("tail"));
        // The host reports 2 through 4 discarded: the gap shows, then the
        // held frame applies.
        let gap = frame(Body::Gap {
            from: 2,
            to: 4,
            bytes: Some(640),
        });
        apply(&model, &mut ordered, gap, &mut exited);
        let text = lock(&model).vt.text();
        assert!(
            text.contains("$ make\n[output lost: 640 bytes discarded by the host]\ntail"),
            "{text}"
        );
        assert_eq!(lock(&model).gaps, 1);
        // A duplicate changes nothing.
        let before = lock(&model).revision;
        apply(&model, &mut ordered, output(5, "tail\r\n"), &mut exited);
        assert_eq!(lock(&model).revision, before);
        let exit = Exit {
            cause: Cause::Exited,
            code: Some(2),
            signal: None,
        };
        apply(
            &model,
            &mut ordered,
            frame(Body::Exit { seq: 6, exit }),
            &mut exited,
        );
        assert!(exited);
        assert_eq!(
            lock(&model).phase.describe(),
            "The shell exited with code 2."
        );
    }

    #[test]
    fn replies_go_to_the_host_and_revocation_ends_the_session() {
        let (model, mut ordered, mut exited) = setup();
        let (replies, _) = apply(&model, &mut ordered, output(1, "\x1b[6n"), &mut exited);
        assert_eq!(replies, b"\x1b[1;1R");
        let (_, next) = apply(
            &model,
            &mut ordered,
            frame(Body::Detached {
                reason: Detached::Revoked,
            }),
            &mut exited,
        );
        assert!(matches!(
            next,
            Some(Next::Stop(Stop::Ended(Phase::Refused(ref reason)))) if reason.contains("revoked")
        ));
        let (_, next) = apply(
            &model,
            &mut ordered,
            frame(Body::Detached {
                reason: Detached::Transport,
            }),
            &mut exited,
        );
        assert!(matches!(next, Some(Next::Reattach { new_link: false })));
    }

    #[test]
    fn effect_frames_change_nothing_on_screen() {
        let (model, mut ordered, mut exited) = setup();
        apply(&model, &mut ordered, output(1, "$ "), &mut exited);
        let before = lock(&model).vt.text();
        let bell = frame(Body::Effect {
            after: 1,
            effect: coder_host::pty::ext::Effect::Bell { count: 1 },
        });
        let (replies, next) = apply(&model, &mut ordered, bell, &mut exited);
        assert!(replies.is_empty() && next.is_none());
        assert_eq!(lock(&model).vt.text(), before);
    }

    #[test]
    fn typist_frames_set_the_role_and_the_grid() {
        let (model, mut ordered, mut exited) = setup();
        let mine = frame(Body::Typist {
            typist: Some("c".repeat(64)),
            size: Size::new(6, 50),
        });
        apply(&model, &mut ordered, mine, &mut exited);
        assert_eq!(lock(&model).typing, Typing::Mine);
        let other = frame(Body::Typist {
            typist: Some("e".repeat(64)),
            size: Size::new(20, 90),
        });
        apply(&model, &mut ordered, other, &mut exited);
        let model = lock(&model);
        assert_eq!(model.typing, Typing::Elsewhere);
        assert_eq!(model.size(), (20, 90));
    }

    #[test]
    fn an_older_host_refusing_effects_gets_the_base_profile() {
        let refused = |reason| {
            TerminalResult::from_outcome(
                "d".repeat(64),
                Err(coder_host::pty::wire::Refusal::new(reason, "no")),
            )
        };
        assert!(refuses_feature(&refused(Reason::UnsupportedFeature)));
        assert!(refuses_feature(&refused(Reason::UnsupportedVersion)));
        assert!(!refuses_feature(&refused(Reason::NotAdmitted)));
        assert!(!refuses_feature(&refused(Reason::Lost)));
    }

    #[test]
    fn refusals_map_to_clear_phases() {
        assert_eq!(attach_refusal(Some(Reason::Lost)), Phase::Lost);
        assert_eq!(attach_refusal(Some(Reason::Closed)), Phase::Closed);
        let Phase::Refused(text) = attach_refusal(Some(Reason::NotAdmitted)) else {
            panic!("a missing right is a refusal")
        };
        assert!(text.contains("\"Open terminals\" right"));
        let mut error = AccessError::new(Code::MissingRight, "missing");
        error.missing = Some(coder_access::Right::Terminal);
        let Some(Phase::Refused(text)) = open_refusal(&HostError::Access(error)) else {
            panic!("a missing right is a refusal")
        };
        assert!(text.contains("\"Open terminals\" right"), "{text}");
        let none = HostError::Access(AccessError::new(Code::Unsupported, "none"));
        let Some(Phase::Refused(text)) = open_refusal(&none) else {
            panic!("no workspace is a refusal")
        };
        assert!(text.contains("serves no workspace"), "{text}");
        let unusable = HostError::Access(AccessError::new(Code::Unavailable, "gone"));
        let Some(Phase::Refused(text)) = open_refusal(&unusable) else {
            panic!("a missing root is a refusal")
        };
        assert!(text.contains("workspace directory is missing"), "{text}");
        let transport = HostError::Access(AccessError::new(Code::Transport, "down"));
        assert!(open_refusal(&transport).is_none());
        assert!(open_refusal(&HostError::Closed(None)).is_none());
    }
    #[test]
    fn proposal_decisions_are_live_exact_revision_actions_not_offline_input() {
        let mut model = Model::new("a".repeat(64), "Scratch host", 24, 80);
        model.phase = Phase::Attached;
        model.proposals = Some(coder_host::pty::proposal::Page {
            entries: vec![coder_host::pty::proposal::Entry {
                proposal: coder_host::pty::proposal::Proposal {
                    thread: "thread".into(),
                    id: "proposal".into(),
                    revision: 7,
                    command: "printf reviewed".into(),
                    binding: coder_host::pty::proposal::Binding {
                        terminal: "a".repeat(64),
                        generation: "b".repeat(64),
                        cwd: "/scratch".into(),
                        shell_directory: Some("/scratch".into()),
                        context_digest: "c".repeat(64),
                    },
                },
                effect: coder_host::pty::proposal::Effect::Destructive("Confirm changes.".into()),
                state: coder_host::pty::proposal::State::Pending,
            }],
            more: false,
        });
        let (commands, mut received) = mpsc::unbounded_channel();
        let session = Session {
            model: Arc::new(Mutex::new(model)),
            commands,
        };
        session.decide_proposal("thread".into(), "proposal".into(), 6, true);
        assert!(received.try_recv().is_err());
        session.model().watch = true;
        session.decide_proposal("thread".into(), "proposal".into(), 7, true);
        assert!(received.try_recv().is_err());
        session.model().watch = false;
        session.model().phase = Phase::Reconnecting;
        session.decide_proposal("thread".into(), "proposal".into(), 7, true);
        assert!(received.try_recv().is_err());
        session.model().phase = Phase::Attached;
        session.decide_proposal("thread".into(), "proposal".into(), 7, true);
        assert!(matches!(
            received.try_recv(),
            Ok(Command::DecideProposal {
                revision: 7,
                approve: true,
                ..
            })
        ));
    }
    #[test]
    fn sharing_mutations_are_refused_offline_and_are_not_replayed() {
        let mut model = Model::new("a".repeat(64), "Scratch host", 24, 80);
        model.phase = Phase::Reconnecting;
        let (commands, mut received) = mpsc::unbounded_channel();
        let session = Session {
            model: Arc::new(Mutex::new(model)),
            commands,
        };
        assert!(
            session
                .owner_sharing(SharingCommand::Pause(true))
                .recv()
                .unwrap()
                .is_err()
        );
        assert!(received.try_recv().is_err());
        session.model().phase = Phase::Attached;
        let reply = session.owner_sharing(SharingCommand::Pause(true));
        let command = received.try_recv().unwrap();
        assert!(matches!(
            command,
            Command::Sharing {
                action: SharingCommand::Pause(true),
                ..
            }
        ));
        drop(command);
        assert!(reply.recv().is_err());
        assert!(received.try_recv().is_err());
    }
}
