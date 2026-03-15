use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_datastream::DatastreamSubjectKind;

use crate::{
    AdapterClusterWindowRecord, AdapterContributionExecutionSummary,
    AdapterContributionUploadLocator, AdapterTargetIdentity, CheckpointPointer, PolicyRevision,
};

/// Error returned by the adapter-worker protocol.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AdapterWorkerProtocolError {
    /// The adapter window contract rejected the requested transition.
    #[error(transparent)]
    WindowContract(#[from] crate::AdapterWindowContractError),
    /// One assignment id was not present in the current window.
    #[error("unknown adapter assignment `{assignment_id}`")]
    UnknownAssignment {
        /// Stable assignment identifier.
        assignment_id: String,
    },
    /// One worker attempted protocol actions without an active session.
    #[error("worker `{worker_id}` has no active adapter-worker session")]
    UnknownWorker {
        /// Stable worker identifier.
        worker_id: String,
    },
    /// One worker attempted to claim another worker's assignment.
    #[error(
        "adapter assignment `{assignment_id}` belongs to worker `{expected_worker_id}` but claim was attempted by `{actual_worker_id}`"
    )]
    AssignmentWorkerMismatch {
        /// Stable assignment identifier.
        assignment_id: String,
        /// Assigned worker id.
        expected_worker_id: String,
        /// Claiming worker id.
        actual_worker_id: String,
    },
    /// The worker heartbeat is too old for a fresh claim or ack.
    #[error(
        "worker `{worker_id}` heartbeat at {last_heartbeat_at_ms} is stale for observation time {observed_at_ms}"
    )]
    WorkerHeartbeatStale {
        /// Stable worker identifier.
        worker_id: String,
        /// Last heartbeat time.
        last_heartbeat_at_ms: u64,
        /// Current observation time.
        observed_at_ms: u64,
    },
    /// One assignment already has an active claim.
    #[error(
        "adapter assignment `{assignment_id}` is already claimed by `{claim_id}` until {claim_expires_at_ms}"
    )]
    AssignmentAlreadyClaimed {
        /// Stable assignment identifier.
        assignment_id: String,
        /// Stable claim identifier.
        claim_id: String,
        /// Claim expiry time.
        claim_expires_at_ms: u64,
    },
    /// One claim id was not present in protocol state.
    #[error("unknown adapter claim `{claim_id}`")]
    UnknownClaim {
        /// Stable claim identifier.
        claim_id: String,
    },
    /// One claim is in the wrong state for the requested action.
    #[error("adapter claim `{claim_id}` cannot {action} while status is `{status}`")]
    ClaimStateMismatch {
        /// Stable claim identifier.
        claim_id: String,
        /// Requested action.
        action: &'static str,
        /// Current claim status.
        status: String,
    },
    /// One session tried to act on another session's claim.
    #[error(
        "adapter claim `{claim_id}` belongs to session `{expected_session_id}` but `{actual_session_id}` attempted the action"
    )]
    ClaimSessionMismatch {
        /// Stable claim identifier.
        claim_id: String,
        /// Session id recorded on the claim.
        expected_session_id: String,
        /// Session id attempting the action.
        actual_session_id: String,
    },
}

/// Trust posture surfaced by one adapter worker session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterWorkerTrustClass {
    /// Trusted coordinator or tightly controlled operator session.
    TrustedOperator,
    /// Semi-trusted contributor session.
    SemiTrustedContributor,
    /// Untrusted or permissionless contributor session.
    UntrustedContributor,
}

/// Stable identity for one adapter worker session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWorkerIdentity {
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Trust posture for the worker.
    pub trust_class: AdapterWorkerTrustClass,
    /// Stable auth subject or operator-visible identity string.
    pub auth_subject: String,
    /// Hex-encoded Ed25519 verifying key for signed adapter submissions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub submission_signing_public_key_hex: String,
}

impl AdapterWorkerIdentity {
    /// Creates an adapter worker identity.
    #[must_use]
    pub fn new(
        worker_id: impl Into<String>,
        session_id: impl Into<String>,
        trust_class: AdapterWorkerTrustClass,
        auth_subject: impl Into<String>,
    ) -> Self {
        Self {
            worker_id: worker_id.into(),
            session_id: session_id.into(),
            trust_class,
            auth_subject: auth_subject.into(),
            submission_signing_public_key_hex: String::new(),
        }
    }

    /// Attaches the worker's submission-signing verifying key.
    #[must_use]
    pub fn with_submission_signing_public_key_hex(
        mut self,
        submission_signing_public_key_hex: impl Into<String>,
    ) -> Self {
        self.submission_signing_public_key_hex = submission_signing_public_key_hex.into();
        self
    }
}

/// Control policy for adapter-worker heartbeats, claims, and retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWorkerProtocolPolicy {
    /// Maximum time a heartbeat remains fresh for new claims or acks.
    pub heartbeat_timeout_ms: u64,
    /// Maximum lifetime for one active claim.
    pub claim_ttl_ms: u64,
}

impl Default for AdapterWorkerProtocolPolicy {
    fn default() -> Self {
        Self {
            heartbeat_timeout_ms: 10_000,
            claim_ttl_ms: 20_000,
        }
    }
}

/// Active session state for one adapter worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWorkerSession {
    /// Stable worker identity.
    pub identity: AdapterWorkerIdentity,
    /// Latest heartbeat timestamp.
    pub last_heartbeat_at_ms: u64,
}

/// Lightweight progress update surfaced in worker heartbeats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionProgress {
    /// Completed local optimization steps.
    pub completed_steps: u32,
    /// Samples processed so far.
    pub processed_samples: u32,
}

