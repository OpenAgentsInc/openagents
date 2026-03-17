use arc_benchmark::{
    ArcBenchmarkHygieneError, ArcConceptSliceSummary, ArcEvaluationVisibility,
    ArcPublicEvalArtifactManifest, ArcStaticHygieneSuite, ArcStaticTaskSubmission,
    ArcVisibilitySummary, run_static_hygiene_suite, validate_public_eval_artifact_manifest,
};
use arc_core::ArcTaskId;
use serde::Deserialize;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn policy_fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/policy/public_eval_hygiene")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct HygieneManifest {
    schema_version: u16,
    bounded_scope: String,
    suite: ArcStaticHygieneSuite,
    submissions: Vec<ArcStaticTaskSubmission>,
    artifact_manifests: Vec<ArcPublicEvalArtifactManifest>,
    expected: HygieneExpected,
}

#[derive(Debug, Deserialize)]
struct HygieneExpected {
    total_tasks: u32,
    exact_match_tasks: u32,
    missing_submission_tasks: usize,
    internal_holdout_exact_match_tasks: u32,
    synthetic_regression_exact_match_tasks: u32,
    public_eval_exact_match_tasks: u32,
    augmentation_exact_match_tasks: u32,
    color_fill_exact_match_tasks: u32,
    template_reuse_exact_match_tasks: u32,
}

#[test]
fn static_hygiene_fixture_reports_holdout_synthetic_and_concept_slices() {
    let manifest: HygieneManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("static_hygiene_manifest.json"))
            .expect("static hygiene fixture should load"),
    )
    .expect("static hygiene fixture should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("static hygiene harness"));

    let report = run_static_hygiene_suite(
        &manifest.suite,
        &manifest.submissions,
        &manifest.artifact_manifests,
    )
    .expect("hygiene suite should run");

    assert_eq!(
        report.overall_summary.total_tasks,
        manifest.expected.total_tasks
    );
    assert_eq!(
        report.overall_summary.exact_match_tasks,
        manifest.expected.exact_match_tasks
    );
    assert_eq!(
        report.missing_submission_tasks.len(),
        manifest.expected.missing_submission_tasks
    );
    assert_eq!(
        report.missing_submission_tasks,
        vec![ArcTaskId::new("public_eval_template").expect("task id")]
    );
    assert!((report.overall_summary.mean_task_score - (2.0 / 3.0)).abs() < 1e-6);

    let internal = visibility_summary(
        &report.visibility_summaries,
        ArcEvaluationVisibility::InternalHoldout,
    );
    assert_eq!(
        internal.exact_match_tasks,
        manifest.expected.internal_holdout_exact_match_tasks
    );
    let synthetic = visibility_summary(
        &report.visibility_summaries,
        ArcEvaluationVisibility::SyntheticRegression,
    );
    assert_eq!(
        synthetic.exact_match_tasks,
        manifest.expected.synthetic_regression_exact_match_tasks
    );
    let public_eval = visibility_summary(
        &report.visibility_summaries,
        ArcEvaluationVisibility::PublicEval,
    );
    assert_eq!(
        public_eval.exact_match_tasks,
        manifest.expected.public_eval_exact_match_tasks
    );

    assert_eq!(
        concept_summary(&report.concept_slice_summaries, "augmentation").exact_match_tasks,
        manifest.expected.augmentation_exact_match_tasks
    );
    assert_eq!(
        concept_summary(&report.concept_slice_summaries, "color_fill").exact_match_tasks,
        manifest.expected.color_fill_exact_match_tasks
    );
    assert_eq!(
        concept_summary(&report.concept_slice_summaries, "template_reuse").exact_match_tasks,
        manifest.expected.template_reuse_exact_match_tasks
    );
    assert!(
        report
            .public_eval_validations
            .iter()
            .all(|validation| validation.valid)
    );
}

#[test]
fn public_eval_policy_fixtures_match_the_repo_hygiene_contract() {
    let valid: ArcPublicEvalArtifactManifest = serde_json::from_str(
        &std::fs::read_to_string(policy_fixture_path("valid_public_eval_non_regression.json"))
            .expect("valid fixture should load"),
    )
    .expect("valid fixture should deserialize");
    let valid_result = validate_public_eval_artifact_manifest(&valid);
    assert!(valid_result.valid);

    for name in [
        "invalid_public_eval_optimization.json",
        "invalid_public_eval_training_feed.json",
    ] {
        let invalid: ArcPublicEvalArtifactManifest = serde_json::from_str(
            &std::fs::read_to_string(policy_fixture_path(name))
                .expect("invalid fixture should load"),
        )
        .expect("invalid fixture should deserialize");
        let invalid_result = validate_public_eval_artifact_manifest(&invalid);
        assert!(!invalid_result.valid, "{name}");
        assert!(!invalid_result.violations.is_empty(), "{name}");
    }
}

#[test]
fn hygiene_suite_refuses_public_eval_leakage_into_synthetic_regression() {
    let mut suite: ArcStaticHygieneSuite = serde_json::from_str(
        r#"{
          "suite_id": "leakage",
          "benchmark": "arc_agi1",
          "cases": [{
            "task": {
              "id": "leaky_case",
              "train": [{
                "input": { "width": 1, "height": 1, "cells": [1] },
                "output": { "width": 1, "height": 1, "cells": [1] }
              }],
              "test": [{ "width": 1, "height": 1, "cells": [2] }]
            },
            "answer_key": {
              "task_id": "leaky_case",
              "outputs": [{ "width": 1, "height": 1, "cells": [2] }]
            },
            "visibility": "synthetic_regression",
            "synthetic_derivation": "from_public_eval",
            "concept_slices": ["leakage"]
          }]
        }"#,
    )
    .expect("suite should deserialize");
    let error = run_static_hygiene_suite(&suite, &[], &[]).expect_err("suite should refuse");
    match error {
        ArcBenchmarkHygieneError::SyntheticRegressionDerivationMismatch { .. }
        | ArcBenchmarkHygieneError::PublicEvalLeakage { .. } => {}
        other => panic!("unexpected hygiene error: {other}"),
    }

    suite.cases[0].visibility = ArcEvaluationVisibility::InternalHoldout;
    suite.cases[0].synthetic_derivation =
        arc_benchmark::ArcSyntheticDerivation::FromInternalHoldout;
    let error = run_static_hygiene_suite(&suite, &[], &[]).expect_err("suite should refuse");
    match error {
        ArcBenchmarkHygieneError::HiddenHoldoutMustBeRaw { .. } => {}
        other => panic!("unexpected hygiene error: {other}"),
    }
}

fn visibility_summary(
    summaries: &[ArcVisibilitySummary],
    visibility: ArcEvaluationVisibility,
) -> ArcVisibilitySummary {
    summaries
        .iter()
        .find(|summary| summary.visibility == visibility)
        .cloned()
        .expect("visibility summary should exist")
}

fn concept_summary(summaries: &[ArcConceptSliceSummary], concept: &str) -> ArcConceptSliceSummary {
    summaries
        .iter()
        .find(|summary| summary.concept_slice == concept)
        .cloned()
        .expect("concept summary should exist")
}
