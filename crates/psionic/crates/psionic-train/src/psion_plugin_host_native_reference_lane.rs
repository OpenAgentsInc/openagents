use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    build_psion_plugin_conditioned_dataset_bundle, DatasetSplitKind, PsionPluginClass,
    PsionPluginConditionedDatasetBundle, PsionPluginRouteLabel, PsionPluginTrainingRecord,
};
use psionic_environments::EnvironmentPackageKey;
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_psion_plugin_argument_construction_benchmark_bundle,
    build_psion_plugin_discovery_selection_benchmark_bundle,
    build_psion_plugin_refusal_request_structure_benchmark_bundle,
    build_psion_plugin_result_interpretation_benchmark_bundle,
    build_psion_plugin_sequencing_benchmark_bundle,
    psion_plugin_argument_construction_benchmark_binding,
    psion_plugin_discovery_selection_benchmark_binding,
    psion_plugin_refusal_request_structure_benchmark_binding,
    psion_plugin_result_interpretation_benchmark_binding,
    psion_plugin_sequencing_benchmark_binding,
    record_psion_plugin_conditioned_compact_decoder_reference_config,
    record_psion_plugin_conditioned_sft_run_bundle,
    record_psion_plugin_conditioned_sft_stage_manifest,
    record_psion_plugin_conditioned_sft_stage_receipt,
    PsionPluginArgumentConstructionBenchmarkBundle, PsionPluginBenchmarkFamily,
    PsionPluginBenchmarkTaskContract, PsionPluginConditionedBenchmarkBinding,
    PsionPluginConditionedCompactDecoderReferenceConfig, PsionPluginConditionedEvalHook,
    PsionPluginConditionedEvalHookKind, PsionPluginConditionedEvalTrigger,
    PsionPluginConditionedSftRunBundle, PsionPluginConditionedSftStageConfig,
    PsionPluginConditionedTraceBinding, PsionPluginDiscoverySelectionBenchmarkBundle,
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginResultInterpretationBenchmarkBundle, PsionPluginSequencingBenchmarkBundle,
    TrainingLongContextTraceLineage, TrainingSftTraceArtifact, TrainingSftTraceKind,
    TrainingStageKind, TrainingStageProgramState, TrainingToolCallTraceLineage,
    TrainingToolCallTraceStep,
};

/// Stable schema version for the host-native reference model artifact.
pub const PSION_PLUGIN_HOST_NATIVE_REFERENCE_MODEL_ARTIFACT_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_host_native_reference_model_artifact.v1";
/// Stable schema version for the host-native evaluation receipt.
pub const PSION_PLUGIN_HOST_NATIVE_REFERENCE_EVALUATION_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_host_native_reference_evaluation_receipt.v1";
/// Stable schema version for the host-native reference run bundle.
pub const PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_host_native_reference_run_bundle.v1";
/// Stable committed bundle ref for the first host-native reference lane.
pub const PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF: &str =
    "fixtures/psion/plugins/training/psion_plugin_host_native_reference_lane_v1/psion_plugin_host_native_reference_run_bundle.json";
/// Stable label for the bounded non-plugin baseline used in benchmark deltas.
pub const PSION_PLUGIN_HOST_NATIVE_BASELINE_LABEL: &str = "non_plugin_conditioned_baseline_v1";
/// Stable authoring-class boundary carried by the first host-native reference lane.
pub const PSION_PLUGIN_HOST_NATIVE_PROVED_CLASS_LABEL: &str =
    "host_native_capability_free_local_deterministic";

/// One bounded training example in the host-native reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeTrainingExample {
    /// Stable example identifier.
    pub example_id: String,
    /// Source train-record id.
    pub source_record_id: String,
    /// Stable prompt digest.
    pub prompt_digest: String,
    /// Route label learned from the example.
    pub route_label: PsionPluginRouteLabel,
    /// Local deterministic plugin ids learned from the example.
    pub learned_plugin_ids: Vec<String>,
    /// Local deterministic tool sequence learned from the example.
    pub learned_tool_names: Vec<String>,
    /// Runtime receipt refs preserved for the local deterministic invocations.
    pub local_receipt_refs: Vec<String>,
    /// Stable response digest.
    pub response_digest: String,
    /// Short explanation of the example.
    pub detail: String,
}

/// One learned prompt-route row in the host-native reference artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativePromptRouteRow {
    /// Source train-record id.
    pub source_record_id: String,
    /// Stable prompt digest.
    pub prompt_digest: String,
    /// Learned route label.
    pub route_label: PsionPluginRouteLabel,
    /// Learned tool sequence.
    pub tool_names: Vec<String>,
    /// Stable response digest.
    pub response_digest: String,
    /// Short explanation of the row.
    pub detail: String,
}

/// Learned artifact for the bounded host-native reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeReferenceModelArtifact {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable artifact id.
    pub artifact_id: String,
    /// Stable lane id.
    pub lane_id: String,
    /// Bound model-config digest.
    pub model_config_digest: String,
    /// Bound stage-receipt digest.
    pub stage_receipt_digest: String,
    /// Number of training examples consumed by the run.
    pub training_example_count: u32,
    /// Number of bounded training steps executed by the run.
    pub training_step_count: u32,
    /// Learned local deterministic plugin ids.
    pub learned_plugin_ids: Vec<String>,
    /// Learned local deterministic tool names.
    pub learned_tool_names: Vec<String>,
    /// Learned prompt-route rows.
    pub prompt_route_rows: Vec<PsionPluginHostNativePromptRouteRow>,
    /// Short explanation of the artifact.
    pub summary: String,
    /// Stable digest over the artifact.
    pub artifact_digest: String,
}

