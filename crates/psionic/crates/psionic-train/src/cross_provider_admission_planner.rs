use std::{collections::BTreeSet, fs, path::Path, sync::OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, canonical_cross_provider_runtime_binder,
    cross_provider_training_program_manifest, CrossProviderComputeSourceContract,
    CrossProviderComputeSourceContractError, CrossProviderComputeSourceLocality,
    CrossProviderCostModel, CrossProviderExecutionClass, CrossProviderRuntimeBinderError,
    CrossProviderStorageKind, CrossProviderTrainingProgramManifestError, CrossProviderTrustTier,
};

/// Stable schema version for the cross-provider admission planner.
pub const CROSS_PROVIDER_ADMISSION_PLANNER_SCHEMA_VERSION: &str =
    "psionic.cross_provider_admission_planner.v1";
/// Stable fixture path for the cross-provider admission planner.
pub const CROSS_PROVIDER_ADMISSION_PLANNER_FIXTURE_PATH: &str =
    "fixtures/training/cross_provider_admission_plan_v1.json";
/// Stable checker path for the cross-provider admission planner.
pub const CROSS_PROVIDER_ADMISSION_PLANNER_CHECK_SCRIPT_PATH: &str =
    "scripts/check-cross-provider-admission-planner.sh";
/// Stable reference doc path for the cross-provider admission planner.
pub const CROSS_PROVIDER_ADMISSION_PLANNER_DOC_PATH: &str =
    "docs/CROSS_PROVIDER_ADMISSION_PLANNER_REFERENCE.md";

/// Errors surfaced while building, validating, or writing the cross-provider admission planner.
#[derive(Debug, Error)]
pub enum CrossProviderAdmissionPlannerError {
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
    RuntimeBinder(#[from] CrossProviderRuntimeBinderError),
    #[error("cross-provider admission planner is invalid: {detail}")]
    InvalidPlan { detail: String },
}

/// Typed refusal kind surfaced by the planner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProviderAdmissionPlannerRefusalKind {
    /// The source contract does not admit the requested execution class.
    SourceExecutionClassNotAdmitted,
    /// The requested background role is too expensive under the current planner policy.
    CostPostureRejected,
}

/// Target count for one execution class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderAdmissionRoleTarget {
    /// Requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Target count for the class in the first planner.
    pub target_count: u16,
    /// Short machine-legible detail.
    pub detail: String,
}

/// One scored candidate evaluation in the planner output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderAdmissionCandidateEvaluation {
    /// Stable source id.
    pub source_id: String,
    /// Requested execution class.
    pub requested_execution_class: CrossProviderExecutionClass,
    /// Whether the planner admitted the source for the class.
    pub admitted: bool,
    /// Whether the planner selected the source for the current target count.
    pub selected: bool,
    /// Deterministic placement rank among admitted candidates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement_rank: Option<u16>,
    /// Trust score contribution.
    pub trust_score: u16,
    /// Network score contribution.
    pub network_score: u16,
    /// Storage score contribution.
    pub storage_score: u16,
    /// Cost score contribution.
    pub cost_score: u16,
    /// Backend and accelerator score contribution.
    pub backend_score: u16,
    /// Whether the source has a shared launch binding today.
    pub binder_alignment_score: u16,
    /// Total score used for deterministic ordering.
    pub total_score: u16,
    /// Typed refusal when not admitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<CrossProviderAdmissionPlannerRefusalKind>,
    /// Machine-legible detail.
    pub detail: String,
}

/// Canonical cross-provider admission plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossProviderAdmissionPlan {
    /// Stable schema version.
    pub schema_version: String,
    /// Root training-program manifest id.
    pub program_manifest_id: String,
    /// Root training-program manifest digest.
    pub program_manifest_digest: String,
    /// Shared runtime binder digest.
    pub runtime_binder_contract_digest: String,
    /// Target counts for each execution class.
    pub role_targets: Vec<CrossProviderAdmissionRoleTarget>,
    /// Candidate evaluations across all sources and execution classes.
    pub candidate_evaluations: Vec<CrossProviderAdmissionCandidateEvaluation>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable plan digest.
    pub plan_digest: String,
}

