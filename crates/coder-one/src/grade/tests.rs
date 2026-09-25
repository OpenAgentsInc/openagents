//! Tests for [`crate::grade`]: the splitter on retained frozen scripts and
//! on scripts that don't split, the instrumentation, and the ranking key.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/grade")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn check_calls_split_one_line_each_with_their_setup() {
    let script = fixture("check-calls.sh");
    let parsed = split(&script);
    assert_eq!(parsed.split, Split::Lines);
    assert_eq!(parsed.sites.len(), 8);
    let last = &parsed.sites[7];
    assert_eq!(last.id, "c8");
    assert!(
        last.text.starts_with("check(np.isfinite(mmd(x,x))"),
        "{}",
        last.text
    );
    assert_eq!(
        script.lines().nth(last.line - 1).map(str::trim),
        Some(last.text.as_str())
    );
    assert!(last.context.contains("x=np.array"), "{}", last.context);
    // The setup statements on a check's own line are its context.
    let first = &parsed.sites[0];
    assert_eq!(first.text, "check(not sr['any_above_threshold'])");
    assert!(
        first
            .context
            .contains("mon=Monitor(r); sr=mon.process_window(st);")
    );
    // The recorder's own definition is never a check line.
    assert!(
        parsed
            .sites
            .iter()
            .all(|s| !s.text.contains("checks.append"))
    );
}

#[test]
fn named_lambdas_split_and_wrap_the_lambda_body() {
    let script = fixture("named-lambdas.sh");
    let parsed = split(&script);
    assert_eq!(parsed.split, Split::Lines);
    assert_eq!(parsed.sites.len(), 13);
    assert!(parsed.sites.iter().all(|s| s.span.lambda));
    let instrumented = instrument(&script, &parsed).expect("an instrumented copy");
    assert!(instrumented.contains(
        "check('KS location shift',lambda: _oa_mark('c4', (ks_test(np.arange(20.),np.arange(20.)+100)>0.9)))"
    ));
    // Checks inside the try block count too.
    assert!(
        parsed
            .sites
            .iter()
            .any(|s| s.text.contains("'drift persistence'"))
    );
}

#[test]
fn counters_and_appends_split() {
    let parsed = split(&fixture("if-counter.sh"));
    assert_eq!(parsed.split, Split::Lines);
    assert_eq!(parsed.sites.len(), 3);
    assert!(parsed.sites[0].text.starts_with("if not GarbageCollector"));
    let parsed = split(&fixture("list-appends.sh"));
    assert_eq!(parsed.split, Split::Lines);
    assert_eq!(parsed.sites.len(), 5);
    assert_eq!(
        parsed.sites[1].text,
        "checks.append(results[-1]['in_alert'])"
    );
}

#[test]
fn a_checks_list_or_a_shell_script_is_one_unit() {
    let parsed = split(&fixture("checks-list.sh"));
    assert_eq!(parsed.split, Split::OneUnit);
    assert_eq!(
        parsed.reason.as_deref(),
        Some("the checks are one list literal")
    );
    assert!(instrument(&fixture("checks-list.sh"), &parsed).is_none());
    let parsed = split(&fixture("shell-only.sh"));
    assert_eq!(parsed.split, Split::OneUnit);
    assert_eq!(
        parsed.reason.as_deref(),
        Some("the script runs no Python heredoc to split")
    );
}

#[test]
fn a_one_unit_script_is_graded_advisory_without_asking_jev() {
    let script = fixture("checks-list.sh");
    let recorder = Recorder::default();
    let (grades, _, calls) = futures_util::FutureExt::now_or_never(grade(
        &JevMode::Off,
        &recorder,
        &support::Context {
            component: COMPONENT,
            name: DECISION,
            id: "grade".to_string(),
            deadline: None,
        },
        &Freeze {
            check: "lean-1/evaluator/score.sh".to_string(),
            script: &script,
            frozen_after_session: 1,
            task: "Fix the monitor.",
            baseline: None,
            start: None,
        },
    ))
    .expect("no Jev call to wait on");
    assert!(calls.is_empty());
    assert_eq!(grades.split, Split::OneUnit);
    assert_eq!(grades.lines.len(), 1);
    assert_eq!(grades.lines[0].grade, Grade::Advisory);
    assert_eq!(grades.lines[0].jev.how, "skipped");
    assert!(!grades.baseline);
    let value = serde_json::to_value(&grades).expect("JSON");
    assert_eq!(value["schema"], SCHEMA);
    assert_eq!(value["split"], "one_unit");
    assert_eq!(value["lines"][0]["basis"], Value::Null);
}

#[test]
fn with_jev_off_every_line_is_unknown_and_ranks_nothing() {
    let script = fixture("check-calls.sh");
    let recorder = Recorder::default();
    let (mut grades, parsed, calls) = futures_util::FutureExt::now_or_never(grade(
        &JevMode::Off,
        &recorder,
        &support::Context {
            component: COMPONENT,
            name: DECISION,
            id: "grade".to_string(),
            deadline: None,
        },
        &Freeze {
            check: "lean-1/evaluator/score.sh".to_string(),
            script: &script,
            frozen_after_session: 1,
            task: "Fix the monitor.",
            baseline: None,
            start: None,
        },
    ))
    .expect("Jev off answers at once");
    assert_eq!(calls.len(), 2);
    assert!(grades.lines.iter().all(|l| l.grade == Grade::Unknown));
    let passed: BTreeMap<String, bool> = parsed
        .sites
        .iter()
        .map(|s| (s.id.clone(), s.id != "c8"))
        .collect();
    assert_eq!(grades.supported(&passed), Some((0, 0)));
    grades.record(1, &passed);
    assert_eq!(
        grades.lines[7].results,
        vec![LineResult {
            session: 1,
            passed: false
        }]
    );
    let value = serde_json::to_value(&grades).expect("JSON");
    assert_eq!(value["lines"][7]["results"][0]["passed"], false);
    assert_eq!(value["lines"][0]["jev"]["how"], "off");
}

