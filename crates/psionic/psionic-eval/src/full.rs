//! Rust-native held-out eval, rubric, and benchmark runtime contracts for
//! Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

#[path = "apple_adapter.rs"]
mod apple_adapter;
#[path = "apple_adapter_benchmark.rs"]
mod apple_adapter_benchmark;
#[path = "tassadar.rs"]
mod tassadar;

use std::collections::{BTreeMap, BTreeSet};

use psionic_data::DatasetKey;
use psionic_datastream::{DatastreamManifestRef, DatastreamSubjectKind};
use psionic_environments::{
    EnvironmentPackageContract, EnvironmentPackageKey, EnvironmentRubricScoreKind,
    EnvironmentSessionSummary,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use apple_adapter::*;
pub use apple_adapter_benchmark::*;
pub use tassadar::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "held-out eval, benchmark packages, and rubric runtime for Psionic";

/// Stable ABI version for Psionic-native eval runs.
pub const EVAL_ABI_VERSION: &str = "psionic.eval.v1";

/// Stable ABI version for validator-style benchmark packages.
pub const BENCHMARK_PACKAGE_ABI_VERSION: &str = "psionic.benchmark.v1";

/// Evaluation mode for one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalRunMode {
    /// Offline held-out evaluation against a fixed sample set.
    OfflineHeldOut,
    /// Online or shadow evaluation over live traffic or live tasks.
    OnlineShadow,
    /// Benchmark-class evaluation under a packaged benchmark contract.
    Benchmark,
}

/// Lifecycle state for one eval run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalRunStatus {
    /// Created but not yet started.
    Queued,
    /// Currently collecting samples.
    Running,
    /// Finalized successfully.
    Finalized,
    /// Failed terminally.
    Failed,
    /// Cancelled terminally.
    Cancelled,
}

/// Terminal sample status for one eval case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalSampleStatus {
    /// The sample passed.
    Passed,
    /// The sample failed.
    Failed,
    /// The sample errored before scoring could complete.
    Errored,
}

/// Robust aggregation mode for repeated benchmark runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkAggregationKind {
    /// Median score across repeated runs.
    MedianScore,
    /// Mean score across repeated runs.
    MeanScore,
}

/// Execution mode for a benchmark package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkExecutionMode {
    /// The validator-owned execution path.
    Validator,
    /// The local operator path that simulates validator execution.
    OperatorSimulation,
}

/// One reusable metric emitted by an eval sample or summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalMetric {
    /// Stable metric identifier.
    pub metric_id: String,
    /// Numeric metric value.
    pub metric_value: f64,
    /// Optional unit label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
}

impl EvalMetric {
    /// Creates a metric.
    #[must_use]
    pub fn new(metric_id: impl Into<String>, metric_value: f64) -> Self {
        Self {
            metric_id: metric_id.into(),
            metric_value,
            unit: None,
            metadata: Value::Null,
        }
    }

    /// Attaches a unit label.
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Attaches metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// One artifact surfaced by an eval sample or summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalArtifact {
    /// Stable artifact kind.
    pub artifact_kind: String,
    /// Stable artifact reference.
    pub artifact_ref: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl EvalArtifact {
    /// Creates an eval artifact and derives a stable digest from visible identity.
    #[must_use]
    pub fn new(
        artifact_kind: impl Into<String>,
        artifact_ref: impl Into<String>,
        artifact_bytes: &[u8],
    ) -> Self {
        let artifact_kind = artifact_kind.into();
        let artifact_ref = artifact_ref.into();
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_eval_artifact|");
        hasher.update(artifact_kind.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact_ref.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact_bytes);
        Self {
            artifact_kind,
            artifact_ref,
            artifact_digest: hex::encode(hasher.finalize()),
        }
    }
}

/// Timer-integrity facts surfaced for validator policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalTimerIntegrityFacts {
    /// Declared time budget when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_budget_ms: Option<u64>,
    /// Observed elapsed wall-clock time.
    pub elapsed_ms: u64,
    /// Whether the run stayed within the declared budget.
    pub within_budget: bool,
}

/// Token-accounting facts surfaced for validator policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalTokenAccountingFacts {
    /// Input token count.
    pub input_tokens: u32,
    /// Output token count.
    pub output_tokens: u32,
    /// Total token count.
    pub total_tokens: u32,
}

impl EvalTokenAccountingFacts {
    /// Creates token-accounting facts and validates the total.
    pub fn new(
        input_tokens: u32,
        output_tokens: u32,
        total_tokens: u32,
    ) -> Result<Self, EvalRuntimeError> {
        if input_tokens.saturating_add(output_tokens) != total_tokens {
            return Err(EvalRuntimeError::TokenAccountingMismatch {
                input_tokens,
                output_tokens,
                total_tokens,
            });
        }
        Ok(Self {
            input_tokens,
            output_tokens,
            total_tokens,
        })
    }
}

/// Final-state capture surfaced for validator policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalFinalStateCapture {
    /// Stable session digest or equivalent final state digest.
    pub session_digest: String,
    /// Optional final output digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_digest: Option<String>,
    /// Artifact digests emitted during execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_digests: Vec<String>,
}

/// Declared execution strategy surfaced for validator policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalExecutionStrategyFacts {
    /// Stable strategy label.
    pub strategy_label: String,
    /// Optional runtime family.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_family: Option<String>,
    /// Optional scheduler or execution posture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_posture: Option<String>,
}

/// Grouped verification facts attached to an eval sample.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvalVerificationFacts {
    /// Timer-integrity facts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timer_integrity: Option<EvalTimerIntegrityFacts>,
    /// Token-accounting facts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_accounting: Option<EvalTokenAccountingFacts>,
    /// Final-state capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_state: Option<EvalFinalStateCapture>,
    /// Declared execution strategy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_strategy: Option<EvalExecutionStrategyFacts>,
}

