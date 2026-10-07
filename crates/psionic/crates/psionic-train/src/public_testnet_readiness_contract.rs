use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_fraud_quarantine_slashing_contract, canonical_operator_bootstrap_package_contract,
    canonical_public_run_explorer_contract, DecentralizedNetworkRoleClass,
    FraudQuarantineSlashingContractError, OperatorBootstrapPackageContractError,
    PublicRunExplorerContractError,
};

pub const PUBLIC_TESTNET_READINESS_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.public_testnet_readiness_contract.v1";
pub const PUBLIC_TESTNET_READINESS_CONTRACT_ID: &str =
    "psionic.public_testnet_readiness_contract.v1";
pub const PUBLIC_TESTNET_READINESS_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/public_testnet_readiness_contract_v1.json";
pub const PUBLIC_TESTNET_READINESS_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-public-testnet-readiness-contract.sh";
pub const PUBLIC_TESTNET_READINESS_CONTRACT_DOC_PATH: &str =
    "docs/PUBLIC_TESTNET_READINESS_REFERENCE.md";
pub const PUBLIC_TESTNET_READINESS_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum PublicTestnetReadinessContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    FraudPolicy(#[from] FraudQuarantineSlashingContractError),
    #[error(transparent)]
    OperatorBootstrap(#[from] OperatorBootstrapPackageContractError),
    #[error(transparent)]
    Explorer(#[from] PublicRunExplorerContractError),
    #[error("public testnet readiness contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicTestnetTier {
    DryRun,
    Canary,
    RewardEligible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicTestnetComplianceCheckKind {
    RegistrationFlow,
    BenchmarkPreflight,
    FraudDrill,
    ExplorerVisibility,
    DryRunParticipation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicTestnetReadinessDisposition {
    RewardEligible,
    CanaryOnly,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTestnetCandidate {
    pub candidate_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_identity_id: Option<String>,
    pub admission_endpoint: String,
    pub requested_tier: PublicTestnetTier,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTestnetComplianceReceipt {
    pub receipt_id: String,
    pub candidate_id: String,
    pub check_kind: PublicTestnetComplianceCheckKind,
    pub reference_id: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTestnetGraduationDecision {
    pub decision_id: String,
    pub candidate_id: String,
    pub disposition: PublicTestnetReadinessDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_tier: Option<PublicTestnetTier>,
    pub blocking_reference_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTestnetReadinessAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTestnetReadinessContract {
    pub schema_version: String,
    pub contract_id: String,
    pub fraud_quarantine_slashing_contract_digest: String,
    pub operator_bootstrap_package_contract_digest: String,
    pub public_run_explorer_contract_digest: String,
    pub candidates: Vec<PublicTestnetCandidate>,
    pub compliance_receipts: Vec<PublicTestnetComplianceReceipt>,
    pub graduation_decisions: Vec<PublicTestnetGraduationDecision>,
    pub authority_paths: PublicTestnetReadinessAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl PublicTestnetReadinessContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_public_testnet_readiness_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), PublicTestnetReadinessContractError> {
        let fraud = canonical_fraud_quarantine_slashing_contract()?;
        let packages = canonical_operator_bootstrap_package_contract()?;
        let explorer = canonical_public_run_explorer_contract()?;

        let package_by_id = packages
            .packages
            .iter()
            .map(|package| (package.package_id.as_str(), package))
            .collect::<BTreeMap<_, _>>();
        let explorer_pane_ids = explorer
            .panes
            .iter()
            .map(|pane| pane.pane_id.as_str())
            .collect::<BTreeSet<_>>();
        let fraud_reference_ids = fraud
            .quarantine_decisions
            .iter()
            .map(|decision| decision.decision_id.as_str())
            .chain(
                fraud
                    .slashing_decisions
                    .iter()
                    .map(|decision| decision.decision_id.as_str()),
            )
            .collect::<BTreeSet<_>>();

        if self.schema_version != PUBLIC_TESTNET_READINESS_CONTRACT_SCHEMA_VERSION {
            return Err(PublicTestnetReadinessContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    PUBLIC_TESTNET_READINESS_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != PUBLIC_TESTNET_READINESS_CONTRACT_ID {
            return Err(PublicTestnetReadinessContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.fraud_quarantine_slashing_contract_digest != fraud.contract_digest
            || self.operator_bootstrap_package_contract_digest != packages.contract_digest
            || self.public_run_explorer_contract_digest != explorer.contract_digest
        {
            return Err(PublicTestnetReadinessContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != PUBLIC_TESTNET_READINESS_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != PUBLIC_TESTNET_READINESS_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != PUBLIC_TESTNET_READINESS_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != PUBLIC_TESTNET_READINESS_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(PublicTestnetReadinessContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.candidates.len() != 5
            || self.compliance_receipts.len() != 8
            || self.graduation_decisions.len() != 5
        {
            return Err(PublicTestnetReadinessContractError::InvalidContract {
                detail: String::from(
                    "expected canonical candidate, compliance, and decision counts",
                ),
            });
        }

        let candidate_by_id = self
            .candidates
            .iter()
            .map(|candidate| (candidate.candidate_id.as_str(), candidate))
            .collect::<BTreeMap<_, _>>();
        for candidate in &self.candidates {
            let package = package_by_id
                .get(candidate.package_id.as_str())
                .ok_or_else(|| PublicTestnetReadinessContractError::InvalidContract {
                    detail: format!(
                        "candidate `{}` references unknown package `{}`",
                        candidate.candidate_id, candidate.package_id
                    ),
                })?;
            if package.role_class != candidate.role_class || candidate.admission_endpoint.is_empty()
            {
                return Err(PublicTestnetReadinessContractError::InvalidContract {
                    detail: format!("candidate `{}` drifted", candidate.candidate_id),
                });
            }
        }

        for receipt in &self.compliance_receipts {
            let candidate = candidate_by_id
                .get(receipt.candidate_id.as_str())
                .ok_or_else(|| PublicTestnetReadinessContractError::InvalidContract {
                    detail: format!(
                        "compliance receipt `{}` references unknown candidate `{}`",
                        receipt.receipt_id, receipt.candidate_id
                    ),
                })?;
            match receipt.check_kind {
                PublicTestnetComplianceCheckKind::RegistrationFlow
                | PublicTestnetComplianceCheckKind::BenchmarkPreflight
                | PublicTestnetComplianceCheckKind::DryRunParticipation => {
                    if !packages.preflight_checks.iter().any(|check| {
                        check.check_id == receipt.reference_id
                            && check.package_id == candidate.package_id
                    }) {
                        return Err(PublicTestnetReadinessContractError::InvalidContract {
                            detail: format!(
                                "compliance receipt `{}` lost bootstrap preflight binding",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                PublicTestnetComplianceCheckKind::FraudDrill => {
                    if !fraud_reference_ids.contains(receipt.reference_id.as_str()) {
                        return Err(PublicTestnetReadinessContractError::InvalidContract {
                            detail: format!(
                                "compliance receipt `{}` lost fraud-policy binding",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                PublicTestnetComplianceCheckKind::ExplorerVisibility => {
                    if !explorer_pane_ids.contains(receipt.reference_id.as_str()) {
                        return Err(PublicTestnetReadinessContractError::InvalidContract {
                            detail: format!(
                                "compliance receipt `{}` lost explorer binding",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
            }
        }

        for decision in &self.graduation_decisions {
            let _candidate = candidate_by_id
                .get(decision.candidate_id.as_str())
                .ok_or_else(|| PublicTestnetReadinessContractError::InvalidContract {
                    detail: format!(
                        "graduation decision `{}` references unknown candidate `{}`",
                        decision.decision_id, decision.candidate_id
                    ),
                })?;
            match decision.disposition {
                PublicTestnetReadinessDisposition::RewardEligible => {
                    if decision.granted_tier != Some(PublicTestnetTier::RewardEligible)
                        || !decision.blocking_reference_ids.is_empty()
                    {
                        return Err(PublicTestnetReadinessContractError::InvalidContract {
                            detail: format!(
                                "graduation decision `{}` lost reward-eligible shape",
                                decision.decision_id
                            ),
                        });
                    }
                }
                PublicTestnetReadinessDisposition::CanaryOnly => {
                    if decision.granted_tier != Some(PublicTestnetTier::Canary)
                        || !decision.blocking_reference_ids.is_empty()
                    {
                        return Err(PublicTestnetReadinessContractError::InvalidContract {
                            detail: format!(
                                "graduation decision `{}` lost canary shape",
                                decision.decision_id
                            ),
                        });
                    }
                }
                PublicTestnetReadinessDisposition::Blocked => {
                    if decision.granted_tier.is_some() || decision.blocking_reference_ids.is_empty()
                    {
                        return Err(PublicTestnetReadinessContractError::InvalidContract {
                            detail: format!(
                                "graduation decision `{}` lost blocked shape",
                                decision.decision_id
                            ),
                        });
                    }
                    for reference_id in &decision.blocking_reference_ids {
                        if !fraud_reference_ids.contains(reference_id.as_str()) {
                            return Err(PublicTestnetReadinessContractError::InvalidContract {
                                detail: format!(
                                    "blocked decision `{}` lost fraud-policy blocker `{}`",
                                    decision.decision_id, reference_id
                                ),
                            });
                        }
                    }
                }
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(PublicTestnetReadinessContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_public_testnet_readiness_contract(
) -> Result<PublicTestnetReadinessContract, PublicTestnetReadinessContractError> {
    let fraud = canonical_fraud_quarantine_slashing_contract()?;
    let packages = canonical_operator_bootstrap_package_contract()?;
    let explorer = canonical_public_run_explorer_contract()?;

    let candidates = vec![
        PublicTestnetCandidate {
            candidate_id: String::from("candidate.public_miner.google"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            package_id: String::from("package.public_miner.bootstrap.v1"),
            node_identity_id: Some(String::from("google_l4_validator_node.identity")),
            admission_endpoint: String::from("https://registry.psionic.testnet/google/miner"),
            requested_tier: PublicTestnetTier::RewardEligible,
            detail: String::from(
                "Google remains one curator-controlled reward-eligible miner candidate for the first staged public testnet.",
            ),
        },
        PublicTestnetCandidate {
            candidate_id: String::from("candidate.public_validator.local_mlx"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            package_id: String::from("package.public_validator.bootstrap.v1"),
            node_identity_id: Some(String::from("local_mlx_mac_workstation.identity")),
            admission_endpoint: String::from("https://registry.psionic.testnet/local-mlx/validator"),
            requested_tier: PublicTestnetTier::RewardEligible,
            detail: String::from(
                "Apple MLX remains one curator-controlled reward-eligible validator candidate for the first staged public testnet.",
            ),
        },
        PublicTestnetCandidate {
            candidate_id: String::from("candidate.public_miner.community_rtx4090_east"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            package_id: String::from("package.public_miner.bootstrap.v1"),
            node_identity_id: None,
            admission_endpoint: String::from("https://registry.psionic.testnet/community/rtx4090-east"),
            requested_tier: PublicTestnetTier::Canary,
            detail: String::from(
                "One outside miner candidate is admitted only to canary scope until public fraud drills and explorer visibility stay stable.",
            ),
        },
        PublicTestnetCandidate {
            candidate_id: String::from("candidate.public_validator.community_h100_central"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            package_id: String::from("package.public_validator.bootstrap.v1"),
            node_identity_id: None,
            admission_endpoint: String::from("https://registry.psionic.testnet/community/h100-central"),
            requested_tier: PublicTestnetTier::Canary,
            detail: String::from(
                "One outside validator candidate is admitted only to canary scope until replay and explorer paths stay stable under third-party participation.",
            ),
        },
        PublicTestnetCandidate {
            candidate_id: String::from("candidate.public_miner.local_rtx4080"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            package_id: String::from("package.public_miner.bootstrap.v1"),
            node_identity_id: Some(String::from("local_rtx4080_workstation.identity")),
            admission_endpoint: String::from("https://registry.psionic.testnet/local-rtx4080/miner"),
            requested_tier: PublicTestnetTier::RewardEligible,
            detail: String::from(
                "The slashed RTX 4080 miner remains a blocked example candidate so the staged testnet has a retained negative-path admission record.",
            ),
        },
    ];

    let compliance_receipts = vec![
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.google.registration"),
            candidate_id: String::from("candidate.public_miner.google"),
            check_kind: PublicTestnetComplianceCheckKind::RegistrationFlow,
            reference_id: String::from("preflight.public_miner.google.benchmark"),
            passed: true,
            detail: String::from("Google passed the registration flow and benchmark preflight."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.google.dry_run"),
            candidate_id: String::from("candidate.public_miner.google"),
            check_kind: PublicTestnetComplianceCheckKind::DryRunParticipation,
            reference_id: String::from("preflight.public_miner.google.dry_run"),
            passed: true,
            detail: String::from("Google passed the miner dry-run participation gate."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.local_mlx.registration"),
            candidate_id: String::from("candidate.public_validator.local_mlx"),
            check_kind: PublicTestnetComplianceCheckKind::RegistrationFlow,
            reference_id: String::from("preflight.public_validator.local_mlx.registration"),
            passed: true,
            detail: String::from("Apple MLX passed the validator registration gate."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.local_mlx.benchmark"),
            candidate_id: String::from("candidate.public_validator.local_mlx"),
            check_kind: PublicTestnetComplianceCheckKind::BenchmarkPreflight,
            reference_id: String::from("preflight.public_validator.google.replay"),
            passed: true,
            detail: String::from("Apple MLX passed the replay-fixture validator benchmark gate."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.community_rtx4090.visibility"),
            candidate_id: String::from("candidate.public_miner.community_rtx4090_east"),
            check_kind: PublicTestnetComplianceCheckKind::ExplorerVisibility,
            reference_id: String::from("pane.node_status"),
            passed: true,
            detail: String::from("The community RTX 4090 miner is admitted only once explorer visibility is live."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.community_h100.visibility"),
            candidate_id: String::from("candidate.public_validator.community_h100_central"),
            check_kind: PublicTestnetComplianceCheckKind::ExplorerVisibility,
            reference_id: String::from("pane.scoreboard"),
            passed: true,
            detail: String::from("The community H100 validator is admitted only once explorer score visibility is live."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.local_rtx4080.fraud_drill"),
            candidate_id: String::from("candidate.public_miner.local_rtx4080"),
            check_kind: PublicTestnetComplianceCheckKind::FraudDrill,
            reference_id: String::from("quarantine.public_miner.local_rtx4080.window1232"),
            passed: false,
            detail: String::from("The RTX 4080 miner fails the fraud drill because the canonical quarantine remains active."),
        },
        PublicTestnetComplianceReceipt {
            receipt_id: String::from("compliance.local_rtx4080.slash_gate"),
            candidate_id: String::from("candidate.public_miner.local_rtx4080"),
            check_kind: PublicTestnetComplianceCheckKind::FraudDrill,
            reference_id: String::from("slash.public_miner.local_rtx4080.window1232"),
            passed: false,
            detail: String::from("The RTX 4080 miner also fails the slashing gate because the canonical penalty remains active."),
        },
    ];

    let graduation_decisions = vec![
        PublicTestnetGraduationDecision {
            decision_id: String::from("decision.google.reward_eligible"),
            candidate_id: String::from("candidate.public_miner.google"),
            disposition: PublicTestnetReadinessDisposition::RewardEligible,
            granted_tier: Some(PublicTestnetTier::RewardEligible),
            blocking_reference_ids: vec![],
            detail: String::from(
                "Google graduates to reward-eligible participation because it passed the miner package, dry-run, and visibility gates.",
            ),
        },
        PublicTestnetGraduationDecision {
            decision_id: String::from("decision.local_mlx.reward_eligible"),
            candidate_id: String::from("candidate.public_validator.local_mlx"),
            disposition: PublicTestnetReadinessDisposition::RewardEligible,
            granted_tier: Some(PublicTestnetTier::RewardEligible),
            blocking_reference_ids: vec![],
            detail: String::from(
                "Apple MLX graduates to reward-eligible participation because it passed the validator package, replay, and visibility gates.",
            ),
        },
        PublicTestnetGraduationDecision {
            decision_id: String::from("decision.community_rtx4090.canary"),
            candidate_id: String::from("candidate.public_miner.community_rtx4090_east"),
            disposition: PublicTestnetReadinessDisposition::CanaryOnly,
            granted_tier: Some(PublicTestnetTier::Canary),
            blocking_reference_ids: vec![],
            detail: String::from(
                "The outside RTX 4090 miner is admitted only to canary scope until open public runs prove stable under third-party participation.",
            ),
        },
        PublicTestnetGraduationDecision {
            decision_id: String::from("decision.community_h100.canary"),
            candidate_id: String::from("candidate.public_validator.community_h100_central"),
            disposition: PublicTestnetReadinessDisposition::CanaryOnly,
            granted_tier: Some(PublicTestnetTier::Canary),
            blocking_reference_ids: vec![],
            detail: String::from(
                "The outside H100 validator is admitted only to canary scope until third-party replay and explorer publication remain stable.",
            ),
        },
        PublicTestnetGraduationDecision {
            decision_id: String::from("decision.local_rtx4080.blocked"),
            candidate_id: String::from("candidate.public_miner.local_rtx4080"),
            disposition: PublicTestnetReadinessDisposition::Blocked,
            granted_tier: None,
            blocking_reference_ids: vec![
                String::from("quarantine.public_miner.local_rtx4080.window1232"),
                String::from("slash.public_miner.local_rtx4080.window1232"),
            ],
            detail: String::from(
                "The RTX 4080 miner remains blocked because both the quarantine and the slashing decision are still active in canonical fraud policy.",
            ),
        },
    ];

    let mut contract = PublicTestnetReadinessContract {
        schema_version: String::from(PUBLIC_TESTNET_READINESS_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PUBLIC_TESTNET_READINESS_CONTRACT_ID),
        fraud_quarantine_slashing_contract_digest: fraud.contract_digest.clone(),
        operator_bootstrap_package_contract_digest: packages.contract_digest.clone(),
        public_run_explorer_contract_digest: explorer.contract_digest.clone(),
        candidates,
        compliance_receipts,
        graduation_decisions,
        authority_paths: PublicTestnetReadinessAuthorityPaths {
            fixture_path: String::from(PUBLIC_TESTNET_READINESS_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PUBLIC_TESTNET_READINESS_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(PUBLIC_TESTNET_READINESS_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                PUBLIC_TESTNET_READINESS_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first staged public-testnet gate: candidates, compliance receipts, reward-eligible versus canary decisions, and explicit blocked admission on fraud policy. It does not yet claim curated or open public training runs.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_public_testnet_readiness_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), PublicTestnetReadinessContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PublicTestnetReadinessContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_public_testnet_readiness_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| PublicTestnetReadinessContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for public testnet readiness contract",
        ),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{canonical_public_testnet_readiness_contract, PublicTestnetReadinessContractError};

    #[test]
    fn canonical_public_testnet_readiness_contract_is_valid(
    ) -> Result<(), PublicTestnetReadinessContractError> {
        let contract = canonical_public_testnet_readiness_contract()?;
        contract.validate()
    }

    #[test]
    fn blocked_candidate_cannot_lose_fraud_references(
    ) -> Result<(), PublicTestnetReadinessContractError> {
        let mut contract = canonical_public_testnet_readiness_contract()?;
        let decision = contract
            .graduation_decisions
            .iter_mut()
            .find(|decision| decision.decision_id == "decision.local_rtx4080.blocked")
            .expect("canonical readiness contract should retain the blocked RTX 4080 decision");
        decision.blocking_reference_ids.clear();
        let error = contract
            .validate()
            .expect_err("blocked candidates cannot lose fraud blockers");
        assert!(matches!(
            error,
            PublicTestnetReadinessContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
