use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use ed25519_dalek::SigningKey;
use psionic_cluster::{
    AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
    ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
    ClusterSnapshot, ClusterStabilityPosture, ClusterState, NodeEpoch, NodeId, NodeRole,
};
use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
use psionic_environments::EnvironmentPackageKey;
use psionic_eval::{
    BenchmarkAggregateSummary, BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode,
    BenchmarkPackage, BenchmarkPackageKey, EvalRunContract, EvalRunMode, EvalRunState,
    EvalSampleRecord, EvalSampleStatus,
};
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterArtifactRetentionPolicy, AdapterArtifactStorageError, AdapterArtifactStorageState,
    AdapterAggregationRule,
    AdapterClusterCoordinationError, AdapterClusterMembershipReceipt,
    AdapterClusterWindowPlanReceipt, AdapterContributionArtifactDisposition,
    AdapterContributionArtifactReceipt, AdapterContributionExecutionSummary,
    AdapterContributionProgress, AdapterContributionProvenanceBundle,
    AdapterContributionReplayReceipt, AdapterContributionSecurityController,
    AdapterContributionSecurityError, AdapterContributionSecurityPolicy,
    AdapterContributionSecurityReceipt, AdapterContributionUploadLocator,
    AdapterContributionValidationBundle, AdapterContributionValidatorPolicy,
    AdapterContributionValidatorState, AdapterDatasetSliceIdentity, AdapterPolicyAggregator,
    AdapterPolicyPromotionDisposition, AdapterPolicyPromotionReceipt, AdapterTargetIdentity,
    AdapterTrainingClusterCoordinator, AdapterWindowCandidateEvaluation,
    AdapterWindowCheckpointReceipt, AdapterWindowContractError, AdapterWindowScoreSummary,
    AdapterWorkerIdentity, AdapterWorkerProtocolError, AdapterWorkerProtocolPolicy,
    AdapterWorkerProtocolState, AdapterWorkerTrustClass, CheckpointPointer,
    CheckpointRecoveryError, CheckpointScopeBinding, CheckpointScopeKind,
    OPEN_ADAPTER_CUDA_BACKEND_LABEL, OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY,
    OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT, PolicyRevision, TrainingRunGraphError,
    TrainingWindowStatus,
};

const APPLE_ADAPTER_BACKEND_LABEL: &str = "apple.foundation_models.adapter_train";
const APPLE_ADAPTER_FAMILY: &str = "apple.foundation_models";
const APPLE_ADAPTER_FORMAT: &str = "apple.fmadapter";
const GIB_BYTES: u64 = 1024 * 1024 * 1024;

/// Stable workload family used by the decentralized adapter QA reference program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedAdapterReferenceFamily {
    /// Apple Foundation Models adapters under the decentralized control plane.
    AppleFoundationModels,
    /// Open `safetensors` LM-head LoRA adapters for GPT-OSS/CUDA participants.
    OpenGptOssLmHead,
}

impl DecentralizedAdapterReferenceFamily {
    /// Returns a short label for operator logs or receipts.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AppleFoundationModels => "apple_foundation_models",
            Self::OpenGptOssLmHead => "open_gpt_oss_lm_head",
        }
    }
}

/// Input contract for the decentralized adapter QA reference program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedAdapterReferenceProgramSpec {
    /// Reference workload family exercised by the program.
    pub family: DecentralizedAdapterReferenceFamily,
    /// Stable training-run identifier.
    pub run_id: String,
    /// Stable checkpoint family used by the coordinator.
    pub checkpoint_family: String,
    /// Stable policy family used by the coordinator.
    pub policy_family: String,
    /// Environment identity carried into eval truth.
    pub environment: EnvironmentPackageKey,
    /// Deterministic base timestamp used by the reference run.
    pub base_time_ms: u64,
}

impl DecentralizedAdapterReferenceProgramSpec {
    /// Returns the canonical Apple QA reference spec.
    #[must_use]
    pub fn apple_default() -> Self {
        Self {
            family: DecentralizedAdapterReferenceFamily::AppleFoundationModels,
            run_id: "adapter-reference.apple.explainer".to_string(),
            checkpoint_family: "adapter.reference.apple".to_string(),
            policy_family: "adapter.reference.apple".to_string(),
            environment: EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
            base_time_ms: 1_763_100_000_000,
        }
    }

    /// Returns the canonical open-backend QA reference spec.
    #[must_use]
    pub fn open_default() -> Self {
        Self {
            family: DecentralizedAdapterReferenceFamily::OpenGptOssLmHead,
            run_id: "adapter-reference.open_gpt_oss_lm_head".to_string(),
            checkpoint_family: "adapter.reference.open".to_string(),
            policy_family: "adapter.reference.open".to_string(),
            environment: EnvironmentPackageKey::new("oa.open.adapter", "2026.03"),
            base_time_ms: 1_763_100_100_000,
        }
    }
}

/// Simple latency envelope carried by the QA reference program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedAdapterLatencyEnvelope {
    /// Time from window activation to validator sealing and score emission.
    pub seal_latency_ms: u64,
    /// Time from first submission to replay receipt completion.
    pub replay_latency_ms: u64,
    /// Time from validator score emission to policy promotion.
    pub promotion_latency_ms: u64,
}

