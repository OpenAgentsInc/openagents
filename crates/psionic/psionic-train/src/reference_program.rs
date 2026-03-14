use std::collections::BTreeMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use psionic_cluster::{
    AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus, ClusterNamespace,
    ClusterNodeIdentity, ClusterSnapshot, NodeEpoch, NodeId, NodeRole,
};
use psionic_core::{DType, Device, Shape, TensorSpec};
use psionic_data::DatasetKey;
use psionic_datastream::{
    DatastreamEncoding, DatastreamManifest, DatastreamPolicyWeightBinding,
    DatastreamPolicyWeightBroadcastReceipt, DatastreamSubjectKind, DatastreamTransferError,
    InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
};
use psionic_environments::{
    EnvironmentArtifactExpectation, EnvironmentArtifactOutput, EnvironmentBenchmarkProfile,
    EnvironmentDatasetBinding, EnvironmentDifficultyMetadata, EnvironmentExecutionEntrypoint,
    EnvironmentPackageContract, EnvironmentPackageFamily, EnvironmentPackageKey,
    EnvironmentPolicyKind, EnvironmentPolicyReference, EnvironmentRubricHook,
    EnvironmentRubricOutcome, EnvironmentRubricScoreKind, EnvironmentRuntimeError,
    EnvironmentRuntimeFamily, EnvironmentSessionSummary, EnvironmentStateMode, EnvironmentToolCall,
    EnvironmentToolContract, EnvironmentToolInterface, EnvironmentToolResult, EnvironmentTurnInput,
    EnvironmentVerificationPosture, EnvironmentWorkloadClass,
};
use psionic_eval::{
    BenchmarkAggregateSummary, BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode,
    BenchmarkPackage, BenchmarkPackageKey, BenchmarkVerificationPolicy, EvalArtifact,
    EvalExecutionStrategyFacts, EvalFinalStateCapture, EvalRunContract, EvalRunMode, EvalRunState,
    EvalRuntimeError, EvalSampleRecord, EvalTimerIntegrityFacts, EvalTokenAccountingFacts,
    EvalVerificationFacts,
};
use psionic_runtime::TrainingCheckpointReference;
use psionic_sandbox::{
    InMemorySandboxPoolService, ProviderSandboxEntrypointType, ProviderSandboxExecutionClass,
    ProviderSandboxExecutionControls, ProviderSandboxProfile, ProviderSandboxResourceRequest,
    ProviderSandboxRuntimeKind, SandboxLoopIterationReceipt, SandboxLoopIterationRequest,
    SandboxPoolAcquisitionReceipt, SandboxPoolError, SandboxPoolSnapshot, SandboxPoolSpec,
    SandboxPoolWarmReceipt, SandboxStageArtifactKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, PolicyRevision, RolloutArtifact, RolloutBenchmarkExpectation,
    RolloutBenchmarkObservation, RolloutProofKind, RolloutProofReference, RolloutSample,
    RolloutTaskClaim, RolloutUploadLocator, RolloutUploadTransport, RolloutValidatorPolicy,
    RolloutValidatorState, RolloutVerificationBundle, RolloutWorkerHeartbeatReceipt,
    RolloutWorkerIdentity, RolloutWorkerProtocolError, RolloutWorkerProtocolPolicy,
    RolloutWorkerProtocolState, RolloutWorkerTrustClass, TrainerBatch, TrainingCoreError,
    TrainingGradientBatch, TrainingLoopBudget, TrainingOffPolicyBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingOrchestratorBatchRecord, TrainingOrchestratorError,
    TrainingOrchestratorState, TrainingOrchestratorWindow, TrainingParameterClass,
    TrainingParameterGroupState, TrainingRunGraphError, TrainingRunOutcome, TrainingRunState,
    TrainingSftTraceArtifact, TrainingSftTraceKind, TrainingStageKind, TrainingStageProgramError,
    TrainingStageProgramState, TrainingStepInput, TrainingTensorBuffer,
    TrainingToolCallTraceLineage, TrainingToolCallTraceStep, TrainingWindowAssignmentRule,
    rl_artifacts::RolloutContractError,
};

