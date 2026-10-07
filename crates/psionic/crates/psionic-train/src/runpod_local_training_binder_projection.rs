use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_runtime_binder, cross_provider_training_program_manifest,
    CrossProviderRuntimeAdapterKind, CrossProviderRuntimeBinderError,
    CrossProviderTrainingProgramManifestError,
};

/// Stable schema version for the RunPod plus local training binder projection set.
pub const RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION: &str =
    "psionic.runpod_local_training_binder_projection.v1";
/// Stable fixture path for the RunPod plus local training binder projection set.
pub const RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_FIXTURE_PATH: &str =
    "fixtures/training/runpod_local_training_binder_projection_v1.json";
/// Stable checker path for the RunPod plus local training binder projection set.
pub const RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_CHECK_SCRIPT_PATH: &str =
    "scripts/check-runpod-local-training-binder-projection.sh";
/// Stable reference doc path for the RunPod plus local training binder projection set.
pub const RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_DOC_PATH: &str =
    "docs/RUNPOD_LOCAL_TRAINING_BINDER_REFERENCE.md";

const RUNPOD_RUNBOOK_PATH: &str = "docs/PARAMETER_GOLF_RUNPOD_8XH100_RUNBOOK.md";
const LOCAL_TRUSTED_LAN_RUNBOOK_PATH: &str = "docs/FIRST_SWARM_TRUSTED_LAN_RUNBOOK.md";
const RUNPOD_LAUNCH_PROFILES_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_launch_profiles_v1.json";
const RUNPOD_OPERATOR_PREFLIGHT_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_operator_preflight_policy_v1.json";
const RUNPOD_COST_GUARDRAILS_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_cost_guardrails_v1.json";
const RUNPOD_LAUNCH_SCRIPT_PATH: &str = "scripts/parameter-golf-runpod-launch-8xh100.sh";
const RUNPOD_FINALIZER_SCRIPT_PATH: &str = "scripts/parameter-golf-runpod-finalize-8xh100.sh";
const RUNPOD_LANE_CHECKER_PATH: &str = "scripts/check-parameter-golf-runpod-8xh100-lane.sh";
const LOCAL_TOPOLOGY_CONTRACT_PATH: &str =
    "fixtures/swarm/first_swarm_trusted_lan_topology_contract_v1.json";
const LOCAL_FAILURE_DRILLS_PATH: &str =
    "fixtures/swarm/reports/first_swarm_trusted_lan_failure_drills_v1.json";
const LOCAL_WORKFLOW_PLAN_PATH: &str = "fixtures/swarm/first_swarm_live_workflow_plan_v1.json";
const LOCAL_MAC_BRINGUP_PATH: &str = "fixtures/swarm/reports/swarm_mac_mlx_bringup_v1.json";
const LOCAL_LINUX_BRINGUP_PATH: &str = "fixtures/swarm/reports/swarm_linux_rtx4080_bringup_v1.json";
const LOCAL_LAUNCH_SCRIPT_PATH: &str = "scripts/first-swarm-launch-trusted-lan.sh";
const LOCAL_TRUSTED_LAN_CHECKER_PATH: &str = "scripts/check-first-swarm-trusted-lan.sh";
const LOCAL_TRUSTED_LAN_REHEARSAL_CHECKER_PATH: &str =
    "scripts/check-first-swarm-trusted-lan-rehearsal.sh";
const LOCAL_TRUSTED_LAN_EVIDENCE_CHECKER_PATH: &str =
    "scripts/check-first-swarm-trusted-lan-evidence-bundle.sh";
const LOCAL_TRUSTED_LAN_CLOSEOUT_CHECKER_PATH: &str =
    "scripts/check-first-swarm-trusted-lan-closeout.sh";

/// Errors surfaced while building, validating, or writing the RunPod plus local training binder projection.
#[derive(Debug, Error)]
pub enum RunPodLocalTrainingBinderProjectionError {
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
    #[error("runpod plus local training binder projection is invalid: {detail}")]
    InvalidProjection { detail: String },
}

/// Bound non-Google lane kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunPodLocalTrainingBinderLaneKind {
    /// RunPod single-pod distributed lane.
    RunPodDistributedEightH100,
    /// Local trusted-LAN mixed-hardware lane.
    LocalTrustedLanSwarm,
}

/// One non-Google lane projected out of the shared binder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPodLocalTrainingBinderLaneProjection {
    /// Stable lane projection id.
    pub lane_projection_id: String,
    /// Bound lane kind.
    pub lane_kind: RunPodLocalTrainingBinderLaneKind,
    /// Shared runtime binding id.
    pub runtime_binding_id: String,
    /// Shared runtime binding digest.
    pub runtime_binding_digest: String,
    /// Shared launch-contract id.
    pub launch_contract_id: String,
    /// Canonical runbook path.
    pub runbook_path: String,
    /// Canonical authority paths retained by the lane.
    pub retained_authority_paths: Vec<String>,
    /// Launch command path.
    pub launch_script_path: String,
    /// Optional startup command path when the lane uses one.
    pub startup_script_path: Option<String>,
    /// Finalizer or closeout command path.
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

