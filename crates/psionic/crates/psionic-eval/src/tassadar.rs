use std::{collections::BTreeMap, time::Instant};

use psionic_data::{DatasetKey, TassadarBenchmarkAxis, TassadarBenchmarkFamily};
use psionic_environments::{
    EnvironmentDatasetBinding, EnvironmentPolicyKind, EnvironmentPolicyReference,
    TassadarBenchmarkPackageSetBinding, TassadarCompilePipelineMatrixBinding,
    TassadarEnvironmentBundle, TassadarEnvironmentError, TassadarEnvironmentPackageRefs,
    TassadarEnvironmentSpec, TassadarExactnessContract, TassadarIoContract, TassadarProgramBinding,
    TassadarWasmConformanceBinding, TassadarWorkloadTarget,
};
use psionic_models::{TassadarExecutorContractError, TassadarExecutorFixture};
use psionic_runtime::{
    build_tassadar_execution_evidence_bundle, diagnose_tassadar_executor_request,
    run_tassadar_exact_equivalence, tassadar_article_class_corpus, tassadar_hungarian_10x10_corpus,
    tassadar_hungarian_v0_corpus, tassadar_sudoku_9x9_corpus, tassadar_sudoku_v0_corpus,
    tassadar_trace_abi_for_profile_id, tassadar_validation_corpus, TassadarClaimClass,
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarExecutorSelectionReason, TassadarExecutorSelectionState, TassadarFixtureRunner,
    TassadarHierarchicalHullCandidateRunner, TassadarHullCacheRunner,
    TassadarHungarian10x10CorpusCase, TassadarHungarianV0CorpusCase, TassadarInstruction,
    TassadarProgram, TassadarProgramArtifact, TassadarProgramArtifactError,
    TassadarSparseTopKRunner, TassadarSudoku9x9CorpusCase, TassadarSudokuV0CorpusCase,
    TassadarSudokuV0CorpusSplit, TassadarTraceAbi, TassadarTraceArtifact, TassadarWasmProfile,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Digest;
use thiserror::Error;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode, BenchmarkPackage,
    BenchmarkPackageKey, BenchmarkVerificationPolicy, EvalArtifact, EvalExecutionStrategyFacts,
    EvalFinalStateCapture, EvalMetric, EvalRunContract, EvalRunMode, EvalRunState,
    EvalRuntimeError, EvalSampleRecord, EvalSampleStatus, EvalTimerIntegrityFacts,
    EvalVerificationFacts, TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF,
    TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF, TASSADAR_WASM_CONFORMANCE_REPORT_REF,
};

/// Stable environment ref for the Tassadar eval package.
pub const TASSADAR_EVAL_ENVIRONMENT_REF: &str = "env.openagents.tassadar.eval";
/// Stable environment ref for the Tassadar benchmark package.
pub const TASSADAR_BENCHMARK_ENVIRONMENT_REF: &str = "env.openagents.tassadar.benchmark";
/// Stable benchmark ref for the Tassadar validation-corpus suite.
pub const TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/reference_fixture/validation_corpus";
/// Stable dataset ref for the current validation corpus.
pub const TASSADAR_VALIDATION_CORPUS_DATASET_REF: &str =
    "dataset://openagents/tassadar/validation_corpus";
/// Stable environment ref for the widened article-class eval package.
pub const TASSADAR_ARTICLE_CLASS_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.article_class.eval";
/// Stable environment ref for the widened article-class benchmark package.
pub const TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.article_class.benchmark";
/// Stable benchmark ref for the widened article-class suite.
pub const TASSADAR_ARTICLE_CLASS_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/article_class/reference_fixture";
/// Stable dataset ref for the widened article-class corpus.
pub const TASSADAR_ARTICLE_CLASS_DATASET_REF: &str =
    "dataset://openagents/tassadar/article_class_corpus";
/// Stable environment ref for the compiled 9x9 Sudoku eval package.
pub const TASSADAR_SUDOKU_9X9_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.sudoku_9x9.eval";
/// Stable environment ref for the compiled 9x9 Sudoku benchmark package.
pub const TASSADAR_SUDOKU_9X9_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.sudoku_9x9.benchmark";
/// Stable benchmark ref for the compiled 9x9 Sudoku suite.
pub const TASSADAR_SUDOKU_9X9_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/sudoku_9x9/reference_fixture";
/// Stable dataset ref for the compiled 9x9 Sudoku corpus.
pub const TASSADAR_SUDOKU_9X9_DATASET_REF: &str = "dataset://openagents/tassadar/sudoku_9x9_corpus";
/// Stable environment ref for the bounded Hungarian-v0 eval package.
pub const TASSADAR_HUNGARIAN_V0_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.hungarian_v0.eval";
/// Stable environment ref for the bounded Hungarian-v0 benchmark package.
pub const TASSADAR_HUNGARIAN_V0_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.hungarian_v0.benchmark";
/// Stable benchmark ref for the bounded Hungarian-v0 suite.
pub const TASSADAR_HUNGARIAN_V0_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/hungarian_v0/reference_fixture";
/// Stable dataset ref for the bounded Hungarian-v0 corpus.
pub const TASSADAR_HUNGARIAN_V0_DATASET_REF: &str =
    "dataset://openagents/tassadar/hungarian_v0_corpus";
/// Stable environment ref for the article-sized Hungarian-10x10 eval package.
pub const TASSADAR_HUNGARIAN_10X10_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.hungarian_10x10.eval";
/// Stable environment ref for the article-sized Hungarian-10x10 benchmark package.
pub const TASSADAR_HUNGARIAN_10X10_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.hungarian_10x10.benchmark";
/// Stable benchmark ref for the article-sized Hungarian-10x10 suite.
pub const TASSADAR_HUNGARIAN_10X10_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/hungarian_10x10/reference_fixture";
/// Stable dataset ref for the article-sized Hungarian-10x10 corpus.
pub const TASSADAR_HUNGARIAN_10X10_DATASET_REF: &str =
    "dataset://openagents/tassadar/hungarian_10x10_corpus";
/// Stable metric id for the Phase 5 hull-cache lane.
pub const TASSADAR_HULL_CACHE_METRIC_ID: &str = "tassadar.hull_cache_steps_per_second";
/// Stable metric id for the Phase 8 sparse-top-k lane.
pub const TASSADAR_SPARSE_TOP_K_METRIC_ID: &str = "tassadar.sparse_top_k_steps_per_second";
/// Canonical machine-readable output path for the workload capability matrix report.
pub const TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_workload_capability_matrix.json";
/// Canonical machine-readable output path for the widened HullCache closure report.
pub const TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hull_cache_closure_report.json";
/// Canonical machine-readable output path for the SparseTopK comparison report.
pub const TASSADAR_SPARSE_TOP_K_COMPARISON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_sparse_top_k_comparison_report.json";
/// Canonical machine-readable output path for the decode-scaling report.
pub const TASSADAR_DECODE_SCALING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_decode_scaling_report.json";
/// Canonical machine-readable output path for the geometric-variant comparison report.
pub const TASSADAR_GEOMETRIC_VARIANT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_geometric_variant_report.json";
/// Canonical machine-readable output path for the benchmark-package-set summary report.
pub const TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json";

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

const TASSADAR_OUTPUT_EXACTNESS_METRIC_ID: &str = "tassadar.final_output_exactness_bps";
const TASSADAR_STEP_EXACTNESS_METRIC_ID: &str = "tassadar.step_exactness_bps";
const TASSADAR_HALT_EXACTNESS_METRIC_ID: &str = "tassadar.halt_exactness_bps";
const TASSADAR_CPU_BASELINE_METRIC_ID: &str = "tassadar.cpu_reference_steps_per_second";
const TASSADAR_REFERENCE_LINEAR_METRIC_ID: &str = "tassadar.reference_linear_steps_per_second";
const TASSADAR_TRACE_DIGEST_EQUAL_METRIC_ID: &str = "tassadar.trace_digest_equal_bps";
const TASSADAR_HULL_CACHE_SPEEDUP_METRIC_ID: &str =
    "tassadar.hull_cache_speedup_over_reference_linear";
const TASSADAR_HULL_CACHE_CPU_GAP_METRIC_ID: &str =
    "tassadar.hull_cache_remaining_gap_vs_cpu_reference";
const TASSADAR_SPARSE_TOP_K_SPEEDUP_METRIC_ID: &str =
    "tassadar.sparse_top_k_speedup_over_reference_linear";
const TASSADAR_SPARSE_TOP_K_CPU_GAP_METRIC_ID: &str =
    "tassadar.sparse_top_k_remaining_gap_vs_cpu_reference";
const TASSADAR_TRACE_STEP_COUNT_METRIC_ID: &str = "tassadar.trace_step_count";
const TASSADAR_WORKLOAD_CAPABILITY_MATRIX_SCHEMA_VERSION: u16 = 1;
const TASSADAR_HULL_CACHE_CLOSURE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_SPARSE_TOP_K_COMPARISON_SCHEMA_VERSION: u16 = 1;
const TASSADAR_DECODE_SCALING_SCHEMA_VERSION: u16 = 1;
const TASSADAR_GEOMETRIC_VARIANT_REPORT_SCHEMA_VERSION: u16 = 1;
const TASSADAR_RUNTIME_HULL_GEOMETRIC_VARIANT_ID: &str =
    "tassadar.geometric_variant.hull_cache_runtime.v1";
const TASSADAR_HIERARCHICAL_HULL_GEOMETRIC_VARIANT_ID: &str =
    "tassadar.geometric_variant.hierarchical_hull_candidate.v0";

/// One packaged Tassadar Phase 3 suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarReferenceFixtureSuite {
    /// Environment bundle shared by eval and benchmark execution.
    pub environment_bundle: TassadarEnvironmentBundle,
    /// Packaged benchmark contract for the current corpus.
    pub benchmark_package: BenchmarkPackage,
    /// Digest-bound artifacts for the benchmark corpus.
    pub artifacts: Vec<TassadarProgramArtifact>,
    /// Stable digest over the ordered artifact set.
    pub corpus_digest: String,
}

/// Per-case benchmark result for the current Tassadar suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarBenchmarkCaseReport {
    /// Stable benchmark case id.
    pub case_id: String,
    /// Workload target for the case.
    pub workload_target: TassadarWorkloadTarget,
    /// Terminal sample status.
    pub status: EvalSampleStatus,
    /// Aggregate score in basis points.
    pub score_bps: u32,
    /// Final-output exactness score.
    pub final_output_exactness_bps: u32,
    /// Step exactness score.
    pub step_exactness_bps: u32,
    /// Halt exactness score.
    pub halt_exactness_bps: u32,
    /// Decode mode requested by the benchmark harness.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode after runtime direct/fallback selection.
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    /// Direct/fallback/refused state emitted before execution.
    pub selection_state: TassadarExecutorSelectionState,
    /// Stable reason for fallback or refusal when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Whether execution used an explicit decode fallback.
    pub used_decode_fallback: bool,
    /// Whether trace digests matched across CPU, linear, hull-cache, and sparse-top-k paths.
    pub trace_digest_equal: bool,
    /// Whether outputs matched across CPU, linear, hull-cache, and sparse-top-k paths.
    pub outputs_equal: bool,
    /// Whether halt reasons matched across CPU, linear, hull-cache, and sparse-top-k paths.
    pub halt_equal: bool,
    /// Observed trace-step count.
    pub trace_steps: u64,
    /// Direct CPU baseline throughput.
    pub cpu_reference_steps_per_second: f64,
    /// Reference-linear executor throughput.
    pub reference_linear_steps_per_second: f64,
    /// Hull-cache executor throughput.
    pub hull_cache_steps_per_second: f64,
    /// Speedup ratio of hull-cache over reference-linear execution.
    pub hull_cache_speedup_over_reference_linear: f64,
    /// Remaining CPU-reference gap ratio, computed as `cpu / hull`.
    pub hull_cache_remaining_gap_vs_cpu_reference: f64,
    /// Direct/fallback/refused state emitted for the sparse-top-k path.
    pub sparse_top_k_selection_state: TassadarExecutorSelectionState,
    /// Stable reason for sparse-top-k fallback or refusal when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse_top_k_selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Whether sparse-top-k execution used an explicit decode fallback.
    pub sparse_top_k_used_decode_fallback: bool,
    /// Sparse-top-k executor throughput.
    pub sparse_top_k_steps_per_second: f64,
    /// Speedup ratio of sparse-top-k over reference-linear execution.
    pub sparse_top_k_speedup_over_reference_linear: f64,
    /// Remaining CPU-reference gap ratio, computed as `cpu / sparse_top_k`.
    pub sparse_top_k_remaining_gap_vs_cpu_reference: f64,
    /// Runner-independent CPU behavior digest.
    pub cpu_behavior_digest: String,
    /// Runner-independent reference-linear behavior digest.
    pub reference_linear_behavior_digest: String,
    /// Runner-independent hull-cache behavior digest.
    pub hull_cache_behavior_digest: String,
    /// Runner-independent sparse-top-k behavior digest.
    pub sparse_top_k_behavior_digest: String,
}

/// Full report for one package-driven Tassadar benchmark run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarBenchmarkReport {
    /// Packaged suite identity.
    pub suite: TassadarReferenceFixtureSuite,
    /// Finalized benchmark-mode eval run.
    pub eval_run: EvalRunState,
    /// Aggregate benchmark summary.
    pub aggregate_summary: crate::BenchmarkAggregateSummary,
    /// Per-case benchmark reports.
    pub case_reports: Vec<TassadarBenchmarkCaseReport>,
}

/// Capability posture for one workload family on one Tassadar surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCapabilityPosture {
    /// Surface is exact on the declared workload family.
    Exact,
    /// Surface remains exact only by explicitly falling back to another mode.
    FallbackOnly,
    /// Surface exists but remains bounded or partial for this family.
    Partial,
    /// Surface is a research-only signal rather than a promoted capability.
    ResearchOnly,
    /// Surface has no landed capability for this family yet.
    NotLanded,
}

/// One capability cell in the Tassadar workload matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityCell {
    /// Stable surface identifier.
    pub surface_id: String,
    /// Current posture on the workload family.
    pub posture: TassadarCapabilityPosture,
    /// Repo-owned artifact anchoring the posture.
    pub artifact_ref: String,
    /// Plain-language note tied to the artifact.
    pub note: String,
}

/// One workload-family row in the capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityRow {
    /// Stable workload-family identifier.
    pub workload_family_id: String,
    /// Optional current benchmark workload target when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_target: Option<TassadarWorkloadTarget>,
    /// Short summary for the row.
    pub summary: String,
    /// Capability cells in stable surface order.
    pub capabilities: Vec<TassadarWorkloadCapabilityCell>,
}

/// Machine-readable workload capability matrix for the current Tassadar lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityMatrixReport {
    /// Stable schema version for the matrix report.
    pub schema_version: u16,
    /// Ordered artifact roots used to generate the matrix.
    pub generated_from_artifacts: Vec<String>,
    /// Ordered workload-family rows.
    pub rows: Vec<TassadarWorkloadCapabilityRow>,
    /// Plain-language boundary statement for the matrix.
    pub claim_boundary: String,
    /// Stable digest over the full report.
    pub report_digest: String,
}

/// One workload-family summary in the widened HullCache closure report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarHullCacheWorkloadSummary {
    /// Stable workload target for the summary.
    pub workload_target: TassadarWorkloadTarget,
    /// Current HullCache posture on the workload family.
    pub posture: TassadarCapabilityPosture,
    /// Number of cases that stayed direct on HullCache.
    pub direct_case_count: usize,
    /// Number of cases that fell back away from HullCache.
    pub fallback_case_count: usize,
    /// Average HullCache speedup over reference-linear execution.
    pub average_speedup_over_reference_linear: f64,
    /// Average remaining CPU gap ratio.
    pub average_remaining_gap_vs_cpu_reference: f64,
    /// Aggregated typed fallback reasons for the workload family.
    pub fallback_reason_counts: BTreeMap<String, u64>,
    /// Stable artifact ref anchoring the summary.
    pub artifact_ref: String,
}

/// Machine-readable widened HullCache closure report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarHullCacheClosureReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Direct exact workload families currently inside the widened closure.
    pub exact_workloads: Vec<TassadarHullCacheWorkloadSummary>,
    /// Workload families that remain fallback-only outside the direct closure.
    pub fallback_only_workloads: Vec<TassadarHullCacheWorkloadSummary>,
    /// Plain-language boundary statement for the current HullCache closure.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// One workload-family summary in the SparseTopK comparison report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSparseTopKWorkloadSummary {
    /// Stable workload target for the summary.
    pub workload_target: TassadarWorkloadTarget,
    /// Current SparseTopK posture on the workload family.
    pub posture: TassadarCapabilityPosture,
    /// Number of cases that stayed direct on SparseTopK.
    pub direct_case_count: usize,
    /// Number of cases that fell back away from SparseTopK.
    pub fallback_case_count: usize,
    /// Average SparseTopK speedup over reference-linear execution.
    pub average_speedup_over_reference_linear: f64,
    /// Average remaining CPU gap ratio.
    pub average_remaining_gap_vs_cpu_reference: f64,
    /// Average speed ratio of SparseTopK relative to HullCache on the same cases.
    pub average_sparse_vs_hull_speed_ratio: f64,
    /// Aggregated typed fallback reasons for the workload family.
    pub fallback_reason_counts: BTreeMap<String, u64>,
    /// Stable artifact ref anchoring the summary.
    pub artifact_ref: String,
}

/// Machine-readable SparseTopK comparison report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSparseTopKComparisonReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Direct exact workload families currently inside SparseTopK support.
    pub exact_workloads: Vec<TassadarSparseTopKWorkloadSummary>,
    /// Workload families that remain fallback-only outside direct SparseTopK support.
    pub fallback_only_workloads: Vec<TassadarSparseTopKWorkloadSummary>,
    /// Plain-language boundary statement for the current SparseTopK comparison.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// One trace-length scaling regime inside one synthetic workload family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarDecodeScalingRegimeReport {
    /// Stable regime identifier inside one workload family.
    pub regime_id: String,
    /// Short summary of the synthetic program shape.
    pub summary: String,
    /// Stable synthetic program identifier.
    pub program_id: String,
    /// Wasm profile targeted by the synthetic program.
    pub profile_id: String,
    /// Stable length-parameter field name for this family.
    pub length_parameter_name: String,
    /// Stable length-parameter value.
    pub length_parameter_value: u64,
    /// Program instruction count.
    pub instruction_count: usize,
    /// Exact trace-step count.
    pub trace_step_count: u64,
    /// Serialized trace-artifact byte count used as the current memory-growth proxy.
    pub trace_artifact_bytes: u64,
    /// Average serialized trace-artifact bytes per step.
    pub trace_artifact_bytes_per_step: f64,
    /// Maximum observed stack depth during execution.
    pub max_stack_depth: usize,
    /// Declared memory-slot count for the program.
    pub memory_slot_count: usize,
    /// CPU-reference throughput.
    pub cpu_reference_steps_per_second: f64,
    /// Reference-linear throughput.
    pub reference_linear_steps_per_second: f64,
    /// Effective HullCache-request throughput.
    pub hull_cache_steps_per_second: f64,
    /// Speedup ratio of the HullCache request over reference-linear execution.
    pub hull_cache_speedup_over_reference_linear: f64,
    /// Remaining CPU gap ratio for the HullCache request.
    pub hull_cache_remaining_gap_vs_cpu_reference: f64,
    /// HullCache-request selection state.
    pub hull_cache_selection_state: TassadarExecutorSelectionState,
    /// Stable fallback reason for the HullCache request when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hull_cache_selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Effective decode mode after HullCache request resolution.
    pub hull_cache_effective_decode_mode: TassadarExecutorDecodeMode,
    /// Effective SparseTopK-request throughput.
    pub sparse_top_k_steps_per_second: f64,
    /// Speedup ratio of the SparseTopK request over reference-linear execution.
    pub sparse_top_k_speedup_over_reference_linear: f64,
    /// Remaining CPU gap ratio for the SparseTopK request.
    pub sparse_top_k_remaining_gap_vs_cpu_reference: f64,
    /// SparseTopK-request selection state.
    pub sparse_top_k_selection_state: TassadarExecutorSelectionState,
    /// Stable fallback reason for the SparseTopK request when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse_top_k_selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Effective decode mode after SparseTopK request resolution.
    pub sparse_top_k_effective_decode_mode: TassadarExecutorDecodeMode,
    /// Whether traces matched across CPU, linear, hull, and sparse requests.
    pub trace_digest_equal: bool,
    /// Whether outputs matched across CPU, linear, hull, and sparse requests.
    pub outputs_equal: bool,
    /// Whether halt reasons matched across CPU, linear, hull, and sparse requests.
    pub halt_equal: bool,
    /// Aggregate exactness score in basis points.
    pub exactness_bps: u32,
    /// CPU-reference behavior digest.
    pub cpu_behavior_digest: String,
    /// Reference-linear behavior digest.
    pub reference_linear_behavior_digest: String,
    /// HullCache-request behavior digest.
    pub hull_cache_behavior_digest: String,
    /// SparseTopK-request behavior digest.
    pub sparse_top_k_behavior_digest: String,
}

/// One synthetic workload family in the decode-scaling report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarDecodeScalingFamilyReport {
    /// Stable family identifier.
    pub family_id: String,
    /// Short family summary.
    pub summary: String,
    /// Stable length-parameter field name shared across the family.
    pub length_parameter_name: String,
    /// Plain-language claim boundary for the family.
    pub claim_boundary: String,
    /// Ordered scaling regimes for the family.
    pub regimes: Vec<TassadarDecodeScalingRegimeReport>,
}

/// Machine-readable decode-scaling report across the current active decode modes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarDecodeScalingReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Ordered artifact anchors used to build the report.
    pub generated_from_artifacts: Vec<String>,
    /// Stable memory-growth metric recorded in each regime.
    pub memory_growth_metric: String,
    /// Ordered synthetic workload families.
    pub families: Vec<TassadarDecodeScalingFamilyReport>,
    /// Plain-language boundary for the full scaling report.
    pub claim_boundary: String,
    /// Stable digest over the full report.
    pub report_digest: String,
}

