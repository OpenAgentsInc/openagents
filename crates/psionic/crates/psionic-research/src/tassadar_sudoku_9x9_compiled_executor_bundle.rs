use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarReferenceFixtureSuite, TassadarSudoku9x9CompiledExecutorBenchmarkReceipt,
    TassadarSudoku9x9CompiledExecutorCompatibilityReport, TassadarSudoku9x9CompiledExecutorCorpus,
    TassadarSudoku9x9CompiledExecutorEvalError, TassadarSudoku9x9CompiledExecutorExactnessReport,
    build_tassadar_sudoku_9x9_compiled_executor_benchmark_receipt,
    build_tassadar_sudoku_9x9_compiled_executor_compatibility_report,
    build_tassadar_sudoku_9x9_compiled_executor_corpus,
    build_tassadar_sudoku_9x9_compiled_executor_exactness_report, build_tassadar_sudoku_9x9_suite,
};
use psionic_models::{TassadarTraceTokenizer, TokenizerBoundary};
use psionic_runtime::{
    TassadarClaimClass, TassadarExecution, TassadarExecutorDecodeMode, TassadarInstruction,
    TassadarTraceEvent,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the exact compiled Sudoku-9x9 lane.
pub const TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0";
/// Top-level benchmark package file.
pub const TASSADAR_SUDOKU_9X9_BENCHMARK_PACKAGE_FILE: &str = "benchmark_package.json";
/// Top-level environment bundle file.
pub const TASSADAR_SUDOKU_9X9_ENVIRONMENT_BUNDLE_FILE: &str = "environment_bundle.json";
/// Top-level exactness report file.
pub const TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE: &str =
    "compiled_executor_exactness_report.json";
/// Top-level compatibility/refusal report file.
pub const TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE: &str =
    "compiled_executor_compatibility_report.json";
/// Top-level throughput receipt file.
pub const TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_BENCHMARK_RECEIPT_FILE: &str =
    "throughput_benchmark_receipt.json";
/// Top-level compiled suite artifact file.
pub const TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE: &str =
    "compiled_weight_suite_artifact.json";

const DEPLOYMENTS_DIR: &str = "deployments";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const PROGRAM_ARTIFACT_FILE: &str = "program_artifact.json";
const COMPILED_WEIGHT_ARTIFACT_FILE: &str = "compiled_weight_artifact.json";
const RUNTIME_CONTRACT_FILE: &str = "runtime_contract.json";
const COMPILED_WEIGHT_BUNDLE_FILE: &str = "compiled_weight_bundle.json";
const COMPILE_EVIDENCE_BUNDLE_FILE: &str = "compile_evidence_bundle.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";
const RUNTIME_EXECUTION_PROOF_BUNDLE_FILE: &str = "runtime_execution_proof_bundle.json";
const RUNTIME_TRACE_PROOF_FILE: &str = "runtime_trace_proof.json";
const TOKEN_TRACE_SUMMARY_FILE: &str = "token_trace_summary.json";
const READABLE_LOG_FILE: &str = "readable_log.txt";
const TOKEN_TRACE_WINDOW_TOKENS: usize = 128;
const TOKEN_TRACE_SUMMARY_SCHEMA_VERSION: u16 = 1;

const fn default_compiled_article_class_claim_class() -> TassadarClaimClass {
    TassadarClaimClass::CompiledArticleClass
}

/// Compact token-trace artifact for one exact compiled Sudoku-9x9 execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9TokenTraceSummary {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable case id.
    pub case_id: String,
    /// Stable tokenizer digest.
    pub tokenizer_digest: String,
    /// Stable sequence digest over the token ids.
    pub sequence_digest: String,
    /// Prompt token count.
    pub prompt_token_count: usize,
    /// Target token count.
    pub target_token_count: usize,
    /// Total token count.
    pub total_token_count: usize,
    /// Output values recovered from the tokenized trace.
    pub extracted_output_values: Vec<i32>,
    /// Halt marker recovered from the tokenized trace.
    pub halt_marker: Option<String>,
    /// Leading prompt-token ids.
    pub first_prompt_token_ids: Vec<u32>,
    /// Leading prompt-token decode.
    pub first_prompt_symbolic_window: String,
    /// Trailing trace-token ids.
    pub last_trace_token_ids: Vec<u32>,
    /// Trailing trace-token decode.
    pub last_trace_symbolic_window: String,
    /// Stable summary digest.
    pub summary_digest: String,
}

