use std::collections::BTreeMap;

use psionic_datastream::DatastreamManifestRef;
use psionic_eval::EvalArtifact;
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::RolloutArtifact;

/// Error returned by the train artifact-storage layer.
#[derive(Debug, Error)]
pub enum TrainArtifactStorageError {
    /// One artifact class had no retention profile.
    #[error("train artifact storage is missing a retention profile for `{artifact_class:?}`")]
    MissingProfile {
        /// Artifact class without a profile.
        artifact_class: TrainArtifactClass,
    },
    /// One retention profile was structurally invalid.
    #[error(
        "retention profile for `{artifact_class:?}` is invalid: hot={hot_retention_ms}, warm={warm_retention_ms}"
    )]
    InvalidRetentionProfile {
        /// Artifact class with invalid settings.
        artifact_class: TrainArtifactClass,
        /// Configured hot-retention threshold.
        hot_retention_ms: u64,
        /// Configured warm-retention threshold.
        warm_retention_ms: u64,
    },
    /// The same artifact locator was registered twice.
    #[error("train artifact `{artifact_id}` is already registered")]
    DuplicateArtifact {
        /// Stable artifact identifier.
        artifact_id: String,
    },
    /// One artifact identifier was not present.
    #[error("unknown train artifact `{artifact_id}`")]
    UnknownArtifact {
        /// Stable artifact identifier.
        artifact_id: String,
    },
    /// The caller requested cold restore for an artifact that is not cold-archived.
    #[error(
        "train artifact `{artifact_id}` is not cold-archived and cannot be restored; tier `{tier:?}`, state `{state:?}`"
    )]
    ArtifactNotColdArchived {
        /// Stable artifact identifier.
        artifact_id: String,
        /// Current storage tier.
        tier: ArtifactStorageTier,
        /// Current lifecycle state.
        state: ArtifactLifecycleState,
    },
    /// The caller requested a mutation for an artifact already garbage collected.
    #[error("train artifact `{artifact_id}` has already been garbage collected")]
    ArtifactGarbageCollected {
        /// Stable artifact identifier.
        artifact_id: String,
    },
}

/// High-level train artifact family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainArtifactClass {
    /// Checkpoint or optimizer-state artifact.
    Checkpoint,
    /// Adapter contribution artifact staged for one decentralized window.
    AdapterContribution,
    /// Window-scoped promoted adapter checkpoint.
    AdapterWindowCheckpoint,
    /// Rollout artifact.
    Rollout,
    /// Eval artifact or bundle.
    EvalArtifact,
    /// Log or trace bundle.
    LogBundle,
}

/// Storage tier used for one train artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStorageTier {
    /// Low-latency active storage.
    Hot,
    /// Lower-cost nearline storage.
    Warm,
    /// Cold archive with explicit restore objectives.
    ColdArchive,
}

/// Lifecycle state for one train artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLifecycleState {
    /// Artifact is active in its current tier.
    Active,
    /// Artifact has a pending cold-restore request.
    RestoreRequested,
    /// Artifact has been garbage collected.
    GarbageCollected,
}

/// Archival class admitted by one retention profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactArchiveClass {
    /// Artifact is short-lived and may skip cold archive.
    Ephemeral,
    /// Artifact should be cold-archived and remain restorable.
    Restorable,
    /// Artifact should be cold-archived and retained immutably.
    Immutable,
}

/// Retention profile for one train artifact class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRetentionProfile {
    /// Time an artifact may remain in hot storage before nearline migration.
    pub hot_retention_ms: u64,
    /// Time an artifact may remain warm before archival or deletion.
    pub warm_retention_ms: u64,
    /// Archival class for the artifact family.
    pub archive_class: ArtifactArchiveClass,
    /// Optional deletion threshold after creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_after_ms: Option<u64>,
    /// Whether artifacts of the same class and digest should be deduplicated.
    pub deduplicate_by_digest: bool,
    /// Cold-restore objective for archived artifacts.
    pub cold_restore_sla_ms: u64,
}

