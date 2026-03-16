use std::fs;
use std::path::{Path, PathBuf};

use psionic_eval::{
    TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF, TASSADAR_ARTICLE_CLASS_BENCHMARK_REF,
    TASSADAR_BENCHMARK_ENVIRONMENT_REF, TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF,
    TassadarBenchmarkReport, run_tassadar_article_class_benchmark,
    run_tassadar_reference_fixture_benchmark,
};
use psionic_models::{TassadarCompiledProgramSuiteArtifact, TassadarExecutorFixture};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ExperimentArtifactKind, ExperimentArtifactOutput, ExperimentBudget, ExperimentComparisonError,
    ExperimentFailureReason, ExperimentFamily, ExperimentMetric, ExperimentReceiptKind,
    ExperimentReceiptRef, ExperimentResult, ExperimentRunStatus, ExperimentScore,
    ExperimentScoreEvaluationError, ExperimentSpec, ResearchSweepEntry, ResearchSweepRecord,
    TassadarExecutorAttentionMode, TassadarExecutorBenchmarkTarget,
    TassadarExecutorDecodeCacheKind, TassadarExecutorExperimentSpec,
    TassadarExecutorWeightConstruction,
};

/// Invocation contract passed to the compiled research runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchRunnerInvocation {
    /// Stable run identifier.
    pub run_id: String,
    /// Typed experiment to execute.
    pub spec: ExperimentSpec,
    /// Logical start timestamp for replay-safe manifests.
    pub started_at_ms: u64,
}

impl ResearchRunnerInvocation {
    /// Creates a typed runner invocation.
    #[must_use]
    pub fn new(run_id: impl Into<String>, spec: ExperimentSpec, started_at_ms: u64) -> Self {
        Self {
            run_id: run_id.into(),
            spec,
            started_at_ms,
        }
    }
}

/// Which profile class was used for local execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerProfileMode {
    Sandbox,
    Runtime,
}

/// Typed local execution receipt for one bounded research run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchExecutionReceipt {
    /// Stable run identifier.
    pub run_id: String,
    /// Digest of the invoked spec.
    pub spec_digest: String,
    /// Profile mode that scoped the run.
    pub profile_mode: RunnerProfileMode,
    /// Profile reference used for local execution.
    pub profile_ref: String,
    /// Final bounded status.
    pub status: ExperimentRunStatus,
    /// Bounded wall-clock duration in milliseconds.
    pub wall_time_ms: u64,
    /// Stdout digest recorded by the runner.
    pub stdout_sha256: String,
    /// Stderr digest recorded by the runner.
    pub stderr_sha256: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl ResearchExecutionReceipt {
    fn new(
        invocation: &ResearchRunnerInvocation,
        profile_mode: RunnerProfileMode,
        profile_ref: &str,
        status: ExperimentRunStatus,
        wall_time_ms: u64,
        stdout_sha256: &str,
        stderr_sha256: &str,
    ) -> Self {
        let receipt_digest = stable_research_execution_receipt_digest(
            invocation.run_id.as_str(),
            invocation.spec.spec_digest.as_str(),
            profile_mode,
            profile_ref,
            status,
            wall_time_ms,
            stdout_sha256,
            stderr_sha256,
        );
        Self {
            run_id: invocation.run_id.clone(),
            spec_digest: invocation.spec.spec_digest.clone(),
            profile_mode,
            profile_ref: profile_ref.to_string(),
            status,
            wall_time_ms,
            stdout_sha256: stdout_sha256.to_string(),
            stderr_sha256: stderr_sha256.to_string(),
            receipt_digest,
        }
    }

    fn as_receipt_ref(&self) -> ExperimentReceiptRef {
        ExperimentReceiptRef::new(
            ExperimentReceiptKind::SandboxExecution,
            format!("receipt://research/{}", self.run_id),
            self.receipt_digest.clone(),
        )
    }
}

/// In-memory record returned by the library runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchRunnerRecord {
    /// Typed result manifest for the run.
    pub result: ExperimentResult,
    /// Typed execution receipt for the run.
    pub receipt: ResearchExecutionReceipt,
    /// Runner stdout payload.
    pub stdout_log: String,
    /// Runner stderr payload.
    pub stderr_log: String,
}

/// File outputs written by the compiled CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchRunnerArtifacts {
    /// Manifest file path.
    pub result_path: PathBuf,
    /// Stdout log path.
    pub stdout_path: PathBuf,
    /// Stderr log path.
    pub stderr_path: PathBuf,
}

/// Typed failure while invoking or persisting the local runner.
#[derive(Debug, Error)]
pub enum ResearchRunnerError {
    /// An invocation was structurally invalid.
    #[error("invalid runner invocation: {0}")]
    InvalidInvocation(String),
    /// The runner could not persist one output file.
    #[error("failed to persist runner artifacts at {path}: {detail}")]
    PersistFailure { path: String, detail: String },
    /// The executor benchmark backend failed.
    #[error("tassadar benchmark failed: {0}")]
    TassadarBenchmark(String),
    /// The compiled-weight suite builder failed.
    #[error("tassadar compiled-weight build failed: {0}")]
    TassadarCompiledWeights(String),
}

