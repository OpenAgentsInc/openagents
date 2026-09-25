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
        parts_met: false,
    };
    // Every part met: a done session's retry moves on, unless the verdict
    // calls it failed.
    let met = Signals {
        parts_met: true,
        ..at(false, false, true, 1)
    };
    assert_eq!(settle(Some(Move::Retry), &ran, met).0, Move::Next);
    let met_failing = Signals {
        verdict_fail: true,
        ..met
    };
    assert_eq!(settle(Some(Move::Retry), &ran, met_failing).0, Move::Retry);
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
        parts_met: false,
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
        general: false,
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

/// A writer that keeps a guard: T1 passes on the untouched workspace,
/// where hello.txt already holds hello, and T2 fails there.
fn writes_a_guard_and_a_deciding_test() -> Vec<microluna::Reply> {
    vec![
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
        finish("w3", "done", "Wrote a guard for R1 and T2 for R2."),
    ]
}

/// Session 1 fixes world.txt and, following the task, changes the file
/// the guard T1 pins; session 2, when it runs, restores the guard.
fn turns_a_guard_red() -> Vec<microluna::Reply> {
    vec![
        call(
            "s1a",
            "write_file",
            &json!({ "path": "world.txt", "contents": "world\n" }),
            usage(900, 0, 40),
        ),
        call(
            "s1b",
            "write_file",
            &json!({ "path": "hello.txt", "contents": "HELLO\n" }),
            usage(900, 800, 40),
        ),
        finish("s1f", "done", "Wrote world.txt and changed hello.txt."),
        call(
            "s2a",
            "write_file",
            &json!({ "path": "hello.txt", "contents": "hello\n" }),
            usage(900, 800, 40),
        ),
        finish("s2f", "done", "Restored hello.txt for T1."),
    ]
}

fn guard_policy(advisory: bool) -> Policy {
    let mut writer = one_round().unwrap();
    writer.guards = true;
    Policy {
        suite: true,
        checks: false,
        suite_writer: Some(writer),
        advisory_guards: advisory,
        ..Policy::default()
    }
}

async fn run_guarded(advisory: bool) -> (Value, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(dir.path(), turns_a_guard_red(), guard_policy(advisory));
    std::fs::write(dir.path().join("work/hello.txt"), "hello\n").unwrap();
    std::fs::write(dir.path().join("work/world.txt"), "nope\n").unwrap();
    fake(&executor).lane(
        "Write an executable acceptance suite",
        writes_a_guard_and_a_deciding_test(),
    );
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    (record, dir)
}

/// v8's advisory guards: a guard that session 1 turns red doesn't send a
/// second session to turn it green; the loop stops on the deciding test
/// and names the guard. v7 sends the second session.
#[tokio::test]
async fn a_guard_an_edit_turns_red_doesnt_hold_the_loop() {
    let (record, dir) = run_guarded(true).await;
    assert_eq!(
        record["sessions"].as_array().unwrap().len(),
        1,
        "{record:#}"
    );
    let stopped = record["stopped"].as_str().unwrap();
    assert!(
        stopped.contains("green after session 1 (1 of 1)"),
        "{stopped}"
    );
    assert!(stopped.contains("the guard T1"), "{stopped}");
    let run = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "run" && m["after_session"] == 1)
        .unwrap();
    assert_eq!(run["advisory_guards"], json!(["T1"]), "{run:#}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("work/hello.txt")).unwrap(),
        "HELLO\n",
        "the change session 1 made stands"
    );

    let (record, dir) = run_guarded(false).await;
    assert_eq!(
        record["sessions"].as_array().unwrap().len(),
        2,
        "{record:#}"
    );
    assert!(
        record["stopped"]
            .as_str()
            .unwrap()
            .contains("green after session 2 (2 of 2)"),
        "{record:#}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("work/hello.txt")).unwrap(),
        "hello\n"
    );
}

/// The note a later session reads names each red guard and says the host
/// doesn't count it.
#[test]
fn the_guard_note_names_each_guard() {
    let one = guard_note(&["T10".to_string()]);
    assert!(one.starts_with("T10 passed on the untouched workspace and fails now."));
    assert!(one.contains("doesn't count it"));
    let two = guard_note(&["T1".to_string(), "T4".to_string()]);
    assert!(two.starts_with("T1, T4 passed on the untouched workspace and fail now."));
    assert!(two.contains("doesn't count them"));
}

