use super::super::studio::{GoalStatus, MemoryKind};
use super::*;

/// A fixture under a temporary directory, never under the real home.
fn fixture() -> (tempfile::TempDir, Fixture) {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Fixture::create(&dir.path().join("sim")).unwrap();
    (dir, fixture)
}

/// `path` as the scratch `origin`'s branch holds it.
fn landed(fixture: &Fixture, path: &str) -> String {
    git(
        &fixture.origin,
        &["show", &format!("{BRANCH}:{path}")],
        START,
        None,
    )
    .unwrap()
}

fn happened(team: &Team, text: &str) -> bool {
    team.events().iter().any(|event| event.text.contains(text))
}

#[test]
fn the_scripted_team_takes_its_goal_to_done_on_a_scratch_repository() {
    let (_dir, fixture) = fixture();
    let mut team = Team::new(fixture.clone()).unwrap();
    team.run_script().unwrap();

    // The goal is done: every planned task is done and nothing waits on
    // the person.
    let view = team.view();
    assert_eq!(view.goals.len(), 1);
    let goal = &view.goals[0];
    assert_eq!(goal.text, GOAL);
    assert_eq!(goal.workspace, LABEL);
    assert_eq!(goal.status, GoalStatus::Done);
    assert_eq!(goal.lead_progress, Progress::Done);
    assert_eq!((goal.final_tasks, goal.total_tasks), (3, 3));
    assert!(goal.decision.is_none());
    let entries: Vec<(&str, &str, Progress)> = goal
        .entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry.seat.as_str(), entry.progress))
        .collect();
    assert_eq!(
        entries,
        vec![
            ("greet", "ada", Progress::Done),
            ("docs", "grace", Progress::Done),
            ("release", "ada", Progress::Done),
        ]
    );
    assert!(view.seats.iter().all(|seat| seat.task_id.is_none()));

    // Every change landed on the scratch origin, the conflict resolved.
    assert_eq!(landed(&fixture, "greeting.txt"), "Hello, studio");
    assert!(landed(&fixture, "README.md").contains("Status: greets people, documented"));
    assert!(!landed(&fixture, "README.md").contains("<<<<<<<"));
    assert!(landed(&fixture, "docs/greeting.md").contains("greeting.txt"));
    assert!(landed(&fixture, "CHANGELOG.md").contains("Hello, studio"));
    assert_eq!(
        team.landed().keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["docs", "greet", "release"]
    );
    assert_eq!(
        git(&fixture.origin, &["rev-parse", BRANCH], START, None).unwrap(),
        team.landed()["release"]
    );

    // Seats authored every change; only the landing's rebase carries the
    // person's identity, and nothing was force-pushed.
    let log = git(
        &fixture.origin,
        &["log", "--first-parent", "--format=%an|%cn|%s", BRANCH],
        START,
        None,
    )
    .unwrap();
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        vec![
            "ada|ada|Add the changelog",
            "grace|person|Name greeting.txt in the documentation",
            "grace|person|Document the greeting",
            "ada|ada|Greet with Hello, studio",
            "fixture|fixture|Seed the scratch repository",
        ]
    );

    // The question, the approval, and the request for changes each
    // started a follow-up turn with the person's text.
    let follow_up = |key: &str| -> Vec<String> {
        let task = team.inbox().task(&team.task_id(key).unwrap()).unwrap();
        task.follow_ups
            .into_iter()
            .map(|item| item.prompt)
            .collect()
    };
    assert_eq!(follow_up("lead"), vec![ANSWER.to_owned()]);
    assert_eq!(follow_up("greet"), Vec::<String>::new());
    assert_eq!(follow_up("docs"), vec![CHANGES.to_owned()]);
    assert_eq!(follow_up("release"), vec![ALLOW.to_owned()]);

    // The steering message reached the queued task through the steer
    // path, and its seat read it when the turn started.
    let state = team.studio().state();
    let docs = team.task_id("docs").unwrap();
    assert_eq!(state.messages.len(), 1);
    assert_eq!(
        state.messages[0].delivery,
        Delivery::Steered {
            task_id: docs.clone()
        }
    );
    assert!(happened(
        &team,
        &format!("@grace read a steering message: {STEER}")
    ));

    // The stale merge was refused, the conflict was the seat's to fix, and
    // the coordinator survived the restart.
    assert!(happened(
        &team,
        &format!("a merge of {docs} at an earlier review was refused")
    ));
    assert!(happened(
        &team,
        &format!("{docs}'s seat resolved a merge conflict")
    ));
    assert!(happened(&team, "the host restarted"));

    // Shared memory holds the plan and each merge decision.
    let kinds: Vec<MemoryKind> = state.memory.iter().map(|entry| entry.kind).collect();
    assert_eq!(
        kinds,
        vec![
            MemoryKind::Plan,
            MemoryKind::Decision,
            MemoryKind::Decision,
            MemoryKind::Decision,
        ]
    );

    // The fixed clock moved one tick per step.
    assert_eq!(
        team.events().last().unwrap().at,
        START + TICK * SCRIPT.len() as u64 - TICK
    );
}

