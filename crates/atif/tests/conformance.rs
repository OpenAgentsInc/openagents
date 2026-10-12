//! The conformance corpus (`tests/conformance/*.json`) pins what the log
//! and document do: the expectations in each case were recorded from this
//! implementation and replayed against it, so a change in read, write, or
//! render behavior fails on a case that says so.
//!
//! A read case carries the log's own bytes — `log` as one string per line,
//! or `log_hex` when a line is not UTF-8 — and the expected recording:
//! every fault at its line, `whole`, `state`, `seconds`,
//! `schema_version`, `trajectory_id`, `unreadable_lines`, `step_count`,
//! `steps_digest`, and the rendered document minus the volatile
//! `exported_at` and `source_path`.
//!
//! A write case carries a `Session` and `Step`s under `write`, runs them
//! through `Log::create_at`/`append`/`finish` (no `finish` is an
//! interrupted session), and expects what the log reads back. Wall seconds
//! is the writer's own clock, so a write case records no `seconds` and its
//! document is compared without `final_metrics.extra.wall_seconds`.
//!
//! `ATIF_CONFORMANCE=write` rewrites every case's `expect` from the
//! implementation, like `NIP_TERM_FIXTURES=write` and
//! `ROUTER_SUITE_WRITE`. A recorded diff is a behavior change: review it
//! before it lands.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance")
}

fn cases() -> Vec<(String, PathBuf, Value)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir())
        .expect("the conformance directory reads")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            let text = std::fs::read_to_string(&path).expect("a case reads");
            (
                name,
                path,
                serde_json::from_str(&text).expect("a case is JSON"),
            )
        })
        .collect()
}

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        text.len() % 2 == 0 && text.bytes().all(|b| b.is_ascii_hexdigit()),
        "log_hex is even-length hex"
    );
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The case's log bytes: `log_hex` when present, else the `log` lines
/// joined with newlines plus one more unless `trailing_newline` is false.
fn log_bytes(case: &Value) -> Vec<u8> {
    if let Some(text) = case.get("log_hex").and_then(Value::as_str) {
        return hex(text);
    }
    let mut bytes = case["log"]
        .as_array()
        .expect("a read case carries log or log_hex")
        .iter()
        .map(|line| line.as_str().expect("a log line is a string").to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    if case
        .get("trailing_newline")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        bytes.push(b'\n');
    }
    bytes
}

/// The recording as the corpus spells it: everything the corpus asserts
/// and nothing it does not. `volatile_seconds` drops the wall-time fields
/// the writer stamps with its own clock.
fn observed(path: &Path, volatile_seconds: bool) -> Value {
    let recording = match atif::log::read(path) {
        Err(_) => return json!({"read_error": true}),
        Ok(recording) => recording,
    };
    let mut document = recording.document();
    if let Some(extra) = document.get_mut("extra").and_then(Value::as_object_mut) {
        extra.remove("exported_at");
        extra.remove("source_path");
    }
    if volatile_seconds {
        if let Some(metrics) = document
            .pointer_mut("/final_metrics/extra")
            .and_then(Value::as_object_mut)
        {
            metrics.remove("wall_seconds");
        }
    }
    let whole_ok = atif::log::read_whole(path).is_ok();
    let mut expect = Map::new();
    expect.insert("read_error".to_string(), json!(false));
    expect.insert("whole".to_string(), json!(recording.whole()));
    expect.insert("whole_ok".to_string(), json!(whole_ok));
    expect.insert("state".to_string(), json!(recording.session.state));
    if !volatile_seconds {
        expect.insert("seconds".to_string(), json!(recording.session.seconds));
    }
    expect.insert(
        "schema_version".to_string(),
        json!(recording.schema_version),
    );
    expect.insert(
        "trajectory_id".to_string(),
        document["trajectory_id"].clone(),
    );
    expect.insert(
        "unreadable_lines".to_string(),
        json!(recording.unreadable_lines),
    );
    expect.insert(
        "faults".to_string(),
        Value::Array(
            recording
                .faults
                .iter()
                .map(|fault| json!({"line": fault.line, "kind": fault.kind.word()}))
                .collect(),
        ),
    );
    expect.insert("step_count".to_string(), json!(recording.steps.len()));
    expect.insert(
        "steps_digest".to_string(),
        json!(atif::digest(&document["steps"])),
    );
    expect.insert("document".to_string(), document);
    Value::Object(expect)
}