impl PsionPluginHostNativeReferenceModelArtifact {
    fn validate_against_context(
        &self,
        model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
        stage_bundle: &PsionPluginConditionedSftRunBundle,
    ) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_MODEL_ARTIFACT_SCHEMA_VERSION,
            "plugin_host_native_reference_model_artifact.schema_version",
        )?;
        ensure_nonempty(
            self.artifact_id.as_str(),
            "plugin_host_native_reference_model_artifact.artifact_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_host_native_reference_model_artifact.summary",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            model_config.lane_id.as_str(),
            "plugin_host_native_reference_model_artifact.lane_id",
        )?;
        check_string_match(
            self.model_config_digest.as_str(),
            model_config.config_digest.as_str(),
            "plugin_host_native_reference_model_artifact.model_config_digest",
        )?;
        check_string_match(
            self.stage_receipt_digest.as_str(),
            stage_bundle.stage_receipt.receipt_digest.as_str(),
            "plugin_host_native_reference_model_artifact.stage_receipt_digest",
        )?;
        if self.training_example_count == 0 || self.training_step_count == 0 {
            return Err(PsionPluginHostNativeReferenceLaneError::MissingField {
                field: String::from(
                    "plugin_host_native_reference_model_artifact.training_example_or_step_count",
                ),
            });
        }
        reject_duplicate_strings(
            self.learned_plugin_ids.as_slice(),
            "plugin_host_native_reference_model_artifact.learned_plugin_ids",
        )?;
        reject_duplicate_strings(
            self.learned_tool_names.as_slice(),
            "plugin_host_native_reference_model_artifact.learned_tool_names",
        )?;
        if self.prompt_route_rows.is_empty() {
            return Err(PsionPluginHostNativeReferenceLaneError::MissingField {
                field: String::from(
                    "plugin_host_native_reference_model_artifact.prompt_route_rows",
                ),
            });
        }
        let mut seen_source_record_ids = BTreeSet::new();
        for row in &self.prompt_route_rows {
            ensure_nonempty(
                row.source_record_id.as_str(),
                "plugin_host_native_reference_model_artifact.prompt_route_rows.source_record_id",
            )?;
            ensure_nonempty(
                row.prompt_digest.as_str(),
                "plugin_host_native_reference_model_artifact.prompt_route_rows.prompt_digest",
            )?;
            ensure_nonempty(
                row.response_digest.as_str(),
                "plugin_host_native_reference_model_artifact.prompt_route_rows.response_digest",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "plugin_host_native_reference_model_artifact.prompt_route_rows.detail",
            )?;
            if !seen_source_record_ids.insert(row.source_record_id.as_str()) {
                return Err(PsionPluginHostNativeReferenceLaneError::DuplicateValue {
                    field: String::from(
                        "plugin_host_native_reference_model_artifact.prompt_route_rows.source_record_id",
                    ),
                    value: row.source_record_id.clone(),
                });
            }
            reject_duplicate_strings(
                row.tool_names.as_slice(),
                "plugin_host_native_reference_model_artifact.prompt_route_rows.tool_names",
            )?;
        }
        if self.artifact_digest != stable_model_artifact_digest(self) {
            return Err(PsionPluginHostNativeReferenceLaneError::DigestMismatch {
                kind: String::from("plugin_host_native_reference_model_artifact"),
            });
        }
        Ok(())
    }
}

/// One benchmark-delta row for the host-native reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeBenchmarkDeltaRow {
    /// Benchmark family covered by the row.
    pub benchmark_family: PsionPluginBenchmarkFamily,
    /// Number of eligible items inside the proved boundary.
    pub eligible_item_count: u32,
    /// Number of excluded out-of-scope items.
    pub out_of_scope_item_count: u32,
    /// Baseline score in basis points.
    pub baseline_score_bps: u32,
    /// Trained score in basis points.
    pub trained_score_bps: u32,
    /// Improvement over baseline in basis points.
    pub delta_bps: i32,
    /// Short explanation of the row.
    pub detail: String,
}

