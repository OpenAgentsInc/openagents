use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind,
};
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, cross_provider_training_program_manifest,
    CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer, CheckpointRecoveryError,
    CheckpointScopeBinding, CheckpointScopeKind, CheckpointShardManifest,
    CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError,
    CrossProviderExecutionClass, CrossProviderTrainingProgramManifest,
    CrossProviderTrainingProgramManifestError, TrainingRecoveryMode,
    CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY,
};

/// Stable schema version for the first provider-neutral distributed checkpoint contract.
pub const SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.sharded_distributed_checkpoint_contract.v1";
/// Stable fixture path for the canonical distributed checkpoint contract.
pub const SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/sharded_distributed_checkpoint_contract_v1.json";
/// Stable checker path for the canonical distributed checkpoint contract.
pub const SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-sharded-distributed-checkpoint-contract.sh";
/// Stable reference doc path for the canonical distributed checkpoint contract.
pub const SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_DOC_PATH: &str =
    "docs/SHARDED_DISTRIBUTED_CHECKPOINT_REFERENCE.md";

const CANONICAL_DISTRIBUTED_CHECKPOINT_STEP: u64 = 2_048;
const CANONICAL_DISTRIBUTED_CHECKPOINT_CREATED_AT_MS: u64 = 1_742_705_120_000;
const CANONICAL_DISTRIBUTED_CHECKPOINT_UPDATED_AT_MS: u64 = 1_742_705_120_600;
const CANONICAL_DISTRIBUTED_CHECKPOINT_STORAGE_ROOT: &str =
    "runs/psion-xprovider-pretrain-step-2048/checkpoints";

/// Error surfaced while building, validating, or writing the distributed checkpoint contract.
#[derive(Debug, Error)]
pub enum DistributedCheckpointContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    CheckpointRecovery(#[from] CheckpointRecoveryError),
    #[error(transparent)]
    ComputeSource(#[from] CrossProviderComputeSourceContractError),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error("distributed checkpoint contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Shard role inside the distributed checkpoint contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCheckpointShardRole {
    /// Dense model parameter shard.
    ParameterState,
    /// Optimizer-state shard coupled to one dense rank.
    OptimizerState,
}

/// One distributed checkpoint shard with its logical owner and role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedCheckpointShardPlacement {
    /// Stable shard id.
    pub shard_id: String,
    /// Logical shard role.
    pub shard_role: DistributedCheckpointShardRole,
    /// Dense rank that owns this shard.
    pub dense_rank: u16,
    /// Producing training source for the shard.
    pub producing_source_id: String,
    /// Writer node that emitted the original shard bytes.
    pub writer_node_id: String,
    /// Stable lineage slot for later artifact evidence.
    pub lineage_slot_id: String,
    /// Datastream-backed shard ref.
    pub manifest: DatastreamManifestRef,
}

/// Outcome for one shard upload attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCheckpointShardUploadDisposition {
    /// Upload completed durably.
    Durable,
    /// Upload was refused because it ended in a partial state.
    PartialUploadRefused,
}

/// One typed upload receipt over one checkpoint shard attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedCheckpointShardUploadReceipt {
    /// Stable receipt id.
    pub receipt_id: String,
    /// Shard targeted by this upload.
    pub shard_id: String,
    /// Attempt ordinal for the shard.
    pub attempt_ordinal: u16,
    /// Source that owned the upload worker.
    pub uploader_source_id: String,
    /// Checkpoint storage root selected for this upload.
    pub storage_root: String,
    /// Final upload disposition.
    pub upload_disposition: DistributedCheckpointShardUploadDisposition,
    /// Bytes committed before completion or refusal.
    pub bytes_committed: u64,
    /// Machine-legible detail.
    pub detail: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl DistributedCheckpointShardUploadReceipt {
    /// Returns the stable digest for this receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_digest(b"psionic_distributed_checkpoint_upload_receipt|", &clone)
    }
}

/// One dense-rank restore assignment over the distributed checkpoint shards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedCheckpointRestoreAssignment {
    /// Stable restore assignment id.
    pub assignment_id: String,
    /// Dense rank restored by this assignment.
    pub dense_rank: u16,
    /// Restore source that will materialize the shards.
    pub restore_source_id: String,
    /// Requested execution class for the restore worker.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Parameter shard restored onto the rank.
    pub parameter_shard_id: String,
    /// Optimizer-state shard restored onto the rank.
    pub optimizer_shard_id: String,
    /// Stable shard load order.
    pub load_order: Vec<String>,
    /// Machine-legible detail.
    pub detail: String,
}

