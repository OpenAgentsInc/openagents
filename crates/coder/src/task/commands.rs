//! Durable device commands for a task: send, queue, steer, interrupt, and
//! answer.
//!
//! A device mints each command's ID once and replays the same ID, however
//! many NIP-HOST requests carry it, after a crash, a reconnect, or a long
//! time offline. The host keeps one journal per task store, keyed by the
//! sending device and that ID, beside the task document in
//! `commands.json`, and writes it only under a stable command-journal lock.
//!
//! [`decide`] is the pure evaluation, in this order:
//!
//! 1. An entry already decided is skipped: a replay returns its outcome.
//! 2. An entry past its time to live ([`TTL`], from when the device minted
//!    it) expires.
//! 3. A newer interrupt supersedes an older one, and only an older one. An
//!    interrupt never runs, queues, or steers anything, and an interrupt
//!    based on a turn that already ended is superseded.
//! 4. The rest follow the task's state and the engine's stated steering
//!    ([`crate::task::steering`]): a send continues an ended task; a queued
//!    message waits for the turn to end and then continues the task, in
//!    order; a steer corrects a turn that has not started, runs the
//!    engine's emulation for a running turn only when the device chose it,
//!    and is refused otherwise; a steer whose turn ended becomes the next
//!    turn. An answer starts the next turn only while the task's ended turn
//!    waits for one ([`super::interaction`]) and the device read that turn;
//!    the first answer wins, and a later or competing one is refused.
//!
//! The host records a command's exact task-store command before it applies
//! it, and applies those exact bytes again after a crash. The store returns
//! the original receipt for an exact retry, so no command runs twice.
//! Every deferred effect rechecks its sender's grant and epoch first.
//!
//! # Editing the queue
//!
//! [`edit_queue`] lists a task's held messages and edits them under an edit
//! lease ([`LEASE`] seconds, renewed by the holder). While a device holds
//! the lease, queued messages wait even when the turn ends, so nothing runs
//! a message being edited. The holder may change the text of its own held
//! message, remove it, send it now (the engine's emulated steering: stop the
//! turn and continue with it, ahead of the queue), or reorder the held
//! messages by naming their exact permutation. An edit keeps the command's
//! original request, so a device's replay of it still matches; the journal
//! records the edited text beside it.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::steering::{self, Plan, Steering, Turn as SteerTurn};
use super::{
    Action, COMMAND_SCHEMA, Command, Error, Status, Store, Task, private_open, regular_or_absent,
};

/// The journal's schema.
pub const SCHEMA: &str = "openagents.coder.device-commands.v1";
/// The journal's filename within the task store directory.
pub const FILE: &str = "commands.json";
const PENDING: &str = ".commands.pending";
/// How long a command stays live after the device minted it, in seconds.
pub const TTL: u64 = 24 * 60 * 60;
/// How far ahead of the host's clock a device may date a command.
pub const SKEW: u64 = 5 * 60;
/// The most entries the journal keeps. Decided entries older than twice
/// the time to live are pruned first; a replay after that expires anyway.
pub const MAX_ENTRIES: usize = 4096;
/// How long a queue edit lease lasts without renewal, in seconds. A holder
/// renews it well within this, such as every 20 seconds.
pub const LEASE: u64 = 60;
/// The most held messages a queue listing carries.
pub const MAX_LISTED: usize = 64;
const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// What a command asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Send,
    Queue,
    Steer,
    Interrupt,
    Answer,
}

/// A device's command, as the journal keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// The device-minted command ID.
    pub command: String,
    pub task: String,
    pub kind: Kind,
    /// The task revision the device last read.
    pub based_on: u64,
    pub text: String,
    /// For a steer: the device chose the engine's emulation.
    pub emulate: bool,
    /// When the device minted it, in Unix seconds.
    pub issued_at: u64,
}

/// Who sent it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sender {
    pub device: String,
    pub grant: Option<String>,
    pub epoch: Option<u64>,
}

/// Why a command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// The task's state does not take this command now, such as a send
    /// while a turn runs.
    Conflict,
    /// The engine cannot do it, such as native steering it lacks.
    Unsupported,
    /// The task's outcome is unknown, or the store refused.
    Unavailable,
    /// The sender no longer holds `operate` under the same grant and epoch.
    Revoked,
    /// The task moved past the revision the command needs.
    Stale,
    /// The task does not exist.
    Missing,
    /// The command is dated too far ahead, or the task is full.
    Bounds,
}

/// A decided command's outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The effect was applied at this task revision.
    Applied {
        revision: u64,
    },
    Rejected {
        reason: Rejection,
    },
    Expired,
    Superseded,
    /// Its device removed it from the queue before it ran.
    Cancelled,
}