/// Runtime-ready versus research-only claim boundary for one geometric variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeometricVariantClaimBoundary {
    /// Variant is a promoted runtime surface with explicit selection truth.
    RuntimeReady,
    /// Variant remains research-only and is not a promoted runtime surface.
    ResearchOnly,
}

/// Per-case comparison artifact for one geometric fast-path variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarGeometricVariantCaseReport {
    /// Stable benchmark case identifier.
    pub case_id: String,
    /// Workload target for the case.
    pub workload_target: TassadarWorkloadTarget,
    /// Stable variant identifier.
    pub variant_id: String,
    /// Runtime-ready or research-only claim boundary for the variant.
    pub claim_boundary: TassadarGeometricVariantClaimBoundary,
    /// Current top-level Tassadar claim class for the variant.
    pub claim_class: TassadarClaimClass,
    /// Direct/fallback/refused state surfaced for the variant on this case.
    pub selection_state: TassadarExecutorSelectionState,
    /// Stable reason for fallback or refusal when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Whether the variant preserved exact trace equality against CPU reference.
    pub trace_digest_equal: bool,
    /// Whether the variant preserved final outputs against CPU reference.
    pub outputs_equal: bool,
    /// Whether the variant preserved halt reason against CPU reference.
    pub halt_equal: bool,
    /// Aggregate exactness score in basis points.
    pub exactness_bps: u32,
    /// Exact trace-step count.
    pub trace_steps: u64,
    /// Reference-linear throughput reused for comparison.
    pub reference_linear_steps_per_second: f64,
    /// CPU-reference throughput reused for comparison.
    pub cpu_reference_steps_per_second: f64,
    /// Variant throughput on the case.
    pub variant_steps_per_second: f64,
    /// Variant speedup over reference-linear execution.
    pub speedup_over_reference_linear: f64,
    /// Remaining CPU-reference gap ratio for the variant.
    pub remaining_gap_vs_cpu_reference: f64,
    /// CPU-reference behavior digest.
    pub cpu_behavior_digest: String,
    /// Variant behavior digest.
    pub variant_behavior_digest: String,
    /// Artifact anchor for the underlying evidence.
    pub artifact_ref: String,
    /// Plain-language note for the case.
    pub note: String,
}

/// Workload-family summary for one geometric fast-path variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarGeometricVariantWorkloadSummary {
    /// Stable variant identifier.
    pub variant_id: String,
    /// Runtime-ready or research-only claim boundary for the variant.
    pub claim_boundary: TassadarGeometricVariantClaimBoundary,
    /// Current top-level Tassadar claim class for the variant.
    pub claim_class: TassadarClaimClass,
    /// Workload target summarized by this row.
    pub workload_target: TassadarWorkloadTarget,
    /// Number of cases that stayed direct on the variant.
    pub direct_case_count: usize,
    /// Number of cases that fell back away from the variant.
    pub fallback_case_count: usize,
    /// Number of cases that were refused by the variant.
    pub refused_case_count: usize,
    /// Number of cases that preserved exact behavior against CPU reference.
    pub exact_case_count: usize,
    /// Average variant throughput across the workload family.
    pub average_steps_per_second: f64,
    /// Average speedup over reference-linear execution.
    pub average_speedup_over_reference_linear: f64,
    /// Average remaining CPU-reference gap ratio.
    pub average_remaining_gap_vs_cpu_reference: f64,
    /// Stable counts for fallback or refusal reasons.
    pub selection_reason_counts: BTreeMap<String, usize>,
    /// Artifact anchor for the summary.
    pub artifact_ref: String,
    /// Plain-language note for the workload summary.
    pub note: String,
}

/// Machine-readable comparison report across the current runtime HullCache and
/// richer geometric research variants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarGeometricVariantReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Ordered artifact anchors used to build the report.
    pub generated_from_artifacts: Vec<String>,
    /// Ordered per-case comparison artifacts.
    pub case_reports: Vec<TassadarGeometricVariantCaseReport>,
    /// Ordered workload-family summaries.
    pub workload_summaries: Vec<TassadarGeometricVariantWorkloadSummary>,
    /// Plain-language boundary for the full report.
    pub claim_boundary: String,
    /// Stable digest over the full report.
    pub report_digest: String,
}

/// Tassadar benchmark build or execution failure.
#[derive(Debug, Error)]
pub enum TassadarBenchmarkError {
    /// Environment bundle build failed.
    #[error(transparent)]
    Environment(#[from] TassadarEnvironmentError),
    /// Eval runtime failed.
    #[error(transparent)]
    EvalRuntime(#[from] EvalRuntimeError),
    /// Program artifact assembly failed.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    /// Runtime execution refused one program.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
    /// Executor descriptor rejected one program artifact.
    #[error(transparent)]
    ExecutorContract(#[from] TassadarExecutorContractError),
    /// Artifact count and validation-corpus count differed.
    #[error("Tassadar artifact count mismatch: expected {expected}, found {actual}")]
    ArtifactCountMismatch { expected: usize, actual: usize },
    /// One artifact targeted a different case than the current corpus ordering.
    #[error(
        "Tassadar artifact `{artifact_id}` does not match case `{case_id}` in the validation corpus"
    )]
    ArtifactCaseMismatch {
        artifact_id: String,
        case_id: String,
    },
}

/// Workload capability matrix build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarWorkloadCapabilityMatrixError {
    /// Underlying filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON parsing or serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// One required workload family was missing from the article benchmark report.
    #[error("missing workload target `{workload_target:?}` in the article-class benchmark report")]
    MissingWorkloadTarget {
        /// Missing workload target.
        workload_target: TassadarWorkloadTarget,
    },
}

/// HullCache closure report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarHullCacheClosureError {
    /// Underlying filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON parsing or serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// SparseTopK comparison report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarSparseTopKComparisonError {
    /// Underlying filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON parsing or serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Decode-scaling report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarDecodeScalingReportError {
    /// Underlying filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON parsing or serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Runtime execution refused one synthetic program directly.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
    /// Runtime execution or benchmarking refused one synthetic program.
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
    /// One decode request unexpectedly resolved without an executable mode.
    #[error(
        "decode-scaling request for program `{program_id}` and mode `{requested_decode_mode:?}` resolved without an effective decode mode"
    )]
    MissingEffectiveDecodeMode {
        /// Program identifier.
        program_id: String,
        /// Requested decode mode.
        requested_decode_mode: TassadarExecutorDecodeMode,
    },
}

/// Geometric-variant comparison report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarGeometricVariantReportError {
    /// Underlying filesystem read/write failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON parsing or serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Runtime execution refused one program directly.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
    /// Benchmark helper or runtime execution failed.
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
    /// One required benchmark case was missing from the article corpus.
    #[error("missing article benchmark case `{case_id}` in the article corpus")]
    MissingCase {
        /// Missing case identifier.
        case_id: String,
    },
}

/// Builds the packaged Phase 3 suite for the current Tassadar validation corpus.
pub fn build_tassadar_reference_fixture_suite(
    version: &str,
) -> Result<TassadarReferenceFixtureSuite, TassadarBenchmarkError> {
    let artifacts = tassadar_program_artifacts(version)?;
    let corpus_digest = stable_corpus_digest(artifacts.as_slice());
    let environment_bundle =
        build_tassadar_environment_bundle(version, &artifacts, &corpus_digest)?;
    let benchmark_package =
        build_tassadar_benchmark_package(version, &environment_bundle, artifacts.as_slice())?;
    Ok(TassadarReferenceFixtureSuite {
        environment_bundle,
        benchmark_package,
        artifacts,
        corpus_digest,
    })
}

/// Builds the widened article-class suite for the current Tassadar executor lane.
pub fn build_tassadar_article_class_suite(
    version: &str,
) -> Result<TassadarReferenceFixtureSuite, TassadarBenchmarkError> {
    let artifacts = tassadar_article_class_program_artifacts(version)?;
    let corpus_digest = stable_corpus_digest(artifacts.as_slice());
    let environment_bundle =
        build_tassadar_article_class_environment_bundle(version, &artifacts, &corpus_digest)?;
    let benchmark_package =
        build_tassadar_article_class_benchmark_package(version, &environment_bundle, &artifacts)?;
    Ok(TassadarReferenceFixtureSuite {
        environment_bundle,
        benchmark_package,
        artifacts,
        corpus_digest,
    })
}

/// Builds the exact 9x9 Sudoku suite for the current search corpus.
pub fn build_tassadar_sudoku_9x9_suite(
    version: &str,
) -> Result<TassadarReferenceFixtureSuite, TassadarBenchmarkError> {
    let artifacts = tassadar_sudoku_9x9_program_artifacts(version)?;
    let corpus_digest = stable_corpus_digest(artifacts.as_slice());
    let environment_bundle =
        build_tassadar_sudoku_9x9_environment_bundle(version, &artifacts, &corpus_digest)?;
    let benchmark_package =
        build_tassadar_sudoku_9x9_benchmark_package(version, &environment_bundle, &artifacts)?;
    Ok(TassadarReferenceFixtureSuite {
        environment_bundle,
        benchmark_package,
        artifacts,
        corpus_digest,
    })
}

/// Builds the bounded Hungarian-v0 suite for the current exact matching corpus.
pub fn build_tassadar_hungarian_v0_suite(
    version: &str,
) -> Result<TassadarReferenceFixtureSuite, TassadarBenchmarkError> {
    let artifacts = tassadar_hungarian_v0_program_artifacts(version)?;
    let corpus_digest = stable_corpus_digest(artifacts.as_slice());
    let environment_bundle =
        build_tassadar_hungarian_v0_environment_bundle(version, &artifacts, &corpus_digest)?;
    let benchmark_package =
        build_tassadar_hungarian_v0_benchmark_package(version, &environment_bundle, &artifacts)?;
    Ok(TassadarReferenceFixtureSuite {
        environment_bundle,
        benchmark_package,
        artifacts,
        corpus_digest,
    })
}

/// Builds the article-sized Hungarian-10x10 suite for the current exact matching corpus.
pub fn build_tassadar_hungarian_10x10_suite(
    version: &str,
) -> Result<TassadarReferenceFixtureSuite, TassadarBenchmarkError> {
    let artifacts = tassadar_hungarian_10x10_program_artifacts(version)?;
    let corpus_digest = stable_corpus_digest(artifacts.as_slice());
    let environment_bundle =
        build_tassadar_hungarian_10x10_environment_bundle(version, &artifacts, &corpus_digest)?;
    let benchmark_package =
        build_tassadar_hungarian_10x10_benchmark_package(version, &environment_bundle, &artifacts)?;
    Ok(TassadarReferenceFixtureSuite {
        environment_bundle,
        benchmark_package,
        artifacts,
        corpus_digest,
    })
}

/// Runs the current packaged Phase 3 suite through the reference-linear executor.
pub fn run_tassadar_reference_fixture_benchmark(
    version: &str,
) -> Result<TassadarBenchmarkReport, TassadarBenchmarkError> {
    let suite = build_tassadar_reference_fixture_suite(version)?;
    let corpus = tassadar_validation_corpus();
    if suite.artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: suite.artifacts.len(),
        });
    }

    let fixture = TassadarExecutorFixture::new();
    let descriptor = fixture.descriptor();
    let model_descriptor_digest = descriptor.stable_digest();
    let runtime_capability = fixture.runtime_capability_report();
    let cpu_runner = TassadarCpuReferenceRunner::new();
    let reference_linear_runner = TassadarFixtureRunner::new();
    let hull_cache_runner = TassadarHullCacheRunner::new();
    let sparse_top_k_runner = TassadarSparseTopKRunner::new();

    let mut eval_run = EvalRunState::open(
        EvalRunContract::new(
            format!("tassadar-benchmark-run-{version}"),
            EvalRunMode::Benchmark,
            suite.environment_bundle.benchmark_package.key.clone(),
        )
        .with_dataset(
            suite.environment_bundle.program_binding.dataset.clone(),
            Some(String::from("benchmark")),
        )
        .with_benchmark_package(suite.benchmark_package.key.clone())
        .with_expected_sample_count(corpus.len() as u64),
    )?;
    eval_run.start(1_000)?;

    let mut case_reports = Vec::new();
    for (ordinal, (case, artifact)) in corpus.into_iter().zip(suite.artifacts.iter()).enumerate() {
        if artifact.validated_program.program_id != case.program.program_id {
            return Err(TassadarBenchmarkError::ArtifactCaseMismatch {
                artifact_id: artifact.artifact_id.clone(),
                case_id: case.case_id,
            });
        }
        descriptor
            .validate_program_artifact(artifact, TassadarExecutorDecodeMode::ReferenceLinear)?;
        descriptor.validate_program_artifact(artifact, TassadarExecutorDecodeMode::HullCache)?;
        descriptor.validate_program_artifact(artifact, TassadarExecutorDecodeMode::SparseTopK)?;
        let selection = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::HullCache);
        let sparse_top_k_selection = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::SparseTopK);
        let effective_decode_mode = selection
            .effective_decode_mode
            .expect("validated benchmark corpus should not be refused");
        let equivalence_started = Instant::now();
        let equivalence_report = run_tassadar_exact_equivalence(&case.program)?;
        let equivalence_elapsed = equivalence_started.elapsed();
        let cpu_execution = &equivalence_report.cpu_reference;
        let reference_execution = &equivalence_report.reference_linear;
        let hull_cache_execution = &equivalence_report.hull_cache;
        let sparse_top_k_execution = &equivalence_report.sparse_top_k;
        let trace_steps = reference_execution.steps.len() as u64;
        let cpu_steps_per_second =
            benchmark_runner_steps_per_second(trace_steps, || cpu_runner.execute(&case.program))?;
        let reference_linear_steps_per_second =
            benchmark_runner_steps_per_second(trace_steps, || {
                reference_linear_runner.execute(&case.program)
            })?;
        let hull_cache_steps_per_second = benchmark_runner_steps_per_second(trace_steps, || {
            hull_cache_runner.execute(&case.program)
        })?;
        let sparse_top_k_steps_per_second = benchmark_runner_steps_per_second(trace_steps, || {
            sparse_top_k_runner.execute(&case.program)
        })?;
        let trace_digest_equal = equivalence_report.trace_digest_equal();
        let outputs_equal = equivalence_report.outputs_equal();
        let halt_equal = equivalence_report.halt_equal();
        let hull_cache_speedup_over_reference_linear =
            hull_cache_steps_per_second / reference_linear_steps_per_second.max(1e-9);
        let hull_cache_remaining_gap_vs_cpu_reference =
            cpu_steps_per_second / hull_cache_steps_per_second.max(1e-9);
        let sparse_top_k_speedup_over_reference_linear =
            sparse_top_k_steps_per_second / reference_linear_steps_per_second.max(1e-9);
        let sparse_top_k_remaining_gap_vs_cpu_reference =
            cpu_steps_per_second / sparse_top_k_steps_per_second.max(1e-9);

        let final_output_exactness_bps =
            u32::from(reference_execution.outputs == case.expected_outputs && outputs_equal)
                * 10_000;
        let step_exactness_bps =
            u32::from(reference_execution.steps == case.expected_trace && trace_digest_equal)
                * 10_000;
        let halt_exactness_bps = u32::from(halt_equal) * 10_000;
        let score_bps = (final_output_exactness_bps + step_exactness_bps + halt_exactness_bps) / 3;
        let status = if score_bps == 10_000 {
            EvalSampleStatus::Passed
        } else {
            EvalSampleStatus::Failed
        };

        let evidence = build_tassadar_execution_evidence_bundle(
            format!("tassadar-case-{}", case.case_id),
            stable_corpus_digest(std::slice::from_ref(artifact)),
            "tassadar_reference_fixture",
            descriptor.model.model_id.clone(),
            model_descriptor_digest.clone(),
            vec![suite.environment_bundle.benchmark_package.storage_key()],
            artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
            &reference_execution,
        );
        let sample_artifacts = build_case_artifacts(
            version,
            &case.case_id,
            artifact,
            reference_execution,
            &case,
            &evidence,
            &selection,
        )?;
        let sample = EvalSampleRecord {
            sample_id: case.case_id.clone(),
            ordinal: Some(ordinal as u64),
            environment: suite.environment_bundle.benchmark_package.key.clone(),
            status,
            input_ref: Some(format!("tassadar://input/{}/none", case.case_id)),
            output_ref: Some(format!(
                "tassadar://output/{}/reference_linear",
                case.case_id
            )),
            expected_output_ref: Some(format!("tassadar://expected_output/{}", case.case_id)),
            score_bps: Some(score_bps),
            metrics: vec![
                EvalMetric::new(
                    TASSADAR_OUTPUT_EXACTNESS_METRIC_ID,
                    f64::from(final_output_exactness_bps),
                )
                .with_unit("bps"),
                EvalMetric::new(
                    TASSADAR_STEP_EXACTNESS_METRIC_ID,
                    f64::from(step_exactness_bps),
                )
                .with_unit("bps"),
                EvalMetric::new(
                    TASSADAR_HALT_EXACTNESS_METRIC_ID,
                    f64::from(halt_exactness_bps),
                )
                .with_unit("bps"),
                EvalMetric::new(
                    TASSADAR_TRACE_DIGEST_EQUAL_METRIC_ID,
                    f64::from(u32::from(trace_digest_equal) * 10_000),
                )
                .with_unit("bps"),
                EvalMetric::new(TASSADAR_CPU_BASELINE_METRIC_ID, cpu_steps_per_second)
                    .with_unit("steps_per_second"),
                EvalMetric::new(
                    TASSADAR_REFERENCE_LINEAR_METRIC_ID,
                    reference_linear_steps_per_second,
                )
                .with_unit("steps_per_second"),
                EvalMetric::new(TASSADAR_HULL_CACHE_METRIC_ID, hull_cache_steps_per_second)
                    .with_unit("steps_per_second"),
                EvalMetric::new(
                    TASSADAR_HULL_CACHE_SPEEDUP_METRIC_ID,
                    hull_cache_speedup_over_reference_linear,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_HULL_CACHE_CPU_GAP_METRIC_ID,
                    hull_cache_remaining_gap_vs_cpu_reference,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_SPARSE_TOP_K_METRIC_ID,
                    sparse_top_k_steps_per_second,
                )
                .with_unit("steps_per_second"),
                EvalMetric::new(
                    TASSADAR_SPARSE_TOP_K_SPEEDUP_METRIC_ID,
                    sparse_top_k_speedup_over_reference_linear,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_SPARSE_TOP_K_CPU_GAP_METRIC_ID,
                    sparse_top_k_remaining_gap_vs_cpu_reference,
                )
                .with_unit("ratio"),
                EvalMetric::new(TASSADAR_TRACE_STEP_COUNT_METRIC_ID, trace_steps as f64)
                    .with_unit("steps"),
            ],
            artifacts: sample_artifacts.clone(),
            error_reason: None,
            verification: Some(EvalVerificationFacts {
                timer_integrity: Some(EvalTimerIntegrityFacts {
                    declared_budget_ms: Some(
                        suite
                            .environment_bundle
                            .exactness_contract
                            .timeout_budget_ms,
                    ),
                    elapsed_ms: equivalence_elapsed.as_millis() as u64,
                    within_budget: equivalence_elapsed.as_millis() as u64
                        <= suite
                            .environment_bundle
                            .exactness_contract
                            .timeout_budget_ms,
                }),
                token_accounting: None,
                final_state: Some(EvalFinalStateCapture {
                    session_digest: reference_execution.behavior_digest(),
                    output_digest: Some(stable_outputs_digest(&reference_execution.outputs)),
                    artifact_digests: sample_artifacts
                        .iter()
                        .map(|artifact| artifact.artifact_digest.clone())
                        .collect(),
                }),
                execution_strategy: Some(EvalExecutionStrategyFacts {
                    strategy_label: String::from("tassadar_exact_equivalence_triplicate"),
                    runtime_family: Some(String::from("tassadar_executor")),
                    scheduler_posture: Some(String::from(
                        "cpu_reference+reference_linear+hull_cache",
                    )),
                }),
            }),
            session_digest: Some(reference_execution.behavior_digest()),
            metadata: std::collections::BTreeMap::from([
                (
                    String::from("workload_target"),
                    serde_json::to_value(classify_case(&case.case_id)).unwrap_or(Value::Null),
                ),
                (
                    String::from("cpu_behavior_digest"),
                    Value::String(cpu_execution.behavior_digest()),
                ),
                (
                    String::from("reference_linear_behavior_digest"),
                    Value::String(reference_execution.behavior_digest()),
                ),
                (
                    String::from("hull_cache_behavior_digest"),
                    Value::String(hull_cache_execution.behavior_digest()),
                ),
                (
                    String::from("sparse_top_k_behavior_digest"),
                    Value::String(sparse_top_k_execution.behavior_digest()),
                ),
                (
                    String::from("trace_digest_equal"),
                    Value::Bool(trace_digest_equal),
                ),
                (String::from("outputs_equal"), Value::Bool(outputs_equal)),
                (String::from("halt_equal"), Value::Bool(halt_equal)),
                (
                    String::from("hull_cache_speedup_over_reference_linear"),
                    serde_json::to_value(hull_cache_speedup_over_reference_linear)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("hull_cache_remaining_gap_vs_cpu_reference"),
                    serde_json::to_value(hull_cache_remaining_gap_vs_cpu_reference)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_speedup_over_reference_linear"),
                    serde_json::to_value(sparse_top_k_speedup_over_reference_linear)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_remaining_gap_vs_cpu_reference"),
                    serde_json::to_value(sparse_top_k_remaining_gap_vs_cpu_reference)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("selection_state"),
                    serde_json::to_value(selection.selection_state).unwrap_or(Value::Null),
                ),
                (
                    String::from("selection_reason"),
                    serde_json::to_value(selection.selection_reason).unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_selection_state"),
                    serde_json::to_value(sparse_top_k_selection.selection_state)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_selection_reason"),
                    serde_json::to_value(sparse_top_k_selection.selection_reason)
                        .unwrap_or(Value::Null),
                ),
            ]),
        };
        eval_run.append_sample(sample)?;
        case_reports.push(TassadarBenchmarkCaseReport {
            case_id: case.case_id,
            workload_target: classify_case(&artifact.validated_program.program_id),
            status,
            score_bps,
            final_output_exactness_bps,
            step_exactness_bps,
            halt_exactness_bps,
            requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
            effective_decode_mode,
            selection_state: selection.selection_state,
            selection_reason: selection.selection_reason,
            used_decode_fallback: selection.is_fallback(),
            trace_digest_equal,
            outputs_equal,
            halt_equal,
            trace_steps,
            cpu_reference_steps_per_second: cpu_steps_per_second,
            reference_linear_steps_per_second,
            hull_cache_steps_per_second,
            hull_cache_speedup_over_reference_linear,
            hull_cache_remaining_gap_vs_cpu_reference,
            sparse_top_k_selection_state: sparse_top_k_selection.selection_state,
            sparse_top_k_selection_reason: sparse_top_k_selection.selection_reason,
            sparse_top_k_used_decode_fallback: sparse_top_k_selection.is_fallback(),
            sparse_top_k_steps_per_second,
            sparse_top_k_speedup_over_reference_linear,
            sparse_top_k_remaining_gap_vs_cpu_reference,
            cpu_behavior_digest: cpu_execution.behavior_digest(),
            reference_linear_behavior_digest: reference_execution.behavior_digest(),
            hull_cache_behavior_digest: hull_cache_execution.behavior_digest(),
            sparse_top_k_behavior_digest: sparse_top_k_execution.behavior_digest(),
        });
    }

    let run_artifacts = vec![
        EvalArtifact::new(
            "tassadar_benchmark_package.json",
            format!("artifact://tassadar/{version}/benchmark_package"),
            &serde_json::to_vec(&suite.benchmark_package).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_environment_bundle.json",
            format!("artifact://tassadar/{version}/environment_bundle"),
            &serde_json::to_vec(&suite.environment_bundle).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_runtime_capability.json",
            format!("artifact://tassadar/{version}/runtime_capability"),
            &serde_json::to_vec(&runtime_capability).unwrap_or_default(),
        ),
    ];
    eval_run.finalize(2_000, run_artifacts)?;

    let mut execution = suite
        .benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    execution.record_round(&eval_run)?;
    let aggregate_summary = execution.finalize()?;

    Ok(TassadarBenchmarkReport {
        suite,
        eval_run,
        aggregate_summary,
        case_reports,
    })
}

