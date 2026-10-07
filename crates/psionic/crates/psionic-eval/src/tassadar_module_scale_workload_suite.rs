use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use psionic_compiler::{
    TassadarWasmModuleArtifactBundle, TassadarWasmTextArtifactBundlePipelineError,
    TassadarWasmTextCompileConfig, compile_tassadar_wasm_text_to_artifact_bundle,
};
use psionic_data::{
    DatasetKey, TassadarBenchmarkAxis, TassadarBenchmarkFamily,
    TassadarModuleScaleWorkloadCaseContract, TassadarModuleScaleWorkloadFamily,
    TassadarModuleScaleWorkloadStatus, TassadarModuleScaleWorkloadSuiteContract,
    TassadarModuleScaleWorkloadSuiteError as DataModuleScaleWorkloadSuiteError,
};
use psionic_environments::{
    EnvironmentDatasetBinding, EnvironmentPolicyKind, EnvironmentPolicyReference,
    TassadarBenchmarkPackageSetBinding, TassadarCompilePipelineMatrixBinding,
    TassadarEnvironmentBundle, TassadarEnvironmentError, TassadarEnvironmentSpec,
    TassadarExactnessContract, TassadarIoContract, TassadarModuleScaleWorkloadSuiteBinding,
    TassadarProgramBinding, TassadarWasmConformanceBinding, TassadarWorkloadTarget,
};
use psionic_ir::parse_tassadar_normalized_wasm_module;
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarProgramSourceKind,
    TassadarTraceAbi, TassadarWasmBinarySummary, TassadarWasmProfile,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF,
    TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF, TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
    TASSADAR_WASM_CONFORMANCE_REPORT_REF,
};

const TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json";
pub const TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REF: &str =
    "benchmark-suite://openagents/tassadar/module_scale";
pub const TASSADAR_MODULE_SCALE_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.module_scale.eval";
pub const TASSADAR_MODULE_SCALE_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.module_scale.benchmark";
pub const TASSADAR_MODULE_SCALE_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/module_scale/reference_fixture";
pub const TASSADAR_MODULE_SCALE_DATASET_REF: &str =
    "dataset://openagents/tassadar/module_scale_suite";

const MODULE_SCALE_AXIS_EXACTNESS_BPS: &str = "exactness_bps";
const MODULE_SCALE_AXIS_TOTAL_TRACE_STEPS: &str = "total_trace_steps";
const MODULE_SCALE_AXIS_CPU_REFERENCE_COST_UNITS: &str = "cpu_reference_cost_units";
const MODULE_SCALE_AXIS_REFUSAL_KIND: &str = "refusal_kind";

static MODULE_SCALE_WORKLOAD_SUITE_BUILD_LOCK: LazyLock<Mutex<()>> =
    LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ModuleScaleCaseSpec {
    case_id: &'static str,
    family: TassadarModuleScaleWorkloadFamily,
    workload_target: TassadarWorkloadTarget,
    summary: &'static str,
    source_ref: &'static str,
    source_file_name: &'static str,
    wasm_file_name: &'static str,
    export_symbols: &'static [&'static str],
    expected_status: TassadarModuleScaleWorkloadStatus,
}

