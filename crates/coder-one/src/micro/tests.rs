use std::collections::BTreeMap;

use microluna::fake::{call, say};

use super::*;
use crate::component::jev::JevMode;

fn usage(input: u64, cached: u64, output: u64) -> TokenUsage {
    TokenUsage {
        input,
        cached,
        output,
        reasoning: 0,
    }
}

fn finish(id: &str, status: &str, summary: &str) -> microluna::Reply {
    call(
        id,
        "finish",
        &json!({ "status": status, "summary": summary, "answer": "" }),
        usage(1_000, 800, 40),
    )
}

fn micro(dir: &Path, replies: Vec<microluna::Reply>, policy: Policy) -> Micro {
    let work = dir.join("work");
    let artifacts = dir.join("artifacts");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    let mut micro = Micro::new(
        "gpt-6-luna",
        None,
        Duration::from_secs(120),
        &work,
        &artifacts,
        Recorder::default(),
        0,
        policy,
        Isolation::TaskContainer,
    );
    micro.wire = Ok(Wire::Fake(FakeTransport::new(replies)));
    micro
}

const TASK: &str =
    "Write hello.txt containing the word hello.\n\nThen write world.txt containing the word world.";

fn prepared() -> Prepared {
    let requirements = crate::requirements::mechanical(TASK);
    Prepared {
        instruction: TASK.to_string(),
        title: "hello world".to_string(),
        directions: String::new(),
        requirements,
        items: vec![crate::pack::Item {
            id: "e1".to_string(),
            source: crate::pack::Source::File,
            label: "notes.md".to_string(),
            p: Some(0.9),
            text: "hello.txt and world.txt go in the workspace root.".to_string(),
        }],
        informs: BTreeMap::new(),
        jev: JevMode::Off,
        deadline: None,
    }
}

fn briefing(text: &str) -> Briefing {
    Briefing {
        text: text.to_string(),
        cap: 12_000,
        included: Vec::new(),
        omitted: Vec::new(),
    }
}

fn kinds(recorder: &Recorder) -> Vec<String> {
    recorder
        .steps()
        .iter()
        .filter_map(|step| step.extensions.get(crate::session::EVENT_KEY))
        .filter_map(|event| event["event"]["kind"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn a_single_session_records_executor_events_and_an_exact_price() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "c1",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "hello\n" }),
                usage(1_200, 0, 60),
            ),
            call(
                "c2",
                "run_command",
                &json!({ "command": "cat hello.txt", "timeout_seconds": null }),
                usage(1_300, 1_024, 30),
            ),
            finish("c3", "done", "Wrote hello.txt."),
        ],
        Policy {
            mode: Mode::Single,
            ..Policy::default()
        },
    );
    let report = executor.execute(&briefing(TASK)).await;
    assert_eq!(report.status, Status::Answered);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("work/hello.txt")).unwrap(),
        "hello\n"
    );
    // Luna at $0.10 uncached, $0.01 cached, and $0.50 output per million.
    let want = (1_676.0 * 0.10 + 1_824.0 * 0.01 + 130.0 * 0.50) / 1_000_000.0;
    let cost = report.summary.total_cost_usd.unwrap();
    assert!((cost - want).abs() < 1e-12, "{cost} against {want}");
    assert_eq!(crate::delegate::charge(&report).0, "priced");
    assert_eq!(report.summary.api_calls, Some(3));
    let kinds = kinds(&executor.recorder);
    for kind in [
        "artifact_changed",
        "command_started",
        "command_completed",
        "session_ended",
    ] {
        assert!(kinds.iter().any(|k| k == kind), "{kind} missing: {kinds:?}");
    }
    // Model tokens never reach the episode as generation steps, which the
    // episode's ledger would count as unpriced door calls.
    assert!(
        executor
            .recorder
            .steps()
            .iter()
            .all(|step| step.tokens.is_none())
    );
    let invocations = crate::record::invocations(&executor.recorder.steps());
    assert!(invocations.iter().any(|i| i.component == SESSION_COMPONENT));
    assert!(
        dir.path()
            .join("artifacts/microluna-1-1.atif.jsonl")
            .is_file()
    );
    assert!(dir.path().join("artifacts/microluna-1.json").is_file());
    assert_eq!(
        report.stream.as_ref().unwrap()["path"],
        json!("artifacts/microluna-1.json")
    );
}

