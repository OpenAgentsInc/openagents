use std::collections::BTreeMap;

use psionic_data::{
    AppleAdapterCorpusExpectedBehavior, AppleAdapterCorpusTaskFamily,
    AppleAdapterCuratedCorpusError, AppleAdapterCuratedCorpusManifest, AppleAdapterCuratedSplit,
    AppleAdapterDatasetContract, AppleAdapterSampleKind, AppleAdapterTrainingSample,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AppleAdapterEvalError, AppleAdapterEvalHarness, AppleAdapterObservedSampleOutput,
    BenchmarkAggregateSummary, BenchmarkExecutionMode, BenchmarkPackage, BenchmarkPackageKey,
    EvalArtifact, EvalMetric, EvalRunMode, EvalRunState, EvalRunStatus, EvalRuntimeError,
    EvalSampleRecord, EvalSampleStatus, EvalSummary,
};

/// Canonical benchmark ref for the first real Apple adapter run.
pub const APPLE_ARCHITECTURE_EXPLAINER_BENCHMARK_REF: &str =
    "benchmark://openagents/apple_adapter/psionic_architecture_explainer/base_vs_adapter";

/// Candidate labels compared by the base-vs-adapter benchmark gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterBenchmarkCandidate {
    /// Base model with no adapter attached.
    BaseModel,
    /// Adapted model after the Apple run.
    AdaptedModel,
}

impl AppleAdapterBenchmarkCandidate {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BaseModel => "base_model",
            Self::AdaptedModel => "adapted_model",
        }
    }
}

/// Explicit acceptance bar for the first real Apple benchmark gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterBaseVsAdapterAcceptancePolicy {
    /// Minimum aggregate score the adapted model must reach.
    pub minimum_adapter_score_bps: u32,
    /// Minimum aggregate pass rate the adapted model must reach.
    pub minimum_adapter_pass_rate_bps: u32,
    /// Minimum aggregate score delta over the base model.
    pub minimum_score_delta_bps: i32,
    /// Minimum aggregate pass-rate delta over the base model.
    pub minimum_pass_rate_delta_bps: i32,
    /// Minimum number of improved benchmark cases.
    pub minimum_improved_case_count: u32,
}

impl AppleAdapterBaseVsAdapterAcceptancePolicy {
    /// Default acceptance policy for the first `Psionic architecture explainer` run.
    #[must_use]
    pub const fn architecture_explainer_default() -> Self {
        Self {
            minimum_adapter_score_bps: 9_000,
            minimum_adapter_pass_rate_bps: 9_000,
            minimum_score_delta_bps: 1_500,
            minimum_pass_rate_delta_bps: 1_500,
            minimum_improved_case_count: 4,
        }
    }
}

/// Machine-legible reason why the gate accepted or rejected a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterBenchmarkAcceptanceReasonCode {
    /// Adapted aggregate score was too low.
    AdapterScoreBelowMinimum,
    /// Adapted aggregate pass rate was too low.
    AdapterPassRateBelowMinimum,
    /// Aggregate score delta over base was too small.
    ScoreDeltaBelowMinimum,
    /// Aggregate pass-rate delta over base was too small.
    PassRateDeltaBelowMinimum,
    /// Too few cases improved over the base model.
    ImprovedCaseCountBelowMinimum,
}

/// Per-case delta surfaced by the first real benchmark gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterBenchmarkCaseDelta {
    /// Stable benchmark case id.
    pub case_id: String,
    /// Curated task family for the case.
    pub task_family: AppleAdapterCorpusTaskFamily,
    /// Expected posture for the case.
    pub expected_behavior: AppleAdapterCorpusExpectedBehavior,
    /// Source ids that justify the case.
    pub source_ids: Vec<String>,
    /// Base-model sample status.
    pub base_status: EvalSampleStatus,
    /// Adapted-model sample status.
    pub adapted_status: EvalSampleStatus,
    /// Base-model score for the case.
    pub base_score_bps: u32,
    /// Adapted-model score for the case.
    pub adapted_score_bps: u32,
    /// Adapted minus base score delta.
    pub score_delta_bps: i32,
    /// Whether the adapted model improved on the base model.
    pub improved: bool,
}

/// Per-task-family delta surfaced by the first real benchmark gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterBenchmarkTaskFamilyDelta {
    /// Curated task family.
    pub task_family: AppleAdapterCorpusTaskFamily,
    /// Number of cases in the family.
    pub case_count: u32,
    /// Base-model mean score across the family.
    pub base_average_score_bps: u32,
    /// Adapted-model mean score across the family.
    pub adapted_average_score_bps: u32,
    /// Adapted minus base mean-score delta.
    pub score_delta_bps: i32,
    /// Improved case count within the family.
    pub improved_case_count: u32,
}

/// Final gate decision for the first real benchmark suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterBaseVsAdapterAcceptance {
    /// Whether the adapted model cleared the acceptance bar.
    pub accepted: bool,
    /// Aggregate score delta.
    pub aggregate_score_delta_bps: i32,
    /// Aggregate pass-rate delta.
    pub aggregate_pass_rate_delta_bps: i32,
    /// Improved case count.
    pub improved_case_count: u32,
    /// Machine-legible reason codes.
    pub reason_codes: Vec<AppleAdapterBenchmarkAcceptanceReasonCode>,
}

/// Machine-legible report for the first real base-vs-adapter benchmark gate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterBaseVsAdapterBenchmarkReport {
    /// Canonical benchmark package key.
    pub benchmark_key: BenchmarkPackageKey,
    /// Curated benchmark package with case metadata.
    pub benchmark_package: BenchmarkPackage,
    /// Full base-model benchmark eval receipt.
    pub base_eval_run: EvalRunState,
    /// Full adapted-model benchmark eval receipt.
    pub adapted_eval_run: EvalRunState,
    /// Base-model aggregate summary.
    pub base_summary: BenchmarkAggregateSummary,
    /// Adapted-model aggregate summary.
    pub adapted_summary: BenchmarkAggregateSummary,
    /// Per-case deltas.
    pub case_deltas: Vec<AppleAdapterBenchmarkCaseDelta>,
    /// Per-task-family deltas.
    pub task_family_deltas: Vec<AppleAdapterBenchmarkTaskFamilyDelta>,
    /// Final acceptance decision.
    pub acceptance: AppleAdapterBaseVsAdapterAcceptance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CuratedTextBehaviorAssessment {
    score_bps: u32,
    passed: bool,
    reason_code: &'static str,
}

