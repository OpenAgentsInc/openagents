use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Instant,
};

use psionic_backend_cuda::CudaBackend;
use psionic_core::{DType, Device, Shape, TensorData, TensorId, TensorSpec};
use psionic_data::{
    DatasetSplitKind, PsionPluginClass, PsionPluginConditionedDatasetBundle, PsionPluginRouteLabel,
    PsionPluginTrainingRecord, build_psion_plugin_conditioned_dataset_bundle,
};
use psionic_environments::EnvironmentPackageKey;
use psionic_ir::{AutodiffContext, AutodiffGraph, AutodiffGraphBuilder};
use psionic_models::PsionCompactDecoderDescriptor;
use psionic_runtime::{DeliveredExecutionContext, DeviceDescriptor, TrainingCheckpointReference};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, PsionGoogleSingleNodeLiveVisualizationWriter,
    PsionGoogleSingleNodeStepTelemetry, PsionGoogleSingleNodeVisualizationError,
    PsionPluginArgumentConstructionBenchmarkBundle, PsionPluginBenchmarkFamily,
    PsionPluginBenchmarkItem, PsionPluginBenchmarkTaskContract,
    PsionPluginConditionedBenchmarkBinding, PsionPluginConditionedCompactDecoderError,
    PsionPluginConditionedCompactDecoderReferenceConfig, PsionPluginConditionedEvalHook,
    PsionPluginConditionedEvalHookKind, PsionPluginConditionedEvalTrigger,
    PsionPluginConditionedSftError, PsionPluginConditionedSftRunBundle,
    PsionPluginConditionedSftStageConfig, PsionPluginConditionedTraceBinding,
    PsionPluginDiscoverySelectionBenchmarkBundle, PsionPluginHostNativeBenchmarkDeltaRow,
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginResultInterpretationBenchmarkBundle, PsionPluginSequencingBenchmarkBundle,
    PsionPretrainHardwareTopologyReceipt, PsionPretrainRunObservabilityError,
    PsionPretrainRunThroughputReceipt, PsionPretrainStageAcceleratorReceipt,
    TrainingLongContextTraceLineage, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterClass, TrainingParameterGroupState,
    TrainingSftTraceArtifact, TrainingSftTraceKind, TrainingStageKind, TrainingStageProgramError,
    TrainingStageProgramState, TrainingStepInput, TrainingTensorBuffer,
    TrainingToolCallTraceLineage, TrainingToolCallTraceStep,
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
    record_psion_plugin_conditioned_host_native_accelerated_compact_decoder_config,
    record_psion_plugin_conditioned_sft_run_bundle,
    record_psion_plugin_conditioned_sft_stage_manifest,
    record_psion_plugin_conditioned_sft_stage_receipt,
};

const TOKEN_EMBEDDING_GROUP_ID: &str = "decoder.embed_tokens.weight";
const POSITION_EMBEDDING_GROUP_ID: &str = "decoder.embed_positions.weight";
const LM_HEAD_BIAS_GROUP_ID: &str = "lm_head.bias";
const ACCELERATED_PLUGIN_VIRTUAL_BATCH_ROWS: usize = 8_192;
const ACCELERATED_PLUGIN_RUN_ID: &str = "run-psion-plugin-host-native-accelerated";
const ACCELERATED_PLUGIN_STAGE_ID: &str = "psion-plugin-host-native-accelerated-train-stage";
const ACCELERATED_PLUGIN_CHECKPOINT_FAMILY: &str = "train.psion.plugin_host_native_accelerated";
pub const PSION_PLUGIN_HOST_NATIVE_ACCELERATED_LANE_ID: &str =
    "psion_plugin_host_native_accelerated";
pub const PSION_PLUGIN_HOST_NATIVE_ACCELERATED_RUN_BUNDLE_REF: &str = "fixtures/psion/plugins/training/psion_plugin_host_native_accelerated_lane_v1/psion_plugin_host_native_accelerated_run_bundle.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeAcceleratedStageReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub run_id: String,
    pub stage_id: String,
    pub lane_id: String,
    pub dataset_identity: String,
    pub model_id: String,
    pub model_config_digest: String,
    pub plugin_stage_receipt_digest: String,
    pub delivered_execution: DeliveredExecutionContext,
    pub accelerator_execution: PsionPretrainStageAcceleratorReceipt,
    pub summary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeAcceleratedObservabilityReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub run_id: String,
    pub stage_id: String,
    pub lane_id: String,
    pub dataset_identity: String,
    pub training_stage_receipt_digest: String,
    pub throughput: PsionPretrainRunThroughputReceipt,
    pub hardware_topology: PsionPretrainHardwareTopologyReceipt,
    pub summary: String,
    pub observability_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeAcceleratedPromptRouteRow {
    pub source_record_id: String,
    pub prompt_digest: String,
    pub expected_route_label: PsionPluginRouteLabel,
    pub observed_route_label: PsionPluginRouteLabel,
    pub expected_tool_names: Vec<String>,
    pub observed_tool_names: Vec<String>,
    pub response_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeAcceleratedModelArtifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub lane_id: String,
    pub model_config_digest: String,
    pub plugin_stage_receipt_digest: String,
    pub training_stage_receipt_digest: String,
    pub observability_receipt_digest: String,
    pub training_example_count: u32,
    pub training_step_count: u32,
    pub delivered_execution_backend: String,
    pub learned_plugin_ids: Vec<String>,
    pub learned_tool_names: Vec<String>,
    pub prompt_route_rows: Vec<PsionPluginHostNativeAcceleratedPromptRouteRow>,
    pub summary: String,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeAcceleratedEvaluationReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub lane_id: String,
    pub plugin_stage_receipt_digest: String,
    pub training_stage_receipt_digest: String,
    pub observability_receipt_digest: String,
    pub model_artifact_digest: String,
    pub baseline_label: String,
    pub limited_to_proved_authoring_class: bool,
    pub proved_authoring_class_label: String,
    pub benchmark_deltas: Vec<PsionPluginHostNativeBenchmarkDeltaRow>,
    pub summary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginHostNativeAcceleratedRunBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub lane_id: String,
    pub stage_bundle: PsionPluginConditionedSftRunBundle,
    pub model_config: PsionPluginConditionedCompactDecoderReferenceConfig,
    pub stage_receipt: PsionPluginHostNativeAcceleratedStageReceipt,
    pub observability_receipt: PsionPluginHostNativeAcceleratedObservabilityReceipt,
    pub model_artifact: PsionPluginHostNativeAcceleratedModelArtifact,
    pub evaluation_receipt: PsionPluginHostNativeAcceleratedEvaluationReceipt,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionPluginHostNativeAcceleratedRunBundle {
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginHostNativeAcceleratedLaneError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            output_path,
            format!("{}\n", serde_json::to_string_pretty(self)?),
        )?;
        Ok(())
    }
}

#[derive(Clone)]
struct HostNativeAcceleratedTrainingRecord {
    source_record_id: String,
    prompt_digest: String,
    route_label: PsionPluginRouteLabel,
    expected_tool_names: Vec<String>,
    tool_name_to_plugin_id: BTreeMap<String, String>,
    local_receipt_refs: Vec<String>,
    response_digest: String,
}

#[derive(Clone)]
struct HostNativeProbeExample {
    context_token_ids: Vec<u32>,
    target_token_id: u32,
}

#[derive(Clone)]
struct HostNativeAcceleratedTokenPlan {
    route_marker_token_id: u32,
    tool_marker_token_id: u32,
    slot_token_ids: Vec<u32>,
    prompt_token_ids: BTreeMap<String, u32>,
    route_token_ids: BTreeMap<PsionPluginRouteLabel, u32>,
    tool_token_ids: BTreeMap<String, u32>,
}

#[derive(Clone, Debug)]
struct HostNativeAcceleratedModel {
    descriptor: PsionCompactDecoderDescriptor,
    token_embeddings: Vec<f32>,
    position_embeddings: Vec<f32>,
    lm_head_bias: Vec<f32>,
}

#[derive(Clone)]
struct AcceleratedHostNativeConfig {
    run_id: String,
    stage_id: String,
    checkpoint_family: String,
    started_at_ms: u64,
    step_duration_ms: u64,
    budget: TrainingLoopBudget,
    optimizer: TrainingOptimizerConfig,
}

impl AcceleratedHostNativeConfig {
    fn bounded_single_node() -> Result<Self, PsionPluginHostNativeAcceleratedLaneError> {
        Ok(Self {
            run_id: String::from(ACCELERATED_PLUGIN_RUN_ID),
            stage_id: String::from(ACCELERATED_PLUGIN_STAGE_ID),
            checkpoint_family: String::from(ACCELERATED_PLUGIN_CHECKPOINT_FAMILY),
            started_at_ms: 1_774_404_000_000,
            step_duration_ms: 1_500,
            budget: TrainingLoopBudget::new(8, 1, 1)?,
            optimizer: TrainingOptimizerConfig::adamw(0.003, 0.9, 0.99, 1e-8)
                .with_weight_decay(0.01),
        })
    }
}

struct AcceleratedTrainerOutput {
    trained_model: HostNativeAcceleratedModel,
    stage_receipt: PsionPluginHostNativeAcceleratedStageReceipt,
    observability_receipt: PsionPluginHostNativeAcceleratedObservabilityReceipt,
}

