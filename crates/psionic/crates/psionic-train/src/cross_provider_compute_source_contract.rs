use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use psionic_environments::EnvironmentPackageKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    cross_provider_training_program_manifest, CrossProviderComputeSourceClass,
    CrossProviderExecutionClass, CrossProviderTrainingProgramManifest,
    FirstSwarmLinuxCudaBringupReport, FirstSwarmMacMlxBringupReport,
    PsionGoogleTwoNodeSwarmContract, OPEN_ADAPTER_CUDA_BACKEND_LABEL,
    OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL, PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH,
    PSION_GOOGLE_TWO_NODE_SWARM_IDENTITY_PROFILE_PATH,
    PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
    PSION_GOOGLE_TWO_NODE_SWARM_NETWORK_POSTURE_PATH, SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH,
    SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
};

/// Stable schema version for the cross-provider compute-source contract family.
pub const CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.cross_provider_compute_source_contract.v1";
/// Stable fixture directory for canonical compute-source examples.
pub const CROSS_PROVIDER_COMPUTE_SOURCE_FIXTURE_DIR: &str = "fixtures/training/compute_sources";
/// Stable canonical Google compute-source fixture path.
pub const CROSS_PROVIDER_GOOGLE_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/google_l4_validator_node_v1.json";
/// Stable canonical RunPod compute-source fixture path.
pub const CROSS_PROVIDER_RUNPOD_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/runpod_8xh100_dense_node_v1.json";
/// Stable canonical local NVIDIA compute-source fixture path.
pub const CROSS_PROVIDER_LOCAL_RTX4080_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/local_rtx4080_workstation_v1.json";
/// Stable canonical local Apple compute-source fixture path.
pub const CROSS_PROVIDER_LOCAL_MLX_MAC_COMPUTE_SOURCE_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/local_mlx_mac_workstation_v1.json";
/// Stable canonical planner-input fixture path.
pub const CROSS_PROVIDER_COMPUTE_SOURCE_PLANNER_INPUT_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/planner_input_v1.json";
/// Stable canonical launch-input fixture path.
pub const CROSS_PROVIDER_COMPUTE_SOURCE_LAUNCH_INPUTS_FIXTURE_PATH: &str =
    "fixtures/training/compute_sources/launch_inputs_v1.json";
/// Stable reference doc path.
pub const CROSS_PROVIDER_COMPUTE_SOURCE_DOC_PATH: &str =
    "docs/COMPUTE_SOURCE_CONTRACT_REFERENCE.md";
/// Stable checker path.
pub const CROSS_PROVIDER_COMPUTE_SOURCE_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-provider-compute-source-contracts.sh";

const CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_launch_profiles_v1.json";
const CROSS_PROVIDER_RUNPOD_COST_GUARDRAILS_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_cost_guardrails_v1.json";
const CROSS_PROVIDER_RUNPOD_OPERATOR_PREFLIGHT_PATH: &str =
    "fixtures/parameter_golf/runpod/parameter_golf_runpod_8xh100_operator_preflight_policy_v1.json";

/// Errors surfaced while building, validating, or writing compute-source contracts.
#[derive(Debug, Error)]
pub enum CrossProviderComputeSourceContractError {
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
    #[error("cross-provider compute-source contract is invalid: {detail}")]
    InvalidContract { detail: String },
    #[error("cross-provider planner input is invalid: {detail}")]
    InvalidPlannerInput { detail: String },
    #[error("cross-provider launch inputs are invalid: {detail}")]
    InvalidLaunchInputs { detail: String },
    #[error("fixture `{path}` is missing required field `{field}`")]
    MissingField { path: String, field: String },
}

/// Provider family behind one training-facing compute source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderComputeProviderKind {
    /// Google Cloud managed infrastructure.
    GoogleCloud,
    /// RunPod rented pod inventory.
    RunPod,
    /// Operator-managed local hardware.
    LocalOperatorManaged,
}

/// Locality class for one training-facing compute source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderLocalityKind {
    /// Private cloud node inside one regional VPC.
    RegionalPrivateCloudNode,
    /// Single rented pod with its own provider-local runtime envelope.
    SingleRentedPod,
    /// One operator-managed workstation on a local network.
    SingleLocalWorkstation,
}

/// Backend family surfaced by one compute-source contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderBackendFamily {
    /// CUDA-backed execution.
    Cuda,
    /// MLX on Apple Metal.
    MlxMetal,
}

/// Trust tier used in admission and launch policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderTrustTier {
    /// Operator-controlled private cloud node.
    PrivateCloudOperatorManaged,
    /// Rented but operator-configured pod.
    RentedProviderOperatorManaged,
    /// Local workstation under operator control.
    LocalOperatorManaged,
}

/// Storage authority class behind one compute-source contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderStorageKind {
    /// Cloud bucket plus local boot disk.
    RemoteBucketPlusLocalDisk,
    /// Persistent provider workspace.
    PersistentProviderWorkspace,
    /// Local filesystem only.
    LocalFilesystemOnly,
}

/// Cost accounting model behind one compute-source contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderCostModel {
    /// Provider spend is bounded by explicit guardrails.
    ProviderGuardrailed,
    /// Local operator-owned workstation with no provider-metered price surface.
    LocalOperatorOwned,
}

/// Typed refusal reason when one source cannot satisfy an execution-class request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderExecutionClassAdmissionRefusalKind {
    /// The root program manifest does not admit the requested source class.
    ProgramSourceClassNotAdmitted,
    /// The root program manifest does not admit the requested execution class.
    ProgramExecutionClassNotAdmitted,
    /// The concrete source contract does not admit the requested execution class.
    SourceExecutionClassNotAdmitted,
}

/// Repo-owned authority artifact that this contract depends on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceAuthorityArtifact {
    /// Repo-local path.
    pub path: String,
    /// SHA256 over the current artifact bytes.
    pub sha256: String,
    /// Why the artifact still matters to this source contract.
    pub detail: String,
}

/// Locality facts for one compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceLocality {
    /// Locality class.
    pub locality_kind: CrossProviderLocalityKind,
    /// Region when the source is not purely local.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Zone or placement when the source retains one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone_or_placement: Option<String>,
    /// Cluster namespace when the source participates in clustered traffic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_namespace: Option<String>,
    /// Discovery or peer posture.
    pub discovery_posture: String,
    /// Short operator-facing detail.
    pub detail: String,
}

/// Accelerator inventory for one compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceAcceleratorInventory {
    /// Accelerator vendor label.
    pub accelerator_vendor: String,
    /// Accelerator model label.
    pub accelerator_model: String,
    /// Accelerator count available to the source.
    pub accelerator_count: u16,
    /// Per-accelerator memory when explicitly retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_accelerator_memory_bytes: Option<u64>,
    /// Whether the source uses unified memory.
    pub unified_memory: bool,
    /// Whether the source requires non-MIG inventory.
    pub require_non_mig: bool,
    /// Short detail for operator-facing comparison.
    pub detail: String,
}

/// Backend posture for one compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceBackendPosture {
    /// Backend family.
    pub backend_family: CrossProviderBackendFamily,
    /// Runtime backend label or short backend id.
    pub runtime_backend_label: String,
    /// Precision posture kept explicit for admission.
    pub precision_posture: String,
    /// Whether this source truthfully claims mixed-backend dense participation.
    pub admits_mixed_backend_dense: bool,
    /// Short detail for operator-facing comparison.
    pub detail: String,
}

/// Network posture for one compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceNetworkPosture {
    /// Trust tier for cluster traffic.
    pub trust_tier: CrossProviderTrustTier,
    /// Whether source-to-source traffic stays on private connectivity.
    pub private_connectivity: bool,
    /// Reserved cluster ports when retained by the source contract.
    pub cluster_port_bindings: Vec<u16>,
    /// Ingress posture label.
    pub ingress_posture: String,
    /// Egress posture label.
    pub egress_posture: String,
    /// Impairment or shaping posture.
    pub impairment_posture: String,
    /// Short detail for operator-facing comparison.
    pub detail: String,
}

