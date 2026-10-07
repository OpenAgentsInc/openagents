use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_cluster::{ClusterState, NodeId};
use psionic_datastream::DatastreamPolicyWeightBroadcastManifest;
use psionic_environments::EnvironmentPackageKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PolicyRevision, RolloutArtifact, RolloutTaskClaim, RolloutTaskClaimStatus,
    RolloutUploadLocator, RolloutValidatorPolicy, RolloutValidatorState, RolloutVerificationBundle,
    RolloutWorkerHeartbeatReceipt, RolloutWorkerIdentity, RolloutWorkerOutcomeReceipt,
    RolloutWorkerProtocolError, RolloutWorkerProtocolPolicy, RolloutWorkerProtocolState,
    TrainingOffPolicyBudget, TrainingOrchestratorBatchRecord, TrainingOrchestratorError,
    TrainingOrchestratorState, TrainingRunGraphError, TrainingRunState, TrainingRunStatus,
    TrainingWindowAssignmentRule, TrainingWindowStatus, ValidatorDisposition,
};

const LIVE_RL_RUN_SCHEMA_VERSION: &str = "psionic.live_rl_run_service.v1";

/// Operator-supplied participant ranking override for one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunParticipantPriority {
    /// Stable participant node id.
    pub node_id: String,
    /// Priority in basis points.
    pub priority_bps: u16,
    /// Reliability in basis points.
    pub reliability_bps: u16,
}

impl LiveRlRunParticipantPriority {
    /// Creates one participant ranking override.
    #[must_use]
    pub fn new(node_id: impl Into<String>, priority_bps: u16, reliability_bps: u16) -> Self {
        Self {
            node_id: node_id.into(),
            priority_bps,
            reliability_bps,
        }
    }
}

/// Request to create one durable live RL run.
#[derive(Clone, Debug)]
pub struct LiveRlRunCreateRequest {
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Checkpoint family bound to the run.
    pub checkpoint_family: String,
    /// Environment package bound to the run.
    pub environment: EnvironmentPackageKey,
    /// Current live cluster membership snapshot.
    pub cluster_state: ClusterState,
    /// Active target policy revision.
    pub target_policy_revision: PolicyRevision,
    /// Active policy-weight broadcast manifest.
    pub policy_weight_broadcast: DatastreamPolicyWeightBroadcastManifest,
    /// Explicit bounded off-policy budget.
    pub off_policy_budget: TrainingOffPolicyBudget,
    /// Optional participant ranking overrides.
    pub participant_priorities: Vec<LiveRlRunParticipantPriority>,
    /// Logical create time.
    pub created_at_ms: u64,
}

impl LiveRlRunCreateRequest {
    /// Creates one live RL run request.
    #[must_use]
    pub fn new(
        run_id: impl Into<String>,
        stage_id: impl Into<String>,
        checkpoint_family: impl Into<String>,
        environment: EnvironmentPackageKey,
        cluster_state: ClusterState,
        target_policy_revision: PolicyRevision,
        policy_weight_broadcast: DatastreamPolicyWeightBroadcastManifest,
        created_at_ms: u64,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            stage_id: stage_id.into(),
            checkpoint_family: checkpoint_family.into(),
            environment,
            cluster_state,
            target_policy_revision,
            policy_weight_broadcast,
            off_policy_budget: TrainingOffPolicyBudget::default(),
            participant_priorities: Vec::new(),
            created_at_ms,
        }
    }

    /// Attaches an explicit off-policy budget.
    #[must_use]
    pub fn with_off_policy_budget(mut self, off_policy_budget: TrainingOffPolicyBudget) -> Self {
        self.off_policy_budget = off_policy_budget;
        self
    }

    /// Adds one participant ranking override.
    #[must_use]
    pub fn with_participant_priority(
        mut self,
        participant_priority: LiveRlRunParticipantPriority,
    ) -> Self {
        self.participant_priorities.push(participant_priority);
        self
    }
}

/// Operator stop posture for one live RL run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveRlRunStopKind {
    /// The run finished successfully.
    Completed,
    /// The run was cancelled by an operator.
    Cancelled,
    /// The run failed terminally.
    Failed,
}

/// Stop request recorded by the live run service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunStopRequest {
    /// Operator stop posture.
    pub stop_kind: LiveRlRunStopKind,
    /// Optional operator-visible detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Logical stop-request time.
    pub requested_at_ms: u64,
}

impl LiveRlRunStopRequest {
    /// Creates one stop request.
    #[must_use]
    pub fn new(stop_kind: LiveRlRunStopKind, requested_at_ms: u64) -> Self {
        Self {
            stop_kind,
            detail: None,
            requested_at_ms,
        }
    }

    /// Attaches an optional operator detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Create receipt for one durable live RL run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunCreateReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Target policy revision id.
    pub target_policy_revision_id: String,
    /// Initial run status after create.
    pub run_status: TrainingRunStatus,
    /// Create time.
    pub created_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Lifecycle receipt for one window transition owned by the run service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlWindowLifecycleReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable window id.
    pub window_id: String,
    /// Window status reached by the service.
    pub window_status: TrainingWindowStatus,
    /// Current run status after the transition.
    pub run_status: TrainingRunStatus,
    /// Logical transition time.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Validator ingestion receipt attached to one live run window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlValidatorIngestionReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable window id.
    pub window_id: String,
    /// Stable bundle id.
    pub bundle_id: String,
    /// Stable rollout artifact id.
    pub artifact_id: String,
    /// Stable rollout artifact digest.
    pub artifact_digest: String,
    /// Stable validator verdict id.
    pub verdict_id: String,
    /// Final validator disposition.
    pub disposition: ValidatorDisposition,
    /// Logical ingestion time.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stop receipt for one live RL run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunStopReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Requested stop posture.
    pub stop_kind: LiveRlRunStopKind,
    /// Resulting run status after the request.
    pub resulting_run_status: TrainingRunStatus,
    /// Current active window when one still exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_window_id: Option<String>,
    /// Logical stop time.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Durable failure artifact emitted by the live run service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunFailureArtifact {
    /// Stable failure id.
    pub failure_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Active window id when one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Resulting terminal run status.
    pub resulting_run_status: TrainingRunStatus,
    /// Operator-visible failure detail.
    pub detail: String,
    /// Logical failure time.
    pub observed_at_ms: u64,
    /// Stable failure digest.
    pub failure_digest: String,
}

/// Operator-facing run status artifact persisted by the service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunStatusArtifact {
    /// Schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Current run status.
    pub run_status: TrainingRunStatus,
    /// Target policy revision id.
    pub target_policy_revision_id: String,
    /// Current window id when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_window_id: Option<String>,
    /// Current window status when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_window_status: Option<TrainingWindowStatus>,
    /// Total window count.
    pub window_count: u32,
    /// Total exact accepted rollout count.
    pub accepted_exact_rollout_count: u64,
    /// Total bounded off-policy accepted rollout count.
    pub accepted_off_policy_rollout_count: u64,
    /// Total quarantined rollout count.
    pub quarantined_rollout_count: u64,
    /// Total discarded rollout count.
    pub discarded_rollout_count: u64,
    /// Total trainer-batch count.
    pub trainer_batch_count: u32,
    /// Current window registered-worker count.
    pub current_window_registered_worker_count: u32,
    /// Current window active-claim count.
    pub current_window_active_claim_count: u32,
    /// Validator accepted count.
    pub validator_accepted_count: u32,
    /// Validator normalized count.
    pub validator_normalized_count: u32,
    /// Validator rejected count.
    pub validator_rejected_count: u32,
    /// Whether a graceful stop request is still pending.
    pub stop_requested: bool,
    /// Last update time persisted by the service.
    pub updated_at_ms: u64,
    /// Stable status digest.
    pub status_digest: String,
}

