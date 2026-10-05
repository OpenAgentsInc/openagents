use super::*;

/// A private scratch directory: the task store, the host root, and the
/// repository path all live under it, never under the real home.
fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

struct Scratch {
    _dir: tempfile::TempDir,
    store: PathBuf,
    root: PathBuf,
    repository: Repository,
}

fn scratch() -> Scratch {
    let dir = private_dir();
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

fn seat(name: &str, role: Role, route: &str, desk: u32) -> Seat {
    Seat {
        name: name.into(),
        role,
        route: parse_route(route).unwrap(),
        look: "default".into(),
        desk,
    }
}

/// The real inbox, with task states a test sets as an owner that ran
/// them would. The store's journal replays each task file, so a test
/// cannot write a finished state into it.
struct Tasks {
    store: Store,
    forced: BTreeMap<String, (Status, Execution)>,
    /// What a task's ended turns spent, as its owner would record it.
    spent: BTreeMap<String, Spend>,
    /// Tasks the inbox no longer holds.
    gone: BTreeSet<String>,
}

impl Tasks {
    fn open(scratch: &Scratch) -> Self {
        Self {
            store: Store::open(&scratch.store).unwrap(),
            forced: BTreeMap::new(),
            spent: BTreeMap::new(),
            gone: BTreeSet::new(),
        }
    }

    fn force(&mut self, task: &str, status: Status, execution: Execution) {
        self.forced.insert(task.into(), (status, execution));
    }

    fn show(&self, task: &str) -> Option<Task> {
        Inbox::task(self, task)
    }

    fn count(&self) -> usize {
        self.store.list().unwrap().len()
    }
}

impl Inbox for Tasks {
    fn apply(&mut self, command: &[u8]) -> Result<(), super::super::Error> {
        Inbox::apply(&mut self.store, command)
    }

    fn task(&self, task: &str) -> Option<Task> {
        if self.gone.contains(task) {
            return None;
        }
        let mut found = self.store.show(task).ok()?;
        if let Some((status, execution)) = self.forced.get(task) {
            found.status = *status;
            found.execution = *execution;
        }
        Some(found)
    }

    fn spend(&self, task: &str) -> Option<Spend> {
        let found = Inbox::task(self, task)?;
        Some(
            self.spent
                .get(task)
                .copied()
                .unwrap_or_else(|| spend_of(&found)),
        )
    }
}

/// A studio with a Codex lead, a Claude Code worker, and a Grok worker.
fn team(scratch: &Scratch) -> (Tasks, Studio) {
    let tasks = Tasks::open(scratch);
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
    (tasks, studio)
}

fn goal(scratch: &Scratch) -> NewGoal {
    NewGoal {
        text: "Add a --verbose flag and document it.".into(),
        repository: scratch.repository.clone(),
        lead: None,
    }
}

fn plan(tasks: serde_json::Value) -> String {
    serde_json::json!({ "schema": PLAN_SCHEMA, "tasks": tasks }).to_string()
}

/// `a`, then `b` after `a`, then `c` after both.
fn diamond() -> String {
    plan(serde_json::json!([
        {"id": "a", "title": "Parse the flag", "description": "Add --verbose.", "seat": "ada"},
        {"id": "b", "title": "Log when verbose", "depends_on": ["a"], "seat": "grace"},
        {"id": "c", "title": "Document the flag", "depends_on": ["a", "b"]},
    ]))
}

fn reply(plan: &str) -> String {
    format!("I read the repository.\n\n```json\n{plan}\n```\n")
}

fn finish(tasks: &mut Tasks, task: &str) {
    tasks.force(task, Status::Finished, Execution::Finished);
}

fn ids(released: &[Released]) -> Vec<String> {
    released.iter().map(|item| item.task_id.clone()).collect()
}

fn no_reply(_: &str) -> Option<String> {
    None
}

#[test]
fn a_goal_queues_its_lead_with_a_briefing_and_notes_it_eligible() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    studio
        .remember(
            MemoryKind::Convention,
            Party::Person,
            None,
            "Run cargo fmt before committing.",
        )
        .unwrap();
    let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    assert_eq!(lead.task_id.len(), 64, "host access names tasks by hex");
    assert!(
        lead.task_id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_eq!(lead.seat, "lead");
    assert_eq!(lead.provider, Provider::Codex);
    let task = tasks.show(&lead.task_id).unwrap();
    assert_eq!(task.status, Status::Queued);
    assert_eq!(
        task.intent.configuration.adapter,
        super::super::adapter::NAME
    );
    assert_eq!(
        task.intent.configuration.model.as_deref(),
        Some("gpt-6-luna")
    );
    assert_eq!(task.intent.workspace.path, scratch.repository.path);
    assert!(task.intent.prompt.contains("Add a --verbose flag"));
    assert!(
        task.intent
            .prompt
            .contains("Run cargo fmt before committing.")
    );
    assert!(task.intent.prompt.contains(PLAN_SCHEMA));
    assert!(
        task.intent
            .prompt
            .contains("- grace (worker, grok:default)")
    );
    let journal = autostart::journal(&scratch.root);
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].event, "eligible");
    assert_eq!(journal[0].task.as_deref(), Some(lead.task_id.as_str()));
    assert_eq!(journal[0].device.as_deref(), Some(DEVICE));
    assert_eq!(journal[0].workspace.as_deref(), Some("demo"));
    assert_eq!(journal[0].requested.as_deref(), Some("codex"));
    let view = studio.view(&tasks);
    assert_eq!(view.goals[0].status, GoalStatus::Planning);
    assert_eq!(view.goals[0].lead_progress, Progress::Queued);
    assert_eq!(
        view.seats[0].task_id.as_deref(),
        Some(lead.task_id.as_str())
    );
}

