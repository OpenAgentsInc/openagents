use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{TassadarArticleTransformer, TassadarArticleTransformerError};
use psionic_runtime::{
    tassadar_article_class_corpus, TassadarExactnessPosture, TassadarExactnessRefusalReport,
    TassadarExecution, TassadarExecutorDecodeMode, TassadarExecutorSelectionDiagnostic,
    TassadarExecutorSelectionState, TassadarFixtureRunner, TassadarValidationCase,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_fixture_transformer_parity_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFixtureTransformerParityError,
    TassadarArticleFixtureTransformerParityReport,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_reference_linear_exactness_gate_report.json";
pub const TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-transformer-reference-linear-exactness.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-171A";
const DIRECT_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";
const TRANSFORMER_REFERENCE_LINEAR_RUNTIME_BACKEND: &str =
    "tassadar_article_transformer.reference_linear.v1";
const DIRECT_PROOF_CASE_IDS: [&str; 3] =
    ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DirectProofReceiptView {
    receipt_id: String,
    article_case_id: String,
    model_id: String,
    model_lineage_contract_ref: String,
    requested_decode_mode: TassadarExecutorDecodeMode,
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
    fallback_observed: bool,
    external_call_count: u32,
    cpu_result_substitution_observed: bool,
    trace_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DirectProofReportView {
    report_id: String,
    model_id: String,
    historical_fixture_model_id: String,
    parity_report_ref: String,
    lineage_contract_ref: String,
    lineage_contract_digest: String,
    receipts: Vec<DirectProofReceiptView>,
    direct_case_count: u32,
    fallback_free_case_count: u32,
    zero_external_call_case_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReferenceLinearExactnessAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub parity_report_ref: String,
    pub direct_proof_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReferenceLinearParityReview {
    pub report_ref: String,
    pub replacement_certified: bool,
    pub all_declared_cases_present: bool,
    pub all_cases_pass: bool,
    pub exact_trace_case_count: usize,
    pub exact_output_case_count: usize,
    pub supported_case_count: usize,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReferenceLinearDirectProofReview {
    pub report_ref: String,
    pub report_id: String,
    pub required_case_count: usize,
    pub bound_case_count: usize,
    pub direct_case_count: u32,
    pub fallback_free_case_count: u32,
    pub zero_external_call_case_count: u32,
    pub all_required_cases_present: bool,
    pub model_id_matches: bool,
    pub historical_fixture_model_id_matches: bool,
    pub parity_report_ref_matches: bool,
    pub lineage_contract_ref_matches: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReferenceLinearExactnessCaseRow {
    pub case_id: String,
    pub program_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub trace_step_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub within_transformer_context_window: bool,
    pub direct_model_weight_proof_required: bool,
    pub direct_model_weight_proof_present: bool,
    pub proof_receipt_id: Option<String>,
    pub fixture_trace_digest: String,
    pub transformer_trace_digest: String,
    pub runtime_report: TassadarExactnessRefusalReport,
    pub proof_detail: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReferenceLinearExactnessGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleTransformerReferenceLinearExactnessAcceptanceGateTie,
    pub parity_review: TassadarArticleTransformerReferenceLinearParityReview,
    pub direct_proof_review: TassadarArticleTransformerReferenceLinearDirectProofReview,
    pub transformer_model_id: String,
    pub historical_fixture_model_id: String,
    pub lineage_contract_ref: String,
    pub lineage_contract_digest: String,
    pub case_rows: Vec<TassadarArticleTransformerReferenceLinearExactnessCaseRow>,
    pub declared_case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub within_transformer_context_window_case_count: usize,
    pub direct_model_weight_proof_case_count: usize,
    pub mismatch_case_ids: Vec<String>,
    pub refused_case_ids: Vec<String>,
    pub all_declared_cases_present: bool,
    pub reference_linear_exactness_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerReferenceLinearExactnessGateReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Parity(#[from] TassadarArticleFixtureTransformerParityError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(
        "failed to execute canonical article case `{case_id}` on the fixture baseline: {detail}"
    )]
    FixtureExecution { case_id: String, detail: String },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_transformer_reference_linear_exactness_gate_report() -> Result<
    TassadarArticleTransformerReferenceLinearExactnessGateReport,
    TassadarArticleTransformerReferenceLinearExactnessGateReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let parity_report = build_tassadar_article_fixture_transformer_parity_report()?;
    let direct_proof_report = read_repo_json::<DirectProofReportView>(DIRECT_PROOF_REPORT_REF)?;
    let model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    let case_rows = build_case_rows(&model, &direct_proof_report)?;
    let parity_review = parity_review(&parity_report);
    let direct_proof_review = direct_proof_review(
        &parity_report,
        &direct_proof_report,
        case_rows.as_slice(),
        model.descriptor().model.model_id.as_str(),
    );
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        &parity_report,
        parity_review,
        direct_proof_review,
        case_rows,
        model.descriptor().model.model_id.clone(),
        direct_proof_report.lineage_contract_ref,
        direct_proof_report.lineage_contract_digest,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    parity_report: &TassadarArticleFixtureTransformerParityReport,
    parity_review: TassadarArticleTransformerReferenceLinearParityReview,
    direct_proof_review: TassadarArticleTransformerReferenceLinearDirectProofReview,
    case_rows: Vec<TassadarArticleTransformerReferenceLinearExactnessCaseRow>,
    transformer_model_id: String,
    lineage_contract_ref: String,
    lineage_contract_digest: String,
) -> TassadarArticleTransformerReferenceLinearExactnessGateReport {
    let acceptance_gate_tie = TassadarArticleTransformerReferenceLinearExactnessAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        parity_report_ref: String::from(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF),
        direct_proof_report_ref: String::from(DIRECT_PROOF_REPORT_REF),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let declared_case_ids = tassadar_article_class_corpus()
        .into_iter()
        .map(|case| case.case_id)
        .collect::<BTreeSet<_>>();
    let observed_case_ids = case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<BTreeSet<_>>();
    let declared_case_count = declared_case_ids.len();
    let exact_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact)
        .count();
    let mismatch_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .count();
    let refused_case_count = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .count();
    let within_transformer_context_window_case_count = case_rows
        .iter()
        .filter(|row| row.within_transformer_context_window)
        .count();
    let direct_model_weight_proof_case_count = case_rows
        .iter()
        .filter(|row| row.direct_model_weight_proof_present)
        .count();
    let mismatch_case_ids = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Mismatch)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let refused_case_ids = case_rows
        .iter()
        .filter(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Refused)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let all_declared_cases_present = !case_rows.is_empty()
        && declared_case_ids == observed_case_ids
        && case_rows.len() == declared_case_count;
    let reference_linear_exactness_green = acceptance_gate_tie.tied_requirement_satisfied
        && parity_review.passed
        && direct_proof_review.passed
        && all_declared_cases_present
        && exact_case_count == declared_case_count
        && mismatch_case_count == 0
        && refused_case_count == 0;
    let article_equivalence_green =
        reference_linear_exactness_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerReferenceLinearExactnessGateReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.article_transformer.reference_linear_exactness_gate.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_CHECKER_REF,
        ),
        acceptance_gate_tie,
        parity_review,
        direct_proof_review,
        transformer_model_id,
        historical_fixture_model_id: parity_report.fixture_model_id.clone(),
        lineage_contract_ref,
        lineage_contract_digest,
        case_rows,
        declared_case_count,
        exact_case_count,
        mismatch_case_count,
        refused_case_count,
        within_transformer_context_window_case_count,
        direct_model_weight_proof_case_count,
        mismatch_case_ids,
        refused_case_ids,
        all_declared_cases_present,
        reference_linear_exactness_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this gate certifies only that the owned Transformer-backed reference-linear route now stays exact on the declared article workload family under the current trace-domain route, with explicit exact/mismatch/refused rows and explicit direct-proof binding for the bounded proof family. It does not yet claim anti-memorization closure, contamination independence, fast-route promotion, benchmark parity, single-run closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Transformer-backed reference-linear exactness gate now records declared_case_count={}, exact_case_count={}, mismatch_case_count={}, refused_case_count={}, direct_model_weight_proof_case_count={}, reference_linear_exactness_green={}, and article_equivalence_green={}.",
        report.declared_case_count,
        report.exact_case_count,
        report.mismatch_case_count,
        report.refused_case_count,
        report.direct_model_weight_proof_case_count,
        report.reference_linear_exactness_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_reference_linear_exactness_gate_report|",
        &report,
    );
    report
}

