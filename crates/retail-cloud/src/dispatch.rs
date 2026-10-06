//! Dispatching one funded task and streaming its retained execution
//! (#10713).
//!
//! The funded execution has one task identity, `task_<execution>`, recorded
//! before the task owner on the sandbox is contacted. A transport attempt is
//! not a task: a lost acknowledgment is resolved by looking the task up by
//! its identity, and the owner's submission is idempotent on it, so a crash
//! or a client restart recovers the same task with one effect. No executor
//! starts before the dispatch authorities, a live hold, a ready sandbox, and
//! delivered material are all in place.
//!
//! Observation reads the owner's event stream by cursor and needs only the
//! observe right. A client that disconnects reattaches with its cursor; it
//! never cancels or redispatches anything.
//!
//! [`TaskOwner`] is the seam to Coder's task owner on the sandbox
//! (`docs/coder/runtime/task-owner.md`); [`crate::fake::FakeTaskOwner`]
//! simulates it.

use pay_ledger::Ledger;
use pay_ledger::compute::HoldState;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::{self, Current, Step};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::provision::ProvisionState;
use crate::{Error, Result};

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS dispatch (
    execution TEXT PRIMARY KEY,
    task TEXT NOT NULL UNIQUE,
    resource TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('intent','sent','acknowledged')),
    transport_attempts INTEGER NOT NULL,
    intent_at INTEGER NOT NULL,
    acknowledged_at INTEGER
);
";

/// What the task owner is asked to run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchSpec {
    pub task: String,
    pub execution: String,
    /// The request digest the admission binds.
    pub request: String,
    /// Frozen before the candidate exists.
    pub checks: Vec<String>,
    pub max_seconds: u64,
    /// `codex`, the one v1 engine.
    pub engine: String,
}

/// How the executor ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorEnd {
    Completed,
    Failed,
    /// The customer's provider refused for a usage or rate limit.
    Limited,
    TimedOut,
}

/// One independent check run on an exact candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRun {
    pub command: String,
    /// The candidate patch's digest the check ran against.
    pub candidate: String,
    pub exit_status: i32,
}

/// The owner's account of a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Ended {
        end: ExecutorEnd,
        /// The retained patch's digest, when the executor changed anything.
        patch: Option<String>,
        checks: Vec<CheckRun>,
    },
    Cancelled,
}

/// One progress event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEvent {
    pub cursor: u64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerError {
    /// The call failed or its answer was lost; the effect is unknown.
    Unknown(String),
}

/// The seam to the task owner on the sandbox.
pub trait TaskOwner {
    /// Submit a task. Submitting an existing task identity returns it and
    /// starts nothing.
    fn submit(&self, resource: &str, spec: &DispatchSpec) -> std::result::Result<(), OwnerError>;
    /// The task's status, or `None` when the owner has no such task.
    fn status(
        &self,
        resource: &str,
        task: &str,
    ) -> std::result::Result<Option<TaskStatus>, OwnerError>;
    /// Events after `cursor`, oldest first.
    fn events(
        &self,
        resource: &str,
        task: &str,
        after: u64,
    ) -> std::result::Result<Vec<TaskEvent>, OwnerError>;
}

/// Where one execution's dispatch stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchState {
    /// Recorded; the owner not contacted yet.
    Intent,
    /// A submission went out; whether it arrived is not known yet.
    Sent,
    /// The owner has the task.
    Acknowledged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispatch {
    pub execution: String,
    pub task: String,
    pub resource: String,
    pub state: DispatchState,
    pub transport_attempts: u32,
    pub intent_at: i64,
    pub acknowledged_at: Option<i64>,
}

/// The task identity for `execution`.
#[must_use]
pub fn task_id(execution: &str) -> String {
    format!("task_{execution}")
}

/// The dispatch specification for `funded`.
#[must_use]
pub fn spec(funded: &FundedRequest) -> DispatchSpec {
    DispatchSpec {
        task: task_id(&funded.execution),
        execution: funded.execution.clone(),
        request: funded.admission.request.clone(),
        checks: funded.task.checks.clone(),
        max_seconds: funded.quote.max_seconds,
        engine: "codex".into(),
    }
}

