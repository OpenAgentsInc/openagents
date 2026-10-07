use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_psionic_train_artifact_binding_from_path, inspect_psionic_train_checkpoint_surface,
    psionic_train_resolved_artifact_cache_candidates, psionic_train_resolved_artifact_cache_key,
    PsionActualPretrainingCheckpointManifest, PsionActualPretrainingCheckpointPointer,
    PsionicTrainArtifactBinding, PsionicTrainCheckpointManifest, PsionicTrainCheckpointPointer,
    PsionicTrainGroupedReplicaStageAssignment, PsionicTrainOperation, PsionicTrainRole,
    PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION,
    PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSIONIC_TRAIN_RESOLVED_ARTIFACT_CACHE_RELATIVE_DIR,
};

/// Stable schema version for the machine-readable peer checkpoint handoff receipt.
pub const PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.train.checkpoint_handoff_receipt.v1";

/// Canonical retained current handoff path under a run root.
pub const PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_RELATIVE_PATH: &str =
    "status/peer_checkpoint_handoff_receipt.json";

/// Canonical retained handoff history directory under a run root.
pub const PSIONIC_TRAIN_CHECKPOINT_HANDOFF_HISTORY_RELATIVE_DIR: &str =
    "status/peer_checkpoint_handoffs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainCheckpointHandoffSourceKind {
    LivePrimaryPointer,
    DurableBackupCopy,
}

