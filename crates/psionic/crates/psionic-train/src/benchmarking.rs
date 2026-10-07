use std::collections::BTreeMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use psionic_cluster::{
    AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace,
    ClusterNodeIdentity, ClusterSnapshot, ClusterState, NodeEpoch, NodeId, NodeRole,
};
use psionic_collectives::{
    CollectiveMeshMember, CollectiveSyncCadenceClass, CollectiveSyncCadencePolicy,
    CollectiveTransportFeedback, ElasticCollectivePlanner, QuantizedCollectiveBenchmark,
    QuantizedCollectiveBenchmarkSample,
};
use psionic_core::{DType, Device, Shape, TensorSpec};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamOpenRequest,
    DatastreamPolicyWeightBinding, DatastreamPolicyWeightBroadcastManifest, DatastreamSubjectKind,
    DatastreamTransferError, InMemoryDatastreamClient, InMemoryDatastreamServer,
    InMemoryPolicyWeightBroadcast,
};
use psionic_environments::EnvironmentPackageKey;
use psionic_runtime::{
    ClusterCommunicationClass, TrainingCollectiveKind, TrainingCollectiveQuantization,
    TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind, TrainingElasticMembershipContext,
};
use psionic_sandbox::{
    InMemorySandboxPoolService, ProviderSandboxEntrypointType, ProviderSandboxExecutionClass,
    ProviderSandboxExecutionControls, ProviderSandboxProfile, ProviderSandboxResourceRequest,
    ProviderSandboxRuntimeKind, SandboxLoopIterationRequest, SandboxPoolError, SandboxPoolSpec,
    SandboxStageArtifactKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CheckpointPointer, CheckpointReadSourceKind, CheckpointScopeBinding, CheckpointScopeKind,
    CheckpointStoreReadOptions, FixedBudgetTrainingRun, InMemoryCheckpointStore, PolicyRevision,
    RolloutAdmissionReceipt, RolloutArtifact, RolloutBenchmarkExpectation,
    RolloutBenchmarkObservation, RolloutContractError, RolloutProofKind, RolloutProofReference,
    RolloutReceiptOutcome, RolloutSample, RolloutTerminationReason, RolloutValidatorPolicy,
    RolloutValidatorState, RolloutVerificationBundle, RolloutWorkerOutcomeKind,
    RolloutWorkerOutcomeReceipt, RolloutWorkerPolicyPosture, RolloutWorkerTrustClass,
    TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget, TrainingOffPolicyBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingOrchestratorError,
    TrainingOrchestratorState, TrainingParameterClass, TrainingParameterGroupState,
    TrainingRecoveryMode, TrainingRunGraphError, TrainingRunState, TrainingSessionError,
    TrainingTensorBuffer, TrainingWindowAssignmentRule, ValidatorDisposition,
};

/// Error returned by the train benchmark and acceptance suite.
#[derive(Debug, Error)]
pub enum TrainBenchmarkError {
    /// Training-session state was invalid.
    #[error(transparent)]
    TrainingSession(#[from] TrainingSessionError),
    /// Checkpoint restore planning failed.
    #[error(transparent)]
    CheckpointRecovery(#[from] crate::CheckpointRecoveryError),
    /// Fixed-budget training core failed.
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    /// Run-graph state was invalid.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// Orchestrator state was invalid.
    #[error(transparent)]
    Orchestrator(#[from] TrainingOrchestratorError),
    /// Rollout artifacts or worker contracts were invalid.
    #[error(transparent)]
    RolloutContract(#[from] RolloutContractError),
    /// Datastream delivery or resume logic failed.
    #[error(transparent)]
    Datastream(#[from] DatastreamTransferError),
    /// Collective planning or benchmark gating failed.
    #[error(transparent)]
    CollectivePlanning(#[from] psionic_collectives::CollectivePlanningError),
    /// Sandbox pool or execution logic failed.
    #[error(transparent)]
    Sandbox(#[from] SandboxPoolError),
    /// Benchmark temp workspace or artifact handling failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Final verdict for one benchmark category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainBenchmarkDisposition {
    /// The benchmark met its acceptance thresholds.
    Passed,
    /// The benchmark fell below one or more thresholds.
    Failed,
}

/// Acceptance thresholds for fixed-budget trainer throughput.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerThroughputThresholds {
    /// Minimum whole steps per second.
    pub min_steps_per_second: u64,
    /// Minimum samples per second across all executed steps.
    pub min_samples_per_second: u64,
    /// Maximum acceptable mean step duration.
    pub max_mean_step_duration_ms: u64,
    /// Maximum acceptable tail step duration.
    pub max_tail_step_duration_ms: u64,
}

/// Acceptance thresholds for rollout ingestion throughput.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutIngestionThresholds {
    /// Minimum accepted ingestion throughput.
    pub min_rollouts_per_second: u64,
    /// Maximum mean spacing between submission timestamps.
    pub max_mean_submission_spacing_ms: u64,
    /// Minimum acceptance ratio in basis points.
    pub min_accepted_ratio_bps: u64,
}

/// Acceptance thresholds for warm sandbox reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxReuseThresholds {
    /// Maximum mean latency to warm one ready session.
    pub max_mean_warm_latency_ms: u64,
    /// Maximum mean latency to acquire a ready session.
    pub max_mean_acquisition_latency_ms: u64,
    /// Minimum workspace reuse ratio in basis points.
    pub min_reuse_ratio_bps: u64,
    /// Minimum number of ready sessions the pool must sustain.
    pub min_ready_sessions: u32,
}

/// Acceptance thresholds for checkpoint recovery and resumable delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecoveryThresholds {
    /// Maximum latency for a healthy pointer-backed restore plan.
    pub max_pointer_restore_latency_ms: u64,
    /// Maximum latency for a manifest-listing fallback restore plan.
    pub max_fallback_restore_latency_ms: u64,
    /// Maximum allowed attempts in the fallback ladder.
    pub max_fallback_attempts: usize,
    /// Minimum checkpoint bytes per second for resumable datastream recovery.
    pub min_delivery_bytes_per_second: u64,
}

/// Acceptance thresholds for validator verification cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorCostThresholds {
    /// Minimum verified bundles per second.
    pub min_bundles_per_second: u64,
    /// Maximum mean verification latency per bundle.
    pub max_mean_verification_latency_ms: u64,
    /// Maximum share of bundles that should run benchmark-class checks.
    pub max_benchmark_checked_share_bps: u64,
}

/// Acceptance thresholds for elastic scaling behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticScalingThresholds {
    /// Minimum relative throughput scaling from two to four members.
    pub min_two_to_four_member_scaling_bps: u64,
    /// Minimum benchmark speedup required for every quantized point.
    pub min_quantized_speedup_bps: u64,
    /// Maximum degraded global interval the planner may widen to.
    pub max_degraded_global_interval_steps: u64,
}

/// Canonical threshold profile for the train acceptance suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainBenchmarkThresholdProfile {
    /// Stable profile identifier.
    pub profile_id: String,
    /// Trainer thresholds.
    pub trainer: TrainerThroughputThresholds,
    /// Rollout-ingestion thresholds.
    pub rollout_ingestion: RolloutIngestionThresholds,
    /// Sandbox thresholds.
    pub sandbox_reuse: SandboxReuseThresholds,
    /// Checkpoint and datastream thresholds.
    pub checkpoint_recovery: CheckpointRecoveryThresholds,
    /// Validator thresholds.
    pub validator_cost: ValidatorCostThresholds,
    /// Elastic-scaling thresholds.
    pub elastic_scaling: ElasticScalingThresholds,
}

