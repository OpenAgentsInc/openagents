use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_content_addressed_artifact_exchange_contract,
    canonical_decentralized_network_contract, canonical_shared_validator_promotion_contract,
    canonical_validator_challenge_scoring_contract, ContentAddressedArtifactExchangeContractError,
    ContentAddressedArtifactKind, DecentralizedNetworkContractError,
    SharedValidatorPromotionContractError, TrainingExecutionPromotionOutcome,
    TrainingExecutionValidatorDisposition, ValidatorChallengeScoringContractError,
};

pub const MULTI_VALIDATOR_CONSENSUS_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.multi_validator_consensus_contract.v1";
pub const MULTI_VALIDATOR_CONSENSUS_CONTRACT_ID: &str =
    "psionic.multi_validator_consensus_contract.v1";
pub const MULTI_VALIDATOR_CONSENSUS_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/multi_validator_consensus_contract_v1.json";
pub const MULTI_VALIDATOR_CONSENSUS_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-multi-validator-consensus-contract.sh";
pub const MULTI_VALIDATOR_CONSENSUS_CONTRACT_DOC_PATH: &str =
    "docs/MULTI_VALIDATOR_CONSENSUS_REFERENCE.md";
pub const MULTI_VALIDATOR_CONSENSUS_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum MultiValidatorConsensusContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    ArtifactExchange(#[from] ContentAddressedArtifactExchangeContractError),
    #[error(transparent)]
    ValidatorScoring(#[from] ValidatorChallengeScoringContractError),
    #[error(transparent)]
    SharedValidatorPromotion(#[from] SharedValidatorPromotionContractError),
    #[error("multi-validator consensus contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiValidatorCandidateKind {
    CheckpointArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiValidatorConsensusPolicy {
    pub minimum_validator_quorum: u16,
    pub require_unanimous_acceptance_for_promotion: bool,
    pub replay_required_blocks_promotion: bool,
    pub disagreement_holds_promotion: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiValidatorConsensusVote {
    pub vote_id: String,
    pub validator_registry_record_id: String,
    pub score_receipt_id: String,
    pub challenged_miner_session_id: String,
    pub disposition: TrainingExecutionValidatorDisposition,
    pub weight_bps: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiValidatorPromotionDecision {
    pub decision_id: String,
    pub candidate_kind: MultiValidatorCandidateKind,
    pub candidate_reference_id: String,
    pub vote_ids: Vec<String>,
    pub accepted_vote_count: u16,
    pub replay_required_vote_count: u16,
    pub outcome: TrainingExecutionPromotionOutcome,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiValidatorDisagreementReceipt {
    pub receipt_id: String,
    pub decision_id: String,
    pub disagreeing_validator_registry_record_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiValidatorConsensusAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiValidatorConsensusContract {
    pub schema_version: String,
    pub contract_id: String,
    pub decentralized_network_contract_digest: String,
    pub validator_challenge_scoring_contract_digest: String,
    pub shared_validator_promotion_contract_digest: String,
    pub content_addressed_artifact_exchange_contract_digest: String,
    pub consensus_policy: MultiValidatorConsensusPolicy,
    pub votes: Vec<MultiValidatorConsensusVote>,
    pub promotion_decisions: Vec<MultiValidatorPromotionDecision>,
    pub disagreement_receipts: Vec<MultiValidatorDisagreementReceipt>,
    pub authority_paths: MultiValidatorConsensusAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl MultiValidatorConsensusContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_multi_validator_consensus_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), MultiValidatorConsensusContractError> {
        let network = canonical_decentralized_network_contract()?;
        let scoring = canonical_validator_challenge_scoring_contract()?;
        let shared_validator = canonical_shared_validator_promotion_contract()?;
        let artifact_exchange = canonical_content_addressed_artifact_exchange_contract()?;

        let score_receipt_by_id = scoring
            .score_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let artifact_by_id = artifact_exchange
            .published_artifacts
            .iter()
            .map(|artifact| (artifact.artifact_id.as_str(), artifact))
            .collect::<BTreeMap<_, _>>();
        let vote_by_id = self
            .votes
            .iter()
            .map(|vote| (vote.vote_id.as_str(), vote))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != MULTI_VALIDATOR_CONSENSUS_CONTRACT_SCHEMA_VERSION {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    MULTI_VALIDATOR_CONSENSUS_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != MULTI_VALIDATOR_CONSENSUS_CONTRACT_ID {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest
            || self.validator_challenge_scoring_contract_digest != scoring.contract_digest
            || self.shared_validator_promotion_contract_digest != shared_validator.contract_digest
            || self.content_addressed_artifact_exchange_contract_digest
                != artifact_exchange.contract_digest
        {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.consensus_policy.minimum_validator_quorum
            != network.governance_revision.minimum_validator_quorum
            || !self
                .consensus_policy
                .require_unanimous_acceptance_for_promotion
            || !self.consensus_policy.replay_required_blocks_promotion
            || !self.consensus_policy.disagreement_holds_promotion
        {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("consensus policy drifted"),
            });
        }
        if self.authority_paths.fixture_path != MULTI_VALIDATOR_CONSENSUS_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != MULTI_VALIDATOR_CONSENSUS_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path
                != MULTI_VALIDATOR_CONSENSUS_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != MULTI_VALIDATOR_CONSENSUS_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let mut vote_ids = BTreeSet::new();
        let mut total_weight = 0_u16;
        for vote in &self.votes {
            if !vote_ids.insert(vote.vote_id.as_str()) {
                return Err(MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!("duplicate vote `{}`", vote.vote_id),
                });
            }
            let receipt = score_receipt_by_id
                .get(vote.score_receipt_id.as_str())
                .ok_or_else(|| MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!(
                        "vote `{}` references unknown score receipt `{}`",
                        vote.vote_id, vote.score_receipt_id
                    ),
                })?;
            if vote.validator_registry_record_id != receipt.validator_registry_record_id
                || vote.challenged_miner_session_id != receipt.challenged_miner_session_id
                || vote.disposition != receipt.disposition
                || vote.weight_bps == 0
            {
                return Err(MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!("vote `{}` drifted", vote.vote_id),
                });
            }
            total_weight = total_weight.saturating_add(vote.weight_bps);
        }
        if total_weight != 10_000 {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("vote weights must total 10_000 basis points"),
            });
        }

        if self.promotion_decisions.len() != 1 {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("expected exactly one promotion decision"),
            });
        }
        for decision in &self.promotion_decisions {
            let candidate = artifact_by_id
                .get(decision.candidate_reference_id.as_str())
                .ok_or_else(|| MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!(
                        "decision `{}` references unknown candidate artifact `{}`",
                        decision.decision_id, decision.candidate_reference_id
                    ),
                })?;
            if candidate.artifact_kind != ContentAddressedArtifactKind::LiveCheckpoint
                || decision.candidate_kind != MultiValidatorCandidateKind::CheckpointArtifact
                || decision.vote_ids.len()
                    < usize::from(self.consensus_policy.minimum_validator_quorum)
                || decision.accepted_vote_count != 1
                || decision.replay_required_vote_count != 1
                || decision.outcome != TrainingExecutionPromotionOutcome::HeldNoPromotion
            {
                return Err(MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!("promotion decision `{}` drifted", decision.decision_id),
                });
            }
            for vote_id in &decision.vote_ids {
                let vote = vote_by_id.get(vote_id.as_str()).ok_or_else(|| {
                    MultiValidatorConsensusContractError::InvalidContract {
                        detail: format!(
                            "decision `{}` references unknown vote `{}`",
                            decision.decision_id, vote_id
                        ),
                    }
                })?;
                if vote.disposition == TrainingExecutionValidatorDisposition::ReplayRequired
                    && !self.consensus_policy.replay_required_blocks_promotion
                {
                    return Err(MultiValidatorConsensusContractError::InvalidContract {
                        detail: format!(
                            "decision `{}` lost replay-required promotion hold",
                            decision.decision_id
                        ),
                    });
                }
            }
        }

        if self.disagreement_receipts.len() != 1 {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("expected exactly one disagreement receipt"),
            });
        }
        for receipt in &self.disagreement_receipts {
            let decision = self
                .promotion_decisions
                .iter()
                .find(|decision| decision.decision_id == receipt.decision_id)
                .ok_or_else(|| MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!(
                        "disagreement receipt `{}` references unknown decision `{}`",
                        receipt.receipt_id, receipt.decision_id
                    ),
                })?;
            if decision.outcome != TrainingExecutionPromotionOutcome::HeldNoPromotion
                || receipt.disagreeing_validator_registry_record_ids.len() != 2
            {
                return Err(MultiValidatorConsensusContractError::InvalidContract {
                    detail: format!("disagreement receipt `{}` drifted", receipt.receipt_id),
                });
            }
        }

        if !shared_validator
            .admitted_promotion_outcomes
            .contains(&TrainingExecutionPromotionOutcome::HeldNoPromotion)
        {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("shared validator promotion contract lost held-no-promotion"),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(MultiValidatorConsensusContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_multi_validator_consensus_contract(
) -> Result<MultiValidatorConsensusContract, MultiValidatorConsensusContractError> {
    let network = canonical_decentralized_network_contract()?;
    let scoring = canonical_validator_challenge_scoring_contract()?;
    let shared_validator = canonical_shared_validator_promotion_contract()?;
    let artifact_exchange = canonical_content_addressed_artifact_exchange_contract()?;

    let votes = vec![
        vote(
            "vote.public_validator.google.local_mlx.window1231",
            "google_l4_validator_node.registry",
            "score.public_validator.google.local_mlx.window1231",
            "session.public_miner.local_mlx.window1231",
            TrainingExecutionValidatorDisposition::Accepted,
            5_000,
            "Google contributes one accepted validator vote for the current checkpoint candidate after replaying the Apple MLX miner session.",
        ),
        vote(
            "vote.public_validator.local_mlx.google.window1231",
            "local_mlx_mac_workstation.registry",
            "score.public_validator.local_mlx.google.window1231",
            "session.public_miner.google.window1231",
            TrainingExecutionValidatorDisposition::ReplayRequired,
            5_000,
            "Apple MLX contributes one replay-required vote for the same checkpoint candidate after seeing excessive replay error on the Google miner session.",
        ),
    ];

    let promotion_decisions = vec![MultiValidatorPromotionDecision {
        decision_id: String::from("decision.checkpoint.step2048.round2056"),
        candidate_kind: MultiValidatorCandidateKind::CheckpointArtifact,
        candidate_reference_id: String::from("artifact.checkpoint.step2048.live"),
        vote_ids: vec![
            String::from("vote.public_validator.google.local_mlx.window1231"),
            String::from("vote.public_validator.local_mlx.google.window1231"),
        ],
        accepted_vote_count: 1,
        replay_required_vote_count: 1,
        outcome: TrainingExecutionPromotionOutcome::HeldNoPromotion,
        detail: String::from(
            "The current checkpoint candidate is held rather than promoted because the network reached quorum but not unanimous acceptance: one validator accepted while the other still requires replay.",
        ),
    }];

    let disagreement_receipts = vec![MultiValidatorDisagreementReceipt {
        receipt_id: String::from("disagreement.checkpoint.step2048.round2056"),
        decision_id: String::from("decision.checkpoint.step2048.round2056"),
        disagreeing_validator_registry_record_ids: vec![
            String::from("google_l4_validator_node.registry"),
            String::from("local_mlx_mac_workstation.registry"),
        ],
        detail: String::from(
            "The two active validators disagree on whether the current checkpoint candidate is ready, so the network records one explicit held-no-promotion disagreement receipt instead of forcing silent human arbitration.",
        ),
    }];

    let mut contract = MultiValidatorConsensusContract {
        schema_version: String::from(MULTI_VALIDATOR_CONSENSUS_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(MULTI_VALIDATOR_CONSENSUS_CONTRACT_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        validator_challenge_scoring_contract_digest: scoring.contract_digest.clone(),
        shared_validator_promotion_contract_digest: shared_validator.contract_digest.clone(),
        content_addressed_artifact_exchange_contract_digest: artifact_exchange.contract_digest.clone(),
        consensus_policy: MultiValidatorConsensusPolicy {
            minimum_validator_quorum: network.governance_revision.minimum_validator_quorum,
            require_unanimous_acceptance_for_promotion: true,
            replay_required_blocks_promotion: true,
            disagreement_holds_promotion: true,
            detail: String::from(
                "Checkpoint promotion requires the network quorum declared by the decentralized network contract plus unanimous acceptance from that quorum; replay-required or disagreement holds the candidate rather than promoting it.",
            ),
        },
        votes,
        promotion_decisions,
        disagreement_receipts,
        authority_paths: MultiValidatorConsensusAuthorityPaths {
            fixture_path: String::from(MULTI_VALIDATOR_CONSENSUS_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(MULTI_VALIDATOR_CONSENSUS_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(MULTI_VALIDATOR_CONSENSUS_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                MULTI_VALIDATOR_CONSENSUS_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first multi-validator checkpoint-promotion surface: quorum policy, weighted validator votes, held-no-promotion decisions, and explicit disagreement receipts. It does not yet claim fraud penalties, slashing, or reward accounting.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_multi_validator_consensus_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), MultiValidatorConsensusContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            MultiValidatorConsensusContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_multi_validator_consensus_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| MultiValidatorConsensusContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn vote(
    vote_id: &str,
    validator_registry_record_id: &str,
    score_receipt_id: &str,
    challenged_miner_session_id: &str,
    disposition: TrainingExecutionValidatorDisposition,
    weight_bps: u16,
    detail: &str,
) -> MultiValidatorConsensusVote {
    MultiValidatorConsensusVote {
        vote_id: String::from(vote_id),
        validator_registry_record_id: String::from(validator_registry_record_id),
        score_receipt_id: String::from(score_receipt_id),
        challenged_miner_session_id: String::from(challenged_miner_session_id),
        disposition,
        weight_bps,
        detail: String::from(detail),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for multi-validator consensus contract",
        ),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_multi_validator_consensus_contract, MultiValidatorConsensusContractError,
        TrainingExecutionPromotionOutcome,
    };

    #[test]
    fn canonical_multi_validator_consensus_contract_is_valid(
    ) -> Result<(), MultiValidatorConsensusContractError> {
        let contract = canonical_multi_validator_consensus_contract()?;
        contract.validate()
    }

    #[test]
    fn disagreement_cannot_silently_promote_checkpoint(
    ) -> Result<(), MultiValidatorConsensusContractError> {
        let mut contract = canonical_multi_validator_consensus_contract()?;
        let decision = contract
            .promotion_decisions
            .iter_mut()
            .find(|decision| decision.decision_id == "decision.checkpoint.step2048.round2056")
            .expect("canonical contract should retain the checkpoint decision");
        decision.outcome = TrainingExecutionPromotionOutcome::PromotedRevision;
        let error = contract
            .validate()
            .expect_err("disagreement cannot silently promote the checkpoint");
        assert!(matches!(
            error,
            MultiValidatorConsensusContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
