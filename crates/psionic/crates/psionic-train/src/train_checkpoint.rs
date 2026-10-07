use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    load_psionic_train_grouped_stage_recovery_receipt, PsionActualPretrainingAutoResumeReceipt,
    PsionActualPretrainingCheckpointBackupReceipt, PsionActualPretrainingCheckpointManifest,
    PsionActualPretrainingCheckpointPointer, PsionActualPretrainingCurrentRunStatus,
    PsionicTrainCheckpointHandoffReceipt, PsionicTrainGroupedReplicaStageAssignment,
    PsionicTrainGroupedReplicaStageRecoveryReceipt, PsionicTrainOperation, PsionicTrainRole,
    PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_RELATIVE_PATH, PSION_ACTUAL_PRETRAINING_LANE_ID,
};

/// Stable schema version for the retained machine-readable checkpoint surface.
pub const PSIONIC_TRAIN_CHECKPOINT_SURFACE_SCHEMA_VERSION: &str =
    "psionic.train.checkpoint_surface.v1";

/// Stable schema version for the retained generic machine checkpoint pointer.
pub const PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION: &str =
    "psionic.train.checkpoint_pointer.v1";

/// Stable schema version for the retained generic machine checkpoint manifest.
pub const PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.train.checkpoint_manifest.v1";

/// Generic machine checkpoint pointer shared by bounded non-actual-pretraining lanes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCheckpointPointer {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable window identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Stable assignment identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    /// Stable grouped stage assignment when the checkpoint belongs to one grouped replica stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Pointer state for the latest checkpoint.
    pub pointer_state: String,
    /// Latest checkpoint label.
    pub checkpoint_label: String,
    /// Latest accepted optimizer step.
    pub optimizer_step: u64,
    /// Latest checkpoint ref.
    pub checkpoint_ref: String,
    /// Relative path to the retained checkpoint manifest.
    pub checkpoint_manifest_relative_path: String,
    /// Short detail.
    pub detail: String,
}

/// Generic machine checkpoint manifest shared by bounded non-actual-pretraining lanes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCheckpointManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable window identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Stable assignment identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    /// Stable grouped stage assignment when the checkpoint belongs to one grouped replica stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Latest checkpoint label.
    pub checkpoint_label: String,
    /// Latest accepted optimizer step.
    pub optimizer_step: u64,
    /// Latest checkpoint ref.
    pub checkpoint_ref: String,
    /// Relative path where this manifest is retained.
    pub relative_manifest_path: String,
    /// Stable checkpoint-object digest.
    pub checkpoint_object_digest: String,
    /// Stable checkpoint byte count.
    pub checkpoint_total_bytes: u64,
    /// Stable canonical manifest digest.
    pub manifest_digest: String,
}

/// Shared retained artifact paths for the latest local checkpoint state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCheckpointArtifactPaths {
    /// Absolute path to the canonical latest-checkpoint pointer when present.
    pub checkpoint_pointer_path: Option<String>,
    /// Absolute path to the canonical latest checkpoint manifest when present.
    pub checkpoint_manifest_path: Option<String>,
    /// Absolute path to the latest durable-backup receipt when present.
    pub checkpoint_backup_receipt_path: Option<String>,
    /// Absolute path to the latest backup pointer copy when present.
    pub checkpoint_backup_pointer_path: Option<String>,
    /// Absolute path to the latest backup manifest copy when present.
    pub checkpoint_backup_manifest_path: Option<String>,
    /// Absolute path to the latest auto-resume receipt when present.
    pub auto_resume_receipt_path: Option<String>,
    /// Absolute path to the latest grouped-stage recovery receipt when present.
    pub grouped_stage_recovery_receipt_path: Option<String>,
    /// Absolute path to the latest peer checkpoint-handoff receipt when present.
    pub peer_checkpoint_handoff_receipt_path: Option<String>,
    /// Absolute path to the latest retained checkpoint failure drill when present.
    pub checkpoint_failure_drill_path: Option<String>,
}

