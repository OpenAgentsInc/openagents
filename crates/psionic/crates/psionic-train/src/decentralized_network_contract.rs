use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_program_run_graph, canonical_shared_validator_promotion_contract,
    cross_provider_training_program_manifest, CrossProviderExecutionClass,
    CrossProviderProgramRunGraphError, CrossProviderTrainingProgramManifestError,
    SharedValidatorPromotionContractError, TrainingParticipantRole,
};

/// Stable schema version for the first decentralized network contract.
pub const DECENTRALIZED_NETWORK_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.decentralized_network_contract.v1";
/// Stable contract id for the first decentralized network contract.
pub const DECENTRALIZED_NETWORK_CONTRACT_ID: &str = "psionic.decentralized_network_contract.v1";
/// Stable fixture path for the canonical decentralized network contract.
pub const DECENTRALIZED_NETWORK_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/decentralized_network_contract_v1.json";
/// Stable checker path for the canonical decentralized network contract.
pub const DECENTRALIZED_NETWORK_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-decentralized-network-contract.sh";
/// Stable focused reference doc path.
pub const DECENTRALIZED_NETWORK_CONTRACT_DOC_PATH: &str =
    "docs/DECENTRALIZED_NETWORK_CONTRACT_REFERENCE.md";
/// Stable train-system doc path.
pub const DECENTRALIZED_NETWORK_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";
/// Stable network id frozen by the first contract.
pub const DECENTRALIZED_NETWORK_ID: &str = "psionic.decentralized_training.testnet.v1";
/// Stable governance revision id frozen by the first contract.
pub const DECENTRALIZED_NETWORK_GOVERNANCE_REVISION_ID: &str =
    "psionic.decentralized_training_governance.v1";
/// Stable settlement namespace frozen by the first contract.
pub const DECENTRALIZED_NETWORK_SETTLEMENT_NAMESPACE: &str =
    "psionic.decentralized_training.settlement.v1";

/// Errors surfaced while building, validating, or writing the contract.
#[derive(Debug, Error)]
pub enum DecentralizedNetworkContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    ProgramRunGraph(#[from] CrossProviderProgramRunGraphError),
    #[error(transparent)]
    ValidatorPromotion(#[from] SharedValidatorPromotionContractError),
    #[error("decentralized network contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Public-network role class frozen by the first contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedNetworkRoleClass {
    /// Public miner that performs local assigned training work.
    PublicMiner,
    /// Public validator that scores or replays miner work.
    PublicValidator,
    /// Relay or overlay support service for peer transport.
    Relay,
    /// Checkpoint authority responsible for durable admitted state.
    CheckpointAuthority,
    /// Aggregator for public network artifact or score flow.
    Aggregator,
}

/// How one public role binds back to the existing Psionic run graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedNetworkRoleBindingKind {
    /// Role maps directly onto an existing execution class and run-graph role.
    DirectExecutionBinding,
    /// Role exists as network support only in the current contract.
    NetworkOnlySupportRole,
}

/// Registration posture frozen by the current governance revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedNetworkRegistrationMode {
    /// Maintainer-controlled testnet or allowlist posture.
    PermissionedTestnet,
}

/// Epoch cadence kind frozen by the first contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedNetworkEpochCadenceKind {
    /// Fixed-length public windows.
    FixedWindowCadence,
}

/// Settlement backend kind frozen by the first contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedSettlementBackendKind {
    /// Signed ledger bundles exported off-chain.
    SignedLedgerBundle,
}

/// Checkpoint promotion mode frozen by the first contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecentralizedCheckpointPromotionMode {
    /// Multiple validators must agree before checkpoint promotion.
    MultiValidatorQuorum,
}

/// One typed binding between a public role and the current run-graph vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkRoleBinding {
    /// Public-network role class.
    pub role_class: DecentralizedNetworkRoleClass,
    /// How the role binds to the current Psionic vocabulary.
    pub binding_kind: DecentralizedNetworkRoleBindingKind,
    /// Existing execution class when a direct binding exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_class: Option<CrossProviderExecutionClass>,
    /// Existing run-graph role when a direct binding exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_graph_role: Option<TrainingParticipantRole>,
    /// Honest detail for the current binding.
    pub detail: String,
}

