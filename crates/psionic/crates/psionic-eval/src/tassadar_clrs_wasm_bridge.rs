use std::{
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use psionic_compiler::{
    TassadarClrsWasmBridgeCaseSpec, TassadarClrsWasmBridgeCompileError,
    compile_tassadar_clrs_wasm_bridge_case, tassadar_clrs_wasm_bridge_case_specs,
};
use psionic_data::{
    DatasetKey, TassadarBenchmarkAxis, TassadarBenchmarkFamily, TassadarClrsAlgorithmFamily,
    TassadarClrsLengthBucket, TassadarClrsTrajectoryFamily, TassadarClrsWasmBridgeCaseContract,
    TassadarClrsWasmBridgeContract, TassadarClrsWasmBridgeError,
    TassadarClrsWasmBridgeExportContract,
};
use psionic_environments::{
    EnvironmentDatasetBinding, EnvironmentPolicyKind, EnvironmentPolicyReference,
    TassadarBenchmarkPackageSetBinding, TassadarClrsWasmBridgeBinding,
    TassadarCompilePipelineMatrixBinding, TassadarEnvironmentBundle, TassadarEnvironmentError,
    TassadarEnvironmentSpec, TassadarExactnessContract, TassadarIoContract, TassadarProgramBinding,
    TassadarWasmConformanceBinding, TassadarWorkloadTarget,
};
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
    BenchmarkVerificationPolicy, EvalRuntimeError,
    TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF, TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF,
    TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF, TASSADAR_WASM_CONFORMANCE_REPORT_REF,
};

const TASSADAR_CLRS_WASM_BRIDGE_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json";
pub const TASSADAR_CLRS_WASM_BRIDGE_REF: &str = "benchmark-bridge://openagents/tassadar/clrs_wasm";
pub const TASSADAR_CLRS_WASM_BRIDGE_EVAL_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.clrs_wasm_bridge.eval";
pub const TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.clrs_wasm_bridge.benchmark";
pub const TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/clrs_wasm_bridge/reference_fixture";
pub const TASSADAR_CLRS_WASM_BRIDGE_DATASET_REF: &str =
    "dataset://openagents/tassadar/clrs_wasm_bridge";

const CLRS_AXIS_EXACTNESS_BPS: &str = "exactness_bps";
const CLRS_AXIS_MODULE_TRACE_STEPS: &str = "module_trace_steps";
const CLRS_AXIS_CPU_REFERENCE_COST_UNITS: &str = "cpu_reference_cost_units";
const CLRS_AXIS_TRAJECTORY_STEP_DELTA: &str = "trajectory_step_delta";

static CLRS_WASM_BRIDGE_BUILD_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Repo-facing outcome for one exported bridge bucket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeExportReport {
    /// Stable length bucket.
    pub length_bucket: TassadarClrsLengthBucket,
    /// Export symbol implementing the bucket.
    pub export_name: String,
    /// Exact outputs observed on the CPU reference lane.
    pub exact_outputs: Vec<i32>,
    /// Trace steps observed for the export.
    pub module_trace_steps: u64,
    /// Deterministic CPU-reference replay cost units for the export.
    pub cpu_reference_cost_units: u64,
    /// Exactness in basis points.
    pub exactness_bps: u32,
}

/// Repo-facing outcome for one CLRS-to-Wasm bridge case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// CLRS algorithm family.
    pub algorithm_family: TassadarClrsAlgorithmFamily,
    /// Trajectory family.
    pub trajectory_family: TassadarClrsTrajectoryFamily,
    /// Human-readable case summary.
    pub summary: String,
    /// Repo-relative source ref.
    pub source_ref: String,
    /// Stable source kind.
    pub source_kind: TassadarProgramSourceKind,
    /// Stable source digest.
    pub source_digest: String,
    /// Stable compile-config digest.
    pub compile_config_digest: String,
    /// Stable compile receipt digest.
    pub compile_receipt_digest: String,
    /// Stable compiler toolchain digest.
    pub toolchain_digest: String,
    /// Repo-relative Wasm output ref.
    pub wasm_binary_ref: String,
    /// Stable Wasm output digest.
    pub wasm_binary_digest: String,
    /// Structural summary over the compiled Wasm output.
    pub wasm_binary_summary: TassadarWasmBinarySummary,
    /// Stable normalized-module digest.
    pub normalized_module_digest: String,
    /// Stable artifact bundle digest.
    pub artifact_bundle_digest: String,
    /// Ordered export outcomes.
    pub exports: Vec<TassadarClrsWasmBridgeExportReport>,
}