/// Typed receipt describing one peer-readable checkpoint handoff source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCheckpointHandoffReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Admitted node pubkey serving the checkpoint state.
    pub serving_node_pubkey: String,
    /// Admitted node pubkey the handoff targets.
    pub peer_node_pubkey: String,
    /// Stable source run identifier.
    pub source_run_id: String,
    /// Stable window identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    /// Stable assignment identifier when the checkpoint belongs to one grouped stage window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    /// Stable grouped stage assignment when the checkpoint belongs to one grouped replica stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Optional source run root retained only for operator diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_run_root: Option<String>,
    /// Whether the source is the live primary pointer or a durable backup copy.
    pub source_kind: PsionicTrainCheckpointHandoffSourceKind,
    /// Latest checkpoint label.
    pub checkpoint_label: String,
    /// Latest checkpoint step.
    pub optimizer_step: u64,
    /// Latest checkpoint ref.
    pub checkpoint_ref: String,
    /// Stable checkpoint-manifest digest.
    pub checkpoint_manifest_digest: String,
    /// Stable checkpoint-object digest.
    pub checkpoint_object_digest: String,
    /// Stable checkpoint byte count.
    pub checkpoint_total_bytes: u64,
    /// Logical binding for the source checkpoint pointer.
    pub source_checkpoint_pointer: PsionicTrainArtifactBinding,
    /// Logical binding for the source checkpoint manifest.
    pub source_checkpoint_manifest: PsionicTrainArtifactBinding,
    /// Whether the source handoff had to fall back to the durable backup copy.
    pub restored_from_backup: bool,
    /// Short claim boundary.
    pub detail: String,
    /// Stable digest over the receipt payload.
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainCheckpointHandoffMaterialization {
    pub local_receipt_path: String,
    pub local_checkpoint_pointer_path: String,
    pub local_checkpoint_manifest_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionicTrainCheckpointHandoffRetention {
    pub current_receipt_path: String,
    pub history_receipt_path: String,
}

#[derive(Debug, Error)]
pub enum PsionicTrainCheckpointHandoffError {
    #[error("checkpoint handoff is unavailable: {detail}")]
    MissingCheckpoint { detail: String },
    #[error("checkpoint handoff is invalid: {detail}")]
    Invalid { detail: String },
    #[error("failed to read `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to parse `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("failed to write `{path}`: {detail}")]
    Write { path: String, detail: String },
}

impl PsionicTrainCheckpointHandoffReceipt {
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.source_run_root = None;
        clone.source_checkpoint_pointer = clone.source_checkpoint_pointer.canonicalize_for_digest();
        clone.source_checkpoint_manifest =
            clone.source_checkpoint_manifest.canonicalize_for_digest();
        clone.receipt_digest.clear();
        stable_digest(&clone)
    }

    pub fn validate(&self) -> Result<(), PsionicTrainCheckpointHandoffError> {
        if self.schema_version != PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_SCHEMA_VERSION {
            return Err(PsionicTrainCheckpointHandoffError::Invalid {
                detail: format!(
                    "checkpoint handoff schema version must stay `{}` but was `{}`",
                    PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        require_nonempty(self.lane_id.as_str(), "checkpoint handoff lane_id")?;
        require_nonempty(
            self.serving_node_pubkey.as_str(),
            "checkpoint handoff serving_node_pubkey",
        )?;
        require_nonempty(
            self.peer_node_pubkey.as_str(),
            "checkpoint handoff peer_node_pubkey",
        )?;
        require_nonempty(
            self.source_run_id.as_str(),
            "checkpoint handoff source_run_id",
        )?;
        validate_grouped_stage_scope(
            self.window_id.as_deref(),
            self.assignment_id.as_deref(),
            self.grouped_stage_assignment.as_ref(),
        )?;
        if let Some(source_run_root) = self.source_run_root.as_deref() {
            require_nonempty(source_run_root, "checkpoint handoff source_run_root")?;
        }
        require_nonempty(
            self.checkpoint_label.as_str(),
            "checkpoint handoff checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionicTrainCheckpointHandoffError::Invalid {
                detail: String::from("checkpoint handoff optimizer_step must be non-zero"),
            });
        }
        require_nonempty(
            self.checkpoint_ref.as_str(),
            "checkpoint handoff checkpoint_ref",
        )?;
        require_nonempty(
            self.checkpoint_manifest_digest.as_str(),
            "checkpoint handoff checkpoint_manifest_digest",
        )?;
        require_nonempty(
            self.checkpoint_object_digest.as_str(),
            "checkpoint handoff checkpoint_object_digest",
        )?;
        if self.checkpoint_total_bytes == 0 {
            return Err(PsionicTrainCheckpointHandoffError::Invalid {
                detail: String::from("checkpoint handoff checkpoint_total_bytes must be non-zero"),
            });
        }
        self.source_checkpoint_pointer
            .validate("checkpoint_handoff.source_checkpoint_pointer")
            .map_err(|detail| PsionicTrainCheckpointHandoffError::Invalid { detail })?;
        self.source_checkpoint_manifest
            .validate("checkpoint_handoff.source_checkpoint_manifest")
            .map_err(|detail| PsionicTrainCheckpointHandoffError::Invalid { detail })?;
        require_nonempty(self.detail.as_str(), "checkpoint handoff detail")?;
        if self.receipt_digest != self.stable_digest() {
            return Err(PsionicTrainCheckpointHandoffError::Invalid {
                detail: String::from("checkpoint handoff receipt digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn build_psionic_train_checkpoint_handoff_receipt(
    run_root: &Path,
    serving_node_pubkey: &str,
    peer_node_pubkey: &str,
) -> Result<PsionicTrainCheckpointHandoffReceipt, PsionicTrainCheckpointHandoffError> {
    let checkpoint_surface = inspect_psionic_train_checkpoint_surface(
        run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    )
    .map_err(|error| PsionicTrainCheckpointHandoffError::Invalid {
        detail: error.to_string(),
    })?
    .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
        detail: String::from(
            "checkpoint handoff requires retained checkpoint state under the source run root",
        ),
    })?;

    let checkpoint_label = checkpoint_surface.checkpoint_label.clone().ok_or_else(|| {
        PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from(
                "checkpoint handoff requires an accepted checkpoint label from the source run root",
            ),
        }
    })?;
    let optimizer_step = checkpoint_surface.optimizer_step.ok_or_else(|| {
        PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from(
                "checkpoint handoff requires an accepted checkpoint step from the source run root",
            ),
        }
    })?;
    let checkpoint_ref = checkpoint_surface.checkpoint_ref.clone().ok_or_else(|| {
        PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from(
                "checkpoint handoff requires an accepted checkpoint ref from the source run root",
            ),
        }
    })?;
    let checkpoint_manifest_digest = checkpoint_surface
        .checkpoint_manifest_digest
        .clone()
        .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from(
                "checkpoint handoff requires an accepted checkpoint manifest digest",
            ),
        })?;
    let checkpoint_object_digest = checkpoint_surface
        .checkpoint_object_digest
        .clone()
        .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from(
                "checkpoint handoff requires an accepted checkpoint object digest",
            ),
        })?;
    let checkpoint_total_bytes = checkpoint_surface.checkpoint_total_bytes.ok_or_else(|| {
        PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from("checkpoint handoff requires an accepted checkpoint byte count"),
        }
    })?;

    let (
        source_kind,
        source_checkpoint_pointer_path,
        source_checkpoint_manifest_path,
        detail,
        restored_from_backup,
    ) = if checkpoint_surface.pointer_state.as_deref() == Some("accepted") {
        (
            PsionicTrainCheckpointHandoffSourceKind::LivePrimaryPointer,
            checkpoint_surface
                .artifacts
                .checkpoint_pointer_path
                .clone()
                .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
                    detail: String::from(
                        "checkpoint handoff requires the live primary pointer path",
                    ),
                })?,
            checkpoint_surface
                .artifacts
                .checkpoint_manifest_path
                .clone()
                .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
                    detail: String::from(
                        "checkpoint handoff requires the live checkpoint manifest path",
                    ),
                })?,
            String::from(
                "Recovery-source handoff points the joiner at the live accepted checkpoint pointer and manifest from the serving run root.",
            ),
            false,
        )
    } else if checkpoint_surface.backup_state.as_deref() == Some("backed_up") {
        (
            PsionicTrainCheckpointHandoffSourceKind::DurableBackupCopy,
            checkpoint_surface
                .artifacts
                .checkpoint_backup_pointer_path
                .clone()
                .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
                    detail: String::from(
                        "checkpoint handoff requires the durable backup pointer path",
                    ),
                })?,
            checkpoint_surface
                .artifacts
                .checkpoint_backup_manifest_path
                .clone()
                .ok_or_else(|| PsionicTrainCheckpointHandoffError::MissingCheckpoint {
                    detail: String::from(
                        "checkpoint handoff requires the durable backup manifest path",
                    ),
                })?,
            String::from(
                "Recovery-source handoff falls back to the durable backup checkpoint copy because the live primary pointer is not presently usable.",
            ),
            true,
        )
    } else {
        return Err(PsionicTrainCheckpointHandoffError::MissingCheckpoint {
            detail: String::from(
                "checkpoint handoff requires an accepted primary checkpoint or a durable backup copy",
            ),
        });
    };
    let source_checkpoint_pointer = build_psionic_train_artifact_binding_from_path(
        "checkpoint_pointer",
        Path::new(source_checkpoint_pointer_path.as_str()),
    )
    .map_err(|detail| PsionicTrainCheckpointHandoffError::Read {
        path: source_checkpoint_pointer_path.clone(),
        detail,
    })?;
    let source_checkpoint_manifest = build_psionic_train_artifact_binding_from_path(
        "checkpoint_manifest",
        Path::new(source_checkpoint_manifest_path.as_str()),
    )
    .map_err(|detail| PsionicTrainCheckpointHandoffError::Read {
        path: source_checkpoint_manifest_path.clone(),
        detail,
    })?;

    let mut receipt = PsionicTrainCheckpointHandoffReceipt {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_SCHEMA_VERSION),
        lane_id: checkpoint_surface.lane_id,
        serving_node_pubkey: String::from(serving_node_pubkey),
        peer_node_pubkey: String::from(peer_node_pubkey),
        source_run_id: checkpoint_surface.run_id,
        window_id: checkpoint_surface.window_id.clone(),
        assignment_id: checkpoint_surface.assignment_id.clone(),
        grouped_stage_assignment: checkpoint_surface.grouped_stage_assignment.clone(),
        source_run_root: Some(run_root.display().to_string()),
        source_kind,
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        checkpoint_manifest_digest,
        checkpoint_object_digest,
        checkpoint_total_bytes,
        source_checkpoint_pointer,
        source_checkpoint_manifest,
        restored_from_backup,
        detail,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt.validate()?;
    Ok(receipt)
}

