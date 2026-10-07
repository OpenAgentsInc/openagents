use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, cross_provider_training_program_manifest,
    ArtifactArchiveClass, ArtifactRetentionProfile, CrossProviderComputeSourceContract,
    CrossProviderComputeSourceContractError, CrossProviderExecutionClass, CrossProviderStorageKind,
    CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError,
    TrainArtifactClass,
};

/// Stable schema version for the provider-neutral remote artifact backend contract set.
pub const REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.remote_train_artifact_backend_contract.v1";
/// Stable fixture path for the canonical remote artifact backend contract set.
pub const REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/remote_train_artifact_backend_contract_v1.json";
/// Stable checker path for the canonical remote artifact backend contract set.
pub const REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-remote-train-artifact-backend-contract.sh";
/// Stable reference doc path for the canonical remote artifact backend contract set.
pub const REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_DOC_PATH: &str =
    "docs/REMOTE_TRAIN_ARTIFACT_BACKEND_REFERENCE.md";

/// Error surfaced while building, validating, or writing the remote backend contract set.
#[derive(Debug, Error)]
pub enum RemoteTrainArtifactBackendContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ComputeSource(#[from] CrossProviderComputeSourceContractError),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error("remote train artifact backend contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Concrete remote backend kind behind one provider-neutral contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainArtifactBackendKind {
    /// Cloud object store with explicit remote authority.
    CloudObjectStore,
    /// Provider workspace mirror with explicit finalizer projection.
    ProviderWorkspaceMirror,
}

/// Placement mode enforced by one remote backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainArtifactPlacementMode {
    /// Upload directly into the remote authoritative root during the run.
    ImmediateRemoteMirror,
    /// Stage locally first, then mirror through the finalizer.
    WorkspaceThenFinalizerMirror,
}

/// Restore mode enforced by one remote backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainArtifactRestoreMode {
    /// Restore directly from the remote authoritative root.
    DirectRemoteHydrate,
    /// Restore from a finalizer-published remote root into a local stage root.
    FinalizerMirroredHydrate,
}

/// Per-class remote storage policy admitted by one backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainArtifactClassPolicy {
    /// Artifact class admitted by this backend.
    pub artifact_class: TrainArtifactClass,
    /// Retention profile applied once the artifact is remote-authoritative.
    pub retention_profile: ArtifactRetentionProfile,
    /// Provider-neutral authoritative path template.
    pub authoritative_path_template: String,
    /// Provider-neutral local stage path template.
    pub local_stage_path_template: String,
    /// Byte-accounting bucket used for cost posture.
    pub byte_accounting_key: String,
    /// Machine-legible detail.
    pub detail: String,
}

/// One concrete remote backend contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainArtifactBackendContract {
    /// Stable backend id.
    pub backend_id: String,
    /// Source that owns this backend in the current provider set.
    pub source_id: String,
    /// Concrete backend kind.
    pub backend_kind: RemoteTrainArtifactBackendKind,
    /// Provider-neutral root template.
    pub authoritative_root_template: String,
    /// Optional local stage root for mirrored backends.
    pub local_stage_root_template: String,
    /// Placement mode enforced by the backend.
    pub placement_mode: RemoteTrainArtifactPlacementMode,
    /// Restore mode enforced by the backend.
    pub restore_mode: RemoteTrainArtifactRestoreMode,
    /// Per-class admitted policies.
    pub class_policies: Vec<RemoteTrainArtifactClassPolicy>,
    /// Byte-accounted cost posture explanation.
    pub byte_cost_posture: String,
    /// Finalizer projection id that removes bespoke provider-root walking.
    pub finalizer_projection_id: String,
    /// Honest refusal posture for unsupported placement or restore requests.
    pub refusal_posture: String,
    /// Stable backend digest.
    pub contract_digest: String,
}

impl RemoteTrainArtifactBackendContract {
    /// Returns the stable digest for the backend payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_remote_train_artifact_backend_contract|", &clone)
    }
}

