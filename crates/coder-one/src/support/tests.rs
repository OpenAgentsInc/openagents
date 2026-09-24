use serde_json::json;

use super::*;
use crate::checks::{Bounds, Covered, Relation, Scenario, Verdict};

fn candidate() -> Candidate {
    Candidate {
        label: "t".to_string(),
        origin: "synthetic".to_string(),
        files: [
            (
                "run.py".to_string(),
                "async def run_tasks(tasks, n):\n    pass\n".to_string(),
            ),
            ("notes.txt".to_string(), "x".repeat(50)),
        ]
        .into_iter()
        .collect(),
        programs: Vec::new(),
        provided: std::collections::BTreeMap::new(),
    }
}

fn report() -> checks::Report {
    let scenario = Scenario {
        id: "cancel.signal.above".to_string(),
        kind: "cancellation".to_string(),
        requirements: vec!["R3".to_string(), "R1".to_string()],
        spans: Vec::new(),
        applies: Vec::new(),
        interface: "from run import run_tasks".to_string(),
        bounds: Bounds {
            seconds: 10,
            processes: 1,
        },
        effects: Vec::new(),
        candidate: "c".to_string(),
        input: "i".to_string(),
        seed: None,
        expected: Relation {
            statement: "Cleanup finishes before the call returns.".to_string(),
            derivation: "d".to_string(),
        },
        params: json!({}),
    };
    let tmp = std::env::temp_dir().display().to_string();
    let mut verdict = Verdict::unavailable("cancel.signal.above", "x");
    verdict.verdict = "failed".to_string();
    verdict.coverage = vec![format!("ran in {tmp}/coder-one-checks-run-1-2/cancel")];
    verdict.observations = vec![
        json!({ "cleaned_up": 0, "log": format!("{tmp}/coder-one-checks-run-1-2/cancel/log.txt") }),
    ];
    let covered = |id: &str, kind: &str, state: &str, scenarios: bool| Covered {
        id: id.to_string(),
        text: format!("requirement {id}"),
        kind: kind.to_string(),
        state: state.to_string(),
        scenarios: if scenarios {
            vec![json!({ "id": "cancel.signal.above" })]
        } else {
            Vec::new()
        },
    };
    checks::Report {
        schema: checks::SCHEMA.to_string(),
        implementation: checks::implementation(),
        candidate: json!({ "digest": "c" }),
        requirements_method: "rule".to_string(),
        ineligible: Vec::new(),
        scenarios: vec![scenario],
        selection: json!({}),
        verdicts: vec![verdict],
        coverage: vec![
            covered("R1", "behavior", "observed", true),
            covered("R2", "deliverable", "unobserved", false),
            covered("R3", "behavior", "contradicted", true),
            covered("R4", "constraint", "unobserved", false),
        ],
        packets: Vec::new(),
    }
}

#[test]
fn evidence_puts_the_contradicted_requirement_first_and_scrubs_scratch_paths() {
    let (evidence, skipped) = evidence(&candidate(), &report(), Params::default());
    let ids: Vec<&str> = evidence.iter().map(|e| e.requirement.id.as_str()).collect();
    assert_eq!(ids, ["R3", "R1", "R2"]);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].id, "R4");
    let r3 = &evidence[0];
    assert_eq!(r3.observations[0].verdict, "failed");
    assert_eq!(evidence[1].observations[0].verdict, "passed");
    let text = serde_json::to_string(r3).unwrap();
    assert!(text.contains("<scratch>/cancel/log.txt"), "{text}");
    assert!(!text.contains("coder-one-checks-run"), "{text}");
    // The interface names run.py, so only it is read.
    assert_eq!(r3.artifact.len(), 1);
    assert_eq!(r3.artifact[0].path, "run.py");
    // No scenario names a file for R2, so it reads every file.
    assert_eq!(evidence[2].artifact.len(), 2);
}

fn judgment(s: Option<f64>, c: Option<f64>) -> Judgment {
    Judgment {
        supports: s,
        contradicts: c,
        how: "recorded".to_string(),
        key: "k".to_string(),
        error: None,
        input_tokens: None,
    }
}

#[test]
fn both_judgments_decide_the_state_and_gaps_leave_it_unresolved() {
    let (evidence, _) = evidence(&candidate(), &report(), Params::default());
    let e = &evidence[0];
    let params = Params {
        supports: 0.5,
        contradicts: 0.5,
        ..Params::default()
    };
    let word = |s, c| establish(&judgment(s, c), e, params).0;
    assert_eq!(word(Some(0.9), Some(0.1)), "supported");
    assert_eq!(word(Some(0.1), Some(0.9)), "contradicted");
    assert_eq!(word(Some(0.9), Some(0.9)), "unresolved");
    assert_eq!(word(Some(0.1), Some(0.1)), "unresolved");
    assert_eq!(word(None, Some(0.9)), "unresolved");
    let mut clipped = e.clone();
    clipped.artifact[0].clipped = true;
    assert_eq!(
        establish(&judgment(Some(0.9), Some(0.1)), &clipped, params).0,
        "unresolved"
    );
}

