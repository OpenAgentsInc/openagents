use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    DatasetKey, DatasetSplitKind, PsionPluginConditionedDatasetBundle, PsionPluginControllerSurface,
    PsionPluginOutcomeLabel, PsionPluginRouteLabel,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionPluginArgumentConstructionBenchmarkBundle, PsionPluginBenchmarkFamily,
    PsionPluginDiscoverySelectionBenchmarkBundle,
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginResultInterpretationBenchmarkBundle, PsionPluginSequencingBenchmarkBundle,
    TrainingSftTraceIngestionReceipt, TrainingStageCompletionReceipt, TrainingStageKind,
    TrainingStageProgramState, TrainingStageTransitionReceipt,
};

/// Stable schema version for the plugin-conditioned SFT stage manifest.
pub const PSION_PLUGIN_CONDITIONED_SFT_STAGE_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_conditioned_sft_stage_manifest.v1";
/// Stable schema version for the plugin-conditioned SFT stage receipt.
pub const PSION_PLUGIN_CONDITIONED_SFT_STAGE_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_conditioned_sft_stage_receipt.v1";
/// Stable schema version for the plugin-conditioned SFT run bundle.
pub const PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_conditioned_sft_run_bundle.v1";
/// Stable committed bundle ref for the reference plugin-conditioned SFT bundle.
pub const PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_REF: &str =
    "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_run_bundle.json";

/// Dataset binding frozen for one plugin-conditioned SFT stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedDatasetBinding {
    /// Stable dataset ref.
    pub dataset_ref: String,
    /// Stable dataset identity string.
    pub stable_dataset_identity: String,
    /// Stable dataset key.
    pub dataset_key: DatasetKey,
    /// Stable dataset bundle digest.
    pub dataset_bundle_digest: String,
    /// Number of records admitted into the train split.
    pub train_record_count: u32,
    /// Number of records reserved for held-out use.
    pub held_out_record_count: u32,
    /// Held-out workflow case ids preserved for later audits.
    pub held_out_workflow_case_ids: Vec<String>,
    /// Short explanation of the dataset binding.
    pub detail: String,
}

impl PsionPluginConditionedDatasetBinding {
    fn validate_against_dataset(
        &self,
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
    ) -> Result<(), PsionPluginConditionedSftError> {
        ensure_nonempty(
            self.dataset_ref.as_str(),
            "plugin_conditioned_dataset_binding.dataset_ref",
        )?;
        ensure_nonempty(
            self.stable_dataset_identity.as_str(),
            "plugin_conditioned_dataset_binding.stable_dataset_identity",
        )?;
        ensure_nonempty(
            self.dataset_bundle_digest.as_str(),
            "plugin_conditioned_dataset_binding.dataset_bundle_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_dataset_binding.detail",
        )?;
        check_string_match(
            self.dataset_ref.as_str(),
            dataset_bundle.dataset_key.dataset_ref.as_str(),
            "plugin_conditioned_dataset_binding.dataset_ref",
        )?;
        check_string_match(
            self.stable_dataset_identity.as_str(),
            dataset_bundle.stable_dataset_identity.as_str(),
            "plugin_conditioned_dataset_binding.stable_dataset_identity",
        )?;
        if self.dataset_key != dataset_bundle.dataset_key {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_dataset_binding.dataset_key"),
                expected: format!("{:?}", dataset_bundle.dataset_key),
                actual: format!("{:?}", self.dataset_key),
            });
        }
        check_string_match(
            self.dataset_bundle_digest.as_str(),
            dataset_bundle.bundle_digest.as_str(),
            "plugin_conditioned_dataset_binding.dataset_bundle_digest",
        )?;
        let train_records = dataset_split_records(dataset_bundle, DatasetSplitKind::Train)?;
        let held_out_records = dataset_split_records(dataset_bundle, DatasetSplitKind::HeldOut)?;
        let expected_held_out_workflow_case_ids = dataset_bundle
            .held_out_isolation
            .held_out_workflow_case_ids
            .clone();
        if self.train_record_count != train_records.len() as u32 {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_dataset_binding.train_record_count"),
                expected: train_records.len().to_string(),
                actual: self.train_record_count.to_string(),
            });
        }
        if self.held_out_record_count != held_out_records.len() as u32 {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_dataset_binding.held_out_record_count"),
                expected: held_out_records.len().to_string(),
                actual: self.held_out_record_count.to_string(),
            });
        }
        if self.held_out_workflow_case_ids != expected_held_out_workflow_case_ids {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from(
                    "plugin_conditioned_dataset_binding.held_out_workflow_case_ids",
                ),
                expected: format!("{expected_held_out_workflow_case_ids:?}"),
                actual: format!("{:?}", self.held_out_workflow_case_ids),
            });
        }
        Ok(())
    }
}

/// One benchmark package binding admitted into the plugin-conditioned stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedBenchmarkBinding {
    /// Package family.
    pub benchmark_family: PsionPluginBenchmarkFamily,
    /// Stable committed bundle ref.
    pub bundle_ref: String,
    /// Stable outer bundle digest.
    pub bundle_digest: String,
    /// Shared package id.
    pub package_id: String,
    /// Shared package digest.
    pub package_digest: String,
    /// Shared receipt digest.
    pub receipt_digest: String,
    /// Whether any item in the package requires execution-backed receipts.
    pub execution_evidence_required: bool,
    /// Short explanation of the benchmark binding.
    pub detail: String,
}

impl PsionPluginConditionedBenchmarkBinding {
    fn validate(&self) -> Result<(), PsionPluginConditionedSftError> {
        ensure_nonempty(
            self.bundle_ref.as_str(),
            "plugin_conditioned_benchmark_binding.bundle_ref",
        )?;
        ensure_nonempty(
            self.bundle_digest.as_str(),
            "plugin_conditioned_benchmark_binding.bundle_digest",
        )?;
        ensure_nonempty(
            self.package_id.as_str(),
            "plugin_conditioned_benchmark_binding.package_id",
        )?;
        ensure_nonempty(
            self.package_digest.as_str(),
            "plugin_conditioned_benchmark_binding.package_digest",
        )?;
        ensure_nonempty(
            self.receipt_digest.as_str(),
            "plugin_conditioned_benchmark_binding.receipt_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_benchmark_binding.detail",
        )?;
        Ok(())
    }
}

/// Trigger point for one later evaluation hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginConditionedEvalTrigger {
    /// Hook fires immediately after stage completion.
    PostStageCompletion,
    /// Hook fires before promotion beyond the plugin-conditioned stage.
    PrePromotionAudit,
}

/// Hook family preserved for later audits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginConditionedEvalHookKind {
    /// Held-out benchmark sweep for one package family.
    BenchmarkSweep,
    /// Replay and receipt review for the stage bindings.
    ReplayReceiptReview,
}

/// One later audit hook preserved by the stage manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedEvalHook {
    /// Stable hook identifier.
    pub hook_id: String,
    /// Hook kind.
    pub hook_kind: PsionPluginConditionedEvalHookKind,
    /// Trigger for the hook.
    pub trigger: PsionPluginConditionedEvalTrigger,
    /// Benchmark family when the hook is benchmark-backed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_family: Option<PsionPluginBenchmarkFamily>,
    /// Bound benchmark bundle ref when the hook is benchmark-backed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_bundle_ref: Option<String>,
    /// Bound benchmark receipt digest when the hook is benchmark-backed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_receipt_digest: Option<String>,
    /// Whether execution-backed evidence is required to satisfy the hook.
    pub execution_evidence_required: bool,
    /// Short explanation of the hook.
    pub detail: String,
}