/// Failure while executing or comparing a sweep of comparable research runs.
#[derive(Debug, Error)]
pub enum ResearchSweepError {
    /// The sweep carried no runs.
    #[error("research sweep is empty")]
    EmptySweep,
    /// One member failed to execute.
    #[error(transparent)]
    Runner(#[from] ResearchRunnerError),
    /// A result could not be evaluated under the shared score contract.
    #[error(transparent)]
    Evaluation(#[from] ExperimentScoreEvaluationError),
    /// Two evaluations could not be compared.
    #[error(transparent)]
    Comparison(#[from] ExperimentComparisonError),
    /// One member belonged to a different family.
    #[error("research sweep family mismatch: expected {expected}, found {actual}")]
    FamilyMismatch { expected: String, actual: String },
    /// One member used a different score contract.
    #[error("research sweep score contract mismatch: expected {expected}, found {actual}")]
    ScoreContractMismatch { expected: String, actual: String },
}

/// Executes bounded research experiments under an explicit runtime or sandbox profile.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResearchRunner;

impl ResearchRunner {
    /// Executes one typed invocation locally and returns its typed record.
    pub fn execute_local(
        invocation: &ResearchRunnerInvocation,
    ) -> Result<ResearchRunnerRecord, ResearchRunnerError> {
        let (profile_mode, profile_ref) = select_profile(&invocation.spec.budget, &invocation.spec);
        let Some((profile_mode, profile_ref)) = profile_mode.zip(profile_ref) else {
            let stdout_log = format!(
                "run_id={} family={} status=sandbox_mismatch reason=missing_execution_profile",
                invocation.run_id,
                invocation.spec.family.kind().label()
            );
            let stderr_log = String::from(
                "research runner refused the invocation because neither sandbox_profile_ref nor runtime_profile_ref was present",
            );
            let stdout_sha256 = stable_text_digest(stdout_log.as_str());
            let stderr_sha256 = stable_text_digest(stderr_log.as_str());
            let receipt = ResearchExecutionReceipt::new(
                invocation,
                RunnerProfileMode::Runtime,
                "missing",
                ExperimentRunStatus::SandboxMismatch,
                0,
                stdout_sha256.as_str(),
                stderr_sha256.as_str(),
            );
            let result = ExperimentResult::new_failure(
                invocation.run_id.clone(),
                &invocation.spec,
                invocation.started_at_ms,
                invocation.started_at_ms,
                ExperimentRunStatus::SandboxMismatch,
                ExperimentFailureReason::MissingExecutionProfile,
                "missing sandbox_profile_ref and runtime_profile_ref",
                vec![receipt.as_receipt_ref()],
                stdout_sha256,
                stderr_sha256,
            );
            return Ok(ResearchRunnerRecord {
                result,
                receipt,
                stdout_log,
                stderr_log,
            });
        };

        let estimated_runtime_ms = estimate_runtime_ms(&invocation.spec);
        if estimated_runtime_ms > invocation.spec.budget.max_wall_time_ms {
            let stdout_log = format!(
                "run_id={} family={} status=timed_out estimated_runtime_ms={} budget_ms={}",
                invocation.run_id,
                invocation.spec.family.kind().label(),
                estimated_runtime_ms,
                invocation.spec.budget.max_wall_time_ms
            );
            let stderr_log = format!(
                "budget too small for bounded experiment: estimated_runtime_ms={} budget_ms={}",
                estimated_runtime_ms, invocation.spec.budget.max_wall_time_ms
            );
            let stdout_sha256 = stable_text_digest(stdout_log.as_str());
            let stderr_sha256 = stable_text_digest(stderr_log.as_str());
            let receipt = ResearchExecutionReceipt::new(
                invocation,
                profile_mode,
                profile_ref.as_str(),
                ExperimentRunStatus::TimedOut,
                invocation.spec.budget.max_wall_time_ms,
                stdout_sha256.as_str(),
                stderr_sha256.as_str(),
            );
            let result = ExperimentResult::new_failure(
                invocation.run_id.clone(),
                &invocation.spec,
                invocation.started_at_ms,
                invocation.started_at_ms + invocation.spec.budget.max_wall_time_ms,
                ExperimentRunStatus::TimedOut,
                ExperimentFailureReason::BudgetTooSmall,
                format!(
                    "estimated_runtime_ms={} exceeds budget_ms={}",
                    estimated_runtime_ms, invocation.spec.budget.max_wall_time_ms
                ),
                vec![receipt.as_receipt_ref()],
                stdout_sha256,
                stderr_sha256,
            );
            return Ok(ResearchRunnerRecord {
                result,
                receipt,
                stdout_log,
                stderr_log,
            });
        }

        match &invocation.spec.family {
            ExperimentFamily::ExecutorVariants { executor } => execute_executor_variants(
                invocation,
                executor,
                profile_mode,
                profile_ref.as_str(),
                estimated_runtime_ms,
            ),
            _ => {
                let scores = synthesize_scores(&invocation.spec);
                let metrics = synthesize_metrics(&invocation.spec, estimated_runtime_ms);
                let stdout_log = format!(
                    "run_id={} family={} profile_mode={:?} profile_ref={} status=succeeded score_count={} metric_count={}",
                    invocation.run_id,
                    invocation.spec.family.kind().label(),
                    profile_mode,
                    profile_ref,
                    scores.len(),
                    metrics.len()
                );
                let stderr_log = String::new();
                let stdout_sha256 = stable_text_digest(stdout_log.as_str());
                let stderr_sha256 = stable_text_digest(stderr_log.as_str());
                let receipt = ResearchExecutionReceipt::new(
                    invocation,
                    profile_mode,
                    profile_ref.as_str(),
                    ExperimentRunStatus::Succeeded,
                    estimated_runtime_ms,
                    stdout_sha256.as_str(),
                    stderr_sha256.as_str(),
                );
                let mut receipt_refs = vec![receipt.as_receipt_ref()];
                receipt_refs.extend(additional_receipt_refs(invocation, estimated_runtime_ms));
                let artifact_outputs = vec![ExperimentArtifactOutput::new(
                    ExperimentArtifactKind::Auxiliary,
                    format!(
                        "artifact://research/{}/{}",
                        invocation.spec.family.kind().label(),
                        invocation.spec.candidate_id
                    ),
                    stable_artifact_digest(invocation),
                )];
                let result = ExperimentResult::new(
                    invocation.run_id.clone(),
                    &invocation.spec,
                    invocation.started_at_ms,
                    invocation.started_at_ms + estimated_runtime_ms,
                    ExperimentRunStatus::Succeeded,
                    scores,
                    metrics,
                    receipt_refs,
                    artifact_outputs,
                    stdout_sha256,
                    stderr_sha256,
                );
                Ok(ResearchRunnerRecord {
                    result,
                    receipt,
                    stdout_log,
                    stderr_log,
                })
            }
        }
    }

    /// Persists a completed runner record to manifest and log files.
    pub fn persist(
        record: &ResearchRunnerRecord,
        result_path: &Path,
    ) -> Result<ResearchRunnerArtifacts, ResearchRunnerError> {
        let Some(parent) = result_path.parent() else {
            return Err(ResearchRunnerError::PersistFailure {
                path: result_path.display().to_string(),
                detail: String::from("result path has no parent directory"),
            });
        };
        fs::create_dir_all(parent).map_err(|error| ResearchRunnerError::PersistFailure {
            path: parent.display().to_string(),
            detail: error.to_string(),
        })?;
        let stdout_path = result_path.with_extension("stdout.log");
        let stderr_path = result_path.with_extension("stderr.log");
        let serialized_result = serde_json::to_vec_pretty(&record.result).map_err(|error| {
            ResearchRunnerError::PersistFailure {
                path: result_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
        fs::write(result_path, serialized_result).map_err(|error| {
            ResearchRunnerError::PersistFailure {
                path: result_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
        fs::write(&stdout_path, record.stdout_log.as_bytes()).map_err(|error| {
            ResearchRunnerError::PersistFailure {
                path: stdout_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
        fs::write(&stderr_path, record.stderr_log.as_bytes()).map_err(|error| {
            ResearchRunnerError::PersistFailure {
                path: stderr_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
        Ok(ResearchRunnerArtifacts {
            result_path: result_path.to_path_buf(),
            stdout_path,
            stderr_path,
        })
    }

    /// Executes a comparable sweep of bounded research runs and returns the
    /// per-run records plus one machine-readable sweep summary.
    pub fn execute_local_sweep(
        sweep_id: impl Into<String>,
        invocations: &[ResearchRunnerInvocation],
    ) -> Result<(Vec<ResearchRunnerRecord>, ResearchSweepRecord), ResearchSweepError> {
        let Some(first) = invocations.first() else {
            return Err(ResearchSweepError::EmptySweep);
        };
        let family = first.spec.family.kind();
        let contract_digest = first.spec.score_contract.contract_digest.clone();
        let mut records = Vec::with_capacity(invocations.len());
        let mut entries = Vec::with_capacity(invocations.len());
        let mut winning_run_id = None;
        let mut winning_candidate_id = None;
        let mut winning_evaluation = None;

        for invocation in invocations {
            if invocation.spec.family.kind() != family {
                return Err(ResearchSweepError::FamilyMismatch {
                    expected: family.label().to_string(),
                    actual: invocation.spec.family.kind().label().to_string(),
                });
            }
            if invocation.spec.score_contract.contract_digest != contract_digest {
                return Err(ResearchSweepError::ScoreContractMismatch {
                    expected: contract_digest.clone(),
                    actual: invocation.spec.score_contract.contract_digest.clone(),
                });
            }

            let record = Self::execute_local(invocation)?;
            let evaluation = invocation
                .spec
                .score_contract
                .evaluate_result(&record.result)?;
            if let Some(current_winner) = &winning_evaluation {
                if evaluation.compare_same_contract(current_winner)?.is_gt() {
                    winning_run_id = Some(record.result.run_id.clone());
                    winning_candidate_id = Some(record.result.candidate_id.clone());
                    winning_evaluation = Some(evaluation.clone());
                }
            } else {
                winning_run_id = Some(record.result.run_id.clone());
                winning_candidate_id = Some(record.result.candidate_id.clone());
                winning_evaluation = Some(evaluation.clone());
            }

            entries.push(ResearchSweepEntry {
                run_id: record.result.run_id.clone(),
                candidate_id: record.result.candidate_id.clone(),
                result_digest: record.result.result_digest.clone(),
                weighted_score: evaluation.weighted_score,
                hard_gate_failed: evaluation.hard_gate_failed,
            });
            records.push(record);
        }

        let sweep = ResearchSweepRecord::new(
            sweep_id,
            family,
            contract_digest,
            entries,
            winning_run_id,
            winning_candidate_id,
        );
        Ok((records, sweep))
    }
}

fn execute_executor_variants(
    invocation: &ResearchRunnerInvocation,
    executor: &TassadarExecutorExperimentSpec,
    profile_mode: RunnerProfileMode,
    profile_ref: &str,
    estimated_runtime_ms: u64,
) -> Result<ResearchRunnerRecord, ResearchRunnerError> {
    validate_executor_variant(invocation, executor)?;
    let report = run_executor_benchmark(executor)?;
    let compiled_suite = maybe_build_compiled_weight_suite(invocation, executor, &report)?;
    let stats = build_executor_benchmark_stats(
        executor,
        &report,
        compiled_suite.as_ref(),
        estimated_runtime_ms,
    );
    let scores = build_executor_scores(invocation, &stats);
    let metrics = build_executor_metrics(&stats);
    let stdout_log = format!(
        "run_id={} family={} benchmark_target={:?} benchmark_ref={} decode_cache={:?} attention_mode={:?} weight_construction={:?} head_dim={} head_count={} d_model={} parameter_count_estimate={} compiled_artifact_bytes={} profile_mode={:?} profile_ref={} status=succeeded case_count={} exactness_bps={} candidate_speedup_ratio_micros={} candidate_cpu_gap_ratio_micros={}",
        invocation.run_id,
        invocation.spec.family.kind().label(),
        executor.benchmark_target,
        executor.benchmark_ref,
        executor.decode_cache.cache_kind,
        executor.decode_cache.attention_mode,
        executor.architecture.weight_construction,
        executor.architecture.head_dim,
        executor.architecture.head_count,
        stats.d_model,
        stats.parameter_count_estimate,
        stats.compiled_weight_artifact_bytes,
        profile_mode,
        profile_ref,
        stats.case_count,
        stats.exactness_bps,
        stats.candidate_speedup_ratio_micros,
        stats.candidate_cpu_gap_ratio_micros
    );
    let stderr_log = String::new();
    let stdout_sha256 = stable_text_digest(stdout_log.as_str());
    let stderr_sha256 = stable_text_digest(stderr_log.as_str());
    let receipt = ResearchExecutionReceipt::new(
        invocation,
        profile_mode,
        profile_ref,
        ExperimentRunStatus::Succeeded,
        estimated_runtime_ms,
        stdout_sha256.as_str(),
        stderr_sha256.as_str(),
    );
    let mut receipt_refs = vec![receipt.as_receipt_ref()];
    receipt_refs.push(build_executor_eval_receipt(invocation, &report)?);
    let artifact_outputs = build_executor_artifact_outputs(invocation, &report, compiled_suite.as_ref())?;
    let result = ExperimentResult::new(
        invocation.run_id.clone(),
        &invocation.spec,
        invocation.started_at_ms,
        invocation.started_at_ms + estimated_runtime_ms,
        ExperimentRunStatus::Succeeded,
        scores,
        metrics,
        receipt_refs,
        artifact_outputs,
        stdout_sha256,
        stderr_sha256,
    );
    Ok(ResearchRunnerRecord {
        result,
        receipt,
        stdout_log,
        stderr_log,
    })
}

#[derive(Clone, Copy)]
struct ExecutorBenchmarkStats {
    exactness_bps: i64,
    direct_selection_bps: i64,
    candidate_speedup_ratio_micros: i64,
    candidate_cpu_gap_ratio_micros: i64,
    d_model: i64,
    parameter_count_estimate: i64,
    compiled_weight_artifact_bytes: i64,
    compiled_program_count: i64,
    total_trace_steps: i64,
    case_count: i64,
    passed_case_count: i64,
    wall_time_ms: i64,
}

fn validate_executor_variant(
    invocation: &ResearchRunnerInvocation,
    executor: &TassadarExecutorExperimentSpec,
) -> Result<(), ResearchRunnerError> {
    require_executor_artifact(invocation, ExperimentArtifactKind::BenchmarkSuite)?;
    require_executor_artifact(invocation, ExperimentArtifactKind::ModelDescriptor)?;
    require_executor_artifact(invocation, ExperimentArtifactKind::ProgramArtifact)?;
    if executor.architecture.variant_id.trim().is_empty()
        || executor.architecture.model_id.trim().is_empty()
        || executor.trace_abi.variant_id.trim().is_empty()
        || executor.trace_abi.abi_id.trim().is_empty()
        || executor.wasm_profile.variant_id.trim().is_empty()
        || executor.wasm_profile.profile_id.trim().is_empty()
        || executor.decode_cache.variant_id.trim().is_empty()
    {
        return Err(ResearchRunnerError::InvalidInvocation(String::from(
            "executor experiment surfaces must declare non-empty variant and model ids",
        )));
    }
    if executor.architecture.head_count == 0 {
        return Err(ResearchRunnerError::InvalidInvocation(String::from(
            "executor architecture must declare a non-zero head_count",
        )));
    }
    if !executor.architecture.is_two_dimensional_lookup_family() {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "executor architecture `{}` left the 2D-head regime: head_dim={} head_count={}",
            executor.architecture.variant_id,
            executor.architecture.head_dim,
            executor.architecture.head_count
        )));
    }
    if executor.trace_abi.abi_id != "tassadar.trace.v1" || executor.trace_abi.schema_version != 1 {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "unsupported Tassadar trace ABI {}@{}",
            executor.trace_abi.abi_id, executor.trace_abi.schema_version
        )));
    }
    let (expected_benchmark_ref, expected_environment_ref, expected_profile_id) =
        expected_executor_target_contract(executor.benchmark_target);
    if executor.benchmark_ref != expected_benchmark_ref {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "benchmark_ref mismatch: expected `{expected_benchmark_ref}`, found `{}`",
            executor.benchmark_ref
        )));
    }
    if executor.environment_ref != expected_environment_ref {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "environment_ref mismatch: expected `{expected_environment_ref}`, found `{}`",
            executor.environment_ref
        )));
    }
    if executor.wasm_profile.profile_id != expected_profile_id {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "profile_id mismatch for benchmark target {:?}: expected `{expected_profile_id}`, found `{}`",
            executor.benchmark_target, executor.wasm_profile.profile_id
        )));
    }
    if executor.decode_cache.attention_mode != TassadarExecutorAttentionMode::HardMax {
        return Err(ResearchRunnerError::InvalidInvocation(String::from(
            "current Tassadar research backend only supports hard-max attention experiments",
        )));
    }
    if !executor.decode_cache.exact_required {
        return Err(ResearchRunnerError::InvalidInvocation(String::from(
            "current Tassadar research backend requires exact_required=true",
        )));
    }
    match executor.architecture.weight_construction {
        TassadarExecutorWeightConstruction::HandcraftedInterpreter => {}
        TassadarExecutorWeightConstruction::ProgramCompiled => {
            if executor.decode_cache.cache_kind == TassadarExecutorDecodeCacheKind::StandardKv {
                return Err(ResearchRunnerError::InvalidInvocation(String::from(
                    "program-compiled Tassadar candidates do not yet support standard-kv cache research",
                )));
            }
        }
    }
    match executor.decode_cache.cache_kind {
        TassadarExecutorDecodeCacheKind::ReferenceLinear
        | TassadarExecutorDecodeCacheKind::HullCache => {
            if executor.decode_cache.sparse_top_k.is_some() {
                return Err(ResearchRunnerError::InvalidInvocation(String::from(
                    "sparse_top_k must be unset for exact reference-linear or hull-cache candidates",
                )));
            }
        }
        TassadarExecutorDecodeCacheKind::SparseTopK => {
            if executor.decode_cache.sparse_top_k != Some(1) {
                return Err(ResearchRunnerError::InvalidInvocation(String::from(
                    "current Tassadar research backend only validates sparse-top-k candidates with sparse_top_k=1",
                )));
            }
        }
        TassadarExecutorDecodeCacheKind::StandardKv => {
            return Err(ResearchRunnerError::InvalidInvocation(String::from(
                "current Tassadar research backend does not yet support standard-kv executor candidates",
            )));
        }
    }
    Ok(())
}