/// Failure returned by the integrated agentic SFT plus RL reference program.
#[derive(Debug, Error)]
pub enum AgenticSftRlReferenceProgramError {
    /// I/O failure while preparing the local pilot workspace.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Datastream transfer or broadcast failure.
    #[error(transparent)]
    Datastream(#[from] DatastreamTransferError),
    /// Environment runtime or contract failure.
    #[error(transparent)]
    Environment(#[from] EnvironmentRuntimeError),
    /// Eval runtime or benchmark failure.
    #[error(transparent)]
    Eval(#[from] EvalRuntimeError),
    /// Sandbox pool or execution failure.
    #[error(transparent)]
    Sandbox(#[from] SandboxPoolError),
    /// Multi-stage training-program failure.
    #[error(transparent)]
    StageProgram(#[from] TrainingStageProgramError),
    /// Training run-graph failure.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// Orchestrator failure.
    #[error(transparent)]
    Orchestrator(#[from] TrainingOrchestratorError),
    /// Rollout worker protocol failure.
    #[error(transparent)]
    WorkerProtocol(#[from] RolloutWorkerProtocolError),
    /// Rollout or trainer-batch contract failure.
    #[error(transparent)]
    RolloutContract(#[from] RolloutContractError),
    /// Training-core loop failure.
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    /// The pilot expected one worker assignment that was not present.
    #[error("reference program is missing a rollout assignment for worker `{worker_id}`")]
    MissingWorkerAssignment {
        /// Stable worker identifier.
        worker_id: String,
    },
    /// The sandbox report did not contain a reusable session.
    #[error("reference program sandbox report did not surface a reusable session")]
    MissingSandboxSession,
    /// One staged sandbox output could not be read back.
    #[error("reference program sandbox output `{relative_path}` is missing")]
    MissingSandboxOutput {
        /// Relative output path.
        relative_path: String,
    },
    /// One sandbox output could not be parsed into the expected JSON shape.
    #[error("reference program sandbox output `{relative_path}` is invalid JSON: {detail}")]
    InvalidSandboxOutput {
        /// Relative output path.
        relative_path: String,
        /// Low-level parse detail.
        detail: String,
    },
    /// The benchmark bundle manifest was not attached where the pilot expected it.
    #[error("reference program benchmark package is missing its eval bundle")]
    MissingBenchmarkBundle,
    /// The orchestrator did not expose the expected active window record.
    #[error("reference program is missing the current orchestrator window")]
    MissingCurrentWindow,
}

/// Stable input contract for the canonical agentic SFT plus RL pilot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgenticSftRlReferenceProgramSpec {
    /// Stable training-program identifier.
    pub run_id: String,
    /// Shared checkpoint family used across stage promotions and RL policy state.
    pub checkpoint_family: String,
    /// Stable policy family used for RL rollout and trainer-batch lineage.
    pub policy_family: String,
    /// Versioned environment identity used across SFT, RL, and eval phases.
    pub environment: EnvironmentPackageKey,
    /// Versioned dataset identity bound into environment and eval contracts.
    pub dataset: DatasetKey,
    /// Versioned benchmark package identity used for validator-style eval.
    pub benchmark_package: BenchmarkPackageKey,
    /// Stable cluster identity used for run-graph and orchestrator truth.
    pub cluster_id: ClusterId,
    /// Workspace root for sandbox-pool and artifact outputs.
    pub workspace_root: PathBuf,
    /// Deterministic base timestamp used by the integrated pilot.
    pub base_time_ms: u64,
}

impl AgenticSftRlReferenceProgramSpec {
    /// Returns the canonical weather-agent reference-program spec.
    #[must_use]
    pub fn weather_default(workspace_root: PathBuf) -> Self {
        Self {
            run_id: String::from("weather-agentic-sft-rl-reference"),
            checkpoint_family: String::from("train.weather.agent"),
            policy_family: String::from("train.weather.agent"),
            environment: EnvironmentPackageKey::new("oa.weather.agent", "2026.03"),
            dataset: DatasetKey::new("oa.weather.reference", "2026.03"),
            benchmark_package: BenchmarkPackageKey::new(
                "oa.weather.agent.reference-benchmark",
                "2026.03",
            ),
            cluster_id: ClusterId::new(
                &ClusterNamespace::new("psionic-reference-program"),
                &AdmissionToken::new("weather-agentic-sft-rl"),
            ),
            workspace_root,
            base_time_ms: 1_763_100_000_000,
        }
    }
}

/// Explicit dataset, trace, checkpoint, and policy lineage exposed by the
/// integrated pilot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgenticSftRlReferenceLineage {
    /// Shared dataset identity surfaced through environment and eval contracts.
    pub dataset_storage_key: String,
    /// Source references carried by the SFT trace set.
    pub sft_trace_source_refs: Vec<String>,
    /// Stable general-SFT checkpoint reference when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub general_checkpoint_ref: Option<String>,
    /// Stable agentic-SFT checkpoint reference when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agentic_checkpoint_ref: Option<String>,
    /// Stable target policy revision id.
    pub target_policy_revision_id: String,
    /// Stable target policy checkpoint reference when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_policy_checkpoint_ref: Option<String>,
    /// Stable policy-weight broadcast digest delivered by datastream.
    pub policy_weight_broadcast_digest: String,
    /// Online-eval run identifier.
    pub online_eval_run_id: String,
    /// Benchmark eval-run identifier.
    pub benchmark_eval_run_id: String,
}

/// Sandbox receipts and snapshots surfaced by the pilot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgenticSftRlReferenceSandboxReport {
    /// Initial warm receipts used to reach the target ready count.
    pub warm_receipts: Vec<SandboxPoolWarmReceipt>,
    /// Acquisition receipts proving ready-session reuse.
    pub acquisition_receipts: Vec<SandboxPoolAcquisitionReceipt>,
    /// Iteration receipts proving staged-input reuse and repeated execution.
    pub iteration_receipts: Vec<SandboxLoopIterationReceipt>,
    /// Final sandbox-pool snapshot after the pilot finished.
    pub final_snapshot: SandboxPoolSnapshot,
}

/// Condensed operator-facing summary derived from the full typed pilot report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgenticSftRlReferenceOperatorView {
    /// Latest stage kind left active in the stage program.
    pub latest_stage_kind: TrainingStageKind,
    /// Window identifier used for the RL rollout phase.
    pub training_window_id: String,
    /// Accepted rollout count in the active window.
    pub accepted_rollout_count: u32,
    /// Quarantined rollout count in the active window.
    pub quarantined_rollout_count: u32,
    /// Discarded rollout count in the active window.
    pub discarded_rollout_count: u32,
    /// Validator-accepted contribution count.
    pub validator_accepted_count: u32,
    /// Validator-normalized contribution count.
    pub validator_normalized_count: u32,
    /// Validator-rejected contribution count.
    pub validator_rejected_count: u32,
    /// Ready sandbox sessions visible after the pilot.
    pub sandbox_ready_sessions: u32,
    /// Iteration count that explicitly reused a prior workspace.
    pub sandbox_reused_iterations: u32,
    /// Average online-eval score in basis points when scoring succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_eval_average_score_bps: Option<u32>,
    /// Aggregate benchmark score in basis points when scoring succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_aggregate_score_bps: Option<u32>,
    /// Completed fixed-budget trainer steps in the pilot.
    pub completed_trainer_steps: u64,
}

impl AgenticSftRlReferenceOperatorView {
    /// Returns a text summary ready for CLI, HUD, or control-plane inspection.
    #[must_use]
    pub fn summary_lines(&self) -> Vec<String> {
        vec![
            format!(
                "latest stage: {}",
                training_stage_kind_label(self.latest_stage_kind)
            ),
            format!("training window: {}", self.training_window_id),
            format!(
                "rollouts accepted/quarantined/discarded: {}/{}/{}",
                self.accepted_rollout_count,
                self.quarantined_rollout_count,
                self.discarded_rollout_count
            ),
            format!(
                "validator accepted/normalized/rejected: {}/{}/{}",
                self.validator_accepted_count,
                self.validator_normalized_count,
                self.validator_rejected_count
            ),
            format!(
                "sandbox ready sessions: {}, reused iterations: {}",
                self.sandbox_ready_sessions, self.sandbox_reused_iterations
            ),
            format!(
                "online eval average score bps: {}",
                self.online_eval_average_score_bps
                    .map_or_else(|| String::from("none"), |value| value.to_string())
            ),
            format!(
                "benchmark aggregate score bps: {}",
                self.benchmark_aggregate_score_bps
                    .map_or_else(|| String::from("none"), |value| value.to_string())
            ),
            format!("completed trainer steps: {}", self.completed_trainer_steps),
        ]
    }
}

/// Full typed report for the canonical agentic SFT plus RL pilot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgenticSftRlReferenceProgramReport {
    /// Input spec that produced this report.
    pub spec: AgenticSftRlReferenceProgramSpec,
    /// Explicit dataset, checkpoint, policy, and eval lineage.
    pub lineage: AgenticSftRlReferenceLineage,
    /// Versioned environment package contract used throughout the pilot.
    pub environment_package: EnvironmentPackageContract,
    /// Validator-style benchmark package used by the pilot.
    pub benchmark_package: BenchmarkPackage,
    /// Stage-program state spanning general SFT, agentic SFT, and RL.
    pub stage_program: TrainingStageProgramState,
    /// Stable checkpoint promoted out of the general-SFT phase.
    pub general_checkpoint: TrainingCheckpointReference,
    /// Stable checkpoint promoted out of the agentic-SFT phase.
    pub agentic_checkpoint: TrainingCheckpointReference,
    /// Delivered policy-weight broadcast receipt for the RL policy revision.
    pub policy_weight_broadcast_receipt: DatastreamPolicyWeightBroadcastReceipt,
    /// Sandbox receipts and snapshots proving warm-pool reuse.
    pub sandbox: AgenticSftRlReferenceSandboxReport,
    /// Agentic environment session summary promoted into eval and trace lineage.
    pub agentic_session_summary: EnvironmentSessionSummary,
    /// Online-eval run over the live policy revision.
    pub online_eval_run: EvalRunState,
    /// Benchmark-mode eval run used for validator-style aggregation.
    pub benchmark_eval_run: EvalRunState,
    /// Aggregate benchmark summary for the pilot.
    pub benchmark_summary: BenchmarkAggregateSummary,
    /// Final orchestrator state after rollout, batch assembly, and reconcile.
    pub orchestrator: TrainingOrchestratorState,
    /// Final window snapshot used by the RL phase.
    pub window: TrainingOrchestratorWindow,
    /// Worker heartbeat receipts proving explicit worker liveness.
    pub worker_heartbeats: Vec<RolloutWorkerHeartbeatReceipt>,
    /// Worker assignment-claim receipts used for rollout admission.
    pub worker_claims: Vec<RolloutTaskClaim>,
    /// Final worker-outcome receipts emitted by the upload protocol.
    pub worker_outcomes: Vec<crate::RolloutWorkerOutcomeReceipt>,
    /// Rollout validator state with emitted verdict history.
    pub validator_state: RolloutValidatorState,
    /// Trainer-batch assembly record emitted by the orchestrator.
    pub trainer_batch_record: TrainingOrchestratorBatchRecord,
    /// Fixed-budget trainer outcome over the assembled trainer batch.
    pub training_outcome: TrainingRunOutcome,
    /// Condensed operator summary derived from the full typed report.
    pub operator_view: AgenticSftRlReferenceOperatorView,
}

/// Runs the canonical end-to-end agentic SFT plus RL pilot over the existing
/// Psionic train, eval, sandbox, datastream, and cluster substrate.
pub fn run_agentic_sft_rl_reference_program(
    spec: &AgenticSftRlReferenceProgramSpec,
) -> Result<AgenticSftRlReferenceProgramReport, AgenticSftRlReferenceProgramError> {
    fs::create_dir_all(spec.workspace_root.as_path())?;

    let environment_package = weather_environment_package(spec);
    let benchmark_eval_bundle = benchmark_eval_bundle(spec);
    let benchmark_eval_bundle_ref = benchmark_eval_bundle.manifest_ref();
    let benchmark_package = weather_benchmark_package(spec, benchmark_eval_bundle_ref.clone());

    let sandbox_run = run_reference_sandbox(spec)?;
    let general_summary = weather_environment_summary(
        &environment_package,
        "weather-session-general",
        "weather-task-general",
        &sandbox_run.outputs[0],
    )?;
    let agentic_summary = weather_environment_summary(
        &environment_package,
        "weather-session-agentic",
        "weather-task-agentic",
        &sandbox_run.outputs[1],
    )?;

    let mut stage_program =
        TrainingStageProgramState::new(spec.run_id.clone(), spec.checkpoint_family.clone())?;
    stage_program.start_initial_stage(spec.environment.clone())?;

    let dataset_source_prefix = format!("dataset://{}", spec.dataset.storage_key());
    let sft_trace_source_refs = vec![
        format!("{dataset_source_prefix}/train/plain-1"),
        format!("{dataset_source_prefix}/train/long-context-1"),
        format!("{dataset_source_prefix}/train/agentic-tool-1"),
    ];
    stage_program.ingest_trace(
        &TrainingSftTraceArtifact::new(
            "general-plain-1",
            spec.environment.clone(),
            TrainingSftTraceKind::PlainCompletion,
            "digest://general/plain/input",
            "digest://general/plain/output",
        )
        .with_session_digest(general_summary.session_digest.clone())
        .with_source_ref(sft_trace_source_refs[0].clone()),
    )?;
    stage_program.ingest_trace(
        &TrainingSftTraceArtifact::new(
            "general-long-context-1",
            spec.environment.clone(),
            TrainingSftTraceKind::LongContext,
            "digest://general/long/input",
            "digest://general/long/output",
        )
        .with_session_digest(general_summary.session_digest.clone())
        .with_source_ref(sft_trace_source_refs[1].clone())
        .with_long_context_lineage(crate::TrainingLongContextTraceLineage::new(
            8_192,
            vec![
                String::from("segment://forecast-context-1"),
                String::from("segment://forecast-context-2"),
                String::from("segment://forecast-context-3"),
            ],
        )),
    )?;
    stage_program.complete_current_stage()?;

    let general_checkpoint =
        training_checkpoint(spec, "general-sft", "trainer-a", 1, spec.base_time_ms + 100);
    stage_program.advance_stage(
        TrainingStageKind::AgenticSft,
        spec.environment.clone(),
        general_checkpoint.clone(),
    )?;

    stage_program.ingest_trace(
        &TrainingSftTraceArtifact::new(
            "agentic-tool-1",
            spec.environment.clone(),
            TrainingSftTraceKind::ToolCall,
            "digest://agentic/tool/input",
            "digest://agentic/tool/output",
        )
        .with_session_digest(agentic_summary.session_digest.clone())
        .with_source_ref(sft_trace_source_refs[2].clone())
        .with_tool_call_lineage(TrainingToolCallTraceLineage::new(vec![
            TrainingToolCallTraceStep {
                tool_name: String::from("get_weather"),
                arguments_digest: digest_json(&json!({
                    "city": sandbox_run.outputs[1].city,
                })),
                result_digest: digest_json(&sandbox_run.outputs[1].weather_output),
            },
        ])),
    )?;
    stage_program.complete_current_stage()?;

    let agentic_checkpoint =
        training_checkpoint(spec, "agentic-sft", "trainer-a", 2, spec.base_time_ms + 200);
    stage_program.advance_stage(
        TrainingStageKind::Rl,
        spec.environment.clone(),
        agentic_checkpoint.clone(),
    )?;

    let target_policy_revision = PolicyRevision::new(
        spec.policy_family.clone(),
        "policy-rev-3",
        "policy-digest-rev-3",
        spec.base_time_ms + 220,
    )
    .with_revision_number(3)
    .with_parent_revision_id("policy-rev-2")
    .with_checkpoint(agentic_checkpoint.clone());
    let off_policy_revision = PolicyRevision::new(
        spec.policy_family.clone(),
        "policy-rev-2",
        "policy-digest-rev-2",
        spec.base_time_ms + 150,
    )
    .with_revision_number(2)
    .with_parent_revision_id("policy-rev-1")
    .with_checkpoint(general_checkpoint.clone());

    let policy_weight_broadcast =
        policy_weight_broadcast_service(spec, &target_policy_revision, spec.base_time_ms + 225)?;
    let policy_weight_broadcast_receipt =
        policy_weight_broadcast.deliver(spec.base_time_ms + 250, 2)?;

    let cluster_state = reference_cluster_state(&spec.cluster_id);
    let mut run = TrainingRunState::new(
        spec.run_id.clone(),
        "stage-rl",
        spec.cluster_id.as_str(),
        spec.checkpoint_family.clone(),
        spec.environment.clone(),
    )?;
    run.apply_cluster_membership_snapshot(&cluster_state, spec.base_time_ms + 260)?;
    run.update_participant_priority(
        &NodeId::new("worker-b"),
        9_600,
        9_500,
        spec.base_time_ms + 261,
    )?;
    run.update_participant_priority(
        &NodeId::new("worker-c"),
        9_100,
        9_300,
        spec.base_time_ms + 262,
    )?;
    run.update_participant_priority(
        &NodeId::new("trainer-a"),
        7_500,
        8_100,
        spec.base_time_ms + 263,
    )?;

    let mut orchestrator = TrainingOrchestratorState::new_with_budget(
        run,
        target_policy_revision.clone(),
        policy_weight_broadcast.broadcast().clone(),
        TrainingOffPolicyBudget::default(),
    )?;
    let initial_window = orchestrator.plan_next_window(
        2,
        TrainingWindowAssignmentRule::RoundRobinByPriority {
            batch_slice_count: 2,
            eval_slice_count: 1,
        },
        7_733,
        spec.base_time_ms + 270,
    )?;
    orchestrator.activate_current_window(spec.base_time_ms + 280)?;

    let mut worker_protocol = RolloutWorkerProtocolState::from_window(
        &initial_window,
        target_policy_revision.clone(),
        RolloutWorkerProtocolPolicy::default(),
    );
    let worker_heartbeats = vec![
        worker_protocol.record_heartbeat(
            RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::SemiTrustedWorker,
                "pilot:worker-b",
            ),
            spec.base_time_ms + 285,
        ),
        worker_protocol.record_heartbeat(
            RolloutWorkerIdentity::new(
                "worker-c",
                RolloutWorkerTrustClass::SemiTrustedWorker,
                "pilot:worker-c",
            ),
            spec.base_time_ms + 286,
        ),
    ];

    let assignment_b = initial_window
        .rollout_assignments
        .iter()
        .find(|assignment| assignment.contributor_node_id == "worker-b")
        .ok_or_else(
            || AgenticSftRlReferenceProgramError::MissingWorkerAssignment {
                worker_id: String::from("worker-b"),
            },
        )?;
    let assignment_c = initial_window
        .rollout_assignments
        .iter()
        .find(|assignment| assignment.contributor_node_id == "worker-c")
        .ok_or_else(
            || AgenticSftRlReferenceProgramError::MissingWorkerAssignment {
                worker_id: String::from("worker-c"),
            },
        )?;

    let claim_b = worker_protocol.claim_assignment(
        "worker-b",
        assignment_b.assignment_id.as_str(),
        spec.base_time_ms + 290,
    )?;
    let claim_c = worker_protocol.claim_assignment(
        "worker-c",
        assignment_c.assignment_id.as_str(),
        spec.base_time_ms + 291,
    )?;

    let rollout_exact = rollout_artifact(
        "worker-b",
        "rollout-exact-1",
        spec.environment.clone(),
        target_policy_revision.clone(),
        spec.base_time_ms + 295,
    )?;
    let rollout_off_policy = rollout_artifact(
        "worker-c",
        "rollout-off-policy-1",
        spec.environment.clone(),
        off_policy_revision,
        spec.base_time_ms + 296,
    )?;

    let outcome_exact = worker_protocol.submit_claimed_rollout(
        &mut orchestrator,
        claim_b.claim_id.as_str(),
        rollout_exact.clone(),
        RolloutUploadLocator::new(
            RolloutUploadTransport::ExternalReference,
            "artifact://rollout-exact-1",
            2_048,
            rollout_exact.artifact_digest.clone(),
        ),
        spec.base_time_ms + 305,
    )?;
    let outcome_off_policy = worker_protocol.submit_claimed_rollout(
        &mut orchestrator,
        claim_c.claim_id.as_str(),
        rollout_off_policy.clone(),
        RolloutUploadLocator::new(
            RolloutUploadTransport::ExternalReference,
            "artifact://rollout-off-policy-1",
            2_064,
            rollout_off_policy.artifact_digest.clone(),
        ),
        spec.base_time_ms + 306,
    )?;
    let worker_claims = vec![claim_b, claim_c];
    let worker_outcomes = vec![outcome_exact.clone(), outcome_off_policy.clone()];

    orchestrator.seal_current_window(spec.base_time_ms + 320)?;
    let trainer_batch_record = orchestrator.assemble_trainer_batch(
        "trainer-batch-weather-1",
        vec![
            rollout_exact.artifact_id.clone(),
            rollout_off_policy.artifact_id.clone(),
        ],
        spec.base_time_ms + 330,
    )?;

    let mut validator_state = RolloutValidatorState::new(RolloutValidatorPolicy::default());
    validator_state.verify_bundle(rollout_bundle(
        "bundle-exact",
        rollout_exact,
        outcome_exact,
        &agentic_summary,
    ));
    validator_state.verify_bundle(rollout_bundle(
        "bundle-off-policy",
        rollout_off_policy,
        outcome_off_policy,
        &agentic_summary,
    ));

    orchestrator.score_current_window(spec.base_time_ms + 340)?;
    orchestrator.reconcile_current_window(spec.base_time_ms + 350)?;

    let online_eval_run = online_eval_run(
        spec,
        &environment_package,
        &agentic_summary,
        &target_policy_revision,
        spec.base_time_ms + 360,
    )?;
    let benchmark_eval_run = benchmark_eval_run(
        spec,
        &environment_package,
        &benchmark_package,
        [&general_summary, &agentic_summary],
        &target_policy_revision,
        spec.base_time_ms + 370,
    )?;
    let benchmark_summary = benchmark_summary(&benchmark_package, &benchmark_eval_run)?;

    let training_outcome = training_outcome(
        spec,
        &target_policy_revision,
        &trainer_batch_record.batch,
        spec.base_time_ms + 380,
    )?;

    let window = orchestrator
        .orchestrator_windows
        .last()
        .cloned()
        .ok_or(AgenticSftRlReferenceProgramError::MissingCurrentWindow)?;
    let operator_view = operator_view(
        stage_program
            .current_stage()
            .map(|stage| stage.kind)
            .unwrap_or(TrainingStageKind::Rl),
        &window,
        &validator_state,
        &sandbox_run.report.final_snapshot,
        &sandbox_run.report.iteration_receipts,
        online_eval_run.summary.as_ref(),
        Some(&benchmark_summary),
        training_outcome.summary.completed_steps,
    );

    let benchmark_eval_bundle_ref = benchmark_package
        .eval_bundle
        .clone()
        .ok_or(AgenticSftRlReferenceProgramError::MissingBenchmarkBundle)?;
    let lineage = AgenticSftRlReferenceLineage {
        dataset_storage_key: spec.dataset.storage_key(),
        sft_trace_source_refs,
        general_checkpoint_ref: general_checkpoint.checkpoint_ref.clone(),
        agentic_checkpoint_ref: agentic_checkpoint.checkpoint_ref.clone(),
        target_policy_revision_id: target_policy_revision.revision_id.clone(),
        target_policy_checkpoint_ref: target_policy_revision
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.checkpoint_ref.clone()),
        policy_weight_broadcast_digest: policy_weight_broadcast_receipt
            .broadcast
            .broadcast_digest
            .clone(),
        online_eval_run_id: online_eval_run.contract.eval_run_id.clone(),
        benchmark_eval_run_id: benchmark_eval_run.contract.eval_run_id.clone(),
    };

    let _ = benchmark_eval_bundle_ref;

    Ok(AgenticSftRlReferenceProgramReport {
        spec: spec.clone(),
        lineage,
        environment_package,
        benchmark_package,
        stage_program,
        general_checkpoint,
        agentic_checkpoint,
        policy_weight_broadcast_receipt,
        sandbox: sandbox_run.report,
        agentic_session_summary: agentic_summary,
        online_eval_run,
        benchmark_eval_run,
        benchmark_summary,
        orchestrator,
        window,
        worker_heartbeats,
        worker_claims,
        worker_outcomes,
        validator_state,
        trainer_batch_record,
        training_outcome,
        operator_view,
    })
}

struct ReferenceSandboxRun {
    report: AgenticSftRlReferenceSandboxReport,
    outputs: Vec<ReferenceSandboxOutput>,
}

struct ReferenceSandboxOutput {
    city: String,
    weather_output: Value,
    weather_artifact: EnvironmentArtifactOutput,
    reward_artifact: EnvironmentArtifactOutput,
    reward_bps: i32,
}

fn weather_environment_package(
    spec: &AgenticSftRlReferenceProgramSpec,
) -> EnvironmentPackageContract {
    EnvironmentPackageContract::new(
        spec.environment.clone(),
        EnvironmentPackageFamily::Agentic,
        "Weather Agent Reference Environment",
        EnvironmentExecutionEntrypoint {
            runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
            entrypoint: String::from("weather_agent"),
            args: vec![String::from("--reference-program")],
            sandbox_profile_ref: Some(String::from("sandbox.weather.posix.reference")),
            max_turns: 4,
            state_mode: EnvironmentStateMode::SessionPersistent,
            time_budget_ms: Some(5_000),
        },
    )
    .with_supported_workloads(vec![
        EnvironmentWorkloadClass::Sft,
        EnvironmentWorkloadClass::Rl,
        EnvironmentWorkloadClass::OnlineEval,
        EnvironmentWorkloadClass::ValidatorBenchmark,
    ])
    .with_datasets(vec![EnvironmentDatasetBinding {
        dataset: spec.dataset.clone(),
        split: Some(String::from("train")),
        mount_path: String::from("/datasets/weather"),
        required: true,
    }])
    .with_tools(vec![EnvironmentToolContract {
        tool_name: String::from("get_weather"),
        interface: EnvironmentToolInterface::NativeFunction,
        description: String::from("Resolve a city into structured weather facts"),
        args_schema: json!({
            "type": "object",
            "required": ["city"],
            "properties": {
                "city": {"type": "string"},
            }
        }),
        result_schema: Some(json!({
            "type": "object",
            "required": ["city", "temperature_c", "condition"],
            "properties": {
                "city": {"type": "string"},
                "temperature_c": {"type": "integer"},
                "condition": {"type": "string"},
            }
        })),
    }])
    .with_rubric_hooks(vec![
        EnvironmentRubricHook {
            rubric_ref: String::from("rubric://weather.correctness"),
            hook_name: String::from("score_correctness"),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(8_500),
        },
        EnvironmentRubricHook {
            rubric_ref: String::from("rubric://weather.tool_use"),
            hook_name: String::from("score_tool_use"),
            score_kind: EnvironmentRubricScoreKind::Binary,
            pass_threshold: Some(10_000),
        },
    ])
    .with_expected_artifacts(vec![
        EnvironmentArtifactExpectation {
            artifact_kind: String::from("weather.json"),
            required: true,
            verification_policy_ref: Some(String::from("verify://weather-artifact")),
        },
        EnvironmentArtifactExpectation {
            artifact_kind: String::from("reward.json"),
            required: true,
            verification_policy_ref: Some(String::from("verify://reward-artifact")),
        },
    ])
    .with_policy_references(vec![
        EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Training,
            policy_ref: String::from("policy://weather/train"),
            required: true,
        },
        EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Reward,
            policy_ref: String::from("policy://weather/reward"),
            required: true,
        },
        EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Verification,
            policy_ref: String::from("policy://weather/verify"),
            required: true,
        },
        EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Benchmark,
            policy_ref: String::from("policy://weather/benchmark"),
            required: true,
        },
    ])
    .with_difficulty(EnvironmentDifficultyMetadata {
        difficulty_tier: String::from("reference"),
        min_agent_level: Some(1),
        tags: vec![
            String::from("tool_call"),
            String::from("structured_output"),
            String::from("weather"),
        ],
    })
    .with_benchmark_profiles(vec![EnvironmentBenchmarkProfile {
        benchmark_profile_ref: String::from("benchmark://weather/reference"),
        runtime_profile_ref: String::from("runtime://weather/reference"),
        verification_posture: EnvironmentVerificationPosture::ValidatorRequired,
        expected_execution_strategy: Some(String::from("multi_turn_dialog")),
    }])
}

