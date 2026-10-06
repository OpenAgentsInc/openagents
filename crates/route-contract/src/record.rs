//! The route record (phase 1, #10207): what one message's route became.
//!
//! A new document beside the frozen ones, not an edit of them. Every
//! message a client routes gets one: the [`RouteResult`], the immutable
//! [`AdmissionSnapshot`] that admitted it (named by its digest), the
//! router's own moves before a task exists, each task the route started
//! with its projected lifecycle, and the per-run cost and wall time.
//!
//! Cost and time are known only after the run, and the snapshot is
//! digested before dispatch, so they live here, bound to the snapshot by
//! [`RouteRecord::snapshot_digest`], never inside the snapshot. OpenAgents'
//! own apps record them without showing them (13.6).
//!
//! Nothing here is a second state machine. Before a task exists the record
//! moves only by [`Lifecycle::router_step`]; afterwards each run's state is
//! [`crate::lifecycle::project`] of the task owner's disposition, read and
//! copied here ([`RouteRecord::observe`]). A local command has no task: its
//! exit is the execution's disposition ([`RouteRecord::ran`]).
//!
//! Idempotency: one request has one record, and a task appears in it once.
//! A client that finds a record with tasks for a request follows those
//! tasks; it never starts the request's work again
//! ([`RouteRecord::dispatched_any`]).

use serde::{Deserialize, Serialize};

use crate::digest::Digest;
use crate::lifecycle::{CheckLabel, Lifecycle, Projection, TaskDisposition, Transition, project};
use crate::route::{Invalid, RefusalReason, RouteFamily, RouteResult};
use crate::snapshot::AdmissionSnapshot;

/// What one task of the route did, as the task owner says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunOutcome {
    pub task: String,
    /// The engine the run used, when known (`codex`, `claude`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// The task revision this was read at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    pub projection: Projection,
    /// The run's whole cost in micro-dollars (engine and Jev), when known.
    /// Recorded, never shown in the apps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
    /// The run's wall time in milliseconds, as its supervisor measured it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_ms: Option<u64>,
    /// Retained evidence by digest: the patch or artifact, the trace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Digest>,
    /// Who paid for the run's model calls, as its run record says (BYOK,
    /// #10176): `ours` or `theirs`. Absent until the run records it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    /// Under `theirs`, each of the person's keys that may have paid, by
    /// provider and fingerprint, never the key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payer_keys: Vec<PayerKey>,
}

/// One of the person's provider keys as a record names it (BYOK): the
/// provider (`openrouter`, `vercel`, `typesafe`) and the first 8 hex
/// characters of the key's SHA-256 digest. Never the key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayerKey {
    pub provider: String,
    pub fingerprint: String,
}

/// What a task owner reports for one task, for [`RouteRecord::observe`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub disposition: TaskDisposition,
    pub revision: u64,
    pub cost_microusd: Option<u64>,
    pub wall_ms: Option<u64>,
    pub artifacts: Vec<Digest>,
    /// The run record's payer, when it names one.
    pub payer: Option<String>,
    pub payer_keys: Vec<PayerKey>,
}

/// One message's route, admission, and outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRecord {
    /// [`crate::RECORD_SCHEMA`].
    pub schema: String,
    pub request: String,
    /// Jev gates evaluated for this request; absent in legacy records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<crate::decision::DecisionReading>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub result: RouteResult,
    pub snapshot: AdmissionSnapshot,
    pub snapshot_digest: Digest,
    /// The route's state: the router's own before a task exists, then the
    /// least settled of its runs' projections ([`RouteRecord::observe`]).
    pub state: Lifecycle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<RunOutcome>,
    /// Why the route ended without running, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalReason>,
    /// Unix milliseconds the message was routed.
    pub received_ms: u64,
    /// Unix milliseconds the route settled (a terminal state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_ms: Option<u64>,
    /// The person's keys that paid for routing and answering this message
    /// (BYOK `mine`, #10176), by provider and fingerprint; empty when it
    /// ran on ours. The snapshot's `money` names the payer per resource.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payer_keys: Vec<PayerKey>,
    /// A remote dispatch sent before its acknowledgment arrived (#10699):
    /// the recipient host and the idempotency key. Recovery asks that host
    /// for that key and never sends the work anywhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<Sent>,
}

/// A dispatch sent to a remote host, kept before the host is asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sent {
    pub recipient: String,
    pub key: String,
}

/// What one run of an admitted plugin or program produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityRun {
    /// The run ended without an error of its own.
    pub ok: bool,
    /// Digests of the artifacts it produced, retained by the caller.
    pub artifacts: Vec<Digest>,
    /// The check on its output: `unchecked` when none ran.
    pub check: CheckLabel,
    pub wall_ms: Option<u64>,
    /// `None` when the cost is unknown, never a stand-in zero.
    pub cost_microusd: Option<u64>,
}

/// Why a record refuses a move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The route result breaks a bound.
    Invalid(Invalid),
    /// The snapshot names another route result or family.
    Unbound,
    /// Not a move the router makes from this state.
    Step { from: Lifecycle, to: Lifecycle },
}

