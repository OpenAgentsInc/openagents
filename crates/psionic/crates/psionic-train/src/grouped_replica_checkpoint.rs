use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_psionic_train_artifact_binding_from_path, PsionicTrainArtifactBinding,
    PsionicTrainCheckpointSurface, PsionicTrainGroupedReplicaStageAssignment,
    PsionicTrainInvocationManifest,
};

pub const PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.train.grouped_replica_stage_recovery_receipt.v1";
pub const PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_RELATIVE_PATH: &str =
    "checkpoints/grouped_stage_recovery_receipt.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainGroupedReplicaRecoverySourceKind {
    RetainedCheckpoint,
    PeerCheckpointHandoff,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainGroupedReplicaStageRecoveryReceipt {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub grouped_stage_assignment: PsionicTrainGroupedReplicaStageAssignment,
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub checkpoint_manifest_digest: String,
    pub checkpoint_object_digest: String,
    pub recovery_source_kind: PsionicTrainGroupedReplicaRecoverySourceKind,
    pub resolution_state: String,
    pub restored_primary_pointer: bool,
    pub checkpoint_pointer: PsionicTrainArtifactBinding,
    pub checkpoint_manifest: PsionicTrainArtifactBinding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_handoff_receipt: Option<PsionicTrainArtifactBinding>,
    pub detail: String,
    pub recovery_receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainGroupedReplicaStageRecoveryReceiptArtifacts {
    pub grouped_stage_recovery_receipt_path: String,
    pub grouped_stage_recovery_receipt_digest: String,
}

#[derive(Debug, Error)]
pub enum PsionicTrainGroupedReplicaCheckpointError {
    #[error("grouped-replica checkpoint is unavailable: {detail}")]
    MissingCheckpoint { detail: String },
    #[error("grouped-replica checkpoint is stale: {detail}")]
    StaleAssignment { detail: String },
    #[error("grouped-replica checkpoint is invalid: {detail}")]
    Invalid { detail: String },
    #[error("failed to read `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to parse `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("failed to write `{path}`: {detail}")]
    Write { path: String, detail: String },
}

impl PsionicTrainGroupedReplicaStageRecoveryReceipt {
    #[must_use]
    pub fn stable_recovery_receipt_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.checkpoint_pointer = digest_basis.checkpoint_pointer.canonicalize_for_digest();
        digest_basis.checkpoint_manifest =
            digest_basis.checkpoint_manifest.canonicalize_for_digest();
        digest_basis.checkpoint_handoff_receipt = digest_basis
            .checkpoint_handoff_receipt
            .map(|value| value.canonicalize_for_digest());
        digest_basis.recovery_receipt_digest.clear();
        stable_digest(
            b"psionic_train_grouped_stage_recovery_receipt|",
            &digest_basis,
        )
    }

    pub fn validate(&self) -> Result<(), PsionicTrainGroupedReplicaCheckpointError> {
        if self.schema_version != PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_SCHEMA_VERSION {
            return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                detail: format!(
                    "grouped stage recovery receipt schema version must stay `{}` but was `{}`",
                    PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_SCHEMA_VERSION,
                    self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "grouped stage recovery lane_id")?;
        require_nonempty(self.run_id.as_str(), "grouped stage recovery run_id")?;
        require_nonempty(self.window_id.as_str(), "grouped stage recovery window_id")?;
        require_nonempty(
            self.assignment_id.as_str(),
            "grouped stage recovery assignment_id",
        )?;
        self.grouped_stage_assignment
            .validate("grouped_stage_recovery_receipt.grouped_stage_assignment")
            .map_err(|error| PsionicTrainGroupedReplicaCheckpointError::Invalid {
                detail: error.to_string(),
            })?;
        require_nonempty(
            self.checkpoint_label.as_str(),
            "grouped stage recovery checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                detail: String::from("grouped stage recovery optimizer_step must be non-zero"),
            });
        }
        require_nonempty(
            self.checkpoint_ref.as_str(),
            "grouped stage recovery checkpoint_ref",
        )?;
        require_nonempty(
            self.checkpoint_manifest_digest.as_str(),
            "grouped stage recovery checkpoint_manifest_digest",
        )?;
        require_nonempty(
            self.checkpoint_object_digest.as_str(),
            "grouped stage recovery checkpoint_object_digest",
        )?;
        self.checkpoint_pointer
            .validate("grouped_stage_recovery_receipt.checkpoint_pointer")
            .map_err(|detail| PsionicTrainGroupedReplicaCheckpointError::Invalid { detail })?;
        self.checkpoint_manifest
            .validate("grouped_stage_recovery_receipt.checkpoint_manifest")
            .map_err(|detail| PsionicTrainGroupedReplicaCheckpointError::Invalid { detail })?;
        require_nonempty(self.detail.as_str(), "grouped stage recovery detail")?;
        match self.recovery_source_kind {
            PsionicTrainGroupedReplicaRecoverySourceKind::RetainedCheckpoint => {
                if self.resolution_state != "accepted_grouped_stage_checkpoint" {
                    return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                        detail: String::from(
                            "retained grouped stage recovery must resolve as accepted_grouped_stage_checkpoint",
                        ),
                    });
                }
                if self.restored_primary_pointer {
                    return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                        detail: String::from(
                            "retained grouped stage recovery must not claim restored_primary_pointer",
                        ),
                    });
                }
                if self.checkpoint_handoff_receipt.is_some() {
                    return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                        detail: String::from(
                            "retained grouped stage recovery must not carry checkpoint_handoff_receipt",
                        ),
                    });
                }
            }
            PsionicTrainGroupedReplicaRecoverySourceKind::PeerCheckpointHandoff => {
                if self.resolution_state != "accepted_peer_handoff_checkpoint" {
                    return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                        detail: String::from(
                            "peer handoff grouped stage recovery must resolve as accepted_peer_handoff_checkpoint",
                        ),
                    });
                }
                if !self.restored_primary_pointer {
                    return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                        detail: String::from(
                            "peer handoff grouped stage recovery must claim restored_primary_pointer",
                        ),
                    });
                }
                let Some(checkpoint_handoff_receipt) = self.checkpoint_handoff_receipt.as_ref()
                else {
                    return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                        detail: String::from(
                            "grouped stage recovery checkpoint_handoff_receipt must not be empty",
                        ),
                    });
                };
                checkpoint_handoff_receipt
                    .validate("grouped_stage_recovery_receipt.checkpoint_handoff_receipt")
                    .map_err(
                        |detail| PsionicTrainGroupedReplicaCheckpointError::Invalid { detail },
                    )?;
            }
        }
        if self.recovery_receipt_digest != self.stable_recovery_receipt_digest() {
            return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
                detail: String::from("grouped stage recovery receipt digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn persist_psionic_train_grouped_stage_recovery_receipt(
    path: &Path,
    mut receipt: PsionicTrainGroupedReplicaStageRecoveryReceipt,
) -> Result<
    PsionicTrainGroupedReplicaStageRecoveryReceiptArtifacts,
    PsionicTrainGroupedReplicaCheckpointError,
> {
    receipt.recovery_receipt_digest = receipt.stable_recovery_receipt_digest();
    receipt.validate()?;
    write_json(path, &receipt)?;
    Ok(PsionicTrainGroupedReplicaStageRecoveryReceiptArtifacts {
        grouped_stage_recovery_receipt_path: path.display().to_string(),
        grouped_stage_recovery_receipt_digest: receipt.recovery_receipt_digest,
    })
}

pub fn load_psionic_train_grouped_stage_recovery_receipt(
    path: &Path,
) -> Result<PsionicTrainGroupedReplicaStageRecoveryReceipt, PsionicTrainGroupedReplicaCheckpointError>
{
    let receipt: PsionicTrainGroupedReplicaStageRecoveryReceipt = read_json(path)?;
    receipt.validate()?;
    Ok(receipt)
}

pub fn persist_psionic_train_grouped_stage_recovery_receipt_from_surface(
    run_root: &Path,
    manifest: &PsionicTrainInvocationManifest,
    checkpoint_surface: &PsionicTrainCheckpointSurface,
    recovery_source_kind: PsionicTrainGroupedReplicaRecoverySourceKind,
) -> Result<
    Option<PsionicTrainGroupedReplicaStageRecoveryReceiptArtifacts>,
    PsionicTrainGroupedReplicaCheckpointError,
> {
    let Some(manifest_stage_assignment) = manifest.grouped_stage_assignment.as_ref() else {
        return Ok(None);
    };
    let Some(window_id) = manifest.coordination.window_id.as_deref() else {
        return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
            detail: String::from(
                "grouped stage recovery requires one manifest coordination.window_id",
            ),
        });
    };
    let Some(assignment_id) = manifest.coordination.assignment_id.as_deref() else {
        return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
            detail: String::from(
                "grouped stage recovery requires one manifest coordination.assignment_id",
            ),
        });
    };
    let Some(surface_stage_assignment) = checkpoint_surface.grouped_stage_assignment.as_ref()
    else {
        return Err(
            PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
                detail: String::from(
                    "grouped stage recovery requires one retained checkpoint surface bound to the same grouped stage assignment",
                ),
            },
        );
    };
    if checkpoint_surface.lane_id != manifest.lane_id {
        return Err(PsionicTrainGroupedReplicaCheckpointError::StaleAssignment {
            detail: format!(
                "grouped stage recovery targets lane `{}` but the retained checkpoint surface is `{}`",
                manifest.lane_id, checkpoint_surface.lane_id
            ),
        });
    }
    if checkpoint_surface.window_id.as_deref() != Some(window_id)
        || checkpoint_surface.assignment_id.as_deref() != Some(assignment_id)
        || surface_stage_assignment != manifest_stage_assignment
    {
        return Err(PsionicTrainGroupedReplicaCheckpointError::StaleAssignment {
            detail: String::from(
                "grouped stage recovery checkpoint surface drifted from the requested window, assignment, or grouped stage assignment",
            ),
        });
    }

    let checkpoint_label = checkpoint_surface.checkpoint_label.clone().ok_or_else(|| {
        PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
            detail: String::from("grouped stage recovery requires one retained checkpoint label"),
        }
    })?;
    let optimizer_step = checkpoint_surface.optimizer_step.ok_or_else(|| {
        PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
            detail: String::from(
                "grouped stage recovery requires one retained checkpoint optimizer_step",
            ),
        }
    })?;
    let checkpoint_ref = checkpoint_surface.checkpoint_ref.clone().ok_or_else(|| {
        PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
            detail: String::from("grouped stage recovery requires one retained checkpoint_ref"),
        }
    })?;
    let checkpoint_manifest_digest = checkpoint_surface
        .checkpoint_manifest_digest
        .clone()
        .ok_or_else(
            || PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
                detail: String::from(
                    "grouped stage recovery requires one retained checkpoint_manifest_digest",
                ),
            },
        )?;
    let checkpoint_object_digest = checkpoint_surface
        .checkpoint_object_digest
        .clone()
        .ok_or_else(
            || PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
                detail: String::from(
                    "grouped stage recovery requires one retained checkpoint_object_digest",
                ),
            },
        )?;
    let checkpoint_pointer_path = checkpoint_surface
        .artifacts
        .checkpoint_pointer_path
        .clone()
        .ok_or_else(
            || PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
                detail: String::from(
                    "grouped stage recovery requires one retained checkpoint pointer path",
                ),
            },
        )?;
    let checkpoint_manifest_path = checkpoint_surface
        .artifacts
        .checkpoint_manifest_path
        .clone()
        .ok_or_else(
            || PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
                detail: String::from(
                    "grouped stage recovery requires one retained checkpoint manifest path",
                ),
            },
        )?;
    let checkpoint_pointer = build_psionic_train_artifact_binding_from_path(
        "checkpoint_pointer",
        Path::new(checkpoint_pointer_path.as_str()),
    )
    .map_err(|detail| PsionicTrainGroupedReplicaCheckpointError::Read {
        path: checkpoint_pointer_path.clone(),
        detail,
    })?;
    let checkpoint_manifest = build_psionic_train_artifact_binding_from_path(
        "checkpoint_manifest",
        Path::new(checkpoint_manifest_path.as_str()),
    )
    .map_err(|detail| PsionicTrainGroupedReplicaCheckpointError::Read {
        path: checkpoint_manifest_path.clone(),
        detail,
    })?;
    let (resolution_state, restored_primary_pointer, checkpoint_handoff_receipt, detail) =
        match recovery_source_kind {
            PsionicTrainGroupedReplicaRecoverySourceKind::RetainedCheckpoint => (
                String::from("accepted_grouped_stage_checkpoint"),
                false,
                None,
                String::from(
                    "Grouped-replica recovery resumed from one retained local stage checkpoint that matched the requested stage assignment.",
                ),
            ),
            PsionicTrainGroupedReplicaRecoverySourceKind::PeerCheckpointHandoff => (
                String::from("accepted_peer_handoff_checkpoint"),
                true,
                Some(
                    checkpoint_surface
                        .artifacts
                        .peer_checkpoint_handoff_receipt_path
                        .as_deref()
                        .map(Path::new)
                        .map(|path| {
                            build_psionic_train_artifact_binding_from_path(
                                "checkpoint_handoff_receipt",
                                path,
                            )
                        })
                        .transpose()
                        .map_err(|detail| PsionicTrainGroupedReplicaCheckpointError::Read {
                            path: checkpoint_surface
                                .artifacts
                                .peer_checkpoint_handoff_receipt_path
                                .clone()
                                .unwrap_or_else(|| String::from("checkpoint_handoff_receipt")),
                            detail,
                        })?
                        .or_else(|| manifest.peer_checkpoint_handoff_receipt.clone())
                        .ok_or_else(|| {
                            PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
                                detail: String::from(
                                    "grouped peer recovery requires one retained checkpoint handoff receipt path",
                                ),
                            }
                        })?,
                ),
                String::from(
                    "Grouped-replica recovery resumed from one peer checkpoint handoff that matched the requested stage assignment.",
                ),
            ),
        };

    let path = run_root.join(PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_RELATIVE_PATH);
    persist_psionic_train_grouped_stage_recovery_receipt(
        path.as_path(),
        PsionicTrainGroupedReplicaStageRecoveryReceipt {
            schema_version: String::from(
                PSIONIC_TRAIN_GROUPED_STAGE_RECOVERY_RECEIPT_SCHEMA_VERSION,
            ),
            lane_id: checkpoint_surface.lane_id.clone(),
            run_id: checkpoint_surface.run_id.clone(),
            window_id: String::from(window_id),
            assignment_id: String::from(assignment_id),
            grouped_stage_assignment: surface_stage_assignment.clone(),
            checkpoint_label,
            optimizer_step,
            checkpoint_ref,
            checkpoint_manifest_digest,
            checkpoint_object_digest,
            recovery_source_kind,
            resolution_state,
            restored_primary_pointer,
            checkpoint_pointer,
            checkpoint_manifest,
            checkpoint_handoff_receipt,
            detail,
            recovery_receipt_digest: String::new(),
        },
    )
    .map(Some)
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PsionicTrainGroupedReplicaCheckpointError> {
    let parent = path
        .parent()
        .ok_or_else(|| PsionicTrainGroupedReplicaCheckpointError::Write {
            path: path.display().to_string(),
            detail: String::from("grouped stage recovery receipt path must have a parent"),
        })?;
    fs::create_dir_all(parent).map_err(|error| {
        PsionicTrainGroupedReplicaCheckpointError::Write {
            path: parent.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        PsionicTrainGroupedReplicaCheckpointError::Write {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    fs::write(path, bytes).map_err(|error| PsionicTrainGroupedReplicaCheckpointError::Write {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn read_json<T: DeserializeOwned>(
    path: &Path,
) -> Result<T, PsionicTrainGroupedReplicaCheckpointError> {
    let bytes =
        fs::read(path).map_err(|error| PsionicTrainGroupedReplicaCheckpointError::Read {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionicTrainGroupedReplicaCheckpointError::Parse {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })
}

fn require_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionicTrainGroupedReplicaCheckpointError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainGroupedReplicaCheckpointError::Invalid {
            detail: format!("{field} must not be empty"),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded =
        serde_json::to_vec(value).expect("grouped stage recovery receipt should serialize");
    let mut digest = Sha256::new();
    digest.update(prefix);
    digest.update(&encoded);
    format!("{:x}", digest.finalize())
}