/// Typed heartbeat receipt for one adapter worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWorkerHeartbeatReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Trust posture for the worker.
    pub trust_class: AdapterWorkerTrustClass,
    /// Active claim id carried by the heartbeat when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_claim_id: Option<String>,
    /// Progress snapshot when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<AdapterContributionProgress>,
    /// Observed heartbeat time.
    pub observed_at_ms: u64,
    /// Next required heartbeat time under the current policy.
    pub next_required_heartbeat_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Upload or manifest expectation bound to one claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionUploadExpectation {
    /// Expected datastream subject kind for the uploaded artifact.
    pub subject: DatastreamSubjectKind,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable adapter target identifier.
    pub adapter_target_id: String,
    /// Stable target policy revision id.
    pub target_policy_revision_id: String,
    /// Stable target checkpoint pointer digest.
    pub target_checkpoint_pointer_digest: String,
    /// Expected upload-reference prefix for the local contribution bundle.
    pub upload_reference_prefix: String,
    /// Stable expectation digest.
    pub expectation_digest: String,
}

/// Worker-facing assignment for one adapter contribution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionWorkAssignment {
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Adapter target bound to the contribution.
    pub adapter_target: AdapterTargetIdentity,
    /// Dataset slice assigned to the worker.
    pub dataset_slice: crate::AdapterDatasetSliceIdentity,
    /// Source policy revision expected by the contribution.
    pub source_policy_revision: PolicyRevision,
    /// Source checkpoint pointer expected by the contribution.
    pub source_checkpoint_pointer: CheckpointPointer,
    /// Upload expectation bound to later completion.
    pub upload_expectation: AdapterContributionUploadExpectation,
    /// Stable assignment digest.
    pub assignment_digest: String,
}

/// Lifecycle state for one adapter assignment claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterAssignmentClaimStatus {
    /// Claim exists but has not yet been acknowledged.
    Claimed,
    /// Claim was acknowledged and may submit a contribution.
    Acknowledged,
    /// Claim completed with a successful submission.
    Completed,
    /// Claim expired before completion.
    Expired,
    /// Claim was superseded by a later claim on the same assignment.
    Superseded,
}

/// Deterministic claim for one adapter assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterAssignmentClaim {
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Trust posture for the session.
    pub trust_class: AdapterWorkerTrustClass,
    /// Source policy revision id expected by the contribution.
    pub target_policy_revision_id: String,
    /// Source checkpoint pointer digest expected by the contribution.
    pub target_checkpoint_pointer_digest: String,
    /// Upload expectation carried by the claim.
    pub upload_expectation: AdapterContributionUploadExpectation,
    /// Deterministic sample-selection seed.
    pub sample_selection_seed: u64,
    /// Monotonic attempt index for the assignment.
    pub attempt_index: u32,
    /// Claim start time.
    pub claimed_at_ms: u64,
    /// Claim expiry time.
    pub claim_expires_at_ms: u64,
    /// Claim lifecycle state.
    pub status: AdapterAssignmentClaimStatus,
    /// Stable claim digest.
    pub claim_digest: String,
}

/// Typed acknowledgement receipt for one claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterAssignmentAckReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Acknowledgement time.
    pub acknowledged_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Submission outcome for one adapter contribution attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionSubmissionOutcome {
    /// Submission was accepted into the adapter window.
    Accepted,
    /// Submission arrived after the claim expired.
    ClaimExpired,
    /// Submission arrived on a superseded claim.
    ClaimSuperseded,
    /// Submission came from the wrong worker id.
    UnauthorizedWorker,
    /// Submission came from the wrong session id.
    UnauthorizedSession,
    /// Submission arrived before the claim was acknowledged.
    ClaimNotAcknowledged,
    /// Submission reported a source policy revision that does not match the claim.
    SourcePolicyMismatch,
    /// Submission reported a source checkpoint pointer that does not match the claim.
    SourceCheckpointMismatch,
    /// Submission upload metadata did not satisfy the claim expectation.
    UploadExpectationMismatch,
}

/// Final machine-legible receipt for one contribution submission attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionSubmissionReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable claim identifier.
    pub claim_id: String,
    /// Stable assignment identifier.
    pub assignment_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable session identifier.
    pub session_id: String,
    /// Trust posture for the session.
    pub trust_class: AdapterWorkerTrustClass,
    /// Target policy revision expected by the claim.
    pub target_policy_revision_id: String,
    /// Source policy revision reported by the worker.
    pub source_policy_revision_id: String,
    /// Target checkpoint pointer digest expected by the claim.
    pub target_checkpoint_pointer_digest: String,
    /// Source checkpoint pointer digest reported by the worker.
    pub source_checkpoint_pointer_digest: String,
    /// Upload expectation bound to the claim.
    pub upload_expectation: AdapterContributionUploadExpectation,
    /// Worker-reported execution summary.
    pub execution_summary: AdapterContributionExecutionSummary,
    /// Worker-reported upload locator.
    pub upload: AdapterContributionUploadLocator,
    /// Final submission outcome.
    pub outcome: AdapterContributionSubmissionOutcome,
    /// Observation time.
    pub observed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stateful adapter-worker protocol over one active or planned adapter window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWorkerProtocolState {
    /// Inspectable cluster-backed window plan.
    pub window_plan: crate::AdapterClusterWindowPlanReceipt,
    /// Mutable adapter window owned by this worker protocol.
    pub window: crate::AdapterTrainingWindowStateMachine,
    /// Protocol control policy.
    pub policy: AdapterWorkerProtocolPolicy,
    /// Worker-facing assignments for the window.
    pub assignments: Vec<AdapterContributionWorkAssignment>,
    /// Known worker sessions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<AdapterWorkerSession>,
    /// Claim history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<AdapterAssignmentClaim>,
    /// Heartbeat receipts emitted by the protocol.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heartbeat_receipts: Vec<AdapterWorkerHeartbeatReceipt>,
    /// Acknowledgement receipts emitted by the protocol.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledgement_receipts: Vec<AdapterAssignmentAckReceipt>,
    /// Submission receipts emitted by the protocol.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub submission_receipts: Vec<AdapterContributionSubmissionReceipt>,
}

