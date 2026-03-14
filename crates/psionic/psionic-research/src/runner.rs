use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ExperimentArtifactKind, ExperimentArtifactOutput, ExperimentBudget, ExperimentFailureReason,
    ExperimentFamily, ExperimentMetric, ExperimentReceiptKind, ExperimentReceiptRef,
    ExperimentResult, ExperimentRunStatus, ExperimentScore, ExperimentSpec,
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
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
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
    use tempfile::tempdir;

    use crate::{
        CandidateMutation, ExperimentArtifactKind, ExperimentArtifactRef, ExperimentBudget,
        ExperimentFamily, ExperimentFamilyKind, ExperimentRunStatus, ExperimentScoreContract,
        ExperimentThreshold, ResearchRunner, ResearchRunnerInvocation, ScoreDirection,
        ScoreMetricSpec, ServingSchedulerPolicy,
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
}