/// v8's gap_overlap: the first gap round runs as soon as the suite is
/// frozen with a gap, beside session 1, so the loop's first run already
/// has the gap's test and no gap round follows the green.
#[tokio::test]
async fn the_gap_round_runs_beside_session_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        Vec::new(),
        Policy {
            suite: true,
            checks: false,
            suite_writer: one_round(),
            overlap_suite: true,
            gap_rounds: 1,
            gap_overlap: true,
            fast_runs: true,
            ..Policy::default()
        },
    );
    wrong_files(dir.path());
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
    let mut early = vec![call(
        "early-sleep",
        "run_command",
        &json!({ "command": "sleep 2", "timeout_seconds": null }),
        usage(900, 0, 20),
    )];
    early.extend([
        call(
            "early-hello",
            "write_file",
            &json!({ "path": "hello.txt", "contents": "hello\n" }),
            usage(900, 800, 40),
        ),
        call(
            "early-world",
            "write_file",
            &json!({ "path": "world.txt", "contents": "world\n" }),
            usage(900, 800, 40),
        ),
        finish("early-finish", "done", "Wrote both files."),
    ]);
    fake(&executor).lane("while the acceptance suite is written", early);
    executor.take_evidence(&prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(report.status, Status::Answered, "{record:#}");
    let gaps: Vec<&Value> = record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["kind"] == "gap")
        .collect();
    assert_eq!(gaps.len(), 1, "{record:#}");
    assert_eq!(gaps[0]["overlapped"], json!(true));
    assert_eq!(gaps[0]["added"], json!(1));
    let session_end = record["sessions"][0]["end_ms"]
        .as_u64()
        .or_else(|| {
            record["parallel"]["tracks"]
                .as_array()?
                .iter()
                .find(|t| t["label"] == "session 1")?["end_ms"]
                .as_u64()
        })
        .unwrap();
    assert!(
        gaps[0]["end_ms"].as_u64().unwrap() <= session_end,
        "the gap round ended before session 1: {record:#}"
    );
    let stopped = record["stopped"].as_str().unwrap();
    assert!(
        stopped.contains("green after session 1 (2 of 2)"),
        "{stopped}"
    );
    assert!(
        !record["parallel"]["batches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["batch"] == "gap round 1"),
        "no gap round after the green: {record:#}"
    );
}

#[test]
fn v8_options_need_the_suite() {
    let policy = Policy {
        advisory_guards: true,
        gap_overlap: true,
        test_jobs: 9,
        ..Policy::default()
    };
    let problems = policy.validate().join("\n");
    assert!(
        problems.contains("test_jobs must be from 1 to 8"),
        "{problems}"
    );
    assert!(problems.contains("advisory_guards and gap_overlap need suite"));
    assert!(problems.contains("gap_overlap needs gap_rounds and overlap_suite"));
}

/// The general texts carry none of the words the flagged texts took from
/// one task's defects.
#[test]
fn the_general_guidance_is_task_neutral() {
    let texts = [
        EARLY_GUIDANCE_GENERAL.to_string(),
        audit_rule(true, false).to_string(),
        audit_rule(true, true).to_string(),
        close_requirement_question_general(0),
        crate::accept::DISCOVER_GENERAL.to_string(),
        crate::accept::STANDARD_METHODS_GENERAL.to_string(),
        crate::accept::verify::FAITHFUL_GENERAL.to_string(),
        crate::accept::GENERAL_MARKS.join(" "),
    ];
    for text in &texts {
        let lower = text.to_ascii_lowercase();
        for word in [
            "biased",
            "estimator",
            "statistic",
            "window",
            "calibrat",
            "25, 50",
            "sizes",
            "adapts",
            "non-degenerate",
            "zero vector",
            "distribution",
            "textbook",
        ] {
            assert!(!lower.contains(word), "{word:?} in {text}");
        }
    }
    // v7's texts are unchanged.
    assert_eq!(
        audit_rule(false, false),
        "Audit the work: for each requirement, read the code that implements it and check it \
         against the task's exact rule and the standard definition of any method the task names, \
         and against the choices the code defends in its comments. Fix what's wrong without \
         turning an acceptance test red, run the suite, and call finish."
    );
}

