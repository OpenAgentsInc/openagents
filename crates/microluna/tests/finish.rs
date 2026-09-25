//! The finish rule's fixtures (issue #9638), each a whole session on the
//! fake transport: a `done` finish waits for the score and a baseline
//! command after the last edit.

use microluna::fake::{FakeTransport, call};
use microluna::finish::UNVERIFIED;
use microluna::{
    Brief, Config, Ending, FinishRule, FinishStatus, Recorder, Report, TokenUsage, Workspace, run,
};
use serde_json::{Value, json};

fn usage() -> TokenUsage {
    TokenUsage {
        input: 10,
        cached: 0,
        output: 5,
        reasoning: 0,
    }
}

fn finish(id: &str, status: &str) -> microluna::Reply {
    call(
        id,
        "finish",
        &json!({"status": status, "summary": "Stopped.", "answer": "", "cause": "other"}),
        usage(),
    )
}

fn command(id: &str, command: &str) -> microluna::Reply {
    call(
        id,
        "run_command",
        &json!({"command": command, "timeout_seconds": null}),
        usage(),
    )
}

fn write(id: &str, path: &str) -> microluna::Reply {
    call(
        id,
        "write_file",
        &json!({"path": path, "contents": "print('fixed')\n"}),
        usage(),
    )
}

fn patch(id: &str, path: &str) -> microluna::Reply {
    call(
        id,
        "apply_patch",
        &json!({"patch": format!(
            "*** Begin Patch\n*** Update File: {path}\n-print('old')\n+print('new')\n*** End Patch"
        )}),
        usage(),
    )
}

fn rule(baseline: &[&str]) -> FinishRule {
    FinishRule {
        baseline: baseline.iter().map(|s| (*s).to_string()).collect(),
        ..FinishRule::score(&["score.sh"])
    }
}

/// Runs `replies` in a workspace holding `main.py` and a `score.sh` that
/// scores it full, under `rule` and `persist`.
async fn session(
    replies: Vec<microluna::Reply>,
    rule: Option<FinishRule>,
    persist: Option<microluna::Persist>,
) -> (Report, Recorder, Vec<microluna::Request>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.py"), "print('old')\n").unwrap();
    std::fs::write(dir.path().join("score.sh"), "echo SCORE 1 1\n").unwrap();
    let workspace = Workspace::new(dir.path())
        .unwrap()
        .isolated_by(microluna::Isolation::TaskContainer);
    let transport = FakeTransport::new(replies);
    let config = Config {
        max_turns: 12,
        finish_rule: rule,
        persist,
        ..Config::luna("t")
    };
    let mut recorder = Recorder::new();
    let report = run(
        &transport,
        &workspace,
        &Brief::task("Fix main.py."),
        &config,
        &mut recorder,
    )
    .await;
    (report, recorder, transport.requests())
}

/// What the session was told back for `call_id`.
fn told(requests: &[microluna::Request], call_id: &str) -> String {
    requests
        .iter()
        .flat_map(|r| r.input.iter())
        .filter(|item| item["type"] == "function_call_output" && item["call_id"] == call_id)
        .filter_map(|item| item["output"].as_str().map(str::to_string))
        .next_back()
        .unwrap_or_default()
}

#[tokio::test]
async fn a_finish_after_the_score_is_allowed() {
    let (report, _, _) = session(
        vec![
            patch("c1", "main.py"),
            command("c2", "sh score.sh"),
            finish("c3", "done"),
        ],
        Some(rule(&[])),
        None,
    )
    .await;
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!((report.refusals, report.unverified), (0, false));
    assert_eq!(report.turns, 3);
}

#[tokio::test]
async fn an_edit_after_the_score_is_refused_and_named() {
    let (report, recorder, requests) = session(
        vec![
            command("c1", "sh score.sh"),
            write("c2", "main.py"),
            finish("c3", "done"),
            command("c4", "sh score.sh"),
            finish("c5", "done"),
        ],
        Some(rule(&[])),
        None,
    )
    .await;
    let refusal = told(&requests, "c3");
    assert!(refusal.contains("you edited `main.py` after your last run of the score"));
    assert!(refusal.contains("`score.sh`"), "{refusal}");
    assert!(recorder.steps().iter().any(|s| s.message == refusal));
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!((report.refusals, report.unverified), (1, false));
    assert_eq!(report.turns, 5);
}

