use std::{error::Error, fs, path::PathBuf};

use psionic_data::{build_psion_plugin_conditioned_dataset_bundle, DatasetSplitKind};
use psionic_environments::EnvironmentPackageKey;
use psionic_runtime::TrainingCheckpointReference;
use psionic_train::{
    build_psion_plugin_argument_construction_benchmark_bundle,
    build_psion_plugin_discovery_selection_benchmark_bundle,
    build_psion_plugin_refusal_request_structure_benchmark_bundle,
    build_psion_plugin_result_interpretation_benchmark_bundle,
    build_psion_plugin_sequencing_benchmark_bundle,
    psion_plugin_argument_construction_benchmark_binding,
    psion_plugin_discovery_selection_benchmark_binding,
    psion_plugin_refusal_request_structure_benchmark_binding,
    psion_plugin_result_interpretation_benchmark_binding,
    psion_plugin_sequencing_benchmark_binding, record_psion_plugin_conditioned_sft_run_bundle,
    record_psion_plugin_conditioned_sft_stage_manifest,
    record_psion_plugin_conditioned_sft_stage_receipt, PsionPluginConditionedEvalHook,
    PsionPluginConditionedEvalHookKind, PsionPluginConditionedEvalTrigger,
    PsionPluginConditionedSftStageConfig, PsionPluginConditionedTraceBinding,
    TrainingLongContextTraceLineage, TrainingSftTraceArtifact, TrainingSftTraceKind,
    TrainingStageKind, TrainingStageProgramState, TrainingToolCallTraceLineage,
    TrainingToolCallTraceStep,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1");
    fs::create_dir_all(&fixtures_dir)?;

    let dataset_bundle = build_psion_plugin_conditioned_dataset_bundle()?;
    let stage_program = plugin_conditioned_stage_program(&dataset_bundle)?;
    let benchmark_bindings = vec![
        psion_plugin_discovery_selection_benchmark_binding(
            &build_psion_plugin_discovery_selection_benchmark_bundle()?,
        ),
        psion_plugin_argument_construction_benchmark_binding(
            &build_psion_plugin_argument_construction_benchmark_bundle()?,
        ),
        psion_plugin_sequencing_benchmark_binding(
            &build_psion_plugin_sequencing_benchmark_bundle()?
        ),
        psion_plugin_refusal_request_structure_benchmark_binding(
            &build_psion_plugin_refusal_request_structure_benchmark_bundle()?,
        ),
        psion_plugin_result_interpretation_benchmark_binding(
            &build_psion_plugin_result_interpretation_benchmark_bundle()?,
        ),
    ];
    let trace_bindings = trace_bindings(&dataset_bundle)?;
    let max_plugin_calls_per_trace = max_plugin_calls_per_trace(&trace_bindings);
    let eval_hooks = eval_hooks(benchmark_bindings.as_slice());
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
    let run_bundle = record_psion_plugin_conditioned_sft_run_bundle(
        "bundle.psion.plugin_conditioned_sft.reference.v1",
        &dataset_bundle,
        stage_program.clone(),
        stage_manifest.clone(),
        stage_receipt.clone(),
        "Bounded output bundle for the first canonical plugin-conditioned SFT stage.",
    )?;

    write_json(
        fixtures_dir.join("psion_plugin_conditioned_sft_stage_manifest.json"),
        &stage_manifest,
    )?;
    write_json(
        fixtures_dir.join("psion_plugin_conditioned_sft_stage_receipt.json"),
        &stage_receipt,
    )?;
    write_json(
        fixtures_dir.join("psion_plugin_conditioned_sft_run_bundle.json"),
        &run_bundle,
    )?;

    Ok(())
}

fn plugin_conditioned_stage_program(
    dataset_bundle: &psionic_data::PsionPluginConditionedDatasetBundle,
) -> Result<TrainingStageProgramState, Box<dyn Error>> {
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
        .with_long_context_lineage(TrainingLongContextTraceLineage::new(
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
    dataset_bundle: &psionic_data::PsionPluginConditionedDatasetBundle,
) -> Result<Vec<PsionPluginConditionedTraceBinding>, Box<dyn Error>> {
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
                    .collect::<std::collections::BTreeSet<_>>()
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

fn eval_hooks(
    benchmark_bindings: &[psionic_train::PsionPluginConditionedBenchmarkBinding],
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

fn training_trace(record: &psionic_data::PsionPluginTrainingRecord) -> TrainingSftTraceArtifact {
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

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| String::from("failed to resolve workspace root"))?
        .to_path_buf();
    Ok(root)
}