/// Comparison between sequential and wavefront bridge cases for one length bucket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmTrajectoryComparison {
    /// CLRS algorithm family.
    pub algorithm_family: TassadarClrsAlgorithmFamily,
    /// Length bucket being compared.
    pub length_bucket: TassadarClrsLengthBucket,
    /// Sequential case identifier.
    pub sequential_case_id: String,
    /// Alternative case identifier.
    pub alternative_case_id: String,
    /// Whether the observed outputs matched exactly.
    pub output_exact_match: bool,
    /// Sequential trace steps for the bucket.
    pub sequential_trace_steps: u64,
    /// Alternative trace steps for the bucket.
    pub alternative_trace_steps: u64,
    /// Signed step delta from sequential to alternative.
    pub trajectory_step_delta: i64,
    /// Sequential cost units for the bucket.
    pub sequential_cpu_reference_cost_units: u64,
    /// Alternative cost units for the bucket.
    pub alternative_cpu_reference_cost_units: u64,
    /// Signed cost delta from sequential to alternative.
    pub cpu_reference_cost_delta: i64,
}

/// One cell in the committed CLRS bridge length-generalization matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmLengthGeneralizationCell {
    /// CLRS algorithm family.
    pub algorithm_family: TassadarClrsAlgorithmFamily,
    /// Trajectory family.
    pub trajectory_family: TassadarClrsTrajectoryFamily,
    /// Length bucket.
    pub length_bucket: TassadarClrsLengthBucket,
    /// Exactness in basis points.
    pub exactness_bps: u32,
    /// Trace steps observed in the bucket.
    pub module_trace_steps: u64,
    /// CPU-reference cost units observed in the bucket.
    pub cpu_reference_cost_units: u64,
}

/// Committed report over the CLRS-to-Wasm bridge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public bridge contract.
    pub bridge: TassadarClrsWasmBridgeContract,
    /// Shared environment bundle bound to the bridge.
    pub environment_bundle: TassadarEnvironmentBundle,
    /// Benchmark package bound to the bridge.
    pub benchmark_package: BenchmarkPackage,
    /// Ordered case outcomes.
    pub cases: Vec<TassadarClrsWasmBridgeCaseReport>,
    /// Sequential-vs-wavefront comparisons.
    pub trajectory_comparisons: Vec<TassadarClrsWasmTrajectoryComparison>,
    /// Length-generalization matrix cells.
    pub length_generalization_matrix: Vec<TassadarClrsWasmLengthGeneralizationCell>,
    /// Stable refs used to generate the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarClrsWasmBridgeReport {
    fn new(
        bridge: TassadarClrsWasmBridgeContract,
        environment_bundle: TassadarEnvironmentBundle,
        benchmark_package: BenchmarkPackage,
        cases: Vec<TassadarClrsWasmBridgeCaseReport>,
        trajectory_comparisons: Vec<TassadarClrsWasmTrajectoryComparison>,
        length_generalization_matrix: Vec<TassadarClrsWasmLengthGeneralizationCell>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_CLRS_WASM_BRIDGE_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.clrs_wasm_bridge.report.v1"),
            bridge,
            environment_bundle,
            benchmark_package,
            cases,
            trajectory_comparisons,
            length_generalization_matrix,
            generated_from_refs,
            claim_boundary: String::from(
                "this report freezes one benchmark-bound CLRS-to-Wasm bridge over fixed shortest-path witnesses compiled into the current bounded Wasm lane, with sequential versus wavefront trajectory families and tiny versus small length buckets kept explicit. It does not claim full CLRS coverage, arbitrary Wasm closure, or learned transfer by itself",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(b"psionic_tassadar_clrs_wasm_bridge_report|", &report);
        report
    }
}

