use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, cross_provider_training_program_manifest,
    CrossProviderBackendFamily, CrossProviderComputeProviderKind, CrossProviderComputeSourceClass,
    CrossProviderExecutionClass, PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH,
    PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
};

/// Stable schema version for the provider-neutral launch-contract family.
pub const CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.cross_provider_launch_contract.v1";
/// Stable fixture directory for launch contracts.
pub const CROSS_PROVIDER_LAUNCH_CONTRACT_FIXTURE_DIR: &str = "fixtures/training/launch_contracts";
/// Stable Google single-node launch-contract fixture path.
pub const CROSS_PROVIDER_GOOGLE_SINGLE_NODE_LAUNCH_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/launch_contracts/google_single_node_accelerated_v1.json";
/// Stable Google two-node swarm launch-contract fixture path.
pub const CROSS_PROVIDER_GOOGLE_SWARM_LAUNCH_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/launch_contracts/google_two_node_swarm_v1.json";
/// Stable RunPod launch-contract fixture path.
pub const CROSS_PROVIDER_RUNPOD_LAUNCH_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/launch_contracts/runpod_8xh100_v1.json";
/// Stable local trusted-LAN launch-contract fixture path.
pub const CROSS_PROVIDER_LOCAL_SWARM_LAUNCH_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/launch_contracts/local_first_swarm_v1.json";
/// Stable reference doc path for the launch-contract family.
pub const CROSS_PROVIDER_LAUNCH_CONTRACT_DOC_PATH: &str = "docs/LAUNCH_CONTRACT_REFERENCE.md";
/// Stable checker path for the launch-contract family.
pub const CROSS_PROVIDER_LAUNCH_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-provider-launch-contracts.sh";

const GOOGLE_SINGLE_NODE_LAUNCH_PROFILES_PATH: &str =
    "fixtures/psion/google/psion_google_single_node_launch_profiles_v1.json";
const GOOGLE_SINGLE_NODE_OBSERVABILITY_POLICY_PATH: &str =
    "fixtures/psion/google/psion_google_host_observability_policy_v1.json";
const GOOGLE_SINGLE_NODE_LAUNCH_SCRIPT_PATH: &str = "scripts/psion-google-launch-single-node.sh";
const GOOGLE_SINGLE_NODE_STARTUP_SCRIPT_PATH: &str = "scripts/psion-google-single-node-startup.sh";
const GOOGLE_SINGLE_NODE_FINALIZER_SCRIPT_PATH: &str = "scripts/psion-google-finalize-run.sh";

const GOOGLE_SWARM_LAUNCH_SCRIPT_PATH: &str = "scripts/psion-google-launch-two-node-swarm.sh";
const GOOGLE_SWARM_STARTUP_SCRIPT_PATH: &str = "scripts/psion-google-two-node-swarm-startup.sh";
const GOOGLE_SWARM_FINALIZER_SCRIPT_PATH: &str =
    "scripts/psion-google-finalize-two-node-swarm-run.sh";

const RUNPOD_LAUNCH_PROFILES_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_launch_profiles_v1.json";
const RUNPOD_LAUNCH_SCRIPT_PATH: &str = "scripts/parameter-golf-runpod-launch-8xh100.sh";
const RUNPOD_FINALIZER_SCRIPT_PATH: &str = "scripts/parameter-golf-runpod-finalize-8xh100.sh";

const LOCAL_SWARM_TOPOLOGY_CONTRACT_PATH: &str =
    "fixtures/swarm/first_swarm_trusted_lan_topology_contract_v1.json";
const LOCAL_SWARM_WORKFLOW_PLAN_PATH: &str =
    "fixtures/swarm/first_swarm_live_workflow_plan_v1.json";
const LOCAL_SWARM_LAUNCH_SCRIPT_PATH: &str = "scripts/first-swarm-launch-trusted-lan.sh";
const LOCAL_SWARM_CLOSEOUT_BIN: &str =
    "cargo run -q -p psionic-train --bin first_swarm_trusted_lan_closeout_report -- ${PSION_FINAL_ROOT}/first_swarm_trusted_lan_closeout_v1.json";

/// Errors surfaced while building or writing launch contracts.
#[derive(Debug, Error)]
pub enum CrossProviderLaunchContractError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("launch contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Launcher binding family behind one provider-neutral launch contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderLaunchBinderKind {
    /// Google single-node launch, startup, and finalizer path.
    GoogleSingleNode,
    /// Google two-node configured-peer swarm path.
    GoogleTwoNodeSwarm,
    /// RunPod single-pod path.
    RunPodSinglePod,
    /// Local trusted-LAN path.
    LocalTrustedLan,
}

/// Phase where one runtime environment variable must be materialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderLaunchPhase {
    /// Provider launch step.
    Launch,
    /// Host or node startup step.
    Startup,
    /// Active runtime execution.
    Runtime,
    /// Finalizer or closeout step.
    Finalizer,
}

/// Startup plan kind for one launch contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderStartupKind {
    /// Metadata-driven host startup script.
    MetadataStartupScript,
    /// Remote SSH-driven pod or host phase chain.
    RemotePhaseChain,
    /// Local operator-bundle materialization.
    LocalBundleMaterialization,
}

/// Finalizer plan kind for one launch contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderFinalizerKind {
    /// Dedicated host finalizer script.
    HostFinalizer,
    /// Dedicated remote finalizer script.
    RemoteFinalizer,
    /// Local closeout report path.
    LocalCloseout,
}

/// One environment variable in the shared launch contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchRuntimeEnvVar {
    /// Environment variable name.
    pub name: String,
    /// Template value rendered per run.
    pub value_template: String,
    /// Phases that require the variable.
    pub phases: Vec<CrossProviderLaunchPhase>,
    /// Why the variable matters.
    pub detail: String,
}

/// Shared artifact-root contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchArtifactRoots {
    /// Provider-neutral run root.
    pub run_root: String,
    /// Provider-neutral launch root.
    pub launch_root: String,
    /// Provider-neutral checkpoint root.
    pub checkpoint_root: String,
    /// Provider-neutral metrics root.
    pub metrics_root: String,
    /// Provider-neutral visualization root.
    pub visualization_root: String,
    /// Provider-neutral final root.
    pub final_root: String,
}

/// One cluster-port binding owned by the launch contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchClusterPortBinding {
    /// Stable binding id.
    pub binding_id: String,
    /// Bound role or traffic class.
    pub role: String,
    /// Reserved port.
    pub port: u16,
    /// Why the binding exists.
    pub detail: String,
}