/// Benchmark build or comparison failure for the first Apple real-run gate.
#[derive(Debug, Error)]
pub enum AppleAdapterBenchmarkError {
    /// Corpus validation failed.
    #[error(transparent)]
    Corpus(#[from] AppleAdapterCuratedCorpusError),
    /// Eval harness or benchmark runtime failed.
    #[error(transparent)]
    Eval(#[from] AppleAdapterEvalError),
    /// Lower-level eval runtime failed.
    #[error(transparent)]
    EvalRuntime(#[from] EvalRuntimeError),
    /// Corpus target does not match the architecture explainer benchmark.
    #[error("expected architecture explainer corpus target `{expected}`, found `{actual}`")]
    UnexpectedCorpusTarget { expected: String, actual: String },
    /// Benchmark sample annotation missing from the curated corpus.
    #[error("benchmark case `{sample_id}` is missing a curated benchmark annotation")]
    MissingBenchmarkAnnotation { sample_id: String },
    /// Benchmark annotation referred to a sample that is not in the benchmark dataset.
    #[error("curated benchmark annotation `{sample_id}` does not exist in the benchmark dataset")]
    UnknownBenchmarkAnnotation { sample_id: String },
    /// One compared run did not finalize.
    #[error("benchmark run `{eval_run_id}` for `{candidate}` is not finalized (found `{status}`)")]
    BenchmarkRunNotFinalized {
        candidate: String,
        eval_run_id: String,
        status: String,
    },
    /// One compared run was not in benchmark mode.
    #[error("benchmark run `{eval_run_id}` for `{candidate}` is not in benchmark mode")]
    BenchmarkRunModeMismatch {
        candidate: String,
        eval_run_id: String,
    },
    /// One compared run targeted the wrong benchmark package.
    #[error(
        "benchmark run package mismatch for `{candidate}`: expected `{expected}`, found `{actual}`"
    )]
    BenchmarkRunPackageMismatch {
        candidate: String,
        expected: String,
        actual: String,
    },
    /// One compared run targeted the wrong environment.
    #[error(
        "benchmark run environment mismatch for `{candidate}`: expected `{expected}`, found `{actual}`"
    )]
    BenchmarkRunEnvironmentMismatch {
        candidate: String,
        expected: String,
        actual: String,
    },
    /// One benchmark case was missing from a compared run.
    #[error("benchmark run for `{candidate}` is missing sample `{sample_id}`")]
    MissingRunSample {
        candidate: String,
        sample_id: String,
    },
}

/// Returns the canonical benchmark key for the architecture-explainer corpus.
pub fn architecture_explainer_benchmark_key(
    corpus: &AppleAdapterCuratedCorpusManifest,
) -> Result<BenchmarkPackageKey, AppleAdapterBenchmarkError> {
    corpus.validate()?;
    if corpus.target_id != "apple_adapter.psionic_architecture_explainer" {
        return Err(AppleAdapterBenchmarkError::UnexpectedCorpusTarget {
            expected: String::from("apple_adapter.psionic_architecture_explainer"),
            actual: corpus.target_id.clone(),
        });
    }
    Ok(BenchmarkPackageKey::new(
        APPLE_ARCHITECTURE_EXPLAINER_BENCHMARK_REF,
        corpus.dataset.version.clone(),
    ))
}

/// Builds the curated benchmark package for the first real Apple run.
pub fn build_curated_benchmark_package(
    harness: &AppleAdapterEvalHarness,
    benchmark_key: BenchmarkPackageKey,
    dataset: &AppleAdapterDatasetContract,
    corpus: &AppleAdapterCuratedCorpusManifest,
    repeat_count: u32,
) -> Result<BenchmarkPackage, AppleAdapterBenchmarkError> {
    let annotations = benchmark_annotation_map(corpus, dataset)?;
    let mut package = harness.build_benchmark_package(benchmark_key, dataset, repeat_count)?;
    for case in &mut package.cases {
        let annotation = annotations.get(case.case_id.as_str()).ok_or_else(|| {
            AppleAdapterBenchmarkError::MissingBenchmarkAnnotation {
                sample_id: case.case_id.clone(),
            }
        })?;
        let mut metadata = case
            .metadata
            .as_object()
            .cloned()
            .unwrap_or_else(Map::<String, Value>::new);
        metadata.insert(
            String::from("task_family"),
            serde_json::to_value(annotation.task_family).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from("expected_behavior"),
            serde_json::to_value(annotation.expected_behavior).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from("source_ids"),
            serde_json::to_value(annotation.source_ids.clone()).unwrap_or(Value::Null),
        );
        case.metadata = Value::Object(metadata);
    }
    package.metadata.insert(
        String::from("apple_adapter.target_id"),
        Value::String(corpus.target_id.clone()),
    );
    package.metadata.insert(
        String::from("apple_adapter.target_title"),
        Value::String(corpus.target_title.clone()),
    );
    package.metadata.insert(
        String::from("apple_adapter.dataset_scope"),
        Value::String(corpus.target_scope.clone()),
    );
    package.validate()?;
    Ok(package)
}

/// Runs the full base-vs-adapter benchmark suite and evaluates the acceptance gate.
pub fn run_curated_base_vs_adapter_benchmark(
    harness: &AppleAdapterEvalHarness,
    benchmark_package: &BenchmarkPackage,
    dataset: &AppleAdapterDatasetContract,
    corpus: &AppleAdapterCuratedCorpusManifest,
    base_outputs: Vec<AppleAdapterObservedSampleOutput>,
    adapted_outputs: Vec<AppleAdapterObservedSampleOutput>,
    acceptance_policy: &AppleAdapterBaseVsAdapterAcceptancePolicy,
    started_at_ms: u64,
    finalized_at_ms: u64,
) -> Result<AppleAdapterBaseVsAdapterBenchmarkReport, AppleAdapterBenchmarkError> {
    let base_run = harness.run_benchmark_round(
        format!("{}.base_model", benchmark_package.key.storage_key()),
        benchmark_package,
        dataset,
        base_outputs,
        started_at_ms,
        finalized_at_ms,
    )?;
    let adapted_run = harness.run_benchmark_round(
        format!("{}.adapted_model", benchmark_package.key.storage_key()),
        benchmark_package,
        dataset,
        adapted_outputs,
        started_at_ms.saturating_add(10),
        finalized_at_ms.saturating_add(10),
    )?;
    compare_curated_base_vs_adapter_runs(
        benchmark_package,
        dataset,
        corpus,
        &base_run,
        &adapted_run,
        acceptance_policy,
    )
}

