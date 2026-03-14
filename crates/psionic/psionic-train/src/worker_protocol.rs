use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PolicyRevision, RolloutAdmissionReceipt, RolloutArtifact, RolloutReceiptOutcome,
    RolloutWorkAssignment, TrainingOrchestratorError, TrainingOrchestratorState,
    TrainingOrchestratorWindow,
};

/// Error returned by the rollout-worker protocol.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RolloutWorkerProtocolError {
    /// One assignment id was not present in the active window.
    #[error("unknown rollout assignment `{assignment_id}`")]
    UnknownAssignment {
        /// Stable assignment identifier.
        assignment_id: String,
    },
    /// One worker attempted a claim without a registered heartbeat.
    #[error("worker `{worker_id}` has no active rollout-worker heartbeat")]
    UnknownWorker {
        /// Stable worker identifier.
        worker_id: String,
    },
    /// One worker attempted to claim another worker's assignment.
    #[error(
        "rollout assignment `{assignment_id}` belongs to worker `{expected_worker_id}` but claim was attempted by `{actual_worker_id}`"
    )]
    AssignmentWorkerMismatch {
        /// Stable assignment identifier.
        assignment_id: String,
        /// Assigned worker id.
        expected_worker_id: String,
        /// Claiming worker id.
        actual_worker_id: String,
    },
    /// The worker heartbeat is too old for a fresh claim.
    #[error(
        "worker `{worker_id}` heartbeat at {last_heartbeat_at_ms} is stale for claim time {claimed_at_ms}"
    )]
    WorkerHeartbeatStale {
        /// Stable worker identifier.
        worker_id: String,
        /// Last heartbeat.
        last_heartbeat_at_ms: u64,
        /// Claim time.
        claimed_at_ms: u64,
    },
    /// One assignment already has an active claim.
    #[error(
        "rollout assignment `{assignment_id}` is already claimed by `{claim_id}` until {claim_expires_at_ms}"
    )]
    AssignmentAlreadyClaimed {
        /// Stable assignment identifier.
        assignment_id: String,
        /// Stable claim identifier.
        claim_id: String,
        /// Claim expiry.
        claim_expires_at_ms: u64,
    },
    /// One claim id was not present in protocol state.
    #[error("unknown rollout task claim `{claim_id}`")]
    UnknownClaim {
        /// Stable claim identifier.
        claim_id: String,
    },
    /// One claim is no longer active.
    #[error("rollout task claim `{claim_id}` is not active; found `{status}`")]
    ClaimNotActive {
        /// Stable claim identifier.
        claim_id: String,
        /// Current claim status.
        status: String,
    },
    /// The uploaded artifact did not match the claiming worker.
    #[error(
        "rollout artifact `{artifact_id}` belongs to worker `{artifact_worker_id}` but claim `{claim_id}` belongs to `{claim_worker_id}`"
    )]
    ArtifactWorkerMismatch {
        /// Stable artifact identifier.
        artifact_id: String,
        /// Worker id on the artifact.
        artifact_worker_id: String,
        /// Stable claim identifier.
        claim_id: String,
        /// Worker id on the claim.
        claim_worker_id: String,
    },
    /// The upload object digest did not match the artifact digest.
    #[error(
        "rollout upload for claim `{claim_id}` declared object digest `{declared_object_digest}` but artifact digest is `{artifact_digest}`"
    )]
    UploadDigestMismatch {
        /// Stable claim identifier.
        claim_id: String,
        /// Declared upload object digest.
        declared_object_digest: String,
        /// Artifact digest.
        artifact_digest: String,
    },
    /// The orchestrator rejected the rollout for structural reasons.
    #[error(transparent)]
    Orchestrator(#[from] TrainingOrchestratorError),
}

/// Trust posture for one rollout-producing worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutWorkerTrustClass {
    /// Trusted trainer or coordinator role.
    TrustedTrainer,
    /// Semi-trusted dedicated rollout worker.
    SemiTrustedWorker,
    /// Untrusted or permissionless rollout worker.
    UntrustedWorker,
}

/// Stable identity for one rollout worker session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutWorkerIdentity {
    /// Stable worker identifier.
    pub worker_id: String,
    /// Trust posture for this worker.
    pub trust_class: RolloutWorkerTrustClass,
    /// Stable auth subject or operator-visible identity string.
    pub auth_subject: String,
}

impl RolloutWorkerIdentity {
    /// Creates a rollout worker identity.
    #[must_use]
    pub fn new(
        worker_id: impl Into<String>,
        trust_class: RolloutWorkerTrustClass,
        auth_subject: impl Into<String>,
    ) -> Self {
        Self {
            worker_id: worker_id.into(),
            trust_class,
            auth_subject: auth_subject.into(),
        }
    }
}

