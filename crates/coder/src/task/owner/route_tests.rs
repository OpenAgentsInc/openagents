//! Router phase 1's exit evidence (#10207; plan section 10): one message,
//! routed by the shared route policy (`openagents_chat::route`), admitted,
//! journaled, and dispatched into this task owner in a scratch store, with
//! a retained patch and an independent check. Synthetic fixtures prove
//! successful delivery, a denied grant, cancellation, and an executor crash
//! without duplicate effects.
//!
//! The dispatcher here is the client's (`openagents_chat::client`) in
//! miniature: the admission is journaled before the executor is asked, the
//! task is named in the record before it runs, a request whose record
//! already names a task is never started again, and the record's states
//! are the owner's dispositions projected (`task::lifecycle`), never a
//! second state machine.

use std::path::Path;
use std::time::{Duration, Instant};

use openagents_chat::route::{self, Journal, Reading, Situation, THIS_COMPUTER};
use openagents_chat::router::{Meta, Offer};
use route_contract::lifecycle::{CheckLabel, Lifecycle};
use route_contract::record::RouteRecord;
use route_contract::route::{RefusalReason, RouteFamily};
use route_contract::snapshot::{CheckScope, Deliverable, Surface, WorkspaceBinding};

use super::tests::{fixture, requirements, settled};
use super::*;
use crate::task::lifecycle;

const THREAD: &str = "thread-1";
const TEXT: &str = "Write one output.";
/// Every run appends one byte here, so a duplicate effect shows as two.
const EFFECTS: &str = "effects.txt";

/// The message routed and admitted: a Coder route, journaled as admitted
/// before anything runs. The current grant epoch is the snapshot's.
fn admitted(store: &Path, request: &str) -> Journal {
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        ..Meta::default()
    };
    let situation = Situation {
        surface: Surface::Terminal,
        caller: "local:openagents-terminal".into(),
        request: request.into(),
        thread: Some(THREAD.into()),
        computer: THIS_COMPUTER.into(),
        project: Some(WorkspaceBinding {
            project: "fixture".into(),
            path: None,
        }),
        ready: true,
        bound: None,
        check: CheckScope::IndependentSuite,
    };
    let reading = Reading {
        meta: Some(&meta),
        computer_lane: false,
        text: TEXT,
        reply: "Writing one output.",
    };
    let result = route::propose(&reading, &situation, &|_| None);
    assert_eq!(result.family(), RouteFamily::Coder);
    let snapshot = route::admit(&result, &situation, Some(&meta), TEXT, None);
    assert_eq!(snapshot.evidence.check, CheckScope::IndependentSuite);
    assert_eq!(
        snapshot.evidence.deliverables,
        [Deliverable::Patch, Deliverable::RetainedArtifacts]
    );
    assert!(!snapshot.money.shown, "our apps record cost, never show it");
    let mut record = RouteRecord::received(
        request,
        Some(THREAD.into()),
        result,
        snapshot,
        route::now_ms(),
    )
    .unwrap();
    record
        .step(Lifecycle::Admitted, "autostart", route::now_ms())
        .unwrap();
    let journal = Journal::beside(store);
    journal.write(&record).unwrap();
    journal
}

/// What a dispatch did.
#[derive(Debug, PartialEq, Eq)]
enum Dispatch {
    /// The route was refused before the executor was asked.
    Refused,
    /// The request's task already started: followed, not started again.
    Followed,
    /// The executor was asked once, with this outcome.
    Executed(Result<Task, String>),
}

/// The host's dispatch of `request`'s route, as the client does it.
async fn dispatch(
    dir: &Path,
    journal: &Journal,
    request: &str,
    grant: &Grant,
    current_epoch: u64,
) -> Dispatch {
    let mut record = journal.latest(THREAD, request).expect("routed");
    if record.dispatched_any() {
        return Dispatch::Followed;
    }
    // Current rights are checked again at dispatch: a grant revoked since
    // admission refuses, and nothing reaches the executor.
    let admitted_epoch = record.snapshot.placement.grant.as_ref().map(|g| g.epoch);
    if admitted_epoch != Some(current_epoch) {
        record
            .refuse(
                Some(RefusalReason::MissingGrant),
                "grant_revoked",
                route::now_ms(),
            )
            .unwrap();
        journal.write(&record).unwrap();
        return Dispatch::Refused;
    }
    // The dispatch intent names the task before the executor is asked.
    record.dispatched(&grant.task_id, Some("fixture")).unwrap();
    journal.write(&record).unwrap();
    let executed = execute(dir, &serde_json::to_vec(grant).unwrap()).await;
    // The owner refused the grant before admitting a run: the queued task
    // is cancelled through the owner, so its disposition says so.
    if executed.is_err() {
        let mut store = Store::open(dir).unwrap();
        let task = store.show(&grant.task_id).unwrap();
        if task.run.is_none() {
            let cancel = Command {
                schema: COMMAND_SCHEMA.into(),
                command_id: format!("refuse-{request}"),
                task_id: task.task_id.clone(),
                expected_revision: Some(task.revision),
                action: Action::Cancel {
                    reason: "The execution grant was refused.".into(),
                },
            };
            store.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
        }
    }
    Dispatch::Executed(executed.map_err(|error| format!("{error:?}")))
}

