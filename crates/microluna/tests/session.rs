//! A whole session on the fake transport: read, patch, finish.

use microluna::fake::{FakeTransport, call, say};
use microluna::session::{INSTRUCTIONS, USAGE_EXTENSION};
use microluna::{
    Brief, Config, Ending, Evidence, FinishStatus, Recorder, TokenUsage, TransportError, Workspace,
    run,
};
use serde_json::json;

fn usage(input: u64, cached: u64, output: u64) -> TokenUsage {
    TokenUsage {
        input,
        cached,
        output,
        reasoning: 0,
    }
}

fn brief() -> Brief {
    Brief {
        task: "Change the greeting to 'hello, world'.".to_string(),
        guidance: "Edit only greet.txt.".to_string(),
        evidence: vec![Evidence {
            label: "greet.txt".to_string(),
            text: "hello\n".to_string(),
        }],
        state: vec!["No earlier session changed anything.".to_string()],
    }
}

#[tokio::test]
async fn a_session_reads_patches_and_finishes_with_a_typed_result() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("greet.txt"), "hello\n").unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let patch = "*** Begin Patch\n*** Update File: greet.txt\n-hello\n+hello, world\n*** End Patch";
    let transport = FakeTransport::new(vec![
        call(
            "c1",
            "read_file",
            &json!({"path": "greet.txt", "start_line": null, "max_lines": null}),
            usage(1_000, 0, 50),
        ),
        call(
            "c2",
            "apply_patch",
            &json!({"patch": patch}),
            usage(1_200, 1_000, 80),
        ),
        call(
            "c3",
            "finish",
            &json!({"status": "done", "summary": "Changed it.", "answer": ""}),
            usage(1_400, 1_200, 30),
        ),
    ]);
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &brief(),
        &Config::luna("task-1"),
        &mut recorder,
    )
    .await;

    assert_eq!(report.ending, Ending::Finished);
    assert_eq!(report.finish.as_ref().unwrap().status, FinishStatus::Done);
    assert_eq!((report.turns, report.calls), (3, 3));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("greet.txt")).unwrap(),
        "hello, world\n"
    );
    assert_eq!(report.usage, usage(3_600, 2_200, 160));
    // 1,400 uncached at $0.10, 2,200 cached at $0.01, 160 out at $0.50.
    let expected = (1_400.0 * 0.10 + 2_200.0 * 0.01 + 160.0 * 0.50) / 1_000_000.0;
    assert!((report.cost_usd.unwrap() - expected).abs() < 1e-12);

    let requests = transport.requests();
    assert_eq!(requests.len(), 3);
    // Every request starts with the same prefix, so the provider can
    // cache it, and each one only grows.
    for request in &requests {
        assert_eq!(request.instructions, INSTRUCTIONS);
        assert_eq!(request.cache_key, "task-1");
        assert_eq!(request.input[..2], requests[0].input[..2]);
        assert_eq!(request.tools.len(), 5);
    }
    let first = requests[0].input[0]["content"][0]["text"].as_str().unwrap();
    assert!(first.starts_with("# Task"));
    assert!(first.contains("# Evidence"));
    let state = requests[0].input[1]["content"][0]["text"].as_str().unwrap();
    assert!(state.starts_with("# Current state"));
    let last = &requests[2].input;
    assert_eq!(last[last.len() - 1]["type"], "function_call_output");
    assert_eq!(last[last.len() - 1]["call_id"], "c2");
    assert!(
        last[last.len() - 1]["output"]
            .as_str()
            .unwrap()
            .contains("M greet.txt")
    );

    let steps = recorder.steps();
    let calls: Vec<&str> = steps
        .iter()
        .filter_map(|step| step.call.as_ref().map(|call| call.name.as_str()))
        .collect();
    assert_eq!(calls, ["read_file", "apply_patch", "finish"]);
    let spent: Vec<_> = steps
        .iter()
        .filter_map(|step| step.extensions.get(USAGE_EXTENSION))
        .collect();
    assert_eq!(spent.len(), 3);
    assert_eq!(spent[1]["cached"], 1_000);
}

