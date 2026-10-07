use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_runtime_binder, cross_provider_training_program_manifest,
    CrossProviderRuntimeAdapterKind, CrossProviderRuntimeBinderError,
    CrossProviderTrainingProgramManifestError,
};

/// Stable schema version for the Google training binder projection set.
pub const GOOGLE_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION: &str =
    "psionic.google_training_binder_projection.v1";
/// Stable fixture path for the Google training binder projection set.
pub const GOOGLE_TRAINING_BINDER_PROJECTION_FIXTURE_PATH: &str =
    "fixtures/training/google_training_binder_projection_v1.json";
/// Stable checker path for the Google training binder projection set.
pub const GOOGLE_TRAINING_BINDER_PROJECTION_CHECK_SCRIPT_PATH: &str =
    "scripts/check-google-training-binder-projection.sh";
/// Stable reference doc path for the Google training binder projection set.
pub const GOOGLE_TRAINING_BINDER_PROJECTION_DOC_PATH: &str =
    "docs/GOOGLE_TRAINING_BINDER_REFERENCE.md";

const GOOGLE_SINGLE_NODE_RUNBOOK_PATH: &str = "docs/PSION_GOOGLE_SINGLE_GPU_RUNBOOK.md";
const GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH: &str = "docs/PSION_GOOGLE_TWO_NODE_SWARM_RUNBOOK.md";
const GOOGLE_SINGLE_NODE_LAUNCH_PROFILES_PATH: &str =
    "fixtures/psion/google/psion_google_single_node_launch_profiles_v1.json";
const GOOGLE_SINGLE_NODE_OPERATOR_PREFLIGHT_PATH: &str =
    "fixtures/psion/google/psion_google_operator_preflight_policy_v1.json";
const GOOGLE_SINGLE_NODE_OBSERVABILITY_PATH: &str =
    "fixtures/psion/google/psion_google_host_observability_policy_v1.json";
const GOOGLE_SINGLE_NODE_LAUNCH_SCRIPT_PATH: &str = "scripts/psion-google-launch-single-node.sh";
const GOOGLE_SINGLE_NODE_STARTUP_SCRIPT_PATH: &str = "scripts/psion-google-single-node-startup.sh";
const GOOGLE_SINGLE_NODE_FINALIZER_SCRIPT_PATH: &str = "scripts/psion-google-finalize-run.sh";
const GOOGLE_TWO_NODE_SWARM_CONTRACT_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_contract_v1.json";
const GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_launch_profiles_v1.json";
const GOOGLE_TWO_NODE_SWARM_OPERATOR_PREFLIGHT_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_operator_preflight_policy_v1.json";
const GOOGLE_TWO_NODE_SWARM_LAUNCH_SCRIPT_PATH: &str =
    "scripts/psion-google-launch-two-node-swarm.sh";
const GOOGLE_TWO_NODE_SWARM_STARTUP_SCRIPT_PATH: &str =
    "scripts/psion-google-two-node-swarm-startup.sh";
const GOOGLE_TWO_NODE_SWARM_FINALIZER_SCRIPT_PATH: &str =
    "scripts/psion-google-finalize-two-node-swarm-run.sh";
const GOOGLE_TWO_NODE_SWARM_CONTRACT_CHECKER_PATH: &str =
    "scripts/check-psion-google-two-node-swarm-contract.sh";
const GOOGLE_TWO_NODE_SWARM_RUNBOOK_CHECKER_PATH: &str =
    "scripts/check-psion-google-two-node-swarm-runbook.sh";
const GOOGLE_TWO_NODE_SWARM_EVIDENCE_CHECKER_PATH: &str =
    "scripts/check-psion-google-two-node-swarm-evidence-bundle.sh";

/// Errors surfaced while building, validating, or writing the Google training binder projection.
#[derive(Debug, Error)]
pub enum GoogleTrainingBinderProjectionError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    RuntimeBinder(#[from] CrossProviderRuntimeBinderError),
    #[error("google training binder projection is invalid: {detail}")]
    InvalidProjection { detail: String },
}

/// Bound Google lane kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoogleTrainingBinderLaneKind {
    /// Google accelerated single-node lane.
    SingleNodeAccelerated,
    /// Google configured-peer two-node swarm lane.
    TwoNodeConfiguredPeerSwarm,
}

/// One Google lane projected out of the shared binder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleTrainingBinderLaneProjection {
    /// Stable lane projection id.
    pub lane_projection_id: String,
    /// Bound Google lane kind.
    pub lane_kind: GoogleTrainingBinderLaneKind,
    /// Shared runtime binding id.
    pub runtime_binding_id: String,
    /// Shared runtime binding digest.
    pub runtime_binding_digest: String,
    /// Shared launch-contract id.
    pub launch_contract_id: String,
    /// Canonical runbook path.
    pub runbook_path: String,
    /// Canonical operator or quota preflight inputs.
    pub preflight_authorities: Vec<String>,
    /// Canonical launch authority path.
    pub launch_authority_path: String,
    /// Launch command path.
    pub launch_script_path: String,
    /// Startup command path.
    pub startup_script_path: String,
    /// Finalizer command path.
    pub finalizer_script_path: String,
    /// Existing checker surfaces that must remain green under the binder.
    pub retained_checker_paths: Vec<String>,
    /// Existing evidence surfaces that remain authoritative.
    pub retained_evidence_paths: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable projection digest.
    pub projection_digest: String,
}