/// Operator-facing window status artifact persisted by the service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlWindowStatusArtifact {
    /// Schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable window id.
    pub window_id: String,
    /// Current window status.
    pub window_status: TrainingWindowStatus,
    /// Assigned worker count.
    pub assigned_worker_count: u32,
    /// Registered worker count in the window protocol.
    pub registered_worker_count: u32,
    /// Active claim count.
    pub active_claim_count: u32,
    /// Uploaded claim count.
    pub uploaded_claim_count: u32,
    /// Expired claim count.
    pub expired_claim_count: u32,
    /// Rejected claim count.
    pub rejected_claim_count: u32,
    /// Exact accepted rollout count.
    pub accepted_exact_rollout_count: u64,
    /// Bounded off-policy accepted rollout count.
    pub accepted_off_policy_rollout_count: u64,
    /// Quarantined rollout count.
    pub quarantined_rollout_count: u64,
    /// Discarded rollout count.
    pub discarded_rollout_count: u64,
    /// Trainer-batch count.
    pub trainer_batch_count: u32,
    /// Validator accepted count for the window.
    pub validator_accepted_count: u32,
    /// Validator normalized count for the window.
    pub validator_normalized_count: u32,
    /// Validator rejected count for the window.
    pub validator_rejected_count: u32,
    /// Last update time persisted by the service.
    pub updated_at_ms: u64,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

/// Durable service-owned worker protocol state for one window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlWindowProtocolRecord {
    /// Stable window id.
    pub window_id: String,
    /// Stateful worker protocol for that window.
    pub protocol: RolloutWorkerProtocolState,
}

/// Durable live RL run state persisted by the service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRlManagedRun {
    /// Schema version.
    pub schema_version: String,
    /// Create time.
    pub created_at_ms: u64,
    /// Last update time.
    pub updated_at_ms: u64,
    /// Service-owned orchestrator state.
    pub orchestrator: TrainingOrchestratorState,
    /// Service-owned validator state.
    pub validator_state: RolloutValidatorState,
    /// Per-window worker protocol state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub window_protocols: Vec<LiveRlWindowProtocolRecord>,
    /// Validator ingestion receipts keyed into run or window lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validator_receipts: Vec<LiveRlValidatorIngestionReceipt>,
    /// Pending stop request when the run is draining.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_request: Option<LiveRlRunStopRequest>,
    /// Failure artifacts emitted by the service.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<LiveRlRunFailureArtifact>,
}

impl LiveRlManagedRun {
    fn run_id(&self) -> &str {
        self.orchestrator.run.run_id.as_str()
    }

    fn stage_id(&self) -> &str {
        self.orchestrator.run.stage_id.as_str()
    }

    fn current_window_id(&self) -> Option<&str> {
        self.orchestrator.current_window_id.as_deref()
    }

    fn current_window_status(&self) -> Option<TrainingWindowStatus> {
        let current_window_id = self.current_window_id()?;
        self.orchestrator
            .run
            .windows
            .iter()
            .find(|window| window.window_id == current_window_id)
            .map(|window| window.status)
    }

    fn window_protocol(&self, window_id: &str) -> Option<&RolloutWorkerProtocolState> {
        self.window_protocols
            .iter()
            .find(|record| record.window_id == window_id)
            .map(|record| &record.protocol)
    }

    fn window_protocol_index(&self, window_id: &str) -> Option<usize> {
        self.window_protocols
            .iter()
            .position(|record| record.window_id == window_id)
    }

    fn totals(&self) -> LiveRlRunTotals {
        let mut totals = LiveRlRunTotals::default();
        for window in &self.orchestrator.orchestrator_windows {
            totals.accepted_exact_rollout_count = totals
                .accepted_exact_rollout_count
                .saturating_add(window.rollout_telemetry.accepted_exact_rollout_count);
            totals.accepted_off_policy_rollout_count = totals
                .accepted_off_policy_rollout_count
                .saturating_add(window.rollout_telemetry.accepted_off_policy_rollout_count);
            totals.quarantined_rollout_count = totals
                .quarantined_rollout_count
                .saturating_add(window.rollout_telemetry.quarantined_rollout_count);
            totals.discarded_rollout_count = totals
                .discarded_rollout_count
                .saturating_add(window.rollout_telemetry.discarded_rollout_count);
            totals.trainer_batch_count = totals
                .trainer_batch_count
                .saturating_add(window.trainer_batches.len() as u32);
        }
        for receipt in &self.validator_receipts {
            match receipt.disposition {
                ValidatorDisposition::Accepted => {
                    totals.validator_accepted_count =
                        totals.validator_accepted_count.saturating_add(1);
                }
                ValidatorDisposition::Normalized => {
                    totals.validator_normalized_count =
                        totals.validator_normalized_count.saturating_add(1);
                }
                ValidatorDisposition::Rejected => {
                    totals.validator_rejected_count =
                        totals.validator_rejected_count.saturating_add(1);
                }
            }
        }
        if let Some(current_window_id) = self.current_window_id() {
            if let Some(protocol) = self.window_protocol(current_window_id) {
                totals.current_window_registered_worker_count =
                    protocol.workers.len().try_into().unwrap_or(u32::MAX);
                totals.current_window_active_claim_count = protocol
                    .claims
                    .iter()
                    .filter(|claim| claim.status == RolloutTaskClaimStatus::Active)
                    .count()
                    .try_into()
                    .unwrap_or(u32::MAX);
            }
        }
        totals
    }

    fn status_artifact(&self) -> LiveRlRunStatusArtifact {
        let totals = self.totals();
        let current_window_id = self.current_window_id().map(String::from);
        let current_window_status = self.current_window_status();
        let status_digest = stable_live_rl_digest(
            "run_status",
            &serde_json::json!({
                "schema_version": LIVE_RL_RUN_SCHEMA_VERSION,
                "run_id": self.run_id(),
                "stage_id": self.stage_id(),
                "run_status": self.orchestrator.run.status,
                "target_policy_revision_id": self.orchestrator.target_policy_revision.revision_id,
                "current_window_id": current_window_id,
                "current_window_status": current_window_status,
                "window_count": self.orchestrator.orchestrator_windows.len(),
                "accepted_exact_rollout_count": totals.accepted_exact_rollout_count,
                "accepted_off_policy_rollout_count": totals.accepted_off_policy_rollout_count,
                "quarantined_rollout_count": totals.quarantined_rollout_count,
                "discarded_rollout_count": totals.discarded_rollout_count,
                "trainer_batch_count": totals.trainer_batch_count,
                "current_window_registered_worker_count": totals.current_window_registered_worker_count,
                "current_window_active_claim_count": totals.current_window_active_claim_count,
                "validator_accepted_count": totals.validator_accepted_count,
                "validator_normalized_count": totals.validator_normalized_count,
                "validator_rejected_count": totals.validator_rejected_count,
                "stop_requested": self.stop_request.is_some(),
                "updated_at_ms": self.updated_at_ms,
            }),
        );
        LiveRlRunStatusArtifact {
            schema_version: String::from(LIVE_RL_RUN_SCHEMA_VERSION),
            run_id: String::from(self.run_id()),
            stage_id: String::from(self.stage_id()),
            run_status: self.orchestrator.run.status,
            target_policy_revision_id: self.orchestrator.target_policy_revision.revision_id.clone(),
            current_window_id: self.current_window_id().map(String::from),
            current_window_status,
            window_count: self
                .orchestrator
                .orchestrator_windows
                .len()
                .try_into()
                .unwrap_or(u32::MAX),
            accepted_exact_rollout_count: totals.accepted_exact_rollout_count,
            accepted_off_policy_rollout_count: totals.accepted_off_policy_rollout_count,
            quarantined_rollout_count: totals.quarantined_rollout_count,
            discarded_rollout_count: totals.discarded_rollout_count,
            trainer_batch_count: totals.trainer_batch_count,
            current_window_registered_worker_count: totals.current_window_registered_worker_count,
            current_window_active_claim_count: totals.current_window_active_claim_count,
            validator_accepted_count: totals.validator_accepted_count,
            validator_normalized_count: totals.validator_normalized_count,
            validator_rejected_count: totals.validator_rejected_count,
            stop_requested: self.stop_request.is_some(),
            updated_at_ms: self.updated_at_ms,
            status_digest,
        }
    }

