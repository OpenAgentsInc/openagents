use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_KERNEL_MODULE_SCALING_ABI_VERSION: &str =
    "psionic.tassadar.kernel_module_scaling.v1";
pub const TASSADAR_KERNEL_MODULE_SCALING_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/kernel_module_scaling";
pub const TASSADAR_KERNEL_MODULE_SCALING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_kernel_module_scaling_report.json";
pub const TASSADAR_KERNEL_MODULE_SCALING_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_kernel_module_scaling_summary.json";

/// Scaling phase compared by the public kernel-vs-module analysis lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarScalingPhase {
    KernelScale,
    BridgeScale,
    ModuleScale,
}

impl TassadarScalingPhase {
    /// Returns the stable phase label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KernelScale => "kernel_scale",
            Self::BridgeScale => "bridge_scale",
            Self::ModuleScale => "module_scale",
        }
    }
}

/// Scaling axis surfaced by the public kernel-vs-module analysis lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarScalingAxis {
    CallGraphWidth,
    ControlFlowDepth,
    TraceLength,
    MemoryFootprint,
    ImportComplexity,
}

impl TassadarScalingAxis {
    /// Returns the stable axis label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CallGraphWidth => "call_graph_width",
            Self::ControlFlowDepth => "control_flow_depth",
            Self::TraceLength => "trace_length",
            Self::MemoryFootprint => "memory_footprint",
            Self::ImportComplexity => "import_complexity",
        }
    }
}

/// Pressure label used by the public scaling contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarScalingPressure {
    Low,
    Medium,
    High,
    Boundary,
}

/// Stable family identifier carried by the kernel-vs-module scaling lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarKernelModuleScalingFamily {
    ArithmeticKernel,
    MemoryUpdateKernel,
    ForwardBranchKernel,
    BackwardLoopKernel,
    ClrsSequentialShortestPath,
    ClrsWavefrontShortestPath,
    ModuleMemcpy,
    ModuleChecksum,
    ModuleParsing,
    ModuleVmDispatch,
    ModuleVmParamBoundary,
    ModuleHostImportBoundary,
}

impl TassadarKernelModuleScalingFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArithmeticKernel => "arithmetic_kernel",
            Self::MemoryUpdateKernel => "memory_update_kernel",
            Self::ForwardBranchKernel => "forward_branch_kernel",
            Self::BackwardLoopKernel => "backward_loop_kernel",
            Self::ClrsSequentialShortestPath => "clrs_sequential_shortest_path",
            Self::ClrsWavefrontShortestPath => "clrs_wavefront_shortest_path",
            Self::ModuleMemcpy => "module_memcpy",
            Self::ModuleChecksum => "module_checksum",
            Self::ModuleParsing => "module_parsing",
            Self::ModuleVmDispatch => "module_vm_dispatch",
            Self::ModuleVmParamBoundary => "module_vm_param_boundary",
            Self::ModuleHostImportBoundary => "module_host_import_boundary",
        }
    }
}

/// Axis-pressure vector attached to one public scaling row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScalingAxisPressureVector {
    pub call_graph_width: TassadarScalingPressure,
    pub control_flow_depth: TassadarScalingPressure,
    pub trace_length: TassadarScalingPressure,
    pub memory_footprint: TassadarScalingPressure,
    pub import_complexity: TassadarScalingPressure,
}

/// One workload row in the public kernel-vs-module scaling contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingWorkloadRow {
    pub workload_family: TassadarKernelModuleScalingFamily,
    pub phase: TassadarScalingPhase,
    pub axis_pressures: TassadarScalingAxisPressureVector,
    pub authority_refs: Vec<String>,
    pub claim_boundary: String,
}

