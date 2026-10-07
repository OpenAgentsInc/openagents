use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionActualPretrainingArtifactRef, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_RECIPE_ID,
};

/// Stable schema version for one retained actual-lane checkpoint manifest.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_MANIFEST_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_manifest.v1";

/// Stable schema version for one retained actual-lane checkpoint backup receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_BACKUP_RECEIPT_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_backup_receipt.v1";

/// Stable schema version for one retained actual-lane auto-resume receipt.
pub const PSION_ACTUAL_PRETRAINING_AUTO_RESUME_RECEIPT_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_auto_resume_receipt.v1";

/// Stable schema version for one retained actual-lane checkpoint failure drill receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_FAILURE_DRILL_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_failure_drill.v1";

/// Canonical fixture path for the retained actual-lane checkpoint manifest.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_manifest_v1.json";

/// Canonical fixture path for the retained actual-lane checkpoint backup receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_BACKUP_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_backup_receipt_v1.json";

/// Canonical fixture path for the retained actual-lane auto-resume receipt.
pub const PSION_ACTUAL_PRETRAINING_AUTO_RESUME_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_auto_resume_receipt_v1.json";

/// Canonical fixture path for the retained actual-lane failed-upload drill receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_FAILURE_DRILL_FAILED_UPLOAD_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_failure_drill_failed_upload_v1.json";

/// Canonical fixture path for the retained actual-lane corrupt-pointer drill receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_FAILURE_DRILL_CORRUPT_POINTER_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_failure_drill_corrupt_pointer_v1.json";

/// Canonical fixture path for the retained actual-lane stale-pointer drill receipt.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_FAILURE_DRILL_STALE_POINTER_FIXTURE_PATH: &str =
    "fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_failure_drill_stale_pointer_v1.json";

/// Focused doc path for the retained actual-lane checkpoint recovery contract.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_RECOVERY_DOC_PATH: &str =
    "docs/PSION_ACTUAL_PRETRAINING_CHECKPOINT_RECOVERY.md";

/// Retained manifest for one accepted actual-lane checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointManifest {
    pub schema_version: String,
    pub lane_id: String,
    pub recipe_id: String,
    pub run_id: String,
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub relative_manifest_path: String,
    pub relative_checkpoint_dir: String,
    pub checkpoint_object_digest: String,
    pub checkpoint_total_bytes: u64,
    pub dataset_identity: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub workspace_status_sha256: Option<String>,
    pub detail: String,
    pub manifest_digest: String,
}

/// Retained backup receipt for the latest accepted actual-lane checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointBackupReceipt {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub checkpoint_label: String,
    pub optimizer_step: u64,
    pub checkpoint_ref: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub workspace_status_sha256: Option<String>,
    pub primary_pointer: PsionActualPretrainingArtifactRef,
    pub primary_checkpoint_manifest: PsionActualPretrainingArtifactRef,
    pub backup_pointer: PsionActualPretrainingArtifactRef,
    pub backup_checkpoint_manifest: PsionActualPretrainingArtifactRef,
    pub remote_backup_root: String,
    pub credential_source_names: Vec<String>,
    pub backup_state: String,
    pub upload_outcome: String,
    pub upload_failure_reason: Option<String>,
    pub claim_boundary: String,
    pub detail: String,
    pub receipt_digest: String,
}

/// Retained auto-resume resolution receipt for the actual lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingAutoResumeReceipt {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub workspace_status_sha256: Option<String>,
    pub requested_primary_pointer_path: String,
    pub backup_receipt_path: String,
    pub backup_pointer_path: String,
    pub primary_pointer_state: String,
    pub resolution_state: String,
    pub resume_source_kind: String,
    pub restored_primary_pointer: bool,
    pub chosen_checkpoint_label: Option<String>,
    pub chosen_optimizer_step: Option<u64>,
    pub chosen_checkpoint_ref: Option<String>,
    pub chosen_checkpoint_manifest: Option<PsionActualPretrainingArtifactRef>,
    pub refusal_reason: Option<String>,
    pub claim_boundary: String,
    pub detail: String,
    pub receipt_digest: String,
}

