use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_hybrid_pretraining_plan, cross_provider_training_program_manifest, PolicyRevision,
    TrainingCheckpointReference,
};

pub const CONTRIBUTOR_PROGRAM_LINEAGE_SCHEMA_VERSION: &str =
    "psionic.contributor_program_lineage.v1";
pub const CONTRIBUTOR_PROGRAM_LINEAGE_FIXTURE_PATH: &str =
    "fixtures/training/contributor_program_lineage_v1.json";
pub const CONTRIBUTOR_PROGRAM_LINEAGE_CHECK_SCRIPT_PATH: &str =
    "scripts/check-contributor-program-lineage.sh";
pub const CONTRIBUTOR_PROGRAM_LINEAGE_DOC_PATH: &str =
    "docs/CONTRIBUTOR_PROGRAM_LINEAGE_REFERENCE.md";

const FIRST_SWARM_RUN_CONTRACT_PATH: &str = "fixtures/swarm/first_swarm_run_contract_v1.json";
const FIRST_SWARM_RECEIPT_CONTRACT_PATH: &str =
    "fixtures/swarm/first_swarm_open_adapter_receipt_contract_v1.json";
const GOOGLE_TRAINING_BINDER_PROJECTION_PATH: &str =
    "fixtures/training/google_training_binder_projection_v1.json";

