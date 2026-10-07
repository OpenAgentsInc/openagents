use std::{fs, path::Path};

use psionic_data::{DatasetKey, DatasetSplitKind, TokenizerDigest, TokenizerFamily};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    OPEN_ADAPTER_CUDA_BACKEND_LABEL, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
    OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY, OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT,
};
/// Stable contract identifier for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_CONTRACT_ID: &str = "swarm.local_open_adapter_contract.v1";
/// Stable scope window for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_SCOPE_WINDOW: &str = "swarm_local_open_adapter_contract_v1";
/// Stable run family identifier for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_FAMILY_ID: &str = "swarm.local.mlx_metal_plus_rtx4080.open_adapter.v1";
/// Stable cluster namespace for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_CLUSTER_NAMESPACE: &str = "cluster.swarm.local.trusted_lan";
/// Stable admission posture for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_ADMISSION_POSTURE: &str = "trusted_lan.shared_secret";
/// Stable dataset reference for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_DATASET_REF: &str = "dataset://openagents/swarm/open_adapter_sft";
/// Stable dataset version for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_DATASET_VERSION: &str = "2026.03.24";
/// Stable validator policy for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_VALIDATOR_POLICY_ID: &str = "validator.open_adapter.reference";
/// Stable aggregation policy for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_AGGREGATION_POLICY_ID: &str = "aggregation.open_adapter.mean_delta";
/// Stable replay policy for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_REPLAY_POLICY_ID: &str = "replay.open_adapter.strict";
/// Stable promotion posture for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_PROMOTION_POSTURE: &str = "local_snapshot_only";
/// Stable published outcome posture for the first local mixed-hardware swarm lane.
pub const SWARM_FIRST_RUN_PUBLISH_POSTURE: &str = "no_served_promotion";

/// Errors surfaced while writing the first swarm contract fixture.
#[derive(Debug, Error)]
pub enum FirstSwarmContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to encode the first swarm contract: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Stable node role admitted by the first local mixed-hardware swarm lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmNodeRoleKind {
    /// Mac node that contributes deltas and owns validator plus aggregation duties.
    MacCoordinatorValidatorContributor,
    /// Linux desktop node that contributes CUDA deltas only.
    LinuxCudaContributor,
}

/// Machine-legible contributor, validator, and aggregation gate for one node role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmNodeRoleContract {
    /// Stable node role identifier.
    pub role_id: String,
    /// Stable node role kind.
    pub role_kind: FirstSwarmNodeRoleKind,
    /// Stable host platform label.
    pub platform: String,
    /// Stable backend label expected in cluster telemetry.
    pub backend_label: String,
    /// Whether the node may contribute adapter deltas.
    pub contributor_eligible: bool,
    /// Whether the node may validate contribution uploads.
    pub validator_eligible: bool,
    /// Whether the node may aggregate accepted deltas.
    pub aggregation_eligible: bool,
    /// Minimum free memory expected for the role.
    pub minimum_free_memory_bytes: u64,
    /// Deterministic runtime notes that make the lane boundary explicit.
    pub notes: Vec<String>,
}

/// One frozen dataset split inside the first swarm contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmDatasetSplitContract {
    /// Stable split name.
    pub split_name: String,
    /// Split role.
    pub split_kind: DatasetSplitKind,
    /// Stable sample identifiers expected inside the split.
    pub sample_ids: Vec<String>,
    /// Stable digest over the split contents expected by the swarm lane.
    pub split_digest: String,
}

/// Frozen dataset and tokenizer identity for the first swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmDatasetContract {
    /// Stable dataset key.
    pub dataset_key: DatasetKey,
    /// Stable tokenizer contract.
    pub tokenizer: TokenizerDigest,
    /// Stable dataset storage key.
    pub dataset_storage_key: String,
    /// Frozen dataset splits.
    pub splits: Vec<FirstSwarmDatasetSplitContract>,
    /// Stable dataset-manifest digest.
    pub dataset_manifest_digest: String,
}

/// Replay, validation, aggregation, and publication posture for the first swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmGovernanceContract {
    /// Stable validator policy id.
    pub validator_policy_id: String,
    /// Stable aggregation policy id.
    pub aggregation_policy_id: String,
    /// Stable replay policy id.
    pub replay_policy_id: String,
    /// Stable promotion posture.
    pub promotion_posture: String,
    /// Stable publish posture.
    pub publish_posture: String,
    /// Whether every accepted contribution must carry a replay receipt.
    pub replay_required_per_contribution: bool,
    /// Whether the first lane requires both contributor roles before aggregation.
    pub require_all_contributor_roles: bool,
}