fn benchmark_eval_bundle(spec: &AgenticSftRlReferenceProgramSpec) -> DatastreamManifest {
    DatastreamManifest::from_bytes(
        "weather-eval-bundle",
        DatastreamSubjectKind::EvalBundle,
        br#"{"benchmark":"weather-reference","cases":2}"#,
        16,
        DatastreamEncoding::Jsonl,
    )
    .with_dataset_binding(
        spec.dataset
            .datastream_binding("benchmark", "weather-reference"),
    )
    .with_provenance_digest("provenance://weather-reference-benchmark")
}

fn weather_benchmark_package(
    spec: &AgenticSftRlReferenceProgramSpec,
    eval_bundle: psionic_datastream::DatastreamManifestRef,
) -> BenchmarkPackage {
    BenchmarkPackage::new(
        spec.benchmark_package.clone(),
        "Weather Agent Reference Benchmark",
        spec.environment.clone(),
        1,
        BenchmarkAggregationKind::MeanScore,
    )
    .with_dataset(spec.dataset.clone(), Some(String::from("benchmark")))
    .with_eval_bundle(eval_bundle)
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: true,
        require_token_accounting: true,
        require_final_state_capture: true,
        require_execution_strategy: true,
    })
    .with_cases(vec![
        BenchmarkCase::new("case-austin"),
        BenchmarkCase::new("case-chicago"),
    ])
}