fn require_executor_artifact(
    invocation: &ResearchRunnerInvocation,
    kind: ExperimentArtifactKind,
) -> Result<(), ResearchRunnerError> {
    if invocation
        .spec
        .base_artifacts
        .iter()
        .any(|artifact| artifact.kind == kind)
    {
        Ok(())
    } else {
        Err(ResearchRunnerError::InvalidInvocation(format!(
            "executor experiment is missing required base artifact kind `{kind:?}`"
        )))
    }
}

fn expected_executor_target_contract(
    target: TassadarExecutorBenchmarkTarget,
) -> (&'static str, &'static str, &'static str) {
    match target {
        TassadarExecutorBenchmarkTarget::ValidationCorpus => (
            TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF,
            TASSADAR_BENCHMARK_ENVIRONMENT_REF,
            "core_i32_v1",
        ),
        TassadarExecutorBenchmarkTarget::ArticleClass => (
            TASSADAR_ARTICLE_CLASS_BENCHMARK_REF,
            TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
            "core_i32_v2",
        ),
    }
}

fn run_executor_benchmark(
    executor: &TassadarExecutorExperimentSpec,
) -> Result<TassadarBenchmarkReport, ResearchRunnerError> {
    let report = match executor.benchmark_target {
        TassadarExecutorBenchmarkTarget::ValidationCorpus => {
            run_tassadar_reference_fixture_benchmark(executor.benchmark_version.as_str())
        }
        TassadarExecutorBenchmarkTarget::ArticleClass => {
            run_tassadar_article_class_benchmark(executor.benchmark_version.as_str())
        }
    }
    .map_err(|error| ResearchRunnerError::TassadarBenchmark(error.to_string()))?;
    if report.suite.benchmark_package.key.benchmark_ref != executor.benchmark_ref {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "benchmark report returned `{}` but executor experiment declared `{}`",
            report.suite.benchmark_package.key.benchmark_ref, executor.benchmark_ref
        )));
    }
    if report.suite.benchmark_package.environment.environment_ref != executor.environment_ref {
        return Err(ResearchRunnerError::InvalidInvocation(format!(
            "benchmark report returned environment `{}` but executor experiment declared `{}`",
            report.suite.benchmark_package.environment.environment_ref, executor.environment_ref
        )));
    }
    Ok(report)
}