/// Governance revision frozen by the current decentralized network contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkGovernanceRevision {
    /// Stable governance revision id.
    pub governance_revision_id: String,
    /// Monotonic revision number.
    pub governance_revision_number: u32,
    /// Current public registration posture.
    pub registration_mode: DecentralizedNetworkRegistrationMode,
    /// Checkpoint promotion policy.
    pub checkpoint_promotion_mode: DecentralizedCheckpointPromotionMode,
    /// Minimum validator quorum required by the current policy.
    pub minimum_validator_quorum: u16,
    /// Honest detail for the current revision.
    pub detail: String,
}

/// Public epoch or window cadence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkEpochCadence {
    /// Cadence kind.
    pub cadence_kind: DecentralizedNetworkEpochCadenceKind,
    /// Epoch id template.
    pub epoch_id_template: String,
    /// Network epoch zero in unix milliseconds.
    pub epoch_zero_unix_ms: u64,
    /// Epoch duration in seconds.
    pub epoch_duration_seconds: u32,
    /// Heartbeat interval in seconds.
    pub heartbeat_interval_seconds: u16,
    /// Stale-peer timeout in seconds.
    pub stale_peer_timeout_seconds: u16,
    /// Checkpoint barrier cadence in epochs.
    pub checkpoint_barrier_every_epochs: u16,
    /// Settlement publication cadence in epochs.
    pub settlement_publish_every_epochs: u16,
}

/// Settlement backend posture frozen by the contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkSettlementBackend {
    /// Settlement backend kind.
    pub backend_kind: DecentralizedSettlementBackendKind,
    /// Stable settlement namespace.
    pub settlement_namespace: String,
    /// Ledger export template.
    pub ledger_export_template: String,
    /// Payout export template.
    pub payout_export_template: String,
    /// Honest detail for the current settlement posture.
    pub detail: String,
}

/// Checkpoint authority posture frozen by the contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkCheckpointAuthorityPolicy {
    /// Promotion mode.
    pub promotion_mode: DecentralizedCheckpointPromotionMode,
    /// Minimum admitted checkpoint authorities.
    pub minimum_checkpoint_authorities: u16,
    /// Minimum validators required to promote new checkpoint state.
    pub minimum_validator_quorum: u16,
    /// Whether promoted shards must be individually signed.
    pub requires_shard_signature: bool,
    /// Public roles currently allowed to participate in checkpoint authority.
    pub admitted_authority_roles: Vec<DecentralizedNetworkRoleClass>,
    /// Honest detail for the current authority posture.
    pub detail: String,
}

/// Repo-authoritative paths for the contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkAuthorityPaths {
    /// Fixture path.
    pub fixture_path: String,
    /// Checker path.
    pub check_script_path: String,
    /// Focused reference doc path.
    pub reference_doc_path: String,
    /// Train-system doc path.
    pub train_system_doc_path: String,
}