fn run_reference_sandbox(
    spec: &AgenticSftRlReferenceProgramSpec,
) -> Result<ReferenceSandboxRun, AgenticSftRlReferenceProgramError> {
    let pool_root = spec.workspace_root.join("sandbox-pool");
    fs::create_dir_all(pool_root.as_path())?;

    let mut service = InMemorySandboxPoolService::default();
    service.create_pool(
        SandboxPoolSpec {
            pool_id: String::from("weather-reference-pool"),
            workspace_root: pool_root,
            target_ready: 1,
            max_sessions: 2,
        },
        sandbox_profile(),
    )?;
    let warm_receipts = service.warm_pool("weather-reference-pool")?;

    let mut acquisition_receipts = Vec::new();
    let mut iteration_receipts = Vec::new();
    let mut outputs = Vec::new();
    for (index, city) in ["Austin", "Chicago"].iter().enumerate() {
        let acquisition = service.acquire_session("weather-reference-pool")?;
        service.stage_artifact(
            acquisition.pool_id.as_str(),
            acquisition.session_id.as_str(),
            acquisition.acquisition_id.as_str(),
            SandboxStageArtifactKind::CommandInput,
            "inputs/request.json",
            json!({ "city": city }).to_string().as_bytes(),
        )?;
        let mut request = SandboxLoopIterationRequest::new(
            ProviderSandboxEntrypointType::InlinePayload,
            "weather-inline",
        );
        request.payload = Some(String::from(
            "#!/bin/sh\nset -eu\ncity=\"$(tr -d '\\n' < inputs/request.json | sed 's/.*\\\"city\\\":\\\"\\([^\\\"]*\\)\\\".*/\\1/')\"\nmkdir -p outputs\ncase \"$city\" in\n  Austin)\n    temperature=\"27\"\n    reward=\"9200\"\n    ;;\n  Chicago)\n    temperature=\"18\"\n    reward=\"8900\"\n    ;;\n  *)\n    temperature=\"20\"\n    reward=\"8600\"\n    ;;\nesac\nprintf '{\"city\":\"%s\",\"temperature_c\":%s,\"condition\":\"sunny\"}' \"$city\" \"$temperature\" > outputs/weather.json\nprintf '{\"reward_bps\":%s}' \"$reward\" > outputs/reward.json\n",
        ));
        request.expected_outputs = vec![
            String::from("outputs/weather.json"),
            String::from("outputs/reward.json"),
        ];
        request.timeout_request_s = 5;
        request.network_request = String::from("host_inherit");
        request.filesystem_request = String::from("host_inherit");
        request.resource_request = ProviderSandboxResourceRequest::default();
        request.payout_reference = Some(format!("pilot-payout-{index}"));
        request.verification_posture = Some(String::from("reference_program"));

        let iteration = service.run_iteration(
            acquisition.pool_id.as_str(),
            acquisition.session_id.as_str(),
            acquisition.acquisition_id.as_str(),
            request,
            ProviderSandboxExecutionControls::default(),
        )?;
        let iteration_snapshot = service.snapshot("weather-reference-pool")?;
        let session = iteration_snapshot
            .sessions
            .iter()
            .find(|session| session.session_id == acquisition.session_id)
            .ok_or(AgenticSftRlReferenceProgramError::MissingSandboxSession)?;
        let weather_relative = String::from("outputs/weather.json");
        let reward_relative = String::from("outputs/reward.json");
        let weather_bytes = fs::read(session.workspace_root.join(weather_relative.as_str()))
            .map_err(
                |_| AgenticSftRlReferenceProgramError::MissingSandboxOutput {
                    relative_path: weather_relative.clone(),
                },
            )?;
        let reward_bytes = fs::read(session.workspace_root.join(reward_relative.as_str()))
            .map_err(
                |_| AgenticSftRlReferenceProgramError::MissingSandboxOutput {
                    relative_path: reward_relative.clone(),
                },
            )?;
        let weather_output =
            serde_json::from_slice::<Value>(weather_bytes.as_slice()).map_err(|error| {
                AgenticSftRlReferenceProgramError::InvalidSandboxOutput {
                    relative_path: weather_relative.clone(),
                    detail: error.to_string(),
                }
            })?;
        let reward_output =
            serde_json::from_slice::<Value>(reward_bytes.as_slice()).map_err(|error| {
                AgenticSftRlReferenceProgramError::InvalidSandboxOutput {
                    relative_path: reward_relative.clone(),
                    detail: error.to_string(),
                }
            })?;
        outputs.push(ReferenceSandboxOutput {
            city: String::from(*city),
            weather_output,
            weather_artifact: EnvironmentArtifactOutput::new(
                "weather.json",
                format!("sandbox://{}/{}", session.session_id, weather_relative),
                weather_bytes.as_slice(),
            ),
            reward_artifact: EnvironmentArtifactOutput::new(
                "reward.json",
                format!("sandbox://{}/{}", session.session_id, reward_relative),
                reward_bytes.as_slice(),
            ),
            reward_bps: reward_output
                .get("reward_bps")
                .and_then(Value::as_i64)
                .map_or(8_600_i32, |value| value as i32),
        });
        acquisition_receipts.push(acquisition);
        iteration_receipts.push(iteration);
    }

    let final_snapshot = service.snapshot("weather-reference-pool")?;

    Ok(ReferenceSandboxRun {
        report: AgenticSftRlReferenceSandboxReport {
            warm_receipts,
            acquisition_receipts,
            iteration_receipts,
            final_snapshot,
        },
        outputs,
    })
}

