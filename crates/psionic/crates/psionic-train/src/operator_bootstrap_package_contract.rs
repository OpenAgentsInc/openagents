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
    canonical_signed_node_identity_contract_set, canonical_validator_challenge_scoring_contract,
    DecentralizedNetworkRoleClass, PublicMinerProtocolContractError,
    PublicNetworkRegistryContractError, SignedNodeIdentityContractSetError,
    ValidatorChallengeScoringContractError,
};

pub const OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.operator_bootstrap_package_contract.v1";
pub const OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_ID: &str =
    "psionic.operator_bootstrap_package_contract.v1";
pub const OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/operator_bootstrap_package_contract_v1.json";
pub const OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-operator-bootstrap-package-contract.sh";
pub const OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_DOC_PATH: &str =
    "docs/OPERATOR_BOOTSTRAP_PACKAGE_REFERENCE.md";
pub const OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum OperatorBootstrapPackageContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    SignedNodeIdentity(#[from] SignedNodeIdentityContractSetError),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    PublicMinerProtocol(#[from] PublicMinerProtocolContractError),
    #[error(transparent)]
    ValidatorScoring(#[from] ValidatorChallengeScoringContractError),
    #[error("operator bootstrap package contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorPreflightCheckKind {
    BenchmarkReceipt,
    RegistrationFlow,
    DryRunHandshake,
    ScoreReplayFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorPackage {
    pub package_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub release_id: String,
    pub container_image: String,
    pub bootstrap_manifest_path: String,
    pub sample_env_path: String,
    pub registration_command: String,
    pub dry_run_command: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorPreflightCheck {
    pub check_id: String,
    pub package_id: String,
    pub check_kind: OperatorPreflightCheckKind,
    pub reference_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorBootstrapKit {
    pub kit_id: String,
    pub role_class: DecentralizedNetworkRoleClass,
    pub package_id: String,
    pub example_registry_record_id: String,
    pub example_protocol_reference_id: String,
    pub operator_doc_path: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorBootstrapPackageAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorBootstrapPackageContract {
    pub schema_version: String,
    pub contract_id: String,
    pub signed_node_identity_contract_set_digest: String,
    pub public_network_registry_contract_digest: String,
    pub public_miner_protocol_contract_digest: String,
    pub validator_challenge_scoring_contract_digest: String,
    pub packages: Vec<OperatorPackage>,
    pub preflight_checks: Vec<OperatorPreflightCheck>,
    pub bootstrap_kits: Vec<OperatorBootstrapKit>,
    pub authority_paths: OperatorBootstrapPackageAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl OperatorBootstrapPackageContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_operator_bootstrap_package_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), OperatorBootstrapPackageContractError> {
        let identities = canonical_signed_node_identity_contract_set()?;
        let registry = canonical_public_network_registry_contract()?;
        let miner_protocol = canonical_public_miner_protocol_contract()?;
        let scoring = canonical_validator_challenge_scoring_contract()?;

        let registry_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let miner_session_ids = miner_protocol
            .sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<BTreeSet<_>>();
        let score_receipt_ids = scoring
            .score_receipts
            .iter()
            .map(|receipt| receipt.receipt_id.as_str())
            .collect::<BTreeSet<_>>();

        if self.schema_version != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_SCHEMA_VERSION {
            return Err(OperatorBootstrapPackageContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_ID {
            return Err(OperatorBootstrapPackageContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.signed_node_identity_contract_set_digest != identities.contract_digest
            || self.public_network_registry_contract_digest != registry.contract_digest
            || self.public_miner_protocol_contract_digest != miner_protocol.contract_digest
            || self.validator_challenge_scoring_contract_digest != scoring.contract_digest
        {
            return Err(OperatorBootstrapPackageContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path
                != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(OperatorBootstrapPackageContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.packages.len() != 2
            || self.preflight_checks.len() != 4
            || self.bootstrap_kits.len() != 2
        {
            return Err(OperatorBootstrapPackageContractError::InvalidContract {
                detail: String::from("expected canonical package, preflight, and kit counts"),
            });
        }

        let mut package_ids = BTreeSet::new();
        for package in &self.packages {
            if !package_ids.insert(package.package_id.as_str()) {
                return Err(OperatorBootstrapPackageContractError::InvalidContract {
                    detail: format!("duplicate package `{}`", package.package_id),
                });
            }
            if !matches!(
                package.role_class,
                DecentralizedNetworkRoleClass::PublicMiner
                    | DecentralizedNetworkRoleClass::PublicValidator
            ) || package.container_image.is_empty()
                || package.registration_command.is_empty()
                || package.dry_run_command.is_empty()
            {
                return Err(OperatorBootstrapPackageContractError::InvalidContract {
                    detail: format!("operator package `{}` drifted", package.package_id),
                });
            }
        }

        let package_by_id = self
            .packages
            .iter()
            .map(|package| (package.package_id.as_str(), package))
            .collect::<BTreeMap<_, _>>();
        for check in &self.preflight_checks {
            let package = package_by_id
                .get(check.package_id.as_str())
                .ok_or_else(|| OperatorBootstrapPackageContractError::InvalidContract {
                    detail: format!(
                        "preflight check `{}` references unknown package `{}`",
                        check.check_id, check.package_id
                    ),
                })?;
            match check.check_kind {
                OperatorPreflightCheckKind::BenchmarkReceipt
                | OperatorPreflightCheckKind::RegistrationFlow => {
                    if !registry_ids.contains(check.reference_id.as_str()) {
                        return Err(OperatorBootstrapPackageContractError::InvalidContract {
                            detail: format!(
                                "preflight check `{}` lost registry binding",
                                check.check_id
                            ),
                        });
                    }
                }
                OperatorPreflightCheckKind::DryRunHandshake => {
                    if package.role_class != DecentralizedNetworkRoleClass::PublicMiner
                        || !miner_session_ids.contains(check.reference_id.as_str())
                    {
                        return Err(OperatorBootstrapPackageContractError::InvalidContract {
                            detail: format!(
                                "preflight check `{}` lost miner dry-run binding",
                                check.check_id
                            ),
                        });
                    }
                }
                OperatorPreflightCheckKind::ScoreReplayFixture => {
                    if package.role_class != DecentralizedNetworkRoleClass::PublicValidator
                        || !score_receipt_ids.contains(check.reference_id.as_str())
                    {
                        return Err(OperatorBootstrapPackageContractError::InvalidContract {
                            detail: format!(
                                "preflight check `{}` lost validator replay binding",
                                check.check_id
                            ),
                        });
                    }
                }
            }
        }

        for kit in &self.bootstrap_kits {
            let package = package_by_id.get(kit.package_id.as_str()).ok_or_else(|| {
                OperatorBootstrapPackageContractError::InvalidContract {
                    detail: format!(
                        "bootstrap kit `{}` references unknown package `{}`",
                        kit.kit_id, kit.package_id
                    ),
                }
            })?;
            if kit.role_class != package.role_class
                || !registry_ids.contains(kit.example_registry_record_id.as_str())
                || kit.operator_doc_path != OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_DOC_PATH
            {
                return Err(OperatorBootstrapPackageContractError::InvalidContract {
                    detail: format!("bootstrap kit `{}` drifted", kit.kit_id),
                });
            }
            match kit.role_class {
                DecentralizedNetworkRoleClass::PublicMiner => {
                    if !miner_session_ids.contains(kit.example_protocol_reference_id.as_str()) {
                        return Err(OperatorBootstrapPackageContractError::InvalidContract {
                            detail: format!(
                                "bootstrap kit `{}` lost miner protocol example",
                                kit.kit_id
                            ),
                        });
                    }
                }
                DecentralizedNetworkRoleClass::PublicValidator => {
                    if !score_receipt_ids.contains(kit.example_protocol_reference_id.as_str()) {
                        return Err(OperatorBootstrapPackageContractError::InvalidContract {
                            detail: format!(
                                "bootstrap kit `{}` lost validator replay example",
                                kit.kit_id
                            ),
                        });
                    }
                }
                _ => {
                    return Err(OperatorBootstrapPackageContractError::InvalidContract {
                        detail: format!(
                            "bootstrap kit `{}` used an unsupported role class",
                            kit.kit_id
                        ),
                    });
                }
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(OperatorBootstrapPackageContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

pub fn canonical_operator_bootstrap_package_contract(
) -> Result<OperatorBootstrapPackageContract, OperatorBootstrapPackageContractError> {
    let identities = canonical_signed_node_identity_contract_set()?;
    let registry = canonical_public_network_registry_contract()?;
    let miner_protocol = canonical_public_miner_protocol_contract()?;
    let scoring = canonical_validator_challenge_scoring_contract()?;

    let packages = vec![
        OperatorPackage {
            package_id: String::from("package.public_miner.bootstrap.v1"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            release_id: String::from("psionic.public_miner.bootstrap.release.v1"),
            container_image: String::from("ghcr.io/openagentsinc/psionic/public-miner:v1"),
            bootstrap_manifest_path: String::from(
                "operators/public-miner/bootstrap-manifest-v1.json",
            ),
            sample_env_path: String::from("operators/public-miner/example.env"),
            registration_command: String::from(
                "psionicctl public-miner register --manifest operators/public-miner/bootstrap-manifest-v1.json",
            ),
            dry_run_command: String::from("psionicctl public-miner dry-run --window window1231"),
            detail: String::from(
                "The first miner package freezes one reproducible image, one bootstrap manifest, one example env file, and one dry-run registration flow for public miners.",
            ),
        },
        OperatorPackage {
            package_id: String::from("package.public_validator.bootstrap.v1"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            release_id: String::from("psionic.public_validator.bootstrap.release.v1"),
            container_image: String::from("ghcr.io/openagentsinc/psionic/public-validator:v1"),
            bootstrap_manifest_path: String::from(
                "operators/public-validator/bootstrap-manifest-v1.json",
            ),
            sample_env_path: String::from("operators/public-validator/example.env"),
            registration_command: String::from(
                "psionicctl public-validator register --manifest operators/public-validator/bootstrap-manifest-v1.json",
            ),
            dry_run_command: String::from(
                "psionicctl public-validator dry-run --score-fixture score.public_validator.google.local_mlx.window1231",
            ),
            detail: String::from(
                "The first validator package freezes one reproducible image, one bootstrap manifest, one example env file, and one dry-run replay flow for public validators.",
            ),
        },
    ];

    let preflight_checks = vec![
        OperatorPreflightCheck {
            check_id: String::from("preflight.public_miner.google.benchmark"),
            package_id: String::from("package.public_miner.bootstrap.v1"),
            check_kind: OperatorPreflightCheckKind::BenchmarkReceipt,
            reference_id: String::from("google_l4_validator_node.registry"),
            detail: String::from(
                "The miner package retains one benchmark preflight example from the Google public node to keep capability verification machine-legible.",
            ),
        },
        OperatorPreflightCheck {
            check_id: String::from("preflight.public_miner.google.dry_run"),
            package_id: String::from("package.public_miner.bootstrap.v1"),
            check_kind: OperatorPreflightCheckKind::DryRunHandshake,
            reference_id: String::from("session.public_miner.google.window1231"),
            detail: String::from(
                "The miner package retains one dry-run handshake against the canonical Google public-miner session.",
            ),
        },
        OperatorPreflightCheck {
            check_id: String::from("preflight.public_validator.local_mlx.registration"),
            package_id: String::from("package.public_validator.bootstrap.v1"),
            check_kind: OperatorPreflightCheckKind::RegistrationFlow,
            reference_id: String::from("local_mlx_mac_workstation.registry"),
            detail: String::from(
                "The validator package retains one canonical registration flow against the Apple MLX validator node.",
            ),
        },
        OperatorPreflightCheck {
            check_id: String::from("preflight.public_validator.google.replay"),
            package_id: String::from("package.public_validator.bootstrap.v1"),
            check_kind: OperatorPreflightCheckKind::ScoreReplayFixture,
            reference_id: String::from("score.public_validator.google.local_mlx.window1231"),
            detail: String::from(
                "The validator package retains one replay-fixture preflight against the accepted Google validator score receipt.",
            ),
        },
    ];

    let bootstrap_kits = vec![
        OperatorBootstrapKit {
            kit_id: String::from("kit.public_miner.bootstrap.v1"),
            role_class: DecentralizedNetworkRoleClass::PublicMiner,
            package_id: String::from("package.public_miner.bootstrap.v1"),
            example_registry_record_id: String::from("google_l4_validator_node.registry"),
            example_protocol_reference_id: String::from("session.public_miner.google.window1231"),
            operator_doc_path: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_DOC_PATH),
            detail: String::from(
                "The first miner kit packages the image, env manifest, and dry-run handshake needed to join the permissioned public-miner testnet path.",
            ),
        },
        OperatorBootstrapKit {
            kit_id: String::from("kit.public_validator.bootstrap.v1"),
            role_class: DecentralizedNetworkRoleClass::PublicValidator,
            package_id: String::from("package.public_validator.bootstrap.v1"),
            example_registry_record_id: String::from("local_mlx_mac_workstation.registry"),
            example_protocol_reference_id: String::from(
                "score.public_validator.google.local_mlx.window1231",
            ),
            operator_doc_path: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_DOC_PATH),
            detail: String::from(
                "The first validator kit packages the image, env manifest, and replay fixture needed to join the permissioned public-validator testnet path.",
            ),
        },
    ];

    let mut contract = OperatorBootstrapPackageContract {
        schema_version: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_ID),
        signed_node_identity_contract_set_digest: identities.contract_digest.clone(),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        public_miner_protocol_contract_digest: miner_protocol.contract_digest.clone(),
        validator_challenge_scoring_contract_digest: scoring.contract_digest.clone(),
        packages,
        preflight_checks,
        bootstrap_kits,
        authority_paths: OperatorBootstrapPackageAuthorityPaths {
            fixture_path: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(
                OPERATOR_BOOTSTRAP_PACKAGE_CONTRACT_TRAIN_SYSTEM_DOC_PATH,
            ),
        },
        claim_boundary: String::from(
            "This contract freezes the first public miner and validator operator packages: reproducible images, env manifests, registration commands, dry-run commands, and preflight checks. It does not yet claim the public explorer or graduated public testnet gates.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn write_operator_bootstrap_package_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), OperatorBootstrapPackageContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OperatorBootstrapPackageContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_operator_bootstrap_package_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| OperatorBootstrapPackageContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect(
        "stable digest serialization must succeed for operator bootstrap package contract",
    ));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_operator_bootstrap_package_contract, OperatorBootstrapPackageContractError,
        OperatorPreflightCheckKind,
    };

    #[test]
    fn canonical_operator_bootstrap_package_contract_is_valid(
    ) -> Result<(), OperatorBootstrapPackageContractError> {
        let contract = canonical_operator_bootstrap_package_contract()?;
        contract.validate()
    }

    #[test]
    fn validator_preflight_cannot_point_at_miner_session(
    ) -> Result<(), OperatorBootstrapPackageContractError> {
        let mut contract = canonical_operator_bootstrap_package_contract()?;
        let check = contract
            .preflight_checks
            .iter_mut()
            .find(|check| check.check_kind == OperatorPreflightCheckKind::ScoreReplayFixture)
            .expect("canonical contract should retain a validator replay preflight");
        check.reference_id = String::from("session.public_miner.google.window1231");
        let error = contract
            .validate()
            .expect_err("validator replay preflight cannot drift onto miner protocol state");
        assert!(matches!(
            error,
            OperatorBootstrapPackageContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
