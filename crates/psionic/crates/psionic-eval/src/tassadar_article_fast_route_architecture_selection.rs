use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_environments::TassadarWorkloadTarget;
use psionic_models::{TassadarArticleTransformer, TassadarWorkloadClass};
use psionic_router::{
    negotiate_tassadar_planner_executor_route, TassadarPlannerExecutorDecodeCapability,
    TassadarPlannerExecutorNegotiatedRouteState, TassadarPlannerExecutorRouteCandidate,
    TassadarPlannerExecutorRouteDescriptor, TassadarPlannerExecutorRouteNegotiationOutcome,
    TassadarPlannerExecutorRouteNegotiationRequest, TassadarPlannerExecutorRoutePosture,
    TassadarPlannerExecutorRouteRefusalReason, TassadarPlannerExecutorWasmCapabilityMatrix,
    TassadarPlannerExecutorWasmCapabilityRow, TassadarPlannerExecutorWasmImportPosture,
    TassadarPlannerExecutorWasmOpcodeFamily,
};
use psionic_runtime::{
    TassadarClaimClass, TassadarExecutorDecodeMode, TassadarRecurrentFastPathRuntimeBaselineReport,
    TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarEfficientAttentionBaselineFamilyKind,
    TassadarEfficientAttentionBaselineMatrixReport, TassadarHullCacheClosureReport,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
    TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_architecture_selection_report.json";
pub const TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_CHECKER_REF: &str =
    "scripts/check-tassadar-article-fast-route-architecture-selection.sh";

const REPORT_SCHEMA_VERSION: u16 = 1;
const TIED_REQUIREMENT_ID: &str = "TAS-172";
const DIRECT_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";
const TWO_DIMENSIONAL_HEAD_RESEARCH_REF: &str =
    "crates/psionic-eval/src/tassadar_executor_architecture_comparison.rs";
const ARTICLE_INTERNAL_COMPUTE_PROFILE_ID: &str = "tassadar.internal_compute.article_closeout.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFastRouteCandidateKind {
    HullCacheRuntime,
    LinearRecurrentRuntime,
    HierarchicalHullResearch,
    TwoDimensionalHeadHardMaxResearch,
}

impl TassadarArticleFastRouteCandidateKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::HullCacheRuntime => "hull_cache_runtime",
            Self::LinearRecurrentRuntime => "linear_recurrent_runtime",
            Self::HierarchicalHullResearch => "hierarchical_hull_research",
            Self::TwoDimensionalHeadHardMaxResearch => "two_dimensional_head_hard_max_research",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteSelectionAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteTransformerModelRouteAnchorReview {
    pub proof_report_ref: String,
    pub model_id: String,
    pub benchmark_report_ref: String,
    pub route_descriptor_digest: String,
    pub direct_case_count: u32,
    pub fallback_free_case_count: u32,
    pub zero_external_call_case_count: u32,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessMatrixRow {
    pub candidate_kind: TassadarArticleFastRouteCandidateKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_target: Option<TassadarWorkloadTarget>,
    pub evidence_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_class: Option<TassadarClaimClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_case_count: Option<u32>,
    pub all_cases_exact: bool,
    pub evidence_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteDirectFallbackMatrixRow {
    pub candidate_kind: TassadarArticleFastRouteCandidateKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_target: Option<TassadarWorkloadTarget>,
    pub evidence_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_case_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_case_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused_case_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub evidence_ref: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFastRouteRouteabilityEvidenceKind {
    ProjectedPlannerRouteDescriptor,
    DecodeModeMissingFromCanonicalRouteContract,
    BoundedResearchWindowOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteModuleClassRouteRow {
    pub module_class: TassadarWorkloadClass,
    pub routeable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negotiated_route_state: Option<TassadarPlannerExecutorNegotiatedRouteState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteRouteabilityCheck {
    pub candidate_kind: TassadarArticleFastRouteCandidateKind,
    pub evidence_kind: TassadarArticleFastRouteRouteabilityEvidenceKind,
    pub transformer_model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_decode_mode: Option<TassadarExecutorDecodeMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate_route_posture: Option<TassadarPlannerExecutorRoutePosture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_route_descriptor_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_report_ref: Option<String>,
    pub module_class_rows: Vec<TassadarArticleFastRouteModuleClassRouteRow>,
    pub routeable: bool,
    pub direct_module_class_count: usize,
    pub fallback_module_class_count: usize,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteCandidateVerdict {
    pub candidate_kind: TassadarArticleFastRouteCandidateKind,
    pub article_scale_exactness_green: bool,
    pub explicit_fallback_boundary: bool,
    pub routeability_green: bool,
    pub promoted_article_lane: bool,
    pub selected: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteArchitectureSelectionReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleFastRouteSelectionAcceptanceGateTie,
    pub transformer_model_route_anchor_review:
        TassadarArticleFastRouteTransformerModelRouteAnchorReview,
    pub exactness_matrix_rows: Vec<TassadarArticleFastRouteExactnessMatrixRow>,
    pub direct_fallback_matrix_rows: Vec<TassadarArticleFastRouteDirectFallbackMatrixRow>,
    pub routeability_checks: Vec<TassadarArticleFastRouteRouteabilityCheck>,
    pub candidate_verdicts: Vec<TassadarArticleFastRouteCandidateVerdict>,
    pub selected_candidate_kind: TassadarArticleFastRouteCandidateKind,
    pub fast_route_selection_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DirectModelWeightExecutionProofReportView {
    model_id: String,
    benchmark_report_ref: String,
    route_descriptor_digest: String,
    direct_case_count: u32,
    fallback_free_case_count: u32,
    zero_external_call_case_count: u32,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteArchitectureSelectionError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
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
    #[error("internal TAS-172 routeability invariant failed: {detail}")]
    Invariant { detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_fast_route_architecture_selection_report() -> Result<
    TassadarArticleFastRouteArchitectureSelectionReport,
    TassadarArticleFastRouteArchitectureSelectionError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let efficient_attention_matrix: TassadarEfficientAttentionBaselineMatrixReport = read_artifact(
        TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
        "efficient_attention_baseline_matrix",
    )?;
    let hull_cache_closure: TassadarHullCacheClosureReport =
        read_artifact(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF, "hull_cache_closure")?;
    let recurrent_runtime_report: TassadarRecurrentFastPathRuntimeBaselineReport = read_artifact(
        TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
        "recurrent_fast_path_runtime_baseline",
    )?;
    let direct_proof_report: DirectModelWeightExecutionProofReportView = read_artifact(
        DIRECT_PROOF_REPORT_REF,
        "direct_model_weight_execution_proof",
    )?;

    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate);
    let transformer_model_route_anchor_review =
        build_transformer_model_route_anchor_review(&direct_proof_report);
    let exactness_matrix_rows = build_exactness_matrix_rows(&efficient_attention_matrix);
    let direct_fallback_matrix_rows =
        build_direct_fallback_matrix_rows(&efficient_attention_matrix);
    let routeability_checks = build_routeability_checks(
        &direct_proof_report,
        &hull_cache_closure,
        &recurrent_runtime_report,
    )?;
    let candidate_verdicts = build_candidate_verdicts(
        exactness_matrix_rows.as_slice(),
        direct_fallback_matrix_rows.as_slice(),
        routeability_checks.as_slice(),
    );
    let selected_candidate_kind = candidate_verdicts
        .iter()
        .find(|verdict| verdict.selected)
        .map(|verdict| verdict.candidate_kind)
        .unwrap_or(TassadarArticleFastRouteCandidateKind::HullCacheRuntime);
    let fast_route_selection_green = candidate_verdicts.iter().filter(|row| row.selected).count()
        == 1
        && selected_candidate_kind == TassadarArticleFastRouteCandidateKind::HullCacheRuntime;

    let routeable_candidate_count = candidate_verdicts
        .iter()
        .filter(|row| row.routeability_green)
        .count();
    let exact_article_matrix_candidate_count = candidate_verdicts
        .iter()
        .filter(|row| row.article_scale_exactness_green)
        .count();
    let selected_routeability = routeability_checks
        .iter()
        .find(|row| row.candidate_kind == selected_candidate_kind)
        .expect("selected candidate should have routeability check");
    let selected_direct_module_class_count = selected_routeability.direct_module_class_count;
    let selected_fallback_module_class_count = selected_routeability.fallback_module_class_count;

    let mut report = TassadarArticleFastRouteArchitectureSelectionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.article_fast_route_architecture_selection.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_CHECKER_REF,
        ),
        acceptance_gate_tie,
        transformer_model_route_anchor_review,
        exactness_matrix_rows,
        direct_fallback_matrix_rows,
        routeability_checks,
        candidate_verdicts,
        selected_candidate_kind,
        fast_route_selection_green,
        article_equivalence_green: acceptance_gate.article_equivalence_green,
        claim_boundary: String::from(
            "this report selects exactly one fast architecture for the canonical article route without pretending that the fast path is already integrated into the owned Transformer-backed model. It keeps same-harness exactness, direct-versus-fallback posture, route-contract fit, promoted runtime truth, and research-only candidates separate so TAS-173 through TAS-175 can close the integration, no-fallback, and throughput tranches explicitly rather than collapsing them into one weak headline.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article fast-route architecture selection now chooses `{}` as the canonical fast article route with routeable_candidate_count={}, exact_article_matrix_candidate_count={}, selected_direct_module_class_count={}, selected_fallback_module_class_count={}, tied_requirement_satisfied={}, and article_equivalence_green={}.",
        report.selected_candidate_kind.label(),
        routeable_candidate_count,
        exact_article_matrix_candidate_count,
        selected_direct_module_class_count,
        selected_fallback_module_class_count,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_fast_route_architecture_selection_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_fast_route_architecture_selection_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_architecture_selection_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFastRouteArchitectureSelectionReport,
    TassadarArticleFastRouteArchitectureSelectionError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteArchitectureSelectionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_fast_route_architecture_selection_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteArchitectureSelectionError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> TassadarArticleFastRouteSelectionAcceptanceGateTie {
    TassadarArticleFastRouteSelectionAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .contains(&String::from(TIED_REQUIREMENT_ID)),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    }
}

fn build_transformer_model_route_anchor_review(
    direct_proof_report: &DirectModelWeightExecutionProofReportView,
) -> TassadarArticleFastRouteTransformerModelRouteAnchorReview {
    let passed = direct_proof_report.model_id
        == TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        && !direct_proof_report.route_descriptor_digest.is_empty()
        && direct_proof_report.direct_case_count == 3
        && direct_proof_report.fallback_free_case_count == 3
        && direct_proof_report.zero_external_call_case_count == 3;
    TassadarArticleFastRouteTransformerModelRouteAnchorReview {
        proof_report_ref: String::from(DIRECT_PROOF_REPORT_REF),
        model_id: direct_proof_report.model_id.clone(),
        benchmark_report_ref: direct_proof_report.benchmark_report_ref.clone(),
        route_descriptor_digest: direct_proof_report.route_descriptor_digest.clone(),
        direct_case_count: direct_proof_report.direct_case_count,
        fallback_free_case_count: direct_proof_report.fallback_free_case_count,
        zero_external_call_case_count: direct_proof_report.zero_external_call_case_count,
        passed,
        detail: format!(
            "the owned Transformer-backed model `{}` already owns one benchmark-gated planner route anchor via the direct-proof report, so TAS-172 can evaluate fast-route fit against a real route identity before TAS-173 integrates the selected fast path into that same owned model boundary",
            direct_proof_report.model_id,
        ),
    }
}

fn build_exactness_matrix_rows(
    efficient_attention_matrix: &TassadarEfficientAttentionBaselineMatrixReport,
) -> Vec<TassadarArticleFastRouteExactnessMatrixRow> {
    let mut rows = Vec::new();
    for matrix_row in &efficient_attention_matrix.rows {
        rows.push(build_exactness_row_for_family(
            efficient_attention_matrix,
            matrix_row.workload_target,
            TassadarArticleFastRouteCandidateKind::HullCacheRuntime,
            TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime,
        ));
        rows.push(build_exactness_row_for_family(
            efficient_attention_matrix,
            matrix_row.workload_target,
            TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime,
            TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime,
        ));
        rows.push(build_exactness_row_for_family(
            efficient_attention_matrix,
            matrix_row.workload_target,
            TassadarArticleFastRouteCandidateKind::HierarchicalHullResearch,
            TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch,
        ));
    }
    rows.push(TassadarArticleFastRouteExactnessMatrixRow {
        candidate_kind: TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch,
        workload_target: None,
        evidence_available: false,
        claim_class: None,
        case_count: None,
        exact_case_count: None,
        all_cases_exact: false,
        evidence_ref: String::from(TWO_DIMENSIONAL_HEAD_RESEARCH_REF),
        detail: String::from(
            "the repo only carries the 2D-head hard-max family as a bounded windowed research lane with hull fallback; no committed article-class exactness matrix exists for it yet, so TAS-172 keeps it explicit but disqualifies it from canonical fast-route selection",
        ),
    });
    rows
}

fn build_exactness_row_for_family(
    efficient_attention_matrix: &TassadarEfficientAttentionBaselineMatrixReport,
    workload_target: TassadarWorkloadTarget,
    candidate_kind: TassadarArticleFastRouteCandidateKind,
    family_kind: TassadarEfficientAttentionBaselineFamilyKind,
) -> TassadarArticleFastRouteExactnessMatrixRow {
    let matrix_row = efficient_attention_matrix
        .rows
        .iter()
        .find(|row| row.workload_target == workload_target)
        .expect("matrix row should exist");
    let cell = matrix_row
        .cells
        .iter()
        .find(|cell| cell.family_kind == family_kind)
        .expect("family cell should exist");
    TassadarArticleFastRouteExactnessMatrixRow {
        candidate_kind,
        workload_target: Some(workload_target),
        evidence_available: true,
        claim_class: Some(cell.claim_class),
        case_count: Some(cell.case_count),
        exact_case_count: Some(cell.exact_case_count),
        all_cases_exact: cell.exact_case_count == cell.case_count && cell.refused_case_count == 0,
        evidence_ref: cell.artifact_ref.clone(),
        detail: format!(
            "{} exactness row for {:?}: exact_case_count={} of case_count={} with note `{}`",
            candidate_kind.label(),
            workload_target,
            cell.exact_case_count,
            cell.case_count,
            cell.note,
        ),
    }
}

fn build_direct_fallback_matrix_rows(
    efficient_attention_matrix: &TassadarEfficientAttentionBaselineMatrixReport,
) -> Vec<TassadarArticleFastRouteDirectFallbackMatrixRow> {
    let mut rows = Vec::new();
    for matrix_row in &efficient_attention_matrix.rows {
        rows.push(build_direct_fallback_row_for_family(
            efficient_attention_matrix,
            matrix_row.workload_target,
            TassadarArticleFastRouteCandidateKind::HullCacheRuntime,
            TassadarEfficientAttentionBaselineFamilyKind::HullCacheRuntime,
        ));
        rows.push(build_direct_fallback_row_for_family(
            efficient_attention_matrix,
            matrix_row.workload_target,
            TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime,
            TassadarEfficientAttentionBaselineFamilyKind::LinearRecurrentRuntime,
        ));
        rows.push(build_direct_fallback_row_for_family(
            efficient_attention_matrix,
            matrix_row.workload_target,
            TassadarArticleFastRouteCandidateKind::HierarchicalHullResearch,
            TassadarEfficientAttentionBaselineFamilyKind::HierarchicalHullResearch,
        ));
    }
    rows.push(TassadarArticleFastRouteDirectFallbackMatrixRow {
        candidate_kind: TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch,
        workload_target: None,
        evidence_available: false,
        direct_case_count: None,
        fallback_case_count: None,
        refused_case_count: None,
        exact_fallback_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
        evidence_ref: String::from(TWO_DIMENSIONAL_HEAD_RESEARCH_REF),
        detail: String::from(
            "the bounded 2D-head hard-max research lane keeps hull fallback explicit inside the research comparison module, but it does not publish an article-class direct-versus-fallback matrix or a canonical decode-mode contract",
        ),
    });
    rows
}

fn build_direct_fallback_row_for_family(
    efficient_attention_matrix: &TassadarEfficientAttentionBaselineMatrixReport,
    workload_target: TassadarWorkloadTarget,
    candidate_kind: TassadarArticleFastRouteCandidateKind,
    family_kind: TassadarEfficientAttentionBaselineFamilyKind,
) -> TassadarArticleFastRouteDirectFallbackMatrixRow {
    let matrix_row = efficient_attention_matrix
        .rows
        .iter()
        .find(|row| row.workload_target == workload_target)
        .expect("matrix row should exist");
    let cell = matrix_row
        .cells
        .iter()
        .find(|cell| cell.family_kind == family_kind)
        .expect("family cell should exist");
    let exact_fallback_decode_mode = if cell.fallback_case_count > 0
        || candidate_kind
            == TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch
    {
        Some(TassadarExecutorDecodeMode::ReferenceLinear)
    } else {
        None
    };
    TassadarArticleFastRouteDirectFallbackMatrixRow {
        candidate_kind,
        workload_target: Some(workload_target),
        evidence_available: true,
        direct_case_count: Some(cell.direct_case_count),
        fallback_case_count: Some(cell.fallback_case_count),
        refused_case_count: Some(cell.refused_case_count),
        exact_fallback_decode_mode,
        evidence_ref: cell.artifact_ref.clone(),
        detail: format!(
            "{} direct-vs-fallback row for {:?}: direct_case_count={}, fallback_case_count={}, refused_case_count={}, note=`{}`",
            candidate_kind.label(),
            workload_target,
            cell.direct_case_count,
            cell.fallback_case_count,
            cell.refused_case_count,
            cell.note,
        ),
    }
}

fn build_routeability_checks(
    direct_proof_report: &DirectModelWeightExecutionProofReportView,
    hull_cache_closure: &TassadarHullCacheClosureReport,
    recurrent_runtime_report: &TassadarRecurrentFastPathRuntimeBaselineReport,
) -> Result<
    Vec<TassadarArticleFastRouteRouteabilityCheck>,
    TassadarArticleFastRouteArchitectureSelectionError,
> {
    let hull_cache = build_hull_cache_routeability_check(direct_proof_report, hull_cache_closure)?;
    let recurrent = TassadarArticleFastRouteRouteabilityCheck {
        candidate_kind: TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime,
        evidence_kind:
            TassadarArticleFastRouteRouteabilityEvidenceKind::DecodeModeMissingFromCanonicalRouteContract,
        transformer_model_id: direct_proof_report.model_id.clone(),
        requested_decode_mode: None,
        aggregate_route_posture: None,
        projected_route_descriptor_digest: None,
        benchmark_report_ref: Some(String::from(
            TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
        )),
        module_class_rows: Vec::new(),
        routeable: false,
        direct_module_class_count: 0,
        fallback_module_class_count: 0,
        detail: format!(
            "the recurrent runtime baseline is still research-only and the canonical route contract has no recurrent decode-mode identifier. The runtime report keeps its direct families {:?} and fallback families {:?} explicit, but TAS-172 cannot promote it as the canonical fast route until a real route-contract surface exists.",
            recurrent_runtime_report.direct_workload_families,
            recurrent_runtime_report.fallback_workload_families,
        ),
    };
    let hierarchical = TassadarArticleFastRouteRouteabilityCheck {
        candidate_kind: TassadarArticleFastRouteCandidateKind::HierarchicalHullResearch,
        evidence_kind:
            TassadarArticleFastRouteRouteabilityEvidenceKind::DecodeModeMissingFromCanonicalRouteContract,
        transformer_model_id: direct_proof_report.model_id.clone(),
        requested_decode_mode: None,
        aggregate_route_posture: None,
        projected_route_descriptor_digest: None,
        benchmark_report_ref: Some(String::from(
            TASSADAR_EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF,
        )),
        module_class_rows: Vec::new(),
        routeable: false,
        direct_module_class_count: 0,
        fallback_module_class_count: 0,
        detail: String::from(
            "the hierarchical-hull candidate is faster on some article workloads, but it remains research-only and the canonical planner route contract has no hierarchical-hull decode-mode identifier. TAS-172 therefore records it as evidence, not as the canonical route.",
        ),
    };
    let two_dimensional_head = TassadarArticleFastRouteRouteabilityCheck {
        candidate_kind: TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch,
        evidence_kind: TassadarArticleFastRouteRouteabilityEvidenceKind::BoundedResearchWindowOnly,
        transformer_model_id: direct_proof_report.model_id.clone(),
        requested_decode_mode: None,
        aggregate_route_posture: None,
        projected_route_descriptor_digest: None,
        benchmark_report_ref: None,
        module_class_rows: Vec::new(),
        routeable: false,
        direct_module_class_count: 0,
        fallback_module_class_count: 0,
        detail: String::from(
            "the 2D-head hard-max candidate only exists as a bounded windowed research lane with hull fallback. It is not an article-class route contract, not an article-scale exactness artifact, and not a canonical decode-mode family.",
        ),
    };
    Ok(vec![
        hull_cache,
        recurrent,
        hierarchical,
        two_dimensional_head,
    ])
}

fn build_hull_cache_routeability_check(
    direct_proof_report: &DirectModelWeightExecutionProofReportView,
    hull_cache_closure: &TassadarHullCacheClosureReport,
) -> Result<
    TassadarArticleFastRouteRouteabilityCheck,
    TassadarArticleFastRouteArchitectureSelectionError,
> {
    let route_descriptor = projected_hull_cache_route_descriptor(
        &direct_proof_report.model_id,
        hull_cache_closure,
        &direct_proof_report.benchmark_report_ref,
    );
    let route_candidate = TassadarPlannerExecutorRouteCandidate::new(
        "tas-172-projection",
        "tas-172-projection-worker",
        "psionic",
        true,
        route_descriptor.clone(),
    );
    let mut module_class_rows = Vec::new();
    let mut direct_module_class_count = 0;
    let mut fallback_module_class_count = 0;
    for module_class in article_workload_classes() {
        let request = TassadarPlannerExecutorRouteNegotiationRequest::new(
            format!("tas-172-hull-{}", module_class.as_str()),
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_requested_model_id(direct_proof_report.model_id.as_str())
        .with_requested_wasm_module_class(module_class);
        match negotiate_tassadar_planner_executor_route(&[route_candidate.clone()], &request) {
            TassadarPlannerExecutorRouteNegotiationOutcome::Selected { selection } => {
                let wasm_capability = selection
                    .wasm_capability
                    .expect("module-class aware selection should include capability");
                let negotiated_route_state = selection.route_state;
                let effective_decode_mode = selection.effective_decode_mode;
                match negotiated_route_state {
                    TassadarPlannerExecutorNegotiatedRouteState::Direct => {
                        direct_module_class_count += 1;
                    }
                    TassadarPlannerExecutorNegotiatedRouteState::Fallback => {
                        fallback_module_class_count += 1;
                    }
                }
                module_class_rows.push(TassadarArticleFastRouteModuleClassRouteRow {
                    module_class,
                    routeable: true,
                    negotiated_route_state: Some(negotiated_route_state),
                    effective_decode_mode: Some(effective_decode_mode),
                    exact_fallback_decode_mode: wasm_capability.exact_fallback_decode_mode,
                    detail: format!(
                        "projected HullCache route negotiation selected {:?} with effective decode {:?} for module class `{}`",
                        negotiated_route_state,
                        effective_decode_mode,
                        module_class.as_str(),
                    ),
                });
            }
            TassadarPlannerExecutorRouteNegotiationOutcome::Refused { refusal } => {
                return Err(
                    TassadarArticleFastRouteArchitectureSelectionError::Invariant {
                        detail: format!(
                            "projected HullCache route unexpectedly refused module class `{}`: {}",
                            module_class.as_str(),
                            refusal.detail,
                        ),
                    },
                );
            }
        }
    }
    Ok(TassadarArticleFastRouteRouteabilityCheck {
        candidate_kind: TassadarArticleFastRouteCandidateKind::HullCacheRuntime,
        evidence_kind:
            TassadarArticleFastRouteRouteabilityEvidenceKind::ProjectedPlannerRouteDescriptor,
        transformer_model_id: direct_proof_report.model_id.clone(),
        requested_decode_mode: Some(TassadarExecutorDecodeMode::HullCache),
        aggregate_route_posture: Some(TassadarPlannerExecutorRoutePosture::FallbackCapable),
        projected_route_descriptor_digest: Some(route_descriptor.descriptor_digest),
        benchmark_report_ref: Some(direct_proof_report.benchmark_report_ref.clone()),
        module_class_rows,
        routeable: true,
        direct_module_class_count,
        fallback_module_class_count,
        detail: format!(
            "HullCache is the only fast family that already fits the canonical planner route contract: the projected Transformer-backed descriptor stays routeable on all 6 declared article module classes, with {} direct and {} explicit reference-linear fallback rows and no hidden fallback.",
            direct_module_class_count, fallback_module_class_count,
        ),
    })
}

fn projected_hull_cache_route_descriptor(
    transformer_model_id: &str,
    hull_cache_closure: &TassadarHullCacheClosureReport,
    benchmark_report_ref: &str,
) -> TassadarPlannerExecutorRouteDescriptor {
    let mut rows = Vec::new();
    for summary in &hull_cache_closure.exact_workloads {
        let module_class = workload_class_for_target(summary.workload_target);
        rows.push(TassadarPlannerExecutorWasmCapabilityRow::new(
            module_class,
            vec![TassadarExecutorDecodeMode::HullCache],
            vec![TassadarExecutorDecodeMode::HullCache],
            None,
            opcode_families_for_workload_class(module_class),
            TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
            Some(String::from(benchmark_report_ref)),
            vec![
                TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
            ],
            format!(
                "projected HullCache direct route from committed closure row `{}`",
                summary.artifact_ref,
            ),
        ));
    }
    for summary in &hull_cache_closure.fallback_only_workloads {
        let module_class = workload_class_for_target(summary.workload_target);
        rows.push(TassadarPlannerExecutorWasmCapabilityRow::new(
            module_class,
            vec![TassadarExecutorDecodeMode::HullCache],
            Vec::new(),
            Some(TassadarExecutorDecodeMode::ReferenceLinear),
            opcode_families_for_workload_class(module_class),
            TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
            Some(String::from(benchmark_report_ref)),
            vec![
                TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
                TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
                TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
            ],
            format!(
                "projected HullCache fallback route from committed closure row `{}`",
                summary.artifact_ref,
            ),
        ));
    }
    let wasm_capability_matrix = TassadarPlannerExecutorWasmCapabilityMatrix::new(
        format!("tassadar.article_fast_route.hull_cache.projected_matrix.{transformer_model_id}.v1"),
        transformer_model_id,
        rows,
        "TAS-172 projected HullCache route matrix derived from the committed HullCache closure report; publication remains blocked until TAS-173 integrates the fast path into the canonical Transformer-backed model".to_string(),
    );
    TassadarPlannerExecutorRouteDescriptor::new(
        format!("tassadar.article_fast_route.hull_cache.projected_route.{transformer_model_id}.v1"),
        transformer_model_id,
        benchmark_report_ref,
        ARTICLE_INTERNAL_COMPUTE_PROFILE_ID,
        stable_digest(
            b"psionic_tas_172_hull_cache_projection_claim|",
            &(
                transformer_model_id,
                hull_cache_closure.report_digest.as_str(),
            ),
        ),
        wasm_capability_matrix.matrix_digest.clone(),
        wasm_capability_matrix,
        vec![TassadarPlannerExecutorDecodeCapability {
            requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
            route_posture: TassadarPlannerExecutorRoutePosture::FallbackCapable,
            benchmark_report_ref: String::from(benchmark_report_ref),
            note: String::from(
                "projected HullCache route posture over the declared article module classes; direct and exact-fallback rows remain explicit pending canonical model integration",
            ),
        }],
        vec![
            TassadarPlannerExecutorRouteRefusalReason::UnsupportedProduct,
            TassadarPlannerExecutorRouteRefusalReason::UnknownModel,
            TassadarPlannerExecutorRouteRefusalReason::ProviderNotReady,
            TassadarPlannerExecutorRouteRefusalReason::BenchmarkGateMissing,
            TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
            TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
            TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
            TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported,
            TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
            TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
        ],
        "TAS-172 projected planner route descriptor used only to prove HullCache fits the canonical Transformer-backed route contract before TAS-173 integration",
    )
}

fn build_candidate_verdicts(
    exactness_matrix_rows: &[TassadarArticleFastRouteExactnessMatrixRow],
    direct_fallback_matrix_rows: &[TassadarArticleFastRouteDirectFallbackMatrixRow],
    routeability_checks: &[TassadarArticleFastRouteRouteabilityCheck],
) -> Vec<TassadarArticleFastRouteCandidateVerdict> {
    candidate_kinds()
        .into_iter()
        .map(|candidate_kind| {
            let exactness_rows = exactness_matrix_rows
                .iter()
                .filter(|row| row.candidate_kind == candidate_kind)
                .collect::<Vec<_>>();
            let fallback_rows = direct_fallback_matrix_rows
                .iter()
                .filter(|row| row.candidate_kind == candidate_kind)
                .collect::<Vec<_>>();
            let routeability = routeability_checks
                .iter()
                .find(|row| row.candidate_kind == candidate_kind)
                .expect("routeability row");
            let article_scale_exactness_green =
                !exactness_rows.is_empty()
                    && exactness_rows.iter().all(|row| row.evidence_available && row.all_cases_exact);
            let explicit_fallback_boundary = !fallback_rows.is_empty()
                && fallback_rows
                    .iter()
                    .all(|row| row.evidence_available || candidate_kind == TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch);
            let promoted_article_lane = exactness_rows.iter().any(|row| {
                row.claim_class == Some(TassadarClaimClass::CompiledArticleClass)
            }) && candidate_kind == TassadarArticleFastRouteCandidateKind::HullCacheRuntime;
            let selected = candidate_kind == TassadarArticleFastRouteCandidateKind::HullCacheRuntime
                && article_scale_exactness_green
                && explicit_fallback_boundary
                && routeability.routeable
                && promoted_article_lane;
            let detail = match candidate_kind {
                TassadarArticleFastRouteCandidateKind::HullCacheRuntime => String::from(
                    "selected because it is exact on the full article-class matrix, keeps fallback explicit on long-loop and Sudoku, already has a canonical HullCache decode-mode contract, and is the only fast family already promoted on the runtime-facing article lane",
                ),
                TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime => String::from(
                    "rejected because it remains research-only and the canonical route contract has no recurrent decode-mode family even though the runtime baseline is fast and exact on its bounded rows",
                ),
                TassadarArticleFastRouteCandidateKind::HierarchicalHullResearch => String::from(
                    "rejected because it remains research-only and lacks a canonical route-contract decode family despite strong same-harness speed and exactness",
                ),
                TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch => {
                    String::from(
                        "rejected because the repo only carries a bounded research lane with hull fallback rather than an article-class fast-route artifact or canonical decode-mode contract",
                    )
                }
            };
            TassadarArticleFastRouteCandidateVerdict {
                candidate_kind,
                article_scale_exactness_green,
                explicit_fallback_boundary,
                routeability_green: routeability.routeable,
                promoted_article_lane,
                selected,
                detail,
            }
        })
        .collect()
}

fn candidate_kinds() -> [TassadarArticleFastRouteCandidateKind; 4] {
    [
        TassadarArticleFastRouteCandidateKind::HullCacheRuntime,
        TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime,
        TassadarArticleFastRouteCandidateKind::HierarchicalHullResearch,
        TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch,
    ]
}

fn article_workload_classes() -> [TassadarWorkloadClass; 6] {
    use TassadarWorkloadClass::{
        BranchHeavyKernel, HungarianMatching, LongLoopKernel, MemoryHeavyKernel, MicroWasmKernel,
        SudokuClass,
    };
    [
        MicroWasmKernel,
        BranchHeavyKernel,
        MemoryHeavyKernel,
        LongLoopKernel,
        SudokuClass,
        HungarianMatching,
    ]
}

fn workload_class_for_target(workload_target: TassadarWorkloadTarget) -> TassadarWorkloadClass {
    use TassadarWorkloadTarget::{
        BranchHeavyKernel, HungarianMatching, LongLoopKernel, MemoryHeavyKernel, MicroWasmKernel,
        SudokuClass,
    };
    match workload_target {
        MicroWasmKernel => TassadarWorkloadClass::MicroWasmKernel,
        BranchHeavyKernel => TassadarWorkloadClass::BranchHeavyKernel,
        MemoryHeavyKernel => TassadarWorkloadClass::MemoryHeavyKernel,
        LongLoopKernel => TassadarWorkloadClass::LongLoopKernel,
        SudokuClass => TassadarWorkloadClass::SudokuClass,
        HungarianMatching => TassadarWorkloadClass::HungarianMatching,
        other => panic!("TAS-172 only expects article-class workload targets, found {other:?}"),
    }
}

fn opcode_families_for_workload_class(
    workload_class: TassadarWorkloadClass,
) -> Vec<TassadarPlannerExecutorWasmOpcodeFamily> {
    use TassadarPlannerExecutorWasmOpcodeFamily::{
        CoreI32Arithmetic, DirectCallFrames, LinearMemoryV2, StructuredControl,
    };
    match workload_class {
        TassadarWorkloadClass::MicroWasmKernel => {
            vec![
                CoreI32Arithmetic,
                StructuredControl,
                LinearMemoryV2,
                DirectCallFrames,
            ]
        }
        TassadarWorkloadClass::BranchHeavyKernel
        | TassadarWorkloadClass::LongLoopKernel
        | TassadarWorkloadClass::SudokuClass => vec![CoreI32Arithmetic, StructuredControl],
        TassadarWorkloadClass::MemoryHeavyKernel | TassadarWorkloadClass::HungarianMatching => {
            vec![CoreI32Arithmetic, StructuredControl, LinearMemoryV2]
        }
        TassadarWorkloadClass::ArithmeticMicroprogram => vec![CoreI32Arithmetic],
        TassadarWorkloadClass::MemoryLookupMicroprogram => {
            vec![CoreI32Arithmetic, LinearMemoryV2]
        }
        TassadarWorkloadClass::BranchControlFlowMicroprogram
        | TassadarWorkloadClass::ClrsShortestPath => vec![CoreI32Arithmetic, StructuredControl],
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn read_artifact<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFastRouteArchitectureSelectionError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteArchitectureSelectionError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteArchitectureSelectionError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_architecture_selection_report, read_artifact,
        tassadar_article_fast_route_architecture_selection_report_path,
        write_tassadar_article_fast_route_architecture_selection_report,
        TassadarArticleFastRouteArchitectureSelectionReport, TassadarArticleFastRouteCandidateKind,
        TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    };

    #[test]
    fn fast_route_selection_report_chooses_hull_cache_without_final_green() {
        let report =
            build_tassadar_article_fast_route_architecture_selection_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.transformer_model_route_anchor_review.passed);
        assert_eq!(
            report.selected_candidate_kind,
            TassadarArticleFastRouteCandidateKind::HullCacheRuntime
        );
        assert!(report.fast_route_selection_green);
        assert!(report.article_equivalence_green);

        let hull_routeability = report
            .routeability_checks
            .iter()
            .find(|row| {
                row.candidate_kind == TassadarArticleFastRouteCandidateKind::HullCacheRuntime
            })
            .expect("HullCache routeability");
        assert!(hull_routeability.routeable);
        assert_eq!(hull_routeability.direct_module_class_count, 6);
        assert_eq!(hull_routeability.fallback_module_class_count, 0);

        let recurrent_verdict = report
            .candidate_verdicts
            .iter()
            .find(|row| {
                row.candidate_kind == TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime
            })
            .expect("recurrent verdict");
        assert!(recurrent_verdict.article_scale_exactness_green);
        assert!(!recurrent_verdict.routeability_green);
        assert!(!recurrent_verdict.selected);

        assert_eq!(
            report
                .acceptance_gate_tie
                .blocked_issue_ids
                .first()
                .map(String::as_str),
            None
        );
    }

    #[test]
    fn fast_route_selection_report_matches_committed_truth() {
        let generated =
            build_tassadar_article_fast_route_architecture_selection_report().expect("report");
        let committed: TassadarArticleFastRouteArchitectureSelectionReport = read_artifact(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            "article_fast_route_architecture_selection_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_selection_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_architecture_selection_report.json");
        let written = write_tassadar_article_fast_route_architecture_selection_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleFastRouteArchitectureSelectionReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_architecture_selection_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_architecture_selection_report.json")
        );
    }
}