#[test]
fn outputs_the_task_names_but_nobody_wrote_are_missing() {
    let dir = tempfile::tempdir().unwrap();
    let (work, base) = (dir.path().join("work"), dir.path().join("base"));
    std::fs::create_dir_all(work.join("logs")).unwrap();
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(work.join("summarize.py"), "x").unwrap();
    std::fs::write(base.join("input.json"), "{}").unwrap();
    let task = "Read input.json and logs/YYYY-MM-DD_web.log, then write a CSV file \
                summary.csv and `report.md`. Keep summarize.py.";
    assert_eq!(
        missing_outputs(task, &work, Some(&base)),
        ["summary.csv", "report.md"]
    );
    std::fs::write(work.join("summary.csv"), "a").unwrap();
    assert_eq!(missing_outputs(task, &work, Some(&base)), ["report.md"]);
}

fn lean_policy(lean: lean::Lean) -> Policy {
    Policy {
        lean: Some(lean),
        spend_usd: 1.0,
        ..Policy::default()
    }
}

fn lean_shape() -> lean::Lean {
    lean::Lean {
        sessions: 1,
        source_chars: 10_000,
        sample_chars: 2_000,
        self_check: true,
        holdout: false,
        hardcode_check: false,
        keep_best: false,
        score_sec: 20,
        persist: None,
        wall_sec: 0,
        practices: false,
        defended: false,
        session_spend: false,
        records: false,
        command_sec: 0,
        protect_candidates: false,
        retain_candidates: false,
        observe_review: false,
        symptoms: false,
        example_first: false,
        standard_forms: false,
        rationale: false,
        lanes: 0,
        lane_sec: 0,
        failures: false,
        structure: false,
        detect: None,
    }
}

#[tokio::test]
async fn the_lean_loop_runs_one_session_then_the_self_check() {
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
            finish("c3", "done", "Wrote hello.txt; cat shows hello."),
            finish("c4", "done", "Checked hello.txt against the task."),
        ],
        lean_policy(lean_shape()),
    );
    executor.prepared = Some(prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert_eq!(record["mode"], "lean");
    assert_eq!(record["sessions"].as_array().unwrap().len(), 2);
    let stopped = record["stopped"].as_str().unwrap();
    assert!(stopped.contains("session 1 ended done"), "{stopped}");
    assert!(stopped.contains("the self-check ended done"), "{stopped}");
    assert_eq!(record["moves"][1]["self_check"], true);
}

#[tokio::test]
async fn the_lean_loop_finishes_on_the_best_scoring_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let eval = lean::eval_dir(&work, Isolation::TaskContainer);
    let _ = std::fs::remove_dir_all(&eval);
    let script = format!(
        "mkdir -p {e} && printf '%s\\n' 'if grep -q hello hello.txt; then echo SCORE 1 1; else echo SCORE 0 1; fi' > {e}/score.sh",
        e = eval.display()
    );
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "a1",
                "run_command",
                &json!({ "command": script, "timeout_seconds": null }),
                usage(1_000, 0, 30),
            ),
            call(
                "a2",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "hello\n" }),
                usage(1_000, 0, 30),
            ),
            finish("a3", "blocked", "Wrote hello.txt but not world.txt."),
            call(
                "b1",
                "write_file",
                &json!({ "path": "hello.txt", "contents": "bye\n" }),
                usage(1_000, 0, 30),
            ),
            finish("b2", "blocked", "Changed hello.txt."),
            finish("c1", "blocked", "Changed nothing."),
        ],
        lean_policy(lean::Lean {
            sessions: 3,
            self_check: false,
            keep_best: true,
            ..lean_shape()
        }),
    );
    executor.prepared = Some(prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    let moves = record["moves"].as_array().unwrap();
    assert_eq!(moves[0]["score"]["passed"], 1);
    assert_eq!(moves[1]["score"]["passed"], 0);
    assert_eq!(moves[0]["kept"], true);
    assert_eq!(moves[1]["kept"], false);
    assert_eq!(moves.last().unwrap()["kind"], "lean.restore");
    assert_eq!(
        std::fs::read_to_string(work.join("hello.txt")).unwrap(),
        "hello\n"
    );
    assert!(!eval.exists(), "the eval directory is removed");
}

