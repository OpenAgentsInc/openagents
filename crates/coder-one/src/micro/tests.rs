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

fn requirement(
    id: &str,
    kind: crate::requirements::Kind,
    text: &str,
) -> crate::requirements::Requirement {
    crate::requirements::Requirement {
        id: id.to_string(),
        spans: Vec::new(),
        kind,
        binding: crate::requirements::Binding::Unjudged,
        state: crate::requirements::State::Unobserved,
        p: None,
        exhaustive: None,
        text: text.to_string(),
        extracted: crate::requirements::Extracted::default(),
    }
}

fn map_of(reqs: Vec<crate::requirements::Requirement>) -> crate::requirements::RequirementMap {
    crate::requirements::RequirementMap {
        schema: "test".to_string(),
        instruction_sha256: String::new(),
        method: "rule".to_string(),
        spans: Vec::new(),
        requirements: reqs,
        coverage: crate::requirements::Coverage {
            chars: 0,
            covered: 0,
            fraction: 0.0,
            spans: 0,
            requirement_spans: 0,
            context_spans: 0,
            unanswered_spans: 0,
        },
    }
}

#[test]
fn focus_actionable_groups_only_actionable_requirements_and_lists_the_constraints() {
    use crate::requirements::Kind;
    let map = map_of(vec![
        requirement("R1", Kind::Constraint, "use gene_name as the identifier"),
        requirement("R2", Kind::Behavior, "run the analysis"),
        requirement("R3", Kind::Deliverable, "save results.csv"),
        requirement("R4", Kind::Constraint, "set the random seed to 149"),
        requirement("R5", Kind::Context, "the data lives in data/"),
    ]);
    // Off: every non-context requirement is grouped, as v1 through v4 do.
    let all = groups(&map, 4, false);
    let all_ids: Vec<String> = all.iter().flat_map(|g| g.ids.clone()).collect();
    assert_eq!(all_ids, vec!["R1", "R2", "R3", "R4"], "{all:?}");
    // On: only the behavior and the deliverable are grouped.
    let focused = groups(&map, 4, true);
    let focused_ids: Vec<String> = focused.iter().flat_map(|g| g.ids.clone()).collect();
    assert_eq!(focused_ids, vec!["R2", "R3"], "{focused:?}");
    // The constraints reach the session through the brief instead.
    let lines = constraints(&map);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("R1") && lines[0].contains("gene_name"));
    assert!(lines[1].contains("R4") && lines[1].contains("149"));
}

#[test]
fn focus_actionable_falls_back_when_a_task_has_only_constraints() {
    use crate::requirements::Kind;
    let map = map_of(vec![
        requirement("R1", Kind::Constraint, "don't cheat"),
        requirement("R2", Kind::Constraint, "you have 600 seconds"),
    ]);
    let focused = groups(&map, 4, true);
    let ids: Vec<String> = focused.iter().flat_map(|g| g.ids.clone()).collect();
    assert_eq!(ids, vec!["R1", "R2"], "a constraint-only task still runs");
}

#[tokio::test]
async fn the_loop_rebuilds_each_session_and_bounds_retries() {
    let dir = tempfile::tempdir().unwrap();
    let map = prepared().requirements;
    let groups = groups(&map, 4, false);
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
            cause: microluna::Cause::None,
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
        changed_workspace: true,
        ran_command: true,
        read_only: false,
        started_at_ms: 0,
        ended_at_ms: 0,
        place: Place::default(),
        read_turns: 0,
    };
    let at = |contradicted, verdict_fail, last, attempts| Signals {
        contradicted,
        verdict_fail,
        last,
        read_only: false,
        evidence: true,
        require_evidence: true,
        accepts: None,
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
        accepts: None,
        attempts: 1,
        max_attempts: 2,
    };
    // The accept gate: an ending move waits for the checks to confirm.
    let ev_last = Signals {
        evidence: true,
        last: true,
        ..no_ev(true)
    };
    assert_eq!(
        settle(
            Some(Move::Next),
            &ran,
            Signals {
                accepts: Some(false),
                ..ev_last
            }
        )
        .0,
        Move::Retry
    );
    assert_eq!(
        settle(
            Some(Move::Next),
            &ran,
            Signals {
                accepts: Some(true),
                ..ev_last
            }
        )
        .0,
        Move::Next
    );
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

