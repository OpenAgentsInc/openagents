use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    PsionicTrainCapabilityProjection, PsionicTrainInvocationManifest, PsionicTrainOperation,
    PsionicTrainOutcomeKind, PsionicTrainRole, PsionicTrainRuntimeAttestation,
    PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
};

/// Stable schema version for one retained cluster-membership revision receipt.
pub const PSIONIC_TRAIN_MEMBERSHIP_REVISION_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.train.membership_revision_receipt.v1";

/// Frozen heartbeat cadence for the first admitted machine membership contract.
pub const PSIONIC_TRAIN_MEMBERSHIP_HEARTBEAT_INTERVAL_MS: u64 = 5_000;

/// Frozen stale threshold for the first admitted machine membership contract.
pub const PSIONIC_TRAIN_MEMBERSHIP_HEARTBEAT_STALE_AFTER_MS: u64 = 15_000;

/// Frozen expiry threshold for the first admitted machine membership contract.
pub const PSIONIC_TRAIN_MEMBERSHIP_HEARTBEAT_EXPIRY_MS: u64 = 30_000;

/// Frozen lease duration for the first admitted machine membership contract.
pub const PSIONIC_TRAIN_MEMBERSHIP_LEASE_DURATION_MS: u64 = 60_000;

/// Frozen lease-renewal threshold for the first admitted machine membership contract.
pub const PSIONIC_TRAIN_MEMBERSHIP_LEASE_RENEWAL_THRESHOLD_MS: u64 = 15_000;

/// Frozen drain grace period for the first admitted machine membership contract.
pub const PSIONIC_TRAIN_MEMBERSHIP_DRAIN_GRACE_MS: u64 = 15_000;

/// Explicit state transition surfaced by the local cluster-membership contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainMembershipTransitionKind {
    /// The worker entered membership without prior retained state.
    Join,
    /// The worker refreshed liveness inside the same retained session.
    Heartbeat,
    /// The same worker re-entered after stale or failed prior state.
    Rejoin,
    /// The worker explicitly entered drain posture.
    Drain,
    /// The previously retained worker was replaced by a different admitted node.
    Replace,
    /// The worker retained an explicit failed posture.
    Fail,
}

/// Current worker posture carried by one membership-revision receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainMembershipWorkerState {
    /// The worker is active for the current admitted lane.
    Active,
    /// The worker is still admitted but draining.
    Draining,
    /// The worker is known failed for the current retained session.
    Failed,
    /// The retained prior worker was explicitly replaced.
    Replaced,
}

/// Liveness classification derived from retained heartbeat timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainMembershipLivenessState {
    /// The latest heartbeat is inside the admitted freshness budget.
    Fresh,
    /// The latest heartbeat exceeded the stale threshold but not the expiry threshold.
    Stale,
    /// The latest heartbeat exceeded the expiry threshold.
    Expired,
}

/// Frozen timing policy for the first admitted cluster-membership contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainMembershipTimingPolicy {
    /// Required heartbeat interval.
    pub heartbeat_interval_ms: u64,
    /// Threshold after which the worker becomes stale.
    pub heartbeat_stale_after_ms: u64,
    /// Threshold after which the worker becomes expired.
    pub heartbeat_expiry_ms: u64,
    /// Lease duration bound to the current session.
    pub lease_duration_ms: u64,
    /// Renewal threshold before lease expiry.
    pub lease_renewal_threshold_ms: u64,
    /// Grace window after drain entry.
    pub drain_grace_ms: u64,
}

impl Default for PsionicTrainMembershipTimingPolicy {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: PSIONIC_TRAIN_MEMBERSHIP_HEARTBEAT_INTERVAL_MS,
            heartbeat_stale_after_ms: PSIONIC_TRAIN_MEMBERSHIP_HEARTBEAT_STALE_AFTER_MS,
            heartbeat_expiry_ms: PSIONIC_TRAIN_MEMBERSHIP_HEARTBEAT_EXPIRY_MS,
            lease_duration_ms: PSIONIC_TRAIN_MEMBERSHIP_LEASE_DURATION_MS,
            lease_renewal_threshold_ms: PSIONIC_TRAIN_MEMBERSHIP_LEASE_RENEWAL_THRESHOLD_MS,
            drain_grace_ms: PSIONIC_TRAIN_MEMBERSHIP_DRAIN_GRACE_MS,
        }
    }
}

