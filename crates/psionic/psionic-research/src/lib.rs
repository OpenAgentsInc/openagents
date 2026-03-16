//! Typed experiment contracts, bounded run manifests, and promotion records for
//! Psionic hillclimb loops.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod runner;

pub use runner::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "typed experiment specs, bounded run results, and promotion records for psionic research loops";

/// Stable schema version for typed experiment specifications.
pub const EXPERIMENT_SPEC_SCHEMA_VERSION: u16 = 1;
/// Stable schema version for bounded experiment results.
pub const EXPERIMENT_RESULT_SCHEMA_VERSION: u16 = 1;
/// Stable schema version for promotion records.
pub const PROMOTION_RECORD_SCHEMA_VERSION: u16 = 1;

/// Top-level experiment family supported by the shared research substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentFamilyKind {
    ServingScheduler,
    BackendTuning,
    DatastreamTransfer,
    SandboxWarmPool,
    TrainingPolicy,
    ValidatorPolicy,
    EnvironmentMix,
    ExecutorVariants,
}

impl ExperimentFamilyKind {
    /// Returns a stable family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ServingScheduler => "serving_scheduler",
            Self::BackendTuning => "backend_tuning",
            Self::DatastreamTransfer => "datastream_transfer",
            Self::SandboxWarmPool => "sandbox_warm_pool",
            Self::TrainingPolicy => "training_policy",
            Self::ValidatorPolicy => "validator_policy",
            Self::EnvironmentMix => "environment_mix",
            Self::ExecutorVariants => "executor_variants",
        }
    }
}

/// High-level artifact kind referenced by one experiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentArtifactKind {
    ServedArtifact,
    CheckpointFamily,
    DatastreamManifest,
    EnvironmentManifest,
    ValidatorPolicy,
    BenchmarkSuite,
    BenchmarkReport,
    ModelDescriptor,
    CompiledWeightArtifact,
    ProgramArtifact,
    RuntimeManifest,
    ExecutionProofBundle,
    RunnerBinary,
    Auxiliary,
}

/// Stable artifact reference carried by an experiment or result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentArtifactRef {
    /// Artifact family.
    pub kind: ExperimentArtifactKind,
    /// Stable artifact reference.
    pub reference: String,
    /// Stable artifact digest.
    pub digest: String,
}

impl ExperimentArtifactRef {
    /// Creates a new artifact reference.
    #[must_use]
    pub fn new(
        kind: ExperimentArtifactKind,
        reference: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            reference: reference.into(),
            digest: digest.into(),
        }
    }
}

/// Declarative mutation record for one candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateMutation {
    /// Stable mutation identifier.
    pub mutation_id: String,
    /// Parent candidate when this mutation branches from an existing frontier node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_candidate_id: Option<String>,
    /// Declared family being mutated.
    pub family: ExperimentFamilyKind,
    /// Human-readable typed policy surfaces touched by the candidate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_surfaces: Vec<String>,
    /// Stable mutation digest.
    pub mutation_digest: String,
}

impl CandidateMutation {
    /// Creates a typed mutation record for one experiment family.
    #[must_use]
    pub fn new(
        mutation_id: impl Into<String>,
        parent_candidate_id: Option<String>,
        family: ExperimentFamilyKind,
        changed_surfaces: Vec<String>,
    ) -> Self {
        let mutation_id = mutation_id.into();
        let mut changed_surfaces = changed_surfaces;
        changed_surfaces.sort();
        changed_surfaces.dedup();
        let mutation_digest = stable_candidate_mutation_digest(
            mutation_id.as_str(),
            parent_candidate_id.as_deref(),
            family,
            changed_surfaces.as_slice(),
        );
        Self {
            mutation_id,
            parent_candidate_id,
            family,
            changed_surfaces,
            mutation_digest,
        }
    }
}

/// Fixed budget for one bounded experiment run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentBudget {
    /// Maximum wall-clock time for the runner.
    pub max_wall_time_ms: u64,
    /// Optional upper bound on logical steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<u64>,
    /// Optional upper bound on processed samples or prompts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_samples: Option<u64>,
    /// Relative output directory used by the runner.
    pub output_root: String,
}

impl ExperimentBudget {
    /// Creates an explicit bounded budget.
    #[must_use]
    pub fn new(max_wall_time_ms: u64, output_root: impl Into<String>) -> Self {
        Self {
            max_wall_time_ms,
            max_steps: None,
            max_samples: None,
            output_root: output_root.into(),
        }
    }

    /// Adds a logical step budget.
    #[must_use]
    pub const fn with_max_steps(mut self, max_steps: u64) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    /// Adds a sample budget.
    #[must_use]
    pub const fn with_max_samples(mut self, max_samples: u64) -> Self {
        self.max_samples = Some(max_samples);
        self
    }
}

/// Runtime and sandbox request attached to one experiment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentRuntimeProfile {
    /// Digest of the compiled runner binary.
    pub runner_binary_digest: String,
    /// Declared sandbox profile reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_profile_ref: Option<String>,
    /// Declared runtime profile reference when sandboxing is not the only constraint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_profile_ref: Option<String>,
    /// Requested backend family.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_backend: Option<String>,
    /// Declared visible devices for local-first runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requested_visible_devices: Vec<String>,
}

impl ExperimentRuntimeProfile {
    /// Creates a runtime profile from the runner digest.
    #[must_use]
    pub fn new(runner_binary_digest: impl Into<String>) -> Self {
        Self {
            runner_binary_digest: runner_binary_digest.into(),
            sandbox_profile_ref: None,
            runtime_profile_ref: None,
            requested_backend: None,
            requested_visible_devices: Vec::new(),
        }
    }

    /// Attaches a sandbox profile reference.
    #[must_use]
    pub fn with_sandbox_profile_ref(mut self, sandbox_profile_ref: impl Into<String>) -> Self {
        self.sandbox_profile_ref = Some(sandbox_profile_ref.into());
        self
    }

    /// Attaches a runtime profile reference.
    #[must_use]
    pub fn with_runtime_profile_ref(mut self, runtime_profile_ref: impl Into<String>) -> Self {
        self.runtime_profile_ref = Some(runtime_profile_ref.into());
        self
    }

    /// Attaches a backend request.
    #[must_use]
    pub fn with_requested_backend(mut self, requested_backend: impl Into<String>) -> Self {
        self.requested_backend = Some(requested_backend.into());
        self
    }

    /// Attaches visible devices.
    #[must_use]
    pub fn with_requested_visible_devices(
        mut self,
        requested_visible_devices: Vec<String>,
    ) -> Self {
        let mut requested_visible_devices = requested_visible_devices;
        requested_visible_devices.sort();
        requested_visible_devices.dedup();
        self.requested_visible_devices = requested_visible_devices;
        self
    }
}

/// Direction for one score metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreDirection {
    Maximize,
    Minimize,
}

/// Threshold comparison for a hard-failure gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdComparison {
    AtLeast,
    AtMost,
}

/// Hard-failure threshold for one required metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentThreshold {
    /// Comparison mode for the threshold.
    pub comparison: ThresholdComparison,
    /// Stable integer score value in micros.
    pub value_micros: i64,
}

impl ExperimentThreshold {
    /// Creates a minimum-value threshold.
    #[must_use]
    pub const fn at_least(value_micros: i64) -> Self {
        Self {
            comparison: ThresholdComparison::AtLeast,
            value_micros,
        }
    }

    /// Creates a maximum-value threshold.
    #[must_use]
    pub const fn at_most(value_micros: i64) -> Self {
        Self {
            comparison: ThresholdComparison::AtMost,
            value_micros,
        }
    }

    /// Evaluates one observed metric.
    #[must_use]
    pub const fn accepts(self, value_micros: i64) -> bool {
        match self.comparison {
            ThresholdComparison::AtLeast => value_micros >= self.value_micros,
            ThresholdComparison::AtMost => value_micros <= self.value_micros,
        }
    }
}