impl TrainBenchmarkThresholdProfile {
    /// Returns the canonical reference threshold profile for the train suite.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            profile_id: String::from("psionic-train-reference-acceptance"),
            trainer: TrainerThroughputThresholds {
                min_steps_per_second: 18,
                min_samples_per_second: 140,
                max_mean_step_duration_ms: 60,
                max_tail_step_duration_ms: 80,
            },
            rollout_ingestion: RolloutIngestionThresholds {
                min_rollouts_per_second: 20,
                max_mean_submission_spacing_ms: 40,
                min_accepted_ratio_bps: 9_000,
            },
            sandbox_reuse: SandboxReuseThresholds {
                max_mean_warm_latency_ms: 1_500,
                max_mean_acquisition_latency_ms: 500,
                min_reuse_ratio_bps: 7_000,
                min_ready_sessions: 2,
            },
            checkpoint_recovery: CheckpointRecoveryThresholds {
                max_pointer_restore_latency_ms: 50,
                max_fallback_restore_latency_ms: 50,
                max_fallback_attempts: 2,
                min_delivery_bytes_per_second: 4_096,
            },
            validator_cost: ValidatorCostThresholds {
                min_bundles_per_second: 200,
                max_mean_verification_latency_ms: 25,
                max_benchmark_checked_share_bps: 5_000,
            },
            elastic_scaling: ElasticScalingThresholds {
                min_two_to_four_member_scaling_bps: 15_000,
                min_quantized_speedup_bps: 2_500,
                max_degraded_global_interval_steps: 4,
            },
        }
    }
}

impl Default for TrainBenchmarkThresholdProfile {
    fn default() -> Self {
        Self::reference()
    }
}

/// Receipt for the fixed-budget trainer throughput benchmark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerThroughputBenchmarkReceipt {
    /// Total executed steps.
    pub step_count: u64,
    /// Total samples processed across all steps.
    pub total_samples: u64,
    /// Summed step duration.
    pub total_duration_ms: u64,
    /// Mean step duration.
    pub mean_step_duration_ms: u64,
    /// Maximum observed step duration.
    pub tail_step_duration_ms: u64,
    /// Realized whole steps per second.
    pub steps_per_second: u64,
    /// Realized whole samples per second.
    pub samples_per_second: u64,
    /// Final benchmark disposition.
    pub disposition: TrainBenchmarkDisposition,
}

/// Receipt for rollout ingestion throughput.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutIngestionBenchmarkReceipt {
    /// Total submitted rollouts.
    pub rollout_count: u64,
    /// Accepted exact rollouts.
    pub accepted_exact_count: u64,
    /// Accepted off-policy rollouts.
    pub accepted_off_policy_count: u64,
    /// Quarantined rollouts.
    pub quarantined_count: u64,
    /// Discarded rollouts.
    pub discarded_count: u64,
    /// Total observed submission window.
    pub observed_window_ms: u64,
    /// Mean spacing between submissions.
    pub mean_submission_spacing_ms: u64,
    /// Realized whole rollouts per second.
    pub rollouts_per_second: u64,
    /// Accepted ratio in basis points.
    pub accepted_ratio_bps: u64,
    /// Final benchmark disposition.
    pub disposition: TrainBenchmarkDisposition,
}

/// Receipt for warm-pool sandbox reuse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxReuseBenchmarkReceipt {
    /// Number of warm receipts emitted while reaching target readiness.
    pub warm_receipt_count: usize,
    /// Number of acquisitions benchmarked.
    pub acquisition_count: usize,
    /// Number of executed iterations benchmarked.
    pub iteration_count: usize,
    /// Mean warm latency.
    pub mean_warm_latency_ms: u64,
    /// Mean acquisition latency.
    pub mean_acquisition_latency_ms: u64,
    /// Final ready session count.
    pub ready_sessions: u32,
    /// Number of iterations that reused prior workspace state.
    pub reused_iterations: u32,
    /// Workspace reuse ratio in basis points.
    pub reuse_ratio_bps: u64,
    /// Final benchmark disposition.
    pub disposition: TrainBenchmarkDisposition,
}

/// Receipt for checkpoint restore latency and resumable datastream delivery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecoveryBenchmarkReceipt {
    /// Source kind selected when a healthy pointer existed.
    pub pointer_source_kind: CheckpointReadSourceKind,
    /// Source kind selected when the pointer was stale.
    pub fallback_source_kind: CheckpointReadSourceKind,
    /// Pointer-backed restore latency.
    pub pointer_restore_latency_ms: u64,
    /// Fallback restore latency.
    pub fallback_restore_latency_ms: u64,
    /// Attempt count in the fallback restore ladder.
    pub fallback_attempt_count: usize,
    /// Delivered checkpoint bytes.
    pub delivered_bytes: u64,
    /// Delivered checkpoint chunks.
    pub delivered_chunks: usize,
    /// Whether the datastream delivery exercised resume.
    pub resumed_delivery: bool,
    /// Realized whole bytes per second for checkpoint delivery.
    pub delivery_bytes_per_second: u64,
    /// Final benchmark disposition.
    pub disposition: TrainBenchmarkDisposition,
}

/// Receipt for validator verification cost and sampled benchmark posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorCostBenchmarkReceipt {
    /// Total validated bundles.
    pub bundle_count: u64,
    /// Accepted verdict count.
    pub accepted_count: u64,
    /// Normalized verdict count.
    pub normalized_count: u64,
    /// Rejected verdict count.
    pub rejected_count: u64,
    /// Bundle count that ran benchmark checks.
    pub benchmark_checked_count: u64,
    /// Total wall time spent validating the bundle set.
    pub total_verification_latency_ms: u64,
    /// Mean latency per validated bundle.
    pub mean_verification_latency_ms: u64,
    /// Realized whole bundles per second.
    pub bundles_per_second: u64,
    /// Share of bundles that ran benchmark checks.
    pub benchmark_checked_share_bps: u64,
    /// Final benchmark disposition.
    pub disposition: TrainBenchmarkDisposition,
}

/// One point on the elastic scaling curve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticScalingCurvePoint {
    /// Stable point identifier.
    pub point_id: String,
    /// Worker count represented by the point.
    pub member_count: usize,
    /// Mesh revision carried by the planner.
    pub mesh_revision: u64,
    /// Effective quantized collective throughput for the point.
    pub effective_bytes_per_second: u64,
    /// Quantized speedup versus the baseline, in basis points.
    pub quantized_speedup_bps: u64,
    /// Selected cadence class.
    pub cadence_class: CollectiveSyncCadenceClass,
    /// Realized global sync interval.
    pub global_interval_steps: u64,
    /// Whether transport was degraded.
    pub degraded_transport: bool,
    /// Local subgroup size used by the planner, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_group_size: Option<usize>,
}

/// Receipt for the elastic scaling benchmark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElasticScalingBenchmarkReceipt {
    /// Healthy and degraded curve points.
    pub points: Vec<ElasticScalingCurvePoint>,
    /// Relative throughput scaling from two to four members.
    pub two_to_four_member_scaling_bps: u64,
    /// Degraded global interval selected by the planner.
    pub degraded_global_interval_steps: u64,
    /// Final benchmark disposition.
    pub disposition: TrainBenchmarkDisposition,
}