impl ArtifactRetentionProfile {
    /// Creates a retention profile with explicit hot and warm thresholds.
    #[must_use]
    pub const fn new(
        hot_retention_ms: u64,
        warm_retention_ms: u64,
        archive_class: ArtifactArchiveClass,
        cold_restore_sla_ms: u64,
    ) -> Self {
        Self {
            hot_retention_ms,
            warm_retention_ms,
            archive_class,
            delete_after_ms: None,
            deduplicate_by_digest: false,
            cold_restore_sla_ms,
        }
    }

    /// Attaches an explicit deletion threshold.
    #[must_use]
    pub const fn with_delete_after_ms(mut self, delete_after_ms: Option<u64>) -> Self {
        self.delete_after_ms = delete_after_ms;
        self
    }

    /// Enables or disables digest deduplication.
    #[must_use]
    pub const fn with_deduplication(mut self, deduplicate_by_digest: bool) -> Self {
        self.deduplicate_by_digest = deduplicate_by_digest;
        self
    }
}

/// Stable locator for one train artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrainArtifactLocator {
    /// Checkpoint artifact backed by a datastream manifest and checkpoint reference.
    Checkpoint {
        /// Compact datastream manifest reference.
        manifest: DatastreamManifestRef,
        /// Runtime-visible checkpoint identity.
        checkpoint: TrainingCheckpointReference,
    },
    /// Adapter contribution artifact backed by a datastream manifest.
    AdapterContribution {
        /// Compact datastream manifest reference.
        manifest: DatastreamManifestRef,
        /// Stable window identifier.
        window_id: String,
        /// Stable contribution identifier.
        contribution_id: String,
        /// Stable writer node identifier.
        writer_node_id: String,
    },
    /// Window-scoped promoted checkpoint backed by a datastream manifest.
    AdapterWindowCheckpoint {
        /// Compact datastream manifest reference.
        manifest: DatastreamManifestRef,
        /// Runtime-visible checkpoint identity.
        checkpoint: TrainingCheckpointReference,
        /// Stable window identifier.
        window_id: String,
        /// Stable policy revision identifier.
        policy_revision_id: String,
    },
    /// Rollout artifact generated by one worker.
    Rollout {
        /// Stable rollout artifact identifier.
        artifact_id: String,
        /// Stable worker identifier.
        worker_id: String,
    },
    /// Eval artifact surfaced by the eval runtime.
    EvalArtifact {
        /// Stable eval artifact kind.
        artifact_kind: String,
        /// Stable eval artifact reference.
        artifact_ref: String,
    },
    /// Log or trace bundle.
    LogBundle {
        /// Stable log reference.
        log_ref: String,
    },
}

impl TrainArtifactLocator {
    /// Returns the canonical artifact identifier for storage control.
    #[must_use]
    pub fn artifact_id(&self) -> String {
        match self {
            Self::Checkpoint { manifest, .. } => checkpoint_artifact_id(manifest),
            Self::AdapterContribution {
                manifest,
                window_id,
                contribution_id,
                ..
            } => format!(
                "adapter_contribution:{window_id}:{contribution_id}:{}",
                manifest.manifest_digest
            ),
            Self::AdapterWindowCheckpoint {
                manifest,
                window_id,
                policy_revision_id,
                ..
            } => format!(
                "adapter_window_checkpoint:{window_id}:{policy_revision_id}:{}",
                manifest.manifest_digest
            ),
            Self::Rollout { artifact_id, .. } => format!("rollout:{artifact_id}"),
            Self::EvalArtifact {
                artifact_kind,
                artifact_ref,
            } => format!("eval:{artifact_kind}:{artifact_ref}"),
            Self::LogBundle { log_ref } => format!("log:{log_ref}"),
        }
    }
}

/// One tracked train artifact and its lifecycle truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainArtifactRecord {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Artifact family.
    pub artifact_class: TrainArtifactClass,
    /// Stable locator for the artifact.
    pub locator: TrainArtifactLocator,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Artifact size in bytes.
    pub byte_length: u64,
    /// Creation time.
    pub created_at_ms: u64,
    /// Last access time.
    pub last_accessed_at_ms: u64,
    /// Current storage tier.
    pub tier: ArtifactStorageTier,
    /// Current lifecycle state.
    pub state: ArtifactLifecycleState,
    /// Derived archive locator when the artifact is cold archived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_locator: Option<String>,
}

