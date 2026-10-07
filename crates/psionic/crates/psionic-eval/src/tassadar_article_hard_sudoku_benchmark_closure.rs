use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    tassadar_article_hard_sudoku_suite, TassadarArticleHardSudokuBenchmarkBundle,
    TassadarExecutorDecodeMode, TassadarExecutorSelectionState,
    TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_REF,
};

use crate::{
    build_tassadar_article_demo_frontend_parity_report,
    build_tassadar_article_equivalence_acceptance_gate_report, TassadarArticleDemoFrontendFamily,
    TassadarArticleDemoFrontendParityReport, TassadarArticleDemoFrontendParityReportError,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hard_sudoku_benchmark_closure_report.json";
pub const TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-hard-sudoku-benchmark-closure.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-181";
const HARD_SUDOKU_SUITE_MANIFEST_REF: &str =
    "fixtures/tassadar/sources/tassadar_article_hard_sudoku_suite_v1.json";
const SUDOKU_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json";
const HARD_SUDOKU_FAST_ROUTE_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hard_sudoku_fast_route_session_artifact.json";
const HARD_SUDOKU_FAST_ROUTE_HYBRID_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hard_sudoku_fast_route_hybrid_workflow_artifact.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuBenchmarkAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuSuiteManifestCaseRow {
    pub case_id: String,
    pub case_role: String,
    pub split: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuSuiteManifest {
    pub schema_version: u16,
    pub manifest_id: String,
    pub manifest_ref: String,
    pub gate_issue_id: String,
    pub canonical_source_case_id: String,
    pub source_ref: String,
    pub frontend_parity_report_ref: String,
    pub runtime_ceiling_seconds: f64,
    pub named_case_id: String,
    pub case_rows: Vec<TassadarArticleHardSudokuSuiteManifestCaseRow>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuManifestReview {
    pub manifest_ref: String,
    pub manifest_id: String,
    pub manifest_source_digest: String,
    pub gate_issue_id: String,
    pub source_ref: String,
    pub runtime_ceiling_seconds: f64,
    pub declared_case_ids: Vec<String>,
    pub named_case_id: String,
    pub manifest_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuFrontendReview {
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
pub struct TassadarArticleHardSudokuNoToolProofReview {
    pub report_ref: String,
    pub canonical_case_id: String,
    pub workload_family_id: String,
    pub source_ref: String,
    pub source_digest: String,
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
pub struct TassadarArticleHardSudokuFastRouteCaseReview {
    pub case_name: String,
    pub case_id: String,
    pub case_role: String,
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
pub struct TassadarArticleHardSudokuFastRouteSessionReview {
    pub artifact_ref: String,
    pub case_reviews: Vec<TassadarArticleHardSudokuFastRouteCaseReview>,
    pub named_arto_green: bool,
    pub suite_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuFastRouteHybridReview {
    pub artifact_ref: String,
    pub case_reviews: Vec<TassadarArticleHardSudokuFastRouteCaseReview>,
    pub named_arto_green: bool,
    pub suite_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuRuntimeCaseReview {
    pub case_id: String,
    pub case_role: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub measured_run_time_seconds: f64,
    pub runtime_ceiling_seconds: f64,
    pub exact_output_match: bool,
    pub exact_behavior_match: bool,
    pub halt_reason_match: bool,
    pub exactness_green: bool,
    pub under_runtime_ceiling: bool,
    pub runtime_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuRuntimeReview {
    pub bundle_ref: String,
    pub runtime_ceiling_seconds: f64,
    pub case_reviews: Vec<TassadarArticleHardSudokuRuntimeCaseReview>,
    pub named_arto_green: bool,
    pub suite_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuBindingReview {
    pub manifest_case_alignment_green: bool,
    pub frontend_source_alignment_green: bool,
    pub no_tool_anchor_alignment_green: bool,
    pub session_case_alignment_green: bool,
    pub hybrid_case_alignment_green: bool,
    pub runtime_case_alignment_green: bool,
    pub named_arto_alignment_green: bool,
    pub binding_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuBenchmarkClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleHardSudokuBenchmarkAcceptanceGateTie,
    pub manifest_review: TassadarArticleHardSudokuManifestReview,
    pub frontend_review: TassadarArticleHardSudokuFrontendReview,
    pub no_tool_proof_review: TassadarArticleHardSudokuNoToolProofReview,
    pub fast_route_session_review: TassadarArticleHardSudokuFastRouteSessionReview,
    pub fast_route_hybrid_review: TassadarArticleHardSudokuFastRouteHybridReview,
    pub runtime_review: TassadarArticleHardSudokuRuntimeReview,
    pub binding_review: TassadarArticleHardSudokuBindingReview,
    pub named_arto_green: bool,
    pub hard_sudoku_suite_green: bool,
    pub hard_sudoku_benchmark_closure_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarSudoku9x9ArticleReproducerView {
    workload_family_id: String,
    source_ref: String,
    source_digest: String,
    canonical_case_id: String,
    exact_trace_match: bool,
    final_output_match: bool,
    halt_match: bool,
    direct_execution_posture: TassadarSudoku9x9DirectExecutionPostureView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarSudoku9x9DirectExecutionPostureView {
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
pub enum TassadarArticleHardSudokuBenchmarkClosureReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Frontend(#[from] TassadarArticleDemoFrontendParityReportError),
    #[error("internal TAS-181 invariant failed: {detail}")]
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

pub fn build_tassadar_article_hard_sudoku_benchmark_closure_report() -> Result<
    TassadarArticleHardSudokuBenchmarkClosureReport,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let frontend_report = build_tassadar_article_demo_frontend_parity_report()?;
    let suite_manifest: TassadarArticleHardSudokuSuiteManifest = read_repo_json(
        HARD_SUDOKU_SUITE_MANIFEST_REF,
        "article_hard_sudoku_suite_manifest",
    )?;
    let no_tool_proof: TassadarSudoku9x9ArticleReproducerView = read_repo_json(
        SUDOKU_REPRODUCER_REPORT_REF,
        "sudoku_9x9_article_reproducer_report",
    )?;
    let session_artifact: ArticleExecutorSessionArtifactView = read_repo_json(
        HARD_SUDOKU_FAST_ROUTE_SESSION_ARTIFACT_REF,
        "article_hard_sudoku_fast_route_session_artifact",
    )?;
    let hybrid_artifact: ArticleHybridWorkflowArtifactView = read_repo_json(
        HARD_SUDOKU_FAST_ROUTE_HYBRID_ARTIFACT_REF,
        "article_hard_sudoku_fast_route_hybrid_workflow_artifact",
    )?;
    let runtime_bundle: TassadarArticleHardSudokuBenchmarkBundle = read_repo_json(
        TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_REF,
        "article_hard_sudoku_benchmark_bundle",
    )?;
    build_report_from_inputs(
        acceptance_gate,
        frontend_report,
        suite_manifest,
        no_tool_proof,
        session_artifact,
        hybrid_artifact,
        runtime_bundle,
    )
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    frontend_report: TassadarArticleDemoFrontendParityReport,
    suite_manifest: TassadarArticleHardSudokuSuiteManifest,
    no_tool_proof: TassadarSudoku9x9ArticleReproducerView,
    session_artifact: ArticleExecutorSessionArtifactView,
    hybrid_artifact: ArticleHybridWorkflowArtifactView,
    runtime_bundle: TassadarArticleHardSudokuBenchmarkBundle,
) -> Result<
    TassadarArticleHardSudokuBenchmarkClosureReport,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let acceptance_gate_tie = TassadarArticleHardSudokuBenchmarkAcceptanceGateTie {
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
    let manifest_review = build_manifest_review(&suite_manifest)?;
    let frontend_review = build_frontend_review(&frontend_report)?;
    let no_tool_proof_review = build_no_tool_proof_review(&no_tool_proof);
    let expected_outputs = expected_hard_sudoku_outputs()?;
    let case_roles = manifest_case_roles(&suite_manifest);
    let fast_route_session_review = build_session_review(
        &suite_manifest,
        &session_artifact,
        &expected_outputs,
        &case_roles,
    )?;
    let fast_route_hybrid_review = build_hybrid_review(
        &suite_manifest,
        &hybrid_artifact,
        &expected_outputs,
        &case_roles,
    )?;
    let runtime_review = build_runtime_review(&suite_manifest, &runtime_bundle)?;
    let binding_review = build_binding_review(
        &manifest_review,
        &frontend_review,
        &no_tool_proof_review,
        &fast_route_session_review,
        &fast_route_hybrid_review,
        &runtime_review,
    );
    let named_arto_green = fast_route_session_review.named_arto_green
        && fast_route_hybrid_review.named_arto_green
        && runtime_review.named_arto_green;
    let hard_sudoku_suite_green = manifest_review.manifest_green
        && fast_route_session_review.suite_green
        && fast_route_hybrid_review.suite_green
        && runtime_review.suite_green;
    let hard_sudoku_benchmark_closure_green = acceptance_gate_tie.tied_requirement_satisfied
        && frontend_review.row_green
        && no_tool_proof_review.no_tool_proof_green
        && named_arto_green
        && hard_sudoku_suite_green
        && binding_review.binding_green;

    let mut report = TassadarArticleHardSudokuBenchmarkClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_hard_sudoku_benchmark_closure.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        manifest_review,
        frontend_review,
        no_tool_proof_review,
        fast_route_session_review,
        fast_route_hybrid_review,
        runtime_review,
        binding_review,
        named_arto_green,
        hard_sudoku_suite_green,
        hard_sudoku_benchmark_closure_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && hard_sudoku_benchmark_closure_green,
        claim_boundary: String::from(
            "this report closes TAS-181 only. It binds the canonical Sudoku article source, the existing no-tool Sudoku reproducer, the declared hard-Sudoku suite manifest, two served HullCache artifacts, and the runtime hard-Sudoku benchmark bundle into one machine-readable closure surface for the named Arto Inkala instance plus the declared benchmark suite. It does not yet claim the later unified demo-and-benchmark equivalence gate, no-spill single-run closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Hard-Sudoku benchmark closure now records tied_requirement_satisfied={}, manifest_green={}, frontend_green={}, no_tool_proof_green={}, session_suite_green={}, hybrid_suite_green={}, runtime_suite_green={}, named_arto_green={}, binding_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.manifest_review.manifest_green,
        report.frontend_review.row_green,
        report.no_tool_proof_review.no_tool_proof_green,
        report.fast_route_session_review.suite_green,
        report.fast_route_hybrid_review.suite_green,
        report.runtime_review.suite_green,
        report.named_arto_green,
        report.binding_review.binding_green,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_hard_sudoku_benchmark_closure_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_article_hard_sudoku_benchmark_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_article_hard_sudoku_benchmark_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleHardSudokuBenchmarkClosureReport,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleHardSudokuBenchmarkClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_hard_sudoku_benchmark_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleHardSudokuBenchmarkClosureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_manifest_review(
    manifest: &TassadarArticleHardSudokuSuiteManifest,
) -> Result<
    TassadarArticleHardSudokuManifestReview,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let manifest_bytes = read_repo_bytes(HARD_SUDOKU_SUITE_MANIFEST_REF)?;
    let declared_case_ids = manifest
        .case_rows
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let manifest_green = manifest.gate_issue_id == TIED_REQUIREMENT_ID
        && manifest.runtime_ceiling_seconds == 180.0
        && declared_case_ids.len() == 2
        && declared_case_ids
            .iter()
            .any(|case_id| case_id == &manifest.named_case_id);
    Ok(TassadarArticleHardSudokuManifestReview {
        manifest_ref: manifest.manifest_ref.clone(),
        manifest_id: manifest.manifest_id.clone(),
        manifest_source_digest: hex::encode(Sha256::digest(manifest_bytes)),
        gate_issue_id: manifest.gate_issue_id.clone(),
        source_ref: manifest.source_ref.clone(),
        runtime_ceiling_seconds: manifest.runtime_ceiling_seconds,
        declared_case_ids,
        named_case_id: manifest.named_case_id.clone(),
        manifest_green,
        detail: String::from(
            "the hard-Sudoku suite manifest must freeze the TAS-181 case set, source anchor, and 180 second runtime ceiling before the benchmark closure can be called green",
        ),
    })
}

fn build_frontend_review(
    frontend_report: &TassadarArticleDemoFrontendParityReport,
) -> Result<
    TassadarArticleHardSudokuFrontendReview,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let row = frontend_report
        .demo_rows
        .iter()
        .find(|row| row.demo_id == TassadarArticleDemoFrontendFamily::Sudoku9x9)
        .ok_or_else(
            || TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                detail: String::from("missing Sudoku frontend parity row"),
            },
        )?;
    Ok(TassadarArticleHardSudokuFrontendReview {
        report_ref: String::from(TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF),
        demo_id: row.demo_id,
        source_case_id: row.source_case_id.clone(),
        canonical_case_id: row.canonical_case_id.clone(),
        canonical_workload_family_id: row.canonical_workload_family_id.clone(),
        source_ref: row.source_ref.clone(),
        source_digest: row.source_digest.clone(),
        compile_receipt_digest: row.compile_receipt_digest.clone(),
        row_green: row.row_green,
        detail: String::from(
            "the hard-Sudoku tranche reuses the committed TAS-178 Sudoku frontend row so the named Arto fixture cannot drift away from the canonical article source surface",
        ),
    })
}

fn build_no_tool_proof_review(
    no_tool_proof: &TassadarSudoku9x9ArticleReproducerView,
) -> TassadarArticleHardSudokuNoToolProofReview {
    let no_tool_proof_green = no_tool_proof.exact_trace_match
        && no_tool_proof.final_output_match
        && no_tool_proof.halt_match
        && !no_tool_proof.direct_execution_posture.fallback_observed
        && !no_tool_proof
            .direct_execution_posture
            .external_tool_surface_observed;
    TassadarArticleHardSudokuNoToolProofReview {
        report_ref: String::from(SUDOKU_REPRODUCER_REPORT_REF),
        canonical_case_id: no_tool_proof.canonical_case_id.clone(),
        workload_family_id: no_tool_proof.workload_family_id.clone(),
        source_ref: no_tool_proof.source_ref.clone(),
        source_digest: no_tool_proof.source_digest.clone(),
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
        detail: String::from(
            "the existing Sudoku article reproducer remains the no-tool source anchor for the declared hard-Sudoku family, even though TAS-181 widens the served fast-route case set beyond that one canonical reproducer case",
        ),
    }
}

fn build_session_review(
    manifest: &TassadarArticleHardSudokuSuiteManifest,
    artifact: &ArticleExecutorSessionArtifactView,
    expected_outputs: &BTreeMap<String, Vec<i32>>,
    case_roles: &BTreeMap<String, String>,
) -> Result<
    TassadarArticleHardSudokuFastRouteSessionReview,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let mut case_reviews = Vec::with_capacity(manifest.case_rows.len());
    for row in &manifest.case_rows {
        let artifact_case = artifact
            .cases
            .iter()
            .find(|case| case.request.article_case_id == row.case_id)
            .ok_or_else(
                || TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                    detail: format!("missing session artifact case for `{}`", row.case_id),
                },
            )?;
        let response = artifact_case.outcome.response.as_ref().ok_or_else(|| {
            TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                detail: format!("session artifact case `{}` did not complete", row.case_id),
            }
        })?;
        let expected = expected_outputs.get(&row.case_id).ok_or_else(|| {
            TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                detail: format!("missing expected outputs for `{}`", row.case_id),
            }
        })?;
        let output_parity_green = response
            .executor_response
            .execution_report
            .execution
            .outputs
            == *expected;
        let fast_route_direct_green = artifact_case.outcome.status == "completed"
            && response.benchmark_identity.case_id == row.case_id
            && artifact_case.request.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
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
        case_reviews.push(TassadarArticleHardSudokuFastRouteCaseReview {
            case_name: artifact_case.name.clone(),
            case_id: row.case_id.clone(),
            case_role: case_roles
                .get(&row.case_id)
                .cloned()
                .unwrap_or_else(|| row.case_role.clone()),
            requested_decode_mode: artifact_case.request.requested_decode_mode,
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
            model_id: response.executor_response.model_descriptor.model.model_id.clone(),
            output_values: response.executor_response.execution_report.execution.outputs.clone(),
            output_parity_green,
            fast_route_direct_green,
            detail: format!(
                "session artifact case `{}` must stay direct on HullCache and match the committed expected Sudoku outputs",
                row.case_id
            ),
        });
    }
    let named_arto_green = case_reviews
        .iter()
        .any(|review| review.case_id == manifest.named_case_id && review.fast_route_direct_green);
    let suite_green = case_reviews
        .iter()
        .all(|review| review.fast_route_direct_green);
    Ok(TassadarArticleHardSudokuFastRouteSessionReview {
        artifact_ref: String::from(HARD_SUDOKU_FAST_ROUTE_SESSION_ARTIFACT_REF),
        case_reviews,
        named_arto_green,
        suite_green,
        detail: String::from(
            "the direct article-session artifact must keep both declared hard-Sudoku cases exact on the canonical HullCache fast route",
        ),
    })
}

fn build_hybrid_review(
    manifest: &TassadarArticleHardSudokuSuiteManifest,
    artifact: &ArticleHybridWorkflowArtifactView,
    expected_outputs: &BTreeMap<String, Vec<i32>>,
    case_roles: &BTreeMap<String, String>,
) -> Result<
    TassadarArticleHardSudokuFastRouteHybridReview,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let mut case_reviews = Vec::with_capacity(manifest.case_rows.len());
    for row in &manifest.case_rows {
        let artifact_case = artifact
            .cases
            .iter()
            .find(|case| case.request.article_case_id == row.case_id)
            .ok_or_else(
                || TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                    detail: format!("missing hybrid artifact case for `{}`", row.case_id),
                },
            )?;
        let response = artifact_case.outcome.response.as_ref().ok_or_else(|| {
            TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                detail: format!("hybrid artifact case `{}` did not complete", row.case_id),
            }
        })?;
        let expected = expected_outputs.get(&row.case_id).ok_or_else(|| {
            TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                detail: format!("missing expected outputs for `{}`", row.case_id),
            }
        })?;
        let output_parity_green = response
            .planner_response
            .executor_response
            .execution_report
            .execution
            .outputs
            == *expected;
        let fast_route_direct_green = artifact_case.outcome.status == "completed"
            && response.benchmark_identity.case_id == row.case_id
            && artifact_case.request.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
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
        case_reviews.push(TassadarArticleHardSudokuFastRouteCaseReview {
            case_name: artifact_case.name.clone(),
            case_id: row.case_id.clone(),
            case_role: case_roles
                .get(&row.case_id)
                .cloned()
                .unwrap_or_else(|| row.case_role.clone()),
            requested_decode_mode: artifact_case.request.requested_decode_mode,
            effective_decode_mode: response
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
                "hybrid artifact case `{}` must stay delegated but direct on HullCache and match the committed expected Sudoku outputs",
                row.case_id
            ),
        });
    }
    let named_arto_green = case_reviews
        .iter()
        .any(|review| review.case_id == manifest.named_case_id && review.fast_route_direct_green);
    let suite_green = case_reviews
        .iter()
        .all(|review| review.fast_route_direct_green);
    Ok(TassadarArticleHardSudokuFastRouteHybridReview {
        artifact_ref: String::from(HARD_SUDOKU_FAST_ROUTE_HYBRID_ARTIFACT_REF),
        case_reviews,
        named_arto_green,
        suite_green,
        detail: String::from(
            "the planner-owned hybrid artifact must keep both declared hard-Sudoku cases delegated but still direct on the canonical HullCache fast route",
        ),
    })
}

fn build_runtime_review(
    manifest: &TassadarArticleHardSudokuSuiteManifest,
    bundle: &TassadarArticleHardSudokuBenchmarkBundle,
) -> Result<
    TassadarArticleHardSudokuRuntimeReview,
    TassadarArticleHardSudokuBenchmarkClosureReportError,
> {
    let mut case_reviews = Vec::with_capacity(manifest.case_rows.len());
    for row in &manifest.case_rows {
        let receipt = bundle
            .case_receipts
            .iter()
            .find(|receipt| receipt.case_id == row.case_id)
            .ok_or_else(
                || TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                    detail: format!("missing runtime bundle case for `{}`", row.case_id),
                },
            )?;
        let runtime_green = receipt.selection_state == TassadarExecutorSelectionState::Direct
            && receipt.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
            && receipt.exactness_green
            && receipt.under_runtime_ceiling;
        case_reviews.push(TassadarArticleHardSudokuRuntimeCaseReview {
            case_id: receipt.case_id.clone(),
            case_role: receipt.case_role.clone(),
            requested_decode_mode: receipt.requested_decode_mode,
            effective_decode_mode: receipt.effective_decode_mode,
            selection_state: receipt.selection_state,
            measured_run_time_seconds: receipt.measured_run_time_seconds,
            runtime_ceiling_seconds: receipt.runtime_ceiling_seconds,
            exact_output_match: receipt.exact_output_match,
            exact_behavior_match: receipt.exact_behavior_match,
            halt_reason_match: receipt.halt_reason_match,
            exactness_green: receipt.exactness_green,
            under_runtime_ceiling: receipt.under_runtime_ceiling,
            runtime_green,
            detail: format!(
                "runtime bundle case `{}` must stay exact and under the declared article ceiling on HullCache",
                receipt.case_id
            ),
        });
    }
    Ok(TassadarArticleHardSudokuRuntimeReview {
        bundle_ref: String::from(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_REF),
        runtime_ceiling_seconds: bundle.runtime_ceiling_seconds,
        case_reviews,
        named_arto_green: bundle.named_arto_green,
        suite_green: bundle.hard_sudoku_suite_green,
        detail: String::from(
            "the runtime hard-Sudoku bundle is the machine-readable authority for exactness plus the under-three-minute ceiling across the declared suite",
        ),
    })
}

fn build_binding_review(
    manifest_review: &TassadarArticleHardSudokuManifestReview,
    frontend_review: &TassadarArticleHardSudokuFrontendReview,
    no_tool_proof_review: &TassadarArticleHardSudokuNoToolProofReview,
    session_review: &TassadarArticleHardSudokuFastRouteSessionReview,
    hybrid_review: &TassadarArticleHardSudokuFastRouteHybridReview,
    runtime_review: &TassadarArticleHardSudokuRuntimeReview,
) -> TassadarArticleHardSudokuBindingReview {
    let declared_case_ids = manifest_review
        .declared_case_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let session_case_ids = session_review
        .case_reviews
        .iter()
        .map(|review| review.case_id.clone())
        .collect::<BTreeSet<_>>();
    let hybrid_case_ids = hybrid_review
        .case_reviews
        .iter()
        .map(|review| review.case_id.clone())
        .collect::<BTreeSet<_>>();
    let runtime_case_ids = runtime_review
        .case_reviews
        .iter()
        .map(|review| review.case_id.clone())
        .collect::<BTreeSet<_>>();
    let manifest_case_alignment_green = declared_case_ids
        == expected_hard_sudoku_case_ids()
            .into_iter()
            .collect::<BTreeSet<_>>();
    let frontend_source_alignment_green = frontend_review.source_case_id == "sudoku_9x9_article"
        && frontend_review
            .source_ref
            .ends_with("tassadar_sudoku_9x9_article.rs")
        && frontend_review.row_green;
    let no_tool_anchor_alignment_green = no_tool_proof_review.canonical_case_id
        == "sudoku_9x9_test_a"
        && no_tool_proof_review.no_tool_proof_green;
    let session_case_alignment_green = session_case_ids == declared_case_ids;
    let hybrid_case_alignment_green = hybrid_case_ids == declared_case_ids;
    let runtime_case_alignment_green = runtime_case_ids == declared_case_ids;
    let named_arto_alignment_green = session_review.named_arto_green
        && hybrid_review.named_arto_green
        && runtime_review.named_arto_green
        && declared_case_ids.contains(&manifest_review.named_case_id);
    let binding_green = manifest_case_alignment_green
        && frontend_source_alignment_green
        && no_tool_anchor_alignment_green
        && session_case_alignment_green
        && hybrid_case_alignment_green
        && runtime_case_alignment_green
        && named_arto_alignment_green;
    TassadarArticleHardSudokuBindingReview {
        manifest_case_alignment_green,
        frontend_source_alignment_green,
        no_tool_anchor_alignment_green,
        session_case_alignment_green,
        hybrid_case_alignment_green,
        runtime_case_alignment_green,
        named_arto_alignment_green,
        binding_green,
        detail: String::from(
            "the TAS-181 closure surface stays green only when the manifest, frontend source anchor, no-tool Sudoku anchor, served fast-route artifacts, and runtime bundle all talk about the same declared two-case hard-Sudoku suite",
        ),
    }
}

fn expected_hard_sudoku_outputs(
) -> Result<BTreeMap<String, Vec<i32>>, TassadarArticleHardSudokuBenchmarkClosureReportError> {
    let mut outputs = BTreeMap::new();
    for case in tassadar_article_hard_sudoku_suite() {
        outputs.insert(case.case_id, case.validation_case.expected_outputs);
    }
    if outputs.is_empty() {
        return Err(
            TassadarArticleHardSudokuBenchmarkClosureReportError::Invariant {
                detail: String::from("hard-Sudoku suite unexpectedly empty"),
            },
        );
    }
    Ok(outputs)
}

fn expected_hard_sudoku_case_ids() -> Vec<String> {
    tassadar_article_hard_sudoku_suite()
        .into_iter()
        .map(|case| case.case_id)
        .collect()
}

fn manifest_case_roles(
    manifest: &TassadarArticleHardSudokuSuiteManifest,
) -> BTreeMap<String, String> {
    manifest
        .case_rows
        .iter()
        .map(|row| (row.case_id.clone(), row.case_role.clone()))
        .collect()
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

fn read_repo_bytes(
    relative_path: &str,
) -> Result<Vec<u8>, TassadarArticleHardSudokuBenchmarkClosureReportError> {
    let path = repo_root().join(relative_path);
    fs::read(&path).map_err(
        |error| TassadarArticleHardSudokuBenchmarkClosureReportError::Read {
            path: path.display().to_string(),
            error,
        },
    )
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleHardSudokuBenchmarkClosureReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleHardSudokuBenchmarkClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleHardSudokuBenchmarkClosureReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_hard_sudoku_benchmark_closure_report, read_repo_json,
        tassadar_article_hard_sudoku_benchmark_closure_report_path,
        write_tassadar_article_hard_sudoku_benchmark_closure_report,
        TassadarArticleHardSudokuBenchmarkClosureReport,
        TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_REPORT_REF,
    };

    #[test]
    fn article_hard_sudoku_benchmark_closure_tracks_green_named_arto_and_suite() {
        let report = build_tassadar_article_hard_sudoku_benchmark_closure_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.manifest_review.manifest_green);
        assert!(report.frontend_review.row_green);
        assert!(report.no_tool_proof_review.no_tool_proof_green);
        assert!(report.fast_route_session_review.suite_green);
        assert!(report.fast_route_hybrid_review.suite_green);
        assert!(report.runtime_review.suite_green);
        assert!(report.named_arto_green);
        assert!(report.hard_sudoku_suite_green);
        assert!(report.binding_review.binding_green);
        assert!(report.hard_sudoku_benchmark_closure_green);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_hard_sudoku_benchmark_closure_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated =
            build_tassadar_article_hard_sudoku_benchmark_closure_report().expect("report");
        let committed: TassadarArticleHardSudokuBenchmarkClosureReport = read_repo_json(
            TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_CLOSURE_REPORT_REF,
            "article_hard_sudoku_benchmark_closure_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_hard_sudoku_benchmark_closure_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_hard_sudoku_benchmark_closure_report.json");
        let written = write_tassadar_article_hard_sudoku_benchmark_closure_report(&output_path)?;
        let persisted: TassadarArticleHardSudokuBenchmarkClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_hard_sudoku_benchmark_closure_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_hard_sudoku_benchmark_closure_report.json")
        );
        Ok(())
    }
}
