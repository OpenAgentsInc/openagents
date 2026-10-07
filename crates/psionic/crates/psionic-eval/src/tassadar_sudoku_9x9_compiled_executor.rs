use std::time::Instant;

use psionic_models::{
    TassadarCompiledProgramError, TassadarCompiledProgramExecution,
    TassadarCompiledProgramExecutor, TassadarCompiledProgramSuiteArtifact,
    TassadarExecutorContractError, TassadarExecutorFixture,
};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarProgramArtifact, TassadarProgramArtifactError, TassadarSudokuV0CorpusSplit,
    TassadarTraceAbi, TassadarWasmProfile, tassadar_sudoku_9x9_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable workload family id for the exact compiled Sudoku-9x9 executor lane.
pub const TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID: &str =
    "tassadar.wasm.sudoku_9x9_search.v1.compiled_executor";

/// One compiled-program deployment bound to a real Sudoku-9x9 corpus case.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarSudoku9x9CompiledExecutorCorpusCase {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable corpus split.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat 9x9 puzzle cells.
    pub puzzle_cells: Vec<i32>,
    /// Number of givens in the puzzle.
    pub given_count: usize,
    /// Digest-bound program artifact for the case.
    pub program_artifact: TassadarProgramArtifact,
    /// Program-specialized compiled executor for the exact artifact.
    pub compiled_executor: TassadarCompiledProgramExecutor,
}

/// Exact compiled-executor corpus and suite artifact for Sudoku-9x9.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarSudoku9x9CompiledExecutorCorpus {
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Ordered compiled corpus cases.
    pub cases: Vec<TassadarSudoku9x9CompiledExecutorCorpusCase>,
    /// Suite-level compiled-weight artifact.
    pub compiled_suite_artifact: TassadarCompiledProgramSuiteArtifact,
}

/// Per-case exactness facts for one compiled Sudoku-9x9 deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorCaseExactnessReport {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable corpus split.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat 9x9 puzzle cells.
    pub puzzle_cells: Vec<i32>,
    /// Number of givens in the puzzle.
    pub given_count: usize,
    /// Stable program-artifact digest.
    pub program_artifact_digest: String,
    /// Stable validated-program digest.
    pub program_digest: String,
    /// Stable compiled-weight artifact digest.
    pub compiled_weight_artifact_digest: String,
    /// Stable runtime-contract digest.
    pub runtime_contract_digest: String,
    /// Stable compile-time trace-proof digest.
    pub compile_trace_proof_digest: String,
    /// Stable compile-time proof-bundle digest.
    pub compile_execution_proof_bundle_digest: String,
    /// Stable runtime execution proof-bundle digest.
    pub runtime_execution_proof_bundle_digest: String,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode realized by the runtime.
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    /// CPU-reference trace digest.
    pub cpu_trace_digest: String,
    /// Compiled-lane trace digest.
    pub compiled_trace_digest: String,
    /// CPU-reference behavior digest.
    pub cpu_behavior_digest: String,
    /// Compiled-lane behavior digest.
    pub compiled_behavior_digest: String,
    /// Whether the full append-only trace stayed exact.
    pub exact_trace_match: bool,
    /// Whether final outputs matched.
    pub final_output_match: bool,
    /// Whether halt reasons matched.
    pub halt_match: bool,
}

/// Machine-readable exactness report for the compiled Sudoku-9x9 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorExactnessReport {
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Stable suite artifact digest.
    pub compiled_suite_artifact_digest: String,
    /// Requested decode mode used for the benchmark.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Number of evaluated cases.
    pub total_case_count: u32,
    /// Number of exact compiled-vs-CPU trace matches.
    pub exact_trace_case_count: u32,
    /// Exact-trace rate in basis points.
    pub exact_trace_rate_bps: u32,
    /// Number of final-output matches.
    pub final_output_match_case_count: u32,
    /// Number of halt matches.
    pub halt_match_case_count: u32,
    /// Per-case exactness facts.
    pub case_reports: Vec<TassadarSudoku9x9CompiledExecutorCaseExactnessReport>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarSudoku9x9CompiledExecutorExactnessReport {
    fn new(
        compiled_suite_artifact_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        case_reports: Vec<TassadarSudoku9x9CompiledExecutorCaseExactnessReport>,
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
        let mut report = Self {
            workload_family_id: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            compiled_suite_artifact_digest,
            requested_decode_mode,
            total_case_count,
            exact_trace_case_count,
            exact_trace_rate_bps: ratio_bps(exact_trace_case_count, total_case_count),
            final_output_match_case_count,
            halt_match_case_count,
            case_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_compiled_executor_exactness_report|",
            &report,
        );
        report
    }
}