/// Dispatch `funded`'s task, or advance a dispatch already under way.
///
/// # Errors
///
/// [`Error::Denied`] without the dispatch authorities, [`Error::Invalid`]
/// without a live hold, a ready sandbox, or delivered material, or a
/// journal failure. Transport failures are recorded as state.
pub fn dispatch(
    journal: &mut Journal,
    ledger: &Ledger,
    owner: &impl TaskOwner,
    funded: &FundedRequest,
    current: &Current,
    now: i64,
) -> Result<Dispatch> {
    if journal.cleanup_requested(&funded.execution)? {
        return Err(Error::Invalid(
            "cleanup prevents new execution or provisioning",
        ));
    }
    if let Some(found) = journal.dispatch(&funded.execution)?
        && found.state == DispatchState::Acknowledged
    {
        return Ok(found);
    }
    authority::check(Step::Dispatch, &funded.admission, current).map_err(Error::Denied)?;
    let hold = ledger
        .hold(&funded.request)?
        .ok_or(Error::Invalid("no hold for the funded request"))?;
    if hold.state != HoldState::Held {
        return Err(Error::Invalid("the funded request's hold is not live"));
    }
    let Some(ProvisionState::Ready { resource, .. }) =
        journal.provisioning(&funded.execution)?.map(|p| p.state)
    else {
        return Err(Error::Invalid("the sandbox is not ready"));
    };
    match journal.delivered(&funded.execution)? {
        Some((delivered, false)) if delivered.resource == resource => {}
        _ => return Err(Error::Invalid("the material is not delivered")),
    }
    let task = task_id(&funded.execution);
    journal.connection.execute(
        "INSERT INTO dispatch(execution,task,resource,state,transport_attempts,intent_at) VALUES(?,?,?,'intent',0,?) ON CONFLICT(execution) DO NOTHING",
        params![funded.execution, task, resource, now],
    )?;
    // A task the owner already has is acknowledged, whatever the journal
    // last saw.
    match owner.status(&resource, &task) {
        Ok(Some(_)) => return journal.acknowledge(&funded.execution, now),
        Ok(None) => {}
        Err(_) => {
            return journal
                .dispatch(&funded.execution)?
                .ok_or(Error::Invalid("missing dispatch"));
        }
    }
    if journal.cleanup_requested(&funded.execution)? {
        return Err(Error::Invalid("cleanup prevents dispatch"));
    }
    journal.connection.execute(
        "UPDATE dispatch SET state='sent', transport_attempts=transport_attempts+1 WHERE execution=?",
        [&funded.execution],
    )?;
    match owner.submit(&resource, &spec(funded)) {
        Ok(()) => journal.acknowledge(&funded.execution, now),
        Err(OwnerError::Unknown(_)) => journal
            .dispatch(&funded.execution)?
            .ok_or(Error::Invalid("missing dispatch")),
    }
}

/// A page of a task's events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub events: Vec<TaskEvent>,
    /// Pass this back to read on.
    pub next: u64,
    pub status: Option<TaskStatus>,
}

/// Read a task's events after `cursor`. Needs only the observe right, and
/// never changes the task.
///
/// # Errors
///
/// [`Error::Denied`] without the observe right, [`Error::Invalid`] before
/// dispatch, or [`Error::Invalid`] when the owner cannot be reached.
pub fn observe(
    journal: &Journal,
    owner: &impl TaskOwner,
    funded: &FundedRequest,
    current: &Current,
    cursor: u64,
) -> Result<Page> {
    authority::check(Step::Read, &funded.admission, current).map_err(Error::Denied)?;
    let found = journal
        .dispatch(&funded.execution)?
        .ok_or(Error::Invalid("not dispatched"))?;
    let events = owner
        .events(&found.resource, &found.task, cursor)
        .map_err(|_| Error::Invalid("the task owner is unreachable"))?;
    let status = owner.status(&found.resource, &found.task).ok().flatten();
    let next = events.last().map_or(cursor, |e| e.cursor);
    Ok(Page {
        events,
        next,
        status,
    })
}

/// The check verdict of an ended task, from the route contract's
/// vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The patch is retained and every declared check passed on it.
    Verified,
    /// At least one declared check failed, or did not run on this exact
    /// candidate.
    CheckFailed,
    /// No change to check.
    Unchecked,
}

/// The verdict for a completed task: honest about which candidate each
/// check ran on.
#[must_use]
pub fn verdict(patch: Option<&str>, declared: &[String], checks: &[CheckRun]) -> Verdict {
    let Some(patch) = patch else {
        return Verdict::Unchecked;
    };
    let all_ran_and_passed = declared.iter().all(|command| {
        checks
            .iter()
            .any(|c| &c.command == command && c.candidate == patch && c.exit_status == 0)
    });
    if all_ran_and_passed {
        Verdict::Verified
    } else {
        Verdict::CheckFailed
    }
}

impl Journal {
    /// One execution's dispatch record.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn dispatch(&self, execution: &str) -> Result<Option<Dispatch>> {
        let row = self
            .connection
            .query_row(
                "SELECT execution,task,resource,state,transport_attempts,intent_at,acknowledged_at FROM dispatch WHERE execution=?",
                [execution],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, u32>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            execution,
            task,
            resource,
            state,
            transport_attempts,
            intent_at,
            acknowledged_at,
        )) = row
        else {
            return Ok(None);
        };
        let state = match state.as_str() {
            "intent" => DispatchState::Intent,
            "sent" => DispatchState::Sent,
            "acknowledged" => DispatchState::Acknowledged,
            _ => return Err(Error::Invalid("dispatch state")),
        };
        Ok(Some(Dispatch {
            execution,
            task,
            resource,
            state,
            transport_attempts,
            intent_at,
            acknowledged_at,
        }))
    }

    fn acknowledge(&mut self, execution: &str, now: i64) -> Result<Dispatch> {
        self.connection.execute(
            "UPDATE dispatch SET state='acknowledged', acknowledged_at=COALESCE(acknowledged_at, ?) WHERE execution=?",
            params![now, execution],
        )?;
        self.dispatch(execution)?
            .ok_or(Error::Invalid("missing dispatch"))
    }
}