fn maybe_build_compiled_weight_suite(
    invocation: &ResearchRunnerInvocation,
    executor: &TassadarExecutorExperimentSpec,
    report: &TassadarBenchmarkReport,
) -> Result<Option<TassadarCompiledProgramSuiteArtifact>, ResearchRunnerError> {
    if executor.architecture.weight_construction
        != TassadarExecutorWeightConstruction::ProgramCompiled
    {
        return Ok(None);
    }
    let fixture = match executor.wasm_profile.profile_id.as_str() {
        "core_i32_v1" => TassadarExecutorFixture::core_i32_v1(),
        "core_i32_v2" => TassadarExecutorFixture::core_i32_v2(),
        profile_id => TassadarExecutorFixture::for_profile_id(profile_id).ok_or_else(|| {
            ResearchRunnerError::InvalidInvocation(format!(
                "no Tassadar fixture exists for profile `{profile_id}`"
            ))
        })?,
    };
    let suite = TassadarCompiledProgramSuiteArtifact::compile(
        format!("{}.compiled_suite", invocation.spec.candidate_id),
        format!("{}@{}", executor.benchmark_ref, executor.benchmark_version),
        &fixture,
        report.suite.artifacts.as_slice(),
    )
    .map_err(|error| ResearchRunnerError::TassadarCompiledWeights(error.to_string()))?;
    Ok(Some(suite))
}

fn build_executor_benchmark_stats(
    executor: &TassadarExecutorExperimentSpec,
    report: &TassadarBenchmarkReport,
    compiled_suite: Option<&TassadarCompiledProgramSuiteArtifact>,
    estimated_runtime_ms: u64,
) -> ExecutorBenchmarkStats {
    let case_count = saturating_i64_from_usize(report.case_reports.len());
    let passed_case_count = saturating_i64_from_usize(
        report
            .case_reports
            .iter()
            .filter(|case| case.score_bps == 10_000)
            .count(),
    );
    let total_trace_steps = report
        .case_reports
        .iter()
        .map(|case| saturating_i64_from_u64(case.trace_steps))
        .sum();
    let exactness_sum: i64 = report
        .case_reports
        .iter()
        .map(|case| i64::from(case.score_bps))
        .sum();
    let exactness_bps = if case_count == 0 {
        0
    } else {
        exactness_sum / case_count
    };
    let direct_selection_bps = match executor.decode_cache.cache_kind {
        TassadarExecutorDecodeCacheKind::ReferenceLinear => 10_000,
        TassadarExecutorDecodeCacheKind::HullCache => {
            let direct_count = saturating_i64_from_usize(
                report
                    .case_reports
                    .iter()
                    .filter(|case| !case.used_decode_fallback)
                    .count(),
            );
            if case_count == 0 {
                0
            } else {
                direct_count.saturating_mul(10_000) / case_count
            }
        }
        TassadarExecutorDecodeCacheKind::SparseTopK => {
            let direct_count = saturating_i64_from_usize(
                report
                    .case_reports
                    .iter()
                    .filter(|case| !case.sparse_top_k_used_decode_fallback)
                    .count(),
            );
            if case_count == 0 {
                0
            } else {
                direct_count.saturating_mul(10_000) / case_count
            }
        }
        TassadarExecutorDecodeCacheKind::StandardKv => 0,
    };
    let candidate_speedup_ratio_micros =
        average_ratio_micros(report.case_reports.iter().map(|case| {
            match executor.decode_cache.cache_kind {
                TassadarExecutorDecodeCacheKind::ReferenceLinear => 1.0,
                TassadarExecutorDecodeCacheKind::HullCache => {
                    case.hull_cache_speedup_over_reference_linear
                }
                TassadarExecutorDecodeCacheKind::SparseTopK => {
                    case.sparse_top_k_speedup_over_reference_linear
                }
                TassadarExecutorDecodeCacheKind::StandardKv => 0.0,
            }
        }));
    let candidate_cpu_gap_ratio_micros =
        average_ratio_micros(report.case_reports.iter().map(|case| {
            match executor.decode_cache.cache_kind {
                TassadarExecutorDecodeCacheKind::ReferenceLinear => {
                    case.cpu_reference_steps_per_second
                        / case.reference_linear_steps_per_second.max(1e-9)
                }
                TassadarExecutorDecodeCacheKind::HullCache => {
                    case.hull_cache_remaining_gap_vs_cpu_reference
                }
                TassadarExecutorDecodeCacheKind::SparseTopK => {
                    case.sparse_top_k_remaining_gap_vs_cpu_reference
                }
                TassadarExecutorDecodeCacheKind::StandardKv => 0.0,
            }
        }));
    let d_model = i64::from(executor.architecture.d_model());
    let parameter_count_estimate =
        saturating_i64_from_u64(executor.architecture.estimated_parameter_count());
    let (compiled_weight_artifact_bytes, compiled_program_count) = compiled_suite
        .map(|suite| {
            (
                saturating_i64_from_u64(suite.total_compiled_weight_artifact_bytes),
                saturating_i64_from_usize(suite.deployments.len()),
            )
        })
        .unwrap_or((0, 0));
    ExecutorBenchmarkStats {
        exactness_bps,
        direct_selection_bps,
        candidate_speedup_ratio_micros,
        candidate_cpu_gap_ratio_micros,
        d_model,
        parameter_count_estimate,
        compiled_weight_artifact_bytes,
        compiled_program_count,
        total_trace_steps,
        case_count,
        passed_case_count,
        wall_time_ms: saturating_i64_from_u64(estimated_runtime_ms),
    }
}

