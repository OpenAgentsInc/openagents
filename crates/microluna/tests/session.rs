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
        status: 401,
        body: "signed out".to_string(),
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
    assert!(matches!(report.ending, Ending::Transport(ref why) if why.contains("401")));
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

#[tokio::test]
async fn a_broken_stream_is_sent_again_and_the_session_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let transport = FakeTransport::new(Vec::new());
    transport.then_fail(TransportError::Stream(
        "error decoding response body".to_string(),
    ));
    transport.then(call(
        "f",
        "finish",
        &json!({"status": "done", "summary": "Done.", "answer": ""}),
        usage(10, 0, 5),
    ));
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Finish."),
        &Config::luna("t"),
        &mut recorder,
    )
    .await;
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!(report.turns, 1);
    assert_eq!(transport.requests().len(), 2);
    assert!(
        recorder
            .steps()
            .iter()
            .any(|step| step.message.contains("trying again, 1 of 2"))
    );
}

#[tokio::test]
async fn a_read_only_workspace_refuses_edits_but_runs_commands() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap().reading_only();
    let wrote = workspace
        .call(
            "write_file",
            &json!({"path": "a.txt", "contents": "x"}).to_string(),
        )
        .await;
    assert_eq!(wrote.status, atif::Outcome::Cancelled);
    assert!(!dir.path().join("a.txt").exists());
    let patched = workspace
        .call(
            "apply_patch",
            &json!({"patch": "*** Begin Patch\n*** Add File: b.txt\n+x\n*** End Patch"})
                .to_string(),
        )
        .await;
    assert_eq!(patched.status, atif::Outcome::Cancelled);
    assert!(!dir.path().join("b.txt").exists());
}

/// Reads the model asks for in one turn run together and come back in
/// the order it asked; the effort is low until the first edit.
#[tokio::test]
async fn reads_in_one_turn_run_together_and_the_effort_rises_after_the_first_edit() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "alpha\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "beta\n").unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let read = |id: &str, path: &str| {
        json!({
            "type": "function_call", "id": format!("fc-{id}"), "call_id": id,
            "name": "read_file",
            "arguments": json!({"path": path, "start_line": null, "max_lines": null}).to_string(),
        })
    };
    let both = microluna::Reply {
        id: Some("r1".to_string()),
        model: "gpt-6-luna".to_string(),
        items: vec![read("c1", "a.txt"), read("c2", "b.txt")],
        usage: usage(1_000, 0, 40),
    };
    let transport = FakeTransport::new(vec![
        both,
        call(
            "c3",
            "write_file",
            &json!({"path": "c.txt", "contents": "gamma\n"}),
            usage(1_100, 900, 40),
        ),
        call(
            "c4",
            "finish",
            &json!({"status": "done", "summary": "Wrote c.", "answer": "", "cause": "none"}),
            usage(1_200, 1_000, 20),
        ),
    ]);
    let config = Config {
        effort: Some("medium".to_string()),
        orient_effort: Some("low".to_string()),
        parallel_tools: true,
        ..Config::luna("t")
    };
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Read a and b, then write c."),
        &config,
        &mut recorder,
    )
    .await;
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!(report.calls, 4);
    let calls: Vec<String> = recorder
        .steps()
        .iter()
        .filter_map(|s| s.call.as_ref())
        .map(|c| format!("{} {}", c.name, c.output.lines().next().unwrap_or_default()))
        .collect();
    assert!(
        calls[0].starts_with("read_file") && calls[0].contains("alpha"),
        "{calls:?}"
    );
    assert!(
        calls[1].starts_with("read_file") && calls[1].contains("beta"),
        "{calls:?}"
    );
    let requests = transport.requests();
    assert!(requests.iter().all(|r| r.parallel_tools));
    let efforts: Vec<Option<&str>> = requests.iter().map(|r| r.effort.as_deref()).collect();
    assert_eq!(efforts, [Some("low"), Some("low"), Some("medium")]);
    // The two outputs follow the two calls, in order.
    let second = &requests[1].input;
    let outputs: Vec<&str> = second
        .iter()
        .filter(|i| i["type"] == "function_call_output")
        .filter_map(|i| i["call_id"].as_str())
        .collect();
    assert_eq!(outputs, ["c1", "c2"]);
}

#[test]
fn only_reading_calls_count_as_reads() {
    use microluna::tools::reads_only;
    let command = |c: &str| json!({ "command": c }).to_string();
    assert!(reads_only("read_file", "{}"));
    assert!(reads_only(
        "run_command",
        &command("cat a.py | grep def && ls -la")
    ));
    assert!(reads_only("run_command", &command("sed -n 1,40p x.py")));
    assert!(!reads_only("run_command", &command("sed -i s/a/b/ x.py")));
    assert!(!reads_only("run_command", &command("cat a > b")));
    assert!(!reads_only("run_command", &command("python3 -m pytest")));
    assert!(!reads_only(
        "run_command",
        &command("awk 'BEGIN { system(\"rm x\") }'")
    ));
    assert!(!reads_only("apply_patch", "{}"));
}

#[tokio::test]
async fn the_host_turns_back_a_finish_below_the_score_then_lets_one_stand() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("score.sh"),
        "if [ -f done.txt ]; then echo SCORE 2 2; else echo SCORE 1 2; fi\n",
    )
    .unwrap();
    let workspace = Workspace::new(dir.path())
        .unwrap()
        .isolated_by(microluna::Isolation::TaskContainer);
    let finish = |id: &str, status: &str| {
        call(
            id,
            "finish",
            &json!({"status": status, "summary": "Stopped.", "answer": ""}),
            usage(10, 0, 5),
        )
    };
    let transport = FakeTransport::new(vec![
        finish("c1", "done"),
        call(
            "c2",
            "write_file",
            &json!({"path": "done.txt", "contents": "yes\n"}),
            usage(10, 0, 5),
        ),
        finish("c3", "done"),
    ]);
    let config = Config {
        max_turns: 10,
        persist: Some(microluna::Persist {
            max_returns: 3,
            not_done: true,
            score_command: Some("sh score.sh".to_string()),
            reserve_turns: 2,
            reserve_sec: 0,
        }),
        ..Config::luna("t")
    };
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Score."),
        &config,
        &mut recorder,
    )
    .await;
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!(report.turns, 3);
    assert!(
        recorder
            .steps()
            .iter()
            .any(|s| s.message.contains("scores the workspace 1 of 2")),
        "the turned-back finish is recorded"
    );
}

#[tokio::test]
async fn a_blocked_finish_goes_back_until_the_returns_run_out() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path())
        .unwrap()
        .isolated_by(microluna::Isolation::TaskContainer);
    let blocked = |id: &str| {
        call(
            id,
            "finish",
            &json!({"status": "blocked", "summary": "Hard.", "answer": "", "cause": "other"}),
            usage(10, 0, 5),
        )
    };
    let transport = FakeTransport::new(vec![blocked("c1"), blocked("c2"), blocked("c3")]);
    let config = Config {
        max_turns: 10,
        persist: Some(microluna::Persist {
            max_returns: 2,
            not_done: true,
            score_command: None,
            reserve_turns: 2,
            reserve_sec: 0,
        }),
        ..Config::luna("t")
    };
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Try."),
        &config,
        &mut Recorder::new(),
    )
    .await;
    assert_eq!(report.turns, 3);
    assert_eq!(report.finish.unwrap().status, FinishStatus::Blocked);
}