impl PsionPluginConditionedEvalHook {
    fn validate(
        &self,
        benchmark_bindings: &[PsionPluginConditionedBenchmarkBinding],
    ) -> Result<(), PsionPluginConditionedSftError> {
        ensure_nonempty(
            self.hook_id.as_str(),
            "plugin_conditioned_eval_hook.hook_id",
        )?;
        ensure_nonempty(self.detail.as_str(), "plugin_conditioned_eval_hook.detail")?;
        match self.hook_kind {
            PsionPluginConditionedEvalHookKind::BenchmarkSweep => {
                let benchmark_family = self.benchmark_family.ok_or_else(|| {
                    PsionPluginConditionedSftError::MissingField {
                        field: String::from("plugin_conditioned_eval_hook.benchmark_family"),
                    }
                })?;
                let benchmark_bundle_ref =
                    self.benchmark_bundle_ref.as_deref().ok_or_else(|| {
                        PsionPluginConditionedSftError::MissingField {
                            field: String::from(
                                "plugin_conditioned_eval_hook.benchmark_bundle_ref",
                            ),
                        }
                    })?;
                let benchmark_receipt_digest = self
                    .benchmark_receipt_digest
                    .as_deref()
                    .ok_or_else(|| PsionPluginConditionedSftError::MissingField {
                        field: String::from(
                            "plugin_conditioned_eval_hook.benchmark_receipt_digest",
                        ),
                    })?;
                let binding = benchmark_bindings
                    .iter()
                    .find(|binding| binding.benchmark_family == benchmark_family)
                    .ok_or_else(|| PsionPluginConditionedSftError::UnknownBenchmarkFamily {
                        benchmark_family,
                    })?;
                check_string_match(
                    benchmark_bundle_ref,
                    binding.bundle_ref.as_str(),
                    "plugin_conditioned_eval_hook.benchmark_bundle_ref",
                )?;
                check_string_match(
                    benchmark_receipt_digest,
                    binding.receipt_digest.as_str(),
                    "plugin_conditioned_eval_hook.benchmark_receipt_digest",
                )?;
                if self.execution_evidence_required && !binding.execution_evidence_required {
                    return Err(PsionPluginConditionedSftError::FieldMismatch {
                        field: String::from(
                            "plugin_conditioned_eval_hook.execution_evidence_required",
                        ),
                        expected: String::from("false"),
                        actual: String::from("true"),
                    });
                }
            }
            PsionPluginConditionedEvalHookKind::ReplayReceiptReview => {
                if self.benchmark_family.is_some()
                    || self.benchmark_bundle_ref.is_some()
                    || self.benchmark_receipt_digest.is_some()
                {
                    return Err(PsionPluginConditionedSftError::FieldMismatch {
                        field: String::from("plugin_conditioned_eval_hook.benchmark_fields"),
                        expected: String::from("none"),
                        actual: String::from("present"),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Stage-level config for plugin-conditioned SFT.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSftStageConfig {
    /// Upper bound on plugin calls preserved in one target trace.
    pub max_plugin_calls_per_trace: u32,
    /// Whether receipt boundaries must stay explicit in target traces.
    pub preserve_receipt_boundaries: bool,
    /// Whether replay-class coverage must stay explicit in stage outputs.
    pub require_replay_class_coverage: bool,
    /// Whether held-out benchmark hooks are mandatory before promotion.
    pub require_held_out_benchmark_hooks: bool,
    /// Short explanation of the config.
    pub detail: String,
}

impl PsionPluginConditionedSftStageConfig {
    fn validate_against_bindings(
        &self,
        trace_bindings: &[PsionPluginConditionedTraceBinding],
        eval_hooks: &[PsionPluginConditionedEvalHook],
    ) -> Result<(), PsionPluginConditionedSftError> {
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_stage_config.detail",
        )?;
        if self.max_plugin_calls_per_trace == 0 {
            return Err(PsionPluginConditionedSftError::MissingField {
                field: String::from("plugin_conditioned_stage_config.max_plugin_calls_per_trace"),
            });
        }
        ensure_bool_true(
            self.preserve_receipt_boundaries,
            "plugin_conditioned_stage_config.preserve_receipt_boundaries",
        )?;
        ensure_bool_true(
            self.require_replay_class_coverage,
            "plugin_conditioned_stage_config.require_replay_class_coverage",
        )?;
        ensure_bool_true(
            self.require_held_out_benchmark_hooks,
            "plugin_conditioned_stage_config.require_held_out_benchmark_hooks",
        )?;
        let max_observed_plugin_calls = trace_bindings
            .iter()
            .map(|binding| binding.receipt_refs.len() as u32)
            .max()
            .unwrap_or(0);
        if self.max_plugin_calls_per_trace < max_observed_plugin_calls {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_stage_config.max_plugin_calls_per_trace"),
                expected: format!("at least {max_observed_plugin_calls}"),
                actual: self.max_plugin_calls_per_trace.to_string(),
            });
        }
        if self.require_held_out_benchmark_hooks
            && !eval_hooks.iter().any(|hook| {
                hook.hook_kind == PsionPluginConditionedEvalHookKind::BenchmarkSweep
                    && hook.trigger == PsionPluginConditionedEvalTrigger::PrePromotionAudit
            })
        {
            return Err(PsionPluginConditionedSftError::MissingField {
                field: String::from("plugin_conditioned_stage_config.pre_promotion_benchmark_hook"),
            });
        }
        Ok(())
    }
}

/// Binding from one canonical train record into one accepted agentic trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedTraceBinding {
    /// Stable train-record id.
    pub record_id: String,
    /// Accepted agentic-SFT trace id.
    pub trace_id: String,
    /// Accepted trace lineage digest.
    pub trace_lineage_digest: String,
    /// Controller surface preserved for the trace.
    pub controller_surface: PsionPluginControllerSurface,
    /// Route label preserved for the trace.
    pub route_label: PsionPluginRouteLabel,
    /// Outcome label preserved for the trace.
    pub outcome_label: PsionPluginOutcomeLabel,
    /// Replay-class ids that remain explicit in the trace binding.
    pub replay_class_ids: Vec<String>,
    /// Runtime receipt refs visible in the trace binding.
    pub receipt_refs: Vec<String>,
    /// Short explanation of the trace binding.
    pub detail: String,
}

impl PsionPluginConditionedTraceBinding {
    fn validate_against_context(
        &self,
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
        stage_receipts: &BTreeMap<&str, &TrainingSftTraceIngestionReceipt>,
    ) -> Result<(), PsionPluginConditionedSftError> {
        ensure_nonempty(
            self.record_id.as_str(),
            "plugin_conditioned_trace_binding.record_id",
        )?;
        ensure_nonempty(
            self.trace_id.as_str(),
            "plugin_conditioned_trace_binding.trace_id",
        )?;
        ensure_nonempty(
            self.trace_lineage_digest.as_str(),
            "plugin_conditioned_trace_binding.trace_lineage_digest",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "plugin_conditioned_trace_binding.detail",
        )?;
        reject_duplicate_strings(
            self.replay_class_ids.as_slice(),
            "plugin_conditioned_trace_binding.replay_class_ids",
        )?;
        reject_duplicate_strings(
            self.receipt_refs.as_slice(),
            "plugin_conditioned_trace_binding.receipt_refs",
        )?;
        let train_records = dataset_split_records(dataset_bundle, DatasetSplitKind::Train)?;
        let record = train_records
            .iter()
            .find(|record| record.record_id == self.record_id)
            .ok_or_else(|| PsionPluginConditionedSftError::UnknownDatasetRecord {
                record_id: self.record_id.clone(),
            })?;
        if self.controller_surface != record.controller_context.controller_surface {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: format!(
                    "plugin_conditioned_trace_binding[{}].controller_surface",
                    self.record_id
                ),
                expected: format!("{:?}", record.controller_context.controller_surface),
                actual: format!("{:?}", self.controller_surface),
            });
        }
        if self.route_label != record.route_label {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: format!(
                    "plugin_conditioned_trace_binding[{}].route_label",
                    self.record_id
                ),
                expected: format!("{:?}", record.route_label),
                actual: format!("{:?}", self.route_label),
            });
        }
        if self.outcome_label != record.outcome_label {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: format!(
                    "plugin_conditioned_trace_binding[{}].outcome_label",
                    self.record_id
                ),
                expected: format!("{:?}", record.outcome_label),
                actual: format!("{:?}", self.outcome_label),
            });
        }
        let expected_replay_class_ids = record
            .admitted_plugins
            .iter()
            .map(|plugin| plugin.replay_class_id.clone())
            .collect::<BTreeSet<_>>();
        let observed_replay_class_ids = self
            .replay_class_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !observed_replay_class_ids.is_subset(&expected_replay_class_ids) {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: format!(
                    "plugin_conditioned_trace_binding[{}].replay_class_ids",
                    self.record_id
                ),
                expected: format!("subset of {expected_replay_class_ids:?}"),
                actual: format!("{observed_replay_class_ids:?}"),
            });
        }
        let expected_receipt_refs = record
            .plugin_invocations
            .iter()
            .map(|invocation| invocation.receipt_ref.clone())
            .collect::<Vec<_>>();
        let expected_receipt_ref_set = expected_receipt_refs.iter().collect::<BTreeSet<_>>();
        let observed_receipt_ref_set = self.receipt_refs.iter().collect::<BTreeSet<_>>();
        if !observed_receipt_ref_set.is_subset(&expected_receipt_ref_set) {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: format!(
                    "plugin_conditioned_trace_binding[{}].receipt_refs",
                    self.record_id
                ),
                expected: format!("subset of {expected_receipt_refs:?}"),
                actual: format!("{:?}", self.receipt_refs),
            });
        }
        let stage_receipt = stage_receipts.get(self.trace_id.as_str()).ok_or_else(|| {
            PsionPluginConditionedSftError::UnknownTraceId {
                trace_id: self.trace_id.clone(),
            }
        })?;
        check_string_match(
            self.trace_lineage_digest.as_str(),
            stage_receipt.lineage_digest.as_str(),
            "plugin_conditioned_trace_binding.trace_lineage_digest",
        )?;
        Ok(())
    }
}

