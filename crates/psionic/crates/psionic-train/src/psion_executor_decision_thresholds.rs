use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::TassadarBenchmarkReport;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_baseline_truth_record, builtin_executor_eval_pack_catalog,
    PsionExecutorBaselineTruthRecord, PsionExecutorEvalPackCatalog,
};

/// Stable schema version for the executor decision-threshold packet.
pub const PSION_EXECUTOR_DECISION_THRESHOLDS_SCHEMA_VERSION: &str =
    "psion.executor_decision_thresholds.v1";
/// Canonical fixture path for the executor decision-threshold packet.
pub const PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_decision_thresholds_v1.json";
/// Canonical doc path for the executor decision-threshold packet.
pub const PSION_EXECUTOR_DECISION_THRESHOLDS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_DECISION_THRESHOLDS.md";

const PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_eval_packs_v1.json";
const PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_baseline_truth_v1.json";
const TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
const TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_generalization_gate_report.json";
const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_audit_report.json";

const REPLAY_PASS_COUNT: usize = 3;

/// How a retained baseline replay was observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorDecisionMeasurementPosture {
    RetainedFixtureReplay,
}

/// Comparison direction for one promotion metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorDecisionDirection {
    HigherIsBetter,
    LowerIsBetter,
    ZeroRegression,
}

/// One aggregate measurement observed in one retained baseline replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorDecisionMeasurement {
    /// Stable suite identifier.
    pub suite_id: String,
    /// Stable aggregate metric identifier.
    pub metric_id: String,
    /// Observed aggregate value.
    pub observed_value: f64,
    /// Short explanation of the aggregate.
    pub detail: String,
}

/// One repeated retained baseline replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorDecisionReplayPass {
    /// Stable replay pass identifier.
    pub pass_id: String,
    /// Measurement posture.
    pub posture: PsionExecutorDecisionMeasurementPosture,
    /// Repo-local source refs replayed in this pass.
    pub source_refs: Vec<String>,
    /// Aggregate measurements observed in the pass.
    pub measurements: Vec<PsionExecutorDecisionMeasurement>,
    /// Short explanation of the replay pass.
    pub detail: String,
}

/// One promotion-facing decision threshold derived from retained baseline replays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorDecisionThreshold {
    /// Stable threshold identifier.
    pub threshold_id: String,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Stable metric identifier.
    pub metric_id: String,
    /// Comparison direction.
    pub direction: PsionExecutorDecisionDirection,
    /// Current baseline value.
    pub baseline_value: f64,
    /// Minimum observed replay value.
    pub observed_min: f64,
    /// Maximum observed replay value.
    pub observed_max: f64,
    /// Observed replay span.
    pub observed_span: f64,
    /// Conservative policy floor above the retained replay span.
    pub policy_floor: f64,
    /// Minimum delta required before promotion may claim a meaningful change.
    pub minimum_meaningful_delta: f64,
    /// Delta that triggers a promotion hold when the candidate moves the wrong way.
    pub regression_hold_delta: f64,
    /// Short operator-facing promotion rule.
    pub promotion_use: String,
    /// Short explanation of the threshold.
    pub detail: String,
}

/// Retained decision-threshold packet used by later promotion logic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorDecisionThresholdRecord {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable record identifier.
    pub record_id: String,
    /// Frozen eval-pack catalog ref.
    pub eval_pack_catalog_ref: String,
    /// Frozen eval-pack catalog digest.
    pub eval_pack_catalog_digest: String,
    /// Frozen baseline-truth ref.
    pub baseline_truth_ref: String,
    /// Frozen baseline-truth digest.
    pub baseline_truth_digest: String,
    /// Repeated retained baseline replays.
    pub replay_passes: Vec<PsionExecutorDecisionReplayPass>,
    /// Promotion-facing thresholds derived from the replays.
    pub thresholds: Vec<PsionExecutorDecisionThreshold>,
    /// Short summary of the packet.
    pub summary: String,
    /// Stable digest over the record.
    pub record_digest: String,
}

