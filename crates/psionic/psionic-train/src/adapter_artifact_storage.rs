use std::collections::BTreeMap;

use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind, DatastreamTransferError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    AdapterContributionUploadLocator, AdapterContributionWorkAssignment, ArtifactArchiveClass,
    ArtifactRetentionProfile, CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer,
    CheckpointRecoveryError, CheckpointScopeBinding, CheckpointScopeKind, CheckpointShardManifest,
    TrainArtifactClass, TrainArtifactStorageController, TrainArtifactStorageError,
};

/// Error surfaced by adapter artifact staging and recovery.
#[derive(Debug, Error)]
pub enum AdapterArtifactStorageError {
    /// The datastream contract rejected the requested resume or validation action.
    #[error(transparent)]
    Datastream(#[from] DatastreamTransferError),
    /// The generic train artifact controller rejected the request.
    #[error(transparent)]
    Storage(#[from] TrainArtifactStorageError),
    /// The checkpoint contract rejected the requested pointer or manifest.
    #[error(transparent)]
    Checkpoint(#[from] CheckpointRecoveryError),
    /// One upload session id was unknown.
    #[error("unknown adapter upload session `{upload_id}`")]
    UnknownUploadSession {
        /// Stable upload session identifier.
        upload_id: String,
    },
    /// One upload session is already complete.
    #[error("adapter upload session `{upload_id}` is already complete")]
    UploadAlreadyCompleted {
        /// Stable upload session identifier.
        upload_id: String,
    },
    /// The declared upload reference did not match the assignment expectation.
    #[error(
        "adapter upload session `{upload_id}` expected upload reference prefix `{expected_prefix}` but found `{actual_reference}`"
    )]
    UploadReferenceMismatch {
        /// Stable upload session identifier.
        upload_id: String,
        /// Expected reference prefix.
        expected_prefix: String,
        /// Actual upload reference.
        actual_reference: String,
    },
    /// The worker-declared manifest digest did not match the locally derived manifest.
    #[error(
        "adapter upload session `{upload_id}` declared manifest digest `{declared_manifest_digest}` but derived `{actual_manifest_digest}`"
    )]
    UploadManifestDigestMismatch {
        /// Stable upload session identifier.
        upload_id: String,
        /// Manifest digest declared by the worker.
        declared_manifest_digest: String,
        /// Manifest digest derived from the payload.
        actual_manifest_digest: String,
    },
    /// The worker-declared payload length did not match the derived payload bytes.
    #[error(
        "adapter upload session `{upload_id}` declared payload bytes `{declared_payload_bytes}` but derived `{actual_payload_bytes}`"
    )]
    UploadByteLengthMismatch {
        /// Stable upload session identifier.
        upload_id: String,
        /// Declared payload length.
        declared_payload_bytes: u64,
        /// Derived payload length.
        actual_payload_bytes: u64,
    },
    /// One chunk was submitted out of order.
    #[error(
        "adapter upload session `{upload_id}` expected chunk index `{expected_chunk_index}` but received `{actual_chunk_index}`"
    )]
    UploadChunkOutOfOrder {
        /// Stable upload session identifier.
        upload_id: String,
        /// Expected next chunk index.
        expected_chunk_index: usize,
        /// Actual attempted chunk index.
        actual_chunk_index: usize,
    },
    /// One chunk length mismatched the manifest declaration.
    #[error(
        "adapter upload session `{upload_id}` chunk `{chunk_index}` expected `{expected_length}` bytes but received `{actual_length}`"
    )]
    UploadChunkLengthMismatch {
        /// Stable upload session identifier.
        upload_id: String,
        /// Chunk index.
        chunk_index: usize,
        /// Expected chunk length.
        expected_length: usize,
        /// Actual chunk length.
        actual_length: usize,
    },
    /// One chunk digest mismatched the manifest declaration.
    #[error(
        "adapter upload session `{upload_id}` chunk `{chunk_index}` expected digest `{expected_digest}` but found `{actual_digest}`"
    )]
    UploadChunkDigestMismatch {
        /// Stable upload session identifier.
        upload_id: String,
        /// Chunk index.
        chunk_index: usize,
        /// Expected digest.
        expected_digest: String,
        /// Actual digest.
        actual_digest: String,
    },
    /// Completion was attempted before all chunks were committed.
    #[error(
        "adapter upload session `{upload_id}` is incomplete: next chunk `{next_chunk_index}` of `{chunk_count}`"
    )]
    UploadIncomplete {
        /// Stable upload session identifier.
        upload_id: String,
        /// Next missing chunk index.
        next_chunk_index: usize,
        /// Total chunk count.
        chunk_count: usize,
    },
    /// One contribution artifact record was not known.
    #[error("unknown adapter contribution `{contribution_id}`")]
    UnknownContribution {
        /// Stable contribution identifier.
        contribution_id: String,
    },
    /// One window had no promoted checkpoint to restore.
    #[error("adapter window `{window_id}` has no promoted checkpoint to restore")]
    UnknownWindowCheckpoint {
        /// Stable window identifier.
        window_id: String,
    },
}

