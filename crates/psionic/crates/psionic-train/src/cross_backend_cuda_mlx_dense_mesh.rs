use std::{collections::BTreeSet, fs, path::Path};

use psionic_runtime::{TrainingCollectiveKind, TrainingCollectiveQuantization};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    cross_provider_training_program_manifest, CrossProviderBackendFamily,
    CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError,
    DenseRankRuntimeReferenceContract, MlxDenseRankRuntimeContract,
    TrainingDistributedOptimizerKind, TrainingOptimizerKind, TrainingPrecisionMode,
    TrainingPrecisionPolicy,
};

/// Stable schema version for the CUDA-plus-MLX dense mesh contract.
pub const CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.cross_backend_cuda_mlx_dense_mesh_contract.v1";
/// Stable fixture path for the CUDA-plus-MLX dense mesh contract.
pub const CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/cross_backend_cuda_mlx_dense_mesh_contract_v1.json";
/// Stable checker path for the CUDA-plus-MLX dense mesh contract.
pub const CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-backend-cuda-mlx-dense-mesh-contract.sh";
/// Stable reference doc path for the CUDA-plus-MLX dense mesh contract.
pub const CROSS_BACKEND_CUDA_MLX_DENSE_MESH_DOC_PATH: &str =
    "docs/CROSS_BACKEND_CUDA_MLX_DENSE_MESH_REFERENCE.md";

/// One backend participant in the shared CUDA-plus-MLX dense mesh contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBackendDenseMeshParticipant {
    /// Stable participant id.
    pub participant_id: String,
    /// Backend family bound by the participant.
    pub backend_family: CrossProviderBackendFamily,
    /// Runtime family id exposed by the participant.
    pub runtime_family_id: String,
    /// Runtime contract digest exposed by the participant.
    pub runtime_contract_digest: String,
    /// Local collective realization retained by the participant.
    pub local_collective_realization: String,
    /// Master-weight role retained by the participant.
    pub master_weight_role: String,
}

/// Shared collective contract across CUDA and MLX dense ranks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBackendCollectiveContract {
    /// Canonical collective kind for gradient exchange.
    pub gradient_collective_kind: TrainingCollectiveKind,
    /// Canonical collective kind for master-weight broadcast.
    pub master_weight_collective_kind: TrainingCollectiveKind,
    /// Canonical reduction precision.
    pub reduction_precision: TrainingPrecisionMode,
    /// Canonical communication quantization.
    pub communication_quantization: TrainingCollectiveQuantization,
    /// Canonical bridge detail.
    pub bridge_detail: String,
}

/// Shared optimizer and master-weight ownership contract across CUDA and MLX dense ranks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBackendOptimizerOwnershipContract {
    /// Distributed optimizer family retained by the shared contract.
    pub distributed_optimizer_kind: TrainingDistributedOptimizerKind,
    /// Per-parameter optimizer family retained by the shared contract.
    pub optimizer_kind: TrainingOptimizerKind,
    /// Master-weight ownership posture.
    pub master_weight_authority: String,
    /// Optimizer-state ownership posture.
    pub optimizer_state_authority: String,
    /// Step-commit posture.
    pub step_commit_posture: String,
    /// Machine-legible detail.
    pub detail: String,
}

/// Refusal family retained by the shared CUDA-plus-MLX dense mesh contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossBackendDenseMeshRefusalKind {
    /// BF16 mixed precision is not admitted across CUDA and MLX yet.
    Bf16MixedPrecision,
    /// FP16 loss scaling is not admitted across CUDA and MLX yet.
    Fp16DynamicLossScaling,
    /// Direct NCCL participation by MLX ranks is refused.
    DirectNcclParticipationByMlxRank,
    /// Split master-weight ownership across backends is refused.
    SplitMasterWeightAuthority,
    /// Checkpointless optimizer-state migration is refused.
    CheckpointlessOptimizerMigration,
}

/// One explicit refusal retained by the shared CUDA-plus-MLX dense mesh contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBackendDenseMeshRefusal {
    /// Stable refusal id.
    pub refusal_id: String,
    /// Refusal kind.
    pub refusal_kind: CrossBackendDenseMeshRefusalKind,
    /// Machine-legible detail.
    pub detail: String,
}