/// Full machine-legible decentralized network contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecentralizedNetworkContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable contract id.
    pub contract_id: String,
    /// Stable network id.
    pub network_id: String,
    /// Bound root training-program manifest id.
    pub program_manifest_id: String,
    /// Bound root training-program manifest digest.
    pub program_manifest_digest: String,
    /// Bound whole-program run-graph schema version.
    pub run_graph_schema_version: String,
    /// Bound whole-program run-graph digest.
    pub run_graph_digest: String,
    /// Bound shared validator and promotion contract id.
    pub validator_promotion_contract_id: String,
    /// Governance revision.
    pub governance_revision: DecentralizedNetworkGovernanceRevision,
    /// Epoch cadence.
    pub epoch_cadence: DecentralizedNetworkEpochCadence,
    /// Settlement backend posture.
    pub settlement_backend: DecentralizedNetworkSettlementBackend,
    /// Checkpoint authority policy.
    pub checkpoint_authority_policy: DecentralizedNetworkCheckpointAuthorityPolicy,
    /// Public role bindings.
    pub role_bindings: Vec<DecentralizedNetworkRoleBinding>,
    /// Repo-authoritative paths.
    pub authority_paths: DecentralizedNetworkAuthorityPaths,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl DecentralizedNetworkContract {
    /// Returns the stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_decentralized_network_contract|", &clone)
    }

    /// Validates contract invariants and current bindings.
    pub fn validate(&self) -> Result<(), DecentralizedNetworkContractError> {
        let manifest = cross_provider_training_program_manifest()?;
        let run_graph = canonical_cross_provider_program_run_graph()?;
        let validator_contract = canonical_shared_validator_promotion_contract()?;

        if self.schema_version != DECENTRALIZED_NETWORK_CONTRACT_SCHEMA_VERSION {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    DECENTRALIZED_NETWORK_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != DECENTRALIZED_NETWORK_CONTRACT_ID {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.network_id != DECENTRALIZED_NETWORK_ID {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("network_id drifted"),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.run_graph_schema_version != crate::CROSS_PROVIDER_PROGRAM_RUN_GRAPH_SCHEMA_VERSION {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("run_graph_schema_version drifted"),
            });
        }
        if self.run_graph_digest != run_graph.contract_digest {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("run_graph_digest drifted"),
            });
        }
        if self.validator_promotion_contract_id != validator_contract.contract_id {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("validator_promotion_contract_id drifted"),
            });
        }
        if self.governance_revision.governance_revision_id
            != DECENTRALIZED_NETWORK_GOVERNANCE_REVISION_ID
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("governance_revision_id drifted"),
            });
        }
        if self.governance_revision.governance_revision_number != 1 {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("governance_revision_number must stay `1`"),
            });
        }
        if self.governance_revision.registration_mode
            != DecentralizedNetworkRegistrationMode::PermissionedTestnet
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("registration_mode drifted from permissioned_testnet"),
            });
        }
        if self.governance_revision.checkpoint_promotion_mode
            != DecentralizedCheckpointPromotionMode::MultiValidatorQuorum
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("checkpoint_promotion_mode drifted"),
            });
        }
        if self.governance_revision.minimum_validator_quorum < 2 {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("minimum validator quorum must stay at least `2`"),
            });
        }
        if self.epoch_cadence.cadence_kind
            != DecentralizedNetworkEpochCadenceKind::FixedWindowCadence
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("epoch cadence kind drifted"),
            });
        }
        if self.epoch_cadence.epoch_duration_seconds == 0
            || self.epoch_cadence.heartbeat_interval_seconds == 0
            || self.epoch_cadence.stale_peer_timeout_seconds
                <= self.epoch_cadence.heartbeat_interval_seconds
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("epoch cadence timings are invalid"),
            });
        }
        if self.epoch_cadence.checkpoint_barrier_every_epochs == 0
            || self.epoch_cadence.settlement_publish_every_epochs == 0
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("epoch cadence barrier counts must stay non-zero"),
            });
        }
        if self.settlement_backend.backend_kind
            != DecentralizedSettlementBackendKind::SignedLedgerBundle
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("settlement backend kind drifted"),
            });
        }
        if self.settlement_backend.settlement_namespace
            != DECENTRALIZED_NETWORK_SETTLEMENT_NAMESPACE
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("settlement namespace drifted"),
            });
        }
        if self.checkpoint_authority_policy.promotion_mode
            != DecentralizedCheckpointPromotionMode::MultiValidatorQuorum
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("checkpoint authority promotion mode drifted"),
            });
        }
        if self
            .checkpoint_authority_policy
            .minimum_checkpoint_authorities
            == 0
            || self.checkpoint_authority_policy.minimum_validator_quorum
                < self.governance_revision.minimum_validator_quorum
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("checkpoint authority quorum posture is invalid"),
            });
        }

        let admitted_authority_roles = self
            .checkpoint_authority_policy
            .admitted_authority_roles
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let expected_authority_roles = BTreeSet::from([
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            DecentralizedNetworkRoleClass::PublicValidator,
        ]);
        if admitted_authority_roles != expected_authority_roles {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("checkpoint authority role set drifted"),
            });
        }

        let expected_roles = BTreeSet::from([
            DecentralizedNetworkRoleClass::PublicMiner,
            DecentralizedNetworkRoleClass::PublicValidator,
            DecentralizedNetworkRoleClass::Relay,
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            DecentralizedNetworkRoleClass::Aggregator,
        ]);
        let actual_roles = self
            .role_bindings
            .iter()
            .map(|binding| binding.role_class)
            .collect::<BTreeSet<_>>();
        if actual_roles != expected_roles {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("role binding set drifted"),
            });
        }

        for binding in &self.role_bindings {
            match binding.binding_kind {
                DecentralizedNetworkRoleBindingKind::DirectExecutionBinding => {
                    let execution_class = binding.execution_class.ok_or_else(|| {
                        DecentralizedNetworkContractError::InvalidContract {
                            detail: format!(
                                "role `{:?}` lost its execution_class binding",
                                binding.role_class
                            ),
                        }
                    })?;
                    let run_graph_role = binding.run_graph_role.ok_or_else(|| {
                        DecentralizedNetworkContractError::InvalidContract {
                            detail: format!(
                                "role `{:?}` lost its run_graph_role binding",
                                binding.role_class
                            ),
                        }
                    })?;
                    let matched_participant = run_graph.participants.iter().any(|participant| {
                        participant.execution_class == execution_class
                            && participant.run_graph_role == run_graph_role
                    });
                    if !matched_participant {
                        return Err(DecentralizedNetworkContractError::InvalidContract {
                            detail: format!(
                                "role `{:?}` no longer resolves against the canonical run graph",
                                binding.role_class
                            ),
                        });
                    }
                }
                DecentralizedNetworkRoleBindingKind::NetworkOnlySupportRole => {
                    if binding.execution_class.is_some() || binding.run_graph_role.is_some() {
                        return Err(DecentralizedNetworkContractError::InvalidContract {
                            detail: format!(
                                "network-only support role `{:?}` must not carry execution or run-graph bindings",
                                binding.role_class
                            ),
                        });
                    }
                }
            }
        }

        if self.authority_paths.fixture_path != DECENTRALIZED_NETWORK_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != DECENTRALIZED_NETWORK_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != DECENTRALIZED_NETWORK_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != DECENTRALIZED_NETWORK_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(DecentralizedNetworkContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical decentralized network contract.
static DECENTRALIZED_NETWORK_CONTRACT_CACHE: std::sync::OnceLock<DecentralizedNetworkContract> =
    std::sync::OnceLock::new();

pub fn canonical_decentralized_network_contract(
) -> Result<DecentralizedNetworkContract, DecentralizedNetworkContractError> {
    if let Some(contract) = DECENTRALIZED_NETWORK_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let run_graph = canonical_cross_provider_program_run_graph()?;
    let validator_contract = canonical_shared_validator_promotion_contract()?;

    let mut contract = DecentralizedNetworkContract {
        schema_version: String::from(DECENTRALIZED_NETWORK_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(DECENTRALIZED_NETWORK_CONTRACT_ID),
        network_id: String::from(DECENTRALIZED_NETWORK_ID),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        run_graph_schema_version: String::from(crate::CROSS_PROVIDER_PROGRAM_RUN_GRAPH_SCHEMA_VERSION),
        run_graph_digest: run_graph.contract_digest.clone(),
        validator_promotion_contract_id: validator_contract.contract_id.clone(),
        governance_revision: DecentralizedNetworkGovernanceRevision {
            governance_revision_id: String::from(DECENTRALIZED_NETWORK_GOVERNANCE_REVISION_ID),
            governance_revision_number: 1,
            registration_mode: DecentralizedNetworkRegistrationMode::PermissionedTestnet,
            checkpoint_promotion_mode: DecentralizedCheckpointPromotionMode::MultiValidatorQuorum,
            minimum_validator_quorum: 2,
            detail: String::from(
                "The first decentralized network contract stays at a permissioned testnet posture. Public roles, epoch truth, validator quorum, checkpoint-promotion policy, and settlement posture are frozen before public registration or reward execution widens further.",
            ),
        },
        epoch_cadence: DecentralizedNetworkEpochCadence {
            cadence_kind: DecentralizedNetworkEpochCadenceKind::FixedWindowCadence,
            epoch_id_template: String::from("network_epoch_${NETWORK_EPOCH}"),
            epoch_zero_unix_ms: 1_711_111_111_000,
            epoch_duration_seconds: 900,
            heartbeat_interval_seconds: 2,
            stale_peer_timeout_seconds: 10,
            checkpoint_barrier_every_epochs: 8,
            settlement_publish_every_epochs: 4,
        },
        settlement_backend: DecentralizedNetworkSettlementBackend {
            backend_kind: DecentralizedSettlementBackendKind::SignedLedgerBundle,
            settlement_namespace: String::from(DECENTRALIZED_NETWORK_SETTLEMENT_NAMESPACE),
            ledger_export_template: String::from(
                "runs/${RUN_ID}/decentralized/ledger/network_epoch_${NETWORK_EPOCH}.json",
            ),
            payout_export_template: String::from(
                "runs/${RUN_ID}/decentralized/payouts/network_epoch_${NETWORK_EPOCH}.json",
            ),
            detail: String::from(
                "The first network contract keeps settlement posture explicit through signed off-chain ledger bundles and payout-ready exports. It does not claim live chain publication or real reward execution yet.",
            ),
        },
        checkpoint_authority_policy: DecentralizedNetworkCheckpointAuthorityPolicy {
            promotion_mode: DecentralizedCheckpointPromotionMode::MultiValidatorQuorum,
            minimum_checkpoint_authorities: 1,
            minimum_validator_quorum: 2,
            requires_shard_signature: true,
            admitted_authority_roles: vec![
                DecentralizedNetworkRoleClass::CheckpointAuthority,
                DecentralizedNetworkRoleClass::PublicValidator,
            ],
            detail: String::from(
                "Checkpoint authority remains explicit. Dedicated checkpoint-authority participants write durable state, and at least two validators must agree before new public checkpoint state is treated as promoted network truth.",
            ),
        },
        role_bindings: vec![
            DecentralizedNetworkRoleBinding {
                role_class: DecentralizedNetworkRoleClass::PublicMiner,
                binding_kind: DecentralizedNetworkRoleBindingKind::DirectExecutionBinding,
                execution_class: Some(CrossProviderExecutionClass::ValidatedContributorWindow),
                run_graph_role: Some(TrainingParticipantRole::ContributorOnly),
                detail: String::from(
                    "The first public miner role binds to the retained validated-contributor execution class. This keeps decentralized miner work attached to the existing contributor-window lineage instead of inventing a second hidden train loop.",
                ),
            },
            DecentralizedNetworkRoleBinding {
                role_class: DecentralizedNetworkRoleClass::PublicValidator,
                binding_kind: DecentralizedNetworkRoleBindingKind::DirectExecutionBinding,
                execution_class: Some(CrossProviderExecutionClass::Validator),
                run_graph_role: Some(TrainingParticipantRole::ContributorOnly),
                detail: String::from(
                    "The first public validator role binds to the retained validator execution class and shared validator-promotion vocabulary already carried by the whole-program run graph.",
                ),
            },
            DecentralizedNetworkRoleBinding {
                role_class: DecentralizedNetworkRoleClass::Relay,
                binding_kind: DecentralizedNetworkRoleBindingKind::NetworkOnlySupportRole,
                execution_class: None,
                run_graph_role: None,
                detail: String::from(
                    "Relay remains a network-only support role in this issue. The contract freezes its existence and governance posture without pretending the current run graph already has a dedicated relay execution class.",
                ),
            },
            DecentralizedNetworkRoleBinding {
                role_class: DecentralizedNetworkRoleClass::CheckpointAuthority,
                binding_kind: DecentralizedNetworkRoleBindingKind::DirectExecutionBinding,
                execution_class: Some(CrossProviderExecutionClass::CheckpointWriter),
                run_graph_role: Some(TrainingParticipantRole::TrainerContributor),
                detail: String::from(
                    "Checkpoint authority binds directly to the retained checkpoint-writer seam in the whole-program run graph so durable promoted state stays attached to an existing machine-legible execution class.",
                ),
            },
            DecentralizedNetworkRoleBinding {
                role_class: DecentralizedNetworkRoleClass::Aggregator,
                binding_kind: DecentralizedNetworkRoleBindingKind::DirectExecutionBinding,
                execution_class: Some(CrossProviderExecutionClass::DataBuilder),
                run_graph_role: Some(TrainingParticipantRole::ContributorOnly),
                detail: String::from(
                    "Aggregator binds to the current data-builder seam as the nearest retained support participant while the decentralized network is still freezing vocabulary. This issue does not yet claim a separate public aggregator execution class.",
                ),
            },
        ],
        authority_paths: DecentralizedNetworkAuthorityPaths {
            fixture_path: String::from(DECENTRALIZED_NETWORK_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(DECENTRALIZED_NETWORK_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(DECENTRALIZED_NETWORK_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(DECENTRALIZED_NETWORK_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract closes one canonical decentralized network epoch, role, governance, settlement, and checkpoint-authority object above the retained cross-provider training-program manifest and whole-program run graph. It does not claim public node registration, public discovery, live public miner or validator runtime, reward execution, or permissionless participation.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = DECENTRALIZED_NETWORK_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

/// Writes the canonical decentralized network contract to disk.
pub fn write_decentralized_network_contract(
    output_path: impl AsRef<Path>,
) -> Result<DecentralizedNetworkContract, DecentralizedNetworkContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DecentralizedNetworkContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_decentralized_network_contract()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| DecentralizedNetworkContractError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(contract)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("decentralized network contract digest serialization must work"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decentralized_network_contract_stays_valid() {
        let contract = canonical_decentralized_network_contract()
            .expect("decentralized network contract should build");
        contract.validate().expect("contract should validate");
    }

    #[test]
    fn decentralized_network_contract_covers_expected_public_roles() {
        let contract = canonical_decentralized_network_contract()
            .expect("decentralized network contract should build");
        let expected = BTreeSet::from([
            DecentralizedNetworkRoleClass::PublicMiner,
            DecentralizedNetworkRoleClass::PublicValidator,
            DecentralizedNetworkRoleClass::Relay,
            DecentralizedNetworkRoleClass::CheckpointAuthority,
            DecentralizedNetworkRoleClass::Aggregator,
        ]);
        let actual = contract
            .role_bindings
            .iter()
            .map(|binding| binding.role_class)
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn decentralized_network_contract_binds_existing_program_truth() {
        let contract = canonical_decentralized_network_contract()
            .expect("decentralized network contract should build");
        let manifest = cross_provider_training_program_manifest()
            .expect("program manifest should build successfully");
        let run_graph = canonical_cross_provider_program_run_graph()
            .expect("run graph should build successfully");
        let validator_contract = canonical_shared_validator_promotion_contract()
            .expect("validator contract should build successfully");

        assert_eq!(contract.program_manifest_id, manifest.program_manifest_id);
        assert_eq!(
            contract.program_manifest_digest,
            manifest.program_manifest_digest
        );
        assert_eq!(contract.run_graph_digest, run_graph.contract_digest);
        assert_eq!(
            contract.validator_promotion_contract_id,
            validator_contract.contract_id
        );
    }
}
