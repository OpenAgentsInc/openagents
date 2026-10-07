use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, canonical_cross_provider_launch_contracts,
    canonical_remote_train_artifact_backend_contract_set, cross_provider_training_program_manifest,
    CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError,
    CrossProviderExecutionClass, CrossProviderLaunchBinderKind, CrossProviderLaunchContract,
    CrossProviderLaunchContractError, CrossProviderLaunchPhase,
    CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError,
    RemoteTrainArtifactBackendContractError, TrainArtifactClass,
};

/// Stable schema version for the provider-neutral runtime binder contract.
pub const CROSS_PROVIDER_RUNTIME_BINDER_SCHEMA_VERSION: &str =
    "psionic.cross_provider_runtime_binder.v1";
/// Stable canonical fixture path for the runtime binder contract.
pub const CROSS_PROVIDER_RUNTIME_BINDER_FIXTURE_PATH: &str =
    "fixtures/training/cross_provider_runtime_binder_v1.json";
/// Stable checker path for the runtime binder contract.
pub const CROSS_PROVIDER_RUNTIME_BINDER_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-provider-runtime-binder.sh";
/// Stable reference doc path for the runtime binder contract.
pub const CROSS_PROVIDER_RUNTIME_BINDER_DOC_PATH: &str =
    "docs/CROSS_PROVIDER_RUNTIME_BINDER_REFERENCE.md";

const GOOGLE_SINGLE_NODE_RUNBOOK_PATH: &str = "docs/PSION_GOOGLE_SINGLE_GPU_RUNBOOK.md";
const GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH: &str = "docs/PSION_GOOGLE_TWO_NODE_SWARM_RUNBOOK.md";
const RUNPOD_8XH100_RUNBOOK_PATH: &str = "docs/PARAMETER_GOLF_RUNPOD_8XH100_RUNBOOK.md";
const FIRST_SWARM_TRUSTED_LAN_RUNBOOK_PATH: &str = "docs/FIRST_SWARM_TRUSTED_LAN_RUNBOOK.md";
const GOOGLE_QUOTA_PREFLIGHT_POLICY_PATH: &str =
    "fixtures/psion/google/psion_google_machine_quota_preflight_v1.json";
const GOOGLE_SWARM_PREFLIGHT_POLICY_PATH: &str =
    "fixtures/psion/google/psion_google_two_node_swarm_preflight_v1.json";
const RUNPOD_OPERATOR_PREFLIGHT_POLICY_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_operator_preflight_policy_v1.json";
const FIRST_SWARM_TOPOLOGY_CONTRACT_PATH: &str =
    "fixtures/swarm/first_swarm_trusted_lan_topology_contract_v1.json";

/// Errors surfaced while building, validating, or writing the runtime binder contract.
#[derive(Debug, Error)]
pub enum CrossProviderRuntimeBinderError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    ComputeSource(#[from] CrossProviderComputeSourceContractError),
    #[error(transparent)]
    LaunchContract(#[from] CrossProviderLaunchContractError),
    #[error(transparent)]
    RemoteArtifactBackend(#[from] RemoteTrainArtifactBackendContractError),
    #[error("runtime binder contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Provider adapter class behind one concrete runtime binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderRuntimeAdapterKind {
    /// Google-managed host lane with quota and bucket preflight.
    GoogleHost,
    /// Google configured-peer cluster lane.
    GoogleConfiguredPeerCluster,
    /// RunPod remote pod lane.
    RunPodRemotePod,
    /// Local trusted-LAN bundle lane.
    LocalTrustedLanBundle,
}

/// Typed provider-side hook consumed by one binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderRuntimeHookKind {
    /// Preflight or quota gate before resource creation.
    Preflight,
    /// Provider-side resource creation or launch invocation.
    Launch,
    /// Startup materialization hook.
    Startup,
    /// Runtime hook that materializes or validates the env contract.
    RuntimeEnv,
    /// Finalizer or closeout hook.
    Finalizer,
    /// Evidence or after-action sealing hook.
    EvidenceSeal,
}

/// One provider-side hook retained by the shared binder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderRuntimeHook {
    /// Hook kind.
    pub hook_kind: CrossProviderRuntimeHookKind,
    /// Repo-local authority path.
    pub authority_path: String,
    /// Machine-legible detail.
    pub detail: String,
}

