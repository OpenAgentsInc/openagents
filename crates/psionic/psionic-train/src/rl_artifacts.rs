use std::collections::{BTreeMap, BTreeSet};

use psionic_environments::EnvironmentPackageKey;
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Error returned by rollout and trainer-batch contract validation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RolloutContractError {
    /// The rollout artifact did not carry any samples.
    #[error("rollout artifact `{artifact_id}` must carry at least one sample")]
    EmptyRolloutSamples {
        /// Stable artifact identifier.
        artifact_id: String,
    },
    /// The rollout samples contained repeated proof digests.
    #[error("rollout artifact `{artifact_id}` contains duplicate proof digest `{digest}`")]
    DuplicateProofDigest {
        /// Stable artifact identifier.
        artifact_id: String,
        /// Repeated digest.
        digest: String,
    },
    /// A trainer batch attempted to reuse an artifact identifier twice.
    #[error("trainer batch `{batch_id}` repeated rollout artifact `{artifact_id}`")]
    DuplicateRolloutArtifact {
        /// Stable batch identifier.
        batch_id: String,
        /// Stable rollout artifact identifier.
        artifact_id: String,
    },
    /// The trainer batch had no rollout artifacts to assemble.
    #[error("trainer batch `{batch_id}` requires at least one rollout artifact")]
    EmptyTrainerBatch {
        /// Stable batch identifier.
        batch_id: String,
    },
    /// One rollout targeted a different policy family than the batch target.
    #[error(
        "trainer batch `{batch_id}` rollout `{artifact_id}` used policy family `{actual_policy_family}` but target family is `{expected_policy_family}`"
    )]
    PolicyFamilyMismatch {
        /// Stable batch identifier.
        batch_id: String,
        /// Stable artifact identifier.
        artifact_id: String,
        /// Expected policy family.
        expected_policy_family: String,
        /// Actual policy family.
        actual_policy_family: String,
    },
}

/// Stable policy revision consumed by rollout workers and trainers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRevision {
    /// Stable policy family, usually aligned with a checkpoint family.
    pub policy_family: String,
    /// Stable revision identifier.
    pub revision_id: String,
    /// Stable digest over the effective policy state.
    pub policy_digest: String,
    /// Parent revision when this is an incremental update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_revision_id: Option<String>,
    /// Checkpoint anchor when the policy is checkpoint-backed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<TrainingCheckpointReference>,
    /// Logical creation timestamp.
    pub produced_at_ms: u64,
}

impl PolicyRevision {
    /// Creates a policy revision.
    #[must_use]
    pub fn new(
        policy_family: impl Into<String>,
        revision_id: impl Into<String>,
        policy_digest: impl Into<String>,
        produced_at_ms: u64,
    ) -> Self {
        Self {
            policy_family: policy_family.into(),
            revision_id: revision_id.into(),
            policy_digest: policy_digest.into(),
            parent_revision_id: None,
            checkpoint: None,
            produced_at_ms,
        }
    }

    /// Attaches a parent revision.
    #[must_use]
    pub fn with_parent_revision_id(mut self, parent_revision_id: impl Into<String>) -> Self {
        self.parent_revision_id = Some(parent_revision_id.into());
        self
    }

    /// Attaches checkpoint lineage.
    #[must_use]
    pub fn with_checkpoint(mut self, checkpoint: TrainingCheckpointReference) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }
}

/// High-level rollout termination posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutTerminationReason {
    /// The rollout completed normally.
    Completed,
    /// The rollout stopped because the token budget ended.
    MaxTokens,
    /// The environment or task exited with an error.
    EnvironmentError,
    /// Tool or side-effect execution failed.
    ToolError,
    /// The rollout was cancelled externally.
    Cancelled,
    /// The rollout was rejected before later promotion or training.
    Rejected,
}

/// Proof family attached to a rollout artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutProofKind {
    /// General execution-proof bundle from Psionic runtime.
    ExecutionProof,
    /// Validator-owned rollout verification bundle.
    ValidatorBundle,
    /// Commitment or digest-only external reference.
    Commitment,
}

/// One proof-bearing reference attached to a rollout artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolloutProofReference {
    /// Proof family.
    pub kind: RolloutProofKind,
    /// Stable digest for the proof material.
    pub digest: String,
    /// Stable operator- or validator-visible reference.
    pub reference: String,
}

impl RolloutProofReference {
    /// Creates a proof reference.
    #[must_use]
    pub fn new(
        kind: RolloutProofKind,
        digest: impl Into<String>,
        reference: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            digest: digest.into(),
            reference: reference.into(),
        }
    }
}