/// Retention posture for one adapter contribution artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionArtifactDisposition {
    /// The artifact is staged and kept for validator or operator review.
    Reviewable,
    /// The artifact was accepted and should remain durable for later promotion lineage.
    Accepted,
    /// The artifact was rejected and may be garbage collected sooner.
    Rejected,
}

/// Retention policy split for accepted, reviewable, and rejected adapter artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterArtifactRetentionPolicy {
    /// Additional retention window for reviewable artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewable_retention_ms: Option<u64>,
    /// Additional retention window for accepted artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_retention_ms: Option<u64>,
    /// Additional retention window for rejected artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_retention_ms: Option<u64>,
}

impl Default for AdapterArtifactRetentionPolicy {
    fn default() -> Self {
        Self {
            reviewable_retention_ms: Some(7 * 24 * 60 * 60 * 1000),
            accepted_retention_ms: None,
            rejected_retention_ms: Some(24 * 60 * 60 * 1000),
        }
    }
}

impl AdapterArtifactRetentionPolicy {
    fn deadline_for(
        &self,
        disposition: AdapterContributionArtifactDisposition,
        observed_at_ms: u64,
    ) -> Option<u64> {
        match disposition {
            AdapterContributionArtifactDisposition::Reviewable => self
                .reviewable_retention_ms
                .map(|retention_ms| observed_at_ms.saturating_add(retention_ms)),
            AdapterContributionArtifactDisposition::Accepted => self
                .accepted_retention_ms
                .map(|retention_ms| observed_at_ms.saturating_add(retention_ms)),
            AdapterContributionArtifactDisposition::Rejected => self
                .rejected_retention_ms
                .map(|retention_ms| observed_at_ms.saturating_add(retention_ms)),
        }
    }
}

/// Upload lifecycle state for one adapter contribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterContributionUploadSessionStatus {
    /// The upload is still accepting chunks.
    InProgress,
    /// The upload is complete and now backed by a stable artifact ref.
    Completed,
}

/// Resume cursor for one interrupted upload session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionUploadResumeCursor {
    /// Stable upload session identifier.
    pub upload_id: String,
    /// Next required chunk index.
    pub next_chunk_index: usize,
    /// Bytes committed so far.
    pub committed_bytes: u64,
    /// Total payload bytes for the upload.
    pub total_bytes: u64,
    /// Stable manifest digest for the upload.
    pub manifest_digest: String,
    /// Stable object digest for the upload.
    pub object_digest: String,
}

/// In-flight upload session for one contribution artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionUploadSession {
    /// Stable upload session identifier.
    pub upload_id: String,
    /// Assignment that owns the upload.
    pub assignment: AdapterContributionWorkAssignment,
    /// Writer node that is staging the payload.
    pub writer_node_id: String,
    /// Worker-reported upload locator.
    pub upload: AdapterContributionUploadLocator,
    /// Derived datastream manifest for the payload.
    pub manifest: DatastreamManifest,
    /// Number of chunks committed so far.
    pub committed_chunk_count: usize,
    /// Upload lifecycle state.
    pub status: AdapterContributionUploadSessionStatus,
    /// Start time.
    pub started_at_ms: u64,
}