/// Repo-facing outcome for one module-scale suite case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarModuleScaleWorkloadCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable workload family.
    pub family: TassadarModuleScaleWorkloadFamily,
    /// Stable workload target used by the environment and benchmark package.
    pub workload_target: TassadarWorkloadTarget,
    /// Human-readable case summary.
    pub summary: String,
    /// Repo-relative source ref.
    pub source_ref: String,
    /// Actual status observed under the current bounded lane.
    pub status: TassadarModuleScaleWorkloadStatus,
    /// Stable source kind for the suite.
    pub source_kind: TassadarProgramSourceKind,
    /// Stable source digest.
    pub source_digest: String,
    /// Stable compile-config digest.
    pub compile_config_digest: String,
    /// Stable compile receipt digest.
    pub compile_receipt_digest: String,
    /// Stable compiler toolchain digest.
    pub toolchain_digest: String,
    /// Repo-relative Wasm output ref when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_ref: Option<String>,
    /// Stable Wasm output digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_digest: Option<String>,
    /// Structural summary over the compiled Wasm output when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_summary: Option<TassadarWasmBinarySummary>,
    /// Stable digest over the normalized module when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_module_digest: Option<String>,
    /// Stable artifact bundle digest when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_bundle_digest: Option<String>,
    /// Ordered lowered export names when lowering succeeded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lowered_export_names: Vec<String>,
    /// Lowered artifact digests when lowering succeeded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lowered_artifact_digests: Vec<String>,
    /// Exact outputs by export when lowering succeeded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exact_outputs_by_export: BTreeMap<String, Vec<i32>>,
    /// Total trace steps across lowered exports when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_trace_steps: Option<u64>,
    /// Maximum trace steps over one lowered export.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_trace_steps: Option<u64>,
    /// Exactness score in basis points when lowering succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exactness_bps: Option<u32>,
    /// Deterministic CPU-reference replay cost units over the lowered export set when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_reference_cost_units: Option<u64>,
    /// Lowering refusal kind when the case refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    /// Lowering refusal detail when the case refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

/// Committed report over the public module-scale Wasm workload suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarModuleScaleWorkloadSuiteReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public suite contract.
    pub suite: TassadarModuleScaleWorkloadSuiteContract,
    /// Shared environment bundle bound to the suite.
    pub environment_bundle: TassadarEnvironmentBundle,
    /// Benchmark package bound to the suite.
    pub benchmark_package: BenchmarkPackage,
    /// Ordered case outcomes.
    pub cases: Vec<TassadarModuleScaleWorkloadCaseReport>,
    /// Stable refs used to generate the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarModuleScaleWorkloadSuiteReport {
    fn new(
        suite: TassadarModuleScaleWorkloadSuiteContract,
        environment_bundle: TassadarEnvironmentBundle,
        benchmark_package: BenchmarkPackage,
        cases: Vec<TassadarModuleScaleWorkloadCaseReport>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.module_scale_workload_suite.report.v1"),
            suite,
            environment_bundle,
            benchmark_package,
            cases,
            generated_from_refs,
            claim_boundary: String::from(
                "this report freezes one public module-scale Wasm workload suite over deterministic fixed-span memcpy, parsing, checksum, and VM-style module fixtures plus one explicit parameter-ABI refusal case. Exactness, trace-length, cost, and refusal remain explicit per source module under the current bounded Wasm-text-to-Tassadar lowering lane; this is not arbitrary Wasm closure or a served capability claim",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_module_scale_workload_suite_report|",
            &report,
        );
        report
    }
}