#[tokio::test]
async fn the_suite_loop_writes_a_suite_then_edits_until_it_is_green() {
    let dir = tempfile::tempdir().unwrap();
    let test = "#!/bin/sh\n# requirement: R1\n# kind: example\n# what: hello.txt holds hello\ngrep -qx hello hello.txt\n";
    let test2 = "#!/bin/sh\n# requirement: R2\n# kind: example\n# what: world.txt holds world\ngrep -qx world world.txt\n";
    let mut executor = micro(
        dir.path(),
        vec![
            // The writing session, whose workspace is the suite directory.
            call(
                "w1",
                "write_file",
                &json!({ "path": "tests/T1.sh", "contents": test }),
                usage(900, 0, 40),
            ),
            call(
                "w2",
                "write_file",
                &json!({ "path": "tests/T2.sh", "contents": test2 }),
                usage(900, 800, 40),
            ),
            finish("w3", "done", "Wrote T1 for R1 and T2 for R2."),
            // Two more writing rounds run with Jev off; the tests are
            // already on disk, so each round just finishes.
            finish("w4", "done", "The tests already cover R1 and R2."),
            finish("w5", "done", "The tests already cover R1 and R2."),
            // The edit session.
            call(
                "e1",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "hello\n" }),
                usage(900, 0, 40),
            ),
            call(
                "e2",
                "write_file",
                &json!({ "path": "world.txt", "contents": "world\n" }),
                usage(900, 800, 40),
            ),
            finish("e3", "done", "Wrote hello.txt and world.txt."),
        ],
        Policy {
            suite: true,
            checks: false,
            ..Policy::default()
        },
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    let stopped = record["stopped"].as_str().unwrap();
    assert!(stopped.contains("green after session 1"), "{record:#}");
    assert_eq!(record["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(record["moves"][0]["kind"], json!("suite"));
    assert_eq!(record["moves"][0]["tests"], json!(2));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("work/hello.txt")).unwrap(),
        "hello\n"
    );
}

const HELLO_TEST: &str = "#!/bin/sh\n# requirement: R1\n# kind: example\n# what: hello.txt holds hello\ngrep -qx hello hello.txt\n";
const WORLD_TEST: &str = "#!/bin/sh\n# requirement: R2\n# kind: example\n# what: world.txt holds world\ngrep -qx world world.txt\n";

/// A writer that writes both tests in one round.
fn writes_the_suite(sleep: bool) -> Vec<microluna::Reply> {
    let mut replies = Vec::new();
    if sleep {
        replies.push(call(
            "w0",
            "run_command",
            &json!({ "command": "sleep 1", "timeout_seconds": null }),
            usage(900, 0, 20),
        ));
    }
    replies.extend([
        call(
            "w1",
            "write_file",
            &json!({ "path": "tests/T1.sh", "contents": HELLO_TEST }),
            usage(900, 0, 40),
        ),
        call(
            "w2",
            "write_file",
            &json!({ "path": "tests/T2.sh", "contents": WORLD_TEST }),
            usage(900, 800, 40),
        ),
        finish("w3", "done", "Wrote T1 for R1 and T2 for R2."),
    ]);
    replies
}

/// A session that sleeps a second, writes `path`, and finishes.
fn sleeps_then_writes(id: &str, path: &str, contents: &str) -> Vec<microluna::Reply> {
    vec![
        call(
            &format!("{id}-sleep"),
            "run_command",
            &json!({ "command": "sleep 1", "timeout_seconds": null }),
            usage(900, 0, 20),
        ),
        call(
            &format!("{id}-write"),
            "write_file",
            &json!({ "path": path, "contents": contents }),
            usage(900, 800, 40),
        ),
        finish(&format!("{id}-finish"), "done", &format!("Wrote {path}.")),
    ]
}

fn fake(executor: &Micro) -> &FakeTransport {
    match &executor.wire {
        Ok(Wire::Fake(fake)) => fake,
        _ => panic!("the test executor has a fake transport"),
    }
}