/// CLRS bridge report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarClrsWasmBridgeReportError {
    /// CLRS bridge contract validation failed.
    #[error(transparent)]
    Contract(#[from] TassadarClrsWasmBridgeError),
    /// Environment bundle validation failed.
    #[error(transparent)]
    Environment(#[from] TassadarEnvironmentError),
    /// Compiler-side case compile failed.
    #[error(transparent)]
    Compiler(#[from] TassadarClrsWasmBridgeCompileError),
    /// Replay of one lowered export failed unexpectedly.
    #[error("failed to replay lowered export `{export_name}` for case `{case_id}`: {error}")]
    Replay {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
        /// Runtime refusal.
        error: TassadarExecutionRefusal,
    },
    /// One lowered export diverged from its execution manifest.
    #[error(
        "lowered export `{export_name}` for case `{case_id}` diverged from its execution manifest"
    )]
    ExactnessMismatch {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
    },
    /// Benchmark package validation failed.
    #[error(transparent)]
    Benchmark(#[from] EvalRuntimeError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write CLRS-to-Wasm bridge report `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed CLRS-to-Wasm bridge report.
pub fn build_tassadar_clrs_wasm_bridge_report()
-> Result<TassadarClrsWasmBridgeReport, TassadarClrsWasmBridgeReportError> {
    let _build_guard = CLRS_WASM_BRIDGE_BUILD_LOCK
        .lock()
        .expect("CLRS-to-Wasm bridge build lock should not be poisoned");
    build_tassadar_clrs_wasm_bridge_report_impl()
}

fn build_tassadar_clrs_wasm_bridge_report_impl()
-> Result<TassadarClrsWasmBridgeReport, TassadarClrsWasmBridgeReportError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let specs = tassadar_clrs_wasm_bridge_case_specs();
    let cases = specs
        .iter()
        .map(|spec| evaluate_bridge_case(spec, &profile))
        .collect::<Result<Vec<_>, _>>()?;
    let bridge = build_tassadar_clrs_wasm_bridge_contract("2026.03.18")?;
    let artifact_digests = cases
        .iter()
        .map(|case| case.artifact_bundle_digest.clone())
        .collect::<Vec<_>>();
    let corpus_digest = stable_digest(
        b"psionic_tassadar_clrs_wasm_bridge_corpus|",
        &(artifact_digests.clone(), bridge.cases.clone()),
    );
    let environment_bundle = build_tassadar_clrs_wasm_bridge_environment_bundle(
        "2026.03.18",
        artifact_digests,
        &corpus_digest,
        &bridge,
        &profile,
        &trace_abi,
    )?;
    let benchmark_package = build_tassadar_clrs_wasm_bridge_benchmark_package(
        "2026.03.18",
        &environment_bundle,
        &cases,
    )?;
    let trajectory_comparisons = build_trajectory_comparisons(&cases);
    let length_generalization_matrix = build_length_generalization_matrix(&cases);
    Ok(TassadarClrsWasmBridgeReport::new(
        bridge,
        environment_bundle,
        benchmark_package,
        cases,
        trajectory_comparisons,
        length_generalization_matrix,
        specs
            .iter()
            .map(|spec| String::from(spec.source_ref))
            .collect(),
    ))
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_clrs_wasm_bridge_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)
}

/// Writes the committed CLRS-to-Wasm bridge report.
pub fn write_tassadar_clrs_wasm_bridge_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarClrsWasmBridgeReport, TassadarClrsWasmBridgeReportError> {
    let _build_guard = CLRS_WASM_BRIDGE_BUILD_LOCK
        .lock()
        .expect("CLRS-to-Wasm bridge build lock should not be poisoned");
    write_tassadar_clrs_wasm_bridge_report_impl(output_path)
}

