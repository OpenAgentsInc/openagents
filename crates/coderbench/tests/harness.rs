//! `coderbench run` drives a turn, reads back what it left, and judges it;
//! `coderbench diff` judges a trace somebody already has.
//!
//! These tests run the built binary rather than calling into the library,
//! because the contract being tested is the binary's: what it refuses to
//! start, what it prints, and what it exits with.
//!
//! Coder stands in as a script here. What is under test is the harness —
//! whether it refuses before starting anything, whether it finds the trace
//! it named, whether the fault list reads in path order, and whether a run
//! that timed out or wrote a file can still exit clean — and a real agent
//! would make every one of those answers slower and none of them clearer.
//!
//! Every run names its own temporary checkout with `--repository`. The
//! harness reads the checkout before and after to see what the run wrote,
//! and pointing that at the workspace this test runs in would read whatever
//! else was happening on the machine.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::authored_text;

/// Runs the harness.
fn coderbench(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_coderbench"))
        .args(arguments)
        .output()
        .expect("the binary runs")
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A stand-in for Coder: it copies `trace` into the file the harness named
/// and reports one JSON object, which is what `coder -p --json` does.
fn fake_coder(directory: &Path, trace: &Path, exit: u8) -> PathBuf {
    fake_coder_that(directory, trace, exit, "")
}

/// The same stand-in with one more line of shell after the copy, for a run
/// that takes too long or writes where it should not.
fn fake_coder_that(directory: &Path, trace: &Path, exit: u8, then: &str) -> PathBuf {
    fake_coder_script(
        directory,
        &format!(
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    --trace) named=\"$2\"; \
             shift 2;;\n    *) shift;;\n  esac\ndone\ncp '{}' \"$named\"\n{then}\necho \
             '{{\"reply\":\"done\",\"outcome\":\"answered\"}}'\nexit {exit}\n",
            trace.display()
        ),
    )
}

fn fake_coder_script(directory: &Path, contents: &str) -> PathBuf {
    let script = directory.join("fake-coder");
    std::fs::write(&script, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    script
}

/// A task with no requirements, so a run of it turns on nothing but the
/// harness.
fn task_with(directory: &Path, requires: &str, id: &str) -> PathBuf {
    let path = directory.join(format!("{id}.json"));
    let expects = serde_json::to_string(&common::task().grade.expects).unwrap();
    std::fs::write(
        &path,
        format!(
            r#"{{
  "schema": "openagents.coderbench.task.v1",
  "id": "{id}",
  "family": "delegation",
  "request": "Delegate six instances of Devin, one for each of these six read-only questions.",
  "requires": {requires},
  "grade": {{
    "kind": "path",
    "program": "delegate-fan-out",
    "delegations": 6,
    "delegations_correct": 6,
    "expects": {expects},
    "writes_expected": 0,
    "decisions": ["program", "independence"],
    "checks": ["admission_check", "capability_probe", "program_registry"],
    "path": ["capability_probe", "program_registry", "program", "independence",
             "admission_check", "delegate"],
    "endings": ["answered"]
  }},
  "timeout_secs": 60
}}
"#
        ),
    )
    .unwrap();
    path
}

fn golden() -> PathBuf {
    coderbench::goldens_dir().join("devin-fan-out-six.atif.jsonl")
}

/// A trace of a run that answered and took none of the path: the session,
/// the operator's sentence, a reply.
fn empty_handed(directory: &Path) -> PathBuf {
    let path = directory.join("empty-handed.atif.jsonl");
    std::fs::write(
        &path,
        concat!(
            r#"{"record":"session","schema_version":"ATIF-v1.7","at":1789869697018,"#,
            r#""session":{"id":"s","model":"stub","door":"stub","repository":"/tmp","#,
            r#""directive":"Delegate six instances of Devin.","version":"coder/0.1.0"}}"#,
            "\n",
            r#"{"record":"step","step":{"at":1789869697018,"source":"User","#,
            r#""message":"Delegate six instances of Devin."}}"#,
            "\n",
            r#"{"record":"step","step":{"at":1789869697118,"source":"Agent","#,
            r#""message":"I can help with that."}}"#,
            "\n",
            r#"{"record":"end","at":1789869697218,"state":"ended"}"#,
            "\n",
        ),
    )
    .unwrap();
    path
}

/// A checkout with one commit, so the base check has something true to
/// compare against and the workspace reading has a checkout to read.
fn checkout(directory: &Path) -> String {
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            said(&output)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "harness@example.com"]);
    git(&["config", "user.name", "harness"]);
    std::fs::write(directory.join("README.md"), "one\n").unwrap();
    git(&["add", "README.md"]);
    git(&["commit", "--quiet", "-m", "one"]);
    git(&["rev-parse", "HEAD"])
}