/// One token- or step-level rollout sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutSample {
    /// Token identifier or step output identifier.
    pub token_id: u32,
    /// Log-probability emitted for the sample.
    pub logprob: f32,
    /// Reward attributed to the sample.
    pub reward: f32,
    /// Advantage attributed to the sample.
    pub advantage: f32,
}

impl RolloutSample {
    /// Creates a rollout sample.
    #[must_use]
    pub const fn new(token_id: u32, logprob: f32, reward: f32, advantage: f32) -> Self {
        Self {
            token_id,
            logprob,
            reward,
            advantage,
        }
    }
}

/// One reusable rollout artifact ready for trainer-batch assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolloutArtifact {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable environment package identity.
    pub environment: EnvironmentPackageKey,
    /// Stable task identifier or task digest.
    pub task_id: String,
    /// Policy revision used to generate the rollout.
    pub source_policy_revision: PolicyRevision,
    /// Token- or step-level samples.
    pub samples: Vec<RolloutSample>,
    /// Terminal rollout posture.
    pub termination_reason: RolloutTerminationReason,
    /// Proof-bearing references for validation or replay.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proof_references: Vec<RolloutProofReference>,
    /// Logical creation timestamp.
    pub created_at_ms: u64,
    /// Stable digest over the rollout contents.
    pub artifact_digest: String,
}

impl RolloutArtifact {
    /// Creates a rollout artifact and validates its proof references.
    pub fn new(
        artifact_id: impl Into<String>,
        worker_id: impl Into<String>,
        environment: EnvironmentPackageKey,
        task_id: impl Into<String>,
        source_policy_revision: PolicyRevision,
        samples: Vec<RolloutSample>,
        termination_reason: RolloutTerminationReason,
        proof_references: Vec<RolloutProofReference>,
        created_at_ms: u64,
    ) -> Result<Self, RolloutContractError> {
        let artifact_id = artifact_id.into();
        let worker_id = worker_id.into();
        let task_id = task_id.into();
        if samples.is_empty() {
            return Err(RolloutContractError::EmptyRolloutSamples { artifact_id });
        }
        let mut digests = BTreeSet::new();
        for proof in &proof_references {
            if !digests.insert(proof.digest.clone()) {
                return Err(RolloutContractError::DuplicateProofDigest {
                    artifact_id,
                    digest: proof.digest.clone(),
                });
            }
        }
        let artifact_digest = stable_rollout_artifact_digest(
            artifact_id.as_str(),
            worker_id.as_str(),
            environment.storage_key().as_str(),
            task_id.as_str(),
            &source_policy_revision,
            samples.as_slice(),
            termination_reason,
            proof_references.as_slice(),
            created_at_ms,
        );
        Ok(Self {
            artifact_id,
            worker_id,
            environment,
            task_id,
            source_policy_revision,
            samples,
            termination_reason,
            proof_references,
            created_at_ms,
            artifact_digest,
        })
    }

    /// Returns the number of token- or step-level samples.
    #[must_use]
    pub fn token_count(&self) -> u64 {
        self.samples.len() as u64
    }

    /// Returns the aggregate reward.
    #[must_use]
    pub fn reward_sum(&self) -> f32 {
        self.samples.iter().map(|sample| sample.reward).sum()
    }

    /// Returns the aggregate advantage.
    #[must_use]
    pub fn advantage_sum(&self) -> f32 {
        self.samples.iter().map(|sample| sample.advantage).sum()
    }
}

/// Deterministic policy-lineage summary carried by a trainer batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRevisionLineage {
    /// Target policy revision the trainer step is updating or evaluating.
    pub target_revision: PolicyRevision,
    /// Unique source policy revisions represented by accepted rollout artifacts.
    pub source_revisions: Vec<PolicyRevision>,
    /// Stable digest over the lineage.
    pub lineage_digest: String,
}

/// One trainer batch assembled from accepted rollout artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainerBatch {
    /// Stable batch identifier.
    pub batch_id: String,
    /// Policy lineage for the batch.
    pub policy_lineage: PolicyRevisionLineage,
    /// Stable rollout artifact identifiers.
    pub rollout_ids: Vec<String>,
    /// Stable rollout artifact digests.
    pub rollout_digests: Vec<String>,
    /// Unique proof references carried by the batch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proof_references: Vec<RolloutProofReference>,
    /// Number of rollout artifacts in the batch.
    pub rollout_count: u32,
    /// Number of token- or step-level samples in the batch.
    pub token_count: u64,
    /// Aggregate reward across all accepted artifacts.
    pub reward_sum: f32,
    /// Aggregate advantage across all accepted artifacts.
    pub advantage_sum: f32,
    /// Logical assembly timestamp.
    pub assembled_at_ms: u64,
    /// Stable digest over the batch contents.
    pub batch_digest: String,
}