impl RouteRecord {
    /// A routed message, `received`, bound to its snapshot.
    ///
    /// # Errors
    ///
    /// An invalid result, or a snapshot that does not name this result.
    pub fn received(
        request: impl Into<String>,
        thread: Option<String>,
        result: RouteResult,
        snapshot: AdmissionSnapshot,
        now_ms: u64,
    ) -> Result<Self, Refused> {
        result.validate().map_err(Refused::Invalid)?;
        if snapshot.route.result != result.digest() || snapshot.route.family != result.family() {
            return Err(Refused::Unbound);
        }
        Ok(Self {
            schema: crate::RECORD_SCHEMA.into(),
            request: request.into(),
            decisions: Vec::new(),
            thread,
            snapshot_digest: snapshot.digest(),
            result,
            snapshot,
            state: Lifecycle::Received,
            transitions: Vec::new(),
            runs: Vec::new(),
            refusal: None,
            received_ms: now_ms,
            settled_ms: None,
            payer_keys: Vec::new(),
            sent: None,
        })
    }

    #[must_use]
    pub fn family(&self) -> RouteFamily {
        self.result.family()
    }

    /// One of the router's own moves ([`Lifecycle::router_step`]).
    ///
    /// # Errors
    ///
    /// A move the router does not make from the current state.
    pub fn step(&mut self, to: Lifecycle, cause: &str, now_ms: u64) -> Result<(), Refused> {
        let from = self.state;
        if !from.router_step(to) {
            return Err(Refused::Step { from, to });
        }
        self.push(None, from, to, cause, None);
        self.state = to;
        self.settle_if_done(now_ms);
        Ok(())
    }

    /// The route ended without running: `failed` with `reason`, from any
    /// router-owned state that can fail.
    ///
    /// # Errors
    ///
    /// The record already left the router's states.
    pub fn refuse(
        &mut self,
        reason: Option<RefusalReason>,
        cause: &str,
        now_ms: u64,
    ) -> Result<(), Refused> {
        self.step(Lifecycle::Failed, cause, now_ms)?;
        self.refusal = reason;
        Ok(())
    }

    /// `task` started for this route: the dispatch intent is kept before
    /// the executor is asked, and a task appears once however often this
    /// is called. From `admitted` the record moves to `dispatch_pending`.
    ///
    /// # Errors
    ///
    /// The route was never admitted.
    pub fn dispatched(&mut self, task: &str, engine: Option<&str>) -> Result<(), Refused> {
        if self.state == Lifecycle::Admitted {
            self.push(
                Some(task),
                Lifecycle::Admitted,
                Lifecycle::DispatchPending,
                "dispatch",
                None,
            );
            self.state = Lifecycle::DispatchPending;
        } else if self.runs.is_empty() || self.state.router_owned() {
            return Err(Refused::Step {
                from: self.state,
                to: Lifecycle::DispatchPending,
            });
        }
        if !self.runs.iter().any(|run| run.task == task) {
            self.runs.push(RunOutcome {
                task: task.to_owned(),
                engine: engine.map(str::to_owned),
                revision: None,
                projection: Projection {
                    state: Lifecycle::DispatchPending,
                    check: CheckLabel::Pending,
                    cancel_requested: false,
                },
                cost_microusd: None,
                wall_ms: None,
                artifacts: Vec::new(),
                payer: None,
                payer_keys: Vec::new(),
            });
        }
        Ok(())
    }

    /// Whether the request's work already started: a client then follows
    /// [`RouteRecord::tasks`] and never starts it again.
    #[must_use]
    pub fn dispatched_any(&self) -> bool {
        !self.runs.is_empty()
    }

    #[must_use]
    pub fn tasks(&self) -> Vec<&str> {
        self.runs.iter().map(|run| run.task.as_str()).collect()
    }

    /// The task owner's disposition of `task`, copied in: its projection,
    /// a transition when the state moved, cost, wall time, and evidence.
    /// A task the route did not start is ignored (`false`).
    pub fn observe(&mut self, task: &str, seen: Observation, now_ms: u64) -> bool {
        let projection = project(seen.disposition);
        let Some(at) = self.runs.iter().position(|run| run.task == task) else {
            return false;
        };
        let before = self.runs[at].projection.state;
        let prior = self.runs[at].revision;
        if before != projection.state {
            self.push(Some(task), before, projection.state, "task_owner", prior);
        }
        let run = &mut self.runs[at];
        run.projection = projection;
        run.revision = Some(seen.revision);
        run.cost_microusd = seen.cost_microusd.or(run.cost_microusd);
        run.wall_ms = seen.wall_ms.or(run.wall_ms);
        if !seen.artifacts.is_empty() {
            run.artifacts = seen.artifacts;
        }
        if seen.payer.is_some() {
            run.payer = seen.payer;
            run.payer_keys = seen.payer_keys;
        }
        self.state = aggregate(self.runs.iter().map(|run| run.projection.state));
        self.settle_if_done(now_ms);
        true
    }

