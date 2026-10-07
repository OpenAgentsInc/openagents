use std::{error::Error, fs, path::PathBuf};

use psionic_data::{
    PsionArtifactLineageManifest, PsionExclusionManifest, PsionSourceLifecycleManifest,
};
use psionic_environments::EnvironmentPackageKey;
use psionic_train::{
    record_psion_reasoning_sft_dataset_bundle, record_psion_reasoning_sft_evaluation_receipt,
    record_psion_reasoning_sft_run_bundle, record_psion_reasoning_sft_stage_receipt,
    PsionPretrainStageRunReceipt, PsionReasoningSftAbstractionLevel,
    PsionReasoningSftControlSurface, PsionReasoningSftDecompositionStrategy,
    PsionReasoningSftEvaluationRow, PsionReasoningSftExplanationOrder,
    PsionReasoningSftStyleProfile, PsionReasoningSftTraceBinding, TrainingLongContextTraceLineage,
    TrainingSftTraceArtifact, TrainingSftTraceKind, TrainingStageKind, TrainingStageProgramState,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/sft");
    fs::create_dir_all(&fixtures_dir)?;

    let lifecycle: PsionSourceLifecycleManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"),
    )?)?;
    let exclusion: PsionExclusionManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/isolation/psion_exclusion_manifest_v1.json"),
    )?)?;
    let artifact_lineage: PsionArtifactLineageManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json"),
        )?)?;
    let pretrain_receipt: PsionPretrainStageRunReceipt =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"),
        )?)?;

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
        TrainingStageKind::GeneralSft,
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

    let sft_artifact_lineage = artifact_lineage
        .sft_artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == "psion_reasoning_sft_seed_v1")
        .ok_or_else(|| String::from("missing canonical reasoning SFT lineage row"))?
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

    let run_bundle = record_psion_reasoning_sft_run_bundle(
        "psion-reasoning-sft-run-bundle-v1",
        &lifecycle,
        &exclusion,
        &artifact_lineage,
        stage_program,
        dataset_bundle.clone(),
        stage_receipt.clone(),
        evaluation_receipt.clone(),
        "The first bounded reasoning-SFT bundle ties the canonical pretrain receipt, general-SFT stage graph, source-lineaged derived traces, truth-control surface, and style-plurality evaluation into one repo-owned receipt bundle.",
    )?;

    fs::write(
        fixtures_dir.join("psion_reasoning_sft_dataset_bundle_v1.json"),
        serde_json::to_vec_pretty(&dataset_bundle)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_reasoning_sft_stage_receipt_v1.json"),
        serde_json::to_vec_pretty(&stage_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_reasoning_sft_evaluation_receipt_v1.json"),
        serde_json::to_vec_pretty(&evaluation_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_reasoning_sft_run_bundle_v1.json"),
        serde_json::to_vec_pretty(&run_bundle)?,
    )?;
    Ok(())
}

fn style_profiles() -> Vec<PsionReasoningSftStyleProfile> {
    vec![
        PsionReasoningSftStyleProfile {
            style_id: String::from("assumptions_then_mechanism_concrete"),
            decomposition_strategy: PsionReasoningSftDecompositionStrategy::AssumptionsThenMechanism,
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
            decomposition_strategy: PsionReasoningSftDecompositionStrategy::ConstraintsThenTradeoffs,
            explanation_order: PsionReasoningSftExplanationOrder::TopDownThenConcrete,
            abstraction_level: PsionReasoningSftAbstractionLevel::Conceptual,
            detail: String::from(
                "Frame the constraints first, then discuss tradeoffs from the conceptual level down.",
            ),
        },
    ]
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| String::from("could not resolve workspace root").into())
}