#[test]
fn a_lookup_table_of_the_examples_is_flagged_and_a_rule_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    std::fs::create_dir_all(work.join("data")).unwrap();
    let pairs: Vec<(String, String)> = (0..40)
        .map(|i| (format!("proto{i}word"), format!("modern{i}form")))
        .collect();
    let table: String = pairs.iter().map(|(a, b)| format!("{a}\t{b}\n")).collect();
    std::fs::write(work.join("data/train.tsv"), &table).unwrap();
    std::fs::write(work.join("solve.py"), "print('todo')\n").unwrap();
    let fields = lean::data_fields(work);
    let start = parallel::tree(work);
    std::fs::write(
        work.join("rules.json"),
        r#"[{"name": "o-to-e", "src": "o", "tgt": "e", "left": "", "right": ""}]"#,
    )
    .unwrap();
    assert!(lean::literal_examples(work, &start, &fields).is_empty());
    let lookup: String = pairs
        .iter()
        .map(|(a, b)| format!("{{\"src\": \"{a}\", \"tgt\": \"{b}\"}},\n"))
        .collect();
    std::fs::write(work.join("rules.json"), lookup).unwrap();
    let flagged = lean::literal_examples(work, &start, &fields);
    assert_eq!(flagged.len(), 1, "{flagged:?}");
    assert_eq!(flagged[0].0, "rules.json");
    assert_eq!(flagged[0].2, 80);
}

#[test]
fn a_score_is_the_last_score_line() {
    assert_eq!(lean::parse_score("x\nSCORE 3 9\nSCORE 5 9\n"), Some((5, 9)));
    assert_eq!(lean::parse_score("SCORE 12 9"), None);
    assert_eq!(lean::parse_score("SCORE 1 1\nSCORE broken"), None);
    assert_eq!(lean::parse_score("SCORE 1 1 trailing"), None);
    assert_eq!(lean::parse_score("SCORE 1 0\nnothing"), None);
}

#[tokio::test]
async fn the_lean_loop_starts_no_session_in_its_last_minute() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![finish("c1", "done", "Never sent.")],
        lean_policy(lean::Lean {
            wall_sec: 30,
            ..lean_shape()
        }),
    );
    executor.prepared = Some(prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    assert!(record["sessions"].as_array().unwrap().is_empty());
    assert!(record["stopped"].as_str().unwrap().contains("time ran out"));
}

#[test]
fn a_word_list_isnt_hard_coded_examples_but_a_table_of_answers_is() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    std::fs::create_dir_all(work.join("data")).unwrap();
    let words: String = (0..60).map(|i| format!("word{i}x\n")).collect();
    std::fs::write(work.join("data/words.txt"), &words).unwrap();
    let pairs: String = (0..40).map(|i| format!("in{i}put\tout{i}put\n")).collect();
    std::fs::write(work.join("data/train.tsv"), &pairs).unwrap();
    let records = lean::data_records(work);
    assert!(!records.contains_key("data/words.txt"), "{records:?}");
    let start = parallel::tree(work);
    std::fs::write(
        work.join("solve.py"),
        format!("WORDS = \"\"\"{words}\"\"\"\n"),
    )
    .unwrap();
    assert!(lean::literal_examples(work, &start, &records).is_empty());
    let table: String = (0..40)
        .map(|i| format!("'in{i}put': 'out{i}put',\n"))
        .collect();
    std::fs::write(work.join("solve.py"), table).unwrap();
    let flagged = lean::literal_examples(work, &start, &records);
    assert_eq!(flagged.len(), 1, "{flagged:?}");
    assert_eq!(flagged[0].2, 40);
}

