use std::collections::BTreeMap;

use psionic_cluster::NodeId;
use psionic_datastream::DatastreamManifestRef;
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TrainingSessionState;

/// Scope kind for checkpoint pointers and manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointScopeKind {
    /// Pointer or manifest scoped to a full run.
    Run,
    /// Pointer or manifest scoped to one stage.
    Stage,
    /// Pointer or manifest scoped to one training window.
    Window,
}

/// Stable run/stage/window scope for checkpoint contracts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointScopeBinding {
    /// Scope kind.
    pub kind: CheckpointScopeKind,
    /// Stable scope id.
    pub scope_id: String,
}

impl CheckpointScopeBinding {
    /// Creates a scope binding.
    #[must_use]
    pub fn new(kind: CheckpointScopeKind, scope_id: impl Into<String>) -> Self {
        Self {
            kind,
            scope_id: scope_id.into(),
        }
    }

    /// Returns a stable storage key for the scope.
    #[must_use]
    pub fn storage_key(&self) -> String {
        format!(
            "{}:{}",
            checkpoint_scope_kind_label(self.kind),
            self.scope_id
        )
    }
}

/// Durability posture for one checkpoint manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointDurabilityPosture {
    /// Upload or flush is incomplete.
    PartialUpload,
    /// The checkpoint is fully durable.
    Durable,
}

/// Recovery mode requested for one restore plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRecoveryMode {
    /// Block recovery until catch-up is complete.
    BlockingCatchUp,
    /// Overlap catch-up with ongoing progress when policy allows.
    OverlappedCatchUp,
    /// Resume directly from the last stable durable checkpoint.
    ResumeFromLastStableCheckpoint,
}

/// One shard inside a checkpoint manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointShardManifest {
    /// Stable shard id.
    pub shard_id: String,
    /// Datastream-backed shard ref.
    pub manifest: DatastreamManifestRef,
    /// Writer node id for the shard.
    pub writer_node_id: String,
}

/// Explicit checkpoint manifest over one or more shards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointManifest {
    /// Checkpoint scope.
    pub scope: CheckpointScopeBinding,
    /// Checkpoint family.
    pub checkpoint_family: String,
    /// Checkpoint identity.
    pub checkpoint: TrainingCheckpointReference,
    /// Shards included in the checkpoint.
    pub shards: Vec<CheckpointShardManifest>,
    /// Durability posture.
    pub durability: CheckpointDurabilityPosture,
    /// Creation timestamp.
    pub created_at_ms: u64,
    /// Stable digest.
    pub manifest_digest: String,
}

impl CheckpointManifest {
    /// Creates and validates a checkpoint manifest.
    pub fn new(
        scope: CheckpointScopeBinding,
        checkpoint_family: impl Into<String>,
        checkpoint: TrainingCheckpointReference,
        shards: Vec<CheckpointShardManifest>,
        durability: CheckpointDurabilityPosture,
        created_at_ms: u64,
    ) -> Result<Self, CheckpointRecoveryError> {
        let checkpoint_family = checkpoint_family.into();
        if scope.scope_id.trim().is_empty() {
            return Err(CheckpointRecoveryError::MissingScopeId);
        }
        if checkpoint_family.trim().is_empty() {
            return Err(CheckpointRecoveryError::MissingCheckpointFamily);
        }
        if shards.is_empty() {
            return Err(CheckpointRecoveryError::CheckpointManifestHasNoShards);
        }
        let manifest_digest = stable_checkpoint_manifest_digest(
            &scope,
            checkpoint_family.as_str(),
            &checkpoint,
            shards.as_slice(),
            durability,
            created_at_ms,
        );
        Ok(Self {
            scope,
            checkpoint_family,
            checkpoint,
            shards,
            durability,
            created_at_ms,
            manifest_digest,
        })
    }
}