impl TrainerBatch {
    /// Assembles a trainer batch from rollout artifacts and explicit target
    /// policy lineage.
    pub fn assemble(
        batch_id: impl Into<String>,
        target_revision: PolicyRevision,
        rollouts: Vec<RolloutArtifact>,
        assembled_at_ms: u64,
    ) -> Result<Self, RolloutContractError> {
        let batch_id = batch_id.into();
        if rollouts.is_empty() {
            return Err(RolloutContractError::EmptyTrainerBatch { batch_id });
        }

        let mut rollout_ids = Vec::new();
        let mut rollout_digests = Vec::new();
        let mut proof_refs = BTreeMap::new();
        let mut source_revisions = BTreeMap::new();
        let mut seen_rollout_ids = BTreeSet::new();
        let mut reward_sum = 0.0_f32;
        let mut advantage_sum = 0.0_f32;
        let mut token_count = 0_u64;

        for rollout in &rollouts {
            if rollout.source_policy_revision.policy_family != target_revision.policy_family {
                return Err(RolloutContractError::PolicyFamilyMismatch {
                    batch_id,
                    artifact_id: rollout.artifact_id.clone(),
                    expected_policy_family: target_revision.policy_family.clone(),
                    actual_policy_family: rollout.source_policy_revision.policy_family.clone(),
                });
            }
            if !seen_rollout_ids.insert(rollout.artifact_id.clone()) {
                return Err(RolloutContractError::DuplicateRolloutArtifact {
                    batch_id,
                    artifact_id: rollout.artifact_id.clone(),
                });
            }
            rollout_ids.push(rollout.artifact_id.clone());
            rollout_digests.push(rollout.artifact_digest.clone());
            reward_sum += rollout.reward_sum();
            advantage_sum += rollout.advantage_sum();
            token_count = token_count.saturating_add(rollout.token_count());
            source_revisions.insert(
                rollout.source_policy_revision.revision_id.clone(),
                rollout.source_policy_revision.clone(),
            );
            for proof in &rollout.proof_references {
                proof_refs
                    .entry(proof.digest.clone())
                    .or_insert_with(|| proof.clone());
            }
        }

        let source_revisions = source_revisions.into_values().collect::<Vec<_>>();
        let policy_lineage = PolicyRevisionLineage {
            lineage_digest: stable_policy_lineage_digest(
                &target_revision,
                source_revisions.as_slice(),
            ),
            target_revision,
            source_revisions,
        };
        let proof_references = proof_refs.into_values().collect::<Vec<_>>();
        let batch_digest = stable_trainer_batch_digest(
            batch_id.as_str(),
            &policy_lineage,
            rollout_ids.as_slice(),
            rollout_digests.as_slice(),
            proof_references.as_slice(),
            reward_sum,
            advantage_sum,
            token_count,
            assembled_at_ms,
        );
        Ok(Self {
            batch_id,
            policy_lineage,
            rollout_ids,
            rollout_digests,
            proof_references,
            rollout_count: rollouts.len() as u32,
            token_count,
            reward_sum,
            advantage_sum,
            assembled_at_ms,
            batch_digest,
        })
    }
}