/// One scored or errored eval sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalSampleRecord {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Optional ordinal in the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u64>,
    /// Environment package identity used for the sample.
    pub environment: EnvironmentPackageKey,
    /// Terminal sample status.
    pub status: EvalSampleStatus,
    /// Optional input reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_ref: Option<String>,
    /// Optional output reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_ref: Option<String>,
    /// Optional expected output reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output_ref: Option<String>,
    /// Aggregate score in basis points when scoring succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_bps: Option<u32>,
    /// Sample metrics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<EvalMetric>,
    /// Sample artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<EvalArtifact>,
    /// Optional error reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    /// Optional verification facts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification: Option<EvalVerificationFacts>,
    /// Optional session digest copied from environment summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_digest: Option<String>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl EvalSampleRecord {
    /// Builds a sample record from an environment session summary and the
    /// package contract that defined the rubric hooks.
    pub fn from_environment_summary(
        sample_id: impl Into<String>,
        ordinal: Option<u64>,
        input_ref: Option<String>,
        output_ref: Option<String>,
        expected_output_ref: Option<String>,
        package: &EnvironmentPackageContract,
        summary: &EnvironmentSessionSummary,
        verification: Option<EvalVerificationFacts>,
    ) -> Result<Self, EvalRuntimeError> {
        package
            .validate()
            .map_err(EvalRuntimeError::EnvironmentContract)?;
        if package.key != summary.package_key {
            return Err(EvalRuntimeError::EnvironmentSummaryPackageMismatch {
                expected: package.storage_key(),
                actual: summary.package_key.storage_key(),
            });
        }
        let sample_id = sample_id.into();
        if sample_id.trim().is_empty() {
            return Err(EvalRuntimeError::MissingSampleId);
        }

        let mut metrics = Vec::new();
        let mut total_score_bps = 0_u32;
        let mut rubric_count = 0_u32;
        let mut all_passed = true;
        for rubric_hook in &package.rubric_hooks {
            let Some(outcome) = summary
                .rubric_outcomes
                .iter()
                .find(|outcome| outcome.rubric_ref == rubric_hook.rubric_ref)
            else {
                return Err(EvalRuntimeError::MissingRubricOutcome {
                    rubric_ref: rubric_hook.rubric_ref.clone(),
                });
            };
            let rubric_score_bps = match rubric_hook.score_kind {
                EnvironmentRubricScoreKind::Binary => {
                    if outcome.passed {
                        10_000
                    } else {
                        0
                    }
                }
                EnvironmentRubricScoreKind::Scalar => outcome.score_value.clamp(0, 10_000) as u32,
            };
            metrics.push(
                EvalMetric::new(
                    rubric_hook.rubric_ref.clone(),
                    f64::from(rubric_score_bps) / 10_000.0,
                )
                .with_unit("fraction")
                .with_metadata(serde_json::json!({
                    "score_kind": match rubric_hook.score_kind {
                        EnvironmentRubricScoreKind::Binary => "binary",
                        EnvironmentRubricScoreKind::Scalar => "scalar",
                    },
                    "passed": outcome.passed,
                    "hook_name": rubric_hook.hook_name,
                })),
            );
            total_score_bps = total_score_bps.saturating_add(rubric_score_bps);
            rubric_count = rubric_count.saturating_add(1);
            all_passed &= outcome.passed;
        }
        let score_bps = if rubric_count == 0 {
            None
        } else {
            Some(total_score_bps / rubric_count)
        };
        let artifacts = summary
            .artifacts
            .iter()
            .map(|artifact| EvalArtifact {
                artifact_kind: artifact.artifact_kind.clone(),
                artifact_ref: artifact.artifact_ref.clone(),
                artifact_digest: artifact.artifact_digest.clone(),
            })
            .collect::<Vec<_>>();
        Ok(Self {
            sample_id,
            ordinal,
            environment: summary.package_key.clone(),
            status: if all_passed {
                EvalSampleStatus::Passed
            } else {
                EvalSampleStatus::Failed
            },
            input_ref,
            output_ref,
            expected_output_ref,
            score_bps,
            metrics,
            artifacts,
            error_reason: None,
            verification,
            session_digest: Some(summary.session_digest.clone()),
            metadata: BTreeMap::new(),
        })
    }

    /// Creates an explicit errored sample.
    #[must_use]
    pub fn errored(
        sample_id: impl Into<String>,
        environment: EnvironmentPackageKey,
        error_reason: impl Into<String>,
    ) -> Self {
        Self {
            sample_id: sample_id.into(),
            ordinal: None,
            environment,
            status: EvalSampleStatus::Errored,
            input_ref: None,
            output_ref: None,
            expected_output_ref: None,
            score_bps: None,
            metrics: Vec::new(),
            artifacts: Vec::new(),
            error_reason: Some(error_reason.into()),
            verification: None,
            session_digest: None,
            metadata: BTreeMap::new(),
        }
    }
}

/// Aggregate summary for one eval run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalSummary {
    /// Total sample count.
    pub total_samples: u64,
    /// Samples that carried a score.
    pub scored_samples: u64,
    /// Samples that passed.
    pub passed_samples: u64,
    /// Samples that failed.
    pub failed_samples: u64,
    /// Samples that errored.
    pub errored_samples: u64,
    /// Average score in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_score_bps: Option<u32>,
    /// Pass rate in basis points.
    pub pass_rate_bps: u32,
    /// Aggregate metrics by metric id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aggregate_metrics: Vec<EvalMetric>,
    /// Aggregate run artifacts surfaced at finalize time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<EvalArtifact>,
    /// Stable digest over the summary.
    pub summary_digest: String,
}