/// Stable refusal surface expected from one mismatch check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSudoku9x9CompiledExecutorRefusalKind {
    /// The supplied program artifact digest mismatched the compiled deployment.
    ProgramArtifactDigestMismatch,
    /// The supplied validated program digest mismatched the compiled deployment.
    ProgramDigestMismatch,
    /// The artifact targeted the wrong Wasm profile.
    WasmProfileMismatch,
    /// The artifact targeted the wrong trace ABI id.
    TraceAbiMismatch,
    /// The artifact targeted the wrong trace ABI version.
    TraceAbiVersionMismatch,
    /// The artifact carried the wrong opcode vocabulary digest.
    OpcodeVocabularyDigestMismatch,
    /// The validated program no longer matches the declared profile.
    ProgramProfileMismatch,
    /// The artifact was internally inconsistent.
    ProgramArtifactInconsistent,
    /// Decode selection refused the request.
    SelectionRefused,
    /// The execution unexpectedly succeeded.
    UnexpectedSuccess,
}

/// One compatibility/refusal check for the compiled Sudoku-9x9 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorRefusalCheckReport {
    /// Stable corpus case id owning the compiled deployment.
    pub deployment_case_id: String,
    /// Stable check id.
    pub check_id: String,
    /// Expected refusal kind for the check.
    pub expected_refusal_kind: TassadarSudoku9x9CompiledExecutorRefusalKind,
    /// Observed refusal kind.
    pub observed_refusal_kind: TassadarSudoku9x9CompiledExecutorRefusalKind,
    /// Whether the observed refusal matched the expectation exactly.
    pub matched_expected_refusal: bool,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Machine-readable compatibility/refusal report for the compiled Sudoku-9x9 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorCompatibilityReport {
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Stable suite artifact digest.
    pub compiled_suite_artifact_digest: String,
    /// Number of refusal checks executed.
    pub total_check_count: u32,
    /// Number of exact refusal matches.
    pub matched_refusal_check_count: u32,
    /// Exact refusal-match rate in basis points.
    pub matched_refusal_rate_bps: u32,
    /// Ordered refusal checks.
    pub check_reports: Vec<TassadarSudoku9x9CompiledExecutorRefusalCheckReport>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarSudoku9x9CompiledExecutorCompatibilityReport {
    fn new(
        compiled_suite_artifact_digest: String,
        check_reports: Vec<TassadarSudoku9x9CompiledExecutorRefusalCheckReport>,
    ) -> Self {
        let total_check_count = check_reports.len() as u32;
        let matched_refusal_check_count = check_reports
            .iter()
            .filter(|check| check.matched_expected_refusal)
            .count() as u32;
        let mut report = Self {
            workload_family_id: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            compiled_suite_artifact_digest,
            total_check_count,
            matched_refusal_check_count,
            matched_refusal_rate_bps: ratio_bps(matched_refusal_check_count, total_check_count),
            check_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_compiled_executor_compatibility_report|",
            &report,
        );
        report
    }
}

/// Per-case throughput receipt for one compiled Sudoku-9x9 deployment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorBenchmarkCaseReceipt {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable split.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Number of givens in the puzzle.
    pub given_count: usize,
    /// Stable program-artifact digest.
    pub program_artifact_digest: String,
    /// Stable compiled-weight artifact digest.
    pub compiled_weight_artifact_digest: String,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode realized by the runtime.
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    /// Exact trace step count.
    pub trace_step_count: u64,
    /// Measured CPU-reference throughput.
    pub cpu_reference_steps_per_second: f64,
    /// Measured compiled-executor throughput.
    pub compiled_executor_steps_per_second: f64,
    /// Compiled-vs-CPU throughput ratio.
    pub compiled_over_cpu_ratio: f64,
    /// Stable runtime trace digest.
    pub runtime_trace_digest: String,
    /// Stable runtime execution proof-bundle digest.
    pub runtime_execution_proof_bundle_digest: String,
}

