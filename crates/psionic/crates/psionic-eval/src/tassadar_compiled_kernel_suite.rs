use std::{collections::BTreeMap, time::Instant};

use psionic_data::{DatasetKey, TassadarBenchmarkAxis, TassadarBenchmarkFamily};
use psionic_environments::{
    EnvironmentDatasetBinding, EnvironmentPolicyKind, EnvironmentPolicyReference,
    TassadarBenchmarkPackageSetBinding, TassadarCompilePipelineMatrixBinding,
    TassadarEnvironmentBundle, TassadarEnvironmentSpec, TassadarExactnessContract,
    TassadarIoContract, TassadarProgramBinding, TassadarWasmConformanceBinding,
    TassadarWorkloadTarget,
};
use psionic_models::{
    TassadarCompiledProgramError, TassadarCompiledProgramExecution,
    TassadarCompiledProgramExecutor, TassadarCompiledProgramSuiteArtifact,
    TassadarExecutorContractError, TassadarExecutorFixture,
};
use psionic_runtime::{
    TassadarClaimClass, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarExecutorDecodeMode, TassadarInstruction, TassadarProgram, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarTraceAbi, TassadarWasmProfile,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF,
    TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF, TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
    TASSADAR_WASM_CONFORMANCE_REPORT_REF, TassadarBenchmarkError, TassadarReferenceFixtureSuite,
};

/// Stable environment ref for the compiled kernel-suite eval package.
pub const TASSADAR_COMPILED_KERNEL_SUITE_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.compiled_kernel_suite.eval";
/// Stable environment ref for the compiled kernel-suite benchmark package.
pub const TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.compiled_kernel_suite.benchmark";
/// Stable benchmark ref for the compiled kernel suite.
pub const TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/compiled_kernel_suite/reference_fixture";
/// Stable dataset ref for the compiled kernel suite.
pub const TASSADAR_COMPILED_KERNEL_SUITE_DATASET_REF: &str =
    "dataset://openagents/tassadar/compiled_kernel_suite";
/// Stable workload family id for the compiled kernel suite.
pub const TASSADAR_COMPILED_KERNEL_SUITE_WORKLOAD_FAMILY_ID: &str =
    "tassadar.wasm.article_i32_compute.v1.compiled_kernel_suite";
/// Stable compiled throughput metric id for the kernel suite.
pub const TASSADAR_COMPILED_KERNEL_SUITE_METRIC_ID: &str =
    "tassadar.compiled_kernel_suite_steps_per_second";

fn standard_compile_pipeline_matrix_binding() -> TassadarCompilePipelineMatrixBinding {
    TassadarCompilePipelineMatrixBinding {
        report_ref: String::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF),
        report_id: String::from("tassadar.compile_pipeline_matrix_report.v1"),
        source_family_ids: vec![
            String::from("wasm_text.multi_export_arithmetic"),
            String::from("wasm_text.memory_lookup"),
            String::from("wasm_text.param_abi"),
            String::from("c_source.toolchain_unavailable"),
        ],
    }
}

fn standard_wasm_conformance_binding() -> TassadarWasmConformanceBinding {
    TassadarWasmConformanceBinding {
        window_report_ref: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF),
        window_report_id: String::from("tassadar.frozen_core_wasm_window.report.v1"),
        window_id: String::from("tassadar.frozen_core_wasm.window.v1"),
        official_harness_id: String::from("tassadar.frozen_core_wasm.harness.v1"),
        text_authority_id: String::from("wat.reference.v1"),
        binary_decode_authority_id: String::from("wasmparser.decode.0.244.0"),
        binary_encode_authority_id: String::from("wasm_encoder.encode.0.244.0"),
        validation_authority_id: String::from("wasmparser.validate.core_int_first.v1"),
        report_ref: String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF),
        report_id: String::from("tassadar.wasm_conformance.report.v1"),
        reference_authority_id: String::from("wasmi.reference.v1"),
        case_family_ids: vec![
            String::from("curated.global_state"),
            String::from("curated.call_indirect"),
            String::from("curated.deterministic_import"),
            String::from("curated.call_indirect_trap"),
            String::from("curated.unsupported_host_import"),
            String::from("generated.call_indirect"),
            String::from("generated.call_indirect_trap"),
            String::from("generated.global_state"),
        ],
        unsupported_proposal_family_ids: vec![
            String::from("component_model"),
            String::from("exceptions"),
            String::from("floating_point"),
            String::from("function_references"),
            String::from("gc"),
            String::from("memory64"),
            String::from("multi_memory"),
            String::from("multi_value"),
            String::from("relaxed_simd"),
            String::from("saturating_float_to_int"),
            String::from("simd"),
            String::from("tail_call"),
            String::from("threads"),
        ],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledKernelFamilyId {
    ArithmeticKernel,
    MemoryUpdateKernel,
    ForwardBranchKernel,
    BackwardLoopKernel,
}

