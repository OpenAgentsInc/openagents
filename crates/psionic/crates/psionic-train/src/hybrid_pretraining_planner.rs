use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use psionic_data::builtin_topology_revisable_distributed_data_feed_semantics_report;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, cross_provider_training_program_manifest,
    AdapterDatasetSliceIdentity, AdapterWindowContractError, CrossProviderComputeSourceContract,
    CrossProviderComputeSourceContractError, CrossProviderExecutionClass,
    CrossProviderTrainingProgramManifest, CrossProviderTrainingProgramManifestError,
    CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY,
    CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID,
};

/// Stable schema version for the hybrid pretraining planner.
pub const HYBRID_PRETRAINING_PLAN_SCHEMA_VERSION: &str = "psionic.hybrid_pretraining_plan.v1";
/// Stable fixture path for the canonical hybrid plan.
pub const HYBRID_PRETRAINING_PLAN_FIXTURE_PATH: &str =
    "fixtures/training/hybrid_pretraining_plan_v1.json";
/// Stable checker path for the canonical hybrid plan.
pub const HYBRID_PRETRAINING_PLAN_CHECK_SCRIPT_PATH: &str =
    "scripts/check-hybrid-pretraining-plan.sh";
/// Stable reference doc path for the canonical hybrid plan.
pub const HYBRID_PRETRAINING_PLAN_DOC_PATH: &str = "docs/HYBRID_PRETRAINING_PLANNER_REFERENCE.md";

/// Failure surfaced while building, validating, or writing the hybrid plan.
#[derive(Debug, Error)]
pub enum HybridPretrainingPlanError {
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
    #[error(transparent)]
    AdapterWindow(#[from] AdapterWindowContractError),
    #[error("hybrid pretraining plan is invalid: {detail}")]
    InvalidPlan { detail: String },
}

/// One dense-rank assignment inside the hybrid pretraining plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingDenseRankAssignment {
    /// Stable dense-rank identifier.
    pub assignment_id: String,
    /// Dense rank number.
    pub dense_rank: u16,
    /// Source backing the rank.
    pub source_id: String,
    /// Topology revision authority for the rank set.
    pub topology_revision_id: String,
    /// Topology-revisable data-feed report digest.
    pub data_feed_report_digest: String,
    /// Planned lineage slot for final evidence.
    pub lineage_slot_id: String,
}

/// One validated contributor window assignment inside the hybrid plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingContributorWindowAssignment {
    /// Stable window identifier.
    pub window_id: String,
    /// Source backing the contributor window.
    pub source_id: String,
    /// Dataset slice assigned to the contributor window.
    pub dataset_slice: AdapterDatasetSliceIdentity,
    /// Planned lineage slot for final evidence.
    pub lineage_slot_id: String,
}

/// One validator assignment inside the hybrid plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingValidatorAssignment {
    /// Stable validator assignment id.
    pub assignment_id: String,
    /// Source backing the validator.
    pub source_id: String,
    /// Planned lineage slot for final evidence.
    pub lineage_slot_id: String,
    /// Work classes this validator is expected to verify.
    pub validates_execution_classes: Vec<CrossProviderExecutionClass>,
}

/// One eval-worker assignment inside the hybrid plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingEvalAssignment {
    /// Stable eval assignment id.
    pub assignment_id: String,
    /// Source backing the eval worker.
    pub source_id: String,
    /// Planned lineage slot for final evidence.
    pub lineage_slot_id: String,
    /// Eval target label.
    pub eval_target: String,
}

/// One checkpoint-writer assignment inside the hybrid plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingCheckpointWriterAssignment {
    /// Stable checkpoint-writer assignment id.
    pub assignment_id: String,
    /// Source backing the writer.
    pub source_id: String,
    /// Planned lineage slot for final evidence.
    pub lineage_slot_id: String,
    /// Checkpoint family this writer owns.
    pub checkpoint_family: String,
}

