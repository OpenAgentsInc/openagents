use super::super::super::{Execution, Status, Store, Task};
use super::super::{NewGoal, PLAN_SCHEMA, Repository, Seat, parse_route};
use super::*;
use std::path::PathBuf;

/// A private scratch directory: the task store, the host root, and the
/// repository path all live under it, never under the real home.
struct Scratch {
    _dir: tempfile::TempDir,
    store: PathBuf,
    root: PathBuf,
    repository: Repository,
}

fn scratch() -> Scratch {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let store = dir.path().join("tasks");
    let root = dir.path().join("host");
    let repository = Repository {
        label: "demo".into(),
        path: dir.path().join("repo").to_string_lossy().into_owned(),
    };
    Scratch {
        _dir: dir,
        store,
        root,
        repository,
    }
}

/// The real inbox, with task states a test sets as an owner that ran
/// them would.
struct Tasks {
    store: Store,
    forced: BTreeMap<String, (Status, Execution)>,
    /// Tasks whose turn ended with this result ending.
    ended: BTreeMap<String, String>,
}

impl Tasks {
    fn force(&mut self, task: &str, status: Status, execution: Execution) {
        self.forced.insert(task.into(), (status, execution));
    }

    /// End `task`'s turn with `ending`, as its owner would record it.
    fn end(&mut self, task: &str, ending: &str) {
        self.force(task, Status::Finished, Execution::Finished);
        self.ended.insert(task.into(), ending.into());
    }
}

impl Inbox for Tasks {
    fn apply(&mut self, command: &[u8]) -> Result<(), super::super::super::Error> {
        Inbox::apply(&mut self.store, command)
    }

    fn task(&self, task: &str) -> Option<Task> {
        let mut found = self.store.show(task).ok()?;
        if let Some((status, execution)) = self.forced.get(task) {
            found.status = *status;
            found.execution = *execution;
        }
        if let Some(ending) = self.ended.get(task) {
            found.run = Some(super::super::super::studio_sim::run(&found, ending));
        }
        Some(found)
    }
}

fn seat(name: &str, role: Role, route: &str, desk: u32) -> Seat {
    Seat {
        name: name.into(),
        role,
        route: parse_route(route).unwrap(),
        look: "default".into(),
        desk,
    }
}

/// A studio with a lead and two workers, and a goal whose lead finished
/// with no plan accepted yet.
fn team(scratch: &Scratch) -> (Tasks, Studio, String) {
    let mut tasks = Tasks {
        store: Store::open(&scratch.store).unwrap(),
        forced: BTreeMap::new(),
        ended: BTreeMap::new(),
    };
    let mut studio = Studio::open(&scratch.store)
        .unwrap()
        .with_host_root(&scratch.root);
    studio
        .set_seat(seat("lead", Role::Lead, "codex:gpt-6-luna", 0))
        .unwrap();
    studio
        .set_seat(seat("ada", Role::Worker, "claude:claude-opus-5-5", 1))
        .unwrap();
    studio
        .set_seat(seat("grace", Role::Worker, "grok:default", 2))
        .unwrap();
    let (goal, released) = studio
        .submit_goal(
            &mut tasks,
            NewGoal {
                text: "Add a --verbose flag and document it.".into(),
                repository: scratch.repository.clone(),
                lead: None,
            },
            1_790_000_000,
        )
        .unwrap();
    tasks.force(&released.task_id, Status::Finished, Execution::Finished);
    (tasks, studio, goal)
}

/// `a` on ada, then `b` after `a` on grace.
fn plan() -> String {
    serde_json::json!({
        "schema": PLAN_SCHEMA,
        "tasks": [
            {"id": "a", "title": "Parse the flag", "seat": "ada"},
            {"id": "b", "title": "Document the flag", "depends_on": ["a"], "seat": "grace"},
        ],
    })
    .to_string()
}

fn no_reply(_: &str) -> Option<String> {
    None
}

fn slot(studio: &Studio, entry: &str) -> super::super::Slot {
    studio.state().goals[0]
        .plan
        .iter()
        .find(|item| item.id == entry)
        .unwrap()
        .slot
        .clone()
}