#[test]
fn the_sim_route_is_refused_outside_its_fixture() {
    let (dir, fixture) = fixture();
    admit(&fixture.checkout, Some(&fixture)).unwrap();
    assert_eq!(
        admit(&fixture.checkout, None).unwrap_err().code(),
        "sim_refused"
    );

    // Another repository, even beside the fixture, is not its scratch
    // repository.
    let other = dir.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    git(&other, &["init", "-q", "-b", BRANCH], START, None).unwrap();
    assert_eq!(
        admit(&other, Some(&fixture)).unwrap_err().code(),
        "sim_refused"
    );

    // Neither a seat nor the owner's auto-start policy takes the route.
    assert!(studio::parse_route(ROUTE).is_err());
    assert!(super::super::autostart::parse_route(ROUTE).is_err());

    // A configuration whose marker is gone is not an explicit fixture.
    std::fs::remove_file(fixture.dir.join(MARKER)).unwrap();
    assert_eq!(
        admit(&fixture.checkout, Some(&fixture)).unwrap_err().code(),
        "sim_refused"
    );
    assert_eq!(
        Team::new(fixture.clone()).err().unwrap().code(),
        "sim_refused"
    );
}

#[test]
fn a_fixture_starts_only_in_an_empty_directory() {
    let (_dir, fixture) = fixture();
    assert_eq!(
        Fixture::create(&fixture.dir).unwrap_err().code(),
        "sim_refused"
    );
}

#[test]
fn the_sim_inbox_accepts_exact_retries_and_refuses_reused_identities() {
    let (_dir, fixture) = fixture();
    let mut team = Team::new(fixture).unwrap();
    team.step(Step::Submit).unwrap();
    let lead = team.task_id("lead").unwrap();
    let command = |prompt: &str| {
        serde_json::to_vec(&Command {
            schema: COMMAND_SCHEMA.into(),
            command_id: "sim-cancel".into(),
            task_id: lead.clone(),
            expected_revision: Some(1),
            action: Action::Cancel {
                reason: prompt.into(),
            },
        })
        .unwrap()
    };
    let mut inbox = SimInbox::default();
    std::mem::swap(&mut inbox, &mut team.inbox);
    inbox.apply(&command("not needed")).unwrap();
    inbox.apply(&command("not needed")).unwrap();
    assert!(matches!(
        inbox.apply(&command("another reason")),
        Err(super::super::Error::Conflict)
    ));
    assert_eq!(inbox.task(&lead).unwrap().status, Status::Cancelled);
    // A question cannot be answered while the task is not waiting.
    std::mem::swap(&mut inbox, &mut team.inbox);
    assert_eq!(
        team.step(Step::Answer("lead", ANSWER)).unwrap_err().code(),
        "invalid_state"
    );
}

// The scripted engine on a scratch host (#10572).

/// A scratch host with the script's team seated, under a temporary
/// directory.
fn scratch() -> (tempfile::TempDir, Scratch) {
    let dir = tempfile::tempdir().unwrap();
    let scratch = Scratch::create(&dir.path().join("scratch")).unwrap();
    scratch.seat_team().unwrap();
    (dir, scratch)
}

/// The coordinator's state now.
fn state_of(store: &Path) -> studio::State {
    Studio::open(store).unwrap().state().clone()
}

/// Plan entry `key`'s task identity and flow, once the plan holds it.
fn entry_of(store: &Path, key: &str) -> Option<(String, studio::Flow)> {
    let state = state_of(store);
    let entry = state
        .goals
        .first()?
        .plan
        .iter()
        .find(|entry| entry.id == key)?;
    Some((entry.slot.task_id.clone(), entry.flow.clone()?))
}

