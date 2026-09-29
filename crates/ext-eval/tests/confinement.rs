//! Confinement: the child can't read a canary in the operator's real
//! `$HOME`, sees none of the operator's `XDG_*` or `CODER_*` variables,
//! never holds the door key, and the key appears nowhere under the
//! results directory.
//!
//! One test sets process environment, so this file holds only it.

#[path = "support/runner.rs"]
mod support;

use std::collections::BTreeSet;

use ext_eval::case::Grant;
use ext_eval::run::{self, Setup};
use ext_eval::signal::Cancel;
use support::*;

const HARNESS_CODER_VARS: [&str; 10] = [
    "CODER_TRACE_DIR",
    "CODER_DOOR_URL",
    "CODER_DOOR_KEY",
    "CODER_MODEL",
    "CODER_DELEGATE",
    "CODER_SHELL",
    "CODER_PROGRAMS",
    "CODER_PROGRAM_EFFECTS",
    "CODER_GUIDANCE",
    "CODER_GUIDANCE_DIGEST",
];

#[test]
fn the_child_sees_nothing_of_the_operator() {
    // The operator's shell carries a config home and its own door.
    // SAFETY: this is the only test in this binary; nothing else reads
    // the environment while it is set.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", "/operator/config");
        std::env::set_var("CODER_DOOR_KEY", "operator-own-door-key");
        std::env::set_var("CODER_OPERATOR_SETTING", "1");
    }
    let home = std::env::var("HOME").expect("a HOME");
    let canary =
        std::path::Path::new(&home).join(format!(".oa-eval-canary-{}", std::process::id()));
    std::fs::write(&canary, "operator secret canary").unwrap();

    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let extra = format!(
        "[run]\nallowed_operations = [\"read\", \"write\"]\nenv = {{ OA_EVAL_FAKE = \"env\", OA_EVAL_CANARY = \"{}\" }}\n",
        canary.display()
    );
    case(
        work.path(),
        "canary",
        &format!(
            "+++\nv = \"openagents.eval-case.v1\"\nruns = 1\n[run]\nenv = {{ OA_EVAL_FAKE = \"canary\", OA_EVAL_CANARY = \"{}\" }}\n+++\n\nSay hello.\n",
            canary.display()
        ),
        &[(
            "unreadable.md",
            "+++\ntype = \"regex\"\ntarget = \"last_message\"\n+++\n\ncanary: unreadable\n",
        )],
    );
    let suite = suite(work.path(), 1, &extra);
    let subject = subject();
    let agent = agent();
    let mut options = options(work.path());
    options.baseline = false;
    options.grants = BTreeSet::from([Grant::Read, Grant::Write]);
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: None,
        options: &options,
    };
    let outcome = run::run_suite(
        &setup,
        &author(),
        &work.path().join("evals/results"),
        None,
        &Cancel::new(),
        &|_| {},
    )
    .expect("the suite runs");
    let _ = std::fs::remove_file(&canary);

    // The canary in the real HOME was unreadable from inside the run.
    let canary_run = outcome
        .evaluation
        .runs
        .iter()
        .find(|run| run.case == "canary")
        .unwrap();
    assert_eq!(canary_run.passed, Some(true), "{canary_run:?}");

    // The environment the child saw.
    let dir = &outcome.results;
    let dump = std::fs::read_to_string(dir.join("runs/find-callers/subject-1/stderr.txt")).unwrap();
    assert!(dump.contains("HOME="), "{dump}");
    for line in dump.lines() {
        let key = line.split('=').next().unwrap_or_default();
        assert!(!key.starts_with("XDG_"), "{line}");
        if key.starts_with("CODER_") {
            assert!(HARNESS_CODER_VARS.contains(&key), "{line}");
        }
        assert!(!line.contains("operator-own-door-key"), "{line}");
    }
    assert!(dump.contains("CODER_DOOR_KEY=[redacted]"), "{dump}");
    let home_line = dump.lines().find(|line| line.starts_with("HOME=")).unwrap();
    assert!(home_line.contains("oa-eval-"), "{home_line}");

    // The file the child wrote in its workspace is kept, scrubbed.
    let written =
        std::fs::read_to_string(dir.join("runs/find-callers/subject-1/files/env.txt")).unwrap();
    assert!(written.contains("CODER_DOOR_KEY=[redacted]"), "{written}");

    // The real door key appears nowhere under the results directory.
    for file in files(dir) {
        let bytes = std::fs::read(&file).unwrap();
        assert!(
            !bytes
                .windows(door.key.len())
                .any(|window| window == door.key.as_bytes()),
            "{} holds the door key",
            file.display()
        );
    }
}