fn build_case_rows(
    model: &TassadarArticleTransformer,
    direct_proof_report: &DirectProofReportView,
) -> Result<
    Vec<TassadarArticleTransformerReferenceLinearExactnessCaseRow>,
    TassadarArticleTransformerReferenceLinearExactnessGateReportError,
> {
    tassadar_article_class_corpus()
        .into_iter()
        .map(|case| build_case_row(&case, model, direct_proof_report))
        .collect()
}

fn build_case_row(
    case: &TassadarValidationCase,
    model: &TassadarArticleTransformer,
    direct_proof_report: &DirectProofReportView,
) -> Result<
    TassadarArticleTransformerReferenceLinearExactnessCaseRow,
    TassadarArticleTransformerReferenceLinearExactnessGateReportError,
> {
    let fixture_execution = fixture_execution_for_case(case)?;
    let fixture_trace_digest = fixture_execution.trace_digest();
    let direct_model_weight_proof_required = DIRECT_PROOF_CASE_IDS.contains(&case.case_id.as_str());
    let matching_receipt = direct_proof_report
        .receipts
        .iter()
        .find(|receipt| receipt.article_case_id == case.case_id);

    match model.roundtrip_article_trace_domain_unbounded(&case.program, &fixture_execution) {
        Ok(roundtrip) => {
            let transformer_execution = roundtrip.decoded_trace.materialize_execution(
                fixture_execution.program_id.clone(),
                fixture_execution.profile_id.clone(),
                String::from(TRANSFORMER_REFERENCE_LINEAR_RUNTIME_BACKEND),
                fixture_execution.trace_abi.clone(),
            );
            let runtime_report = TassadarExactnessRefusalReport::from_selection_and_execution(
                exactness_subject_id(case.case_id.as_str()),
                &direct_reference_linear_selection(
                    case,
                    fixture_execution.trace_abi.schema_version,
                ),
                &fixture_execution,
                &transformer_execution,
            );
            let within_transformer_context_window = within_transformer_context_window(
                model,
                roundtrip.batch.source_token_ids.len(),
                roundtrip.batch.target_token_ids.len(),
            );
            let (direct_model_weight_proof_present, proof_receipt_id, proof_detail) =
                evaluate_direct_proof_binding(
                    direct_model_weight_proof_required,
                    matching_receipt,
                    direct_proof_report,
                    model.descriptor().model.model_id.as_str(),
                    fixture_trace_digest.as_str(),
                );
            let detail = if runtime_report.exactness_posture == TassadarExactnessPosture::Exact {
                if direct_model_weight_proof_required {
                    format!(
                        "Transformer-backed reference-linear route stayed exact on `{}` and direct-proof binding status is: {}",
                        case.case_id, proof_detail
                    )
                } else {
                    format!(
                        "Transformer-backed reference-linear route stayed exact on `{}`; direct model-weight proof remains intentionally limited to the bounded proof family",
                        case.case_id
                    )
                }
            } else {
                runtime_report.detail.clone()
            };
            Ok(TassadarArticleTransformerReferenceLinearExactnessCaseRow {
                case_id: case.case_id.clone(),
                program_id: case.program.program_id.clone(),
                requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                trace_step_count: fixture_execution.steps.len(),
                prompt_token_count: roundtrip.batch.prompt_token_count,
                target_token_count: roundtrip.batch.target_token_count,
                within_transformer_context_window,
                direct_model_weight_proof_required,
                direct_model_weight_proof_present,
                proof_receipt_id,
                fixture_trace_digest,
                transformer_trace_digest: transformer_execution.trace_digest(),
                runtime_report,
                proof_detail,
                detail,
            })
        }
        Err(error) => {
            let runtime_report = TassadarExactnessRefusalReport::from_refusal(
                exactness_subject_id(case.case_id.as_str()),
                &refused_reference_linear_selection(
                    case,
                    fixture_execution.trace_abi.schema_version,
                    error.to_string(),
                ),
                None,
            );
            let (direct_model_weight_proof_present, proof_receipt_id, proof_detail) =
                evaluate_direct_proof_binding(
                    direct_model_weight_proof_required,
                    matching_receipt,
                    direct_proof_report,
                    model.descriptor().model.model_id.as_str(),
                    fixture_trace_digest.as_str(),
                );
            Ok(TassadarArticleTransformerReferenceLinearExactnessCaseRow {
                case_id: case.case_id.clone(),
                program_id: case.program.program_id.clone(),
                requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
                trace_step_count: fixture_execution.steps.len(),
                prompt_token_count: 0,
                target_token_count: 0,
                within_transformer_context_window: false,
                direct_model_weight_proof_required,
                direct_model_weight_proof_present,
                proof_receipt_id,
                fixture_trace_digest,
                transformer_trace_digest: String::new(),
                runtime_report: runtime_report.clone(),
                proof_detail,
                detail: runtime_report.detail,
            })
        }
    }
}

