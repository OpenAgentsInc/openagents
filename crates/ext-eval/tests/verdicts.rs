//! Gate verdict fixtures and the report contract.
//!
//! Each scenario under `tests/fixtures/scenarios/` is a set of finished runs
//! of the tiny suite, built from hand-written ATIF trajectories. The test
//! evaluates it with a fake decision door and compares `report.json` with
//! `tests/fixtures/expected/<scenario>.report.json`. Set
//! `EXT_EVAL_BLESS=1` to rewrite the expected files after a reviewed change.

mod common;

use ext_eval::{Doors, Evaluation, Verdict, evaluate, load_gate, validate};
use gym::gate::Verdict as GateVerdict;
use serde_json::Value;

fn run(name: &str) -> Evaluation {
    let scenario = common::scenario(name);
    let (gate, gate_file) = load_gate().expect("the committed gate loads");
    let door = common::decision_door();
    evaluate(
        &common::suite(),
        &common::plan(scenario.runs),
        scenario.records,
        &common::identity(),
        (&gate, &gate_file),
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    )
    .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn bless() -> bool {
    std::env::var_os("EXT_EVAL_BLESS").is_some()
}

/// Compares `actual` with the committed expected file, or rewrites it.
fn expect_file(name: &str, actual: &[u8]) {
    let path = common::fixtures().join("expected").join(name);
    if bless() {
        std::fs::write(&path, actual).expect("the expected file writes");
        return;
    }
    let expected = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("{}: {error}; run with EXT_EVAL_BLESS=1", path.display()));
    assert!(
        expected == actual,
        "{name} differs from the committed file; review the change and rerun with \
         EXT_EVAL_BLESS=1\n--- actual ---\n{}",
        String::from_utf8_lossy(actual)
    );
}

fn check(name: &str, verdict: Verdict) -> Evaluation {
    let evaluation = run(name);
    assert_eq!(
        evaluation.verdict, verdict,
        "{name}: {:#?}",
        evaluation.gate
    );
    validate(&evaluation.report, Some(&evaluation.artifacts))
        .unwrap_or_else(|problems| panic!("{name}: {problems:#?}"));
    expect_file(&format!("{name}.report.json"), &evaluation.report_bytes);
    evaluation
}

fn criterion<'a>(evaluation: &'a Evaluation, name: &str) -> &'a gym::gate::Criterion {
    evaluation
        .gate
        .criteria
        .iter()
        .find(|criterion| criterion.name == name)
        .unwrap_or_else(|| panic!("no criterion {name}"))
}

#[test]
fn better_when_the_tool_wins_cases_and_clears_the_spread() {
    let evaluation = check("better", Verdict::Pass);
    let scores = &evaluation.scores;
    assert_eq!(scores.subject.cases_passed, 4);
    assert_eq!(scores.baseline.as_ref().unwrap().cases_passed, 2);
    assert_eq!(scores.comparison.cases, 3, "map-only is not compared");
    assert_eq!(scores.comparison.subject_passed, 3);
    assert_eq!(scores.comparison.baseline_passed, 2);
    assert_eq!(evaluation.exit_code(), 0);
    assert_eq!(evaluation.verdict.plain(), "Better");
    assert_eq!(
        criterion(&evaluation, "improvement_clears_the_spread").verdict,
        GateVerdict::Passed
    );
    let headline = &evaluation.report["meta"]["ext_eval"]["headline"];
    assert_eq!(headline["subject_passed"], 4);
    assert_eq!(headline["baseline_passed"], 2);
    assert_eq!(headline["total"], 4);
    assert!(
        evaluation.fits_inline(),
        "{} bytes",
        evaluation.report_bytes.len()
    );
}

#[test]
fn worse_when_the_tool_passes_fewer_cases() {
    let evaluation = check("worse-fewer", Verdict::Fail);
    assert_eq!(
        criterion(&evaluation, "subject_passes_at_least_as_many_cases").verdict,
        GateVerdict::Failed
    );
    assert_eq!(evaluation.exit_code(), 1);
    assert_eq!(evaluation.verdict.plain(), "Worse");
}

#[test]
fn worse_when_the_tool_loses_a_should_not_fire_case() {
    let evaluation = check("worse-should-not-fire", Verdict::Fail);
    assert_eq!(
        criterion(&evaluation, "subject_passes_at_least_as_many_cases").verdict,
        GateVerdict::Passed,
        "the passes tie"
    );
    let lost = criterion(&evaluation, "subject_keeps_every_should_not_fire_case");
    assert_eq!(lost.verdict, GateVerdict::Failed);
    assert!(lost.detail.contains("typo-fix"), "{}", lost.detail);
}