pub fn materialize_psionic_train_checkpoint_handoff(
    joiner_run_root: &Path,
    handoff_receipt_binding: &PsionicTrainArtifactBinding,
    joiner_node_pubkey: &str,
) -> Result<PsionicTrainCheckpointHandoffMaterialization, PsionicTrainCheckpointHandoffError> {
    let handoff_receipt_path = resolve_artifact_binding_path(
        joiner_run_root,
        None,
        handoff_receipt_binding,
        "checkpoint_handoff.receipt",
    )?;
    let handoff_receipt_path = PathBuf::from(handoff_receipt_path);
    let mut receipt: PsionicTrainCheckpointHandoffReceipt =
        load_json(handoff_receipt_path.as_path())?;
    receipt.validate()?;
    if receipt.peer_node_pubkey != joiner_node_pubkey {
        return Err(PsionicTrainCheckpointHandoffError::Invalid {
            detail: format!(
                "checkpoint handoff receipt targets peer `{}` but the joiner is `{}`",
                receipt.peer_node_pubkey, joiner_node_pubkey
            ),
        });
    }

    let source_checkpoint_pointer_path = resolve_artifact_binding_path(
        joiner_run_root,
        Some(handoff_receipt_path.as_path()),
        &receipt.source_checkpoint_pointer,
        "checkpoint_handoff.source_checkpoint_pointer",
    )?;
    let source_checkpoint_manifest_path = resolve_artifact_binding_path(
        joiner_run_root,
        Some(handoff_receipt_path.as_path()),
        &receipt.source_checkpoint_manifest,
        "checkpoint_handoff.source_checkpoint_manifest",
    )?;
    receipt.source_checkpoint_pointer.materialized_path =
        Some(source_checkpoint_pointer_path.clone());
    receipt.source_checkpoint_manifest.materialized_path =
        Some(source_checkpoint_manifest_path.clone());
    match checkpoint_schema_version(Path::new(source_checkpoint_pointer_path.as_str()))? {
        Some(schema_version)
            if schema_version == PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION =>
        {
            materialize_generic_checkpoint_handoff(
                joiner_run_root,
                &receipt,
                Path::new(source_checkpoint_pointer_path.as_str()),
                Path::new(source_checkpoint_manifest_path.as_str()),
            )
        }
        _ => materialize_actual_pretraining_checkpoint_handoff(
            joiner_run_root,
            &receipt,
            Path::new(source_checkpoint_pointer_path.as_str()),
            Path::new(source_checkpoint_manifest_path.as_str()),
        ),
    }
}