/// One lineage binding used later by final evidence closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingLineageBinding {
    /// Stable lineage slot id.
    pub lineage_slot_id: String,
    /// Execution class that owns the slot.
    pub execution_class: CrossProviderExecutionClass,
    /// Source backing the slot.
    pub source_id: String,
    /// Dataset family id shared by the whole plan.
    pub dataset_family_id: String,
    /// Checkpoint family shared by the whole plan.
    pub checkpoint_family: String,
    /// Stable artifact family expected to occupy the slot.
    pub artifact_family: String,
    /// Stable evidence trace key that later proof bundles must carry forward.
    pub evidence_trace_key: String,
}

/// Canonical hybrid pretraining plan over dense ranks and contributor windows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPretrainingPlan {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable program manifest id.
    pub program_manifest_id: String,
    /// Stable program manifest digest.
    pub program_manifest_digest: String,
    /// Shared dataset family id.
    pub dataset_family_id: String,
    /// Shared checkpoint family.
    pub checkpoint_family: String,
    /// Dense full-model rank assignments.
    pub dense_rank_assignments: Vec<HybridPretrainingDenseRankAssignment>,
    /// Validated contributor window assignments.
    pub contributor_window_assignments: Vec<HybridPretrainingContributorWindowAssignment>,
    /// Validator assignments.
    pub validator_assignments: Vec<HybridPretrainingValidatorAssignment>,
    /// Eval-worker assignments.
    pub eval_assignments: Vec<HybridPretrainingEvalAssignment>,
    /// Checkpoint-writer assignments.
    pub checkpoint_writer_assignments: Vec<HybridPretrainingCheckpointWriterAssignment>,
    /// Shared lineage bindings across work classes.
    pub lineage_bindings: Vec<HybridPretrainingLineageBinding>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable plan digest.
    pub plan_digest: String,
}