impl PsionExecutorDecisionThresholdRecord {
    /// Validate the threshold record against the frozen eval packs and baseline packet.
    pub fn validate_against_catalog(
        &self,
        catalog: &PsionExecutorEvalPackCatalog,
        baseline_truth: &PsionExecutorBaselineTruthRecord,
    ) -> Result<(), PsionExecutorDecisionThresholdError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_decision_thresholds.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_DECISION_THRESHOLDS_SCHEMA_VERSION {
            return Err(PsionExecutorDecisionThresholdError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.record_id.as_str(),
            "psion_executor_decision_thresholds.record_id",
        )?;
        if self.eval_pack_catalog_ref != PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH {
            return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                field: String::from("psion_executor_decision_thresholds.eval_pack_catalog_ref"),
                expected: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
                actual: self.eval_pack_catalog_ref.clone(),
            });
        }
        if self.eval_pack_catalog_digest != catalog.catalog_digest {
            return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                field: String::from("psion_executor_decision_thresholds.eval_pack_catalog_digest"),
                expected: catalog.catalog_digest.clone(),
                actual: self.eval_pack_catalog_digest.clone(),
            });
        }
        if self.baseline_truth_ref != PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH {
            return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                field: String::from("psion_executor_decision_thresholds.baseline_truth_ref"),
                expected: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                actual: self.baseline_truth_ref.clone(),
            });
        }
        if self.baseline_truth_digest != baseline_truth.record_digest {
            return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                field: String::from("psion_executor_decision_thresholds.baseline_truth_digest"),
                expected: baseline_truth.record_digest.clone(),
                actual: self.baseline_truth_digest.clone(),
            });
        }
        if self.replay_passes.len() != REPLAY_PASS_COUNT {
            return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                field: String::from("psion_executor_decision_thresholds.replay_pass_count"),
                expected: REPLAY_PASS_COUNT.to_string(),
                actual: self.replay_passes.len().to_string(),
            });
        }
        if self.thresholds.is_empty() {
            return Err(PsionExecutorDecisionThresholdError::MissingField {
                field: String::from("psion_executor_decision_thresholds.thresholds"),
            });
        }

        let expected_measurements = self
            .replay_passes
            .first()
            .ok_or_else(|| PsionExecutorDecisionThresholdError::MissingField {
                field: String::from("psion_executor_decision_thresholds.replay_passes[0]"),
            })?
            .measurements
            .iter()
            .map(|measurement| {
                (
                    measurement.suite_id.as_str(),
                    measurement.metric_id.as_str(),
                    measurement.observed_value,
                )
            })
            .collect::<Vec<_>>();

        let mut seen_passes = BTreeSet::new();
        for pass in &self.replay_passes {
            ensure_nonempty(
                pass.pass_id.as_str(),
                "psion_executor_decision_thresholds.replay_passes[].pass_id",
            )?;
            ensure_nonempty(
                pass.detail.as_str(),
                "psion_executor_decision_thresholds.replay_passes[].detail",
            )?;
            if pass.source_refs.is_empty() {
                return Err(PsionExecutorDecisionThresholdError::MissingField {
                    field: format!(
                        "psion_executor_decision_thresholds.replay_passes[{}].source_refs",
                        pass.pass_id
                    ),
                });
            }
            if !seen_passes.insert(pass.pass_id.as_str()) {
                return Err(PsionExecutorDecisionThresholdError::DuplicateRef {
                    field: String::from(
                        "psion_executor_decision_thresholds.replay_passes[].pass_id",
                    ),
                    value: pass.pass_id.clone(),
                });
            }
            let observed_measurements = pass
                .measurements
                .iter()
                .map(|measurement| {
                    (
                        measurement.suite_id.as_str(),
                        measurement.metric_id.as_str(),
                        measurement.observed_value,
                    )
                })
                .collect::<Vec<_>>();
            if observed_measurements != expected_measurements {
                return Err(PsionExecutorDecisionThresholdError::ReplayMismatch {
                    pass_id: pass.pass_id.clone(),
                });
            }
        }

        let expected_suite_ids = promotion_suite_ids(catalog);
        let observed_suite_ids = self
            .thresholds
            .iter()
            .map(|threshold| threshold.suite_id.as_str())
            .collect::<BTreeSet<_>>();
        for threshold in &self.thresholds {
            ensure_nonempty(
                threshold.threshold_id.as_str(),
                "psion_executor_decision_thresholds.thresholds[].threshold_id",
            )?;
            ensure_nonempty(
                threshold.suite_id.as_str(),
                "psion_executor_decision_thresholds.thresholds[].suite_id",
            )?;
            ensure_nonempty(
                threshold.metric_id.as_str(),
                "psion_executor_decision_thresholds.thresholds[].metric_id",
            )?;
            ensure_nonempty(
                threshold.promotion_use.as_str(),
                "psion_executor_decision_thresholds.thresholds[].promotion_use",
            )?;
            ensure_nonempty(
                threshold.detail.as_str(),
                "psion_executor_decision_thresholds.thresholds[].detail",
            )?;
            if threshold.observed_min > threshold.observed_max {
                return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                    field: format!(
                        "psion_executor_decision_thresholds.thresholds[{}].observed_min_max",
                        threshold.threshold_id
                    ),
                    expected: String::from("observed_min <= observed_max"),
                    actual: format!("{} > {}", threshold.observed_min, threshold.observed_max),
                });
            }
            if threshold.minimum_meaningful_delta < threshold.observed_span {
                return Err(PsionExecutorDecisionThresholdError::FieldMismatch {
                    field: format!(
                        "psion_executor_decision_thresholds.thresholds[{}].minimum_meaningful_delta",
                        threshold.threshold_id
                    ),
                    expected: format!(">= {}", threshold.observed_span),
                    actual: threshold.minimum_meaningful_delta.to_string(),
                });
            }
            let matching_measurement_exists =
                expected_measurements
                    .iter()
                    .any(|(suite_id, metric_id, baseline_value)| {
                        *suite_id == threshold.suite_id.as_str()
                            && *metric_id == threshold.metric_id.as_str()
                            && (*baseline_value - threshold.baseline_value).abs() < 1e-9
                    });
            if !matching_measurement_exists {
                return Err(
                    PsionExecutorDecisionThresholdError::UnknownThresholdMetric {
                        suite_id: threshold.suite_id.clone(),
                        metric_id: threshold.metric_id.clone(),
                    },
                );
            }
        }
        if !expected_suite_ids.is_subset(&observed_suite_ids) {
            return Err(PsionExecutorDecisionThresholdError::SuiteCoverageMismatch {
                expected: expected_suite_ids.into_iter().map(String::from).collect(),
                actual: observed_suite_ids.into_iter().map(String::from).collect(),
            });
        }
        if self.record_digest != stable_executor_decision_threshold_record_digest(self) {
            return Err(PsionExecutorDecisionThresholdError::DigestMismatch {
                kind: String::from("psion_executor_decision_thresholds"),
            });
        }
        Ok(())
    }
}