/// Where a command is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    /// Recorded, not yet evaluated.
    Received,
    /// Waiting for the task's turn to end. `priority` is an emulated steer,
    /// which goes before queued messages.
    Held {
        priority: bool,
    },
    /// The exact store command is recorded and may have been applied; it is
    /// applied again, byte for byte, before anything else. `then_hold`
    /// means the entry waits for the turn to end afterwards.
    Dispatching {
        bytes: String,
        then_hold: bool,
    },
    Done(Outcome),
}

/// One journal entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub sender: Sender,
    pub request: Request,
    /// Host time it was first recorded.
    pub received_at: u64,
    pub state: State,
    /// An exact task fence checked when this entry is first evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
    /// Text its device put in place of the request's while it was held. The
    /// request itself stays as sent, so a replay still matches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<String>,
    /// Its device chose to send this held message now: it runs as the
    /// engine's emulated steering, ahead of the queue.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub promoted: bool,
}

impl Entry {
    fn done(&self) -> bool {
        matches!(self.state, State::Done(_))
    }
    fn held(&self) -> Option<bool> {
        match self.state {
            State::Held { priority } => Some(priority),
            _ => None,
        }
    }

    /// The message this entry runs with: the edited text, else the sent one.
    #[must_use]
    pub fn text(&self) -> &str {
        self.edited.as_deref().unwrap_or(&self.request.text)
    }

    /// The request as it is evaluated now: the edited text, and for a
    /// message sent now, an emulated steer.
    fn effective(&self) -> Request {
        let mut request = self.request.clone();
        request.text = self.text().to_owned();
        if self.promoted {
            request.kind = Kind::Steer;
            request.emulate = true;
        }
        request
    }
}

/// A device's edit lease on one task's queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub task: String,
    pub device: String,
    /// Host time it lapses unless renewed.
    pub expires_at: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: String,
    entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    leases: Vec<Lease>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    queue_requests: Vec<QueueReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QueueReceipt {
    request: String,
    sender: Sender,
    task: String,
    revision: u64,
    digest: String,
    edit: QueueEdit,
    received_at: u64,
    /// Absence keeps an interrupted dispatch unknown; it never repeats the edit.
    result: Option<QueueResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct QueueResult {
    queue: coder_host::access::protocol::TaskQueue,
    digest: String,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            entries: Vec::new(),
            leases: Vec::new(),
            queue_requests: Vec::new(),
        }
    }
}

impl Journal {
    /// The current lease on `task`'s queue, if one has not lapsed.
    fn lease(&self, task: &str, now: u64) -> Option<&Lease> {
        self.leases
            .iter()
            .find(|lease| lease.task == task && lease.expires_at > now)
    }
}

/// The task's turn as a command sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Queued and not started.
    Pending,
    Running,
    /// A stop was requested and the run has not ended.
    Stopping,
    /// Finished or cancelled: a new turn may start.
    Ended,
    /// The run's outcome is unknown; nothing starts after it automatically.
    Unknown,
}

/// The task state [`decide`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub phase: Phase,
    pub revision: u64,
    /// The revision the current turn started at.
    pub turn_started: u64,
    /// What the ended turn asked, while nothing has answered it.
    pub question: Option<super::interaction::Kind>,
    /// A device holds the queue's edit lease: queued messages wait.
    pub paused: bool,
    /// The task is archived: nothing continues it.
    pub archived: bool,
}

impl View {
    #[must_use]
    pub fn of(task: &Task) -> Self {
        Self {
            phase: match task.status {
                Status::Queued => Phase::Pending,
                Status::Running => Phase::Running,
                Status::CancelRequested => Phase::Stopping,
                Status::Finished | Status::Cancelled => Phase::Ended,
                Status::Unknown => Phase::Unknown,
            },
            revision: task.revision,
            turn_started: task.turn_started(),
            question: super::interaction::pending(task),
            paused: false,
            archived: false,
        }
    }
}

/// An effect on the task store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Start the next turn with this message.
    Continue(String),
    /// Replace the instructions of a turn that has not started.
    Correct(String),
    /// Stop the current turn.
    Cancel(String),
    /// Stop the current turn, then continue with the message when it ends:
    /// the engine's emulated steering, which the device chose.
    CancelThenContinue(String),
}

/// What evaluation decided for one entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Already decided; nothing to do.
    Skip,
    /// Wait for the turn to end.
    Hold {
        priority: bool,
    },
    Dispatch(Effect),
    Done(Outcome),
}