fn write_tassadar_clrs_wasm_bridge_report_impl(
    output_path: impl AsRef<Path>,
) -> Result<TassadarClrsWasmBridgeReport, TassadarClrsWasmBridgeReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarClrsWasmBridgeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_clrs_wasm_bridge_report_impl()?;
    let json =
        serde_json::to_string_pretty(&report).expect("CLRS-to-Wasm bridge report should serialize");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarClrsWasmBridgeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_clrs_wasm_bridge_contract(
    version: &str,
) -> Result<TassadarClrsWasmBridgeContract, TassadarClrsWasmBridgeError> {
    TassadarClrsWasmBridgeContract::new(
        TASSADAR_CLRS_WASM_BRIDGE_REF,
        version,
        TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_REF,
        TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_ENVIRONMENT_REF,
        vec![TassadarClrsAlgorithmFamily::ShortestPath],
        vec![
            TassadarClrsTrajectoryFamily::SequentialRelaxation,
            TassadarClrsTrajectoryFamily::WavefrontRelaxation,
        ],
        vec![
            TassadarClrsLengthBucket::Tiny,
            TassadarClrsLengthBucket::Small,
        ],
        vec![
            String::from(CLRS_AXIS_EXACTNESS_BPS),
            String::from(CLRS_AXIS_MODULE_TRACE_STEPS),
            String::from(CLRS_AXIS_CPU_REFERENCE_COST_UNITS),
            String::from(CLRS_AXIS_TRAJECTORY_STEP_DELTA),
        ],
        tassadar_clrs_wasm_bridge_case_specs()
            .iter()
            .map(|spec| TassadarClrsWasmBridgeCaseContract {
                case_id: String::from(spec.case_id),
                algorithm_family: map_algorithm_family(spec.algorithm_id),
                trajectory_family: map_trajectory_family(spec.trajectory_family_id),
                summary: String::from(spec.summary),
                source_ref: String::from(spec.source_ref),
                export_bindings: spec
                    .export_specs
                    .iter()
                    .map(|export| TassadarClrsWasmBridgeExportContract {
                        length_bucket: map_length_bucket(export.length_bucket_id),
                        export_name: String::from(export.export_symbol),
                    })
                    .collect(),
            })
            .collect(),
        TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF,
    )
}

fn build_tassadar_clrs_wasm_bridge_environment_bundle(
    version: &str,
    artifact_digests: Vec<String>,
    corpus_digest: &str,
    bridge: &TassadarClrsWasmBridgeContract,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
) -> Result<TassadarEnvironmentBundle, TassadarEnvironmentError> {
    let dataset = DatasetKey::new(TASSADAR_CLRS_WASM_BRIDGE_DATASET_REF, version);
    TassadarEnvironmentSpec {
        version: String::from(version),
        display_name: String::from("Tassadar CLRS-to-Wasm Bridge"),
        eval_environment_ref: String::from(TASSADAR_CLRS_WASM_BRIDGE_EVAL_ENVIRONMENT_REF),
        benchmark_environment_ref: String::from(
            TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_ENVIRONMENT_REF,
        ),
        eval_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("validation")),
            mount_path: String::from("/datasets/tassadar/clrs_wasm_bridge/validation"),
            required: true,
        },
        benchmark_dataset: EnvironmentDatasetBinding {
            dataset: dataset.clone(),
            split: Some(String::from("benchmark")),
            mount_path: String::from("/datasets/tassadar/clrs_wasm_bridge/benchmark"),
            required: true,
        },
        package_refs: psionic_environments::TassadarEnvironmentPackageRefs {
            group_ref: String::from("group.tassadar.clrs_wasm_bridge"),
            eval_pin_alias: String::from("tassadar_clrs_wasm_bridge_eval"),
            benchmark_pin_alias: String::from("tassadar_clrs_wasm_bridge_benchmark"),
            eval_member_ref: String::from("tassadar_clrs_wasm_bridge_eval_member"),
            benchmark_member_ref: String::from("tassadar_clrs_wasm_bridge_benchmark_member"),
            program_corpus_ref: String::from("tassadar://corpus/clrs_wasm_bridge.validation"),
            io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
            rubric_binding_ref: String::from("tassadar://rubric/exactness"),
            eval_runtime_profile_ref: String::from("runtime://tassadar/clrs_wasm_bridge/eval"),
            benchmark_profile_ref: String::from(TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_REF),
            benchmark_runtime_profile_ref: String::from(
                "runtime://tassadar/clrs_wasm_bridge/benchmark",
            ),
        },
        program_binding: TassadarProgramBinding {
            dataset,
            program_corpus_ref: String::from("tassadar://corpus/clrs_wasm_bridge.validation"),
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
            supported_families: vec![TassadarBenchmarkFamily::ClrsSubset],
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
        clrs_wasm_bridge_binding: Some(TassadarClrsWasmBridgeBinding {
            bridge_ref: bridge.bridge_ref.clone(),
            bridge_version: bridge.version.clone(),
            supported_algorithms: bridge.supported_algorithms.clone(),
            trajectory_families: bridge.trajectory_families.clone(),
            length_buckets: bridge.length_buckets.clone(),
            report_ref: String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF),
        }),
        eval_policy_references: vec![EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Verification,
            policy_ref: String::from("policy://tassadar/clrs_wasm_bridge/eval/verification"),
            required: true,
        }],
        benchmark_policy_references: vec![
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://tassadar/clrs_wasm_bridge/benchmark"),
                required: true,
            },
            EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from(
                    "policy://tassadar/clrs_wasm_bridge/benchmark/verification",
                ),
                required: true,
            },
        ],
        current_workload_targets: vec![TassadarWorkloadTarget::ClrsShortestPath],
        planned_workload_targets: Vec::new(),
    }
    .build_bundle()
}