/// Score metric contract for one family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreMetricSpec {
    /// Stable metric identifier.
    pub metric_id: String,
    /// Metric unit.
    pub unit: String,
    /// Whether higher or lower numbers are preferred.
    pub direction: ScoreDirection,
    /// Relative score weight in basis points.
    pub weight_bps: u16,
    /// Whether the metric must always be present.
    pub required: bool,
    /// Optional hard-failure gate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hard_gate: Option<ExperimentThreshold>,
}

impl ScoreMetricSpec {
    /// Creates a score metric contract.
    #[must_use]
    pub fn new(
        metric_id: impl Into<String>,
        unit: impl Into<String>,
        direction: ScoreDirection,
        weight_bps: u16,
    ) -> Self {
        Self {
            metric_id: metric_id.into(),
            unit: unit.into(),
            direction,
            weight_bps,
            required: true,
            hard_gate: None,
        }
    }

    /// Marks the metric as optional.
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Adds a hard-failure gate.
    #[must_use]
    pub const fn with_hard_gate(mut self, hard_gate: ExperimentThreshold) -> Self {
        self.hard_gate = Some(hard_gate);
        self
    }
}

/// Full score contract for one experiment family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentScoreContract {
    /// Stable score contract identifier.
    pub contract_id: String,
    /// Declared experiment family.
    pub family: ExperimentFamilyKind,
    /// Metrics that participate in scoring and gating.
    pub metrics: Vec<ScoreMetricSpec>,
    /// Stable digest over the full score contract.
    pub contract_digest: String,
}

impl ExperimentScoreContract {
    /// Creates a typed score contract.
    #[must_use]
    pub fn new(
        contract_id: impl Into<String>,
        family: ExperimentFamilyKind,
        metrics: Vec<ScoreMetricSpec>,
    ) -> Self {
        let contract_id = contract_id.into();
        let mut metrics = metrics;
        metrics.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
        metrics.dedup_by(|left, right| left.metric_id == right.metric_id);
        let contract_digest =
            stable_score_contract_digest(contract_id.as_str(), family, metrics.as_slice());
        Self {
            contract_id,
            family,
            metrics,
            contract_digest,
        }
    }

    /// Evaluates one bounded result against the score contract.
    pub fn evaluate_result(
        &self,
        result: &ExperimentResult,
    ) -> Result<ExperimentScoreEvaluation, ExperimentScoreEvaluationError> {
        if self.family != result.family_kind() {
            return Err(ExperimentScoreEvaluationError::FamilyMismatch {
                expected: self.family,
                actual: result.family_kind(),
            });
        }
        let mut per_metric = Vec::with_capacity(self.metrics.len());
        let mut missing_metrics = Vec::new();
        let mut hard_gate_failed = false;
        let mut weighted_score = 0_i128;
        for metric in &self.metrics {
            let maybe_score = result
                .scores
                .iter()
                .find(|score| score.metric_id == metric.metric_id);
            match maybe_score {
                Some(score) => {
                    let hard_gate_passed = metric
                        .hard_gate
                        .map_or(true, |threshold| threshold.accepts(score.value_micros));
                    if !hard_gate_passed {
                        hard_gate_failed = true;
                    }
                    let signed_value = match metric.direction {
                        ScoreDirection::Maximize => i128::from(score.value_micros),
                        ScoreDirection::Minimize => -i128::from(score.value_micros),
                    };
                    let weighted_value = signed_value * i128::from(metric.weight_bps);
                    weighted_score += weighted_value;
                    per_metric.push(ExperimentMetricEvaluation {
                        metric_id: metric.metric_id.clone(),
                        value_micros: score.value_micros,
                        unit: score.unit.clone(),
                        weighted_value,
                        hard_gate_passed,
                    });
                }
                None if metric.required => {
                    hard_gate_failed = true;
                    missing_metrics.push(metric.metric_id.clone());
                }
                None => {
                    missing_metrics.push(metric.metric_id.clone());
                }
            }
        }
        Ok(ExperimentScoreEvaluation {
            contract_id: self.contract_id.clone(),
            contract_digest: self.contract_digest.clone(),
            result_digest: result.result_digest.clone(),
            weighted_score,
            hard_gate_failed,
            missing_metrics,
            per_metric,
        })
    }
}

/// Serving-scheduler policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServingSchedulerPolicy {
    pub max_batch_tokens: u32,
    pub max_active_sequences: u16,
    pub prefill_share_bps: u16,
    pub decode_share_bps: u16,
    pub queue_slack_ms: u64,
}

impl ServingSchedulerPolicy {
    /// Creates a serving scheduler policy.
    #[must_use]
    pub const fn new(
        max_batch_tokens: u32,
        max_active_sequences: u16,
        prefill_share_bps: u16,
        decode_share_bps: u16,
        queue_slack_ms: u64,
    ) -> Self {
        Self {
            max_batch_tokens,
            max_active_sequences,
            prefill_share_bps,
            decode_share_bps,
            queue_slack_ms,
        }
    }
}

/// Backend tuning policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendTuningPolicy {
    pub backend_id: String,
    pub compile_cache_mode: String,
    pub preferred_chunk_tokens: u32,
    pub kernel_fusion: bool,
    pub compile_parallelism: u16,
}

impl BackendTuningPolicy {
    /// Creates a backend tuning policy.
    #[must_use]
    pub fn new(
        backend_id: impl Into<String>,
        compile_cache_mode: impl Into<String>,
        preferred_chunk_tokens: u32,
        kernel_fusion: bool,
        compile_parallelism: u16,
    ) -> Self {
        Self {
            backend_id: backend_id.into(),
            compile_cache_mode: compile_cache_mode.into(),
            preferred_chunk_tokens,
            kernel_fusion,
            compile_parallelism,
        }
    }
}

/// Datastream transfer policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatastreamTransferPolicy {
    pub manifest_ref: String,
    pub chunk_bytes: u32,
    pub concurrent_streams: u16,
    pub prefetch_depth: u16,
    pub checksum_interval_chunks: u16,
}

impl DatastreamTransferPolicy {
    /// Creates a datastream transfer policy.
    #[must_use]
    pub fn new(
        manifest_ref: impl Into<String>,
        chunk_bytes: u32,
        concurrent_streams: u16,
        prefetch_depth: u16,
        checksum_interval_chunks: u16,
    ) -> Self {
        Self {
            manifest_ref: manifest_ref.into(),
            chunk_bytes,
            concurrent_streams,
            prefetch_depth,
            checksum_interval_chunks,
        }
    }
}

/// Sandbox warm-pool policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxWarmPoolPolicy {
    pub pool_profile_id: String,
    pub min_warm_workers: u16,
    pub max_warm_workers: u16,
    pub reuse_ttl_ms: u64,
    pub parallel_start_limit: u16,
}

impl SandboxWarmPoolPolicy {
    /// Creates a sandbox warm-pool policy.
    #[must_use]
    pub fn new(
        pool_profile_id: impl Into<String>,
        min_warm_workers: u16,
        max_warm_workers: u16,
        reuse_ttl_ms: u64,
        parallel_start_limit: u16,
    ) -> Self {
        Self {
            pool_profile_id: pool_profile_id.into(),
            min_warm_workers,
            max_warm_workers,
            reuse_ttl_ms,
            parallel_start_limit,
        }
    }
}

/// Training policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingPolicySpec {
    pub policy_id: String,
    pub checkpoint_every_steps: u64,
    pub optimizer_posture: String,
    pub stage_transition_id: String,
    pub off_policy_budget_bps: u16,
    pub instability_loss_spike_bps: u16,
    pub halt_on_nan: bool,
}

