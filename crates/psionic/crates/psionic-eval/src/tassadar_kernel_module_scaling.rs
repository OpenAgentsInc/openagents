use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    tassadar_kernel_module_scaling_contract, TassadarClrsLengthBucket,
    TassadarClrsTrajectoryFamily, TassadarKernelModuleScalingContract,
    TassadarKernelModuleScalingFamily, TassadarModuleScaleWorkloadFamily,
    TassadarModuleScaleWorkloadStatus, TassadarScalingAxis, TassadarScalingAxisPressureVector,
    TassadarScalingPhase, TASSADAR_KERNEL_MODULE_SCALING_REPORT_REF,
};
use psionic_runtime::TassadarModuleDifferentialStatus;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarClrsWasmBridgeReport, TassadarCompiledKernelFamilyId,
    TassadarCompiledKernelSuiteScalingFamilyReport, TassadarCompiledKernelSuiteScalingRegimeReport,
    TassadarCompiledKernelSuiteScalingReport, TassadarModuleScaleWorkloadCaseReport,
    TassadarModuleScaleWorkloadSuiteReport, TassadarWasmConformanceReport,
    TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF, TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
    TASSADAR_WASM_CONFORMANCE_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const COMPILED_KERNEL_SCALING_REPORT_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json";

/// Current posture of one observed scaling point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarKernelModuleScalingPosture {
    Exact,
    ExactButCostDegraded,
    Refused,
}

/// One observed point in the kernel-vs-module scaling report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingPoint {
    pub point_id: String,
    pub posture: TassadarKernelModuleScalingPosture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exactness_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_step_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_reference_cost_units: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled_over_cpu_ratio: Option<f64>,
    pub call_graph_width_value: u32,
    pub control_flow_depth_value: u32,
    pub memory_footprint_units: u32,
    pub import_complexity_value: u32,
    pub supporting_refs: Vec<String>,
    pub note: String,
}

/// Family-level report over one scaling family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingFamilyReport {
    pub workload_family: TassadarKernelModuleScalingFamily,
    pub phase: TassadarScalingPhase,
    pub axis_pressures: TassadarScalingAxisPressureVector,
    pub points: Vec<TassadarKernelModuleScalingPoint>,
    pub exact_point_count: u32,
    pub cost_degraded_point_count: u32,
    pub refusal_point_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_cost_degraded_point_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_refusal_point_id: Option<String>,
    pub route_threshold_note: String,
    pub claim_boundary: String,
}

/// Phase-level summary over the scaling report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingPhaseSummary {
    pub phase: TassadarScalingPhase,
    pub family_count: u32,
    pub exact_point_count: u32,
    pub cost_degraded_point_count: u32,
    pub refusal_point_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exact_call_graph_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exact_trace_step_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exact_memory_footprint_units: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exact_import_complexity: Option<u32>,
}

/// Route-threshold row derived from the observed scaling report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingRouteThreshold {
    pub threshold_id: String,
    pub phase: TassadarScalingPhase,
    pub axis: TassadarScalingAxis,
    pub max_exact_value: u64,
    pub next_posture: TassadarKernelModuleScalingPosture,
    pub supporting_refs: Vec<String>,
    pub note: String,
}