/// Control policy for rollout-worker heartbeats, claims, and uploads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutWorkerProtocolPolicy {
    /// Maximum time a heartbeat remains fresh for new claims.
    pub heartbeat_timeout_ms: u64,
    /// Maximum lifetime for one active task claim.
    pub claim_ttl_ms: u64,
    /// Maximum inline upload size before the worker must use an external handle.
    pub max_inline_upload_bytes: u64,
}

impl Default for RolloutWorkerProtocolPolicy {
    fn default() -> Self {
        Self {
            heartbeat_timeout_ms: 10_000,
            claim_ttl_ms: 20_000,
            max_inline_upload_bytes: 64 * 1024,
        }
    }
}

/// Active session state for one rollout worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutWorkerSession {
    /// Stable worker identity.
    pub identity: RolloutWorkerIdentity,
    /// Latest heartbeat timestamp.
    pub last_heartbeat_at_ms: u64,
}

/// Typed heartbeat receipt for one rollout worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutWorkerHeartbeatReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Trust posture surfaced by the protocol.
    pub trust_class: RolloutWorkerTrustClass,
    /// Observed heartbeat time.
    pub observed_at_ms: u64,
    /// Next required heartbeat time under the current policy.
    pub next_required_heartbeat_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Lifecycle state for one rollout-task claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutTaskClaimStatus {
    /// Claim is active and may still upload a rollout.
    Active,
    /// Claim completed with one upload outcome.
    Uploaded,
    /// Claim expired before upload completed.
    Expired,
    /// Claim was rejected locally by upload policy.
    Rejected,
}

/// Deterministic claim for one rollout assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutTaskClaim {
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Trust posture for the claiming worker.
    pub trust_class: RolloutWorkerTrustClass,
    /// Target policy revision expected by the assignment.
    pub target_policy_revision_id: String,
    /// Target weight-broadcast digest expected by the assignment.
    pub policy_weight_broadcast_digest: String,
    /// Deterministic seed for sample selection or environment randomization.
    pub sample_selection_seed: u64,
    /// Monotonic attempt index for the assignment.
    pub attempt_index: u32,
    /// Claim start time.
    pub claimed_at_ms: u64,
    /// Claim expiry time.
    pub claim_expires_at_ms: u64,
    /// Claim lifecycle state.
    pub status: RolloutTaskClaimStatus,
    /// Stable claim digest.
    pub claim_digest: String,
}

/// Upload transport chosen by one rollout worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutUploadTransport {
    /// Inline payload delivery.
    InlineArtifact,
    /// External object or manifest handle.
    ExternalReference,
}

/// Typed rollout upload locator returned by the worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutUploadLocator {
    /// Transport family.
    pub transport: RolloutUploadTransport,
    /// Stable external handle or inline placeholder.
    pub reference: String,
    /// Declared payload size.
    pub payload_bytes: u64,
    /// Declared object digest for the uploaded artifact.
    pub object_digest: String,
    /// Stable upload digest.
    pub upload_digest: String,
}

impl RolloutUploadLocator {
    /// Creates an upload locator.
    #[must_use]
    pub fn new(
        transport: RolloutUploadTransport,
        reference: impl Into<String>,
        payload_bytes: u64,
        object_digest: impl Into<String>,
    ) -> Self {
        let reference = reference.into();
        let object_digest = object_digest.into();
        let upload_digest = stable_upload_digest(
            transport,
            reference.as_str(),
            payload_bytes,
            object_digest.as_str(),
        );
        Self {
            transport,
            reference,
            payload_bytes,
            object_digest,
            upload_digest,
        }
    }
}

/// Policy freshness posture observed when one worker uploads a rollout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutWorkerPolicyPosture {
    /// The worker used the exact target policy revision.
    ExactTarget,
    /// The worker used an older policy revision but remained admissible.
    AcceptedOffPolicy,
    /// The worker used an older policy revision and was quarantined.
    QuarantinedOffPolicy,
    /// The worker used an older policy revision and was discarded.
    DiscardedOffPolicy,
}

/// Final upload outcome for one worker claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutWorkerOutcomeKind {
    /// The worker uploaded an exact-policy rollout and it was accepted.
    UploadedAcceptedExact,
    /// The worker uploaded a bounded off-policy rollout and it was accepted.
    UploadedAcceptedOffPolicy,
    /// The worker uploaded a rollout that was quarantined.
    UploadedQuarantined,
    /// The worker uploaded a rollout that was discarded.
    UploadedDiscarded,
    /// The worker missed the claim deadline.
    ClaimExpired,
    /// The worker violated upload policy before orchestrator admission.
    UploadRuleRejected,
}

/// Machine-readable rejection reason for non-admitted worker outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutWorkerRejectionReason {
    /// Inline upload exceeded policy.
    InlineUploadTooLarge,
    /// Claim expired before upload.
    ClaimExpired,
}