#[test]
fn plan_entries_release_only_when_their_dependencies_are_done() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    let task = |id: &str| slot_task_id(&goal_id, id);

    // The lead is still planning: nothing else is created.
    let lead_reply = |_: &str| Some(reply(&diamond()));
    assert!(
        studio
            .reconcile(&mut tasks, 101, &lead_reply)
            .unwrap()
            .is_empty()
    );
    assert_eq!(tasks.count(), 1);

    finish(&mut tasks, &lead.task_id);
    let released = studio.reconcile(&mut tasks, 102, &lead_reply).unwrap();
    assert_eq!(ids(&released), [task("a")]);
    assert!(tasks.show(&task("b")).is_none(), "b is held, not queued");
    let view = studio.view(&tasks);
    let goal = &view.goals[0];
    assert_eq!(goal.status, GoalStatus::Running);
    assert_eq!((goal.final_tasks, goal.total_tasks), (0, 3));
    let progress: Vec<Progress> = goal.entries.iter().map(|entry| entry.progress).collect();
    assert_eq!(progress, [Progress::Queued, Progress::Held, Progress::Held]);
    // The plan is shared memory, and the unassigned task got a worker.
    assert_eq!(studio.state().memory[0].kind, MemoryKind::Plan);
    assert!(
        studio.state().memory[0]
            .text
            .contains("- b: Log when verbose [@grace] (after a)")
    );
    assert_eq!(goal.entries[2].seat, "ada");

    // A running dependency holds its dependents.
    tasks.force(&task("a"), Status::Running, Execution::Running);
    assert!(
        studio
            .reconcile(&mut tasks, 103, &no_reply)
            .unwrap()
            .is_empty()
    );

    finish(&mut tasks, &task("a"));
    let released = studio.reconcile(&mut tasks, 104, &no_reply).unwrap();
    assert_eq!(ids(&released), [task("b")]);
    assert_eq!(released[0].provider, Provider::Grok);
    // c waits on b as well as a.
    assert!(tasks.show(&task("c")).is_none());

    finish(&mut tasks, &task("b"));
    let released = studio.reconcile(&mut tasks, 105, &no_reply).unwrap();
    assert_eq!(ids(&released), [task("c")]);
    let briefing = tasks.show(&task("c")).unwrap().intent.prompt;
    assert!(briefing.contains("Task: Document the flag"));
    assert!(briefing.contains("- a: Parse the flag"));
    assert!(briefing.contains("Plan for goal"));

    finish(&mut tasks, &task("c"));
    assert!(
        studio
            .reconcile(&mut tasks, 106, &no_reply)
            .unwrap()
            .is_empty()
    );
    let view = studio.view(&tasks);
    assert_eq!(view.goals[0].status, GoalStatus::Done);
    assert_eq!(
        (view.goals[0].final_tasks, view.goals[0].total_tasks),
        (3, 3)
    );
    // Every release was noted for the auto-start policy once.
    let noted: Vec<String> = autostart::journal(&scratch.root)
        .into_iter()
        .filter_map(|entry| entry.task)
        .collect();
    assert_eq!(noted, [lead.task_id, task("a"), task("b"), task("c")]);
}

#[test]
fn an_invalid_plan_becomes_a_decision_and_creates_nothing() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    finish(&mut tasks, &lead.task_id);
    let cyclic = plan(serde_json::json!([
        {"id": "a", "title": "One", "depends_on": ["b"]},
        {"id": "b", "title": "Two", "depends_on": ["a"]},
        {"id": "c", "title": "Three"},
    ]));
    let lead_reply = |_: &str| Some(reply(&cyclic));
    assert!(
        studio
            .reconcile(&mut tasks, 101, &lead_reply)
            .unwrap()
            .is_empty()
    );
    assert_eq!(tasks.count(), 1, "only the lead exists");
    let view = studio.view(&tasks);
    assert_eq!(view.goals[0].status, GoalStatus::Decision);
    let decision = view.goals[0].decision.clone().unwrap();
    assert_eq!(decision.kind, DecisionKind::InvalidPlan);
    assert_eq!(
        decision.reasons,
        ["the dependencies form a cycle through `a`, `b`"]
    );
    assert!(view.goals[0].entries.is_empty(), "no partial plan");
    // The open decision is not reread on the next pass.
    assert!(
        studio
            .reconcile(&mut tasks, 102, &lead_reply)
            .unwrap()
            .is_empty()
    );

    // The person delivers a corrected plan, which answers the decision.
    match studio
        .accept_plan(&mut tasks, &goal_id, diamond().as_bytes(), 103)
        .unwrap()
    {
        PlanOutcome::Accepted { released } => {
            assert_eq!(ids(&released), [slot_task_id(&goal_id, "a")]);
        }
        PlanOutcome::Decision(decision) => panic!("{decision:?}"),
    }
    assert_eq!(studio.view(&tasks).goals[0].status, GoalStatus::Running);
    assert!(matches!(
        studio.accept_plan(&mut tasks, &goal_id, diamond().as_bytes(), 104),
        Err(Error::State(_))
    ));
}