/// Evaluate one entry against the task's state.
///
/// `newer_interrupt` says whether an undecided interrupt for the same task
/// arrived after this entry; `held_ahead` whether a held entry goes before
/// it; `standing` whether its sender still holds `operate` under the same
/// grant and epoch.
#[must_use]
pub fn decide(
    entry: &Entry,
    view: &View,
    steering: &Steering,
    newer_interrupt: bool,
    held_ahead: bool,
    standing: bool,
    now: u64,
) -> Decision {
    if entry.done() {
        return Decision::Skip;
    }
    let request = &entry.request;
    if now > request.issued_at.saturating_add(TTL) {
        return Decision::Done(Outcome::Expired);
    }
    if request.issued_at > now.saturating_add(SKEW) {
        return rejected(Rejection::Bounds);
    }
    if !standing {
        return rejected(Rejection::Revoked);
    }
    if matches!(entry.state, State::Received)
        && entry
            .expected_revision
            .is_some_and(|expected| expected != view.revision)
    {
        return rejected(Rejection::Stale);
    }
    let request = &entry.effective();
    // An archived task left every list; a waiting message never revives it.
    if view.archived && request.kind != Kind::Interrupt {
        return rejected(Rejection::Conflict);
    }
    let ended_turn = request.based_on < view.turn_started;
    match request.kind {
        Kind::Interrupt => {
            if newer_interrupt || ended_turn {
                return Decision::Done(Outcome::Superseded);
            }
            match view.phase {
                Phase::Pending | Phase::Running => {
                    Decision::Dispatch(Effect::Cancel(request.text.clone()))
                }
                Phase::Stopping | Phase::Ended => Decision::Done(Outcome::Superseded),
                Phase::Unknown => rejected(Rejection::Unavailable),
            }
        }
        Kind::Send => match view.phase {
            Phase::Ended if !held_ahead => {
                Decision::Dispatch(Effect::Continue(request.text.clone()))
            }
            Phase::Unknown => rejected(Rejection::Unavailable),
            _ => rejected(Rejection::Conflict),
        },
        // A queue being edited waits, even after its turn ends.
        Kind::Queue if view.paused => Decision::Hold { priority: false },
        Kind::Queue => continue_or_hold(view, held_ahead, false, &request.text),
        Kind::Steer => {
            let turn = match view.phase {
                Phase::Unknown => return rejected(Rejection::Unavailable),
                _ if ended_turn => SteerTurn::Ended,
                Phase::Ended => SteerTurn::Ended,
                // A turn that has not started takes new instructions at its
                // boundary, which is where every engine reads them.
                Phase::Pending => {
                    return Decision::Dispatch(Effect::Correct(request.text.clone()));
                }
                Phase::Running | Phase::Stopping => SteerTurn::Running,
            };
            let asked = if request.emulate {
                steering::Request::Emulated
            } else {
                steering::Request::Native
            };
            match steering.admit(turn, asked) {
                Ok(Plan::NewTurn) => continue_or_hold(view, held_ahead, true, &request.text),
                Ok(Plan::CancelAndContinue) if view.phase == Phase::Stopping => {
                    Decision::Hold { priority: true }
                }
                Ok(Plan::CancelAndContinue) => {
                    Decision::Dispatch(Effect::CancelThenContinue(request.text.clone()))
                }
                // A mid-turn engine takes the correction in its running turn.
                Ok(Plan::Deliver) => Decision::Dispatch(Effect::Correct(request.text.clone())),
                Err(_) => rejected(Rejection::Unsupported),
            }
        }
        // An answer needs a question the device has read. The first answer
        // continues the task, so a later or competing one finds no
        // question waiting.
        Kind::Answer => match view.phase {
            Phase::Unknown => rejected(Rejection::Unavailable),
            _ if ended_turn => rejected(Rejection::Stale),
            Phase::Ended if view.question.is_some() && !held_ahead => {
                Decision::Dispatch(Effect::Continue(request.text.clone()))
            }
            _ => rejected(Rejection::Conflict),
        },
    }
}

fn rejected(reason: Rejection) -> Decision {
    Decision::Done(Outcome::Rejected { reason })
}

fn continue_or_hold(view: &View, held_ahead: bool, priority: bool, text: &str) -> Decision {
    match view.phase {
        Phase::Ended if !held_ahead => Decision::Dispatch(Effect::Continue(text.to_owned())),
        _ => Decision::Hold { priority },
    }
}

/// What recording a command returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    pub state: State,
    /// The task after evaluation.
    pub task: Option<Task>,
}

/// Evaluate every undecided entry for `task` in the store at `dir`, in
/// arrival order, and apply what they decide. `standing` rechecks each
/// sender. Returns the IDs of tasks that continued into a new turn, with
/// the sender that continued them and the new turn.
///
/// # Errors
/// Store and journal I/O failures.
pub fn process(
    dir: &Path,
    task: &str,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<Vec<Continued>, Error> {
    let mut store = Store::open(dir)?;
    let _journal = journal_lock(&store)?;
    let mut journal = read(&store.dir)?;
    let continued = evaluate(
        &mut store,
        &mut journal,
        task,
        steering,
        standing,
        now,
        None,
    )?;
    Ok(continued)
}

/// A task a command continued into a new turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continued {
    pub task: String,
    /// The task revision the new turn started at.
    pub turn: u64,
    pub device: String,
}

