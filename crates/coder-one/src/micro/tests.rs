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
        commands: Vec::new(),
        changed: Vec::new(),
    };
    assert_eq!(settle(Some(Move::Next), &ran, false, 1, 2).0, Move::Next);
    assert_eq!(settle(Some(Move::Done), &ran, true, 1, 2).0, Move::Retry);
    assert_eq!(settle(Some(Move::Next), &ran, true, 2, 2).0, Move::Stuck);
    assert_eq!(settle(None, &ran, false, 1, 2).0, Move::Next);
    assert_eq!(settle(Some(Move::Retry), &ran, false, 2, 2).0, Move::Stuck);
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