#[test]
fn inconclusive_when_the_change_is_inside_the_spread() {
    let evaluation = check("inconclusive-spread", Verdict::Inconclusive);
    assert_eq!(
        criterion(&evaluation, "improvement_clears_the_spread").verdict,
        GateVerdict::Unverifiable
    );
    assert_eq!(evaluation.exit_code(), 1);
    assert_eq!(evaluation.verdict.plain(), "No clear change");
}

#[test]
fn inconclusive_with_one_run_whatever_the_passes_say() {
    let evaluation = check("runs-1", Verdict::Inconclusive);
    let floor = criterion(&evaluation, "runs_per_arm>=2");
    assert_eq!(floor.verdict, GateVerdict::Unverifiable);
    for name in [
        "subject_passes_at_least_as_many_cases",
        "subject_keeps_every_should_not_fire_case",
        "improvement_clears_the_spread",
    ] {
        let judged = criterion(&evaluation, name);
        assert_eq!(judged.verdict, GateVerdict::Unverifiable, "{name}");
        assert!(judged.detail.starts_with("not judged"), "{name}");
    }
}

#[test]
fn a_run_that_did_not_finish_makes_the_report_partial() {
    // What finished is the better scenario less two runs; the timed-out run
    // scores zero, which widens the spread past the improvement.
    let evaluation = check("partial", Verdict::Inconclusive);
    assert_eq!(evaluation.partial.as_deref(), Some("1 run did not finish"));
    assert_eq!(evaluation.exit_code(), 2);
    let coverage = &evaluation.report["coverage"];
    assert_eq!(coverage["subject"]["failed"], 1, "the timeout");
    assert_eq!(coverage["baseline"]["unknown"], 1);
    let limitations = evaluation
        .artifacts
        .get("limitations.json")
        .map(|bytes| serde_json::from_slice::<Value>(bytes).unwrap())
        .unwrap();
    assert_eq!(limitations["partial"], "1 run did not finish");
    // The timed-out run's cost is unknown, so the arm's cost is unknown.
    let cost = evaluation.report["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["arm"] == "subject" && m["metric"] == "cost_usd")
        .unwrap();
    assert_eq!(cost["value"], Value::Null);
    assert_eq!(cost["unknown_count"], 1);
}

#[test]
fn the_gate_digest_is_recorded_in_the_report() {
    let evaluation = run("better");
    let (gate, _) = load_gate().unwrap();
    assert_eq!(evaluation.gate.gate_id, "ext-eval-v1");
    assert_eq!(evaluation.gate.gate_digest, gate.digest());
    assert_eq!(
        evaluation.report["meta"]["ext_eval"]["gate"],
        format!("sha256:{}", &gate.digest()["gate:".len()..])
    );
    let suite: Value = serde_json::from_slice(&evaluation.artifacts["suite.json"]).unwrap();
    let acceptance = &suite["acceptance"];
    nostr::contracts::parse_definition(acceptance).expect("acceptance is a DefinitionRef");
    let gate_file = std::fs::read(gym::gate::gates_dir().join("ext-eval-v1.json")).unwrap();
    let artifact = nostr::contracts::parse_artifact(&acceptance["artifact"]).unwrap();
    nostr::contracts::check_artifact_bytes(&artifact, &gate_file).expect("the gate's exact bytes");
}

#[test]
fn every_reference_in_the_report_resolves_to_exact_bytes() {
    let evaluation = run("better");
    for (name, bytes) in &evaluation.artifacts {
        let value: Value = serde_json::from_slice(bytes).unwrap_or_else(|_| panic!("{name}"));
        assert!(value.get("v").is_some(), "{name} names its schema");
    }
    let runs: Value = serde_json::from_slice(&evaluation.artifacts["runs.json"]).unwrap();
    let entries = runs["runs"].as_array().unwrap();
    // 4 cases x 3 runs with the tool, 3 cases x 3 runs without (map-only is
    // subject-only and never runs without the tool).
    assert_eq!(entries.len(), 21);
    for entry in entries {
        let keys: Vec<&str> = entry
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["arm", "case", "attempt", "outcome", "receipts", "artifacts"],
            "NIP-EVAL run entries"
        );
        let artifacts = entry["artifacts"].as_array().unwrap();
        let trajectory = nostr::contracts::parse_artifact(&artifacts[0]).unwrap();
        assert_eq!(trajectory.schema.as_deref(), Some("ATIF-v1.8"));
    }
    // The report's own reference is over the bytes written.
    let reference = nostr::contracts::parse_artifact(&evaluation.report_ref.value()).unwrap();
    nostr::contracts::check_artifact_bytes(&reference, &evaluation.report_bytes).unwrap();
}