/// Report build failures for the module-scale workload suite.
#[derive(Debug, Error)]
pub enum TassadarModuleScaleWorkloadSuiteReportError {
    /// The suite contract was invalid.
    #[error(transparent)]
    Contract(#[from] DataModuleScaleWorkloadSuiteError),
    /// The environment bundle was invalid.
    #[error(transparent)]
    Environment(#[from] TassadarEnvironmentError),
    /// Failed to read one source fixture.
    #[error("failed to read source fixture `{path}`: {error}")]
    ReadSource { path: String, error: std::io::Error },
    /// Failed to read one compiled Wasm output.
    #[error("failed to read compiled Wasm for case `{case_id}` at `{path}`: {error}")]
    ReadCompiledWasm {
        case_id: String,
        path: String,
        error: std::io::Error,
    },
    /// Failed to normalize one compiled Wasm output.
    #[error("failed to normalize compiled Wasm for case `{case_id}`: {error}")]
    NormalizeCompiledWasm {
        case_id: String,
        error: psionic_ir::TassadarNormalizedWasmModuleError,
    },
    /// Lowered export replay failed unexpectedly.
    #[error("failed to replay lowered export `{export_name}` for case `{case_id}`: {error}")]
    Replay {
        case_id: String,
        export_name: String,
        error: TassadarExecutionRefusal,
    },
    /// Lowered export diverged from its execution manifest.
    #[error(
        "lowered export `{export_name}` for case `{case_id}` diverged from its execution manifest"
    )]
    ExactnessMismatch {
        case_id: String,
        export_name: String,
    },
    /// The observed pipeline outcome did not match the declared suite contract.
    #[error(
        "module-scale workload case `{case_id}` expected `{expected_status:?}` but observed `{actual_status:?}`"
    )]
    UnexpectedStatus {
        case_id: String,
        expected_status: TassadarModuleScaleWorkloadStatus,
        actual_status: TassadarModuleScaleWorkloadStatus,
    },
    /// Benchmark package validation failed.
    #[error(transparent)]
    Benchmark(#[from] crate::EvalRuntimeError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write module-scale workload suite report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Filesystem read failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed report for the public module-scale Wasm workload suite.
pub fn build_tassadar_module_scale_workload_suite_report()
-> Result<TassadarModuleScaleWorkloadSuiteReport, TassadarModuleScaleWorkloadSuiteReportError> {
    let _build_guard = MODULE_SCALE_WORKLOAD_SUITE_BUILD_LOCK
        .lock()
        .expect("module-scale workload suite build lock should not be poisoned");
    build_tassadar_module_scale_workload_suite_report_impl()
}

fn build_tassadar_module_scale_workload_suite_report_impl()
-> Result<TassadarModuleScaleWorkloadSuiteReport, TassadarModuleScaleWorkloadSuiteReportError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let specs = module_scale_case_specs();
    let cases = specs
        .iter()
        .map(|spec| evaluate_module_scale_case(spec, &profile))
        .collect::<Result<Vec<_>, _>>()?;
    let suite = build_tassadar_module_scale_workload_suite_contract("2026.03.17")?;
    let artifact_digests = cases
        .iter()
        .flat_map(|case| case.lowered_artifact_digests.iter().cloned())
        .collect::<Vec<_>>();
    let corpus_digest = stable_digest(
        b"psionic_tassadar_module_scale_workload_suite_corpus|",
        &(artifact_digests.clone(), suite.cases.clone()),
    );
    let environment_bundle = build_tassadar_module_scale_environment_bundle(
        "2026.03.17",
        artifact_digests,
        &corpus_digest,
        &profile,
        &trace_abi,
    )?;
    let benchmark_package =
        build_tassadar_module_scale_benchmark_package("2026.03.17", &environment_bundle, &cases)?;
    Ok(TassadarModuleScaleWorkloadSuiteReport::new(
        suite,
        environment_bundle,
        benchmark_package,
        cases,
        module_scale_case_specs()
            .iter()
            .map(|spec| String::from(spec.source_ref))
            .collect(),
    ))
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_module_scale_workload_suite_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF)
}

/// Writes the committed report for the public module-scale Wasm workload suite.
pub fn write_tassadar_module_scale_workload_suite_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleScaleWorkloadSuiteReport, TassadarModuleScaleWorkloadSuiteReportError> {
    let _build_guard = MODULE_SCALE_WORKLOAD_SUITE_BUILD_LOCK
        .lock()
        .expect("module-scale workload suite build lock should not be poisoned");
    write_tassadar_module_scale_workload_suite_report_impl(output_path)
}