/// Reason-code family for one storage lifecycle transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStorageTransitionReason {
    /// Hot retention expired and the artifact moved to warm storage.
    HotRetentionExpired,
    /// Warm retention expired and the artifact moved to cold archive.
    WarmRetentionExpired,
    /// Deduplication removed this artifact in favor of an earlier copy.
    DigestDeduplicated,
    /// Delete-after threshold expired.
    DeleteAfterExpired,
    /// Cold restore was requested.
    ColdRestoreRequested,
    /// Cold restore completed and the artifact returned to warm storage.
    ColdRestoreCompleted,
}

/// One explicit storage lifecycle transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactStorageTransition {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Previous tier.
    pub from_tier: ArtifactStorageTier,
    /// Next tier.
    pub to_tier: ArtifactStorageTier,
    /// Previous lifecycle state.
    pub from_state: ArtifactLifecycleState,
    /// Next lifecycle state.
    pub to_state: ArtifactLifecycleState,
    /// Transition reason.
    pub reason: ArtifactStorageTransitionReason,
}

/// Sweep receipt covering one retention or deduplication pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactStorageSweepReceipt {
    /// Stable sweep identifier.
    pub sweep_id: String,
    /// Sweep time.
    pub observed_at_ms: u64,
    /// Emitted transitions.
    pub transitions: Vec<ArtifactStorageTransition>,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Cold-restore action family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactColdRestoreAction {
    /// Cold restore was requested and is now pending.
    Requested,
    /// Cold restore completed and the artifact returned to warm storage.
    Completed,
}

/// Cold-restore receipt for one archived artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactColdRestoreReceipt {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Action performed.
    pub action: ArtifactColdRestoreAction,
    /// Archive locator used for restore.
    pub archive_locator: String,
    /// Request or completion time.
    pub observed_at_ms: u64,
    /// Target ready time under the storage objective.
    pub target_ready_at_ms: u64,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Stateful train artifact-storage controller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainArtifactStorageController {
    /// Retention profiles keyed by artifact class.
    pub profiles: BTreeMap<TrainArtifactClass, ArtifactRetentionProfile>,
    /// Tracked artifact records.
    pub artifacts: Vec<TrainArtifactRecord>,
}

impl TrainArtifactStorageController {
    /// Creates a validated artifact-storage controller.
    pub fn new(
        profiles: BTreeMap<TrainArtifactClass, ArtifactRetentionProfile>,
    ) -> Result<Self, TrainArtifactStorageError> {
        for (artifact_class, profile) in &profiles {
            if profile.warm_retention_ms < profile.hot_retention_ms {
                return Err(TrainArtifactStorageError::InvalidRetentionProfile {
                    artifact_class: *artifact_class,
                    hot_retention_ms: profile.hot_retention_ms,
                    warm_retention_ms: profile.warm_retention_ms,
                });
            }
        }
        Ok(Self {
            profiles,
            artifacts: Vec::new(),
        })
    }

    /// Registers one checkpoint artifact.
    pub fn register_checkpoint(
        &mut self,
        manifest: DatastreamManifestRef,
        checkpoint: TrainingCheckpointReference,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        let artifact_digest = manifest.object_digest.clone();
        let artifact_id = self.register_artifact(
            TrainArtifactClass::Checkpoint,
            TrainArtifactLocator::Checkpoint {
                manifest,
                checkpoint,
            },
            byte_length,
            created_at_ms,
        )?;
        if let Some(record) = self.artifact_mut(artifact_id.as_str()) {
            record.artifact_digest = artifact_digest;
        }
        Ok(artifact_id)
    }

    /// Registers one adapter contribution artifact.
    pub fn register_adapter_contribution(
        &mut self,
        manifest: DatastreamManifestRef,
        window_id: impl Into<String>,
        contribution_id: impl Into<String>,
        writer_node_id: impl Into<String>,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        let artifact_digest = manifest.object_digest.clone();
        let artifact_id = self.register_artifact(
            TrainArtifactClass::AdapterContribution,
            TrainArtifactLocator::AdapterContribution {
                manifest,
                window_id: window_id.into(),
                contribution_id: contribution_id.into(),
                writer_node_id: writer_node_id.into(),
            },
            byte_length,
            created_at_ms,
        )?;
        if let Some(record) = self.artifact_mut(artifact_id.as_str()) {
            record.artifact_digest = artifact_digest;
        }
        Ok(artifact_id)
    }

