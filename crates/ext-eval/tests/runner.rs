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

/// Whether the subject arm's callers run reached the extension's
/// program (its `repo_map` step), with `decision` as the child's decision
/// door.
#[cfg(unix)]
fn subject_reached_the_program(decision: &run::DecisionPin) -> bool {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let suite = suite(work.path(), 1, "");
    let subject = subject();
    let agent = agent();
    let options = options(work.path());
    let door_pin = door.door();
    let setup = Setup {
        suite: &suite,
        subject: &subject,
        agent: &agent,
        door: &door_pin,
        decision: Some(decision),
        options: &options,
    };
    let outcome = run::run_suite(
        &setup,
        &author(),
        &work.path().join("evals/results"),
        None,
        &Cancel::new(),
        &quiet,
    )
    .expect("the suite runs");
    let run = outcome
        .evaluation
        .runs
        .iter()
        .find(|run| run.arm == Arm::Subject && run.case == "find-callers")
        .expect("the subject arm ran the callers case");
    run.graders
        .iter()
        .find(|grader| grader.name == "used-map")
        .expect("the used-map grader ran")
        .passed
}

/// #10122: TypeSafe's account ran out of credits, the child's classifier
/// failed, and the subject arm never ran its program, so it scored what
/// the baseline did. With Jev's gateway door on the pin, a decision asks
/// the gateway first (under Jev's gateway name) and the subject arm gets
/// its extension again.
#[test]
#[cfg(unix)]
fn the_subject_arm_gets_its_program_when_typesafe_is_out_of_credits() {
    let typesafe = fake_jev("/v1/systemone", 402);
    let gateway = fake_jev("/typesafe/v1/systemone", 200);
    let alone = run::DecisionPin::new(
        typesafe.url.clone(),
        ext_eval::proxy::Secret::new(typesafe.key.clone()),
    );
    // TypeSafe alone: the program never runs.
    assert!(!subject_reached_the_program(&alone));
    let mut with_gateway = alone.clone();
    with_gateway.fallbacks.push(jev::doors::Door::new(
        gateway.url.clone(),
        format!("{}/typesafe/v1/systemone", gateway.url),
        jev::doors::Naming::Gateway,
        jev::ApiKey::new(gateway.key.clone()),
    ));
    assert!(subject_reached_the_program(&with_gateway));
    let models = gateway.models.lock().unwrap().clone();
    assert!(!models.is_empty(), "the gateway was asked");
    assert!(
        models.iter().all(|model| model == "typesafe-ai/jev"),
        "{models:?}"
    );
}

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

/// A files test with the fake Coder: the run starts in a Git repository
/// holding the template and the case's fixtures, Coder may write it and
/// run commands without `--grant`, the checks read the files, the
/// changes, the diff, and commands' exit codes, the results keep the
/// diff, and the scratch folder is gone afterwards. The subject arm (the
/// skill on) adds the changelog; the baseline doesn't, and fails.
#[test]
#[cfg(unix)]
fn a_files_test_checks_the_files_coder_changed() {
    let door = fake_door();
    let work = tempfile::tempdir().unwrap();
    let temp = work.path().join("tmp");
    let cargo = !ext_eval::sandbox::toolchains().is_empty();
    let mut graders = vec![
        (
            "made.md",
            "+++\ntype = \"file_exists\"\npath = \"CHANGELOG.md\"\n+++\n".to_string(),
        ),
        (
            "entry.md",
            "+++\ntype = \"regex\"\ntarget = { file = \"CHANGELOG.md\" }\n+++\n\n## Unreleased\n"
                .to_string(),
        ),
        (
            "diff.md",
            "+++\ntype = \"regex\"\ntarget = \"diff\"\n+++\n\n\\+See CHANGELOG\\.md\n".to_string(),
        ),
        (
            "changed.md",
            "+++\ntype = \"regex\"\ntarget = \"changed\"\n+++\n\nmodified README\\.md\n"
                .to_string(),
        ),
        (
            "listed.md",
            "+++\ntype = \"command\"\n+++\n\ngrep -q Unreleased CHANGELOG.md && git rev-parse --verify -q HEAD\n"
                .to_string(),
        ),
    ];
    if cargo {
        graders.push((
            "builds.md",
            "+++\ntype = \"command\"\n+++\n\ncargo test --offline --quiet\n".to_string(),
        ));
    }
    let graders: Vec<(&str, &str)> = graders.iter().map(|(n, t)| (*n, t.as_str())).collect();
    case(
        work.path(),
        "changelog",
        "+++\nv = \"openagents.eval-case.v1\"\nruns = 1\nworkspace = \"rust-crate\"\n\n[run]\nenv = { OA_EVAL_FAKE = \"files\" }\n+++\n\nAdd a greeting and record it.\n",
        &graders,
    );
    let fixture = work.path().join("evals/changelog/fixtures/README.md");
    std::fs::create_dir_all(fixture.parent().unwrap()).unwrap();
    std::fs::write(&fixture, "# Fixture\n").unwrap();
    let suite = ext_eval::Suite::load(&work.path().join("evals"), ext_eval::LoadOptions::default())
        .unwrap();
    let subject = subject();
    let agent = agent();
    let door_pin = door.door();
    let mut options = options(&temp);
    options.grants = BTreeSet::from([Grant::Read]);
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
        &quiet,
    )
    .expect("the suite runs");
    let runs = &outcome.evaluation.runs;
    let subject_run = runs.iter().find(|r| r.arm == Arm::Subject).unwrap();
    let baseline_run = runs.iter().find(|r| r.arm == Arm::Baseline).unwrap();
    assert_eq!(subject_run.passed, Some(true), "{:#?}", subject_run.graders);
    assert_eq!(
        baseline_run.passed,
        Some(false),
        "{:#?}",
        baseline_run.graders
    );
    assert_eq!(subject_run.created_files, vec!["CHANGELOG.md".to_string()]);
    assert_eq!(
        subject_run.changed_files,
        vec![
            "added CHANGELOG.md".to_string(),
            "modified README.md".to_string()
        ]
    );
    assert!(baseline_run.changed_files.is_empty());
    let failed: Vec<&str> = baseline_run
        .graders
        .iter()
        .filter(|g| !g.passed)
        .map(|g| g.name.as_str())
        .collect();
    assert!(
        failed.contains(&"made") && failed.contains(&"listed"),
        "{failed:?}"
    );
    if cargo {
        let builds = baseline_run
            .graders
            .iter()
            .find(|g| g.name == "builds")
            .unwrap();
        assert!(
            builds.passed,
            "the template's own test passes: {}",
            builds.explanation
        );
    }
    let listed = subject_run
        .graders
        .iter()
        .find(|g| g.name == "listed")
        .unwrap();
    assert!(
        listed.explanation.contains("exited 0"),
        "{}",
        listed.explanation
    );
    let run_dir = outcome.results.join("runs/changelog/subject-1");
    let diff = std::fs::read_to_string(run_dir.join("diff.patch")).unwrap();
    assert!(diff.contains("+++ b/CHANGELOG.md"), "{diff}");
    assert!(diff.contains("+See CHANGELOG.md"), "{diff}");
    assert!(run_dir.join("changed.json").is_file());
    assert!(run_dir.join("commands.json").is_file());
    assert_eq!(outcome.evaluation.verdict, ext_eval::Verdict::Inconclusive);
    // Cleaned up: no scratch folder is left.
    let left: Vec<_> = std::fs::read_dir(&temp)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("oa-eval-"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}