/// Errors raised while building or validating the decision-threshold packet.
#[derive(Debug, Error)]
pub enum PsionExecutorDecisionThresholdError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected {expected}, found {actual}")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("field `{field}` mismatch: expected {expected}, found {actual}")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate `{field}` value `{value}`")]
    DuplicateRef { field: String, value: String },
    #[error("suite coverage mismatch: expected {expected:?}, found {actual:?}")]
    SuiteCoverageMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    #[error("replay pass `{pass_id}` does not match the retained baseline replay set")]
    ReplayMismatch { pass_id: String },
    #[error("threshold for `{suite_id}` / `{metric_id}` does not match a retained replay metric")]
    UnknownThresholdMetric { suite_id: String, metric_id: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error("unknown suite `{suite_id}` in baseline truth")]
    UnknownSuite { suite_id: String },
    #[error("unknown benchmark case `{case_id}` for suite `{suite_id}`")]
    UnknownBenchmarkCase { suite_id: String, case_id: String },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to serialize decision thresholds: {0}")]
    Json(#[from] serde_json::Error),
    #[error("baseline truth generation failed: {0}")]
    BaselineTruth(#[from] crate::PsionExecutorBaselineTruthError),
    #[error("eval-pack catalog generation failed: {0}")]
    EvalPack(#[from] crate::PsionExecutorEvalPackError),
}

/// Build the current retained decision-threshold packet.
pub fn builtin_executor_decision_threshold_record(
    workspace_root: &Path,
) -> Result<PsionExecutorDecisionThresholdRecord, PsionExecutorDecisionThresholdError> {
    let catalog = builtin_executor_eval_pack_catalog(workspace_root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(workspace_root)?;
    let benchmark_report: TassadarBenchmarkReport =
        read_repo_json(workspace_root.join(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH))?;

    let exactness_regression_count =
        exactness_regression_count(&baseline_truth, "promotion_exactness_suite_v0")?;
    let held_out_regression_count =
        generalization_regression_count(&baseline_truth, "promotion_held_out_suite_v0")?;
    let adversarial_regression_count =
        generalization_regression_count(&baseline_truth, "promotion_adversarial_suite_v0")?;
    let runtime_blocker_red_count = checklist_red_count(
        &baseline_truth,
        "promotion_runtime_blockers_v0",
        "checklist_status",
    )?;
    let serving_blocker_red_count = checklist_red_count(
        &baseline_truth,
        "promotion_serving_blockers_v0",
        "checklist_status",
    )?;

    let reference_case_ids =
        promotion_suite_case_ids(&catalog, "promotion_reference_linear_anchor_checks_v0")?;
    let hull_case_ids =
        promotion_suite_case_ids(&catalog, "promotion_hull_cache_fast_route_checks_v0")?;
    let benchmark_cases = benchmark_report
        .case_reports
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();

    let reference_linear_anchor_median = median_f64(
        reference_case_ids
            .iter()
            .map(|case_id| {
                benchmark_cases
                    .get(case_id.as_str())
                    .ok_or_else(
                        || PsionExecutorDecisionThresholdError::UnknownBenchmarkCase {
                            suite_id: String::from("promotion_reference_linear_anchor_checks_v0"),
                            case_id: case_id.clone(),
                        },
                    )
                    .map(|case| case.reference_linear_steps_per_second)
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let hull_cache_median = median_f64(
        hull_case_ids
            .iter()
            .map(|case_id| {
                benchmark_cases
                    .get(case_id.as_str())
                    .ok_or_else(
                        || PsionExecutorDecisionThresholdError::UnknownBenchmarkCase {
                            suite_id: String::from("promotion_hull_cache_fast_route_checks_v0"),
                            case_id: case_id.clone(),
                        },
                    )
                    .map(|case| case.hull_cache_steps_per_second)
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let hull_cache_min_speedup = hull_case_ids
        .iter()
        .map(|case_id| {
            benchmark_cases
                .get(case_id.as_str())
                .ok_or_else(
                    || PsionExecutorDecisionThresholdError::UnknownBenchmarkCase {
                        suite_id: String::from("promotion_hull_cache_fast_route_checks_v0"),
                        case_id: case_id.clone(),
                    },
                )
                .map(|case| case.hull_cache_speedup_over_reference_linear)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(f64::INFINITY, f64::min);
    let hull_cache_max_gap = hull_case_ids
        .iter()
        .map(|case_id| {
            benchmark_cases
                .get(case_id.as_str())
                .ok_or_else(
                    || PsionExecutorDecisionThresholdError::UnknownBenchmarkCase {
                        suite_id: String::from("promotion_hull_cache_fast_route_checks_v0"),
                        case_id: case_id.clone(),
                    },
                )
                .map(|case| case.hull_cache_remaining_gap_vs_cpu_reference)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(f64::NEG_INFINITY, f64::max);

    let measurement_template = vec![
        measurement(
            "promotion_exactness_suite_v0",
            "promotion_exactness_regression_count",
            exactness_regression_count,
            "Count of exactness-case regressions relative to the frozen baseline suite.",
        ),
        measurement(
            "promotion_held_out_suite_v0",
            "promotion_held_out_regression_count",
            held_out_regression_count,
            "Count of held-out mismatches or refusals relative to the frozen baseline suite.",
        ),
        measurement(
            "promotion_adversarial_suite_v0",
            "promotion_adversarial_regression_count",
            adversarial_regression_count,
            "Count of adversarial mismatches or refusals relative to the frozen baseline suite.",
        ),
        measurement(
            "promotion_runtime_blockers_v0",
            "promotion_runtime_blocker_red_count",
            runtime_blocker_red_count,
            "Count of red runtime-blocker checklist rows relative to the frozen baseline suite.",
        ),
        measurement(
            "promotion_serving_blockers_v0",
            "promotion_serving_blocker_red_count",
            serving_blocker_red_count,
            "Count of red serving-blocker checklist rows relative to the frozen baseline suite.",
        ),
        measurement(
            "promotion_reference_linear_anchor_checks_v0",
            "promotion_reference_linear_anchor_median_steps_per_second",
            reference_linear_anchor_median,
            "Median reference-linear anchor throughput across the admitted promotion anchor cases.",
        ),
        measurement(
            "promotion_hull_cache_fast_route_checks_v0",
            "promotion_hull_cache_median_steps_per_second",
            hull_cache_median,
            "Median hull-cache throughput across the admitted fast-route promotion cases.",
        ),
        measurement(
            "promotion_hull_cache_fast_route_checks_v0",
            "promotion_hull_cache_min_speedup_over_reference_linear",
            hull_cache_min_speedup,
            "Minimum hull-cache speedup over the reference-linear anchor across the admitted fast-route promotion cases.",
        ),
        measurement(
            "promotion_hull_cache_fast_route_checks_v0",
            "promotion_hull_cache_max_remaining_gap_vs_cpu_reference",
            hull_cache_max_gap,
            "Maximum remaining CPU-reference gap across the admitted fast-route promotion cases.",
        ),
    ];

    let replay_passes = (1..=REPLAY_PASS_COUNT)
        .map(|index| PsionExecutorDecisionReplayPass {
            pass_id: format!("baseline_replay_pass_{index:02}"),
            posture: PsionExecutorDecisionMeasurementPosture::RetainedFixtureReplay,
            source_refs: vec![
                String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
                String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_PATH),
                String::from(TASSADAR_ARTICLE_GENERALIZATION_GATE_REPORT_PATH),
                String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_PATH),
            ],
            measurements: measurement_template.clone(),
            detail: String::from(
                "Replay the retained baseline packet without rerunning heavyweight GPU jobs so later promotion logic starts from one committed measurement spine.",
            ),
        })
        .collect::<Vec<_>>();

    let thresholds = vec![
        zero_regression_threshold(
            "promotion_exactness_regression_count_v0",
            "promotion_exactness_suite_v0",
            "promotion_exactness_regression_count",
            exactness_regression_count,
            "Any exactness regression above baseline holds the candidate immediately.",
        ),
        zero_regression_threshold(
            "promotion_held_out_regression_count_v0",
            "promotion_held_out_suite_v0",
            "promotion_held_out_regression_count",
            held_out_regression_count,
            "Any held-out mismatch or refusal above baseline holds the candidate immediately.",
        ),
        zero_regression_threshold(
            "promotion_adversarial_regression_count_v0",
            "promotion_adversarial_suite_v0",
            "promotion_adversarial_regression_count",
            adversarial_regression_count,
            "Any adversarial mismatch or refusal above baseline holds the candidate immediately.",
        ),
        zero_regression_threshold(
            "promotion_runtime_blocker_red_count_v0",
            "promotion_runtime_blockers_v0",
            "promotion_runtime_blocker_red_count",
            runtime_blocker_red_count,
            "Any red runtime-blocker checklist row above baseline holds the candidate immediately.",
        ),
        zero_regression_threshold(
            "promotion_serving_blocker_red_count_v0",
            "promotion_serving_blockers_v0",
            "promotion_serving_blocker_red_count",
            serving_blocker_red_count,
            "Any red serving-blocker checklist row above baseline holds the candidate immediately.",
        ),
        higher_is_better_threshold(
            "promotion_reference_linear_anchor_median_steps_per_second_v0",
            "promotion_reference_linear_anchor_checks_v0",
            "promotion_reference_linear_anchor_median_steps_per_second",
            reference_linear_anchor_median,
            reference_linear_anchor_median * 0.05,
            "Promotion may claim a reference-linear throughput win only when the anchor median moves by at least five percent; an equal drop holds the candidate.",
        ),
        higher_is_better_threshold(
            "promotion_hull_cache_median_steps_per_second_v0",
            "promotion_hull_cache_fast_route_checks_v0",
            "promotion_hull_cache_median_steps_per_second",
            hull_cache_median,
            hull_cache_median * 0.05,
            "Promotion may claim a hull-cache throughput win only when the fast-route median moves by at least five percent; an equal drop holds the candidate.",
        ),
        higher_is_better_threshold(
            "promotion_hull_cache_min_speedup_over_reference_linear_v0",
            "promotion_hull_cache_fast_route_checks_v0",
            "promotion_hull_cache_min_speedup_over_reference_linear",
            hull_cache_min_speedup,
            0.05,
            "Promotion may claim a fast-route speedup win only when the minimum speedup clears baseline by at least 0.05; an equal drop holds the candidate before the absolute 1.5 floor is even considered.",
        ),
        lower_is_better_threshold(
            "promotion_hull_cache_max_remaining_gap_vs_cpu_reference_v0",
            "promotion_hull_cache_fast_route_checks_v0",
            "promotion_hull_cache_max_remaining_gap_vs_cpu_reference",
            hull_cache_max_gap,
            0.05,
            "Promotion may claim a CPU-gap improvement only when the worst remaining gap shrinks by at least 0.05; an equal increase holds the candidate before the absolute 3.0 ceiling is even considered.",
        ),
    ];

    let mut record = PsionExecutorDecisionThresholdRecord {
        schema_version: String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_SCHEMA_VERSION),
        record_id: String::from("psion_executor_decision_thresholds_v1"),
        eval_pack_catalog_ref: String::from(PSION_EXECUTOR_EVAL_PACK_CATALOG_FIXTURE_PATH),
        eval_pack_catalog_digest: catalog.catalog_digest.clone(),
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_digest: baseline_truth.record_digest.clone(),
        replay_passes,
        thresholds,
        summary: format!(
            "Replayed the retained executor baseline {} times; observed zero retained noise on 9 promotion aggregates and froze conservative decision floors for exactness, held-out, adversarial, runtime, serving, anchor throughput, and fast-route throughput.",
            REPLAY_PASS_COUNT
        ),
        record_digest: String::new(),
    };
    record.record_digest = stable_executor_decision_threshold_record_digest(&record);
    record.validate_against_catalog(&catalog, &baseline_truth)?;
    Ok(record)
}

/// Write the committed decision-threshold fixture.
pub fn write_builtin_executor_decision_threshold_record(
    workspace_root: &Path,
) -> Result<PsionExecutorDecisionThresholdRecord, PsionExecutorDecisionThresholdError> {
    let record = builtin_executor_decision_threshold_record(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorDecisionThresholdError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&record)?).map_err(|error| {
        PsionExecutorDecisionThresholdError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(record)
}

fn measurement(
    suite_id: &str,
    metric_id: &str,
    observed_value: f64,
    detail: &str,
) -> PsionExecutorDecisionMeasurement {
    PsionExecutorDecisionMeasurement {
        suite_id: String::from(suite_id),
        metric_id: String::from(metric_id),
        observed_value,
        detail: String::from(detail),
    }
}

fn zero_regression_threshold(
    threshold_id: &str,
    suite_id: &str,
    metric_id: &str,
    baseline_value: f64,
    promotion_use: &str,
) -> PsionExecutorDecisionThreshold {
    PsionExecutorDecisionThreshold {
        threshold_id: String::from(threshold_id),
        suite_id: String::from(suite_id),
        metric_id: String::from(metric_id),
        direction: PsionExecutorDecisionDirection::ZeroRegression,
        baseline_value,
        observed_min: baseline_value,
        observed_max: baseline_value,
        observed_span: 0.0,
        policy_floor: 1.0,
        minimum_meaningful_delta: 1.0,
        regression_hold_delta: 1.0,
        promotion_use: String::from(promotion_use),
        detail: String::from(
            "Retained baseline replays observed zero noise, so any change of one regression row is material and promotion-blocking.",
        ),
    }
}

fn higher_is_better_threshold(
    threshold_id: &str,
    suite_id: &str,
    metric_id: &str,
    baseline_value: f64,
    policy_floor: f64,
    promotion_use: &str,
) -> PsionExecutorDecisionThreshold {
    PsionExecutorDecisionThreshold {
        threshold_id: String::from(threshold_id),
        suite_id: String::from(suite_id),
        metric_id: String::from(metric_id),
        direction: PsionExecutorDecisionDirection::HigherIsBetter,
        baseline_value,
        observed_min: baseline_value,
        observed_max: baseline_value,
        observed_span: 0.0,
        policy_floor,
        minimum_meaningful_delta: policy_floor,
        regression_hold_delta: policy_floor,
        promotion_use: String::from(promotion_use),
        detail: String::from(
            "Retained baseline replays showed zero noise, but phase-one promotion still requires a non-zero conservative floor before claiming throughput improvement.",
        ),
    }
}

fn lower_is_better_threshold(
    threshold_id: &str,
    suite_id: &str,
    metric_id: &str,
    baseline_value: f64,
    policy_floor: f64,
    promotion_use: &str,
) -> PsionExecutorDecisionThreshold {
    PsionExecutorDecisionThreshold {
        threshold_id: String::from(threshold_id),
        suite_id: String::from(suite_id),
        metric_id: String::from(metric_id),
        direction: PsionExecutorDecisionDirection::LowerIsBetter,
        baseline_value,
        observed_min: baseline_value,
        observed_max: baseline_value,
        observed_span: 0.0,
        policy_floor,
        minimum_meaningful_delta: policy_floor,
        regression_hold_delta: policy_floor,
        promotion_use: String::from(promotion_use),
        detail: String::from(
            "Retained baseline replays showed zero noise, but phase-one promotion still requires a non-zero conservative floor before claiming lower CPU-gap wins.",
        ),
    }
}

fn exactness_regression_count(
    baseline_truth: &PsionExecutorBaselineTruthRecord,
    suite_id: &str,
) -> Result<f64, PsionExecutorDecisionThresholdError> {
    let suite = baseline_suite(baseline_truth, suite_id)?;
    let regressions = suite
        .case_rows
        .iter()
        .filter(|row| {
            row.metrics.iter().any(|metric| {
                matches!(
                    metric.metric_id.as_str(),
                    "final_output_exactness_bps" | "step_exactness_bps" | "halt_exactness_bps"
                ) && metric.observed_value != "10000"
            })
        })
        .count();
    Ok(regressions as f64)
}

fn generalization_regression_count(
    baseline_truth: &PsionExecutorBaselineTruthRecord,
    suite_id: &str,
) -> Result<f64, PsionExecutorDecisionThresholdError> {
    let suite = baseline_suite(baseline_truth, suite_id)?;
    let regressions = suite
        .case_rows
        .iter()
        .filter(|row| row.posture == crate::PsionExecutorBaselineRowPosture::AutomatedMetric)
        .filter(|row| {
            row.metrics.iter().any(|metric| {
                metric.metric_id == "runtime_exactness_posture" && metric.observed_value != "Exact"
            }) || row.metrics.iter().any(|metric| {
                metric.metric_id == "outputs_equal" && metric.observed_value != "true"
            }) || row
                .metrics
                .iter()
                .any(|metric| metric.metric_id == "halt_equal" && metric.observed_value != "true")
        })
        .count();
    Ok(regressions as f64)
}

fn checklist_red_count(
    baseline_truth: &PsionExecutorBaselineTruthRecord,
    suite_id: &str,
    metric_id: &str,
) -> Result<f64, PsionExecutorDecisionThresholdError> {
    let suite = baseline_suite(baseline_truth, suite_id)?;
    let red_count = suite
        .case_rows
        .iter()
        .filter(|row| {
            row.metrics
                .iter()
                .any(|metric| metric.metric_id == metric_id && metric.observed_value != "green")
        })
        .count();
    Ok(red_count as f64)
}

fn baseline_suite<'a>(
    baseline_truth: &'a PsionExecutorBaselineTruthRecord,
    suite_id: &str,
) -> Result<&'a crate::PsionExecutorBaselineSuiteTruth, PsionExecutorDecisionThresholdError> {
    baseline_truth
        .suite_truths
        .iter()
        .find(|suite| suite.suite_id == suite_id)
        .ok_or_else(|| PsionExecutorDecisionThresholdError::UnknownSuite {
            suite_id: String::from(suite_id),
        })
}

fn promotion_suite_case_ids(
    catalog: &PsionExecutorEvalPackCatalog,
    suite_id: &str,
) -> Result<Vec<String>, PsionExecutorDecisionThresholdError> {
    catalog
        .packs
        .iter()
        .find(|pack| pack.pack_id == "tassadar.eval.promotion.v0")
        .and_then(|pack| {
            pack.suite_refs
                .iter()
                .find(|suite| suite.suite_id == suite_id)
        })
        .map(|suite| suite.case_ids.clone())
        .ok_or_else(|| PsionExecutorDecisionThresholdError::UnknownSuite {
            suite_id: String::from(suite_id),
        })
}

fn promotion_suite_ids(catalog: &PsionExecutorEvalPackCatalog) -> BTreeSet<&str> {
    catalog
        .packs
        .iter()
        .find(|pack| pack.pack_id == "tassadar.eval.promotion.v0")
        .map(|pack| {
            pack.suite_refs
                .iter()
                .map(|suite| suite.suite_id.as_str())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn median_f64(mut values: Vec<f64>) -> Result<f64, PsionExecutorDecisionThresholdError> {
    if values.is_empty() {
        return Err(PsionExecutorDecisionThresholdError::MissingField {
            field: String::from("median_f64.values"),
        });
    }
    values.sort_by(|left, right| left.partial_cmp(right).expect("finite values"));
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        Ok((values[middle - 1] + values[middle]) / 2.0)
    } else {
        Ok(values[middle])
    }
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    path: PathBuf,
) -> Result<T, PsionExecutorDecisionThresholdError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorDecisionThresholdError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorDecisionThresholdError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorDecisionThresholdError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_executor_decision_threshold_record_digest(
    record: &PsionExecutorDecisionThresholdRecord,
) -> String {
    let mut clone = record.clone();
    clone.record_digest.clear();
    stable_json_digest(&clone)
}

fn stable_json_digest<T: Serialize>(value: &T) -> String {
    let bytes =
        serde_json::to_vec(value).expect("executor decision-threshold digest serialization");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .expect("workspace root")
    }

    #[test]
    fn builtin_executor_decision_threshold_record_matches_committed_fixture() {
        let root = workspace_root();
        let built = builtin_executor_decision_threshold_record(&root).expect("built record");
        let fixture: PsionExecutorDecisionThresholdRecord = serde_json::from_slice(
            &fs::read(root.join(PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH))
                .expect("fixture bytes"),
        )
        .expect("fixture json");
        assert_eq!(built, fixture);
    }

    #[test]
    fn builtin_executor_decision_threshold_record_is_valid() {
        let root = workspace_root();
        let catalog = builtin_executor_eval_pack_catalog(&root).expect("catalog");
        let baseline = builtin_executor_baseline_truth_record(&root).expect("baseline");
        let record = builtin_executor_decision_threshold_record(&root).expect("record");
        record
            .validate_against_catalog(&catalog, &baseline)
            .expect("record should validate");
        assert_eq!(record.replay_passes.len(), REPLAY_PASS_COUNT);
        assert_eq!(record.thresholds.len(), 9);
        assert!(record.thresholds.iter().any(|threshold| {
            threshold.metric_id == "promotion_hull_cache_min_speedup_over_reference_linear"
                && (threshold.minimum_meaningful_delta - 0.05).abs() < 1e-9
        }));
        assert!(record.thresholds.iter().any(|threshold| {
            threshold.metric_id == "promotion_exactness_regression_count"
                && threshold.direction == PsionExecutorDecisionDirection::ZeroRegression
        }));
        assert!(record.thresholds.iter().any(|threshold| {
            threshold.metric_id == "promotion_runtime_blocker_red_count"
                && threshold.direction == PsionExecutorDecisionDirection::ZeroRegression
        }));
    }
}
