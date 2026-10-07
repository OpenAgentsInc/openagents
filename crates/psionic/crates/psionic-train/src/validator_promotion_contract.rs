use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_contributor_program_lineage_contract, canonical_training_execution_evidence_bundle,
    cross_provider_training_program_manifest, CrossProviderExecutionClass,
    TrainingExecutionPromotionOutcome, TrainingExecutionValidatorDisposition,
};

pub const SHARED_VALIDATOR_PROMOTION_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.shared_validator_promotion_contract.v1";
pub const SHARED_VALIDATOR_PROMOTION_CONTRACT_ID: &str =
    "psionic.shared_validator_promotion_contract.v1";
pub const SHARED_VALIDATOR_PROMOTION_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/shared_validator_promotion_contract_v1.json";
pub const SHARED_VALIDATOR_PROMOTION_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-shared-validator-promotion-contract.sh";
pub const SHARED_VALIDATOR_PROMOTION_CONTRACT_DOC_PATH: &str =
    "docs/SHARED_VALIDATOR_PROMOTION_CONTRACT_REFERENCE.md";

#[derive(Debug, Error)]
pub enum SharedValidatorPromotionContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("shared validator and promotion contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedValidatorExecutionClassPolicy {
    pub execution_class: CrossProviderExecutionClass,
    pub admitted_dispositions: Vec<TrainingExecutionValidatorDisposition>,
    pub quarantine_blocks_promotion: bool,
    pub replay_required_blocks_promotion: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedValidatorPromotionContract {
    pub schema_version: String,
    pub contract_id: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub contributor_program_lineage_digest: String,
    pub evidence_bundle_schema_version: String,
    pub admitted_validator_dispositions: Vec<TrainingExecutionValidatorDisposition>,
    pub admitted_promotion_outcomes: Vec<TrainingExecutionPromotionOutcome>,
    pub execution_class_policies: Vec<SharedValidatorExecutionClassPolicy>,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl SharedValidatorPromotionContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_shared_validator_promotion_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), SharedValidatorPromotionContractError> {
        let manifest = cross_provider_training_program_manifest().map_err(|error| {
            SharedValidatorPromotionContractError::InvalidContract {
                detail: format!("failed to load root training-program manifest: {error}"),
            }
        })?;
        let contributor_lineage =
            canonical_contributor_program_lineage_contract().map_err(|error| {
                SharedValidatorPromotionContractError::InvalidContract {
                    detail: format!("failed to load contributor program lineage contract: {error}"),
                }
            })?;
        let evidence_bundle = canonical_training_execution_evidence_bundle().map_err(|error| {
            SharedValidatorPromotionContractError::InvalidContract {
                detail: format!("failed to load training execution evidence bundle: {error}"),
            }
        })?;
        if self.schema_version != SHARED_VALIDATOR_PROMOTION_CONTRACT_SCHEMA_VERSION {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    SHARED_VALIDATOR_PROMOTION_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != SHARED_VALIDATOR_PROMOTION_CONTRACT_ID {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: String::from("program-manifest binding drifted"),
            });
        }
        if self.contributor_program_lineage_digest != contributor_lineage.contract_digest {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: String::from("contributor program-lineage digest drifted"),
            });
        }
        if evidence_bundle.validator_promotion_contract_id != self.contract_id {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: String::from("evidence bundle lost the shared validator contract binding"),
            });
        }
        for segment in &evidence_bundle.segment_evidence {
            for result in &segment.validator_results {
                if !self
                    .admitted_validator_dispositions
                    .contains(&result.disposition)
                {
                    return Err(SharedValidatorPromotionContractError::InvalidContract {
                        detail: format!(
                            "validator result on segment `{}` used disposition `{:?}` outside the shared contract",
                            segment.segment_id, result.disposition
                        ),
                    });
                }
                let policy = self
                    .execution_class_policies
                    .iter()
                    .find(|policy| policy.execution_class == result.execution_class)
                    .ok_or_else(|| SharedValidatorPromotionContractError::InvalidContract {
                        detail: format!(
                            "missing execution-class validator policy for `{:?}`",
                            result.execution_class
                        ),
                    })?;
                if !policy.admitted_dispositions.contains(&result.disposition) {
                    return Err(SharedValidatorPromotionContractError::InvalidContract {
                        detail: format!(
                            "validator result on segment `{}` used disposition `{:?}` outside the execution-class policy",
                            segment.segment_id, result.disposition
                        ),
                    });
                }
            }
        }
        if !self
            .admitted_promotion_outcomes
            .contains(&evidence_bundle.final_disposition.promotion_outcome)
        {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: String::from(
                    "evidence bundle final promotion outcome drifted from shared contract",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(SharedValidatorPromotionContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

static SHARED_VALIDATOR_PROMOTION_CONTRACT_CACHE: std::sync::OnceLock<
    SharedValidatorPromotionContract,
> = std::sync::OnceLock::new();

pub fn canonical_shared_validator_promotion_contract(
) -> Result<SharedValidatorPromotionContract, SharedValidatorPromotionContractError> {
    if let Some(contract) = SHARED_VALIDATOR_PROMOTION_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        SharedValidatorPromotionContractError::InvalidContract {
            detail: format!("failed to load root training-program manifest: {error}"),
        }
    })?;
    let contributor_lineage =
        canonical_contributor_program_lineage_contract().map_err(|error| {
            SharedValidatorPromotionContractError::InvalidContract {
                detail: format!("failed to load contributor program lineage contract: {error}"),
            }
        })?;
    let admitted_validator_dispositions = vec![
        TrainingExecutionValidatorDisposition::Accepted,
        TrainingExecutionValidatorDisposition::Quarantined,
        TrainingExecutionValidatorDisposition::Rejected,
        TrainingExecutionValidatorDisposition::ReplayRequired,
    ];
    let admitted_promotion_outcomes = vec![
        TrainingExecutionPromotionOutcome::PromotedRevision,
        TrainingExecutionPromotionOutcome::HeldNoPromotion,
        TrainingExecutionPromotionOutcome::RefusedPromotion,
    ];
    let mut contract = SharedValidatorPromotionContract {
        schema_version: String::from(SHARED_VALIDATOR_PROMOTION_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(SHARED_VALIDATOR_PROMOTION_CONTRACT_ID),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        contributor_program_lineage_digest: contributor_lineage.contract_digest.clone(),
        evidence_bundle_schema_version: String::from("psionic.training_execution_evidence_bundle.v1"),
        admitted_validator_dispositions: admitted_validator_dispositions.clone(),
        admitted_promotion_outcomes,
        execution_class_policies: vec![
            policy(
                CrossProviderExecutionClass::DenseFullModelRank,
                admitted_validator_dispositions.clone(),
                true,
                true,
                "Dense-rank execution may be accepted, quarantined, rejected, or replay-required. Quarantine or replay-required blocks promotion until the missing proof closes.",
            ),
            policy(
                CrossProviderExecutionClass::ValidatedContributorWindow,
                admitted_validator_dispositions.clone(),
                true,
                true,
                "Contributor windows use the same acceptance vocabulary as dense work and keep quarantine and replay-required as explicit no-promotion states.",
            ),
            policy(
                CrossProviderExecutionClass::Validator,
                admitted_validator_dispositions.clone(),
                true,
                true,
                "Validator-only work shares the same vocabulary so replay and quarantine never become provider-local language.",
            ),
            policy(
                CrossProviderExecutionClass::CheckpointWriter,
                admitted_validator_dispositions.clone(),
                true,
                true,
                "Checkpoint-writer closure uses the same shared validator and promotion vocabulary.",
            ),
            policy(
                CrossProviderExecutionClass::EvalWorker,
                admitted_validator_dispositions,
                true,
                true,
                "Eval-worker proof uses the same shared validator and promotion vocabulary.",
            ),
        ],
        claim_boundary: String::from(
            "This contract closes one shared validator, replay, quarantine, rejection, acceptance, and promotion vocabulary across the current execution classes and the provider-neutral evidence bundle. It does not claim app-review workflow closure or weaken refusal posture for replay, provenance, or validator gaps.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = SHARED_VALIDATOR_PROMOTION_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_shared_validator_promotion_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), SharedValidatorPromotionContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            SharedValidatorPromotionContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_shared_validator_promotion_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| SharedValidatorPromotionContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn policy(
    execution_class: CrossProviderExecutionClass,
    admitted_dispositions: Vec<TrainingExecutionValidatorDisposition>,
    quarantine_blocks_promotion: bool,
    replay_required_blocks_promotion: bool,
    detail: impl Into<String>,
) -> SharedValidatorExecutionClassPolicy {
    SharedValidatorExecutionClassPolicy {
        execution_class,
        admitted_dispositions,
        quarantine_blocks_promotion,
        replay_required_blocks_promotion,
        detail: detail.into(),
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("shared validator and promotion contract values must serialize"),
    );
    format!("{:x}", hasher.finalize())
}