/// One runtime environment contract surfaced by the binder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderBoundRuntimeEnv {
    /// Environment variable name.
    pub name: String,
    /// Value template frozen by the launch contract.
    pub value_template: String,
    /// Phases that require the variable.
    pub phases: Vec<CrossProviderLaunchPhase>,
}

/// One class-specific artifact binding retained by the shared binder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderBoundArtifactClass {
    /// Artifact class under the binding.
    pub artifact_class: TrainArtifactClass,
    /// Authoritative backend id.
    pub authoritative_backend_id: String,
    /// Mirror backends that must also retain the artifact.
    pub mirror_backend_ids: Vec<String>,
    /// Finalizer projection ids required for this class.
    pub finalizer_projection_ids: Vec<String>,
    /// Machine-legible detail.
    pub detail: String,
}

/// One provider-neutral binding record projected into one concrete provider lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderRuntimeBindingRecord {
    /// Stable binding id.
    pub binding_id: String,
    /// Shared training-program manifest id.
    pub program_manifest_id: String,
    /// Shared training-program manifest digest.
    pub program_manifest_digest: String,
    /// Stable launch-contract id consumed by this binding.
    pub launch_contract_id: String,
    /// Stable launch-contract digest consumed by this binding.
    pub launch_contract_digest: String,
    /// Stable source id.
    pub source_id: String,
    /// Admitted compute-source contract id that justifies the binding.
    pub admitted_source_contract_id: String,
    /// Shared requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Provider adapter kind.
    pub adapter_kind: CrossProviderRuntimeAdapterKind,
    /// Stable runbook path for the lane.
    pub runbook_path: String,
    /// Provider-side hooks still left to the adapter layer.
    pub provider_hooks: Vec<CrossProviderRuntimeHook>,
    /// Shared runtime env contract.
    pub bound_runtime_env: Vec<CrossProviderBoundRuntimeEnv>,
    /// Shared artifact bindings projected into provider roots.
    pub bound_artifact_classes: Vec<CrossProviderBoundArtifactClass>,
    /// Startup entrypoint inherited from the launch contract.
    pub startup_entrypoint: String,
    /// Finalizer entrypoint inherited from the launch contract.
    pub finalizer_entrypoint: String,
    /// Provider-specific projected launch sequence.
    pub projected_step_ids: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable binding digest.
    pub binding_digest: String,
}

impl CrossProviderRuntimeBindingRecord {
    /// Returns the stable digest over the binding record.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.binding_digest.clear();
        stable_digest(b"psionic_cross_provider_runtime_binding_record|", &clone)
    }
}

