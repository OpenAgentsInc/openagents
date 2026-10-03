//! Retrospective diagnostic for trace integrity, separate from the frozen gate.
//! This source has not been compiled or run. It uses only public ATIF and
//! CoderBench APIs available at the task's pinned source.
use atif::{Log, Session, Source, Step, log};
use coderbench::{Ending, Task, Verdict, Workspace};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn clean_records(path: &Path) -> Vec<Vec<u8>> {
    let session = Session::opening(
        "posthoc-integrity",
        "fixture",
        "test",
        "/workspace",
        "fixture",
    );
    let mut writer = Log::create_at(path, &session).unwrap();
    writer
        .append(&Step::said(Source::User, "read without writing"))
        .unwrap();
    writer
        .append(&Step::said(Source::Agent, "reported answer"))
        .unwrap();
    writer.finish(log::ENDED).unwrap();
    let records: Vec<Vec<u8>> = fs::read(path)
        .unwrap()
        .split_inclusive(|byte| *byte == b'\n')
        .map(Vec::from)
        .collect();
    assert_eq!(records.len(), 4);
    for (record, kind) in records.iter().zip(["session", "step", "step", "end"]) {
        let value: Value = serde_json::from_slice(record).unwrap();
        assert_eq!(value["record"], kind);
    }
    records
}

fn grade(path: &Path) -> Result<Verdict, String> {
    let task: Task = serde_json::from_value(json!({
        "schema": coderbench::TASK_SCHEMA,
        "id": "posthoc-integrity",
        "family": "trace",
        "request": "Read without writing",
        "requires": {},
        "grade": {"kind": "trace", "writes_expected": 0},
        "timeout_secs": 1
    }))
    .unwrap();
    let mut observed = coderbench::observe(path)?;
    // These represent the independent observations a driver supplies. They do
    // not replace the trace reader's integrity or lifecycle observations.
    observed.ending = Ending::Answered;
    observed.workspace = Some(Workspace::default());
    Ok(task.judge(&observed).verdict)
}

fn assert_clean(path: &Path) {
    let recording = log::read(path).unwrap();
    assert!(recording.ended());
    assert_eq!(recording.unreadable_lines, 0);
    assert_eq!(recording.steps.len(), 2);
    assert_eq!(recording.steps[0].message, "read without writing");
    assert_eq!(recording.steps[1].message, "reported answer");
    assert_eq!(grade(path).unwrap(), Verdict::Passed);
}

fn assert_not_successful_evidence(path: &Path) {
    // A strict consumer can reject at ingestion or produce a non-Passed grade.
    // The diagnostic does not require a particular new field, error, or enum.
    if let Ok(verdict) = grade(path) {
        assert_ne!(verdict, Verdict::Passed);
    }
}

#[test]
fn clean_trace_remains_successful_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("clean.jsonl");
    clean_records(&path);
    assert_clean(&path);
}

#[test]
fn step_before_session_is_not_successful_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let clean = directory.path().join("clean.jsonl");
    let records = clean_records(&clean);
    assert_clean(&clean);
    // Move only the first step ahead of its header. Its contents and its
    // order relative to the other step are unchanged.
    let disordered = [
        records[1].as_slice(),
        records[0].as_slice(),
        records[2].as_slice(),
        records[3].as_slice(),
    ]
    .concat();
    let path = directory.path().join("step-before-session.jsonl");
    fs::write(&path, disordered).unwrap();
    assert_not_successful_evidence(&path);
}

#[test]
fn form_feed_before_an_interior_record_is_not_successful_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let clean = directory.path().join("clean.jsonl");
    let records = clean_records(&clean);
    assert_clean(&clean);
    let damaged_record = [b"\x0c".as_slice(), records[1].as_slice()].concat();
    // Form feed is ASCII whitespace but is not JSON whitespace. This is a
    // malformed interior JSON line, followed by another valid step and end.
    assert!(serde_json::from_slice::<Value>(&damaged_record).is_err());
    let damaged = [
        records[0].as_slice(),
        damaged_record.as_slice(),
        records[2].as_slice(),
        records[3].as_slice(),
    ]
    .concat();
    let path = directory.path().join("form-feed-prefix.jsonl");
    fs::write(&path, damaged).unwrap();
    assert_not_successful_evidence(&path);
}
