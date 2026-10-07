use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_multi_validator_consensus_contract, canonical_public_network_registry_contract,
    canonical_reward_ledger_contract, canonical_settlement_publication_contract,
    MultiValidatorConsensusContractError, PublicNetworkRegistryContractError,
    RewardLedgerContractError, SettlementPublicationContractError,
};

pub const PUBLIC_RUN_EXPLORER_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.public_run_explorer_contract.v1";
pub const PUBLIC_RUN_EXPLORER_CONTRACT_ID: &str = "psionic.public_run_explorer_contract.v1";
pub const PUBLIC_RUN_EXPLORER_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/public_run_explorer_contract_v1.json";
pub const PUBLIC_RUN_EXPLORER_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-public-run-explorer-contract.sh";
pub const PUBLIC_RUN_EXPLORER_CONTRACT_DOC_PATH: &str = "docs/PUBLIC_RUN_EXPLORER_REFERENCE.md";
pub const PUBLIC_RUN_EXPLORER_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum PublicRunExplorerContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    MultiValidatorConsensus(#[from] MultiValidatorConsensusContractError),
    #[error(transparent)]
    RewardLedger(#[from] RewardLedgerContractError),
    #[error(transparent)]
    SettlementPublication(#[from] SettlementPublicationContractError),
    #[error("public run explorer contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplorerPaneKind {
    NetworkEpoch,
    NodeStatus,
    Scoreboard,
    PromotionTimeline,
    RewardLedger,
    SettlementFeed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerPane {
    pub pane_id: String,
    pub pane_kind: ExplorerPaneKind,
    pub backing_reference_ids: Vec<String>,
    pub refresh_interval_seconds: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerSnapshot {
    pub snapshot_id: String,
    pub current_epoch_id: String,
    pub online_node_count: u16,
    pub validator_node_count: u16,
    pub held_promotion_count: u16,
    pub published_settlement_count: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerScoreRow {
    pub row_id: String,
    pub node_identity_id: String,
    pub accepted_score_receipt_ids: Vec<String>,
    pub replay_required_score_receipt_ids: Vec<String>,
    pub net_reward_microunits: i64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerStaleDataPolicy {
    pub pane_id: String,
    pub stale_after_seconds: u16,
    pub fallback_message: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicRunExplorerAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicRunExplorerContract {
    pub schema_version: String,
    pub contract_id: String,
    pub public_network_registry_contract_digest: String,
    pub multi_validator_consensus_contract_digest: String,
    pub reward_ledger_contract_digest: String,
    pub settlement_publication_contract_digest: String,
    pub panes: Vec<ExplorerPane>,
    pub snapshot: ExplorerSnapshot,
    pub score_rows: Vec<ExplorerScoreRow>,
    pub stale_data_policies: Vec<ExplorerStaleDataPolicy>,
    pub authority_paths: PublicRunExplorerAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl PublicRunExplorerContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_public_run_explorer_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), PublicRunExplorerContractError> {
        let registry = canonical_public_network_registry_contract()?;
        let consensus = canonical_multi_validator_consensus_contract()?;
        let ledger = canonical_reward_ledger_contract()?;
        let settlement = canonical_settlement_publication_contract()?;

        let registry_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let score_receipt_ids = settlement
            .validator_weight_publications
            .iter()
            .map(|publication| publication.score_receipt_id.as_str())
            .collect::<BTreeSet<_>>();
        let allocation_by_node = ledger
            .final_allocations
            .iter()
            .map(|allocation| {
                (
                    allocation.node_identity_id.as_str(),
                    allocation.net_reward_microunits,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let decision_ids = consensus
            .promotion_decisions
            .iter()
            .map(|decision| decision.decision_id.as_str())
            .collect::<BTreeSet<_>>();
        let settlement_record_ids = settlement
            .settlement_records
            .iter()
            .map(|record| record.record_id.as_str())
            .collect::<BTreeSet<_>>();

        if self.schema_version != PUBLIC_RUN_EXPLORER_CONTRACT_SCHEMA_VERSION {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    PUBLIC_RUN_EXPLORER_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != PUBLIC_RUN_EXPLORER_CONTRACT_ID {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.public_network_registry_contract_digest != registry.contract_digest
            || self.multi_validator_consensus_contract_digest != consensus.contract_digest
            || self.reward_ledger_contract_digest != ledger.contract_digest
            || self.settlement_publication_contract_digest != settlement.contract_digest
        {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != PUBLIC_RUN_EXPLORER_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != PUBLIC_RUN_EXPLORER_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != PUBLIC_RUN_EXPLORER_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != PUBLIC_RUN_EXPLORER_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.panes.len() != 6
            || self.score_rows.len() != 4
            || self.stale_data_policies.len() != 6
        {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: String::from(
                    "expected canonical pane, scoreboard, and stale-policy counts",
                ),
            });
        }

        for pane in &self.panes {
            if pane.backing_reference_ids.is_empty() || pane.refresh_interval_seconds == 0 {
                return Err(PublicRunExplorerContractError::InvalidContract {
                    detail: format!("explorer pane `{}` drifted", pane.pane_id),
                });
            }
            match pane.pane_kind {
                ExplorerPaneKind::NetworkEpoch | ExplorerPaneKind::NodeStatus => {
                    for reference_id in &pane.backing_reference_ids {
                        if !registry_ids.contains(reference_id.as_str()) {
                            return Err(PublicRunExplorerContractError::InvalidContract {
                                detail: format!(
                                    "explorer pane `{}` lost registry binding `{}`",
                                    pane.pane_id, reference_id
                                ),
                            });
                        }
                    }
                }
                ExplorerPaneKind::Scoreboard => {
                    for reference_id in &pane.backing_reference_ids {
                        if !score_receipt_ids.contains(reference_id.as_str()) {
                            return Err(PublicRunExplorerContractError::InvalidContract {
                                detail: format!(
                                    "explorer pane `{}` lost score binding `{}`",
                                    pane.pane_id, reference_id
                                ),
                            });
                        }
                    }
                }
                ExplorerPaneKind::PromotionTimeline => {
                    for reference_id in &pane.backing_reference_ids {
                        if !decision_ids.contains(reference_id.as_str()) {
                            return Err(PublicRunExplorerContractError::InvalidContract {
                                detail: format!(
                                    "explorer pane `{}` lost decision binding `{}`",
                                    pane.pane_id, reference_id
                                ),
                            });
                        }
                    }
                }
                ExplorerPaneKind::RewardLedger => {
                    if pane.backing_reference_ids
                        != vec![ledger.accounting_period.accounting_period_id.clone()]
                    {
                        return Err(PublicRunExplorerContractError::InvalidContract {
                            detail: format!("reward-ledger pane `{}` drifted", pane.pane_id),
                        });
                    }
                }
                ExplorerPaneKind::SettlementFeed => {
                    for reference_id in &pane.backing_reference_ids {
                        if !settlement_record_ids.contains(reference_id.as_str()) {
                            return Err(PublicRunExplorerContractError::InvalidContract {
                                detail: format!(
                                    "explorer pane `{}` lost settlement binding `{}`",
                                    pane.pane_id, reference_id
                                ),
                            });
                        }
                    }
                }
            }
        }

        if self.snapshot.online_node_count != 4
            || self.snapshot.validator_node_count != 2
            || self.snapshot.held_promotion_count != 1
            || self.snapshot.published_settlement_count != 1
        {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: String::from("explorer snapshot drifted"),
            });
        }

        for row in &self.score_rows {
            let net_reward = allocation_by_node
                .get(row.node_identity_id.as_str())
                .ok_or_else(|| PublicRunExplorerContractError::InvalidContract {
                    detail: format!(
                        "score row `{}` references unknown allocation node `{}`",
                        row.row_id, row.node_identity_id
                    ),
                })?;
            if *net_reward != row.net_reward_microunits {
                return Err(PublicRunExplorerContractError::InvalidContract {
                    detail: format!("score row `{}` drifted", row.row_id),
                });
            }
            for reference_id in &row.accepted_score_receipt_ids {
                if !score_receipt_ids.contains(reference_id.as_str()) {
                    return Err(PublicRunExplorerContractError::InvalidContract {
                        detail: format!(
                            "score row `{}` lost accepted score binding `{}`",
                            row.row_id, reference_id
                        ),
                    });
                }
            }
            for reference_id in &row.replay_required_score_receipt_ids {
                if !score_receipt_ids.contains(reference_id.as_str()) {
                    return Err(PublicRunExplorerContractError::InvalidContract {
                        detail: format!(
                            "score row `{}` lost replay-required score binding `{}`",
                            row.row_id, reference_id
                        ),
                    });
                }
            }
        }

        for policy in &self.stale_data_policies {
            if !self.panes.iter().any(|pane| pane.pane_id == policy.pane_id)
                || policy.stale_after_seconds == 0
            {
                return Err(PublicRunExplorerContractError::InvalidContract {
                    detail: format!("stale-data policy for `{}` drifted", policy.pane_id),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(PublicRunExplorerContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_public_run_explorer_contract(
) -> Result<PublicRunExplorerContract, PublicRunExplorerContractError> {
    let registry = canonical_public_network_registry_contract()?;
    let consensus = canonical_multi_validator_consensus_contract()?;
    let ledger = canonical_reward_ledger_contract()?;
    let settlement = canonical_settlement_publication_contract()?;

    let panes = vec![
        ExplorerPane {
            pane_id: String::from("pane.network_epoch"),
            pane_kind: ExplorerPaneKind::NetworkEpoch,
            backing_reference_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ],
            refresh_interval_seconds: 15,
            detail: String::from(
                "The epoch pane shows the current public window against the two most important online public nodes.",
            ),
        },
        ExplorerPane {
            pane_id: String::from("pane.node_status"),
            pane_kind: ExplorerPaneKind::NodeStatus,
            backing_reference_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
                String::from("local_rtx4080_workstation.registry"),
                String::from("local_mlx_mac_workstation.registry"),
            ],
            refresh_interval_seconds: 20,
            detail: String::from(
                "The node-status pane publishes the current online public-network registry state for all currently admitted nodes.",
            ),
        },
        ExplorerPane {
            pane_id: String::from("pane.scoreboard"),
            pane_kind: ExplorerPaneKind::Scoreboard,
            backing_reference_ids: vec![
                String::from("score.public_validator.google.local_mlx.window1231"),
                String::from("score.public_validator.local_mlx.google.window1231"),
            ],
            refresh_interval_seconds: 20,
            detail: String::from(
                "The scoreboard pane publishes accepted and replay-required validator score receipts for the current public window.",
            ),
        },
        ExplorerPane {
            pane_id: String::from("pane.promotion"),
            pane_kind: ExplorerPaneKind::PromotionTimeline,
            backing_reference_ids: vec![String::from("decision.checkpoint.step2048.round2056")],
            refresh_interval_seconds: 30,
            detail: String::from(
                "The promotion pane publishes the held-no-promotion checkpoint decision and its current disagreement status.",
            ),
        },
        ExplorerPane {
            pane_id: String::from("pane.reward_ledger"),
            pane_kind: ExplorerPaneKind::RewardLedger,
            backing_reference_ids: vec![String::from("ledger.window1231")],
            refresh_interval_seconds: 30,
            detail: String::from(
                "The reward-ledger pane publishes the current accounting-period closeout and payout weights.",
            ),
        },
        ExplorerPane {
            pane_id: String::from("pane.settlement_feed"),
            pane_kind: ExplorerPaneKind::SettlementFeed,
            backing_reference_ids: vec![String::from("settlement.window1231.signed")],
            refresh_interval_seconds: 45,
            detail: String::from(
                "The settlement pane publishes the signed-ledger settlement record once the accounting period closes.",
            ),
        },
    ];

    let score_rows = vec![
        ExplorerScoreRow {
            row_id: String::from("row.local_mlx.window1231"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            accepted_score_receipt_ids: vec![String::from(
                "score.public_validator.google.local_mlx.window1231",
            )],
            replay_required_score_receipt_ids: vec![String::from(
                "score.public_validator.local_mlx.google.window1231",
            )],
            net_reward_microunits: 710_000,
            detail: String::from(
                "Apple MLX shows up on the explorer both as the accepted miner and as the second validator in the current public window.",
            ),
        },
        ExplorerScoreRow {
            row_id: String::from("row.google.window1231"),
            node_identity_id: String::from("google_l4_validator_node.identity"),
            accepted_score_receipt_ids: vec![String::from(
                "score.public_validator.google.local_mlx.window1231",
            )],
            replay_required_score_receipt_ids: vec![],
            net_reward_microunits: 240_000,
            detail: String::from(
                "Google shows up as the validator that admitted the accepted miner contribution and as a checkpoint authority.",
            ),
        },
        ExplorerScoreRow {
            row_id: String::from("row.runpod.window1231"),
            node_identity_id: String::from("runpod_8xh100_dense_node.identity"),
            accepted_score_receipt_ids: vec![],
            replay_required_score_receipt_ids: vec![],
            net_reward_microunits: 50_000,
            detail: String::from(
                "RunPod stays visible on the explorer through checkpoint-authority accounting even without validator score receipts.",
            ),
        },
        ExplorerScoreRow {
            row_id: String::from("row.local_rtx4080.window1231"),
            node_identity_id: String::from("local_rtx4080_workstation.identity"),
            accepted_score_receipt_ids: vec![],
            replay_required_score_receipt_ids: vec![],
            net_reward_microunits: -150_000,
            detail: String::from(
                "The slashed RTX 4080 node remains visible with a negative balance instead of being silently erased from the public ledger view.",
            ),
        },
    ];

    let stale_data_policies = panes
        .iter()
        .map(|pane| ExplorerStaleDataPolicy {
            pane_id: pane.pane_id.clone(),
            stale_after_seconds: 90,
            fallback_message: String::from("stale_data"),
            detail: String::from(
                "Each explorer pane must surface explicit stale-data state instead of silently presenting old network information as current truth.",
            ),
        })
        .collect::<Vec<_>>();

    let mut contract = PublicRunExplorerContract {
        schema_version: String::from(PUBLIC_RUN_EXPLORER_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PUBLIC_RUN_EXPLORER_CONTRACT_ID),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        multi_validator_consensus_contract_digest: consensus.contract_digest.clone(),
        reward_ledger_contract_digest: ledger.contract_digest.clone(),
        settlement_publication_contract_digest: settlement.contract_digest.clone(),
        panes,
        snapshot: ExplorerSnapshot {
            snapshot_id: String::from("snapshot.public_run.window1231"),
            current_epoch_id: String::from("window1231"),
            online_node_count: 4,
            validator_node_count: 2,
            held_promotion_count: 1,
            published_settlement_count: 1,
            detail: String::from(
                "The first explorer snapshot freezes one current public-network view: four online nodes, two validator-capable nodes, one held checkpoint decision, and one published signed-ledger settlement.",
            ),
        },
        score_rows,
        stale_data_policies,
        authority_paths: PublicRunExplorerAuthorityPaths {
            fixture_path: String::from(PUBLIC_RUN_EXPLORER_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PUBLIC_RUN_EXPLORER_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(PUBLIC_RUN_EXPLORER_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                PUBLIC_RUN_EXPLORER_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first public run explorer surface: panes, one network snapshot, score rows, and stale-data policy. It does not yet claim staged public testnet graduation or public participation windows.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_public_run_explorer_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), PublicRunExplorerContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PublicRunExplorerContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_public_run_explorer_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| PublicRunExplorerContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for public run explorer contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{canonical_public_run_explorer_contract, PublicRunExplorerContractError};

    #[test]
    fn canonical_public_run_explorer_contract_is_valid(
    ) -> Result<(), PublicRunExplorerContractError> {
        let contract = canonical_public_run_explorer_contract()?;
        contract.validate()
    }

    #[test]
    fn reward_pane_cannot_drift_to_unknown_period() -> Result<(), PublicRunExplorerContractError> {
        let mut contract = canonical_public_run_explorer_contract()?;
        let pane = contract
            .panes
            .iter_mut()
            .find(|pane| pane.pane_kind == super::ExplorerPaneKind::RewardLedger)
            .expect("canonical explorer should retain the reward ledger pane");
        pane.backing_reference_ids = vec![String::from("ledger.window9999")];
        let error = contract
            .validate()
            .expect_err("reward pane cannot drift to an unknown accounting period");
        assert!(matches!(
            error,
            PublicRunExplorerContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