/// Durable artifact receipt for one completed contribution upload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContributionArtifactReceipt {
    /// Stable upload session identifier.
    pub upload_id: String,
    /// Stable artifact identifier from the generic artifact controller.
    pub artifact_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable contribution identifier.
    pub contribution_id: String,
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable writer node identifier.
    pub writer_node_id: String,
    /// Stable adapter target identifier.
    pub adapter_target_id: String,
    /// Stable dataset slice identifier.
    pub dataset_slice_id: String,
    /// Source policy revision id for the contribution.
    pub source_policy_revision_id: String,
    /// Source checkpoint pointer digest for the contribution.
    pub source_checkpoint_pointer_digest: String,
    /// Datastream manifest reference for the staged artifact.
    pub manifest: DatastreamManifestRef,
    /// Worker-reported upload locator.
    pub upload: AdapterContributionUploadLocator,
    /// Current retention posture for this artifact.
    pub disposition: AdapterContributionArtifactDisposition,
    /// Optional garbage-collection deadline under the current posture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_deadline_ms: Option<u64>,
    /// Optional garbage-collection completion time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub garbage_collected_at_ms: Option<u64>,
    /// Completion time.
    pub completed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Durable checkpoint receipt for one promoted adapter window state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWindowCheckpointReceipt {
    /// Stable artifact identifier from the generic artifact controller.
    pub artifact_id: String,
    /// Stable window identifier.
    pub window_id: String,
    /// Stable writer node identifier.
    pub writer_node_id: String,
    /// Stable policy revision id promoted by the checkpoint.
    pub policy_revision_id: String,
    /// Datastream manifest reference for the checkpoint payload.
    pub manifest: DatastreamManifestRef,
    /// Explicit checkpoint manifest.
    pub checkpoint_manifest: CheckpointManifest,
    /// Stable checkpoint pointer for window recovery.
    pub checkpoint_pointer: CheckpointPointer,
    /// Completion time.
    pub completed_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Adapter contribution artifact staging and checkpoint recovery state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdapterArtifactStorageState {
    /// Generic train artifact controller.
    pub controller: TrainArtifactStorageController,
    /// Retention policy split by contribution disposition.
    pub retention_policy: AdapterArtifactRetentionPolicy,
    /// In-flight contribution uploads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upload_sessions: Vec<AdapterContributionUploadSession>,
    /// Completed contribution artifacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contribution_artifacts: Vec<AdapterContributionArtifactReceipt>,
    /// Completed window checkpoints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub window_checkpoints: Vec<AdapterWindowCheckpointReceipt>,
}

impl AdapterArtifactStorageState {
    /// Creates an adapter artifact storage state with default artifact profiles.
    pub fn new(
        retention_policy: AdapterArtifactRetentionPolicy,
    ) -> Result<Self, AdapterArtifactStorageError> {
        let controller = TrainArtifactStorageController::new(BTreeMap::from([
            (
                TrainArtifactClass::AdapterContribution,
                ArtifactRetentionProfile::new(
                    60_000,
                    10 * 60_000,
                    ArtifactArchiveClass::Restorable,
                    60_000,
                )
                .with_deduplication(false),
            ),
            (
                TrainArtifactClass::AdapterWindowCheckpoint,
                ArtifactRetentionProfile::new(
                    60_000,
                    30 * 60_000,
                    ArtifactArchiveClass::Immutable,
                    60_000,
                )
                .with_deduplication(true),
            ),
        ]))?;
        Ok(Self {
            controller,
            retention_policy,
            upload_sessions: Vec::new(),
            contribution_artifacts: Vec::new(),
            window_checkpoints: Vec::new(),
        })
    }

    /// Starts one contribution upload by deriving and validating the manifest.
    pub fn start_contribution_upload(
        &mut self,
        assignment: &AdapterContributionWorkAssignment,
        upload: AdapterContributionUploadLocator,
        payload: &[u8],
        chunk_bytes: usize,
        writer_node_id: impl Into<String>,
        started_at_ms: u64,
    ) -> Result<AdapterContributionUploadResumeCursor, AdapterArtifactStorageError> {
        let writer_node_id = writer_node_id.into();
        if !upload
            .upload_reference
            .starts_with(&assignment.upload_expectation.upload_reference_prefix)
        {
            return Err(AdapterArtifactStorageError::UploadReferenceMismatch {
                upload_id: format!("{}-upload", assignment.contribution_id),
                expected_prefix: assignment
                    .upload_expectation
                    .upload_reference_prefix
                    .clone(),
                actual_reference: upload.upload_reference,
            });
        }

        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            payload,
            chunk_bytes.max(1),
            DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let manifest_ref = manifest.manifest_ref();
        let upload_id = format!(
            "{}-upload-{}",
            assignment.contribution_id,
            self.upload_sessions
                .iter()
                .filter(|session| session.assignment.contribution_id == assignment.contribution_id)
                .count()
                + 1
        );
        if manifest_ref.manifest_digest != upload.upload_manifest_digest {
            return Err(AdapterArtifactStorageError::UploadManifestDigestMismatch {
                upload_id,
                declared_manifest_digest: upload.upload_manifest_digest,
                actual_manifest_digest: manifest_ref.manifest_digest,
            });
        }
        if manifest_ref.total_bytes != upload.payload_bytes {
            return Err(AdapterArtifactStorageError::UploadByteLengthMismatch {
                upload_id,
                declared_payload_bytes: upload.payload_bytes,
                actual_payload_bytes: manifest_ref.total_bytes,
            });
        }
        let upload_id = format!(
            "{}-upload-{}",
            assignment.contribution_id,
            self.upload_sessions
                .iter()
                .filter(|session| session.assignment.contribution_id == assignment.contribution_id)
                .count()
                + 1
        );
        self.upload_sessions.push(AdapterContributionUploadSession {
            upload_id: upload_id.clone(),
            assignment: assignment.clone(),
            writer_node_id,
            upload,
            manifest,
            committed_chunk_count: 0,
            status: AdapterContributionUploadSessionStatus::InProgress,
            started_at_ms,
        });
        self.resume_upload(upload_id.as_str())
    }

    /// Returns the current resume cursor for one upload session.
    pub fn resume_upload(
        &self,
        upload_id: &str,
    ) -> Result<AdapterContributionUploadResumeCursor, AdapterArtifactStorageError> {
        let session = self
            .upload_sessions
            .iter()
            .find(|session| session.upload_id == upload_id)
            .ok_or_else(|| AdapterArtifactStorageError::UnknownUploadSession {
                upload_id: upload_id.to_string(),
            })?;
        let manifest_ref = session.manifest.manifest_ref();
        Ok(AdapterContributionUploadResumeCursor {
            upload_id: session.upload_id.clone(),
            next_chunk_index: session.committed_chunk_count,
            committed_bytes: session
                .manifest
                .bytes_committed_for_chunk_count(session.committed_chunk_count)?,
            total_bytes: manifest_ref.total_bytes,
            manifest_digest: manifest_ref.manifest_digest,
            object_digest: manifest_ref.object_digest,
        })
    }

    /// Commits the next expected chunk in one upload session.
    pub fn commit_next_chunk(
        &mut self,
        upload_id: &str,
        chunk_payload: &[u8],
    ) -> Result<AdapterContributionUploadResumeCursor, AdapterArtifactStorageError> {
        let session = self
            .upload_sessions
            .iter_mut()
            .find(|session| session.upload_id == upload_id)
            .ok_or_else(|| AdapterArtifactStorageError::UnknownUploadSession {
                upload_id: upload_id.to_string(),
            })?;
        if session.status == AdapterContributionUploadSessionStatus::Completed {
            return Err(AdapterArtifactStorageError::UploadAlreadyCompleted {
                upload_id: upload_id.to_string(),
            });
        }
        let expected_chunk_index = session.committed_chunk_count;
        let Some(expected_chunk) = session.manifest.chunks.get(expected_chunk_index) else {
            return Err(AdapterArtifactStorageError::UploadAlreadyCompleted {
                upload_id: upload_id.to_string(),
            });
        };
        if chunk_payload.len() != expected_chunk.length {
            return Err(AdapterArtifactStorageError::UploadChunkLengthMismatch {
                upload_id: upload_id.to_string(),
                chunk_index: expected_chunk.index,
                expected_length: expected_chunk.length,
                actual_length: chunk_payload.len(),
            });
        }
        let actual_digest = digest_bytes(chunk_payload);
        if actual_digest != expected_chunk.chunk_digest {
            return Err(AdapterArtifactStorageError::UploadChunkDigestMismatch {
                upload_id: upload_id.to_string(),
                chunk_index: expected_chunk.index,
                expected_digest: expected_chunk.chunk_digest.clone(),
                actual_digest,
            });
        }
        session.committed_chunk_count = session.committed_chunk_count.saturating_add(1);
        self.resume_upload(upload_id)
    }

    /// Completes one upload session into a durable contribution artifact receipt.
    pub fn complete_contribution_upload(
        &mut self,
        upload_id: &str,
        completed_at_ms: u64,
    ) -> Result<AdapterContributionArtifactReceipt, AdapterArtifactStorageError> {
        let session_index = self
            .upload_sessions
            .iter()
            .position(|session| session.upload_id == upload_id)
            .ok_or_else(|| AdapterArtifactStorageError::UnknownUploadSession {
                upload_id: upload_id.to_string(),
            })?;
        let session = self.upload_sessions[session_index].clone();
        if session.status == AdapterContributionUploadSessionStatus::Completed {
            return Err(AdapterArtifactStorageError::UploadAlreadyCompleted {
                upload_id: upload_id.to_string(),
            });
        }
        if session.committed_chunk_count != session.manifest.chunks.len() {
            return Err(AdapterArtifactStorageError::UploadIncomplete {
                upload_id: upload_id.to_string(),
                next_chunk_index: session.committed_chunk_count,
                chunk_count: session.manifest.chunks.len(),
            });
        }
        let manifest_ref = session.manifest.manifest_ref();
        let artifact_id = self.controller.register_adapter_contribution(
            manifest_ref.clone(),
            session.assignment.window_id.clone(),
            session.assignment.contribution_id.clone(),
            session.writer_node_id.clone(),
            manifest_ref.total_bytes,
            completed_at_ms,
        )?;
        self.upload_sessions[session_index].status =
            AdapterContributionUploadSessionStatus::Completed;
        let receipt = AdapterContributionArtifactReceipt {
            upload_id: session.upload_id.clone(),
            artifact_id,
            window_id: session.assignment.window_id.clone(),
            contribution_id: session.assignment.contribution_id.clone(),
            worker_id: session.assignment.worker_id.clone(),
            writer_node_id: session.writer_node_id.clone(),
            adapter_target_id: session.assignment.adapter_target.adapter_target_id.clone(),
            dataset_slice_id: session.assignment.dataset_slice.slice_id.clone(),
            source_policy_revision_id: session
                .assignment
                .source_policy_revision
                .revision_id
                .clone(),
            source_checkpoint_pointer_digest: session
                .assignment
                .source_checkpoint_pointer
                .pointer_digest
                .clone(),
            manifest: manifest_ref.clone(),
            upload: session.upload.clone(),
            disposition: AdapterContributionArtifactDisposition::Reviewable,
            retention_deadline_ms: self.retention_policy.deadline_for(
                AdapterContributionArtifactDisposition::Reviewable,
                completed_at_ms,
            ),
            garbage_collected_at_ms: None,
            completed_at_ms,
            receipt_digest: stable_contribution_receipt_digest(
                session.upload_id.as_str(),
                session.assignment.window_id.as_str(),
                session.assignment.contribution_id.as_str(),
                session.assignment.worker_id.as_str(),
                session.writer_node_id.as_str(),
                manifest_ref.manifest_digest.as_str(),
                manifest_ref.object_digest.as_str(),
                completed_at_ms,
            ),
        };
        self.contribution_artifacts.push(receipt.clone());
        Ok(receipt)
    }

    /// Updates the retention posture for one contribution artifact.
    pub fn set_contribution_disposition(
        &mut self,
        contribution_id: &str,
        disposition: AdapterContributionArtifactDisposition,
        observed_at_ms: u64,
    ) -> Result<(), AdapterArtifactStorageError> {
        let record = self
            .contribution_artifacts
            .iter_mut()
            .find(|record| {
                record.contribution_id == contribution_id
                    && record.garbage_collected_at_ms.is_none()
            })
            .ok_or_else(|| AdapterArtifactStorageError::UnknownContribution {
                contribution_id: contribution_id.to_string(),
            })?;
        record.disposition = disposition;
        record.retention_deadline_ms = self
            .retention_policy
            .deadline_for(disposition, observed_at_ms);
        Ok(())
    }

    /// Garbage collects contribution artifacts whose current posture expired.
    pub fn garbage_collect_expired_contributions(&mut self, observed_at_ms: u64) -> Vec<String> {
        let mut collected = Vec::new();
        for record in &mut self.contribution_artifacts {
            if record.garbage_collected_at_ms.is_none()
                && record
                    .retention_deadline_ms
                    .is_some_and(|deadline_ms| observed_at_ms >= deadline_ms)
            {
                record.garbage_collected_at_ms = Some(observed_at_ms);
                collected.push(record.artifact_id.clone());
            }
        }
        collected
    }

    /// Promotes one window payload into a durable checkpoint manifest and pointer.
    pub fn promote_window_checkpoint(
        &mut self,
        window_id: &str,
        policy_revision: &crate::PolicyRevision,
        payload: &[u8],
        chunk_bytes: usize,
        writer_node_id: impl Into<String>,
        completed_at_ms: u64,
    ) -> Result<AdapterWindowCheckpointReceipt, AdapterArtifactStorageError> {
        let writer_node_id = writer_node_id.into();
        let checkpoint_ref = policy_revision.checkpoint.clone().unwrap_or_else(|| {
            psionic_runtime::TrainingCheckpointReference::new(
                policy_revision.policy_family.clone(),
                format!(
                    "stream://adapter-window-checkpoint/{window_id}/{}",
                    policy_revision.revision_id
                ),
                String::new(),
                String::new(),
                writer_node_id.clone(),
                1,
                format!("cluster-digest:{window_id}"),
                format!("topology-digest:{window_id}"),
                completed_at_ms,
            )
            .with_checkpoint_ref(format!(
                "adapter-window/{window_id}/{}",
                policy_revision.revision_id
            ))
            .with_step(policy_revision.revision_number.unwrap_or_default())
            .with_durable_at_ms(completed_at_ms)
        });
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-window-checkpoint:{}:{}",
                window_id, policy_revision.revision_id
            ),
            DatastreamSubjectKind::Checkpoint,
            payload,
            chunk_bytes.max(1),
            DatastreamEncoding::RawBinary,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new(policy_revision.policy_family.clone())
                .with_checkpoint_ref(
                    checkpoint_ref
                        .checkpoint_ref
                        .clone()
                        .unwrap_or_else(|| format!("adapter-window/{window_id}")),
                )
                .with_step(checkpoint_ref.step.unwrap_or_default()),
        )
        .with_provenance_digest(policy_revision.policy_digest.clone());
        let manifest_ref = manifest.manifest_ref();
        let checkpoint = psionic_runtime::TrainingCheckpointReference::new(
            policy_revision.policy_family.clone(),
            manifest_ref.stream_id.clone(),
            manifest_ref.manifest_digest.clone(),
            manifest_ref.object_digest.clone(),
            writer_node_id.clone(),
            checkpoint_ref.membership_epoch,
            checkpoint_ref.cluster_state_digest.clone(),
            checkpoint_ref.topology_digest.clone(),
            checkpoint_ref.started_at_ms,
        )
        .with_checkpoint_ref(
            checkpoint_ref
                .checkpoint_ref
                .clone()
                .unwrap_or_else(|| format!("adapter-window/{window_id}")),
        )
        .with_step(checkpoint_ref.step.unwrap_or_default())
        .with_durable_at_ms(completed_at_ms);
        let checkpoint_manifest = CheckpointManifest::new(
            CheckpointScopeBinding::new(CheckpointScopeKind::Window, window_id),
            policy_revision.policy_family.clone(),
            checkpoint.clone(),
            vec![CheckpointShardManifest {
                shard_id: String::from("adapter-window-shard-0"),
                manifest: manifest_ref.clone(),
                writer_node_id: writer_node_id.clone(),
            }],
            CheckpointDurabilityPosture::Durable,
            completed_at_ms,
        )?;
        let checkpoint_pointer = CheckpointPointer::new(
            CheckpointScopeBinding::new(CheckpointScopeKind::Window, window_id),
            policy_revision.policy_family.clone(),
            checkpoint.clone(),
            checkpoint_manifest.manifest_digest.clone(),
            completed_at_ms,
        )?;
        let artifact_id = self.controller.register_adapter_window_checkpoint(
            manifest_ref.clone(),
            checkpoint.clone(),
            window_id.to_string(),
            policy_revision.revision_id.clone(),
            manifest_ref.total_bytes,
            completed_at_ms,
        )?;
        let receipt = AdapterWindowCheckpointReceipt {
            artifact_id,
            window_id: window_id.to_string(),
            writer_node_id,
            policy_revision_id: policy_revision.revision_id.clone(),
            manifest: manifest_ref.clone(),
            checkpoint_manifest,
            checkpoint_pointer,
            completed_at_ms,
            receipt_digest: stable_window_checkpoint_receipt_digest(
                window_id,
                policy_revision.revision_id.as_str(),
                manifest_ref.manifest_digest.as_str(),
                manifest_ref.object_digest.as_str(),
                completed_at_ms,
            ),
        };
        self.window_checkpoints.push(receipt.clone());
        Ok(receipt)
    }

    /// Returns the latest promoted checkpoint receipt for one window.
    pub fn latest_window_checkpoint(
        &self,
        window_id: &str,
    ) -> Result<&AdapterWindowCheckpointReceipt, AdapterArtifactStorageError> {
        self.window_checkpoints
            .iter()
            .rev()
            .find(|record| record.window_id == window_id)
            .ok_or_else(|| AdapterArtifactStorageError::UnknownWindowCheckpoint {
                window_id: window_id.to_string(),
            })
    }
}