    /// Registers one window-scoped promoted adapter checkpoint artifact.
    pub fn register_adapter_window_checkpoint(
        &mut self,
        manifest: DatastreamManifestRef,
        checkpoint: TrainingCheckpointReference,
        window_id: impl Into<String>,
        policy_revision_id: impl Into<String>,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        let artifact_digest = manifest.object_digest.clone();
        let artifact_id = self.register_artifact(
            TrainArtifactClass::AdapterWindowCheckpoint,
            TrainArtifactLocator::AdapterWindowCheckpoint {
                manifest,
                checkpoint,
                window_id: window_id.into(),
                policy_revision_id: policy_revision_id.into(),
            },
            byte_length,
            created_at_ms,
        )?;
        if let Some(record) = self.artifact_mut(artifact_id.as_str()) {
            record.artifact_digest = artifact_digest;
        }
        Ok(artifact_id)
    }

    /// Registers one rollout artifact.
    pub fn register_rollout(
        &mut self,
        artifact: &RolloutArtifact,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        self.register_artifact(
            TrainArtifactClass::Rollout,
            TrainArtifactLocator::Rollout {
                artifact_id: artifact.artifact_id.clone(),
                worker_id: artifact.worker_id.clone(),
            },
            byte_length,
            created_at_ms,
        )
        .map(|artifact_id| {
            if let Some(record) = self.artifact_mut(artifact_id.as_str()) {
                record.artifact_digest = artifact.artifact_digest.clone();
            }
            artifact_id
        })
    }

    /// Registers one eval artifact.
    pub fn register_eval_artifact(
        &mut self,
        artifact: &EvalArtifact,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        self.register_artifact(
            TrainArtifactClass::EvalArtifact,
            TrainArtifactLocator::EvalArtifact {
                artifact_kind: artifact.artifact_kind.clone(),
                artifact_ref: artifact.artifact_ref.clone(),
            },
            byte_length,
            created_at_ms,
        )
        .map(|artifact_id| {
            if let Some(record) = self.artifact_mut(artifact_id.as_str()) {
                record.artifact_digest = artifact.artifact_digest.clone();
            }
            artifact_id
        })
    }

    /// Registers one log or trace bundle.
    pub fn register_log_bundle(
        &mut self,
        log_ref: impl Into<String>,
        artifact_digest: impl Into<String>,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        let artifact_id = self.register_artifact(
            TrainArtifactClass::LogBundle,
            TrainArtifactLocator::LogBundle {
                log_ref: log_ref.into(),
            },
            byte_length,
            created_at_ms,
        )?;
        if let Some(record) = self.artifact_mut(artifact_id.as_str()) {
            record.artifact_digest = artifact_digest.into();
        }
        Ok(artifact_id)
    }

    /// Returns one tracked artifact when present.
    #[must_use]
    pub fn artifact(&self, artifact_id: &str) -> Option<&TrainArtifactRecord> {
        self.artifacts
            .iter()
            .find(|record| record.artifact_id == artifact_id)
    }

    /// Records one access time for the artifact.
    pub fn record_access(
        &mut self,
        artifact_id: &str,
        accessed_at_ms: u64,
    ) -> Result<(), TrainArtifactStorageError> {
        let record = self.artifact_mut(artifact_id).ok_or_else(|| {
            TrainArtifactStorageError::UnknownArtifact {
                artifact_id: String::from(artifact_id),
            }
        })?;
        if record.state == ArtifactLifecycleState::GarbageCollected {
            return Err(TrainArtifactStorageError::ArtifactGarbageCollected {
                artifact_id: String::from(artifact_id),
            });
        }
        record.last_accessed_at_ms = accessed_at_ms;
        Ok(())
    }