fn parity_review(
    parity_report: &TassadarArticleFixtureTransformerParityReport,
) -> TassadarArticleTransformerReferenceLinearParityReview {
    let passed = parity_report.replacement_certified
        && parity_report.all_declared_cases_present
        && parity_report.all_cases_pass
        && parity_report.exact_trace_case_count == parity_report.supported_case_count
        && parity_report.exact_output_case_count == parity_report.supported_case_count;
    let detail = format!(
        "replacement_certified={} all_declared_cases_present={} all_cases_pass={} exact_trace_case_count={}/{} exact_output_case_count={}/{}",
        parity_report.replacement_certified,
        parity_report.all_declared_cases_present,
        parity_report.all_cases_pass,
        parity_report.exact_trace_case_count,
        parity_report.supported_case_count,
        parity_report.exact_output_case_count,
        parity_report.supported_case_count,
    );
    TassadarArticleTransformerReferenceLinearParityReview {
        report_ref: String::from(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF),
        replacement_certified: parity_report.replacement_certified,
        all_declared_cases_present: parity_report.all_declared_cases_present,
        all_cases_pass: parity_report.all_cases_pass,
        exact_trace_case_count: parity_report.exact_trace_case_count,
        exact_output_case_count: parity_report.exact_output_case_count,
        supported_case_count: parity_report.supported_case_count,
        passed,
        detail,
    }
}