fn sandbox_profile() -> ProviderSandboxProfile {
    let profile_id = String::from("sandbox.weather.posix.reference");
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
        runtime_binary_path: Some(String::from("/bin/sh")),
        capability_summary: String::from("posix inline payload execution for reference pilot"),
    }
}

fn weather_environment_summary(
    package: &EnvironmentPackageContract,
    session_id: &str,
    task_id: &str,
    output: &ReferenceSandboxOutput,
) -> Result<EnvironmentSessionSummary, AgenticSftRlReferenceProgramError> {
    let mut session = package.clone().open_session(session_id, task_id)?;
    session.begin_turn(EnvironmentTurnInput::new(format!(
        "What is the weather in {}?",
        output.city
    )))?;
    let tool_call = session.request_tool("get_weather", json!({ "city": output.city }))?;
    session.resolve_tool(tool_result(&tool_call, output.weather_output.clone()))?;
    let final_text = format!(
        "In {}, it is {}C and sunny.",
        output.city,
        output
            .weather_output
            .get("temperature_c")
            .and_then(Value::as_i64)
            .unwrap_or(20)
    );
    session.complete_turn(
        final_text.as_str(),
        vec![
            output.weather_artifact.clone(),
            output.reward_artifact.clone(),
        ],
    )?;
    Ok(session.finalize(vec![
        EnvironmentRubricOutcome {
            rubric_ref: String::from("rubric://weather.correctness"),
            score_value: output.reward_bps,
            passed: output.reward_bps >= 8_500,
        },
        EnvironmentRubricOutcome {
            rubric_ref: String::from("rubric://weather.tool_use"),
            score_value: 10_000,
            passed: true,
        },
    ])?)
}