impl TrainingPolicySpec {
    /// Creates a training policy spec.
    #[must_use]
    pub fn new(
        policy_id: impl Into<String>,
        checkpoint_every_steps: u64,
        optimizer_posture: impl Into<String>,
        stage_transition_id: impl Into<String>,
        off_policy_budget_bps: u16,
        instability_loss_spike_bps: u16,
        halt_on_nan: bool,
    ) -> Self {
        Self {
            policy_id: policy_id.into(),
            checkpoint_every_steps,
            optimizer_posture: optimizer_posture.into(),
            stage_transition_id: stage_transition_id.into(),
            off_policy_budget_bps,
            instability_loss_spike_bps,
            halt_on_nan,
        }
    }
}

/// Validator policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorPolicySpec {
    pub policy_id: String,
    pub sample_rate_bps: u16,
    pub duplicate_signature_limit: u16,
    pub contribution_normalization: String,
    pub verdict_sample_rate_bps: u16,
}

impl ValidatorPolicySpec {
    /// Creates a validator policy spec.
    #[must_use]
    pub fn new(
        policy_id: impl Into<String>,
        sample_rate_bps: u16,
        duplicate_signature_limit: u16,
        contribution_normalization: impl Into<String>,
        verdict_sample_rate_bps: u16,
    ) -> Self {
        Self {
            policy_id: policy_id.into(),
            sample_rate_bps,
            duplicate_signature_limit,
            contribution_normalization: contribution_normalization.into(),
            verdict_sample_rate_bps,
        }
    }
}

/// Weighted environment selection entry for environment-mix experiments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightedEnvironmentRef {
    pub environment_ref: String,
    pub weight_bps: u16,
    pub timeout_ms: u64,
    pub rubric_mix_ref: String,
}

impl WeightedEnvironmentRef {
    /// Creates a weighted environment entry.
    #[must_use]
    pub fn new(
        environment_ref: impl Into<String>,
        weight_bps: u16,
        timeout_ms: u64,
        rubric_mix_ref: impl Into<String>,
    ) -> Self {
        Self {
            environment_ref: environment_ref.into(),
            weight_bps,
            timeout_ms,
            rubric_mix_ref: rubric_mix_ref.into(),
        }
    }
}

/// Environment-mix policy under test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentMixPolicy {
    pub mix_id: String,
    pub selection_posture: String,
    pub environments: Vec<WeightedEnvironmentRef>,
}

impl EnvironmentMixPolicy {
    /// Creates an environment-mix policy.
    #[must_use]
    pub fn new(
        mix_id: impl Into<String>,
        selection_posture: impl Into<String>,
        mut environments: Vec<WeightedEnvironmentRef>,
    ) -> Self {
        environments.sort_by(|left, right| left.environment_ref.cmp(&right.environment_ref));
        environments.dedup_by(|left, right| left.environment_ref == right.environment_ref);
        Self {
            mix_id: mix_id.into(),
            selection_posture: selection_posture.into(),
            environments,
        }
    }
}

/// Benchmark target for one Tassadar executor experiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorBenchmarkTarget {
    ValidationCorpus,
    ArticleClass,
}

/// Attention mode under test for one executor candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorAttentionMode {
    HardMax,
    SparseTopK,
    SoftmaxApprox,
}

/// Decode-cache posture under test for one executor candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorDecodeCacheKind {
    ReferenceLinear,
    HullCache,
    SparseTopK,
    StandardKv,
}

/// How one executor-family candidate obtains or constructs its weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorWeightConstruction {
    HandcraftedInterpreter,
    ProgramCompiled,
}

/// Architecture variant under test for one executor candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureVariant {
    pub variant_id: String,
    pub model_id: String,
    pub head_dim: u16,
    pub head_count: u16,
    pub layer_count: u16,
    pub feed_forward_width: u32,
    pub weight_construction: TassadarExecutorWeightConstruction,
}

impl TassadarExecutorArchitectureVariant {
    /// Creates an architecture variant declaration.
    #[must_use]
    pub fn new(
        variant_id: impl Into<String>,
        model_id: impl Into<String>,
        head_dim: u16,
        head_count: u16,
        layer_count: u16,
        feed_forward_width: u32,
        weight_construction: TassadarExecutorWeightConstruction,
    ) -> Self {
        Self {
            variant_id: variant_id.into(),
            model_id: model_id.into(),
            head_dim,
            head_count,
            layer_count,
            feed_forward_width,
            weight_construction,
        }
    }

    /// Returns the model width implied by the current head geometry.
    #[must_use]
    pub const fn d_model(&self) -> u32 {
        (self.head_dim as u32) * (self.head_count as u32)
    }

    /// Returns whether the candidate stays inside the 2D-head executor regime.
    #[must_use]
    pub const fn is_two_dimensional_lookup_family(&self) -> bool {
        self.head_dim == 2
    }

    /// Returns a deterministic parameter-count estimate for comparable research runs.
    #[must_use]
    pub fn estimated_parameter_count(&self) -> u64 {
        let d_model = u64::from(self.d_model());
        let layer_count = u64::from(self.layer_count);
        let feed_forward_width = u64::from(self.feed_forward_width);
        let attention_parameters = 4_u64.saturating_mul(d_model).saturating_mul(d_model);
        let feed_forward_parameters = 3_u64
            .saturating_mul(d_model)
            .saturating_mul(feed_forward_width);
        layer_count.saturating_mul(
            attention_parameters.saturating_add(feed_forward_parameters),
        )
    }
}

/// Trace-ABI variant under test for one executor candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTraceAbiVariant {
    pub variant_id: String,
    pub abi_id: String,
    pub schema_version: u16,
    pub append_only: bool,
    pub includes_stack_snapshots: bool,
    pub includes_local_snapshots: bool,
    pub includes_memory_snapshots: bool,
}

impl TassadarExecutorTraceAbiVariant {
    /// Creates a trace-ABI declaration.
    #[must_use]
    pub fn new(
        variant_id: impl Into<String>,
        abi_id: impl Into<String>,
        schema_version: u16,
        append_only: bool,
        includes_stack_snapshots: bool,
        includes_local_snapshots: bool,
        includes_memory_snapshots: bool,
    ) -> Self {
        Self {
            variant_id: variant_id.into(),
            abi_id: abi_id.into(),
            schema_version,
            append_only,
            includes_stack_snapshots,
            includes_local_snapshots,
            includes_memory_snapshots,
        }
    }
}

/// WebAssembly-profile variant under test for one executor candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorWasmProfileVariant {
    pub variant_id: String,
    pub profile_id: String,
    pub max_locals: u16,
    pub max_memory_slots: u16,
    pub max_program_len: u16,
    pub max_steps: u64,
}

impl TassadarExecutorWasmProfileVariant {
    /// Creates a Wasm-profile declaration.
    #[must_use]
    pub fn new(
        variant_id: impl Into<String>,
        profile_id: impl Into<String>,
        max_locals: u16,
        max_memory_slots: u16,
        max_program_len: u16,
        max_steps: u64,
    ) -> Self {
        Self {
            variant_id: variant_id.into(),
            profile_id: profile_id.into(),
            max_locals,
            max_memory_slots,
            max_program_len,
            max_steps,
        }
    }
}

/// Decode-cache and attention variant under test for one executor candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorDecodeCacheVariant {
    pub variant_id: String,
    pub cache_kind: TassadarExecutorDecodeCacheKind,
    pub attention_mode: TassadarExecutorAttentionMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_top_k: Option<u16>,
    pub exact_required: bool,
}