/// Receipt for the full train acceptance suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainBenchmarkSuiteReceipt {
    /// Threshold profile used by the suite.
    pub profile: TrainBenchmarkThresholdProfile,
    /// Fixed-budget trainer benchmark receipt.
    pub trainer: TrainerThroughputBenchmarkReceipt,
    /// Rollout-ingestion benchmark receipt.
    pub rollout_ingestion: RolloutIngestionBenchmarkReceipt,
    /// Sandbox warm-reuse benchmark receipt.
    pub sandbox_reuse: SandboxReuseBenchmarkReceipt,
    /// Checkpoint restore and datastream benchmark receipt.
    pub checkpoint_recovery: CheckpointRecoveryBenchmarkReceipt,
    /// Validator verification benchmark receipt.
    pub validator_cost: ValidatorCostBenchmarkReceipt,
    /// Elastic scaling benchmark receipt.
    pub elastic_scaling: ElasticScalingBenchmarkReceipt,
    /// Number of passing benchmark categories.
    pub passed_count: usize,
    /// Number of failing benchmark categories.
    pub failed_count: usize,
    /// Stable digest over the suite receipt.
    pub receipt_digest: String,
}

/// Train-owned benchmark and acceptance harness.
pub struct TrainBenchmarkHarness;

impl TrainBenchmarkHarness {
    /// Runs the canonical reference threshold suite.
    pub fn run_reference_suite() -> Result<TrainBenchmarkSuiteReceipt, TrainBenchmarkError> {
        Self::run_suite_with_profile(&TrainBenchmarkThresholdProfile::reference())
    }

    /// Runs the suite with an explicit threshold profile.
    pub fn run_suite_with_profile(
        profile: &TrainBenchmarkThresholdProfile,
    ) -> Result<TrainBenchmarkSuiteReceipt, TrainBenchmarkError> {
        let benchmark_root = benchmark_root()?;
        fs::create_dir_all(benchmark_root.as_path())?;

        let trainer = trainer_benchmark(&profile.trainer)?;
        let rollout_ingestion = rollout_ingestion_benchmark(&profile.rollout_ingestion)?;
        let sandbox_reuse =
            sandbox_reuse_benchmark(&profile.sandbox_reuse, benchmark_root.join("sandbox"))?;
        let checkpoint_recovery = checkpoint_recovery_benchmark(&profile.checkpoint_recovery)?;
        let validator_cost = validator_cost_benchmark(&profile.validator_cost)?;
        let elastic_scaling = elastic_scaling_benchmark(&profile.elastic_scaling)?;

        let dispositions = [
            trainer.disposition,
            rollout_ingestion.disposition,
            sandbox_reuse.disposition,
            checkpoint_recovery.disposition,
            validator_cost.disposition,
            elastic_scaling.disposition,
        ];
        let passed_count = dispositions
            .iter()
            .filter(|disposition| **disposition == TrainBenchmarkDisposition::Passed)
            .count();
        let failed_count = dispositions.len().saturating_sub(passed_count);
        let receipt_digest = stable_suite_digest(
            profile,
            &trainer,
            &rollout_ingestion,
            &sandbox_reuse,
            &checkpoint_recovery,
            &validator_cost,
            &elastic_scaling,
            passed_count,
            failed_count,
        );

        let receipt = TrainBenchmarkSuiteReceipt {
            profile: profile.clone(),
            trainer,
            rollout_ingestion,
            sandbox_reuse,
            checkpoint_recovery,
            validator_cost,
            elastic_scaling,
            passed_count,
            failed_count,
            receipt_digest,
        };

        let _ = fs::remove_dir_all(benchmark_root);
        Ok(receipt)
    }
}

fn trainer_benchmark(
    thresholds: &TrainerThroughputThresholds,
) -> Result<TrainerThroughputBenchmarkReceipt, TrainBenchmarkError> {
    let budget = TrainingLoopBudget::new(4, 2, 2)?;
    let mut run = FixedBudgetTrainingRun::new(
        "train-benchmark-trainer",
        "train.benchmark",
        budget,
        training_parameter_groups()?,
    )?;
    let outcome = run.run_fixed_budget([
        trainer_step_input(
            "batch-1",
            [0.03, 0.01, 0.02, 0.01],
            [0.01, 0.00],
            1_000,
            1_050,
        )?,
        trainer_step_input(
            "batch-2",
            [0.02, 0.02, 0.01, 0.01],
            [0.00, 0.01],
            1_050,
            1_100,
        )?,
        trainer_step_input(
            "batch-3",
            [0.01, 0.01, 0.02, 0.02],
            [0.01, 0.01],
            1_100,
            1_150,
        )?,
        trainer_step_input(
            "batch-4",
            [0.02, 0.01, 0.01, 0.02],
            [0.00, 0.01],
            1_150,
            1_200,
        )?,
    ])?;
    let step_count = outcome.receipts.len() as u64;
    let total_samples = outcome
        .receipts
        .iter()
        .map(|receipt| u64::from(receipt.sample_count))
        .sum::<u64>();
    let total_duration_ms = outcome
        .receipts
        .iter()
        .map(|receipt| receipt.timing.duration_ms)
        .sum::<u64>();
    let mean_step_duration_ms = mean_u64(
        outcome
            .receipts
            .iter()
            .map(|receipt| receipt.timing.duration_ms)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let tail_step_duration_ms = outcome
        .receipts
        .iter()
        .map(|receipt| receipt.timing.duration_ms)
        .max()
        .unwrap_or_default();
    let steps_per_second = units_per_second(step_count, total_duration_ms);
    let samples_per_second = units_per_second(total_samples, total_duration_ms);
    let disposition = if steps_per_second >= thresholds.min_steps_per_second
        && samples_per_second >= thresholds.min_samples_per_second
        && mean_step_duration_ms <= thresholds.max_mean_step_duration_ms
        && tail_step_duration_ms <= thresholds.max_tail_step_duration_ms
    {
        TrainBenchmarkDisposition::Passed
    } else {
        TrainBenchmarkDisposition::Failed
    };
    Ok(TrainerThroughputBenchmarkReceipt {
        step_count,
        total_samples,
        total_duration_ms,
        mean_step_duration_ms,
        tail_step_duration_ms,
        steps_per_second,
        samples_per_second,
        disposition,
    })
}

fn rollout_ingestion_benchmark(
    thresholds: &RolloutIngestionThresholds,
) -> Result<RolloutIngestionBenchmarkReceipt, TrainBenchmarkError> {
    let mut orchestrator = benchmark_orchestrator_state(TrainingOffPolicyBudget::default())?;
    let window = orchestrator.plan_next_window(
        2,
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count: 2,
            eval_slice_count: 1,
        },
        19,
        1_100,
    )?;
    orchestrator.activate_current_window(1_110)?;
    let target_policy = orchestrator.target_policy_revision.clone();
    let assigned_workers = unique_assigned_workers(&window);

    let mut accepted_exact_count = 0_u64;
    let mut accepted_off_policy_count = 0_u64;
    let mut quarantined_count = 0_u64;
    let mut discarded_count = 0_u64;
    let mut first_submission_ms = None;
    let mut last_submission_ms = None;

    for index in 0..8_u64 {
        let observed_at_ms = 1_120 + (index * 25);
        let worker_id = assigned_workers[index as usize % assigned_workers.len()].clone();
        let receipt = orchestrator.submit_rollout(
            rollout_artifact(
                worker_id.as_str(),
                format!("ingest-{index}").as_str(),
                target_policy.clone(),
                observed_at_ms.saturating_sub(5),
            )?,
            observed_at_ms,
        )?;
        first_submission_ms.get_or_insert(observed_at_ms);
        last_submission_ms = Some(observed_at_ms);
        match receipt.outcome {
            RolloutReceiptOutcome::AcceptedExact => {
                accepted_exact_count = accepted_exact_count.saturating_add(1);
            }
            RolloutReceiptOutcome::AcceptedOffPolicy => {
                accepted_off_policy_count = accepted_off_policy_count.saturating_add(1);
            }
            RolloutReceiptOutcome::Quarantined => {
                quarantined_count = quarantined_count.saturating_add(1);
            }
            RolloutReceiptOutcome::Discarded => {
                discarded_count = discarded_count.saturating_add(1);
            }
        }
    }

    let rollout_count =
        accepted_exact_count + accepted_off_policy_count + quarantined_count + discarded_count;
    let observed_window_ms = last_submission_ms
        .unwrap_or_default()
        .saturating_sub(first_submission_ms.unwrap_or_default())
        .max(1);
    let mean_submission_spacing_ms = if rollout_count > 1 {
        observed_window_ms / rollout_count.saturating_sub(1)
    } else {
        observed_window_ms
    };
    let rollouts_per_second = units_per_second(rollout_count, observed_window_ms);
    let accepted_ratio_bps = ratio_bps(
        accepted_exact_count.saturating_add(accepted_off_policy_count),
        rollout_count,
    );
    let disposition = if rollouts_per_second >= thresholds.min_rollouts_per_second
        && mean_submission_spacing_ms <= thresholds.max_mean_submission_spacing_ms
        && accepted_ratio_bps >= thresholds.min_accepted_ratio_bps
    {
        TrainBenchmarkDisposition::Passed
    } else {
        TrainBenchmarkDisposition::Failed
    };

    Ok(RolloutIngestionBenchmarkReceipt {
        rollout_count,
        accepted_exact_count,
        accepted_off_policy_count,
        quarantined_count,
        discarded_count,
        observed_window_ms,
        mean_submission_spacing_ms,
        rollouts_per_second,
        accepted_ratio_bps,
        disposition,
    })
}