/// Retained failure-injection receipt for the actual-lane checkpoint path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointFailureDrill {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub drill_id: String,
    pub drill_kind: String,
    pub trigger_surface: String,
    pub selected_git_ref: String,
    pub git_commit_sha: String,
    pub dirty_tree_admission: String,
    pub workspace_status_sha256: Option<String>,
    pub outcome: String,
    pub evidence_paths: Vec<String>,
    pub refusal_reason: Option<String>,
    pub claim_boundary: String,
    pub detail: String,
    pub receipt_digest: String,
}

impl PsionActualPretrainingCheckpointManifest {
    pub fn validate(
        &self,
    ) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_manifest.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_MANIFEST_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "checkpoint_manifest.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_exact(
            self.recipe_id.as_str(),
            "checkpoint_manifest.recipe_id",
            PSION_ACTUAL_PRETRAINING_RECIPE_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "checkpoint_manifest.run_id")?;
        ensure_nonempty(
            self.checkpoint_label.as_str(),
            "checkpoint_manifest.checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
                field: String::from("checkpoint_manifest.optimizer_step"),
            });
        }
        ensure_nonempty(
            self.checkpoint_ref.as_str(),
            "checkpoint_manifest.checkpoint_ref",
        )?;
        ensure_exact(
            self.relative_manifest_path.as_str(),
            "checkpoint_manifest.relative_manifest_path",
            &format!(
                "checkpoints/step-{}/checkpoint_manifest.json",
                self.optimizer_step
            ),
        )?;
        ensure_exact(
            self.relative_checkpoint_dir.as_str(),
            "checkpoint_manifest.relative_checkpoint_dir",
            &format!("checkpoints/step-{}", self.optimizer_step),
        )?;
        ensure_nonempty(
            self.checkpoint_object_digest.as_str(),
            "checkpoint_manifest.checkpoint_object_digest",
        )?;
        if self.checkpoint_total_bytes == 0 {
            return Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
                field: String::from("checkpoint_manifest.checkpoint_total_bytes"),
            });
        }
        ensure_nonempty(
            self.dataset_identity.as_str(),
            "checkpoint_manifest.dataset_identity",
        )?;
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "checkpoint_manifest.selected_git_ref",
        )?;
        ensure_git_sha(
            self.git_commit_sha.as_str(),
            "checkpoint_manifest.git_commit_sha",
        )?;
        ensure_dirty_tree_admission(
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            "checkpoint_manifest",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_manifest.detail")?;
        if self.manifest_digest != stable_checkpoint_manifest_digest(self)? {
            return Err(PsionActualPretrainingCheckpointRecoveryError::DigestMismatch {
                field: String::from("checkpoint_manifest.manifest_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingCheckpointBackupReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_backup_receipt.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_BACKUP_RECEIPT_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "checkpoint_backup_receipt.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "checkpoint_backup_receipt.run_id",
        )?;
        ensure_nonempty(
            self.checkpoint_label.as_str(),
            "checkpoint_backup_receipt.checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
                field: String::from("checkpoint_backup_receipt.optimizer_step"),
            });
        }
        ensure_nonempty(
            self.checkpoint_ref.as_str(),
            "checkpoint_backup_receipt.checkpoint_ref",
        )?;
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "checkpoint_backup_receipt.selected_git_ref",
        )?;
        ensure_git_sha(
            self.git_commit_sha.as_str(),
            "checkpoint_backup_receipt.git_commit_sha",
        )?;
        ensure_dirty_tree_admission(
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            "checkpoint_backup_receipt",
        )?;
        ensure_artifact_ref(
            &self.primary_pointer,
            "checkpoint_backup_receipt.primary_pointer",
        )?;
        ensure_artifact_ref(
            &self.primary_checkpoint_manifest,
            "checkpoint_backup_receipt.primary_checkpoint_manifest",
        )?;
        ensure_artifact_ref(
            &self.backup_pointer,
            "checkpoint_backup_receipt.backup_pointer",
        )?;
        ensure_artifact_ref(
            &self.backup_checkpoint_manifest,
            "checkpoint_backup_receipt.backup_checkpoint_manifest",
        )?;
        ensure_exact(
            self.primary_pointer.path.as_str(),
            "checkpoint_backup_receipt.primary_pointer.path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_exact(
            self.primary_checkpoint_manifest.path.as_str(),
            "checkpoint_backup_receipt.primary_checkpoint_manifest.path",
            &format!(
                "checkpoints/step-{}/checkpoint_manifest.json",
                self.optimizer_step
            ),
        )?;
        ensure_exact(
            self.backup_pointer.path.as_str(),
            "checkpoint_backup_receipt.backup_pointer.path",
            "checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json",
        )?;
        ensure_exact(
            self.backup_checkpoint_manifest.path.as_str(),
            "checkpoint_backup_receipt.backup_checkpoint_manifest.path",
            &format!(
                "checkpoints/backups/step-{}/checkpoint_manifest.backup.json",
                self.optimizer_step
            ),
        )?;
        ensure_nonempty(
            self.remote_backup_root.as_str(),
            "checkpoint_backup_receipt.remote_backup_root",
        )?;
        ensure_unique_nonempty_strings(
            &self.credential_source_names,
            "checkpoint_backup_receipt.credential_source_names[]",
        )?;
        match (self.backup_state.as_str(), self.upload_outcome.as_str()) {
            ("backed_up", "succeeded") => {
                if self.upload_failure_reason.is_some() {
                    return Err(
                        PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                            field: String::from(
                                "checkpoint_backup_receipt.upload_failure_reason",
                            ),
                            detail: String::from(
                                "successful backup receipts must not retain upload_failure_reason",
                            ),
                        },
                    );
                }
            }
            ("refused", "failed") => {
                ensure_nonempty_option(
                    self.upload_failure_reason.as_deref(),
                    "checkpoint_backup_receipt.upload_failure_reason",
                )?;
            }
            _ => {
                return Err(
                    PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                        field: String::from("checkpoint_backup_receipt.backup_state"),
                        detail: String::from(
                            "backup receipt must be backed_up/succeeded or refused/failed",
                        ),
                    },
                );
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "checkpoint_backup_receipt.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_backup_receipt.detail")?;
        if self.receipt_digest != stable_checkpoint_backup_receipt_digest(self)? {
            return Err(PsionActualPretrainingCheckpointRecoveryError::DigestMismatch {
                field: String::from("checkpoint_backup_receipt.receipt_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingAutoResumeReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
        ensure_exact(
            self.schema_version.as_str(),
            "auto_resume_receipt.schema_version",
            PSION_ACTUAL_PRETRAINING_AUTO_RESUME_RECEIPT_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "auto_resume_receipt.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "auto_resume_receipt.run_id")?;
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "auto_resume_receipt.selected_git_ref",
        )?;
        ensure_git_sha(
            self.git_commit_sha.as_str(),
            "auto_resume_receipt.git_commit_sha",
        )?;
        ensure_dirty_tree_admission(
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            "auto_resume_receipt",
        )?;
        ensure_exact(
            self.requested_primary_pointer_path.as_str(),
            "auto_resume_receipt.requested_primary_pointer_path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_exact(
            self.backup_receipt_path.as_str(),
            "auto_resume_receipt.backup_receipt_path",
            "checkpoints/latest_accepted_checkpoint_backup_receipt.json",
        )?;
        ensure_exact(
            self.backup_pointer_path.as_str(),
            "auto_resume_receipt.backup_pointer_path",
            "checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json",
        )?;
        match self.primary_pointer_state.as_str() {
            "accepted" | "corrupt" | "stale" | "missing" => {}
            _ => {
                return Err(
                    PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                        field: String::from("auto_resume_receipt.primary_pointer_state"),
                        detail: String::from(
                            "primary pointer state must be accepted, corrupt, stale, or missing",
                        ),
                    },
                );
            }
        }
        match (
            self.resolution_state.as_str(),
            self.resume_source_kind.as_str(),
            self.restored_primary_pointer,
        ) {
            ("accepted_primary_pointer", "primary_pointer", false) => {
                validate_resolved_auto_resume(self)?;
                if self.refusal_reason.is_some() {
                    return Err(
                        PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                            field: String::from("auto_resume_receipt.refusal_reason"),
                            detail: String::from(
                                "accepted auto-resume receipts must not retain refusal_reason",
                            ),
                        },
                    );
                }
            }
            ("recovered_from_backup", "backup_receipt", true) => {
                validate_resolved_auto_resume(self)?;
                if self.refusal_reason.is_some() {
                    return Err(
                        PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                            field: String::from("auto_resume_receipt.refusal_reason"),
                            detail: String::from(
                                "recovered auto-resume receipts must not retain refusal_reason",
                            ),
                        },
                    );
                }
            }
            ("refused", "none", false) => {
                ensure_nonempty_option(
                    self.refusal_reason.as_deref(),
                    "auto_resume_receipt.refusal_reason",
                )?;
                if self.chosen_checkpoint_label.is_some()
                    || self.chosen_optimizer_step.is_some()
                    || self.chosen_checkpoint_ref.is_some()
                    || self.chosen_checkpoint_manifest.is_some()
                {
                    return Err(
                        PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                            field: String::from("auto_resume_receipt.chosen_checkpoint"),
                            detail: String::from(
                                "refused auto-resume receipts must not retain a chosen checkpoint",
                            ),
                        },
                    );
                }
            }
            _ => {
                return Err(
                    PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                        field: String::from("auto_resume_receipt.resolution_state"),
                        detail: String::from(
                            "auto-resume receipt must be accepted_primary_pointer, recovered_from_backup, or refused",
                        ),
                    },
                );
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "auto_resume_receipt.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "auto_resume_receipt.detail")?;
        if self.receipt_digest != stable_auto_resume_receipt_digest(self)? {
            return Err(PsionActualPretrainingCheckpointRecoveryError::DigestMismatch {
                field: String::from("auto_resume_receipt.receipt_digest"),
            });
        }
        Ok(())
    }
}