fn tool_result(tool_call: &EnvironmentToolCall, output: Value) -> EnvironmentToolResult {
    EnvironmentToolResult {
        call_id: tool_call.call_id.clone(),
        tool_name: tool_call.tool_name.clone(),
        output,
        succeeded: true,
    }
}

fn training_checkpoint(
    spec: &AgenticSftRlReferenceProgramSpec,
    stage_label: &str,
    writer_node_id: &str,
    step: u64,
    started_at_ms: u64,
) -> TrainingCheckpointReference {
    TrainingCheckpointReference::new(
        spec.checkpoint_family.clone(),
        format!("stream://{stage_label}/{step}"),
        format!("manifest-digest-{stage_label}-{step}"),
        format!("object-digest-{stage_label}-{step}"),
        writer_node_id,
        step,
        format!("cluster-state-digest-{stage_label}-{step}"),
        format!("topology-digest-{stage_label}-{step}"),
        started_at_ms,
    )
    .with_checkpoint_ref(format!("checkpoint://{stage_label}/{step}"))
    .with_step(step)
    .with_durable_at_ms(started_at_ms + 10)
}

fn policy_weight_broadcast_service(
    spec: &AgenticSftRlReferenceProgramSpec,
    target_policy_revision: &PolicyRevision,
    published_at_ms: u64,
) -> Result<InMemoryPolicyWeightBroadcast, AgenticSftRlReferenceProgramError> {
    let shard_a = b"weather-policy-shard-a".repeat(16);
    let shard_b = b"weather-policy-shard-b".repeat(16);
    let assembled_digest = {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(shard_a.as_slice());
        bytes.extend_from_slice(shard_b.as_slice());
        digest_bytes(bytes.as_slice())
    };
    let revision_number = target_policy_revision.revision_number.unwrap_or(0);
    Ok(InMemoryPolicyWeightBroadcast::new(
        vec![
            InMemoryDatastreamServer::new(
                DatastreamManifest::from_bytes(
                    format!("{}-policy-shard-a", spec.run_id),
                    DatastreamSubjectKind::PolicyWeights,
                    shard_a.as_slice(),
                    32,
                    DatastreamEncoding::Safetensors,
                )
                .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
                    spec.policy_family.clone(),
                    revision_number,
                    "shard-a",
                    0,
                    2,
                    assembled_digest.clone(),
                    published_at_ms,
                    10_000,
                )),
                shard_a,
            )?,
            InMemoryDatastreamServer::new(
                DatastreamManifest::from_bytes(
                    format!("{}-policy-shard-b", spec.run_id),
                    DatastreamSubjectKind::PolicyWeights,
                    shard_b.as_slice(),
                    32,
                    DatastreamEncoding::Safetensors,
                )
                .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
                    spec.policy_family.clone(),
                    revision_number,
                    "shard-b",
                    1,
                    2,
                    assembled_digest,
                    published_at_ms,
                    10_000,
                )),
                shard_b,
            )?,
        ],
        published_at_ms + 5,
    )?)
}

