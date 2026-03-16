use psionic_models::{
    TassadarCompiledProgramError, TassadarCompiledProgramExecution,
    TassadarCompiledProgramExecutor, TassadarCompiledProgramSuiteArtifact,
    TassadarExecutorContractError, TassadarExecutorFixture,
};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarProgramArtifact, TassadarProgramArtifactError, TassadarSudokuV0CorpusSplit,
    TassadarTraceAbi, TassadarWasmProfile, tassadar_hungarian_v0_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable workload family id for the bounded Hungarian-v0 compiled executor lane.
pub const TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID: &str =
    "tassadar.wasm.hungarian_v0_matching.v1.compiled_executor";

/// One compiled-program deployment bound to a real Hungarian-v0 corpus case.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarHungarianCompiledExecutorCorpusCase {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable corpus split.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat row-major cost matrix.
    pub cost_matrix: Vec<i32>,
    /// Exact optimal assignment.
    pub optimal_assignment: Vec<i32>,
    /// Exact optimal cost.
    pub optimal_cost: i32,
    /// Digest-bound program artifact for the case.
    pub program_artifact: TassadarProgramArtifact,
    /// Program-specialized compiled executor for the exact artifact.
    pub compiled_executor: TassadarCompiledProgramExecutor,
}

/// Bounded compiled-executor corpus and suite artifact for Hungarian-v0.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarHungarianCompiledExecutorCorpus {
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Ordered compiled corpus cases.
    pub cases: Vec<TassadarHungarianCompiledExecutorCorpusCase>,
    /// Suite-level compiled-weight artifact.
    pub compiled_suite_artifact: TassadarCompiledProgramSuiteArtifact,
}

/// Per-case exactness facts for one Hungarian compiled deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianCompiledExecutorCaseExactnessReport {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable corpus split.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat row-major cost matrix.
    pub cost_matrix: Vec<i32>,
    /// Exact optimal assignment.
    pub optimal_assignment: Vec<i32>,
    /// Exact optimal cost.
    pub optimal_cost: i32,
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

/// Machine-readable exactness report for the bounded Hungarian-v0 compiled lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianCompiledExecutorExactnessReport {
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
    pub case_reports: Vec<TassadarHungarianCompiledExecutorCaseExactnessReport>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarHungarianCompiledExecutorExactnessReport {
    fn new(
        compiled_suite_artifact_digest: String,
        requested_decode_mode: TassadarExecutorDecodeMode,
        case_reports: Vec<TassadarHungarianCompiledExecutorCaseExactnessReport>,
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
                TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
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
            b"psionic_tassadar_hungarian_compiled_executor_exactness_report|",
            &report,
        );
        report
    }
}

/// Stable refusal surface expected from one mismatch check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHungarianCompiledExecutorRefusalKind {
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