pub fn retain_psionic_train_checkpoint_handoff_receipt(
    run_root: &Path,
    receipt: &PsionicTrainCheckpointHandoffReceipt,
) -> Result<PsionicTrainCheckpointHandoffRetention, PsionicTrainCheckpointHandoffError> {
    receipt.validate()?;
    fs::create_dir_all(run_root.join("status")).map_err(|error| {
        PsionicTrainCheckpointHandoffError::Write {
            path: run_root.join("status").display().to_string(),
            detail: error.to_string(),
        }
    })?;
    fs::create_dir_all(run_root.join(PSIONIC_TRAIN_CHECKPOINT_HANDOFF_HISTORY_RELATIVE_DIR))
        .map_err(|error| PsionicTrainCheckpointHandoffError::Write {
            path: run_root
                .join(PSIONIC_TRAIN_CHECKPOINT_HANDOFF_HISTORY_RELATIVE_DIR)
                .display()
                .to_string(),
            detail: error.to_string(),
        })?;

    let current_receipt_path =
        run_root.join(PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_RELATIVE_PATH);
    let history_receipt_path = run_root.join(format!(
        "{}/step-{:06}-{}.json",
        PSIONIC_TRAIN_CHECKPOINT_HANDOFF_HISTORY_RELATIVE_DIR,
        receipt.optimizer_step,
        short_digest(receipt.receipt_digest.as_str()),
    ));
    write_json(current_receipt_path.as_path(), receipt)?;
    write_json(history_receipt_path.as_path(), receipt)?;

    Ok(PsionicTrainCheckpointHandoffRetention {
        current_receipt_path: current_receipt_path.display().to_string(),
        history_receipt_path: history_receipt_path.display().to_string(),
    })
}

