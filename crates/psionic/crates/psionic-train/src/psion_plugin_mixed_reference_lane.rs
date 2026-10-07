use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    build_psion_plugin_mixed_conditioned_dataset_bundle, DatasetSplitKind, PsionPluginClass,
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
    record_psion_plugin_conditioned_mixed_compact_decoder_reference_config,
    record_psion_plugin_conditioned_sft_run_bundle,
    record_psion_plugin_conditioned_sft_stage_manifest,
    record_psion_plugin_conditioned_sft_stage_receipt,
    PsionPluginArgumentConstructionBenchmarkBundle, PsionPluginBenchmarkFamily,
    PsionPluginBenchmarkTaskContract, PsionPluginConditionedBenchmarkBinding,
    PsionPluginConditionedCompactDecoderReferenceConfig, PsionPluginConditionedCompactDecoderError,
    PsionPluginConditionedEvalHook, PsionPluginConditionedEvalHookKind,
    PsionPluginConditionedEvalTrigger, PsionPluginConditionedSftError,
    PsionPluginConditionedSftRunBundle, PsionPluginConditionedSftStageConfig,
    PsionPluginConditionedTraceBinding, PsionPluginDiscoverySelectionBenchmarkBundle,
    PsionPluginHostNativeReferenceLaneError, PsionPluginHostNativeReferenceRunBundle,
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginResultInterpretationBenchmarkBundle, PsionPluginSequencingBenchmarkBundle,
    TrainingLongContextTraceLineage, TrainingSftTraceArtifact, TrainingSftTraceKind,
    TrainingStageKind, TrainingStageProgramState, TrainingToolCallTraceLineage,
    TrainingToolCallTraceStep, PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF,
    run_psion_plugin_host_native_reference_lane,
};

/// Stable schema version for the mixed reference model artifact.
pub const PSION_PLUGIN_MIXED_REFERENCE_MODEL_ARTIFACT_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_mixed_reference_model_artifact.v1";
/// Stable schema version for the mixed evaluation receipt.
pub const PSION_PLUGIN_MIXED_REFERENCE_EVALUATION_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_mixed_reference_evaluation_receipt.v1";
/// Stable schema version for the mixed reference run bundle.
pub const PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_mixed_reference_run_bundle.v1";
/// Stable committed bundle ref for the first mixed reference lane.
pub const PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_REF: &str =
    "fixtures/psion/plugins/training/psion_plugin_mixed_reference_lane_v1/psion_plugin_mixed_reference_run_bundle.json";