fn build_tassadar_clrs_wasm_bridge_benchmark_package(
    version: &str,
    environment_bundle: &TassadarEnvironmentBundle,
    cases: &[TassadarClrsWasmBridgeCaseReport],
) -> Result<BenchmarkPackage, EvalRuntimeError> {
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
                "algorithm_family": case.algorithm_family,
                "trajectory_family": case.trajectory_family,
                "summary": case.summary,
                "source_ref": case.source_ref,
                "compile_receipt_digest": case.compile_receipt_digest,
                "artifact_bundle_digest": case.artifact_bundle_digest,
                "exports": case.exports,
                "trace_budget_steps": environment_bundle.exactness_contract.trace_budget_steps,
                "timeout_budget_ms": environment_bundle.exactness_contract.timeout_budget_ms
            });
            benchmark_case
        })
        .collect::<Vec<_>>();

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(TASSADAR_CLRS_WASM_BRIDGE_BENCHMARK_REF, version),
        "Tassadar CLRS-to-Wasm Bridge",
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
        serde_json::to_value(&environment_bundle.current_workload_targets).unwrap_or(Value::Null),
    );
    package.metadata.insert(
        String::from("tassadar.clrs_wasm_bridge_ref"),
        Value::String(String::from(TASSADAR_CLRS_WASM_BRIDGE_REF)),
    );
    package.metadata.insert(
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.validate()?;
    Ok(package)
}