/// Provider-neutral placement decision over one artifact class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainArtifactPlacementDecision {
    /// Artifact class governed by this decision.
    pub artifact_class: TrainArtifactClass,
    /// Primary authoritative backend.
    pub primary_backend_id: String,
    /// Mirror backends that must also retain the artifact.
    pub mirror_backend_ids: Vec<String>,
    /// Restore authority backend for the class.
    pub restore_authority_backend_id: String,
    /// Whether byte accounting is mandatory for this class.
    pub byte_accounting_required: bool,
    /// Machine-legible detail.
    pub detail: String,
}

/// Finalizer-facing projection that removes provider-specific root walking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainArtifactFinalizerProjection {
    /// Stable projection id.
    pub projection_id: String,
    /// Source that will use this projection.
    pub source_id: String,
    /// Artifact class governed by the projection.
    pub artifact_class: TrainArtifactClass,
    /// Provider-neutral authoritative prefix.
    pub authoritative_prefix_template: String,
    /// Local stage prefix for provider-side writes.
    pub local_stage_prefix_template: String,
    /// Finalizer-published manifest path.
    pub finalizer_manifest_relpath: String,
    /// Machine-legible detail.
    pub detail: String,
}

/// Canonical provider-neutral remote artifact backend contract set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainArtifactBackendContractSet {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable cross-provider program manifest id.
    pub program_manifest_id: String,
    /// Stable cross-provider program manifest digest.
    pub program_manifest_digest: String,
    /// Concrete remote backends currently admitted.
    pub backends: Vec<RemoteTrainArtifactBackendContract>,
    /// Placement policy across the admitted backends.
    pub placement_decisions: Vec<RemoteTrainArtifactPlacementDecision>,
    /// Finalizer projections over the admitted backends.
    pub finalizer_projections: Vec<RemoteTrainArtifactFinalizerProjection>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl RemoteTrainArtifactBackendContractSet {
    /// Returns the stable digest for the contract-set payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(
            b"psionic_remote_train_artifact_backend_contract_set|",
            &clone,
        )
    }

    /// Validates the contract set against the canonical manifest and compute-source contracts.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        source_contracts: &[CrossProviderComputeSourceContract],
    ) -> Result<(), RemoteTrainArtifactBackendContractError> {
        if self.schema_version != REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_SCHEMA_VERSION {
            return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id {
            return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                detail: String::from("program_manifest_id drifted from the root manifest"),
            });
        }
        if self.program_manifest_digest != manifest.program_manifest_digest {
            return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                detail: String::from("program_manifest_digest drifted from the root manifest"),
            });
        }

        let sources_by_id = source_contracts
            .iter()
            .map(|contract| (contract.source_id.as_str(), contract))
            .collect::<BTreeMap<_, _>>();
        let mut backend_ids = BTreeSet::new();
        let mut projection_ids = BTreeSet::new();
        let mut backend_support =
            BTreeMap::<(&str, TrainArtifactClass), &RemoteTrainArtifactClassPolicy>::new();
        for backend in &self.backends {
            if !backend_ids.insert(backend.backend_id.as_str()) {
                return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                    detail: format!("duplicate backend id `{}`", backend.backend_id),
                });
            }
            let source = sources_by_id
                .get(backend.source_id.as_str())
                .ok_or_else(
                    || RemoteTrainArtifactBackendContractError::InvalidContract {
                        detail: format!("missing compute source `{}`", backend.source_id),
                    },
                )?;
            validate_backend_storage_kind(backend, source)?;
            if backend.class_policies.is_empty() {
                return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                    detail: format!("backend `{}` has no class policies", backend.backend_id),
                });
            }
            for policy in &backend.class_policies {
                validate_class_policy(policy)?;
                if !backend_support
                    .insert((backend.backend_id.as_str(), policy.artifact_class), policy)
                    .is_none()
                {
                    return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                        detail: format!(
                            "backend `{}` repeats class policy for `{}`",
                            backend.backend_id,
                            artifact_class_label(policy.artifact_class)
                        ),
                    });
                }
                if matches!(policy.artifact_class, TrainArtifactClass::Checkpoint) {
                    source
                        .admit_execution_class(
                            manifest,
                            CrossProviderExecutionClass::CheckpointWriter,
                        )
                        .map_err(|refusal| {
                            RemoteTrainArtifactBackendContractError::InvalidContract {
                                detail: format!(
                                    "backend `{}` cannot own checkpoint placement: {}",
                                    backend.backend_id, refusal.detail
                                ),
                            }
                        })?;
                }
            }
            if backend.contract_digest != backend.stable_digest() {
                return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                    detail: format!("backend `{}` digest drifted", backend.backend_id),
                });
            }
            if !projection_ids.insert(backend.finalizer_projection_id.as_str()) {
                return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                    detail: format!(
                        "backend `{}` reused finalizer projection id `{}`",
                        backend.backend_id, backend.finalizer_projection_id
                    ),
                });
            }
        }

        let required_classes = required_remote_artifact_classes();
        let mut covered_classes = BTreeSet::new();
        for decision in &self.placement_decisions {
            covered_classes.insert(decision.artifact_class);
            if decision.detail.trim().is_empty() {
                return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                    detail: format!(
                        "placement decision for `{}` is missing detail",
                        artifact_class_label(decision.artifact_class)
                    ),
                });
            }
            validate_backend_support(
                &backend_support,
                decision.primary_backend_id.as_str(),
                decision.artifact_class,
            )?;
            validate_backend_support(
                &backend_support,
                decision.restore_authority_backend_id.as_str(),
                decision.artifact_class,
            )?;
            for backend_id in &decision.mirror_backend_ids {
                validate_backend_support(
                    &backend_support,
                    backend_id.as_str(),
                    decision.artifact_class,
                )?;
            }
        }
        if covered_classes != required_classes {
            return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                detail: String::from(
                    "placement decisions must cover checkpoint, log, metrics, and final evidence classes",
                ),
            });
        }

        let projection_index = self
            .finalizer_projections
            .iter()
            .map(|projection| {
                (
                    (
                        projection.projection_id.as_str(),
                        projection.artifact_class,
                        projection.source_id.as_str(),
                    ),
                    projection,
                )
            })
            .collect::<BTreeMap<_, _>>();
        for projection in &self.finalizer_projections {
            if projection.authoritative_prefix_template.trim().is_empty()
                || projection.local_stage_prefix_template.trim().is_empty()
                || projection.finalizer_manifest_relpath.trim().is_empty()
                || projection.detail.trim().is_empty()
            {
                return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                    detail: format!(
                        "finalizer projection `{}` is missing one or more required fields",
                        projection.projection_id
                    ),
                });
            }
        }
        for backend in &self.backends {
            for policy in &backend.class_policies {
                projection_index
                    .get(&(
                        backend.finalizer_projection_id.as_str(),
                        policy.artifact_class,
                        backend.source_id.as_str(),
                    ))
                    .ok_or_else(
                        || RemoteTrainArtifactBackendContractError::InvalidContract {
                            detail: format!(
                                "backend `{}` is missing finalizer projection coverage for `{}`",
                                backend.backend_id,
                                artifact_class_label(policy.artifact_class)
                            ),
                        },
                    )?;
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

/// Trait implemented by concrete remote artifact backends.
pub trait RemoteTrainArtifactBackend {
    /// Returns the typed backend contract exposed by this backend.
    fn contract(&self) -> RemoteTrainArtifactBackendContract;
}

/// Google Cloud bucket-backed artifact backend.
#[derive(Clone, Debug, Default)]
pub struct GoogleCloudTrainArtifactBackend;

impl RemoteTrainArtifactBackend for GoogleCloudTrainArtifactBackend {
    fn contract(&self) -> RemoteTrainArtifactBackendContract {
        let mut contract = RemoteTrainArtifactBackendContract {
            backend_id: String::from("google_train_bucket_backend"),
            source_id: String::from("google_l4_validator_node"),
            backend_kind: RemoteTrainArtifactBackendKind::CloudObjectStore,
            authoritative_root_template: String::from(
                "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}",
            ),
            local_stage_root_template: String::from("/var/lib/psion/runs/${RUN_ID}"),
            placement_mode: RemoteTrainArtifactPlacementMode::ImmediateRemoteMirror,
            restore_mode: RemoteTrainArtifactRestoreMode::DirectRemoteHydrate,
            class_policies: canonical_backend_class_policies(
                "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}",
                "/var/lib/psion/runs/${RUN_ID}",
            ),
            byte_cost_posture: String::from(
                "Byte accounting stays explicit through bucket-class metering, restore-egress accounting, and per-class cost buckets.",
            ),
            finalizer_projection_id: String::from("google_remote_projection"),
            refusal_posture: String::from(
                "Requests that bypass the authoritative remote prefix or omit byte-accounting keys are refused instead of falling back to provider-specific path walking.",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest = contract.stable_digest();
        contract
    }
}

/// RunPod workspace-backed artifact backend with explicit finalizer mirroring.
#[derive(Clone, Debug, Default)]
pub struct RunPodWorkspaceTrainArtifactBackend;

impl RemoteTrainArtifactBackend for RunPodWorkspaceTrainArtifactBackend {
    fn contract(&self) -> RemoteTrainArtifactBackendContract {
        let mut contract = RemoteTrainArtifactBackendContract {
            backend_id: String::from("runpod_workspace_backend"),
            source_id: String::from("runpod_8xh100_dense_node"),
            backend_kind: RemoteTrainArtifactBackendKind::ProviderWorkspaceMirror,
            authoritative_root_template: String::from("/workspace/psionic/runs/${RUN_ID}"),
            local_stage_root_template: String::from("/workspace/psionic/runs/${RUN_ID}"),
            placement_mode: RemoteTrainArtifactPlacementMode::WorkspaceThenFinalizerMirror,
            restore_mode: RemoteTrainArtifactRestoreMode::FinalizerMirroredHydrate,
            class_policies: canonical_backend_class_policies(
                "/workspace/psionic/runs/${RUN_ID}",
                "/workspace/psionic/runs/${RUN_ID}",
            ),
            byte_cost_posture: String::from(
                "Byte accounting stays explicit through workspace-capacity budgets, upload batch size ceilings, and finalizer mirror byte totals.",
            ),
            finalizer_projection_id: String::from("runpod_workspace_projection"),
            refusal_posture: String::from(
                "Requests that assume bespoke workspace scans or skip the finalizer mirror manifest are refused instead of silently probing provider-local roots.",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest = contract.stable_digest();
        contract
    }
}

/// Returns the canonical remote artifact backend contract set.
pub fn canonical_remote_train_artifact_backend_contract_set(
) -> Result<RemoteTrainArtifactBackendContractSet, RemoteTrainArtifactBackendContractError> {
    let manifest = cross_provider_training_program_manifest()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let google_backend = GoogleCloudTrainArtifactBackend.contract();
    let runpod_backend = RunPodWorkspaceTrainArtifactBackend.contract();
    let backends = vec![google_backend.clone(), runpod_backend.clone()];
    let placement_decisions = vec![
        placement_decision(
            TrainArtifactClass::Checkpoint,
            google_backend.backend_id.as_str(),
            vec![runpod_backend.backend_id.as_str()],
            google_backend.backend_id.as_str(),
            true,
            "Checkpoints stay authoritative in the bucket backend while the RunPod workspace keeps a mirrored operator-visible copy.",
        ),
        placement_decision(
            TrainArtifactClass::LogBundle,
            google_backend.backend_id.as_str(),
            vec![runpod_backend.backend_id.as_str()],
            google_backend.backend_id.as_str(),
            true,
            "Logs stay bucket-authoritative so finalizers and later providers read one shared prefix instead of provider-local folders.",
        ),
        placement_decision(
            TrainArtifactClass::MetricsBundle,
            google_backend.backend_id.as_str(),
            vec![runpod_backend.backend_id.as_str()],
            google_backend.backend_id.as_str(),
            true,
            "Metrics bundles stay bucket-authoritative so live and final telemetry share one byte-accounted root.",
        ),
        placement_decision(
            TrainArtifactClass::FinalEvidenceBundle,
            google_backend.backend_id.as_str(),
            vec![runpod_backend.backend_id.as_str()],
            google_backend.backend_id.as_str(),
            true,
            "Final evidence stays bucket-authoritative and mirrored into the workspace so finalizers no longer infer roots from provider-specific directory walks.",
        ),
    ];
    let finalizer_projections = vec![
        projection(
            google_backend.finalizer_projection_id.as_str(),
            google_backend.source_id.as_str(),
            TrainArtifactClass::Checkpoint,
            "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}/checkpoints",
            "/var/lib/psion/runs/${RUN_ID}/checkpoints",
        ),
        projection(
            google_backend.finalizer_projection_id.as_str(),
            google_backend.source_id.as_str(),
            TrainArtifactClass::LogBundle,
            "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}/logs",
            "/var/lib/psion/runs/${RUN_ID}/logs",
        ),
        projection(
            google_backend.finalizer_projection_id.as_str(),
            google_backend.source_id.as_str(),
            TrainArtifactClass::MetricsBundle,
            "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}/metrics",
            "/var/lib/psion/runs/${RUN_ID}/metrics",
        ),
        projection(
            google_backend.finalizer_projection_id.as_str(),
            google_backend.source_id.as_str(),
            TrainArtifactClass::FinalEvidenceBundle,
            "gs://openagentsgemini-psion-train-us-central1/runs/${RUN_ID}/evidence",
            "/var/lib/psion/runs/${RUN_ID}/evidence",
        ),
        projection(
            runpod_backend.finalizer_projection_id.as_str(),
            runpod_backend.source_id.as_str(),
            TrainArtifactClass::Checkpoint,
            "runs/${RUN_ID}/checkpoints",
            "/workspace/psionic/runs/${RUN_ID}/checkpoints",
        ),
        projection(
            runpod_backend.finalizer_projection_id.as_str(),
            runpod_backend.source_id.as_str(),
            TrainArtifactClass::LogBundle,
            "runs/${RUN_ID}/logs",
            "/workspace/psionic/runs/${RUN_ID}/logs",
        ),
        projection(
            runpod_backend.finalizer_projection_id.as_str(),
            runpod_backend.source_id.as_str(),
            TrainArtifactClass::MetricsBundle,
            "runs/${RUN_ID}/metrics",
            "/workspace/psionic/runs/${RUN_ID}/metrics",
        ),
        projection(
            runpod_backend.finalizer_projection_id.as_str(),
            runpod_backend.source_id.as_str(),
            TrainArtifactClass::FinalEvidenceBundle,
            "runs/${RUN_ID}/evidence",
            "/workspace/psionic/runs/${RUN_ID}/evidence",
        ),
    ];
    let mut contract_set = RemoteTrainArtifactBackendContractSet {
        schema_version: String::from(REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        backends,
        placement_decisions,
        finalizer_projections,
        claim_boundary: String::from(
            "This contract closes provider-neutral remote artifact backend identity, placement policy, restore policy, and finalizer projection for the current Google and RunPod lanes. It does not yet claim generic multi-cloud storage clients or same-job mixed-backend checkpoint portability.",
        ),
        contract_digest: String::new(),
    };
    contract_set.contract_digest = contract_set.stable_digest();
    contract_set.validate(&manifest, &sources)?;
    Ok(contract_set)
}

/// Writes the canonical remote artifact backend contract set to the requested path.
pub fn write_remote_train_artifact_backend_contract_set(
    output_path: impl AsRef<Path>,
) -> Result<(), RemoteTrainArtifactBackendContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            RemoteTrainArtifactBackendContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract_set = canonical_remote_train_artifact_backend_contract_set()?;
    let bytes = serde_json::to_vec_pretty(&contract_set)?;
    fs::write(output_path, bytes).map_err(|error| {
        RemoteTrainArtifactBackendContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn validate_backend_storage_kind(
    backend: &RemoteTrainArtifactBackendContract,
    source: &CrossProviderComputeSourceContract,
) -> Result<(), RemoteTrainArtifactBackendContractError> {
    let expected_storage_kind = match backend.backend_kind {
        RemoteTrainArtifactBackendKind::CloudObjectStore => {
            CrossProviderStorageKind::RemoteBucketPlusLocalDisk
        }
        RemoteTrainArtifactBackendKind::ProviderWorkspaceMirror => {
            CrossProviderStorageKind::PersistentProviderWorkspace
        }
    };
    if source.storage.storage_kind != expected_storage_kind {
        return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
            detail: format!(
                "backend `{}` expects storage kind `{:?}` but source `{}` exposes `{:?}`",
                backend.backend_id,
                expected_storage_kind,
                backend.source_id,
                source.storage.storage_kind
            ),
        });
    }
    Ok(())
}

fn validate_class_policy(
    policy: &RemoteTrainArtifactClassPolicy,
) -> Result<(), RemoteTrainArtifactBackendContractError> {
    if policy.authoritative_path_template.trim().is_empty()
        || policy.local_stage_path_template.trim().is_empty()
        || policy.byte_accounting_key.trim().is_empty()
        || policy.detail.trim().is_empty()
    {
        return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
            detail: format!(
                "class policy for `{}` is missing one or more required fields",
                artifact_class_label(policy.artifact_class)
            ),
        });
    }
    if policy.retention_profile.warm_retention_ms < policy.retention_profile.hot_retention_ms {
        return Err(RemoteTrainArtifactBackendContractError::InvalidContract {
            detail: format!(
                "class policy for `{}` has invalid retention profile ordering",
                artifact_class_label(policy.artifact_class)
            ),
        });
    }
    Ok(())
}

fn validate_backend_support<'a>(
    backend_support: &BTreeMap<(&'a str, TrainArtifactClass), &'a RemoteTrainArtifactClassPolicy>,
    backend_id: &'a str,
    artifact_class: TrainArtifactClass,
) -> Result<(), RemoteTrainArtifactBackendContractError> {
    backend_support
        .get(&(backend_id, artifact_class))
        .ok_or_else(
            || RemoteTrainArtifactBackendContractError::InvalidContract {
                detail: format!(
                    "backend `{backend_id}` does not admit `{}`",
                    artifact_class_label(artifact_class)
                ),
            },
        )?;
    Ok(())
}

fn required_remote_artifact_classes() -> BTreeSet<TrainArtifactClass> {
    BTreeSet::from([
        TrainArtifactClass::Checkpoint,
        TrainArtifactClass::LogBundle,
        TrainArtifactClass::MetricsBundle,
        TrainArtifactClass::FinalEvidenceBundle,
    ])
}

fn canonical_backend_class_policies(
    authoritative_root: &str,
    local_stage_root: &str,
) -> Vec<RemoteTrainArtifactClassPolicy> {
    vec![
        class_policy(
            TrainArtifactClass::Checkpoint,
            ArtifactRetentionProfile::new(
                30 * 60 * 1_000,
                4 * 60 * 60 * 1_000,
                ArtifactArchiveClass::Restorable,
                15 * 60 * 1_000,
            )
            .with_deduplication(false),
            format!("{authoritative_root}/checkpoints"),
            format!("{local_stage_root}/checkpoints"),
            "checkpoint_bytes",
            "Checkpoint placement stays restorable and byte-accounted because dense restart and later finalizers depend on exact shard closure.",
        ),
        class_policy(
            TrainArtifactClass::LogBundle,
            ArtifactRetentionProfile::new(
                15 * 60 * 1_000,
                24 * 60 * 60 * 1_000,
                ArtifactArchiveClass::Restorable,
                10 * 60 * 1_000,
            )
            .with_deduplication(false),
            format!("{authoritative_root}/logs"),
            format!("{local_stage_root}/logs"),
            "log_bytes",
            "Log placement stays explicit so finalizers read one typed prefix instead of ad hoc provider-local directories.",
        ),
        class_policy(
            TrainArtifactClass::MetricsBundle,
            ArtifactRetentionProfile::new(
                5 * 60 * 1_000,
                24 * 60 * 60 * 1_000,
                ArtifactArchiveClass::Restorable,
                5 * 60 * 1_000,
            )
            .with_deduplication(true),
            format!("{authoritative_root}/metrics"),
            format!("{local_stage_root}/metrics"),
            "metrics_bytes",
            "Metrics bundles are deduplicated by digest but still keep explicit restore and byte-accounting posture.",
        ),
        class_policy(
            TrainArtifactClass::FinalEvidenceBundle,
            ArtifactRetentionProfile::new(
                60 * 60 * 1_000,
                7 * 24 * 60 * 60 * 1_000,
                ArtifactArchiveClass::Immutable,
                30 * 60 * 1_000,
            )
            .with_deduplication(false),
            format!("{authoritative_root}/evidence"),
            format!("{local_stage_root}/evidence"),
            "final_evidence_bytes",
            "Final evidence remains immutable and byte-accounted because accepted run closure should not depend on provider-specific walking after publication.",
        ),
    ]
}

fn class_policy(
    artifact_class: TrainArtifactClass,
    retention_profile: ArtifactRetentionProfile,
    authoritative_path_template: String,
    local_stage_path_template: String,
    byte_accounting_key: &str,
    detail: &str,
) -> RemoteTrainArtifactClassPolicy {
    RemoteTrainArtifactClassPolicy {
        artifact_class,
        retention_profile,
        authoritative_path_template,
        local_stage_path_template,
        byte_accounting_key: String::from(byte_accounting_key),
        detail: String::from(detail),
    }
}

fn placement_decision(
    artifact_class: TrainArtifactClass,
    primary_backend_id: &str,
    mirror_backend_ids: Vec<&str>,
    restore_authority_backend_id: &str,
    byte_accounting_required: bool,
    detail: &str,
) -> RemoteTrainArtifactPlacementDecision {
    RemoteTrainArtifactPlacementDecision {
        artifact_class,
        primary_backend_id: String::from(primary_backend_id),
        mirror_backend_ids: mirror_backend_ids.into_iter().map(String::from).collect(),
        restore_authority_backend_id: String::from(restore_authority_backend_id),
        byte_accounting_required,
        detail: String::from(detail),
    }
}

fn projection(
    projection_id: &str,
    source_id: &str,
    artifact_class: TrainArtifactClass,
    authoritative_prefix_template: &str,
    local_stage_prefix_template: &str,
) -> RemoteTrainArtifactFinalizerProjection {
    RemoteTrainArtifactFinalizerProjection {
        projection_id: String::from(projection_id),
        source_id: String::from(source_id),
        artifact_class,
        authoritative_prefix_template: String::from(authoritative_prefix_template),
        local_stage_prefix_template: String::from(local_stage_prefix_template),
        finalizer_manifest_relpath: format!(
            "training_artifacts/{}/projection_manifest_v1.json",
            artifact_class_label(artifact_class)
        ),
        detail: String::from(
            "Finalizer projection freezes one provider-neutral authoritative prefix so publication no longer depends on bespoke provider-root walking.",
        ),
    }
}

fn artifact_class_label(artifact_class: TrainArtifactClass) -> &'static str {
    match artifact_class {
        TrainArtifactClass::Checkpoint => "checkpoint",
        TrainArtifactClass::AdapterContribution => "adapter_contribution",
        TrainArtifactClass::AdapterWindowCheckpoint => "adapter_window_checkpoint",
        TrainArtifactClass::Rollout => "rollout",
        TrainArtifactClass::EvalArtifact => "eval_artifact",
        TrainArtifactClass::LogBundle => "log_bundle",
        TrainArtifactClass::MetricsBundle => "metrics_bundle",
        TrainArtifactClass::FinalEvidenceBundle => "final_evidence_bundle",
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("stable digest serialization should succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_remote_train_artifact_backend_contract_set,
        RemoteTrainArtifactBackendContractError,
        REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_SCHEMA_VERSION,
    };
    use crate::TrainArtifactClass;

    #[test]
    fn canonical_remote_backend_contract_covers_required_classes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_remote_train_artifact_backend_contract_set()?;
        assert_eq!(
            contract.schema_version,
            REMOTE_TRAIN_ARTIFACT_BACKEND_CONTRACT_SCHEMA_VERSION
        );
        assert!(contract
            .placement_decisions
            .iter()
            .any(|decision| decision.artifact_class == TrainArtifactClass::MetricsBundle));
        assert!(contract.placement_decisions.iter().any(|decision| {
            decision.artifact_class == TrainArtifactClass::FinalEvidenceBundle
        }));
        Ok(())
    }

    #[test]
    fn canonical_remote_backend_contract_rejects_missing_finalizer_projection(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut contract = canonical_remote_train_artifact_backend_contract_set()?;
        contract.finalizer_projections.retain(|projection| {
            !(projection.source_id == "google_l4_validator_node"
                && projection.artifact_class == TrainArtifactClass::Checkpoint)
        });
        let manifest = crate::cross_provider_training_program_manifest()?;
        let sources = crate::canonical_cross_provider_compute_source_contracts()?;
        let err = contract
            .validate(&manifest, &sources)
            .expect_err("missing finalizer projection must be rejected");
        assert!(matches!(
            err,
            RemoteTrainArtifactBackendContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
