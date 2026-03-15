use std::collections::BTreeMap;

use psionic_data::{
    AppleAdapterCorpusExpectedBehavior, AppleAdapterCorpusTaskFamily,
    AppleAdapterCuratedCorpusError, AppleAdapterCuratedCorpusManifest, AppleAdapterCuratedSplit,
    AppleAdapterDatasetContract,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    AppleAdapterEvalError, AppleAdapterEvalHarness, AppleAdapterObservedSampleOutput,
    BenchmarkAggregateSummary, BenchmarkExecutionMode, BenchmarkPackage, BenchmarkPackageKey,
    EvalRunMode, EvalRunState, EvalRunStatus, EvalRuntimeError, EvalSampleStatus,
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

    let mut base_execution = benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    base_execution.record_round(base_run)?;
    let base_summary = base_execution.finalize()?;

    let mut adapted_execution = benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    adapted_execution.record_round(adapted_run)?;
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
}