/// A directory holding a fresh checkout, which is what a run needs before
/// it can say anything about what the run wrote.
fn repository(directory: &Path) -> PathBuf {
    let repository = directory.join("checkout");
    std::fs::create_dir(&repository).unwrap();
    checkout(&repository);
    repository
}

/// A run drives Coder, reads back the trace it named, and says the run took
/// the path when it did.
#[test]
fn a_run_judges_the_trace_it_captured() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(directory.path(), r#"{}"#, "clean");
    let coder = fake_coder(directory.path(), &golden(), 0);
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository(directory.path()).display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert!(report.contains("No faults"), "{report}");
    assert!(
        report.contains("6 started, 6 verified against the task's expected answers"),
        "{report}"
    );
    assert!(report.contains("the workspace is unchanged"), "{report}");
    assert!(trace.exists(), "the harness reads the file it named");
}

/// A run of a task that owns its answers is judged against the manifest's
/// copy rather than the run's claims: an authored trace in the shape a
/// sentence-driven run is expected to record passes with no `correct`
/// flag anywhere in it.
#[test]
fn a_run_is_judged_against_the_manifests_own_answers() {
    let directory = tempfile::tempdir().unwrap();
    // The task is the real manifest with the machine requirements blanked
    // out — the temporary checkout cannot hold them — so a run of it turns
    // on the harness and the delegation checks alone.
    let mut manifest = serde_json::to_value(
        coderbench::load_task("devin-fan-out-six").expect("the real manifest loads"),
    )
    .unwrap();
    manifest["requires"] = serde_json::json!({});
    let task = directory.path().join("expects.json");
    std::fs::write(&task, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let authored = directory.path().join("authored.atif.jsonl");
    std::fs::write(&authored, authored_text()).unwrap();
    let coder = fake_coder(directory.path(), &authored, 0);
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository(directory.path()).display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert!(report.contains("No faults"), "{report}");
    assert!(
        report.contains("6 started, 6 verified against the task's expected answers"),
        "{report}"
    );
}

/// A run that took none of the path faults on every step of it, and the
/// faults read in the order the path takes rather than the order the
/// checker tests them.
#[test]
fn faults_read_in_the_order_the_path_takes() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(directory.path(), r#"{}"#, "empty");
    let nothing = empty_handed(directory.path());
    let coder = fake_coder(directory.path(), &nothing, 0);
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository(directory.path()).display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(1), "{report}");
    let order: Vec<usize> = [
        "never ran the capability_probe check",
        "never ran the program_registry check",
        "selected program none",
        "never asked the independence decision",
        "never ran the admission_check check",
        "started 0 delegations",
    ]
    .iter()
    .map(|fault| {
        report
            .find(fault)
            .unwrap_or_else(|| panic!("{fault} is missing from {report}"))
    })
    .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "the faults are out of path order:\n{report}");
}

/// A requirement that does not hold refuses before Coder is started, names
/// the requirement, and leaves no trace behind — there was no run to
/// record.
#[test]
fn a_wrong_commit_refuses_before_the_run() {
    let directory = tempfile::tempdir().unwrap();
    let repository = repository(directory.path());
    let base = "0".repeat(40);
    let task = task_with(
        directory.path(),
        &format!(r#"{{"base": "{base}", "capabilities": ["nothing-describes-this"]}}"#),
        "elsewhere",
    );
    let coder = fake_coder(directory.path(), &golden(), 0);
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository.display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(2), "{report}");
    assert!(report.contains("repository at 000000000000"), "{report}");
    assert!(
        report.contains("capability nothing-describes-this"),
        "{report}"
    );
    assert!(report.contains("Coder did not start"), "{report}");
    assert!(!trace.exists(), "nothing ran, so nothing recorded");
}

