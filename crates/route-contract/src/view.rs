//! The workbench's view of one route (#10698): what a pane shows of a
//! [`RouteRecord`], read and never advanced.
//!
//! A new document beside the frozen ones. [`RouteView::of`] projects a
//! record the shared policy wrote: the request, thread, and task
//! identities; the computer the snapshot placed it on; the executor and
//! model; the payer of every resource; each run's lifecycle, check, and
//! artifacts; and the route's cost and cancellation. Three distinctions
//! the record keeps apart stay apart here:
//!
//! - [`Cost::Unknown`] (a run reported no cost), [`Cost::Recorded`] (every
//!   run's cost is known, and the route may still move), and
//!   [`Cost::Settled`] (every run's cost is known and the route ended).
//! - [`Outcome::Verified`] only when every run's independent check passed;
//!   a finished run with no check, an unverifiable check, or a dispute is
//!   [`Outcome::Unchecked`], and a failed check is [`Outcome::CheckFailed`].
//! - [`Cancellation::Requested`] (asked, not yet acknowledged by the run)
//!   and [`Cancellation::Acknowledged`] (the run ended cancelled).
//!
//! The view holds identities and labels, never the snapshot's workspace
//! path or the message, so the same view serves the Grid, the standalone
//! window, and any other surface that reads a route.

use serde::{Deserialize, Serialize};

use crate::digest::Digest;
use crate::lifecycle::{CheckLabel, Lifecycle};
use crate::record::RouteRecord;
use crate::route::RouteFamily;
use crate::snapshot::{Payer, Resource};

/// A route view's schema.
pub const VIEW_SCHEMA: &str = "openagents.route.view.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteView {
    /// [`VIEW_SCHEMA`].
    pub schema: String,
    pub request: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub family: RouteFamily,
    pub state: Lifecycle,
    pub snapshot: Digest,
    /// The computer the snapshot placed the work on; `None` for a route
    /// that runs nowhere (an answer, a refusal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    /// The grant that admitted the placement, with its epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    pub executor: Executor,
    /// Who pays for each resource, as the snapshot admitted it.
    pub payers: Vec<PayerLine>,
    pub runs: Vec<RunView>,
    pub cost: Cost,
    pub outcome: Outcome,
    pub cancellation: Cancellation,
    /// The route's wall time, received to settled, once settled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_ms: Option<u64>,
}

/// What executes the route.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executor {
    /// The engines the runs used, in run order, without repeats.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engines: Vec<String>,
    /// `provider/model`, when the snapshot pins one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `id@version`, for a plugin or program route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayerLine {
    pub resource: Resource,
    /// `openagents`, `key:PROVIDER`, or `login:ENGINE`.
    pub payer: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunView {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    pub state: Lifecycle,
    pub check: CheckLabel,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cancel_requested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
    pub artifacts: usize,
    /// The run record's payer (`ours` or `theirs`), once recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
}

/// The route's cost, as the record knows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Cost {
    /// Nothing ran, so nothing was charged.
    None,
    /// At least one run reported no cost: never shown as zero.
    Unknown { known_microusd: u64, missing: usize },
    /// Every run's cost is known; the route may still move.
    Recorded { microusd: u64 },
    /// Every run's cost is known and the route ended.
    Settled { microusd: u64 },
}

/// What the route established.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Waiting for the person: an offer or a command to confirm.
    Proposed,
    /// Admitted, dispatched, running, or checking.
    InProgress,
    /// Every run finished and its independent check passed.
    Verified,
    /// Finished, without a passing independent check on every run: no
    /// check, an unverifiable one, or a dispute. Never verified success.
    Unchecked,
    /// A finished run's check failed.
    CheckFailed,
    /// A run, or the route itself, failed.
    Failed,
    /// Cancelled, with the cancellation acknowledged.
    Cancelled,
    /// The record and the task owner disagree or a run's state is
    /// unknown: recovery reconciles the same task, never starts another.
    NeedsReconciliation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cancellation {
    None,
    /// Asked, and no run has acknowledged it yet.
    Requested,
    /// A run ended cancelled.
    Acknowledged,
}

fn payer_word(payer: &Payer) -> String {
    match payer {
        Payer::OpenAgents => "openagents".into(),
        Payer::CallerKey { provider } => format!("key:{provider}"),
        Payer::CallerLogin { engine } => format!("login:{engine}"),
    }
}