/// Record a device's command, then evaluate the task's commands. A replay
/// of a recorded command with the same content returns its current state
/// without evaluating it again; different content under the same ID is a
/// conflict.
///
/// # Errors
/// `Conflict` for a reused ID, `LimitExceeded` for a full journal, and I/O
/// failures.
pub fn record(
    dir: &Path,
    sender: &Sender,
    request: &Request,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<(Recorded, Vec<Continued>), Error> {
    record_inner(dir, sender, request, None, steering, standing, now)
}

/// Record a command under the same lock that applies its exact task revision.
pub fn record_at_revision(
    dir: &Path,
    sender: &Sender,
    request: &Request,
    revision: u64,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<(Recorded, Vec<Continued>), Error> {
    if request.based_on != revision {
        return Err(Error::Conflict);
    }
    record_inner(
        dir,
        sender,
        request,
        Some(revision),
        steering,
        standing,
        now,
    )
}

fn record_inner(
    dir: &Path,
    sender: &Sender,
    request: &Request,
    expected: Option<u64>,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<(Recorded, Vec<Continued>), Error> {
    let mut store = Store::open(dir)?;
    let _journal = journal_lock(&store)?;
    let mut journal = read(&store.dir)?;
    let existing = journal.entries.iter().find(|entry| {
        entry.sender.device == sender.device && entry.request.command == request.command
    });
    if let Some(entry) = existing {
        if entry.request != *request
            || entry.expected_revision != expected
            || entry.sender != *sender
        {
            return Err(Error::Conflict);
        }
        let state = entry.state.clone();
        return Ok((
            Recorded {
                state,
                task: store.show(&request.task).ok(),
            },
            Vec::new(),
        ));
    }
    if store.show(&request.task).is_err() {
        return Err(Error::NotFound);
    }
    // A run whose process is gone is ended before the command is read
    // (#10124): a stop then finds it stopped, and a message continues it.
    let guard = expected
        .map(|_| store.task_guard(&request.task))
        .transpose()?;
    if expected.is_some_and(|expected| {
        store
            .show(&request.task)
            .map_or(true, |task| task.revision != expected)
    }) {
        return Err(Error::RevisionMismatch);
    }
    let settled = if guard.is_none() {
        store.settle(&request.task)?
    } else {
        None
    };
    prune(&mut journal, now);
    if journal.entries.len() >= MAX_ENTRIES {
        return Err(Error::LimitExceeded);
    }
    journal.entries.push(Entry {
        sender: sender.clone(),
        request: request.clone(),
        received_at: now,
        state: State::Received,
        expected_revision: expected,
        edited: None,
        promoted: false,
    });
    // An interrupt that found its run's process gone is what ended it.
    if request.kind == Kind::Interrupt
        && let Some(task) = &settled
        && let Some(entry) = journal.entries.last_mut()
    {
        entry.state = State::Done(Outcome::Applied {
            revision: task.revision,
        });
    }
    // The command is durable before anything evaluates it.
    write(&store.dir, &journal)?;
    let continued = evaluate(
        &mut store,
        &mut journal,
        &request.task,
        steering,
        standing,
        now,
        guard.as_ref(),
    )?;
    let state = journal
        .entries
        .iter()
        .find(|entry| {
            entry.sender.device == sender.device && entry.request.command == request.command
        })
        .map(|entry| entry.state.clone())
        .unwrap_or(State::Received);
    Ok((
        Recorded {
            state,
            task: store.show(&request.task).ok(),
        },
        continued,
    ))
}

/// The tasks with undecided commands in the store at `dir`.
#[must_use]
pub fn open_tasks(dir: &Path) -> Vec<String> {
    let Ok(journal) = read(dir) else {
        return Vec::new();
    };
    let mut tasks: Vec<String> = journal
        .entries
        .iter()
        .filter(|entry| !entry.done())
        .map(|entry| entry.request.task.clone())
        .collect();
    tasks.sort();
    tasks.dedup();
    tasks
}

/// Every entry in the journal, for inspection.
///
/// # Errors
/// A malformed journal and I/O failures other than absence.
pub fn entries(dir: &Path) -> Result<Vec<Entry>, Error> {
    Ok(read(dir)?.entries)
}

/// One change to a task's queue.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueueEdit {
    /// Read the queue.
    List,
    /// Take or renew the edit lease.
    Lease,
    /// Give the lease up.
    Release,
    /// Replace the text of the device's own held message.
    Edit { command: String, text: String },
    /// Remove the device's own held message.
    Remove { command: String },
    /// Put the held queued messages in this exact order.
    Reorder { commands: Vec<String> },
    /// Send the device's own held message now, as emulated steering.
    SendNow { command: String },
}

/// One held message, as a queue listing shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub command: String,
    pub device: String,
    /// The message the entry runs with.
    pub text: String,
    /// It goes before queued messages: an emulated steer waiting for its
    /// stop, or a message sent now.
    pub priority: bool,
}

/// A task's queue after a [`QueueEdit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueState {
    pub task: Task,
    pub lease: Option<Lease>,
    /// Held messages in the order they run: priority first, then queued,
    /// each in journal order.
    pub items: Vec<Queued>,
}