fn reference_cluster_state(cluster_id: &ClusterId) -> psionic_cluster::ClusterState {
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships = BTreeMap::from([
        (
            NodeId::new("trainer-a"),
            membership_record(cluster_id, "trainer-a", NodeRole::CoordinatorOnly, 31_000),
        ),
        (
            NodeId::new("worker-b"),
            membership_record(cluster_id, "worker-b", NodeRole::ExecutorOnly, 31_001),
        ),
        (
            NodeId::new("worker-c"),
            membership_record(cluster_id, "worker-c", NodeRole::ExecutorOnly, 31_002),
        ),
    ]);
    psionic_cluster::ClusterState::from_snapshot(snapshot)
}

fn membership_record(
    cluster_id: &ClusterId,
    node_id: &str,
    role: NodeRole,
    port: u16,
) -> ClusterMembershipRecord {
    ClusterMembershipRecord::new(
        ClusterNodeIdentity {
            cluster_id: cluster_id.clone(),
            node_id: NodeId::new(node_id),
            node_epoch: NodeEpoch::initial(),
            role,
            auth_public_key: format!("{node_id}-public-key"),
            attestation: None,
        },
        Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)),
        ClusterMembershipStatus::Ready,
    )
}

fn rollout_artifact(
    worker_id: &str,
    artifact_id: &str,
    environment: EnvironmentPackageKey,
    source_policy_revision: PolicyRevision,
    created_at_ms: u64,
) -> Result<RolloutArtifact, AgenticSftRlReferenceProgramError> {
    Ok(RolloutArtifact::new(
        artifact_id,
        worker_id,
        environment,
        format!("task://weather/{artifact_id}"),
        source_policy_revision,
        vec![
            RolloutSample::new(1, -0.31, 0.8, 0.6),
            RolloutSample::new(2, -0.19, 0.7, 0.5),
            RolloutSample::new(3, -0.11, 0.6, 0.4),
        ],
        crate::RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            format!("proof-digest-{artifact_id}"),
            format!("exec://{artifact_id}"),
        )],
        created_at_ms,
    )?)
}

fn rollout_bundle(
    bundle_id: &str,
    artifact: RolloutArtifact,
    worker_outcome: crate::RolloutWorkerOutcomeReceipt,
    session_summary: &EnvironmentSessionSummary,
) -> RolloutVerificationBundle {
    RolloutVerificationBundle::new(
        bundle_id,
        artifact,
        worker_outcome,
        Some(RolloutBenchmarkObservation {
            observed_runtime_ms: 1_200,
            observed_token_count: 128,
            observed_final_state_digest: Some(session_summary.session_digest.clone()),
            declared_execution_strategy: Some(String::from("multi_turn_dialog")),
        }),
        Some(RolloutBenchmarkExpectation {
            min_runtime_ms: 800,
            max_runtime_ms: 2_000,
            expected_token_count: 128,
            expected_final_state_digest: Some(session_summary.session_digest.clone()),
            expected_execution_strategy: Some(String::from("multi_turn_dialog")),
        }),
    )
}

fn online_eval_run(
    spec: &AgenticSftRlReferenceProgramSpec,
    environment_package: &EnvironmentPackageContract,
    session_summary: &EnvironmentSessionSummary,
    target_policy_revision: &PolicyRevision,
    started_at_ms: u64,
) -> Result<EvalRunState, AgenticSftRlReferenceProgramError> {
    let mut contract = EvalRunContract::new(
        "weather-online-eval-run",
        EvalRunMode::OnlineShadow,
        spec.environment.clone(),
    )
    .with_dataset(spec.dataset.clone(), Some(String::from("online")))
    .with_expected_sample_count(1);
    contract.policy_revision_id = Some(target_policy_revision.revision_id.clone());
    contract.source_ref = Some(String::from("pilot://agentic-session"));
    let mut run = EvalRunState::open(contract)?;
    run.start(started_at_ms)?;
    run.append_sample(eval_sample(
        environment_package,
        session_summary,
        "online-sample-1",
        0,
    )?)?;
    run.finalize(
        started_at_ms + 20,
        vec![EvalArtifact::new(
            "online-eval-report",
            "artifact://online-eval-report",
            b"reference online eval",
        )],
    )?;
    Ok(run)
}

fn benchmark_eval_run(
    spec: &AgenticSftRlReferenceProgramSpec,
    environment_package: &EnvironmentPackageContract,
    benchmark_package: &BenchmarkPackage,
    summaries: [&EnvironmentSessionSummary; 2],
    target_policy_revision: &PolicyRevision,
    started_at_ms: u64,
) -> Result<EvalRunState, AgenticSftRlReferenceProgramError> {
    let mut contract = EvalRunContract::new(
        "weather-benchmark-run",
        EvalRunMode::Benchmark,
        spec.environment.clone(),
    )
    .with_dataset(spec.dataset.clone(), Some(String::from("benchmark")))
    .with_expected_sample_count(2)
    .with_benchmark_package(benchmark_package.key.clone());
    contract.policy_revision_id = Some(target_policy_revision.revision_id.clone());
    contract.source_ref = Some(String::from("pilot://benchmark"));
    let mut run = EvalRunState::open(contract)?;
    run.start(started_at_ms)?;
    run.append_sample(eval_sample(
        environment_package,
        summaries[0],
        "benchmark-sample-1",
        0,
    )?)?;
    run.append_sample(eval_sample(
        environment_package,
        summaries[1],
        "benchmark-sample-2",
        1,
    )?)?;
    run.finalize(
        started_at_ms + 25,
        vec![EvalArtifact::new(
            "benchmark-report",
            "artifact://benchmark-report",
            b"reference benchmark",
        )],
    )?;
    Ok(run)
}

fn benchmark_summary(
    benchmark_package: &BenchmarkPackage,
    benchmark_eval_run: &EvalRunState,
) -> Result<BenchmarkAggregateSummary, AgenticSftRlReferenceProgramError> {
    let mut session = benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    session.record_round(benchmark_eval_run)?;
    Ok(session.finalize()?)
}

fn eval_sample(
    environment_package: &EnvironmentPackageContract,
    session_summary: &EnvironmentSessionSummary,
    sample_id: &str,
    ordinal: u64,
) -> Result<EvalSampleRecord, AgenticSftRlReferenceProgramError> {
    Ok(EvalSampleRecord::from_environment_summary(
        sample_id,
        Some(ordinal),
        Some(format!("input://{sample_id}")),
        Some(format!("output://{sample_id}")),
        Some(format!("expected://{sample_id}")),
        environment_package,
        session_summary,
        Some(EvalVerificationFacts {
            timer_integrity: Some(EvalTimerIntegrityFacts {
                declared_budget_ms: Some(5_000),
                elapsed_ms: 1_250,
                within_budget: true,
            }),
            token_accounting: Some(EvalTokenAccountingFacts::new(96, 32, 128)?),
            final_state: Some(EvalFinalStateCapture {
                session_digest: session_summary.session_digest.clone(),
                output_digest: Some(format!("output-digest://{sample_id}")),
                artifact_digests: session_summary
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.artifact_digest.clone())
                    .collect(),
            }),
            execution_strategy: Some(EvalExecutionStrategyFacts {
                strategy_label: String::from("reference_program"),
                runtime_family: Some(String::from("multi_turn_dialog")),
                scheduler_posture: Some(String::from("deterministic")),
            }),
        }),
    )?)
}