/// Stable pointer to the latest accepted checkpoint for one scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPointer {
    /// Checkpoint scope.
    pub scope: CheckpointScopeBinding,
    /// Checkpoint family.
    pub checkpoint_family: String,
    /// Latest accepted checkpoint.
    pub checkpoint: TrainingCheckpointReference,
    /// Digest of the preferred manifest.
    pub manifest_digest: String,
    /// Pointer update time.
    pub updated_at_ms: u64,
    /// Stable digest over the pointer.
    pub pointer_digest: String,
}

impl CheckpointPointer {
    /// Creates a pointer.
    pub fn new(
        scope: CheckpointScopeBinding,
        checkpoint_family: impl Into<String>,
        checkpoint: TrainingCheckpointReference,
        manifest_digest: impl Into<String>,
        updated_at_ms: u64,
    ) -> Result<Self, CheckpointRecoveryError> {
        let checkpoint_family = checkpoint_family.into();
        let manifest_digest = manifest_digest.into();
        if scope.scope_id.trim().is_empty() {
            return Err(CheckpointRecoveryError::MissingScopeId);
        }
        if checkpoint_family.trim().is_empty() {
            return Err(CheckpointRecoveryError::MissingCheckpointFamily);
        }
        if manifest_digest.trim().is_empty() {
            return Err(CheckpointRecoveryError::MissingManifestDigest);
        }
        let pointer_digest = stable_checkpoint_pointer_digest(
            &scope,
            checkpoint_family.as_str(),
            &checkpoint,
            manifest_digest.as_str(),
            updated_at_ms,
        );
        Ok(Self {
            scope,
            checkpoint_family,
            checkpoint,
            manifest_digest,
            updated_at_ms,
            pointer_digest,
        })
    }
}

/// Restore source used by a recovery plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointReadSourceKind {
    /// Preferred pointer lookup.
    PointerLookup,
    /// Manifest listing fallback.
    ManifestListingFallback,
}

/// One attempt in the restore ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRestoreAttempt {
    /// Source kind attempted.
    pub source_kind: CheckpointReadSourceKind,
    /// Manifest digest attempted when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_digest: Option<String>,
    /// Whether the source was accepted.
    pub accepted: bool,
    /// Human-readable machine-legible detail.
    pub detail: String,
}

/// Deterministic uploader assignment for one shard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointShardUploaderAssignment {
    /// Shard id.
    pub shard_id: String,
    /// Selected uploader node id.
    pub uploader_node_id: String,
}

/// Durable recovery receipt explaining why one source was selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRestoreReceipt {
    /// Requested recovery mode.
    pub recovery_mode: TrainingRecoveryMode,
    /// Selected read source.
    pub source_kind: CheckpointReadSourceKind,
    /// Pointer used when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_pointer: Option<CheckpointPointer>,
    /// Selected manifest.
    pub selected_manifest: CheckpointManifest,
    /// Restore ladder.
    pub attempts: Vec<CheckpointRestoreAttempt>,
    /// Deterministic shard uploader assignments.
    pub uploader_assignments: Vec<CheckpointShardUploaderAssignment>,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Options for manifest-listing fallback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointStoreReadOptions {
    /// Maximum number of manifests visible to the listing fallback when one exists.
    pub manifest_listing_limit: Option<usize>,
}

/// Fake in-memory checkpoint store used by train restore planning tests.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InMemoryCheckpointStore {
    pointers: BTreeMap<String, CheckpointPointer>,
    manifests: BTreeMap<String, Vec<CheckpointManifest>>,
}

impl InMemoryCheckpointStore {
    /// Stores one checkpoint pointer.
    pub fn store_pointer(&mut self, pointer: CheckpointPointer) {
        self.pointers.insert(
            pointer_key(&pointer.scope, pointer.checkpoint_family.as_str()),
            pointer,
        );
    }

    /// Removes one checkpoint pointer.
    pub fn remove_pointer(&mut self, scope: &CheckpointScopeBinding, checkpoint_family: &str) {
        self.pointers.remove(&pointer_key(scope, checkpoint_family));
    }

    /// Stores one checkpoint manifest.
    pub fn store_manifest(&mut self, manifest: CheckpointManifest) {
        self.manifests
            .entry(pointer_key(
                &manifest.scope,
                manifest.checkpoint_family.as_str(),
            ))
            .or_default()
            .push(manifest);
    }