/// List or edit `task`'s held messages for `sender`. Changes need the edit
/// lease, and a device changes only its own messages, except that the
/// lease holder may reorder every queued message. After a change the
/// task's commands are evaluated again.
///
/// # Errors
/// `NotFound` for an unknown task or a command this device has not queued,
/// `Conflict` when another device holds the lease, this device does not,
/// the message is no longer held, or a reorder is not an exact permutation
/// of the queued messages; and I/O failures.
pub fn edit_queue(
    dir: &Path,
    task: &str,
    sender: &Sender,
    edit: &QueueEdit,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<(QueueState, Vec<Continued>), Error> {
    let mut store = Store::open(dir)?;
    store.show(task)?;
    let _journal = journal_lock(&store)?;
    let mut journal = read(&store.dir)?;
    edit_queue_inner(
        &mut store,
        &mut journal,
        task,
        sender,
        edit,
        steering,
        standing,
        now,
        None,
    )
}

fn edit_queue_inner(
    store: &mut Store,
    journal: &mut Journal,
    task: &str,
    sender: &Sender,
    edit: &QueueEdit,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
    guard: Option<&super::TaskWriteGuard>,
) -> Result<(QueueState, Vec<Continued>), Error> {
    journal.leases.retain(|lease| lease.expires_at > now);
    let holder = journal.lease(task, now).map(|lease| lease.device.clone());
    let mine = holder.as_deref() == Some(sender.device.as_str());
    let own = |journal: &Journal, command: &str| {
        journal
            .entries
            .iter()
            .position(|entry| {
                entry.request.task == task
                    && entry.sender.device == sender.device
                    && entry.request.command == command
            })
            .ok_or(Error::NotFound)
    };
    let queued = |journal: &Journal| -> Vec<usize> {
        journal
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.request.task == task && !entry.promoted && entry.held() == Some(false)
            })
            .map(|(index, _)| index)
            .collect()
    };
    let changed = match edit {
        QueueEdit::List => false,
        QueueEdit::Lease => {
            if holder
                .as_ref()
                .is_some_and(|device| *device != sender.device)
            {
                return Err(Error::Conflict);
            }
            journal.leases.retain(|lease| lease.task != task);
            journal.leases.push(Lease {
                task: task.into(),
                device: sender.device.clone(),
                expires_at: now + LEASE,
            });
            true
        }
        QueueEdit::Release => {
            let before = journal.leases.len();
            journal
                .leases
                .retain(|lease| !(lease.task == task && lease.device == sender.device));
            journal.leases.len() != before
        }
        _ if !mine => return Err(Error::Conflict),
        QueueEdit::Edit { command, text } => {
            let index = own(journal, command)?;
            let entry = &mut journal.entries[index];
            if entry.held().is_none() {
                return Err(Error::Conflict);
            }
            entry.edited = (*text != entry.request.text).then(|| text.clone());
            true
        }
        QueueEdit::Remove { command } => {
            let index = own(journal, command)?;
            let entry = &mut journal.entries[index];
            match entry.state {
                // Removing it again succeeds again.
                State::Done(Outcome::Cancelled) => false,
                State::Held { .. } => {
                    entry.state = State::Done(Outcome::Cancelled);
                    true
                }
                _ => return Err(Error::Conflict),
            }
        }
        QueueEdit::SendNow { command } => {
            let index = own(journal, command)?;
            let entry = &mut journal.entries[index];
            match entry.held() {
                Some(_) if entry.promoted => false,
                Some(false) => {
                    entry.promoted = true;
                    true
                }
                _ => return Err(Error::Conflict),
            }
        }
        QueueEdit::Reorder { commands } => {
            let slots = queued(journal);
            let mut order = Vec::with_capacity(commands.len());
            for command in commands {
                let found = slots
                    .iter()
                    .copied()
                    .find(|&slot| journal.entries[slot].request.command == *command);
                match found {
                    Some(slot) if !order.contains(&slot) => order.push(slot),
                    _ => return Err(Error::Conflict),
                }
            }
            if order.len() != slots.len() {
                return Err(Error::Conflict);
            }
            // The queued messages trade places among their own slots, so
            // every other entry keeps its arrival order.
            let moved: Vec<Entry> = order
                .iter()
                .map(|&slot| journal.entries[slot].clone())
                .collect();
            for (slot, entry) in slots.iter().zip(moved) {
                journal.entries[*slot] = entry;
            }
            order != slots
        }
    };
    let mut continued = Vec::new();
    if changed {
        write(&store.dir, journal)?;
        continued = evaluate(store, journal, task, steering, standing, now, guard)?;
    }
    let mut items: Vec<(bool, usize)> = journal
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.request.task == task)
        .filter_map(|(index, entry)| entry.held().map(|priority| (!priority, index)))
        .collect();
    items.sort_unstable();
    items.truncate(MAX_LISTED);
    let items = items
        .into_iter()
        .map(|(queued, index)| {
            let entry = &journal.entries[index];
            Queued {
                command: entry.request.command.clone(),
                device: entry.sender.device.clone(),
                text: entry.text().to_owned(),
                priority: !queued,
            }
        })
        .collect();
    Ok((
        QueueState {
            task: store.show(task)?,
            lease: journal.lease(task, now).cloned(),
            items,
        },
        continued,
    ))
}