/// A write case run for real: the log lands in a tempdir and reads back.
fn written(case: &Value, name: &str) -> Value {
    let write = &case["write"];
    let session: atif::Session =
        serde_json::from_value(write["session"].clone()).expect("write.session is a session");
    let steps: Vec<atif::Step> =
        serde_json::from_value(write["steps"].clone()).expect("write.steps are steps");
    let dir = tempfile::tempdir().expect("a tempdir");
    let path = dir.path().join(format!("{name}.{}", atif::log::EXTENSION));
    let mut log = atif::Log::create_at(&path, &session).expect("the log opens");
    for step in &steps {
        log.append(step).expect("a step appends");
    }
    if let Some(state) = write.get("end").and_then(Value::as_str) {
        log.finish(state).expect("the log finishes");
    }
    drop(log);
    observed(&path, true)
}

fn run(case: &Value, name: &str) -> Value {
    if case.get("write").is_some() {
        return written(case, name);
    }
    let dir = tempfile::tempdir().expect("a tempdir");
    let path = dir.path().join(format!("{name}.{}", atif::log::EXTENSION));
    std::fs::write(&path, log_bytes(case)).expect("the case log lands");
    observed(&path, false)
}

/// The case with its `expect` rewritten by the run; every author field
/// keeps its place and `expect` comes last.
fn recorded(case: &Value, expect: Value) -> Value {
    let mut out = Map::new();
    for key in ["v", "about", "log", "log_hex", "trailing_newline", "write"] {
        if let Some(value) = case.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    out.insert("expect".to_string(), expect);
    Value::Object(out)
}

#[test]
fn every_conformance_case_replays() {
    let write = std::env::var_os("ATIF_CONFORMANCE").is_some_and(|v| v == "write");
    let mut failures = Vec::new();
    for (name, path, case) in cases() {
        let expect = run(&case, &name);
        if write {
            let mut text =
                serde_json::to_string_pretty(&recorded(&case, expect)).expect("serializes");
            text.push('\n');
            std::fs::write(&path, text).expect("records the case");
            continue;
        }
        if expect != case["expect"] {
            failures.push(format!(
                "{name}\nexpected: {}\nrecorded: {}",
                serde_json::to_string_pretty(&case["expect"]).unwrap(),
                serde_json::to_string_pretty(&expect).unwrap(),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} conformance case(s) differ; regenerate with ATIF_CONFORMANCE=write after review\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// The corpus asserts coverage as data: every fault kind and both end
/// states appear in at least one recorded expectation, so a case cannot be
/// dropped without the suite saying so.
#[test]
fn the_corpus_covers_faults_and_states() {
    let mut kinds = std::collections::BTreeSet::new();
    let mut states = std::collections::BTreeSet::new();
    let mut reads = 0;
    let mut writes = 0;
    for (_name, _path, case) in cases() {
        let expect = &case["expect"];
        if let Some(faults) = expect["faults"].as_array() {
            for fault in faults {
                kinds.insert(fault["kind"].as_str().unwrap().to_string());
            }
        }
        if let Some(state) = expect.get("state").and_then(Value::as_str) {
            states.insert(state.to_string());
        }
        match case.get("write") {
            Some(_) => writes += 1,
            None => reads += 1,
        }
    }
    let all = [
        "torn",
        "not_utf8",
        "not_json",
        "unknown_record",
        "bad_session",
        "bad_step",
        "repeated_session",
        "repeated_end",
        "after_end",
        "before_session",
        "unsupported_version",
    ];
    for kind in all {
        assert!(kinds.contains(kind), "the corpus has no {kind} case");
    }
    for state in ["ended", "interrupted"] {
        assert!(states.contains(state), "the corpus has no {state} case");
    }
    assert!(
        reads >= 10 && writes >= 2,
        "the corpus thinned to {reads} reads and {writes} writes"
    );
}
