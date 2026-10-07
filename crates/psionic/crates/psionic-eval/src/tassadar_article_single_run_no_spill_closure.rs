use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{TassadarArticleRuntimeFloorStatus, TassadarExecutorSelectionState};

use crate::{
    build_tassadar_article_demo_benchmark_equivalence_gate_report,
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_fast_route_throughput_floor_report,
    build_tassadar_article_runtime_closeout_report, build_tassadar_dynamic_memory_resume_report,
    build_tassadar_effect_safe_resume_report, build_tassadar_execution_checkpoint_report,
    build_tassadar_spill_tape_store_report, TassadarArticleDemoBenchmarkEquivalenceGateReport,
    TassadarArticleDemoBenchmarkEquivalenceGateReportError,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFastRouteThroughputFloorReport,
    TassadarArticleFastRouteThroughputFloorReportError, TassadarArticleRuntimeCloseoutReport,
    TassadarArticleRuntimeCloseoutReportError, TassadarDynamicMemoryResumeReport,
    TassadarDynamicMemoryResumeReportError, TassadarEffectSafeResumeReport,
    TassadarEffectSafeResumeReportError, TassadarExecutionCheckpointReport,
    TassadarExecutionCheckpointReportError, TassadarSpillTapeStoreReport,
    TassadarSpillTapeStoreReportError, TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
    TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF, TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF,
    TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF, TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
    TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_single_run_no_spill_closure_report.json";
pub const TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-single-run-no-spill-closure.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-183";
const BENCHMARK_REQUIREMENT_ID: &str = "TAS-182";
const MODEL_DESCRIPTOR_REF: &str =
    "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_descriptor.json";
const EXPECTED_MODEL_ID: &str = "tassadar-article-transformer-trace-bound-trained-v0";
const EXPECTED_SELECTED_CANDIDATE_KIND: &str = "hull_cache_runtime";
const EXPECTED_SELECTED_DECODE_MODE: &str = "tassadar.decode.hull_cache.v1";
const EXPECTED_EXECUTION_CHECKPOINT_FAMILY_ID: &str = "tassadar.execution_checkpoint.v1";
const EXPECTED_SPILL_PROFILE_ID: &str = "tassadar.internal_compute.spill_tape_store.v1";
const EXPECTED_EXTERNAL_TAPE_STORE_FAMILY_ID: &str = "tassadar.external_tape_store.v1";
const EXPECTED_EFFECT_SAFE_RESUME_PROFILE_ID: &str =
    "tassadar.internal_compute.deterministic_import_subset.v1";
const EXPECTED_DYNAMIC_MEMORY_RESUME_FAMILY_ID: &str = "tassadar.dynamic_memory_resume.v1";
const TOKEN_STREAM_CONTINUITY_CONTRACT_ID: &str =
    "tassadar.article_single_run.token_stream_continuity.v1";
const STEP_TO_COMPUTE_CONTRACT_ID: &str =
    "tassadar.article_single_run.step_to_compute_consistency.v1";
const OWNED_ROUTE_BOUNDARY_REFS: [&str; 2] = [
    "crates/psionic-transformer/Cargo.toml",
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md",
];
const ALLOWED_SCHEDULING_BEHAVIOR_IDS: [&str; 3] = [
    "deterministic_reference_linear_anchor",
    "direct_hull_cache_fast_route",
    "bounded_kernel_horizon_exactness",
];
const DISALLOWED_SCHEDULING_BEHAVIOR_IDS: [&str; 10] = [
    "checkpoint_restore",
    "spill_tape_extension",
    "external_persisted_continuation",
    "hidden_reentry",
    "implicit_segmentation",
    "runtime_loop_unrolling",
    "teacher_forcing",
    "oracle_leakage",
    "retry_farming",
    "oversized_context_memory",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureBenchmarkPrerequisite {
    pub report_ref: String,
    pub report_id: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: Option<String>,
    pub hungarian_demo_parity_green: bool,
    pub named_arto_parity_green: bool,
    pub benchmark_wide_sudoku_parity_green: bool,
    pub binding_green: bool,
    pub article_demo_benchmark_equivalence_gate_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureOperatorEnvelope {
    pub token_stream_continuity_contract_id: String,
    pub step_to_compute_contract_id: String,
    pub allowed_scheduling_behavior_ids: Vec<String>,
    pub disallowed_scheduling_behavior_ids: Vec<String>,
    pub checkpoint_restore_allowed: bool,
    pub spill_tape_extension_allowed: bool,
    pub external_persisted_continuation_allowed: bool,
    pub hidden_reentry_allowed: bool,
    pub implicit_segmentation_allowed: bool,
    pub runtime_loop_unrolling_allowed: bool,
    pub teacher_forcing_allowed: bool,
    pub oracle_leakage_allowed: bool,
    pub oversized_context_memory_allowed: bool,
    pub deterministic_mode_required: bool,
    pub stochastic_mode_policy: String,
    pub operator_envelope_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillHorizonRow {
    pub horizon_id: String,
    pub exact_step_count: u64,
    pub reference_linear_direct: bool,
    pub hull_cache_direct: bool,
    pub exactness_bps: u32,
    pub hull_cache_exactness_bps: u32,
    pub throughput_floor_passed: bool,
    pub step_count_alignment_green: bool,
    pub trace_digest_alignment_green: bool,
    pub behavior_digest_alignment_green: bool,
    pub terminal_state_alignment_green: bool,
    pub context_to_horizon_ratio_bps: u16,
    pub context_within_limit: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureHorizonReview {
    pub runtime_closeout_report_ref: String,
    pub throughput_floor_report_ref: String,
    pub selected_candidate_kind: String,
    pub selected_decode_mode: String,
    pub horizon_rows: Vec<TassadarArticleSingleRunNoSpillHorizonRow>,
    pub million_step_horizon_ids: Vec<String>,
    pub multi_million_step_horizon_ids: Vec<String>,
    pub total_exact_step_count: u64,
    pub deterministic_exactness_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureStepConsistencyReview {
    pub exact_horizon_count: u32,
    pub step_count_alignment_green: bool,
    pub trace_digest_alignment_green: bool,
    pub behavior_digest_alignment_green: bool,
    pub terminal_state_alignment_green: bool,
    pub consistency_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureContextSensitivityReview {
    pub model_descriptor_ref: String,
    pub model_id: String,
    pub max_source_positions: u32,
    pub max_target_positions: u32,
    pub allowed_max_context_to_horizon_ratio_bps: u16,
    pub worst_horizon_id: String,
    pub worst_context_to_horizon_ratio_bps: u16,
    pub context_sensitivity_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview {
    pub execution_checkpoint_report_ref: String,
    pub spill_tape_store_report_ref: String,
    pub effect_safe_resume_report_ref: String,
    pub dynamic_memory_resume_report_ref: String,
    pub checkpoint_family_id: String,
    pub spill_profile_id: String,
    pub external_tape_store_family_id: String,
    pub effect_safe_resume_target_profile_id: String,
    pub dynamic_memory_resume_family_id: String,
    pub checkpoint_resume_marker_green: bool,
    pub spill_tape_marker_green: bool,
    pub external_continuation_marker_green: bool,
    pub dynamic_memory_resume_marker_green: bool,
    pub perturbation_negative_control_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureStochasticModeReview {
    pub stochastic_mode_supported: bool,
    pub retry_farming_admitted: bool,
    pub lucky_sampling_admitted: bool,
    pub teacher_forcing_admitted: bool,
    pub oracle_leakage_admitted: bool,
    pub stochastic_mode_robustness_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureBindingReview {
    pub owned_route_boundary_refs: Vec<String>,
    pub owned_route_boundary_refs_exist: bool,
    pub model_descriptor_ref: String,
    pub model_id: String,
    pub model_id_matches_expected: bool,
    pub benchmark_prerequisite_green: bool,
    pub selected_fast_route_green: bool,
    pub operator_envelope_green: bool,
    pub deterministic_exactness_green: bool,
    pub step_consistency_green: bool,
    pub context_sensitivity_green: bool,
    pub perturbation_negative_control_green: bool,
    pub stochastic_mode_robustness_green: bool,
    pub binding_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleSingleRunNoSpillClosureAcceptanceGateTie,
    pub benchmark_prerequisite: TassadarArticleSingleRunNoSpillClosureBenchmarkPrerequisite,
    pub operator_envelope: TassadarArticleSingleRunNoSpillClosureOperatorEnvelope,
    pub horizon_review: TassadarArticleSingleRunNoSpillClosureHorizonReview,
    pub step_consistency_review: TassadarArticleSingleRunNoSpillClosureStepConsistencyReview,
    pub context_sensitivity_review: TassadarArticleSingleRunNoSpillClosureContextSensitivityReview,
    pub boundary_perturbation_review:
        TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview,
    pub stochastic_mode_review: TassadarArticleSingleRunNoSpillClosureStochasticModeReview,
    pub binding_review: TassadarArticleSingleRunNoSpillClosureBindingReview,
    pub single_run_no_spill_closure_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NoSpillPolicy {
    checkpoint_restore_allowed: bool,
    spill_tape_extension_allowed: bool,
    external_persisted_continuation_allowed: bool,
    hidden_reentry_allowed: bool,
    implicit_segmentation_allowed: bool,
    runtime_loop_unrolling_allowed: bool,
    teacher_forcing_allowed: bool,
    oracle_leakage_allowed: bool,
    oversized_context_memory_allowed: bool,
    deterministic_mode_required: bool,
    stochastic_mode_supported: bool,
    retry_farming_admitted: bool,
    lucky_sampling_admitted: bool,
    allowed_scheduling_behavior_ids: Vec<String>,
    disallowed_scheduling_behavior_ids: Vec<String>,
    allowed_max_context_to_horizon_ratio_bps: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarArticleTransformerDescriptorView {
    model: TassadarArticleTransformerDescriptorModelView,
    config: TassadarArticleTransformerDescriptorConfigView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarArticleTransformerDescriptorModelView {
    model_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarArticleTransformerDescriptorConfigView {
    max_source_positions: u32,
    max_target_positions: u32,
}

#[derive(Debug, Error)]
pub enum TassadarArticleSingleRunNoSpillClosureReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Benchmark(#[from] TassadarArticleDemoBenchmarkEquivalenceGateReportError),
    #[error(transparent)]
    RuntimeCloseout(#[from] TassadarArticleRuntimeCloseoutReportError),
    #[error(transparent)]
    Throughput(#[from] TassadarArticleFastRouteThroughputFloorReportError),
    #[error(transparent)]
    ExecutionCheckpoint(#[from] TassadarExecutionCheckpointReportError),
    #[error(transparent)]
    SpillTape(#[from] TassadarSpillTapeStoreReportError),
    #[error(transparent)]
    EffectSafeResume(#[from] TassadarEffectSafeResumeReportError),
    #[error(transparent)]
    DynamicMemoryResume(#[from] TassadarDynamicMemoryResumeReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_single_run_no_spill_closure_report() -> Result<
    TassadarArticleSingleRunNoSpillClosureReport,
    TassadarArticleSingleRunNoSpillClosureReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let benchmark_prerequisite = build_tassadar_article_demo_benchmark_equivalence_gate_report()?;
    let runtime_closeout = build_tassadar_article_runtime_closeout_report()?;
    let throughput_floor = build_tassadar_article_fast_route_throughput_floor_report()?;
    let execution_checkpoint = build_tassadar_execution_checkpoint_report()?;
    let spill_tape_store = build_tassadar_spill_tape_store_report()?;
    let effect_safe_resume = build_tassadar_effect_safe_resume_report()?;
    let dynamic_memory_resume = build_tassadar_dynamic_memory_resume_report()?;
    let descriptor: TassadarArticleTransformerDescriptorView = read_repo_json(
        MODEL_DESCRIPTOR_REF,
        "tassadar_article_transformer_trace_bound_trained_descriptor",
    )?;

    Ok(build_report_from_inputs(
        acceptance_gate,
        benchmark_prerequisite,
        runtime_closeout,
        throughput_floor,
        execution_checkpoint,
        spill_tape_store,
        effect_safe_resume,
        dynamic_memory_resume,
        descriptor,
        &default_no_spill_policy(),
    ))
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    benchmark_prerequisite: TassadarArticleDemoBenchmarkEquivalenceGateReport,
    runtime_closeout: TassadarArticleRuntimeCloseoutReport,
    throughput_floor: TassadarArticleFastRouteThroughputFloorReport,
    execution_checkpoint: TassadarExecutionCheckpointReport,
    spill_tape_store: TassadarSpillTapeStoreReport,
    effect_safe_resume: TassadarEffectSafeResumeReport,
    dynamic_memory_resume: TassadarDynamicMemoryResumeReport,
    descriptor: TassadarArticleTransformerDescriptorView,
    policy: &NoSpillPolicy,
) -> TassadarArticleSingleRunNoSpillClosureReport {
    let acceptance_gate_tie = TassadarArticleSingleRunNoSpillClosureAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    };
    let benchmark_prerequisite = build_benchmark_prerequisite(&benchmark_prerequisite);
    let operator_envelope = build_operator_envelope(policy);
    let horizon_review = build_horizon_review(&runtime_closeout, &throughput_floor, &descriptor);
    let step_consistency_review = build_step_consistency_review(&horizon_review);
    let context_sensitivity_review =
        build_context_sensitivity_review(&horizon_review, &descriptor, policy);
    let boundary_perturbation_review = build_boundary_perturbation_review(
        &execution_checkpoint,
        &spill_tape_store,
        &effect_safe_resume,
        &dynamic_memory_resume,
    );
    let stochastic_mode_review = build_stochastic_mode_review(policy);
    let binding_review = build_binding_review(
        &benchmark_prerequisite,
        &operator_envelope,
        &horizon_review,
        &step_consistency_review,
        &context_sensitivity_review,
        &boundary_perturbation_review,
        &stochastic_mode_review,
        &descriptor,
    );
    let single_run_no_spill_closure_green = acceptance_gate_tie.tied_requirement_satisfied
        && benchmark_prerequisite.article_demo_benchmark_equivalence_gate_green
        && operator_envelope.operator_envelope_green
        && horizon_review.deterministic_exactness_green
        && step_consistency_review.consistency_green
        && context_sensitivity_review.context_sensitivity_green
        && boundary_perturbation_review.perturbation_negative_control_green
        && stochastic_mode_review.stochastic_mode_robustness_green
        && binding_review.binding_green;

    let mut report = TassadarArticleSingleRunNoSpillClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_single_run_no_spill_closure.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        benchmark_prerequisite,
        operator_envelope,
        horizon_review,
        step_consistency_review,
        context_sensitivity_review,
        boundary_perturbation_review,
        stochastic_mode_review,
        binding_review,
        single_run_no_spill_closure_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && single_run_no_spill_closure_green,
        claim_boundary: String::from(
            "this report closes TAS-183 only. It freezes one no-resume single-run operator envelope on top of the canonical owned `psionic-transformer` route boundary by requiring the unified TAS-182 benchmark gate to stay green, the declared million-step and multi-million-step horizon receipts to stay exact and under floor, the context window to remain small relative to horizon size, and checkpoint/spill/resume lanes to stay explicit negative controls instead of inheriting article closure. It does not imply clean-room weight causality, route minimality, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article single-run no-spill closure now records tied_requirement_satisfied={}, benchmark_gate_green={}, operator_envelope_green={}, deterministic_exactness_green={}, step_consistency_green={}, context_sensitivity_green={}, perturbation_negative_control_green={}, stochastic_mode_robustness_green={}, binding_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report
            .benchmark_prerequisite
            .article_demo_benchmark_equivalence_gate_green,
        report.operator_envelope.operator_envelope_green,
        report.horizon_review.deterministic_exactness_green,
        report.step_consistency_review.consistency_green,
        report.context_sensitivity_review.context_sensitivity_green,
        report
            .boundary_perturbation_review
            .perturbation_negative_control_green,
        report.stochastic_mode_review.stochastic_mode_robustness_green,
        report.binding_review.binding_green,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_single_run_no_spill_closure_report|",
        &report,
    );
    report
}

fn build_benchmark_prerequisite(
    benchmark_prerequisite: &TassadarArticleDemoBenchmarkEquivalenceGateReport,
) -> TassadarArticleSingleRunNoSpillClosureBenchmarkPrerequisite {
    TassadarArticleSingleRunNoSpillClosureBenchmarkPrerequisite {
        report_ref: String::from(TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF),
        report_id: benchmark_prerequisite.report_id.clone(),
        tied_requirement_id: benchmark_prerequisite
            .acceptance_gate_tie
            .tied_requirement_id
            .clone(),
        tied_requirement_satisfied: benchmark_prerequisite
            .acceptance_gate_tie
            .tied_requirement_satisfied,
        blocked_issue_frontier: benchmark_prerequisite
            .acceptance_gate_tie
            .blocked_issue_ids
            .first()
            .cloned(),
        hungarian_demo_parity_green: benchmark_prerequisite.hungarian_demo_parity_green,
        named_arto_parity_green: benchmark_prerequisite.named_arto_parity_green,
        benchmark_wide_sudoku_parity_green: benchmark_prerequisite
            .benchmark_wide_sudoku_parity_green,
        binding_green: benchmark_prerequisite.binding_review.binding_green,
        article_demo_benchmark_equivalence_gate_green: benchmark_prerequisite
            .article_demo_benchmark_equivalence_gate_green,
        detail: format!(
            "Benchmark prerequisite `{}` keeps tied_requirement_id={}, blocked_issue_frontier={:?}, Hungarian={}, named_arto={}, hard_sudoku_suite={}, binding_green={}, and gate_green={}.",
            benchmark_prerequisite.report_id,
            benchmark_prerequisite.acceptance_gate_tie.tied_requirement_id,
            benchmark_prerequisite.acceptance_gate_tie.blocked_issue_ids.first(),
            benchmark_prerequisite.hungarian_demo_parity_green,
            benchmark_prerequisite.named_arto_parity_green,
            benchmark_prerequisite.benchmark_wide_sudoku_parity_green,
            benchmark_prerequisite.binding_review.binding_green,
            benchmark_prerequisite.article_demo_benchmark_equivalence_gate_green,
        ),
    }
}

fn build_operator_envelope(
    policy: &NoSpillPolicy,
) -> TassadarArticleSingleRunNoSpillClosureOperatorEnvelope {
    let operator_envelope_green = !policy.checkpoint_restore_allowed
        && !policy.spill_tape_extension_allowed
        && !policy.external_persisted_continuation_allowed
        && !policy.hidden_reentry_allowed
        && !policy.implicit_segmentation_allowed
        && !policy.runtime_loop_unrolling_allowed
        && !policy.teacher_forcing_allowed
        && !policy.oracle_leakage_allowed
        && !policy.oversized_context_memory_allowed
        && policy.deterministic_mode_required
        && !policy.allowed_scheduling_behavior_ids.is_empty()
        && !policy.disallowed_scheduling_behavior_ids.is_empty();

    TassadarArticleSingleRunNoSpillClosureOperatorEnvelope {
        token_stream_continuity_contract_id: String::from(TOKEN_STREAM_CONTINUITY_CONTRACT_ID),
        step_to_compute_contract_id: String::from(STEP_TO_COMPUTE_CONTRACT_ID),
        allowed_scheduling_behavior_ids: policy.allowed_scheduling_behavior_ids.clone(),
        disallowed_scheduling_behavior_ids: policy.disallowed_scheduling_behavior_ids.clone(),
        checkpoint_restore_allowed: policy.checkpoint_restore_allowed,
        spill_tape_extension_allowed: policy.spill_tape_extension_allowed,
        external_persisted_continuation_allowed: policy.external_persisted_continuation_allowed,
        hidden_reentry_allowed: policy.hidden_reentry_allowed,
        implicit_segmentation_allowed: policy.implicit_segmentation_allowed,
        runtime_loop_unrolling_allowed: policy.runtime_loop_unrolling_allowed,
        teacher_forcing_allowed: policy.teacher_forcing_allowed,
        oracle_leakage_allowed: policy.oracle_leakage_allowed,
        oversized_context_memory_allowed: policy.oversized_context_memory_allowed,
        deterministic_mode_required: policy.deterministic_mode_required,
        stochastic_mode_policy: if policy.stochastic_mode_supported {
            String::from("seeded_single_attempt_only")
        } else {
            String::from("explicit_refusal")
        },
        operator_envelope_green,
        detail: format!(
            "The no-spill operator envelope freezes token_stream_continuity_contract_id=`{}`, step_to_compute_contract_id=`{}`, deterministic_mode_required={}, stochastic_mode_policy=`{}`, and disallows checkpoint_restore={}, spill_tape_extension={}, external_persisted_continuation={}, hidden_reentry={}, implicit_segmentation={}, runtime_loop_unrolling={}, teacher_forcing={}, oracle_leakage={}, and oversized_context_memory={}.",
            TOKEN_STREAM_CONTINUITY_CONTRACT_ID,
            STEP_TO_COMPUTE_CONTRACT_ID,
            policy.deterministic_mode_required,
            if policy.stochastic_mode_supported {
                "seeded_single_attempt_only"
            } else {
                "explicit_refusal"
            },
            policy.checkpoint_restore_allowed,
            policy.spill_tape_extension_allowed,
            policy.external_persisted_continuation_allowed,
            policy.hidden_reentry_allowed,
            policy.implicit_segmentation_allowed,
            policy.runtime_loop_unrolling_allowed,
            policy.teacher_forcing_allowed,
            policy.oracle_leakage_allowed,
            policy.oversized_context_memory_allowed,
        ),
    }
}

fn build_horizon_review(
    runtime_closeout: &TassadarArticleRuntimeCloseoutReport,
    throughput_floor: &TassadarArticleFastRouteThroughputFloorReport,
    descriptor: &TassadarArticleTransformerDescriptorView,
) -> TassadarArticleSingleRunNoSpillClosureHorizonReview {
    let mut horizon_rows = runtime_closeout
        .bundle
        .horizon_receipts
        .iter()
        .map(|receipt| {
            let reference_linear_direct =
                receipt.reference_linear.selection_state == TassadarExecutorSelectionState::Direct;
            let hull_cache_direct =
                receipt.hull_cache.selection_state == TassadarExecutorSelectionState::Direct;
            let throughput_floor_passed = receipt.floor_status == TassadarArticleRuntimeFloorStatus::Passed
                && receipt.hull_cache_floor_status == TassadarArticleRuntimeFloorStatus::Passed;
            let step_count_alignment_green = receipt.exact_step_count
                == receipt.cpu_reference_summary.step_count
                && receipt.exact_step_count == receipt.direct_executor_summary.step_count;
            let trace_digest_alignment_green = receipt.cpu_reference_summary.trace_digest
                == receipt.direct_executor_summary.trace_digest;
            let behavior_digest_alignment_green = receipt.cpu_reference_summary.behavior_digest
                == receipt.direct_executor_summary.behavior_digest;
            let terminal_state_alignment_green = receipt.cpu_reference_summary.outputs
                == receipt.direct_executor_summary.outputs
                && receipt.cpu_reference_summary.final_locals
                    == receipt.direct_executor_summary.final_locals
                && receipt.cpu_reference_summary.final_memory
                    == receipt.direct_executor_summary.final_memory
                && receipt.cpu_reference_summary.final_stack
                    == receipt.direct_executor_summary.final_stack
                && receipt.cpu_reference_summary.halt_reason
                    == receipt.direct_executor_summary.halt_reason;
            let context_to_horizon_ratio_bps =
                ratio_bps(descriptor.config.max_target_positions, receipt.exact_step_count);
            let context_within_limit = context_to_horizon_ratio_bps <= 200;
            TassadarArticleSingleRunNoSpillHorizonRow {
                horizon_id: receipt.horizon_id.clone(),
                exact_step_count: receipt.exact_step_count,
                reference_linear_direct,
                hull_cache_direct,
                exactness_bps: receipt.exactness_bps,
                hull_cache_exactness_bps: receipt.hull_cache_exactness_bps,
                throughput_floor_passed,
                step_count_alignment_green,
                trace_digest_alignment_green,
                behavior_digest_alignment_green,
                terminal_state_alignment_green,
                context_to_horizon_ratio_bps,
                context_within_limit,
                detail: format!(
                    "Horizon `{}` keeps exact_step_count={}, reference_linear_direct={}, hull_cache_direct={}, exactness_bps={}, hull_cache_exactness_bps={}, throughput_floor_passed={}, step_count_alignment_green={}, trace_digest_alignment_green={}, behavior_digest_alignment_green={}, terminal_state_alignment_green={}, and context_to_horizon_ratio_bps={}.",
                    receipt.horizon_id,
                    receipt.exact_step_count,
                    reference_linear_direct,
                    hull_cache_direct,
                    receipt.exactness_bps,
                    receipt.hull_cache_exactness_bps,
                    throughput_floor_passed,
                    step_count_alignment_green,
                    trace_digest_alignment_green,
                    behavior_digest_alignment_green,
                    terminal_state_alignment_green,
                    context_to_horizon_ratio_bps,
                ),
            }
        })
        .collect::<Vec<_>>();
    horizon_rows.sort_by(|left, right| left.horizon_id.cmp(&right.horizon_id));

    let million_step_horizon_ids = horizon_rows
        .iter()
        .filter(|row| row.horizon_id.ends_with(".million_step"))
        .map(|row| row.horizon_id.clone())
        .collect::<Vec<_>>();
    let multi_million_step_horizon_ids = horizon_rows
        .iter()
        .filter(|row| row.horizon_id.ends_with(".two_million_step"))
        .map(|row| row.horizon_id.clone())
        .collect::<Vec<_>>();
    let deterministic_exactness_green = throughput_floor.throughput_floor_green
        && throughput_floor.throughput_bundle.selected_candidate_kind
            == EXPECTED_SELECTED_CANDIDATE_KIND
        && throughput_floor
            .throughput_bundle
            .selected_decode_mode
            .as_str()
            == EXPECTED_SELECTED_DECODE_MODE
        && horizon_rows.iter().all(|row| {
            row.reference_linear_direct
                && row.hull_cache_direct
                && row.exactness_bps == 10000
                && row.hull_cache_exactness_bps == 10000
                && row.throughput_floor_passed
        });

    TassadarArticleSingleRunNoSpillClosureHorizonReview {
        runtime_closeout_report_ref: String::from(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF),
        throughput_floor_report_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
        ),
        selected_candidate_kind: throughput_floor
            .throughput_bundle
            .selected_candidate_kind
            .clone(),
        selected_decode_mode: throughput_floor
            .throughput_bundle
            .selected_decode_mode
            .as_str()
            .to_string(),
        total_exact_step_count: horizon_rows
            .iter()
            .map(|row| u64::from(row.exact_step_count))
            .sum(),
        horizon_rows,
        million_step_horizon_ids,
        multi_million_step_horizon_ids,
        deterministic_exactness_green,
        detail: format!(
            "The single-run horizon review stays tied to `{}` plus `{}` on selected_candidate_kind=`{}` and selected_decode_mode=`{}` with deterministic_exactness_green={}.",
            TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF,
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
            throughput_floor.throughput_bundle.selected_candidate_kind,
            throughput_floor.throughput_bundle.selected_decode_mode.as_str(),
            deterministic_exactness_green,
        ),
    }
}

fn build_step_consistency_review(
    horizon_review: &TassadarArticleSingleRunNoSpillClosureHorizonReview,
) -> TassadarArticleSingleRunNoSpillClosureStepConsistencyReview {
    let step_count_alignment_green = horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.step_count_alignment_green);
    let trace_digest_alignment_green = horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.trace_digest_alignment_green);
    let behavior_digest_alignment_green = horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.behavior_digest_alignment_green);
    let terminal_state_alignment_green = horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.terminal_state_alignment_green);
    let consistency_green = step_count_alignment_green
        && trace_digest_alignment_green
        && behavior_digest_alignment_green
        && terminal_state_alignment_green;

    TassadarArticleSingleRunNoSpillClosureStepConsistencyReview {
        exact_horizon_count: horizon_review.horizon_rows.len() as u32,
        step_count_alignment_green,
        trace_digest_alignment_green,
        behavior_digest_alignment_green,
        terminal_state_alignment_green,
        consistency_green,
        detail: format!(
            "Step-to-compute consistency keeps exact_horizon_count={}, step_count_alignment_green={}, trace_digest_alignment_green={}, behavior_digest_alignment_green={}, terminal_state_alignment_green={}, and consistency_green={}.",
            horizon_review.horizon_rows.len(),
            step_count_alignment_green,
            trace_digest_alignment_green,
            behavior_digest_alignment_green,
            terminal_state_alignment_green,
            consistency_green,
        ),
    }
}

fn build_context_sensitivity_review(
    horizon_review: &TassadarArticleSingleRunNoSpillClosureHorizonReview,
    descriptor: &TassadarArticleTransformerDescriptorView,
    policy: &NoSpillPolicy,
) -> TassadarArticleSingleRunNoSpillClosureContextSensitivityReview {
    let worst_row = horizon_review
        .horizon_rows
        .iter()
        .max_by_key(|row| row.context_to_horizon_ratio_bps)
        .expect("single-run no-spill horizon review should contain at least one row");
    let context_sensitivity_green = horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.context_within_limit)
        && !policy.oversized_context_memory_allowed;

    TassadarArticleSingleRunNoSpillClosureContextSensitivityReview {
        model_descriptor_ref: String::from(MODEL_DESCRIPTOR_REF),
        model_id: descriptor.model.model_id.clone(),
        max_source_positions: descriptor.config.max_source_positions,
        max_target_positions: descriptor.config.max_target_positions,
        allowed_max_context_to_horizon_ratio_bps: policy.allowed_max_context_to_horizon_ratio_bps,
        worst_horizon_id: worst_row.horizon_id.clone(),
        worst_context_to_horizon_ratio_bps: worst_row.context_to_horizon_ratio_bps,
        context_sensitivity_green,
        detail: format!(
            "Context-length sensitivity binds `{}` model_id=`{}` max_source_positions={}, max_target_positions={}, allowed_max_context_to_horizon_ratio_bps={}, worst_horizon_id=`{}`, worst_context_to_horizon_ratio_bps={}, and context_sensitivity_green={}.",
            MODEL_DESCRIPTOR_REF,
            descriptor.model.model_id,
            descriptor.config.max_source_positions,
            descriptor.config.max_target_positions,
            policy.allowed_max_context_to_horizon_ratio_bps,
            worst_row.horizon_id,
            worst_row.context_to_horizon_ratio_bps,
            context_sensitivity_green,
        ),
    }
}

fn build_boundary_perturbation_review(
    execution_checkpoint: &TassadarExecutionCheckpointReport,
    spill_tape_store: &TassadarSpillTapeStoreReport,
    effect_safe_resume: &TassadarEffectSafeResumeReport,
    dynamic_memory_resume: &TassadarDynamicMemoryResumeReport,
) -> TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview {
    let checkpoint_resume_marker_green = execution_checkpoint.checkpoint_family_id
        == EXPECTED_EXECUTION_CHECKPOINT_FAMILY_ID
        && execution_checkpoint.exact_resume_parity_count > 0
        && execution_checkpoint.latest_checkpoint_locator_count > 0;
    let spill_tape_marker_green = spill_tape_store.profile_id == EXPECTED_SPILL_PROFILE_ID
        && spill_tape_store.external_tape_store_family_id == EXPECTED_EXTERNAL_TAPE_STORE_FAMILY_ID
        && spill_tape_store.exact_case_count > 0;
    let external_continuation_marker_green = effect_safe_resume.target_profile_id
        == EXPECTED_EFFECT_SAFE_RESUME_PROFILE_ID
        && effect_safe_resume.admitted_case_count > 0
        && effect_safe_resume.refusal_case_count > 0
        && !effect_safe_resume
            .continuation_refused_effect_refs
            .is_empty();
    let dynamic_memory_resume_marker_green = dynamic_memory_resume.checkpoint_family_id
        == EXPECTED_DYNAMIC_MEMORY_RESUME_FAMILY_ID
        && dynamic_memory_resume.exact_resume_parity_count > 0;
    let perturbation_negative_control_green = checkpoint_resume_marker_green
        && spill_tape_marker_green
        && external_continuation_marker_green
        && dynamic_memory_resume_marker_green;

    TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview {
        execution_checkpoint_report_ref: String::from(TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF),
        spill_tape_store_report_ref: String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
        effect_safe_resume_report_ref: String::from(TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF),
        dynamic_memory_resume_report_ref: String::from(TASSADAR_DYNAMIC_MEMORY_RESUME_REPORT_REF),
        checkpoint_family_id: execution_checkpoint.checkpoint_family_id.clone(),
        spill_profile_id: spill_tape_store.profile_id.clone(),
        external_tape_store_family_id: spill_tape_store.external_tape_store_family_id.clone(),
        effect_safe_resume_target_profile_id: effect_safe_resume.target_profile_id.clone(),
        dynamic_memory_resume_family_id: dynamic_memory_resume.checkpoint_family_id.clone(),
        checkpoint_resume_marker_green,
        spill_tape_marker_green,
        external_continuation_marker_green,
        dynamic_memory_resume_marker_green,
        perturbation_negative_control_green,
        detail: format!(
            "Boundary perturbation review keeps checkpoint_resume_marker_green={}, spill_tape_marker_green={}, external_continuation_marker_green={}, dynamic_memory_resume_marker_green={}, and perturbation_negative_control_green={} by treating `{}`, `{}`, `{}`, and `{}` as explicit disallowed continuation-marker lanes rather than inheriting them into article closure.",
            checkpoint_resume_marker_green,
            spill_tape_marker_green,
            external_continuation_marker_green,
            dynamic_memory_resume_marker_green,
            perturbation_negative_control_green,
            execution_checkpoint.report_id,
            spill_tape_store.report_id,
            effect_safe_resume.report_id,
            dynamic_memory_resume.report_id,
        ),
    }
}

fn build_stochastic_mode_review(
    policy: &NoSpillPolicy,
) -> TassadarArticleSingleRunNoSpillClosureStochasticModeReview {
    let stochastic_mode_robustness_green = !policy.stochastic_mode_supported
        && !policy.retry_farming_admitted
        && !policy.lucky_sampling_admitted
        && !policy.teacher_forcing_allowed
        && !policy.oracle_leakage_allowed;

    TassadarArticleSingleRunNoSpillClosureStochasticModeReview {
        stochastic_mode_supported: policy.stochastic_mode_supported,
        retry_farming_admitted: policy.retry_farming_admitted,
        lucky_sampling_admitted: policy.lucky_sampling_admitted,
        teacher_forcing_admitted: policy.teacher_forcing_allowed,
        oracle_leakage_admitted: policy.oracle_leakage_allowed,
        stochastic_mode_robustness_green,
        detail: format!(
            "Stochastic-mode posture keeps stochastic_mode_supported={}, retry_farming_admitted={}, lucky_sampling_admitted={}, teacher_forcing_admitted={}, oracle_leakage_admitted={}, and stochastic_mode_robustness_green={}.",
            policy.stochastic_mode_supported,
            policy.retry_farming_admitted,
            policy.lucky_sampling_admitted,
            policy.teacher_forcing_allowed,
            policy.oracle_leakage_allowed,
            stochastic_mode_robustness_green,
        ),
    }
}

fn build_binding_review(
    benchmark_prerequisite: &TassadarArticleSingleRunNoSpillClosureBenchmarkPrerequisite,
    operator_envelope: &TassadarArticleSingleRunNoSpillClosureOperatorEnvelope,
    horizon_review: &TassadarArticleSingleRunNoSpillClosureHorizonReview,
    step_consistency_review: &TassadarArticleSingleRunNoSpillClosureStepConsistencyReview,
    context_sensitivity_review: &TassadarArticleSingleRunNoSpillClosureContextSensitivityReview,
    boundary_perturbation_review: &TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview,
    stochastic_mode_review: &TassadarArticleSingleRunNoSpillClosureStochasticModeReview,
    descriptor: &TassadarArticleTransformerDescriptorView,
) -> TassadarArticleSingleRunNoSpillClosureBindingReview {
    let owned_route_boundary_refs = OWNED_ROUTE_BOUNDARY_REFS
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    let owned_route_boundary_refs_exist = owned_route_boundary_refs
        .iter()
        .all(|path| repo_root().join(path).is_file());
    let model_id_matches_expected = descriptor.model.model_id == EXPECTED_MODEL_ID;
    let benchmark_prerequisite_green = benchmark_prerequisite.tied_requirement_id
        == BENCHMARK_REQUIREMENT_ID
        && benchmark_prerequisite.tied_requirement_satisfied
        && benchmark_prerequisite.article_demo_benchmark_equivalence_gate_green
        && benchmark_prerequisite.binding_green;
    let selected_fast_route_green = horizon_review.selected_candidate_kind
        == EXPECTED_SELECTED_CANDIDATE_KIND
        && horizon_review.selected_decode_mode == EXPECTED_SELECTED_DECODE_MODE;
    let binding_green = owned_route_boundary_refs_exist
        && model_id_matches_expected
        && benchmark_prerequisite_green
        && selected_fast_route_green
        && operator_envelope.operator_envelope_green
        && horizon_review.deterministic_exactness_green
        && step_consistency_review.consistency_green
        && context_sensitivity_review.context_sensitivity_green
        && boundary_perturbation_review.perturbation_negative_control_green
        && stochastic_mode_review.stochastic_mode_robustness_green;

    TassadarArticleSingleRunNoSpillClosureBindingReview {
        owned_route_boundary_refs,
        owned_route_boundary_refs_exist,
        model_descriptor_ref: String::from(MODEL_DESCRIPTOR_REF),
        model_id: descriptor.model.model_id.clone(),
        model_id_matches_expected,
        benchmark_prerequisite_green,
        selected_fast_route_green,
        operator_envelope_green: operator_envelope.operator_envelope_green,
        deterministic_exactness_green: horizon_review.deterministic_exactness_green,
        step_consistency_green: step_consistency_review.consistency_green,
        context_sensitivity_green: context_sensitivity_review.context_sensitivity_green,
        perturbation_negative_control_green: boundary_perturbation_review
            .perturbation_negative_control_green,
        stochastic_mode_robustness_green: stochastic_mode_review
            .stochastic_mode_robustness_green,
        binding_green,
        detail: format!(
            "Binding review keeps owned_route_boundary_refs_exist={}, model_id_matches_expected={}, benchmark_prerequisite_green={}, selected_fast_route_green={}, operator_envelope_green={}, deterministic_exactness_green={}, step_consistency_green={}, context_sensitivity_green={}, perturbation_negative_control_green={}, stochastic_mode_robustness_green={}, and binding_green={}.",
            owned_route_boundary_refs_exist,
            model_id_matches_expected,
            benchmark_prerequisite_green,
            selected_fast_route_green,
            operator_envelope.operator_envelope_green,
            horizon_review.deterministic_exactness_green,
            step_consistency_review.consistency_green,
            context_sensitivity_review.context_sensitivity_green,
            boundary_perturbation_review.perturbation_negative_control_green,
            stochastic_mode_review.stochastic_mode_robustness_green,
            binding_green,
        ),
    }
}

pub fn tassadar_article_single_run_no_spill_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_article_single_run_no_spill_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleSingleRunNoSpillClosureReport,
    TassadarArticleSingleRunNoSpillClosureReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleSingleRunNoSpillClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_single_run_no_spill_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleSingleRunNoSpillClosureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn default_no_spill_policy() -> NoSpillPolicy {
    NoSpillPolicy {
        checkpoint_restore_allowed: false,
        spill_tape_extension_allowed: false,
        external_persisted_continuation_allowed: false,
        hidden_reentry_allowed: false,
        implicit_segmentation_allowed: false,
        runtime_loop_unrolling_allowed: false,
        teacher_forcing_allowed: false,
        oracle_leakage_allowed: false,
        oversized_context_memory_allowed: false,
        deterministic_mode_required: true,
        stochastic_mode_supported: false,
        retry_farming_admitted: false,
        lucky_sampling_admitted: false,
        allowed_scheduling_behavior_ids: ALLOWED_SCHEDULING_BEHAVIOR_IDS
            .into_iter()
            .map(String::from)
            .collect(),
        disallowed_scheduling_behavior_ids: DISALLOWED_SCHEDULING_BEHAVIOR_IDS
            .into_iter()
            .map(String::from)
            .collect(),
        allowed_max_context_to_horizon_ratio_bps: 200,
    }
}

fn ratio_bps(window: u32, horizon: u64) -> u16 {
    (((u64::from(window) * 10_000) + horizon - 1) / horizon) as u16
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleSingleRunNoSpillClosureReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleSingleRunNoSpillClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleSingleRunNoSpillClosureReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs, build_tassadar_article_single_run_no_spill_closure_report,
        default_no_spill_policy, read_repo_json,
        tassadar_article_single_run_no_spill_closure_report_path,
        write_tassadar_article_single_run_no_spill_closure_report, NoSpillPolicy,
        TassadarArticleSingleRunNoSpillClosureReport, TassadarArticleTransformerDescriptorView,
        TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_demo_benchmark_equivalence_gate_report,
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_article_fast_route_throughput_floor_report,
        build_tassadar_article_runtime_closeout_report,
        build_tassadar_dynamic_memory_resume_report, build_tassadar_effect_safe_resume_report,
        build_tassadar_execution_checkpoint_report, build_tassadar_spill_tape_store_report,
    };

    fn build_test_report(
        policy: &NoSpillPolicy,
        descriptor: TassadarArticleTransformerDescriptorView,
    ) -> TassadarArticleSingleRunNoSpillClosureReport {
        build_report_from_inputs(
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate"),
            build_tassadar_article_demo_benchmark_equivalence_gate_report()
                .expect("benchmark prerequisite"),
            build_tassadar_article_runtime_closeout_report().expect("runtime closeout"),
            build_tassadar_article_fast_route_throughput_floor_report().expect("throughput floor"),
            build_tassadar_execution_checkpoint_report().expect("execution checkpoint"),
            build_tassadar_spill_tape_store_report().expect("spill/tape"),
            build_tassadar_effect_safe_resume_report().expect("effect safe resume"),
            build_tassadar_dynamic_memory_resume_report().expect("dynamic memory"),
            descriptor,
            policy,
        )
    }

    fn committed_descriptor() -> TassadarArticleTransformerDescriptorView {
        read_repo_json(
            "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_descriptor.json",
            "tassadar_article_transformer_trace_bound_trained_descriptor",
        )
        .expect("descriptor")
    }

    #[test]
    fn article_single_run_no_spill_closure_tracks_green_gate() {
        let report = build_tassadar_article_single_run_no_spill_closure_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(
            report
                .benchmark_prerequisite
                .article_demo_benchmark_equivalence_gate_green
        );
        assert!(report.operator_envelope.operator_envelope_green);
        assert!(report.horizon_review.deterministic_exactness_green);
        assert!(report.step_consistency_review.consistency_green);
        assert!(report.context_sensitivity_review.context_sensitivity_green);
        assert!(
            report
                .boundary_perturbation_review
                .perturbation_negative_control_green
        );
        assert!(
            report
                .stochastic_mode_review
                .stochastic_mode_robustness_green
        );
        assert!(report.binding_review.binding_green);
        assert!(report.single_run_no_spill_closure_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_single_run_no_spill_closure_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_single_run_no_spill_closure_report()?;
        let committed: TassadarArticleSingleRunNoSpillClosureReport = read_repo_json(
            TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
            "tassadar_article_single_run_no_spill_closure_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_single_run_no_spill_closure_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_single_run_no_spill_closure_report.json");
        let written = write_tassadar_article_single_run_no_spill_closure_report(&output_path)?;
        let persisted: TassadarArticleSingleRunNoSpillClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_single_run_no_spill_closure_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_single_run_no_spill_closure_report.json")
        );
        Ok(())
    }

    #[test]
    fn article_single_run_no_spill_closure_fails_when_checkpoint_markers_disappear() {
        let mut execution_checkpoint =
            build_tassadar_execution_checkpoint_report().expect("execution checkpoint");
        execution_checkpoint.exact_resume_parity_count = 0;
        execution_checkpoint.latest_checkpoint_locator_count = 0;
        let report = build_report_from_inputs(
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate"),
            build_tassadar_article_demo_benchmark_equivalence_gate_report()
                .expect("benchmark prerequisite"),
            build_tassadar_article_runtime_closeout_report().expect("runtime closeout"),
            build_tassadar_article_fast_route_throughput_floor_report().expect("throughput floor"),
            execution_checkpoint,
            build_tassadar_spill_tape_store_report().expect("spill/tape"),
            build_tassadar_effect_safe_resume_report().expect("effect safe resume"),
            build_tassadar_dynamic_memory_resume_report().expect("dynamic memory"),
            committed_descriptor(),
            &default_no_spill_policy(),
        );

        assert!(
            !report
                .boundary_perturbation_review
                .checkpoint_resume_marker_green
        );
        assert!(!report.single_run_no_spill_closure_green);
    }

    #[test]
    fn article_single_run_no_spill_closure_fails_when_spill_markers_disappear() {
        let mut spill_tape_store = build_tassadar_spill_tape_store_report().expect("spill/tape");
        spill_tape_store.exact_case_count = 0;
        let report = build_report_from_inputs(
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate"),
            build_tassadar_article_demo_benchmark_equivalence_gate_report()
                .expect("benchmark prerequisite"),
            build_tassadar_article_runtime_closeout_report().expect("runtime closeout"),
            build_tassadar_article_fast_route_throughput_floor_report().expect("throughput floor"),
            build_tassadar_execution_checkpoint_report().expect("execution checkpoint"),
            spill_tape_store,
            build_tassadar_effect_safe_resume_report().expect("effect safe resume"),
            build_tassadar_dynamic_memory_resume_report().expect("dynamic memory"),
            committed_descriptor(),
            &default_no_spill_policy(),
        );

        assert!(!report.boundary_perturbation_review.spill_tape_marker_green);
        assert!(!report.single_run_no_spill_closure_green);
    }

    #[test]
    fn article_single_run_no_spill_closure_fails_when_hidden_reentry_is_admitted() {
        let mut policy = default_no_spill_policy();
        policy.hidden_reentry_allowed = true;
        let report = build_test_report(&policy, committed_descriptor());

        assert!(!report.operator_envelope.operator_envelope_green);
        assert!(!report.single_run_no_spill_closure_green);
    }

    #[test]
    fn article_single_run_no_spill_closure_fails_when_teacher_forcing_is_admitted() {
        let mut policy = default_no_spill_policy();
        policy.teacher_forcing_allowed = true;
        let report = build_test_report(&policy, committed_descriptor());

        assert!(!report.operator_envelope.operator_envelope_green);
        assert!(
            !report
                .stochastic_mode_review
                .stochastic_mode_robustness_green
        );
        assert!(!report.single_run_no_spill_closure_green);
    }

    #[test]
    fn article_single_run_no_spill_closure_fails_when_context_window_is_oversized() {
        let mut descriptor = committed_descriptor();
        descriptor.config.max_target_positions = 4_000_000;
        let report = build_test_report(&default_no_spill_policy(), descriptor);

        assert!(!report.context_sensitivity_review.context_sensitivity_green);
        assert!(!report.single_run_no_spill_closure_green);
    }
}