#[test]
fn a_lead_without_a_plan_or_that_failed_opens_a_decision() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (_, first) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    let (_, second) = studio.submit_goal(&mut tasks, goal(&scratch), 101).unwrap();
    finish(&mut tasks, &first.task_id);
    tasks.force(&second.task_id, Status::Finished, Execution::Failed);
    let lead_reply = |_: &str| Some("I could not decide on a plan.".to_owned());
    studio.reconcile(&mut tasks, 102, &lead_reply).unwrap();
    let kinds: Vec<DecisionKind> = studio
        .state()
        .goals
        .iter()
        .map(|goal| goal.decision.as_ref().unwrap().kind)
        .collect();
    assert_eq!(kinds, [DecisionKind::NoPlan, DecisionKind::LeadFailed]);
}

#[test]
fn a_failed_dependency_blocks_its_dependents_and_opens_a_decision() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    finish(&mut tasks, &lead.task_id);
    let lead_reply = |_: &str| Some(reply(&diamond()));
    studio.reconcile(&mut tasks, 101, &lead_reply).unwrap();
    tasks.force(
        &slot_task_id(&goal_id, "a"),
        Status::Cancelled,
        Execution::Stopped,
    );
    assert!(
        studio
            .reconcile(&mut tasks, 102, &no_reply)
            .unwrap()
            .is_empty()
    );
    let view = studio.view(&tasks);
    let progress: Vec<Progress> = view.goals[0].entries.iter().map(|e| e.progress).collect();
    assert_eq!(
        progress,
        [Progress::Cancelled, Progress::Blocked, Progress::Blocked]
    );
    let decision = view.goals[0].decision.clone().unwrap();
    assert_eq!(decision.kind, DecisionKind::DependencyFailed);
    assert_eq!(decision.reasons.len(), 2);
}

#[test]
fn a_restart_finishes_an_interrupted_release_exactly_once() {
    for fault in [Fault::BeforeApply, Fault::AfterApply] {
        let scratch = scratch();
        let (mut tasks, mut studio) = team(&scratch);
        let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
        finish(&mut tasks, &lead.task_id);
        studio.fail_at(fault);
        let lead_reply = |_: &str| Some(reply(&diamond()));
        assert!(studio.reconcile(&mut tasks, 101, &lead_reply).is_err());
        drop(studio);
        let forced = tasks.forced.clone();
        drop(tasks);

        // A new process: the document holds the plan and the saved command.
        let mut tasks = Tasks::open(&scratch);
        tasks.forced = forced;
        let mut studio = Studio::open(&scratch.store)
            .unwrap()
            .with_host_root(&scratch.root);
        let goal = &studio.state().goals[0];
        assert!(goal.planned, "{fault:?}");
        assert_eq!(goal.plan[0].slot.state, SlotState::Releasing, "{fault:?}");
        assert!(goal.plan[0].slot.command.is_some());
        let a = slot_task_id(&goal_id, "a");
        assert_eq!(tasks.show(&a).is_some(), fault == Fault::AfterApply);

        let released = studio.reconcile(&mut tasks, 102, &no_reply).unwrap();
        assert_eq!(ids(&released), [a.clone()], "{fault:?}");
        let task = tasks.show(&a).unwrap();
        assert_eq!(task.revision, 1, "one submission, {fault:?}");
        assert_eq!(tasks.count(), 2);
        assert_eq!(
            studio.state().goals[0].plan[0].slot.state,
            SlotState::Submitted
        );
        assert!(
            studio
                .reconcile(&mut tasks, 103, &no_reply)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn seats_goals_memory_and_messages_survive_a_restart() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    studio
        .remember(
            MemoryKind::Decision,
            Party::Person,
            None,
            "Keep the CLI stable.",
        )
        .unwrap();
    studio
        .message(
            &tasks,
            Party::Person,
            Party::Seat {
                name: "grace".into(),
            },
            "Prefer small commits.",
            100,
        )
        .unwrap();
    let before = studio.state().clone();
    drop(studio);
    let studio = Studio::open(&scratch.store).unwrap();
    assert_eq!(studio.state(), &before);
    let path = scratch.store.join(DIR).join(STATE_FILE);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "the document is private");
    }
}

