use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::OPEN_ADAPTER_CUDA_BACKEND_LABEL;

/// Stable schema version for the Google two-node swarm contract.
pub const PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_SCHEMA_VERSION: &str =
    "psion.google_two_node_swarm_contract.v1";
/// Stable fixture path for the Google two-node swarm contract.
pub const PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_contract_v1.json";
/// Stable contract identifier for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_ID: &str =
    "openagentsgemini-psion-google-two-node-swarm-contract-v1";
/// Stable scope window for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_SCOPE_WINDOW: &str =
    "psion_google_two_node_swarm_contract_v1";
/// Stable run family identifier for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_RUN_FAMILY_ID: &str =
    "psion.google.configured_peer_two_node_swarm.v1";
/// Stable project id for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_PROJECT_ID: &str = "openagentsgemini";
/// Stable region family for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_REGION_FAMILY: &str = "us-central1";
/// Stable bucket URL for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_BUCKET_URL: &str =
    "gs://openagentsgemini-psion-train-us-central1";
/// Stable network name for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_NETWORK: &str = "oa-lightning";
/// Stable coordinator subnetwork name for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_COORDINATOR_SUBNETWORK: &str =
    "oa-lightning-us-central1-psion-swarm-coordinator";
/// Stable contributor subnetwork name for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_CONTRIBUTOR_SUBNETWORK: &str =
    "oa-lightning-us-central1-psion-swarm-contributor";
/// Stable cluster namespace for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_CLUSTER_NAMESPACE: &str =
    "cluster.psion.google.configured_peer_swarm";
/// Stable configured-peer admission posture for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_ADMISSION_POSTURE: &str =
    "authenticated_configured_peers.operator_manifest";
/// Stable discovery posture for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_DISCOVERY_POSTURE: &str = "configured_peer_only";
/// Stable training command identifier for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_TRAINING_COMMAND_ID: &str =
    "psion_google_two_node_configured_peer_open_adapter_swarm";
/// Stable checker path for the Google two-node swarm contract.
pub const PSION_GOOGLE_TWO_NODE_SWARM_CHECK_SCRIPT_PATH: &str =
    "scripts/check-psion-google-two-node-swarm-contract.sh";
/// Stable future launch-profiles fixture path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_launch_profiles_v1.json";
/// Stable future network-posture fixture path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_NETWORK_POSTURE_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_network_posture_v1.json";
/// Stable future identity fixture path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_IDENTITY_PROFILE_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_identity_profile_v1.json";
/// Stable future operator-preflight policy fixture path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_OPERATOR_PREFLIGHT_POLICY_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_operator_preflight_policy_v1.json";
/// Stable future impairment policy fixture path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_IMPAIRMENT_POLICY_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_impairment_policy_v1.json";
/// Stable future launch entrypoint path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_SCRIPT_PATH: &str =
    "scripts/psion-google-launch-two-node-swarm.sh";
/// Stable future operator-preflight entrypoint path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_OPERATOR_PREFLIGHT_SCRIPT_PATH: &str =
    "scripts/psion-google-operator-preflight-two-node-swarm.sh";
/// Stable future startup entrypoint path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_STARTUP_SCRIPT_PATH: &str =
    "scripts/psion-google-two-node-swarm-startup.sh";
/// Stable future teardown entrypoint path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_DELETE_SCRIPT_PATH: &str =
    "scripts/psion-google-delete-two-node-swarm.sh";
/// Stable future finalizer entrypoint path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_FINALIZE_SCRIPT_PATH: &str =
    "scripts/psion-google-finalize-two-node-swarm-run.sh";
/// Stable future runbook path for the Google two-node swarm lane.
pub const PSION_GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH: &str =
    "docs/PSION_GOOGLE_TWO_NODE_SWARM_RUNBOOK.md";

const PSION_GOOGLE_SINGLE_NODE_LAUNCH_PROFILES_PATH: &str =
    "fixtures/psion/google/psion_google_single_node_launch_profiles_v1.json";
const PSION_GOOGLE_SINGLE_NODE_NETWORK_POSTURE_PATH: &str =
    "fixtures/psion/google/psion_google_network_posture_v1.json";
