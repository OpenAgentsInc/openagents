use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    OPEN_ADAPTER_CUDA_BACKEND_LABEL, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
    SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH, SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
};

/// Stable schema version for the executor admitted-profile catalog.
pub const PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_SCHEMA_VERSION: &str =
    "psion.executor_admitted_profile_catalog.v1";
/// Canonical fixture path for the admitted-profile catalog.
pub const PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_admitted_profiles_v1.json";
/// Canonical doc path for the admitted local profiles.
pub const PSION_EXECUTOR_LOCAL_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_PROFILE_REFERENCE.md";

const CROSS_PROVIDER_LOCAL_MLX_MAC_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/local_mlx_mac_workstation_v1.json";
const CROSS_PROVIDER_LOCAL_RTX4080_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/local_rtx4080_workstation_v1.json";
const FIRST_SWARM_TAILNET_RUN_SUMMARY_FIXTURE_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/tailrun_admitted_home_run_summary.json";
const FIRST_SWARM_TAILNET_OPERATOR_MANIFEST_FIXTURE_PATH: &str =
    "fixtures/swarm/runs/tailrun-home-admitted-20260327e/operator_manifest.json";
const FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/swarm/first_swarm_trusted_lan_topology_contract_v1.json";
const TAILNET_SHORT_RUN_DEVICE_AUDIT_PATH: &str =
    "docs/audits/2026-03-27-tailnet-short-run-device-audit.md";
const TAILRUN_ADMITTED_HOME_TAILNET_AUDIT_PATH: &str =
    "docs/audits/2026-03-27-tailrun-admitted-home-tailnet-run-audit.md";

/// Run types admitted by the phase-one executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorRunType {
    MlxSmoke,
    MlxDecisionGrade,
    Cuda4080Smoke,
    Cuda4080DecisionGrade,
    Cuda4080Confirmation,
    CpuValidation,
    H100Escalation,
}

/// Admission posture for one run type on one admitted profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionExecutorRunTypeAdmissionPosture {
    Primary,
    Allowed,
    NotAdmitted,
}

/// One run-type admission row for one profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorRunTypeAdmission {
    /// Stable run type id.
    pub run_type: PsionExecutorRunType,
    /// Final admission posture on this profile.
    pub posture: PsionExecutorRunTypeAdmissionPosture,
    /// Short operator-facing detail.
    pub detail: String,
}

/// Repo-owned authority artifact used to admit one executor profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorAuthorityArtifact {
    /// Repo-local path.
    pub path: String,
    /// Stable SHA256 over the artifact bytes.
    pub sha256: String,
    /// Why the artifact matters to this profile.
    pub detail: String,
}

/// Expected same-lane throughput band retained for one admitted profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorThroughputBand {
    /// Stable metric id.
    pub metric_id: String,
    /// Minimum retained value still considered admitted.
    pub minimum_value: f64,
    /// Expected current retained value.
    pub expected_value: f64,
    /// Maximum retained value used for the band description.
    pub maximum_value: f64,
    /// Short operator-facing detail.
    pub detail: String,
}

/// Explicit controller/worker split retained for one admitted control-plane profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorResponsibilitySplit {
    /// Responsibilities frozen for the controller host.
    pub controller_responsibilities: Vec<String>,
    /// Responsibilities frozen for the worker host.
    pub worker_responsibilities: Vec<String>,
}

/// One admitted executor profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorAdmittedProfile {
    /// Stable profile id.
    pub profile_id: String,
    /// Short purpose statement.
    pub purpose: String,
    /// Runtime backend label if the profile maps to one backend directly.
    pub runtime_backend_label: Option<String>,
    /// Admitted run-type posture on this profile.
    pub run_type_admissions: Vec<PsionExecutorRunTypeAdmission>,
    /// Local operator requirements that must remain true before a run counts.
    pub local_requirements: Vec<String>,
    /// Checkpoint expectations for this profile.
    pub checkpoint_expectations: String,
    /// Connectivity expectations for this profile.
    pub connectivity_expectations: Vec<String>,
    /// Shipped entrypoints this profile is allowed to use.
    pub shipped_entrypoints: Vec<String>,
    /// Authority artifacts proving the profile boundary.
    pub authority_artifacts: Vec<PsionExecutorAuthorityArtifact>,
    /// Retained throughput band when the profile has one.
    pub throughput_band: Option<PsionExecutorThroughputBand>,
    /// Explicit controller/worker split when the profile admits a control plane.
    pub responsibility_split: Option<PsionExecutorResponsibilitySplit>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the profile.
    pub profile_digest: String,
}