fn write_tassadar_module_scale_workload_suite_report_impl(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleScaleWorkloadSuiteReport, TassadarModuleScaleWorkloadSuiteReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleScaleWorkloadSuiteReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_scale_workload_suite_report_impl()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("module-scale workload suite report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleScaleWorkloadSuiteReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn module_scale_case_specs() -> Vec<ModuleScaleCaseSpec> {
    vec![
        ModuleScaleCaseSpec {
            case_id: "memcpy_fixed_span_exact",
            family: TassadarModuleScaleWorkloadFamily::Memcpy,
            workload_target: TassadarWorkloadTarget::ModuleMemcpy,
            summary: "fixed-span memcpy-style module with explicit load/store replay",
            source_ref: "fixtures/tassadar/sources/tassadar_module_memcpy_suite.wat",
            source_file_name: "tassadar_module_memcpy_suite.wat",
            wasm_file_name: "tassadar_module_memcpy_suite.wasm",
            export_symbols: &["copy_sum", "copy_tail"],
            expected_status: TassadarModuleScaleWorkloadStatus::LoweredExact,
        },
        ModuleScaleCaseSpec {
            case_id: "checksum_fixed_span_exact",
            family: TassadarModuleScaleWorkloadFamily::Checksum,
            workload_target: TassadarWorkloadTarget::ModuleChecksum,
            summary: "fixed-span checksum-style module with additive and weighted exports",
            source_ref: "fixtures/tassadar/sources/tassadar_module_checksum_suite.wat",
            source_file_name: "tassadar_module_checksum_suite.wat",
            wasm_file_name: "tassadar_module_checksum_suite.wasm",
            export_symbols: &["checksum_sum", "checksum_weighted"],
            expected_status: TassadarModuleScaleWorkloadStatus::LoweredExact,
        },
        ModuleScaleCaseSpec {
            case_id: "parsing_token_triplet_exact",
            family: TassadarModuleScaleWorkloadFamily::Parsing,
            workload_target: TassadarWorkloadTarget::ModuleParsing,
            summary: "fixed-token parse module that reconstructs one three-token value exactly",
            source_ref: "fixtures/tassadar/sources/tassadar_module_parsing_suite.wat",
            source_file_name: "tassadar_module_parsing_suite.wat",
            wasm_file_name: "tassadar_module_parsing_suite.wasm",
            export_symbols: &["parse_triplet", "parse_gap"],
            expected_status: TassadarModuleScaleWorkloadStatus::LoweredExact,
        },
        ModuleScaleCaseSpec {
            case_id: "vm_style_dispatch_exact",
            family: TassadarModuleScaleWorkloadFamily::VmStyle,
            workload_target: TassadarWorkloadTarget::ModuleVmStyle,
            summary: "multi-export VM-style dispatch module over deterministic fixed operands",
            source_ref: "fixtures/tassadar/sources/tassadar_module_vm_style_suite.wat",
            source_file_name: "tassadar_module_vm_style_suite.wat",
            wasm_file_name: "tassadar_module_vm_style_suite.wasm",
            export_symbols: &["dispatch_add", "dispatch_mul_add"],
            expected_status: TassadarModuleScaleWorkloadStatus::LoweredExact,
        },
        ModuleScaleCaseSpec {
            case_id: "vm_style_param_refusal",
            family: TassadarModuleScaleWorkloadFamily::VmStyle,
            workload_target: TassadarWorkloadTarget::ModuleVmStyle,
            summary: "parameter-ABI VM-style module that still refuses lowering explicitly",
            source_ref: "fixtures/tassadar/sources/tassadar_module_vm_style_param_refusal.wat",
            source_file_name: "tassadar_module_vm_style_param_refusal.wat",
            wasm_file_name: "tassadar_module_vm_style_param_refusal.wasm",
            export_symbols: &["dispatch_param"],
            expected_status: TassadarModuleScaleWorkloadStatus::LoweringRefused,
        },
    ]
}

fn build_tassadar_module_scale_workload_suite_contract(
    version: &str,
) -> Result<TassadarModuleScaleWorkloadSuiteContract, DataModuleScaleWorkloadSuiteError> {
    TassadarModuleScaleWorkloadSuiteContract::new(
        TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REF,
        version,
        TASSADAR_MODULE_SCALE_BENCHMARK_REF,
        TASSADAR_MODULE_SCALE_BENCHMARK_ENVIRONMENT_REF,
        vec![
            TassadarModuleScaleWorkloadFamily::Memcpy,
            TassadarModuleScaleWorkloadFamily::Parsing,
            TassadarModuleScaleWorkloadFamily::Checksum,
            TassadarModuleScaleWorkloadFamily::VmStyle,
        ],
        vec![
            String::from(MODULE_SCALE_AXIS_EXACTNESS_BPS),
            String::from(MODULE_SCALE_AXIS_TOTAL_TRACE_STEPS),
            String::from(MODULE_SCALE_AXIS_CPU_REFERENCE_COST_UNITS),
            String::from(MODULE_SCALE_AXIS_REFUSAL_KIND),
        ],
        module_scale_case_specs()
            .iter()
            .map(|spec| TassadarModuleScaleWorkloadCaseContract {
                case_id: String::from(spec.case_id),
                family: spec.family,
                summary: String::from(spec.summary),
                source_ref: String::from(spec.source_ref),
                expected_status: spec.expected_status,
            })
            .collect(),
        TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
    )
}

fn build_tassadar_module_scale_environment_bundle(
    version: &str,
    artifact_digests: Vec<String>,
    corpus_digest: &str,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
) -> Result<TassadarEnvironmentBundle, TassadarEnvironmentError> {
    let dataset = DatasetKey::new(TASSADAR_MODULE_SCALE_DATASET_REF, version);
    let suite_contract = build_tassadar_module_scale_workload_suite_contract(version)
        .expect("suite contract should validate");
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar Module-Scale Wasm Workload Suite"),
        eval_environment_ref: String::from(TASSADAR_MODULE_SCALE_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(TASSADAR_MODULE_SCALE_BENCHMARK_ENVIRONMENT_REF),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/module_scale/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/module_scale/benchmark"),
            required: true,
        },
        package_refs: psionic_environments::TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.module_scale"),
            eval_pin_alias: String::from("tassadar_module_scale_eval"),
            benchmark_pin_alias: String::from("tassadar_module_scale_benchmark"),
            eval_member_ref: String::from("tassadar_module_scale_eval_member"),
            benchmark_member_ref: String::from("tassadar_module_scale_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/module_scale.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/module_scale/eval"),
            benchmark_profile_ref: String::from(TASSADAR_MODULE_SCALE_BENCHMARK_REF),
            benchmark_runtime_profile_ref: String::from(
                "runtime://tassadar/module_scale/benchmark",
            ),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/module_scale.validation"),
            corpus_digest: String::from(corpus_digest),
            wasm_profile_id: profile.profile_id.clone(),
            trace_abi_id: trace_abi.abi_id.clone(),
            trace_abi_version: trace_abi.schema_version,
            opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
            artifact_digests,
        },
        io_contract: TassadarIoContract::exact_i32_sequence(),
        exactness_contract: TassadarExactnessContract {
            require_final_output_exactness: true,
            require_step_exactness: true,
            require_halt_exactness: true,
            timeout_budget_ms: 15_000,
            trace_budget_steps: 256,
            require_cpu_reference_baseline: true,
            require_reference_linear_baseline: true,
            future_throughput_metric_ids: Vec::new(),
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
        module_scale_workload_suite_binding: Some(TassadarModuleScaleWorkloadSuiteBinding {
            suite_ref: String::from(suite_contract.suite_ref.clone()),
            suite_version: String::from(suite_contract.version.clone()),
            supported_families: suite_contract.supported_families.clone(),
            evaluation_axes: suite_contract.evaluation_axes.clone(),
            report_ref: String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF),
        }),
        clrs_wasm_bridge_binding: None,
        eval_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Verification,
            policy_ref: String::from("policy://tassadar/module_scale/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/module_scale/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from("policy://tassadar/module_scale/benchmark/verification"),
                required: true,
            },
        ],
        current_workload_targets: vec![
            TassadarWorkloadTarget::ModuleMemcpy,
            TassadarWorkloadTarget::ModuleParsing,
            TassadarWorkloadTarget::ModuleChecksum,
            TassadarWorkloadTarget::ModuleVmStyle,
        ],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
}