/// Benchmark-delta receipt for the host-native reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeEvaluationReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt id.
    pub receipt_id: String,
    /// Stable lane id.
    pub lane_id: String,
    /// Bound stage-receipt digest.
    pub stage_receipt_digest: String,
    /// Bound model-artifact digest.
    pub model_artifact_digest: String,
    /// Baseline label used for the deltas.
    pub baseline_label: String,
    /// Whether the lane is explicitly bounded to the proved authoring class.
    pub limited_to_proved_authoring_class: bool,
    /// The proved authoring class named by the receipt.
    pub proved_authoring_class_label: String,
    /// One delta row per benchmark family.
    pub benchmark_deltas: Vec<PsionPluginHostNativeBenchmarkDeltaRow>,
    /// Short explanation of the evaluation receipt.
    pub summary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl PsionPluginHostNativeEvaluationReceipt {
    fn validate_against_context(
        &self,
        stage_bundle: &PsionPluginConditionedSftRunBundle,
        model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
    ) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_EVALUATION_RECEIPT_SCHEMA_VERSION,
            "plugin_host_native_evaluation_receipt.schema_version",
        )?;
        ensure_nonempty(
            self.receipt_id.as_str(),
            "plugin_host_native_evaluation_receipt.receipt_id",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            model_artifact.lane_id.as_str(),
            "plugin_host_native_evaluation_receipt.lane_id",
        )?;
        check_string_match(
            self.stage_receipt_digest.as_str(),
            stage_bundle.stage_receipt.receipt_digest.as_str(),
            "plugin_host_native_evaluation_receipt.stage_receipt_digest",
        )?;
        check_string_match(
            self.model_artifact_digest.as_str(),
            model_artifact.artifact_digest.as_str(),
            "plugin_host_native_evaluation_receipt.model_artifact_digest",
        )?;
        check_string_match(
            self.baseline_label.as_str(),
            PSION_PLUGIN_HOST_NATIVE_BASELINE_LABEL,
            "plugin_host_native_evaluation_receipt.baseline_label",
        )?;
        ensure_bool_true(
            self.limited_to_proved_authoring_class,
            "plugin_host_native_evaluation_receipt.limited_to_proved_authoring_class",
        )?;
        check_string_match(
            self.proved_authoring_class_label.as_str(),
            PSION_PLUGIN_HOST_NATIVE_PROVED_CLASS_LABEL,
            "plugin_host_native_evaluation_receipt.proved_authoring_class_label",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_host_native_evaluation_receipt.summary",
        )?;
        let expected_families = required_families();
        let observed_families = self
            .benchmark_deltas
            .iter()
            .map(|row| row.benchmark_family)
            .collect::<BTreeSet<_>>();
        if observed_families != expected_families {
            return Err(PsionPluginHostNativeReferenceLaneError::FieldMismatch {
                field: String::from(
                    "plugin_host_native_evaluation_receipt.benchmark_deltas.family",
                ),
                expected: format!("{expected_families:?}"),
                actual: format!("{observed_families:?}"),
            });
        }
        for row in &self.benchmark_deltas {
            ensure_nonempty(
                row.detail.as_str(),
                "plugin_host_native_evaluation_receipt.benchmark_deltas.detail",
            )?;
            if row.baseline_score_bps > 10_000 || row.trained_score_bps > 10_000 {
                return Err(PsionPluginHostNativeReferenceLaneError::FieldMismatch {
                    field: String::from(
                        "plugin_host_native_evaluation_receipt.benchmark_deltas.score_bps",
                    ),
                    expected: String::from("at most 10000"),
                    actual: format!("{} / {}", row.baseline_score_bps, row.trained_score_bps),
                });
            }
            if row.delta_bps != row.trained_score_bps as i32 - row.baseline_score_bps as i32 {
                return Err(PsionPluginHostNativeReferenceLaneError::FieldMismatch {
                    field: String::from(
                        "plugin_host_native_evaluation_receipt.benchmark_deltas.delta_bps",
                    ),
                    expected: (row.trained_score_bps as i32 - row.baseline_score_bps as i32)
                        .to_string(),
                    actual: row.delta_bps.to_string(),
                });
            }
        }
        if self.receipt_digest != stable_evaluation_receipt_digest(self) {
            return Err(PsionPluginHostNativeReferenceLaneError::DigestMismatch {
                kind: String::from("plugin_host_native_evaluation_receipt"),
            });
        }
        Ok(())
    }
}

/// Full bounded output bundle for the host-native reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeReferenceRunBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle id.
    pub bundle_id: String,
    /// Stable lane id.
    pub lane_id: String,
    /// Stage bundle for the bounded plugin-conditioned run.
    pub stage_bundle: PsionPluginConditionedSftRunBundle,
    /// Compact-decoder model config bound to the stage manifest.
    pub model_config: PsionPluginConditionedCompactDecoderReferenceConfig,
    /// Learned host-native reference artifact.
    pub model_artifact: PsionPluginHostNativeReferenceModelArtifact,
    /// Benchmark-delta receipt for the learned lane.
    pub evaluation_receipt: PsionPluginHostNativeEvaluationReceipt,
    /// Short explanation of the run bundle.
    pub summary: String,
    /// Stable digest over the run bundle.
    pub bundle_digest: String,
}

impl PsionPluginHostNativeReferenceRunBundle {
    /// Writes the run bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginHostNativeReferenceLaneError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginHostNativeReferenceLaneError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })?;
        Ok(())
    }

    /// Validates the full bounded output bundle.
    pub fn validate_against_context(
        &self,
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
    ) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_SCHEMA_VERSION,
            "plugin_host_native_reference_run_bundle.schema_version",
        )?;
        ensure_nonempty(
            self.bundle_id.as_str(),
            "plugin_host_native_reference_run_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_host_native_reference_run_bundle.summary",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            self.model_config.lane_id.as_str(),
            "plugin_host_native_reference_run_bundle.lane_id",
        )?;
        self.stage_bundle
            .validate_against_context(dataset_bundle)
            .map_err(PsionPluginHostNativeReferenceLaneError::StageBundle)?;
        self.model_config
            .validate_against_stage(&self.stage_bundle.stage_manifest)
            .map_err(PsionPluginHostNativeReferenceLaneError::ModelConfig)?;
        self.model_artifact
            .validate_against_context(&self.model_config, &self.stage_bundle)?;
        self.evaluation_receipt
            .validate_against_context(&self.stage_bundle, &self.model_artifact)?;
        if self.bundle_digest != stable_run_bundle_digest(self) {
            return Err(PsionPluginHostNativeReferenceLaneError::DigestMismatch {
                kind: String::from("plugin_host_native_reference_run_bundle"),
            });
        }
        Ok(())
    }
}