const PSION_GOOGLE_SINGLE_NODE_IDENTITY_PROFILE_PATH: &str =
    "fixtures/psion/google/psion_google_training_identity_profile_v1.json";
const PSION_GOOGLE_SINGLE_NODE_STORAGE_PROFILE_PATH: &str =
    "fixtures/psion/google/psion_google_training_storage_profile_v1.json";

/// Errors surfaced while building or writing the Google two-node swarm contract.
#[derive(Debug, Error)]
pub enum PsionGoogleTwoNodeSwarmContractError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("google two-node swarm contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Stable role admitted by the Google two-node swarm contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionGoogleTwoNodeSwarmNodeRoleKind {
    /// Coordinator node that also validates, aggregates, and contributes.
    CoordinatorValidatorAggregatorContributor,
    /// Contributor-only node.
    Contributor,
}

/// One exact zone-pair fallback admitted by the Google two-node swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmZonePair {
    /// Stable zone-pair identifier.
    pub pair_id: String,
    /// Coordinator zone.
    pub coordinator_zone: String,
    /// Contributor zone.
    pub contributor_zone: String,
    /// Short detail for operator use.
    pub detail: String,
}

/// One named baseline single-node artifact this contract builds on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleSingleNodeBaselineArtifact {
    /// Artifact path in the repo.
    pub path: String,
    /// SHA256 over the current artifact bytes.
    pub sha256: String,
    /// Why this artifact still matters to the two-node lane.
    pub detail: String,
}

/// Future repo-owned artifact authority reserved by the contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmArtifactAuthority {
    /// Contract fixture path.
    pub contract_fixture_path: String,
    /// Checker script path.
    pub check_script_path: String,
    /// Dual-node launch profiles fixture path.
    pub launch_profiles_path: String,
    /// Dual-node network posture fixture path.
    pub network_posture_path: String,
    /// Dual-node identity fixture path.
    pub identity_profile_path: String,
    /// Dual-node operator preflight policy path.
    pub operator_preflight_policy_path: String,
    /// Dual-node impairment policy path.
    pub impairment_policy_path: String,
    /// Dual-node operator preflight script path.
    pub operator_preflight_script_path: String,
    /// Dual-node launch script path.
    pub launch_script_path: String,
    /// Dual-node startup script path.
    pub startup_script_path: String,
    /// Dual-node teardown script path.
    pub delete_script_path: String,
    /// Dual-node finalizer path.
    pub finalize_script_path: String,
    /// Dual-node runbook path.
    pub runbook_path: String,
}

/// One per-node contract frozen by the Google two-node swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmNodeContract {
    /// Stable node identifier.
    pub node_id: String,
    /// Stable role identifier.
    pub role_id: String,
    /// Stable role kind.
    pub role_kind: PsionGoogleTwoNodeSwarmNodeRoleKind,
    /// Stable launch profile identifier.
    pub launch_profile_id: String,
    /// Preferred zone for the node.
    pub preferred_zone: String,
    /// Named dedicated subnetwork for the node.
    pub subnetwork: String,
    /// Stable backend label.
    pub backend_label: String,
    /// Stable logical-device label.
    pub logical_device_label: String,
    /// Stable machine type.
    pub machine_type: String,
    /// Stable accelerator type.
    pub accelerator_type: String,
    /// Stable accelerator count.
    pub accelerator_count: u16,
    /// Stable cluster port reserved for the node.
    pub cluster_port: u16,
    /// Stable endpoint manifest object path relative to the run prefix.
    pub endpoint_manifest_object: String,
    /// Short detail for the node.
    pub detail: String,
}

/// Stable bucket prefixes and object names admitted by the Google two-node swarm contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmBucketAuthority {
    /// Stable run prefix template.
    pub run_prefix_template: String,
    /// Stable launch manifest object path.
    pub cluster_manifest_object: String,
    /// Stable coordinator bring-up object path.
    pub coordinator_bringup_report_object: String,
    /// Stable contributor bring-up object path.
    pub contributor_bringup_report_object: String,
    /// Stable cluster evidence bundle object path.
    pub cluster_evidence_bundle_object: String,
    /// Stable final manifest object path.
    pub final_manifest_object: String,
}