fn build_executor_scores(
    invocation: &ResearchRunnerInvocation,
    stats: &ExecutorBenchmarkStats,
) -> Vec<ExperimentScore> {
    invocation
        .spec
        .score_contract
        .metrics
        .iter()
        .map(|metric| {
            ExperimentScore::new(
                metric.metric_id.clone(),
                metric.unit.clone(),
                executor_metric_value(stats, metric.metric_id.as_str()),
            )
        })
        .collect()
}

fn build_executor_metrics(stats: &ExecutorBenchmarkStats) -> Vec<ExperimentMetric> {
    vec![
        ExperimentMetric::new("wall_time_ms", "milliseconds", stats.wall_time_ms),
        ExperimentMetric::new("executor_exactness_bps", "bps", stats.exactness_bps),
        ExperimentMetric::new("executor_d_model", "count", stats.d_model),
        ExperimentMetric::new(
            "executor_parameter_count_estimate",
            "count",
            stats.parameter_count_estimate,
        ),
        ExperimentMetric::new(
            "executor_direct_selection_bps",
            "bps",
            stats.direct_selection_bps,
        ),
        ExperimentMetric::new(
            "executor_candidate_speedup_ratio_micros",
            "ratio_micros",
            stats.candidate_speedup_ratio_micros,
        ),
        ExperimentMetric::new(
            "executor_candidate_cpu_gap_ratio_micros",
            "ratio_micros",
            stats.candidate_cpu_gap_ratio_micros,
        ),
        ExperimentMetric::new(
            "executor_total_trace_steps",
            "steps",
            stats.total_trace_steps,
        ),
        ExperimentMetric::new("executor_case_count", "count", stats.case_count),
        ExperimentMetric::new(
            "executor_passed_case_count",
            "count",
            stats.passed_case_count,
        ),
        ExperimentMetric::new(
            "executor_compiled_weight_artifact_bytes",
            "bytes",
            stats.compiled_weight_artifact_bytes,
        ),
        ExperimentMetric::new(
            "executor_compiled_program_count",
            "count",
            stats.compiled_program_count,
        ),
    ]
}

fn executor_metric_value(stats: &ExecutorBenchmarkStats, metric_id: &str) -> i64 {
    if metric_id.contains("exactness") {
        stats.exactness_bps
    } else if metric_id.contains("parameter_count") {
        stats.parameter_count_estimate
    } else if metric_id.contains("compiled_weight_artifact_bytes")
        || metric_id.contains("bundle_bytes")
    {
        stats.compiled_weight_artifact_bytes
    } else if metric_id.contains("compiled_program_count") {
        stats.compiled_program_count
    } else if metric_id.contains("d_model") {
        stats.d_model
    } else if metric_id.contains("direct_selection") {
        stats.direct_selection_bps
    } else if metric_id.contains("speedup") {
        stats.candidate_speedup_ratio_micros
    } else if metric_id.contains("cpu_gap") || metric_id.contains("gap") {
        stats.candidate_cpu_gap_ratio_micros
    } else if metric_id.contains("trace_steps") {
        stats.total_trace_steps
    } else if metric_id.contains("passed_case_count") {
        stats.passed_case_count
    } else if metric_id.contains("case_count") {
        stats.case_count
    } else {
        stats.wall_time_ms
    }
}

fn build_executor_eval_receipt(
    invocation: &ResearchRunnerInvocation,
    report: &TassadarBenchmarkReport,
) -> Result<ExperimentReceiptRef, ResearchRunnerError> {
    let eval_run_bytes = serde_json::to_vec(&report.eval_run)
        .map_err(|error| ResearchRunnerError::TassadarBenchmark(error.to_string()))?;
    Ok(ExperimentReceiptRef::new(
        ExperimentReceiptKind::EvalRun,
        format!(
            "receipt://eval/{}/{}",
            invocation.spec.experiment_id, invocation.run_id
        ),
        stable_bytes_digest(eval_run_bytes.as_slice()),
    ))
}

fn build_executor_artifact_outputs(
    invocation: &ResearchRunnerInvocation,
    report: &TassadarBenchmarkReport,
    compiled_suite: Option<&TassadarCompiledProgramSuiteArtifact>,
) -> Result<Vec<ExperimentArtifactOutput>, ResearchRunnerError> {
    let benchmark_report_bytes = serde_json::to_vec(report)
        .map_err(|error| ResearchRunnerError::TassadarBenchmark(error.to_string()))?;
    let benchmark_suite_bytes = serde_json::to_vec(&report.suite.benchmark_package)
        .map_err(|error| ResearchRunnerError::TassadarBenchmark(error.to_string()))?;
    let program_artifact_bytes = serde_json::to_vec(&report.suite.artifacts)
        .map_err(|error| ResearchRunnerError::TassadarBenchmark(error.to_string()))?;
    let mut artifacts = vec![
        ExperimentArtifactOutput::new(
            ExperimentArtifactKind::BenchmarkReport,
            format!(
                "artifact://research/{}/benchmark_report",
                invocation.spec.candidate_id
            ),
            stable_bytes_digest(benchmark_report_bytes.as_slice()),
        ),
        ExperimentArtifactOutput::new(
            ExperimentArtifactKind::BenchmarkSuite,
            report.suite.benchmark_package.key.storage_key(),
            stable_bytes_digest(benchmark_suite_bytes.as_slice()),
        ),
        ExperimentArtifactOutput::new(
            ExperimentArtifactKind::ProgramArtifact,
            format!(
                "artifact://research/{}/program_artifacts",
                invocation.spec.candidate_id
            ),
            stable_bytes_digest(program_artifact_bytes.as_slice()),
        ),
        ExperimentArtifactOutput::new(
            ExperimentArtifactKind::RuntimeManifest,
            format!(
                "artifact://research/{}/runtime_manifests",
                invocation.spec.candidate_id
            ),
            collect_eval_artifact_digest(report, "tassadar_runtime_manifest.json"),
        ),
        ExperimentArtifactOutput::new(
            ExperimentArtifactKind::ExecutionProofBundle,
            format!(
                "artifact://research/{}/execution_proof_bundles",
                invocation.spec.candidate_id
            ),
            collect_eval_artifact_digest(report, "tassadar_execution_proof_bundle.json"),
        ),
    ];
    if let Some(compiled_suite) = compiled_suite {
        artifacts.push(ExperimentArtifactOutput::new(
            ExperimentArtifactKind::CompiledWeightArtifact,
            format!(
                "artifact://research/{}/compiled_weight_suite",
                invocation.spec.candidate_id
            ),
            compiled_suite.artifact_digest.clone(),
        ));
    }
    Ok(artifacts)
}

fn collect_eval_artifact_digest(report: &TassadarBenchmarkReport, artifact_kind: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_research_eval_artifact_set|");
    hasher.update(artifact_kind.as_bytes());
    for sample in &report.eval_run.samples {
        for artifact in &sample.artifacts {
            if artifact.artifact_kind == artifact_kind {
                hasher.update(b"|artifact_ref|");
                hasher.update(artifact.artifact_ref.as_bytes());
                hasher.update(b"|artifact_digest|");
                hasher.update(artifact.artifact_digest.as_bytes());
            }
        }
    }
    hex::encode(hasher.finalize())
}

