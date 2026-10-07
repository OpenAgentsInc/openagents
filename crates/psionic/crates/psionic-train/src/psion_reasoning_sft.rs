use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{
    PsionArtifactLineageManifest, PsionBenchmarkIsolationError, PsionExclusionManifest,
    PsionLoaderSurface, PsionSftArtifactLineage, PsionSourceLifecycleError,
    PsionSourceLifecycleManifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionPretrainStageRunReceipt, TrainingCheckpointPromotionReceipt, TrainingSftTraceKind,
    TrainingStageCompletionReceipt, TrainingStageKind, TrainingStageProgramState,
    TrainingStageState, TrainingStageTransitionReceipt,
};

/// Stable schema version for the first Psion reasoning-SFT dataset bundle.
pub const PSION_REASONING_SFT_DATASET_BUNDLE_SCHEMA_VERSION: &str =
    "psion.reasoning_sft_dataset_bundle.v1";
/// Stable schema version for the first Psion reasoning-SFT stage receipt.
pub const PSION_REASONING_SFT_STAGE_RECEIPT_SCHEMA_VERSION: &str =
    "psion.reasoning_sft_stage_receipt.v1";
/// Stable schema version for the first Psion reasoning-SFT evaluation receipt.
pub const PSION_REASONING_SFT_EVALUATION_RECEIPT_SCHEMA_VERSION: &str =
    "psion.reasoning_sft_evaluation_receipt.v1";
/// Stable schema version for the first Psion reasoning-SFT run bundle.
pub const PSION_REASONING_SFT_RUN_BUNDLE_SCHEMA_VERSION: &str = "psion.reasoning_sft_run_bundle.v1";
/// Minimum number of preserved style profiles for the bounded reasoning-SFT lane.
pub const PSION_REASONING_SFT_MINIMUM_STYLE_PROFILES: usize = 3;
/// Maximum share any single style may occupy before the lane is treated as collapsed.
pub const PSION_REASONING_SFT_MAX_SINGLE_STYLE_SHARE_BPS: u32 = 6000;
/// Minimum retained explicit-assumption rate for the bounded reasoning-SFT lane.
pub const PSION_REASONING_SFT_MINIMUM_ASSUMPTION_RETENTION_BPS: u32 = 9500;
/// Minimum retained uncertainty-language rate for the bounded reasoning-SFT lane.
pub const PSION_REASONING_SFT_MINIMUM_UNCERTAINTY_RETENTION_BPS: u32 = 9400;
/// Minimum normative-versus-inference separation rate for the bounded reasoning-SFT lane.
pub const PSION_REASONING_SFT_MINIMUM_NORMATIVE_INFERENCE_SEPARATION_BPS: u32 = 9500;
/// Minimum multi-style evaluation pass rate for the bounded reasoning-SFT lane.
pub const PSION_REASONING_SFT_MINIMUM_MULTI_STYLE_PASS_RATE_BPS: u32 = 9000;

/// Explicit control surface for the first bounded reasoning-SFT lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftControlSurface {
    /// Whether assumption statements must stay explicit in the output.
    pub explicit_assumptions_required: bool,
    /// Whether uncertainty language must stay explicit instead of being deleted.
    pub explicit_uncertainty_language_required: bool,
    /// Whether normative statements and engineering inference must stay separated.
    pub normative_vs_inference_separation_required: bool,
    /// Short explanation of the truth-control posture.
    pub detail: String,
}

impl PsionReasoningSftControlSurface {
    fn validate(&self, field_prefix: &str) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.detail.as_str(),
            format!("{field_prefix}.detail").as_str(),
        )?;
        ensure_bool_true(
            self.explicit_assumptions_required,
            format!("{field_prefix}.explicit_assumptions_required").as_str(),
        )?;
        ensure_bool_true(
            self.explicit_uncertainty_language_required,
            format!("{field_prefix}.explicit_uncertainty_language_required").as_str(),
        )?;
        ensure_bool_true(
            self.normative_vs_inference_separation_required,
            format!("{field_prefix}.normative_vs_inference_separation_required").as_str(),
        )?;
        Ok(())
    }
}

/// Decomposition strategy preserved by the reasoning-SFT lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionReasoningSftDecompositionStrategy {
    /// Surface assumptions first, then explain the mechanism.
    AssumptionsThenMechanism,
    /// State the bounded answer first, then show supporting evidence.
    AnswerThenEvidence,
    /// Start from constraints, then walk tradeoffs.
    ConstraintsThenTradeoffs,
}

/// Explanation order preserved by the reasoning-SFT lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionReasoningSftExplanationOrder {
    /// Premises first, conclusion second.
    PremisesThenConclusion,
    /// Conclusion first, premises second.
    ConclusionThenPremises,
    /// High-level frame first, concrete details second.
    TopDownThenConcrete,
}

/// Abstraction level preserved by the reasoning-SFT lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionReasoningSftAbstractionLevel {
    /// Stay close to implementation details.
    Concrete,
    /// Mix conceptual framing with implementation detail.
    Hybrid,
    /// Stay at the higher-level reasoning frame.
    Conceptual,
}

/// One admissible reasoning style profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftStyleProfile {
    /// Stable style identifier.
    pub style_id: String,
    /// Decomposition strategy for the style.
    pub decomposition_strategy: PsionReasoningSftDecompositionStrategy,
    /// Explanation order for the style.
    pub explanation_order: PsionReasoningSftExplanationOrder,
    /// Abstraction level for the style.
    pub abstraction_level: PsionReasoningSftAbstractionLevel,
    /// Short explanation of the profile.
    pub detail: String,
}

impl PsionReasoningSftStyleProfile {
    fn validate(&self) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.style_id.as_str(),
            "reasoning_sft_style_profile.style_id",
        )?;
        ensure_nonempty(self.detail.as_str(), "reasoning_sft_style_profile.detail")?;
        Ok(())
    }
}

/// Trace-level provenance and truth annotations for one reasoning-SFT example.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftTraceBinding {
    /// Stable trace identifier.
    pub trace_id: String,
    /// Trace family admitted by the stage program.
    pub trace_kind: TrainingSftTraceKind,
    /// Stable trace-lineage digest from the stage program.
    pub trace_lineage_digest: String,
    /// Style profile selected for the trace.
    pub style_profile_id: String,
    /// Parent source ids used to derive the trace.
    pub parent_source_ids: Vec<String>,
    /// Parent tokenized corpus ids used to derive the trace.
    pub parent_tokenized_corpus_ids: Vec<String>,
    /// Whether the trace is explicitly derived rather than raw.
    pub derived_from_parent_sources: bool,
    /// Whether held-out exclusion was mechanically checked for the parent source set.
    pub held_out_exclusion_checked: bool,
    /// Whether assumptions remain explicit in the target output.
    pub explicit_assumptions_present: bool,
    /// Whether uncertainty language remains explicit in the target output.
    pub explicit_uncertainty_language_present: bool,
    /// Whether normative statements stay separated from engineering inference.
    pub normative_vs_inference_separated: bool,
    /// Short explanation of the trace binding.
    pub detail: String,
}

impl PsionReasoningSftTraceBinding {
    fn validate(&self) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.trace_id.as_str(),
            "reasoning_sft_trace_binding.trace_id",
        )?;
        ensure_nonempty(
            self.trace_lineage_digest.as_str(),
            "reasoning_sft_trace_binding.trace_lineage_digest",
        )?;
        ensure_nonempty(
            self.style_profile_id.as_str(),
            "reasoning_sft_trace_binding.style_profile_id",
        )?;
        ensure_nonempty(self.detail.as_str(), "reasoning_sft_trace_binding.detail")?;
        if self.parent_source_ids.is_empty() {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from("reasoning_sft_trace_binding.parent_source_ids"),
            });
        }
        if self.parent_tokenized_corpus_ids.is_empty() {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from("reasoning_sft_trace_binding.parent_tokenized_corpus_ids"),
            });
        }
        ensure_bool_true(
            self.derived_from_parent_sources,
            "reasoning_sft_trace_binding.derived_from_parent_sources",
        )?;
        ensure_bool_true(
            self.held_out_exclusion_checked,
            "reasoning_sft_trace_binding.held_out_exclusion_checked",
        )?;
        ensure_bool_true(
            self.explicit_assumptions_present,
            "reasoning_sft_trace_binding.explicit_assumptions_present",
        )?;
        ensure_bool_true(
            self.explicit_uncertainty_language_present,
            "reasoning_sft_trace_binding.explicit_uncertainty_language_present",
        )?;
        ensure_bool_true(
            self.normative_vs_inference_separated,
            "reasoning_sft_trace_binding.normative_vs_inference_separated",
        )?;
        Ok(())
    }
}