impl AdapterWorkerProtocolState {
    /// Creates an adapter-worker protocol from one cluster-backed window record.
    #[must_use]
    pub fn from_window_record(
        record: &AdapterClusterWindowRecord,
        policy: AdapterWorkerProtocolPolicy,
    ) -> Self {
        let assignments = record
            .window
            .contributions
            .iter()
            .map(|contribution| {
                let binding = &contribution.assignment.binding;
                let upload_expectation = AdapterContributionUploadExpectation {
                    subject: DatastreamSubjectKind::AdapterPackage,
                    window_id: binding.window_id.clone(),
                    contribution_id: binding.contribution_id.clone(),
                    adapter_target_id: binding.adapter_target.adapter_target_id.clone(),
                    target_policy_revision_id: binding.source_policy_revision.revision_id.clone(),
                    target_checkpoint_pointer_digest: binding
                        .source_checkpoint_pointer
                        .pointer_digest
                        .clone(),
                    upload_reference_prefix: format!(
                        "object://adapter-window/{}/{}",
                        binding.window_id, binding.contribution_id
                    ),
                    expectation_digest: stable_upload_expectation_digest(
                        DatastreamSubjectKind::AdapterPackage,
                        binding.window_id.as_str(),
                        binding.contribution_id.as_str(),
                        binding.adapter_target.adapter_target_id.as_str(),
                        binding.source_policy_revision.revision_id.as_str(),
                        binding.source_checkpoint_pointer.pointer_digest.as_str(),
                    ),
                };
                AdapterContributionWorkAssignment {
                    assignment_id: binding.assignment_id.clone(),
                    contribution_id: binding.contribution_id.clone(),
                    window_id: binding.window_id.clone(),
                    worker_id: binding.contributor_node_id.clone(),
                    adapter_target: binding.adapter_target.clone(),
                    dataset_slice: binding.dataset_slice.clone(),
                    source_policy_revision: binding.source_policy_revision.clone(),
                    source_checkpoint_pointer: binding.source_checkpoint_pointer.clone(),
                    assignment_digest: stable_assignment_digest(
                        binding.assignment_id.as_str(),
                        binding.contribution_id.as_str(),
                        binding.window_id.as_str(),
                        binding.contributor_node_id.as_str(),
                        upload_expectation.expectation_digest.as_str(),
                        record.plan.assignment_seed,
                    ),
                    upload_expectation,
                }
            })
            .collect::<Vec<_>>();
        Self {
            window_plan: record.plan.clone(),
            window: record.window.clone(),
            policy,
            assignments,
            sessions: Vec::new(),
            claims: Vec::new(),
            heartbeat_receipts: Vec::new(),
            acknowledgement_receipts: Vec::new(),
            submission_receipts: Vec::new(),
        }
    }

    /// Activates the owned adapter window before workers begin claiming it.
    pub fn activate_window(&mut self) -> Result<(), AdapterWorkerProtocolError> {
        if self.window.status == crate::TrainingWindowStatus::Planned {
            self.window.activate()?;
        }
        Ok(())
    }

    /// Records or refreshes one adapter-worker heartbeat.
    pub fn record_heartbeat(
        &mut self,
        identity: AdapterWorkerIdentity,
        active_claim_id: Option<&str>,
        progress: Option<AdapterContributionProgress>,
        observed_at_ms: u64,
    ) -> Result<AdapterWorkerHeartbeatReceipt, AdapterWorkerProtocolError> {
        if let Some(active_claim_id) = active_claim_id {
            let claim = self
                .claims
                .iter()
                .find(|claim| claim.claim_id == active_claim_id)
                .ok_or_else(|| AdapterWorkerProtocolError::UnknownClaim {
                    claim_id: active_claim_id.to_string(),
                })?;
            if claim.worker_id != identity.worker_id {
                return Err(AdapterWorkerProtocolError::AssignmentWorkerMismatch {
                    assignment_id: claim.assignment_id.clone(),
                    expected_worker_id: claim.worker_id.clone(),
                    actual_worker_id: identity.worker_id.clone(),
                });
            }
        }
        match self
            .sessions
            .iter_mut()
            .find(|session| session.identity.worker_id == identity.worker_id)
        {
            Some(session) => {
                session.identity = identity.clone();
                session.last_heartbeat_at_ms = observed_at_ms;
            }
            None => self.sessions.push(AdapterWorkerSession {
                identity: identity.clone(),
                last_heartbeat_at_ms: observed_at_ms,
            }),
        }
        let receipt = AdapterWorkerHeartbeatReceipt {
            receipt_id: format!(
                "{}-heartbeat-{}",
                self.window_plan.window_id, identity.worker_id
            ),
            window_id: self.window_plan.window_id.clone(),
            worker_id: identity.worker_id.clone(),
            session_id: identity.session_id.clone(),
            trust_class: identity.trust_class,
            active_claim_id: active_claim_id.map(str::to_string),
            progress,
            observed_at_ms,
            next_required_heartbeat_at_ms: observed_at_ms
                .saturating_add(self.policy.heartbeat_timeout_ms),
            receipt_digest: stable_heartbeat_digest(
                self.window_plan.window_id.as_str(),
                identity.worker_id.as_str(),
                identity.session_id.as_str(),
                identity.trust_class,
                active_claim_id,
                observed_at_ms,
                observed_at_ms.saturating_add(self.policy.heartbeat_timeout_ms),
            ),
        };
        self.heartbeat_receipts.push(receipt.clone());
        Ok(receipt)
    }