fn evaluate_bridge_case(
    spec: &TassadarClrsWasmBridgeCaseSpec,
    profile: &TassadarWasmProfile,
) -> Result<TassadarClrsWasmBridgeCaseReport, TassadarClrsWasmBridgeReportError> {
    let pipeline = compile_tassadar_clrs_wasm_bridge_case(spec, profile)?;
    let mut exports = Vec::new();
    for export_spec in &spec.export_specs {
        let artifact = pipeline
            .artifact_bundle
            .lowered_exports
            .iter()
            .find(|artifact| artifact.export_name == export_spec.export_symbol)
            .expect("bridge export should lower");
        let execution =
            TassadarCpuReferenceRunner::for_program(&artifact.program_artifact.validated_program)
                .expect("lowered program should select a runner")
                .execute(&artifact.program_artifact.validated_program)
                .map_err(|error| TassadarClrsWasmBridgeReportError::Replay {
                    case_id: String::from(spec.case_id),
                    export_name: artifact.export_name.clone(),
                    error,
                })?;
        if execution.outputs != artifact.execution_manifest.expected_outputs {
            return Err(TassadarClrsWasmBridgeReportError::ExactnessMismatch {
                case_id: String::from(spec.case_id),
                export_name: artifact.export_name.clone(),
            });
        }
        let trace_steps = execution.steps.len() as u64;
        let instruction_count = artifact
            .program_artifact
            .validated_program
            .instructions
            .len() as u64;
        exports.push(TassadarClrsWasmBridgeExportReport {
            length_bucket: map_length_bucket(export_spec.length_bucket_id),
            export_name: artifact.export_name.clone(),
            exact_outputs: execution.outputs,
            module_trace_steps: trace_steps,
            cpu_reference_cost_units: trace_steps.saturating_mul(instruction_count.max(1)),
            exactness_bps: 10_000,
        });
    }
    exports.sort_by_key(|export| export.length_bucket);

    Ok(TassadarClrsWasmBridgeCaseReport {
        case_id: String::from(spec.case_id),
        algorithm_family: map_algorithm_family(spec.algorithm_id),
        trajectory_family: map_trajectory_family(spec.trajectory_family_id),
        summary: String::from(spec.summary),
        source_ref: String::from(spec.source_ref),
        source_kind: pipeline.compile_receipt.source_identity.source_kind,
        source_digest: pipeline
            .compile_receipt
            .source_identity
            .source_digest
            .clone(),
        compile_config_digest: spec.compile_config().stable_digest(),
        compile_receipt_digest: pipeline.compile_receipt.receipt_digest.clone(),
        toolchain_digest: pipeline.compile_receipt.toolchain_identity.stable_digest(),
        wasm_binary_ref: pipeline
            .compile_receipt
            .wasm_binary_ref()
            .expect("compiled bridge case should publish Wasm ref")
            .to_string(),
        wasm_binary_digest: pipeline
            .compile_receipt
            .wasm_binary_digest()
            .expect("compiled bridge case should publish Wasm digest")
            .to_string(),
        wasm_binary_summary: pipeline
            .compile_receipt
            .wasm_binary_summary()
            .expect("compiled bridge case should publish Wasm summary")
            .clone(),
        normalized_module_digest: pipeline
            .artifact_bundle
            .normalized_module
            .module_digest
            .clone(),
        artifact_bundle_digest: pipeline.artifact_bundle.bundle_digest.clone(),
        exports,
    })
}

fn build_trajectory_comparisons(
    cases: &[TassadarClrsWasmBridgeCaseReport],
) -> Vec<TassadarClrsWasmTrajectoryComparison> {
    let mut comparisons = Vec::new();
    let Some(sequential_case) = cases
        .iter()
        .find(|case| case.trajectory_family == TassadarClrsTrajectoryFamily::SequentialRelaxation)
    else {
        return comparisons;
    };
    let Some(wavefront_case) = cases
        .iter()
        .find(|case| case.trajectory_family == TassadarClrsTrajectoryFamily::WavefrontRelaxation)
    else {
        return comparisons;
    };
    for length_bucket in [
        TassadarClrsLengthBucket::Tiny,
        TassadarClrsLengthBucket::Small,
    ] {
        let sequential_export = sequential_case
            .exports
            .iter()
            .find(|export| export.length_bucket == length_bucket)
            .expect("sequential export should exist");
        let wavefront_export = wavefront_case
            .exports
            .iter()
            .find(|export| export.length_bucket == length_bucket)
            .expect("wavefront export should exist");
        comparisons.push(TassadarClrsWasmTrajectoryComparison {
            algorithm_family: sequential_case.algorithm_family,
            length_bucket,
            sequential_case_id: sequential_case.case_id.clone(),
            alternative_case_id: wavefront_case.case_id.clone(),
            output_exact_match: sequential_export.exact_outputs == wavefront_export.exact_outputs,
            sequential_trace_steps: sequential_export.module_trace_steps,
            alternative_trace_steps: wavefront_export.module_trace_steps,
            trajectory_step_delta: wavefront_export.module_trace_steps as i64
                - sequential_export.module_trace_steps as i64,
            sequential_cpu_reference_cost_units: sequential_export.cpu_reference_cost_units,
            alternative_cpu_reference_cost_units: wavefront_export.cpu_reference_cost_units,
            cpu_reference_cost_delta: wavefront_export.cpu_reference_cost_units as i64
                - sequential_export.cpu_reference_cost_units as i64,
        });
    }
    comparisons
}