/// v7's writer: one round, rewrites only for hard failures.
fn one_round() -> Option<SuiteWriter> {
    Some(SuiteWriter {
        writers: 1,
        rewrite: crate::accept::Rewrite::Hard,
        rounds: 1,
        turns: 10,
        repair_turns: 4,
        effort: None,
        one_pass: true,
        discover: false,
        inventory: false,
        standard_methods: false,
        guards: false,
    })
}

fn wrong_files(dir: &Path) {
    std::fs::write(dir.join("work/hello.txt"), "nope\n").unwrap();
    std::fs::write(dir.join("work/world.txt"), "nope\n").unwrap();
}

/// Two red tests on different files run as two sessions at once, each in
/// its own copy; each sleeps a second, and the round takes about one
/// second, not two. Their changes merge and the suite goes green.
#[tokio::test]
async fn independent_red_tests_run_at_once_and_merge() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        Vec::new(),
        Policy {
            suite: true,
            checks: false,
            suite_writer: one_round(),
            parallel_edits: 2,
            ..Policy::default()
        },
    );
    wrong_files(dir.path());
    fake(&executor).lane(
        "Write an executable acceptance suite",
        writes_the_suite(false),
    );
    fake(&executor).lane(
        "only on the red tests T1 (",
        sleeps_then_writes("a", "hello.txt", "hello\n"),
    );
    fake(&executor).lane(
        "only on the red tests T2 (",
        sleeps_then_writes("b", "world.txt", "world\n"),
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    assert!(
        record["stopped"]
            .as_str()
            .unwrap()
            .contains("green after session 2"),
        "{record:#}"
    );
    assert_eq!(record["mode"], json!("suite"));
    let read = |p: &str| std::fs::read_to_string(dir.path().join("work").join(p)).unwrap();
    assert_eq!(read("hello.txt"), "hello\n");
    assert_eq!(read("world.txt"), "world\n");
    // The two sessions overlapped: the round's span is about one sleep.
    let parallel = &record["parallel"];
    let round = parallel["batches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["batch"] == "round 1")
        .unwrap();
    let span = round["end_ms"].as_u64().unwrap() - round["start_ms"].as_u64().unwrap();
    assert!(round["sum_ms"].as_u64().unwrap() >= 2_000, "{round:#}");
    assert!(
        span < 1_800,
        "the sessions ran one after another: {round:#}"
    );
    assert_eq!(parallel["peak"], json!(2));
    assert!(
        parallel["overlaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["a"] == "session 1" && o["b"] == "session 2"),
        "{parallel:#}"
    );
    assert!(
        parallel["concurrency"].as_f64().unwrap() > 1.0,
        "{parallel:#}"
    );
    assert_eq!(parallel["merges"], json!(1));
    assert_eq!(parallel["conflicts"], json!(0));
    // Each session says its group and that it ran beside the other.
    let sessions = record["sessions"].as_array().unwrap();
    assert_eq!(sessions[0]["group"], json!("group 1 of 2: T1"));
    assert_eq!(sessions[1]["parallel_with"], json!([1]));
    let lanes: Vec<Value> = executor
        .recorder
        .steps()
        .iter()
        .filter_map(|s| s.extensions.get(parallel::LANE_EXTENSION).cloned())
        .collect();
    assert_eq!(lanes.len(), 2);
    assert_eq!(lanes[0]["batch"], json!("round 1"));
    assert!(
        executor
            .recorder
            .steps()
            .iter()
            .any(|s| s.extensions.contains_key(parallel::SUMMARY_EXTENSION))
    );
}

/// Two sessions at once that both write the same new file: the first
/// session's change stands, the second's is discarded, its test is
/// requeued, and the next round's one session finishes it.
#[tokio::test]
async fn a_conflicting_session_is_requeued() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![
            // The requeued round: one session on T2.
            call(
                "c1",
                "write_file",
                &json!({ "path": "world.txt", "contents": "world\n" }),
                usage(900, 800, 40),
            ),
            finish("c2", "done", "Wrote world.txt."),
        ],
        Policy {
            suite: true,
            checks: false,
            suite_writer: one_round(),
            parallel_edits: 2,
            ..Policy::default()
        },
    );
    wrong_files(dir.path());
    fake(&executor).lane(
        "Write an executable acceptance suite",
        writes_the_suite(false),
    );
    let clash = |id: &str, path: &str, contents: &str, note: &str| {
        vec![
            call(
                &format!("{id}-note"),
                "write_file",
                &json!({ "path": "notes.txt", "contents": note }),
                usage(900, 0, 20),
            ),
            call(
                &format!("{id}-write"),
                "write_file",
                &json!({ "path": path, "contents": contents }),
                usage(900, 800, 40),
            ),
            finish(&format!("{id}-finish"), "done", &format!("Wrote {path}.")),
        ]
    };
    fake(&executor).lane(
        "only on the red tests T1 (",
        clash("a", "hello.txt", "hello\n", "from session 1\n"),
    );
    fake(&executor).lane(
        "only on the red tests T2 (",
        clash("b", "world.txt", "world\n", "from session 2\n"),
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    let merge = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "merge")
        .unwrap();
    assert_eq!(merge["merge"]["applied"], json!([1]), "{merge:#}");
    assert_eq!(merge["merge"]["conflicts"][0]["session"], json!(2));
    assert_eq!(
        merge["merge"]["conflicts"][0]["files"],
        json!(["notes.txt"])
    );
    assert_eq!(merge["merge"]["requeued"], json!(["T2"]));
    let read = |p: &str| std::fs::read_to_string(dir.path().join("work").join(p)).unwrap();
    assert_eq!(
        read("notes.txt"),
        "from session 1\n",
        "the first diff stands"
    );
    assert_eq!(read("world.txt"), "world\n");
    assert!(
        record["stopped"]
            .as_str()
            .unwrap()
            .contains("green after session 3"),
        "{record:#}"
    );
    assert_eq!(record["parallel"]["conflicts"], json!(1));
    // The requeued session was told why.
    let requests = fake(&executor).requests();
    let last = serde_json::to_string(&requests.last().unwrap().input).unwrap();
    assert!(last.contains("clashed with an earlier session"), "{last}");
}

