use std::time::Instant;

use psionic_models::{
    TassadarCompiledProgramError, TassadarCompiledProgramExecution,
    TassadarCompiledProgramExecutor, TassadarCompiledProgramSuiteArtifact,
    TassadarExecutorContractError, TassadarExecutorFixture,
};
use psionic_runtime::{
    tassadar_hungarian_10x10_corpus, TassadarClaimClass, TassadarCpuReferenceRunner,
    TassadarExecutionRefusal, TassadarExecutorDecodeMode, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarSudokuV0CorpusSplit, TassadarTraceAbi,
    TassadarWasmProfile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable workload family id for the article-sized compiled Hungarian-10x10 lane.
pub const TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID: &str =
    "tassadar.wasm.hungarian_10x10_matching.v1.compiled_executor";

/// One compiled-program deployment bound to a real Hungarian-10x10 corpus case.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarHungarian10x10CompiledExecutorCorpusCase {
    pub case_id: String,
    pub split: TassadarSudokuV0CorpusSplit,
    pub cost_matrix: Vec<i32>,
    pub search_row_order: Vec<usize>,
    pub optimal_assignment: Vec<i32>,
    pub optimal_cost: i32,
    pub program_artifact: TassadarProgramArtifact,
    pub compiled_executor: TassadarCompiledProgramExecutor,
}