fn sandbox_reuse_benchmark(
    thresholds: &SandboxReuseThresholds,
    workspace_root: PathBuf,
) -> Result<SandboxReuseBenchmarkReceipt, TrainBenchmarkError> {
    fs::create_dir_all(workspace_root.as_path())?;
    let mut service = InMemorySandboxPoolService::default();
    service.create_pool(
        SandboxPoolSpec {
            pool_id: String::from("train-benchmark-pool"),
            workspace_root,
            target_ready: 2,
            max_sessions: 2,
        },
        benchmark_sandbox_profile(),
    )?;
    let warm_receipts = service.warm_pool("train-benchmark-pool")?;
    let mut acquisition_receipts = Vec::new();
    let mut reused_iterations = 0_u32;

    for (index, city) in ["Austin", "Chicago", "Dallas", "Boston"].iter().enumerate() {
        let acquisition = service.acquire_session("train-benchmark-pool")?;
        service.stage_artifact(
            acquisition.pool_id.as_str(),
            acquisition.session_id.as_str(),
            acquisition.acquisition_id.as_str(),
            SandboxStageArtifactKind::CommandInput,
            "inputs/request.json",
            format!("{{\"city\":\"{city}\"}}").as_bytes(),
        )?;
        let mut request = SandboxLoopIterationRequest::new(
            ProviderSandboxEntrypointType::InlinePayload,
            "train-benchmark-inline",
        );
        request.payload = Some(String::from(
            "#!/bin/sh\nset -eu\nmkdir -p state outputs\ncount=0\nif [ -f state/counter.txt ]; then count=$(/bin/cat state/counter.txt); fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > state/counter.txt\nprintf '{\"run\":%s,\"city\":\"%s\"}' \"$count\" \"benchmark\" > outputs/result.json\n",
        ));
        request.expected_outputs = vec![String::from("outputs/result.json")];
        request.timeout_request_s = 5;
        request.network_request = String::from("host_inherit");
        request.filesystem_request = String::from("host_inherit");
        request.resource_request = ProviderSandboxResourceRequest::default();
        request.payout_reference = Some(format!("train-benchmark-payout-{index}"));
        request.verification_posture = Some(String::from("train_benchmark"));
        let iteration = service.run_iteration(
            acquisition.pool_id.as_str(),
            acquisition.session_id.as_str(),
            acquisition.acquisition_id.as_str(),
            request,
            ProviderSandboxExecutionControls::default(),
        )?;
        if iteration.reused_workspace {
            reused_iterations = reused_iterations.saturating_add(1);
        }
        acquisition_receipts.push(acquisition);
    }

    let final_snapshot = service.snapshot("train-benchmark-pool")?;
    let mean_warm_latency_ms = mean_u64(
        warm_receipts
            .iter()
            .map(|receipt| receipt.warm_latency_ms)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let mean_acquisition_latency_ms = mean_u64(
        acquisition_receipts
            .iter()
            .map(|receipt| receipt.acquisition_latency_ms)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let reuse_ratio_bps = ratio_bps(reused_iterations as u64, acquisition_receipts.len() as u64);
    let disposition = if mean_warm_latency_ms <= thresholds.max_mean_warm_latency_ms
        && mean_acquisition_latency_ms <= thresholds.max_mean_acquisition_latency_ms
        && reuse_ratio_bps >= thresholds.min_reuse_ratio_bps
        && final_snapshot.ready_sessions >= thresholds.min_ready_sessions
    {
        TrainBenchmarkDisposition::Passed
    } else {
        TrainBenchmarkDisposition::Failed
    };

    Ok(SandboxReuseBenchmarkReceipt {
        warm_receipt_count: warm_receipts.len(),
        acquisition_count: acquisition_receipts.len(),
        iteration_count: acquisition_receipts.len(),
        mean_warm_latency_ms,
        mean_acquisition_latency_ms,
        ready_sessions: final_snapshot.ready_sessions,
        reused_iterations,
        reuse_ratio_bps,
        disposition,
    })
}

fn checkpoint_recovery_benchmark(
    thresholds: &CheckpointRecoveryThresholds,
) -> Result<CheckpointRecoveryBenchmarkReceipt, TrainBenchmarkError> {
    let state = cluster_state(&[
        ("worker-a", ClusterMembershipStatus::Ready),
        ("worker-b", ClusterMembershipStatus::Ready),
    ]);
    let checkpoint_bytes = vec![7_u8; 16 * 1024];
    let manifest = DatastreamManifest::from_bytes(
        "train-benchmark-checkpoint",
        DatastreamSubjectKind::Checkpoint,
        checkpoint_bytes.as_slice(),
        1_024,
        DatastreamEncoding::Safetensors,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new("train.benchmark")
            .with_checkpoint_ref("step-12")
            .with_step(12),
    );
    let mut session =
        crate::TrainingSessionState::new(state.cluster_id().as_str(), "train.benchmark");
    let write =
        session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;
    session.mark_checkpoint_durable(write.write_id.as_str(), 1_200)?;

    let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, "train-benchmark-run");
    let durable_manifest = session.checkpoint_manifest_for_latest_durable(
        scope.clone(),
        &NodeId::new("worker-a"),
        1_220,
    )?;
    let pointer =
        session.checkpoint_pointer_for_latest_durable(scope.clone(), &durable_manifest, 1_225)?;

    let mut pointer_store = InMemoryCheckpointStore::default();
    pointer_store.store_manifest(durable_manifest.clone());
    pointer_store.store_pointer(pointer);
    let pointer_started = Instant::now();
    let pointer_restore = pointer_store.plan_restore(
        &scope,
        "train.benchmark",
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        &[NodeId::new("worker-a")],
        CheckpointStoreReadOptions::default(),
    )?;
    let pointer_restore_latency_ms = elapsed_nonzero_ms(pointer_started);

    let stale_pointer = CheckpointPointer::new(
        scope.clone(),
        "train.benchmark",
        durable_manifest.checkpoint.clone(),
        "missing-manifest-digest",
        1_230,
    )?;
    let mut fallback_store = InMemoryCheckpointStore::default();
    fallback_store.store_manifest(durable_manifest.clone());
    fallback_store.store_pointer(stale_pointer);
    let fallback_started = Instant::now();
    let fallback_restore = fallback_store.plan_restore(
        &scope,
        "train.benchmark",
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        &[NodeId::new("worker-a")],
        CheckpointStoreReadOptions::default(),
    )?;
    let fallback_restore_latency_ms = elapsed_nonzero_ms(fallback_started);

    let stream_server = InMemoryDatastreamServer::new(manifest.clone(), checkpoint_bytes)?;
    let delivery_started = Instant::now();
    let mut client = InMemoryDatastreamClient::new(stream_server.manifest().clone());
    let mut first_session = stream_server.open(
        DatastreamOpenRequest::new(stream_server.manifest().stable_digest())
            .with_max_chunks_in_flight(2),
    )?;
    for chunk in first_session.next_window()? {
        client.apply_chunk(chunk)?;
    }
    let progress = client.progress()?;
    let mut resumed = InMemoryDatastreamClient::resume(
        stream_server.manifest().clone(),
        client.received_bytes().to_vec(),
        progress.next_chunk_index,
    )?;
    let mut resumed_session = stream_server.open(
        DatastreamOpenRequest::new(stream_server.manifest().stable_digest())
            .with_resume_cursor(resumed.progress()?.cursor())
            .with_max_chunks_in_flight(3),
    )?;
    loop {
        let window = resumed_session.next_window()?;
        if window.is_empty() {
            break;
        }
        for chunk in window {
            resumed.apply_chunk(chunk)?;
        }
    }
    let delivery = resumed.finish()?;
    let delivery_bytes_per_second = units_per_second(
        delivery.bytes_delivered,
        elapsed_nonzero_ms(delivery_started),
    );

    let disposition = if pointer_restore.source_kind == CheckpointReadSourceKind::PointerLookup
        && fallback_restore.source_kind == CheckpointReadSourceKind::ManifestListingFallback
        && pointer_restore_latency_ms <= thresholds.max_pointer_restore_latency_ms
        && fallback_restore_latency_ms <= thresholds.max_fallback_restore_latency_ms
        && fallback_restore.attempts.len() <= thresholds.max_fallback_attempts
        && delivery.resumed
        && delivery_bytes_per_second >= thresholds.min_delivery_bytes_per_second
    {
        TrainBenchmarkDisposition::Passed
    } else {
        TrainBenchmarkDisposition::Failed
    };

    Ok(CheckpointRecoveryBenchmarkReceipt {
        pointer_source_kind: pointer_restore.source_kind,
        fallback_source_kind: fallback_restore.source_kind,
        pointer_restore_latency_ms,
        fallback_restore_latency_ms,
        fallback_attempt_count: fallback_restore.attempts.len(),
        delivered_bytes: delivery.bytes_delivered,
        delivered_chunks: delivery.chunks_delivered,
        resumed_delivery: delivery.resumed,
        delivery_bytes_per_second,
        disposition,
    })
}

fn validator_cost_benchmark(
    thresholds: &ValidatorCostThresholds,
) -> Result<ValidatorCostBenchmarkReceipt, TrainBenchmarkError> {
    let mut validator = RolloutValidatorState::new(RolloutValidatorPolicy::default());
    let exact_policy =
        PolicyRevision::new("train.benchmark", "policy-rev-7", "policy-digest-7", 1_100)
            .with_revision_number(7);
    let stale_policy =
        PolicyRevision::new("train.benchmark", "policy-rev-1", "policy-digest-1", 900)
            .with_revision_number(1);
    let bundles = vec![
        validator_bundle(
            "bundle-a-accepted",
            "worker-a",
            "artifact-a",
            "task-a",
            exact_policy.clone(),
            true,
            false,
        )?,
        validator_bundle(
            "bundle-b-accepted",
            "worker-b",
            "artifact-b",
            "task-b",
            exact_policy.clone(),
            true,
            false,
        )?,
        validator_bundle(
            "bundle-c-accepted",
            "worker-c",
            "artifact-c",
            "task-shared-c",
            exact_policy.clone(),
            true,
            false,
        )?,
        validator_bundle(
            "bundle-c-duplicate",
            "worker-d",
            "artifact-d",
            "task-shared-c",
            exact_policy.clone(),
            true,
            false,
        )?,
        validator_bundle(
            "bundle-e-accepted",
            "worker-e",
            "artifact-e",
            "task-shared-e",
            exact_policy.clone(),
            false,
            false,
        )?,
        validator_bundle(
            "bundle-e-duplicate",
            "worker-f",
            "artifact-f",
            "task-shared-e",
            exact_policy.clone(),
            false,
            false,
        )?,
        validator_bundle(
            "bundle-g-accepted",
            "worker-g",
            "artifact-g",
            "task-g",
            exact_policy.clone(),
            false,
            false,
        )?,
        validator_bundle(
            "bundle-stale-1",
            "worker-h",
            "artifact-h",
            "task-stale-h",
            stale_policy.clone(),
            false,
            true,
        )?,
        validator_bundle(
            "bundle-stale-2",
            "worker-i",
            "artifact-i",
            "task-stale-i",
            stale_policy.clone(),
            false,
            true,
        )?,
        validator_bundle(
            "bundle-stale-3",
            "worker-j",
            "artifact-j",
            "task-stale-j",
            stale_policy,
            false,
            true,
        )?,
    ];

    let started = Instant::now();
    for bundle in bundles {
        validator.verify_bundle(bundle);
    }
    let total_verification_latency_ms = elapsed_nonzero_ms(started);
    let bundle_count = validator.verdicts.len() as u64;
    let accepted_count = validator
        .verdicts
        .iter()
        .filter(|verdict| verdict.disposition == ValidatorDisposition::Accepted)
        .count() as u64;
    let normalized_count = validator
        .verdicts
        .iter()
        .filter(|verdict| verdict.disposition == ValidatorDisposition::Normalized)
        .count() as u64;
    let rejected_count = validator
        .verdicts
        .iter()
        .filter(|verdict| verdict.disposition == ValidatorDisposition::Rejected)
        .count() as u64;
    let benchmark_checked_count = validator
        .verdicts
        .iter()
        .filter(|verdict| verdict.ran_benchmark_checks)
        .count() as u64;
    let mean_verification_latency_ms = total_verification_latency_ms / bundle_count.max(1);
    let bundles_per_second = units_per_second(bundle_count, total_verification_latency_ms);
    let benchmark_checked_share_bps = ratio_bps(benchmark_checked_count, bundle_count);
    let mixed_outcomes = accepted_count > 0 && normalized_count > 0 && rejected_count > 0;
    let disposition = if mixed_outcomes
        && bundles_per_second >= thresholds.min_bundles_per_second
        && mean_verification_latency_ms <= thresholds.max_mean_verification_latency_ms
        && benchmark_checked_share_bps <= thresholds.max_benchmark_checked_share_bps
    {
        TrainBenchmarkDisposition::Passed
    } else {
        TrainBenchmarkDisposition::Failed
    };

    Ok(ValidatorCostBenchmarkReceipt {
        bundle_count,
        accepted_count,
        normalized_count,
        rejected_count,
        benchmark_checked_count,
        total_verification_latency_ms,
        mean_verification_latency_ms,
        bundles_per_second,
        benchmark_checked_share_bps,
        disposition,
    })
}

fn elastic_scaling_benchmark(
    thresholds: &ElasticScalingThresholds,
) -> Result<ElasticScalingBenchmarkReceipt, TrainBenchmarkError> {
    let healthy_policy = CollectiveSyncCadencePolicy::new()
        .with_degraded_global_interval_steps(4)
        .with_transport_thresholds(800, 8, 8);
    let two_member_point = scaling_point(
        "two-member-healthy",
        1,
        vec!["worker-a", "worker-b"],
        vec![
            TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                .with_collective_group_size(2),
        ],
        QuantizedCollectiveBenchmark::new(
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            QuantizedCollectiveBenchmarkSample::new(2_400, 8 * 1024 * 1024, 0),
            QuantizedCollectiveBenchmarkSample::new(1_600, 8 * 1024 * 1024, 45),
            100,
            1_000,
        ),
        None,
        &healthy_policy,
    )?;
    let four_member_point = scaling_point(
        "four-member-healthy",
        2,
        vec!["worker-a", "worker-b", "worker-c", "worker-d"],
        vec![
            TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                .with_collective_group_size(2),
            TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                .with_collective_group_size(2),
        ],
        QuantizedCollectiveBenchmark::new(
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            QuantizedCollectiveBenchmarkSample::new(4_800, 16 * 1024 * 1024, 0),
            QuantizedCollectiveBenchmarkSample::new(2_000, 16 * 1024 * 1024, 50),
            100,
            1_000,
        ),
        None,
        &healthy_policy,
    )?;
    let degraded_four_member_point = scaling_point(
        "four-member-degraded",
        3,
        vec!["worker-a", "worker-b", "worker-c", "worker-d"],
        vec![
            TrainingDeviceMeshAxis::new("dp", TrainingDeviceMeshAxisKind::DataParallel, 2)
                .with_collective_group_size(2),
            TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 2)
                .with_collective_group_size(2),
        ],
        QuantizedCollectiveBenchmark::new(
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::Int8Symmetric,
            QuantizedCollectiveBenchmarkSample::new(4_800, 16 * 1024 * 1024, 0),
            QuantizedCollectiveBenchmarkSample::new(2_000, 16 * 1024 * 1024, 50),
            100,
            1_000,
        ),
        Some(
            CollectiveTransportFeedback::new(2_100, 400, 12, 2)
                .with_detail("elastic benchmark degraded transport"),
        ),
        &healthy_policy,
    )?;

    let two_to_four_member_scaling_bps = ratio_bps(
        four_member_point.effective_bytes_per_second,
        two_member_point.effective_bytes_per_second,
    );
    let disposition = if two_to_four_member_scaling_bps
        >= thresholds.min_two_to_four_member_scaling_bps
        && [
            two_member_point.quantized_speedup_bps,
            four_member_point.quantized_speedup_bps,
            degraded_four_member_point.quantized_speedup_bps,
        ]
        .into_iter()
        .all(|speedup| speedup >= thresholds.min_quantized_speedup_bps)
        && degraded_four_member_point.degraded_transport
        && degraded_four_member_point.cadence_class
            == CollectiveSyncCadenceClass::LocalOnlyDeferredGlobal
        && degraded_four_member_point.global_interval_steps
            <= thresholds.max_degraded_global_interval_steps
    {
        TrainBenchmarkDisposition::Passed
    } else {
        TrainBenchmarkDisposition::Failed
    };

    Ok(ElasticScalingBenchmarkReceipt {
        points: vec![
            two_member_point,
            four_member_point,
            degraded_four_member_point.clone(),
        ],
        two_to_four_member_scaling_bps,
        degraded_global_interval_steps: degraded_four_member_point.global_interval_steps,
        disposition,
    })
}