#[test]
fn a_paused_seat_keeps_its_work_held_until_it_resumes() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio.pause_seat("ada").unwrap();
    assert!(studio.paused().contains("ada"));
    let PlanOutcome::Accepted { released } = studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap()
    else {
        panic!("the plan is valid");
    };
    assert!(released.is_empty());
    assert_eq!(slot(&studio, "a").state, SlotState::Held);
    // Reconciling leaves a paused seat's work held too.
    assert!(
        studio
            .reconcile(&mut tasks, 2, &no_reply)
            .unwrap()
            .is_empty()
    );
    let released = studio.resume_seat(&mut tasks, "ada", 3).unwrap();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].seat, "ada");
    assert!(studio.paused().is_empty());
    // The pause survives a restart, and an unknown seat is refused.
    studio.pause_seat("grace").unwrap();
    drop(studio);
    let mut studio = Studio::open(&scratch.store).unwrap();
    assert!(studio.paused().contains("grace"));
    assert!(matches!(
        studio.pause_seat("nobody"),
        Err(Error::UnknownSeat(_))
    ));
}

#[test]
fn stopping_a_seat_cancels_its_task_and_returns_it_to_the_board() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap();
    let before = slot(&studio, "a");
    let queued = before.task_id.clone();
    let returned = studio.stop_seat(&mut tasks, "ada").unwrap();
    assert_eq!(returned, vec![next_id(&before).unwrap().0]);
    assert_eq!(slot(&studio, "a").attempt, 2);
    assert_eq!(
        tasks.task(&queued).unwrap().status,
        Status::Cancelled,
        "the queued task is cancelled in the inbox"
    );
    let now = slot(&studio, "a");
    assert_eq!(
        (now.task_id.as_str(), now.state),
        (returned[0].as_str(), SlotState::Held)
    );
    assert!(studio.paused().contains("ada"));
    // Resuming releases it again under its new identity.
    let released = studio.resume_seat(&mut tasks, "ada", 2).unwrap();
    assert_eq!(released[0].task_id, returned[0]);
    assert!(tasks.task(&returned[0]).is_some());
}

#[test]
fn only_planned_tasks_move_between_seats_or_ahead() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap();
    let started = slot(&studio, "a").task_id;
    let planned = slot(&studio, "b").task_id;
    studio.reassign(&planned, "ada").unwrap();
    assert_eq!(slot(&studio, "b").seat, "ada");
    assert!(matches!(
        studio.reassign(&started, "grace"),
        Err(Error::State(_))
    ));
    assert!(matches!(
        studio.reassign(&planned, "nobody"),
        Err(Error::UnknownSeat(_))
    ));
    let lead = studio.state().goals[0].lead.task_id.clone();
    assert!(matches!(
        studio.reassign(&lead, "ada"),
        Err(Error::Invalid(_))
    ));
    studio.prioritize(&planned).unwrap();
    assert_eq!(studio.state().goals[0].plan[0].id, "b");
    assert!(matches!(studio.prioritize(&started), Err(Error::State(_))));
    assert!(matches!(studio.prioritize(&lead), Err(Error::Invalid(_))));
    assert!(matches!(
        studio.prioritize("studio-nothing"),
        Err(Error::Tasks(super::super::super::Error::NotFound))
    ));
}

#[test]
fn a_cancelled_task_is_retried_under_a_new_identity() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap();
    // A planned task is cancelled in the inbox without being started.
    let planned = slot(&studio, "b").task_id;
    studio.cancel_task(&mut tasks, &planned, 2).unwrap();
    assert_eq!(tasks.task(&planned).unwrap().status, Status::Cancelled);
    assert_eq!(slot(&studio, "b").state, SlotState::Submitted);
    assert!(matches!(
        studio.cancel_task(&mut tasks, &planned, 3),
        Err(Error::State(_))
    ));
    // A queued one is cancelled there too, then retried.
    let before = slot(&studio, "a");
    let queued = before.task_id.clone();
    studio.cancel_task(&mut tasks, &queued, 4).unwrap();
    let fresh = studio.retry_task(&mut tasks, &queued, 5).unwrap();
    assert_eq!(fresh, next_id(&before).unwrap().0);
    assert_eq!(slot(&studio, "a").attempt, 2);
    let now = slot(&studio, "a");
    assert_eq!(
        (now.task_id, now.state),
        (fresh.clone(), SlotState::Submitted)
    );
    assert!(tasks.task(&fresh).is_some());
    // A task that did not fail is not retried.
    assert!(matches!(
        studio.retry_task(&mut tasks, &fresh, 6),
        Err(Error::State(_))
    ));
}