fn show(store: &Path, task: &str) -> Task {
    Store::open(store).unwrap().show(task).unwrap()
}

/// Whether `key`'s change waits on the person's merge decision after its
/// `turn`th turn.
fn at_merge(store: &Path, key: &str, turn: u64) -> bool {
    entry_of(store, key)
        .is_some_and(|(_, flow)| flow.stage == studio::Stage::Merge && flow.turn == turn)
}

/// Step `engine` until `test` holds, or fail naming `what`.
fn run_until(engine: &Engine, what: &str, mut test: impl FnMut() -> bool) {
    for _ in 0..16 {
        if test() {
            return;
        }
        engine.step().unwrap();
    }
    assert!(test(), "the scripted engine never reached {what}");
}

/// Start `task`'s next turn with `text`, as an answer or a request for
/// changes does through the host.
fn continue_with(store: &Path, task: &str, text: &str) {
    let revision = show(store, task).revision;
    let command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: format!("test-{task}-{revision}"),
        task_id: task.into(),
        expected_revision: Some(revision),
        action: Action::Continue {
            prompt: text.into(),
        },
    };
    Store::open(store)
        .unwrap()
        .apply(&serde_json::to_vec(&command).unwrap())
        .unwrap();
}

/// `task`'s review at its worktree's revisions now.
fn review_now(store: &Path, task: &str) -> super::super::publish::Reviewed {
    let record = super::super::local::record(store, task).unwrap();
    let review =
        super::super::review::read(task, Path::new(&record.worktree), &record.base, REVIEW_MAX)
            .unwrap();
    super::super::publish::Reviewed {
        base: review.base,
        head_commit: review.head_commit,
        head: review.head,
    }
}

/// The person's merge of `task` at `reviewed`, as the host makes it.
fn merge_at(
    store: &Path,
    task: &str,
    reviewed: &super::super::publish::Reviewed,
) -> coder_host::access::review::Publication {
    studio::git::merge(store, task, reviewed).unwrap()
}

#[test]
fn the_engine_runs_only_on_a_scratch_host() {
    let (dir, scratch) = scratch();
    Engine::open(&scratch.root, &scratch.store).unwrap();
    assert_eq!(
        Scratch::create(&scratch.dir).unwrap_err().code(),
        "sim_refused"
    );
    assert_eq!(Scratch::open(&scratch.dir).unwrap().store, scratch.store);

    // Another task store, or a root no scratch host marked.
    let other = dir.path().join("other-tasks");
    Store::open(&other).unwrap();
    assert_eq!(
        Engine::open(&scratch.root, &other).unwrap_err().code(),
        "sim_refused"
    );
    let bare = dir.path().join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    assert_eq!(
        Engine::open(&bare, &scratch.store).unwrap_err().code(),
        "sim_refused"
    );

    // A root with an auto-start policy, even one that cannot be read.
    let policy = scratch.root.join(autostart::POLICY_FILE);
    std::fs::write(&policy, b"{}").unwrap();
    assert_eq!(
        Engine::open(&scratch.root, &scratch.store)
            .unwrap_err()
            .code(),
        "sim_refused"
    );
    std::fs::remove_file(&policy).unwrap();
    let engine = Engine::open(&scratch.root, &scratch.store).unwrap();
    std::fs::write(&policy, b"{}").unwrap();
    assert_eq!(engine.step().unwrap_err().code(), "sim_refused");
    std::fs::remove_file(&policy).unwrap();

    // A store that no longer admits scripted turns.
    std::fs::remove_file(scratch.store.join(owner::SCRIPTED_MARKER)).unwrap();
    assert_eq!(
        Engine::open(&scratch.root, &scratch.store)
            .unwrap_err()
            .code(),
        "sim_refused"
    );
}

