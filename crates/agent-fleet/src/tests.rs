use super::*;
use std::time::{Duration, Instant};

fn spec(engine: &str, task: &str) -> Spec {
    Spec {
        engine: engine.into(),
        task: task.into(),
        ..Spec::default()
    }
}

fn wait_for(registry: &Registry<String>, count: usize) -> Vec<Notice> {
    let started = Instant::now();
    let mut notices = Vec::new();
    while notices.len() < count {
        notices.extend(registry.drain_notices());
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "only {} notices",
            notices.len()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    notices
}

#[test]
fn three_agents_run_at_once_and_each_sends_one_notice() {
    let registry: Registry<String> = Registry::default();
    let gate = Arc::new(std::sync::Barrier::new(3));
    let mut handles = Vec::new();
    for task in ["fix the login page", "write the docs", "add a test"] {
        let control = registry.start(spec("codex", task)).unwrap();
        let gate = Arc::clone(&gate);
        handles.push(
            spawn(control, move |control| {
                // All three are inside their runs at the same moment.
                gate.wait();
                control.event(format!("working on {}", control.id()));
                control.add_usage(1200, Some(0.25));
                Outcome::Done(format!("{} is done", control.id()))
            })
            .unwrap(),
        );
    }
    assert_eq!(registry.running(), 3);
    for handle in handles {
        handle.join().unwrap();
    }
    let notices = wait_for(&registry, 3);
    assert_eq!(notices.len(), 3);
    assert!(registry.drain_notices().is_empty(), "one notice per run");
    for notice in &notices {
        assert_eq!(notice.status, Status::Done);
        assert_eq!(notice.tokens, 1200);
        assert_eq!(notice.cost_usd, Some(0.25));
        let text = notice.text();
        assert!(text.contains("finished after"), "{text}");
        assert!(text.contains("1.2k tokens · $0.25"), "{text}");
        assert!(text.contains(&format!("{} is done", notice.id)), "{text}");
    }
    let names: Vec<_> = registry.list().into_iter().map(|row| row.name).collect();
    assert_eq!(names, ["fix-the-login", "write-the-docs", "add-a-test"]);
    assert_eq!(registry.drain_events().len(), 3);
    assert!(registry.drain_events().is_empty());
}

#[test]
fn stop_ends_a_running_agent_as_stopped_with_one_notice() {
    let registry: Registry<String> = Registry::default();
    let control = registry
        .start(spec("claude", "loop until stopped"))
        .unwrap();
    let handle = spawn(control, |control| {
        while !control.stopped() {
            std::thread::sleep(Duration::from_millis(2));
        }
        Outcome::Failed("The Claude Code task was canceled.".into())
    })
    .unwrap();
    assert!(registry.stop("loop-until-stopped").is_ok());
    handle.join().unwrap();
    let notices = wait_for(&registry, 1);
    assert_eq!(notices[0].status, Status::Stopped);
    assert_eq!(notices[0].error, None, "a stop is not an error");
    assert!(notices[0].text().contains("was stopped"));
    assert!(registry.stop("agent-1").is_err(), "already stopped");
    assert!(registry.stop("nobody").is_err());
}

#[test]
fn messages_queue_while_running_and_resume_after_the_end() {
    let registry: Registry<String> = Registry::default();
    let control = registry.start(spec("codex", "first task")).unwrap();
    match registry
        .message("agent-1", "also update the readme")
        .unwrap()
    {
        Delivery::Queued(row) => assert_eq!(row.pending_messages, 1),
        other => panic!("{other:?}"),
    }
    assert_eq!(control.take_messages(), ["also update the readme"]);
    assert_eq!(registry.get("agent-1").unwrap().pending_messages, 0);
    control.finish(Outcome::Done("done once".into()));
    match registry.message("first-task", "one more thing").unwrap() {
        Delivery::Resume(row) => assert_eq!(row.status, Status::Done),
        other => panic!("{other:?}"),
    }
    let again = registry.resume("first-task").unwrap();
    let row = registry.get("agent-1").unwrap();
    assert_eq!((row.status, row.runs), (Status::Running, 2));
    assert!(registry.resume("agent-1").is_err(), "still running");
    again.finish(Outcome::Done("done twice".into()));
    let notices = registry.drain_notices();
    assert_eq!(notices.len(), 2);
    assert_eq!(notices[1].report.as_deref(), Some("done twice"));
    assert!(registry.message("agent-1", "  ").is_err());
}

#[test]
fn a_message_too_late_for_the_run_is_named_in_its_notice() {
    let registry: Registry<String> = Registry::default();
    let control = registry.start(spec("codex", "task")).unwrap();
    registry.message("agent-1", "late words").unwrap();
    let notice = control.finish(Outcome::Done("ok".into())).unwrap();
    assert!(notice.error.unwrap().contains("late words"));
}

#[test]
fn a_dropped_control_fails_the_agent_once() {
    let registry: Registry<String> = Registry::default();
    let control = registry.start(spec("codex", "task")).unwrap();
    let handle = std::thread::spawn(move || {
        let _held = control;
        panic!("runner bug");
    });
    assert!(handle.join().is_err());
    let notices = registry.drain_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].status, Status::Failed);
    assert_eq!(registry.running(), 0);
}