impl TassadarSudoku9x9TokenTraceSummary {
    fn new(
        case_id: &str,
        tokenizer: &TassadarTraceTokenizer,
        program: &psionic_runtime::TassadarProgram,
        execution: &TassadarExecution,
    ) -> Self {
        let tokenized = tokenizer.tokenize_program_and_execution(program, execution);
        let token_slice = tokenized.sequence.as_slice();
        let prompt_window_len = tokenized.prompt_token_count.min(TOKEN_TRACE_WINDOW_TOKENS);
        let trace_start = tokenized.prompt_token_count.min(token_slice.len());
        let trace_window_start =
            trace_start.max(token_slice.len().saturating_sub(TOKEN_TRACE_WINDOW_TOKENS));
        let first_prompt_tokens = &token_slice[..prompt_window_len];
        let last_trace_tokens = &token_slice[trace_window_start..];

        let mut summary = Self {
            schema_version: TOKEN_TRACE_SUMMARY_SCHEMA_VERSION,
            case_id: case_id.to_string(),
            tokenizer_digest: tokenizer.stable_digest(),
            sequence_digest: tokenized.sequence_digest.clone(),
            prompt_token_count: tokenized.prompt_token_count,
            target_token_count: tokenized.target_token_count,
            total_token_count: token_slice.len(),
            extracted_output_values: tokenizer.extract_output_values(token_slice),
            halt_marker: tokenizer.extract_halt_marker(token_slice),
            first_prompt_token_ids: first_prompt_tokens
                .iter()
                .map(|token| token.as_u32())
                .collect(),
            first_prompt_symbolic_window: tokenizer.decode(first_prompt_tokens),
            last_trace_token_ids: last_trace_tokens
                .iter()
                .map(|token| token.as_u32())
                .collect(),
            last_trace_symbolic_window: tokenizer.decode(last_trace_tokens),
            summary_digest: String::new(),
        };
        summary.summary_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_token_trace_summary|",
            &summary,
        );
        summary
    }
}