/// A checkout at the commit the task pins holds the requirement, so the
/// refusal is about the commit rather than about checking at all.
#[test]
fn the_right_commit_holds_the_requirement() {
    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path().join("checkout");
    std::fs::create_dir(&repository).unwrap();
    let head = checkout(&repository);
    let task = task_with(
        directory.path(),
        &format!(r#"{{"base": "{head}"}}"#),
        "here",
    );
    let coder = fake_coder(directory.path(), &golden(), 0);
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository.display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert!(report.contains("No faults"), "{report}");
}

/// A timeout before any trace exists reports that there is nothing to judge.
/// The complete-trace timeout is tested at the driver's result boundary in
/// the binary, where fixture publication does not race the process deadline.
#[test]
fn a_timeout_before_any_trace_exits_with_no_trace() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(directory.path(), r#"{}"#, "slow");
    let coder = fake_coder_script(directory.path(), "#!/bin/sh\nexec sleep 30\n");
    let trace = directory.path().join("run.atif.jsonl");

    let started = std::time::Instant::now();
    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository(directory.path()).display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
        "--timeout",
        "1",
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(3), "{report}");
    assert!(report.contains("no trace to check"), "{report}");
    assert!(!trace.exists());
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

/// A file the run wrote and never mentioned is still a write, because the
/// workspace is read rather than asked.
#[test]
fn a_write_nobody_reported_is_still_a_write() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(directory.path(), r#"{}"#, "wrote");
    let coder = fake_coder_that(directory.path(), &golden(), 0, "echo one > souvenir.txt");
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &repository(directory.path()).display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("wrote souvenir.txt, expected no writes"),
        "{report}"
    );
}