#[test]
fn a_second_coordinator_waits_for_the_lock() {
    let scratch = scratch();
    let _tasks = Tasks::open(&scratch);
    let held = Studio::open(&scratch.store).unwrap();
    let path = scratch.store.join(DIR).join(LOCK_FILE);
    let other = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    assert!(matches!(
        other.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(held);
    assert!(other.try_lock().is_ok());
}

#[test]
fn a_message_steers_a_running_task_or_waits_for_the_next_briefing() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    tasks.force(&lead.task_id, Status::Running, Execution::Running);

    // The lead's engine reads steering mid-turn.
    let sent = studio
        .message(
            &tasks,
            Party::Person,
            Party::Seat {
                name: "lead".into(),
            },
            "Keep the plan to three tasks.",
            101,
        )
        .unwrap();
    assert_eq!(
        sent[0].delivery,
        Delivery::Steered {
            task_id: lead.task_id.clone()
        }
    );
    assert_eq!(
        super::super::steer::take(studio.store(), &lead.task_id),
        ["Keep the plan to three tasks."]
    );

    // Everyone else has no task yet: their messages wait.
    let sent = studio
        .message(
            &tasks,
            Party::Seat {
                name: "lead".into(),
            },
            Party::Everyone,
            "Use the existing logger.",
            102,
        )
        .unwrap();
    let to: Vec<Party> = sent.iter().map(|message| message.to.clone()).collect();
    assert_eq!(
        to,
        [
            Party::Seat { name: "ada".into() },
            Party::Seat {
                name: "grace".into()
            }
        ]
    );
    assert!(
        sent.iter()
            .all(|message| message.delivery == Delivery::Waiting)
    );
    let sequences: Vec<u64> = sent.iter().map(|message| message.sequence).collect();
    assert_eq!(sequences[1], sequences[0] + 1);
    assert_eq!(studio.state().sequence, sequences[1]);

    // Grace's first task carries hers, and the message records it.
    finish(&mut tasks, &lead.task_id);
    let lead_reply = |_: &str| Some(reply(&diamond()));
    studio.reconcile(&mut tasks, 103, &lead_reply).unwrap();
    finish(&mut tasks, &slot_task_id(&goal_id, "a"));
    studio.reconcile(&mut tasks, 104, &no_reply).unwrap();
    let b = slot_task_id(&goal_id, "b");
    let briefing = tasks.show(&b).unwrap().intent.prompt;
    assert!(briefing.contains("Messages for you:\n- From @lead: Use the existing logger."));
    let grace = studio
        .state()
        .messages
        .iter()
        .find(|message| {
            message.to
                == Party::Seat {
                    name: "grace".into(),
                }
        })
        .unwrap();
    assert_eq!(grace.delivery, Delivery::Briefed { task_id: b.clone() });

    // A Grok Build turn reads instructions only when it starts: a message
    // to its running task waits for the next one instead of steering.
    tasks.force(&b, Status::Running, Execution::Running);
    let sent = studio
        .message(
            &tasks,
            Party::Person,
            Party::Seat {
                name: "grace".into(),
            },
            "Also log the flag's value.",
            105,
        )
        .unwrap();
    assert_eq!(sent[0].delivery, Delivery::Waiting);
    assert!(matches!(
        studio.message(
            &tasks,
            Party::Person,
            Party::Seat {
                name: "nobody".into()
            },
            "Hello.",
            106
        ),
        Err(Error::UnknownSeat(_))
    ));
}

#[test]
fn seats_are_validated_and_a_pending_seat_cannot_be_removed() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    assert!(matches!(
        studio.set_seat(seat("Bad Name", Role::Worker, "codex:gpt-6-luna", 5)),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        studio.set_seat(seat("dup-desk", Role::Worker, "codex:gpt-6-luna", 1)),
        Err(Error::Invalid(_))
    ));
    assert!(parse_route("vertex:gemini").is_err());
    assert!(parse_route("codex").is_err());
    assert_eq!(studio.free_desk(), 3);
    // Replacing a seat keeps one seat of that name.
    studio
        .set_seat(seat("ada", Role::Worker, "codex:gpt-6-luna", 1))
        .unwrap();
    assert_eq!(studio.state().seats.len(), 3);
    assert_eq!(
        studio.state().seat("ada").unwrap().route.provider,
        Provider::Codex
    );

    let (_, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    finish(&mut tasks, &lead.task_id);
    let lead_reply = |_: &str| Some(reply(&diamond()));
    studio.reconcile(&mut tasks, 101, &lead_reply).unwrap();
    // grace holds b, which has not started.
    assert!(matches!(studio.remove_seat("grace"), Err(Error::State(_))));
    assert!(matches!(
        studio.remove_seat("nobody"),
        Err(Error::UnknownSeat(_))
    ));
    // A goal needs a lead seat that is a lead.
    let mut not_lead = goal(&scratch);
    not_lead.lead = Some("ada".into());
    assert!(matches!(
        studio.submit_goal(&mut tasks, not_lead, 102),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn validation_names_every_problem() {
    let seats = vec![
        seat("lead", Role::Lead, "codex:gpt-6-luna", 0),
        seat("ada", Role::Worker, "claude:claude-opus-5-5", 1),
    ];
    let bad = plan(serde_json::json!([
        {"id": "a", "title": "One", "depends_on": ["a", "zz"]},
        {"id": "a", "title": "Again"},
        {"id": "Bad", "title": "Upper"},
        {"id": "d", "title": "", "seat": "nobody"},
    ]));
    let reasons = validate(bad.as_bytes(), &seats).unwrap_err();
    assert_eq!(
        reasons,
        [
            "task id `a` appears twice",
            "task id `Bad` is not 1 to 32 lowercase letters, digits, and hyphens",
            "task `a` depends on itself",
            "task `a` depends on `zz`, which the plan does not hold",
            "task `d` needs a one-line title of 1 to 256 bytes",
            "task `d` names seat `nobody`, which the studio does not have",
        ]
    );
    let many: Vec<serde_json::Value> = (0..=MAX_PLAN_TASKS)
        .map(|n| serde_json::json!({"id": format!("t{n}"), "title": "Step"}))
        .collect();
    let reasons = validate(plan(many.into()).as_bytes(), &seats).unwrap_err();
    assert_eq!(reasons, ["the plan has 33 tasks; the most is 32"]);
    assert_eq!(
        validate(b"{\"schema\":1}", &seats).unwrap_err().len(),
        1,
        "a malformed plan is one reason"
    );
    assert!(validate(plan(serde_json::json!([])).as_bytes(), &seats).is_err());
    let leads_only = vec![seats[0].clone()];
    let reasons = validate(
        plan(serde_json::json!([{"id": "a", "title": "One"}])).as_bytes(),
        &leads_only,
    )
    .unwrap_err();
    assert_eq!(
        reasons,
        ["a task names no seat and the studio has no worker seat"]
    );
    assert!(
        validate(diamond().as_bytes(), &seats).is_err(),
        "grace is unknown here"
    );
}

#[test]
fn a_reply_carries_its_last_plan_block() {
    let first = plan(serde_json::json!([{"id": "a", "title": "One"}]));
    let second = plan(serde_json::json!([{"id": "b", "title": "Two"}]));
    let text = format!(
        "Draft:\n```json\n{first}\n```\nA shell aside:\n```sh\nls\n```\nFinal:\n```json\n{second}\n```"
    );
    assert_eq!(plan_in_reply(&text).as_deref(), Some(second.as_str()));
    assert_eq!(plan_in_reply(&first).as_deref(), Some(first.as_str()));
    assert_eq!(plan_in_reply("No plan here."), None);
    assert_eq!(plan_in_reply("```json\n{\"a\":1}\n```"), None);
}

#[test]
fn sweep_leaves_a_store_without_a_studio_alone() {
    let scratch = scratch();
    sweep(&scratch.store, &scratch.root, 100);
    assert!(!scratch.store.exists());
}

fn ended(cost: super::super::owner::Cost) -> super::super::owner::ResultRecord {
    let mut result = super::super::owner::ResultRecord {
        ending: "completed".into(),
        exit_code: Some(0),
        stop_requested: false,
        group_clear: true,
        elapsed_ms: 1,
        trace_digest: String::new(),
        candidate_snapshot: None,
        artifact_file: None,
        artifact_digest: None,
        output_incomplete: false,
        cost_status: String::new(),
        cost_microusd: None,
        engine_microusd: None,
        jev_microusd: None,
        payer: None,
        payer_keys: Vec::new(),
    };
    result.priced(cost);
    result
}

/// Each ended turn adds its recorded cost; a turn whose whole cost is not
/// known adds its known part and counts as unpriced.
#[test]
fn a_task_spends_what_its_ended_turns_cost() {
    use super::super::owner::Cost;
    let first = ended(Cost {
        engine_microusd: Some(300_000),
        jev_microusd: Some(20_000),
    });
    let second = ended(Cost {
        engine_microusd: Some(150_000),
        jev_microusd: None,
    });
    let third = ended(Cost::default());
    assert_eq!(
        spend_of_results([&first]),
        Spend {
            microusd: 320_000,
            unpriced: 0
        }
    );
    assert_eq!(
        spend_of_results([&first, &second, &third]),
        Spend {
            microusd: 470_000,
            unpriced: 2
        }
    );
    assert!(spend_of_results(Vec::<&super::super::owner::ResultRecord>::new()).is_zero());
}

/// Spend accumulates over a task's turns, sums per task, seat, and goal,
/// survives a restart and a task the inbox no longer holds, and reaches
/// the device's view.
#[test]
fn spend_accumulates_across_turns_and_restarts_and_reaches_the_snapshot() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (goal_id, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    let PlanOutcome::Accepted { released } = studio
        .accept_plan(&mut tasks, &goal_id, diamond().as_bytes(), 100)
        .unwrap()
    else {
        panic!("the plan is valid");
    };
    let a = released[0].task_id.clone();
    // Nothing spent yet: the document carries no spend, as before.
    assert!(!studio.record_spend(&tasks).unwrap());
    let path = scratch.store.join(DIR).join(STATE_FILE);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("\"spend\""));
    assert!(studio.view(&tasks).spend.is_zero());

    // The lead's turn and `a`'s first turn end.
    let spent = |microusd| Spend {
        microusd,
        unpriced: 0,
    };
    tasks.spent.insert(lead.task_id.clone(), spent(400_000));
    tasks.spent.insert(a.clone(), spent(250_000));
    studio.reconcile(&mut tasks, 101, &no_reply).unwrap();
    let view = studio.view(&tasks);
    assert_eq!(view.spend, spent(650_000));
    assert_eq!(view.goals[0].spend, spent(650_000));
    assert_eq!(view.goals[0].lead_spend, spent(400_000));
    assert_eq!(view.goals[0].entries[0].spend, spent(250_000));
    let seat = |view: &View, name: &str| {
        view.seats
            .iter()
            .find(|seat| seat.seat.name == name)
            .unwrap()
            .spend
    };
    assert_eq!(seat(&view, "lead"), spent(400_000));
    assert_eq!(seat(&view, "ada"), spent(250_000));
    assert!(seat(&view, "grace").is_zero());

    // `a`'s second turn adds to its first; recording it twice changes
    // nothing.
    tasks.spent.insert(
        a.clone(),
        Spend {
            microusd: 600_000,
            unpriced: 1,
        },
    );
    studio.reconcile(&mut tasks, 102, &no_reply).unwrap();
    let sequence = studio.state().sequence;
    assert!(!studio.record_spend(&tasks).unwrap());
    assert_eq!(studio.state().sequence, sequence);
    assert_eq!(
        studio.view(&tasks).goals[0].spend,
        Spend {
            microusd: 1_000_000,
            unpriced: 1
        }
    );

    // A restart, with an inbox that no longer holds either task.
    drop(studio);
    drop(tasks);
    let mut tasks = Tasks::open(&scratch);
    tasks.gone.insert(lead.task_id.clone());
    tasks.gone.insert(a.clone());
    let mut studio = Studio::open(&scratch.store).unwrap();
    studio.reconcile(&mut tasks, 103, &no_reply).unwrap();
    let view = studio.view(&tasks);
    let total = Spend {
        microusd: 1_000_000,
        unpriced: 1,
    };
    assert_eq!(view.spend, total);
    assert_eq!(view.goals[0].spend, total);
    assert_eq!(
        seat(&view, "ada"),
        Spend {
            microusd: 600_000,
            unpriced: 1
        }
    );
    assert_eq!(studio.state().spend[&a].goal_id, goal_id);
    assert_eq!(studio.state().spend[&a].seat, "ada");

    // The snapshot a device draws carries it per goal, seat, and task.
    let wire = studio.wire(&tasks, &scratch.store);
    assert_eq!(wire.goals[0].spend.microusd, 1_000_000);
    assert_eq!(wire.goals[0].spend.label(), "$1.00+");
    let ada = wire.seats.iter().find(|seat| seat.seat == "ada").unwrap();
    assert_eq!(ada.spend.microusd, 600_000);
    let row = wire.tasks.iter().find(|task| task.task == a).unwrap();
    assert_eq!(row.spend.microusd, 600_000);
    let lead_row = wire
        .tasks
        .iter()
        .find(|task| task.task == lead.task_id)
        .unwrap();
    assert_eq!(lead_row.spend.microusd, 400_000);
}

/// The inbox with `asking`'s tasks waiting on an approval: finished, the
/// run ended asking.
struct Asking<'a> {
    tasks: &'a Tasks,
    asking: BTreeSet<String>,
}

impl Inbox for Asking<'_> {
    fn apply(&mut self, _command: &[u8]) -> Result<(), super::super::Error> {
        Err(super::super::Error::NotFound)
    }

    fn task(&self, task: &str) -> Option<Task> {
        let mut found = Inbox::task(self.tasks, task)?;
        if self.asking.contains(task) {
            found.status = Status::Finished;
            found.execution = Execution::Finished;
            found.run = Some(super::super::studio_sim::run(
                &found,
                interaction::APPROVAL_ENDING,
            ));
        }
        Some(found)
    }
}

fn step(command: &str, cwd: &str) -> interaction::Step {
    interaction::Step {
        schema: interaction::STEP_SCHEMA.into(),
        tool: "shell".into(),
        command: command.into(),
        cwd: cwd.into(),
        reason: "checks the change".into(),
    }
}

fn asks(step: &interaction::Step) -> String {
    format!(
        "May I run the tests?\n\n```json\n{}\n```\n",
        serde_json::to_string(step).unwrap()
    )
}

fn author() -> rules::Author {
    rules::Author {
        device: "a".repeat(64),
        grant: Some("b".repeat(64)),
        epoch: Some(1),
    }
}

/// A standing rule admits exactly the step it names, for its own seat:
/// another seat, command, or directory still asks, as does a paused seat,
/// a task that waits on nothing, and a revoked rule. The host answers
/// each wait once, and the rules survive a restart.
#[test]
fn a_standing_rule_admits_only_the_matching_step_for_its_seat() {
    let scratch = scratch();
    let (mut tasks, mut studio) = team(&scratch);
    let (_, lead) = studio.submit_goal(&mut tasks, goal(&scratch), 100).unwrap();
    let tests = step("cargo test -p coder", "/work/repo");
    let offered = Studio::offer("lead", &tests).unwrap();
    assert_eq!(
        offered,
        "lead may run shell `cargo test -p coder` in /work/repo without asking again"
    );
    let asked_for = |wanted: interaction::Step| move |_: &str| Some(asks(&wanted));
    // A rule for another seat admits nothing the lead asks.
    let ada = Studio::offer("ada", &tests).unwrap();
    studio
        .allow_always("ada", &tests, &ada, author(), 101)
        .unwrap();
    let waiting = Asking {
        tasks: &tasks,
        asking: BTreeSet::from([lead.task_id.clone()]),
    };
    assert!(
        studio
            .standing_answers(&waiting, &asked_for(tests.clone()))
            .is_empty()
    );
    // The text the person saw must be the step's own.
    assert!(matches!(
        studio.allow_always("lead", &tests, &ada, author(), 102),
        Err(Error::State(_))
    ));
    assert!(matches!(
        studio.allow_always("nobody", &tests, &offered, author(), 102),
        Err(Error::UnknownSeat(_))
    ));
    let push = step("git push origin main", "/work/repo");
    assert_eq!(
        Studio::offer("lead", &push),
        None,
        "high risk asks each time"
    );
    assert!(matches!(
        studio.allow_always("lead", &push, &offered, author(), 102),
        Err(Error::State(_))
    ));
    let rule = studio
        .allow_always("lead", &tests, &offered, author(), 102)
        .unwrap();
    assert_eq!(rule.text, offered);
    assert_eq!(
        studio
            .allow_always("lead", &tests, &offered, author(), 103)
            .unwrap(),
        rule,
        "the same rule again keeps the first"
    );
    let due = studio.standing_answers(&waiting, &asked_for(tests.clone()));
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].task, lead.task_id);
    assert_eq!(due[0].rule, rule);
    let command = due[0].command();
    assert_eq!(command.len(), 64);
    assert_eq!(command, due[0].clone().command(), "one answer per wait");
    assert!(due[0].answer().contains(&offered));
    // A wider or different step still asks.
    for other in [
        step("cargo test -p coder --release", "/work/repo"),
        step("cargo test -p coder", "/work/other"),
        step("cargo test", "/work/repo"),
    ] {
        assert!(
            studio
                .standing_answers(&waiting, &asked_for(other.clone()))
                .is_empty(),
            "{other:?}"
        );
    }
    // An approval that names no step, or a task that waits on nothing.
    assert!(
        studio
            .standing_answers(&waiting, &|_: &str| Some("May I go ahead?".to_owned()))
            .is_empty()
    );
    let idle = Asking {
        tasks: &tasks,
        asking: BTreeSet::new(),
    };
    assert!(
        studio
            .standing_answers(&idle, &asked_for(tests.clone()))
            .is_empty()
    );
    // A paused seat's approvals wait for the person.
    studio.pause_seat("lead").unwrap();
    assert!(
        studio
            .standing_answers(&waiting, &asked_for(tests.clone()))
            .is_empty()
    );
    studio.resume_seat(&mut tasks, "lead", 104).unwrap();
    let waiting = Asking {
        tasks: &tasks,
        asking: BTreeSet::from([lead.task_id.clone()]),
    };
    assert_eq!(
        studio
            .standing_answers(&waiting, &asked_for(tests.clone()))
            .len(),
        1
    );
    // The rules survive a restart, and a revoked rule admits nothing.
    drop(studio);
    let mut studio = Studio::open(&scratch.store).unwrap();
    assert_eq!(studio.rules().len(), 2);
    assert_eq!(
        studio
            .standing_answers(&waiting, &asked_for(tests.clone()))
            .len(),
        1
    );
    assert!(studio.revoke_rule(&rule.id).unwrap());
    assert!(!studio.revoke_rule(&rule.id).unwrap());
    assert!(
        studio
            .standing_answers(&waiting, &asked_for(tests.clone()))
            .is_empty()
    );
    studio
        .allow_always("lead", &tests, &offered, author(), 105)
        .unwrap();
    let due = studio.standing_answers(&waiting, &asked_for(tests.clone()));
    assert_eq!(due.len(), 1);
    // The host consumes the rule for this wait once, before it answers.
    assert!(studio.note_applied(&due[0], 106).unwrap());
    assert!(!studio.note_applied(&due[0], 107).unwrap());
    assert_eq!(studio.applied().len(), 1, "one record per wait");
    assert!(
        studio
            .standing_answers(&waiting, &asked_for(tests))
            .is_empty(),
        "a consumed wait is not answered again"
    );
}

