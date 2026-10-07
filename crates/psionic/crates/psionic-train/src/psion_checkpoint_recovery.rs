use std::collections::{BTreeMap, BTreeSet};

use psionic_datastream::DatastreamManifestRef;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CheckpointManifest, CheckpointPointer, CheckpointReadSourceKind, CheckpointRecoveryError,
    CheckpointScopeKind, CheckpointShardManifest, PsionPretrainRunObservabilityReceipt,
    PsionPretrainRunScaleProfile, PsionPretrainStageRunReceipt, TrainingInstabilityTelemetry,
    TrainingOperationalAction, TrainingRecoveryMode, TrainingStabilityVerdict,
};

/// Stable schema version for one Psion checkpoint artifact receipt.
pub const PSION_CHECKPOINT_ARTIFACT_SCHEMA_VERSION: &str = "psion.checkpoint_artifact.v1";
/// Stable schema version for one Psion checkpoint corruption receipt.
pub const PSION_CHECKPOINT_CORRUPTION_RECEIPT_SCHEMA_VERSION: &str =
    "psion.checkpoint_corruption_receipt.v1";
/// Stable schema version for one Psion checkpoint recovery event receipt.
pub const PSION_CHECKPOINT_RECOVERY_EVENT_SCHEMA_VERSION: &str =
    "psion.checkpoint_recovery_event.v1";
/// Stable schema version for the first Psion checkpoint recovery bundle.
pub const PSION_CHECKPOINT_RECOVERY_BUNDLE_SCHEMA_VERSION: &str =
    "psion.checkpoint_recovery_bundle.v1";

/// Checkpoint layout frozen by the Psion recovery contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCheckpointLayoutKind {
    /// Single-object dense checkpoint and optimizer state.
    Dense,
    /// Multi-shard checkpoint and optimizer state for distributed recovery.
    Sharded,
}

impl PsionCheckpointLayoutKind {
    #[must_use]
    pub const fn required_layouts() -> [Self; 2] {
        [Self::Dense, Self::Sharded]
    }
}

/// Recovery event class frozen by the first Psion checkpoint bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCheckpointRecoveryEventKind {
    /// Resume after one bounded forced interruption.
    ForcedInterruptionRestart,
    /// Resume on a multi-worker recovery topology.
    DistributedRestart,
    /// Detect corruption and roll back to the last stable checkpoint artifact.
    CorruptionDetectedRollback,
    /// Detect corruption and invalidate the run instead of continuing.
    CorruptionDetectedInvalidation,
}

impl PsionCheckpointRecoveryEventKind {
    #[must_use]
    pub const fn required_kinds() -> [Self; 4] {
        [
            Self::ForcedInterruptionRestart,
            Self::DistributedRestart,
            Self::CorruptionDetectedRollback,
            Self::CorruptionDetectedInvalidation,
        ]
    }
}

/// Final disposition for one recovery event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCheckpointRecoveryDisposition {
    /// Restart resumed successfully.
    Resumed,
    /// Recovery rolled back to the last stable checkpoint artifact.
    RolledBackToStableCheckpoint,
    /// Recovery invalidated the run rather than continuing.
    Invalidated,
}

/// Corruption class frozen by the first Psion recovery bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCheckpointCorruptionKind {
    /// The checkpoint manifest or pointer digest no longer matched.
    ManifestDigestMismatch,
    /// One or more checkpoint shards were missing during recovery.
    MissingShard,
    /// Optimizer-state shards no longer matched the checkpoint state.
    OptimizerStateMismatch,
}

/// Dataset, sampling, and topology context preserved on one checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointContextReceipt {
    /// Training run profile bound to the source artifact.
    pub training_run_profile: PsionPretrainRunScaleProfile,
    /// Stable dataset identity.
    pub dataset_identity: String,
    /// Stable sampling-policy identifier.
    pub sampling_policy_id: String,
    /// Stable sampling-policy version.
    pub sampling_policy_version: String,
    /// Stable topology digest carried by the source checkpoint lineage.
    pub source_checkpoint_topology_digest: String,
    /// Stable hardware-topology digest carried by the run observability receipt.
    pub training_hardware_topology_digest: String,
    /// Worker count observed by the source run.
    pub observed_worker_count: u16,
    /// Short summary of the bound context.
    pub detail: String,
}

impl PsionCheckpointContextReceipt {
    fn validate_against_inputs(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
        observability_receipt: &PsionPretrainRunObservabilityReceipt,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        if self.training_run_profile != observability_receipt.run_profile {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryContextMismatch {
                    field: String::from("checkpoint_context.training_run_profile"),
                    expected: format!("{:?}", observability_receipt.run_profile),
                    actual: format!("{:?}", self.training_run_profile),
                },
            );
        }
        check_string_match(
            self.dataset_identity.as_str(),
            stage_receipt.dataset_identity.as_str(),
            "checkpoint_context.dataset_identity",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            observability_receipt.dataset_identity.as_str(),
            "checkpoint_context.dataset_identity",
        )?;
        check_string_match(
            self.sampling_policy_id.as_str(),
            stage_receipt.sampling_policy_id.as_str(),
            "checkpoint_context.sampling_policy_id",
        )?;
        check_string_match(
            self.sampling_policy_id.as_str(),
            observability_receipt.sampling_policy_id.as_str(),
            "checkpoint_context.sampling_policy_id",
        )?;
        check_string_match(
            self.sampling_policy_version.as_str(),
            stage_receipt.sampling_policy_version.as_str(),
            "checkpoint_context.sampling_policy_version",
        )?;
        check_string_match(
            self.sampling_policy_version.as_str(),
            observability_receipt.sampling_policy_version.as_str(),
            "checkpoint_context.sampling_policy_version",
        )?;
        check_string_match(
            self.source_checkpoint_topology_digest.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .topology_digest
                .as_str(),
            "checkpoint_context.source_checkpoint_topology_digest",
        )?;
        check_string_match(
            self.training_hardware_topology_digest.as_str(),
            observability_receipt
                .hardware_topology
                .topology_digest
                .as_str(),
            "checkpoint_context.training_hardware_topology_digest",
        )?;
        if self.observed_worker_count
            != observability_receipt
                .hardware_topology
                .observed_worker_count
        {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryContextMismatch {
                    field: String::from("checkpoint_context.observed_worker_count"),
                    expected: observability_receipt
                        .hardware_topology
                        .observed_worker_count
                        .to_string(),
                    actual: self.observed_worker_count.to_string(),
                },
            );
        }
        ensure_nonempty(self.detail.as_str(), "checkpoint_context.detail")?;
        Ok(())
    }
}

/// Explicit optimizer-state restart surface for one checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionOptimizerStateRestartReceipt {
    /// Optimizer family bound to the checkpoint artifact.
    pub optimizer_family: String,
    /// Stable optimizer-state checkpoint family.
    pub optimizer_checkpoint_family: String,
    /// Logical optimizer step preserved by restart.
    pub optimizer_state_step: u64,
    /// Number of parameter groups bound to restore.
    pub parameter_group_count: u16,
    /// Optimizer-state artifacts required for restart.
    pub optimizer_state_artifacts: Vec<DatastreamManifestRef>,
    /// Whether parameter-group order must match exactly on resume.
    pub strict_parameter_group_order_restore: bool,
    /// Whether the dataset and sampling cursor must match exactly on resume.
    pub resume_requires_matching_sampling_cursor: bool,
    /// Short summary of the restart semantics.
    pub summary: String,
}

impl PsionOptimizerStateRestartReceipt {
    fn validate(
        &self,
        layout_kind: PsionCheckpointLayoutKind,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        ensure_nonempty(
            self.optimizer_family.as_str(),
            "optimizer_state_restart.optimizer_family",
        )?;
        ensure_nonempty(
            self.optimizer_checkpoint_family.as_str(),
            "optimizer_state_restart.optimizer_checkpoint_family",
        )?;
        if self.optimizer_state_step == 0 {
            return Err(PsionCheckpointRecoveryContractError::InvalidOptimizerStateStep);
        }
        if self.parameter_group_count == 0 {
            return Err(PsionCheckpointRecoveryContractError::InvalidParameterGroupCount);
        }
        if self.optimizer_state_artifacts.is_empty() {
            return Err(PsionCheckpointRecoveryContractError::MissingField {
                field: String::from("optimizer_state_restart.optimizer_state_artifacts"),
            });
        }
        ensure_nonempty(self.summary.as_str(), "optimizer_state_restart.summary")?;
        for artifact in &self.optimizer_state_artifacts {
            validate_manifest_ref(
                artifact,
                self.optimizer_checkpoint_family.as_str(),
                Some(self.optimizer_state_step),
                "optimizer_state_restart.optimizer_state_artifacts[]",
            )?;
        }
        let expected_shard_count = match layout_kind {
            PsionCheckpointLayoutKind::Dense => 1,
            PsionCheckpointLayoutKind::Sharded => self.optimizer_state_artifacts.len().max(2),
        };
        if match layout_kind {
            PsionCheckpointLayoutKind::Dense => self.optimizer_state_artifacts.len() != 1,
            PsionCheckpointLayoutKind::Sharded => self.optimizer_state_artifacts.len() < 2,
        } {
            return Err(
                PsionCheckpointRecoveryContractError::OptimizerStateLayoutMismatch {
                    layout: checkpoint_layout_name(layout_kind),
                    artifact_count: self.optimizer_state_artifacts.len(),
                    expected_minimum: expected_shard_count,
                },
            );
        }
        Ok(())
    }
}

/// Dense or sharded checkpoint artifact bound to one logical promoted checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointArtifactReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Dense versus sharded layout.
    pub layout_kind: PsionCheckpointLayoutKind,
    /// Stable promoted-checkpoint label from the source stage.
    pub source_promoted_checkpoint_label: String,
    /// Stable object digest from the source promoted checkpoint.
    pub source_checkpoint_object_digest: String,
    /// Explicit checkpoint manifest for restart.
    pub checkpoint_manifest: CheckpointManifest,
    /// Explicit latest-accepted pointer for restart.
    pub checkpoint_pointer: CheckpointPointer,
    /// Dataset, sampling, and topology context carried by the artifact.
    pub checkpoint_context: PsionCheckpointContextReceipt,
    /// Explicit optimizer-state restart surface.
    pub optimizer_state_restart: PsionOptimizerStateRestartReceipt,
    /// Short summary of the artifact semantics.
    pub summary: String,
    /// Stable digest over the artifact receipt.
    pub artifact_digest: String,
}

