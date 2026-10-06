//! One NIP-TERM operation, whichever transport carried it.

use std::sync::Arc;

use coder_pty::host::FrameSink;
use coder_pty::wire::{Reason, Refusal, TerminalResult};

use super::Shared;
use crate::authority::Standing;
use crate::message::TermRequest;
use crate::unix_time;

/// Run one terminal operation for `principal`. The terminal host asks the
/// grant store for the right each operation needs; a revoked device is told
/// `revoked` rather than `not_admitted`. A device without a current grant
/// reaches the terminal host only while it holds a terminal share, which
/// the terminal host checks per operation and terminal. `sink` builds the frame sink an
/// attach delivers through. Blocking: run it off the async workers.
pub(crate) fn run(
    shared: &Arc<Shared>,
    principal: &str,
    request: &TermRequest,
    sink: impl FnOnce() -> Box<dyn FrameSink>,
) -> TerminalResult {
    let id = request.request().to_owned();
    let now = unix_time().unwrap_or(0);
    match shared.authority.standing(principal, now) {
        Standing::Active(_) => {}
        Standing::Revoked => {
            return TerminalResult::from_outcome(
                id,
                Err(Refusal::new(
                    Reason::Revoked,
                    "this device's grant is revoked",
                )),
            );
        }
        Standing::Expired | Standing::Unknown if shared.pty.shared_with(principal) => {}
        Standing::Expired | Standing::Unknown => {
            return TerminalResult::from_outcome(
                id,
                Err(Refusal::new(
                    Reason::NotAdmitted,
                    "this device holds no current grant",
                )),
            );
        }
    }
    if let Some(reference) = task_reference(request)
        && reference.generation == shared.pty.generation()
    {
        let bound = shared
            .task_terminals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&reference.terminal)
            .cloned();
        if let Some((task, admitted)) = bound {
            let current = shared.tasks.terminal_binding(&task);
            if !current.is_ok_and(|binding| {
                binding.interactive
                    && binding.directory.canonicalize().ok().as_ref() == Some(&admitted.directory)
            }) {
                return TerminalResult::from_outcome(
                    id,
                    Err(Refusal::new(
                        Reason::NotAdmitted,
                        "The task no longer admits this terminal binding.",
                    )),
                );
            }
        }
    }
    let pty = &shared.pty;
    if let Some(outcome) = session(shared, principal, request, now) {
        return TerminalResult::from_outcome(id, outcome);
    }
    let mut outcome = match request {
        TermRequest::Proposal(r) => pty.proposal(principal, r),
        TermRequest::Open(r) => pty.open(principal, r),
        TermRequest::Attach(r) => pty.attach(principal, r, sink()),
        TermRequest::Detach(r) => pty.detach(principal, r),
        TermRequest::Input(r) => pty.input(principal, r),
        TermRequest::Resize(r) => pty.resize(principal, r),
        TermRequest::Signal(r) => pty.signal(principal, r),
        TermRequest::Close(r) => pty.close(principal, r),
        TermRequest::History(r) => pty.history(principal, r),
        TermRequest::BlockPage(r) => pty.block_page(principal, r),
        TermRequest::Seat(r) => pty.seat(principal, r),
        TermRequest::Share(r) => pty.share(principal, r),
        TermRequest::Unshare(r) => pty.unshare(principal, r),
        TermRequest::SharePause(r) => pty.pause(principal, r),
        TermRequest::Handoff(r) => pty.hand_off(principal, r),
        TermRequest::Viewers(r) => pty.viewers(principal, r),
        TermRequest::SessionRead(_)
        | TermRequest::SessionWrite(_)
        | TermRequest::SessionList(_)
        | TermRequest::SessionRemove(_) => unreachable!("answered above"),
    };
    if let Ok((_, coder_pty::wire::Value::Attached { task_binding, .. })) = &mut outcome
        && let Some(reference) = task_reference(request)
        && let Some((task, binding)) = shared
            .task_terminals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&reference.terminal)
    {
        *task_binding = Some(coder_pty::wire::TaskBinding {
            task: task.clone(),
            directory: binding.directory.to_string_lossy().into_owned(),
            mode: "interactive".into(),
        });
    }
    TerminalResult::from_outcome(id, outcome)
}

/// Answers a session-record request: it needs the `terminal` right, and a
/// share never reaches it. Reading a session opens nothing; each terminal
/// member reads as the terminal host knows it now.
fn session(
    shared: &Arc<Shared>,
    principal: &str,
    request: &TermRequest,
    now: u64,
) -> Option<coder_pty::host::Outcome> {
    use coder_pty::ext::{Features, MemberState};
    let features = Features {
        sessions: true,
        ..shared.pty.features()
    };
    let checked = match request {
        TermRequest::SessionRead(r) => r.check_with(features),
        TermRequest::SessionWrite(r) => r.check_with(features),
        TermRequest::SessionList(r) => r.check_with(features),
        TermRequest::SessionRemove(r) => r.check_with(features),
        _ => return None,
    };
    if let Err(refusal) = checked {
        return Some(Err(refusal));
    }
    if !shared
        .authority
        .holds(principal, coder_access::Right::Terminal, now)
    {
        return Some(Err(Refusal::new(
            Reason::NotAdmitted,
            "session records need the terminal right",
        )));
    }
    let pty = &shared.pty;
    let state = |terminal: &coder_pty::wire::TerminalRef| {
        if terminal.generation != pty.generation() {
            return MemberState::Lost;
        }
        match pty.head(terminal) {
            Ok((_, true)) => MemberState::Live,
            Err(refusal) if refusal.reason == Reason::Lost => MemberState::Lost,
            Ok((_, false)) | Err(_) => MemberState::Closed,
        }
    };
    let books = &shared.sessions;
    Some(match request {
        TermRequest::SessionRead(r) => books.read(r, state),
        TermRequest::SessionWrite(r) => books.write(principal, r, state),
        TermRequest::SessionList(r) => books.list(r),
        TermRequest::SessionRemove(r) => books.remove(principal, r),
        _ => return None,
    })
}

/// Cleanup remains available after a task loses its binding.
fn task_reference(request: &TermRequest) -> Option<&coder_pty::wire::TerminalRef> {
    match request {
        TermRequest::Proposal(r) => Some(&r.terminal),
        TermRequest::Attach(r) => Some(&r.terminal),
        TermRequest::Input(r) => Some(&r.terminal),
        TermRequest::Resize(r) => Some(&r.terminal),
        TermRequest::Signal(r) => Some(&r.terminal),
        TermRequest::History(r) => Some(&r.terminal),
        TermRequest::BlockPage(r) => Some(&r.terminal),
        TermRequest::Seat(r) => Some(&r.terminal),
        TermRequest::Share(r) => Some(&r.terminal),
        TermRequest::Unshare(r) => Some(&r.terminal),
        TermRequest::SharePause(r) => Some(&r.terminal),
        TermRequest::Handoff(r) => Some(&r.terminal),
        TermRequest::Viewers(r) => Some(&r.terminal),
        TermRequest::Open(_)
        | TermRequest::Close(_)
        | TermRequest::Detach(_)
        | TermRequest::SessionRead(_)
        | TermRequest::SessionWrite(_)
        | TermRequest::SessionList(_)
        | TermRequest::SessionRemove(_) => None,
    }
}