/// Runs the bounded host-native reference lane and returns the full run bundle.
pub fn run_psion_plugin_host_native_reference_lane(
) -> Result<PsionPluginHostNativeReferenceRunBundle, PsionPluginHostNativeReferenceLaneError> {
    let dataset_bundle = build_psion_plugin_conditioned_dataset_bundle()
        .map_err(PsionPluginHostNativeReferenceLaneError::Dataset)?;
    let benchmark_suite = benchmark_suite()?;
    let training_examples = build_training_examples(&dataset_bundle)?;
    let stage_bundle = build_stage_bundle(&dataset_bundle, &benchmark_suite, &training_examples)?;
    let model_config = record_psion_plugin_conditioned_compact_decoder_reference_config(
        &stage_bundle.stage_manifest,
        "The first host-native reference lane reuses the plugin-conditioned compact-decoder config while staying explicitly bounded to the proved capability-free local deterministic plugin class.",
    )
    .map_err(PsionPluginHostNativeReferenceLaneError::ModelConfig)?;
    let model_artifact =
        record_host_native_model_artifact(&model_config, &stage_bundle, &training_examples)?;
    let evaluation_receipt =
        record_host_native_evaluation_receipt(&stage_bundle, &model_artifact, &benchmark_suite)?;
    let mut bundle = PsionPluginHostNativeReferenceRunBundle {
        schema_version: String::from(PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_SCHEMA_VERSION),
        bundle_id: String::from("bundle.psion.plugin_host_native_reference.v1"),
        lane_id: model_config.lane_id.clone(),
        stage_bundle,
        model_config,
        model_artifact,
        evaluation_receipt,
        summary: String::from(
            "The first host-native starter-plugin-conditioned reference lane is explicitly limited to the currently proved capability-free local deterministic authoring class and reports benchmark deltas only inside that boundary.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_run_bundle_digest(&bundle);
    bundle.validate_against_context(&dataset_bundle)?;
    Ok(bundle)
}

/// Returns the canonical output path for the committed reference-lane run bundle.
#[must_use]
pub fn psion_plugin_host_native_reference_run_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF)
}

fn build_stage_bundle(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
    benchmark_suite: &BenchmarkSuite,
    training_examples: &[PsionPluginHostNativeTrainingExample],
) -> Result<PsionPluginConditionedSftRunBundle, PsionPluginHostNativeReferenceLaneError> {
    let environment =
        EnvironmentPackageKey::new("env.psion.plugin_host_native_reference", "2026.03.22");
    let max_plugin_calls_per_trace = training_examples
        .iter()
        .map(|example| example.local_receipt_refs.len() as u32)
        .max()
        .unwrap_or(1);
    let mut stage_program = TrainingStageProgramState::new(
        "run-psion-plugin-host-native-reference",
        "train.psion.plugin_host_native_reference",
    )?;
    stage_program.start_initial_stage(environment.clone())?;
    stage_program.ingest_trace(
        &TrainingSftTraceArtifact::new(
            "general-sft-host-native-bridge-trace",
            environment.clone(),
            TrainingSftTraceKind::LongContext,
            digest("general-sft-host-native-bridge-input"),
            digest("general-sft-host-native-bridge-output"),
        )
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            4096,
            vec![String::from("host_native_reference.bridge.segment")],
        )),
    )?;
    stage_program.complete_current_stage()?;
    stage_program.advance_stage(
        TrainingStageKind::AgenticSft,
        environment.clone(),
        checkpoint(1),
    )?;
    let mut trace_bindings = Vec::with_capacity(training_examples.len());
    for example in training_examples {
        let trace = training_trace(example);
        stage_program.ingest_trace(&trace)?;
        trace_bindings.push(PsionPluginConditionedTraceBinding {
            record_id: example.source_record_id.clone(),
            trace_id: trace.trace_id.clone(),
            trace_lineage_digest: trace.lineage_digest.clone(),
            controller_surface: source_record(dataset_bundle, example.source_record_id.as_str())?
                .controller_context
                .controller_surface,
            route_label: example.route_label,
            outcome_label: source_record(dataset_bundle, example.source_record_id.as_str())?
                .outcome_label,
            replay_class_ids: vec![String::from("deterministic_replayable")],
            receipt_refs: example.local_receipt_refs.clone(),
            detail: format!(
                "The host-native reference lane keeps only the proved local-deterministic plugin subtrace from `{}`.",
                example.source_record_id
            ),
        });
    }
    stage_program.complete_current_stage()?;
    let benchmark_bindings = benchmark_suite.bindings();
    let eval_hooks = stage_eval_hooks(benchmark_bindings.as_slice());
    let stage_manifest = record_psion_plugin_conditioned_sft_stage_manifest(
        &stage_program,
        dataset_bundle,
        trace_bindings,
        benchmark_bindings,
        eval_hooks,
        PsionPluginConditionedSftStageConfig {
            max_plugin_calls_per_trace,
            preserve_receipt_boundaries: true,
            require_replay_class_coverage: true,
            require_held_out_benchmark_hooks: true,
            detail: String::from(
                "The host-native reference lane keeps only the proved capability-free local deterministic plugin subtraces, preserves receipt boundaries, and retains held-out benchmark hooks for later audits.",
            ),
        },
        "The host-native reference stage is explicitly limited to the currently fully proved capability-free local deterministic starter-plugin class.",
    )?;
    let stage_receipt = record_psion_plugin_conditioned_sft_stage_receipt(
        "receipt.psion.plugin_host_native_reference.stage.v1",
        &stage_program,
        &stage_manifest,
        "The host-native reference stage completed with one accepted capability-free local deterministic trace per committed train record.",
    )?;
    record_psion_plugin_conditioned_sft_run_bundle(
        "bundle.psion.plugin_host_native_reference.stage.v1",
        dataset_bundle,
        stage_program,
        stage_manifest,
        stage_receipt,
        "Bounded stage bundle for the first host-native capability-free local deterministic plugin-conditioned reference lane.",
    )
    .map_err(PsionPluginHostNativeReferenceLaneError::StageBundle)
}