    /// Runs one retention, archive, and deduplication sweep.
    pub fn sweep(
        &mut self,
        observed_at_ms: u64,
    ) -> Result<ArtifactStorageSweepReceipt, TrainArtifactStorageError> {
        let mut transitions = Vec::new();
        let mut canonical_by_digest = BTreeMap::<(TrainArtifactClass, String), String>::new();

        for record in &mut self.artifacts {
            if record.state == ArtifactLifecycleState::GarbageCollected {
                continue;
            }
            let profile = self.profiles.get(&record.artifact_class).ok_or_else(|| {
                TrainArtifactStorageError::MissingProfile {
                    artifact_class: record.artifact_class,
                }
            })?;

            let canonical_key = (record.artifact_class, record.artifact_digest.clone());
            if profile.deduplicate_by_digest && !record.artifact_digest.is_empty() {
                if let Some(canonical_artifact_id) = canonical_by_digest.get(&canonical_key) {
                    if canonical_artifact_id != &record.artifact_id {
                        transitions.push(apply_transition(
                            record,
                            record.tier,
                            ArtifactLifecycleState::GarbageCollected,
                            ArtifactStorageTransitionReason::DigestDeduplicated,
                        ));
                        continue;
                    }
                }
            }
            if !record.artifact_digest.is_empty() {
                canonical_by_digest
                    .entry(canonical_key)
                    .or_insert_with(|| record.artifact_id.clone());
            }

            let age_ms = observed_at_ms.saturating_sub(record.created_at_ms);
            if profile.archive_class != ArtifactArchiveClass::Immutable
                && profile
                    .delete_after_ms
                    .is_some_and(|delete_after_ms| age_ms > delete_after_ms)
            {
                transitions.push(apply_transition(
                    record,
                    record.tier,
                    ArtifactLifecycleState::GarbageCollected,
                    ArtifactStorageTransitionReason::DeleteAfterExpired,
                ));
                continue;
            }

            if age_ms > profile.warm_retention_ms {
                match profile.archive_class {
                    ArtifactArchiveClass::Ephemeral => {
                        transitions.push(apply_transition(
                            record,
                            record.tier,
                            ArtifactLifecycleState::GarbageCollected,
                            ArtifactStorageTransitionReason::DeleteAfterExpired,
                        ));
                    }
                    ArtifactArchiveClass::Restorable | ArtifactArchiveClass::Immutable
                        if record.tier != ArtifactStorageTier::ColdArchive =>
                    {
                        record.archive_locator = Some(archive_locator(record));
                        transitions.push(apply_transition(
                            record,
                            ArtifactStorageTier::ColdArchive,
                            ArtifactLifecycleState::Active,
                            ArtifactStorageTransitionReason::WarmRetentionExpired,
                        ));
                    }
                    ArtifactArchiveClass::Restorable | ArtifactArchiveClass::Immutable => {}
                }
                continue;
            }

            if age_ms > profile.hot_retention_ms && record.tier == ArtifactStorageTier::Hot {
                transitions.push(apply_transition(
                    record,
                    ArtifactStorageTier::Warm,
                    ArtifactLifecycleState::Active,
                    ArtifactStorageTransitionReason::HotRetentionExpired,
                ));
            }
        }

        let sweep_id = format!("artifact-sweep-{}", observed_at_ms);
        let receipt_digest =
            stable_sweep_receipt_digest(sweep_id.as_str(), observed_at_ms, transitions.as_slice());
        Ok(ArtifactStorageSweepReceipt {
            sweep_id,
            observed_at_ms,
            transitions,
            receipt_digest,
        })
    }

    /// Requests cold restore for one archived artifact.
    pub fn request_cold_restore(
        &mut self,
        artifact_id: &str,
        observed_at_ms: u64,
    ) -> Result<ArtifactColdRestoreReceipt, TrainArtifactStorageError> {
        let artifact_class = self
            .artifact(artifact_id)
            .ok_or_else(|| TrainArtifactStorageError::UnknownArtifact {
                artifact_id: String::from(artifact_id),
            })?
            .artifact_class;
        let cold_restore_sla_ms = self
            .profiles
            .get(&artifact_class)
            .ok_or_else(|| TrainArtifactStorageError::MissingProfile { artifact_class })?
            .cold_restore_sla_ms;
        let record = self.artifact_mut(artifact_id).ok_or_else(|| {
            TrainArtifactStorageError::UnknownArtifact {
                artifact_id: String::from(artifact_id),
            }
        })?;
        if record.state == ArtifactLifecycleState::GarbageCollected {
            return Err(TrainArtifactStorageError::ArtifactGarbageCollected {
                artifact_id: String::from(artifact_id),
            });
        }
        if record.tier != ArtifactStorageTier::ColdArchive {
            return Err(TrainArtifactStorageError::ArtifactNotColdArchived {
                artifact_id: String::from(artifact_id),
                tier: record.tier,
                state: record.state,
            });
        }
        record.state = ArtifactLifecycleState::RestoreRequested;
        record.last_accessed_at_ms = observed_at_ms;
        let archive_locator = record
            .archive_locator
            .clone()
            .unwrap_or_else(|| archive_locator(record));
        let target_ready_at_ms = observed_at_ms.saturating_add(cold_restore_sla_ms);
        Ok(ArtifactColdRestoreReceipt {
            artifact_id: String::from(artifact_id),
            action: ArtifactColdRestoreAction::Requested,
            archive_locator: archive_locator.clone(),
            observed_at_ms,
            target_ready_at_ms,
            receipt_digest: stable_cold_restore_receipt_digest(
                artifact_id,
                ArtifactColdRestoreAction::Requested,
                archive_locator.as_str(),
                observed_at_ms,
                target_ready_at_ms,
            ),
        })
    }