/// Copy the owner's disposition of the route's tasks into its record.
fn observed(dir: &Path, journal: &Journal, request: &str) -> RouteRecord {
    let mut record = journal.latest(THREAD, request).unwrap();
    for task in record
        .tasks()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>()
    {
        let seen = lifecycle::observe(dir, &task).expect("the task reads");
        record.observe(&task, seen, route::now_ms());
    }
    journal.write(&record).unwrap();
    record
}

fn counted(workspace: &str) -> usize {
    std::fs::read_to_string(Path::new(workspace).join(EFFECTS)).map_or(0, |text| text.len())
}

/// Delivery: one routed message starts exactly one admitted task, which
/// leaves a retained patch; the independent check passes, so the route is
/// completed and verified, with the run's wall time and its cost recorded
/// (unknown for this fixture's bounded command, never a stand-in zero).
#[tokio::test]
async fn a_routed_message_delivers_a_retained_patch_with_an_independent_check() {
    use crate::capability::Trust;
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.arguments[1] = format!("printf x >> {EFFECTS}; printf output > result.txt");
    grant.requirements = Some(requirements(
        root.path(),
        r#"test "$(cat result.txt)" = output || exit 2; emit passed "$1""#,
    ));
    let journal = admitted(&dir, "req-ok");
    let Dispatch::Executed(Ok(task)) = dispatch(&dir, &journal, "req-ok", &grant, 0).await else {
        panic!("not executed");
    };
    assert_eq!(task.execution, Execution::Finished);
    let record = observed(&dir, &journal, "req-ok");
    assert_eq!(record.state, Lifecycle::Completed);
    assert_eq!(record.runs[0].projection.check, CheckLabel::Unchecked);
    let checked = check(&dir, "task-one", &Trust::everything()).await.unwrap();
    assert_eq!(checked.checks, Checks::Passed);
    let record = observed(&dir, &journal, "req-ok");
    assert_eq!(record.state, Lifecycle::Completed);
    assert_eq!(record.runs.len(), 1, "exactly one task");
    let run = &record.runs[0];
    assert_eq!(run.task, "task-one");
    assert_eq!(run.projection.check, CheckLabel::Verified);
    // The retained patch (the run's output) and its trace, by digest.
    assert_eq!(
        artifact::read(&dir, "task-one", Path::new("result.txt")).unwrap(),
        b"output"
    );
    let result = checked.run.as_ref().unwrap().result.as_ref().unwrap();
    assert!(result.artifact_digest.is_some());
    assert_eq!(run.artifacts.len(), 2, "{:?}", run.artifacts);
    assert_eq!(run.wall_ms, Some(result.elapsed_ms));
    assert_eq!(run.cost_microusd, None);
    assert_eq!(record.cost_microusd(), None);
    assert!(record.wall_ms().is_some());
    // The record's moves: admitted by the router, then the owner's.
    let moves: Vec<_> = record
        .transitions
        .iter()
        .map(|transition| (transition.from, transition.to))
        .collect();
    assert_eq!(
        moves,
        [
            (Lifecycle::Received, Lifecycle::Admitted),
            (Lifecycle::Admitted, Lifecycle::DispatchPending),
            (Lifecycle::DispatchPending, Lifecycle::Completed),
        ]
    );
    // Asking again for the same message follows it; nothing runs twice.
    assert_eq!(
        dispatch(&dir, &journal, "req-ok", &grant, 0).await,
        Dispatch::Followed
    );
    assert_eq!(counted(&task.intent.workspace.path), 1);
}