/// Full contract for one eval run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalRunContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable eval run identifier.
    pub eval_run_id: String,
    /// Eval mode.
    pub mode: EvalRunMode,
    /// Environment package identity.
    pub environment: EnvironmentPackageKey,
    /// Optional dataset identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetKey>,
    /// Optional split name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Optional model reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_ref: Option<String>,
    /// Optional source reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Optional policy revision reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_revision_id: Option<String>,
    /// Expected sample count when bounded ahead of time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sample_count: Option<u64>,
    /// Optional benchmark package identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_package: Option<BenchmarkPackageKey>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl EvalRunContract {
    /// Creates a run contract.
    #[must_use]
    pub fn new(
        eval_run_id: impl Into<String>,
        mode: EvalRunMode,
        environment: EnvironmentPackageKey,
    ) -> Self {
        Self {
            abi_version: String::from(EVAL_ABI_VERSION),
            eval_run_id: eval_run_id.into(),
            mode,
            environment,
            dataset: None,
            split: None,
            model_ref: None,
            source_ref: None,
            policy_revision_id: None,
            expected_sample_count: None,
            benchmark_package: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Attaches a dataset binding.
    #[must_use]
    pub fn with_dataset(mut self, dataset: DatasetKey, split: Option<String>) -> Self {
        self.dataset = Some(dataset);
        self.split = split;
        self
    }

    /// Attaches an expected sample count.
    #[must_use]
    pub const fn with_expected_sample_count(mut self, expected_sample_count: u64) -> Self {
        self.expected_sample_count = Some(expected_sample_count);
        self
    }

    /// Attaches a benchmark package.
    #[must_use]
    pub fn with_benchmark_package(mut self, benchmark_package: BenchmarkPackageKey) -> Self {
        self.benchmark_package = Some(benchmark_package);
        self
    }

    /// Returns a stable digest over the run contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_eval_run_contract|");
        hasher.update(self.abi_version.as_bytes());
        hasher.update(b"|");
        hasher.update(self.eval_run_id.as_bytes());
        hasher.update(b"|");
        hasher.update(eval_run_mode_label(self.mode));
        hasher.update(b"|");
        hasher.update(self.environment.storage_key().as_bytes());
        if let Some(dataset) = &self.dataset {
            hasher.update(b"|dataset|");
            hasher.update(dataset.storage_key().as_bytes());
        }
        if let Some(split) = &self.split {
            hasher.update(b"|split|");
            hasher.update(split.as_bytes());
        }
        if let Some(model_ref) = &self.model_ref {
            hasher.update(b"|model|");
            hasher.update(model_ref.as_bytes());
        }
        if let Some(source_ref) = &self.source_ref {
            hasher.update(b"|source|");
            hasher.update(source_ref.as_bytes());
        }
        if let Some(policy_revision_id) = &self.policy_revision_id {
            hasher.update(b"|policy|");
            hasher.update(policy_revision_id.as_bytes());
        }
        if let Some(expected_sample_count) = self.expected_sample_count {
            hasher.update(b"|expected|");
            hasher.update(expected_sample_count.to_string().as_bytes());
        }
        if let Some(benchmark_package) = &self.benchmark_package {
            hasher.update(b"|benchmark|");
            hasher.update(benchmark_package.storage_key().as_bytes());
        }
        hasher.update(stable_json_bytes(&self.metadata));
        hex::encode(hasher.finalize())
    }

    /// Validates the run contract.
    pub fn validate(&self) -> Result<(), EvalRuntimeError> {
        if self.abi_version != EVAL_ABI_VERSION {
            return Err(EvalRuntimeError::UnsupportedEvalAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.eval_run_id.trim().is_empty() {
            return Err(EvalRuntimeError::MissingEvalRunId);
        }
        if self.environment.environment_ref.trim().is_empty() {
            return Err(EvalRuntimeError::MissingEnvironmentRef);
        }
        if self.environment.version.trim().is_empty() {
            return Err(EvalRuntimeError::MissingEnvironmentVersion);
        }
        if self
            .expected_sample_count
            .is_some_and(|expected_sample_count| expected_sample_count == 0)
        {
            return Err(EvalRuntimeError::InvalidExpectedSampleCount);
        }
        if self.mode == EvalRunMode::Benchmark && self.benchmark_package.is_none() {
            return Err(EvalRuntimeError::BenchmarkPackageMissingForBenchmarkMode);
        }
        if let Some(dataset) = &self.dataset {
            if dataset.dataset_ref.trim().is_empty() {
                return Err(EvalRuntimeError::MissingDatasetRef);
            }
            if dataset.version.trim().is_empty() {
                return Err(EvalRuntimeError::MissingDatasetVersion);
            }
        }
        Ok(())
    }
}

/// Mutable state machine for one local eval run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvalRunState {
    /// Run contract.
    pub contract: EvalRunContract,
    /// Current lifecycle status.
    pub status: EvalRunStatus,
    /// Start time when the run began.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    /// Finalize time when the run terminated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finalized_at_ms: Option<u64>,
    /// Recorded samples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<EvalSampleRecord>,
    /// Terminal summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<EvalSummary>,
    /// Aggregate artifacts emitted at finalize time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run_artifacts: Vec<EvalArtifact>,
}

impl EvalRunState {
    /// Opens a run state from a validated contract.
    pub fn open(contract: EvalRunContract) -> Result<Self, EvalRuntimeError> {
        contract.validate()?;
        Ok(Self {
            contract,
            status: EvalRunStatus::Queued,
            started_at_ms: None,
            finalized_at_ms: None,
            samples: Vec::new(),
            summary: None,
            run_artifacts: Vec::new(),
        })
    }

    /// Starts the run.
    pub fn start(&mut self, started_at_ms: u64) -> Result<(), EvalRuntimeError> {
        if started_at_ms == 0 {
            return Err(EvalRuntimeError::InvalidStartedAt);
        }
        if self.status != EvalRunStatus::Queued {
            return Err(EvalRuntimeError::RunNotStartable {
                eval_run_id: self.contract.eval_run_id.clone(),
                status: eval_run_status_label(self.status).to_string(),
            });
        }
        self.status = EvalRunStatus::Running;
        self.started_at_ms = Some(started_at_ms);
        Ok(())
    }

    /// Appends one sample.
    pub fn append_sample(&mut self, sample: EvalSampleRecord) -> Result<(), EvalRuntimeError> {
        if self.status != EvalRunStatus::Running {
            return Err(EvalRuntimeError::RunNotAcceptingSamples {
                eval_run_id: self.contract.eval_run_id.clone(),
                status: eval_run_status_label(self.status).to_string(),
            });
        }
        if sample.environment != self.contract.environment {
            return Err(EvalRuntimeError::SampleEnvironmentMismatch {
                expected: self.contract.environment.storage_key(),
                actual: sample.environment.storage_key(),
            });
        }
        if sample.sample_id.trim().is_empty() {
            return Err(EvalRuntimeError::MissingSampleId);
        }
        if self
            .samples
            .iter()
            .any(|existing| existing.sample_id == sample.sample_id)
        {
            return Err(EvalRuntimeError::DuplicateSample {
                sample_id: sample.sample_id,
            });
        }
        if self
            .contract
            .expected_sample_count
            .is_some_and(|expected_sample_count| self.samples.len() as u64 >= expected_sample_count)
        {
            return Err(EvalRuntimeError::ExpectedSampleCountExceeded {
                expected_sample_count: self.contract.expected_sample_count.unwrap_or_default(),
            });
        }
        self.samples.push(sample);
        Ok(())
    }

    /// Finalizes the run and derives the summary.
    pub fn finalize(
        &mut self,
        finalized_at_ms: u64,
        run_artifacts: Vec<EvalArtifact>,
    ) -> Result<&EvalSummary, EvalRuntimeError> {
        if self.status != EvalRunStatus::Running {
            return Err(EvalRuntimeError::RunNotFinalizable {
                eval_run_id: self.contract.eval_run_id.clone(),
                status: eval_run_status_label(self.status).to_string(),
            });
        }
        let started_at_ms = self
            .started_at_ms
            .ok_or(EvalRuntimeError::RunNeverStarted)?;
        if finalized_at_ms < started_at_ms {
            return Err(EvalRuntimeError::FinalizeBeforeStart {
                started_at_ms,
                finalized_at_ms,
            });
        }
        if self
            .contract
            .expected_sample_count
            .is_some_and(|expected_sample_count| self.samples.len() as u64 != expected_sample_count)
        {
            return Err(EvalRuntimeError::ExpectedSampleCountIncomplete {
                expected_sample_count: self.contract.expected_sample_count.unwrap_or_default(),
                actual_sample_count: self.samples.len() as u64,
            });
        }
        self.run_artifacts = run_artifacts;
        self.summary = Some(build_summary(
            self.contract.stable_digest().as_str(),
            self.samples.as_slice(),
            self.run_artifacts.as_slice(),
        ));
        self.status = EvalRunStatus::Finalized;
        self.finalized_at_ms = Some(finalized_at_ms);
        self.summary
            .as_ref()
            .ok_or(EvalRuntimeError::SummaryMissing)
    }
}

/// Stable identity for one benchmark package.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BenchmarkPackageKey {
    /// Stable benchmark reference.
    pub benchmark_ref: String,
    /// Immutable benchmark package version.
    pub version: String,
}

impl BenchmarkPackageKey {
    /// Creates a key.
    #[must_use]
    pub fn new(benchmark_ref: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            benchmark_ref: benchmark_ref.into(),
            version: version.into(),
        }
    }

    /// Returns the canonical `benchmark_ref@version` storage key.
    #[must_use]
    pub fn storage_key(&self) -> String {
        format!("{}@{}", self.benchmark_ref, self.version)
    }
}

/// Verification policy for validator-style benchmark execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BenchmarkVerificationPolicy {
    /// Whether timer-integrity facts are required.
    pub require_timer_integrity: bool,
    /// Whether token-accounting facts are required.
    pub require_token_accounting: bool,
    /// Whether final-state capture is required.
    pub require_final_state_capture: bool,
    /// Whether execution-strategy facts are required.
    pub require_execution_strategy: bool,
}

/// One case inside a benchmark package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Optional ordinal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u64>,
    /// Optional input reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_ref: Option<String>,
    /// Optional expected output reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output_ref: Option<String>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
}

impl BenchmarkCase {
    /// Creates a benchmark case.
    #[must_use]
    pub fn new(case_id: impl Into<String>) -> Self {
        Self {
            case_id: case_id.into(),
            ordinal: None,
            input_ref: None,
            expected_output_ref: None,
            metadata: Value::Null,
        }
    }
}