fn direct_proof_review(
    parity_report: &TassadarArticleFixtureTransformerParityReport,
    direct_proof_report: &DirectProofReportView,
    case_rows: &[TassadarArticleTransformerReferenceLinearExactnessCaseRow],
    transformer_model_id: &str,
) -> TassadarArticleTransformerReferenceLinearDirectProofReview {
    let required_case_count = DIRECT_PROOF_CASE_IDS.len();
    let required_case_ids = DIRECT_PROOF_CASE_IDS
        .iter()
        .map(|case_id| String::from(*case_id))
        .collect::<BTreeSet<_>>();
    let bound_case_count = case_rows
        .iter()
        .filter(|row| {
            row.direct_model_weight_proof_required && row.direct_model_weight_proof_present
        })
        .count();
    let present_case_ids = direct_proof_report
        .receipts
        .iter()
        .map(|receipt| receipt.article_case_id.clone())
        .collect::<BTreeSet<_>>();
    let all_required_cases_present = required_case_ids.is_subset(&present_case_ids);
    let model_id_matches = direct_proof_report.model_id == transformer_model_id
        && direct_proof_report
            .receipts
            .iter()
            .all(|receipt| receipt.model_id == transformer_model_id);
    let historical_fixture_model_id_matches =
        direct_proof_report.historical_fixture_model_id == parity_report.fixture_model_id;
    let parity_report_ref_matches = direct_proof_report.parity_report_ref
        == TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF;
    let lineage_contract_ref_matches =
        direct_proof_report.lineage_contract_ref == parity_report.lineage_contract_ref;
    let passed = bound_case_count == required_case_count
        && all_required_cases_present
        && model_id_matches
        && historical_fixture_model_id_matches
        && parity_report_ref_matches
        && lineage_contract_ref_matches
        && direct_proof_report.direct_case_count == required_case_count as u32
        && direct_proof_report.fallback_free_case_count == required_case_count as u32
        && direct_proof_report.zero_external_call_case_count == required_case_count as u32;
    let detail = format!(
        "required_case_count={} bound_case_count={} all_required_cases_present={} direct_case_count={} fallback_free_case_count={} zero_external_call_case_count={} model_id_matches={} historical_fixture_model_id_matches={} parity_report_ref_matches={} lineage_contract_ref_matches={}",
        required_case_count,
        bound_case_count,
        all_required_cases_present,
        direct_proof_report.direct_case_count,
        direct_proof_report.fallback_free_case_count,
        direct_proof_report.zero_external_call_case_count,
        model_id_matches,
        historical_fixture_model_id_matches,
        parity_report_ref_matches,
        lineage_contract_ref_matches,
    );
    TassadarArticleTransformerReferenceLinearDirectProofReview {
        report_ref: String::from(DIRECT_PROOF_REPORT_REF),
        report_id: direct_proof_report.report_id.clone(),
        required_case_count,
        bound_case_count,
        direct_case_count: direct_proof_report.direct_case_count,
        fallback_free_case_count: direct_proof_report.fallback_free_case_count,
        zero_external_call_case_count: direct_proof_report.zero_external_call_case_count,
        all_required_cases_present,
        model_id_matches,
        historical_fixture_model_id_matches,
        parity_report_ref_matches,
        lineage_contract_ref_matches,
        passed,
        detail,
    }
}