/// Runs the widened article-class suite through the exact executor harness.
pub fn run_tassadar_article_class_benchmark(
    version: &str,
) -> Result<TassadarBenchmarkReport, TassadarBenchmarkError> {
    let suite = build_tassadar_article_class_suite(version)?;
    let corpus = tassadar_article_class_corpus();
    if suite.artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: suite.artifacts.len(),
        });
    }

    let fixture = TassadarExecutorFixture::article_i32_compute_v1();
    let descriptor = fixture.descriptor();
    let model_descriptor_digest = descriptor.stable_digest();
    let runtime_capability = fixture.runtime_capability_report();

    let mut eval_run = EvalRunState::open(
        EvalRunContract::new(
            format!("tassadar-article-class-benchmark-run-{version}"),
            EvalRunMode::Benchmark,
            suite.environment_bundle.benchmark_package.key.clone(),
        )
        .with_dataset(
            suite.environment_bundle.program_binding.dataset.clone(),
            Some(String::from("benchmark")),
        )
        .with_benchmark_package(suite.benchmark_package.key.clone())
        .with_expected_sample_count(corpus.len() as u64),
    )?;
    eval_run.start(3_000)?;

    let mut case_reports = Vec::new();
    for (ordinal, (case, artifact)) in corpus.into_iter().zip(suite.artifacts.iter()).enumerate() {
        if artifact.validated_program.program_id != case.program.program_id {
            return Err(TassadarBenchmarkError::ArtifactCaseMismatch {
                artifact_id: artifact.artifact_id.clone(),
                case_id: case.case_id,
            });
        }
        descriptor
            .validate_program_artifact(artifact, TassadarExecutorDecodeMode::ReferenceLinear)?;
        descriptor.validate_program_artifact(artifact, TassadarExecutorDecodeMode::HullCache)?;
        descriptor.validate_program_artifact(artifact, TassadarExecutorDecodeMode::SparseTopK)?;
        let selection = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::HullCache);
        let sparse_top_k_selection = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::SparseTopK);
        let effective_decode_mode = selection
            .effective_decode_mode
            .expect("validated article-class corpus should not be refused");

        let cpu_runner = TassadarCpuReferenceRunner::for_program(&case.program)?;
        let reference_linear_runner = TassadarFixtureRunner::for_program(&case.program)?;
        let hull_cache_runner = TassadarHullCacheRunner::for_program(&case.program)?;
        let sparse_top_k_runner = TassadarSparseTopKRunner::for_program(&case.program)?;
        let equivalence_started = Instant::now();
        let equivalence_report = run_tassadar_exact_equivalence(&case.program)?;
        let equivalence_elapsed = equivalence_started.elapsed();
        let cpu_execution = &equivalence_report.cpu_reference;
        let reference_execution = &equivalence_report.reference_linear;
        let hull_cache_execution = &equivalence_report.hull_cache;
        let sparse_top_k_execution = &equivalence_report.sparse_top_k;
        let trace_steps = reference_execution.steps.len() as u64;
        let cpu_steps_per_second =
            benchmark_runner_steps_per_second(trace_steps, || cpu_runner.execute(&case.program))?;
        let reference_linear_steps_per_second =
            benchmark_runner_steps_per_second(trace_steps, || {
                reference_linear_runner.execute(&case.program)
            })?;
        let hull_cache_steps_per_second = match effective_decode_mode {
            TassadarExecutorDecodeMode::ReferenceLinear => {
                benchmark_runner_steps_per_second(trace_steps, || {
                    reference_linear_runner.execute(&case.program)
                })?
            }
            TassadarExecutorDecodeMode::HullCache => {
                benchmark_runner_steps_per_second(trace_steps, || {
                    hull_cache_runner.execute(&case.program)
                })?
            }
            TassadarExecutorDecodeMode::SparseTopK => {
                benchmark_runner_steps_per_second(trace_steps, || {
                    sparse_top_k_runner.execute(&case.program)
                })?
            }
        };
        let sparse_top_k_steps_per_second = match sparse_top_k_selection
            .effective_decode_mode
            .expect("validated article-class corpus should not be refused")
        {
            TassadarExecutorDecodeMode::ReferenceLinear => {
                benchmark_runner_steps_per_second(trace_steps, || {
                    reference_linear_runner.execute(&case.program)
                })?
            }
            TassadarExecutorDecodeMode::HullCache => {
                benchmark_runner_steps_per_second(trace_steps, || {
                    hull_cache_runner.execute(&case.program)
                })?
            }
            TassadarExecutorDecodeMode::SparseTopK => {
                benchmark_runner_steps_per_second(trace_steps, || {
                    sparse_top_k_runner.execute(&case.program)
                })?
            }
        };
        let trace_digest_equal = equivalence_report.trace_digest_equal();
        let outputs_equal = equivalence_report.outputs_equal();
        let halt_equal = equivalence_report.halt_equal();
        let hull_cache_speedup_over_reference_linear =
            hull_cache_steps_per_second / reference_linear_steps_per_second.max(1e-9);
        let hull_cache_remaining_gap_vs_cpu_reference =
            cpu_steps_per_second / hull_cache_steps_per_second.max(1e-9);
        let sparse_top_k_speedup_over_reference_linear =
            sparse_top_k_steps_per_second / reference_linear_steps_per_second.max(1e-9);
        let sparse_top_k_remaining_gap_vs_cpu_reference =
            cpu_steps_per_second / sparse_top_k_steps_per_second.max(1e-9);

        let final_output_exactness_bps =
            u32::from(reference_execution.outputs == case.expected_outputs && outputs_equal)
                * 10_000;
        let step_exactness_bps =
            u32::from(reference_execution.steps == case.expected_trace && trace_digest_equal)
                * 10_000;
        let halt_exactness_bps = u32::from(halt_equal) * 10_000;
        let score_bps = (final_output_exactness_bps + step_exactness_bps + halt_exactness_bps) / 3;
        let status = if score_bps == 10_000 {
            EvalSampleStatus::Passed
        } else {
            EvalSampleStatus::Failed
        };

        let evidence = build_tassadar_execution_evidence_bundle(
            format!("tassadar-article-class-case-{}", case.case_id),
            stable_corpus_digest(std::slice::from_ref(artifact)),
            "tassadar_article_class_fixture",
            descriptor.model.model_id.clone(),
            model_descriptor_digest.clone(),
            vec![suite.environment_bundle.benchmark_package.storage_key()],
            artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
            reference_execution,
        );
        let sample_artifacts = build_case_artifacts(
            version,
            &case.case_id,
            artifact,
            reference_execution,
            &case,
            &evidence,
            &selection,
        )?;
        let sample = EvalSampleRecord {
            sample_id: case.case_id.clone(),
            ordinal: Some(ordinal as u64),
            environment: suite.environment_bundle.benchmark_package.key.clone(),
            status,
            input_ref: Some(format!("tassadar://input/{}/none", case.case_id)),
            output_ref: Some(format!(
                "tassadar://output/{}/reference_linear",
                case.case_id
            )),
            expected_output_ref: Some(format!("tassadar://expected_output/{}", case.case_id)),
            score_bps: Some(score_bps),
            metrics: vec![
                EvalMetric::new(
                    TASSADAR_OUTPUT_EXACTNESS_METRIC_ID,
                    f64::from(final_output_exactness_bps),
                )
                .with_unit("bps"),
                EvalMetric::new(
                    TASSADAR_STEP_EXACTNESS_METRIC_ID,
                    f64::from(step_exactness_bps),
                )
                .with_unit("bps"),
                EvalMetric::new(
                    TASSADAR_HALT_EXACTNESS_METRIC_ID,
                    f64::from(halt_exactness_bps),
                )
                .with_unit("bps"),
                EvalMetric::new(TASSADAR_CPU_BASELINE_METRIC_ID, cpu_steps_per_second)
                    .with_unit("steps_per_second"),
                EvalMetric::new(
                    TASSADAR_REFERENCE_LINEAR_METRIC_ID,
                    reference_linear_steps_per_second,
                )
                .with_unit("steps_per_second"),
                EvalMetric::new(TASSADAR_HULL_CACHE_METRIC_ID, hull_cache_steps_per_second)
                    .with_unit("steps_per_second"),
                EvalMetric::new(
                    TASSADAR_HULL_CACHE_SPEEDUP_METRIC_ID,
                    hull_cache_speedup_over_reference_linear,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_HULL_CACHE_CPU_GAP_METRIC_ID,
                    hull_cache_remaining_gap_vs_cpu_reference,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_SPARSE_TOP_K_METRIC_ID,
                    sparse_top_k_steps_per_second,
                )
                .with_unit("steps_per_second"),
                EvalMetric::new(
                    TASSADAR_SPARSE_TOP_K_SPEEDUP_METRIC_ID,
                    sparse_top_k_speedup_over_reference_linear,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_SPARSE_TOP_K_CPU_GAP_METRIC_ID,
                    sparse_top_k_remaining_gap_vs_cpu_reference,
                )
                .with_unit("ratio"),
                EvalMetric::new(
                    TASSADAR_TRACE_DIGEST_EQUAL_METRIC_ID,
                    f64::from(u32::from(trace_digest_equal) * 10_000),
                )
                .with_unit("bps"),
                EvalMetric::new(TASSADAR_TRACE_STEP_COUNT_METRIC_ID, trace_steps as f64)
                    .with_unit("steps"),
            ],
            artifacts: sample_artifacts.clone(),
            error_reason: None,
            verification: Some(EvalVerificationFacts {
                timer_integrity: Some(EvalTimerIntegrityFacts {
                    declared_budget_ms: Some(
                        suite
                            .environment_bundle
                            .exactness_contract
                            .timeout_budget_ms,
                    ),
                    elapsed_ms: equivalence_elapsed.as_millis() as u64,
                    within_budget: equivalence_elapsed.as_millis() as u64
                        <= suite
                            .environment_bundle
                            .exactness_contract
                            .timeout_budget_ms,
                }),
                token_accounting: None,
                final_state: Some(EvalFinalStateCapture {
                    session_digest: reference_execution.behavior_digest(),
                    output_digest: Some(stable_outputs_digest(&reference_execution.outputs)),
                    artifact_digests: sample_artifacts
                        .iter()
                        .map(|artifact| artifact.artifact_digest.clone())
                        .collect(),
                }),
                execution_strategy: Some(EvalExecutionStrategyFacts {
                    strategy_label: String::from(
                        "tassadar_article_class_exact_equivalence_triplicate",
                    ),
                    runtime_family: Some(String::from("tassadar_executor")),
                    scheduler_posture: Some(String::from(
                        "cpu_reference+reference_linear+hull_cache",
                    )),
                }),
            }),
            session_digest: Some(reference_execution.behavior_digest()),
            metadata: std::collections::BTreeMap::from([
                (
                    String::from("workload_target"),
                    serde_json::to_value(classify_case(&case.case_id)).unwrap_or(Value::Null),
                ),
                (
                    String::from("cpu_behavior_digest"),
                    Value::String(cpu_execution.behavior_digest()),
                ),
                (
                    String::from("reference_linear_behavior_digest"),
                    Value::String(reference_execution.behavior_digest()),
                ),
                (
                    String::from("hull_cache_behavior_digest"),
                    Value::String(hull_cache_execution.behavior_digest()),
                ),
                (
                    String::from("sparse_top_k_behavior_digest"),
                    Value::String(sparse_top_k_execution.behavior_digest()),
                ),
                (
                    String::from("trace_digest_equal"),
                    Value::Bool(trace_digest_equal),
                ),
                (String::from("outputs_equal"), Value::Bool(outputs_equal)),
                (String::from("halt_equal"), Value::Bool(halt_equal)),
                (
                    String::from("hull_cache_speedup_over_reference_linear"),
                    serde_json::to_value(hull_cache_speedup_over_reference_linear)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("hull_cache_remaining_gap_vs_cpu_reference"),
                    serde_json::to_value(hull_cache_remaining_gap_vs_cpu_reference)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_speedup_over_reference_linear"),
                    serde_json::to_value(sparse_top_k_speedup_over_reference_linear)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_remaining_gap_vs_cpu_reference"),
                    serde_json::to_value(sparse_top_k_remaining_gap_vs_cpu_reference)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("selection_state"),
                    serde_json::to_value(selection.selection_state).unwrap_or(Value::Null),
                ),
                (
                    String::from("selection_reason"),
                    serde_json::to_value(selection.selection_reason).unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_selection_state"),
                    serde_json::to_value(sparse_top_k_selection.selection_state)
                        .unwrap_or(Value::Null),
                ),
                (
                    String::from("sparse_top_k_selection_reason"),
                    serde_json::to_value(sparse_top_k_selection.selection_reason)
                        .unwrap_or(Value::Null),
                ),
            ]),
        };
        eval_run.append_sample(sample)?;

        case_reports.push(TassadarBenchmarkCaseReport {
            case_id: case.case_id,
            workload_target: classify_case(&artifact.validated_program.program_id),
            status,
            score_bps,
            final_output_exactness_bps,
            step_exactness_bps,
            halt_exactness_bps,
            requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
            effective_decode_mode,
            selection_state: selection.selection_state,
            selection_reason: selection.selection_reason,
            used_decode_fallback: selection.selection_state
                == TassadarExecutorSelectionState::Fallback,
            trace_digest_equal,
            outputs_equal,
            halt_equal,
            trace_steps,
            cpu_reference_steps_per_second: cpu_steps_per_second,
            reference_linear_steps_per_second,
            hull_cache_steps_per_second,
            hull_cache_speedup_over_reference_linear,
            hull_cache_remaining_gap_vs_cpu_reference,
            sparse_top_k_selection_state: sparse_top_k_selection.selection_state,
            sparse_top_k_selection_reason: sparse_top_k_selection.selection_reason,
            sparse_top_k_used_decode_fallback: sparse_top_k_selection.selection_state
                == TassadarExecutorSelectionState::Fallback,
            sparse_top_k_steps_per_second,
            sparse_top_k_speedup_over_reference_linear,
            sparse_top_k_remaining_gap_vs_cpu_reference,
            cpu_behavior_digest: cpu_execution.behavior_digest(),
            reference_linear_behavior_digest: reference_execution.behavior_digest(),
            hull_cache_behavior_digest: hull_cache_execution.behavior_digest(),
            sparse_top_k_behavior_digest: sparse_top_k_execution.behavior_digest(),
        });
    }

    let run_artifacts = vec![
        EvalArtifact::new(
            "tassadar_article_class_benchmark_package.json",
            format!("artifact://tassadar/{version}/article_class_benchmark_package"),
            &serde_json::to_vec(&suite.benchmark_package).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_article_class_environment_bundle.json",
            format!("artifact://tassadar/{version}/article_class_environment_bundle"),
            &serde_json::to_vec(&suite.environment_bundle).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_runtime_capability.json",
            format!("artifact://tassadar/{version}/runtime_capability"),
            &serde_json::to_vec(&runtime_capability).unwrap_or_default(),
        ),
    ];
    eval_run.finalize(4_000, run_artifacts)?;

    let mut execution = suite
        .benchmark_package
        .clone()
        .open_execution(BenchmarkExecutionMode::OperatorSimulation)?;
    execution.record_round(&eval_run)?;
    let aggregate_summary = execution.finalize()?;

    Ok(TassadarBenchmarkReport {
        suite,
        eval_run,
        aggregate_summary,
        case_reports,
    })
}

/// Builds digest-bound fixture artifacts for the current validation corpus.
pub fn tassadar_program_artifacts(
    version: &str,
) -> Result<Vec<TassadarProgramArtifact>, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::core_i32_v1();
    let trace_abi = TassadarTraceAbi::core_i32_v1();
    tassadar_validation_corpus()
        .into_iter()
        .map(|case| {
            TassadarProgramArtifact::fixture_reference(
                format!("tassadar://artifact/{version}/{}", case.case_id),
                &profile,
                &trace_abi,
                case.program,
            )
            .map_err(TassadarBenchmarkError::from)
        })
        .collect()
}

fn build_tassadar_environment_bundle(
    version: &str,
    artifacts: &[TassadarProgramArtifact],
    corpus_digest: &str,
) -> Result<TassadarEnvironmentBundle, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::core_i32_v1();
    let trace_abi = TassadarTraceAbi::core_i32_v1();
    let dataset = DatasetKey::new(TASSADAR_VALIDATION_CORPUS_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Validation Corpus"),
        eval_environment_ref: String::from(TASSADAR_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(TASSADAR_BENCHMARK_ENVIRONMENT_REF),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/benchmark"),
            required: true,
        },
        package_refs: TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.validation"),
            eval_pin_alias: String::from("tassadar_eval"),
            benchmark_pin_alias: String::from("tassadar_benchmark"),
            eval_member_ref: String::from("tassadar_eval_member"),
            benchmark_member_ref: String::from("tassadar_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/phase1.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/eval"),
            benchmark_profile_ref: String::from("benchmark://tassadar/reference_fixture"),
            benchmark_runtime_profile_ref: String::from("runtime://tassadar/benchmark"),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/phase1.validation"),
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
            timeout_budget_ms: 5_000,
            trace_budget_steps: 128,
            require_cpu_reference_baseline: true,
            require_reference_linear_baseline: true,
            future_throughput_metric_ids: vec![String::from(TASSADAR_HULL_CACHE_METRIC_ID)],
        },
        benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
            package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
            package_set_version: String::from(version),
            supported_families: vec![
                TassadarBenchmarkFamily::Arithmetic,
                TassadarBenchmarkFamily::ClrsSubset,
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
        spill_tape_store_binding: psionic_environments::default_tassadar_spill_tape_store_binding(),
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
            policy_ref: String::from("policy://tassadar/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from("policy://tassadar/benchmark/verification"),
                required: true,
            },
        ],
        current_workload_targets: vec![
            TassadarWorkloadTarget::ArithmeticMicroprogram,
            TassadarWorkloadTarget::ClrsShortestPath,
            TassadarWorkloadTarget::MemoryLookupMicroprogram,
            TassadarWorkloadTarget::BranchControlFlowMicroprogram,
        ],
        planned_workload_targets: vec![
            TassadarWorkloadTarget::MicroWasmKernel,
            TassadarWorkloadTarget::SudokuClass,
            TassadarWorkloadTarget::HungarianMatching,
        ],
    }
    .build_bundle()
    .map_err(TassadarBenchmarkError::from)
}