/// Storage posture for one compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceStoragePosture {
    /// Storage class.
    pub storage_kind: CrossProviderStorageKind,
    /// Local workspace root when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    /// Remote artifact root when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_artifact_root: Option<String>,
    /// Remote checkpoint root when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_checkpoint_root: Option<String>,
    /// Local disk or scratch bytes when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_disk_budget_bytes: Option<u64>,
    /// Whether the source may write shared checkpoints.
    pub checkpoint_writer_capable: bool,
    /// Short detail for operator-facing comparison.
    pub detail: String,
}

/// Cost posture for one compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceCostPosture {
    /// Cost model.
    pub cost_model: CrossProviderCostModel,
    /// Declared run ceiling in USD cents when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_run_cost_ceiling_usd_cents: Option<u32>,
    /// Declared hourly price in USD cents when retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_hourly_cost_usd_cents: Option<u32>,
    /// Short detail for operator-facing comparison.
    pub detail: String,
}

/// Typed refusal surfaced when a requested execution class is not admitted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderExecutionClassAdmissionRefusal {
    /// Stable source identifier.
    pub source_id: String,
    /// Requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Typed refusal kind.
    pub refusal_kind: CrossProviderExecutionClassAdmissionRefusalKind,
    /// Explicit refusal detail.
    pub detail: String,
}

/// Canonical training-facing compute-source contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderComputeSourceContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source class already admitted by the root program manifest.
    pub source_class: CrossProviderComputeSourceClass,
    /// Provider family behind the source.
    pub provider: CrossProviderComputeProviderKind,
    /// Locality posture.
    pub locality: CrossProviderComputeSourceLocality,
    /// Accelerator inventory.
    pub accelerators: CrossProviderComputeSourceAcceleratorInventory,
    /// Backend posture.
    pub backend: CrossProviderComputeSourceBackendPosture,
    /// Network posture.
    pub network: CrossProviderComputeSourceNetworkPosture,
    /// Storage posture.
    pub storage: CrossProviderComputeSourceStoragePosture,
    /// Cost posture.
    pub cost: CrossProviderComputeSourceCostPosture,
    /// Execution classes the source contract explicitly admits.
    pub admitted_execution_classes: Vec<CrossProviderExecutionClass>,
    /// Typed refusals for the execution classes the source does not admit.
    pub refusal_examples: Vec<CrossProviderExecutionClassAdmissionRefusal>,
    /// Repo-owned authority artifacts behind the contract.
    pub authority_artifacts: Vec<CrossProviderComputeSourceAuthorityArtifact>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl CrossProviderComputeSourceContract {
    /// Returns the stable digest over the compute-source contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_cross_provider_compute_source_contract|", &clone)
    }

    /// Validates the compute-source contract against the cross-provider program manifest.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
    ) -> Result<(), CrossProviderComputeSourceContractError> {
        if self.schema_version != CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION {
            return Err(CrossProviderComputeSourceContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.source_id.trim().is_empty() {
            return Err(CrossProviderComputeSourceContractError::InvalidContract {
                detail: String::from("source_id must not be empty"),
            });
        }
        if !manifest
            .admitted_compute_source_classes
            .contains(&self.source_class)
        {
            return Err(CrossProviderComputeSourceContractError::InvalidContract {
                detail: format!(
                    "source_class `{:?}` is not admitted by the root program manifest",
                    self.source_class
                ),
            });
        }
        if self.admitted_execution_classes.is_empty() {
            return Err(CrossProviderComputeSourceContractError::InvalidContract {
                detail: String::from("admitted_execution_classes must not be empty"),
            });
        }
        require_exactly_one_refusal_per_unsupported_class(
            self,
            manifest.admitted_execution_classes.as_slice(),
        )?;
        if self.contract_digest != self.stable_digest() {
            return Err(CrossProviderComputeSourceContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable contract digest"),
            });
        }
        Ok(())
    }

    /// Returns the typed refusal for one requested execution class.
    #[must_use]
    pub fn refusal_for_class(
        &self,
        requested_execution_class: CrossProviderExecutionClass,
    ) -> Option<CrossProviderExecutionClassAdmissionRefusal> {
        self.refusal_examples
            .iter()
            .find(|refusal| refusal.requested_execution_class == requested_execution_class)
            .cloned()
    }

    /// Admits or refuses one execution-class request under the root program manifest.
    pub fn admit_execution_class(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        requested_execution_class: CrossProviderExecutionClass,
    ) -> Result<(), CrossProviderExecutionClassAdmissionRefusal> {
        if !manifest
            .admitted_compute_source_classes
            .contains(&self.source_class)
        {
            return Err(CrossProviderExecutionClassAdmissionRefusal {
                source_id: self.source_id.clone(),
                requested_execution_class,
                refusal_kind:
                    CrossProviderExecutionClassAdmissionRefusalKind::ProgramSourceClassNotAdmitted,
                detail: format!(
                    "program manifest `{}` does not admit source class `{:?}` for `{}`",
                    manifest.program_manifest_id, self.source_class, self.source_id
                ),
            });
        }
        if !manifest
            .admitted_execution_classes
            .contains(&requested_execution_class)
        {
            return Err(CrossProviderExecutionClassAdmissionRefusal {
                source_id: self.source_id.clone(),
                requested_execution_class,
                refusal_kind:
                    CrossProviderExecutionClassAdmissionRefusalKind::ProgramExecutionClassNotAdmitted,
                detail: format!(
                    "program manifest `{}` does not admit execution class `{:?}`",
                    manifest.program_manifest_id, requested_execution_class
                ),
            });
        }
        if self
            .admitted_execution_classes
            .contains(&requested_execution_class)
        {
            return Ok(());
        }
        Err(self.refusal_for_class(requested_execution_class).unwrap_or(
            CrossProviderExecutionClassAdmissionRefusal {
                source_id: self.source_id.clone(),
                requested_execution_class,
                refusal_kind:
                    CrossProviderExecutionClassAdmissionRefusalKind::SourceExecutionClassNotAdmitted,
                detail: format!(
                    "source `{}` does not admit execution class `{:?}`",
                    self.source_id, requested_execution_class
                ),
            },
        ))
    }
}

/// Expected disposition for one planner candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderPlannerCandidateDisposition {
    /// The candidate must admit the requested execution class.
    Admitted,
    /// The candidate must refuse the requested execution class.
    Refused,
}

/// One planner candidate built from a compute-source contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderPlannerCandidateInput {
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source-contract digest.
    pub source_contract_digest: String,
    /// Requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Expected planner disposition.
    pub expected_disposition: CrossProviderPlannerCandidateDisposition,
    /// Typed refusal when the disposition is refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_refusal: Option<CrossProviderExecutionClassAdmissionRefusal>,
}

/// Provider-neutral planner input for the first cross-provider admission examples.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderPlannerInput {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable program manifest id.
    pub program_manifest_id: String,
    /// Stable program manifest digest.
    pub program_manifest_digest: String,
    /// Candidate requests admitted or refused by the same planner contract.
    pub candidates: Vec<CrossProviderPlannerCandidateInput>,
    /// Stable planner-input digest.
    pub planner_input_digest: String,
}

impl CrossProviderPlannerInput {
    /// Returns the stable digest over the planner input.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.planner_input_digest.clear();
        stable_digest(
            b"psionic_cross_provider_compute_source_planner_input|",
            &clone,
        )
    }
}

