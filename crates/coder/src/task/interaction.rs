//! Questions and approvals an engine raises to the person who sent the task.
//!
//! An engine that cannot go on without the person ends its turn with a
//! question, or with a request to approve a step before it takes it. The
//! turn's reply is the question; the run's result ending names the kind.
//! The task then waits: its summary reports [`nostr::activity_summary::Phase::Waiting`]
//! with input or approval attention, and a device's `answer` command
//! (NIP-HOST `task.command`) starts the next turn with the answer.
//!
//! An answer is data for the engine. An approval answer never widens the
//! task's grant, boundary, routes, or spend: the next turn runs under a
//! fresh grant with every usual check, exactly like any follow-up. It is
//! not a POL approval.

use super::{Status, Task};

/// The result ending of a turn that asked a question.
pub const QUESTION_ENDING: &str = "asked_question";
/// The result ending of a turn that asked for an approval.
pub const APPROVAL_ENDING: &str = "asked_approval";

/// What a waiting turn asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An open question; the answer is free text.
    Question,
    /// Approval of a step the engine named; the answer approves or denies.
    Approval,
}

impl Kind {
    /// The result ending that records this kind.
    #[must_use]
    pub const fn ending(self) -> &'static str {
        match self {
            Self::Question => QUESTION_ENDING,
            Self::Approval => APPROVAL_ENDING,
        }
    }

    /// The kind a result ending records, if it records one.
    #[must_use]
    pub fn of_ending(ending: &str) -> Option<Self> {
        [Self::Question, Self::Approval]
            .into_iter()
            .find(|kind| kind.ending() == ending)
    }
}

/// What the task's ended turn asked, while nothing has answered it: the
/// task is finished and its current run's result ending names a kind. A
/// follow-up of any kind starts a new turn and so ends the wait.
#[must_use]
pub fn pending(task: &Task) -> Option<Kind> {
    if task.status != Status::Finished {
        return None;
    }
    let result = task.run.as_ref()?.result.as_ref()?;
    Kind::of_ending(&result.ending)
}
