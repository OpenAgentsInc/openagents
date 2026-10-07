use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_decentralized_network_contract, canonical_elastic_device_mesh_contract,
    canonical_public_network_registry_contract, CrossProviderExecutionClass,
    DecentralizedNetworkContractError, DecentralizedNetworkRoleClass,
    ElasticDeviceMeshContractError, ElasticMeshLeaseStatus, PublicNetworkRegistryContractError,
    PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID,
};

pub const PUBLIC_WORK_ASSIGNMENT_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.public_work_assignment_contract.v1";
pub const PUBLIC_WORK_ASSIGNMENT_CONTRACT_ID: &str = "psionic.public_work_assignment_contract.v1";
pub const PUBLIC_WORK_ASSIGNMENT_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/public_work_assignment_contract_v1.json";
pub const PUBLIC_WORK_ASSIGNMENT_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-public-work-assignment-contract.sh";
pub const PUBLIC_WORK_ASSIGNMENT_CONTRACT_DOC_PATH: &str =
    "docs/PUBLIC_WORK_ASSIGNMENT_REFERENCE.md";
pub const PUBLIC_WORK_ASSIGNMENT_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

#[derive(Debug, Error)]
pub enum PublicWorkAssignmentContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    NetworkContract(#[from] DecentralizedNetworkContractError),
    #[error(transparent)]
    PublicRegistry(#[from] PublicNetworkRegistryContractError),
    #[error(transparent)]
    ElasticMesh(#[from] ElasticDeviceMeshContractError),
    #[error("public work assignment contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicWorkAssignmentKind {
    PublicMinerTrain,
    PublicValidatorChallenge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicWorkAssignmentRefusalKind {
    WindowClosed,
    LateWindowReplayRefused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicTrainingWindow {
    pub window_id: String,
    pub window_number: u64,
    pub assignment_seed: String,
    pub opens_at_unix_ms: u64,
    pub closes_at_unix_ms: u64,
    pub checkpoint_authority_registry_record_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicWorkAssignment {
    pub assignment_id: String,
    pub assignment_kind: PublicWorkAssignmentKind,
    pub window_id: String,
    pub registry_record_id: String,
    pub allowed_execution_class: CrossProviderExecutionClass,
    pub dataset_page_selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub challenged_assignment_id: Option<String>,
    pub checkpoint_authority_registry_record_id: String,
    pub planned_local_steps: u16,
    pub assignment_seed_material: String,
    pub detail: String,
}

impl PublicWorkAssignment {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_public_work_assignment|", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicWorkAssignmentReceipt {
    pub receipt_id: String,
    pub assignment_id: String,
    pub selection_source_id: String,
    pub selection_ordinal: u16,
    pub assignment_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicWorkLateWindowRefusal {
    pub refusal_id: String,
    pub assignment_id: String,
    pub refusal_kind: PublicWorkAssignmentRefusalKind,
    pub attempted_at_unix_ms: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicWorkAssignmentAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicWorkAssignmentContract {
    pub schema_version: String,
    pub contract_id: String,
    pub network_id: String,
    pub governance_revision_id: String,
    pub current_epoch_id: String,
    pub decentralized_network_contract_digest: String,
    pub public_network_registry_contract_digest: String,
    pub elastic_device_mesh_contract_digest: String,
    pub windows: Vec<PublicTrainingWindow>,
    pub assignments: Vec<PublicWorkAssignment>,
    pub assignment_receipts: Vec<PublicWorkAssignmentReceipt>,
    pub late_window_refusals: Vec<PublicWorkLateWindowRefusal>,
    pub authority_paths: PublicWorkAssignmentAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl PublicWorkAssignmentContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_public_work_assignment_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), PublicWorkAssignmentContractError> {
        let network = canonical_decentralized_network_contract()?;
        let registry = canonical_public_network_registry_contract()?;
        let mesh = canonical_elastic_device_mesh_contract()?;

        let active_role_pairs = mesh
            .member_leases
            .iter()
            .filter(|lease| lease.status == ElasticMeshLeaseStatus::Active)
            .map(|lease| (lease.registry_record_id.as_str(), lease.role_class))
            .collect::<BTreeSet<_>>();
        let window_by_id = self
            .windows
            .iter()
            .map(|window| (window.window_id.as_str(), window))
            .collect::<BTreeMap<_, _>>();
        let assignment_by_id = self
            .assignments
            .iter()
            .map(|assignment| (assignment.assignment_id.as_str(), assignment))
            .collect::<BTreeMap<_, _>>();
        let record_ids = registry
            .registry_records
            .iter()
            .map(|record| record.registry_record_id.as_str())
            .collect::<BTreeSet<_>>();
        let valid_selection_sources =
            BTreeSet::from(["public_miner_window_offer_v1", "validator_quorum_offer_v1"]);

        if self.schema_version != PUBLIC_WORK_ASSIGNMENT_CONTRACT_SCHEMA_VERSION {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    PUBLIC_WORK_ASSIGNMENT_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != PUBLIC_WORK_ASSIGNMENT_CONTRACT_ID {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.network_id != network.network_id {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("network_id drifted"),
            });
        }
        if self.governance_revision_id != network.governance_revision.governance_revision_id {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("governance revision drifted"),
            });
        }
        if self.current_epoch_id != PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("current_epoch_id drifted"),
            });
        }
        if self.decentralized_network_contract_digest != network.contract_digest
            || self.public_network_registry_contract_digest != registry.contract_digest
            || self.elastic_device_mesh_contract_digest != mesh.contract_digest
        {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("upstream contract digest drifted"),
            });
        }
        if self.authority_paths.fixture_path != PUBLIC_WORK_ASSIGNMENT_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != PUBLIC_WORK_ASSIGNMENT_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != PUBLIC_WORK_ASSIGNMENT_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != PUBLIC_WORK_ASSIGNMENT_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        if self.windows.len() != 2 {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("expected exactly two canonical public windows"),
            });
        }
        let mut last_close = 0_u64;
        let mut window_numbers = BTreeSet::new();
        for window in &self.windows {
            if !window_numbers.insert(window.window_number) {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!("duplicate window_number `{}`", window.window_number),
                });
            }
            if window.opens_at_unix_ms >= window.closes_at_unix_ms
                || window.opens_at_unix_ms < last_close
            {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!("window `{}` timing drifted", window.window_id),
                });
            }
            last_close = window.closes_at_unix_ms;
            for checkpoint_authority_registry_record_id in
                &window.checkpoint_authority_registry_record_ids
            {
                if !active_role_pairs.contains(&(
                    checkpoint_authority_registry_record_id.as_str(),
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                )) {
                    return Err(PublicWorkAssignmentContractError::InvalidContract {
                        detail: format!(
                            "window `{}` lost checkpoint authority `{}`",
                            window.window_id, checkpoint_authority_registry_record_id
                        ),
                    });
                }
            }
        }

        let mut assignment_ids = BTreeSet::new();
        let mut per_window_miner_count = BTreeMap::<&str, u16>::new();
        let mut per_window_validator_count = BTreeMap::<&str, u16>::new();
        for assignment in &self.assignments {
            if !assignment_ids.insert(assignment.assignment_id.as_str()) {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!("duplicate assignment `{}`", assignment.assignment_id),
                });
            }
            let window = window_by_id
                .get(assignment.window_id.as_str())
                .ok_or_else(|| PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "assignment `{}` references unknown window `{}`",
                        assignment.assignment_id, assignment.window_id
                    ),
                })?;
            if !record_ids.contains(assignment.registry_record_id.as_str())
                || !active_role_pairs.contains(&(
                    assignment.checkpoint_authority_registry_record_id.as_str(),
                    DecentralizedNetworkRoleClass::CheckpointAuthority,
                ))
                || !window
                    .checkpoint_authority_registry_record_ids
                    .contains(&assignment.checkpoint_authority_registry_record_id)
            {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "assignment `{}` lost participant or checkpoint-authority binding",
                        assignment.assignment_id
                    ),
                });
            }
            if assignment.planned_local_steps == 0 {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "assignment `{}` lost planned_local_steps",
                        assignment.assignment_id
                    ),
                });
            }
            match assignment.assignment_kind {
                PublicWorkAssignmentKind::PublicMinerTrain => {
                    if assignment.allowed_execution_class
                        != CrossProviderExecutionClass::ValidatedContributorWindow
                        || !active_role_pairs.contains(&(
                            assignment.registry_record_id.as_str(),
                            DecentralizedNetworkRoleClass::PublicMiner,
                        ))
                        || assignment.challenged_assignment_id.is_some()
                    {
                        return Err(PublicWorkAssignmentContractError::InvalidContract {
                            detail: format!(
                                "public miner assignment `{}` drifted",
                                assignment.assignment_id
                            ),
                        });
                    }
                    *per_window_miner_count
                        .entry(assignment.window_id.as_str())
                        .or_default() += 1;
                }
                PublicWorkAssignmentKind::PublicValidatorChallenge => {
                    let challenged_assignment_id =
                        assignment
                            .challenged_assignment_id
                            .as_deref()
                            .ok_or_else(|| PublicWorkAssignmentContractError::InvalidContract {
                                detail: format!(
                                    "validator assignment `{}` lost challenge target",
                                    assignment.assignment_id
                                ),
                            })?;
                    let challenged_assignment =
                        assignment_by_id.get(challenged_assignment_id).ok_or_else(|| {
                            PublicWorkAssignmentContractError::InvalidContract {
                                detail: format!(
                                    "validator assignment `{}` references unknown challenged assignment `{}`",
                                    assignment.assignment_id, challenged_assignment_id
                                ),
                            }
                        })?;
                    if assignment.allowed_execution_class != CrossProviderExecutionClass::Validator
                        || !active_role_pairs.contains(&(
                            assignment.registry_record_id.as_str(),
                            DecentralizedNetworkRoleClass::PublicValidator,
                        ))
                        || challenged_assignment.assignment_kind
                            != PublicWorkAssignmentKind::PublicMinerTrain
                        || challenged_assignment.window_id != assignment.window_id
                        || challenged_assignment.registry_record_id == assignment.registry_record_id
                    {
                        return Err(PublicWorkAssignmentContractError::InvalidContract {
                            detail: format!(
                                "validator assignment `{}` drifted",
                                assignment.assignment_id
                            ),
                        });
                    }
                    *per_window_validator_count
                        .entry(assignment.window_id.as_str())
                        .or_default() += 1;
                }
            }
        }
        for window in &self.windows {
            if per_window_miner_count
                .get(window.window_id.as_str())
                .copied()
                .unwrap_or_default()
                != 2
                || per_window_validator_count
                    .get(window.window_id.as_str())
                    .copied()
                    .unwrap_or_default()
                    != 2
            {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "window `{}` must retain two miner and two validator assignments",
                        window.window_id
                    ),
                });
            }
        }

        let mut receipt_ids = BTreeSet::new();
        for receipt in &self.assignment_receipts {
            if !receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!("duplicate receipt `{}`", receipt.receipt_id),
                });
            }
            let assignment = assignment_by_id
                .get(receipt.assignment_id.as_str())
                .ok_or_else(|| PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "receipt `{}` references unknown assignment `{}`",
                        receipt.receipt_id, receipt.assignment_id
                    ),
                })?;
            if !valid_selection_sources.contains(receipt.selection_source_id.as_str())
                || receipt.selection_ordinal == 0
                || receipt.assignment_digest != assignment.stable_digest()
            {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "receipt `{}` lost deterministic assignment binding",
                        receipt.receipt_id
                    ),
                });
            }
            match assignment.assignment_kind {
                PublicWorkAssignmentKind::PublicMinerTrain => {
                    if receipt.selection_source_id != "public_miner_window_offer_v1" {
                        return Err(PublicWorkAssignmentContractError::InvalidContract {
                            detail: format!(
                                "miner receipt `{}` drifted from public_miner_window_offer_v1",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                PublicWorkAssignmentKind::PublicValidatorChallenge => {
                    if receipt.selection_source_id != "validator_quorum_offer_v1" {
                        return Err(PublicWorkAssignmentContractError::InvalidContract {
                            detail: format!(
                                "validator receipt `{}` drifted from validator_quorum_offer_v1",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
            }
        }
        if self.assignment_receipts.len() != self.assignments.len() {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("every assignment must retain one receipt"),
            });
        }

        if self.late_window_refusals.len() != 1 {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("expected exactly one late-window refusal"),
            });
        }
        for refusal in &self.late_window_refusals {
            let assignment = assignment_by_id
                .get(refusal.assignment_id.as_str())
                .ok_or_else(|| PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "late-window refusal `{}` references unknown assignment `{}`",
                        refusal.refusal_id, refusal.assignment_id
                    ),
                })?;
            let window = window_by_id
                .get(assignment.window_id.as_str())
                .ok_or_else(|| PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "late-window refusal `{}` lost window `{}`",
                        refusal.refusal_id, assignment.window_id
                    ),
                })?;
            if refusal.attempted_at_unix_ms <= window.closes_at_unix_ms {
                return Err(PublicWorkAssignmentContractError::InvalidContract {
                    detail: format!(
                        "late-window refusal `{}` must happen after the window closed",
                        refusal.refusal_id
                    ),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(PublicWorkAssignmentContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

static PUBLIC_WORK_ASSIGNMENT_CONTRACT_CACHE: std::sync::OnceLock<PublicWorkAssignmentContract> =
    std::sync::OnceLock::new();

pub fn canonical_public_work_assignment_contract(
) -> Result<PublicWorkAssignmentContract, PublicWorkAssignmentContractError> {
    if let Some(contract) = PUBLIC_WORK_ASSIGNMENT_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let network = canonical_decentralized_network_contract()?;
    let registry = canonical_public_network_registry_contract()?;
    let mesh = canonical_elastic_device_mesh_contract()?;

    let windows = vec![
        PublicTrainingWindow {
            window_id: String::from("window.public.1230"),
            window_number: 1_230,
            assignment_seed: String::from("seed.window.1230.public"),
            opens_at_unix_ms: 1_711_111_220_000,
            closes_at_unix_ms: 1_711_111_280_000,
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The first public work window binds one deterministic seed, one active miner pair, one active validator pair, and the current checkpoint-authority pair.",
            ),
        },
        PublicTrainingWindow {
            window_id: String::from("window.public.1231"),
            window_number: 1_231,
            assignment_seed: String::from("seed.window.1231.public"),
            opens_at_unix_ms: 1_711_111_281_000,
            closes_at_unix_ms: 1_711_111_341_000,
            checkpoint_authority_registry_record_ids: vec![
                String::from("google_l4_validator_node.registry"),
                String::from("runpod_8xh100_dense_node.registry"),
            ],
            detail: String::from(
                "The second public work window advances the clock but keeps the same checkpoint-authority pair, so the network-time and assignment-time boundary remains machine legible.",
            ),
        },
    ];

    let assignments = vec![
        miner_assignment(
            "assignment.public_miner.window1230.google",
            "window.public.1230",
            "google_l4_validator_node.registry",
            "dataset.page.train.0001_0004",
            "runpod_8xh100_dense_node.registry",
            64,
            "seed.window.1230.public|miner|google|ordinal1",
            "Google receives the first public miner page slice in window 1230.",
        ),
        miner_assignment(
            "assignment.public_miner.window1230.local_mlx",
            "window.public.1230",
            "local_mlx_mac_workstation.registry",
            "dataset.page.train.0005_0008",
            "runpod_8xh100_dense_node.registry",
            64,
            "seed.window.1230.public|miner|local_mlx|ordinal2",
            "Apple MLX receives the second public miner page slice in window 1230 after the standby promotion closed.",
        ),
        validator_assignment(
            "assignment.public_validator.window1230.google",
            "window.public.1230",
            "google_l4_validator_node.registry",
            "assignment.public_miner.window1230.local_mlx",
            "dataset.page.validation.challenge.0001",
            "google_l4_validator_node.registry",
            16,
            "seed.window.1230.public|validator|google|ordinal1",
            "Google validates the Apple MLX miner slice for window 1230.",
        ),
        validator_assignment(
            "assignment.public_validator.window1230.local_mlx",
            "window.public.1230",
            "local_mlx_mac_workstation.registry",
            "assignment.public_miner.window1230.google",
            "dataset.page.validation.challenge.0002",
            "google_l4_validator_node.registry",
            16,
            "seed.window.1230.public|validator|local_mlx|ordinal2",
            "Apple MLX validates the Google miner slice for window 1230.",
        ),
        miner_assignment(
            "assignment.public_miner.window1231.google",
            "window.public.1231",
            "google_l4_validator_node.registry",
            "dataset.page.train.0009_0012",
            "runpod_8xh100_dense_node.registry",
            64,
            "seed.window.1231.public|miner|google|ordinal1",
            "Google receives the first public miner page slice in window 1231.",
        ),
        miner_assignment(
            "assignment.public_miner.window1231.local_mlx",
            "window.public.1231",
            "local_mlx_mac_workstation.registry",
            "dataset.page.train.0013_0016",
            "runpod_8xh100_dense_node.registry",
            64,
            "seed.window.1231.public|miner|local_mlx|ordinal2",
            "Apple MLX receives the second public miner page slice in window 1231.",
        ),
        validator_assignment(
            "assignment.public_validator.window1231.google",
            "window.public.1231",
            "google_l4_validator_node.registry",
            "assignment.public_miner.window1231.local_mlx",
            "dataset.page.validation.challenge.0003",
            "google_l4_validator_node.registry",
            16,
            "seed.window.1231.public|validator|google|ordinal1",
            "Google validates the Apple MLX miner slice for window 1231.",
        ),
        validator_assignment(
            "assignment.public_validator.window1231.local_mlx",
            "window.public.1231",
            "local_mlx_mac_workstation.registry",
            "assignment.public_miner.window1231.google",
            "dataset.page.validation.challenge.0004",
            "google_l4_validator_node.registry",
            16,
            "seed.window.1231.public|validator|local_mlx|ordinal2",
            "Apple MLX validates the Google miner slice for window 1231.",
        ),
    ];

    let assignment_receipts = assignments
        .iter()
        .enumerate()
        .map(|(index, assignment)| PublicWorkAssignmentReceipt {
            receipt_id: format!("receipt.{}", assignment.assignment_id),
            assignment_id: assignment.assignment_id.clone(),
            selection_source_id: match assignment.assignment_kind {
                PublicWorkAssignmentKind::PublicMinerTrain => {
                    String::from("public_miner_window_offer_v1")
                }
                PublicWorkAssignmentKind::PublicValidatorChallenge => {
                    String::from("validator_quorum_offer_v1")
                }
            },
            selection_ordinal: (index as u16 % 2) + 1,
            assignment_digest: assignment.stable_digest(),
            detail: format!(
                "The network retains the deterministic selection receipt for `{}`.",
                assignment.assignment_id
            ),
        })
        .collect::<Vec<_>>();

    let late_window_refusals = vec![PublicWorkLateWindowRefusal {
        refusal_id: String::from("late_refusal.window1230.google.replay"),
        assignment_id: String::from("assignment.public_miner.window1230.google"),
        refusal_kind: PublicWorkAssignmentRefusalKind::LateWindowReplayRefused,
        attempted_at_unix_ms: 1_711_111_286_000,
        detail: String::from(
            "A post-close replay attempt against the first Google miner assignment is refused because the assignment belongs to the already sealed window 1230.",
        ),
    }];

    let mut contract = PublicWorkAssignmentContract {
        schema_version: String::from(PUBLIC_WORK_ASSIGNMENT_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PUBLIC_WORK_ASSIGNMENT_CONTRACT_ID),
        network_id: network.network_id.clone(),
        governance_revision_id: network.governance_revision.governance_revision_id.clone(),
        current_epoch_id: String::from(PUBLIC_NETWORK_REGISTRY_CURRENT_EPOCH_ID),
        decentralized_network_contract_digest: network.contract_digest.clone(),
        public_network_registry_contract_digest: registry.contract_digest.clone(),
        elastic_device_mesh_contract_digest: mesh.contract_digest.clone(),
        windows,
        assignments,
        assignment_receipts,
        late_window_refusals,
        authority_paths: PublicWorkAssignmentAuthorityPaths {
            fixture_path: String::from(PUBLIC_WORK_ASSIGNMENT_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PUBLIC_WORK_ASSIGNMENT_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(PUBLIC_WORK_ASSIGNMENT_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(PUBLIC_WORK_ASSIGNMENT_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first deterministic public-work surface: public window clocks, miner and validator assignments, assignment receipts, and one late-window refusal. It does not yet claim page-proofed data authority, content-addressed artifact exchange, or the full public miner execution protocol.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = PUBLIC_WORK_ASSIGNMENT_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_public_work_assignment_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), PublicWorkAssignmentContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PublicWorkAssignmentContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_public_work_assignment_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| PublicWorkAssignmentContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn miner_assignment(
    assignment_id: &str,
    window_id: &str,
    registry_record_id: &str,
    dataset_page_selector: &str,
    checkpoint_authority_registry_record_id: &str,
    planned_local_steps: u16,
    assignment_seed_material: &str,
    detail: &str,
) -> PublicWorkAssignment {
    PublicWorkAssignment {
        assignment_id: String::from(assignment_id),
        assignment_kind: PublicWorkAssignmentKind::PublicMinerTrain,
        window_id: String::from(window_id),
        registry_record_id: String::from(registry_record_id),
        allowed_execution_class: CrossProviderExecutionClass::ValidatedContributorWindow,
        dataset_page_selector: String::from(dataset_page_selector),
        challenged_assignment_id: None,
        checkpoint_authority_registry_record_id: String::from(
            checkpoint_authority_registry_record_id,
        ),
        planned_local_steps,
        assignment_seed_material: String::from(assignment_seed_material),
        detail: String::from(detail),
    }
}

fn validator_assignment(
    assignment_id: &str,
    window_id: &str,
    registry_record_id: &str,
    challenged_assignment_id: &str,
    dataset_page_selector: &str,
    checkpoint_authority_registry_record_id: &str,
    planned_local_steps: u16,
    assignment_seed_material: &str,
    detail: &str,
) -> PublicWorkAssignment {
    PublicWorkAssignment {
        assignment_id: String::from(assignment_id),
        assignment_kind: PublicWorkAssignmentKind::PublicValidatorChallenge,
        window_id: String::from(window_id),
        registry_record_id: String::from(registry_record_id),
        allowed_execution_class: CrossProviderExecutionClass::Validator,
        dataset_page_selector: String::from(dataset_page_selector),
        challenged_assignment_id: Some(String::from(challenged_assignment_id)),
        checkpoint_authority_registry_record_id: String::from(
            checkpoint_authority_registry_record_id,
        ),
        planned_local_steps,
        assignment_seed_material: String::from(assignment_seed_material),
        detail: String::from(detail),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher
        .update(serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for public work assignment contract",
        ));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_public_work_assignment_contract, PublicWorkAssignmentContractError,
        PublicWorkAssignmentKind,
    };

    #[test]
    fn canonical_public_work_assignment_contract_is_valid(
    ) -> Result<(), PublicWorkAssignmentContractError> {
        let contract = canonical_public_work_assignment_contract()?;
        contract.validate()
    }

    #[test]
    fn validator_assignments_must_target_miner_assignments(
    ) -> Result<(), PublicWorkAssignmentContractError> {
        let mut contract = canonical_public_work_assignment_contract()?;
        let validator_assignment = contract
            .assignments
            .iter_mut()
            .find(|assignment| {
                assignment.assignment_kind == PublicWorkAssignmentKind::PublicValidatorChallenge
            })
            .expect("canonical contract should contain validator assignments");
        validator_assignment.challenged_assignment_id = Some(String::from(
            "assignment.public_validator.window1230.local_mlx",
        ));
        let error = contract
            .validate()
            .expect_err("validator assignments must target miner assignments");
        assert!(matches!(
            error,
            PublicWorkAssignmentContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
