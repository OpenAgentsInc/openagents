use std::time::Instant;

use psionic_data::DatasetKey;
use psionic_environments::{
    EnvironmentDatasetBinding, EnvironmentPolicyKind, EnvironmentPolicyReference,
    TassadarEnvironmentBundle, TassadarEnvironmentError, TassadarEnvironmentPackageRefs,
    TassadarEnvironmentSpec, TassadarExactnessContract, TassadarIoContract, TassadarProgramBinding,
    TassadarWorkloadTarget,
};
use psionic_models::{TassadarExecutorContractError, TassadarExecutorFixture};
use psionic_runtime::{
    build_tassadar_execution_evidence_bundle, run_tassadar_exact_equivalence,
    tassadar_validation_corpus, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarExecutorDecodeMode, TassadarExecutorSelectionReason, TassadarExecutorSelectionState,
    TassadarFixtureRunner, TassadarHullCacheRunner, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarTraceAbi, TassadarWasmProfile,
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
    EvalVerificationFacts,
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
/// Stable metric id for the Phase 5 hull-cache lane.
pub const TASSADAR_HULL_CACHE_METRIC_ID: &str = "tassadar.hull_cache_steps_per_second";

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
const TASSADAR_TRACE_STEP_COUNT_METRIC_ID: &str = "tassadar.trace_step_count";

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
    /// Whether trace digests matched across CPU, linear, and hull-cache paths.
    pub trace_digest_equal: bool,
    /// Whether outputs matched across CPU, linear, and hull-cache paths.
    pub outputs_equal: bool,
    /// Whether halt reasons matched across CPU, linear, and hull-cache paths.
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
    /// Runner-independent CPU behavior digest.
    pub cpu_behavior_digest: String,
    /// Runner-independent reference-linear behavior digest.
    pub reference_linear_behavior_digest: String,
    /// Runner-independent hull-cache behavior digest.
    pub hull_cache_behavior_digest: String,
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
        let selection = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::HullCache);
        let effective_decode_mode = selection
            .effective_decode_mode
            .expect("validated benchmark corpus should not be refused");

        let equivalence_started = Instant::now();
        let equivalence_report = run_tassadar_exact_equivalence(&case.program)?;
        let equivalence_elapsed = equivalence_started.elapsed();
        let cpu_execution = &equivalence_report.cpu_reference;
        let reference_execution = &equivalence_report.reference_linear;
        let hull_cache_execution = &equivalence_report.hull_cache;
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
        let trace_digest_equal = equivalence_report.trace_digest_equal();
        let outputs_equal = equivalence_report.outputs_equal();
        let halt_equal = equivalence_report.halt_equal();
        let hull_cache_speedup_over_reference_linear =
            hull_cache_steps_per_second / reference_linear_steps_per_second.max(1e-9);
        let hull_cache_remaining_gap_vs_cpu_reference =
            cpu_steps_per_second / hull_cache_steps_per_second.max(1e-9);

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
                    String::from("selection_state"),
                    serde_json::to_value(selection.selection_state).unwrap_or(Value::Null),
                ),
                (
                    String::from("selection_reason"),
                    serde_json::to_value(selection.selection_reason).unwrap_or(Value::Null),
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
            cpu_behavior_digest: cpu_execution.behavior_digest(),
            reference_linear_behavior_digest: reference_execution.behavior_digest(),
            hull_cache_behavior_digest: hull_cache_execution.behavior_digest(),
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
        String::from("tassadar.corpus_digest"),
        Value::String(environment_bundle.program_binding.corpus_digest.clone()),
    );
    package.validate()?;
    Ok(package)
}

fn classify_case(case_id: &str) -> TassadarWorkloadTarget {
    match case_id {
        "locals_add" | "tassadar.locals_add.v1" => TassadarWorkloadTarget::ArithmeticMicroprogram,
        "memory_roundtrip" | "tassadar.memory_roundtrip.v1" => {
            TassadarWorkloadTarget::MemoryLookupMicroprogram
        }
        "branch_guard" | "tassadar.branch_guard.v1" => {
            TassadarWorkloadTarget::BranchControlFlowMicroprogram
        }
        _ => TassadarWorkloadTarget::MicroWasmKernel,
    }
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

fn stable_outputs_digest(outputs: &[i32]) -> String {
    let bytes = serde_json::to_vec(outputs).unwrap_or_default();
    hex::encode(sha2::Sha256::digest(bytes))
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

    #[test]
    fn tassadar_reference_fixture_suite_builds_package_and_environment_contracts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let suite = build_tassadar_reference_fixture_suite("2026.03.15")?;
        assert_eq!(suite.artifacts.len(), 3);
        assert_eq!(suite.benchmark_package.cases.len(), 3);
        assert_eq!(
            suite
                .benchmark_package
                .metadata
                .get("tassadar.hull_cache_metric_id")
                .and_then(Value::as_str),
            Some(TASSADAR_HULL_CACHE_METRIC_ID)
        );
        assert_eq!(
            suite.environment_bundle.current_workload_targets,
            vec![
                TassadarWorkloadTarget::ArithmeticMicroprogram,
                TassadarWorkloadTarget::MemoryLookupMicroprogram,
                TassadarWorkloadTarget::BranchControlFlowMicroprogram,
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
            .all(|case| case.hull_cache_speedup_over_reference_linear > 1.0));
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
}