#[tokio::test]
async fn retention_keeps_an_editing_reviews_tied_improvement_without_an_extra_evaluation() {
    for break_copy in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let eval = lean::eval_dir(&work, Isolation::TaskContainer);
        let calls = dir.path().join("score-calls");
        let script = format!(
            "mkdir -p {} && printf 'printf x >> {}\\necho SCORE 1 1\\n' > {}/score.sh",
            eval.display(),
            calls.display(),
            eval.display()
        );
        let mut executor = micro(
            dir.path(),
            vec![
                call(
                    "a1",
                    "run_command",
                    &json!({"command": script}),
                    usage(100, 0, 10),
                ),
                call(
                    "a2",
                    "write_file",
                    &json!({"path":"hello.txt","contents":"incomplete\n"}),
                    usage(100, 0, 10),
                ),
                finish("a3", "done", "The weak check passes."),
                call(
                    "b1",
                    "write_file",
                    &json!({"path":"hello.txt","contents":"corrected\n"}),
                    usage(100, 0, 10),
                ),
                finish("b2", "done", "The review fixed a missed requirement."),
            ],
            lean_policy(lean::Lean {
                keep_best: true,
                retain_candidates: true,
                ..lean_shape()
            }),
        );
        if break_copy {
            let retained = dir.path().join("artifacts/lean-1");
            std::fs::create_dir_all(&retained).unwrap();
            std::fs::write(retained.join("session-2"), "not a directory").unwrap();
        }
        executor.prepared = Some(prepared());
        executor.execute(&briefing(TASK)).await;
        assert_eq!(
            std::fs::read_to_string(work.join("hello.txt")).unwrap(),
            "corrected\n"
        );
        assert_eq!(std::fs::read_to_string(calls).unwrap(), "xx");
        let record = executor.last.as_ref().unwrap();
        let moves = record["moves"].as_array().unwrap();
        let attempts: Vec<_> = moves.iter().filter(|m| m["kind"] == "lean").collect();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1]["kept"], true);
        assert_eq!(attempts[1]["self_check"], true);
        let first = PathBuf::from(attempts[0]["candidate"].as_str().unwrap());
        assert_eq!(
            std::fs::read_to_string(first.join("hello.txt")).unwrap(),
            "incomplete\n"
        );
        let submitted = moves.last().unwrap();
        assert_eq!(submitted["evaluation_rerun"], false);
        assert_eq!(submitted["result"], "observed_without_revalidation");
        assert_eq!(submitted["review_status"], "done");
        if break_copy {
            assert!(attempts[1]["snapshot_error"].is_string());
            assert!(submitted["selected_session"].is_null());
        } else {
            assert_eq!(submitted["selected_session"], 2);
            let second = PathBuf::from(attempts[1]["candidate"].as_str().unwrap());
            assert_eq!(
                std::fs::read_to_string(second.join("hello.txt")).unwrap(),
                "corrected\n"
            );
        }
    }
}