impl GoogleTrainingBinderLaneProjection {
    /// Returns the stable digest over the lane projection.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.projection_digest.clear();
        stable_digest(b"psionic_google_training_binder_lane_projection|", &clone)
    }
}

/// Canonical Google training binder projection set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleTrainingBinderProjectionSet {
    /// Stable schema version.
    pub schema_version: String,
    /// Root training-program manifest id.
    pub program_manifest_id: String,
    /// Root training-program manifest digest.
    pub program_manifest_digest: String,
    /// Shared runtime binder contract digest.
    pub runtime_binder_contract_digest: String,
    /// Google lane projections.
    pub lane_projections: Vec<GoogleTrainingBinderLaneProjection>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl GoogleTrainingBinderProjectionSet {
    /// Returns the stable digest over the projection set.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_google_training_binder_projection_set|", &clone)
    }

    /// Validates the projection set against the shared runtime binder.
    pub fn validate(&self) -> Result<(), GoogleTrainingBinderProjectionError> {
        let manifest = cross_provider_training_program_manifest()?;
        let binder = canonical_cross_provider_runtime_binder()?;
        if self.schema_version != GOOGLE_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION {
            return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    GOOGLE_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                detail: String::from("program-manifest binding drifted"),
            });
        }
        if self.runtime_binder_contract_digest != binder.contract_digest {
            return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                detail: String::from("runtime binder digest drifted"),
            });
        }
        if self.lane_projections.len() != 2 {
            return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                detail: format!(
                    "lane_projections must stay at 2 Google lanes but found {}",
                    self.lane_projections.len()
                ),
            });
        }
        for projection in &self.lane_projections {
            let binding = binder
                .binding_records
                .iter()
                .find(|binding| binding.binding_id == projection.runtime_binding_id)
                .ok_or_else(|| GoogleTrainingBinderProjectionError::InvalidProjection {
                    detail: format!(
                        "lane projection `{}` referenced unknown runtime binding `{}`",
                        projection.lane_projection_id, projection.runtime_binding_id
                    ),
                })?;
            match projection.lane_kind {
                GoogleTrainingBinderLaneKind::SingleNodeAccelerated => {
                    if binding.adapter_kind != CrossProviderRuntimeAdapterKind::GoogleHost {
                        return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                            detail: String::from(
                                "single-node accelerated projection lost GoogleHost binding",
                            ),
                        });
                    }
                }
                GoogleTrainingBinderLaneKind::TwoNodeConfiguredPeerSwarm => {
                    if binding.adapter_kind
                        != CrossProviderRuntimeAdapterKind::GoogleConfiguredPeerCluster
                    {
                        return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                            detail: String::from(
                                "two-node swarm projection lost GoogleConfiguredPeerCluster binding",
                            ),
                        });
                    }
                }
            }
            if projection.runtime_binding_digest != binding.binding_digest {
                return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                    detail: format!(
                        "lane projection `{}` binding digest drifted",
                        projection.lane_projection_id
                    ),
                });
            }
            if projection.projection_digest != projection.stable_digest() {
                return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                    detail: format!(
                        "lane projection `{}` digest drifted",
                        projection.lane_projection_id
                    ),
                });
            }
        }
        if self.contract_digest != self.stable_digest() {
            return Err(GoogleTrainingBinderProjectionError::InvalidProjection {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical Google training binder projection set.
pub fn canonical_google_training_binder_projection_set(
) -> Result<GoogleTrainingBinderProjectionSet, GoogleTrainingBinderProjectionError> {
    let manifest = cross_provider_training_program_manifest()?;
    let binder = canonical_cross_provider_runtime_binder()?;
    let mut set = GoogleTrainingBinderProjectionSet {
        schema_version: String::from(GOOGLE_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        runtime_binder_contract_digest: binder.contract_digest.clone(),
        lane_projections: vec![
            projection_for_binding(
                binder
                    .binding_records
                    .iter()
                    .find(|binding| binding.adapter_kind == CrossProviderRuntimeAdapterKind::GoogleHost)
                    .expect("canonical runtime binder must retain the Google single-node binding"),
                GoogleTrainingBinderLaneKind::SingleNodeAccelerated,
            ),
            projection_for_binding(
                binder
                    .binding_records
                    .iter()
                    .find(|binding| {
                        binding.adapter_kind
                            == CrossProviderRuntimeAdapterKind::GoogleConfiguredPeerCluster
                    })
                    .expect("canonical runtime binder must retain the Google swarm binding"),
                GoogleTrainingBinderLaneKind::TwoNodeConfiguredPeerSwarm,
            ),
        ],
        claim_boundary: String::from(
            "This projection closes the Google single-node and Google swarm lanes as consumers of the shared cross-provider runtime binder. It does not widen the Google swarm claim to dense full-model training and it does not replace the current Google operator scripts with generic provider automation.",
        ),
        contract_digest: String::new(),
    };
    set.contract_digest = set.stable_digest();
    set.validate()?;
    Ok(set)
}

/// Writes the canonical Google training binder projection fixture.
pub fn write_google_training_binder_projection_set(
    output_path: impl AsRef<Path>,
) -> Result<(), GoogleTrainingBinderProjectionError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            GoogleTrainingBinderProjectionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let projection = canonical_google_training_binder_projection_set()?;
    let json = serde_json::to_vec_pretty(&projection)?;
    fs::write(output_path, json).map_err(|error| GoogleTrainingBinderProjectionError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn projection_for_binding(
    binding: &crate::CrossProviderRuntimeBindingRecord,
    lane_kind: GoogleTrainingBinderLaneKind,
) -> GoogleTrainingBinderLaneProjection {
    let (
        lane_projection_id,
        runbook_path,
        preflight_authorities,
        launch_authority_path,
        launch_script_path,
        startup_script_path,
        finalizer_script_path,
        retained_checker_paths,
        retained_evidence_paths,
        claim_boundary,
    ) = match lane_kind {
        GoogleTrainingBinderLaneKind::SingleNodeAccelerated => (
            String::from("google_single_node_accelerated"),
            String::from(GOOGLE_SINGLE_NODE_RUNBOOK_PATH),
            vec![
                String::from(GOOGLE_SINGLE_NODE_LAUNCH_PROFILES_PATH),
                String::from(GOOGLE_SINGLE_NODE_OPERATOR_PREFLIGHT_PATH),
            ],
            String::from(GOOGLE_SINGLE_NODE_OBSERVABILITY_PATH),
            String::from(GOOGLE_SINGLE_NODE_LAUNCH_SCRIPT_PATH),
            String::from(GOOGLE_SINGLE_NODE_STARTUP_SCRIPT_PATH),
            String::from(GOOGLE_SINGLE_NODE_FINALIZER_SCRIPT_PATH),
            vec![String::from(
                "scripts/check-cross-provider-runtime-binder.sh",
            )],
            vec![
                String::from("psion_google_accelerator_validation_receipt.json"),
                String::from("psion_google_run_cost_receipt.json"),
                String::from("training_visualization/psion_google_live_remote_training_visualization_bundle_v1.json"),
            ],
            String::from(
                "The accelerated single-node Google lane still owns its quota, launch, and finalizer scripts, but the training-facing runtime and evidence semantics now come from the shared binder.",
            ),
        ),
        GoogleTrainingBinderLaneKind::TwoNodeConfiguredPeerSwarm => (
            String::from("google_two_node_configured_peer_swarm"),
            String::from(GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH),
            vec![
                String::from(GOOGLE_TWO_NODE_SWARM_CONTRACT_PATH),
                String::from(GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH),
                String::from(GOOGLE_TWO_NODE_SWARM_OPERATOR_PREFLIGHT_PATH),
            ],
            String::from(GOOGLE_TWO_NODE_SWARM_CONTRACT_PATH),
            String::from(GOOGLE_TWO_NODE_SWARM_LAUNCH_SCRIPT_PATH),
            String::from(GOOGLE_TWO_NODE_SWARM_STARTUP_SCRIPT_PATH),
            String::from(GOOGLE_TWO_NODE_SWARM_FINALIZER_SCRIPT_PATH),
            vec![
                String::from(GOOGLE_TWO_NODE_SWARM_CONTRACT_CHECKER_PATH),
                String::from(GOOGLE_TWO_NODE_SWARM_RUNBOOK_CHECKER_PATH),
                String::from(GOOGLE_TWO_NODE_SWARM_EVIDENCE_CHECKER_PATH),
            ],
            vec![
                String::from("final/psion_google_two_node_swarm_evidence_bundle.json"),
                String::from("final/psion_google_two_node_swarm_final_manifest.json"),
                String::from("training_visualization/remote_training_run_index_v1.json"),
            ],
            String::from(
                "The Google two-node swarm lane still keeps its bounded configured-peer claim, but its launch, runtime env, artifact roots, and finalizer semantics now come from the shared binder instead of Google-only training truth.",
            ),
        ),
    };
    let mut projection = GoogleTrainingBinderLaneProjection {
        lane_projection_id,
        lane_kind,
        runtime_binding_id: binding.binding_id.clone(),
        runtime_binding_digest: binding.binding_digest.clone(),
        launch_contract_id: binding.launch_contract_id.clone(),
        runbook_path,
        preflight_authorities,
        launch_authority_path,
        launch_script_path,
        startup_script_path,
        finalizer_script_path,
        retained_checker_paths,
        retained_evidence_paths,
        claim_boundary,
        projection_digest: String::new(),
    };
    projection.projection_digest = projection.stable_digest();
    projection
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("google training binder values must serialize"));
    format!("{:x}", hasher.finalize())
}