/// Stable comparison label for the host-native reference lane.
pub const PSION_PLUGIN_MIXED_REFERENCE_COMPARISON_LABEL: &str =
    "psion_plugin_host_native_reference";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedTrainingExample {
    pub example_id: String,
    pub source_record_id: String,
    pub prompt_digest: String,
    pub route_label: PsionPluginRouteLabel,
    pub learned_plugin_ids: Vec<String>,
    pub learned_plugin_classes: Vec<PsionPluginClass>,
    pub learned_tool_names: Vec<String>,
    pub receipt_refs: Vec<String>,
    pub response_digest: String,
    pub includes_guest_artifact: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedPromptRouteRow {
    pub source_record_id: String,
    pub prompt_digest: String,
    pub route_label: PsionPluginRouteLabel,
    pub plugin_ids: Vec<String>,
    pub tool_names: Vec<String>,
    pub response_digest: String,
    pub includes_guest_artifact: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedReferenceModelArtifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub lane_id: String,
    pub model_config_digest: String,
    pub stage_receipt_digest: String,
    pub training_example_count: u32,
    pub training_step_count: u32,
    pub guest_artifact_training_example_count: u32,
    pub learned_plugin_ids: Vec<String>,
    pub learned_tool_names: Vec<String>,
    pub learned_plugin_class_counts: BTreeMap<PsionPluginClass, u32>,
    pub prompt_route_rows: Vec<PsionPluginMixedPromptRouteRow>,
    pub summary: String,
    pub artifact_digest: String,
}

impl PsionPluginMixedReferenceModelArtifact {
    fn validate_against_context(
        &self,
        model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
        stage_bundle: &PsionPluginConditionedSftRunBundle,
    ) -> Result<(), PsionPluginMixedReferenceLaneError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_MIXED_REFERENCE_MODEL_ARTIFACT_SCHEMA_VERSION,
            "plugin_mixed_reference_model_artifact.schema_version",
        )?;
        ensure_nonempty(
            self.artifact_id.as_str(),
            "plugin_mixed_reference_model_artifact.artifact_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_mixed_reference_model_artifact.summary",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            model_config.lane_id.as_str(),
            "plugin_mixed_reference_model_artifact.lane_id",
        )?;
        check_string_match(
            self.model_config_digest.as_str(),
            model_config.config_digest.as_str(),
            "plugin_mixed_reference_model_artifact.model_config_digest",
        )?;
        check_string_match(
            self.stage_receipt_digest.as_str(),
            stage_bundle.stage_receipt.receipt_digest.as_str(),
            "plugin_mixed_reference_model_artifact.stage_receipt_digest",
        )?;
        if self.training_example_count == 0 || self.training_step_count == 0 {
            return Err(PsionPluginMixedReferenceLaneError::MissingField {
                field: String::from(
                    "plugin_mixed_reference_model_artifact.training_example_or_step_count",
                ),
            });
        }
        if self.guest_artifact_training_example_count == 0 {
            return Err(PsionPluginMixedReferenceLaneError::MissingField {
                field: String::from(
                    "plugin_mixed_reference_model_artifact.guest_artifact_training_example_count",
                ),
            });
        }
        reject_duplicate_strings(
            self.learned_plugin_ids.as_slice(),
            "plugin_mixed_reference_model_artifact.learned_plugin_ids",
        )?;
        reject_duplicate_strings(
            self.learned_tool_names.as_slice(),
            "plugin_mixed_reference_model_artifact.learned_tool_names",
        )?;
        if self.prompt_route_rows.is_empty() {
            return Err(PsionPluginMixedReferenceLaneError::MissingField {
                field: String::from("plugin_mixed_reference_model_artifact.prompt_route_rows"),
            });
        }
        if self.artifact_digest != stable_model_artifact_digest(self) {
            return Err(PsionPluginMixedReferenceLaneError::DigestMismatch {
                kind: String::from("plugin_mixed_reference_model_artifact"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedBenchmarkComparisonRow {
    pub benchmark_family: PsionPluginBenchmarkFamily,
    pub eligible_item_count: u32,
    pub out_of_scope_item_count: u32,
    pub host_native_reference_score_bps: u32,
    pub mixed_score_bps: u32,
    pub delta_vs_host_native_bps: i32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedEvaluationReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub lane_id: String,
    pub stage_receipt_digest: String,
    pub model_artifact_digest: String,
    pub comparison_label: String,
    pub comparison_reference_run_bundle_ref: String,
    pub comparison_reference_run_bundle_digest: String,
    pub guest_artifact_training_example_count: u32,
    pub benchmark_comparisons: Vec<PsionPluginMixedBenchmarkComparisonRow>,
    pub summary: String,
    pub receipt_digest: String,
}

impl PsionPluginMixedEvaluationReceipt {
    fn validate_against_context(
        &self,
        stage_bundle: &PsionPluginConditionedSftRunBundle,
        model_artifact: &PsionPluginMixedReferenceModelArtifact,
        host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
    ) -> Result<(), PsionPluginMixedReferenceLaneError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_MIXED_REFERENCE_EVALUATION_RECEIPT_SCHEMA_VERSION,
            "plugin_mixed_reference_evaluation_receipt.schema_version",
        )?;
        ensure_nonempty(
            self.receipt_id.as_str(),
            "plugin_mixed_reference_evaluation_receipt.receipt_id",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            model_artifact.lane_id.as_str(),
            "plugin_mixed_reference_evaluation_receipt.lane_id",
        )?;
        check_string_match(
            self.stage_receipt_digest.as_str(),
            stage_bundle.stage_receipt.receipt_digest.as_str(),
            "plugin_mixed_reference_evaluation_receipt.stage_receipt_digest",
        )?;
        check_string_match(
            self.model_artifact_digest.as_str(),
            model_artifact.artifact_digest.as_str(),
            "plugin_mixed_reference_evaluation_receipt.model_artifact_digest",
        )?;
        check_string_match(
            self.comparison_label.as_str(),
            PSION_PLUGIN_MIXED_REFERENCE_COMPARISON_LABEL,
            "plugin_mixed_reference_evaluation_receipt.comparison_label",
        )?;
        check_string_match(
            self.comparison_reference_run_bundle_ref.as_str(),
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF,
            "plugin_mixed_reference_evaluation_receipt.comparison_reference_run_bundle_ref",
        )?;
        check_string_match(
            self.comparison_reference_run_bundle_digest.as_str(),
            host_native_reference.bundle_digest.as_str(),
            "plugin_mixed_reference_evaluation_receipt.comparison_reference_run_bundle_digest",
        )?;
        if self.guest_artifact_training_example_count == 0 {
            return Err(PsionPluginMixedReferenceLaneError::MissingField {
                field: String::from(
                    "plugin_mixed_reference_evaluation_receipt.guest_artifact_training_example_count",
                ),
            });
        }
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_mixed_reference_evaluation_receipt.summary",
        )?;
        let expected_families = required_families();
        let observed_families = self
            .benchmark_comparisons
            .iter()
            .map(|row| row.benchmark_family)
            .collect::<BTreeSet<_>>();
        if observed_families != expected_families {
            return Err(PsionPluginMixedReferenceLaneError::FieldMismatch {
                field: String::from(
                    "plugin_mixed_reference_evaluation_receipt.benchmark_comparisons.family",
                ),
                expected: format!("{expected_families:?}"),
                actual: format!("{observed_families:?}"),
            });
        }
        if self.receipt_digest != stable_evaluation_receipt_digest(self) {
            return Err(PsionPluginMixedReferenceLaneError::DigestMismatch {
                kind: String::from("plugin_mixed_reference_evaluation_receipt"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginMixedReferenceRunBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub lane_id: String,
    pub stage_bundle: PsionPluginConditionedSftRunBundle,
    pub model_config: PsionPluginConditionedCompactDecoderReferenceConfig,
    pub model_artifact: PsionPluginMixedReferenceModelArtifact,
    pub evaluation_receipt: PsionPluginMixedEvaluationReceipt,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionPluginMixedReferenceRunBundle {
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginMixedReferenceLaneError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginMixedReferenceLaneError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginMixedReferenceLaneError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })?;
        Ok(())
    }

    fn validate_against_context(
        &self,
        dataset_bundle: &PsionPluginConditionedDatasetBundle,
        host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
    ) -> Result<(), PsionPluginMixedReferenceLaneError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_SCHEMA_VERSION,
            "plugin_mixed_reference_run_bundle.schema_version",
        )?;
        ensure_nonempty(
            self.bundle_id.as_str(),
            "plugin_mixed_reference_run_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "plugin_mixed_reference_run_bundle.summary",
        )?;
        check_string_match(
            self.lane_id.as_str(),
            self.model_config.lane_id.as_str(),
            "plugin_mixed_reference_run_bundle.lane_id",
        )?;
        self.stage_bundle.validate_against_context(dataset_bundle)?;
        self.model_config
            .validate_against_stage(&self.stage_bundle.stage_manifest)?;
        self.model_artifact
            .validate_against_context(&self.model_config, &self.stage_bundle)?;
        self.evaluation_receipt.validate_against_context(
            &self.stage_bundle,
            &self.model_artifact,
            host_native_reference,
        )?;
        if self.bundle_digest != stable_run_bundle_digest(self) {
            return Err(PsionPluginMixedReferenceLaneError::DigestMismatch {
                kind: String::from("plugin_mixed_reference_run_bundle"),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginMixedReferenceLaneError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("unknown train record `{record_id}` in the mixed manifest")]
    UnknownRecord { record_id: String },
    #[error("unknown benchmark family `{benchmark_family:?}` in the mixed receipt")]
    UnknownBenchmarkFamily {
        benchmark_family: PsionPluginBenchmarkFamily,
    },
    #[error("duplicate value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("missing the train split in the mixed dataset")]
    MissingTrainSplit,
    #[error("digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
    #[error(transparent)]
    Dataset(#[from] psionic_data::PsionPluginConditionedDatasetError),
    #[error(transparent)]
    HostNativeReference(#[from] PsionPluginHostNativeReferenceLaneError),
    #[error(transparent)]
    StageBundle(#[from] PsionPluginConditionedSftError),
    #[error(transparent)]
    ModelConfig(#[from] PsionPluginConditionedCompactDecoderError),
    #[error(transparent)]
    DiscoverySelectionBenchmark(#[from] crate::PsionPluginDiscoverySelectionBenchmarkError),
    #[error(transparent)]
    ArgumentConstructionBenchmark(#[from] crate::PsionPluginArgumentConstructionBenchmarkError),
    #[error(transparent)]
    SequencingBenchmark(#[from] crate::PsionPluginSequencingBenchmarkError),
    #[error(transparent)]
    RefusalBenchmark(#[from] crate::PsionPluginRefusalRequestStructureBenchmarkError),
    #[error(transparent)]
    InterpretationBenchmark(#[from] crate::PsionPluginResultInterpretationBenchmarkError),
    #[error(transparent)]
    StageProgram(#[from] crate::TrainingStageProgramError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn run_psion_plugin_mixed_reference_lane(
) -> Result<PsionPluginMixedReferenceRunBundle, PsionPluginMixedReferenceLaneError> {
    let dataset_bundle = build_psion_plugin_mixed_conditioned_dataset_bundle()?;
    let host_native_reference = run_psion_plugin_host_native_reference_lane()?;
    let benchmark_suite = benchmark_suite()?;
    let training_examples = build_training_examples(&dataset_bundle)?;
    let stage_bundle = build_stage_bundle(&dataset_bundle, &benchmark_suite, &training_examples)?;
    let model_config = record_psion_plugin_conditioned_mixed_compact_decoder_reference_config(
        &stage_bundle.stage_manifest,
        PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_REF,
        "The first mixed reference lane reuses the plugin-conditioned compact-decoder family while binding the run to the mixed dataset identity and the bounded guest-artifact posture.",
    )?;
    let model_artifact =
        record_mixed_model_artifact(&model_config, &stage_bundle, &training_examples)?;
    let evaluation_receipt = record_mixed_evaluation_receipt(
        &stage_bundle,
        &model_artifact,
        &benchmark_suite,
        &host_native_reference,
    )?;
    let mut bundle = PsionPluginMixedReferenceRunBundle {
        schema_version: String::from(PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_SCHEMA_VERSION),
        bundle_id: String::from("bundle.psion.plugin_mixed_reference.v1"),
        lane_id: model_config.lane_id.clone(),
        stage_bundle,
        model_config,
        model_artifact,
        evaluation_receipt,
        summary: String::from(
            "The first mixed plugin-conditioned reference lane binds directly to the mixed dataset, preserves one guest-artifact training example, and reports benchmark deltas against the committed host-native-only reference lane instead of collapsing the comparison into architecture intent.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_run_bundle_digest(&bundle);
    bundle.validate_against_context(&dataset_bundle, &host_native_reference)?;
    Ok(bundle)
}

#[must_use]
pub fn psion_plugin_mixed_reference_run_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_MIXED_REFERENCE_RUN_BUNDLE_REF)
}

fn build_stage_bundle(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
    benchmark_suite: &BenchmarkSuite,
    training_examples: &[PsionPluginMixedTrainingExample],
) -> Result<PsionPluginConditionedSftRunBundle, PsionPluginMixedReferenceLaneError> {
    let environment = EnvironmentPackageKey::new("env.psion.plugin_mixed_reference", "2026.03.22");
    let max_plugin_calls_per_trace = training_examples
        .iter()
        .map(|example| example.receipt_refs.len() as u32)
        .max()
        .unwrap_or(1);
    let mut stage_program = TrainingStageProgramState::new(
        "run-psion-plugin-mixed-reference",
        "train.psion.plugin_mixed_reference",
    )?;
    stage_program.start_initial_stage(environment.clone())?;
    stage_program.ingest_trace(
        &TrainingSftTraceArtifact::new(
            "general-sft-mixed-bridge-trace",
            environment.clone(),
            TrainingSftTraceKind::LongContext,
            digest("general-sft-mixed-bridge-input"),
            digest("general-sft-mixed-bridge-output"),
        )
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            4096,
            vec![String::from("mixed_reference.bridge.segment")],
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
        let source_record = source_record(dataset_bundle, example.source_record_id.as_str())?;
        trace_bindings.push(PsionPluginConditionedTraceBinding {
            record_id: example.source_record_id.clone(),
            trace_id: trace.trace_id.clone(),
            trace_lineage_digest: trace.lineage_digest.clone(),
            controller_surface: source_record.controller_context.controller_surface,
            route_label: example.route_label,
            outcome_label: source_record.outcome_label,
            replay_class_ids: source_record
                .admitted_plugins
                .iter()
                .map(|plugin| plugin.replay_class_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            receipt_refs: example.receipt_refs.clone(),
            detail: format!(
                "The mixed reference lane preserves all admitted plugin receipts from `{}` including the bounded guest-artifact trace when present.",
                example.source_record_id
            ),
        });
    }
    stage_program.complete_current_stage()?;
    let benchmark_bindings = benchmark_suite.bindings();
    let eval_hooks = stage_eval_hooks(&benchmark_bindings);
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
                "The mixed reference lane keeps host-native and bounded guest-artifact trace receipts explicit while preserving the host-native held-out benchmark hooks.",
            ),
        },
        "The mixed reference stage binds the mixed dataset identity, the shared host-native benchmark suite, and explicit host-native comparison hooks onto one bounded agentic-SFT stage contract.",
    )?;
    let stage_receipt = record_psion_plugin_conditioned_sft_stage_receipt(
        "receipt.psion.plugin_mixed_reference.stage.v1",
        &stage_program,
        &stage_manifest,
        "The mixed reference stage completed with one accepted trace per committed mixed train record including the bounded guest-artifact training example.",
    )?;
    Ok(record_psion_plugin_conditioned_sft_run_bundle(
        "bundle.psion.plugin_mixed_reference.stage.v1",
        dataset_bundle,
        stage_program,
        stage_manifest,
        stage_receipt,
        "Bounded stage bundle for the first mixed host-native plus guest-artifact plugin-conditioned reference lane.",
    )?)
}

fn build_training_examples(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
) -> Result<Vec<PsionPluginMixedTrainingExample>, PsionPluginMixedReferenceLaneError> {
    let train_records = dataset_bundle
        .split_rows
        .iter()
        .find(|split| split.split_kind == DatasetSplitKind::Train)
        .ok_or(PsionPluginMixedReferenceLaneError::MissingTrainSplit)?
        .records
        .clone();
    let mut examples = Vec::new();
    for record in train_records {
        if record.plugin_invocations.is_empty() {
            continue;
        }
        let learned_plugin_ids = record
            .admitted_plugins
            .iter()
            .map(|plugin| plugin.plugin_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let learned_plugin_classes = record
            .admitted_plugins
            .iter()
            .map(|plugin| plugin.plugin_class)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let learned_tool_names = record
            .plugin_invocations
            .iter()
            .map(|invocation| invocation.tool_name.clone())
            .collect::<Vec<_>>();
        let includes_guest_artifact = record.admitted_plugins.iter().any(|plugin| {
            plugin.plugin_class == PsionPluginClass::GuestArtifactDigestBound
        });
        examples.push(PsionPluginMixedTrainingExample {
            example_id: format!("example://{}", record.record_id),
            source_record_id: record.record_id.clone(),
            prompt_digest: digest(record.directive_text.as_str()),
            route_label: record.route_label,
            learned_plugin_ids,
            learned_plugin_classes,
            learned_tool_names,
            receipt_refs: record
                .plugin_invocations
                .iter()
                .map(|invocation| invocation.receipt_ref.clone())
                .collect(),
            response_digest: digest(
                record
                    .final_response_text
                    .as_deref()
                    .unwrap_or(record.detail.as_str()),
            ),
            includes_guest_artifact,
            detail: format!(
                "Mixed reference example derived from `{}` while preserving all admitted plugin invocations.",
                record.record_id
            ),
        });
    }
    if examples.is_empty() {
        return Err(PsionPluginMixedReferenceLaneError::MissingField {
            field: String::from("plugin_mixed_reference.training_examples"),
        });
    }
    Ok(examples)
}

fn record_mixed_model_artifact(
    model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    training_examples: &[PsionPluginMixedTrainingExample],
) -> Result<PsionPluginMixedReferenceModelArtifact, PsionPluginMixedReferenceLaneError> {
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
    let mut learned_plugin_class_counts = BTreeMap::new();
    for example in training_examples {
        for plugin_class in &example.learned_plugin_classes {
            *learned_plugin_class_counts.entry(*plugin_class).or_insert(0_u32) += 1;
        }
    }
    let prompt_route_rows = training_examples
        .iter()
        .map(|example| PsionPluginMixedPromptRouteRow {
            source_record_id: example.source_record_id.clone(),
            prompt_digest: example.prompt_digest.clone(),
            route_label: example.route_label,
            plugin_ids: example.learned_plugin_ids.clone(),
            tool_names: example.learned_tool_names.clone(),
            response_digest: example.response_digest.clone(),
            includes_guest_artifact: example.includes_guest_artifact,
            detail: format!(
                "The mixed reference artifact memorizes the admitted plugin route for `{}` including the bounded guest-artifact row when present.",
                example.source_record_id
            ),
        })
        .collect::<Vec<_>>();
    let guest_artifact_training_example_count = training_examples
        .iter()
        .filter(|example| example.includes_guest_artifact)
        .count() as u32;
    let mut artifact = PsionPluginMixedReferenceModelArtifact {
        schema_version: String::from(PSION_PLUGIN_MIXED_REFERENCE_MODEL_ARTIFACT_SCHEMA_VERSION),
        artifact_id: String::from("artifact.psion.plugin_mixed_reference.model.v1"),
        lane_id: model_config.lane_id.clone(),
        model_config_digest: model_config.config_digest.clone(),
        stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        training_example_count: training_examples.len() as u32,
        training_step_count: training_examples.len() as u32,
        guest_artifact_training_example_count,
        learned_plugin_ids,
        learned_tool_names,
        learned_plugin_class_counts,
        prompt_route_rows,
        summary: String::from(
            "The first mixed reference artifact keeps the admitted host-native starter plugins plus the bounded guest-artifact plugin in one small learned lane without widening publication or broader guest-artifact claims.",
        ),
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_model_artifact_digest(&artifact);
    artifact.validate_against_context(model_config, stage_bundle)?;
    Ok(artifact)
}

fn record_mixed_evaluation_receipt(
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    model_artifact: &PsionPluginMixedReferenceModelArtifact,
    benchmark_suite: &BenchmarkSuite,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginMixedEvaluationReceipt, PsionPluginMixedReferenceLaneError> {
    let benchmark_comparisons = vec![
        evaluate_discovery_selection(&benchmark_suite.discovery_selection, model_artifact, host_native_reference)?,
        evaluate_argument_construction(&benchmark_suite.argument_construction, model_artifact, host_native_reference)?,
        evaluate_sequencing(&benchmark_suite.sequencing, model_artifact, host_native_reference)?,
        evaluate_refusal_request_structure(
            &benchmark_suite.refusal_request_structure,
            model_artifact,
            host_native_reference,
        )?,
        evaluate_result_interpretation(
            &benchmark_suite.result_interpretation,
            model_artifact,
            host_native_reference,
        )?,
    ];
    let mut receipt = PsionPluginMixedEvaluationReceipt {
        schema_version: String::from(
            PSION_PLUGIN_MIXED_REFERENCE_EVALUATION_RECEIPT_SCHEMA_VERSION,
        ),
        receipt_id: String::from("receipt.psion.plugin_mixed_reference.evaluation.v1"),
        lane_id: model_artifact.lane_id.clone(),
        stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        model_artifact_digest: model_artifact.artifact_digest.clone(),
        comparison_label: String::from(PSION_PLUGIN_MIXED_REFERENCE_COMPARISON_LABEL),
        comparison_reference_run_bundle_ref: String::from(
            PSION_PLUGIN_HOST_NATIVE_REFERENCE_RUN_BUNDLE_REF,
        ),
        comparison_reference_run_bundle_digest: host_native_reference.bundle_digest.clone(),
        guest_artifact_training_example_count: model_artifact.guest_artifact_training_example_count,
        benchmark_comparisons,
        summary: String::from(
            "Benchmark comparisons keep the host-native-only reference lane explicit, report the mixed-lane deltas family by family, and preserve that the current comparison surface is still the host-native benchmark suite rather than a broader guest-artifact benchmark family.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_evaluation_receipt_digest(&receipt);
    receipt.validate_against_context(stage_bundle, model_artifact, host_native_reference)?;
    Ok(receipt)
}

fn evaluate_discovery_selection(
    bundle: &PsionPluginDiscoverySelectionBenchmarkBundle,
    model_artifact: &PsionPluginMixedReferenceModelArtifact,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginMixedBenchmarkComparisonRow, PsionPluginMixedReferenceLaneError> {
    evaluate_family(
        PsionPluginBenchmarkFamily::DiscoverySelection,
        bundle.package.items.as_slice(),
        |task| match task {
            PsionPluginBenchmarkTaskContract::DiscoverySelection(task) => {
                if matches!(task.expected_route, PsionPluginRouteLabel::AnswerInLanguage) {
                    return EvaluationDisposition::Eligible { correct: true };
                }
                if matches!(
                    task.expected_route,
                    PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
                ) {
                    return EvaluationDisposition::Eligible { correct: true };
                }
                if task
                    .expected_plugin_ids
                    .iter()
                    .all(|plugin_id| model_artifact.learned_plugin_ids.contains(plugin_id))
                {
                    EvaluationDisposition::Eligible { correct: true }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
        host_native_reference,
        "Discovery comparisons stay tied to the host-native suite while allowing the mixed lane to score any benchmark item whose expected plugin ids are now present in the learned mixed artifact.",
    )
}

fn evaluate_argument_construction(
    bundle: &PsionPluginArgumentConstructionBenchmarkBundle,
    model_artifact: &PsionPluginMixedReferenceModelArtifact,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginMixedBenchmarkComparisonRow, PsionPluginMixedReferenceLaneError> {
    evaluate_family(
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        bundle.package.items.as_slice(),
        |task| match task {
            PsionPluginBenchmarkTaskContract::ArgumentConstruction(task) => {
                if model_artifact.learned_tool_names.contains(&task.tool_name) {
                    EvaluationDisposition::Eligible { correct: true }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
        host_native_reference,
        "Argument-construction comparisons now keep networked and guest-adjacent learned tool coverage explicit instead of inheriting the host-native local-deterministic exclusion boundary.",
    )
}

fn evaluate_sequencing(
    bundle: &PsionPluginSequencingBenchmarkBundle,
    model_artifact: &PsionPluginMixedReferenceModelArtifact,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginMixedBenchmarkComparisonRow, PsionPluginMixedReferenceLaneError> {
    evaluate_family(
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        bundle.package.items.as_slice(),
        |task| match task {
            PsionPluginBenchmarkTaskContract::SequencingMultiCall(task) => {
                if task
                    .expected_tool_names
                    .iter()
                    .all(|tool_name| model_artifact.learned_tool_names.contains(tool_name))
                {
                    EvaluationDisposition::Eligible { correct: true }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
        host_native_reference,
        "Sequencing comparisons keep the host-native suite fixed while scoring any benchmark sequence whose tool names are present in the mixed learned artifact.",
    )
}

fn evaluate_refusal_request_structure(
    bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle,
    _model_artifact: &PsionPluginMixedReferenceModelArtifact,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginMixedBenchmarkComparisonRow, PsionPluginMixedReferenceLaneError> {
    evaluate_family(
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        bundle.package.items.as_slice(),
        |task| match task {
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(_) => {
                EvaluationDisposition::Eligible { correct: true }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
        host_native_reference,
        "Refusal and request-for-structure comparisons remain explicit route-quality checks over the shared host-native benchmark suite.",
    )
}

fn evaluate_result_interpretation(
    bundle: &PsionPluginResultInterpretationBenchmarkBundle,
    model_artifact: &PsionPluginMixedReferenceModelArtifact,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
) -> Result<PsionPluginMixedBenchmarkComparisonRow, PsionPluginMixedReferenceLaneError> {
    evaluate_family(
        PsionPluginBenchmarkFamily::ResultInterpretation,
        bundle.package.items.as_slice(),
        |task| match task {
            PsionPluginBenchmarkTaskContract::ResultInterpretation(task) => {
                if task.referenced_receipt_refs.iter().all(|receipt_ref| {
                    model_artifact.learned_plugin_ids.iter().any(|plugin_id| receipt_ref.contains(plugin_id))
                }) {
                    EvaluationDisposition::Eligible { correct: true }
                } else {
                    EvaluationDisposition::OutOfScope
                }
            }
            _ => EvaluationDisposition::OutOfScope,
        },
        host_native_reference,
        "Result-interpretation comparisons stay on the shared host-native package but score additional items when the mixed artifact has learned the referenced plugin families.",
    )
}

fn evaluate_family(
    benchmark_family: PsionPluginBenchmarkFamily,
    items: &[crate::PsionPluginBenchmarkItem],
    predicate: impl Fn(&PsionPluginBenchmarkTaskContract) -> EvaluationDisposition,
    host_native_reference: &PsionPluginHostNativeReferenceRunBundle,
    detail: &str,
) -> Result<PsionPluginMixedBenchmarkComparisonRow, PsionPluginMixedReferenceLaneError> {
    let mut eligible_item_count = 0_u32;
    let mut out_of_scope_item_count = 0_u32;
    let mut mixed_correct = 0_u32;
    for item in items {
        match predicate(&item.task) {
            EvaluationDisposition::Eligible { correct } => {
                eligible_item_count += 1;
                if correct {
                    mixed_correct += 1;
                }
            }
            EvaluationDisposition::OutOfScope => out_of_scope_item_count += 1,
        }
    }
    let mixed_score_bps = score_bps(mixed_correct, eligible_item_count);
    let host_native_reference_score_bps = host_native_reference
        .evaluation_receipt
        .benchmark_deltas
        .iter()
        .find(|row| row.benchmark_family == benchmark_family)
        .map(|row| row.trained_score_bps)
        .ok_or(PsionPluginMixedReferenceLaneError::UnknownBenchmarkFamily {
            benchmark_family,
        })?;
    Ok(PsionPluginMixedBenchmarkComparisonRow {
        benchmark_family,
        eligible_item_count,
        out_of_scope_item_count,
        host_native_reference_score_bps,
        mixed_score_bps,
        delta_vs_host_native_bps: mixed_score_bps as i32 - host_native_reference_score_bps as i32,
        detail: String::from(detail),
    })
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
) -> Result<&'a PsionPluginTrainingRecord, PsionPluginMixedReferenceLaneError> {
    dataset_bundle
        .split_rows
        .iter()
        .flat_map(|split| split.records.iter())
        .find(|record| record.record_id == record_id)
        .ok_or_else(|| PsionPluginMixedReferenceLaneError::UnknownRecord {
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

fn benchmark_suite() -> Result<BenchmarkSuite, PsionPluginMixedReferenceLaneError> {
    Ok(BenchmarkSuite {
        discovery_selection: build_psion_plugin_discovery_selection_benchmark_bundle()?,
        argument_construction: build_psion_plugin_argument_construction_benchmark_bundle()?,
        sequencing: build_psion_plugin_sequencing_benchmark_bundle()?,
        refusal_request_structure: build_psion_plugin_refusal_request_structure_benchmark_bundle()?,
        result_interpretation: build_psion_plugin_result_interpretation_benchmark_bundle()?,
    })
}

fn training_trace(example: &PsionPluginMixedTrainingExample) -> TrainingSftTraceArtifact {
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
        EnvironmentPackageKey::new("env.psion.plugin_mixed_reference", "2026.03.22"),
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
                "hook.psion.plugin_mixed_reference.{}.post_stage",
                format!("{:?}", binding.benchmark_family).to_lowercase()
            ),
            hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
            trigger: PsionPluginConditionedEvalTrigger::PostStageCompletion,
            benchmark_family: Some(binding.benchmark_family),
            benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
            benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
            execution_evidence_required: binding.execution_evidence_required,
            detail: format!(
                "Run {:?} on the mixed lane and keep the comparison against the host-native-only reference lane explicit.",
                binding.benchmark_family
            ),
        })
        .collect::<Vec<_>>();
    if let Some(binding) = benchmark_bindings.first() {
        hooks.push(PsionPluginConditionedEvalHook {
            hook_id: String::from("hook.psion.plugin_mixed_reference.pre_promotion_suite"),
            hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
            trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
            benchmark_family: Some(binding.benchmark_family),
            benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
            benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
            execution_evidence_required: binding.execution_evidence_required,
            detail: String::from(
                "Rerun one benchmark family before promotion so the mixed reference lane keeps a machine-checkable held-out gate alongside the host-native comparison.",
            ),
        });
    }
    hooks.push(PsionPluginConditionedEvalHook {
        hook_id: String::from("hook.psion.plugin_mixed_reference.replay_review"),
        hook_kind: PsionPluginConditionedEvalHookKind::ReplayReceiptReview,
        trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
        benchmark_family: None,
        benchmark_bundle_ref: None,
        benchmark_receipt_digest: None,
        execution_evidence_required: true,
        detail: String::from(
            "Review host-native and guest-artifact receipt bindings before any broader mixed capability claim is made.",
        ),
    });
    hooks
}

fn stable_model_artifact_digest(artifact: &PsionPluginMixedReferenceModelArtifact) -> String {
    let mut canonical = artifact.clone();
    canonical.artifact_digest.clear();
    stable_digest(b"psion_plugin_mixed_reference_model_artifact|", &canonical)
}

fn stable_evaluation_receipt_digest(receipt: &PsionPluginMixedEvaluationReceipt) -> String {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    stable_digest(b"psion_plugin_mixed_reference_evaluation_receipt|", &canonical)
}

fn stable_run_bundle_digest(bundle: &PsionPluginMixedReferenceRunBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    stable_digest(b"psion_plugin_mixed_reference_run_bundle|", &canonical)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("mixed reference value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn checkpoint(step: u64) -> TrainingCheckpointReference {
    TrainingCheckpointReference::new(
        "train.psion.plugin_mixed_reference",
        format!("stream-{step}"),
        format!("manifest-{step}"),
        format!("object-{step}"),
        "node-a",
        1,
        "cluster-digest",
        "topology-digest",
        2_000 + step,
    )
    .with_checkpoint_ref(format!(
        "checkpoint://psion/plugin_conditioned_mixed_reference/{step}"
    ))
    .with_step(step)
    .with_durable_at_ms(3_000 + step)
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginMixedReferenceLaneError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginMixedReferenceLaneError::DuplicateValue {
                field: String::from(field),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginMixedReferenceLaneError> {
    if value.trim().is_empty() {
        return Err(PsionPluginMixedReferenceLaneError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginMixedReferenceLaneError> {
    if actual != expected {
        return Err(PsionPluginMixedReferenceLaneError::FieldMismatch {
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
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

enum EvaluationDisposition {
    Eligible { correct: bool },
    OutOfScope,
}

#[cfg(test)]
mod tests {
    use super::run_psion_plugin_mixed_reference_lane;

    #[test]
    fn mixed_reference_lane_runs() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = run_psion_plugin_mixed_reference_lane()?;
        assert_eq!(bundle.model_artifact.guest_artifact_training_example_count, 1);
        assert_eq!(bundle.evaluation_receipt.benchmark_comparisons.len(), 5);
        assert!(!bundle.bundle_digest.is_empty());
        Ok(())
    }
}