/// Eval-facing report over kernel-scale versus module-scale scaling laws.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub contract_ref: String,
    pub contract_digest: String,
    pub family_reports: Vec<TassadarKernelModuleScalingFamilyReport>,
    pub phase_summaries: Vec<TassadarKernelModuleScalingPhaseSummary>,
    pub route_thresholds: Vec<TassadarKernelModuleScalingRouteThreshold>,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarKernelModuleScalingReport {
    fn new(
        contract: &TassadarKernelModuleScalingContract,
        family_reports: Vec<TassadarKernelModuleScalingFamilyReport>,
        phase_summaries: Vec<TassadarKernelModuleScalingPhaseSummary>,
        route_thresholds: Vec<TassadarKernelModuleScalingRouteThreshold>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let exact_point_total = family_reports
            .iter()
            .map(|report| report.exact_point_count)
            .sum::<u32>();
        let cost_degraded_point_total = family_reports
            .iter()
            .map(|report| report.cost_degraded_point_count)
            .sum::<u32>();
        let refusal_point_total = family_reports
            .iter()
            .map(|report| report.refusal_point_count)
            .sum::<u32>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.kernel_module_scaling.report.v1"),
            contract_ref: contract.contract_ref.clone(),
            contract_digest: contract.contract_digest.clone(),
            family_reports,
            phase_summaries,
            route_thresholds,
            generated_from_refs,
            claim_boundary: String::from(
                "this report is a research-only scaling map over the current compiled kernel suite, CLRS bridge, module-scale workload suite, and Wasm conformance boundary. It keeps exact points, cost-degraded exact points, and refusal boundaries explicit instead of inferring module-scale closure from kernel-scale wins or smoothing all workload families into one generic curve",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Kernel-vs-module scaling now covers {} families with {} exact points, {} cost-degraded exact points, {} refusal points, and {} route thresholds kept explicit.",
            report.family_reports.len(),
            exact_point_total,
            cost_degraded_point_total,
            refusal_point_total,
            report.route_thresholds.len(),
        );
        report.report_digest =
            stable_digest(b"psionic_tassadar_kernel_module_scaling_report|", &report);
        report
    }
}