/// One compatibility/refusal check for the bounded Hungarian-v0 compiled lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianCompiledExecutorRefusalCheckReport {
    /// Stable corpus case id owning the compiled deployment.
    pub deployment_case_id: String,
    /// Stable check id.
    pub check_id: String,
    /// Expected refusal kind for the check.
    pub expected_refusal_kind: TassadarHungarianCompiledExecutorRefusalKind,
    /// Observed refusal kind.
    pub observed_refusal_kind: TassadarHungarianCompiledExecutorRefusalKind,
    /// Whether the observed refusal matched the expectation exactly.
    pub matched_expected_refusal: bool,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Machine-readable compatibility/refusal report for the bounded Hungarian compiled lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianCompiledExecutorCompatibilityReport {
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
    pub check_reports: Vec<TassadarHungarianCompiledExecutorRefusalCheckReport>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarHungarianCompiledExecutorCompatibilityReport {
    fn new(
        compiled_suite_artifact_digest: String,
        check_reports: Vec<TassadarHungarianCompiledExecutorRefusalCheckReport>,
    ) -> Self {
        let total_check_count = check_reports.len() as u32;
        let matched_refusal_check_count = check_reports
            .iter()
            .filter(|check| check.matched_expected_refusal)
            .count() as u32;
        let mut report = Self {
            workload_family_id: String::from(
                TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            compiled_suite_artifact_digest,
            total_check_count,
            matched_refusal_check_count,
            matched_refusal_rate_bps: ratio_bps(matched_refusal_check_count, total_check_count),
            check_reports,
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_hungarian_compiled_executor_compatibility_report|",
            &report,
        );
        report
    }
}

/// Explicit lane-status taxonomy for the Hungarian Phase 18 closeout report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHungarianLaneClaimStatus {
    /// The lane has exact artifact-backed evidence.
    Exact,
    /// The lane is still research-only.
    ResearchOnly,
    /// The lane has not been implemented truthfully yet.
    NotDone,
}

/// Machine-readable split between learned and compiled Hungarian claim posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianLaneStatusReport {
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Learned-lane posture.
    pub learned_lane_status: TassadarHungarianLaneClaimStatus,
    /// Human-readable learned-lane detail.
    pub learned_lane_detail: String,
    /// Compiled/proof-backed lane posture.
    pub compiled_lane_status: TassadarHungarianLaneClaimStatus,
    /// Human-readable compiled-lane detail.
    pub compiled_lane_detail: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarHungarianLaneStatusReport {
    fn new() -> Self {
        let mut report = Self {
            workload_family_id: String::from(
                TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID,
            ),
            learned_lane_status: TassadarHungarianLaneClaimStatus::NotDone,
            learned_lane_detail: String::from(
                "no learned Hungarian-v0 executor lane exists yet; Phase 14 remains blocked on Sudoku",
            ),
            compiled_lane_status: TassadarHungarianLaneClaimStatus::Exact,
            compiled_lane_detail: String::from(
                "bounded compiled/proof-backed Hungarian-v0 lane is exact on the matched corpus",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_hungarian_lane_status_report|", &report);
        report
    }
}

/// Typed failure while building or evaluating the bounded Hungarian compiled lane.
#[derive(Debug, Error)]
pub enum TassadarHungarianCompiledExecutorEvalError {
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

/// Builds the bounded compiled-executor corpus for the real Hungarian-v0 family.
pub fn build_tassadar_hungarian_v0_compiled_executor_corpus(
    split_filter: Option<TassadarSudokuV0CorpusSplit>,
) -> Result<TassadarHungarianCompiledExecutorCorpus, TassadarHungarianCompiledExecutorEvalError> {
    let fixture = TassadarExecutorFixture::hungarian_v0_matching_v1();
    let mut cases = Vec::new();
    let mut artifacts = Vec::new();
    for corpus_case in tassadar_hungarian_v0_corpus() {
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
        cases.push(TassadarHungarianCompiledExecutorCorpusCase {
            case_id: corpus_case.case_id,
            split: corpus_case.split,
            cost_matrix: corpus_case.cost_matrix,
            optimal_assignment: corpus_case.optimal_assignment,
            optimal_cost: corpus_case.optimal_cost,
            program_artifact: artifact,
            compiled_executor,
        });
    }
    let compiled_suite_artifact = TassadarCompiledProgramSuiteArtifact::compile(
        "tassadar.hungarian_v0.compiled_executor_suite",
        "benchmark://tassadar/hungarian_v0_compiled_executor@v0",
        &fixture,
        artifacts.as_slice(),
    )?;
    Ok(TassadarHungarianCompiledExecutorCorpus {
        workload_family_id: String::from(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_WORKLOAD_FAMILY_ID),
        cases,
        compiled_suite_artifact,
    })
}

/// Benchmarks the bounded compiled Hungarian-v0 lane against CPU reference truth.
pub fn build_tassadar_hungarian_compiled_executor_exactness_report(
    corpus: &TassadarHungarianCompiledExecutorCorpus,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<
    TassadarHungarianCompiledExecutorExactnessReport,
    TassadarHungarianCompiledExecutorEvalError,
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
        case_reports.push(TassadarHungarianCompiledExecutorCaseExactnessReport {
            case_id: corpus_case.case_id.clone(),
            split: corpus_case.split,
            cost_matrix: corpus_case.cost_matrix.clone(),
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
    Ok(TassadarHungarianCompiledExecutorExactnessReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        requested_decode_mode,
        case_reports,
    ))
}

/// Builds a machine-readable refusal report for bounded compiled Hungarian deployments.
pub fn build_tassadar_hungarian_compiled_executor_compatibility_report(
    corpus: &TassadarHungarianCompiledExecutorCorpus,
) -> Result<
    TassadarHungarianCompiledExecutorCompatibilityReport,
    TassadarHungarianCompiledExecutorEvalError,
> {
    let mut check_reports = Vec::new();
    for (index, corpus_case) in corpus.cases.iter().enumerate() {
        let wrong_case = &corpus.cases[(index + 1) % corpus.cases.len()];
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            "wrong_program_artifact",
            TassadarHungarianCompiledExecutorRefusalKind::ProgramArtifactDigestMismatch,
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
            "wrong_wasm_profile",
            TassadarHungarianCompiledExecutorRefusalKind::WasmProfileMismatch,
            corpus_case.compiled_executor.execute(
                &wrong_profile_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));

        let mut wrong_trace_abi_artifact = corpus_case.program_artifact.clone();
        wrong_trace_abi_artifact.trace_abi_version = TassadarTraceAbi::hungarian_v0_matching_v1()
            .schema_version
            .saturating_add(1);
        check_reports.push(run_refusal_check(
            &corpus_case.case_id,
            "wrong_trace_abi_version",
            TassadarHungarianCompiledExecutorRefusalKind::TraceAbiVersionMismatch,
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
            TassadarHungarianCompiledExecutorRefusalKind::ProgramArtifactInconsistent,
            corpus_case.compiled_executor.execute(
                &inconsistent_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
        ));
    }
    Ok(TassadarHungarianCompiledExecutorCompatibilityReport::new(
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        check_reports,
    ))
}

/// Builds the machine-readable learned-vs-compiled Hungarian lane status report.
#[must_use]
pub fn build_tassadar_hungarian_lane_status_report() -> TassadarHungarianLaneStatusReport {
    TassadarHungarianLaneStatusReport::new()
}

fn run_refusal_check(
    deployment_case_id: &str,
    check_id: &str,
    expected_refusal_kind: TassadarHungarianCompiledExecutorRefusalKind,
    outcome: Result<TassadarCompiledProgramExecution, TassadarCompiledProgramError>,
) -> TassadarHungarianCompiledExecutorRefusalCheckReport {
    match outcome {
        Ok(_) => TassadarHungarianCompiledExecutorRefusalCheckReport {
            deployment_case_id: deployment_case_id.to_string(),
            check_id: check_id.to_string(),
            expected_refusal_kind,
            observed_refusal_kind: TassadarHungarianCompiledExecutorRefusalKind::UnexpectedSuccess,
            matched_expected_refusal: false,
            detail: String::from("compiled executor unexpectedly accepted mismatched artifact"),
        },
        Err(error) => {
            let observed_refusal_kind = refusal_kind_from_error(&error);
            TassadarHungarianCompiledExecutorRefusalCheckReport {
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
) -> TassadarHungarianCompiledExecutorRefusalKind {
    match error {
        TassadarCompiledProgramError::DescriptorContract { error } => match error {
            TassadarExecutorContractError::ProgramArtifactInconsistent { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::ProgramArtifactInconsistent
            }
            TassadarExecutorContractError::WasmProfileMismatch { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::WasmProfileMismatch
            }
            TassadarExecutorContractError::TraceAbiMismatch { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::TraceAbiMismatch
            }
            TassadarExecutorContractError::TraceAbiVersionMismatch { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::TraceAbiVersionMismatch
            }
            TassadarExecutorContractError::OpcodeVocabularyDigestMismatch { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::OpcodeVocabularyDigestMismatch
            }
            TassadarExecutorContractError::ProgramProfileMismatch { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::ProgramProfileMismatch
            }
            TassadarExecutorContractError::DecodeModeUnsupported { .. } => {
                TassadarHungarianCompiledExecutorRefusalKind::SelectionRefused
            }
        },
        TassadarCompiledProgramError::SelectionRefused { .. } => {
            TassadarHungarianCompiledExecutorRefusalKind::SelectionRefused
        }
        TassadarCompiledProgramError::ProgramArtifactDigestMismatch { .. } => {
            TassadarHungarianCompiledExecutorRefusalKind::ProgramArtifactDigestMismatch
        }
        TassadarCompiledProgramError::ProgramDigestMismatch { .. } => {
            TassadarHungarianCompiledExecutorRefusalKind::ProgramDigestMismatch
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
        .expect("Tassadar Hungarian compiled executor eval artifact should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_hungarian_compiled_executor_compatibility_report,
        build_tassadar_hungarian_compiled_executor_exactness_report,
        build_tassadar_hungarian_lane_status_report,
        build_tassadar_hungarian_v0_compiled_executor_corpus,
    };
    use psionic_runtime::{TassadarExecutorDecodeMode, TassadarSudokuV0CorpusSplit};

    #[test]
    fn compiled_executor_exactness_report_is_exact_for_hungarian_v0_validation_corpus()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_hungarian_v0_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let report = build_tassadar_hungarian_compiled_executor_exactness_report(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;

        assert_eq!(report.total_case_count, 2);
        assert_eq!(report.exact_trace_case_count, 2);
        assert_eq!(report.exact_trace_rate_bps, 10_000);
        assert_eq!(report.final_output_match_case_count, 2);
        assert_eq!(report.halt_match_case_count, 2);
        Ok(())
    }

    #[test]
    fn compiled_executor_compatibility_report_records_exact_refusals_for_hungarian_v0_corpus()
    -> Result<(), Box<dyn std::error::Error>> {
        let corpus = build_tassadar_hungarian_v0_compiled_executor_corpus(Some(
            TassadarSudokuV0CorpusSplit::Validation,
        ))?;
        let report = build_tassadar_hungarian_compiled_executor_compatibility_report(&corpus)?;

        assert_eq!(report.total_check_count, 8);
        assert_eq!(report.matched_refusal_check_count, 8);
        assert_eq!(report.matched_refusal_rate_bps, 10_000);
        Ok(())
    }

    #[test]
    fn hungarian_lane_status_report_separates_learned_and_compiled_claims() {
        let report = build_tassadar_hungarian_lane_status_report();

        assert_eq!(
            report.learned_lane_status,
            super::TassadarHungarianLaneClaimStatus::NotDone
        );
        assert_eq!(
            report.compiled_lane_status,
            super::TassadarHungarianLaneClaimStatus::Exact
        );
    }
}