impl CrossProviderAdmissionPlan {
    /// Returns the stable digest over the admission plan.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.plan_digest.clear();
        stable_digest(b"psionic_cross_provider_admission_plan|", &clone)
    }

    /// Validates the admission plan against the canonical sources and role targets.
    pub fn validate(&self) -> Result<(), CrossProviderAdmissionPlannerError> {
        let manifest = cross_provider_training_program_manifest()?;
        let binder = canonical_cross_provider_runtime_binder()?;
        let sources = canonical_cross_provider_compute_source_contracts()?;
        if self.schema_version != CROSS_PROVIDER_ADMISSION_PLANNER_SCHEMA_VERSION {
            return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    CROSS_PROVIDER_ADMISSION_PLANNER_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                detail: String::from("program-manifest binding drifted"),
            });
        }
        if self.runtime_binder_contract_digest != binder.contract_digest {
            return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                detail: String::from("runtime binder digest drifted"),
            });
        }
        let source_ids: BTreeSet<_> = sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect();
        for source_id in &source_ids {
            for execution_class in &manifest.admitted_execution_classes {
                if !self.candidate_evaluations.iter().any(|evaluation| {
                    evaluation.source_id == *source_id
                        && evaluation.requested_execution_class == *execution_class
                }) {
                    return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                        detail: format!(
                            "missing evaluation for source `{}` and execution class `{:?}`",
                            source_id, execution_class
                        ),
                    });
                }
            }
        }
        for target in &self.role_targets {
            let mut selected = self
                .candidate_evaluations
                .iter()
                .filter(|evaluation| {
                    evaluation.requested_execution_class == target.requested_execution_class
                        && evaluation.selected
                })
                .collect::<Vec<_>>();
            selected.sort_by_key(|evaluation| evaluation.placement_rank.unwrap_or(u16::MAX));
            let admitted_count = self
                .candidate_evaluations
                .iter()
                .filter(|evaluation| {
                    evaluation.requested_execution_class == target.requested_execution_class
                        && evaluation.admitted
                })
                .count() as u16;
            if selected.len() as u16 != admitted_count.min(target.target_count) {
                return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                    detail: format!(
                        "selected count for `{:?}` drifted from target or admitted count",
                        target.requested_execution_class
                    ),
                });
            }
            for pair in selected.windows(2) {
                if pair[0].placement_rank.unwrap_or(u16::MAX)
                    >= pair[1].placement_rank.unwrap_or(u16::MAX)
                    && pair[0].source_id >= pair[1].source_id
                {
                    return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                        detail: format!(
                            "placement ordering for `{:?}` is no longer deterministic",
                            target.requested_execution_class
                        ),
                    });
                }
            }
        }
        if self.plan_digest != self.stable_digest() {
            return Err(CrossProviderAdmissionPlannerError::InvalidPlan {
                detail: String::from("plan_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical cross-provider admission plan.
static CROSS_PROVIDER_ADMISSION_PLAN_CACHE: OnceLock<CrossProviderAdmissionPlan> = OnceLock::new();

pub fn canonical_cross_provider_admission_plan(
) -> Result<CrossProviderAdmissionPlan, CrossProviderAdmissionPlannerError> {
    if let Some(plan) = CROSS_PROVIDER_ADMISSION_PLAN_CACHE.get() {
        return Ok(plan.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let binder = canonical_cross_provider_runtime_binder()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let role_targets = canonical_role_targets();
    let binder_alignment: BTreeSet<_> = binder
        .binding_records
        .iter()
        .map(|binding| binding.admitted_source_contract_id.as_str())
        .collect();

    let mut evaluations = Vec::new();
    for execution_class in &manifest.admitted_execution_classes {
        let mut admitted = Vec::new();
        let mut refused = Vec::new();
        for source in &sources {
            let evaluation = evaluate_source_for_class(
                source,
                *execution_class,
                binder_alignment.contains(source.source_id.as_str()),
            );
            if evaluation.admitted {
                admitted.push(evaluation);
            } else {
                refused.push(evaluation);
            }
        }
        admitted.sort_by_key(|evaluation| {
            (
                std::cmp::Reverse(evaluation.total_score),
                evaluation.source_id.clone(),
            )
        });
        let target_count = role_targets
            .iter()
            .find(|target| target.requested_execution_class == *execution_class)
            .map(|target| target.target_count)
            .unwrap_or(0);
        for (index, evaluation) in admitted.iter_mut().enumerate() {
            evaluation.placement_rank = Some((index + 1) as u16);
            evaluation.selected = (index as u16) < target_count;
        }
        evaluations.extend(admitted);
        evaluations.extend(refused);
    }
    evaluations.sort_by_key(|evaluation| {
        (
            evaluation.requested_execution_class,
            evaluation.source_id.clone(),
        )
    });

    let mut plan = CrossProviderAdmissionPlan {
        schema_version: String::from(CROSS_PROVIDER_ADMISSION_PLANNER_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        runtime_binder_contract_digest: binder.contract_digest.clone(),
        role_targets,
        candidate_evaluations: evaluations,
        claim_boundary: String::from(
            "This planner closes one deterministic cross-provider role-placement policy across the current Google, RunPod, and local sources using cost, network, trust-tier, storage, backend, and binder alignment facts. It does not yet claim public-swarm discovery, automatic provider acquisition, or same-job mixed-backend dense placement.",
        ),
        plan_digest: String::new(),
    };
    plan.plan_digest = plan.stable_digest();
    plan.validate()?;
    let _ = CROSS_PROVIDER_ADMISSION_PLAN_CACHE.set(plan.clone());
    Ok(plan)
}

/// Writes the canonical cross-provider admission plan fixture.
pub fn write_cross_provider_admission_plan(
    output_path: impl AsRef<Path>,
) -> Result<(), CrossProviderAdmissionPlannerError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CrossProviderAdmissionPlannerError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let plan = canonical_cross_provider_admission_plan()?;
    let json = serde_json::to_vec_pretty(&plan)?;
    fs::write(output_path, json).map_err(|error| CrossProviderAdmissionPlannerError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn canonical_role_targets() -> Vec<CrossProviderAdmissionRoleTarget> {
    vec![
        role_target(
            CrossProviderExecutionClass::DenseFullModelRank,
            1,
            "The first cross-provider dense program selects one primary dense-rank source while the broader dense runtime is still widening.",
        ),
        role_target(
            CrossProviderExecutionClass::ValidatedContributorWindow,
            2,
            "The first planner selects the two strongest bounded contributor-window sources.",
        ),
        role_target(
            CrossProviderExecutionClass::Validator,
            2,
            "The first planner selects two validator-capable sources for replay and quarantine review.",
        ),
        role_target(
            CrossProviderExecutionClass::CheckpointWriter,
            1,
            "The first planner selects one primary checkpoint writer with remote-authoritative storage.",
        ),
        role_target(
            CrossProviderExecutionClass::EvalWorker,
            2,
            "The first planner keeps two eval-worker slots active for ongoing model checks.",
        ),
        role_target(
            CrossProviderExecutionClass::DataBuilder,
            2,
            "The first planner selects cheaper background-capable sources for data building.",
        ),
    ]
}

fn evaluate_source_for_class(
    source: &CrossProviderComputeSourceContract,
    execution_class: CrossProviderExecutionClass,
    binder_aligned: bool,
) -> CrossProviderAdmissionCandidateEvaluation {
    if !source.admitted_execution_classes.contains(&execution_class) {
        return refused_evaluation(
            source,
            execution_class,
            CrossProviderAdmissionPlannerRefusalKind::SourceExecutionClassNotAdmitted,
            "The source contract does not admit this execution class under its current backend and claim boundary.",
        );
    }
    if execution_class == CrossProviderExecutionClass::DataBuilder
        && source.cost.cost_model == CrossProviderCostModel::ProviderGuardrailed
        && source
            .cost
            .declared_hourly_cost_usd_cents
            .unwrap_or_default()
            > 10_000
    {
        return refused_evaluation(
            source,
            execution_class,
            CrossProviderAdmissionPlannerRefusalKind::CostPostureRejected,
            "The current planner refuses very expensive provider-metered background data-builder slots when cheaper admitted sources exist.",
        );
    }

    let trust_score = trust_score(source.network.trust_tier, execution_class);
    let network_score = network_score(
        source.locality.clone(),
        source.network.private_connectivity,
        execution_class,
    );
    let storage_score = storage_score(
        source.storage.storage_kind,
        source.storage.checkpoint_writer_capable,
        execution_class,
    );
    let cost_score = cost_score(
        source.cost.cost_model,
        source.cost.declared_hourly_cost_usd_cents,
        execution_class,
    );
    let backend_score = backend_score(source, execution_class);
    let binder_alignment_score = if binder_aligned { 12 } else { 0 };
    let total_score = trust_score
        + network_score
        + storage_score
        + cost_score
        + backend_score
        + binder_alignment_score;
    CrossProviderAdmissionCandidateEvaluation {
        source_id: source.source_id.clone(),
        requested_execution_class: execution_class,
        admitted: true,
        selected: false,
        placement_rank: None,
        trust_score,
        network_score,
        storage_score,
        cost_score,
        backend_score,
        binder_alignment_score,
        total_score,
        refusal_kind: None,
        detail: format!(
            "Planner admitted source `{}` for `{:?}` using trust={}, network={}, storage={}, cost={}, backend={}, binder={}.",
            source.source_id,
            execution_class,
            trust_score,
            network_score,
            storage_score,
            cost_score,
            backend_score,
            binder_alignment_score
        ),
    }
}

fn role_target(
    requested_execution_class: CrossProviderExecutionClass,
    target_count: u16,
    detail: impl Into<String>,
) -> CrossProviderAdmissionRoleTarget {
    CrossProviderAdmissionRoleTarget {
        requested_execution_class,
        target_count,
        detail: detail.into(),
    }
}

fn refused_evaluation(
    source: &CrossProviderComputeSourceContract,
    execution_class: CrossProviderExecutionClass,
    refusal_kind: CrossProviderAdmissionPlannerRefusalKind,
    detail: impl Into<String>,
) -> CrossProviderAdmissionCandidateEvaluation {
    CrossProviderAdmissionCandidateEvaluation {
        source_id: source.source_id.clone(),
        requested_execution_class: execution_class,
        admitted: false,
        selected: false,
        placement_rank: None,
        trust_score: 0,
        network_score: 0,
        storage_score: 0,
        cost_score: 0,
        backend_score: 0,
        binder_alignment_score: 0,
        total_score: 0,
        refusal_kind: Some(refusal_kind),
        detail: detail.into(),
    }
}

fn trust_score(
    trust_tier: CrossProviderTrustTier,
    execution_class: CrossProviderExecutionClass,
) -> u16 {
    match (trust_tier, execution_class) {
        (
            CrossProviderTrustTier::PrivateCloudOperatorManaged,
            CrossProviderExecutionClass::CheckpointWriter,
        ) => 28,
        (CrossProviderTrustTier::PrivateCloudOperatorManaged, _) => 24,
        (
            CrossProviderTrustTier::LocalOperatorManaged,
            CrossProviderExecutionClass::ValidatedContributorWindow,
        ) => 26,
        (CrossProviderTrustTier::LocalOperatorManaged, CrossProviderExecutionClass::Validator) => {
            24
        }
        (CrossProviderTrustTier::LocalOperatorManaged, _) => 18,
        (
            CrossProviderTrustTier::RentedProviderOperatorManaged,
            CrossProviderExecutionClass::DenseFullModelRank,
        ) => 20,
        (CrossProviderTrustTier::RentedProviderOperatorManaged, _) => 8,
    }
}

fn network_score(
    locality: CrossProviderComputeSourceLocality,
    private_connectivity: bool,
    execution_class: CrossProviderExecutionClass,
) -> u16 {
    match execution_class {
        CrossProviderExecutionClass::DenseFullModelRank => {
            if private_connectivity || locality.discovery_posture.contains("single_pod") {
                24
            } else {
                10
            }
        }
        CrossProviderExecutionClass::Validator | CrossProviderExecutionClass::CheckpointWriter => {
            if private_connectivity {
                22
            } else {
                8
            }
        }
        CrossProviderExecutionClass::ValidatedContributorWindow => {
            if private_connectivity {
                20
            } else {
                12
            }
        }
        CrossProviderExecutionClass::EvalWorker | CrossProviderExecutionClass::DataBuilder => {
            if private_connectivity {
                16
            } else {
                10
            }
        }
    }
}

fn storage_score(
    storage_kind: CrossProviderStorageKind,
    checkpoint_writer_capable: bool,
    execution_class: CrossProviderExecutionClass,
) -> u16 {
    match execution_class {
        CrossProviderExecutionClass::CheckpointWriter => {
            match (storage_kind, checkpoint_writer_capable) {
                (CrossProviderStorageKind::RemoteBucketPlusLocalDisk, true) => 30,
                (CrossProviderStorageKind::PersistentProviderWorkspace, true) => 20,
                _ => 0,
            }
        }
        CrossProviderExecutionClass::DenseFullModelRank => match storage_kind {
            CrossProviderStorageKind::PersistentProviderWorkspace => 22,
            CrossProviderStorageKind::RemoteBucketPlusLocalDisk => 18,
            CrossProviderStorageKind::LocalFilesystemOnly => 8,
        },
        CrossProviderExecutionClass::ValidatedContributorWindow => match storage_kind {
            CrossProviderStorageKind::LocalFilesystemOnly => 18,
            CrossProviderStorageKind::RemoteBucketPlusLocalDisk => 16,
            CrossProviderStorageKind::PersistentProviderWorkspace => 10,
        },
        _ => match storage_kind {
            CrossProviderStorageKind::RemoteBucketPlusLocalDisk => 18,
            CrossProviderStorageKind::LocalFilesystemOnly => 16,
            CrossProviderStorageKind::PersistentProviderWorkspace => 8,
        },
    }
}

fn cost_score(
    cost_model: CrossProviderCostModel,
    hourly_cost_cents: Option<u32>,
    execution_class: CrossProviderExecutionClass,
) -> u16 {
    match cost_model {
        CrossProviderCostModel::LocalOperatorOwned => match execution_class {
            CrossProviderExecutionClass::DenseFullModelRank => 8,
            CrossProviderExecutionClass::ValidatedContributorWindow
            | CrossProviderExecutionClass::Validator
            | CrossProviderExecutionClass::EvalWorker
            | CrossProviderExecutionClass::DataBuilder => 24,
            CrossProviderExecutionClass::CheckpointWriter => 12,
        },
        CrossProviderCostModel::ProviderGuardrailed => {
            let hourly = hourly_cost_cents.unwrap_or(2_000);
            match execution_class {
                CrossProviderExecutionClass::DenseFullModelRank => {
                    if hourly <= 15_000 {
                        18
                    } else {
                        10
                    }
                }
                CrossProviderExecutionClass::CheckpointWriter => 14,
                CrossProviderExecutionClass::Validator
                | CrossProviderExecutionClass::EvalWorker
                | CrossProviderExecutionClass::DataBuilder
                | CrossProviderExecutionClass::ValidatedContributorWindow => {
                    if hourly <= 3_000 {
                        18
                    } else {
                        6
                    }
                }
            }
        }
    }
}

fn backend_score(
    source: &CrossProviderComputeSourceContract,
    execution_class: CrossProviderExecutionClass,
) -> u16 {
    match execution_class {
        CrossProviderExecutionClass::DenseFullModelRank => {
            if source.backend.runtime_backend_label == "cuda"
                && source.accelerators.accelerator_count >= 8
            {
                48
            } else if source.backend.runtime_backend_label.contains("cuda") {
                20
            } else {
                6
            }
        }
        CrossProviderExecutionClass::ValidatedContributorWindow => {
            if source.backend.runtime_backend_label.contains("cuda") {
                24
            } else {
                20
            }
        }
        CrossProviderExecutionClass::Validator => {
            if source.backend.runtime_backend_label.contains("mlx") {
                24
            } else {
                20
            }
        }
        CrossProviderExecutionClass::CheckpointWriter => {
            if source.storage.checkpoint_writer_capable {
                20
            } else {
                0
            }
        }
        CrossProviderExecutionClass::EvalWorker => 18,
        CrossProviderExecutionClass::DataBuilder => 16,
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("cross-provider admission planner values must serialize"),
    );
    format!("{:x}", hasher.finalize())
}