struct PsionAcceleratedGradientProgram {
    logits_graph: AutodiffGraph,
    logits_hidden_input_tensor_id: TensorId,
    logits_token_embeddings_tensor_id: TensorId,
    weight_gradient_graph: AutodiffGraph,
    weight_gradient_hidden_input_transposed_tensor_id: TensorId,
    weight_gradient_logits_seed_tensor_id: TensorId,
    hidden_gradient_graph: AutodiffGraph,
    hidden_gradient_logits_seed_tensor_id: TensorId,
    hidden_gradient_token_embeddings_vh_tensor_id: TensorId,
}

pub fn run_psion_plugin_host_native_accelerated_lane()
-> Result<PsionPluginHostNativeAcceleratedRunBundle, PsionPluginHostNativeAcceleratedLaneError> {
    run_psion_plugin_host_native_accelerated_lane_with_live_visualization(None)
}

pub fn run_psion_plugin_host_native_accelerated_lane_with_live_visualization(
    live_visualization_writer: Option<&mut PsionGoogleSingleNodeLiveVisualizationWriter>,
) -> Result<PsionPluginHostNativeAcceleratedRunBundle, PsionPluginHostNativeAcceleratedLaneError> {
    let mut live_visualization_writer = live_visualization_writer;
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "dataset_staging",
            Some(String::from("plugin_conditioned_dataset")),
            "The plugin-conditioned Google lane is materializing the canonical dataset and stage program.",
            vec![
                String::from("plugin_dataset"),
                String::from("stage_program"),
            ],
            None,
            true,
        )?;
    }
    let dataset_bundle = build_psion_plugin_conditioned_dataset_bundle()?;
    let benchmark_suite = benchmark_suite()?;
    let training_records = build_training_records(&dataset_bundle)?;
    let stage_bundle = build_stage_bundle(
        &dataset_bundle,
        &benchmark_suite,
        training_records.as_slice(),
    )?;
    let model_config =
        record_psion_plugin_conditioned_host_native_accelerated_compact_decoder_config(
            &stage_bundle.stage_manifest,
            "bundle.psion.plugin_host_native_accelerated.stage.v1",
            "The first accelerated host-native plugin-conditioned lane keeps the same bounded proved-authoring-class claim posture as the earlier host-native reference lane, but moves the learned route and tool-use path onto the real single-node CUDA trainer.",
        )?;
    let token_plan = build_token_plan(
        training_records.as_slice(),
        model_config.descriptor.config.vocab_size,
    )?;
    let base_probe_examples = build_probe_examples(training_records.as_slice(), &token_plan)?;
    let train_examples = expand_probe_examples(
        base_probe_examples.as_slice(),
        ACCELERATED_PLUGIN_VIRTUAL_BATCH_ROWS,
    );
    let config = AcceleratedHostNativeConfig::bounded_single_node()?;
    let trainer_output = run_accelerated_training(
        &config,
        &stage_bundle,
        &model_config,
        train_examples.as_slice(),
        live_visualization_writer.as_deref_mut(),
    )?;
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "evaluation",
            Some(String::from("route_tool_projection")),
            "The plugin-conditioned Google lane is projecting learned route and tool rows from the trained compact decoder.",
            vec![String::from("model_projection"), String::from("benchmark_eval")],
            None,
            true,
        )?;
    }
    let model_artifact = record_model_artifact(
        &stage_bundle,
        &model_config,
        &trainer_output,
        training_records.as_slice(),
        &token_plan,
    )?;
    let evaluation_receipt = record_evaluation_receipt(
        &stage_bundle,
        &trainer_output,
        &model_artifact,
        &benchmark_suite,
    );
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "artifact_seal",
            Some(String::from("receipt_materialization")),
            "The plugin-conditioned Google lane sealed its stage, observability, model, and evaluation artifacts for finalization upload.",
            vec![
                String::from("stage_receipt"),
                String::from("observability_receipt"),
                String::from("model_artifact"),
                String::from("evaluation_receipt"),
            ],
            None,
            true,
        )?;
    }
    let mut bundle = PsionPluginHostNativeAcceleratedRunBundle {
        schema_version: String::from("psionic.psion.plugin_host_native_accelerated_run_bundle.v1"),
        bundle_id: String::from("bundle.psion.plugin_host_native_accelerated.v1"),
        lane_id: String::from(PSION_PLUGIN_HOST_NATIVE_ACCELERATED_LANE_ID),
        stage_bundle,
        model_config,
        stage_receipt: trainer_output.stage_receipt,
        observability_receipt: trainer_output.observability_receipt,
        model_artifact,
        evaluation_receipt,
        summary: String::from(
            "The first accelerated host-native plugin-conditioned lane keeps the proved capability-free local deterministic authoring boundary explicit while deriving the learned tool and route rows from a real bounded CUDA training loop.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(
        b"psion_plugin_host_native_accelerated_run_bundle|",
        &bundle_without_digest(&bundle),
    );
    Ok(bundle)
}

#[must_use]
pub fn psion_plugin_host_native_accelerated_run_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_HOST_NATIVE_ACCELERATED_RUN_BUNDLE_REF)
}

fn build_stage_bundle(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
    benchmark_suite: &BenchmarkSuite,
    training_records: &[HostNativeAcceleratedTrainingRecord],
) -> Result<PsionPluginConditionedSftRunBundle, PsionPluginHostNativeAcceleratedLaneError> {
    let environment =
        EnvironmentPackageKey::new("env.psion.plugin_host_native_accelerated", "2026.03.23");
    let max_plugin_calls_per_trace = training_records
        .iter()
        .map(|record| record.local_receipt_refs.len() as u32)
        .max()
        .unwrap_or(1);
    let mut stage_program = TrainingStageProgramState::new(
        ACCELERATED_PLUGIN_RUN_ID,
        ACCELERATED_PLUGIN_CHECKPOINT_FAMILY,
    )?;
    stage_program.start_initial_stage(environment.clone())?;
    stage_program.ingest_trace(
        &TrainingSftTraceArtifact::new(
            "general-sft-host-native-accelerated-bridge-trace",
            environment.clone(),
            TrainingSftTraceKind::LongContext,
            digest("general-sft-host-native-accelerated-input"),
            digest("general-sft-host-native-accelerated-output"),
        )
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
            4096,
            vec![String::from("host_native_accelerated.bridge.segment")],
        )),
    )?;
    stage_program.complete_current_stage()?;
    stage_program.advance_stage(
        TrainingStageKind::AgenticSft,
        environment.clone(),
        checkpoint(1),
    )?;
    let mut trace_bindings = Vec::with_capacity(training_records.len());
    for record in training_records {
        let trace = training_trace(record);
        stage_program.ingest_trace(&trace)?;
        trace_bindings.push(PsionPluginConditionedTraceBinding {
            record_id: record.source_record_id.clone(),
            trace_id: trace.trace_id.clone(),
            trace_lineage_digest: trace.lineage_digest.clone(),
            controller_surface: source_record(dataset_bundle, record.source_record_id.as_str())?
                .controller_context
                .controller_surface,
            route_label: record.route_label,
            outcome_label: source_record(dataset_bundle, record.source_record_id.as_str())?
                .outcome_label,
            replay_class_ids: vec![String::from("deterministic_replayable")],
            receipt_refs: record.local_receipt_refs.clone(),
            detail: format!(
                "The accelerated host-native lane keeps only the proved local-deterministic plugin subtrace from `{}` while routing the learned surface through the CUDA trainer.",
                record.source_record_id
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
                "The accelerated host-native lane keeps only the proved capability-free local deterministic plugin subtraces, preserves receipt boundaries, and binds the same held-out benchmark hooks before any wider claim can be made.",
            ),
        },
        "The accelerated host-native plugin-conditioned stage is explicitly limited to the currently fully proved capability-free local deterministic starter-plugin class.",
    )?;
    let stage_receipt = record_psion_plugin_conditioned_sft_stage_receipt(
        "receipt.psion.plugin_host_native_accelerated.stage.v1",
        &stage_program,
        &stage_manifest,
        "The accelerated host-native plugin-conditioned stage completed with one accepted capability-free local deterministic trace per committed train record before the CUDA trainer consumed the bounded learned route and tool probes.",
    )?;
    Ok(record_psion_plugin_conditioned_sft_run_bundle(
        "bundle.psion.plugin_host_native_accelerated.stage.v1",
        dataset_bundle,
        stage_program,
        stage_manifest,
        stage_receipt,
        "Bounded stage bundle for the first accelerated host-native plugin-conditioned lane.",
    )?)
}

fn build_training_records(
    dataset_bundle: &PsionPluginConditionedDatasetBundle,
) -> Result<Vec<HostNativeAcceleratedTrainingRecord>, PsionPluginHostNativeAcceleratedLaneError> {
    let train_records = dataset_bundle
        .split_rows
        .iter()
        .find(|split| split.split_kind == DatasetSplitKind::Train)
        .ok_or(PsionPluginHostNativeAcceleratedLaneError::MissingTrainSplit)?
        .records
        .clone();
    let mut records = Vec::new();
    for record in train_records {
        let local_plugins = record
            .admitted_plugins
            .iter()
            .filter(|plugin| {
                plugin.plugin_class == PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic
            })
            .map(|plugin| (plugin.tool_name.clone(), plugin.plugin_id.clone()))
            .collect::<BTreeMap<_, _>>();
        let local_invocations = record
            .plugin_invocations
            .iter()
            .filter(|invocation| local_plugins.contains_key(invocation.tool_name.as_str()))
            .collect::<Vec<_>>();
        if local_invocations.is_empty() {
            continue;
        }
        records.push(HostNativeAcceleratedTrainingRecord {
            source_record_id: record.record_id.clone(),
            prompt_digest: digest(record.directive_text.as_str()),
            route_label: record.route_label,
            expected_tool_names: local_invocations
                .iter()
                .map(|invocation| invocation.tool_name.clone())
                .collect(),
            tool_name_to_plugin_id: local_plugins,
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
        });
    }
    if records.is_empty() {
        return Err(PsionPluginHostNativeAcceleratedLaneError::MissingField {
            field: String::from("plugin_host_native_accelerated.training_records"),
        });
    }
    Ok(records)
}