    fn window_status_artifacts(&self) -> Vec<LiveRlWindowStatusArtifact> {
        self.orchestrator
            .orchestrator_windows
            .iter()
            .map(|window| {
                let protocol = self.window_protocol(window.window_id.as_str());
                let (
                    registered_worker_count,
                    active_claim_count,
                    uploaded_claim_count,
                    expired_claim_count,
                    rejected_claim_count,
                ) = match protocol {
                    Some(protocol) => (
                        protocol.workers.len().try_into().unwrap_or(u32::MAX),
                        protocol
                            .claims
                            .iter()
                            .filter(|claim| claim.status == RolloutTaskClaimStatus::Active)
                            .count()
                            .try_into()
                            .unwrap_or(u32::MAX),
                        protocol
                            .claims
                            .iter()
                            .filter(|claim| claim.status == RolloutTaskClaimStatus::Uploaded)
                            .count()
                            .try_into()
                            .unwrap_or(u32::MAX),
                        protocol
                            .claims
                            .iter()
                            .filter(|claim| claim.status == RolloutTaskClaimStatus::Expired)
                            .count()
                            .try_into()
                            .unwrap_or(u32::MAX),
                        protocol
                            .claims
                            .iter()
                            .filter(|claim| claim.status == RolloutTaskClaimStatus::Rejected)
                            .count()
                            .try_into()
                            .unwrap_or(u32::MAX),
                    ),
                    None => (0, 0, 0, 0, 0),
                };
                let validator_accepted_count = self
                    .validator_receipts
                    .iter()
                    .filter(|receipt| {
                        receipt.window_id == window.window_id
                            && receipt.disposition == ValidatorDisposition::Accepted
                    })
                    .count()
                    .try_into()
                    .unwrap_or(u32::MAX);
                let validator_normalized_count = self
                    .validator_receipts
                    .iter()
                    .filter(|receipt| {
                        receipt.window_id == window.window_id
                            && receipt.disposition == ValidatorDisposition::Normalized
                    })
                    .count()
                    .try_into()
                    .unwrap_or(u32::MAX);
                let validator_rejected_count = self
                    .validator_receipts
                    .iter()
                    .filter(|receipt| {
                        receipt.window_id == window.window_id
                            && receipt.disposition == ValidatorDisposition::Rejected
                    })
                    .count()
                    .try_into()
                    .unwrap_or(u32::MAX);
                let artifact_digest = stable_live_rl_digest(
                    "window_status",
                    &serde_json::json!({
                        "schema_version": LIVE_RL_RUN_SCHEMA_VERSION,
                        "run_id": self.run_id(),
                        "window_id": window.window_id,
                        "window_status": window_status_label(window_status_from_run(
                            &self.orchestrator.run,
                            window.window_id.as_str(),
                        )),
                        "assigned_worker_count": window.rollout_assignments.len(),
                        "registered_worker_count": registered_worker_count,
                        "active_claim_count": active_claim_count,
                        "uploaded_claim_count": uploaded_claim_count,
                        "expired_claim_count": expired_claim_count,
                        "rejected_claim_count": rejected_claim_count,
                        "accepted_exact_rollout_count": window.rollout_telemetry.accepted_exact_rollout_count,
                        "accepted_off_policy_rollout_count": window.rollout_telemetry.accepted_off_policy_rollout_count,
                        "quarantined_rollout_count": window.rollout_telemetry.quarantined_rollout_count,
                        "discarded_rollout_count": window.rollout_telemetry.discarded_rollout_count,
                        "trainer_batch_count": window.trainer_batches.len(),
                        "validator_accepted_count": validator_accepted_count,
                        "validator_normalized_count": validator_normalized_count,
                        "validator_rejected_count": validator_rejected_count,
                        "updated_at_ms": self.updated_at_ms,
                    }),
                );
                LiveRlWindowStatusArtifact {
                    schema_version: String::from(LIVE_RL_RUN_SCHEMA_VERSION),
                    run_id: String::from(self.run_id()),
                    window_id: window.window_id.clone(),
                    window_status: window_status_from_run(
                        &self.orchestrator.run,
                        window.window_id.as_str(),
                    ),
                    assigned_worker_count: window
                        .rollout_assignments
                        .len()
                        .try_into()
                        .unwrap_or(u32::MAX),
                    registered_worker_count,
                    active_claim_count,
                    uploaded_claim_count,
                    expired_claim_count,
                    rejected_claim_count,
                    accepted_exact_rollout_count: window
                        .rollout_telemetry
                        .accepted_exact_rollout_count,
                    accepted_off_policy_rollout_count: window
                        .rollout_telemetry
                        .accepted_off_policy_rollout_count,
                    quarantined_rollout_count: window.rollout_telemetry.quarantined_rollout_count,
                    discarded_rollout_count: window.rollout_telemetry.discarded_rollout_count,
                    trainer_batch_count: window
                        .trainer_batches
                        .len()
                        .try_into()
                        .unwrap_or(u32::MAX),
                    validator_accepted_count,
                    validator_normalized_count,
                    validator_rejected_count,
                    updated_at_ms: self.updated_at_ms,
                    artifact_digest,
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
struct LiveRlRunTotals {
    accepted_exact_rollout_count: u64,
    accepted_off_policy_rollout_count: u64,
    quarantined_rollout_count: u64,
    discarded_rollout_count: u64,
    trainer_batch_count: u32,
    current_window_registered_worker_count: u32,
    current_window_active_claim_count: u32,
    validator_accepted_count: u32,
    validator_normalized_count: u32,
    validator_rejected_count: u32,
}

/// Configuration for the durable live RL run service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRlRunServiceConfig {
    /// Durable storage root owned by the service.
    pub storage_root: PathBuf,
    /// Worker protocol policy applied to each planned window.
    pub worker_protocol_policy: RolloutWorkerProtocolPolicy,
    /// Validator policy applied to the run-owned validator state.
    pub validator_policy: RolloutValidatorPolicy,
}

impl LiveRlRunServiceConfig {
    /// Creates a bounded live RL run service config.
    #[must_use]
    pub fn new(storage_root: impl Into<PathBuf>) -> Self {
        Self {
            storage_root: storage_root.into(),
            worker_protocol_policy: RolloutWorkerProtocolPolicy::default(),
            validator_policy: RolloutValidatorPolicy::default(),
        }
    }
}

/// Durable live RL run service failure.
#[derive(Debug, Error)]
pub enum LiveRlRunServiceError {
    /// One service persistence step failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// One service persistence step could not serialize or deserialize JSON.
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    /// The requested run already exists.
    #[error("live RL run service already knows run `{run_id}`")]
    DuplicateRun {
        /// Stable run id.
        run_id: String,
    },
    /// The requested run is unknown.
    #[error("live RL run service does not know run `{run_id}`")]
    UnknownRun {
        /// Stable run id.
        run_id: String,
    },
    /// The run no longer accepts new windows.
    #[error("live RL run `{run_id}` cannot plan a new window while status is `{status}`")]
    RunNotAcceptingNewWindows {
        /// Stable run id.
        run_id: String,
        /// Current run status.
        status: String,
    },
    /// The current window protocol is missing.
    #[error("live RL run `{run_id}` is missing worker protocol state for window `{window_id}`")]
    MissingWindowProtocol {
        /// Stable run id.
        run_id: String,
        /// Stable window id.
        window_id: String,
    },
    /// The validator bundle id was already ingested.
    #[error("live RL run `{run_id}` already ingested validator bundle `{bundle_id}`")]
    DuplicateValidatorBundle {
        /// Stable run id.
        run_id: String,
        /// Stable bundle id.
        bundle_id: String,
    },
    /// The validator bundle referenced a rollout artifact the run does not know.
    #[error(
        "live RL run `{run_id}` window `{window_id}` does not know rollout artifact `{artifact_id}` for validator ingestion"
    )]
    UnknownValidatorArtifact {
        /// Stable run id.
        run_id: String,
        /// Stable window id.
        window_id: String,
        /// Stable rollout artifact id.
        artifact_id: String,
    },
    /// The validator bundle referenced a worker outcome the run does not know.
    #[error(
        "live RL run `{run_id}` window `{window_id}` does not know worker outcome receipt `{receipt_digest}` for validator ingestion"
    )]
    UnknownValidatorOutcome {
        /// Stable run id.
        run_id: String,
        /// Stable window id.
        window_id: String,
        /// Stable outcome receipt digest.
        receipt_digest: String,
    },
    /// The run graph rejected one participant update.
    #[error(transparent)]
    RunGraph(#[from] TrainingRunGraphError),
    /// The orchestrator rejected one state transition.
    #[error(transparent)]
    Orchestrator(#[from] TrainingOrchestratorError),
    /// The worker protocol rejected one heartbeat, claim, or upload.
    #[error(transparent)]
    WorkerProtocol(#[from] RolloutWorkerProtocolError),
}

/// Durable live RL run service above the run graph, orchestrator, worker protocol, and validator state.
#[derive(Debug)]
pub struct LiveRlRunService {
    config: LiveRlRunServiceConfig,
    runs: BTreeMap<String, LiveRlManagedRun>,
}

impl LiveRlRunService {
    /// Opens the service and loads any existing durable runs from disk.
    pub fn new(config: LiveRlRunServiceConfig) -> Result<Self, LiveRlRunServiceError> {
        fs::create_dir_all(config.storage_root.join("runs"))?;
        let runs = load_runs(config.storage_root.as_path())?;
        Ok(Self { config, runs })
    }

    /// Returns inspectable current status for one live RL run.
    pub fn status(&self, run_id: &str) -> Result<LiveRlRunStatusArtifact, LiveRlRunServiceError> {
        Ok(self.run(run_id)?.status_artifact())
    }

    /// Returns inspectable status for one live RL window.
    pub fn window_status(
        &self,
        run_id: &str,
        window_id: &str,
    ) -> Result<LiveRlWindowStatusArtifact, LiveRlRunServiceError> {
        self.run(run_id)?
            .window_status_artifacts()
            .into_iter()
            .find(|artifact| artifact.window_id == window_id)
            .ok_or_else(|| LiveRlRunServiceError::MissingWindowProtocol {
                run_id: String::from(run_id),
                window_id: String::from(window_id),
            })
    }

    /// Creates and persists one new live RL run.
    pub fn create_run(
        &mut self,
        request: LiveRlRunCreateRequest,
    ) -> Result<LiveRlRunCreateReceipt, LiveRlRunServiceError> {
        if self.runs.contains_key(request.run_id.as_str()) {
            return Err(LiveRlRunServiceError::DuplicateRun {
                run_id: request.run_id,
            });
        }

        let mut run = TrainingRunState::new(
            request.run_id.clone(),
            request.stage_id.clone(),
            request.cluster_state.cluster_id().as_str(),
            request.checkpoint_family,
            request.environment,
        )?;
        run.apply_cluster_membership_snapshot(&request.cluster_state, request.created_at_ms)?;
        for participant_priority in &request.participant_priorities {
            run.update_participant_priority(
                &NodeId::new(participant_priority.node_id.as_str()),
                participant_priority.priority_bps,
                participant_priority.reliability_bps,
                request.created_at_ms,
            )?;
        }

        let orchestrator = TrainingOrchestratorState::new_with_budget(
            run,
            request.target_policy_revision.clone(),
            request.policy_weight_broadcast,
            request.off_policy_budget,
        )?;
        let managed = LiveRlManagedRun {
            schema_version: String::from(LIVE_RL_RUN_SCHEMA_VERSION),
            created_at_ms: request.created_at_ms,
            updated_at_ms: request.created_at_ms,
            orchestrator,
            validator_state: RolloutValidatorState::new(self.config.validator_policy.clone()),
            window_protocols: Vec::new(),
            validator_receipts: Vec::new(),
            stop_request: None,
            failures: Vec::new(),
        };
        let receipt = LiveRlRunCreateReceipt {
            receipt_id: format!("{}-created", managed.run_id()),
            run_id: String::from(managed.run_id()),
            stage_id: String::from(managed.stage_id()),
            target_policy_revision_id: managed
                .orchestrator
                .target_policy_revision
                .revision_id
                .clone(),
            run_status: managed.orchestrator.run.status,
            created_at_ms: request.created_at_ms,
            receipt_digest: stable_live_rl_digest(
                "run_create_receipt",
                &(
                    managed.run_id(),
                    managed.stage_id(),
                    managed
                        .orchestrator
                        .target_policy_revision
                        .revision_id
                        .as_str(),
                    managed.orchestrator.run.status,
                    request.created_at_ms,
                ),
            ),
        };
        let run_id = receipt.run_id.clone();
        self.runs.insert(run_id.clone(), managed);
        self.persist_run(run_id.as_str())?;
        self.persist_artifact(
            run_id.as_str(),
            format!("run-created-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Plans the next window and creates worker protocol state for it.
    pub fn plan_next_window(
        &mut self,
        run_id: &str,
        max_contributors: usize,
        assignment_rule: TrainingWindowAssignmentRule,
        assignment_seed: u64,
        planned_at_ms: u64,
    ) -> Result<LiveRlWindowLifecycleReceipt, LiveRlRunServiceError> {
        let worker_protocol_policy = self.config.worker_protocol_policy.clone();
        let receipt = {
            let run = self.run_mut(run_id)?;
            if run.orchestrator.run.status != TrainingRunStatus::Running {
                return Err(LiveRlRunServiceError::RunNotAcceptingNewWindows {
                    run_id: String::from(run_id),
                    status: run_status_label(run.orchestrator.run.status).to_string(),
                });
            }
            let window = run.orchestrator.plan_next_window(
                max_contributors,
                assignment_rule,
                assignment_seed,
                planned_at_ms,
            )?;
            if run.window_protocol(window.window_id.as_str()).is_some() {
                return Err(LiveRlRunServiceError::MissingWindowProtocol {
                    run_id: String::from(run_id),
                    window_id: window.window_id.clone(),
                });
            }
            run.window_protocols.push(LiveRlWindowProtocolRecord {
                window_id: window.window_id.clone(),
                protocol: RolloutWorkerProtocolState::from_window(
                    &window,
                    run.orchestrator.target_policy_revision.clone(),
                    worker_protocol_policy,
                ),
            });
            run.updated_at_ms = planned_at_ms;
            LiveRlWindowLifecycleReceipt {
                receipt_id: format!("{}-planned", window.window_id),
                run_id: String::from(run.run_id()),
                window_id: window.window_id.clone(),
                window_status: TrainingWindowStatus::Planned,
                run_status: run.orchestrator.run.status,
                observed_at_ms: planned_at_ms,
                receipt_digest: stable_live_rl_digest(
                    "window_lifecycle_receipt",
                    &(
                        run.run_id(),
                        window.window_id.as_str(),
                        window_status_label(TrainingWindowStatus::Planned),
                        run_status_label(run.orchestrator.run.status),
                        planned_at_ms,
                    ),
                ),
            }
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("window-planned-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Activates the current window.
    pub fn activate_current_window(
        &mut self,
        run_id: &str,
        active_at_ms: u64,
    ) -> Result<LiveRlWindowLifecycleReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            let window_id = String::from(current_window_id(run)?);
            run.orchestrator.activate_current_window(active_at_ms)?;
            run.updated_at_ms = active_at_ms;
            LiveRlWindowLifecycleReceipt {
                receipt_id: format!("{window_id}-active"),
                run_id: String::from(run.run_id()),
                window_id,
                window_status: TrainingWindowStatus::Active,
                run_status: run.orchestrator.run.status,
                observed_at_ms: active_at_ms,
                receipt_digest: stable_live_rl_digest(
                    "window_lifecycle_receipt",
                    &(
                        run.run_id(),
                        current_window_id(run)?,
                        window_status_label(TrainingWindowStatus::Active),
                        run_status_label(run.orchestrator.run.status),
                        active_at_ms,
                    ),
                ),
            }
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("window-active-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Records or refreshes one worker heartbeat against the current window.
    pub fn record_worker_heartbeat(
        &mut self,
        run_id: &str,
        identity: RolloutWorkerIdentity,
        observed_at_ms: u64,
    ) -> Result<RolloutWorkerHeartbeatReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            let worker_id = identity.worker_id.clone();
            run.orchestrator
                .run
                .record_heartbeat(&NodeId::new(worker_id.as_str()), observed_at_ms)?;
            let protocol = current_window_protocol_mut(run)?;
            let receipt = protocol.record_heartbeat(identity, observed_at_ms);
            run.updated_at_ms = observed_at_ms;
            receipt
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("worker-heartbeat-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Claims one current-window rollout assignment.
    pub fn claim_assignment(
        &mut self,
        run_id: &str,
        worker_id: &str,
        assignment_id: &str,
        claimed_at_ms: u64,
    ) -> Result<RolloutTaskClaim, LiveRlRunServiceError> {
        let claim = {
            let run = self.run_mut(run_id)?;
            let claim = current_window_protocol_mut(run)?.claim_assignment(
                worker_id,
                assignment_id,
                claimed_at_ms,
            )?;
            run.updated_at_ms = claimed_at_ms;
            claim
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("worker-claim-{}", claim.claim_digest).as_str(),
            &claim,
        )?;
        Ok(claim)
    }

    /// Submits one claimed rollout through the live service.
    pub fn submit_claimed_rollout(
        &mut self,
        run_id: &str,
        claim_id: &str,
        artifact: RolloutArtifact,
        upload: RolloutUploadLocator,
        observed_at_ms: u64,
    ) -> Result<RolloutWorkerOutcomeReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            let current_window_id = String::from(current_window_id(run)?);
            let protocol_index = run
                .window_protocol_index(current_window_id.as_str())
                .ok_or_else(|| LiveRlRunServiceError::MissingWindowProtocol {
                    run_id: String::from(run_id),
                    window_id: current_window_id.clone(),
                })?;
            let receipt = {
                let orchestrator = &mut run.orchestrator;
                let protocol = &mut run.window_protocols[protocol_index].protocol;
                protocol.submit_claimed_rollout(
                    orchestrator,
                    claim_id,
                    artifact,
                    upload,
                    observed_at_ms,
                )?
            };
            run.updated_at_ms = observed_at_ms;
            receipt
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("worker-outcome-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Ingests one validator bundle against service-owned run state.
    pub fn ingest_validator_bundle(
        &mut self,
        run_id: &str,
        bundle: RolloutVerificationBundle,
        observed_at_ms: u64,
    ) -> Result<LiveRlValidatorIngestionReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            if run
                .validator_receipts
                .iter()
                .any(|receipt| receipt.bundle_id == bundle.bundle_id)
            {
                return Err(LiveRlRunServiceError::DuplicateValidatorBundle {
                    run_id: String::from(run_id),
                    bundle_id: bundle.bundle_id,
                });
            }
            let window_id = bundle.worker_outcome.window_id.clone();
            let window = run
                .orchestrator
                .orchestrator_windows
                .iter()
                .find(|window| window.window_id == window_id)
                .ok_or_else(|| LiveRlRunServiceError::MissingWindowProtocol {
                    run_id: String::from(run_id),
                    window_id: window_id.clone(),
                })?;
            if !window_knows_artifact(
                window,
                bundle.artifact.artifact_id.as_str(),
                bundle.artifact.artifact_digest.as_str(),
            ) {
                return Err(LiveRlRunServiceError::UnknownValidatorArtifact {
                    run_id: String::from(run_id),
                    window_id,
                    artifact_id: bundle.artifact.artifact_id.clone(),
                });
            }
            let protocol = run
                .window_protocol(bundle.worker_outcome.window_id.as_str())
                .ok_or_else(|| LiveRlRunServiceError::MissingWindowProtocol {
                    run_id: String::from(run_id),
                    window_id: bundle.worker_outcome.window_id.clone(),
                })?;
            if !protocol
                .outcome_receipts
                .iter()
                .any(|receipt| receipt.receipt_digest == bundle.worker_outcome.receipt_digest)
            {
                return Err(LiveRlRunServiceError::UnknownValidatorOutcome {
                    run_id: String::from(run_id),
                    window_id: bundle.worker_outcome.window_id.clone(),
                    receipt_digest: bundle.worker_outcome.receipt_digest.clone(),
                });
            }
            let window_id = bundle.worker_outcome.window_id.clone();
            let bundle_id = bundle.bundle_id.clone();
            let artifact_id = bundle.artifact.artifact_id.clone();
            let artifact_digest = bundle.artifact.artifact_digest.clone();
            let verdict = run.validator_state.verify_bundle(bundle);
            let receipt = LiveRlValidatorIngestionReceipt {
                receipt_id: format!("{bundle_id}-validator"),
                run_id: String::from(run.run_id()),
                window_id,
                bundle_id,
                artifact_id,
                artifact_digest,
                verdict_id: verdict.verdict_id.clone(),
                disposition: verdict.disposition,
                observed_at_ms,
                receipt_digest: stable_live_rl_digest(
                    "validator_ingestion_receipt",
                    &(
                        run.run_id(),
                        verdict.verdict_id.as_str(),
                        verdict.artifact_digest.as_str(),
                        validator_disposition_label(verdict.disposition),
                        observed_at_ms,
                    ),
                ),
            };
            run.validator_receipts.push(receipt.clone());
            run.updated_at_ms = observed_at_ms;
            receipt
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("validator-verdict-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Seals the current window.
    pub fn seal_current_window(
        &mut self,
        run_id: &str,
        sealed_at_ms: u64,
    ) -> Result<LiveRlWindowLifecycleReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            let window_id = String::from(current_window_id(run)?);
            run.orchestrator.seal_current_window(sealed_at_ms)?;
            run.updated_at_ms = sealed_at_ms;
            LiveRlWindowLifecycleReceipt {
                receipt_id: format!("{window_id}-sealed"),
                run_id: String::from(run.run_id()),
                window_id,
                window_status: TrainingWindowStatus::Sealed,
                run_status: run.orchestrator.run.status,
                observed_at_ms: sealed_at_ms,
                receipt_digest: stable_live_rl_digest(
                    "window_lifecycle_receipt",
                    &(
                        run.run_id(),
                        current_window_id(run)?,
                        window_status_label(TrainingWindowStatus::Sealed),
                        run_status_label(run.orchestrator.run.status),
                        sealed_at_ms,
                    ),
                ),
            }
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("window-sealed-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Assembles one trainer batch from current-window rollout ids.
    pub fn assemble_trainer_batch(
        &mut self,
        run_id: &str,
        batch_id: impl Into<String>,
        rollout_ids: Vec<String>,
        assembled_at_ms: u64,
    ) -> Result<TrainingOrchestratorBatchRecord, LiveRlRunServiceError> {
        let batch_id = batch_id.into();
        let record = {
            let run = self.run_mut(run_id)?;
            let record =
                run.orchestrator
                    .assemble_trainer_batch(batch_id, rollout_ids, assembled_at_ms)?;
            run.updated_at_ms = assembled_at_ms;
            record
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("trainer-batch-{}", record.request.request_digest).as_str(),
            &record,
        )?;
        Ok(record)
    }

    /// Scores the current window.
    pub fn score_current_window(
        &mut self,
        run_id: &str,
        scored_at_ms: u64,
    ) -> Result<LiveRlWindowLifecycleReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            let window_id = String::from(current_window_id(run)?);
            run.orchestrator.score_current_window(scored_at_ms)?;
            run.updated_at_ms = scored_at_ms;
            LiveRlWindowLifecycleReceipt {
                receipt_id: format!("{window_id}-scored"),
                run_id: String::from(run.run_id()),
                window_id,
                window_status: TrainingWindowStatus::Scored,
                run_status: run.orchestrator.run.status,
                observed_at_ms: scored_at_ms,
                receipt_digest: stable_live_rl_digest(
                    "window_lifecycle_receipt",
                    &(
                        run.run_id(),
                        current_window_id(run)?,
                        window_status_label(TrainingWindowStatus::Scored),
                        run_status_label(run.orchestrator.run.status),
                        scored_at_ms,
                    ),
                ),
            }
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("window-scored-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Reconciles the current window and finalizes any pending stop request.
    pub fn reconcile_current_window(
        &mut self,
        run_id: &str,
        reconciled_at_ms: u64,
    ) -> Result<LiveRlWindowLifecycleReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            let window_id = String::from(current_window_id(run)?);
            run.orchestrator
                .reconcile_current_window(reconciled_at_ms)?;
            finalize_stop_if_ready(run, reconciled_at_ms);
            run.updated_at_ms = reconciled_at_ms;
            let receipt_digest = stable_live_rl_digest(
                "window_lifecycle_receipt",
                &(
                    run.run_id(),
                    window_id.as_str(),
                    window_status_label(TrainingWindowStatus::Reconciled),
                    run_status_label(run.orchestrator.run.status),
                    reconciled_at_ms,
                ),
            );
            LiveRlWindowLifecycleReceipt {
                receipt_id: format!("{window_id}-reconciled"),
                run_id: String::from(run.run_id()),
                window_id,
                window_status: TrainingWindowStatus::Reconciled,
                run_status: run.orchestrator.run.status,
                observed_at_ms: reconciled_at_ms,
                receipt_digest,
            }
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("window-reconciled-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    /// Requests graceful stop or immediate finalization for the run.
    pub fn stop_run(
        &mut self,
        run_id: &str,
        stop_request: LiveRlRunStopRequest,
    ) -> Result<LiveRlRunStopReceipt, LiveRlRunServiceError> {
        let receipt = {
            let run = self.run_mut(run_id)?;
            run.stop_request = Some(stop_request.clone());
            if run.current_window_id().is_some() {
                run.orchestrator.run.status = TrainingRunStatus::Draining;
            } else {
                finalize_stop_if_ready(run, stop_request.requested_at_ms);
            }
            run.updated_at_ms = stop_request.requested_at_ms;
            LiveRlRunStopReceipt {
                receipt_id: format!("{}-stop", run.run_id()),
                run_id: String::from(run.run_id()),
                stop_kind: stop_request.stop_kind,
                resulting_run_status: run.orchestrator.run.status,
                current_window_id: run.current_window_id().map(String::from),
                observed_at_ms: stop_request.requested_at_ms,
                receipt_digest: stable_live_rl_digest(
                    "run_stop_receipt",
                    &(
                        run.run_id(),
                        stop_kind_label(stop_request.stop_kind),
                        run_status_label(run.orchestrator.run.status),
                        run.current_window_id(),
                        stop_request.requested_at_ms,
                        stop_request.detail.as_deref(),
                    ),
                ),
            }
        };
        self.persist_run(run_id)?;
        self.persist_artifact(
            run_id,
            format!("run-stop-{}", receipt.receipt_digest).as_str(),
            &receipt,
        )?;
        Ok(receipt)
    }

    fn run(&self, run_id: &str) -> Result<&LiveRlManagedRun, LiveRlRunServiceError> {
        self.runs
            .get(run_id)
            .ok_or_else(|| LiveRlRunServiceError::UnknownRun {
                run_id: String::from(run_id),
            })
    }

    fn run_mut(&mut self, run_id: &str) -> Result<&mut LiveRlManagedRun, LiveRlRunServiceError> {
        self.runs
            .get_mut(run_id)
            .ok_or_else(|| LiveRlRunServiceError::UnknownRun {
                run_id: String::from(run_id),
            })
    }

    fn persist_run(&self, run_id: &str) -> Result<(), LiveRlRunServiceError> {
        let run = self.run(run_id)?;
        let run_dir = self.run_dir(run.run_id());
        write_json(run_dir.join("state.json").as_path(), run)?;
        write_json(
            run_dir.join("status.json").as_path(),
            &run.status_artifact(),
        )?;
        for window_artifact in run.window_status_artifacts() {
            write_json(
                run_dir
                    .join("windows")
                    .join(format!("{}.json", window_artifact.window_id))
                    .as_path(),
                &window_artifact,
            )?;
        }
        for failure in &run.failures {
            write_json(
                run_dir
                    .join("failures")
                    .join(format!("{}.json", failure.failure_id))
                    .as_path(),
                failure,
            )?;
        }
        Ok(())
    }

    fn persist_artifact<T: Serialize>(
        &self,
        run_id: &str,
        artifact_stem: &str,
        artifact: &T,
    ) -> Result<(), LiveRlRunServiceError> {
        write_json(
            self.run_dir(run_id)
                .join("artifacts")
                .join(format!("{artifact_stem}.json"))
                .as_path(),
            artifact,
        )
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.config.storage_root.join("runs").join(run_id)
    }
}

fn load_runs(root: &Path) -> Result<BTreeMap<String, LiveRlManagedRun>, LiveRlRunServiceError> {
    let runs_dir = root.join("runs");
    fs::create_dir_all(&runs_dir)?;
    let mut runs = BTreeMap::new();
    for entry in fs::read_dir(&runs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let state_path = entry.path().join("state.json");
        if !state_path.exists() {
            continue;
        }
        let bytes = fs::read(&state_path)?;
        let run: LiveRlManagedRun = serde_json::from_slice(&bytes)?;
        runs.insert(String::from(run.run_id()), run);
    }
    Ok(runs)
}

fn current_window_id(run: &LiveRlManagedRun) -> Result<&str, LiveRlRunServiceError> {
    run.current_window_id()
        .ok_or_else(|| LiveRlRunServiceError::MissingWindowProtocol {
            run_id: String::from(run.run_id()),
            window_id: String::from("missing"),
        })
}

fn current_window_protocol_mut(
    run: &mut LiveRlManagedRun,
) -> Result<&mut RolloutWorkerProtocolState, LiveRlRunServiceError> {
    let run_id = String::from(run.run_id());
    let current_window_id = current_window_id(run)?.to_string();
    run.window_protocols
        .iter_mut()
        .find(|record| record.window_id == current_window_id)
        .map(|record| &mut record.protocol)
        .ok_or_else(|| LiveRlRunServiceError::MissingWindowProtocol {
            run_id,
            window_id: current_window_id,
        })
}

fn finalize_stop_if_ready(run: &mut LiveRlManagedRun, observed_at_ms: u64) {
    if run.current_window_id().is_some() {
        return;
    }
    let Some(stop_request) = run.stop_request.take() else {
        return;
    };
    run.orchestrator.run.status = match stop_request.stop_kind {
        LiveRlRunStopKind::Completed => TrainingRunStatus::Completed,
        LiveRlRunStopKind::Cancelled => TrainingRunStatus::Cancelled,
        LiveRlRunStopKind::Failed => TrainingRunStatus::Failed,
    };
    if stop_request.stop_kind == LiveRlRunStopKind::Failed {
        let detail = stop_request
            .detail
            .clone()
            .unwrap_or_else(|| String::from("live RL run stopped with unspecified failure"));
        let failure_id = format!("{}-failure-{}", run.run_id(), run.failures.len() + 1);
        let failure = LiveRlRunFailureArtifact {
            failure_id: failure_id.clone(),
            run_id: String::from(run.run_id()),
            window_id: None,
            resulting_run_status: TrainingRunStatus::Failed,
            detail: detail.clone(),
            observed_at_ms,
            failure_digest: stable_live_rl_digest(
                "run_failure_artifact",
                &(
                    run.run_id(),
                    failure_id.as_str(),
                    detail.as_str(),
                    observed_at_ms,
                ),
            ),
        };
        run.failures.push(failure);
    }
}

fn window_knows_artifact(
    window: &crate::TrainingOrchestratorWindow,
    artifact_id: &str,
    artifact_digest: &str,
) -> bool {
    window.accepted_rollouts.iter().any(|record| {
        record.reference.artifact_id == artifact_id
            && record.reference.artifact_digest == artifact_digest
    }) || window.quarantined_rollouts.iter().any(|record| {
        record.artifact.artifact_id == artifact_id
            && record.artifact.artifact_digest == artifact_digest
    }) || window.discarded_rollout_receipts.iter().any(|receipt| {
        receipt.artifact_id == artifact_id && receipt.artifact_digest == artifact_digest
    })
}

fn window_status_from_run(run: &TrainingRunState, window_id: &str) -> TrainingWindowStatus {
    run.windows
        .iter()
        .find(|window| window.window_id == window_id)
        .map(|window| window.status)
        .unwrap_or(TrainingWindowStatus::Planned)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), LiveRlRunServiceError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn stable_live_rl_digest<T: Serialize>(namespace: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_live_rl_run_service|");
    hasher.update(namespace.as_bytes());
    hasher.update(b"|");
    hasher.update(
        serde_json::to_vec(value).expect("live RL run service values must serialize for digest"),
    );
    hex::encode(hasher.finalize())
}

fn run_status_label(status: TrainingRunStatus) -> &'static str {
    match status {
        TrainingRunStatus::Initializing => "initializing",
        TrainingRunStatus::Running => "running",
        TrainingRunStatus::Draining => "draining",
        TrainingRunStatus::Completed => "completed",
        TrainingRunStatus::Failed => "failed",
        TrainingRunStatus::Cancelled => "cancelled",
    }
}

fn window_status_label(status: TrainingWindowStatus) -> &'static str {
    match status {
        TrainingWindowStatus::Planned => "planned",
        TrainingWindowStatus::Active => "active",
        TrainingWindowStatus::Sealed => "sealed",
        TrainingWindowStatus::Scored => "scored",
        TrainingWindowStatus::Reconciled => "reconciled",
    }
}

fn stop_kind_label(stop_kind: LiveRlRunStopKind) -> &'static str {
    match stop_kind {
        LiveRlRunStopKind::Completed => "completed",
        LiveRlRunStopKind::Cancelled => "cancelled",
        LiveRlRunStopKind::Failed => "failed",
    }
}

fn validator_disposition_label(disposition: ValidatorDisposition) -> &'static str {
    match disposition {
        ValidatorDisposition::Accepted => "accepted",
        ValidatorDisposition::Normalized => "normalized",
        ValidatorDisposition::Rejected => "rejected",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterSnapshot, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_datastream::{
        DatastreamEncoding, DatastreamManifest, DatastreamPolicyWeightBinding,
        DatastreamSubjectKind, InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
    };
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{
        LiveRlRunCreateRequest, LiveRlRunParticipantPriority, LiveRlRunService,
        LiveRlRunServiceConfig, LiveRlRunStopKind, LiveRlRunStopRequest,
    };
    use crate::{
        PolicyRevision, RolloutArtifact, RolloutProofKind, RolloutProofReference,
        RolloutReceiptOutcome, RolloutSample, RolloutTerminationReason, RolloutUploadLocator,
        RolloutUploadTransport, RolloutWorkerIdentity, RolloutWorkerTrustClass,
        TrainingWindowAssignmentRule, ValidatorDisposition,
    };

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("cluster-rl-service"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([
            (
                NodeId::new("trainer-a"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new("trainer-a"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::CoordinatorOnly,
                        auth_public_key: String::from("trainer-a-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_000)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-b"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new("worker-b"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::ExecutorOnly,
                        auth_public_key: String::from("worker-b-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_001)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
            (
                NodeId::new("worker-c"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id,
                        node_id: NodeId::new("worker-c"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::ExecutorOnly,
                        auth_public_key: String::from("worker-c-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_002)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
        ]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn target_policy_revision() -> PolicyRevision {
        PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100)
            .with_revision_number(7)
            .with_parent_revision_id("policy-rev-6")
    }

    fn off_policy_revision() -> PolicyRevision {
        PolicyRevision::new("train.decoder", "policy-rev-6", "policy-digest-6", 1_050)
            .with_revision_number(6)
            .with_parent_revision_id("policy-rev-5")
    }

    fn policy_weight_broadcast() -> Result<
        psionic_datastream::DatastreamPolicyWeightBroadcastManifest,
        Box<dyn std::error::Error>,
    > {
        let shard_a = b"weights-a".repeat(16);
        let shard_b = b"weights-b".repeat(16);
        let assembled = {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&shard_a);
            bytes.extend_from_slice(&shard_b);
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            hex::encode(hasher.finalize())
        };
        let manifest_a = DatastreamManifest::from_bytes(
            "policy-shard-a",
            DatastreamSubjectKind::PolicyWeights,
            &shard_a,
            8,
            DatastreamEncoding::Safetensors,
        )
        .with_policy_weight_binding(DatastreamPolicyWeightBinding::new(
            "train.decoder",
            7,
            "shard-a",
            0,
            2,
            assembled.clone(),
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
            "train.decoder",
            7,
            "shard-b",
            1,
            2,
            assembled,
            1_000,
            10_000,
        ));
        Ok(InMemoryPolicyWeightBroadcast::new(
            vec![
                InMemoryDatastreamServer::new(manifest_a, shard_a)?,
                InMemoryDatastreamServer::new(manifest_b, shard_b)?,
            ],
            1_500,
        )?
        .broadcast()
        .clone())
    }

    fn rollout_artifact(
        worker_id: &str,
        artifact_id: &str,
        source_policy_revision: PolicyRevision,
        created_at_ms: u64,
    ) -> Result<RolloutArtifact, Box<dyn std::error::Error>> {
        Ok(RolloutArtifact::new(
            artifact_id,
            worker_id,
            psionic_environments::EnvironmentPackageKey::new("oa.weather.train", "2026.03"),
            format!("task-{artifact_id}"),
            source_policy_revision,
            vec![
                RolloutSample::new(1, -0.2, 1.0, 0.8),
                RolloutSample::new(2, -0.1, 0.5, 0.4),
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

    #[test]
    fn live_rl_run_service_round_trips_restart_and_completes_bounded_window(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempdir()?;
        let config = LiveRlRunServiceConfig::new(tempdir.path());
        let mut service = LiveRlRunService::new(config.clone())?;
        let create = service.create_run(
            LiveRlRunCreateRequest::new(
                "run-rl-service-1",
                "stage-rl",
                "checkpoint-family-weather",
                psionic_environments::EnvironmentPackageKey::new("oa.weather.train", "2026.03"),
                cluster_state(),
                target_policy_revision(),
                policy_weight_broadcast()?,
                1_000,
            )
            .with_participant_priority(LiveRlRunParticipantPriority::new("worker-b", 9_600, 9_500))
            .with_participant_priority(LiveRlRunParticipantPriority::new("worker-c", 9_400, 9_300))
            .with_participant_priority(LiveRlRunParticipantPriority::new(
                "trainer-a",
                7_000,
                8_100,
            )),
        )?;
        assert_eq!(create.run_status, crate::TrainingRunStatus::Running);

        let planned = service.plan_next_window(
            "run-rl-service-1",
            2,
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 2,
                eval_slice_count: 1,
            },
            77,
            1_020,
        )?;
        assert_eq!(planned.window_status, crate::TrainingWindowStatus::Planned);
        let active = service.activate_current_window("run-rl-service-1", 1_030)?;
        assert_eq!(active.window_status, crate::TrainingWindowStatus::Active);

        let status = service.status("run-rl-service-1")?;
        assert_eq!(
            status.current_window_id.as_deref(),
            Some("run-rl-service-1-window-1")
        );

        let window_status =
            service.window_status("run-rl-service-1", "run-rl-service-1-window-1")?;
        let assignment_ids = {
            let state_path = tempdir
                .path()
                .join("runs")
                .join("run-rl-service-1")
                .join("state.json");
            let bytes = fs::read(&state_path)?;
            let run: super::LiveRlManagedRun = serde_json::from_slice(&bytes)?;
            let window = run
                .orchestrator
                .orchestrator_windows
                .iter()
                .find(|window| window.window_id == "run-rl-service-1-window-1")
                .expect("planned window must be present");
            let mut ids = window
                .rollout_assignments
                .iter()
                .map(|assignment| {
                    (
                        assignment.contributor_node_id.clone(),
                        assignment.assignment_id.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(window_status.assigned_worker_count, 2);
            vec![
                ids.remove("worker-b")
                    .expect("worker-b assignment must exist"),
                ids.remove("worker-c")
                    .expect("worker-c assignment must exist"),
            ]
        };

        service.record_worker_heartbeat(
            "run-rl-service-1",
            RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::SemiTrustedWorker,
                "unit:worker-b",
            ),
            1_040,
        )?;
        service.record_worker_heartbeat(
            "run-rl-service-1",
            RolloutWorkerIdentity::new(
                "worker-c",
                RolloutWorkerTrustClass::SemiTrustedWorker,
                "unit:worker-c",
            ),
            1_041,
        )?;
        let claim_b = service.claim_assignment(
            "run-rl-service-1",
            "worker-b",
            assignment_ids[0].as_str(),
            1_050,
        )?;
        let claim_c = service.claim_assignment(
            "run-rl-service-1",
            "worker-c",
            assignment_ids[1].as_str(),
            1_051,
        )?;

        drop(service);

        let mut service = LiveRlRunService::new(config)?;
        let reloaded = service.status("run-rl-service-1")?;
        assert_eq!(reloaded.current_window_registered_worker_count, 2);
        assert_eq!(reloaded.current_window_active_claim_count, 2);

        let artifact_b = rollout_artifact(
            "worker-b",
            "artifact-exact",
            target_policy_revision(),
            1_060,
        )?;
        let artifact_c = rollout_artifact(
            "worker-c",
            "artifact-off-policy",
            off_policy_revision(),
            1_061,
        )?;
        let outcome_b = service.submit_claimed_rollout(
            "run-rl-service-1",
            claim_b.claim_id.as_str(),
            artifact_b.clone(),
            RolloutUploadLocator::new(
                RolloutUploadTransport::ExternalReference,
                "artifact://artifact-exact",
                2_048,
                artifact_b.artifact_digest.as_str(),
            ),
            1_070,
        )?;
        let outcome_c = service.submit_claimed_rollout(
            "run-rl-service-1",
            claim_c.claim_id.as_str(),
            artifact_c.clone(),
            RolloutUploadLocator::new(
                RolloutUploadTransport::ExternalReference,
                "artifact://artifact-off-policy",
                2_064,
                artifact_c.artifact_digest.as_str(),
            ),
            1_071,
        )?;
        assert_eq!(
            outcome_b
                .admission_receipt
                .as_ref()
                .map(|receipt| receipt.outcome),
            Some(RolloutReceiptOutcome::AcceptedExact)
        );
        assert_eq!(
            outcome_c
                .admission_receipt
                .as_ref()
                .map(|receipt| receipt.outcome),
            Some(RolloutReceiptOutcome::AcceptedOffPolicy)
        );

        let verdict_b = service.ingest_validator_bundle(
            "run-rl-service-1",
            crate::RolloutVerificationBundle::new(
                "bundle-exact",
                artifact_b.clone(),
                outcome_b.clone(),
                None,
                None,
            ),
            1_080,
        )?;
        let verdict_c = service.ingest_validator_bundle(
            "run-rl-service-1",
            crate::RolloutVerificationBundle::new(
                "bundle-off-policy",
                artifact_c.clone(),
                outcome_c.clone(),
                None,
                None,
            ),
            1_081,
        )?;
        assert_eq!(verdict_b.disposition, ValidatorDisposition::Accepted);
        assert_eq!(verdict_c.disposition, ValidatorDisposition::Accepted);

        service.seal_current_window("run-rl-service-1", 1_090)?;
        let batch = service.assemble_trainer_batch(
            "run-rl-service-1",
            "trainer-batch-1",
            vec![
                artifact_b.artifact_id.clone(),
                artifact_c.artifact_id.clone(),
            ],
            1_100,
        )?;
        assert_eq!(batch.batch.rollout_count, 2);
        service.score_current_window("run-rl-service-1", 1_110)?;
        let draining = service.stop_run(
            "run-rl-service-1",
            LiveRlRunStopRequest::new(LiveRlRunStopKind::Completed, 1_120),
        )?;
        assert_eq!(
            draining.resulting_run_status,
            crate::TrainingRunStatus::Draining
        );
        let reconciled = service.reconcile_current_window("run-rl-service-1", 1_130)?;
        assert_eq!(reconciled.run_status, crate::TrainingRunStatus::Completed);

        let final_status = service.status("run-rl-service-1")?;
        assert_eq!(final_status.run_status, crate::TrainingRunStatus::Completed);
        assert_eq!(final_status.current_window_id, None);
        assert_eq!(final_status.trainer_batch_count, 1);
        assert_eq!(final_status.validator_accepted_count, 2);

        let status_path = tempdir
            .path()
            .join("runs")
            .join("run-rl-service-1")
            .join("status.json");
        let window_path = tempdir
            .path()
            .join("runs")
            .join("run-rl-service-1")
            .join("windows")
            .join("run-rl-service-1-window-1.json");
        let artifacts_dir = tempdir
            .path()
            .join("runs")
            .join("run-rl-service-1")
            .join("artifacts");
        assert!(status_path.exists());
        assert!(window_path.exists());
        assert!(fs::read_dir(&artifacts_dir)?.next().is_some());

        Ok(())
    }

    #[test]
    fn live_rl_run_service_emits_failure_artifact_for_failed_stop(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempdir()?;
        let mut service = LiveRlRunService::new(LiveRlRunServiceConfig::new(tempdir.path()))?;
        service.create_run(
            LiveRlRunCreateRequest::new(
                "run-rl-service-failed",
                "stage-rl",
                "checkpoint-family-weather",
                psionic_environments::EnvironmentPackageKey::new("oa.weather.train", "2026.03"),
                cluster_state(),
                target_policy_revision(),
                policy_weight_broadcast()?,
                2_000,
            )
            .with_participant_priority(LiveRlRunParticipantPriority::new("worker-b", 9_600, 9_500)),
        )?;
        let stop = service.stop_run(
            "run-rl-service-failed",
            LiveRlRunStopRequest::new(LiveRlRunStopKind::Failed, 2_010)
                .with_detail("validator quorum lost"),
        )?;
        assert_eq!(stop.resulting_run_status, crate::TrainingRunStatus::Failed);

        let failure_dir = tempdir
            .path()
            .join("runs")
            .join("run-rl-service-failed")
            .join("failures");
        let entries = fs::read_dir(&failure_dir)?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(entries.len(), 1);
        Ok(())
    }
}