/// Canonical machine-legible contract for the first local mixed-hardware swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmRunContract {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Stable cluster namespace.
    pub cluster_namespace: String,
    /// Stable cluster admission posture.
    pub cluster_admission_posture: String,
    /// Frozen dataset and tokenizer identity.
    pub dataset: FirstSwarmDatasetContract,
    /// Stable adapter family.
    pub adapter_family: String,
    /// Stable adapter artifact format.
    pub adapter_format: String,
    /// Frozen node roles admitted by the lane.
    pub node_roles: Vec<FirstSwarmNodeRoleContract>,
    /// Frozen replay, validator, aggregation, and publish posture.
    pub governance: FirstSwarmGovernanceContract,
    /// Explicit non-goals for the lane.
    pub non_goals: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl FirstSwarmRunContract {
    /// Returns the stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_swarm_digest(b"psionic_first_swarm_run_contract|", &clone)
    }
}

/// Returns the canonical tokenizer digest for the first local mixed-hardware swarm lane.
#[must_use]
pub fn first_swarm_tokenizer_digest() -> TokenizerDigest {
    TokenizerDigest::new(
        TokenizerFamily::BytePairEncoding,
        "swarm-open-adapter-tokenizer-v1",
        32,
    )
    .with_special_tokens_digest("swarm-open-adapter-specials-v1")
    .with_template_digest("swarm-open-adapter-template-v1")
}

/// Returns the canonical dataset contract for the first local mixed-hardware swarm lane.
#[must_use]
pub fn first_swarm_dataset_contract() -> FirstSwarmDatasetContract {
    let dataset_key = DatasetKey::new(SWARM_FIRST_RUN_DATASET_REF, SWARM_FIRST_RUN_DATASET_VERSION);
    let splits = vec![
        FirstSwarmDatasetSplitContract {
            split_name: String::from("train"),
            split_kind: DatasetSplitKind::Train,
            sample_ids: vec![
                String::from("swarm-train-001"),
                String::from("swarm-train-002"),
                String::from("swarm-train-003"),
                String::from("swarm-train-004"),
            ],
            split_digest: stable_split_digest(
                dataset_key.storage_key().as_str(),
                "train",
                &[
                    "swarm-train-001",
                    "swarm-train-002",
                    "swarm-train-003",
                    "swarm-train-004",
                ],
            ),
        },
        FirstSwarmDatasetSplitContract {
            split_name: String::from("validation"),
            split_kind: DatasetSplitKind::Validation,
            sample_ids: vec![String::from("swarm-val-001"), String::from("swarm-val-002")],
            split_digest: stable_split_digest(
                dataset_key.storage_key().as_str(),
                "validation",
                &["swarm-val-001", "swarm-val-002"],
            ),
        },
    ];
    let tokenizer = first_swarm_tokenizer_digest();
    let dataset_manifest_digest = stable_dataset_manifest_digest(
        dataset_key.storage_key().as_str(),
        tokenizer.stable_digest().as_str(),
        splits.as_slice(),
    );
    FirstSwarmDatasetContract {
        dataset_storage_key: dataset_key.storage_key(),
        dataset_key,
        tokenizer,
        splits,
        dataset_manifest_digest,
    }
}

/// Returns the canonical machine-legible contract for the first local mixed-hardware swarm lane.
#[must_use]
pub fn first_swarm_run_contract() -> FirstSwarmRunContract {
    let dataset = first_swarm_dataset_contract();
    let mut contract = FirstSwarmRunContract {
        schema_version: 1,
        contract_id: String::from(SWARM_FIRST_RUN_CONTRACT_ID),
        scope_window: String::from(SWARM_FIRST_RUN_SCOPE_WINDOW),
        run_family_id: String::from(SWARM_FIRST_RUN_FAMILY_ID),
        cluster_namespace: String::from(SWARM_FIRST_RUN_CLUSTER_NAMESPACE),
        cluster_admission_posture: String::from(SWARM_FIRST_RUN_ADMISSION_POSTURE),
        dataset,
        adapter_family: String::from(OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY),
        adapter_format: String::from(OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT),
        node_roles: vec![
            FirstSwarmNodeRoleContract {
                role_id: String::from("swarm.mac.mlx.coordinator_validator_contributor"),
                role_kind: FirstSwarmNodeRoleKind::MacCoordinatorValidatorContributor,
                platform: String::from("macos_apple_silicon"),
                backend_label: String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL),
                contributor_eligible: true,
                validator_eligible: true,
                aggregation_eligible: true,
                minimum_free_memory_bytes: 12 * 1024 * 1024 * 1024,
                notes: vec![
                    String::from(
                        "The Mac node owns the MLX-backed Metal contributor role plus validator and aggregation duties for the first swarm lane.",
                    ),
                    String::from(
                        "This node may publish only a local snapshot when the validator accepts both contributors and replay receipts stay intact.",
                    ),
                ],
            },
            FirstSwarmNodeRoleContract {
                role_id: String::from("swarm.linux.cuda.rtx4080.contributor"),
                role_kind: FirstSwarmNodeRoleKind::LinuxCudaContributor,
                platform: String::from("linux_nvidia_rtx_4080"),
                backend_label: String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL),
                contributor_eligible: true,
                validator_eligible: false,
                aggregation_eligible: false,
                minimum_free_memory_bytes: 10 * 1024 * 1024 * 1024,
                notes: vec![
                    String::from(
                        "The Linux desktop node contributes one CUDA adapter delta on display-attached RTX 4080 hardware.",
                    ),
                    String::from(
                        "This node is not treated as validator or aggregation authority in the first swarm lane.",
                    ),
                ],
            },
        ],
        governance: FirstSwarmGovernanceContract {
            validator_policy_id: String::from(SWARM_FIRST_RUN_VALIDATOR_POLICY_ID),
            aggregation_policy_id: String::from(SWARM_FIRST_RUN_AGGREGATION_POLICY_ID),
            replay_policy_id: String::from(SWARM_FIRST_RUN_REPLAY_POLICY_ID),
            promotion_posture: String::from(SWARM_FIRST_RUN_PROMOTION_POSTURE),
            publish_posture: String::from(SWARM_FIRST_RUN_PUBLISH_POSTURE),
            replay_required_per_contribution: true,
            require_all_contributor_roles: true,
        },
        non_goals: vec![
            String::from("full-model mixed-backend all-reduce or FSDP training"),
            String::from("Apple FM package promotion"),
            String::from("a second trainer architecture outside psionic-train"),
            String::from("implicit publication to serving without validator and replay proof"),
        ],
        claim_boundary: String::from(
            "This contract freezes one bounded decentralized open-adapter delta lane across one Mac MLX Metal contributor and one Linux RTX 4080 CUDA contributor under trusted-LAN admission, explicit validator and replay gates, and local-snapshot-only publication posture. It does not claim one coherent mixed-backend full-model optimizer, full-model gradient exchange, or automatic served promotion.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract
}