fn evaluate_direct_proof_binding(
    required: bool,
    receipt: Option<&DirectProofReceiptView>,
    direct_proof_report: &DirectProofReportView,
    transformer_model_id: &str,
    expected_trace_digest: &str,
) -> (bool, Option<String>, String) {
    let Some(receipt) = receipt else {
        if required {
            return (
                false,
                None,
                String::from("required direct model-weight proof receipt is missing"),
            );
        }
        return (
            false,
            None,
            String::from("no direct model-weight proof receipt is required for this case"),
        );
    };

    let mut mismatches = Vec::new();
    if receipt.model_id != transformer_model_id {
        mismatches.push(format!(
            "receipt model `{}` did not match Transformer model `{}`",
            receipt.model_id, transformer_model_id
        ));
    }
    if receipt.model_id != direct_proof_report.model_id {
        mismatches.push(format!(
            "receipt model `{}` did not match direct-proof report model `{}`",
            receipt.model_id, direct_proof_report.model_id
        ));
    }
    if receipt.model_lineage_contract_ref != direct_proof_report.lineage_contract_ref {
        mismatches.push(format!(
            "receipt lineage `{}` did not match direct-proof report lineage `{}`",
            receipt.model_lineage_contract_ref, direct_proof_report.lineage_contract_ref
        ));
    }
    if receipt.requested_decode_mode != TassadarExecutorDecodeMode::ReferenceLinear {
        mismatches.push(format!(
            "receipt requested decode `{}` was not reference_linear",
            receipt.requested_decode_mode.as_str()
        ));
    }
    if receipt.effective_decode_mode != Some(TassadarExecutorDecodeMode::ReferenceLinear) {
        mismatches.push(format!(
            "receipt effective decode was `{}` instead of reference_linear",
            receipt
                .effective_decode_mode
                .map_or("none", TassadarExecutorDecodeMode::as_str)
        ));
    }
    if receipt.selection_state != TassadarExecutorSelectionState::Direct {
        mismatches.push(format!(
            "receipt selection state `{:?}` was not direct",
            receipt.selection_state
        ));
    }
    if receipt.fallback_observed {
        mismatches.push(String::from("receipt observed fallback"));
    }
    if receipt.external_call_count != 0 {
        mismatches.push(format!(
            "receipt external_call_count={} was not zero",
            receipt.external_call_count
        ));
    }
    if receipt.cpu_result_substitution_observed {
        mismatches.push(String::from(
            "receipt observed CPU result substitution on the proof lane",
        ));
    }
    if receipt.trace_digest != expected_trace_digest {
        mismatches.push(format!(
            "receipt trace digest `{}` did not match expected exactness trace `{}`",
            receipt.trace_digest, expected_trace_digest
        ));
    }

    if mismatches.is_empty() {
        (
            true,
            Some(receipt.receipt_id.clone()),
            format!(
                "direct model-weight proof receipt `{}` matches the Transformer exactness row",
                receipt.receipt_id
            ),
        )
    } else {
        (
            false,
            Some(receipt.receipt_id.clone()),
            mismatches.join("; "),
        )
    }
}

