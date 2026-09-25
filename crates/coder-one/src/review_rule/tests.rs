use super::*;

fn target(id: &str, kind: &str, paths: &[&str], commands: &[&str]) -> Target {
    Target {
        id: id.to_string(),
        kind: kind.to_string(),
        paths: paths.iter().map(ToString::to_string).collect(),
        commands: commands.iter().map(ToString::to_string).collect(),
    }
}

fn check(session: u32, command: &str, verdict: Option<&str>) -> Executed {
    Executed {
        stage: "after_session".to_string(),
        session: Some(session),
        kind: "named".to_string(),
        command: command.to_string(),
        cwd: Some("/app".to_string()),
        exit: Some(0),
        requirements: Vec::new(),
        verdict: verdict.map(str::to_string),
    }
}

/// Every trigger clear: a full score, one executed check that ran the
/// named program on the deliverable, and a hard-coding answer of no.
fn quiet() -> Evidence {
    Evidence {
        session: 1,
        score: Some((6, 6)),
        grades: None,
        executed: Some(vec![check(
            1,
            "python3 -m report --out /app/out/report.csv",
            Some("ok"),
        )]),
        hardcoded: Some(false),
        targets: vec![
            target("R1", "behavior", &[], &[]),
            target("R2", "deliverable", &["/app/out/report.csv"], &[]),
        ],
    }
}

fn reading(decision: &Decision, trigger: Trigger) -> Reading {
    decision
        .triggers
        .iter()
        .find(|r| r.trigger == trigger)
        .unwrap()
        .reading
}

#[test]
fn nothing_disagrees_so_the_episode_finishes() {
    let decision = decide(&quiet(), Params::default());
    assert!(!decision.review, "{decision:?}");
    assert!(decision.fired.is_empty());
    assert!(decision.unknown.is_empty());
    assert_eq!(decision.words(), "none");
    assert_eq!(decision.reason, "no review: no trigger fired");
}

#[test]
fn a_score_below_full_fires_without_grades() {
    let mut evidence = quiet();
    evidence.score = Some((5, 6));
    let decision = decide(&evidence, Params::default());
    assert!(decision.review);
    assert_eq!(decision.fired, vec![Trigger::Score]);
}

#[test]
fn an_advisory_line_that_fails_doesnt_fire() {
    let mut evidence = quiet();
    evidence.score = Some((5, 6));
    evidence.grades = Some(Grades {
        lines: vec![
            GradedLine {
                id: "c1".to_string(),
                grade: "follows".to_string(),
                results: vec![LineResult {
                    session: 1,
                    passed: true,
                }],
                ..GradedLine::default()
            },
            GradedLine {
                id: "c8".to_string(),
                grade: "advisory".to_string(),
                results: vec![LineResult {
                    session: 1,
                    passed: false,
                }],
                ..GradedLine::default()
            },
        ],
    });
    let decision = decide(&evidence, Params::default());
    assert_eq!(reading(&decision, Trigger::Score), Reading::Clear);
    assert!(!decision.review);
    // A failing line that counts fires.
    evidence.grades.as_mut().unwrap().lines[0].results[0].passed = false;
    let decision = decide(&evidence, Params::default());
    assert_eq!(decision.fired, vec![Trigger::Score]);
    assert!(decision.triggers[0].detail.contains("c1"));
}

#[test]
fn a_regressed_check_fires_and_names_its_command() {
    let mut evidence = quiet();
    let mut regressed = check(1, "python3 -m report --check", Some("regressed"));
    regressed.requirements = vec!["R2".to_string()];
    evidence.executed.as_mut().unwrap().push(regressed);
    let decision = decide(&evidence, Params::default());
    assert_eq!(decision.fired, vec![Trigger::Regressed]);
    let reason = &decision.triggers[1];
    assert_eq!(reason.commands, vec!["python3 -m report --check"]);
    assert_eq!(reason.requirements, vec!["R2"]);
}