/// One controller-coverage row on the stage receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedControllerCoverageRow {
    /// Controller surface represented by the row.
    pub controller_surface: PsionPluginControllerSurface,
    /// Number of traces carrying the controller surface.
    pub trace_count: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// One route-coverage row on the stage receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedRouteCoverageRow {
    /// Route label represented by the row.
    pub route_label: PsionPluginRouteLabel,
    /// Number of traces carrying the route label.
    pub trace_count: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// One replay-class coverage row on the stage receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedReplayClassCoverageRow {
    /// Replay class identifier represented by the row.
    pub replay_class_id: String,
    /// Number of traces carrying the replay class.
    pub trace_count: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// Manifest for one canonical plugin-conditioned agentic-SFT stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSftStageManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Current agentic-SFT stage id.
    pub stage_id: String,
    /// Immediately preceding general-SFT stage id.
    pub previous_stage_id: String,
    /// Shared checkpoint family.
    pub checkpoint_family: String,
    /// Dataset binding for the stage.
    pub dataset_binding: PsionPluginConditionedDatasetBinding,
    /// Trace bindings admitted into the stage.
    pub trace_bindings: Vec<PsionPluginConditionedTraceBinding>,
    /// Benchmark bindings preserved for later held-out audits.
    pub benchmark_bindings: Vec<PsionPluginConditionedBenchmarkBinding>,
    /// Evaluation hooks preserved for later audits.
    pub eval_hooks: Vec<PsionPluginConditionedEvalHook>,
    /// Stage-level config.
    pub config: PsionPluginConditionedSftStageConfig,
    /// Short explanation of the stage manifest.
    pub summary: String,
    /// Stable digest over the manifest.
    pub manifest_digest: String,
}

impl PsionPluginConditionedSftStageManifest {
    /// Validates the manifest against the stage program and canonical dataset bundle.
    pub fn validate_against_context(
        &self,
        stage_program: &TrainingStageProgramState,
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
    ) -> Result<(), PsionPluginConditionedSftError> {
        let stage_context = plugin_conditioned_stage_context(stage_program)?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_CONDITIONED_SFT_STAGE_MANIFEST_SCHEMA_VERSION,
            "plugin_conditioned_stage_manifest.schema_version",
        )?;
        check_string_match(
            self.run_id.as_str(),
            stage_program.run_id.as_str(),
            "plugin_conditioned_stage_manifest.run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_context.agentic_sft_stage.stage_id.as_str(),
            "plugin_conditioned_stage_manifest.stage_id",
        )?;
        check_string_match(
            self.previous_stage_id.as_str(),
            stage_context.general_sft_stage.stage_id.as_str(),
            "plugin_conditioned_stage_manifest.previous_stage_id",
        )?;
        check_string_match(
            self.checkpoint_family.as_str(),
            stage_program.checkpoint_family.as_str(),
            "plugin_conditioned_stage_manifest.checkpoint_family",
        )?;
        self.dataset_binding
            .validate_against_dataset(dataset_bundle)?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_conditioned_stage_manifest.summary",
        )?;
        if self.trace_bindings.is_empty() {
            return Err(PsionPluginConditionedSftError::MissingField {
                field: String::from("plugin_conditioned_stage_manifest.trace_bindings"),
            });
        }
        if self.benchmark_bindings.is_empty() {
            return Err(PsionPluginConditionedSftError::MissingField {
                field: String::from("plugin_conditioned_stage_manifest.benchmark_bindings"),
            });
        }
        if self.eval_hooks.is_empty() {
            return Err(PsionPluginConditionedSftError::MissingField {
                field: String::from("plugin_conditioned_stage_manifest.eval_hooks"),
            });
        }
        let mut seen_record_ids = BTreeSet::new();
        let mut seen_trace_ids = BTreeSet::new();
        let stage_receipts = stage_context
            .agentic_sft_stage
            .ingested_traces
            .iter()
            .map(|receipt| (receipt.trace_id.as_str(), receipt))
            .collect::<BTreeMap<_, _>>();
        for binding in &self.trace_bindings {
            binding.validate_against_context(dataset_bundle, &stage_receipts)?;
            if !seen_record_ids.insert(binding.record_id.as_str()) {
                return Err(PsionPluginConditionedSftError::DuplicateValue {
                    field: String::from(
                        "plugin_conditioned_stage_manifest.trace_bindings.record_id",
                    ),
                    value: binding.record_id.clone(),
                });
            }
            if !seen_trace_ids.insert(binding.trace_id.as_str()) {
                return Err(PsionPluginConditionedSftError::DuplicateValue {
                    field: String::from(
                        "plugin_conditioned_stage_manifest.trace_bindings.trace_id",
                    ),
                    value: binding.trace_id.clone(),
                });
            }
        }
        if self.trace_bindings.len() != stage_context.agentic_sft_stage.ingested_traces.len() {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_stage_manifest.trace_bindings"),
                expected: stage_context
                    .agentic_sft_stage
                    .ingested_traces
                    .len()
                    .to_string(),
                actual: self.trace_bindings.len().to_string(),
            });
        }
        let train_records = dataset_split_records(dataset_bundle, DatasetSplitKind::Train)?;
        if self.trace_bindings.len() != train_records.len() {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_stage_manifest.trace_bindings"),
                expected: train_records.len().to_string(),
                actual: self.trace_bindings.len().to_string(),
            });
        }
        let required_families = required_host_native_benchmark_families();
        let observed_families = self
            .benchmark_bindings
            .iter()
            .map(|binding| binding.benchmark_family)
            .collect::<BTreeSet<_>>();
        if observed_families != required_families {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_stage_manifest.benchmark_bindings.family"),
                expected: format!("{required_families:?}"),
                actual: format!("{observed_families:?}"),
            });
        }
        let mut seen_bundle_refs = BTreeSet::new();
        for binding in &self.benchmark_bindings {
            binding.validate()?;
            if !seen_bundle_refs.insert(binding.bundle_ref.as_str()) {
                return Err(PsionPluginConditionedSftError::DuplicateValue {
                    field: String::from(
                        "plugin_conditioned_stage_manifest.benchmark_bindings.bundle_ref",
                    ),
                    value: binding.bundle_ref.clone(),
                });
            }
        }
        let mut seen_hook_ids = BTreeSet::new();
        for hook in &self.eval_hooks {
            hook.validate(self.benchmark_bindings.as_slice())?;
            if !seen_hook_ids.insert(hook.hook_id.as_str()) {
                return Err(PsionPluginConditionedSftError::DuplicateValue {
                    field: String::from("plugin_conditioned_stage_manifest.eval_hooks.hook_id"),
                    value: hook.hook_id.clone(),
                });
            }
        }
        self.config.validate_against_bindings(
            self.trace_bindings.as_slice(),
            self.eval_hooks.as_slice(),
        )?;
        if self.manifest_digest != stable_manifest_digest(self) {
            return Err(PsionPluginConditionedSftError::DigestMismatch {
                kind: String::from("plugin_conditioned_stage_manifest"),
            });
        }
        Ok(())
    }
}