    /// A local command's exit: it has no task, so its exit is the
    /// execution's disposition. From `admitted` (or `dispatch_pending`).
    ///
    /// # Errors
    ///
    /// Not a local command, or not admitted.
    pub fn ran(&mut self, ok: bool, wall_ms: Option<u64>, now_ms: u64) -> Result<(), Refused> {
        let to = if ok {
            Lifecycle::Completed
        } else {
            Lifecycle::Failed
        };
        let from = self.state;
        if self.family() != RouteFamily::LocalCommand
            || !matches!(from, Lifecycle::Admitted | Lifecycle::DispatchPending)
        {
            return Err(Refused::Step { from, to });
        }
        self.push(None, from, to, "command_exit", None);
        self.state = to;
        self.runs.push(RunOutcome {
            task: format!("command:{}", self.request),
            engine: None,
            revision: None,
            projection: Projection {
                state: to,
                check: CheckLabel::Pending,
                cancel_requested: false,
            },
            cost_microusd: Some(0),
            wall_ms,
            artifacts: Vec::new(),
            payer: None,
            payer_keys: Vec::new(),
        });
        self.settle_if_done(now_ms);
        Ok(())
    }

    /// A plugin or program route ran once (#10670): the digests of what it
    /// produced and the check on them settle it. A failed run, or one whose
    /// check failed, ends `failed`; a run is never recorded twice.
    ///
    /// # Errors
    ///
    /// Another family, a record not admitted, or one that already ran.
    pub fn capability_ran(&mut self, run: CapabilityRun, now_ms: u64) -> Result<(), Refused> {
        let to = if run.ok && run.check != CheckLabel::CheckFailed {
            Lifecycle::Completed
        } else {
            Lifecycle::Failed
        };
        let from = self.state;
        if self.family() != RouteFamily::Plugin
            || !matches!(from, Lifecycle::Admitted | Lifecycle::DispatchPending)
            || !self.runs.is_empty()
        {
            return Err(Refused::Step { from, to });
        }
        let task = format!("capability:{}", self.request);
        self.push(Some(&task), from, to, "capability_exit", None);
        if let Some(last) = self.transitions.last_mut() {
            last.artifacts.clone_from(&run.artifacts);
        }
        self.state = to;
        self.runs.push(RunOutcome {
            task,
            engine: None,
            revision: None,
            projection: Projection {
                state: to,
                check: run.check,
                cancel_requested: false,
            },
            cost_microusd: run.cost_microusd,
            wall_ms: run.wall_ms,
            artifacts: run.artifacts,
            payer: None,
            payer_keys: Vec::new(),
        });
        self.settle_if_done(now_ms);
        Ok(())
    }

    /// Whether the route reached a state nothing moves on its own.
    #[must_use]
    pub fn settled(&self) -> bool {
        terminal(self.state)
    }

    /// The route's wall time, received to settled.
    #[must_use]
    pub fn wall_ms(&self) -> Option<u64> {
        self.settled_ms
            .map(|settled| settled.saturating_sub(self.received_ms))
    }

    /// The whole route's cost: the sum of its runs', when every run's is
    /// known.
    #[must_use]
    pub fn cost_microusd(&self) -> Option<u64> {
        self.runs
            .iter()
            .map(|run| run.cost_microusd)
            .try_fold(0u64, |sum, cost| cost.map(|cost| sum.saturating_add(cost)))
    }

    fn push(
        &mut self,
        task: Option<&str>,
        from: Lifecycle,
        to: Lifecycle,
        cause: &str,
        prior_revision: Option<u64>,
    ) {
        self.transitions.push(Transition {
            schema: crate::TRANSITION_SCHEMA.into(),
            request: self.request.clone(),
            task: task.map(str::to_owned),
            from,
            to,
            cause: cause.to_owned(),
            prior_revision,
            attempt: None,
            artifacts: Vec::new(),
        });
    }

    fn settle_if_done(&mut self, now_ms: u64) {
        if terminal(self.state) && self.settled_ms.is_none() {
            self.settled_ms = Some(now_ms);
        } else if !terminal(self.state) {
            self.settled_ms = None;
        }
    }
}

/// States nothing moves on its own. `proposed` waits for the person, so
/// it is not settled; `needs_reconciliation` waits for recovery.
fn terminal(state: Lifecycle) -> bool {
    matches!(
        state,
        Lifecycle::Completed | Lifecycle::Failed | Lifecycle::Cancelled
    )
}

/// A route's state from its runs': the least settled one leads
/// (reconciliation, then work in flight), and once all settled a failure
/// outranks a cancellation, which outranks completion.
fn aggregate(states: impl Iterator<Item = Lifecycle>) -> Lifecycle {
    const ORDER: [Lifecycle; 7] = [
        Lifecycle::NeedsReconciliation,
        Lifecycle::DispatchPending,
        Lifecycle::Running,
        Lifecycle::Checking,
        Lifecycle::Failed,
        Lifecycle::Cancelled,
        Lifecycle::Completed,
    ];
    let states: Vec<Lifecycle> = states.collect();
    ORDER
        .into_iter()
        .find(|state| states.contains(state))
        .unwrap_or(Lifecycle::DispatchPending)
}
