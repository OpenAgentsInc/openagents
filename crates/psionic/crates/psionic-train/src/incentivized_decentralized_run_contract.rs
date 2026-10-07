use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_open_public_decentralized_run_contract, canonical_reward_ledger_contract,
    canonical_settlement_publication_contract, OpenPublicDecentralizedRunContractError,
    RewardLedgerContractError, SettlementPublicationContractError,
};

pub const INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.incentivized_decentralized_run_contract.v1";
pub const INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_ID: &str =
    "psionic.incentivized_decentralized_run_contract.v1";
pub const INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/incentivized_decentralized_run_contract_v1.json";
pub const INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-incentivized-decentralized-run-contract.sh";
pub const INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_DOC_PATH: &str =
    "docs/INCENTIVIZED_DECENTRALIZED_RUN_REFERENCE.md";
pub const INCENTIVIZED_DECENTRALIZED_RUN_AUDIT_DOC_PATH: &str =
    "docs/audits/2026-03-26-incentivized-decentralized-run-audit.md";
pub const INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str =
    "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum IncentivizedDecentralizedRunContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    RewardLedger(#[from] RewardLedgerContractError),
    #[error(transparent)]
    Settlement(#[from] SettlementPublicationContractError),
    #[error(transparent)]
    OpenRun(#[from] OpenPublicDecentralizedRunContractError),
    #[error("incentivized decentralized run contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncentivizedRunParticipant {
    pub participant_id: String,
    pub node_identity_id: String,
    pub allocation_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncentivizedRunPayoutPublication {
    pub publication_id: String,
    pub settlement_record_id: String,
    pub payout_export_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncentivizedRunOutcome {
    pub outcome_id: String,
    pub published_weight_publication_ids: Vec<String>,
    pub audit_doc_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncentivizedDecentralizedRunAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub audit_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncentivizedDecentralizedRunContract {
    pub schema_version: String,
    pub contract_id: String,
    pub reward_ledger_contract_digest: String,
    pub settlement_publication_contract_digest: String,
    pub open_public_decentralized_run_contract_digest: String,
    pub participants: Vec<IncentivizedRunParticipant>,
    pub payout_publication: IncentivizedRunPayoutPublication,
    pub outcome: IncentivizedRunOutcome,
    pub authority_paths: IncentivizedDecentralizedRunAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl IncentivizedDecentralizedRunContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_incentivized_decentralized_run_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), IncentivizedDecentralizedRunContractError> {
        let ledger = canonical_reward_ledger_contract()?;
        let settlement = canonical_settlement_publication_contract()?;
        let open_run = canonical_open_public_decentralized_run_contract()?;

        if self.schema_version != INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION {
            return Err(IncentivizedDecentralizedRunContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_ID {
            return Err(IncentivizedDecentralizedRunContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.reward_ledger_contract_digest != ledger.contract_digest
            || self.settlement_publication_contract_digest != settlement.contract_digest
            || self.open_public_decentralized_run_contract_digest != open_run.contract_digest
        {
            return Err(IncentivizedDecentralizedRunContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.participants.len() != 3
            || self.payout_publication.payout_export_ids.len() != 3
            || self.outcome.published_weight_publication_ids.len() != 2
            || self.outcome.audit_doc_path != INCENTIVIZED_DECENTRALIZED_RUN_AUDIT_DOC_PATH
            || self.authority_paths.audit_doc_path != INCENTIVIZED_DECENTRALIZED_RUN_AUDIT_DOC_PATH
            || self.payout_publication.settlement_record_id
                != settlement.settlement_records[0].record_id
        {
            return Err(IncentivizedDecentralizedRunContractError::InvalidContract {
                detail: String::from("incentivized-run bindings drifted"),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(IncentivizedDecentralizedRunContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

pub fn canonical_incentivized_decentralized_run_contract(
) -> Result<IncentivizedDecentralizedRunContract, IncentivizedDecentralizedRunContractError> {
    let ledger = canonical_reward_ledger_contract()?;
    let settlement = canonical_settlement_publication_contract()?;
    let open_run = canonical_open_public_decentralized_run_contract()?;

    let participants = vec![
        IncentivizedRunParticipant {
            participant_id: String::from("participant.local_mlx.paid"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            allocation_id: String::from("allocation.local_mlx.window1231"),
            detail: String::from("Apple MLX remains the largest paid participant."),
        },
        IncentivizedRunParticipant {
            participant_id: String::from("participant.google.paid"),
            node_identity_id: String::from("google_l4_validator_node.identity"),
            allocation_id: String::from("allocation.google.window1231"),
            detail: String::from("Google remains the second paid participant."),
        },
        IncentivizedRunParticipant {
            participant_id: String::from("participant.runpod.paid"),
            node_identity_id: String::from("runpod_8xh100_dense_node.identity"),
            allocation_id: String::from("allocation.runpod.window1231"),
            detail: String::from("RunPod remains the third paid participant."),
        },
    ];

    let mut contract = IncentivizedDecentralizedRunContract {
        schema_version: String::from(INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_ID),
        reward_ledger_contract_digest: ledger.contract_digest.clone(),
        settlement_publication_contract_digest: settlement.contract_digest.clone(),
        open_public_decentralized_run_contract_digest: open_run.contract_digest.clone(),
        participants,
        payout_publication: IncentivizedRunPayoutPublication {
            publication_id: String::from("publication.incentivized_run.window1231"),
            settlement_record_id: settlement.settlement_records[0].record_id.clone(),
            payout_export_ids: vec![
                String::from("payout.local_mlx.window1231"),
                String::from("payout.google.window1231"),
                String::from("payout.runpod.window1231"),
            ],
            detail: String::from("The first incentivized run publishes the signed-ledger payout bundle."),
        },
        outcome: IncentivizedRunOutcome {
            outcome_id: String::from("outcome.incentivized_run.window1231"),
            published_weight_publication_ids: vec![
                String::from("validator_weight.google.window1231"),
                String::from("validator_weight.local_mlx.window1231"),
            ],
            audit_doc_path: String::from(INCENTIVIZED_DECENTRALIZED_RUN_AUDIT_DOC_PATH),
            detail: String::from("The first incentivized run retains published weights and payout-ready exports."),
        },
        authority_paths: IncentivizedDecentralizedRunAuthorityPaths {
            fixture_path: String::from(INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_DOC_PATH),
            audit_doc_path: String::from(INCENTIVIZED_DECENTRALIZED_RUN_AUDIT_DOC_PATH),
            train_system_doc_path: String::from(INCENTIVIZED_DECENTRALIZED_RUN_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first incentivized decentralized run surface: retained paid participants, one payout publication bundle, and one incentive-focused audit. It does not claim canary payouts.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_incentivized_decentralized_run_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), IncentivizedDecentralizedRunContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            IncentivizedDecentralizedRunContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_incentivized_decentralized_run_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| {
        IncentivizedDecentralizedRunContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for incentivized run contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_incentivized_decentralized_run_contract,
        IncentivizedDecentralizedRunContractError,
    };

    #[test]
    fn canonical_incentivized_decentralized_run_contract_is_valid(
    ) -> Result<(), IncentivizedDecentralizedRunContractError> {
        let contract = canonical_incentivized_decentralized_run_contract()?;
        contract.validate()
    }
}