impl PsionCheckpointArtifactReceipt {
    /// Validates one checkpoint artifact receipt against the bound stage and observability inputs.
    pub fn validate_against_inputs(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
        observability_receipt: &PsionPretrainRunObservabilityReceipt,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "checkpoint_artifact.schema_version",
        )?;
        if self.schema_version != PSION_CHECKPOINT_ARTIFACT_SCHEMA_VERSION {
            return Err(
                PsionCheckpointRecoveryContractError::SchemaVersionMismatch {
                    expected: String::from(PSION_CHECKPOINT_ARTIFACT_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(self.artifact_id.as_str(), "checkpoint_artifact.artifact_id")?;
        check_string_match(
            self.run_id.as_str(),
            stage_receipt.run_id.as_str(),
            "checkpoint_artifact.run_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            observability_receipt.run_id.as_str(),
            "checkpoint_artifact.run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_receipt.stage_id.as_str(),
            "checkpoint_artifact.stage_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            observability_receipt.stage_id.as_str(),
            "checkpoint_artifact.stage_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            stage_receipt.model_id.as_str(),
            "checkpoint_artifact.model_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            observability_receipt.model_id.as_str(),
            "checkpoint_artifact.model_id",
        )?;
        check_string_match(
            self.source_promoted_checkpoint_label.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .as_str(),
            "checkpoint_artifact.source_promoted_checkpoint_label",
        )?;
        check_string_match(
            self.source_checkpoint_object_digest.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .object_digest
                .as_str(),
            "checkpoint_artifact.source_checkpoint_object_digest",
        )?;
        self.validate_checkpoint_manifest(stage_receipt)?;
        self.validate_checkpoint_pointer()?;
        self.checkpoint_context
            .validate_against_inputs(stage_receipt, observability_receipt)?;
        self.optimizer_state_restart.validate(self.layout_kind)?;
        ensure_nonempty(self.summary.as_str(), "checkpoint_artifact.summary")?;
        if self.artifact_digest != stable_checkpoint_artifact_digest(self) {
            return Err(
                PsionCheckpointRecoveryContractError::ArtifactDigestMismatch {
                    artifact_id: self.artifact_id.clone(),
                },
            );
        }
        Ok(())
    }

    fn validate_checkpoint_manifest(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        if self.checkpoint_manifest.scope.kind != CheckpointScopeKind::Run {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointScopeMismatch {
                    artifact_id: self.artifact_id.clone(),
                    expected: String::from("run"),
                    actual: format!("{:?}", self.checkpoint_manifest.scope.kind),
                },
            );
        }
        check_string_match(
            self.checkpoint_manifest.scope.scope_id.as_str(),
            self.run_id.as_str(),
            "checkpoint_artifact.checkpoint_manifest.scope.scope_id",
        )?;
        check_string_match(
            self.checkpoint_manifest.checkpoint_family.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .checkpoint_family
                .as_str(),
            "checkpoint_artifact.checkpoint_manifest.checkpoint_family",
        )?;
        if self.checkpoint_manifest.checkpoint.checkpoint_family
            != self.checkpoint_manifest.checkpoint_family
        {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointManifestMismatch {
                    artifact_id: self.artifact_id.clone(),
                    field: String::from("checkpoint_family"),
                    expected: self.checkpoint_manifest.checkpoint_family.clone(),
                    actual: self
                        .checkpoint_manifest
                        .checkpoint
                        .checkpoint_family
                        .clone(),
                },
            );
        }
        if self.checkpoint_manifest.shards.is_empty() {
            return Err(PsionCheckpointRecoveryContractError::MissingField {
                field: String::from("checkpoint_artifact.checkpoint_manifest.shards"),
            });
        }
        let expected_manifest = CheckpointManifest::new(
            self.checkpoint_manifest.scope.clone(),
            self.checkpoint_manifest.checkpoint_family.clone(),
            self.checkpoint_manifest.checkpoint.clone(),
            self.checkpoint_manifest.shards.clone(),
            self.checkpoint_manifest.durability,
            self.checkpoint_manifest.created_at_ms,
        )?;
        if expected_manifest.manifest_digest != self.checkpoint_manifest.manifest_digest {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointManifestDigestMismatch {
                    artifact_id: self.artifact_id.clone(),
                },
            );
        }
        let expected_step = self.checkpoint_manifest.checkpoint.step;
        for shard in &self.checkpoint_manifest.shards {
            validate_checkpoint_shard(
                shard,
                self.checkpoint_manifest.checkpoint_family.as_str(),
                expected_step,
                "checkpoint_artifact.checkpoint_manifest.shards[]",
            )?;
        }
        match self.layout_kind {
            PsionCheckpointLayoutKind::Dense => {
                if self.checkpoint_manifest.shards.len() != 1 {
                    return Err(
                        PsionCheckpointRecoveryContractError::CheckpointLayoutMismatch {
                            artifact_id: self.artifact_id.clone(),
                            layout: String::from("dense"),
                            shard_count: self.checkpoint_manifest.shards.len(),
                        },
                    );
                }
            }
            PsionCheckpointLayoutKind::Sharded => {
                if self.checkpoint_manifest.shards.len() < 2 {
                    return Err(
                        PsionCheckpointRecoveryContractError::CheckpointLayoutMismatch {
                            artifact_id: self.artifact_id.clone(),
                            layout: String::from("sharded"),
                            shard_count: self.checkpoint_manifest.shards.len(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_checkpoint_pointer(&self) -> Result<(), PsionCheckpointRecoveryContractError> {
        if self.checkpoint_pointer.scope != self.checkpoint_manifest.scope {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointPointerMismatch {
                    artifact_id: self.artifact_id.clone(),
                    field: String::from("scope"),
                },
            );
        }
        if self.checkpoint_pointer.checkpoint_family != self.checkpoint_manifest.checkpoint_family {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointPointerMismatch {
                    artifact_id: self.artifact_id.clone(),
                    field: String::from("checkpoint_family"),
                },
            );
        }
        if self.checkpoint_pointer.checkpoint != self.checkpoint_manifest.checkpoint {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointPointerMismatch {
                    artifact_id: self.artifact_id.clone(),
                    field: String::from("checkpoint"),
                },
            );
        }
        if self.checkpoint_pointer.manifest_digest != self.checkpoint_manifest.manifest_digest {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointPointerMismatch {
                    artifact_id: self.artifact_id.clone(),
                    field: String::from("manifest_digest"),
                },
            );
        }
        let expected_pointer = CheckpointPointer::new(
            self.checkpoint_pointer.scope.clone(),
            self.checkpoint_pointer.checkpoint_family.clone(),
            self.checkpoint_pointer.checkpoint.clone(),
            self.checkpoint_pointer.manifest_digest.clone(),
            self.checkpoint_pointer.updated_at_ms,
        )?;
        if expected_pointer.pointer_digest != self.checkpoint_pointer.pointer_digest {
            return Err(
                PsionCheckpointRecoveryContractError::CheckpointPointerDigestMismatch {
                    artifact_id: self.artifact_id.clone(),
                },
            );
        }
        Ok(())
    }
}

/// Explicit corruption receipt for one checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCheckpointCorruptionReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Artifact the corruption was detected against.
    pub source_artifact_id: String,
    /// Digest of the corrupted manifest or optimizer artifact.
    pub corrupted_manifest_digest: String,
    /// Corruption class.
    pub corruption_kind: PsionCheckpointCorruptionKind,
    /// Whether recovery blocked continuation on detection.
    pub continuation_blocked: bool,
    /// Short summary of the corruption result.
    pub summary: String,
    /// Stable digest over the corruption receipt.
    pub receipt_digest: String,
}

impl PsionCheckpointCorruptionReceipt {
    fn validate_against_artifact(
        &self,
        source_artifact: &PsionCheckpointArtifactReceipt,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "checkpoint_corruption.schema_version",
        )?;
        if self.schema_version != PSION_CHECKPOINT_CORRUPTION_RECEIPT_SCHEMA_VERSION {
            return Err(
                PsionCheckpointRecoveryContractError::SchemaVersionMismatch {
                    expected: String::from(PSION_CHECKPOINT_CORRUPTION_RECEIPT_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(self.receipt_id.as_str(), "checkpoint_corruption.receipt_id")?;
        check_string_match(
            self.source_artifact_id.as_str(),
            source_artifact.artifact_id.as_str(),
            "checkpoint_corruption.source_artifact_id",
        )?;
        ensure_nonempty(
            self.corrupted_manifest_digest.as_str(),
            "checkpoint_corruption.corrupted_manifest_digest",
        )?;
        if !artifact_contains_digest(source_artifact, self.corrupted_manifest_digest.as_str()) {
            return Err(
                PsionCheckpointRecoveryContractError::UnknownCorruptedDigest {
                    artifact_id: source_artifact.artifact_id.clone(),
                    digest: self.corrupted_manifest_digest.clone(),
                },
            );
        }
        if !self.continuation_blocked {
            return Err(
                PsionCheckpointRecoveryContractError::CorruptionContinuationAllowed {
                    artifact_id: source_artifact.artifact_id.clone(),
                },
            );
        }
        ensure_nonempty(self.summary.as_str(), "checkpoint_corruption.summary")?;
        if self.receipt_digest != stable_checkpoint_corruption_digest(self) {
            return Err(
                PsionCheckpointRecoveryContractError::CorruptionDigestMismatch {
                    receipt_id: self.receipt_id.clone(),
                },
            );
        }
        Ok(())
    }
}

/// Recovery or invalidation event over one Psion checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCheckpointRecoveryEventReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Recovery event class.
    pub event_kind: PsionCheckpointRecoveryEventKind,
    /// Artifact the event started from.
    pub source_artifact_id: String,
    /// Requested recovery mode.
    pub recovery_mode: TrainingRecoveryMode,
    /// Restore receipt when recovery continued from a checkpoint source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_receipt: Option<crate::CheckpointRestoreReceipt>,
    /// Stable topology digest observed on the recovery or invalidation path.
    pub recovered_topology_digest: String,
    /// Worker count observed on the recovery or invalidation path.
    pub recovered_worker_count: u16,
    /// Dataset identity preserved across the event.
    pub dataset_identity: String,
    /// Sampling-policy identifier preserved across the event.
    pub sampling_policy_id: String,
    /// Sampling-policy version preserved across the event.
    pub sampling_policy_version: String,
    /// Optimizer step restored or evaluated by the event.
    pub optimizer_state_step_restored: u64,
    /// Instability telemetry observed on the event path.
    pub instability_telemetry: TrainingInstabilityTelemetry,
    /// Policy verdict over the event telemetry.
    pub stability_verdict: TrainingStabilityVerdict,
    /// Corruption receipt when corruption was detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corruption_receipt: Option<PsionCheckpointCorruptionReceipt>,
    /// Final disposition of the event.
    pub disposition: PsionCheckpointRecoveryDisposition,
    /// Stable rollback target when the event rolled back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_target_artifact_id: Option<String>,
    /// Short summary of the event result.
    pub summary: String,
    /// Stable digest over the event receipt.
    pub receipt_digest: String,
}

impl PsionCheckpointRecoveryEventReceipt {
    fn validate_against_artifacts(
        &self,
        artifacts: &BTreeMap<String, &PsionCheckpointArtifactReceipt>,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "checkpoint_recovery_event.schema_version",
        )?;
        if self.schema_version != PSION_CHECKPOINT_RECOVERY_EVENT_SCHEMA_VERSION {
            return Err(
                PsionCheckpointRecoveryContractError::SchemaVersionMismatch {
                    expected: String::from(PSION_CHECKPOINT_RECOVERY_EVENT_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "checkpoint_recovery_event.receipt_id",
        )?;
        ensure_nonempty(
            self.source_artifact_id.as_str(),
            "checkpoint_recovery_event.source_artifact_id",
        )?;
        ensure_nonempty(
            self.recovered_topology_digest.as_str(),
            "checkpoint_recovery_event.recovered_topology_digest",
        )?;
        if self.recovered_worker_count == 0 {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryWorkerCountMissing {
                    receipt_id: self.receipt_id.clone(),
                },
            );
        }
        ensure_nonempty(
            self.dataset_identity.as_str(),
            "checkpoint_recovery_event.dataset_identity",
        )?;
        ensure_nonempty(
            self.sampling_policy_id.as_str(),
            "checkpoint_recovery_event.sampling_policy_id",
        )?;
        ensure_nonempty(
            self.sampling_policy_version.as_str(),
            "checkpoint_recovery_event.sampling_policy_version",
        )?;
        if self.optimizer_state_step_restored == 0 {
            return Err(PsionCheckpointRecoveryContractError::InvalidOptimizerStateStep);
        }
        ensure_nonempty(self.summary.as_str(), "checkpoint_recovery_event.summary")?;
        validate_stability_verdict(
            &self.stability_verdict,
            "checkpoint_recovery_event.stability_verdict",
        )?;
        if !telemetry_has_any_signal(&self.instability_telemetry) {
            return Err(
                PsionCheckpointRecoveryContractError::MissingInstabilitySignal {
                    receipt_id: self.receipt_id.clone(),
                },
            );
        }
        let source_artifact = artifacts.get(&self.source_artifact_id).copied().ok_or(
            PsionCheckpointRecoveryContractError::UnknownArtifactId {
                artifact_id: self.source_artifact_id.clone(),
            },
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            source_artifact.checkpoint_context.dataset_identity.as_str(),
            "checkpoint_recovery_event.dataset_identity",
        )?;
        check_string_match(
            self.sampling_policy_id.as_str(),
            source_artifact
                .checkpoint_context
                .sampling_policy_id
                .as_str(),
            "checkpoint_recovery_event.sampling_policy_id",
        )?;
        check_string_match(
            self.sampling_policy_version.as_str(),
            source_artifact
                .checkpoint_context
                .sampling_policy_version
                .as_str(),
            "checkpoint_recovery_event.sampling_policy_version",
        )?;

        match self.event_kind {
            PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart => {
                if source_artifact.layout_kind != PsionCheckpointLayoutKind::Dense {
                    return Err(
                        PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                            receipt_id: self.receipt_id.clone(),
                            detail: String::from(
                                "forced-interruption restart must begin from a dense artifact",
                            ),
                        },
                    );
                }
                if self.disposition != PsionCheckpointRecoveryDisposition::Resumed
                    || self.corruption_receipt.is_some()
                    || self.rollback_target_artifact_id.is_some()
                {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "forced-interruption restart must resume directly without corruption or rollback target",
                        ),
                    });
                }
                self.validate_restore_receipt(source_artifact, source_artifact)?;
                if self
                    .restore_receipt
                    .as_ref()
                    .map(|receipt| receipt.source_kind)
                    != Some(CheckpointReadSourceKind::PointerLookup)
                {
                    return Err(
                        PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                            receipt_id: self.receipt_id.clone(),
                            detail: String::from(
                                "forced-interruption restart must recover through pointer lookup",
                            ),
                        },
                    );
                }
            }
            PsionCheckpointRecoveryEventKind::DistributedRestart => {
                if source_artifact.layout_kind != PsionCheckpointLayoutKind::Sharded {
                    return Err(
                        PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                            receipt_id: self.receipt_id.clone(),
                            detail: String::from(
                                "distributed restart must begin from a sharded artifact",
                            ),
                        },
                    );
                }
                if self.disposition != PsionCheckpointRecoveryDisposition::Resumed
                    || self.corruption_receipt.is_some()
                    || self.rollback_target_artifact_id.is_some()
                {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "distributed restart must resume directly without corruption or rollback target",
                        ),
                    });
                }
                if self.recovered_worker_count < 2 {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "distributed restart must preserve a multi-worker recovery topology",
                        ),
                    });
                }
                self.validate_restore_receipt(source_artifact, source_artifact)?;
                if self
                    .restore_receipt
                    .as_ref()
                    .map(|receipt| receipt.source_kind)
                    != Some(CheckpointReadSourceKind::PointerLookup)
                {
                    return Err(
                        PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                            receipt_id: self.receipt_id.clone(),
                            detail: String::from(
                                "distributed restart must recover through pointer lookup",
                            ),
                        },
                    );
                }
            }
            PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback => {
                if self.disposition
                    != PsionCheckpointRecoveryDisposition::RolledBackToStableCheckpoint
                {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption rollback must end with rolled_back_to_stable_checkpoint",
                        ),
                    });
                }
                let rollback_target_id = self.rollback_target_artifact_id.as_ref().ok_or(
                    PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption rollback must name the rollback target artifact",
                        ),
                    },
                )?;
                let rollback_target = artifacts.get(rollback_target_id).copied().ok_or(
                    PsionCheckpointRecoveryContractError::UnknownArtifactId {
                        artifact_id: rollback_target_id.clone(),
                    },
                )?;
                if rollback_target.artifact_id == source_artifact.artifact_id {
                    return Err(
                        PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                            receipt_id: self.receipt_id.clone(),
                            detail: String::from(
                                "corruption rollback must target a different stable artifact",
                            ),
                        },
                    );
                }
                let corruption_receipt = self.corruption_receipt.as_ref().ok_or(
                    PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption rollback must include a corruption receipt",
                        ),
                    },
                )?;
                corruption_receipt.validate_against_artifact(source_artifact)?;
                self.validate_restore_receipt(source_artifact, rollback_target)?;
                if self
                    .restore_receipt
                    .as_ref()
                    .map(|receipt| receipt.source_kind)
                    != Some(CheckpointReadSourceKind::ManifestListingFallback)
                {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption rollback must recover through manifest-listing fallback",
                        ),
                    });
                }
                if self.stability_verdict.action == TrainingOperationalAction::Continue {
                    return Err(
                        PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                            receipt_id: self.receipt_id.clone(),
                            detail: String::from(
                                "corruption rollback must not keep a continue stability verdict",
                            ),
                        },
                    );
                }
            }
            PsionCheckpointRecoveryEventKind::CorruptionDetectedInvalidation => {
                if self.disposition != PsionCheckpointRecoveryDisposition::Invalidated
                    || self.restore_receipt.is_some()
                    || self.rollback_target_artifact_id.is_some()
                {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption invalidation must invalidate without restore receipt or rollback target",
                        ),
                    });
                }
                let corruption_receipt = self.corruption_receipt.as_ref().ok_or(
                    PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption invalidation must include a corruption receipt",
                        ),
                    },
                )?;
                corruption_receipt.validate_against_artifact(source_artifact)?;
                if self.stability_verdict.action == TrainingOperationalAction::Continue {
                    return Err(PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                        receipt_id: self.receipt_id.clone(),
                        detail: String::from(
                            "corruption invalidation must not keep a continue stability verdict",
                        ),
                    });
                }
            }
        }

        if self.receipt_digest != stable_checkpoint_recovery_event_digest(self) {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryEventDigestMismatch {
                    receipt_id: self.receipt_id.clone(),
                },
            );
        }
        Ok(())
    }

    fn validate_restore_receipt(
        &self,
        source_artifact: &PsionCheckpointArtifactReceipt,
        target_artifact: &PsionCheckpointArtifactReceipt,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        let restore_receipt = self.restore_receipt.as_ref().ok_or(
            PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                receipt_id: self.receipt_id.clone(),
                detail: String::from("recovery event requires a restore receipt"),
            },
        )?;
        if restore_receipt.recovery_mode != self.recovery_mode {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                    receipt_id: self.receipt_id.clone(),
                    detail: String::from("restore receipt recovery mode must match the event"),
                },
            );
        }
        if restore_receipt.attempts.is_empty() || restore_receipt.receipt_digest.is_empty() {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                    receipt_id: self.receipt_id.clone(),
                    detail: String::from(
                        "restore receipt must preserve at least one attempt and a stable digest",
                    ),
                },
            );
        }
        if restore_receipt.selected_manifest != target_artifact.checkpoint_manifest {
            return Err(
                PsionCheckpointRecoveryContractError::RestoreTargetMismatch {
                    receipt_id: self.receipt_id.clone(),
                    expected_artifact_id: target_artifact.artifact_id.clone(),
                    actual_manifest_digest: restore_receipt
                        .selected_manifest
                        .manifest_digest
                        .clone(),
                },
            );
        }
        if self.optimizer_state_step_restored
            != target_artifact.optimizer_state_restart.optimizer_state_step
        {
            return Err(PsionCheckpointRecoveryContractError::RestoreStepMismatch {
                receipt_id: self.receipt_id.clone(),
                expected: target_artifact.optimizer_state_restart.optimizer_state_step,
                actual: self.optimizer_state_step_restored,
            });
        }
        if matches!(
            self.event_kind,
            PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart
                | PsionCheckpointRecoveryEventKind::DistributedRestart
        ) && restore_receipt.selected_manifest.manifest_digest
            != source_artifact.checkpoint_manifest.manifest_digest
        {
            return Err(
                PsionCheckpointRecoveryContractError::RestoreTargetMismatch {
                    receipt_id: self.receipt_id.clone(),
                    expected_artifact_id: source_artifact.artifact_id.clone(),
                    actual_manifest_digest: restore_receipt
                        .selected_manifest
                        .manifest_digest
                        .clone(),
                },
            );
        }
        Ok(())
    }
}

