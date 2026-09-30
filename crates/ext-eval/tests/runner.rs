//! The runner end to end with the fake agent and a fake door: both arms
//! run confined, the report is written, the gate decides, and the result
//! is the same across two runs. Stopping, deadlines, refusals, and grants
//! are covered here too.

// Off Unix only the refusal case runs, so most helpers go unused.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

#[path = "support/runner.rs"]
mod support;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use ext_eval::case::{Grant, RunFailure};
use ext_eval::run::{self, Progress, Setup};
use ext_eval::signal::Cancel;
use ext_eval::{Arm, RunOutcome, Verdict};
use serde_json::Value;
use support::*;

fn quiet(_: Progress) {}

#[test]
// The run sandbox is Unix's; elsewhere every run refuses as unconfined.
#[cfg(unix)]
fn both_arms_run_and_the_gate_says_better_twice() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let suite = suite(work.path(), 3, "");
    let subject = subject();
    let agent = agent();
    let options = options(work.path());
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: None,
        options: &options,
    };
    let mut verdicts = Vec::new();
    for _ in 0..2 {
        let outcome = run::run_suite(
            &setup,
            &author(),
            &work.path().join("evals/results"),
            None,
            &Cancel::new(),
            &quiet,
        )
        .expect("the suite runs");
        let dir = &outcome.results;
        for file in [
            "report.json",
            "report.html",
            "run.json",
            "artifacts/suite.json",
        ] {
            assert!(dir.join(file).is_file(), "{file} missing");
        }
        let report = std::fs::read(dir.join("report.json")).unwrap();
        let parsed = nostr::eval_ext::parse_report(&report).expect("a profile report");
        assert_eq!(parsed.profile.headline.subject_passed, 2);
        assert_eq!(parsed.profile.headline.baseline_passed, Some(1));
        let run_dir = dir.join("runs/find-callers/subject-1");
        assert!(run_dir.join("trajectory.json").is_file());
        assert!(run_dir.join("stdout.jsonl").is_file());
        assert!(dir.join("suite/find-callers/prompt.md").is_file());
        // The subject arm reached the extension; the baseline didn't.
        let subject_run = &outcome.evaluation.runs[0];
        assert_eq!(subject_run.arm, Arm::Subject);
        assert!(subject_run.passed.unwrap_or(false), "{subject_run:?}");
        assert_eq!(outcome.evaluation.verdict, Verdict::Pass);
        assert_eq!(outcome.exit_code, 0);
        let value: Value = serde_json::from_slice(&report).unwrap();
        verdicts.push((
            value["verdict"].clone(),
            value["measurements"].clone(),
            value["coverage"].clone(),
        ));
    }
    let strip = |measurements: &Value| -> Vec<Value> {
        measurements
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| {
                !m["metric"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("seconds")
            })
            .cloned()
            .map(|mut m| {
                m.as_object_mut().unwrap().remove("evidence");
                m
            })
            .collect()
    };
    assert_eq!(verdicts[0].0, verdicts[1].0, "the verdict repeats");
    assert_eq!(verdicts[0].2, verdicts[1].2, "the coverage repeats");
    assert_eq!(
        strip(&verdicts[0].1),
        strip(&verdicts[1].1),
        "the scores repeat"
    );
    assert!(door.requests.load(std::sync::atomic::Ordering::SeqCst) >= 12);
}

#[test]
fn an_unconfined_host_refuses_every_run_and_spawns_nothing() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let suite = suite(work.path(), 1, "");
    let subject = subject();
    let agent = agent();
    let mut options = options(work.path());
    options.backend = Some(work.path().join("no-such-sandbox-exec"));
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: None,
        options: &options,
    };
    let runs = run::execute(&setup, &Cancel::new(), &quiet).unwrap();
    assert_eq!(runs.made.len(), 4);
    for made in &runs.made {
        assert_eq!(
            made.record.outcome,
            RunOutcome::Errored(RunFailure::UnconfinedHost)
        );
        assert!(made.sandbox.is_none(), "no run directory was made");
    }
    assert_eq!(door.requests.load(std::sync::atomic::Ordering::SeqCst), 0);
    let evaluation = run::evaluate_runs(&setup, &author(), &runs, None).unwrap();
    let coverage = &evaluation.report["coverage"]["subject"];
    assert_eq!(coverage["refused"], 2);
    assert_eq!(coverage["completed"], 0);
}

