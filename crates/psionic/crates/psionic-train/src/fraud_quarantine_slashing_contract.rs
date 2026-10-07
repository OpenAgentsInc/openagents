use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_multi_validator_consensus_contract, canonical_public_dataset_authority_contract,
    canonical_signed_node_identity_contract_set, canonical_validator_challenge_scoring_contract,
    MultiValidatorConsensusContractError, PublicDatasetAuthorityContractError,
    SignedNodeIdentityContractSetError, TrainingExecutionValidatorDisposition,
    ValidatorChallengeScoringContractError,
};

pub const FRAUD_QUARANTINE_SLASHING_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.fraud_quarantine_slashing_contract.v1";
pub const FRAUD_QUARANTINE_SLASHING_CONTRACT_ID: &str =
    "psionic.fraud_quarantine_slashing_contract.v1";
pub const FRAUD_QUARANTINE_SLASHING_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/fraud_quarantine_slashing_contract_v1.json";
pub const FRAUD_QUARANTINE_SLASHING_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-fraud-quarantine-slashing-contract.sh";
pub const FRAUD_QUARANTINE_SLASHING_CONTRACT_DOC_PATH: &str =
    "docs/FRAUD_QUARANTINE_SLASHING_REFERENCE.md";
pub const FRAUD_QUARANTINE_SLASHING_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum FraudQuarantineSlashingContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    SignedNodeIdentity(#[from] SignedNodeIdentityContractSetError),
    #[error(transparent)]
    PublicDataset(#[from] PublicDatasetAuthorityContractError),
    #[error(transparent)]
    ValidatorScoring(#[from] ValidatorChallengeScoringContractError),
    #[error(transparent)]
    MultiValidatorConsensus(#[from] MultiValidatorConsensusContractError),
    #[error("fraud quarantine slashing contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FraudSignalKind {
    SybilWalletReuse,
    DatasetReplayAbuse,
    ValidatorDisagreementWatch,
    StaleSoftwareDigest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineClass {
    ObservationOnly,
    ContributionBlocked,
    ValidatorSuspended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppealStatus {
    Open,
    Denied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FraudSignal {
    pub signal_id: String,
    pub node_identity_id: String,
    pub evidence_reference_id: String,
    pub signal_kind: FraudSignalKind,
    pub severity_bps: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantineDecision {
    pub decision_id: String,
    pub node_identity_id: String,
    pub signal_ids: Vec<String>,
    pub quarantine_class: QuarantineClass,
    pub starts_epoch_id: String,
    pub ends_after_epoch_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashingDecision {
    pub decision_id: String,
    pub node_identity_id: String,
    pub supporting_signal_ids: Vec<String>,
    pub linked_quarantine_decision_id: String,
    pub penalty_bps: u16,
    pub settlement_hold_epochs: u16,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppealWindow {
    pub appeal_id: String,
    pub node_identity_id: String,
    pub challenged_action_id: String,
    pub status: AppealStatus,
    pub opens_epoch_id: String,
    pub closes_epoch_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FraudQuarantineSlashingAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FraudQuarantineSlashingContract {
    pub schema_version: String,
    pub contract_id: String,
    pub signed_node_identity_contract_set_digest: String,
    pub public_dataset_authority_contract_digest: String,
    pub validator_challenge_scoring_contract_digest: String,
    pub multi_validator_consensus_contract_digest: String,
    pub fraud_signals: Vec<FraudSignal>,
    pub quarantine_decisions: Vec<QuarantineDecision>,
    pub slashing_decisions: Vec<SlashingDecision>,
    pub appeal_windows: Vec<AppealWindow>,
    pub authority_paths: FraudQuarantineSlashingAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl FraudQuarantineSlashingContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_fraud_quarantine_slashing_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), FraudQuarantineSlashingContractError> {
        let identities = canonical_signed_node_identity_contract_set()?;
        let dataset = canonical_public_dataset_authority_contract()?;
        let scoring = canonical_validator_challenge_scoring_contract()?;
        let consensus = canonical_multi_validator_consensus_contract()?;

        let identity_by_id = identities
            .identities
            .iter()
            .map(|identity| (identity.node_identity_id.as_str(), identity))
            .collect::<BTreeMap<_, _>>();
        let dataset_receipt_ids = dataset
            .anti_replay_receipts
            .iter()
            .map(|receipt| receipt.receipt_id.as_str())
            .collect::<BTreeSet<_>>();
        let score_receipt_ids = scoring
            .score_receipts
            .iter()
            .map(|receipt| (receipt.receipt_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        let disagreement_ids = consensus
            .disagreement_receipts
            .iter()
            .map(|receipt| receipt.receipt_id.as_str())
            .collect::<BTreeSet<_>>();

        if self.schema_version != FRAUD_QUARANTINE_SLASHING_CONTRACT_SCHEMA_VERSION {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    FRAUD_QUARANTINE_SLASHING_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != FRAUD_QUARANTINE_SLASHING_CONTRACT_ID {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.signed_node_identity_contract_set_digest != identities.contract_digest
            || self.public_dataset_authority_contract_digest != dataset.contract_digest
            || self.validator_challenge_scoring_contract_digest != scoring.contract_digest
            || self.multi_validator_consensus_contract_digest != consensus.contract_digest
        {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != FRAUD_QUARANTINE_SLASHING_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != FRAUD_QUARANTINE_SLASHING_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path
                != FRAUD_QUARANTINE_SLASHING_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != FRAUD_QUARANTINE_SLASHING_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.fraud_signals.len() != 4
            || self.quarantine_decisions.len() != 2
            || self.slashing_decisions.len() != 1
            || self.appeal_windows.len() != 1
        {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: String::from(
                    "expected canonical counts for signals, quarantines, slashing, and appeals",
                ),
            });
        }

        let mut signal_ids = BTreeSet::new();
        for signal in &self.fraud_signals {
            if !signal_ids.insert(signal.signal_id.as_str()) {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!("duplicate fraud signal `{}`", signal.signal_id),
                });
            }
            let identity = identity_by_id
                .get(signal.node_identity_id.as_str())
                .ok_or_else(|| FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!(
                        "fraud signal `{}` references unknown node identity `{}`",
                        signal.signal_id, signal.node_identity_id
                    ),
                })?;
            if signal.severity_bps == 0 {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!("fraud signal `{}` lost severity", signal.signal_id),
                });
            }
            match signal.signal_kind {
                FraudSignalKind::SybilWalletReuse => {
                    if signal.evidence_reference_id
                        != format!("wallet_watch:{}", identity.wallet.wallet_address)
                    {
                        return Err(FraudQuarantineSlashingContractError::InvalidContract {
                            detail: format!(
                                "fraud signal `{}` lost wallet-watch binding",
                                signal.signal_id
                            ),
                        });
                    }
                }
                FraudSignalKind::DatasetReplayAbuse => {
                    if !dataset_receipt_ids.contains(signal.evidence_reference_id.as_str())
                        || signal.evidence_reference_id
                            != "anti_replay.assignment.public_miner.window1230.google.duplicate"
                    {
                        return Err(FraudQuarantineSlashingContractError::InvalidContract {
                            detail: format!(
                                "fraud signal `{}` lost duplicate replay evidence binding",
                                signal.signal_id
                            ),
                        });
                    }
                }
                FraudSignalKind::ValidatorDisagreementWatch => {
                    if !disagreement_ids.contains(signal.evidence_reference_id.as_str()) {
                        return Err(FraudQuarantineSlashingContractError::InvalidContract {
                            detail: format!(
                                "fraud signal `{}` lost disagreement evidence binding",
                                signal.signal_id
                            ),
                        });
                    }
                }
                FraudSignalKind::StaleSoftwareDigest => {
                    if signal.evidence_reference_id != identity.node_identity_id {
                        return Err(FraudQuarantineSlashingContractError::InvalidContract {
                            detail: format!(
                                "fraud signal `{}` lost self-attested stale software binding",
                                signal.signal_id
                            ),
                        });
                    }
                }
            }
        }

        let signal_by_id = self
            .fraud_signals
            .iter()
            .map(|signal| (signal.signal_id.as_str(), signal))
            .collect::<BTreeMap<_, _>>();
        let mut quarantine_ids = BTreeSet::new();
        for decision in &self.quarantine_decisions {
            if !quarantine_ids.insert(decision.decision_id.as_str()) {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!("duplicate quarantine decision `{}`", decision.decision_id),
                });
            }
            let identity = identity_by_id
                .get(decision.node_identity_id.as_str())
                .ok_or_else(|| FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!(
                        "quarantine decision `{}` references unknown node identity `{}`",
                        decision.decision_id, decision.node_identity_id
                    ),
                })?;
            if decision.signal_ids.is_empty()
                || decision.starts_epoch_id >= decision.ends_after_epoch_id
            {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!("quarantine decision `{}` drifted", decision.decision_id),
                });
            }
            for signal_id in &decision.signal_ids {
                let signal = signal_by_id.get(signal_id.as_str()).ok_or_else(|| {
                    FraudQuarantineSlashingContractError::InvalidContract {
                        detail: format!(
                            "quarantine decision `{}` references unknown signal `{}`",
                            decision.decision_id, signal_id
                        ),
                    }
                })?;
                if signal.node_identity_id != decision.node_identity_id {
                    return Err(FraudQuarantineSlashingContractError::InvalidContract {
                        detail: format!(
                            "quarantine decision `{}` mixes signals across node identities",
                            decision.decision_id
                        ),
                    });
                }
            }
            if decision.quarantine_class == QuarantineClass::ValidatorSuspended
                && !identity
                    .admitted_role_classes
                    .contains(&crate::DecentralizedNetworkRoleClass::PublicValidator)
            {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!(
                        "quarantine decision `{}` suspended a non-validator",
                        decision.decision_id
                    ),
                });
            }
        }

        let quarantine_by_id = self
            .quarantine_decisions
            .iter()
            .map(|decision| (decision.decision_id.as_str(), decision))
            .collect::<BTreeMap<_, _>>();
        for decision in &self.slashing_decisions {
            let quarantine = quarantine_by_id
                .get(decision.linked_quarantine_decision_id.as_str())
                .ok_or_else(|| FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!(
                        "slashing decision `{}` references unknown quarantine decision `{}`",
                        decision.decision_id, decision.linked_quarantine_decision_id
                    ),
                })?;
            if decision.node_identity_id != quarantine.node_identity_id
                || decision.penalty_bps == 0
                || decision.penalty_bps > 10_000
                || decision.settlement_hold_epochs == 0
            {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!("slashing decision `{}` drifted", decision.decision_id),
                });
            }
            for signal_id in &decision.supporting_signal_ids {
                let signal = signal_by_id.get(signal_id.as_str()).ok_or_else(|| {
                    FraudQuarantineSlashingContractError::InvalidContract {
                        detail: format!(
                            "slashing decision `{}` references unknown signal `{}`",
                            decision.decision_id, signal_id
                        ),
                    }
                })?;
                if signal.node_identity_id != decision.node_identity_id {
                    return Err(FraudQuarantineSlashingContractError::InvalidContract {
                        detail: format!(
                            "slashing decision `{}` mixes signals across node identities",
                            decision.decision_id
                        ),
                    });
                }
            }
        }

        for appeal in &self.appeal_windows {
            let slashing = self
                .slashing_decisions
                .iter()
                .find(|decision| decision.decision_id == appeal.challenged_action_id)
                .ok_or_else(|| FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!(
                        "appeal `{}` references unknown challenged action `{}`",
                        appeal.appeal_id, appeal.challenged_action_id
                    ),
                })?;
            if appeal.node_identity_id != slashing.node_identity_id
                || appeal.opens_epoch_id >= appeal.closes_epoch_id
            {
                return Err(FraudQuarantineSlashingContractError::InvalidContract {
                    detail: format!("appeal `{}` drifted", appeal.appeal_id),
                });
            }
        }

        if !scoring.score_receipts.iter().any(|receipt| {
            receipt.disposition == TrainingExecutionValidatorDisposition::ReplayRequired
        }) || score_receipt_ids.is_empty()
        {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: String::from(
                    "validator scoring lost the replay-required evidence required by fraud policy",
                ),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(FraudQuarantineSlashingContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_fraud_quarantine_slashing_contract(
) -> Result<FraudQuarantineSlashingContract, FraudQuarantineSlashingContractError> {
    let identities = canonical_signed_node_identity_contract_set()?;
    let dataset = canonical_public_dataset_authority_contract()?;
    let scoring = canonical_validator_challenge_scoring_contract()?;
    let consensus = canonical_multi_validator_consensus_contract()?;

    let local_rtx_wallet = identities
        .identities
        .iter()
        .find(|identity| identity.node_identity_id == "local_rtx4080_workstation.identity")
        .expect("canonical identity set must retain the RTX 4080 node")
        .wallet
        .wallet_address
        .clone();

    let fraud_signals = vec![
        FraudSignal {
            signal_id: String::from("signal.sybil.local_rtx4080.wallet_watch"),
            node_identity_id: String::from("local_rtx4080_workstation.identity"),
            evidence_reference_id: format!("wallet_watch:{local_rtx_wallet}"),
            signal_kind: FraudSignalKind::SybilWalletReuse,
            severity_bps: 6500,
            detail: String::from(
                "The network freezes one sybil-watch signal keyed by the RTX 4080 wallet address so future duplicate-wallet or duplicate-capability admissions fail closed instead of being resolved ad hoc.",
            ),
        },
        FraudSignal {
            signal_id: String::from("signal.replay.local_rtx4080.window1230"),
            node_identity_id: String::from("local_rtx4080_workstation.identity"),
            evidence_reference_id: String::from(
                "anti_replay.assignment.public_miner.window1230.google.duplicate",
            ),
            signal_kind: FraudSignalKind::DatasetReplayAbuse,
            severity_bps: 9300,
            detail: String::from(
                "The RTX 4080 node triggers one explicit replay-abuse signal because the public dataset authority recorded a duplicate assignment receipt for the same miner window.",
            ),
        },
        FraudSignal {
            signal_id: String::from("signal.disagreement.local_mlx.window1231"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            evidence_reference_id: String::from("disagreement.checkpoint.step2048.round2056"),
            signal_kind: FraudSignalKind::ValidatorDisagreementWatch,
            severity_bps: 2200,
            detail: String::from(
                "The Apple MLX validator receives one low-severity disagreement watch because the current checkpoint remained held-no-promotion under split validator votes.",
            ),
        },
        FraudSignal {
            signal_id: String::from("signal.software.runpod.release_watch"),
            node_identity_id: String::from("runpod_8xh100_dense_node.identity"),
            evidence_reference_id: String::from("runpod_8xh100_dense_node.identity"),
            signal_kind: FraudSignalKind::StaleSoftwareDigest,
            severity_bps: 1400,
            detail: String::from(
                "RunPod checkpoint authority receives one stale-software watch example to freeze the retained evidence shape for build-digest drift without claiming that the current node is already malicious.",
            ),
        },
    ];

    let quarantine_decisions = vec![
        QuarantineDecision {
            decision_id: String::from("quarantine.public_miner.local_rtx4080.window1232"),
            node_identity_id: String::from("local_rtx4080_workstation.identity"),
            signal_ids: vec![
                String::from("signal.sybil.local_rtx4080.wallet_watch"),
                String::from("signal.replay.local_rtx4080.window1230"),
            ],
            quarantine_class: QuarantineClass::ContributionBlocked,
            starts_epoch_id: String::from("window1232"),
            ends_after_epoch_id: String::from("window1234"),
            detail: String::from(
                "Psionic blocks the RTX 4080 miner from new public work for two follow-on windows because duplicate-work evidence and wallet-watch risk together exceed the miner admission ceiling.",
            ),
        },
        QuarantineDecision {
            decision_id: String::from("quarantine.public_validator.local_mlx.observe"),
            node_identity_id: String::from("local_mlx_mac_workstation.identity"),
            signal_ids: vec![String::from("signal.disagreement.local_mlx.window1231")],
            quarantine_class: QuarantineClass::ObservationOnly,
            starts_epoch_id: String::from("window1232"),
            ends_after_epoch_id: String::from("window1233"),
            detail: String::from(
                "Validator disagreement alone does not trigger a ban, but Psionic still records one observation-only quarantine so future repeated disagreements accumulate machine-legible incident history.",
            ),
        },
    ];

    let slashing_decisions = vec![SlashingDecision {
        decision_id: String::from("slash.public_miner.local_rtx4080.window1232"),
        node_identity_id: String::from("local_rtx4080_workstation.identity"),
        supporting_signal_ids: vec![
            String::from("signal.sybil.local_rtx4080.wallet_watch"),
            String::from("signal.replay.local_rtx4080.window1230"),
        ],
        linked_quarantine_decision_id: String::from(
            "quarantine.public_miner.local_rtx4080.window1232",
        ),
        penalty_bps: 1500,
        settlement_hold_epochs: 3,
        detail: String::from(
            "The first canonical slashing decision withholds fifteen percent of the RTX 4080 miner's provisional reward share for three epochs because replay-abuse evidence already crossed the admitted fraud threshold.",
        ),
    }];

    let appeal_windows = vec![AppealWindow {
        appeal_id: String::from("appeal.public_miner.local_rtx4080.window1232"),
        node_identity_id: String::from("local_rtx4080_workstation.identity"),
        challenged_action_id: String::from("slash.public_miner.local_rtx4080.window1232"),
        status: AppealStatus::Denied,
        opens_epoch_id: String::from("window1232"),
        closes_epoch_id: String::from("window1235"),
        detail: String::from(
            "The first canonical appeal window is retained and denied because the duplicate dataset receipt remains canonical evidence rather than an operator-side dispute.",
        ),
    }];

    let mut contract = FraudQuarantineSlashingContract {
        schema_version: String::from(FRAUD_QUARANTINE_SLASHING_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(FRAUD_QUARANTINE_SLASHING_CONTRACT_ID),
        signed_node_identity_contract_set_digest: identities.contract_digest.clone(),
        public_dataset_authority_contract_digest: dataset.contract_digest.clone(),
        validator_challenge_scoring_contract_digest: scoring.contract_digest.clone(),
        multi_validator_consensus_contract_digest: consensus.contract_digest.clone(),
        fraud_signals,
        quarantine_decisions,
        slashing_decisions,
        appeal_windows,
        authority_paths: FraudQuarantineSlashingAuthorityPaths {
            fixture_path: String::from(FRAUD_QUARANTINE_SLASHING_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(FRAUD_QUARANTINE_SLASHING_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(FRAUD_QUARANTINE_SLASHING_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                FRAUD_QUARANTINE_SLASHING_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first decentralized fraud-defense surface: sybil-watch signals, duplicate-work evidence, observation versus blocking quarantines, one slashing decision, and one explicit appeal window. It does not yet claim automatic fraud-proof generation or fully open permissionless admission.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_fraud_quarantine_slashing_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), FraudQuarantineSlashingContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FraudQuarantineSlashingContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_fraud_quarantine_slashing_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| FraudQuarantineSlashingContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for fraud quarantine slashing contract",
        ),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_fraud_quarantine_slashing_contract, FraudQuarantineSlashingContractError,
        FraudSignalKind,
    };

    #[test]
    fn canonical_fraud_quarantine_slashing_contract_is_valid(
    ) -> Result<(), FraudQuarantineSlashingContractError> {
        let contract = canonical_fraud_quarantine_slashing_contract()?;
        contract.validate()
    }

    #[test]
    fn duplicate_replay_signal_cannot_drift() -> Result<(), FraudQuarantineSlashingContractError> {
        let mut contract = canonical_fraud_quarantine_slashing_contract()?;
        let signal = contract
            .fraud_signals
            .iter_mut()
            .find(|signal| signal.signal_kind == FraudSignalKind::DatasetReplayAbuse)
            .expect("canonical contract should retain the duplicate replay signal");
        signal.evidence_reference_id =
            String::from("anti_replay.assignment.public_miner.window1231.google");
        let error = contract
            .validate()
            .expect_err("duplicate replay evidence cannot drift");
        assert!(matches!(
            error,
            FraudQuarantineSlashingContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