/// Machine-readable throughput receipt for the compiled Sudoku-9x9 lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorBenchmarkReceipt {
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Requested decode mode measured by the receipt.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Number of measured cases.
    pub total_case_count: u32,
    /// Average CPU-reference throughput.
    pub average_cpu_reference_steps_per_second: f64,
    /// Average compiled-executor throughput.
    pub average_compiled_executor_steps_per_second: f64,
    /// Average compiled-vs-CPU throughput ratio.
    pub average_compiled_over_cpu_ratio: f64,
    /// Per-case receipts.
    pub case_receipts: Vec<TassadarSudoku9x9CompiledExecutorBenchmarkCaseReceipt>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarSudoku9x9CompiledExecutorBenchmarkReceipt {
    fn new(
        requested_decode_mode: TassadarExecutorDecodeMode,
        case_receipts: Vec<TassadarSudoku9x9CompiledExecutorBenchmarkCaseReceipt>,
    ) -> Self {
        let total_case_count = case_receipts.len() as u32;
        let average_cpu_reference_steps_per_second = round_metric(
            case_receipts
                .iter()
                .map(|case| case.cpu_reference_steps_per_second)
                .sum::<f64>()
                / total_case_count.max(1) as f64,
        );
        let average_compiled_executor_steps_per_second = round_metric(
            case_receipts
                .iter()
                .map(|case| case.compiled_executor_steps_per_second)
                .sum::<f64>()
                / total_case_count.max(1) as f64,
        );
        let average_compiled_over_cpu_ratio = round_metric(
            case_receipts
                .iter()
                .map(|case| case.compiled_over_cpu_ratio)
                .sum::<f64>()
                / total_case_count.max(1) as f64,
        );
        let mut receipt = Self {
            workload_family_id: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            requested_decode_mode,
            total_case_count,
            average_cpu_reference_steps_per_second,
            average_compiled_executor_steps_per_second,
            average_compiled_over_cpu_ratio,
            case_receipts,
            report_digest: String::new(),
        };
        receipt.report_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_compiled_executor_benchmark_receipt|",
            &receipt,
        );
        receipt
    }
}

/// Typed failure while building or evaluating the exact compiled Sudoku-9x9 lane.
#[derive(Debug, Error)]
pub enum TassadarSudoku9x9CompiledExecutorEvalError {
    /// Program-artifact assembly failed.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    /// CPU reference execution failed.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// Compiling or executing one compiled deployment failed.
    #[error(transparent)]
    Compiled(#[from] TassadarCompiledProgramError),
}

/// Builds the exact compiled-executor corpus for the real Sudoku-9x9 family.
pub fn build_tassadar_sudoku_9x9_compiled_executor_corpus(
    split_filter: Option<TassadarSudokuV0CorpusSplit>,
) -> Result<TassadarSudoku9x9CompiledExecutorCorpus, TassadarSudoku9x9CompiledExecutorEvalError>
{
    let fixture = TassadarExecutorFixture::sudoku_9x9_search_v1();
    let mut cases = Vec::new();
    let mut artifacts = Vec::new();
    for corpus_case in tassadar_sudoku_9x9_corpus() {
        if split_filter.is_some_and(|split| corpus_case.split != split) {
            continue;
        }
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("{}.compiled_program_artifact", corpus_case.case_id),
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            corpus_case.validation_case.program.clone(),
        )?;
        let compiled_executor = fixture.compile_program(
            format!("{}.compiled_executor", corpus_case.case_id),
            &artifact,
        )?;
        artifacts.push(artifact.clone());
        cases.push(TassadarSudoku9x9CompiledExecutorCorpusCase {
            case_id: corpus_case.case_id,
            split: corpus_case.split,
            puzzle_cells: corpus_case.puzzle_cells,
            given_count: corpus_case.given_count,
            program_artifact: artifact,
            compiled_executor,
        });
    }
    let compiled_suite_artifact = TassadarCompiledProgramSuiteArtifact::compile(
        "tassadar.sudoku_9x9.compiled_executor_suite",
        "benchmark://tassadar/sudoku_9x9_compiled_executor@v0",
        &fixture,
        artifacts.as_slice(),
    )?;
    Ok(TassadarSudoku9x9CompiledExecutorCorpus {
        workload_family_id: String::from(
            TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
        ),
        cases,
        compiled_suite_artifact,
    })
}

