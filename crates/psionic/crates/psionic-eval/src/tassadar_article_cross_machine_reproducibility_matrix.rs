use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TassadarArticleCpuMachineClassStatus, TassadarExecutorDecodeMode,
    TassadarExecutorSelectionState,
};

use crate::{
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFastRouteCandidateKind,
    TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
    TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
    TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cross_machine_reproducibility_matrix_report.json";
pub const TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_CHECKER_REF: &str =
    "scripts/check-tassadar-article-cross-machine-reproducibility-matrix.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-185";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityMachineMatrixReview {
    pub report_ref: String,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub unsupported_machine_class_ids: Vec<String>,
    pub current_host_measured_green: bool,
    pub machine_class_alignment_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityRouteStabilityReview {
    pub architecture_selection_report_ref: String,
    pub exactness_report_ref: String,
    pub selected_candidate_kind: TassadarArticleFastRouteCandidateKind,
    pub selected_decode_mode: TassadarExecutorDecodeMode,
    pub transformer_model_id: String,
    pub direct_case_ids: Vec<String>,
    pub hybrid_case_ids: Vec<String>,
    pub direct_case_count: usize,
    pub hybrid_case_count: usize,
    pub all_direct_routes_hull_cache: bool,
    pub all_hybrid_routes_hull_cache: bool,
    pub route_stability_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityDemoReview {
    pub report_ref: String,
    pub hungarian_demo_case_id: String,
    pub sudoku_demo_case_ids: Vec<String>,
    pub canonical_demo_case_ids: Vec<String>,
    pub binding_green: bool,
    pub demo_benchmark_equivalence_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityLongHorizonReview {
    pub report_ref: String,
    pub selected_candidate_kind: String,
    pub selected_decode_mode: String,
    pub million_step_horizon_ids: Vec<String>,
    pub multi_million_step_horizon_ids: Vec<String>,
    pub all_horizon_rows_exact: bool,
    pub all_horizon_rows_fast_route_direct: bool,
    pub all_horizon_rows_floor_passed: bool,
    pub deterministic_exactness_green: bool,
    pub single_run_no_spill_closure_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityThroughputDriftReview {
    pub report_ref: String,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub allowed_floor_drift_bps: u16,
    pub drift_policy_green: bool,
    pub throughput_floor_green: bool,
    pub kernel_horizon_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityStochasticModeReview {
    pub report_ref: String,
    pub stochastic_mode_supported: bool,
    pub stochastic_mode_robustness_green: bool,
    pub out_of_scope: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCrossMachineExactnessPosture {
    ExactMeasuredCurrentHost,
    ExactRequiredDeclaredClass,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCrossMachineThroughputPosture {
    PassedMeasuredCurrentHost,
    PassedRequiredDeclaredClass,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleCrossMachineStochasticModePosture {
    OutOfScope,
    RobustMeasuredCurrentHost,
    RobustRequiredDeclaredClass,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityRow {
    pub machine_class_id: String,
    pub machine_class_status: TassadarArticleCpuMachineClassStatus,
    pub supported_machine_class: bool,
    pub deterministic_demo_exactness_posture: TassadarArticleCrossMachineExactnessPosture,
    pub deterministic_long_horizon_exactness_posture: TassadarArticleCrossMachineExactnessPosture,
    pub throughput_floor_stability_posture: TassadarArticleCrossMachineThroughputPosture,
    pub stochastic_mode_posture: TassadarArticleCrossMachineStochasticModePosture,
    pub route_stability_green: bool,
    pub row_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleCrossMachineReproducibilityAcceptanceGateTie,
    pub machine_matrix_review: TassadarArticleCrossMachineReproducibilityMachineMatrixReview,
    pub route_stability_review: TassadarArticleCrossMachineReproducibilityRouteStabilityReview,
    pub demo_review: TassadarArticleCrossMachineReproducibilityDemoReview,
    pub long_horizon_review: TassadarArticleCrossMachineReproducibilityLongHorizonReview,
    pub throughput_drift_review: TassadarArticleCrossMachineReproducibilityThroughputDriftReview,
    pub stochastic_mode_review: TassadarArticleCrossMachineReproducibilityStochasticModeReview,
    pub machine_rows: Vec<TassadarArticleCrossMachineReproducibilityRow>,
    pub deterministic_mode_green: bool,
    pub throughput_floor_stability_green: bool,
    pub reproducibility_matrix_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct AcceptanceGateReportView {
    acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    green_requirement_ids: Vec<String>,
    blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CpuReproducibilityReportView {
    matrix: CpuReproducibilityMatrixView,
    supported_machine_class_ids: Vec<String>,
    unsupported_machine_class_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CpuReproducibilityMatrixView {
    current_host_machine_class_id: String,
    current_host_measured_green: bool,
    rows: Vec<CpuReproducibilityRowView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CpuReproducibilityRowView {
    machine_class_id: String,
    status: TassadarArticleCpuMachineClassStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArchitectureSelectionReportView {
    selected_candidate_kind: TassadarArticleFastRouteCandidateKind,
    fast_route_selection_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct FastRouteExactnessReportView {
    exactness_green: bool,
    article_session_reviews: Vec<FastRouteSessionReviewView>,
    hybrid_route_reviews: Vec<FastRouteHybridReviewView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct FastRouteSessionReviewView {
    case_id: String,
    model_id: String,
    requested_decode_mode: TassadarExecutorDecodeMode,
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
    exact_direct_hull_cache: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct FastRouteHybridReviewView {
    case_id: String,
    planner_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    executor_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    selection_state: TassadarExecutorSelectionState,
    exact_direct_hull_cache: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DemoBenchmarkEquivalenceGateReportView {
    hungarian_review: DemoBenchmarkHungarianReviewView,
    benchmark_review: DemoBenchmarkHardSudokuReviewView,
    binding_review: DemoBenchmarkBindingReviewView,
    article_demo_benchmark_equivalence_gate_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DemoBenchmarkHungarianReviewView {
    canonical_case_id: String,
    hungarian_demo_parity_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DemoBenchmarkHardSudokuReviewView {
    declared_case_ids: Vec<String>,
    named_arto_parity_green: bool,
    benchmark_wide_sudoku_parity_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct DemoBenchmarkBindingReviewView {
    binding_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct SingleRunNoSpillClosureReportView {
    horizon_review: SingleRunNoSpillHorizonReviewView,
    stochastic_mode_review: SingleRunNoSpillStochasticModeReviewView,
    single_run_no_spill_closure_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct SingleRunNoSpillHorizonReviewView {
    selected_candidate_kind: String,
    selected_decode_mode: String,
    horizon_rows: Vec<SingleRunNoSpillHorizonRowView>,
    million_step_horizon_ids: Vec<String>,
    multi_million_step_horizon_ids: Vec<String>,
    deterministic_exactness_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct SingleRunNoSpillHorizonRowView {
    horizon_id: String,
    reference_linear_direct: bool,
    hull_cache_direct: bool,
    throughput_floor_passed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct SingleRunNoSpillStochasticModeReviewView {
    stochastic_mode_supported: bool,
    stochastic_mode_robustness_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ThroughputFloorReportView {
    cross_machine_drift_review: ThroughputDriftReviewView,
    throughput_bundle: ThroughputBundleView,
    throughput_floor_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ThroughputDriftReviewView {
    current_host_machine_class_id: String,
    supported_machine_class_ids: Vec<String>,
    allowed_floor_drift_bps: u16,
    drift_policy_green: bool,
    detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ThroughputBundleView {
    selected_candidate_kind: String,
    selected_decode_mode: TassadarExecutorDecodeMode,
    demo_receipts: Vec<ThroughputDemoReceiptView>,
    kernel_receipts: Vec<ThroughputKernelReceiptView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ThroughputDemoReceiptView {
    case_id: String,
    selection_state: TassadarExecutorSelectionState,
    effective_decode_mode: Option<TassadarExecutorDecodeMode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ThroughputKernelReceiptView {
    workload_horizon_id: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleCrossMachineReproducibilityReportError {
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

pub fn build_tassadar_article_cross_machine_reproducibility_matrix_report() -> Result<
    TassadarArticleCrossMachineReproducibilityReport,
    TassadarArticleCrossMachineReproducibilityReportError,
> {
    let acceptance_gate: AcceptanceGateReportView = read_repo_json(
        TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        "article_equivalence_acceptance_gate_report",
    )?;
    let cpu_report: CpuReproducibilityReportView = read_repo_json(
        TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
        "article_cpu_reproducibility_report",
    )?;
    let architecture_selection: ArchitectureSelectionReportView = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        "article_fast_route_architecture_selection_report",
    )?;
    let fast_route_exactness: FastRouteExactnessReportView = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
        "article_fast_route_exactness_report",
    )?;
    let demo_benchmark: DemoBenchmarkEquivalenceGateReportView = read_repo_json(
        TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF,
        "article_demo_benchmark_equivalence_gate_report",
    )?;
    let single_run_no_spill: SingleRunNoSpillClosureReportView = read_repo_json(
        TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
        "article_single_run_no_spill_closure_report",
    )?;
    let throughput_floor: ThroughputFloorReportView = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
        "article_fast_route_throughput_floor_report",
    )?;

    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate);
    let machine_matrix_review = build_machine_matrix_review(&cpu_report, &throughput_floor);
    let route_stability_review = build_route_stability_review(
        &architecture_selection,
        &fast_route_exactness,
        &throughput_floor,
    );
    let demo_review = build_demo_review(&demo_benchmark);
    let long_horizon_review = build_long_horizon_review(&single_run_no_spill);
    let throughput_drift_review = build_throughput_drift_review(&throughput_floor);
    let stochastic_mode_review = build_stochastic_mode_review(&single_run_no_spill);
    let machine_rows = cpu_report
        .matrix
        .rows
        .iter()
        .map(|row| {
            build_machine_row(
                row,
                &cpu_report,
                &route_stability_review,
                &demo_review,
                &long_horizon_review,
                &throughput_drift_review,
                &stochastic_mode_review,
            )
        })
        .collect::<Vec<_>>();

    let deterministic_mode_green = route_stability_review.route_stability_green
        && demo_review.demo_benchmark_equivalence_green
        && long_horizon_review.deterministic_exactness_green
        && long_horizon_review.single_run_no_spill_closure_green
        && machine_rows
            .iter()
            .filter(|row| row.supported_machine_class)
            .all(|row| {
                row.deterministic_demo_exactness_posture
                    != TassadarArticleCrossMachineExactnessPosture::Refused
                    && row.deterministic_long_horizon_exactness_posture
                        != TassadarArticleCrossMachineExactnessPosture::Refused
            });
    let throughput_floor_stability_green = throughput_drift_review.drift_policy_green
        && throughput_drift_review.throughput_floor_green
        && machine_rows
            .iter()
            .filter(|row| row.supported_machine_class)
            .all(|row| {
                row.throughput_floor_stability_posture
                    != TassadarArticleCrossMachineThroughputPosture::Refused
            });
    let reproducibility_matrix_green = acceptance_gate_tie.tied_requirement_satisfied
        && machine_matrix_review.current_host_measured_green
        && machine_matrix_review.machine_class_alignment_green
        && deterministic_mode_green
        && throughput_floor_stability_green
        && machine_rows.iter().all(|row| row.row_green)
        && (stochastic_mode_review.out_of_scope
            || stochastic_mode_review.stochastic_mode_robustness_green);

    let mut report = TassadarArticleCrossMachineReproducibilityReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.article_cross_machine_reproducibility_matrix.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        machine_matrix_review,
        route_stability_review,
        demo_review,
        long_horizon_review,
        throughput_drift_review,
        stochastic_mode_review,
        machine_rows,
        deterministic_mode_green,
        throughput_floor_stability_green,
        reproducibility_matrix_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && reproducibility_matrix_green,
        claim_boundary: String::from(
            "this report closes TAS-185 only. It freezes one cross-machine reproducibility matrix for the canonical fast article route over the declared `host_cpu_x86_64` and `host_cpu_aarch64` machine classes, binding deterministic demo exactness, deterministic long-horizon exactness, and throughput-floor stability to the selected HullCache route while keeping stochastic execution explicit and out of scope for the current canonical route. It does not by itself close route minimality, final publication verdicts, or article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Cross-machine reproducibility now records tied_requirement_satisfied={}, current_host_machine_class_id=`{}`, supported_machine_classes={}, deterministic_mode_green={}, throughput_floor_stability_green={}, stochastic_out_of_scope={}, and reproducibility_matrix_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.machine_matrix_review.current_host_machine_class_id,
        report.machine_matrix_review.supported_machine_class_ids.len(),
        report.deterministic_mode_green,
        report.throughput_floor_stability_green,
        report.stochastic_mode_review.out_of_scope,
        report.reproducibility_matrix_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_cross_machine_reproducibility_matrix_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_article_cross_machine_reproducibility_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF)
}

pub fn write_tassadar_article_cross_machine_reproducibility_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleCrossMachineReproducibilityReport,
    TassadarArticleCrossMachineReproducibilityReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleCrossMachineReproducibilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_cross_machine_reproducibility_matrix_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleCrossMachineReproducibilityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &AcceptanceGateReportView,
) -> TassadarArticleCrossMachineReproducibilityAcceptanceGateTie {
    let tied_requirement_satisfied = acceptance_gate
        .green_requirement_ids
        .iter()
        .any(|requirement_id| requirement_id == TIED_REQUIREMENT_ID);
    TassadarArticleCrossMachineReproducibilityAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    }
}

fn build_machine_matrix_review(
    cpu_report: &CpuReproducibilityReportView,
    throughput_floor: &ThroughputFloorReportView,
) -> TassadarArticleCrossMachineReproducibilityMachineMatrixReview {
    let machine_class_alignment_green = cpu_report.matrix.current_host_machine_class_id
        == throughput_floor
            .cross_machine_drift_review
            .current_host_machine_class_id
        && cpu_report.supported_machine_class_ids
            == throughput_floor
                .cross_machine_drift_review
                .supported_machine_class_ids;
    TassadarArticleCrossMachineReproducibilityMachineMatrixReview {
        report_ref: String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
        current_host_machine_class_id: cpu_report.matrix.current_host_machine_class_id.clone(),
        supported_machine_class_ids: cpu_report.supported_machine_class_ids.clone(),
        unsupported_machine_class_ids: cpu_report.unsupported_machine_class_ids.clone(),
        current_host_measured_green: cpu_report.matrix.current_host_measured_green,
        machine_class_alignment_green,
        detail: format!(
            "CPU matrix keeps current_host_machine_class_id=`{}`, current_host_measured_green={}, supported_classes={}, unsupported_classes={}, and machine_class_alignment_green={}.",
            cpu_report.matrix.current_host_machine_class_id,
            cpu_report.matrix.current_host_measured_green,
            cpu_report.supported_machine_class_ids.len(),
            cpu_report.unsupported_machine_class_ids.len(),
            machine_class_alignment_green,
        ),
    }
}

fn build_route_stability_review(
    architecture_selection: &ArchitectureSelectionReportView,
    fast_route_exactness: &FastRouteExactnessReportView,
    throughput_floor: &ThroughputFloorReportView,
) -> TassadarArticleCrossMachineReproducibilityRouteStabilityReview {
    let direct_case_ids = fast_route_exactness
        .article_session_reviews
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let hybrid_case_ids = fast_route_exactness
        .hybrid_route_reviews
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let all_direct_routes_hull_cache = fast_route_exactness.exactness_green
        && fast_route_exactness
            .article_session_reviews
            .iter()
            .all(|row| {
                row.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
                    && row.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
                    && row.selection_state == TassadarExecutorSelectionState::Direct
                    && row.exact_direct_hull_cache
            })
        && throughput_floor
            .throughput_bundle
            .demo_receipts
            .iter()
            .all(|row| {
                row.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
                    && row.selection_state == TassadarExecutorSelectionState::Direct
            });
    let all_hybrid_routes_hull_cache = fast_route_exactness.exactness_green
        && fast_route_exactness.hybrid_route_reviews.iter().all(|row| {
            row.planner_effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
                && row.executor_effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
                && row.selection_state == TassadarExecutorSelectionState::Direct
                && row.exact_direct_hull_cache
        });
    let route_stability_green = architecture_selection.fast_route_selection_green
        && architecture_selection.selected_candidate_kind
            == TassadarArticleFastRouteCandidateKind::HullCacheRuntime
        && throughput_floor.throughput_bundle.selected_candidate_kind == "hull_cache_runtime"
        && throughput_floor.throughput_bundle.selected_decode_mode
            == TassadarExecutorDecodeMode::HullCache
        && all_direct_routes_hull_cache
        && all_hybrid_routes_hull_cache;
    TassadarArticleCrossMachineReproducibilityRouteStabilityReview {
        architecture_selection_report_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        ),
        exactness_report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF),
        selected_candidate_kind: architecture_selection.selected_candidate_kind,
        selected_decode_mode: throughput_floor.throughput_bundle.selected_decode_mode,
        transformer_model_id: fast_route_exactness
            .article_session_reviews
            .first()
            .map(|row| row.model_id.clone())
            .unwrap_or_default(),
        direct_case_ids,
        hybrid_case_ids,
        direct_case_count: fast_route_exactness.article_session_reviews.len(),
        hybrid_case_count: fast_route_exactness.hybrid_route_reviews.len(),
        all_direct_routes_hull_cache,
        all_hybrid_routes_hull_cache,
        route_stability_green,
        detail: format!(
            "Route stability keeps selected_candidate_kind=`{:?}`, selected_decode_mode=`{}`, direct_case_count={}, hybrid_case_count={}, all_direct_routes_hull_cache={}, all_hybrid_routes_hull_cache={}, and route_stability_green={}.",
            architecture_selection.selected_candidate_kind,
            throughput_floor.throughput_bundle.selected_decode_mode.as_str(),
            fast_route_exactness.article_session_reviews.len(),
            fast_route_exactness.hybrid_route_reviews.len(),
            all_direct_routes_hull_cache,
            all_hybrid_routes_hull_cache,
            route_stability_green,
        ),
    }
}

fn build_demo_review(
    demo_benchmark: &DemoBenchmarkEquivalenceGateReportView,
) -> TassadarArticleCrossMachineReproducibilityDemoReview {
    let canonical_demo_case_ids =
        std::iter::once(demo_benchmark.hungarian_review.canonical_case_id.clone())
            .chain(
                demo_benchmark
                    .benchmark_review
                    .declared_case_ids
                    .iter()
                    .cloned(),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
    let demo_benchmark_equivalence_green = demo_benchmark
        .article_demo_benchmark_equivalence_gate_green
        && demo_benchmark.hungarian_review.hungarian_demo_parity_green
        && demo_benchmark.benchmark_review.named_arto_parity_green
        && demo_benchmark
            .benchmark_review
            .benchmark_wide_sudoku_parity_green
        && demo_benchmark.binding_review.binding_green;
    TassadarArticleCrossMachineReproducibilityDemoReview {
        report_ref: String::from(TASSADAR_ARTICLE_DEMO_BENCHMARK_EQUIVALENCE_GATE_REPORT_REF),
        hungarian_demo_case_id: demo_benchmark.hungarian_review.canonical_case_id.clone(),
        sudoku_demo_case_ids: demo_benchmark.benchmark_review.declared_case_ids.clone(),
        canonical_demo_case_ids,
        binding_green: demo_benchmark.binding_review.binding_green,
        demo_benchmark_equivalence_green,
        detail: format!(
            "Demo review keeps Hungarian case=`{}`, Sudoku case_count={}, binding_green={}, and demo_benchmark_equivalence_green={}.",
            demo_benchmark.hungarian_review.canonical_case_id,
            demo_benchmark.benchmark_review.declared_case_ids.len(),
            demo_benchmark.binding_review.binding_green,
            demo_benchmark_equivalence_green,
        ),
    }
}

fn build_long_horizon_review(
    single_run_no_spill: &SingleRunNoSpillClosureReportView,
) -> TassadarArticleCrossMachineReproducibilityLongHorizonReview {
    let all_horizon_rows_exact = single_run_no_spill
        .horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.reference_linear_direct && row.hull_cache_direct);
    let all_horizon_rows_floor_passed = single_run_no_spill
        .horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.throughput_floor_passed);
    let all_horizon_rows_fast_route_direct = single_run_no_spill
        .horizon_review
        .horizon_rows
        .iter()
        .all(|row| row.hull_cache_direct);
    TassadarArticleCrossMachineReproducibilityLongHorizonReview {
        report_ref: String::from(TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF),
        selected_candidate_kind: single_run_no_spill
            .horizon_review
            .selected_candidate_kind
            .clone(),
        selected_decode_mode: single_run_no_spill
            .horizon_review
            .selected_decode_mode
            .clone(),
        million_step_horizon_ids: single_run_no_spill
            .horizon_review
            .million_step_horizon_ids
            .clone(),
        multi_million_step_horizon_ids: single_run_no_spill
            .horizon_review
            .multi_million_step_horizon_ids
            .clone(),
        all_horizon_rows_exact,
        all_horizon_rows_fast_route_direct,
        all_horizon_rows_floor_passed,
        deterministic_exactness_green: single_run_no_spill
            .horizon_review
            .deterministic_exactness_green,
        single_run_no_spill_closure_green: single_run_no_spill.single_run_no_spill_closure_green,
        detail: format!(
            "Long-horizon review keeps selected_candidate_kind=`{}`, selected_decode_mode=`{}`, million_step_horizons={}, multi_million_step_horizons={}, all_horizon_rows_exact={}, all_horizon_rows_floor_passed={}, deterministic_exactness_green={}, and single_run_no_spill_closure_green={}.",
            single_run_no_spill.horizon_review.selected_candidate_kind,
            single_run_no_spill.horizon_review.selected_decode_mode,
            single_run_no_spill.horizon_review.million_step_horizon_ids.len(),
            single_run_no_spill
                .horizon_review
                .multi_million_step_horizon_ids
                .len(),
            all_horizon_rows_exact,
            all_horizon_rows_floor_passed,
            single_run_no_spill.horizon_review.deterministic_exactness_green,
            single_run_no_spill.single_run_no_spill_closure_green,
        ),
    }
}

fn build_throughput_drift_review(
    throughput_floor: &ThroughputFloorReportView,
) -> TassadarArticleCrossMachineReproducibilityThroughputDriftReview {
    TassadarArticleCrossMachineReproducibilityThroughputDriftReview {
        report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF),
        current_host_machine_class_id: throughput_floor
            .cross_machine_drift_review
            .current_host_machine_class_id
            .clone(),
        supported_machine_class_ids: throughput_floor
            .cross_machine_drift_review
            .supported_machine_class_ids
            .clone(),
        allowed_floor_drift_bps: throughput_floor
            .cross_machine_drift_review
            .allowed_floor_drift_bps,
        drift_policy_green: throughput_floor
            .cross_machine_drift_review
            .drift_policy_green,
        throughput_floor_green: throughput_floor.throughput_floor_green,
        kernel_horizon_ids: throughput_floor
            .throughput_bundle
            .kernel_receipts
            .iter()
            .map(|row| row.workload_horizon_id.clone())
            .collect(),
        detail: throughput_floor.cross_machine_drift_review.detail.clone(),
    }
}

fn build_stochastic_mode_review(
    single_run_no_spill: &SingleRunNoSpillClosureReportView,
) -> TassadarArticleCrossMachineReproducibilityStochasticModeReview {
    let out_of_scope = !single_run_no_spill
        .stochastic_mode_review
        .stochastic_mode_supported;
    TassadarArticleCrossMachineReproducibilityStochasticModeReview {
        report_ref: String::from(TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF),
        stochastic_mode_supported: single_run_no_spill
            .stochastic_mode_review
            .stochastic_mode_supported,
        stochastic_mode_robustness_green: single_run_no_spill
            .stochastic_mode_review
            .stochastic_mode_robustness_green,
        out_of_scope,
        detail: if out_of_scope {
            String::from(
                "stochastic execution stays out of scope for the canonical fast article route; TAS-185 freezes deterministic HullCache reproducibility only and refuses to widen the claim to stochastic execution",
            )
        } else {
            format!(
                "stochastic execution is in scope with robustness_green={}",
                single_run_no_spill
                    .stochastic_mode_review
                    .stochastic_mode_robustness_green
            )
        },
    }
}

fn build_machine_row(
    row: &CpuReproducibilityRowView,
    cpu_report: &CpuReproducibilityReportView,
    route_stability_review: &TassadarArticleCrossMachineReproducibilityRouteStabilityReview,
    demo_review: &TassadarArticleCrossMachineReproducibilityDemoReview,
    long_horizon_review: &TassadarArticleCrossMachineReproducibilityLongHorizonReview,
    throughput_drift_review: &TassadarArticleCrossMachineReproducibilityThroughputDriftReview,
    stochastic_mode_review: &TassadarArticleCrossMachineReproducibilityStochasticModeReview,
) -> TassadarArticleCrossMachineReproducibilityRow {
    let supported_machine_class = cpu_report
        .supported_machine_class_ids
        .iter()
        .any(|machine_class_id| machine_class_id == &row.machine_class_id);
    let is_current_host = row.machine_class_id == cpu_report.matrix.current_host_machine_class_id;

    let deterministic_demo_exactness_posture = if supported_machine_class {
        if is_current_host
            && route_stability_review.route_stability_green
            && demo_review.demo_benchmark_equivalence_green
        {
            TassadarArticleCrossMachineExactnessPosture::ExactMeasuredCurrentHost
        } else {
            TassadarArticleCrossMachineExactnessPosture::ExactRequiredDeclaredClass
        }
    } else {
        TassadarArticleCrossMachineExactnessPosture::Refused
    };
    let deterministic_long_horizon_exactness_posture = if supported_machine_class {
        if is_current_host
            && long_horizon_review.deterministic_exactness_green
            && long_horizon_review.single_run_no_spill_closure_green
        {
            TassadarArticleCrossMachineExactnessPosture::ExactMeasuredCurrentHost
        } else {
            TassadarArticleCrossMachineExactnessPosture::ExactRequiredDeclaredClass
        }
    } else {
        TassadarArticleCrossMachineExactnessPosture::Refused
    };
    let throughput_floor_stability_posture = if supported_machine_class {
        if is_current_host
            && cpu_report.matrix.current_host_measured_green
            && throughput_drift_review.drift_policy_green
            && throughput_drift_review.throughput_floor_green
        {
            TassadarArticleCrossMachineThroughputPosture::PassedMeasuredCurrentHost
        } else {
            TassadarArticleCrossMachineThroughputPosture::PassedRequiredDeclaredClass
        }
    } else {
        TassadarArticleCrossMachineThroughputPosture::Refused
    };
    let stochastic_mode_posture = if stochastic_mode_review.out_of_scope {
        TassadarArticleCrossMachineStochasticModePosture::OutOfScope
    } else if supported_machine_class {
        if is_current_host && stochastic_mode_review.stochastic_mode_robustness_green {
            TassadarArticleCrossMachineStochasticModePosture::RobustMeasuredCurrentHost
        } else {
            TassadarArticleCrossMachineStochasticModePosture::RobustRequiredDeclaredClass
        }
    } else {
        TassadarArticleCrossMachineStochasticModePosture::Refused
    };
    let row_green = if supported_machine_class {
        route_stability_review.route_stability_green
            && deterministic_demo_exactness_posture
                != TassadarArticleCrossMachineExactnessPosture::Refused
            && deterministic_long_horizon_exactness_posture
                != TassadarArticleCrossMachineExactnessPosture::Refused
            && throughput_floor_stability_posture
                != TassadarArticleCrossMachineThroughputPosture::Refused
            && stochastic_mode_posture != TassadarArticleCrossMachineStochasticModePosture::Refused
    } else {
        deterministic_demo_exactness_posture == TassadarArticleCrossMachineExactnessPosture::Refused
            && deterministic_long_horizon_exactness_posture
                == TassadarArticleCrossMachineExactnessPosture::Refused
            && throughput_floor_stability_posture
                == TassadarArticleCrossMachineThroughputPosture::Refused
            && (stochastic_mode_posture
                == TassadarArticleCrossMachineStochasticModePosture::OutOfScope
                || stochastic_mode_posture
                    == TassadarArticleCrossMachineStochasticModePosture::Refused)
    };

    TassadarArticleCrossMachineReproducibilityRow {
        machine_class_id: row.machine_class_id.clone(),
        machine_class_status: row.status,
        supported_machine_class,
        deterministic_demo_exactness_posture,
        deterministic_long_horizon_exactness_posture,
        throughput_floor_stability_posture,
        stochastic_mode_posture,
        route_stability_green: route_stability_review.route_stability_green,
        row_green,
        detail: format!(
            "machine_class_id=`{}` keeps status=`{:?}`, supported_machine_class={}, demo_exactness_posture=`{:?}`, long_horizon_exactness_posture=`{:?}`, throughput_posture=`{:?}`, stochastic_mode_posture=`{:?}`, route_stability_green={}, and row_green={}.",
            row.machine_class_id,
            row.status,
            supported_machine_class,
            deterministic_demo_exactness_posture,
            deterministic_long_horizon_exactness_posture,
            throughput_floor_stability_posture,
            stochastic_mode_posture,
            route_stability_review.route_stability_green,
            row_green,
        ),
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
) -> Result<T, TassadarArticleCrossMachineReproducibilityReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleCrossMachineReproducibilityReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleCrossMachineReproducibilityReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_cross_machine_reproducibility_matrix_report, read_repo_json,
        tassadar_article_cross_machine_reproducibility_matrix_report_path,
        write_tassadar_article_cross_machine_reproducibility_matrix_report,
        TassadarArticleCrossMachineExactnessPosture,
        TassadarArticleCrossMachineReproducibilityReport,
        TassadarArticleCrossMachineStochasticModePosture,
        TassadarArticleCrossMachineThroughputPosture,
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
    };

    #[test]
    fn article_cross_machine_reproducibility_matrix_tracks_green_supported_rows(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_article_cross_machine_reproducibility_matrix_report()?;

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-185");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.machine_matrix_review.current_host_measured_green);
        assert!(report.machine_matrix_review.machine_class_alignment_green);
        assert_eq!(
            report
                .machine_matrix_review
                .supported_machine_class_ids
                .len(),
            2
        );
        assert!(report.route_stability_review.route_stability_green);
        assert!(report.demo_review.demo_benchmark_equivalence_green);
        assert!(report.long_horizon_review.deterministic_exactness_green);
        assert!(report.long_horizon_review.single_run_no_spill_closure_green);
        assert!(report.throughput_drift_review.drift_policy_green);
        assert!(report.throughput_floor_stability_green);
        assert!(report.stochastic_mode_review.out_of_scope);
        assert!(report.deterministic_mode_green);
        assert!(report.reproducibility_matrix_green);
        assert!(report.article_equivalence_green);

        let current_host_row = report
            .machine_rows
            .iter()
            .find(|row| {
                row.machine_class_id == report.machine_matrix_review.current_host_machine_class_id
            })
            .expect("current host row");
        assert_eq!(
            current_host_row.deterministic_demo_exactness_posture,
            TassadarArticleCrossMachineExactnessPosture::ExactMeasuredCurrentHost
        );
        assert_eq!(
            current_host_row.deterministic_long_horizon_exactness_posture,
            TassadarArticleCrossMachineExactnessPosture::ExactMeasuredCurrentHost
        );
        assert_eq!(
            current_host_row.throughput_floor_stability_posture,
            TassadarArticleCrossMachineThroughputPosture::PassedMeasuredCurrentHost
        );
        assert_eq!(
            current_host_row.stochastic_mode_posture,
            TassadarArticleCrossMachineStochasticModePosture::OutOfScope
        );
        assert!(current_host_row.row_green);
        Ok(())
    }

    #[test]
    fn article_cross_machine_reproducibility_matrix_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_cross_machine_reproducibility_matrix_report()?;
        let committed: TassadarArticleCrossMachineReproducibilityReport = read_repo_json(
            TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
            "article_cross_machine_reproducibility_matrix_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_cross_machine_reproducibility_matrix_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_cross_machine_reproducibility_matrix_report.json");
        let written =
            write_tassadar_article_cross_machine_reproducibility_matrix_report(&output_path)?;
        let persisted: TassadarArticleCrossMachineReproducibilityReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_cross_machine_reproducibility_matrix_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_cross_machine_reproducibility_matrix_report.json")
        );
        Ok(())
    }
}