fn average_ratio_micros(values: impl Iterator<Item = f64>) -> i64 {
    let mut total = 0.0_f64;
    let mut count = 0_u64;
    for value in values {
        total += value;
        count = count.saturating_add(1);
    }
    if count == 0 {
        return 0;
    }
    ratio_to_micros(total / (count as f64))
}

fn ratio_to_micros(value: f64) -> i64 {
    if !value.is_finite() {
        return 0;
    }
    let scaled = value * 1_000_000.0;
    if scaled >= i64::MAX as f64 {
        i64::MAX
    } else if scaled <= i64::MIN as f64 {
        i64::MIN
    } else {
        scaled.round() as i64
    }
}

fn saturating_i64_from_usize(value: usize) -> i64 {
    match i64::try_from(value) {
        Ok(value) => value,
        Err(_) => i64::MAX,
    }
}

fn saturating_i64_from_u64(value: u64) -> i64 {
    match i64::try_from(value) {
        Ok(value) => value,
        Err(_) => i64::MAX,
    }
}

fn select_profile(
    _budget: &ExperimentBudget,
    spec: &ExperimentSpec,
) -> (Option<RunnerProfileMode>, Option<String>) {
    if let Some(sandbox_profile_ref) = spec.runtime_profile.sandbox_profile_ref.as_ref() {
        return (
            Some(RunnerProfileMode::Sandbox),
            Some(sandbox_profile_ref.clone()),
        );
    }
    if let Some(runtime_profile_ref) = spec.runtime_profile.runtime_profile_ref.as_ref() {
        return (
            Some(RunnerProfileMode::Runtime),
            Some(runtime_profile_ref.clone()),
        );
    }
    (None, None)
}

fn estimate_runtime_ms(spec: &ExperimentSpec) -> u64 {
    match &spec.family {
        ExperimentFamily::ServingScheduler { policy, .. } => {
            u64::from(policy.max_active_sequences) * 120
                + policy.queue_slack_ms * 10
                + u64::from(policy.max_batch_tokens / 8)
        }
        ExperimentFamily::BackendTuning { policy, .. } => {
            900 + u64::from(policy.preferred_chunk_tokens / 4)
                + u64::from(policy.compile_parallelism) * 75
        }
        ExperimentFamily::DatastreamTransfer { policy } => {
            1_200
                + u64::from(policy.concurrent_streams) * 90
                + u64::from(policy.prefetch_depth) * 40
        }
        ExperimentFamily::SandboxWarmPool { policy, .. } => {
            600 + u64::from(policy.max_warm_workers) * 180 + policy.reuse_ttl_ms / 200
        }
        ExperimentFamily::TrainingPolicy { policy, .. } => {
            2_000 + policy.checkpoint_every_steps.saturating_mul(3)
        }
        ExperimentFamily::ValidatorPolicy { policy, .. } => {
            1_000 + u64::from(policy.sample_rate_bps) + u64::from(policy.verdict_sample_rate_bps)
        }
        ExperimentFamily::EnvironmentMix { mix } => {
            1_200
                + mix
                    .environments
                    .iter()
                    .map(|env| env.timeout_ms / 100)
                    .sum::<u64>()
        }
        ExperimentFamily::ExecutorVariants { executor } => {
            let target_base = match executor.benchmark_target {
                TassadarExecutorBenchmarkTarget::ValidationCorpus => 2_500,
                TassadarExecutorBenchmarkTarget::ArticleClass => 4_500,
            };
            let architecture_cost = u64::from(executor.architecture.layer_count) * 40
                + u64::from(executor.architecture.head_count) * 12
                + u64::from(executor.architecture.feed_forward_width / 4);
            let compile_cost = match executor.architecture.weight_construction {
                TassadarExecutorWeightConstruction::HandcraftedInterpreter => 0,
                TassadarExecutorWeightConstruction::ProgramCompiled => {
                    u64::from(executor.wasm_profile.max_program_len) * 6
                        + u64::from(executor.wasm_profile.max_memory_slots) * 20
                }
            };
            target_base
                + executor.wasm_profile.max_steps.saturating_mul(2)
                + u64::from(executor.wasm_profile.max_program_len) * 4
                + architecture_cost
                + compile_cost
        }
    }
}

fn synthesize_scores(spec: &ExperimentSpec) -> Vec<ExperimentScore> {
    spec.score_contract
        .metrics
        .iter()
        .map(|metric| {
            ExperimentScore::new(
                metric.metric_id.clone(),
                metric.unit.clone(),
                synthetic_metric_value(spec, metric.metric_id.as_str()),
            )
        })
        .collect()
}

fn synthesize_metrics(spec: &ExperimentSpec, estimated_runtime_ms: u64) -> Vec<ExperimentMetric> {
    vec![
        ExperimentMetric::new("wall_time_ms", "milliseconds", estimated_runtime_ms as i64),
        ExperimentMetric::new(
            "profile_mode",
            "enum_discriminant",
            match select_profile(&spec.budget, spec).0 {
                Some(RunnerProfileMode::Sandbox) => 1,
                Some(RunnerProfileMode::Runtime) => 2,
                None => 0,
            },
        ),
    ]
}