impl PsionActualPretrainingCheckpointFailureDrill {
    pub fn validate(
        &self,
    ) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_failure_drill.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_FAILURE_DRILL_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "checkpoint_failure_drill.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "checkpoint_failure_drill.run_id",
        )?;
        ensure_nonempty(
            self.drill_id.as_str(),
            "checkpoint_failure_drill.drill_id",
        )?;
        match self.drill_kind.as_str() {
            "failed_upload" | "corrupt_pointer" | "stale_pointer" => {}
            _ => {
                return Err(
                    PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                        field: String::from("checkpoint_failure_drill.drill_kind"),
                        detail: String::from(
                            "checkpoint failure drill kind must be failed_upload, corrupt_pointer, or stale_pointer",
                        ),
                    },
                );
            }
        }
        match self.trigger_surface.as_str() {
            "backup" | "resume" => {}
            _ => {
                return Err(
                    PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                        field: String::from("checkpoint_failure_drill.trigger_surface"),
                        detail: String::from(
                            "checkpoint failure drill trigger surface must be backup or resume",
                        ),
                    },
                );
            }
        }
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "checkpoint_failure_drill.selected_git_ref",
        )?;
        ensure_git_sha(
            self.git_commit_sha.as_str(),
            "checkpoint_failure_drill.git_commit_sha",
        )?;
        ensure_dirty_tree_admission(
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            "checkpoint_failure_drill",
        )?;
        ensure_unique_nonempty_strings(
            &self.evidence_paths,
            "checkpoint_failure_drill.evidence_paths[]",
        )?;
        match self.outcome.as_str() {
            "retained_refusal" => ensure_nonempty_option(
                self.refusal_reason.as_deref(),
                "checkpoint_failure_drill.refusal_reason",
            )?,
            "recovered_without_manual_edit" => {
                if self.refusal_reason.is_some() {
                    return Err(
                        PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                            field: String::from("checkpoint_failure_drill.refusal_reason"),
                            detail: String::from(
                                "recovered failure drills must not retain refusal_reason",
                            ),
                        },
                    );
                }
            }
            _ => {
                return Err(
                    PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                        field: String::from("checkpoint_failure_drill.outcome"),
                        detail: String::from(
                            "checkpoint failure drill outcome must be retained_refusal or recovered_without_manual_edit",
                        ),
                    },
                );
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "checkpoint_failure_drill.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_failure_drill.detail")?;
        if self.receipt_digest != stable_checkpoint_failure_drill_digest(self)? {
            return Err(PsionActualPretrainingCheckpointRecoveryError::DigestMismatch {
                field: String::from("checkpoint_failure_drill.receipt_digest"),
            });
        }
        Ok(())
    }
}