/// The first edit session fixes hello.txt in the workspace while the
/// suite is written: the proof ran on the snapshot, so T1 was red there
/// and stays in the suite, the writer and the session overlapped, and the
/// suite then runs on the real workspace with T1 already green.
#[tokio::test]
async fn the_first_session_edits_while_the_suite_is_proven_on_a_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "e1",
                "write_file",
                &json!({ "path": "world.txt", "contents": "world\n" }),
                usage(900, 800, 40),
            ),
            finish("e2", "done", "Wrote world.txt."),
        ],
        Policy {
            suite: true,
            checks: false,
            suite_writer: one_round(),
            overlap_suite: true,
            ..Policy::default()
        },
    );
    wrong_files(dir.path());
    fake(&executor).lane(
        "Write an executable acceptance suite",
        writes_the_suite(true),
    );
    fake(&executor).lane(
        "while the acceptance suite is written",
        sleeps_then_writes("early", "hello.txt", "hello\n"),
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    let suite = &record["moves"][0];
    assert_eq!(
        suite["tests"],
        json!(2),
        "T1 was red on the snapshot: {suite:#}"
    );
    assert_eq!(suite["rejected"], json!(0));
    assert_eq!(suite["on_snapshot"], json!(true));
    let first_run = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "run")
        .unwrap();
    assert_eq!(first_run["after_session"], json!(1));
    assert_eq!(first_run["passed"], json!(1), "{first_run:#}");
    assert!(
        record["stopped"]
            .as_str()
            .unwrap()
            .contains("green after session 2"),
        "{record:#}"
    );
    let parallel = &record["parallel"];
    assert!(
        parallel["overlaps"].as_array().unwrap().iter().any(|o| {
            (o["a"] == "accept.define" && o["b"] == "session 1")
                || (o["a"] == "session 1" && o["b"] == "accept.define")
        }),
        "{parallel:#}"
    );
    let suite_ms = parallel["suite_ms"].as_u64().unwrap();
    assert!(suite_ms >= 1_000, "{parallel:#}");
    assert!(
        parallel["suite_on_critical_path_ms"].as_u64().unwrap() < suite_ms,
        "{parallel:#}"
    );
    let sessions = record["sessions"].as_array().unwrap();
    assert_eq!(sessions[0]["alongside"], json!("accept.define"));
    // The frozen suite names the real workspace, not the snapshot.
    let run_sh = std::fs::read_to_string(dir.path().join("accept-suite-1/run.sh")).unwrap();
    assert!(run_sh.contains(&dir.path().join("work").display().to_string()));
}

