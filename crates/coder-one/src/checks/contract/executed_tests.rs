use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

const INSTRUCTION: &str = include_str!("../../../fixtures/executed/instruction.md");

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/executed")
}

fn exited(code: i32) -> Ran {
    Ran {
        exit: Some(code),
        ..Ran::default()
    }
}

fn observed(code: i32) -> Option<Observed> {
    Some(Observed {
        exit: Some(code),
        timed_out: false,
    })
}

#[test]
fn the_rule_compares_with_the_untouched_outcome() {
    assert_eq!(verdict(observed(0), &exited(0), 60).0, Verdict::Ok);
    let (v, rule) = verdict(observed(0), &exited(1), 60);
    assert_eq!(v, Verdict::Regressed);
    assert_eq!(
        rule,
        "exited 0 on the untouched workspace and 1 on the candidate"
    );
    let killed = Ran::default();
    assert_eq!(verdict(observed(0), &killed, 60).0, Verdict::Regressed);
    assert_eq!(
        verdict(observed(2), &exited(2), 60).0,
        Verdict::NotARegression
    );
    assert_eq!(verdict(observed(2), &exited(0), 60).0, Verdict::Ok);
    let timed_out = Ran {
        timed_out: true,
        ..Ran::default()
    };
    let (v, rule) = verdict(observed(0), &timed_out, 60);
    assert_eq!(v, Verdict::Unknown);
    assert!(rule.contains("isn't a regression"), "{rule}");
    let refused = Ran {
        failed: Some("no enforced boundary".to_string()),
        ..Ran::default()
    };
    assert_eq!(verdict(observed(0), &refused, 60).0, Verdict::Unknown);
    assert_eq!(verdict(None, &exited(1), 60).0, Verdict::Unknown);
    let before_timed_out = Some(Observed {
        exit: None,
        timed_out: true,
    });
    assert_eq!(
        verdict(before_timed_out, &exited(1), 60).0,
        Verdict::NotARegression
    );
}

#[test]
fn compile_commands_follow_the_workspace_s_files() {
    let files = |list: &[&str]| list.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    let python = compile(&files(&[
        "tally/__init__.py",
        "tally/__main__.py",
        "tools/__init__.py",
        "run.py",
        "data/x.csv",
        "deep/pkg/__init__.py",
    ]));
    let commands: Vec<&str> = python.iter().map(|p| p.command.as_str()).collect();
    assert_eq!(
        commands,
        [
            "python3 -m compileall -q 'tally' 'tools' 'run.py'",
            "python3 -c 'import tally'",
            "python3 -c 'import tools'",
        ]
    );
    assert!(python.iter().all(|p| p.kind == "compile"));
    let nested = compile(&files(&["src/app/main.py"]));
    assert_eq!(nested[0].command, "python3 -m compileall -q .");
    let rust = compile(&files(&["Cargo.toml", "src/main.rs"]));
    assert_eq!(rust[0].command, "cargo check --offline --quiet");
    let go = compile(&files(&["go.mod", "main.go"]));
    assert_eq!(go[0].command, "go build ./...");
    assert!(compile(&files(&["README.md", "data.csv"])).is_empty());
}

#[tokio::test]
async fn named_commands_leave_out_expected_failures_and_installs() {
    let dir = tempfile::tempdir().expect("a directory");
    let host = Local {
        workdir: dir.path().to_path_buf(),
    };
    let pristine = extract::gather(&host, INSTRUCTION, "/app").await;
    let found: Vec<String> = named(INSTRUCTION, "/app", &pristine)
        .into_iter()
        .map(|p| p.command)
        .collect();
    assert_eq!(found, ["python3 -m tally data/numbers.txt", "sh report.sh"]);
    let install =
        "Install it with `pip install numpy`, then run `python3 tool.py`; it must succeed.";
    let pristine = extract::gather(&host, install, "/app").await;
    let found: Vec<String> = named(install, "/app", &pristine)
        .into_iter()
        .map(|p| p.command)
        .collect();
    assert_eq!(found, ["python3 tool.py"]);
}

fn record_of(stage: Stage, command: &str, exit: Option<i32>) -> Record {
    Record {
        schema: SCHEMA.to_string(),
        at: 1,
        stage,
        session: None,
        candidate: None,
        kind: "module".to_string(),
        command: command.to_string(),
        cwd: "/app".to_string(),
        exit,
        timed_out: false,
        ms: 1,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stdout_head: String::new(),
        stderr_head: String::new(),
        requirements: vec!["R3".to_string()],
        verdict: None,
        rule: None,
    }
}

