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
use coder_access::protocol::TaskCreate;
use nostr::activity_summary::Phase;

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

    /// Every task's current revision and phase, including changes made
    /// outside a device operation, such as an auto-started run finishing.
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
    use nostr::activity_summary::{self, Attention, SubjectKind, SummaryDraft};

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
}