#[tokio::test]
async fn the_loop_rebuilds_each_session_and_bounds_retries() {
    let dir = tempfile::tempdir().unwrap();
    let map = prepared().requirements;
    let groups = groups(&map, 4);
    assert_eq!(groups.len(), 2, "{groups:?}");
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "a1",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "hello\n" }),
                usage(900, 0, 40),
            ),
            finish("a2", "done", "hello.txt holds hello."),
            finish("b1", "blocked", "world.txt needs a word I can't find."),
            finish("c1", "failed", "Still can't."),
        ],
        Policy {
            checks: false,
            ..Policy::default()
        },
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    assert_eq!(report.status, Status::Answered);
    let record = executor.last.clone().unwrap();
    let moves: Vec<&str> = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["move"].as_str().unwrap())
        .collect();
    // Jev is off: the session's own status decides, and the second group
    // gets its two attempts and no more.
    assert_eq!(moves, ["next", "retry", "stuck"]);
    assert_eq!(record["sessions"].as_array().unwrap().len(), 3);
    let handoffs = executor
        .recorder
        .steps()
        .iter()
        .filter(|step| step.extensions.contains_key(crate::handoff::KEY))
        .count();
    assert_eq!(handoffs, 3);
    let Wire::Fake(fake) = executor.wire.as_ref().unwrap() else {
        panic!("a fake wire");
    };
    let requests = fake.requests();
    // Every session opens with the same instructions and task text, so the
    // provider can cache the prefix across sessions.
    let first_text = |i: usize| {
        requests[i].input[0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(first_text(0).starts_with(&format!("# Task\n\n{TASK}")));
    assert!(first_text(2).starts_with(&format!("# Task\n\n{TASK}")));
    assert!(first_text(0).contains(&groups[0].ids[0]));
    assert!(first_text(2).contains(&groups[1].ids[0]));
    // The third session is a retry: its state names the blocked session.
    let state = requests[3].input[1]["content"][0]["text"].as_str().unwrap();
    assert!(state.contains("ended blocked"), "{state}");
    assert!(
        requests
            .iter()
            .all(|r| r.cache_key == requests[0].cache_key)
    );
}

#[test]
fn a_contradicting_check_keeps_the_loop_on_its_group() {
    let ran = Ran {
        number: 1,
        focus: vec!["R1".to_string()],
        ending: Ending::Finished,
        finish: Some(microluna::Finish {
            status: microluna::FinishStatus::Done,
            summary: "done".to_string(),
            answer: String::new(),
        }),
        turns: 1,
        calls: 1,
        usage: TokenUsage::default(),
        cost_usd: Some(0.0),
        milliseconds: 0,
        session_id: "s".to_string(),
        trace: String::new(),
        commands: vec![("cargo test".to_string(), Some(0))],
        changed: vec!["src/lib.rs".to_string()],
        edited: true,
        ran_after_edit: true,
        read_only: false,
    };
    let at = |contradicted, verdict_fail, last, attempts| Signals {
        contradicted,
        verdict_fail,
        last,
        read_only: false,
        evidence: true,
        require_evidence: true,
        attempts,
        max_attempts: 2,
    };
    assert_eq!(
        settle(Some(Move::Next), &ran, at(false, false, false, 1)).0,
        Move::Next
    );
    assert_eq!(
        settle(Some(Move::Done), &ran, at(true, false, false, 1)).0,
        Move::Retry
    );
    assert_eq!(
        settle(Some(Move::Next), &ran, at(true, false, false, 2)).0,
        Move::Stuck
    );
    assert_eq!(settle(None, &ran, at(false, false, false, 1)).0, Move::Next);
    assert_eq!(
        settle(Some(Move::Retry), &ran, at(false, false, false, 2)).0,
        Move::Stuck
    );
    // A combined verdict of fail keeps the loop from ending, and only that.
    assert_eq!(
        settle(Some(Move::Next), &ran, at(false, true, false, 1)).0,
        Move::Next
    );
    assert_eq!(
        settle(Some(Move::Next), &ran, at(false, true, true, 1)).0,
        Move::Retry
    );
    assert_eq!(
        settle(Some(Move::Done), &ran, at(false, true, false, 1)).0,
        Move::Retry
    );
    assert_eq!(
        settle(Some(Move::Done), &ran, at(false, true, false, 2)).0,
        Move::Stuck
    );
    // Without evidence, an ending move is a retry; a between-group next is
    // not gated.
    let no_ev = |ending| Signals {
        contradicted: false,
        verdict_fail: false,
        last: ending,
        read_only: false,
        evidence: false,
        require_evidence: true,
        attempts: 1,
        max_attempts: 2,
    };
    assert_eq!(settle(Some(Move::Done), &ran, no_ev(false)).0, Move::Retry);
    assert_eq!(settle(Some(Move::Next), &ran, no_ev(true)).0, Move::Retry);
    assert_eq!(settle(Some(Move::Next), &ran, no_ev(false)).0, Move::Next);
    // A read-only session always retries into an edit session.
    let read = Signals {
        read_only: true,
        ..no_ev(false)
    };
    assert_eq!(settle(Some(Move::Done), &ran, read).0, Move::Retry);
}

#[tokio::test]
async fn read_first_runs_a_read_only_reconnaissance_then_an_edit_session() {
    let dir = tempfile::tempdir().unwrap();
    // One group: read-only session 1 finishes without editing, then the
    // edit session writes and tests, then finishes done.
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "r1",
                "run_command",
                &json!({ "command": "cat hello.txt || true", "timeout_seconds": null }),
                usage(600, 0, 20),
            ),
            finish(
                "r2",
                "done",
                "hello.txt is missing; the edit session should create it.",
            ),
            call(
                "e1",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "hello\n" }),
                usage(700, 0, 30),
            ),
            call(
                "e2",
                "run_command",
                &json!({ "command": "cat hello.txt", "timeout_seconds": null }),
                usage(800, 0, 20),
            ),
            finish("e3", "done", "Wrote and checked hello.txt."),
        ],
        Policy {
            checks: false,
            read_first: true,
            max_groups: 1,
            ..Policy::default()
        },
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    assert_eq!(report.status, Status::Answered);
    let record = executor.last.clone().unwrap();
    let sessions = record["sessions"].as_array().unwrap();
    // The first session ran read-only and made no edit; the second edited.
    assert_eq!(sessions[0]["read_only"], json!(true));
    assert_eq!(sessions[0]["changed"], json!([]));
    assert_eq!(sessions[1]["read_only"], json!(false));
    assert_eq!(sessions[1]["changed"], json!(["hello.txt"]));
    let moves: Vec<&str> = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["move"].as_str().unwrap())
        .collect();
    // The read session retries into the edit session, which then advances
    // past the last group with its edit and test as evidence.
    assert_eq!(moves, ["retry", "next"]);
    // The edit session had its write refused? No: read-only was only the
    // first session, so hello.txt exists.
    assert_eq!(
        std::fs::read_to_string(dir.path().join("work/hello.txt")).unwrap(),
        "hello\n"
    );
}

#[tokio::test]
async fn a_missing_login_is_a_transport_refusal_with_no_charge() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        Vec::new(),
        Policy {
            mode: Mode::Single,
            ..Policy::default()
        },
    );
    executor.wire = Err("the Codex login is missing".to_string());
    let report = executor.execute(&briefing(TASK)).await;
    assert!(matches!(
        report.status,
        Status::Transport { reached: false, .. }
    ));
    assert_eq!(crate::delegate::charge(&report).0, "zero");
    let _ = say("unused", TokenUsage::default());
}