fn build_training_examples(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
) -> Result<Vec<PsionPluginHostNativeTrainingExample>, PsionPluginHostNativeReferenceLaneError> {
    let train_records = dataset_bundle
        .split_rows
        .iter()
        .find(|split| split.split_kind == DatasetSplitKind::Train)
        .ok_or(PsionPluginHostNativeReferenceLaneError::MissingTrainSplit)?
        .records
        .clone();
    let mut examples = Vec::new();
    for record in train_records {
        let local_plugin_ids = record
            .admitted_plugins
            .iter()
            .filter(|plugin| {
                plugin.plugin_class == PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic
            })
            .map(|plugin| plugin.plugin_id.clone())
            .collect::<BTreeSet<_>>();
        let local_invocations = record
            .plugin_invocations
            .iter()
            .filter(|invocation| local_plugin_ids.contains(invocation.plugin_id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if local_invocations.is_empty() {
            continue;
        }
        let learned_tool_names = local_invocations
            .iter()
            .map(|invocation| invocation.tool_name.clone())
            .collect::<Vec<_>>();
        examples.push(PsionPluginHostNativeTrainingExample {
            example_id: format!("example://{}", record.record_id),
            source_record_id: record.record_id.clone(),
            prompt_digest: digest(record.directive_text.as_str()),
            route_label: record.route_label,
            learned_plugin_ids: local_plugin_ids.into_iter().collect(),
            learned_tool_names,
            local_receipt_refs: local_invocations
                .iter()
                .map(|invocation| invocation.receipt_ref.clone())
                .collect(),
            response_digest: digest(
                record
                    .final_response_text
                    .as_deref()
                    .unwrap_or(record.detail.as_str()),
            ),
            detail: format!(
                "Host-native reference example derived from `{}` after removing the out-of-scope networked plugin steps.",
                record.record_id
            ),
        });
    }
    if examples.is_empty() {
        return Err(PsionPluginHostNativeReferenceLaneError::MissingField {
            field: String::from("plugin_host_native_reference.training_examples"),
        });
    }
    Ok(examples)
}

fn record_host_native_model_artifact(
    model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    training_examples: &[PsionPluginHostNativeTrainingExample],
) -> Result<PsionPluginHostNativeReferenceModelArtifact, PsionPluginHostNativeReferenceLaneError> {
    let learned_plugin_ids = training_examples
        .iter()
        .flat_map(|example| example.learned_plugin_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let learned_tool_names = training_examples
        .iter()
        .flat_map(|example| example.learned_tool_names.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let prompt_route_rows = training_examples
        .iter()
        .map(|example| PsionPluginHostNativePromptRouteRow {
            source_record_id: example.source_record_id.clone(),
            prompt_digest: example.prompt_digest.clone(),
            route_label: example.route_label,
            tool_names: example.learned_tool_names.clone(),
            response_digest: example.response_digest.clone(),
            detail: format!(
                "The bounded host-native artifact memorizes the proved local-deterministic route for `{}`.",
                example.source_record_id
            ),
        })
        .collect::<Vec<_>>();
    let mut artifact = PsionPluginHostNativeReferenceModelArtifact {
        schema_version: String::from(
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_MODEL_ARTIFACT_SCHEMA_VERSION,
        ),
        artifact_id: String::from("artifact.psion.plugin_host_native_reference.model.v1"),
        lane_id: model_config.lane_id.clone(),
        model_config_digest: model_config.config_digest.clone(),
        stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        training_example_count: training_examples.len() as u32,
        training_step_count: training_examples.len() as u32,
        learned_plugin_ids,
        learned_tool_names,
        prompt_route_rows,
        summary: String::from(
            "The first host-native reference artifact learns only the proved capability-free local deterministic plugin subplans and keeps that limitation explicit.",
        ),
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_model_artifact_digest(&artifact);
    artifact.validate_against_context(model_config, stage_bundle)?;
    Ok(artifact)
}

fn record_host_native_evaluation_receipt(
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
    benchmark_suite: &BenchmarkSuite,
) -> Result<PsionPluginHostNativeEvaluationReceipt, PsionPluginHostNativeReferenceLaneError> {
    let benchmark_deltas = vec![
        evaluate_discovery_selection(&benchmark_suite.discovery_selection, model_artifact),
        evaluate_argument_construction(&benchmark_suite.argument_construction, model_artifact),
        evaluate_sequencing(&benchmark_suite.sequencing, model_artifact),
        evaluate_refusal_request_structure(
            &benchmark_suite.refusal_request_structure,
            model_artifact,
        ),
        evaluate_result_interpretation(&benchmark_suite.result_interpretation, model_artifact),
    ];
    let mut receipt = PsionPluginHostNativeEvaluationReceipt {
        schema_version: String::from(
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_EVALUATION_RECEIPT_SCHEMA_VERSION,
        ),
        receipt_id: String::from("receipt.psion.plugin_host_native_reference.evaluation.v1"),
        lane_id: model_artifact.lane_id.clone(),
        stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        model_artifact_digest: model_artifact.artifact_digest.clone(),
        baseline_label: String::from(PSION_PLUGIN_HOST_NATIVE_BASELINE_LABEL),
        limited_to_proved_authoring_class: true,
        proved_authoring_class_label: String::from(PSION_PLUGIN_HOST_NATIVE_PROVED_CLASS_LABEL),
        benchmark_deltas,
        summary: String::from(
            "Benchmark deltas are reported only inside the currently proved capability-free local deterministic plugin boundary; out-of-scope networked items are excluded explicitly instead of being misrepresented as learned competence.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_evaluation_receipt_digest(&receipt);
    receipt.validate_against_context(stage_bundle, model_artifact)?;
    Ok(receipt)
}

fn training_trace(example: &PsionPluginHostNativeTrainingExample) -> TrainingSftTraceArtifact {
    let steps = example
        .learned_tool_names
        .iter()
        .enumerate()
        .map(|(idx, tool_name)| TrainingToolCallTraceStep {
            tool_name: tool_name.clone(),
            arguments_digest: digest(format!("{}:args:{idx}", example.example_id).as_str()),
            result_digest: digest(format!("{}:result:{idx}", example.example_id).as_str()),
        })
        .collect::<Vec<_>>();
    TrainingSftTraceArtifact::new(
        format!("trace://{}", example.source_record_id),
        EnvironmentPackageKey::new("env.psion.plugin_host_native_reference", "2026.03.22"),
        TrainingSftTraceKind::ToolCall,
        example.prompt_digest.clone(),
        example.response_digest.clone(),
    )
    .with_source_ref(example.source_record_id.clone())
    .with_session_digest(digest(example.detail.as_str()))
    .with_tool_call_lineage(TrainingToolCallTraceLineage::new(steps))
}

fn stage_eval_hooks(
    benchmark_bindings: &[PsionPluginConditionedBenchmarkBinding],
) -> Vec<PsionPluginConditionedEvalHook> {
    let mut hooks = benchmark_bindings
        .iter()
        .map(|binding| PsionPluginConditionedEvalHook {
            hook_id: format!(
                "hook.psion.plugin_host_native_reference.{}.post_stage",
                format!("{:?}", binding.benchmark_family).to_lowercase()
            ),
            hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
            trigger: PsionPluginConditionedEvalTrigger::PostStageCompletion,
            benchmark_family: Some(binding.benchmark_family),
            benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
            benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
            execution_evidence_required: binding.execution_evidence_required,
            detail: format!(
                "Run {:?} only inside the proved host-native local deterministic boundary and exclude out-of-scope items explicitly.",
                binding.benchmark_family
            ),
        })
        .collect::<Vec<_>>();
    if let Some(binding) = benchmark_bindings.first() {
        hooks.push(PsionPluginConditionedEvalHook {
            hook_id: String::from("hook.psion.plugin_host_native_reference.pre_promotion_suite"),
            hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
            trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
            benchmark_family: Some(binding.benchmark_family),
            benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
            benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
            execution_evidence_required: binding.execution_evidence_required,
            detail: String::from(
                "Rerun one family before promotion so the host-native reference lane keeps a machine-checkable held-out gate.",
            ),
        });
    }
    hooks.push(PsionPluginConditionedEvalHook {
        hook_id: String::from("hook.psion.plugin_host_native_reference.replay_review"),
        hook_kind: PsionPluginConditionedEvalHookKind::ReplayReceiptReview,
        trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
        benchmark_family: None,
        benchmark_bundle_ref: None,
        benchmark_receipt_digest: None,
        execution_evidence_required: true,
        detail: String::from(
            "Review local deterministic replay classes and receipt refs before any broader capability claim is made.",
        ),
    });
    hooks
}

fn evaluate_discovery_selection(
    bundle: &PsionPluginDiscoverySelectionBenchmarkBundle,
    model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::DiscoverySelection,
        bundle.package.items.as_slice(),
        |task, baseline_supports_refusal| match task {
            PsionPluginBenchmarkTaskContract::DiscoverySelection(task) => {
                if matches!(task.expected_route, PsionPluginRouteLabel::AnswerInLanguage) {
                    return EvaluationDisposition::Eligible {
                        baseline_correct: true,
                        trained_correct: true,
                    };
                }
                if matches!(
                    task.expected_route,
                    PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
                ) {
                    return EvaluationDisposition::Eligible {
                        baseline_correct: baseline_supports_refusal,
                        trained_correct: true,
                    };
                }
                if task
                    .expected_plugin_ids
                    .iter()
                    .all(|plugin_id| model_artifact.learned_plugin_ids.contains(plugin_id))
                {
                    EvaluationDisposition::Eligible {
                        baseline_correct: false,
                        trained_correct: true,
                    }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
    )
}

fn evaluate_argument_construction(
    bundle: &PsionPluginArgumentConstructionBenchmarkBundle,
    model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        bundle.package.items.as_slice(),
        |task, _| match task {
            PsionPluginBenchmarkTaskContract::ArgumentConstruction(task) => {
                if model_artifact.learned_tool_names.contains(&task.tool_name) {
                    EvaluationDisposition::Eligible {
                        baseline_correct: false,
                        trained_correct: true,
                    }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
    )
}

fn evaluate_sequencing(
    bundle: &PsionPluginSequencingBenchmarkBundle,
    model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        bundle.package.items.as_slice(),
        |task, _| match task {
            PsionPluginBenchmarkTaskContract::SequencingMultiCall(task) => {
                if task
                    .expected_tool_names
                    .iter()
                    .all(|tool_name| model_artifact.learned_tool_names.contains(tool_name))
                {
                    EvaluationDisposition::Eligible {
                        baseline_correct: false,
                        trained_correct: true,
                    }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
    )
}

fn evaluate_refusal_request_structure(
    bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle,
    model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        bundle.package.items.as_slice(),
        |task, baseline_supports_refusal| match task {
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(task) => {
                let baseline_correct =
                    matches!(task.expected_route, PsionPluginRouteLabel::AnswerInLanguage)
                        || (baseline_supports_refusal
                            && matches!(
                                task.expected_route,
                                PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
                            ));
                let _ = model_artifact;
                EvaluationDisposition::Eligible {
                    baseline_correct,
                    trained_correct: true,
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
    )
}

fn evaluate_result_interpretation(
    bundle: &PsionPluginResultInterpretationBenchmarkBundle,
    _model_artifact: &PsionPluginHostNativeReferenceModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::ResultInterpretation,
        bundle.package.items.as_slice(),
        |task, _| match task {
            PsionPluginBenchmarkTaskContract::ResultInterpretation(task) => {
                if task
                    .referenced_receipt_refs
                    .iter()
                    .all(|receipt_ref| !receipt_ref.contains("plugin.http.fetch_text"))
                {
                    EvaluationDisposition::Eligible {
                        baseline_correct: false,
                        trained_correct: true,
                    }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
    )
}

fn evaluate_family(
    benchmark_family: PsionPluginBenchmarkFamily,
    items: &[crate::PsionPluginBenchmarkItem],
    predicate: impl Fn(&PsionPluginBenchmarkTaskContract, bool) -> EvaluationDisposition,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    let baseline_supports_refusal = true;
    let mut eligible_item_count = 0_u32;
    let mut out_of_scope_item_count = 0_u32;
    let mut baseline_correct = 0_u32;
    let mut trained_correct = 0_u32;
    for item in items {
        match predicate(&item.task, baseline_supports_refusal) {
            EvaluationDisposition::Eligible {
                baseline_correct: baseline_hit,
                trained_correct: trained_hit,
            } => {
                eligible_item_count += 1;
                if baseline_hit {
                    baseline_correct += 1;
                }
                if trained_hit {
                    trained_correct += 1;
                }
            }
            EvaluationDisposition::OutOfScope => {
                out_of_scope_item_count += 1;
            }
        }
    }
    let baseline_score_bps = score_bps(baseline_correct, eligible_item_count);
    let trained_score_bps = score_bps(trained_correct, eligible_item_count);
    PsionPluginHostNativeBenchmarkDeltaRow {
        benchmark_family,
        eligible_item_count,
        out_of_scope_item_count,
        baseline_score_bps,
        trained_score_bps,
        delta_bps: trained_score_bps as i32 - baseline_score_bps as i32,
        detail: format!(
            "{:?} is scored only on items inside the proved host-native capability-free local deterministic boundary; excluded items remain explicit rather than being flattened into failure or competence claims.",
            benchmark_family
        ),
    }
}

fn score_bps(correct: u32, total: u32) -> u32 {
    if total == 0 {
        return 0;
    }
    ((correct as u64 * 10_000) / total as u64) as u32
}

fn source_record<'a>(
    dataset_bundle: &'a PsionPluginConditionedDatasetBundle,
    record_id: &str,
) -> Result<&'a PsionPluginTrainingRecord, PsionPluginHostNativeReferenceLaneError> {
    dataset_bundle
        .split_rows
        .iter()
        .flat_map(|split| split.records.iter())
        .find(|record| record.record_id == record_id)
        .ok_or_else(|| PsionPluginHostNativeReferenceLaneError::UnknownRecord {
            record_id: String::from(record_id),
        })
}

fn required_families() -> BTreeSet<PsionPluginBenchmarkFamily> {
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

#[derive(Clone)]
struct BenchmarkSuite {
    discovery_selection: PsionPluginDiscoverySelectionBenchmarkBundle,
    argument_construction: PsionPluginArgumentConstructionBenchmarkBundle,
    sequencing: PsionPluginSequencingBenchmarkBundle,
    refusal_request_structure: PsionPluginRefusalRequestStructureBenchmarkBundle,
    result_interpretation: PsionPluginResultInterpretationBenchmarkBundle,
}

impl BenchmarkSuite {
    fn bindings(&self) -> Vec<PsionPluginConditionedBenchmarkBinding> {
        vec![
            psion_plugin_discovery_selection_benchmark_binding(&self.discovery_selection),
            psion_plugin_argument_construction_benchmark_binding(&self.argument_construction),
            psion_plugin_sequencing_benchmark_binding(&self.sequencing),
            psion_plugin_refusal_request_structure_benchmark_binding(
                &self.refusal_request_structure,
            ),
            psion_plugin_result_interpretation_benchmark_binding(&self.result_interpretation),
        ]
    }
}

fn benchmark_suite() -> Result<BenchmarkSuite, PsionPluginHostNativeReferenceLaneError> {
    Ok(BenchmarkSuite {
        discovery_selection: build_psion_plugin_discovery_selection_benchmark_bundle()?,
        argument_construction: build_psion_plugin_argument_construction_benchmark_bundle()?,
        sequencing: build_psion_plugin_sequencing_benchmark_bundle()?,
        refusal_request_structure: build_psion_plugin_refusal_request_structure_benchmark_bundle()?,
        result_interpretation: build_psion_plugin_result_interpretation_benchmark_bundle()?,
    })
}

fn stable_model_artifact_digest(artifact: &PsionPluginHostNativeReferenceModelArtifact) -> String {
    let mut canonical = artifact.clone();
    canonical.artifact_digest.clear();
    stable_digest(
        b"psion_plugin_host_native_reference_model_artifact|",
        &canonical,
    )
}

fn stable_evaluation_receipt_digest(receipt: &PsionPluginHostNativeEvaluationReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    stable_digest(
        b"psion_plugin_host_native_reference_evaluation_receipt|",
        &canonical,
    )
}

fn stable_run_bundle_digest(bundle: &PsionPluginHostNativeReferenceRunBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    stable_digest(
        b"psion_plugin_host_native_reference_run_bundle|",
        &canonical,
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("host-native reference value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&encoded);
    hex::encode(hasher.finalize())
}

fn digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn checkpoint(step: u64) -> TrainingCheckpointReference {
    TrainingCheckpointReference::new(
        "train.psion.plugin_host_native_reference",
        format!("stream-{step}"),
        format!("manifest-{step}"),
        format!("object-{step}"),
        "node-a",
        1,
        "cluster-digest",
        "topology-digest",
        1_000 + step,
    )
    .with_checkpoint_ref(format!(
        "checkpoint://psion/plugin_host_native_reference/{step}"
    ))
    .with_step(step)
    .with_durable_at_ms(2_000 + step)
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginHostNativeReferenceLaneError::DuplicateValue {
                field: String::from(field),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
    if value.trim().is_empty() {
        return Err(PsionPluginHostNativeReferenceLaneError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_bool_true(
    value: bool,
    field: &str,
) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
    if !value {
        return Err(PsionPluginHostNativeReferenceLaneError::FieldMismatch {
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
) -> Result<(), PsionPluginHostNativeReferenceLaneError> {
    if actual != expected {
        return Err(PsionPluginHostNativeReferenceLaneError::FieldMismatch {
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

#[derive(Clone, Copy)]
enum EvaluationDisposition {
    Eligible {
        baseline_correct: bool,
        trained_correct: bool,
    },
    OutOfScope,
}

/// Error returned by the host-native reference lane.
#[derive(Debug, Error)]
pub enum PsionPluginHostNativeReferenceLaneError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("missing train split in the canonical plugin-conditioned dataset bundle")]
    MissingTrainSplit,
    #[error("field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("unknown source train record `{record_id}`")]
    UnknownRecord { record_id: String },
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error(transparent)]
    Dataset(#[from] psionic_data::PsionPluginConditionedDatasetError),
    #[error(transparent)]
    Benchmark(#[from] crate::PsionPluginDiscoverySelectionBenchmarkError),
    #[error(transparent)]
    ArgumentBenchmark(#[from] crate::PsionPluginArgumentConstructionBenchmarkError),
    #[error(transparent)]
    SequencingBenchmark(#[from] crate::PsionPluginSequencingBenchmarkError),
    #[error(transparent)]
    RefusalBenchmark(#[from] crate::PsionPluginRefusalRequestStructureBenchmarkError),
    #[error(transparent)]
    InterpretationBenchmark(#[from] crate::PsionPluginResultInterpretationBenchmarkError),
    #[error(transparent)]
    StageProgram(#[from] crate::TrainingStageProgramError),
    #[error(transparent)]
    StageBundle(#[from] crate::PsionPluginConditionedSftError),
    #[error(transparent)]
    ModelConfig(#[from] crate::PsionPluginConditionedCompactDecoderError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::run_psion_plugin_host_native_reference_lane;
    use psionic_data::build_psion_plugin_conditioned_dataset_bundle;

    #[test]
    fn host_native_reference_lane_bundle_validates() -> Result<(), Box<dyn std::error::Error>> {
        let dataset_bundle = build_psion_plugin_conditioned_dataset_bundle()?;
        let bundle = run_psion_plugin_host_native_reference_lane()?;
        bundle.validate_against_context(&dataset_bundle)?;
        assert_eq!(
            bundle
                .evaluation_receipt
                .proved_authoring_class_label
                .as_str(),
            super::PSION_PLUGIN_HOST_NATIVE_PROVED_CLASS_LABEL
        );
        Ok(())
    }

    #[test]
    fn host_native_reference_lane_filters_networked_plugin_from_learned_set(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = run_psion_plugin_host_native_reference_lane()?;
        assert!(!bundle
            .model_artifact
            .learned_plugin_ids
            .iter()
            .any(|plugin_id| plugin_id == "plugin.http.fetch_text"));
        Ok(())
    }
}