#[test]
fn baseline_records_come_first_and_each_command_runs_once() {
    let baseline = vec![
        record_of(
            Stage::Baseline,
            "python3 -m tally data/numbers.txt",
            Some(0),
        ),
        record_of(Stage::AfterSession, "ignored", Some(0)),
    ];
    let planned = commands(
        &[
            "python3 -m tally data/numbers.txt".to_string(),
            "make test".to_string(),
        ],
        &baseline,
        vec![
            Planned::new("named", "python3 -m tally data/numbers.txt"),
            Planned::new("named", "sh report.sh"),
        ],
        vec![Planned::new("compile", "python3 -m compileall -q 'tally'")],
    );
    let listed: Vec<(&str, &str)> = planned
        .iter()
        .map(|p| (p.kind.as_str(), p.command.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("module", "python3 -m tally data/numbers.txt"),
            ("named", "make test"),
            ("named", "sh report.sh"),
            ("compile", "python3 -m compileall -q 'tally'"),
        ]
    );
    assert_eq!(planned[0].requirements, ["R3"]);
    let known = untouched(&baseline);
    assert_eq!(known.len(), 1);
    assert!(known["python3 -m tally data/numbers.txt"].passed());
}

#[test]
fn records_round_trip_in_the_run_card_s_shape() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("artifacts").join(FILE);
    let mut one = record_of(Stage::AfterSession, "sh report.sh", Some(1));
    one.session = Some(2);
    one.candidate = Some("lean-1/session-2".to_string());
    one.verdict = Some(Verdict::NotARegression);
    append(&path, std::slice::from_ref(&one)).expect("appends");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(b"{\"schema\": \"something else\"}\n"))
        .expect("appends a foreign line");
    append(&path, &[record_of(Stage::Baseline, "x", Some(0))]).expect("appends");
    let back = read(&path);
    assert_eq!(back.len(), 2);
    assert_eq!(back[0], one);
    let line: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(&path)
            .expect("reads")
            .lines()
            .next()
            .expect("a line"),
    )
    .expect("JSON");
    for field in [
        "schema",
        "at",
        "stage",
        "session",
        "candidate",
        "kind",
        "command",
        "cwd",
        "exit",
        "timed_out",
        "ms",
        "stdout_digest",
        "stderr_digest",
        "stdout_head",
        "stderr_head",
        "requirements",
        "verdict",
        "rule",
    ] {
        assert!(line.get(field).is_some(), "{field} is missing");
    }
    assert_eq!(line["stage"], "after_session");
    assert_eq!(line["verdict"], "not_a_regression");
}

#[test]
fn heads_keep_sixteen_kib_on_a_character_boundary() {
    let text = "é".repeat(HEAD_BYTES);
    let kept = head(&text);
    assert!(kept.len() <= HEAD_BYTES);
    assert_eq!(kept.len(), HEAD_BYTES);
    assert_eq!(head("short"), "short");
}