/// A directory the workspace reading cannot read leaves the write question
/// open. That is exit 4, not exit 0: nobody showed the run wrote nothing.
#[test]
fn a_workspace_removed_during_the_run_is_not_a_pass() {
    let directory = tempfile::tempdir().unwrap();
    let elsewhere = directory.path().join("not-a-checkout");
    std::fs::create_dir(&elsewhere).unwrap();
    let task = task_with(directory.path(), r#"{}"#, "unreadable");
    let coder = fake_coder_that(
        directory.path(),
        &golden(),
        0,
        &format!("cd /; rmdir '{}'", elsewhere.display()),
    );
    let trace = directory.path().join("run.atif.jsonl");

    let output = coderbench(&[
        "run",
        &task.display().to_string(),
        "--repository",
        &elsewhere.display().to_string(),
        "--coder",
        &coder.display().to_string(),
        "--trace",
        &trace.display().to_string(),
    ]);

    let report = said(&output);
    assert_eq!(output.status.code(), Some(4), "{report}");
    assert!(report.contains("unverifiable"), "{report}");
    assert!(
        report.contains("nothing compared the workspace"),
        "{report}"
    );
}

/// `diff` judges a trace that already exists and runs nothing.
///
/// A trace cannot carry the exit code or the workspace, so the best a diff
/// of a clean trace can answer is `unverifiable`. A trace that took none of
/// the path still fails, because a fault that was measured beats evidence
/// that was not — and so does a trace whose delegations are not the task's,
/// whatever it asserted about itself.
#[test]
fn a_diff_judges_a_trace_that_already_exists() {
    // The observed trace matches the questions, but an offline diff cannot
    // establish the live driver's exit or independent workspace observation.
    let output = coderbench(&["diff", "devin-fan-out-six", &golden().display().to_string()]);
    let report = said(&output);
    assert_eq!(output.status.code(), Some(4), "{report}");
    assert!(report.contains("unverifiable: 2 faults"), "{report}");

    // A trace holding the calls a sentence-driven run is expected to make
    // is missing the two things only a driver sees, which is
    // `unverifiable` rather than a pass.
    let directory = tempfile::tempdir().unwrap();
    let authored = directory.path().join("authored.atif.jsonl");
    std::fs::write(&authored, authored_text()).unwrap();
    let output = coderbench(&["diff", "devin-fan-out-six", &authored.display().to_string()]);
    let report = said(&output);
    assert_eq!(output.status.code(), Some(4), "{report}");
    assert!(report.contains("unverifiable: 2 faults"), "{report}");
    assert!(
        report.contains("nothing compared the workspace"),
        "{report}"
    );
    assert!(
        report.contains("the trace closed without saying how the episode ended"),
        "{report}"
    );

    let nothing = empty_handed(directory.path());
    let output = coderbench(&["diff", "devin-fan-out-six", &nothing.display().to_string()]);
    let report = said(&output);
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(report.contains("failed: 16 faults"), "{report}");
}

/// A trace that is not there is not a judgment, and it is not a clean run
/// either.
#[test]
fn a_missing_trace_is_its_own_exit_code() {
    let output = coderbench(&["diff", "devin-fan-out-six", "/nowhere/at/all.jsonl"]);
    assert_eq!(output.status.code(), Some(3), "{}", said(&output));
}

/// A wrong command line takes the usage code, outside the codes a judgment
/// produces.
#[test]
fn a_wrong_command_line_takes_the_usage_code() {
    let output = coderbench(&["run"]);
    assert_eq!(output.status.code(), Some(64), "{}", said(&output));
    assert!(said(&output).contains("run needs a task"));
}

/// Git status stays identical across both of these writes. The harness must
/// inspect contents, including ignored files, to detect them.
#[test]
fn writes_to_already_dirty_and_ignored_files_are_observed() {
    for name in ["README.md", "ignored.txt"] {
        let directory = tempfile::tempdir().unwrap();
        let repository = repository(directory.path());
        std::fs::write(repository.join(".gitignore"), "ignored.txt\n").unwrap();
        std::fs::write(repository.join(name), "before\n").unwrap();
        let status = || {
            let output = Command::new("git")
                .args(["status", "--porcelain=v1", "--untracked-files=all"])
                .current_dir(&repository)
                .output()
                .unwrap();
            assert!(output.status.success());
            output.stdout
        };
        let before = status();
        let task = task_with(directory.path(), r#"{}"#, "dirty-write");
        let coder = fake_coder_that(
            directory.path(),
            &golden(),
            0,
            &format!("printf after > '{name}'"),
        );
        let trace = directory.path().join("run.atif.jsonl");
        let output = coderbench(&[
            "run",
            &task.display().to_string(),
            "--repository",
            &repository.display().to_string(),
            "--coder",
            &coder.display().to_string(),
            "--trace",
            &trace.display().to_string(),
        ]);
        assert_eq!(
            before,
            status(),
            "Git status alone cannot detect this write"
        );
        let report = said(&output);
        assert_eq!(output.status.code(), Some(1), "{report}");
        assert!(
            report.contains(&format!("wrote {name}, expected no writes")),
            "{report}"
        );
    }
}

#[test]
fn tune_reads_recorded_series_and_uses_the_decision_gate() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(directory.path(), r#"{}"#, "tune-recorded");
    let traces = (1..=3)
        .map(|index| {
            let path = directory
                .path()
                .join(format!("recorded-{index}.atif.jsonl"));
            std::fs::write(&path, authored_text()).unwrap();
            path
        })
        .collect::<Vec<_>>();
    let arguments = vec![
        "tune".to_string(),
        task.display().to_string(),
        "--trace".to_string(),
        traces[0].display().to_string(),
        "--trace".to_string(),
        traces[1].display().to_string(),
        "--trace".to_string(),
        traces[2].display().to_string(),
        "--against".to_string(),
        directory.path().display().to_string(),
    ];
    let refs = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    let output = coderbench(&refs);
    let report = said(&output);
    assert_eq!(output.status.code(), Some(4), "{report}");
    assert!(report.contains("noise floor"), "{report}");
    assert!(report.contains("decision-v1"), "{report}");
    assert!(report.contains("unverifiable"), "{report}");
}

#[test]
fn tune_runs_the_requested_number_of_live_trials() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(directory.path(), r#"{}"#, "tune-live");
    let coder = fake_coder(directory.path(), &golden(), 0);
    let out = directory.path().join("trials");
    let task_arg = task.display().to_string();
    let repository_arg = repository(directory.path()).display().to_string();
    let coder_arg = coder.display().to_string();
    let out_arg = out.display().to_string();
    let output = coderbench(&[
        "tune",
        &task_arg,
        "--repository",
        &repository_arg,
        "--coder",
        &coder_arg,
        "--runs",
        "2",
        "--out",
        &out_arg,
    ]);
    let report = said(&output);
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert!(report.contains("2 runs"), "{report}");
    assert!(out.join("run-1.atif.jsonl").exists());
    assert!(out.join("run-2.atif.jsonl").exists());
}

#[test]
fn tune_refuses_before_a_required_capability_run() {
    let directory = tempfile::tempdir().unwrap();
    let task = task_with(
        directory.path(),
        r#"{"capabilities":["devin-local"]}"#,
        "tune-refused",
    );
    let task_arg = task.display().to_string();
    let out_arg = directory.path().join("trials").display().to_string();
    // With no approval on record the probe is `unprobed`, whatever the host
    // has installed.
    let output = Command::new(env!("CARGO_BIN_EXE_coderbench"))
        .args(["tune", &task_arg, "--runs", "2", "--out", &out_arg])
        .env(
            capability::STORE_ENV,
            directory.path().join("no-approvals.json"),
        )
        .output()
        .expect("the binary runs");
    let report = said(&output);
    assert_eq!(output.status.code(), Some(2), "{report}");
    assert!(report.contains("Coder did not start"), "{report}");
}

#[test]
fn tune_rejects_invalid_usage() {
    for arguments in [
        vec!["tune"],
        vec!["tune", "task", "--trace", "trace.atif.jsonl", "--runs", "2"],
        vec!["tune", "task", "--runs", "soon"],
    ] {
        let output = coderbench(&arguments);
        assert_eq!(output.status.code(), Some(64), "{}", said(&output));
    }
}
