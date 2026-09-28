//! The task owner a host hands admitted task operations to.
//!
//! The host checks the device's grant and the `operate` right first; the
//! owner then records the effect durably. Every call carries the NIP-HOST
//! request ID as its idempotency key, so a retry after an uncertain save
//! repeats the same logical operation rather than minting a new one.
//!
//! Creating a task records intent only. It grants no execution authority:
//! the local task owner still needs its own explicit execution grant before
//! anything runs. Steering and cancelling follow the CTRL semantics of the
//! local owner: a steer records a replacement instruction and supersedes a
//! running context, and a cancel requests a stop.

use coder_access::Code;
use coder_access::protocol::{QueueEdit, TaskCommand, TaskCreate, TaskQueue};
use nostr::activity_summary::{Attention, Phase};

/// A task after an accepted operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRef {
    /// The host-issued task ID: 64 lowercase hexadecimal characters.
    pub task: String,
    /// The task's revision after the operation.
    pub revision: u64,
    /// The phase an activity summary reports.
    pub phase: Phase,
}

/// Something typed the host can say about a task in its summary headline.
/// The host builds the text from this state alone, never from a prompt or
/// engine output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// No admitted model provider had capacity, so the task ended without
    /// running, or stopped when the last one refused. `until` is the
    /// earliest reset, in Unix seconds, when a provider reported one.
    NoCapacity { until: Option<u64> },
    /// The engine ended its turn with a question and waits for an answer.
    Question,
    /// The engine ended its turn asking to approve a step and waits for the
    /// answer.
    Approval,
}

impl Note {
    /// The summary headline, such as
    /// `No model capacity until 2026-10-03 18:07 UTC`.
    #[must_use]
    pub fn headline(self) -> String {
        match self {
            Note::NoCapacity { until: Some(until) } => {
                format!("No model capacity until {}", utc(until))
            }
            Note::NoCapacity { until: None } => "No model capacity".to_owned(),
            Note::Question => "Coder asked a question".to_owned(),
            Note::Approval => "Coder asked for approval".to_owned(),
        }
    }

    /// The attention a summary with this note carries: a waiting question
    /// asks for input, a waiting approval for an approval.
    #[must_use]
    pub fn attention(self) -> Option<Attention> {
        match self {
            Note::NoCapacity { .. } => None,
            Note::Question => Some(Attention::Input),
            Note::Approval => Some(Attention::Approval),
        }
    }
}

/// `YYYY-MM-DD HH:MM UTC` for Unix seconds.
#[must_use]
pub fn utc(seconds: u64) -> String {
    format!(
        "{} {:02}:{:02} UTC",
        nostr::git_sign::utc_date(seconds),
        seconds % 86_400 / 3_600,
        seconds % 3_600 / 60
    )
}

/// Who sent a durable task command: the device key and, for a device, the
/// grant and epoch the host admitted it under. A deferred effect rechecks
/// these before it runs; the owner has no grant.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub device: String,
    pub grant: Option<String>,
    pub epoch: Option<u64>,
}

/// Whether a principal still holds the `operate` right under the same
/// grant and epoch, now.
pub type Standing<'a> = &'a (dyn Fn(&Principal) -> bool + Sync);

/// Where admitted task operations go.
///
/// Implementations return promptly and never call back into the host.
pub trait Tasks: Send + Sync {
    /// Record a new task. `key` is the idempotency key.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn create(&self, key: &str, device: &str, task: &TaskCreate) -> Result<TaskRef, Code>;