    /// Plans a restore from pointers first and listing fallback second.
    pub fn plan_restore(
        &self,
        scope: &CheckpointScopeBinding,
        checkpoint_family: &str,
        recovery_mode: TrainingRecoveryMode,
        uploader_candidates: &[NodeId],
        read_options: CheckpointStoreReadOptions,
    ) -> Result<CheckpointRestoreReceipt, CheckpointRecoveryError> {
        let key = pointer_key(scope, checkpoint_family);
        let mut attempts = Vec::new();
        let manifests = self
            .manifests
            .get(&key)
            .map(|manifests| apply_listing_limit(manifests.as_slice(), read_options))
            .unwrap_or_default();

        if let Some(pointer) = self.pointers.get(&key) {
            let maybe_manifest = manifests
                .iter()
                .find(|manifest| manifest.manifest_digest == pointer.manifest_digest);
            if let Some(manifest) = maybe_manifest {
                if manifest.durability == CheckpointDurabilityPosture::Durable {
                    attempts.push(CheckpointRestoreAttempt {
                        source_kind: CheckpointReadSourceKind::PointerLookup,
                        manifest_digest: Some(manifest.manifest_digest.clone()),
                        accepted: true,
                        detail: String::from(
                            "pointer lookup selected a durable manifest for restore",
                        ),
                    });
                    return Ok(build_restore_receipt(
                        recovery_mode,
                        CheckpointReadSourceKind::PointerLookup,
                        Some(pointer.clone()),
                        manifest.clone(),
                        attempts,
                        uploader_candidates,
                    ));
                }
                attempts.push(CheckpointRestoreAttempt {
                    source_kind: CheckpointReadSourceKind::PointerLookup,
                    manifest_digest: Some(manifest.manifest_digest.clone()),
                    accepted: false,
                    detail: String::from(
                        "pointer lookup found a manifest but it is not yet durable",
                    ),
                });
            } else {
                attempts.push(CheckpointRestoreAttempt {
                    source_kind: CheckpointReadSourceKind::PointerLookup,
                    manifest_digest: Some(pointer.manifest_digest.clone()),
                    accepted: false,
                    detail: String::from(
                        "pointer lookup could not find the referenced manifest in store",
                    ),
                });
            }
        } else {
            attempts.push(CheckpointRestoreAttempt {
                source_kind: CheckpointReadSourceKind::PointerLookup,
                manifest_digest: None,
                accepted: false,
                detail: String::from("pointer lookup missing for the requested scope"),
            });
        }

        let manifest = manifests
            .into_iter()
            .filter(|manifest| manifest.durability == CheckpointDurabilityPosture::Durable)
            .max_by(|left, right| {
                left.checkpoint
                    .step
                    .unwrap_or_default()
                    .cmp(&right.checkpoint.step.unwrap_or_default())
                    .then_with(|| left.created_at_ms.cmp(&right.created_at_ms))
            })
            .ok_or(CheckpointRecoveryError::NoDurableCheckpointSource {
                scope: scope.storage_key(),
                checkpoint_family: String::from(checkpoint_family),
            })?;
        attempts.push(CheckpointRestoreAttempt {
            source_kind: CheckpointReadSourceKind::ManifestListingFallback,
            manifest_digest: Some(manifest.manifest_digest.clone()),
            accepted: true,
            detail: String::from("manifest listing fallback selected the latest durable manifest"),
        });
        Ok(build_restore_receipt(
            recovery_mode,
            CheckpointReadSourceKind::ManifestListingFallback,
            self.pointers.get(&key).cloned(),
            manifest,
            attempts,
            uploader_candidates,
        ))
    }
}