/// Article-sized compiled-executor corpus and suite artifact for Hungarian-10x10.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarHungarian10x10CompiledExecutorCorpus {
    pub workload_family_id: String,
    pub cases: Vec<TassadarHungarian10x10CompiledExecutorCorpusCase>,
    pub compiled_suite_artifact: TassadarCompiledProgramSuiteArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CompiledExecutorCaseExactnessReport {
    pub case_id: String,
    pub split: TassadarSudokuV0CorpusSplit,
    pub cost_matrix: Vec<i32>,
    pub search_row_order: Vec<usize>,
    pub optimal_assignment: Vec<i32>,
    pub optimal_cost: i32,
    pub program_artifact_digest: String,
    pub program_digest: String,
    pub compiled_weight_artifact_digest: String,
    pub runtime_contract_digest: String,
    pub compile_trace_proof_digest: String,
    pub compile_execution_proof_bundle_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    pub cpu_trace_digest: String,
    pub compiled_trace_digest: String,
    pub cpu_behavior_digest: String,
    pub compiled_behavior_digest: String,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub halt_match: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CompiledExecutorExactnessReport {
    pub workload_family_id: String,
    pub compiled_suite_artifact_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub total_case_count: u32,
    pub exact_trace_case_count: u32,
    pub exact_trace_rate_bps: u32,
    pub final_output_match_case_count: u32,
    pub halt_match_case_count: u32,
    pub case_reports: Vec<TassadarHungarian10x10CompiledExecutorCaseExactnessReport>,
    pub report_digest: String,
}

impl TassadarHungarian10x10CompiledExecutorExactnessReport {
    fn new(
        compiled_suite_artifact_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        case_reports: Vec<TassadarHungarian10x10CompiledExecutorCaseExactnessReport>,
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
                TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
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
            b"psionic_tassadar_hungarian_10x10_compiled_executor_exactness_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHungarian10x10CompiledExecutorRefusalKind {
    ProgramArtifactDigestMismatch,
    ProgramDigestMismatch,
    WasmProfileMismatch,
    TraceAbiMismatch,
    TraceAbiVersionMismatch,
    OpcodeVocabularyDigestMismatch,
    ProgramProfileMismatch,
    ProgramArtifactInconsistent,
    SelectionRefused,
    UnexpectedSuccess,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CompiledExecutorRefusalCheckReport {
    pub deployment_case_id: String,
    pub check_id: String,
    pub expected_refusal_kind: TassadarHungarian10x10CompiledExecutorRefusalKind,
    pub observed_refusal_kind: TassadarHungarian10x10CompiledExecutorRefusalKind,
    pub matched_expected_refusal: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CompiledExecutorCompatibilityReport {
    pub workload_family_id: String,
    pub compiled_suite_artifact_digest: String,
    pub total_check_count: u32,
    pub matched_refusal_check_count: u32,
    pub matched_refusal_rate_bps: u32,
    pub check_reports: Vec<TassadarHungarian10x10CompiledExecutorRefusalCheckReport>,
    pub report_digest: String,
}

impl TassadarHungarian10x10CompiledExecutorCompatibilityReport {
    fn new(
        compiled_suite_artifact_digest: String,
        check_reports: Vec<TassadarHungarian10x10CompiledExecutorRefusalCheckReport>,
    ) -> Self {
        let total_check_count = check_reports.len() as u32;
        let matched_refusal_check_count = check_reports
            .iter()
            .filter(|check| check.matched_expected_refusal)
            .count() as u32;
        let mut report = Self {
            workload_family_id: String::from(
                TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            compiled_suite_artifact_digest,
            total_check_count,
            matched_refusal_check_count,
            matched_refusal_rate_bps: ratio_bps(matched_refusal_check_count, total_check_count),
            check_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_hungarian_10x10_compiled_executor_compatibility_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CompiledExecutorBenchmarkCaseReceipt {
    pub case_id: String,
    pub split: TassadarSudokuV0CorpusSplit,
    pub optimal_cost: i32,
    pub program_artifact_digest: String,
    pub compiled_weight_artifact_digest: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: TassadarExecutorDecodeMode,
    pub trace_step_count: u64,
    pub cpu_reference_steps_per_second: f64,
    pub compiled_executor_steps_per_second: f64,
    pub compiled_over_cpu_ratio: f64,
    pub runtime_trace_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CompiledExecutorBenchmarkReceipt {
    pub workload_family_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub total_case_count: u32,
    pub average_cpu_reference_steps_per_second: f64,
    pub average_compiled_executor_steps_per_second: f64,
    pub average_compiled_over_cpu_ratio: f64,
    pub case_receipts: Vec<TassadarHungarian10x10CompiledExecutorBenchmarkCaseReceipt>,
    pub report_digest: String,
}

impl TassadarHungarian10x10CompiledExecutorBenchmarkReceipt {
    fn new(
        requested_decode_mode: TassadarExecutorDecodeMode,
        case_receipts: Vec<TassadarHungarian10x10CompiledExecutorBenchmarkCaseReceipt>,
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
                TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
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
            b"psionic_tassadar_hungarian_10x10_compiled_executor_benchmark_receipt|",
            &receipt,
        );
        receipt
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHungarian10x10LaneClaimStatus {
    Exact,
    ResearchOnly,
    NotDone,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10ClaimBoundaryReport {
    pub workload_family_id: String,
    pub claim_class: TassadarClaimClass,
    pub compiled_lane_status: TassadarHungarian10x10LaneClaimStatus,
    pub compiled_lane_detail: String,
    pub learned_lane_status: TassadarHungarian10x10LaneClaimStatus,
    pub learned_lane_detail: String,
    pub report_digest: String,
}

impl TassadarHungarian10x10ClaimBoundaryReport {
    fn new() -> Self {
        let mut report = Self {
            workload_family_id: String::from(
                TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            claim_class: TassadarClaimClass::CompiledArticleClass,
            compiled_lane_status: TassadarHungarian10x10LaneClaimStatus::Exact,
            compiled_lane_detail: String::from(
                "exact compiled/proof-backed article-sized 10x10 Hungarian lane is landed on the committed corpus via the larger search-oriented Wasm profile",
            ),
            learned_lane_status: TassadarHungarian10x10LaneClaimStatus::NotDone,
            learned_lane_detail: String::from(
                "no learned 10x10 Hungarian long-trace lane is landed; compiled exactness is the only promoted closure here",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_hungarian_10x10_claim_boundary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarHungarian10x10CompiledExecutorEvalError {
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    #[error(transparent)]
    Compiled(#[from] TassadarCompiledProgramError),
}

pub fn build_tassadar_hungarian_10x10_compiled_executor_corpus(
    split_filter: Option<TassadarSudokuV0CorpusSplit>,
) -> Result<
    TassadarHungarian10x10CompiledExecutorCorpus,
    TassadarHungarian10x10CompiledExecutorEvalError,
> {
    let fixture = TassadarExecutorFixture::hungarian_10x10_matching_v1();
    let mut cases = Vec::new();
    let mut artifacts = Vec::new();
    for corpus_case in tassadar_hungarian_10x10_corpus() {
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
        cases.push(TassadarHungarian10x10CompiledExecutorCorpusCase {
            case_id: corpus_case.case_id,
            split: corpus_case.split,
            cost_matrix: corpus_case.cost_matrix,
            search_row_order: corpus_case.search_row_order,
            optimal_assignment: corpus_case.optimal_assignment,
            optimal_cost: corpus_case.optimal_cost,
            program_artifact: artifact,
            compiled_executor,
        });
    }
    let compiled_suite_artifact = TassadarCompiledProgramSuiteArtifact::compile(
        "tassadar.hungarian_10x10.compiled_executor_suite",
        "benchmark://tassadar/hungarian_10x10_compiled_executor@v0",
        &fixture,
        artifacts.as_slice(),
    )?;
    Ok(TassadarHungarian10x10CompiledExecutorCorpus {
        workload_family_id: String::from(
            TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
        ),
        cases,
        compiled_suite_artifact,
    })
}

pub fn build_tassadar_hungarian_10x10_compiled_executor_exactness_report(
    corpus: &TassadarHungarian10x10CompiledExecutorCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    TassadarHungarian10x10CompiledExecutorExactnessReport,
    TassadarHungarian10x10CompiledExecutorEvalError,
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
        case_reports.push(TassadarHungarian10x10CompiledExecutorCaseExactnessReport {
            case_id: corpus_case.case_id.clone(),
            split: corpus_case.split,
            cost_matrix: corpus_case.cost_matrix.clone(),
            search_row_order: corpus_case.search_row_order.clone(),
            optimal_assignment: corpus_case.optimal_assignment.clone(),
            optimal_cost: corpus_case.optimal_cost,
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
    Ok(TassadarHungarian10x10CompiledExecutorExactnessReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        requested_decode_mode,
        case_reports,
    ))
}

pub fn build_tassadar_hungarian_10x10_compiled_executor_compatibility_report(
    corpus: &TassadarHungarian10x10CompiledExecutorCorpus,
) -> Result<
    TassadarHungarian10x10CompiledExecutorCompatibilityReport,
    TassadarHungarian10x10CompiledExecutorEvalError,
> {
    let mut check_reports = Vec::new();
    for (index, corpus_case) in corpus.cases.iter().enumerate() {
        if corpus.cases.len() > 1 {
            let wrong_case = &corpus.cases[(index + 1) % corpus.cases.len()];
            check_reports.push(run_refusal_check(
                &corpus_case.case_id,
                "wrong_program_artifact",
                TassadarHungarian10x10CompiledExecutorRefusalKind::ProgramArtifactDigestMismatch,
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
            TassadarHungarian10x10CompiledExecutorRefusalKind::WasmProfileMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_profile_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut wrong_trace_abi_artifact = corpus_case.program_artifact.clone();
        wrong_trace_abi_artifact.trace_abi_version =
            TassadarTraceAbi::hungarian_10x10_matching_v1()
                .schema_version
                .saturating_add(1);
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            "wrong_trace_abi_version",
            TassadarHungarian10x10CompiledExecutorRefusalKind::TraceAbiVersionMismatch,
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
            TassadarHungarian10x10CompiledExecutorRefusalKind::ProgramArtifactInconsistent,
            corpus_case.compiled_executor.execute(
                &inconsistent_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));
    }
    Ok(
        TassadarHungarian10x10CompiledExecutorCompatibilityReport::new(
            corpus.compiled_suite_artifact.artifact_digest.clone(),
            check_reports,
        ),
    )
}

pub fn build_tassadar_hungarian_10x10_compiled_executor_benchmark_receipt(
    corpus: &TassadarHungarian10x10CompiledExecutorCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    TassadarHungarian10x10CompiledExecutorBenchmarkReceipt,
    TassadarHungarian10x10CompiledExecutorEvalError,
> {
    let mut case_receipts = Vec::with_capacity(corpus.cases.len());
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
        let effective_decode_mode = compiled_sample
            .execution_report
            .selection
            .effective_decode_mode
            .unwrap_or(TassadarExecutorDecodeMode::ReferenceLinear);
        case_receipts.push(TassadarHungarian10x10CompiledExecutorBenchmarkCaseReceipt {
            case_id: corpus_case.case_id.clone(),
            split: corpus_case.split,
            optimal_cost: corpus_case.optimal_cost,
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
    Ok(TassadarHungarian10x10CompiledExecutorBenchmarkReceipt::new(
        requested_decode_mode,
        case_receipts,
    ))
}

#[must_use]
pub fn build_tassadar_hungarian_10x10_claim_boundary_report(
) -> TassadarHungarian10x10ClaimBoundaryReport {
    TassadarHungarian10x10ClaimBoundaryReport::new()
}

fn run_refusal_check(
    deployment_case_id: &str,
    check_id: &str,
    expected_refusal_kind: TassadarHungarian10x10CompiledExecutorRefusalKind,
    outcome: Result<TassadarCompiledProgramExecution, TassadarCompiledProgramError>,
) -> TassadarHungarian10x10CompiledExecutorRefusalCheckReport {
    match outcome {
        Ok(_) => TassadarHungarian10x10CompiledExecutorRefusalCheckReport {
            deployment_case_id: deployment_case_id.to_string(),
            check_id: check_id.to_string(),
            expected_refusal_kind,
            observed_refusal_kind:
                TassadarHungarian10x10CompiledExecutorRefusalKind::UnexpectedSuccess,
            matched_expected_refusal: false,
            detail: String::from("compiled executor unexpectedly accepted mismatched artifact"),
        },
        Err(error) => {
            let observed_refusal_kind = refusal_kind_from_error(&error);
            TassadarHungarian10x10CompiledExecutorRefusalCheckReport {
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
) -> TassadarHungarian10x10CompiledExecutorRefusalKind {
    match error {
        TassadarCompiledProgramError::DescriptorContract { error } => match error {
            TassadarExecutorContractError::ProgramArtifactInconsistent { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::ProgramArtifactInconsistent
            }
            TassadarExecutorContractError::WasmProfileMismatch { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::WasmProfileMismatch
            }
            TassadarExecutorContractError::TraceAbiMismatch { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::TraceAbiMismatch
            }
            TassadarExecutorContractError::TraceAbiVersionMismatch { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::TraceAbiVersionMismatch
            }
            TassadarExecutorContractError::OpcodeVocabularyDigestMismatch { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::OpcodeVocabularyDigestMismatch
            }
            TassadarExecutorContractError::ProgramProfileMismatch { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::ProgramProfileMismatch
            }
            TassadarExecutorContractError::DecodeModeUnsupported { .. } => {
                TassadarHungarian10x10CompiledExecutorRefusalKind::SelectionRefused
            }
        },
        TassadarCompiledProgramError::SelectionRefused { .. } => {
            TassadarHungarian10x10CompiledExecutorRefusalKind::SelectionRefused
        }
        TassadarCompiledProgramError::ProgramArtifactDigestMismatch { .. } => {
            TassadarHungarian10x10CompiledExecutorRefusalKind::ProgramArtifactDigestMismatch
        }
        TassadarCompiledProgramError::ProgramDigestMismatch { .. } => {
            TassadarHungarian10x10CompiledExecutorRefusalKind::ProgramDigestMismatch
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
        .expect("Tassadar Hungarian-10x10 compiled executor eval artifact should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_hungarian_10x10_claim_boundary_report,
        build_tassadar_hungarian_10x10_compiled_executor_benchmark_receipt,
        build_tassadar_hungarian_10x10_compiled_executor_compatibility_report,
        build_tassadar_hungarian_10x10_compiled_executor_corpus,
        build_tassadar_hungarian_10x10_compiled_executor_exactness_report,
        TassadarHungarian10x10LaneClaimStatus,
    };
    use psionic_runtime::{
        TassadarClaimClass, TassadarExecutorDecodeMode, TassadarSudokuV0CorpusSplit,
    };

    #[test]
    fn hungarian_10x10_compiled_executor_exactness_is_exact_for_validation_corpus(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_hungarian_10x10_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let report = build_tassadar_hungarian_10x10_compiled_executor_exactness_report(
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
    fn hungarian_10x10_compiled_executor_refusal_report_matches_expected_surface(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_hungarian_10x10_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let report =
            build_tassadar_hungarian_10x10_compiled_executor_compatibility_report(&corpus)?;

        assert_eq!(report.total_check_count, 3);
        assert_eq!(report.matched_refusal_check_count, 3);
        assert_eq!(report.matched_refusal_rate_bps, 10_000);
        Ok(())
    }

    #[test]
    fn hungarian_10x10_compiled_executor_benchmark_receipt_has_positive_throughput(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_hungarian_10x10_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let receipt = build_tassadar_hungarian_10x10_compiled_executor_benchmark_receipt(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;

        assert_eq!(receipt.total_case_count, 1);
        assert!(receipt.average_cpu_reference_steps_per_second > 0.0);
        assert!(receipt.average_compiled_executor_steps_per_second > 0.0);
        Ok(())
    }

    #[test]
    fn hungarian_10x10_claim_boundary_report_is_honest() {
        let report = build_tassadar_hungarian_10x10_claim_boundary_report();
        assert_eq!(report.claim_class, TassadarClaimClass::CompiledArticleClass);
        assert_eq!(
            report.learned_lane_status,
            TassadarHungarian10x10LaneClaimStatus::NotDone
        );
    }
}