#[tokio::test]
async fn protected_candidates_keep_the_first_tie_and_an_observer_cannot_edit_it() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let eval = lean::eval_dir(&work, Isolation::TaskContainer);
    let script = format!(
        "mkdir -p {} && printf 'echo SCORE 1 1\\n' > {}/score.sh",
        eval.display(),
        eval.display()
    );
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "a1",
                "run_command",
                &json!({"command": script}),
                usage(100, 0, 10),
            ),
            call(
                "a2",
                "write_file",
                &json!({"path":"hello.txt","contents":"correct\n"}),
                usage(100, 0, 10),
            ),
            finish("a3", "blocked", "Another requirement is unknown."),
            call(
                "b1",
                "write_file",
                &json!({"path":"hello.txt","contents":"regression\n"}),
                usage(100, 0, 10),
            ),
            finish("b2", "blocked", "The weak score still passes."),
            call(
                "c1",
                "write_file",
                &json!({"path":"hello.txt","contents":"review edit\n"}),
                usage(100, 0, 10),
            ),
            call(
                "c2",
                "run_command",
                &json!({"command":"printf 'shell edit' > hello.txt"}),
                usage(100, 0, 10),
            ),
            finish("c3", "blocked", "The self-test does not cover the task."),
        ],
        lean_policy(lean::Lean {
            sessions: 2,
            keep_best: true,
            protect_candidates: true,
            observe_review: true,
            ..lean_shape()
        }),
    );
    executor.prepared = Some(prepared());
    let report = executor.execute(&briefing(TASK)).await;
    let output = report.output();
    assert!(output.contains("Another requirement is unknown."));
    assert!(output.contains("The self-test does not cover the task."));
    assert!(!output.contains("The weak score still passes."));
    assert_eq!(
        std::fs::read_to_string(work.join("hello.txt")).unwrap(),
        "correct\n"
    );
    let record = executor.last.as_ref().unwrap();
    let moves = record["moves"].as_array().unwrap();
    assert_eq!(moves[0]["kept"], true);
    assert_eq!(moves[1]["kept"], false);
    let candidate = PathBuf::from(moves[1]["candidate"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(candidate.join("hello.txt")).unwrap(),
        "regression\n"
    );
    let reviewed = PathBuf::from(moves[2]["candidate"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(reviewed.join("hello.txt")).unwrap(),
        "correct\n"
    );
    let submitted = moves.last().unwrap();
    assert_eq!(submitted["kind"], "lean.submitted");
    assert_eq!(submitted["selected_session"], 1);
    assert_eq!(submitted["review_status"], "blocked");
    assert!(submitted["benchmark_outcome"].is_null());
    assert!(candidate.parent().unwrap().join("selection.json").is_file());
    assert!(
        candidate
            .parent()
            .unwrap()
            .join("evaluator/score.sh")
            .is_file()
    );
}

#[tokio::test]
async fn missing_scores_remain_unknown_and_retries_stay_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let mut executor = micro(
        dir.path(),
        vec![
            finish("a", "done", "No evidence."),
            finish("b", "done", "Still no evidence."),
        ],
        lean_policy(lean::Lean {
            sessions: 2,
            self_check: false,
            keep_best: true,
            protect_candidates: true,
            ..lean_shape()
        }),
    );
    executor.prepared = Some(prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.as_ref().unwrap();
    assert_eq!(record["sessions"].as_array().unwrap().len(), 2);
    let submitted = record["moves"].as_array().unwrap().last().unwrap();
    assert_eq!(submitted["result"], "unknown");
    assert!(submitted["score"].is_null());
}

#[tokio::test]
async fn failed_or_timed_out_evaluation_cannot_supply_a_green_score() {
    for script in ["echo SCORE 1 1; exit 7", "echo SCORE 1 1; sleep 2"] {
        let dir = tempfile::tempdir().unwrap();
        let executor = micro(dir.path(), vec![], lean_policy(lean_shape()));
        let frozen = dir.path().join("evaluation");
        std::fs::create_dir_all(&frozen).unwrap();
        std::fs::write(frozen.join("score.sh"), script).unwrap();
        let (score, output) = executor
            .lean_score(&frozen, &lean_shape(), Duration::from_secs(1))
            .await;
        assert!(score.is_none(), "{script}: {output}");
        assert!(output.contains("SCORE 1 1"));
    }
}

#[tokio::test]
async fn a_final_evaluator_edit_cannot_claim_the_retained_candidates_identity() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let eval = lean::eval_dir(&work, Isolation::TaskContainer);
    let script = format!(
        "mkdir -p {} && printf 'printf x >> touched\\necho SCORE 1 1\\n' > {}/score.sh",
        eval.display(),
        eval.display()
    );
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "a1",
                "run_command",
                &json!({"command": script}),
                usage(100, 0, 10),
            ),
            finish("a2", "done", "The local check passes."),
        ],
        lean_policy(lean::Lean {
            sessions: 1,
            self_check: false,
            keep_best: true,
            protect_candidates: true,
            ..lean_shape()
        }),
    );
    executor.prepared = Some(prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.as_ref().unwrap();
    let submitted = record["moves"].as_array().unwrap().last().unwrap();
    assert_eq!(submitted["score"], json!({"passed": 1, "total": 1}));
    assert_eq!(submitted["result"], "unknown");
    assert_eq!(submitted["selection_matches_workspace"], false);
    assert!(submitted["selected_session"].is_null());
    assert_eq!(std::fs::read_to_string(work.join("touched")).unwrap(), "xx");
}