/// Provider-neutral launch input derived from the root program manifest and one admitted source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderLaunchInput {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable program manifest id.
    pub program_manifest_id: String,
    /// Stable program manifest digest.
    pub program_manifest_digest: String,
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source-contract digest.
    pub source_contract_digest: String,
    /// Requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Stable run id.
    pub run_id: String,
    /// Environment package key projected from the program manifest.
    pub environment: EnvironmentPackageKey,
    /// Stable launch root.
    pub launch_root: String,
    /// Stable checkpoint root.
    pub checkpoint_root: String,
    /// Stable metrics root.
    pub metrics_root: String,
    /// Stable visualization root.
    pub visualization_root: String,
    /// Stable final root.
    pub final_root: String,
    /// Cluster-port binding when the requested class participates in cluster traffic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_port_binding: Option<u16>,
    /// Stable launch-input digest.
    pub launch_input_digest: String,
}

impl CrossProviderLaunchInput {
    /// Returns the stable digest over the launch input.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.launch_input_digest.clear();
        stable_digest(
            b"psionic_cross_provider_compute_source_launch_input|",
            &clone,
        )
    }
}

/// Returns the four canonical compute-source contracts for the first cross-provider examples.
static CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACTS_CACHE: OnceLock<
    Vec<CrossProviderComputeSourceContract>,
> = OnceLock::new();

pub fn canonical_cross_provider_compute_source_contracts(
) -> Result<Vec<CrossProviderComputeSourceContract>, CrossProviderComputeSourceContractError> {
    if let Some(contracts) = CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACTS_CACHE.get() {
        return Ok(contracts.clone());
    }
    let manifest = canonical_program_manifest()?;
    let contracts = vec![
        google_l4_validator_compute_source_contract()?,
        runpod_8xh100_dense_compute_source_contract()?,
        local_rtx4080_compute_source_contract()?,
        local_mlx_mac_compute_source_contract()?,
    ];
    for contract in &contracts {
        contract.validate(&manifest)?;
    }
    let _ = CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACTS_CACHE.set(contracts.clone());
    Ok(contracts)
}

/// Returns the canonical planner input built from the canonical compute-source contracts.
pub fn canonical_cross_provider_planner_input(
) -> Result<CrossProviderPlannerInput, CrossProviderComputeSourceContractError> {
    let manifest = canonical_program_manifest()?;
    let contracts = canonical_cross_provider_compute_source_contracts()?;
    let by_id = contracts_by_id(contracts.as_slice());
    let mut planner = CrossProviderPlannerInput {
        schema_version: String::from(CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        candidates: vec![
            planner_candidate(
                &by_id,
                &manifest,
                "google_l4_validator_node",
                CrossProviderExecutionClass::Validator,
            )?,
            planner_candidate(
                &by_id,
                &manifest,
                "runpod_8xh100_dense_node",
                CrossProviderExecutionClass::DenseFullModelRank,
            )?,
            planner_candidate(
                &by_id,
                &manifest,
                "local_rtx4080_workstation",
                CrossProviderExecutionClass::ValidatedContributorWindow,
            )?,
            planner_candidate(
                &by_id,
                &manifest,
                "local_mlx_mac_workstation",
                CrossProviderExecutionClass::DataBuilder,
            )?,
            planner_candidate(
                &by_id,
                &manifest,
                "local_mlx_mac_workstation",
                CrossProviderExecutionClass::DenseFullModelRank,
            )?,
        ],
        planner_input_digest: String::new(),
    };
    planner.planner_input_digest = planner.stable_digest();
    validate_planner_input(&planner, &manifest, contracts.as_slice())?;
    Ok(planner)
}

/// Returns the canonical launch inputs built from the canonical compute-source contracts.
pub fn canonical_cross_provider_launch_inputs(
) -> Result<Vec<CrossProviderLaunchInput>, CrossProviderComputeSourceContractError> {
    let manifest = canonical_program_manifest()?;
    let contracts = canonical_cross_provider_compute_source_contracts()?;
    let by_id = contracts_by_id(contracts.as_slice());
    let inputs = vec![
        build_launch_input(
            &manifest,
            &by_id,
            "google_l4_validator_node",
            CrossProviderExecutionClass::Validator,
            "psion-xprovider-pretrain-google-validator-example",
        )?,
        build_launch_input(
            &manifest,
            &by_id,
            "runpod_8xh100_dense_node",
            CrossProviderExecutionClass::DenseFullModelRank,
            "psion-xprovider-pretrain-runpod-dense-example",
        )?,
        build_launch_input(
            &manifest,
            &by_id,
            "local_rtx4080_workstation",
            CrossProviderExecutionClass::ValidatedContributorWindow,
            "psion-xprovider-pretrain-local-rtx4080-window-example",
        )?,
        build_launch_input(
            &manifest,
            &by_id,
            "local_mlx_mac_workstation",
            CrossProviderExecutionClass::DataBuilder,
            "psion-xprovider-pretrain-local-mac-data-example",
        )?,
    ];
    validate_launch_inputs(inputs.as_slice(), &manifest, contracts.as_slice())?;
    Ok(inputs)
}

/// Writes the canonical compute-source fixtures into the supplied directory.
pub fn write_cross_provider_compute_source_contracts(
    output_dir: impl AsRef<Path>,
) -> Result<(), CrossProviderComputeSourceContractError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| {
        CrossProviderComputeSourceContractError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let contracts = canonical_cross_provider_compute_source_contracts()?;
    let planner_input = canonical_cross_provider_planner_input()?;
    let launch_inputs = canonical_cross_provider_launch_inputs()?;
    for (path, contract) in canonical_fixture_targets(output_dir)?
        .into_iter()
        .zip(contracts.iter())
    {
        write_json(path, contract)?;
    }
    write_json(output_dir.join("planner_input_v1.json"), &planner_input)?;
    write_json(output_dir.join("launch_inputs_v1.json"), &launch_inputs)?;
    Ok(())
}

fn google_l4_validator_compute_source_contract(
) -> Result<CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError> {
    let contract: PsionGoogleTwoNodeSwarmContract =
        read_json_file(PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH)?;
    let launch_profiles = read_json_value(PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH)?;
    let network_posture = read_json_value(PSION_GOOGLE_TWO_NODE_SWARM_NETWORK_POSTURE_PATH)?;
    let identity_profile = read_json_value(PSION_GOOGLE_TWO_NODE_SWARM_IDENTITY_PROFILE_PATH)?;
    let coordinator = contract
        .nodes
        .iter()
        .find(|node| {
            node.role_id == "psion.google_swarm.coordinator_validator_aggregator_contributor"
        })
        .ok_or_else(
            || CrossProviderComputeSourceContractError::InvalidContract {
                detail: String::from("missing Google coordinator node in swarm contract"),
            },
        )?;
    let profile = launch_profiles
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|profiles| {
            profiles.iter().find(|profile| {
                profile.get("profile_id").and_then(Value::as_str)
                    == Some(coordinator.launch_profile_id.as_str())
            })
        })
        .ok_or_else(|| CrossProviderComputeSourceContractError::MissingField {
            path: PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH.to_string(),
            field: format!("profiles[profile_id={}]", coordinator.launch_profile_id),
        })?;
    let target_tags = identity_profile
        .get("node_roles")
        .and_then(Value::as_array)
        .and_then(|roles| {
            roles.iter().find(|role| {
                role.get("role_id").and_then(Value::as_str) == Some(coordinator.role_id.as_str())
            })
        })
        .and_then(|role| role.get("target_tags"))
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| String::from("not_retained"));
    let bucket_url = contract.bucket_url.clone();
    let mut source = CrossProviderComputeSourceContract {
        schema_version: String::from(CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION),
        source_id: String::from("google_l4_validator_node"),
        source_class: CrossProviderComputeSourceClass::GoogleCloud,
        provider: CrossProviderComputeProviderKind::GoogleCloud,
        locality: CrossProviderComputeSourceLocality {
            locality_kind: CrossProviderLocalityKind::RegionalPrivateCloudNode,
            region: Some(contract.region_family.clone()),
            zone_or_placement: Some(coordinator.preferred_zone.clone()),
            cluster_namespace: Some(contract.cluster_namespace.clone()),
            discovery_posture: contract.discovery_posture.clone(),
            detail: format!(
                "Google g2 + L4 coordinator node in project `{}` on private subnetwork `{}`.",
                contract.project_id, coordinator.subnetwork
            ),
        },
        accelerators: CrossProviderComputeSourceAcceleratorInventory {
            accelerator_vendor: String::from("nvidia"),
            accelerator_model: json_string(
                profile,
                "accelerator_type",
                PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
            )?,
            accelerator_count: json_u64(
                profile,
                "accelerator_count",
                PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
            )? as u16,
            per_accelerator_memory_bytes: None,
            unified_memory: false,
            require_non_mig: false,
            detail: format!(
                "Machine type `{}` with one retained L4 accelerator.",
                json_string(
                    profile,
                    "machine_type",
                    PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
                )?
            ),
        },
        backend: CrossProviderComputeSourceBackendPosture {
            backend_family: CrossProviderBackendFamily::Cuda,
            runtime_backend_label: coordinator.backend_label.clone(),
            precision_posture: String::from("f32_reference"),
            admits_mixed_backend_dense: false,
            detail: String::from(
                "Google swarm nodes currently retain the CUDA open-adapter backend and do not claim mixed-backend dense participation.",
            ),
        },
        network: CrossProviderComputeSourceNetworkPosture {
            trust_tier: CrossProviderTrustTier::PrivateCloudOperatorManaged,
            private_connectivity: !contract.external_ip_permitted,
            cluster_port_bindings: vec![coordinator.cluster_port],
            ingress_posture: format!("configured_peer_only tags={target_tags}"),
            egress_posture: format!(
                "private VPC regional egress in `{}` on `{}`",
                contract.region_family,
                json_string(&network_posture, "network", PSION_GOOGLE_TWO_NODE_SWARM_NETWORK_POSTURE_PATH)?,
            ),
            impairment_posture: String::from("admitted_host_side_netem_profiles"),
            detail: format!(
                "Cluster traffic stays inside dedicated subnetworks with explicit impairment profiles and no external IPs."
            ),
        },
        storage: CrossProviderComputeSourceStoragePosture {
            storage_kind: CrossProviderStorageKind::RemoteBucketPlusLocalDisk,
            workspace_root: Some(String::from("/var/lib/psion")),
            remote_artifact_root: Some(bucket_url.clone()),
            remote_checkpoint_root: Some(format!("{bucket_url}/runs/${{RUN_ID}}/checkpoints")),
            local_disk_budget_bytes: json_u64(
                profile,
                "boot_disk_gb",
                PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
            )
            .ok()
            .map(|gb| gb * 1024 * 1024 * 1024),
            checkpoint_writer_capable: true,
            detail: String::from(
                "Google launch authority keeps shared artifacts in GCS and uses the local boot disk only as node-local staging.",
            ),
        },
        cost: CrossProviderComputeSourceCostPosture {
            cost_model: CrossProviderCostModel::ProviderGuardrailed,
            declared_run_cost_ceiling_usd_cents: json_f64(
                profile,
                "declared_run_cost_ceiling_usd",
                PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
            )
            .ok()
            .map(|usd| dollars_to_cents(usd)),
            declared_hourly_cost_usd_cents: None,
            detail: String::from(
                "The retained Google fixture keeps an explicit per-run ceiling but does not retain a canonical hourly price quote.",
            ),
        },
        admitted_execution_classes: vec![
            CrossProviderExecutionClass::ValidatedContributorWindow,
            CrossProviderExecutionClass::Validator,
            CrossProviderExecutionClass::CheckpointWriter,
            CrossProviderExecutionClass::EvalWorker,
            CrossProviderExecutionClass::DataBuilder,
        ],
        refusal_examples: vec![],
        authority_artifacts: vec![
            authority_artifact(
                PSION_GOOGLE_TWO_NODE_SWARM_CONTRACT_FIXTURE_PATH,
                "Frozen Google two-node swarm contract that owns the node role, cluster namespace, bucket root, and cluster ports.",
            )?,
            authority_artifact(
                PSION_GOOGLE_TWO_NODE_SWARM_LAUNCH_PROFILES_PATH,
                "Google launch profile that retains the g2-standard-8 plus L4 machine facts and run-cost ceiling.",
            )?,
            authority_artifact(
                PSION_GOOGLE_TWO_NODE_SWARM_NETWORK_POSTURE_PATH,
                "Google network posture that retains the dedicated subnetworks and regional private-network assumptions.",
            )?,
            authority_artifact(
                PSION_GOOGLE_TWO_NODE_SWARM_IDENTITY_PROFILE_PATH,
                "Google identity profile that retains the target tags and service-account authority behind the node.",
            )?,
        ],
        claim_boundary: String::from(
            "This source contract proves one private Google g2 plus L4 node can be compared and admitted through the shared training-facing machine contract. It does not claim provider API automation closure, dense-rank runtime closure, or mixed-backend dense participation by itself.",
        ),
        contract_digest: String::new(),
    };
    source.refusal_examples =
        unsupported_execution_refusals(&source, &[CrossProviderExecutionClass::DenseFullModelRank]);
    source.contract_digest = source.stable_digest();
    Ok(source)
}