/// Scaling-report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarKernelModuleScalingReportError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing scaling authority for `{family}`")]
    MissingFamilyAuthority { family: String },
    #[error("missing module-scale case `{case_id}`")]
    MissingModuleCase { case_id: String },
    #[error("missing Wasm conformance case family `{family_id}`")]
    MissingConformanceCase { family_id: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed kernel-vs-module scaling report.
pub fn build_tassadar_kernel_module_scaling_report(
) -> Result<TassadarKernelModuleScalingReport, TassadarKernelModuleScalingReportError> {
    let contract = tassadar_kernel_module_scaling_contract();
    let kernel_scaling = read_repo_json::<TassadarCompiledKernelSuiteScalingReport>(
        COMPILED_KERNEL_SCALING_REPORT_REF,
    )?;
    let clrs_bridge =
        read_repo_json::<TassadarClrsWasmBridgeReport>(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)?;
    let module_scale = read_repo_json::<TassadarModuleScaleWorkloadSuiteReport>(
        TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
    )?;
    let wasm_conformance =
        read_repo_json::<TassadarWasmConformanceReport>(TASSADAR_WASM_CONFORMANCE_REPORT_REF)?;

    let family_reports = contract
        .workload_rows
        .iter()
        .map(|row| {
            build_family_report(
                row,
                &kernel_scaling,
                &clrs_bridge,
                &module_scale,
                &wasm_conformance,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let phase_summaries = build_phase_summaries(&family_reports);
    let route_thresholds = build_route_thresholds(&phase_summaries);

    Ok(TassadarKernelModuleScalingReport::new(
        &contract,
        family_reports,
        phase_summaries,
        route_thresholds,
        generated_from_refs(),
    ))
}

/// Returns the canonical absolute path for the committed scaling report.
#[must_use]
pub fn tassadar_kernel_module_scaling_report_path() -> PathBuf {
    repo_root().join(TASSADAR_KERNEL_MODULE_SCALING_REPORT_REF)
}

/// Writes the committed kernel-vs-module scaling report.
pub fn write_tassadar_kernel_module_scaling_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarKernelModuleScalingReport, TassadarKernelModuleScalingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarKernelModuleScalingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_kernel_module_scaling_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarKernelModuleScalingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_family_report(
    row: &psionic_data::TassadarKernelModuleScalingWorkloadRow,
    kernel_scaling: &TassadarCompiledKernelSuiteScalingReport,
    clrs_bridge: &TassadarClrsWasmBridgeReport,
    module_scale: &TassadarModuleScaleWorkloadSuiteReport,
    wasm_conformance: &TassadarWasmConformanceReport,
) -> Result<TassadarKernelModuleScalingFamilyReport, TassadarKernelModuleScalingReportError> {
    let points = match row.workload_family {
        TassadarKernelModuleScalingFamily::ArithmeticKernel
        | TassadarKernelModuleScalingFamily::MemoryUpdateKernel
        | TassadarKernelModuleScalingFamily::ForwardBranchKernel
        | TassadarKernelModuleScalingFamily::BackwardLoopKernel => {
            let family_report = kernel_scaling_family(kernel_scaling, row.workload_family)?;
            family_report
                .regimes
                .iter()
                .map(build_kernel_point)
                .collect::<Vec<_>>()
        }
        TassadarKernelModuleScalingFamily::ClrsSequentialShortestPath
        | TassadarKernelModuleScalingFamily::ClrsWavefrontShortestPath => clrs_bridge
            .length_generalization_matrix
            .iter()
            .filter(|cell| clrs_family_matches(row.workload_family, cell.trajectory_family))
            .map(|cell| build_clrs_point(row.workload_family, cell))
            .collect::<Vec<_>>(),
        TassadarKernelModuleScalingFamily::ModuleMemcpy
        | TassadarKernelModuleScalingFamily::ModuleChecksum
        | TassadarKernelModuleScalingFamily::ModuleParsing
        | TassadarKernelModuleScalingFamily::ModuleVmDispatch
        | TassadarKernelModuleScalingFamily::ModuleVmParamBoundary => vec![build_module_point(
            module_scale_case(module_scale, row.workload_family)?,
            row.workload_family,
        )],
        TassadarKernelModuleScalingFamily::ModuleHostImportBoundary => {
            vec![build_host_import_boundary_point(wasm_conformance_case(
                wasm_conformance,
                "curated.unsupported_host_import",
            )?)]
        }
    };
    let exact_point_count = points
        .iter()
        .filter(|point| point.posture == TassadarKernelModuleScalingPosture::Exact)
        .count() as u32;
    let cost_degraded_point_count = points
        .iter()
        .filter(|point| point.posture == TassadarKernelModuleScalingPosture::ExactButCostDegraded)
        .count() as u32;
    let refusal_point_count = points
        .iter()
        .filter(|point| point.posture == TassadarKernelModuleScalingPosture::Refused)
        .count() as u32;
    let first_cost_degraded_point_id = points
        .iter()
        .find(|point| point.posture == TassadarKernelModuleScalingPosture::ExactButCostDegraded)
        .map(|point| point.point_id.clone());
    let first_refusal_point_id = points
        .iter()
        .find(|point| point.posture == TassadarKernelModuleScalingPosture::Refused)
        .map(|point| point.point_id.clone());

    Ok(TassadarKernelModuleScalingFamilyReport {
        workload_family: row.workload_family,
        phase: row.phase,
        axis_pressures: row.axis_pressures.clone(),
        points,
        exact_point_count,
        cost_degraded_point_count,
        refusal_point_count,
        first_cost_degraded_point_id,
        first_refusal_point_id,
        route_threshold_note: route_threshold_note(row.workload_family),
        claim_boundary: row.claim_boundary.clone(),
    })
}

fn kernel_scaling_family(
    report: &TassadarCompiledKernelSuiteScalingReport,
    family: TassadarKernelModuleScalingFamily,
) -> Result<&TassadarCompiledKernelSuiteScalingFamilyReport, TassadarKernelModuleScalingReportError>
{
    let family_id = match family {
        TassadarKernelModuleScalingFamily::ArithmeticKernel => {
            TassadarCompiledKernelFamilyId::ArithmeticKernel
        }
        TassadarKernelModuleScalingFamily::MemoryUpdateKernel => {
            TassadarCompiledKernelFamilyId::MemoryUpdateKernel
        }
        TassadarKernelModuleScalingFamily::ForwardBranchKernel => {
            TassadarCompiledKernelFamilyId::ForwardBranchKernel
        }
        TassadarKernelModuleScalingFamily::BackwardLoopKernel => {
            TassadarCompiledKernelFamilyId::BackwardLoopKernel
        }
        _ => unreachable!("non-kernel family requested kernel scaling authority"),
    };
    report
        .family_reports
        .iter()
        .find(|family_report| family_report.family_id == family_id)
        .ok_or_else(
            || TassadarKernelModuleScalingReportError::MissingFamilyAuthority {
                family: family.as_str().to_string(),
            },
        )
}

fn build_kernel_point(
    regime: &TassadarCompiledKernelSuiteScalingRegimeReport,
) -> TassadarKernelModuleScalingPoint {
    let posture = if regime.exactness_bps == 10_000
        && (regime.compiled_over_cpu_ratio < 0.02
            || regime.trace_step_count > 128
            || regime.memory_slot_count > 16)
    {
        TassadarKernelModuleScalingPosture::ExactButCostDegraded
    } else {
        TassadarKernelModuleScalingPosture::Exact
    };
    TassadarKernelModuleScalingPoint {
        point_id: regime.case_id.clone(),
        posture,
        exactness_bps: Some(regime.exactness_bps),
        trace_step_count: Some(regime.trace_step_count),
        cpu_reference_cost_units: None,
        compiled_over_cpu_ratio: Some(regime.compiled_over_cpu_ratio),
        call_graph_width_value: 1,
        control_flow_depth_value: kernel_control_flow_depth(regime.family_id),
        memory_footprint_units: regime.memory_slot_count,
        import_complexity_value: 0,
        supporting_refs: vec![String::from(COMPILED_KERNEL_SCALING_REPORT_REF)],
        note: format!(
            "{} currently keeps exactness={}, trace_steps={}, memory_units={}, and compiled_over_cpu_ratio={:.6}",
            regime.summary,
            regime.exactness_bps,
            regime.trace_step_count,
            regime.memory_slot_count,
            regime.compiled_over_cpu_ratio,
        ),
    }
}

fn kernel_control_flow_depth(family_id: TassadarCompiledKernelFamilyId) -> u32 {
    match family_id {
        TassadarCompiledKernelFamilyId::ArithmeticKernel => 1,
        TassadarCompiledKernelFamilyId::MemoryUpdateKernel => 1,
        TassadarCompiledKernelFamilyId::ForwardBranchKernel => 2,
        TassadarCompiledKernelFamilyId::BackwardLoopKernel => 3,
    }
}

fn clrs_family_matches(
    family: TassadarKernelModuleScalingFamily,
    trajectory_family: TassadarClrsTrajectoryFamily,
) -> bool {
    matches!(
        (family, trajectory_family),
        (
            TassadarKernelModuleScalingFamily::ClrsSequentialShortestPath,
            TassadarClrsTrajectoryFamily::SequentialRelaxation
        ) | (
            TassadarKernelModuleScalingFamily::ClrsWavefrontShortestPath,
            TassadarClrsTrajectoryFamily::WavefrontRelaxation
        )
    )
}

fn build_clrs_point(
    family: TassadarKernelModuleScalingFamily,
    cell: &crate::TassadarClrsWasmLengthGeneralizationCell,
) -> TassadarKernelModuleScalingPoint {
    let posture = if cell.module_trace_steps > 13 || cell.cpu_reference_cost_units > 180 {
        TassadarKernelModuleScalingPosture::ExactButCostDegraded
    } else {
        TassadarKernelModuleScalingPosture::Exact
    };
    TassadarKernelModuleScalingPoint {
        point_id: format!(
            "{}.{}",
            family.as_str(),
            clrs_bucket_label(cell.length_bucket)
        ),
        posture,
        exactness_bps: Some(cell.exactness_bps),
        trace_step_count: Some(cell.module_trace_steps),
        cpu_reference_cost_units: Some(cell.cpu_reference_cost_units),
        compiled_over_cpu_ratio: None,
        call_graph_width_value: 1,
        control_flow_depth_value: match family {
            TassadarKernelModuleScalingFamily::ClrsSequentialShortestPath => 3,
            TassadarKernelModuleScalingFamily::ClrsWavefrontShortestPath => 2,
            _ => 1,
        },
        memory_footprint_units: clrs_memory_units(cell.length_bucket),
        import_complexity_value: 0,
        supporting_refs: vec![String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)],
        note: format!(
            "CLRS {:?} {:?} keeps exactness={} with trace_steps={} and cpu_cost={}",
            cell.trajectory_family,
            cell.length_bucket,
            cell.exactness_bps,
            cell.module_trace_steps,
            cell.cpu_reference_cost_units,
        ),
    }
}

fn clrs_bucket_label(length_bucket: TassadarClrsLengthBucket) -> &'static str {
    match length_bucket {
        TassadarClrsLengthBucket::Tiny => "tiny",
        TassadarClrsLengthBucket::Small => "small",
    }
}

fn clrs_memory_units(length_bucket: TassadarClrsLengthBucket) -> u32 {
    match length_bucket {
        TassadarClrsLengthBucket::Tiny => 4,
        TassadarClrsLengthBucket::Small => 8,
    }
}

fn module_scale_case(
    report: &TassadarModuleScaleWorkloadSuiteReport,
    family: TassadarKernelModuleScalingFamily,
) -> Result<&TassadarModuleScaleWorkloadCaseReport, TassadarKernelModuleScalingReportError> {
    let case_id = match family {
        TassadarKernelModuleScalingFamily::ModuleMemcpy => "memcpy_fixed_span_exact",
        TassadarKernelModuleScalingFamily::ModuleChecksum => "checksum_fixed_span_exact",
        TassadarKernelModuleScalingFamily::ModuleParsing => "parsing_token_triplet_exact",
        TassadarKernelModuleScalingFamily::ModuleVmDispatch => "vm_style_dispatch_exact",
        TassadarKernelModuleScalingFamily::ModuleVmParamBoundary => "vm_style_param_refusal",
        _ => unreachable!("non-module family requested module-scale case"),
    };
    report
        .cases
        .iter()
        .find(|case| case.case_id == case_id)
        .ok_or_else(
            || TassadarKernelModuleScalingReportError::MissingModuleCase {
                case_id: case_id.to_string(),
            },
        )
}

fn build_module_point(
    case: &TassadarModuleScaleWorkloadCaseReport,
    family: TassadarKernelModuleScalingFamily,
) -> TassadarKernelModuleScalingPoint {
    let posture = match case.status {
        TassadarModuleScaleWorkloadStatus::LoweredExact => {
            if case.total_trace_steps.unwrap_or(0) > 16
                || case.cpu_reference_cost_units.unwrap_or(0) > 180
            {
                TassadarKernelModuleScalingPosture::ExactButCostDegraded
            } else {
                TassadarKernelModuleScalingPosture::Exact
            }
        }
        TassadarModuleScaleWorkloadStatus::LoweringRefused
        | TassadarModuleScaleWorkloadStatus::CompileRefused => {
            TassadarKernelModuleScalingPosture::Refused
        }
    };
    TassadarKernelModuleScalingPoint {
        point_id: case.case_id.clone(),
        posture,
        exactness_bps: case.exactness_bps,
        trace_step_count: case.total_trace_steps,
        cpu_reference_cost_units: case.cpu_reference_cost_units,
        compiled_over_cpu_ratio: None,
        call_graph_width_value: case
            .wasm_binary_summary
            .as_ref()
            .map(|summary| summary.function_count)
            .unwrap_or(1),
        control_flow_depth_value: module_control_flow_depth(case.family, family),
        memory_footprint_units: case
            .wasm_binary_summary
            .as_ref()
            .map(|summary| summary.memory_count)
            .unwrap_or(0),
        import_complexity_value: case
            .wasm_binary_summary
            .as_ref()
            .map(|summary| summary.imported_function_count)
            .unwrap_or(0),
        supporting_refs: vec![String::from(
            TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
        )],
        note: if posture == TassadarKernelModuleScalingPosture::Refused {
            format!(
                "{} currently refuses with {:?}",
                case.summary, case.refusal_kind
            )
        } else {
            format!(
                "{} currently keeps exactness={} with trace_steps={} and cpu_cost={}",
                case.summary,
                case.exactness_bps.unwrap_or(0),
                case.total_trace_steps.unwrap_or(0),
                case.cpu_reference_cost_units.unwrap_or(0),
            )
        },
    }
}

fn module_control_flow_depth(
    family: TassadarModuleScaleWorkloadFamily,
    scaling_family: TassadarKernelModuleScalingFamily,
) -> u32 {
    match (family, scaling_family) {
        (TassadarModuleScaleWorkloadFamily::Memcpy, _) => 1,
        (TassadarModuleScaleWorkloadFamily::Checksum, _) => 1,
        (TassadarModuleScaleWorkloadFamily::Parsing, _) => 2,
        (
            TassadarModuleScaleWorkloadFamily::VmStyle,
            TassadarKernelModuleScalingFamily::ModuleVmParamBoundary,
        ) => 2,
        (TassadarModuleScaleWorkloadFamily::VmStyle, _) => 2,
    }
}

fn wasm_conformance_case<'a>(
    report: &'a TassadarWasmConformanceReport,
    family_id: &str,
) -> Result<
    &'a psionic_runtime::TassadarModuleExecutionDifferentialResult,
    TassadarKernelModuleScalingReportError,
> {
    report
        .cases
        .iter()
        .find(|case| case.family_id == family_id)
        .ok_or_else(
            || TassadarKernelModuleScalingReportError::MissingConformanceCase {
                family_id: family_id.to_string(),
            },
        )
}

fn build_host_import_boundary_point(
    case: &psionic_runtime::TassadarModuleExecutionDifferentialResult,
) -> TassadarKernelModuleScalingPoint {
    let posture = match case.status {
        TassadarModuleDifferentialStatus::BoundaryRefusal => {
            TassadarKernelModuleScalingPosture::Refused
        }
        _ => TassadarKernelModuleScalingPosture::Exact,
    };
    TassadarKernelModuleScalingPoint {
        point_id: case.case_id.clone(),
        posture,
        exactness_bps: None,
        trace_step_count: None,
        cpu_reference_cost_units: None,
        compiled_over_cpu_ratio: None,
        call_graph_width_value: 1,
        control_flow_depth_value: 1,
        memory_footprint_units: 0,
        import_complexity_value: 1,
        supporting_refs: vec![String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF)],
        note: format!(
            "{} keeps runtime_error_kind={:?} explicit",
            case.family_id, case.runtime_error_kind
        ),
    }
}