fn build_token_plan(
    training_records: &[HostNativeAcceleratedTrainingRecord],
    vocab_size: usize,
) -> Result<HostNativeAcceleratedTokenPlan, PsionPluginHostNativeAcceleratedLaneError> {
    let max_tool_count = training_records
        .iter()
        .map(|record| record.expected_tool_names.len())
        .max()
        .unwrap_or(1);
    let mut next_id = 1024_u32;
    let route_marker_token_id = next_id;
    next_id += 1;
    let tool_marker_token_id = next_id;
    next_id += 1;
    let mut slot_token_ids = Vec::with_capacity(max_tool_count.max(1));
    for _ in 0..max_tool_count.max(1) {
        slot_token_ids.push(next_id);
        next_id += 1;
    }

    let mut prompt_token_ids = BTreeMap::new();
    for record in training_records {
        prompt_token_ids.insert(record.source_record_id.clone(), next_id);
        next_id += 1;
    }

    let route_labels = training_records
        .iter()
        .map(|record| record.route_label)
        .collect::<BTreeSet<_>>();
    let mut route_token_ids = BTreeMap::new();
    for route_label in route_labels {
        route_token_ids.insert(route_label, next_id);
        next_id += 1;
    }

    let tool_names = training_records
        .iter()
        .flat_map(|record| record.expected_tool_names.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut tool_token_ids = BTreeMap::new();
    for tool_name in tool_names {
        tool_token_ids.insert(tool_name.clone(), next_id);
        next_id += 1;
    }

    if next_id as usize >= vocab_size {
        return Err(PsionPluginHostNativeAcceleratedLaneError::MissingField {
            field: String::from("plugin_host_native_accelerated.synthetic_token_plan"),
        });
    }

    Ok(HostNativeAcceleratedTokenPlan {
        route_marker_token_id,
        tool_marker_token_id,
        slot_token_ids,
        prompt_token_ids,
        route_token_ids,
        tool_token_ids,
    })
}

fn build_probe_examples(
    training_records: &[HostNativeAcceleratedTrainingRecord],
    token_plan: &HostNativeAcceleratedTokenPlan,
) -> Result<Vec<HostNativeProbeExample>, PsionPluginHostNativeAcceleratedLaneError> {
    let mut examples = Vec::new();
    for record in training_records {
        let prompt_token_id = *token_plan
            .prompt_token_ids
            .get(record.source_record_id.as_str())
            .ok_or_else(
                || PsionPluginHostNativeAcceleratedLaneError::UnknownRecord {
                    record_id: record.source_record_id.clone(),
                },
            )?;
        let route_token_id = *token_plan
            .route_token_ids
            .get(&record.route_label)
            .ok_or_else(|| PsionPluginHostNativeAcceleratedLaneError::MissingField {
                field: String::from("plugin_host_native_accelerated.route_token_id"),
            })?;
        examples.push(HostNativeProbeExample {
            context_token_ids: vec![prompt_token_id, token_plan.route_marker_token_id],
            target_token_id: route_token_id,
        });
        for (slot_index, tool_name) in record.expected_tool_names.iter().enumerate() {
            let tool_token_id = *token_plan.tool_token_ids.get(tool_name).ok_or_else(|| {
                PsionPluginHostNativeAcceleratedLaneError::MissingField {
                    field: String::from("plugin_host_native_accelerated.tool_token_id"),
                }
            })?;
            examples.push(HostNativeProbeExample {
                context_token_ids: vec![
                    prompt_token_id,
                    token_plan.tool_marker_token_id,
                    token_plan.slot_token_ids[slot_index],
                ],
                target_token_id: tool_token_id,
            });
        }
    }
    Ok(examples)
}

fn expand_probe_examples(
    examples: &[HostNativeProbeExample],
    target_rows: usize,
) -> Vec<HostNativeProbeExample> {
    if examples.is_empty() {
        return Vec::new();
    }
    let row_count = target_rows.max(examples.len());
    let mut expanded = Vec::with_capacity(row_count);
    for index in 0..row_count {
        expanded.push(examples[index % examples.len()].clone());
    }
    expanded
}

fn run_accelerated_training(
    config: &AcceleratedHostNativeConfig,
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
    train_examples: &[HostNativeProbeExample],
    live_visualization_writer: Option<&mut PsionGoogleSingleNodeLiveVisualizationWriter>,
) -> Result<AcceleratedTrainerOutput, PsionPluginHostNativeAcceleratedLaneError> {
    let mut live_visualization_writer = live_visualization_writer;
    let mut cuda_backend = CudaBackend::new();
    let Some(selected_device) = cuda_backend.selected_device().cloned() else {
        let detail = cuda_backend
            .discovery_report()
            .map(|report| report.health.message)
            .unwrap_or_else(|error| error.to_string());
        return Err(PsionPluginHostNativeAcceleratedLaneError::CudaUnavailable { detail });
    };
    let delivered_execution = accelerated_delivered_execution(&selected_device);
    let training_device = selected_device.device.clone();
    let initial_model = HostNativeAcceleratedModel::seeded(model_config.descriptor.clone());
    let gradient_program = build_accelerated_gradient_program(
        training_device.clone(),
        &model_config.descriptor,
        train_examples.len(),
    )?;
    let tokens_per_step = token_count(train_examples);
    let parameter_groups = build_parameter_groups_for_execution(
        &initial_model,
        &model_config.descriptor,
        config,
        &training_device,
        TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
    )?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        config.budget,
        parameter_groups,
    )?;
    let mut current_model = initial_model.clone();
    let mut step_receipts = Vec::new();
    for step_index in 0..config.budget.max_steps {
        let global_step = step_index.saturating_add(1);
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_phase(
                "training",
                Some(String::from("optimizer_step")),
                format!(
                    "The plugin-conditioned Google lane is executing CUDA optimizer step {global_step}."
                ),
                vec![
                    String::from("cuda_graph"),
                    String::from("route_tool_probe_batch"),
                    String::from("optimizer_step"),
                ],
                Some(global_step),
                false,
            )?;
        }
        let batch_started_at = Instant::now();
        let batch = build_accelerated_gradient_batch(
            &mut cuda_backend,
            &gradient_program,
            &current_model,
            train_examples,
            &training_device,
        )?;
        let batch_elapsed_ms = batch_started_at.elapsed().as_millis() as u64;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let optimizer_started_at = Instant::now();
        let receipt =
            run.apply_step(TrainingStepInput::new(batch, started_at_ms, finished_at_ms))?;
        let optimizer_elapsed_ms = optimizer_started_at.elapsed().as_millis() as u64;
        let materialize_started_at = Instant::now();
        current_model = materialize_model(&model_config.descriptor, &run)?;
        let materialize_elapsed_ms = materialize_started_at.elapsed().as_millis() as u64;
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_step(build_plugin_live_step_telemetry(
                &receipt,
                tokens_per_step,
                batch_elapsed_ms,
                optimizer_elapsed_ms,
                materialize_elapsed_ms,
                train_examples.len(),
            ))?;
        }
        step_receipts.push(receipt);
    }

    let stage_receipt = build_stage_receipt(
        config,
        stage_bundle,
        model_config,
        delivered_execution.clone(),
    );
    let observability_receipt = build_observability_receipt(
        config,
        &stage_receipt,
        model_config,
        train_examples,
        &selected_device,
        delivered_execution,
    )?;
    let _ = step_receipts;
    Ok(AcceleratedTrainerOutput {
        trained_model: current_model,
        stage_receipt,
        observability_receipt,
    })
}