/// Receipt for one completed plugin-conditioned stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSftStageReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Current agentic-SFT stage id.
    pub stage_id: String,
    /// Shared checkpoint family.
    pub checkpoint_family: String,
    /// Bound stage manifest digest.
    pub stage_manifest_digest: String,
    /// Bound dataset bundle digest.
    pub dataset_bundle_digest: String,
    /// Transition digest into the agentic-SFT stage.
    pub agentic_sft_transition_digest: String,
    /// Completion digest for the preceding general-SFT stage.
    pub general_sft_completion_digest: String,
    /// Completion digest for the current agentic-SFT stage.
    pub agentic_sft_completion_digest: String,
    /// Number of accepted plugin-conditioned traces.
    pub ingested_trace_count: u32,
    /// Number of traces with explicit receipt evidence.
    pub execution_evidence_trace_count: u32,
    /// Number of later evaluation hooks bound to the stage.
    pub eval_hook_count: u32,
    /// Controller coverage for the stage.
    pub controller_coverage: Vec<PsionPluginConditionedControllerCoverageRow>,
    /// Route coverage for the stage.
    pub route_coverage: Vec<PsionPluginConditionedRouteCoverageRow>,
    /// Replay-class coverage for the stage.
    pub replay_class_coverage: Vec<PsionPluginConditionedReplayClassCoverageRow>,
    /// Short explanation of the stage receipt.
    pub summary: String,
    /// Stable digest over the stage receipt.
    pub receipt_digest: String,
}

impl PsionPluginConditionedSftStageReceipt {
    /// Validates the receipt against the stage manifest and stage program.
    pub fn validate_against_context(
        &self,
        stage_program: &TrainingStageProgramState,
        stage_manifest: &PsionPluginConditionedSftStageManifest,
    ) -> Result<(), PsionPluginConditionedSftError> {
        let stage_context = plugin_conditioned_stage_context(stage_program)?;
        let agentic_completion =
            stage_context
                .agentic_sft_completion
                .ok_or_else(|| PsionPluginConditionedSftError::UnexpectedStageGraph {
                    detail: String::from(
                        "plugin-conditioned stage receipt requires the agentic_sft stage to be completed",
                    ),
                })?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_CONDITIONED_SFT_STAGE_RECEIPT_SCHEMA_VERSION,
            "plugin_conditioned_stage_receipt.schema_version",
        )?;
        ensure_nonempty(
            self.receipt_id.as_str(),
            "plugin_conditioned_stage_receipt.receipt_id",
        )?;
        check_string_match(
            self.run_id.as_str(),
            stage_program.run_id.as_str(),
            "plugin_conditioned_stage_receipt.run_id",
        )?;
        check_string_match(
            self.stage_id.as_str(),
            stage_context.agentic_sft_stage.stage_id.as_str(),
            "plugin_conditioned_stage_receipt.stage_id",
        )?;
        check_string_match(
            self.checkpoint_family.as_str(),
            stage_program.checkpoint_family.as_str(),
            "plugin_conditioned_stage_receipt.checkpoint_family",
        )?;
        check_string_match(
            self.stage_manifest_digest.as_str(),
            stage_manifest.manifest_digest.as_str(),
            "plugin_conditioned_stage_receipt.stage_manifest_digest",
        )?;
        check_string_match(
            self.dataset_bundle_digest.as_str(),
            stage_manifest
                .dataset_binding
                .dataset_bundle_digest
                .as_str(),
            "plugin_conditioned_stage_receipt.dataset_bundle_digest",
        )?;
        check_string_match(
            self.agentic_sft_transition_digest.as_str(),
            stage_context
                .agentic_sft_transition
                .transition_digest
                .as_str(),
            "plugin_conditioned_stage_receipt.agentic_sft_transition_digest",
        )?;
        check_string_match(
            self.general_sft_completion_digest.as_str(),
            stage_context
                .general_sft_completion
                .completion_digest
                .as_str(),
            "plugin_conditioned_stage_receipt.general_sft_completion_digest",
        )?;
        check_string_match(
            self.agentic_sft_completion_digest.as_str(),
            agentic_completion.completion_digest.as_str(),
            "plugin_conditioned_stage_receipt.agentic_sft_completion_digest",
        )?;
        if self.ingested_trace_count != stage_manifest.trace_bindings.len() as u32 {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_stage_receipt.ingested_trace_count"),
                expected: stage_manifest.trace_bindings.len().to_string(),
                actual: self.ingested_trace_count.to_string(),
            });
        }
        let expected_execution_evidence_trace_count = stage_manifest
            .trace_bindings
            .iter()
            .filter(|binding| !binding.receipt_refs.is_empty())
            .count() as u32;
        if self.execution_evidence_trace_count != expected_execution_evidence_trace_count {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from(
                    "plugin_conditioned_stage_receipt.execution_evidence_trace_count",
                ),
                expected: expected_execution_evidence_trace_count.to_string(),
                actual: self.execution_evidence_trace_count.to_string(),
            });
        }
        if self.eval_hook_count != stage_manifest.eval_hooks.len() as u32 {
            return Err(PsionPluginConditionedSftError::FieldMismatch {
                field: String::from("plugin_conditioned_stage_receipt.eval_hook_count"),
                expected: stage_manifest.eval_hooks.len().to_string(),
                actual: self.eval_hook_count.to_string(),
            });
        }
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_conditioned_stage_receipt.summary",
        )?;
        validate_controller_coverage(
            self.controller_coverage.as_slice(),
            stage_manifest.trace_bindings.as_slice(),
        )?;
        validate_route_coverage(
            self.route_coverage.as_slice(),
            stage_manifest.trace_bindings.as_slice(),
        )?;
        validate_replay_coverage(
            self.replay_class_coverage.as_slice(),
            stage_manifest.trace_bindings.as_slice(),
        )?;
        if self.receipt_digest != stable_stage_receipt_digest(self) {
            return Err(PsionPluginConditionedSftError::DigestMismatch {
                kind: String::from("plugin_conditioned_stage_receipt"),
            });
        }
        Ok(())
    }
}

/// Bounded output bundle for the plugin-conditioned SFT stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginConditionedSftRunBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle id.
    pub bundle_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Shared checkpoint family.
    pub checkpoint_family: String,
    /// Full training-stage program state.
    pub stage_program: TrainingStageProgramState,
    /// Stage manifest.
    pub stage_manifest: PsionPluginConditionedSftStageManifest,
    /// Stage receipt.
    pub stage_receipt: PsionPluginConditionedSftStageReceipt,
    /// Short explanation of the bundle.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionPluginConditionedSftRunBundle {
    /// Writes the run bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginConditionedSftError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginConditionedSftError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginConditionedSftError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })?;
        Ok(())
    }

    /// Validates the bounded output bundle.
    pub fn validate_against_context(
        &self,
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
    ) -> Result<(), PsionPluginConditionedSftError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_SCHEMA_VERSION,
            "plugin_conditioned_run_bundle.schema_version",
        )?;
        ensure_nonempty(
            self.bundle_id.as_str(),
            "plugin_conditioned_run_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_conditioned_run_bundle.summary",
        )?;
        check_string_match(
            self.run_id.as_str(),
            self.stage_program.run_id.as_str(),
            "plugin_conditioned_run_bundle.run_id",
        )?;
        check_string_match(
            self.checkpoint_family.as_str(),
            self.stage_program.checkpoint_family.as_str(),
            "plugin_conditioned_run_bundle.checkpoint_family",
        )?;
        self.stage_manifest
            .validate_against_context(&self.stage_program, dataset_bundle)?;
        self.stage_receipt
            .validate_against_context(&self.stage_program, &self.stage_manifest)?;
        if self.bundle_digest != stable_run_bundle_digest(self) {
            return Err(PsionPluginConditionedSftError::DigestMismatch {
                kind: String::from("plugin_conditioned_run_bundle"),
            });
        }
        Ok(())
    }
}