fn runpod_8xh100_dense_compute_source_contract(
) -> Result<CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError> {
    let launch_profiles = read_json_value(CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH)?;
    let cost_guardrails = read_json_value(CROSS_PROVIDER_RUNPOD_COST_GUARDRAILS_PATH)?;
    let operator_policy = read_json_value(CROSS_PROVIDER_RUNPOD_OPERATOR_PREFLIGHT_PATH)?;
    let profile = launch_profiles
        .get("profiles")
        .and_then(Value::as_array)
        .and_then(|profiles| profiles.first())
        .ok_or_else(|| CrossProviderComputeSourceContractError::MissingField {
            path: CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH.to_string(),
            field: String::from("profiles[0]"),
        })?;
    let max_runtime_hours = json_f64(
        &cost_guardrails,
        "max_runtime_hours",
        CROSS_PROVIDER_RUNPOD_COST_GUARDRAILS_PATH,
    )?;
    let declared_run_cost_ceiling_usd = json_f64(
        &cost_guardrails,
        "declared_run_cost_ceiling_usd",
        CROSS_PROVIDER_RUNPOD_COST_GUARDRAILS_PATH,
    )?;
    let hourly_cents = if max_runtime_hours > 0.0 {
        Some((declared_run_cost_ceiling_usd / max_runtime_hours * 100.0).round() as u32)
    } else {
        None
    };
    let mut source = CrossProviderComputeSourceContract {
        schema_version: String::from(CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION),
        source_id: String::from("runpod_8xh100_dense_node"),
        source_class: CrossProviderComputeSourceClass::RunPod,
        provider: CrossProviderComputeProviderKind::RunPod,
        locality: CrossProviderComputeSourceLocality {
            locality_kind: CrossProviderLocalityKind::SingleRentedPod,
            region: None,
            zone_or_placement: Some(json_string(
                profile,
                "pod_shape",
                CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH,
            )?),
            cluster_namespace: None,
            discovery_posture: String::from("single_pod_world_size_fixed"),
            detail: String::from(
                "One RunPod pod with exactly eight non-MIG H100 devices under the retained Parameter Golf operator lane.",
            ),
        },
        accelerators: CrossProviderComputeSourceAcceleratorInventory {
            accelerator_vendor: String::from("nvidia"),
            accelerator_model: json_string(
                profile,
                "accelerator_type",
                CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH,
            )?,
            accelerator_count: json_u64(
                profile,
                "accelerator_count",
                CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH,
            )? as u16,
            per_accelerator_memory_bytes: Some(80 * 1024 * 1024 * 1024),
            unified_memory: false,
            require_non_mig: json_bool(
                profile,
                "require_non_mig",
                CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH,
            )?,
            detail: format!(
                "World size {} on one pod with explicit H100-only inventory.",
                json_u64(profile, "world_size", CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH)?,
            ),
        },
        backend: CrossProviderComputeSourceBackendPosture {
            backend_family: CrossProviderBackendFamily::Cuda,
            runtime_backend_label: String::from("cuda"),
            precision_posture: String::from("provider_runtime_defined"),
            admits_mixed_backend_dense: false,
            detail: String::from(
                "The retained RunPod lane is CUDA-only and is the strongest current dense-rank candidate surface in the repo.",
            ),
        },
        network: CrossProviderComputeSourceNetworkPosture {
            trust_tier: CrossProviderTrustTier::RentedProviderOperatorManaged,
            private_connectivity: false,
            cluster_port_bindings: Vec::new(),
            ingress_posture: String::from("single_pod_local_process_mesh"),
            egress_posture: String::from("provider_egress_with_operator_ssh"),
            impairment_posture: String::from("not_retained"),
            detail: String::from(
                "The current RunPod lane assumes one pod-local process mesh rather than a cross-host private cluster fabric.",
            ),
        },
        storage: CrossProviderComputeSourceStoragePosture {
            storage_kind: CrossProviderStorageKind::PersistentProviderWorkspace,
            workspace_root: Some(json_string(
                profile,
                "workspace_mount_posture",
                CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH,
            )?),
            remote_artifact_root: None,
            remote_checkpoint_root: None,
            local_disk_budget_bytes: None,
            checkpoint_writer_capable: true,
            detail: String::from(
                "The retained RunPod lane writes into the mounted workspace and finalizes artifacts from there.",
            ),
        },
        cost: CrossProviderComputeSourceCostPosture {
            cost_model: CrossProviderCostModel::ProviderGuardrailed,
            declared_run_cost_ceiling_usd_cents: Some(dollars_to_cents(declared_run_cost_ceiling_usd)),
            declared_hourly_cost_usd_cents: hourly_cents,
            detail: format!(
                "The retained RunPod guardrail caps the run at ${declared_run_cost_ceiling_usd:.2} across {max_runtime_hours:.1} hours."
            ),
        },
        admitted_execution_classes: vec![
            CrossProviderExecutionClass::DenseFullModelRank,
            CrossProviderExecutionClass::CheckpointWriter,
            CrossProviderExecutionClass::EvalWorker,
            CrossProviderExecutionClass::DataBuilder,
        ],
        refusal_examples: vec![],
        authority_artifacts: vec![
            authority_artifact(
                CROSS_PROVIDER_RUNPOD_LAUNCH_PROFILES_PATH,
                "RunPod launch profile that retains the exact 8xH100 pod shape and runtime envelope.",
            )?,
            authority_artifact(
                CROSS_PROVIDER_RUNPOD_COST_GUARDRAILS_PATH,
                "RunPod cost guardrails that retain the bounded cost ceiling for the dense lane.",
            )?,
            authority_artifact(
                CROSS_PROVIDER_RUNPOD_OPERATOR_PREFLIGHT_PATH,
                "RunPod operator preflight policy that retains the required commands and local artifacts.",
            )?,
        ],
        claim_boundary: String::from(
            "This source contract proves one 8xH100 RunPod pod can be admitted as a dense CUDA source through the shared training-facing machine contract. It does not claim cross-host RunPod cluster closure, mixed-backend dense participation, or provider-neutral finalization by itself.",
        ),
        contract_digest: String::new(),
    };
    let required_commands = operator_policy
        .get("required_commands")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    source.network.detail = format!(
        "{} The retained operator preflight keeps {} required remote commands explicit.",
        source.network.detail, required_commands
    );
    source.refusal_examples = unsupported_execution_refusals(
        &source,
        &[
            CrossProviderExecutionClass::ValidatedContributorWindow,
            CrossProviderExecutionClass::Validator,
        ],
    );
    source.contract_digest = source.stable_digest();
    Ok(source)
}