#[test]
fn a_plan_decision_is_answered_with_a_plan_at_its_sequence() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    // The lead finished with no plan in its reply.
    studio.reconcile(&mut tasks, 1, &no_reply).unwrap();
    let sequence = studio.state().goals[0].decision.as_ref().unwrap().sequence;
    assert!(matches!(
        studio.answer_goal(&mut tasks, &goal, sequence + 1, &plan(), 2),
        Err(Error::Tasks(super::super::super::Error::RevisionMismatch))
    ));
    let PlanOutcome::Accepted { released } = studio
        .answer_goal(&mut tasks, &goal, sequence, &plan(), 3)
        .unwrap()
    else {
        panic!("the plan is valid");
    };
    assert_eq!(released.len(), 1);
    assert!(studio.state().goals[0].decision.is_none());
    assert!(matches!(
        studio.answer_goal(&mut tasks, &goal, sequence, &plan(), 4),
        Err(Error::State(_))
    ));
}

#[test]
fn a_rejection_is_remembered_and_an_intent_answers_once() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap();
    let task = slot(&studio, "a").task_id;
    studio
        .reject(&mut tasks, &task, &"c".repeat(40), "Out of scope.")
        .unwrap();
    assert_eq!(tasks.task(&task).unwrap().status, Status::Cancelled);
    let entry = studio.state().memory.last().unwrap();
    assert_eq!(entry.kind, MemoryKind::Decision);
    assert!(entry.text.contains("cccccccccccc") && entry.text.contains("Out of scope."));
    // The ledger keeps each request's reference, the newest within bound.
    assert_eq!(studio.answered("r1"), None);
    studio.record_answer("r1", &goal).unwrap();
    assert_eq!(studio.answered("r1").as_deref(), Some(goal.as_str()));
    for n in 0..MAX_INTENTS {
        studio.record_answer(&format!("k{n}"), "x").unwrap();
    }
    assert_eq!(studio.answered("r1"), None);
    assert_eq!(studio.answered("k0").as_deref(), Some("x"));
}

#[test]
fn a_returned_task_counts_its_attempts_under_hex_identities() {
    let first = super::super::Slot {
        task_id: "a".repeat(64),
        seat: "ada".into(),
        state: super::super::SlotState::Held,
        command: None,
        attempt: 1,
    };
    let (second, attempt) = next_id(&first).unwrap();
    assert_eq!(attempt, 2);
    assert_eq!(second.len(), 64);
    assert!(
        second
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_ne!(second, first.task_id);
    // The same slot always returns to the same identity.
    assert_eq!(next_id(&first).unwrap().0, second);
    // A slot recorded before attempts were counted is its first attempt.
    let legacy = super::super::Slot {
        attempt: 0,
        ..first.clone()
    };
    assert_eq!(next_id(&legacy).unwrap(), (second, 2));
    let spent = super::super::Slot {
        attempt: 99,
        ..first
    };
    assert!(matches!(next_id(&spent), Err(Error::LimitExceeded(_))));
}

#[test]
fn the_wire_view_is_bounded_and_shows_pauses_decisions_and_tasks() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio.reconcile(&mut tasks, 1, &no_reply).unwrap();
    studio.pause_seat("grace").unwrap();
    let mut view = studio.wire(&tasks, &scratch.store);
    view.validate().unwrap();
    let before = view.clone();
    view.canonicalize();
    assert_eq!(view, before, "the wire view is already in key order");
    assert_eq!(view.goals[0].goal, goal);
    assert_eq!(view.goals[0].status, wire::GoalStatus::Decision);
    assert_eq!(view.tasks.len(), 1);
    assert_eq!(view.tasks[0].entry, "lead");
    assert_eq!(view.decisions[0].kind, wire::DecisionKind::NoPlan);
    assert_eq!(view.decisions[0].decision, goal);
    let grace = view.seats.iter().find(|seat| seat.seat == "grace").unwrap();
    assert!(grace.paused);
    assert_eq!(
        (grace.activity, grace.station),
        (wire::Activity::Paused, wire::Station::Lounge)
    );
    assert_eq!(view.repositories[0].workspace, "demo");
    // Accepting the plan puts its tasks on the board.
    studio
        .answer_goal(
            &mut tasks,
            &goal,
            studio.state().goals[0].decision.as_ref().unwrap().sequence,
            &plan(),
            2,
        )
        .unwrap();
    let view = studio.wire(&tasks, &scratch.store);
    view.validate().unwrap();
    assert_eq!(view.tasks.len(), 3);
    let a = view.tasks.iter().find(|task| task.entry == "a").unwrap();
    assert_eq!((a.status, a.position), (wire::TaskStatus::Queued, 1));
    let b = view.tasks.iter().find(|task| task.entry == "b").unwrap();
    assert_eq!(b.depends_on, vec!["a".to_owned()]);
    assert_eq!(view.repositories[0].open_tasks, 1);
    // The accepted plan is the pinned memory entry.
    let pinned = view.plan().expect("the plan is pinned");
    assert_eq!(pinned.kind, wire::MemoryKind::Plan);
    assert_eq!(pinned.goal.as_deref(), Some(goal.as_str()));
    assert!(pinned.text.starts_with("Plan for goal"), "{}", pinned.text);
}