fn route_threshold_note(family: TassadarKernelModuleScalingFamily) -> String {
    match family {
        TassadarKernelModuleScalingFamily::ArithmeticKernel
        | TassadarKernelModuleScalingFamily::MemoryUpdateKernel
        | TassadarKernelModuleScalingFamily::ForwardBranchKernel
        | TassadarKernelModuleScalingFamily::BackwardLoopKernel => String::from(
            "kernel rows keep exactness separate from cost degradation; once trace length or compiled-over-cpu ratio crosses the current exact ceiling, the report marks the family as exact-but-cost-degraded rather than silently widening the route claim",
        ),
        TassadarKernelModuleScalingFamily::ClrsSequentialShortestPath
        | TassadarKernelModuleScalingFamily::ClrsWavefrontShortestPath => String::from(
            "bridge rows keep length-bucket and trajectory-family breakpoints explicit; small sequential shortest-path cases already sit beyond the current exact trace ceiling even while outputs remain exact",
        ),
        TassadarKernelModuleScalingFamily::ModuleMemcpy
        | TassadarKernelModuleScalingFamily::ModuleChecksum
        | TassadarKernelModuleScalingFamily::ModuleParsing
        | TassadarKernelModuleScalingFamily::ModuleVmDispatch => String::from(
            "module rows keep small exact cases and cost-degraded exact cases explicit; they do not widen to arbitrary module families or implicit planner routing",
        ),
        TassadarKernelModuleScalingFamily::ModuleVmParamBoundary => String::from(
            "parameter-ABI rows remain an explicit module refusal threshold instead of being hidden inside broader module success claims",
        ),
        TassadarKernelModuleScalingFamily::ModuleHostImportBoundary => String::from(
            "host-import rows remain a direct import-complexity refusal threshold with current exact ceiling fixed at zero imported functions",
        ),
    }
}