/// Canonical shared CUDA-plus-MLX dense mesh contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossBackendCudaMlxDenseMeshContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable program manifest id.
    pub program_manifest_id: String,
    /// Stable program manifest digest.
    pub program_manifest_digest: String,
    /// Backend participants admitted by the contract.
    pub participant_backends: Vec<CrossBackendDenseMeshParticipant>,
    /// Shared collective contract.
    pub collective_contract: CrossBackendCollectiveContract,
    /// Shared precision policy.
    pub precision_policy: TrainingPrecisionPolicy,
    /// Shared optimizer and master-weight ownership contract.
    pub optimizer_ownership: CrossBackendOptimizerOwnershipContract,
    /// Explicit later issues or surfaces required by a real proof run.
    pub proof_preconditions: Vec<String>,
    /// Explicit refusal set.
    pub refusal_set: Vec<CrossBackendDenseMeshRefusal>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl CrossBackendCudaMlxDenseMeshContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(
            b"psionic_cross_backend_cuda_mlx_dense_mesh_contract|",
            &clone,
        )
    }

    /// Validates the contract against the retained program manifest and runtime references.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        cuda_reference: &DenseRankRuntimeReferenceContract,
        mlx_reference: &MlxDenseRankRuntimeContract,
    ) -> Result<(), CrossBackendCudaMlxDenseMeshError> {
        if self.schema_version != CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_SCHEMA_VERSION {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from("program manifest identity drifted"),
            });
        }
        if self.participant_backends.len() != 2 {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from(
                    "participant_backends must retain exactly one CUDA and one MLX participant",
                ),
            });
        }
        let families = self
            .participant_backends
            .iter()
            .map(|participant| participant.backend_family)
            .collect::<BTreeSet<_>>();
        if families
            != BTreeSet::from([
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::MlxMetal,
            ])
        {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from(
                    "participant_backends drifted from the required CUDA plus MLX family set",
                ),
            });
        }
        let cuda_participant = self
            .participant_backends
            .iter()
            .find(|participant| participant.backend_family == CrossProviderBackendFamily::Cuda)
            .ok_or_else(|| CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from("missing CUDA participant"),
            })?;
        let mlx_participant = self
            .participant_backends
            .iter()
            .find(|participant| participant.backend_family == CrossProviderBackendFamily::MlxMetal)
            .ok_or_else(|| CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from("missing MLX participant"),
            })?;
        if cuda_participant.runtime_family_id != cuda_reference.runtime.runtime_family_id
            || cuda_participant.runtime_contract_digest != cuda_reference.contract_digest
        {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from(
                    "CUDA participant drifted from the generic dense CUDA reference",
                ),
            });
        }
        if mlx_participant.runtime_family_id != mlx_reference.runtime.runtime_family_id
            || mlx_participant.runtime_contract_digest != mlx_reference.contract_digest
        {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from(
                    "MLX participant drifted from the MLX dense-rank runtime reference",
                ),
            });
        }
        if self.collective_contract.gradient_collective_kind != TrainingCollectiveKind::AllReduce
            || self.collective_contract.master_weight_collective_kind
                != TrainingCollectiveKind::Broadcast
            || self.collective_contract.reduction_precision != TrainingPrecisionMode::Fp32
            || self.collective_contract.communication_quantization
                != TrainingCollectiveQuantization::None
        {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from("collective_contract drifted from the canonical fp32 all-reduce/broadcast contract"),
            });
        }
        let expected_precision = TrainingPrecisionPolicy {
            parameter_precision: TrainingPrecisionMode::Fp32,
            gradient_precision: TrainingPrecisionMode::Fp32,
            optimizer_state_precision: TrainingPrecisionMode::Fp32,
            master_weight_precision: TrainingPrecisionMode::Fp32,
            reduction_precision: TrainingPrecisionMode::Fp32,
            communication_quantization: TrainingCollectiveQuantization::None,
            stochastic_rounding: false,
            loss_scale: None,
        };
        if self.precision_policy != expected_precision {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from(
                    "precision_policy drifted from the canonical fp32 shared policy",
                ),
            });
        }
        if self.optimizer_ownership.distributed_optimizer_kind
            != TrainingDistributedOptimizerKind::DataParallel
            || self.optimizer_ownership.optimizer_kind != TrainingOptimizerKind::AdamW
        {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from("optimizer_ownership drifted from the canonical mirrored AdamW data-parallel posture"),
            });
        }
        let required_refusals = BTreeSet::from([
            CrossBackendDenseMeshRefusalKind::Bf16MixedPrecision,
            CrossBackendDenseMeshRefusalKind::Fp16DynamicLossScaling,
            CrossBackendDenseMeshRefusalKind::DirectNcclParticipationByMlxRank,
            CrossBackendDenseMeshRefusalKind::SplitMasterWeightAuthority,
            CrossBackendDenseMeshRefusalKind::CheckpointlessOptimizerMigration,
        ]);
        let actual_refusals = self
            .refusal_set
            .iter()
            .map(|refusal| refusal.refusal_kind)
            .collect::<BTreeSet<_>>();
        if actual_refusals != required_refusals {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from(
                    "refusal_set drifted from the canonical mixed-backend refusal boundary",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(CrossBackendCudaMlxDenseMeshError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

/// Errors surfaced while building or writing the CUDA-plus-MLX dense mesh contract.
#[derive(Debug, Error)]
pub enum CrossBackendCudaMlxDenseMeshError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    MlxRuntime(#[from] crate::MlxDenseRankRuntimeError),
    #[error("cross-backend CUDA-plus-MLX dense mesh contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Returns the canonical CUDA-plus-MLX dense mesh contract.
pub fn canonical_cross_backend_cuda_mlx_dense_mesh_contract(
) -> Result<CrossBackendCudaMlxDenseMeshContract, CrossBackendCudaMlxDenseMeshError> {
    let manifest = cross_provider_training_program_manifest()?;
    let cuda_reference = crate::dense_rank_runtime_reference_contract();
    let mlx_reference = crate::canonical_mlx_dense_rank_runtime_contract()?;
    let mut contract = CrossBackendCudaMlxDenseMeshContract {
        schema_version: String::from(CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        participant_backends: vec![
            CrossBackendDenseMeshParticipant {
                participant_id: String::from("cuda_dense_reference"),
                backend_family: CrossProviderBackendFamily::Cuda,
                runtime_family_id: cuda_reference.runtime.runtime_family_id.clone(),
                runtime_contract_digest: cuda_reference.contract_digest.clone(),
                local_collective_realization: String::from(
                    "CUDA ranks realize the shared gradient contract through the generic dense CUDA runtime and an NCCL-compatible fp32 all-reduce surface.",
                ),
                master_weight_role: String::from(
                    "Each CUDA rank holds a mirrored fp32 master-weight view and may only apply the optimizer step after the shared fp32 gradient barrier.",
                ),
            },
            CrossBackendDenseMeshParticipant {
                participant_id: String::from("mlx_dense_reference"),
                backend_family: CrossProviderBackendFamily::MlxMetal,
                runtime_family_id: mlx_reference.runtime.runtime_family_id.clone(),
                runtime_contract_digest: mlx_reference.contract_digest.clone(),
                local_collective_realization: String::from(
                    "MLX ranks canonicalize local gradients into the shared fp32 bridge format instead of claiming native NCCL participation or backend-local quantized exchange.",
                ),
                master_weight_role: String::from(
                    "Each MLX rank mirrors the same fp32 master-weight state and may only apply the optimizer step after the shared fp32 gradient barrier.",
                ),
            },
        ],
        collective_contract: CrossBackendCollectiveContract {
            gradient_collective_kind: TrainingCollectiveKind::AllReduce,
            master_weight_collective_kind: TrainingCollectiveKind::Broadcast,
            reduction_precision: TrainingPrecisionMode::Fp32,
            communication_quantization: TrainingCollectiveQuantization::None,
            bridge_detail: String::from(
                "The first mixed CUDA-plus-MLX dense mesh uses one canonical fp32 gradient all-reduce plus fp32 master-weight broadcast contract. Backend-local kernels may differ, but the shared wire contract, step barrier, and math surface stay identical.",
            ),
        },
        precision_policy: TrainingPrecisionPolicy {
            parameter_precision: TrainingPrecisionMode::Fp32,
            gradient_precision: TrainingPrecisionMode::Fp32,
            optimizer_state_precision: TrainingPrecisionMode::Fp32,
            master_weight_precision: TrainingPrecisionMode::Fp32,
            reduction_precision: TrainingPrecisionMode::Fp32,
            communication_quantization: TrainingCollectiveQuantization::None,
            stochastic_rounding: false,
            loss_scale: None,
        },
        optimizer_ownership: CrossBackendOptimizerOwnershipContract {
            distributed_optimizer_kind: TrainingDistributedOptimizerKind::DataParallel,
            optimizer_kind: TrainingOptimizerKind::AdamW,
            master_weight_authority: String::from(
                "Mirrored fp32 master weights on every rank with one shared post-all-reduce step barrier.",
            ),
            optimizer_state_authority: String::from(
                "Mirrored fp32 AdamW state on every rank until the later mixed-backend checkpoint contract lands.",
            ),
            step_commit_posture: String::from(
                "Every rank applies the same optimizer step only after the shared fp32 gradient barrier and the canonical step hash agree.",
            ),
            detail: String::from(
                "The first mixed CUDA-plus-MLX dense mesh stays on the simplest honest optimizer law: mirrored fp32 AdamW state and mirrored fp32 master weights on every rank. Later sharded or checkpoint-portable optimizer ownership remains a separate issue.",
            ),
        },
        proof_preconditions: vec![
            String::from("#536 MLX dense-rank runtime parity"),
            String::from("#538 mixed-backend checkpoint and restore parity"),
            String::from("#539 first same-job CUDA-plus-MLX dense proof run"),
        ],
        refusal_set: vec![
            refusal(
                "cross_backend_cuda_mlx_dense.bf16_mixed_precision",
                CrossBackendDenseMeshRefusalKind::Bf16MixedPrecision,
                "The first shared CUDA-plus-MLX dense mesh refuses BF16 mixed precision and keeps one fp32 reference law so backend-local numerical differences do not leak into the shared step contract.",
            ),
            refusal(
                "cross_backend_cuda_mlx_dense.fp16_dynamic_loss_scaling",
                CrossBackendDenseMeshRefusalKind::Fp16DynamicLossScaling,
                "The first shared CUDA-plus-MLX dense mesh refuses fp16 loss scaling because MLX and CUDA must first agree on one typed overflow and underflow contract before low-precision scaling is admissible.",
            ),
            refusal(
                "cross_backend_cuda_mlx_dense.direct_nccl_participation_by_mlx_rank",
                CrossBackendDenseMeshRefusalKind::DirectNcclParticipationByMlxRank,
                "The shared mixed-backend mesh refuses to pretend an MLX Metal rank can participate in NCCL directly. MLX gradients must first canonicalize into the shared fp32 bridge format.",
            ),
            refusal(
                "cross_backend_cuda_mlx_dense.split_master_weight_authority",
                CrossBackendDenseMeshRefusalKind::SplitMasterWeightAuthority,
                "The shared mixed-backend mesh refuses split master-weight authority. CUDA and MLX ranks must carry the same mirrored fp32 master weights and the same post-barrier step hash.",
            ),
            refusal(
                "cross_backend_cuda_mlx_dense.checkpointless_optimizer_migration",
                CrossBackendDenseMeshRefusalKind::CheckpointlessOptimizerMigration,
                "The shared mixed-backend mesh refuses checkpointless optimizer-state migration. Backend transitions must go through the later typed mixed-backend checkpoint family.",
            ),
        ],
        claim_boundary: String::from(
            "This contract closes one typed CUDA-plus-MLX dense mesh math law: fp32 gradient all-reduce, fp32 master-weight broadcast, mirrored fp32 AdamW state, and one explicit refusal set for unsupported mixed-backend operations. It does not claim checkpoint portability or a real proof run by itself.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate(&manifest, &cuda_reference, &mlx_reference)?;
    Ok(contract)
}

/// Writes the canonical CUDA-plus-MLX dense mesh contract fixture.
pub fn write_cross_backend_cuda_mlx_dense_mesh_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), CrossBackendCudaMlxDenseMeshError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CrossBackendCudaMlxDenseMeshError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes =
        serde_json::to_vec_pretty(&canonical_cross_backend_cuda_mlx_dense_mesh_contract()?)?;
    fs::write(output_path, bytes).map_err(|error| CrossBackendCudaMlxDenseMeshError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn refusal(
    refusal_id: &str,
    refusal_kind: CrossBackendDenseMeshRefusalKind,
    detail: &str,
) -> CrossBackendDenseMeshRefusal {
    CrossBackendDenseMeshRefusal {
        refusal_id: refusal_id.to_string(),
        refusal_kind,
        detail: detail.to_string(),
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
        canonical_cross_backend_cuda_mlx_dense_mesh_contract, CrossBackendDenseMeshRefusalKind,
    };

    #[test]
    fn canonical_contract_is_self_digesting() -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_cross_backend_cuda_mlx_dense_mesh_contract()?;
        assert_eq!(contract.contract_digest, contract.stable_digest());
        Ok(())
    }

    #[test]
    fn canonical_contract_keeps_bf16_mixed_refused() -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_cross_backend_cuda_mlx_dense_mesh_contract()?;
        assert!(contract.refusal_set.iter().any(|refusal| {
            refusal.refusal_kind == CrossBackendDenseMeshRefusalKind::Bf16MixedPrecision
        }));
        Ok(())
    }
}