    /// Claims one adapter assignment after verifying session freshness.
    pub fn claim_assignment(
        &mut self,
        worker_id: &str,
        assignment_id: &str,
        claimed_at_ms: u64,
    ) -> Result<AdapterAssignmentClaim, AdapterWorkerProtocolError> {
        self.expire_stale_claims(claimed_at_ms);
        let session = self
            .sessions
            .iter()
            .find(|session| session.identity.worker_id == worker_id)
            .cloned()
            .ok_or_else(|| AdapterWorkerProtocolError::UnknownWorker {
                worker_id: worker_id.to_string(),
            })?;
        self.require_fresh_session(&session, claimed_at_ms)?;
        let assignment = self
            .assignments
            .iter()
            .find(|assignment| assignment.assignment_id == assignment_id)
            .cloned()
            .ok_or_else(|| AdapterWorkerProtocolError::UnknownAssignment {
                assignment_id: assignment_id.to_string(),
            })?;
        if assignment.worker_id != worker_id {
            return Err(AdapterWorkerProtocolError::AssignmentWorkerMismatch {
                assignment_id: assignment.assignment_id.clone(),
                expected_worker_id: assignment.worker_id.clone(),
                actual_worker_id: worker_id.to_string(),
            });
        }
        if let Some(existing_claim) = self.claims.iter().find(|claim| {
            claim.assignment_id == assignment_id
                && matches!(
                    claim.status,
                    AdapterAssignmentClaimStatus::Claimed
                        | AdapterAssignmentClaimStatus::Acknowledged
                )
        }) {
            return Err(AdapterWorkerProtocolError::AssignmentAlreadyClaimed {
                assignment_id: assignment_id.to_string(),
                claim_id: existing_claim.claim_id.clone(),
                claim_expires_at_ms: existing_claim.claim_expires_at_ms,
            });
        }
        for claim in &mut self.claims {
            if claim.assignment_id == assignment_id
                && claim.status == AdapterAssignmentClaimStatus::Expired
            {
                claim.status = AdapterAssignmentClaimStatus::Superseded;
            }
        }
        let attempt_index = self
            .claims
            .iter()
            .filter(|claim| claim.assignment_id == assignment_id)
            .count() as u32
            + 1;
        let sample_selection_seed = stable_sample_selection_seed(
            self.window_plan.assignment_seed,
            assignment.assignment_digest.as_str(),
            worker_id,
            session.identity.session_id.as_str(),
            attempt_index,
        );
        let claim = AdapterAssignmentClaim {
            claim_id: format!("{assignment_id}-claim-{attempt_index}"),
            assignment_id: assignment.assignment_id.clone(),
            contribution_id: assignment.contribution_id.clone(),
            window_id: assignment.window_id.clone(),
            worker_id: worker_id.to_string(),
            session_id: session.identity.session_id.clone(),
            trust_class: session.identity.trust_class,
            target_policy_revision_id: assignment.source_policy_revision.revision_id.clone(),
            target_checkpoint_pointer_digest: assignment
                .source_checkpoint_pointer
                .pointer_digest
                .clone(),
            upload_expectation: assignment.upload_expectation.clone(),
            sample_selection_seed,
            attempt_index,
            claimed_at_ms,
            claim_expires_at_ms: claimed_at_ms.saturating_add(self.policy.claim_ttl_ms),
            status: AdapterAssignmentClaimStatus::Claimed,
            claim_digest: stable_claim_digest(
                assignment.assignment_id.as_str(),
                assignment.contribution_id.as_str(),
                assignment.window_id.as_str(),
                worker_id,
                session.identity.session_id.as_str(),
                assignment.source_policy_revision.revision_id.as_str(),
                assignment.source_checkpoint_pointer.pointer_digest.as_str(),
                assignment.upload_expectation.expectation_digest.as_str(),
                sample_selection_seed,
                attempt_index,
                claimed_at_ms,
                claimed_at_ms.saturating_add(self.policy.claim_ttl_ms),
            ),
        };
        self.claims.push(claim.clone());
        Ok(claim)
    }

    /// Acknowledges one claim before contribution execution starts.
    pub fn acknowledge_assignment(
        &mut self,
        worker_id: &str,
        session_id: &str,
        claim_id: &str,
        acknowledged_at_ms: u64,
    ) -> Result<AdapterAssignmentAckReceipt, AdapterWorkerProtocolError> {
        self.expire_stale_claims(acknowledged_at_ms);
        let session = self
            .sessions
            .iter()
            .find(|session| session.identity.worker_id == worker_id)
            .cloned()
            .ok_or_else(|| AdapterWorkerProtocolError::UnknownWorker {
                worker_id: worker_id.to_string(),
            })?;
        self.require_fresh_session(&session, acknowledged_at_ms)?;
        let claim_index = self
            .claims
            .iter()
            .position(|claim| claim.claim_id == claim_id)
            .ok_or_else(|| AdapterWorkerProtocolError::UnknownClaim {
                claim_id: claim_id.to_string(),
            })?;
        let claim = self.claims[claim_index].clone();
        if claim.worker_id != worker_id {
            return Err(AdapterWorkerProtocolError::AssignmentWorkerMismatch {
                assignment_id: claim.assignment_id.clone(),
                expected_worker_id: claim.worker_id,
                actual_worker_id: worker_id.to_string(),
            });
        }
        if claim.session_id != session_id {
            return Err(AdapterWorkerProtocolError::ClaimSessionMismatch {
                claim_id: claim.claim_id,
                expected_session_id: claim.session_id,
                actual_session_id: session_id.to_string(),
            });
        }
        if claim.status != AdapterAssignmentClaimStatus::Claimed {
            return Err(AdapterWorkerProtocolError::ClaimStateMismatch {
                claim_id: claim_id.to_string(),
                action: "acknowledge",
                status: adapter_claim_status_label(claim.status).to_string(),
            });
        }
        self.claims[claim_index].status = AdapterAssignmentClaimStatus::Acknowledged;
        let receipt = AdapterAssignmentAckReceipt {
            receipt_id: format!("{claim_id}-ack"),
            claim_id: claim_id.to_string(),
            assignment_id: claim.assignment_id,
            contribution_id: claim.contribution_id,
            worker_id: worker_id.to_string(),
            session_id: session_id.to_string(),
            acknowledged_at_ms,
            receipt_digest: stable_ack_digest(claim_id, worker_id, session_id, acknowledged_at_ms),
        };
        self.acknowledgement_receipts.push(receipt.clone());
        Ok(receipt)
    }