fn local_rtx4080_compute_source_contract(
) -> Result<CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError> {
    let report: FirstSwarmLinuxCudaBringupReport =
        read_json_file(SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH)?;
    let device = report.observed_cuda_devices.first().ok_or_else(|| {
        CrossProviderComputeSourceContractError::InvalidContract {
            detail: String::from("Linux RTX 4080 bring-up report retained no CUDA device"),
        }
    })?;
    let accelerator_model = device.device_name.clone().ok_or_else(|| {
        CrossProviderComputeSourceContractError::InvalidContract {
            detail: String::from("Linux RTX 4080 bring-up report retained no CUDA device_name"),
        }
    })?;
    let mut source = CrossProviderComputeSourceContract {
        schema_version: String::from(CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION),
        source_id: String::from("local_rtx4080_workstation"),
        source_class: CrossProviderComputeSourceClass::LocalWorkstation,
        provider: CrossProviderComputeProviderKind::LocalOperatorManaged,
        locality: CrossProviderComputeSourceLocality {
            locality_kind: CrossProviderLocalityKind::SingleLocalWorkstation,
            region: None,
            zone_or_placement: Some(String::from("trusted_lan_or_local_host")),
            cluster_namespace: None,
            discovery_posture: String::from("operator_direct_host_selection"),
            detail: String::from(
                "One operator-managed Linux workstation retained through the first swarm RTX 4080 bring-up report.",
            ),
        },
        accelerators: CrossProviderComputeSourceAcceleratorInventory {
            accelerator_vendor: String::from("nvidia"),
            accelerator_model,
            accelerator_count: report.matching_rtx4080_device_count as u16,
            per_accelerator_memory_bytes: device.memory_capacity_bytes,
            unified_memory: device.unified_memory.unwrap_or(false),
            require_non_mig: false,
            detail: String::from(
                "The retained workstation report proves one RTX 4080 CUDA device with explicit memory and topology facts.",
            ),
        },
        backend: CrossProviderComputeSourceBackendPosture {
            backend_family: CrossProviderBackendFamily::Cuda,
            runtime_backend_label: String::from(OPEN_ADAPTER_CUDA_BACKEND_LABEL),
            precision_posture: report.machine_thresholds.precision_policy.clone(),
            admits_mixed_backend_dense: false,
            detail: String::from(
                "The local RTX source currently closes validated contributor work, eval work, and data building under the CUDA open-adapter backend, not dense distributed training.",
            ),
        },
        network: CrossProviderComputeSourceNetworkPosture {
            trust_tier: CrossProviderTrustTier::LocalOperatorManaged,
            private_connectivity: true,
            cluster_port_bindings: Vec::new(),
            ingress_posture: String::from("operator_owned_local_network"),
            egress_posture: String::from("operator_owned_local_network"),
            impairment_posture: String::from("not_retained"),
            detail: String::from(
                "The retained workstation bring-up report does not freeze cluster ports or remote artifact transport, so those claims remain refused until a later binder or lane supplies them.",
            ),
        },
        storage: CrossProviderComputeSourceStoragePosture {
            storage_kind: CrossProviderStorageKind::LocalFilesystemOnly,
            workspace_root: Some(String::from("operator_local_workspace")),
            remote_artifact_root: None,
            remote_checkpoint_root: None,
            local_disk_budget_bytes: None,
            checkpoint_writer_capable: false,
            detail: String::from(
                "The retained workstation source has a truthful local filesystem posture only and does not yet claim shared checkpoint authority.",
            ),
        },
        cost: CrossProviderComputeSourceCostPosture {
            cost_model: CrossProviderCostModel::LocalOperatorOwned,
            declared_run_cost_ceiling_usd_cents: None,
            declared_hourly_cost_usd_cents: None,
            detail: String::from(
                "The retained local workstation source does not publish a provider-metered price surface.",
            ),
        },
        admitted_execution_classes: vec![
            CrossProviderExecutionClass::ValidatedContributorWindow,
            CrossProviderExecutionClass::EvalWorker,
            CrossProviderExecutionClass::DataBuilder,
        ],
        refusal_examples: vec![],
        authority_artifacts: vec![authority_artifact(
            SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH,
            "Local RTX 4080 bring-up report that retains the source inventory, CUDA health, and bounded parity harness.",
        )?],
        claim_boundary: String::from(
            "This source contract proves one local RTX 4080 workstation can be compared and admitted through the shared training-facing machine contract. It does not claim dense-rank runtime closure, shared checkpoint authority, or provider-neutral remote launch by itself.",
        ),
        contract_digest: String::new(),
    };
    source.refusal_examples = unsupported_execution_refusals(
        &source,
        &[
            CrossProviderExecutionClass::DenseFullModelRank,
            CrossProviderExecutionClass::Validator,
            CrossProviderExecutionClass::CheckpointWriter,
        ],
    );
    source.contract_digest = source.stable_digest();
    Ok(source)
}