fn scaling_point(
    point_id: &str,
    mesh_revision: u64,
    nodes: Vec<&str>,
    axes: Vec<TrainingDeviceMeshAxis>,
    benchmark: QuantizedCollectiveBenchmark,
    feedback: Option<CollectiveTransportFeedback>,
    cadence_policy: &CollectiveSyncCadencePolicy,
) -> Result<ElasticScalingCurvePoint, TrainBenchmarkError> {
    let mut planner = ElasticCollectivePlanner::new(
        format!("mesh-{point_id}"),
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        axes,
    );
    planner.observe_mesh(
        collectives_membership_context(mesh_revision, nodes.as_slice()),
        mesh_members(nodes.as_slice()),
    )?;
    planner.record_benchmark(benchmark.clone());
    if let Some(feedback) = feedback {
        planner.observe_transport_feedback(feedback);
    }
    let plan = planner.plan_sync(
        2,
        TrainingCollectiveKind::AllReduce,
        benchmark.quantized.wire_bytes,
        TrainingCollectiveQuantization::Int8Symmetric,
        cadence_policy,
    )?;
    Ok(ElasticScalingCurvePoint {
        point_id: String::from(point_id),
        member_count: nodes.len(),
        mesh_revision: plan.cadence_receipt.mesh_revision,
        effective_bytes_per_second: bytes_per_second_from_micros(
            benchmark.quantized.wire_bytes,
            benchmark.quantized.duration_us,
        ),
        quantized_speedup_bps: benchmark.speedup_bps,
        cadence_class: plan.cadence_receipt.cadence_class,
        global_interval_steps: plan.cadence_receipt.global_interval_steps,
        degraded_transport: plan.cadence_receipt.degraded_transport,
        local_group_size: plan.cadence_receipt.local_group_size,
    })
}