    /// Submits one contribution attempt and records either success or refusal.
    pub fn submit_contribution(
        &mut self,
        claim_id: &str,
        worker_id: &str,
        session_id: &str,
        source_policy_revision_id: &str,
        source_checkpoint_pointer_digest: &str,
        execution_summary: AdapterContributionExecutionSummary,
        upload: AdapterContributionUploadLocator,
        observed_at_ms: u64,
    ) -> Result<AdapterContributionSubmissionReceipt, AdapterWorkerProtocolError> {
        self.expire_stale_claims(observed_at_ms);
        let session_last_heartbeat_at_ms = self.session_last_heartbeat(worker_id, session_id);
        let claim_index = self
            .claims
            .iter()
            .position(|claim| claim.claim_id == claim_id)
            .ok_or_else(|| AdapterWorkerProtocolError::UnknownClaim {
                claim_id: claim_id.to_string(),
            })?;
        let claim = self.claims[claim_index].clone();
        let outcome = if claim.worker_id != worker_id {
            AdapterContributionSubmissionOutcome::UnauthorizedWorker
        } else if claim.session_id != session_id {
            AdapterContributionSubmissionOutcome::UnauthorizedSession
        } else if claim.status == AdapterAssignmentClaimStatus::Superseded {
            AdapterContributionSubmissionOutcome::ClaimSuperseded
        } else if matches!(claim.status, AdapterAssignmentClaimStatus::Expired)
            || observed_at_ms > claim.claim_expires_at_ms
            || session_last_heartbeat_at_ms.is_none_or(|last_heartbeat_at_ms| {
                observed_at_ms.saturating_sub(last_heartbeat_at_ms)
                    > self.policy.heartbeat_timeout_ms
            })
        {
            self.claims[claim_index].status = AdapterAssignmentClaimStatus::Expired;
            AdapterContributionSubmissionOutcome::ClaimExpired
        } else if claim.status != AdapterAssignmentClaimStatus::Acknowledged {
            AdapterContributionSubmissionOutcome::ClaimNotAcknowledged
        } else if source_policy_revision_id != claim.target_policy_revision_id {
            AdapterContributionSubmissionOutcome::SourcePolicyMismatch
        } else if source_checkpoint_pointer_digest != claim.target_checkpoint_pointer_digest {
            AdapterContributionSubmissionOutcome::SourceCheckpointMismatch
        } else if !upload
            .upload_reference
            .starts_with(&claim.upload_expectation.upload_reference_prefix)
        {
            AdapterContributionSubmissionOutcome::UploadExpectationMismatch
        } else {
            AdapterContributionSubmissionOutcome::Accepted
        };

        if outcome == AdapterContributionSubmissionOutcome::Accepted {
            self.window
                .record_execution(claim.contribution_id.as_str(), execution_summary.clone())?;
            self.window.record_upload(
                claim.contribution_id.as_str(),
                upload.clone(),
                observed_at_ms,
            )?;
            self.claims[claim_index].status = AdapterAssignmentClaimStatus::Completed;
        }

        let receipt = AdapterContributionSubmissionReceipt {
            receipt_id: format!("{claim_id}-submission"),
            claim_id: claim.claim_id.clone(),
            assignment_id: claim.assignment_id.clone(),
            contribution_id: claim.contribution_id.clone(),
            window_id: claim.window_id.clone(),
            worker_id: worker_id.to_string(),
            session_id: session_id.to_string(),
            trust_class: claim.trust_class,
            target_policy_revision_id: claim.target_policy_revision_id.clone(),
            source_policy_revision_id: source_policy_revision_id.to_string(),
            target_checkpoint_pointer_digest: claim.target_checkpoint_pointer_digest.clone(),
            source_checkpoint_pointer_digest: source_checkpoint_pointer_digest.to_string(),
            upload_expectation: claim.upload_expectation.clone(),
            execution_summary,
            upload,
            outcome,
            observed_at_ms,
            receipt_digest: stable_submission_digest(
                claim.claim_id.as_str(),
                claim.assignment_id.as_str(),
                claim.contribution_id.as_str(),
                claim.window_id.as_str(),
                worker_id,
                session_id,
                claim.trust_class,
                claim.target_policy_revision_id.as_str(),
                source_policy_revision_id,
                claim.target_checkpoint_pointer_digest.as_str(),
                source_checkpoint_pointer_digest,
                claim.upload_expectation.expectation_digest.as_str(),
                outcome,
                observed_at_ms,
            ),
        };
        self.submission_receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn expire_stale_claims(&mut self, observed_at_ms: u64) {
        let session_heartbeats = self
            .sessions
            .iter()
            .map(|session| {
                (
                    (
                        session.identity.worker_id.clone(),
                        session.identity.session_id.clone(),
                    ),
                    session.last_heartbeat_at_ms,
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        for claim in &mut self.claims {
            if matches!(
                claim.status,
                AdapterAssignmentClaimStatus::Claimed | AdapterAssignmentClaimStatus::Acknowledged
            ) && (observed_at_ms > claim.claim_expires_at_ms
                || session_heartbeats
                    .get(&(claim.worker_id.clone(), claim.session_id.clone()))
                    .copied()
                    .is_none_or(|last_heartbeat_at_ms| {
                        observed_at_ms.saturating_sub(last_heartbeat_at_ms)
                            > self.policy.heartbeat_timeout_ms
                    }))
            {
                claim.status = AdapterAssignmentClaimStatus::Expired;
            }
        }
    }

    fn require_fresh_session(
        &self,
        session: &AdapterWorkerSession,
        observed_at_ms: u64,
    ) -> Result<(), AdapterWorkerProtocolError> {
        if observed_at_ms.saturating_sub(session.last_heartbeat_at_ms)
            > self.policy.heartbeat_timeout_ms
        {
            Err(AdapterWorkerProtocolError::WorkerHeartbeatStale {
                worker_id: session.identity.worker_id.clone(),
                last_heartbeat_at_ms: session.last_heartbeat_at_ms,
                observed_at_ms,
            })
        } else {
            Ok(())
        }
    }

    fn session_last_heartbeat(&self, worker_id: &str, session_id: &str) -> Option<u64> {
        self.sessions
            .iter()
            .find(|session| {
                session.identity.worker_id == worker_id && session.identity.session_id == session_id
            })
            .map(|session| session.last_heartbeat_at_ms)
    }
}

fn stable_upload_expectation_digest(
    subject: DatastreamSubjectKind,
    window_id: &str,
    contribution_id: &str,
    adapter_target_id: &str,
    target_policy_revision_id: &str,
    target_checkpoint_pointer_digest: &str,
) -> String {
    stable_digest([
        "adapter_upload_expectation",
        subject.as_str(),
        window_id,
        contribution_id,
        adapter_target_id,
        target_policy_revision_id,
        target_checkpoint_pointer_digest,
    ])
}

fn stable_assignment_digest(
    assignment_id: &str,
    contribution_id: &str,
    window_id: &str,
    worker_id: &str,
    expectation_digest: &str,
    assignment_seed: u64,
) -> String {
    stable_digest([
        "adapter_work_assignment",
        assignment_id,
        contribution_id,
        window_id,
        worker_id,
        expectation_digest,
        assignment_seed.to_string().as_str(),
    ])
}

fn stable_heartbeat_digest(
    window_id: &str,
    worker_id: &str,
    session_id: &str,
    trust_class: AdapterWorkerTrustClass,
    active_claim_id: Option<&str>,
    observed_at_ms: u64,
    next_required_heartbeat_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_worker_heartbeat",
        window_id,
        worker_id,
        session_id,
        adapter_worker_trust_class_label(trust_class),
        active_claim_id.unwrap_or("-"),
        observed_at_ms.to_string().as_str(),
        next_required_heartbeat_at_ms.to_string().as_str(),
    ])
}

fn stable_sample_selection_seed(
    assignment_seed: u64,
    assignment_digest: &str,
    worker_id: &str,
    session_id: &str,
    attempt_index: u32,
) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"adapter_sample_selection_seed|");
    hasher.update(assignment_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(assignment_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(attempt_index.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

fn stable_claim_digest(
    assignment_id: &str,
    contribution_id: &str,
    window_id: &str,
    worker_id: &str,
    session_id: &str,
    target_policy_revision_id: &str,
    target_checkpoint_pointer_digest: &str,
    expectation_digest: &str,
    sample_selection_seed: u64,
    attempt_index: u32,
    claimed_at_ms: u64,
    claim_expires_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_assignment_claim",
        assignment_id,
        contribution_id,
        window_id,
        worker_id,
        session_id,
        target_policy_revision_id,
        target_checkpoint_pointer_digest,
        expectation_digest,
        sample_selection_seed.to_string().as_str(),
        attempt_index.to_string().as_str(),
        claimed_at_ms.to_string().as_str(),
        claim_expires_at_ms.to_string().as_str(),
    ])
}

fn stable_ack_digest(
    claim_id: &str,
    worker_id: &str,
    session_id: &str,
    acknowledged_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_assignment_ack",
        claim_id,
        worker_id,
        session_id,
        acknowledged_at_ms.to_string().as_str(),
    ])
}