/// A denied grant: revoked between admission and dispatch, the route is
/// refused before the executor is asked; a grant the owner refuses (it no
/// longer matches the task) admits no run, and the task ends cancelled.
/// Neither runs anything.
#[tokio::test]
async fn a_denied_grant_runs_nothing() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.arguments[1] = format!("printf x >> {EFFECTS}");
    let journal = admitted(&dir, "req-revoked");
    assert_eq!(
        dispatch(&dir, &journal, "req-revoked", &grant, 1).await,
        Dispatch::Refused
    );
    let record = journal.latest(THREAD, "req-revoked").unwrap();
    assert_eq!(record.state, Lifecycle::Failed);
    assert_eq!(record.refusal, Some(RefusalReason::MissingGrant));
    assert!(record.runs.is_empty());
    let queued = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert!(queued.run.is_none());
    assert_eq!(counted(&queued.intent.workspace.path), 0);

    // The owner's own check: a grant for another intent is refused before
    // admission.
    let journal = admitted(&dir, "req-stale");
    grant.intent_digest = format!("sha256:{}", "0".repeat(64));
    let Dispatch::Executed(Err(_)) = dispatch(&dir, &journal, "req-stale", &grant, 0).await else {
        panic!("the owner admitted a stale grant");
    };
    let record = observed(&dir, &journal, "req-stale");
    assert_eq!(record.state, Lifecycle::Cancelled);
    let task = Store::open(&dir).unwrap().show("task-one").unwrap();
    assert!(task.run.is_none());
    assert_eq!(counted(&task.intent.workspace.path), 0);
}

/// Cancellation: requested while the run works, it is reported apart from
/// the acknowledged cancellation, and the run ends stopped, before its
/// later effect.
#[tokio::test]
async fn a_cancelled_route_ends_cancelled_without_its_later_effects() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.arguments[1] = format!("printf x >> {EFFECTS}; sleep 20; printf late > late.txt");
    let journal = admitted(&dir, "req-cancel");
    let (run_dir, run_journal, run_grant) = (dir.clone(), journal.clone(), grant.clone());
    let handle =
        tokio::spawn(
            async move { dispatch(&run_dir, &run_journal, "req-cancel", &run_grant, 0).await },
        );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "the run did not start");
        let mut store = Store::open(&dir).unwrap();
        let task = store.show("task-one").unwrap();
        if task.run.as_ref().is_some_and(|run| run.effect_id.is_some()) {
            let cancel = Command {
                schema: COMMAND_SCHEMA.into(),
                command_id: "cancel-route".into(),
                task_id: task.task_id,
                expected_revision: Some(task.revision),
                action: Action::Cancel {
                    reason: "The person stopped it.".into(),
                },
            };
            store.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
            break;
        }
        drop(store);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let requested = observed(&dir, &journal, "req-cancel");
    if requested.state == Lifecycle::Running {
        assert!(requested.runs[0].projection.cancel_requested);
    }
    let Dispatch::Executed(Ok(task)) = handle.await.unwrap() else {
        panic!("not executed");
    };
    assert_eq!(task.execution, Execution::Stopped);
    let record = observed(&dir, &journal, "req-cancel");
    assert_eq!(record.state, Lifecycle::Cancelled);
    assert!(!record.runs[0].projection.cancel_requested);
    assert!(record.settled());
    let workspace = Path::new(&task.intent.workspace.path);
    assert!(!workspace.join("late.txt").exists());
    assert_eq!(counted(&task.intent.workspace.path), 1);
}