pub fn stable_checkpoint_manifest_digest(
    manifest: &PsionActualPretrainingCheckpointManifest,
) -> Result<String, PsionActualPretrainingCheckpointRecoveryError> {
    let mut copy = manifest.clone();
    copy.manifest_digest.clear();
    stable_digest(b"psion_actual_pretraining_checkpoint_manifest|", &copy)
}

pub fn stable_checkpoint_backup_receipt_digest(
    receipt: &PsionActualPretrainingCheckpointBackupReceipt,
) -> Result<String, PsionActualPretrainingCheckpointRecoveryError> {
    let mut copy = receipt.clone();
    copy.receipt_digest.clear();
    stable_digest(
        b"psion_actual_pretraining_checkpoint_backup_receipt|",
        &copy,
    )
}

pub fn stable_auto_resume_receipt_digest(
    receipt: &PsionActualPretrainingAutoResumeReceipt,
) -> Result<String, PsionActualPretrainingCheckpointRecoveryError> {
    let mut copy = receipt.clone();
    copy.receipt_digest.clear();
    stable_digest(b"psion_actual_pretraining_auto_resume_receipt|", &copy)
}

pub fn stable_checkpoint_failure_drill_digest(
    receipt: &PsionActualPretrainingCheckpointFailureDrill,
) -> Result<String, PsionActualPretrainingCheckpointRecoveryError> {
    let mut copy = receipt.clone();
    copy.receipt_digest.clear();
    stable_digest(
        b"psion_actual_pretraining_checkpoint_failure_drill|",
        &copy,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn record_psion_actual_pretraining_checkpoint_manifest(
    run_id: &str,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    checkpoint_object_digest: &str,
    checkpoint_total_bytes: u64,
    dataset_identity: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    detail: &str,
) -> Result<
    PsionActualPretrainingCheckpointManifest,
    PsionActualPretrainingCheckpointRecoveryError,
> {
    let mut manifest = PsionActualPretrainingCheckpointManifest {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_MANIFEST_SCHEMA_VERSION,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
        run_id: String::from(run_id),
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: String::from(checkpoint_ref),
        relative_manifest_path: format!(
            "checkpoints/step-{optimizer_step}/checkpoint_manifest.json"
        ),
        relative_checkpoint_dir: format!("checkpoints/step-{optimizer_step}"),
        checkpoint_object_digest: String::from(checkpoint_object_digest),
        checkpoint_total_bytes,
        dataset_identity: String::from(dataset_identity),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        detail: String::from(detail),
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = stable_checkpoint_manifest_digest(&manifest)?;
    manifest.validate()?;
    Ok(manifest)
}

#[allow(clippy::too_many_arguments)]
pub fn record_psion_actual_pretraining_checkpoint_backup_receipt(
    run_id: &str,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    primary_pointer: PsionActualPretrainingArtifactRef,
    primary_checkpoint_manifest: PsionActualPretrainingArtifactRef,
    backup_pointer: PsionActualPretrainingArtifactRef,
    backup_checkpoint_manifest: PsionActualPretrainingArtifactRef,
    remote_backup_root: &str,
    credential_source_names: Vec<String>,
    backup_state: &str,
    upload_outcome: &str,
    upload_failure_reason: Option<String>,
    claim_boundary: &str,
    detail: &str,
) -> Result<
    PsionActualPretrainingCheckpointBackupReceipt,
    PsionActualPretrainingCheckpointRecoveryError,
> {
    let mut receipt = PsionActualPretrainingCheckpointBackupReceipt {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_BACKUP_RECEIPT_SCHEMA_VERSION,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: String::from(checkpoint_ref),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        primary_pointer,
        primary_checkpoint_manifest,
        backup_pointer,
        backup_checkpoint_manifest,
        remote_backup_root: String::from(remote_backup_root),
        credential_source_names,
        backup_state: String::from(backup_state),
        upload_outcome: String::from(upload_outcome),
        upload_failure_reason,
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_checkpoint_backup_receipt_digest(&receipt)?;
    receipt.validate()?;
    Ok(receipt)
}

#[allow(clippy::too_many_arguments)]
pub fn record_psion_actual_pretraining_auto_resume_receipt(
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    primary_pointer_state: &str,
    resolution_state: &str,
    resume_source_kind: &str,
    restored_primary_pointer: bool,
    chosen_checkpoint_label: Option<String>,
    chosen_optimizer_step: Option<u64>,
    chosen_checkpoint_ref: Option<String>,
    chosen_checkpoint_manifest: Option<PsionActualPretrainingArtifactRef>,
    refusal_reason: Option<String>,
    claim_boundary: &str,
    detail: &str,
) -> Result<
    PsionActualPretrainingAutoResumeReceipt,
    PsionActualPretrainingCheckpointRecoveryError,
> {
    let mut receipt = PsionActualPretrainingAutoResumeReceipt {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_AUTO_RESUME_RECEIPT_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        requested_primary_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        backup_receipt_path: String::from(
            "checkpoints/latest_accepted_checkpoint_backup_receipt.json",
        ),
        backup_pointer_path: String::from(
            "checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json",
        ),
        primary_pointer_state: String::from(primary_pointer_state),
        resolution_state: String::from(resolution_state),
        resume_source_kind: String::from(resume_source_kind),
        restored_primary_pointer,
        chosen_checkpoint_label,
        chosen_optimizer_step,
        chosen_checkpoint_ref,
        chosen_checkpoint_manifest,
        refusal_reason,
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_auto_resume_receipt_digest(&receipt)?;
    receipt.validate()?;
    Ok(receipt)
}

#[allow(clippy::too_many_arguments)]
pub fn record_psion_actual_pretraining_checkpoint_failure_drill(
    run_id: &str,
    drill_id: &str,
    drill_kind: &str,
    trigger_surface: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    outcome: &str,
    evidence_paths: Vec<String>,
    refusal_reason: Option<String>,
    claim_boundary: &str,
    detail: &str,
) -> Result<
    PsionActualPretrainingCheckpointFailureDrill,
    PsionActualPretrainingCheckpointRecoveryError,
> {
    let mut receipt = PsionActualPretrainingCheckpointFailureDrill {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_FAILURE_DRILL_SCHEMA_VERSION,
        ),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        drill_id: String::from(drill_id),
        drill_kind: String::from(drill_kind),
        trigger_surface: String::from(trigger_surface),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        outcome: String::from(outcome),
        evidence_paths,
        refusal_reason,
        claim_boundary: String::from(claim_boundary),
        detail: String::from(detail),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_checkpoint_failure_drill_digest(&receipt)?;
    receipt.validate()?;
    Ok(receipt)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingCheckpointRecoveryError {
    #[error("psion actual-pretraining checkpoint recovery field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining checkpoint recovery field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining checkpoint recovery field `{field}` is unsupported: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("psion actual-pretraining checkpoint recovery digest drifted for `{field}`")]
    DigestMismatch { field: String },
}

fn validate_resolved_auto_resume(
    receipt: &PsionActualPretrainingAutoResumeReceipt,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    ensure_nonempty_option(
        receipt.chosen_checkpoint_label.as_deref(),
        "auto_resume_receipt.chosen_checkpoint_label",
    )?;
    if receipt.chosen_optimizer_step.unwrap_or(0) == 0 {
        return Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
            field: String::from("auto_resume_receipt.chosen_optimizer_step"),
        });
    }
    ensure_nonempty_option(
        receipt.chosen_checkpoint_ref.as_deref(),
        "auto_resume_receipt.chosen_checkpoint_ref",
    )?;
    match &receipt.chosen_checkpoint_manifest {
        Some(manifest) => ensure_artifact_ref(manifest, "auto_resume_receipt.chosen_checkpoint_manifest"),
        None => Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
            field: String::from("auto_resume_receipt.chosen_checkpoint_manifest"),
        }),
    }
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionActualPretrainingCheckpointRecoveryError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_nonempty_option(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    match value {
        Some(value) => ensure_nonempty(value, field),
        None => Err(PsionActualPretrainingCheckpointRecoveryError::MissingField {
            field: String::from(field),
        }),
    }
}

fn ensure_unique_nonempty_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value) {
            return Err(PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                field: String::from(field),
                detail: format!("duplicate value `{value}`"),
            });
        }
    }
    Ok(())
}