impl TassadarExecutorDecodeCacheVariant {
    /// Creates a decode-cache declaration.
    #[must_use]
    pub fn new(
        variant_id: impl Into<String>,
        cache_kind: TassadarExecutorDecodeCacheKind,
        attention_mode: TassadarExecutorAttentionMode,
        sparse_top_k: Option<u16>,
        exact_required: bool,
    ) -> Self {
        Self {
            variant_id: variant_id.into(),
            cache_kind,
            attention_mode,
            sparse_top_k,
            exact_required,
        }
    }
}

/// Full Tassadar executor experiment payload for one bounded candidate run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorExperimentSpec {
    pub benchmark_target: TassadarExecutorBenchmarkTarget,
    pub benchmark_ref: String,
    pub benchmark_version: String,
    pub environment_ref: String,
    pub architecture: TassadarExecutorArchitectureVariant,
    pub trace_abi: TassadarExecutorTraceAbiVariant,
    pub wasm_profile: TassadarExecutorWasmProfileVariant,
    pub decode_cache: TassadarExecutorDecodeCacheVariant,
}

impl TassadarExecutorExperimentSpec {
    /// Creates an executor experiment payload.
    #[must_use]
    pub fn new(
        benchmark_target: TassadarExecutorBenchmarkTarget,
        benchmark_ref: impl Into<String>,
        benchmark_version: impl Into<String>,
        environment_ref: impl Into<String>,
        architecture: TassadarExecutorArchitectureVariant,
        trace_abi: TassadarExecutorTraceAbiVariant,
        wasm_profile: TassadarExecutorWasmProfileVariant,
        decode_cache: TassadarExecutorDecodeCacheVariant,
    ) -> Self {
        Self {
            benchmark_target,
            benchmark_ref: benchmark_ref.into(),
            benchmark_version: benchmark_version.into(),
            environment_ref: environment_ref.into(),
            architecture,
            trace_abi,
            wasm_profile,
            decode_cache,
        }
    }
}

/// Typed experiment family payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum ExperimentFamily {
    ServingScheduler {
        model_id: String,
        benchmark_suite_ref: String,
        policy: ServingSchedulerPolicy,
    },
    BackendTuning {
        benchmark_suite_ref: String,
        policy: BackendTuningPolicy,
    },
    DatastreamTransfer {
        policy: DatastreamTransferPolicy,
    },
    SandboxWarmPool {
        benchmark_suite_ref: String,
        policy: SandboxWarmPoolPolicy,
    },
    TrainingPolicy {
        train_objective_ref: String,
        policy: TrainingPolicySpec,
    },
    ValidatorPolicy {
        validator_suite_ref: String,
        policy: ValidatorPolicySpec,
    },
    EnvironmentMix {
        mix: EnvironmentMixPolicy,
    },
    ExecutorVariants {
        executor: TassadarExecutorExperimentSpec,
    },
}

impl ExperimentFamily {
    /// Returns the top-level family kind.
    #[must_use]
    pub const fn kind(&self) -> ExperimentFamilyKind {
        match self {
            Self::ServingScheduler { .. } => ExperimentFamilyKind::ServingScheduler,
            Self::BackendTuning { .. } => ExperimentFamilyKind::BackendTuning,
            Self::DatastreamTransfer { .. } => ExperimentFamilyKind::DatastreamTransfer,
            Self::SandboxWarmPool { .. } => ExperimentFamilyKind::SandboxWarmPool,
            Self::TrainingPolicy { .. } => ExperimentFamilyKind::TrainingPolicy,
            Self::ValidatorPolicy { .. } => ExperimentFamilyKind::ValidatorPolicy,
            Self::EnvironmentMix { .. } => ExperimentFamilyKind::EnvironmentMix,
            Self::ExecutorVariants { .. } => ExperimentFamilyKind::ExecutorVariants,
        }
    }
}

/// Full typed specification for one experiment candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentSpec {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable experiment identifier.
    pub experiment_id: String,
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Typed family payload.
    pub family: ExperimentFamily,
    /// Base artifacts or manifests the candidate depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub base_artifacts: Vec<ExperimentArtifactRef>,
    /// Declarative mutation lineage.
    pub mutation: CandidateMutation,
    /// Requested runtime/sandbox profile.
    pub runtime_profile: ExperimentRuntimeProfile,
    /// Fixed execution budget.
    pub budget: ExperimentBudget,
    /// Score contract used for comparison.
    pub score_contract: ExperimentScoreContract,
    /// Stable experiment digest.
    pub spec_digest: String,
}

impl ExperimentSpec {
    /// Creates a typed experiment specification.
    #[must_use]
    pub fn new(
        experiment_id: impl Into<String>,
        candidate_id: impl Into<String>,
        family: ExperimentFamily,
        base_artifacts: Vec<ExperimentArtifactRef>,
        mutation: CandidateMutation,
        runtime_profile: ExperimentRuntimeProfile,
        budget: ExperimentBudget,
        score_contract: ExperimentScoreContract,
    ) -> Self {
        let experiment_id = experiment_id.into();
        let candidate_id = candidate_id.into();
        let mut base_artifacts = base_artifacts;
        base_artifacts.sort_by(|left, right| left.reference.cmp(&right.reference));
        base_artifacts.dedup_by(|left, right| left.reference == right.reference);
        let spec_digest = stable_experiment_spec_digest(
            experiment_id.as_str(),
            candidate_id.as_str(),
            &family,
            base_artifacts.as_slice(),
            &mutation,
            &runtime_profile,
            &budget,
            &score_contract,
        );
        Self {
            schema_version: EXPERIMENT_SPEC_SCHEMA_VERSION,
            experiment_id,
            candidate_id,
            family,
            base_artifacts,
            mutation,
            runtime_profile,
            budget,
            score_contract,
            spec_digest,
        }
    }
}

/// Score value emitted by one bounded experiment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentScore {
    /// Stable metric identifier.
    pub metric_id: String,
    /// Metric unit.
    pub unit: String,
    /// Value in micros for stable ordering.
    pub value_micros: i64,
}

impl ExperimentScore {
    /// Creates one score value.
    #[must_use]
    pub fn new(metric_id: impl Into<String>, unit: impl Into<String>, value_micros: i64) -> Self {
        Self {
            metric_id: metric_id.into(),
            unit: unit.into(),
            value_micros,
        }
    }
}

/// Ancillary metric emitted by one bounded experiment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentMetric {
    /// Stable metric identifier.
    pub metric_id: String,
    /// Metric unit.
    pub unit: String,
    /// Value in micros for stable ordering.
    pub value_micros: i64,
}

impl ExperimentMetric {
    /// Creates one ancillary metric.
    #[must_use]
    pub fn new(metric_id: impl Into<String>, unit: impl Into<String>, value_micros: i64) -> Self {
        Self {
            metric_id: metric_id.into(),
            unit: unit.into(),
            value_micros,
        }
    }
}

/// Kind of typed receipt surfaced by one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentReceiptKind {
    SandboxExecution,
    ServingBenchmark,
    TrainingRun,
    EvalRun,
    ValidatorVerdict,
}

/// Typed receipt or receipt reference surfaced by one bounded run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentReceiptRef {
    /// Receipt family.
    pub kind: ExperimentReceiptKind,
    /// Stable receipt reference.
    pub reference: String,
    /// Stable receipt digest.
    pub digest: String,
}

impl ExperimentReceiptRef {
    /// Creates a typed receipt reference.
    #[must_use]
    pub fn new(
        kind: ExperimentReceiptKind,
        reference: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            reference: reference.into(),
            digest: digest.into(),
        }
    }
}

/// Status of one bounded experiment run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentRunStatus {
    Succeeded,
    Failed,
    TimedOut,
    BudgetExhausted,
    SandboxMismatch,
}

/// Typed execution failure reason for bounded runner outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentFailureReason {
    MissingExecutionProfile,
    BudgetTooSmall,
    UnsupportedFamily,
    InvalidInvocation,
    InternalRunnerFailure,
}

