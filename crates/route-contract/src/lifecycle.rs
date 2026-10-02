//! The domain lifecycle (plan section 6) and its mapping onto the task
//! owner's dispositions.
//!
//! The task owner's journal (`docs/coder/runtime/task-owner.md`) is the only
//! execution state machine. This module adds none:
//!
//! - Before a task exists, the router owns four states: `received`,
//!   `proposed`, `awaiting_authority_or_payment`, and `admitted`.
//!   [`Lifecycle::router_step`] lists their few moves. `admitted` means the
//!   snapshot and dispatch intent are persisted; submitting the task to the
//!   owner follows.
//! - From the moment the owner accepts the task, the lifecycle is a pure
//!   projection of the owner's `(status, execution, checks)` triple:
//!   [`project`]. Nothing here is stored or advanced on its own.
//!
//! [`TaskStatus`], [`TaskExecution`], and [`TaskChecks`] carry the exact
//! wire words of `coder::task::{Status, Execution, Checks}`; a test in
//! `coder` keeps the words equal, so an API adapter reading a task view
//! over the wire can project it without linking Coder.

use serde::{Deserialize, Serialize};

use crate::digest::Digest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Received,
    Proposed,
    AwaitingAuthorityOrPayment,
    Admitted,
    DispatchPending,
    Running,
    Checking,
    Completed,
    Failed,
    Cancelled,
    NeedsReconciliation,
}

impl Lifecycle {
    pub const ALL: [Lifecycle; 11] = [
        Lifecycle::Received,
        Lifecycle::Proposed,
        Lifecycle::AwaitingAuthorityOrPayment,
        Lifecycle::Admitted,
        Lifecycle::DispatchPending,
        Lifecycle::Running,
        Lifecycle::Checking,
        Lifecycle::Completed,
        Lifecycle::Failed,
        Lifecycle::Cancelled,
        Lifecycle::NeedsReconciliation,
    ];

    /// Whether the router owns this state (no task yet).
    #[must_use]
    pub fn router_owned(self) -> bool {
        matches!(
            self,
            Lifecycle::Received
                | Lifecycle::Proposed
                | Lifecycle::AwaitingAuthorityOrPayment
                | Lifecycle::Admitted
        )
    }

    /// The router's own moves before a task exists. An answer, refusal, or
    /// clarification ends at `completed` or `failed` with no task; a
    /// declined or expired offer ends at `cancelled`; `admitted` hands off
    /// to the task owner (`dispatch_pending`). Every later state comes from
    /// [`project`], never from here.
    #[must_use]
    pub fn router_step(self, to: Lifecycle) -> bool {
        use Lifecycle::{
            Admitted, AwaitingAuthorityOrPayment, Cancelled, Completed, DispatchPending, Failed,
            Proposed, Received,
        };
        matches!(
            (self, to),
            (Received, Proposed | Admitted | Completed | Failed)
                | (
                    Proposed,
                    AwaitingAuthorityOrPayment | Admitted | Cancelled | Failed
                )
                | (AwaitingAuthorityOrPayment, Admitted | Cancelled | Failed)
                | (Admitted, DispatchPending | Failed)
        )
    }
}

/// `coder::task::Status`, by its wire words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Cancelled,
    Running,
    CancelRequested,
    Finished,
    Unknown,
}

/// `coder::task::Execution`, by its wire words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecution {
    NotStarted,
    Running,
    Finished,
    Failed,
    Stopped,
    Unknown,
}

/// `coder::task::Checks`, by its wire words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskChecks {
    NotRun,
    Running,
    Passed,
    Failed,
    Unavailable,
    Disputed,
}

impl TaskStatus {
    pub const ALL: [TaskStatus; 6] = [
        TaskStatus::Queued,
        TaskStatus::Cancelled,
        TaskStatus::Running,
        TaskStatus::CancelRequested,
        TaskStatus::Finished,
        TaskStatus::Unknown,
    ];
}
impl TaskExecution {
    pub const ALL: [TaskExecution; 6] = [
        TaskExecution::NotStarted,
        TaskExecution::Running,
        TaskExecution::Finished,
        TaskExecution::Failed,
        TaskExecution::Stopped,
        TaskExecution::Unknown,
    ];
}
impl TaskChecks {
    pub const ALL: [TaskChecks; 6] = [
        TaskChecks::NotRun,
        TaskChecks::Running,
        TaskChecks::Passed,
        TaskChecks::Failed,
        TaskChecks::Unavailable,
        TaskChecks::Disputed,
    ];
}

/// The task owner's disposition of one task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDisposition {
    pub status: TaskStatus,
    pub execution: TaskExecution,
    pub checks: TaskChecks,
}

