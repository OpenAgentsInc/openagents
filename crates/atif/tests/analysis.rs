//! The analysis scans over the three record shapes: a session log, the
//! document it renders to, an SDK event stream, and a run directory.

use std::path::Path;

use atif::analysis::{self, TraceKind};
use atif::document::{Call, Outcome, Session, Source, Step};
use atif::log::{self, Log};
use serde_json::json;

fn call(id: &str, name: &str, args: serde_json::Value, outcome: Outcome) -> Call {
    Call {
        id: id.to_string(),
        name: name.to_string(),
        arguments: args,
        output: String::new(),
        outcome,
        milliseconds: 5,
        purpose: None,
        extra: Default::default(),
    }
}

fn step(at: u64, source: Source, message: &str) -> Step {
    Step {
        at,
        source,
        message: message.to_string(),
        reasoning: None,
        model: None,
        call: None,
        tokens: None,
        milliseconds: None,
        extensions: Default::default(),
    }
}

/// A log that exercises every anomaly kind: a repeated identical call run,
/// an identical failing run, a refusal, a stall, a huge result, an empty
/// agent step.
fn write_log(dir: &Path) -> std::path::PathBuf {
    let session = Session::opening("s-test", "m-test", "d-test", "/r", "0");
    let mut log = Log::create_at(&dir.join("s-test.atif.jsonl"), &session).expect("log");
    log.append(&step(1_000, Source::System, "boot")).unwrap();
    log.append(&step(1_100, Source::User, "do it")).unwrap();
    let mut s = step(1_200, Source::Agent, "reading");
    s.call = Some(call(
        "c1",
        "Read",
        json!({"file_path": "/r/a.rs"}),
        Outcome::Completed,
    ));
    log.append(&s).unwrap();
    // Three identical calls in a row: a loop.
    for (i, at) in [(2, 2_000u64), (3, 2_500), (4, 3_000)] {
        let mut s = step(at, Source::Agent, "");
        s.call = Some(call(
            &format!("c{i}"),
            "shell",
            json!({"command": "git status", "workdir": "/r"}),
            Outcome::Completed,
        ));
        log.append(&s).unwrap();
    }
    // Two identical failures in a row.
    for (i, at) in [(5, 4_000u64), (6, 4_500)] {
        let mut s = step(at, Source::Agent, "");
        s.call = Some(call(
            &format!("c{i}"),
            "shell",
            json!({"command": "cargo test -p missing", "workdir": "/r"}),
            Outcome::Failed,
        ));
        log.append(&s).unwrap();
    }
    // A refused call.
    let mut s = step(5_000, Source::Agent, "");
    s.call = Some(call(
        "c7",
        "shell",
        json!({"command": "rm -rf /"}),
        Outcome::Cancelled,
    ));
    log.append(&s).unwrap();
    // A stall: 75 seconds with no record.
    let mut s = step(80_500, Source::Agent, "");
    s.call = Some(call(
        "c8",
        "github",
        json!({"action": "issue_list"}),
        Outcome::Completed,
    ));
    s.call.as_mut().unwrap().output = "x".repeat(70 * 1024);
    log.append(&s).unwrap();
    // An agent step carrying nothing at all.
    log.append(&step(81_000, Source::Agent, "")).unwrap();
    log.finish(log::ENDED).unwrap();
    log.path().to_path_buf()
}

#[test]
fn a_log_reads_into_a_trace_with_every_anomaly() {
    let dir = tempfile::tempdir().expect("dir");
    let path = write_log(dir.path());
    let trace = analysis::read(&path).expect("trace");
    assert_eq!(trace.kind, TraceKind::Log);
    assert_eq!(trace.session_id, "s-test");
    assert_eq!(trace.state, "ended");
    assert_eq!(trace.calls.len(), 8);

    let found = analysis::anomalies(&trace);
    let kinds: Vec<&str> = found.iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&"stuck"), "{kinds:?}");
    assert!(kinds.contains(&"stuck_failure"), "{kinds:?}");
    assert!(kinds.contains(&"cancelled"), "{kinds:?}");
    assert!(kinds.contains(&"stall"), "{kinds:?}");
    assert!(kinds.contains(&"large_result"), "{kinds:?}");
    assert!(kinds.contains(&"empty_step"), "{kinds:?}");
    assert!(kinds.contains(&"failed"), "{kinds:?}");

    let report = analysis::stats(&trace);
    assert_eq!(report["calls"], 8);
    assert_eq!(report["calls_failed"], 2);
    assert_eq!(report["calls_cancelled"], 1);
    // git status x3 and cargo test x2 are the only identical repeats.
    assert_eq!(report["repeated"].as_array().unwrap().len(), 2);
}