/// Shared startup plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchStartupPlan {
    /// Startup kind.
    pub startup_kind: CrossProviderStartupKind,
    /// Startup entrypoint path.
    pub startup_entrypoint: String,
    /// Runtime env variables the startup path must materialize.
    pub required_env_names: Vec<String>,
    /// Artifacts the startup path must fetch or materialize.
    pub required_artifacts: Vec<String>,
    /// Provider-specific projected argv template.
    pub projected_startup_argv: Vec<String>,
    /// Why the startup path exists.
    pub detail: String,
}

/// Shared finalizer plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchFinalizerPlan {
    /// Finalizer kind.
    pub finalizer_kind: CrossProviderFinalizerKind,
    /// Finalizer entrypoint path or command template.
    pub finalizer_entrypoint: String,
    /// Runtime env variables the finalizer consumes.
    pub required_env_names: Vec<String>,
    /// Artifacts the finalizer expects as input.
    pub required_input_artifacts: Vec<String>,
    /// Artifacts the finalizer must emit.
    pub expected_output_artifacts: Vec<String>,
    /// Typed result classifications retained by the finalizer.
    pub admitted_result_classifications: Vec<String>,
    /// Provider-specific projected argv template.
    pub projected_finalizer_argv: Vec<String>,
    /// Why the finalizer exists.
    pub detail: String,
}

/// Provider-specific step projected from the shared launch contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderProjectedStep {
    /// Step identifier.
    pub step_id: String,
    /// Step phase.
    pub phase: CrossProviderLaunchPhase,
    /// Provider-specific command argv template.
    pub argv_template: Vec<String>,
    /// Environment variables referenced by the step.
    pub env_names: Vec<String>,
    /// Why the step exists.
    pub detail: String,
}

/// Binding back to the provider-facing source and script surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchSourceBinding {
    /// Source identifier.
    pub source_id: String,
    /// Source class.
    pub source_class: CrossProviderComputeSourceClass,
    /// Provider kind.
    pub provider: CrossProviderComputeProviderKind,
    /// Backend family.
    pub backend_family: CrossProviderBackendFamily,
    /// Source-contract digest when one already exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_contract_digest: Option<String>,
    /// Why the source binding is truthful.
    pub detail: String,
}

/// Full provider-neutral launch contract for one training lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable launch-contract id.
    pub launch_contract_id: String,
    /// Binder family used to project provider-specific steps.
    pub binder_kind: CrossProviderLaunchBinderKind,
    /// Program-manifest binding.
    pub program_manifest_id: String,
    /// Program-manifest digest.
    pub program_manifest_digest: String,
    /// Source binding behind the launch contract.
    pub source_binding: CrossProviderLaunchSourceBinding,
    /// Requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Stable run id.
    pub run_id: String,
    /// Shared runtime environment contract.
    pub runtime_env: Vec<CrossProviderLaunchRuntimeEnvVar>,
    /// Shared artifact roots.
    pub artifact_roots: CrossProviderLaunchArtifactRoots,
    /// Shared cluster-port bindings.
    pub cluster_port_bindings: Vec<CrossProviderLaunchClusterPortBinding>,
    /// Shared startup plan.
    pub startup_plan: CrossProviderLaunchStartupPlan,
    /// Shared finalizer plan.
    pub finalizer_plan: CrossProviderLaunchFinalizerPlan,
    /// Provider-specific projected steps.
    pub projected_steps: Vec<CrossProviderProjectedStep>,
    /// Claim boundary.
    pub claim_boundary: String,
    /// Stable launch-contract digest.
    pub launch_contract_digest: String,
}