/// Packaged benchmark contract shared by validator and local simulation paths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkPackage {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable benchmark identity.
    pub key: BenchmarkPackageKey,
    /// Human-readable benchmark name.
    pub display_name: String,
    /// Environment package identity.
    pub environment: EnvironmentPackageKey,
    /// Optional dataset identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetKey>,
    /// Optional split name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Optional eval bundle manifest ref.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_bundle: Option<DatastreamManifestRef>,
    /// Number of repeat runs the package requires.
    pub repeat_count: u32,
    /// Robust aggregation mode.
    pub aggregation: BenchmarkAggregationKind,
    /// Verification policy expected by validator logic.
    pub verification_policy: BenchmarkVerificationPolicy,
    /// Cases in the benchmark package.
    pub cases: Vec<BenchmarkCase>,
    /// Extension metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl BenchmarkPackage {
    /// Creates a benchmark package.
    #[must_use]
    pub fn new(
        key: BenchmarkPackageKey,
        display_name: impl Into<String>,
        environment: EnvironmentPackageKey,
        repeat_count: u32,
        aggregation: BenchmarkAggregationKind,
    ) -> Self {
        Self {
            abi_version: String::from(BENCHMARK_PACKAGE_ABI_VERSION),
            key,
            display_name: display_name.into(),
            environment,
            dataset: None,
            split: None,
            eval_bundle: None,
            repeat_count,
            aggregation,
            verification_policy: BenchmarkVerificationPolicy::default(),
            cases: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Attaches a dataset binding.
    #[must_use]
    pub fn with_dataset(mut self, dataset: DatasetKey, split: Option<String>) -> Self {
        self.dataset = Some(dataset);
        self.split = split;
        self
    }

    /// Attaches an eval bundle.
    #[must_use]
    pub fn with_eval_bundle(mut self, eval_bundle: DatastreamManifestRef) -> Self {
        self.eval_bundle = Some(eval_bundle);
        self
    }

    /// Attaches verification policy.
    #[must_use]
    pub fn with_verification_policy(
        mut self,
        verification_policy: BenchmarkVerificationPolicy,
    ) -> Self {
        self.verification_policy = verification_policy;
        self
    }

    /// Attaches cases.
    #[must_use]
    pub fn with_cases(mut self, cases: Vec<BenchmarkCase>) -> Self {
        self.cases = cases;
        self
    }

    /// Returns a stable digest over the package.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_benchmark_package|");
        hasher.update(self.abi_version.as_bytes());
        hasher.update(b"|");
        hasher.update(self.key.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(self.display_name.as_bytes());
        hasher.update(b"|");
        hasher.update(self.environment.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(self.repeat_count.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(benchmark_aggregation_kind_label(self.aggregation));
        if let Some(dataset) = &self.dataset {
            hasher.update(b"|dataset|");
            hasher.update(dataset.storage_key().as_bytes());
        }
        if let Some(split) = &self.split {
            hasher.update(b"|split|");
            hasher.update(split.as_bytes());
        }
        if let Some(eval_bundle) = &self.eval_bundle {
            hasher.update(b"|bundle|");
            hasher.update(eval_bundle.manifest_digest.as_bytes());
        }
        hasher.update(stable_json_bytes(&self.verification_policy));
        for case in &self.cases {
            hasher.update(b"|case|");
            hasher.update(case.case_id.as_bytes());
            if let Some(ordinal) = case.ordinal {
                hasher.update(b"|");
                hasher.update(ordinal.to_string().as_bytes());
            }
        }
        hasher.update(stable_json_bytes(&self.metadata));
        hex::encode(hasher.finalize())
    }

    /// Validates the benchmark package.
    pub fn validate(&self) -> Result<(), EvalRuntimeError> {
        if self.abi_version != BENCHMARK_PACKAGE_ABI_VERSION {
            return Err(EvalRuntimeError::UnsupportedBenchmarkAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.key.benchmark_ref.trim().is_empty() {
            return Err(EvalRuntimeError::MissingBenchmarkRef);
        }
        if self.key.version.trim().is_empty() {
            return Err(EvalRuntimeError::MissingBenchmarkVersion);
        }
        if self.display_name.trim().is_empty() {
            return Err(EvalRuntimeError::MissingBenchmarkDisplayName);
        }
        if self.environment.environment_ref.trim().is_empty() {
            return Err(EvalRuntimeError::MissingEnvironmentRef);
        }
        if self.environment.version.trim().is_empty() {
            return Err(EvalRuntimeError::MissingEnvironmentVersion);
        }
        if self.repeat_count == 0 {
            return Err(EvalRuntimeError::InvalidBenchmarkRepeatCount);
        }
        if self.cases.is_empty() {
            return Err(EvalRuntimeError::BenchmarkCasesMissing);
        }
        let mut case_ids = BTreeSet::new();
        for case in &self.cases {
            if case.case_id.trim().is_empty() {
                return Err(EvalRuntimeError::MissingBenchmarkCaseId);
            }
            if !case_ids.insert(case.case_id.clone()) {
                return Err(EvalRuntimeError::DuplicateBenchmarkCase {
                    case_id: case.case_id.clone(),
                });
            }
        }
        if let Some(eval_bundle) = &self.eval_bundle {
            if eval_bundle.subject != DatastreamSubjectKind::EvalBundle {
                return Err(EvalRuntimeError::BenchmarkEvalBundleSubjectMismatch {
                    subject: String::from(eval_bundle.subject.as_str()),
                });
            }
        }
        Ok(())
    }

    /// Opens a benchmark execution session that can represent either validator
    /// execution or a local operator simulation.
    pub fn open_execution(
        self,
        execution_mode: BenchmarkExecutionMode,
    ) -> Result<BenchmarkExecutionSession, EvalRuntimeError> {
        self.validate()?;
        Ok(BenchmarkExecutionSession {
            package: self,
            execution_mode,
            rounds: Vec::new(),
        })
    }
}

/// One recorded repeated benchmark round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRoundReceipt {
    /// One-based round index.
    pub round_index: u32,
    /// Execution mode used for the round.
    pub execution_mode: BenchmarkExecutionMode,
    /// Eval run identifier.
    pub eval_run_id: String,
    /// Summary emitted by the round.
    pub summary: EvalSummary,
    /// Stable digest over the round receipt.
    pub round_digest: String,
}

/// Aggregate result over repeated benchmark rounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkAggregateSummary {
    /// Benchmark package identity.
    pub package_key: BenchmarkPackageKey,
    /// Execution mode used for aggregation.
    pub execution_mode: BenchmarkExecutionMode,
    /// Aggregation mode.
    pub aggregation: BenchmarkAggregationKind,
    /// Number of recorded rounds.
    pub round_count: u32,
    /// Aggregate score in basis points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_score_bps: Option<u32>,
    /// Aggregate pass rate in basis points.
    pub aggregate_pass_rate_bps: u32,
    /// Per-round average scores.
    pub per_round_scores_bps: Vec<u32>,
    /// Per-round pass rates.
    pub per_round_pass_rates_bps: Vec<u32>,
    /// Stable digest over the aggregate.
    pub summary_digest: String,
}

/// Execution session over one benchmark package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkExecutionSession {
    package: BenchmarkPackage,
    execution_mode: BenchmarkExecutionMode,
    rounds: Vec<BenchmarkRoundReceipt>,
}

impl BenchmarkExecutionSession {
    /// Records one finalized eval run as a benchmark round.
    pub fn record_round(&mut self, eval_run: &EvalRunState) -> Result<(), EvalRuntimeError> {
        if eval_run.status != EvalRunStatus::Finalized {
            return Err(EvalRuntimeError::BenchmarkRoundNotFinalized {
                eval_run_id: eval_run.contract.eval_run_id.clone(),
                status: eval_run_status_label(eval_run.status).to_string(),
            });
        }
        if eval_run.contract.environment != self.package.environment {
            return Err(EvalRuntimeError::BenchmarkEnvironmentMismatch {
                expected: self.package.environment.storage_key(),
                actual: eval_run.contract.environment.storage_key(),
            });
        }
        if eval_run.contract.mode != EvalRunMode::Benchmark {
            return Err(EvalRuntimeError::EvalRunNotBenchmarkMode {
                eval_run_id: eval_run.contract.eval_run_id.clone(),
            });
        }
        if eval_run.contract.benchmark_package.as_ref() != Some(&self.package.key) {
            return Err(EvalRuntimeError::BenchmarkPackageMismatch {
                expected: self.package.key.storage_key(),
                actual: eval_run
                    .contract
                    .benchmark_package
                    .as_ref()
                    .map_or_else(String::new, BenchmarkPackageKey::storage_key),
            });
        }
        if eval_run.samples.len() != self.package.cases.len() {
            return Err(EvalRuntimeError::BenchmarkCaseCountMismatch {
                expected: self.package.cases.len() as u64,
                actual: eval_run.samples.len() as u64,
            });
        }
        for sample in &eval_run.samples {
            validate_verification_policy(
                sample,
                &self.package.verification_policy,
                eval_run.contract.eval_run_id.as_str(),
            )?;
        }
        let summary = eval_run
            .summary
            .clone()
            .ok_or(EvalRuntimeError::SummaryMissing)?;
        let round_index = self.rounds.len() as u32 + 1;
        let round_digest = stable_benchmark_round_digest(
            self.package.key.storage_key().as_str(),
            round_index,
            self.execution_mode,
            summary.summary_digest.as_str(),
        );
        self.rounds.push(BenchmarkRoundReceipt {
            round_index,
            execution_mode: self.execution_mode,
            eval_run_id: eval_run.contract.eval_run_id.clone(),
            summary,
            round_digest,
        });
        Ok(())
    }

    /// Finalizes repeated rounds into one aggregate result.
    pub fn finalize(&self) -> Result<BenchmarkAggregateSummary, EvalRuntimeError> {
        if self.rounds.is_empty() {
            return Err(EvalRuntimeError::BenchmarkRoundsMissing);
        }
        let scores = self
            .rounds
            .iter()
            .filter_map(|round| round.summary.average_score_bps)
            .collect::<Vec<_>>();
        let pass_rates = self
            .rounds
            .iter()
            .map(|round| round.summary.pass_rate_bps)
            .collect::<Vec<_>>();
        let aggregate_score_bps = if scores.is_empty() {
            None
        } else {
            Some(match self.package.aggregation {
                BenchmarkAggregationKind::MedianScore => robust_median(scores.as_slice()),
                BenchmarkAggregationKind::MeanScore => {
                    (scores.iter().copied().map(u64::from).sum::<u64>() / scores.len() as u64)
                        as u32
                }
            })
        };
        let aggregate_pass_rate_bps = match self.package.aggregation {
            BenchmarkAggregationKind::MedianScore => robust_median(pass_rates.as_slice()),
            BenchmarkAggregationKind::MeanScore => {
                (pass_rates.iter().copied().map(u64::from).sum::<u64>() / pass_rates.len() as u64)
                    as u32
            }
        };
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_benchmark_aggregate|");
        hasher.update(self.package.key.storage_key().as_bytes());
        hasher.update(b"|");
        hasher.update(benchmark_execution_mode_label(self.execution_mode));
        hasher.update(b"|");
        hasher.update(benchmark_aggregation_kind_label(self.package.aggregation));
        if let Some(aggregate_score_bps) = aggregate_score_bps {
            hasher.update(b"|score|");
            hasher.update(aggregate_score_bps.to_string().as_bytes());
        }
        hasher.update(b"|pass_rate|");
        hasher.update(aggregate_pass_rate_bps.to_string().as_bytes());
        for round in &self.rounds {
            hasher.update(b"|round|");
            hasher.update(round.round_digest.as_bytes());
        }
        Ok(BenchmarkAggregateSummary {
            package_key: self.package.key.clone(),
            execution_mode: self.execution_mode,
            aggregation: self.package.aggregation,
            round_count: self.rounds.len() as u32,
            aggregate_score_bps,
            aggregate_pass_rate_bps,
            per_round_scores_bps: scores,
            per_round_pass_rates_bps: pass_rates,
            summary_digest: hex::encode(hasher.finalize()),
        })
    }
}

/// Eval contract, runtime, or benchmark error.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EvalRuntimeError {
    /// Invalid environment contract passed through sample construction.
    #[error("{0}")]
    EnvironmentContract(#[from] psionic_environments::EnvironmentContractError),
    /// Unsupported eval ABI version.
    #[error("unsupported eval ABI version `{abi_version}`")]
    UnsupportedEvalAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Unsupported benchmark ABI version.
    #[error("unsupported benchmark ABI version `{abi_version}`")]
    UnsupportedBenchmarkAbiVersion {
        /// Observed ABI version.
        abi_version: String,
    },
    /// Missing eval run id.
    #[error("eval run is missing `eval_run_id`")]
    MissingEvalRunId,
    /// Missing environment ref.
    #[error("eval contract is missing `environment_ref`")]
    MissingEnvironmentRef,
    /// Missing environment version.
    #[error("eval contract is missing immutable environment `version`")]
    MissingEnvironmentVersion,
    /// Missing dataset ref.
    #[error("eval contract dataset binding is missing `dataset_ref`")]
    MissingDatasetRef,
    /// Missing dataset version.
    #[error("eval contract dataset binding is missing immutable `version`")]
    MissingDatasetVersion,
    /// Invalid expected sample count.
    #[error("eval contract `expected_sample_count` must be greater than zero when provided")]
    InvalidExpectedSampleCount,
    /// Benchmark mode lacked a benchmark package.
    #[error("benchmark eval runs require a `benchmark_package` binding")]
    BenchmarkPackageMissingForBenchmarkMode,
    /// Missing benchmark ref.
    #[error("benchmark package is missing `benchmark_ref`")]
    MissingBenchmarkRef,
    /// Missing benchmark version.
    #[error("benchmark package is missing `version`")]
    MissingBenchmarkVersion,
    /// Missing benchmark display name.
    #[error("benchmark package is missing `display_name`")]
    MissingBenchmarkDisplayName,
    /// Invalid repeat count.
    #[error("benchmark package `repeat_count` must be greater than zero")]
    InvalidBenchmarkRepeatCount,
    /// Benchmark package omitted cases.
    #[error("benchmark package must declare at least one case")]
    BenchmarkCasesMissing,
    /// Missing benchmark case id.
    #[error("benchmark package case is missing `case_id`")]
    MissingBenchmarkCaseId,
    /// Duplicate benchmark case.
    #[error("benchmark package repeated case `{case_id}`")]
    DuplicateBenchmarkCase {
        /// Repeated case id.
        case_id: String,
    },
    /// Eval bundle subject mismatch.
    #[error("benchmark eval bundle must use `eval_bundle`, found `{subject}`")]
    BenchmarkEvalBundleSubjectMismatch {
        /// Observed subject.
        subject: String,
    },
    /// Sample id missing.
    #[error("eval sample is missing `sample_id`")]
    MissingSampleId,
    /// Environment summary package mismatch.
    #[error("environment summary package mismatch: expected `{expected}`, found `{actual}`")]
    EnvironmentSummaryPackageMismatch {
        /// Expected package storage key.
        expected: String,
        /// Actual package storage key.
        actual: String,
    },
    /// Environment summary omitted one required rubric.
    #[error("environment summary is missing rubric outcome `{rubric_ref}`")]
    MissingRubricOutcome {
        /// Missing rubric ref.
        rubric_ref: String,
    },
    /// Token accounting mismatch.
    #[error(
        "token accounting mismatch: input_tokens={input_tokens}, output_tokens={output_tokens}, total_tokens={total_tokens}"
    )]
    TokenAccountingMismatch {
        /// Input tokens.
        input_tokens: u32,
        /// Output tokens.
        output_tokens: u32,
        /// Total tokens.
        total_tokens: u32,
    },
    /// Invalid start timestamp.
    #[error("eval run `started_at_ms` must be greater than zero")]
    InvalidStartedAt,
    /// Run cannot start from its current state.
    #[error("eval run `{eval_run_id}` cannot start from status `{status}`")]
    RunNotStartable {
        /// Run id.
        eval_run_id: String,
        /// Current status.
        status: String,
    },
    /// Run is not currently collecting samples.
    #[error("eval run `{eval_run_id}` is not accepting samples while status is `{status}`")]
    RunNotAcceptingSamples {
        /// Run id.
        eval_run_id: String,
        /// Current status.
        status: String,
    },
    /// Sample environment mismatch.
    #[error("eval sample environment mismatch: expected `{expected}`, found `{actual}`")]
    SampleEnvironmentMismatch {
        /// Expected environment storage key.
        expected: String,
        /// Actual environment storage key.
        actual: String,
    },
    /// Duplicate sample id.
    #[error("eval run repeated sample `{sample_id}`")]
    DuplicateSample {
        /// Repeated sample id.
        sample_id: String,
    },
    /// Expected sample count exceeded.
    #[error("eval run exceeded expected_sample_count={expected_sample_count}")]
    ExpectedSampleCountExceeded {
        /// Expected sample count.
        expected_sample_count: u64,
    },
    /// Run cannot finalize from its current state.
    #[error("eval run `{eval_run_id}` cannot finalize from status `{status}`")]
    RunNotFinalizable {
        /// Run id.
        eval_run_id: String,
        /// Current status.
        status: String,
    },
    /// Run was never started.
    #[error("eval run must be started before finalization")]
    RunNeverStarted,
    /// Finalize timestamp before start.
    #[error("finalized_at_ms {finalized_at_ms} is earlier than started_at_ms {started_at_ms}")]
    FinalizeBeforeStart {
        /// Start time.
        started_at_ms: u64,
        /// Finalize time.
        finalized_at_ms: u64,
    },
    /// Expected sample count not yet reached at finalize time.
    #[error(
        "eval run expected {expected_sample_count} samples but only recorded {actual_sample_count}"
    )]
    ExpectedSampleCountIncomplete {
        /// Expected count.
        expected_sample_count: u64,
        /// Actual count.
        actual_sample_count: u64,
    },
    /// Summary unexpectedly missing.
    #[error("eval summary missing after finalize")]
    SummaryMissing,
    /// Benchmark round uses a different environment.
    #[error("benchmark environment mismatch: expected `{expected}`, found `{actual}`")]
    BenchmarkEnvironmentMismatch {
        /// Expected environment storage key.
        expected: String,
        /// Actual environment storage key.
        actual: String,
    },
    /// Eval run was not in benchmark mode.
    #[error("eval run `{eval_run_id}` is not in benchmark mode")]
    EvalRunNotBenchmarkMode {
        /// Eval run id.
        eval_run_id: String,
    },
    /// Benchmark package mismatch.
    #[error("benchmark package mismatch: expected `{expected}`, found `{actual}`")]
    BenchmarkPackageMismatch {
        /// Expected benchmark storage key.
        expected: String,
        /// Actual benchmark storage key.
        actual: String,
    },
    /// Case count mismatch.
    #[error("benchmark package expected {expected} cases but eval run recorded {actual} samples")]
    BenchmarkCaseCountMismatch {
        /// Expected case count.
        expected: u64,
        /// Actual sample count.
        actual: u64,
    },
    /// One benchmark round was not finalized.
    #[error("benchmark round eval run `{eval_run_id}` must be finalized, found status `{status}`")]
    BenchmarkRoundNotFinalized {
        /// Eval run id.
        eval_run_id: String,
        /// Observed status.
        status: String,
    },
    /// Benchmark rounds missing.
    #[error("benchmark execution recorded no rounds")]
    BenchmarkRoundsMissing,
    /// Timer-integrity facts missing.
    #[error("eval run `{eval_run_id}` sample `{sample_id}` is missing timer-integrity facts")]
    BenchmarkTimerIntegrityMissing {
        /// Eval run id.
        eval_run_id: String,
        /// Sample id.
        sample_id: String,
    },
    /// Token-accounting facts missing.
    #[error("eval run `{eval_run_id}` sample `{sample_id}` is missing token-accounting facts")]
    BenchmarkTokenAccountingMissing {
        /// Eval run id.
        eval_run_id: String,
        /// Sample id.
        sample_id: String,
    },
    /// Final-state capture missing.
    #[error("eval run `{eval_run_id}` sample `{sample_id}` is missing final-state capture")]
    BenchmarkFinalStateMissing {
        /// Eval run id.
        eval_run_id: String,
        /// Sample id.
        sample_id: String,
    },
    /// Execution-strategy facts missing.
    #[error("eval run `{eval_run_id}` sample `{sample_id}` is missing execution-strategy facts")]
    BenchmarkExecutionStrategyMissing {
        /// Eval run id.
        eval_run_id: String,
        /// Sample id.
        sample_id: String,
    },
}