fn local_mlx_mac_compute_source_contract(
) -> Result<CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError> {
    let report: FirstSwarmMacMlxBringupReport = read_json_file(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH)?;
    let mut source = CrossProviderComputeSourceContract {
        schema_version: String::from(CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION),
        source_id: String::from("local_mlx_mac_workstation"),
        source_class: CrossProviderComputeSourceClass::LocalWorkstation,
        provider: CrossProviderComputeProviderKind::LocalOperatorManaged,
        locality: CrossProviderComputeSourceLocality {
            locality_kind: CrossProviderLocalityKind::SingleLocalWorkstation,
            region: None,
            zone_or_placement: Some(report.host.hostname.clone()),
            cluster_namespace: None,
            discovery_posture: String::from("operator_direct_host_selection"),
            detail: format!(
                "One operator-managed Apple Silicon workstation retained through the MLX bring-up report on `{}`.",
                report.host.hardware_model
            ),
        },
        accelerators: CrossProviderComputeSourceAcceleratorInventory {
            accelerator_vendor: String::from("apple"),
            accelerator_model: report.host.chip_name.clone(),
            accelerator_count: 1,
            per_accelerator_memory_bytes: Some(report.host.unified_memory_bytes),
            unified_memory: true,
            require_non_mig: false,
            detail: format!(
                "The retained MLX bring-up report proves one Apple `{}` chip with {} GPU cores and {} bytes of unified memory.",
                report.host.chip_name,
                report.host.gpu_core_count.unwrap_or_default(),
                report.host.unified_memory_bytes
            ),
        },
        backend: CrossProviderComputeSourceBackendPosture {
            backend_family: CrossProviderBackendFamily::MlxMetal,
            runtime_backend_label: String::from(OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL),
            precision_posture: report.machine_thresholds.precision_policy.clone(),
            admits_mixed_backend_dense: false,
            detail: String::from(
                "The local Apple source now closes one bounded single-rank MLX dense runtime plus validator-adjacent work, but not same-job mixed-backend dense training.",
            ),
        },
        network: CrossProviderComputeSourceNetworkPosture {
            trust_tier: CrossProviderTrustTier::LocalOperatorManaged,
            private_connectivity: true,
            cluster_port_bindings: Vec::new(),
            ingress_posture: String::from("operator_owned_local_network"),
            egress_posture: String::from("operator_owned_local_network"),
            impairment_posture: String::from("not_retained"),
            detail: String::from(
                "The retained MLX bring-up report proves the machine contract and bounded backend slice, but it does not yet freeze remote launch or cluster-port authority.",
            ),
        },
        storage: CrossProviderComputeSourceStoragePosture {
            storage_kind: CrossProviderStorageKind::LocalFilesystemOnly,
            workspace_root: Some(String::from("operator_local_workspace")),
            remote_artifact_root: None,
            remote_checkpoint_root: None,
            local_disk_budget_bytes: None,
            checkpoint_writer_capable: false,
            detail: String::from(
                "The retained Apple source currently keeps local-only artifact truth and does not yet claim shared checkpoint writer authority.",
            ),
        },
        cost: CrossProviderComputeSourceCostPosture {
            cost_model: CrossProviderCostModel::LocalOperatorOwned,
            declared_run_cost_ceiling_usd_cents: None,
            declared_hourly_cost_usd_cents: None,
            detail: String::from(
                "The retained Apple source does not publish a provider-metered price surface.",
            ),
        },
        admitted_execution_classes: vec![
            CrossProviderExecutionClass::DenseFullModelRank,
            CrossProviderExecutionClass::ValidatedContributorWindow,
            CrossProviderExecutionClass::Validator,
            CrossProviderExecutionClass::EvalWorker,
            CrossProviderExecutionClass::DataBuilder,
        ],
        refusal_examples: vec![],
        authority_artifacts: vec![authority_artifact(
            SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
            "Local Mac MLX bring-up report that retains the machine thresholds, Metal probe, and bounded overfit gate.",
        )?],
        claim_boundary: String::from(
            "This source contract proves one local Apple Silicon workstation can be compared and admitted through the shared training-facing machine contract, including one bounded single-rank MLX dense runtime. It does not claim mixed-backend same-job training or shared checkpoint authority by itself.",
        ),
        contract_digest: String::new(),
    };
    source.refusal_examples =
        unsupported_execution_refusals(&source, &[CrossProviderExecutionClass::CheckpointWriter]);
    source.contract_digest = source.stable_digest();
    Ok(source)
}

fn planner_candidate<'a>(
    contracts_by_id: &'a BTreeMap<&'a str, &'a CrossProviderComputeSourceContract>,
    manifest: &CrossProviderTrainingProgramManifest,
    source_id: &str,
    requested_execution_class: CrossProviderExecutionClass,
) -> Result<CrossProviderPlannerCandidateInput, CrossProviderComputeSourceContractError> {
    let source = contracts_by_id.get(source_id).ok_or_else(|| {
        CrossProviderComputeSourceContractError::InvalidPlannerInput {
            detail: format!("missing source `{source_id}` while building planner input"),
        }
    })?;
    match source.admit_execution_class(manifest, requested_execution_class) {
        Ok(()) => Ok(CrossProviderPlannerCandidateInput {
            source_id: source_id.to_string(),
            source_contract_digest: source.contract_digest.clone(),
            requested_execution_class,
            expected_disposition: CrossProviderPlannerCandidateDisposition::Admitted,
            expected_refusal: None,
        }),
        Err(refusal) => Ok(CrossProviderPlannerCandidateInput {
            source_id: source_id.to_string(),
            source_contract_digest: source.contract_digest.clone(),
            requested_execution_class,
            expected_disposition: CrossProviderPlannerCandidateDisposition::Refused,
            expected_refusal: Some(refusal),
        }),
    }
}

fn build_launch_input<'a>(
    manifest: &CrossProviderTrainingProgramManifest,
    contracts_by_id: &'a BTreeMap<&'a str, &'a CrossProviderComputeSourceContract>,
    source_id: &str,
    requested_execution_class: CrossProviderExecutionClass,
    run_id: &str,
) -> Result<CrossProviderLaunchInput, CrossProviderComputeSourceContractError> {
    let source = contracts_by_id.get(source_id).ok_or_else(|| {
        CrossProviderComputeSourceContractError::InvalidLaunchInputs {
            detail: format!("missing source `{source_id}` while building launch inputs"),
        }
    })?;
    source
        .admit_execution_class(manifest, requested_execution_class)
        .map_err(
            |refusal| CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                detail: format!(
                    "launch input requested `{:?}` for `{}` but the source refused: {}",
                    requested_execution_class, source_id, refusal.detail
                ),
            },
        )?;
    let mut launch_input = CrossProviderLaunchInput {
        schema_version: String::from(CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        source_id: source_id.to_string(),
        source_contract_digest: source.contract_digest.clone(),
        requested_execution_class,
        run_id: run_id.to_string(),
        environment: manifest.environment.clone(),
        launch_root: render_root(
            manifest.artifact_roots.launch_root_template.as_str(),
            run_id,
        ),
        checkpoint_root: render_root(
            manifest.artifact_roots.checkpoint_root_template.as_str(),
            run_id,
        ),
        metrics_root: render_root(
            manifest.artifact_roots.metrics_root_template.as_str(),
            run_id,
        ),
        visualization_root: render_root(
            manifest.artifact_roots.visualization_root_template.as_str(),
            run_id,
        ),
        final_root: render_root(manifest.artifact_roots.final_root_template.as_str(), run_id),
        cluster_port_binding: cluster_port_binding_for_class(source, requested_execution_class),
        launch_input_digest: String::new(),
    };
    launch_input.launch_input_digest = launch_input.stable_digest();
    Ok(launch_input)
}