/// Compares finalized base and adapted benchmark runs under one acceptance policy.
pub fn compare_curated_base_vs_adapter_runs(
    benchmark_package: &BenchmarkPackage,
    dataset: &AppleAdapterDatasetContract,
    corpus: &AppleAdapterCuratedCorpusManifest,
    base_run: &EvalRunState,
    adapted_run: &EvalRunState,
    acceptance_policy: &AppleAdapterBaseVsAdapterAcceptancePolicy,
) -> Result<AppleAdapterBaseVsAdapterBenchmarkReport, AppleAdapterBenchmarkError> {
    let annotations = benchmark_annotation_map(corpus, dataset)?;
    ensure_run_matches_package(
        AppleAdapterBenchmarkCandidate::BaseModel,
        benchmark_package,
        base_run,
    )?;
    ensure_run_matches_package(
        AppleAdapterBenchmarkCandidate::AdaptedModel,
        benchmark_package,
        adapted_run,
    )?;
    let base_run = behavior_adjusted_benchmark_run(dataset, &annotations, base_run)?;
    let adapted_run = behavior_adjusted_benchmark_run(dataset, &annotations, adapted_run)?;

    let mut base_execution = benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    base_execution.record_round(&base_run)?;
    let base_summary = base_execution.finalize()?;

    let mut adapted_execution = benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    adapted_execution.record_round(&adapted_run)?;
    let adapted_summary = adapted_execution.finalize()?;

    let base_samples = base_run
        .samples
        .iter()
        .map(|sample| (sample.sample_id.clone(), sample))
        .collect::<BTreeMap<_, _>>();
    let adapted_samples = adapted_run
        .samples
        .iter()
        .map(|sample| (sample.sample_id.clone(), sample))
        .collect::<BTreeMap<_, _>>();

    let mut case_deltas = Vec::new();
    let mut task_family_rollups: BTreeMap<AppleAdapterCorpusTaskFamily, (u64, u64, u32, u32)> =
        BTreeMap::new();
    for case in &benchmark_package.cases {
        let annotation = annotations.get(case.case_id.as_str()).ok_or_else(|| {
            AppleAdapterBenchmarkError::MissingBenchmarkAnnotation {
                sample_id: case.case_id.clone(),
            }
        })?;
        let base_sample = base_samples.get(case.case_id.as_str()).ok_or_else(|| {
            AppleAdapterBenchmarkError::MissingRunSample {
                candidate: AppleAdapterBenchmarkCandidate::BaseModel
                    .label()
                    .to_string(),
                sample_id: case.case_id.clone(),
            }
        })?;
        let adapted_sample = adapted_samples.get(case.case_id.as_str()).ok_or_else(|| {
            AppleAdapterBenchmarkError::MissingRunSample {
                candidate: AppleAdapterBenchmarkCandidate::AdaptedModel
                    .label()
                    .to_string(),
                sample_id: case.case_id.clone(),
            }
        })?;
        let base_score_bps = base_sample.score_bps.unwrap_or(0);
        let adapted_score_bps = adapted_sample.score_bps.unwrap_or(0);
        let improved = adapted_score_bps > base_score_bps;
        let score_delta_bps = adapted_score_bps as i32 - base_score_bps as i32;
        case_deltas.push(AppleAdapterBenchmarkCaseDelta {
            case_id: case.case_id.clone(),
            task_family: annotation.task_family,
            expected_behavior: annotation.expected_behavior,
            source_ids: annotation.source_ids.clone(),
            base_status: base_sample.status,
            adapted_status: adapted_sample.status,
            base_score_bps,
            adapted_score_bps,
            score_delta_bps,
            improved,
        });
        let rollup = task_family_rollups
            .entry(annotation.task_family)
            .or_insert((0, 0, 0, 0));
        rollup.0 = rollup.0.saturating_add(u64::from(base_score_bps));
        rollup.1 = rollup.1.saturating_add(u64::from(adapted_score_bps));
        rollup.2 = rollup.2.saturating_add(1);
        if improved {
            rollup.3 = rollup.3.saturating_add(1);
        }
    }

    let task_family_deltas = task_family_rollups
        .into_iter()
        .map(
            |(task_family, (base_total, adapted_total, case_count, improved_case_count))| {
                let base_average_score_bps = (base_total / u64::from(case_count.max(1))) as u32;
                let adapted_average_score_bps =
                    (adapted_total / u64::from(case_count.max(1))) as u32;
                AppleAdapterBenchmarkTaskFamilyDelta {
                    task_family,
                    case_count,
                    base_average_score_bps,
                    adapted_average_score_bps,
                    score_delta_bps: adapted_average_score_bps as i32
                        - base_average_score_bps as i32,
                    improved_case_count,
                }
            },
        )
        .collect::<Vec<_>>();

    let base_score = base_summary.aggregate_score_bps.unwrap_or(0);
    let adapted_score = adapted_summary.aggregate_score_bps.unwrap_or(0);
    let aggregate_score_delta_bps = adapted_score as i32 - base_score as i32;
    let aggregate_pass_rate_delta_bps = adapted_summary.aggregate_pass_rate_bps as i32
        - base_summary.aggregate_pass_rate_bps as i32;
    let improved_case_count = case_deltas.iter().filter(|delta| delta.improved).count() as u32;
    let mut reason_codes = Vec::new();
    if adapted_score < acceptance_policy.minimum_adapter_score_bps {
        reason_codes.push(AppleAdapterBenchmarkAcceptanceReasonCode::AdapterScoreBelowMinimum);
    }
    if adapted_summary.aggregate_pass_rate_bps < acceptance_policy.minimum_adapter_pass_rate_bps {
        reason_codes.push(AppleAdapterBenchmarkAcceptanceReasonCode::AdapterPassRateBelowMinimum);
    }
    if aggregate_score_delta_bps < acceptance_policy.minimum_score_delta_bps {
        reason_codes.push(AppleAdapterBenchmarkAcceptanceReasonCode::ScoreDeltaBelowMinimum);
    }
    if aggregate_pass_rate_delta_bps < acceptance_policy.minimum_pass_rate_delta_bps {
        reason_codes.push(AppleAdapterBenchmarkAcceptanceReasonCode::PassRateDeltaBelowMinimum);
    }
    if improved_case_count < acceptance_policy.minimum_improved_case_count {
        reason_codes.push(AppleAdapterBenchmarkAcceptanceReasonCode::ImprovedCaseCountBelowMinimum);
    }

    Ok(AppleAdapterBaseVsAdapterBenchmarkReport {
        benchmark_key: benchmark_package.key.clone(),
        benchmark_package: benchmark_package.clone(),
        base_eval_run: base_run,
        adapted_eval_run: adapted_run,
        base_summary,
        adapted_summary,
        case_deltas,
        task_family_deltas,
        acceptance: AppleAdapterBaseVsAdapterAcceptance {
            accepted: reason_codes.is_empty(),
            aggregate_score_delta_bps,
            aggregate_pass_rate_delta_bps,
            improved_case_count,
            reason_codes,
        },
    })
}