#[test]
fn a_rule_text_fences_backticks_in_its_command() {
    let quoted = step("echo `date`", "/work/repo");
    assert_eq!(
        rules::text("ada", &quoted),
        "ada may run shell `` echo `date` `` in /work/repo without asking again"
    );
}

/// Records each launch's grant instead of starting a process.
struct Grants(std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>);

impl autostart::Launch for Grants {
    fn launch(
        &self,
        _: &autostart::Engine,
        grant: &Path,
        _: &Path,
    ) -> std::result::Result<autostart::Launched, String> {
        self.0.lock().unwrap().push(grant.to_path_buf());
        Ok(autostart::Launched {
            owner_process: std::process::id(),
            grant_digest: "sha256:fake".into(),
        })
    }
}

fn at_200() -> u64 {
    200
}

/// The owner's policy admitting one Claude route, which names no engine,
/// in the scratch workspace, with the host-wide `coder.claude` setting.
fn claude_policy(
    access: super::super::adapter::Access,
    claude: autostart::ClaudeRuns,
) -> autostart::Policy {
    autostart::Policy {
        schema: autostart::POLICY_SCHEMA.into(),
        enabled: true,
        workspaces: vec!["demo".into()],
        max_running: 2,
        engine: autostart::Engine {
            adapter: super::super::adapter::NAME.into(),
            controller: PathBuf::from("/opt/coder/microcoder"),
            model: "claude-opus-5-5".into(),
            effort: None,
            max_steps: None,
            wall_seconds: None,
            memory_bytes: 1 << 30,
            write_workspace: false,
            decision_endpoint: "https://api.typesafe.ai".into(),
            decision_model: "jev-latest".into(),
            routes: vec![parse_route("claude:claude-opus-5-5").unwrap()],
            usage_probe: None,
            access,
            claude,
            codex: autostart::CodexRuns::default(),
        },
        changed_at: 1,
    }
}