fn training_outcome(
    spec: &AgenticSftRlReferenceProgramSpec,
    target_policy_revision: &PolicyRevision,
    trainer_batch: &TrainerBatch,
    started_at_ms: u64,
) -> Result<TrainingRunOutcome, AgenticSftRlReferenceProgramError> {
    let mut run = FixedBudgetTrainingRun::new(
        format!("{}-trainer", spec.run_id),
        spec.checkpoint_family.clone(),
        TrainingLoopBudget::new(1, 1, 1)?,
        parameter_groups()?,
    )?;
    let reward_scale = trainer_batch.reward_sum / trainer_batch.rollout_count.max(1) as f32;
    let advantage_scale = trainer_batch.advantage_sum / trainer_batch.rollout_count.max(1) as f32;
    let gradients = BTreeMap::from([
        (
            String::from("decoder.weight"),
            TrainingTensorBuffer::from_f32(
                "decoder.weight",
                TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                vec![
                    0.01 + reward_scale * 0.01,
                    0.02,
                    0.03,
                    0.04 + advantage_scale * 0.01,
                ],
            )?,
        ),
        (
            String::from("decoder.bias"),
            TrainingTensorBuffer::from_f32(
                "decoder.bias",
                TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                vec![
                    0.005 + reward_scale * 0.005,
                    0.002 + advantage_scale * 0.005,
                ],
            )?,
        ),
    ]);
    let batch = TrainingGradientBatch::new(
        format!("{}-gradient-batch", trainer_batch.batch_id),
        0.42,
        trainer_batch.rollout_count,
        gradients,
    );
    let _ = target_policy_revision;
    Ok(run.run_fixed_budget([TrainingStepInput::new(
        batch,
        started_at_ms,
        started_at_ms + 20,
    )])?)
}

fn parameter_groups() -> Result<Vec<TrainingParameterGroupState>, AgenticSftRlReferenceProgramError>
{
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

fn operator_view(
    latest_stage_kind: TrainingStageKind,
    window: &TrainingOrchestratorWindow,
    validator_state: &RolloutValidatorState,
    sandbox_snapshot: &SandboxPoolSnapshot,
    iteration_receipts: &[SandboxLoopIterationReceipt],
    online_eval_summary: Option<&psionic_eval::EvalSummary>,
    benchmark_summary: Option<&BenchmarkAggregateSummary>,
    completed_trainer_steps: u64,
) -> AgenticSftRlReferenceOperatorView {
    let validator_accepted_count = validator_state
        .verdicts
        .iter()
        .filter(|verdict| verdict.disposition == crate::ValidatorDisposition::Accepted)
        .count() as u32;
    let validator_normalized_count = validator_state
        .verdicts
        .iter()
        .filter(|verdict| verdict.disposition == crate::ValidatorDisposition::Normalized)
        .count() as u32;
    let validator_rejected_count = validator_state
        .verdicts
        .iter()
        .filter(|verdict| verdict.disposition == crate::ValidatorDisposition::Rejected)
        .count() as u32;
    AgenticSftRlReferenceOperatorView {
        latest_stage_kind,
        training_window_id: window.window_id.clone(),
        accepted_rollout_count: (window.rollout_telemetry.accepted_exact_rollout_count
            + window.rollout_telemetry.accepted_off_policy_rollout_count)
            as u32,
        quarantined_rollout_count: window.rollout_telemetry.quarantined_rollout_count as u32,
        discarded_rollout_count: window.rollout_telemetry.discarded_rollout_count as u32,
        validator_accepted_count,
        validator_normalized_count,
        validator_rejected_count,
        sandbox_ready_sessions: sandbox_snapshot.ready_sessions,
        sandbox_reused_iterations: iteration_receipts
            .iter()
            .filter(|receipt| receipt.reused_workspace)
            .count() as u32,
        online_eval_average_score_bps: online_eval_summary
            .and_then(|summary| summary.average_score_bps),
        benchmark_aggregate_score_bps: benchmark_summary
            .and_then(|summary| summary.aggregate_score_bps),
        completed_trainer_steps,
    }
}

fn digest_json(value: &Value) -> String {
    digest_string(value.to_string().as_str())
}

fn digest_string(value: &str) -> String {
    digest_bytes(value.as_bytes())
}

fn digest_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn training_stage_kind_label(kind: TrainingStageKind) -> &'static str {
    match kind {
        TrainingStageKind::GeneralSft => "general_sft",
        TrainingStageKind::AgenticSft => "agentic_sft",
        TrainingStageKind::Rl => "rl",
    }
}

#[cfg(test)]
mod tests {
    use super::{AgenticSftRlReferenceProgramSpec, run_agentic_sft_rl_reference_program};

    #[test]
    fn agentic_sft_rl_reference_program_is_end_to_end_machine_legible()
    -> Result<(), Box<dyn std::error::Error>> {
        let unique_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_millis(),
            Err(_) => 0,
        };
        let workspace_root = std::env::temp_dir().join(format!(
            "openagents-psionic-agentic-sft-rl-reference-{}-{}",
            std::process::id(),
            unique_ms
        ));
        let spec = AgenticSftRlReferenceProgramSpec::weather_default(workspace_root.clone());
        let report = run_agentic_sft_rl_reference_program(&spec)?;

        assert_eq!(
            report.stage_program.current_stage().map(|stage| stage.kind),
            Some(crate::TrainingStageKind::Rl)
        );
        assert_eq!(report.stage_program.completions.len(), 2);
        assert_eq!(report.stage_program.promotions.len(), 2);
        assert_eq!(report.sandbox.iteration_receipts.len(), 2);
        assert!(
            report
                .sandbox
                .iteration_receipts
                .iter()
                .any(|receipt| receipt.reused_workspace)
        );
        assert_eq!(report.worker_outcomes.len(), 2);
        assert!(report.worker_outcomes.iter().all(|receipt| matches!(
            receipt.outcome,
            crate::RolloutWorkerOutcomeKind::UploadedAcceptedExact
                | crate::RolloutWorkerOutcomeKind::UploadedAcceptedOffPolicy
        )));
        assert_eq!(report.validator_state.verdicts.len(), 2);
        assert!(
            report
                .validator_state
                .verdicts
                .iter()
                .all(|verdict| verdict.disposition == crate::ValidatorDisposition::Accepted)
        );
        assert_eq!(report.training_outcome.summary.completed_steps, 1);
        assert!(
            report
                .online_eval_run
                .summary
                .as_ref()
                .and_then(|summary| summary.average_score_bps)
                .is_some()
        );
        assert!(report.benchmark_summary.aggregate_score_bps.is_some());
        assert_eq!(
            report.lineage.dataset_storage_key,
            spec.dataset.storage_key()
        );
        assert_eq!(report.operator_view.accepted_rollout_count, 2);
        assert_eq!(report.operator_view.validator_accepted_count, 2);
        assert_eq!(report.operator_view.completed_trainer_steps, 1);
        assert!(
            report
                .operator_view
                .summary_lines()
                .iter()
                .any(|line| line.contains("latest stage: rl"))
        );

        if workspace_root.exists() {
            std::fs::remove_dir_all(workspace_root)?;
        }
        Ok(())
    }
}