fn build_tassadar_module_scale_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    cases: &[TassadarModuleScaleWorkloadCaseReport],
) -> Result<BenchmarkPackage, crate::EvalRuntimeError> {
    let benchmark_cases = cases
        .iter()
        .enumerate()
        .map(|(ordinal, case)| {
            let mut benchmark_case = BenchmarkCase::new(case.case_id.clone());
            benchmark_case.ordinal = Some(ordinal as u64);
            benchmark_case.input_ref = Some(format!("tassadar://input/{}/none", case.case_id));
            benchmark_case.expected_output_ref =
                Some(format!("tassadar://expected_output/{}", case.case_id));
            benchmark_case.metadata = json!({
                "summary": case.summary,
                "family": case.family,
                "workload_target": case.workload_target,
                "source_ref": case.source_ref,
                "status": case.status,
                "compile_receipt_digest": case.compile_receipt_digest,
                "artifact_bundle_digest": case.artifact_bundle_digest,
                "lowered_export_names": case.lowered_export_names,
                "exact_outputs_by_export": case.exact_outputs_by_export,
                "exactness_bps": case.exactness_bps,
                "total_trace_steps": case.total_trace_steps,
                "max_trace_steps": case.max_trace_steps,
                "cpu_reference_cost_units": case.cpu_reference_cost_units,
                "refusal_kind": case.refusal_kind,
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_MODULE_SCALE_BENCHMARK_REF, version),
        "Tassadar Module-Scale Wasm Workload Suite",
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
    .with_cases(benchmark_cases);
    package.metadata.insert(
        String::from("tassadar.current_workload_targets"),
        serde_json::to_value(&environment_bundle.current_workload_targets)
            .unwrap_or(serde_json::Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.module_scale_suite_ref"),
        Value::String(String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REF)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.validate()?;
    Ok(package)
}

fn evaluate_module_scale_case(
    spec: &ModuleScaleCaseSpec,
    profile: &TassadarWasmProfile,
) -> Result<TassadarModuleScaleWorkloadCaseReport, TassadarModuleScaleWorkloadSuiteReportError> {
    let source_path = fixture_source_path(spec.source_file_name);
    let source_bytes = fs::read(&source_path).map_err(|error| {
        TassadarModuleScaleWorkloadSuiteReportError::ReadSource {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let source_text =
        std::str::from_utf8(&source_bytes).expect("module-scale WAT fixture should be valid UTF-8");
    let output_wasm_path = fixture_wasm_path(spec.wasm_file_name);
    let compile_config = TassadarWasmTextCompileConfig {
        export_symbols: spec
            .export_symbols
            .iter()
            .map(|symbol| String::from(*symbol))
            .collect(),
    };

    match compile_tassadar_wasm_text_to_artifact_bundle(
        spec.source_ref,
        source_text,
        &output_wasm_path,
        &compile_config,
        profile,
    ) {
        Ok(pipeline) => {
            if spec.expected_status != TassadarModuleScaleWorkloadStatus::LoweredExact {
                return Err(
                    TassadarModuleScaleWorkloadSuiteReportError::UnexpectedStatus {
                        case_id: String::from(spec.case_id),
                        expected_status: spec.expected_status,
                        actual_status: TassadarModuleScaleWorkloadStatus::LoweredExact,
                    },
                );
            }
            exact_case_report(
                spec,
                &pipeline.artifact_bundle,
                &pipeline.compile_receipt,
                &compile_config,
            )
        }
        Err(TassadarWasmTextArtifactBundlePipelineError::LoweringRefused {
            compile_receipt,
            error,
            ..
        }) => {
            if spec.expected_status != TassadarModuleScaleWorkloadStatus::LoweringRefused {
                return Err(
                    TassadarModuleScaleWorkloadSuiteReportError::UnexpectedStatus {
                        case_id: String::from(spec.case_id),
                        expected_status: spec.expected_status,
                        actual_status: TassadarModuleScaleWorkloadStatus::LoweringRefused,
                    },
                );
            }
            let normalized_module_digest = Some(read_normalized_module_digest(
                spec.case_id,
                &output_wasm_path,
            )?);
            Ok(TassadarModuleScaleWorkloadCaseReport {
                case_id: String::from(spec.case_id),
                family: spec.family,
                workload_target: spec.workload_target,
                summary: String::from(spec.summary),
                source_ref: String::from(spec.source_ref),
                status: TassadarModuleScaleWorkloadStatus::LoweringRefused,
                source_kind: compile_receipt.source_identity.source_kind,
                source_digest: compile_receipt.source_identity.source_digest.clone(),
                compile_config_digest: compile_config.stable_digest(),
                compile_receipt_digest: compile_receipt.receipt_digest.clone(),
                toolchain_digest: compile_receipt.toolchain_identity.stable_digest(),
                wasm_binary_ref: compile_receipt.wasm_binary_ref().map(ToOwned::to_owned),
                wasm_binary_digest: compile_receipt.wasm_binary_digest().map(ToOwned::to_owned),
                wasm_binary_summary: compile_receipt.wasm_binary_summary().cloned(),
                normalized_module_digest,
                artifact_bundle_digest: None,
                lowered_export_names: Vec::new(),
                lowered_artifact_digests: Vec::new(),
                exact_outputs_by_export: BTreeMap::new(),
                total_trace_steps: None,
                max_trace_steps: None,
                exactness_bps: None,
                cpu_reference_cost_units: None,
                refusal_kind: Some(String::from(error.kind_slug())),
                refusal_detail: Some(error.to_string()),
            })
        }
        Err(TassadarWasmTextArtifactBundlePipelineError::CompileRefused {
            compile_receipt,
            refusal,
        }) => {
            if spec.expected_status != TassadarModuleScaleWorkloadStatus::CompileRefused {
                return Err(
                    TassadarModuleScaleWorkloadSuiteReportError::UnexpectedStatus {
                        case_id: String::from(spec.case_id),
                        expected_status: spec.expected_status,
                        actual_status: TassadarModuleScaleWorkloadStatus::CompileRefused,
                    },
                );
            }
            Ok(TassadarModuleScaleWorkloadCaseReport {
                case_id: String::from(spec.case_id),
                family: spec.family,
                workload_target: spec.workload_target,
                summary: String::from(spec.summary),
                source_ref: String::from(spec.source_ref),
                status: TassadarModuleScaleWorkloadStatus::CompileRefused,
                source_kind: compile_receipt.source_identity.source_kind,
                source_digest: compile_receipt.source_identity.source_digest.clone(),
                compile_config_digest: compile_config.stable_digest(),
                compile_receipt_digest: compile_receipt.receipt_digest.clone(),
                toolchain_digest: compile_receipt.toolchain_identity.stable_digest(),
                wasm_binary_ref: None,
                wasm_binary_digest: None,
                wasm_binary_summary: None,
                normalized_module_digest: None,
                artifact_bundle_digest: None,
                lowered_export_names: Vec::new(),
                lowered_artifact_digests: Vec::new(),
                exact_outputs_by_export: BTreeMap::new(),
                total_trace_steps: None,
                max_trace_steps: None,
                exactness_bps: None,
                cpu_reference_cost_units: None,
                refusal_kind: Some(String::from(refusal.kind_slug())),
                refusal_detail: Some(refusal.to_string()),
            })
        }
        Err(TassadarWasmTextArtifactBundlePipelineError::WriteCompiledWasm {
            path,
            message,
            ..
        }) => Err(
            TassadarModuleScaleWorkloadSuiteReportError::ReadCompiledWasm {
                case_id: String::from(spec.case_id),
                path,
                error: std::io::Error::other(message),
            },
        ),
    }
}

fn exact_case_report(
    spec: &ModuleScaleCaseSpec,
    bundle: &TassadarWasmModuleArtifactBundle,
    compile_receipt: &psionic_compiler::TassadarWasmTextCompileReceipt,
    compile_config: &TassadarWasmTextCompileConfig,
) -> Result<TassadarModuleScaleWorkloadCaseReport, TassadarModuleScaleWorkloadSuiteReportError> {
    let mut exact_outputs_by_export = BTreeMap::new();
    let mut total_trace_steps = 0u64;
    let mut max_trace_steps = 0u64;
    let mut cpu_reference_cost_units = 0u64;
    for artifact in &bundle.lowered_exports {
        let execution =
            TassadarCpuReferenceRunner::for_program(&artifact.program_artifact.validated_program)
                .expect("lowered program should select a runner")
                .execute(&artifact.program_artifact.validated_program)
                .map_err(
                    |error| TassadarModuleScaleWorkloadSuiteReportError::Replay {
                        case_id: String::from(spec.case_id),
                        export_name: artifact.export_name.clone(),
                        error,
                    },
                )?;
        if execution.outputs != artifact.execution_manifest.expected_outputs {
            return Err(
                TassadarModuleScaleWorkloadSuiteReportError::ExactnessMismatch {
                    case_id: String::from(spec.case_id),
                    export_name: artifact.export_name.clone(),
                },
            );
        }
        exact_outputs_by_export.insert(artifact.export_name.clone(), execution.outputs);
        let step_count = execution.steps.len() as u64;
        total_trace_steps = total_trace_steps.saturating_add(step_count);
        max_trace_steps = max_trace_steps.max(step_count);
        let instruction_count = artifact
            .program_artifact
            .validated_program
            .instructions
            .len() as u64;
        cpu_reference_cost_units = cpu_reference_cost_units
            .saturating_add(step_count.saturating_mul(instruction_count.max(1)));
    }

    Ok(TassadarModuleScaleWorkloadCaseReport {
        case_id: String::from(spec.case_id),
        family: spec.family,
        workload_target: spec.workload_target,
        summary: String::from(spec.summary),
        source_ref: String::from(spec.source_ref),
        status: TassadarModuleScaleWorkloadStatus::LoweredExact,
        source_kind: compile_receipt.source_identity.source_kind,
        source_digest: compile_receipt.source_identity.source_digest.clone(),
        compile_config_digest: compile_config.stable_digest(),
        compile_receipt_digest: compile_receipt.receipt_digest.clone(),
        toolchain_digest: compile_receipt.toolchain_identity.stable_digest(),
        wasm_binary_ref: compile_receipt.wasm_binary_ref().map(ToOwned::to_owned),
        wasm_binary_digest: compile_receipt.wasm_binary_digest().map(ToOwned::to_owned),
        wasm_binary_summary: compile_receipt.wasm_binary_summary().cloned(),
        normalized_module_digest: Some(bundle.normalized_module.module_digest.clone()),
        artifact_bundle_digest: Some(bundle.bundle_digest.clone()),
        lowered_export_names: bundle
            .lowered_exports
            .iter()
            .map(|artifact| artifact.export_name.clone())
            .collect(),
        lowered_artifact_digests: bundle
            .lowered_exports
            .iter()
            .map(|artifact| artifact.program_artifact.artifact_digest.clone())
            .collect(),
        exact_outputs_by_export,
        total_trace_steps: Some(total_trace_steps),
        max_trace_steps: Some(max_trace_steps),
        exactness_bps: Some(10_000),
        cpu_reference_cost_units: Some(cpu_reference_cost_units),
        refusal_kind: None,
        refusal_detail: None,
    })
}

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

fn read_normalized_module_digest(
    case_id: &str,
    wasm_path: &Path,
) -> Result<String, TassadarModuleScaleWorkloadSuiteReportError> {
    let wasm_bytes = fs::read(wasm_path).map_err(|error| {
        TassadarModuleScaleWorkloadSuiteReportError::ReadCompiledWasm {
            case_id: String::from(case_id),
            path: wasm_path.display().to_string(),
            error,
        }
    })?;
    let normalized = parse_tassadar_normalized_wasm_module(&wasm_bytes).map_err(|error| {
        TassadarModuleScaleWorkloadSuiteReportError::NormalizeCompiledWasm {
            case_id: String::from(case_id),
            error,
        }
    })?;
    Ok(normalized.module_digest)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn fixture_source_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("tassadar")
        .join("sources")
        .join(file_name)
}

fn fixture_wasm_path(file_name: &str) -> PathBuf {
    repo_root()
        .join("fixtures")
        .join("tassadar")
        .join("wasm")
        .join(file_name)
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
        TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF, TassadarModuleScaleWorkloadStatus,
        TassadarModuleScaleWorkloadSuiteReport, build_tassadar_module_scale_workload_suite_report,
        repo_root, write_tassadar_module_scale_workload_suite_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn module_scale_workload_suite_report_captures_exact_and_refused_cases() {
        let report = build_tassadar_module_scale_workload_suite_report().expect("report");
        assert_eq!(report.cases.len(), 5);
        assert!(
            report
                .cases
                .iter()
                .filter(|case| case.status == TassadarModuleScaleWorkloadStatus::LoweredExact)
                .count()
                >= 4
        );
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.refusal_kind.as_deref() == Some("unsupported_param_count"))
        );
    }

    #[test]
    fn module_scale_workload_suite_report_matches_committed_truth() {
        let generated = build_tassadar_module_scale_workload_suite_report().expect("report");
        let committed: TassadarModuleScaleWorkloadSuiteReport =
            read_repo_json(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_module_scale_workload_suite_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_module_scale_workload_suite_report.json");
        let written =
            write_tassadar_module_scale_workload_suite_report(&output_path).expect("write report");
        let persisted: TassadarModuleScaleWorkloadSuiteReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
