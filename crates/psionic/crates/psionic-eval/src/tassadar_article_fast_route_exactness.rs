use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_environments::TassadarWorkloadTarget;
use psionic_models::TassadarArticleTransformer;
use psionic_runtime::{TassadarExecutorDecodeMode, TassadarExecutorSelectionState};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarCapabilityPosture,
    TassadarHullCacheClosureReport, TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_report.json";
pub const TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_CHECKER_REF: &str =
    "scripts/check-tassadar-article-fast-route-exactness.sh";

const TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_implementation_report.json";
const ARTICLE_FAST_ROUTE_EXACTNESS_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_session_artifact.json";
const ARTICLE_FAST_ROUTE_EXACTNESS_HYBRID_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_hybrid_workflow_artifact.json";
const TIED_REQUIREMENT_ID: &str = "TAS-174";
const ARTICLE_SESSION_CASES: [(&str, &str); 3] = [
    ("direct_long_loop_hull", "long_loop_kernel"),
    ("direct_sudoku_v0_hull", "sudoku_v0_test_a"),
    ("direct_hungarian_hull", "hungarian_matching"),
];
const ARTICLE_HYBRID_CASES: [(&str, &str); 3] = [
    ("delegated_long_loop_hull", "long_loop_kernel"),
    ("delegated_sudoku_v0_hull", "sudoku_v0_test_a"),
    ("delegated_hungarian_hull", "hungarian_matching"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessImplementationPrerequisite {
    pub report_ref: String,
    pub selected_candidate_kind: String,
    pub fast_route_implementation_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessClosureReview {
    pub report_ref: String,
    pub long_loop_direct_case_count: usize,
    pub sudoku_direct_case_count: usize,
    pub hungarian_direct_case_count: usize,
    pub long_loop_fallback_case_count: usize,
    pub sudoku_fallback_case_count: usize,
    pub hungarian_fallback_case_count: usize,
    pub all_article_workloads_exact: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessSessionCaseReview {
    pub artifact_ref: String,
    pub case_name: String,
    pub case_id: String,
    pub model_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub exact_direct_hull_cache: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessHybridCaseReview {
    pub artifact_ref: String,
    pub case_name: String,
    pub case_id: String,
    pub model_id: String,
    pub planner_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub executor_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub exact_direct_hull_cache: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleFastRouteExactnessAcceptanceGateTie,
    pub implementation_prerequisite: TassadarArticleFastRouteExactnessImplementationPrerequisite,
    pub hull_cache_closure_review: TassadarArticleFastRouteExactnessClosureReview,
    pub article_session_reviews: Vec<TassadarArticleFastRouteExactnessSessionCaseReview>,
    pub hybrid_route_reviews: Vec<TassadarArticleFastRouteExactnessHybridCaseReview>,
    pub exactness_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct FastRouteImplementationReportView {
    selected_candidate_kind: String,
    fast_route_implementation_green: bool,
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
    executor_response: ArticleExecutorResponseView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorResponseView {
    model_descriptor: psionic_models::TassadarExecutorModelDescriptor,
    execution_report: ArticleExecutorExecutionReportView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorExecutionReportView {
    selection: ArticleExecutorSelectionView,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleExecutorSelectionView {
    requested_decode_mode: TassadarExecutorDecodeMode,
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
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
pub enum TassadarArticleFastRouteExactnessReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
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
    #[error("internal TAS-174 invariant failed: {detail}")]
    Invariant { detail: String },
}

pub fn build_tassadar_article_fast_route_exactness_report(
) -> Result<TassadarArticleFastRouteExactnessReport, TassadarArticleFastRouteExactnessReportError> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let implementation_report: FastRouteImplementationReportView = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
        "article_fast_route_implementation_report",
    )?;
    let hull_cache_closure: TassadarHullCacheClosureReport = read_repo_json(
        TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF,
        "tassadar_hull_cache_closure_report",
    )?;
    let article_session_artifact: ArticleExecutorSessionArtifactView = read_repo_json(
        ARTICLE_FAST_ROUTE_EXACTNESS_SESSION_ARTIFACT_REF,
        "article_fast_route_exactness_session_artifact",
    )?;
    let article_hybrid_artifact: ArticleHybridWorkflowArtifactView = read_repo_json(
        ARTICLE_FAST_ROUTE_EXACTNESS_HYBRID_ARTIFACT_REF,
        "article_fast_route_exactness_hybrid_workflow_artifact",
    )?;

    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate)?;
    let implementation_prerequisite = build_implementation_prerequisite(&implementation_report);
    let hull_cache_closure_review = build_hull_cache_closure_review(&hull_cache_closure)?;
    let article_session_reviews = ARTICLE_SESSION_CASES
        .into_iter()
        .map(|(case_name, case_id)| {
            build_article_session_case_review(&article_session_artifact, case_name, case_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let hybrid_route_reviews = ARTICLE_HYBRID_CASES
        .into_iter()
        .map(|(case_name, case_id)| {
            build_article_hybrid_case_review(&article_hybrid_artifact, case_name, case_id)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let exactness_green = acceptance_gate_tie.tied_requirement_satisfied
        && implementation_prerequisite.fast_route_implementation_green
        && hull_cache_closure_review.all_article_workloads_exact
        && article_session_reviews
            .iter()
            .all(|review| review.exact_direct_hull_cache)
        && hybrid_route_reviews
            .iter()
            .all(|review| review.exact_direct_hull_cache);

    let mut report = TassadarArticleFastRouteExactnessReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_fast_route_exactness.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        implementation_prerequisite,
        hull_cache_closure_review,
        article_session_reviews,
        hybrid_route_reviews,
        exactness_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && exactness_green,
        claim_boundary: String::from(
            "this report closes only TAS-174. It proves the selected HullCache fast path is now exact and no-fallback on the declared canonical article workload families inside the current article profile, and that the served article-session plus hybrid route surfaces expose those workloads as direct HullCache executions on the trained Transformer-backed model. It does not yet claim throughput-floor closure or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article fast-route exactness report now records tied_requirement_satisfied={}, implementation_prerequisite_green={}, all_article_workloads_exact={}, article_session_direct_cases={}, hybrid_direct_cases={}, exactness_green={}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report
            .implementation_prerequisite
            .fast_route_implementation_green,
        report.hull_cache_closure_review.all_article_workloads_exact,
        report
            .article_session_reviews
            .iter()
            .filter(|review| review.exact_direct_hull_cache)
            .count(),
        report
            .hybrid_route_reviews
            .iter()
            .filter(|review| review.exact_direct_hull_cache)
            .count(),
        report.exactness_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_fast_route_exactness_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_fast_route_exactness_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_exactness_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleFastRouteExactnessReport, TassadarArticleFastRouteExactnessReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteExactnessReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_fast_route_exactness_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteExactnessReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> Result<
    TassadarArticleFastRouteExactnessAcceptanceGateTie,
    TassadarArticleFastRouteExactnessReportError,
> {
    let tied_requirement = acceptance_gate
        .requirement_rows
        .iter()
        .find(|row| row.requirement_id == TIED_REQUIREMENT_ID)
        .ok_or_else(|| TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!("acceptance gate missing requirement `{TIED_REQUIREMENT_ID}`"),
        })?;
    Ok(TassadarArticleFastRouteExactnessAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: tied_requirement.satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    })
}

fn build_implementation_prerequisite(
    implementation_report: &FastRouteImplementationReportView,
) -> TassadarArticleFastRouteExactnessImplementationPrerequisite {
    TassadarArticleFastRouteExactnessImplementationPrerequisite {
        report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
        selected_candidate_kind: implementation_report.selected_candidate_kind.clone(),
        fast_route_implementation_green: implementation_report.fast_route_implementation_green,
        detail: String::from(
            "TAS-174 builds on TAS-173, so the canonical trained Transformer-backed article route must already own the selected HullCache path before exactness and no-fallback closure can turn green",
        ),
    }
}

fn build_hull_cache_closure_review(
    hull_cache_closure: &TassadarHullCacheClosureReport,
) -> Result<
    TassadarArticleFastRouteExactnessClosureReview,
    TassadarArticleFastRouteExactnessReportError,
> {
    let long_loop = closure_row(hull_cache_closure, TassadarWorkloadTarget::LongLoopKernel)?;
    let sudoku = closure_row(hull_cache_closure, TassadarWorkloadTarget::SudokuClass)?;
    let hungarian = closure_row(
        hull_cache_closure,
        TassadarWorkloadTarget::HungarianMatching,
    )?;
    let all_article_workloads_exact = long_loop.posture == TassadarCapabilityPosture::Exact
        && sudoku.posture == TassadarCapabilityPosture::Exact
        && hungarian.posture == TassadarCapabilityPosture::Exact
        && long_loop.fallback_case_count == 0
        && sudoku.fallback_case_count == 0
        && hungarian.fallback_case_count == 0;

    Ok(TassadarArticleFastRouteExactnessClosureReview {
        report_ref: String::from(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF),
        long_loop_direct_case_count: long_loop.direct_case_count,
        sudoku_direct_case_count: sudoku.direct_case_count,
        hungarian_direct_case_count: hungarian.direct_case_count,
        long_loop_fallback_case_count: long_loop.fallback_case_count,
        sudoku_fallback_case_count: sudoku.fallback_case_count,
        hungarian_fallback_case_count: hungarian.fallback_case_count,
        all_article_workloads_exact,
        detail: String::from(
            "the widened HullCache closure report must now show LongLoopKernel, SudokuClass, and HungarianMatching as exact with zero fallback cases before TAS-174 can close",
        ),
    })
}

fn closure_row(
    hull_cache_closure: &TassadarHullCacheClosureReport,
    workload_target: TassadarWorkloadTarget,
) -> Result<crate::TassadarHullCacheWorkloadSummary, TassadarArticleFastRouteExactnessReportError> {
    hull_cache_closure
        .exact_workloads
        .iter()
        .chain(hull_cache_closure.fallback_only_workloads.iter())
        .find(|row| row.workload_target == workload_target)
        .cloned()
        .ok_or_else(|| TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!(
                "HullCache closure report is missing workload `{}`",
                workload_target_id(workload_target)
            ),
        })
}

fn build_article_session_case_review(
    artifact: &ArticleExecutorSessionArtifactView,
    case_name: &str,
    case_id: &str,
) -> Result<
    TassadarArticleFastRouteExactnessSessionCaseReview,
    TassadarArticleFastRouteExactnessReportError,
> {
    let case = artifact
        .cases
        .iter()
        .find(|entry| entry.name == case_name)
        .ok_or_else(|| TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!("article-session artifact is missing case `{case_name}`"),
        })?;
    if case.request.article_case_id != case_id {
        return Err(TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!(
                "article-session case `{case_name}` targeted `{}`, expected `{case_id}`",
                case.request.article_case_id
            ),
        });
    }
    let response = case.outcome.response.as_ref().ok_or_else(|| {
        TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!(
                "article-session case `{case_name}` did not complete on the fast route"
            ),
        }
    })?;
    let model_id = response
        .executor_response
        .model_descriptor
        .model
        .model_id
        .clone();
    let selection = &response.executor_response.execution_report.selection;
    let exact_direct_hull_cache = case.outcome.status == "completed"
        && model_id == TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        && selection.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        && selection.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
        && selection.selection_state == TassadarExecutorSelectionState::Direct;
    Ok(TassadarArticleFastRouteExactnessSessionCaseReview {
        artifact_ref: String::from(ARTICLE_FAST_ROUTE_EXACTNESS_SESSION_ARTIFACT_REF),
        case_name: String::from(case_name),
        case_id: String::from(case_id),
        model_id,
        requested_decode_mode: selection.requested_decode_mode,
        effective_decode_mode: selection.effective_decode_mode,
        selection_state: selection.selection_state,
        exact_direct_hull_cache,
        detail: String::from(
            "the canonical article-session surface must complete each representative article workload as a direct HullCache execution on the trained Transformer-backed model",
        ),
    })
}

fn build_article_hybrid_case_review(
    artifact: &ArticleHybridWorkflowArtifactView,
    case_name: &str,
    case_id: &str,
) -> Result<
    TassadarArticleFastRouteExactnessHybridCaseReview,
    TassadarArticleFastRouteExactnessReportError,
> {
    let case = artifact
        .cases
        .iter()
        .find(|entry| entry.name == case_name)
        .ok_or_else(|| TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!("article-hybrid artifact is missing case `{case_name}`"),
        })?;
    if case.request.article_case_id != case_id {
        return Err(TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!(
                "article-hybrid case `{case_name}` targeted `{}`, expected `{case_id}`",
                case.request.article_case_id
            ),
        });
    }
    let response = case.outcome.response.as_ref().ok_or_else(|| {
        TassadarArticleFastRouteExactnessReportError::Invariant {
            detail: format!("article-hybrid case `{case_name}` did not complete on the fast route"),
        }
    })?;
    let model_id = response
        .planner_response
        .executor_response
        .model_descriptor
        .model
        .model_id
        .clone();
    let selection = &response
        .planner_response
        .executor_response
        .execution_report
        .selection;
    let planner_effective_decode_mode = response
        .planner_response
        .routing_decision
        .effective_decode_mode;
    let exact_direct_hull_cache = case.outcome.status == "completed"
        && model_id == TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        && planner_effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
        && selection.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        && selection.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
        && selection.selection_state == TassadarExecutorSelectionState::Direct;
    Ok(TassadarArticleFastRouteExactnessHybridCaseReview {
        artifact_ref: String::from(ARTICLE_FAST_ROUTE_EXACTNESS_HYBRID_ARTIFACT_REF),
        case_name: String::from(case_name),
        case_id: String::from(case_id),
        model_id,
        planner_effective_decode_mode,
        executor_effective_decode_mode: selection.effective_decode_mode,
        selection_state: selection.selection_state,
        exact_direct_hull_cache,
        detail: String::from(
            "the canonical article hybrid workflow must delegate each representative article workload through the planner-owned direct HullCache route on the trained Transformer-backed model",
        ),
    })
}

fn workload_target_id(workload_target: TassadarWorkloadTarget) -> &'static str {
    match workload_target {
        TassadarWorkloadTarget::ArithmeticMicroprogram => "arithmetic_microprogram",
        TassadarWorkloadTarget::ClrsShortestPath => "clrs_shortest_path",
        TassadarWorkloadTarget::MemoryLookupMicroprogram => "memory_lookup_microprogram",
        TassadarWorkloadTarget::BranchControlFlowMicroprogram => "branch_control_flow_microprogram",
        TassadarWorkloadTarget::MicroWasmKernel => "micro_wasm_kernel",
        TassadarWorkloadTarget::BranchHeavyKernel => "branch_heavy_kernel",
        TassadarWorkloadTarget::MemoryHeavyKernel => "memory_heavy_kernel",
        TassadarWorkloadTarget::LongLoopKernel => "long_loop_kernel",
        TassadarWorkloadTarget::SudokuClass => "sudoku_class",
        TassadarWorkloadTarget::HungarianMatching => "hungarian_matching",
        TassadarWorkloadTarget::ModuleMemcpy => "module_memcpy",
        TassadarWorkloadTarget::ModuleParsing => "module_parsing",
        TassadarWorkloadTarget::ModuleChecksum => "module_checksum",
        TassadarWorkloadTarget::ModuleVmStyle => "module_vm_style",
    }
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
) -> Result<T, TassadarArticleFastRouteExactnessReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleFastRouteExactnessReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteExactnessReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_exactness_report, read_repo_json,
        tassadar_article_fast_route_exactness_report_path,
        write_tassadar_article_fast_route_exactness_report,
        TassadarArticleFastRouteExactnessReport, TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
    };

    #[test]
    fn fast_route_exactness_report_tracks_no_fallback_article_closure() {
        let report = build_tassadar_article_fast_route_exactness_report().expect("report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-174");
        assert!(
            report
                .implementation_prerequisite
                .fast_route_implementation_green
        );
        assert!(report.hull_cache_closure_review.all_article_workloads_exact);
        assert!(report
            .article_session_reviews
            .iter()
            .all(|review| review.exact_direct_hull_cache));
        assert!(report
            .hybrid_route_reviews
            .iter()
            .all(|review| review.exact_direct_hull_cache));
        assert!(report.exactness_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn fast_route_exactness_report_matches_committed_truth() {
        let generated = build_tassadar_article_fast_route_exactness_report().expect("report");
        let committed: TassadarArticleFastRouteExactnessReport =
            read_repo_json(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF, "report")
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_exactness_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_exactness_report.json");
        let written =
            write_tassadar_article_fast_route_exactness_report(&output_path).expect("write report");
        let persisted: TassadarArticleFastRouteExactnessReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_exactness_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_exactness_report.json")
        );
    }
}