/// Checkpoint lineage/recovery error.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CheckpointRecoveryError {
    /// Missing scope id.
    #[error("checkpoint scope is missing `scope_id`")]
    MissingScopeId,
    /// Missing checkpoint family.
    #[error("checkpoint contract is missing `checkpoint_family`")]
    MissingCheckpointFamily,
    /// Missing manifest digest.
    #[error("checkpoint pointer is missing `manifest_digest`")]
    MissingManifestDigest,
    /// No shards in manifest.
    #[error("checkpoint manifest must contain at least one shard")]
    CheckpointManifestHasNoShards,
    /// No durable checkpoint available in the session.
    #[error("training session has no durable checkpoint to export")]
    DurableCheckpointMissing,
    /// No durable manifest available in the session.
    #[error("training session has no durable manifest to export")]
    DurableManifestMissing,
    /// No durable restore source available in the store.
    #[error(
        "checkpoint store has no durable restore source for scope `{scope}` and family `{checkpoint_family}`"
    )]
    NoDurableCheckpointSource {
        /// Scope key.
        scope: String,
        /// Checkpoint family.
        checkpoint_family: String,
    },
}

impl TrainingSessionState {
    /// Exports the latest durable checkpoint as an explicit manifest contract.
    pub fn checkpoint_manifest_for_latest_durable(
        &self,
        scope: CheckpointScopeBinding,
        writer_node_id: &NodeId,
        created_at_ms: u64,
    ) -> Result<CheckpointManifest, CheckpointRecoveryError> {
        let checkpoint = self
            .latest_durable_checkpoint
            .clone()
            .ok_or(CheckpointRecoveryError::DurableCheckpointMissing)?;
        let manifest = self
            .latest_durable_manifest
            .clone()
            .ok_or(CheckpointRecoveryError::DurableManifestMissing)?;
        CheckpointManifest::new(
            scope,
            self.checkpoint_family.clone(),
            checkpoint,
            vec![CheckpointShardManifest {
                shard_id: String::from("shard-0"),
                manifest,
                writer_node_id: String::from(writer_node_id.as_str()),
            }],
            CheckpointDurabilityPosture::Durable,
            created_at_ms,
        )
    }

    /// Exports the latest durable checkpoint as an explicit pointer contract.
    pub fn checkpoint_pointer_for_latest_durable(
        &self,
        scope: CheckpointScopeBinding,
        manifest: &CheckpointManifest,
        updated_at_ms: u64,
    ) -> Result<CheckpointPointer, CheckpointRecoveryError> {
        let checkpoint = self
            .latest_durable_checkpoint
            .clone()
            .ok_or(CheckpointRecoveryError::DurableCheckpointMissing)?;
        CheckpointPointer::new(
            scope,
            self.checkpoint_family.clone(),
            checkpoint,
            manifest.manifest_digest.clone(),
            updated_at_ms,
        )
    }
}

fn pointer_key(scope: &CheckpointScopeBinding, checkpoint_family: &str) -> String {
    format!("{}|{}", scope.storage_key(), checkpoint_family)
}

fn apply_listing_limit(
    manifests: &[CheckpointManifest],
    read_options: CheckpointStoreReadOptions,
) -> Vec<CheckpointManifest> {
    let mut manifests = manifests.to_vec();
    manifests.sort_by(|left, right| {
        right
            .checkpoint
            .step
            .unwrap_or_default()
            .cmp(&left.checkpoint.step.unwrap_or_default())
            .then_with(|| right.created_at_ms.cmp(&left.created_at_ms))
    });
    if let Some(limit) = read_options.manifest_listing_limit {
        manifests.truncate(limit);
    }
    manifests
}

fn build_restore_receipt(
    recovery_mode: TrainingRecoveryMode,
    source_kind: CheckpointReadSourceKind,
    selected_pointer: Option<CheckpointPointer>,
    selected_manifest: CheckpointManifest,
    attempts: Vec<CheckpointRestoreAttempt>,
    uploader_candidates: &[NodeId],
) -> CheckpointRestoreReceipt {
    let uploader_assignments =
        deterministic_shard_uploaders(selected_manifest.shards.as_slice(), uploader_candidates);
    let receipt_digest = stable_restore_receipt_digest(
        recovery_mode,
        source_kind,
        selected_pointer.as_ref(),
        &selected_manifest,
        attempts.as_slice(),
        uploader_assignments.as_slice(),
    );
    CheckpointRestoreReceipt {
        recovery_mode,
        source_kind,
        selected_pointer,
        selected_manifest,
        attempts,
        uploader_assignments,
        receipt_digest,
    }
}