fn behavior_adjusted_benchmark_run(
    dataset: &AppleAdapterDatasetContract,
    annotations: &BTreeMap<String, &psionic_data::AppleAdapterCuratedSampleAnnotation>,
    run: &EvalRunState,
) -> Result<EvalRunState, AppleAdapterBenchmarkError> {
    let samples_by_id = dataset
        .samples
        .iter()
        .map(|sample| (sample.sample_id.as_str(), sample))
        .collect::<BTreeMap<_, _>>();
    let adjusted_samples = run
        .samples
        .iter()
        .map(|record| {
            let sample = samples_by_id
                .get(record.sample_id.as_str())
                .ok_or_else(|| AppleAdapterBenchmarkError::MissingBenchmarkAnnotation {
                    sample_id: record.sample_id.clone(),
                })?;
            let annotation = annotations.get(record.sample_id.as_str()).ok_or_else(|| {
                AppleAdapterBenchmarkError::MissingBenchmarkAnnotation {
                    sample_id: record.sample_id.clone(),
                }
            })?;
            Ok(behavior_adjusted_sample_record(sample, annotation, record))
        })
        .collect::<Result<Vec<_>, AppleAdapterBenchmarkError>>()?;
    let mut adjusted_run = run.clone();
    adjusted_run.samples = adjusted_samples;
    adjusted_run.summary = Some(build_adjusted_summary(
        adjusted_run.contract.stable_digest().as_str(),
        adjusted_run.samples.as_slice(),
        adjusted_run.run_artifacts.as_slice(),
    ));
    Ok(adjusted_run)
}

fn behavior_adjusted_sample_record(
    sample: &AppleAdapterTrainingSample,
    annotation: &psionic_data::AppleAdapterCuratedSampleAnnotation,
    record: &EvalSampleRecord,
) -> EvalSampleRecord {
    if sample.sample_kind != AppleAdapterSampleKind::SupervisedFineTune || !sample.tools.is_empty()
    {
        return record.clone();
    }
    let Some(observed_text) = record
        .metadata
        .get("apple_adapter.observed_output_text")
        .and_then(Value::as_str)
    else {
        return record.clone();
    };
    let expected_text = sample
        .messages
        .last()
        .map(|message| message.content.as_str())
        .unwrap_or_default();
    let assessment = curated_text_behavior_assessment(
        annotation.expected_behavior,
        expected_text,
        observed_text,
    );
    let mut adjusted = record.clone();
    adjusted.score_bps = Some(assessment.score_bps);
    adjusted.status = if assessment.passed {
        EvalSampleStatus::Passed
    } else {
        EvalSampleStatus::Failed
    };
    adjusted.metrics.push(
        EvalMetric::new(
            "apple_adapter.benchmark_behavior_text_score",
            f64::from(assessment.score_bps) / 10_000.0,
        )
        .with_unit("fraction")
        .with_metadata(serde_json::json!({
            "expected_behavior": annotation.expected_behavior,
            "reason_code": assessment.reason_code,
        })),
    );
    if let Some(raw_score_bps) = record.score_bps {
        adjusted.metadata.insert(
            String::from("apple_adapter.raw_eval_score_bps"),
            Value::from(raw_score_bps),
        );
    }
    if let Some(raw_error_reason) = record.error_reason.as_ref() {
        adjusted.metadata.insert(
            String::from("apple_adapter.raw_eval_error_reason"),
            Value::String(raw_error_reason.clone()),
        );
    }
    adjusted.metadata.insert(
        String::from("apple_adapter.benchmark_behavior_score_bps"),
        Value::from(assessment.score_bps),
    );
    adjusted.metadata.insert(
        String::from("apple_adapter.benchmark_behavior_passed"),
        Value::Bool(assessment.passed),
    );
    adjusted.metadata.insert(
        String::from("apple_adapter.benchmark_behavior_reason_code"),
        Value::String(assessment.reason_code.to_string()),
    );
    adjusted.error_reason = (!assessment.passed).then(|| assessment.reason_code.to_string());
    adjusted
}

fn curated_text_behavior_assessment(
    expected_behavior: AppleAdapterCorpusExpectedBehavior,
    expected_text: &str,
    observed_text: &str,
) -> CuratedTextBehaviorAssessment {
    if normalized_text(expected_text) == normalized_text(observed_text) {
        return CuratedTextBehaviorAssessment {
            score_bps: 10_000,
            passed: true,
            reason_code: "exact_match",
        };
    }
    if observed_text.trim().starts_with("runtime_error:") {
        return CuratedTextBehaviorAssessment {
            score_bps: 0,
            passed: false,
            reason_code: "runtime_error",
        };
    }
    let observed_lower = observed_text.to_ascii_lowercase();
    let expected_lower = expected_text.to_ascii_lowercase();
    let keyword_overlap = text_keyword_overlap(expected_text, observed_text);
    let refusal_like = contains_any(
        observed_lower.as_str(),
        &[
            "can't assist",
            "cannot assist",
            "i'm sorry",
            "illegal activities",
            "illegal",
            "prohibited",
        ],
    );

    match expected_behavior {
        AppleAdapterCorpusExpectedBehavior::DirectAnswer => {
            if refusal_like {
                return CuratedTextBehaviorAssessment {
                    score_bps: 0,
                    passed: false,
                    reason_code: "direct_answer_refused",
                };
            }
            let score = ((keyword_overlap.recall * 0.7) + (keyword_overlap.precision * 0.3))
                .clamp(0.0, 1.0);
            CuratedTextBehaviorAssessment {
                score_bps: fraction_to_bps(score),
                passed: keyword_overlap.recall >= 0.72 && keyword_overlap.precision >= 0.38,
                reason_code: "direct_answer_overlap",
            }
        }
        AppleAdapterCorpusExpectedBehavior::Correction => {
            let expected_stance = leading_stance(expected_lower.as_str());
            let observed_stance = leading_stance(observed_lower.as_str());
            if expected_stance.is_some() && expected_stance != observed_stance {
                return CuratedTextBehaviorAssessment {
                    score_bps: 0,
                    passed: false,
                    reason_code: "correction_stance_mismatch",
                };
            }
            if refusal_like {
                return CuratedTextBehaviorAssessment {
                    score_bps: 0,
                    passed: false,
                    reason_code: "correction_refused",
                };
            }
            let score = ((keyword_overlap.recall * 0.8) + (keyword_overlap.precision * 0.2))
                .clamp(0.0, 1.0);
            CuratedTextBehaviorAssessment {
                score_bps: fraction_to_bps(score),
                passed: expected_stance == observed_stance && keyword_overlap.recall >= 0.45,
                reason_code: "correction_overlap",
            }
        }
        AppleAdapterCorpusExpectedBehavior::Refusal => {
            let uncertainty_score = marker_fraction(
                observed_lower.as_str(),
                &[
                    "cannot answer",
                    "can't answer",
                    "do not have access",
                    "don't have access",
                    "real-time access",
                    "up-to-date",
                    "need retrieval",
                    "have to be checked",
                ],
            );
            let freshness_score = marker_fraction(
                observed_lower.as_str(),
                &[
                    "latest",
                    "current",
                    "today",
                    "installed",
                    "runtime",
                    "assets",
                    "compatibility",
                ],
            );
            let validation_score = marker_fraction(
                observed_lower.as_str(),
                &[
                    "live runtime validation",
                    "runtime validation",
                    "checked",
                    "check",
                    "depends",
                    "apple developer",
                    "apple support",
                ],
            );
            let score = keyword_overlap.f1.max(
                (uncertainty_score * 0.4) + (freshness_score * 0.3) + (validation_score * 0.3),
            );
            CuratedTextBehaviorAssessment {
                score_bps: fraction_to_bps(score),
                passed: uncertainty_score >= 0.25
                    && freshness_score >= 0.25
                    && (validation_score >= 0.16 || keyword_overlap.f1 >= 0.15),
                reason_code: "refusal_grounded_currentness",
            }
        }
        AppleAdapterCorpusExpectedBehavior::StructuredAnswer
        | AppleAdapterCorpusExpectedBehavior::ToolLookupRouting => CuratedTextBehaviorAssessment {
            score_bps: record_score_bps(expected_text, observed_text),
            passed: normalized_text(expected_text) == normalized_text(observed_text),
            reason_code: "exact_text_only",
        },
    }
}

