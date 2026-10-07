use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_decentralized_network_contract, canonical_fraud_quarantine_slashing_contract,
    canonical_multi_validator_consensus_contract, canonical_signed_node_identity_contract_set,
    canonical_validator_challenge_scoring_contract, DecentralizedNetworkContractError,
    DecentralizedNetworkRoleClass, FraudQuarantineSlashingContractError,
    MultiValidatorConsensusContractError, SignedNodeIdentityContractSetError,
    TrainingExecutionValidatorDisposition, ValidatorChallengeScoringContractError,
};

pub const REWARD_LEDGER_CONTRACT_SCHEMA_VERSION: &str = "psionic.reward_ledger_contract.v1";
pub const REWARD_LEDGER_CONTRACT_ID: &str = "psionic.reward_ledger_contract.v1";
pub const REWARD_LEDGER_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/reward_ledger_contract_v1.json";
pub const REWARD_LEDGER_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-reward-ledger-contract.sh";
pub const REWARD_LEDGER_CONTRACT_DOC_PATH: &str = "docs/REWARD_LEDGER_REFERENCE.md";
pub const REWARD_LEDGER_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum RewardLedgerContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    SignedNodeIdentity(#[from] SignedNodeIdentityContractSetError),
    #[error(transparent)]
    ValidatorScoring(#[from] ValidatorChallengeScoringContractError),
    #[error(transparent)]
    MultiValidatorConsensus(#[from] MultiValidatorConsensusContractError),
    #[error(transparent)]
    FraudPolicy(#[from] FraudQuarantineSlashingContractError),
    #[error("reward ledger contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLedgerAccountingPeriod {
    pub accounting_period_id: String,
    pub opens_epoch_id: String,
    pub closes_epoch_id: String,
    pub reward_budget_microunits: u64,
    pub penalty_pool_microunits: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLedgerContributionEntry {
    pub entry_id: String,
    pub node_identity_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub evidence_reference_id: String,
    pub gross_reward_microunits: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLedgerPenaltyEntry {
    pub entry_id: String,
    pub node_identity_id: String,
    pub slashing_decision_id: String,
    pub penalty_microunits: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLedgerFinalAllocation {
    pub allocation_id: String,
    pub node_identity_id: String,
    pub net_reward_microunits: i64,
    pub payout_weight_bps: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLedgerAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLedgerContract {
    pub schema_version: String,
    pub contract_id: String,
    pub decentralized_network_contract_digest: String,
    pub signed_node_identity_contract_set_digest: String,
    pub validator_challenge_scoring_contract_digest: String,
    pub multi_validator_consensus_contract_digest: String,
    pub fraud_quarantine_slashing_contract_digest: String,
    pub accounting_period: RewardLedgerAccountingPeriod,
    pub contribution_entries: Vec<RewardLedgerContributionEntry>,
    pub penalty_entries: Vec<RewardLedgerPenaltyEntry>,
    pub final_allocations: Vec<RewardLedgerFinalAllocation>,
    pub authority_paths: RewardLedgerAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl RewardLedgerContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_reward_ledger_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), RewardLedgerContractError> {
        let network = canonical_decentralized_network_contract()?;
        let identities = canonical_signed_node_identity_contract_set()?;
        let scoring = canonical_validator_challenge_scoring_contract()?;
        let consensus = canonical_multi_validator_consensus_contract()?;
        let fraud = canonical_fraud_quarantine_slashing_contract()?;

        let identity_by_id = identities
            .identities
            .iter()
            .map(|identity| (identity.node_identity_id.as_str(), identity))
            .collect::<BTreeMap<_, _>>();
        let score_receipt_by_id = scoring
            .score_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let decision_ids = consensus
            .promotion_decisions
            .iter()
            .map(|decision| decision.decision_id.as_str())
            .collect::<BTreeSet<_>>();
        let slashing_by_id = fraud
            .slashing_decisions
            .iter()
            .map(|decision| (decision.decision_id.as_str(), decision))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != REWARD_LEDGER_CONTRACT_SCHEMA_VERSION {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    REWARD_LEDGER_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != REWARD_LEDGER_CONTRACT_ID {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest
            || self.signed_node_identity_contract_set_digest != identities.contract_digest
            || self.validator_challenge_scoring_contract_digest != scoring.contract_digest
            || self.multi_validator_consensus_contract_digest != consensus.contract_digest
            || self.fraud_quarantine_slashing_contract_digest != fraud.contract_digest
        {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != REWARD_LEDGER_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path != REWARD_LEDGER_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != REWARD_LEDGER_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != REWARD_LEDGER_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.accounting_period.opens_epoch_id >= self.accounting_period.closes_epoch_id
            || self.accounting_period.reward_budget_microunits == 0
        {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("accounting period drifted"),
            });
        }
        if self.contribution_entries.len() != 5
            || self.penalty_entries.len() != 1
            || self.final_allocations.len() != 4
        {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from(
                    "expected canonical contribution, penalty, and allocation counts",
                ),
            });
        }

        let mut gross_total = 0_u64;
        for entry in &self.contribution_entries {
            let identity = identity_by_id
                .get(entry.node_identity_id.as_str())
                .ok_or_else(|| RewardLedgerContractError::InvalidContract {
                    detail: format!(
                        "contribution entry `{}` references unknown node identity `{}`",
                        entry.entry_id, entry.node_identity_id
                    ),
                })?;
            if entry.gross_reward_microunits == 0
                || !identity.admitted_role_classes.contains(&entry.role_class)
            {
                return Err(RewardLedgerContractError::InvalidContract {
                    detail: format!("contribution entry `{}` drifted", entry.entry_id),
                });
            }
            match entry.role_class {
                DecentralizedNetworkRoleClass::PublicMiner
                | DecentralizedNetworkRoleClass::PublicValidator => {
                    let receipt = score_receipt_by_id
                        .get(entry.evidence_reference_id.as_str())
                        .ok_or_else(|| RewardLedgerContractError::InvalidContract {
                            detail: format!(
                                "contribution entry `{}` references unknown score receipt `{}`",
                                entry.entry_id, entry.evidence_reference_id
                            ),
                        })?;
                    if entry.role_class == DecentralizedNetworkRoleClass::PublicMiner
                        && receipt.disposition != TrainingExecutionValidatorDisposition::Accepted
                    {
                        return Err(RewardLedgerContractError::InvalidContract {
                            detail: format!(
                                "miner contribution entry `{}` must stay tied to an accepted receipt",
                                entry.entry_id
                            ),
                        });
                    }
                }
                DecentralizedNetworkRoleClass::CheckpointAuthority => {
                    if !decision_ids.contains(entry.evidence_reference_id.as_str()) {
                        return Err(RewardLedgerContractError::InvalidContract {
                            detail: format!(
                                "checkpoint-authority contribution `{}` lost consensus decision binding",
                                entry.entry_id
                            ),
                        });
                    }
                }
                _ => {
                    return Err(RewardLedgerContractError::InvalidContract {
                        detail: format!(
                            "contribution entry `{}` uses unsupported role class in the first ledger",
                            entry.entry_id
                        ),
                    });
                }
            }
            gross_total = gross_total.saturating_add(entry.gross_reward_microunits);
        }
        if gross_total != self.accounting_period.reward_budget_microunits {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("contribution entries no longer match the reward budget"),
            });
        }

        let mut penalty_total = 0_u64;
        for entry in &self.penalty_entries {
            let slashing = slashing_by_id
                .get(entry.slashing_decision_id.as_str())
                .ok_or_else(|| RewardLedgerContractError::InvalidContract {
                    detail: format!(
                        "penalty entry `{}` references unknown slashing decision `{}`",
                        entry.entry_id, entry.slashing_decision_id
                    ),
                })?;
            if entry.node_identity_id != slashing.node_identity_id || entry.penalty_microunits == 0
            {
                return Err(RewardLedgerContractError::InvalidContract {
                    detail: format!("penalty entry `{}` drifted", entry.entry_id),
                });
            }
            penalty_total = penalty_total.saturating_add(entry.penalty_microunits);
        }
        if penalty_total != self.accounting_period.penalty_pool_microunits {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("penalty entries no longer match the penalty pool"),
            });
        }

        let mut payout_weight_total = 0_u16;
        let mut allocation_total = 0_i64;
        for allocation in &self.final_allocations {
            if !identity_by_id.contains_key(allocation.node_identity_id.as_str()) {
                return Err(RewardLedgerContractError::InvalidContract {
                    detail: format!(
                        "allocation `{}` references unknown node identity `{}`",
                        allocation.allocation_id, allocation.node_identity_id
                    ),
                });
            }
            if allocation.net_reward_microunits > 0 {
                payout_weight_total =
                    payout_weight_total.saturating_add(allocation.payout_weight_bps);
            } else if allocation.payout_weight_bps != 0 {
                return Err(RewardLedgerContractError::InvalidContract {
                    detail: format!(
                        "allocation `{}` retained payout weight despite nonpositive reward",
                        allocation.allocation_id
                    ),
                });
            }
            allocation_total = allocation_total.saturating_add(allocation.net_reward_microunits);
        }
        if payout_weight_total != 10_000 {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from(
                    "positive allocations must retain 10_000 basis points of payout weight",
                ),
            });
        }
        if allocation_total
            != i64::try_from(self.accounting_period.reward_budget_microunits).unwrap_or(i64::MAX)
                - i64::try_from(self.accounting_period.penalty_pool_microunits).unwrap_or(i64::MAX)
        {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from(
                    "final allocations no longer reconcile against budget minus penalties",
                ),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(RewardLedgerContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_reward_ledger_contract() -> Result<RewardLedgerContract, RewardLedgerContractError>
{
    let network = canonical_decentralized_network_contract()?;
    let identities = canonical_signed_node_identity_contract_set()?;
    let scoring = canonical_validator_challenge_scoring_contract()?;
    let consensus = canonical_multi_validator_consensus_contract()?;
    let fraud = canonical_fraud_quarantine_slashing_contract()?;

    let contribution_entries = vec![
        RewardLedgerContributionEntry {
            entry_id: String::from("contribution.public_miner.local_mlx.window1231"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            evidence_reference_id: String::from("score.public_validator.google.local_mlx.window1231"),
            gross_reward_microunits: 620_000,
            detail: String::from(
                "Apple MLX earns the miner share because the accepted validator receipt proves a positive contribution for the live public-miner window.",
            ),
        },
        RewardLedgerContributionEntry {
            entry_id: String::from("contribution.public_validator.google.window1231"),
            node_identity_id: String::from("google_l4_validator_node.identity"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            evidence_reference_id: String::from("score.public_validator.google.local_mlx.window1231"),
            gross_reward_microunits: 180_000,
            detail: String::from(
                "Google earns the larger validator share because it supplied the accepted challenge and score receipt for the canonical miner contribution.",
            ),
        },
        RewardLedgerContributionEntry {
            entry_id: String::from("contribution.public_validator.local_mlx.window1231"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            evidence_reference_id: String::from("score.public_validator.local_mlx.google.window1231"),
            gross_reward_microunits: 90_000,
            detail: String::from(
                "Apple MLX still earns a smaller validator accounting share because replay-required work remains billable validator labor even when it blocks promotion.",
            ),
        },
        RewardLedgerContributionEntry {
            entry_id: String::from("contribution.checkpoint_authority.google.window1231"),
            node_identity_id: String::from("google_l4_validator_node.identity"),
            role_class: DecentralizedNetworkRoleClass::CheckpointAuthority,
            evidence_reference_id: String::from("decision.checkpoint.step2048.round2056"),
            gross_reward_microunits: 60_000,
            detail: String::from(
                "Google earns one checkpoint-authority share for retaining and serving the held checkpoint candidate under the current multi-validator decision.",
            ),
        },
        RewardLedgerContributionEntry {
            entry_id: String::from("contribution.checkpoint_authority.runpod.window1231"),
            node_identity_id: String::from("runpod_8xh100_dense_node.identity"),
            role_class: DecentralizedNetworkRoleClass::CheckpointAuthority,
            evidence_reference_id: String::from("decision.checkpoint.step2048.round2056"),
            gross_reward_microunits: 50_000,
            detail: String::from(
                "RunPod earns the secondary checkpoint-authority share because the decentralized network still retains it as a mirrored durable authority.",
            ),
        },
    ];

    let penalty_entries = vec![RewardLedgerPenaltyEntry {
        entry_id: String::from("penalty.public_miner.local_rtx4080.window1231"),
        node_identity_id: String::from("local_rtx4080_workstation.identity"),
        slashing_decision_id: String::from("slash.public_miner.local_rtx4080.window1232"),
        penalty_microunits: 150_000,
        detail: String::from(
            "The first ledger deducts the slashed RTX 4080 miner amount directly from the provisional reward pool so the retained ledger says both who earned and who was penalized.",
        ),
    }];

    let final_allocations = vec![
        RewardLedgerFinalAllocation {
            allocation_id: String::from("allocation.local_mlx.window1231"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            net_reward_microunits: 710_000,
            payout_weight_bps: 7100,
            detail: String::from(
                "Apple MLX combines miner and validator earnings into the largest net allocation for the window.",
            ),
        },
        RewardLedgerFinalAllocation {
            allocation_id: String::from("allocation.google.window1231"),
            node_identity_id: String::from("google_l4_validator_node.identity"),
            net_reward_microunits: 240_000,
            payout_weight_bps: 2400,
            detail: String::from(
                "Google combines validator and checkpoint-authority earnings into the second-largest net allocation for the window.",
            ),
        },
        RewardLedgerFinalAllocation {
            allocation_id: String::from("allocation.runpod.window1231"),
            node_identity_id: String::from("runpod_8xh100_dense_node.identity"),
            net_reward_microunits: 50_000,
            payout_weight_bps: 500,
            detail: String::from(
                "RunPod retains one smaller checkpoint-authority allocation for mirrored state retention.",
            ),
        },
        RewardLedgerFinalAllocation {
            allocation_id: String::from("allocation.local_rtx4080.window1231"),
            node_identity_id: String::from("local_rtx4080_workstation.identity"),
            net_reward_microunits: -150_000,
            payout_weight_bps: 0,
            detail: String::from(
                "The slashed RTX 4080 miner carries a negative net allocation and no payout weight because the duplicate-work incident crossed the current fraud threshold.",
            ),
        },
    ];

    let mut contract = RewardLedgerContract {
        schema_version: String::from(REWARD_LEDGER_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(REWARD_LEDGER_CONTRACT_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        signed_node_identity_contract_set_digest: identities.contract_digest.clone(),
        validator_challenge_scoring_contract_digest: scoring.contract_digest.clone(),
        multi_validator_consensus_contract_digest: consensus.contract_digest.clone(),
        fraud_quarantine_slashing_contract_digest: fraud.contract_digest.clone(),
        accounting_period: RewardLedgerAccountingPeriod {
            accounting_period_id: String::from("ledger.window1231"),
            opens_epoch_id: String::from("window1231"),
            closes_epoch_id: String::from("window1232"),
            reward_budget_microunits: 1_000_000,
            penalty_pool_microunits: 150_000,
            detail: String::from(
                "The first public ledger closes one public accounting window under the current decentralized network cadence and records both positive work and penalties in the same retained period.",
            ),
        },
        contribution_entries,
        penalty_entries,
        final_allocations,
        authority_paths: RewardLedgerAuthorityPaths {
            fixture_path: String::from(REWARD_LEDGER_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(REWARD_LEDGER_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(REWARD_LEDGER_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(REWARD_LEDGER_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first tamper-evident contribution ledger: one accounting period, retained gross work entries, retained penalty entries, and payout-ready net allocations. It does not yet publish those outcomes outside the local accounting surface.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_reward_ledger_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), RewardLedgerContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| RewardLedgerContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_reward_ledger_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| RewardLedgerContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for reward ledger contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{canonical_reward_ledger_contract, RewardLedgerContractError};

    #[test]
    fn canonical_reward_ledger_contract_is_valid() -> Result<(), RewardLedgerContractError> {
        let contract = canonical_reward_ledger_contract()?;
        contract.validate()
    }

    #[test]
    fn positive_allocations_must_keep_payout_weight() -> Result<(), RewardLedgerContractError> {
        let mut contract = canonical_reward_ledger_contract()?;
        let allocation = contract
            .final_allocations
            .iter_mut()
            .find(|allocation| allocation.allocation_id == "allocation.runpod.window1231")
            .expect("canonical contract should retain the RunPod allocation");
        allocation.payout_weight_bps = 0;
        let error = contract
            .validate()
            .expect_err("positive allocations cannot lose payout weight");
        assert!(matches!(
            error,
            RewardLedgerContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