fn stable_contribution_receipt_digest(
    upload_id: &str,
    window_id: &str,
    contribution_id: &str,
    worker_id: &str,
    writer_node_id: &str,
    manifest_digest: &str,
    object_digest: &str,
    completed_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_contribution_artifact",
        upload_id,
        window_id,
        contribution_id,
        worker_id,
        writer_node_id,
        manifest_digest,
        object_digest,
        completed_at_ms.to_string().as_str(),
    ])
}

fn stable_window_checkpoint_receipt_digest(
    window_id: &str,
    policy_revision_id: &str,
    manifest_digest: &str,
    object_digest: &str,
    completed_at_ms: u64,
) -> String {
    stable_digest([
        "adapter_window_checkpoint",
        window_id,
        policy_revision_id,
        manifest_digest,
        object_digest,
        completed_at_ms.to_string().as_str(),
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

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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
    use psionic_datastream::{DatastreamManifest, DatastreamSubjectKind};

    use super::{
        AdapterArtifactRetentionPolicy, AdapterArtifactStorageState,
        AdapterContributionArtifactDisposition,
    };
    use crate::{
        AdapterContributionUploadLocator, AdapterContributorCapabilityPolicy,
        AdapterDatasetSliceIdentity, AdapterTargetIdentity, AdapterTrainingClusterCoordinator,
        CheckpointPointer, CheckpointScopeBinding, CheckpointScopeKind, PolicyRevision,
    };

    const GIB_BYTES: u64 = 1024 * 1024 * 1024;

    fn cluster_state() -> psionic_cluster::ClusterState {
        let cluster_id = ClusterId::new(
            &ClusterNamespace::new("adapter-artifact-storage"),
            &AdmissionToken::new("shared-secret"),
        );
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = BTreeMap::from([(
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
                Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 33_100)),
                ClusterMembershipStatus::Ready,
            ),
        )]);
        snapshot.telemetry = BTreeMap::from([(
            NodeId::new("worker-b"),
            ClusterNodeTelemetry::new(NodeId::new("worker-b"))
                .with_memory(Some(24 * GIB_BYTES), Some(24 * GIB_BYTES))
                .with_accelerator_count(1)
                .with_backend_readiness(
                    AdapterContributorCapabilityPolicy::default().backend_label,
                    ClusterBackendReadinessStatus::Ready,
                )
                .with_stability_posture(ClusterStabilityPosture::Stable),
        )]);
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
            "worker-b",
            7,
            "cluster-digest-weather",
            "topology-digest-weather",
            started_at_ms,
        )
        .with_checkpoint_ref(checkpoint_ref)
        .with_step(70)
    }

    fn assignment() -> Result<crate::AdapterContributionWorkAssignment, Box<dyn std::error::Error>>
    {
        let state = cluster_state();
        let run = crate::TrainingRunState::new(
            "adapter-run-storage",
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
                CheckpointScopeBinding::new(CheckpointScopeKind::Window, "window-weather-4"),
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
            vec![AdapterDatasetSliceIdentity::new(
                "dataset.weather",
                "train",
                "slice-b",
                "slice-digest-b",
            )?],
            1,
            1_020,
        )?;
        Ok(crate::AdapterWorkerProtocolState::from_window_record(
            &record,
            crate::AdapterWorkerProtocolPolicy::default(),
        )
        .assignments[0]
            .clone())
    }

    #[test]
    fn contribution_upload_resume_and_complete_is_replay_safe()
    -> Result<(), Box<dyn std::error::Error>> {
        let assignment = assignment()?;
        let payload = b"adapter-delta-payload".repeat(4);
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            &payload,
            8,
            psionic_datastream::DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            assignment
                .upload_expectation
                .upload_reference_prefix
                .clone(),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let first_cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            &payload,
            8,
            "worker-b",
            1_030,
        )?;
        assert_eq!(first_cursor.next_chunk_index, 0);
        let second_cursor =
            storage.commit_next_chunk(first_cursor.upload_id.as_str(), &payload[..8])?;
        assert_eq!(second_cursor.next_chunk_index, 1);
        let resumed = storage.resume_upload(first_cursor.upload_id.as_str())?;
        assert_eq!(resumed.next_chunk_index, 1);
        for chunk in payload[8..].chunks(8) {
            storage.commit_next_chunk(first_cursor.upload_id.as_str(), chunk)?;
        }
        let receipt =
            storage.complete_contribution_upload(first_cursor.upload_id.as_str(), 1_040)?;
        assert_eq!(
            receipt.disposition,
            AdapterContributionArtifactDisposition::Reviewable
        );
        assert!(!receipt.artifact_id.is_empty());
        Ok(())
    }

    #[test]
    fn contribution_upload_rejects_corrupt_chunk_without_advancing_cursor()
    -> Result<(), Box<dyn std::error::Error>> {
        let assignment = assignment()?;
        let payload = b"adapter-delta-payload".repeat(2);
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            &payload,
            8,
            psionic_datastream::DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            assignment
                .upload_expectation
                .upload_reference_prefix
                .clone(),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage =
            AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy::default())?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            &payload,
            8,
            "worker-b",
            1_030,
        )?;
        let error = storage
            .commit_next_chunk(cursor.upload_id.as_str(), b"corrupt!")
            .expect_err("corrupt chunk should be refused");
        assert!(matches!(
            error,
            super::AdapterArtifactStorageError::UploadChunkDigestMismatch { .. }
                | super::AdapterArtifactStorageError::UploadChunkLengthMismatch { .. }
        ));
        let resumed = storage.resume_upload(cursor.upload_id.as_str())?;
        assert_eq!(resumed.next_chunk_index, 0);
        Ok(())
    }

    #[test]
    fn window_checkpoint_restore_and_retention_posture_are_tracked()
    -> Result<(), Box<dyn std::error::Error>> {
        let assignment = assignment()?;
        let payload = b"adapter-delta-payload".repeat(2);
        let manifest = DatastreamManifest::from_bytes(
            format!(
                "adapter-contribution:{}:{}",
                assignment.window_id, assignment.contribution_id
            ),
            DatastreamSubjectKind::AdapterPackage,
            &payload,
            8,
            psionic_datastream::DatastreamEncoding::RawBinary,
        )
        .with_provenance_digest(assignment.upload_expectation.expectation_digest.clone());
        let upload = AdapterContributionUploadLocator::new(
            assignment
                .upload_expectation
                .upload_reference_prefix
                .clone(),
            manifest.manifest_ref().manifest_digest.clone(),
            manifest.manifest_ref().total_bytes,
        )?;
        let mut storage = AdapterArtifactStorageState::new(AdapterArtifactRetentionPolicy {
            reviewable_retention_ms: Some(1_000),
            accepted_retention_ms: None,
            rejected_retention_ms: Some(10),
        })?;
        let cursor = storage.start_contribution_upload(
            &assignment,
            upload,
            &payload,
            8,
            "worker-b",
            1_030,
        )?;
        for chunk in payload.chunks(8) {
            storage.commit_next_chunk(cursor.upload_id.as_str(), chunk)?;
        }
        let receipt = storage.complete_contribution_upload(cursor.upload_id.as_str(), 1_040)?;
        storage.set_contribution_disposition(
            receipt.contribution_id.as_str(),
            AdapterContributionArtifactDisposition::Rejected,
            1_050,
        )?;
        let collected = storage.garbage_collect_expired_contributions(1_061);
        assert_eq!(collected, vec![receipt.artifact_id.clone()]);

        let checkpoint = storage.promote_window_checkpoint(
            assignment.window_id.as_str(),
            &assignment.source_policy_revision,
            b"promoted-adapter-checkpoint",
            8,
            "worker-b",
            1_070,
        )?;
        let restored = storage.latest_window_checkpoint(assignment.window_id.as_str())?;
        assert_eq!(restored.checkpoint_pointer, checkpoint.checkpoint_pointer);
        assert_eq!(restored.manifest, checkpoint.manifest);
        Ok(())
    }
}
