//! Route views (#10698): identities, placement, executor, and payers read
//! from the record, and unknown cost, failed checks, and requested
//! cancellation kept apart from settled cost, verified completion, and
//! acknowledged cancellation.

use crate::lifecycle::{Lifecycle, TaskChecks, TaskDisposition, TaskExecution, TaskStatus};
use crate::record::{Observation, RouteRecord};
use crate::route::RouteResult;
use crate::snapshot::Resource;
use crate::tests::{plan, snapshot};
use crate::view::{Cancellation, Cost, Outcome, RouteView, VIEW_SCHEMA};

fn dispatched(tasks: &[&str]) -> RouteRecord {
    let result = RouteResult::Coder { plan: plan() };
    let mut record =
        RouteRecord::received("req_1", Some("th_1".into()), result, snapshot(), 1_000).unwrap();
    record.step(Lifecycle::Admitted, "admit", 1_001).unwrap();
    for (index, task) in tasks.iter().enumerate() {
        let engine = ["codex", "claude", "grok"][index % 3];
        record.dispatched(task, Some(engine)).unwrap();
    }
    record
}

fn seen(
    status: TaskStatus,
    execution: TaskExecution,
    checks: TaskChecks,
    cost: Option<u64>,
) -> Observation {
    Observation {
        disposition: TaskDisposition {
            status,
            execution,
            checks,
        },
        revision: 4,
        cost_microusd: cost,
        wall_ms: Some(10),
        artifacts: Vec::new(),
        payer: Some("theirs".into()),
        payer_keys: Vec::new(),
    }
}

fn finished(checks: TaskChecks, cost: Option<u64>) -> Observation {
    seen(TaskStatus::Finished, TaskExecution::Finished, checks, cost)
}

#[test]
fn a_view_names_the_records_identities_placement_executor_and_payers() {
    let mut record = dispatched(&["task_1"]);
    record.observe("task_1", finished(TaskChecks::Passed, Some(1_500)), 2_000);
    let view = RouteView::of(&record);
    assert_eq!(view.schema, VIEW_SCHEMA);
    assert_eq!(view.request, "req_1");
    assert_eq!(view.thread.as_deref(), Some("th_1"));
    assert_eq!(view.snapshot, record.snapshot_digest);
    assert_eq!(view.computer.as_deref(), Some("cmp_here"));
    assert_eq!(view.grant.as_deref(), Some("grant_1 epoch 3"));
    assert_eq!(view.executor.engines, ["codex"]);
    assert_eq!(view.payers[0].resource, Resource::Executor);
    assert_eq!(view.payers[0].payer, "login:codex");
    assert_eq!(view.payers[1].payer, "openagents");
    assert_eq!(view.runs[0].task, "task_1");
    assert_eq!(view.runs[0].revision, Some(4));
    assert_eq!(view.runs[0].payer.as_deref(), Some("theirs"));
    assert_eq!(view.outcome, Outcome::Verified);
    assert_eq!(view.cost, Cost::Settled { microusd: 1_500 });
    assert_eq!(view.wall_ms, Some(1_000));
    // The view never carries the checkout's path.
    let json = serde_json::to_string(&view).unwrap();
    assert!(!json.contains("/Users/example"));
    let back: RouteView = serde_json::from_str(&json).unwrap();
    assert_eq!(back, view);
}

#[test]
fn unknown_cost_is_never_settled_or_zero() {
    let mut record = dispatched(&["task_1", "task_2"]);
    record.observe("task_1", finished(TaskChecks::Passed, Some(700)), 2_000);
    record.observe("task_2", finished(TaskChecks::Passed, None), 2_000);
    let view = RouteView::of(&record);
    assert_eq!(view.state, Lifecycle::Completed);
    assert_eq!(
        view.cost,
        Cost::Unknown {
            known_microusd: 700,
            missing: 1
        }
    );
    // Every cost known while a run still works is recorded, not settled.
    let mut record = dispatched(&["task_1", "task_2"]);
    record.observe("task_1", finished(TaskChecks::Passed, Some(700)), 2_000);
    record.observe(
        "task_2",
        seen(
            TaskStatus::Running,
            TaskExecution::Running,
            TaskChecks::NotRun,
            Some(5),
        ),
        2_000,
    );
    let view = RouteView::of(&record);
    assert_eq!(view.cost, Cost::Recorded { microusd: 705 });
    assert_eq!(view.outcome, Outcome::InProgress);
}

#[test]
fn only_passing_independent_checks_are_verified() {
    for (checks, want) in [
        (TaskChecks::Passed, Outcome::Verified),
        (TaskChecks::NotRun, Outcome::Unchecked),
        (TaskChecks::Unavailable, Outcome::Unchecked),
        (TaskChecks::Disputed, Outcome::Unchecked),
        (TaskChecks::Failed, Outcome::CheckFailed),
    ] {
        let mut record = dispatched(&["task_1"]);
        record.observe("task_1", finished(checks, Some(1)), 2_000);
        assert_eq!(RouteView::of(&record).outcome, want, "{checks:?}");
    }
    let mut record = dispatched(&["task_1"]);
    record.observe(
        "task_1",
        seen(
            TaskStatus::Finished,
            TaskExecution::Failed,
            TaskChecks::NotRun,
            Some(1),
        ),
        2_000,
    );
    assert_eq!(RouteView::of(&record).outcome, Outcome::Failed);
    // One verified run and one unchecked run is not verified.
    let mut record = dispatched(&["task_1", "task_2"]);
    record.observe("task_1", finished(TaskChecks::Passed, Some(1)), 2_000);
    record.observe("task_2", finished(TaskChecks::NotRun, Some(1)), 2_000);
    assert_eq!(RouteView::of(&record).outcome, Outcome::Unchecked);
}

#[test]
fn a_requested_cancellation_is_not_an_acknowledged_one() {
    let mut record = dispatched(&["task_1"]);
    record.observe(
        "task_1",
        seen(
            TaskStatus::CancelRequested,
            TaskExecution::Running,
            TaskChecks::NotRun,
            None,
        ),
        2_000,
    );
    let view = RouteView::of(&record);
    assert_eq!(view.cancellation, Cancellation::Requested);
    assert_eq!(view.outcome, Outcome::InProgress);
    assert!(view.runs[0].cancel_requested);
    record.observe(
        "task_1",
        seen(
            TaskStatus::Finished,
            TaskExecution::Stopped,
            TaskChecks::NotRun,
            Some(3),
        ),
        3_000,
    );
    let view = RouteView::of(&record);
    assert_eq!(view.cancellation, Cancellation::Acknowledged);
    assert_eq!(view.outcome, Outcome::Cancelled);
    assert_eq!(view.cost, Cost::Settled { microusd: 3 });
}

#[test]
fn an_unknown_task_state_needs_reconciliation_and_reading_changes_nothing() {
    let mut record = dispatched(&["task_1"]);
    record.observe(
        "task_1",
        seen(
            TaskStatus::Unknown,
            TaskExecution::Unknown,
            TaskChecks::NotRun,
            None,
        ),
        2_000,
    );
    let before = record.clone();
    let view = RouteView::of(&record);
    assert_eq!(view.outcome, Outcome::NeedsReconciliation);
    assert_eq!(record, before);
    // A route that never dispatched shows no runs and no charge.
    let fresh = dispatched(&[]);
    let view = RouteView::of(&fresh);
    assert_eq!(view.cost, Cost::None);
    assert!(view.runs.is_empty());
    assert_eq!(view.outcome, Outcome::InProgress);
}