/// Benchmarks the exact compiled Sudoku-9x9 lane against CPU reference truth.
pub fn build_tassadar_sudoku_9x9_compiled_executor_exactness_report(
    corpus: &TassadarSudoku9x9CompiledExecutorCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    TassadarSudoku9x9CompiledExecutorExactnessReport,
    TassadarSudoku9x9CompiledExecutorEvalError,
> {
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
        case_reports.push(TassadarSudoku9x9CompiledExecutorCaseExactnessReport {
            case_id: corpus_case.case_id.clone(),
            split: corpus_case.split,
            puzzle_cells: corpus_case.puzzle_cells.clone(),
            given_count: corpus_case.given_count,
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
            cpu_trace_digest: cpu_execution.trace_digest(),
            compiled_trace_digest: runtime_execution.trace_digest(),
            cpu_behavior_digest: cpu_execution.behavior_digest(),
            compiled_behavior_digest: runtime_execution.behavior_digest(),
            exact_trace_match: runtime_execution.steps == cpu_execution.steps,
            final_output_match: runtime_execution.outputs == cpu_execution.outputs,
            halt_match: runtime_execution.halt_reason == cpu_execution.halt_reason,
        });
    }
    Ok(TassadarSudoku9x9CompiledExecutorExactnessReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        requested_decode_mode,
        case_reports,
    ))
}

/// Builds a machine-readable refusal report for exact compiled Sudoku-9x9 deployments.
pub fn build_tassadar_sudoku_9x9_compiled_executor_compatibility_report(
    corpus: &TassadarSudoku9x9CompiledExecutorCorpus,
) -> Result<
    TassadarSudoku9x9CompiledExecutorCompatibilityReport,
    TassadarSudoku9x9CompiledExecutorEvalError,
> {
    let mut check_reports = Vec::new();
    for (index, corpus_case) in corpus.cases.iter().enumerate() {
        if corpus.cases.len() > 1 {
            let wrong_case = &corpus.cases[(index + 1) % corpus.cases.len()];
            check_reports.push(run_refusal_check(
                &corpus_case.case_id,
                "wrong_program_artifact",
                TassadarSudoku9x9CompiledExecutorRefusalKind::ProgramArtifactDigestMismatch,
                corpus_case.compiled_executor.execute(
                    &wrong_case.program_artifact,
                    TassadarExecutorDecodeMode::ReferenceLinear,
                ),
            ));
        }

        let mut wrong_profile_artifact = corpus_case.program_artifact.clone();
        wrong_profile_artifact.wasm_profile_id =
            TassadarWasmProfile::sudoku_v0_search_v1().profile_id;
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            "wrong_wasm_profile",
            TassadarSudoku9x9CompiledExecutorRefusalKind::WasmProfileMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_profile_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut wrong_trace_abi_artifact = corpus_case.program_artifact.clone();
        wrong_trace_abi_artifact.trace_abi_version = TassadarTraceAbi::sudoku_9x9_search_v1()
            .schema_version
            .saturating_add(1);
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            "wrong_trace_abi_version",
            TassadarSudoku9x9CompiledExecutorRefusalKind::TraceAbiVersionMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_trace_abi_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut inconsistent_artifact = corpus_case.program_artifact.clone();
        inconsistent_artifact.validated_program_digest = String::from("bogus_program_digest");
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            "artifact_inconsistent",
            TassadarSudoku9x9CompiledExecutorRefusalKind::ProgramArtifactInconsistent,
            corpus_case.compiled_executor.execute(
                &inconsistent_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));
    }
    Ok(TassadarSudoku9x9CompiledExecutorCompatibilityReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        check_reports,
    ))
}