fn build_phase_summaries(
    family_reports: &[TassadarKernelModuleScalingFamilyReport],
) -> Vec<TassadarKernelModuleScalingPhaseSummary> {
    [
        TassadarScalingPhase::KernelScale,
        TassadarScalingPhase::BridgeScale,
        TassadarScalingPhase::ModuleScale,
    ]
    .into_iter()
    .map(|phase| {
        let phase_reports = family_reports
            .iter()
            .filter(|report| report.phase == phase)
            .collect::<Vec<_>>();
        let exact_points = phase_reports
            .iter()
            .flat_map(|report| report.points.iter())
            .filter(|point| point.posture == TassadarKernelModuleScalingPosture::Exact)
            .collect::<Vec<_>>();
        TassadarKernelModuleScalingPhaseSummary {
            phase,
            family_count: phase_reports.len() as u32,
            exact_point_count: exact_points.len() as u32,
            cost_degraded_point_count: phase_reports
                .iter()
                .map(|report| report.cost_degraded_point_count)
                .sum(),
            refusal_point_count: phase_reports
                .iter()
                .map(|report| report.refusal_point_count)
                .sum(),
            max_exact_call_graph_width: exact_points
                .iter()
                .map(|point| point.call_graph_width_value)
                .max(),
            max_exact_trace_step_count: exact_points
                .iter()
                .filter_map(|point| point.trace_step_count)
                .max(),
            max_exact_memory_footprint_units: exact_points
                .iter()
                .map(|point| point.memory_footprint_units)
                .max(),
            max_exact_import_complexity: exact_points
                .iter()
                .map(|point| point.import_complexity_value)
                .max(),
        }
    })
    .collect()
}