    /// Completes cold restore for one previously archived artifact.
    pub fn complete_cold_restore(
        &mut self,
        artifact_id: &str,
        observed_at_ms: u64,
    ) -> Result<ArtifactColdRestoreReceipt, TrainArtifactStorageError> {
        let artifact_class = self
            .artifact(artifact_id)
            .ok_or_else(|| TrainArtifactStorageError::UnknownArtifact {
                artifact_id: String::from(artifact_id),
            })?
            .artifact_class;
        let cold_restore_sla_ms = self
            .profiles
            .get(&artifact_class)
            .ok_or_else(|| TrainArtifactStorageError::MissingProfile { artifact_class })?
            .cold_restore_sla_ms;
        let record = self.artifact_mut(artifact_id).ok_or_else(|| {
            TrainArtifactStorageError::UnknownArtifact {
                artifact_id: String::from(artifact_id),
            }
        })?;
        if record.state == ArtifactLifecycleState::GarbageCollected {
            return Err(TrainArtifactStorageError::ArtifactGarbageCollected {
                artifact_id: String::from(artifact_id),
            });
        }
        if record.tier != ArtifactStorageTier::ColdArchive {
            return Err(TrainArtifactStorageError::ArtifactNotColdArchived {
                artifact_id: String::from(artifact_id),
                tier: record.tier,
                state: record.state,
            });
        }
        record.tier = ArtifactStorageTier::Warm;
        record.state = ArtifactLifecycleState::Active;
        record.last_accessed_at_ms = observed_at_ms;
        let archive_locator = record
            .archive_locator
            .clone()
            .unwrap_or_else(|| archive_locator(record));
        let target_ready_at_ms = observed_at_ms.saturating_add(cold_restore_sla_ms);
        Ok(ArtifactColdRestoreReceipt {
            artifact_id: String::from(artifact_id),
            action: ArtifactColdRestoreAction::Completed,
            archive_locator: archive_locator.clone(),
            observed_at_ms,
            target_ready_at_ms,
            receipt_digest: stable_cold_restore_receipt_digest(
                artifact_id,
                ArtifactColdRestoreAction::Completed,
                archive_locator.as_str(),
                observed_at_ms,
                target_ready_at_ms,
            ),
        })
    }

    fn register_artifact(
        &mut self,
        artifact_class: TrainArtifactClass,
        locator: TrainArtifactLocator,
        byte_length: u64,
        created_at_ms: u64,
    ) -> Result<String, TrainArtifactStorageError> {
        if !self.profiles.contains_key(&artifact_class) {
            return Err(TrainArtifactStorageError::MissingProfile { artifact_class });
        }
        let artifact_id = locator.artifact_id();
        if self
            .artifacts
            .iter()
            .any(|record| record.artifact_id == artifact_id)
        {
            return Err(TrainArtifactStorageError::DuplicateArtifact { artifact_id });
        }
        let artifact_id = locator.artifact_id();
        self.artifacts.push(TrainArtifactRecord {
            artifact_id: artifact_id.clone(),
            artifact_class,
            locator,
            artifact_digest: String::new(),
            byte_length,
            created_at_ms,
            last_accessed_at_ms: created_at_ms,
            tier: ArtifactStorageTier::Hot,
            state: ArtifactLifecycleState::Active,
            archive_locator: None,
        });
        Ok(artifact_id)
    }