/// One retained machine-readable membership revision for the admitted train lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainMembershipRevisionReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable runtime surface id.
    pub runtime_surface_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable node pubkey bound to the admitted worker identity.
    pub node_pubkey: String,
    /// Stable local session id.
    pub session_id: String,
    /// Current local membership revision.
    pub local_membership_revision: u64,
    /// Coordinator-provided membership revision when one exists already.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinator_membership_revision: Option<u64>,
    /// Previous local membership revision when one existed already.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_local_membership_revision: Option<u64>,
    /// Transition that produced the current revision.
    pub transition: PsionicTrainMembershipTransitionKind,
    /// Current worker posture.
    pub worker_state: PsionicTrainMembershipWorkerState,
    /// Prior worker posture when one existed already.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_worker_state: Option<PsionicTrainMembershipWorkerState>,
    /// Prior liveness posture when one existed already.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_liveness_state: Option<PsionicTrainMembershipLivenessState>,
    /// Previous node pubkey when the current worker replaced another admitted node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_node_pubkey: Option<String>,
    /// Stable admitted release id.
    pub release_id: String,
    /// Stable admitted build digest.
    pub build_digest: String,
    /// Stable admitted environment ref.
    pub environment_ref: String,
    /// Stable backend family.
    pub backend_family: String,
    /// Stable topology class.
    pub topology_class: String,
    /// Frozen timing policy.
    pub timing_policy: PsionicTrainMembershipTimingPolicy,
    /// First observation time for the current session.
    pub session_started_at_ms: u64,
    /// Observation time for the current receipt.
    pub observed_at_ms: u64,
    /// Latest heartbeat observation time retained for the worker.
    pub last_heartbeat_at_ms: u64,
    /// Next required heartbeat time.
    pub next_required_heartbeat_at_ms: u64,
    /// Stale threshold for this worker session.
    pub stale_after_ms: u64,
    /// Expiry threshold for this worker session.
    pub expires_at_ms: u64,
    /// Lease start time for the current session.
    pub lease_started_at_ms: u64,
    /// Lease renewal deadline.
    pub lease_renewal_required_at_ms: u64,
    /// Lease expiry time.
    pub lease_expires_at_ms: u64,
    /// Drain deadline when the worker is draining.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drain_deadline_at_ms: Option<u64>,
    /// Receipt digest over the canonical payload.
    pub receipt_digest: String,
}