#[test]
fn the_document_a_log_renders_reads_the_same_way() {
    let dir = tempfile::tempdir().expect("dir");
    let path = write_log(dir.path());
    let document = log::read(&path).expect("recording").document();
    let doc_path = dir.path().join("s-test.atif.json");
    std::fs::write(&doc_path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();

    let trace = analysis::read(&doc_path).expect("trace");
    assert_eq!(trace.kind, TraceKind::Document);
    assert_eq!(trace.calls.len(), 8);
    let kinds: Vec<&str> = analysis::anomalies(&trace).iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&"stuck"));
    assert!(kinds.contains(&"stuck_failure"));
    assert!(kinds.contains(&"cancelled"));
    assert!(kinds.contains(&"large_result"));
}

#[test]
fn an_event_stream_reads_its_calls_and_their_results() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("coder-0-events.jsonl");
    let mut lines = String::new();
    for i in 0..4u64 {
        lines.push_str(&format!(
            "{{\"type\":\"assistant\",\"t_ms\":{},\"message\":{{\"model\":\"m\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"t{i}\",\"name\":\"Read\",\"input\":{{\"file_path\":\"/r/same.rs\"}}}}]}}}}\n",
            1_000 + i * 500
        ));
        lines.push_str(&format!(
            "{{\"type\":\"user\",\"t_ms\":{},\"message\":{{\"content\":[{{\"type\":\"tool_result\",\"tool_use_id\":\"t{i}\",\"content\":\"ok\"}}]}}}}\n",
            1_000 + i * 500 + 100
        ));
    }
    std::fs::write(&path, lines).unwrap();

    let trace = analysis::read(&path).expect("trace");
    assert_eq!(trace.kind, TraceKind::Events);
    assert_eq!(trace.calls.len(), 4);
    assert!(
        trace
            .calls
            .iter()
            .all(|c| c.outcome == Some(Outcome::Completed))
    );

    let found = analysis::anomalies(&trace);
    assert!(
        found
            .iter()
            .any(|a| a.kind == "stuck" && a.detail.contains("4 times")),
        "{found:?}"
    );
}

#[test]
fn a_run_directory_rolls_up_its_summary_and_events() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(
        dir.path().join("summary.json"),
        serde_json::to_vec(&json!({
            "issue": 42,
            "outcome": {"status": "passed"},
            "wall_ms": 60_000,
            "cost": {"total_usd": 0.25},
            "solver": {"hands": [
                {"hand": "intaker", "round": 0, "wall_ms": 4000, "turns": 3,
                 "cost_usd": 0.001, "input_tokens": 4, "output_tokens": 283,
                 "verdict": "looks fine\nINTAKE: ready"},
                {"hand": "coder", "round": 0, "wall_ms": 40_000, "turns": 12,
                 "cost_usd": 0.20, "input_tokens": 100, "output_tokens": 4000,
                 "verdict": null},
            ]},
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("coder-0-events.jsonl"),
        "{\"type\":\"assistant\",\"t_ms\":5,\"message\":{\"model\":\"m\",\"content\":[{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"Edit\",\"input\":{\"file_path\":\"/r/b.rs\"}}]}}\n",
    )
    .unwrap();

    let run = analysis::read_dir(dir.path()).expect("run dir");
    assert!(run.summary.is_some());
    assert_eq!(run.traces.len(), 1);
    let text = analysis::run_dir_text(&run, dir.path());
    assert!(text.contains("issue #42"));
    assert!(text.contains("intaker"));
    assert!(text.contains("coder"));
}

#[test]
fn a_file_that_is_none_of_the_shapes_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "just some prose\nmore prose\n").unwrap();
    let error = analysis::read(&path).expect_err("not a trace");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