fn benchmark_root() -> Result<PathBuf, TrainBenchmarkError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    Ok(std::env::temp_dir().join(format!("psionic-train-benchmark-{millis}")))
}

fn training_parameter_groups() -> Result<Vec<TrainingParameterGroupState>, TrainBenchmarkError> {
    Ok(vec![
        TrainingParameterGroupState::new(
            "decoder.weight",
            TrainingParameterClass::Matrix,
            TrainingTensorBuffer::from_f32(
                "decoder.weight",
                TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                vec![0.12, 0.18, 0.22, 0.27],
            )?,
            TrainingOptimizerConfig::adamw(0.001, 0.9, 0.999, 1e-8)
                .with_weight_decay(0.01)
                .with_gradient_clip_norm(1.0),
            TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
        )?,
        TrainingParameterGroupState::new(
            "decoder.bias",
            TrainingParameterClass::Bias,
            TrainingTensorBuffer::from_f32(
                "decoder.bias",
                TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                vec![0.0, 0.1],
            )?,
            TrainingOptimizerConfig::sgd(0.01).with_momentum(0.9),
            TrainingOptimizerResidencyPolicy::host_only(),
        )?,
    ])
}

fn trainer_step_input(
    batch_id: &str,
    weight_gradient: [f32; 4],
    bias_gradient: [f32; 2],
    started_at_ms: u64,
    finished_at_ms: u64,
) -> Result<crate::TrainingStepInput, TrainBenchmarkError> {
    Ok(crate::TrainingStepInput::new(
        TrainingGradientBatch::new(
            batch_id,
            0.25,
            8,
            BTreeMap::from([
                (
                    String::from("decoder.weight"),
                    TrainingTensorBuffer::from_f32(
                        "decoder.weight",
                        TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                        weight_gradient.to_vec(),
                    )?,
                ),
                (
                    String::from("decoder.bias"),
                    TrainingTensorBuffer::from_f32(
                        "decoder.bias",
                        TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                        bias_gradient.to_vec(),
                    )?,
                ),
            ]),
        ),
        started_at_ms,
        finished_at_ms,
    ))
}