#[test]
fn the_engine_takes_the_goal_through_a_scratch_hosts_coordinator_to_done() {
    let (_dir, scratch) = scratch();
    let store = scratch.store.clone();
    let checkout = scratch.fixture.checkout.clone();
    let engine = Engine::open(&scratch.root, &store).unwrap();
    let published = coder_host::access::review::PublishState::Published;
    let refused = coder_host::access::review::PublishState::Refused;

    // The goal, submitted as the host's studio intent submits it.
    let lead = {
        let mut tasks = Store::open(&store).unwrap();
        let mut studio = Studio::open(&store)
            .unwrap()
            .with_host_root(&scratch.root)
            .with_worktrees(studio::git::worktrees_dir(&scratch.root));
        let goal = NewGoal {
            text: GOAL.into(),
            repository: Repository {
                label: WORKSPACE.into(),
                path: checkout.to_string_lossy().into_owned(),
            },
            lead: None,
        };
        studio
            .submit_goal(&mut tasks, goal, autostart::unix_now())
            .unwrap()
            .1
            .task_id
    };

    // The lead asks; the person answers; the lead plans, and both
    // workers' changes reach the merge decision through the lead's review.
    run_until(&engine, "the lead's question", || {
        interaction::pending(&show(&store, &lead)) == Some(interaction::Kind::Question)
    });
    let run = show(&store, &lead).run.unwrap();
    assert_eq!(run.admission.adapter, owner::SCRIPTED_ADAPTER);
    assert_eq!(run.result.unwrap().cost_microusd, Some(0));
    continue_with(&store, &lead, ANSWER);
    run_until(&engine, "both changes at the merge decision", || {
        at_merge(&store, "greet", 1) && at_merge(&store, "docs", 1)
    });
    let (greet, _) = entry_of(&store, "greet").unwrap();
    let (docs, _) = entry_of(&store, "docs").unwrap();

    // The greeting lands in the person's checkout.
    let landed = merge_at(&store, &greet, &review_now(&store, &greet));
    assert_eq!(landed.state, published, "{}", landed.note);
    assert_eq!(
        std::fs::read_to_string(checkout.join("greeting.txt")).unwrap(),
        "Hello, studio\n"
    );

    // The person asks for a change; a merge at the earlier review is
    // refused once the change moved.
    let first = review_now(&store, &docs);
    continue_with(&store, &docs, CHANGES);
    run_until(&engine, "the changed documentation", || {
        at_merge(&store, "docs", 2)
    });
    let stale = merge_at(&store, &docs, &first);
    assert_eq!(stale.state, refused);
    assert!(
        stale.note.contains("changed since the review"),
        "{}",
        stale.note
    );

    // Both workers edited the status line: the merge conflicts and goes
    // back to grace, who merges main in and resolves it.
    let conflicted = merge_at(&store, &docs, &review_now(&store, &docs));
    assert_eq!(conflicted.state, refused);
    run_until(&engine, "the resolved conflict", || {
        at_merge(&store, "docs", 3)
            && entry_of(&store, "docs").is_some_and(|(_, flow)| flow.conflicts == 1)
    });
    let landed = merge_at(&store, &docs, &review_now(&store, &docs));
    assert_eq!(landed.state, published, "{}", landed.note);
    let readme = std::fs::read_to_string(checkout.join("README.md")).unwrap();
    assert!(
        readme.contains("Status: greets people, documented"),
        "{readme}"
    );
    assert!(!readme.contains("<<<<<<<"));
    assert!(checkout.join("docs/greeting.md").is_file());

    // The release waits on both, then asks to approve its step.
    run_until(&engine, "the release's approval", || {
        entry_of(&store, "release").is_some_and(|(task, _)| {
            interaction::pending(&show(&store, &task)) == Some(interaction::Kind::Approval)
        })
    });
    let (release, _) = entry_of(&store, "release").unwrap();
    continue_with(&store, &release, ALLOW);
    run_until(&engine, "the changelog at the merge decision", || {
        at_merge(&store, "release", 2)
    });
    let landed = merge_at(&store, &release, &review_now(&store, &release));
    assert_eq!(landed.state, published, "{}", landed.note);
    assert!(
        std::fs::read_to_string(checkout.join("CHANGELOG.md"))
            .unwrap()
            .contains("Hello, studio")
    );

    // The goal is done, and seats authored every change.
    run_until(&engine, "the goal done", || {
        let tasks = Store::open(&store).unwrap();
        Studio::open(&store).unwrap().view(&tasks).goals[0].status == GoalStatus::Done
    });
    let authors = git(&checkout, &["log", "--format=%an", BRANCH], START, None).unwrap();
    for seat in ["Studio Ada", "Studio Grace"] {
        assert!(authors.lines().any(|name| name == seat), "{authors}");
    }
}
