use std::{error::Error, fs, path::PathBuf};

use psionic_data::{PsionExclusionManifest, PsionSourceLifecycleManifest};
use psionic_environments::EnvironmentPackageKey;
use psionic_eval::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
};
use psionic_train::{
    record_psion_benchmark_catalog, record_psion_benchmark_contamination_inputs,
    record_psion_benchmark_package, record_psion_benchmark_package_receipt,
    record_psion_benchmark_receipt_set, PsionArchitectureReasoningProbeKind,
    PsionBenchmarkExactLabelGrader, PsionBenchmarkExactRefusalGrader,
    PsionBenchmarkExactRouteGrader, PsionBenchmarkExpectedResponseFormat,
    PsionBenchmarkGraderInterface, PsionBenchmarkItem, PsionBenchmarkPackageFamily,
    PsionBenchmarkPromptEnvelope, PsionBenchmarkPromptFormat, PsionBenchmarkRubricDimension,
    PsionBenchmarkRubricGrader, PsionBenchmarkTaskContract,
    PsionEngineeringSpecInterpretationProbeKind, PsionMemorizationVersusReasoningProbeKind,
    PsionMetricKind, PsionNormativeSpecReadingProbeKind, PsionObservedMetric, PsionPhaseGate,
    PsionRefusalProbeKind, PsionRouteClass,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/benchmarks");
    fs::create_dir_all(&fixtures_dir)?;

    let lifecycle: PsionSourceLifecycleManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json"),
    )?)?;
    let exclusion: PsionExclusionManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/isolation/psion_exclusion_manifest_v1.json"),
    )?)?;

    let packages = package_contracts()?;
    let catalog = record_psion_benchmark_catalog(
        "psion-benchmark-catalog-v1",
        &lifecycle,
        &exclusion,
        packages.clone(),
        "Canonical catalog proving the main Psion benchmark families can all build on one shared prompt, item, grader, contamination-input, and receipt contract.",
    )?;
    let receipts = vec![
        record_psion_benchmark_package_receipt(
            "psion-architecture-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            &packages[0],
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::PassRateBps,
                observed_bps: 8420,
                regression_from_baseline_bps: 0,
            }],
            "Architecture reasoning benchmark receipt on the shared contract.",
        )?,
        record_psion_benchmark_package_receipt(
            "psion-normative-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            &packages[1],
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::PassRateBps,
                observed_bps: 8910,
                regression_from_baseline_bps: 0,
            }],
            "Normative spec reading benchmark receipt on the shared contract.",
        )?,
        record_psion_benchmark_package_receipt(
            "psion-engineering-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            &packages[2],
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::PassRateBps,
                observed_bps: 8760,
                regression_from_baseline_bps: 0,
            }],
            "Engineering spec interpretation benchmark receipt on the shared contract.",
        )?,
        record_psion_benchmark_package_receipt(
            "psion-memorization-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            &packages[3],
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::PassRateBps,
                observed_bps: 8040,
                regression_from_baseline_bps: 0,
            }],
            "Memorization-versus-reasoning benchmark receipt on the shared contract.",
        )?,
        record_psion_benchmark_package_receipt(
            "psion-route-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            &packages[4],
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::RouteSelectionAccuracyBps,
                observed_bps: 9730,
                regression_from_baseline_bps: 0,
            }],
            "Route benchmark receipt on the shared contract.",
        )?,
        record_psion_benchmark_package_receipt(
            "psion-unsupported-request-refusal-benchmark-receipt-v1",
            PsionPhaseGate::Pilot,
            &packages[5],
            vec![
                PsionObservedMetric {
                    metric_kind: PsionMetricKind::UnsupportedRequestRefusalBps,
                    observed_bps: 9910,
                    regression_from_baseline_bps: 0,
                },
                PsionObservedMetric {
                    metric_kind: PsionMetricKind::OverrefusalBps,
                    observed_bps: 340,
                    regression_from_baseline_bps: 0,
                },
            ],
            "Unsupported-request refusal benchmark receipt on the shared contract.",
        )?,
    ];
    let receipt_set = record_psion_benchmark_receipt_set(
        "psion-benchmark-receipt-set-v1",
        &catalog,
        &lifecycle,
        &exclusion,
        receipts,
        "Canonical receipt set proving the shared Psion benchmark contract emits acceptance-ready package receipts across the main benchmark families.",
    )?;

    fs::write(
        fixtures_dir.join("psion_benchmark_catalog_v1.json"),
        serde_json::to_vec_pretty(&catalog)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_benchmark_receipt_set_v1.json"),
        serde_json::to_vec_pretty(&receipt_set)?,
    )?;
    Ok(())
}