#[test]
fn the_wire_view_keeps_the_newest_memory_and_the_pinned_plan() {
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio.reconcile(&mut tasks, 1, &no_reply).unwrap();
    studio
        .answer_goal(
            &mut tasks,
            &goal,
            studio.state().goals[0].decision.as_ref().unwrap().sequence,
            &plan(),
            2,
        )
        .unwrap();
    let plan_entry = studio
        .state()
        .memory
        .iter()
        .find(|item| item.kind == MemoryKind::Plan)
        .map(|item| item.sequence)
        .unwrap();
    for n in 0..wire::MAX_MEMORY + 3 {
        studio
            .remember(
                MemoryKind::Convention,
                Party::Person,
                None,
                &format!("Convention {n}: keep the palette amber."),
            )
            .unwrap();
    }
    let view = studio.wire(&tasks, &scratch.store);
    view.validate().unwrap();
    assert_eq!(view.memory.len(), wire::MAX_MEMORY);
    let pinned = view.plan().expect("the plan stays pinned");
    assert_eq!(pinned.entry, wire::memory_key(plan_entry));
    assert_eq!(view.memory.iter().filter(|entry| entry.pinned).count(), 1);
    let newest = view.memory.last().unwrap();
    assert!(
        newest
            .text
            .starts_with(&format!("Convention {}", wire::MAX_MEMORY + 2))
    );
    assert_eq!(newest.author, "the person");
}

#[test]
fn a_waiting_decision_is_named_from_host_state() {
    use super::super::super::interaction::{APPROVAL_ENDING, QUESTION_ENDING};
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    // The lead finished with no plan: the goal's decision raises a summary
    // of its own, never under a task's identity.
    studio.reconcile(&mut tasks, 1, &no_reply).unwrap();
    let sequence = studio.state().goals[0].decision.as_ref().unwrap().sequence;
    let open = studio.goal_decisions();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].subject, super::super::goal_subject(&goal));
    assert_ne!(open[0].subject, studio.state().goals[0].lead.task_id);
    assert_eq!(open[0].sequence, sequence);
    assert_eq!(open[0].headline, "lead finished without a plan");
    // Answering it closes it.
    studio
        .answer_goal(&mut tasks, &goal, sequence, &plan(), 2)
        .unwrap();
    assert!(studio.goal_decisions().is_empty());
    // A task's approval or question names its seat and title.
    let a = slot(&studio, "a").task_id;
    assert_eq!(studio.decision_headline(&tasks, &a), None);
    tasks.end(&a, APPROVAL_ENDING);
    assert_eq!(
        studio.decision_headline(&tasks, &a).as_deref(),
        Some("ada asks for approval: Parse the flag")
    );
    tasks.end(&a, QUESTION_ENDING);
    assert_eq!(
        studio.decision_headline(&tasks, &a).as_deref(),
        Some("ada has a question: Parse the flag")
    );
    let lead = studio.state().goals[0].lead.task_id.clone();
    tasks.end(&lead, APPROVAL_ENDING);
    assert_eq!(
        studio.decision_headline(&tasks, &lead).as_deref(),
        Some("lead asks for approval on the plan")
    );
    // A task the studio does not hold has none.
    assert_eq!(studio.decision_headline(&tasks, &"f".repeat(64)), None);
}