/// Two lead seats on one Claude model, one naming the Claude Code
/// session and one Microcoder's loop, each with a goal whose lead task
/// the coordinator released. Returns the two lead task IDs.
fn session_and_loop_seats(scratch: &Scratch) -> (String, String) {
    let mut tasks = Tasks::open(scratch);
    let mut studio = Studio::open(&scratch.store)
        .unwrap()
        .with_host_root(&scratch.root);
    studio
        .set_seat(seat("ada", Role::Lead, "claude/session:claude-opus-5-5", 0))
        .unwrap();
    studio
        .set_seat(seat("bob", Role::Lead, "claude/loop:claude-opus-5-5", 1))
        .unwrap();
    let mut released = Vec::new();
    for lead in ["ada", "bob"] {
        let mut new = goal(scratch);
        new.lead = Some(lead.into());
        let (_, task) = studio.submit_goal(&mut tasks, new, 100).unwrap();
        assert_eq!(task.provider, Provider::Claude);
        released.push(task.task_id);
    }
    // The snapshot's route names each seat's engine, so a nameplate tells
    // the Claude Code seat from Microcoder on Claude.
    let view = studio.wire(&tasks, &scratch.store);
    let routes: Vec<(String, String)> = view
        .seats
        .iter()
        .map(|seat| (seat.seat.clone(), seat.route.clone()))
        .collect();
    assert_eq!(
        routes,
        [
            (
                "ada".to_owned(),
                "claude/session:claude-opus-5-5".to_owned()
            ),
            ("bob".to_owned(), "claude/loop:claude-opus-5-5".to_owned()),
        ]
    );
    let bob = released.pop().unwrap();
    let ada = released.pop().unwrap();
    (ada, bob)
}