    fn artifact_mut(&mut self, artifact_id: &str) -> Option<&mut TrainArtifactRecord> {
        self.artifacts
            .iter_mut()
            .find(|record| record.artifact_id == artifact_id)
    }
}

fn apply_transition(
    record: &mut TrainArtifactRecord,
    to_tier: ArtifactStorageTier,
    to_state: ArtifactLifecycleState,
    reason: ArtifactStorageTransitionReason,
) -> ArtifactStorageTransition {
    let transition = ArtifactStorageTransition {
        artifact_id: record.artifact_id.clone(),
        from_tier: record.tier,
        to_tier,
        from_state: record.state,
        to_state,
        reason,
    };
    record.tier = to_tier;
    record.state = to_state;
    transition
}

fn checkpoint_artifact_id(manifest: &DatastreamManifestRef) -> String {
    format!(
        "checkpoint:{}:{}",
        manifest.stream_id, manifest.manifest_digest
    )
}

fn archive_locator(record: &TrainArtifactRecord) -> String {
    format!(
        "archive://{}/{}",
        artifact_class_label(record.artifact_class),
        record.artifact_digest
    )
}

fn artifact_class_label(artifact_class: TrainArtifactClass) -> &'static str {
    match artifact_class {
        TrainArtifactClass::Checkpoint => "checkpoint",
        TrainArtifactClass::AdapterContribution => "adapter_contribution",
        TrainArtifactClass::AdapterWindowCheckpoint => "adapter_window_checkpoint",
        TrainArtifactClass::Rollout => "rollout",
        TrainArtifactClass::EvalArtifact => "eval_artifact",
        TrainArtifactClass::LogBundle => "log_bundle",
    }
}

fn stable_sweep_receipt_digest(
    sweep_id: &str,
    observed_at_ms: u64,
    transitions: &[ArtifactStorageTransition],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_artifact_storage_sweep|");
    hasher.update(sweep_id.as_bytes());
    hasher.update(b"|");
    hasher.update(observed_at_ms.to_string().as_bytes());
    for transition in transitions {
        hasher.update(stable_json_bytes(transition));
    }
    hex::encode(hasher.finalize())
}

