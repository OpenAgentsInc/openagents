use std::path::PathBuf;

use arc_core::{ArcAction, ArcScorecardMetadata, ArcTaskId};
use arc_engine::{ArcEngine, load_game_package};
use arc_ml::{
    ArcInteractivePracticeAttempt, ArcInteractivePracticeCase, ArcInteractivePracticeSuite,
    ArcMlDataProvenance, ArcMlEvalError, estimate_pass_at_k, evaluate_interactive_practice_suite,
};
use psionic_eval::BenchmarkPackageKey;
use serde::Deserialize;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct PracticeManifest {
    schema_version: u16,
    bounded_scope: String,
    package_key: BenchmarkPackageKey,
    k_values: Vec<u32>,
    cases: Vec<PracticeCaseFixture>,
    expected_aggregate: PracticeAggregateExpectation,
}

#[derive(Debug, Deserialize)]
struct PracticeCaseFixture {
    id: String,
    task_id: ArcTaskId,
    package_path: String,
    metadata: ArcScorecardMetadata,
    baseline_actions: Vec<u32>,
    attempts: Vec<PracticeAttemptFixture>,
    expected: PracticeCaseExpectation,
}

#[derive(Debug, Deserialize)]
struct PracticeAttemptFixture {
    id: String,
    actions: Vec<ArcAction>,
}

#[derive(Debug, Deserialize)]
struct PracticeCaseExpectation {
    successful_attempts: u32,
    best_score_bps: u32,
    pass_at_k_bps: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct PracticeAggregateExpectation {
    total_cases: u32,
    total_attempts: u32,
    successful_attempts: u32,
    mean_best_score_bps: u32,
    pass_at_k_bps: Vec<u32>,
}

#[test]
fn interactive_practice_suite_scores_synthetic_attempts_and_aggregates_pass_at_k() {
    let manifest: PracticeManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("interactive_practice_suite.json"))
            .expect("practice manifest should load"),
    )
    .expect("practice manifest should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(
        manifest
            .bounded_scope
            .contains("synthetic ARC-AGI-3-style attempts")
    );

    let suite = build_suite(&manifest);
    let report = evaluate_interactive_practice_suite(&suite).expect("practice suite should score");

    assert_eq!(report.total_cases, manifest.expected_aggregate.total_cases);
    assert_eq!(
        report.total_attempts,
        manifest.expected_aggregate.total_attempts
    );
    assert_eq!(
        report.successful_attempts,
        manifest.expected_aggregate.successful_attempts
    );
    assert_eq!(
        report.mean_best_score_bps,
        manifest.expected_aggregate.mean_best_score_bps
    );
    assert_eq!(
        report
            .pass_at_k
            .iter()
            .map(|estimate| estimate.mean_pass_rate_bps)
            .collect::<Vec<_>>(),
        manifest.expected_aggregate.pass_at_k_bps
    );
    assert!(!report.summary_digest.is_empty());

    for (case_report, case_fixture) in report.cases.iter().zip(&manifest.cases) {
        assert_eq!(
            case_report.successful_attempts, case_fixture.expected.successful_attempts,
            "{}",
            case_fixture.id
        );
        assert_eq!(
            case_report.best_score_bps, case_fixture.expected.best_score_bps,
            "{}",
            case_fixture.id
        );
        assert_eq!(
            case_report
                .pass_at_k
                .iter()
                .map(|estimate| estimate.pass_rate_bps)
                .collect::<Vec<_>>(),
            case_fixture.expected.pass_at_k_bps,
            "{}",
            case_fixture.id
        );
    }
}

#[test]
fn pass_at_k_estimator_matches_reference_probability() {
    let pass_at_1 = estimate_pass_at_k(3, 1, 1).expect("pass@1 should estimate");
    let pass_at_2 = estimate_pass_at_k(3, 1, 2).expect("pass@2 should estimate");
    let pass_at_4 = estimate_pass_at_k(3, 1, 4).expect("pass@4 should clamp");

    assert!((pass_at_1 - (1.0 / 3.0)).abs() < 1e-9);
    assert!((pass_at_2 - (2.0 / 3.0)).abs() < 1e-9);
    assert!((pass_at_4 - 1.0).abs() < 1e-9);
}

#[test]
fn interactive_practice_suite_refuses_attempt_task_mismatch() {
    let manifest: PracticeManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("interactive_practice_suite.json"))
            .expect("practice manifest should load"),
    )
    .expect("practice manifest should deserialize");
    let mut suite = build_suite(&manifest);
    suite.cases[0].attempts[0].recording.task_id =
        ArcTaskId::new("wrong-task").expect("task id should validate");

    let error =
        evaluate_interactive_practice_suite(&suite).expect_err("task mismatch should refuse");

    match error {
        ArcMlEvalError::AttemptTaskMismatch {
            case_id,
            attempt_id,
            expected,
            actual,
        } => {
            assert_eq!(case_id, "demo-practice-mixed");
            assert_eq!(attempt_id, "perfect-win");
            assert_eq!(expected.as_str(), "arc-engine-demo");
            assert_eq!(actual.as_str(), "wrong-task");
        }
        other => panic!("unexpected evaluator error: {other}"),
    }
}

fn build_suite(manifest: &PracticeManifest) -> ArcInteractivePracticeSuite {
    ArcInteractivePracticeSuite {
        suite_id: String::from("arc-ml-synthetic-practice-v1"),
        bounded_scope: manifest.bounded_scope.clone(),
        package_key: manifest.package_key.clone(),
        data_provenance: ArcMlDataProvenance::SyntheticArcAgi3Practice,
        k_values: manifest.k_values.clone(),
        cases: manifest.cases.iter().map(build_case).collect::<Vec<_>>(),
    }
}

fn build_case(case: &PracticeCaseFixture) -> ArcInteractivePracticeCase {
    ArcInteractivePracticeCase {
        case_id: case.id.clone(),
        task_id: case.task_id.clone(),
        metadata: case.metadata.clone(),
        baseline_actions: case.baseline_actions.clone(),
        attempts: case
            .attempts
            .iter()
            .map(|attempt| ArcInteractivePracticeAttempt {
                attempt_id: attempt.id.clone(),
                recording: replay_actions(&case.package_path, attempt.actions.as_slice()),
            })
            .collect::<Vec<_>>(),
    }
}

fn replay_actions(package_path: &str, actions: &[ArcAction]) -> arc_core::ArcRecording {
    let package =
        load_game_package(resolve_package_path(package_path)).expect("fixture package should load");
    ArcEngine::replay(package, actions).expect("fixture actions should replay")
}

fn resolve_package_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}