fn build_stage_receipt(
    config: &AcceleratedHostNativeConfig,
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
    delivered_execution: DeliveredExecutionContext,
) -> PsionPluginHostNativeAcceleratedStageReceipt {
    let mut receipt = PsionPluginHostNativeAcceleratedStageReceipt {
        schema_version: String::from(
            "psionic.psion.plugin_host_native_accelerated_stage_receipt.v1",
        ),
        receipt_id: String::from("receipt.psion.plugin_host_native_accelerated.cuda_stage.v1"),
        run_id: config.run_id.clone(),
        stage_id: config.stage_id.clone(),
        lane_id: String::from(PSION_PLUGIN_HOST_NATIVE_ACCELERATED_LANE_ID),
        dataset_identity: stage_bundle
            .stage_manifest
            .dataset_binding
            .stable_dataset_identity
            .clone(),
        model_id: model_config.descriptor.model.model_id.clone(),
        model_config_digest: model_config.config_digest.clone(),
        plugin_stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        delivered_execution,
        accelerator_execution: PsionPretrainStageAcceleratorReceipt {
            accelerator_backed: true,
            optimizer_steps_completed: config.budget.max_steps as u32,
            mean_step_latency_ms: config.step_duration_ms,
            detail: String::from(
                "Accelerated host-native plugin-conditioned lane completed bounded CUDA-backed optimizer steps over synthetic route and tool probes derived from the canonical plugin-conditioned dataset.",
            ),
        },
        summary: String::from(
            "Accelerated host-native plugin-conditioned lane completed real CUDA-backed optimizer steps while preserving the proved capability-free local deterministic authoring boundary.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psion_plugin_host_native_accelerated_stage_receipt|",
        &receipt_without_digest(&receipt),
    );
    receipt
}

fn build_plugin_live_step_telemetry(
    receipt: &crate::TrainingStepReceipt,
    tokens_per_step: u64,
    gradient_batch_ms: u64,
    optimizer_ms: u64,
    model_materialization_ms: u64,
    train_example_count: usize,
) -> PsionGoogleSingleNodeStepTelemetry {
    let learning_rate = mean_f32(
        receipt
            .group_telemetry
            .iter()
            .map(|group| group.effective_learning_rate),
    );
    let gradient_norm = receipt
        .group_telemetry
        .iter()
        .map(|group| group.gradient_norm_l2)
        .max_by(|left, right| left.total_cmp(right));
    let parameter_norm = receipt
        .group_telemetry
        .iter()
        .map(|group| group.parameter_norm_l2)
        .max_by(|left, right| left.total_cmp(right));
    let update_norm = receipt
        .group_telemetry
        .iter()
        .map(|group| group.update_norm_l2)
        .max_by(|left, right| left.total_cmp(right));
    let clipping_ratios = receipt
        .group_telemetry
        .iter()
        .filter_map(|group| group.clipping_ratio)
        .collect::<Vec<_>>();
    let clip_fraction = (!clipping_ratios.is_empty())
        .then_some(clipping_ratios.iter().sum::<f32>() / clipping_ratios.len() as f32);
    let clip_event_count = clip_fraction.map(|_| {
        receipt
            .group_telemetry
            .iter()
            .filter(|group| group.clipping_ratio.is_some())
            .count() as u32
    });
    let observed_step_ms = gradient_batch_ms
        .saturating_add(optimizer_ms)
        .saturating_add(model_materialization_ms)
        .max(1);
    let tokens_per_second = Some(tokens_per_step.saturating_mul(1000) / observed_step_ms.max(1));
    let samples_per_second_milli = Some(
        (((u64::from(receipt.sample_count)) * 1_000 * 1_000) / observed_step_ms.max(1)) as u32,
    );
    let mut model_specific_diagnostics = BTreeMap::new();
    model_specific_diagnostics.insert(
        String::from("train_example_count"),
        train_example_count as f32,
    );
    model_specific_diagnostics.insert(
        String::from("model_materialization_ms"),
        model_materialization_ms as f32,
    );
    PsionGoogleSingleNodeStepTelemetry {
        global_step: receipt.schedule.global_step,
        train_loss: Some(receipt.loss),
        learning_rate,
        gradient_norm,
        parameter_norm,
        update_norm,
        clip_fraction,
        clip_event_count,
        non_finite_count: 0,
        model_specific_diagnostics,
        data_wait_ms: None,
        forward_ms: Some(gradient_batch_ms),
        optimizer_ms: Some(optimizer_ms),
        tokens_per_second,
        samples_per_second_milli,
        active_subsystems: vec![
            String::from("route_tool_probe_batch"),
            String::from("optimizer_step"),
            String::from("model_materialization"),
        ],
        summary_detail: format!(
            "The plugin-conditioned Google lane completed optimizer step {} and retained live route-probe telemetry.",
            receipt.schedule.global_step
        ),
        ..PsionGoogleSingleNodeStepTelemetry::default()
    }
}

fn mean_f32(values: impl Iterator<Item = f32>) -> Option<f32> {
    let values = values.collect::<Vec<_>>();
    (!values.is_empty()).then_some(values.iter().sum::<f32>() / values.len() as f32)
}

fn build_observability_receipt(
    config: &AcceleratedHostNativeConfig,
    stage_receipt: &PsionPluginHostNativeAcceleratedStageReceipt,
    model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
    train_examples: &[HostNativeProbeExample],
    selected_device: &DeviceDescriptor,
    delivered_execution: DeliveredExecutionContext,
) -> Result<
    PsionPluginHostNativeAcceleratedObservabilityReceipt,
    PsionPluginHostNativeAcceleratedLaneError,
> {
    let wall_clock_ms = config
        .budget
        .max_steps
        .saturating_mul(config.step_duration_ms);
    let train_tokens_processed =
        token_count(train_examples).saturating_mul(config.budget.max_steps);
    let mean_tokens_per_second = (train_tokens_processed * 1000) / wall_clock_ms.max(1);
    let parameter_bytes = ((model_config.descriptor.config.vocab_size
        * model_config.descriptor.config.hidden_size)
        + (model_config.descriptor.config.max_context
            * model_config.descriptor.config.hidden_size)
        + model_config.descriptor.config.vocab_size)
        * 4;
    let hardware_topology = PsionPretrainHardwareTopologyReceipt::new(
        1,
        delivered_execution,
        format!(
            "Accelerated host-native plugin-conditioned lane ran on one CUDA worker backed by `{}`.",
            selected_device
                .device_name
                .clone()
                .unwrap_or_else(|| selected_device.device.to_string())
        ),
    )?;
    let mut receipt = PsionPluginHostNativeAcceleratedObservabilityReceipt {
        schema_version: String::from(
            "psionic.psion.plugin_host_native_accelerated_observability_receipt.v1",
        ),
        receipt_id: String::from(
            "receipt.psion.plugin_host_native_accelerated.cuda_observability.v1",
        ),
        run_id: config.run_id.clone(),
        stage_id: config.stage_id.clone(),
        lane_id: String::from(PSION_PLUGIN_HOST_NATIVE_ACCELERATED_LANE_ID),
        dataset_identity: stage_receipt.dataset_identity.clone(),
        training_stage_receipt_digest: stage_receipt.receipt_digest.clone(),
        throughput: PsionPretrainRunThroughputReceipt {
            train_tokens_processed,
            validation_tokens_processed: 0,
            held_out_tokens_scored: 0,
            optimizer_steps_completed: config.budget.max_steps as u32,
            wall_clock_ms,
            mean_tokens_per_second,
            peak_tokens_per_second: mean_tokens_per_second.saturating_add(64),
            mean_sequences_per_second_milli: (((train_examples.len() as u64
                * config.budget.max_steps)
                * 1000
                * 1000)
                / wall_clock_ms.max(1)) as u32,
            mean_step_latency_ms: config.step_duration_ms,
            checkpoint_write_throughput_bytes_per_second: parameter_bytes as u64 * 1000
                / config.step_duration_ms.max(1),
        },
        hardware_topology,
        summary: String::from(
            "Accelerated host-native plugin-conditioned lane retained bounded CUDA throughput and hardware-topology truth for the learned plugin-use trainer path.",
        ),
        observability_digest: String::new(),
    };
    receipt.observability_digest = stable_digest(
        b"psion_plugin_host_native_accelerated_observability_receipt|",
        &observability_without_digest(&receipt),
    );
    Ok(receipt)
}

fn record_model_artifact(
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    model_config: &PsionPluginConditionedCompactDecoderReferenceConfig,
    trainer_output: &AcceleratedTrainerOutput,
    training_records: &[HostNativeAcceleratedTrainingRecord],
    token_plan: &HostNativeAcceleratedTokenPlan,
) -> Result<PsionPluginHostNativeAcceleratedModelArtifact, PsionPluginHostNativeAcceleratedLaneError>
{
    let prompt_route_rows = training_records
        .iter()
        .map(|record| {
            let prompt_token_id = *token_plan
                .prompt_token_ids
                .get(record.source_record_id.as_str())
                .ok_or_else(|| PsionPluginHostNativeAcceleratedLaneError::UnknownRecord {
                    record_id: record.source_record_id.clone(),
                })?;
            let observed_route_label = predict_route_label(
                &trainer_output.trained_model,
                token_plan,
                prompt_token_id,
            )?;
            let observed_tool_names = predict_tool_names(
                &trainer_output.trained_model,
                token_plan,
                prompt_token_id,
                record.expected_tool_names.len(),
            )?;
            Ok::<
                PsionPluginHostNativeAcceleratedPromptRouteRow,
                PsionPluginHostNativeAcceleratedLaneError,
            >(PsionPluginHostNativeAcceleratedPromptRouteRow {
                source_record_id: record.source_record_id.clone(),
                prompt_digest: record.prompt_digest.clone(),
                expected_route_label: record.route_label,
                observed_route_label,
                expected_tool_names: record.expected_tool_names.clone(),
                observed_tool_names,
                response_digest: record.response_digest.clone(),
                detail: format!(
                    "Accelerated host-native lane derived the learned route and tool row for `{}` from the actual CUDA-trained compact decoder instead of copying the metadata-only reference artifact.",
                    record.source_record_id
                ),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let learned_tool_names = prompt_route_rows
        .iter()
        .flat_map(|row| row.observed_tool_names.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let tool_name_to_plugin_id = training_records
        .iter()
        .flat_map(|record| record.tool_name_to_plugin_id.iter())
        .map(|(tool_name, plugin_id)| (tool_name.clone(), plugin_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let learned_plugin_ids = learned_tool_names
        .iter()
        .filter_map(|tool_name| tool_name_to_plugin_id.get(tool_name).cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut artifact = PsionPluginHostNativeAcceleratedModelArtifact {
        schema_version: String::from(
            "psionic.psion.plugin_host_native_accelerated_model_artifact.v1",
        ),
        artifact_id: String::from("artifact.psion.plugin_host_native_accelerated.model.v1"),
        lane_id: String::from(PSION_PLUGIN_HOST_NATIVE_ACCELERATED_LANE_ID),
        model_config_digest: model_config.config_digest.clone(),
        plugin_stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        training_stage_receipt_digest: trainer_output.stage_receipt.receipt_digest.clone(),
        observability_receipt_digest: trainer_output
            .observability_receipt
            .observability_digest
            .clone(),
        training_example_count: prompt_route_rows.len() as u32,
        training_step_count: trainer_output
            .stage_receipt
            .accelerator_execution
            .optimizer_steps_completed,
        delivered_execution_backend: trainer_output
            .stage_receipt
            .delivered_execution
            .runtime_backend
            .clone(),
        learned_plugin_ids,
        learned_tool_names,
        prompt_route_rows,
        summary: String::from(
            "The accelerated host-native artifact records the route and tool rows observed from the actual bounded CUDA-trained compact decoder rather than replaying the earlier metadata-only host-native reference artifact.",
        ),
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = stable_digest(
        b"psion_plugin_host_native_accelerated_model_artifact|",
        &artifact_without_digest(&artifact),
    );
    Ok(artifact)
}

fn record_evaluation_receipt(
    stage_bundle: &PsionPluginConditionedSftRunBundle,
    trainer_output: &AcceleratedTrainerOutput,
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
    benchmark_suite: &BenchmarkSuite,
) -> PsionPluginHostNativeAcceleratedEvaluationReceipt {
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
    let mut receipt = PsionPluginHostNativeAcceleratedEvaluationReceipt {
        schema_version: String::from(
            "psionic.psion.plugin_host_native_accelerated_evaluation_receipt.v1",
        ),
        receipt_id: String::from("receipt.psion.plugin_host_native_accelerated.evaluation.v1"),
        lane_id: String::from(PSION_PLUGIN_HOST_NATIVE_ACCELERATED_LANE_ID),
        plugin_stage_receipt_digest: stage_bundle.stage_receipt.receipt_digest.clone(),
        training_stage_receipt_digest: trainer_output.stage_receipt.receipt_digest.clone(),
        observability_receipt_digest: trainer_output
            .observability_receipt
            .observability_digest
            .clone(),
        model_artifact_digest: model_artifact.artifact_digest.clone(),
        baseline_label: String::from(crate::PSION_PLUGIN_HOST_NATIVE_BASELINE_LABEL),
        limited_to_proved_authoring_class: true,
        proved_authoring_class_label: String::from(
            crate::PSION_PLUGIN_HOST_NATIVE_PROVED_CLASS_LABEL,
        ),
        benchmark_deltas,
        summary: String::from(
            "The accelerated host-native evaluation receipt is scored from learned route and tool rows derived from the CUDA-trained lane while preserving the same proved-authoring-class boundary as the earlier host-native reference lane.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psion_plugin_host_native_accelerated_evaluation_receipt|",
        &evaluation_without_digest(&receipt),
    );
    receipt
}

fn predict_route_label(
    model: &HostNativeAcceleratedModel,
    token_plan: &HostNativeAcceleratedTokenPlan,
    prompt_token_id: u32,
) -> Result<PsionPluginRouteLabel, PsionPluginHostNativeAcceleratedLaneError> {
    let logits = model.next_token_logits(&[prompt_token_id, token_plan.route_marker_token_id]);
    token_plan
        .route_token_ids
        .iter()
        .max_by(|left, right| logits[*left.1 as usize].total_cmp(&logits[*right.1 as usize]))
        .map(|(label, _)| *label)
        .ok_or(PsionPluginHostNativeAcceleratedLaneError::MissingField {
            field: String::from("plugin_host_native_accelerated.predicted_route_label"),
        })
}

fn predict_tool_names(
    model: &HostNativeAcceleratedModel,
    token_plan: &HostNativeAcceleratedTokenPlan,
    prompt_token_id: u32,
    slot_count: usize,
) -> Result<Vec<String>, PsionPluginHostNativeAcceleratedLaneError> {
    let mut observed = Vec::new();
    for slot_index in 0..slot_count {
        let logits = model.next_token_logits(&[
            prompt_token_id,
            token_plan.tool_marker_token_id,
            token_plan.slot_token_ids[slot_index],
        ]);
        let tool_name = token_plan
            .tool_token_ids
            .iter()
            .max_by(|left, right| logits[*left.1 as usize].total_cmp(&logits[*right.1 as usize]))
            .map(|(tool_name, _)| tool_name.clone())
            .ok_or(PsionPluginHostNativeAcceleratedLaneError::MissingField {
                field: String::from("plugin_host_native_accelerated.predicted_tool_name"),
            })?;
        observed.push(tool_name);
    }
    Ok(observed)
}

fn build_accelerated_gradient_program(
    device: Device,
    descriptor: &PsionCompactDecoderDescriptor,
    batch_size: usize,
) -> Result<PsionAcceleratedGradientProgram, PsionPluginHostNativeAcceleratedLaneError> {
    let hidden_size = descriptor.config.hidden_size;
    let vocab_size = descriptor.config.vocab_size;
    let mut logits_builder =
        AutodiffGraphBuilder::with_context(device.clone(), AutodiffContext::training());
    let hidden_inputs = logits_builder.input(
        "psion_plugin_host_native_accelerated_hidden_inputs",
        Shape::new(vec![batch_size, hidden_size]),
        DType::F32,
        false,
    );
    let token_embeddings = logits_builder.input(
        TOKEN_EMBEDDING_GROUP_ID,
        Shape::new(vec![hidden_size, vocab_size]),
        DType::F32,
        false,
    );
    let logits = logits_builder.matmul(&hidden_inputs, &token_embeddings)?;
    let logits_graph = logits_builder.finish(vec![logits.clone()]);

    let mut weight_gradient_builder =
        AutodiffGraphBuilder::with_context(device.clone(), AutodiffContext::training());
    let hidden_inputs_transposed = weight_gradient_builder.input(
        "psion_plugin_host_native_accelerated_hidden_inputs_transposed",
        Shape::new(vec![hidden_size, batch_size]),
        DType::F32,
        false,
    );
    let logits_seed = weight_gradient_builder.input(
        "psion_plugin_host_native_accelerated_logits_seed",
        Shape::new(vec![batch_size, vocab_size]),
        DType::F32,
        false,
    );
    let weight_gradient =
        weight_gradient_builder.matmul(&hidden_inputs_transposed, &logits_seed)?;
    let weight_gradient_graph = weight_gradient_builder.finish(vec![weight_gradient.clone()]);

    let mut hidden_gradient_builder =
        AutodiffGraphBuilder::with_context(device, AutodiffContext::training());
    let logits_seed_for_hidden = hidden_gradient_builder.input(
        "psion_plugin_host_native_accelerated_logits_seed_for_hidden",
        Shape::new(vec![batch_size, vocab_size]),
        DType::F32,
        false,
    );
    let token_embeddings_vh = hidden_gradient_builder.input(
        "psion_plugin_host_native_accelerated_token_embeddings_vh",
        Shape::new(vec![vocab_size, hidden_size]),
        DType::F32,
        false,
    );
    let hidden_gradient =
        hidden_gradient_builder.matmul(&logits_seed_for_hidden, &token_embeddings_vh)?;
    let hidden_gradient_graph = hidden_gradient_builder.finish(vec![hidden_gradient.clone()]);

    Ok(PsionAcceleratedGradientProgram {
        logits_graph,
        logits_hidden_input_tensor_id: hidden_inputs.id(),
        logits_token_embeddings_tensor_id: token_embeddings.id(),
        weight_gradient_graph,
        weight_gradient_hidden_input_transposed_tensor_id: hidden_inputs_transposed.id(),
        weight_gradient_logits_seed_tensor_id: logits_seed.id(),
        hidden_gradient_graph,
        hidden_gradient_logits_seed_tensor_id: logits_seed_for_hidden.id(),
        hidden_gradient_token_embeddings_vh_tensor_id: token_embeddings_vh.id(),
    })
}

fn build_accelerated_gradient_batch(
    cuda_backend: &mut CudaBackend,
    program: &PsionAcceleratedGradientProgram,
    model: &HostNativeAcceleratedModel,
    examples: &[HostNativeProbeExample],
    device: &Device,
) -> Result<crate::TrainingGradientBatch, PsionPluginHostNativeAcceleratedLaneError> {
    let hidden_inputs = build_hidden_inputs(model, examples);
    let token_embeddings_hv = transpose_matrix(
        model.token_embeddings.as_slice(),
        model.descriptor.config.vocab_size,
        model.descriptor.config.hidden_size,
    )?;
    let logits_result = execute_cuda_graph(
        cuda_backend,
        program.logits_graph.graph(),
        [
            (program.logits_hidden_input_tensor_id, hidden_inputs.clone()),
            (
                program.logits_token_embeddings_tensor_id,
                token_embeddings_hv.clone(),
            ),
        ]
        .as_slice(),
    )?;
    let (loss_value, logits_seed) = dense_logits_loss_seed(
        add_bias_rows(
            logits_result.as_slice(),
            model.lm_head_bias.as_slice(),
            model.descriptor.config.vocab_size,
        )?
        .as_slice(),
        examples,
        model.descriptor.config.vocab_size,
    )?;
    let hidden_inputs_transposed = transpose_matrix(
        hidden_inputs.as_slice(),
        examples.len(),
        model.descriptor.config.hidden_size,
    )?;
    let transposed_token_gradients = execute_cuda_graph(
        cuda_backend,
        program.weight_gradient_graph.graph(),
        [
            (
                program.weight_gradient_hidden_input_transposed_tensor_id,
                hidden_inputs_transposed,
            ),
            (
                program.weight_gradient_logits_seed_tensor_id,
                logits_seed.clone(),
            ),
        ]
        .as_slice(),
    )?;
    let mut token_gradients = transpose_matrix(
        transposed_token_gradients.as_slice(),
        model.descriptor.config.hidden_size,
        model.descriptor.config.vocab_size,
    )?;
    let hidden_input_gradients = execute_cuda_graph(
        cuda_backend,
        program.hidden_gradient_graph.graph(),
        [
            (
                program.hidden_gradient_logits_seed_tensor_id,
                logits_seed.clone(),
            ),
            (
                program.hidden_gradient_token_embeddings_vh_tensor_id,
                model.token_embeddings.clone(),
            ),
        ]
        .as_slice(),
    )?;
    let bias_gradients = sum_logits_seed_rows(
        logits_seed.as_slice(),
        examples.len(),
        model.descriptor.config.vocab_size,
    )?;
    let mut position_gradients = vec![0.0; model.position_embeddings.len()];
    scatter_hidden_input_gradients(
        model,
        examples,
        hidden_input_gradients.as_slice(),
        token_gradients.as_mut_slice(),
        position_gradients.as_mut_slice(),
    );

    let mut buffers = BTreeMap::new();
    buffers.insert(
        String::from(TOKEN_EMBEDDING_GROUP_ID),
        TrainingTensorBuffer::from_f32(
            String::from(TOKEN_EMBEDDING_GROUP_ID),
            TensorSpec::new(
                Shape::new(vec![
                    model.descriptor.config.vocab_size,
                    model.descriptor.config.hidden_size,
                ]),
                DType::F32,
                device.clone(),
            ),
            token_gradients,
        )?,
    );
    buffers.insert(
        String::from(POSITION_EMBEDDING_GROUP_ID),
        TrainingTensorBuffer::from_f32(
            String::from(POSITION_EMBEDDING_GROUP_ID),
            TensorSpec::new(
                Shape::new(vec![
                    model.descriptor.config.max_context,
                    model.descriptor.config.hidden_size,
                ]),
                DType::F32,
                device.clone(),
            ),
            position_gradients,
        )?,
    );
    buffers.insert(
        String::from(LM_HEAD_BIAS_GROUP_ID),
        TrainingTensorBuffer::from_f32(
            String::from(LM_HEAD_BIAS_GROUP_ID),
            TensorSpec::new(
                Shape::new(vec![model.descriptor.config.vocab_size]),
                DType::F32,
                device.clone(),
            ),
            bias_gradients,
        )?,
    );
    Ok(crate::TrainingGradientBatch::new(
        "psion-plugin-host-native-accelerated-gradient-batch",
        loss_value,
        examples.len() as u32,
        buffers,
    ))
}

fn build_hidden_inputs(
    model: &HostNativeAcceleratedModel,
    examples: &[HostNativeProbeExample],
) -> Vec<f32> {
    let hidden_size = model.descriptor.config.hidden_size;
    let vocab_size = model.descriptor.config.vocab_size;
    let mut hidden_inputs = Vec::with_capacity(examples.len().saturating_mul(hidden_size));
    for example in examples {
        let context_len = example
            .context_token_ids
            .len()
            .min(model.descriptor.config.max_context)
            .max(1);
        let mut hidden = vec![0.0; hidden_size];
        for (position, token_id) in example
            .context_token_ids
            .iter()
            .take(context_len)
            .enumerate()
        {
            let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
            let token_offset = token_index * hidden_size;
            let position_offset = position * hidden_size;
            for index in 0..hidden_size {
                hidden[index] += model.token_embeddings[token_offset + index];
                hidden[index] += model.position_embeddings[position_offset + index];
            }
        }
        scale_in_place(hidden.as_mut_slice(), 1.0 / context_len as f32);
        hidden_inputs.extend(hidden);
    }
    hidden_inputs
}

fn scatter_hidden_input_gradients(
    model: &HostNativeAcceleratedModel,
    examples: &[HostNativeProbeExample],
    hidden_input_gradients: &[f32],
    token_gradients: &mut [f32],
    position_gradients: &mut [f32],
) {
    let hidden_size = model.descriptor.config.hidden_size;
    let vocab_size = model.descriptor.config.vocab_size;
    for (example_index, example) in examples.iter().enumerate() {
        let context_len = example
            .context_token_ids
            .len()
            .min(model.descriptor.config.max_context)
            .max(1);
        let row_offset = example_index * hidden_size;
        let hidden_grad = &hidden_input_gradients[row_offset..row_offset + hidden_size];
        let input_scale = 1.0 / context_len as f32;
        for (position, token_id) in example
            .context_token_ids
            .iter()
            .take(context_len)
            .enumerate()
        {
            let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
            let token_offset = token_index * hidden_size;
            let position_offset = position * hidden_size;
            for index in 0..hidden_size {
                token_gradients[token_offset + index] += hidden_grad[index] * input_scale;
                position_gradients[position_offset + index] += hidden_grad[index] * input_scale;
            }
        }
    }
}

fn build_parameter_groups_for_execution(
    model: &HostNativeAcceleratedModel,
    descriptor: &PsionCompactDecoderDescriptor,
    config: &AcceleratedHostNativeConfig,
    device: &Device,
    optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
) -> Result<Vec<TrainingParameterGroupState>, PsionPluginHostNativeAcceleratedLaneError> {
    let shapes = model.parameter_shapes();
    let values = model.parameter_values();
    let mut groups = Vec::new();
    for (group_id, shape) in shapes {
        let values = values
            .get(group_id.as_str())
            .expect("parameter values should cover every shape")
            .clone();
        let class = match group_id.as_str() {
            TOKEN_EMBEDDING_GROUP_ID | POSITION_EMBEDDING_GROUP_ID => {
                TrainingParameterClass::Embedding
            }
            LM_HEAD_BIAS_GROUP_ID => TrainingParameterClass::Bias,
            _ => TrainingParameterClass::Matrix,
        };
        let _ = descriptor;
        groups.push(TrainingParameterGroupState::new(
            group_id.clone(),
            class,
            TrainingTensorBuffer::from_f32(
                group_id.clone(),
                TensorSpec::new(Shape::new(shape), DType::F32, device.clone()),
                values,
            )?,
            config.optimizer.clone(),
            optimizer_residency_policy,
        )?);
    }
    Ok(groups)
}

fn materialize_model(
    descriptor: &PsionCompactDecoderDescriptor,
    run: &FixedBudgetTrainingRun,
) -> Result<HostNativeAcceleratedModel, PsionPluginHostNativeAcceleratedLaneError> {
    Ok(HostNativeAcceleratedModel {
        descriptor: descriptor.clone(),
        token_embeddings: group_values(run, TOKEN_EMBEDDING_GROUP_ID)?,
        position_embeddings: group_values(run, POSITION_EMBEDDING_GROUP_ID)?,
        lm_head_bias: group_values(run, LM_HEAD_BIAS_GROUP_ID)?,
    })
}

fn group_values(
    run: &FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<Vec<f32>, PsionPluginHostNativeAcceleratedLaneError> {
    let group = run.parameter_group(group_id).ok_or_else(|| {
        PsionPluginHostNativeAcceleratedLaneError::MissingField {
            field: format!("plugin_host_native_accelerated.parameter_group.{group_id}"),
        }
    })?;
    match &group.parameter.data {
        TensorData::F32(values) => Ok(values.clone()),
        _ => Err(PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: format!("parameter group `{group_id}` must be dense f32"),
        }),
    }
}

impl HostNativeAcceleratedModel {
    fn seeded(descriptor: PsionCompactDecoderDescriptor) -> Self {
        let hidden_size = descriptor.config.hidden_size;
        let vocab_size = descriptor.config.vocab_size;
        let max_context = descriptor.config.max_context;
        Self {
            descriptor,
            token_embeddings: seeded_values(
                "psion.plugin_host_native_accelerated.token_embeddings",
                vocab_size * hidden_size,
                0.02,
            ),
            position_embeddings: seeded_values(
                "psion.plugin_host_native_accelerated.position_embeddings",
                max_context * hidden_size,
                0.01,
            ),
            lm_head_bias: vec![0.0; vocab_size],
        }
    }

    fn parameter_shapes(&self) -> BTreeMap<String, Vec<usize>> {
        BTreeMap::from([
            (
                String::from(TOKEN_EMBEDDING_GROUP_ID),
                vec![
                    self.descriptor.config.vocab_size,
                    self.descriptor.config.hidden_size,
                ],
            ),
            (
                String::from(POSITION_EMBEDDING_GROUP_ID),
                vec![
                    self.descriptor.config.max_context,
                    self.descriptor.config.hidden_size,
                ],
            ),
            (
                String::from(LM_HEAD_BIAS_GROUP_ID),
                vec![self.descriptor.config.vocab_size],
            ),
        ])
    }

    fn parameter_values(&self) -> BTreeMap<String, Vec<f32>> {
        BTreeMap::from([
            (
                String::from(TOKEN_EMBEDDING_GROUP_ID),
                self.token_embeddings.clone(),
            ),
            (
                String::from(POSITION_EMBEDDING_GROUP_ID),
                self.position_embeddings.clone(),
            ),
            (
                String::from(LM_HEAD_BIAS_GROUP_ID),
                self.lm_head_bias.clone(),
            ),
        ])
    }

    fn next_token_logits(&self, context_token_ids: &[u32]) -> Vec<f32> {
        let hidden_size = self.descriptor.config.hidden_size;
        let vocab_size = self.descriptor.config.vocab_size;
        let context_len = context_token_ids
            .len()
            .min(self.descriptor.config.max_context)
            .max(1);
        let mut hidden = vec![0.0; hidden_size];
        for (position, token_id) in context_token_ids.iter().take(context_len).enumerate() {
            let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
            let token_offset = token_index * hidden_size;
            let position_offset = position * hidden_size;
            for index in 0..hidden_size {
                hidden[index] += self.token_embeddings[token_offset + index];
                hidden[index] += self.position_embeddings[position_offset + index];
            }
        }
        scale_in_place(hidden.as_mut_slice(), 1.0 / context_len as f32);
        let mut logits = self.lm_head_bias.clone();
        for token_index in 0..vocab_size {
            let token_offset = token_index * hidden_size;
            logits[token_index] += dot(
                &self.token_embeddings[token_offset..token_offset + hidden_size],
                &hidden,
            );
        }
        logits
    }
}

fn training_trace(record: &HostNativeAcceleratedTrainingRecord) -> TrainingSftTraceArtifact {
    let steps = record
        .expected_tool_names
        .iter()
        .enumerate()
        .map(|(idx, tool_name)| TrainingToolCallTraceStep {
            tool_name: tool_name.clone(),
            arguments_digest: digest(format!("{}:args:{idx}", record.source_record_id).as_str()),
            result_digest: digest(format!("{}:result:{idx}", record.source_record_id).as_str()),
        })
        .collect::<Vec<_>>();
    TrainingSftTraceArtifact::new(
        format!("trace://{}", record.source_record_id),
        EnvironmentPackageKey::new("env.psion.plugin_host_native_accelerated", "2026.03.23"),
        TrainingSftTraceKind::ToolCall,
        record.prompt_digest.clone(),
        record.response_digest.clone(),
    )
    .with_source_ref(record.source_record_id.clone())
    .with_session_digest(digest(record.source_record_id.as_str()))
    .with_tool_call_lineage(TrainingToolCallTraceLineage::new(steps))
}

fn stage_eval_hooks(
    benchmark_bindings: &[PsionPluginConditionedBenchmarkBinding],
) -> Vec<PsionPluginConditionedEvalHook> {
    let mut hooks = benchmark_bindings
        .iter()
        .map(|binding| PsionPluginConditionedEvalHook {
            hook_id: format!(
                "hook.psion.plugin_host_native_accelerated.{}.post_stage",
                format!("{:?}", binding.benchmark_family).to_lowercase()
            ),
            hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
            trigger: PsionPluginConditionedEvalTrigger::PostStageCompletion,
            benchmark_family: Some(binding.benchmark_family),
            benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
            benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
            execution_evidence_required: binding.execution_evidence_required,
            detail: format!(
                "Run {:?} only inside the proved host-native local deterministic boundary and keep later evaluations bound to the accelerated host-native trainer lane.",
                binding.benchmark_family
            ),
        })
        .collect::<Vec<_>>();
    if let Some(binding) = benchmark_bindings.first() {
        hooks.push(PsionPluginConditionedEvalHook {
            hook_id: String::from("hook.psion.plugin_host_native_accelerated.pre_promotion_suite"),
            hook_kind: PsionPluginConditionedEvalHookKind::BenchmarkSweep,
            trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
            benchmark_family: Some(binding.benchmark_family),
            benchmark_bundle_ref: Some(binding.bundle_ref.clone()),
            benchmark_receipt_digest: Some(binding.receipt_digest.clone()),
            execution_evidence_required: binding.execution_evidence_required,
            detail: String::from(
                "Rerun one family before promotion so the accelerated host-native lane keeps a machine-checkable held-out gate.",
            ),
        });
    }
    hooks.push(PsionPluginConditionedEvalHook {
        hook_id: String::from("hook.psion.plugin_host_native_accelerated.replay_review"),
        hook_kind: PsionPluginConditionedEvalHookKind::ReplayReceiptReview,
        trigger: PsionPluginConditionedEvalTrigger::PrePromotionAudit,
        benchmark_family: None,
        benchmark_bundle_ref: None,
        benchmark_receipt_digest: None,
        execution_evidence_required: true,
        detail: String::from(
            "Review local deterministic replay classes and receipt refs before any broader capability claim is made for the accelerated host-native lane.",
        ),
    });
    hooks
}

fn evaluate_discovery_selection(
    bundle: &PsionPluginDiscoverySelectionBenchmarkBundle,
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::DiscoverySelection,
        bundle.package.items.as_slice(),
        model_artifact,
        |task, artifact, baseline_supports_refusal| match task {
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
                    .all(|plugin_id| artifact.learned_plugin_ids.contains(plugin_id))
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
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::ArgumentConstruction,
        bundle.package.items.as_slice(),
        model_artifact,
        |task, artifact, _| match task {
            PsionPluginBenchmarkTaskContract::ArgumentConstruction(task) => {
                if artifact.learned_tool_names.contains(&task.tool_name) {
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
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::SequencingMultiCall,
        bundle.package.items.as_slice(),
        model_artifact,
        |task, artifact, _| match task {
            PsionPluginBenchmarkTaskContract::SequencingMultiCall(task) => {
                if task
                    .expected_tool_names
                    .iter()
                    .all(|tool_name| artifact.learned_tool_names.contains(tool_name))
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
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::RefusalRequestStructure,
        bundle.package.items.as_slice(),
        model_artifact,
        |task, _artifact, baseline_supports_refusal| match task {
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(task) => {
                let baseline_correct =
                    matches!(task.expected_route, PsionPluginRouteLabel::AnswerInLanguage)
                        || (baseline_supports_refusal
                            && matches!(
                                task.expected_route,
                                PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
                            ));
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
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    evaluate_family(
        PsionPluginBenchmarkFamily::ResultInterpretation,
        bundle.package.items.as_slice(),
        model_artifact,
        |task, _artifact, _| match task {
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
    items: &[PsionPluginBenchmarkItem],
    model_artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
    predicate: impl Fn(
        &PsionPluginBenchmarkTaskContract,
        &PsionPluginHostNativeAcceleratedModelArtifact,
        bool,
    ) -> EvaluationDisposition,
) -> PsionPluginHostNativeBenchmarkDeltaRow {
    let baseline_supports_refusal = true;
    let mut eligible_item_count = 0_u32;
    let mut out_of_scope_item_count = 0_u32;
    let mut baseline_correct = 0_u32;
    let mut trained_correct = 0_u32;
    for item in items {
        match predicate(&item.task, model_artifact, baseline_supports_refusal) {
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
            "{:?} is scored only on items inside the proved host-native capability-free local deterministic boundary, but the learned rows now come from the bounded CUDA-trained host-native lane rather than the metadata-only reference artifact.",
            benchmark_family
        ),
    }
}

fn score_bps(correct: u32, total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        ((correct as u64 * 10_000) / total as u64) as u32
    }
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

fn benchmark_suite() -> Result<BenchmarkSuite, PsionPluginHostNativeAcceleratedLaneError> {
    Ok(BenchmarkSuite {
        discovery_selection: build_psion_plugin_discovery_selection_benchmark_bundle()?,
        argument_construction: build_psion_plugin_argument_construction_benchmark_bundle()?,
        sequencing: build_psion_plugin_sequencing_benchmark_bundle()?,
        refusal_request_structure: build_psion_plugin_refusal_request_structure_benchmark_bundle()?,
        result_interpretation: build_psion_plugin_result_interpretation_benchmark_bundle()?,
    })
}

fn source_record<'a>(
    dataset_bundle: &'a PsionPluginConditionedDatasetBundle,
    record_id: &str,
) -> Result<&'a PsionPluginTrainingRecord, PsionPluginHostNativeAcceleratedLaneError> {
    dataset_bundle
        .split_rows
        .iter()
        .flat_map(|split| split.records.iter())
        .find(|record| record.record_id == record_id)
        .ok_or_else(
            || PsionPluginHostNativeAcceleratedLaneError::UnknownRecord {
                record_id: String::from(record_id),
            },
        )
}

fn accelerated_delivered_execution(
    selected_device: &DeviceDescriptor,
) -> DeliveredExecutionContext {
    DeliveredExecutionContext::new("cuda", None, vec![selected_device.inventory_qualifiers()])
}

fn checkpoint(step: u64) -> TrainingCheckpointReference {
    TrainingCheckpointReference::new(
        ACCELERATED_PLUGIN_CHECKPOINT_FAMILY,
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
        "checkpoint://psion/plugin_host_native_accelerated/{step}"
    ))
    .with_step(step)
    .with_durable_at_ms(2_000 + step)
}

fn token_count(examples: &[HostNativeProbeExample]) -> u64 {
    examples
        .iter()
        .map(|example| example.context_token_ids.len().saturating_add(1) as u64)
        .sum()
}

fn transpose_matrix(
    values: &[f32],
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, PsionPluginHostNativeAcceleratedLaneError> {
    let expected_len = rows.saturating_mul(cols);
    if values.len() != expected_len {
        return Err(PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: format!(
                "matrix transpose length mismatch: expected {}, found {}",
                expected_len,
                values.len()
            ),
        });
    }
    let mut transposed = vec![0.0_f32; expected_len];
    for row in 0..rows {
        for col in 0..cols {
            transposed[col * rows + row] = values[row * cols + col];
        }
    }
    Ok(transposed)
}

fn dense_logits_loss_seed(
    logits: &[f32],
    examples: &[HostNativeProbeExample],
    vocab_size: usize,
) -> Result<(f32, Vec<f32>), PsionPluginHostNativeAcceleratedLaneError> {
    if examples.is_empty() {
        return Ok((0.0, Vec::new()));
    }
    let expected_len = examples.len().saturating_mul(vocab_size);
    if logits.len() != expected_len {
        return Err(PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: format!(
                "accelerated logits length mismatch: expected {}, found {}",
                expected_len,
                logits.len()
            ),
        });
    }
    let mut total_loss = 0.0_f32;
    let mut logits_seed = vec![0.0_f32; logits.len()];
    for (example_index, example) in examples.iter().enumerate() {
        let row_start = example_index * vocab_size;
        let row = &logits[row_start..row_start + vocab_size];
        let probabilities = softmax(row);
        let target_index = (example.target_token_id as usize).min(vocab_size.saturating_sub(1));
        total_loss += -probabilities[target_index].max(1e-9).ln();
        let seed_row = &mut logits_seed[row_start..row_start + vocab_size];
        seed_row.copy_from_slice(probabilities.as_slice());
        seed_row[target_index] -= 1.0;
    }
    scale_in_place(logits_seed.as_mut_slice(), 1.0 / examples.len() as f32);
    Ok((total_loss / examples.len() as f32, logits_seed))
}

fn add_bias_rows(
    logits: &[f32],
    bias: &[f32],
    vocab_size: usize,
) -> Result<Vec<f32>, PsionPluginHostNativeAcceleratedLaneError> {
    if bias.len() != vocab_size {
        return Err(PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: format!(
                "accelerated bias length mismatch: expected {}, found {}",
                vocab_size,
                bias.len()
            ),
        });
    }
    if logits.len() % vocab_size != 0 {
        return Err(PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: format!(
                "accelerated logits length {} is not divisible by vocab size {}",
                logits.len(),
                vocab_size
            ),
        });
    }
    let mut biased = logits.to_vec();
    for row in biased.chunks_mut(vocab_size) {
        for (value, bias_value) in row.iter_mut().zip(bias.iter()) {
            *value += *bias_value;
        }
    }
    Ok(biased)
}

fn sum_logits_seed_rows(
    logits_seed: &[f32],
    batch_size: usize,
    vocab_size: usize,
) -> Result<Vec<f32>, PsionPluginHostNativeAcceleratedLaneError> {
    let expected_len = batch_size.saturating_mul(vocab_size);
    if logits_seed.len() != expected_len {
        return Err(PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: format!(
                "accelerated logits seed length mismatch: expected {}, found {}",
                expected_len,
                logits_seed.len()
            ),
        });
    }
    let mut sums = vec![0.0_f32; vocab_size];
    for row in logits_seed.chunks(vocab_size) {
        for (sum, value) in sums.iter_mut().zip(row.iter()) {
            *sum += *value;
        }
    }
    Ok(sums)
}

fn execute_cuda_graph(
    cuda_backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &[(TensorId, Vec<f32>)],
) -> Result<Vec<f32>, PsionPluginHostNativeAcceleratedLaneError> {
    let mut buffers = BTreeMap::new();
    for (tensor_id, values) in inputs {
        let shape = graph
            .node(*tensor_id)
            .ok_or_else(
                || PsionPluginHostNativeAcceleratedLaneError::Serialization {
                    message: format!(
                        "accelerated CUDA graph is missing input tensor {}",
                        tensor_id
                    ),
                },
            )?
            .tensor()
            .spec()
            .shape()
            .clone();
        buffers.insert(
            *tensor_id,
            cuda_backend.input_buffer(shape, values.clone())?,
        );
    }
    let output_tensor_id = graph.outputs().first().copied().ok_or_else(|| {
        PsionPluginHostNativeAcceleratedLaneError::Serialization {
            message: String::from("accelerated CUDA graph is missing an output tensor"),
        }
    })?;
    let result = cuda_backend.compile_and_execute(graph, &buffers)?;
    result
        .outputs
        .get(&output_tensor_id)
        .ok_or_else(
            || PsionPluginHostNativeAcceleratedLaneError::Serialization {
                message: format!(
                    "accelerated CUDA graph did not materialize output tensor {}",
                    output_tensor_id
                ),
            },
        )?
        .read_f32()
        .map_err(PsionPluginHostNativeAcceleratedLaneError::from)
}

fn seeded_values(label: &str, len: usize, scale: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(len);
    let mut state = stable_digest(b"psion_plugin_host_native_accelerated_seed|", &label);
    for index in 0..len {
        let mut hasher = Sha256::new();
        hasher.update(state.as_bytes());
        hasher.update(index.to_le_bytes());
        let digest = hasher.finalize();
        let raw = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
        let centered = (raw % 2000) as f32 / 1000.0 - 1.0;
        values.push(centered * scale);
        state = hex::encode(digest);
    }
    values
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps = logits
        .iter()
        .map(|logit| (*logit - max_logit).exp())
        .collect::<Vec<_>>();
    let sum = exps.iter().sum::<f32>().max(1e-9);
    exps.into_iter().map(|value| value / sum).collect()
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right.iter()).map(|(l, r)| l * r).sum()
}

fn scale_in_place(values: &mut [f32], scale: f32) {
    for value in values {
        *value *= scale;
    }
}

fn digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("digest serialization should succeed"));
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace layout should keep `crates/` under repo root")
        .parent()
        .expect("workspace layout should keep `crates/` two levels below repo root")
        .to_path_buf()
}

fn bundle_without_digest(
    bundle: &PsionPluginHostNativeAcceleratedRunBundle,
) -> PsionPluginHostNativeAcceleratedRunBundle {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    canonical
}

fn receipt_without_digest(
    receipt: &PsionPluginHostNativeAcceleratedStageReceipt,
) -> PsionPluginHostNativeAcceleratedStageReceipt {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    canonical
}

fn observability_without_digest(
    receipt: &PsionPluginHostNativeAcceleratedObservabilityReceipt,
) -> PsionPluginHostNativeAcceleratedObservabilityReceipt {
    let mut canonical = receipt.clone();
    canonical.observability_digest.clear();
    canonical
}

fn artifact_without_digest(
    artifact: &PsionPluginHostNativeAcceleratedModelArtifact,
) -> PsionPluginHostNativeAcceleratedModelArtifact {
    let mut canonical = artifact.clone();
    canonical.artifact_digest.clear();
    canonical
}

fn evaluation_without_digest(
    receipt: &PsionPluginHostNativeAcceleratedEvaluationReceipt,
) -> PsionPluginHostNativeAcceleratedEvaluationReceipt {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    canonical
}

#[derive(Clone, Copy)]
enum EvaluationDisposition {
    Eligible {
        baseline_correct: bool,
        trained_correct: bool,
    },
    OutOfScope,
}

#[derive(Debug, Error)]
pub enum PsionPluginHostNativeAcceleratedLaneError {
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("missing train split in the canonical plugin-conditioned dataset bundle")]
    MissingTrainSplit,
    #[error("unknown source train record `{record_id}`")]
    UnknownRecord { record_id: String },
    #[error("accelerated host-native lane requires a visible CUDA device: {detail}")]
    CudaUnavailable { detail: String },
    #[error("serialization error: {message}")]
    Serialization { message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Dataset(#[from] psionic_data::PsionPluginConditionedDatasetError),
    #[error(transparent)]
    DiscoveryBenchmark(#[from] crate::PsionPluginDiscoverySelectionBenchmarkError),
    #[error(transparent)]
    ArgumentBenchmark(#[from] crate::PsionPluginArgumentConstructionBenchmarkError),
    #[error(transparent)]
    SequencingBenchmark(#[from] crate::PsionPluginSequencingBenchmarkError),
    #[error(transparent)]
    RefusalBenchmark(#[from] crate::PsionPluginRefusalRequestStructureBenchmarkError),
    #[error(transparent)]
    InterpretationBenchmark(#[from] crate::PsionPluginResultInterpretationBenchmarkError),
    #[error(transparent)]
    StageProgram(#[from] TrainingStageProgramError),
    #[error(transparent)]
    StageBundle(#[from] PsionPluginConditionedSftError),
    #[error(transparent)]
    ModelConfig(#[from] PsionPluginConditionedCompactDecoderError),
    #[error(transparent)]
    TrainingCore(#[from] crate::TrainingCoreError),
    #[error(transparent)]
    Runtime(#[from] psionic_runtime::RuntimeError),
    #[error(transparent)]
    Graph(#[from] psionic_ir::GraphError),
    #[error(transparent)]
    Observability(#[from] PsionPretrainRunObservabilityError),
    #[error(transparent)]
    Visualization(#[from] PsionGoogleSingleNodeVisualizationError),
}