#[test]
fn the_validator_refuses_broken_reports() {
    let evaluation = run("better");
    let broken = |edit: &dyn Fn(&mut Value)| {
        let mut report = evaluation.report.clone();
        edit(&mut report);
        validate(&report, Some(&evaluation.artifacts)).expect_err("refused")
    };
    let problems = broken(&|report| report["coverage"]["subject"]["completed"] = 0.into());
    assert!(
        problems.iter().any(|p| p.contains("sum to")),
        "{problems:?}"
    );
    let problems = broken(&|report| report["measurements"][0]["denominator"] = (-1).into());
    assert!(
        problems.iter().any(|p| p.contains("denominator")),
        "{problems:?}"
    );
    let problems = broken(&|report| report["measurements"][0]["unknown_count"] = 1.5.into());
    assert!(
        problems.iter().any(|p| p.contains("unknown_count")),
        "{problems:?}"
    );
    let problems = broken(&|report| report["measurements"][0]["metric"] = "accuracy".into());
    assert!(
        problems.iter().any(|p| p.contains("not a suite metric")),
        "{problems:?}"
    );
    let problems = broken(&|report| {
        report["baseline"] = Value::Null;
        report["coverage"]["baseline"] = Value::Null;
    });
    assert!(
        problems.iter().any(|p| p.contains("can't claim a change")),
        "{problems:?}"
    );
    let problems = broken(&|report| report["meta"]["ext_eval"]["headline"]["total"] = 9.into());
    assert!(problems.iter().any(|p| p.contains("total")), "{problems:?}");
    let problems =
        broken(&|report| report["runs"]["digest"] = format!("sha256:{}", "0".repeat(64)).into());
    assert!(
        problems.iter().any(|p| p.contains("exact bytes")),
        "{problems:?}"
    );
    let problems = broken(&|report| report["extra"] = 1.into());
    assert!(
        problems.iter().any(|p| p.contains("unknown report key")),
        "{problems:?}"
    );
    let problems = broken(&|report| report["started_at"] = 1_900_000_000u64.into());
    assert!(problems.iter().any(|p| p.contains("after")), "{problems:?}");
}

#[test]
fn a_single_arm_run_claims_no_change() {
    let scenario = common::scenario("better");
    let records = scenario
        .records
        .into_iter()
        .filter(|record| record.arm == ext_eval::Arm::Subject)
        .collect();
    let (gate, gate_file) = load_gate().unwrap();
    let door = common::decision_door();
    let mut plan = common::plan(None);
    plan.baseline = false;
    let mut identity = common::identity();
    identity.baseline = None;
    let evaluation = evaluate(
        &common::suite(),
        &plan,
        records,
        &identity,
        (&gate, &gate_file),
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    )
    .unwrap();
    assert_eq!(evaluation.verdict, Verdict::Inconclusive);
    assert_eq!(evaluation.exit_code(), 0, "a clean single-arm run");
    assert_eq!(evaluation.report["baseline"], Value::Null);
    assert_eq!(evaluation.report["coverage"]["baseline"], Value::Null);
    validate(&evaluation.report, Some(&evaluation.artifacts)).unwrap();
}

#[test]
fn records_outside_the_plan_are_refused() {
    let (gate, gate_file) = load_gate().unwrap();
    let mut records = common::scenario("better").records;
    let mut extra = records[0].clone();
    extra.attempt = 9;
    records.push(extra);
    let error = evaluate(
        &common::suite(),
        &common::plan(None),
        records,
        &common::identity(),
        (&gate, &gate_file),
        Doors::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("not in the plan"), "{error}");

    let mut records = common::scenario("better").records;
    records.push(records[0].clone());
    let error = evaluate(
        &common::suite(),
        &common::plan(None),
        records,
        &common::identity(),
        (&gate, &gate_file),
        Doors::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("two records"), "{error}");
}

#[test]
fn another_gate_is_refused() {
    let decision = gym::gate::load("decision-v1").unwrap();
    let error = evaluate(
        &common::suite(),
        &common::plan(None),
        Vec::new(),
        &common::identity(),
        (&decision, b"{}"),
        Doors::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("does not judge"), "{error}");
}