fn benchmark_orchestrator_state(
    off_policy_budget: TrainingOffPolicyBudget,
) -> Result<TrainingOrchestratorState, TrainBenchmarkError> {
    let state = cluster_state(&[
        ("worker-b", ClusterMembershipStatus::Ready),
        ("trainer-a", ClusterMembershipStatus::Ready),
        ("worker-c", ClusterMembershipStatus::Ready),
    ]);
    let environment = EnvironmentPackageKey::new("oa.train", "2026.03");
    let mut run = TrainingRunState::new(
        "train-benchmark-run",
        "stage-rl",
        state.cluster_id().as_str(),
        "train.benchmark",
        environment,
    )?;
    run.apply_cluster_membership_snapshot(&state, 1_000)?;
    run.update_participant_priority(&NodeId::new("worker-b"), 9_200, 9_000, 1_010)?;
    run.update_participant_priority(&NodeId::new("trainer-a"), 8_700, 8_500, 1_020)?;
    run.update_participant_priority(&NodeId::new("worker-c"), 4_800, 4_900, 1_030)?;
    let target_policy =
        PolicyRevision::new("train.benchmark", "policy-rev-7", "policy-digest-7", 1_100)
            .with_revision_number(7)
            .with_parent_revision_id("policy-rev-6");
    Ok(TrainingOrchestratorState::new_with_budget(
        run,
        target_policy,
        benchmark_policy_weight_broadcast()?,
        off_policy_budget,
    )?)
}

fn benchmark_policy_weight_broadcast()
-> Result<DatastreamPolicyWeightBroadcastManifest, TrainBenchmarkError> {
    let shard_a = b"weight-shard-a".to_vec();
    let shard_b = b"weight-shard-b".to_vec();
    let assembled_digest =
        digest_bytes([shard_a.as_slice(), shard_b.as_slice()].concat().as_slice());
    let manifest_a = DatastreamManifest::from_bytes(
        "policy-shard-a",
        DatastreamSubjectKind::PolicyWeights,
        &shard_a,
        8,
        DatastreamEncoding::Safetensors,
    )
    .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
        "train.benchmark",
        7,
        "shard-a",
        0,
        2,
        assembled_digest.clone(),
        1_000,
        10_000,
    ));
    let manifest_b = DatastreamManifest::from_bytes(
        "policy-shard-b",
        DatastreamSubjectKind::PolicyWeights,
        &shard_b,
        8,
        DatastreamEncoding::Safetensors,
    )
    .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
        "train.benchmark",
        7,
        "shard-b",
        1,
        2,
        assembled_digest,
        1_000,
        10_000,
    ));
    Ok(InMemoryPolicyWeightBroadcast::new(
        vec![
            InMemoryDatastreamServer::new(manifest_a, shard_a)?,
            InMemoryDatastreamServer::new(manifest_b, shard_b)?,
        ],
        1_200,
    )?
    .broadcast()
    .clone())
}

fn benchmark_sandbox_profile() -> ProviderSandboxProfile {
    let profile_id = String::from("sandbox.train.benchmark.posix");
    ProviderSandboxProfile {
        profile_digest: digest_string(profile_id.as_str()),
        profile_id,
        execution_class: ProviderSandboxExecutionClass::PosixExec,
        runtime_family: String::from("posix"),
        runtime_version: String::from("system"),
        sandbox_engine: String::from("local_subprocess"),
        os_family: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu_limit: 2,
        memory_limit_mb: 512,
        disk_limit_mb: 512,
        timeout_limit_s: 10,
        network_mode: String::from("host_inherit"),
        filesystem_mode: String::from("host_inherit"),
        workspace_mode: String::from("persistent"),
        artifact_output_mode: String::from("workspace_relative"),
        secrets_mode: String::from("none"),
        allowed_binaries: vec![String::from("sh")],
        toolchain_inventory: vec![String::from("posix-shell")],
        container_image: None,
        runtime_image_digest: None,
        accelerator_policy: None,
        runtime_kind: ProviderSandboxRuntimeKind::Posix,
        runtime_ready: true,
        runtime_binary_path: None,
        capability_summary: String::from("train benchmark inline shell"),
    }
}

fn cluster_id() -> ClusterId {
    ClusterId::new(
        &ClusterNamespace::new("train-benchmark"),
        &AdmissionToken::new("train-benchmark-token"),
    )
}

fn membership_record(
    cluster_id: &ClusterId,
    node_id: &str,
    status: ClusterMembershipStatus,
) -> ClusterMembershipRecord {
    ClusterMembershipRecord::new(
        ClusterNodeIdentity {
            cluster_id: cluster_id.clone(),
            node_id: NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role: NodeRole::Mixed,
            auth_public_key: format!("{node_id}-pub"),
            attestation: None,
        },
        Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_000)),
        status,
    )
}

fn cluster_state(records: &[(&str, ClusterMembershipStatus)]) -> ClusterState {
    let cluster_id = cluster_id();
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships = records
        .iter()
        .map(|(node_id, status)| {
            (
                NodeId::new(*node_id),
                membership_record(&cluster_id, node_id, *status),
            )
        })
        .collect::<BTreeMap<_, _>>();
    ClusterState::from_snapshot(snapshot)
}

fn collectives_membership_context(
    mesh_revision: u64,
    nodes: &[&str],
) -> TrainingElasticMembershipContext {
    TrainingElasticMembershipContext::new(
        mesh_revision,
        format!("cluster-state-{mesh_revision}"),
        format!("topology-{mesh_revision}"),
        nodes.iter().map(ToString::to_string).collect(),
    )
}

fn mesh_members(nodes: &[&str]) -> Vec<CollectiveMeshMember> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node_id)| {
            CollectiveMeshMember::new(*node_id, index, index, format!("cuda:{index}"))
        })
        .collect()
}

fn unique_assigned_workers(window: &crate::TrainingOrchestratorWindow) -> Vec<String> {
    let mut assigned_workers = window
        .rollout_assignments
        .iter()
        .map(|assignment| assignment.contributor_node_id.clone())
        .collect::<Vec<_>>();
    assigned_workers.sort();
    assigned_workers.dedup();
    assigned_workers
}

fn rollout_artifact(
    worker_id: &str,
    artifact_id: &str,
    source_policy_revision: PolicyRevision,
    created_at_ms: u64,
) -> Result<RolloutArtifact, TrainBenchmarkError> {
    rollout_artifact_with_task(
        worker_id,
        artifact_id,
        format!("task-{artifact_id}").as_str(),
        source_policy_revision,
        created_at_ms,
    )
}

fn rollout_artifact_with_task(
    worker_id: &str,
    artifact_id: &str,
    task_id: &str,
    source_policy_revision: PolicyRevision,
    created_at_ms: u64,
) -> Result<RolloutArtifact, TrainBenchmarkError> {
    Ok(RolloutArtifact::new(
        artifact_id,
        worker_id,
        EnvironmentPackageKey::new("oa.train", "2026.03"),
        task_id,
        source_policy_revision,
        vec![
            RolloutSample::new(1, -0.2, 1.0, 0.8),
            RolloutSample::new(2, -0.1, 0.6, 0.4),
        ],
        RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            format!("proof-{artifact_id}"),
            format!("exec://{artifact_id}"),
        )],
        created_at_ms,
    )?)
}