/// Public contract for the kernel-vs-module scaling analysis lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingContract {
    pub abi_version: String,
    pub contract_ref: String,
    pub version: String,
    pub phases: Vec<TassadarScalingPhase>,
    pub scaling_axes: Vec<TassadarScalingAxis>,
    pub workload_rows: Vec<TassadarKernelModuleScalingWorkloadRow>,
    pub evaluation_axes: Vec<String>,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarKernelModuleScalingContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_KERNEL_MODULE_SCALING_ABI_VERSION),
            contract_ref: String::from(TASSADAR_KERNEL_MODULE_SCALING_CONTRACT_REF),
            version: String::from("2026.03.18"),
            phases: vec![
                TassadarScalingPhase::KernelScale,
                TassadarScalingPhase::BridgeScale,
                TassadarScalingPhase::ModuleScale,
            ],
            scaling_axes: vec![
                TassadarScalingAxis::CallGraphWidth,
                TassadarScalingAxis::ControlFlowDepth,
                TassadarScalingAxis::TraceLength,
                TassadarScalingAxis::MemoryFootprint,
                TassadarScalingAxis::ImportComplexity,
            ],
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("exactness_bps"),
                String::from("trace_step_count"),
                String::from("cpu_reference_cost_units"),
                String::from("compiled_over_cpu_ratio"),
                String::from("refusal_boundary"),
            ],
            report_ref: String::from(TASSADAR_KERNEL_MODULE_SCALING_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_KERNEL_MODULE_SCALING_SUMMARY_REPORT_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("kernel-module scaling contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_kernel_module_scaling_contract|",
            &contract,
        );
        contract
    }

    /// Validates the public scaling contract.
    pub fn validate(&self) -> Result<(), TassadarKernelModuleScalingContractError> {
        if self.abi_version != TASSADAR_KERNEL_MODULE_SCALING_ABI_VERSION {
            return Err(
                TassadarKernelModuleScalingContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarKernelModuleScalingContractError::MissingContractRef);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarKernelModuleScalingContractError::MissingWorkloads);
        }
        if self.report_ref.trim().is_empty() || self.summary_report_ref.trim().is_empty() {
            return Err(TassadarKernelModuleScalingContractError::MissingReportRefs);
        }
        Ok(())
    }
}