/// Records one stage manifest for the canonical plugin-conditioned SFT lane.
pub fn record_psion_plugin_conditioned_sft_stage_manifest(
    stage_program: &TrainingStageProgramState,
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
    trace_bindings: Vec<PsionPluginConditionedTraceBinding>,
    benchmark_bindings: Vec<PsionPluginConditionedBenchmarkBinding>,
    eval_hooks: Vec<PsionPluginConditionedEvalHook>,
    config: PsionPluginConditionedSftStageConfig,
    summary: impl Into<String>,
) -> Result<PsionPluginConditionedSftStageManifest, PsionPluginConditionedSftError> {
    let stage_context = plugin_conditioned_stage_context(stage_program)?;
    let held_out_records = dataset_split_records(dataset_bundle, DatasetSplitKind::HeldOut)?;
    let held_out_workflow_case_ids = dataset_bundle
        .held_out_isolation
        .held_out_workflow_case_ids
        .clone();
    let mut manifest = PsionPluginConditionedSftStageManifest {
        schema_version: String::from(PSION_PLUGIN_CONDITIONED_SFT_STAGE_MANIFEST_SCHEMA_VERSION),
        run_id: stage_program.run_id.clone(),
        stage_id: stage_context.agentic_sft_stage.stage_id.clone(),
        previous_stage_id: stage_context.general_sft_stage.stage_id.clone(),
        checkpoint_family: stage_program.checkpoint_family.clone(),
        dataset_binding: PsionPluginConditionedDatasetBinding {
            dataset_ref: dataset_bundle.dataset_key.dataset_ref.clone(),
            stable_dataset_identity: dataset_bundle.stable_dataset_identity.clone(),
            dataset_key: dataset_bundle.dataset_key.clone(),
            dataset_bundle_digest: dataset_bundle.bundle_digest.clone(),
            train_record_count: dataset_split_records(dataset_bundle, DatasetSplitKind::Train)?
                .len() as u32,
            held_out_record_count: held_out_records.len() as u32,
            held_out_workflow_case_ids,
            detail: String::from(
                "The plugin-conditioned stage binds directly to the committed dataset identity and preserves the held-out workflow split for later audits.",
            ),
        },
        trace_bindings,
        benchmark_bindings,
        eval_hooks,
        config,
        summary: summary.into(),
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = stable_manifest_digest(&manifest);
    manifest.validate_against_context(stage_program, dataset_bundle)?;
    Ok(manifest)
}

/// Records one stage receipt for the canonical plugin-conditioned SFT lane.
pub fn record_psion_plugin_conditioned_sft_stage_receipt(
    receipt_id: impl Into<String>,
    stage_program: &TrainingStageProgramState,
    stage_manifest: &PsionPluginConditionedSftStageManifest,
    summary: impl Into<String>,
) -> Result<PsionPluginConditionedSftStageReceipt, PsionPluginConditionedSftError> {
    let stage_context = plugin_conditioned_stage_context(stage_program)?;
    let agentic_completion = stage_context.agentic_sft_completion.ok_or_else(|| {
        PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: String::from(
                "plugin-conditioned stage receipt requires the agentic_sft stage to be completed",
            ),
        }
    })?;
    let mut receipt = PsionPluginConditionedSftStageReceipt {
        schema_version: String::from(PSION_PLUGIN_CONDITIONED_SFT_STAGE_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        run_id: stage_program.run_id.clone(),
        stage_id: stage_context.agentic_sft_stage.stage_id.clone(),
        checkpoint_family: stage_program.checkpoint_family.clone(),
        stage_manifest_digest: stage_manifest.manifest_digest.clone(),
        dataset_bundle_digest: stage_manifest.dataset_binding.dataset_bundle_digest.clone(),
        agentic_sft_transition_digest: stage_context
            .agentic_sft_transition
            .transition_digest
            .clone(),
        general_sft_completion_digest: stage_context
            .general_sft_completion
            .completion_digest
            .clone(),
        agentic_sft_completion_digest: agentic_completion.completion_digest.clone(),
        ingested_trace_count: stage_manifest.trace_bindings.len() as u32,
        execution_evidence_trace_count: stage_manifest
            .trace_bindings
            .iter()
            .filter(|binding| !binding.receipt_refs.is_empty())
            .count() as u32,
        eval_hook_count: stage_manifest.eval_hooks.len() as u32,
        controller_coverage: expected_controller_coverage(stage_manifest.trace_bindings.as_slice()),
        route_coverage: expected_route_coverage(stage_manifest.trace_bindings.as_slice()),
        replay_class_coverage: expected_replay_coverage(stage_manifest.trace_bindings.as_slice()),
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_stage_receipt_digest(&receipt);
    receipt.validate_against_context(stage_program, stage_manifest)?;
    Ok(receipt)
}

/// Records the bounded plugin-conditioned SFT run bundle.
pub fn record_psion_plugin_conditioned_sft_run_bundle(
    bundle_id: impl Into<String>,
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
    stage_program: TrainingStageProgramState,
    stage_manifest: PsionPluginConditionedSftStageManifest,
    stage_receipt: PsionPluginConditionedSftStageReceipt,
    summary: impl Into<String>,
) -> Result<PsionPluginConditionedSftRunBundle, PsionPluginConditionedSftError> {
    let mut bundle = PsionPluginConditionedSftRunBundle {
        schema_version: String::from(PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        run_id: stage_program.run_id.clone(),
        checkpoint_family: stage_program.checkpoint_family.clone(),
        stage_program,
        stage_manifest,
        stage_receipt,
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_run_bundle_digest(&bundle);
    bundle.validate_against_context(dataset_bundle)?;
    Ok(bundle)
}

/// Returns the canonical output path for the reference run bundle.
#[must_use]
pub fn psion_plugin_conditioned_sft_run_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_CONDITIONED_SFT_RUN_BUNDLE_REF)
}

/// Creates one benchmark binding from the discovery-selection benchmark bundle.
pub fn psion_plugin_discovery_selection_benchmark_binding(
    bundle: &PsionPluginDiscoverySelectionBenchmarkBundle,
) -> PsionPluginConditionedBenchmarkBinding {
    benchmark_binding(
        PsionPluginBenchmarkFamily::DiscoverySelection,
        crate::PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_REF,
        bundle.bundle_digest.as_str(),
        bundle.package.package_id.as_str(),
        bundle.package.package_digest.as_str(),
        bundle.receipt.receipt_digest.as_str(),
        bundle
            .package
            .items
            .iter()
            .any(|item| item.receipt_posture.execution_evidence_required),
        bundle.summary.as_str(),
    )
}

/// Creates one benchmark binding from the argument-construction benchmark bundle.
pub fn psion_plugin_argument_construction_benchmark_binding(
    bundle: &PsionPluginArgumentConstructionBenchmarkBundle,
) -> PsionPluginConditionedBenchmarkBinding {
    benchmark_binding(
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        crate::PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_REF,
        bundle.bundle_digest.as_str(),
        bundle.package.package_id.as_str(),
        bundle.package.package_digest.as_str(),
        bundle.receipt.receipt_digest.as_str(),
        bundle
            .package
            .items
            .iter()
            .any(|item| item.receipt_posture.execution_evidence_required),
        bundle.summary.as_str(),
    )
}

/// Creates one benchmark binding from the sequencing benchmark bundle.
pub fn psion_plugin_sequencing_benchmark_binding(
    bundle: &PsionPluginSequencingBenchmarkBundle,
) -> PsionPluginConditionedBenchmarkBinding {
    benchmark_binding(
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        crate::PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_REF,
        bundle.bundle_digest.as_str(),
        bundle.package.package_id.as_str(),
        bundle.package.package_digest.as_str(),
        bundle.receipt.receipt_digest.as_str(),
        bundle
            .package
            .items
            .iter()
            .any(|item| item.receipt_posture.execution_evidence_required),
        bundle.summary.as_str(),
    )
}

/// Creates one benchmark binding from the refusal/request-structure benchmark bundle.
pub fn psion_plugin_refusal_request_structure_benchmark_binding(
    bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle,
) -> PsionPluginConditionedBenchmarkBinding {
    benchmark_binding(
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        crate::PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF,
        bundle.bundle_digest.as_str(),
        bundle.package.package_id.as_str(),
        bundle.package.package_digest.as_str(),
        bundle.receipt.receipt_digest.as_str(),
        bundle
            .package
            .items
            .iter()
            .any(|item| item.receipt_posture.execution_evidence_required),
        bundle.summary.as_str(),
    )
}

/// Creates one benchmark binding from the result-interpretation benchmark bundle.
pub fn psion_plugin_result_interpretation_benchmark_binding(
    bundle: &PsionPluginResultInterpretationBenchmarkBundle,
) -> PsionPluginConditionedBenchmarkBinding {
    benchmark_binding(
        PsionPluginBenchmarkFamily::ResultInterpretation,
        crate::PSION_PLUGIN_RESULT_INTERPRETATION_BENCHMARK_BUNDLE_REF,
        bundle.bundle_digest.as_str(),
        bundle.package.package_id.as_str(),
        bundle.package.package_digest.as_str(),
        bundle.receipt.receipt_digest.as_str(),
        bundle
            .package
            .items
            .iter()
            .any(|item| item.receipt_posture.execution_evidence_required),
        bundle.summary.as_str(),
    )
}

#[derive(Clone)]
struct PluginConditionedStageContext<'a> {
    general_sft_stage: &'a crate::TrainingStageState,
    general_sft_completion: &'a TrainingStageCompletionReceipt,
    agentic_sft_stage: &'a crate::TrainingStageState,
    agentic_sft_transition: &'a TrainingStageTransitionReceipt,
    agentic_sft_completion: Option<&'a TrainingStageCompletionReceipt>,
}

fn plugin_conditioned_stage_context(
    stage_program: &TrainingStageProgramState,
) -> Result<PluginConditionedStageContext<'_>, PsionPluginConditionedSftError> {
    if stage_program.stages.len() < 2 {
        return Err(PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: format!(
                "expected at least 2 stages ending in general_sft -> agentic_sft, found {}",
                stage_program.stages.len()
            ),
        });
    }
    let agentic_sft_stage = stage_program.stages.last().ok_or_else(|| {
        PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: String::from("missing agentic_sft stage"),
        }
    })?;
    if agentic_sft_stage.kind != TrainingStageKind::AgenticSft {
        return Err(PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: format!(
                "expected final stage kind agentic_sft, found {:?}",
                agentic_sft_stage.kind
            ),
        });
    }
    let general_sft_stage = &stage_program.stages[stage_program.stages.len() - 2];
    if general_sft_stage.kind != TrainingStageKind::GeneralSft {
        return Err(PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: format!(
                "expected penultimate stage kind general_sft, found {:?}",
                general_sft_stage.kind
            ),
        });
    }
    let general_sft_completion = stage_program
        .completions
        .iter()
        .find(|completion| completion.stage_id == general_sft_stage.stage_id)
        .ok_or_else(|| PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: String::from("general_sft stage is missing its completion receipt"),
        })?;
    let agentic_sft_transition = stage_program
        .transitions
        .iter()
        .find(|transition| transition.next_stage_id == agentic_sft_stage.stage_id)
        .ok_or_else(|| PsionPluginConditionedSftError::UnexpectedStageGraph {
            detail: String::from("agentic_sft stage is missing its transition receipt"),
        })?;
    let agentic_sft_completion = stage_program
        .completions
        .iter()
        .find(|completion| completion.stage_id == agentic_sft_stage.stage_id);
    Ok(PluginConditionedStageContext {
        general_sft_stage,
        general_sft_completion,
        agentic_sft_stage,
        agentic_sft_transition,
        agentic_sft_completion,
    })
}