#[test]
fn candidate_identity_refuses_an_incomplete_inventory() {
    use std::os::unix::ffi::OsStringExt;

    let dir = tempfile::tempdir().unwrap();
    assert!(lean::evidence_tree(&dir.path().join("missing")).is_err());
    std::fs::write(dir.path().join("source.txt"), "retained").unwrap();
    assert_eq!(lean::evidence_tree(dir.path()).unwrap().len(), 1);
    let name = std::ffi::OsString::from_vec(vec![0xff]);
    std::fs::write(dir.path().join(name), "cannot identify this path in JSON").unwrap();
    assert!(lean::evidence_tree(dir.path()).is_err());
}

#[test]
fn protected_candidates_refuse_parallel_lanes_without_retained_lane_evidence() {
    for protect_candidates in [false, true] {
        let shape = lean::Lean {
            keep_best: true,
            protect_candidates,
            retain_candidates: !protect_candidates,
            lanes: 3,
            ..lean_shape()
        };
        assert!(
            shape
                .validate()
                .iter()
                .any(|problem| problem.contains("one first-attempt lane"))
        );
    }
}

#[test]
fn a_comment_that_gives_a_reason_is_a_suspect() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("window.py"),
        "\"\"\"Keeps a window.\n\nThe window slides because recent data matters more.\n\"\"\"\n\
         def keep(x):\n    # plain comment\n    return x\n",
    )
    .unwrap();
    let found = crate::accept::rationale_choices(dir.path());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("window.py:3:"), "{found:?}");
}

#[test]
fn a_rebase_changes_whole_path_mentions_only() {
    assert_eq!(
        lean::rebase_text(
            "cd /app && cat /app/x '/app' /apple /app_b",
            "/app",
            "/tmp/l"
        ),
        "cd /tmp/l && cat /tmp/l/x '/tmp/l' /apple /app_b"
    );
}

#[tokio::test]
async fn lanes_run_at_once_and_the_best_scoring_copy_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let eval = lean::eval_dir(&work, Isolation::TaskContainer);
    let _ = std::fs::remove_dir_all(&eval);
    // The score reads the workspace by its absolute path, as a task's
    // scripts do, so each lane's copy is rebased.
    let score = format!(
        "mkdir -p {e} && printf '%s\\n' 'if grep -q good {w}/answer.txt 2>/dev/null; then echo SCORE 1 1; else echo SCORE 0 1; fi' > {e}/score.sh",
        e = eval.display(),
        w = work
            .canonicalize()
            .unwrap_or_else(|_| work.clone())
            .display()
    );
    let mut executor = micro(
        dir.path(),
        vec![
            call(
                "s1",
                "run_command",
                &json!({ "command": score, "timeout_seconds": null }),
                usage(1_000, 0, 30),
            ),
            finish("s2", "done", "Wrote the score."),
            // The fake transport answers the two lanes in turn.
            call(
                "l1",
                "write_file",
                &json!({ "path": "answer.txt", "contents": "bad\n" }),
                usage(1_000, 0, 30),
            ),
            call(
                "l2",
                "write_file",
                &json!({ "path": "answer.txt", "contents": "good\n" }),
                usage(1_000, 0, 30),
            ),
            finish("l3", "done", "Wrote an answer."),
            finish("l4", "done", "Wrote an answer."),
            finish("c1", "done", "Checked."),
        ],
        lean_policy(lean::Lean {
            sessions: 1,
            self_check: false,
            keep_best: true,
            lanes: 2,
            ..lean_shape()
        }),
    );
    executor.prepared = Some(prepared());
    executor.execute(&briefing(TASK)).await;
    let record = executor.last.clone().unwrap();
    let moves = record["moves"].as_array().unwrap();
    let lanes = moves
        .iter()
        .find(|m| m["kind"] == "lean.lanes")
        .expect("a lanes record");
    let scores: Vec<u64> = lanes["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["score"]["passed"].as_u64().unwrap())
        .collect();
    assert_eq!(scores.iter().sum::<u64>(), 1, "{lanes}");
    assert_eq!(
        std::fs::read_to_string(work.join("answer.txt")).unwrap(),
        "good\n"
    );
    assert_eq!(lanes["leaked"], false);
}