/// Full machine-legible contract for the Google two-node swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleTwoNodeSwarmContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run family identifier.
    pub run_family_id: String,
    /// Stable project id.
    pub project_id: String,
    /// Stable region family.
    pub region_family: String,
    /// Stable bucket URL.
    pub bucket_url: String,
    /// Stable network name.
    pub network: String,
    /// Stable cluster namespace.
    pub cluster_namespace: String,
    /// Stable configured-peer admission posture.
    pub cluster_admission_posture: String,
    /// Stable discovery posture.
    pub discovery_posture: String,
    /// Stable training command identifier.
    pub training_command_id: String,
    /// Whether the contract permits external IPs.
    pub external_ip_permitted: bool,
    /// Future artifact authority reserved by this contract.
    pub artifact_authority: PsionGoogleTwoNodeSwarmArtifactAuthority,
    /// Baseline single-node artifacts this contract still depends on.
    pub baseline_single_node_artifacts: Vec<PsionGoogleSingleNodeBaselineArtifact>,
    /// Admitted zone-pair fallback order.
    pub admitted_zone_pairs: Vec<PsionGoogleTwoNodeSwarmZonePair>,
    /// Frozen per-node contract.
    pub nodes: Vec<PsionGoogleTwoNodeSwarmNodeContract>,
    /// Stable bucket authority.
    pub bucket_authority: PsionGoogleTwoNodeSwarmBucketAuthority,
    /// Admitted impairment profile identifiers.
    pub admitted_impairment_profile_ids: Vec<String>,
    /// Typed result classifications admitted by the lane.
    pub result_classifications: Vec<String>,
    /// Explicit non-goals.
    pub non_goals: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl PsionGoogleTwoNodeSwarmContract {
    /// Returns the stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_google_two_node_swarm_contract|", &clone)
    }

    /// Validates the contract invariants.
    pub fn validate(&self) -> Result<(), PsionGoogleTwoNodeSwarmContractError> {
        if self.schema_version != PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_SCHEMA_VERSION {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: format!(
                    "schema_version must be `{}` but was `{}`",
                    PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.project_id != PSION_GOOGLE_TWO_NODE_SWARM_PROJECT_ID {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: format!(
                    "project_id must stay `{}` but was `{}`",
                    PSION_GOOGLE_TWO_NODE_SWARM_PROJECT_ID, self.project_id
                ),
            });
        }
        if self.cluster_admission_posture != PSION_GOOGLE_TWO_NODE_SWARM_ADMISSION_POSTURE {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "cluster admission posture drifted away from the configured-peer contract",
                ),
            });
        }
        if self.discovery_posture != PSION_GOOGLE_TWO_NODE_SWARM_DISCOVERY_POSTURE {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "discovery posture must stay configured_peer_only for this lane",
                ),
            });
        }
        if self.external_ip_permitted {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "google two-node swarm contract must keep external_ip_permitted=false",
                ),
            });
        }
        if self.nodes.len() != 2 {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: format!("expected exactly two nodes but found {}", self.nodes.len()),
            });
        }
        if self
            .nodes
            .iter()
            .any(|node| node.backend_label != OPEN_ADAPTER_CUDA_BACKEND_LABEL)
        {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "all Google two-node swarm nodes must stay on the CUDA open-adapter backend",
                ),
            });
        }
        let coordinator = self
            .nodes
            .iter()
            .find(|node| {
                node.role_kind
                    == PsionGoogleTwoNodeSwarmNodeRoleKind::CoordinatorValidatorAggregatorContributor
            })
            .ok_or_else(|| PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from("missing coordinator-validator-aggregator-contributor node"),
            })?;
        let contributor = self
            .nodes
            .iter()
            .find(|node| node.role_kind == PsionGoogleTwoNodeSwarmNodeRoleKind::Contributor)
            .ok_or_else(|| PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from("missing contributor-only node"),
            })?;
        if coordinator.preferred_zone == contributor.preferred_zone {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from("coordinator and contributor zones must stay distinct"),
            });
        }
        if coordinator.subnetwork == contributor.subnetwork {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from("coordinator and contributor subnetworks must stay distinct"),
            });
        }
        let required_impairment_profiles = [
            "clean_baseline",
            "mild_wan",
            "asymmetric_degraded",
            "temporary_partition",
        ];
        if required_impairment_profiles.iter().any(|profile_id| {
            !self
                .admitted_impairment_profile_ids
                .iter()
                .any(|id| id == profile_id)
        }) {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "admitted_impairment_profile_ids lost one or more required impairment profiles",
                ),
            });
        }
        let required_result_classes = [
            "configured_peer_launch_failure",
            "cluster_membership_failure",
            "network_impairment_gate_failure",
            "contributor_execution_failure",
            "validator_refusal",
            "aggregation_failure",
            "bounded_success",
        ];
        if required_result_classes.iter().any(|class_id| {
            !self
                .result_classifications
                .iter()
                .any(|candidate| candidate == class_id)
        }) {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "result_classifications lost one or more admitted Google swarm result classes",
                ),
            });
        }
        if self.admitted_zone_pairs.len() < 3 {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from("expected at least three admitted zone-pair fallbacks"),
            });
        }
        if self.artifact_authority.launch_profiles_path
            != PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH
        {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from(
                    "launch_profiles_path drifted away from the reserved authority path",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(PsionGoogleTwoNodeSwarmContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical Google two-node swarm contract.
pub fn psion_google_two_node_swarm_contract(
) -> Result<PsionGoogleTwoNodeSwarmContract, PsionGoogleTwoNodeSwarmContractError> {
    let baseline_single_node_artifacts = vec![
        baseline_artifact(
            PSION_GOOGLE_SINGLE_NODE_LAUNCH_PROFILES_PATH,
            "The dual-node lane still depends on the existing single-node launch authority as the starting Google machine-shape substrate.",
        )?,
        baseline_artifact(
            PSION_GOOGLE_SINGLE_NODE_NETWORK_POSTURE_PATH,
            "The dual-node lane still depends on the existing private-egress plus IAP SSH Google network posture and widens it explicitly rather than replacing it implicitly.",
        )?,
        baseline_artifact(
            PSION_GOOGLE_SINGLE_NODE_IDENTITY_PROFILE_PATH,
            "The dual-node lane still depends on the existing Google training identity posture and widens it to a role-aware swarm identity contract later.",
        )?,
        baseline_artifact(
            PSION_GOOGLE_SINGLE_NODE_STORAGE_PROFILE_PATH,
            "The dual-node lane still uses the existing dedicated training bucket and durable prefix layout as its storage authority.",
        )?,
    ];
    let mut contract = PsionGoogleTwoNodeSwarmContract {
        schema_version: String::from(PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_ID),
        scope_window: String::from(PSION_GOOGLE_TWO_NODE_SWARM_SCOPE_WINDOW),
        run_family_id: String::from(PSION_GOOGLE_TWO_NODE_SWARM_RUN_FAMILY_ID),
        project_id: String::from(PSION_GOOGLE_TWO_NODE_SWARM_PROJECT_ID),
        region_family: String::from(PSION_GOOGLE_TWO_NODE_SWARM_REGION_FAMILY),
        bucket_url: String::from(PSION_GOOGLE_TWO_NODE_SWARM_BUCKET_URL),
        network: String::from(PSION_GOOGLE_TWO_NODE_SWARM_NETWORK),
        cluster_namespace: String::from(PSION_GOOGLE_TWO_NODE_SWARM_CLUSTER_NAMESPACE),
        cluster_admission_posture: String::from(PSION_GOOGLE_TWO_NODE_SWARM_ADMISSION_POSTURE),
        discovery_posture: String::from(PSION_GOOGLE_TWO_NODE_SWARM_DISCOVERY_POSTURE),
        training_command_id: String::from(PSION_GOOGLE_TWO_NODE_SWARM_TRAINING_COMMAND_ID),
        external_ip_permitted: false,
        artifact_authority: PsionGoogleTwoNodeSwarmArtifactAuthority {
            contract_fixture_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_CHECK_SCRIPT_PATH),
            launch_profiles_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH),
            network_posture_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_NETWORK_POSTURE_PATH),
            identity_profile_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_IDENTITY_PROFILE_PATH),
            operator_preflight_policy_path: String::from(
                PSION_GOOGLE_TWO_NODE_SWARM_OPERATOR_PREFLIGHT_POLICY_PATH,
            ),
            impairment_policy_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_IMPAIRMENT_POLICY_PATH),
            operator_preflight_script_path: String::from(
                PSION_GOOGLE_TWO_NODE_SWARM_OPERATOR_PREFLIGHT_SCRIPT_PATH,
            ),
            launch_script_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_SCRIPT_PATH),
            startup_script_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_STARTUP_SCRIPT_PATH),
            delete_script_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_DELETE_SCRIPT_PATH),
            finalize_script_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_FINALIZE_SCRIPT_PATH),
            runbook_path: String::from(PSION_GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH),
        },
        baseline_single_node_artifacts,
        admitted_zone_pairs: vec![
            PsionGoogleTwoNodeSwarmZonePair {
                pair_id: String::from("us-central1-a__us-central1-b"),
                coordinator_zone: String::from("us-central1-a"),
                contributor_zone: String::from("us-central1-b"),
                detail: String::from(
                    "Primary dual-zone pair for the first Google swarm rehearsal.",
                ),
            },
            PsionGoogleTwoNodeSwarmZonePair {
                pair_id: String::from("us-central1-a__us-central1-c"),
                coordinator_zone: String::from("us-central1-a"),
                contributor_zone: String::from("us-central1-c"),
                detail: String::from(
                    "First fallback pair when `us-central1-b` is quota-constrained or unavailable.",
                ),
            },
            PsionGoogleTwoNodeSwarmZonePair {
                pair_id: String::from("us-central1-b__us-central1-c"),
                coordinator_zone: String::from("us-central1-b"),
                contributor_zone: String::from("us-central1-c"),
                detail: String::from(
                    "Second fallback pair when the primary coordinator zone is unavailable.",
                ),
            },
        ],
        nodes: vec![
            PsionGoogleTwoNodeSwarmNodeContract {
                node_id: String::from("psion-google-swarm-coordinator-a"),
                role_id: String::from(
                    "psion.google_swarm.coordinator_validator_aggregator_contributor",
                ),
                role_kind:
                    PsionGoogleTwoNodeSwarmNodeRoleKind::CoordinatorValidatorAggregatorContributor,
                launch_profile_id: String::from("g2_l4_two_node_swarm_coordinator"),
                preferred_zone: String::from("us-central1-a"),
                subnetwork: String::from(PSION_GOOGLE_TWO_NODE_SWARM_COORDINATOR_SUBNETWORK),
                backend_label: String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL),
                logical_device_label: String::from("cuda:0"),
                machine_type: String::from("g2-standard-8"),
                accelerator_type: String::from("nvidia-l4"),
                accelerator_count: 1,
                cluster_port: 34100,
                endpoint_manifest_object: String::from(
                    "manifests/psion_google_two_node_swarm_coordinator_endpoint_manifest.json",
                ),
                detail: String::from(
                    "The coordinator node owns configured-peer authority, validator visibility, aggregation authority, and one CUDA contributor slot.",
                ),
            },
            PsionGoogleTwoNodeSwarmNodeContract {
                node_id: String::from("psion-google-swarm-contributor-b"),
                role_id: String::from("psion.google_swarm.contributor"),
                role_kind: PsionGoogleTwoNodeSwarmNodeRoleKind::Contributor,
                launch_profile_id: String::from("g2_l4_two_node_swarm_contributor"),
                preferred_zone: String::from("us-central1-b"),
                subnetwork: String::from(PSION_GOOGLE_TWO_NODE_SWARM_CONTRIBUTOR_SUBNETWORK),
                backend_label: String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL),
                logical_device_label: String::from("cuda:0"),
                machine_type: String::from("g2-standard-8"),
                accelerator_type: String::from("nvidia-l4"),
                accelerator_count: 1,
                cluster_port: 34101,
                endpoint_manifest_object: String::from(
                    "manifests/psion_google_two_node_swarm_contributor_endpoint_manifest.json",
                ),
                detail: String::from(
                    "The contributor node owns one CUDA contributor slot and no validator or aggregation authority.",
                ),
            },
        ],
        bucket_authority: PsionGoogleTwoNodeSwarmBucketAuthority {
            run_prefix_template: String::from(
                "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}",
            ),
            cluster_manifest_object: String::from(
                "launch/psion_google_two_node_swarm_cluster_manifest.json",
            ),
            coordinator_bringup_report_object: String::from(
                "host/coordinator/psion_google_two_node_swarm_bringup_report.json",
            ),
            contributor_bringup_report_object: String::from(
                "host/contributor/psion_google_two_node_swarm_bringup_report.json",
            ),
            cluster_evidence_bundle_object: String::from(
                "final/psion_google_two_node_swarm_evidence_bundle.json",
            ),
            final_manifest_object: String::from(
                "final/psion_google_two_node_swarm_final_manifest.json",
            ),
        },
        admitted_impairment_profile_ids: vec![
            String::from("clean_baseline"),
            String::from("mild_wan"),
            String::from("asymmetric_degraded"),
            String::from("temporary_partition"),
        ],
        result_classifications: vec![
            String::from("configured_peer_launch_failure"),
            String::from("cluster_membership_failure"),
            String::from("network_impairment_gate_failure"),
            String::from("contributor_execution_failure"),
            String::from("validator_refusal"),
            String::from("aggregation_failure"),
            String::from("bounded_success"),
        ],
        non_goals: vec![
            String::from("trusted-cluster full-model Google training"),
            String::from("cross-region or public-internet cluster execution"),
            String::from("wider-network discovery rollout"),
            String::from("mixed-backend or mixed-hardware math parity closure"),
        ],
        claim_boundary: String::from(
            "This contract freezes one bounded Google two-node configured-peer swarm rehearsal: two GCE L4 nodes in one project and one region family, distinct zones, distinct dedicated subnetworks, no external IPs, explicit configured-peer admission, explicit impairment profile ids, and one CUDA open-adapter adapter-delta lane. It does not claim trusted-cluster full-model training, wider-network discovery, cross-region rollout, or internet-wide swarm compute.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

/// Writes the canonical Google two-node swarm contract to one JSON path.
pub fn write_psion_google_two_node_swarm_contract(
    output_path: impl AsRef<Path>,
) -> Result<PsionGoogleTwoNodeSwarmContract, PsionGoogleTwoNodeSwarmContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionGoogleTwoNodeSwarmContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = psion_google_two_node_swarm_contract()?;
    let encoded = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        PsionGoogleTwoNodeSwarmContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

fn baseline_artifact(
    path: &str,
    detail: &str,
) -> Result<PsionGoogleSingleNodeBaselineArtifact, PsionGoogleTwoNodeSwarmContractError> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| PsionGoogleTwoNodeSwarmContractError::InvalidContract {
            detail: String::from("could not derive the repo root from CARGO_MANIFEST_DIR"),
        })?
        .to_path_buf();
    let artifact_path = repo_root.join(path);
    let bytes =
        fs::read(&artifact_path).map_err(|error| PsionGoogleTwoNodeSwarmContractError::Read {
            path: artifact_path.display().to_string(),
            error,
        })?;
    Ok(PsionGoogleSingleNodeBaselineArtifact {
        path: String::from(path),
        sha256: hex_sha256(bytes.as_slice()),
        detail: String::from(detail),
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization should succeed"));
    format!("{:x}", hasher.finalize())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_two_node_swarm_contract_stays_valid() {
        let contract =
            psion_google_two_node_swarm_contract().expect("contract should build successfully");
        contract.validate().expect("contract should validate");
        assert_eq!(contract.contract_digest, contract.stable_digest());
        assert_eq!(contract.nodes.len(), 2);
        assert_eq!(
            contract.nodes[0].backend_label,
            String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL)
        );
    }

    #[test]
    fn google_two_node_swarm_contract_keeps_required_profiles() {
        let contract =
            psion_google_two_node_swarm_contract().expect("contract should build successfully");
        assert!(contract
            .admitted_impairment_profile_ids
            .contains(&String::from("clean_baseline")));
        assert!(contract
            .result_classifications
            .contains(&String::from("bounded_success")));
        assert_eq!(
            contract.artifact_authority.launch_profiles_path,
            String::from(PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH)
        );
    }
}