#[test]
fn the_html_is_self_contained() {
    let evaluation = run("better");
    let html = &evaluation.html;
    for forbidden in [
        "http://", "https://", "//cdn", "<script", "<link", "<img", "<iframe", "@import", "url(",
        " src=",
    ] {
        assert!(
            !html.contains(forbidden),
            "report.html contains {forbidden}"
        );
    }
    assert!(html.contains("Better"));
    assert!(html.contains("href=\"runs/find-callers/subject-1/trajectory.json\""));
    assert!(html.contains(&evaluation.gate.gate_digest));
    expect_file("better.report.html", html.as_bytes());
}

#[test]
fn the_results_directory_holds_the_report_and_its_documents() {
    let evaluation = run("better");
    let dir = tempfile::tempdir().unwrap();
    evaluation.write(dir.path()).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("report.json")).unwrap(),
        evaluation.report_bytes
    );
    assert!(dir.path().join("report.html").is_file());
    assert!(dir.path().join("artifacts/suite.json").is_file());
    assert!(dir.path().join("artifacts/runs.json").is_file());
    assert!(
        dir.path()
            .join("artifacts/grades/find-callers/subject-1.json")
            .is_file()
    );
    assert!(dir.path().join("artifacts/cases.json").is_file());
}

#[test]
fn the_report_and_suite_read_as_the_nostr_wire_contract() {
    use nostr::eval_ext;
    let evaluation = run("better");
    let report = eval_ext::parse_report(&evaluation.report_bytes).expect("parse_report");
    assert_eq!(report.verdict, eval_ext::Verdict::Pass);
    assert_eq!(report.profile.headline.baseline_passed, Some(2));
    assert_eq!(report.profile.cases.len(), 4);

    let suite = eval_ext::parse_suite(&evaluation.artifacts["suite.json"]).expect("parse_suite");
    nostr::contracts::check_artifact_bytes(&suite.cases, &evaluation.artifacts["cases.json"])
        .expect("the cases artifact");
    let manifest =
        eval_ext::parse_case_manifest(&evaluation.artifacts["cases.json"]).expect("the manifest");
    let ids: Vec<&str> = manifest.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(
        ids,
        ["find-callers", "map-only", "typo-fix", "write-summary"]
    );
    assert_eq!(manifest.should_not_fire(), 1);
    let summary = &manifest.cases[3];
    assert!(summary.config.is_some(), "case.toml is the config");
    assert_eq!(summary.graders[0].name, "summary-mentions");
    assert_eq!(summary.fixtures[0].name, "README.md");

    // A published suite's release makes the report publishable as a 3189.
    let mut identity = common::identity();
    identity.suite_release = Some(serde_json::json!({
        "id": "ab".repeat(32),
        "pubkey": common::AUTHOR,
        "kind": nostr::kinds::EXT_RELEASE,
    }));
    let scenario = common::scenario("better");
    let (gate, gate_file) = load_gate().unwrap();
    let door = common::decision_door();
    let published = evaluate(
        &common::suite(),
        &common::plan(None),
        scenario.records,
        &identity,
        (&gate, &gate_file),
        Doors {
            decision: Some(&door),
            ..Doors::default()
        },
    )
    .unwrap();
    let text = String::from_utf8(published.report_bytes.clone()).unwrap();
    let unsigned = eval_ext::publication(&text, None).expect("a 3189 builds");
    assert_eq!(unsigned.kind, 3189);
}

#[test]
fn an_identity_that_fails_the_contracts_is_refused() {
    let (gate, gate_file) = load_gate().unwrap();
    let attempt = |edit: &dyn Fn(&mut ext_eval::Identity)| {
        let mut identity = common::identity();
        edit(&mut identity);
        evaluate(
            &common::suite(),
            &common::plan(None),
            Vec::new(),
            &identity,
            (&gate, &gate_file),
            Doors::default(),
        )
        .unwrap_err()
        .to_string()
    };
    assert!(attempt(&|i| i.evaluator = "local-provenance".into()).contains("evaluator"));
    assert!(attempt(&|i| i.author = "nope".into()).contains("author"));
    assert!(attempt(&|i| i.baseline = None).contains("baseline"));
    assert!(
        attempt(&|i| {
            i.requester = Some(
                serde_json::json!({"id": "ab".repeat(32), "pubkey": common::AUTHOR, "kind": 1}),
            );
        })
        .contains("execution request")
    );
    assert!(attempt(&|i| i.started_at = i.ended_at + 1).contains("ended before"));
}