/// Final worker outcome receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutWorkerOutcomeReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Worker trust posture.
    pub trust_class: RolloutWorkerTrustClass,
    /// Target policy revision from the claim.
    pub target_policy_revision_id: String,
    /// Source policy revision from the uploaded artifact.
    pub source_policy_revision_id: String,
    /// Final protocol outcome.
    pub outcome: RolloutWorkerOutcomeKind,
    /// Final policy posture for the upload.
    pub policy_posture: RolloutWorkerPolicyPosture,
    /// Upload locator surfaced by the worker.
    pub upload: RolloutUploadLocator,
    /// Optional local rejection reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<RolloutWorkerRejectionReason>,
    /// Admission receipt returned by the orchestrator when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_receipt: Option<RolloutAdmissionReceipt>,
    /// Outcome timestamp.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stateful rollout-worker protocol over one orchestrated window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutWorkerProtocolState {
    /// Stable window identifier.
    pub window_id: String,
    /// Stable assignment seed used for deterministic claim seeds.
    pub assignment_seed: u64,
    /// Active target policy revision.
    pub target_policy_revision: PolicyRevision,
    /// Active policy-weight broadcast digest.
    pub policy_weight_broadcast_digest: String,
    /// Protocol-level control policy.
    pub policy: RolloutWorkerProtocolPolicy,
    /// Known rollout assignments for this window.
    pub assignments: Vec<RolloutWorkAssignment>,
    /// Known worker sessions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workers: Vec<RolloutWorkerSession>,
    /// Claim history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<RolloutTaskClaim>,
    /// Outcome receipts emitted by the protocol.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcome_receipts: Vec<RolloutWorkerOutcomeReceipt>,
}

impl RolloutWorkerProtocolState {
    /// Creates a rollout-worker protocol over one orchestrated window.
    #[must_use]
    pub fn from_window(
        window: &TrainingOrchestratorWindow,
        target_policy_revision: PolicyRevision,
        policy: RolloutWorkerProtocolPolicy,
    ) -> Self {
        Self {
            window_id: window.window_id.clone(),
            assignment_seed: window.assignment_posture.assignment_seed,
            target_policy_revision,
            policy_weight_broadcast_digest: window
                .assignment_posture
                .policy_weight_broadcast_digest
                .clone(),
            policy,
            assignments: window.rollout_assignments.clone(),
            workers: Vec::new(),
            claims: Vec::new(),
            outcome_receipts: Vec::new(),
        }
    }

    /// Records or refreshes one rollout-worker heartbeat.
    pub fn record_heartbeat(
        &mut self,
        identity: RolloutWorkerIdentity,
        observed_at_ms: u64,
    ) -> RolloutWorkerHeartbeatReceipt {
        match self
            .workers
            .iter_mut()
            .find(|session| session.identity.worker_id == identity.worker_id)
        {
            Some(session) => {
                session.identity = identity.clone();
                session.last_heartbeat_at_ms = observed_at_ms;
            }
            None => self.workers.push(RolloutWorkerSession {
                identity: identity.clone(),
                last_heartbeat_at_ms: observed_at_ms,
            }),
        }
        let receipt_id = format!("{}-heartbeat-{}", self.window_id, identity.worker_id);
        RolloutWorkerHeartbeatReceipt {
            receipt_digest: stable_heartbeat_receipt_digest(
                self.window_id.as_str(),
                identity.worker_id.as_str(),
                identity.trust_class,
                observed_at_ms,
                observed_at_ms.saturating_add(self.policy.heartbeat_timeout_ms),
            ),
            receipt_id,
            window_id: self.window_id.clone(),
            worker_id: identity.worker_id,
            trust_class: identity.trust_class,
            observed_at_ms,
            next_required_heartbeat_at_ms: observed_at_ms
                .saturating_add(self.policy.heartbeat_timeout_ms),
        }
    }