fn required_host_native_benchmark_families() -> BTreeSet<PsionPluginBenchmarkFamily> {
    [
        PsionPluginBenchmarkFamily::DiscoverySelection,
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        PsionPluginBenchmarkFamily::ResultInterpretation,
    ]
    .into_iter()
    .collect()
}

fn dataset_split_records(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
    split_kind: DatasetSplitKind,
) -> Result<Vec<psionic_data::PsionPluginTrainingRecord>, PsionPluginConditionedSftError> {
    dataset_bundle
        .split_rows
        .iter()
        .find(|split| split.split_kind == split_kind)
        .map(|split| split.records.clone())
        .ok_or_else(|| match split_kind {
            DatasetSplitKind::Train => PsionPluginConditionedSftError::MissingTrainSplit,
            DatasetSplitKind::HeldOut => PsionPluginConditionedSftError::MissingHeldOutSplit,
            _ => PsionPluginConditionedSftError::UnexpectedStageGraph {
                detail: format!("unsupported plugin-conditioned split {split_kind:?}"),
            },
        })
}

fn expected_controller_coverage(
    trace_bindings: &[PsionPluginConditionedTraceBinding],
) -> Vec<PsionPluginConditionedControllerCoverageRow> {
    let mut counts = BTreeMap::new();
    for binding in trace_bindings {
        *counts.entry(binding.controller_surface).or_insert(0_u32) += 1;
    }
    counts
        .into_iter()
        .map(
            |(controller_surface, trace_count)| PsionPluginConditionedControllerCoverageRow {
                controller_surface,
                trace_count,
                detail: format!(
                    "The stage preserves {} accepted plugin-conditioned traces from {:?}.",
                    trace_count, controller_surface
                ),
            },
        )
        .collect()
}

fn expected_route_coverage(
    trace_bindings: &[PsionPluginConditionedTraceBinding],
) -> Vec<PsionPluginConditionedRouteCoverageRow> {
    let mut counts = BTreeMap::new();
    for binding in trace_bindings {
        *counts.entry(binding.route_label).or_insert(0_u32) += 1;
    }
    counts
        .into_iter()
        .map(
            |(route_label, trace_count)| PsionPluginConditionedRouteCoverageRow {
                route_label,
                trace_count,
                detail: format!(
                    "The stage preserves {} traces carrying {:?}.",
                    trace_count, route_label
                ),
            },
        )
        .collect()
}

fn expected_replay_coverage(
    trace_bindings: &[PsionPluginConditionedTraceBinding],
) -> Vec<PsionPluginConditionedReplayClassCoverageRow> {
    let mut counts = BTreeMap::new();
    for binding in trace_bindings {
        for replay_class_id in &binding.replay_class_ids {
            *counts.entry(replay_class_id.clone()).or_insert(0_u32) += 1;
        }
    }
    counts
        .into_iter()
        .map(
            |(replay_class_id, trace_count)| PsionPluginConditionedReplayClassCoverageRow {
                detail: format!(
                "Replay class `{replay_class_id}` stays explicit on {trace_count} accepted traces."
            ),
                replay_class_id,
                trace_count,
            },
        )
        .collect()
}