fn build_summary(
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

fn validate_verification_policy(
    sample: &EvalSampleRecord,
    verification_policy: &BenchmarkVerificationPolicy,
    eval_run_id: &str,
) -> Result<(), EvalRuntimeError> {
    let verification = sample.verification.as_ref();
    if verification_policy.require_timer_integrity
        && verification
            .and_then(|verification| verification.timer_integrity.as_ref())
            .is_none()
    {
        return Err(EvalRuntimeError::BenchmarkTimerIntegrityMissing {
            eval_run_id: String::from(eval_run_id),
            sample_id: sample.sample_id.clone(),
        });
    }
    if verification_policy.require_token_accounting
        && verification
            .and_then(|verification| verification.token_accounting.as_ref())
            .is_none()
    {
        return Err(EvalRuntimeError::BenchmarkTokenAccountingMissing {
            eval_run_id: String::from(eval_run_id),
            sample_id: sample.sample_id.clone(),
        });
    }
    if verification_policy.require_final_state_capture
        && verification
            .and_then(|verification| verification.final_state.as_ref())
            .is_none()
    {
        return Err(EvalRuntimeError::BenchmarkFinalStateMissing {
            eval_run_id: String::from(eval_run_id),
            sample_id: sample.sample_id.clone(),
        });
    }
    if verification_policy.require_execution_strategy
        && verification
            .and_then(|verification| verification.execution_strategy.as_ref())
            .is_none()
    {
        return Err(EvalRuntimeError::BenchmarkExecutionStrategyMissing {
            eval_run_id: String::from(eval_run_id),
            sample_id: sample.sample_id.clone(),
        });
    }
    Ok(())
}

fn robust_median(values: &[u32]) -> u32 {
    let mut values = values.to_vec();
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        ((u64::from(values[middle - 1]) + u64::from(values[middle])) / 2) as u32
    }
}