#[test]
fn names_are_unique_and_checked() {
    let registry: Registry<String> = Registry::default();
    let a = registry.start(spec("codex", "Fix login!")).unwrap();
    let b = registry.start(spec("codex", "fix login")).unwrap();
    assert_eq!(registry.get(a.id()).unwrap().name, "fix-login");
    assert_eq!(registry.get(b.id()).unwrap().name, "fix-login-2");
    let mut named = spec("codex", "x");
    named.name = Some("fix-login".into());
    assert!(registry.start(named.clone()).is_err());
    named.name = Some("bad name".into());
    assert!(registry.start(named).is_err());
    assert!(registry.start(spec("codex", " ")).is_err());
    assert!(registry.start(spec("", "task")).is_err());
}

#[test]
fn rows_serialize_for_the_website_and_apps() {
    let registry: Registry<String> = Registry::default();
    let control = registry.start(spec("codex", "task")).unwrap();
    control.add_usage(10, None);
    control.add_usage(5, Some(0.5));
    let json = registry.snapshot_json();
    assert_eq!(json[0]["schema"], SCHEMA);
    assert_eq!(json[0]["status"], "running");
    assert_eq!(json[0]["place"], "this computer");
    assert_eq!(json[0]["tokens"], 15);
    assert_eq!(json[0]["cost_usd"], 0.5);
    let row: AgentRow = serde_json::from_value(json[0].clone()).unwrap();
    assert_eq!(row.id, "agent-1");
    drop(control);
}

#[test]
fn elapsed_counts_every_run() {
    let mut row = AgentRow {
        id: "a".into(),
        name: "a".into(),
        engine: "codex".into(),
        place: "this computer".into(),
        status: Status::Done,
        task: "t".into(),
        started_ms: 0,
        ended_ms: Some(70_000),
        earlier_seconds: 30,
        tokens: 0,
        cost_usd: None,
        worktree: None,
        branch: None,
        parent_session: None,
        transcript: None,
        report: None,
        error: None,
        pending_messages: 0,
        runs: 2,
        run_started_ms: 10_000,
    };
    assert_eq!(row.elapsed_seconds(999_999), 90);
    row.ended_ms = None;
    assert_eq!(row.elapsed_seconds(20_000), 40);
    assert_eq!(elapsed_words(252), "4m 12s");
    assert_eq!(elapsed_words(3900), "1h 5m");
    assert_eq!(dollars(0.003), "$0.0030");
}

#[test]
fn the_disk_floor_refuses_new_worktree_agents() {
    let low = |_: &Path| Ok(5_000_000_000);
    let error = guard::check_disk(Path::new("."), 20, &low).unwrap_err();
    assert!(error.contains("Only 5 GB is free"), "{error}");
    let plenty = |_: &Path| Ok(300_000_000_000);
    assert!(guard::check_disk(Path::new("."), 20, &plenty).is_ok());
    assert_eq!(guard::floor_gb(&|_| Some("7".into())), 7);
    assert_eq!(guard::floor_gb(&|_| None), guard::DEFAULT_FLOOR_GB);
    assert_eq!(guard::pool_size(&|_| Some("0".into())), 1);
    assert_eq!(
        guard::target_slot(Path::new("/p"), 5, 4),
        PathBuf::from("/p/slot-1")
    );
}

#[test]
fn a_running_agents_worktree_lease_blocks_removal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("leases");
    assert!(lease::removable(&root, "agent-1").is_ok());
    let held = lease::hold(&root, "agent-1").unwrap();
    assert!(lease::held(&root, "agent-1"));
    assert!(lease::removable(&root, "agent-1").is_err());
    assert!(lease::hold(&root, "agent-1").is_err());
    assert!(lease::removable(&root, "agent-2").is_ok());
    drop(held);
    assert!(lease::removable(&root, "agent-1").is_ok());
}

#[test]
fn transcripts_are_grouped_by_parent_chat() {
    let root = Path::new("/t");
    assert_eq!(
        transcript_path(root, Some("chat-1"), "agent-2"),
        PathBuf::from("/t/chat-1/agent-2.json")
    );
    assert_eq!(
        transcript_path(root, Some("../x"), "agent-2"),
        PathBuf::from("/t/___x/agent-2.json")
    );
    assert_eq!(
        transcript_path(root, None, "agent-2"),
        PathBuf::from("/t/unsaved/agent-2.json")
    );
}
