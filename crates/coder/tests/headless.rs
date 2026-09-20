//! `coder --print` drives a turn from a script, and the trace it leaves is
//! the trace the terminal leaves.
//!
//! These tests run the built binary rather than calling into the library,
//! because the thing being tested is the binary's contract: what goes to
//! standard output, where the trace lands, and what the exit code says.
//! Every run is given a temporary trace directory and stripped of the
//! credentials a developer's shell may be carrying, so the door is the
//! stub and the turn is the same turn on every machine.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

/// The variables that would otherwise let one machine's environment decide
/// what these tests measure.
const CREDENTIALS: [&str; 7] = [
    "TYPESAFE_API_KEY",
    "CODER_DOOR_KEY",
    "CODER_AI_GATEWAY_KEY",
    "CODER_DOOR_URL",
    "CODER_MODEL",
    "CODER_WORKER",
    "CODER_RELAY",
];

/// Runs the binary with `arguments`, recording into `traces`.
fn coder(arguments: &[&str], traces: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_coder"));
    for name in CREDENTIALS {
        command.env_remove(name);
    }
    command
        .env_remove("CODER_TRACE")
        .env("CODER_TRACE_DIR", traces)
        .args(arguments)
        .output()
        .expect("the binary runs")
}

/// What the stub door answers with, which is what a turn with no
/// credentials produces.
fn stub_answer(text: &str) -> bool {
    text.contains("stub door")
}

/// A headless turn writes the reply to standard output, exits 0, and
/// leaves the trace where it was told to.
#[test]
fn a_headless_turn_prints_a_reply_and_records_it() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("one.atif.jsonl");
    let output = coder(
        &[
            "-p",
            "what crates are here",
            "--trace",
            &trace.display().to_string(),
        ],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let reply = String::from_utf8(output.stdout).unwrap();
    assert!(stub_answer(&reply), "{reply:?}");

    // The trace is read back, and it is the same record the terminal
    // writes: the person's turn, the note about the missing classifier,
    // the instructions, and the answer.
    let recording = atif::log::read(&trace).expect("the trace reads back");
    assert_eq!(recording.session.state, atif::log::ENDED);
    let document = recording.document();
    let steps = document["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4, "{document:#}");
    assert_eq!(steps[0]["source"], "user");
    assert_eq!(steps[0]["message"], "what crates are here");
    assert!(
        steps[1]["message"]
            .as_str()
            .unwrap()
            .contains("TYPESAFE_API_KEY")
    );
    assert_eq!(steps[2]["extra"]["kind"], "instructions");
    assert_eq!(steps[3]["source"], "agent");
    assert_eq!(steps[3]["message"], reply.trim_end());
    assert_eq!(document["extra"]["directive"], "what crates are here");
}

/// With `--json` the whole report is one object on standard output: the
/// reply, the trace it came from, and how the turn finished.
#[test]
fn a_headless_turn_reports_one_json_object() {
    let dir = tempfile::tempdir().unwrap();
    let output = coder(&["-p", "--json", "count the crates"], dir.path());

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).expect("one JSON object");
    assert!(stub_answer(report["reply"].as_str().unwrap()));
    assert_eq!(report["outcome"], "answered");
    assert_eq!(report["route"], "respond");
    assert!(report["error"].is_null());

    // The trace the report names is a trace, and it holds this turn.
    let trace = report["trace"].as_str().expect("a trace path");
    let recording = atif::log::read(Path::new(trace)).expect("the trace reads back");
    assert_eq!(recording.session.directive, "count the crates");
    assert_eq!(recording.session.state, atif::log::ENDED);
    assert_eq!(recording.document()["steps"][3]["message"], report["reply"]);
}

/// A prompt with newlines comes from a file, without shell quoting.
#[test]
fn a_prompt_reads_from_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("ask.txt");
    std::fs::write(&prompt, "count the crates\nthen name them\n").unwrap();
    let output = coder(
        &["--prompt-file", &prompt.display().to_string(), "--json"],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let trace = report["trace"].as_str().expect("a trace path");
    let recording = atif::log::read(Path::new(trace)).unwrap();
    assert_eq!(
        recording.document()["steps"][0]["message"],
        "count the crates\nthen name them\n"
    );
}

/// A named trace that cannot be opened ends the run. A caller that named a
/// file is going to read it back, and an unrecorded turn that exits 0
/// would read as a recorded one.
#[test]
fn a_named_trace_that_cannot_be_opened_ends_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("taken.atif.jsonl");
    std::fs::write(&trace, "").unwrap();
    let output = coder(
        &["-p", "hello", "--trace", &trace.display().to_string()],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot record"),
        "{output:?}"
    );
}

/// A wrong command line is its own kind of wrong, and its exit code says
/// so rather than passing for a turn that failed.
#[test]
fn a_wrong_command_line_exits_with_the_usage_code() {
    let dir = tempfile::tempdir().unwrap();
    let output = coder(&["-p"], dir.path());
    assert_eq!(output.status.code(), Some(64), "{output:?}");
    assert!(output.stdout.is_empty());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("--print needs a prompt"), "{said}");
    assert!(said.contains("Usage:"), "{said}");
}

/// `--help` says what the flags are and exits 0.
#[test]
fn help_says_what_the_flags_are() {
    let dir = tempfile::tempdir().unwrap();
    let output = coder(&["--help"], dir.path());
    assert_eq!(output.status.code(), Some(0));
    let said = String::from_utf8_lossy(&output.stdout);
    for flag in ["--print", "--prompt-file", "--trace", "--json"] {
        assert!(said.contains(flag), "{said}");
    }
}