/// One produced artifact from a bounded experiment run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentArtifactOutput {
    /// Stable artifact family.
    pub kind: ExperimentArtifactKind,
    /// Stable artifact reference.
    pub reference: String,
    /// Stable artifact digest.
    pub digest: String,
}

impl ExperimentArtifactOutput {
    /// Creates one produced artifact output.
    #[must_use]
    pub fn new(
        kind: ExperimentArtifactKind,
        reference: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            reference: reference.into(),
            digest: digest.into(),
        }
    }
}

/// Full result manifest from one bounded experiment run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentResult {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable experiment identifier.
    pub experiment_id: String,
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Family kind the result belongs to.
    pub family: ExperimentFamilyKind,
    /// Digest of the source experiment spec.
    pub spec_digest: String,
    /// Digest of the candidate mutation.
    pub mutation_digest: String,
    /// Start timestamp.
    pub started_at_ms: u64,
    /// Finish timestamp.
    pub finished_at_ms: u64,
    /// Final bounded status.
    pub status: ExperimentRunStatus,
    /// Typed failure reason when the run did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<ExperimentFailureReason>,
    /// Human-readable failure detail when the run did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_detail: Option<String>,
    /// Typed score outputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scores: Vec<ExperimentScore>,
    /// Ancillary metrics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<ExperimentMetric>,
    /// Receipt references carried by the run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipt_refs: Vec<ExperimentReceiptRef>,
    /// Produced artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_outputs: Vec<ExperimentArtifactOutput>,
    /// Stdout digest for the concrete runner.
    pub stdout_sha256: String,
    /// Stderr digest for the concrete runner.
    pub stderr_sha256: String,
    /// Stable result digest.
    pub result_digest: String,
}

impl ExperimentResult {
    /// Creates a typed bounded run result.
    #[must_use]
    pub fn new(
        run_id: impl Into<String>,
        spec: &ExperimentSpec,
        started_at_ms: u64,
        finished_at_ms: u64,
        status: ExperimentRunStatus,
        scores: Vec<ExperimentScore>,
        metrics: Vec<ExperimentMetric>,
        receipt_refs: Vec<ExperimentReceiptRef>,
        artifact_outputs: Vec<ExperimentArtifactOutput>,
        stdout_sha256: impl Into<String>,
        stderr_sha256: impl Into<String>,
    ) -> Self {
        let run_id = run_id.into();
        let stdout_sha256 = stdout_sha256.into();
        let stderr_sha256 = stderr_sha256.into();
        let mut scores = scores;
        scores.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
        scores.dedup_by(|left, right| left.metric_id == right.metric_id);
        let mut metrics = metrics;
        metrics.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
        metrics.dedup_by(|left, right| left.metric_id == right.metric_id);
        let mut receipt_refs = receipt_refs;
        receipt_refs.sort_by(|left, right| left.reference.cmp(&right.reference));
        receipt_refs.dedup_by(|left, right| left.reference == right.reference);
        let mut artifact_outputs = artifact_outputs;
        artifact_outputs.sort_by(|left, right| left.reference.cmp(&right.reference));
        artifact_outputs.dedup_by(|left, right| left.reference == right.reference);
        let result_digest = stable_experiment_result_digest(
            run_id.as_str(),
            spec,
            started_at_ms,
            finished_at_ms,
            status,
            None,
            None,
            scores.as_slice(),
            metrics.as_slice(),
            receipt_refs.as_slice(),
            artifact_outputs.as_slice(),
            stdout_sha256.as_str(),
            stderr_sha256.as_str(),
        );
        Self {
            schema_version: EXPERIMENT_RESULT_SCHEMA_VERSION,
            run_id,
            experiment_id: spec.experiment_id.clone(),
            candidate_id: spec.candidate_id.clone(),
            family: spec.family.kind(),
            spec_digest: spec.spec_digest.clone(),
            mutation_digest: spec.mutation.mutation_digest.clone(),
            started_at_ms,
            finished_at_ms,
            status,
            failure_reason: None,
            failure_detail: None,
            scores,
            metrics,
            receipt_refs,
            artifact_outputs,
            stdout_sha256,
            stderr_sha256,
            result_digest,
        }
    }

    /// Creates a typed failure result for a bounded run.
    #[must_use]
    pub fn new_failure(
        run_id: impl Into<String>,
        spec: &ExperimentSpec,
        started_at_ms: u64,
        finished_at_ms: u64,
        status: ExperimentRunStatus,
        failure_reason: ExperimentFailureReason,
        failure_detail: impl Into<String>,
        receipt_refs: Vec<ExperimentReceiptRef>,
        stdout_sha256: impl Into<String>,
        stderr_sha256: impl Into<String>,
    ) -> Self {
        let run_id = run_id.into();
        let failure_detail = failure_detail.into();
        let stdout_sha256 = stdout_sha256.into();
        let stderr_sha256 = stderr_sha256.into();
        let mut receipt_refs = receipt_refs;
        receipt_refs.sort_by(|left, right| left.reference.cmp(&right.reference));
        receipt_refs.dedup_by(|left, right| left.reference == right.reference);
        let result_digest = stable_experiment_result_digest(
            run_id.as_str(),
            spec,
            started_at_ms,
            finished_at_ms,
            status,
            Some(failure_reason),
            Some(failure_detail.as_str()),
            &[],
            &[],
            receipt_refs.as_slice(),
            &[],
            stdout_sha256.as_str(),
            stderr_sha256.as_str(),
        );
        Self {
            schema_version: EXPERIMENT_RESULT_SCHEMA_VERSION,
            run_id,
            experiment_id: spec.experiment_id.clone(),
            candidate_id: spec.candidate_id.clone(),
            family: spec.family.kind(),
            spec_digest: spec.spec_digest.clone(),
            mutation_digest: spec.mutation.mutation_digest.clone(),
            started_at_ms,
            finished_at_ms,
            status,
            failure_reason: Some(failure_reason),
            failure_detail: Some(failure_detail),
            scores: Vec::new(),
            metrics: Vec::new(),
            receipt_refs,
            artifact_outputs: Vec::new(),
            stdout_sha256,
            stderr_sha256,
            result_digest,
        }
    }

    /// Returns the top-level family kind.
    #[must_use]
    pub const fn family_kind(&self) -> ExperimentFamilyKind {
        self.family
    }
}

/// One metric-level evaluation after applying a score contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentMetricEvaluation {
    /// Stable metric identifier.
    pub metric_id: String,
    /// Observed value in micros.
    pub value_micros: i64,
    /// Metric unit.
    pub unit: String,
    /// Signed weighted value used for same-contract comparisons.
    pub weighted_value: i128,
    /// Whether any hard gate passed.
    pub hard_gate_passed: bool,
}

/// Aggregate evaluation of one result against one score contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentScoreEvaluation {
    /// Stable score contract identifier.
    pub contract_id: String,
    /// Digest of the contract used for evaluation.
    pub contract_digest: String,
    /// Digest of the evaluated result.
    pub result_digest: String,
    /// Aggregate weighted score for same-contract comparisons.
    pub weighted_score: i128,
    /// Whether a hard gate failed.
    pub hard_gate_failed: bool,
    /// Missing metrics from the result.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_metrics: Vec<String>,
    /// Per-metric evaluation details.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub per_metric: Vec<ExperimentMetricEvaluation>,
}

impl ExperimentScoreEvaluation {
    /// Compares two evaluations built from the same score contract.
    #[must_use]
    pub fn compare_same_contract(
        &self,
        other: &Self,
    ) -> Result<Ordering, ExperimentComparisonError> {
        if self.contract_digest != other.contract_digest {
            return Err(ExperimentComparisonError::ContractMismatch {
                left: self.contract_digest.clone(),
                right: other.contract_digest.clone(),
            });
        }
        match (self.hard_gate_failed, other.hard_gate_failed) {
            (false, true) => Ok(Ordering::Greater),
            (true, false) => Ok(Ordering::Less),
            _ => Ok(self.weighted_score.cmp(&other.weighted_score)),
        }
    }
}