/// Persisted per-case compiled deployment bundle for one Sudoku-9x9 program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorDeploymentBundle {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable split name.
    pub split: String,
    /// Relative deployment directory.
    pub deployment_dir: String,
    /// Relative source program-artifact file.
    pub program_artifact_file: String,
    /// Relative compiled-weight artifact file.
    pub compiled_weight_artifact_file: String,
    /// Relative runtime-contract file.
    pub runtime_contract_file: String,
    /// Relative compiled weight-bundle file.
    pub compiled_weight_bundle_file: String,
    /// Relative compile-evidence-bundle file.
    pub compile_evidence_bundle_file: String,
    /// Relative model-descriptor file.
    pub model_descriptor_file: String,
    /// Relative runtime execution proof-bundle file.
    pub runtime_execution_proof_bundle_file: String,
    /// Relative runtime trace-proof file.
    pub runtime_trace_proof_file: String,
    /// Relative compact token-trace summary file.
    pub token_trace_summary_file: String,
    /// Relative readable-log file.
    pub readable_log_file: String,
    /// Stable compiled-weight artifact digest.
    pub compiled_weight_artifact_digest: String,
    /// Stable runtime-contract digest.
    pub runtime_contract_digest: String,
    /// Stable compile proof-bundle digest.
    pub compile_execution_proof_bundle_digest: String,
    /// Stable runtime execution proof-bundle digest.
    pub runtime_execution_proof_bundle_digest: String,
    /// Stable runtime trace-proof digest.
    pub runtime_trace_proof_digest: String,
    /// Stable token-trace summary digest.
    pub token_trace_summary_digest: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarSudoku9x9CompiledExecutorDeploymentBundle {
    #[allow(clippy::too_many_arguments)]
    fn new(
        case_id: &str,
        split: &str,
        deployment_dir: &str,
        compiled_weight_artifact_digest: String,
        runtime_contract_digest: String,
        compile_execution_proof_bundle_digest: String,
        runtime_execution_proof_bundle_digest: String,
        runtime_trace_proof_digest: String,
        token_trace_summary_digest: String,
    ) -> Self {
        let mut bundle = Self {
            case_id: case_id.to_string(),
            split: split.to_string(),
            deployment_dir: deployment_dir.to_string(),
            program_artifact_file: String::from(PROGRAM_ARTIFACT_FILE),
            compiled_weight_artifact_file: String::from(COMPILED_WEIGHT_ARTIFACT_FILE),
            runtime_contract_file: String::from(RUNTIME_CONTRACT_FILE),
            compiled_weight_bundle_file: String::from(COMPILED_WEIGHT_BUNDLE_FILE),
            compile_evidence_bundle_file: String::from(COMPILE_EVIDENCE_BUNDLE_FILE),
            model_descriptor_file: String::from(MODEL_DESCRIPTOR_FILE),
            runtime_execution_proof_bundle_file: String::from(RUNTIME_EXECUTION_PROOF_BUNDLE_FILE),
            runtime_trace_proof_file: String::from(RUNTIME_TRACE_PROOF_FILE),
            token_trace_summary_file: String::from(TOKEN_TRACE_SUMMARY_FILE),
            readable_log_file: String::from(READABLE_LOG_FILE),
            compiled_weight_artifact_digest,
            runtime_contract_digest,
            compile_execution_proof_bundle_digest,
            runtime_execution_proof_bundle_digest,
            runtime_trace_proof_digest,
            token_trace_summary_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_compiled_executor_deployment_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Top-level persisted bundle for the exact compiled Sudoku-9x9 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CompiledExecutorRunBundle {
    /// Stable run id.
    pub run_id: String,
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Coarse claim class.
    #[serde(default = "default_compiled_article_class_claim_class")]
    pub claim_class: TassadarClaimClass,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Serving posture for the lane.
    pub serve_posture: String,
    /// Requested decode mode used for the exactness run.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Relative benchmark package file.
    pub benchmark_package_file: String,
    /// Relative environment bundle file.
    pub environment_bundle_file: String,
    /// Relative exactness report file.
    pub exactness_report_file: String,
    /// Relative compatibility/refusal report file.
    pub compatibility_report_file: String,
    /// Relative throughput benchmark receipt file.
    pub throughput_benchmark_receipt_file: String,
    /// Relative compiled suite-artifact file.
    pub compiled_suite_artifact_file: String,
    /// Ordered per-case deployment bundles.
    pub deployments: Vec<TassadarSudoku9x9CompiledExecutorDeploymentBundle>,
    /// Stable benchmark package digest.
    pub benchmark_package_digest: String,
    /// Stable environment bundle digest.
    pub environment_bundle_digest: String,
    /// Stable exactness-report digest.
    pub exactness_report_digest: String,
    /// Stable compatibility-report digest.
    pub compatibility_report_digest: String,
    /// Stable throughput benchmark receipt digest.
    pub throughput_benchmark_receipt_digest: String,
    /// Stable compiled suite-artifact digest.
    pub compiled_suite_artifact_digest: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarSudoku9x9CompiledExecutorRunBundle {
    fn new(
        suite: &TassadarReferenceFixtureSuite,
        exactness_report: &TassadarSudoku9x9CompiledExecutorExactnessReport,
        compatibility_report: &TassadarSudoku9x9CompiledExecutorCompatibilityReport,
        throughput_benchmark_receipt: &TassadarSudoku9x9CompiledExecutorBenchmarkReceipt,
        compiled_suite_artifact_digest: String,
        deployments: Vec<TassadarSudoku9x9CompiledExecutorDeploymentBundle>,
    ) -> Self {
        let mut bundle = Self {
            run_id: String::from("tassadar-sudoku-9x9-v0-compiled-executor-v0"),
            workload_family_id: exactness_report.workload_family_id.clone(),
            claim_class: TassadarClaimClass::CompiledArticleClass,
            claim_boundary: String::from(
                "exact compiled/proof-backed Sudoku-9x9 lane on the matched 9x9 corpus with benchmark, proof, readable-log, and compact token-trace artifacts; this is the article-sized Sudoku closure, not full compiled article parity by itself",
            ),
            serve_posture: String::from("eval_only"),
            requested_decode_mode: exactness_report.requested_decode_mode,
            benchmark_package_file: String::from(TASSADAR_SUDOKU_9X9_BENCHMARK_PACKAGE_FILE),
            environment_bundle_file: String::from(TASSADAR_SUDOKU_9X9_ENVIRONMENT_BUNDLE_FILE),
            exactness_report_file: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE,
            ),
            compatibility_report_file: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE,
            ),
            throughput_benchmark_receipt_file: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_BENCHMARK_RECEIPT_FILE,
            ),
            compiled_suite_artifact_file: String::from(
                TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE,
            ),
            deployments,
            benchmark_package_digest: suite.benchmark_package.stable_digest(),
            environment_bundle_digest: stable_digest(
                b"psionic_tassadar_sudoku_9x9_environment_bundle|",
                &suite.environment_bundle,
            ),
            exactness_report_digest: exactness_report.report_digest.clone(),
            compatibility_report_digest: compatibility_report.report_digest.clone(),
            throughput_benchmark_receipt_digest: throughput_benchmark_receipt.report_digest.clone(),
            compiled_suite_artifact_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_sudoku_9x9_compiled_executor_run_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Errors while writing the exact compiled Sudoku-9x9 bundle.
#[derive(Debug, Error)]
pub enum TassadarSudoku9x9CompiledExecutorPersistError {
    /// Building or evaluating the compiled lane failed.
    #[error(transparent)]
    Eval(#[from] TassadarSudoku9x9CompiledExecutorEvalError),
    /// Executing one compiled deployment for persisted artifacts failed.
    #[error(transparent)]
    Compiled(#[from] psionic_models::TassadarCompiledProgramError),
    /// Building the benchmark/environment package failed.
    #[error(transparent)]
    Benchmark(#[from] psionic_eval::TassadarBenchmarkError),
    /// The benchmark package and compiled lane did not target the same program digests.
    #[error(
        "benchmark/compiled program digest mismatch for `{case_id}`: benchmark `{benchmark_program_digest}` vs compiled `{compiled_program_digest}`"
    )]
    ProgramDigestMismatch {
        /// Stable case id.
        case_id: String,
        /// Benchmark-program digest.
        benchmark_program_digest: String,
        /// Compiled-program digest.
        compiled_program_digest: String,
    },
    /// Creating one output directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Writing one artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

/// Executes the exact compiled Sudoku-9x9 lane and writes the resulting bundle.
pub fn run_tassadar_sudoku_9x9_compiled_executor_bundle(
    output_dir: &Path,
) -> Result<TassadarSudoku9x9CompiledExecutorRunBundle, TassadarSudoku9x9CompiledExecutorPersistError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarSudoku9x9CompiledExecutorPersistError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let suite = build_tassadar_sudoku_9x9_suite("v0")?;
    let corpus = build_tassadar_sudoku_9x9_compiled_executor_corpus(None)?;
    assert_program_digest_alignment(&suite, &corpus)?;
    let exactness_report = build_tassadar_sudoku_9x9_compiled_executor_exactness_report(
        &corpus,
        TassadarExecutorDecodeMode::ReferenceLinear,
    )?;
    let compatibility_report =
        build_tassadar_sudoku_9x9_compiled_executor_compatibility_report(&corpus)?;
    let throughput_benchmark_receipt =
        build_tassadar_sudoku_9x9_compiled_executor_benchmark_receipt(
            &corpus,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;

    write_json(
        output_dir.join(TASSADAR_SUDOKU_9X9_BENCHMARK_PACKAGE_FILE),
        &suite.benchmark_package,
    )?;
    write_json(
        output_dir.join(TASSADAR_SUDOKU_9X9_ENVIRONMENT_BUNDLE_FILE),
        &suite.environment_bundle,
    )?;
    write_json(
        output_dir.join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE),
        &exactness_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE),
        &compatibility_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_BENCHMARK_RECEIPT_FILE),
        &throughput_benchmark_receipt,
    )?;
    write_json(
        output_dir.join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE),
        &corpus.compiled_suite_artifact,
    )?;

    let deployments = persist_deployments(output_dir, &corpus)?;
    let bundle = TassadarSudoku9x9CompiledExecutorRunBundle::new(
        &suite,
        &exactness_report,
        &compatibility_report,
        &throughput_benchmark_receipt,
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        deployments,
    );
    write_json(output_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(bundle)
}

fn assert_program_digest_alignment(
    suite: &TassadarReferenceFixtureSuite,
    corpus: &TassadarSudoku9x9CompiledExecutorCorpus,
) -> Result<(), TassadarSudoku9x9CompiledExecutorPersistError> {
    let benchmark_digests = suite
        .artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.validated_program.program_id.clone(),
                artifact.validated_program_digest.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let compiled_digests = corpus
        .cases
        .iter()
        .map(|case| {
            (
                case.program_artifact.validated_program.program_id.clone(),
                case.program_artifact.validated_program_digest.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (case_id, benchmark_program_digest) in benchmark_digests {
        let Some(compiled_program_digest) = compiled_digests.get(&case_id) else {
            return Err(
                TassadarSudoku9x9CompiledExecutorPersistError::ProgramDigestMismatch {
                    case_id,
                    benchmark_program_digest,
                    compiled_program_digest: String::from("missing"),
                },
            );
        };
        if compiled_program_digest != &benchmark_program_digest {
            return Err(
                TassadarSudoku9x9CompiledExecutorPersistError::ProgramDigestMismatch {
                    case_id,
                    benchmark_program_digest,
                    compiled_program_digest: compiled_program_digest.clone(),
                },
            );
        }
    }

    Ok(())
}

fn persist_deployments(
    output_dir: &Path,
    corpus: &TassadarSudoku9x9CompiledExecutorCorpus,
) -> Result<
    Vec<TassadarSudoku9x9CompiledExecutorDeploymentBundle>,
    TassadarSudoku9x9CompiledExecutorPersistError,
> {
    let deployments_root = output_dir.join(DEPLOYMENTS_DIR);
    fs::create_dir_all(&deployments_root).map_err(|error| {
        TassadarSudoku9x9CompiledExecutorPersistError::CreateDir {
            path: deployments_root.display().to_string(),
            error,
        }
    })?;

    let tokenizer = TassadarTraceTokenizer::new();
    let mut bundles = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        let deployment_dir = deployments_root.join(case.case_id.as_str());
        let relative_deployment_dir = PathBuf::from(DEPLOYMENTS_DIR)
            .join(case.case_id.as_str())
            .display()
            .to_string();
        fs::create_dir_all(&deployment_dir).map_err(|error| {
            TassadarSudoku9x9CompiledExecutorPersistError::CreateDir {
                path: deployment_dir.display().to_string(),
                error,
            }
        })?;

        let compiled_execution = case.compiled_executor.execute(
            &case.program_artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        let token_trace_summary = TassadarSudoku9x9TokenTraceSummary::new(
            case.case_id.as_str(),
            &tokenizer,
            &case.program_artifact.validated_program,
            &compiled_execution.execution_report.execution,
        );
        let readable_log = render_readable_log(
            case.case_id.as_str(),
            &compiled_execution.execution_report.execution,
        );

        write_json(
            deployment_dir.join(PROGRAM_ARTIFACT_FILE),
            &case.program_artifact,
        )?;
        write_json(
            deployment_dir.join(COMPILED_WEIGHT_ARTIFACT_FILE),
            case.compiled_executor.compiled_weight_artifact(),
        )?;
        write_json(
            deployment_dir.join(RUNTIME_CONTRACT_FILE),
            case.compiled_executor.runtime_contract(),
        )?;
        write_json(
            deployment_dir.join(COMPILED_WEIGHT_BUNDLE_FILE),
            case.compiled_executor.weight_bundle(),
        )?;
        write_json(
            deployment_dir.join(COMPILE_EVIDENCE_BUNDLE_FILE),
            case.compiled_executor.compile_evidence_bundle(),
        )?;
        write_json(
            deployment_dir.join(MODEL_DESCRIPTOR_FILE),
            case.compiled_executor.descriptor(),
        )?;
        write_json(
            deployment_dir.join(RUNTIME_EXECUTION_PROOF_BUNDLE_FILE),
            &compiled_execution.evidence_bundle.proof_bundle,
        )?;
        write_json(
            deployment_dir.join(RUNTIME_TRACE_PROOF_FILE),
            &compiled_execution.evidence_bundle.trace_proof,
        )?;
        write_json(
            deployment_dir.join(TOKEN_TRACE_SUMMARY_FILE),
            &token_trace_summary,
        )?;
        write_text(deployment_dir.join(READABLE_LOG_FILE), &readable_log)?;

        bundles.push(TassadarSudoku9x9CompiledExecutorDeploymentBundle::new(
            case.case_id.as_str(),
            case.split.as_str(),
            relative_deployment_dir.as_str(),
            case.compiled_executor
                .compiled_weight_artifact()
                .artifact_digest
                .clone(),
            case.compiled_executor
                .runtime_contract()
                .contract_digest
                .clone(),
            case.compiled_executor
                .compile_evidence_bundle()
                .proof_bundle
                .stable_digest(),
            compiled_execution
                .evidence_bundle
                .proof_bundle
                .stable_digest(),
            compiled_execution
                .evidence_bundle
                .trace_proof
                .proof_digest
                .clone(),
            token_trace_summary.summary_digest,
        ));
    }
    Ok(bundles)
}

fn render_readable_log(case_id: &str, execution: &TassadarExecution) -> String {
    let mut lines = Vec::with_capacity(execution.steps.len().saturating_add(4));
    lines.push(format!("case={case_id}"));
    lines.push(format!("program_id={}", execution.program_id));
    lines.push(format!("runner_id={}", execution.runner_id));
    for step in &execution.steps {
        lines.push(format!(
            "step={} pc={}->{} instr={} event={}",
            step.step_index,
            step.pc,
            step.next_pc,
            format_instruction(&step.instruction),
            format_event(&step.event)
        ));
    }
    lines.push(format!("halt={:?}", execution.halt_reason));
    lines.push(format!("outputs={:?}", execution.outputs));
    lines.join("\n")
}

fn format_instruction(instruction: &TassadarInstruction) -> String {
    match instruction {
        TassadarInstruction::Nop => String::from("nop"),
        TassadarInstruction::I32Const { value } => format!("i32.const {value}"),
        TassadarInstruction::LocalGet { local } => format!("local.get {local}"),
        TassadarInstruction::LocalSet { local } => format!("local.set {local}"),
        TassadarInstruction::LocalTee { local } => format!("local.tee {local}"),
        TassadarInstruction::Drop => String::from("drop"),
        TassadarInstruction::I32Add => String::from("i32.add"),
        TassadarInstruction::I32Sub => String::from("i32.sub"),
        TassadarInstruction::I32Mul => String::from("i32.mul"),
        TassadarInstruction::I32Lt => String::from("i32.lt"),
        TassadarInstruction::I32Eqz => String::from("i32.eqz"),
        TassadarInstruction::I32Eq => String::from("i32.eq"),
        TassadarInstruction::I32Ne => String::from("i32.ne"),
        TassadarInstruction::I32Gt => String::from("i32.gt"),
        TassadarInstruction::I32Le => String::from("i32.le"),
        TassadarInstruction::I32Ge => String::from("i32.ge"),
        TassadarInstruction::I32Load { slot } => format!("i32.load {slot}"),
        TassadarInstruction::I32Store { slot } => format!("i32.store {slot}"),
        TassadarInstruction::BrIf { target_pc } => format!("br_if {target_pc}"),
        TassadarInstruction::Output => String::from("output"),
        TassadarInstruction::Return => String::from("return"),
    }
}

fn format_event(event: &TassadarTraceEvent) -> String {
    match event {
        TassadarTraceEvent::Nop => String::from("nop"),
        TassadarTraceEvent::ConstPush { value } => format!("const_push value={value}"),
        TassadarTraceEvent::LocalGet { local, value } => {
            format!("local_get local={local} value={value}")
        }
        TassadarTraceEvent::LocalSet { local, value } => {
            format!("local_set local={local} value={value}")
        }
        TassadarTraceEvent::LocalTee { local, value } => {
            format!("local_tee local={local} value={value}")
        }
        TassadarTraceEvent::Drop { value } => format!("drop value={value}"),
        TassadarTraceEvent::UnaryOp {
            op,
            operand,
            result,
        } => format!("unary_{op:?} operand={operand} result={result}"),
        TassadarTraceEvent::BinaryOp {
            op,
            left,
            right,
            result,
        } => format!("binary_{op:?} left={left} right={right} result={result}"),
        TassadarTraceEvent::Load { slot, value } => format!("load slot={slot} value={value}"),
        TassadarTraceEvent::Store { slot, value } => format!("store slot={slot} value={value}"),
        TassadarTraceEvent::Branch {
            condition,
            taken,
            target_pc,
        } => format!("branch condition={condition} taken={taken} target_pc={target_pc}"),
        TassadarTraceEvent::Output { value } => format!("output value={value}"),
        TassadarTraceEvent::Return => String::from("return"),
    }
}

fn write_json<T>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), TassadarSudoku9x9CompiledExecutorPersistError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)
        .expect("Tassadar Sudoku-9x9 compiled executor bundle artifact should serialize");
    fs::write(path, &bytes).map_err(
        |error| TassadarSudoku9x9CompiledExecutorPersistError::Write {
            path: path.display().to_string(),
            error,
        },
    )
}

fn write_text(
    path: impl AsRef<Path>,
    value: &str,
) -> Result<(), TassadarSudoku9x9CompiledExecutorPersistError> {
    let path = path.as_ref();
    fs::write(path, value).map_err(
        |error| TassadarSudoku9x9CompiledExecutorPersistError::Write {
            path: path.display().to_string(),
            error,
        },
    )
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("Tassadar Sudoku-9x9 compiled executor bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        RUN_BUNDLE_FILE, TASSADAR_SUDOKU_9X9_BENCHMARK_PACKAGE_FILE,
        TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_BENCHMARK_RECEIPT_FILE,
        TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE,
        TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE,
        TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE,
        TASSADAR_SUDOKU_9X9_ENVIRONMENT_BUNDLE_FILE,
        run_tassadar_sudoku_9x9_compiled_executor_bundle,
    };
    use psionic_runtime::TassadarClaimClass;

    #[test]
    fn compiled_sudoku_9x9_executor_bundle_writes_reports_and_deployments()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bundle = run_tassadar_sudoku_9x9_compiled_executor_bundle(temp.path())?;

        assert_eq!(bundle.claim_class, TassadarClaimClass::CompiledArticleClass);
        assert_eq!(bundle.deployments.len(), 4);
        assert!(
            temp.path()
                .join(TASSADAR_SUDOKU_9X9_BENCHMARK_PACKAGE_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_SUDOKU_9X9_ENVIRONMENT_BUNDLE_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_BENCHMARK_RECEIPT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_SUDOKU_9X9_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE)
                .exists()
        );
        assert!(temp.path().join(RUN_BUNDLE_FILE).exists());
        assert!(
            temp.path()
                .join("deployments")
                .join("sudoku_9x9_validation_a")
                .join("runtime_execution_proof_bundle.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("deployments")
                .join("sudoku_9x9_validation_a")
                .join("token_trace_summary.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("deployments")
                .join("sudoku_9x9_validation_a")
                .join("readable_log.txt")
                .exists()
        );
        Ok(())
    }
}