fn build_route_thresholds(
    phase_summaries: &[TassadarKernelModuleScalingPhaseSummary],
) -> Vec<TassadarKernelModuleScalingRouteThreshold> {
    let mut thresholds = Vec::new();
    for summary in phase_summaries {
        if let Some(max_exact_trace_step_count) = summary.max_exact_trace_step_count {
            thresholds.push(TassadarKernelModuleScalingRouteThreshold {
                threshold_id: format!("{}.trace_length_exact_ceiling", summary.phase.as_str()),
                phase: summary.phase,
                axis: TassadarScalingAxis::TraceLength,
                max_exact_value: max_exact_trace_step_count,
                next_posture: TassadarKernelModuleScalingPosture::ExactButCostDegraded,
                supporting_refs: match summary.phase {
                    TassadarScalingPhase::KernelScale => {
                        vec![String::from(COMPILED_KERNEL_SCALING_REPORT_REF)]
                    }
                    TassadarScalingPhase::BridgeScale => {
                        vec![String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)]
                    }
                    TassadarScalingPhase::ModuleScale => {
                        vec![String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF)]
                    }
                },
                note: format!(
                    "{} keeps exact trace-length support explicit up to {} measured steps before the current report switches to exact-but-cost-degraded posture",
                    summary.phase.as_str(),
                    max_exact_trace_step_count,
                ),
            });
        }
    }
    if let Some(summary) = phase_summaries
        .iter()
        .find(|summary| summary.phase == TassadarScalingPhase::KernelScale)
    {
        if let Some(max_exact_memory_footprint_units) = summary.max_exact_memory_footprint_units {
            thresholds.push(TassadarKernelModuleScalingRouteThreshold {
                threshold_id: String::from("kernel_scale.memory_footprint_exact_ceiling"),
                phase: TassadarScalingPhase::KernelScale,
                axis: TassadarScalingAxis::MemoryFootprint,
                max_exact_value: u64::from(max_exact_memory_footprint_units),
                next_posture: TassadarKernelModuleScalingPosture::ExactButCostDegraded,
                supporting_refs: vec![String::from(COMPILED_KERNEL_SCALING_REPORT_REF)],
                note: format!(
                    "kernel-scale exact memory pressure is currently grounded up to {} units before the report marks later regimes exact-but-cost-degraded",
                    max_exact_memory_footprint_units,
                ),
            });
        }
    }
    if let Some(summary) = phase_summaries
        .iter()
        .find(|summary| summary.phase == TassadarScalingPhase::ModuleScale)
    {
        if let Some(max_exact_import_complexity) = summary.max_exact_import_complexity {
            thresholds.push(TassadarKernelModuleScalingRouteThreshold {
                threshold_id: String::from("module_scale.import_complexity_exact_ceiling"),
                phase: TassadarScalingPhase::ModuleScale,
                axis: TassadarScalingAxis::ImportComplexity,
                max_exact_value: u64::from(max_exact_import_complexity),
                next_posture: TassadarKernelModuleScalingPosture::Refused,
                supporting_refs: vec![
                    String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF),
                    String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF),
                ],
                note: format!(
                    "module-scale exact import complexity is currently grounded only up to {} imported functions; the next measured host-import point is refusal-first",
                    max_exact_import_complexity,
                ),
            });
        }
    }
    thresholds
}