/// Promotion decision recorded after a bounded evaluation pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecision {
    Keep,
    Discard,
    Branch,
    Promote,
    Blocked,
}

/// Reason code explaining one promotion decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionReasonCode {
    BetterNumberOnly,
    HardGateFailed,
    MissingEvidence,
    SafetyRegression,
    BranchForFurtherSearch,
    RecheckedWinner,
    FreshnessExpired,
}

/// Durable record explaining why one candidate was kept, discarded, branched, or promoted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionRecord {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable promotion identifier.
    pub promotion_id: String,
    /// Stable experiment identifier.
    pub experiment_id: String,
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Stable result digest.
    pub result_digest: String,
    /// Final keep/discard/branch/promote decision.
    pub decision: PromotionDecision,
    /// Whether the candidate is actually promotable.
    pub promotable: bool,
    /// Reason codes supporting the decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<PromotionReasonCode>,
    /// Optional operator-facing note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Stable digest for the promotion record.
    pub promotion_digest: String,
}

impl PromotionRecord {
    /// Creates a promotion record from one evaluated result.
    #[must_use]
    pub fn new(
        promotion_id: impl Into<String>,
        result: &ExperimentResult,
        decision: PromotionDecision,
        promotable: bool,
        reasons: Vec<PromotionReasonCode>,
        note: Option<String>,
    ) -> Self {
        let promotion_id = promotion_id.into();
        let mut reasons = reasons;
        reasons.sort_by_key(|reason| *reason as u8);
        reasons.dedup();
        let promotion_digest = stable_promotion_record_digest(
            promotion_id.as_str(),
            result,
            decision,
            promotable,
            reasons.as_slice(),
            note.as_deref(),
        );
        Self {
            schema_version: PROMOTION_RECORD_SCHEMA_VERSION,
            promotion_id,
            experiment_id: result.experiment_id.clone(),
            candidate_id: result.candidate_id.clone(),
            result_digest: result.result_digest.clone(),
            decision,
            promotable,
            reasons,
            note,
            promotion_digest,
        }
    }
}

/// Comparable summary for one candidate inside a bounded research sweep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchSweepEntry {
    /// Stable run identifier.
    pub run_id: String,
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Stable result digest.
    pub result_digest: String,
    /// Aggregate weighted score under the shared contract.
    pub weighted_score: i128,
    /// Whether the shared score contract marked the candidate as gate-failed.
    pub hard_gate_failed: bool,
}

/// Machine-readable sweep record for one comparable family run-set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchSweepRecord {
    /// Stable sweep identifier.
    pub sweep_id: String,
    /// Shared experiment family.
    pub family: ExperimentFamilyKind,
    /// Shared score contract digest.
    pub contract_digest: String,
    /// Ordered candidate entries.
    pub entries: Vec<ResearchSweepEntry>,
    /// Winning run when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winning_run_id: Option<String>,
    /// Winning candidate when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winning_candidate_id: Option<String>,
    /// Stable sweep digest.
    pub sweep_digest: String,
}

impl ResearchSweepRecord {
    /// Creates a sweep record from ordered entries and an optional winner.
    #[must_use]
    pub fn new(
        sweep_id: impl Into<String>,
        family: ExperimentFamilyKind,
        contract_digest: impl Into<String>,
        mut entries: Vec<ResearchSweepEntry>,
        winning_run_id: Option<String>,
        winning_candidate_id: Option<String>,
    ) -> Self {
        let sweep_id = sweep_id.into();
        let contract_digest = contract_digest.into();
        entries.sort_by(|left, right| left.run_id.cmp(&right.run_id));
        let sweep_digest = stable_research_sweep_digest(
            sweep_id.as_str(),
            family,
            contract_digest.as_str(),
            entries.as_slice(),
            winning_run_id.as_deref(),
            winning_candidate_id.as_deref(),
        );
        Self {
            sweep_id,
            family,
            contract_digest,
            entries,
            winning_run_id,
            winning_candidate_id,
            sweep_digest,
        }
    }
}

/// Error while evaluating a result under one score contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExperimentScoreEvaluationError {
    /// The result belongs to a different experiment family.
    #[error("experiment family mismatch: expected {expected:?}, found {actual:?}")]
    FamilyMismatch {
        expected: ExperimentFamilyKind,
        actual: ExperimentFamilyKind,
    },
}

/// Error while comparing two result evaluations.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExperimentComparisonError {
    /// The two evaluations were built from different score contracts.
    #[error("score contract mismatch: left={left} right={right}")]
    ContractMismatch { left: String, right: String },
}

