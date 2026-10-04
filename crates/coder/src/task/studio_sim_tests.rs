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