#[test]
// The run sandbox is Unix's; elsewhere every run refuses as unconfined.
#[cfg(unix)]
fn stopping_a_run_stops_its_child_and_marks_it_cancelled() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let extra = "[run]\nenv = { OA_EVAL_FAKE = \"sleep\" }\n";
    let suite = suite(work.path(), 1, extra);
    let subject = subject();
    let agent = agent();
    let mut options = options(work.path());
    options.concurrency = 1;
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: None,
        options: &options,
    };
    let cancel = Cancel::new();
    let started = std::sync::Mutex::new(Vec::<Instant>::new());
    let stopper = cancel.clone();
    let progress = |event: Progress| {
        if matches!(event, Progress::Started { .. }) {
            started.lock().unwrap().push(Instant::now());
            let stopper = stopper.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                stopper.cancel();
            });
        }
    };
    let began = Instant::now();
    let runs = run::execute(&setup, &cancel, &progress).unwrap();
    assert!(runs.cancelled);
    assert!(
        began.elapsed() < Duration::from_secs(20),
        "the sleeping child was stopped, not waited out"
    );
    let first = &runs.made[0].record;
    assert_eq!(first.outcome, RunOutcome::Cancelled);
    let stdout = String::from_utf8_lossy(&runs.made[0].outputs["stdout.jsonl"]).to_string();
    let pid: i32 = serde_json::from_str::<Value>(stdout.lines().next().unwrap()).unwrap()["pid"]
        .as_i64()
        .unwrap()
        .try_into()
        .unwrap();
    // SAFETY: signal 0 only asks whether the process exists.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    assert!(!alive, "the child {pid} is gone");
    for made in &runs.made[1..] {
        assert_eq!(made.record.outcome, RunOutcome::Cancelled);
    }
    assert_eq!(
        started.lock().unwrap().len(),
        1,
        "nothing started after stop"
    );
    let evaluation = run::evaluate_runs(&setup, &author(), &runs, None).unwrap();
    assert!(evaluation.partial.is_some());
    assert_eq!(evaluation.exit_code(), 2);
}

#[test]
// The run sandbox is Unix's; elsewhere every run refuses as unconfined.
#[cfg(unix)]
fn a_deadline_stops_the_child_as_a_timeout() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let extra = "[run]\ndeadline_seconds = 1\nenv = { OA_EVAL_FAKE = \"sleep\" }\n";
    let suite = suite(work.path(), 1, extra);
    let subject = subject();
    let agent = agent();
    let mut options = options(work.path());
    options.baseline = false;
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: None,
        options: &options,
    };
    let runs = run::execute(&setup, &Cancel::new(), &quiet).unwrap();
    let callers = runs
        .made
        .iter()
        .find(|made| made.record.case == "find-callers")
        .unwrap();
    assert_eq!(
        callers.record.outcome,
        RunOutcome::Errored(RunFailure::Timeout)
    );
    assert!(callers.record.seconds.unwrap() < 10.0);
}

#[test]
// The run sandbox is Unix's; elsewhere every run refuses as unconfined.
#[cfg(unix)]
fn an_env_key_outside_oa_eval_is_rejected_before_spawning() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let suite = suite(work.path(), 1, "[run]\nenv = { PATH = \"/tmp\" }\n");
    let subject = subject();
    let agent = agent();
    let mut options = options(work.path());
    options.baseline = false;
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: None,
        options: &options,
    };
    let runs = run::execute(&setup, &Cancel::new(), &quiet).unwrap();
    let callers = runs
        .made
        .iter()
        .find(|made| made.record.case == "find-callers")
        .unwrap();
    assert_eq!(
        callers.record.outcome,
        RunOutcome::Errored(RunFailure::EnvVarRejected)
    );
    assert!(callers.sandbox.is_none());
}

#[test]
// The run sandbox is Unix's; elsewhere every run refuses as unconfined.
#[cfg(unix)]
fn the_workspace_is_writable_only_with_the_write_grant() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let extra =
        "[run]\nallowed_operations = [\"read\", \"write\"]\nenv = { OA_EVAL_FAKE = \"write\" }\n";
    let suite = suite(work.path(), 1, extra);
    let subject = subject();
    let agent = agent();
    let door_pin = door.door();
    for (grants, expect) in [
        (BTreeSet::from([Grant::Read]), false),
        (BTreeSet::from([Grant::Read, Grant::Write]), true),
    ] {
        let mut options = options(work.path());
        options.baseline = false;
        options.grants = grants;
        let setup = Setup {
            suite: &suite,
            subject: &subject,
            agent: &agent,
            door: &door_pin,
            decision: None,
            options: &options,
        };
        let runs = run::execute(&setup, &Cancel::new(), &quiet).unwrap();
        let callers = runs
            .made
            .iter()
            .find(|made| made.record.case == "find-callers")
            .unwrap();
        assert_eq!(
            callers
                .record
                .created_files
                .contains(&"summary.md".to_string()),
            expect,
            "{:?}",
            callers.record
        );
    }
}