    /// Claims one rollout assignment after verifying heartbeat freshness.
    pub fn claim_assignment(
        &mut self,
        worker_id: &str,
        assignment_id: &str,
        claimed_at_ms: u64,
    ) -> Result<RolloutTaskClaim, RolloutWorkerProtocolError> {
        self.expire_stale_claims(claimed_at_ms);
        let session = self
            .workers
            .iter()
            .find(|session| session.identity.worker_id == worker_id)
            .ok_or_else(|| RolloutWorkerProtocolError::UnknownWorker {
                worker_id: String::from(worker_id),
            })?;
        if claimed_at_ms.saturating_sub(session.last_heartbeat_at_ms)
            > self.policy.heartbeat_timeout_ms
        {
            return Err(RolloutWorkerProtocolError::WorkerHeartbeatStale {
                worker_id: String::from(worker_id),
                last_heartbeat_at_ms: session.last_heartbeat_at_ms,
                claimed_at_ms,
            });
        }
        let assignment = self
            .assignments
            .iter()
            .find(|assignment| assignment.assignment_id == assignment_id)
            .ok_or_else(|| RolloutWorkerProtocolError::UnknownAssignment {
                assignment_id: String::from(assignment_id),
            })?;
        if assignment.contributor_node_id != worker_id {
            return Err(RolloutWorkerProtocolError::AssignmentWorkerMismatch {
                assignment_id: assignment.assignment_id.clone(),
                expected_worker_id: assignment.contributor_node_id.clone(),
                actual_worker_id: String::from(worker_id),
            });
        }
        if let Some(existing_claim) = self.claims.iter().find(|claim| {
            claim.assignment_id == assignment_id && claim.status == RolloutTaskClaimStatus::Active
        }) {
            return Err(RolloutWorkerProtocolError::AssignmentAlreadyClaimed {
                assignment_id: String::from(assignment_id),
                claim_id: existing_claim.claim_id.clone(),
                claim_expires_at_ms: existing_claim.claim_expires_at_ms,
            });
        }
        let attempt_index = self
            .claims
            .iter()
            .filter(|claim| claim.assignment_id == assignment_id)
            .count() as u32
            + 1;
        let sample_selection_seed = stable_sample_selection_seed(
            self.assignment_seed,
            assignment.assignment_digest.as_str(),
            worker_id,
            attempt_index,
        );
        let claim = RolloutTaskClaim {
            claim_id: format!("{assignment_id}-claim-{attempt_index}"),
            assignment_id: assignment.assignment_id.clone(),
            window_id: assignment.window_id.clone(),
            worker_id: String::from(worker_id),
            trust_class: session.identity.trust_class,
            target_policy_revision_id: assignment.policy_revision_id.clone(),
            policy_weight_broadcast_digest: assignment.policy_weight_broadcast_digest.clone(),
            sample_selection_seed,
            attempt_index,
            claimed_at_ms,
            claim_expires_at_ms: claimed_at_ms.saturating_add(self.policy.claim_ttl_ms),
            status: RolloutTaskClaimStatus::Active,
            claim_digest: stable_claim_digest(
                assignment.assignment_id.as_str(),
                assignment.window_id.as_str(),
                worker_id,
                assignment.policy_revision_id.as_str(),
                assignment.policy_weight_broadcast_digest.as_str(),
                sample_selection_seed,
                attempt_index,
                claimed_at_ms,
                claimed_at_ms.saturating_add(self.policy.claim_ttl_ms),
            ),
        };
        self.claims.push(claim);
        self.claims
            .last()
            .cloned()
            .ok_or_else(|| RolloutWorkerProtocolError::UnknownClaim {
                claim_id: format!("{assignment_id}-claim-{attempt_index}"),
            })
    }