fn validator_bundle(
    bundle_id: &str,
    worker_id: &str,
    artifact_id: &str,
    task_id: &str,
    policy_revision: PolicyRevision,
    include_benchmark: bool,
    stale_outcome: bool,
) -> Result<RolloutVerificationBundle, TrainBenchmarkError> {
    let artifact =
        rollout_artifact_with_task(worker_id, artifact_id, task_id, policy_revision, 1_120)?;
    let outcome = if stale_outcome {
        RolloutWorkerOutcomeKind::UploadedDiscarded
    } else {
        RolloutWorkerOutcomeKind::UploadedAcceptedExact
    };
    let policy_posture = if stale_outcome {
        RolloutWorkerPolicyPosture::DiscardedOffPolicy
    } else {
        RolloutWorkerPolicyPosture::ExactTarget
    };
    let admission_outcome = if stale_outcome {
        RolloutReceiptOutcome::Discarded
    } else {
        RolloutReceiptOutcome::AcceptedExact
    };
    Ok(RolloutVerificationBundle::new(
        bundle_id,
        artifact.clone(),
        worker_outcome(&artifact, outcome, policy_posture, admission_outcome),
        include_benchmark.then_some(RolloutBenchmarkObservation {
            observed_runtime_ms: 18,
            observed_token_count: 128,
            observed_final_state_digest: Some(String::from("final-state-digest")),
            declared_execution_strategy: Some(String::from("single_node")),
        }),
        include_benchmark.then_some(RolloutBenchmarkExpectation {
            min_runtime_ms: 5,
            max_runtime_ms: 25,
            expected_token_count: 128,
            expected_final_state_digest: Some(String::from("final-state-digest")),
            expected_execution_strategy: Some(String::from("single_node")),
        }),
    ))
}

fn admission_receipt(
    artifact: &RolloutArtifact,
    outcome: RolloutReceiptOutcome,
) -> RolloutAdmissionReceipt {
    RolloutAdmissionReceipt {
        receipt_id: format!("receipt-{}", artifact.artifact_id),
        run_id: String::from("train-benchmark-run"),
        stage_id: String::from("stage-rl"),
        window_id: String::from("train-benchmark-window"),
        artifact_id: artifact.artifact_id.clone(),
        artifact_digest: artifact.artifact_digest.clone(),
        worker_id: artifact.worker_id.clone(),
        environment_key: artifact.environment.storage_key(),
        target_policy_revision_id: String::from("policy-rev-7"),
        source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
        source_policy_digest: artifact.source_policy_revision.policy_digest.clone(),
        outcome,
        revision_drift: None,
        policy_age_ms: None,
        rollout_age_ms: 5,
        signals: Vec::new(),
        token_count: artifact.token_count(),
        reward_sum: artifact.reward_sum(),
        termination_reason: artifact.termination_reason,
        observed_at_ms: 1_125,
        receipt_digest: format!("receipt-digest-{}", artifact.artifact_id),
    }
}

fn worker_outcome(
    artifact: &RolloutArtifact,
    outcome: RolloutWorkerOutcomeKind,
    policy_posture: RolloutWorkerPolicyPosture,
    admission_outcome: RolloutReceiptOutcome,
) -> RolloutWorkerOutcomeReceipt {
    RolloutWorkerOutcomeReceipt {
        receipt_id: format!("worker-outcome-{}", artifact.artifact_id),
        claim_id: format!("claim-{}", artifact.artifact_id),
        assignment_id: format!("assignment-{}", artifact.worker_id),
        window_id: String::from("train-benchmark-window"),
        worker_id: artifact.worker_id.clone(),
        trust_class: RolloutWorkerTrustClass::UntrustedWorker,
        target_policy_revision_id: String::from("policy-rev-7"),
        source_policy_revision_id: artifact.source_policy_revision.revision_id.clone(),
        outcome,
        policy_posture,
        upload: crate::RolloutUploadLocator::new(
            crate::RolloutUploadTransport::InlineArtifact,
            format!("inline://{}", artifact.artifact_id),
            512,
            artifact.artifact_digest.as_str(),
        ),
        rejection_reason: None,
        admission_receipt: Some(admission_receipt(artifact, admission_outcome)),
        observed_at_ms: 1_125,
        receipt_digest: format!("worker-outcome-digest-{}", artifact.artifact_id),
    }
}

fn elapsed_nonzero_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().max(1) as u64
}

fn units_per_second(units: u64, elapsed_ms: u64) -> u64 {
    units.saturating_mul(1_000) / elapsed_ms.max(1)
}

fn bytes_per_second_from_micros(bytes: u64, duration_us: u64) -> u64 {
    bytes.saturating_mul(1_000_000) / duration_us.max(1)
}

fn ratio_bps(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    numerator.saturating_mul(10_000) / denominator
}

fn mean_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().copied().sum::<u64>() / values.len() as u64
}

fn stable_suite_digest(
    profile: &TrainBenchmarkThresholdProfile,
    trainer: &TrainerThroughputBenchmarkReceipt,
    rollout_ingestion: &RolloutIngestionBenchmarkReceipt,
    sandbox_reuse: &SandboxReuseBenchmarkReceipt,
    checkpoint_recovery: &CheckpointRecoveryBenchmarkReceipt,
    validator_cost: &ValidatorCostBenchmarkReceipt,
    elastic_scaling: &ElasticScalingBenchmarkReceipt,
    passed_count: usize,
    failed_count: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_benchmark_suite|");
    hasher.update(stable_json_bytes(profile));
    hasher.update(stable_json_bytes(trainer));
    hasher.update(stable_json_bytes(rollout_ingestion));
    hasher.update(stable_json_bytes(sandbox_reuse));
    hasher.update(stable_json_bytes(checkpoint_recovery));
    hasher.update(stable_json_bytes(validator_cost));
    hasher.update(stable_json_bytes(elastic_scaling));
    hasher.update(passed_count.to_string().as_bytes());
    hasher.update(failed_count.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_json_bytes(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("benchmark JSON serialization failed")
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn digest_string(value: &str) -> String {
    digest_bytes(value.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_benchmark_suite_closes_all_acceptance_categories()
    -> Result<(), Box<dyn std::error::Error>> {
        let suite = TrainBenchmarkHarness::run_reference_suite()?;
        assert_eq!(suite.failed_count, 0);
        assert_eq!(suite.passed_count, 6);
        assert_eq!(suite.trainer.disposition, TrainBenchmarkDisposition::Passed);
        assert_eq!(
            suite.rollout_ingestion.disposition,
            TrainBenchmarkDisposition::Passed
        );
        assert_eq!(
            suite.sandbox_reuse.disposition,
            TrainBenchmarkDisposition::Passed
        );
        assert_eq!(
            suite.checkpoint_recovery.disposition,
            TrainBenchmarkDisposition::Passed
        );
        assert!(suite.checkpoint_recovery.resumed_delivery);
        assert_eq!(
            suite.validator_cost.disposition,
            TrainBenchmarkDisposition::Passed
        );
        assert!(suite.validator_cost.accepted_count > 0);
        assert!(suite.validator_cost.normalized_count > 0);
        assert!(suite.validator_cost.rejected_count > 0);
        assert_eq!(
            suite.elastic_scaling.disposition,
            TrainBenchmarkDisposition::Passed
        );
        assert_eq!(suite.elastic_scaling.points.len(), 3);
        Ok(())
    }

    #[test]
    fn stricter_trainer_thresholds_fail_the_suite() -> Result<(), Box<dyn std::error::Error>> {
        let mut profile = TrainBenchmarkThresholdProfile::reference();
        profile.trainer.min_steps_per_second = 1_000;
        let suite = TrainBenchmarkHarness::run_suite_with_profile(&profile)?;
        assert_eq!(suite.trainer.disposition, TrainBenchmarkDisposition::Failed);
        assert!(suite.failed_count >= 1);
        Ok(())
    }
}