impl CrossProviderLaunchContract {
    /// Returns the stable digest over the launch contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.launch_contract_digest.clear();
        stable_digest(b"psionic_cross_provider_launch_contract|", &clone)
    }

    /// Validates the launch contract against the root program manifest.
    pub fn validate(&self) -> Result<(), CrossProviderLaunchContractError> {
        let manifest = cross_provider_training_program_manifest().map_err(|error| {
            CrossProviderLaunchContractError::InvalidContract {
                detail: format!("failed to load canonical program manifest: {error}"),
            }
        })?;
        if self.schema_version != CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION {
            return Err(CrossProviderLaunchContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(CrossProviderLaunchContractError::InvalidContract {
                detail: String::from("program-manifest binding drifted"),
            });
        }
        if !manifest
            .admitted_compute_source_classes
            .contains(&self.source_binding.source_class)
        {
            return Err(CrossProviderLaunchContractError::InvalidContract {
                detail: format!(
                    "source class `{:?}` is not admitted by the root program manifest",
                    self.source_binding.source_class
                ),
            });
        }
        if !manifest
            .admitted_execution_classes
            .contains(&self.requested_execution_class)
        {
            return Err(CrossProviderLaunchContractError::InvalidContract {
                detail: format!(
                    "requested execution class `{:?}` is not admitted by the root program manifest",
                    self.requested_execution_class
                ),
            });
        }
        require_env(self, "PSION_PROGRAM_MANIFEST_ID")?;
        require_env(self, "PSION_PROGRAM_MANIFEST_DIGEST")?;
        require_env(self, "PSION_RUN_ID")?;
        require_env(self, "PSION_EXECUTION_CLASS")?;
        require_env(self, "PSION_RUN_ROOT")?;
        require_env(self, "PSION_LAUNCH_ROOT")?;
        require_env(self, "PSION_CHECKPOINT_ROOT")?;
        require_env(self, "PSION_METRICS_ROOT")?;
        require_env(self, "PSION_VISUALIZATION_ROOT")?;
        require_env(self, "PSION_FINAL_ROOT")?;
        if self.projected_steps.is_empty() {
            return Err(CrossProviderLaunchContractError::InvalidContract {
                detail: format!(
                    "launch contract `{}` projected no provider steps",
                    self.launch_contract_id
                ),
            });
        }
        if self.launch_contract_digest != self.stable_digest() {
            return Err(CrossProviderLaunchContractError::InvalidContract {
                detail: String::from("launch_contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical provider-neutral launch contracts.
static CROSS_PROVIDER_LAUNCH_CONTRACTS_CACHE: OnceLock<Vec<CrossProviderLaunchContract>> =
    OnceLock::new();

pub fn canonical_cross_provider_launch_contracts(
) -> Result<Vec<CrossProviderLaunchContract>, CrossProviderLaunchContractError> {
    if let Some(contracts) = CROSS_PROVIDER_LAUNCH_CONTRACTS_CACHE.get() {
        return Ok(contracts.clone());
    }
    let contracts = vec![
        google_single_node_accelerated_launch_contract()?,
        google_two_node_swarm_launch_contract()?,
        runpod_8xh100_launch_contract()?,
        local_first_swarm_launch_contract()?,
    ];
    for contract in &contracts {
        contract.validate()?;
    }
    let _ = CROSS_PROVIDER_LAUNCH_CONTRACTS_CACHE.set(contracts.clone());
    Ok(contracts)
}

/// Writes the canonical launch-contract fixtures into the supplied directory.
pub fn write_cross_provider_launch_contracts(
    output_dir: impl AsRef<Path>,
) -> Result<(), CrossProviderLaunchContractError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| {
        CrossProviderLaunchContractError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let contracts = canonical_cross_provider_launch_contracts()?;
    for contract in contracts {
        let file_name = match contract.binder_kind {
            CrossProviderLaunchBinderKind::GoogleSingleNode => {
                "google_single_node_accelerated_v1.json"
            }
            CrossProviderLaunchBinderKind::GoogleTwoNodeSwarm => "google_two_node_swarm_v1.json",
            CrossProviderLaunchBinderKind::RunPodSinglePod => "runpod_8xh100_v1.json",
            CrossProviderLaunchBinderKind::LocalTrustedLan => "local_first_swarm_v1.json",
        };
        write_json(output_dir.join(file_name), &contract)?;
    }
    Ok(())
}

fn google_single_node_accelerated_launch_contract(
) -> Result<CrossProviderLaunchContract, CrossProviderLaunchContractError> {
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical program manifest: {error}"),
        }
    })?;
    let launch_profiles = read_json_value(GOOGLE_SINGLE_NODE_LAUNCH_PROFILES_PATH)?;
    let _startup_policy = launch_profiles.get("startup_policy").ok_or_else(|| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: String::from("google single-node launch profiles lost startup_policy"),
        }
    })?;
    let _profile = launch_profiles
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|profiles| {
            profiles.iter().find(|profile| {
                profile.get("profile_id").and_then(Value::as_str)
                    == Some("g2_l4_single_node_accelerated")
            })
        })
        .ok_or_else(|| CrossProviderLaunchContractError::InvalidContract {
            detail: String::from("missing g2_l4_single_node_accelerated profile"),
        })?;
    let observability = read_json_value(GOOGLE_SINGLE_NODE_OBSERVABILITY_POLICY_PATH)?;
    let run_id = String::from("psion-xprovider-pretrain-google-single-node-accelerated");
    let artifact_roots = CrossProviderLaunchArtifactRoots {
        run_root: String::from("/var/lib/psion-google/runs/${RUN_ID}"),
        launch_root: render_program_root(
            manifest.artifact_roots.launch_root_template.as_str(),
            &run_id,
        ),
        checkpoint_root: render_program_root(
            manifest.artifact_roots.checkpoint_root_template.as_str(),
            &run_id,
        ),
        metrics_root: render_program_root(
            manifest.artifact_roots.metrics_root_template.as_str(),
            &run_id,
        ),
        visualization_root: render_program_root(
            manifest.artifact_roots.visualization_root_template.as_str(),
            &run_id,
        ),
        final_root: render_program_root(
            manifest.artifact_roots.final_root_template.as_str(),
            &run_id,
        ),
    };
    let mut contract = CrossProviderLaunchContract {
        schema_version: String::from(CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION),
        launch_contract_id: String::from("psionic-google-single-node-accelerated-launch-contract-v1"),
        binder_kind: CrossProviderLaunchBinderKind::GoogleSingleNode,
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        source_binding: CrossProviderLaunchSourceBinding {
            source_id: String::from("google_l4_single_node_training_host"),
            source_class: CrossProviderComputeSourceClass::GoogleCloud,
            provider: CrossProviderComputeProviderKind::GoogleCloud,
            backend_family: CrossProviderBackendFamily::Cuda,
            source_contract_digest: None,
            detail: String::from(
                "This launch contract binds the Google single-node accelerated g2 plus L4 lane directly to the committed single-node launch profile until a dedicated single-node compute-source contract is frozen.",
            ),
        },
        requested_execution_class: CrossProviderExecutionClass::DenseFullModelRank,
        run_id: run_id.clone(),
        runtime_env: vec![
            env_var(
                "PSION_PROGRAM_MANIFEST_ID",
                manifest.program_manifest_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Bind the runtime to the root cross-provider pretraining program.",
            ),
            env_var(
                "PSION_PROGRAM_MANIFEST_DIGEST",
                manifest.program_manifest_digest.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the exact program-manifest digest into startup and finalizer surfaces.",
            ),
            env_var(
                "PSION_RUN_ID",
                run_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Stable run identifier for the Google single-node lane.",
            ),
            env_var(
                "PSION_EXECUTION_CLASS",
                "dense_full_model_rank",
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the dense-rank execution-class identity through all phases.",
            ),
            generic_root_env("PSION_RUN_ROOT", artifact_roots.run_root.as_str()),
            generic_root_env("PSION_LAUNCH_ROOT", artifact_roots.launch_root.as_str()),
            generic_root_env("PSION_CHECKPOINT_ROOT", artifact_roots.checkpoint_root.as_str()),
            generic_root_env("PSION_METRICS_ROOT", artifact_roots.metrics_root.as_str()),
            generic_root_env(
                "PSION_VISUALIZATION_ROOT",
                artifact_roots.visualization_root.as_str(),
            ),
            generic_root_env("PSION_FINAL_ROOT", artifact_roots.final_root.as_str()),
            env_var(
                "PSION_BUCKET_URL",
                "gs://openagentsgemini-psion-train-us-central1",
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Finalizer],
                "Bucket authority for Google-hosted retained artifacts.",
            ),
            env_var(
                "PSION_REPO_GIT_REVISION",
                "workspace@${GIT_REVISION}",
                &[CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Propagate the detached repo revision chosen by the launcher into startup and finalization.",
            ),
        ],
        artifact_roots,
        cluster_port_bindings: Vec::new(),
        startup_plan: CrossProviderLaunchStartupPlan {
            startup_kind: CrossProviderStartupKind::MetadataStartupScript,
            startup_entrypoint: String::from(GOOGLE_SINGLE_NODE_STARTUP_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PSION_RUN_ROOT"),
                String::from("PSION_OUTPUT_DIR"),
                String::from("PSION_LOG_DIR"),
                String::from("PSION_SCRATCH_DIR"),
                String::from("PSION_REPO_DIR"),
            ],
            required_artifacts: vec![
                String::from("launch_manifest"),
                String::from("input_package_descriptor"),
                String::from("input_package_archive"),
            ],
            projected_startup_argv: vec![String::from(GOOGLE_SINGLE_NODE_STARTUP_SCRIPT_PATH)],
            detail: String::from(
                "The Google single-node startup path remains metadata-driven, but the shared launch contract now owns the runtime env and artifact-root envelope above those metadata bindings.",
            ),
        },
        finalizer_plan: CrossProviderLaunchFinalizerPlan {
            finalizer_kind: CrossProviderFinalizerKind::HostFinalizer,
            finalizer_entrypoint: String::from(GOOGLE_SINGLE_NODE_FINALIZER_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PSION_RUN_ROOT"),
                String::from("PSION_REPO_DIR"),
                String::from("PSION_FINAL_ROOT"),
            ],
            required_input_artifacts: vec![
                String::from("launch_manifest"),
                String::from("stage_receipt"),
                String::from("observability_receipt"),
            ],
            expected_output_artifacts: vec![
                format!(
                    "{}/{}",
                    observability
                        .get("artifact_paths")
                        .and_then(|paths| paths.get("final_prefix"))
                        .and_then(Value::as_str)
                        .unwrap_or("final"),
                    "psion_google_final_manifest.json"
                ),
                String::from("training_visualization/remote_training_run_index_v1.json"),
            ],
            admitted_result_classifications: vec![
                String::from("bounded_success"),
                String::from("training_runtime_failure"),
                String::from("artifact_upload_failure"),
                String::from("checkpoint_restore_failure"),
            ],
            projected_finalizer_argv: vec![
                String::from(GOOGLE_SINGLE_NODE_FINALIZER_SCRIPT_PATH),
                String::from("--run-root"),
                String::from("${PSION_RUN_ROOT}"),
                String::from("--repo-dir"),
                String::from("${PSION_REPO_DIR}"),
                String::from("--run-id"),
                String::from("${PSION_RUN_ID}"),
                String::from("--launch-manifest-uri"),
                String::from("${PSION_LAUNCH_MANIFEST_URI}"),
            ],
            detail: String::from(
                "The finalizer expectations stay explicit for final manifest upload, accelerator evidence sealing, and retained visualization artifacts.",
            ),
        },
        projected_steps: vec![
            projected_step(
                "google_single_node_launch",
                CrossProviderLaunchPhase::Launch,
                &[
                    GOOGLE_SINGLE_NODE_LAUNCH_SCRIPT_PATH,
                    "--profile",
                    "g2_l4_single_node_accelerated",
                    "--run-id",
                    "${PSION_RUN_ID}",
                ],
                &["PSION_RUN_ID", "PSION_PROGRAM_MANIFEST_ID", "PSION_LAUNCH_ROOT"],
                "Provider-specific Google launcher that creates the VM and binds metadata.",
            ),
            projected_step(
                "google_single_node_startup",
                CrossProviderLaunchPhase::Startup,
                &[GOOGLE_SINGLE_NODE_STARTUP_SCRIPT_PATH],
                &["PSION_RUN_ID", "PSION_RUN_ROOT", "PSION_REPO_GIT_REVISION"],
                "Metadata-driven startup script that materializes the runtime envelope.",
            ),
            projected_step(
                "google_single_node_finalizer",
                CrossProviderLaunchPhase::Finalizer,
                &[
                    GOOGLE_SINGLE_NODE_FINALIZER_SCRIPT_PATH,
                    "--run-root",
                    "${PSION_RUN_ROOT}",
                    "--repo-dir",
                    "${PSION_REPO_DIR}",
                    "--run-id",
                    "${PSION_RUN_ID}",
                ],
                &["PSION_RUN_ID", "PSION_RUN_ROOT", "PSION_FINAL_ROOT"],
                "Host finalizer that seals retained evidence under the shared finalizer contract.",
            ),
        ],
        claim_boundary: String::from(
            "This launch contract proves the Google single-node accelerated lane can be described through the same provider-neutral runtime envelope as the other launchers. It does not claim provider API automation closure beyond the existing Google scripts, nor dense multi-node closure by itself.",
        ),
        launch_contract_digest: String::new(),
    };
    contract.launch_contract_digest = contract.stable_digest();
    Ok(contract)
}