fn exactness_subject_id(case_id: &str) -> String {
    format!("tassadar.article_transformer.reference_linear_exactness.{case_id}")
}

fn direct_reference_linear_selection(
    case: &TassadarValidationCase,
    trace_abi_version: u16,
) -> TassadarExecutorSelectionDiagnostic {
    TassadarExecutorSelectionDiagnostic {
        program_id: case.program.program_id.clone(),
        runtime_backend: String::from(TRANSFORMER_REFERENCE_LINEAR_RUNTIME_BACKEND),
        requested_profile_id: case.program.profile_id.clone(),
        requested_trace_abi_version: trace_abi_version,
        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
        selection_state: TassadarExecutorSelectionState::Direct,
        selection_reason: None,
        detail: String::from(
            "reference-linear Transformer-backed exactness lane stayed direct on the declared article case",
        ),
        model_supported_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
    }
}

fn refused_reference_linear_selection(
    case: &TassadarValidationCase,
    trace_abi_version: u16,
    detail: String,
) -> TassadarExecutorSelectionDiagnostic {
    TassadarExecutorSelectionDiagnostic {
        program_id: case.program.program_id.clone(),
        runtime_backend: String::from(TRANSFORMER_REFERENCE_LINEAR_RUNTIME_BACKEND),
        requested_profile_id: case.program.profile_id.clone(),
        requested_trace_abi_version: trace_abi_version,
        requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        effective_decode_mode: None,
        selection_state: TassadarExecutorSelectionState::Refused,
        selection_reason: None,
        detail,
        model_supported_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
    }
}

fn within_transformer_context_window(
    model: &TassadarArticleTransformer,
    source_token_count: usize,
    target_token_count: usize,
) -> bool {
    source_token_count <= model.descriptor().config.max_source_positions
        && target_token_count <= model.descriptor().config.max_target_positions
}

fn fixture_execution_for_case(
    case: &TassadarValidationCase,
) -> Result<TassadarExecution, TassadarArticleTransformerReferenceLinearExactnessGateReportError> {
    let runner = TassadarFixtureRunner::for_program(&case.program).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::FixtureExecution {
            case_id: case.case_id.clone(),
            detail: error.to_string(),
        }
    })?;
    runner.execute(&case.program).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::FixtureExecution {
            case_id: case.case_id.clone(),
            detail: error.to_string(),
        }
    })
}

pub fn tassadar_article_transformer_reference_linear_exactness_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF)
}