/// Dataset bundle for the first bounded reasoning-SFT lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftDatasetBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Bound pretrain-stage receipt digest.
    pub pretrain_stage_receipt_digest: String,
    /// Bound SFT artifact-lineage row.
    pub sft_artifact_lineage: PsionSftArtifactLineage,
    /// Truth-control surface frozen for the bundle.
    pub control_surface: PsionReasoningSftControlSurface,
    /// Admitted reasoning styles.
    pub style_profiles: Vec<PsionReasoningSftStyleProfile>,
    /// Trace bindings admitted into the stage.
    pub trace_bindings: Vec<PsionReasoningSftTraceBinding>,
    /// Short explanation of the bundle.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionReasoningSftDatasetBundle {
    /// Validates the dataset bundle against the stage program and Psion data governance state.
    pub fn validate_against_context(
        &self,
        stage_program: &TrainingStageProgramState,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
        artifact_lineage: &PsionArtifactLineageManifest,
    ) -> Result<(), PsionReasoningSftError> {
        self.validate_shape()?;
        let stage_context = reasoning_stage_context(stage_program)?;
        check_string_match(
            self.run_id.as_str(),
            stage_context.stage_program.run_id.as_str(),
            "reasoning_sft_dataset_bundle.run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_context.general_sft_stage.stage_id.as_str(),
            "reasoning_sft_dataset_bundle.stage_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            stage_context.pretrain_receipt.model_id.as_str(),
            "reasoning_sft_dataset_bundle.model_id",
        )?;
        check_string_match(
            self.pretrain_stage_receipt_digest.as_str(),
            stage_context.pretrain_receipt.receipt_digest.as_str(),
            "reasoning_sft_dataset_bundle.pretrain_stage_receipt_digest",
        )?;

        artifact_lineage
            .validate_against_lifecycle(lifecycle)
            .map_err(PsionReasoningSftError::SourceLifecycle)?;
        exclusion
            .validate_against_lifecycle(lifecycle)
            .map_err(PsionReasoningSftError::BenchmarkIsolation)?;

        let expected_lineage = artifact_lineage
            .sft_artifacts
            .iter()
            .find(|lineage| lineage.artifact_id == self.sft_artifact_lineage.artifact_id)
            .ok_or_else(|| PsionReasoningSftError::UnknownSftArtifactLineage {
                artifact_id: self.sft_artifact_lineage.artifact_id.clone(),
            })?;
        if &self.sft_artifact_lineage != expected_lineage {
            return Err(PsionReasoningSftError::SftArtifactLineageMismatch {
                artifact_id: self.sft_artifact_lineage.artifact_id.clone(),
            });
        }
        exclusion
            .assert_source_ids_allowed(
                lifecycle,
                PsionLoaderSurface::ModelTraining,
                self.sft_artifact_lineage.source_ids.as_slice(),
            )
            .map_err(PsionReasoningSftError::BenchmarkIsolation)?;

        if stage_context.general_sft_stage.ingested_traces.len() != self.trace_bindings.len() {
            return Err(PsionReasoningSftError::CountMismatch {
                field: String::from("reasoning_sft_dataset_bundle.trace_bindings"),
                expected: stage_context.general_sft_stage.ingested_traces.len(),
                actual: self.trace_bindings.len(),
            });
        }

        let ingested_by_trace_id = stage_context
            .general_sft_stage
            .ingested_traces
            .iter()
            .map(|trace| (trace.trace_id.as_str(), trace))
            .collect::<BTreeMap<_, _>>();
        for binding in &self.trace_bindings {
            let ingested = ingested_by_trace_id
                .get(binding.trace_id.as_str())
                .ok_or_else(|| PsionReasoningSftError::UnknownStageTrace {
                    trace_id: binding.trace_id.clone(),
                })?;
            if ingested.trace_kind != binding.trace_kind {
                return Err(PsionReasoningSftError::FieldMismatch {
                    field: format!(
                        "reasoning_sft_dataset_bundle.trace_bindings[{}].trace_kind",
                        binding.trace_id
                    ),
                    expected: format!("{:?}", ingested.trace_kind),
                    actual: format!("{:?}", binding.trace_kind),
                });
            }
            check_string_match(
                binding.trace_lineage_digest.as_str(),
                ingested.lineage_digest.as_str(),
                format!(
                    "reasoning_sft_dataset_bundle.trace_bindings[{}].trace_lineage_digest",
                    binding.trace_id
                )
                .as_str(),
            )?;
            for source_id in &binding.parent_source_ids {
                if !self
                    .sft_artifact_lineage
                    .source_ids
                    .iter()
                    .any(|candidate| candidate == source_id)
                {
                    return Err(PsionReasoningSftError::UnknownTraceSourceInSftLineage {
                        trace_id: binding.trace_id.clone(),
                        source_id: source_id.clone(),
                    });
                }
            }
            for corpus_id in &binding.parent_tokenized_corpus_ids {
                if !self
                    .sft_artifact_lineage
                    .tokenized_corpus_ids
                    .iter()
                    .any(|candidate| candidate == corpus_id)
                {
                    return Err(PsionReasoningSftError::UnknownTraceCorpusInSftLineage {
                        trace_id: binding.trace_id.clone(),
                        corpus_id: corpus_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "reasoning_sft_dataset_bundle.schema_version",
        )?;
        if self.schema_version != PSION_REASONING_SFT_DATASET_BUNDLE_SCHEMA_VERSION {
            return Err(PsionReasoningSftError::SchemaVersionMismatch {
                expected: String::from(PSION_REASONING_SFT_DATASET_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.run_id.as_str(), "reasoning_sft_dataset_bundle.run_id")?;
        ensure_nonempty(
            self.stage_id.as_str(),
            "reasoning_sft_dataset_bundle.stage_id",
        )?;
        ensure_nonempty(
            self.model_id.as_str(),
            "reasoning_sft_dataset_bundle.model_id",
        )?;
        ensure_nonempty(
            self.pretrain_stage_receipt_digest.as_str(),
            "reasoning_sft_dataset_bundle.pretrain_stage_receipt_digest",
        )?;
        ensure_nonempty(
            self.sft_artifact_lineage.artifact_id.as_str(),
            "reasoning_sft_dataset_bundle.sft_artifact_lineage.artifact_id",
        )?;
        ensure_nonempty(
            self.sft_artifact_lineage.artifact_digest.as_str(),
            "reasoning_sft_dataset_bundle.sft_artifact_lineage.artifact_digest",
        )?;
        if self.sft_artifact_lineage.tokenized_corpus_ids.is_empty() {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from(
                    "reasoning_sft_dataset_bundle.sft_artifact_lineage.tokenized_corpus_ids",
                ),
            });
        }
        if self.sft_artifact_lineage.source_ids.is_empty() {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from("reasoning_sft_dataset_bundle.sft_artifact_lineage.source_ids"),
            });
        }
        self.control_surface
            .validate("reasoning_sft_dataset_bundle.control_surface")?;
        ensure_nonempty(
            self.summary.as_str(),
            "reasoning_sft_dataset_bundle.summary",
        )?;
        if self.style_profiles.len() < PSION_REASONING_SFT_MINIMUM_STYLE_PROFILES {
            return Err(PsionReasoningSftError::StyleProfileCountTooSmall {
                expected: PSION_REASONING_SFT_MINIMUM_STYLE_PROFILES,
                actual: self.style_profiles.len(),
            });
        }
        if self.trace_bindings.is_empty() {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from("reasoning_sft_dataset_bundle.trace_bindings"),
            });
        }

        let mut style_ids = BTreeSet::new();
        for profile in &self.style_profiles {
            profile.validate()?;
            if !style_ids.insert(profile.style_id.clone()) {
                return Err(PsionReasoningSftError::DuplicateStyleProfile {
                    style_id: profile.style_id.clone(),
                });
            }
        }

        let mut trace_ids = BTreeSet::new();
        let mut trace_digests = BTreeSet::new();
        let mut trace_count_by_style = BTreeMap::<&str, usize>::new();
        for binding in &self.trace_bindings {
            binding.validate()?;
            if !trace_ids.insert(binding.trace_id.clone()) {
                return Err(PsionReasoningSftError::DuplicateTraceBinding {
                    trace_id: binding.trace_id.clone(),
                });
            }
            if !trace_digests.insert(binding.trace_lineage_digest.clone()) {
                return Err(PsionReasoningSftError::DuplicateTraceLineageDigest {
                    trace_lineage_digest: binding.trace_lineage_digest.clone(),
                });
            }
            if !style_ids.contains(binding.style_profile_id.as_str()) {
                return Err(PsionReasoningSftError::UnknownStyleProfile {
                    style_id: binding.style_profile_id.clone(),
                });
            }
            *trace_count_by_style
                .entry(binding.style_profile_id.as_str())
                .or_default() += 1;
        }
        for style_id in style_ids {
            if !trace_count_by_style.contains_key(style_id.as_str()) {
                return Err(PsionReasoningSftError::UnusedStyleProfile { style_id });
            }
        }

        if self.bundle_digest != stable_reasoning_sft_dataset_bundle_digest(self) {
            return Err(PsionReasoningSftError::DigestMismatch {
                kind: String::from("reasoning_sft_dataset_bundle"),
            });
        }
        Ok(())
    }
}

/// One style-coverage row recorded on the reasoning-SFT stage receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftStyleCoverageRow {
    /// Stable style identifier.
    pub style_id: String,
    /// Accepted trace count for the style.
    pub trace_count: u32,
    /// Share of accepted traces in basis points.
    pub share_bps: u32,
    /// Short explanation of the style row.
    pub detail: String,
}