fn stable_benchmark_round_digest(
    package_key: &str,
    round_index: u32,
    execution_mode: BenchmarkExecutionMode,
    summary_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_benchmark_round|");
    hasher.update(package_key.as_bytes());
    hasher.update(b"|");
    hasher.update(round_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(benchmark_execution_mode_label(execution_mode));
    hasher.update(b"|");
    hasher.update(summary_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(_) => Vec::new(),
    }
}

fn eval_run_mode_label(mode: EvalRunMode) -> &'static [u8] {
    match mode {
        EvalRunMode::OfflineHeldOut => b"offline_held_out",
        EvalRunMode::OnlineShadow => b"online_shadow",
        EvalRunMode::Benchmark => b"benchmark",
    }
}

fn eval_run_status_label(status: EvalRunStatus) -> &'static str {
    match status {
        EvalRunStatus::Queued => "queued",
        EvalRunStatus::Running => "running",
        EvalRunStatus::Finalized => "finalized",
        EvalRunStatus::Failed => "failed",
        EvalRunStatus::Cancelled => "cancelled",
    }
}

fn benchmark_aggregation_kind_label(aggregation: BenchmarkAggregationKind) -> &'static [u8] {
    match aggregation {
        BenchmarkAggregationKind::MedianScore => b"median_score",
        BenchmarkAggregationKind::MeanScore => b"mean_score",
    }
}