/// Canonical provider-neutral runtime binder contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderRuntimeBinderContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable training-program manifest id.
    pub program_manifest_id: String,
    /// Stable training-program manifest digest.
    pub program_manifest_digest: String,
    /// Admitted source ids under the current binder closure.
    pub admitted_source_ids: Vec<String>,
    /// Admitted launch contracts under the current binder closure.
    pub admitted_launch_contract_ids: Vec<String>,
    /// Concrete binding records.
    pub binding_records: Vec<CrossProviderRuntimeBindingRecord>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl CrossProviderRuntimeBinderContract {
    /// Returns the stable digest over the binder contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_cross_provider_runtime_binder_contract|", &clone)
    }

    /// Validates the contract against the canonical manifest, sources, launch contracts, and backends.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        sources: &[CrossProviderComputeSourceContract],
        launches: &[CrossProviderLaunchContract],
    ) -> Result<(), CrossProviderRuntimeBinderError> {
        if self.schema_version != CROSS_PROVIDER_RUNTIME_BINDER_SCHEMA_VERSION {
            return Err(CrossProviderRuntimeBinderError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_RUNTIME_BINDER_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(CrossProviderRuntimeBinderError::InvalidContract {
                detail: String::from("program-manifest binding drifted"),
            });
        }
        let source_ids: BTreeSet<_> = sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect();
        let launch_ids: BTreeSet<_> = launches
            .iter()
            .map(|launch| launch.launch_contract_id.as_str())
            .collect();
        let admitted_sources: BTreeSet<_> = self
            .admitted_source_ids
            .iter()
            .map(String::as_str)
            .collect();
        let admitted_launches: BTreeSet<_> = self
            .admitted_launch_contract_ids
            .iter()
            .map(String::as_str)
            .collect();
        if admitted_sources != source_ids {
            return Err(CrossProviderRuntimeBinderError::InvalidContract {
                detail: String::from("admitted_source_ids drifted from canonical source contracts"),
            });
        }
        if admitted_launches != launch_ids {
            return Err(CrossProviderRuntimeBinderError::InvalidContract {
                detail: String::from(
                    "admitted_launch_contract_ids drifted from canonical launch contracts",
                ),
            });
        }
        if self.binding_records.len() != launches.len() {
            return Err(CrossProviderRuntimeBinderError::InvalidContract {
                detail: format!(
                    "binding_records must stay aligned with launch contracts: expected {}, found {}",
                    launches.len(),
                    self.binding_records.len()
                ),
            });
        }
        let mut seen_binding_ids = BTreeSet::new();
        for record in &self.binding_records {
            if !seen_binding_ids.insert(record.binding_id.as_str()) {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!("binding_id `{}` repeated", record.binding_id),
                });
            }
            let launch = launches
                .iter()
                .find(|launch| launch.launch_contract_id == record.launch_contract_id)
                .ok_or_else(|| CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` referenced unknown launch contract `{}`",
                        record.binding_id, record.launch_contract_id
                    ),
                })?;
            if record.launch_contract_digest != launch.launch_contract_digest {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` launch digest drifted from `{}`",
                        record.binding_id, record.launch_contract_id
                    ),
                });
            }
            if record.source_id != launch.source_binding.source_id {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` source_id drifted from launch source binding",
                        record.binding_id
                    ),
                });
            }
            if record.admitted_source_contract_id
                != resolved_source_for_launch(sources, launch)?.source_id
            {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` admitted source contract drifted from canonical source resolution",
                        record.binding_id
                    ),
                });
            }
            if record.requested_execution_class != launch.requested_execution_class {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` requested_execution_class drifted from launch contract",
                        record.binding_id
                    ),
                });
            }
            if record.bound_runtime_env.len() != launch.runtime_env.len() {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` lost one or more runtime env entries",
                        record.binding_id
                    ),
                });
            }
            let projected_step_ids: BTreeSet<_> = record
                .projected_step_ids
                .iter()
                .map(String::as_str)
                .collect();
            let launch_step_ids: BTreeSet<_> = launch
                .projected_steps
                .iter()
                .map(|step| step.step_id.as_str())
                .collect();
            if projected_step_ids != launch_step_ids {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` projected_step_ids drifted from launch contract",
                        record.binding_id
                    ),
                });
            }
            if record.startup_entrypoint != launch.startup_plan.startup_entrypoint
                || record.finalizer_entrypoint != launch.finalizer_plan.finalizer_entrypoint
            {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` startup or finalizer entrypoint drifted from launch contract",
                        record.binding_id
                    ),
                });
            }
            if record.provider_hooks.is_empty() {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!("binding `{}` must retain provider hooks", record.binding_id),
                });
            }
            if record.bound_artifact_classes.len() < 4 {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!(
                        "binding `{}` must retain at least checkpoint, log, metrics, and final evidence bindings",
                        record.binding_id
                    ),
                });
            }
            if record.binding_digest != record.stable_digest() {
                return Err(CrossProviderRuntimeBinderError::InvalidContract {
                    detail: format!("binding `{}` digest drifted", record.binding_id),
                });
            }
        }
        if self.contract_digest != self.stable_digest() {
            return Err(CrossProviderRuntimeBinderError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical provider-neutral runtime binder contract.
static CROSS_PROVIDER_RUNTIME_BINDER_CACHE: OnceLock<CrossProviderRuntimeBinderContract> =
    OnceLock::new();

pub fn canonical_cross_provider_runtime_binder(
) -> Result<CrossProviderRuntimeBinderContract, CrossProviderRuntimeBinderError> {
    if let Some(contract) = CROSS_PROVIDER_RUNTIME_BINDER_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let launches = canonical_cross_provider_launch_contracts()?;
    let artifact_backends = canonical_remote_train_artifact_backend_contract_set()?;

    let placement_map: BTreeMap<_, _> = artifact_backends
        .placement_decisions
        .iter()
        .map(|decision| (decision.artifact_class, decision.clone()))
        .collect();
    let projection_map: BTreeMap<_, Vec<_>> = artifact_backends.finalizer_projections.iter().fold(
        BTreeMap::<TrainArtifactClass, Vec<String>>::new(),
        |mut map, projection| {
            map.entry(projection.artifact_class)
                .or_default()
                .push(projection.projection_id.clone());
            map
        },
    );
    let binding_records = launches
        .iter()
        .map(|launch| {
            let resolved_source = resolved_source_for_launch(&sources, launch)
                .expect("canonical launch contracts must resolve to canonical source contracts");
            binding_record_for_launch(
                &manifest,
                resolved_source,
                launch,
                &placement_map,
                &projection_map,
            )
        })
        .collect::<Vec<_>>();
    let mut contract = CrossProviderRuntimeBinderContract {
        schema_version: String::from(CROSS_PROVIDER_RUNTIME_BINDER_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        admitted_source_ids: sources.iter().map(|source| source.source_id.clone()).collect(),
        admitted_launch_contract_ids: launches
            .iter()
            .map(|launch| launch.launch_contract_id.clone())
            .collect(),
        binding_records,
        claim_boundary: String::from(
            "This binder closes one provider-neutral mapping from the cross-provider training-program manifest plus admitted compute sources into concrete launch contracts, runtime env, startup plans, finalizer plans, and provider hooks for the current Google, RunPod, and local lanes. It does not claim dense runtime parity, provider API automation beyond the retained adapter hooks, or same-job mixed-backend dense training.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate(&manifest, &sources, &launches)?;
    let _ = CROSS_PROVIDER_RUNTIME_BINDER_CACHE.set(contract.clone());
    Ok(contract)
}

/// Writes the canonical runtime binder fixture.
pub fn write_cross_provider_runtime_binder(
    output_path: impl AsRef<Path>,
) -> Result<(), CrossProviderRuntimeBinderError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| CrossProviderRuntimeBinderError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_cross_provider_runtime_binder()?;
    let json = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, json).map_err(|error| CrossProviderRuntimeBinderError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn binding_record_for_launch(
    manifest: &CrossProviderTrainingProgramManifest,
    source: &CrossProviderComputeSourceContract,
    launch: &CrossProviderLaunchContract,
    placement_map: &BTreeMap<TrainArtifactClass, crate::RemoteTrainArtifactPlacementDecision>,
    projection_map: &BTreeMap<TrainArtifactClass, Vec<String>>,
) -> CrossProviderRuntimeBindingRecord {
    let (adapter_kind, runbook_path, provider_hooks) = provider_surface_for_launch(launch);
    let bound_runtime_env = launch
        .runtime_env
        .iter()
        .map(|entry| CrossProviderBoundRuntimeEnv {
            name: entry.name.clone(),
            value_template: entry.value_template.clone(),
            phases: entry.phases.clone(),
        })
        .collect();
    let bound_artifact_classes = [
        TrainArtifactClass::Checkpoint,
        TrainArtifactClass::LogBundle,
        TrainArtifactClass::MetricsBundle,
        TrainArtifactClass::FinalEvidenceBundle,
    ]
    .into_iter()
    .map(|artifact_class| {
        let placement = placement_map
            .get(&artifact_class)
            .expect("canonical placement decisions must cover all required artifact classes");
        let finalizer_projection_ids = projection_map
            .get(&artifact_class)
            .cloned()
            .unwrap_or_default();
        CrossProviderBoundArtifactClass {
            artifact_class,
            authoritative_backend_id: placement.primary_backend_id.clone(),
            mirror_backend_ids: placement.mirror_backend_ids.clone(),
            finalizer_projection_ids,
            detail: format!(
                "The shared binder projects {:?} for source `{}` through the provider-neutral remote artifact backend policy.",
                artifact_class, source.source_id
            ),
        }
    })
    .collect();
    let mut record = CrossProviderRuntimeBindingRecord {
        binding_id: format!(
            "{}.{}.binding",
            manifest.program_manifest_id, launch.launch_contract_id
        ),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        launch_contract_id: launch.launch_contract_id.clone(),
        launch_contract_digest: launch.launch_contract_digest.clone(),
        source_id: launch.source_binding.source_id.clone(),
        admitted_source_contract_id: source.source_id.clone(),
        requested_execution_class: launch.requested_execution_class,
        adapter_kind,
        runbook_path: String::from(runbook_path),
        provider_hooks,
        bound_runtime_env,
        bound_artifact_classes,
        startup_entrypoint: launch.startup_plan.startup_entrypoint.clone(),
        finalizer_entrypoint: launch.finalizer_plan.finalizer_entrypoint.clone(),
        projected_step_ids: launch
            .projected_steps
            .iter()
            .map(|step| step.step_id.clone())
            .collect(),
        claim_boundary: format!(
            "This binding keeps `{}` on one shared runtime contract above the provider adapter. It does not widen the lane beyond its current execution-class and evidence boundary.",
            launch.launch_contract_id
        ),
        binding_digest: String::new(),
    };
    record.binding_digest = record.stable_digest();
    record
}

fn resolved_source_for_launch<'a>(
    sources: &'a [CrossProviderComputeSourceContract],
    launch: &CrossProviderLaunchContract,
) -> Result<&'a CrossProviderComputeSourceContract, CrossProviderRuntimeBinderError> {
    if let Some(source) = sources
        .iter()
        .find(|source| source.source_id == launch.source_binding.source_id)
    {
        return Ok(source);
    }
    sources
        .iter()
        .find(|source| {
            source.source_class == launch.source_binding.source_class
                && source.provider == launch.source_binding.provider
                && source.backend.backend_family == launch.source_binding.backend_family
        })
        .ok_or_else(|| CrossProviderRuntimeBinderError::InvalidContract {
            detail: format!(
                "launch contract `{}` could not resolve an admitted compute source",
                launch.launch_contract_id
            ),
        })
}

fn provider_surface_for_launch(
    launch: &CrossProviderLaunchContract,
) -> (
    CrossProviderRuntimeAdapterKind,
    &'static str,
    Vec<CrossProviderRuntimeHook>,
) {
    match launch.binder_kind {
        CrossProviderLaunchBinderKind::GoogleSingleNode => (
            CrossProviderRuntimeAdapterKind::GoogleHost,
            GOOGLE_SINGLE_NODE_RUNBOOK_PATH,
            vec![
                hook(
                    CrossProviderRuntimeHookKind::Preflight,
                    GOOGLE_QUOTA_PREFLIGHT_POLICY_PATH,
                    "Google single-node quota and machine preflight remain adapter-owned, but the binder now names them explicitly.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Launch,
                    "scripts/psion-google-launch-single-node.sh",
                    "Host creation remains Google-specific, but it is now a provider hook under the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Startup,
                    "scripts/psion-google-single-node-startup.sh",
                    "Host startup is still metadata-driven, but the runtime env and artifact roots now come from the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::RuntimeEnv,
                    GOOGLE_SINGLE_NODE_RUNBOOK_PATH,
                    "The Google runbook now consumes shared runtime env and finalizer semantics instead of defining them ad hoc.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Finalizer,
                    "scripts/psion-google-finalize-run.sh",
                    "Final evidence sealing remains Google-hosted, but its inputs and outputs now come from the shared binder.",
                ),
            ],
        ),
        CrossProviderLaunchBinderKind::GoogleTwoNodeSwarm => (
            CrossProviderRuntimeAdapterKind::GoogleConfiguredPeerCluster,
            GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH,
            vec![
                hook(
                    CrossProviderRuntimeHookKind::Preflight,
                    GOOGLE_SWARM_PREFLIGHT_POLICY_PATH,
                    "Dual-node Google network, quota, and identity preflight remain adapter-owned under the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Launch,
                    "scripts/psion-google-launch-two-node-swarm.sh",
                    "Cluster bring-up remains Google-specific resource work above the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Startup,
                    "scripts/psion-google-two-node-swarm-startup.sh",
                    "Swarm node startup keeps its Google implementation while consuming the shared env and artifact layout.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::RuntimeEnv,
                    GOOGLE_TWO_NODE_SWARM_RUNBOOK_PATH,
                    "The configured-peer runtime now projects cluster namespace, ports, and artifact roots from the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::EvidenceSeal,
                    "scripts/psion-google-finalize-two-node-swarm-run.sh",
                    "Swarm finalization stays provider-owned while using the shared finalizer plan and evidence family.",
                ),
            ],
        ),
        CrossProviderLaunchBinderKind::RunPodSinglePod => (
            CrossProviderRuntimeAdapterKind::RunPodRemotePod,
            RUNPOD_8XH100_RUNBOOK_PATH,
            vec![
                hook(
                    CrossProviderRuntimeHookKind::Preflight,
                    RUNPOD_OPERATOR_PREFLIGHT_POLICY_PATH,
                    "RunPod operator preflight remains adapter-owned and is now retained explicitly by the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Launch,
                    "scripts/parameter-golf-runpod-launch-8xh100.sh",
                    "Pod launch and SSH setup remain RunPod-specific resource work above the binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::RuntimeEnv,
                    RUNPOD_8XH100_RUNBOOK_PATH,
                    "RunPod runtime env, workspace roots, and visualization roots now come from the shared binder instead of lane-local assumptions.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Finalizer,
                    "scripts/parameter-golf-runpod-finalize-8xh100.sh",
                    "The RunPod finalizer still seals remote evidence, but its inputs and outputs now come from the shared binder.",
                ),
            ],
        ),
        CrossProviderLaunchBinderKind::LocalTrustedLan => (
            CrossProviderRuntimeAdapterKind::LocalTrustedLanBundle,
            FIRST_SWARM_TRUSTED_LAN_RUNBOOK_PATH,
            vec![
                hook(
                    CrossProviderRuntimeHookKind::Preflight,
                    FIRST_SWARM_TOPOLOGY_CONTRACT_PATH,
                    "Trusted-LAN topology, stale thresholds, and host identities remain explicit adapter inputs under the shared binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Launch,
                    "scripts/first-swarm-launch-trusted-lan.sh",
                    "Local staging and operator launch remain host-owned resource work above the binder.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::RuntimeEnv,
                    FIRST_SWARM_TRUSTED_LAN_RUNBOOK_PATH,
                    "The local swarm runbook now projects shared runtime env and final-root semantics instead of lane-local naming.",
                ),
                hook(
                    CrossProviderRuntimeHookKind::Finalizer,
                    "cargo run -q -p psionic-train --bin first_swarm_trusted_lan_closeout_report",
                    "Local closeout still stays host-owned, but the binder now names its expected env and evidence outputs.",
                ),
            ],
        ),
    }
}

fn hook(
    hook_kind: CrossProviderRuntimeHookKind,
    authority_path: impl Into<String>,
    detail: impl Into<String>,
) -> CrossProviderRuntimeHook {
    CrossProviderRuntimeHook {
        hook_kind,
        authority_path: authority_path.into(),
        detail: detail.into(),
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("cross-provider runtime binder values must serialize"),
    );
    format!("{:x}", hasher.finalize())
}