fn generated_from_refs() -> Vec<String> {
    vec![
        String::from(COMPILED_KERNEL_SCALING_REPORT_REF),
        String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF),
        String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF),
        String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF),
    ]
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarKernelModuleScalingReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarKernelModuleScalingReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarKernelModuleScalingReportError::Deserialize {
            path: path.display().to_string(),
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
        build_tassadar_kernel_module_scaling_report, read_repo_json,
        tassadar_kernel_module_scaling_report_path, write_tassadar_kernel_module_scaling_report,
        TassadarKernelModuleScalingPosture, TassadarKernelModuleScalingReport,
    };
    use psionic_data::{
        TassadarKernelModuleScalingFamily, TassadarScalingPhase,
        TASSADAR_KERNEL_MODULE_SCALING_REPORT_REF,
    };

    #[test]
    fn kernel_module_scaling_report_surfaces_kernel_degradation_and_module_refusal_boundaries() {
        let report = build_tassadar_kernel_module_scaling_report().expect("scaling report");

        let kernel_phase = report
            .phase_summaries
            .iter()
            .find(|summary| summary.phase == TassadarScalingPhase::KernelScale)
            .expect("kernel phase summary");
        assert_eq!(kernel_phase.max_exact_trace_step_count, Some(61));

        let bridge_phase = report
            .phase_summaries
            .iter()
            .find(|summary| summary.phase == TassadarScalingPhase::BridgeScale)
            .expect("bridge phase summary");
        assert_eq!(bridge_phase.max_exact_trace_step_count, Some(13));

        let module_phase = report
            .phase_summaries
            .iter()
            .find(|summary| summary.phase == TassadarScalingPhase::ModuleScale)
            .expect("module phase summary");
        assert_eq!(module_phase.max_exact_import_complexity, Some(0));

        let backward_loop = report
            .family_reports
            .iter()
            .find(|family| {
                family.workload_family == TassadarKernelModuleScalingFamily::BackwardLoopKernel
            })
            .expect("backward-loop family");
        assert_eq!(
            backward_loop.points[0].posture,
            TassadarKernelModuleScalingPosture::ExactButCostDegraded
        );

        let host_import = report
            .family_reports
            .iter()
            .find(|family| {
                family.workload_family
                    == TassadarKernelModuleScalingFamily::ModuleHostImportBoundary
            })
            .expect("host-import boundary");
        assert_eq!(host_import.refusal_point_count, 1);
    }

    #[test]
    fn kernel_module_scaling_report_matches_committed_truth() {
        let generated = build_tassadar_kernel_module_scaling_report().expect("scaling report");
        let committed: TassadarKernelModuleScalingReport =
            read_repo_json(TASSADAR_KERNEL_MODULE_SCALING_REPORT_REF)
                .expect("committed scaling report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_kernel_module_scaling_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_kernel_module_scaling_report.json");
        let written = write_tassadar_kernel_module_scaling_report(&output_path)
            .expect("write scaling report");
        let persisted: TassadarKernelModuleScalingReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");

        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_kernel_module_scaling_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_kernel_module_scaling_report.json")
        );
    }
}