#[test]
fn a_later_rejected_regression_fires_and_an_earlier_one_doesnt() {
    // Session 2 regressed and was rejected, so the review reads session
    // 1's kept candidate: the later work still disagrees.
    let mut evidence = quiet();
    evidence.executed.as_mut().unwrap().push(check(
        2,
        "python3 -m report --check",
        Some("regressed"),
    ));
    let decision = decide(&evidence, Params::default());
    assert_eq!(decision.fired, vec![Trigger::Regressed]);
    assert!(decision.triggers[1].detail.contains("session 2"));
    // A regression before the reviewed candidate is history.
    let mut evidence = quiet();
    evidence.session = 2;
    evidence.executed = Some(vec![
        check(1, "python3 -m report --check", Some("regressed")),
        check(2, "python3 -m report --out /app/out/report.csv", Some("ok")),
    ]);
    assert!(!decide(&evidence, Params::default()).review);
}

#[test]
fn the_hardcoding_flag_fires() {
    let mut evidence = quiet();
    evidence.hardcoded = Some(true);
    assert_eq!(
        decide(&evidence, Params::default()).fired,
        vec![Trigger::Hardcoded]
    );
}

#[test]
fn a_deliverable_no_check_touches_fires() {
    let mut evidence = quiet();
    evidence
        .targets
        .push(target("R3", "deliverable", &["/app/out/rwa.xlsx"], &[]));
    evidence
        .targets
        .push(target("R4", "check", &[], &["pytest tests/test_rwa.py"]));
    // A constraint that names a path isn't in scope.
    evidence
        .targets
        .push(target("R5", "constraint", &["/app/src/only.py"], &[]));
    let decision = decide(&evidence, Params::default());
    assert_eq!(decision.fired, vec![Trigger::Uncovered]);
    assert_eq!(decision.triggers[3].requirements, vec!["R3", "R4"]);
    // A check that names the file, relative to the working directory, and
    // one that lists the requirement cover both.
    let executed = evidence.executed.as_mut().unwrap();
    executed.push(check(1, "python3 verify.py out/rwa.xlsx", Some("ok")));
    let mut listed = check(1, "sh run_tests.sh", Some("ok"));
    listed.requirements = vec!["R4".to_string()];
    executed.push(listed);
    assert!(!decide(&evidence, Params::default()).review);
}

#[test]
fn a_named_command_covers_its_requirement() {
    let t = target("R4", "check", &[], &["pytest  tests/test_rwa.py"]);
    assert!(touches(
        &check(1, "cd /app && pytest tests/test_rwa.py -q", None),
        &t
    ));
    assert!(!touches(&check(1, "pytest tests/other.py", None), &t));
}

#[test]
fn a_directory_path_is_touched_by_a_file_under_it() {
    let t = target("R3", "deliverable", &["/app/interface/"], &[]);
    assert!(touches(
        &check(1, "scalac /app/interface/Dedup.scala", None),
        &t
    ));
}

#[test]
fn missing_records_read_unknown_and_unknown_fires_by_default() {
    let evidence = Evidence {
        executed: None,
        hardcoded: None,
        ..quiet()
    };
    let decision = decide(&evidence, Params::default());
    assert!(decision.fired.is_empty());
    assert_eq!(
        decision.unknown,
        vec![Trigger::Regressed, Trigger::Hardcoded, Trigger::Uncovered]
    );
    assert!(decision.review);
    assert_eq!(decision.words(), "unknown: regressed, hardcoded, uncovered");
    let decision = decide(
        &evidence,
        Params {
            unknown_fires: false,
        },
    );
    assert!(!decision.review);
    assert!(decision.reason.contains("unknown doesn't fire"));
}