/// Machine-readable summary of the retained checkpoint and recovery state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCheckpointSurface {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Runtime role that produced the latest state transition.
    pub role: PsionicTrainRole,
    /// Runtime operation that produced the latest state transition.
    pub operation: PsionicTrainOperation,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable window identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Stable assignment identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    /// Stable grouped stage assignment when the checkpoint belongs to one grouped replica stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Absolute local run root.
    pub run_root: String,
    /// Latest retained actual-lane phase when present.
    pub current_phase: Option<String>,
    /// Pointer state for the latest checkpoint pointer when present.
    pub pointer_state: Option<String>,
    /// Latest checkpoint label when present.
    pub checkpoint_label: Option<String>,
    /// Latest checkpoint step when present.
    pub optimizer_step: Option<u64>,
    /// Latest checkpoint ref when present.
    pub checkpoint_ref: Option<String>,
    /// Stable checkpoint-manifest digest when present.
    pub checkpoint_manifest_digest: Option<String>,
    /// Stable checkpoint-object digest when present.
    pub checkpoint_object_digest: Option<String>,
    /// Stable checkpoint byte count when present.
    pub checkpoint_total_bytes: Option<u64>,
    /// Backup state when the durable-backup receipt exists.
    pub backup_state: Option<String>,
    /// Upload outcome when the durable-backup receipt exists.
    pub upload_outcome: Option<String>,
    /// Upload failure reason when the durable-backup receipt was refused.
    pub upload_failure_reason: Option<String>,
    /// Auto-resume resolution state when the recovery receipt exists.
    pub recovery_resolution_state: Option<String>,
    /// Auto-resume source kind when the recovery receipt exists.
    pub recovery_source_kind: Option<String>,
    /// Whether the recovery receipt restored the primary pointer.
    pub restored_primary_pointer: Option<bool>,
    /// Shared retained artifact paths for the latest checkpoint family.
    pub artifacts: PsionicTrainCheckpointArtifactPaths,
}

impl PsionicTrainCheckpointPointer {
    /// Validates the retained generic machine checkpoint pointer.
    pub fn validate(&self) -> Result<(), PsionicTrainCheckpointSurfaceError> {
        require_nonempty(
            self.schema_version.as_str(),
            "checkpoint_pointer.schema_version",
        )?;
        if self.schema_version != PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION {
            return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from("checkpoint_pointer"),
                detail: format!(
                    "checkpoint pointer schema version `{}` is not admitted",
                    self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "checkpoint_pointer.lane_id")?;
        require_nonempty(self.run_id.as_str(), "checkpoint_pointer.run_id")?;
        validate_grouped_checkpoint_scope(
            self.window_id.as_deref(),
            self.assignment_id.as_deref(),
            self.grouped_stage_assignment.as_ref(),
            "checkpoint_pointer",
        )?;
        match self.pointer_state.as_str() {
            "accepted" | "accepted_primary" => {}
            other => {
                return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                    path: String::from("checkpoint_pointer"),
                    detail: format!(
                        "checkpoint pointer state `{other}` is not admitted for the generic machine surface"
                    ),
                });
            }
        }
        require_nonempty(
            self.checkpoint_label.as_str(),
            "checkpoint_pointer.checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from("checkpoint_pointer"),
                detail: String::from("checkpoint pointer optimizer_step must be non-zero"),
            });
        }
        require_nonempty(
            self.checkpoint_ref.as_str(),
            "checkpoint_pointer.checkpoint_ref",
        )?;
        require_nonempty(
            self.checkpoint_manifest_relative_path.as_str(),
            "checkpoint_pointer.checkpoint_manifest_relative_path",
        )?;
        require_nonempty(self.detail.as_str(), "checkpoint_pointer.detail")?;
        Ok(())
    }
}

impl PsionicTrainCheckpointManifest {
    /// Computes the stable canonical digest for the manifest.
    pub fn stable_manifest_digest(&self) -> String {
        let mut copy = self.clone();
        copy.manifest_digest.clear();
        let encoded =
            serde_json::to_vec(&copy).expect("generic checkpoint manifest should serialize");
        let mut digest = Sha256::new();
        digest.update(b"psionic_train_checkpoint_manifest|");
        digest.update(&encoded);
        format!("{:x}", digest.finalize())
    }