fn benchmark_execution_mode_label(mode: BenchmarkExecutionMode) -> &'static [u8] {
    match mode {
        BenchmarkExecutionMode::Validator => b"validator",
        BenchmarkExecutionMode::OperatorSimulation => b"operator_simulation",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode, BenchmarkPackage,
        BenchmarkPackageKey, BenchmarkVerificationPolicy, EvalExecutionStrategyFacts,
        EvalFinalStateCapture, EvalRunContract, EvalRunMode, EvalRunState, EvalRuntimeError,
        EvalSampleRecord, EvalTimerIntegrityFacts, EvalTokenAccountingFacts, EvalVerificationFacts,
    };
    use psionic_data::DatasetKey;
    use psionic_environments::{
        EnvironmentArtifactExpectation, EnvironmentArtifactOutput, EnvironmentDatasetBinding,
        EnvironmentExecutionEntrypoint, EnvironmentPackageContract, EnvironmentPackageFamily,
        EnvironmentPackageKey, EnvironmentRubricHook, EnvironmentRubricOutcome,
        EnvironmentRubricScoreKind, EnvironmentRuntimeFamily, EnvironmentStateMode,
        EnvironmentToolContract, EnvironmentToolInterface, EnvironmentToolResult,
        EnvironmentTurnInput,
    };

    fn eval_environment() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("env.openagents.math.eval", "2026.03.14"),
            EnvironmentPackageFamily::Evaluation,
            "Math Eval",
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::Evaluator,
                entrypoint: String::from("math_eval::run"),
                args: Vec::new(),
                sandbox_profile_ref: Some(String::from("sandbox.profile.eval")),
                max_turns: 1,
                state_mode: EnvironmentStateMode::TurnScoped,
                time_budget_ms: Some(5_000),
            },
        )
        .with_datasets(vec![EnvironmentDatasetBinding {
            dataset: DatasetKey::new("dataset://openagents/math-eval", "2026.03.14"),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/math"),
            required: true,
        }])
        .with_tools(vec![EnvironmentToolContract {
            tool_name: String::from("grade_answer"),
            interface: EnvironmentToolInterface::NativeFunction,
            description: String::from("Grades an answer"),
            args_schema: json!({"type": "object"}),
            result_schema: Some(json!({"type": "object"})),
        }])
        .with_rubric_hooks(vec![
            EnvironmentRubricHook {
                rubric_ref: String::from("rubric://math.correctness"),
                hook_name: String::from("grade_correctness"),
                score_kind: EnvironmentRubricScoreKind::Scalar,
                pass_threshold: Some(8_000),
            },
            EnvironmentRubricHook {
                rubric_ref: String::from("rubric://math.format"),
                hook_name: String::from("grade_format"),
                score_kind: EnvironmentRubricScoreKind::Binary,
                pass_threshold: Some(10_000),
            },
        ])
        .with_expected_artifacts(vec![EnvironmentArtifactExpectation {
            artifact_kind: String::from("trace.json"),
            required: true,
            verification_policy_ref: Some(String::from("verify://trace")),
        }])
    }

    fn scored_summary(
        session_id: &str,
        task_id: &str,
        correctness_score: i32,
        format_passed: bool,
    ) -> Result<psionic_environments::EnvironmentSessionSummary, Box<dyn std::error::Error>> {
        let package = eval_environment();
        let mut session = package.open_session(session_id, task_id)?;
        session.begin_turn(EnvironmentTurnInput::new("2+2=?"))?;
        let tool_call = session.request_tool("grade_answer", json!({"answer": "4"}))?;
        session.resolve_tool(EnvironmentToolResult {
            call_id: tool_call.call_id.clone(),
            tool_name: String::from("grade_answer"),
            output: json!({"correct": true}),
            succeeded: true,
        })?;
        session.complete_turn(
            "4",
            vec![EnvironmentArtifactOutput::new(
                "trace.json",
                format!("artifact://{session_id}"),
                b"{\"answer\":\"4\"}",
            )],
        )?;
        Ok(session.finalize(vec![
            EnvironmentRubricOutcome {
                rubric_ref: String::from("rubric://math.correctness"),
                score_value: correctness_score,
                passed: correctness_score >= 8_000,
            },
            EnvironmentRubricOutcome {
                rubric_ref: String::from("rubric://math.format"),
                score_value: if format_passed { 10_000 } else { 0 },
                passed: format_passed,
            },
        ])?)
    }

    fn verification() -> EvalVerificationFacts {
        EvalVerificationFacts {
            timer_integrity: Some(EvalTimerIntegrityFacts {
                declared_budget_ms: Some(5_000),
                elapsed_ms: 2_100,
                within_budget: true,
            }),
            token_accounting: Some(
                EvalTokenAccountingFacts::new(12, 4, 16).expect("token accounting should validate"),
            ),
            final_state: Some(EvalFinalStateCapture {
                session_digest: String::from("session-digest"),
                output_digest: Some(String::from("output-digest")),
                artifact_digests: vec![String::from("artifact-digest")],
            }),
            execution_strategy: Some(EvalExecutionStrategyFacts {
                strategy_label: String::from("single_process"),
                runtime_family: Some(String::from("evaluator")),
                scheduler_posture: Some(String::from("deterministic")),
            }),
        }
    }

    #[test]
    fn eval_sample_from_environment_summary_is_machine_legible()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = eval_environment();
        let summary = scored_summary("session-1", "task-1", 8_600, true)?;
        let sample = EvalSampleRecord::from_environment_summary(
            "sample-1",
            Some(0),
            Some(String::from("input://1")),
            Some(String::from("output://1")),
            Some(String::from("expected://1")),
            &package,
            &summary,
            Some(verification()),
        )?;

        assert_eq!(sample.environment.storage_key(), package.storage_key());
        assert_eq!(sample.status, super::EvalSampleStatus::Passed);
        assert_eq!(sample.score_bps, Some(9_300));
        assert_eq!(sample.metrics.len(), 2);
        assert_eq!(sample.artifacts.len(), 1);
        assert!(sample.session_digest.is_some());
        Ok(())
    }

    #[test]
    fn eval_run_finalizes_summary_and_preserves_online_offline_parity()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = eval_environment();
        let offline_sample = EvalSampleRecord::from_environment_summary(
            "sample-offline",
            Some(0),
            None,
            None,
            None,
            &package,
            &scored_summary("offline", "task-offline", 8_200, true)?,
            None,
        )?;
        let online_sample = EvalSampleRecord::from_environment_summary(
            "sample-online",
            Some(0),
            None,
            None,
            None,
            &package,
            &scored_summary("online", "task-online", 8_200, true)?,
            None,
        )?;

        let mut offline_run = EvalRunState::open(
            EvalRunContract::new(
                "eval-offline",
                EvalRunMode::OfflineHeldOut,
                package.key.clone(),
            )
            .with_dataset(
                DatasetKey::new("dataset://openagents/math-eval", "2026.03.14"),
                Some(String::from("validation")),
            )
            .with_expected_sample_count(1),
        )?;
        let mut online_run = EvalRunState::open(
            EvalRunContract::new(
                "eval-online",
                EvalRunMode::OnlineShadow,
                package.key.clone(),
            )
            .with_expected_sample_count(1),
        )?;
        offline_run.start(1_000)?;
        online_run.start(1_000)?;
        offline_run.append_sample(offline_sample)?;
        online_run.append_sample(online_sample)?;
        let offline_summary = offline_run
            .finalize(1_100, Vec::new())
            .expect("offline finalize should succeed")
            .clone();
        let online_summary = online_run
            .finalize(1_100, Vec::new())
            .expect("online finalize should succeed")
            .clone();

        assert_eq!(
            offline_summary.average_score_bps,
            online_summary.average_score_bps
        );
        assert_eq!(offline_summary.pass_rate_bps, online_summary.pass_rate_bps);
        Ok(())
    }

    #[test]
    fn benchmark_package_supports_repeat_aggregation_and_operator_simulation()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = eval_environment();
        let benchmark_package = BenchmarkPackage::new(
            BenchmarkPackageKey::new("benchmark://openagents/math/basic", "2026.03.14"),
            "Math Basic Benchmark",
            package.key.clone(),
            3,
            BenchmarkAggregationKind::MedianScore,
        )
        .with_dataset(
            DatasetKey::new("dataset://openagents/math-eval", "2026.03.14"),
            Some(String::from("validation")),
        )
        .with_verification_policy(BenchmarkVerificationPolicy {
            require_timer_integrity: true,
            require_token_accounting: true,
            require_final_state_capture: true,
            require_execution_strategy: true,
        })
        .with_cases(vec![
            BenchmarkCase::new("case-1"),
            BenchmarkCase::new("case-2"),
        ]);
        let mut session =
            benchmark_package.open_execution(BenchmarkExecutionMode::OperatorSimulation)?;

        for (round, score_a, score_b) in
            [(1_u32, 8_000, 9_000), (2, 9_000, 9_000), (3, 10_000, 8_000)]
        {
            let mut run = EvalRunState::open(
                EvalRunContract::new(
                    format!("benchmark-run-{round}"),
                    EvalRunMode::Benchmark,
                    package.key.clone(),
                )
                .with_benchmark_package(BenchmarkPackageKey::new(
                    "benchmark://openagents/math/basic",
                    "2026.03.14",
                ))
                .with_expected_sample_count(2),
            )?;
            run.start(1_000 + u64::from(round))?;
            run.append_sample(EvalSampleRecord::from_environment_summary(
                format!("sample-{round}-1"),
                Some(0),
                None,
                None,
                None,
                &package,
                &scored_summary(&format!("session-{round}-1"), "task-a", score_a, true)?,
                Some(verification()),
            )?)?;
            run.append_sample(EvalSampleRecord::from_environment_summary(
                format!("sample-{round}-2"),
                Some(1),
                None,
                None,
                None,
                &package,
                &scored_summary(&format!("session-{round}-2"), "task-b", score_b, true)?,
                Some(verification()),
            )?)?;
            run.finalize(2_000 + u64::from(round), Vec::new())?;
            session.record_round(&run)?;
        }

        let aggregate = session.finalize()?;
        assert_eq!(aggregate.round_count, 3);
        assert_eq!(
            aggregate.execution_mode,
            BenchmarkExecutionMode::OperatorSimulation
        );
        assert_eq!(aggregate.aggregate_score_bps, Some(9_500));
        assert_eq!(aggregate.aggregate_pass_rate_bps, 10_000);
        assert_eq!(aggregate.per_round_scores_bps.len(), 3);
        Ok(())
    }

    #[test]
    fn benchmark_policy_refuses_missing_verification_facts()
    -> Result<(), Box<dyn std::error::Error>> {
        let package = eval_environment();
        let benchmark_package = BenchmarkPackage::new(
            BenchmarkPackageKey::new("benchmark://openagents/math/basic", "2026.03.14"),
            "Math Basic Benchmark",
            package.key.clone(),
            1,
            BenchmarkAggregationKind::MedianScore,
        )
        .with_verification_policy(BenchmarkVerificationPolicy {
            require_timer_integrity: true,
            require_token_accounting: false,
            require_final_state_capture: false,
            require_execution_strategy: false,
        })
        .with_cases(vec![BenchmarkCase::new("case-1")]);
        let mut session = benchmark_package.open_execution(BenchmarkExecutionMode::Validator)?;
        let mut run = EvalRunState::open(
            EvalRunContract::new(
                "benchmark-run-missing",
                EvalRunMode::Benchmark,
                package.key.clone(),
            )
            .with_benchmark_package(BenchmarkPackageKey::new(
                "benchmark://openagents/math/basic",
                "2026.03.14",
            ))
            .with_expected_sample_count(1),
        )?;
        run.start(1_000)?;
        run.append_sample(EvalSampleRecord::from_environment_summary(
            "sample-missing",
            Some(0),
            None,
            None,
            None,
            &package,
            &scored_summary("session-missing", "task-missing", 8_400, true)?,
            None,
        )?)?;
        run.finalize(1_200, Vec::new())?;

        assert_eq!(
            session
                .record_round(&run)
                .expect_err("missing timer integrity should fail"),
            EvalRuntimeError::BenchmarkTimerIntegrityMissing {
                eval_run_id: String::from("benchmark-run-missing"),
                sample_id: String::from("sample-missing"),
            }
        );
        Ok(())
    }
}