/// Stage receipt for the first bounded reasoning-SFT lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftStageReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Bound pretrain-stage receipt digest.
    pub pretrain_stage_receipt_digest: String,
    /// Bound dataset-bundle digest.
    pub dataset_bundle_digest: String,
    /// Bound transition digest into the general-SFT stage.
    pub general_sft_transition_digest: String,
    /// Bound completion digest for the general-SFT stage.
    pub general_sft_completion_digest: String,
    /// Truth-control surface frozen for the stage.
    pub control_surface: PsionReasoningSftControlSurface,
    /// Style coverage rows across the accepted trace set.
    pub style_coverage: Vec<PsionReasoningSftStyleCoverageRow>,
    /// Number of sanity probes used to verify explicit reasoning posture.
    pub sanity_probe_count: u32,
    /// Number of probes that preserved explicit assumptions.
    pub assumption_retained_probe_count: u32,
    /// Number of probes that preserved uncertainty language.
    pub uncertainty_retained_probe_count: u32,
    /// Number of probes that preserved normative-vs-inference separation.
    pub normative_vs_inference_separated_probe_count: u32,
    /// Aggregate assumption-retention rate in basis points.
    pub assumption_retention_bps: u32,
    /// Aggregate uncertainty-retention rate in basis points.
    pub uncertainty_retention_bps: u32,
    /// Aggregate normative-vs-inference separation rate in basis points.
    pub normative_vs_inference_separation_bps: u32,
    /// Short explanation of the stage result.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionReasoningSftStageReceipt {
    /// Validates the stage receipt against the program state and dataset bundle.
    pub fn validate_against_stage(
        &self,
        stage_program: &TrainingStageProgramState,
        dataset_bundle: &PsionReasoningSftDatasetBundle,
    ) -> Result<(), PsionReasoningSftError> {
        self.validate_shape()?;
        let stage_context = reasoning_stage_context(stage_program)?;
        check_string_match(
            self.run_id.as_str(),
            stage_context.stage_program.run_id.as_str(),
            "reasoning_sft_stage_receipt.run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_context.general_sft_stage.stage_id.as_str(),
            "reasoning_sft_stage_receipt.stage_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            dataset_bundle.model_id.as_str(),
            "reasoning_sft_stage_receipt.model_id",
        )?;
        check_string_match(
            self.pretrain_stage_receipt_digest.as_str(),
            dataset_bundle.pretrain_stage_receipt_digest.as_str(),
            "reasoning_sft_stage_receipt.pretrain_stage_receipt_digest",
        )?;
        check_string_match(
            self.dataset_bundle_digest.as_str(),
            dataset_bundle.bundle_digest.as_str(),
            "reasoning_sft_stage_receipt.dataset_bundle_digest",
        )?;
        check_string_match(
            self.general_sft_transition_digest.as_str(),
            stage_context
                .general_sft_transition
                .transition_digest
                .as_str(),
            "reasoning_sft_stage_receipt.general_sft_transition_digest",
        )?;
        check_string_match(
            self.general_sft_completion_digest.as_str(),
            stage_context
                .general_sft_completion
                .completion_digest
                .as_str(),
            "reasoning_sft_stage_receipt.general_sft_completion_digest",
        )?;
        if self.control_surface != dataset_bundle.control_surface {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from("reasoning_sft_stage_receipt.control_surface"),
                expected: String::from("stage control surface must match the dataset bundle"),
                actual: String::from("control surface drifted"),
            });
        }

        let expected_coverage = expected_style_coverage(dataset_bundle)?;
        if self.style_coverage.len() != expected_coverage.len() {
            return Err(PsionReasoningSftError::CountMismatch {
                field: String::from("reasoning_sft_stage_receipt.style_coverage"),
                expected: expected_coverage.len(),
                actual: self.style_coverage.len(),
            });
        }
        for (observed, expected) in self.style_coverage.iter().zip(expected_coverage.iter()) {
            check_string_match(
                observed.style_id.as_str(),
                expected.style_id.as_str(),
                "reasoning_sft_stage_receipt.style_coverage[].style_id",
            )?;
            if observed.trace_count != expected.trace_count {
                return Err(PsionReasoningSftError::FieldMismatch {
                    field: format!(
                        "reasoning_sft_stage_receipt.style_coverage[{}].trace_count",
                        observed.style_id
                    ),
                    expected: expected.trace_count.to_string(),
                    actual: observed.trace_count.to_string(),
                });
            }
            if observed.share_bps != expected.share_bps {
                return Err(PsionReasoningSftError::FieldMismatch {
                    field: format!(
                        "reasoning_sft_stage_receipt.style_coverage[{}].share_bps",
                        observed.style_id
                    ),
                    expected: expected.share_bps.to_string(),
                    actual: observed.share_bps.to_string(),
                });
            }
            ensure_nonempty(
                observed.detail.as_str(),
                "reasoning_sft_stage_receipt.style_coverage[].detail",
            )?;
        }
        let max_style_share = self
            .style_coverage
            .iter()
            .map(|row| row.share_bps)
            .max()
            .unwrap_or(0);
        if max_style_share > PSION_REASONING_SFT_MAX_SINGLE_STYLE_SHARE_BPS {
            return Err(PsionReasoningSftError::StyleCollapse {
                observed_share_bps: max_style_share,
                maximum_allowed_bps: PSION_REASONING_SFT_MAX_SINGLE_STYLE_SHARE_BPS,
            });
        }
        let total_share_bps = self
            .style_coverage
            .iter()
            .fold(0_u32, |acc, row| acc.saturating_add(row.share_bps));
        if total_share_bps != 10_000 {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from("reasoning_sft_stage_receipt.style_coverage.share_bps"),
                expected: String::from("10000"),
                actual: total_share_bps.to_string(),
            });
        }

        validate_retention_metric(
            self.sanity_probe_count,
            self.assumption_retained_probe_count,
            self.assumption_retention_bps,
            PSION_REASONING_SFT_MINIMUM_ASSUMPTION_RETENTION_BPS,
            "reasoning_sft_stage_receipt.assumption_retention_bps",
        )?;
        validate_retention_metric(
            self.sanity_probe_count,
            self.uncertainty_retained_probe_count,
            self.uncertainty_retention_bps,
            PSION_REASONING_SFT_MINIMUM_UNCERTAINTY_RETENTION_BPS,
            "reasoning_sft_stage_receipt.uncertainty_retention_bps",
        )?;
        validate_retention_metric(
            self.sanity_probe_count,
            self.normative_vs_inference_separated_probe_count,
            self.normative_vs_inference_separation_bps,
            PSION_REASONING_SFT_MINIMUM_NORMATIVE_INFERENCE_SEPARATION_BPS,
            "reasoning_sft_stage_receipt.normative_vs_inference_separation_bps",
        )?;

        if self.receipt_digest != stable_reasoning_sft_stage_receipt_digest(self) {
            return Err(PsionReasoningSftError::DigestMismatch {
                kind: String::from("reasoning_sft_stage_receipt"),
            });
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "reasoning_sft_stage_receipt.schema_version",
        )?;
        if self.schema_version != PSION_REASONING_SFT_STAGE_RECEIPT_SCHEMA_VERSION {
            return Err(PsionReasoningSftError::SchemaVersionMismatch {
                expected: String::from(PSION_REASONING_SFT_STAGE_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "reasoning_sft_stage_receipt.receipt_id",
        )?;
        ensure_nonempty(self.run_id.as_str(), "reasoning_sft_stage_receipt.run_id")?;
        ensure_nonempty(
            self.stage_id.as_str(),
            "reasoning_sft_stage_receipt.stage_id",
        )?;
        ensure_nonempty(
            self.model_id.as_str(),
            "reasoning_sft_stage_receipt.model_id",
        )?;
        ensure_nonempty(
            self.pretrain_stage_receipt_digest.as_str(),
            "reasoning_sft_stage_receipt.pretrain_stage_receipt_digest",
        )?;
        ensure_nonempty(
            self.dataset_bundle_digest.as_str(),
            "reasoning_sft_stage_receipt.dataset_bundle_digest",
        )?;
        ensure_nonempty(
            self.general_sft_transition_digest.as_str(),
            "reasoning_sft_stage_receipt.general_sft_transition_digest",
        )?;
        ensure_nonempty(
            self.general_sft_completion_digest.as_str(),
            "reasoning_sft_stage_receipt.general_sft_completion_digest",
        )?;
        self.control_surface
            .validate("reasoning_sft_stage_receipt.control_surface")?;
        ensure_nonempty(self.summary.as_str(), "reasoning_sft_stage_receipt.summary")?;
        if self.style_coverage.is_empty() {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from("reasoning_sft_stage_receipt.style_coverage"),
            });
        }
        for row in &self.style_coverage {
            ensure_nonempty(
                row.style_id.as_str(),
                "reasoning_sft_stage_receipt.style_coverage[].style_id",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "reasoning_sft_stage_receipt.style_coverage[].detail",
            )?;
            validate_bps(
                row.share_bps,
                "reasoning_sft_stage_receipt.style_coverage[].share_bps",
            )?;
        }
        if self.sanity_probe_count == 0 {
            return Err(PsionReasoningSftError::MissingField {
                field: String::from("reasoning_sft_stage_receipt.sanity_probe_count"),
            });
        }
        Ok(())
    }
}

/// One evaluation row in the bounded reasoning-style plurality check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftEvaluationRow {
    /// Stable evaluation case identifier.
    pub case_id: String,
    /// Stable digest over the prompt or case input.
    pub prompt_digest: String,
    /// Style profiles accepted as valid for the case.
    pub accepted_style_profile_ids: Vec<String>,
    /// Style profile observed in the bounded model output.
    pub observed_style_profile_id: String,
    /// Stable digest over the bounded output.
    pub output_digest: String,
    /// Whether explicit assumptions stayed visible.
    pub assumptions_preserved: bool,
    /// Whether uncertainty language stayed visible.
    pub uncertainty_preserved: bool,
    /// Whether normative statements stayed separated from engineering inference.
    pub normative_vs_inference_separated: bool,
    /// Short explanation of the case.
    pub detail: String,
}

impl PsionReasoningSftEvaluationRow {
    fn validate(&self) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.case_id.as_str(),
            "reasoning_sft_evaluation_row.case_id",
        )?;
        ensure_nonempty(
            self.prompt_digest.as_str(),
            "reasoning_sft_evaluation_row.prompt_digest",
        )?;
        ensure_nonempty(
            self.observed_style_profile_id.as_str(),
            "reasoning_sft_evaluation_row.observed_style_profile_id",
        )?;
        ensure_nonempty(
            self.output_digest.as_str(),
            "reasoning_sft_evaluation_row.output_digest",
        )?;
        ensure_nonempty(self.detail.as_str(), "reasoning_sft_evaluation_row.detail")?;
        if self.accepted_style_profile_ids.len() < 2 {
            return Err(PsionReasoningSftError::EvaluationCaseNeedsMultipleStyles {
                case_id: self.case_id.clone(),
            });
        }
        let mut accepted = BTreeSet::new();
        for style_id in &self.accepted_style_profile_ids {
            if !accepted.insert(style_id.as_str()) {
                return Err(PsionReasoningSftError::DuplicateEvaluationAcceptedStyle {
                    case_id: self.case_id.clone(),
                    style_id: style_id.clone(),
                });
            }
        }
        if !accepted.contains(self.observed_style_profile_id.as_str()) {
            return Err(PsionReasoningSftError::ObservedStyleNotAccepted {
                case_id: self.case_id.clone(),
                style_id: self.observed_style_profile_id.clone(),
            });
        }
        Ok(())
    }
}