/// Read an exact queue or dispatch one edit once under both native locks.
/// A saved request without a result stays unknown after a crash.
pub fn edit_queue_at_revision(
    dir: &Path,
    request: &str,
    task: &str,
    sender: &Sender,
    revision: u64,
    edit: &QueueEdit,
    expected_digest: Option<&str>,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<
    (
        coder_host::access::protocol::TaskQueue,
        String,
        Vec<Continued>,
        bool,
    ),
    Error,
> {
    if !standing(sender) {
        return Err(Error::Conflict);
    }
    let mut store = Store::open(dir)?;
    let _journal = journal_lock(&store)?;
    let mut journal = read(&store.dir)?;
    let is_read = matches!(edit, QueueEdit::List);
    if !is_read {
        if let Some(record) = journal
            .queue_requests
            .iter()
            .find(|record| record.request == request)
        {
            if record.sender != *sender
                || record.task != task
                || record.revision != revision
                || record.edit != *edit
                || Some(record.digest.as_str()) != expected_digest
            {
                return Err(Error::Conflict);
            }
            super::sync_directory(&store.dir)?;
            let result = record.result.clone().ok_or(Error::ReopenRequired)?;
            return Ok((result.queue, result.digest, Vec::new(), false));
        }
    }
    let guard = store.task_guard(task)?;
    let before = store.show(task)?;
    if before.revision != revision {
        return Err(Error::RevisionMismatch);
    }
    let digest = queue_digest(&journal, task, revision, now)?;
    if expected_digest.is_some_and(|expected| expected != digest) {
        return Err(Error::RevisionMismatch);
    }
    if !is_read && expected_digest.is_none() {
        return Err(Error::Conflict);
    }
    let held = journal
        .entries
        .iter()
        .filter(|entry| entry.request.task == task && entry.held().is_some())
        .count();
    if held > MAX_LISTED {
        return Err(Error::LimitExceeded);
    }
    if !is_read {
        journal
            .queue_requests
            .retain(|record| now.saturating_sub(record.received_at) <= 2 * TTL);
        if journal.queue_requests.len() >= 128 {
            return Err(Error::LimitExceeded);
        }
        journal.queue_requests.push(QueueReceipt {
            request: request.into(),
            sender: sender.clone(),
            task: task.into(),
            revision,
            digest,
            edit: edit.clone(),
            received_at: now,
            result: None,
        });
        // Admission is durable before an edit or task transition can happen.
        write(&store.dir, &journal)?;
    }
    let (state, continued) = edit_queue_inner(
        &mut store,
        &mut journal,
        task,
        sender,
        edit,
        steering,
        standing,
        now,
        Some(&guard),
    )?;
    let changed = before.revision != state.task.revision;
    let queue = public_queue(state, sender);
    let digest = queue_digest(&journal, task, queue.revision, now)?;
    let outcome = coder_host::access::protocol::Outcome::QueueAtRevision {
        queue: queue.clone(),
        revision: queue.revision,
        queue_digest: digest.clone(),
    };
    outcome.validate().map_err(|_| {
        if is_read {
            Error::LimitExceeded
        } else {
            Error::ReopenRequired
        }
    })?;
    if !is_read {
        let record = journal
            .queue_requests
            .iter_mut()
            .find(|record| record.request == request)
            .ok_or(Error::Corrupt("the queue request lost its record"))?;
        record.result = Some(QueueResult {
            queue: queue.clone(),
            digest: digest.clone(),
        });
        write(&store.dir, &journal)?;
    }
    Ok((queue, digest, continued, changed))
}

fn public_queue(state: QueueState, sender: &Sender) -> coder_host::access::protocol::TaskQueue {
    use coder_host::access::protocol::{QueueItem, QueueLease, TaskQueue};
    TaskQueue {
        task: state.task.task_id,
        revision: state.task.revision,
        lease: state.lease.map(|lease| QueueLease {
            device: lease.device,
            expires_at: lease.expires_at,
        }),
        items: state
            .items
            .into_iter()
            .map(|item| QueueItem {
                text: (item.device == sender.device).then_some(item.text),
                command: item.command,
                device: item.device,
                priority: item.priority,
            })
            .collect(),
    }
}

fn queue_digest(journal: &Journal, task: &str, revision: u64, now: u64) -> Result<String, Error> {
    let entries: Vec<&Entry> = journal
        .entries
        .iter()
        .filter(|entry| entry.request.task == task && entry.held().is_some())
        .collect();
    let bytes = serde_json::to_vec(&(task, revision, journal.lease(task, now), entries))
        .map_err(|_| Error::Corrupt("the queue snapshot could not be encoded"))?;
    Ok(nostr::contracts::digest_bytes(&bytes))
}

fn evaluate(
    store: &mut Store,
    journal: &mut Journal,
    task: &str,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
    guard: Option<&super::TaskWriteGuard>,
) -> Result<Vec<Continued>, Error> {
    let mut continued = Vec::new();
    let archived = super::archive::archived(&store.dir).contains(task);
    // Each pass decides at most one entry's effect on the task, then reads
    // the task again, so a later entry sees the state an earlier one made.
    for _ in 0..journal.entries.len() + 1 {
        let Ok(current) = store.show(task) else {
            break;
        };
        let view = View {
            paused: journal.lease(task, now).is_some(),
            archived,
            ..View::of(&current)
        };
        let indices: Vec<usize> = journal
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.request.task == task && !entry.done())
            .map(|(index, _)| index)
            .collect();
        // A recorded dispatch is finished before anything else.
        if let Some(&index) = indices
            .iter()
            .find(|&&index| matches!(journal.entries[index].state, State::Dispatching { .. }))
        {
            let entry = &journal.entries[index];
            let State::Dispatching { bytes, .. } = &entry.state else {
                unreachable!()
            };
            let accepted = store.read_task(task)?.is_some_and(|file| {
                file.commands
                    .iter()
                    .any(|accepted| accepted.request == *bytes)
            });
            if !accepted
                && (!standing(&entry.sender) || now > entry.request.issued_at.saturating_add(TTL))
            {
                journal.entries[index].state =
                    State::Done(if now > entry.request.issued_at.saturating_add(TTL) {
                        Outcome::Expired
                    } else {
                        Outcome::Rejected {
                            reason: Rejection::Revoked,
                        }
                    });
                write(&store.dir, journal)?;
                continue;
            }
            finish_dispatch(store, journal, index, &mut continued, guard)?;
            continue;
        }
        let mut changed = false;
        for (position, &index) in indices.iter().enumerate() {
            let entry = &journal.entries[index];
            let later = &indices[position + 1..];
            let newer_interrupt = later
                .iter()
                .any(|&other| journal.entries[other].request.kind == Kind::Interrupt);
            let priority = entry.promoted || entry.request.kind == Kind::Steer;
            // An emulated steer goes before every queued message; within
            // each kind, arrival order holds.
            let held_ahead = indices.iter().any(|&other| {
                other != index
                    && journal.entries[other].held().is_some_and(|held_priority| {
                        if priority {
                            held_priority && other < index
                        } else {
                            held_priority || other < index
                        }
                    })
            });
            let decision = decide(
                entry,
                &view,
                steering,
                newer_interrupt,
                held_ahead,
                standing(&entry.sender),
                now,
            );
            match decision {
                Decision::Skip => {}
                Decision::Hold { priority } => {
                    if entry.state != (State::Held { priority }) {
                        journal.entries[index].state = State::Held { priority };
                        write(&store.dir, journal)?;
                    }
                }
                Decision::Done(outcome) => {
                    journal.entries[index].state = State::Done(outcome);
                    write(&store.dir, journal)?;
                }
                Decision::Dispatch(effect) => {
                    let (action, then_hold, suffix) = match effect {
                        Effect::Continue(prompt) => (Action::Continue { prompt }, false, ""),
                        Effect::Correct(prompt) => (
                            Action::Correct {
                                prompt,
                                reason: "Steered from a paired device".into(),
                            },
                            false,
                            "",
                        ),
                        Effect::Cancel(reason) => (Action::Cancel { reason }, false, ""),
                        Effect::CancelThenContinue(_) => (
                            Action::Cancel {
                                reason: "Stopped to continue with a steer".into(),
                            },
                            true,
                            "-stop",
                        ),
                    };
                    let command = Command {
                        schema: COMMAND_SCHEMA.into(),
                        command_id: store_command_id(&journal.entries[index], suffix),
                        task_id: task.into(),
                        expected_revision: Some(view.revision),
                        action,
                    };
                    let bytes = serde_json::to_string(&command)
                        .map_err(|_| Error::InvalidCommand("the command could not be encoded"))?;
                    // Mark it before applying it: a crash between the two
                    // applies the same bytes again, never new ones.
                    journal.entries[index].state = State::Dispatching { bytes, then_hold };
                    write(&store.dir, journal)?;
                    finish_dispatch(store, journal, index, &mut continued, guard)?;
                    changed = true;
                }
            }
            if changed {
                break;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(continued)
}

/// The task-store command ID for a journal entry: bounded, and distinct
/// per device and command.
fn store_command_id(entry: &Entry, suffix: &str) -> String {
    let key = format!("{}:{}", entry.sender.device, entry.request.command);
    let digest = nostr::contracts::digest_bytes(key.as_bytes());
    format!(
        "device-{}{suffix}",
        &digest.trim_start_matches("sha256:")[..48]
    )
}

fn finish_dispatch(
    store: &mut Store,
    journal: &mut Journal,
    index: usize,
    continued: &mut Vec<Continued>,
    guard: Option<&super::TaskWriteGuard>,
) -> Result<(), Error> {
    let State::Dispatching { bytes, then_hold } = journal.entries[index].state.clone() else {
        return Ok(());
    };
    let applied = match guard {
        Some(guard) => store.apply_locked(bytes.as_bytes(), guard),
        None => store.apply(bytes.as_bytes()),
    };
    let next = match applied {
        Ok(receipt) => {
            let command: Command = serde_json::from_str(&bytes)
                .map_err(|_| Error::Corrupt("a recorded device command is invalid"))?;
            if matches!(command.action, Action::Continue { .. }) {
                let turn = receipt.revision;
                continued.push(Continued {
                    task: command.task_id,
                    turn,
                    device: journal.entries[index].sender.device.clone(),
                });
            }
            if then_hold {
                State::Held { priority: true }
            } else {
                State::Done(Outcome::Applied {
                    revision: receipt.revision,
                })
            }
        }
        Err(Error::RevisionMismatch) => State::Done(Outcome::Rejected {
            reason: Rejection::Stale,
        }),
        Err(Error::InvalidTransition | Error::Conflict) => State::Done(Outcome::Rejected {
            reason: Rejection::Conflict,
        }),
        Err(Error::NotFound) => State::Done(Outcome::Rejected {
            reason: Rejection::Missing,
        }),
        Err(Error::LimitExceeded) => State::Done(Outcome::Rejected {
            reason: Rejection::Bounds,
        }),
        // An uncertain store failure keeps the recorded bytes for the next
        // evaluation.
        Err(error) => return Err(error),
    };
    journal.entries[index].state = next;
    write(&store.dir, journal)
}

/// Drop decided entries older than twice the time to live. A replay of one
/// is evaluated afresh and expires.
fn prune(journal: &mut Journal, now: u64) {
    journal
        .entries
        .retain(|entry| !entry.done() || now.saturating_sub(entry.received_at) <= 2 * TTL);
}

fn journal_lock(store: &Store) -> Result<std::fs::File, Error> {
    let path = store.dir.join("commands.lock");
    let file = super::open_lock(&path)?;
    super::take_lock(&file, store.wait)?;
    super::verify_same_file(&path, &file)?;
    Ok(file)
}

fn read(dir: &Path) -> Result<Journal, Error> {
    let path = dir.join(FILE);
    if !regular_or_absent(&path)? {
        return Ok(Journal::default());
    }
    let mut bytes = Vec::new();
    private_open(&path, false, false)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::LimitExceeded);
    }
    let journal: Journal = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Corrupt("the device command journal is malformed"))?;
    if journal.schema != SCHEMA {
        return Err(Error::UnsupportedSchema);
    }
    Ok(journal)
}

fn write(dir: &Path, journal: &Journal) -> Result<(), Error> {
    let bytes = serde_json::to_vec(journal)
        .map_err(|_| Error::Corrupt("the device command journal could not be encoded"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::LimitExceeded);
    }
    let pending = dir.join(PENDING);
    if regular_or_absent(&pending)? {
        std::fs::remove_file(&pending)?;
    }
    let result = (|| {
        let mut file = private_open(&pending, true, true)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        regular_or_absent(&dir.join(FILE))?;
        std::fs::rename(&pending, dir.join(FILE))?;
        super::sync_directory(dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result
}

/// The device-visible label of a task's workspace, from the host's map of
/// labels to roots.
#[must_use]
pub fn label_for<'a>(
    workspaces: &'a BTreeMap<String, std::path::PathBuf>,
    task: &Task,
) -> Option<&'a str> {
    workspaces
        .iter()
        .find(|(_, root)| root.to_string_lossy() == task.intent.workspace.path)
        .map(|(label, _)| label.as_str())
}

#[cfg(test)]
mod tests;