#[tokio::test]
async fn a_shell_command_that_changes_a_file_is_an_edit() {
    let (report, recorder, requests) = session(
        vec![
            command("c1", "sh score.sh"),
            command("c2", "printf 'x\\n' >> main.py"),
            finish("c3", "done"),
            command("c4", "sh score.sh"),
            finish("c5", "done"),
        ],
        Some(rule(&[])),
        None,
    )
    .await;
    assert!(told(&requests, "c3").contains("you edited `main.py`"));
    assert_eq!(report.refusals, 1);
    let changed: Vec<&Value> = recorder
        .steps()
        .iter()
        .filter_map(|s| s.call.as_ref()?.extra.get("changed"))
        .collect();
    assert_eq!(changed, [&json!(["main.py"])]);
}

#[tokio::test]
async fn with_no_baseline_command_only_the_score_is_required() {
    let replies = || {
        vec![
            write("c1", "main.py"),
            command("c2", "sh score.sh"),
            finish("c3", "done"),
            command("c4", "python3 main.py"),
            finish("c5", "done"),
        ]
    };
    let (report, _, _) = session(replies(), Some(rule(&[])), None).await;
    assert_eq!((report.refusals, report.turns), (0, 3));

    // With a baseline command, the same score run isn't enough.
    let (report, _, requests) = session(replies(), Some(rule(&["python3 main.py"])), None).await;
    let refusal = told(&requests, "c3");
    assert!(
        refusal.contains("after your last run of a baseline command"),
        "{refusal}"
    );
    assert!(refusal.contains("`python3 main.py`"), "{refusal}");
    assert_eq!((report.refusals, report.turns), (1, 5));
}

#[tokio::test]
async fn after_three_refusals_the_finish_is_accepted_as_unverified() {
    let (report, recorder, _) = session(
        vec![
            write("c1", "main.py"),
            finish("c2", "done"),
            finish("c3", "done"),
            finish("c4", "done"),
            finish("c5", "done"),
        ],
        Some(rule(&[])),
        None,
    )
    .await;
    assert_eq!(report.ending, Ending::Finished);
    assert_eq!((report.refusals, report.unverified), (3, true));
    assert_eq!(report.turns, 5);
    assert_eq!(report.finish.unwrap().status, FinishStatus::Done);
    assert!(recorder.steps().iter().any(|s| {
        s.message
            .contains(&format!("accepted this finish as {UNVERIFIED}"))
    }));
}

#[tokio::test]
async fn blocked_and_failed_finishes_are_not_gated() {
    for status in ["blocked", "failed"] {
        let (report, _, _) = session(
            vec![write("c1", "main.py"), finish("c2", status)],
            Some(rule(&[])),
            None,
        )
        .await;
        assert_eq!(report.ending, Ending::Finished);
        assert_eq!((report.refusals, report.turns), (0, 2));
    }
}

#[tokio::test]
async fn a_refusal_does_not_count_against_the_turn_backs() {
    // One turn-back allowed: the rule's refusal doesn't spend it, so the
    // score's turn-back still happens after it.
    let (report, recorder, _) = session(
        vec![
            write("c1", "main.py"),
            finish("c2", "done"),
            command("c3", "sh score.sh"),
            finish("c4", "done"),
            finish("c5", "done"),
        ],
        Some(rule(&[])),
        Some(microluna::Persist {
            max_returns: 1,
            not_done: false,
            score_command: Some("echo SCORE 0 1".to_string()),
            reserve_turns: 0,
            reserve_sec: 0,
        }),
    )
    .await;
    assert_eq!(report.refusals, 1);
    assert_eq!(report.turns, 5);
    assert!(
        recorder
            .steps()
            .iter()
            .any(|s| s.message.contains("scores the workspace 0 of 1"))
    );
}

#[tokio::test]
async fn without_the_rule_every_finish_stands() {
    let (report, _, _) = session(
        vec![write("c1", "main.py"), finish("c2", "done")],
        None,
        None,
    )
    .await;
    assert_eq!((report.refusals, report.turns), (0, 2));
}