fn ensure_git_sha(
    value: &str,
    field: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    ensure_nonempty(value, field)?;
    if value.len() != 40 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
            field: String::from(field),
            detail: String::from("git commit SHA must be a 40-character hex string"),
        });
    }
    Ok(())
}

fn ensure_dirty_tree_admission(
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingCheckpointRecoveryError> {
    match dirty_tree_admission {
        "refuse_by_default" => Ok(()),
        "allowed_by_operator_override" => ensure_nonempty_option(
            workspace_status_sha256,
            &format!("{field_prefix}.workspace_status_sha256"),
        ),
        _ => Err(PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
            field: String::from(field_prefix),
            detail: String::from(
                "dirty-tree admission must be refuse_by_default or allowed_by_operator_override",
            ),
        }),
    }
}

fn stable_digest<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<String, PsionActualPretrainingCheckpointRecoveryError> {
    let canonical = serde_json::to_vec(value).map_err(|error| {
        PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
            field: String::from("serialization"),
            detail: error.to_string(),
        }
    })?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical);
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{
        PsionActualPretrainingAutoResumeReceipt, PsionActualPretrainingCheckpointBackupReceipt,
        PsionActualPretrainingCheckpointFailureDrill, PsionActualPretrainingCheckpointManifest,
        PsionActualPretrainingCheckpointRecoveryError,
    };

    fn checkpoint_manifest() -> PsionActualPretrainingCheckpointManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_manifest_v1.json"
        ))
        .expect("actual pretraining checkpoint manifest fixture should parse")
    }

    fn checkpoint_backup_receipt() -> PsionActualPretrainingCheckpointBackupReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_backup_receipt_v1.json"
        ))
        .expect("actual pretraining checkpoint backup receipt fixture should parse")
    }

    fn auto_resume_receipt() -> PsionActualPretrainingAutoResumeReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_auto_resume_receipt_v1.json"
        ))
        .expect("actual pretraining auto-resume receipt fixture should parse")
    }

    fn failed_upload_drill() -> PsionActualPretrainingCheckpointFailureDrill {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_failure_drill_failed_upload_v1.json"
        ))
        .expect("actual pretraining failed-upload drill fixture should parse")
    }

    fn corrupt_pointer_drill() -> PsionActualPretrainingCheckpointFailureDrill {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_failure_drill_corrupt_pointer_v1.json"
        ))
        .expect("actual pretraining corrupt-pointer drill fixture should parse")
    }

    fn stale_pointer_drill() -> PsionActualPretrainingCheckpointFailureDrill {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_failure_drill_stale_pointer_v1.json"
        ))
        .expect("actual pretraining stale-pointer drill fixture should parse")
    }

    #[test]
    fn actual_pretraining_checkpoint_manifest_fixture_validates() {
        checkpoint_manifest()
            .validate()
            .expect("actual pretraining checkpoint manifest fixture should validate");
    }

    #[test]
    fn actual_pretraining_checkpoint_backup_receipt_fixture_validates() {
        checkpoint_backup_receipt()
            .validate()
            .expect("actual pretraining checkpoint backup receipt fixture should validate");
    }

    #[test]
    fn actual_pretraining_auto_resume_receipt_fixture_validates() {
        auto_resume_receipt()
            .validate()
            .expect("actual pretraining auto-resume receipt fixture should validate");
    }

    #[test]
    fn actual_pretraining_failure_drill_fixtures_validate() {
        failed_upload_drill()
            .validate()
            .expect("failed-upload drill fixture should validate");
        corrupt_pointer_drill()
            .validate()
            .expect("corrupt-pointer drill fixture should validate");
        stale_pointer_drill()
            .validate()
            .expect("stale-pointer drill fixture should validate");
    }

    #[test]
    fn backup_receipt_rejects_success_with_failure_reason() {
        let mut receipt = checkpoint_backup_receipt();
        receipt.upload_failure_reason = Some(String::from("unexpected"));
        let error = receipt
            .validate()
            .expect_err("successful backup receipt should reject failure reason");
        assert_eq!(
            error,
            PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                field: String::from("checkpoint_backup_receipt.upload_failure_reason"),
                detail: String::from(
                    "successful backup receipts must not retain upload_failure_reason",
                ),
            }
        );
    }

    #[test]
    fn auto_resume_receipt_rejects_refused_resolution_with_checkpoint_choice() {
        let mut receipt = auto_resume_receipt();
        receipt.resolution_state = String::from("refused");
        receipt.resume_source_kind = String::from("none");
        receipt.restored_primary_pointer = false;
        receipt.refusal_reason = Some(String::from("pointer and backup unavailable"));
        let error = receipt
            .validate()
            .expect_err("refused auto-resume receipt should reject chosen checkpoint");
        assert_eq!(
            error,
            PsionActualPretrainingCheckpointRecoveryError::UnsupportedValue {
                field: String::from("auto_resume_receipt.chosen_checkpoint"),
                detail: String::from(
                    "refused auto-resume receipts must not retain a chosen checkpoint",
                ),
            }
        );
    }
}