impl HybridPretrainingPlan {
    /// Returns the stable digest over the plan payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.plan_digest.clear();
        stable_digest(b"psionic_hybrid_pretraining_plan|", &clone)
    }

    /// Validates the hybrid plan against the root manifest and source contracts.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        source_contracts: &[CrossProviderComputeSourceContract],
    ) -> Result<(), HybridPretrainingPlanError> {
        if self.schema_version != HYBRID_PRETRAINING_PLAN_SCHEMA_VERSION {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    HYBRID_PRETRAINING_PLAN_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("program_manifest_id drifted from the root manifest"),
            });
        }
        if self.program_manifest_digest != manifest.program_manifest_digest {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("program_manifest_digest drifted from the root manifest"),
            });
        }
        if self.dataset_family_id != manifest.dataset_family_id
            || self.dataset_family_id != CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID
        {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("dataset_family_id must stay on the canonical program family"),
            });
        }
        if self.checkpoint_family != manifest.checkpoint_family
            || self.checkpoint_family != CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY
        {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("checkpoint_family must stay on the canonical program family"),
            });
        }
        if self.dense_rank_assignments.len()
            > usize::from(manifest.budget_policy.max_dense_full_model_ranks)
        {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("dense_rank_assignments exceeded manifest budget"),
            });
        }
        if self.contributor_window_assignments.len()
            > usize::from(manifest.budget_policy.max_validated_contributors)
        {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("contributor_window_assignments exceeded manifest budget"),
            });
        }
        if self.validator_assignments.len() > usize::from(manifest.budget_policy.max_validators) {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("validator_assignments exceeded manifest budget"),
            });
        }
        let sources_by_id = source_contracts
            .iter()
            .map(|contract| (contract.source_id.as_str(), contract))
            .collect::<BTreeMap<_, _>>();
        for assignment in &self.dense_rank_assignments {
            validate_source_admits(
                &sources_by_id,
                assignment.source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::DenseFullModelRank,
            )?;
        }
        for assignment in &self.contributor_window_assignments {
            validate_source_admits(
                &sources_by_id,
                assignment.source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::ValidatedContributorWindow,
            )?;
        }
        for assignment in &self.validator_assignments {
            validate_source_admits(
                &sources_by_id,
                assignment.source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::Validator,
            )?;
        }
        for assignment in &self.eval_assignments {
            validate_source_admits(
                &sources_by_id,
                assignment.source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::EvalWorker,
            )?;
        }
        for assignment in &self.checkpoint_writer_assignments {
            validate_source_admits(
                &sources_by_id,
                assignment.source_id.as_str(),
                manifest,
                CrossProviderExecutionClass::CheckpointWriter,
            )?;
        }

        let expected_lineage_slots = self
            .dense_rank_assignments
            .iter()
            .map(|assignment| assignment.lineage_slot_id.clone())
            .chain(
                self.contributor_window_assignments
                    .iter()
                    .map(|assignment| assignment.lineage_slot_id.clone()),
            )
            .chain(
                self.validator_assignments
                    .iter()
                    .map(|assignment| assignment.lineage_slot_id.clone()),
            )
            .chain(
                self.eval_assignments
                    .iter()
                    .map(|assignment| assignment.lineage_slot_id.clone()),
            )
            .chain(
                self.checkpoint_writer_assignments
                    .iter()
                    .map(|assignment| assignment.lineage_slot_id.clone()),
            )
            .collect::<BTreeSet<_>>();
        let actual_lineage_slots = self
            .lineage_bindings
            .iter()
            .map(|binding| binding.lineage_slot_id.clone())
            .collect::<BTreeSet<_>>();
        if expected_lineage_slots != actual_lineage_slots {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from(
                    "lineage_bindings must cover every planned work slot exactly once",
                ),
            });
        }
        if self.plan_digest != self.stable_digest() {
            return Err(HybridPretrainingPlanError::InvalidPlan {
                detail: String::from("plan_digest does not match the stable plan digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical hybrid pretraining plan for the first mixed execution-class wave.
static HYBRID_PRETRAINING_PLAN_CACHE: std::sync::OnceLock<HybridPretrainingPlan> =
    std::sync::OnceLock::new();

pub fn canonical_hybrid_pretraining_plan(
) -> Result<HybridPretrainingPlan, HybridPretrainingPlanError> {
    if let Some(plan) = HYBRID_PRETRAINING_PLAN_CACHE.get() {
        return Ok(plan.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let data_feed_report = builtin_topology_revisable_distributed_data_feed_semantics_report();

    let dense_rank_assignments = (0..8_u16)
        .map(|dense_rank| HybridPretrainingDenseRankAssignment {
            assignment_id: format!("dense-rank-{dense_rank}"),
            dense_rank,
            source_id: String::from("runpod_8xh100_dense_node"),
            topology_revision_id: String::from("xtrain-hybrid-topology-1"),
            data_feed_report_digest: data_feed_report.report_digest.clone(),
            lineage_slot_id: format!("lineage.dense_rank.{dense_rank}"),
        })
        .collect::<Vec<_>>();

    let contributor_window_assignments = vec![
        HybridPretrainingContributorWindowAssignment {
            window_id: String::from("contributor-window-mac-1"),
            source_id: String::from("local_mlx_mac_workstation"),
            dataset_slice: contributor_dataset_slice("mac", 0)?,
            lineage_slot_id: String::from("lineage.contributor_window.mac.1"),
        },
        HybridPretrainingContributorWindowAssignment {
            window_id: String::from("contributor-window-rtx4080-1"),
            source_id: String::from("local_rtx4080_workstation"),
            dataset_slice: contributor_dataset_slice("rtx4080", 1)?,
            lineage_slot_id: String::from("lineage.contributor_window.rtx4080.1"),
        },
        HybridPretrainingContributorWindowAssignment {
            window_id: String::from("contributor-window-google-1"),
            source_id: String::from("google_l4_validator_node"),
            dataset_slice: contributor_dataset_slice("google", 2)?,
            lineage_slot_id: String::from("lineage.contributor_window.google.1"),
        },
    ];

    let validator_assignments = vec![
        HybridPretrainingValidatorAssignment {
            assignment_id: String::from("validator-google-1"),
            source_id: String::from("google_l4_validator_node"),
            lineage_slot_id: String::from("lineage.validator.google.1"),
            validates_execution_classes: vec![
                CrossProviderExecutionClass::ValidatedContributorWindow,
                CrossProviderExecutionClass::DenseFullModelRank,
            ],
        },
        HybridPretrainingValidatorAssignment {
            assignment_id: String::from("validator-mac-1"),
            source_id: String::from("local_mlx_mac_workstation"),
            lineage_slot_id: String::from("lineage.validator.mac.1"),
            validates_execution_classes: vec![
                CrossProviderExecutionClass::ValidatedContributorWindow,
            ],
        },
    ];

    let eval_assignments = vec![
        HybridPretrainingEvalAssignment {
            assignment_id: String::from("eval-google-1"),
            source_id: String::from("google_l4_validator_node"),
            lineage_slot_id: String::from("lineage.eval.google.1"),
            eval_target: String::from("dense_pretrain_holdout"),
        },
        HybridPretrainingEvalAssignment {
            assignment_id: String::from("eval-rtx4080-1"),
            source_id: String::from("local_rtx4080_workstation"),
            lineage_slot_id: String::from("lineage.eval.rtx4080.1"),
            eval_target: String::from("contributor_replay_probe"),
        },
    ];

    let checkpoint_writer_assignments = vec![
        HybridPretrainingCheckpointWriterAssignment {
            assignment_id: String::from("checkpoint-writer-runpod-1"),
            source_id: String::from("runpod_8xh100_dense_node"),
            lineage_slot_id: String::from("lineage.checkpoint_writer.runpod.1"),
            checkpoint_family: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY),
        },
        HybridPretrainingCheckpointWriterAssignment {
            assignment_id: String::from("checkpoint-writer-google-1"),
            source_id: String::from("google_l4_validator_node"),
            lineage_slot_id: String::from("lineage.checkpoint_writer.google.1"),
            checkpoint_family: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY),
        },
    ];

    let lineage_bindings = dense_rank_assignments
        .iter()
        .map(|assignment| {
            lineage_binding(
                assignment.lineage_slot_id.as_str(),
                CrossProviderExecutionClass::DenseFullModelRank,
                assignment.source_id.as_str(),
                "dense_full_model_update",
            )
        })
        .chain(contributor_window_assignments.iter().map(|assignment| {
            lineage_binding(
                assignment.lineage_slot_id.as_str(),
                CrossProviderExecutionClass::ValidatedContributorWindow,
                assignment.source_id.as_str(),
                "validated_contributor_delta",
            )
        }))
        .chain(validator_assignments.iter().map(|assignment| {
            lineage_binding(
                assignment.lineage_slot_id.as_str(),
                CrossProviderExecutionClass::Validator,
                assignment.source_id.as_str(),
                "validator_verdict",
            )
        }))
        .chain(eval_assignments.iter().map(|assignment| {
            lineage_binding(
                assignment.lineage_slot_id.as_str(),
                CrossProviderExecutionClass::EvalWorker,
                assignment.source_id.as_str(),
                "eval_metrics",
            )
        }))
        .chain(checkpoint_writer_assignments.iter().map(|assignment| {
            lineage_binding(
                assignment.lineage_slot_id.as_str(),
                CrossProviderExecutionClass::CheckpointWriter,
                assignment.source_id.as_str(),
                "checkpoint_manifest",
            )
        }))
        .collect::<Vec<_>>();

    let mut plan = HybridPretrainingPlan {
        schema_version: String::from(HYBRID_PRETRAINING_PLAN_SCHEMA_VERSION),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        dataset_family_id: manifest.dataset_family_id.clone(),
        checkpoint_family: manifest.checkpoint_family.clone(),
        dense_rank_assignments,
        contributor_window_assignments,
        validator_assignments,
        eval_assignments,
        checkpoint_writer_assignments,
        lineage_bindings,
        claim_boundary: String::from(
            "This hybrid plan proves one provider-neutral pretraining program can emit dense-rank work, validated contributor windows, validator work, eval work, and checkpoint-writer work under one shared dataset family and checkpoint family. It does not claim same-job mixed-backend dense training or provider launch closure by itself.",
        ),
        plan_digest: String::new(),
    };
    plan.plan_digest = plan.stable_digest();
    plan.validate(&manifest, sources.as_slice())?;
    let _ = HYBRID_PRETRAINING_PLAN_CACHE.set(plan.clone());
    Ok(plan)
}

/// Writes the canonical hybrid pretraining plan fixture.
pub fn write_hybrid_pretraining_plan(
    output_path: impl AsRef<Path>,
) -> Result<(), HybridPretrainingPlanError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| HybridPretrainingPlanError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&canonical_hybrid_pretraining_plan()?)?;
    fs::write(output_path, bytes).map_err(|error| HybridPretrainingPlanError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn contributor_dataset_slice(
    source_label: &str,
    slice_index: u32,
) -> Result<AdapterDatasetSliceIdentity, AdapterWindowContractError> {
    let slice_id = format!("contributor-slice-{source_label}-{slice_index}");
    AdapterDatasetSliceIdentity::new(
        CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID,
        "train",
        slice_id.clone(),
        stable_digest(b"psionic_hybrid_contributor_slice|", &slice_id),
    )
}

fn lineage_binding(
    lineage_slot_id: &str,
    execution_class: CrossProviderExecutionClass,
    source_id: &str,
    artifact_family: &str,
) -> HybridPretrainingLineageBinding {
    HybridPretrainingLineageBinding {
        lineage_slot_id: String::from(lineage_slot_id),
        execution_class,
        source_id: String::from(source_id),
        dataset_family_id: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_DATASET_FAMILY_ID),
        checkpoint_family: String::from(CROSS_PROVIDER_TRAINING_PROGRAM_CHECKPOINT_FAMILY),
        artifact_family: String::from(artifact_family),
        evidence_trace_key: stable_digest(
            b"psionic_hybrid_pretraining_lineage_trace|",
            &(lineage_slot_id, artifact_family, source_id),
        ),
    }
}

fn validate_source_admits(
    sources_by_id: &BTreeMap<&str, &CrossProviderComputeSourceContract>,
    source_id: &str,
    manifest: &CrossProviderTrainingProgramManifest,
    execution_class: CrossProviderExecutionClass,
) -> Result<(), HybridPretrainingPlanError> {
    let source =
        sources_by_id
            .get(source_id)
            .ok_or_else(|| HybridPretrainingPlanError::InvalidPlan {
                detail: format!("plan referenced unknown source `{source_id}`"),
            })?;
    source
        .admit_execution_class(manifest, execution_class)
        .map_err(|refusal| HybridPretrainingPlanError::InvalidPlan {
            detail: refusal.detail,
        })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable hybrid planner serialization"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::canonical_hybrid_pretraining_plan;
    use crate::CrossProviderExecutionClass;

    #[test]
    fn hybrid_plan_carries_dense_and_contributor_work_classes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_hybrid_pretraining_plan()?;
        assert!(!plan.dense_rank_assignments.is_empty());
        assert!(!plan.contributor_window_assignments.is_empty());
        assert!(plan
            .lineage_bindings
            .iter()
            .any(|binding| binding.execution_class
                == CrossProviderExecutionClass::DenseFullModelRank));
        assert!(plan
            .lineage_bindings
            .iter()
            .any(|binding| binding.execution_class
                == CrossProviderExecutionClass::ValidatedContributorWindow));
        Ok(())
    }

    #[test]
    fn hybrid_plan_digest_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_hybrid_pretraining_plan()?;
        assert_eq!(plan.plan_digest, plan.stable_digest());
        Ok(())
    }
}