fn record_score_bps(expected_text: &str, observed_text: &str) -> u32 {
    if normalized_text(expected_text) == normalized_text(observed_text) {
        10_000
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TextKeywordOverlap {
    precision: f64,
    recall: f64,
    f1: f64,
}

fn text_keyword_overlap(expected_text: &str, observed_text: &str) -> TextKeywordOverlap {
    let expected = benchmark_keywords(expected_text);
    let observed = benchmark_keywords(observed_text);
    if expected.is_empty() || observed.is_empty() {
        return TextKeywordOverlap::default();
    }
    let expected_map = expected
        .iter()
        .map(|token| (token.as_str(), ()))
        .collect::<BTreeMap<_, _>>();
    let observed_map = observed
        .iter()
        .map(|token| (token.as_str(), ()))
        .collect::<BTreeMap<_, _>>();
    let matches = expected_map
        .keys()
        .filter(|token| observed_map.contains_key(**token))
        .count();
    if matches == 0 {
        return TextKeywordOverlap::default();
    }
    let precision = matches as f64 / observed_map.len() as f64;
    let recall = matches as f64 / expected_map.len() as f64;
    let f1 = (2.0 * precision * recall) / (precision + recall);
    TextKeywordOverlap {
        precision,
        recall,
        f1,
    }
}

fn benchmark_keywords(text: &str) -> Vec<String> {
    lexical_tokens(text)
        .into_iter()
        .filter(|token| token.len() > 2 && !BENCHMARK_STOP_WORDS.contains(&token.as_str()))
        .collect()
}

fn lexical_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

const BENCHMARK_STOP_WORDS: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "from", "into", "your", "you", "our", "are",
    "can", "what", "when", "where", "which", "their", "they", "them", "will", "would", "should",
    "could", "about", "have", "has", "had", "was", "were", "but", "not", "still", "already",
    "current", "latest", "today", "there", "here", "then", "than", "its", "it's", "able",
];

fn marker_fraction(text: &str, markers: &[&str]) -> f64 {
    if markers.is_empty() {
        return 0.0;
    }
    markers
        .iter()
        .filter(|marker| text.contains(**marker))
        .count() as f64
        / markers.len() as f64
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn leading_stance(text: &str) -> Option<&'static str> {
    lexical_tokens(text)
        .into_iter()
        .take(4)
        .find_map(|token| match token.as_str() {
            "yes" => Some("yes"),
            "no" => Some("no"),
            _ => None,
        })
}

fn fraction_to_bps(value: f64) -> u32 {
    (value.clamp(0.0, 1.0) * 10_000.0).round() as u32
}

fn normalized_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn build_adjusted_summary(
    contract_digest: &str,
    samples: &[EvalSampleRecord],
    run_artifacts: &[EvalArtifact],
) -> EvalSummary {
    let total_samples = samples.len() as u64;
    let scored_samples = samples
        .iter()
        .filter(|sample| sample.score_bps.is_some())
        .count() as u64;
    let passed_samples = samples
        .iter()
        .filter(|sample| sample.status == EvalSampleStatus::Passed)
        .count() as u64;
    let failed_samples = samples
        .iter()
        .filter(|sample| sample.status == EvalSampleStatus::Failed)
        .count() as u64;
    let errored_samples = samples
        .iter()
        .filter(|sample| sample.status == EvalSampleStatus::Errored)
        .count() as u64;
    let average_score_bps = if scored_samples == 0 {
        None
    } else {
        Some(
            (samples
                .iter()
                .filter_map(|sample| sample.score_bps)
                .map(u64::from)
                .sum::<u64>()
                / scored_samples) as u32,
        )
    };
    let pass_rate_bps = if total_samples == 0 {
        0
    } else {
        ((passed_samples.saturating_mul(10_000)) / total_samples) as u32
    };
    let mut metric_rollups: BTreeMap<String, (f64, u64, Option<String>, Value)> = BTreeMap::new();
    for sample in samples {
        for metric in &sample.metrics {
            let entry = metric_rollups.entry(metric.metric_id.clone()).or_insert((
                0.0,
                0,
                metric.unit.clone(),
                metric.metadata.clone(),
            ));
            entry.0 += metric.metric_value;
            entry.1 = entry.1.saturating_add(1);
            if entry.2.is_none() {
                entry.2 = metric.unit.clone();
            }
        }
    }
    let aggregate_metrics = metric_rollups
        .into_iter()
        .map(|(metric_id, (sum, count, unit, metadata))| EvalMetric {
            metric_id,
            metric_value: if count == 0 { 0.0 } else { sum / count as f64 },
            unit,
            metadata,
        })
        .collect::<Vec<_>>();
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_eval_summary|");
    hasher.update(contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(total_samples.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(scored_samples.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(passed_samples.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(failed_samples.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(errored_samples.to_string().as_bytes());
    if let Some(average_score_bps) = average_score_bps {
        hasher.update(b"|score|");
        hasher.update(average_score_bps.to_string().as_bytes());
    }
    hasher.update(b"|pass_rate|");
    hasher.update(pass_rate_bps.to_string().as_bytes());
    for metric in &aggregate_metrics {
        hasher.update(b"|metric|");
        hasher.update(metric.metric_id.as_bytes());
        hasher.update(b"|");
        hasher.update(metric.metric_value.to_string().as_bytes());
    }
    for artifact in run_artifacts {
        hasher.update(b"|artifact|");
        hasher.update(artifact.artifact_kind.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact.artifact_digest.as_bytes());
    }
    EvalSummary {
        total_samples,
        scored_samples,
        passed_samples,
        failed_samples,
        errored_samples,
        average_score_bps,
        pass_rate_bps,
        aggregate_metrics,
        artifacts: run_artifacts.to_vec(),
        summary_digest: hex::encode(hasher.finalize()),
    }
}

fn benchmark_annotation_map<'a>(
    corpus: &'a AppleAdapterCuratedCorpusManifest,
    benchmark_dataset: &AppleAdapterDatasetContract,
) -> Result<
    BTreeMap<String, &'a psionic_data::AppleAdapterCuratedSampleAnnotation>,
    AppleAdapterBenchmarkError,
> {
    corpus.validate()?;
    let annotations = corpus
        .samples
        .iter()
        .filter(|sample| sample.split == AppleAdapterCuratedSplit::Benchmark)
        .map(|sample| (sample.sample_id.clone(), sample))
        .collect::<BTreeMap<_, _>>();
    for sample in &benchmark_dataset.samples {
        if !annotations.contains_key(sample.sample_id.as_str()) {
            return Err(AppleAdapterBenchmarkError::MissingBenchmarkAnnotation {
                sample_id: sample.sample_id.clone(),
            });
        }
    }
    for sample_id in annotations.keys() {
        if !benchmark_dataset
            .samples
            .iter()
            .any(|sample| sample.sample_id == *sample_id)
        {
            return Err(AppleAdapterBenchmarkError::UnknownBenchmarkAnnotation {
                sample_id: sample_id.clone(),
            });
        }
    }
    Ok(annotations)
}

fn ensure_run_matches_package(
    candidate: AppleAdapterBenchmarkCandidate,
    benchmark_package: &BenchmarkPackage,
    run: &EvalRunState,
) -> Result<(), AppleAdapterBenchmarkError> {
    if run.status != EvalRunStatus::Finalized {
        return Err(AppleAdapterBenchmarkError::BenchmarkRunNotFinalized {
            candidate: candidate.label().to_string(),
            eval_run_id: run.contract.eval_run_id.clone(),
            status: serde_json::to_string(&run.status).unwrap_or_else(|_| String::from("unknown")),
        });
    }
    if run.contract.mode != EvalRunMode::Benchmark {
        return Err(AppleAdapterBenchmarkError::BenchmarkRunModeMismatch {
            candidate: candidate.label().to_string(),
            eval_run_id: run.contract.eval_run_id.clone(),
        });
    }
    if run.contract.environment != benchmark_package.environment {
        return Err(
            AppleAdapterBenchmarkError::BenchmarkRunEnvironmentMismatch {
                candidate: candidate.label().to_string(),
                expected: benchmark_package.environment.storage_key(),
                actual: run.contract.environment.storage_key(),
            },
        );
    }
    if run.contract.benchmark_package.as_ref() != Some(&benchmark_package.key) {
        return Err(AppleAdapterBenchmarkError::BenchmarkRunPackageMismatch {
            candidate: candidate.label().to_string(),
            expected: benchmark_package.key.storage_key(),
            actual: run
                .contract
                .benchmark_package
                .as_ref()
                .map_or_else(String::new, BenchmarkPackageKey::storage_key),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppleAdapterObservedToolCall, EvalExecutionStrategyFacts, EvalFinalStateCapture,
        EvalTimerIntegrityFacts, EvalTokenAccountingFacts, EvalVerificationFacts,
    };
    use psionic_data::{AppleAdapterDatasetMetadata, TokenizerDigest, TokenizerFamily};
    use psionic_environments::EnvironmentDatasetBinding;
    use psionic_environments::{
        AppleAdapterEnvironmentPackageRefs, AppleAdapterEnvironmentRuntimeRequirements,
        AppleAdapterEnvironmentSpec, EnvironmentArtifactExpectation, EnvironmentDifficultyMetadata,
        EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRubricHook,
        EnvironmentRubricScoreKind, EnvironmentToolContract, EnvironmentToolInterface,
    };

    fn dataset_metadata() -> AppleAdapterDatasetMetadata {
        AppleAdapterDatasetMetadata::new(
            TokenizerDigest::new(
                TokenizerFamily::SentencePiece,
                "apple-tokenizer-digest-v1",
                32_768,
            )
            .with_special_tokens_digest("apple-special-tokens-v1")
            .with_template_digest("apple-template-v1"),
            "apple-prompt-shaping-v1",
        )
        .with_default_instruction("A conversation between a user and a helpful assistant.")
        .with_locale("en-US")
    }

    fn dataset_contract() -> AppleAdapterDatasetContract {
        AppleAdapterDatasetContract::from_jsonl_str(
            include_str!(
                "../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/benchmark.jsonl"
            ),
            dataset_metadata(),
        )
        .expect("benchmark corpus fixture should import")
    }

    fn corpus_manifest() -> AppleAdapterCuratedCorpusManifest {
        serde_json::from_str(include_str!(
            "../../fixtures/apple_adapter/datasets/psionic_architecture_explainer/corpus_manifest.json"
        ))
        .expect("curated corpus manifest should parse")
    }

    fn environment_bundle() -> psionic_environments::AppleAdapterEnvironmentBundle {
        let spec = AppleAdapterEnvironmentSpec {
            version: String::from("2026.03.15"),
            display_name: String::from("Apple Architecture Explainer Eval"),
            core_environment_ref: String::from("env.openagents.apple.architecture_explainer.core"),
            benchmark_environment_ref: String::from(
                "env.openagents.apple.architecture_explainer.benchmark",
            ),
            train_dataset: EnvironmentDatasetBinding {
                dataset: psionic_data::DatasetKey::new(
                    "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                    "2026.03.15.2",
                ),
                split: Some(String::from("train")),
                mount_path: String::from("/datasets/apple/train"),
                required: true,
            },
            held_out_eval_dataset: EnvironmentDatasetBinding {
                dataset: psionic_data::DatasetKey::new(
                    "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                    "2026.03.15.2",
                ),
                split: Some(String::from("held_out")),
                mount_path: String::from("/datasets/apple/held_out"),
                required: true,
            },
            benchmark_dataset: Some(EnvironmentDatasetBinding {
                dataset: psionic_data::DatasetKey::new(
                    "dataset://openagents/apple_adapter/psionic_architecture_explainer",
                    "2026.03.15.2",
                ),
                split: Some(String::from("benchmark")),
                mount_path: String::from("/datasets/apple/benchmark"),
                required: true,
            }),
            package_refs: AppleAdapterEnvironmentPackageRefs {
                group_ref: String::from("group.apple.architecture_explainer"),
                core_pin_alias: String::from("apple_architecture_explainer_core"),
                benchmark_pin_alias: String::from("apple_architecture_explainer_benchmark"),
                core_member_ref: String::from("apple_architecture_explainer_core_member"),
                benchmark_member_ref: String::from("apple_architecture_explainer_benchmark_member"),
                session_profile_ref: String::from("session://apple/architecture_explainer"),
                runtime_profile_ref: String::from("runtime://apple/fm"),
                tool_bundle_ref: String::from("tools://apple/architecture_explainer"),
                rubric_binding_ref: String::from("rubric://apple/architecture_explainer"),
                structured_output_profile_ref: Some(String::from(
                    "structured://apple/architecture_explainer",
                )),
                benchmark_profile_ref: String::from("benchmark://apple/architecture_explainer"),
                benchmark_runtime_profile_ref: String::from(
                    "runtime://apple/architecture_explainer/benchmark",
                ),
            },
            runtime_requirements: AppleAdapterEnvironmentRuntimeRequirements {
                foundation_bridge_ref: String::from("bridge://apple-foundation-models"),
                model_id: String::from("apple-foundation-model"),
                platform_requirement: String::from("macos26_apple_silicon"),
                adapter_inventory_required: true,
                session_attach_required: true,
                structured_output_supported: true,
                tool_calling_supported: true,
                max_context_tokens: 4096,
                max_session_turns: 4,
                time_budget_ms: 30_000,
            },
            tools: vec![EnvironmentToolContract {
                tool_name: String::from("lookup_doc"),
                interface: EnvironmentToolInterface::NativeFunction,
                description: String::from("Inspect one canonical repo doc by path"),
                args_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"],
                    "additionalProperties": false
                }),
                result_schema: None,
            }],
            rubric_hooks: vec![EnvironmentRubricHook {
                rubric_ref: String::from("rubric://apple/architecture_explainer/quality"),
                hook_name: String::from("answer_quality"),
                score_kind: EnvironmentRubricScoreKind::Scalar,
                pass_threshold: Some(8000),
            }],
            expected_artifacts: vec![EnvironmentArtifactExpectation {
                artifact_kind: String::from("apple_adapter.eval.transcript"),
                required: false,
                verification_policy_ref: Some(String::from(
                    "verify://apple/architecture_explainer/trace",
                )),
            }],
            core_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Training,
                policy_ref: String::from("policy://apple/architecture_explainer/eval"),
                required: true,
            }],
            benchmark_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://apple/architecture_explainer/benchmark"),
                required: true,
            }],
            difficulty: Some(EnvironmentDifficultyMetadata {
                difficulty_tier: String::from("narrow"),
                min_agent_level: Some(1),
                tags: vec![String::from("architecture"), String::from("benchmark")],
            }),
        };
        spec.build_bundle()
            .expect("environment bundle should validate")
    }

    fn benchmark_verification(sample_id: &str) -> EvalVerificationFacts {
        EvalVerificationFacts {
            timer_integrity: Some(EvalTimerIntegrityFacts {
                declared_budget_ms: Some(2_000),
                elapsed_ms: 800,
                within_budget: true,
            }),
            token_accounting: Some(
                EvalTokenAccountingFacts::new(12, 18, 30).expect("token accounting"),
            ),
            final_state: Some(EvalFinalStateCapture {
                session_digest: format!("session:{sample_id}"),
                output_digest: Some(format!("output:{sample_id}")),
                artifact_digests: vec![format!("artifact:{sample_id}")],
            }),
            execution_strategy: Some(EvalExecutionStrategyFacts {
                strategy_label: String::from("operator_simulation"),
                runtime_family: Some(String::from("apple_foundation_models")),
                scheduler_posture: Some(String::from("single_host")),
            }),
        }
    }

    fn exact_outputs(
        dataset: &AppleAdapterDatasetContract,
    ) -> Vec<AppleAdapterObservedSampleOutput> {
        dataset
            .samples
            .iter()
            .map(|sample| {
                let expected_text = sample
                    .messages
                    .last()
                    .map(|message| message.content.clone())
                    .unwrap_or_default();
                let mut observed = AppleAdapterObservedSampleOutput::from_text(
                    sample.sample_id.clone(),
                    expected_text,
                );
                if let Some(structured) = sample.structured_assistant_output.clone() {
                    observed = observed.with_structured_output(structured);
                }
                if !sample.tools.is_empty() {
                    observed = observed.with_tool_calls(
                        sample
                            .tools
                            .iter()
                            .map(|tool| AppleAdapterObservedToolCall {
                                tool_name: tool.function.name.clone(),
                                succeeded: true,
                                arguments: None,
                            })
                            .collect(),
                    );
                }
                observed.with_verification(benchmark_verification(sample.sample_id.as_str()))
            })
            .collect()
    }

    fn base_outputs(
        dataset: &AppleAdapterDatasetContract,
    ) -> Vec<AppleAdapterObservedSampleOutput> {
        let wrong_text = BTreeMap::from([
            (
                String::from("sample-000001"),
                String::from("Psionic is still only planning decentralized adapter training."),
            ),
            (
                String::from("sample-000003"),
                String::from(
                    "{\"apple_lane\": \"distributed_cluster\", \"decentralized_adapter\": \"planned\"}",
                ),
            ),
            (
                String::from("sample-000005"),
                String::from(
                    "Yes. The current Apple lane already trains across multiple machines.",
                ),
            ),
            (
                String::from("sample-000006"),
                String::from("Yes. The Foundation Models bridge performs the training math."),
            ),
            (
                String::from("sample-000007"),
                String::from(
                    "The latest adapter will definitely be compatible with today's runtime assets.",
                ),
            ),
        ]);
        dataset
            .samples
            .iter()
            .map(|sample| {
                let expected_text = sample
                    .messages
                    .last()
                    .map(|message| message.content.clone())
                    .unwrap_or_default();
                let output_text = wrong_text
                    .get(sample.sample_id.as_str())
                    .cloned()
                    .unwrap_or(expected_text);
                let mut observed = AppleAdapterObservedSampleOutput::from_text(
                    sample.sample_id.clone(),
                    output_text,
                );
                if let Some(structured) = sample.structured_assistant_output.clone() {
                    observed = observed.with_structured_output(structured);
                }
                if !sample.tools.is_empty() {
                    observed = observed.with_tool_calls(
                        sample
                            .tools
                            .iter()
                            .map(|tool| AppleAdapterObservedToolCall {
                                tool_name: tool.function.name.clone(),
                                succeeded: true,
                                arguments: None,
                            })
                            .collect(),
                    );
                }
                observed.with_verification(benchmark_verification(sample.sample_id.as_str()))
            })
            .collect()
    }

    fn semantically_better_refusal_outputs(
        dataset: &AppleAdapterDatasetContract,
    ) -> Vec<AppleAdapterObservedSampleOutput> {
        dataset
            .samples
            .iter()
            .map(|sample| {
                let output_text = match sample.sample_id.as_str() {
                    "sample-000007" => String::from(
                        "I cannot answer that from static repo files alone. The exact compatibility result depends on the current Apple runtime and installed assets, so it has to be checked with live runtime validation.",
                    ),
                    "sample-000006" => String::from(
                        "Yes, the Foundation Models bridge performs the repo's Apple adapter training math.",
                    ),
                    _ => sample
                        .messages
                        .last()
                        .map(|message| message.content.clone())
                        .unwrap_or_default(),
                };
                let mut observed =
                    AppleAdapterObservedSampleOutput::from_text(sample.sample_id.clone(), output_text);
                if let Some(structured) = sample.structured_assistant_output.clone() {
                    observed = observed.with_structured_output(structured);
                }
                if !sample.tools.is_empty() {
                    observed = observed.with_tool_calls(
                        sample
                            .tools
                            .iter()
                            .map(|tool| AppleAdapterObservedToolCall {
                                tool_name: tool.function.name.clone(),
                                succeeded: true,
                                arguments: None,
                            })
                            .collect(),
                    );
                }
                observed.with_verification(benchmark_verification(sample.sample_id.as_str()))
            })
            .collect()
    }

    #[test]
    fn curated_benchmark_package_carries_task_family_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = corpus_manifest();
        let benchmark_key = architecture_explainer_benchmark_key(&corpus)?;
        let package = build_curated_benchmark_package(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            benchmark_key,
            &dataset_contract(),
            &corpus,
            1,
        )?;
        assert_eq!(
            package
                .cases
                .iter()
                .find(|case| case.case_id == "sample-000005")
                .and_then(|case| case.metadata.get("task_family"))
                .and_then(Value::as_str),
            Some("negative_refusal_correction")
        );
        Ok(())
    }

    #[test]
    fn architecture_explainer_base_vs_adapter_gate_accepts_better_adapter()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = dataset_contract();
        let corpus = corpus_manifest();
        let benchmark_key = architecture_explainer_benchmark_key(&corpus)?;
        let package = build_curated_benchmark_package(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            benchmark_key,
            &dataset,
            &corpus,
            1,
        )?;
        let report = run_curated_base_vs_adapter_benchmark(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            &package,
            &dataset,
            &corpus,
            base_outputs(&dataset),
            exact_outputs(&dataset),
            &AppleAdapterBaseVsAdapterAcceptancePolicy::architecture_explainer_default(),
            1_000,
            2_000,
        )?;
        assert!(report.acceptance.accepted);
        assert!(report.acceptance.aggregate_score_delta_bps > 0);
        assert!(report.acceptance.aggregate_pass_rate_delta_bps > 0);
        assert!(
            report
                .task_family_deltas
                .iter()
                .any(|delta| delta.task_family
                    == AppleAdapterCorpusTaskFamily::NegativeRefusalCorrection)
        );
        Ok(())
    }

    #[test]
    fn architecture_explainer_base_vs_adapter_gate_rejects_non_improving_adapter()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = dataset_contract();
        let corpus = corpus_manifest();
        let benchmark_key = architecture_explainer_benchmark_key(&corpus)?;
        let package = build_curated_benchmark_package(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            benchmark_key,
            &dataset,
            &corpus,
            1,
        )?;
        let report = run_curated_base_vs_adapter_benchmark(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            &package,
            &dataset,
            &corpus,
            base_outputs(&dataset),
            base_outputs(&dataset),
            &AppleAdapterBaseVsAdapterAcceptancePolicy::architecture_explainer_default(),
            1_000,
            2_000,
        )?;
        assert!(!report.acceptance.accepted);
        assert!(
            report
                .acceptance
                .reason_codes
                .contains(&AppleAdapterBenchmarkAcceptanceReasonCode::ScoreDeltaBelowMinimum)
        );
        assert!(
            report.acceptance.reason_codes.contains(
                &AppleAdapterBenchmarkAcceptanceReasonCode::ImprovedCaseCountBelowMinimum
            )
        );
        Ok(())
    }

    #[test]
    fn curated_benchmark_behavior_scoring_recognizes_grounded_refusal_improvement()
    -> Result<(), Box<dyn std::error::Error>> {
        let dataset = dataset_contract();
        let corpus = corpus_manifest();
        let benchmark_key = architecture_explainer_benchmark_key(&corpus)?;
        let package = build_curated_benchmark_package(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            benchmark_key,
            &dataset,
            &corpus,
            1,
        )?;
        let report = run_curated_base_vs_adapter_benchmark(
            &AppleAdapterEvalHarness::new(environment_bundle())?,
            &package,
            &dataset,
            &corpus,
            base_outputs(&dataset),
            semantically_better_refusal_outputs(&dataset),
            &AppleAdapterBaseVsAdapterAcceptancePolicy {
                minimum_adapter_score_bps: 1,
                minimum_adapter_pass_rate_bps: 1,
                minimum_score_delta_bps: 1,
                minimum_pass_rate_delta_bps: 1,
                minimum_improved_case_count: 1,
            },
            1_000,
            2_000,
        )?;
        assert!(report.adapted_summary.aggregate_score_bps.unwrap_or(0) > 0);
        assert!(report.adapted_summary.aggregate_pass_rate_bps > 0);
        assert!(report.acceptance.improved_case_count >= 1);
        assert!(
            report
                .adapted_eval_run
                .samples
                .iter()
                .find(|sample| sample.sample_id == "sample-000007")
                .and_then(|sample| sample
                    .metadata
                    .get("apple_adapter.benchmark_behavior_passed"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        );
        assert_eq!(
            report
                .adapted_eval_run
                .samples
                .iter()
                .find(|sample| sample.sample_id == "sample-000006")
                .and_then(|sample| sample
                    .metadata
                    .get("apple_adapter.benchmark_behavior_passed"))
                .and_then(Value::as_bool),
            Some(false)
        );
        Ok(())
    }
}