fn stable_submission_digest(
    claim_id: &str,
    assignment_id: &str,
    contribution_id: &str,
    window_id: &str,
    worker_id: &str,
    session_id: &str,
    trust_class: AdapterWorkerTrustClass,
    target_policy_revision_id: &str,
    source_policy_revision_id: &str,
    target_checkpoint_pointer_digest: &str,
    source_checkpoint_pointer_digest: &str,
    expectation_digest: &str,
    outcome: AdapterContributionSubmissionOutcome,
    observed_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_contribution_submission",
        claim_id,
        assignment_id,
        contribution_id,
        window_id,
        worker_id,
        session_id,
        adapter_worker_trust_class_label(trust_class),
        target_policy_revision_id,
        source_policy_revision_id,
        target_checkpoint_pointer_digest,
        source_checkpoint_pointer_digest,
        expectation_digest,
        adapter_submission_outcome_label(outcome),
        observed_at_ms.to_string().as_str(),
    ])
}

fn stable_digest<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    format!("{:x}", hasher.finalize())
}

fn adapter_worker_trust_class_label(trust_class: AdapterWorkerTrustClass) -> &'static str {
    match trust_class {
        AdapterWorkerTrustClass::TrustedOperator => "trusted_operator",
        AdapterWorkerTrustClass::SemiTrustedContributor => "semi_trusted_contributor",
        AdapterWorkerTrustClass::UntrustedContributor => "untrusted_contributor",
    }
}

fn adapter_claim_status_label(status: AdapterAssignmentClaimStatus) -> &'static str {
    match status {
        AdapterAssignmentClaimStatus::Claimed => "claimed",
        AdapterAssignmentClaimStatus::Acknowledged => "acknowledged",
        AdapterAssignmentClaimStatus::Completed => "completed",
        AdapterAssignmentClaimStatus::Expired => "expired",
        AdapterAssignmentClaimStatus::Superseded => "superseded",
    }
}