impl RunPodLocalTrainingBinderLaneProjection {
    /// Returns the stable digest over the lane projection.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.projection_digest.clear();
        stable_digest(
            b"psionic_runpod_local_training_binder_lane_projection|",
            &clone,
        )
    }
}

/// Canonical RunPod plus local training binder projection set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPodLocalTrainingBinderProjectionSet {
    /// Stable schema version.
    pub schema_version: String,
    /// Root training-program manifest id.
    pub program_manifest_id: String,
    /// Root training-program manifest digest.
    pub program_manifest_digest: String,
    /// Shared runtime binder contract digest.
    pub runtime_binder_contract_digest: String,
    /// Lane projections.
    pub lane_projections: Vec<RunPodLocalTrainingBinderLaneProjection>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl RunPodLocalTrainingBinderProjectionSet {
    /// Returns the stable digest over the projection set.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(
            b"psionic_runpod_local_training_binder_projection_set|",
            &clone,
        )
    }

    /// Validates the projection set against the shared runtime binder.
    pub fn validate(&self) -> Result<(), RunPodLocalTrainingBinderProjectionError> {
        let manifest = cross_provider_training_program_manifest()?;
        let binder = canonical_cross_provider_runtime_binder()?;
        if self.schema_version != RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION {
            return Err(
                RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                    detail: format!(
                        "schema_version must stay `{}` but was `{}`",
                        RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION, self.schema_version
                    ),
                },
            );
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(
                RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                    detail: String::from("program-manifest binding drifted"),
                },
            );
        }
        if self.runtime_binder_contract_digest != binder.contract_digest {
            return Err(
                RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                    detail: String::from("runtime binder digest drifted"),
                },
            );
        }
        if self.lane_projections.len() != 2 {
            return Err(
                RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                    detail: format!(
                        "lane_projections must stay at 2 non-Google lanes but found {}",
                        self.lane_projections.len()
                    ),
                },
            );
        }
        for projection in &self.lane_projections {
            let binding = binder
                .binding_records
                .iter()
                .find(|binding| binding.binding_id == projection.runtime_binding_id)
                .ok_or_else(
                    || RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                        detail: format!(
                            "lane projection `{}` referenced unknown runtime binding `{}`",
                            projection.lane_projection_id, projection.runtime_binding_id
                        ),
                    },
                )?;
            match projection.lane_kind {
                RunPodLocalTrainingBinderLaneKind::RunPodDistributedEightH100 => {
                    if binding.adapter_kind != CrossProviderRuntimeAdapterKind::RunPodRemotePod {
                        return Err(
                            RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                                detail: String::from(
                                    "RunPod lane projection lost RunPodRemotePod binding",
                                ),
                            },
                        );
                    }
                }
                RunPodLocalTrainingBinderLaneKind::LocalTrustedLanSwarm => {
                    if binding.adapter_kind
                        != CrossProviderRuntimeAdapterKind::LocalTrustedLanBundle
                    {
                        return Err(RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                            detail: String::from(
                                "local trusted-LAN projection lost LocalTrustedLanBundle binding",
                            ),
                        });
                    }
                }
            }
            if projection.runtime_binding_digest != binding.binding_digest {
                return Err(
                    RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                        detail: format!(
                            "lane projection `{}` binding digest drifted",
                            projection.lane_projection_id
                        ),
                    },
                );
            }
            if projection.projection_digest != projection.stable_digest() {
                return Err(
                    RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                        detail: format!(
                            "lane projection `{}` digest drifted",
                            projection.lane_projection_id
                        ),
                    },
                );
            }
        }
        if self.contract_digest != self.stable_digest() {
            return Err(
                RunPodLocalTrainingBinderProjectionError::InvalidProjection {
                    detail: String::from("contract_digest does not match the stable digest"),
                },
            );
        }
        Ok(())
    }
}