fn package_contracts() -> Result<Vec<psionic_train::PsionBenchmarkPackageContract>, Box<dyn Error>>
{
    Ok(vec![
        record_psion_benchmark_package(
            "psion_architecture_reasoning_benchmark_v1",
            PsionBenchmarkPackageFamily::ArchitectureReasoning,
            benchmark_package(
                "psion_architecture_reasoning_benchmark_v1",
                &[
                    "arch-case-constraint",
                    "arch-case-bottleneck",
                    "arch-case-scheduling",
                    "arch-case-tradeoff",
                ],
            ),
            vec![explanation_prompt_format()],
            vec![rubric_grader()],
            contamination_inputs(&["spec_quiz_eval_pack_v1"])?,
            vec![
                PsionBenchmarkItem {
                    item_id: String::from("arch-case-constraint"),
                    family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("arch-prompt-digest-1"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                        target_architecture: String::from("bounded_three_stage_pipeline"),
                        workload_ref: String::from("workload://psion/ingest/cache-sensitive"),
                        probe_kind: PsionArchitectureReasoningProbeKind::DominantConstraint,
                        dominant_constraint: String::from("memory bandwidth"),
                        explicit_assumptions_required: true,
                        expected_focus: String::from("dominant resource constraint"),
                    },
                    detail: String::from(
                        "Architecture constraint item requires explicit assumptions about the bounded ingest workload before naming the dominant constraint.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("arch-case-bottleneck"),
                    family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("arch-prompt-digest-2"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                        target_architecture: String::from("bounded_three_stage_pipeline"),
                        workload_ref: String::from("workload://psion/ingest/high-fanout"),
                        probe_kind: PsionArchitectureReasoningProbeKind::Bottleneck,
                        dominant_constraint: String::from("queueing pressure"),
                        explicit_assumptions_required: true,
                        expected_focus: String::from("steady-state bottleneck"),
                    },
                    detail: String::from(
                        "Architecture bottleneck item requires the answer to surface the limiting stage under the stated assumptions instead of generic optimization advice.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("arch-case-scheduling"),
                    family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("arch-prompt-digest-3"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                        target_architecture: String::from("bounded_three_stage_pipeline"),
                        workload_ref: String::from("workload://psion/ingest/bursty-latency"),
                        probe_kind: PsionArchitectureReasoningProbeKind::SchedulingBehavior,
                        dominant_constraint: String::from("tail-latency budget"),
                        explicit_assumptions_required: true,
                        expected_focus: String::from("scheduler behavior under burst load"),
                    },
                    detail: String::from(
                        "Architecture scheduling item checks whether the answer reasons about bounded scheduling behavior rather than only naming hardware resources.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("arch-case-tradeoff"),
                    family: PsionBenchmarkPackageFamily::ArchitectureReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("arch-prompt-digest-4"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::ArchitectureReasoning {
                        target_architecture: String::from("bounded_three_stage_pipeline"),
                        workload_ref: String::from("workload://psion/ingest/cost-capped"),
                        probe_kind: PsionArchitectureReasoningProbeKind::TradeoffAnalysis,
                        dominant_constraint: String::from("cost ceiling"),
                        explicit_assumptions_required: true,
                        expected_focus: String::from("throughput versus resilience tradeoff"),
                    },
                    detail: String::from(
                        "Architecture tradeoff item checks whether the answer keeps the stated assumptions explicit while reasoning about bounded tradeoffs.",
                    ),
                },
            ],
            "Architecture reasoning benchmark package uses typed items that explicitly cover dominant constraints, bottlenecks, scheduling behavior, and tradeoff analysis under stated assumptions.",
        )?,
        record_psion_benchmark_package(
            "psion_normative_spec_benchmark_v1",
            PsionBenchmarkPackageFamily::NormativeSpecReading,
            benchmark_package(
                "psion_normative_spec_benchmark_v1",
                &[
                    "spec-case-definition",
                    "spec-case-edge-condition",
                    "spec-case-guarantee",
                ],
            ),
            vec![explanation_prompt_format()],
            vec![exact_label_grader()],
            contamination_inputs(&["spec_quiz_eval_pack_v1", "wasm_core_spec_release_2"])?,
            vec![
                PsionBenchmarkItem {
                    item_id: String::from("spec-case-definition"),
                    family: PsionBenchmarkPackageFamily::NormativeSpecReading,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("spec-prompt-digest-1"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::NormativeSpecReading {
                        normative_source_ref: String::from("wasm://core/validation"),
                        required_section_anchor: String::from("2.5.1"),
                        probe_kind: PsionNormativeSpecReadingProbeKind::ExactDefinition,
                        expected_fact: String::from("definition of validation context"),
                        grounded_reading_required: true,
                        engineering_inference_forbidden: true,
                    },
                    detail: String::from(
                        "Normative definition item checks exact source-grounded reading of the named term without allowing later engineering inference to stand in for the quoted fact.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("spec-case-edge-condition"),
                    family: PsionBenchmarkPackageFamily::NormativeSpecReading,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("spec-prompt-digest-2"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::NormativeSpecReading {
                        normative_source_ref: String::from("wasm://core/validation"),
                        required_section_anchor: String::from("2.5.4"),
                        probe_kind: PsionNormativeSpecReadingProbeKind::NamedEdgeCondition,
                        expected_fact: String::from("named malformed-module condition"),
                        grounded_reading_required: true,
                        engineering_inference_forbidden: true,
                    },
                    detail: String::from(
                        "Normative edge-condition item checks whether the answer names the specific edge case stated in the text rather than smoothing it into generic advice.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("spec-case-guarantee"),
                    family: PsionBenchmarkPackageFamily::NormativeSpecReading,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("spec-prompt-digest-3"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::NormativeSpecReading {
                        normative_source_ref: String::from("wasm://core/execution"),
                        required_section_anchor: String::from("4.2.15"),
                        probe_kind: PsionNormativeSpecReadingProbeKind::NamedGuarantee,
                        expected_fact: String::from("named execution guarantee"),
                        grounded_reading_required: true,
                        engineering_inference_forbidden: true,
                    },
                    detail: String::from(
                        "Normative guarantee item checks whether the answer states what the spec actually guarantees without adding portability claims that the text does not name.",
                    ),
                },
            ],
            "Normative spec package uses typed exact-label items that separate exact definitions and named edge conditions from any later engineering inference.",
        )?,
        record_psion_benchmark_package(
            "psion_engineering_spec_benchmark_v1",
            PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
            benchmark_package(
                "psion_engineering_spec_benchmark_v1",
                &[
                    "eng-case-implication",
                    "eng-case-ambiguity",
                    "eng-case-unspecified",
                    "eng-case-portability",
                ],
            ),
            vec![explanation_prompt_format()],
            vec![rubric_grader(), exact_label_grader()],
            contamination_inputs(&["spec_quiz_eval_pack_v1", "wasm_core_spec_release_2"])?,
            vec![
                PsionBenchmarkItem {
                    item_id: String::from("eng-case-implication"),
                    family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("eng-prompt-digest-implication"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                        normative_source_ref: String::from("wasm://core/execution"),
                        required_section_anchor: String::from("4.4.9"),
                        probe_kind:
                            PsionEngineeringSpecInterpretationProbeKind::ImplementationImplication,
                        implementation_target: String::from("single-threaded runtime scheduler"),
                        expected_consequence: String::from(
                            "trap propagation ordering must remain visible to the runtime",
                        ),
                        normative_boundary_required: true,
                        explicit_uncertainty_required: true,
                        unsupported_certainty_forbidden: true,
                    },
                    detail: String::from(
                        "Engineering implication item checks whether the answer separates what the execution text guarantees from the scheduling implication an embedding should preserve.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("eng-case-ambiguity"),
                    family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("eng-prompt-digest-ambiguity"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                        normative_source_ref: String::from("wasm://core/validation"),
                        required_section_anchor: String::from("2.5.7"),
                        probe_kind: PsionEngineeringSpecInterpretationProbeKind::AmbiguityRisk,
                        implementation_target: String::from("module loader import matcher"),
                        expected_consequence: String::from(
                            "ambiguous import matching must be surfaced as a compatibility risk",
                        ),
                        normative_boundary_required: true,
                        explicit_uncertainty_required: true,
                        unsupported_certainty_forbidden: true,
                    },
                    detail: String::from(
                        "Engineering ambiguity item checks whether the answer admits where the normative text leaves matching behavior open instead of projecting one implementation as universal.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("eng-case-unspecified"),
                    family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("eng-prompt-digest-unspecified"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                        normative_source_ref: String::from("wasm://core/memory"),
                        required_section_anchor: String::from("4.3.6"),
                        probe_kind:
                            PsionEngineeringSpecInterpretationProbeKind::UnspecifiedRegion,
                        implementation_target: String::from("embedding memory-growth policy"),
                        expected_consequence: String::from(
                            "the answer must name the policy choice as implementation-defined rather than spec-mandated",
                        ),
                        normative_boundary_required: true,
                        explicit_uncertainty_required: true,
                        unsupported_certainty_forbidden: true,
                    },
                    detail: String::from(
                        "Engineering unspecified-region item checks deterministic recognition that the embedding policy remains outside the normative text.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("eng-case-portability"),
                    family: PsionBenchmarkPackageFamily::EngineeringSpecInterpretation,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("rubric_reasoning_v1"),
                    prompt_digest: String::from("eng-prompt-digest-portability"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::EngineeringSpecInterpretation {
                        normative_source_ref: String::from("wasm://core/modules"),
                        required_section_anchor: String::from("2.5.11"),
                        probe_kind:
                            PsionEngineeringSpecInterpretationProbeKind::PortabilityConsequence,
                        implementation_target: String::from("cross-engine module artifact cache"),
                        expected_consequence: String::from(
                            "the answer must identify portability risk when engine-specific assumptions outrun the spec",
                        ),
                        normative_boundary_required: true,
                        explicit_uncertainty_required: true,
                        unsupported_certainty_forbidden: true,
                    },
                    detail: String::from(
                        "Engineering portability item checks whether the answer names cross-engine compatibility risk without claiming the spec guarantees more than it states.",
                    ),
                },
            ],
            "Engineering spec package uses typed items that keep normative anchors explicit while testing implementation implications, ambiguity risks, unspecified regions, and portability consequences.",
        )?,
        record_psion_benchmark_package(
            "psion_memorization_reasoning_benchmark_v1",
            PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
            benchmark_package(
                "psion_memorization_reasoning_benchmark_v1",
                &[
                    "mem-case-altered-constraints",
                    "mem-case-unfamiliar-synthesis",
                    "mem-case-historical-transfer",
                    "mem-case-paraphrase",
                    "mem-case-spec-edge",
                ],
            ),
            vec![explanation_prompt_format()],
            vec![exact_label_grader()],
            contamination_inputs(&["spec_quiz_eval_pack_v1", "wasm_core_spec_release_2"])?,
            vec![
                PsionBenchmarkItem {
                    item_id: String::from("mem-case-altered-constraints"),
                    family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("mem-prompt-digest-altered"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                        seed_fact_ref: String::from(
                            "seed://psion/memorization/cache-hierarchy-baseline",
                        ),
                        perturbation_ref: String::from(
                            "perturbation://psion/memorization/cache-latency-budget-tightened",
                        ),
                        probe_kind:
                            PsionMemorizationVersusReasoningProbeKind::AlteredConstraintRecombination,
                        expected_transfer: String::from(
                            "adapt the pipeline recommendation under a tightened latency budget instead of replaying the baseline answer",
                        ),
                        recombination_required: true,
                        surface_form_shift_required: true,
                        verbatim_recall_forbidden: true,
                    },
                    detail: String::from(
                        "Altered-constraint probe checks whether the answer recombines the learned architecture ontology after one key resource bound is changed.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("mem-case-unfamiliar-synthesis"),
                    family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("mem-prompt-digest-synthesis"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                        seed_fact_ref: String::from(
                            "seed://psion/memorization/queueing-and-backpressure",
                        ),
                        perturbation_ref: String::from(
                            "perturbation://psion/memorization/queueing-plus-capability-routing",
                        ),
                        probe_kind:
                            PsionMemorizationVersusReasoningProbeKind::UnfamiliarDesignSynthesis,
                        expected_transfer: String::from(
                            "combine queueing and capability-routing ideas for an unfamiliar system composition",
                        ),
                        recombination_required: true,
                        surface_form_shift_required: true,
                        verbatim_recall_forbidden: true,
                    },
                    detail: String::from(
                        "Unfamiliar-synthesis probe checks whether the answer composes known mechanisms in a new configuration rather than reciting one familiar design.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("mem-case-historical-transfer"),
                    family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("mem-prompt-digest-historical"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                        seed_fact_ref: String::from(
                            "seed://psion/memorization/vector-era-scheduling",
                        ),
                        perturbation_ref: String::from(
                            "perturbation://psion/memorization/gpu-batch-scheduler-analogy",
                        ),
                        probe_kind:
                            PsionMemorizationVersusReasoningProbeKind::HistoricalAnalogyTransfer,
                        expected_transfer: String::from(
                            "transfer a bounded historical scheduling analogy without claiming the old and new systems are identical",
                        ),
                        recombination_required: true,
                        surface_form_shift_required: true,
                        verbatim_recall_forbidden: true,
                    },
                    detail: String::from(
                        "Historical-transfer probe checks whether the answer uses historical systems knowledge as an analogy instead of as a canned template.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("mem-case-paraphrase"),
                    family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("mem-prompt-digest-paraphrase"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                        seed_fact_ref: String::from(
                            "seed://psion/memorization/wasm-validation-definition",
                        ),
                        perturbation_ref: String::from(
                            "perturbation://psion/memorization/wasm-validation-paraphrase-variant",
                        ),
                        probe_kind:
                            PsionMemorizationVersusReasoningProbeKind::ParaphraseVariation,
                        expected_transfer: String::from(
                            "answer the paraphrased specification prompt without depending on the memorized stock wording",
                        ),
                        recombination_required: true,
                        surface_form_shift_required: true,
                        verbatim_recall_forbidden: true,
                    },
                    detail: String::from(
                        "Paraphrase-variation probe checks whether the answer survives surface-form changes around one familiar technical fact.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("mem-case-spec-edge"),
                    family: PsionBenchmarkPackageFamily::MemorizationVersusReasoning,
                    prompt_format_id: String::from("bounded_explanation_v1"),
                    grader_id: String::from("exact_label_v1"),
                    prompt_digest: String::from("mem-prompt-digest-edge"),
                    source_ids: vec![
                        String::from("spec_quiz_eval_pack_v1"),
                        String::from("wasm_core_spec_release_2"),
                    ],
                    task: PsionBenchmarkTaskContract::MemorizationVersusReasoning {
                        seed_fact_ref: String::from(
                            "seed://psion/memorization/module-linking-baseline",
                        ),
                        perturbation_ref: String::from(
                            "perturbation://psion/memorization/spec-adjacent-edge-case",
                        ),
                        probe_kind:
                            PsionMemorizationVersusReasoningProbeKind::SpecAdjacentEdgeCase,
                        expected_transfer: String::from(
                            "apply the learned specification ontology to a nearby edge case that is not quoted verbatim in the seed material",
                        ),
                        recombination_required: true,
                        surface_form_shift_required: true,
                        verbatim_recall_forbidden: true,
                    },
                    detail: String::from(
                        "Spec-adjacent edge-case probe checks whether the answer can reason next to the source text instead of only replaying the exact benchmark passage.",
                    ),
                },
            ],
            "Memorization-versus-reasoning package uses typed exact-label probes that force altered constraints, unfamiliar synthesis, historical transfer, paraphrase variation, and spec-adjacent edge cases.",
        )?,
        record_psion_benchmark_package(
            "psion_route_benchmark_v1",
            PsionBenchmarkPackageFamily::RouteEvaluation,
            benchmark_package(
                "psion_route_benchmark_v1",
                &[
                    "route-case-answer",
                    "route-case-uncertainty",
                    "route-case-structure",
                    "route-case-delegate",
                ],
            ),
            vec![route_prompt_format()],
            vec![
                exact_route_grader("route_answer_v1", PsionRouteClass::AnswerInLanguage),
                exact_route_grader(
                    "route_uncertainty_v1",
                    PsionRouteClass::AnswerWithUncertainty,
                ),
                exact_route_grader(
                    "route_structure_v1",
                    PsionRouteClass::RequestStructuredInputs,
                ),
                exact_route_grader(
                    "route_delegate_v1",
                    PsionRouteClass::DelegateToExactExecutor,
                ),
            ],
            contamination_inputs(&["spec_quiz_eval_pack_v1"])?,
            vec![
                PsionBenchmarkItem {
                    item_id: String::from("route-case-answer"),
                    family: PsionBenchmarkPackageFamily::RouteEvaluation,
                    prompt_format_id: String::from("route_decision_v1"),
                    grader_id: String::from("route_answer_v1"),
                    prompt_digest: String::from("route-prompt-digest-answer"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RouteEvaluation {
                        route_class: PsionRouteClass::AnswerInLanguage,
                        route_boundary_ref: String::from(
                            "route://psion/language_judgment_boundary",
                        ),
                        required_signal: String::from("answer directly in language"),
                        structured_input_schema_ref: None,
                        uncertainty_required: false,
                    },
                    detail: String::from(
                        "Route answer item checks the bounded direct-answer class without forcing delegation or a structured-input ask.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("route-case-uncertainty"),
                    family: PsionBenchmarkPackageFamily::RouteEvaluation,
                    prompt_format_id: String::from("route_decision_v1"),
                    grader_id: String::from("route_uncertainty_v1"),
                    prompt_digest: String::from("route-prompt-digest-uncertainty"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RouteEvaluation {
                        route_class: PsionRouteClass::AnswerWithUncertainty,
                        route_boundary_ref: String::from("route://psion/uncertainty_boundary"),
                        required_signal: String::from("answer with explicit uncertainty"),
                        structured_input_schema_ref: None,
                        uncertainty_required: true,
                    },
                    detail: String::from(
                        "Route uncertainty item checks whether the learned lane can stay in-language while making uncertainty explicit.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("route-case-structure"),
                    family: PsionBenchmarkPackageFamily::RouteEvaluation,
                    prompt_format_id: String::from("route_decision_v1"),
                    grader_id: String::from("route_structure_v1"),
                    prompt_digest: String::from("route-prompt-digest-structure"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RouteEvaluation {
                        route_class: PsionRouteClass::RequestStructuredInputs,
                        route_boundary_ref: String::from(
                            "route://psion/structured-input-boundary",
                        ),
                        required_signal: String::from(
                            "request the missing structured inputs before answering",
                        ),
                        structured_input_schema_ref: Some(String::from(
                            "schema://psion/route/design_inputs_v1",
                        )),
                        uncertainty_required: false,
                    },
                    detail: String::from(
                        "Route structure item checks whether the learned lane asks for the named structured fields instead of guessing them.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("route-case-delegate"),
                    family: PsionBenchmarkPackageFamily::RouteEvaluation,
                    prompt_format_id: String::from("route_decision_v1"),
                    grader_id: String::from("route_delegate_v1"),
                    prompt_digest: String::from("route-prompt-digest-delegate"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RouteEvaluation {
                        route_class: PsionRouteClass::DelegateToExactExecutor,
                        route_boundary_ref: String::from("route://psion/exactness_boundary"),
                        required_signal: String::from(
                            "delegate to the exact executor instead of improvising",
                        ),
                        structured_input_schema_ref: None,
                        uncertainty_required: false,
                    },
                    detail: String::from(
                        "Route delegate item checks the bounded exact-executor handoff class without over-reading delegation as learned execution.",
                    ),
                },
            ],
            "Route package uses the shared structured route prompt and exact route-class graders across answer, uncertainty, structure-request, and exact-delegation paths.",
        )?,
        record_psion_benchmark_package(
            "psion_unsupported_request_refusal_benchmark_v1",
            PsionBenchmarkPackageFamily::RefusalEvaluation,
            benchmark_package(
                "psion_unsupported_request_refusal_benchmark_v1",
                &[
                    "refusal-case-exactness",
                    "refusal-case-missing-constraints",
                    "refusal-case-over-context",
                    "refusal-case-freshness",
                    "refusal-case-open-ended",
                ],
            ),
            vec![refusal_prompt_format()],
            vec![
                exact_refusal_grader(
                    "exact_refusal_exactness_v1",
                    "unsupported_exactness_request",
                ),
                exact_refusal_grader(
                    "exact_refusal_missing_constraints_v1",
                    "missing_required_constraints",
                ),
                exact_refusal_grader(
                    "exact_refusal_context_v1",
                    "unsupported_context_length",
                ),
                exact_refusal_grader(
                    "exact_refusal_freshness_v1",
                    "currentness_or_run_artifact_dependency",
                ),
                exact_refusal_grader(
                    "exact_refusal_open_ended_v1",
                    "open_ended_general_assistant_unsupported",
                ),
            ],
            contamination_inputs(&["spec_quiz_eval_pack_v1"])?,
            vec![
                PsionBenchmarkItem {
                    item_id: String::from("refusal-case-exactness"),
                    family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                    prompt_format_id: String::from("refusal_decision_v1"),
                    grader_id: String::from("exact_refusal_exactness_v1"),
                    prompt_digest: String::from("refusal-prompt-digest-exactness"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RefusalEvaluation {
                        expected_reason_code: String::from("unsupported_exactness_request"),
                        refusal_boundary_ref: String::from(
                            "route://psion/refusal/exactness-without-executor",
                        ),
                        probe_kind: PsionRefusalProbeKind::UnsupportedExactnessRequest,
                        capability_region_id: String::from(
                            "unsupported_exact_execution_without_executor_surface",
                        ),
                        unsupported_region_evidence_ref: String::from(
                            "evidence://psion/refusal/exactness-without-executor",
                        ),
                        claim_boundary_required: true,
                    },
                    detail: String::from(
                        "Refusal exactness item checks that exact-execution requests without a surfaced executor path refuse instead of improvising an answer.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("refusal-case-missing-constraints"),
                    family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                    prompt_format_id: String::from("refusal_decision_v1"),
                    grader_id: String::from("exact_refusal_missing_constraints_v1"),
                    prompt_digest: String::from("refusal-prompt-digest-missing-constraints"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RefusalEvaluation {
                        expected_reason_code: String::from("missing_required_constraints"),
                        refusal_boundary_ref: String::from(
                            "route://psion/refusal/missing-required-constraints",
                        ),
                        probe_kind: PsionRefusalProbeKind::MissingConstraints,
                        capability_region_id: String::from(
                            "underspecified_design_without_required_constraints",
                        ),
                        unsupported_region_evidence_ref: String::from(
                            "evidence://psion/refusal/missing-required-constraints",
                        ),
                        claim_boundary_required: true,
                    },
                    detail: String::from(
                        "Refusal missing-constraints item checks that underspecified design asks refuse instead of fabricating requirements.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("refusal-case-over-context"),
                    family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                    prompt_format_id: String::from("refusal_decision_v1"),
                    grader_id: String::from("exact_refusal_context_v1"),
                    prompt_digest: String::from("refusal-prompt-digest-over-context"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RefusalEvaluation {
                        expected_reason_code: String::from("unsupported_context_length"),
                        refusal_boundary_ref: String::from(
                            "route://psion/refusal/hard-context-boundary",
                        ),
                        probe_kind: PsionRefusalProbeKind::OverContextEnvelope,
                        capability_region_id: String::from("over_context_envelope_requests"),
                        unsupported_region_evidence_ref: String::from(
                            "evidence://psion/refusal/over-context-envelope",
                        ),
                        claim_boundary_required: true,
                    },
                    detail: String::from(
                        "Refusal over-context item checks that prompts beyond the hard context envelope refuse instead of truncating or stretching the lane silently.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("refusal-case-freshness"),
                    family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                    prompt_format_id: String::from("refusal_decision_v1"),
                    grader_id: String::from("exact_refusal_freshness_v1"),
                    prompt_digest: String::from("refusal-prompt-digest-freshness"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RefusalEvaluation {
                        expected_reason_code: String::from(
                            "currentness_or_run_artifact_dependency",
                        ),
                        refusal_boundary_ref: String::from(
                            "route://psion/refusal/currentness-or-hidden-artifact",
                        ),
                        probe_kind: PsionRefusalProbeKind::FreshnessOrHiddenArtifactDependency,
                        capability_region_id: String::from(
                            "freshness_or_run_artifact_dependent_requests",
                        ),
                        unsupported_region_evidence_ref: String::from(
                            "evidence://psion/refusal/currentness-or-hidden-artifact",
                        ),
                        claim_boundary_required: true,
                    },
                    detail: String::from(
                        "Refusal freshness item checks that currentness-sensitive or hidden-artifact asks refuse instead of pretending the lane can inspect mutable state.",
                    ),
                },
                PsionBenchmarkItem {
                    item_id: String::from("refusal-case-open-ended"),
                    family: PsionBenchmarkPackageFamily::RefusalEvaluation,
                    prompt_format_id: String::from("refusal_decision_v1"),
                    grader_id: String::from("exact_refusal_open_ended_v1"),
                    prompt_digest: String::from("refusal-prompt-digest-open-ended"),
                    source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                    task: PsionBenchmarkTaskContract::RefusalEvaluation {
                        expected_reason_code: String::from(
                            "open_ended_general_assistant_unsupported",
                        ),
                        refusal_boundary_ref: String::from(
                            "route://psion/refusal/open-ended-assistant",
                        ),
                        probe_kind: PsionRefusalProbeKind::UnsupportedGeneralAssistantChat,
                        capability_region_id: String::from("open_ended_general_assistant_chat"),
                        unsupported_region_evidence_ref: String::from(
                            "evidence://psion/refusal/open-ended-assistant",
                        ),
                        claim_boundary_required: true,
                    },
                    detail: String::from(
                        "Refusal open-ended item checks that broad assistant chat stays explicitly unsupported instead of being half-served by vibe.",
                    ),
                },
            ],
            "Refusal package uses the shared structured refusal prompt with capability-matrix-bound refusal probes for exactness, missing constraints, context overflow, freshness, and open-ended assistant asks.",
        )?,
    ])
}

fn benchmark_package(package_id: &str, case_ids: &[&str]) -> BenchmarkPackage {
    BenchmarkPackage::new(
        BenchmarkPackageKey::new(package_id, "v1"),
        format!("Display {package_id}"),
        EnvironmentPackageKey::new("env.psion.benchmark", "2026.03.22"),
        3,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_cases(
        case_ids
            .iter()
            .map(|case_id| BenchmarkCase::new(*case_id))
            .collect(),
    )
}

fn contamination_inputs(
    source_ids: &[&str],
) -> Result<psionic_train::PsionBenchmarkContaminationInputs, Box<dyn Error>> {
    Ok(record_psion_benchmark_contamination_inputs(
        source_ids.iter().map(|source| String::from(*source)).collect(),
        vec![String::from("spec_quiz_eval_pack_v1")],
        vec![String::from("spec_quiz_eval_pack_v1")],
        "review://psion/benchmark/near-duplicate-v1",
        "Benchmark package preserves the held-out source ids, training-excluded ids, and near-duplicate review reference required by the isolation contract.",
    )?)
}

fn explanation_prompt_format() -> PsionBenchmarkPromptFormat {
    PsionBenchmarkPromptFormat {
        format_id: String::from("bounded_explanation_v1"),
        system_instruction_ref: String::from("prompt://psion/benchmark/system/bounded-explanation"),
        user_template_ref: String::from("prompt://psion/benchmark/user/bounded-explanation"),
        envelope: PsionBenchmarkPromptEnvelope::CitedSectionPrompt,
        expected_response_format: PsionBenchmarkExpectedResponseFormat::BoundedExplanationJson,
        preserve_source_boundaries: true,
        detail: String::from(
            "Explanation prompts preserve source boundaries and produce bounded explanation JSON.",
        ),
    }
}

fn route_prompt_format() -> PsionBenchmarkPromptFormat {
    PsionBenchmarkPromptFormat {
        format_id: String::from("route_decision_v1"),
        system_instruction_ref: String::from("prompt://psion/benchmark/system/route"),
        user_template_ref: String::from("prompt://psion/benchmark/user/route"),
        envelope: PsionBenchmarkPromptEnvelope::StructuredRouteDecisionJson,
        expected_response_format: PsionBenchmarkExpectedResponseFormat::RouteDecisionJson,
        preserve_source_boundaries: true,
        detail: String::from("Route prompts require a structured route-decision JSON response."),
    }
}

fn refusal_prompt_format() -> PsionBenchmarkPromptFormat {
    PsionBenchmarkPromptFormat {
        format_id: String::from("refusal_decision_v1"),
        system_instruction_ref: String::from("prompt://psion/benchmark/system/refusal"),
        user_template_ref: String::from("prompt://psion/benchmark/user/refusal"),
        envelope: PsionBenchmarkPromptEnvelope::StructuredRefusalDecisionJson,
        expected_response_format: PsionBenchmarkExpectedResponseFormat::RefusalDecisionJson,
        preserve_source_boundaries: true,
        detail: String::from(
            "Refusal prompts require a structured refusal-decision JSON response.",
        ),
    }
}

fn rubric_grader() -> PsionBenchmarkGraderInterface {
    PsionBenchmarkGraderInterface::RubricScore(PsionBenchmarkRubricGrader {
        grader_id: String::from("rubric_reasoning_v1"),
        rubric_ref: String::from("rubric://psion/benchmark/reasoning"),
        minimum_pass_bps: 7800,
        dimensions: vec![
            PsionBenchmarkRubricDimension {
                dimension_id: String::from("correctness"),
                weight_bps: 6000,
                detail: String::from("Checks the substantive answer."),
            },
            PsionBenchmarkRubricDimension {
                dimension_id: String::from("truth_boundary"),
                weight_bps: 4000,
                detail: String::from(
                    "Checks explicit assumptions, uncertainty, and normative-versus-inference separation.",
                ),
            },
        ],
        detail: String::from(
            "Rubric grader supports bounded reasoning labels without collapsing them into one exact string.",
        ),
    })
}

fn exact_label_grader() -> PsionBenchmarkGraderInterface {
    PsionBenchmarkGraderInterface::ExactLabel(PsionBenchmarkExactLabelGrader {
        grader_id: String::from("exact_label_v1"),
        label_namespace: String::from("psion.spec.reading"),
        accepted_labels: vec![String::from("pass"), String::from("boundary_clear")],
        detail: String::from("Exact-label grader supports deterministic label-based grading."),
    })
}

fn exact_route_grader(
    grader_id: &str,
    expected_route: PsionRouteClass,
) -> PsionBenchmarkGraderInterface {
    PsionBenchmarkGraderInterface::ExactRoute(PsionBenchmarkExactRouteGrader {
        grader_id: String::from(grader_id),
        expected_route,
        detail: String::from("Route grader requires the declared route exactly."),
    })
}

fn exact_refusal_grader(
    grader_id: &str,
    accepted_reason_code: &str,
) -> PsionBenchmarkGraderInterface {
    PsionBenchmarkGraderInterface::ExactRefusal(PsionBenchmarkExactRefusalGrader {
        grader_id: String::from(grader_id),
        accepted_reason_codes: vec![String::from(accepted_reason_code)],
        detail: String::from("Refusal grader requires one of the admitted refusal codes exactly."),
    })
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| String::from("could not resolve workspace root").into())
}