/// Condensed operator-facing summary for the decentralized adapter reference run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedAdapterReferenceOperatorView {
    /// Reference workload family exercised by the run.
    pub family: DecentralizedAdapterReferenceFamily,
    /// Stable first-window identifier.
    pub first_window_id: String,
    /// Stable second-window identifier.
    pub second_window_id: String,
    /// Contributors selected for the first window.
    pub first_window_selected_node_ids: Vec<String>,
    /// Contributors selected for the second window after churn.
    pub second_window_selected_node_ids: Vec<String>,
    /// Accepted contribution count across both windows.
    pub accepted_contribution_count: u32,
    /// Replay-checked contribution count across both windows.
    pub replay_checked_contribution_count: u32,
    /// Policy revision identifiers promoted by the run.
    pub promoted_policy_revision_ids: Vec<String>,
    /// Aggregate latency envelope for the reference run.
    pub latency_envelope: DecentralizedAdapterLatencyEnvelope,
}

impl DecentralizedAdapterReferenceOperatorView {
    /// Returns compact summary lines for CLI or test logs.
    #[must_use]
    pub fn summary_lines(&self) -> Vec<String> {
        vec![
            format!("family: {}", self.family.label()),
            format!(
                "windows: {} -> {}",
                self.first_window_id, self.second_window_id
            ),
            format!(
                "selected contributors: [{}] then [{}]",
                self.first_window_selected_node_ids.join(","),
                self.second_window_selected_node_ids.join(",")
            ),
            format!(
                "accepted contributions: {}, replay checked: {}",
                self.accepted_contribution_count, self.replay_checked_contribution_count
            ),
            format!(
                "promoted revisions: {}",
                self.promoted_policy_revision_ids.join(", ")
            ),
            format!(
                "latency envelope ms: seal={} replay={} promotion={}",
                self.latency_envelope.seal_latency_ms,
                self.latency_envelope.replay_latency_ms,
                self.latency_envelope.promotion_latency_ms
            ),
        ]
    }
}

/// Full typed report for the decentralized adapter QA reference run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecentralizedAdapterReferenceProgramReport {
    /// Input spec that produced this report.
    pub spec: DecentralizedAdapterReferenceProgramSpec,
    /// Initial cluster-backed membership receipt.
    pub initial_membership: AdapterClusterMembershipReceipt,
    /// Membership receipt after contributor churn between windows.
    pub churned_membership: AdapterClusterMembershipReceipt,
    /// First window plan receipt.
    pub first_window_plan: AdapterClusterWindowPlanReceipt,
    /// Second window plan receipt.
    pub second_window_plan: AdapterClusterWindowPlanReceipt,
    /// First window validator summary.
    pub first_window_summary: AdapterWindowScoreSummary,
    /// Second window validator summary.
    pub second_window_summary: AdapterWindowScoreSummary,
    /// First window promotion receipt.
    pub first_promotion: AdapterPolicyPromotionReceipt,
    /// Second window promotion receipt.
    pub second_promotion: AdapterPolicyPromotionReceipt,
    /// Artifact receipts across both windows.
    pub artifact_receipts: Vec<AdapterContributionArtifactReceipt>,
    /// Security receipts across both windows.
    pub security_receipts: Vec<AdapterContributionSecurityReceipt>,
    /// Window checkpoint receipts emitted by artifact storage.
    pub window_checkpoint_receipts: Vec<AdapterWindowCheckpointReceipt>,
    /// Condensed operator-facing summary.
    pub operator_view: DecentralizedAdapterReferenceOperatorView,
}

