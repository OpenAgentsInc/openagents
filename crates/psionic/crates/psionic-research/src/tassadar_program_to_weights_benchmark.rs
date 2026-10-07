use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use psionic_eval::{
    TassadarCompiledKernelSuiteEvalError, TassadarHungarian10x10CompiledExecutorEvalError,
    TassadarSudoku9x9CompiledExecutorEvalError, build_tassadar_compiled_kernel_suite_corpus,
    build_tassadar_hungarian_10x10_compiled_executor_corpus,
    build_tassadar_sudoku_9x9_compiled_executor_corpus,
};
use psionic_models::{
    TassadarCompiledProgramError, TassadarCompiledProgramExecutor, TassadarExecutorFixture,
    TassadarTraceTokenizer,
};
use psionic_runtime::{
    TassadarExecutionEvidenceBundle, TassadarExecutorDecodeMode, TassadarExecutorExecutionReport,
    TassadarProgramArtifact, TassadarSudokuV0CorpusSplit,
    build_tassadar_execution_evidence_bundle, execute_tassadar_executor_request,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_REPORT_FILE: &str =
    "tassadar_program_to_weights_benchmark_suite.json";
pub const TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_program_to_weights_benchmark_suite.json";
pub const TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_program_to_weights_benchmark_suite";
pub const TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_TEST_COMMAND: &str =
    "cargo test -p psionic-research program_to_weights_benchmark_suite_covers_widened_workloads -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;
const DIRECT_PATH_KIND: &str = "direct_tokenized_reference_linear";
const COMPILED_PATH_KIND: &str = "program_specialized_compiled_weight_deployment";
const COMPILED_KERNEL_SUITE_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json";
const SUDOKU_9X9_COMPILED_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json";
const HUNGARIAN_10X10_COMPILED_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json";

#[derive(Clone, Debug)]
struct ProgramToWeightsCaseSpec {
    workload_group_id: String,
    workload_family_id: String,
    case_id: String,
    workload_variant_id: String,
    summary: String,
    program_artifact: TassadarProgramArtifact,
    compiled_executor: TassadarCompiledProgramExecutor,
}

#[derive(Clone, Debug)]
struct DirectProgramExecution {
    model_id: String,
    model_descriptor_digest: String,
    execution_report: TassadarExecutorExecutionReport,
    evidence_bundle: TassadarExecutionEvidenceBundle,
    token_trace_total_token_count: u64,
    token_trace_sequence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarProgramToWeightsBenchmarkCaseReport {
    pub workload_group_id: String,
    pub workload_family_id: String,
    pub case_id: String,
    pub workload_variant_id: String,
    pub summary: String,
    pub program_artifact_ref: String,
    pub program_artifact_digest: String,
    pub program_digest: String,
    pub program_artifact_bytes: u64,
    pub trace_step_count: u64,
    pub token_trace_total_token_count: u64,
    pub direct_path_kind: String,
    pub direct_requested_decode_mode: TassadarExecutorDecodeMode,
    pub direct_model_id: String,
    pub direct_model_descriptor_digest: String,
    pub direct_steps_per_second: f64,
    pub direct_trace_digest: String,
    pub direct_behavior_digest: String,
    pub direct_token_trace_sequence_digest: String,
    pub direct_runtime_manifest_digest: String,
    pub direct_trace_proof_digest: String,
    pub direct_execution_proof_bundle_digest: String,
    pub direct_trace_artifact_bytes: u64,
    pub direct_execution_proof_bundle_bytes: u64,
    pub direct_lineage_verified: bool,
    pub compiled_path_kind: String,
    pub compiled_model_id: String,
    pub compiled_model_descriptor_digest: String,
    pub compiled_steps_per_second: f64,
    pub compiled_over_direct_ratio: f64,
    pub compiled_weight_artifact_digest: String,
    pub compiled_weight_bundle_digest: String,
    pub compiled_weight_artifact_bytes: u64,
    pub compiled_weight_bundle_bytes: u64,
    pub compiled_weight_over_program_artifact_ratio: f64,
    pub runtime_contract_digest: String,
    pub compile_runtime_manifest_digest: String,
    pub compile_trace_proof_digest: String,
    pub compile_execution_proof_bundle_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
    pub compiled_runtime_manifest_digest: String,
    pub compiled_runtime_trace_proof_digest: String,
    pub compiled_trace_digest: String,
    pub compiled_behavior_digest: String,
    pub compiled_trace_artifact_bytes: u64,
    pub compiled_execution_proof_bundle_bytes: u64,
    pub compiled_lineage_verified: bool,
    pub shared_source_program_verified: bool,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub halt_match: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarProgramToWeightsBenchmarkFamilyReport {
    pub workload_family_id: String,
    pub total_case_count: u32,
    pub exact_trace_case_count: u32,
    pub final_output_match_case_count: u32,
    pub halt_match_case_count: u32,
    pub average_trace_step_count: u64,
    pub average_token_trace_total_token_count: u64,
    pub average_direct_steps_per_second: f64,
    pub average_compiled_steps_per_second: f64,
    pub average_compiled_over_direct_ratio: f64,
    pub average_program_artifact_bytes: u64,
    pub average_compiled_weight_artifact_bytes: u64,
    pub average_compiled_weight_bundle_bytes: u64,
    pub average_compiled_weight_over_program_artifact_ratio: f64,
    pub shared_source_programs_verified: bool,
    pub proof_lineage_verified: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarProgramToWeightsBenchmarkSuiteReport {
    pub schema_version: u16,
    pub benchmark_suite_id: String,
    pub report_ref: String,
    pub regeneration_commands: Vec<String>,
    pub compared_run_bundle_refs: Vec<String>,
    pub direct_path_kind: String,
    pub direct_requested_decode_mode: TassadarExecutorDecodeMode,
    pub compiled_path_kind: String,
    pub total_case_count: u32,
    pub exact_trace_case_count: u32,
    pub final_output_match_case_count: u32,
    pub halt_match_case_count: u32,
    pub shared_source_programs_verified: bool,
    pub proof_lineage_verified: bool,
    pub family_reports: Vec<TassadarProgramToWeightsBenchmarkFamilyReport>,
    pub case_reports: Vec<TassadarProgramToWeightsBenchmarkCaseReport>,
    pub claim_boundary: String,
    pub detail: String,
    pub report_digest: String,
}

impl TassadarProgramToWeightsBenchmarkSuiteReport {
    fn new(case_reports: Vec<TassadarProgramToWeightsBenchmarkCaseReport>) -> Self {
        let total_case_count = case_reports.len() as u32;
        let exact_trace_case_count = case_reports
            .iter()
            .filter(|case| case.exact_trace_match)
            .count() as u32;
        let final_output_match_case_count = case_reports
            .iter()
            .filter(|case| case.final_output_match)
            .count() as u32;
        let halt_match_case_count = case_reports.iter().filter(|case| case.halt_match).count() as u32;
        let shared_source_programs_verified = case_reports
            .iter()
            .all(|case| case.shared_source_program_verified);
        let proof_lineage_verified = case_reports.iter().all(|case| {
            case.direct_lineage_verified && case.compiled_lineage_verified
        });
        let family_reports = build_family_reports(case_reports.as_slice());
        let average_compiled_over_direct_ratio = round_metric(
            case_reports
                .iter()
                .map(|case| case.compiled_over_direct_ratio)
                .sum::<f64>()
                / total_case_count.max(1) as f64,
        );
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            benchmark_suite_id: String::from("tassadar.program_to_weights_benchmark_suite.v0"),
            report_ref: String::from(TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_REPORT_REF),
            regeneration_commands: vec![
                String::from(TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_EXAMPLE_COMMAND),
                String::from(TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_TEST_COMMAND),
            ],
            compared_run_bundle_refs: vec![
                String::from(COMPILED_KERNEL_SUITE_RUN_BUNDLE_REF),
                String::from(SUDOKU_9X9_COMPILED_RUN_BUNDLE_REF),
                String::from(HUNGARIAN_10X10_COMPILED_RUN_BUNDLE_REF),
            ],
            direct_path_kind: String::from(DIRECT_PATH_KIND),
            direct_requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            compiled_path_kind: String::from(COMPILED_PATH_KIND),
            total_case_count,
            exact_trace_case_count,
            final_output_match_case_count,
            halt_match_case_count,
            shared_source_programs_verified,
            proof_lineage_verified,
            family_reports,
            case_reports,
            claim_boundary: String::from(
                "this suite compares today's exact direct reference-linear executor path against today's exact program-specialized compiled-weight deployment path on the same committed Wasm workloads; it preserves exactness, artifact size, and lineage workload by workload and does not imply that compiled weights are already a generally faster runtime",
            ),
            detail: format!(
                "All compared cases keep shared source-program identity explicit, exact_trace_case_count={exact_trace_case_count}/{total_case_count}, proof_lineage_verified={proof_lineage_verified}, and average compiled-over-direct throughput ratio={average_compiled_over_direct_ratio}; this is workload-specific evidence, not a universal speedup claim."
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_program_to_weights_benchmark_suite|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarProgramToWeightsBenchmarkError {
    #[error(transparent)]
    KernelSuiteEval(#[from] TassadarCompiledKernelSuiteEvalError),
    #[error(transparent)]
    SudokuEval(#[from] TassadarSudoku9x9CompiledExecutorEvalError),
    #[error(transparent)]
    HungarianEval(#[from] TassadarHungarian10x10CompiledExecutorEvalError),
    #[error(transparent)]
    Compiled(#[from] TassadarCompiledProgramError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("unsupported direct executor profile `{profile_id}` for case `{case_id}`")]
    UnsupportedDirectProfile { case_id: String, profile_id: String },
    #[error("direct executor selection refused `{case_id}`: {detail}")]
    DirectSelectionRefused { case_id: String, detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_program_to_weights_benchmark_suite()
-> Result<TassadarProgramToWeightsBenchmarkSuiteReport, TassadarProgramToWeightsBenchmarkError> {
    let case_specs = collect_case_specs()?;
    let mut case_reports = Vec::with_capacity(case_specs.len());
    for spec in &case_specs {
        case_reports.push(build_case_report(spec)?);
    }
    Ok(TassadarProgramToWeightsBenchmarkSuiteReport::new(case_reports))
}

pub fn run_tassadar_program_to_weights_benchmark_suite(
    output_dir: &Path,
) -> Result<TassadarProgramToWeightsBenchmarkSuiteReport, TassadarProgramToWeightsBenchmarkError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarProgramToWeightsBenchmarkError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_program_to_weights_benchmark_suite()?;
    write_json(
        output_dir.join(TASSADAR_PROGRAM_TO_WEIGHTS_BENCHMARK_REPORT_FILE),
        &report,
    )?;
    Ok(report)
}

fn collect_case_specs() -> Result<Vec<ProgramToWeightsCaseSpec>, TassadarProgramToWeightsBenchmarkError>
{
    let mut case_specs = Vec::new();

    let kernel_corpus = build_tassadar_compiled_kernel_suite_corpus()?;
    for case in kernel_corpus.cases {
        case_specs.push(ProgramToWeightsCaseSpec {
            workload_group_id: String::from("compiled_kernel_suite"),
            workload_family_id: case.family_id.as_str().to_string(),
            case_id: case.case_id,
            workload_variant_id: case.regime_id,
            summary: format!(
                "{} ({}={})",
                case.summary, case.length_parameter_name, case.length_parameter_value
            ),
            program_artifact: case.program_artifact,
            compiled_executor: case.compiled_executor,
        });
    }

    let sudoku_corpus = build_tassadar_sudoku_9x9_compiled_executor_corpus(Some(
        TassadarSudokuV0CorpusSplit::Validation,
    ))?;
    for case in sudoku_corpus.cases {
        case_specs.push(ProgramToWeightsCaseSpec {
            workload_group_id: String::from("sudoku_9x9_compiled_executor"),
            workload_family_id: String::from("sudoku_search_9x9"),
            case_id: case.case_id,
            workload_variant_id: case.split.as_str().to_string(),
            summary: format!("sudoku_9x9 given_count={}", case.given_count),
            program_artifact: case.program_artifact,
            compiled_executor: case.compiled_executor,
        });
    }

    let hungarian_corpus = build_tassadar_hungarian_10x10_compiled_executor_corpus(Some(
        TassadarSudokuV0CorpusSplit::Validation,
    ))?;
    for case in hungarian_corpus.cases {
        case_specs.push(ProgramToWeightsCaseSpec {
            workload_group_id: String::from("hungarian_10x10_compiled_executor"),
            workload_family_id: String::from("hungarian_matching_10x10"),
            case_id: case.case_id,
            workload_variant_id: case.split.as_str().to_string(),
            summary: format!(
                "hungarian_10x10 optimal_cost={} search_rows={}",
                case.optimal_cost,
                case.search_row_order.len()
            ),
            program_artifact: case.program_artifact,
            compiled_executor: case.compiled_executor,
        });
    }

    Ok(case_specs)
}

fn build_case_report(
    spec: &ProgramToWeightsCaseSpec,
) -> Result<TassadarProgramToWeightsBenchmarkCaseReport, TassadarProgramToWeightsBenchmarkError> {
    let direct_execution = run_direct_execution(spec)?;
    let trace_step_count = direct_execution.execution_report.execution.steps.len() as u64;
    let direct_steps_per_second = round_metric(benchmark_steps_per_second(trace_step_count, || {
        run_direct_execution(spec).map(|_| ())
    })?);
    let compiled_execution = spec
        .compiled_executor
        .execute(&spec.program_artifact, TassadarExecutorDecodeMode::ReferenceLinear)?;
    let compiled_steps_per_second = round_metric(benchmark_steps_per_second(trace_step_count, || {
        spec.compiled_executor
            .execute(&spec.program_artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .map(|_| ())
            .map_err(TassadarProgramToWeightsBenchmarkError::from)
    })?);

    let compiled_descriptor_digest = spec.compiled_executor.descriptor().stable_digest();
    let program_artifact_bytes = serialized_size(&spec.program_artifact)?;
    let compiled_weight_artifact = spec.compiled_executor.compiled_weight_artifact();
    let compiled_weight_artifact_bytes = compiled_weight_artifact.compiled_weight_artifact_bytes;
    let compiled_weight_bundle_bytes = compiled_weight_bundle_bytes(spec.compiled_executor.weight_bundle())?;
    let compiled_weight_over_program_artifact_ratio = round_metric(
        compiled_weight_artifact_bytes as f64 / program_artifact_bytes.max(1) as f64,
    );

    let direct_lineage_verified = direct_execution
        .evidence_bundle
        .trace_proof
        .program_artifact_digest
        == spec.program_artifact.artifact_digest
        && direct_execution.evidence_bundle.trace_proof.program_digest
            == spec.program_artifact.validated_program_digest
        && direct_execution.evidence_bundle.trace_proof.model_descriptor_digest
            == direct_execution.model_descriptor_digest
        && direct_execution.evidence_bundle.trace_proof.runtime_manifest_digest
            == direct_execution.evidence_bundle.runtime_manifest.manifest_digest;
    let compiled_lineage_verified = compiled_weight_artifact.program_artifact_digest
        == spec.program_artifact.artifact_digest
        && compiled_weight_artifact.program_digest == spec.program_artifact.validated_program_digest
        && spec.compiled_executor.runtime_contract().program_artifact_digest
            == spec.program_artifact.artifact_digest
        && spec.compiled_executor.runtime_contract().program_digest
            == spec.program_artifact.validated_program_digest
        && compiled_weight_artifact.compile_runtime_manifest_digest
            == spec
                .compiled_executor
                .compile_evidence_bundle()
                .runtime_manifest
                .manifest_digest
        && compiled_weight_artifact.compile_trace_proof_digest
            == spec.compiled_executor.compile_evidence_bundle().trace_proof.proof_digest
        && compiled_weight_artifact.compile_execution_proof_bundle_digest
            == spec
                .compiled_executor
                .compile_evidence_bundle()
                .proof_bundle
                .stable_digest()
        && compiled_execution.evidence_bundle.trace_proof.program_artifact_digest
            == spec.program_artifact.artifact_digest
        && compiled_execution.evidence_bundle.trace_proof.program_digest
            == spec.program_artifact.validated_program_digest
        && compiled_execution.evidence_bundle.trace_proof.model_descriptor_digest
            == compiled_descriptor_digest
        && compiled_execution.evidence_bundle.trace_proof.runtime_manifest_digest
            == compiled_execution.evidence_bundle.runtime_manifest.manifest_digest;
    let shared_source_program_verified = direct_execution
        .evidence_bundle
        .trace_proof
        .program_artifact_digest
        == compiled_weight_artifact.program_artifact_digest
        && direct_execution.evidence_bundle.trace_proof.program_digest == compiled_weight_artifact.program_digest
        && spec.compiled_executor.runtime_contract().program_digest
            == spec.program_artifact.validated_program_digest;

    let exact_trace_match = direct_execution.execution_report.execution.trace_digest()
        == compiled_execution.execution_report.execution.trace_digest();
    let final_output_match = direct_execution.execution_report.execution.outputs
        == compiled_execution.execution_report.execution.outputs;
    let halt_match = direct_execution.execution_report.execution.halt_reason
        == compiled_execution.execution_report.execution.halt_reason;

    Ok(TassadarProgramToWeightsBenchmarkCaseReport {
        workload_group_id: spec.workload_group_id.clone(),
        workload_family_id: spec.workload_family_id.clone(),
        case_id: spec.case_id.clone(),
        workload_variant_id: spec.workload_variant_id.clone(),
        summary: spec.summary.clone(),
        program_artifact_ref: spec.program_artifact.artifact_id.clone(),
        program_artifact_digest: spec.program_artifact.artifact_digest.clone(),
        program_digest: spec.program_artifact.validated_program_digest.clone(),
        program_artifact_bytes,
        trace_step_count,
        token_trace_total_token_count: direct_execution.token_trace_total_token_count,
        direct_path_kind: String::from(DIRECT_PATH_KIND),
        direct_requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        direct_model_id: direct_execution.model_id,
        direct_model_descriptor_digest: direct_execution.model_descriptor_digest,
        direct_steps_per_second,
        direct_trace_digest: direct_execution.execution_report.execution.trace_digest(),
        direct_behavior_digest: direct_execution.execution_report.execution.behavior_digest(),
        direct_token_trace_sequence_digest: direct_execution.token_trace_sequence_digest,
        direct_runtime_manifest_digest: direct_execution
            .evidence_bundle
            .runtime_manifest
            .manifest_digest
            .clone(),
        direct_trace_proof_digest: direct_execution.evidence_bundle.trace_proof.proof_digest.clone(),
        direct_execution_proof_bundle_digest: direct_execution
            .evidence_bundle
            .proof_bundle
            .stable_digest(),
        direct_trace_artifact_bytes: serialized_size(&direct_execution.evidence_bundle.trace_artifact)?,
        direct_execution_proof_bundle_bytes: serialized_size(
            &direct_execution.evidence_bundle.proof_bundle,
        )?,
        direct_lineage_verified,
        compiled_path_kind: String::from(COMPILED_PATH_KIND),
        compiled_model_id: spec.compiled_executor.descriptor().model.model_id.clone(),
        compiled_model_descriptor_digest: compiled_descriptor_digest,
        compiled_steps_per_second,
        compiled_over_direct_ratio: round_metric(
            compiled_steps_per_second / direct_steps_per_second.max(1e-9),
        ),
        compiled_weight_artifact_digest: compiled_weight_artifact.artifact_digest.clone(),
        compiled_weight_bundle_digest: compiled_weight_artifact.compiled_weight_bundle_digest.clone(),
        compiled_weight_artifact_bytes,
        compiled_weight_bundle_bytes,
        compiled_weight_over_program_artifact_ratio,
        runtime_contract_digest: spec.compiled_executor.runtime_contract().contract_digest.clone(),
        compile_runtime_manifest_digest: compiled_weight_artifact
            .compile_runtime_manifest_digest
            .clone(),
        compile_trace_proof_digest: compiled_weight_artifact.compile_trace_proof_digest.clone(),
        compile_execution_proof_bundle_digest: compiled_weight_artifact
            .compile_execution_proof_bundle_digest
            .clone(),
        runtime_execution_proof_bundle_digest: compiled_execution
            .evidence_bundle
            .proof_bundle
            .stable_digest(),
        compiled_runtime_manifest_digest: compiled_execution
            .evidence_bundle
            .runtime_manifest
            .manifest_digest
            .clone(),
        compiled_runtime_trace_proof_digest: compiled_execution
            .evidence_bundle
            .trace_proof
            .proof_digest
            .clone(),
        compiled_trace_digest: compiled_execution.execution_report.execution.trace_digest(),
        compiled_behavior_digest: compiled_execution.execution_report.execution.behavior_digest(),
        compiled_trace_artifact_bytes: serialized_size(&compiled_execution.evidence_bundle.trace_artifact)?,
        compiled_execution_proof_bundle_bytes: serialized_size(
            &compiled_execution.evidence_bundle.proof_bundle,
        )?,
        compiled_lineage_verified,
        shared_source_program_verified,
        exact_trace_match,
        final_output_match,
        halt_match,
    })
}

fn run_direct_execution(
    spec: &ProgramToWeightsCaseSpec,
) -> Result<DirectProgramExecution, TassadarProgramToWeightsBenchmarkError> {
    let Some(fixture) = TassadarExecutorFixture::for_profile_id(
        spec.program_artifact.wasm_profile_id.as_str(),
    ) else {
        return Err(TassadarProgramToWeightsBenchmarkError::UnsupportedDirectProfile {
            case_id: spec.case_id.clone(),
            profile_id: spec.program_artifact.wasm_profile_id.clone(),
        });
    };

    let descriptor = fixture.descriptor();
    let model_descriptor_digest = descriptor.stable_digest();
    let execution_report = execute_tassadar_executor_request(
        &spec.program_artifact.validated_program,
        TassadarExecutorDecodeMode::ReferenceLinear,
        spec.program_artifact.trace_abi_version,
        Some(descriptor.compatibility.supported_decode_modes.as_slice()),
    )
    .map_err(|diagnostic| TassadarProgramToWeightsBenchmarkError::DirectSelectionRefused {
        case_id: spec.case_id.clone(),
        detail: diagnostic.detail,
    })?;
    let evidence_bundle = build_tassadar_execution_evidence_bundle(
        format!(
            "program-to-weights-direct-{}-{}",
            spec.workload_group_id, spec.case_id
        ),
        stable_digest(
            b"psionic_tassadar_program_to_weights_direct_request|",
            &(
                spec.case_id.as_str(),
                spec.program_artifact.artifact_digest.as_str(),
                model_descriptor_digest.as_str(),
            ),
        ),
        "psionic.tassadar.program_to_weights.direct_executor",
        descriptor.model.model_id.clone(),
        model_descriptor_digest.clone(),
        vec![format!(
            "benchmark://tassadar/program_to_weights/{}",
            spec.workload_group_id
        )],
        &spec.program_artifact,
        TassadarExecutorDecodeMode::ReferenceLinear,
        &execution_report.execution,
    );
    let tokenizer = TassadarTraceTokenizer::new();
    let tokenized = tokenizer.tokenize_program_and_execution(
        &spec.program_artifact.validated_program,
        &execution_report.execution,
    );

    Ok(DirectProgramExecution {
        model_id: descriptor.model.model_id.clone(),
        model_descriptor_digest,
        execution_report,
        evidence_bundle,
        token_trace_total_token_count: tokenized.sequence.as_slice().len() as u64,
        token_trace_sequence_digest: tokenized.sequence_digest,
    })
}

fn build_family_reports(
    case_reports: &[TassadarProgramToWeightsBenchmarkCaseReport],
) -> Vec<TassadarProgramToWeightsBenchmarkFamilyReport> {
    let mut grouped = BTreeMap::<String, Vec<&TassadarProgramToWeightsBenchmarkCaseReport>>::new();
    for case in case_reports {
        grouped
            .entry(case.workload_family_id.clone())
            .or_default()
            .push(case);
    }

    grouped
        .into_iter()
        .map(|(workload_family_id, reports)| {
            let total_case_count = reports.len() as u32;
            let exact_trace_case_count = reports
                .iter()
                .filter(|case| case.exact_trace_match)
                .count() as u32;
            let final_output_match_case_count = reports
                .iter()
                .filter(|case| case.final_output_match)
                .count() as u32;
            let halt_match_case_count = reports.iter().filter(|case| case.halt_match).count() as u32;
            let shared_source_programs_verified = reports
                .iter()
                .all(|case| case.shared_source_program_verified);
            let proof_lineage_verified = reports.iter().all(|case| {
                case.direct_lineage_verified && case.compiled_lineage_verified
            });
            let average_trace_step_count = average_u64(
                reports.iter().map(|case| case.trace_step_count).collect::<Vec<_>>().as_slice(),
            );
            let average_token_trace_total_token_count = average_u64(
                reports
                    .iter()
                    .map(|case| case.token_trace_total_token_count)
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let average_direct_steps_per_second = round_metric(
                reports
                    .iter()
                    .map(|case| case.direct_steps_per_second)
                    .sum::<f64>()
                    / total_case_count.max(1) as f64,
            );
            let average_compiled_steps_per_second = round_metric(
                reports
                    .iter()
                    .map(|case| case.compiled_steps_per_second)
                    .sum::<f64>()
                    / total_case_count.max(1) as f64,
            );
            let average_compiled_over_direct_ratio = round_metric(
                reports
                    .iter()
                    .map(|case| case.compiled_over_direct_ratio)
                    .sum::<f64>()
                    / total_case_count.max(1) as f64,
            );
            let average_program_artifact_bytes = average_u64(
                reports
                    .iter()
                    .map(|case| case.program_artifact_bytes)
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let average_compiled_weight_artifact_bytes = average_u64(
                reports
                    .iter()
                    .map(|case| case.compiled_weight_artifact_bytes)
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let average_compiled_weight_bundle_bytes = average_u64(
                reports
                    .iter()
                    .map(|case| case.compiled_weight_bundle_bytes)
                    .collect::<Vec<_>>()
                    .as_slice(),
            );
            let average_compiled_weight_over_program_artifact_ratio = round_metric(
                reports
                    .iter()
                    .map(|case| case.compiled_weight_over_program_artifact_ratio)
                    .sum::<f64>()
                    / total_case_count.max(1) as f64,
            );

            TassadarProgramToWeightsBenchmarkFamilyReport {
                workload_family_id: workload_family_id.clone(),
                total_case_count,
                exact_trace_case_count,
                final_output_match_case_count,
                halt_match_case_count,
                average_trace_step_count,
                average_token_trace_total_token_count,
                average_direct_steps_per_second,
                average_compiled_steps_per_second,
                average_compiled_over_direct_ratio,
                average_program_artifact_bytes,
                average_compiled_weight_artifact_bytes,
                average_compiled_weight_bundle_bytes,
                average_compiled_weight_over_program_artifact_ratio,
                shared_source_programs_verified,
                proof_lineage_verified,
                detail: format!(
                    "exact_trace_case_count={exact_trace_case_count}/{total_case_count}; average_compiled_over_direct_ratio={average_compiled_over_direct_ratio}; average_program_artifact_bytes={average_program_artifact_bytes}; average_compiled_weight_artifact_bytes={average_compiled_weight_artifact_bytes}; shared_source_programs_verified={shared_source_programs_verified}; proof_lineage_verified={proof_lineage_verified}"
                ),
            }
        })
        .collect()
}

fn benchmark_steps_per_second<F>(
    steps_per_run: u64,
    mut runner: F,
) -> Result<f64, TassadarProgramToWeightsBenchmarkError>
where
    F: FnMut() -> Result<(), TassadarProgramToWeightsBenchmarkError>,
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
            return Ok(total_steps as f64 / elapsed.max(1e-9));
        }
    }
}

fn compiled_weight_bundle_bytes(
    executor: &psionic_models::TassadarCompiledProgramWeightBundle,
) -> Result<u64, TassadarProgramToWeightsBenchmarkError> {
    let artifact_bytes: u64 = executor
        .metadata()
        .artifacts
        .iter()
        .map(|artifact| artifact.byte_length)
        .sum();
    if artifact_bytes > 0 {
        Ok(artifact_bytes)
    } else {
        serialized_size(executor)
    }
}

fn average_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().sum::<u64>() / values.len() as u64
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn serialized_size<T: Serialize>(
    value: &T,
) -> Result<u64, TassadarProgramToWeightsBenchmarkError> {
    Ok(serde_json::to_vec(value)?.len() as u64)
}

fn write_json<T: Serialize>(
    path: PathBuf,
    value: &T,
) -> Result<(), TassadarProgramToWeightsBenchmarkError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| TassadarProgramToWeightsBenchmarkError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("program-to-weights report value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::build_tassadar_program_to_weights_benchmark_suite;

    #[test]
    fn program_to_weights_benchmark_suite_covers_widened_workloads()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_program_to_weights_benchmark_suite()?;
        assert_eq!(report.total_case_count, 14);
        assert_eq!(report.exact_trace_case_count, 14);
        assert!(report.shared_source_programs_verified);
        assert!(report.proof_lineage_verified);

        let family_ids = report
            .family_reports
            .iter()
            .map(|family| family.workload_family_id.as_str())
            .collect::<Vec<_>>();
        assert!(family_ids.contains(&"arithmetic_kernel"));
        assert!(family_ids.contains(&"memory_update_kernel"));
        assert!(family_ids.contains(&"forward_branch_kernel"));
        assert!(family_ids.contains(&"backward_loop_kernel"));
        assert!(family_ids.contains(&"sudoku_search_9x9"));
        assert!(family_ids.contains(&"hungarian_matching_10x10"));
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| case.compiled_steps_per_second > 0.0)
        );
        assert!(
            report
                .case_reports
                .iter()
                .all(|case| case.direct_steps_per_second > 0.0)
        );
        Ok(())
    }
}