fn deterministic_shard_uploaders(
    shards: &[CheckpointShardManifest],
    uploader_candidates: &[NodeId],
) -> Vec<CheckpointShardUploaderAssignment> {
    let mut uploader_candidates = uploader_candidates
        .iter()
        .map(|node_id| String::from(node_id.as_str()))
        .collect::<Vec<_>>();
    uploader_candidates.sort();
    shards
        .iter()
        .map(|shard| {
            let uploader_node_id = if uploader_candidates.is_empty() {
                shard.writer_node_id.clone()
            } else {
                let mut hasher = Sha256::new();
                hasher.update(shard.manifest.manifest_digest.as_bytes());
                hasher.update(b"|");
                hasher.update(shard.shard_id.as_bytes());
                let digest = hasher.finalize();
                let mut bytes = [0_u8; 8];
                bytes.copy_from_slice(&digest[..8]);
                let index = u64::from_be_bytes(bytes) as usize % uploader_candidates.len();
                uploader_candidates[index].clone()
            };
            CheckpointShardUploaderAssignment {
                shard_id: shard.shard_id.clone(),
                uploader_node_id,
            }
        })
        .collect()
}

fn stable_checkpoint_manifest_digest(
    scope: &CheckpointScopeBinding,
    checkpoint_family: &str,
    checkpoint: &TrainingCheckpointReference,
    shards: &[CheckpointShardManifest],
    durability: CheckpointDurabilityPosture,
    created_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_checkpoint_manifest|");
    hasher.update(scope.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_checkpoint_identity(checkpoint).as_bytes());
    if let Some(step) = checkpoint.step {
        hasher.update(b"|step|");
        hasher.update(step.to_string().as_bytes());
    }
    hasher.update(b"|");
    hasher.update(checkpoint_durability_label(durability));
    for shard in shards {
        hasher.update(b"|shard|");
        hasher.update(shard.shard_id.as_bytes());
        hasher.update(b"|");
        hasher.update(shard.manifest.manifest_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(shard.writer_node_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(created_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_checkpoint_pointer_digest(
    scope: &CheckpointScopeBinding,
    checkpoint_family: &str,
    checkpoint: &TrainingCheckpointReference,
    manifest_digest: &str,
    updated_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_checkpoint_pointer|");
    hasher.update(scope.storage_key().as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_checkpoint_identity(checkpoint).as_bytes());
    hasher.update(b"|");
    hasher.update(manifest_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(updated_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_restore_receipt_digest(
    recovery_mode: TrainingRecoveryMode,
    source_kind: CheckpointReadSourceKind,
    selected_pointer: Option<&CheckpointPointer>,
    selected_manifest: &CheckpointManifest,
    attempts: &[CheckpointRestoreAttempt],
    uploader_assignments: &[CheckpointShardUploaderAssignment],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_checkpoint_restore_receipt|");
    hasher.update(training_recovery_mode_label(recovery_mode));
    hasher.update(b"|");
    hasher.update(checkpoint_read_source_label(source_kind));
    if let Some(selected_pointer) = selected_pointer {
        hasher.update(b"|pointer|");
        hasher.update(selected_pointer.pointer_digest.as_bytes());
    }
    hasher.update(b"|manifest|");
    hasher.update(selected_manifest.manifest_digest.as_bytes());
    for attempt in attempts {
        hasher.update(b"|attempt|");
        hasher.update(checkpoint_read_source_label(attempt.source_kind));
        hasher.update(if attempt.accepted {
            b"|accepted|"
        } else {
            b"|rejected|"
        });
        hasher.update(attempt.detail.as_bytes());
    }
    for assignment in uploader_assignments {
        hasher.update(b"|uploader|");
        hasher.update(assignment.shard_id.as_bytes());
        hasher.update(b"|");
        hasher.update(assignment.uploader_node_id.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_checkpoint_identity(checkpoint: &TrainingCheckpointReference) -> &str {
    checkpoint
        .checkpoint_ref
        .as_deref()
        .unwrap_or(checkpoint.stream_id.as_str())
}

fn checkpoint_scope_kind_label(kind: CheckpointScopeKind) -> &'static str {
    match kind {
        CheckpointScopeKind::Run => "run",
        CheckpointScopeKind::Stage => "stage",
        CheckpointScopeKind::Window => "window",
    }
}

fn checkpoint_durability_label(durability: CheckpointDurabilityPosture) -> &'static [u8] {
    match durability {
        CheckpointDurabilityPosture::PartialUpload => b"partial_upload",
        CheckpointDurabilityPosture::Durable => b"durable",
    }
}

fn training_recovery_mode_label(mode: TrainingRecoveryMode) -> &'static [u8] {
    match mode {
        TrainingRecoveryMode::BlockingCatchUp => b"blocking_catch_up",
        TrainingRecoveryMode::OverlappedCatchUp => b"overlapped_catch_up",
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint => {
            b"resume_from_last_stable_checkpoint"
        }
    }
}

fn checkpoint_read_source_label(source_kind: CheckpointReadSourceKind) -> &'static [u8] {
    match source_kind {
        CheckpointReadSourceKind::PointerLookup => b"pointer_lookup",
        CheckpointReadSourceKind::ManifestListingFallback => b"manifest_listing_fallback",
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
        DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
        DatastreamSubjectKind,
    };

    use super::{
        CheckpointDurabilityPosture, CheckpointReadSourceKind, CheckpointRecoveryError,
        CheckpointScopeBinding, CheckpointScopeKind, CheckpointStoreReadOptions,
        InMemoryCheckpointStore, TrainingRecoveryMode,
    };
    use crate::TrainingSessionState;

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("checkpoint-recovery"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([(
            NodeId::new("worker-a"),
            ClusterMembershipRecord::new(
                ClusterNodeIdentity {
                    cluster_id,
                    node_id: NodeId::new("worker-a"),
                    node_epoch: NodeEpoch::initial(),
                    role: NodeRole::ExecutorOnly,
                    auth_public_key: String::from("worker-a-pk"),
                    attestation: None,
                },
                Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 30_000)),
                ClusterMembershipStatus::Ready,
            ),
        )]);
        psionic_cluster::ClusterState::from_snapshot(snapshot)
    }

    fn durable_session()
    -> Result<(TrainingSessionState, DatastreamManifestRef), Box<dyn std::error::Error>> {
        let state = cluster_state();
        let payload = vec![7_u8; 32];
        let manifest = DatastreamManifest::from_bytes(
            "checkpoint-stream",
            DatastreamSubjectKind::Checkpoint,
            payload.as_slice(),
            8,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("train.decoder")
                .with_checkpoint_ref("step-12")
                .with_step(12),
        );
        let mut session = TrainingSessionState::new(state.cluster_id().as_str(), "train.decoder");
        let write =
            session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;
        session.mark_checkpoint_durable(write.write_id.as_str(), 1_200)?;
        Ok((session, manifest.manifest_ref()))
    }

    #[test]
    fn durable_session_emits_explicit_pointer_and_manifest_contracts()
    -> Result<(), Box<dyn std::error::Error>> {
        let (session, manifest_ref) = durable_session()?;
        let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, "train-run-1");
        let manifest = session.checkpoint_manifest_for_latest_durable(
            scope.clone(),
            &NodeId::new("worker-a"),
            1_250,
        )?;
        let pointer = session.checkpoint_pointer_for_latest_durable(scope, &manifest, 1_260)?;
        assert_eq!(manifest.shards.len(), 1);
        assert_eq!(manifest.shards[0].manifest, manifest_ref);
        assert_eq!(manifest.durability, CheckpointDurabilityPosture::Durable);
        assert_eq!(pointer.manifest_digest, manifest.manifest_digest);
        Ok(())
    }

    #[test]
    fn restore_prefers_pointer_before_listing_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let (session, _) = durable_session()?;
        let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, "train-run-2");
        let manifest = session.checkpoint_manifest_for_latest_durable(
            scope.clone(),
            &NodeId::new("worker-a"),
            1_250,
        )?;
        let pointer =
            session.checkpoint_pointer_for_latest_durable(scope.clone(), &manifest, 1_260)?;
        let mut store = InMemoryCheckpointStore::default();
        store.store_manifest(manifest.clone());
        store.store_pointer(pointer.clone());

        let receipt = store.plan_restore(
            &scope,
            "train.decoder",
            TrainingRecoveryMode::BlockingCatchUp,
            &[NodeId::new("worker-a"), NodeId::new("worker-b")],
            CheckpointStoreReadOptions::default(),
        )?;
        assert_eq!(receipt.source_kind, CheckpointReadSourceKind::PointerLookup);
        assert_eq!(
            receipt.selected_manifest.manifest_digest,
            manifest.manifest_digest
        );
        assert_eq!(receipt.selected_pointer, Some(pointer));
        assert_eq!(receipt.uploader_assignments.len(), 1);
        Ok(())
    }

    #[test]
    fn restore_falls_back_when_pointer_is_missing_or_stale()
    -> Result<(), Box<dyn std::error::Error>> {
        let (session, _) = durable_session()?;
        let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, "train-run-3");
        let manifest = session.checkpoint_manifest_for_latest_durable(
            scope.clone(),
            &NodeId::new("worker-a"),
            1_250,
        )?;
        let stale_pointer = super::CheckpointPointer::new(
            scope.clone(),
            "train.decoder",
            manifest.checkpoint.clone(),
            "missing-manifest-digest",
            1_260,
        )?;
        let mut store = InMemoryCheckpointStore::default();
        store.store_manifest(manifest.clone());
        store.store_pointer(stale_pointer);

        let receipt = store.plan_restore(
            &scope,
            "train.decoder",
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
            &[NodeId::new("worker-a")],
            CheckpointStoreReadOptions::default(),
        )?;
        assert_eq!(
            receipt.source_kind,
            CheckpointReadSourceKind::ManifestListingFallback
        );
        assert_eq!(
            receipt.selected_manifest.manifest_digest,
            manifest.manifest_digest
        );
        assert_eq!(receipt.attempts.len(), 2);
        Ok(())
    }

    #[test]
    fn restore_skips_partial_upload_and_respects_partial_listing_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let (session, _) = durable_session()?;
        let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, "train-run-4");
        let durable_manifest = session.checkpoint_manifest_for_latest_durable(
            scope.clone(),
            &NodeId::new("worker-a"),
            1_200,
        )?;
        let partial_manifest = super::CheckpointManifest::new(
            scope.clone(),
            "train.decoder",
            durable_manifest.checkpoint.clone().with_step(13),
            durable_manifest.shards.clone(),
            CheckpointDurabilityPosture::PartialUpload,
            1_300,
        )?;
        let mut store = InMemoryCheckpointStore::default();
        store.store_manifest(durable_manifest.clone());
        store.store_manifest(partial_manifest);

        let receipt = store.plan_restore(
            &scope,
            "train.decoder",
            TrainingRecoveryMode::OverlappedCatchUp,
            &[NodeId::new("worker-a")],
            CheckpointStoreReadOptions::default(),
        )?;
        assert_eq!(
            receipt.selected_manifest.manifest_digest,
            durable_manifest.manifest_digest
        );

        let err = store
            .plan_restore(
                &scope,
                "train.decoder",
                TrainingRecoveryMode::OverlappedCatchUp,
                &[NodeId::new("worker-a")],
                CheckpointStoreReadOptions {
                    manifest_listing_limit: Some(1),
                },
            )
            .expect_err("partial listing limited to the newest partial upload should fail");
        assert_eq!(
            err,
            CheckpointRecoveryError::NoDurableCheckpointSource {
                scope: scope.storage_key(),
                checkpoint_family: String::from("train.decoder"),
            }
        );
        Ok(())
    }
}