impl PsionicTrainMembershipRevisionReceipt {
    /// Materializes the next local membership revision from the retained prior receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn next_for_manifest(
        manifest: &PsionicTrainInvocationManifest,
        runtime_attestation: &PsionicTrainRuntimeAttestation,
        capability_projection: &PsionicTrainCapabilityProjection,
        resolved_run_id: impl Into<String>,
        observed_at_ms: u64,
        outcome: PsionicTrainOutcomeKind,
        previous: Option<&Self>,
    ) -> Result<Self, String> {
        let node_pubkey = manifest.coordination.node_pubkey.clone().ok_or_else(|| {
            String::from("machine membership receipt requires coordination.node_pubkey")
        })?;
        let run_id = resolved_run_id.into();
        let timing_policy = PsionicTrainMembershipTimingPolicy::default();
        let previous_liveness_state = previous.map(|value| value.liveness_state_at(observed_at_ms));
        let worker_state = current_worker_state(manifest, outcome);
        let transition = current_transition(manifest, outcome, previous, previous_liveness_state);
        let previous_worker_state =
            previous.map(|value| inferred_previous_worker_state(value, transition, observed_at_ms));
        let replaced_node_pubkey = previous.and_then(|value| {
            (value.node_pubkey != node_pubkey).then(|| value.node_pubkey.clone())
        });
        let local_membership_revision = previous
            .map(|value| value.local_membership_revision.saturating_add(1))
            .unwrap_or(1);
        let session_started_at_ms = match transition {
            PsionicTrainMembershipTransitionKind::Join
            | PsionicTrainMembershipTransitionKind::Rejoin
            | PsionicTrainMembershipTransitionKind::Replace => observed_at_ms,
            _ => previous
                .map(|value| value.session_started_at_ms)
                .unwrap_or(observed_at_ms),
        };
        let lease_started_at_ms = match transition {
            PsionicTrainMembershipTransitionKind::Join
            | PsionicTrainMembershipTransitionKind::Rejoin
            | PsionicTrainMembershipTransitionKind::Replace => observed_at_ms,
            _ => previous
                .map(|value| value.lease_started_at_ms)
                .unwrap_or(observed_at_ms),
        };
        let last_heartbeat_at_ms = match transition {
            PsionicTrainMembershipTransitionKind::Fail => previous
                .map(|value| value.last_heartbeat_at_ms)
                .unwrap_or(observed_at_ms),
            _ => observed_at_ms,
        };
        let mut receipt = Self {
            schema_version: String::from(PSIONIC_TRAIN_MEMBERSHIP_REVISION_RECEIPT_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: manifest.lane_id.clone(),
            role: manifest.role,
            operation: manifest.operation,
            run_id: run_id.clone(),
            node_pubkey: node_pubkey.clone(),
            session_id: stable_membership_session_id(
                run_id.as_str(),
                node_pubkey.as_str(),
                session_started_at_ms,
            ),
            local_membership_revision,
            coordinator_membership_revision: manifest.coordination.membership_revision,
            previous_local_membership_revision: previous
                .map(|value| value.local_membership_revision),
            transition,
            worker_state,
            previous_worker_state,
            previous_liveness_state,
            replaced_node_pubkey,
            release_id: runtime_attestation.release_id.clone(),
            build_digest: runtime_attestation.build_digest.clone(),
            environment_ref: runtime_attestation.environment_ref.clone(),
            backend_family: capability_projection.backend_family.clone(),
            topology_class: capability_projection.topology_class.clone(),
            timing_policy: timing_policy.clone(),
            session_started_at_ms,
            observed_at_ms,
            last_heartbeat_at_ms,
            next_required_heartbeat_at_ms: last_heartbeat_at_ms
                .saturating_add(timing_policy.heartbeat_interval_ms),
            stale_after_ms: last_heartbeat_at_ms
                .saturating_add(timing_policy.heartbeat_stale_after_ms),
            expires_at_ms: last_heartbeat_at_ms.saturating_add(timing_policy.heartbeat_expiry_ms),
            lease_started_at_ms,
            lease_renewal_required_at_ms: lease_started_at_ms.saturating_add(
                timing_policy
                    .lease_duration_ms
                    .saturating_sub(timing_policy.lease_renewal_threshold_ms),
            ),
            lease_expires_at_ms: lease_started_at_ms
                .saturating_add(timing_policy.lease_duration_ms),
            drain_deadline_at_ms: (worker_state == PsionicTrainMembershipWorkerState::Draining)
                .then(|| observed_at_ms.saturating_add(timing_policy.drain_grace_ms)),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.canonical_digest()?;
        Ok(receipt)
    }

    /// Computes the current liveness classification at one observation time.
    #[must_use]
    pub fn liveness_state_at(&self, observed_at_ms: u64) -> PsionicTrainMembershipLivenessState {
        let elapsed = observed_at_ms.saturating_sub(self.last_heartbeat_at_ms);
        if elapsed >= self.timing_policy.heartbeat_expiry_ms {
            PsionicTrainMembershipLivenessState::Expired
        } else if elapsed >= self.timing_policy.heartbeat_stale_after_ms {
            PsionicTrainMembershipLivenessState::Stale
        } else {
            PsionicTrainMembershipLivenessState::Fresh
        }
    }

    fn canonical_digest(&self) -> Result<String, String> {
        let mut digest_basis = self.clone();
        digest_basis.receipt_digest.clear();
        let encoded = serde_json::to_vec(&digest_basis).map_err(|error| {
            format!("failed to serialize membership receipt digest basis: {error}")
        })?;
        Ok(sha256_hex(&encoded))
    }
}

fn current_worker_state(
    manifest: &PsionicTrainInvocationManifest,
    outcome: PsionicTrainOutcomeKind,
) -> PsionicTrainMembershipWorkerState {
    if outcome == PsionicTrainOutcomeKind::Refused {
        return PsionicTrainMembershipWorkerState::Failed;
    }
    match manifest.operation {
        PsionicTrainOperation::Backup | PsionicTrainOperation::DecideContinueRestart => {
            PsionicTrainMembershipWorkerState::Draining
        }
        _ if manifest.role == PsionicTrainRole::RecoverySource => {
            PsionicTrainMembershipWorkerState::Draining
        }
        _ => PsionicTrainMembershipWorkerState::Active,
    }
}

fn current_transition(
    manifest: &PsionicTrainInvocationManifest,
    outcome: PsionicTrainOutcomeKind,
    previous: Option<&PsionicTrainMembershipRevisionReceipt>,
    previous_liveness_state: Option<PsionicTrainMembershipLivenessState>,
) -> PsionicTrainMembershipTransitionKind {
    if outcome == PsionicTrainOutcomeKind::Refused && previous.is_some() {
        return PsionicTrainMembershipTransitionKind::Fail;
    }
    let Some(previous) = previous else {
        return PsionicTrainMembershipTransitionKind::Join;
    };
    let Some(current_node_pubkey) = manifest.coordination.node_pubkey.as_deref() else {
        return PsionicTrainMembershipTransitionKind::Join;
    };
    if previous.node_pubkey != current_node_pubkey {
        return PsionicTrainMembershipTransitionKind::Replace;
    }
    if matches!(
        previous_liveness_state,
        Some(PsionicTrainMembershipLivenessState::Expired)
    ) || previous.worker_state == PsionicTrainMembershipWorkerState::Failed
    {
        return PsionicTrainMembershipTransitionKind::Rejoin;
    }
    if current_worker_state(manifest, outcome) == PsionicTrainMembershipWorkerState::Draining
        && previous.worker_state != PsionicTrainMembershipWorkerState::Draining
    {
        return PsionicTrainMembershipTransitionKind::Drain;
    }
    PsionicTrainMembershipTransitionKind::Heartbeat
}

fn inferred_previous_worker_state(
    previous: &PsionicTrainMembershipRevisionReceipt,
    transition: PsionicTrainMembershipTransitionKind,
    observed_at_ms: u64,
) -> PsionicTrainMembershipWorkerState {
    match transition {
        PsionicTrainMembershipTransitionKind::Replace => {
            if previous.liveness_state_at(observed_at_ms)
                == PsionicTrainMembershipLivenessState::Expired
            {
                PsionicTrainMembershipWorkerState::Failed
            } else {
                PsionicTrainMembershipWorkerState::Replaced
            }
        }
        _ => previous.worker_state,
    }
}

fn stable_membership_session_id(run_id: &str, node_pubkey: &str, observed_at_ms: u64) -> String {
    sha256_hex(
        format!("psionic_train_membership_session|{run_id}|{node_pubkey}|{observed_at_ms}")
            .as_bytes(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PsionicTrainAdmissionIdentity, PsionicTrainCoordinationContext,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS,
        PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION, PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
    };

    fn manifest_for(node_pubkey: &str) -> PsionicTrainInvocationManifest {
        PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            work_class: crate::PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("network.psionic.test")),
                window_id: Some(String::from("window-001")),
                assignment_id: Some(String::from("assignment-001")),
                challenge_id: None,
                node_pubkey: Some(String::from(node_pubkey)),
                membership_revision: Some(41),
            },
            grouped_stage_assignment: None,
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID),
                build_digest: String::from("sha256:test-build"),
                environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
            },
            run_id: Some(String::from("run-psionic-train-membership-test")),
            output_root: Some(String::from("/tmp/psionic-train-membership-test")),
            run_root: None,
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt: None,
            validator_target_contribution_receipt: None,
            validator_target_contribution_artifact_manifest: None,
            validator_target_work_class: None,
            grouped_stage_input_transport: None,
            selected_git_ref: Some(String::from("HEAD")),
            hardware_observation_path: None,
            run_shape_observation_path: None,
            allow_dirty_tree: false,
            dry_run: true,
            checkpoint_label: None,
            optimizer_step: None,
            checkpoint_ref: None,
            checkpoint_object_digest: None,
            checkpoint_total_bytes: None,
            inject_failed_upload: false,
            inject_eval_worker_unavailable: false,
            manifest_digest: None,
        }
    }

    fn attestation() -> PsionicTrainRuntimeAttestation {
        PsionicTrainRuntimeAttestation::new(
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
            "sha256:test-build",
            "deadbeef",
            "refuse_by_default",
            None,
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        )
    }

    fn capability_projection() -> PsionicTrainCapabilityProjection {
        PsionicTrainCapabilityProjection {
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            backend_family: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY),
            topology_class: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS),
            environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
        }
    }

    #[test]
    fn timing_policy_defaults_are_frozen() {
        let policy = PsionicTrainMembershipTimingPolicy::default();
        assert_eq!(policy.heartbeat_interval_ms, 5_000);
        assert_eq!(policy.heartbeat_stale_after_ms, 15_000);
        assert_eq!(policy.heartbeat_expiry_ms, 30_000);
        assert_eq!(policy.lease_duration_ms, 60_000);
        assert_eq!(policy.lease_renewal_threshold_ms, 15_000);
        assert_eq!(policy.drain_grace_ms, 15_000);
    }

    #[test]
    fn expired_same_node_receipt_rejoins_without_manual_metadata_edits() {
        let manifest = manifest_for("npub1-psionic-worker-a");
        let initial = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest,
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            1_000,
            PsionicTrainOutcomeKind::Succeeded,
            None,
        )
        .expect("initial receipt should build");
        let rejoined = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest,
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            40_000,
            PsionicTrainOutcomeKind::Succeeded,
            Some(&initial),
        )
        .expect("rejoined receipt should build");
        assert_eq!(
            initial.liveness_state_at(40_000),
            PsionicTrainMembershipLivenessState::Expired
        );
        assert_eq!(
            rejoined.transition,
            PsionicTrainMembershipTransitionKind::Rejoin
        );
        assert_eq!(rejoined.local_membership_revision, 2);
        assert_eq!(rejoined.previous_local_membership_revision, Some(1));
        assert_eq!(
            rejoined.worker_state,
            PsionicTrainMembershipWorkerState::Active
        );
    }

    #[test]
    fn different_node_pubkey_replaces_prior_worker_automatically() {
        let initial = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest_for("npub1-psionic-worker-a"),
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            1_000,
            PsionicTrainOutcomeKind::Succeeded,
            None,
        )
        .expect("initial receipt should build");
        let replacement = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest_for("npub1-psionic-worker-b"),
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            6_000,
            PsionicTrainOutcomeKind::Succeeded,
            Some(&initial),
        )
        .expect("replacement receipt should build");
        assert_eq!(
            replacement.transition,
            PsionicTrainMembershipTransitionKind::Replace
        );
        assert_eq!(
            replacement.replaced_node_pubkey.as_deref(),
            Some("npub1-psionic-worker-a")
        );
        assert_eq!(
            replacement.previous_worker_state,
            Some(PsionicTrainMembershipWorkerState::Replaced)
        );
        assert_eq!(replacement.local_membership_revision, 2);
    }

    #[test]
    fn failure_revision_can_rejoin_later() {
        let manifest = manifest_for("npub1-psionic-worker-a");
        let initial = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest,
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            1_000,
            PsionicTrainOutcomeKind::Succeeded,
            None,
        )
        .expect("initial receipt should build");
        let failed = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest,
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            9_000,
            PsionicTrainOutcomeKind::Refused,
            Some(&initial),
        )
        .expect("failed receipt should build");
        let rejoined = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
            &manifest,
            &attestation(),
            &capability_projection(),
            "run-psionic-train-membership-test",
            12_000,
            PsionicTrainOutcomeKind::Succeeded,
            Some(&failed),
        )
        .expect("rejoined receipt should build");
        assert_eq!(
            failed.transition,
            PsionicTrainMembershipTransitionKind::Fail
        );
        assert_eq!(
            failed.worker_state,
            PsionicTrainMembershipWorkerState::Failed
        );
        assert_eq!(
            rejoined.transition,
            PsionicTrainMembershipTransitionKind::Rejoin
        );
        assert_eq!(
            rejoined.worker_state,
            PsionicTrainMembershipWorkerState::Active
        );
    }
}