#[tokio::test]
async fn text_without_a_tool_call_is_nudged_once_then_stops() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let transport = FakeTransport::new(vec![
        say("I think it's fine.", usage(10, 0, 5)),
        say("Still fine.", usage(10, 0, 5)),
    ]);
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Look around."),
        &Config::luna("t"),
        &mut recorder,
    )
    .await;
    assert_eq!(report.ending, Ending::Stopped);
    assert_eq!(report.turns, 2);
    let second = &transport.requests()[1].input;
    assert_eq!(
        second[second.len() - 1]["content"][0]["text"],
        microluna::session::NUDGE
    );
}

#[tokio::test]
async fn a_transport_failure_ends_the_session_with_its_usage_kept() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let transport = FakeTransport::new(vec![call(
        "c1",
        "read_file",
        &json!({"path": "missing.txt", "start_line": null, "max_lines": null}),
        usage(100, 0, 10),
    )]);
    transport.then_fail(TransportError::Http {
        status: 429,
        body: "slow down".to_string(),
    });
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Read it."),
        &Config::luna("t"),
        &mut recorder,
    )
    .await;
    assert!(matches!(report.ending, Ending::Transport(ref why) if why.contains("429")));
    assert_eq!(report.usage.input, 100);
    let read = recorder
        .steps()
        .iter()
        .find_map(|step| step.call.as_ref())
        .unwrap();
    assert_eq!(read.outcome, atif::Outcome::Failed);
}

#[tokio::test]
async fn the_turn_limit_ends_a_session_that_never_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let again = || {
        call(
            "c",
            "read_file",
            &json!({"path": "x", "start_line": null, "max_lines": null}),
            usage(1, 0, 1),
        )
    };
    let transport = FakeTransport::new(vec![again(), again(), again()]);
    let config = Config {
        max_turns: 2,
        ..Config::luna("t")
    };
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Loop."),
        &config,
        &mut Recorder::new(),
    )
    .await;
    assert_eq!(report.ending, Ending::TurnLimit);
    assert_eq!(report.turns, 2);
}

#[tokio::test]
async fn a_passed_time_bound_ends_the_session_before_any_request() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let transport = FakeTransport::new(vec![say("never sent", usage(1, 0, 1))]);
    let config = Config {
        deadline: Some(std::time::Duration::ZERO),
        ..Config::luna("t")
    };
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Anything."),
        &config,
        &mut Recorder::new(),
    )
    .await;
    assert_eq!(report.ending, Ending::Deadline);
    assert_eq!(report.turns, 0);
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn a_task_container_runs_commands_directly_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path())
        .unwrap()
        .isolated_by(microluna::Isolation::TaskContainer);
    let outcome = workspace
        .call(
            "run_command",
            &json!({"command": "echo hi > out.txt", "timeout_seconds": null}).to_string(),
        )
        .await;
    assert_eq!(
        outcome.status,
        atif::Outcome::Completed,
        "{}",
        outcome.output
    );
    assert_eq!(outcome.extra["boundary"], json!("task-container"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out.txt")).unwrap(),
        "hi\n"
    );
}

#[tokio::test]
async fn a_forwarding_recorder_hands_every_step_to_the_host() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let transport = FakeTransport::new(vec![call(
        "f",
        "finish",
        &json!({"status": "done", "summary": "Nothing to do.", "answer": ""}),
        usage(10, 0, 5),
    )]);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counter = seen.clone();
    let mut recorder = Recorder::new().forwarding(move |_| counter.set(counter.get() + 1));
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Finish."),
        &Config::luna("t"),
        &mut recorder,
    )
    .await;
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!(seen.get(), recorder.steps().len());
    assert!(seen.get() >= 3);
}