pub fn write_tassadar_article_transformer_reference_linear_exactness_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerReferenceLinearExactnessGateReport,
    TassadarArticleTransformerReferenceLinearExactnessGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerReferenceLinearExactnessGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_reference_linear_exactness_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarArticleTransformerReferenceLinearExactnessGateReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleTransformerReferenceLinearExactnessGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_case_rows, build_report_from_inputs,
        build_tassadar_article_transformer_reference_linear_exactness_gate_report,
        direct_proof_review, parity_review, read_json,
        tassadar_article_transformer_reference_linear_exactness_gate_report_path,
        write_tassadar_article_transformer_reference_linear_exactness_gate_report,
        DirectProofReportView, TassadarArticleTransformerReferenceLinearExactnessGateReport,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_article_fixture_transformer_parity_report,
    };
    use psionic_models::TassadarArticleTransformer;
    use psionic_runtime::TassadarExactnessPosture;

    #[test]
    fn reference_linear_exactness_gate_is_green_without_final_article_equivalence() {
        let report = build_tassadar_article_transformer_reference_linear_exactness_gate_report()
            .expect("report");

        assert_eq!(report.declared_case_count, 13);
        assert_eq!(report.exact_case_count, 13);
        assert_eq!(report.mismatch_case_count, 0);
        assert_eq!(report.refused_case_count, 0);
        assert_eq!(report.within_transformer_context_window_case_count, 4);
        assert_eq!(report.direct_model_weight_proof_case_count, 3);
        assert!(report.reference_linear_exactness_green);
        assert!(report.article_equivalence_green);
        assert!(report.parity_review.passed);
        assert!(report.direct_proof_review.passed);
        assert!(report
            .case_rows
            .iter()
            .all(|row| row.runtime_report.exactness_posture == TassadarExactnessPosture::Exact));
    }

    #[test]
    fn reference_linear_exactness_gate_turns_red_on_case_mismatch() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let parity_report =
            build_tassadar_article_fixture_transformer_parity_report().expect("parity report");
        let direct_proof_report: DirectProofReportView = super::read_repo_json(
            "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json",
        )
        .expect("direct proof report");
        let model = TassadarArticleTransformer::trained_trace_domain_reference().expect("model");
        let mut case_rows =
            build_case_rows(&model, &direct_proof_report).expect("exactness case rows");
        let row = case_rows.first_mut().expect("first case row");
        row.runtime_report.exactness_posture = TassadarExactnessPosture::Mismatch;
        row.runtime_report.trace_digest_equal = false;
        row.detail = String::from("synthetic mismatch for gate red-path coverage");

        let report = build_report_from_inputs(
            acceptance_gate_report,
            &parity_report,
            parity_review(&parity_report),
            direct_proof_review(
                &parity_report,
                &direct_proof_report,
                case_rows.as_slice(),
                model.descriptor().model.model_id.as_str(),
            ),
            case_rows,
            model.descriptor().model.model_id.clone(),
            direct_proof_report.lineage_contract_ref.clone(),
            direct_proof_report.lineage_contract_digest.clone(),
        );

        assert_eq!(report.exact_case_count, 12);
        assert_eq!(report.mismatch_case_count, 1);
        assert!(!report.reference_linear_exactness_green);
        assert_eq!(report.mismatch_case_ids.len(), 1);
    }

    #[test]
    fn reference_linear_exactness_gate_matches_committed_truth() {
        let generated = build_tassadar_article_transformer_reference_linear_exactness_gate_report()
            .expect("report");
        let committed: TassadarArticleTransformerReferenceLinearExactnessGateReport =
            read_json(tassadar_article_transformer_reference_linear_exactness_gate_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_reference_linear_exactness_gate_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_reference_linear_exactness_gate_report.json");
        let written =
            write_tassadar_article_transformer_reference_linear_exactness_gate_report(&output_path)
                .expect("write report");
        let persisted: TassadarArticleTransformerReferenceLinearExactnessGateReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_reference_linear_exactness_gate_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_reference_linear_exactness_gate_report.json")
        );
        assert_eq!(written.report_id, persisted.report_id);
    }
}
