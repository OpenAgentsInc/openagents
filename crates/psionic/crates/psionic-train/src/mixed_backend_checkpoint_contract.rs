use std::{collections::BTreeSet, fs, path::Path};

use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind,
};
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    checkpoint_recovery::CheckpointRecoveryError, cross_provider_training_program_manifest,
    CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer, CheckpointScopeBinding,
    CheckpointScopeKind, CheckpointShardManifest, CrossBackendCudaMlxDenseMeshContract,
    CrossBackendCudaMlxDenseMeshError, CrossProviderBackendFamily,
    CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError,
    ModelArtifactFormat, TrainingPrecisionMode, TrainingRecoveryMode,
};

/// Stable schema version for the mixed-backend checkpoint contract.
pub const MIXED_BACKEND_CHECKPOINT_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.mixed_backend_checkpoint_contract.v1";
/// Stable fixture path for the mixed-backend checkpoint contract.
pub const MIXED_BACKEND_CHECKPOINT_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/mixed_backend_checkpoint_contract_v1.json";
/// Stable checker path for the mixed-backend checkpoint contract.
pub const MIXED_BACKEND_CHECKPOINT_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-mixed-backend-checkpoint-contract.sh";
/// Stable reference doc path for the mixed-backend checkpoint contract.
pub const MIXED_BACKEND_CHECKPOINT_CONTRACT_DOC_PATH: &str =
    "docs/MIXED_BACKEND_CHECKPOINT_REFERENCE.md";

const MIXED_BACKEND_CHECKPOINT_STARTED_AT_MS: u64 = 1_742_900_800_000;
const MIXED_BACKEND_CHECKPOINT_DURABLE_AT_MS: u64 = 1_742_900_800_880;
const MIXED_BACKEND_CHECKPOINT_STEP: u64 = 4_096;
const MIXED_BACKEND_CHECKPOINT_REF: &str = "checkpoint://psion/xprovider/mixed_backend/4096";

/// State role surfaced by one mixed-backend checkpoint shard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixedBackendCheckpointStateRole {
    Parameters,
    OptimizerState,
}

/// One portable backend-local state receipt retained by the mixed-backend checkpoint contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedBackendPortableStateReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Backend family represented by the receipt.
    pub backend_family: CrossProviderBackendFamily,
    /// Artifact format for the portable state.
    pub artifact_format: ModelArtifactFormat,
    /// Stable portable state-dict digest.
    pub state_dict_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Total named tensor count in the portable state.
    pub tensor_count: usize,
    /// Shared parameter precision carried by the portable state.
    pub parameter_precision: TrainingPrecisionMode,
    /// Shared optimizer-state precision carried by the portable state.
    pub optimizer_state_precision: TrainingPrecisionMode,
    /// Machine-legible detail.
    pub detail: String,
}

/// One restore disposition retained by the mixed-backend checkpoint contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixedBackendRestoreDisposition {
    Recovered,
    RestoredWithCanonicalCast,
    Refused,
}

/// One restore receipt retained by the mixed-backend checkpoint contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedBackendRestoreReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Source backend family for the restore.
    pub source_backend_family: CrossProviderBackendFamily,
    /// Target backend family for the restore.
    pub target_backend_family: CrossProviderBackendFamily,
    /// Recovery mode used by the restore.
    pub recovery_mode: TrainingRecoveryMode,
    /// Pointer digest that selected the checkpoint.
    pub checkpoint_pointer_digest: String,
    /// Portable optimizer-state rule used by the restore.
    pub optimizer_state_portability_rule: String,
    /// Final restore disposition.
    pub disposition: MixedBackendRestoreDisposition,
    /// Machine-legible detail.
    pub detail: String,
}

/// Refusal family retained by the mixed-backend checkpoint contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixedBackendCheckpointRefusalKind {
    Bf16OptimizerStateMigration,
    QuantizedCheckpointResume,
    CheckpointlessMigration,
    IncompletePortableGroupSelection,
}