impl RouteView {
    /// Projects `record`. Reading a record never changes it.
    #[must_use]
    pub fn of(record: &RouteRecord) -> Self {
        let snapshot = &record.snapshot;
        let mut engines: Vec<String> = Vec::new();
        for engine in record.runs.iter().filter_map(|run| run.engine.as_ref()) {
            if !engines.contains(engine) {
                engines.push(engine.clone());
            }
        }
        let executor = Executor {
            engines,
            model: snapshot
                .route
                .model
                .as_ref()
                .map(|pin| format!("{}/{}", pin.provider, pin.model)),
            capability: snapshot
                .route
                .capability
                .as_ref()
                .map(|pin| format!("{}@{}", pin.id, pin.version)),
            revision: snapshot.route.executor_revision.clone(),
        };
        let runs: Vec<RunView> = record
            .runs
            .iter()
            .map(|run| RunView {
                task: run.task.clone(),
                engine: run.engine.clone(),
                revision: run.revision,
                state: run.projection.state,
                check: run.projection.check,
                cancel_requested: run.projection.cancel_requested,
                cost_microusd: run.cost_microusd,
                artifacts: run.artifacts.len(),
                payer: run.payer.clone(),
            })
            .collect();
        let missing = runs
            .iter()
            .filter(|run| run.cost_microusd.is_none())
            .count();
        let known: u64 = runs
            .iter()
            .filter_map(|run| run.cost_microusd)
            .fold(0, u64::saturating_add);
        let cost = if runs.is_empty() {
            Cost::None
        } else if missing > 0 {
            Cost::Unknown {
                known_microusd: known,
                missing,
            }
        } else if record.settled() {
            Cost::Settled { microusd: known }
        } else {
            Cost::Recorded { microusd: known }
        };
        let cancellation = if runs.iter().any(|run| run.state == Lifecycle::Cancelled)
            || (runs.is_empty() && record.state == Lifecycle::Cancelled)
        {
            Cancellation::Acknowledged
        } else if runs.iter().any(|run| run.cancel_requested) {
            Cancellation::Requested
        } else {
            Cancellation::None
        };
        Self {
            schema: VIEW_SCHEMA.into(),
            request: record.request.clone(),
            thread: record.thread.clone(),
            family: record.family(),
            state: record.state,
            snapshot: record.snapshot_digest.clone(),
            computer: snapshot.placement.computer.clone(),
            grant: snapshot
                .placement
                .grant
                .as_ref()
                .map(|grant| format!("{} epoch {}", grant.id, grant.epoch)),
            executor,
            payers: snapshot
                .money
                .payers
                .iter()
                .map(|entry| PayerLine {
                    resource: entry.resource,
                    payer: payer_word(&entry.payer),
                })
                .collect(),
            outcome: outcome(record.state, &runs),
            runs,
            cost,
            cancellation,
            wall_ms: record.wall_ms(),
        }
    }
}

fn outcome(state: Lifecycle, runs: &[RunView]) -> Outcome {
    match state {
        Lifecycle::Proposed | Lifecycle::AwaitingAuthorityOrPayment => Outcome::Proposed,
        Lifecycle::Received
        | Lifecycle::Admitted
        | Lifecycle::DispatchPending
        | Lifecycle::Running
        | Lifecycle::Checking => Outcome::InProgress,
        Lifecycle::NeedsReconciliation => Outcome::NeedsReconciliation,
        Lifecycle::Cancelled => Outcome::Cancelled,
        Lifecycle::Failed => {
            if runs.iter().any(|run| run.check == CheckLabel::CheckFailed) {
                Outcome::CheckFailed
            } else {
                Outcome::Failed
            }
        }
        Lifecycle::Completed => {
            // A route that completed with no task (an answer) has nothing
            // an independent check could verify.
            if !runs.is_empty() && runs.iter().all(|run| run.check == CheckLabel::Verified) {
                Outcome::Verified
            } else {
                Outcome::Unchecked
            }
        }
    }
}

impl Outcome {
    /// The outcome's wire word with spaces.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Proposed => "proposed",
            Outcome::InProgress => "in progress",
            Outcome::Verified => "verified",
            Outcome::Unchecked => "finished, not verified",
            Outcome::CheckFailed => "check failed",
            Outcome::Failed => "failed",
            Outcome::Cancelled => "cancelled",
            Outcome::NeedsReconciliation => "needs reconciliation",
        }
    }
}