/// Deterministic restore plan over the distributed checkpoint shards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedCheckpointRestorePlan {
    /// Selected recovery mode.
    pub recovery_mode: TrainingRecoveryMode,
    /// Source that owns pointer and manifest authority for restore.
    pub manifest_authority_source_id: String,
    /// Dense-rank restore assignments.
    pub assignments: Vec<DistributedCheckpointRestoreAssignment>,
    /// Stable restore-plan digest.
    pub restore_plan_digest: String,
}

impl DistributedCheckpointRestorePlan {
    /// Returns the stable digest for the restore-plan payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.restore_plan_digest.clear();
        stable_digest(b"psionic_distributed_checkpoint_restore_plan|", &clone)
    }
}

/// Canonical provider-neutral distributed checkpoint contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardedDistributedCheckpointContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable cross-provider program manifest id.
    pub program_manifest_id: String,
    /// Stable cross-provider program manifest digest.
    pub program_manifest_digest: String,
    /// Root checkpoint manifest carried by the distributed checkpoint.
    pub checkpoint_manifest: CheckpointManifest,
    /// Root checkpoint pointer carried by the distributed checkpoint.
    pub checkpoint_pointer: CheckpointPointer,
    /// Explicit shard placements with role identity.
    pub shard_placements: Vec<DistributedCheckpointShardPlacement>,
    /// Upload receipts covering durable and refused attempts.
    pub shard_upload_receipts: Vec<DistributedCheckpointShardUploadReceipt>,
    /// Deterministic restore plan over the durable shards.
    pub restore_plan: DistributedCheckpointRestorePlan,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl ShardedDistributedCheckpointContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_sharded_distributed_checkpoint_contract|", &clone)
    }

    /// Validates the contract against the canonical manifest and source contracts.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        source_contracts: &[CrossProviderComputeSourceContract],
    ) -> Result<(), DistributedCheckpointContractError> {
        if self.schema_version != SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_SCHEMA_VERSION {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("program_manifest_id drifted from the root manifest"),
            });
        }
        if self.program_manifest_digest != manifest.program_manifest_digest {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("program_manifest_digest drifted from the root manifest"),
            });
        }
        if self.checkpoint_manifest.checkpoint_family != manifest.checkpoint_family
            || self.checkpoint_manifest.checkpoint_family
                != CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY
        {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("checkpoint family drifted from the canonical program family"),
            });
        }
        if self.checkpoint_pointer.checkpoint_family != self.checkpoint_manifest.checkpoint_family {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("checkpoint pointer family drifted from the manifest family"),
            });
        }
        if self.checkpoint_pointer.manifest_digest != self.checkpoint_manifest.manifest_digest {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from(
                    "checkpoint pointer no longer points at the selected manifest",
                ),
            });
        }
        if self.checkpoint_manifest.durability != CheckpointDurabilityPosture::Durable {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("distributed checkpoint manifest must be durable"),
            });
        }

        let sources_by_id = source_contracts
            .iter()
            .map(|contract| (contract.source_id.as_str(), contract))
            .collect::<BTreeMap<_, _>>();
        let manifest_shards = self
            .checkpoint_manifest
            .shards
            .iter()
            .map(|shard| (shard.shard_id.as_str(), shard))
            .collect::<BTreeMap<_, _>>();
        if manifest_shards.len() != self.shard_placements.len() {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from(
                    "checkpoint_manifest.shards and shard_placements must stay in one-to-one correspondence",
                ),
            });
        }
        let mut parameter_ranks = BTreeSet::new();
        let mut optimizer_ranks = BTreeSet::new();
        let mut placement_ids = BTreeSet::new();
        for placement in &self.shard_placements {
            let shard = manifest_shards
                .get(placement.shard_id.as_str())
                .ok_or_else(|| DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "shard placement `{}` is missing from checkpoint_manifest.shards",
                        placement.shard_id
                    ),
                })?;
            if shard.writer_node_id != placement.writer_node_id
                || shard.manifest != placement.manifest
            {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "shard placement `{}` drifted from the root checkpoint manifest",
                        placement.shard_id
                    ),
                });
            }
            if !placement_ids.insert(placement.shard_id.clone()) {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!("duplicate shard placement `{}`", placement.shard_id),
                });
            }
            validate_source_admits(
                &sources_by_id,
                placement.producing_source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::DenseFullModelRank,
            )?;
            match placement.shard_role {
                DistributedCheckpointShardRole::ParameterState => {
                    parameter_ranks.insert(placement.dense_rank);
                }
                DistributedCheckpointShardRole::OptimizerState => {
                    optimizer_ranks.insert(placement.dense_rank);
                }
            }
        }
        if parameter_ranks.is_empty() || optimizer_ranks.is_empty() {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from(
                    "distributed checkpoint contract must retain both parameter and optimizer shards",
                ),
            });
        }
        if parameter_ranks != optimizer_ranks {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from(
                    "parameter and optimizer shard ranks must stay paired one-to-one",
                ),
            });
        }

        let mut durable_receipts = BTreeMap::new();
        for receipt in &self.shard_upload_receipts {
            if !placement_ids.contains(receipt.shard_id.as_str()) {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "upload receipt `{}` references unknown shard `{}`",
                        receipt.receipt_id, receipt.shard_id
                    ),
                });
            }
            validate_source_admits(
                &sources_by_id,
                receipt.uploader_source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::CheckpointWriter,
            )?;
            if receipt.storage_root.trim().is_empty() || receipt.detail.trim().is_empty() {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "upload receipt `{}` must keep storage_root and detail non-empty",
                        receipt.receipt_id
                    ),
                });
            }
            if receipt.receipt_digest != receipt.stable_digest() {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!("upload receipt `{}` digest drifted", receipt.receipt_id),
                });
            }
            if receipt.upload_disposition == DistributedCheckpointShardUploadDisposition::Durable {
                let prior =
                    durable_receipts.insert(receipt.shard_id.as_str(), receipt.receipt_id.as_str());
                if prior.is_some() {
                    return Err(DistributedCheckpointContractError::InvalidContract {
                        detail: format!(
                            "shard `{}` has more than one durable upload receipt",
                            receipt.shard_id
                        ),
                    });
                }
            }
        }
        for shard_id in &placement_ids {
            if !durable_receipts.contains_key(shard_id.as_str()) {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!("shard `{shard_id}` is missing a durable upload receipt"),
                });
            }
        }

        validate_source_admits(
            &sources_by_id,
            self.restore_plan.manifest_authority_source_id.as_str(),
            manifest,
            CrossProviderExecutionClass::CheckpointWriter,
        )?;
        if self.restore_plan.restore_plan_digest != self.restore_plan.stable_digest() {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("restore_plan digest drifted"),
            });
        }
        let placement_by_role_and_rank = self
            .shard_placements
            .iter()
            .map(|placement| ((placement.shard_role, placement.dense_rank), placement))
            .collect::<BTreeMap<_, _>>();
        let mut covered_parameter_shards = BTreeSet::new();
        let mut covered_optimizer_shards = BTreeSet::new();
        for assignment in &self.restore_plan.assignments {
            if assignment.requested_execution_class
                != CrossProviderExecutionClass::DenseFullModelRank
            {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "restore assignment `{}` must use dense_full_model_rank execution",
                        assignment.assignment_id
                    ),
                });
            }
            validate_source_admits(
                &sources_by_id,
                assignment.restore_source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::DenseFullModelRank,
            )?;
            let parameter = placement_by_role_and_rank
                .get(&(
                    DistributedCheckpointShardRole::ParameterState,
                    assignment.dense_rank,
                ))
                .ok_or_else(|| DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "restore assignment `{}` has no parameter shard for dense rank {}",
                        assignment.assignment_id, assignment.dense_rank
                    ),
                })?;
            let optimizer = placement_by_role_and_rank
                .get(&(
                    DistributedCheckpointShardRole::OptimizerState,
                    assignment.dense_rank,
                ))
                .ok_or_else(|| DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "restore assignment `{}` has no optimizer shard for dense rank {}",
                        assignment.assignment_id, assignment.dense_rank
                    ),
                })?;
            if assignment.parameter_shard_id != parameter.shard_id
                || assignment.optimizer_shard_id != optimizer.shard_id
            {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "restore assignment `{}` drifted from the rank-paired shard mapping",
                        assignment.assignment_id
                    ),
                });
            }
            if assignment.load_order
                != vec![
                    assignment.parameter_shard_id.clone(),
                    assignment.optimizer_shard_id.clone(),
                ]
            {
                return Err(DistributedCheckpointContractError::InvalidContract {
                    detail: format!(
                        "restore assignment `{}` must keep parameter-first load order",
                        assignment.assignment_id
                    ),
                });
            }
            covered_parameter_shards.insert(assignment.parameter_shard_id.clone());
            covered_optimizer_shards.insert(assignment.optimizer_shard_id.clone());
        }
        let expected_parameter_shards = self
            .shard_placements
            .iter()
            .filter(|placement| {
                placement.shard_role == DistributedCheckpointShardRole::ParameterState
            })
            .map(|placement| placement.shard_id.clone())
            .collect::<BTreeSet<_>>();
        let expected_optimizer_shards = self
            .shard_placements
            .iter()
            .filter(|placement| {
                placement.shard_role == DistributedCheckpointShardRole::OptimizerState
            })
            .map(|placement| placement.shard_id.clone())
            .collect::<BTreeSet<_>>();
        if covered_parameter_shards != expected_parameter_shards
            || covered_optimizer_shards != expected_optimizer_shards
        {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from(
                    "restore assignments must cover every parameter and optimizer shard exactly once",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(DistributedCheckpointContractError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical provider-neutral distributed checkpoint contract.
pub fn canonical_sharded_distributed_checkpoint_contract(
) -> Result<ShardedDistributedCheckpointContract, DistributedCheckpointContractError> {
    let manifest = cross_provider_training_program_manifest()?;
    let source_contracts = canonical_cross_provider_compute_source_contracts()?;
    let scope = CheckpointScopeBinding::new(
        CheckpointScopeKind::Run,
        "psion-xprovider-pretrain-step-2048",
    );
    let checkpoint = TrainingCheckpointReference::new(
        manifest.checkpoint_family.clone(),
        "stream-psion-xprovider-pretrain-step-2048",
        "manifest-psion-xprovider-pretrain-step-2048",
        "object-psion-xprovider-pretrain-step-2048",
        "runpod-rank-0",
        12,
        "cluster-state-digest-psion-xprovider-step-2048",
        "xtrain-dense-topology-step-2048",
        CANONICAL_DISTRIBUTED_CHECKPOINT_CREATED_AT_MS - 60_000,
    )
    .with_checkpoint_ref("checkpoint://psion/xprovider/pretrain/2048")
    .with_step(CANONICAL_DISTRIBUTED_CHECKPOINT_STEP)
    .with_durable_at_ms(CANONICAL_DISTRIBUTED_CHECKPOINT_CREATED_AT_MS);

    let shard_placements = (0_u16..8)
        .flat_map(|dense_rank| {
            let parameter_shard_id = format!("parameter-shard-{dense_rank}");
            let optimizer_shard_id = format!("optimizer-shard-{dense_rank}");
            let writer_node_id = format!("runpod-rank-{dense_rank}");
            vec![
                DistributedCheckpointShardPlacement {
                    shard_id: parameter_shard_id.clone(),
                    shard_role: DistributedCheckpointShardRole::ParameterState,
                    dense_rank,
                    producing_source_id: String::from("runpod_8xh100_dense_node"),
                    writer_node_id: writer_node_id.clone(),
                    lineage_slot_id: format!("lineage.checkpoint.parameter.{dense_rank}"),
                    manifest: shard_manifest_ref(
                        parameter_shard_id.as_str(),
                        checkpoint.checkpoint_family.as_str(),
                        checkpoint.checkpoint_ref.as_deref().unwrap_or_default(),
                        checkpoint.step.unwrap_or_default(),
                        dense_rank,
                        DistributedCheckpointShardRole::ParameterState,
                    ),
                },
                DistributedCheckpointShardPlacement {
                    shard_id: optimizer_shard_id.clone(),
                    shard_role: DistributedCheckpointShardRole::OptimizerState,
                    dense_rank,
                    producing_source_id: String::from("runpod_8xh100_dense_node"),
                    writer_node_id,
                    lineage_slot_id: format!("lineage.checkpoint.optimizer.{dense_rank}"),
                    manifest: shard_manifest_ref(
                        optimizer_shard_id.as_str(),
                        checkpoint.checkpoint_family.as_str(),
                        checkpoint.checkpoint_ref.as_deref().unwrap_or_default(),
                        checkpoint.step.unwrap_or_default(),
                        dense_rank,
                        DistributedCheckpointShardRole::OptimizerState,
                    ),
                },
            ]
        })
        .collect::<Vec<_>>();
    let checkpoint_manifest = CheckpointManifest::new(
        scope.clone(),
        manifest.checkpoint_family.clone(),
        checkpoint.clone(),
        shard_placements
            .iter()
            .map(|placement| CheckpointShardManifest {
                shard_id: placement.shard_id.clone(),
                manifest: placement.manifest.clone(),
                writer_node_id: placement.writer_node_id.clone(),
            })
            .collect(),
        CheckpointDurabilityPosture::Durable,
        CANONICAL_DISTRIBUTED_CHECKPOINT_CREATED_AT_MS,
    )?;
    let checkpoint_pointer = CheckpointPointer::new(
        scope,
        manifest.checkpoint_family.clone(),
        checkpoint,
        checkpoint_manifest.manifest_digest.clone(),
        CANONICAL_DISTRIBUTED_CHECKPOINT_UPDATED_AT_MS,
    )?;

    let mut shard_upload_receipts = Vec::new();
    for placement in &shard_placements {
        if placement.shard_role == DistributedCheckpointShardRole::OptimizerState
            && placement.dense_rank == 6
        {
            shard_upload_receipts.push(upload_receipt(
                placement.shard_id.as_str(),
                0,
                "google_l4_validator_node",
                CANONICAL_DISTRIBUTED_CHECKPOINT_STORAGE_ROOT,
                DistributedCheckpointShardUploadDisposition::PartialUploadRefused,
                placement.manifest.total_bytes / 2,
                "partial optimizer upload was refused before pointer promotion",
            ));
        }
        let uploader_source_id = if placement.dense_rank % 2 == 0 {
            "google_l4_validator_node"
        } else {
            "runpod_8xh100_dense_node"
        };
        shard_upload_receipts.push(upload_receipt(
            placement.shard_id.as_str(),
            1,
            uploader_source_id,
            CANONICAL_DISTRIBUTED_CHECKPOINT_STORAGE_ROOT,
            DistributedCheckpointShardUploadDisposition::Durable,
            placement.manifest.total_bytes,
            "durable shard upload is explicit and provider-neutral",
        ));
    }
    shard_upload_receipts.sort_by(|left, right| {
        left.shard_id
            .cmp(&right.shard_id)
            .then_with(|| left.attempt_ordinal.cmp(&right.attempt_ordinal))
    });

    let assignments = (0_u16..8)
        .map(|dense_rank| DistributedCheckpointRestoreAssignment {
            assignment_id: format!("restore-rank-{dense_rank}"),
            dense_rank,
            restore_source_id: String::from("runpod_8xh100_dense_node"),
            requested_execution_class: CrossProviderExecutionClass::DenseFullModelRank,
            parameter_shard_id: format!("parameter-shard-{dense_rank}"),
            optimizer_shard_id: format!("optimizer-shard-{dense_rank}"),
            load_order: vec![
                format!("parameter-shard-{dense_rank}"),
                format!("optimizer-shard-{dense_rank}"),
            ],
            detail: String::from(
                "restore stays parameter-first and loads one optimizer shard immediately after the matching parameter shard",
            ),
        })
        .collect::<Vec<_>>();
    let mut restore_plan = DistributedCheckpointRestorePlan {
        recovery_mode: TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        manifest_authority_source_id: String::from("google_l4_validator_node"),
        assignments,
        restore_plan_digest: String::new(),
    };
    restore_plan.restore_plan_digest = restore_plan.stable_digest();

    let mut contract = ShardedDistributedCheckpointContract {
        schema_version: String::from(SHARDED_DISTRIBUTED_CHECKPOINT_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        checkpoint_manifest,
        checkpoint_pointer,
        shard_placements,
        shard_upload_receipts,
        restore_plan,
        claim_boundary: String::from(
            "This contract closes provider-neutral distributed checkpoint manifests, shard upload receipts, and deterministic restore planning over the current cross-provider training substrate. It does not yet claim same-job mixed-backend dense restore or a generic multi-store replication layer.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate(&manifest, &source_contracts)?;
    Ok(contract)
}

/// Writes the canonical distributed checkpoint contract to the requested path.
pub fn write_sharded_distributed_checkpoint_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), DistributedCheckpointContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DistributedCheckpointContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_sharded_distributed_checkpoint_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| DistributedCheckpointContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn validate_source_admits(
    sources_by_id: &BTreeMap<&str, &CrossProviderComputeSourceContract>,
    source_id: &str,
    manifest: &CrossProviderTrainingProgramManifest,
    requested_execution_class: CrossProviderExecutionClass,
) -> Result<(), DistributedCheckpointContractError> {
    let source = sources_by_id.get(source_id).ok_or_else(|| {
        DistributedCheckpointContractError::InvalidContract {
            detail: format!("missing compute source `{source_id}`"),
        }
    })?;
    source
        .admit_execution_class(manifest, requested_execution_class)
        .map_err(
            |refusal| DistributedCheckpointContractError::InvalidContract {
                detail: format!(
                    "source `{source_id}` refused execution class `{:?}`: {}",
                    requested_execution_class, refusal.detail
                ),
            },
        )?;
    Ok(())
}

fn shard_manifest_ref(
    shard_id: &str,
    checkpoint_family: &str,
    checkpoint_ref: &str,
    step: u64,
    dense_rank: u16,
    shard_role: DistributedCheckpointShardRole,
) -> DatastreamManifestRef {
    let payload = format!("psionic|{checkpoint_ref}|{shard_id}|rank:{dense_rank}|{shard_role:?}");
    DatastreamManifest::from_bytes(
        format!("stream-{shard_id}"),
        DatastreamSubjectKind::Checkpoint,
        payload.as_bytes(),
        16,
        DatastreamEncoding::Safetensors,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new(checkpoint_family)
            .with_checkpoint_ref(checkpoint_ref)
            .with_step(step),
    )
    .manifest_ref()
}

fn upload_receipt(
    shard_id: &str,
    attempt_ordinal: u16,
    uploader_source_id: &str,
    storage_root: &str,
    upload_disposition: DistributedCheckpointShardUploadDisposition,
    bytes_committed: u64,
    detail: &str,
) -> DistributedCheckpointShardUploadReceipt {
    let mut receipt = DistributedCheckpointShardUploadReceipt {
        receipt_id: format!("upload-{shard_id}-{attempt_ordinal}"),
        shard_id: shard_id.to_string(),
        attempt_ordinal,
        uploader_source_id: uploader_source_id.to_string(),
        storage_root: storage_root.to_string(),
        upload_disposition,
        bytes_committed,
        detail: detail.to_string(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("stable digest serialization should succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_sharded_distributed_checkpoint_contract, DistributedCheckpointContractError,
        DistributedCheckpointShardRole, DistributedCheckpointShardUploadDisposition,
    };

    #[test]
    fn canonical_contract_pairs_parameter_and_optimizer_shards_per_rank(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_sharded_distributed_checkpoint_contract()?;
        let parameter_count = contract
            .shard_placements
            .iter()
            .filter(|placement| {
                placement.shard_role == DistributedCheckpointShardRole::ParameterState
            })
            .count();
        let optimizer_count = contract
            .shard_placements
            .iter()
            .filter(|placement| {
                placement.shard_role == DistributedCheckpointShardRole::OptimizerState
            })
            .count();
        assert_eq!(parameter_count, 8);
        assert_eq!(optimizer_count, 8);
        assert_eq!(contract.restore_plan.assignments.len(), 8);
        Ok(())
    }

    #[test]
    fn canonical_contract_rejects_missing_durable_optimizer_receipt(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut contract = canonical_sharded_distributed_checkpoint_contract()?;
        contract.shard_upload_receipts.retain(|receipt| {
            !(receipt.shard_id == "optimizer-shard-0"
                && receipt.upload_disposition
                    == DistributedCheckpointShardUploadDisposition::Durable)
        });
        let manifest = crate::cross_provider_training_program_manifest()?;
        let sources = crate::canonical_cross_provider_compute_source_contracts()?;
        let err = contract
            .validate(&manifest, &sources)
            .expect_err("missing durable optimizer receipt must be rejected");
        assert!(matches!(
            err,
            DistributedCheckpointContractError::InvalidContract { .. }
        ));
        Ok(())
    }

    #[test]
    fn canonical_contract_digest_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_sharded_distributed_checkpoint_contract()?;
        assert_eq!(contract.contract_digest, contract.stable_digest());
        Ok(())
    }
}