/// One explicit refusal retained by the mixed-backend checkpoint contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedBackendCheckpointRefusal {
    /// Stable refusal id.
    pub refusal_id: String,
    /// Refusal kind.
    pub refusal_kind: MixedBackendCheckpointRefusalKind,
    /// Machine-legible detail.
    pub detail: String,
}

/// Canonical mixed-backend checkpoint and restore contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedBackendCheckpointContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable program manifest id.
    pub program_manifest_id: String,
    /// Stable program manifest digest.
    pub program_manifest_digest: String,
    /// Shared mixed-backend mesh contract digest.
    pub mixed_mesh_contract_digest: String,
    /// Canonical mixed-backend checkpoint manifest.
    pub checkpoint_manifest: CheckpointManifest,
    /// Canonical mixed-backend checkpoint pointer.
    pub checkpoint_pointer: CheckpointPointer,
    /// Portable backend-local state receipts retained by the contract.
    pub portable_state_receipts: Vec<MixedBackendPortableStateReceipt>,
    /// Restore receipts retained by the contract.
    pub restore_receipts: Vec<MixedBackendRestoreReceipt>,
    /// Explicit refusal set.
    pub refusal_set: Vec<MixedBackendCheckpointRefusal>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl MixedBackendCheckpointContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_mixed_backend_checkpoint_contract|", &clone)
    }

    /// Validates the contract against the retained manifest and mixed mesh contract.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        mixed_mesh: &CrossBackendCudaMlxDenseMeshContract,
    ) -> Result<(), MixedBackendCheckpointContractError> {
        if self.schema_version != MIXED_BACKEND_CHECKPOINT_CONTRACT_SCHEMA_VERSION {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    MIXED_BACKEND_CHECKPOINT_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from("program manifest identity drifted"),
            });
        }
        if self.mixed_mesh_contract_digest != mixed_mesh.contract_digest {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from("mixed_mesh_contract_digest drifted"),
            });
        }
        if self.checkpoint_manifest.checkpoint_family != manifest.checkpoint_family
            || self.checkpoint_pointer.checkpoint_family != manifest.checkpoint_family
            || self.checkpoint_pointer.manifest_digest != self.checkpoint_manifest.manifest_digest
        {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from("checkpoint family or pointer binding drifted"),
            });
        }
        if self.checkpoint_manifest.durability != CheckpointDurabilityPosture::Durable {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from("mixed-backend checkpoint manifest must stay durable"),
            });
        }
        if self.checkpoint_manifest.shards.len() != 4 {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from(
                    "mixed-backend checkpoint manifest must retain exactly four shards",
                ),
            });
        }
        let shard_families = self
            .checkpoint_manifest
            .shards
            .iter()
            .map(|shard| {
                if shard.shard_id.starts_with("cuda") {
                    CrossProviderBackendFamily::Cuda
                } else {
                    CrossProviderBackendFamily::MlxMetal
                }
            })
            .collect::<BTreeSet<_>>();
        if shard_families
            != BTreeSet::from([
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::MlxMetal,
            ])
        {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from(
                    "checkpoint shards drifted from the required CUDA plus MLX backend set",
                ),
            });
        }
        if self.portable_state_receipts.len() != 2 {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from(
                    "portable_state_receipts must retain exactly one CUDA and one MLX receipt",
                ),
            });
        }
        let receipt_families = self
            .portable_state_receipts
            .iter()
            .map(|receipt| receipt.backend_family)
            .collect::<BTreeSet<_>>();
        if receipt_families
            != BTreeSet::from([
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::MlxMetal,
            ])
        {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from(
                    "portable_state_receipts drifted from the required backend set",
                ),
            });
        }
        for receipt in &self.portable_state_receipts {
            if receipt.artifact_format != ModelArtifactFormat::Safetensors
                || receipt.parameter_precision != TrainingPrecisionMode::Fp32
                || receipt.optimizer_state_precision != TrainingPrecisionMode::Fp32
            {
                return Err(MixedBackendCheckpointContractError::InvalidContract {
                    detail: format!(
                        "portable state receipt `{}` drifted from the canonical fp32 safetensors posture",
                        receipt.receipt_id
                    ),
                });
            }
        }
        let expected_restore_set = BTreeSet::from([
            ("cuda", "cuda", MixedBackendRestoreDisposition::Recovered),
            (
                "mlx_metal",
                "mlx_metal",
                MixedBackendRestoreDisposition::Recovered,
            ),
            (
                "cuda",
                "mlx_metal",
                MixedBackendRestoreDisposition::RestoredWithCanonicalCast,
            ),
            (
                "mlx_metal",
                "cuda",
                MixedBackendRestoreDisposition::RestoredWithCanonicalCast,
            ),
            ("cuda", "mlx_metal", MixedBackendRestoreDisposition::Refused),
        ]);
        let actual_restore_set = self
            .restore_receipts
            .iter()
            .map(|receipt| {
                if receipt.checkpoint_pointer_digest != self.checkpoint_pointer.pointer_digest {
                    return Err(MixedBackendCheckpointContractError::InvalidContract {
                        detail: format!(
                            "restore receipt `{}` drifted from the canonical checkpoint pointer digest",
                            receipt.receipt_id
                        ),
                    });
                }
                Ok((
                    backend_family_label(receipt.source_backend_family),
                    backend_family_label(receipt.target_backend_family),
                    receipt.disposition,
                ))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if actual_restore_set != expected_restore_set {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from("restore_receipts drifted from the canonical recovery ladder"),
            });
        }
        let expected_refusals = BTreeSet::from([
            MixedBackendCheckpointRefusalKind::Bf16OptimizerStateMigration,
            MixedBackendCheckpointRefusalKind::QuantizedCheckpointResume,
            MixedBackendCheckpointRefusalKind::CheckpointlessMigration,
            MixedBackendCheckpointRefusalKind::IncompletePortableGroupSelection,
        ]);
        let actual_refusals = self
            .refusal_set
            .iter()
            .map(|refusal| refusal.refusal_kind)
            .collect::<BTreeSet<_>>();
        if actual_refusals != expected_refusals {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from(
                    "refusal_set drifted from the canonical mixed-backend checkpoint boundary",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(MixedBackendCheckpointContractError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

/// Errors surfaced while building or writing the mixed-backend checkpoint contract.
#[derive(Debug, Error)]
pub enum MixedBackendCheckpointContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    MixedMesh(#[from] CrossBackendCudaMlxDenseMeshError),
    #[error(transparent)]
    CheckpointRecovery(#[from] CheckpointRecoveryError),
    #[error("mixed-backend checkpoint contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Returns the canonical mixed-backend checkpoint contract.
pub fn canonical_mixed_backend_checkpoint_contract(
) -> Result<MixedBackendCheckpointContract, MixedBackendCheckpointContractError> {
    let manifest = cross_provider_training_program_manifest()?;
    let mixed_mesh = crate::canonical_cross_backend_cuda_mlx_dense_mesh_contract()?;
    let (checkpoint_manifest, checkpoint_pointer) = mixed_backend_checkpoint_manifest(&manifest)?;
    let checkpoint_pointer_digest = checkpoint_pointer.pointer_digest.clone();
    let mut contract = MixedBackendCheckpointContract {
        schema_version: String::from(MIXED_BACKEND_CHECKPOINT_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        mixed_mesh_contract_digest: mixed_mesh.contract_digest.clone(),
        checkpoint_manifest,
        checkpoint_pointer,
        portable_state_receipts: vec![
            portable_state_receipt(
                "cuda-portable-state",
                CrossProviderBackendFamily::Cuda,
                "state-dict-digest-cuda-mixed-backend-4096",
                96,
                "The CUDA rank exports one canonical fp32 safetensors state dict plus fp32 optimizer state under the shared mixed-backend checkpoint family.",
            ),
            portable_state_receipt(
                "mlx-portable-state",
                CrossProviderBackendFamily::MlxMetal,
                "state-dict-digest-mlx-mixed-backend-4096",
                96,
                "The MLX rank exports the same canonical fp32 safetensors state dict layout so later CUDA restores do not depend on backend-local conversion scripts.",
            ),
        ],
        restore_receipts: vec![
            restore_receipt(
                "restore-cuda-to-cuda",
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::Cuda,
                TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
                checkpoint_pointer_digest.as_str(),
                "restore stays on the native CUDA backend and reuses the canonical portable fp32 state dict without conversion.",
                MixedBackendRestoreDisposition::Recovered,
            ),
            restore_receipt(
                "restore-mlx-to-mlx",
                CrossProviderBackendFamily::MlxMetal,
                CrossProviderBackendFamily::MlxMetal,
                TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
                checkpoint_pointer_digest.as_str(),
                "restore stays on the native MLX backend and reuses the canonical portable fp32 state dict without conversion.",
                MixedBackendRestoreDisposition::Recovered,
            ),
            restore_receipt(
                "restore-cuda-to-mlx",
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::MlxMetal,
                TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
                checkpoint_pointer_digest.as_str(),
                "restore accepts the CUDA portable fp32 state dict and rehydrates the MLX rank through the same safetensors-backed portable state family with no manual conversion step.",
                MixedBackendRestoreDisposition::RestoredWithCanonicalCast,
            ),
            restore_receipt(
                "restore-mlx-to-cuda",
                CrossProviderBackendFamily::MlxMetal,
                CrossProviderBackendFamily::Cuda,
                TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
                checkpoint_pointer_digest.as_str(),
                "restore accepts the MLX portable fp32 state dict and rehydrates the CUDA rank through the same safetensors-backed portable state family with no manual conversion step.",
                MixedBackendRestoreDisposition::RestoredWithCanonicalCast,
            ),
            restore_receipt(
                "restore-cuda-bf16-to-mlx-refused",
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::MlxMetal,
                TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
                checkpoint_pointer_digest.as_str(),
                "restore refuses when the checkpoint was authored under a BF16 optimizer-state posture because the shared mixed-backend checkpoint family is still fp32-only.",
                MixedBackendRestoreDisposition::Refused,
            ),
        ],
        refusal_set: vec![
            refusal(
                "mixed_backend_checkpoint.bf16_optimizer_state_migration",
                MixedBackendCheckpointRefusalKind::Bf16OptimizerStateMigration,
                "The mixed-backend checkpoint contract refuses BF16 optimizer-state migration until the shared CUDA-plus-MLX mesh stops using the fp32-only reference law.",
            ),
            refusal(
                "mixed_backend_checkpoint.quantized_checkpoint_resume",
                MixedBackendCheckpointRefusalKind::QuantizedCheckpointResume,
                "The mixed-backend checkpoint contract refuses quantized checkpoint resume because the canonical portable state family remains dense fp32 safetensors.",
            ),
            refusal(
                "mixed_backend_checkpoint.checkpointless_migration",
                MixedBackendCheckpointRefusalKind::CheckpointlessMigration,
                "The mixed-backend checkpoint contract refuses checkpointless migration. Backend changes must go through the canonical mixed-backend checkpoint pointer and manifest.",
            ),
            refusal(
                "mixed_backend_checkpoint.incomplete_portable_group_selection",
                MixedBackendCheckpointRefusalKind::IncompletePortableGroupSelection,
                "The mixed-backend checkpoint contract refuses partial portable group selection so optimizer-state portability cannot silently drop one tensor out of a training-group assignment.",
            ),
        ],
        claim_boundary: String::from(
            "This contract closes one typed mixed-backend checkpoint family for CUDA plus MLX dense runs: canonical fp32 safetensors-backed portable state, a shared checkpoint manifest and pointer, same-backend resume, cross-backend restore through canonical portable state, and one explicit refusal set for unsupported migrations. It does not claim low-precision mixed-backend portability or a real proof run by itself.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate(&manifest, &mixed_mesh)?;
    Ok(contract)
}

/// Writes the canonical mixed-backend checkpoint contract fixture.
pub fn write_mixed_backend_checkpoint_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), MixedBackendCheckpointContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            MixedBackendCheckpointContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&canonical_mixed_backend_checkpoint_contract()?)?;
    fs::write(output_path, bytes).map_err(|error| MixedBackendCheckpointContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn mixed_backend_checkpoint_manifest(
    manifest: &CrossProviderTrainingProgramManifest,
) -> Result<(CheckpointManifest, CheckpointPointer), MixedBackendCheckpointContractError> {
    let checkpoint = TrainingCheckpointReference::new(
        manifest.checkpoint_family.clone(),
        "stream-psion-xprovider-mixed-backend-step-4096",
        "manifest-digest-psion-xprovider-mixed-backend-step-4096",
        "object-digest-psion-xprovider-mixed-backend-step-4096",
        "coordinator-rank-0",
        2,
        "cluster-state-digest-psion-xprovider-mixed-backend-step-4096",
        "topology-digest-psion-xprovider-mixed-backend-step-4096",
        MIXED_BACKEND_CHECKPOINT_STARTED_AT_MS,
    )
    .with_checkpoint_ref(MIXED_BACKEND_CHECKPOINT_REF)
    .with_step(MIXED_BACKEND_CHECKPOINT_STEP)
    .with_durable_at_ms(MIXED_BACKEND_CHECKPOINT_DURABLE_AT_MS);
    let scope = CheckpointScopeBinding::new(
        CheckpointScopeKind::Run,
        "psion-xprovider-mixed-backend-4096",
    );
    let shards = vec![
        checkpoint_shard(
            "cuda-parameters",
            "cuda-rank-0",
            &manifest.checkpoint_family,
        ),
        checkpoint_shard(
            "cuda-optimizer-state",
            "cuda-rank-0",
            &manifest.checkpoint_family,
        ),
        checkpoint_shard("mlx-parameters", "mlx-rank-0", &manifest.checkpoint_family),
        checkpoint_shard(
            "mlx-optimizer-state",
            "mlx-rank-0",
            &manifest.checkpoint_family,
        ),
    ];
    let manifest = CheckpointManifest::new(
        scope.clone(),
        manifest.checkpoint_family.clone(),
        checkpoint.clone(),
        shards,
        CheckpointDurabilityPosture::Durable,
        MIXED_BACKEND_CHECKPOINT_DURABLE_AT_MS,
    )?;
    let pointer = CheckpointPointer::new(
        scope,
        manifest.checkpoint_family.clone(),
        checkpoint,
        manifest.manifest_digest.clone(),
        MIXED_BACKEND_CHECKPOINT_DURABLE_AT_MS + 12,
    )?;
    Ok((manifest, pointer))
}

fn checkpoint_shard(
    shard_id: &str,
    writer_node_id: &str,
    checkpoint_family: &str,
) -> CheckpointShardManifest {
    CheckpointShardManifest {
        shard_id: shard_id.to_string(),
        manifest: shard_manifest_ref(shard_id, checkpoint_family),
        writer_node_id: writer_node_id.to_string(),
    }
}

fn shard_manifest_ref(shard_id: &str, checkpoint_family: &str) -> DatastreamManifestRef {
    let payload =
        format!("psionic|mixed_backend_checkpoint|{shard_id}|step:{MIXED_BACKEND_CHECKPOINT_STEP}");
    DatastreamManifest::from_bytes(
        format!("stream-{shard_id}"),
        DatastreamSubjectKind::Checkpoint,
        payload.as_bytes(),
        16,
        DatastreamEncoding::Safetensors,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new(checkpoint_family)
            .with_checkpoint_ref(MIXED_BACKEND_CHECKPOINT_REF)
            .with_step(MIXED_BACKEND_CHECKPOINT_STEP),
    )
    .manifest_ref()
}

fn portable_state_receipt(
    receipt_id: &str,
    backend_family: CrossProviderBackendFamily,
    state_dict_digest: &str,
    tensor_count: usize,
    detail: &str,
) -> MixedBackendPortableStateReceipt {
    MixedBackendPortableStateReceipt {
        receipt_id: receipt_id.to_string(),
        backend_family,
        artifact_format: ModelArtifactFormat::Safetensors,
        state_dict_digest: state_dict_digest.to_string(),
        tokenizer_contract_digest: String::from(
            "978a733f8b8f48aa9d77ea658fd640b6d61a40c18bd3f692cad93065237fff51",
        ),
        tensor_count,
        parameter_precision: TrainingPrecisionMode::Fp32,
        optimizer_state_precision: TrainingPrecisionMode::Fp32,
        detail: detail.to_string(),
    }
}

fn restore_receipt(
    receipt_id: &str,
    source_backend_family: CrossProviderBackendFamily,
    target_backend_family: CrossProviderBackendFamily,
    recovery_mode: TrainingRecoveryMode,
    checkpoint_pointer_digest: &str,
    detail: &str,
    disposition: MixedBackendRestoreDisposition,
) -> MixedBackendRestoreReceipt {
    let portability_rule = match disposition {
        MixedBackendRestoreDisposition::Recovered => {
            "native_backend_resume_from_canonical_fp32_portable_state"
        }
        MixedBackendRestoreDisposition::RestoredWithCanonicalCast => {
            "cross_backend_restore_through_canonical_fp32_portable_state"
        }
        MixedBackendRestoreDisposition::Refused => "mixed_backend_restore_refused",
    };
    MixedBackendRestoreReceipt {
        receipt_id: receipt_id.to_string(),
        source_backend_family,
        target_backend_family,
        recovery_mode,
        checkpoint_pointer_digest: checkpoint_pointer_digest.to_string(),
        optimizer_state_portability_rule: portability_rule.to_string(),
        disposition,
        detail: detail.to_string(),
    }
}

fn refusal(
    refusal_id: &str,
    refusal_kind: MixedBackendCheckpointRefusalKind,
    detail: &str,
) -> MixedBackendCheckpointRefusal {
    MixedBackendCheckpointRefusal {
        refusal_id: refusal_id.to_string(),
        refusal_kind,
        detail: detail.to_string(),
    }
}

fn backend_family_label(backend_family: CrossProviderBackendFamily) -> &'static str {
    match backend_family {
        CrossProviderBackendFamily::Cuda => "cuda",
        CrossProviderBackendFamily::MlxMetal => "mlx_metal",
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization must succeed"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_mixed_backend_checkpoint_contract, MixedBackendCheckpointRefusalKind,
        MixedBackendRestoreDisposition,
    };

    #[test]
    fn canonical_contract_is_self_digesting() -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_mixed_backend_checkpoint_contract()?;
        assert_eq!(contract.contract_digest, contract.stable_digest());
        Ok(())
    }

    #[test]
    fn canonical_contract_retains_cross_backend_restore_receipts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_mixed_backend_checkpoint_contract()?;
        assert!(contract.restore_receipts.iter().any(|receipt| {
            receipt.disposition == MixedBackendRestoreDisposition::RestoredWithCanonicalCast
        }));
        assert!(contract.refusal_set.iter().any(|refusal| {
            refusal.refusal_kind == MixedBackendCheckpointRefusalKind::Bf16OptimizerStateMigration
        }));
        Ok(())
    }
}