/// Evaluation receipt for the bounded reasoning-SFT lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftEvaluationReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Bound stage-receipt digest.
    pub stage_receipt_digest: String,
    /// Stable evaluation suite id.
    pub evaluation_suite_id: String,
    /// Evaluation rows.
    pub rows: Vec<PsionReasoningSftEvaluationRow>,
    /// Aggregate multi-style pass rate in basis points.
    pub multiple_valid_style_pass_rate_bps: u32,
    /// Aggregate assumption-retention rate in basis points.
    pub assumption_retention_bps: u32,
    /// Aggregate uncertainty-retention rate in basis points.
    pub uncertainty_retention_bps: u32,
    /// Aggregate normative-vs-inference separation rate in basis points.
    pub normative_vs_inference_separation_bps: u32,
    /// Short explanation of the evaluation result.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionReasoningSftEvaluationReceipt {
    /// Validates the evaluation receipt against the dataset bundle and stage receipt.
    pub fn validate_against_stage(
        &self,
        dataset_bundle: &PsionReasoningSftDatasetBundle,
        stage_receipt: &PsionReasoningSftStageReceipt,
    ) -> Result<(), PsionReasoningSftError> {
        self.validate_shape()?;
        check_string_match(
            self.run_id.as_str(),
            stage_receipt.run_id.as_str(),
            "reasoning_sft_evaluation_receipt.run_id",
        )?;
        check_string_match(
            self.stage_receipt_digest.as_str(),
            stage_receipt.receipt_digest.as_str(),
            "reasoning_sft_evaluation_receipt.stage_receipt_digest",
        )?;

        let known_styles = dataset_bundle
            .style_profiles
            .iter()
            .map(|profile| profile.style_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut observed_styles = BTreeSet::new();
        let mut assumption_preserved = 0_u32;
        let mut uncertainty_preserved = 0_u32;
        let mut separation_preserved = 0_u32;
        let mut multi_style_passes = 0_u32;
        let mut case_ids = BTreeSet::new();
        for row in &self.rows {
            row.validate()?;
            if !case_ids.insert(row.case_id.clone()) {
                return Err(PsionReasoningSftError::DuplicateEvaluationCase {
                    case_id: row.case_id.clone(),
                });
            }
            for style_id in &row.accepted_style_profile_ids {
                if !known_styles.contains(style_id.as_str()) {
                    return Err(PsionReasoningSftError::UnknownStyleProfile {
                        style_id: style_id.clone(),
                    });
                }
            }
            if !known_styles.contains(row.observed_style_profile_id.as_str()) {
                return Err(PsionReasoningSftError::UnknownStyleProfile {
                    style_id: row.observed_style_profile_id.clone(),
                });
            }
            observed_styles.insert(row.observed_style_profile_id.clone());
            multi_style_passes += 1;
            if row.assumptions_preserved {
                assumption_preserved += 1;
            }
            if row.uncertainty_preserved {
                uncertainty_preserved += 1;
            }
            if row.normative_vs_inference_separated {
                separation_preserved += 1;
            }
        }
        if observed_styles.len() != dataset_bundle.style_profiles.len() {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from(
                    "reasoning_sft_evaluation_receipt.rows.observed_style_profile_id",
                ),
                expected: format!(
                    "{} distinct observed styles",
                    dataset_bundle.style_profiles.len()
                ),
                actual: observed_styles.len().to_string(),
            });
        }
        let row_count = self.rows.len() as u32;
        let expected_multi_style_pass_rate = compute_bps(multi_style_passes, row_count)?;
        let expected_assumption_retention = compute_bps(assumption_preserved, row_count)?;
        let expected_uncertainty_retention = compute_bps(uncertainty_preserved, row_count)?;
        let expected_separation = compute_bps(separation_preserved, row_count)?;
        if self.multiple_valid_style_pass_rate_bps != expected_multi_style_pass_rate {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from(
                    "reasoning_sft_evaluation_receipt.multiple_valid_style_pass_rate_bps",
                ),
                expected: expected_multi_style_pass_rate.to_string(),
                actual: self.multiple_valid_style_pass_rate_bps.to_string(),
            });
        }
        if self.assumption_retention_bps != expected_assumption_retention {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from("reasoning_sft_evaluation_receipt.assumption_retention_bps"),
                expected: expected_assumption_retention.to_string(),
                actual: self.assumption_retention_bps.to_string(),
            });
        }
        if self.uncertainty_retention_bps != expected_uncertainty_retention {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from("reasoning_sft_evaluation_receipt.uncertainty_retention_bps"),
                expected: expected_uncertainty_retention.to_string(),
                actual: self.uncertainty_retention_bps.to_string(),
            });
        }
        if self.normative_vs_inference_separation_bps != expected_separation {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from(
                    "reasoning_sft_evaluation_receipt.normative_vs_inference_separation_bps",
                ),
                expected: expected_separation.to_string(),
                actual: self.normative_vs_inference_separation_bps.to_string(),
            });
        }
        if self.multiple_valid_style_pass_rate_bps
            < PSION_REASONING_SFT_MINIMUM_MULTI_STYLE_PASS_RATE_BPS
        {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from(
                    "reasoning_sft_evaluation_receipt.multiple_valid_style_pass_rate_bps",
                ),
                expected: format!(
                    "at least {}",
                    PSION_REASONING_SFT_MINIMUM_MULTI_STYLE_PASS_RATE_BPS
                ),
                actual: self.multiple_valid_style_pass_rate_bps.to_string(),
            });
        }
        if self.assumption_retention_bps < PSION_REASONING_SFT_MINIMUM_ASSUMPTION_RETENTION_BPS {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from("reasoning_sft_evaluation_receipt.assumption_retention_bps"),
                expected: format!(
                    "at least {}",
                    PSION_REASONING_SFT_MINIMUM_ASSUMPTION_RETENTION_BPS
                ),
                actual: self.assumption_retention_bps.to_string(),
            });
        }
        if self.uncertainty_retention_bps < PSION_REASONING_SFT_MINIMUM_UNCERTAINTY_RETENTION_BPS {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from("reasoning_sft_evaluation_receipt.uncertainty_retention_bps"),
                expected: format!(
                    "at least {}",
                    PSION_REASONING_SFT_MINIMUM_UNCERTAINTY_RETENTION_BPS
                ),
                actual: self.uncertainty_retention_bps.to_string(),
            });
        }
        if self.normative_vs_inference_separation_bps
            < PSION_REASONING_SFT_MINIMUM_NORMATIVE_INFERENCE_SEPARATION_BPS
        {
            return Err(PsionReasoningSftError::FieldMismatch {
                field: String::from(
                    "reasoning_sft_evaluation_receipt.normative_vs_inference_separation_bps",
                ),
                expected: format!(
                    "at least {}",
                    PSION_REASONING_SFT_MINIMUM_NORMATIVE_INFERENCE_SEPARATION_BPS
                ),
                actual: self.normative_vs_inference_separation_bps.to_string(),
            });
        }
        if self.receipt_digest != stable_reasoning_sft_evaluation_receipt_digest(self) {
            return Err(PsionReasoningSftError::DigestMismatch {
                kind: String::from("reasoning_sft_evaluation_receipt"),
            });
        }
        Ok(())
    }

    fn validate_shape(&self) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "reasoning_sft_evaluation_receipt.schema_version",
        )?;
        if self.schema_version != PSION_REASONING_SFT_EVALUATION_RECEIPT_SCHEMA_VERSION {
            return Err(PsionReasoningSftError::SchemaVersionMismatch {
                expected: String::from(PSION_REASONING_SFT_EVALUATION_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "reasoning_sft_evaluation_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "reasoning_sft_evaluation_receipt.run_id",
        )?;
        ensure_nonempty(
            self.stage_receipt_digest.as_str(),
            "reasoning_sft_evaluation_receipt.stage_receipt_digest",
        )?;
        ensure_nonempty(
            self.evaluation_suite_id.as_str(),
            "reasoning_sft_evaluation_receipt.evaluation_suite_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "reasoning_sft_evaluation_receipt.summary",
        )?;
        if self.rows.len() < PSION_REASONING_SFT_MINIMUM_STYLE_PROFILES {
            return Err(PsionReasoningSftError::CountMismatch {
                field: String::from("reasoning_sft_evaluation_receipt.rows"),
                expected: PSION_REASONING_SFT_MINIMUM_STYLE_PROFILES,
                actual: self.rows.len(),
            });
        }
        validate_bps(
            self.multiple_valid_style_pass_rate_bps,
            "reasoning_sft_evaluation_receipt.multiple_valid_style_pass_rate_bps",
        )?;
        validate_bps(
            self.assumption_retention_bps,
            "reasoning_sft_evaluation_receipt.assumption_retention_bps",
        )?;
        validate_bps(
            self.uncertainty_retention_bps,
            "reasoning_sft_evaluation_receipt.uncertainty_retention_bps",
        )?;
        validate_bps(
            self.normative_vs_inference_separation_bps,
            "reasoning_sft_evaluation_receipt.normative_vs_inference_separation_bps",
        )?;
        Ok(())
    }
}

/// Full run bundle for the first bounded reasoning-SFT lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReasoningSftRunBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable model id.
    pub model_id: String,
    /// Bound lifecycle schema version.
    pub lifecycle_schema_version: String,
    /// Bound exclusion schema version.
    pub exclusion_schema_version: String,
    /// Bound artifact-lineage schema version.
    pub artifact_lineage_schema_version: String,
    /// Stage-program state carrying pretrain-to-general-SFT lineage.
    pub stage_program: TrainingStageProgramState,
    /// Reasoning-SFT dataset bundle.
    pub dataset_bundle: PsionReasoningSftDatasetBundle,
    /// Reasoning-SFT stage receipt.
    pub stage_receipt: PsionReasoningSftStageReceipt,
    /// Reasoning-SFT evaluation receipt.
    pub evaluation_receipt: PsionReasoningSftEvaluationReceipt,
    /// Short explanation of the full run.
    pub summary: String,
    /// Stable digest over the full bundle.
    pub bundle_digest: String,
}