fn validate_controller_coverage(
    actual: &[PsionPluginConditionedControllerCoverageRow],
    trace_bindings: &[PsionPluginConditionedTraceBinding],
) -> Result<(), PsionPluginConditionedSftError> {
    let expected = expected_controller_coverage(trace_bindings);
    if actual != expected {
        return Err(PsionPluginConditionedSftError::FieldMismatch {
            field: String::from("plugin_conditioned_stage_receipt.controller_coverage"),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
    Ok(())
}

fn validate_route_coverage(
    actual: &[PsionPluginConditionedRouteCoverageRow],
    trace_bindings: &[PsionPluginConditionedTraceBinding],
) -> Result<(), PsionPluginConditionedSftError> {
    let expected = expected_route_coverage(trace_bindings);
    if actual != expected {
        return Err(PsionPluginConditionedSftError::FieldMismatch {
            field: String::from("plugin_conditioned_stage_receipt.route_coverage"),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
    Ok(())
}

fn validate_replay_coverage(
    actual: &[PsionPluginConditionedReplayClassCoverageRow],
    trace_bindings: &[PsionPluginConditionedTraceBinding],
) -> Result<(), PsionPluginConditionedSftError> {
    let expected = expected_replay_coverage(trace_bindings);
    if actual != expected {
        return Err(PsionPluginConditionedSftError::FieldMismatch {
            field: String::from("plugin_conditioned_stage_receipt.replay_class_coverage"),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        });
    }
    Ok(())
}

fn benchmark_binding(
    benchmark_family: PsionPluginBenchmarkFamily,
    bundle_ref: &str,
    bundle_digest: &str,
    package_id: &str,
    package_digest: &str,
    receipt_digest: &str,
    execution_evidence_required: bool,
    detail: &str,
) -> PsionPluginConditionedBenchmarkBinding {
    PsionPluginConditionedBenchmarkBinding {
        benchmark_family,
        bundle_ref: String::from(bundle_ref),
        bundle_digest: String::from(bundle_digest),
        package_id: String::from(package_id),
        package_digest: String::from(package_digest),
        receipt_digest: String::from(receipt_digest),
        execution_evidence_required,
        detail: String::from(detail),
    }
}

fn stable_manifest_digest(manifest: &PsionPluginConditionedSftStageManifest) -> String {
    let mut canonical = manifest.clone();
    canonical.manifest_digest.clear();
    stable_digest(b"psion_plugin_conditioned_stage_manifest|", &canonical)
}

fn stable_stage_receipt_digest(receipt: &PsionPluginConditionedSftStageReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    stable_digest(b"psion_plugin_conditioned_stage_receipt|", &canonical)
}

fn stable_run_bundle_digest(bundle: &PsionPluginConditionedSftRunBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    stable_digest(b"psion_plugin_conditioned_run_bundle|", &canonical)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("plugin conditioned SFT value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&encoded);
    hex::encode(hasher.finalize())
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginConditionedSftError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginConditionedSftError::DuplicateValue {
                field: String::from(field),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginConditionedSftError> {
    if value.trim().is_empty() {
        return Err(PsionPluginConditionedSftError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_bool_true(value: bool, field: &str) -> Result<(), PsionPluginConditionedSftError> {
    if !value {
        return Err(PsionPluginConditionedSftError::FieldMismatch {
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
) -> Result<(), PsionPluginConditionedSftError> {
    if actual != expected {
        return Err(PsionPluginConditionedSftError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace layout should keep `crates/` under the repo root")
        .parent()
        .expect("workspace layout should keep `crates/` two levels below the repo root")
        .to_path_buf()
}

/// Error returned by the plugin-conditioned SFT stage contract.
#[derive(Debug, Error)]
pub enum PsionPluginConditionedSftError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("unexpected stage graph: {detail}")]
    UnexpectedStageGraph { detail: String },
    #[error("missing train split in the plugin-conditioned dataset bundle")]
    MissingTrainSplit,
    #[error("missing held-out split in the plugin-conditioned dataset bundle")]
    MissingHeldOutSplit,
    #[error("unknown train record `{record_id}` in the plugin-conditioned manifest")]
    UnknownDatasetRecord { record_id: String },
    #[error("unknown accepted trace `{trace_id}` in the plugin-conditioned manifest")]
    UnknownTraceId { trace_id: String },
    #[error("duplicate value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("unknown benchmark family `{benchmark_family:?}` in the plugin-conditioned manifest")]
    UnknownBenchmarkFamily {
        benchmark_family: PsionPluginBenchmarkFamily,
    },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        psion_plugin_argument_construction_benchmark_binding,
        psion_plugin_discovery_selection_benchmark_binding,
        psion_plugin_refusal_request_structure_benchmark_binding,
        psion_plugin_result_interpretation_benchmark_binding,
        psion_plugin_sequencing_benchmark_binding, record_psion_plugin_conditioned_sft_run_bundle,
        record_psion_plugin_conditioned_sft_stage_manifest,
        record_psion_plugin_conditioned_sft_stage_receipt, PsionPluginConditionedEvalHook,
        PsionPluginConditionedEvalHookKind, PsionPluginConditionedEvalTrigger,
        PsionPluginConditionedSftStageConfig, PsionPluginConditionedTraceBinding,
    };
    use crate::{
        build_psion_plugin_argument_construction_benchmark_bundle,
        build_psion_plugin_discovery_selection_benchmark_bundle,
        build_psion_plugin_refusal_request_structure_benchmark_bundle,
        build_psion_plugin_result_interpretation_benchmark_bundle,
        build_psion_plugin_sequencing_benchmark_bundle, TrainingSftTraceArtifact,
        TrainingSftTraceKind, TrainingStageKind, TrainingStageProgramState,
        TrainingToolCallTraceLineage, TrainingToolCallTraceStep,
    };
    use psionic_data::{
        build_psion_plugin_conditioned_dataset_bundle, DatasetSplitKind,
        PsionPluginConditionedDatasetBundle,
    };
    use psionic_environments::EnvironmentPackageKey;
    use psionic_runtime::TrainingCheckpointReference;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    #[test]
    fn plugin_conditioned_run_bundle_validates() -> Result<(), Box<dyn std::error::Error>> {
        let dataset_bundle = build_psion_plugin_conditioned_dataset_bundle()?;
        let stage_program = plugin_conditioned_stage_program(&dataset_bundle)?;
        let benchmark_bindings = benchmark_bindings()?;
        let trace_bindings = trace_bindings(&dataset_bundle, &stage_program)?;
        let max_plugin_calls_per_trace = max_plugin_calls_per_trace(&trace_bindings);
        let eval_hooks = eval_hooks(&benchmark_bindings);
        let stage_manifest = record_psion_plugin_conditioned_sft_stage_manifest(
            &stage_program,
            &dataset_bundle,
            trace_bindings,
            benchmark_bindings,
            eval_hooks,
            PsionPluginConditionedSftStageConfig {
                max_plugin_calls_per_trace,
                preserve_receipt_boundaries: true,
                require_replay_class_coverage: true,
                require_held_out_benchmark_hooks: true,
                detail: String::from(
                    "The first plugin-conditioned agentic-SFT stage preserves receipt boundaries, replay classes, and later held-out benchmark hooks explicitly.",
                ),
            },
            "The first plugin-conditioned stage binds the canonical host-native dataset, all five benchmark families, and explicit later audit hooks onto one agentic-SFT stage contract.",
        )?;
        let stage_receipt = record_psion_plugin_conditioned_sft_stage_receipt(
            "receipt.psion.plugin_conditioned_sft.reference.v1",
            &stage_program,
            &stage_manifest,
            "The first plugin-conditioned stage completed with one accepted trace per committed host-native train record and explicit replay plus held-out audit posture.",
        )?;
        let bundle = record_psion_plugin_conditioned_sft_run_bundle(
            "bundle.psion.plugin_conditioned_sft.reference.v1",
            &dataset_bundle,
            stage_program,
            stage_manifest,
            stage_receipt,
            "Bounded output bundle for the first canonical plugin-conditioned SFT stage.",
        )?;
        bundle.validate_against_context(&dataset_bundle)?;
        Ok(())
    }

    #[test]
    fn plugin_conditioned_manifest_rejects_missing_benchmark_family(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dataset_bundle = build_psion_plugin_conditioned_dataset_bundle()?;
        let stage_program = plugin_conditioned_stage_program(&dataset_bundle)?;
        let full_benchmark_bindings = benchmark_bindings()?;
        let mut incomplete_benchmark_bindings = full_benchmark_bindings.clone();
        incomplete_benchmark_bindings.pop();
        let trace_bindings = trace_bindings(&dataset_bundle, &stage_program)?;
        let max_plugin_calls_per_trace = max_plugin_calls_per_trace(&trace_bindings);
        let error = record_psion_plugin_conditioned_sft_stage_manifest(
            &stage_program,
            &dataset_bundle,
            trace_bindings,
            incomplete_benchmark_bindings,
            eval_hooks(&full_benchmark_bindings),
            PsionPluginConditionedSftStageConfig {
                max_plugin_calls_per_trace,
                preserve_receipt_boundaries: true,
                require_replay_class_coverage: true,
                require_held_out_benchmark_hooks: true,
                detail: String::from("config"),
            },
            "summary",
        )
        .expect_err("manifest should reject missing benchmark family");
        assert!(error
            .to_string()
            .contains("plugin_conditioned_stage_manifest.benchmark_bindings.family"));
        Ok(())
    }

    fn benchmark_bindings(
    ) -> Result<Vec<super::PsionPluginConditionedBenchmarkBinding>, Box<dyn std::error::Error>>
    {
        Ok(vec![
            psion_plugin_discovery_selection_benchmark_binding(
                &build_psion_plugin_discovery_selection_benchmark_bundle()?,
            ),
            psion_plugin_argument_construction_benchmark_binding(
                &build_psion_plugin_argument_construction_benchmark_bundle()?,
            ),
            psion_plugin_sequencing_benchmark_binding(
                &build_psion_plugin_sequencing_benchmark_bundle()?,
            ),
            psion_plugin_refusal_request_structure_benchmark_binding(
                &build_psion_plugin_refusal_request_structure_benchmark_bundle()?,
            ),
            psion_plugin_result_interpretation_benchmark_binding(
                &build_psion_plugin_result_interpretation_benchmark_bundle()?,
            ),
        ])
    }

    fn eval_hooks(
        benchmark_bindings: &[super::PsionPluginConditionedBenchmarkBinding],
    ) -> Vec<PsionPluginConditionedEvalHook> {
        let mut hooks = benchmark_bindings
            .iter()
            .map(|binding| PsionPluginConditionedEvalHook {
                hook_id: format!(
                    "hook.psion.plugin_conditioned_sft.{}.post_stage",
                    format!("{:?}", binding.benchmark_family).to_lowercase()
                ),
                hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
                trigger: PsionPluginConditionedEvalTrigger::PostStageCompletion,
                benchmark_family: Some(binding.benchmark_family),
                benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
                benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
                execution_evidence_required: binding.execution_evidence_required,
                detail: format!(
                    "Run the {:?} held-out benchmark package immediately after stage completion.",
                    binding.benchmark_family
                ),
            })
            .collect::<Vec<_>>();
        if let Some(binding) = benchmark_bindings.first() {
            hooks.push(PsionPluginConditionedEvalHook {
                hook_id: String::from("hook.psion.plugin_conditioned_sft.pre_promotion_suite"),
                hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
                trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
                benchmark_family: Some(binding.benchmark_family),
                benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
                benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
                execution_evidence_required: binding.execution_evidence_required,
                detail: String::from(
                    "One benchmark family must be rerun before promotion so the stage contract preserves a machine-checkable held-out gate.",
                ),
            });
        }
        hooks.push(PsionPluginConditionedEvalHook {
            hook_id: String::from("hook.psion.plugin_conditioned_sft.replay_receipt_review"),
            hook_kind: PsionPluginConditionedEvalHookKind::ReplayReceiptReview,
            trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
            benchmark_family: None,
            benchmark_bundle_ref: None,
            benchmark_receipt_digest: None,
            execution_evidence_required: true,
            detail: String::from(
                "Review replay classes and runtime receipt references before promotion beyond the plugin-conditioned stage.",
            ),
        });
        hooks
    }

    fn plugin_conditioned_stage_program(
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
    ) -> Result<TrainingStageProgramState, Box<dyn std::error::Error>> {
        let environment = EnvironmentPackageKey::new("env.psion.plugin_conditioned", "2026.03.22");
        let mut program = TrainingStageProgramState::new(
            "run-psion-plugin-conditioned-reference",
            "train.psion.plugin_conditioned.reference",
        )?;
        program.start_initial_stage(environment.clone())?;
        program.ingest_trace(
            &TrainingSftTraceArtifact::new(
                "general-sft-bridge-trace",
                environment.clone(),
                TrainingSftTraceKind::LongContext,
                digest("general-sft-bridge-input"),
                digest("general-sft-bridge-output"),
            )
            .with_long_context_lineage(crate::TrainingLongContextTraceLineage::new(
                4096,
                vec![String::from("general_sft.bridge.segment")],
            )),
        )?;
        program.complete_current_stage()?;
        program.advance_stage(
            TrainingStageKind::AgenticSft,
            environment.clone(),
            checkpoint(1),
        )?;
        let train_records = dataset_bundle
            .split_rows
            .iter()
            .find(|split| split.split_kind == DatasetSplitKind::Train)
            .expect("train split should exist")
            .records
            .clone();
        for record in &train_records {
            program.ingest_trace(&training_trace(record))?;
        }
        program.complete_current_stage()?;
        Ok(program)
    }

    fn trace_bindings(
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
        _stage_program: &TrainingStageProgramState,
    ) -> Result<Vec<PsionPluginConditionedTraceBinding>, Box<dyn std::error::Error>> {
        let train_records = dataset_bundle
            .split_rows
            .iter()
            .find(|split| split.split_kind == DatasetSplitKind::Train)
            .expect("train split should exist")
            .records
            .clone();
        Ok(train_records
            .iter()
            .map(|record| {
                let trace = training_trace(record);
                let trace_id = trace.trace_id.clone();
                let trace_lineage_digest = trace.lineage_digest.clone();
                PsionPluginConditionedTraceBinding {
                    record_id: record.record_id.clone(),
                    trace_id,
                    trace_lineage_digest,
                    controller_surface: record.controller_context.controller_surface,
                    route_label: record.route_label,
                    outcome_label: record.outcome_label,
                    replay_class_ids: record
                        .admitted_plugins
                        .iter()
                        .map(|plugin| plugin.replay_class_id.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    receipt_refs: record
                        .plugin_invocations
                        .iter()
                        .map(|invocation| invocation.receipt_ref.clone())
                        .collect(),
                    detail: format!(
                        "Trace `{}` preserves the canonical plugin-conditioned record `{}` with explicit receipt refs and replay classes.",
                        trace.trace_id, record.record_id
                    ),
                }
            })
            .collect())
    }

    fn max_plugin_calls_per_trace(trace_bindings: &[PsionPluginConditionedTraceBinding]) -> u32 {
        trace_bindings
            .iter()
            .map(|binding| binding.receipt_refs.len() as u32)
            .max()
            .unwrap_or(1)
    }

    fn training_trace(
        record: &psionic_data::PsionPluginTrainingRecord,
    ) -> TrainingSftTraceArtifact {
        let steps = record
            .plugin_invocations
            .iter()
            .map(|invocation| TrainingToolCallTraceStep {
                tool_name: invocation.tool_name.clone(),
                arguments_digest: digest_value(&invocation.arguments),
                result_digest: invocation
                    .result_payload
                    .as_ref()
                    .map(digest_value)
                    .unwrap_or_else(|| {
                        digest(
                            invocation
                                .refusal_schema_id
                                .as_deref()
                                .unwrap_or("typed_refusal_or_runtime_boundary"),
                        )
                    }),
            })
            .collect::<Vec<_>>();
        TrainingSftTraceArtifact::new(
            format!("trace://{}", record.record_id),
            EnvironmentPackageKey::new("env.psion.plugin_conditioned", "2026.03.22"),
            TrainingSftTraceKind::ToolCall,
            digest(record.directive_text.as_str()),
            digest(
                record
                    .final_response_text
                    .as_deref()
                    .unwrap_or(record.detail.as_str()),
            ),
        )
        .with_session_digest(record.controller_context.source_bundle_digest.clone())
        .with_source_ref(record.record_id.clone())
        .with_tool_call_lineage(TrainingToolCallTraceLineage::new(steps))
    }

    fn checkpoint(step: u64) -> TrainingCheckpointReference {
        TrainingCheckpointReference::new(
            "train.psion.plugin_conditioned.reference",
            format!("stream-{step}"),
            format!("manifest-{step}"),
            format!("object-{step}"),
            "node-a",
            1,
            "cluster-digest",
            "topology-digest",
            1_000 + step,
        )
        .with_checkpoint_ref(format!("checkpoint://plugin-conditioned/{step}"))
        .with_step(step)
        .with_durable_at_ms(2_000 + step)
    }

    fn digest(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn digest_value(value: &Value) -> String {
        let encoded = serde_json::to_vec(value).expect("json value should serialize");
        let mut hasher = Sha256::new();
        hasher.update(&encoded);
        hex::encode(hasher.finalize())
    }
}