fn workload_rows() -> Vec<TassadarKernelModuleScalingWorkloadRow> {
    vec![
        row(
            TassadarKernelModuleScalingFamily::ArithmeticKernel,
            TassadarScalingPhase::KernelScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json"],
            "arithmetic kernels stay bounded to the current article_i32 compiled kernel suite and do not imply module-scale closure by extrapolation",
        ),
        row(
            TassadarKernelModuleScalingFamily::MemoryUpdateKernel,
            TassadarScalingPhase::KernelScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::High,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json"],
            "memory-update kernels keep bounded state pressure explicit, but they remain kernel-class evidence rather than module-scale proof",
        ),
        row(
            TassadarKernelModuleScalingFamily::ForwardBranchKernel,
            TassadarScalingPhase::KernelScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json"],
            "forward-branch kernels map bounded control-flow pressure only; they do not by themselves settle module call-graph scaling",
        ),
        row(
            TassadarKernelModuleScalingFamily::BackwardLoopKernel,
            TassadarScalingPhase::KernelScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::High,
                TassadarScalingPressure::High,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json"],
            "backward-loop kernels keep long-horizon cost pressure explicit instead of treating exact compiled replay as free scaling closure",
        ),
        row(
            TassadarKernelModuleScalingFamily::ClrsSequentialShortestPath,
            TassadarScalingPhase::BridgeScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::High,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "sequential CLRS bridge rows remain a bounded shortest-path bridge and do not imply broad CLRS or module-scale Wasm closure",
        ),
        row(
            TassadarKernelModuleScalingFamily::ClrsWavefrontShortestPath,
            TassadarScalingPhase::BridgeScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "wavefront CLRS bridge rows remain a bounded shortest-path bridge and keep trajectory-family differences explicit instead of smoothing them into one scaling curve",
        ),
        row(
            TassadarKernelModuleScalingFamily::ModuleMemcpy,
            TassadarScalingPhase::ModuleScale,
            pressures(
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "module memcpy rows stay bounded to the public deterministic module suite and do not imply arbitrary module execution closure",
        ),
        row(
            TassadarKernelModuleScalingFamily::ModuleChecksum,
            TassadarScalingPhase::ModuleScale,
            pressures(
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "module checksum rows remain bounded to the current deterministic suite and keep module-scale cost pressure explicit",
        ),
        row(
            TassadarKernelModuleScalingFamily::ModuleParsing,
            TassadarScalingPhase::ModuleScale,
            pressures(
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "module parsing rows remain bounded to the seeded fixed-token module family and do not imply broad parser closure",
        ),
        row(
            TassadarKernelModuleScalingFamily::ModuleVmDispatch,
            TassadarScalingPhase::ModuleScale,
            pressures(
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "VM-style dispatch rows keep bounded multi-export module scaling explicit without widening to arbitrary module graphs",
        ),
        row(
            TassadarKernelModuleScalingFamily::ModuleVmParamBoundary,
            TassadarScalingPhase::ModuleScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Medium,
                TassadarScalingPressure::Boundary,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
            ),
            vec!["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "parameter-ABI VM rows remain an explicit module-scale refusal boundary rather than a hidden unsupported corner",
        ),
        row(
            TassadarKernelModuleScalingFamily::ModuleHostImportBoundary,
            TassadarScalingPhase::ModuleScale,
            pressures(
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Low,
                TassadarScalingPressure::Boundary,
            ),
            vec!["fixtures/tassadar/reports/tassadar_wasm_conformance_report.json"],
            "host-import rows remain an explicit import-complexity refusal boundary and do not imply any host-import support in the current lane",
        ),
    ]
}

fn row(
    workload_family: TassadarKernelModuleScalingFamily,
    phase: TassadarScalingPhase,
    axis_pressures: TassadarScalingAxisPressureVector,
    authority_refs: Vec<&str>,
    claim_boundary: &str,
) -> TassadarKernelModuleScalingWorkloadRow {
    TassadarKernelModuleScalingWorkloadRow {
        workload_family,
        phase,
        axis_pressures,
        authority_refs: authority_refs
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>(),
        claim_boundary: String::from(claim_boundary),
    }
}

fn pressures(
    call_graph_width: TassadarScalingPressure,
    control_flow_depth: TassadarScalingPressure,
    trace_length: TassadarScalingPressure,
    memory_footprint: TassadarScalingPressure,
    import_complexity: TassadarScalingPressure,
) -> TassadarScalingAxisPressureVector {
    TassadarScalingAxisPressureVector {
        call_graph_width,
        control_flow_depth,
        trace_length,
        memory_footprint,
        import_complexity,
    }
}

/// Contract validation failure.
#[derive(Debug, Error)]
pub enum TassadarKernelModuleScalingContractError {
    #[error("unsupported kernel-module scaling ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("kernel-module scaling contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("kernel-module scaling contract must declare workloads")]
    MissingWorkloads,
    #[error("kernel-module scaling contract must declare report refs")]
    MissingReportRefs,
}

/// Returns the canonical kernel-vs-module scaling contract.
#[must_use]
pub fn tassadar_kernel_module_scaling_contract() -> TassadarKernelModuleScalingContract {
    TassadarKernelModuleScalingContract::new()
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
        tassadar_kernel_module_scaling_contract, TassadarKernelModuleScalingFamily,
        TassadarScalingPhase, TassadarScalingPressure, TASSADAR_KERNEL_MODULE_SCALING_ABI_VERSION,
    };

    #[test]
    fn kernel_module_scaling_contract_is_machine_legible() {
        let contract = tassadar_kernel_module_scaling_contract();

        assert_eq!(
            contract.abi_version,
            TASSADAR_KERNEL_MODULE_SCALING_ABI_VERSION
        );
        assert!(contract
            .workload_rows
            .iter()
            .any(|row| row.phase == TassadarScalingPhase::KernelScale));
        assert!(contract.workload_rows.iter().any(|row| {
            row.workload_family == TassadarKernelModuleScalingFamily::ModuleHostImportBoundary
                && row.axis_pressures.import_complexity == TassadarScalingPressure::Boundary
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