/// Writes the canonical first swarm contract to one JSON path.
pub fn write_first_swarm_run_contract(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmRunContract, FirstSwarmContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| FirstSwarmContractError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = first_swarm_run_contract();
    let encoded = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

fn stable_split_digest(dataset_storage_key: &str, split_name: &str, sample_ids: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_first_swarm_split|");
    hasher.update(dataset_storage_key.as_bytes());
    hasher.update(b"|");
    hasher.update(split_name.as_bytes());
    for sample_id in sample_ids {
        hasher.update(b"|sample|");
        hasher.update(sample_id.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_dataset_manifest_digest(
    dataset_storage_key: &str,
    tokenizer_contract_digest: &str,
    splits: &[FirstSwarmDatasetSplitContract],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_first_swarm_dataset_manifest|");
    hasher.update(dataset_storage_key.as_bytes());
    hasher.update(b"|");
    hasher.update(tokenizer_contract_digest.as_bytes());
    for split in splits {
        hasher.update(b"|split|");
        hasher.update(split.split_name.as_bytes());
        hasher.update(b"|");
        hasher.update(split.split_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_swarm_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn first_swarm_contract_freezes_expected_backend_and_adapter_truth() {
        let contract = first_swarm_run_contract();
        assert_eq!(contract.run_family_id, SWARM_FIRST_RUN_FAMILY_ID);
        assert_eq!(
            contract.adapter_family,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FAMILY
        );
        assert_eq!(
            contract.adapter_format,
            OPEN_ADAPTER_REFERENCE_ADAPTER_FORMAT
        );
        assert_eq!(
            contract.dataset.dataset_key.dataset_ref,
            SWARM_FIRST_RUN_DATASET_REF
        );
        assert_eq!(contract.dataset.splits.len(), 2);
        assert!(contract
            .node_roles
            .iter()
            .any(|role| role.backend_label == OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL));
        assert!(contract
            .node_roles
            .iter()
            .any(|role| role.backend_label == OPEN_ADAPTER_CUDA_BACKEND_LABEL));
        assert!(contract.governance.replay_required_per_contribution);
        assert!(contract.governance.require_all_contributor_roles);
        assert!(!contract.contract_digest.is_empty());
        assert_eq!(contract.contract_digest, contract.stable_digest());
    }

    #[test]
    fn first_swarm_contract_refuses_full_model_overclaim_in_non_goals() {
        let contract = first_swarm_run_contract();
        assert!(contract
            .non_goals
            .iter()
            .any(|goal| goal.contains("full-model mixed-backend all-reduce")));
        assert!(contract
            .claim_boundary
            .contains("does not claim one coherent mixed-backend full-model optimizer"));
    }

    #[test]
    fn first_swarm_dataset_manifest_digest_tracks_split_and_tokenizer_identity() {
        let contract = first_swarm_run_contract();
        let expected = stable_dataset_manifest_digest(
            contract.dataset.dataset_storage_key.as_str(),
            contract.dataset.tokenizer.stable_digest().as_str(),
            contract.dataset.splits.as_slice(),
        );
        assert_eq!(contract.dataset.dataset_manifest_digest, expected);
        let split_map = contract
            .dataset
            .splits
            .iter()
            .map(|split| (split.split_name.as_str(), split.split_digest.as_str()))
            .collect::<BTreeMap<_, _>>();
        assert!(split_map.contains_key("train"));
        assert!(split_map.contains_key("validation"));
    }
}