/// With `handoff_jev`, two sessions in a row that leave the same red
/// tests with the same output stop the loop, and each move is recorded.
#[tokio::test]
async fn the_same_failure_twice_stops_the_loop() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![
            finish("s1", "done", "Looked around."),
            finish("s2", "done", "Looked around again."),
            finish("s3", "done", "Never reached."),
        ],
        Policy {
            suite: true,
            checks: false,
            suite_writer: one_round(),
            handoff_jev: true,
            ..Policy::default()
        },
    );
    wrong_files(dir.path());
    fake(&executor).lane(
        "Write an executable acceptance suite",
        writes_the_suite(false),
    );
    executor.take_evidence(&prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(
        record["stopped"],
        json!("the same red tests failed the same way after sessions 1 and 2"),
        "{record:#}"
    );
    assert_eq!(record["sessions"].as_array().unwrap().len(), 2);
    let moves: Vec<&Value> = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["kind"] == "move")
        .collect();
    assert_eq!(moves.len(), 1, "one Jev move, after session 1");
    assert_eq!(moves[0]["move"], json!("retry"));
    assert!(
        moves[0]["overridden"]
            .as_str()
            .unwrap()
            .contains("no answer")
    );
}

/// A green suite that is partial isn't done: a gap round writes a
/// deciding test for the open requirement on the snapshot, the suite is
/// frozen again with it, and the loop resumes until the whole suite is
/// green.
#[tokio::test]
async fn a_partial_green_runs_a_gap_round_then_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "s1",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "hello\n" }),
                usage(900, 0, 40),
            ),
            finish("s1f", "done", "Wrote hello.txt."),
            call(
                "s2",
                "write_file",
                &json!({ "path": "world.txt", "contents": "world\n" }),
                usage(900, 800, 40),
            ),
            finish("s2f", "done", "Wrote world.txt."),
        ],
        Policy {
            suite: true,
            checks: false,
            suite_writer: one_round(),
            gap_rounds: 1,
            fast_runs: true,
            ..Policy::default()
        },
    );
    wrong_files(dir.path());
    // The gap writer's brief also names the suite, so its lane comes first.
    fake(&executor).lane(
        "Write new tests only for them",
        vec![
            call(
                "g1",
                "write_file",
                &json!({ "path": "tests/T1.sh", "contents": WORLD_TEST }),
                usage(900, 0, 40),
            ),
            finish("g2", "done", "Wrote a test for R2."),
        ],
    );
    fake(&executor).lane(
        "Write an executable acceptance suite",
        vec![
            call(
                "w1",
                "write_file",
                &json!({ "path": "tests/T1.sh", "contents": HELLO_TEST }),
                usage(900, 0, 40),
            ),
            finish("w2", "done", "Wrote T1 for R1."),
        ],
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    let gap = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "gap")
        .unwrap_or_else(|| panic!("no gap round: {record:#}"));
    assert_eq!(gap["open"], json!(["R2"]));
    assert_eq!(gap["added"], json!(1));
    assert_eq!(gap["status"], json!("accepted"));
    let stopped = record["stopped"].as_str().unwrap();
    assert!(
        stopped.contains("green after session 2 (2 of 2)"),
        "{stopped}"
    );
    assert!(!stopped.contains("partial"), "{stopped}");
    // The new test joined the frozen suite as T2, naming the workspace.
    let t2 = std::fs::read_to_string(dir.path().join("accept-suite-1/tests/T2.sh")).unwrap();
    assert!(t2.contains("world.txt"));
    assert!(
        record["parallel"]["batches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["batch"] == "gap round 1")
    );
}