fn stable_rollout_artifact_digest(
    artifact_id: &str,
    worker_id: &str,
    environment_id: &str,
    task_id: &str,
    source_policy_revision: &PolicyRevision,
    samples: &[RolloutSample],
    termination_reason: RolloutTerminationReason,
    proof_references: &[RolloutProofReference],
    created_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_artifact|");
    hasher.update(artifact_id.as_bytes());
    hasher.update(b"|worker|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|env|");
    hasher.update(environment_id.as_bytes());
    hasher.update(b"|task|");
    hasher.update(task_id.as_bytes());
    hasher.update(b"|policy_family|");
    hasher.update(source_policy_revision.policy_family.as_bytes());
    hasher.update(b"|revision|");
    hasher.update(source_policy_revision.revision_id.as_bytes());
    hasher.update(b"|policy_digest|");
    hasher.update(source_policy_revision.policy_digest.as_bytes());
    hasher.update(b"|created_at_ms|");
    hasher.update(created_at_ms.to_string().as_bytes());
    hasher.update(b"|termination|");
    hasher.update(rollout_termination_reason_label(termination_reason));
    for sample in samples {
        hasher.update(b"|sample|");
        hasher.update(sample.token_id.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(sample.logprob.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(sample.reward.to_bits().to_le_bytes());
        hasher.update(b"|");
        hasher.update(sample.advantage.to_bits().to_le_bytes());
    }
    for proof in proof_references {
        hasher.update(b"|proof|");
        hasher.update(rollout_proof_kind_label(proof.kind));
        hasher.update(b"|");
        hasher.update(proof.digest.as_bytes());
        hasher.update(b"|");
        hasher.update(proof.reference.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_policy_lineage_digest(
    target_revision: &PolicyRevision,
    source_revisions: &[PolicyRevision],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_policy_lineage|");
    hasher.update(target_revision.policy_family.as_bytes());
    hasher.update(b"|target|");
    hasher.update(target_revision.revision_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_revision.policy_digest.as_bytes());
    for source in source_revisions {
        hasher.update(b"|source|");
        hasher.update(source.revision_id.as_bytes());
        hasher.update(b"|");
        hasher.update(source.policy_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_trainer_batch_digest(
    batch_id: &str,
    policy_lineage: &PolicyRevisionLineage,
    rollout_ids: &[String],
    rollout_digests: &[String],
    proof_references: &[RolloutProofReference],
    reward_sum: f32,
    advantage_sum: f32,
    token_count: u64,
    assembled_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_trainer_batch|");
    hasher.update(batch_id.as_bytes());
    hasher.update(b"|lineage|");
    hasher.update(policy_lineage.lineage_digest.as_bytes());
    hasher.update(b"|reward_sum|");
    hasher.update(reward_sum.to_bits().to_le_bytes());
    hasher.update(b"|advantage_sum|");
    hasher.update(advantage_sum.to_bits().to_le_bytes());
    hasher.update(b"|token_count|");
    hasher.update(token_count.to_string().as_bytes());
    hasher.update(b"|assembled_at_ms|");
    hasher.update(assembled_at_ms.to_string().as_bytes());
    for rollout_id in rollout_ids {
        hasher.update(b"|rollout_id|");
        hasher.update(rollout_id.as_bytes());
    }
    for rollout_digest in rollout_digests {
        hasher.update(b"|rollout_digest|");
        hasher.update(rollout_digest.as_bytes());
    }
    for proof in proof_references {
        hasher.update(b"|proof|");
        hasher.update(proof.digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn rollout_termination_reason_label(reason: RolloutTerminationReason) -> &'static [u8] {
    match reason {
        RolloutTerminationReason::Completed => b"completed",
        RolloutTerminationReason::MaxTokens => b"max_tokens",
        RolloutTerminationReason::EnvironmentError => b"environment_error",
        RolloutTerminationReason::ToolError => b"tool_error",
        RolloutTerminationReason::Cancelled => b"cancelled",
        RolloutTerminationReason::Rejected => b"rejected",
    }
}

fn rollout_proof_kind_label(kind: RolloutProofKind) -> &'static [u8] {
    match kind {
        RolloutProofKind::ExecutionProof => b"execution_proof",
        RolloutProofKind::ValidatorBundle => b"validator_bundle",
        RolloutProofKind::Commitment => b"commitment",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic_in_result_fn)]

    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_environments::EnvironmentPackageKey;
    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterSnapshot, ClusterState, NodeEpoch, NodeId,
        NodeRole,
    };
    use psionic_datastream::{
        DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind,
    };

    use crate::TrainingSessionState;

    use super::{
        PolicyRevision, RolloutArtifact, RolloutContractError, RolloutProofKind,
        RolloutProofReference, RolloutSample, RolloutTerminationReason, TrainerBatch,
    };

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("train-rollout-cluster"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn membership(
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
                    membership(&cluster_id, node_id, *status),
                )
            })
            .collect::<BTreeMap<_, _>>();
        ClusterState::from_snapshot(snapshot)
    }

    fn checkpoint_backed_policy(
        revision_id: &str,
        policy_digest: &str,
        checkpoint_ref: &str,
        step: u64,
    ) -> Result<PolicyRevision, Box<dyn std::error::Error>> {
        let state = cluster_state(&[
            ("worker-a", ClusterMembershipStatus::Ready),
            ("worker-b", ClusterMembershipStatus::Ready),
        ]);
        let manifest = DatastreamManifest::from_bytes(
            format!("checkpoint-stream-{revision_id}"),
            DatastreamSubjectKind::Checkpoint,
            b"checkpoint-bytes",
            4,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("train.decoder")
                .with_checkpoint_ref(checkpoint_ref)
                .with_step(step),
        );
        let mut session = TrainingSessionState::new(state.cluster_id().as_str(), "train.decoder");
        let write =
            session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;
        session.mark_checkpoint_durable(write.write_id.as_str(), 1_040)?;
        Ok(PolicyRevision::new("train.decoder", revision_id, policy_digest, 1_050)
            .with_checkpoint(
                session
                    .latest_durable_checkpoint()
                    .expect("durable checkpoint")
                    .clone(),
            ))
    }

    #[test]
    fn rollout_artifacts_and_trainer_batch_are_machine_legible()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_a =
            checkpoint_backed_policy("policy-r1", "digest-r1", "step-12", 12)?;
        let source_b =
            checkpoint_backed_policy("policy-r2", "digest-r2", "step-16", 16)?
                .with_parent_revision_id("policy-r1");
        let target = PolicyRevision::new("train.decoder", "policy-r3", "digest-r3", 1_200)
            .with_parent_revision_id("policy-r2");
        let rollout_a = RolloutArtifact::new(
            "rollout-a",
            "worker-a",
            EnvironmentPackageKey::new("env.weather", "1"),
            "task-paris",
            source_a,
            vec![
                RolloutSample::new(101, -0.2, 0.4, 0.3),
                RolloutSample::new(202, -0.3, 0.1, 0.2),
            ],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                "proof-a",
                "exec://proof-a",
            )],
            1_100,
        )?;
        let rollout_b = RolloutArtifact::new(
            "rollout-b",
            "worker-b",
            EnvironmentPackageKey::new("env.weather", "1"),
            "task-berlin",
            source_b,
            vec![
                RolloutSample::new(303, -0.1, 0.2, 0.05),
                RolloutSample::new(404, -0.4, -0.1, -0.02),
            ],
            RolloutTerminationReason::MaxTokens,
            vec![
                RolloutProofReference::new(
                    RolloutProofKind::ExecutionProof,
                    "proof-b",
                    "exec://proof-b",
                ),
                RolloutProofReference::new(
                    RolloutProofKind::ValidatorBundle,
                    "validator-1",
                    "validator://bundle-1",
                ),
            ],
            1_110,
        )?;

        let batch = TrainerBatch::assemble("trainer-batch-1", target, vec![rollout_a, rollout_b], 1_300)?;

        assert_eq!(batch.rollout_count, 2);
        assert_eq!(batch.token_count, 4);
        assert!((batch.reward_sum - 0.6).abs() < 0.0001);
        assert!((batch.advantage_sum - 0.53).abs() < 0.0001);
        assert_eq!(batch.policy_lineage.source_revisions.len(), 2);
        assert_eq!(batch.policy_lineage.source_revisions[0].revision_id, "policy-r1");
        assert_eq!(batch.policy_lineage.source_revisions[1].revision_id, "policy-r2");
        assert_eq!(batch.proof_references.len(), 3);
        assert!(!batch.batch_digest.is_empty());
        assert!(!batch.policy_lineage.lineage_digest.is_empty());
        Ok(())
    }

    #[test]
    fn trainer_batch_refuses_cross_family_rollouts() -> Result<(), Box<dyn std::error::Error>> {
        let rollout = RolloutArtifact::new(
            "rollout-cross",
            "worker-a",
            EnvironmentPackageKey::new("env.weather", "1"),
            "task-cross",
            PolicyRevision::new("other.family", "policy-r1", "digest-r1", 100),
            vec![RolloutSample::new(1, -0.2, 0.5, 0.4)],
            RolloutTerminationReason::Completed,
            Vec::new(),
            110,
        )?;

        let error = TrainerBatch::assemble(
            "trainer-batch-cross",
            PolicyRevision::new("train.decoder", "policy-r2", "digest-r2", 120),
            vec![rollout],
            130,
        )
        .expect_err("cross-family rollout should be refused");

        assert_eq!(
            error,
            RolloutContractError::PolicyFamilyMismatch {
                batch_id: String::from("trainer-batch-cross"),
                artifact_id: String::from("rollout-cross"),
                expected_policy_family: String::from("train.decoder"),
                actual_policy_family: String::from("other.family"),
            }
        );
        Ok(())
    }

    #[test]
    fn rollout_artifact_requires_samples() {
        let artifact = RolloutArtifact::new(
            "rollout-empty",
            "worker-a",
            EnvironmentPackageKey::new("env.weather", "1"),
            "task-empty",
            PolicyRevision::new("train.decoder", "policy-r1", "digest-r1", 100),
            Vec::new(),
            RolloutTerminationReason::Rejected,
            Vec::new(),
            120,
        );
        assert_eq!(
            artifact.expect_err("empty rollout should be refused"),
            RolloutContractError::EmptyRolloutSamples {
                artifact_id: String::from("rollout-empty"),
            }
        );
    }
}