fn stable_cold_restore_receipt_digest(
    artifact_id: &str,
    action: ArtifactColdRestoreAction,
    archive_locator: &str,
    observed_at_ms: u64,
    target_ready_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_cold_restore_receipt|");
    hasher.update(artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(match action {
        ArtifactColdRestoreAction::Requested => b"requested".as_slice(),
        ArtifactColdRestoreAction::Completed => b"completed".as_slice(),
    });
    hasher.update(b"|");
    hasher.update(archive_locator.as_bytes());
    hasher.update(b"|");
    hasher.update(observed_at_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(target_ready_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_json_bytes(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("stable JSON serialization failed")
}

#[cfg(test)]
mod tests {
    use psionic_datastream::{
        DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind,
    };

    use crate::{PolicyRevision, RolloutSample, RolloutTerminationReason};

    use super::*;

    #[test]
    fn artifact_storage_sweep_moves_checkpoint_through_warm_archive_and_gc()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = TrainArtifactStorageController::new(BTreeMap::from([(
            TrainArtifactClass::Checkpoint,
            ArtifactRetentionProfile::new(1_000, 5_000, ArtifactArchiveClass::Restorable, 2_000)
                .with_delete_after_ms(Some(10_000)),
        )]))?;
        let checkpoint_id = controller.register_checkpoint(
            checkpoint_manifest(),
            checkpoint_reference(),
            4_096,
            0,
        )?;

        let warm = controller.sweep(1_500)?;
        assert_eq!(warm.transitions.len(), 1);
        assert_eq!(warm.transitions[0].to_tier, ArtifactStorageTier::Warm);

        let archived = controller.sweep(6_000)?;
        assert_eq!(archived.transitions.len(), 1);
        assert_eq!(
            archived.transitions[0].to_tier,
            ArtifactStorageTier::ColdArchive
        );

        let deleted = controller.sweep(11_000)?;
        assert_eq!(deleted.transitions.len(), 1);
        assert_eq!(
            controller
                .artifact(checkpoint_id.as_str())
                .expect("checkpoint should still exist as record")
                .state,
            ArtifactLifecycleState::GarbageCollected
        );
        Ok(())
    }

    #[test]
    fn artifact_storage_controller_deduplicates_rollout_artifacts_by_digest()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = TrainArtifactStorageController::new(BTreeMap::from([(
            TrainArtifactClass::Rollout,
            ArtifactRetentionProfile::new(1_000, 5_000, ArtifactArchiveClass::Ephemeral, 1_000)
                .with_deduplication(true),
        )]))?;
        let environment =
            psionic_environments::EnvironmentPackageKey::new("weather.agent", "1.0.0");
        let rollout_a = sample_rollout("rollout-a", "worker-a", &environment);
        let rollout_b = sample_rollout("rollout-b", "worker-b", &environment);
        let id_a = controller.register_rollout(&rollout_a, 512, 0)?;
        let id_b = controller.register_rollout(&rollout_b, 512, 100)?;
        controller
            .artifact_mut(id_b.as_str())
            .expect("rollout-b")
            .artifact_digest = rollout_a.artifact_digest.clone();

        let receipt = controller.sweep(200)?;
        assert_eq!(receipt.transitions.len(), 1);
        assert_eq!(
            controller.artifact(id_a.as_str()).expect("rollout-a").state,
            ArtifactLifecycleState::Active
        );
        assert_eq!(
            controller.artifact(id_b.as_str()).expect("rollout-b").state,
            ArtifactLifecycleState::GarbageCollected
        );
        Ok(())
    }

    #[test]
    fn artifact_storage_controller_plans_and_completes_cold_restore()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut controller = TrainArtifactStorageController::new(BTreeMap::from([(
            TrainArtifactClass::EvalArtifact,
            ArtifactRetentionProfile::new(1_000, 3_000, ArtifactArchiveClass::Restorable, 2_500),
        )]))?;
        let eval_artifact = EvalArtifact::new("trace", "artifact://eval/1", b"eval-bytes");
        let artifact_id = controller.register_eval_artifact(&eval_artifact, 256, 0)?;
        controller.sweep(4_000)?;

        let requested = controller.request_cold_restore(artifact_id.as_str(), 5_000)?;
        assert_eq!(requested.action, ArtifactColdRestoreAction::Requested);
        assert_eq!(requested.target_ready_at_ms, 7_500);

        let completed = controller.complete_cold_restore(artifact_id.as_str(), 7_500)?;
        assert_eq!(completed.action, ArtifactColdRestoreAction::Completed);
        assert_eq!(
            controller
                .artifact(artifact_id.as_str())
                .expect("eval artifact")
                .tier,
            ArtifactStorageTier::Warm
        );
        Ok(())
    }

    fn checkpoint_manifest() -> DatastreamManifestRef {
        DatastreamManifest::from_bytes(
            "checkpoint-stream",
            DatastreamSubjectKind::Checkpoint,
            &[7_u8; 4_096],
            1_024,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(DatastreamCheckpointBinding::new("train.weather").with_step(7))
        .manifest_ref()
    }

    fn checkpoint_reference() -> TrainingCheckpointReference {
        let manifest = checkpoint_manifest();
        TrainingCheckpointReference::new(
            "train.weather",
            manifest.stream_id.clone(),
            manifest.manifest_digest.clone(),
            manifest.object_digest.clone(),
            "node-a",
            3,
            "cluster-state-digest",
            "topology-digest",
            0,
        )
        .with_checkpoint_ref("checkpoint://weather/7")
        .with_step(7)
    }

    fn sample_rollout(
        artifact_id: &str,
        worker_id: &str,
        environment: &psionic_environments::EnvironmentPackageKey,
    ) -> RolloutArtifact {
        RolloutArtifact::new(
            artifact_id,
            worker_id,
            environment.clone(),
            "task-a",
            PolicyRevision::new("weather.policy", "rev-1", "policy-digest", 1_000)
                .with_revision_number(1),
            vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
            RolloutTerminationReason::Completed,
            Vec::new(),
            2_000,
        )
        .expect("rollout artifact should be valid")
    }
}