fn synthetic_metric_value(spec: &ExperimentSpec, metric_id: &str) -> i64 {
    match &spec.family {
        ExperimentFamily::ServingScheduler { policy, .. } => {
            if metric_id.contains("throughput") {
                120_000_000
                    + i64::from(policy.max_batch_tokens) * 9_000
                    + i64::from(policy.decode_share_bps) * 1_500
                    - i64::from(policy.prefill_share_bps) * 300
            } else if metric_id.contains("latency") {
                42_000
                    + i64::from(policy.max_active_sequences) * 2_000
                    + i64::try_from(policy.queue_slack_ms).unwrap_or(i64::MAX) * 15
            } else if metric_id.contains("memory") {
                18_000_000_000
                    + i64::from(policy.max_batch_tokens) * 250_000
                    + i64::from(policy.max_active_sequences) * 50_000_000
            } else if metric_id.contains("conformance") {
                999_500
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::TrainingPolicy { policy, .. } => {
            if metric_id.contains("eval") || metric_id.contains("score") {
                700_000 + i64::from(policy.off_policy_budget_bps) * 30
                    - i64::from(policy.instability_loss_spike_bps) * 20
            } else if metric_id.contains("loss") {
                350_000 - i64::from(policy.off_policy_budget_bps) * 10
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::BackendTuning { policy, .. } => {
            if metric_id.contains("throughput") {
                100_000_000
                    + i64::from(policy.preferred_chunk_tokens) * 8_000
                    + if policy.kernel_fusion { 8_000_000 } else { 0 }
            } else if metric_id.contains("latency") {
                55_000 - i64::from(policy.compile_parallelism) * 500
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::DatastreamTransfer { policy } => {
            if metric_id.contains("throughput") {
                i64::from(policy.chunk_bytes) * i64::from(policy.concurrent_streams)
            } else if metric_id.contains("latency") {
                70_000 - i64::from(policy.prefetch_depth) * 1_000
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::SandboxWarmPool { policy, .. } => {
            if metric_id.contains("latency") {
                25_000 - i64::from(policy.min_warm_workers) * 2_000
            } else if metric_id.contains("memory") {
                i64::from(policy.max_warm_workers) * 2_500_000_000
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::ValidatorPolicy { policy, .. } => {
            if metric_id.contains("score") {
                500_000 + i64::from(policy.sample_rate_bps) * 25
                    - i64::from(policy.duplicate_signature_limit) * 100
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::EnvironmentMix { mix } => {
            if metric_id.contains("score") {
                400_000 + i64::try_from(mix.environments.len()).unwrap_or(0) * 25_000
            } else {
                i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
            }
        }
        ExperimentFamily::ExecutorVariants { .. } => {
            i64::try_from(estimate_runtime_ms(spec)).unwrap_or(i64::MAX)
        }
    }
}

fn additional_receipt_refs(
    invocation: &ResearchRunnerInvocation,
    estimated_runtime_ms: u64,
) -> Vec<ExperimentReceiptRef> {
    match &invocation.spec.family {
        ExperimentFamily::ServingScheduler { .. } => vec![ExperimentReceiptRef::new(
            ExperimentReceiptKind::ServingBenchmark,
            format!(
                "receipt://serving/{}/{}",
                invocation.spec.experiment_id, invocation.run_id
            ),
            stable_aux_receipt_digest(invocation, "serving_benchmark", estimated_runtime_ms),
        )],
        ExperimentFamily::TrainingPolicy { .. } => vec![
            ExperimentReceiptRef::new(
                ExperimentReceiptKind::TrainingRun,
                format!(
                    "receipt://train/{}/{}",
                    invocation.spec.experiment_id, invocation.run_id
                ),
                stable_aux_receipt_digest(invocation, "training_run", estimated_runtime_ms),
            ),
            ExperimentReceiptRef::new(
                ExperimentReceiptKind::EvalRun,
                format!(
                    "receipt://eval/{}/{}",
                    invocation.spec.experiment_id, invocation.run_id
                ),
                stable_aux_receipt_digest(invocation, "eval_run", estimated_runtime_ms),
            ),
        ],
        ExperimentFamily::ValidatorPolicy { .. } => vec![ExperimentReceiptRef::new(
            ExperimentReceiptKind::ValidatorVerdict,
            format!(
                "receipt://validator/{}/{}",
                invocation.spec.experiment_id, invocation.run_id
            ),
            stable_aux_receipt_digest(invocation, "validator_verdict", estimated_runtime_ms),
        )],
        ExperimentFamily::ExecutorVariants { .. } => Vec::new(),
        _ => Vec::new(),
    }
}

fn stable_artifact_digest(invocation: &ResearchRunnerInvocation) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_research_artifact|");
    hasher.update(invocation.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(invocation.spec.spec_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_text_digest(text: &str) -> String {
    stable_bytes_digest(text.as_bytes())
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn stable_aux_receipt_digest(
    invocation: &ResearchRunnerInvocation,
    receipt_kind: &str,
    estimated_runtime_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_research_aux_receipt|");
    hasher.update(receipt_kind.as_bytes());
    hasher.update(b"|");
    hasher.update(invocation.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(invocation.spec.spec_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(estimated_runtime_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_research_execution_receipt_digest(
    run_id: &str,
    spec_digest: &str,
    profile_mode: RunnerProfileMode,
    profile_ref: &str,
    status: ExperimentRunStatus,
    wall_time_ms: u64,
    stdout_sha256: &str,
    stderr_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_research_execution_receipt|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(spec_digest.as_bytes());
    hasher.update(b"|profile_mode|");
    hasher.update(
        serde_json::to_vec(&profile_mode)
            .unwrap_or_else(|_| unreachable!("profile mode should serialize")),
    );
    hasher.update(b"|profile_ref|");
    hasher.update(profile_ref.as_bytes());
    hasher.update(b"|status|");
    hasher.update(
        serde_json::to_vec(&status).unwrap_or_else(|_| unreachable!("status should serialize")),
    );
    hasher.update(b"|wall_time_ms|");
    hasher.update(wall_time_ms.to_string().as_bytes());
    hasher.update(b"|stdout|");
    hasher.update(stdout_sha256.as_bytes());
    hasher.update(b"|stderr|");
    hasher.update(stderr_sha256.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_eval::{
        TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF, TASSADAR_ARTICLE_CLASS_BENCHMARK_REF,
    };
    use tempfile::tempdir;

    use crate::{
        CandidateMutation, ExperimentArtifactKind, ExperimentArtifactRef, ExperimentBudget,
        ExperimentFamily, ExperimentFamilyKind, ExperimentRunStatus, ExperimentScoreContract,
        ExperimentThreshold, ResearchRunner, ResearchRunnerError, ResearchRunnerInvocation,
        ScoreDirection, ScoreMetricSpec, ServingSchedulerPolicy,
        TassadarExecutorArchitectureVariant,
        TassadarExecutorAttentionMode, TassadarExecutorBenchmarkTarget,
        TassadarExecutorDecodeCacheKind, TassadarExecutorDecodeCacheVariant,
        TassadarExecutorExperimentSpec, TassadarExecutorTraceAbiVariant,
        TassadarExecutorWeightConstruction,
        TassadarExecutorWasmProfileVariant,
    };

    fn sample_serving_invocation() -> ResearchRunnerInvocation {
        let spec = crate::ExperimentSpec::new(
            "exp.serve.1",
            "candidate-a",
            ExperimentFamily::ServingScheduler {
                model_id: String::from("gpt-oss-20b"),
                benchmark_suite_ref: String::from("benchmark://serve/local-weather"),
                policy: ServingSchedulerPolicy::new(8192, 8, 4_500, 5_500, 25),
            },
            vec![ExperimentArtifactRef::new(
                ExperimentArtifactKind::ServedArtifact,
                "served://gpt-oss-20b",
                "served-digest-a",
            )],
            CandidateMutation::new(
                "mutation-a",
                Some(String::from("baseline")),
                ExperimentFamilyKind::ServingScheduler,
                vec![String::from("serve.scheduler.prefill_share_bps")],
            ),
            crate::ExperimentRuntimeProfile::new("runner-digest")
                .with_sandbox_profile_ref("sandbox://research/local")
                .with_requested_backend("cuda"),
            ExperimentBudget::new(30_000, "runs/serve"),
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
                        3_000,
                    )
                    .with_hard_gate(ExperimentThreshold::at_most(65_000)),
                ],
            ),
        );
        ResearchRunnerInvocation::new("run-a", spec, 1_000)
    }

    fn sample_executor_invocation(
        candidate_id: &str,
        run_id: &str,
        cache_kind: TassadarExecutorDecodeCacheKind,
        weight_construction: TassadarExecutorWeightConstruction,
    ) -> ResearchRunnerInvocation {
        let spec = crate::ExperimentSpec::new(
            "exp.tassadar.1",
            candidate_id,
            ExperimentFamily::ExecutorVariants {
                executor: TassadarExecutorExperimentSpec::new(
                    TassadarExecutorBenchmarkTarget::ArticleClass,
                    TASSADAR_ARTICLE_CLASS_BENCHMARK_REF,
                    "2026.03.16",
                    TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
                    TassadarExecutorArchitectureVariant::new(
                        format!("{candidate_id}.arch"),
                        "tassadar.executor.article_class.v2",
                        2,
                        18,
                        7,
                        36,
                        weight_construction,
                    ),
                    TassadarExecutorTraceAbiVariant::new(
                        format!("{candidate_id}.abi"),
                        "tassadar.trace.v1",
                        1,
                        true,
                        true,
                        true,
                        true,
                    ),
                    TassadarExecutorWasmProfileVariant::new(
                        format!("{candidate_id}.profile"),
                        "core_i32_v2",
                        8,
                        16,
                        128,
                        512,
                    ),
                    TassadarExecutorDecodeCacheVariant::new(
                        format!("{candidate_id}.cache"),
                        cache_kind,
                        TassadarExecutorAttentionMode::HardMax,
                        None,
                        true,
                    ),
                ),
            },
            vec![
                ExperimentArtifactRef::new(
                    ExperimentArtifactKind::BenchmarkSuite,
                    format!("{}@2026.03.16", TASSADAR_ARTICLE_CLASS_BENCHMARK_REF),
                    "benchmark-suite-digest",
                ),
                ExperimentArtifactRef::new(
                    ExperimentArtifactKind::ModelDescriptor,
                    "model://tassadar/article_class_fixture",
                    "model-descriptor-digest",
                ),
                ExperimentArtifactRef::new(
                    ExperimentArtifactKind::ProgramArtifact,
                    "artifact://tassadar/article_class/programs",
                    "program-artifact-digest",
                ),
            ],
            CandidateMutation::new(
                format!("{candidate_id}.mutation"),
                Some(String::from("baseline")),
                ExperimentFamilyKind::ExecutorVariants,
                vec![
                    String::from("tassadar.executor.decode_cache"),
                    String::from("tassadar.executor.wasm_profile"),
                ],
            ),
            crate::ExperimentRuntimeProfile::new("runner-digest-executor")
                .with_runtime_profile_ref("runtime://research/local"),
            ExperimentBudget::new(20_000, "runs/tassadar"),
            ExperimentScoreContract::new(
                "tassadar.executor.score.v1",
                ExperimentFamilyKind::ExecutorVariants,
                vec![
                    ScoreMetricSpec::new(
                        "executor_exactness_bps",
                        "bps",
                        ScoreDirection::Maximize,
                        6_000,
                    )
                    .with_hard_gate(ExperimentThreshold::at_least(10_000)),
                    ScoreMetricSpec::new(
                        "executor_candidate_speedup_ratio_micros",
                        "ratio_micros",
                        ScoreDirection::Maximize,
                        3_000,
                    ),
                    ScoreMetricSpec::new(
                        "executor_candidate_cpu_gap_ratio_micros",
                        "ratio_micros",
                        ScoreDirection::Minimize,
                        1_000,
                    )
                    .with_hard_gate(ExperimentThreshold::at_most(5_000_000)),
                ],
            ),
        );
        ResearchRunnerInvocation::new(run_id, spec, 2_000)
    }

    #[test]
    fn runner_executes_serving_experiment_end_to_end() {
        let invocation = sample_serving_invocation();
        let record = ResearchRunner::execute_local(&invocation).expect("runner should succeed");
        assert_eq!(record.result.status, ExperimentRunStatus::Succeeded);
        assert!(record.result.scores.len() >= 2);
        assert!(
            record
                .result
                .receipt_refs
                .iter()
                .any(|receipt| receipt.kind == crate::ExperimentReceiptKind::ServingBenchmark)
        );
    }

    #[test]
    fn runner_marks_missing_profile_as_sandbox_mismatch() {
        let mut invocation = sample_serving_invocation();
        invocation.spec.runtime_profile.sandbox_profile_ref = None;
        invocation.spec.runtime_profile.runtime_profile_ref = None;
        let record = ResearchRunner::execute_local(&invocation).expect("runner should succeed");
        assert_eq!(record.result.status, ExperimentRunStatus::SandboxMismatch);
        assert_eq!(
            record.result.failure_reason,
            Some(crate::ExperimentFailureReason::MissingExecutionProfile)
        );
    }

    #[test]
    fn runner_marks_underbudget_run_as_timed_out() {
        let mut invocation = sample_serving_invocation();
        invocation.spec.budget.max_wall_time_ms = 250;
        let record = ResearchRunner::execute_local(&invocation).expect("runner should succeed");
        assert_eq!(record.result.status, ExperimentRunStatus::TimedOut);
        assert_eq!(
            record.result.failure_reason,
            Some(crate::ExperimentFailureReason::BudgetTooSmall)
        );
    }

    #[test]
    fn persist_writes_result_and_logs() {
        let invocation = sample_serving_invocation();
        let record = ResearchRunner::execute_local(&invocation).expect("runner should succeed");
        let tempdir = tempdir().expect("tempdir should exist");
        let artifacts = ResearchRunner::persist(&record, &tempdir.path().join("result.json"))
            .expect("persist should succeed");
        assert!(artifacts.result_path.exists());
        assert!(artifacts.stdout_path.exists());
        assert!(artifacts.stderr_path.exists());
    }

    #[test]
    fn runner_executes_executor_experiment_end_to_end() {
        let invocation = sample_executor_invocation(
            "candidate-hull",
            "run-hull",
            TassadarExecutorDecodeCacheKind::HullCache,
            TassadarExecutorWeightConstruction::HandcraftedInterpreter,
        );
        let record = ResearchRunner::execute_local(&invocation).expect("runner should succeed");
        assert_eq!(record.result.status, ExperimentRunStatus::Succeeded);
        assert_eq!(record.result.family, ExperimentFamilyKind::ExecutorVariants);
        assert!(
            record
                .result
                .receipt_refs
                .iter()
                .any(|receipt| receipt.kind == crate::ExperimentReceiptKind::EvalRun)
        );
        assert!(
            record
                .result
                .artifact_outputs
                .iter()
                .any(|artifact| artifact.kind == ExperimentArtifactKind::BenchmarkReport)
        );
        assert!(
            record
                .result
                .artifact_outputs
                .iter()
                .any(|artifact| artifact.kind == ExperimentArtifactKind::ExecutionProofBundle)
        );
        assert!(
            record
                .result
                .artifact_outputs
                .iter()
                .any(|artifact| artifact.kind == ExperimentArtifactKind::RuntimeManifest)
        );
    }

    #[test]
    fn execute_local_sweep_ranks_executor_candidates_reproducibly() {
        let hull = sample_executor_invocation(
            "candidate-hull",
            "run-hull",
            TassadarExecutorDecodeCacheKind::HullCache,
            TassadarExecutorWeightConstruction::HandcraftedInterpreter,
        );
        let reference = sample_executor_invocation(
            "candidate-reference",
            "run-reference",
            TassadarExecutorDecodeCacheKind::ReferenceLinear,
            TassadarExecutorWeightConstruction::HandcraftedInterpreter,
        );
        let (_records, sweep) =
            ResearchRunner::execute_local_sweep("sweep.tassadar.1", &[hull, reference])
                .expect("sweep should succeed");
        assert_eq!(sweep.family, ExperimentFamilyKind::ExecutorVariants);
        assert_eq!(sweep.entries.len(), 2);
        assert_eq!(
            sweep.winning_candidate_id.as_deref(),
            Some("candidate-hull")
        );
        assert!(!sweep.sweep_digest.is_empty());
    }

    #[test]
    fn runner_emits_compiled_weight_artifact_for_program_compiled_candidate() {
        let invocation = sample_executor_invocation(
            "candidate-compiled",
            "run-compiled",
            TassadarExecutorDecodeCacheKind::HullCache,
            TassadarExecutorWeightConstruction::ProgramCompiled,
        );
        let record = ResearchRunner::execute_local(&invocation).expect("runner should succeed");
        assert!(
            record
                .result
                .artifact_outputs
                .iter()
                .any(|artifact| artifact.kind == ExperimentArtifactKind::CompiledWeightArtifact)
        );
        assert!(
            record
                .result
                .metrics
                .iter()
                .any(|metric| metric.metric_id == "executor_compiled_weight_artifact_bytes"
                    && metric.value_micros > 0)
        );
    }

    #[test]
    fn runner_rejects_non_two_dimensional_executor_candidates() {
        let mut invocation = sample_executor_invocation(
            "candidate-bad-heads",
            "run-bad-heads",
            TassadarExecutorDecodeCacheKind::HullCache,
            TassadarExecutorWeightConstruction::HandcraftedInterpreter,
        );
        let ExperimentFamily::ExecutorVariants { executor } = &mut invocation.spec.family else {
            panic!("executor variants");
        };
        executor.architecture.head_dim = 4;
        let error = ResearchRunner::execute_local(&invocation).expect_err("runner should refuse");
        assert!(matches!(error, ResearchRunnerError::InvalidInvocation(_)));
    }
}
