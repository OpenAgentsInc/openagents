use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_public_miner_protocol_contract, canonical_public_network_registry_contract,
    canonical_public_work_assignment_contract, canonical_shared_validator_promotion_contract,
    PublicMinerProtocolContractError, PublicNetworkRegistryContractError,
    PublicWorkAssignmentContractError, PublicWorkAssignmentKind,
    SharedValidatorPromotionContractError, TrainingExecutionValidatorDisposition,
};

pub const VALIDATOR_CHALLENGE_SCORING_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.validator_challenge_scoring_contract.v1";
pub const VALIDATOR_CHALLENGE_SCORING_CONTRACT_ID: &str =
    "psionic.validator_challenge_scoring_contract.v1";
pub const VALIDATOR_CHALLENGE_SCORING_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/validator_challenge_scoring_contract_v1.json";
pub const VALIDATOR_CHALLENGE_SCORING_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-validator-challenge-scoring-contract.sh";
pub const VALIDATOR_CHALLENGE_SCORING_CONTRACT_DOC_PATH: &str =
    "docs/VALIDATOR_CHALLENGE_SCORING_REFERENCE.md";
pub const VALIDATOR_CHALLENGE_SCORING_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum ValidatorChallengeScoringContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    PublicWork(#[from] PublicWorkAssignmentContractError),
    #[error(transparent)]
    PublicMinerProtocol(#[from] PublicMinerProtocolContractError),
    #[error(transparent)]
    SharedValidatorPromotion(#[from] SharedValidatorPromotionContractError),
    #[error("validator challenge scoring contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorChallengeRefusalKind {
    StaleCheckpoint,
    MissingReplayArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorImprovementScoringPolicy {
    pub accepted_min_improvement_bps: i32,
    pub quarantine_min_improvement_bps: i32,
    pub rejected_max_improvement_bps: i32,
    pub replay_required_error_ppm: u32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeReplayRule {
    pub replay_rule_id: String,
    pub validator_assignment_id: String,
    pub challenged_miner_session_id: String,
    pub required_dataset_receipt_id: String,
    pub required_delta_artifact_id: String,
    pub required_checkpoint_reference_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeScoreReceipt {
    pub receipt_id: String,
    pub validator_registry_record_id: String,
    pub validator_assignment_id: String,
    pub challenged_miner_session_id: String,
    pub replay_rule_id: String,
    pub replay_error_ppm: u32,
    pub before_eval_loss_milli: u32,
    pub after_eval_loss_milli: u32,
    pub improvement_bps: i32,
    pub disposition: TrainingExecutionValidatorDisposition,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeRefusal {
    pub refusal_id: String,
    pub validator_registry_record_id: String,
    pub challenged_reference_id: String,
    pub refusal_kind: ValidatorChallengeRefusalKind,
    pub disposition: TrainingExecutionValidatorDisposition,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeScoringAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorChallengeScoringContract {
    pub schema_version: String,
    pub contract_id: String,
    pub public_network_registry_contract_digest: String,
    pub public_work_assignment_contract_digest: String,
    pub public_miner_protocol_contract_digest: String,
    pub shared_validator_promotion_contract_digest: String,
    pub scoring_policy: ValidatorImprovementScoringPolicy,
    pub replay_rules: Vec<ValidatorChallengeReplayRule>,
    pub score_receipts: Vec<ValidatorChallengeScoreReceipt>,
    pub refusals: Vec<ValidatorChallengeRefusal>,
    pub authority_paths: ValidatorChallengeScoringAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl ValidatorChallengeScoringContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_validator_challenge_scoring_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), ValidatorChallengeScoringContractError> {
        let registry = canonical_public_network_registry_contract()?;
        let public_work = canonical_public_work_assignment_contract()?;
        let miner_protocol = canonical_public_miner_protocol_contract()?;
        let shared_validator = canonical_shared_validator_promotion_contract()?;

        let registry_by_id = registry
            .registry_records
            .iter()
            .map(|record| (record.registry_record_id.as_str(), record))
            .collect::<BTreeMap<_, _>>();
        let validator_assignment_by_id = public_work
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.assignment_kind == PublicWorkAssignmentKind::PublicValidatorChallenge
            })
            .map(|assignment| (assignment.assignment_id.as_str(), assignment))
            .collect::<BTreeMap<_, _>>();
        let session_by_id = miner_protocol
            .sessions
            .iter()
            .map(|session| (session.session_id.as_str(), session))
            .collect::<BTreeMap<_, _>>();
        let refusal_ids = miner_protocol
            .refusals
            .iter()
            .map(|refusal| refusal.refusal_id.as_str())
            .collect::<BTreeSet<_>>();
        let replay_rule_by_id = self
            .replay_rules
            .iter()
            .map(|rule| (rule.replay_rule_id.as_str(), rule))
            .collect::<BTreeMap<_, _>>();

        if self.schema_version != VALIDATOR_CHALLENGE_SCORING_CONTRACT_SCHEMA_VERSION {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    VALIDATOR_CHALLENGE_SCORING_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != VALIDATOR_CHALLENGE_SCORING_CONTRACT_ID {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.public_network_registry_contract_digest != registry.contract_digest
            || self.public_work_assignment_contract_digest != public_work.contract_digest
            || self.public_miner_protocol_contract_digest != miner_protocol.contract_digest
            || self.shared_validator_promotion_contract_digest != shared_validator.contract_digest
        {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.scoring_policy.accepted_min_improvement_bps
            <= self.scoring_policy.quarantine_min_improvement_bps
            || self.scoring_policy.quarantine_min_improvement_bps
                <= self.scoring_policy.rejected_max_improvement_bps
            || self.scoring_policy.replay_required_error_ppm == 0
        {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: String::from("scoring policy drifted"),
            });
        }
        if self.authority_paths.fixture_path != VALIDATOR_CHALLENGE_SCORING_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != VALIDATOR_CHALLENGE_SCORING_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path
                != VALIDATOR_CHALLENGE_SCORING_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != VALIDATOR_CHALLENGE_SCORING_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let mut replay_rule_ids = BTreeSet::new();
        for rule in &self.replay_rules {
            if !replay_rule_ids.insert(rule.replay_rule_id.as_str()) {
                return Err(ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!("duplicate replay rule `{}`", rule.replay_rule_id),
                });
            }
            let validator_assignment = validator_assignment_by_id
                .get(rule.validator_assignment_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "replay rule `{}` references unknown validator assignment `{}`",
                        rule.replay_rule_id, rule.validator_assignment_id
                    ),
                })?;
            let challenged_session = session_by_id
                .get(rule.challenged_miner_session_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "replay rule `{}` references unknown miner session `{}`",
                        rule.replay_rule_id, rule.challenged_miner_session_id
                    ),
                })?;
            if validator_assignment.registry_record_id
                == challenged_session.miner_registry_record_id
                || validator_assignment.challenged_assignment_id.as_deref()
                    != Some(challenged_session.assignment_id.as_str())
                || rule.required_dataset_receipt_id != challenged_session.dataset_receipt_id
                || rule.required_delta_artifact_id != challenged_session.delta_artifact_id
                || rule.required_checkpoint_reference_id
                    != challenged_session.checkpoint_reference_id
            {
                return Err(ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!("replay rule `{}` drifted", rule.replay_rule_id),
                });
            }
        }

        let mut receipt_ids = BTreeSet::new();
        for receipt in &self.score_receipts {
            if !receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!("duplicate score receipt `{}`", receipt.receipt_id),
                });
            }
            let registry_record = registry_by_id
                .get(receipt.validator_registry_record_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "score receipt `{}` references unknown validator `{}`",
                        receipt.receipt_id, receipt.validator_registry_record_id
                    ),
                })?;
            let validator_assignment = validator_assignment_by_id
                .get(receipt.validator_assignment_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "score receipt `{}` references unknown validator assignment `{}`",
                        receipt.receipt_id, receipt.validator_assignment_id
                    ),
                })?;
            let challenged_session = session_by_id
                .get(receipt.challenged_miner_session_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "score receipt `{}` references unknown miner session `{}`",
                        receipt.receipt_id, receipt.challenged_miner_session_id
                    ),
                })?;
            let replay_rule = replay_rule_by_id
                .get(receipt.replay_rule_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "score receipt `{}` references unknown replay rule `{}`",
                        receipt.receipt_id, receipt.replay_rule_id
                    ),
                })?;
            if !registry_record
                .role_classes
                .contains(&crate::DecentralizedNetworkRoleClass::PublicValidator)
                || validator_assignment.registry_record_id != receipt.validator_registry_record_id
                || replay_rule.validator_assignment_id != receipt.validator_assignment_id
                || replay_rule.challenged_miner_session_id != receipt.challenged_miner_session_id
                || validator_assignment.challenged_assignment_id.as_deref()
                    != Some(challenged_session.assignment_id.as_str())
                || !shared_validator
                    .admitted_validator_dispositions
                    .contains(&receipt.disposition)
                || receipt.improvement_bps
                    != compute_improvement_bps(
                        receipt.before_eval_loss_milli,
                        receipt.after_eval_loss_milli,
                    )
            {
                return Err(ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!("score receipt `{}` drifted", receipt.receipt_id),
                });
            }
            if receipt.replay_error_ppm > self.scoring_policy.replay_required_error_ppm {
                if receipt.disposition != TrainingExecutionValidatorDisposition::ReplayRequired {
                    return Err(ValidatorChallengeScoringContractError::InvalidContract {
                        detail: format!(
                            "score receipt `{}` must stay replay_required once replay error exceeds the policy ceiling",
                            receipt.receipt_id
                        ),
                    });
                }
            } else if receipt.improvement_bps >= self.scoring_policy.accepted_min_improvement_bps {
                if receipt.disposition != TrainingExecutionValidatorDisposition::Accepted {
                    return Err(ValidatorChallengeScoringContractError::InvalidContract {
                        detail: format!(
                            "score receipt `{}` must stay accepted at the current policy threshold",
                            receipt.receipt_id
                        ),
                    });
                }
            } else if receipt.improvement_bps <= self.scoring_policy.rejected_max_improvement_bps {
                if receipt.disposition != TrainingExecutionValidatorDisposition::Rejected {
                    return Err(ValidatorChallengeScoringContractError::InvalidContract {
                        detail: format!(
                            "score receipt `{}` must stay rejected at the current policy threshold",
                            receipt.receipt_id
                        ),
                    });
                }
            } else if receipt.disposition != TrainingExecutionValidatorDisposition::Quarantined {
                return Err(ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "score receipt `{}` must stay quarantined inside the middle improvement band",
                        receipt.receipt_id
                    ),
                });
            }
        }

        if self.refusals.len() != 1 {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: String::from("expected exactly one validator refusal"),
            });
        }
        for refusal in &self.refusals {
            let registry_record = registry_by_id
                .get(refusal.validator_registry_record_id.as_str())
                .ok_or_else(|| ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!(
                        "refusal `{}` references unknown validator `{}`",
                        refusal.refusal_id, refusal.validator_registry_record_id
                    ),
                })?;
            if !registry_record
                .role_classes
                .contains(&crate::DecentralizedNetworkRoleClass::PublicValidator)
                || !refusal_ids.contains(refusal.challenged_reference_id.as_str())
                || refusal.refusal_kind != ValidatorChallengeRefusalKind::StaleCheckpoint
                || refusal.disposition != TrainingExecutionValidatorDisposition::ReplayRequired
            {
                return Err(ValidatorChallengeScoringContractError::InvalidContract {
                    detail: format!("refusal `{}` drifted", refusal.refusal_id),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(ValidatorChallengeScoringContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_validator_challenge_scoring_contract(
) -> Result<ValidatorChallengeScoringContract, ValidatorChallengeScoringContractError> {
    let registry = canonical_public_network_registry_contract()?;
    let public_work = canonical_public_work_assignment_contract()?;
    let miner_protocol = canonical_public_miner_protocol_contract()?;
    let shared_validator = canonical_shared_validator_promotion_contract()?;

    let replay_rules = vec![
        replay_rule(
            "rule.public_validator.google.local_mlx.window1231",
            "assignment.public_validator.window1231.google",
            "session.public_miner.local_mlx.window1231",
            "anti_replay.assignment.public_miner.window1231.local_mlx",
            "artifact.delta.local_mlx.nf4.round2056",
            "catchup.public_miner.local_mlx.after_deathrattle",
            "Google validates the rejoined Apple MLX miner session against the exact dataset receipt, delta artifact, and live catch-up reference admitted by the miner protocol.",
        ),
        replay_rule(
            "rule.public_validator.local_mlx.google.window1231",
            "assignment.public_validator.window1231.local_mlx",
            "session.public_miner.google.window1231",
            "anti_replay.assignment.public_miner.window1231.google",
            "artifact.delta.google.int8.round2056",
            "advertisement.checkpoint_authority.google.mirror",
            "Apple MLX validates the Google miner session against the exact dataset receipt, delta artifact, and mirrored checkpoint reference admitted by the miner protocol.",
        ),
    ];

    let score_receipts = vec![
        score_receipt(
            "score.public_validator.google.local_mlx.window1231",
            "google_l4_validator_node.registry",
            "assignment.public_validator.window1231.google",
            "session.public_miner.local_mlx.window1231",
            "rule.public_validator.google.local_mlx.window1231",
            18,
            10_000,
            9_980,
            TrainingExecutionValidatorDisposition::Accepted,
            "Google replays the Apple MLX miner delta against the admitted checkpoint and challenge slice, measures a twenty-basis-point improvement, and accepts the contribution.",
        ),
        score_receipt(
            "score.public_validator.local_mlx.google.window1231",
            "local_mlx_mac_workstation.registry",
            "assignment.public_validator.window1231.local_mlx",
            "session.public_miner.google.window1231",
            "rule.public_validator.local_mlx.google.window1231",
            1_200,
            10_000,
            9_998,
            TrainingExecutionValidatorDisposition::ReplayRequired,
            "Apple MLX sees an excessive replay error while validating the Google miner delta, so the contribution stays replay-required instead of being softly accepted.",
        ),
    ];

    let refusals = vec![ValidatorChallengeRefusal {
        refusal_id: String::from("refusal.public_validator.google.local_rtx4080.stale"),
        validator_registry_record_id: String::from("google_l4_validator_node.registry"),
        challenged_reference_id: String::from("refusal.public_miner.local_rtx4080.checkpoint_lag"),
        refusal_kind: ValidatorChallengeRefusalKind::StaleCheckpoint,
        disposition: TrainingExecutionValidatorDisposition::ReplayRequired,
        detail: String::from(
            "Google refuses to score the stale RTX 4080 standby miner because the miner protocol already failed the submission on checkpoint-lag grounds, so replay remains the only truthful verdict posture.",
        ),
    }];

    let mut contract = ValidatorChallengeScoringContract {
        schema_version: String::from(VALIDATOR_CHALLENGE_SCORING_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(VALIDATOR_CHALLENGE_SCORING_CONTRACT_ID),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        public_work_assignment_contract_digest: public_work.contract_digest.clone(),
        public_miner_protocol_contract_digest: miner_protocol.contract_digest.clone(),
        shared_validator_promotion_contract_digest: shared_validator.contract_digest.clone(),
        scoring_policy: ValidatorImprovementScoringPolicy {
            accepted_min_improvement_bps: 10,
            quarantine_min_improvement_bps: -5,
            rejected_max_improvement_bps: -25,
            replay_required_error_ppm: 100,
            detail: String::from(
                "Public validators accept clear positive improvement below the replay-error ceiling, quarantine marginal or ambiguous results, reject clearly regressive outputs, and require replay once replay-error exceeds the admitted ceiling.",
            ),
        },
        replay_rules,
        score_receipts,
        refusals,
        authority_paths: ValidatorChallengeScoringAuthorityPaths {
            fixture_path: String::from(VALIDATOR_CHALLENGE_SCORING_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(VALIDATOR_CHALLENGE_SCORING_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(VALIDATOR_CHALLENGE_SCORING_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                VALIDATOR_CHALLENGE_SCORING_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first public validator challenge and scoring surface: replay rules, improvement thresholds, typed score receipts, and one stale-checkpoint refusal. It does not yet claim multi-validator consensus, checkpoint promotion, or fraud penalties.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_validator_challenge_scoring_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), ValidatorChallengeScoringContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ValidatorChallengeScoringContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_validator_challenge_scoring_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| ValidatorChallengeScoringContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn replay_rule(
    replay_rule_id: &str,
    validator_assignment_id: &str,
    challenged_miner_session_id: &str,
    required_dataset_receipt_id: &str,
    required_delta_artifact_id: &str,
    required_checkpoint_reference_id: &str,
    detail: &str,
) -> ValidatorChallengeReplayRule {
    ValidatorChallengeReplayRule {
        replay_rule_id: String::from(replay_rule_id),
        validator_assignment_id: String::from(validator_assignment_id),
        challenged_miner_session_id: String::from(challenged_miner_session_id),
        required_dataset_receipt_id: String::from(required_dataset_receipt_id),
        required_delta_artifact_id: String::from(required_delta_artifact_id),
        required_checkpoint_reference_id: String::from(required_checkpoint_reference_id),
        detail: String::from(detail),
    }
}

fn score_receipt(
    receipt_id: &str,
    validator_registry_record_id: &str,
    validator_assignment_id: &str,
    challenged_miner_session_id: &str,
    replay_rule_id: &str,
    replay_error_ppm: u32,
    before_eval_loss_milli: u32,
    after_eval_loss_milli: u32,
    disposition: TrainingExecutionValidatorDisposition,
    detail: &str,
) -> ValidatorChallengeScoreReceipt {
    ValidatorChallengeScoreReceipt {
        receipt_id: String::from(receipt_id),
        validator_registry_record_id: String::from(validator_registry_record_id),
        validator_assignment_id: String::from(validator_assignment_id),
        challenged_miner_session_id: String::from(challenged_miner_session_id),
        replay_rule_id: String::from(replay_rule_id),
        replay_error_ppm,
        before_eval_loss_milli,
        after_eval_loss_milli,
        improvement_bps: compute_improvement_bps(before_eval_loss_milli, after_eval_loss_milli),
        disposition,
        detail: String::from(detail),
    }
}

fn compute_improvement_bps(before_eval_loss_milli: u32, after_eval_loss_milli: u32) -> i32 {
    let before = i64::from(before_eval_loss_milli);
    let after = i64::from(after_eval_loss_milli);
    (((before - after) * 10_000) / before.max(1)) as i32
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect(
        "stable digest serialization must succeed for validator challenge scoring contract",
    ));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_validator_challenge_scoring_contract, TrainingExecutionValidatorDisposition,
        ValidatorChallengeScoringContractError,
    };

    #[test]
    fn canonical_validator_challenge_scoring_contract_is_valid(
    ) -> Result<(), ValidatorChallengeScoringContractError> {
        let contract = canonical_validator_challenge_scoring_contract()?;
        contract.validate()
    }

    #[test]
    fn high_replay_error_must_stay_replay_required(
    ) -> Result<(), ValidatorChallengeScoringContractError> {
        let mut contract = canonical_validator_challenge_scoring_contract()?;
        let receipt = contract
            .score_receipts
            .iter_mut()
            .find(|receipt| {
                receipt.receipt_id == "score.public_validator.local_mlx.google.window1231"
            })
            .expect("canonical contract should retain the replay-required score receipt");
        receipt.disposition = TrainingExecutionValidatorDisposition::Accepted;
        let error = contract
            .validate()
            .expect_err("high replay error cannot silently become accepted");
        assert!(matches!(
            error,
            ValidatorChallengeScoringContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