/// Sweep the owner's policy once over the scratch store, as the host
/// does, and return what it wrote and the grants it launched.
fn sweep_once(scratch: &Scratch) -> (Vec<autostart::Entry>, Vec<super::super::owner::Grant>) {
    let launched = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let workspaces = BTreeMap::from([("demo".to_owned(), PathBuf::from(&scratch.repository.path))]);
    let autostart = autostart::Autostart::new(
        scratch.root.clone(),
        scratch.store.clone(),
        workspaces,
        Box::new(Grants(launched.clone())),
        at_200,
    )
    .with_probe(|_| super::super::capacity::Connection::Connected)
    .with_identify(|_| None)
    .foreground();
    let written = autostart.sweep();
    let grants = launched
        .lock()
        .unwrap()
        .iter()
        .map(|path| super::super::owner::Grant::parse(&std::fs::read(path).unwrap()).unwrap())
        .collect();
    (written, grants)
}

fn endpoint_of(grants: &[super::super::owner::Grant], task: &str) -> String {
    grants
        .iter()
        .find(|grant| grant.task_id == task)
        .and_then(|grant| grant.adapter_configuration.as_ref())
        .map(|configuration| configuration.generation_endpoint.clone())
        .unwrap_or_else(|| panic!("no grant for {task}"))
}