fn have_python() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn overlay(from: &Path, to: &Path) {
    for entry in walk(from) {
        let relative = entry.strip_prefix(from).expect("under the fixture");
        let target = to.join(relative);
        std::fs::create_dir_all(target.parent().expect("a parent")).expect("a directory");
        std::fs::copy(&entry, &target).expect("copies");
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("reads").flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// The untouched fixture's baseline, and a candidate's after-session
/// records, run for real with the contained host.
async fn fixture_run(candidate: &str) -> (Baseline, Vec<Record>, Vec<Record>) {
    let dir = tempfile::tempdir().expect("a directory");
    let work = dir.path().join("app");
    overlay(&fixtures().join("untouched"), &work);
    let pristine = dir.path().join("pristine");
    overlay(&fixtures().join("untouched"), &pristine);
    let place = Place {
        workdir: work.clone(),
        contained: true,
        wall: Duration::from_secs(5),
        budget: Duration::from_secs(120),
    };
    let (baseline, before) = prepare(INSTRUCTION, &pristine, &place, &[], &[]).await;
    overlay(&fixtures().join("candidates").join(candidate), &work);
    let after = after_session(&baseline, &place, 1, Some("lean-1/session-1".to_string()))
        .await
        .expect("a scratch copy");
    (baseline, before, after)
}

fn verdict_of(records: &[Record], command: &str) -> Option<Verdict> {
    records
        .iter()
        .find(|r| r.command == command)
        .and_then(|r| r.verdict)
}

const MODULE: &str = "python3 -m tally data/numbers.txt";
const IMPORT: &str = "python3 -c 'import tally'";
const REPORT: &str = "sh report.sh";

#[tokio::test]
async fn the_untouched_workspace_sets_the_baseline() {
    if !have_python() {
        return;
    }
    let (baseline, before, _) = fixture_run("no-regression").await;
    let commands: Vec<&str> = baseline
        .commands
        .iter()
        .map(|p| p.command.as_str())
        .collect();
    assert_eq!(
        commands,
        [MODULE, REPORT, "python3 -m compileall -q 'tally'", IMPORT]
    );
    assert!(before.iter().all(|r| r.stage == Stage::Baseline));
    assert!(before.iter().all(|r| r.verdict.is_none()));
    assert!(baseline.untouched[MODULE].passed());
    assert!(baseline.untouched[IMPORT].passed());
    assert!(!baseline.untouched[REPORT].passed());
}

#[tokio::test]
async fn a_candidate_with_no_regression_is_kept() {
    if !have_python() {
        return;
    }
    let (_, _, after) = fixture_run("no-regression").await;
    assert_eq!(verdict_of(&after, MODULE), Some(Verdict::Ok));
    assert_eq!(verdict_of(&after, IMPORT), Some(Verdict::Ok));
    assert_eq!(verdict_of(&after, REPORT), Some(Verdict::NotARegression));
    assert!(!rejects(&after));
    let module = after.iter().find(|r| r.command == MODULE).expect("ran");
    assert_eq!(module.stdout_head.trim(), "12 4.0");
    assert_eq!(module.session, Some(1));
    assert_eq!(module.candidate.as_deref(), Some("lean-1/session-1"));
    assert_eq!(module.stage, Stage::AfterSession);
}

#[tokio::test]
async fn a_crash_is_rejected() {
    if !have_python() {
        return;
    }
    let (_, _, after) = fixture_run("crash").await;
    assert_eq!(verdict_of(&after, MODULE), Some(Verdict::Regressed));
    assert_eq!(verdict_of(&after, IMPORT), Some(Verdict::Ok));
    assert!(rejects(&after));
    let module = after.iter().find(|r| r.command == MODULE).expect("ran");
    assert!(module.stderr_head.contains("ZeroDivisionError"));
    let note = note(1, &after, Some(0));
    assert!(note.contains(MODULE), "{note}");
    assert!(note.contains("ZeroDivisionError"), "{note}");
}

#[tokio::test]
async fn an_import_failure_is_rejected() {
    if !have_python() {
        return;
    }
    let (_, _, after) = fixture_run("import-failure").await;
    assert_eq!(verdict_of(&after, IMPORT), Some(Verdict::Regressed));
    assert_eq!(
        verdict_of(&after, "python3 -m compileall -q 'tally'"),
        Some(Verdict::Ok)
    );
    assert!(rejects(&after));
}

#[tokio::test]
async fn a_command_that_failed_before_too_is_not_a_regression() {
    if !have_python() {
        return;
    }
    let (_, _, after) = fixture_run("unrunnable").await;
    assert_eq!(verdict_of(&after, REPORT), Some(Verdict::NotARegression));
    assert!(!rejects(&after));
}

#[tokio::test]
async fn a_timeout_is_unknown_and_not_a_regression() {
    if !have_python() {
        return;
    }
    let (_, _, after) = fixture_run("timeout").await;
    let module = after.iter().find(|r| r.command == MODULE).expect("ran");
    assert_eq!(module.verdict, Some(Verdict::Unknown));
    assert!(module.timed_out);
    assert_eq!(module.exit, None);
    assert!(!rejects(&after));
}

#[tokio::test]
async fn nothing_is_written_to_the_real_workspace() {
    if !have_python() {
        return;
    }
    let dir = tempfile::tempdir().expect("a directory");
    let work = dir.path().join("app");
    overlay(&fixtures().join("untouched"), &work);
    let place = Place {
        workdir: work.clone(),
        contained: true,
        wall: Duration::from_secs(5),
        budget: Duration::from_secs(60),
    };
    let baseline = Baseline {
        commands: vec![
            Planned::new("compile", "python3 -m compileall -q 'tally'"),
            Planned::new("named", format!("touch {}/written", work.display())),
        ],
        untouched: BTreeMap::new(),
    };
    let after = after_session(&baseline, &place, 1, None)
        .await
        .expect("a scratch copy");
    assert_eq!(after.len(), 2);
    assert!(!work.join("tally/__pycache__").exists());
    assert!(
        !work.join("written").exists(),
        "the command was moved to the copy"
    );
    assert_eq!(
        after[1].command,
        format!("touch {}/written", work.display())
    );
}