fn adapter_submission_outcome_label(outcome: AdapterContributionSubmissionOutcome) -> &'static str {
    match outcome {
        AdapterContributionSubmissionOutcome::Accepted => "accepted",
        AdapterContributionSubmissionOutcome::ClaimExpired => "claim_expired",
        AdapterContributionSubmissionOutcome::ClaimSuperseded => "claim_superseded",
        AdapterContributionSubmissionOutcome::UnauthorizedWorker => "unauthorized_worker",
        AdapterContributionSubmissionOutcome::UnauthorizedSession => "unauthorized_session",
        AdapterContributionSubmissionOutcome::ClaimNotAcknowledged => "claim_not_acknowledged",
        AdapterContributionSubmissionOutcome::SourcePolicyMismatch => "source_policy_mismatch",
        AdapterContributionSubmissionOutcome::SourceCheckpointMismatch => {
            "source_checkpoint_mismatch"
        }
        AdapterContributionSubmissionOutcome::UploadExpectationMismatch => {
            "upload_expectation_mismatch"
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterBackendReadinessStatus, ClusterId, ClusterMembershipRecord,
        ClusterMembershipStatus, ClusterNamespace, ClusterNodeIdentity, ClusterNodeTelemetry,
        ClusterSnapshot, ClusterStabilityPosture, NodeEpoch, NodeId, NodeRole,
    };

    use super::{
        AdapterContributionProgress, AdapterContributionSubmissionOutcome, AdapterWorkerIdentity,
        AdapterWorkerProtocolPolicy, AdapterWorkerProtocolState, AdapterWorkerTrustClass,
    };
    use crate::{
        AdapterContributionExecutionSummary, AdapterContributionUploadLocator,
        AdapterContributorCapabilityPolicy, AdapterDatasetSliceIdentity, AdapterTargetIdentity,
        AdapterTrainingClusterCoordinator, CheckpointPointer, CheckpointScopeBinding,
        CheckpointScopeKind, PolicyRevision,
    };

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("adapter-worker-protocol"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([
            (
                NodeId::new("worker-a"),
                ClusterMembershipRecord::new(
                    ClusterNodeIdentity {
                        cluster_id: cluster_id.clone(),
                        node_id: NodeId::new("worker-a"),
                        node_epoch: NodeEpoch::initial(),
                        role: NodeRole::ExecutorOnly,
                        auth_public_key: String::from("worker-a-pk"),
                        attestation: None,
                    },
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 32_100)),
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
                    Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 32_101)),
                    ClusterMembershipStatus::Ready,
                ),
            ),
        ]);
        snapshot.telemetry = BTreeMap::from([
            (
                NodeId::new("worker-a"),
                ClusterNodeTelemetry::new(NodeId::new("worker-a"))
                    .with_memory(Some(20 * GIB_BYTES), Some(20 * GIB_BYTES))
                    .with_accelerator_count(1)
                    .with_backend_readiness(
                        AdapterContributorCapabilityPolicy::default().backend_label,
                        ClusterBackendReadinessStatus::Ready,
                    )
                    .with_stability_posture(ClusterStabilityPosture::Stable),
            ),
            (
                NodeId::new("worker-b"),
                ClusterNodeTelemetry::new(NodeId::new("worker-b"))
                    .with_memory(Some(28 * GIB_BYTES), Some(28 * GIB_BYTES))
                    .with_accelerator_count(1)
                    .with_backend_readiness(
                        AdapterContributorCapabilityPolicy::default().backend_label,
                        ClusterBackendReadinessStatus::Ready,
                    )
                    .with_stability_posture(ClusterStabilityPosture::Stable),
            ),
        ]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn checkpoint_reference(
        checkpoint_ref: &str,
        started_at_ms: u64,
    ) -> psionic_runtime::TrainingCheckpointReference {
        psionic_runtime::TrainingCheckpointReference::new(
            "apple.weather.policy",
            format!("stream://{checkpoint_ref}"),
            format!("manifest://{checkpoint_ref}"),
            format!("object://{checkpoint_ref}"),
            "worker-a",
            7,
            "cluster-digest-weather",
            "topology-digest-weather",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(70)
    }

    fn window_record() -> Result<crate::AdapterClusterWindowRecord, Box<dyn std::error::Error>> {
        let state = cluster_state();
        let run = crate::TrainingRunState::new(
            "adapter-run-worker",
            "adapter-sft",
            state.cluster_id().as_str(),
            "apple.weather.policy",
            psionic_environments::EnvironmentPackageKey::new("oa.apple.adapter", "2026.03"),
        )?;
        let mut coordinator = AdapterTrainingClusterCoordinator::new(
            run,
            AdapterTargetIdentity::new(
                "apple.weather.adapter",
                "apple.foundation_models",
                "apple://foundation-model/base",
                "apple.fmadapter",
            )?,
            PolicyRevision::new(
                "apple.weather.policy",
                "policy-r7",
                "policy-digest-r7",
                1_000,
            )
            .with_revision_number(7),
            CheckpointPointer::new(
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-3"),
                "apple.weather.policy",
                checkpoint_reference("checkpoint/weather/r7", 1_000),
                "manifest-digest-r7",
                1_001,
            )?,
            AdapterContributorCapabilityPolicy {
                minimum_free_memory_bytes: 12 * GIB_BYTES,
                ..AdapterContributorCapabilityPolicy::default()
            },
        );
        coordinator.observe_cluster_state(&state, 1_010)?;
        let record = coordinator.plan_next_window(
            vec![
                AdapterDatasetSliceIdentity::new(
                    "dataset.weather",
                    "train",
                    "slice-a",
                    "slice-digest-a",
                )?,
                AdapterDatasetSliceIdentity::new(
                    "dataset.weather",
                    "train",
                    "slice-b",
                    "slice-digest-b",
                )?,
            ],
            2,
            1_020,
        )?;
        Ok(record)
    }

    #[test]
    fn adapter_worker_protocol_claim_ack_and_submit_updates_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let record = window_record()?;
        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        protocol.record_heartbeat(
            AdapterWorkerIdentity::new(
                "worker-b",
                "session-1",
                AdapterWorkerTrustClass::SemiTrustedContributor,
                "auth://worker-b",
            ),
            None,
            None,
            1_030,
        )?;
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        let claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 1_031)?;
        protocol.acknowledge_assignment("worker-b", "session-1", claim.claim_id.as_str(), 1_032)?;
        let heartbeat = protocol.record_heartbeat(
            AdapterWorkerIdentity::new(
                "worker-b",
                "session-1",
                AdapterWorkerTrustClass::SemiTrustedContributor,
                "auth://worker-b",
            ),
            Some(claim.claim_id.as_str()),
            Some(AdapterContributionProgress {
                completed_steps: 4,
                processed_samples: 16,
            }),
            1_033,
        )?;
        assert_eq!(
            heartbeat.active_claim_id.as_deref(),
            Some(claim.claim_id.as_str())
        );
        let receipt = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "session-1",
            "policy-r7",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            AdapterContributionExecutionSummary::new(
                1_034,
                1_040,
                6,
                24,
                Some(190),
                "delta-digest-worker-b",
            )?,
            AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, claim.contribution_id
                ),
                "upload-manifest-worker-b",
                4_096,
            )?,
            1_041,
        )?;
        assert_eq!(
            receipt.outcome,
            AdapterContributionSubmissionOutcome::Accepted
        );
        assert!(
            protocol
                .window
                .contributions
                .iter()
                .find(
                    |contribution| contribution.assignment.binding.contribution_id
                        == claim.contribution_id
                )
                .is_some_and(|contribution| contribution.upload.is_some())
        );
        Ok(())
    }

    #[test]
    fn adapter_worker_protocol_refuses_superseded_claim_submissions()
    -> Result<(), Box<dyn std::error::Error>> {
        let record = window_record()?;
        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        protocol.record_heartbeat(
            AdapterWorkerIdentity::new(
                "worker-b",
                "session-1",
                AdapterWorkerTrustClass::UntrustedContributor,
                "auth://worker-b",
            ),
            None,
            None,
            1_030,
        )?;
        let old_claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 1_031)?;
        protocol.record_heartbeat(
            AdapterWorkerIdentity::new(
                "worker-b",
                "session-2",
                AdapterWorkerTrustClass::UntrustedContributor,
                "auth://worker-b",
            ),
            None,
            None,
            30_100,
        )?;
        let new_claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 30_101)?;
        assert_eq!(new_claim.attempt_index, 2);
        let receipt = protocol.submit_contribution(
            old_claim.claim_id.as_str(),
            "worker-b",
            "session-1",
            "policy-r7",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            AdapterContributionExecutionSummary::new(
                30_102,
                30_110,
                5,
                20,
                Some(210),
                "delta-digest-old-claim",
            )?,
            AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, old_claim.contribution_id
                ),
                "upload-manifest-old-claim",
                4_096,
            )?,
            30_111,
        )?;
        assert_eq!(
            receipt.outcome,
            AdapterContributionSubmissionOutcome::ClaimSuperseded
        );
        Ok(())
    }

    #[test]
    fn adapter_worker_protocol_refuses_unauthorized_or_mismatched_submissions()
    -> Result<(), Box<dyn std::error::Error>> {
        let record = window_record()?;
        let mut protocol = AdapterWorkerProtocolState::from_window_record(
            &record,
            AdapterWorkerProtocolPolicy::default(),
        );
        protocol.activate_window()?;
        protocol.record_heartbeat(
            AdapterWorkerIdentity::new(
                "worker-b",
                "session-1",
                AdapterWorkerTrustClass::SemiTrustedContributor,
                "auth://worker-b",
            ),
            None,
            None,
            1_030,
        )?;
        let assignment_id = protocol.assignments[0].assignment_id.clone();
        let claim = protocol.claim_assignment("worker-b", assignment_id.as_str(), 1_031)?;
        protocol.acknowledge_assignment("worker-b", "session-1", claim.claim_id.as_str(), 1_032)?;

        let wrong_session = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "session-2",
            "policy-r7",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            AdapterContributionExecutionSummary::new(
                1_033,
                1_040,
                5,
                20,
                Some(205),
                "delta-digest-wrong-session",
            )?,
            AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, claim.contribution_id
                ),
                "upload-manifest-wrong-session",
                4_096,
            )?,
            1_041,
        )?;
        assert_eq!(
            wrong_session.outcome,
            AdapterContributionSubmissionOutcome::UnauthorizedSession
        );

        let policy_mismatch = protocol.submit_contribution(
            claim.claim_id.as_str(),
            "worker-b",
            "session-1",
            "policy-r6",
            record.plan.input_checkpoint_pointer.pointer_digest.as_str(),
            AdapterContributionExecutionSummary::new(
                1_042,
                1_048,
                5,
                20,
                Some(205),
                "delta-digest-policy-mismatch",
            )?,
            AdapterContributionUploadLocator::new(
                format!(
                    "object://adapter-window/{}/{}",
                    record.plan.window_id, claim.contribution_id
                ),
                "upload-manifest-policy-mismatch",
                4_096,
            )?,
            1_049,
        )?;
        assert_eq!(
            policy_mismatch.outcome,
            AdapterContributionSubmissionOutcome::SourcePolicyMismatch
        );
        Ok(())
    }
}
