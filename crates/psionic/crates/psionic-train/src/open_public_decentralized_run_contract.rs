use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_curated_decentralized_run_contract, canonical_public_run_explorer_contract,
    canonical_public_testnet_readiness_contract, CuratedDecentralizedRunContractError,
    DecentralizedNetworkRoleClass, PublicRunExplorerContractError,
    PublicTestnetReadinessContractError, PublicTestnetTier,
};

pub const OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.open_public_decentralized_run_contract.v1";
pub const OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_ID: &str =
    "psionic.open_public_decentralized_run_contract.v1";
pub const OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/open_public_decentralized_run_contract_v1.json";
pub const OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-open-public-decentralized-run-contract.sh";
pub const OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_DOC_PATH: &str =
    "docs/OPEN_PUBLIC_DECENTRALIZED_RUN_REFERENCE.md";
pub const OPEN_PUBLIC_DECENTRALIZED_RUN_AUDIT_DOC_PATH: &str =
    "docs/audits/2026-03-26-open-public-miner-validator-run-audit.md";
pub const OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str =
    "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum OpenPublicDecentralizedRunContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Readiness(#[from] PublicTestnetReadinessContractError),
    #[error(transparent)]
    CuratedRun(#[from] CuratedDecentralizedRunContractError),
    #[error(transparent)]
    Explorer(#[from] PublicRunExplorerContractError),
    #[error("open public decentralized run contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenRunEventKind {
    CandidateAdmitted,
    ScorePublished,
    FraudBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRunParticipant {
    pub participant_id: String,
    pub candidate_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub admitted_tier: PublicTestnetTier,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRunEvent {
    pub event_id: String,
    pub event_kind: OpenRunEventKind,
    pub reference_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRunOutcome {
    pub outcome_id: String,
    pub outside_candidate_ids: Vec<String>,
    pub handled_attack_reference_ids: Vec<String>,
    pub audit_doc_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPublicDecentralizedRunAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub audit_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPublicDecentralizedRunContract {
    pub schema_version: String,
    pub contract_id: String,
    pub public_testnet_readiness_contract_digest: String,
    pub curated_decentralized_run_contract_digest: String,
    pub public_run_explorer_contract_digest: String,
    pub participants: Vec<OpenRunParticipant>,
    pub events: Vec<OpenRunEvent>,
    pub outcome: OpenRunOutcome,
    pub authority_paths: OpenPublicDecentralizedRunAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl OpenPublicDecentralizedRunContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_open_public_decentralized_run_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), OpenPublicDecentralizedRunContractError> {
        let readiness = canonical_public_testnet_readiness_contract()?;
        let curated = canonical_curated_decentralized_run_contract()?;
        let explorer = canonical_public_run_explorer_contract()?;

        let candidate_ids = readiness
            .candidates
            .iter()
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<BTreeSet<_>>();

        if self.schema_version != OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_ID {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.public_testnet_readiness_contract_digest != readiness.contract_digest
            || self.curated_decentralized_run_contract_digest != curated.contract_digest
            || self.public_run_explorer_contract_digest != explorer.contract_digest
        {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.participants.len() != 4
            || self.events.len() != 4
            || self.outcome.audit_doc_path != OPEN_PUBLIC_DECENTRALIZED_RUN_AUDIT_DOC_PATH
            || self.authority_paths.audit_doc_path != OPEN_PUBLIC_DECENTRALIZED_RUN_AUDIT_DOC_PATH
        {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: String::from("open-run counts or audit path drifted"),
            });
        }
        for participant in &self.participants {
            if !candidate_ids.contains(participant.candidate_id.as_str()) {
                return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                    detail: format!(
                        "participant `{}` references unknown candidate `{}`",
                        participant.participant_id, participant.candidate_id
                    ),
                });
            }
        }
        let event_refs = self
            .events
            .iter()
            .map(|event| event.reference_id.as_str())
            .collect::<BTreeSet<_>>();
        if !event_refs.contains("pane.scoreboard")
            || !event_refs.contains("decision.local_rtx4080.blocked")
        {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: String::from("open-run events lost explorer or fraud-block evidence"),
            });
        }
        if self.outcome.outside_candidate_ids.len() != 2
            || self.outcome.handled_attack_reference_ids
                != vec![String::from("decision.local_rtx4080.blocked")]
        {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: String::from("open-run outcome drifted"),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(OpenPublicDecentralizedRunContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

pub fn canonical_open_public_decentralized_run_contract(
) -> Result<OpenPublicDecentralizedRunContract, OpenPublicDecentralizedRunContractError> {
    let readiness = canonical_public_testnet_readiness_contract()?;
    let curated = canonical_curated_decentralized_run_contract()?;
    let explorer = canonical_public_run_explorer_contract()?;

    let participants = vec![
        OpenRunParticipant {
            participant_id: String::from("participant.google.public_miner"),
            candidate_id: String::from("candidate.public_miner.google"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            admitted_tier: PublicTestnetTier::RewardEligible,
            detail: String::from("Google stays in the open run as a reward-eligible anchor miner."),
        },
        OpenRunParticipant {
            participant_id: String::from("participant.local_mlx.public_validator"),
            candidate_id: String::from("candidate.public_validator.local_mlx"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            admitted_tier: PublicTestnetTier::RewardEligible,
            detail: String::from(
                "Apple MLX stays in the open run as a reward-eligible anchor validator.",
            ),
        },
        OpenRunParticipant {
            participant_id: String::from("participant.community_rtx4090.public_miner"),
            candidate_id: String::from("candidate.public_miner.community_rtx4090_east"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            admitted_tier: PublicTestnetTier::Canary,
            detail: String::from("One outside RTX 4090 operator joins as a canary miner."),
        },
        OpenRunParticipant {
            participant_id: String::from("participant.community_h100.public_validator"),
            candidate_id: String::from("candidate.public_validator.community_h100_central"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            admitted_tier: PublicTestnetTier::Canary,
            detail: String::from("One outside H100 operator joins as a canary validator."),
        },
    ];

    let events = vec![
        OpenRunEvent {
            event_id: String::from("event.community_rtx4090.admitted"),
            event_kind: OpenRunEventKind::CandidateAdmitted,
            reference_id: String::from("candidate.public_miner.community_rtx4090_east"),
            detail: String::from(
                "The open run admits the outside RTX 4090 miner through the canary gate.",
            ),
        },
        OpenRunEvent {
            event_id: String::from("event.community_h100.admitted"),
            event_kind: OpenRunEventKind::CandidateAdmitted,
            reference_id: String::from("candidate.public_validator.community_h100_central"),
            detail: String::from(
                "The open run admits the outside H100 validator through the canary gate.",
            ),
        },
        OpenRunEvent {
            event_id: String::from("event.scoreboard.published"),
            event_kind: OpenRunEventKind::ScorePublished,
            reference_id: String::from("pane.scoreboard"),
            detail: String::from(
                "The explorer scoreboard stays public during the open participation window.",
            ),
        },
        OpenRunEvent {
            event_id: String::from("event.rtx4080.blocked"),
            event_kind: OpenRunEventKind::FraudBlocked,
            reference_id: String::from("decision.local_rtx4080.blocked"),
            detail: String::from(
                "The blocked RTX 4080 candidate remains excluded during the open window.",
            ),
        },
    ];

    let mut contract = OpenPublicDecentralizedRunContract {
        schema_version: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_ID),
        public_testnet_readiness_contract_digest: readiness.contract_digest.clone(),
        curated_decentralized_run_contract_digest: curated.contract_digest.clone(),
        public_run_explorer_contract_digest: explorer.contract_digest.clone(),
        participants,
        events,
        outcome: OpenRunOutcome {
            outcome_id: String::from("outcome.open_public_run.window1232"),
            outside_candidate_ids: vec![
                String::from("candidate.public_miner.community_rtx4090_east"),
                String::from("candidate.public_validator.community_h100_central"),
            ],
            handled_attack_reference_ids: vec![String::from("decision.local_rtx4080.blocked")],
            audit_doc_path: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_AUDIT_DOC_PATH),
            detail: String::from("The first open public run retains outside-operator participation and blocked-fraud evidence."),
        },
        authority_paths: OpenPublicDecentralizedRunAuthorityPaths {
            fixture_path: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_DOC_PATH),
            audit_doc_path: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_AUDIT_DOC_PATH),
            train_system_doc_path: String::from(OPEN_PUBLIC_DECENTRALIZED_RUN_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first open public miner-validator participation window: outside canary candidates, public score visibility, and one retained blocked-fraud example. It does not yet claim canary payouts.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_open_public_decentralized_run_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), OpenPublicDecentralizedRunContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OpenPublicDecentralizedRunContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_open_public_decentralized_run_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| OpenPublicDecentralizedRunContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for open public run contract"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_open_public_decentralized_run_contract, OpenPublicDecentralizedRunContractError,
    };

    #[test]
    fn canonical_open_public_decentralized_run_contract_is_valid(
    ) -> Result<(), OpenPublicDecentralizedRunContractError> {
        let contract = canonical_open_public_decentralized_run_contract()?;
        contract.validate()
    }
}