/// Builds a machine-readable throughput receipt for the compiled Sudoku-9x9 lane.
pub fn build_tassadar_sudoku_9x9_compiled_executor_benchmark_receipt(
    corpus: &TassadarSudoku9x9CompiledExecutorCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    TassadarSudoku9x9CompiledExecutorBenchmarkReceipt,
    TassadarSudoku9x9CompiledExecutorEvalError,
> {
    let mut case_receipts = Vec::with_capacity(corpus.cases.len());
    for corpus_case in &corpus.cases {
        let cpu_execution = TassadarCpuReferenceRunner::for_program(
            &corpus_case.program_artifact.validated_program,
        )?
        .execute(&corpus_case.program_artifact.validated_program)?;
        let trace_step_count = cpu_execution.steps.len() as u64;
        let cpu_reference_steps_per_second =
            benchmark_steps_per_second(trace_step_count, || {
                TassadarCpuReferenceRunner::for_program(
                    &corpus_case.program_artifact.validated_program,
                )?
                .execute(&corpus_case.program_artifact.validated_program)
            })?;
        let compiled_sample = corpus_case
            .compiled_executor
            .execute(&corpus_case.program_artifact, requested_decode_mode)?;
        let compiled_executor_steps_per_second =
            benchmark_steps_per_second(trace_step_count, || {
                corpus_case
                    .compiled_executor
                    .execute(&corpus_case.program_artifact, requested_decode_mode)
            })?;
        let effective_decode_mode = compiled_sample
            .execution_report
            .selection
            .effective_decode_mode
            .unwrap_or(TassadarExecutorDecodeMode::ReferenceLinear);
        case_receipts.push(TassadarSudoku9x9CompiledExecutorBenchmarkCaseReceipt {
            case_id: corpus_case.case_id.clone(),
            split: corpus_case.split,
            given_count: corpus_case.given_count,
            program_artifact_digest: corpus_case.program_artifact.artifact_digest.clone(),
            compiled_weight_artifact_digest: corpus_case
                .compiled_executor
                .compiled_weight_artifact()
                .artifact_digest
                .clone(),
            requested_decode_mode,
            effective_decode_mode,
            trace_step_count,
            cpu_reference_steps_per_second: round_metric(cpu_reference_steps_per_second),
            compiled_executor_steps_per_second: round_metric(compiled_executor_steps_per_second),
            compiled_over_cpu_ratio: round_metric(
                compiled_executor_steps_per_second / cpu_reference_steps_per_second.max(1e-9),
            ),
            runtime_trace_digest: compiled_sample.execution_report.execution.trace_digest(),
            runtime_execution_proof_bundle_digest: compiled_sample
                .evidence_bundle
                .proof_bundle
                .stable_digest(),
        });
    }
    Ok(TassadarSudoku9x9CompiledExecutorBenchmarkReceipt::new(
        requested_decode_mode,
        case_receipts,
    ))
}

fn run_refusal_check(
    deployment_case_id: &str,
    check_id: &str,
    expected_refusal_kind: TassadarSudoku9x9CompiledExecutorRefusalKind,
    outcome: Result<TassadarCompiledProgramExecution, TassadarCompiledProgramError>,
) -> TassadarSudoku9x9CompiledExecutorRefusalCheckReport {
    match outcome {
        Ok(_) => TassadarSudoku9x9CompiledExecutorRefusalCheckReport {
            deployment_case_id: deployment_case_id.to_string(),
            check_id: check_id.to_string(),
            expected_refusal_kind,
            observed_refusal_kind: TassadarSudoku9x9CompiledExecutorRefusalKind::UnexpectedSuccess,
            matched_expected_refusal: false,
            detail: String::from("compiled executor unexpectedly accepted mismatched artifact"),
        },
        Err(error) => {
            let observed_refusal_kind = refusal_kind_from_error(&error);
            TassadarSudoku9x9CompiledExecutorRefusalCheckReport {
                deployment_case_id: deployment_case_id.to_string(),
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
) -> TassadarSudoku9x9CompiledExecutorRefusalKind {
    match error {
        TassadarCompiledProgramError::DescriptorContract { error } => match error {
            TassadarExecutorContractError::ProgramArtifactInconsistent { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::ProgramArtifactInconsistent
            }
            TassadarExecutorContractError::WasmProfileMismatch { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::WasmProfileMismatch
            }
            TassadarExecutorContractError::TraceAbiMismatch { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::TraceAbiMismatch
            }
            TassadarExecutorContractError::TraceAbiVersionMismatch { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::TraceAbiVersionMismatch
            }
            TassadarExecutorContractError::OpcodeVocabularyDigestMismatch { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::OpcodeVocabularyDigestMismatch
            }
            TassadarExecutorContractError::ProgramProfileMismatch { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::ProgramProfileMismatch
            }
            TassadarExecutorContractError::DecodeModeUnsupported { .. } => {
                TassadarSudoku9x9CompiledExecutorRefusalKind::SelectionRefused
            }
        },
        TassadarCompiledProgramError::SelectionRefused { .. } => {
            TassadarSudoku9x9CompiledExecutorRefusalKind::SelectionRefused
        }
        TassadarCompiledProgramError::ProgramArtifactDigestMismatch { .. } => {
            TassadarSudoku9x9CompiledExecutorRefusalKind::ProgramArtifactDigestMismatch
        }
        TassadarCompiledProgramError::ProgramDigestMismatch { .. } => {
            TassadarSudoku9x9CompiledExecutorRefusalKind::ProgramDigestMismatch
        }
    }
}

fn throughput_steps_per_second(steps: u64, elapsed_seconds: f64) -> f64 {
    steps as f64 / elapsed_seconds.max(1e-9)
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn benchmark_steps_per_second<F, T, E>(steps_per_run: u64, mut runner: F) -> Result<f64, E>
where
    F: FnMut() -> Result<T, E>,
{
    let normalized_steps = steps_per_run.max(1);
    let target_steps = normalized_steps.saturating_mul(64).max(1_024);
    let minimum_runs = 4u64;
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
        .expect("Tassadar Sudoku-9x9 compiled executor eval artifact should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_sudoku_9x9_compiled_executor_benchmark_receipt,
        build_tassadar_sudoku_9x9_compiled_executor_compatibility_report,
        build_tassadar_sudoku_9x9_compiled_executor_exactness_report,
        build_tassadar_sudoku_9x9_compiled_executor_corpus,
    };
    use psionic_runtime::{TassadarExecutorDecodeMode, TassadarSudokuV0CorpusSplit};

    #[test]
    fn compiled_executor_exactness_report_is_exact_for_sudoku_9x9_validation_corpus()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_sudoku_9x9_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let report = build_tassadar_sudoku_9x9_compiled_executor_exactness_report(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;

        assert_eq!(report.total_case_count, 1);
        assert_eq!(report.exact_trace_case_count, 1);
        assert_eq!(report.exact_trace_rate_bps, 10_000);
        assert_eq!(report.final_output_match_case_count, 1);
        assert_eq!(report.halt_match_case_count, 1);
        Ok(())
    }

    #[test]
    fn compiled_executor_compatibility_report_records_exact_refusals_for_sudoku_9x9_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_sudoku_9x9_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let report = build_tassadar_sudoku_9x9_compiled_executor_compatibility_report(&corpus)?;

        assert_eq!(report.total_check_count, 3);
        assert_eq!(report.matched_refusal_check_count, 3);
        assert_eq!(report.matched_refusal_rate_bps, 10_000);
        Ok(())
    }

    #[test]
    fn compiled_executor_benchmark_receipt_records_positive_throughput_for_sudoku_9x9_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_sudoku_9x9_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let receipt = build_tassadar_sudoku_9x9_compiled_executor_benchmark_receipt(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;

        assert_eq!(receipt.total_case_count, 1);
        assert!(receipt.average_cpu_reference_steps_per_second > 0.0);
        assert!(receipt.average_compiled_executor_steps_per_second > 0.0);
        assert_eq!(receipt.case_receipts.len(), 1);
        Ok(())
    }
}
