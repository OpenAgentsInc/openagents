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
/// `revoked` rather than `not_admitted`. `sink` builds the frame sink an
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
    let pty = &shared.pty;
    let outcome = match request {
        TermRequest::Open(r) => pty.open(principal, r),
        TermRequest::Attach(r) => pty.attach(principal, r, sink()),
        TermRequest::Detach(r) => pty.detach(principal, r),
        TermRequest::Input(r) => pty.input(principal, r),
        TermRequest::Resize(r) => pty.resize(principal, r),
        TermRequest::Signal(r) => pty.signal(principal, r),
        TermRequest::Close(r) => pty.close(principal, r),
        TermRequest::History(r) => pty.history(principal, r),
    };
    TerminalResult::from_outcome(id, outcome)
}