    /// Validates the retained generic machine checkpoint manifest.
    pub fn validate(&self) -> Result<(), PsionicTrainCheckpointSurfaceError> {
        require_nonempty(
            self.schema_version.as_str(),
            "checkpoint_manifest.schema_version",
        )?;
        if self.schema_version != PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION {
            return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from("checkpoint_manifest"),
                detail: format!(
                    "checkpoint manifest schema version `{}` is not admitted",
                    self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "checkpoint_manifest.lane_id")?;
        require_nonempty(self.run_id.as_str(), "checkpoint_manifest.run_id")?;
        validate_grouped_checkpoint_scope(
            self.window_id.as_deref(),
            self.assignment_id.as_deref(),
            self.grouped_stage_assignment.as_ref(),
            "checkpoint_manifest",
        )?;
        require_nonempty(
            self.checkpoint_label.as_str(),
            "checkpoint_manifest.checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from("checkpoint_manifest"),
                detail: String::from("checkpoint manifest optimizer_step must be non-zero"),
            });
        }
        require_nonempty(
            self.checkpoint_ref.as_str(),
            "checkpoint_manifest.checkpoint_ref",
        )?;
        require_nonempty(
            self.relative_manifest_path.as_str(),
            "checkpoint_manifest.relative_manifest_path",
        )?;
        require_nonempty(
            self.checkpoint_object_digest.as_str(),
            "checkpoint_manifest.checkpoint_object_digest",
        )?;
        if self.checkpoint_total_bytes == 0 {
            return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from("checkpoint_manifest"),
                detail: String::from("checkpoint manifest checkpoint_total_bytes must be non-zero"),
            });
        }
        require_nonempty(
            self.manifest_digest.as_str(),
            "checkpoint_manifest.manifest_digest",
        )?;
        let expected_digest = self.stable_manifest_digest();
        if self.manifest_digest != expected_digest {
            return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from("checkpoint_manifest"),
                detail: String::from("checkpoint manifest digest drifted from canonical form"),
            });
        }
        Ok(())
    }
}

/// Errors while deriving the retained checkpoint surface from a run root.
#[derive(Debug, Error)]
pub enum PsionicTrainCheckpointSurfaceError {
    #[error("failed to read retained checkpoint artifact `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to parse retained checkpoint artifact `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("retained checkpoint artifact `{path}` is invalid: {detail}")]
    Invalid { path: String, detail: String },
}