impl PsionReasoningSftRunBundle {
    /// Validates the full bundle against the current Psion data-governance state.
    pub fn validate_against_context(
        &self,
        lifecycle: &PsionSourceLifecycleManifest,
        exclusion: &PsionExclusionManifest,
        artifact_lineage: &PsionArtifactLineageManifest,
    ) -> Result<(), PsionReasoningSftError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "reasoning_sft_run_bundle.schema_version",
        )?;
        if self.schema_version != PSION_REASONING_SFT_RUN_BUNDLE_SCHEMA_VERSION {
            return Err(PsionReasoningSftError::SchemaVersionMismatch {
                expected: String::from(PSION_REASONING_SFT_RUN_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.bundle_id.as_str(),
            "reasoning_sft_run_bundle.bundle_id",
        )?;
        ensure_nonempty(self.run_id.as_str(), "reasoning_sft_run_bundle.run_id")?;
        ensure_nonempty(self.model_id.as_str(), "reasoning_sft_run_bundle.model_id")?;
        ensure_nonempty(
            self.lifecycle_schema_version.as_str(),
            "reasoning_sft_run_bundle.lifecycle_schema_version",
        )?;
        ensure_nonempty(
            self.exclusion_schema_version.as_str(),
            "reasoning_sft_run_bundle.exclusion_schema_version",
        )?;
        ensure_nonempty(
            self.artifact_lineage_schema_version.as_str(),
            "reasoning_sft_run_bundle.artifact_lineage_schema_version",
        )?;
        ensure_nonempty(self.summary.as_str(), "reasoning_sft_run_bundle.summary")?;
        check_string_match(
            self.lifecycle_schema_version.as_str(),
            lifecycle.schema_version.as_str(),
            "reasoning_sft_run_bundle.lifecycle_schema_version",
        )?;
        check_string_match(
            self.exclusion_schema_version.as_str(),
            exclusion.schema_version.as_str(),
            "reasoning_sft_run_bundle.exclusion_schema_version",
        )?;
        check_string_match(
            self.artifact_lineage_schema_version.as_str(),
            artifact_lineage.schema_version.as_str(),
            "reasoning_sft_run_bundle.artifact_lineage_schema_version",
        )?;
        check_string_match(
            self.run_id.as_str(),
            self.stage_program.run_id.as_str(),
            "reasoning_sft_run_bundle.run_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            self.dataset_bundle.run_id.as_str(),
            "reasoning_sft_run_bundle.run_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            self.stage_receipt.run_id.as_str(),
            "reasoning_sft_run_bundle.run_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            self.evaluation_receipt.run_id.as_str(),
            "reasoning_sft_run_bundle.run_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            self.dataset_bundle.model_id.as_str(),
            "reasoning_sft_run_bundle.model_id",
        )?;
        check_string_match(
            self.model_id.as_str(),
            self.stage_receipt.model_id.as_str(),
            "reasoning_sft_run_bundle.model_id",
        )?;
        self.dataset_bundle.validate_against_context(
            &self.stage_program,
            lifecycle,
            exclusion,
            artifact_lineage,
        )?;
        self.stage_receipt
            .validate_against_stage(&self.stage_program, &self.dataset_bundle)?;
        self.evaluation_receipt
            .validate_against_stage(&self.dataset_bundle, &self.stage_receipt)?;
        if self.bundle_digest != stable_reasoning_sft_run_bundle_digest(self) {
            return Err(PsionReasoningSftError::DigestMismatch {
                kind: String::from("reasoning_sft_run_bundle"),
            });
        }
        Ok(())
    }
}

/// Records the bounded reasoning-SFT dataset bundle on top of the stage program.
pub fn record_psion_reasoning_sft_dataset_bundle(
    stage_program: &TrainingStageProgramState,
    sft_artifact_lineage: PsionSftArtifactLineage,
    control_surface: PsionReasoningSftControlSurface,
    style_profiles: Vec<PsionReasoningSftStyleProfile>,
    trace_bindings: Vec<PsionReasoningSftTraceBinding>,
    summary: impl Into<String>,
) -> Result<PsionReasoningSftDatasetBundle, PsionReasoningSftError> {
    let stage_context = reasoning_stage_context(stage_program)?;
    let summary = summary.into();
    let mut bundle = PsionReasoningSftDatasetBundle {
        schema_version: String::from(PSION_REASONING_SFT_DATASET_BUNDLE_SCHEMA_VERSION),
        run_id: stage_context.stage_program.run_id.clone(),
        stage_id: stage_context.general_sft_stage.stage_id.clone(),
        model_id: stage_context.pretrain_receipt.model_id.clone(),
        pretrain_stage_receipt_digest: stage_context.pretrain_receipt.receipt_digest.clone(),
        sft_artifact_lineage,
        control_surface,
        style_profiles,
        trace_bindings,
        summary,
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_reasoning_sft_dataset_bundle_digest(&bundle);
    bundle.validate_shape()?;
    Ok(bundle)
}

/// Records the bounded reasoning-SFT stage receipt.
pub fn record_psion_reasoning_sft_stage_receipt(
    receipt_id: impl Into<String>,
    stage_program: &TrainingStageProgramState,
    dataset_bundle: &PsionReasoningSftDatasetBundle,
    sanity_probe_count: u32,
    assumption_retained_probe_count: u32,
    uncertainty_retained_probe_count: u32,
    normative_vs_inference_separated_probe_count: u32,
    summary: impl Into<String>,
) -> Result<PsionReasoningSftStageReceipt, PsionReasoningSftError> {
    let stage_context = reasoning_stage_context(stage_program)?;
    let style_coverage = expected_style_coverage(dataset_bundle)?;
    let mut receipt = PsionReasoningSftStageReceipt {
        schema_version: String::from(PSION_REASONING_SFT_STAGE_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        run_id: stage_context.stage_program.run_id.clone(),
        stage_id: stage_context.general_sft_stage.stage_id.clone(),
        model_id: dataset_bundle.model_id.clone(),
        pretrain_stage_receipt_digest: dataset_bundle.pretrain_stage_receipt_digest.clone(),
        dataset_bundle_digest: dataset_bundle.bundle_digest.clone(),
        general_sft_transition_digest: stage_context
            .general_sft_transition
            .transition_digest
            .clone(),
        general_sft_completion_digest: stage_context
            .general_sft_completion
            .completion_digest
            .clone(),
        control_surface: dataset_bundle.control_surface.clone(),
        style_coverage,
        sanity_probe_count,
        assumption_retained_probe_count,
        uncertainty_retained_probe_count,
        normative_vs_inference_separated_probe_count,
        assumption_retention_bps: compute_bps(assumption_retained_probe_count, sanity_probe_count)?,
        uncertainty_retention_bps: compute_bps(
            uncertainty_retained_probe_count,
            sanity_probe_count,
        )?,
        normative_vs_inference_separation_bps: compute_bps(
            normative_vs_inference_separated_probe_count,
            sanity_probe_count,
        )?,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_reasoning_sft_stage_receipt_digest(&receipt);
    receipt.validate_against_stage(stage_program, dataset_bundle)?;
    Ok(receipt)
}

/// Records the bounded reasoning-style plurality evaluation receipt.
pub fn record_psion_reasoning_sft_evaluation_receipt(
    receipt_id: impl Into<String>,
    evaluation_suite_id: impl Into<String>,
    dataset_bundle: &PsionReasoningSftDatasetBundle,
    stage_receipt: &PsionReasoningSftStageReceipt,
    rows: Vec<PsionReasoningSftEvaluationRow>,
    summary: impl Into<String>,
) -> Result<PsionReasoningSftEvaluationReceipt, PsionReasoningSftError> {
    let total_rows = rows.len() as u32;
    let assumption_retained = rows.iter().filter(|row| row.assumptions_preserved).count() as u32;
    let uncertainty_retained = rows.iter().filter(|row| row.uncertainty_preserved).count() as u32;
    let separation_retained = rows
        .iter()
        .filter(|row| row.normative_vs_inference_separated)
        .count() as u32;
    let multi_style_passes = rows.len() as u32;
    let mut receipt = PsionReasoningSftEvaluationReceipt {
        schema_version: String::from(PSION_REASONING_SFT_EVALUATION_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        run_id: stage_receipt.run_id.clone(),
        stage_receipt_digest: stage_receipt.receipt_digest.clone(),
        evaluation_suite_id: evaluation_suite_id.into(),
        rows,
        multiple_valid_style_pass_rate_bps: compute_bps(multi_style_passes, total_rows)?,
        assumption_retention_bps: compute_bps(assumption_retained, total_rows)?,
        uncertainty_retention_bps: compute_bps(uncertainty_retained, total_rows)?,
        normative_vs_inference_separation_bps: compute_bps(separation_retained, total_rows)?,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_reasoning_sft_evaluation_receipt_digest(&receipt);
    receipt.validate_against_stage(dataset_bundle, stage_receipt)?;
    Ok(receipt)
}

/// Records the full bounded reasoning-SFT run bundle.
pub fn record_psion_reasoning_sft_run_bundle(
    bundle_id: impl Into<String>,
    lifecycle: &PsionSourceLifecycleManifest,
    exclusion: &PsionExclusionManifest,
    artifact_lineage: &PsionArtifactLineageManifest,
    stage_program: TrainingStageProgramState,
    dataset_bundle: PsionReasoningSftDatasetBundle,
    stage_receipt: PsionReasoningSftStageReceipt,
    evaluation_receipt: PsionReasoningSftEvaluationReceipt,
    summary: impl Into<String>,
) -> Result<PsionReasoningSftRunBundle, PsionReasoningSftError> {
    let mut bundle = PsionReasoningSftRunBundle {
        schema_version: String::from(PSION_REASONING_SFT_RUN_BUNDLE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        run_id: stage_program.run_id.clone(),
        model_id: dataset_bundle.model_id.clone(),
        lifecycle_schema_version: lifecycle.schema_version.clone(),
        exclusion_schema_version: exclusion.schema_version.clone(),
        artifact_lineage_schema_version: artifact_lineage.schema_version.clone(),
        stage_program,
        dataset_bundle,
        stage_receipt,
        evaluation_receipt,
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_reasoning_sft_run_bundle_digest(&bundle);
    bundle.validate_against_context(lifecycle, exclusion, artifact_lineage)?;
    Ok(bundle)
}

#[derive(Clone)]
struct ReasoningStageContext<'a> {
    stage_program: &'a TrainingStageProgramState,
    pretrain_receipt: &'a PsionPretrainStageRunReceipt,
    #[allow(dead_code)]
    pretrain_stage: &'a TrainingStageState,
    #[allow(dead_code)]
    pretrain_completion: &'a TrainingStageCompletionReceipt,
    #[allow(dead_code)]
    pretrain_to_general_sft_promotion: &'a TrainingCheckpointPromotionReceipt,
    general_sft_stage: &'a TrainingStageState,
    general_sft_transition: &'a TrainingStageTransitionReceipt,
    general_sft_completion: &'a TrainingStageCompletionReceipt,
}

fn reasoning_stage_context(
    stage_program: &TrainingStageProgramState,
) -> Result<ReasoningStageContext<'_>, PsionReasoningSftError> {
    if stage_program.stages.len() != 2 {
        return Err(PsionReasoningSftError::UnexpectedStageGraph {
            detail: format!(
                "expected exactly 2 stages (pretrain -> general_sft), found {}",
                stage_program.stages.len()
            ),
        });
    }
    let pretrain_stage = &stage_program.stages[0];
    if pretrain_stage.kind != TrainingStageKind::Pretrain {
        return Err(PsionReasoningSftError::UnexpectedStageGraph {
            detail: format!(
                "expected first stage to be pretrain, found {:?}",
                pretrain_stage.kind
            ),
        });
    }
    let general_sft_stage = &stage_program.stages[1];
    if general_sft_stage.kind != TrainingStageKind::GeneralSft {
        return Err(PsionReasoningSftError::UnexpectedStageGraph {
            detail: format!(
                "expected second stage to be general_sft, found {:?}",
                general_sft_stage.kind
            ),
        });
    }
    let pretrain_receipt = pretrain_stage.pretrain_runs.first().ok_or_else(|| {
        PsionReasoningSftError::UnexpectedStageGraph {
            detail: String::from("pretrain stage is missing its bound Psion pretrain receipt"),
        }
    })?;
    let pretrain_completion = stage_program
        .completions
        .iter()
        .find(|completion| completion.stage_id == pretrain_stage.stage_id)
        .ok_or_else(|| PsionReasoningSftError::UnexpectedStageGraph {
            detail: String::from("pretrain stage is missing its completion receipt"),
        })?;
    let pretrain_to_general_sft_promotion = stage_program
        .promotions
        .iter()
        .find(|promotion| {
            promotion.from_stage_id == pretrain_stage.stage_id
                && promotion.to_stage_kind == TrainingStageKind::GeneralSft
        })
        .ok_or_else(|| PsionReasoningSftError::UnexpectedStageGraph {
            detail: String::from("pretrain stage is missing the promotion into general_sft"),
        })?;
    let general_sft_transition = stage_program
        .transitions
        .iter()
        .find(|transition| transition.next_stage_id == general_sft_stage.stage_id)
        .ok_or_else(|| PsionReasoningSftError::UnexpectedStageGraph {
            detail: String::from("general_sft stage is missing its transition receipt"),
        })?;
    let general_sft_completion = stage_program
        .completions
        .iter()
        .find(|completion| completion.stage_id == general_sft_stage.stage_id)
        .ok_or_else(|| PsionReasoningSftError::UnexpectedStageGraph {
            detail: String::from("general_sft stage is missing its completion receipt"),
        })?;
    Ok(ReasoningStageContext {
        stage_program,
        pretrain_receipt,
        pretrain_stage,
        pretrain_completion,
        pretrain_to_general_sft_promotion,
        general_sft_stage,
        general_sft_transition,
        general_sft_completion,
    })
}

fn expected_style_coverage(
    dataset_bundle: &PsionReasoningSftDatasetBundle,
) -> Result<Vec<PsionReasoningSftStyleCoverageRow>, PsionReasoningSftError> {
    let mut counts = dataset_bundle
        .style_profiles
        .iter()
        .map(|profile| (profile.style_id.as_str(), 0_u32))
        .collect::<BTreeMap<_, _>>();
    for binding in &dataset_bundle.trace_bindings {
        *counts
            .get_mut(binding.style_profile_id.as_str())
            .ok_or_else(|| PsionReasoningSftError::UnknownStyleProfile {
                style_id: binding.style_profile_id.clone(),
            })? += 1;
    }
    let total = dataset_bundle.trace_bindings.len() as u32;
    let mut remaining_share = 10_000_u32;
    let mut rows = Vec::with_capacity(dataset_bundle.style_profiles.len());
    for (idx, profile) in dataset_bundle.style_profiles.iter().enumerate() {
        let trace_count = *counts.get(profile.style_id.as_str()).unwrap_or(&0);
        let share_bps = if idx + 1 == dataset_bundle.style_profiles.len() {
            remaining_share
        } else {
            let share_bps = compute_bps(trace_count, total)?;
            remaining_share = remaining_share.saturating_sub(share_bps);
            share_bps
        };
        rows.push(PsionReasoningSftStyleCoverageRow {
            style_id: profile.style_id.clone(),
            trace_count,
            share_bps,
            detail: format!(
                "Style `{}` preserves one valid {} / {} / {} reasoning narration path inside the bounded SFT lane.",
                profile.style_id,
                decomposition_strategy_label(profile.decomposition_strategy),
                explanation_order_label(profile.explanation_order),
                abstraction_level_label(profile.abstraction_level),
            ),
        });
    }
    Ok(rows)
}

fn validate_retention_metric(
    total: u32,
    retained: u32,
    observed_bps: u32,
    minimum_bps: u32,
    field: &str,
) -> Result<(), PsionReasoningSftError> {
    if retained > total {
        return Err(PsionReasoningSftError::FieldMismatch {
            field: format!("{field}.count"),
            expected: format!("at most {total}"),
            actual: retained.to_string(),
        });
    }
    let expected_bps = compute_bps(retained, total)?;
    if observed_bps != expected_bps {
        return Err(PsionReasoningSftError::FieldMismatch {
            field: String::from(field),
            expected: expected_bps.to_string(),
            actual: observed_bps.to_string(),
        });
    }
    if observed_bps < minimum_bps {
        return Err(PsionReasoningSftError::FieldMismatch {
            field: String::from(field),
            expected: format!("at least {minimum_bps}"),
            actual: observed_bps.to_string(),
        });
    }
    Ok(())
}

fn decomposition_strategy_label(strategy: PsionReasoningSftDecompositionStrategy) -> &'static str {
    match strategy {
        PsionReasoningSftDecompositionStrategy::AssumptionsThenMechanism => {
            "assumptions_then_mechanism"
        }
        PsionReasoningSftDecompositionStrategy::AnswerThenEvidence => "answer_then_evidence",
        PsionReasoningSftDecompositionStrategy::ConstraintsThenTradeoffs => {
            "constraints_then_tradeoffs"
        }
    }
}

fn explanation_order_label(order: PsionReasoningSftExplanationOrder) -> &'static str {
    match order {
        PsionReasoningSftExplanationOrder::PremisesThenConclusion => "premises_then_conclusion",
        PsionReasoningSftExplanationOrder::ConclusionThenPremises => "conclusion_then_premises",
        PsionReasoningSftExplanationOrder::TopDownThenConcrete => "top_down_then_concrete",
    }
}

fn abstraction_level_label(level: PsionReasoningSftAbstractionLevel) -> &'static str {
    match level {
        PsionReasoningSftAbstractionLevel::Concrete => "concrete",
        PsionReasoningSftAbstractionLevel::Hybrid => "hybrid",
        PsionReasoningSftAbstractionLevel::Conceptual => "conceptual",
    }
}

fn stable_reasoning_sft_dataset_bundle_digest(bundle: &PsionReasoningSftDatasetBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_reasoning_sft_dataset_bundle|");
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.pretrain_stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.sft_artifact_lineage.artifact_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.sft_artifact_lineage.artifact_digest.as_bytes());
    for corpus_id in &bundle.sft_artifact_lineage.tokenized_corpus_ids {
        hasher.update(b"|corpus|");
        hasher.update(corpus_id.as_bytes());
    }
    for source_id in &bundle.sft_artifact_lineage.source_ids {
        hasher.update(b"|source|");
        hasher.update(source_id.as_bytes());
    }
    hasher.update(b"|controls|");
    hasher.update(bundle.control_surface.detail.as_bytes());
    hasher.update(b"|");
    hasher.update(
        u8::from(bundle.control_surface.explicit_assumptions_required)
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        u8::from(
            bundle
                .control_surface
                .explicit_uncertainty_language_required,
        )
        .to_string()
        .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        u8::from(
            bundle
                .control_surface
                .normative_vs_inference_separation_required,
        )
        .to_string()
        .as_bytes(),
    );
    for profile in &bundle.style_profiles {
        hasher.update(b"|style|");
        hasher.update(profile.style_id.as_bytes());
        hasher.update(b"|");
        hasher.update(decomposition_strategy_label(profile.decomposition_strategy).as_bytes());
        hasher.update(b"|");
        hasher.update(explanation_order_label(profile.explanation_order).as_bytes());
        hasher.update(b"|");
        hasher.update(abstraction_level_label(profile.abstraction_level).as_bytes());
        hasher.update(b"|");
        hasher.update(profile.detail.as_bytes());
    }
    for binding in &bundle.trace_bindings {
        hasher.update(b"|trace|");
        hasher.update(binding.trace_id.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", binding.trace_kind).as_bytes());
        hasher.update(b"|");
        hasher.update(binding.trace_lineage_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(binding.style_profile_id.as_bytes());
        for source_id in &binding.parent_source_ids {
            hasher.update(b"|binding_source|");
            hasher.update(source_id.as_bytes());
        }
        for corpus_id in &binding.parent_tokenized_corpus_ids {
            hasher.update(b"|binding_corpus|");
            hasher.update(corpus_id.as_bytes());
        }
        hasher.update(b"|");
        hasher.update(
            u8::from(binding.derived_from_parent_sources)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            u8::from(binding.held_out_exclusion_checked)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            u8::from(binding.explicit_assumptions_present)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            u8::from(binding.explicit_uncertainty_language_present)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            u8::from(binding.normative_vs_inference_separated)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(binding.detail.as_bytes());
    }
    hasher.update(b"|summary|");
    hasher.update(bundle.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_reasoning_sft_stage_receipt_digest(receipt: &PsionReasoningSftStageReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_reasoning_sft_stage_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.pretrain_stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.dataset_bundle_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.general_sft_transition_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.general_sft_completion_digest.as_bytes());
    hasher.update(b"|controls|");
    hasher.update(receipt.control_surface.detail.as_bytes());
    for row in &receipt.style_coverage {
        hasher.update(b"|style|");
        hasher.update(row.style_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.trace_count.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.share_bps.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.detail.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(receipt.sanity_probe_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .assumption_retained_probe_count
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .uncertainty_retained_probe_count
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(
        receipt
            .normative_vs_inference_separated_probe_count
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(receipt.assumption_retention_bps.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.uncertainty_retention_bps.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .normative_vs_inference_separation_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|summary|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_reasoning_sft_evaluation_receipt_digest(
    receipt: &PsionReasoningSftEvaluationReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_reasoning_sft_evaluation_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.evaluation_suite_id.as_bytes());
    for row in &receipt.rows {
        hasher.update(b"|case|");
        hasher.update(row.case_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.prompt_digest.as_bytes());
        for style_id in &row.accepted_style_profile_ids {
            hasher.update(b"|accepted_style|");
            hasher.update(style_id.as_bytes());
        }
        hasher.update(b"|");
        hasher.update(row.observed_style_profile_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.output_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(u8::from(row.assumptions_preserved).to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(u8::from(row.uncertainty_preserved).to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(
            u8::from(row.normative_vs_inference_separated)
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(row.detail.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(
        receipt
            .multiple_valid_style_pass_rate_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|");
    hasher.update(receipt.assumption_retention_bps.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.uncertainty_retention_bps.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .normative_vs_inference_separation_bps
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|summary|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_reasoning_sft_run_bundle_digest(bundle: &PsionReasoningSftRunBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_reasoning_sft_run_bundle|");
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.lifecycle_schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.exclusion_schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.artifact_lineage_schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.dataset_bundle.bundle_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.stage_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.evaluation_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionReasoningSftError> {
    if value.trim().is_empty() {
        return Err(PsionReasoningSftError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_bool_true(value: bool, field: &str) -> Result<(), PsionReasoningSftError> {
    if !value {
        return Err(PsionReasoningSftError::FieldMismatch {
            field: String::from(field),
            expected: String::from("true"),
            actual: String::from("false"),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionReasoningSftError> {
    if actual != expected {
        return Err(PsionReasoningSftError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn validate_bps(value: u32, field: &str) -> Result<(), PsionReasoningSftError> {
    if value > 10_000 {
        return Err(PsionReasoningSftError::FieldMismatch {
            field: String::from(field),
            expected: String::from("0..=10000"),
            actual: value.to_string(),
        });
    }
    Ok(())
}

fn compute_bps(numerator: u32, denominator: u32) -> Result<u32, PsionReasoningSftError> {
    if denominator == 0 {
        return Err(PsionReasoningSftError::MissingField {
            field: String::from("bps_denominator"),
        });
    }
    Ok(((numerator as u64 * 10_000) / denominator as u64) as u32)
}

/// Validation failures for the bounded reasoning-SFT lane.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionReasoningSftError {
    /// A required field was missing or empty.
    #[error("Psion reasoning SFT is missing `{field}`")]
    MissingField {
        /// Missing field.
        field: String,
    },
    /// One field drifted from the required value.
    #[error(
        "Psion reasoning SFT field mismatch on `{field}`: expected `{expected}`, found `{actual}`"
    )]
    FieldMismatch {
        /// Mismatched field.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// One schema version drifted.
    #[error("Psion reasoning SFT schema mismatch: expected `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// The reasoning stage graph drifted from the bounded pretrain-to-general-SFT path.
    #[error(
        "Psion reasoning SFT requires the bounded pretrain -> general_sft stage graph: {detail}"
    )]
    UnexpectedStageGraph {
        /// Short explanation of the drift.
        detail: String,
    },
    /// One style profile was duplicated.
    #[error("Psion reasoning SFT repeats style profile `{style_id}`")]
    DuplicateStyleProfile {
        /// Style id.
        style_id: String,
    },
    /// One style profile was declared but never used.
    #[error("Psion reasoning SFT style profile `{style_id}` is declared but unused")]
    UnusedStyleProfile {
        /// Style id.
        style_id: String,
    },
    /// The reasoning-SFT lane declared too few styles.
    #[error("Psion reasoning SFT requires at least {expected} style profiles, found {actual}")]
    StyleProfileCountTooSmall {
        /// Expected style count.
        expected: usize,
        /// Actual style count.
        actual: usize,
    },
    /// One trace binding was duplicated.
    #[error("Psion reasoning SFT repeats trace binding `{trace_id}`")]
    DuplicateTraceBinding {
        /// Trace id.
        trace_id: String,
    },
    /// One trace-lineage digest was duplicated.
    #[error("Psion reasoning SFT repeats trace-lineage digest `{trace_lineage_digest}`")]
    DuplicateTraceLineageDigest {
        /// Duplicated digest.
        trace_lineage_digest: String,
    },
    /// One style id was unknown.
    #[error("Psion reasoning SFT references unknown style profile `{style_id}`")]
    UnknownStyleProfile {
        /// Unknown style id.
        style_id: String,
    },
    /// One trace referenced a source outside the SFT lineage row.
    #[error("Psion reasoning SFT trace `{trace_id}` references source `{source_id}` outside the SFT artifact lineage")]
    UnknownTraceSourceInSftLineage {
        /// Trace id.
        trace_id: String,
        /// Unknown source id.
        source_id: String,
    },
    /// One trace referenced a corpus outside the SFT lineage row.
    #[error("Psion reasoning SFT trace `{trace_id}` references corpus `{corpus_id}` outside the SFT artifact lineage")]
    UnknownTraceCorpusInSftLineage {
        /// Trace id.
        trace_id: String,
        /// Unknown corpus id.
        corpus_id: String,
    },
    /// The stage program omitted one bound trace.
    #[error("Psion reasoning SFT dataset references trace `{trace_id}` that the stage program did not ingest")]
    UnknownStageTrace {
        /// Trace id.
        trace_id: String,
    },
    /// The artifact-lineage manifest omitted the bound SFT artifact.
    #[error("Psion reasoning SFT could not find SFT artifact lineage `{artifact_id}` in the artifact-lineage manifest")]
    UnknownSftArtifactLineage {
        /// Artifact id.
        artifact_id: String,
    },
    /// The embedded SFT artifact-lineage row drifted from the canonical manifest.
    #[error("Psion reasoning SFT artifact lineage drifted from the canonical manifest for `{artifact_id}`")]
    SftArtifactLineageMismatch {
        /// Artifact id.
        artifact_id: String,
    },
    /// One count drifted from the required count.
    #[error(
        "Psion reasoning SFT count mismatch on `{field}`: expected {expected}, found {actual}"
    )]
    CountMismatch {
        /// Mismatched field.
        field: String,
        /// Expected count.
        expected: usize,
        /// Actual count.
        actual: usize,
    },
    /// The lane collapsed onto one dominant style.
    #[error("Psion reasoning SFT style collapse detected: observed {observed_share_bps} bps, maximum allowed {maximum_allowed_bps} bps")]
    StyleCollapse {
        /// Observed dominant-style share.
        observed_share_bps: u32,
        /// Maximum allowed share.
        maximum_allowed_bps: u32,
    },
    /// One evaluation case admitted only one style.
    #[error("Psion reasoning SFT evaluation case `{case_id}` must admit multiple valid styles")]
    EvaluationCaseNeedsMultipleStyles {
        /// Case id.
        case_id: String,
    },
    /// One evaluation case repeated an accepted style id.
    #[error("Psion reasoning SFT evaluation case `{case_id}` repeats accepted style `{style_id}`")]
    DuplicateEvaluationAcceptedStyle {
        /// Case id.
        case_id: String,
        /// Style id.
        style_id: String,
    },
    /// One observed style was not in the accepted set.
    #[error("Psion reasoning SFT evaluation case `{case_id}` observed style `{style_id}` outside the accepted style set")]
    ObservedStyleNotAccepted {
        /// Case id.
        case_id: String,
        /// Style id.
        style_id: String,
    },
    /// One evaluation case id was duplicated.
    #[error("Psion reasoning SFT evaluation repeats case `{case_id}`")]
    DuplicateEvaluationCase {
        /// Case id.
        case_id: String,
    },
    /// One digest drifted from the computed value.
    #[error("Psion reasoning SFT digest drifted for `{kind}`")]
    DigestMismatch {
        /// Digest-bearing artifact kind.
        kind: String,
    },
    /// Lifecycle validation failed.
    #[error(transparent)]
    SourceLifecycle(#[from] PsionSourceLifecycleError),
    /// Benchmark-isolation validation failed.
    #[error(transparent)]
    BenchmarkIsolation(#[from] PsionBenchmarkIsolationError),
}

#[cfg(test)]
mod tests {
    use super::{
        record_psion_reasoning_sft_dataset_bundle, record_psion_reasoning_sft_evaluation_receipt,
        record_psion_reasoning_sft_run_bundle, record_psion_reasoning_sft_stage_receipt,
        PsionReasoningSftAbstractionLevel, PsionReasoningSftControlSurface,
        PsionReasoningSftDecompositionStrategy, PsionReasoningSftError,
        PsionReasoningSftEvaluationRow, PsionReasoningSftExplanationOrder,
        PsionReasoningSftStyleProfile, PsionReasoningSftTraceBinding,
    };
    use crate::{
        TrainingLongContextTraceLineage, TrainingSftTraceArtifact, TrainingSftTraceKind,
        TrainingStageProgramState,
    };
    use psionic_data::{
        PsionArtifactLineageManifest, PsionExclusionManifest, PsionSourceLifecycleManifest,
    };
    use psionic_environments::EnvironmentPackageKey;

    fn lifecycle_manifest() -> PsionSourceLifecycleManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"
        ))
        .expect("lifecycle manifest should parse")
    }

    fn exclusion_manifest() -> PsionExclusionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/isolation/psion_exclusion_manifest_v1.json"
        ))
        .expect("exclusion manifest should parse")
    }

    fn artifact_lineage_manifest() -> PsionArtifactLineageManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"
        ))
        .expect("artifact-lineage manifest should parse")
    }

    fn pretrain_receipt() -> crate::PsionPretrainStageRunReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"
        ))
        .expect("pretrain receipt should parse")
    }

    fn style_profiles() -> Vec<PsionReasoningSftStyleProfile> {
        vec![
            PsionReasoningSftStyleProfile {
                style_id: String::from("assumptions_then_mechanism_concrete"),
                decomposition_strategy:
                    PsionReasoningSftDecompositionStrategy::AssumptionsThenMechanism,
                explanation_order: PsionReasoningSftExplanationOrder::PremisesThenConclusion,
                abstraction_level: PsionReasoningSftAbstractionLevel::Concrete,
                detail: String::from(
                    "Surface the bounded assumptions first, then explain the concrete mechanism.",
                ),
            },
            PsionReasoningSftStyleProfile {
                style_id: String::from("answer_then_evidence_hybrid"),
                decomposition_strategy: PsionReasoningSftDecompositionStrategy::AnswerThenEvidence,
                explanation_order: PsionReasoningSftExplanationOrder::ConclusionThenPremises,
                abstraction_level: PsionReasoningSftAbstractionLevel::Hybrid,
                detail: String::from(
                    "State the bounded answer first, then mix conceptual and concrete support.",
                ),
            },
            PsionReasoningSftStyleProfile {
                style_id: String::from("constraints_then_tradeoffs_conceptual"),
                decomposition_strategy:
                    PsionReasoningSftDecompositionStrategy::ConstraintsThenTradeoffs,
                explanation_order: PsionReasoningSftExplanationOrder::TopDownThenConcrete,
                abstraction_level: PsionReasoningSftAbstractionLevel::Conceptual,
                detail: String::from(
                    "Frame the constraints first, then discuss tradeoffs from the conceptual level down.",
                ),
            },
        ]
    }

    fn build_stage_program_and_dataset() -> Result<
        (
            TrainingStageProgramState,
            super::PsionReasoningSftDatasetBundle,
            super::PsionReasoningSftStageReceipt,
            super::PsionReasoningSftEvaluationReceipt,
        ),
        Box<dyn std::error::Error>,
    > {
        let pretrain_receipt = pretrain_receipt();
        let mut stage_program = TrainingStageProgramState::new(
            pretrain_receipt.run_id.clone(),
            pretrain_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .checkpoint_family
                .clone(),
        )?;
        stage_program.start_initial_pretrain_stage(EnvironmentPackageKey::new(
            "env.psion.pretrain",
            "2026.03.22",
        ))?;
        stage_program.record_psion_pretrain_run(&pretrain_receipt)?;
        stage_program.complete_current_stage()?;
        stage_program.advance_stage(
            crate::TrainingStageKind::GeneralSft,
            EnvironmentPackageKey::new("env.psion.reasoning", "2026.03.22"),
            pretrain_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .clone(),
        )?;

        let traces = vec![
            TrainingSftTraceArtifact::new(
                "psion-reasoning-trace-1",
                EnvironmentPackageKey::new("env.psion.reasoning", "2026.03.22"),
                TrainingSftTraceKind::PlainCompletion,
                "prompt-digest-1",
                "output-digest-1",
            )
            .with_source_ref("psion_reasoning_sft_seed_v1"),
            TrainingSftTraceArtifact::new(
                "psion-reasoning-trace-2",
                EnvironmentPackageKey::new("env.psion.reasoning", "2026.03.22"),
                TrainingSftTraceKind::LongContext,
                "prompt-digest-2",
                "output-digest-2",
            )
            .with_source_ref("psion_reasoning_sft_seed_v1")
            .with_long_context_lineage(TrainingLongContextTraceLineage::new(
                4096,
                vec![
                    String::from("segment-wasm-spec-1"),
                    String::from("segment-wasm-spec-2"),
                ],
            )),
            TrainingSftTraceArtifact::new(
                "psion-reasoning-trace-3",
                EnvironmentPackageKey::new("env.psion.reasoning", "2026.03.22"),
                TrainingSftTraceKind::PlainCompletion,
                "prompt-digest-3",
                "output-digest-3",
            )
            .with_source_ref("psion_reasoning_sft_seed_v1"),
            TrainingSftTraceArtifact::new(
                "psion-reasoning-trace-4",
                EnvironmentPackageKey::new("env.psion.reasoning", "2026.03.22"),
                TrainingSftTraceKind::LongContext,
                "prompt-digest-4",
                "output-digest-4",
            )
            .with_source_ref("psion_reasoning_sft_seed_v1")
            .with_long_context_lineage(TrainingLongContextTraceLineage::new(
                4096,
                vec![
                    String::from("segment-wasm-spec-3"),
                    String::from("segment-wasm-spec-4"),
                ],
            )),
        ];
        for trace in &traces {
            stage_program.ingest_trace(trace)?;
        }
        stage_program.complete_current_stage()?;

        let lineage_manifest = artifact_lineage_manifest();
        let sft_artifact_lineage = lineage_manifest
            .sft_artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == "psion_reasoning_sft_seed_v1")
            .expect("reasoning sft lineage row should exist")
            .clone();
        let dataset_bundle = record_psion_reasoning_sft_dataset_bundle(
            &stage_program,
            sft_artifact_lineage,
            PsionReasoningSftControlSurface {
                explicit_assumptions_required: true,
                explicit_uncertainty_language_required: true,
                normative_vs_inference_separation_required: true,
                detail: String::from(
                    "Reasoning SFT keeps assumptions, uncertainty, and normative-versus-inference separation explicit instead of deleting them during style shaping.",
                ),
            },
            style_profiles(),
            vec![
                PsionReasoningSftTraceBinding {
                    trace_id: traces[0].trace_id.clone(),
                    trace_kind: traces[0].trace_kind,
                    trace_lineage_digest: traces[0].lineage_digest.clone(),
                    style_profile_id: String::from("assumptions_then_mechanism_concrete"),
                    parent_source_ids: vec![String::from("wasm_core_spec_release_2")],
                    parent_tokenized_corpus_ids: vec![String::from("psion_seed_corpus_v1")],
                    derived_from_parent_sources: true,
                    held_out_exclusion_checked: true,
                    explicit_assumptions_present: true,
                    explicit_uncertainty_language_present: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "Concrete reasoning trace keeps assumptions and uncertainty explicit while separating normative source reading from engineering commentary.",
                    ),
                },
                PsionReasoningSftTraceBinding {
                    trace_id: traces[1].trace_id.clone(),
                    trace_kind: traces[1].trace_kind,
                    trace_lineage_digest: traces[1].lineage_digest.clone(),
                    style_profile_id: String::from("answer_then_evidence_hybrid"),
                    parent_source_ids: vec![String::from("wasm_core_spec_release_2")],
                    parent_tokenized_corpus_ids: vec![String::from("psion_seed_corpus_v1")],
                    derived_from_parent_sources: true,
                    held_out_exclusion_checked: true,
                    explicit_assumptions_present: true,
                    explicit_uncertainty_language_present: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "Hybrid long-context reasoning trace starts with the bounded answer and then cites supporting spec segments.",
                    ),
                },
                PsionReasoningSftTraceBinding {
                    trace_id: traces[2].trace_id.clone(),
                    trace_kind: traces[2].trace_kind,
                    trace_lineage_digest: traces[2].lineage_digest.clone(),
                    style_profile_id: String::from("constraints_then_tradeoffs_conceptual"),
                    parent_source_ids: vec![String::from("wasm_core_spec_release_2")],
                    parent_tokenized_corpus_ids: vec![String::from("psion_seed_corpus_v1")],
                    derived_from_parent_sources: true,
                    held_out_exclusion_checked: true,
                    explicit_assumptions_present: true,
                    explicit_uncertainty_language_present: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "Conceptual reasoning trace stays explicit about constraints and tradeoffs rather than forcing one narration template.",
                    ),
                },
                PsionReasoningSftTraceBinding {
                    trace_id: traces[3].trace_id.clone(),
                    trace_kind: traces[3].trace_kind,
                    trace_lineage_digest: traces[3].lineage_digest.clone(),
                    style_profile_id: String::from("assumptions_then_mechanism_concrete"),
                    parent_source_ids: vec![String::from("wasm_core_spec_release_2")],
                    parent_tokenized_corpus_ids: vec![String::from("psion_seed_corpus_v1")],
                    derived_from_parent_sources: true,
                    held_out_exclusion_checked: true,
                    explicit_assumptions_present: true,
                    explicit_uncertainty_language_present: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "A second concrete trace proves the lane can preserve more than one example without collapsing every answer shape into one exact outline.",
                    ),
                },
            ],
            "The bounded reasoning-SFT dataset ties derived traces back to the canonical seed SFT lineage row and requires explicit assumptions, uncertainty language, and normative-versus-inference separation across multiple valid reasoning styles.",
        )?;
        let stage_receipt = record_psion_reasoning_sft_stage_receipt(
            "psion-reasoning-sft-stage-receipt-v1",
            &stage_program,
            &dataset_bundle,
            4,
            4,
            4,
            4,
            "The first bounded reasoning-SFT stage preserves explicit assumptions, uncertainty language, and normative-versus-inference separation while keeping three valid style families live in the accepted trace mix.",
        )?;
        let evaluation_receipt = record_psion_reasoning_sft_evaluation_receipt(
            "psion-reasoning-sft-eval-receipt-v1",
            "psion_reasoning_style_plurality_eval_v1",
            &dataset_bundle,
            &stage_receipt,
            vec![
                PsionReasoningSftEvaluationRow {
                    case_id: String::from("psion-reasoning-eval-case-1"),
                    prompt_digest: String::from("eval-prompt-digest-1"),
                    accepted_style_profile_ids: vec![
                        String::from("assumptions_then_mechanism_concrete"),
                        String::from("answer_then_evidence_hybrid"),
                    ],
                    observed_style_profile_id: String::from("assumptions_then_mechanism_concrete"),
                    output_digest: String::from("eval-output-digest-1"),
                    assumptions_preserved: true,
                    uncertainty_preserved: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "Case 1 accepts either assumptions-first or answer-first reasoning; the observed output kept the assumptions-first path.",
                    ),
                },
                PsionReasoningSftEvaluationRow {
                    case_id: String::from("psion-reasoning-eval-case-2"),
                    prompt_digest: String::from("eval-prompt-digest-2"),
                    accepted_style_profile_ids: vec![
                        String::from("answer_then_evidence_hybrid"),
                        String::from("constraints_then_tradeoffs_conceptual"),
                    ],
                    observed_style_profile_id: String::from("answer_then_evidence_hybrid"),
                    output_digest: String::from("eval-output-digest-2"),
                    assumptions_preserved: true,
                    uncertainty_preserved: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "Case 2 accepts both answer-first and constraint-first narration; the observed output kept the answer-first hybrid path.",
                    ),
                },
                PsionReasoningSftEvaluationRow {
                    case_id: String::from("psion-reasoning-eval-case-3"),
                    prompt_digest: String::from("eval-prompt-digest-3"),
                    accepted_style_profile_ids: vec![
                        String::from("constraints_then_tradeoffs_conceptual"),
                        String::from("assumptions_then_mechanism_concrete"),
                    ],
                    observed_style_profile_id: String::from("constraints_then_tradeoffs_conceptual"),
                    output_digest: String::from("eval-output-digest-3"),
                    assumptions_preserved: true,
                    uncertainty_preserved: true,
                    normative_vs_inference_separated: true,
                    detail: String::from(
                        "Case 3 accepts either conceptual tradeoff framing or concrete assumptions-first framing; the observed output kept the conceptual tradeoff path.",
                    ),
                },
            ],
            "The bounded reasoning-style evaluation accepts multiple valid style families per case and observes all three declared style profiles across the evaluation set.",
        )?;
        Ok((
            stage_program,
            dataset_bundle,
            stage_receipt,
            evaluation_receipt,
        ))
    }

    #[test]
    fn reasoning_sft_run_bundle_fixture_validates() -> Result<(), Box<dyn std::error::Error>> {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let artifact_lineage = artifact_lineage_manifest();
        let (stage_program, dataset_bundle, stage_receipt, evaluation_receipt) =
            build_stage_program_and_dataset()?;
        let bundle = record_psion_reasoning_sft_run_bundle(
            "psion-reasoning-sft-run-bundle-v1",
            &lifecycle,
            &exclusion,
            &artifact_lineage,
            stage_program,
            dataset_bundle,
            stage_receipt,
            evaluation_receipt,
            "The first bounded reasoning-SFT bundle ties the canonical pretrain receipt, general-SFT stage graph, source-lineaged derived traces, truth-control surface, and style-plurality evaluation into one repo-owned receipt bundle.",
        )?;
        bundle.validate_against_context(&lifecycle, &exclusion, &artifact_lineage)?;
        Ok(())
    }

    #[test]
    fn reasoning_sft_stage_receipt_rejects_style_collapse() -> Result<(), Box<dyn std::error::Error>>
    {
        let (stage_program, dataset_bundle, mut stage_receipt, _) =
            build_stage_program_and_dataset()?;
        stage_receipt.style_coverage[0].share_bps = 7000;
        stage_receipt.style_coverage[1].share_bps = 1500;
        stage_receipt.style_coverage[2].share_bps = 1500;
        let error = stage_receipt
            .validate_against_stage(&stage_program, &dataset_bundle)
            .expect_err("style collapse should be rejected");
        assert!(matches!(
            error,
            PsionReasoningSftError::FieldMismatch { .. }
                | PsionReasoningSftError::StyleCollapse { .. }
        ));
        Ok(())
    }

    #[test]
    fn reasoning_sft_evaluation_requires_multiple_valid_styles(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_, dataset_bundle, stage_receipt, mut evaluation_receipt) =
            build_stage_program_and_dataset()?;
        evaluation_receipt.rows[0].accepted_style_profile_ids =
            vec![String::from("assumptions_then_mechanism_concrete")];
        evaluation_receipt.multiple_valid_style_pass_rate_bps = 3333;
        evaluation_receipt.receipt_digest =
            super::stable_reasoning_sft_evaluation_receipt_digest(&evaluation_receipt);
        let error = evaluation_receipt
            .validate_against_stage(&dataset_bundle, &stage_receipt)
            .expect_err("single-style evaluation row should be rejected");
        assert!(matches!(
            error,
            PsionReasoningSftError::EvaluationCaseNeedsMultipleStyles { .. }
        ));
        Ok(())
    }

    #[test]
    fn reasoning_sft_dataset_rejects_trace_lineage_drift() -> Result<(), Box<dyn std::error::Error>>
    {
        let lifecycle = lifecycle_manifest();
        let exclusion = exclusion_manifest();
        let artifact_lineage = artifact_lineage_manifest();
        let (stage_program, mut dataset_bundle, _, _) = build_stage_program_and_dataset()?;
        dataset_bundle.trace_bindings[0].trace_lineage_digest = String::from("drifted-digest");
        dataset_bundle.bundle_digest =
            super::stable_reasoning_sft_dataset_bundle_digest(&dataset_bundle);
        let error = dataset_bundle
            .validate_against_context(&stage_program, &lifecycle, &exclusion, &artifact_lineage)
            .expect_err("trace-lineage drift should be rejected");
        assert!(matches!(
            error,
            PsionReasoningSftError::FieldMismatch { .. }
        ));
        Ok(())
    }
}