#[test]
fn an_approval_binds_the_answering_device_to_the_exact_step_once() {
    use super::super::super::interaction::{APPROVAL_ENDING, QUESTION_ENDING};
    use super::super::approvals::{Action, Approver, Verdict};
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap();
    let a = slot(&studio, "a").task_id;
    tasks.end(&a, APPROVAL_ENDING);
    let record = tasks.task(&a).unwrap();
    let action = Action::of(&record).unwrap();
    assert_eq!(
        (action.task.as_str(), action.revision, action.turn),
        (a.as_str(), record.revision, 1)
    );
    let device = Approver {
        device: "d".repeat(64),
        grant: Some("grant-1".into()),
        epoch: Some(2),
    };
    let bound = studio
        .bind_approval(&action, device.clone(), "Approved.", "c1", 5)
        .unwrap();
    assert_eq!(bound.verdict, Verdict::Approve);
    studio.consume_approval(&bound.subject, "c1", 6).unwrap();
    let held = studio.approvals(&a).unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].approver, device);
    assert_eq!(held[0].consumed_at, Some(6));
    // Single use: another answer to the same step refuses.
    assert!(matches!(
        studio.bind_approval(&action, device, "Approved.", "c2", 7),
        Err(Error::State(_))
    ));
    // A question asks no approval, so it binds no approver.
    tasks.end(&a, QUESTION_ENDING);
    assert!(Action::of(&tasks.task(&a).unwrap()).is_none());
}

#[test]
fn a_seat_message_says_whether_its_engine_read_it() {
    use super::super::super::steer;
    let scratch = scratch();
    let (mut tasks, mut studio, goal) = team(&scratch);
    studio
        .accept_plan(&mut tasks, &goal, plan().as_bytes(), 1)
        .unwrap();
    let a = slot(&studio, "a").task_id;
    tasks.force(&a, Status::Running, Execution::Running);
    let ada = || Party::Seat { name: "ada".into() };
    let sent = studio
        .message(&tasks, Party::Person, ada(), "Use tabs.", 2)
        .unwrap();
    assert_eq!(sent[0].steer, Some(1));
    // Accepted for the running turn, not read yet.
    let view = studio.wire(&tasks, &scratch.store);
    view.validate().unwrap();
    let shown = &view.messages[0];
    assert_eq!(
        (shown.mode, shown.state, shown.task.as_deref()),
        (
            wire::DeliveryMode::MidTurn,
            wire::DeliveryState::Accepted,
            Some(a.as_str())
        )
    );
    assert_eq!(shown.from, None);
    // The turn reads it between steps.
    assert_eq!(steer::take(studio.store(), &a), ["Use tabs."]);
    let view = studio.wire(&tasks, &scratch.store);
    assert_eq!(view.messages[0].state, wire::DeliveryState::Consumed);
    // A message the turn ended before reading returns to the seat's next
    // briefing, and leaves the task's steering.
    studio
        .message(&tasks, Party::Person, ada(), "Keep the README.", 3)
        .unwrap();
    tasks.force(&a, Status::Finished, Execution::Finished);
    studio.reconcile(&mut tasks, 4, &no_reply).unwrap();
    let returned = studio
        .state()
        .messages
        .iter()
        .find(|message| message.text == "Keep the README.")
        .unwrap();
    assert_eq!(returned.delivery, Delivery::Waiting);
    assert_eq!(returned.steer, None);
    assert!(steer::take(studio.store(), &a).is_empty());
    let view = studio.wire(&tasks, &scratch.store);
    view.validate().unwrap();
    let shown = view
        .messages
        .iter()
        .find(|message| message.text == "Keep the README.")
        .unwrap();
    assert_eq!(
        (shown.mode, shown.state, shown.task.as_deref()),
        (
            wire::DeliveryMode::TurnBoundary,
            wire::DeliveryState::Waiting,
            None
        )
    );
    // The message that was read stays read.
    let read = view
        .messages
        .iter()
        .find(|message| message.text == "Use tabs.")
        .unwrap();
    assert_eq!(read.state, wire::DeliveryState::Consumed);
}

#[test]
fn a_log_line_never_carries_a_calls_arguments_or_output() {
    let call = atif::Step::called(atif::Call {
        id: "1".into(),
        name: "shell".into(),
        arguments: serde_json::json!({"command": "cat secret.env"}),
        output: "TOKEN=hunter2".into(),
        outcome: atif::Outcome::Completed,
        milliseconds: 3,
        purpose: Some("Look around\nand more".into()),
        extra: serde_json::Map::new(),
    });
    let line = disclose(&call).unwrap();
    assert_eq!(line.text, "running: shell — Look around");
    assert_eq!(line.activity, wire::Activity::Running);
    assert!(!line.text.contains("secret") && !line.text.contains("hunter2"));
    let said = atif::Step::said(atif::Source::Agent, "\nI will add the flag.\nThen docs.");
    assert_eq!(disclose(&said).unwrap().text, "I will add the flag.");
    assert!(disclose(&atif::Step::said(atif::Source::User, "my prompt")).is_none());
    assert!(disclose(&atif::Step::said(atif::Source::Agent, "  ")).is_none());
}