/// Failure returned by the decentralized adapter QA reference program.
#[derive(Debug, Error)]
pub enum DecentralizedAdapterReferenceProgramError {
    /// Cluster/window coordination failure.
    #[error(transparent)]
    Cluster(#[from] AdapterClusterCoordinationError),
    /// Window-contract failure.
    #[error(transparent)]
    WindowContract(#[from] AdapterWindowContractError),
    /// Worker-protocol failure.
    #[error(transparent)]
    WorkerProtocol(#[from] AdapterWorkerProtocolError),
    /// Artifact-staging failure.
    #[error(transparent)]
    ArtifactStorage(#[from] AdapterArtifactStorageError),
    /// Provenance/security failure.
    #[error(transparent)]
    Security(#[from] AdapterContributionSecurityError),
    /// Validator or window-score failure.
    #[error(transparent)]
    Validation(#[from] crate::AdapterValidationError),
    /// Aggregation or promotion failure.
    #[error(transparent)]
    Aggregation(#[from] crate::AdapterAggregationError),
    /// Training run-graph construction or transition failure.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// Checkpoint pointer construction or validation failure.
    #[error(transparent)]
    Checkpoint(#[from] CheckpointRecoveryError),
    /// Eval-run construction or finalization failure.
    #[error(transparent)]
    Eval(#[from] psionic_eval::EvalRuntimeError),
    /// One promotion did not actually advance policy state.
    #[error("reference program window `{window_id}` did not promote a new policy revision")]
    PromotionHeld {
        /// Stable window identifier.
        window_id: String,
    },
}

/// Runs the canonical decentralized adapter QA reference program.
pub fn run_decentralized_adapter_reference_program(
    spec: &DecentralizedAdapterReferenceProgramSpec,
) -> Result<DecentralizedAdapterReferenceProgramReport, DecentralizedAdapterReferenceProgramError> {
    let family = family_config(spec.family)?;
    let initial_cluster = cluster_state(
        family.backend_label,
        &[
            (
                "trainer-a",
                NodeRole::CoordinatorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-b",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-c",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
        ],
        &[
            ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-b", 22, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-c", 20, 1, ClusterBackendReadinessStatus::Ready),
        ],
    );

    let mut coordinator = AdapterTrainingClusterCoordinator::new(
        crate::TrainingRunState::new(
            spec.run_id.clone(),
            "adapter-sft",
            initial_cluster.cluster_id().as_str(),
            spec.policy_family.clone(),
            spec.environment.clone(),
        )?,
        family.adapter_target.clone(),
        PolicyRevision::new(
            spec.policy_family.clone(),
            "policy-r1",
            "policy-digest-r1",
            spec.base_time_ms,
        )
        .with_revision_number(1)
        .with_checkpoint(reference_checkpoint(
            spec.policy_family.as_str(),
            spec.run_id.as_str(),
            "policy-r1",
            1,
            spec.base_time_ms,
        )),
        CheckpointPointer::new(
            CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-reference-initial"),
            spec.policy_family.clone(),
            reference_checkpoint(
                spec.policy_family.as_str(),
                spec.run_id.as_str(),
                "policy-r1",
                1,
                spec.base_time_ms,
            )
            .with_durable_at_ms(spec.base_time_ms + 1),
            "manifest-digest-r1",
            spec.base_time_ms + 1,
        )?,
        family.capability_policy.clone(),
    );

    let initial_membership = coordinator
        .observe_cluster_state(&initial_cluster, spec.base_time_ms + 10)?
        .clone();
    let first_window = coordinator.plan_next_window(
        vec![
            AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-a",
                "slice-digest-a",
            )?,
            AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-b",
                "slice-digest-b",
            )?,
        ],
        2,
        spec.base_time_ms + 20,
    )?;
    coordinator.activate_current_window(spec.base_time_ms + 30)?;

    let mut storage = AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
    let mut security =
        AdapterContributionSecurityController::new(AdapterContributionSecurityPolicy::default());
    let mut validator = AdapterContributionValidatorState::new(family.validator_policy.clone());
    let mut aggregator = AdapterPolicyAggregator::new(AdapterAggregationRule::default());

    let first_execution = execute_window(
        spec,
        &family,
        &mut coordinator,
        &first_window,
        &mut storage,
        &mut security,
        &mut validator,
        &mut aggregator,
        1,
    )?;

    let churned_cluster = cluster_state(
        family.backend_label,
        &[
            (
                "trainer-a",
                NodeRole::CoordinatorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-b",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
            (
                "worker-d",
                NodeRole::ExecutorOnly,
                ClusterMembershipStatus::Ready,
            ),
        ],
        &[
            ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-b", 22, 1, ClusterBackendReadinessStatus::Ready),
            ("worker-d", 26, 1, ClusterBackendReadinessStatus::Ready),
        ],
    );
    let churned_membership = coordinator
        .observe_cluster_state(&churned_cluster, spec.base_time_ms + 200)?
        .clone();
    let second_window = coordinator.plan_next_window(
        vec![
            AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-c",
                "slice-digest-c",
            )?,
            AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-d",
                "slice-digest-d",
            )?,
        ],
        2,
        spec.base_time_ms + 210,
    )?;
    coordinator.activate_current_window(spec.base_time_ms + 220)?;

    let second_execution = execute_window(
        spec,
        &family,
        &mut coordinator,
        &second_window,
        &mut storage,
        &mut security,
        &mut validator,
        &mut aggregator,
        2,
    )?;

    let operator_view = DecentralizedAdapterReferenceOperatorView {
        family: spec.family,
        first_window_id: first_window.plan.window_id.clone(),
        second_window_id: second_window.plan.window_id.clone(),
        first_window_selected_node_ids: first_window.plan.selected_node_ids.clone(),
        second_window_selected_node_ids: second_window.plan.selected_node_ids.clone(),
        accepted_contribution_count: first_execution.summary.accepted_contributions
            + second_execution.summary.accepted_contributions,
        replay_checked_contribution_count: first_execution.summary.replay_checked_contributions
            + second_execution.summary.replay_checked_contributions,
        promoted_policy_revision_ids: vec![
            first_execution
                .promotion
                .output_policy_revision
                .as_ref()
                .map(|revision| revision.revision_id.clone())
                .unwrap_or_else(|| String::from("held")),
            second_execution
                .promotion
                .output_policy_revision
                .as_ref()
                .map(|revision| revision.revision_id.clone())
                .unwrap_or_else(|| String::from("held")),
        ],
        latency_envelope: DecentralizedAdapterLatencyEnvelope {
            seal_latency_ms: first_execution
                .latency_envelope
                .seal_latency_ms
                .max(second_execution.latency_envelope.seal_latency_ms),
            replay_latency_ms: first_execution
                .latency_envelope
                .replay_latency_ms
                .max(second_execution.latency_envelope.replay_latency_ms),
            promotion_latency_ms: first_execution
                .latency_envelope
                .promotion_latency_ms
                .max(second_execution.latency_envelope.promotion_latency_ms),
        },
    };

    Ok(DecentralizedAdapterReferenceProgramReport {
        spec: spec.clone(),
        initial_membership,
        churned_membership,
        first_window_plan: first_window.plan,
        second_window_plan: second_window.plan,
        first_window_summary: first_execution.summary,
        second_window_summary: second_execution.summary,
        first_promotion: first_execution.promotion,
        second_promotion: second_execution.promotion,
        artifact_receipts: storage.contribution_artifacts.clone(),
        security_receipts: security.receipts.clone(),
        window_checkpoint_receipts: storage.window_checkpoints.clone(),
        operator_view,
    })
}

#[derive(Clone)]
struct ReferenceFamilyConfig {
    adapter_target: AdapterTargetIdentity,
    capability_policy: crate::AdapterContributorCapabilityPolicy,
    validator_policy: AdapterContributionValidatorPolicy,
    backend_label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExecutedReferenceWindow {
    summary: AdapterWindowScoreSummary,
    promotion: AdapterPolicyPromotionReceipt,
    latency_envelope: DecentralizedAdapterLatencyEnvelope,
}

fn family_config(
    family: DecentralizedAdapterReferenceFamily,
) -> Result<ReferenceFamilyConfig, AdapterWindowContractError> {
    match family {
        DecentralizedAdapterReferenceFamily::AppleFoundationModels => Ok(ReferenceFamilyConfig {
            adapter_target: AdapterTargetIdentity::new(
                "apple.explainer.adapter",
                APPLE_ADAPTER_FAMILY,
                "apple://foundation-model/reference",
                APPLE_ADAPTER_FORMAT,
            )?,
            capability_policy: crate::AdapterContributorCapabilityPolicy {
                backend_label: APPLE_ADAPTER_BACKEND_LABEL.to_string(),
                minimum_free_memory_bytes: 12 * GIB_BYTES,
                require_accelerator: true,
                allow_degraded_backend: false,
                allow_flaky_nodes: false,
            },
            validator_policy: AdapterContributionValidatorPolicy {
                validator_policy_id: "validator.apple.reference".to_string(),
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            },
            backend_label: APPLE_ADAPTER_BACKEND_LABEL,
        }),
        DecentralizedAdapterReferenceFamily::OpenGptOssLmHead => Ok(ReferenceFamilyConfig {
            adapter_target: AdapterTargetIdentity::new(
                "gpt_oss.explainer.lm_head",
                OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY,
                "gpt-oss-20b@2026-03",
                OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT,
            )?,
            capability_policy: crate::AdapterContributorCapabilityPolicy {
                backend_label: OPEN_ADAPTER_CUDA_BACKEND_LABEL.to_string(),
                minimum_free_memory_bytes: 10 * GIB_BYTES,
                require_accelerator: true,
                allow_degraded_backend: false,
                allow_flaky_nodes: false,
            },
            validator_policy: AdapterContributionValidatorPolicy {
                validator_policy_id: "validator.open_adapter.reference".to_string(),
                replay_sample_bps: 10_000,
                ..AdapterContributionValidatorPolicy::default()
            },
            backend_label: OPEN_ADAPTER_CUDA_BACKEND_LABEL,
        }),
    }
}

fn execute_window(
    spec: &DecentralizedAdapterReferenceProgramSpec,
    _family: &ReferenceFamilyConfig,
    coordinator: &mut AdapterTrainingClusterCoordinator,
    record: &crate::AdapterClusterWindowRecord,
    storage: &mut AdapterArtifactStorageState,
    security: &mut AdapterContributionSecurityController,
    validator: &mut AdapterContributionValidatorState,
    aggregator: &mut AdapterPolicyAggregator,
    window_index: u64,
) -> Result<ExecutedReferenceWindow, DecentralizedAdapterReferenceProgramError> {
    let active_at_ms = spec.base_time_ms + window_index * 100 + 30;
    let mut protocol = AdapterWorkerProtocolState::from_window_record(
        record,
        AdapterWorkerProtocolPolicy::default(),
    );
    protocol.activate_window()?;
    let mut bundles = Vec::new();
    let mut first_submission_at_ms = None;
    let mut first_replay_at_ms = None;

    for (assignment_index, assignment) in protocol.assignments.clone().iter().enumerate() {
        let signing_key = signing_key_for(
            record.plan.window_id.as_str(),
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
        );
        let session_id = format!("{}-session-{}", assignment.worker_id, window_index);
        let identity = AdapterWorkerIdentity::new(
            assignment.worker_id.clone(),
            session_id.clone(),
            AdapterWorkerTrustClass::SemiTrustedContributor,
            format!("reference:{}", assignment.worker_id),
        )
        .with_submission_signing_public_key_hex(hex::encode(
            signing_key.verifying_key().to_bytes(),
        ));
        protocol.record_heartbeat(identity.clone(), None, None, active_at_ms + 1)?;
        let claim = protocol.claim_assignment(
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
            active_at_ms + 2,
        )?;
        protocol.acknowledge_assignment(
            assignment.worker_id.as_str(),
            session_id.as_str(),
            claim.claim_id.as_str(),
            active_at_ms + 3,
        )?;
        protocol.record_heartbeat(
            identity.clone(),
            Some(claim.claim_id.as_str()),
            Some(AdapterContributionProgress {
                completed_steps: 4,
                processed_samples: 16,
            }),
            active_at_ms + 4,
        )?;

        let execution_summary = AdapterContributionExecutionSummary::new(
            active_at_ms + 5 + assignment_index as u64,
            active_at_ms + 10 + assignment_index as u64,
            8 + assignment_index as u32,
            32 + assignment_index as u32,
            Some(180 + assignment_index as u32 * 10),
            format!(
                "adapter-delta:{}:{}:{}",
                record.plan.window_id, assignment.worker_id, assignment.assignment_id
            ),
        )?;
        let payload = contribution_payload(
            spec.family,
            record.plan.window_id.as_str(),
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
        );
        let chunk_bytes = 8;
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            payload.as_slice(),
            chunk_bytes,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            format!(
                "{}/artifact-{}",
                assignment.upload_expectation.upload_reference_prefix, window_index
            ),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let submission = protocol.submit_contribution(
            claim.claim_id.as_str(),
            assignment.worker_id.as_str(),
            session_id.as_str(),
            assignment.source_policy_revision.revision_id.as_str(),
            assignment.source_checkpoint_pointer.pointer_digest.as_str(),
            execution_summary.clone(),
            upload.clone(),
            active_at_ms + 12 + assignment_index as u64,
        )?;
        first_submission_at_ms.get_or_insert(submission.observed_at_ms);

        let cursor = storage.start_contribution_upload(
            assignment,
            upload,
            payload.as_slice(),
            chunk_bytes,
            assignment.worker_id.clone(),
            active_at_ms + 13 + assignment_index as u64,
        )?;
        for chunk in payload.chunks(chunk_bytes) {
            let _ = storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
        }
        let artifact = storage.complete_contribution_upload(
            cursor.upload_id.as_str(),
            active_at_ms + 20 + assignment_index as u64,
        )?;
        let provenance = AdapterContributionProvenanceBundle::new_signed(
            assignment,
            &claim,
            &identity,
            &submission,
            &artifact,
            &signing_key,
            active_at_ms + 21 + assignment_index as u64,
        );
        let security_receipt = security.assess_submission(
            &protocol,
            &artifact,
            &submission,
            provenance.clone(),
            active_at_ms + 22 + assignment_index as u64,
        )?;
        storage.set_contribution_disposition(
            artifact.contribution_id.as_str(),
            AdapterContributionArtifactDisposition::Accepted,
            active_at_ms + 23 + assignment_index as u64,
        )?;
        let replay_at_ms = active_at_ms + 24 + assignment_index as u64;
        first_replay_at_ms.get_or_insert(replay_at_ms);
        let replay = AdapterContributionReplayReceipt::new(
            submission.contribution_id.clone(),
            execution_summary.adapter_delta_digest.clone(),
            execution_summary.adapter_delta_digest.clone(),
            replay_at_ms,
        );
        bundles.push(AdapterContributionValidationBundle::new(
            submission,
            artifact,
            provenance,
            security_receipt,
            Some(replay),
        ));
    }

    let current_revision_number = coordinator
        .current_policy_revision
        .revision_number
        .unwrap_or(1);
    let candidate_policy = PolicyRevision::new(
        spec.policy_family.clone(),
        format!("policy-r{}", current_revision_number + 1),
        format!("policy-digest-r{}", current_revision_number + 1),
        active_at_ms + 90,
    )
    .with_revision_number(current_revision_number + 1)
    .with_checkpoint(reference_checkpoint(
        spec.policy_family.as_str(),
        spec.run_id.as_str(),
        format!("policy-r{}", current_revision_number + 1).as_str(),
        current_revision_number + 1,
        active_at_ms + 90,
    ));
    let candidate = AdapterWindowCandidateEvaluation {
        candidate_policy_revision: Some(candidate_policy),
        held_out_eval: Some(finalized_eval_run(
            format!("heldout-{}-{}", spec.family.label(), window_index).as_str(),
            EvalRunMode::OfflineHeldOut,
            spec.environment.clone(),
            1,
            9_100,
        )?),
        benchmark_summary: Some(benchmark_summary(spec.environment.clone(), 9_400)?),
        runtime_smoke_eval: if spec.family
            == DecentralizedAdapterReferenceFamily::AppleFoundationModels
        {
            Some(finalized_eval_run(
                format!("runtime-smoke-{}-{}", spec.family.label(), window_index).as_str(),
                EvalRunMode::OnlineShadow,
                spec.environment.clone(),
                1,
                10_000,
            )?)
        } else {
            None
        },
    };
    let summary = validator.validate_window(
        &mut protocol.window,
        bundles.clone(),
        Some(&candidate),
        active_at_ms + 100,
    )?;
    *coordinator.current_window_mut()? = protocol.window.clone();
    let promotion = aggregator.promote_current_window(
        coordinator,
        &summary,
        bundles.clone(),
        active_at_ms + 120,
        active_at_ms + 130,
    )?;
    if promotion.promotion_disposition != AdapterPolicyPromotionDisposition::Promoted {
        return Err(DecentralizedAdapterReferenceProgramError::PromotionHeld {
            window_id: summary.window_id.clone(),
        });
    }
    let checkpoint_payload = promotion
        .aggregated_delta_digest
        .clone()
        .unwrap_or_else(|| String::from("no-aggregate"))
        .into_bytes();
    let policy_revision = promotion
        .output_policy_revision
        .as_ref()
        .expect("promoted revision present");
    let _ = storage.promote_window_checkpoint(
        summary.window_id.as_str(),
        policy_revision,
        checkpoint_payload.as_slice(),
        8,
        "validator-a",
        active_at_ms + 125,
    )?;

    Ok(ExecutedReferenceWindow {
        summary,
        promotion,
        latency_envelope: DecentralizedAdapterLatencyEnvelope {
            seal_latency_ms: (active_at_ms + 100).saturating_sub(active_at_ms),
            replay_latency_ms: first_replay_at_ms
                .unwrap_or(active_at_ms)
                .saturating_sub(first_submission_at_ms.unwrap_or(active_at_ms)),
            promotion_latency_ms: (active_at_ms + 120).saturating_sub(active_at_ms + 100),
        },
    })
}

fn cluster_state(
    backend_label: &str,
    memberships: &[(&str, NodeRole, ClusterMembershipStatus)],
    telemetry: &[(&str, u64, u16, ClusterBackendReadinessStatus)],
) -> ClusterState {
    let cluster_id = ClusterId::new(
        &ClusterNamespace::new("adapter-reference-program"),
        &AdmissionToken::new("shared-secret"),
    );
    let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
    snapshot.memberships = memberships
        .iter()
        .map(|(node_id, role, status)| {
            (
                NodeId::new(*node_id),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new(*node_id),
                        node_epoch: NodeEpoch::initial(),
                        role: *role,
                        auth_public_key: format!("{node_id}-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 20_000)),
                    *status,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    snapshot.telemetry = telemetry
        .iter()
        .map(
            |(node_id, free_memory_gib, accelerator_count, backend_status)| {
                (
                    NodeId::new(*node_id),
                    ClusterNodeTelemetry::new(NodeId::new(*node_id))
                        .with_memory(
                            Some(free_memory_gib.saturating_mul(GIB_BYTES)),
                            Some(free_memory_gib.saturating_mul(GIB_BYTES)),
                        )
                        .with_accelerator_count(*accelerator_count)
                        .with_backend_readiness(backend_label, *backend_status)
                        .with_stability_posture(ClusterStabilityPosture::Stable),
                )
            },
        )
        .collect::<BTreeMap<_, _>>();
    ClusterState::from_snapshot(snapshot)
}

fn reference_checkpoint(
    policy_family: &str,
    run_id: &str,
    revision_id: &str,
    revision_number: u64,
    started_at_ms: u64,
) -> TrainingCheckpointReference {
    TrainingCheckpointReference::new(
        policy_family.to_string(),
        format!("stream://{run_id}/{revision_id}"),
        format!("manifest://{run_id}/{revision_id}"),
        format!("object://{run_id}/{revision_id}"),
        "trainer-a",
        revision_number,
        format!("cluster-digest:{run_id}"),
        format!("topology-digest:{run_id}"),
        started_at_ms,
    )
    .with_checkpoint_ref(format!("{run_id}/{revision_id}"))
    .with_step(revision_number)
}

fn finalized_eval_run(
    eval_run_id: &str,
    mode: EvalRunMode,
    environment: EnvironmentPackageKey,
    expected_sample_count: u64,
    score_bps: u32,
) -> Result<EvalRunState, psionic_eval::EvalRuntimeError> {
    let mut run = EvalRunState::open(
        EvalRunContract::new(eval_run_id, mode, environment)
            .with_expected_sample_count(expected_sample_count),
    )?;
    run.start(10_000)?;
    run.append_sample(EvalSampleRecord {
        sample_id: String::from("sample-1"),
        ordinal: Some(1),
        environment: run.contract.environment.clone(),
        status: EvalSampleStatus::Passed,
        input_ref: Some(String::from("input://1")),
        output_ref: Some(String::from("output://1")),
        expected_output_ref: Some(String::from("expected://1")),
        score_bps: Some(score_bps),
        metrics: Vec::new(),
        artifacts: Vec::new(),
        error_reason: None,
        verification: None,
        session_digest: None,
        metadata: BTreeMap::new(),
    })?;
    run.finalize(10_010, Vec::new())?;
    Ok(run)
}

fn benchmark_summary(
    environment: EnvironmentPackageKey,
    score_bps: u32,
) -> Result<BenchmarkAggregateSummary, psionic_eval::EvalRuntimeError> {
    let package = BenchmarkPackage::new(
        BenchmarkPackageKey::new("adapter.reference.benchmark", "2026.03"),
        "Adapter Reference Benchmark",
        environment.clone(),
        1,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_cases(vec![BenchmarkCase::new("case-1")]);
    let mut round = EvalRunState::open(
        EvalRunContract::new("benchmark-round-1", EvalRunMode::Benchmark, environment)
            .with_expected_sample_count(1)
            .with_benchmark_package(package.key.clone()),
    )?;
    round.start(10_000)?;
    round.append_sample(EvalSampleRecord {
        sample_id: String::from("sample-1"),
        ordinal: Some(1),
        environment: round.contract.environment.clone(),
        status: EvalSampleStatus::Passed,
        input_ref: Some(String::from("input://1")),
        output_ref: Some(String::from("output://1")),
        expected_output_ref: Some(String::from("expected://1")),
        score_bps: Some(score_bps),
        metrics: Vec::new(),
        artifacts: Vec::new(),
        error_reason: None,
        verification: None,
        session_digest: None,
        metadata: BTreeMap::new(),
    })?;
    round.finalize(10_010, Vec::new())?;
    let mut execution = package.open_execution(BenchmarkExecutionMode::Validator)?;
    execution.record_round(&round)?;
    execution.finalize()
}

fn signing_key_for(window_id: &str, worker_id: &str, assignment_id: &str) -> SigningKey {
    let digest = Sha256::digest(format!("{window_id}|{worker_id}|{assignment_id}").as_bytes());
    let key_bytes: [u8; 32] = digest.into();
    SigningKey::from_bytes(&key_bytes)
}

fn contribution_payload(
    family: DecentralizedAdapterReferenceFamily,
    window_id: &str,
    worker_id: &str,
    assignment_id: &str,
) -> Vec<u8> {
    match family {
        DecentralizedAdapterReferenceFamily::AppleFoundationModels => format!(
            "apple-fmadapter-reference|window={window_id}|worker={worker_id}|assignment={assignment_id}"
        )
        .into_bytes(),
        DecentralizedAdapterReferenceFamily::OpenGptOssLmHead => format!(
            "safetensors-reference|family={OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY}|window={window_id}|worker={worker_id}|assignment={assignment_id}"
        )
        .into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decentralized_adapter_reference_program_runs_for_apple_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = run_decentralized_adapter_reference_program(
            &DecentralizedAdapterReferenceProgramSpec::apple_default(),
        )?;
        assert_eq!(
            report.first_window_plan.selected_node_ids,
            vec!["worker-b", "worker-c"]
        );
        assert_eq!(
            report.second_window_plan.selected_node_ids,
            vec!["worker-d", "worker-b"]
        );
        assert_eq!(report.first_window_summary.accepted_contributions, 2);
        assert_eq!(report.second_window_summary.accepted_contributions, 2);
        assert_eq!(
            report.first_promotion.promotion_disposition,
            AdapterPolicyPromotionDisposition::Promoted
        );
        assert!(
            report
                .operator_view
                .summary_lines()
                .iter()
                .any(|line| line.contains("family: apple_foundation_models"))
        );
        Ok(())
    }

    #[test]
    fn decentralized_adapter_reference_program_runs_for_open_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = run_decentralized_adapter_reference_program(
            &DecentralizedAdapterReferenceProgramSpec::open_default(),
        )?;
        assert_eq!(
            report.first_window_plan.adapter_target.adapter_family,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY
        );
        assert_eq!(
            report.first_window_plan.adapter_target.adapter_format,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT
        );
        assert_eq!(report.first_window_summary.accepted_contributions, 2);
        assert_eq!(report.second_window_summary.accepted_contributions, 2);
        assert!(report.operator_view.latency_envelope.seal_latency_ms > 0);
        assert!(report.operator_view.latency_envelope.replay_latency_ms > 0);
        assert!(report.operator_view.latency_envelope.promotion_latency_ms > 0);
        Ok(())
    }

    #[test]
    fn decentralized_adapter_stale_upload_remains_incomplete()
    -> Result<(), Box<dyn std::error::Error>> {
        let family = family_config(DecentralizedAdapterReferenceFamily::OpenGptOssLmHead)?;
        let state = cluster_state(
            family.backend_label,
            &[
                (
                    "trainer-a",
                    NodeRole::CoordinatorOnly,
                    ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-b",
                    NodeRole::ExecutorOnly,
                    ClusterMembershipStatus::Ready,
                ),
            ],
            &[
                ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
                ("worker-b", 22, 1, ClusterBackendReadinessStatus::Ready),
            ],
        );
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            crate::TrainingRunState::new(
                "stale-upload-run",
                "adapter-sft",
                state.cluster_id().as_str(),
                "adapter.policy",
                EnvironmentPackageKey::new("oa.open.adapter", "2026.03"),
            )?,
            family.adapter_target.clone(),
            PolicyRevision::new("adapter.policy", "policy-r1", "policy-digest-r1", 1_000)
                .with_revision_number(1)
                .with_checkpoint(reference_checkpoint(
                    "adapter.policy",
                    "stale-upload-run",
                    "policy-r1",
                    1,
                    1_000,
                )),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-stale-upload"),
                "adapter.policy",
                reference_checkpoint("adapter.policy", "stale-upload-run", "policy-r1", 1, 1_000)
                    .with_durable_at_ms(1_001),
                "manifest-r1",
                1_001,
            )?,
            family.capability_policy.clone(),
        );
        let _ = coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-a",
                "slice-digest-a",
            )?],
            1,
            1_020,
        )?;
        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        let assignment = protocol.assignments[0].clone();
        let payload = contribution_payload(
            DecentralizedAdapterReferenceFamily::OpenGptOssLmHead,
            record.plan.window_id.as_str(),
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
        );
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            payload.as_slice(),
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            format!(
                "{}/artifact",
                assignment.upload_expectation.upload_reference_prefix
            ),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            payload.as_slice(),
            8,
            assignment.worker_id.clone(),
            1_030,
        )?;
        let error = storage
            .complete_contribution_upload(cursor.upload_id.as_str(), 1_031)
            .expect_err("incomplete upload should fail");
        assert!(matches!(
            error,
            AdapterArtifactStorageError::UploadIncomplete { .. }
        ));
        Ok(())
    }

    #[test]
    fn decentralized_adapter_manifest_corruption_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let family = family_config(DecentralizedAdapterReferenceFamily::AppleFoundationModels)?;
        let state = cluster_state(
            family.backend_label,
            &[
                (
                    "trainer-a",
                    NodeRole::CoordinatorOnly,
                    ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-b",
                    NodeRole::ExecutorOnly,
                    ClusterMembershipStatus::Ready,
                ),
            ],
            &[
                ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
                ("worker-b", 22, 1, ClusterBackendReadinessStatus::Ready),
            ],
        );
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            crate::TrainingRunState::new(
                "manifest-corruption-run",
                "adapter-sft",
                state.cluster_id().as_str(),
                "adapter.policy",
                EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
            )?,
            family.adapter_target.clone(),
            PolicyRevision::new("adapter.policy", "policy-r1", "policy-digest-r1", 1_000)
                .with_revision_number(1)
                .with_checkpoint(reference_checkpoint(
                    "adapter.policy",
                    "manifest-corruption-run",
                    "policy-r1",
                    1,
                    1_000,
                )),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(
                    CheckpointScopeKind::Window,
                    "window-manifest-corruption",
                ),
                "adapter.policy",
                reference_checkpoint(
                    "adapter.policy",
                    "manifest-corruption-run",
                    "policy-r1",
                    1,
                    1_000,
                )
                .with_durable_at_ms(1_001),
                "manifest-r1",
                1_001,
            )?,
            family.capability_policy.clone(),
        );
        let _ = coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-a",
                "slice-digest-a",
            )?],
            1,
            1_020,
        )?;
        let assignment = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        )
        .assignments[0]
            .clone();
        let payload = contribution_payload(
            DecentralizedAdapterReferenceFamily::AppleFoundationModels,
            record.plan.window_id.as_str(),
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
        );
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            payload.as_slice(),
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            format!(
                "{}/artifact",
                assignment.upload_expectation.upload_reference_prefix
            ),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            payload.as_slice(),
            8,
            assignment.worker_id.clone(),
            1_030,
        )?;
        let mut corrupted = payload[..8].to_vec();
        corrupted[0] ^= 0xFF;
        let error = storage
            .commit_next_chunk(cursor.upload_id.as_str(), corrupted.as_slice())
            .expect_err("corrupted chunk should fail");
        assert!(matches!(
            error,
            AdapterArtifactStorageError::UploadChunkDigestMismatch { .. }
        ));
        Ok(())
    }

    #[test]
    fn decentralized_adapter_missing_replay_blocks_promotion()
    -> Result<(), Box<dyn std::error::Error>> {
        let family = family_config(DecentralizedAdapterReferenceFamily::OpenGptOssLmHead)?;
        let state = cluster_state(
            family.backend_label,
            &[
                (
                    "trainer-a",
                    NodeRole::CoordinatorOnly,
                    ClusterMembershipStatus::Ready,
                ),
                (
                    "worker-b",
                    NodeRole::ExecutorOnly,
                    ClusterMembershipStatus::Ready,
                ),
            ],
            &[
                ("trainer-a", 24, 1, ClusterBackendReadinessStatus::Ready),
                ("worker-b", 22, 1, ClusterBackendReadinessStatus::Ready),
            ],
        );
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            crate::TrainingRunState::new(
                "missing-replay-run",
                "adapter-sft",
                state.cluster_id().as_str(),
                "adapter.policy",
                EnvironmentPackageKey::new("oa.open.adapter", "2026.03"),
            )?,
            family.adapter_target.clone(),
            PolicyRevision::new("adapter.policy", "policy-r1", "policy-digest-r1", 1_000)
                .with_revision_number(1)
                .with_checkpoint(reference_checkpoint(
                    "adapter.policy",
                    "missing-replay-run",
                    "policy-r1",
                    1,
                    1_000,
                )),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-missing-replay"),
                "adapter.policy",
                reference_checkpoint(
                    "adapter.policy",
                    "missing-replay-run",
                    "policy-r1",
                    1,
                    1_000,
                )
                .with_durable_at_ms(1_001),
                "manifest-r1",
                1_001,
            )?,
            family.capability_policy.clone(),
        );
        let _ = coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.reference",
                "train",
                "slice-a",
                "slice-digest-a",
            )?],
            1,
            1_020,
        )?;
        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        let assignment = protocol.assignments[0].clone();
        let signing_key = signing_key_for(
            record.plan.window_id.as_str(),
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
        );
        let identity = AdapterWorkerIdentity::new(
            assignment.worker_id.clone(),
            "worker-b-session".to_string(),
            AdapterWorkerTrustClass::SemiTrustedContributor,
            "reference:worker-b".to_string(),
        )
        .with_submission_signing_public_key_hex(hex::encode(
            signing_key.verifying_key().to_bytes(),
        ));
        protocol.record_heartbeat(identity.clone(), None, None, 1_021)?;
        let claim =
            protocol.claim_assignment("worker-b", assignment.assignment_id.as_str(), 1_022)?;
        protocol.acknowledge_assignment(
            "worker-b",
            "worker-b-session",
            claim.claim_id.as_str(),
            1_023,
        )?;
        let execution_summary = AdapterContributionExecutionSummary::new(
            1_024,
            1_025,
            8,
            32,
            Some(180),
            "adapter-delta:missing-replay".to_string(),
        )?;
        let payload = contribution_payload(
            DecentralizedAdapterReferenceFamily::OpenGptOssLmHead,
            record.plan.window_id.as_str(),
            assignment.worker_id.as_str(),
            assignment.assignment_id.as_str(),
        );
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            payload.as_slice(),
            8,
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            format!(
                "{}/artifact",
                assignment.upload_expectation.upload_reference_prefix
            ),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let submission = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "worker-b-session",
            assignment.source_policy_revision.revision_id.as_str(),
            assignment.source_checkpoint_pointer.pointer_digest.as_str(),
            execution_summary.clone(),
            upload.clone(),
            1_026,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            payload.as_slice(),
            8,
            assignment.worker_id.clone(),
            1_027,
        )?;
        for chunk in payload.chunks(8) {
            let _ = storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
        }
        let artifact = storage.complete_contribution_upload(cursor.upload_id.as_str(), 1_028)?;
        let provenance = AdapterContributionProvenanceBundle::new_signed(
            &assignment,
            &claim,
            &identity,
            &submission,
            &artifact,
            &signing_key,
            1_029,
        );
        let mut security = AdapterContributionSecurityController::new(
            AdapterContributionSecurityPolicy::default(),
        );
        let security_receipt = security.assess_submission(
            &protocol,
            &artifact,
            &submission,
            provenance.clone(),
            1_030,
        )?;
        let bundle = AdapterContributionValidationBundle::new(
            submission,
            artifact,
            provenance,
            security_receipt,
            None,
        );
        let mut validator =
            AdapterContributionValidatorState::new(AdapterContributionValidatorPolicy {
                replay_sample_bps: 10_000,
                ..family.validator_policy.clone()
            });
        let summary = validator.validate_window(&mut protocol.window, vec![bundle], None, 1_031)?;
        assert_eq!(summary.replay_required_contributions, 1);
        assert!(!summary.promotion_ready);
        assert_eq!(protocol.window.status, TrainingWindowStatus::Sealed);
        Ok(())
    }
}