fn build_length_generalization_matrix(
    cases: &[TassadarClrsWasmBridgeCaseReport],
) -> Vec<TassadarClrsWasmLengthGeneralizationCell> {
    let mut cells = cases
        .iter()
        .flat_map(|case| {
            case.exports
                .iter()
                .map(|export| TassadarClrsWasmLengthGeneralizationCell {
                    algorithm_family: case.algorithm_family,
                    trajectory_family: case.trajectory_family,
                    length_bucket: export.length_bucket,
                    exactness_bps: export.exactness_bps,
                    module_trace_steps: export.module_trace_steps,
                    cpu_reference_cost_units: export.cpu_reference_cost_units,
                })
        })
        .collect::<Vec<_>>();
    cells.sort_by_key(|cell| {
        (
            cell.algorithm_family,
            cell.trajectory_family,
            cell.length_bucket,
        )
    });
    cells
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

fn map_algorithm_family(algorithm_id: &str) -> TassadarClrsAlgorithmFamily {
    match algorithm_id {
        "shortest_path" => TassadarClrsAlgorithmFamily::ShortestPath,
        other => panic!("unsupported CLRS algorithm id `{other}`"),
    }
}

fn map_trajectory_family(trajectory_family_id: &str) -> TassadarClrsTrajectoryFamily {
    match trajectory_family_id {
        "sequential_relaxation" => TassadarClrsTrajectoryFamily::SequentialRelaxation,
        "wavefront_relaxation" => TassadarClrsTrajectoryFamily::WavefrontRelaxation,
        other => panic!("unsupported trajectory family id `{other}`"),
    }
}

fn map_length_bucket(length_bucket_id: &str) -> TassadarClrsLengthBucket {
    match length_bucket_id {
        "tiny" => TassadarClrsLengthBucket::Tiny,
        "small" => TassadarClrsLengthBucket::Small,
        other => panic!("unsupported length bucket id `{other}`"),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("CLRS-to-Wasm bridge value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        build_tassadar_clrs_wasm_bridge_report, tassadar_clrs_wasm_bridge_report_path,
        write_tassadar_clrs_wasm_bridge_report,
    };

    #[test]
    fn clrs_wasm_bridge_report_keeps_sequential_and_wavefront_separate() {
        let report = build_tassadar_clrs_wasm_bridge_report()
            .expect("CLRS-to-Wasm bridge report should build");
        assert_eq!(report.cases.len(), 2);
        assert_eq!(report.trajectory_comparisons.len(), 2);
        assert!(
            report
                .trajectory_comparisons
                .iter()
                .all(|comparison| comparison.output_exact_match)
        );
        assert!(
            report
                .trajectory_comparisons
                .iter()
                .any(|comparison| comparison.trajectory_step_delta < 0)
        );
    }

    #[test]
    fn clrs_wasm_bridge_report_matches_committed_truth() {
        let report = build_tassadar_clrs_wasm_bridge_report()
            .expect("CLRS-to-Wasm bridge report should build");
        let committed = fs::read_to_string(tassadar_clrs_wasm_bridge_report_path())
            .expect("committed CLRS-to-Wasm bridge report should exist");
        let committed_report = serde_json::from_str(&committed)
            .expect("committed CLRS-to-Wasm bridge report should parse");
        assert_eq!(report, committed_report);
    }

    #[test]
    fn write_clrs_wasm_bridge_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_clrs_wasm_bridge_report.json");
        let report = write_tassadar_clrs_wasm_bridge_report(&output_path)
            .expect("writing CLRS-to-Wasm bridge report should succeed");
        let written = fs::read_to_string(&output_path).expect("written report should exist");
        let reparsed = serde_json::from_str(&written).expect("written report should parse");
        assert_eq!(report, reparsed);
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_clrs_wasm_bridge_report.json")
        );
        let _ = fs::remove_file(output_path);
    }
}