/// Acceptance for #10568: a `claude/session` seat's task starts in the
/// Claude Code session and a `claude/loop` seat's on the provider, through
/// Microcoder's loop, whichever the owner's host-wide `coder.claude` is.
#[test]
fn a_seat_route_names_the_engine_its_grants_run() {
    for host in [autostart::ClaudeRuns::Session, autostart::ClaudeRuns::Loop] {
        let scratch = scratch();
        let (ada, bob) = session_and_loop_seats(&scratch);
        claude_policy(super::super::adapter::Access::Full, host)
            .save(&scratch.root)
            .unwrap();
        let (written, grants) = sweep_once(&scratch);
        assert_eq!(grants.len(), 2, "{host:?}: {written:?}");
        assert_eq!(
            endpoint_of(&grants, &ada),
            super::super::capacity::CLAUDE_SESSION_ENDPOINT,
            "{host:?}"
        );
        assert_eq!(
            endpoint_of(&grants, &bob),
            Provider::Claude.endpoint(),
            "{host:?}"
        );
        for grant in &grants {
            let configuration = grant.adapter_configuration.as_ref().unwrap();
            assert_eq!(configuration.provider, "claude");
            assert_eq!(configuration.model, "claude-opus-5-5");
        }
        // The journal says which engine each start ran on.
        let started: Vec<String> = written
            .iter()
            .filter(|entry| entry.event == "started")
            .filter_map(|entry| entry.detail.clone())
            .collect();
        assert!(
            started
                .iter()
                .any(|detail| detail.contains("claude/session:claude-opus-5-5")),
            "{started:?}"
        );
        assert!(
            started
                .iter()
                .any(|detail| detail.contains("claude/loop:claude-opus-5-5")),
            "{started:?}"
        );
    }
}

/// A seat that names a session under the boundary is refused with the
/// reason, at launch and, once the policy exists, when it is seated; the
/// loop seat beside it starts (#10568).
#[test]
fn a_session_seat_under_the_boundary_is_refused_with_the_reason() {
    let scratch = scratch();
    let (ada, bob) = session_and_loop_seats(&scratch);
    claude_policy(
        super::super::adapter::Access::Boundary,
        autostart::ClaudeRuns::Session,
    )
    .save(&scratch.root)
    .unwrap();
    let (written, grants) = sweep_once(&scratch);
    assert_eq!(grants.len(), 1, "{written:?}");
    assert_eq!(endpoint_of(&grants, &bob), Provider::Claude.endpoint());
    let refused = written
        .iter()
        .find(|entry| entry.event == "refused" && entry.task.as_deref() == Some(ada.as_str()))
        .and_then(|entry| entry.detail.clone())
        .unwrap_or_else(|| panic!("ada's start was not refused: {written:?}"));
    assert!(refused.starts_with("seat ada's route"), "{refused}");
    assert!(refused.contains("Claude Code session"), "{refused}");
    assert!(refused.contains("full access"), "{refused}");
    assert!(refused.contains("boundary"), "{refused}");
    assert!(
        refused.contains("claude/loop:claude-opus-5-5"),
        "the reason names the route that would start: {refused}"
    );
    // With the policy in place, seating a session says so at once.
    let mut studio = Studio::open(&scratch.store)
        .unwrap()
        .with_host_root(&scratch.root);
    let Err(Error::Invalid(why)) = studio.set_seat(seat(
        "cy",
        Role::Worker,
        "claude/session:claude-opus-5-5",
        2,
    )) else {
        panic!("a session seat under the boundary was seated");
    };
    assert!(why.contains("full access"), "{why}");
    studio
        .set_seat(seat("cy", Role::Worker, "claude/loop:claude-opus-5-5", 2))
        .unwrap();
}

/// A seat names an engine only on a Claude or Codex route.
#[test]
fn only_claude_and_codex_seats_name_an_engine() {
    assert!(parse_route("grok/session:default").is_err());
    let unknown = parse_route("claude/acp:claude-opus-5-5").unwrap_err();
    assert!(unknown.to_string().contains("session or loop"), "{unknown}");
    let scratch = scratch();
    let mut studio = Studio::open(&scratch.store).unwrap();
    let mut devin = seat("dee", Role::Worker, "devin:default", 0);
    devin.route.engine = Some(autostart::RouteEngine::Session);
    assert!(matches!(studio.set_seat(devin), Err(Error::Invalid(_))));
}