/// Returns the canonical RunPod plus local training binder projection set.
pub fn canonical_runpod_local_training_binder_projection_set(
) -> Result<RunPodLocalTrainingBinderProjectionSet, RunPodLocalTrainingBinderProjectionError> {
    let manifest = cross_provider_training_program_manifest()?;
    let binder = canonical_cross_provider_runtime_binder()?;
    let mut set = RunPodLocalTrainingBinderProjectionSet {
        schema_version: String::from(RUNPOD_LOCAL_TRAINING_BINDER_PROJECTION_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        runtime_binder_contract_digest: binder.contract_digest.clone(),
        lane_projections: vec![
            projection_for_binding(
                binder
                    .binding_records
                    .iter()
                    .find(|binding| binding.adapter_kind == CrossProviderRuntimeAdapterKind::RunPodRemotePod)
                    .expect("canonical runtime binder must retain the RunPod binding"),
                RunPodLocalTrainingBinderLaneKind::RunPodDistributedEightH100,
            ),
            projection_for_binding(
                binder
                    .binding_records
                    .iter()
                    .find(|binding| {
                        binding.adapter_kind == CrossProviderRuntimeAdapterKind::LocalTrustedLanBundle
                    })
                    .expect("canonical runtime binder must retain the local trusted-LAN binding"),
                RunPodLocalTrainingBinderLaneKind::LocalTrustedLanSwarm,
            ),
        ],
        claim_boundary: String::from(
            "This projection closes the current RunPod and local trusted-LAN lanes as consumers of the shared cross-provider runtime binder. It does not widen the RunPod 8xH100 lane to a finished dense runtime and it does not widen the local swarm lane to a successful same-job mixed-backend dense trainer.",
        ),
        contract_digest: String::new(),
    };
    set.contract_digest = set.stable_digest();
    set.validate()?;
    Ok(set)
}

/// Writes the canonical RunPod plus local training binder projection fixture.
pub fn write_runpod_local_training_binder_projection_set(
    output_path: impl AsRef<Path>,
) -> Result<(), RunPodLocalTrainingBinderProjectionError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RunPodLocalTrainingBinderProjectionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let projection = canonical_runpod_local_training_binder_projection_set()?;
    let json = serde_json::to_vec_pretty(&projection)?;
    fs::write(output_path, json).map_err(|error| {
        RunPodLocalTrainingBinderProjectionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn projection_for_binding(
    binding: &crate::CrossProviderRuntimeBindingRecord,
    lane_kind: RunPodLocalTrainingBinderLaneKind,
) -> RunPodLocalTrainingBinderLaneProjection {
    let (
        lane_projection_id,
        runbook_path,
        retained_authority_paths,
        launch_script_path,
        startup_script_path,
        finalizer_script_path,
        retained_checker_paths,
        retained_evidence_paths,
        claim_boundary,
    ) = match lane_kind {
        RunPodLocalTrainingBinderLaneKind::RunPodDistributedEightH100 => (
            String::from("runpod_8xh100_parameter_golf"),
            String::from(RUNPOD_RUNBOOK_PATH),
            vec![
                String::from(RUNPOD_LAUNCH_PROFILES_PATH),
                String::from(RUNPOD_OPERATOR_PREFLIGHT_PATH),
                String::from(RUNPOD_COST_GUARDRAILS_PATH),
            ],
            String::from(RUNPOD_LAUNCH_SCRIPT_PATH),
            None,
            String::from(RUNPOD_FINALIZER_SCRIPT_PATH),
            vec![String::from(RUNPOD_LANE_CHECKER_PATH)],
            vec![
                String::from("parameter_golf_runpod_8xh100_launch_manifest.json"),
                String::from("parameter_golf_runpod_8xh100_launch_receipt.json"),
                String::from("training_visualization/parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json"),
            ],
            String::from(
                "The RunPod 8xH100 lane still keeps its bounded operator and finalizer posture, but the launch, runtime env, artifact roots, and final evidence expectations now come from the shared binder instead of a RunPod-only training API.",
            ),
        ),
        RunPodLocalTrainingBinderLaneKind::LocalTrustedLanSwarm => (
            String::from("local_trusted_lan_first_swarm"),
            String::from(LOCAL_TRUSTED_LAN_RUNBOOK_PATH),
            vec![
                String::from(LOCAL_TOPOLOGY_CONTRACT_PATH),
                String::from(LOCAL_FAILURE_DRILLS_PATH),
                String::from(LOCAL_WORKFLOW_PLAN_PATH),
                String::from(LOCAL_MAC_BRINGUP_PATH),
                String::from(LOCAL_LINUX_BRINGUP_PATH),
            ],
            String::from(LOCAL_LAUNCH_SCRIPT_PATH),
            None,
            String::from(
                "cargo run -q -p psionic-train --bin first_swarm_trusted_lan_closeout_report",
            ),
            vec![
                String::from(LOCAL_TRUSTED_LAN_CHECKER_PATH),
                String::from(LOCAL_TRUSTED_LAN_REHEARSAL_CHECKER_PATH),
                String::from(LOCAL_TRUSTED_LAN_EVIDENCE_CHECKER_PATH),
                String::from(LOCAL_TRUSTED_LAN_CLOSEOUT_CHECKER_PATH),
            ],
            vec![
                String::from("fixtures/swarm/reports/first_swarm_trusted_lan_rehearsal_v1.json"),
                String::from("fixtures/swarm/reports/first_swarm_trusted_lan_evidence_bundle_v1.json"),
                String::from("fixtures/swarm/reports/first_swarm_trusted_lan_closeout_v1.json"),
            ],
            String::from(
                "The first local trusted-LAN lane still keeps its bounded mixed-hardware swarm posture, but launch, runtime env, artifact roots, and closeout expectations now come from the shared binder instead of lane-local training truth.",
            ),
        ),
    };
    let mut projection = RunPodLocalTrainingBinderLaneProjection {
        lane_projection_id,
        lane_kind,
        runtime_binding_id: binding.binding_id.clone(),
        runtime_binding_digest: binding.binding_digest.clone(),
        launch_contract_id: binding.launch_contract_id.clone(),
        runbook_path,
        retained_authority_paths,
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
    hasher.update(
        serde_json::to_vec(value).expect("runpod local training binder values must serialize"),
    );
    format!("{:x}", hasher.finalize())
}