fn google_two_node_swarm_launch_contract(
) -> Result<CrossProviderLaunchContract, CrossProviderLaunchContractError> {
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical program manifest: {error}"),
        }
    })?;
    let compute_sources = canonical_cross_provider_compute_source_contracts().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical compute-source contracts: {error}"),
        }
    })?;
    let google_source = compute_sources
        .iter()
        .find(|contract| contract.source_id == "google_l4_validator_node")
        .ok_or_else(|| CrossProviderLaunchContractError::InvalidContract {
            detail: String::from("missing google_l4_validator_node compute source"),
        })?;
    let swarm_contract = read_json_value(PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH)?;
    let launch_profiles = read_json_value(PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH)?;
    let coordinator_port = launch_profiles
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|profiles| profiles.first())
        .and_then(|profile| profile.get("cluster_port"))
        .and_then(Value::as_u64)
        .unwrap_or(34100) as u16;
    let run_id = String::from("psion-xprovider-pretrain-google-two-node-swarm");
    let artifact_roots = CrossProviderLaunchArtifactRoots {
        run_root: String::from("gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}"),
        launch_root: render_program_root(
            manifest.artifact_roots.launch_root_template.as_str(),
            &run_id,
        ),
        checkpoint_root: render_program_root(
            manifest.artifact_roots.checkpoint_root_template.as_str(),
            &run_id,
        ),
        metrics_root: render_program_root(
            manifest.artifact_roots.metrics_root_template.as_str(),
            &run_id,
        ),
        visualization_root: render_program_root(
            manifest.artifact_roots.visualization_root_template.as_str(),
            &run_id,
        ),
        final_root: render_program_root(
            manifest.artifact_roots.final_root_template.as_str(),
            &run_id,
        ),
    };
    let mut contract = CrossProviderLaunchContract {
        schema_version: String::from(CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION),
        launch_contract_id: String::from("psionic-google-two-node-swarm-launch-contract-v1"),
        binder_kind: CrossProviderLaunchBinderKind::GoogleTwoNodeSwarm,
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        source_binding: CrossProviderLaunchSourceBinding {
            source_id: google_source.source_id.clone(),
            source_class: google_source.source_class,
            provider: google_source.provider,
            backend_family: google_source.backend.backend_family,
            source_contract_digest: Some(google_source.contract_digest.clone()),
            detail: String::from(
                "This launch contract binds directly to the canonical Google L4 compute-source contract from XTRAIN-2.",
            ),
        },
        requested_execution_class: CrossProviderExecutionClass::ValidatedContributorWindow,
        run_id: run_id.clone(),
        runtime_env: vec![
            env_var(
                "PSION_PROGRAM_MANIFEST_ID",
                manifest.program_manifest_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Bind the Google swarm lane to the root cross-provider pretraining program.",
            ),
            env_var(
                "PSION_PROGRAM_MANIFEST_DIGEST",
                manifest.program_manifest_digest.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the exact program-manifest digest into the swarm lane.",
            ),
            env_var(
                "PSION_RUN_ID",
                run_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Stable run id for the Google two-node swarm lane.",
            ),
            env_var(
                "PSION_EXECUTION_CLASS",
                "validated_contributor_window",
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the validated contributor execution class through the swarm lane.",
            ),
            generic_root_env("PSION_RUN_ROOT", artifact_roots.run_root.as_str()),
            generic_root_env("PSION_LAUNCH_ROOT", artifact_roots.launch_root.as_str()),
            generic_root_env("PSION_CHECKPOINT_ROOT", artifact_roots.checkpoint_root.as_str()),
            generic_root_env("PSION_METRICS_ROOT", artifact_roots.metrics_root.as_str()),
            generic_root_env(
                "PSION_VISUALIZATION_ROOT",
                artifact_roots.visualization_root.as_str(),
            ),
            generic_root_env("PSION_FINAL_ROOT", artifact_roots.final_root.as_str()),
            env_var(
                "PSION_CLUSTER_NAMESPACE",
                swarm_contract
                    .get("cluster_namespace")
                    .and_then(Value::as_str)
                    .unwrap_or("cluster.psion.google.configured_peer_swarm"),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Bind the swarm lane to the committed configured-peer cluster namespace.",
            ),
        ],
        artifact_roots,
        cluster_port_bindings: vec![CrossProviderLaunchClusterPortBinding {
            binding_id: String::from("configured_peer_cluster_port"),
            role: String::from("coordinator_listener"),
            port: coordinator_port,
            detail: String::from(
                "Reserved cluster port for the configured-peer Google swarm coordinator.",
            ),
        }],
        startup_plan: CrossProviderLaunchStartupPlan {
            startup_kind: CrossProviderStartupKind::MetadataStartupScript,
            startup_entrypoint: String::from(GOOGLE_SWARM_STARTUP_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PSION_CLUSTER_NAMESPACE"),
                String::from("PSION_RUN_ROOT"),
            ],
            required_artifacts: vec![
                String::from("cluster_manifest"),
                String::from("endpoint_manifest"),
            ],
            projected_startup_argv: vec![String::from(GOOGLE_SWARM_STARTUP_SCRIPT_PATH)],
            detail: String::from(
                "The swarm startup path stays metadata-driven, but the shared launch contract now owns the cluster namespace, run root, and cluster-port authority above it.",
            ),
        },
        finalizer_plan: CrossProviderLaunchFinalizerPlan {
            finalizer_kind: CrossProviderFinalizerKind::HostFinalizer,
            finalizer_entrypoint: String::from(GOOGLE_SWARM_FINALIZER_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PSION_FINAL_ROOT"),
                String::from("PSION_VISUALIZATION_ROOT"),
            ],
            required_input_artifacts: vec![
                String::from("cluster_manifest"),
                String::from("launch_receipt"),
                String::from("coordinator_runtime_report"),
                String::from("contributor_runtime_report"),
            ],
            expected_output_artifacts: vec![
                String::from("psion_google_two_node_swarm_evidence_bundle.json"),
                String::from("psion_google_two_node_swarm_final_manifest.json"),
            ],
            admitted_result_classifications: vec![
                String::from("configured_peer_launch_failure"),
                String::from("cluster_membership_failure"),
                String::from("bounded_success"),
            ],
            projected_finalizer_argv: vec![
                String::from(GOOGLE_SWARM_FINALIZER_SCRIPT_PATH),
                String::from("--run-id"),
                String::from("${PSION_RUN_ID}"),
            ],
            detail: String::from(
                "The finalizer expectations stay explicit for the cluster-wide evidence bundle and final manifest that the Google swarm lane already retains.",
            ),
        },
        projected_steps: vec![
            projected_step(
                "google_swarm_launch",
                CrossProviderLaunchPhase::Launch,
                &[
                    GOOGLE_SWARM_LAUNCH_SCRIPT_PATH,
                    "--run-id",
                    "${PSION_RUN_ID}",
                ],
                &["PSION_RUN_ID", "PSION_CLUSTER_NAMESPACE", "PSION_LAUNCH_ROOT"],
                "Provider-specific Google swarm launcher that binds the configured-peer nodes.",
            ),
            projected_step(
                "google_swarm_startup",
                CrossProviderLaunchPhase::Startup,
                &[GOOGLE_SWARM_STARTUP_SCRIPT_PATH],
                &["PSION_RUN_ID", "PSION_RUN_ROOT", "PSION_CLUSTER_NAMESPACE"],
                "Metadata-driven startup script for the two-node swarm lane.",
            ),
            projected_step(
                "google_swarm_finalizer",
                CrossProviderLaunchPhase::Finalizer,
                &[
                    GOOGLE_SWARM_FINALIZER_SCRIPT_PATH,
                    "--run-id",
                    "${PSION_RUN_ID}",
                ],
                &["PSION_RUN_ID", "PSION_FINAL_ROOT", "PSION_VISUALIZATION_ROOT"],
                "Host finalizer for cluster-wide evidence bundle sealing.",
            ),
        ],
        claim_boundary: String::from(
            "This launch contract proves the Google two-node swarm lane can be projected from the same provider-neutral runtime envelope as the other launchers. It does not claim dense full-model Google training closure by itself.",
        ),
        launch_contract_digest: String::new(),
    };
    contract.launch_contract_digest = contract.stable_digest();
    Ok(contract)
}