fn validate_planner_input(
    planner_input: &CrossProviderPlannerInput,
    manifest: &CrossProviderTrainingProgramManifest,
    contracts: &[CrossProviderComputeSourceContract],
) -> Result<(), CrossProviderComputeSourceContractError> {
    if planner_input.schema_version != CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION {
        return Err(
            CrossProviderComputeSourceContractError::InvalidPlannerInput {
                detail: String::from("planner_input.schema_version drifted"),
            },
        );
    }
    if planner_input.program_manifest_id != manifest.program_manifest_id {
        return Err(
            CrossProviderComputeSourceContractError::InvalidPlannerInput {
                detail: String::from("planner_input.program_manifest_id drifted"),
            },
        );
    }
    if planner_input.program_manifest_digest != manifest.program_manifest_digest {
        return Err(
            CrossProviderComputeSourceContractError::InvalidPlannerInput {
                detail: String::from("planner_input.program_manifest_digest drifted"),
            },
        );
    }
    if planner_input.planner_input_digest != planner_input.stable_digest() {
        return Err(
            CrossProviderComputeSourceContractError::InvalidPlannerInput {
                detail: String::from("planner_input_digest does not match the stable digest"),
            },
        );
    }
    let by_id = contracts_by_id(contracts);
    for candidate in &planner_input.candidates {
        let source = by_id.get(candidate.source_id.as_str()).ok_or_else(|| {
            CrossProviderComputeSourceContractError::InvalidPlannerInput {
                detail: format!(
                    "planner candidate references unknown source `{}`",
                    candidate.source_id
                ),
            }
        })?;
        if candidate.source_contract_digest != source.contract_digest {
            return Err(
                CrossProviderComputeSourceContractError::InvalidPlannerInput {
                    detail: format!(
                        "planner candidate `{}` drifted source_contract_digest",
                        candidate.source_id
                    ),
                },
            );
        }
        match source.admit_execution_class(manifest, candidate.requested_execution_class) {
            Ok(()) => {
                if candidate.expected_disposition
                    != CrossProviderPlannerCandidateDisposition::Admitted
                    || candidate.expected_refusal.is_some()
                {
                    return Err(
                        CrossProviderComputeSourceContractError::InvalidPlannerInput {
                            detail: format!(
                                "planner candidate `{}` should be admitted for `{:?}`",
                                candidate.source_id, candidate.requested_execution_class
                            ),
                        },
                    );
                }
            }
            Err(refusal) => {
                if candidate.expected_disposition
                    != CrossProviderPlannerCandidateDisposition::Refused
                    || candidate.expected_refusal.as_ref() != Some(&refusal)
                {
                    return Err(
                        CrossProviderComputeSourceContractError::InvalidPlannerInput {
                            detail: format!(
                                "planner candidate `{}` should carry the exact refusal for `{:?}`",
                                candidate.source_id, candidate.requested_execution_class
                            ),
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_launch_inputs(
    launch_inputs: &[CrossProviderLaunchInput],
    manifest: &CrossProviderTrainingProgramManifest,
    contracts: &[CrossProviderComputeSourceContract],
) -> Result<(), CrossProviderComputeSourceContractError> {
    let by_id = contracts_by_id(contracts);
    for launch_input in launch_inputs {
        if launch_input.schema_version != CROSS_PROVIDER_COMPUTE_SOURCE_CONTRACT_SCHEMA_VERSION {
            return Err(
                CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                    detail: format!(
                        "launch input for `{}` drifted schema_version",
                        launch_input.source_id
                    ),
                },
            );
        }
        if launch_input.program_manifest_id != manifest.program_manifest_id
            || launch_input.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                detail: format!(
                    "launch input for `{}` drifted away from the canonical program manifest binding",
                    launch_input.source_id
                ),
            });
        }
        if launch_input.launch_input_digest != launch_input.stable_digest() {
            return Err(
                CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                    detail: format!(
                        "launch input for `{}` drifted launch_input_digest",
                        launch_input.source_id
                    ),
                },
            );
        }
        let source = by_id.get(launch_input.source_id.as_str()).ok_or_else(|| {
            CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                detail: format!(
                    "launch input references unknown source `{}`",
                    launch_input.source_id
                ),
            }
        })?;
        if launch_input.source_contract_digest != source.contract_digest {
            return Err(
                CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                    detail: format!(
                        "launch input for `{}` drifted source_contract_digest",
                        launch_input.source_id
                    ),
                },
            );
        }
        source
            .admit_execution_class(manifest, launch_input.requested_execution_class)
            .map_err(
                |refusal| CrossProviderComputeSourceContractError::InvalidLaunchInputs {
                    detail: format!(
                        "launch input for `{}` requested a refused execution class: {}",
                        launch_input.source_id, refusal.detail
                    ),
                },
            )?;
    }
    Ok(())
}

fn contracts_by_id<'a>(
    contracts: &'a [CrossProviderComputeSourceContract],
) -> BTreeMap<&'a str, &'a CrossProviderComputeSourceContract> {
    contracts
        .iter()
        .map(|contract| (contract.source_id.as_str(), contract))
        .collect()
}

fn canonical_program_manifest(
) -> Result<CrossProviderTrainingProgramManifest, CrossProviderComputeSourceContractError> {
    let manifest = cross_provider_training_program_manifest().map_err(|error| {
        CrossProviderComputeSourceContractError::InvalidContract {
            detail: format!("failed to load canonical program manifest: {error}"),
        }
    })?;
    manifest.validate().map_err(|error| {
        CrossProviderComputeSourceContractError::InvalidContract {
            detail: format!("canonical program manifest drifted: {error}"),
        }
    })?;
    Ok(manifest)
}

fn require_exactly_one_refusal_per_unsupported_class(
    contract: &CrossProviderComputeSourceContract,
    admitted_program_classes: &[CrossProviderExecutionClass],
) -> Result<(), CrossProviderComputeSourceContractError> {
    let admitted: BTreeSet<_> = contract
        .admitted_execution_classes
        .iter()
        .copied()
        .collect();
    let refusals: BTreeMap<_, _> = contract
        .refusal_examples
        .iter()
        .map(|refusal| (refusal.requested_execution_class, refusal))
        .collect();
    for execution_class in admitted_program_classes {
        if admitted.contains(execution_class) {
            if refusals.contains_key(execution_class) {
                return Err(CrossProviderComputeSourceContractError::InvalidContract {
                    detail: format!(
                        "source `{}` cannot both admit and refuse `{:?}`",
                        contract.source_id, execution_class
                    ),
                });
            }
            continue;
        }
        let refusal = refusals.get(execution_class).ok_or_else(|| {
            CrossProviderComputeSourceContractError::InvalidContract {
                detail: format!(
                    "source `{}` must carry one typed refusal for unsupported execution class `{:?}`",
                    contract.source_id, execution_class
                ),
            }
        })?;
        if refusal.refusal_kind
            != CrossProviderExecutionClassAdmissionRefusalKind::SourceExecutionClassNotAdmitted
        {
            return Err(CrossProviderComputeSourceContractError::InvalidContract {
                detail: format!(
                    "source `{}` must use source_execution_class_not_admitted for unsupported execution class `{:?}`",
                    contract.source_id, execution_class
                ),
            });
        }
    }
    Ok(())
}