fn require_nonempty(value: &str, field: &str) -> Result<(), PsionicTrainCheckpointHandoffError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainCheckpointHandoffError::Invalid {
            detail: format!("{field} must not be empty"),
        });
    }
    Ok(())
}

fn require_nonempty_option(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionicTrainCheckpointHandoffError> {
    let value = value.ok_or_else(|| PsionicTrainCheckpointHandoffError::Invalid {
        detail: format!("{field} must not be empty"),
    })?;
    require_nonempty(value, field)
}

fn validate_grouped_stage_scope(
    window_id: Option<&str>,
    assignment_id: Option<&str>,
    grouped_stage_assignment: Option<&PsionicTrainGroupedReplicaStageAssignment>,
) -> Result<(), PsionicTrainCheckpointHandoffError> {
    if let Some(window_id) = window_id {
        require_nonempty(window_id, "checkpoint handoff window_id")?;
    }
    if let Some(assignment_id) = assignment_id {
        require_nonempty(assignment_id, "checkpoint handoff assignment_id")?;
    }
    if window_id.is_some() || assignment_id.is_some() {
        require_nonempty_option(window_id, "checkpoint handoff window_id")?;
        require_nonempty_option(assignment_id, "checkpoint handoff assignment_id")?;
    }
    if let Some(grouped_stage_assignment) = grouped_stage_assignment {
        grouped_stage_assignment
            .validate("checkpoint_handoff.grouped_stage_assignment")
            .map_err(|error| PsionicTrainCheckpointHandoffError::Invalid {
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

fn short_digest(value: &str) -> String {
    let hex = sha256_hex(value.as_bytes());
    hex[..12].to_string()
}

fn stable_digest(receipt: &PsionicTrainCheckpointHandoffReceipt) -> String {
    let encoded = serde_json::to_vec(receipt)
        .expect("checkpoint handoff receipt serialization should not fail");
    let mut digest = Sha256::new();
    digest.update(b"psionic_train_checkpoint_handoff|");
    digest.update(&encoded);
    format!("{:x}", digest.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, PsionicTrainCheckpointHandoffError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainCheckpointHandoffError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionicTrainCheckpointHandoffError::Parse {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn resolve_artifact_binding_path(
    joiner_run_root: &Path,
    handoff_receipt_path: Option<&Path>,
    binding: &PsionicTrainArtifactBinding,
    field: &str,
) -> Result<String, PsionicTrainCheckpointHandoffError> {
    let mut candidates = Vec::new();
    if let Some(materialized_path) = binding.materialized_path.as_deref() {
        push_candidate_path(&mut candidates, PathBuf::from(materialized_path));
        let materialized_path = Path::new(materialized_path);
        if materialized_path.is_relative() {
            if let Some(receipt_parent) = handoff_receipt_path.and_then(Path::parent) {
                push_candidate_path(&mut candidates, receipt_parent.join(materialized_path));
            }
            push_candidate_path(&mut candidates, joiner_run_root.join(materialized_path));
        }
    }
    if let Some(receipt_parent) = handoff_receipt_path.and_then(Path::parent) {
        let cache_key =
            psionic_train_resolved_artifact_cache_key(binding.artifact_ref.artifact_id.as_str());
        push_candidate_path(
            &mut candidates,
            receipt_parent.join(format!("{cache_key}.json")),
        );
        push_candidate_path(&mut candidates, receipt_parent.join(cache_key));
    }
    for candidate in psionic_train_resolved_artifact_cache_candidates(
        joiner_run_root,
        binding.artifact_ref.artifact_id.as_str(),
    ) {
        push_candidate_path(&mut candidates, candidate);
    }

    let mut first_invalid = None;
    let mut attempted_paths = Vec::new();
    for candidate in candidates {
        attempted_paths.push(candidate.display().to_string());
        if !candidate.is_file() {
            continue;
        }
        match validate_artifact_binding_candidate(binding, candidate.as_path(), field) {
            Ok(()) => return Ok(candidate.display().to_string()),
            Err(error @ PsionicTrainCheckpointHandoffError::Invalid { .. }) => {
                if first_invalid.is_none() {
                    first_invalid = Some(error);
                }
            }
            Err(error) => return Err(error),
        }
    }

    if let Some(error) = first_invalid {
        return Err(error);
    }

    Err(PsionicTrainCheckpointHandoffError::MissingCheckpoint {
        detail: format!(
            "{field} requires one local copy of artifact `{}`; automatic launch expects resolver-backed materialization under `{}`; checked {}",
            binding.artifact_ref.artifact_id,
            joiner_run_root
                .join(PSIONIC_TRAIN_RESOLVED_ARTIFACT_CACHE_RELATIVE_DIR)
                .display(),
            attempted_paths.join(", ")
        ),
    })
}

fn materialize_generic_checkpoint_handoff(
    joiner_run_root: &Path,
    receipt: &PsionicTrainCheckpointHandoffReceipt,
    source_checkpoint_pointer_path: &Path,
    source_checkpoint_manifest_path: &Path,
) -> Result<PsionicTrainCheckpointHandoffMaterialization, PsionicTrainCheckpointHandoffError> {
    let checkpoint_pointer: PsionicTrainCheckpointPointer =
        load_json(source_checkpoint_pointer_path)?;
    checkpoint_pointer
        .validate()
        .map_err(|error| PsionicTrainCheckpointHandoffError::Invalid {
            detail: error.to_string(),
        })?;
    let checkpoint_manifest: PsionicTrainCheckpointManifest =
        load_json(source_checkpoint_manifest_path)?;
    checkpoint_manifest.validate().map_err(|error| {
        PsionicTrainCheckpointHandoffError::Invalid {
            detail: error.to_string(),
        }
    })?;

    if checkpoint_pointer.lane_id != receipt.lane_id
        || checkpoint_manifest.lane_id != receipt.lane_id
        || checkpoint_pointer.run_id != receipt.source_run_id
        || checkpoint_pointer.window_id.as_deref() != receipt.window_id.as_deref()
        || checkpoint_pointer.assignment_id.as_deref() != receipt.assignment_id.as_deref()
        || checkpoint_pointer.grouped_stage_assignment != receipt.grouped_stage_assignment
        || checkpoint_manifest.run_id != receipt.source_run_id
        || checkpoint_manifest.window_id.as_deref() != receipt.window_id.as_deref()
        || checkpoint_manifest.assignment_id.as_deref() != receipt.assignment_id.as_deref()
        || checkpoint_manifest.grouped_stage_assignment != receipt.grouped_stage_assignment
        || checkpoint_pointer.checkpoint_manifest_relative_path
            != checkpoint_manifest.relative_manifest_path
        || checkpoint_manifest.manifest_digest != receipt.checkpoint_manifest_digest
        || checkpoint_manifest.checkpoint_object_digest != receipt.checkpoint_object_digest
        || checkpoint_manifest.checkpoint_total_bytes != receipt.checkpoint_total_bytes
    {
        return Err(PsionicTrainCheckpointHandoffError::Invalid {
            detail: String::from("checkpoint handoff receipt drifted from the source checkpoint"),
        });
    }

    let local_pointer_path =
        joiner_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    let local_manifest_path = joiner_run_root.join(&checkpoint_manifest.relative_manifest_path);
    let retention = retain_psionic_train_checkpoint_handoff_receipt(joiner_run_root, receipt)?;

    write_json(local_pointer_path.as_path(), &checkpoint_pointer)?;
    write_json(local_manifest_path.as_path(), &checkpoint_manifest)?;

    Ok(PsionicTrainCheckpointHandoffMaterialization {
        local_receipt_path: retention.current_receipt_path,
        local_checkpoint_pointer_path: local_pointer_path.display().to_string(),
        local_checkpoint_manifest_path: local_manifest_path.display().to_string(),
    })
}

fn materialize_actual_pretraining_checkpoint_handoff(
    joiner_run_root: &Path,
    receipt: &PsionicTrainCheckpointHandoffReceipt,
    source_checkpoint_pointer_path: &Path,
    source_checkpoint_manifest_path: &Path,
) -> Result<PsionicTrainCheckpointHandoffMaterialization, PsionicTrainCheckpointHandoffError> {
    let checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
        load_json(source_checkpoint_pointer_path)?;
    checkpoint_pointer
        .validate()
        .map_err(|error| PsionicTrainCheckpointHandoffError::Invalid {
            detail: format!("source checkpoint pointer is invalid: {error}"),
        })?;
    let checkpoint_manifest: PsionActualPretrainingCheckpointManifest =
        load_json(source_checkpoint_manifest_path)?;
    checkpoint_manifest.validate().map_err(|error| {
        PsionicTrainCheckpointHandoffError::Invalid {
            detail: format!("source checkpoint manifest is invalid: {error}"),
        }
    })?;

    if checkpoint_pointer.run_id != receipt.source_run_id
        || checkpoint_manifest.run_id != receipt.source_run_id
        || checkpoint_pointer
            .checkpoint_manifest_relative_path
            .as_deref()
            != Some(checkpoint_manifest.relative_manifest_path.as_str())
        || checkpoint_manifest.manifest_digest != receipt.checkpoint_manifest_digest
        || checkpoint_manifest.checkpoint_object_digest != receipt.checkpoint_object_digest
        || checkpoint_manifest.checkpoint_total_bytes != receipt.checkpoint_total_bytes
    {
        return Err(PsionicTrainCheckpointHandoffError::Invalid {
            detail: String::from("checkpoint handoff receipt drifted from the source checkpoint"),
        });
    }

    let local_pointer_path =
        joiner_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    let local_manifest_path = joiner_run_root.join(&checkpoint_manifest.relative_manifest_path);
    let retention = retain_psionic_train_checkpoint_handoff_receipt(joiner_run_root, receipt)?;

    write_json(local_pointer_path.as_path(), &checkpoint_pointer)?;
    write_json(local_manifest_path.as_path(), &checkpoint_manifest)?;

    Ok(PsionicTrainCheckpointHandoffMaterialization {
        local_receipt_path: retention.current_receipt_path,
        local_checkpoint_pointer_path: local_pointer_path.display().to_string(),
        local_checkpoint_manifest_path: local_manifest_path.display().to_string(),
    })
}

fn push_candidate_path(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn validate_artifact_binding_candidate(
    binding: &PsionicTrainArtifactBinding,
    path: &Path,
    field: &str,
) -> Result<(), PsionicTrainCheckpointHandoffError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainCheckpointHandoffError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    if let Some(expected_bytes) = binding.artifact_ref.artifact_bytes {
        let actual_bytes = u64::try_from(bytes.len()).map_err(|error| {
            PsionicTrainCheckpointHandoffError::Read {
                path: path.display().to_string(),
                detail: error.to_string(),
            }
        })?;
        if actual_bytes != expected_bytes {
            return Err(PsionicTrainCheckpointHandoffError::Invalid {
                detail: format!(
                    "{field} byte count mismatch for `{}`: expected {} but found {}",
                    path.display(),
                    expected_bytes,
                    actual_bytes
                ),
            });
        }
    }
    if let Some(expected_digest) = binding.artifact_ref.artifact_digest.as_deref() {
        let actual_digest = sha256_hex(bytes.as_slice());
        if normalize_sha256_digest(expected_digest) != actual_digest {
            return Err(PsionicTrainCheckpointHandoffError::Invalid {
                detail: format!(
                    "{field} digest mismatch for `{}`: expected `{}` but found `sha256:{}`",
                    path.display(),
                    expected_digest,
                    actual_digest
                ),
            });
        }
    }
    Ok(())
}

fn normalize_sha256_digest(value: &str) -> String {
    value
        .strip_prefix("sha256:")
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn checkpoint_schema_version(
    path: &Path,
) -> Result<Option<String>, PsionicTrainCheckpointHandoffError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| PsionicTrainCheckpointHandoffError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        PsionicTrainCheckpointHandoffError::Parse {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    let schema_version = value
        .get("schema_version")
        .and_then(|value| value.as_str())
        .map(String::from);
    if matches!(
        schema_version.as_deref(),
        Some(PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION)
    ) {
        return Ok(None);
    }
    Ok(schema_version)
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PsionicTrainCheckpointHandoffError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionicTrainCheckpointHandoffError::Write {
            path: parent.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|error| {
            PsionicTrainCheckpointHandoffError::Write {
                path: path.display().to_string(),
                detail: error.to_string(),
            }
        })?,
    )
    .map_err(|error| PsionicTrainCheckpointHandoffError::Write {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_binding(id: &str, path: &str) -> PsionicTrainArtifactBinding {
        PsionicTrainArtifactBinding {
            artifact_ref: crate::PsionicTrainArtifactRef {
                artifact_id: String::from(id),
                artifact_digest: Some(format!("sha256:{id}")),
                artifact_bytes: Some(128),
            },
            materialized_path: Some(String::from(path)),
        }
    }

    #[test]
    fn handoff_receipt_digest_ignores_local_paths() {
        let mut receipt = PsionicTrainCheckpointHandoffReceipt {
            schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_HANDOFF_RECEIPT_SCHEMA_VERSION),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            serving_node_pubkey: String::from("npub1-serving"),
            peer_node_pubkey: String::from("npub1-peer"),
            source_run_id: String::from("source-run"),
            window_id: None,
            assignment_id: None,
            grouped_stage_assignment: None,
            source_run_root: Some(String::from("/tmp/source-run")),
            source_kind: PsionicTrainCheckpointHandoffSourceKind::LivePrimaryPointer,
            checkpoint_label: String::from("accepted"),
            optimizer_step: 42,
            checkpoint_ref: String::from("checkpoint://psion/test"),
            checkpoint_manifest_digest: String::from("manifest-digest"),
            checkpoint_object_digest: String::from("object-digest"),
            checkpoint_total_bytes: 4096,
            source_checkpoint_pointer: artifact_binding(
                "artifact://checkpoint-pointer",
                "/tmp/source/checkpoints/latest_pointer.json",
            ),
            source_checkpoint_manifest: artifact_binding(
                "artifact://checkpoint-manifest",
                "/tmp/source/checkpoints/manifest.json",
            ),
            restored_from_backup: false,
            detail: String::from("test handoff"),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.stable_digest();
        let mut relocated = receipt.clone();
        relocated.source_run_root = Some(String::from("/var/tmp/relocated-source-run"));
        relocated.source_checkpoint_pointer.materialized_path =
            Some(String::from("/var/tmp/relocated/latest_pointer.json"));
        relocated.source_checkpoint_manifest.materialized_path =
            Some(String::from("/var/tmp/relocated/manifest.json"));
        assert_eq!(receipt.stable_digest(), relocated.stable_digest());
    }
}
