use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_decentralized_network_contract, canonical_public_run_explorer_contract,
    canonical_public_testnet_readiness_contract, canonical_settlement_publication_contract,
    DecentralizedNetworkContractError, DecentralizedNetworkRoleClass,
    PublicRunExplorerContractError, PublicTestnetReadinessContractError,
    SettlementPublicationContractError,
};

pub const CURATED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.curated_decentralized_run_contract.v1";
pub const CURATED_DECENTRALIZED_RUN_CONTRACT_ID: &str =
    "psionic.curated_decentralized_run_contract.v1";
pub const CURATED_DECENTRALIZED_RUN_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/curated_decentralized_run_contract_v1.json";
pub const CURATED_DECENTRALIZED_RUN_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-curated-decentralized-run-contract.sh";
pub const CURATED_DECENTRALIZED_RUN_CONTRACT_DOC_PATH: &str =
    "docs/CURATED_DECENTRALIZED_RUN_REFERENCE.md";
pub const CURATED_DECENTRALIZED_RUN_AUDIT_DOC_PATH: &str =
    "docs/audits/2026-03-26-curated-decentralized-run-after-action-audit.md";
pub const CURATED_DECENTRALIZED_RUN_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum CuratedDecentralizedRunContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    Readiness(#[from] PublicTestnetReadinessContractError),
    #[error(transparent)]
    Explorer(#[from] PublicRunExplorerContractError),
    #[error(transparent)]
    Settlement(#[from] SettlementPublicationContractError),
    #[error("curated decentralized run contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CuratedRunParticipationMode {
    CuratedPermissioned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CuratedRunPromotionStatus {
    HeldNoPromotion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedRunParticipant {
    pub participant_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub authority_reference_id: String,
    pub runtime_reference_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedRunEvidenceBundle {
    pub bundle_id: String,
    pub explorer_snapshot_id: String,
    pub settlement_record_id: String,
    pub retained_paths: Vec<String>,
    pub after_action_audit_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedRunOutcome {
    pub outcome_id: String,
    pub start_epoch_id: String,
    pub end_epoch_id: String,
    pub participation_mode: CuratedRunParticipationMode,
    pub promotion_status: CuratedRunPromotionStatus,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedDecentralizedRunAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub audit_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedDecentralizedRunContract {
    pub schema_version: String,
    pub contract_id: String,
    pub decentralized_network_contract_digest: String,
    pub public_testnet_readiness_contract_digest: String,
    pub public_run_explorer_contract_digest: String,
    pub settlement_publication_contract_digest: String,
    pub participants: Vec<CuratedRunParticipant>,
    pub evidence_bundle: CuratedRunEvidenceBundle,
    pub outcome: CuratedRunOutcome,
    pub authority_paths: CuratedDecentralizedRunAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl CuratedDecentralizedRunContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_curated_decentralized_run_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), CuratedDecentralizedRunContractError> {
        let network = canonical_decentralized_network_contract()?;
        let readiness = canonical_public_testnet_readiness_contract()?;
        let explorer = canonical_public_run_explorer_contract()?;
        let settlement = canonical_settlement_publication_contract()?;

        let readiness_ids = readiness
            .graduation_decisions
            .iter()
            .map(|decision| decision.decision_id.as_str())
            .collect::<BTreeSet<_>>();
        let registry_ids = [
            "google_l4_validator_node.registry",
            "runpod_8xh100_dense_node.registry",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();

        if self.schema_version != CURATED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION {
            return Err(CuratedDecentralizedRunContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CURATED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != CURATED_DECENTRALIZED_RUN_CONTRACT_ID {
            return Err(CuratedDecentralizedRunContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest
            || self.public_testnet_readiness_contract_digest != readiness.contract_digest
            || self.public_run_explorer_contract_digest != explorer.contract_digest
            || self.settlement_publication_contract_digest != settlement.contract_digest
        {
            return Err(CuratedDecentralizedRunContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.participants.len() != 4
            || self.evidence_bundle.after_action_audit_path
                != CURATED_DECENTRALIZED_RUN_AUDIT_DOC_PATH
            || self.authority_paths.audit_doc_path != CURATED_DECENTRALIZED_RUN_AUDIT_DOC_PATH
            || self.evidence_bundle.retained_paths.len() != 4
        {
            return Err(CuratedDecentralizedRunContractError::InvalidContract {
                detail: String::from("curated run counts or audit path drifted"),
            });
        }

        for participant in &self.participants {
            match participant.role_class {
                DecentralizedNetworkRoleClass::PublicMiner
                | DecentralizedNetworkRoleClass::PublicValidator => {
                    if !readiness_ids.contains(participant.authority_reference_id.as_str()) {
                        return Err(CuratedDecentralizedRunContractError::InvalidContract {
                            detail: format!(
                                "participant `{}` lost readiness-decision binding",
                                participant.participant_id
                            ),
                        });
                    }
                }
                DecentralizedNetworkRoleClass::Relay
                | DecentralizedNetworkRoleClass::CheckpointAuthority => {
                    if !registry_ids.contains(participant.authority_reference_id.as_str()) {
                        return Err(CuratedDecentralizedRunContractError::InvalidContract {
                            detail: format!(
                                "participant `{}` lost registry binding",
                                participant.participant_id
                            ),
                        });
                    }
                }
                _ => {
                    return Err(CuratedDecentralizedRunContractError::InvalidContract {
                        detail: format!(
                            "participant `{}` used unsupported role class",
                            participant.participant_id
                        ),
                    });
                }
            }
        }

        if self.evidence_bundle.explorer_snapshot_id != explorer.snapshot.snapshot_id
            || self.evidence_bundle.settlement_record_id
                != settlement.settlement_records[0].record_id
            || self.outcome.participation_mode != CuratedRunParticipationMode::CuratedPermissioned
            || self.outcome.promotion_status != CuratedRunPromotionStatus::HeldNoPromotion
            || self.outcome.start_epoch_id >= self.outcome.end_epoch_id
        {
            return Err(CuratedDecentralizedRunContractError::InvalidContract {
                detail: String::from("curated run outcome drifted"),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(CuratedDecentralizedRunContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

pub fn canonical_curated_decentralized_run_contract(
) -> Result<CuratedDecentralizedRunContract, CuratedDecentralizedRunContractError> {
    let network = canonical_decentralized_network_contract()?;
    let readiness = canonical_public_testnet_readiness_contract()?;
    let explorer = canonical_public_run_explorer_contract()?;
    let settlement = canonical_settlement_publication_contract()?;

    let participants = vec![
        CuratedRunParticipant {
            participant_id: String::from("participant.google.public_miner"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            authority_reference_id: String::from("decision.google.reward_eligible"),
            runtime_reference_id: String::from("candidate.public_miner.google"),
            detail: String::from("Google participates as the curated public miner."),
        },
        CuratedRunParticipant {
            participant_id: String::from("participant.local_mlx.public_validator"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            authority_reference_id: String::from("decision.local_mlx.reward_eligible"),
            runtime_reference_id: String::from("candidate.public_validator.local_mlx"),
            detail: String::from("Apple MLX participates as the curated public validator."),
        },
        CuratedRunParticipant {
            participant_id: String::from("participant.google.relay"),
            role_class: DecentralizedNetworkRoleClass::Relay,
            authority_reference_id: String::from("google_l4_validator_node.registry"),
            runtime_reference_id: String::from("pane.node_status"),
            detail: String::from("Google carries the relay role for the curated run."),
        },
        CuratedRunParticipant {
            participant_id: String::from("participant.runpod.checkpoint_authority"),
            role_class: DecentralizedNetworkRoleClass::CheckpointAuthority,
            authority_reference_id: String::from("runpod_8xh100_dense_node.registry"),
            runtime_reference_id: String::from("settlement.window1231.signed"),
            detail: String::from("RunPod carries the mirrored checkpoint-authority role."),
        },
    ];

    let mut contract = CuratedDecentralizedRunContract {
        schema_version: String::from(CURATED_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(CURATED_DECENTRALIZED_RUN_CONTRACT_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        public_testnet_readiness_contract_digest: readiness.contract_digest.clone(),
        public_run_explorer_contract_digest: explorer.contract_digest.clone(),
        settlement_publication_contract_digest: settlement.contract_digest.clone(),
        participants,
        evidence_bundle: CuratedRunEvidenceBundle {
            bundle_id: String::from("bundle.curated_decentralized_run.window1231"),
            explorer_snapshot_id: explorer.snapshot.snapshot_id.clone(),
            settlement_record_id: settlement.settlement_records[0].record_id.clone(),
            retained_paths: vec![
                String::from("artifacts/curated-run/window1231/network-contract.json"),
                String::from("artifacts/curated-run/window1231/explorer-snapshot.json"),
                String::from("artifacts/curated-run/window1231/settlement-record.json"),
                String::from("artifacts/curated-run/window1231/event-log.ndjson"),
            ],
            after_action_audit_path: String::from(CURATED_DECENTRALIZED_RUN_AUDIT_DOC_PATH),
            detail: String::from("The first curated run retains a full evidence bundle and after-action audit."),
        },
        outcome: CuratedRunOutcome {
            outcome_id: String::from("outcome.curated_decentralized_run.window1231"),
            start_epoch_id: String::from("window1231"),
            end_epoch_id: String::from("window1232"),
            participation_mode: CuratedRunParticipationMode::CuratedPermissioned,
            promotion_status: CuratedRunPromotionStatus::HeldNoPromotion,
            detail: String::from("The curated run closes truthfully with retained evidence and held-no-promotion."),
        },
        authority_paths: CuratedDecentralizedRunAuthorityPaths {
            fixture_path: String::from(CURATED_DECENTRALIZED_RUN_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(CURATED_DECENTRALIZED_RUN_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(CURATED_DECENTRALIZED_RUN_CONTRACT_DOC_PATH),
            audit_doc_path: String::from(CURATED_DECENTRALIZED_RUN_AUDIT_DOC_PATH),
            train_system_doc_path: String::from(CURATED_DECENTRALIZED_RUN_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first curated internet-scale decentralized run: permissioned participants, one retained evidence bundle, and one after-action audit. It does not yet claim outside-operator admission or canary payouts.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_curated_decentralized_run_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), CuratedDecentralizedRunContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CuratedDecentralizedRunContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_curated_decentralized_run_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| CuratedDecentralizedRunContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for curated decentralized run contract",
        ),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_curated_decentralized_run_contract, CuratedDecentralizedRunContractError,
    };

    #[test]
    fn canonical_curated_decentralized_run_contract_is_valid(
    ) -> Result<(), CuratedDecentralizedRunContractError> {
        let contract = canonical_curated_decentralized_run_contract()?;
        contract.validate()
    }
}