fn unsupported_execution_refusals(
    source: &CrossProviderComputeSourceContract,
    unsupported_classes: &[CrossProviderExecutionClass],
) -> Vec<CrossProviderExecutionClassAdmissionRefusal> {
    unsupported_classes
        .iter()
        .copied()
        .map(|requested_execution_class| CrossProviderExecutionClassAdmissionRefusal {
            source_id: source.source_id.clone(),
            requested_execution_class,
            refusal_kind: CrossProviderExecutionClassAdmissionRefusalKind::SourceExecutionClassNotAdmitted,
            detail: format!(
                "source `{}` does not yet admit `{:?}` under backend `{:?}` and the current claim boundary refuses widening beyond its retained lane evidence",
                source.source_id, requested_execution_class, source.backend.backend_family
            ),
        })
        .collect()
}

fn cluster_port_binding_for_class(
    source: &CrossProviderComputeSourceContract,
    requested_execution_class: CrossProviderExecutionClass,
) -> Option<u16> {
    match requested_execution_class {
        CrossProviderExecutionClass::DenseFullModelRank
        | CrossProviderExecutionClass::ValidatedContributorWindow
        | CrossProviderExecutionClass::Validator => {
            source.network.cluster_port_bindings.first().copied()
        }
        CrossProviderExecutionClass::CheckpointWriter
        | CrossProviderExecutionClass::EvalWorker
        | CrossProviderExecutionClass::DataBuilder => None,
    }
}

fn render_root(template: &str, run_id: &str) -> String {
    template.replace("${RUN_ID}", run_id)
}

fn canonical_fixture_targets(
    output_dir: &Path,
) -> Result<Vec<PathBuf>, CrossProviderComputeSourceContractError> {
    let mut targets = vec![
        output_dir.join("google_l4_validator_node_v1.json"),
        output_dir.join("runpod_8xh100_dense_node_v1.json"),
        output_dir.join("local_rtx4080_workstation_v1.json"),
        output_dir.join("local_mlx_mac_workstation_v1.json"),
    ];
    if targets.len() != 4 {
        return Err(CrossProviderComputeSourceContractError::InvalidContract {
            detail: String::from("expected exactly four canonical fixture targets"),
        });
    }
    Ok(std::mem::take(&mut targets))
}

fn authority_artifact(
    path: &str,
    detail: &str,
) -> Result<CrossProviderComputeSourceAuthorityArtifact, CrossProviderComputeSourceContractError> {
    Ok(CrossProviderComputeSourceAuthorityArtifact {
        path: path.to_string(),
        sha256: sha256_file(path)?,
        detail: detail.to_string(),
    })
}

fn read_json_file<T: serde::de::DeserializeOwned>(
    path: &str,
) -> Result<T, CrossProviderComputeSourceContractError> {
    let resolved = resolve_repo_path(path);
    let bytes =
        fs::read(&resolved).map_err(|error| CrossProviderComputeSourceContractError::Read {
            path: path.to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CrossProviderComputeSourceContractError::Deserialize {
            path: path.to_string(),
            error,
        }
    })
}

fn read_json_value(path: &str) -> Result<Value, CrossProviderComputeSourceContractError> {
    read_json_file(path)
}

fn write_json(
    path: impl AsRef<Path>,
    value: &impl Serialize,
) -> Result<(), CrossProviderComputeSourceContractError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CrossProviderComputeSourceContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|error| CrossProviderComputeSourceContractError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn sha256_file(path: &str) -> Result<String, CrossProviderComputeSourceContractError> {
    let resolved = resolve_repo_path(path);
    let bytes =
        fs::read(&resolved).map_err(|error| CrossProviderComputeSourceContractError::Read {
            path: path.to_string(),
            error,
        })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("cross-provider compute source contract stable serialization"),
    );
    hex::encode(hasher.finalize())
}

fn json_string(
    value: &Value,
    field: &str,
    path: &str,
) -> Result<String, CrossProviderComputeSourceContractError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CrossProviderComputeSourceContractError::MissingField {
            path: path.to_string(),
            field: field.to_string(),
        })
}

fn json_u64(
    value: &Value,
    field: &str,
    path: &str,
) -> Result<u64, CrossProviderComputeSourceContractError> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        CrossProviderComputeSourceContractError::MissingField {
            path: path.to_string(),
            field: field.to_string(),
        }
    })
}

fn json_bool(
    value: &Value,
    field: &str,
    path: &str,
) -> Result<bool, CrossProviderComputeSourceContractError> {
    value.get(field).and_then(Value::as_bool).ok_or_else(|| {
        CrossProviderComputeSourceContractError::MissingField {
            path: path.to_string(),
            field: field.to_string(),
        }
    })
}

fn json_f64(
    value: &Value,
    field: &str,
    path: &str,
) -> Result<f64, CrossProviderComputeSourceContractError> {
    value.get(field).and_then(Value::as_f64).ok_or_else(|| {
        CrossProviderComputeSourceContractError::MissingField {
            path: path.to_string(),
            field: field.to_string(),
        }
    })
}

fn dollars_to_cents(dollars: f64) -> u32 {
    (dollars * 100.0).round() as u32
}

fn resolve_repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_provider_compute_source_contracts_stay_valid() {
        let manifest = cross_provider_training_program_manifest().expect("manifest");
        let contracts = canonical_cross_provider_compute_source_contracts().expect("contracts");
        assert_eq!(contracts.len(), 4);
        for contract in &contracts {
            contract
                .validate(&manifest)
                .expect("contract should validate");
        }
    }

    #[test]
    fn cross_provider_planner_input_tracks_admitted_and_refused_roles() {
        let planner = canonical_cross_provider_planner_input().expect("planner");
        // Since commit b8056a78 (MLX dense rank runtime contract), the local
        // MLX Mac workstation admits one bounded single-rank
        // DenseFullModelRank lane, so this candidate is Admitted.
        let admitted = planner
            .candidates
            .iter()
            .find(|candidate| {
                candidate.source_id == "local_mlx_mac_workstation"
                    && candidate.requested_execution_class
                        == CrossProviderExecutionClass::DenseFullModelRank
            })
            .expect("admitted candidate");
        assert_eq!(
            admitted.expected_disposition,
            CrossProviderPlannerCandidateDisposition::Admitted
        );
        assert!(admitted.expected_refusal.is_none());
        // The refused-role tracking survives at the contract boundary: the
        // same MLX source still refuses CheckpointWriter authority.
        let contracts = canonical_cross_provider_compute_source_contracts().expect("contracts");
        let mlx_contract = contracts
            .iter()
            .find(|contract| contract.source_id == "local_mlx_mac_workstation")
            .expect("mlx contract");
        let checkpoint_refusal = mlx_contract
            .refusal_examples
            .iter()
            .find(|refusal| {
                refusal.requested_execution_class == CrossProviderExecutionClass::CheckpointWriter
            })
            .expect("checkpoint writer refusal");
        assert_eq!(
            checkpoint_refusal.refusal_kind,
            CrossProviderExecutionClassAdmissionRefusalKind::SourceExecutionClassNotAdmitted
        );
    }

    #[test]
    fn cross_provider_launch_inputs_bind_run_roots() {
        let launch_inputs = canonical_cross_provider_launch_inputs().expect("launch inputs");
        let runpod = launch_inputs
            .iter()
            .find(|input| input.source_id == "runpod_8xh100_dense_node")
            .expect("runpod launch input");
        assert_eq!(
            runpod.checkpoint_root,
            "runs/psion-xprovider-pretrain-runpod-dense-example/checkpoints"
        );
        assert_eq!(
            runpod.requested_execution_class,
            CrossProviderExecutionClass::DenseFullModelRank
        );
    }
}