#[test]
fn a_state_holds_for_its_own_candidate_revision_only() {
    let (evidence, _) = evidence(&candidate(), &report(), Params::default());
    let state = state_of(
        evidence[0].clone(),
        judgment(Some(0.1), Some(0.9)),
        "rev-a",
        Params {
            supports: 0.5,
            contradicts: 0.5,
            ..Params::default()
        },
    );
    assert_eq!(state.state, "contradicted");
    assert!(state.fresh_for("rev-a"));
    assert!(!state.fresh_for("rev-b"));
    assert!(
        state
            .observed
            .iter()
            .any(|o| o.contains("cancel.signal.above failed"))
    );
    assert_eq!(state.checker["scenarios"], json!(["cancel.signal.above"]));
    let report = Report {
        schema: SCHEMA.to_string(),
        implementation: implementation(Params::default()),
        params: Params::default(),
        candidate: "rev-a".to_string(),
        states: vec![state],
        skipped: Vec::new(),
    };
    assert_eq!(report.stale_for("rev-b"), ["R3"]);
    assert!(report.stale_for("rev-a").is_empty());
}

#[tokio::test]
async fn off_leaves_every_requirement_unresolved() {
    let (evidence, skipped) = evidence(&candidate(), &report(), Params::default());
    let task = TaskText {
        title: "t".to_string(),
        instruction: "Write run.py.".to_string(),
    };
    let recorder = Recorder::default();
    let report = judge_evidence(
        &task,
        "rev-a",
        evidence,
        skipped,
        &JevMode::Off,
        &recorder,
        Params::default(),
        None,
    )
    .await;
    assert!(report.states.iter().all(|s| s.state == "unresolved"));
    let components: Vec<String> = crate::record::invocations(&recorder.steps())
        .into_iter()
        .map(|i| i.component)
        .collect();
    // One parent and one request per requirement.
    assert_eq!(
        components.iter().filter(|c| *c == "verify.support").count(),
        4
    );
}

#[test]
fn scrub_leaves_other_paths_alone() {
    assert_eq!(scrub("/app/run.py"), "/app/run.py");
    assert_eq!(
        scrub("\"/home/u/runs/minitask-x-1/checks-scratch/data/in.log\""),
        "\"<scratch>/data/in.log\""
    );
}

#[test]
fn scrub_handles_repeated_temp_separators_without_changing_other_text() {
    let tmp = std::env::temp_dir();
    let tmp = tmp.to_str().unwrap().trim_end_matches('/');
    for separator in ["/", "//", "///"] {
        let other = format!("{tmp}{separator}other/log.txt");
        let text = format!(
            "unchanged {other}; ran in '{tmp}{separator}coder-one-checks-1/cancel/log.txt' \
             then \"{tmp}{separator}coder-one-checks-2/data/in.log\"; /app/run.py"
        );
        let expected = format!(
            "unchanged {other}; ran in '<scratch>/cancel/log.txt' \
             then \"<scratch>/data/in.log\"; /app/run.py"
        );
        assert_eq!(scrub(&text), expected);
        assert_eq!(scrub(&expected), expected);
    }
}

#[tokio::test]
async fn the_checked_in_cutoffs_are_fitted_on_development_fixtures_only() {
    let dirs = cli::labeled(&crate::component::default_fixtures());
    assert_eq!(
        dirs.len(),
        18,
        "nine development and nine evaluation fixtures"
    );
    let (suite, evaluation) = cli::evaluate(
        &dirs,
        &crate::component::JevChoice::Recorded,
        &Recorder::default(),
        false,
    )
    .await
    .unwrap();
    assert!(suite.runs.iter().all(|run| run.jev.get("miss").is_none()));
    assert_eq!(
        evaluation["checked_in"]["matches"],
        json!(true),
        "{evaluation:#}"
    );
    // The evaluation split never moves the fit: flip every evaluation
    // label and refit.
    let outputs: Vec<(String, serde_json::Value)> = suite
        .runs
        .iter()
        .map(|run| (run.fixture.clone(), run.output.clone()))
        .collect();
    let mut rows = fit::rows(&outputs);
    for row in rows.iter_mut().filter(|r| r.split == "evaluation") {
        for triple in &mut row.triples {
            triple.met = !triple.met;
        }
    }
    assert_eq!(fit::evaluate(&rows)["cutoffs"], evaluation["cutoffs"]);
    // On the recovered v3 candidates, the broad "done" judgment at 0.5
    // accepts every failure; the pair beside the checks accepts fewer.
    let v3 = &evaluation["evaluation"]["candidates_by_rule"];
    assert_eq!(v3["done_at_0.5"]["false_accepts"], json!(5), "{v3:#}");
    assert!(
        v3["support_and_checks"]["false_accepts"].as_u64().unwrap()
            < v3["done_at_0.5"]["false_accepts"].as_u64().unwrap(),
        "{v3:#}"
    );
}