fn runpod_8xh100_launch_contract(
) -> Result<CrossProviderLaunchContract, CrossProviderLaunchContractError> {
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical program manifest: {error}"),
        }
    })?;
    let compute_sources = canonical_cross_provider_compute_source_contracts().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical compute-source contracts: {error}"),
        }
    })?;
    let runpod_source = compute_sources
        .iter()
        .find(|contract| contract.source_id == "runpod_8xh100_dense_node")
        .ok_or_else(|| CrossProviderLaunchContractError::InvalidContract {
            detail: String::from("missing runpod_8xh100_dense_node compute source"),
        })?;
    let launch_profiles = read_json_value(RUNPOD_LAUNCH_PROFILES_PATH)?;
    let profile = launch_profiles
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|profiles| profiles.first())
        .ok_or_else(|| CrossProviderLaunchContractError::InvalidContract {
            detail: String::from("missing RunPod launch profile"),
        })?;
    let workspace_root = launch_profiles
        .get("workspace_root")
        .and_then(Value::as_str)
        .unwrap_or("/workspace");
    let run_id = String::from("psion-xprovider-pretrain-runpod-8xh100");
    let run_root = format!("{workspace_root}/parameter-golf-runpod/${{RUN_ID}}");
    let artifact_roots = CrossProviderLaunchArtifactRoots {
        run_root: run_root.clone(),
        launch_root: render_program_root(
            manifest.artifact_roots.launch_root_template.as_str(),
            &run_id,
        ),
        checkpoint_root: render_program_root(
            manifest.artifact_roots.checkpoint_root_template.as_str(),
            &run_id,
        ),
        metrics_root: render_program_root(
            manifest.artifact_roots.metrics_root_template.as_str(),
            &run_id,
        ),
        visualization_root: String::from("${PSION_RUN_ROOT}/training_visualization"),
        final_root: String::from("${PSION_RUN_ROOT}"),
    };
    let submission_dir =
        String::from("${PSION_RUN_ROOT}/exported_submission/records/track_non_record_16mb/2026-03-18_psionic_local_reference_runtime_replay_v2");
    let mut contract = CrossProviderLaunchContract {
        schema_version: String::from(CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION),
        launch_contract_id: String::from("psionic-runpod-8xh100-launch-contract-v1"),
        binder_kind: CrossProviderLaunchBinderKind::RunPodSinglePod,
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        source_binding: CrossProviderLaunchSourceBinding {
            source_id: runpod_source.source_id.clone(),
            source_class: runpod_source.source_class,
            provider: runpod_source.provider,
            backend_family: runpod_source.backend.backend_family,
            source_contract_digest: Some(runpod_source.contract_digest.clone()),
            detail: String::from(
                "This launch contract binds directly to the canonical RunPod 8xH100 compute-source contract from XTRAIN-2.",
            ),
        },
        requested_execution_class: CrossProviderExecutionClass::DenseFullModelRank,
        run_id: run_id.clone(),
        runtime_env: vec![
            env_var(
                "PSION_PROGRAM_MANIFEST_ID",
                manifest.program_manifest_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Bind the RunPod lane to the root cross-provider pretraining program.",
            ),
            env_var(
                "PSION_PROGRAM_MANIFEST_DIGEST",
                manifest.program_manifest_digest.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the exact program-manifest digest into RunPod runtime and finalizer phases.",
            ),
            env_var(
                "PSION_RUN_ID",
                run_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Stable run id for the RunPod dense lane.",
            ),
            env_var(
                "PSION_EXECUTION_CLASS",
                "dense_full_model_rank",
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the dense-rank execution class through the RunPod lane.",
            ),
            generic_root_env("PSION_RUN_ROOT", artifact_roots.run_root.as_str()),
            generic_root_env("PSION_LAUNCH_ROOT", artifact_roots.launch_root.as_str()),
            generic_root_env("PSION_CHECKPOINT_ROOT", artifact_roots.checkpoint_root.as_str()),
            generic_root_env("PSION_METRICS_ROOT", artifact_roots.metrics_root.as_str()),
            generic_root_env(
                "PSION_VISUALIZATION_ROOT",
                artifact_roots.visualization_root.as_str(),
            ),
            generic_root_env("PSION_FINAL_ROOT", artifact_roots.final_root.as_str()),
            env_var(
                "PGOLF_RUN_ROOT",
                "${PSION_RUN_ROOT}",
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Retain compatibility with the existing RunPod operator script surface.",
            ),
            env_var(
                "PGOLF_SUBMISSION_DIR",
                submission_dir.as_str(),
                &[CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Retain compatibility with the existing exported-folder finalizer and runtime paths.",
            ),
            env_var(
                "PGOLF_REPO_DIR",
                format!("{workspace_root}/psionic").as_str(),
                &[CrossProviderLaunchPhase::Startup, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Retain compatibility with the existing RunPod repo checkout location.",
            ),
        ],
        artifact_roots,
        cluster_port_bindings: Vec::new(),
        startup_plan: CrossProviderLaunchStartupPlan {
            startup_kind: CrossProviderStartupKind::RemotePhaseChain,
            startup_entrypoint: String::from(RUNPOD_LAUNCH_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PGOLF_RUN_ROOT"),
                String::from("PGOLF_SUBMISSION_DIR"),
            ],
            required_artifacts: vec![
                String::from("launch_manifest"),
                String::from("launch_receipt"),
                String::from("remote_preflight.log"),
            ],
            projected_startup_argv: vec![
                String::from(RUNPOD_LAUNCH_SCRIPT_PATH),
                String::from("--profile"),
                String::from(
                    profile
                        .get("profile_id")
                        .and_then(Value::as_str)
                        .unwrap_or("runpod_8xh100_parameter_golf"),
                ),
                String::from("--run-id"),
                String::from("${PSION_RUN_ID}"),
            ],
            detail: String::from(
                "The RunPod lane keeps its remote preflight, pre-training, execution, and finalizer chain, but the shared launch contract now owns the env and artifact-root envelope above that phase chain.",
            ),
        },
        finalizer_plan: CrossProviderLaunchFinalizerPlan {
            finalizer_kind: CrossProviderFinalizerKind::RemoteFinalizer,
            finalizer_entrypoint: String::from(RUNPOD_FINALIZER_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PGOLF_RUN_ROOT"),
                String::from("PGOLF_SUBMISSION_DIR"),
                String::from("PSION_VISUALIZATION_ROOT"),
            ],
            required_input_artifacts: vec![
                String::from("parameter_golf_runpod_8xh100_launch_manifest.json"),
                String::from("execution.log"),
                String::from("nvidia_smi_inventory.txt"),
            ],
            expected_output_artifacts: vec![
                String::from("parameter_golf_distributed_8xh100_receipt.json"),
                String::from("parameter_golf_runpod_8xh100_finalizer_report.json"),
                String::from("training_visualization/parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json"),
                String::from("training_visualization/remote_training_run_index_v1.json"),
            ],
            admitted_result_classifications: vec![
                String::from("bounded_success"),
                String::from("measurements_missing"),
                String::from("inventory_mismatch"),
            ],
            projected_finalizer_argv: vec![
                String::from(RUNPOD_FINALIZER_SCRIPT_PATH),
                String::from("--run-root"),
                String::from("${PGOLF_RUN_ROOT}"),
                String::from("--submission-dir"),
                String::from("${PGOLF_SUBMISSION_DIR}"),
                String::from("--output"),
                String::from("${PSION_FINAL_ROOT}/parameter_golf_runpod_8xh100_finalizer_report.json"),
            ],
            detail: String::from(
                "The finalizer expectations remain explicit for the distributed receipt, exported-folder evidence, and always-live visualization bundle family.",
            ),
        },
        projected_steps: vec![
            projected_step(
                "runpod_launch",
                CrossProviderLaunchPhase::Launch,
                &[
                    RUNPOD_LAUNCH_SCRIPT_PATH,
                    "--profile",
                    "runpod_8xh100_parameter_golf",
                    "--run-id",
                    "${PSION_RUN_ID}",
                ],
                &["PSION_RUN_ID", "PGOLF_RUN_ROOT", "PGOLF_SUBMISSION_DIR"],
                "Provider-specific RunPod launcher that binds SSH, workspace, and manifest roots.",
            ),
            projected_step(
                "runpod_runtime",
                CrossProviderLaunchPhase::Runtime,
                &[
                    "python3",
                    "train_gpt.py",
                ],
                &["PGOLF_RUN_ROOT", "PGOLF_SUBMISSION_DIR", "PSION_EXECUTION_CLASS"],
                "Remote dense runtime entrypoint inside the pod-local phase chain.",
            ),
            projected_step(
                "runpod_finalizer",
                CrossProviderLaunchPhase::Finalizer,
                &[
                    RUNPOD_FINALIZER_SCRIPT_PATH,
                    "--run-root",
                    "${PGOLF_RUN_ROOT}",
                    "--submission-dir",
                    "${PGOLF_SUBMISSION_DIR}",
                ],
                &["PGOLF_RUN_ROOT", "PGOLF_SUBMISSION_DIR", "PSION_VISUALIZATION_ROOT"],
                "Remote finalizer for the distributed receipt and visualization bundle.",
            ),
        ],
        claim_boundary: String::from(
            "This launch contract proves the RunPod 8xH100 lane can be projected from the same provider-neutral runtime envelope as the Google and local lanes. It does not claim cross-host RunPod cluster closure or dense mixed-backend training.",
        ),
        launch_contract_digest: String::new(),
    };
    contract.launch_contract_digest = contract.stable_digest();
    Ok(contract)
}

fn local_first_swarm_launch_contract(
) -> Result<CrossProviderLaunchContract, CrossProviderLaunchContractError> {
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical program manifest: {error}"),
        }
    })?;
    let compute_sources = canonical_cross_provider_compute_source_contracts().map_err(|error| {
        CrossProviderLaunchContractError::InvalidContract {
            detail: format!("failed to load canonical compute-source contracts: {error}"),
        }
    })?;
    let local_mac = compute_sources
        .iter()
        .find(|contract| contract.source_id == "local_mlx_mac_workstation")
        .ok_or_else(|| CrossProviderLaunchContractError::InvalidContract {
            detail: String::from("missing local_mlx_mac_workstation compute source"),
        })?;
    let topology = read_json_value(LOCAL_SWARM_TOPOLOGY_CONTRACT_PATH)?;
    let workflow = read_json_value(LOCAL_SWARM_WORKFLOW_PLAN_PATH)?;
    let run_id = String::from("psion-xprovider-pretrain-local-first-swarm");
    let bundle_dir = String::from("${PSION_RUN_ROOT}/bundle");
    let artifact_roots = CrossProviderLaunchArtifactRoots {
        run_root: String::from("${HOME}/swarm-runs/${RUN_ID}"),
        launch_root: render_program_root(
            manifest.artifact_roots.launch_root_template.as_str(),
            &run_id,
        ),
        checkpoint_root: render_program_root(
            manifest.artifact_roots.checkpoint_root_template.as_str(),
            &run_id,
        ),
        metrics_root: render_program_root(
            manifest.artifact_roots.metrics_root_template.as_str(),
            &run_id,
        ),
        visualization_root: render_program_root(
            manifest.artifact_roots.visualization_root_template.as_str(),
            &run_id,
        ),
        final_root: render_program_root(
            manifest.artifact_roots.final_root_template.as_str(),
            &run_id,
        ),
    };
    let mut contract = CrossProviderLaunchContract {
        schema_version: String::from(CROSS_PROVIDER_LAUNCH_CONTRACT_SCHEMA_VERSION),
        launch_contract_id: String::from("psionic-local-first-swarm-launch-contract-v1"),
        binder_kind: CrossProviderLaunchBinderKind::LocalTrustedLan,
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        source_binding: CrossProviderLaunchSourceBinding {
            source_id: local_mac.source_id.clone(),
            source_class: CrossProviderComputeSourceClass::TrustedLanCluster,
            provider: CrossProviderComputeProviderKind::LocalOperatorManaged,
            backend_family: CrossProviderBackendFamily::MlxMetal,
            source_contract_digest: Some(local_mac.contract_digest.clone()),
            detail: String::from(
                "This launch contract binds the first local trusted-LAN swarm to the canonical local Apple contributor source while keeping the wider cluster posture explicit.",
            ),
        },
        requested_execution_class: CrossProviderExecutionClass::ValidatedContributorWindow,
        run_id: run_id.clone(),
        runtime_env: vec![
            env_var(
                "PSION_PROGRAM_MANIFEST_ID",
                manifest.program_manifest_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Bind the local swarm lane to the root cross-provider pretraining program.",
            ),
            env_var(
                "PSION_PROGRAM_MANIFEST_DIGEST",
                manifest.program_manifest_digest.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the exact program-manifest digest into the local swarm lane.",
            ),
            env_var(
                "PSION_RUN_ID",
                run_id.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Stable run id for the first local trusted-LAN swarm.",
            ),
            env_var(
                "PSION_EXECUTION_CLASS",
                "validated_contributor_window",
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Runtime, CrossProviderLaunchPhase::Finalizer],
                "Carry the validated contributor execution class through the local swarm lane.",
            ),
            generic_root_env("PSION_RUN_ROOT", artifact_roots.run_root.as_str()),
            generic_root_env("PSION_LAUNCH_ROOT", artifact_roots.launch_root.as_str()),
            generic_root_env("PSION_CHECKPOINT_ROOT", artifact_roots.checkpoint_root.as_str()),
            generic_root_env("PSION_METRICS_ROOT", artifact_roots.metrics_root.as_str()),
            generic_root_env(
                "PSION_VISUALIZATION_ROOT",
                artifact_roots.visualization_root.as_str(),
            ),
            generic_root_env("PSION_FINAL_ROOT", artifact_roots.final_root.as_str()),
            env_var(
                "PSION_BUNDLE_DIR",
                bundle_dir.as_str(),
                &[CrossProviderLaunchPhase::Launch, CrossProviderLaunchPhase::Runtime],
                "Local operator-bundle directory retained by the first swarm launcher.",
            ),
        ],
        artifact_roots,
        cluster_port_bindings: Vec::new(),
        startup_plan: CrossProviderLaunchStartupPlan {
            startup_kind: CrossProviderStartupKind::LocalBundleMaterialization,
            startup_entrypoint: String::from(LOCAL_SWARM_LAUNCH_SCRIPT_PATH),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PSION_RUN_ROOT"),
                String::from("PSION_BUNDLE_DIR"),
            ],
            required_artifacts: vec![
                String::from("first_swarm_trusted_lan_topology_contract_v1.json"),
                String::from("first_swarm_live_workflow_plan_v1.json"),
            ],
            projected_startup_argv: vec![
                String::from(LOCAL_SWARM_LAUNCH_SCRIPT_PATH),
                String::from("--run-id"),
                String::from("${PSION_RUN_ID}"),
                String::from("--bundle-dir"),
                String::from("${PSION_BUNDLE_DIR}"),
            ],
            detail: format!(
                "The local swarm startup path remains a local operator bundle that reuses the committed topology contract and workflow plan digests `{}` and `{}`.",
                topology
                    .get("contract_digest")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                workflow.get("plan_digest").and_then(Value::as_str).unwrap_or("unknown"),
            ),
        },
        finalizer_plan: CrossProviderLaunchFinalizerPlan {
            finalizer_kind: CrossProviderFinalizerKind::LocalCloseout,
            finalizer_entrypoint: String::from(LOCAL_SWARM_CLOSEOUT_BIN),
            required_env_names: vec![
                String::from("PSION_RUN_ID"),
                String::from("PSION_FINAL_ROOT"),
            ],
            required_input_artifacts: vec![
                String::from("first_swarm_trusted_lan_launch_manifest.json"),
                String::from("first_swarm_trusted_lan_evidence_bundle_v1.json"),
            ],
            expected_output_artifacts: vec![
                String::from("first_swarm_trusted_lan_closeout_v1.json"),
            ],
            admitted_result_classifications: vec![
                String::from("no_merge"),
                String::from("publish_refused"),
            ],
            projected_finalizer_argv: vec![
                String::from("cargo"),
                String::from("run"),
                String::from("-q"),
                String::from("-p"),
                String::from("psionic-train"),
                String::from("--bin"),
                String::from("first_swarm_trusted_lan_closeout_report"),
                String::from("--"),
                String::from("${PSION_FINAL_ROOT}/first_swarm_trusted_lan_closeout_v1.json"),
            ],
            detail: String::from(
                "The first local swarm lane still closes through a local closeout report rather than a remote host finalizer, but the expected inputs and output are now explicit in the shared launch contract.",
            ),
        },
        projected_steps: vec![
            projected_step(
                "local_swarm_launch",
                CrossProviderLaunchPhase::Launch,
                &[
                    LOCAL_SWARM_LAUNCH_SCRIPT_PATH,
                    "--run-id",
                    "${PSION_RUN_ID}",
                    "--bundle-dir",
                    "${PSION_BUNDLE_DIR}",
                    "--manifest-only",
                ],
                &["PSION_RUN_ID", "PSION_BUNDLE_DIR", "PSION_RUN_ROOT"],
                "Provider-specific local launcher that materializes the operator bundle and trusted-LAN manifest.",
            ),
            projected_step(
                "local_swarm_closeout",
                CrossProviderLaunchPhase::Finalizer,
                &[
                    "cargo",
                    "run",
                    "-q",
                    "-p",
                    "psionic-train",
                    "--bin",
                    "first_swarm_trusted_lan_closeout_report",
                    "--",
                    "${PSION_FINAL_ROOT}/first_swarm_trusted_lan_closeout_v1.json",
                ],
                &["PSION_RUN_ID", "PSION_FINAL_ROOT"],
                "Local closeout report path for the first swarm lane.",
            ),
        ],
        claim_boundary: String::from(
            "This launch contract proves the first local trusted-LAN swarm can be projected from the same provider-neutral runtime envelope as the Google and RunPod lanes. It does not claim dense-rank closure, public swarm discovery, or same-job mixed-backend dense training.",
        ),
        launch_contract_digest: String::new(),
    };
    contract.launch_contract_digest = contract.stable_digest();
    Ok(contract)
}