    /// Replace a task's instructions at the revision the device last read.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn steer(
        &self,
        key: &str,
        device: &str,
        task: &str,
        revision: u64,
        prompt: &str,
    ) -> Result<TaskRef, Code>;

    /// Request a task's cancellation at the revision the device last read.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn cancel(
        &self,
        key: &str,
        device: &str,
        task: &str,
        revision: u64,
        reason: &str,
    ) -> Result<TaskRef, Code>;

    /// Take a finished or cancelled task off every device's lists, deleting
    /// nothing. Archiving an archived task succeeds again. The default
    /// refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn archive(&self, _key: &str, _device: &str, _task: &str) -> Result<(), Code> {
        Err(Code::Unsupported)
    }

    /// Record and evaluate a durable task command. The device's command ID
    /// is its idempotency key across NIP-HOST requests: a replay returns
    /// the recorded disposition and never runs the command twice. The
    /// default refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn command(
        &self,
        _principal: &Principal,
        _command: &TaskCommand,
        _standing: Standing<'_>,
    ) -> Result<TaskRef, Code> {
        Err(Code::Unsupported)
    }

    /// List or edit a task's held messages for `principal`: the edit lease,
    /// and changes to this device's own messages under it. `standing`
    /// rechecks each sender whose held message the edit lets run. Returns
    /// the queue, and the task when the edit changed it, such as a message
    /// sent now. The default refuses as `unsupported`.
    ///
    /// # Errors
    /// Returns the NIP-HOST refusal code the device receives.
    fn queue(
        &self,
        _principal: &Principal,
        _task: &str,
        _edit: &QueueEdit,
        _standing: Standing<'_>,
    ) -> Result<(TaskQueue, Option<TaskRef>), Code> {
        Err(Code::Unsupported)
    }

    /// Evaluate held commands again, such as a queued message after its
    /// task's turn ends. `standing` rechecks each sender's grant before a
    /// deferred command runs. The host calls this periodically, off its
    /// async runtime. The default does nothing.
    fn tick(&self, _standing: Standing<'_>) {}

    /// Every listed task's current revision and phase, including changes
    /// made outside a device operation, such as an auto-started run
    /// finishing. Archived tasks are not listed.
    /// The host publishes a summary when a revision changes. The default
    /// reports none.
    fn current(&self) -> Vec<TaskRef> {
        Vec::new()
    }

    /// A typed note for a task's summary headline, such as why it ended
    /// without running. The default has none.
    fn note(&self, _task: &str) -> Option<Note> {
        None
    }

    /// A cheap fingerprint of the task store, such as its files' lengths and
    /// modification times, that changes whenever a task or a held command
    /// may have. The host reads it often and runs [`Tasks::tick`] and
    /// [`Tasks::current`] as soon as it moves, so a device hears of a run
    /// starting or ending at once. The default, `None`, leaves the host to
    /// its periodic sweep.
    fn stamp(&self) -> Option<Vec<u8>> {
        None
    }
}

/// A host without a task owner. Every task operation refuses as
/// `unavailable`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoTasks;

impl Tasks for NoTasks {
    fn create(&self, _: &str, _: &str, _: &TaskCreate) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn steer(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
    fn cancel(&self, _: &str, _: &str, _: &str, _: u64, _: &str) -> Result<TaskRef, Code> {
        Err(Code::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::activity_summary::{self, SubjectKind, SummaryDraft};

    #[test]
    fn a_no_capacity_note_survives_the_summary_disclosure_rules() {
        let note = Note::NoCapacity {
            until: Some(1_791_050_823),
        };
        assert_eq!(
            note.headline(),
            "No model capacity until 2026-10-03 18:07 UTC"
        );
        let headline = note.headline();
        let summary = activity_summary::encode(&SummaryDraft {
            host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            subject_kind: SubjectKind::Task,
            subject: &"a".repeat(64),
            sequence: 2,
            phase: Phase::Cancelled,
            headline: &headline,
            attention: Attention::None,
            updated_at: 1_790_572_210,
        })
        .unwrap();
        assert_eq!(summary.headline, headline);
        assert_eq!(
            Note::NoCapacity { until: None }.headline(),
            "No model capacity"
        );
    }

    #[test]
    fn a_waiting_question_or_approval_asks_for_attention_without_its_text() {
        for (note, attention) in [
            (Note::Question, Attention::Input),
            (Note::Approval, Attention::Approval),
        ] {
            assert_eq!(note.attention(), Some(attention));
            let headline = note.headline();
            let summary = activity_summary::encode(&SummaryDraft {
                host: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                subject_kind: SubjectKind::Task,
                subject: &"a".repeat(64),
                sequence: 3,
                phase: Phase::Waiting,
                headline: &headline,
                attention,
                updated_at: 1_790_572_210,
            })
            .unwrap();
            assert_eq!(summary.attention, attention);
        }
        assert_eq!(Note::NoCapacity { until: None }.attention(), None);
    }
}