#[test]
fn records_that_dont_cover_the_candidate_read_unknown() {
    let mut evidence = quiet();
    evidence.executed = Some(vec![Executed {
        stage: "baseline".to_string(),
        ..check(1, "python3 -m report", None)
    }]);
    let decision = decide(&evidence, Params::default());
    assert_eq!(reading(&decision, Trigger::Regressed), Reading::Unknown);
    assert_eq!(reading(&decision, Trigger::Uncovered), Reading::Unknown);
}

#[test]
fn an_unknown_verdict_leaves_the_regression_unknown() {
    let mut evidence = quiet();
    evidence.executed.as_mut().unwrap()[0].verdict = Some("unknown".to_string());
    assert_eq!(
        reading(&decide(&evidence, Params::default()), Trigger::Regressed),
        Reading::Unknown
    );
}

#[test]
fn the_hardcoding_record_reads_as_the_loop_wrote_it() {
    assert_eq!(hardcoded_of(&Value::Null), None);
    assert_eq!(
        hardcoded_of(&json!({"literal": [], "p": 0.02, "flagged": false})),
        Some(false)
    );
    assert_eq!(
        hardcoded_of(&json!({"literal": [], "p": null, "flagged": false})),
        None
    );
    assert_eq!(
        hardcoded_of(&json!({"literal": [{"file": "a"}], "p": null, "flagged": true})),
        Some(true)
    );
}

#[test]
fn concerns_are_read_from_the_summary() {
    let summary = "Checked every requirement.\n\n\
        CONCERN R3: the replacement cost is 0 where the task's netting rule gives a positive value \
        | command: `python3 -c \"import calc; print(calc.rc())\"`\n\
        - CONCERN R7: the workbook has no live formulas\n\
        CONCERN the summary says nothing about units\n";
    let concerns = parse_concerns(summary);
    assert_eq!(concerns.len(), 3);
    assert_eq!(concerns[0].requirement.as_deref(), Some("R3"));
    assert_eq!(
        concerns[0].command.as_deref(),
        Some("python3 -c \"import calc; print(calc.rc())\"")
    );
    assert_eq!(concerns[1].requirement.as_deref(), Some("R7"));
    assert_eq!(concerns[1].command, None);
    assert_eq!(concerns[2].requirement, None);
    assert!(parse_concerns("All good.\nCONCERN none").is_empty());
    let record = concerns_record(2, summary, &[target("R3", "deliverable", &[], &[])]);
    assert_eq!(record["with_requirement"], 2);
    assert_eq!(record["with_command"], 1);
    assert_eq!(record["unknown_requirements"], json!(["R7"]));
}

#[test]
fn the_guidance_names_what_fired() {
    let mut evidence = quiet();
    evidence.hardcoded = Some(true);
    let text = guidance(&decide(&evidence, Params::default()));
    assert!(text.contains("flagged session 1's candidate as hard-coded"));
    assert!(text.contains("CONCERN none"));
}

#[test]
fn the_records_are_read_from_the_first_directory_that_has_them() {
    let dir = tempfile::tempdir().unwrap();
    let group = dir.path().join("lean-1");
    std::fs::create_dir_all(&group).unwrap();
    assert_eq!(read_executed(&[&group, dir.path()]), None);
    let line = json!({
        "schema": EXECUTED_SCHEMA,
        "stage": "after_session",
        "session": 1,
        "kind": "score",
        "command": "sh score.sh",
        "exit": 0,
        "requirements": [],
        "verdict": null,
    });
    std::fs::write(
        dir.path().join(EXECUTED_FILE),
        format!("{line}\n{{\"schema\": \"other\"}}\n"),
    )
    .unwrap();
    let read = read_executed(&[&group, dir.path()]).unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].kind, "score");
    std::fs::write(
        group.join(GRADES_FILE),
        json!({"schema": GRADES_SCHEMA, "lines": [{"id": "c1", "grade": "follows"}]}).to_string(),
    )
    .unwrap();
    assert_eq!(read_grades(&[&group]).unwrap().lines[0].id, "c1");
}
