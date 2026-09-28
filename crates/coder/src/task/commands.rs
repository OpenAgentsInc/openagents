//! Durable device commands for a task: send, queue, steer, interrupt, and
//! answer.
//!
//! A device mints each command's ID once and replays the same ID, however
//! many NIP-HOST requests carry it, after a crash, a reconnect, or a long
//! time offline. The host keeps one journal per task store, keyed by the
//! sending device and that ID, beside the task document in
//! `commands.json`, and writes it only under the task store's lock.
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
//!    turn. An answer is refused: this engine raises no approvals or
//!    questions a device can answer yet.
//!
//! The host records a command's exact task-store command before it applies
//! it, and applies those exact bytes again after a crash. The store returns
//! the original receipt for an exact retry, so no command runs twice.
//! Every deferred effect rechecks its sender's grant and epoch first.

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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: String,
    entries: Vec<Entry>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            entries: Vec::new(),
        }
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
        Kind::Answer => rejected(Rejection::Unsupported),
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
    let mut journal = read(&store.dir)?;
    let continued = evaluate(&mut store, &mut journal, task, steering, standing, now)?;
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
    let mut store = Store::open(dir)?;
    let mut journal = read(&store.dir)?;
    let existing = journal.entries.iter().find(|entry| {
        entry.sender.device == sender.device && entry.request.command == request.command
    });
    if let Some(entry) = existing {
        if entry.request != *request {
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
    prune(&mut journal, now);
    if journal.entries.len() >= MAX_ENTRIES {
        return Err(Error::LimitExceeded);
    }
    journal.entries.push(Entry {
        sender: sender.clone(),
        request: request.clone(),
        received_at: now,
        state: State::Received,
    });
    // The command is durable before anything evaluates it.
    write(&store.dir, &journal)?;
    let continued = evaluate(
        &mut store,
        &mut journal,
        &request.task,
        steering,
        standing,
        now,
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

fn evaluate(
    store: &mut Store,
    journal: &mut Journal,
    task: &str,
    steering: &Steering,
    standing: &dyn Fn(&Sender) -> bool,
    now: u64,
) -> Result<Vec<Continued>, Error> {
    let mut continued = Vec::new();
    // Each pass decides at most one entry's effect on the task, then reads
    // the task again, so a later entry sees the state an earlier one made.
    for _ in 0..journal.entries.len() + 1 {
        let Ok(current) = store.show(task) else {
            break;
        };
        let view = View::of(&current);
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
            finish_dispatch(store, journal, index, &mut continued)?;
            continue;
        }
        let mut changed = false;
        for (position, &index) in indices.iter().enumerate() {
            let entry = &journal.entries[index];
            let later = &indices[position + 1..];
            let newer_interrupt = later
                .iter()
                .any(|&other| journal.entries[other].request.kind == Kind::Interrupt);
            let priority = entry.request.kind == Kind::Steer;
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
                                reason: "Steered by an enrolled device".into(),
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
                    finish_dispatch(store, journal, index, &mut continued)?;
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
) -> Result<(), Error> {
    let State::Dispatching { bytes, then_hold } = journal.entries[index].state.clone() else {
        return Ok(());
    };
    let next = match store.apply(bytes.as_bytes()) {
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
        std::fs::File::open(dir)?.sync_all()?;
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