fn env_var(
    name: &str,
    value_template: &str,
    phases: &[CrossProviderLaunchPhase],
    detail: &str,
) -> CrossProviderLaunchRuntimeEnvVar {
    CrossProviderLaunchRuntimeEnvVar {
        name: String::from(name),
        value_template: String::from(value_template),
        phases: phases.to_vec(),
        detail: String::from(detail),
    }
}

fn generic_root_env(name: &str, value_template: &str) -> CrossProviderLaunchRuntimeEnvVar {
    env_var(
        name,
        value_template,
        &[
            CrossProviderLaunchPhase::Launch,
            CrossProviderLaunchPhase::Startup,
            CrossProviderLaunchPhase::Runtime,
            CrossProviderLaunchPhase::Finalizer,
        ],
        "Shared artifact-root binding in the provider-neutral launch contract.",
    )
}

fn projected_step(
    step_id: &str,
    phase: CrossProviderLaunchPhase,
    argv_template: &[&str],
    env_names: &[&str],
    detail: &str,
) -> CrossProviderProjectedStep {
    CrossProviderProjectedStep {
        step_id: String::from(step_id),
        phase,
        argv_template: argv_template.iter().map(|arg| String::from(*arg)).collect(),
        env_names: env_names.iter().map(|arg| String::from(*arg)).collect(),
        detail: String::from(detail),
    }
}

fn require_env(
    contract: &CrossProviderLaunchContract,
    name: &str,
) -> Result<(), CrossProviderLaunchContractError> {
    if contract.runtime_env.iter().any(|env| env.name == name) {
        return Ok(());
    }
    Err(CrossProviderLaunchContractError::InvalidContract {
        detail: format!(
            "launch contract `{}` is missing required runtime env `{}`",
            contract.launch_contract_id, name
        ),
    })
}

fn render_program_root(template: &str, run_id: &str) -> String {
    template.replace("${RUN_ID}", run_id)
}

fn read_json_value(path: &str) -> Result<Value, CrossProviderLaunchContractError> {
    let resolved = resolve_repo_path(path);
    let bytes = fs::read(&resolved).map_err(|error| CrossProviderLaunchContractError::Read {
        path: path.to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| CrossProviderLaunchContractError::Deserialize {
        path: path.to_string(),
        error,
    })
}

fn write_json(
    path: impl AsRef<Path>,
    value: &impl Serialize,
) -> Result<(), CrossProviderLaunchContractError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CrossProviderLaunchContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|error| CrossProviderLaunchContractError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable launch-contract serialization"));
    hex::encode(hasher.finalize())
}

fn resolve_repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}