#[test]
fn results_read_every_report_and_fail_a_silent_line() {
    let parsed = split(&fixture("check-calls.sh"));
    let output = "OA-CHECK c1 1\nOA-CHECK c2 0\nOA-CHECK c3 1\nOA-CHECK c3 0\nnoise\n";
    let got = results(output, &parsed);
    assert_eq!(got.len(), 8);
    assert!(got["c1"]);
    assert!(!got["c2"]);
    assert!(!got["c3"]);
    assert!(!got["c4"]);
    assert!(results("SCORE 8 8\n", &parsed).is_empty());
    assert_eq!(without_marks("OA-CHECK c1 1\nSCORE 1 1"), "SCORE 1 1");
}

#[test]
fn the_key_ranks_supported_lines_before_the_full_score() {
    // Two candidates with full raw scores tie on the raw score alone...
    let a = key(Some((3, 4)), Some((8, 8)));
    let b = key(Some((4, 4)), Some((7, 8)));
    assert!(ahead(b, a, false));
    assert!(!ahead(a, b, true));
    // ...and with no supported lines the full score decides.
    let c = key(Some((0, 0)), Some((8, 8)));
    let d = key(Some((0, 0)), Some((7, 8)));
    assert!(ahead(c, d, false));
    assert!(ahead(c, c, true));
    assert!(!ahead(c, c, false));
    // Unknown results rank below known ones.
    assert!(ahead(
        key(Some((0, 1)), Some((0, 8))),
        key(None, Some((8, 8))),
        false
    ));
}

#[test]
fn the_instrumented_script_scores_the_same_and_reports_each_line() {
    let python = std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !python {
        eprintln!("python3 isn't installed; skipping");
        return;
    }
    let script = fixture("runnable.sh");
    let parsed = split(&script);
    assert_eq!(parsed.split, Split::Lines);
    assert_eq!(parsed.sites.len(), 6);
    let instrumented = instrument(&script, &parsed).expect("an instrumented copy");
    let dir = tempfile::tempdir().expect("a directory");
    let run = |text: &str| {
        let path = dir.path().join("score.sh");
        std::fs::write(&path, text).expect("write");
        let out = std::process::Command::new("/bin/sh")
            .arg(&path)
            .output()
            .expect("sh runs");
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let (plain, _) = run(&script);
    let (stdout, stderr) = run(&instrumented);
    assert_eq!(plain.trim(), "SCORE 5 8");
    assert_eq!(stdout.trim(), "SCORE 5 8");
    let got = results(&stderr, &parsed);
    let expected: BTreeMap<String, bool> = [
        ("c1", true),
        ("c2", false),
        ("c3", true),
        ("c4", false),
        ("c5", false),
        ("c6", true),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(got, expected, "{stderr}");
}

#[test]
fn the_authority_class_sets_the_grade() {
    use crate::accept::authority::{Authority, Evidence};
    let evidence = |green: Option<bool>, p: Option<f64>| Evidence {
        green_at_start: green,
        faithful: p,
        ..Evidence::default()
    };
    // Supported and red on the untouched workspace: writer-derived, which
    // may rank, so it follows.
    let (grade, basis, class) = line_grade(
        Some((Basis::Standard, 0.8)),
        evidence(Some(false), Some(0.8)),
    );
    assert_eq!(grade, Grade::Follows);
    assert_eq!(basis, Some(Basis::Standard));
    assert_eq!(class.class, Authority::WriterDerived);
    // Supported but green on the untouched workspace: a guard, as v7's
    // frozen `mmd(x, x) == 0` was on the biased code. Advisory.
    let (grade, basis, class) = line_grade(
        Some((Basis::Standard, 0.82)),
        evidence(Some(true), Some(0.82)),
    );
    assert_eq!(grade, Grade::Advisory);
    assert_eq!(basis, None);
    assert_eq!(class.class, Authority::Guard);
    // Red but below the support threshold: unsupported, advisory.
    let (grade, _, class) = line_grade(Some((Basis::Task, 0.3)), evidence(Some(false), Some(0.3)));
    assert_eq!(grade, Grade::Advisory);
    assert_eq!(class.class, Authority::Unsupported);
    // Never run on the untouched workspace: unsupported, advisory.
    let (grade, _, class) = line_grade(Some((Basis::Task, 0.9)), evidence(None, Some(0.9)));
    assert_eq!(grade, Grade::Advisory);
    assert_eq!(class.class, Authority::Unsupported);
    // No support answer: unknown.
    let (grade, _, _) = line_grade(None, evidence(Some(false), None));
    assert_eq!(grade, Grade::Unknown);
}
