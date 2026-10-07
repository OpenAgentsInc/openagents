use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_core::{Shape, TensorData};
use psionic_models::{TassadarArticleTransformer, TassadarArticleTransformerError};
use psionic_runtime::{
    tassadar_article_class_corpus, TassadarExecution, TassadarFixtureRunner, TassadarValidationCase,
};
use psionic_transformer::{AttentionMask, TransformerExecutionMode};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleInterpreterOwnershipGateReport,
    TassadarArticleTransformerReferenceLinearExactnessGateReport,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_kv_activation_discipline_audit_report.json";
pub const TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-article-kv-activation-discipline-audit.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-184A";
const EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_reference_linear_exactness_gate_report.json";
const MODERATE_HISTORY_WINDOW_TOKENS: usize = 8;
const STRICT_HISTORY_WINDOW_TOKENS: usize = 1;
const DECLARED_CASE_IDS: &[&str] = &[
    "micro_wasm_kernel",
    "branch_heavy_kernel",
    "memory_heavy_kernel",
    "long_loop_kernel",
    "sudoku_v0_test_a",
    "hungarian_matching",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationDisciplineAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub ownership_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationGrowthCaseRow {
    pub case_id: String,
    pub workload_family_id: String,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub constraint_audit_supported: bool,
    pub decoder_kv_bytes: u64,
    pub cross_attention_kv_bytes: u64,
    pub encoder_activation_bytes: u64,
    pub decoder_activation_bytes: u64,
    pub total_activation_bytes: u64,
    pub dynamic_state_bytes: u64,
    pub total_state_bytes: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationGrowthReport {
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub weight_artifact_bytes: u64,
    pub decoder_kv_bytes_per_generated_token: u64,
    pub cross_attention_kv_bytes_per_prompt_token: u64,
    pub feasible_constraint_case_ids: Vec<String>,
    pub case_rows: Vec<TassadarArticleKvActivationGrowthCaseRow>,
    pub cache_growth_scales_with_problem_size: bool,
    pub dynamic_state_exceeds_weight_artifact_bytes: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleStateConstraintKind {
    SlidingWindowModerate,
    SlidingWindowStrict,
    MidDecodeReset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleStateConstraintCaseRow {
    pub case_id: String,
    pub workload_family_id: String,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub baseline_exact: bool,
    pub constrained_exact: bool,
    pub behavior_changed: bool,
    pub baseline_behavior_digest: String,
    pub constrained_behavior_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleStateConstraintRow {
    pub constraint_kind: TassadarArticleStateConstraintKind,
    pub audited_case_ids: Vec<String>,
    pub case_rows: Vec<TassadarArticleStateConstraintCaseRow>,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub behavior_changed_case_count: usize,
    pub all_audited_cases_sensitive: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationSensitivityReview {
    pub audited_case_ids: Vec<String>,
    pub moderate_window_tokens: usize,
    pub strict_window_tokens: usize,
    pub constraint_rows: Vec<TassadarArticleStateConstraintRow>,
    pub cache_truncation_breaks_correctness: bool,
    pub cache_reset_breaks_correctness: bool,
    pub equivalent_behavior_survives_under_constrained_cache: bool,
    pub constraint_suite_complete: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleStateCarrierKind {
    KvCache,
    ResidualStream,
    AttentionHistory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleStateCarrierBoundaryRow {
    pub carrier_kind: TassadarArticleStateCarrierKind,
    pub acceptable_use: String,
    pub non_acceptable_use: String,
    pub declared_boundary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleStateDominanceVerdictKind {
    WeightDominant,
    ActivationDominant,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleStateDominanceVerdict {
    pub verdict: TassadarArticleStateDominanceVerdictKind,
    pub weight_perturbation_sensitivity_green: bool,
    pub cache_truncation_sensitivity_green: bool,
    pub cache_reset_sensitivity_green: bool,
    pub cache_growth_scales_with_problem_size: bool,
    pub dynamic_state_exceeds_weight_artifact_bytes: bool,
    pub carrier_boundary_declared: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationBindingReview {
    pub ownership_gate_green: bool,
    pub weight_perturbation_sensitivity_green: bool,
    pub cache_growth_scales_with_problem_size: bool,
    pub cache_truncation_breaks_correctness: bool,
    pub cache_reset_breaks_correctness: bool,
    pub carrier_boundary_declared: bool,
    pub dominance_verdict_declared: bool,
    pub discipline_audit_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleKvActivationDisciplineAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleKvActivationDisciplineAcceptanceGateTie,
    pub ownership_gate_report_ref: String,
    pub ownership_gate_green: bool,
    pub growth_report: TassadarArticleKvActivationGrowthReport,
    pub sensitivity_review: TassadarArticleKvActivationSensitivityReview,
    pub carrier_boundary_rows: Vec<TassadarArticleStateCarrierBoundaryRow>,
    pub dominance_verdict: TassadarArticleStateDominanceVerdict,
    pub binding_review: TassadarArticleKvActivationBindingReview,
    pub kv_activation_discipline_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleKvActivationDisciplineAuditError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
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
    #[error("expected exactness row for case `{case_id}`")]
    MissingExactnessRow { case_id: String },
    #[error("expected article corpus case `{case_id}`")]
    MissingCorpusCase { case_id: String },
    #[error("constraint audit failed for case `{case_id}`: {detail}")]
    ConstraintCase { case_id: String, detail: String },
}

#[derive(Clone, Debug)]
struct AuditCaseInputs {
    case: TassadarValidationCase,
    workload_family_id: String,
    batch: psionic_models::TassadarArticleTransformerTraceDomainBatch,
    baseline_exact: bool,
    baseline_behavior_digest: String,
}

#[derive(Clone, Debug)]
struct ModelCaseBehavior {
    exact: bool,
    behavior_digest: String,
}

pub fn build_tassadar_article_kv_activation_discipline_audit_report() -> Result<
    TassadarArticleKvActivationDisciplineAuditReport,
    TassadarArticleKvActivationDisciplineAuditError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let ownership_report: TassadarArticleInterpreterOwnershipGateReport = read_repo_json(
        TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
        "article_interpreter_ownership_gate",
    )?;
    let exactness_report: TassadarArticleTransformerReferenceLinearExactnessGateReport =
        read_repo_json(
            EXACTNESS_REPORT_REF,
            "article_transformer_reference_linear_exactness",
        )?;
    let model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    let growth_report = build_growth_report(&model, &exactness_report)?;
    let sensitivity_review = build_sensitivity_review(&model, &growth_report)?;
    let carrier_boundary_rows = carrier_boundary_rows();
    let dominance_verdict = build_dominance_verdict(
        &ownership_report,
        &growth_report,
        &sensitivity_review,
        &carrier_boundary_rows,
    );
    Ok(build_report_from_inputs(
        acceptance_gate,
        ownership_report,
        growth_report,
        sensitivity_review,
        carrier_boundary_rows,
        dominance_verdict,
    ))
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    ownership_report: TassadarArticleInterpreterOwnershipGateReport,
    growth_report: TassadarArticleKvActivationGrowthReport,
    sensitivity_review: TassadarArticleKvActivationSensitivityReview,
    carrier_boundary_rows: Vec<TassadarArticleStateCarrierBoundaryRow>,
    dominance_verdict: TassadarArticleStateDominanceVerdict,
) -> TassadarArticleKvActivationDisciplineAuditReport {
    let acceptance_gate_tie = TassadarArticleKvActivationDisciplineAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        ownership_gate_report_ref: String::from(
            TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    };
    let kv_activation_discipline_green = acceptance_gate_tie.tied_requirement_satisfied
        && ownership_report.interpreter_ownership_green
        && growth_report.cache_growth_scales_with_problem_size
        && sensitivity_review.constraint_suite_complete
        && !carrier_boundary_rows.is_empty();
    let binding_review = TassadarArticleKvActivationBindingReview {
        ownership_gate_green: ownership_report.interpreter_ownership_green,
        weight_perturbation_sensitivity_green: ownership_report
            .weight_perturbation_review
            .all_interventions_show_sensitivity,
        cache_growth_scales_with_problem_size: growth_report.cache_growth_scales_with_problem_size,
        cache_truncation_breaks_correctness: sensitivity_review.cache_truncation_breaks_correctness,
        cache_reset_breaks_correctness: sensitivity_review.cache_reset_breaks_correctness,
        carrier_boundary_declared: !carrier_boundary_rows.is_empty(),
        dominance_verdict_declared: true,
        discipline_audit_green: kv_activation_discipline_green,
        detail: format!(
            "ownership_gate_green={} weight_perturbation_sensitivity_green={} cache_growth_scales_with_problem_size={} cache_truncation_breaks_correctness={} cache_reset_breaks_correctness={} carrier_boundary_declared={}",
            ownership_report.interpreter_ownership_green,
            ownership_report
                .weight_perturbation_review
                .all_interventions_show_sensitivity,
            growth_report.cache_growth_scales_with_problem_size,
            sensitivity_review.cache_truncation_breaks_correctness,
            sensitivity_review.cache_reset_breaks_correctness,
            !carrier_boundary_rows.is_empty(),
        ),
    };
    let mut report = TassadarArticleKvActivationDisciplineAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_kv_activation_discipline_audit.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        ownership_gate_report_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF),
        ownership_gate_green: ownership_report.interpreter_ownership_green,
        growth_report,
        sensitivity_review,
        carrier_boundary_rows,
        dominance_verdict,
        binding_review,
        kv_activation_discipline_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && kv_activation_discipline_green,
        claim_boundary: String::from(
            "this report closes TAS-184A only. It declares the article-route KV-cache and activation-state discipline verdict for the bounded public envelope, including explicit acceptable versus non-acceptable state carriers and an explicit weight-dominant versus activation-dominant versus mixed verdict. By itself it still does not replace the later cross-machine reproducibility matrix, route-minimality audit, or final article-equivalence audit.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article KV-cache and activation-state discipline audit now records tied_requirement_satisfied={}, verdict={:?}, cache_growth_scales_with_problem_size={}, truncation_breaks_correctness={}, reset_breaks_correctness={}, constrained_cache_equivalence={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.dominance_verdict.verdict,
        report.growth_report.cache_growth_scales_with_problem_size,
        report.sensitivity_review.cache_truncation_breaks_correctness,
        report.sensitivity_review.cache_reset_breaks_correctness,
        report
            .sensitivity_review
            .equivalent_behavior_survives_under_constrained_cache,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_kv_activation_discipline_audit_report|",
        &report,
    );
    report
}

fn build_growth_report(
    model: &TassadarArticleTransformer,
    exactness_report: &TassadarArticleTransformerReferenceLinearExactnessGateReport,
) -> Result<TassadarArticleKvActivationGrowthReport, TassadarArticleKvActivationDisciplineAuditError>
{
    let exactness_by_case_id = exactness_report
        .case_rows
        .iter()
        .map(|row| (row.case_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let model_config = &model.descriptor().config;
    let weight_artifact_bytes = model
        .weight_metadata()
        .artifacts
        .iter()
        .map(|artifact| artifact.byte_length)
        .sum::<u64>();
    let bytes_per_f32 = 4u64;
    let hidden_size = model_config.hidden_size as u64;
    let decoder_layer_count = model_config.decoder_layer_count as u64;
    let encoder_layer_count = model_config.encoder_layer_count as u64;
    let decoder_kv_bytes_per_generated_token =
        decoder_layer_count * 2 * hidden_size * bytes_per_f32;
    let cross_attention_kv_bytes_per_prompt_token =
        decoder_layer_count * 2 * hidden_size * bytes_per_f32;
    let encoder_activation_bytes_per_token =
        (encoder_layer_count + 1) * hidden_size * bytes_per_f32;
    let decoder_activation_bytes_per_token =
        (decoder_layer_count + 1) * hidden_size * bytes_per_f32;
    let case_rows = DECLARED_CASE_IDS
        .iter()
        .map(|case_id| {
            let exactness_row = exactness_by_case_id.get(case_id).ok_or_else(|| {
                TassadarArticleKvActivationDisciplineAuditError::MissingExactnessRow {
                    case_id: String::from(*case_id),
                }
            })?;
            Ok(TassadarArticleKvActivationGrowthCaseRow {
                case_id: String::from(*case_id),
                workload_family_id: workload_family_id_for_case(case_id),
                prompt_token_count: exactness_row.prompt_token_count,
                target_token_count: exactness_row.target_token_count,
                constraint_audit_supported: exactness_row.prompt_token_count
                    <= model_config.max_source_positions
                    && exactness_row.target_token_count <= model_config.max_target_positions,
                decoder_kv_bytes: exactness_row.target_token_count as u64
                    * decoder_kv_bytes_per_generated_token,
                cross_attention_kv_bytes: exactness_row.prompt_token_count as u64
                    * cross_attention_kv_bytes_per_prompt_token,
                encoder_activation_bytes: exactness_row.prompt_token_count as u64
                    * encoder_activation_bytes_per_token,
                decoder_activation_bytes: exactness_row.target_token_count as u64
                    * decoder_activation_bytes_per_token,
                total_activation_bytes: exactness_row.prompt_token_count as u64
                    * encoder_activation_bytes_per_token
                    + exactness_row.target_token_count as u64 * decoder_activation_bytes_per_token,
                dynamic_state_bytes: exactness_row.target_token_count as u64
                    * (decoder_kv_bytes_per_generated_token + decoder_activation_bytes_per_token),
                total_state_bytes: exactness_row.prompt_token_count as u64
                    * (encoder_activation_bytes_per_token + cross_attention_kv_bytes_per_prompt_token)
                    + exactness_row.target_token_count as u64
                        * (decoder_kv_bytes_per_generated_token + decoder_activation_bytes_per_token),
                detail: String::from(
                    "analytic state accounting uses the declared encoder/decoder layer counts plus hidden size to make the article-route KV and activation carrier explicit per token instead of hiding it behind runtime heuristics",
                ),
            })
        })
        .collect::<Result<Vec<_>, TassadarArticleKvActivationDisciplineAuditError>>()?;
    let feasible_constraint_case_ids = case_rows
        .iter()
        .filter(|row| row.constraint_audit_supported)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let feasible_constraint_case_count = feasible_constraint_case_ids.len();
    let cache_growth_scales_with_problem_size = case_rows
        .iter()
        .all(|row| row.decoder_kv_bytes > 0 && row.dynamic_state_bytes >= row.decoder_kv_bytes)
        && case_rows.windows(2).all(|pair| {
            pair[0].target_token_count <= pair[1].target_token_count
                || pair[0].decoder_kv_bytes != pair[1].decoder_kv_bytes
        });
    let dynamic_state_exceeds_weight_artifact_bytes = case_rows
        .iter()
        .any(|row| row.dynamic_state_bytes > weight_artifact_bytes);
    Ok(TassadarArticleKvActivationGrowthReport {
        model_id: model.descriptor().model.model_id.clone(),
        model_descriptor_digest: model.descriptor().stable_digest(),
        weight_artifact_bytes,
        decoder_kv_bytes_per_generated_token,
        cross_attention_kv_bytes_per_prompt_token,
        feasible_constraint_case_ids,
        case_rows,
        cache_growth_scales_with_problem_size,
        dynamic_state_exceeds_weight_artifact_bytes,
        detail: format!(
            "decoder_kv_bytes_per_generated_token={} cross_attention_kv_bytes_per_prompt_token={} weight_artifact_bytes={} feasible_constraint_case_count={}",
            decoder_kv_bytes_per_generated_token,
            cross_attention_kv_bytes_per_prompt_token,
            weight_artifact_bytes,
            feasible_constraint_case_count,
        ),
    })
}

fn build_sensitivity_review(
    model: &TassadarArticleTransformer,
    growth_report: &TassadarArticleKvActivationGrowthReport,
) -> Result<
    TassadarArticleKvActivationSensitivityReview,
    TassadarArticleKvActivationDisciplineAuditError,
> {
    let corpus = tassadar_article_class_corpus();
    let corpus_by_case_id = corpus
        .into_iter()
        .map(|case| (case.case_id.clone(), case))
        .collect::<BTreeMap<_, _>>();
    let audited_case_ids = growth_report.feasible_constraint_case_ids.clone();
    let audited_cases = audited_case_ids
        .iter()
        .map(|case_id| {
            let case = corpus_by_case_id.get(case_id).ok_or_else(|| {
                TassadarArticleKvActivationDisciplineAuditError::MissingCorpusCase {
                    case_id: case_id.clone(),
                }
            })?;
            build_audit_case_inputs(model, case)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let moderate_row = build_constraint_row(
        model,
        &audited_cases,
        TassadarArticleStateConstraintKind::SlidingWindowModerate,
    )?;
    let strict_row = build_constraint_row(
        model,
        &audited_cases,
        TassadarArticleStateConstraintKind::SlidingWindowStrict,
    )?;
    let reset_row = build_constraint_row(
        model,
        &audited_cases,
        TassadarArticleStateConstraintKind::MidDecodeReset,
    )?;
    let constraint_rows = vec![moderate_row, strict_row, reset_row];
    let cache_truncation_breaks_correctness = constraint_rows
        .iter()
        .filter(|row| {
            matches!(
                row.constraint_kind,
                TassadarArticleStateConstraintKind::SlidingWindowModerate
                    | TassadarArticleStateConstraintKind::SlidingWindowStrict
            )
        })
        .any(|row| row.mismatch_case_count > 0);
    let cache_reset_breaks_correctness = constraint_rows
        .iter()
        .find(|row| row.constraint_kind == TassadarArticleStateConstraintKind::MidDecodeReset)
        .is_some_and(|row| row.mismatch_case_count > 0);
    let equivalent_behavior_survives_under_constrained_cache = constraint_rows
        .iter()
        .filter(|row| {
            matches!(
                row.constraint_kind,
                TassadarArticleStateConstraintKind::SlidingWindowModerate
                    | TassadarArticleStateConstraintKind::SlidingWindowStrict
            )
        })
        .all(|row| row.mismatch_case_count == 0);
    Ok(TassadarArticleKvActivationSensitivityReview {
        audited_case_ids,
        moderate_window_tokens: MODERATE_HISTORY_WINDOW_TOKENS,
        strict_window_tokens: STRICT_HISTORY_WINDOW_TOKENS,
        constraint_rows,
        cache_truncation_breaks_correctness,
        cache_reset_breaks_correctness,
        equivalent_behavior_survives_under_constrained_cache,
        constraint_suite_complete: !growth_report.feasible_constraint_case_ids.is_empty(),
        detail: format!(
            "audited_case_count={} moderate_window_tokens={} strict_window_tokens={} cache_truncation_breaks_correctness={} cache_reset_breaks_correctness={} constrained_cache_equivalence={}",
            growth_report.feasible_constraint_case_ids.len(),
            MODERATE_HISTORY_WINDOW_TOKENS,
            STRICT_HISTORY_WINDOW_TOKENS,
            cache_truncation_breaks_correctness,
            cache_reset_breaks_correctness,
            equivalent_behavior_survives_under_constrained_cache,
        ),
    })
}

fn build_constraint_row(
    model: &TassadarArticleTransformer,
    audited_cases: &[AuditCaseInputs],
    constraint_kind: TassadarArticleStateConstraintKind,
) -> Result<TassadarArticleStateConstraintRow, TassadarArticleKvActivationDisciplineAuditError> {
    let case_rows = audited_cases
        .iter()
        .map(|inputs| {
            let constrained_behavior =
                evaluate_model_behavior_with_constraint(model, inputs, constraint_kind)?;
            Ok(TassadarArticleStateConstraintCaseRow {
                case_id: inputs.case.case_id.clone(),
                workload_family_id: inputs.workload_family_id.clone(),
                prompt_token_count: inputs.batch.prompt_token_count,
                target_token_count: inputs.batch.target_token_count,
                baseline_exact: inputs.baseline_exact,
                constrained_exact: constrained_behavior.exact,
                behavior_changed: constrained_behavior.behavior_digest
                    != inputs.baseline_behavior_digest,
                baseline_behavior_digest: inputs.baseline_behavior_digest.clone(),
                constrained_behavior_digest: constrained_behavior.behavior_digest,
                detail: constraint_detail(constraint_kind),
            })
        })
        .collect::<Result<Vec<_>, TassadarArticleKvActivationDisciplineAuditError>>()?;
    let exact_case_count = case_rows.iter().filter(|row| row.constrained_exact).count();
    let mismatch_case_count = case_rows.len().saturating_sub(exact_case_count);
    let behavior_changed_case_count = case_rows.iter().filter(|row| row.behavior_changed).count();
    Ok(TassadarArticleStateConstraintRow {
        constraint_kind,
        audited_case_ids: case_rows.iter().map(|row| row.case_id.clone()).collect(),
        exact_case_count,
        mismatch_case_count,
        behavior_changed_case_count,
        all_audited_cases_sensitive: mismatch_case_count == case_rows.len()
            && behavior_changed_case_count == case_rows.len(),
        detail: format!(
            "exact_case_count={} mismatch_case_count={} behavior_changed_case_count={}",
            exact_case_count, mismatch_case_count, behavior_changed_case_count,
        ),
        case_rows,
    })
}

fn carrier_boundary_rows() -> Vec<TassadarArticleStateCarrierBoundaryRow> {
    vec![
        TassadarArticleStateCarrierBoundaryRow {
            carrier_kind: TassadarArticleStateCarrierKind::KvCache,
            acceptable_use: String::from(
                "request-local decoder and cross-attention cache state inside one admitted continuous forward-pass route",
            ),
            non_acceptable_use: String::from(
                "persisted, resumed, or cross-run cache state that silently reintroduces the blocked continuation or spill surfaces from TAS-183",
            ),
            declared_boundary: String::from(
                "KV state is acceptable only as ephemeral same-run attention carrier state that is reset at the request boundary and stays subordinate to the declared Transformer route",
            ),
        },
        TassadarArticleStateCarrierBoundaryRow {
            carrier_kind: TassadarArticleStateCarrierKind::ResidualStream,
            acceptable_use: String::from(
                "ephemeral layer-local activation flow inside the declared encoder-decoder forward pass",
            ),
            non_acceptable_use: String::from(
                "externalized residual activations or hidden helper-mediated activation replay that would act as an undeclared control plane",
            ),
            declared_boundary: String::from(
                "residual activations are admitted only as transient within-pass state, not as a durable interpreter substrate outside the forward pass",
            ),
        },
        TassadarArticleStateCarrierBoundaryRow {
            carrier_kind: TassadarArticleStateCarrierKind::AttentionHistory,
            acceptable_use: String::from(
                "the explicit source-token and prior-target-token history already declared by the article trace domain",
            ),
            non_acceptable_use: String::from(
                "extra hidden history, undeclared prompt extensions, or cache reuse across unrelated requests that smuggle in additional interpreter state",
            ),
            declared_boundary: String::from(
                "attention history is acceptable only when it is exactly the current request-local article trace, with no undeclared history channel beyond the admitted source and target tokens",
            ),
        },
    ]
}

fn build_dominance_verdict(
    ownership_report: &TassadarArticleInterpreterOwnershipGateReport,
    growth_report: &TassadarArticleKvActivationGrowthReport,
    sensitivity_review: &TassadarArticleKvActivationSensitivityReview,
    carrier_boundary_rows: &[TassadarArticleStateCarrierBoundaryRow],
) -> TassadarArticleStateDominanceVerdict {
    let weight_perturbation_sensitivity_green = ownership_report
        .weight_perturbation_review
        .all_interventions_show_sensitivity;
    let cache_truncation_sensitivity_green = sensitivity_review.cache_truncation_breaks_correctness;
    let cache_reset_sensitivity_green = sensitivity_review.cache_reset_breaks_correctness;
    let verdict = match (
        weight_perturbation_sensitivity_green,
        cache_truncation_sensitivity_green || cache_reset_sensitivity_green,
    ) {
        (true, false) => TassadarArticleStateDominanceVerdictKind::WeightDominant,
        (false, true) => TassadarArticleStateDominanceVerdictKind::ActivationDominant,
        _ => TassadarArticleStateDominanceVerdictKind::Mixed,
    };
    TassadarArticleStateDominanceVerdict {
        verdict,
        weight_perturbation_sensitivity_green,
        cache_truncation_sensitivity_green,
        cache_reset_sensitivity_green,
        cache_growth_scales_with_problem_size: growth_report.cache_growth_scales_with_problem_size,
        dynamic_state_exceeds_weight_artifact_bytes: growth_report
            .dynamic_state_exceeds_weight_artifact_bytes,
        carrier_boundary_declared: !carrier_boundary_rows.is_empty(),
        detail: format!(
            "weight_perturbation_sensitivity_green={} cache_truncation_sensitivity_green={} cache_reset_sensitivity_green={} cache_growth_scales_with_problem_size={} dynamic_state_exceeds_weight_artifact_bytes={} verdict={:?}",
            weight_perturbation_sensitivity_green,
            cache_truncation_sensitivity_green,
            cache_reset_sensitivity_green,
            growth_report.cache_growth_scales_with_problem_size,
            growth_report.dynamic_state_exceeds_weight_artifact_bytes,
            verdict,
        ),
    }
}

fn build_audit_case_inputs(
    model: &TassadarArticleTransformer,
    case: &TassadarValidationCase,
) -> Result<AuditCaseInputs, TassadarArticleKvActivationDisciplineAuditError> {
    let fixture_execution = fixture_execution_for_case(case)?;
    let batch = model
        .tokenize_article_trace_domain_unbounded(&case.program, &fixture_execution)
        .map_err(
            |error| TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
                case_id: case.case_id.clone(),
                detail: error.to_string(),
            },
        )?;
    let baseline_behavior =
        evaluate_model_behavior_with_mask(model, &batch, None, case.case_id.as_str())?;
    Ok(AuditCaseInputs {
        case: case.clone(),
        workload_family_id: workload_family_id_for_case(case.case_id.as_str()),
        batch,
        baseline_exact: baseline_behavior.exact,
        baseline_behavior_digest: baseline_behavior.behavior_digest,
    })
}

fn evaluate_model_behavior_with_constraint(
    model: &TassadarArticleTransformer,
    inputs: &AuditCaseInputs,
    constraint_kind: TassadarArticleStateConstraintKind,
) -> Result<ModelCaseBehavior, TassadarArticleKvActivationDisciplineAuditError> {
    let batch_size = inputs.batch.target_shape[0];
    let target_len = inputs.batch.target_shape[1];
    let decoder_mask = match constraint_kind {
        TassadarArticleStateConstraintKind::SlidingWindowModerate => Some(
            decoder_sliding_window_mask(batch_size, target_len, MODERATE_HISTORY_WINDOW_TOKENS)?,
        ),
        TassadarArticleStateConstraintKind::SlidingWindowStrict => Some(
            decoder_sliding_window_mask(batch_size, target_len, STRICT_HISTORY_WINDOW_TOKENS)?,
        ),
        TassadarArticleStateConstraintKind::MidDecodeReset => {
            Some(decoder_mid_decode_reset_mask(batch_size, target_len)?)
        }
    };
    evaluate_model_behavior_with_mask(
        model,
        &inputs.batch,
        decoder_mask.as_ref(),
        inputs.case.case_id.as_str(),
    )
}

fn evaluate_model_behavior_with_mask(
    model: &TassadarArticleTransformer,
    batch: &psionic_models::TassadarArticleTransformerTraceDomainBatch,
    decoder_mask: Option<&AttentionMask>,
    case_id: &str,
) -> Result<ModelCaseBehavior, TassadarArticleKvActivationDisciplineAuditError> {
    let output = model
        .forward_with_masks(
            Shape::new(batch.source_shape.clone()),
            batch.source_token_ids.as_slice(),
            Shape::new(batch.target_shape.clone()),
            batch.target_token_ids.as_slice(),
            None,
            decoder_mask,
            None,
            TransformerExecutionMode::Eval,
        )
        .map_err(
            |error| TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
                case_id: String::from(case_id),
                detail: error.to_string(),
            },
        )?;
    let predicted_token_ids =
        predicted_token_ids_from_logits(output.logits.dims(), &output.logits.data, case_id)?;
    Ok(ModelCaseBehavior {
        exact: predicted_token_ids == batch.target_token_ids,
        behavior_digest: stable_digest(
            b"psionic_tassadar_article_kv_activation_discipline_behavior_digest|",
            &(
                case_id,
                batch.sequence_digest.as_str(),
                predicted_token_ids,
                tensor_data_digest(&output.logits.data),
            ),
        ),
    })
}

fn decoder_sliding_window_mask(
    batch_size: usize,
    target_len: usize,
    window_tokens: usize,
) -> Result<AttentionMask, TassadarArticleKvActivationDisciplineAuditError> {
    let mut allowed = vec![false; batch_size * target_len * target_len];
    for batch in 0..batch_size {
        for query in 0..target_len {
            let window_start = query.saturating_add(1).saturating_sub(window_tokens);
            for key in window_start..=query {
                let index = (batch * target_len + query) * target_len + key;
                allowed[index] = true;
            }
        }
    }
    AttentionMask::new([batch_size, target_len, target_len], allowed).map_err(|error| {
        TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
            case_id: String::from("decoder_sliding_window_mask"),
            detail: error.to_string(),
        }
    })
}

fn decoder_mid_decode_reset_mask(
    batch_size: usize,
    target_len: usize,
) -> Result<AttentionMask, TassadarArticleKvActivationDisciplineAuditError> {
    let reset_at = target_len / 2;
    let mut allowed = vec![false; batch_size * target_len * target_len];
    for batch in 0..batch_size {
        for query in 0..target_len {
            let segment_start = if query >= reset_at { reset_at } else { 0 };
            for key in segment_start..=query {
                let index = (batch * target_len + query) * target_len + key;
                allowed[index] = true;
            }
        }
    }
    AttentionMask::new([batch_size, target_len, target_len], allowed).map_err(|error| {
        TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
            case_id: String::from("decoder_mid_decode_reset_mask"),
            detail: error.to_string(),
        }
    })
}

fn constraint_detail(constraint_kind: TassadarArticleStateConstraintKind) -> String {
    match constraint_kind {
        TassadarArticleStateConstraintKind::SlidingWindowModerate => format!(
            "only the last {MODERATE_HISTORY_WINDOW_TOKENS} target tokens remain visible to each decode step"
        ),
        TassadarArticleStateConstraintKind::SlidingWindowStrict => format!(
            "only the current target token remains visible to each decode step"
        ),
        TassadarArticleStateConstraintKind::MidDecodeReset => String::from(
            "mid-sequence history reset blocks every post-reset decode step from attending to the pre-reset target history",
        ),
    }
}

fn workload_family_id_for_case(case_id: &str) -> String {
    match case_id {
        "micro_wasm_kernel" => String::from("MicroWasmKernel"),
        "branch_heavy_kernel" => String::from("BranchHeavyKernel"),
        "memory_heavy_kernel" => String::from("MemoryHeavyKernel"),
        "long_loop_kernel" => String::from("LongLoopKernel"),
        "sudoku_v0_test_a" => String::from("SudokuClass"),
        "hungarian_matching" => String::from("HungarianMatching"),
        _ => String::from("ArticleGeneric"),
    }
}

fn fixture_execution_for_case(
    case: &TassadarValidationCase,
) -> Result<TassadarExecution, TassadarArticleKvActivationDisciplineAuditError> {
    let runner = TassadarFixtureRunner::for_program(&case.program).map_err(|error| {
        TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
            case_id: case.case_id.clone(),
            detail: error.to_string(),
        }
    })?;
    runner.execute(&case.program).map_err(|error| {
        TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
            case_id: case.case_id.clone(),
            detail: error.to_string(),
        }
    })
}

pub fn tassadar_article_kv_activation_discipline_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF)
}

pub fn write_tassadar_article_kv_activation_discipline_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleKvActivationDisciplineAuditReport,
    TassadarArticleKvActivationDisciplineAuditError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleKvActivationDisciplineAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_kv_activation_discipline_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleKvActivationDisciplineAuditError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn predicted_token_ids_from_logits(
    dims: &[usize],
    data: &TensorData,
    case_id: &str,
) -> Result<Vec<usize>, TassadarArticleKvActivationDisciplineAuditError> {
    if dims.len() != 3 {
        return Err(
            TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
                case_id: String::from(case_id),
                detail: format!("expected logits rank 3, found shape {dims:?}"),
            },
        );
    }
    let batch_size = dims[0];
    let target_len = dims[1];
    let vocab_size = dims[2];
    let values = match data {
        TensorData::F32(values) => values.as_slice(),
        other => {
            return Err(
                TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
                    case_id: String::from(case_id),
                    detail: format!("expected dense f32 logits, found {other:?}"),
                },
            );
        }
    };
    let expected_len = batch_size * target_len * vocab_size;
    if values.len() != expected_len {
        return Err(
            TassadarArticleKvActivationDisciplineAuditError::ConstraintCase {
                case_id: String::from(case_id),
                detail: format!(
                    "logits value count {} does not match shape {dims:?} (expected {expected_len})",
                    values.len()
                ),
            },
        );
    }
    let mut predictions = Vec::with_capacity(batch_size * target_len);
    for position_index in 0..(batch_size * target_len) {
        let offset = position_index * vocab_size;
        let mut best_index = 0usize;
        let mut best_value = values[offset];
        for vocab_index in 1..vocab_size {
            let candidate = values[offset + vocab_index];
            if candidate > best_value {
                best_index = vocab_index;
                best_value = candidate;
            }
        }
        predictions.push(best_index);
    }
    Ok(predictions)
}

fn tensor_data_digest(data: &TensorData) -> String {
    match data {
        TensorData::F32(values) => stable_digest(
            b"psionic_tassadar_article_kv_activation_discipline_tensor_data_f32|",
            values,
        ),
        other => stable_digest(
            b"psionic_tassadar_article_kv_activation_discipline_tensor_data_debug|",
            &format!("{other:?}"),
        ),
    }
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleKvActivationDisciplineAuditError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleKvActivationDisciplineAuditError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleKvActivationDisciplineAuditError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization should succeed"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use tempfile::tempdir;

    use super::{
        build_tassadar_article_kv_activation_discipline_audit_report, read_repo_json,
        tassadar_article_kv_activation_discipline_audit_report_path,
        write_tassadar_article_kv_activation_discipline_audit_report,
        TassadarArticleKvActivationDisciplineAuditReport,
        TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
    };

    fn generated_report() -> &'static TassadarArticleKvActivationDisciplineAuditReport {
        static REPORT: OnceLock<TassadarArticleKvActivationDisciplineAuditReport> = OnceLock::new();
        REPORT.get_or_init(|| {
            build_tassadar_article_kv_activation_discipline_audit_report()
                .expect("kv activation discipline audit report")
        })
    }

    #[test]
    fn article_kv_activation_discipline_audit_tracks_mixed_boundary() {
        let report = generated_report();

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.kv_activation_discipline_green);
        assert!(report.growth_report.cache_growth_scales_with_problem_size);
        assert!(
            report
                .growth_report
                .dynamic_state_exceeds_weight_artifact_bytes
        );
        assert!(
            report
                .sensitivity_review
                .cache_truncation_breaks_correctness
        );
        assert!(report.sensitivity_review.cache_reset_breaks_correctness);
        assert!(
            !report
                .sensitivity_review
                .equivalent_behavior_survives_under_constrained_cache
        );
        assert_eq!(
            report.dominance_verdict.verdict,
            super::TassadarArticleStateDominanceVerdictKind::Mixed
        );
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_kv_activation_discipline_audit_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = generated_report().clone();
        let committed: TassadarArticleKvActivationDisciplineAuditReport = read_repo_json(
            TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
            "article_kv_activation_discipline_audit_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_kv_activation_discipline_audit_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_kv_activation_discipline_audit_report.json");
        let written = write_tassadar_article_kv_activation_discipline_audit_report(&output_path)?;
        let persisted: TassadarArticleKvActivationDisciplineAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_kv_activation_discipline_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_kv_activation_discipline_audit_report.json")
        );
        Ok(())
    }
}
