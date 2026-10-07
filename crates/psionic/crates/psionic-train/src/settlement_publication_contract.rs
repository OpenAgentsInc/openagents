use std::{
    collections::BTreeMap,
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_decentralized_network_contract, canonical_reward_ledger_contract,
    canonical_signed_node_identity_contract_set, canonical_validator_challenge_scoring_contract,
    DecentralizedNetworkContractError, RewardLedgerContractError,
    SignedNodeIdentityContractSetError, ValidatorChallengeScoringContractError,
};

pub const SETTLEMENT_PUBLICATION_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.settlement_publication_contract.v1";
pub const SETTLEMENT_PUBLICATION_CONTRACT_ID: &str = "psionic.settlement_publication_contract.v1";
pub const SETTLEMENT_PUBLICATION_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/settlement_publication_contract_v1.json";
pub const SETTLEMENT_PUBLICATION_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-settlement-publication-contract.sh";
pub const SETTLEMENT_PUBLICATION_CONTRACT_DOC_PATH: &str =
    "docs/SETTLEMENT_PUBLICATION_REFERENCE.md";
pub const SETTLEMENT_PUBLICATION_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum SettlementPublicationContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    RewardLedger(#[from] RewardLedgerContractError),
    #[error(transparent)]
    SignedNodeIdentity(#[from] SignedNodeIdentityContractSetError),
    #[error(transparent)]
    ValidatorScoring(#[from] ValidatorChallengeScoringContractError),
    #[error("settlement publication contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementPublicationBackendKind {
    SignedLedgerBundle,
    ChainAttestationAdapter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementRefusalKind {
    ChainAdapterDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorWeightPublication {
    pub publication_id: String,
    pub validator_registry_record_id: String,
    pub score_receipt_id: String,
    pub published_weight_bps: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementRecord {
    pub record_id: String,
    pub backend_kind: SettlementPublicationBackendKind,
    pub accounting_period_id: String,
    pub published_epoch_id: String,
    pub published_ledger_path: String,
    pub included_allocation_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayoutExport {
    pub export_id: String,
    pub settlement_record_id: String,
    pub node_identity_id: String,
    pub destination_address: String,
    pub payout_weight_bps: u16,
    pub payout_microunits: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementPublicationRefusal {
    pub refusal_id: String,
    pub backend_kind: SettlementPublicationBackendKind,
    pub refusal_kind: SettlementRefusalKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementPublicationAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementPublicationContract {
    pub schema_version: String,
    pub contract_id: String,
    pub decentralized_network_contract_digest: String,
    pub signed_node_identity_contract_set_digest: String,
    pub reward_ledger_contract_digest: String,
    pub validator_challenge_scoring_contract_digest: String,
    pub validator_weight_publications: Vec<ValidatorWeightPublication>,
    pub settlement_records: Vec<SettlementRecord>,
    pub payout_exports: Vec<PayoutExport>,
    pub refusals: Vec<SettlementPublicationRefusal>,
    pub authority_paths: SettlementPublicationAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl SettlementPublicationContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_settlement_publication_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), SettlementPublicationContractError> {
        let network = canonical_decentralized_network_contract()?;
        let identities = canonical_signed_node_identity_contract_set()?;
        let ledger = canonical_reward_ledger_contract()?;
        let scoring = canonical_validator_challenge_scoring_contract()?;

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
        let allocation_by_id = ledger
            .final_allocations
            .iter()
            .map(|allocation| (allocation.allocation_id.as_str(), allocation))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != SETTLEMENT_PUBLICATION_CONTRACT_SCHEMA_VERSION {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    SETTLEMENT_PUBLICATION_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != SETTLEMENT_PUBLICATION_CONTRACT_ID {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest
            || self.signed_node_identity_contract_set_digest != identities.contract_digest
            || self.reward_ledger_contract_digest != ledger.contract_digest
            || self.validator_challenge_scoring_contract_digest != scoring.contract_digest
        {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != SETTLEMENT_PUBLICATION_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != SETTLEMENT_PUBLICATION_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != SETTLEMENT_PUBLICATION_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != SETTLEMENT_PUBLICATION_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.validator_weight_publications.len() != 2
            || self.settlement_records.len() != 1
            || self.payout_exports.len() != 3
            || self.refusals.len() != 1
        {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from(
                    "expected canonical weight publication, settlement, payout, and refusal counts",
                ),
            });
        }

        let mut weight_total = 0_u16;
        for publication in &self.validator_weight_publications {
            let receipt = score_receipt_by_id
                .get(publication.score_receipt_id.as_str())
                .ok_or_else(|| SettlementPublicationContractError::InvalidContract {
                    detail: format!(
                        "weight publication `{}` references unknown score receipt `{}`",
                        publication.publication_id, publication.score_receipt_id
                    ),
                })?;
            if publication.validator_registry_record_id != receipt.validator_registry_record_id
                || publication.published_weight_bps == 0
            {
                return Err(SettlementPublicationContractError::InvalidContract {
                    detail: format!(
                        "weight publication `{}` drifted",
                        publication.publication_id
                    ),
                });
            }
            weight_total = weight_total.saturating_add(publication.published_weight_bps);
        }
        if weight_total != 10_000 {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from(
                    "validator weight publications must total 10_000 basis points",
                ),
            });
        }

        let record = self.settlement_records.first().ok_or_else(|| {
            SettlementPublicationContractError::InvalidContract {
                detail: String::from("expected one settlement record"),
            }
        })?;
        if record.backend_kind != SettlementPublicationBackendKind::SignedLedgerBundle
            || record.accounting_period_id != ledger.accounting_period.accounting_period_id
            || record.included_allocation_ids.len() != 3
        {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from("settlement record drifted"),
            });
        }
        for allocation_id in &record.included_allocation_ids {
            let allocation = allocation_by_id
                .get(allocation_id.as_str())
                .ok_or_else(|| SettlementPublicationContractError::InvalidContract {
                    detail: format!(
                        "settlement record references unknown allocation `{}`",
                        allocation_id
                    ),
                })?;
            if allocation.net_reward_microunits <= 0 {
                return Err(SettlementPublicationContractError::InvalidContract {
                    detail: format!(
                        "settlement record retained nonpositive allocation `{}`",
                        allocation_id
                    ),
                });
            }
        }

        let mut payout_total = 0_u64;
        let mut payout_weight_total = 0_u16;
        for payout in &self.payout_exports {
            let identity = identity_by_id
                .get(payout.node_identity_id.as_str())
                .ok_or_else(|| SettlementPublicationContractError::InvalidContract {
                    detail: format!(
                        "payout export `{}` references unknown node identity `{}`",
                        payout.export_id, payout.node_identity_id
                    ),
                })?;
            if payout.settlement_record_id != record.record_id
                || payout.destination_address != identity.wallet.wallet_address
                || payout.payout_weight_bps == 0
                || payout.payout_microunits == 0
            {
                return Err(SettlementPublicationContractError::InvalidContract {
                    detail: format!("payout export `{}` drifted", payout.export_id),
                });
            }
            payout_total = payout_total.saturating_add(payout.payout_microunits);
            payout_weight_total = payout_weight_total.saturating_add(payout.payout_weight_bps);
        }
        if payout_total != 1_000_000 || payout_weight_total != 10_000 {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from(
                    "payout exports no longer match published rewards and weights",
                ),
            });
        }

        let refusal = self.refusals.first().ok_or_else(|| {
            SettlementPublicationContractError::InvalidContract {
                detail: String::from("expected one settlement refusal"),
            }
        })?;
        if refusal.backend_kind != SettlementPublicationBackendKind::ChainAttestationAdapter
            || refusal.refusal_kind != SettlementRefusalKind::ChainAdapterDisabled
        {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from("settlement refusal drifted"),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(SettlementPublicationContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_settlement_publication_contract(
) -> Result<SettlementPublicationContract, SettlementPublicationContractError> {
    let network = canonical_decentralized_network_contract()?;
    let identities = canonical_signed_node_identity_contract_set()?;
    let ledger = canonical_reward_ledger_contract()?;
    let scoring = canonical_validator_challenge_scoring_contract()?;

    let validator_weight_publications = vec![
        ValidatorWeightPublication {
            publication_id: String::from("validator_weight.google.window1231"),
            validator_registry_record_id: String::from("google_l4_validator_node.registry"),
            score_receipt_id: String::from("score.public_validator.google.local_mlx.window1231"),
            published_weight_bps: 5000,
            detail: String::from(
                "Google publishes half of the current validator weight surface because the first public network still retains a two-validator quorum.",
            ),
        },
        ValidatorWeightPublication {
            publication_id: String::from("validator_weight.local_mlx.window1231"),
            validator_registry_record_id: String::from("local_mlx_mac_workstation.registry"),
            score_receipt_id: String::from("score.public_validator.local_mlx.google.window1231"),
            published_weight_bps: 5000,
            detail: String::from(
                "Apple MLX publishes the remaining half of the validator weight surface under the same two-validator quorum.",
            ),
        },
    ];

    let settlement_records = vec![SettlementRecord {
        record_id: String::from("settlement.window1231.signed"),
        backend_kind: SettlementPublicationBackendKind::SignedLedgerBundle,
        accounting_period_id: String::from("ledger.window1231"),
        published_epoch_id: String::from("window1232"),
        published_ledger_path: String::from(
            "artifacts/settlement/window1231/signed-ledger-bundle.json",
        ),
        included_allocation_ids: vec![
            String::from("allocation.local_mlx.window1231"),
            String::from("allocation.google.window1231"),
            String::from("allocation.runpod.window1231"),
        ],
        detail: String::from(
            "The first settlement record publishes the positive public-network allocations through the signed-ledger backend already frozen by the decentralized network contract.",
        ),
    }];

    let wallet_by_id = identities
        .identities
        .iter()
        .map(|identity| {
            (
                identity.node_identity_id.as_str(),
                identity.wallet.wallet_address.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let payout_exports = vec![
        PayoutExport {
            export_id: String::from("payout.local_mlx.window1231"),
            settlement_record_id: String::from("settlement.window1231.signed"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            destination_address: String::from(wallet_by_id["local_mlx_mac_workstation.identity"]),
            payout_weight_bps: 7100,
            payout_microunits: 710_000,
            detail: String::from(
                "Apple MLX receives the largest payout export because it carried both the accepted miner contribution and one validator share.",
            ),
        },
        PayoutExport {
            export_id: String::from("payout.google.window1231"),
            settlement_record_id: String::from("settlement.window1231.signed"),
            node_identity_id: String::from("google_l4_validator_node.identity"),
            destination_address: String::from(wallet_by_id["google_l4_validator_node.identity"]),
            payout_weight_bps: 2400,
            payout_microunits: 240_000,
            detail: String::from(
                "Google receives the second-largest payout export for validator and checkpoint-authority work.",
            ),
        },
        PayoutExport {
            export_id: String::from("payout.runpod.window1231"),
            settlement_record_id: String::from("settlement.window1231.signed"),
            node_identity_id: String::from("runpod_8xh100_dense_node.identity"),
            destination_address: String::from(wallet_by_id["runpod_8xh100_dense_node.identity"]),
            payout_weight_bps: 500,
            payout_microunits: 50_000,
            detail: String::from(
                "RunPod receives the remaining positive payout export for mirrored checkpoint-authority work.",
            ),
        },
    ];

    let refusals = vec![SettlementPublicationRefusal {
        refusal_id: String::from("refusal.settlement.chain.window1231"),
        backend_kind: SettlementPublicationBackendKind::ChainAttestationAdapter,
        refusal_kind: SettlementRefusalKind::ChainAdapterDisabled,
        detail: String::from(
            "The first settlement surface refuses chain publication by default because the signed-ledger backend is canonical today and optional chain adapters remain disabled until later operator rollout.",
        ),
    }];

    let mut contract = SettlementPublicationContract {
        schema_version: String::from(SETTLEMENT_PUBLICATION_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(SETTLEMENT_PUBLICATION_CONTRACT_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        signed_node_identity_contract_set_digest: identities.contract_digest.clone(),
        reward_ledger_contract_digest: ledger.contract_digest.clone(),
        validator_challenge_scoring_contract_digest: scoring.contract_digest.clone(),
        validator_weight_publications,
        settlement_records,
        payout_exports,
        refusals,
        authority_paths: SettlementPublicationAuthorityPaths {
            fixture_path: String::from(SETTLEMENT_PUBLICATION_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(SETTLEMENT_PUBLICATION_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(SETTLEMENT_PUBLICATION_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                SETTLEMENT_PUBLICATION_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first settlement publication surface: validator weight publication, a signed-ledger settlement record, payout-ready exports, and one explicit chain-adapter refusal. It does not yet claim public explorer packaging or open-operator onboarding.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_settlement_publication_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), SettlementPublicationContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            SettlementPublicationContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_settlement_publication_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| SettlementPublicationContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher
        .update(serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for settlement publication contract",
        ));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{canonical_settlement_publication_contract, SettlementPublicationContractError};

    #[test]
    fn canonical_settlement_publication_contract_is_valid(
    ) -> Result<(), SettlementPublicationContractError> {
        let contract = canonical_settlement_publication_contract()?;
        contract.validate()
    }

    #[test]
    fn settlement_record_cannot_include_negative_allocation(
    ) -> Result<(), SettlementPublicationContractError> {
        let mut contract = canonical_settlement_publication_contract()?;
        contract.settlement_records[0]
            .included_allocation_ids
            .push(String::from("allocation.local_rtx4080.window1231"));
        let error = contract
            .validate()
            .expect_err("negative allocations cannot be published as payouts");
        assert!(matches!(
            error,
            SettlementPublicationContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