#[derive(Debug, Error)]
pub enum ContributorProgramLineageError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("contributor program-lineage contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributorPromotionPosture {
    CandidateProgramRevision,
    HoldOrQuarantineOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseProgramLineageAnchor {
    pub lineage_slot_id: String,
    pub source_id: String,
    pub checkpoint_family: String,
    pub policy_revision_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributorWindowProgramBinding {
    pub window_id: String,
    pub source_id: String,
    pub lineage_slot_id: String,
    pub dataset_family_id: String,
    pub dataset_slice_id: String,
    pub dataset_slice_digest: String,
    pub checkpoint_family: String,
    pub input_policy_revision_id: String,
    pub promotion_contract_id: String,
    pub authority_paths: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributorPromotionContract {
    pub promotion_contract_id: String,
    pub window_id: String,
    pub input_policy_revision: PolicyRevision,
    pub candidate_policy_revision: PolicyRevision,
    pub checkpoint_family: String,
    pub candidate_checkpoint: TrainingCheckpointReference,
    pub promotion_posture: ContributorPromotionPosture,
    pub admitted_validator_dispositions: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributorProgramLineageContract {
    pub schema_version: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub dataset_family_id: String,
    pub checkpoint_family: String,
    pub input_policy_revision: PolicyRevision,
    pub dense_lineage_anchors: Vec<DenseProgramLineageAnchor>,
    pub contributor_window_bindings: Vec<ContributorWindowProgramBinding>,
    pub promotion_contracts: Vec<ContributorPromotionContract>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl ContributorProgramLineageContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_contributor_program_lineage_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), ContributorProgramLineageError> {
        let manifest = cross_provider_training_program_manifest().map_err(|error| {
            ContributorProgramLineageError::InvalidContract {
                detail: format!("failed to load root training-program manifest: {error}"),
            }
        })?;
        let hybrid_plan = canonical_hybrid_pretraining_plan().map_err(|error| {
            ContributorProgramLineageError::InvalidContract {
                detail: format!("failed to load hybrid pretraining plan: {error}"),
            }
        })?;
        if self.schema_version != CONTRIBUTOR_PROGRAM_LINEAGE_SCHEMA_VERSION {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CONTRIBUTOR_PROGRAM_LINEAGE_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: String::from("program-manifest binding drifted"),
            });
        }
        if self.dataset_family_id != manifest.dataset_family_id
            || self.checkpoint_family != manifest.checkpoint_family
        {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: String::from("dataset family or checkpoint family drifted from manifest"),
            });
        }
        if self.input_policy_revision.policy_family != self.checkpoint_family {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: String::from(
                    "input policy revision must stay aligned with checkpoint family",
                ),
            });
        }
        if self.contributor_window_bindings.len()
            != hybrid_plan.contributor_window_assignments.len()
        {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: String::from("contributor-window binding count drifted from hybrid plan"),
            });
        }
        for binding in &self.contributor_window_bindings {
            let assignment = hybrid_plan
                .contributor_window_assignments
                .iter()
                .find(|assignment| assignment.window_id == binding.window_id)
                .ok_or_else(|| ContributorProgramLineageError::InvalidContract {
                    detail: format!(
                        "contributor window `{}` was not found in the hybrid plan",
                        binding.window_id
                    ),
                })?;
            if binding.source_id != assignment.source_id
                || binding.lineage_slot_id != assignment.lineage_slot_id
                || binding.dataset_slice_id != assignment.dataset_slice.slice_id
                || binding.dataset_slice_digest != assignment.dataset_slice.slice_digest
                || binding.dataset_family_id != assignment.dataset_slice.dataset_id
            {
                return Err(ContributorProgramLineageError::InvalidContract {
                    detail: format!(
                        "binding `{}` drifted from the hybrid contributor window assignment",
                        binding.window_id
                    ),
                });
            }
            if binding.checkpoint_family != self.checkpoint_family
                || binding.input_policy_revision_id != self.input_policy_revision.revision_id
            {
                return Err(ContributorProgramLineageError::InvalidContract {
                    detail: format!(
                        "binding `{}` lost the canonical checkpoint or policy revision identity",
                        binding.window_id
                    ),
                });
            }
        }
        if self.promotion_contracts.len() != self.contributor_window_bindings.len() {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: String::from(
                    "promotion-contract count drifted from contributor-window count",
                ),
            });
        }
        for contract in &self.promotion_contracts {
            if contract.input_policy_revision != self.input_policy_revision {
                return Err(ContributorProgramLineageError::InvalidContract {
                    detail: format!(
                        "promotion contract `{}` lost the shared input policy revision",
                        contract.promotion_contract_id
                    ),
                });
            }
            if contract.candidate_policy_revision.policy_family != self.checkpoint_family
                || contract.candidate_checkpoint.checkpoint_family != self.checkpoint_family
            {
                return Err(ContributorProgramLineageError::InvalidContract {
                    detail: format!(
                        "promotion contract `{}` drifted off the canonical checkpoint family",
                        contract.promotion_contract_id
                    ),
                });
            }
        }
        if self.contract_digest != self.stable_digest() {
            return Err(ContributorProgramLineageError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

static CONTRIBUTOR_PROGRAM_LINEAGE_CONTRACT_CACHE: std::sync::OnceLock<
    ContributorProgramLineageContract,
> = std::sync::OnceLock::new();

pub fn canonical_contributor_program_lineage_contract(
) -> Result<ContributorProgramLineageContract, ContributorProgramLineageError> {
    if let Some(contract) = CONTRIBUTOR_PROGRAM_LINEAGE_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        ContributorProgramLineageError::InvalidContract {
            detail: format!("failed to load root training-program manifest: {error}"),
        }
    })?;
    let hybrid_plan = canonical_hybrid_pretraining_plan().map_err(|error| {
        ContributorProgramLineageError::InvalidContract {
            detail: format!("failed to load hybrid pretraining plan: {error}"),
        }
    })?;
    let input_policy_revision = PolicyRevision::new(
        manifest.checkpoint_family.clone(),
        "pretrain-policy-r7",
        "pretrain-policy-r7-digest",
        1_710_000_700_000,
    )
    .with_revision_number(7)
    .with_parent_revision_id("pretrain-policy-r6");
    let dense_lineage_anchors = hybrid_plan
        .dense_rank_assignments
        .iter()
        .take(2)
        .map(|assignment| DenseProgramLineageAnchor {
            lineage_slot_id: assignment.lineage_slot_id.clone(),
            source_id: assignment.source_id.clone(),
            checkpoint_family: manifest.checkpoint_family.clone(),
            policy_revision_id: input_policy_revision.revision_id.clone(),
            detail: String::from(
                "Contributor windows inherit the same checkpoint family and current shared policy revision as the dense anchors they later validate against.",
            ),
        })
        .collect::<Vec<_>>();
    let contributor_window_bindings = hybrid_plan
        .contributor_window_assignments
        .iter()
        .map(|assignment| {
            contributor_binding_for_assignment(
                assignment,
                &manifest.checkpoint_family,
                &input_policy_revision.revision_id,
            )
        })
        .collect::<Vec<_>>();
    let promotion_contracts = contributor_window_bindings
        .iter()
        .map(|binding| {
            promotion_contract_for_binding(
                binding,
                &input_policy_revision,
                &manifest.checkpoint_family,
            )
        })
        .collect::<Vec<_>>();

    let mut contract = ContributorProgramLineageContract {
        schema_version: String::from(CONTRIBUTOR_PROGRAM_LINEAGE_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        dataset_family_id: manifest.dataset_family_id.clone(),
        checkpoint_family: manifest.checkpoint_family.clone(),
        input_policy_revision,
        dense_lineage_anchors,
        contributor_window_bindings,
        promotion_contracts,
        claim_boundary: String::from(
            "This contract binds the current validated contributor windows to the same canonical pretraining dataset family, checkpoint family, and shared policy revision used by the hybrid dense program. It does not claim that every existing bounded local swarm lane has already been migrated onto this dataset family or that contributor windows replace dense-rank training.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = CONTRIBUTOR_PROGRAM_LINEAGE_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_contributor_program_lineage_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), ContributorProgramLineageError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| ContributorProgramLineageError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_contributor_program_lineage_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| ContributorProgramLineageError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn contributor_binding_for_assignment(
    assignment: &crate::HybridPretrainingContributorWindowAssignment,
    checkpoint_family: &str,
    input_policy_revision_id: &str,
) -> ContributorWindowProgramBinding {
    let authority_paths = match assignment.source_id.as_str() {
        "local_mlx_mac_workstation" | "local_rtx4080_workstation" => vec![
            String::from(FIRST_SWARM_RUN_CONTRACT_PATH),
            String::from(FIRST_SWARM_RECEIPT_CONTRACT_PATH),
        ],
        "google_l4_validator_node" => vec![String::from(GOOGLE_TRAINING_BINDER_PROJECTION_PATH)],
        _ => Vec::new(),
    };
    ContributorWindowProgramBinding {
        window_id: assignment.window_id.clone(),
        source_id: assignment.source_id.clone(),
        lineage_slot_id: assignment.lineage_slot_id.clone(),
        dataset_family_id: assignment.dataset_slice.dataset_id.clone(),
        dataset_slice_id: assignment.dataset_slice.slice_id.clone(),
        dataset_slice_digest: assignment.dataset_slice.slice_digest.clone(),
        checkpoint_family: checkpoint_family.to_owned(),
        input_policy_revision_id: input_policy_revision_id.to_owned(),
        promotion_contract_id: format!("promotion.{}", assignment.window_id),
        authority_paths,
        detail: String::from(
            "This contributor window now binds back to the canonical pretraining checkpoint family and shared policy revision instead of floating as a standalone bounded window.",
        ),
    }
}

fn promotion_contract_for_binding(
    binding: &ContributorWindowProgramBinding,
    input_policy_revision: &PolicyRevision,
    checkpoint_family: &str,
) -> ContributorPromotionContract {
    let candidate_policy_revision = PolicyRevision::new(
        checkpoint_family.to_owned(),
        format!("{}-candidate-r8", binding.window_id),
        format!("{}-candidate-r8-digest", binding.window_id),
        1_710_000_800_000,
    )
    .with_revision_number(8)
    .with_parent_revision_id(input_policy_revision.revision_id.clone())
    .with_checkpoint(TrainingCheckpointReference::new(
        checkpoint_family.to_owned(),
        format!("checkpoint.{}", binding.window_id),
        format!("manifest-digest.{}", binding.window_id),
        format!("object-digest.{}", binding.window_id),
        format!("writer.{}", binding.source_id),
        8,
        format!("cluster-state.{}", binding.window_id),
        format!("topology.{}", binding.window_id),
        1_710_000_800_000,
    ));
    let candidate_checkpoint = candidate_policy_revision
        .checkpoint
        .clone()
        .expect("candidate policy revision must keep a checkpoint anchor");
    ContributorPromotionContract {
        promotion_contract_id: binding.promotion_contract_id.clone(),
        window_id: binding.window_id.clone(),
        input_policy_revision: input_policy_revision.clone(),
        candidate_policy_revision,
        checkpoint_family: checkpoint_family.to_owned(),
        candidate_checkpoint,
        promotion_posture: ContributorPromotionPosture::CandidateProgramRevision,
        admitted_validator_dispositions: vec![
            String::from("accepted"),
            String::from("quarantined"),
            String::from("rejected"),
            String::from("replay_required"),
        ],
        detail: String::from(
            "Accepted contributor windows may mint a candidate shared pretraining policy revision under the canonical checkpoint family, while quarantine, reject, and replay-required outcomes remain explicit no-promotion paths.",
        ),
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("contributor program-lineage values must serialize"),
    );
    format!("{:x}", hasher.finalize())
}