/// Loads the latest retained checkpoint surface from a run root when checkpoint state exists.
pub fn inspect_psionic_train_checkpoint_surface(
    run_root: &Path,
    role: PsionicTrainRole,
    operation: PsionicTrainOperation,
) -> Result<Option<PsionicTrainCheckpointSurface>, PsionicTrainCheckpointSurfaceError> {
    let retained_surface_path = run_root.join("status/checkpoint_surface.json");
    let retained_surface: Option<PsionicTrainCheckpointSurface> =
        load_optional_json(retained_surface_path.as_path())?;

    let current_status_path = run_root.join("status/current_run_status.json");
    let checkpoint_pointer_path =
        run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    let checkpoint_backup_receipt_path =
        run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json");
    let auto_resume_receipt_path = run_root.join("checkpoints/auto_resume_receipt.json");
    let grouped_stage_recovery_receipt_path =
        run_root.join(PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_RELATIVE_PATH);
    let peer_checkpoint_handoff_receipt_path =
        run_root.join("status/peer_checkpoint_handoff_receipt.json");

    if checkpoint_pointer_path.is_file()
        && checkpoint_schema_version(checkpoint_pointer_path.as_path())?
            == Some(String::from(
                PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION,
            ))
    {
        return load_generic_checkpoint_surface(
            run_root,
            role,
            operation,
            checkpoint_pointer_path.as_path(),
            peer_checkpoint_handoff_receipt_path.as_path(),
        )
        .map(Some);
    }

    let current_status: Option<PsionActualPretrainingCurrentRunStatus> =
        load_optional_json(current_status_path.as_path())?;
    if let Some(current_status) = &current_status {
        current_status
            .validate()
            .map_err(|error| PsionicTrainCheckpointSurfaceError::Invalid {
                path: current_status_path.display().to_string(),
                detail: error.to_string(),
            })?;
    }

    let checkpoint_pointer: Option<PsionActualPretrainingCheckpointPointer> =
        load_optional_json(checkpoint_pointer_path.as_path())?;
    if let Some(checkpoint_pointer) = &checkpoint_pointer {
        checkpoint_pointer.validate().map_err(|error| {
            PsionicTrainCheckpointSurfaceError::Invalid {
                path: checkpoint_pointer_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }

    let checkpoint_manifest_path = checkpoint_pointer
        .as_ref()
        .and_then(|value| value.checkpoint_manifest_relative_path.as_ref())
        .map(|value| run_root.join(value));
    let checkpoint_manifest: Option<PsionActualPretrainingCheckpointManifest> =
        match checkpoint_manifest_path.as_ref() {
            Some(path) => {
                let manifest: PsionActualPretrainingCheckpointManifest = load_json(path.as_path())?;
                manifest.validate().map_err(|error| {
                    PsionicTrainCheckpointSurfaceError::Invalid {
                        path: path.display().to_string(),
                        detail: error.to_string(),
                    }
                })?;
                Some(manifest)
            }
            None => None,
        };

    let checkpoint_backup_receipt: Option<PsionActualPretrainingCheckpointBackupReceipt> =
        load_optional_json(checkpoint_backup_receipt_path.as_path())?;
    if let Some(checkpoint_backup_receipt) = &checkpoint_backup_receipt {
        checkpoint_backup_receipt.validate().map_err(|error| {
            PsionicTrainCheckpointSurfaceError::Invalid {
                path: checkpoint_backup_receipt_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }

    let checkpoint_backup_pointer_path = checkpoint_backup_receipt
        .as_ref()
        .map(|receipt| run_root.join(receipt.backup_pointer.path.as_str()));
    let checkpoint_backup_manifest_path = checkpoint_backup_receipt
        .as_ref()
        .map(|receipt| run_root.join(receipt.backup_checkpoint_manifest.path.as_str()));
    let checkpoint_backup_manifest: Option<PsionActualPretrainingCheckpointManifest> =
        match checkpoint_backup_manifest_path.as_ref() {
            Some(path) if path.is_file() => {
                let manifest: PsionActualPretrainingCheckpointManifest = load_json(path.as_path())?;
                manifest.validate().map_err(|error| {
                    PsionicTrainCheckpointSurfaceError::Invalid {
                        path: path.display().to_string(),
                        detail: error.to_string(),
                    }
                })?;
                Some(manifest)
            }
            _ => None,
        };

    let auto_resume_receipt: Option<PsionActualPretrainingAutoResumeReceipt> =
        load_optional_json(auto_resume_receipt_path.as_path())?;
    if let Some(auto_resume_receipt) = &auto_resume_receipt {
        auto_resume_receipt.validate().map_err(|error| {
            PsionicTrainCheckpointSurfaceError::Invalid {
                path: auto_resume_receipt_path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }
    let grouped_stage_recovery_receipt: Option<PsionicTrainGroupedReplicaStageRecoveryReceipt> =
        if grouped_stage_recovery_receipt_path.is_file() {
            Some(
                load_psionic_train_grouped_stage_recovery_receipt(
                    grouped_stage_recovery_receipt_path.as_path(),
                )
                .map_err(|error| PsionicTrainCheckpointSurfaceError::Invalid {
                    path: grouped_stage_recovery_receipt_path.display().to_string(),
                    detail: error.to_string(),
                })?,
            )
        } else {
            None
        };
    let peer_checkpoint_handoff_receipt: Option<PsionicTrainCheckpointHandoffReceipt> =
        load_optional_json(peer_checkpoint_handoff_receipt_path.as_path())?;
    if let Some(peer_checkpoint_handoff_receipt) = &peer_checkpoint_handoff_receipt {
        peer_checkpoint_handoff_receipt
            .validate()
            .map_err(|error| PsionicTrainCheckpointSurfaceError::Invalid {
                path: peer_checkpoint_handoff_receipt_path.display().to_string(),
                detail: error.to_string(),
            })?;
    }

    let checkpoint_failure_drill_path = checkpoint_failure_drill_path(run_root, operation);

    if current_status.is_none()
        && checkpoint_pointer.is_none()
        && checkpoint_backup_receipt.is_none()
        && auto_resume_receipt.is_none()
        && grouped_stage_recovery_receipt.is_none()
        && peer_checkpoint_handoff_receipt.is_none()
    {
        return Ok(retained_surface);
    }

    let run_id = current_status
        .as_ref()
        .map(|value| value.run_id.clone())
        .or_else(|| {
            checkpoint_pointer
                .as_ref()
                .map(|value| value.run_id.clone())
        })
        .or_else(|| {
            checkpoint_backup_receipt
                .as_ref()
                .map(|value| value.run_id.clone())
        })
        .or_else(|| {
            auto_resume_receipt
                .as_ref()
                .map(|value| value.run_id.clone())
        })
        .unwrap_or_else(|| String::from("unknown_run"));

    let grouped_window_id = grouped_stage_recovery_receipt
        .as_ref()
        .map(|value| value.window_id.clone());
    let grouped_assignment_id = grouped_stage_recovery_receipt
        .as_ref()
        .map(|value| value.assignment_id.clone());
    let grouped_stage_assignment = grouped_stage_recovery_receipt
        .as_ref()
        .map(|value| value.grouped_stage_assignment.clone());

    let checkpoint_label = checkpoint_pointer
        .as_ref()
        .map(|value| value.checkpoint_label.clone())
        .or_else(|| {
            checkpoint_backup_receipt
                .as_ref()
                .map(|value| value.checkpoint_label.clone())
        })
        .or_else(|| {
            auto_resume_receipt
                .as_ref()
                .and_then(|value| value.chosen_checkpoint_label.clone())
        });

    let optimizer_step = checkpoint_pointer
        .as_ref()
        .map(|value| value.optimizer_step)
        .filter(|value| *value > 0)
        .or_else(|| {
            checkpoint_backup_receipt
                .as_ref()
                .map(|value| value.optimizer_step)
        })
        .or_else(|| {
            auto_resume_receipt
                .as_ref()
                .and_then(|value| value.chosen_optimizer_step)
        });

    let checkpoint_ref = checkpoint_pointer
        .as_ref()
        .and_then(|value| value.checkpoint_ref.clone())
        .or_else(|| {
            checkpoint_backup_receipt
                .as_ref()
                .map(|value| value.checkpoint_ref.clone())
        })
        .or_else(|| {
            auto_resume_receipt
                .as_ref()
                .and_then(|value| value.chosen_checkpoint_ref.clone())
        });

    Ok(Some(PsionicTrainCheckpointSurface {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_SURFACE_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        role,
        operation,
        run_id,
        window_id: grouped_window_id,
        assignment_id: grouped_assignment_id,
        grouped_stage_assignment,
        run_root: run_root.display().to_string(),
        current_phase: current_status.as_ref().map(|value| value.phase.clone()),
        pointer_state: checkpoint_pointer
            .as_ref()
            .map(|value| value.pointer_state.clone()),
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        checkpoint_manifest_digest: checkpoint_manifest
            .as_ref()
            .or(checkpoint_backup_manifest.as_ref())
            .map(|value| value.manifest_digest.clone()),
        checkpoint_object_digest: checkpoint_manifest
            .as_ref()
            .or(checkpoint_backup_manifest.as_ref())
            .map(|value| value.checkpoint_object_digest.clone()),
        checkpoint_total_bytes: checkpoint_manifest
            .as_ref()
            .or(checkpoint_backup_manifest.as_ref())
            .map(|value| value.checkpoint_total_bytes),
        backup_state: checkpoint_backup_receipt
            .as_ref()
            .map(|value| value.backup_state.clone()),
        upload_outcome: checkpoint_backup_receipt
            .as_ref()
            .map(|value| value.upload_outcome.clone()),
        upload_failure_reason: checkpoint_backup_receipt
            .as_ref()
            .and_then(|value| value.upload_failure_reason.clone()),
        recovery_resolution_state: auto_resume_receipt
            .as_ref()
            .map(|value| value.resolution_state.clone()),
        recovery_source_kind: auto_resume_receipt
            .as_ref()
            .map(|value| value.resume_source_kind.clone()),
        restored_primary_pointer: auto_resume_receipt
            .as_ref()
            .map(|value| value.restored_primary_pointer),
        artifacts: PsionicTrainCheckpointArtifactPaths {
            checkpoint_pointer_path: checkpoint_pointer_path
                .is_file()
                .then(|| checkpoint_pointer_path.display().to_string()),
            checkpoint_manifest_path: checkpoint_manifest_path
                .as_ref()
                .filter(|value| value.is_file())
                .map(|value| value.display().to_string())
                .or_else(|| {
                    checkpoint_backup_manifest_path
                        .as_ref()
                        .filter(|value| value.is_file())
                        .map(|value| value.display().to_string())
                }),
            checkpoint_backup_receipt_path: checkpoint_backup_receipt_path
                .is_file()
                .then(|| checkpoint_backup_receipt_path.display().to_string()),
            checkpoint_backup_pointer_path: checkpoint_backup_pointer_path
                .as_ref()
                .filter(|value| value.is_file())
                .map(|value| value.display().to_string()),
            checkpoint_backup_manifest_path: checkpoint_backup_manifest_path
                .as_ref()
                .filter(|value| value.is_file())
                .map(|value| value.display().to_string()),
            auto_resume_receipt_path: auto_resume_receipt_path
                .is_file()
                .then(|| auto_resume_receipt_path.display().to_string()),
            grouped_stage_recovery_receipt_path: grouped_stage_recovery_receipt_path
                .is_file()
                .then(|| grouped_stage_recovery_receipt_path.display().to_string()),
            peer_checkpoint_handoff_receipt_path: peer_checkpoint_handoff_receipt_path
                .is_file()
                .then(|| peer_checkpoint_handoff_receipt_path.display().to_string()),
            checkpoint_failure_drill_path: checkpoint_failure_drill_path
                .as_ref()
                .filter(|value| value.is_file())
                .map(|value| value.display().to_string()),
        },
    }))
}

fn load_generic_checkpoint_surface(
    run_root: &Path,
    role: PsionicTrainRole,
    operation: PsionicTrainOperation,
    checkpoint_pointer_path: &Path,
    peer_checkpoint_handoff_receipt_path: &Path,
) -> Result<PsionicTrainCheckpointSurface, PsionicTrainCheckpointSurfaceError> {
    let checkpoint_pointer: PsionicTrainCheckpointPointer = load_json(checkpoint_pointer_path)?;
    checkpoint_pointer.validate()?;

    let checkpoint_manifest_path = run_root.join(
        checkpoint_pointer
            .checkpoint_manifest_relative_path
            .as_str(),
    );
    let checkpoint_manifest: PsionicTrainCheckpointManifest =
        load_json(checkpoint_manifest_path.as_path())?;
    checkpoint_manifest.validate()?;
    if checkpoint_manifest.lane_id != checkpoint_pointer.lane_id
        || checkpoint_manifest.run_id != checkpoint_pointer.run_id
        || checkpoint_manifest.window_id != checkpoint_pointer.window_id
        || checkpoint_manifest.assignment_id != checkpoint_pointer.assignment_id
        || checkpoint_manifest.grouped_stage_assignment
            != checkpoint_pointer.grouped_stage_assignment
        || checkpoint_manifest.checkpoint_label != checkpoint_pointer.checkpoint_label
        || checkpoint_manifest.optimizer_step != checkpoint_pointer.optimizer_step
        || checkpoint_manifest.checkpoint_ref != checkpoint_pointer.checkpoint_ref
        || checkpoint_manifest.relative_manifest_path
            != checkpoint_pointer.checkpoint_manifest_relative_path
    {
        return Err(PsionicTrainCheckpointSurfaceError::Invalid {
            path: checkpoint_manifest_path.display().to_string(),
            detail: String::from(
                "generic checkpoint manifest drifted from the retained generic checkpoint pointer",
            ),
        });
    }

    let grouped_stage_recovery_receipt_path =
        run_root.join(PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_RELATIVE_PATH);
    let grouped_stage_recovery_receipt: Option<PsionicTrainGroupedReplicaStageRecoveryReceipt> =
        if grouped_stage_recovery_receipt_path.is_file() {
            let receipt = load_psionic_train_grouped_stage_recovery_receipt(
                grouped_stage_recovery_receipt_path.as_path(),
            )
            .map_err(|error| PsionicTrainCheckpointSurfaceError::Invalid {
                path: grouped_stage_recovery_receipt_path.display().to_string(),
                detail: error.to_string(),
            })?;
            if receipt.lane_id != checkpoint_pointer.lane_id
                || receipt.run_id != checkpoint_pointer.run_id
                || Some(receipt.window_id.as_str()) != checkpoint_pointer.window_id.as_deref()
                || Some(receipt.assignment_id.as_str())
                    != checkpoint_pointer.assignment_id.as_deref()
                || receipt.grouped_stage_assignment != checkpoint_pointer.grouped_stage_assignment.clone().ok_or_else(|| PsionicTrainCheckpointSurfaceError::Invalid {
                    path: grouped_stage_recovery_receipt_path.display().to_string(),
                    detail: String::from(
                        "grouped stage recovery receipt was retained without one grouped stage checkpoint pointer",
                    ),
                })?
                || receipt.checkpoint_manifest_digest != checkpoint_manifest.manifest_digest
                || receipt.checkpoint_object_digest != checkpoint_manifest.checkpoint_object_digest
                || receipt
                    .checkpoint_pointer
                    .materialized_path
                    .as_deref()
                    .is_some_and(|value| value != checkpoint_pointer_path.display().to_string())
                || receipt
                    .checkpoint_manifest
                    .materialized_path
                    .as_deref()
                    .is_some_and(|value| value != checkpoint_manifest_path.display().to_string())
            {
                return Err(PsionicTrainCheckpointSurfaceError::Invalid {
                    path: grouped_stage_recovery_receipt_path.display().to_string(),
                    detail: String::from(
                        "grouped stage recovery receipt drifted from the retained generic checkpoint surface",
                    ),
                });
            }
            Some(receipt)
        } else {
            None
        };

    let peer_checkpoint_handoff_receipt: Option<PsionicTrainCheckpointHandoffReceipt> =
        load_optional_json(peer_checkpoint_handoff_receipt_path)?;
    if let Some(peer_checkpoint_handoff_receipt) = &peer_checkpoint_handoff_receipt {
        peer_checkpoint_handoff_receipt
            .validate()
            .map_err(|error| PsionicTrainCheckpointSurfaceError::Invalid {
                path: peer_checkpoint_handoff_receipt_path.display().to_string(),
                detail: error.to_string(),
            })?;
    }

    Ok(PsionicTrainCheckpointSurface {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_SURFACE_SCHEMA_VERSION),
        lane_id: checkpoint_pointer.lane_id,
        role,
        operation,
        run_id: checkpoint_pointer.run_id,
        window_id: checkpoint_pointer.window_id.clone(),
        assignment_id: checkpoint_pointer.assignment_id.clone(),
        grouped_stage_assignment: checkpoint_pointer.grouped_stage_assignment.clone(),
        run_root: run_root.display().to_string(),
        current_phase: None,
        pointer_state: Some(checkpoint_pointer.pointer_state),
        checkpoint_label: Some(checkpoint_pointer.checkpoint_label),
        optimizer_step: Some(checkpoint_pointer.optimizer_step),
        checkpoint_ref: Some(checkpoint_pointer.checkpoint_ref),
        checkpoint_manifest_digest: Some(checkpoint_manifest.manifest_digest),
        checkpoint_object_digest: Some(checkpoint_manifest.checkpoint_object_digest),
        checkpoint_total_bytes: Some(checkpoint_manifest.checkpoint_total_bytes),
        backup_state: None,
        upload_outcome: Some(String::from("succeeded")),
        upload_failure_reason: None,
        recovery_resolution_state: grouped_stage_recovery_receipt
            .as_ref()
            .map(|value| value.resolution_state.clone()),
        recovery_source_kind: grouped_stage_recovery_receipt.as_ref().map(|value| {
            match value.recovery_source_kind {
                crate::PsionicTrainGroupedReplicaRecoverySourceKind::RetainedCheckpoint => {
                    String::from("retained_grouped_stage_checkpoint")
                }
                crate::PsionicTrainGroupedReplicaRecoverySourceKind::PeerCheckpointHandoff => {
                    String::from("peer_checkpoint_handoff")
                }
            }
        }),
        restored_primary_pointer: Some(
            grouped_stage_recovery_receipt
                .as_ref()
                .map(|value| value.restored_primary_pointer)
                .unwrap_or(false),
        ),
        artifacts: PsionicTrainCheckpointArtifactPaths {
            checkpoint_pointer_path: Some(checkpoint_pointer_path.display().to_string()),
            checkpoint_manifest_path: Some(checkpoint_manifest_path.display().to_string()),
            checkpoint_backup_receipt_path: None,
            checkpoint_backup_pointer_path: None,
            checkpoint_backup_manifest_path: None,
            auto_resume_receipt_path: None,
            grouped_stage_recovery_receipt_path: grouped_stage_recovery_receipt
                .map(|_| grouped_stage_recovery_receipt_path.display().to_string()),
            peer_checkpoint_handoff_receipt_path: peer_checkpoint_handoff_receipt
                .map(|_| peer_checkpoint_handoff_receipt_path.display().to_string()),
            checkpoint_failure_drill_path: checkpoint_failure_drill_path(run_root, operation)
                .map(|path| path.display().to_string()),
        },
    })
}

fn checkpoint_failure_drill_path(
    run_root: &Path,
    operation: PsionicTrainOperation,
) -> Option<std::path::PathBuf> {
    let candidates: &[&str] = match operation {
        PsionicTrainOperation::Backup => &[
            "checkpoints/failures/failed_upload_drill.json",
            "checkpoints/failures/corrupt_pointer_drill.json",
            "checkpoints/failures/stale_pointer_drill.json",
        ],
        PsionicTrainOperation::Resume => &[
            "checkpoints/failures/corrupt_pointer_drill.json",
            "checkpoints/failures/stale_pointer_drill.json",
            "checkpoints/failures/failed_upload_drill.json",
        ],
        _ => &[
            "checkpoints/failures/failed_upload_drill.json",
            "checkpoints/failures/corrupt_pointer_drill.json",
            "checkpoints/failures/stale_pointer_drill.json",
        ],
    };
    candidates
        .iter()
        .map(|value| run_root.join(value))
        .find(|value| value.is_file())
}

fn validate_grouped_checkpoint_scope(
    window_id: Option<&str>,
    assignment_id: Option<&str>,
    grouped_stage_assignment: Option<&PsionicTrainGroupedReplicaStageAssignment>,
    field_prefix: &str,
) -> Result<(), PsionicTrainCheckpointSurfaceError> {
    validate_optional_nonempty(window_id, format!("{field_prefix}.window_id").as_str())?;
    validate_optional_nonempty(
        assignment_id,
        format!("{field_prefix}.assignment_id").as_str(),
    )?;
    if window_id.is_some() || assignment_id.is_some() {
        require_nonempty_option(window_id, format!("{field_prefix}.window_id").as_str())?;
        require_nonempty_option(
            assignment_id,
            format!("{field_prefix}.assignment_id").as_str(),
        )?;
    }
    if let Some(grouped_stage_assignment) = grouped_stage_assignment {
        grouped_stage_assignment
            .validate(format!("{field_prefix}.grouped_stage_assignment").as_str())
            .map_err(|error| PsionicTrainCheckpointSurfaceError::Invalid {
                path: String::from(field_prefix),
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, PsionicTrainCheckpointSurfaceError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainCheckpointSurfaceError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionicTrainCheckpointSurfaceError::Parse {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn require_nonempty_option(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionicTrainCheckpointSurfaceError> {
    let value = value.ok_or_else(|| PsionicTrainCheckpointSurfaceError::Invalid {
        path: String::from(field),
        detail: format!("{field} must not be empty"),
    })?;
    require_nonempty(value, field)
}

fn validate_optional_nonempty(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionicTrainCheckpointSurfaceError> {
    if let Some(value) = value {
        require_nonempty(value, field)?;
    }
    Ok(())
}

fn load_optional_json<T: DeserializeOwned>(
    path: &Path,
) -> Result<Option<T>, PsionicTrainCheckpointSurfaceError> {
    if !path.is_file() {
        return Ok(None);
    }
    load_json(path).map(Some)
}

fn checkpoint_schema_version(
    path: &Path,
) -> Result<Option<String>, PsionicTrainCheckpointSurfaceError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| PsionicTrainCheckpointSurfaceError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        PsionicTrainCheckpointSurfaceError::Parse {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    Ok(value
        .get("schema_version")
        .and_then(|value| value.as_str())
        .map(String::from))
}

fn require_nonempty(value: &str, field: &str) -> Result<(), PsionicTrainCheckpointSurfaceError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainCheckpointSurfaceError::Invalid {
            path: String::from(field),
            detail: String::from("value must not be empty"),
        });
    }
    Ok(())
}