impl TassadarCompiledKernelFamilyId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArithmeticKernel => "arithmetic_kernel",
            Self::MemoryUpdateKernel => "memory_update_kernel",
            Self::ForwardBranchKernel => "forward_branch_kernel",
            Self::BackwardLoopKernel => "backward_loop_kernel",
        }
    }

    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::ArithmeticKernel => {
                "local-only arithmetic cascade over the article i32 executor profile"
            }
            Self::MemoryUpdateKernel => {
                "memory update accumulator with repeated load/add/store passes"
            }
            Self::ForwardBranchKernel => {
                "acyclic forward-branch ladder with alternating taken and untaken pivots"
            }
            Self::BackwardLoopKernel => {
                "backward-branch decrement loop that grows trace length honestly"
            }
        }
    }

    #[must_use]
    pub const fn workload_target(self) -> TassadarWorkloadTarget {
        match self {
            Self::ArithmeticKernel => TassadarWorkloadTarget::ArithmeticMicroprogram,
            Self::MemoryUpdateKernel => TassadarWorkloadTarget::MemoryLookupMicroprogram,
            Self::ForwardBranchKernel => TassadarWorkloadTarget::BranchControlFlowMicroprogram,
            Self::BackwardLoopKernel => TassadarWorkloadTarget::LongLoopKernel,
        }
    }

    #[must_use]
    pub const fn length_parameter_name(self) -> &'static str {
        match self {
            Self::ArithmeticKernel => "operation_count",
            Self::MemoryUpdateKernel => "slot_count",
            Self::ForwardBranchKernel => "branch_count",
            Self::BackwardLoopKernel => "iteration_count",
        }
    }

    #[must_use]
    pub const fn claim_boundary(self) -> &'static str {
        match self {
            Self::ArithmeticKernel => {
                "compiled exactness here covers a bounded arithmetic cascade family under the article_i32 profile; it is evidence for executor integrity, not arbitrary-program closure"
            }
            Self::MemoryUpdateKernel => {
                "compiled exactness here covers bounded memory update kernels with real load/store mutation under the article_i32 profile; it does not claim whole-program closure by itself"
            }
            Self::ForwardBranchKernel => {
                "compiled exactness here covers bounded forward-branch kernels with varied control flow under the article_i32 profile; it is one control-flow family, not a universal claim"
            }
            Self::BackwardLoopKernel => {
                "compiled exactness here covers bounded backward-loop kernels with explicit trace-length growth under the article_i32 profile; it keeps the horizon explicit instead of implying arbitrary long-horizon closure"
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TassadarCompiledKernelSuiteCaseSpec {
    case_id: String,
    family_id: TassadarCompiledKernelFamilyId,
    regime_id: String,
    summary: String,
    length_parameter_name: String,
    length_parameter_value: u64,
    program: TassadarProgram,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TassadarCompiledKernelSuiteCorpusCase {
    pub case_id: String,
    pub family_id: TassadarCompiledKernelFamilyId,
    pub regime_id: String,
    pub summary: String,
    pub length_parameter_name: String,
    pub length_parameter_value: u64,
    pub program_artifact: TassadarProgramArtifact,
    pub compiled_executor: TassadarCompiledProgramExecutor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TassadarCompiledKernelSuiteCorpus {
    pub workload_family_id: String,
    pub cases: Vec<TassadarCompiledKernelSuiteCorpusCase>,
    pub compiled_suite_artifact: TassadarCompiledProgramSuiteArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteCaseExactnessReport {
    pub case_id: String,
    pub family_id: TassadarCompiledKernelFamilyId,
    pub regime_id: String,
    pub summary: String,
    pub length_parameter_name: String,
    pub length_parameter_value: u64,
    pub program_artifact_digest: String,
    pub program_digest: String,
    pub compiled_weight_artifact_digest: String,
    pub runtime_contract_digest: String,
    pub compile_trace_proof_digest: String,
    pub compile_execution_proof_bundle_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    pub trace_step_count: u64,
    pub instruction_count: u32,
    pub cpu_trace_digest: String,
    pub compiled_trace_digest: String,
    pub cpu_behavior_digest: String,
    pub compiled_behavior_digest: String,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub halt_match: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteFamilyExactnessReport {
    pub family_id: TassadarCompiledKernelFamilyId,
    pub total_case_count: u32,
    pub exact_trace_case_count: u32,
    pub exact_trace_rate_bps: u32,
    pub max_trace_step_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteExactnessReport {
    pub workload_family_id: String,
    pub compiled_suite_artifact_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub total_case_count: u32,
    pub exact_trace_case_count: u32,
    pub exact_trace_rate_bps: u32,
    pub final_output_match_case_count: u32,
    pub halt_match_case_count: u32,
    pub family_reports: Vec<TassadarCompiledKernelSuiteFamilyExactnessReport>,
    pub case_reports: Vec<TassadarCompiledKernelSuiteCaseExactnessReport>,
    pub report_digest: String,
}

impl TassadarCompiledKernelSuiteExactnessReport {
    fn new(
        compiled_suite_artifact_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        case_reports: Vec<TassadarCompiledKernelSuiteCaseExactnessReport>,
    ) -> Self {
        let total_case_count = case_reports.len() as u32;
        let exact_trace_case_count = case_reports
            .iter()
            .filter(|case| case.exact_trace_match)
            .count() as u32;
        let final_output_match_case_count = case_reports
            .iter()
            .filter(|case| case.final_output_match)
            .count() as u32;
        let halt_match_case_count =
            case_reports.iter().filter(|case| case.halt_match).count() as u32;
        let family_reports = build_family_exactness_reports(&case_reports);
        let mut report = Self {
            workload_family_id: String::from(TASSADAR_COMPILED_KERNEL_SUITE_WORKLOAD_FAMILY_ID),
            compiled_suite_artifact_digest,
            requested_decode_mode,
            total_case_count,
            exact_trace_case_count,
            exact_trace_rate_bps: ratio_bps(exact_trace_case_count, total_case_count),
            final_output_match_case_count,
            halt_match_case_count,
            family_reports,
            case_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_compiled_kernel_suite_exactness_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledKernelSuiteRefusalKind {
    ProgramArtifactDigestMismatch,
    WasmProfileMismatch,
    TraceAbiVersionMismatch,
    ProgramArtifactInconsistent,
    UnexpectedSuccess,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteRefusalCheckReport {
    pub case_id: String,
    pub family_id: TassadarCompiledKernelFamilyId,
    pub check_id: String,
    pub expected_refusal_kind: TassadarCompiledKernelSuiteRefusalKind,
    pub observed_refusal_kind: TassadarCompiledKernelSuiteRefusalKind,
    pub matched_expected_refusal: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteFamilyCompatibilityReport {
    pub family_id: TassadarCompiledKernelFamilyId,
    pub total_check_count: u32,
    pub matched_refusal_check_count: u32,
    pub matched_refusal_rate_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteCompatibilityReport {
    pub workload_family_id: String,
    pub compiled_suite_artifact_digest: String,
    pub total_check_count: u32,
    pub matched_refusal_check_count: u32,
    pub matched_refusal_rate_bps: u32,
    pub family_reports: Vec<TassadarCompiledKernelSuiteFamilyCompatibilityReport>,
    pub check_reports: Vec<TassadarCompiledKernelSuiteRefusalCheckReport>,
    pub report_digest: String,
}

impl TassadarCompiledKernelSuiteCompatibilityReport {
    fn new(
        compiled_suite_artifact_digest: String,
        check_reports: Vec<TassadarCompiledKernelSuiteRefusalCheckReport>,
    ) -> Self {
        let total_check_count = check_reports.len() as u32;
        let matched_refusal_check_count = check_reports
            .iter()
            .filter(|check| check.matched_expected_refusal)
            .count() as u32;
        let family_reports = build_family_compatibility_reports(&check_reports);
        let mut report = Self {
            workload_family_id: String::from(TASSADAR_COMPILED_KERNEL_SUITE_WORKLOAD_FAMILY_ID),
            compiled_suite_artifact_digest,
            total_check_count,
            matched_refusal_check_count,
            matched_refusal_rate_bps: ratio_bps(matched_refusal_check_count, total_check_count),
            family_reports,
            check_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_compiled_kernel_suite_compatibility_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteScalingRegimeReport {
    pub case_id: String,
    pub family_id: TassadarCompiledKernelFamilyId,
    pub regime_id: String,
    pub summary: String,
    pub length_parameter_name: String,
    pub length_parameter_value: u64,
    pub instruction_count: u32,
    pub memory_slot_count: u32,
    pub trace_step_count: u64,
    pub exactness_bps: u32,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    pub cpu_reference_steps_per_second: f64,
    pub compiled_executor_steps_per_second: f64,
    pub compiled_over_cpu_ratio: f64,
    pub program_artifact_digest: String,
    pub compiled_weight_artifact_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
    pub runtime_trace_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteScalingFamilyReport {
    pub family_id: TassadarCompiledKernelFamilyId,
    pub summary: String,
    pub length_parameter_name: String,
    pub claim_boundary: String,
    pub regimes: Vec<TassadarCompiledKernelSuiteScalingRegimeReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteScalingReport {
    pub workload_family_id: String,
    pub compiled_suite_artifact_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub total_case_count: u32,
    pub family_reports: Vec<TassadarCompiledKernelSuiteScalingFamilyReport>,
    pub report_digest: String,
}

impl TassadarCompiledKernelSuiteScalingReport {
    fn new(
        compiled_suite_artifact_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        family_reports: Vec<TassadarCompiledKernelSuiteScalingFamilyReport>,
    ) -> Self {
        let total_case_count = family_reports
            .iter()
            .map(|family| family.regimes.len() as u32)
            .sum();
        let mut report = Self {
            workload_family_id: String::from(TASSADAR_COMPILED_KERNEL_SUITE_WORKLOAD_FAMILY_ID),
            compiled_suite_artifact_digest,
            requested_decode_mode,
            total_case_count,
            family_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_compiled_kernel_suite_scaling_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledKernelSuiteLaneClaimStatus {
    Exact,
    NotDone,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteClaimBoundaryReport {
    pub workload_family_id: String,
    pub claim_class: TassadarClaimClass,
    pub compiled_lane_status: TassadarCompiledKernelSuiteLaneClaimStatus,
    pub compiled_lane_detail: String,
    pub learned_lane_status: TassadarCompiledKernelSuiteLaneClaimStatus,
    pub learned_lane_detail: String,
    pub report_digest: String,
}

impl TassadarCompiledKernelSuiteClaimBoundaryReport {
    fn new() -> Self {
        let mut report = Self {
            workload_family_id: String::from(TASSADAR_COMPILED_KERNEL_SUITE_WORKLOAD_FAMILY_ID),
            claim_class: TassadarClaimClass::CompiledArticleClass,
            compiled_lane_status: TassadarCompiledKernelSuiteLaneClaimStatus::Exact,
            compiled_lane_detail: String::from(
                "exact compiled/proof-backed generic kernel suite is landed for bounded arithmetic, memory-update, forward-branch, and backward-loop families under the article_i32 profile; it widens compiled closure beyond Sudoku and Hungarian but does not claim arbitrary-program parity by itself",
            ),
            learned_lane_status: TassadarCompiledKernelSuiteLaneClaimStatus::NotDone,
            learned_lane_detail: String::from(
                "no learned generic long-horizon kernel suite is landed; current closure here is compiled/proof-backed only",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_compiled_kernel_suite_claim_boundary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarCompiledKernelSuiteEvalError {
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    #[error(transparent)]
    Compiled(#[from] TassadarCompiledProgramError),
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
}

#[must_use]
fn build_tassadar_compiled_kernel_suite_case_specs() -> Vec<TassadarCompiledKernelSuiteCaseSpec> {
    let arithmetic =
        [8usize, 32, 96]
            .into_iter()
            .map(|operation_count| TassadarCompiledKernelSuiteCaseSpec {
                case_id: format!("arithmetic_kernel_ops_{operation_count}"),
                family_id: TassadarCompiledKernelFamilyId::ArithmeticKernel,
                regime_id: format!("operation_count_{operation_count}"),
                summary: format!(
                    "execute {operation_count} alternating arithmetic updates in locals"
                ),
                length_parameter_name: String::from("operation_count"),
                length_parameter_value: operation_count as u64,
                program: build_arithmetic_kernel_program(operation_count),
            });
    let memory =
        [4usize, 16, 32]
            .into_iter()
            .map(|slot_count| TassadarCompiledKernelSuiteCaseSpec {
                case_id: format!("memory_update_kernel_slots_{slot_count}"),
                family_id: TassadarCompiledKernelFamilyId::MemoryUpdateKernel,
                regime_id: format!("slot_count_{slot_count}"),
                summary: format!("update and accumulate the first {slot_count} memory slots"),
                length_parameter_name: String::from("slot_count"),
                length_parameter_value: slot_count as u64,
                program: build_memory_update_kernel_program(slot_count),
            });
    let branches =
        [2usize, 8, 16]
            .into_iter()
            .map(|branch_count| TassadarCompiledKernelSuiteCaseSpec {
                case_id: format!("forward_branch_kernel_branches_{branch_count}"),
                family_id: TassadarCompiledKernelFamilyId::ForwardBranchKernel,
                regime_id: format!("branch_count_{branch_count}"),
                summary: format!(
                    "evaluate {branch_count} forward-branch pivots with alternating lanes"
                ),
                length_parameter_name: String::from("branch_count"),
                length_parameter_value: branch_count as u64,
                program: build_forward_branch_kernel_program(branch_count),
            });
    let loops =
        [63i32, 255, 1023]
            .into_iter()
            .map(|iteration_count| TassadarCompiledKernelSuiteCaseSpec {
                case_id: format!("backward_loop_kernel_iters_{iteration_count}"),
                family_id: TassadarCompiledKernelFamilyId::BackwardLoopKernel,
                regime_id: format!("iteration_count_{iteration_count}"),
                summary: format!(
                    "count down from {iteration_count} through one backward-branch loop"
                ),
                length_parameter_name: String::from("iteration_count"),
                length_parameter_value: iteration_count as u64,
                program: build_backward_loop_kernel_program(iteration_count),
            });

    arithmetic
        .chain(memory)
        .chain(branches)
        .chain(loops)
        .collect()
}

pub fn build_tassadar_compiled_kernel_suite(
    version: &str,
) -> Result<TassadarReferenceFixtureSuite, TassadarCompiledKernelSuiteEvalError> {
    let artifacts = tassadar_compiled_kernel_suite_program_artifacts(version)?;
    let corpus_digest = stable_digest(
        b"psionic_tassadar_compiled_kernel_suite_corpus_digest|",
        &artifacts
            .iter()
            .map(|artifact| {
                (
                    artifact.artifact_id.clone(),
                    artifact.artifact_digest.clone(),
                    artifact.validated_program_digest.clone(),
                )
            })
            .collect::<Vec<_>>(),
    );
    let environment_bundle = build_tassadar_compiled_kernel_suite_environment_bundle(
        version,
        &artifacts,
        &corpus_digest,
    )?;
    let benchmark_package = build_tassadar_compiled_kernel_suite_benchmark_package(
        version,
        &environment_bundle,
        &artifacts,
    )?;
    Ok(TassadarReferenceFixtureSuite {
        environment_bundle,
        benchmark_package,
        artifacts,
        corpus_digest,
    })
}

pub fn tassadar_compiled_kernel_suite_program_artifacts(
    version: &str,
) -> Result<Vec<TassadarProgramArtifact>, TassadarCompiledKernelSuiteEvalError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    build_tassadar_compiled_kernel_suite_case_specs()
        .into_iter()
        .map(|case| {
            TassadarProgramArtifact::fixture_reference(
                format!(
                    "tassadar://artifact/{version}/compiled_kernel_suite/{}",
                    case.case_id
                ),
                &profile,
                &trace_abi,
                case.program,
            )
            .map_err(TassadarCompiledKernelSuiteEvalError::from)
        })
        .collect()
}

pub fn build_tassadar_compiled_kernel_suite_corpus()
-> Result<TassadarCompiledKernelSuiteCorpus, TassadarCompiledKernelSuiteEvalError> {
    let fixture = TassadarExecutorFixture::article_i32_compute_v1();
    let mut cases = Vec::new();
    let mut artifacts = Vec::new();
    for spec in build_tassadar_compiled_kernel_suite_case_specs() {
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("{}.compiled_program_artifact", spec.case_id),
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            spec.program,
        )?;
        let compiled_executor =
            fixture.compile_program(format!("{}.compiled_executor", spec.case_id), &artifact)?;
        artifacts.push(artifact.clone());
        cases.push(TassadarCompiledKernelSuiteCorpusCase {
            case_id: spec.case_id,
            family_id: spec.family_id,
            regime_id: spec.regime_id,
            summary: spec.summary,
            length_parameter_name: spec.length_parameter_name,
            length_parameter_value: spec.length_parameter_value,
            program_artifact: artifact,
            compiled_executor,
        });
    }
    let compiled_suite_artifact = TassadarCompiledProgramSuiteArtifact::compile(
        "tassadar.compiled_kernel_suite",
        "benchmark://tassadar/compiled_kernel_suite@v0",
        &fixture,
        artifacts.as_slice(),
    )?;
    Ok(TassadarCompiledKernelSuiteCorpus {
        workload_family_id: String::from(TASSADAR_COMPILED_KERNEL_SUITE_WORKLOAD_FAMILY_ID),
        cases,
        compiled_suite_artifact,
    })
}

pub fn build_tassadar_compiled_kernel_suite_exactness_report(
    corpus: &TassadarCompiledKernelSuiteCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<TassadarCompiledKernelSuiteExactnessReport, TassadarCompiledKernelSuiteEvalError> {
    let mut case_reports = Vec::with_capacity(corpus.cases.len());
    for corpus_case in &corpus.cases {
        let cpu_execution = TassadarCpuReferenceRunner::for_program(
            &corpus_case.program_artifact.validated_program,
        )?
        .execute(&corpus_case.program_artifact.validated_program)?;
        let compiled_execution = corpus_case
            .compiled_executor
            .execute(&corpus_case.program_artifact, requested_decode_mode)?;
        let runtime_execution = &compiled_execution.execution_report.execution;
        case_reports.push(TassadarCompiledKernelSuiteCaseExactnessReport {
            case_id: corpus_case.case_id.clone(),
            family_id: corpus_case.family_id,
            regime_id: corpus_case.regime_id.clone(),
            summary: corpus_case.summary.clone(),
            length_parameter_name: corpus_case.length_parameter_name.clone(),
            length_parameter_value: corpus_case.length_parameter_value,
            program_artifact_digest: corpus_case.program_artifact.artifact_digest.clone(),
            program_digest: corpus_case
                .program_artifact
                .validated_program_digest
                .clone(),
            compiled_weight_artifact_digest: corpus_case
                .compiled_executor
                .compiled_weight_artifact()
                .artifact_digest
                .clone(),
            runtime_contract_digest: corpus_case
                .compiled_executor
                .runtime_contract()
                .contract_digest
                .clone(),
            compile_trace_proof_digest: corpus_case
                .compiled_executor
                .compile_evidence_bundle()
                .trace_proof
                .proof_digest
                .clone(),
            compile_execution_proof_bundle_digest: corpus_case
                .compiled_executor
                .compile_evidence_bundle()
                .proof_bundle
                .stable_digest(),
            runtime_execution_proof_bundle_digest: compiled_execution
                .evidence_bundle
                .proof_bundle
                .stable_digest(),
            requested_decode_mode,
            effective_decode_mode: compiled_execution
                .execution_report
                .selection
                .effective_decode_mode
                .unwrap_or(TassadarExecutorDecodeMode::ReferenceLinear),
            trace_step_count: runtime_execution.steps.len() as u64,
            instruction_count: corpus_case
                .program_artifact
                .validated_program
                .instructions
                .len() as u32,
            cpu_trace_digest: cpu_execution.trace_digest(),
            compiled_trace_digest: runtime_execution.trace_digest(),
            cpu_behavior_digest: cpu_execution.behavior_digest(),
            compiled_behavior_digest: runtime_execution.behavior_digest(),
            exact_trace_match: runtime_execution.steps == cpu_execution.steps,
            final_output_match: runtime_execution.outputs == cpu_execution.outputs,
            halt_match: runtime_execution.halt_reason == cpu_execution.halt_reason,
        });
    }
    Ok(TassadarCompiledKernelSuiteExactnessReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        requested_decode_mode,
        case_reports,
    ))
}

pub fn build_tassadar_compiled_kernel_suite_compatibility_report(
    corpus: &TassadarCompiledKernelSuiteCorpus,
) -> Result<TassadarCompiledKernelSuiteCompatibilityReport, TassadarCompiledKernelSuiteEvalError> {
    let mut check_reports = Vec::new();
    for (index, corpus_case) in corpus.cases.iter().enumerate() {
        let wrong_case = &corpus.cases[(index + 1) % corpus.cases.len()];
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            corpus_case.family_id,
            "wrong_program_artifact",
            TassadarCompiledKernelSuiteRefusalKind::ProgramArtifactDigestMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_case.program_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut wrong_profile_artifact = corpus_case.program_artifact.clone();
        wrong_profile_artifact.wasm_profile_id =
            TassadarWasmProfile::sudoku_v0_search_v1().profile_id;
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            corpus_case.family_id,
            "wrong_wasm_profile",
            TassadarCompiledKernelSuiteRefusalKind::WasmProfileMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_profile_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut wrong_trace_abi_artifact = corpus_case.program_artifact.clone();
        wrong_trace_abi_artifact.trace_abi_version = TassadarTraceAbi::article_i32_compute_v1()
            .schema_version
            .saturating_add(1);
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            corpus_case.family_id,
            "wrong_trace_abi_version",
            TassadarCompiledKernelSuiteRefusalKind::TraceAbiVersionMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_trace_abi_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut inconsistent_artifact = corpus_case.program_artifact.clone();
        inconsistent_artifact.validated_program_digest = String::from("bogus_program_digest");
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            corpus_case.family_id,
            "artifact_inconsistent",
            TassadarCompiledKernelSuiteRefusalKind::ProgramArtifactInconsistent,
            corpus_case.compiled_executor.execute(
                &inconsistent_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));
    }
    Ok(TassadarCompiledKernelSuiteCompatibilityReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        check_reports,
    ))
}

pub fn build_tassadar_compiled_kernel_suite_scaling_report(
    corpus: &TassadarCompiledKernelSuiteCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<TassadarCompiledKernelSuiteScalingReport, TassadarCompiledKernelSuiteEvalError> {
    let mut grouped = BTreeMap::<
        TassadarCompiledKernelFamilyId,
        Vec<TassadarCompiledKernelSuiteScalingRegimeReport>,
    >::new();
    for corpus_case in &corpus.cases {
        let cpu_execution = TassadarCpuReferenceRunner::for_program(
            &corpus_case.program_artifact.validated_program,
        )?
        .execute(&corpus_case.program_artifact.validated_program)?;
        let trace_step_count = cpu_execution.steps.len() as u64;
        let cpu_reference_steps_per_second = single_run_steps_per_second(trace_step_count, || {
            TassadarCpuReferenceRunner::for_program(
                &corpus_case.program_artifact.validated_program,
            )?
            .execute(&corpus_case.program_artifact.validated_program)
        })?;
        let compiled_sample = corpus_case
            .compiled_executor
            .execute(&corpus_case.program_artifact, requested_decode_mode)?;
        let compiled_executor_steps_per_second =
            single_run_steps_per_second(trace_step_count, || {
                corpus_case
                    .compiled_executor
                    .execute(&corpus_case.program_artifact, requested_decode_mode)
            })?;
        let runtime_execution = &compiled_sample.execution_report.execution;
        let exact_trace_match = runtime_execution.steps == cpu_execution.steps;
        let final_output_match = runtime_execution.outputs == cpu_execution.outputs;
        let halt_match = runtime_execution.halt_reason == cpu_execution.halt_reason;
        grouped.entry(corpus_case.family_id).or_default().push(
            TassadarCompiledKernelSuiteScalingRegimeReport {
                case_id: corpus_case.case_id.clone(),
                family_id: corpus_case.family_id,
                regime_id: corpus_case.regime_id.clone(),
                summary: corpus_case.summary.clone(),
                length_parameter_name: corpus_case.length_parameter_name.clone(),
                length_parameter_value: corpus_case.length_parameter_value,
                instruction_count: corpus_case
                    .program_artifact
                    .validated_program
                    .instructions
                    .len() as u32,
                memory_slot_count: corpus_case.program_artifact.validated_program.memory_slots
                    as u32,
                trace_step_count,
                exactness_bps: ((u32::from(exact_trace_match)
                    + u32::from(final_output_match)
                    + u32::from(halt_match))
                    * 10_000)
                    / 3,
                requested_decode_mode,
                effective_decode_mode: compiled_sample
                    .execution_report
                    .selection
                    .effective_decode_mode
                    .unwrap_or(TassadarExecutorDecodeMode::ReferenceLinear),
                cpu_reference_steps_per_second: round_metric(cpu_reference_steps_per_second),
                compiled_executor_steps_per_second: round_metric(
                    compiled_executor_steps_per_second,
                ),
                compiled_over_cpu_ratio: round_metric(
                    compiled_executor_steps_per_second / cpu_reference_steps_per_second.max(1e-9),
                ),
                program_artifact_digest: corpus_case.program_artifact.artifact_digest.clone(),
                compiled_weight_artifact_digest: corpus_case
                    .compiled_executor
                    .compiled_weight_artifact()
                    .artifact_digest
                    .clone(),
                runtime_execution_proof_bundle_digest: compiled_sample
                    .evidence_bundle
                    .proof_bundle
                    .stable_digest(),
                runtime_trace_digest: runtime_execution.trace_digest(),
            },
        );
    }

    let family_reports = grouped
        .into_iter()
        .map(|(family_id, mut regimes)| {
            regimes.sort_by_key(|regime| regime.length_parameter_value);
            TassadarCompiledKernelSuiteScalingFamilyReport {
                family_id,
                summary: String::from(family_id.summary()),
                length_parameter_name: String::from(family_id.length_parameter_name()),
                claim_boundary: String::from(family_id.claim_boundary()),
                regimes,
            }
        })
        .collect::<Vec<_>>();
    Ok(TassadarCompiledKernelSuiteScalingReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        requested_decode_mode,
        family_reports,
    ))
}

#[must_use]
pub fn build_tassadar_compiled_kernel_suite_claim_boundary_report()
-> TassadarCompiledKernelSuiteClaimBoundaryReport {
    TassadarCompiledKernelSuiteClaimBoundaryReport::new()
}

fn build_tassadar_compiled_kernel_suite_environment_bundle(
    version: &str,
    artifacts: &[TassadarProgramArtifact],
    corpus_digest: &str,
) -> Result<TassadarEnvironmentBundle, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let dataset = DatasetKey::new(TASSADAR_COMPILED_KERNEL_SUITE_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Compiled Kernel Suite"),
        eval_environment_ref: String::from(TASSADAR_COMPILED_KERNEL_SUITE_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(
            TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_ENVIRONMENT_REF,
        ),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/compiled_kernel_suite/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/compiled_kernel_suite/benchmark"),
            required: true,
        },
        package_refs: psionic_environments::TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.compiled_kernel_suite"),
            eval_pin_alias: String::from("tassadar_compiled_kernel_suite_eval"),
            benchmark_pin_alias: String::from("tassadar_compiled_kernel_suite_benchmark"),
            eval_member_ref: String::from("tassadar_compiled_kernel_suite_eval_member"),
            benchmark_member_ref: String::from("tassadar_compiled_kernel_suite_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/compiled_kernel_suite.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/compiled_kernel_suite/eval"),
            benchmark_profile_ref: String::from(
                "benchmark://tassadar/compiled_kernel_suite/reference_fixture",
            ),
            benchmark_runtime_profile_ref: String::from(
                "runtime://tassadar/compiled_kernel_suite/benchmark",
            ),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/compiled_kernel_suite.validation"),
            corpus_digest: String::from(corpus_digest),
            wasm_profile_id: profile.profile_id.clone(),
            trace_abi_id: trace_abi.abi_id.clone(),
            trace_abi_version: trace_abi.schema_version,
            opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
            artifact_digests: artifacts
                .iter()
                .map(|artifact| artifact.artifact_digest.clone())
                .collect(),
        },
        io_contract: TassadarIoContract::exact_i32_sequence(),
        exactness_contract: TassadarExactnessContract {
            require_final_output_exactness: true,
            require_step_exactness: true,
            require_halt_exactness: true,
            timeout_budget_ms: 30_000,
            trace_budget_steps: 131_072,
            require_cpu_reference_baseline: true,
            require_reference_linear_baseline: true,
            future_throughput_metric_ids: vec![String::from(
                TASSADAR_COMPILED_KERNEL_SUITE_METRIC_ID,
            )],
        },
        benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
            package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
            package_set_version: String::from(version),
            supported_families: vec![
                TassadarBenchmarkFamily::Arithmetic,
                TassadarBenchmarkFamily::TraceLengthStress,
            ],
            axis_coverage: vec![
                TassadarBenchmarkAxis::Exactness,
                TassadarBenchmarkAxis::LengthGeneralization,
                TassadarBenchmarkAxis::PlannerUsefulness,
            ],
            summary_report_ref: String::from(TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF),
        },
        compile_pipeline_matrix_binding: standard_compile_pipeline_matrix_binding(),
        execution_checkpoint_binding:
            psionic_environments::default_tassadar_execution_checkpoint_binding(),
        process_object_binding: psionic_environments::default_tassadar_process_object_binding(),
        spill_tape_store_binding:
            psionic_environments::default_tassadar_spill_tape_store_binding(),
        dynamic_memory_resume_binding:
            psionic_environments::default_tassadar_dynamic_memory_resume_binding(),
        simulator_effect_profile_binding:
            psionic_environments::default_tassadar_simulator_effect_profile_binding(),
        memory64_profile_binding: psionic_environments::default_tassadar_memory64_profile_binding(),
        broad_internal_compute_portability_binding: Some(
            psionic_environments::default_tassadar_broad_internal_compute_portability_binding(),
        ),
        numeric_portability_binding: Some(
            psionic_environments::default_tassadar_numeric_portability_binding(),
        ),
        float_semantics_binding: psionic_environments::default_tassadar_float_semantics_binding(),
        wasm_conformance_binding: standard_wasm_conformance_binding(),
        architecture_bakeoff_binding: Some(
            psionic_environments::default_tassadar_architecture_bakeoff_binding(),
        ),
        module_scale_workload_suite_binding: None,
        clrs_wasm_bridge_binding: None,
        eval_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Verification,
            policy_ref: String::from("policy://tassadar/compiled_kernel_suite/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/compiled_kernel_suite/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from(
                    "policy://tassadar/compiled_kernel_suite/benchmark/verification",
                ),
                required: true,
            },
        ],
        current_workload_targets: vec![
            TassadarWorkloadTarget::ArithmeticMicroprogram,
            TassadarWorkloadTarget::MemoryLookupMicroprogram,
            TassadarWorkloadTarget::BranchControlFlowMicroprogram,
            TassadarWorkloadTarget::LongLoopKernel,
        ],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
    .map_err(TassadarBenchmarkError::from)
}

fn build_tassadar_compiled_kernel_suite_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    artifacts: &[TassadarProgramArtifact],
) -> Result<BenchmarkPackage, TassadarBenchmarkError> {
    let case_specs = build_tassadar_compiled_kernel_suite_case_specs();
    if artifacts.len() != case_specs.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: case_specs.len(),
            actual: artifacts.len(),
        });
    }

    let cases = case_specs
        .iter()
        .zip(artifacts.iter())
        .enumerate()
        .map(|(ordinal, (spec, artifact))| {
            let cpu_execution =
                TassadarCpuReferenceRunner::for_program(&artifact.validated_program)
                    .and_then(|runner| runner.execute(&artifact.validated_program))
                    .map_err(TassadarBenchmarkError::from)?;
            let mut benchmark_case = BenchmarkCase::new(spec.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", spec.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", spec.case_id));
            benchmark_case.metadata = json!({
                "summary": spec.summary,
                "family_id": spec.family_id,
                "family_summary": spec.family_id.summary(),
                "length_parameter_name": spec.length_parameter_name,
                "length_parameter_value": spec.length_parameter_value,
                "workload_target": spec.family_id.workload_target(),
                "artifact_id": artifact.artifact_id,
                "artifact_digest": artifact.artifact_digest,
                "program_digest": artifact.validated_program_digest,
                "expected_outputs": cpu_execution.outputs,
                "expected_trace_steps": cpu_execution.steps.len(),
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms
            });
            Ok::<BenchmarkCase, TassadarBenchmarkError>(benchmark_case)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_REF, version),
        "Tassadar Compiled Kernel Suite Benchmark",
        environment_bundle.benchmark_package.key.clone(),
        1,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_dataset(
        environment_bundle.program_binding.dataset.clone(),
        Some(String::from("benchmark")),
    )
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: true,
        require_token_accounting: false,
        require_final_state_capture: true,
        require_execution_strategy: true,
    })
    .with_cases(cases);
    package.metadata.insert(
        String::from("tassadar.current_workload_targets"),
        serde_json::to_value(&environment_bundle.current_workload_targets)
            .unwrap_or(serde_json::Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.compiled_executor_metric_id"),
        serde_json::Value::String(String::from(TASSADAR_COMPILED_KERNEL_SUITE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        serde_json::Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.validate()?;
    Ok(package)
}

fn build_arithmetic_kernel_program(operation_count: usize) -> TassadarProgram {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let mut instructions = vec![
        TassadarInstruction::I32Const { value: 0 },
        TassadarInstruction::LocalSet { local: 0 },
    ];
    for operation_index in 0..operation_count {
        let value = (operation_index as i32 + 3) * 2;
        instructions.push(TassadarInstruction::LocalGet { local: 0 });
        instructions.push(TassadarInstruction::I32Const { value });
        if operation_index % 3 == 1 {
            instructions.push(TassadarInstruction::I32Sub);
        } else {
            instructions.push(TassadarInstruction::I32Add);
        }
        instructions.push(TassadarInstruction::LocalSet { local: 0 });
    }
    instructions.push(TassadarInstruction::LocalGet { local: 0 });
    instructions.push(TassadarInstruction::Output);
    instructions.push(TassadarInstruction::Return);
    TassadarProgram::new(
        format!("tassadar.compiled_kernel_suite.arithmetic.ops_{operation_count}.v1"),
        &profile,
        1,
        0,
        instructions,
    )
}

fn build_memory_update_kernel_program(slot_count: usize) -> TassadarProgram {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let mut instructions = vec![
        TassadarInstruction::I32Const { value: 0 },
        TassadarInstruction::LocalSet { local: 0 },
    ];
    for slot in 0..slot_count {
        instructions.push(TassadarInstruction::I32Load { slot: slot as u8 });
        instructions.push(TassadarInstruction::I32Const {
            value: (slot as i32) + 1,
        });
        instructions.push(TassadarInstruction::I32Add);
        instructions.push(TassadarInstruction::I32Store { slot: slot as u8 });
        instructions.push(TassadarInstruction::I32Load { slot: slot as u8 });
        instructions.push(TassadarInstruction::LocalGet { local: 0 });
        instructions.push(TassadarInstruction::I32Add);
        instructions.push(TassadarInstruction::LocalSet { local: 0 });
    }
    instructions.push(TassadarInstruction::LocalGet { local: 0 });
    instructions.push(TassadarInstruction::Output);
    instructions.push(TassadarInstruction::Return);
    TassadarProgram::new(
        format!("tassadar.compiled_kernel_suite.memory_update.slots_{slot_count}.v1"),
        &profile,
        1,
        slot_count,
        instructions,
    )
    .with_initial_memory((1..=slot_count as i32).collect())
}

fn build_forward_branch_kernel_program(branch_count: usize) -> TassadarProgram {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let mut instructions = vec![
        TassadarInstruction::I32Const { value: 0 },
        TassadarInstruction::LocalSet { local: 0 },
    ];
    let mut initial_memory = Vec::with_capacity(branch_count);
    for branch_index in 0..branch_count {
        initial_memory.push(i32::from(branch_index % 2 == 0));
        let false_value = 11 + (branch_index as i32 * 4);
        let true_value = 7 + (branch_index as i32 * 4);
        instructions.push(TassadarInstruction::I32Load {
            slot: branch_index as u8,
        });
        let select_true_pc = instructions.len();
        instructions.push(TassadarInstruction::BrIf { target_pc: 0 });
        instructions.push(TassadarInstruction::I32Const { value: false_value });
        instructions.push(TassadarInstruction::LocalGet { local: 0 });
        instructions.push(TassadarInstruction::I32Add);
        instructions.push(TassadarInstruction::LocalSet { local: 0 });
        instructions.push(TassadarInstruction::I32Const { value: 1 });
        let skip_true_pc = instructions.len();
        instructions.push(TassadarInstruction::BrIf { target_pc: 0 });

        let true_target_pc = instructions.len() as u16;
        instructions.push(TassadarInstruction::I32Const { value: true_value });
        instructions.push(TassadarInstruction::LocalGet { local: 0 });
        instructions.push(TassadarInstruction::I32Add);
        instructions.push(TassadarInstruction::LocalSet { local: 0 });

        let after_target_pc = instructions.len() as u16;
        if let TassadarInstruction::BrIf { target_pc } = &mut instructions[select_true_pc] {
            *target_pc = true_target_pc;
        }
        if let TassadarInstruction::BrIf { target_pc } = &mut instructions[skip_true_pc] {
            *target_pc = after_target_pc;
        }
    }
    instructions.push(TassadarInstruction::LocalGet { local: 0 });
    instructions.push(TassadarInstruction::Output);
    instructions.push(TassadarInstruction::Return);
    TassadarProgram::new(
        format!("tassadar.compiled_kernel_suite.forward_branch.count_{branch_count}.v1"),
        &profile,
        1,
        branch_count,
        instructions,
    )
    .with_initial_memory(initial_memory)
}

fn build_backward_loop_kernel_program(iteration_count: i32) -> TassadarProgram {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    TassadarProgram::new(
        format!("tassadar.compiled_kernel_suite.backward_loop.iters_{iteration_count}.v1"),
        &profile,
        1,
        0,
        vec![
            TassadarInstruction::I32Const {
                value: iteration_count,
            },
            TassadarInstruction::LocalSet { local: 0 },
            TassadarInstruction::LocalGet { local: 0 },
            TassadarInstruction::BrIf { target_pc: 7 },
            TassadarInstruction::I32Const { value: 0 },
            TassadarInstruction::Output,
            TassadarInstruction::Return,
            TassadarInstruction::LocalGet { local: 0 },
            TassadarInstruction::I32Const { value: 1 },
            TassadarInstruction::I32Sub,
            TassadarInstruction::LocalSet { local: 0 },
            TassadarInstruction::I32Const { value: 1 },
            TassadarInstruction::BrIf { target_pc: 2 },
        ],
    )
}

fn build_family_exactness_reports(
    case_reports: &[TassadarCompiledKernelSuiteCaseExactnessReport],
) -> Vec<TassadarCompiledKernelSuiteFamilyExactnessReport> {
    let mut grouped = BTreeMap::<
        TassadarCompiledKernelFamilyId,
        Vec<&TassadarCompiledKernelSuiteCaseExactnessReport>,
    >::new();
    for case_report in case_reports {
        grouped
            .entry(case_report.family_id)
            .or_default()
            .push(case_report);
    }
    grouped
        .into_iter()
        .map(|(family_id, reports)| {
            let total_case_count = reports.len() as u32;
            let exact_trace_case_count = reports
                .iter()
                .filter(|report| report.exact_trace_match)
                .count() as u32;
            let max_trace_step_count = reports
                .iter()
                .map(|report| report.trace_step_count)
                .max()
                .unwrap_or(0);
            TassadarCompiledKernelSuiteFamilyExactnessReport {
                family_id,
                total_case_count,
                exact_trace_case_count,
                exact_trace_rate_bps: ratio_bps(exact_trace_case_count, total_case_count),
                max_trace_step_count,
            }
        })
        .collect()
}

fn build_family_compatibility_reports(
    check_reports: &[TassadarCompiledKernelSuiteRefusalCheckReport],
) -> Vec<TassadarCompiledKernelSuiteFamilyCompatibilityReport> {
    let mut grouped = BTreeMap::<
        TassadarCompiledKernelFamilyId,
        Vec<&TassadarCompiledKernelSuiteRefusalCheckReport>,
    >::new();
    for check_report in check_reports {
        grouped
            .entry(check_report.family_id)
            .or_default()
            .push(check_report);
    }
    grouped
        .into_iter()
        .map(|(family_id, reports)| {
            let total_check_count = reports.len() as u32;
            let matched_refusal_check_count = reports
                .iter()
                .filter(|report| report.matched_expected_refusal)
                .count() as u32;
            TassadarCompiledKernelSuiteFamilyCompatibilityReport {
                family_id,
                total_check_count,
                matched_refusal_check_count,
                matched_refusal_rate_bps: ratio_bps(matched_refusal_check_count, total_check_count),
            }
        })
        .collect()
}

fn run_refusal_check(
    case_id: &str,
    family_id: TassadarCompiledKernelFamilyId,
    check_id: &str,
    expected_refusal_kind: TassadarCompiledKernelSuiteRefusalKind,
    outcome: Result<TassadarCompiledProgramExecution, TassadarCompiledProgramError>,
) -> TassadarCompiledKernelSuiteRefusalCheckReport {
    match outcome {
        Ok(_) => TassadarCompiledKernelSuiteRefusalCheckReport {
            case_id: case_id.to_string(),
            family_id,
            check_id: check_id.to_string(),
            expected_refusal_kind,
            observed_refusal_kind: TassadarCompiledKernelSuiteRefusalKind::UnexpectedSuccess,
            matched_expected_refusal: false,
            detail: String::from("compiled executor unexpectedly accepted mismatched artifact"),
        },
        Err(error) => {
            let observed_refusal_kind = refusal_kind_from_error(&error);
            TassadarCompiledKernelSuiteRefusalCheckReport {
                case_id: case_id.to_string(),
                family_id,
                check_id: check_id.to_string(),
                expected_refusal_kind,
                observed_refusal_kind,
                matched_expected_refusal: observed_refusal_kind == expected_refusal_kind,
                detail: error.to_string(),
            }
        }
    }
}

fn refusal_kind_from_error(
    error: &TassadarCompiledProgramError,
) -> TassadarCompiledKernelSuiteRefusalKind {
    match error {
        TassadarCompiledProgramError::DescriptorContract { error } => match error {
            TassadarExecutorContractError::ProgramArtifactInconsistent { .. } => {
                TassadarCompiledKernelSuiteRefusalKind::ProgramArtifactInconsistent
            }
            TassadarExecutorContractError::WasmProfileMismatch { .. } => {
                TassadarCompiledKernelSuiteRefusalKind::WasmProfileMismatch
            }
            TassadarExecutorContractError::TraceAbiVersionMismatch { .. } => {
                TassadarCompiledKernelSuiteRefusalKind::TraceAbiVersionMismatch
            }
            TassadarExecutorContractError::TraceAbiMismatch { .. }
            | TassadarExecutorContractError::OpcodeVocabularyDigestMismatch { .. }
            | TassadarExecutorContractError::ProgramProfileMismatch { .. }
            | TassadarExecutorContractError::DecodeModeUnsupported { .. } => {
                TassadarCompiledKernelSuiteRefusalKind::ProgramArtifactInconsistent
            }
        },
        TassadarCompiledProgramError::SelectionRefused { .. }
        | TassadarCompiledProgramError::ProgramDigestMismatch { .. } => {
            TassadarCompiledKernelSuiteRefusalKind::ProgramArtifactInconsistent
        }
        TassadarCompiledProgramError::ProgramArtifactDigestMismatch { .. } => {
            TassadarCompiledKernelSuiteRefusalKind::ProgramArtifactDigestMismatch
        }
    }
}

fn throughput_steps_per_second(steps: u64, elapsed_seconds: f64) -> f64 {
    steps as f64 / elapsed_seconds.max(1e-9)
}

fn single_run_steps_per_second<F, T, E>(steps: u64, runner: F) -> Result<f64, E>
where
    F: FnOnce() -> Result<T, E>,
{
    let started = Instant::now();
    runner()?;
    Ok(throughput_steps_per_second(
        steps.max(1),
        started.elapsed().as_secs_f64(),
    ))
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        return 0;
    }
    ((numerator as f64 / denominator as f64) * 10_000.0).round() as u32
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("Tassadar compiled kernel suite artifact should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarCompiledKernelFamilyId, TassadarCompiledKernelSuiteLaneClaimStatus,
        build_tassadar_compiled_kernel_suite,
        build_tassadar_compiled_kernel_suite_claim_boundary_report,
        build_tassadar_compiled_kernel_suite_compatibility_report,
        build_tassadar_compiled_kernel_suite_corpus,
        build_tassadar_compiled_kernel_suite_exactness_report,
        build_tassadar_compiled_kernel_suite_scaling_report,
    };
    use psionic_runtime::{TassadarClaimClass, TassadarExecutorDecodeMode};

    #[test]
    fn compiled_kernel_suite_builds_fixture_suite() -> Result<(), Box<dyn std::error::Error>> {
        let suite = build_tassadar_compiled_kernel_suite("v0")?;
        assert_eq!(suite.artifacts.len(), 12);
        assert_eq!(suite.benchmark_package.cases.len(), 12);
        Ok(())
    }

    #[test]
    fn compiled_kernel_suite_exactness_is_exact() -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_compiled_kernel_suite_corpus()?;
        let report = build_tassadar_compiled_kernel_suite_exactness_report(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        assert_eq!(report.total_case_count, 12);
        assert_eq!(report.exact_trace_case_count, 12);
        assert_eq!(report.exact_trace_rate_bps, 10_000);
        assert_eq!(report.family_reports.len(), 4);
        Ok(())
    }

    #[test]
    fn compiled_kernel_suite_compatibility_matches_expected_surface()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_compiled_kernel_suite_corpus()?;
        let report = build_tassadar_compiled_kernel_suite_compatibility_report(&corpus)?;
        assert_eq!(report.total_check_count, 48);
        assert_eq!(report.matched_refusal_check_count, 48);
        assert_eq!(report.matched_refusal_rate_bps, 10_000);
        Ok(())
    }

    #[test]
    fn compiled_kernel_suite_scaling_report_tracks_each_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_compiled_kernel_suite_corpus()?;
        let report = build_tassadar_compiled_kernel_suite_scaling_report(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        assert_eq!(report.total_case_count, 12);
        assert_eq!(report.family_reports.len(), 4);
        let loop_family = report
            .family_reports
            .iter()
            .find(|family| family.family_id == TassadarCompiledKernelFamilyId::BackwardLoopKernel)
            .expect("loop family");
        assert_eq!(loop_family.regimes.len(), 3);
        assert!(
            loop_family
                .regimes
                .windows(2)
                .all(|pair| pair[0].trace_step_count < pair[1].trace_step_count)
        );
        assert!(
            report
                .family_reports
                .iter()
                .flat_map(|family| family.regimes.iter())
                .all(|regime| regime.exactness_bps == 10_000)
        );
        Ok(())
    }

    #[test]
    fn compiled_kernel_suite_claim_boundary_report_is_honest() {
        let report = build_tassadar_compiled_kernel_suite_claim_boundary_report();
        assert_eq!(report.claim_class, TassadarClaimClass::CompiledArticleClass);
        assert_eq!(
            report.compiled_lane_status,
            TassadarCompiledKernelSuiteLaneClaimStatus::Exact
        );
        assert_eq!(
            report.learned_lane_status,
            TassadarCompiledKernelSuiteLaneClaimStatus::NotDone
        );
    }
}