fn stable_candidate_mutation_digest(
    mutation_id: &str,
    parent_candidate_id: Option<&str>,
    family: ExperimentFamilyKind,
    changed_surfaces: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_candidate_mutation|");
    hasher.update(mutation_id.as_bytes());
    hasher.update(b"|family|");
    hasher.update(family.label().as_bytes());
    if let Some(parent_candidate_id) = parent_candidate_id {
        hasher.update(b"|parent|");
        hasher.update(parent_candidate_id.as_bytes());
    }
    for changed_surface in changed_surfaces {
        hasher.update(b"|surface|");
        hasher.update(changed_surface.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_score_contract_digest(
    contract_id: &str,
    family: ExperimentFamilyKind,
    metrics: &[ScoreMetricSpec],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_score_contract|");
    hasher.update(contract_id.as_bytes());
    hasher.update(b"|family|");
    hasher.update(family.label().as_bytes());
    for metric in metrics {
        hasher.update(b"|metric|");
        hasher.update(
            serde_json::to_vec(metric)
                .unwrap_or_else(|_| unreachable!("score metric should serialize")),
        );
    }
    hex::encode(hasher.finalize())
}

fn stable_experiment_spec_digest(
    experiment_id: &str,
    candidate_id: &str,
    family: &ExperimentFamily,
    base_artifacts: &[ExperimentArtifactRef],
    mutation: &CandidateMutation,
    runtime_profile: &ExperimentRuntimeProfile,
    budget: &ExperimentBudget,
    score_contract: &ExperimentScoreContract,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_experiment_spec|");
    hasher.update(experiment_id.as_bytes());
    hasher.update(b"|candidate|");
    hasher.update(candidate_id.as_bytes());
    hasher.update(b"|family|");
    hasher.update(
        serde_json::to_vec(family).unwrap_or_else(|_| unreachable!("family should serialize")),
    );
    for artifact in base_artifacts {
        hasher.update(b"|artifact|");
        hasher.update(
            serde_json::to_vec(artifact)
                .unwrap_or_else(|_| unreachable!("artifact should serialize")),
        );
    }
    hasher.update(b"|mutation|");
    hasher.update(mutation.mutation_digest.as_bytes());
    hasher.update(b"|runtime_profile|");
    hasher.update(
        serde_json::to_vec(runtime_profile)
            .unwrap_or_else(|_| unreachable!("runtime profile should serialize")),
    );
    hasher.update(b"|budget|");
    hasher.update(
        serde_json::to_vec(budget).unwrap_or_else(|_| unreachable!("budget should serialize")),
    );
    hasher.update(b"|score_contract|");
    hasher.update(score_contract.contract_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_experiment_result_digest(
    run_id: &str,
    spec: &ExperimentSpec,
    started_at_ms: u64,
    finished_at_ms: u64,
    status: ExperimentRunStatus,
    failure_reason: Option<ExperimentFailureReason>,
    failure_detail: Option<&str>,
    scores: &[ExperimentScore],
    metrics: &[ExperimentMetric],
    receipt_refs: &[ExperimentReceiptRef],
    artifact_outputs: &[ExperimentArtifactOutput],
    stdout_sha256: &str,
    stderr_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_experiment_result|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|spec|");
    hasher.update(spec.spec_digest.as_bytes());
    hasher.update(b"|started|");
    hasher.update(started_at_ms.to_string().as_bytes());
    hasher.update(b"|finished|");
    hasher.update(finished_at_ms.to_string().as_bytes());
    hasher.update(b"|status|");
    hasher.update(
        serde_json::to_vec(&status).unwrap_or_else(|_| unreachable!("status should serialize")),
    );
    if let Some(failure_reason) = failure_reason {
        hasher.update(b"|failure_reason|");
        hasher.update(
            serde_json::to_vec(&failure_reason)
                .unwrap_or_else(|_| unreachable!("failure reason should serialize")),
        );
    }
    if let Some(failure_detail) = failure_detail {
        hasher.update(b"|failure_detail|");
        hasher.update(failure_detail.as_bytes());
    }
    for score in scores {
        hasher.update(b"|score|");
        hasher.update(
            serde_json::to_vec(score).unwrap_or_else(|_| unreachable!("score should serialize")),
        );
    }
    for metric in metrics {
        hasher.update(b"|metric|");
        hasher.update(
            serde_json::to_vec(metric).unwrap_or_else(|_| unreachable!("metric should serialize")),
        );
    }
    for receipt_ref in receipt_refs {
        hasher.update(b"|receipt|");
        hasher.update(
            serde_json::to_vec(receipt_ref)
                .unwrap_or_else(|_| unreachable!("receipt should serialize")),
        );
    }
    for artifact_output in artifact_outputs {
        hasher.update(b"|artifact_output|");
        hasher.update(
            serde_json::to_vec(artifact_output)
                .unwrap_or_else(|_| unreachable!("artifact output should serialize")),
        );
    }
    hasher.update(b"|stdout|");
    hasher.update(stdout_sha256.as_bytes());
    hasher.update(b"|stderr|");
    hasher.update(stderr_sha256.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_promotion_record_digest(
    promotion_id: &str,
    result: &ExperimentResult,
    decision: PromotionDecision,
    promotable: bool,
    reasons: &[PromotionReasonCode],
    note: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_promotion_record|");
    hasher.update(promotion_id.as_bytes());
    hasher.update(b"|result|");
    hasher.update(result.result_digest.as_bytes());
    hasher.update(b"|decision|");
    hasher.update(
        serde_json::to_vec(&decision).unwrap_or_else(|_| unreachable!("decision should serialize")),
    );
    hasher.update(b"|promotable|");
    hasher.update(if promotable {
        &b"true"[..]
    } else {
        &b"false"[..]
    });
    for reason in reasons {
        hasher.update(b"|reason|");
        hasher.update(
            serde_json::to_vec(reason).unwrap_or_else(|_| unreachable!("reason should serialize")),
        );
    }
    if let Some(note) = note {
        hasher.update(b"|note|");
        hasher.update(note.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_research_sweep_digest(
    sweep_id: &str,
    family: ExperimentFamilyKind,
    contract_digest: &str,
    entries: &[ResearchSweepEntry],
    winning_run_id: Option<&str>,
    winning_candidate_id: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_research_sweep|");
    hasher.update(sweep_id.as_bytes());
    hasher.update(b"|family|");
    hasher.update(family.label().as_bytes());
    hasher.update(b"|contract|");
    hasher.update(contract_digest.as_bytes());
    for entry in entries {
        hasher.update(b"|entry|");
        hasher.update(
            serde_json::to_vec(entry)
                .unwrap_or_else(|_| unreachable!("sweep entry should serialize")),
        );
    }
    if let Some(winning_run_id) = winning_run_id {
        hasher.update(b"|winning_run|");
        hasher.update(winning_run_id.as_bytes());
    }
    if let Some(winning_candidate_id) = winning_candidate_id {
        hasher.update(b"|winning_candidate|");
        hasher.update(winning_candidate_id.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        CandidateMutation, EnvironmentMixPolicy, ExperimentArtifactKind, ExperimentArtifactOutput,
        ExperimentArtifactRef, ExperimentBudget, ExperimentComparisonError,
        ExperimentFailureReason, ExperimentFamily, ExperimentFamilyKind, ExperimentReceiptKind,
        ExperimentReceiptRef, ExperimentResult, ExperimentRunStatus, ExperimentScore,
        ExperimentScoreContract, ExperimentThreshold, PromotionDecision, PromotionReasonCode,
        PromotionRecord, SandboxWarmPoolPolicy, ScoreDirection, ScoreMetricSpec,
        ServingSchedulerPolicy, TrainingPolicySpec, WeightedEnvironmentRef,
    };

    fn sample_serving_contract() -> ExperimentScoreContract {
        ExperimentScoreContract::new(
            "serve.score.v1",
            ExperimentFamilyKind::ServingScheduler,
            vec![
                ScoreMetricSpec::new(
                    "throughput_tokens_per_second",
                    "tokens_per_second",
                    ScoreDirection::Maximize,
                    7_000,
                )
                .with_hard_gate(ExperimentThreshold::at_least(150_000_000)),
                ScoreMetricSpec::new(
                    "p95_latency_ms",
                    "milliseconds",
                    ScoreDirection::Minimize,
                    2_000,
                )
                .with_hard_gate(ExperimentThreshold::at_most(65_000)),
                ScoreMetricSpec::new(
                    "peak_memory_bytes",
                    "bytes",
                    ScoreDirection::Minimize,
                    1_000,
                ),
            ],
        )
    }

    fn sample_serving_spec() -> super::ExperimentSpec {
        super::ExperimentSpec::new(
            "exp.serve.scheduler.1",
            "candidate-a",
            ExperimentFamily::ServingScheduler {
                model_id: String::from("gpt-oss-20b"),
                benchmark_suite_ref: String::from("benchmark://serve/local-weather"),
                policy: ServingSchedulerPolicy::new(8192, 8, 4500, 5500, 30),
            },
            vec![
                ExperimentArtifactRef::new(
                    ExperimentArtifactKind::RunnerBinary,
                    "bin://psionic-research-runner",
                    "runner-digest-1",
                ),
                ExperimentArtifactRef::new(
                    ExperimentArtifactKind::ServedArtifact,
                    "served://gpt-oss-20b",
                    "served-digest-1",
                ),
            ],
            CandidateMutation::new(
                "mutation-a",
                Some(String::from("baseline")),
                ExperimentFamilyKind::ServingScheduler,
                vec![
                    String::from("serve.scheduler.prefill_share_bps"),
                    String::from("serve.scheduler.max_batch_tokens"),
                ],
            ),
            super::ExperimentRuntimeProfile::new("runner-digest-1")
                .with_sandbox_profile_ref("sandbox://research/local")
                .with_requested_backend("cuda")
                .with_requested_visible_devices(vec![String::from("cuda:0")]),
            ExperimentBudget::new(30_000, "runs/serve.scheduler.1")
                .with_max_steps(2_000)
                .with_max_samples(32),
            sample_serving_contract(),
        )
    }

    #[test]
    fn experiment_spec_digest_is_stable_for_equivalent_inputs() {
        let first = sample_serving_spec();
        let second = sample_serving_spec();
        assert_eq!(first.spec_digest, second.spec_digest);
    }

    #[test]
    fn score_contract_evaluation_tracks_gates_and_missing_metrics() {
        let spec = sample_serving_spec();
        let result = ExperimentResult::new(
            "run-a",
            &spec,
            1_000,
            2_000,
            ExperimentRunStatus::Succeeded,
            vec![
                ExperimentScore::new(
                    "throughput_tokens_per_second",
                    "tokens_per_second",
                    175_000_000,
                ),
                ExperimentScore::new("p95_latency_ms", "milliseconds", 72_000),
            ],
            vec![],
            vec![ExperimentReceiptRef::new(
                ExperimentReceiptKind::SandboxExecution,
                "receipt://sandbox/run-a",
                "receipt-digest-a",
            )],
            vec![],
            "stdout-a",
            "stderr-a",
        );
        let evaluation = spec
            .score_contract
            .evaluate_result(&result)
            .expect("family should match");
        assert!(evaluation.hard_gate_failed);
        assert_eq!(
            evaluation.missing_metrics,
            vec![String::from("peak_memory_bytes")]
        );
        assert_eq!(evaluation.per_metric.len(), 2);
        assert!(
            evaluation
                .per_metric
                .iter()
                .any(|metric| !metric.hard_gate_passed)
        );
    }

    #[test]
    fn evaluation_prefers_gate_passing_candidate() {
        let spec = sample_serving_spec();
        let baseline = ExperimentResult::new(
            "run-baseline",
            &spec,
            1_000,
            2_000,
            ExperimentRunStatus::Succeeded,
            vec![
                ExperimentScore::new(
                    "throughput_tokens_per_second",
                    "tokens_per_second",
                    170_000_000,
                ),
                ExperimentScore::new("p95_latency_ms", "milliseconds", 60_000),
                ExperimentScore::new("peak_memory_bytes", "bytes", 22_000_000_000),
            ],
            vec![],
            vec![],
            vec![],
            "stdout-base",
            "stderr-base",
        );
        let unsafe_candidate = ExperimentResult::new(
            "run-unsafe",
            &spec,
            3_000,
            4_000,
            ExperimentRunStatus::Succeeded,
            vec![
                ExperimentScore::new(
                    "throughput_tokens_per_second",
                    "tokens_per_second",
                    190_000_000,
                ),
                ExperimentScore::new("p95_latency_ms", "milliseconds", 80_000),
                ExperimentScore::new("peak_memory_bytes", "bytes", 21_000_000_000),
            ],
            vec![],
            vec![],
            vec![],
            "stdout-unsafe",
            "stderr-unsafe",
        );
        let baseline_eval = spec
            .score_contract
            .evaluate_result(&baseline)
            .expect("family should match");
        let unsafe_eval = spec
            .score_contract
            .evaluate_result(&unsafe_candidate)
            .expect("family should match");
        assert_eq!(
            unsafe_eval.compare_same_contract(&baseline_eval),
            Ok(std::cmp::Ordering::Less)
        );
        let other_eval = ExperimentScoreContract::new(
            "train.score.v1",
            ExperimentFamilyKind::TrainingPolicy,
            vec![ScoreMetricSpec::new(
                "eval",
                "score",
                ScoreDirection::Maximize,
                10_000,
            )],
        )
        .evaluate_result(&ExperimentResult::new(
            "run-other",
            &super::ExperimentSpec::new(
                "exp.train.1",
                "train-candidate",
                ExperimentFamily::TrainingPolicy {
                    train_objective_ref: String::from("train://policy"),
                    policy: TrainingPolicySpec::new(
                        "train.policy",
                        128,
                        "adamw",
                        "stage.v1",
                        1500,
                        500,
                        true,
                    ),
                },
                vec![],
                CandidateMutation::new(
                    "mutation-train",
                    None,
                    ExperimentFamilyKind::TrainingPolicy,
                    vec![String::from("train.optimizer")],
                ),
                super::ExperimentRuntimeProfile::new("runner-digest-train"),
                ExperimentBudget::new(10_000, "runs/train"),
                ExperimentScoreContract::new(
                    "train.score.v1",
                    ExperimentFamilyKind::TrainingPolicy,
                    vec![ScoreMetricSpec::new(
                        "eval",
                        "score",
                        ScoreDirection::Maximize,
                        10_000,
                    )],
                ),
            ),
            1,
            2,
            ExperimentRunStatus::Succeeded,
            vec![ExperimentScore::new("eval", "score", 100)],
            vec![],
            vec![],
            vec![],
            "stdout-other",
            "stderr-other",
        ))
        .expect("train family should match");
        assert!(matches!(
            baseline_eval.compare_same_contract(&other_eval),
            Err(ExperimentComparisonError::ContractMismatch { .. })
        ));
    }

    #[test]
    fn promotion_record_distinguishes_better_number_from_promotable_candidate() {
        let spec = sample_serving_spec();
        let result = ExperimentResult::new(
            "run-better-number",
            &spec,
            1_000,
            2_000,
            ExperimentRunStatus::Succeeded,
            vec![
                ExperimentScore::new(
                    "throughput_tokens_per_second",
                    "tokens_per_second",
                    190_000_000,
                ),
                ExperimentScore::new("p95_latency_ms", "milliseconds", 82_000),
                ExperimentScore::new("peak_memory_bytes", "bytes", 20_000_000_000),
            ],
            vec![super::ExperimentMetric::new("prompt_count", "count", 32)],
            vec![ExperimentReceiptRef::new(
                ExperimentReceiptKind::ServingBenchmark,
                "receipt://serve/run-better-number",
                "serve-receipt-digest",
            )],
            vec![ExperimentArtifactOutput::new(
                ExperimentArtifactKind::Auxiliary,
                "artifact://serve/policy-better-number",
                "artifact-digest",
            )],
            "stdout-better-number",
            "stderr-better-number",
        );
        let blocked = PromotionRecord::new(
            "promotion-blocked",
            &result,
            PromotionDecision::Blocked,
            false,
            vec![
                PromotionReasonCode::BetterNumberOnly,
                PromotionReasonCode::HardGateFailed,
            ],
            Some(String::from("won throughput but violated latency gate")),
        );
        assert!(!blocked.promotable);
        assert_eq!(blocked.decision, PromotionDecision::Blocked);
        assert!(
            blocked
                .reasons
                .contains(&PromotionReasonCode::BetterNumberOnly)
        );
    }

    #[test]
    fn family_payloads_cover_all_declared_mutation_surfaces() {
        let sandbox = ExperimentFamily::SandboxWarmPool {
            benchmark_suite_ref: String::from("benchmark://sandbox/pool"),
            policy: SandboxWarmPoolPolicy::new("pool://weather", 1, 3, 60_000, 2),
        };
        let environment_mix = ExperimentFamily::EnvironmentMix {
            mix: EnvironmentMixPolicy::new(
                "env.mix.weather",
                "weighted_round_robin",
                vec![
                    WeightedEnvironmentRef::new(
                        "env://weather/us",
                        6000,
                        45_000,
                        "rubric://weather/base",
                    ),
                    WeightedEnvironmentRef::new(
                        "env://weather/eu",
                        4000,
                        45_000,
                        "rubric://weather/base",
                    ),
                ],
            ),
        };
        assert_eq!(sandbox.kind(), ExperimentFamilyKind::SandboxWarmPool);
        assert_eq!(environment_mix.kind(), ExperimentFamilyKind::EnvironmentMix);
    }

    #[test]
    fn failure_results_surface_reason_codes() {
        let spec = sample_serving_spec();
        let failure = ExperimentResult::new_failure(
            "run-failure",
            &spec,
            1_000,
            1_200,
            ExperimentRunStatus::SandboxMismatch,
            ExperimentFailureReason::MissingExecutionProfile,
            "sandbox profile ref missing",
            vec![ExperimentReceiptRef::new(
                ExperimentReceiptKind::SandboxExecution,
                "receipt://research/run-failure",
                "receipt-failure",
            )],
            "stdout-failure",
            "stderr-failure",
        );
        assert_eq!(
            failure.failure_reason,
            Some(ExperimentFailureReason::MissingExecutionProfile)
        );
        assert_eq!(failure.status, ExperimentRunStatus::SandboxMismatch);
        assert!(failure.scores.is_empty());
    }
}