/// Full dense-plus-sharded checkpoint recovery bundle for one Psion run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCheckpointRecoveryBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Stable digest of the bound pretrain-stage receipt.
    pub pretrain_stage_receipt_digest: String,
    /// Stable digest of the bound run-observability receipt.
    pub observability_receipt_digest: String,
    /// Dense and sharded checkpoint artifacts bound to the run.
    pub checkpoint_artifacts: Vec<PsionCheckpointArtifactReceipt>,
    /// Forced interruption, distributed restart, rollback, and invalidation events.
    pub recovery_events: Vec<PsionCheckpointRecoveryEventReceipt>,
    /// Stable artifact id of the last stable checkpoint target.
    pub last_stable_artifact_id: String,
    /// Short summary of the full recovery contract.
    pub summary: String,
    /// Stable digest over the full bundle.
    pub bundle_digest: String,
}

impl PsionCheckpointRecoveryBundle {
    /// Validates the bundle against the bound stage and observability receipts.
    pub fn validate_against_inputs(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
        observability_receipt: &PsionPretrainRunObservabilityReceipt,
    ) -> Result<(), PsionCheckpointRecoveryContractError> {
        observability_receipt.validate_against_stage(stage_receipt)?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "checkpoint_recovery_bundle.schema_version",
        )?;
        if self.schema_version != PSION_CHECKPOINT_RECOVERY_BUNDLE_SCHEMA_VERSION {
            return Err(
                PsionCheckpointRecoveryContractError::SchemaVersionMismatch {
                    expected: String::from(PSION_CHECKPOINT_RECOVERY_BUNDLE_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        ensure_nonempty(
            self.bundle_id.as_str(),
            "checkpoint_recovery_bundle.bundle_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            stage_receipt.run_id.as_str(),
            "checkpoint_recovery_bundle.run_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            observability_receipt.run_id.as_str(),
            "checkpoint_recovery_bundle.run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_receipt.stage_id.as_str(),
            "checkpoint_recovery_bundle.stage_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            observability_receipt.stage_id.as_str(),
            "checkpoint_recovery_bundle.stage_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            stage_receipt.model_id.as_str(),
            "checkpoint_recovery_bundle.model_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            observability_receipt.model_id.as_str(),
            "checkpoint_recovery_bundle.model_id",
        )?;
        check_string_match(
            self.pretrain_stage_receipt_digest.as_str(),
            stage_receipt.receipt_digest.as_str(),
            "checkpoint_recovery_bundle.pretrain_stage_receipt_digest",
        )?;
        check_string_match(
            self.observability_receipt_digest.as_str(),
            observability_receipt.observability_digest.as_str(),
            "checkpoint_recovery_bundle.observability_receipt_digest",
        )?;
        if self.checkpoint_artifacts.is_empty() {
            return Err(PsionCheckpointRecoveryContractError::MissingField {
                field: String::from("checkpoint_recovery_bundle.checkpoint_artifacts"),
            });
        }
        if self.recovery_events.is_empty() {
            return Err(PsionCheckpointRecoveryContractError::MissingField {
                field: String::from("checkpoint_recovery_bundle.recovery_events"),
            });
        }
        ensure_nonempty(self.summary.as_str(), "checkpoint_recovery_bundle.summary")?;

        let mut artifacts_by_id = BTreeMap::new();
        let mut layouts = BTreeSet::new();
        for artifact in &self.checkpoint_artifacts {
            if artifacts_by_id
                .insert(artifact.artifact_id.clone(), artifact)
                .is_some()
            {
                return Err(PsionCheckpointRecoveryContractError::DuplicateArtifactId {
                    artifact_id: artifact.artifact_id.clone(),
                });
            }
            artifact.validate_against_inputs(stage_receipt, observability_receipt)?;
            layouts.insert(artifact.layout_kind);
        }
        for layout in PsionCheckpointLayoutKind::required_layouts() {
            if !layouts.contains(&layout) {
                return Err(
                    PsionCheckpointRecoveryContractError::MissingCheckpointLayout {
                        layout: checkpoint_layout_name(layout),
                    },
                );
            }
        }
        let last_stable_artifact = artifacts_by_id
            .get(&self.last_stable_artifact_id)
            .copied()
            .ok_or(PsionCheckpointRecoveryContractError::UnknownArtifactId {
                artifact_id: self.last_stable_artifact_id.clone(),
            })?;
        if last_stable_artifact.layout_kind != PsionCheckpointLayoutKind::Dense {
            return Err(
                PsionCheckpointRecoveryContractError::RecoveryEventContractViolation {
                    receipt_id: String::from("checkpoint_recovery_bundle"),
                    detail: String::from(
                        "last stable artifact must be the dense checkpoint artifact",
                    ),
                },
            );
        }

        let mut event_ids = BTreeSet::new();
        let mut event_kinds = BTreeSet::new();
        let mut rollback_targets = BTreeSet::new();
        for event in &self.recovery_events {
            if !event_ids.insert(event.receipt_id.clone()) {
                return Err(
                    PsionCheckpointRecoveryContractError::DuplicateRecoveryEventId {
                        receipt_id: event.receipt_id.clone(),
                    },
                );
            }
            event.validate_against_artifacts(&artifacts_by_id)?;
            event_kinds.insert(event.event_kind);
            if let Some(target) = &event.rollback_target_artifact_id {
                rollback_targets.insert(target.clone());
            }
        }
        for event_kind in PsionCheckpointRecoveryEventKind::required_kinds() {
            if !event_kinds.contains(&event_kind) {
                return Err(
                    PsionCheckpointRecoveryContractError::MissingRecoveryEventKind {
                        event_kind: recovery_event_kind_name(event_kind),
                    },
                );
            }
        }
        if !rollback_targets.contains(&self.last_stable_artifact_id) {
            return Err(
                PsionCheckpointRecoveryContractError::MissingRollbackToLastStable {
                    last_stable_artifact_id: self.last_stable_artifact_id.clone(),
                },
            );
        }
        if self.bundle_digest != stable_checkpoint_recovery_bundle_digest(self) {
            return Err(PsionCheckpointRecoveryContractError::BundleDigestMismatch);
        }
        Ok(())
    }
}

/// Creates one Psion checkpoint artifact receipt and computes its stable digest.
pub fn record_psion_checkpoint_artifact(
    artifact_id: impl Into<String>,
    layout_kind: PsionCheckpointLayoutKind,
    source_promoted_checkpoint_label: impl Into<String>,
    source_checkpoint_object_digest: impl Into<String>,
    checkpoint_manifest: CheckpointManifest,
    checkpoint_pointer: CheckpointPointer,
    checkpoint_context: PsionCheckpointContextReceipt,
    optimizer_state_restart: PsionOptimizerStateRestartReceipt,
    summary: impl Into<String>,
    stage_receipt: &PsionPretrainStageRunReceipt,
    observability_receipt: &PsionPretrainRunObservabilityReceipt,
) -> Result<PsionCheckpointArtifactReceipt, PsionCheckpointRecoveryContractError> {
    let mut artifact = PsionCheckpointArtifactReceipt {
        schema_version: String::from(PSION_CHECKPOINT_ARTIFACT_SCHEMA_VERSION),
        artifact_id: artifact_id.into(),
        run_id: stage_receipt.run_id.clone(),
        stage_id: stage_receipt.stage_id.clone(),
        model_id: stage_receipt.model_id.clone(),
        layout_kind,
        source_promoted_checkpoint_label: source_promoted_checkpoint_label.into(),
        source_checkpoint_object_digest: source_checkpoint_object_digest.into(),
        checkpoint_manifest,
        checkpoint_pointer,
        checkpoint_context,
        optimizer_state_restart,
        summary: summary.into(),
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_checkpoint_artifact_digest(&artifact);
    artifact.validate_against_inputs(stage_receipt, observability_receipt)?;
    Ok(artifact)
}

/// Creates one corruption receipt and computes its stable digest.
pub fn record_psion_checkpoint_corruption(
    receipt_id: impl Into<String>,
    source_artifact_id: impl Into<String>,
    corrupted_manifest_digest: impl Into<String>,
    corruption_kind: PsionCheckpointCorruptionKind,
    summary: impl Into<String>,
    source_artifact: &PsionCheckpointArtifactReceipt,
) -> Result<PsionCheckpointCorruptionReceipt, PsionCheckpointRecoveryContractError> {
    let mut receipt = PsionCheckpointCorruptionReceipt {
        schema_version: String::from(PSION_CHECKPOINT_CORRUPTION_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        source_artifact_id: source_artifact_id.into(),
        corrupted_manifest_digest: corrupted_manifest_digest.into(),
        corruption_kind,
        continuation_blocked: true,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_checkpoint_corruption_digest(&receipt);
    receipt.validate_against_artifact(source_artifact)?;
    Ok(receipt)
}

/// Creates one recovery or invalidation event receipt and computes its stable digest.
pub fn record_psion_checkpoint_recovery_event(
    receipt_id: impl Into<String>,
    event_kind: PsionCheckpointRecoveryEventKind,
    source_artifact_id: impl Into<String>,
    recovery_mode: TrainingRecoveryMode,
    restore_receipt: Option<crate::CheckpointRestoreReceipt>,
    recovered_topology_digest: impl Into<String>,
    recovered_worker_count: u16,
    dataset_identity: impl Into<String>,
    sampling_policy_id: impl Into<String>,
    sampling_policy_version: impl Into<String>,
    optimizer_state_step_restored: u64,
    instability_telemetry: TrainingInstabilityTelemetry,
    stability_verdict: TrainingStabilityVerdict,
    corruption_receipt: Option<PsionCheckpointCorruptionReceipt>,
    disposition: PsionCheckpointRecoveryDisposition,
    rollback_target_artifact_id: Option<String>,
    summary: impl Into<String>,
    checkpoint_artifacts: &[PsionCheckpointArtifactReceipt],
) -> Result<PsionCheckpointRecoveryEventReceipt, PsionCheckpointRecoveryContractError> {
    let mut receipt = PsionCheckpointRecoveryEventReceipt {
        schema_version: String::from(PSION_CHECKPOINT_RECOVERY_EVENT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        event_kind,
        source_artifact_id: source_artifact_id.into(),
        recovery_mode,
        restore_receipt,
        recovered_topology_digest: recovered_topology_digest.into(),
        recovered_worker_count,
        dataset_identity: dataset_identity.into(),
        sampling_policy_id: sampling_policy_id.into(),
        sampling_policy_version: sampling_policy_version.into(),
        optimizer_state_step_restored,
        instability_telemetry,
        stability_verdict,
        corruption_receipt,
        disposition,
        rollback_target_artifact_id,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_checkpoint_recovery_event_digest(&receipt);
    let artifacts = checkpoint_artifacts
        .iter()
        .map(|artifact| (artifact.artifact_id.clone(), artifact))
        .collect::<BTreeMap<_, _>>();
    receipt.validate_against_artifacts(&artifacts)?;
    Ok(receipt)
}

/// Creates the full Psion checkpoint recovery bundle and computes its stable digest.
pub fn record_psion_checkpoint_recovery_bundle(
    bundle_id: impl Into<String>,
    checkpoint_artifacts: Vec<PsionCheckpointArtifactReceipt>,
    recovery_events: Vec<PsionCheckpointRecoveryEventReceipt>,
    last_stable_artifact_id: impl Into<String>,
    summary: impl Into<String>,
    stage_receipt: &PsionPretrainStageRunReceipt,
    observability_receipt: &PsionPretrainRunObservabilityReceipt,
) -> Result<PsionCheckpointRecoveryBundle, PsionCheckpointRecoveryContractError> {
    let mut bundle = PsionCheckpointRecoveryBundle {
        schema_version: String::from(PSION_CHECKPOINT_RECOVERY_BUNDLE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        run_id: stage_receipt.run_id.clone(),
        stage_id: stage_receipt.stage_id.clone(),
        model_id: stage_receipt.model_id.clone(),
        pretrain_stage_receipt_digest: stage_receipt.receipt_digest.clone(),
        observability_receipt_digest: observability_receipt.observability_digest.clone(),
        checkpoint_artifacts,
        recovery_events,
        last_stable_artifact_id: last_stable_artifact_id.into(),
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_checkpoint_recovery_bundle_digest(&bundle);
    bundle.validate_against_inputs(stage_receipt, observability_receipt)?;
    Ok(bundle)
}

/// Recovery-contract error for Psion checkpoint artifacts and events.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionCheckpointRecoveryContractError {
    /// Wrapped preexisting checkpoint-recovery validation error.
    #[error(transparent)]
    CheckpointRecovery(#[from] CheckpointRecoveryError),
    /// Wrapped run-observability validation error.
    #[error(transparent)]
    Observability(#[from] crate::PsionPretrainRunObservabilityError),
    /// Missing required field.
    #[error("psion checkpoint recovery contract is missing `{field}`")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// Schema mismatch.
    #[error("psion checkpoint recovery schema mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch {
        /// Expected version.
        expected: String,
        /// Actual version.
        actual: String,
    },
    /// String mismatch.
    #[error(
        "psion checkpoint recovery field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        /// Field path.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Recovery context mismatch.
    #[error(
        "psion recovery context field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    RecoveryContextMismatch {
        /// Field path.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Invalid manifest ref details.
    #[error("psion checkpoint recovery manifest ref `{field}` is invalid")]
    InvalidManifestRef {
        /// Field path.
        field: String,
    },
    /// Dense or sharded layout facts do not match the artifact.
    #[error("psion checkpoint layout `{layout}` for artifact `{artifact_id}` has invalid shard count `{shard_count}`")]
    CheckpointLayoutMismatch {
        /// Artifact id.
        artifact_id: String,
        /// Layout label.
        layout: String,
        /// Shard count observed.
        shard_count: usize,
    },
    /// Optimizer-state layout did not match the checkpoint layout.
    #[error("psion optimizer-state layout `{layout}` carries `{artifact_count}` artifacts but requires at least `{expected_minimum}`")]
    OptimizerStateLayoutMismatch {
        /// Layout label.
        layout: String,
        /// Artifact count observed.
        artifact_count: usize,
        /// Expected minimum count.
        expected_minimum: usize,
    },
    /// Invalid optimizer-state step.
    #[error("psion optimizer-state step must be non-zero")]
    InvalidOptimizerStateStep,
    /// Invalid parameter-group count.
    #[error("psion optimizer-state restart must preserve at least one parameter group")]
    InvalidParameterGroupCount,
    /// Checkpoint scope mismatch.
    #[error(
        "psion checkpoint artifact `{artifact_id}` expected scope `{expected}` but got `{actual}`"
    )]
    CheckpointScopeMismatch {
        /// Artifact id.
        artifact_id: String,
        /// Expected label.
        expected: String,
        /// Actual label.
        actual: String,
    },
    /// Checkpoint manifest mismatch.
    #[error("psion checkpoint artifact `{artifact_id}` manifest field `{field}` mismatch: expected `{expected}`, got `{actual}`")]
    CheckpointManifestMismatch {
        /// Artifact id.
        artifact_id: String,
        /// Field path.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Checkpoint manifest digest mismatch.
    #[error("psion checkpoint artifact `{artifact_id}` manifest digest did not recompute")]
    CheckpointManifestDigestMismatch {
        /// Artifact id.
        artifact_id: String,
    },
    /// Checkpoint pointer mismatch.
    #[error("psion checkpoint artifact `{artifact_id}` pointer field `{field}` does not match the manifest")]
    CheckpointPointerMismatch {
        /// Artifact id.
        artifact_id: String,
        /// Field path.
        field: String,
    },
    /// Checkpoint pointer digest mismatch.
    #[error("psion checkpoint artifact `{artifact_id}` pointer digest did not recompute")]
    CheckpointPointerDigestMismatch {
        /// Artifact id.
        artifact_id: String,
    },
    /// Artifact digest mismatch.
    #[error("psion checkpoint artifact `{artifact_id}` digest did not recompute")]
    ArtifactDigestMismatch {
        /// Artifact id.
        artifact_id: String,
    },
    /// Corruption digest did not recompute.
    #[error("psion checkpoint corruption receipt `{receipt_id}` digest did not recompute")]
    CorruptionDigestMismatch {
        /// Receipt id.
        receipt_id: String,
    },
    /// Corruption listed an unknown digest.
    #[error(
        "psion checkpoint artifact `{artifact_id}` does not contain corrupted digest `{digest}`"
    )]
    UnknownCorruptedDigest {
        /// Artifact id.
        artifact_id: String,
        /// Unknown digest.
        digest: String,
    },
    /// Corruption was allowed to continue silently.
    #[error("psion checkpoint artifact `{artifact_id}` cannot continue after corruption without explicit invalidation or rollback")]
    CorruptionContinuationAllowed {
        /// Artifact id.
        artifact_id: String,
    },
    /// Missing required layout.
    #[error("psion checkpoint recovery bundle is missing `{layout}` checkpoint layout")]
    MissingCheckpointLayout {
        /// Layout label.
        layout: String,
    },
    /// Duplicate artifact id.
    #[error("psion checkpoint recovery bundle repeats artifact id `{artifact_id}`")]
    DuplicateArtifactId {
        /// Artifact id.
        artifact_id: String,
    },
    /// Duplicate event id.
    #[error("psion checkpoint recovery bundle repeats event id `{receipt_id}`")]
    DuplicateRecoveryEventId {
        /// Event receipt id.
        receipt_id: String,
    },
    /// Unknown artifact id.
    #[error("psion checkpoint recovery bundle references unknown artifact id `{artifact_id}`")]
    UnknownArtifactId {
        /// Artifact id.
        artifact_id: String,
    },
    /// Missing required event kind.
    #[error("psion checkpoint recovery bundle is missing event kind `{event_kind}`")]
    MissingRecoveryEventKind {
        /// Event kind label.
        event_kind: String,
    },
    /// Missing rollback to the last stable checkpoint.
    #[error("psion checkpoint recovery bundle never rolls back to last stable artifact `{last_stable_artifact_id}`")]
    MissingRollbackToLastStable {
        /// Stable artifact id.
        last_stable_artifact_id: String,
    },
    /// Recovery event worker count missing.
    #[error(
        "psion checkpoint recovery event `{receipt_id}` must preserve a non-zero worker count"
    )]
    RecoveryWorkerCountMissing {
        /// Receipt id.
        receipt_id: String,
    },
    /// Missing instability signal.
    #[error("psion checkpoint recovery event `{receipt_id}` must preserve at least one instability signal")]
    MissingInstabilitySignal {
        /// Receipt id.
        receipt_id: String,
    },
    /// Recovery event contract violation.
    #[error(
        "psion checkpoint recovery event `{receipt_id}` violates the event contract: {detail}"
    )]
    RecoveryEventContractViolation {
        /// Receipt id.
        receipt_id: String,
        /// Detail.
        detail: String,
    },
    /// Restore target mismatch.
    #[error("psion checkpoint recovery event `{receipt_id}` expected restore target `{expected_artifact_id}` but selected manifest `{actual_manifest_digest}`")]
    RestoreTargetMismatch {
        /// Receipt id.
        receipt_id: String,
        /// Expected artifact id.
        expected_artifact_id: String,
        /// Actual manifest digest.
        actual_manifest_digest: String,
    },
    /// Restore step mismatch.
    #[error("psion checkpoint recovery event `{receipt_id}` expected restored optimizer step `{expected}` but saw `{actual}`")]
    RestoreStepMismatch {
        /// Receipt id.
        receipt_id: String,
        /// Expected step.
        expected: u64,
        /// Actual step.
        actual: u64,
    },
    /// Recovery event digest mismatch.
    #[error("psion checkpoint recovery event `{receipt_id}` digest did not recompute")]
    RecoveryEventDigestMismatch {
        /// Receipt id.
        receipt_id: String,
    },
    /// Bundle digest mismatch.
    #[error("psion checkpoint recovery bundle digest did not recompute")]
    BundleDigestMismatch,
}

fn stable_checkpoint_artifact_digest(artifact: &PsionCheckpointArtifactReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_checkpoint_artifact|");
    hasher.update(artifact.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_layout_label(artifact.layout_kind));
    hasher.update(b"|");
    hasher.update(artifact.source_promoted_checkpoint_label.as_bytes());
    hasher.update(b"|");
    hasher.update(artifact.source_checkpoint_object_digest.as_bytes());
    hasher.update(b"|manifest|");
    hasher.update(stable_json_bytes(&artifact.checkpoint_manifest));
    hasher.update(b"|pointer|");
    hasher.update(stable_json_bytes(&artifact.checkpoint_pointer));
    hasher.update(b"|context|");
    hasher.update(stable_json_bytes(&artifact.checkpoint_context));
    hasher.update(b"|optimizer|");
    hasher.update(stable_json_bytes(&artifact.optimizer_state_restart));
    hasher.update(b"|");
    hasher.update(artifact.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_checkpoint_corruption_digest(receipt: &PsionCheckpointCorruptionReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_checkpoint_corruption|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.source_artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.corrupted_manifest_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(corruption_kind_label(receipt.corruption_kind));
    if receipt.continuation_blocked {
        hasher.update(b"|blocked|");
    } else {
        hasher.update(b"|continued|");
    }
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_checkpoint_recovery_event_digest(
    receipt: &PsionCheckpointRecoveryEventReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_checkpoint_recovery_event|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(recovery_event_kind_label(receipt.event_kind));
    hasher.update(b"|");
    hasher.update(receipt.source_artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(training_recovery_mode_label(receipt.recovery_mode));
    if let Some(restore_receipt) = &receipt.restore_receipt {
        hasher.update(b"|restore|");
        hasher.update(stable_json_bytes(restore_receipt));
    }
    hasher.update(b"|");
    hasher.update(receipt.recovered_topology_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.recovered_worker_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.dataset_identity.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.sampling_policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.sampling_policy_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.optimizer_state_step_restored.to_string().as_bytes());
    hasher.update(b"|telemetry|");
    hasher.update(stable_json_bytes(&receipt.instability_telemetry));
    hasher.update(b"|verdict|");
    hasher.update(stable_json_bytes(&receipt.stability_verdict));
    if let Some(corruption_receipt) = &receipt.corruption_receipt {
        hasher.update(b"|corruption|");
        hasher.update(stable_json_bytes(corruption_receipt));
    }
    hasher.update(b"|");
    hasher.update(recovery_disposition_label(receipt.disposition));
    if let Some(rollback_target_artifact_id) = &receipt.rollback_target_artifact_id {
        hasher.update(b"|rollback|");
        hasher.update(rollback_target_artifact_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_checkpoint_recovery_bundle_digest(bundle: &PsionCheckpointRecoveryBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_checkpoint_recovery_bundle|");
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.pretrain_stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.observability_receipt_digest.as_bytes());
    for artifact in &bundle.checkpoint_artifacts {
        hasher.update(b"|artifact|");
        hasher.update(artifact.artifact_digest.as_bytes());
    }
    for event in &bundle.recovery_events {
        hasher.update(b"|event|");
        hasher.update(event.receipt_digest.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(bundle.last_stable_artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn validate_checkpoint_shard(
    shard: &CheckpointShardManifest,
    checkpoint_family: &str,
    expected_step: Option<u64>,
    field_prefix: &str,
) -> Result<(), PsionCheckpointRecoveryContractError> {
    ensure_nonempty(
        shard.shard_id.as_str(),
        format!("{field_prefix}.shard_id").as_str(),
    )?;
    ensure_nonempty(
        shard.writer_node_id.as_str(),
        format!("{field_prefix}.writer_node_id").as_str(),
    )?;
    validate_manifest_ref(
        &shard.manifest,
        checkpoint_family,
        expected_step,
        format!("{field_prefix}.manifest").as_str(),
    )?;
    Ok(())
}

fn validate_manifest_ref(
    manifest_ref: &DatastreamManifestRef,
    checkpoint_family: &str,
    expected_step: Option<u64>,
    field: &str,
) -> Result<(), PsionCheckpointRecoveryContractError> {
    if manifest_ref.stream_id.trim().is_empty()
        || manifest_ref.manifest_digest.trim().is_empty()
        || manifest_ref.object_digest.trim().is_empty()
        || manifest_ref.total_bytes == 0
        || manifest_ref.chunk_count == 0
        || manifest_ref.chunk_bytes == 0
    {
        return Err(PsionCheckpointRecoveryContractError::InvalidManifestRef {
            field: String::from(field),
        });
    }
    let binding = manifest_ref.checkpoint_binding.as_ref().ok_or(
        PsionCheckpointRecoveryContractError::InvalidManifestRef {
            field: String::from(field),
        },
    )?;
    if binding.checkpoint_family != checkpoint_family {
        return Err(PsionCheckpointRecoveryContractError::FieldMismatch {
            field: String::from(field),
            expected: String::from(checkpoint_family),
            actual: binding.checkpoint_family.clone(),
        });
    }
    if binding
        .checkpoint_ref
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        return Err(PsionCheckpointRecoveryContractError::InvalidManifestRef {
            field: String::from(field),
        });
    }
    if binding.step != expected_step {
        return Err(PsionCheckpointRecoveryContractError::FieldMismatch {
            field: String::from(field),
            expected: expected_step
                .map(|step| step.to_string())
                .unwrap_or_else(|| String::from("none")),
            actual: binding
                .step
                .map(|step| step.to_string())
                .unwrap_or_else(|| String::from("none")),
        });
    }
    Ok(())
}

fn validate_stability_verdict(
    verdict: &TrainingStabilityVerdict,
    field: &str,
) -> Result<(), PsionCheckpointRecoveryContractError> {
    if verdict.policy_digest.trim().is_empty() || verdict.verdict_digest.trim().is_empty() {
        return Err(PsionCheckpointRecoveryContractError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn telemetry_has_any_signal(telemetry: &TrainingInstabilityTelemetry) -> bool {
    telemetry.max_gradient_norm_l2.is_some()
        || telemetry.mean_clipping_ratio.is_some()
        || telemetry.entropy_drift_bps.is_some()
        || telemetry.stale_rollout_drop_rate_bps > 0
        || telemetry.checkpoint_catchup_latency_ms.unwrap_or_default() > 0
        || telemetry.topology_churn_events > 0
        || telemetry.environment_failure_rate_bps > 0
        || telemetry.sandbox_failure_rate_bps > 0
}

fn artifact_contains_digest(artifact: &PsionCheckpointArtifactReceipt, digest: &str) -> bool {
    artifact.checkpoint_manifest.manifest_digest == digest
        || artifact
            .checkpoint_manifest
            .shards
            .iter()
            .any(|shard| shard.manifest.manifest_digest == digest)
        || artifact
            .optimizer_state_restart
            .optimizer_state_artifacts
            .iter()
            .any(|entry| entry.manifest_digest == digest)
}

fn stable_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("serializing stable psion checkpoint JSON should succeed")
}

fn checkpoint_layout_label(layout: PsionCheckpointLayoutKind) -> &'static [u8] {
    match layout {
        PsionCheckpointLayoutKind::Dense => b"dense",
        PsionCheckpointLayoutKind::Sharded => b"sharded",
    }
}

fn checkpoint_layout_name(layout: PsionCheckpointLayoutKind) -> String {
    String::from_utf8(checkpoint_layout_label(layout).to_vec())
        .expect("checkpoint layout label should be valid UTF-8")
}

fn recovery_event_kind_label(kind: PsionCheckpointRecoveryEventKind) -> &'static [u8] {
    match kind {
        PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart => {
            b"forced_interruption_restart"
        }
        PsionCheckpointRecoveryEventKind::DistributedRestart => b"distributed_restart",
        PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback => {
            b"corruption_detected_rollback"
        }
        PsionCheckpointRecoveryEventKind::CorruptionDetectedInvalidation => {
            b"corruption_detected_invalidation"
        }
    }
}

fn recovery_event_kind_name(kind: PsionCheckpointRecoveryEventKind) -> String {
    String::from_utf8(recovery_event_kind_label(kind).to_vec())
        .expect("recovery event label should be valid UTF-8")
}

fn recovery_disposition_label(disposition: PsionCheckpointRecoveryDisposition) -> &'static [u8] {
    match disposition {
        PsionCheckpointRecoveryDisposition::Resumed => b"resumed",
        PsionCheckpointRecoveryDisposition::RolledBackToStableCheckpoint => {
            b"rolled_back_to_stable_checkpoint"
        }
        PsionCheckpointRecoveryDisposition::Invalidated => b"invalidated",
    }
}

fn corruption_kind_label(kind: PsionCheckpointCorruptionKind) -> &'static [u8] {
    match kind {
        PsionCheckpointCorruptionKind::ManifestDigestMismatch => b"manifest_digest_mismatch",
        PsionCheckpointCorruptionKind::MissingShard => b"missing_shard",
        PsionCheckpointCorruptionKind::OptimizerStateMismatch => b"optimizer_state_mismatch",
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

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionCheckpointRecoveryContractError> {
    if value.trim().is_empty() {
        return Err(PsionCheckpointRecoveryContractError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionCheckpointRecoveryContractError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionCheckpointRecoveryContractError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use psionic_cluster::NodeId;
    use psionic_datastream::{
        DatastreamCheckpointBinding, DatastreamEncoding, DatastreamSubjectKind,
    };
    use psionic_runtime::TrainingCheckpointReference;

    use super::*;
    use crate::{
        record_psion_checkpoint_artifact, record_psion_checkpoint_corruption,
        record_psion_checkpoint_recovery_bundle, record_psion_checkpoint_recovery_event,
        CheckpointDurabilityPosture, CheckpointScopeBinding, CheckpointStoreReadOptions,
        InMemoryCheckpointStore, TrainingInstabilityPolicy, TrainingInstabilityRule,
        TrainingRiskyOptimizationRule, TrainingStabilityController,
    };

    fn stage_receipt() -> PsionPretrainStageRunReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"
        ))
        .expect("stage receipt should parse")
    }

    fn observability_receipt() -> PsionPretrainRunObservabilityReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/observability/psion_pilot_pretrain_run_observability_receipt_v1.json"
        ))
        .expect("observability receipt should parse")
    }

    fn recovery_bundle() -> PsionCheckpointRecoveryBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/checkpoint_recovery/psion_checkpoint_recovery_bundle_v1.json"
        ))
        .expect("checkpoint recovery bundle should parse")
    }

    fn dense_artifact() -> PsionCheckpointArtifactReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/checkpoint_recovery/psion_dense_checkpoint_artifact_v1.json"
        ))
        .expect("dense artifact should parse")
    }

    fn sharded_artifact() -> PsionCheckpointArtifactReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/checkpoint_recovery/psion_sharded_checkpoint_artifact_v1.json"
        ))
        .expect("sharded artifact should parse")
    }

    #[test]
    fn recovery_bundle_validates_dense_sharded_restart_and_corruption_paths(
    ) -> Result<(), Box<dyn std::error::Error>> {
        recovery_bundle().validate_against_inputs(&stage_receipt(), &observability_receipt())?;
        Ok(())
    }

    #[test]
    fn dense_artifact_requires_single_checkpoint_shard() {
        let mut artifact = dense_artifact();
        artifact
            .checkpoint_manifest
            .shards
            .push(artifact.checkpoint_manifest.shards[0].clone());
        artifact.checkpoint_manifest = CheckpointManifest::new(
            artifact.checkpoint_manifest.scope.clone(),
            artifact.checkpoint_manifest.checkpoint_family.clone(),
            artifact.checkpoint_manifest.checkpoint.clone(),
            artifact.checkpoint_manifest.shards.clone(),
            artifact.checkpoint_manifest.durability,
            artifact.checkpoint_manifest.created_at_ms,
        )
        .expect("checkpoint manifest should rebuild");
        artifact.checkpoint_pointer = CheckpointPointer::new(
            artifact.checkpoint_pointer.scope.clone(),
            artifact.checkpoint_pointer.checkpoint_family.clone(),
            artifact.checkpoint_pointer.checkpoint.clone(),
            artifact.checkpoint_manifest.manifest_digest.clone(),
            artifact.checkpoint_pointer.updated_at_ms,
        )
        .expect("checkpoint pointer should rebuild");
        artifact.artifact_digest = stable_checkpoint_artifact_digest(&artifact);
        let error = artifact
            .validate_against_inputs(&stage_receipt(), &observability_receipt())
            .expect_err("dense artifact should reject multi-shard checkpoint layout");
        assert!(matches!(
            error,
            PsionCheckpointRecoveryContractError::CheckpointLayoutMismatch { .. }
        ));
    }

    #[test]
    fn corruption_rollback_requires_explicit_rollback_target() {
        let mut bundle = recovery_bundle();
        let event = bundle
            .recovery_events
            .iter_mut()
            .find(|event| {
                event.event_kind == PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback
            })
            .expect("bundle should include rollback event");
        event.rollback_target_artifact_id = None;
        event.receipt_digest = stable_checkpoint_recovery_event_digest(event);
        let error = bundle
            .validate_against_inputs(&stage_receipt(), &observability_receipt())
            .expect_err("rollback event should require a rollback target");
        assert!(matches!(
            error,
            PsionCheckpointRecoveryContractError::RecoveryEventContractViolation { .. }
        ));
    }

    #[test]
    fn corruption_invalidation_rejects_restore_receipt() {
        let mut bundle = recovery_bundle();
        let forced_restore = bundle
            .recovery_events
            .iter()
            .find(|other| {
                other.event_kind == PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart
            })
            .and_then(|other| other.restore_receipt.clone());
        let event = bundle
            .recovery_events
            .iter_mut()
            .find(|event| {
                event.event_kind == PsionCheckpointRecoveryEventKind::CorruptionDetectedInvalidation
            })
            .expect("bundle should include invalidation event");
        event.restore_receipt = forced_restore;
        event.receipt_digest = stable_checkpoint_recovery_event_digest(event);
        let dense = dense_artifact();
        let sharded = sharded_artifact();
        let error = event
            .validate_against_artifacts(
                &[
                    (dense.artifact_id.clone(), &dense),
                    (sharded.artifact_id.clone(), &sharded),
                ]
                .into_iter()
                .collect(),
            )
            .expect_err("invalidation should not include a restore receipt");
        assert!(matches!(
            error,
            PsionCheckpointRecoveryContractError::RecoveryEventContractViolation { .. }
        ));
    }

    #[test]
    fn recovery_bundle_builder_recomputes_fixture_digest() -> Result<(), Box<dyn std::error::Error>>
    {
        let stage_receipt = stage_receipt();
        let observability_receipt = observability_receipt();
        let scope =
            CheckpointScopeBinding::new(CheckpointScopeKind::Run, stage_receipt.run_id.clone());
        let checkpoint_family = stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint
            .checkpoint_family
            .clone();
        let base_checkpoint = stage_receipt.checkpoint_lineage.promoted_checkpoint.clone();

        let dense_manifest = CheckpointManifest::new(
            scope.clone(),
            checkpoint_family.clone(),
            base_checkpoint.clone(),
            vec![CheckpointShardManifest {
                shard_id: String::from("dense-shard-0"),
                manifest: checkpoint_stream_ref(
                    "stream-psion-pretrain-final-v1",
                    "manifest-psion-pretrain-final-v1",
                    "object-psion-pretrain-final-v1",
                    checkpoint_family.as_str(),
                    base_checkpoint
                        .checkpoint_ref
                        .as_deref()
                        .unwrap_or("checkpoint://psion/pilot/pretrain/final"),
                    2048,
                    143_654_912,
                ),
                writer_node_id: String::from("node-psion-a"),
            }],
            CheckpointDurabilityPosture::Durable,
            1_742_615_100_000,
        )?;
        let dense_pointer = CheckpointPointer::new(
            scope.clone(),
            checkpoint_family.clone(),
            base_checkpoint.clone(),
            dense_manifest.manifest_digest.clone(),
            1_742_615_100_500,
        )?;

        let sharded_checkpoint = TrainingCheckpointReference::new(
            checkpoint_family.clone(),
            "stream-psion-pretrain-final-sharded-v1",
            "manifest-psion-pretrain-final-sharded-v1",
            "object-psion-pretrain-final-sharded-v1",
            "node-psion-a",
            base_checkpoint.membership_epoch,
            base_checkpoint.cluster_state_digest.clone(),
            base_checkpoint.topology_digest.clone(),
            base_checkpoint.started_at_ms,
        )
        .with_checkpoint_ref("checkpoint://psion/pilot/pretrain/final/sharded")
        .with_step(base_checkpoint.step.unwrap_or(2048))
        .with_durable_at_ms(base_checkpoint.durable_at_ms.unwrap_or(1_742_615_100_000));
        let sharded_manifest = CheckpointManifest::new(
            scope.clone(),
            checkpoint_family.clone(),
            sharded_checkpoint.clone(),
            vec![
                checkpoint_shard_manifest(
                    "sharded-shard-0",
                    "stream-psion-pretrain-final-shard-0-v1",
                    "manifest-psion-pretrain-final-shard-0-v1",
                    "object-psion-pretrain-final-shard-0-v1",
                    checkpoint_family.as_str(),
                    "checkpoint://psion/pilot/pretrain/final/sharded",
                    2048,
                    36_000_000,
                    "node-psion-a",
                ),
                checkpoint_shard_manifest(
                    "sharded-shard-1",
                    "stream-psion-pretrain-final-shard-1-v1",
                    "manifest-psion-pretrain-final-shard-1-v1",
                    "object-psion-pretrain-final-shard-1-v1",
                    checkpoint_family.as_str(),
                    "checkpoint://psion/pilot/pretrain/final/sharded",
                    2048,
                    36_000_000,
                    "node-psion-b",
                ),
                checkpoint_shard_manifest(
                    "sharded-shard-2",
                    "stream-psion-pretrain-final-shard-2-v1",
                    "manifest-psion-pretrain-final-shard-2-v1",
                    "object-psion-pretrain-final-shard-2-v1",
                    checkpoint_family.as_str(),
                    "checkpoint://psion/pilot/pretrain/final/sharded",
                    2048,
                    36_000_000,
                    "node-psion-c",
                ),
                checkpoint_shard_manifest(
                    "sharded-shard-3",
                    "stream-psion-pretrain-final-shard-3-v1",
                    "manifest-psion-pretrain-final-shard-3-v1",
                    "object-psion-pretrain-final-shard-3-v1",
                    checkpoint_family.as_str(),
                    "checkpoint://psion/pilot/pretrain/final/sharded",
                    2048,
                    36_000_000,
                    "node-psion-d",
                ),
            ],
            CheckpointDurabilityPosture::Durable,
            1_742_615_120_000,
        )?;
        let sharded_pointer = CheckpointPointer::new(
            scope.clone(),
            checkpoint_family.clone(),
            sharded_checkpoint.clone(),
            sharded_manifest.manifest_digest.clone(),
            1_742_615_120_500,
        )?;

        let dense_artifact = record_psion_checkpoint_artifact(
            "psion-dense-checkpoint-artifact-v1",
            PsionCheckpointLayoutKind::Dense,
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .object_digest
                .clone(),
            dense_manifest.clone(),
            dense_pointer.clone(),
            checkpoint_context(&stage_receipt, &observability_receipt),
            dense_optimizer_restart(),
            "Dense checkpoint artifact preserves exact pointer-first restart semantics for the promoted checkpoint.",
            &stage_receipt,
            &observability_receipt,
        )?;
        let sharded_artifact = record_psion_checkpoint_artifact(
            "psion-sharded-checkpoint-artifact-v1",
            PsionCheckpointLayoutKind::Sharded,
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .object_digest
                .clone(),
            sharded_manifest.clone(),
            sharded_pointer.clone(),
            checkpoint_context(&stage_receipt, &observability_receipt),
            sharded_optimizer_restart(),
            "Sharded checkpoint artifact freezes the distributed restart mirror over the same logical promoted checkpoint.",
            &stage_receipt,
            &observability_receipt,
        )?;

        let dense_restore = restore_receipt(
            dense_manifest.clone(),
            dense_pointer.clone(),
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
            &[NodeId::new("node-psion-a")],
        )?;
        let sharded_restore = restore_receipt(
            sharded_manifest.clone(),
            sharded_pointer.clone(),
            TrainingRecoveryMode::BlockingCatchUp,
            &[
                NodeId::new("node-psion-a"),
                NodeId::new("node-psion-b"),
                NodeId::new("node-psion-c"),
                NodeId::new("node-psion-d"),
            ],
        )?;
        let rollback_restore = stale_pointer_fallback_restore(
            dense_manifest.clone(),
            sharded_pointer.clone(),
            checkpoint_family.as_str(),
            scope.clone(),
        )?;

        let artifacts = vec![dense_artifact.clone(), sharded_artifact.clone()];
        let forced_restart = record_psion_checkpoint_recovery_event(
            "psion-forced-interruption-restart-v1",
            PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart,
            dense_artifact.artifact_id.clone(),
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
            Some(dense_restore),
            "psion-recovery-topology-single-device-v1",
            1,
            stage_receipt.dataset_identity.clone(),
            stage_receipt.sampling_policy_id.clone(),
            stage_receipt.sampling_policy_version.clone(),
            2048,
            TrainingInstabilityTelemetry::default().with_checkpoint_catchup_latency_ms(220),
            continue_verdict(220, 0, 0),
            None,
            PsionCheckpointRecoveryDisposition::Resumed,
            None,
            "Forced interruption restarted from the dense checkpoint through pointer lookup.",
            &artifacts,
        )?;
        let distributed_restart = record_psion_checkpoint_recovery_event(
            "psion-distributed-restart-v1",
            PsionCheckpointRecoveryEventKind::DistributedRestart,
            sharded_artifact.artifact_id.clone(),
            TrainingRecoveryMode::BlockingCatchUp,
            Some(sharded_restore),
            "psion-recovery-topology-four-worker-v1",
            4,
            stage_receipt.dataset_identity.clone(),
            stage_receipt.sampling_policy_id.clone(),
            stage_receipt.sampling_policy_version.clone(),
            2048,
            TrainingInstabilityTelemetry::default()
                .with_checkpoint_catchup_latency_ms(420)
                .with_topology_churn_events(1),
            continue_verdict(420, 1, 0),
            None,
            PsionCheckpointRecoveryDisposition::Resumed,
            None,
            "Distributed restart resumed the sharded mirror on a four-worker recovery topology.",
            &artifacts,
        )?;
        let rollback_corruption = record_psion_checkpoint_corruption(
            "psion-sharded-corruption-v1",
            sharded_artifact.artifact_id.clone(),
            sharded_artifact.checkpoint_manifest.manifest_digest.clone(),
            PsionCheckpointCorruptionKind::ManifestDigestMismatch,
            "Sharded checkpoint corruption blocked continuation and forced rollback to the last stable dense artifact.",
            &sharded_artifact,
        )?;
        let corruption_rollback = record_psion_checkpoint_recovery_event(
            "psion-corruption-rollback-v1",
            PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback,
            sharded_artifact.artifact_id.clone(),
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
            Some(rollback_restore),
            "psion-recovery-topology-rollback-v1",
            1,
            stage_receipt.dataset_identity.clone(),
            stage_receipt.sampling_policy_id.clone(),
            stage_receipt.sampling_policy_version.clone(),
            2048,
            TrainingInstabilityTelemetry::default()
                .with_checkpoint_catchup_latency_ms(780)
                .with_topology_churn_events(2),
            quarantine_verdict(780, 2),
            Some(rollback_corruption),
            PsionCheckpointRecoveryDisposition::RolledBackToStableCheckpoint,
            Some(dense_artifact.artifact_id.clone()),
            "Manifest corruption triggered listing fallback and rollback to the last stable dense artifact.",
            &artifacts,
        )?;
        let invalidation_corruption = record_psion_checkpoint_corruption(
            "psion-dense-optimizer-corruption-v1",
            dense_artifact.artifact_id.clone(),
            dense_artifact
                .optimizer_state_restart
                .optimizer_state_artifacts[0]
                .manifest_digest
                .clone(),
            PsionCheckpointCorruptionKind::OptimizerStateMismatch,
            "Optimizer-state corruption invalidated the run rather than allowing silent continuation.",
            &dense_artifact,
        )?;
        let corruption_invalidation = record_psion_checkpoint_recovery_event(
            "psion-corruption-invalidation-v1",
            PsionCheckpointRecoveryEventKind::CorruptionDetectedInvalidation,
            dense_artifact.artifact_id.clone(),
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
            None,
            "psion-recovery-topology-invalidation-v1",
            1,
            stage_receipt.dataset_identity.clone(),
            stage_receipt.sampling_policy_id.clone(),
            stage_receipt.sampling_policy_version.clone(),
            2048,
            TrainingInstabilityTelemetry::default()
                .with_checkpoint_catchup_latency_ms(910)
                .with_environment_failure_rate_bps(140),
            halt_verdict(910, 140),
            Some(invalidation_corruption),
            PsionCheckpointRecoveryDisposition::Invalidated,
            None,
            "Optimizer-state corruption invalidated the run instead of resuming from a possibly poisoned state.",
            &artifacts,
        )?;

        let bundle = record_psion_checkpoint_recovery_bundle(
            "psion-checkpoint-recovery-bundle-v1",
            artifacts,
            vec![
                forced_restart,
                distributed_restart,
                corruption_rollback,
                corruption_invalidation,
            ],
            dense_artifact.artifact_id.clone(),
            "Psion checkpoint recovery bundle freezes dense restart, sharded distributed restart, corruption rollback, and corruption invalidation over one bounded promoted checkpoint.",
            &stage_receipt,
            &observability_receipt,
        )?;
        assert_eq!(bundle.bundle_digest, recovery_bundle().bundle_digest);
        Ok(())
    }

    fn checkpoint_context(
        stage_receipt: &PsionPretrainStageRunReceipt,
        observability_receipt: &PsionPretrainRunObservabilityReceipt,
    ) -> PsionCheckpointContextReceipt {
        PsionCheckpointContextReceipt {
            training_run_profile: observability_receipt.run_profile,
            dataset_identity: stage_receipt.dataset_identity.clone(),
            sampling_policy_id: stage_receipt.sampling_policy_id.clone(),
            sampling_policy_version: stage_receipt.sampling_policy_version.clone(),
            source_checkpoint_topology_digest: stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .topology_digest
                .clone(),
            training_hardware_topology_digest: observability_receipt
                .hardware_topology
                .topology_digest
                .clone(),
            observed_worker_count: observability_receipt.hardware_topology.observed_worker_count,
            detail: String::from(
                "Checkpoint artifacts preserve the source checkpoint topology and the realized training hardware topology separately.",
            ),
        }
    }

    fn dense_optimizer_restart() -> PsionOptimizerStateRestartReceipt {
        PsionOptimizerStateRestartReceipt {
            optimizer_family: String::from("adamw"),
            optimizer_checkpoint_family: String::from("train.psion.decoder.optimizer_state"),
            optimizer_state_step: 2048,
            parameter_group_count: 8,
            optimizer_state_artifacts: vec![checkpoint_stream_ref(
                "stream-psion-pretrain-final-optimizer-v1",
                "manifest-psion-pretrain-final-optimizer-v1",
                "object-psion-pretrain-final-optimizer-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/pilot/pretrain/final/optimizer_state",
                2048,
                71_827_456,
            )],
            strict_parameter_group_order_restore: true,
            resume_requires_matching_sampling_cursor: true,
            summary: String::from(
                "Dense optimizer-state restart keeps exact parameter-group order and sampling cursor binding.",
            ),
        }
    }

    fn sharded_optimizer_restart() -> PsionOptimizerStateRestartReceipt {
        PsionOptimizerStateRestartReceipt {
            optimizer_family: String::from("adamw"),
            optimizer_checkpoint_family: String::from("train.psion.decoder.optimizer_state"),
            optimizer_state_step: 2048,
            parameter_group_count: 8,
            optimizer_state_artifacts: vec![
                checkpoint_stream_ref(
                    "stream-psion-pretrain-final-optimizer-shard-0-v1",
                    "manifest-psion-pretrain-final-optimizer-shard-0-v1",
                    "object-psion-pretrain-final-optimizer-shard-0-v1",
                    "train.psion.decoder.optimizer_state",
                    "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                    2048,
                    18_000_000,
                ),
                checkpoint_stream_ref(
                    "stream-psion-pretrain-final-optimizer-shard-1-v1",
                    "manifest-psion-pretrain-final-optimizer-shard-1-v1",
                    "object-psion-pretrain-final-optimizer-shard-1-v1",
                    "train.psion.decoder.optimizer_state",
                    "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                    2048,
                    18_000_000,
                ),
                checkpoint_stream_ref(
                    "stream-psion-pretrain-final-optimizer-shard-2-v1",
                    "manifest-psion-pretrain-final-optimizer-shard-2-v1",
                    "object-psion-pretrain-final-optimizer-shard-2-v1",
                    "train.psion.decoder.optimizer_state",
                    "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                    2048,
                    18_000_000,
                ),
                checkpoint_stream_ref(
                    "stream-psion-pretrain-final-optimizer-shard-3-v1",
                    "manifest-psion-pretrain-final-optimizer-shard-3-v1",
                    "object-psion-pretrain-final-optimizer-shard-3-v1",
                    "train.psion.decoder.optimizer_state",
                    "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                    2048,
                    18_000_000,
                ),
            ],
            strict_parameter_group_order_restore: true,
            resume_requires_matching_sampling_cursor: true,
            summary: String::from(
                "Sharded optimizer-state restart preserves group order, step identity, and exact sampling cursor binding across four shards.",
            ),
        }
    }

    fn checkpoint_stream_ref(
        stream_id: &str,
        manifest_digest: &str,
        object_digest: &str,
        checkpoint_family: &str,
        checkpoint_ref: &str,
        step: u64,
        total_bytes: u64,
    ) -> DatastreamManifestRef {
        DatastreamManifestRef {
            stream_id: String::from(stream_id),
            manifest_digest: String::from(manifest_digest),
            subject: DatastreamSubjectKind::Checkpoint,
            object_digest: String::from(object_digest),
            total_bytes,
            chunk_count: 8,
            chunk_bytes: 4 * 1024 * 1024,
            encoding: DatastreamEncoding::Safetensors,
            compression: None,
            provenance_digest: None,
            dataset_binding: None,
            checkpoint_binding: Some(
                DatastreamCheckpointBinding::new(checkpoint_family)
                    .with_checkpoint_ref(checkpoint_ref)
                    .with_step(step),
            ),
            policy_weight_binding: None,
            mirrors: Vec::new(),
        }
    }

    fn checkpoint_shard_manifest(
        shard_id: &str,
        stream_id: &str,
        manifest_digest: &str,
        object_digest: &str,
        checkpoint_family: &str,
        checkpoint_ref: &str,
        step: u64,
        total_bytes: u64,
        writer_node_id: &str,
    ) -> CheckpointShardManifest {
        CheckpointShardManifest {
            shard_id: String::from(shard_id),
            manifest: checkpoint_stream_ref(
                stream_id,
                manifest_digest,
                object_digest,
                checkpoint_family,
                checkpoint_ref,
                step,
                total_bytes,
            ),
            writer_node_id: String::from(writer_node_id),
        }
    }

    fn restore_receipt(
        manifest: CheckpointManifest,
        pointer: CheckpointPointer,
        recovery_mode: TrainingRecoveryMode,
        uploader_candidates: &[NodeId],
    ) -> Result<crate::CheckpointRestoreReceipt, Box<dyn std::error::Error>> {
        let mut store = InMemoryCheckpointStore::default();
        store.store_manifest(manifest.clone());
        store.store_pointer(pointer);
        Ok(store.plan_restore(
            &manifest.scope,
            manifest.checkpoint_family.as_str(),
            recovery_mode,
            uploader_candidates,
            CheckpointStoreReadOptions::default(),
        )?)
    }

    fn stale_pointer_fallback_restore(
        dense_manifest: CheckpointManifest,
        stale_pointer: CheckpointPointer,
        checkpoint_family: &str,
        scope: CheckpointScopeBinding,
    ) -> Result<crate::CheckpointRestoreReceipt, Box<dyn std::error::Error>> {
        let mut store = InMemoryCheckpointStore::default();
        store.store_manifest(dense_manifest);
        store.store_pointer(stale_pointer);
        Ok(store.plan_restore(
            &scope,
            checkpoint_family,
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
            &[NodeId::new("node-psion-a")],
            CheckpointStoreReadOptions::default(),
        )?)
    }

    fn continue_verdict(
        checkpoint_catchup_latency_ms: u64,
        topology_churn_events: u32,
        environment_failure_rate_bps: u32,
    ) -> TrainingStabilityVerdict {
        stability_controller().evaluate(
            &TrainingInstabilityTelemetry::default()
                .with_checkpoint_catchup_latency_ms(checkpoint_catchup_latency_ms)
                .with_topology_churn_events(topology_churn_events)
                .with_environment_failure_rate_bps(environment_failure_rate_bps),
            &[],
        )
    }

    fn quarantine_verdict(
        checkpoint_catchup_latency_ms: u64,
        topology_churn_events: u32,
    ) -> TrainingStabilityVerdict {
        stability_controller().evaluate(
            &TrainingInstabilityTelemetry::default()
                .with_checkpoint_catchup_latency_ms(checkpoint_catchup_latency_ms)
                .with_topology_churn_events(topology_churn_events),
            &[],
        )
    }

    fn halt_verdict(
        checkpoint_catchup_latency_ms: u64,
        environment_failure_rate_bps: u32,
    ) -> TrainingStabilityVerdict {
        stability_controller().evaluate(
            &TrainingInstabilityTelemetry::default()
                .with_checkpoint_catchup_latency_ms(checkpoint_catchup_latency_ms)
                .with_environment_failure_rate_bps(environment_failure_rate_bps),
            &[],
        )
    }

    fn stability_controller() -> TrainingStabilityController {
        TrainingStabilityController::new(TrainingInstabilityPolicy::new(
            vec![
                TrainingInstabilityRule {
                    signal: crate::TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs,
                    max_value: 500.0,
                    action: TrainingOperationalAction::Quarantine,
                },
                TrainingInstabilityRule {
                    signal: crate::TrainingInstabilitySignalKind::EnvironmentFailureRateBps,
                    max_value: 100.0,
                    action: TrainingOperationalAction::Halt,
                },
            ],
            vec![TrainingRiskyOptimizationRule {
                optimization: crate::TrainingRiskyOptimization::AsyncCheckpointOverlap,
                action: TrainingOperationalAction::Quarantine,
            }],
        ))
    }
}