    /// Submits one claimed rollout through upload policy and orchestrator admission.
    pub fn submit_claimed_rollout(
        &mut self,
        orchestrator: &mut TrainingOrchestratorState,
        claim_id: &str,
        artifact: RolloutArtifact,
        upload: RolloutUploadLocator,
        observed_at_ms: u64,
    ) -> Result<RolloutWorkerOutcomeReceipt, RolloutWorkerProtocolError> {
        self.expire_stale_claims(observed_at_ms);
        let claim_index = self
            .claims
            .iter()
            .position(|claim| claim.claim_id == claim_id)
            .ok_or_else(|| RolloutWorkerProtocolError::UnknownClaim {
                claim_id: String::from(claim_id),
            })?;
        let claim = self.claims[claim_index].clone();
        if artifact.worker_id != claim.worker_id {
            return Err(RolloutWorkerProtocolError::ArtifactWorkerMismatch {
                artifact_id: artifact.artifact_id.clone(),
                artifact_worker_id: artifact.worker_id.clone(),
                claim_id: claim.claim_id.clone(),
                claim_worker_id: claim.worker_id.clone(),
            });
        }
        if upload.object_digest != artifact.artifact_digest {
            return Err(RolloutWorkerProtocolError::UploadDigestMismatch {
                claim_id: claim.claim_id.clone(),
                declared_object_digest: upload.object_digest.clone(),
                artifact_digest: artifact.artifact_digest.clone(),
            });
        }
        match claim.status {
            RolloutTaskClaimStatus::Active => {}
            RolloutTaskClaimStatus::Expired => {
                let receipt = self.build_local_outcome_receipt(
                    &claim,
                    artifact.source_policy_revision.revision_id.as_str(),
                    RolloutWorkerOutcomeKind::ClaimExpired,
                    RolloutWorkerPolicyPosture::ExactTarget,
                    upload,
                    Some(RolloutWorkerRejectionReason::ClaimExpired),
                    None,
                    observed_at_ms,
                );
                self.outcome_receipts.push(receipt.clone());
                return Ok(receipt);
            }
            status => {
                return Err(RolloutWorkerProtocolError::ClaimNotActive {
                    claim_id: claim.claim_id,
                    status: rollout_task_claim_status_label(status).to_string(),
                });
            }
        }
        if observed_at_ms > claim.claim_expires_at_ms {
            let receipt = self.build_local_outcome_receipt(
                &claim,
                artifact.source_policy_revision.revision_id.as_str(),
                RolloutWorkerOutcomeKind::ClaimExpired,
                RolloutWorkerPolicyPosture::ExactTarget,
                upload,
                Some(RolloutWorkerRejectionReason::ClaimExpired),
                None,
                observed_at_ms,
            );
            self.claims[claim_index].status = RolloutTaskClaimStatus::Expired;
            self.outcome_receipts.push(receipt.clone());
            return Ok(receipt);
        }
        if upload.transport == RolloutUploadTransport::InlineArtifact
            && upload.payload_bytes > self.policy.max_inline_upload_bytes
        {
            let receipt = self.build_local_outcome_receipt(
                &claim,
                artifact.source_policy_revision.revision_id.as_str(),
                RolloutWorkerOutcomeKind::UploadRuleRejected,
                policy_posture_for_claim(
                    &claim,
                    artifact.source_policy_revision.revision_id.as_str(),
                    RolloutReceiptOutcome::Discarded,
                ),
                upload,
                Some(RolloutWorkerRejectionReason::InlineUploadTooLarge),
                None,
                observed_at_ms,
            );
            self.claims[claim_index].status = RolloutTaskClaimStatus::Rejected;
            self.outcome_receipts.push(receipt.clone());
            return Ok(receipt);
        }

        let admission_receipt = orchestrator.submit_rollout(artifact, observed_at_ms)?;
        let outcome = worker_outcome_from_admission(admission_receipt.outcome);
        let source_policy_revision_id = admission_receipt.source_policy_revision_id.clone();
        let policy_posture = policy_posture_for_claim(
            &claim,
            source_policy_revision_id.as_str(),
            admission_receipt.outcome,
        );
        let receipt = self.build_local_outcome_receipt(
            &claim,
            source_policy_revision_id.as_str(),
            outcome,
            policy_posture,
            upload,
            None,
            Some(admission_receipt),
            observed_at_ms,
        );
        self.claims[claim_index].status = RolloutTaskClaimStatus::Uploaded;
        self.outcome_receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn expire_stale_claims(&mut self, observed_at_ms: u64) {
        for claim in &mut self.claims {
            if claim.status == RolloutTaskClaimStatus::Active
                && observed_at_ms > claim.claim_expires_at_ms
            {
                claim.status = RolloutTaskClaimStatus::Expired;
            }
        }
    }

    fn build_local_outcome_receipt(
        &self,
        claim: &RolloutTaskClaim,
        source_policy_revision_id: &str,
        outcome: RolloutWorkerOutcomeKind,
        policy_posture: RolloutWorkerPolicyPosture,
        upload: RolloutUploadLocator,
        rejection_reason: Option<RolloutWorkerRejectionReason>,
        admission_receipt: Option<RolloutAdmissionReceipt>,
        observed_at_ms: u64,
    ) -> RolloutWorkerOutcomeReceipt {
        let receipt_id = format!("{}-outcome", claim.claim_id);
        let receipt_digest = stable_worker_outcome_digest(
            claim.claim_id.as_str(),
            claim.assignment_id.as_str(),
            claim.window_id.as_str(),
            claim.worker_id.as_str(),
            claim.trust_class,
            claim.target_policy_revision_id.as_str(),
            source_policy_revision_id,
            outcome,
            policy_posture,
            upload.upload_digest.as_str(),
            rejection_reason,
            admission_receipt
                .as_ref()
                .map(|receipt| receipt.receipt_digest.as_str()),
            observed_at_ms,
        );
        RolloutWorkerOutcomeReceipt {
            receipt_id,
            claim_id: claim.claim_id.clone(),
            assignment_id: claim.assignment_id.clone(),
            window_id: claim.window_id.clone(),
            worker_id: claim.worker_id.clone(),
            trust_class: claim.trust_class,
            target_policy_revision_id: claim.target_policy_revision_id.clone(),
            source_policy_revision_id: String::from(source_policy_revision_id),
            outcome,
            policy_posture,
            upload,
            rejection_reason,
            admission_receipt,
            observed_at_ms,
            receipt_digest,
        }
    }
}

fn stable_sample_selection_seed(
    assignment_seed: u64,
    assignment_digest: &str,
    worker_id: &str,
    attempt_index: u32,
) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_sample_selection_seed|");
    hasher.update(assignment_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(assignment_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(attempt_index.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

fn stable_heartbeat_receipt_digest(
    window_id: &str,
    worker_id: &str,
    trust_class: RolloutWorkerTrustClass,
    observed_at_ms: u64,
    next_required_heartbeat_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_worker_heartbeat|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(rollout_worker_trust_class_label(trust_class));
    hasher.update(b"|");
    hasher.update(observed_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(next_required_heartbeat_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_claim_digest(
    assignment_id: &str,
    window_id: &str,
    worker_id: &str,
    target_policy_revision_id: &str,
    policy_weight_broadcast_digest: &str,
    sample_selection_seed: u64,
    attempt_index: u32,
    claimed_at_ms: u64,
    claim_expires_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_task_claim|");
    hasher.update(assignment_id.as_bytes());
    hasher.update(b"|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(policy_weight_broadcast_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(sample_selection_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(attempt_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(claimed_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(claim_expires_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_upload_digest(
    transport: RolloutUploadTransport,
    reference: &str,
    payload_bytes: u64,
    object_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_upload_locator|");
    hasher.update(rollout_upload_transport_label(transport));
    hasher.update(b"|");
    hasher.update(reference.as_bytes());
    hasher.update(b"|");
    hasher.update(payload_bytes.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(object_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_worker_outcome_digest(
    claim_id: &str,
    assignment_id: &str,
    window_id: &str,
    worker_id: &str,
    trust_class: RolloutWorkerTrustClass,
    target_policy_revision_id: &str,
    source_policy_revision_id: &str,
    outcome: RolloutWorkerOutcomeKind,
    policy_posture: RolloutWorkerPolicyPosture,
    upload_digest: &str,
    rejection_reason: Option<RolloutWorkerRejectionReason>,
    admission_receipt_digest: Option<&str>,
    observed_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_worker_outcome|");
    hasher.update(claim_id.as_bytes());
    hasher.update(b"|");
    hasher.update(assignment_id.as_bytes());
    hasher.update(b"|");
    hasher.update(window_id.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(rollout_worker_trust_class_label(trust_class));
    hasher.update(b"|");
    hasher.update(target_policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(source_policy_revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(rollout_worker_outcome_kind_label(outcome));
    hasher.update(b"|");
    hasher.update(rollout_worker_policy_posture_label(policy_posture));
    hasher.update(b"|");
    hasher.update(upload_digest.as_bytes());
    if let Some(rejection_reason) = rejection_reason {
        hasher.update(b"|rejection|");
        hasher.update(rollout_worker_rejection_reason_label(rejection_reason));
    }
    if let Some(admission_receipt_digest) = admission_receipt_digest {
        hasher.update(b"|admission|");
        hasher.update(admission_receipt_digest.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(observed_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn worker_outcome_from_admission(outcome: RolloutReceiptOutcome) -> RolloutWorkerOutcomeKind {
    match outcome {
        RolloutReceiptOutcome::AcceptedExact => RolloutWorkerOutcomeKind::UploadedAcceptedExact,
        RolloutReceiptOutcome::AcceptedOffPolicy => {
            RolloutWorkerOutcomeKind::UploadedAcceptedOffPolicy
        }
        RolloutReceiptOutcome::Quarantined => RolloutWorkerOutcomeKind::UploadedQuarantined,
        RolloutReceiptOutcome::Discarded => RolloutWorkerOutcomeKind::UploadedDiscarded,
    }
}

fn policy_posture_for_claim(
    claim: &RolloutTaskClaim,
    source_policy_revision_id: &str,
    admission_outcome: RolloutReceiptOutcome,
) -> RolloutWorkerPolicyPosture {
    if source_policy_revision_id == claim.target_policy_revision_id {
        return RolloutWorkerPolicyPosture::ExactTarget;
    }
    match admission_outcome {
        RolloutReceiptOutcome::AcceptedExact | RolloutReceiptOutcome::AcceptedOffPolicy => {
            RolloutWorkerPolicyPosture::AcceptedOffPolicy
        }
        RolloutReceiptOutcome::Quarantined => RolloutWorkerPolicyPosture::QuarantinedOffPolicy,
        RolloutReceiptOutcome::Discarded => RolloutWorkerPolicyPosture::DiscardedOffPolicy,
    }
}

fn rollout_task_claim_status_label(status: RolloutTaskClaimStatus) -> &'static str {
    match status {
        RolloutTaskClaimStatus::Active => "active",
        RolloutTaskClaimStatus::Uploaded => "uploaded",
        RolloutTaskClaimStatus::Expired => "expired",
        RolloutTaskClaimStatus::Rejected => "rejected",
    }
}

fn rollout_worker_trust_class_label(trust_class: RolloutWorkerTrustClass) -> &'static [u8] {
    match trust_class {
        RolloutWorkerTrustClass::TrustedTrainer => b"trusted_trainer",
        RolloutWorkerTrustClass::SemiTrustedWorker => b"semi_trusted_worker",
        RolloutWorkerTrustClass::UntrustedWorker => b"untrusted_worker",
    }
}

fn rollout_upload_transport_label(transport: RolloutUploadTransport) -> &'static [u8] {
    match transport {
        RolloutUploadTransport::InlineArtifact => b"inline_artifact",
        RolloutUploadTransport::ExternalReference => b"external_reference",
    }
}

fn rollout_worker_policy_posture_label(posture: RolloutWorkerPolicyPosture) -> &'static [u8] {
    match posture {
        RolloutWorkerPolicyPosture::ExactTarget => b"exact_target",
        RolloutWorkerPolicyPosture::AcceptedOffPolicy => b"accepted_off_policy",
        RolloutWorkerPolicyPosture::QuarantinedOffPolicy => b"quarantined_off_policy",
        RolloutWorkerPolicyPosture::DiscardedOffPolicy => b"discarded_off_policy",
    }
}

fn rollout_worker_outcome_kind_label(kind: RolloutWorkerOutcomeKind) -> &'static [u8] {
    match kind {
        RolloutWorkerOutcomeKind::UploadedAcceptedExact => b"uploaded_accepted_exact",
        RolloutWorkerOutcomeKind::UploadedAcceptedOffPolicy => b"uploaded_accepted_off_policy",
        RolloutWorkerOutcomeKind::UploadedQuarantined => b"uploaded_quarantined",
        RolloutWorkerOutcomeKind::UploadedDiscarded => b"uploaded_discarded",
        RolloutWorkerOutcomeKind::ClaimExpired => b"claim_expired",
        RolloutWorkerOutcomeKind::UploadRuleRejected => b"upload_rule_rejected",
    }
}

fn rollout_worker_rejection_reason_label(reason: RolloutWorkerRejectionReason) -> &'static [u8] {
    match reason {
        RolloutWorkerRejectionReason::InlineUploadTooLarge => b"inline_upload_too_large",
        RolloutWorkerRejectionReason::ClaimExpired => b"claim_expired",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterSnapshot, NodeEpoch, NodeId, NodeRole,
    };
    use psionic_datastream::{
        DatastreamEncoding, DatastreamPolicyWeightBinding, DatastreamSubjectKind,
        InMemoryDatastreamServer, InMemoryPolicyWeightBroadcast,
    };
    use psionic_environments::EnvironmentPackageKey;
    use sha2::{Digest, Sha256};

    use super::{
        RolloutUploadLocator, RolloutUploadTransport, RolloutWorkerIdentity,
        RolloutWorkerOutcomeKind, RolloutWorkerProtocolError, RolloutWorkerProtocolPolicy,
        RolloutWorkerProtocolState, RolloutWorkerRejectionReason, RolloutWorkerTrustClass,
    };
    use crate::{
        PolicyRevision, RolloutArtifact, RolloutProofKind, RolloutProofReference, RolloutSample,
        TrainingOrchestratorState, TrainingRunState, TrainingWindowAssignmentRule,
    };

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("train-worker-protocol"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = cluster_id();
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
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_100)),
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
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 31_101)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
        ]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
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
        let manifest_a = psionic_datastream::DatastreamManifest::from_bytes(
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
        let manifest_b = psionic_datastream::DatastreamManifest::from_bytes(
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

    fn orchestrator() -> Result<TrainingOrchestratorState, Box<dyn std::error::Error>> {
        let state = cluster_state();
        let environment = EnvironmentPackageKey::new("oa.train", "2026.03");
        let mut run = TrainingRunState::new(
            "run-1",
            "stage-rl",
            state.cluster_id().as_str(),
            "train.decoder",
            environment,
        )?;
        run.apply_cluster_membership_snapshot(&state, 1_000)?;
        run.update_participant_priority(&NodeId::new("worker-b"), 9_200, 9_000, 1_010)?;
        run.update_participant_priority(&NodeId::new("trainer-a"), 8_700, 8_500, 1_020)?;
        Ok(TrainingOrchestratorState::new(
            run,
            PolicyRevision::new("train.decoder", "policy-rev-7", "policy-digest-7", 1_100)
                .with_revision_number(7)
                .with_parent_revision_id("policy-rev-6"),
            policy_weight_broadcast()?,
        )?)
    }

    fn rollout(
        worker_id: &str,
        artifact_id: &str,
        source_policy_revision: PolicyRevision,
    ) -> Result<RolloutArtifact, Box<dyn std::error::Error>> {
        Ok(RolloutArtifact::new(
            artifact_id,
            worker_id,
            EnvironmentPackageKey::new("oa.train", "2026.03"),
            format!("task-{artifact_id}"),
            source_policy_revision,
            vec![
                RolloutSample::new(1, -0.2, 1.0, 0.8),
                RolloutSample::new(2, -0.1, 0.6, 0.4),
            ],
            crate::RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                format!("proof-{artifact_id}"),
                format!("exec://{artifact_id}"),
            )],
            1_120,
        )?)
    }

    fn protocol(
        orchestrator: &mut TrainingOrchestratorState,
    ) -> Result<RolloutWorkerProtocolState, Box<dyn std::error::Error>> {
        let window = orchestrator.plan_next_window(
            2,
            TrainingWindowAssignmentRule::RoundRobinByPriority {
                batch_slice_count: 2,
                eval_slice_count: 0,
            },
            42,
            1_100,
        )?;
        orchestrator.activate_current_window(1_110)?;
        Ok(RolloutWorkerProtocolState::from_window(
            &window,
            orchestrator.target_policy_revision.clone(),
            RolloutWorkerProtocolPolicy::default(),
        ))
    }

    #[test]
    fn worker_protocol_claims_require_recent_heartbeat() -> Result<(), Box<dyn std::error::Error>> {
        let mut orchestrator = orchestrator()?;
        let mut protocol = protocol(&mut orchestrator)?;
        let worker_identity = RolloutWorkerIdentity::new(
            "worker-b",
            RolloutWorkerTrustClass::UntrustedWorker,
            "auth://worker-b",
        );
        let heartbeat = protocol.record_heartbeat(worker_identity, 1_115);
        assert_eq!(heartbeat.worker_id, "worker-b");
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        let claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 1_120)?;
        assert_eq!(claim.worker_id, "worker-b");
        assert!(claim.sample_selection_seed > 0);

        let error = protocol
            .claim_assignment("trainer-a", assignment_id.as_str(), 1_121)
            .expect_err("missing heartbeat should be refused");
        assert_eq!(
            error,
            RolloutWorkerProtocolError::UnknownWorker {
                worker_id: String::from("trainer-a"),
            }
        );
        Ok(())
    }

    #[test]
    fn worker_protocol_wraps_orchestrator_admission_in_worker_outcomes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut orchestrator = orchestrator()?;
        let mut protocol = protocol(&mut orchestrator)?;
        protocol.record_heartbeat(
            RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::SemiTrustedWorker,
                "auth://worker-b",
            ),
            1_115,
        );
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        let claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 1_120)?;
        let artifact = rollout(
            "worker-b",
            "artifact-b",
            PolicyRevision::new("train.decoder", "policy-rev-6", "policy-digest-6", 1_090)
                .with_revision_number(6),
        )?;
        let receipt = protocol.submit_claimed_rollout(
            &mut orchestrator,
            claim.claim_id.as_str(),
            artifact.clone(),
            RolloutUploadLocator::new(
                RolloutUploadTransport::InlineArtifact,
                "inline://artifact-b",
                512,
                artifact.artifact_digest.as_str(),
            ),
            1_125,
        )?;
        assert_eq!(
            receipt.outcome,
            RolloutWorkerOutcomeKind::UploadedAcceptedOffPolicy
        );
        assert!(receipt.admission_receipt.is_some());
        assert_eq!(receipt.worker_id, "worker-b");
        Ok(())
    }

    #[test]
    fn worker_protocol_emits_local_receipts_for_expired_or_invalid_uploads()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut orchestrator = orchestrator()?;
        let mut protocol = protocol(&mut orchestrator)?;
        protocol.record_heartbeat(
            RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::UntrustedWorker,
                "auth://worker-b",
            ),
            1_115,
        );
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        let claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 1_120)?;
        let exact_policy = orchestrator.target_policy_revision.clone();
        let expired_artifact = rollout("worker-b", "artifact-expired", exact_policy)?;
        let expired = protocol.submit_claimed_rollout(
            &mut orchestrator,
            claim.claim_id.as_str(),
            expired_artifact.clone(),
            RolloutUploadLocator::new(
                RolloutUploadTransport::InlineArtifact,
                "inline://artifact-expired",
                512,
                expired_artifact.artifact_digest.as_str(),
            ),
            50_000,
        )?;
        assert_eq!(expired.outcome, RolloutWorkerOutcomeKind::ClaimExpired);
        assert_eq!(
            expired.rejection_reason,
            Some(RolloutWorkerRejectionReason::ClaimExpired)
        );

        protocol.record_heartbeat(
            RolloutWorkerIdentity::new(
                "worker-b",
                RolloutWorkerTrustClass::UntrustedWorker,
                "auth://worker-b",
            ),
            50_010,
        );
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        let claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 50_011)?;
        let exact_policy = orchestrator.target_policy_revision.clone();
        let large_artifact = rollout("worker-b", "artifact-large", exact_policy)?;
        let rejected = protocol.submit_claimed_rollout(
            &mut orchestrator,
            claim.claim_id.as_str(),
            large_artifact.clone(),
            RolloutUploadLocator::new(
                RolloutUploadTransport::InlineArtifact,
                "inline://artifact-large",
                128 * 1024,
                large_artifact.artifact_digest.as_str(),
            ),
            50_015,
        )?;
        assert_eq!(
            rejected.outcome,
            RolloutWorkerOutcomeKind::UploadRuleRejected
        );
        assert_eq!(
            rejected.rejection_reason,
            Some(RolloutWorkerRejectionReason::InlineUploadTooLarge)
        );
        Ok(())
    }
}