/// The lifecycle a disposition projects to, with what the state alone
/// cannot say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub state: Lifecycle,
    /// What a finished result's check establishes; `verified` only when
    /// independent checks passed.
    pub check: CheckLabel,
    /// A stop was requested and not yet acknowledged by the supervisor:
    /// reported apart from an acknowledged cancellation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cancel_requested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckLabel {
    /// Not finished yet, or nothing to check.
    Pending,
    Verified,
    /// Finished with no independent check run: labeled, never presented
    /// as verified success.
    Unchecked,
    /// The checker could not establish a result.
    Unverifiable,
    CheckFailed,
    /// A correction arrived after the result; it answers the old request.
    Disputed,
}

/// The mapping table, as one total function over every
/// `(status, execution, checks)` triple.
///
/// | status | execution | checks | lifecycle | check |
/// | --- | --- | --- | --- | --- |
/// | `unknown` (any) or any with execution `unknown` | | | `needs_reconciliation` | `pending` |
/// | `queued` | any | any | `dispatch_pending` | `pending` |
/// | `cancelled` | any | any | `cancelled` | `pending` |
/// | `running` / `cancel_requested` | `not_started` | any | `dispatch_pending` | `pending` |
/// | `running` / `cancel_requested` | `running` | any | `running` | `pending` |
/// | `running` / `cancel_requested` | `finished`, `failed`, `stopped` | any | `needs_reconciliation` | `pending` |
/// | `finished` | `finished` | `running` | `checking` | `pending` |
/// | `finished` | `finished` | `passed` | `completed` | `verified` |
/// | `finished` | `finished` | `not_run` | `completed` | `unchecked` |
/// | `finished` | `finished` | `unavailable` | `completed` | `unverifiable` |
/// | `finished` | `finished` | `disputed` | `completed` | `disputed` |
/// | `finished` | `finished` | `failed` | `failed` | `check_failed` |
/// | `finished` | `failed` | any | `failed` | `pending` |
/// | `finished` | `stopped` | any | `cancelled` | `pending` |
/// | `finished` | `not_started`, `running` | any | `needs_reconciliation` | `pending` |
///
/// `cancel_requested` is set for status `cancel_requested`. The owner's
/// settle path ends an orphaned run as finished/failed (stopped when a stop
/// was asked), so it projects to `failed` or `cancelled`, not
/// reconciliation.
#[must_use]
pub fn project(disposition: TaskDisposition) -> Projection {
    use TaskChecks as C;
    use TaskExecution as E;
    use TaskStatus as S;
    let TaskDisposition {
        status,
        execution,
        checks,
    } = disposition;
    let plain = |state| Projection {
        state,
        check: CheckLabel::Pending,
        cancel_requested: status == S::CancelRequested,
    };
    if status == S::Unknown || execution == E::Unknown {
        return plain(Lifecycle::NeedsReconciliation);
    }
    match status {
        S::Unknown => plain(Lifecycle::NeedsReconciliation),
        S::Queued => plain(Lifecycle::DispatchPending),
        S::Cancelled => plain(Lifecycle::Cancelled),
        S::Running | S::CancelRequested => match execution {
            E::NotStarted => plain(Lifecycle::DispatchPending),
            E::Running => plain(Lifecycle::Running),
            E::Finished | E::Failed | E::Stopped | E::Unknown => {
                plain(Lifecycle::NeedsReconciliation)
            }
        },
        S::Finished => match execution {
            E::Finished => {
                let (state, check) = match checks {
                    C::Running => (Lifecycle::Checking, CheckLabel::Pending),
                    C::Passed => (Lifecycle::Completed, CheckLabel::Verified),
                    C::NotRun => (Lifecycle::Completed, CheckLabel::Unchecked),
                    C::Unavailable => (Lifecycle::Completed, CheckLabel::Unverifiable),
                    C::Disputed => (Lifecycle::Completed, CheckLabel::Disputed),
                    C::Failed => (Lifecycle::Failed, CheckLabel::CheckFailed),
                };
                Projection {
                    state,
                    check,
                    cancel_requested: false,
                }
            }
            E::Failed => plain(Lifecycle::Failed),
            E::Stopped => plain(Lifecycle::Cancelled),
            E::NotStarted | E::Running | E::Unknown => plain(Lifecycle::NeedsReconciliation),
        },
    }
}

/// One recorded move: its cause, the prior revision, the attempt, and the
/// artifacts it references (section 6). For task-owned states this is a
/// projection of a journal event, never a separate write path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    /// [`crate::TRANSITION_SCHEMA`].
    pub schema: String,
    pub request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub from: Lifecycle,
    pub to: Lifecycle,
    pub cause: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Digest>,
}
