use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    tassadar_hungarian_10x10_corpus, TassadarArticleFastRouteThroughputFloorStatus,
    TassadarExecutorDecodeMode, TassadarExecutorSelectionState,
};

use crate::{
    build_tassadar_article_demo_frontend_parity_report,
    build_tassadar_article_equivalence_acceptance_gate_report, TassadarArticleDemoFrontendFamily,
    TassadarArticleDemoFrontendParityReport, TassadarArticleDemoFrontendParityReportError,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFastRouteThroughputFloorReport,
    TassadarArticleFastRouteThroughputFloorReportError,
    TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
};

pub const TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hungarian_demo_parity_report.json";
pub const TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_CHECKER_REF: &str =
    "scripts/check-tassadar-article-hungarian-demo-parity.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-180";
const EXPECTED_CASE_ID: &str = "hungarian_10x10_test_a";
const EXPECTED_SESSION_CASE_NAME: &str = "direct_hungarian_10x10_hull";
const EXPECTED_HYBRID_CASE_NAME: &str = "delegated_hungarian_10x10_hull";
const HUNGARIAN_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json";
const HUNGARIAN_FAST_ROUTE_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hungarian_demo_fast_route_session_artifact.json";
const HUNGARIAN_FAST_ROUTE_HYBRID_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hungarian_demo_fast_route_hybrid_workflow_artifact.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityFrontendReview {
    pub report_ref: String,
    pub demo_id: TassadarArticleDemoFrontendFamily,
    pub source_case_id: String,
    pub canonical_case_id: String,
    pub canonical_workload_family_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub compile_receipt_digest: String,
    pub row_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityNoToolProofReview {
    pub report_ref: String,
    pub canonical_case_id: String,
    pub workload_family_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub canonical_case_optimal_cost: i32,
    pub canonical_case_assignment: Vec<i32>,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub halt_match: bool,
    pub requested_decode_mode: String,
    pub effective_decode_mode: String,
    pub fallback_observed: bool,
    pub external_tool_surface_observed: bool,
    pub runtime_backend: String,
    pub no_tool_proof_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityFastRouteSessionReview {
    pub artifact_ref: String,
    pub case_name: String,
    pub case_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub model_id: String,
    pub output_values: Vec<i32>,
    pub output_parity_green: bool,
    pub fast_route_direct_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityFastRouteHybridReview {
    pub artifact_ref: String,
    pub case_name: String,
    pub case_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub planner_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub executor_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub model_id: String,
    pub output_values: Vec<i32>,
    pub output_parity_green: bool,
    pub fast_route_direct_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityThroughputReview {
    pub report_ref: String,
    pub case_id: String,
    pub workload_id: String,
    pub program_profile_id: String,
    pub runtime_runner_id: String,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub exactness_bps: u32,
    pub public_anchor_token_throughput_per_second: u64,
    pub public_anchor_line_throughput_per_second: u64,
    pub public_anchor_total_token_count: u64,
    pub measured_tokens_per_second: f64,
    pub measured_lines_per_second: f64,
    pub measured_total_token_count: u64,
    pub public_token_anchor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub public_line_anchor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub internal_token_floor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub declared_floor_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityBindingReview {
    pub frontend_source_matches_reproducer: bool,
    pub canonical_case_alignment_green: bool,
    pub session_case_alignment_green: bool,
    pub hybrid_case_alignment_green: bool,
    pub throughput_case_alignment_green: bool,
    pub canonical_output_parity_green: bool,
    pub binding_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleHungarianDemoParityAcceptanceGateTie,
    pub frontend_review: TassadarArticleHungarianDemoParityFrontendReview,
    pub no_tool_proof_review: TassadarArticleHungarianDemoParityNoToolProofReview,
    pub fast_route_session_review: TassadarArticleHungarianDemoParityFastRouteSessionReview,
    pub fast_route_hybrid_review: TassadarArticleHungarianDemoParityFastRouteHybridReview,
    pub throughput_review: TassadarArticleHungarianDemoParityThroughputReview,
    pub binding_review: TassadarArticleHungarianDemoParityBindingReview,
    pub hungarian_demo_parity_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarHungarian10x10ArticleReproducerView {
    workload_family_id: String,
    source_ref: String,
    source_digest: String,
    canonical_case_id: String,
    canonical_case_optimal_cost: i32,
    canonical_case_assignment: Vec<i32>,
    exact_trace_match: bool,
    final_output_match: bool,
    halt_match: bool,
    direct_execution_posture: TassadarHungarian10x10DirectExecutionPostureView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarHungarian10x10DirectExecutionPostureView {
    requested_decode_mode: String,
    effective_decode_mode: String,
    fallback_observed: bool,
    external_tool_surface_observed: bool,
    runtime_backend: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionArtifactView {
    cases: Vec<ArticleExecutorSessionArtifactCaseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionArtifactCaseView {
    name: String,
    request: ArticleExecutorSessionRequestView,
    outcome: ArticleExecutorSessionOutcomeView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionRequestView {
    article_case_id: String,
    requested_decode_mode: TassadarExecutorDecodeMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionOutcomeView {
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<ArticleExecutorSessionCompletedResponseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSessionCompletedResponseView {
    benchmark_identity: ArticleBenchmarkIdentityView,
    executor_response: ArticleExecutorResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleBenchmarkIdentityView {
    case_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorResponseView {
    model_descriptor: ArticleExecutorModelDescriptorView,
    execution_report: ArticleExecutorExecutionReportView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorModelDescriptorView {
    model: ArticleExecutorModelView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorModelView {
    model_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorExecutionReportView {
    selection: ArticleExecutorSelectionView,
    execution: ArticleExecutionView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSelectionView {
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutionView {
    outputs: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowArtifactView {
    cases: Vec<ArticleHybridWorkflowArtifactCaseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowArtifactCaseView {
    name: String,
    request: ArticleHybridWorkflowRequestView,
    outcome: ArticleHybridWorkflowOutcomeView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowRequestView {
    article_case_id: String,
    requested_decode_mode: TassadarExecutorDecodeMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowOutcomeView {
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<ArticleHybridWorkflowCompletedResponseView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridWorkflowCompletedResponseView {
    benchmark_identity: ArticleBenchmarkIdentityView,
    planner_response: ArticleHybridPlannerResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridPlannerResponseView {
    routing_decision: ArticleHybridRoutingDecisionView,
    executor_response: ArticleExecutorResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleHybridRoutingDecisionView {
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
}

#[derive(Debug, Error)]
pub enum TassadarArticleHungarianDemoParityReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Frontend(#[from] TassadarArticleDemoFrontendParityReportError),
    #[error(transparent)]
    Throughput(#[from] TassadarArticleFastRouteThroughputFloorReportError),
    #[error("internal TAS-180 invariant failed: {detail}")]
    Invariant { detail: String },
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

pub fn build_tassadar_article_hungarian_demo_parity_report(
) -> Result<TassadarArticleHungarianDemoParityReport, TassadarArticleHungarianDemoParityReportError>
{
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let frontend_report = build_tassadar_article_demo_frontend_parity_report()?;
    let throughput_report: TassadarArticleFastRouteThroughputFloorReport = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
        "tassadar_article_fast_route_throughput_floor_report",
    )?;
    let no_tool_proof: TassadarHungarian10x10ArticleReproducerView = read_repo_json(
        HUNGARIAN_REPRODUCER_REPORT_REF,
        "hungarian_10x10_article_reproducer_report",
    )?;
    let session_artifact: ArticleExecutorSessionArtifactView = read_repo_json(
        HUNGARIAN_FAST_ROUTE_SESSION_ARTIFACT_REF,
        "article_hungarian_demo_fast_route_session_artifact",
    )?;
    let hybrid_artifact: ArticleHybridWorkflowArtifactView = read_repo_json(
        HUNGARIAN_FAST_ROUTE_HYBRID_ARTIFACT_REF,
        "article_hungarian_demo_fast_route_hybrid_workflow_artifact",
    )?;
    Ok(build_report_from_inputs(
        acceptance_gate,
        frontend_report,
        throughput_report,
        no_tool_proof,
        session_artifact,
        hybrid_artifact,
    )?)
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    frontend_report: TassadarArticleDemoFrontendParityReport,
    throughput_report: TassadarArticleFastRouteThroughputFloorReport,
    no_tool_proof: TassadarHungarian10x10ArticleReproducerView,
    session_artifact: ArticleExecutorSessionArtifactView,
    hybrid_artifact: ArticleHybridWorkflowArtifactView,
) -> Result<TassadarArticleHungarianDemoParityReport, TassadarArticleHungarianDemoParityReportError>
{
    let acceptance_gate_tie = TassadarArticleHungarianDemoParityAcceptanceGateTie {
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
    let frontend_review = build_frontend_review(&frontend_report)?;
    let no_tool_proof_review = build_no_tool_proof_review(&no_tool_proof);
    let expected_outputs = expected_hungarian_outputs()?;
    let fast_route_session_review = build_session_review(&session_artifact, &expected_outputs)?;
    let fast_route_hybrid_review = build_hybrid_review(&hybrid_artifact, &expected_outputs)?;
    let throughput_review = build_throughput_review(&throughput_report)?;
    let binding_review = build_binding_review(
        &frontend_review,
        &no_tool_proof_review,
        &fast_route_session_review,
        &fast_route_hybrid_review,
        &throughput_review,
    );
    let hungarian_demo_parity_green = acceptance_gate_tie.tied_requirement_satisfied
        && frontend_review.row_green
        && no_tool_proof_review.no_tool_proof_green
        && fast_route_session_review.fast_route_direct_green
        && fast_route_hybrid_review.fast_route_direct_green
        && throughput_review.declared_floor_green
        && binding_review.binding_green;

    let mut report = TassadarArticleHungarianDemoParityReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_hungarian_demo_parity.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        frontend_review,
        no_tool_proof_review,
        fast_route_session_review,
        fast_route_hybrid_review,
        throughput_review,
        binding_review,
        hungarian_demo_parity_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && hungarian_demo_parity_green,
        claim_boundary: String::from(
            "this report closes TAS-180 only. It binds the canonical 10x10 Hungarian article demo source to one direct HullCache fast-route session, one planner-owned HullCache hybrid workflow, the declared throughput-floor artifact, and the existing no-tool reproducer proof surface. It does not yet claim Arto closure, benchmark-wide hard-Sudoku closure, unified demo-and-benchmark equivalence, single-run closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Hungarian article demo parity now records tied_requirement_satisfied={}, frontend_green={}, no_tool_proof_green={}, session_fast_route_green={}, hybrid_fast_route_green={}, throughput_floor_green={}, binding_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.frontend_review.row_green,
        report.no_tool_proof_review.no_tool_proof_green,
        report.fast_route_session_review.fast_route_direct_green,
        report.fast_route_hybrid_review.fast_route_direct_green,
        report.throughput_review.declared_floor_green,
        report.binding_review.binding_green,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_hungarian_demo_parity_report|",
        &report,
    );
    Ok(report)
}

fn build_frontend_review(
    frontend_report: &TassadarArticleDemoFrontendParityReport,
) -> Result<
    TassadarArticleHungarianDemoParityFrontendReview,
    TassadarArticleHungarianDemoParityReportError,
> {
    let row = frontend_report
        .demo_rows
        .iter()
        .find(|row| row.demo_id == TassadarArticleDemoFrontendFamily::Hungarian10x10)
        .ok_or_else(
            || TassadarArticleHungarianDemoParityReportError::Invariant {
                detail: String::from("frontend parity report is missing the Hungarian10x10 row"),
            },
        )?;
    Ok(TassadarArticleHungarianDemoParityFrontendReview {
        report_ref: String::from(TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF),
        demo_id: row.demo_id,
        source_case_id: row.source_case_id.clone(),
        canonical_case_id: row.canonical_case_id.clone(),
        canonical_workload_family_id: row.canonical_workload_family_id.clone(),
        source_ref: row.source_ref.clone(),
        source_digest: row.source_digest.clone(),
        compile_receipt_digest: row.compile_receipt_digest.clone(),
        row_green: row.row_green,
        detail: row.detail.clone(),
    })
}

fn build_no_tool_proof_review(
    no_tool_proof: &TassadarHungarian10x10ArticleReproducerView,
) -> TassadarArticleHungarianDemoParityNoToolProofReview {
    let no_tool_proof_green = no_tool_proof.canonical_case_id == EXPECTED_CASE_ID
        && no_tool_proof.exact_trace_match
        && no_tool_proof.final_output_match
        && no_tool_proof.halt_match
        && !no_tool_proof.direct_execution_posture.fallback_observed
        && !no_tool_proof
            .direct_execution_posture
            .external_tool_surface_observed
        && no_tool_proof.direct_execution_posture.requested_decode_mode
            == no_tool_proof.direct_execution_posture.effective_decode_mode;
    TassadarArticleHungarianDemoParityNoToolProofReview {
        report_ref: String::from(HUNGARIAN_REPRODUCER_REPORT_REF),
        canonical_case_id: no_tool_proof.canonical_case_id.clone(),
        workload_family_id: no_tool_proof.workload_family_id.clone(),
        source_ref: no_tool_proof.source_ref.clone(),
        source_digest: no_tool_proof.source_digest.clone(),
        canonical_case_optimal_cost: no_tool_proof.canonical_case_optimal_cost,
        canonical_case_assignment: no_tool_proof.canonical_case_assignment.clone(),
        exact_trace_match: no_tool_proof.exact_trace_match,
        final_output_match: no_tool_proof.final_output_match,
        halt_match: no_tool_proof.halt_match,
        requested_decode_mode: no_tool_proof
            .direct_execution_posture
            .requested_decode_mode
            .clone(),
        effective_decode_mode: no_tool_proof
            .direct_execution_posture
            .effective_decode_mode
            .clone(),
        fallback_observed: no_tool_proof.direct_execution_posture.fallback_observed,
        external_tool_surface_observed: no_tool_proof
            .direct_execution_posture
            .external_tool_surface_observed,
        runtime_backend: no_tool_proof.direct_execution_posture.runtime_backend.clone(),
        no_tool_proof_green,
        detail: format!(
            "Hungarian 10x10 no-tool proof stayed exact on `{}` with requested/effective decode `{}`/`{}`, fallback_observed={}, and external_tool_surface_observed={}.",
            no_tool_proof.canonical_case_id,
            no_tool_proof.direct_execution_posture.requested_decode_mode,
            no_tool_proof.direct_execution_posture.effective_decode_mode,
            no_tool_proof.direct_execution_posture.fallback_observed,
            no_tool_proof
                .direct_execution_posture
                .external_tool_surface_observed
        ),
    }
}

fn build_session_review(
    session_artifact: &ArticleExecutorSessionArtifactView,
    expected_outputs: &[i32],
) -> Result<
    TassadarArticleHungarianDemoParityFastRouteSessionReview,
    TassadarArticleHungarianDemoParityReportError,
> {
    let case = session_artifact
        .cases
        .iter()
        .find(|case| case.name == EXPECTED_SESSION_CASE_NAME)
        .ok_or_else(
            || TassadarArticleHungarianDemoParityReportError::Invariant {
                detail: format!(
                    "session artifact is missing case `{}`",
                    EXPECTED_SESSION_CASE_NAME
                ),
            },
        )?;
    let response = case.outcome.response.as_ref().ok_or_else(|| {
        TassadarArticleHungarianDemoParityReportError::Invariant {
            detail: format!("session artifact case `{}` did not complete", case.name),
        }
    })?;
    let output_parity_green = response
        .executor_response
        .execution_report
        .execution
        .outputs
        == expected_outputs;
    let fast_route_direct_green = case.outcome.status == "completed"
        && case.request.article_case_id == EXPECTED_CASE_ID
        && response.benchmark_identity.case_id == EXPECTED_CASE_ID
        && case.request.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        && response
            .executor_response
            .execution_report
            .selection
            .effective_decode_mode
            == Some(TassadarExecutorDecodeMode::HullCache)
        && response
            .executor_response
            .execution_report
            .selection
            .selection_state
            == TassadarExecutorSelectionState::Direct
        && output_parity_green;
    Ok(TassadarArticleHungarianDemoParityFastRouteSessionReview {
        artifact_ref: String::from(HUNGARIAN_FAST_ROUTE_SESSION_ARTIFACT_REF),
        case_name: case.name.clone(),
        case_id: response.benchmark_identity.case_id.clone(),
        requested_decode_mode: case.request.requested_decode_mode,
        effective_decode_mode: response
            .executor_response
            .execution_report
            .selection
            .effective_decode_mode,
        selection_state: response
            .executor_response
            .execution_report
            .selection
            .selection_state,
        model_id: response
            .executor_response
            .model_descriptor
            .model
            .model_id
            .clone(),
        output_values: response
            .executor_response
            .execution_report
            .execution
            .outputs
            .clone(),
        output_parity_green,
        fast_route_direct_green,
        detail: format!(
            "session artifact `{}` kept `{}` on direct HullCache with outputs {:?}.",
            case.name,
            response.benchmark_identity.case_id,
            response
                .executor_response
                .execution_report
                .execution
                .outputs
        ),
    })
}

fn build_hybrid_review(
    hybrid_artifact: &ArticleHybridWorkflowArtifactView,
    expected_outputs: &[i32],
) -> Result<
    TassadarArticleHungarianDemoParityFastRouteHybridReview,
    TassadarArticleHungarianDemoParityReportError,
> {
    let case = hybrid_artifact
        .cases
        .iter()
        .find(|case| case.name == EXPECTED_HYBRID_CASE_NAME)
        .ok_or_else(
            || TassadarArticleHungarianDemoParityReportError::Invariant {
                detail: format!(
                    "hybrid artifact is missing case `{}`",
                    EXPECTED_HYBRID_CASE_NAME
                ),
            },
        )?;
    let response = case.outcome.response.as_ref().ok_or_else(|| {
        TassadarArticleHungarianDemoParityReportError::Invariant {
            detail: format!("hybrid artifact case `{}` did not complete", case.name),
        }
    })?;
    let output_parity_green = response
        .planner_response
        .executor_response
        .execution_report
        .execution
        .outputs
        == expected_outputs;
    let fast_route_direct_green = case.outcome.status == "completed"
        && case.request.article_case_id == EXPECTED_CASE_ID
        && response.benchmark_identity.case_id == EXPECTED_CASE_ID
        && case.request.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        && response
            .planner_response
            .routing_decision
            .effective_decode_mode
            == Some(TassadarExecutorDecodeMode::HullCache)
        && response
            .planner_response
            .executor_response
            .execution_report
            .selection
            .effective_decode_mode
            == Some(TassadarExecutorDecodeMode::HullCache)
        && response
            .planner_response
            .executor_response
            .execution_report
            .selection
            .selection_state
            == TassadarExecutorSelectionState::Direct
        && output_parity_green;
    Ok(TassadarArticleHungarianDemoParityFastRouteHybridReview {
        artifact_ref: String::from(HUNGARIAN_FAST_ROUTE_HYBRID_ARTIFACT_REF),
        case_name: case.name.clone(),
        case_id: response.benchmark_identity.case_id.clone(),
        requested_decode_mode: case.request.requested_decode_mode,
        planner_effective_decode_mode: response
            .planner_response
            .routing_decision
            .effective_decode_mode,
        executor_effective_decode_mode: response
            .planner_response
            .executor_response
            .execution_report
            .selection
            .effective_decode_mode,
        selection_state: response
            .planner_response
            .executor_response
            .execution_report
            .selection
            .selection_state,
        model_id: response
            .planner_response
            .executor_response
            .model_descriptor
            .model
            .model_id
            .clone(),
        output_values: response
            .planner_response
            .executor_response
            .execution_report
            .execution
            .outputs
            .clone(),
        output_parity_green,
        fast_route_direct_green,
        detail: format!(
            "hybrid artifact `{}` kept `{}` delegated to direct HullCache with outputs {:?}.",
            case.name,
            response.benchmark_identity.case_id,
            response
                .planner_response
                .executor_response
                .execution_report
                .execution
                .outputs
        ),
    })
}

fn build_throughput_review(
    throughput_report: &TassadarArticleFastRouteThroughputFloorReport,
) -> Result<
    TassadarArticleHungarianDemoParityThroughputReview,
    TassadarArticleHungarianDemoParityReportError,
> {
    let receipt = throughput_report
        .throughput_bundle
        .demo_receipts
        .iter()
        .find(|receipt| receipt.case_id == EXPECTED_CASE_ID)
        .ok_or_else(
            || TassadarArticleHungarianDemoParityReportError::Invariant {
                detail: format!("throughput report is missing case `{}`", EXPECTED_CASE_ID),
            },
        )?;
    let declared_floor_green = throughput_report.throughput_floor_green
        && receipt.selection_state == TassadarExecutorSelectionState::Direct
        && receipt.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
        && receipt.exactness_bps == 10_000
        && receipt.public_token_anchor_status
            == TassadarArticleFastRouteThroughputFloorStatus::Passed
        && receipt.public_line_anchor_status
            == TassadarArticleFastRouteThroughputFloorStatus::Passed
        && receipt.internal_token_floor_status
            == TassadarArticleFastRouteThroughputFloorStatus::Passed;
    Ok(TassadarArticleHungarianDemoParityThroughputReview {
        report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF),
        case_id: receipt.case_id.clone(),
        workload_id: receipt.workload_id.clone(),
        program_profile_id: receipt.program_profile_id.clone(),
        runtime_runner_id: receipt.runtime_runner_id.clone(),
        effective_decode_mode: receipt.effective_decode_mode,
        selection_state: receipt.selection_state,
        exactness_bps: receipt.exactness_bps,
        public_anchor_token_throughput_per_second: receipt.public_anchor.token_throughput_per_second,
        public_anchor_line_throughput_per_second: receipt.public_anchor.line_throughput_per_second,
        public_anchor_total_token_count: receipt.public_anchor.total_token_count,
        measured_tokens_per_second: receipt.tokens_per_second,
        measured_lines_per_second: receipt.lines_per_second,
        measured_total_token_count: receipt.token_trace_counts.total_token_count,
        public_token_anchor_status: receipt.public_token_anchor_status,
        public_line_anchor_status: receipt.public_line_anchor_status,
        internal_token_floor_status: receipt.internal_token_floor_status,
        declared_floor_green,
        detail: format!(
            "throughput report kept `{}` direct on HullCache with exactness_bps={} and public token/line anchors {}/{} at measured tok/s {:.2}.",
            receipt.case_id,
            receipt.exactness_bps,
            receipt.public_anchor.token_throughput_per_second,
            receipt.public_anchor.line_throughput_per_second,
            receipt.tokens_per_second
        ),
    })
}

fn build_binding_review(
    frontend_review: &TassadarArticleHungarianDemoParityFrontendReview,
    no_tool_proof_review: &TassadarArticleHungarianDemoParityNoToolProofReview,
    fast_route_session_review: &TassadarArticleHungarianDemoParityFastRouteSessionReview,
    fast_route_hybrid_review: &TassadarArticleHungarianDemoParityFastRouteHybridReview,
    throughput_review: &TassadarArticleHungarianDemoParityThroughputReview,
) -> TassadarArticleHungarianDemoParityBindingReview {
    let canonical_output_parity_green = fast_route_session_review.output_parity_green
        && fast_route_hybrid_review.output_parity_green
        && no_tool_proof_review.final_output_match;
    let frontend_source_matches_reproducer = frontend_review.source_ref
        == no_tool_proof_review.source_ref
        && frontend_review.source_digest == no_tool_proof_review.source_digest;
    let canonical_case_alignment_green = frontend_review.canonical_case_id == EXPECTED_CASE_ID
        && no_tool_proof_review.canonical_case_id == EXPECTED_CASE_ID;
    let session_case_alignment_green = fast_route_session_review.case_id == EXPECTED_CASE_ID;
    let hybrid_case_alignment_green = fast_route_hybrid_review.case_id == EXPECTED_CASE_ID;
    let throughput_case_alignment_green = throughput_review.case_id == EXPECTED_CASE_ID;
    let binding_green = frontend_source_matches_reproducer
        && canonical_case_alignment_green
        && session_case_alignment_green
        && hybrid_case_alignment_green
        && throughput_case_alignment_green
        && canonical_output_parity_green;
    TassadarArticleHungarianDemoParityBindingReview {
        frontend_source_matches_reproducer,
        canonical_case_alignment_green,
        session_case_alignment_green,
        hybrid_case_alignment_green,
        throughput_case_alignment_green,
        canonical_output_parity_green,
        binding_green,
        detail: format!(
            "frontend source matches reproducer={}, case_alignment(frontend+proof/session/hybrid/throughput)={}/{}/{}/{}, canonical_output_parity_green={}.",
            frontend_source_matches_reproducer,
            canonical_case_alignment_green,
            session_case_alignment_green,
            hybrid_case_alignment_green,
            throughput_case_alignment_green,
            canonical_output_parity_green
        ),
    }
}

fn expected_hungarian_outputs() -> Result<Vec<i32>, TassadarArticleHungarianDemoParityReportError> {
    tassadar_hungarian_10x10_corpus()
        .into_iter()
        .find(|case| case.case_id == EXPECTED_CASE_ID)
        .map(|case| case.validation_case.expected_outputs)
        .ok_or_else(
            || TassadarArticleHungarianDemoParityReportError::Invariant {
                detail: format!(
                    "Hungarian 10x10 corpus is missing canonical case `{}`",
                    EXPECTED_CASE_ID
                ),
            },
        )
}

#[must_use]
pub fn tassadar_article_hungarian_demo_parity_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF)
}

pub fn write_tassadar_article_hungarian_demo_parity_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleHungarianDemoParityReport, TassadarArticleHungarianDemoParityReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleHungarianDemoParityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_hungarian_demo_parity_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleHungarianDemoParityReportError::Write {
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

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleHungarianDemoParityReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleHungarianDemoParityReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleHungarianDemoParityReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_hungarian_demo_parity_report, read_repo_json,
        tassadar_article_hungarian_demo_parity_report_path,
        write_tassadar_article_hungarian_demo_parity_report,
        TassadarArticleHungarianDemoParityReport,
        TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF,
    };

    #[test]
    fn article_hungarian_demo_parity_closes_hungarian_without_final_article_equivalence() {
        let report = build_tassadar_article_hungarian_demo_parity_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.frontend_review.row_green);
        assert!(report.no_tool_proof_review.no_tool_proof_green);
        assert!(report.fast_route_session_review.fast_route_direct_green);
        assert!(report.fast_route_hybrid_review.fast_route_direct_green);
        assert!(report.throughput_review.declared_floor_green);
        assert!(report.binding_review.binding_green);
        assert!(report.hungarian_demo_parity_green);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_hungarian_demo_parity_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_hungarian_demo_parity_report().expect("report");
        let committed: TassadarArticleHungarianDemoParityReport = read_repo_json(
            TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF,
            "article_hungarian_demo_parity_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_hungarian_demo_parity_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_hungarian_demo_parity_report.json");
        let written = write_tassadar_article_hungarian_demo_parity_report(&output_path)?;
        let persisted: TassadarArticleHungarianDemoParityReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_hungarian_demo_parity_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_hungarian_demo_parity_report.json")
        );
        Ok(())
    }
}