/// An executor crash after its command was dispatched: the route waits for
/// reconciliation or ends failed once recovery settles it, and asking
/// again for the same message never runs the command again.
#[tokio::test]
async fn an_executor_crash_never_duplicates_effects() {
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.arguments[1] = format!("printf x >> {EFFECTS}; printf output > result.txt");
    let journal = admitted(&dir, "req-crash");
    super::OWNER_FAULT.with(|fault| fault.set(Some("after_dispatch")));
    let Dispatch::Executed(Err(_)) = dispatch(&dir, &journal, "req-crash", &grant, 0).await else {
        panic!("the owner did not crash");
    };
    let crashed = observed(&dir, &journal, "req-crash");
    assert!(
        matches!(
            crashed.state,
            Lifecycle::NeedsReconciliation | Lifecycle::Running | Lifecycle::Failed
        ),
        "{:?}",
        crashed.state
    );
    // Recovery ends it; it reruns nothing.
    let recovered = settled(&dir, "task-one").await;
    assert_eq!(recovered.execution, Execution::Failed);
    let record = observed(&dir, &journal, "req-crash");
    assert_eq!(record.state, Lifecycle::Failed);
    assert!(record.settled());
    // The same message again: followed, never dispatched again, and the
    // owner refuses a second execution of the same grant anyway.
    assert_eq!(
        dispatch(&dir, &journal, "req-crash", &grant, 0).await,
        Dispatch::Followed
    );
    assert!(
        execute(&dir, &serde_json::to_vec(&grant).unwrap())
            .await
            .is_err()
    );
    assert_eq!(record.runs.len(), 1);
    assert!(counted(&recovered.intent.workspace.path) <= 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
}

/// The task's owner lock, as the run's owner holds it. A child process
/// another test forks while holding its own lock can keep one busy for a
/// moment, so this waits.
fn hold(dir: &Path, id: &str) -> Owner {
    let store = Store::open_for_owner(dir).unwrap();
    let started = Instant::now();
    loop {
        match Owner::acquire(&store, id) {
            Ok(owner) => return owner,
            Err(Error::Busy) if started.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("{error:?}"),
        }
    }
}

/// A local run (#10232): its grant carries the host's own suite, the run's
/// end lists the recipe's frozen checks, and the independent check on the
/// candidate ends the route verified when they pass, check_failed when
/// one fails, and unchecked when nothing was listed.
#[cfg(unix)]
#[tokio::test]
async fn a_local_runs_listed_checks_end_its_route_verified_or_check_failed() {
    use crate::task::local_checks;
    for (frozen, expected, label) in [
        (
            Some(r#"test "$(cat result.txt)" = output"#),
            Checks::Passed,
            CheckLabel::Verified,
        ),
        (
            Some(r#"test "$(cat result.txt)" = other"#),
            Checks::Failed,
            CheckLabel::CheckFailed,
        ),
        (None, Checks::NotRun, CheckLabel::Unchecked),
    ] {
        let (root, _workspace, mut grant) = fixture();
        let dir = root.path().join("store");
        grant.arguments[1] = "printf output > result.txt".into();
        grant.requirements = local_checks::requirements(
            &root.path().join("grants"),
            &grant.task_id,
            grant.expected_revision,
        )
        .unwrap();
        let journal = admitted(&dir, "req-local");
        let Dispatch::Executed(Ok(task)) = dispatch(&dir, &journal, "req-local", &grant, 0).await
        else {
            panic!("not executed");
        };
        assert_eq!(task.execution, Execution::Finished);
        let run = task.run.as_ref().unwrap();
        let listed = local_checks::list(
            grant.requirements.as_ref().unwrap(),
            &run.admission.workspace,
            &run.admission.source_revision,
            &frozen.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            &[],
        );
        assert_eq!(listed.is_some(), frozen.is_some());
        if listed.is_some() {
            // The run's owner records the intent as the run ends, then
            // runs the check.
            {
                let owner = hold(&dir, &grant.task_id);
                std::thread::scope(|scope| {
                    scope.spawn(|| assert!(local_checks::pending(&dir, &grant.task_id)));
                });
                owner.record(Event::CheckIntent).unwrap();
            }
            let checked = local_checks::complete(&dir, &grant.task_id).await.unwrap();
            assert_eq!(checked.checks, expected);
        }
        assert!(!local_checks::pending(&dir, &grant.task_id));
        let record = observed(&dir, &journal, "req-local");
        assert_eq!(record.runs[0].projection.check, label);
    }
}

/// A local check whose owner is gone is not waited on: it ends
/// unavailable (#10232).
#[cfg(unix)]
#[tokio::test]
async fn a_local_check_whose_owner_is_gone_is_not_waited_on() {
    use crate::task::local_checks;
    let (root, _workspace, mut grant) = fixture();
    let dir = root.path().join("store");
    grant.arguments[1] = "printf output > result.txt".into();
    grant.requirements = local_checks::requirements(
        &root.path().join("grants"),
        &grant.task_id,
        grant.expected_revision,
    )
    .unwrap();
    let task = execute(&dir, &serde_json::to_vec(&grant).unwrap())
        .await
        .unwrap();
    let run = task.run.as_ref().unwrap();
    local_checks::list(
        grant.requirements.as_ref().unwrap(),
        &run.admission.workspace,
        &run.admission.source_revision,
        &["true".into()],
        &[],
    )
    .unwrap();
    {
        let owner = hold(&dir, &grant.task_id);
        // While the owner holds the task, its check is to come.
        std::thread::scope(|scope| {
            scope.spawn(|| assert!(local_checks::pending(&dir, &grant.task_id)));
        });
        owner.record(Event::CheckIntent).unwrap();
    }
    // A child another test forks may hold the lock for a moment.
    let started = Instant::now();
    while local_checks::pending(&dir, &grant.task_id) {
        assert!(started.elapsed() < Duration::from_secs(30));
    }
    let task = Store::open(&dir).unwrap().show(&grant.task_id).unwrap();
    assert_eq!(task.checks, Checks::Unavailable);
}