/// Catalog of admitted phase-one executor profiles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorAdmittedProfileCatalog {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable catalog id.
    pub catalog_id: String,
    /// Ordered admitted profiles.
    pub profiles: Vec<PsionExecutorAdmittedProfile>,
    /// Short explanation of the catalog.
    pub summary: String,
    /// Stable digest over the catalog.
    pub catalog_digest: String,
}

impl PsionExecutorAdmittedProfileCatalog {
    /// Validate catalog structure and digests.
    pub fn validate(&self) -> Result<(), PsionExecutorAdmittedProfileError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_admitted_profile_catalog.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_SCHEMA_VERSION {
            return Err(PsionExecutorAdmittedProfileError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.catalog_id.as_str(),
            "psion_executor_admitted_profile_catalog.catalog_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_executor_admitted_profile_catalog.summary",
        )?;
        if self.profiles.is_empty() {
            return Err(PsionExecutorAdmittedProfileError::MissingField {
                field: String::from("psion_executor_admitted_profile_catalog.profiles"),
            });
        }
        let mut seen_profiles = BTreeSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            if !seen_profiles.insert(profile.profile_id.as_str()) {
                return Err(PsionExecutorAdmittedProfileError::DuplicateProfile {
                    profile_id: profile.profile_id.clone(),
                });
            }
        }
        if self.catalog_digest != stable_executor_profile_catalog_digest(self) {
            return Err(PsionExecutorAdmittedProfileError::DigestMismatch {
                kind: String::from("psion_executor_admitted_profile_catalog"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorAdmittedProfile {
    fn validate(&self) -> Result<(), PsionExecutorAdmittedProfileError> {
        ensure_nonempty(
            self.profile_id.as_str(),
            "psion_executor_admitted_profile.profile_id",
        )?;
        ensure_nonempty(
            self.purpose.as_str(),
            "psion_executor_admitted_profile.purpose",
        )?;
        if self.run_type_admissions.is_empty() {
            return Err(PsionExecutorAdmittedProfileError::MissingField {
                field: format!(
                    "psion_executor_admitted_profile[{}].run_type_admissions",
                    self.profile_id
                ),
            });
        }
        let mut seen_run_types = BTreeSet::new();
        for admission in &self.run_type_admissions {
            ensure_nonempty(
                admission.detail.as_str(),
                "psion_executor_admitted_profile.run_type_admissions[].detail",
            )?;
            if !seen_run_types.insert(admission.run_type) {
                return Err(
                    PsionExecutorAdmittedProfileError::DuplicateRunTypeAdmission {
                        profile_id: self.profile_id.clone(),
                        run_type: format!("{:?}", admission.run_type),
                    },
                );
            }
        }
        if self.local_requirements.is_empty() {
            return Err(PsionExecutorAdmittedProfileError::MissingField {
                field: format!(
                    "psion_executor_admitted_profile[{}].local_requirements",
                    self.profile_id
                ),
            });
        }
        for requirement in &self.local_requirements {
            ensure_nonempty(
                requirement.as_str(),
                "psion_executor_admitted_profile.local_requirements[]",
            )?;
        }
        ensure_nonempty(
            self.checkpoint_expectations.as_str(),
            "psion_executor_admitted_profile.checkpoint_expectations",
        )?;
        if self.connectivity_expectations.is_empty() {
            return Err(PsionExecutorAdmittedProfileError::MissingField {
                field: format!(
                    "psion_executor_admitted_profile[{}].connectivity_expectations",
                    self.profile_id
                ),
            });
        }
        for expectation in &self.connectivity_expectations {
            ensure_nonempty(
                expectation.as_str(),
                "psion_executor_admitted_profile.connectivity_expectations[]",
            )?;
        }
        if self.shipped_entrypoints.is_empty() {
            return Err(PsionExecutorAdmittedProfileError::MissingField {
                field: format!(
                    "psion_executor_admitted_profile[{}].shipped_entrypoints",
                    self.profile_id
                ),
            });
        }
        for entrypoint in &self.shipped_entrypoints {
            ensure_nonempty(
                entrypoint.as_str(),
                "psion_executor_admitted_profile.shipped_entrypoints[]",
            )?;
        }
        if self.authority_artifacts.is_empty() {
            return Err(PsionExecutorAdmittedProfileError::MissingField {
                field: format!(
                    "psion_executor_admitted_profile[{}].authority_artifacts",
                    self.profile_id
                ),
            });
        }
        for artifact in &self.authority_artifacts {
            ensure_nonempty(
                artifact.path.as_str(),
                "psion_executor_admitted_profile.authority_artifacts[].path",
            )?;
            ensure_nonempty(
                artifact.sha256.as_str(),
                "psion_executor_admitted_profile.authority_artifacts[].sha256",
            )?;
            ensure_nonempty(
                artifact.detail.as_str(),
                "psion_executor_admitted_profile.authority_artifacts[].detail",
            )?;
        }
        if let Some(throughput_band) = &self.throughput_band {
            ensure_nonempty(
                throughput_band.metric_id.as_str(),
                "psion_executor_admitted_profile.throughput_band.metric_id",
            )?;
            ensure_nonempty(
                throughput_band.detail.as_str(),
                "psion_executor_admitted_profile.throughput_band.detail",
            )?;
            if !(throughput_band.minimum_value <= throughput_band.expected_value
                && throughput_band.expected_value <= throughput_band.maximum_value)
            {
                return Err(PsionExecutorAdmittedProfileError::InvalidThroughputBand {
                    profile_id: self.profile_id.clone(),
                });
            }
        }
        if let Some(responsibility_split) = &self.responsibility_split {
            if responsibility_split.controller_responsibilities.is_empty() {
                return Err(PsionExecutorAdmittedProfileError::MissingField {
                    field: format!(
                        "psion_executor_admitted_profile[{}].responsibility_split.controller_responsibilities",
                        self.profile_id
                    ),
                });
            }
            if responsibility_split.worker_responsibilities.is_empty() {
                return Err(PsionExecutorAdmittedProfileError::MissingField {
                    field: format!(
                        "psion_executor_admitted_profile[{}].responsibility_split.worker_responsibilities",
                        self.profile_id
                    ),
                });
            }
            for responsibility in &responsibility_split.controller_responsibilities {
                ensure_nonempty(
                    responsibility.as_str(),
                    "psion_executor_admitted_profile.responsibility_split.controller_responsibilities[]",
                )?;
            }
            for responsibility in &responsibility_split.worker_responsibilities {
                ensure_nonempty(
                    responsibility.as_str(),
                    "psion_executor_admitted_profile.responsibility_split.worker_responsibilities[]",
                )?;
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_executor_admitted_profile.claim_boundary",
        )?;
        if self.profile_digest != stable_executor_profile_digest(self) {
            return Err(PsionExecutorAdmittedProfileError::DigestMismatch {
                kind: format!("psion_executor_admitted_profile.{}", self.profile_id),
            });
        }
        Ok(())
    }
}

/// Errors surfaced while building or validating executor admitted profiles.
#[derive(Debug, Error)]
pub enum PsionExecutorAdmittedProfileError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("schema version mismatch: expected `{expected}`, got `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("duplicate profile `{profile_id}`")]
    DuplicateProfile { profile_id: String },
    #[error("duplicate run-type admission `{run_type}` in profile `{profile_id}`")]
    DuplicateRunTypeAdmission {
        profile_id: String,
        run_type: String,
    },
    #[error("invalid throughput band in profile `{profile_id}`")]
    InvalidThroughputBand { profile_id: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
}

/// Build the current canonical executor admitted-profile catalog.
pub fn builtin_executor_admitted_profile_catalog(
    workspace_root: &Path,
) -> Result<PsionExecutorAdmittedProfileCatalog, PsionExecutorAdmittedProfileError> {
    let profiles = vec![
        builtin_local_mac_mlx_profile(workspace_root)?,
        builtin_local_4080_cuda_tailnet_profile(workspace_root)?,
        builtin_local_tailnet_cluster_control_plane_profile(workspace_root)?,
    ];
    let mut catalog = PsionExecutorAdmittedProfileCatalog {
        schema_version: String::from(PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_SCHEMA_VERSION),
        catalog_id: String::from("psion_executor_admitted_profiles_v1"),
        profiles,
        summary: String::from(
            "Phase-one executor admitted-profile catalog freezing the local Mac MLX lane, the admitted RTX 4080 Tailnet worker lane, and the bounded Mac-to-4080 control-plane roundtrip before later EPICs widen the same catalog.",
        ),
        catalog_digest: String::new(),
    };
    catalog.catalog_digest = stable_executor_profile_catalog_digest(&catalog);
    catalog.validate()?;
    Ok(catalog)
}

/// Write the current admitted-profile catalog fixture.
pub fn write_builtin_executor_admitted_profile_catalog(
    workspace_root: &Path,
) -> Result<PsionExecutorAdmittedProfileCatalog, PsionExecutorAdmittedProfileError> {
    let catalog = builtin_executor_admitted_profile_catalog(workspace_root)?;
    let fixture_path = workspace_root.join(PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_FIXTURE_PATH);
    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorAdmittedProfileError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&fixture_path, serde_json::to_vec_pretty(&catalog)?).map_err(|error| {
        PsionExecutorAdmittedProfileError::Write {
            path: fixture_path.display().to_string(),
            error,
        }
    })?;
    Ok(catalog)
}

fn builtin_local_mac_mlx_profile(
    workspace_root: &Path,
) -> Result<PsionExecutorAdmittedProfile, PsionExecutorAdmittedProfileError> {
    let authority_artifacts = vec![
        authority_artifact(
            workspace_root,
            SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
            "Retained MLX bring-up report proving the bounded Mac Metal backend, machine envelope, and same-node bring-up gate.",
        )?,
        authority_artifact(
            workspace_root,
            CROSS_PROVIDER_LOCAL_MLX_MAC_COMPUTE_SOURCE_FIXTURE_PATH,
            "Shared compute-source contract proving the Mac can be admitted under the existing cross-provider train substrate without inventing a second launcher or machine vocabulary.",
        )?,
    ];
    let mut profile = PsionExecutorAdmittedProfile {
        profile_id: String::from("local_mac_mlx_aarch64"),
        purpose: String::from(
            "Local Apple Silicon MLX machine for executor smoke training, short training runs, eval-pack execution, checkpoint restore rehearsal, export inspection, and CPU-validation ownership.",
        ),
        runtime_backend_label: Some(String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL)),
        run_type_admissions: vec![
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::MlxSmoke,
                posture: PsionExecutorRunTypeAdmissionPosture::Primary,
                detail: String::from(
                    "This is the primary admitted machine for MLX smoke runs and local bring-up validation.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::MlxDecisionGrade,
                posture: PsionExecutorRunTypeAdmissionPosture::Allowed,
                detail: String::from(
                    "Decision-grade MLX runs are allowed only when the question is explicitly MLX-local and the frozen pack or subset is declared before launch.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080Smoke,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The Mac profile does not count as the CUDA smoke lane.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080DecisionGrade,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The Mac profile does not count as the 4080 decision-grade lane.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080Confirmation,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The Mac profile does not count as the 4080 confirmation lane.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::CpuValidation,
                posture: PsionExecutorRunTypeAdmissionPosture::Primary,
                detail: String::from(
                    "The Mac profile is a primary owner for CPU-matrix replacement validation, export inspection, and restore rehearsal.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::H100Escalation,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The local Mac profile does not count as the H100 escalation lane.",
                ),
            },
        ],
        local_requirements: vec![
            String::from(
                "Run the retained MLX bring-up gate before roadmap-tracked smoke or decision-grade work.",
            ),
            String::from(
                "Keep the run local-first: this profile does not claim remote checkpoint-writer or remote cluster-port authority by itself.",
            ),
            String::from(
                "Use only shipped Psionic entrypoints when the profile is cited in roadmap-tracked evidence.",
            ),
        ],
        checkpoint_expectations: String::from(
            "Checkpoint restore rehearsal and export inspection must stay inside the local operator-owned workspace; this profile does not claim shared checkpoint-writer authority by itself.",
        ),
        connectivity_expectations: vec![
            String::from(
                "No remote launch or cluster-port claim is inherited from the Mac profile alone.",
            ),
            String::from(
                "The profile remains valid for local eval and control-plane ownership even when the later Tailnet worker is offline.",
            ),
        ],
        shipped_entrypoints: vec![
            String::from("scripts/check-swarm-mac-mlx-bringup.sh"),
            String::from("crates/psionic-train/src/swarm_mlx_bringup.rs"),
            String::from("crates/psionic-train/src/bin/swarm_mac_mlx_bringup.rs"),
        ],
        authority_artifacts,
        throughput_band: None,
        responsibility_split: None,
        claim_boundary: String::from(
            "This profile admits the local Apple Silicon MLX machine as a real roadmap-tracked executor development and eval host. It proves local MLX smoke, short-run, eval, restore, export, and CPU-validation posture. It does not by itself claim remote launch, shared checkpoint authority, or cross-device training closure.",
        ),
        profile_digest: String::new(),
    };
    profile.profile_digest = stable_executor_profile_digest(&profile);
    profile.validate()?;
    Ok(profile)
}

fn builtin_local_4080_cuda_tailnet_profile(
    workspace_root: &Path,
) -> Result<PsionExecutorAdmittedProfile, PsionExecutorAdmittedProfileError> {
    let authority_artifacts = vec![
        authority_artifact(
            workspace_root,
            SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH,
            "Retained Linux RTX 4080 bring-up report proving the CUDA-labeled worker contract, inventory, and bounded contributor harness.",
        )?,
        authority_artifact(
            workspace_root,
            CROSS_PROVIDER_LOCAL_RTX4080_COMPUTE_SOURCE_FIXTURE_PATH,
            "Shared compute-source contract freezing the admitted RTX 4080 workstation under the existing cross-provider train substrate.",
        )?,
        authority_artifact(
            workspace_root,
            FIRST_SWARM_TAILNET_RUN_SUMMARY_FIXTURE_PATH,
            "Retained Tailnet admitted-run summary proving the Mac-plus-4080 device set and the current worker contribution path.",
        )?,
        authority_artifact(
            workspace_root,
            TAILNET_SHORT_RUN_DEVICE_AUDIT_PATH,
            "Retained short-run device audit recording the current honest 4080 same-node throughput band and its current comparison against the local M5 lane.",
        )?,
    ];
    let mut profile = PsionExecutorAdmittedProfile {
        profile_id: String::from("local_4080_cuda_tailnet_x86_64"),
        purpose: String::from(
            "Operator-selected RTX 4080 CUDA Tailnet worker for bounded smoke, decision-grade, confirmation, eval, and replay-accounted accelerator work inside the admitted local-first executor lane.",
        ),
        runtime_backend_label: Some(String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL)),
        run_type_admissions: vec![
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::MlxSmoke,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The 4080 profile does not count as the MLX smoke lane.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::MlxDecisionGrade,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The 4080 profile does not count as the MLX decision-grade lane.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080Smoke,
                posture: PsionExecutorRunTypeAdmissionPosture::Primary,
                detail: String::from(
                    "This is the primary admitted accelerator for 4080 smoke runs.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080DecisionGrade,
                posture: PsionExecutorRunTypeAdmissionPosture::Primary,
                detail: String::from(
                    "This is the primary admitted accelerator for 4080 decision-grade runs once the frozen pack and artifact packet are declared before launch.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080Confirmation,
                posture: PsionExecutorRunTypeAdmissionPosture::Primary,
                detail: String::from(
                    "This is the primary admitted accelerator for 4080 confirmation reruns after a decision-grade candidate already clears.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::CpuValidation,
                posture: PsionExecutorRunTypeAdmissionPosture::Allowed,
                detail: String::from(
                    "The 4080 lane can participate in cross-check validation, but CPU-matrix ownership still sits with the Mac profile.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::H100Escalation,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The admitted 4080 worker does not count as the H100 escalation lane.",
                ),
            },
        ],
        local_requirements: vec![
            String::from(
                "Run the retained RTX 4080 bring-up gate before roadmap-tracked smoke, decision-grade, or confirmation work counts.",
            ),
            String::from(
                "Use only the admitted Tailnet operator path and shipped Psionic entrypoints; no ad hoc remote wrapper or elastic membership path counts.",
            ),
            String::from(
                "Keep the work inside the bounded Rust-only same-node and admitted Tailnet lane; this profile does not widen into dense distributed training.",
            ),
        ],
        checkpoint_expectations: String::from(
            "Worker-local scratch may live under the staged remote bundle root such as `$HOME/code/psionic-tailrun/<run_id>/linux`, but retained checkpoints and any roadmap-counting artifact packet must return through the controller-owned bundle under `fixtures/swarm/runs/<run_id>/`.",
        ),
        connectivity_expectations: vec![
            String::from(
                "Tailnet SSH reachability to `archlinux` is required before the 4080 worker profile is considered admitted for a counted run.",
            ),
            String::from(
                "The controller-selected coordinator and contributor ports must remain explicit and operator-owned; this profile does not claim wider-network discovery or autonomous launch authority.",
            ),
            String::from(
                "If the Tailnet worker is offline, the profile remains documented but does not count for smoke, decision-grade, or confirmation evidence until the admitted path is reachable again.",
            ),
        ],
        shipped_entrypoints: vec![
            String::from("scripts/check-swarm-linux-4080-bringup.sh"),
            String::from("scripts/run-first-swarm-tailnet-admitted-live.sh"),
            String::from("crates/psionic-train/src/swarm_cuda_bringup.rs"),
            String::from("crates/psionic-train/src/bin/swarm_linux_cuda_bringup.rs"),
        ],
        authority_artifacts,
        throughput_band: Some(PsionExecutorThroughputBand {
            metric_id: String::from("same_node_open_adapter_steps_per_second"),
            minimum_value: 80.0,
            expected_value: 122.8920,
            maximum_value: 125.0,
            detail: String::from(
                "The retained 2026-03-27 short-run audit freezes the honest current 4080 same-node band: 82.4025 steps/s before the retained fix and 122.8920 steps/s after it.",
            ),
        }),
        responsibility_split: None,
        claim_boundary: String::from(
            "This profile admits one reachable RTX 4080 CUDA Tailnet worker as the real local accelerator lane for bounded smoke, decision-grade, confirmation, and replay-accounted eval work. It does not claim dense CUDA training closure, public-worker authority, shared checkpoint-writer authority, or independent promotion authority.",
        ),
        profile_digest: String::new(),
    };
    profile.profile_digest = stable_executor_profile_digest(&profile);
    profile.validate()?;
    Ok(profile)
}

fn builtin_local_tailnet_cluster_control_plane_profile(
    workspace_root: &Path,
) -> Result<PsionExecutorAdmittedProfile, PsionExecutorAdmittedProfileError> {
    let authority_artifacts = vec![
        authority_artifact(
            workspace_root,
            FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH,
            "Trusted-LAN topology contract freezing the Mac coordinator, the 4080 contributor, artifact staging roots, and launch sequencing for the bounded mixed-hardware lane.",
        )?,
        authority_artifact(
            workspace_root,
            FIRST_SWARM_TAILNET_OPERATOR_MANIFEST_FIXTURE_PATH,
            "Retained operator manifest proving the controller-selected endpoints, bundle root, and exact Tailnet run identity for the admitted Mac-to-4080 roundtrip.",
        )?,
        authority_artifact(
            workspace_root,
            FIRST_SWARM_TAILNET_RUN_SUMMARY_FIXTURE_PATH,
            "Retained Tailnet run summary proving both admitted nodes contributed inside one real bounded run.",
        )?,
        authority_artifact(
            workspace_root,
            TAILRUN_ADMITTED_HOME_TAILNET_AUDIT_PATH,
            "Retained audit describing the exact controller workflow, contributor workflow, and returned artifact packet for the first admitted home-Tailnet run.",
        )?,
    ];
    let mut profile = PsionExecutorAdmittedProfile {
        profile_id: String::from("local_tailnet_cluster_control_plane"),
        purpose: String::from(
            "Mac-as-controller plus admitted RTX 4080 Tailnet worker roundtrip for bounded mixed-hardware executor runs, retained artifact return, and controller-owned validation inside the local-first lane.",
        ),
        runtime_backend_label: None,
        run_type_admissions: vec![
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::MlxSmoke,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The control-plane profile is not needed for same-node MLX smoke work.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::MlxDecisionGrade,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The control-plane profile does not count as the MLX-only decision-grade lane.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080Smoke,
                posture: PsionExecutorRunTypeAdmissionPosture::Allowed,
                detail: String::from(
                    "The control plane is the admitted coordinator surface for Tailnet-backed 4080 smoke work, but the worker-machine authority still lives in the 4080 profile.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080DecisionGrade,
                posture: PsionExecutorRunTypeAdmissionPosture::Allowed,
                detail: String::from(
                    "The control plane is allowed for decision-grade 4080 runs when the full Mac -> 4080 -> Mac bundle path is the evidence carrier.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::Cuda4080Confirmation,
                posture: PsionExecutorRunTypeAdmissionPosture::Allowed,
                detail: String::from(
                    "The control plane is allowed for confirmation reruns when the controller replays the same admitted worker path and bundle return rules.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::CpuValidation,
                posture: PsionExecutorRunTypeAdmissionPosture::Allowed,
                detail: String::from(
                    "The control plane may own bundle collection and review while the Mac profile remains the CPU-validation truth anchor.",
                ),
            },
            PsionExecutorRunTypeAdmission {
                run_type: PsionExecutorRunType::H100Escalation,
                posture: PsionExecutorRunTypeAdmissionPosture::NotAdmitted,
                detail: String::from(
                    "The local Tailnet control plane does not count as the H100 escalation lane.",
                ),
            },
        ],
        local_requirements: vec![
            String::from(
                "Counted control-plane runs require both admitted machine profiles to stay green before launch.",
            ),
            String::from(
                "Use the shipped Tailnet operator script and bounded first-swarm runtime path instead of inventing a second launcher or planner.",
            ),
            String::from(
                "The retained bundle under `fixtures/swarm/runs/<run_id>/` is the evidence carrier; remote scratch alone never counts as the final artifact packet.",
            ),
        ],
        checkpoint_expectations: String::from(
            "The Mac controller owns the retained artifact packet: it stages the run, launches the remote worker, pulls back contributor outputs, writes the bundle under `fixtures/swarm/runs/<run_id>/`, and only then counts the run. Remote scratch remains staging space, not the canonical final artifact home.",
        ),
        connectivity_expectations: vec![
            String::from(
                "The controller must resolve both the local and remote Tailnet IPv4 endpoints and bind explicit coordinator and contributor ports before launch.",
            ),
            String::from(
                "The control plane remains trusted-LAN-only with no internet discovery, no elastic membership, and no hidden scheduler above the admitted workflow plan.",
            ),
            String::from(
                "If the worker path drops before the bundle returns to the controller-owned run root, the run stays non-counting regardless of remote partial outputs.",
            ),
        ],
        shipped_entrypoints: vec![
            String::from("scripts/run-first-swarm-tailnet-admitted-live.sh"),
            String::from("scripts/check-first-swarm-trusted-lan-real-run.sh"),
            String::from("crates/psionic-train/src/swarm_first_live_runtime.rs"),
            String::from("crates/psionic-train/src/swarm_trusted_lan.rs"),
        ],
        authority_artifacts,
        throughput_band: None,
        responsibility_split: Some(PsionExecutorResponsibilitySplit {
            controller_responsibilities: vec![
                String::from(
                    "Materialize the operator manifest and exact bundle root before either node starts the counted run.",
                ),
                String::from(
                    "Select and publish the explicit Tailnet endpoints plus coordinator and contributor ports for the run.",
                ),
                String::from(
                    "Own validator-visible aggregation, replay-accounted bundle assembly, and final artifact retention under `fixtures/swarm/runs/<run_id>/`.",
                ),
            ],
            worker_responsibilities: vec![
                String::from(
                    "Execute only the bounded contributor role on the admitted RTX 4080 worker with the shipped runtime path.",
                ),
                String::from(
                    "Return contributor reports and staged artifacts to the controller-selected bundle path instead of claiming local publish authority.",
                ),
                String::from(
                    "Stay inside the admitted Tailnet lane with no independent scheduler, promotion, or validator authority.",
                ),
            ],
        }),
        claim_boundary: String::from(
            "This profile admits the bounded Mac-to-4080 control-plane roundtrip as the real local-first cluster workflow for the executor lane. It proves controller responsibilities, worker responsibilities, and the controller-owned artifact return path. It does not claim elastic cluster management, public-worker orchestration, independent worker publish authority, or a second training control plane beyond the shipped first-swarm runtime surfaces.",
        ),
        profile_digest: String::new(),
    };
    profile.profile_digest = stable_executor_profile_digest(&profile);
    profile.validate()?;
    Ok(profile)
}

fn authority_artifact(
    workspace_root: &Path,
    rel_path: &str,
    detail: &str,
) -> Result<PsionExecutorAuthorityArtifact, PsionExecutorAdmittedProfileError> {
    Ok(PsionExecutorAuthorityArtifact {
        path: String::from(rel_path),
        sha256: sha256_for_path(workspace_root.join(rel_path))?,
        detail: String::from(detail),
    })
}

fn sha256_for_path(path: PathBuf) -> Result<String, PsionExecutorAdmittedProfileError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorAdmittedProfileError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorAdmittedProfileError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorAdmittedProfileError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_executor_profile_digest(profile: &PsionExecutorAdmittedProfile) -> String {
    let mut clone = profile.clone();
    clone.profile_digest.clear();
    stable_json_digest(&clone)
}

fn stable_executor_profile_catalog_digest(catalog: &PsionExecutorAdmittedProfileCatalog) -> String {
    let mut clone = catalog.clone();
    clone.catalog_digest.clear();
    stable_json_digest(&clone)
}

fn stable_json_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("executor admitted-profile digest serialization");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
            .expect("workspace root")
    }

    #[test]
    fn builtin_executor_profile_catalog_matches_committed_fixture() {
        let root = workspace_root();
        let built = builtin_executor_admitted_profile_catalog(&root).expect("built catalog");
        let fixture: PsionExecutorAdmittedProfileCatalog = serde_json::from_slice(
            &fs::read(root.join(PSION_EXECUTOR_ADMITTED_PROFILE_CATALOG_FIXTURE_PATH))
                .expect("fixture bytes"),
        )
        .expect("fixture json");
        assert_eq!(built, fixture);
    }

    #[test]
    fn builtin_executor_profile_catalog_is_valid() {
        let root = workspace_root();
        let catalog = builtin_executor_admitted_profile_catalog(&root).expect("catalog");
        catalog.validate().expect("catalog should validate");
        assert_eq!(catalog.profiles.len(), 3);
        assert_eq!(catalog.profiles[0].profile_id, "local_mac_mlx_aarch64");
        assert_eq!(
            catalog.profiles[1].profile_id,
            "local_4080_cuda_tailnet_x86_64"
        );
        assert_eq!(
            catalog.profiles[2].profile_id,
            "local_tailnet_cluster_control_plane"
        );
    }
}