fn build_tassadar_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    artifacts: &[TassadarProgramArtifact],
) -> Result<BenchmarkPackage, TassadarBenchmarkError> {
    let corpus = tassadar_validation_corpus();
    if artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: artifacts.len(),
        });
    }

    let cases = corpus
        .into_iter()
        .zip(artifacts.iter())
        .enumerate()
        .map(|(ordinal, (case, artifact))| {
            let mut benchmark_case = BenchmarkCase::new(case.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", case.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", case.case_id));
            benchmark_case.metadata = json!({
                "summary": case.summary,
                "workload_target": classify_case(&case.case_id),
                "artifact_id": artifact.artifact_id,
                "artifact_digest": artifact.artifact_digest,
                "program_digest": artifact.validated_program_digest,
                "expected_outputs": case.expected_outputs,
                "expected_trace_steps": case.expected_trace.len(),
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF, version),
        "Tassadar Validation Corpus Benchmark",
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
        serde_json::to_value(&environment_bundle.current_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.planned_workload_targets"),
        serde_json::to_value(&environment_bundle.planned_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.cpu_baseline_metric_id"),
        Value::String(String::from(TASSADAR_CPU_BASELINE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.reference_linear_metric_id"),
        Value::String(String::from(TASSADAR_REFERENCE_LINEAR_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.hull_cache_metric_id"),
        Value::String(String::from(TASSADAR_HULL_CACHE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.sparse_top_k_metric_id"),
        Value::String(String::from(TASSADAR_SPARSE_TOP_K_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.validate()?;
    Ok(package)
}

/// Builds digest-bound fixture artifacts for the widened article-class corpus.
pub fn tassadar_article_class_program_artifacts(
    version: &str,
) -> Result<Vec<TassadarProgramArtifact>, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    tassadar_article_class_corpus()
        .into_iter()
        .map(|case| {
            TassadarProgramArtifact::fixture_reference(
                format!(
                    "tassadar://artifact/{version}/article_class/{}",
                    case.case_id
                ),
                &profile,
                &trace_abi,
                case.program,
            )
            .map_err(TassadarBenchmarkError::from)
        })
        .collect()
}

fn build_tassadar_article_class_environment_bundle(
    version: &str,
    artifacts: &[TassadarProgramArtifact],
    corpus_digest: &str,
) -> Result<TassadarEnvironmentBundle, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let dataset = DatasetKey::new(TASSADAR_ARTICLE_CLASS_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Article-Class Corpus"),
        eval_environment_ref: String::from(TASSADAR_ARTICLE_CLASS_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/article_class/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/article_class/benchmark"),
            required: true,
        },
        package_refs: TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.article_class"),
            eval_pin_alias: String::from("tassadar_article_class_eval"),
            benchmark_pin_alias: String::from("tassadar_article_class_benchmark"),
            eval_member_ref: String::from("tassadar_article_class_eval_member"),
            benchmark_member_ref: String::from("tassadar_article_class_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/article_class.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/article_class/eval"),
            benchmark_profile_ref: String::from(
                "benchmark://tassadar/article_class/reference_fixture",
            ),
            benchmark_runtime_profile_ref: String::from(
                "runtime://tassadar/article_class/benchmark",
            ),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/article_class.validation"),
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
            timeout_budget_ms: 15_000,
            trace_budget_steps: 512,
            require_cpu_reference_baseline: true,
            require_reference_linear_baseline: true,
            future_throughput_metric_ids: vec![String::from(TASSADAR_HULL_CACHE_METRIC_ID)],
        },
        benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
            package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
            package_set_version: String::from(version),
            supported_families: vec![
                TassadarBenchmarkFamily::Sudoku,
                TassadarBenchmarkFamily::Hungarian,
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
        spill_tape_store_binding: psionic_environments::default_tassadar_spill_tape_store_binding(),
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
            policy_ref: String::from("policy://tassadar/article_class/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/article_class/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from("policy://tassadar/article_class/benchmark/verification"),
                required: true,
            },
        ],
        current_workload_targets: vec![
            TassadarWorkloadTarget::MicroWasmKernel,
            TassadarWorkloadTarget::BranchHeavyKernel,
            TassadarWorkloadTarget::MemoryHeavyKernel,
            TassadarWorkloadTarget::LongLoopKernel,
            TassadarWorkloadTarget::SudokuClass,
            TassadarWorkloadTarget::HungarianMatching,
        ],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
    .map_err(TassadarBenchmarkError::from)
}

fn build_tassadar_article_class_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    artifacts: &[TassadarProgramArtifact],
) -> Result<BenchmarkPackage, TassadarBenchmarkError> {
    let corpus = tassadar_article_class_corpus();
    let sudoku_v0_corpus = tassadar_sudoku_v0_corpus();
    let sudoku_v0_metadata = sudoku_v0_case_metadata_map(sudoku_v0_corpus.as_slice());
    if artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: artifacts.len(),
        });
    }

    let cases = corpus
        .into_iter()
        .zip(artifacts.iter())
        .enumerate()
        .map(|(ordinal, (case, artifact))| {
            let mut benchmark_case = BenchmarkCase::new(case.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", case.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", case.case_id));
            let sudoku_v0_case = sudoku_v0_metadata.get(case.case_id.as_str());
            benchmark_case.metadata = json!({
                "summary": case.summary,
                "workload_target": classify_case(&case.case_id),
                "artifact_id": artifact.artifact_id,
                "artifact_digest": artifact.artifact_digest,
                "program_digest": artifact.validated_program_digest,
                "expected_outputs": case.expected_outputs,
                "expected_trace_steps": case.expected_trace.len(),
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms,
                "sudoku_v0_split": sudoku_v0_case.map(|case| case.split.as_str()),
                "sudoku_v0_given_count": sudoku_v0_case.map(|case| case.given_count),
                "sudoku_v0_puzzle_cells": sudoku_v0_case.map(|case| case.puzzle_cells.clone()),
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_ARTICLE_CLASS_BENCHMARK_REF, version),
        "Tassadar Article-Class Benchmark",
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
        serde_json::to_value(&environment_bundle.current_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.planned_workload_targets"),
        serde_json::to_value(&environment_bundle.planned_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.cpu_baseline_metric_id"),
        Value::String(String::from(TASSADAR_CPU_BASELINE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.reference_linear_metric_id"),
        Value::String(String::from(TASSADAR_REFERENCE_LINEAR_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.hull_cache_metric_id"),
        Value::String(String::from(TASSADAR_HULL_CACHE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.sparse_top_k_metric_id"),
        Value::String(String::from(TASSADAR_SPARSE_TOP_K_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.metadata.insert(
        String::from("tassadar.sudoku_v0_train_case_ids"),
        serde_json::to_value(sudoku_v0_case_ids_for_split(
            sudoku_v0_corpus.as_slice(),
            TassadarSudokuV0CorpusSplit::Train,
        ))
        .unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.sudoku_v0_validation_case_ids"),
        serde_json::to_value(sudoku_v0_case_ids_for_split(
            sudoku_v0_corpus.as_slice(),
            TassadarSudokuV0CorpusSplit::Validation,
        ))
        .unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.sudoku_v0_test_case_ids"),
        serde_json::to_value(sudoku_v0_case_ids_for_split(
            sudoku_v0_corpus.as_slice(),
            TassadarSudokuV0CorpusSplit::Test,
        ))
        .unwrap_or(Value::Null),
    );
    package.validate()?;
    Ok(package)
}

/// Builds digest-bound fixture artifacts for the exact 9x9 Sudoku corpus.
pub fn tassadar_sudoku_9x9_program_artifacts(
    version: &str,
) -> Result<Vec<TassadarProgramArtifact>, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::sudoku_9x9_search_v1();
    let trace_abi = TassadarTraceAbi::sudoku_9x9_search_v1();
    tassadar_sudoku_9x9_corpus()
        .into_iter()
        .map(|case| {
            TassadarProgramArtifact::fixture_reference(
                format!("tassadar://artifact/{version}/sudoku_9x9/{}", case.case_id),
                &profile,
                &trace_abi,
                case.validation_case.program,
            )
            .map_err(TassadarBenchmarkError::from)
        })
        .collect()
}

fn build_tassadar_sudoku_9x9_environment_bundle(
    version: &str,
    artifacts: &[TassadarProgramArtifact],
    corpus_digest: &str,
) -> Result<TassadarEnvironmentBundle, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::sudoku_9x9_search_v1();
    let trace_abi = TassadarTraceAbi::sudoku_9x9_search_v1();
    let dataset = DatasetKey::new(TASSADAR_SUDOKU_9X9_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Sudoku-9x9 Corpus"),
        eval_environment_ref: String::from(TASSADAR_SUDOKU_9X9_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(TASSADAR_SUDOKU_9X9_BENCHMARK_ENVIRONMENT_REF),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/sudoku_9x9/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/sudoku_9x9/benchmark"),
            required: true,
        },
        package_refs: TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.sudoku_9x9"),
            eval_pin_alias: String::from("tassadar_sudoku_9x9_eval"),
            benchmark_pin_alias: String::from("tassadar_sudoku_9x9_benchmark"),
            eval_member_ref: String::from("tassadar_sudoku_9x9_eval_member"),
            benchmark_member_ref: String::from("tassadar_sudoku_9x9_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/sudoku_9x9.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/sudoku_9x9/eval"),
            benchmark_profile_ref: String::from(
                "benchmark://tassadar/sudoku_9x9/reference_fixture",
            ),
            benchmark_runtime_profile_ref: String::from("runtime://tassadar/sudoku_9x9/benchmark"),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/sudoku_9x9.validation"),
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
            timeout_budget_ms: 120_000,
            trace_budget_steps: 1_048_576,
            require_cpu_reference_baseline: true,
            require_reference_linear_baseline: true,
            future_throughput_metric_ids: vec![String::from(TASSADAR_HULL_CACHE_METRIC_ID)],
        },
        benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
            package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
            package_set_version: String::from(version),
            supported_families: vec![TassadarBenchmarkFamily::Sudoku],
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
        spill_tape_store_binding: psionic_environments::default_tassadar_spill_tape_store_binding(),
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
            policy_ref: String::from("policy://tassadar/sudoku_9x9/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/sudoku_9x9/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from("policy://tassadar/sudoku_9x9/benchmark/verification"),
                required: true,
            },
        ],
        current_workload_targets: vec![TassadarWorkloadTarget::SudokuClass],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
    .map_err(TassadarBenchmarkError::from)
}

fn build_tassadar_sudoku_9x9_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    artifacts: &[TassadarProgramArtifact],
) -> Result<BenchmarkPackage, TassadarBenchmarkError> {
    let corpus = tassadar_sudoku_9x9_corpus();
    let train_case_ids =
        sudoku_9x9_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Train);
    let validation_case_ids =
        sudoku_9x9_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Validation);
    let test_case_ids =
        sudoku_9x9_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Test);
    if artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: artifacts.len(),
        });
    }

    let cases = corpus
        .into_iter()
        .zip(artifacts.iter())
        .enumerate()
        .map(|(ordinal, (case, artifact))| {
            let mut benchmark_case = BenchmarkCase::new(case.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", case.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", case.case_id));
            benchmark_case.metadata = json!({
                "summary": case.validation_case.summary,
                "workload_target": classify_case(&case.case_id),
                "artifact_id": artifact.artifact_id,
                "artifact_digest": artifact.artifact_digest,
                "program_digest": artifact.validated_program_digest,
                "expected_outputs": case.validation_case.expected_outputs,
                "expected_trace_steps": case.validation_case.expected_trace.len(),
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms,
                "sudoku_9x9_split": case.split.as_str(),
                "sudoku_9x9_given_count": case.given_count,
                "sudoku_9x9_puzzle_cells": case.puzzle_cells.clone(),
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_SUDOKU_9X9_BENCHMARK_REF, version),
        "Tassadar Sudoku-9x9 Benchmark",
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
        serde_json::to_value(&environment_bundle.current_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.planned_workload_targets"),
        serde_json::to_value(&environment_bundle.planned_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.cpu_baseline_metric_id"),
        Value::String(String::from(TASSADAR_CPU_BASELINE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.reference_linear_metric_id"),
        Value::String(String::from(TASSADAR_REFERENCE_LINEAR_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.hull_cache_metric_id"),
        Value::String(String::from(TASSADAR_HULL_CACHE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.sparse_top_k_metric_id"),
        Value::String(String::from(TASSADAR_SPARSE_TOP_K_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.metadata.insert(
        String::from("tassadar.sudoku_9x9_train_case_ids"),
        serde_json::to_value(train_case_ids).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.sudoku_9x9_validation_case_ids"),
        serde_json::to_value(validation_case_ids).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.sudoku_9x9_test_case_ids"),
        serde_json::to_value(test_case_ids).unwrap_or(Value::Null),
    );
    package.validate()?;
    Ok(package)
}

/// Builds digest-bound fixture artifacts for the bounded Hungarian-v0 corpus.
pub fn tassadar_hungarian_v0_program_artifacts(
    version: &str,
) -> Result<Vec<TassadarProgramArtifact>, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::hungarian_v0_matching_v1();
    let trace_abi = TassadarTraceAbi::hungarian_v0_matching_v1();
    tassadar_hungarian_v0_corpus()
        .into_iter()
        .map(|case| {
            TassadarProgramArtifact::fixture_reference(
                format!(
                    "tassadar://artifact/{version}/hungarian_v0/{}",
                    case.case_id
                ),
                &profile,
                &trace_abi,
                case.validation_case.program,
            )
            .map_err(TassadarBenchmarkError::from)
        })
        .collect()
}

fn build_tassadar_hungarian_v0_environment_bundle(
    version: &str,
    artifacts: &[TassadarProgramArtifact],
    corpus_digest: &str,
) -> Result<TassadarEnvironmentBundle, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::hungarian_v0_matching_v1();
    let trace_abi = TassadarTraceAbi::hungarian_v0_matching_v1();
    let dataset = DatasetKey::new(TASSADAR_HUNGARIAN_V0_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Hungarian-v0 Corpus"),
        eval_environment_ref: String::from(TASSADAR_HUNGARIAN_V0_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(TASSADAR_HUNGARIAN_V0_BENCHMARK_ENVIRONMENT_REF),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/hungarian_v0/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/hungarian_v0/benchmark"),
            required: true,
        },
        package_refs: TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.hungarian_v0"),
            eval_pin_alias: String::from("tassadar_hungarian_v0_eval"),
            benchmark_pin_alias: String::from("tassadar_hungarian_v0_benchmark"),
            eval_member_ref: String::from("tassadar_hungarian_v0_eval_member"),
            benchmark_member_ref: String::from("tassadar_hungarian_v0_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/hungarian_v0.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/hungarian_v0/eval"),
            benchmark_profile_ref: String::from(
                "benchmark://tassadar/hungarian_v0/reference_fixture",
            ),
            benchmark_runtime_profile_ref: String::from(
                "runtime://tassadar/hungarian_v0/benchmark",
            ),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/hungarian_v0.validation"),
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
            timeout_budget_ms: 15_000,
            trace_budget_steps: 4_096,
            require_cpu_reference_baseline: true,
            require_reference_linear_baseline: true,
            future_throughput_metric_ids: vec![String::from(TASSADAR_HULL_CACHE_METRIC_ID)],
        },
        benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
            package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
            package_set_version: String::from(version),
            supported_families: vec![TassadarBenchmarkFamily::Hungarian],
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
        spill_tape_store_binding: psionic_environments::default_tassadar_spill_tape_store_binding(),
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
            policy_ref: String::from("policy://tassadar/hungarian_v0/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/hungarian_v0/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from("policy://tassadar/hungarian_v0/benchmark/verification"),
                required: true,
            },
        ],
        current_workload_targets: vec![TassadarWorkloadTarget::HungarianMatching],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
    .map_err(TassadarBenchmarkError::from)
}

fn build_tassadar_hungarian_v0_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    artifacts: &[TassadarProgramArtifact],
) -> Result<BenchmarkPackage, TassadarBenchmarkError> {
    let corpus = tassadar_hungarian_v0_corpus();
    let train_case_ids =
        hungarian_v0_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Train);
    let validation_case_ids =
        hungarian_v0_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Validation);
    let test_case_ids =
        hungarian_v0_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Test);
    if artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: artifacts.len(),
        });
    }

    let cases = corpus
        .into_iter()
        .zip(artifacts.iter())
        .enumerate()
        .map(|(ordinal, (case, artifact))| {
            let mut benchmark_case = BenchmarkCase::new(case.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", case.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", case.case_id));
            benchmark_case.metadata = json!({
                "summary": case.validation_case.summary,
                "workload_target": classify_case(&case.case_id),
                "artifact_id": artifact.artifact_id,
                "artifact_digest": artifact.artifact_digest,
                "program_digest": artifact.validated_program_digest,
                "expected_outputs": case.validation_case.expected_outputs,
                "expected_trace_steps": case.validation_case.expected_trace.len(),
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms,
                "hungarian_v0_split": case.split.as_str(),
                "hungarian_v0_cost_matrix": case.cost_matrix.clone(),
                "hungarian_v0_optimal_assignment": case.optimal_assignment.clone(),
                "hungarian_v0_optimal_cost": case.optimal_cost,
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_HUNGARIAN_V0_BENCHMARK_REF, version),
        "Tassadar Hungarian-v0 Benchmark",
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
        serde_json::to_value(&environment_bundle.current_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.planned_workload_targets"),
        serde_json::to_value(&environment_bundle.planned_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.cpu_baseline_metric_id"),
        Value::String(String::from(TASSADAR_CPU_BASELINE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.reference_linear_metric_id"),
        Value::String(String::from(TASSADAR_REFERENCE_LINEAR_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.hull_cache_metric_id"),
        Value::String(String::from(TASSADAR_HULL_CACHE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.sparse_top_k_metric_id"),
        Value::String(String::from(TASSADAR_SPARSE_TOP_K_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.metadata.insert(
        String::from("tassadar.hungarian_v0_train_case_ids"),
        serde_json::to_value(train_case_ids).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.hungarian_v0_validation_case_ids"),
        serde_json::to_value(validation_case_ids).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.hungarian_v0_test_case_ids"),
        serde_json::to_value(test_case_ids).unwrap_or(Value::Null),
    );
    package.validate()?;
    Ok(package)
}

/// Builds digest-bound fixture artifacts for the article-sized Hungarian-10x10 corpus.
pub fn tassadar_hungarian_10x10_program_artifacts(
    version: &str,
) -> Result<Vec<TassadarProgramArtifact>, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::hungarian_10x10_matching_v1();
    let trace_abi = TassadarTraceAbi::hungarian_10x10_matching_v1();
    tassadar_hungarian_10x10_corpus()
        .into_iter()
        .map(|case| {
            TassadarProgramArtifact::fixture_reference(
                format!(
                    "tassadar://artifact/{version}/hungarian_10x10/{}",
                    case.case_id
                ),
                &profile,
                &trace_abi,
                case.validation_case.program,
            )
            .map_err(TassadarBenchmarkError::from)
        })
        .collect()
}

fn build_tassadar_hungarian_10x10_environment_bundle(
    version: &str,
    artifacts: &[TassadarProgramArtifact],
    corpus_digest: &str,
) -> Result<TassadarEnvironmentBundle, TassadarBenchmarkError> {
    let profile = TassadarWasmProfile::hungarian_10x10_matching_v1();
    let trace_abi = TassadarTraceAbi::hungarian_10x10_matching_v1();
    let dataset = DatasetKey::new(TASSADAR_HUNGARIAN_10X10_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Hungarian-10x10 Corpus"),
        eval_environment_ref: String::from(TASSADAR_HUNGARIAN_10X10_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(TASSADAR_HUNGARIAN_10X10_BENCHMARK_ENVIRONMENT_REF),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/hungarian_10x10/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/hungarian_10x10/benchmark"),
            required: true,
        },
        package_refs: TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.hungarian_10x10"),
            eval_pin_alias: String::from("tassadar_hungarian_10x10_eval"),
            benchmark_pin_alias: String::from("tassadar_hungarian_10x10_benchmark"),
            eval_member_ref: String::from("tassadar_hungarian_10x10_eval_member"),
            benchmark_member_ref: String::from("tassadar_hungarian_10x10_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/hungarian_10x10.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/hungarian_10x10/eval"),
            benchmark_profile_ref: String::from(
                "benchmark://tassadar/hungarian_10x10/reference_fixture",
            ),
            benchmark_runtime_profile_ref: String::from(
                "runtime://tassadar/hungarian_10x10/benchmark",
            ),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/hungarian_10x10.validation"),
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
            future_throughput_metric_ids: vec![String::from(TASSADAR_HULL_CACHE_METRIC_ID)],
        },
        benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
            package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
            package_set_version: String::from(version),
            supported_families: vec![TassadarBenchmarkFamily::Hungarian],
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
        spill_tape_store_binding: psionic_environments::default_tassadar_spill_tape_store_binding(),
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
            policy_ref: String::from("policy://tassadar/hungarian_10x10/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/hungarian_10x10/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from(
                    "policy://tassadar/hungarian_10x10/benchmark/verification",
                ),
                required: true,
            },
        ],
        current_workload_targets: vec![TassadarWorkloadTarget::HungarianMatching],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
    .map_err(TassadarBenchmarkError::from)
}

fn build_tassadar_hungarian_10x10_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    artifacts: &[TassadarProgramArtifact],
) -> Result<BenchmarkPackage, TassadarBenchmarkError> {
    let corpus = tassadar_hungarian_10x10_corpus();
    let train_case_ids =
        hungarian_10x10_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Train);
    let validation_case_ids = hungarian_10x10_case_ids_for_split(
        corpus.as_slice(),
        TassadarSudokuV0CorpusSplit::Validation,
    );
    let test_case_ids =
        hungarian_10x10_case_ids_for_split(corpus.as_slice(), TassadarSudokuV0CorpusSplit::Test);
    if artifacts.len() != corpus.len() {
        return Err(TassadarBenchmarkError::ArtifactCountMismatch {
            expected: corpus.len(),
            actual: artifacts.len(),
        });
    }

    let cases = corpus
        .into_iter()
        .zip(artifacts.iter())
        .enumerate()
        .map(|(ordinal, (case, artifact))| {
            let mut benchmark_case = BenchmarkCase::new(case.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", case.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", case.case_id));
            benchmark_case.metadata = json!({
                "summary": case.validation_case.summary,
                "workload_target": classify_case(&case.case_id),
                "artifact_id": artifact.artifact_id,
                "artifact_digest": artifact.artifact_digest,
                "program_digest": artifact.validated_program_digest,
                "expected_outputs": case.validation_case.expected_outputs,
                "expected_trace_steps": case.validation_case.expected_trace.len(),
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms,
                "hungarian_10x10_split": case.split.as_str(),
                "hungarian_10x10_cost_matrix": case.cost_matrix.clone(),
                "hungarian_10x10_search_row_order": case.search_row_order.clone(),
                "hungarian_10x10_optimal_assignment": case.optimal_assignment.clone(),
                "hungarian_10x10_optimal_cost": case.optimal_cost,
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_HUNGARIAN_10X10_BENCHMARK_REF, version),
        "Tassadar Hungarian-10x10 Benchmark",
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
        serde_json::to_value(&environment_bundle.current_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.planned_workload_targets"),
        serde_json::to_value(&environment_bundle.planned_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.cpu_baseline_metric_id"),
        Value::String(String::from(TASSADAR_CPU_BASELINE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.reference_linear_metric_id"),
        Value::String(String::from(TASSADAR_REFERENCE_LINEAR_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.hull_cache_metric_id"),
        Value::String(String::from(TASSADAR_HULL_CACHE_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.sparse_top_k_metric_id"),
        Value::String(String::from(TASSADAR_SPARSE_TOP_K_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.metadata.insert(
        String::from("tassadar.hungarian_10x10_train_case_ids"),
        serde_json::to_value(train_case_ids).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.hungarian_10x10_validation_case_ids"),
        serde_json::to_value(validation_case_ids).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.hungarian_10x10_test_case_ids"),
        serde_json::to_value(test_case_ids).unwrap_or(Value::Null),
    );
    package.validate()?;
    Ok(package)
}

fn classify_case(case_id: &str) -> TassadarWorkloadTarget {
    match case_id {
        "locals_add" | "tassadar.locals_add.v1" => TassadarWorkloadTarget::ArithmeticMicroprogram,
        "shortest_path_two_route" | "tassadar.shortest_path_two_route.v1" => {
            TassadarWorkloadTarget::ClrsShortestPath
        }
        "memory_roundtrip" | "tassadar.memory_roundtrip.v1" => {
            TassadarWorkloadTarget::MemoryLookupMicroprogram
        }
        "branch_guard" | "tassadar.branch_guard.v1" => {
            TassadarWorkloadTarget::BranchControlFlowMicroprogram
        }
        "micro_wasm_kernel" | "tassadar.micro_wasm_kernel.v2" => {
            TassadarWorkloadTarget::MicroWasmKernel
        }
        "branch_heavy_kernel" | "tassadar.branch_heavy_kernel.v1" => {
            TassadarWorkloadTarget::BranchHeavyKernel
        }
        "memory_heavy_kernel" | "tassadar.memory_heavy_kernel.v1" => {
            TassadarWorkloadTarget::MemoryHeavyKernel
        }
        "long_loop_kernel" | "tassadar.long_loop_kernel.v1" => {
            TassadarWorkloadTarget::LongLoopKernel
        }
        "sudoku_class" | "tassadar.sudoku_class.v2" => TassadarWorkloadTarget::SudokuClass,
        value if value.starts_with("sudoku_v0_") || value.starts_with("tassadar.sudoku_v0_") => {
            TassadarWorkloadTarget::SudokuClass
        }
        "hungarian_matching" | "tassadar.hungarian_matching.v2" => {
            TassadarWorkloadTarget::HungarianMatching
        }
        value
            if value.starts_with("hungarian_v0_")
                || value.starts_with("hungarian_10x10_")
                || value.starts_with("tassadar.hungarian_v0_")
                || value.starts_with("tassadar.hungarian_10x10_") =>
        {
            TassadarWorkloadTarget::HungarianMatching
        }
        _ => TassadarWorkloadTarget::MicroWasmKernel,
    }
}

fn sudoku_v0_case_metadata_map(
    sudoku_v0_corpus: &[TassadarSudokuV0CorpusCase],
) -> std::collections::BTreeMap<&str, &TassadarSudokuV0CorpusCase> {
    sudoku_v0_corpus
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect()
}

fn sudoku_v0_case_ids_for_split(
    sudoku_v0_corpus: &[TassadarSudokuV0CorpusCase],
    split: TassadarSudokuV0CorpusSplit,
) -> Vec<String> {
    sudoku_v0_corpus
        .iter()
        .filter(|case| case.split == split)
        .map(|case| case.case_id.clone())
        .collect()
}

fn sudoku_9x9_case_ids_for_split(
    corpus: &[TassadarSudoku9x9CorpusCase],
    split: TassadarSudokuV0CorpusSplit,
) -> Vec<String> {
    corpus
        .iter()
        .filter(|case| case.split == split)
        .map(|case| case.case_id.clone())
        .collect()
}

fn hungarian_v0_case_ids_for_split(
    corpus: &[TassadarHungarianV0CorpusCase],
    split: TassadarSudokuV0CorpusSplit,
) -> Vec<String> {
    corpus
        .iter()
        .filter(|case| case.split == split)
        .map(|case| case.case_id.clone())
        .collect()
}

fn hungarian_10x10_case_ids_for_split(
    corpus: &[TassadarHungarian10x10CorpusCase],
    split: TassadarSudokuV0CorpusSplit,
) -> Vec<String> {
    corpus
        .iter()
        .filter(|case| case.split == split)
        .map(|case| case.case_id.clone())
        .collect()
}

fn eval_repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
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

fn read_repo_json<T: serde::de::DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWorkloadCapabilityMatrixError> {
    let bytes = std::fs::read(eval_repo_root().join(relative_path))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn runtime_capability_cell(
    benchmark_report: &TassadarBenchmarkReport,
    workload_target: TassadarWorkloadTarget,
    surface_id: &str,
) -> Result<TassadarWorkloadCapabilityCell, TassadarWorkloadCapabilityMatrixError> {
    let cases = benchmark_report
        .case_reports
        .iter()
        .filter(|case| case.workload_target == workload_target)
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return Err(
            TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { workload_target },
        );
    }
    let exact_count = cases.iter().filter(|case| case.score_bps == 10_000).count();
    let total = cases.len();
    let (posture, note) = match surface_id {
        "runtime.reference_linear" => (
            TassadarCapabilityPosture::Exact,
            format!(
                "reference-linear exact on {exact_count}/{total} benchmark cases for `{}`",
                workload_target_id(workload_target)
            ),
        ),
        "runtime.hull_cache" => {
            let direct_count = cases
                .iter()
                .filter(|case| case.selection_state == TassadarExecutorSelectionState::Direct)
                .count();
            let fallback_count = total.saturating_sub(direct_count);
            let posture = if direct_count == total {
                TassadarCapabilityPosture::Exact
            } else {
                TassadarCapabilityPosture::FallbackOnly
            };
            (
                posture,
                format!(
                    "hull-cache exact on {exact_count}/{total} cases with {direct_count} direct and {fallback_count} fallback selections for `{}`",
                    workload_target_id(workload_target)
                ),
            )
        }
        "runtime.sparse_top_k" => {
            let direct_count = cases
                .iter()
                .filter(|case| {
                    case.sparse_top_k_selection_state == TassadarExecutorSelectionState::Direct
                })
                .count();
            let fallback_count = total.saturating_sub(direct_count);
            let posture = if direct_count == total {
                TassadarCapabilityPosture::Exact
            } else {
                TassadarCapabilityPosture::FallbackOnly
            };
            (
                posture,
                format!(
                    "sparse-top-k exact on {exact_count}/{total} cases with {direct_count} direct and {fallback_count} fallback selections for `{}`",
                    workload_target_id(workload_target)
                ),
            )
        }
        _ => unreachable!("unsupported runtime surface"),
    };
    Ok(TassadarWorkloadCapabilityCell {
        surface_id: String::from(surface_id),
        posture,
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        note,
    })
}

fn compiled_kernel_family_max_trace_step_count(report: &Value, family_id: &str) -> u64 {
    report["family_reports"]
        .as_array()
        .and_then(|families| {
            families
                .iter()
                .find(|family| family["family_id"].as_str() == Some(family_id))
        })
        .and_then(|family| family["regimes"].as_array())
        .map(|regimes| {
            regimes
                .iter()
                .filter_map(|regime| regime["trace_step_count"].as_u64())
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// Builds the machine-readable Tassadar workload capability matrix from the
/// current committed benchmark, compiled, and learned artifacts.
pub fn build_tassadar_workload_capability_matrix_report(
) -> Result<TassadarWorkloadCapabilityMatrixReport, TassadarWorkloadCapabilityMatrixError> {
    let benchmark_report =
        read_repo_json::<TassadarBenchmarkReport>(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF)?;
    let sudoku_compiled_run_bundle = read_repo_json::<Value>(
        "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/run_bundle.json",
    )?;
    let sudoku_9x9_compiled_run_bundle = read_repo_json::<Value>(
        "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json",
    )?;
    let hungarian_compiled_run_bundle = read_repo_json::<Value>(
        "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json",
    )?;
    let hungarian_10x10_compiled_run_bundle = read_repo_json::<Value>(
        "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json",
    )?;
    let compiled_kernel_suite_run_bundle =
        read_repo_json::<Value>("fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json")?;
    let compiled_kernel_suite_scaling_report = read_repo_json::<Value>(
        "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json",
    )?;
    let sudoku_promotion_bundle = read_repo_json::<Value>(
        "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_bundle.json",
    )?;
    let sudoku_promotion_gate = read_repo_json::<Value>(
        "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_gate_report.json",
    )?;
    let sudoku_promotion_family = read_repo_json::<Value>(
        "fixtures/tassadar/runs/sudoku_v0_promotion_v3/family_report.json",
    )?;
    let sudoku_9x9_fit_report = read_repo_json::<Value>(
        "fixtures/tassadar/runs/sudoku_9x9_v0_reference_run_v0/sequence_fit_report.json",
    )?;

    let rows = vec![
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("micro_wasm_kernel"),
            workload_target: Some(TassadarWorkloadTarget::MicroWasmKernel),
            summary: String::from("unrolled article-class micro-kernel over memory-backed inputs"),
            capabilities: vec![
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::MicroWasmKernel,
                    "runtime.reference_linear",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::MicroWasmKernel,
                    "runtime.hull_cache",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::MicroWasmKernel,
                    "runtime.sparse_top_k",
                )?,
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("branch_heavy_kernel"),
            workload_target: Some(TassadarWorkloadTarget::BranchHeavyKernel),
            summary: String::from(
                "forward-branch ladder kernel that widens control-flow coverage without backward-branch fast-path erasure",
            ),
            capabilities: vec![
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::BranchHeavyKernel,
                    "runtime.reference_linear",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::BranchHeavyKernel,
                    "runtime.hull_cache",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::BranchHeavyKernel,
                    "runtime.sparse_top_k",
                )?,
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("memory_heavy_kernel"),
            workload_target: Some(TassadarWorkloadTarget::MemoryHeavyKernel),
            summary: String::from(
                "dense load/store kernel with staged accumulator writes across the article benchmark path",
            ),
            capabilities: vec![
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::MemoryHeavyKernel,
                    "runtime.reference_linear",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::MemoryHeavyKernel,
                    "runtime.hull_cache",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::MemoryHeavyKernel,
                    "runtime.sparse_top_k",
                )?,
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("long_loop_kernel"),
            workload_target: Some(TassadarWorkloadTarget::LongLoopKernel),
            summary: String::from(
                "current article-class long-horizon decrement loop and trace-ABI exemplar",
            ),
            capabilities: vec![
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::LongLoopKernel,
                    "runtime.reference_linear",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::LongLoopKernel,
                    "runtime.hull_cache",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::LongLoopKernel,
                    "runtime.sparse_top_k",
                )?,
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("compiled.proof_backed"),
                    posture: TassadarCapabilityPosture::Exact,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json",
                    ),
                    note: format!(
                        "{}; max_trace_step_count={}",
                        compiled_kernel_suite_run_bundle["claim_boundary"]
                            .as_str()
                            .unwrap_or("compiled kernel suite boundary missing"),
                        compiled_kernel_family_max_trace_step_count(
                            &compiled_kernel_suite_scaling_report,
                            "backward_loop_kernel",
                        ),
                    ),
                },
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("arithmetic_kernel"),
            workload_target: Some(TassadarWorkloadTarget::ArithmeticMicroprogram),
            summary: String::from(
                "compiled-only arithmetic cascade family with exactness-vs-trace-length coverage",
            ),
            capabilities: vec![TassadarWorkloadCapabilityCell {
                surface_id: String::from("compiled.proof_backed"),
                posture: TassadarCapabilityPosture::Exact,
                artifact_ref: String::from(
                    "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json",
                ),
                note: format!(
                    "{}; max_trace_step_count={}",
                    compiled_kernel_suite_run_bundle["claim_boundary"]
                        .as_str()
                        .unwrap_or("compiled kernel suite boundary missing"),
                    compiled_kernel_family_max_trace_step_count(
                        &compiled_kernel_suite_scaling_report,
                        "arithmetic_kernel",
                    ),
                ),
            }],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("memory_update_kernel"),
            workload_target: Some(TassadarWorkloadTarget::MemoryLookupMicroprogram),
            summary: String::from(
                "compiled-only memory update family with exactness-vs-trace-length coverage",
            ),
            capabilities: vec![TassadarWorkloadCapabilityCell {
                surface_id: String::from("compiled.proof_backed"),
                posture: TassadarCapabilityPosture::Exact,
                artifact_ref: String::from(
                    "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json",
                ),
                note: format!(
                    "{}; max_trace_step_count={}",
                    compiled_kernel_suite_run_bundle["claim_boundary"]
                        .as_str()
                        .unwrap_or("compiled kernel suite boundary missing"),
                    compiled_kernel_family_max_trace_step_count(
                        &compiled_kernel_suite_scaling_report,
                        "memory_update_kernel",
                    ),
                ),
            }],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("forward_branch_kernel"),
            workload_target: Some(TassadarWorkloadTarget::BranchControlFlowMicroprogram),
            summary: String::from(
                "compiled-only forward-branch ladder family with exactness-vs-trace-length coverage",
            ),
            capabilities: vec![TassadarWorkloadCapabilityCell {
                surface_id: String::from("compiled.proof_backed"),
                posture: TassadarCapabilityPosture::Exact,
                artifact_ref: String::from(
                    "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json",
                ),
                note: format!(
                    "{}; max_trace_step_count={}",
                    compiled_kernel_suite_run_bundle["claim_boundary"]
                        .as_str()
                        .unwrap_or("compiled kernel suite boundary missing"),
                    compiled_kernel_family_max_trace_step_count(
                        &compiled_kernel_suite_scaling_report,
                        "forward_branch_kernel",
                    ),
                ),
            }],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("sudoku_class_4x4"),
            workload_target: Some(TassadarWorkloadTarget::SudokuClass),
            summary: String::from(
                "real 4x4 search-heavy Sudoku family with runtime, compiled, and bounded learned evidence",
            ),
            capabilities: vec![
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::SudokuClass,
                    "runtime.reference_linear",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::SudokuClass,
                    "runtime.hull_cache",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::SudokuClass,
                    "runtime.sparse_top_k",
                )?,
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("compiled.proof_backed"),
                    posture: TassadarCapabilityPosture::Exact,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/run_bundle.json",
                    ),
                    note: format!(
                        "{}; serve_posture={}",
                        sudoku_compiled_run_bundle["claim_boundary"]
                            .as_str()
                            .unwrap_or("compiled Sudoku boundary missing"),
                        sudoku_compiled_run_bundle["serve_posture"]
                            .as_str()
                            .unwrap_or("unknown")
                    ),
                },
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("learned.bounded"),
                    posture: TassadarCapabilityPosture::Partial,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_bundle.json",
                    ),
                    note: format!(
                        "claim_class={}; first_target_exactness_bps={}; first_32_token_exactness_bps={}; exact_trace_case_count={}; claim_boundary={}",
                        sudoku_promotion_bundle["claim_class"]
                            .as_str()
                            .unwrap_or("unknown"),
                        sudoku_promotion_gate["first_target_exactness_bps"]
                            .as_u64()
                            .unwrap_or(0),
                        sudoku_promotion_gate["first_32_token_exactness_bps"]
                            .as_u64()
                            .unwrap_or(0),
                        sudoku_promotion_gate["exact_trace_case_count"]
                            .as_u64()
                            .unwrap_or(0),
                        sudoku_promotion_family["claim_boundary"]
                            .as_str()
                            .unwrap_or("unknown")
                    ),
                },
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("sudoku_search_9x9"),
            workload_target: None,
            summary: String::from(
                "real 9x9 Sudoku search workload with exact compiled closure and still-bounded learned long-horizon coverage",
            ),
            capabilities: vec![
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("compiled.proof_backed"),
                    posture: TassadarCapabilityPosture::Exact,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json",
                    ),
                    note: format!(
                        "{}; serve_posture={}",
                        sudoku_9x9_compiled_run_bundle["claim_boundary"]
                            .as_str()
                            .unwrap_or("compiled Sudoku-9x9 boundary missing"),
                        sudoku_9x9_compiled_run_bundle["serve_posture"]
                            .as_str()
                            .unwrap_or("unknown")
                    ),
                },
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("learned.long_horizon"),
                    posture: TassadarCapabilityPosture::Partial,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/sudoku_9x9_v0_reference_run_v0/sequence_fit_report.json",
                    ),
                    note: format!(
                        "{}; model_max_sequence_tokens={}; total_token_count_min={}; total_token_count_max={}; strategy={}",
                        sudoku_9x9_fit_report["outcome_statement"]
                            .as_str()
                            .unwrap_or("unknown"),
                        sudoku_9x9_fit_report["model_max_sequence_tokens"]
                            .as_u64()
                            .unwrap_or(0),
                        sudoku_9x9_fit_report["total_token_count_min"]
                            .as_u64()
                            .unwrap_or(0),
                        sudoku_9x9_fit_report["total_token_count_max"]
                            .as_u64()
                            .unwrap_or(0),
                        sudoku_9x9_fit_report["teacher_forced_training_strategy"]
                            .as_str()
                            .unwrap_or("unknown")
                    ),
                },
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("hungarian_matching_4x4"),
            workload_target: Some(TassadarWorkloadTarget::HungarianMatching),
            summary: String::from(
                "bounded exact 4x4 Hungarian matching family with runtime and compiled coverage",
            ),
            capabilities: vec![
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::HungarianMatching,
                    "runtime.reference_linear",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::HungarianMatching,
                    "runtime.hull_cache",
                )?,
                runtime_capability_cell(
                    &benchmark_report,
                    TassadarWorkloadTarget::HungarianMatching,
                    "runtime.sparse_top_k",
                )?,
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("compiled.proof_backed"),
                    posture: TassadarCapabilityPosture::Exact,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json",
                    ),
                    note: format!(
                        "{}; serve_posture={}",
                        hungarian_compiled_run_bundle["claim_boundary"]
                            .as_str()
                            .unwrap_or("compiled Hungarian boundary missing"),
                        hungarian_compiled_run_bundle["serve_posture"]
                            .as_str()
                            .unwrap_or("unknown")
                    ),
                },
            ],
        },
        TassadarWorkloadCapabilityRow {
            workload_family_id: String::from("hungarian_matching_10x10"),
            workload_target: Some(TassadarWorkloadTarget::HungarianMatching),
            summary: String::from(
                "article-sized 10x10 Hungarian matching family with exact compiled/proof-backed closure and no learned long-horizon closure yet",
            ),
            capabilities: vec![
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("compiled.proof_backed"),
                    posture: TassadarCapabilityPosture::Exact,
                    artifact_ref: String::from(
                        "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json",
                    ),
                    note: format!(
                        "{}; serve_posture={}",
                        hungarian_10x10_compiled_run_bundle["claim_boundary"]
                            .as_str()
                            .unwrap_or("compiled Hungarian-10x10 boundary missing"),
                        hungarian_10x10_compiled_run_bundle["serve_posture"]
                            .as_str()
                            .unwrap_or("unknown")
                    ),
                },
                TassadarWorkloadCapabilityCell {
                    surface_id: String::from("learned.long_horizon"),
                    posture: TassadarCapabilityPosture::NotLanded,
                    artifact_ref: String::from(
                        "fixtures/tassadar/reports/tassadar_acceptance_report.json",
                    ),
                    note: String::from(
                        "no learned 10x10 Hungarian lane is landed; current article-sized matching closure is compiled/proof-backed only",
                    ),
                },
            ],
        },
    ];

    let mut report = TassadarWorkloadCapabilityMatrixReport {
        schema_version: TASSADAR_WORKLOAD_CAPABILITY_MATRIX_SCHEMA_VERSION,
        generated_from_artifacts: vec![
            String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
            String::from("fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/run_bundle.json"),
            String::from(
                "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json",
            ),
            String::from(
                "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json",
            ),
            String::from(
                "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json",
            ),
            String::from("fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json"),
            String::from(
                "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json",
            ),
            String::from("fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_bundle.json"),
            String::from(
                "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_gate_report.json",
            ),
            String::from("fixtures/tassadar/runs/sudoku_v0_promotion_v3/family_report.json"),
            String::from(
                "fixtures/tassadar/runs/sudoku_9x9_v0_reference_run_v0/sequence_fit_report.json",
            ),
        ],
        rows,
        claim_boundary: String::from(
            "this matrix is a workload-family capability view over current committed artifacts only; exact runtime, fallback-only runtime, compiled exact, and bounded/partial learned results remain separated and this report does not imply article parity",
        ),
        report_digest: String::new(),
    };
    report.report_digest =
        stable_serialized_digest(b"tassadar_workload_capability_matrix|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the workload capability matrix report.
#[must_use]
pub fn tassadar_workload_capability_matrix_report_path() -> std::path::PathBuf {
    eval_repo_root().join(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF)
}

/// Writes the canonical workload capability matrix report.
pub fn write_tassadar_workload_capability_matrix_report(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarWorkloadCapabilityMatrixReport, TassadarWorkloadCapabilityMatrixError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_workload_capability_matrix_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    std::fs::write(output_path, bytes)?;
    Ok(report)
}

fn build_hull_cache_workload_summary(
    benchmark_report: &TassadarBenchmarkReport,
    workload_target: TassadarWorkloadTarget,
) -> Option<TassadarHullCacheWorkloadSummary> {
    let cases = benchmark_report
        .case_reports
        .iter()
        .filter(|case| case.workload_target == workload_target)
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return None;
    }
    let direct_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Direct)
        .count();
    let fallback_case_count = cases.len().saturating_sub(direct_case_count);
    let posture = if fallback_case_count == 0 {
        TassadarCapabilityPosture::Exact
    } else {
        TassadarCapabilityPosture::FallbackOnly
    };
    let average_speedup_over_reference_linear = round_metric(
        cases
            .iter()
            .map(|case| case.hull_cache_speedup_over_reference_linear)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let average_remaining_gap_vs_cpu_reference = round_metric(
        cases
            .iter()
            .map(|case| case.hull_cache_remaining_gap_vs_cpu_reference)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let mut fallback_reason_counts = BTreeMap::new();
    for case in cases.iter().filter_map(|case| case.selection_reason) {
        let key = serde_json::to_string(&case)
            .unwrap_or_else(|_| String::from("\"unknown_selection_reason\""))
            .trim_matches('"')
            .to_string();
        *fallback_reason_counts.entry(key).or_insert(0) += 1;
    }
    Some(TassadarHullCacheWorkloadSummary {
        workload_target,
        posture,
        direct_case_count,
        fallback_case_count,
        average_speedup_over_reference_linear,
        average_remaining_gap_vs_cpu_reference,
        fallback_reason_counts,
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
    })
}

/// Builds the widened HullCache closure report from the committed article-class
/// benchmark artifact.
pub fn build_tassadar_hull_cache_closure_report(
) -> Result<TassadarHullCacheClosureReport, TassadarHullCacheClosureError> {
    let benchmark_report =
        read_repo_json::<TassadarBenchmarkReport>(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF)
            .map_err(|error| match error {
                TassadarWorkloadCapabilityMatrixError::Io(error) => {
                    TassadarHullCacheClosureError::Io(error)
                }
                TassadarWorkloadCapabilityMatrixError::Json(error) => {
                    TassadarHullCacheClosureError::Json(error)
                }
                TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { .. } => {
                    unreachable!("benchmark report read cannot surface missing workload-target")
                }
            })?;
    let summaries = [
        TassadarWorkloadTarget::MicroWasmKernel,
        TassadarWorkloadTarget::BranchHeavyKernel,
        TassadarWorkloadTarget::MemoryHeavyKernel,
        TassadarWorkloadTarget::LongLoopKernel,
        TassadarWorkloadTarget::SudokuClass,
        TassadarWorkloadTarget::HungarianMatching,
    ]
    .into_iter()
    .filter_map(|workload_target| {
        build_hull_cache_workload_summary(&benchmark_report, workload_target)
    })
    .collect::<Vec<_>>();
    let (exact_workloads, fallback_only_workloads): (Vec<_>, Vec<_>) = summaries
        .into_iter()
        .partition(|summary| summary.posture == TassadarCapabilityPosture::Exact);
    let mut report = TassadarHullCacheClosureReport {
        schema_version: TASSADAR_HULL_CACHE_CLOSURE_SCHEMA_VERSION,
        exact_workloads,
        fallback_only_workloads,
        claim_boundary: String::from(
            "the widened HullCache closure is now artifact-backed for direct micro, branch-heavy, memory-heavy, long-loop, Sudoku search, and bounded Hungarian article workloads inside the current canonical article profile. It proves exact direct HullCache posture on those declared workload families, but it does not yet claim throughput-floor closure or broader arbitrary-program control-flow support beyond the committed article corpus",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_serialized_digest(b"tassadar_hull_cache_closure|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the widened HullCache closure report.
#[must_use]
pub fn tassadar_hull_cache_closure_report_path() -> std::path::PathBuf {
    eval_repo_root().join(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF)
}

/// Writes the canonical widened HullCache closure report.
pub fn write_tassadar_hull_cache_closure_report(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarHullCacheClosureReport, TassadarHullCacheClosureError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_hull_cache_closure_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    std::fs::write(output_path, bytes)?;
    Ok(report)
}

fn build_sparse_top_k_workload_summary(
    benchmark_report: &TassadarBenchmarkReport,
    workload_target: TassadarWorkloadTarget,
) -> Option<TassadarSparseTopKWorkloadSummary> {
    let cases = benchmark_report
        .case_reports
        .iter()
        .filter(|case| case.workload_target == workload_target)
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return None;
    }
    let direct_case_count = cases
        .iter()
        .filter(|case| case.sparse_top_k_selection_state == TassadarExecutorSelectionState::Direct)
        .count();
    let fallback_case_count = cases.len().saturating_sub(direct_case_count);
    let posture = if fallback_case_count == 0 {
        TassadarCapabilityPosture::Exact
    } else {
        TassadarCapabilityPosture::FallbackOnly
    };
    let average_speedup_over_reference_linear = round_metric(
        cases
            .iter()
            .map(|case| case.sparse_top_k_speedup_over_reference_linear)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let average_remaining_gap_vs_cpu_reference = round_metric(
        cases
            .iter()
            .map(|case| case.sparse_top_k_remaining_gap_vs_cpu_reference)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let average_sparse_vs_hull_speed_ratio = round_metric(
        cases
            .iter()
            .map(|case| {
                case.sparse_top_k_steps_per_second / case.hull_cache_steps_per_second.max(1e-9)
            })
            .sum::<f64>()
            / cases.len() as f64,
    );
    let mut fallback_reason_counts = BTreeMap::new();
    for case in cases
        .iter()
        .filter_map(|case| case.sparse_top_k_selection_reason)
    {
        let key = serde_json::to_string(&case)
            .unwrap_or_else(|_| String::from("\"unknown_selection_reason\""))
            .trim_matches('"')
            .to_string();
        *fallback_reason_counts.entry(key).or_insert(0) += 1;
    }
    Some(TassadarSparseTopKWorkloadSummary {
        workload_target,
        posture,
        direct_case_count,
        fallback_case_count,
        average_speedup_over_reference_linear,
        average_remaining_gap_vs_cpu_reference,
        average_sparse_vs_hull_speed_ratio,
        fallback_reason_counts,
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
    })
}

/// Builds the SparseTopK comparison report from the committed article-class
/// benchmark artifact.
pub fn build_tassadar_sparse_top_k_comparison_report(
) -> Result<TassadarSparseTopKComparisonReport, TassadarSparseTopKComparisonError> {
    let benchmark_report =
        read_repo_json::<TassadarBenchmarkReport>(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF)
            .map_err(|error| match error {
                TassadarWorkloadCapabilityMatrixError::Io(error) => {
                    TassadarSparseTopKComparisonError::Io(error)
                }
                TassadarWorkloadCapabilityMatrixError::Json(error) => {
                    TassadarSparseTopKComparisonError::Json(error)
                }
                TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { .. } => {
                    unreachable!("benchmark report read cannot surface missing workload-target")
                }
            })?;
    let summaries = [
        TassadarWorkloadTarget::MicroWasmKernel,
        TassadarWorkloadTarget::BranchHeavyKernel,
        TassadarWorkloadTarget::MemoryHeavyKernel,
        TassadarWorkloadTarget::LongLoopKernel,
        TassadarWorkloadTarget::SudokuClass,
        TassadarWorkloadTarget::HungarianMatching,
    ]
    .into_iter()
    .filter_map(|workload_target| {
        build_sparse_top_k_workload_summary(&benchmark_report, workload_target)
    })
    .collect::<Vec<_>>();
    let (exact_workloads, fallback_only_workloads): (Vec<_>, Vec<_>) = summaries
        .into_iter()
        .partition(|summary| summary.posture == TassadarCapabilityPosture::Exact);
    let mut report = TassadarSparseTopKComparisonReport {
        schema_version: TASSADAR_SPARSE_TOP_K_COMPARISON_SCHEMA_VERSION,
        exact_workloads,
        fallback_only_workloads,
        claim_boundary: String::from(
            "SparseTopK now has an artifact-backed one-for-one comparison against reference-linear and HullCache on the shared article workload set; branch-heavy, long-loop, and Sudoku search workloads remain explicit fallback-only families under the current validation contract",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_serialized_digest(b"tassadar_sparse_top_k_comparison|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the SparseTopK comparison report.
#[must_use]
pub fn tassadar_sparse_top_k_comparison_report_path() -> std::path::PathBuf {
    eval_repo_root().join(TASSADAR_SPARSE_TOP_K_COMPARISON_REPORT_REF)
}

/// Writes the canonical SparseTopK comparison report.
pub fn write_tassadar_sparse_top_k_comparison_report(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarSparseTopKComparisonReport, TassadarSparseTopKComparisonError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_sparse_top_k_comparison_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    std::fs::write(output_path, bytes)?;
    Ok(report)
}

fn decode_scaling_profile() -> TassadarWasmProfile {
    TassadarWasmProfile::article_i32_compute_v1()
}

fn build_linear_memory_accumulator_program(element_count: usize) -> TassadarProgram {
    let profile = decode_scaling_profile();
    let mut instructions = vec![
        TassadarInstruction::I32Const { value: 0 },
        TassadarInstruction::LocalSet { local: 0 },
    ];
    for slot in 0..element_count {
        instructions.push(TassadarInstruction::I32Load { slot: slot as u8 });
        instructions.push(TassadarInstruction::LocalGet { local: 0 });
        instructions.push(TassadarInstruction::I32Add);
        instructions.push(TassadarInstruction::LocalSet { local: 0 });
    }
    instructions.push(TassadarInstruction::LocalGet { local: 0 });
    instructions.push(TassadarInstruction::Output);
    instructions.push(TassadarInstruction::Return);
    TassadarProgram::new(
        format!("tassadar.scaling.linear_memory_accumulator.e{element_count}.v1"),
        &profile,
        1,
        element_count,
        instructions,
    )
    .with_initial_memory((1..=element_count as i32).collect())
}

fn build_forward_branch_ladder_program(branch_count: usize) -> TassadarProgram {
    let profile = decode_scaling_profile();
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
        format!("tassadar.scaling.forward_branch_ladder.b{branch_count}.v1"),
        &profile,
        1,
        branch_count,
        instructions,
    )
    .with_initial_memory(initial_memory)
}

fn build_backward_branch_loop_program(iteration_count: i32) -> TassadarProgram {
    let profile = decode_scaling_profile();
    TassadarProgram::new(
        format!("tassadar.scaling.backward_branch_loop.i{iteration_count}.v1"),
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

fn benchmark_decode_mode_steps_per_second(
    program: &TassadarProgram,
    trace_steps: u64,
    requested_decode_mode: TassadarExecutorDecodeMode,
    reference_linear_steps_per_second: f64,
    cached_hull_cache_steps_per_second: Option<f64>,
    cached_sparse_top_k_steps_per_second: Option<f64>,
) -> Result<
    (
        TassadarExecutorSelectionState,
        Option<TassadarExecutorSelectionReason>,
        TassadarExecutorDecodeMode,
        f64,
    ),
    TassadarDecodeScalingReportError,
> {
    let trace_abi =
        tassadar_trace_abi_for_profile_id(program.profile_id.as_str()).ok_or_else(|| {
            TassadarDecodeScalingReportError::Benchmark(TassadarBenchmarkError::ExecutionRefusal(
                TassadarExecutionRefusal::ProfileMismatch {
                    expected: String::from("one of the supported Tassadar Wasm profiles"),
                    actual: program.profile_id.clone(),
                },
            ))
        })?;
    let selection = diagnose_tassadar_executor_request(
        program,
        requested_decode_mode,
        trace_abi.schema_version,
        Some(&[
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarExecutorDecodeMode::HullCache,
            TassadarExecutorDecodeMode::SparseTopK,
        ]),
    );
    let effective_decode_mode = selection.effective_decode_mode.ok_or(
        TassadarDecodeScalingReportError::MissingEffectiveDecodeMode {
            program_id: program.program_id.clone(),
            requested_decode_mode,
        },
    )?;

    let steps_per_second = match effective_decode_mode {
        TassadarExecutorDecodeMode::ReferenceLinear => reference_linear_steps_per_second,
        TassadarExecutorDecodeMode::HullCache => {
            if let Some(cached) = cached_hull_cache_steps_per_second {
                cached
            } else {
                let hull_cache_runner = TassadarHullCacheRunner::for_program(program)?;
                benchmark_scaling_runner_steps_per_second(trace_steps, || {
                    hull_cache_runner.execute(program)
                })?
            }
        }
        TassadarExecutorDecodeMode::SparseTopK => {
            if let Some(cached) = cached_sparse_top_k_steps_per_second {
                cached
            } else {
                let sparse_top_k_runner = TassadarSparseTopKRunner::for_program(program)?;
                benchmark_scaling_runner_steps_per_second(trace_steps, || {
                    sparse_top_k_runner.execute(program)
                })?
            }
        }
    };

    Ok((
        selection.selection_state,
        selection.selection_reason,
        effective_decode_mode,
        steps_per_second,
    ))
}

fn build_decode_scaling_regime(
    regime_id: impl Into<String>,
    summary: impl Into<String>,
    length_parameter_name: &str,
    length_parameter_value: u64,
    program: TassadarProgram,
) -> Result<TassadarDecodeScalingRegimeReport, TassadarDecodeScalingReportError> {
    let cpu_runner = TassadarCpuReferenceRunner::for_program(&program)?;
    let reference_linear_runner = TassadarFixtureRunner::for_program(&program)?;
    let equivalence_report = run_tassadar_exact_equivalence(&program)?;
    let reference_execution = &equivalence_report.reference_linear;
    let trace_step_count = reference_execution.steps.len() as u64;
    let cpu_reference_steps_per_second =
        benchmark_scaling_runner_steps_per_second(trace_step_count, || {
            cpu_runner.execute(&program)
        })?;
    let reference_linear_steps_per_second =
        benchmark_scaling_runner_steps_per_second(trace_step_count, || {
            reference_linear_runner.execute(&program)
        })?;

    let hull_cache_runner = TassadarHullCacheRunner::for_program(&program)?;
    let hull_cache_direct_steps_per_second =
        benchmark_scaling_runner_steps_per_second(trace_step_count, || {
            hull_cache_runner.execute(&program)
        })
        .ok();

    let sparse_top_k_runner = TassadarSparseTopKRunner::for_program(&program)?;
    let sparse_top_k_direct_steps_per_second =
        benchmark_scaling_runner_steps_per_second(trace_step_count, || {
            sparse_top_k_runner.execute(&program)
        })
        .ok();

    let (
        hull_cache_selection_state,
        hull_cache_selection_reason,
        hull_cache_effective_decode_mode,
        hull_cache_steps_per_second,
    ) = benchmark_decode_mode_steps_per_second(
        &program,
        trace_step_count,
        TassadarExecutorDecodeMode::HullCache,
        reference_linear_steps_per_second,
        hull_cache_direct_steps_per_second,
        sparse_top_k_direct_steps_per_second,
    )?;
    let (
        sparse_top_k_selection_state,
        sparse_top_k_selection_reason,
        sparse_top_k_effective_decode_mode,
        sparse_top_k_steps_per_second,
    ) = benchmark_decode_mode_steps_per_second(
        &program,
        trace_step_count,
        TassadarExecutorDecodeMode::SparseTopK,
        reference_linear_steps_per_second,
        hull_cache_direct_steps_per_second,
        sparse_top_k_direct_steps_per_second,
    )?;

    let trace_digest_equal = equivalence_report.trace_digest_equal();
    let outputs_equal = equivalence_report.outputs_equal();
    let halt_equal = equivalence_report.halt_equal();
    let exactness_bps =
        (u32::from(trace_digest_equal) + u32::from(outputs_equal) + u32::from(halt_equal)) * 10_000
            / 3;
    let hull_cache_speedup_over_reference_linear =
        round_metric(hull_cache_steps_per_second / reference_linear_steps_per_second.max(1e-9));
    let hull_cache_remaining_gap_vs_cpu_reference =
        round_metric(cpu_reference_steps_per_second / hull_cache_steps_per_second.max(1e-9));
    let sparse_top_k_speedup_over_reference_linear =
        round_metric(sparse_top_k_steps_per_second / reference_linear_steps_per_second.max(1e-9));
    let sparse_top_k_remaining_gap_vs_cpu_reference =
        round_metric(cpu_reference_steps_per_second / sparse_top_k_steps_per_second.max(1e-9));

    let trace_artifact = TassadarTraceArtifact::from_execution(
        format!("tassadar://artifact/decode_scaling/{}", program.program_id),
        &equivalence_report.cpu_reference,
    );
    let trace_artifact_bytes = serde_json::to_vec(&trace_artifact)?.len() as u64;
    let trace_artifact_bytes_per_step =
        round_metric(trace_artifact_bytes as f64 / trace_step_count.max(1) as f64);
    let max_stack_depth = equivalence_report
        .cpu_reference
        .steps
        .iter()
        .map(|step| step.stack_before.len().max(step.stack_after.len()))
        .max()
        .unwrap_or(0);

    Ok(TassadarDecodeScalingRegimeReport {
        regime_id: regime_id.into(),
        summary: summary.into(),
        program_id: program.program_id.clone(),
        profile_id: program.profile_id.clone(),
        length_parameter_name: String::from(length_parameter_name),
        length_parameter_value,
        instruction_count: program.instructions.len(),
        trace_step_count,
        trace_artifact_bytes,
        trace_artifact_bytes_per_step,
        max_stack_depth,
        memory_slot_count: program.memory_slots,
        cpu_reference_steps_per_second: round_metric(cpu_reference_steps_per_second),
        reference_linear_steps_per_second: round_metric(reference_linear_steps_per_second),
        hull_cache_steps_per_second: round_metric(hull_cache_steps_per_second),
        hull_cache_speedup_over_reference_linear,
        hull_cache_remaining_gap_vs_cpu_reference,
        hull_cache_selection_state,
        hull_cache_selection_reason,
        hull_cache_effective_decode_mode,
        sparse_top_k_steps_per_second: round_metric(sparse_top_k_steps_per_second),
        sparse_top_k_speedup_over_reference_linear,
        sparse_top_k_remaining_gap_vs_cpu_reference,
        sparse_top_k_selection_state,
        sparse_top_k_selection_reason,
        sparse_top_k_effective_decode_mode,
        trace_digest_equal,
        outputs_equal,
        halt_equal,
        exactness_bps,
        cpu_behavior_digest: equivalence_report.cpu_reference.behavior_digest(),
        reference_linear_behavior_digest: equivalence_report.reference_linear.behavior_digest(),
        hull_cache_behavior_digest: equivalence_report.hull_cache.behavior_digest(),
        sparse_top_k_behavior_digest: equivalence_report.sparse_top_k.behavior_digest(),
    })
}

/// Builds the machine-readable decode-scaling report across the current active
/// Tassadar decode modes.
pub fn build_tassadar_decode_scaling_report(
) -> Result<TassadarDecodeScalingReport, TassadarDecodeScalingReportError> {
    let _article_benchmark_report: Value = read_repo_json(
        TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
    )
    .map_err(|error| match error {
        TassadarWorkloadCapabilityMatrixError::Io(error) => {
            TassadarDecodeScalingReportError::Io(error)
        }
        TassadarWorkloadCapabilityMatrixError::Json(error) => {
            TassadarDecodeScalingReportError::Json(error)
        }
        TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { .. } => {
            unreachable!("repo JSON reads do not synthesize workload-target errors")
        }
    })?;
    let _hull_cache_closure_report: Value = read_repo_json(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF)
        .map_err(|error| match error {
            TassadarWorkloadCapabilityMatrixError::Io(error) => {
                TassadarDecodeScalingReportError::Io(error)
            }
            TassadarWorkloadCapabilityMatrixError::Json(error) => {
                TassadarDecodeScalingReportError::Json(error)
            }
            TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { .. } => {
                unreachable!("repo JSON reads do not synthesize workload-target errors")
            }
        })?;
    let _sparse_top_k_comparison_report: Value = read_repo_json(
        TASSADAR_SPARSE_TOP_K_COMPARISON_REPORT_REF,
    )
    .map_err(|error| match error {
        TassadarWorkloadCapabilityMatrixError::Io(error) => {
            TassadarDecodeScalingReportError::Io(error)
        }
        TassadarWorkloadCapabilityMatrixError::Json(error) => {
            TassadarDecodeScalingReportError::Json(error)
        }
        TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { .. } => {
            unreachable!("repo JSON reads do not synthesize workload-target errors")
        }
    })?;

    let families = vec![
        TassadarDecodeScalingFamilyReport {
            family_id: String::from("linear_memory_accumulator"),
            summary: String::from(
                "acyclic load-add accumulation family that stays inside the current SparseTopK validation ceiling while growing trace and trace-artifact size",
            ),
            length_parameter_name: String::from("element_count"),
            claim_boundary: String::from(
                "all current decode modes stay direct on this bounded straight-line family because the instruction count remains within the sparse validated subset",
            ),
            regimes: [4usize, 10, 14]
                .into_iter()
                .map(|element_count| {
                    build_decode_scaling_regime(
                        format!("element_count_{element_count}"),
                        format!(
                            "accumulate the first {element_count} memory slots into one output"
                        ),
                        "element_count",
                        element_count as u64,
                        build_linear_memory_accumulator_program(element_count),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        TassadarDecodeScalingFamilyReport {
            family_id: String::from("forward_branch_ladder"),
            summary: String::from(
                "acyclic forward-branch family that keeps HullCache direct as control-flow width grows while SparseTopK eventually falls back on its validated instruction ceiling",
            ),
            length_parameter_name: String::from("branch_count"),
            claim_boundary: String::from(
                "HullCache stays direct on this acyclic branch family, but SparseTopK becomes fallback-only once the ladder exceeds its current 64-instruction validation limit",
            ),
            regimes: [2usize, 8, 16]
                .into_iter()
                .map(|branch_count| {
                    build_decode_scaling_regime(
                        format!("branch_count_{branch_count}"),
                        format!(
                            "evaluate {branch_count} forward branch pivots with alternating taken and untaken lanes"
                        ),
                        "branch_count",
                        branch_count as u64,
                        build_forward_branch_ladder_program(branch_count),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        TassadarDecodeScalingFamilyReport {
            family_id: String::from("backward_branch_loop"),
            summary: String::from(
                "long-horizon decrement-loop family that makes the current fallback-only control-flow boundary explicit as trace length approaches the article-profile step ceiling",
            ),
            length_parameter_name: String::from("iteration_count"),
            claim_boundary: String::from(
                "backward-branch programs remain explicit fallback-only on the current HullCache and SparseTopK surfaces, so this family records truthful long-horizon scaling under fallback instead of pretending direct fast-path closure",
            ),
            regimes: [255i32, 1_023, 2_047]
                .into_iter()
                .map(|iteration_count| {
                    build_decode_scaling_regime(
                        format!("iteration_count_{iteration_count}"),
                        format!(
                            "count down from {iteration_count} through one backward-branch loop before halting"
                        ),
                        "iteration_count",
                        iteration_count as u64,
                        build_backward_branch_loop_program(iteration_count),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
    ];

    let mut report = TassadarDecodeScalingReport {
        schema_version: TASSADAR_DECODE_SCALING_SCHEMA_VERSION,
        generated_from_artifacts: vec![
            String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
            String::from(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF),
            String::from(TASSADAR_SPARSE_TOP_K_COMPARISON_REPORT_REF),
        ],
        memory_growth_metric: String::from("serialized_trace_artifact_bytes"),
        families,
        claim_boundary: String::from(
            "this scaling report compares the current requested decode modes on shared synthetic workload families, records trace-artifact bytes as the current deterministic memory-growth proxy, and keeps direct-vs-fallback posture explicit instead of pretending that every long-horizon trace is inside the fast-path subset",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_serialized_digest(b"tassadar_decode_scaling_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the decode-scaling report.
#[must_use]
pub fn tassadar_decode_scaling_report_path() -> std::path::PathBuf {
    eval_repo_root().join(TASSADAR_DECODE_SCALING_REPORT_REF)
}

/// Writes the canonical decode-scaling report.
pub fn write_tassadar_decode_scaling_report(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarDecodeScalingReport, TassadarDecodeScalingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_decode_scaling_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    std::fs::write(output_path, bytes)?;
    Ok(report)
}

fn geometric_variant_reason_key(reason: Option<TassadarExecutorSelectionReason>) -> Option<String> {
    reason.map(|reason| {
        serde_json::to_string(&reason)
            .unwrap_or_else(|_| String::from("\"unknown_selection_reason\""))
            .trim_matches('"')
            .to_string()
    })
}

fn build_runtime_hull_cache_variant_case_report(
    benchmark_case: &TassadarBenchmarkCaseReport,
) -> TassadarGeometricVariantCaseReport {
    let exactness_bps =
        u32::from(benchmark_case.hull_cache_behavior_digest == benchmark_case.cpu_behavior_digest)
            * 10_000;
    let note = if benchmark_case.selection_state == TassadarExecutorSelectionState::Direct {
        String::from(
            "current promoted HullCache runtime path stayed direct and exact on this workload case",
        )
    } else {
        String::from(
            "current promoted HullCache runtime path remained exact here only by explicitly falling back to the runtime's exact reference-linear mode",
        )
    };
    TassadarGeometricVariantCaseReport {
        case_id: benchmark_case.case_id.clone(),
        workload_target: benchmark_case.workload_target,
        variant_id: String::from(TASSADAR_RUNTIME_HULL_GEOMETRIC_VARIANT_ID),
        claim_boundary: TassadarGeometricVariantClaimBoundary::RuntimeReady,
        claim_class: TassadarClaimClass::CompiledExact,
        selection_state: benchmark_case.selection_state,
        selection_reason: benchmark_case.selection_reason,
        trace_digest_equal: benchmark_case.trace_digest_equal,
        outputs_equal: benchmark_case.outputs_equal,
        halt_equal: benchmark_case.halt_equal,
        exactness_bps,
        trace_steps: benchmark_case.trace_steps,
        reference_linear_steps_per_second: benchmark_case.reference_linear_steps_per_second,
        cpu_reference_steps_per_second: benchmark_case.cpu_reference_steps_per_second,
        variant_steps_per_second: benchmark_case.hull_cache_steps_per_second,
        speedup_over_reference_linear: benchmark_case.hull_cache_speedup_over_reference_linear,
        remaining_gap_vs_cpu_reference: benchmark_case.hull_cache_remaining_gap_vs_cpu_reference,
        cpu_behavior_digest: benchmark_case.cpu_behavior_digest.clone(),
        variant_behavior_digest: benchmark_case.hull_cache_behavior_digest.clone(),
        artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        note,
    }
}

fn build_hierarchical_hull_variant_case_report(
    benchmark_case: &TassadarBenchmarkCaseReport,
    program: &TassadarProgram,
) -> Result<TassadarGeometricVariantCaseReport, TassadarGeometricVariantReportError> {
    let cpu_runner = TassadarCpuReferenceRunner::for_program(program)?;
    let cpu_execution = cpu_runner.execute(program)?;
    let candidate_runner = TassadarHierarchicalHullCandidateRunner::for_program(program)?;
    let candidate_execution = candidate_runner.execute(program)?;
    let trace_steps = candidate_execution.steps.len() as u64;
    let variant_steps_per_second =
        benchmark_runner_steps_per_second(trace_steps, || candidate_runner.execute(program))?;
    let trace_digest_equal = candidate_execution.trace_digest() == cpu_execution.trace_digest();
    let outputs_equal = candidate_execution.outputs == cpu_execution.outputs;
    let halt_equal = candidate_execution.halt_reason == cpu_execution.halt_reason;
    let exactness_bps = u32::from(
        trace_digest_equal
            && outputs_equal
            && halt_equal
            && candidate_execution.behavior_digest() == cpu_execution.behavior_digest(),
    ) * 10_000;
    let note = if benchmark_case.selection_state == TassadarExecutorSelectionState::Direct {
        String::from(
            "research-only hierarchical hull candidate matches the promoted HullCache lane on this already-direct workload case",
        )
    } else {
        String::from(
            "research-only hierarchical hull candidate stays direct on this case while the promoted HullCache runtime path still falls back, so the widened exact class remains explicit but unpromoted",
        )
    };
    Ok(TassadarGeometricVariantCaseReport {
        case_id: benchmark_case.case_id.clone(),
        workload_target: benchmark_case.workload_target,
        variant_id: String::from(TASSADAR_HIERARCHICAL_HULL_GEOMETRIC_VARIANT_ID),
        claim_boundary: TassadarGeometricVariantClaimBoundary::ResearchOnly,
        claim_class: TassadarClaimClass::ResearchOnly,
        selection_state: TassadarExecutorSelectionState::Direct,
        selection_reason: None,
        trace_digest_equal,
        outputs_equal,
        halt_equal,
        exactness_bps,
        trace_steps,
        reference_linear_steps_per_second: benchmark_case.reference_linear_steps_per_second,
        cpu_reference_steps_per_second: benchmark_case.cpu_reference_steps_per_second,
        variant_steps_per_second: round_metric(variant_steps_per_second),
        speedup_over_reference_linear: round_metric(
            variant_steps_per_second / benchmark_case.reference_linear_steps_per_second.max(1e-9),
        ),
        remaining_gap_vs_cpu_reference: round_metric(
            benchmark_case.cpu_reference_steps_per_second / variant_steps_per_second.max(1e-9),
        ),
        cpu_behavior_digest: cpu_execution.behavior_digest(),
        variant_behavior_digest: candidate_execution.behavior_digest(),
        artifact_ref: String::from(TASSADAR_GEOMETRIC_VARIANT_REPORT_REF),
        note,
    })
}

fn build_geometric_variant_workload_summary(
    case_reports: &[TassadarGeometricVariantCaseReport],
    variant_id: &str,
    workload_target: TassadarWorkloadTarget,
) -> Option<TassadarGeometricVariantWorkloadSummary> {
    let cases = case_reports
        .iter()
        .filter(|case| case.variant_id == variant_id && case.workload_target == workload_target)
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return None;
    }
    let direct_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Direct)
        .count();
    let fallback_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Fallback)
        .count();
    let refused_case_count = cases
        .iter()
        .filter(|case| case.selection_state == TassadarExecutorSelectionState::Refused)
        .count();
    let exact_case_count = cases
        .iter()
        .filter(|case| case.exactness_bps == 10_000)
        .count();
    let mut selection_reason_counts = BTreeMap::new();
    for key in cases
        .iter()
        .filter_map(|case| geometric_variant_reason_key(case.selection_reason))
    {
        *selection_reason_counts.entry(key).or_insert(0) += 1;
    }
    let average_steps_per_second = round_metric(
        cases
            .iter()
            .map(|case| case.variant_steps_per_second)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let average_speedup_over_reference_linear = round_metric(
        cases
            .iter()
            .map(|case| case.speedup_over_reference_linear)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let average_remaining_gap_vs_cpu_reference = round_metric(
        cases
            .iter()
            .map(|case| case.remaining_gap_vs_cpu_reference)
            .sum::<f64>()
            / cases.len() as f64,
    );
    let claim_boundary = cases[0].claim_boundary;
    let claim_class = cases[0].claim_class;
    let artifact_ref = cases[0].artifact_ref.clone();
    let note = if variant_id == TASSADAR_RUNTIME_HULL_GEOMETRIC_VARIANT_ID {
        String::from(
            "current promoted HullCache runtime posture, including explicit fallback truth where the validated subset still ends",
        )
    } else {
        String::from(
            "research-only hierarchical hull candidate posture; any widened exact class here is evidence for follow-on work, not a promoted runtime claim",
        )
    };
    Some(TassadarGeometricVariantWorkloadSummary {
        variant_id: String::from(variant_id),
        claim_boundary,
        claim_class,
        workload_target,
        direct_case_count,
        fallback_case_count,
        refused_case_count,
        exact_case_count,
        average_steps_per_second,
        average_speedup_over_reference_linear,
        average_remaining_gap_vs_cpu_reference,
        selection_reason_counts,
        artifact_ref,
        note,
    })
}

/// Builds the machine-readable geometric-variant comparison report.
pub fn build_tassadar_geometric_variant_report(
) -> Result<TassadarGeometricVariantReport, TassadarGeometricVariantReportError> {
    let benchmark_report: TassadarBenchmarkReport = read_repo_json(
        TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
    )
    .map_err(|error| match error {
        TassadarWorkloadCapabilityMatrixError::Io(error) => {
            TassadarGeometricVariantReportError::Io(error)
        }
        TassadarWorkloadCapabilityMatrixError::Json(error) => {
            TassadarGeometricVariantReportError::Json(error)
        }
        TassadarWorkloadCapabilityMatrixError::MissingWorkloadTarget { .. } => {
            unreachable!("repo JSON reads do not synthesize workload-target errors")
        }
    })?;
    let article_cases = tassadar_article_class_corpus()
        .into_iter()
        .map(|case| (case.case_id.clone(), case))
        .collect::<BTreeMap<_, _>>();

    let mut case_reports = benchmark_report
        .case_reports
        .iter()
        .map(build_runtime_hull_cache_variant_case_report)
        .collect::<Vec<_>>();
    for benchmark_case in &benchmark_report.case_reports {
        let Some(article_case) = article_cases.get(benchmark_case.case_id.as_str()) else {
            return Err(TassadarGeometricVariantReportError::MissingCase {
                case_id: benchmark_case.case_id.clone(),
            });
        };
        case_reports.push(build_hierarchical_hull_variant_case_report(
            benchmark_case,
            &article_case.program,
        )?);
    }

    let workload_targets = [
        TassadarWorkloadTarget::MicroWasmKernel,
        TassadarWorkloadTarget::BranchHeavyKernel,
        TassadarWorkloadTarget::MemoryHeavyKernel,
        TassadarWorkloadTarget::LongLoopKernel,
        TassadarWorkloadTarget::SudokuClass,
        TassadarWorkloadTarget::HungarianMatching,
    ];
    let variant_ids = [
        TASSADAR_RUNTIME_HULL_GEOMETRIC_VARIANT_ID,
        TASSADAR_HIERARCHICAL_HULL_GEOMETRIC_VARIANT_ID,
    ];
    let mut workload_summaries = Vec::new();
    for variant_id in variant_ids {
        for workload_target in workload_targets {
            if let Some(summary) =
                build_geometric_variant_workload_summary(&case_reports, variant_id, workload_target)
            {
                workload_summaries.push(summary);
            }
        }
    }

    let mut report = TassadarGeometricVariantReport {
        schema_version: TASSADAR_GEOMETRIC_VARIANT_REPORT_SCHEMA_VERSION,
        generated_from_artifacts: vec![String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF)],
        case_reports,
        workload_summaries,
        claim_boundary: String::from(
            "this report keeps the promoted runtime HullCache surface separate from a research-only hierarchical hull candidate. The candidate materially widens exact direct coverage on loop-heavy article workloads under the same artifact contract, but it remains research_only until decode-mode identity, refusal policy, and runtime closure bars are promoted explicitly",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_serialized_digest(b"tassadar_geometric_variant_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the geometric-variant comparison report.
#[must_use]
pub fn tassadar_geometric_variant_report_path() -> std::path::PathBuf {
    eval_repo_root().join(TASSADAR_GEOMETRIC_VARIANT_REPORT_REF)
}

/// Writes the canonical geometric-variant comparison report.
pub fn write_tassadar_geometric_variant_report(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarGeometricVariantReport, TassadarGeometricVariantReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_geometric_variant_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    std::fs::write(output_path, bytes)?;
    Ok(report)
}

fn stable_corpus_digest(artifacts: &[TassadarProgramArtifact]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"tassadar_corpus|");
    for artifact in artifacts {
        hasher.update(artifact.artifact_id.as_bytes());
        hasher.update(b"|");
        hasher.update(artifact.artifact_digest.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

fn throughput_steps_per_second(steps: u64, elapsed_seconds: f64) -> f64 {
    steps as f64 / elapsed_seconds.max(1e-9)
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn benchmark_runner_steps_per_second<F>(
    steps_per_run: u64,
    mut runner: F,
) -> Result<f64, TassadarBenchmarkError>
where
    F: FnMut() -> Result<psionic_runtime::TassadarExecution, TassadarExecutionRefusal>,
{
    let normalized_steps = steps_per_run.max(1);
    let target_steps = normalized_steps.saturating_mul(256).max(8_192);
    let minimum_runs = 16u64;
    let started = Instant::now();
    let mut run_count = 0u64;
    let mut total_steps = 0u64;

    loop {
        runner()?;
        run_count += 1;
        total_steps = total_steps.saturating_add(normalized_steps);
        let elapsed = started.elapsed().as_secs_f64();
        if run_count >= minimum_runs && (total_steps >= target_steps || elapsed >= 0.050) {
            return Ok(throughput_steps_per_second(total_steps, elapsed));
        }
    }
}

fn benchmark_scaling_runner_steps_per_second<F>(
    steps_per_run: u64,
    mut runner: F,
) -> Result<f64, TassadarBenchmarkError>
where
    F: FnMut() -> Result<psionic_runtime::TassadarExecution, TassadarExecutionRefusal>,
{
    let normalized_steps = steps_per_run.max(1);
    let target_steps = normalized_steps.saturating_mul(16).max(1_024);
    let minimum_runs = 1u64;
    let started = Instant::now();
    let mut run_count = 0u64;
    let mut total_steps = 0u64;

    loop {
        runner()?;
        run_count += 1;
        total_steps = total_steps.saturating_add(normalized_steps);
        let elapsed = started.elapsed().as_secs_f64();
        if run_count >= minimum_runs && (total_steps >= target_steps || elapsed >= 0.020) {
            return Ok(throughput_steps_per_second(total_steps, elapsed));
        }
    }
}

fn stable_outputs_digest(outputs: &[i32]) -> String {
    let bytes = serde_json::to_vec(outputs).unwrap_or_default();
    hex::encode(sha2::Sha256::digest(bytes))
}

fn stable_serialized_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn build_case_artifacts(
    version: &str,
    case_id: &str,
    artifact: &TassadarProgramArtifact,
    execution: &psionic_runtime::TassadarExecution,
    case: &psionic_runtime::TassadarValidationCase,
    evidence: &psionic_runtime::TassadarExecutionEvidenceBundle,
    selection: &psionic_runtime::TassadarExecutorSelectionDiagnostic,
) -> Result<Vec<EvalArtifact>, TassadarBenchmarkError> {
    Ok(vec![
        EvalArtifact::new(
            "tassadar_program_artifact.json",
            format!("artifact://tassadar/{version}/{case_id}/program"),
            &serde_json::to_vec(artifact).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_trace.json",
            format!("artifact://tassadar/{version}/{case_id}/trace"),
            &serde_json::to_vec(&execution.steps).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_expected_trace.json",
            format!("artifact://tassadar/{version}/{case_id}/expected_trace"),
            &serde_json::to_vec(&case.expected_trace).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_runtime_manifest.json",
            format!("artifact://tassadar/{version}/{case_id}/runtime_manifest"),
            &serde_json::to_vec(&evidence.runtime_manifest).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_trace_proof.json",
            format!("artifact://tassadar/{version}/{case_id}/trace_proof"),
            &serde_json::to_vec(&evidence.trace_proof).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_execution_proof_bundle.json",
            format!("artifact://tassadar/{version}/{case_id}/execution_proof_bundle"),
            &serde_json::to_vec(&evidence.proof_bundle).unwrap_or_default(),
        ),
        EvalArtifact::new(
            "tassadar_selection_diagnostic.json",
            format!("artifact://tassadar/{version}/{case_id}/selection_diagnostic"),
            &serde_json::to_vec(selection).unwrap_or_default(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn cached_decode_scaling_report() -> &'static TassadarDecodeScalingReport {
        static REPORT: OnceLock<TassadarDecodeScalingReport> = OnceLock::new();
        REPORT.get_or_init(|| {
            build_tassadar_decode_scaling_report().expect("decode-scaling report should build")
        })
    }

    fn cached_geometric_variant_report() -> &'static TassadarGeometricVariantReport {
        static REPORT: OnceLock<TassadarGeometricVariantReport> = OnceLock::new();
        REPORT.get_or_init(|| {
            build_tassadar_geometric_variant_report()
                .expect("geometric-variant report should build")
        })
    }

    fn normalized_geometric_variant_report_value(
        report: &TassadarGeometricVariantReport,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(report).expect("report should serialize");
        value["report_digest"] = Value::Null;
        if let Some(case_reports) = value["case_reports"].as_array_mut() {
            for case in case_reports {
                case["variant_steps_per_second"] = Value::Null;
                case["speedup_over_reference_linear"] = Value::Null;
                case["remaining_gap_vs_cpu_reference"] = Value::Null;
            }
        }
        if let Some(workload_summaries) = value["workload_summaries"].as_array_mut() {
            for summary in workload_summaries {
                summary["average_steps_per_second"] = Value::Null;
                summary["average_speedup_over_reference_linear"] = Value::Null;
                summary["average_remaining_gap_vs_cpu_reference"] = Value::Null;
            }
        }
        value
    }

    fn assert_decode_scaling_report_matches_stable_shape(
        expected: &TassadarDecodeScalingReport,
        actual: &TassadarDecodeScalingReport,
    ) {
        assert_eq!(actual.schema_version, expected.schema_version);
        assert_eq!(
            actual.generated_from_artifacts,
            expected.generated_from_artifacts
        );
        assert_eq!(actual.memory_growth_metric, expected.memory_growth_metric);
        assert_eq!(actual.claim_boundary, expected.claim_boundary);
        assert_eq!(actual.families.len(), expected.families.len());

        for (actual_family, expected_family) in actual.families.iter().zip(expected.families.iter())
        {
            assert_eq!(actual_family.family_id, expected_family.family_id);
            assert_eq!(actual_family.summary, expected_family.summary);
            assert_eq!(
                actual_family.length_parameter_name,
                expected_family.length_parameter_name
            );
            assert_eq!(actual_family.claim_boundary, expected_family.claim_boundary);
            assert_eq!(actual_family.regimes.len(), expected_family.regimes.len());

            for (actual_regime, expected_regime) in actual_family
                .regimes
                .iter()
                .zip(expected_family.regimes.iter())
            {
                assert_eq!(actual_regime.regime_id, expected_regime.regime_id);
                assert_eq!(actual_regime.summary, expected_regime.summary);
                assert_eq!(actual_regime.program_id, expected_regime.program_id);
                assert_eq!(actual_regime.profile_id, expected_regime.profile_id);
                assert_eq!(
                    actual_regime.length_parameter_name,
                    expected_regime.length_parameter_name
                );
                assert_eq!(
                    actual_regime.length_parameter_value,
                    expected_regime.length_parameter_value
                );
                assert_eq!(
                    actual_regime.instruction_count,
                    expected_regime.instruction_count
                );
                assert_eq!(
                    actual_regime.trace_step_count,
                    expected_regime.trace_step_count
                );
                assert_eq!(
                    actual_regime.trace_artifact_bytes,
                    expected_regime.trace_artifact_bytes
                );
                assert_eq!(
                    actual_regime.trace_artifact_bytes_per_step,
                    expected_regime.trace_artifact_bytes_per_step
                );
                assert_eq!(
                    actual_regime.max_stack_depth,
                    expected_regime.max_stack_depth
                );
                assert_eq!(
                    actual_regime.memory_slot_count,
                    expected_regime.memory_slot_count
                );
                assert_eq!(
                    actual_regime.hull_cache_selection_state,
                    expected_regime.hull_cache_selection_state
                );
                assert_eq!(
                    actual_regime.hull_cache_selection_reason,
                    expected_regime.hull_cache_selection_reason
                );
                assert_eq!(
                    actual_regime.hull_cache_effective_decode_mode,
                    expected_regime.hull_cache_effective_decode_mode
                );
                assert_eq!(
                    actual_regime.sparse_top_k_selection_state,
                    expected_regime.sparse_top_k_selection_state
                );
                assert_eq!(
                    actual_regime.sparse_top_k_selection_reason,
                    expected_regime.sparse_top_k_selection_reason
                );
                assert_eq!(
                    actual_regime.sparse_top_k_effective_decode_mode,
                    expected_regime.sparse_top_k_effective_decode_mode
                );
                assert_eq!(
                    actual_regime.trace_digest_equal,
                    expected_regime.trace_digest_equal
                );
                assert_eq!(actual_regime.outputs_equal, expected_regime.outputs_equal);
                assert_eq!(actual_regime.halt_equal, expected_regime.halt_equal);
                assert_eq!(actual_regime.exactness_bps, expected_regime.exactness_bps);
                assert_eq!(
                    actual_regime.cpu_behavior_digest,
                    expected_regime.cpu_behavior_digest
                );
                assert_eq!(
                    actual_regime.reference_linear_behavior_digest,
                    expected_regime.reference_linear_behavior_digest
                );
                assert_eq!(
                    actual_regime.hull_cache_behavior_digest,
                    expected_regime.hull_cache_behavior_digest
                );
                assert_eq!(
                    actual_regime.sparse_top_k_behavior_digest,
                    expected_regime.sparse_top_k_behavior_digest
                );
                assert!(actual_regime.cpu_reference_steps_per_second > 0.0);
                assert!(actual_regime.reference_linear_steps_per_second > 0.0);
                assert!(actual_regime.hull_cache_steps_per_second > 0.0);
                assert!(actual_regime.sparse_top_k_steps_per_second > 0.0);
                assert!(actual_regime.hull_cache_speedup_over_reference_linear >= 1.0);
                assert!(actual_regime.sparse_top_k_speedup_over_reference_linear >= 1.0);
                assert!(actual_regime.hull_cache_remaining_gap_vs_cpu_reference >= 1.0);
                assert!(actual_regime.sparse_top_k_remaining_gap_vs_cpu_reference >= 1.0);
            }
        }
    }

    #[test]
    fn tassadar_reference_fixture_suite_builds_package_and_environment_contracts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let suite = build_tassadar_reference_fixture_suite("2026.03.15")?;
        assert_eq!(suite.artifacts.len(), 4);
        assert_eq!(suite.benchmark_package.cases.len(), 4);
        assert_eq!(
            suite
                .benchmark_package
                .metadata
                .get("tassadar.hull_cache_metric_id")
                .and_then(Value::as_str),
            Some(TASSADAR_HULL_CACHE_METRIC_ID)
        );
        assert_eq!(
            suite
                .benchmark_package
                .metadata
                .get("tassadar.sparse_top_k_metric_id")
                .and_then(Value::as_str),
            Some(TASSADAR_SPARSE_TOP_K_METRIC_ID)
        );
        assert_eq!(
            suite.environment_bundle.current_workload_targets,
            vec![
                TassadarWorkloadTarget::ArithmeticMicroprogram,
                TassadarWorkloadTarget::ClrsShortestPath,
                TassadarWorkloadTarget::MemoryLookupMicroprogram,
                TassadarWorkloadTarget::BranchControlFlowMicroprogram,
            ]
        );
        assert_eq!(
            suite
                .environment_bundle
                .benchmark_package_set_binding
                .supported_families,
            vec![
                TassadarBenchmarkFamily::Arithmetic,
                TassadarBenchmarkFamily::ClrsSubset,
            ]
        );
        Ok(())
    }

    #[test]
    fn tassadar_reference_fixture_benchmark_is_exact_on_current_validation_corpus(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = run_tassadar_reference_fixture_benchmark("2026.03.15")?;
        assert_eq!(report.aggregate_summary.round_count, 1);
        assert_eq!(report.aggregate_summary.aggregate_score_bps, Some(10_000));
        assert_eq!(report.aggregate_summary.aggregate_pass_rate_bps, 10_000);
        assert_eq!(report.eval_run.status, crate::EvalRunStatus::Finalized);
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.status == EvalSampleStatus::Passed));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.cpu_reference_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.reference_linear_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.hull_cache_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.sparse_top_k_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.hull_cache_speedup_over_reference_linear > 1.0));
        assert!(report.case_reports.iter().all(|case| {
            case.sparse_top_k_selection_state == TassadarExecutorSelectionState::Direct
                && case.sparse_top_k_selection_reason.is_none()
                && !case.sparse_top_k_used_decode_fallback
                && case.sparse_top_k_speedup_over_reference_linear > 0.0
        }));
        assert!(report.case_reports.iter().all(|case| {
            case.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
                && case.effective_decode_mode == TassadarExecutorDecodeMode::HullCache
                && case.selection_state == TassadarExecutorSelectionState::Direct
                && case.selection_reason.is_none()
                && !case.used_decode_fallback
        }));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.trace_digest_equal));
        assert!(report.case_reports.iter().all(|case| case.outputs_equal));
        assert!(report.case_reports.iter().all(|case| case.halt_equal));
        assert!(report.eval_run.samples.iter().all(|sample| {
            sample
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_kind == "tassadar_trace_proof.json")
        }));
        assert!(report.eval_run.samples.iter().all(|sample| {
            sample
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_kind == "tassadar_runtime_manifest.json")
        }));
        assert!(report.eval_run.samples.iter().all(|sample| {
            sample
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_kind == "tassadar_execution_proof_bundle.json")
        }));
        assert!(report.eval_run.samples.iter().all(|sample| {
            sample
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_kind == "tassadar_selection_diagnostic.json")
        }));
        assert!(report
            .eval_run
            .run_artifacts
            .iter()
            .any(|artifact| { artifact.artifact_kind == "tassadar_runtime_capability.json" }));
        Ok(())
    }

    #[test]
    fn tassadar_article_class_suite_builds_package_and_environment_contracts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let suite = build_tassadar_article_class_suite("2026.03.16")?;
        let sudoku_v0_corpus = tassadar_sudoku_v0_corpus();
        assert_eq!(suite.artifacts.len(), sudoku_v0_corpus.len() + 5);
        assert_eq!(
            suite.benchmark_package.cases.len(),
            sudoku_v0_corpus.len() + 5
        );
        assert_eq!(
            suite.environment_bundle.current_workload_targets,
            vec![
                TassadarWorkloadTarget::MicroWasmKernel,
                TassadarWorkloadTarget::BranchHeavyKernel,
                TassadarWorkloadTarget::MemoryHeavyKernel,
                TassadarWorkloadTarget::LongLoopKernel,
                TassadarWorkloadTarget::SudokuClass,
                TassadarWorkloadTarget::HungarianMatching,
            ]
        );
        assert!(
            suite.environment_bundle.planned_workload_targets.is_empty(),
            "article-class targets should be current rather than planned"
        );
        assert_eq!(
            suite.environment_bundle.program_binding.wasm_profile_id,
            TassadarWasmProfile::article_i32_compute_v1().profile_id
        );
        assert_eq!(
            suite.benchmark_package.metadata["tassadar.sudoku_v0_train_case_ids"]
                .as_array()
                .map_or(0, Vec::len),
            4
        );
        assert_eq!(
            suite.benchmark_package.metadata["tassadar.sudoku_v0_validation_case_ids"]
                .as_array()
                .map_or(0, Vec::len),
            2
        );
        assert_eq!(
            suite.benchmark_package.metadata["tassadar.sudoku_v0_test_case_ids"]
                .as_array()
                .map_or(0, Vec::len),
            2
        );
        Ok(())
    }

    #[test]
    fn tassadar_article_class_benchmark_is_exact_on_widened_corpus(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = run_tassadar_article_class_benchmark("2026.03.16")?;
        assert_eq!(report.aggregate_summary.round_count, 1);
        assert_eq!(report.aggregate_summary.aggregate_score_bps, Some(10_000));
        assert_eq!(report.aggregate_summary.aggregate_pass_rate_bps, 10_000);
        assert_eq!(report.eval_run.status, crate::EvalRunStatus::Finalized);
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.status == EvalSampleStatus::Passed));
        assert_eq!(
            report
                .case_reports
                .iter()
                .filter(|case| case.workload_target == TassadarWorkloadTarget::SudokuClass)
                .count(),
            8
        );
        assert_eq!(
            report
                .case_reports
                .iter()
                .filter(|case| case.workload_target == TassadarWorkloadTarget::MicroWasmKernel)
                .count(),
            1
        );
        assert_eq!(
            report
                .case_reports
                .iter()
                .filter(|case| case.workload_target == TassadarWorkloadTarget::BranchHeavyKernel)
                .count(),
            1
        );
        assert_eq!(
            report
                .case_reports
                .iter()
                .filter(|case| case.workload_target == TassadarWorkloadTarget::MemoryHeavyKernel)
                .count(),
            1
        );
        assert_eq!(
            report
                .case_reports
                .iter()
                .filter(|case| case.workload_target == TassadarWorkloadTarget::LongLoopKernel)
                .count(),
            1
        );
        assert_eq!(
            report
                .case_reports
                .iter()
                .filter(|case| case.workload_target == TassadarWorkloadTarget::HungarianMatching)
                .count(),
            1
        );
        for case in &report.case_reports {
            assert_eq!(
                case.requested_decode_mode,
                TassadarExecutorDecodeMode::HullCache
            );
            match case.workload_target {
                TassadarWorkloadTarget::SudokuClass => {
                    assert_eq!(
                        case.effective_decode_mode,
                        TassadarExecutorDecodeMode::ReferenceLinear
                    );
                    assert_eq!(
                        case.selection_state,
                        TassadarExecutorSelectionState::Fallback
                    );
                    assert_eq!(
                        case.selection_reason,
                        Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
                    );
                    assert!(case.used_decode_fallback);
                    assert_eq!(
                        case.sparse_top_k_selection_state,
                        TassadarExecutorSelectionState::Fallback
                    );
                    assert_eq!(
                        case.sparse_top_k_selection_reason,
                        Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
                    );
                    assert!(case.sparse_top_k_used_decode_fallback);
                }
                TassadarWorkloadTarget::LongLoopKernel => {
                    assert_eq!(
                        case.effective_decode_mode,
                        TassadarExecutorDecodeMode::ReferenceLinear
                    );
                    assert_eq!(
                        case.selection_state,
                        TassadarExecutorSelectionState::Fallback
                    );
                    assert_eq!(
                        case.selection_reason,
                        Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
                    );
                    assert!(case.used_decode_fallback);
                    assert_eq!(
                        case.sparse_top_k_selection_state,
                        TassadarExecutorSelectionState::Fallback
                    );
                    assert_eq!(
                        case.sparse_top_k_selection_reason,
                        Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
                    );
                    assert!(case.sparse_top_k_used_decode_fallback);
                }
                TassadarWorkloadTarget::BranchHeavyKernel => {
                    assert_eq!(
                        case.effective_decode_mode,
                        TassadarExecutorDecodeMode::HullCache
                    );
                    assert_eq!(case.selection_state, TassadarExecutorSelectionState::Direct);
                    assert_eq!(case.selection_reason, None);
                    assert!(!case.used_decode_fallback);
                    assert_eq!(
                        case.sparse_top_k_selection_state,
                        TassadarExecutorSelectionState::Fallback
                    );
                    assert_eq!(
                        case.sparse_top_k_selection_reason,
                        Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
                    );
                    assert!(case.sparse_top_k_used_decode_fallback);
                }
                TassadarWorkloadTarget::MicroWasmKernel
                | TassadarWorkloadTarget::MemoryHeavyKernel
                | TassadarWorkloadTarget::HungarianMatching => {
                    assert_eq!(
                        case.effective_decode_mode,
                        TassadarExecutorDecodeMode::HullCache
                    );
                    assert_eq!(case.selection_state, TassadarExecutorSelectionState::Direct);
                    assert_eq!(case.selection_reason, None);
                    assert!(!case.used_decode_fallback);
                    assert_eq!(
                        case.sparse_top_k_selection_state,
                        TassadarExecutorSelectionState::Direct
                    );
                    assert_eq!(case.sparse_top_k_selection_reason, None);
                    assert!(!case.sparse_top_k_used_decode_fallback);
                }
                _ => {}
            }
        }
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.trace_digest_equal));
        assert!(report.case_reports.iter().all(|case| case.outputs_equal));
        assert!(report.case_reports.iter().all(|case| case.halt_equal));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.cpu_reference_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.reference_linear_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.hull_cache_steps_per_second > 0.0));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.sparse_top_k_steps_per_second > 0.0));
        Ok(())
    }

    #[test]
    fn tassadar_workload_capability_matrix_maps_current_families(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_workload_capability_matrix_report()?;
        assert_eq!(
            report.schema_version,
            TASSADAR_WORKLOAD_CAPABILITY_MATRIX_SCHEMA_VERSION
        );
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "micro_wasm_kernel"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "runtime.hull_cache"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "branch_heavy_kernel"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "runtime.sparse_top_k"
                        && cell.posture == TassadarCapabilityPosture::FallbackOnly
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "arithmetic_kernel"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "compiled.proof_backed"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "long_loop_kernel"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "compiled.proof_backed"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "sudoku_class_4x4"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "compiled.proof_backed"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "learned.bounded"
                        && cell.posture == TassadarCapabilityPosture::Partial
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "sudoku_search_9x9"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "compiled.proof_backed"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "learned.long_horizon"
                        && cell.posture == TassadarCapabilityPosture::Partial
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "hungarian_matching_4x4"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "compiled.proof_backed"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
        }));
        assert!(report.rows.iter().any(|row| {
            row.workload_family_id == "hungarian_matching_10x10"
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "compiled.proof_backed"
                        && cell.posture == TassadarCapabilityPosture::Exact
                })
                && row.capabilities.iter().any(|cell| {
                    cell.surface_id == "learned.long_horizon"
                        && cell.posture == TassadarCapabilityPosture::NotLanded
                })
        }));
        Ok(())
    }

    #[test]
    fn tassadar_workload_capability_matrix_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_workload_capability_matrix_report()?;
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF),
        )?;
        let persisted: TassadarWorkloadCapabilityMatrixReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn write_tassadar_workload_capability_matrix_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-eval-capability-matrix-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let report_path = temp_dir.join("tassadar_workload_capability_matrix.json");
        let report = write_tassadar_workload_capability_matrix_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarWorkloadCapabilityMatrixReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        std::fs::remove_file(&report_path)?;
        std::fs::remove_dir(&temp_dir)?;
        Ok(())
    }

    #[test]
    fn tassadar_hull_cache_closure_report_tracks_widened_direct_subset(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_hull_cache_closure_report()?;
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::MicroWasmKernel
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::BranchHeavyKernel
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::MemoryHeavyKernel
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::HungarianMatching
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::LongLoopKernel
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::SudokuClass
                && row.direct_case_count == 8
                && row.fallback_case_count == 0
        }));
        assert!(!report
            .fallback_only_workloads
            .iter()
            .any(|row| { row.workload_target == TassadarWorkloadTarget::LongLoopKernel }));
        assert!(!report
            .fallback_only_workloads
            .iter()
            .any(|row| { row.workload_target == TassadarWorkloadTarget::SudokuClass }));
        Ok(())
    }

    #[test]
    fn tassadar_hull_cache_closure_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_hull_cache_closure_report()?;
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF),
        )?;
        let persisted: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, serde_json::to_value(&report)?);
        Ok(())
    }

    #[test]
    fn write_tassadar_hull_cache_closure_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-eval-hull-cache-closure-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let report_path = temp_dir.join("tassadar_hull_cache_closure_report.json");
        let report = write_tassadar_hull_cache_closure_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, serde_json::to_value(&report)?);
        std::fs::remove_file(&report_path)?;
        std::fs::remove_dir(&temp_dir)?;
        Ok(())
    }

    #[test]
    fn tassadar_sparse_top_k_comparison_report_tracks_direct_and_fallback_families(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_sparse_top_k_comparison_report()?;
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::MicroWasmKernel
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::MemoryHeavyKernel
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.exact_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::HungarianMatching
                && row.direct_case_count == 1
                && row.fallback_case_count == 0
        }));
        assert!(report.fallback_only_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::BranchHeavyKernel
                && row.fallback_case_count == 1
        }));
        assert!(report.fallback_only_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::LongLoopKernel
                && row.fallback_case_count == 1
        }));
        assert!(report.fallback_only_workloads.iter().any(|row| {
            row.workload_target == TassadarWorkloadTarget::SudokuClass
                && row.fallback_case_count == 8
        }));
        Ok(())
    }

    #[test]
    fn tassadar_sparse_top_k_comparison_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_sparse_top_k_comparison_report()?;
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(TASSADAR_SPARSE_TOP_K_COMPARISON_REPORT_REF),
        )?;
        let persisted: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, serde_json::to_value(&report)?);
        Ok(())
    }

    #[test]
    fn write_tassadar_sparse_top_k_comparison_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-eval-sparse-top-k-comparison-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let report_path = temp_dir.join("tassadar_sparse_top_k_comparison_report.json");
        let report = write_tassadar_sparse_top_k_comparison_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, serde_json::to_value(&report)?);
        std::fs::remove_file(&report_path)?;
        std::fs::remove_dir(&temp_dir)?;
        Ok(())
    }

    #[test]
    fn tassadar_geometric_variant_report_tracks_runtime_and_research_boundaries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = cached_geometric_variant_report();
        assert_eq!(
            report.schema_version,
            TASSADAR_GEOMETRIC_VARIANT_REPORT_SCHEMA_VERSION
        );
        assert!(report.case_reports.iter().any(|case| {
            case.variant_id == TASSADAR_RUNTIME_HULL_GEOMETRIC_VARIANT_ID
                && case.workload_target == TassadarWorkloadTarget::LongLoopKernel
                && case.claim_boundary == TassadarGeometricVariantClaimBoundary::RuntimeReady
                && case.selection_state == TassadarExecutorSelectionState::Fallback
                && case.selection_reason
                    == Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
                && case.exactness_bps == 10_000
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.variant_id == TASSADAR_HIERARCHICAL_HULL_GEOMETRIC_VARIANT_ID
                && case.workload_target == TassadarWorkloadTarget::LongLoopKernel
                && case.claim_boundary == TassadarGeometricVariantClaimBoundary::ResearchOnly
                && case.selection_state == TassadarExecutorSelectionState::Direct
                && case.exactness_bps == 10_000
                && case.trace_digest_equal
        }));
        assert!(report.workload_summaries.iter().any(|summary| {
            summary.variant_id == TASSADAR_RUNTIME_HULL_GEOMETRIC_VARIANT_ID
                && summary.workload_target == TassadarWorkloadTarget::SudokuClass
                && summary.fallback_case_count == 8
                && summary.direct_case_count == 0
        }));
        assert!(report.workload_summaries.iter().any(|summary| {
            summary.variant_id == TASSADAR_HIERARCHICAL_HULL_GEOMETRIC_VARIANT_ID
                && summary.workload_target == TassadarWorkloadTarget::SudokuClass
                && summary.direct_case_count == 8
                && summary.fallback_case_count == 0
                && summary.exact_case_count == 8
        }));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.variant_steps_per_second > 0.0));
        Ok(())
    }

    #[test]
    fn tassadar_geometric_variant_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(TASSADAR_GEOMETRIC_VARIANT_REPORT_REF),
        )?;
        let persisted: TassadarGeometricVariantReport = serde_json::from_slice(&bytes)?;
        assert!(persisted
            .case_reports
            .iter()
            .all(|case| case.variant_steps_per_second > 0.0));
        assert_eq!(
            normalized_geometric_variant_report_value(cached_geometric_variant_report()),
            normalized_geometric_variant_report_value(&persisted)
        );
        Ok(())
    }

    #[test]
    fn write_tassadar_geometric_variant_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-eval-geometric-variants-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let report_path = temp_dir.join("tassadar_geometric_variant_report.json");
        let report = write_tassadar_geometric_variant_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarGeometricVariantReport = serde_json::from_slice(&bytes)?;
        assert!(persisted
            .case_reports
            .iter()
            .all(|case| case.variant_steps_per_second > 0.0));
        assert_eq!(
            normalized_geometric_variant_report_value(&report),
            normalized_geometric_variant_report_value(&persisted)
        );
        std::fs::remove_file(&report_path)?;
        std::fs::remove_dir(&temp_dir)?;
        Ok(())
    }

    #[test]
    fn tassadar_decode_scaling_report_tracks_direct_and_fallback_regimes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = cached_decode_scaling_report();
        assert_eq!(
            report.schema_version,
            TASSADAR_DECODE_SCALING_SCHEMA_VERSION
        );

        let linear_family = report
            .families
            .iter()
            .find(|family| family.family_id == "linear_memory_accumulator")
            .expect("linear accumulator family");
        assert!(linear_family.regimes.windows(2).all(|window| {
            window[0].trace_step_count < window[1].trace_step_count
                && window[0].trace_artifact_bytes < window[1].trace_artifact_bytes
        }));
        assert!(linear_family.regimes.iter().all(|regime| {
            regime.hull_cache_selection_state == TassadarExecutorSelectionState::Direct
                && regime.hull_cache_effective_decode_mode == TassadarExecutorDecodeMode::HullCache
                && regime.sparse_top_k_selection_state == TassadarExecutorSelectionState::Direct
                && regime.sparse_top_k_effective_decode_mode
                    == TassadarExecutorDecodeMode::SparseTopK
                && regime.exactness_bps == 10_000
        }));

        let branch_family = report
            .families
            .iter()
            .find(|family| family.family_id == "forward_branch_ladder")
            .expect("forward branch family");
        let last_branch_regime = branch_family.regimes.last().expect("branch regime");
        assert_eq!(
            last_branch_regime.hull_cache_selection_state,
            TassadarExecutorSelectionState::Direct
        );
        assert_eq!(
            last_branch_regime.hull_cache_effective_decode_mode,
            TassadarExecutorDecodeMode::HullCache
        );
        assert_eq!(
            last_branch_regime.sparse_top_k_selection_state,
            TassadarExecutorSelectionState::Fallback
        );
        assert_eq!(
            last_branch_regime.sparse_top_k_selection_reason,
            Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
        );
        assert_eq!(
            last_branch_regime.sparse_top_k_effective_decode_mode,
            TassadarExecutorDecodeMode::ReferenceLinear
        );

        let loop_family = report
            .families
            .iter()
            .find(|family| family.family_id == "backward_branch_loop")
            .expect("backward loop family");
        assert!(loop_family.regimes.iter().all(|regime| {
            regime.hull_cache_selection_state == TassadarExecutorSelectionState::Fallback
                && regime.hull_cache_selection_reason
                    == Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
                && regime.hull_cache_effective_decode_mode
                    == TassadarExecutorDecodeMode::ReferenceLinear
                && regime.sparse_top_k_selection_state == TassadarExecutorSelectionState::Fallback
                && regime.sparse_top_k_selection_reason
                    == Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
                && regime.sparse_top_k_effective_decode_mode
                    == TassadarExecutorDecodeMode::ReferenceLinear
                && regime.exactness_bps == 10_000
        }));
        assert_eq!(
            loop_family
                .regimes
                .last()
                .expect("largest loop regime")
                .trace_step_count,
            16_383
        );
        Ok(())
    }

    #[test]
    fn tassadar_decode_scaling_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(TASSADAR_DECODE_SCALING_REPORT_REF),
        )?;
        let persisted: TassadarDecodeScalingReport = serde_json::from_slice(&bytes)?;
        assert_decode_scaling_report_matches_stable_shape(
            cached_decode_scaling_report(),
            &persisted,
        );
        Ok(())
    }

    #[test]
    fn write_tassadar_decode_scaling_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-eval-decode-scaling-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let report_path = temp_dir.join("tassadar_decode_scaling_report.json");
        let report = write_tassadar_decode_scaling_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarDecodeScalingReport = serde_json::from_slice(&bytes)?;
        assert_decode_scaling_report_matches_stable_shape(&report, &persisted);
        std::fs::remove_file(&report_path)?;
        std::fs::remove_dir(&temp_dir)?;
        Ok(())
    }
}
