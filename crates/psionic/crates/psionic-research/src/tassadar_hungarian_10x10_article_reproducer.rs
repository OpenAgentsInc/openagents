use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarHungarian10x10CompiledExecutorExactnessReport, TassadarRustSourceCanonCaseStatus,
    TassadarRustSourceCanonReport, build_tassadar_hungarian_10x10_compiled_executor_corpus,
};
use psionic_models::{TassadarTraceTokenizer, TokenizerBoundary};
use psionic_runtime::{
    TassadarExecution, TassadarExecutorDecodeMode, TassadarInstruction,
    TassadarSudokuV0CorpusSplit, TassadarTraceEvent,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
const TOKEN_TRACE_SUMMARY_SCHEMA_VERSION: u16 = 1;
const TOKEN_TRACE_WINDOW_TOKENS: usize = 128;
const SOURCE_CANON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json";
const EXISTING_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json";
const EXACTNESS_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const CANONICAL_CASE_ID: &str = "hungarian_10x10_test_a";
const ROOT_BUNDLE_FILE: &str = "reproducer_bundle.json";
const COMPILE_EVIDENCE_BUNDLE_FILE: &str = "compile_evidence_bundle.json";
const PROGRAM_ARTIFACT_FILE: &str = "program_artifact.json";
const COMPILED_WEIGHT_ARTIFACT_FILE: &str = "compiled_weight_artifact.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";
const RUNTIME_EXECUTION_PROOF_BUNDLE_FILE: &str = "runtime_execution_proof_bundle.json";
const RUNTIME_TRACE_PROOF_FILE: &str = "runtime_trace_proof.json";
const TOKEN_TRACE_SUMMARY_FILE: &str = "token_trace_summary.json";
const READABLE_LOG_FILE: &str = "readable_log.txt";

pub const TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_ROOT_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_article_reproducer_v1";
pub const TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10ArticleTokenTraceSummary {
    pub schema_version: u16,
    pub case_id: String,
    pub tokenizer_digest: String,
    pub sequence_digest: String,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub total_token_count: usize,
    pub extracted_output_values: Vec<i32>,
    pub halt_marker: Option<String>,
    pub first_prompt_token_ids: Vec<u32>,
    pub first_prompt_symbolic_window: String,
    pub last_trace_token_ids: Vec<u32>,
    pub last_trace_symbolic_window: String,
    pub summary_digest: String,
}

impl TassadarHungarian10x10ArticleTokenTraceSummary {
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
            b"psionic_tassadar_hungarian_10x10_article_token_trace_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10ArticleDeploymentArtifactRef {
    pub artifact_ref: String,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10ArticleReproducerBundle {
    pub bundle_id: String,
    pub root_ref: String,
    pub case_id: String,
    pub compile_evidence_bundle: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub program_artifact: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub compiled_weight_artifact: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub model_descriptor: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub runtime_execution_proof_bundle: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub runtime_trace_proof: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub token_trace_summary: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub readable_log: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    pub bundle_digest: String,
}

impl TassadarHungarian10x10ArticleReproducerBundle {
    fn new(
        compile_evidence_bundle: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        program_artifact: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        compiled_weight_artifact: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        model_descriptor: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        runtime_execution_proof_bundle: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        runtime_trace_proof: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        token_trace_summary: TassadarHungarian10x10ArticleDeploymentArtifactRef,
        readable_log: TassadarHungarian10x10ArticleDeploymentArtifactRef,
    ) -> Self {
        let mut bundle = Self {
            bundle_id: String::from("tassadar.hungarian_10x10.article_reproducer_bundle.v1"),
            root_ref: String::from(TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_ROOT_REF),
            case_id: String::from(CANONICAL_CASE_ID),
            compile_evidence_bundle,
            program_artifact,
            compiled_weight_artifact,
            model_descriptor,
            runtime_execution_proof_bundle,
            runtime_trace_proof,
            token_trace_summary,
            readable_log,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_hungarian_10x10_article_reproducer_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10ArticleDirectExecutionPosture {
    pub requested_decode_mode: String,
    pub effective_decode_mode: String,
    pub fallback_observed: bool,
    pub external_tool_surface_observed: bool,
    pub runtime_backend: String,
    pub compiled_backend_features: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10ArticleReproducerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub workload_family_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub source_receipt_digest: String,
    pub wasm_binary_ref: String,
    pub wasm_binary_digest: String,
    pub canonical_run_bundle_ref: String,
    pub exactness_report_ref: String,
    pub reproducer_bundle_ref: String,
    pub canonical_case_id: String,
    pub canonical_case_split: String,
    pub canonical_case_optimal_cost: i32,
    pub canonical_case_assignment: Vec<i32>,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub halt_match: bool,
    pub reproducer_bundle_digest: String,
    pub direct_execution_posture: TassadarHungarian10x10ArticleDirectExecutionPosture,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarHungarian10x10ArticleReproducerReport {
    fn new(
        source_case: &psionic_eval::TassadarRustSourceCanonCase,
        exactness_case: &psionic_eval::TassadarHungarian10x10CompiledExecutorCaseExactnessReport,
        reproducer_bundle: &TassadarHungarian10x10ArticleReproducerBundle,
        direct_execution_posture: TassadarHungarian10x10ArticleDirectExecutionPosture,
        workload_family_id: &str,
    ) -> Self {
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.hungarian_10x10.article_reproducer.v1"),
            workload_family_id: workload_family_id.to_string(),
            source_ref: source_case.source_ref.clone(),
            source_digest: source_case.source_digest.clone(),
            source_receipt_digest: source_case.receipt_digest.clone(),
            wasm_binary_ref: source_case.wasm_binary_ref.clone().unwrap_or_default(),
            wasm_binary_digest: source_case.wasm_binary_digest.clone().unwrap_or_default(),
            canonical_run_bundle_ref: String::from(EXISTING_RUN_BUNDLE_REF),
            exactness_report_ref: String::from(EXACTNESS_REPORT_REF),
            reproducer_bundle_ref: format!(
                "{}/{}",
                TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_ROOT_REF, ROOT_BUNDLE_FILE
            ),
            canonical_case_id: String::from(CANONICAL_CASE_ID),
            canonical_case_split: exactness_case.split.as_str().to_string(),
            canonical_case_optimal_cost: exactness_case.optimal_cost,
            canonical_case_assignment: exactness_case.optimal_assignment.clone(),
            exact_trace_match: exactness_case.exact_trace_match,
            final_output_match: exactness_case.final_output_match,
            halt_match: exactness_case.halt_match,
            reproducer_bundle_digest: reproducer_bundle.bundle_digest.clone(),
            direct_execution_posture,
            claim_boundary: String::from(
                "this report closes one canonical Rust-only Hungarian-10x10 article reproducer by binding the committed Rust source canon receipt to one exact compiled deployment with readable log, compact token trace, compile lineage, runtime proof lineage, and explicit direct/no-fallback posture. It closes this one matching workload only and does not by itself imply hard-Sudoku closure, million-step closure, or arbitrary-program closure.",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_hungarian_10x10_article_reproducer_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarHungarian10x10ArticleReproducerError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Decode {
        path: String,
        artifact_kind: String,
        error: serde_json::Error,
    },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing rust source canon case `hungarian_10x10_article`")]
    MissingSourceCanonCase,
    #[error("rust source canon case `hungarian_10x10_article` did not compile")]
    SourceCanonNotCompiled,
    #[error("missing compiled corpus case `{case_id}`")]
    MissingCompiledCorpusCase { case_id: String },
    #[error("missing exactness case `{case_id}`")]
    MissingExactnessCase { case_id: String },
}

#[must_use]
pub fn tassadar_hungarian_10x10_article_reproducer_report_path() -> PathBuf {
    repo_root().join(TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF)
}

#[must_use]
pub fn tassadar_hungarian_10x10_article_reproducer_root_path() -> PathBuf {
    repo_root().join(TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_ROOT_REF)
}

pub fn build_tassadar_hungarian_10x10_article_reproducer_report() -> Result<
    TassadarHungarian10x10ArticleReproducerReport,
    TassadarHungarian10x10ArticleReproducerError,
> {
    let source_report: TassadarRustSourceCanonReport =
        read_repo_json(SOURCE_CANON_REPORT_REF, "tassadar_rust_source_canon_report")?;
    let source_case = source_report
        .cases
        .iter()
        .find(|case| case.case_id == "hungarian_10x10_article")
        .ok_or(TassadarHungarian10x10ArticleReproducerError::MissingSourceCanonCase)?;
    if source_case.status != TassadarRustSourceCanonCaseStatus::Compiled {
        return Err(TassadarHungarian10x10ArticleReproducerError::SourceCanonNotCompiled);
    }

    let exactness_report: TassadarHungarian10x10CompiledExecutorExactnessReport = read_repo_json(
        EXACTNESS_REPORT_REF,
        "tassadar_hungarian_10x10_compiled_executor_exactness_report",
    )?;
    let exactness_case = exactness_report
        .case_reports
        .iter()
        .find(|case| case.case_id == CANONICAL_CASE_ID)
        .ok_or_else(
            || TassadarHungarian10x10ArticleReproducerError::MissingExactnessCase {
                case_id: String::from(CANONICAL_CASE_ID),
            },
        )?;

    let reproducer_bundle: TassadarHungarian10x10ArticleReproducerBundle = read_json(
        tassadar_hungarian_10x10_article_reproducer_root_path().join(ROOT_BUNDLE_FILE),
        "tassadar_hungarian_10x10_article_reproducer_bundle",
    )?;
    let runtime_execution_proof_bundle: serde_json::Value = read_json(
        repo_root().join(
            &reproducer_bundle
                .runtime_execution_proof_bundle
                .artifact_ref,
        ),
        "execution_proof_bundle",
    )?;
    let runtime_backend = runtime_execution_proof_bundle["runtime_identity"]["runtime_backend"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let compiled_backend_features = runtime_execution_proof_bundle["runtime_identity"]
        ["backend_toolchain"]["compiled_backend_features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(String::from))
        .collect::<Vec<_>>();
    let external_tool_surface_observed = compiled_backend_features
        .iter()
        .any(|feature| feature.contains("external_tool") || feature.contains("tool_call"));

    Ok(TassadarHungarian10x10ArticleReproducerReport::new(
        source_case,
        exactness_case,
        &reproducer_bundle,
        TassadarHungarian10x10ArticleDirectExecutionPosture {
            requested_decode_mode: exactness_case.requested_decode_mode.as_str().to_string(),
            effective_decode_mode: exactness_case.effective_decode_mode.as_str().to_string(),
            fallback_observed: exactness_case.requested_decode_mode
                != exactness_case.effective_decode_mode,
            external_tool_surface_observed,
            runtime_backend,
            compiled_backend_features,
            detail: String::from(
                "the canonical Hungarian reproducer stays on the direct compiled executor lane with reference-linear decode parity, no observed fallback, and no external-tool feature markers in the runtime identity",
            ),
        },
        exactness_report.workload_family_id.as_str(),
    ))
}

pub fn write_tassadar_hungarian_10x10_article_reproducer_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarHungarian10x10ArticleReproducerReport,
    TassadarHungarian10x10ArticleReproducerError,
> {
    let root_bundle = write_tassadar_hungarian_10x10_article_reproducer_root(
        tassadar_hungarian_10x10_article_reproducer_root_path(),
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarHungarian10x10ArticleReproducerError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_hungarian_10x10_article_reproducer_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("Hungarian article reproducer report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarHungarian10x10ArticleReproducerError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    let _ = root_bundle;
    Ok(report)
}

pub fn write_tassadar_hungarian_10x10_article_reproducer_root(
    output_root: impl AsRef<Path>,
) -> Result<
    TassadarHungarian10x10ArticleReproducerBundle,
    TassadarHungarian10x10ArticleReproducerError,
> {
    let output_root = output_root.as_ref();
    fs::create_dir_all(output_root).map_err(|error| {
        TassadarHungarian10x10ArticleReproducerError::CreateDir {
            path: output_root.display().to_string(),
            error,
        }
    })?;

    let corpus = build_tassadar_hungarian_10x10_compiled_executor_corpus(Some(
        TassadarSudokuV0CorpusSplit::Test,
    ))
    .map_err(
        |error| TassadarHungarian10x10ArticleReproducerError::Decode {
            path: String::from("in_memory"),
            artifact_kind: String::from("tassadar_hungarian_10x10_compiled_executor_corpus"),
            error: serde_json::Error::io(std::io::Error::other(error.to_string())),
        },
    )?;
    let case = corpus
        .cases
        .iter()
        .find(|case| case.case_id == CANONICAL_CASE_ID)
        .ok_or_else(
            || TassadarHungarian10x10ArticleReproducerError::MissingCompiledCorpusCase {
                case_id: String::from(CANONICAL_CASE_ID),
            },
        )?;
    let compiled_execution = case
        .compiled_executor
        .execute(
            &case.program_artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )
        .map_err(
            |error| TassadarHungarian10x10ArticleReproducerError::Decode {
                path: String::from("in_memory"),
                artifact_kind: String::from("tassadar_hungarian_10x10_compiled_execution"),
                error: serde_json::Error::io(std::io::Error::other(error.to_string())),
            },
        )?;
    let tokenizer = TassadarTraceTokenizer::new();
    let token_trace_summary = TassadarHungarian10x10ArticleTokenTraceSummary::new(
        CANONICAL_CASE_ID,
        &tokenizer,
        &case.program_artifact.validated_program,
        &compiled_execution.execution_report.execution,
    );
    let readable_log = render_readable_log(
        CANONICAL_CASE_ID,
        &compiled_execution.execution_report.execution,
    );

    write_json(
        output_root.join(COMPILE_EVIDENCE_BUNDLE_FILE),
        case.compiled_executor.compile_evidence_bundle(),
    )?;
    write_json(
        output_root.join(PROGRAM_ARTIFACT_FILE),
        &case.program_artifact,
    )?;
    write_json(
        output_root.join(COMPILED_WEIGHT_ARTIFACT_FILE),
        case.compiled_executor.compiled_weight_artifact(),
    )?;
    write_json(
        output_root.join(MODEL_DESCRIPTOR_FILE),
        case.compiled_executor.descriptor(),
    )?;
    write_json(
        output_root.join(RUNTIME_EXECUTION_PROOF_BUNDLE_FILE),
        &compiled_execution.evidence_bundle.proof_bundle,
    )?;
    write_json(
        output_root.join(RUNTIME_TRACE_PROOF_FILE),
        &compiled_execution.evidence_bundle.trace_proof,
    )?;
    write_json(
        output_root.join(TOKEN_TRACE_SUMMARY_FILE),
        &token_trace_summary,
    )?;
    write_text(output_root.join(READABLE_LOG_FILE), &readable_log)?;

    let bundle = TassadarHungarian10x10ArticleReproducerBundle::new(
        artifact_ref(output_root, COMPILE_EVIDENCE_BUNDLE_FILE)?,
        artifact_ref(output_root, PROGRAM_ARTIFACT_FILE)?,
        artifact_ref(output_root, COMPILED_WEIGHT_ARTIFACT_FILE)?,
        artifact_ref(output_root, MODEL_DESCRIPTOR_FILE)?,
        artifact_ref(output_root, RUNTIME_EXECUTION_PROOF_BUNDLE_FILE)?,
        artifact_ref(output_root, RUNTIME_TRACE_PROOF_FILE)?,
        TassadarHungarian10x10ArticleDeploymentArtifactRef {
            artifact_ref: canonical_repo_relative_path(&output_root.join(TOKEN_TRACE_SUMMARY_FILE)),
            artifact_digest: token_trace_summary.summary_digest.clone(),
        },
        artifact_ref(output_root, READABLE_LOG_FILE)?,
    );
    write_json(output_root.join(ROOT_BUNDLE_FILE), &bundle)?;
    Ok(bundle)
}

fn artifact_ref(
    output_root: &Path,
    file_name: &str,
) -> Result<
    TassadarHungarian10x10ArticleDeploymentArtifactRef,
    TassadarHungarian10x10ArticleReproducerError,
> {
    let path = output_root.join(file_name);
    Ok(TassadarHungarian10x10ArticleDeploymentArtifactRef {
        artifact_ref: canonical_repo_relative_path(&path),
        artifact_digest: file_digest(&path)?,
    })
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
) -> Result<(), TassadarHungarian10x10ArticleReproducerError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)
        .expect("Hungarian article reproducer artifact should serialize");
    fs::write(path, &bytes).map_err(
        |error| TassadarHungarian10x10ArticleReproducerError::Write {
            path: path.display().to_string(),
            error,
        },
    )
}

fn write_text(
    path: impl AsRef<Path>,
    value: &str,
) -> Result<(), TassadarHungarian10x10ArticleReproducerError> {
    let path = path.as_ref();
    fs::write(path, value.as_bytes()).map_err(|error| {
        TassadarHungarian10x10ArticleReproducerError::Write {
            path: path.display().to_string(),
            error,
        }
    })
}

fn file_digest(path: &Path) -> Result<String, TassadarHungarian10x10ArticleReproducerError> {
    let bytes =
        fs::read(path).map_err(|error| TassadarHungarian10x10ArticleReproducerError::Read {
            path: path.display().to_string(),
            error,
        })?;
    Ok(stable_bytes_digest(&bytes))
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn read_repo_json<T>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarHungarian10x10ArticleReproducerError>
where
    T: DeserializeOwned,
{
    read_json(repo_root().join(relative_path), artifact_kind)
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarHungarian10x10ArticleReproducerError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarHungarian10x10ArticleReproducerError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarHungarian10x10ArticleReproducerError::Decode {
            path: path.display().to_string(),
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
}

fn canonical_repo_relative_path(path: &Path) -> String {
    let repo_root = repo_root();
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical_path
        .strip_prefix(&repo_root)
        .unwrap_or(&canonical_path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("Hungarian reproducer report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF,
        build_tassadar_hungarian_10x10_article_reproducer_report, repo_root,
        tassadar_hungarian_10x10_article_reproducer_report_path,
        write_tassadar_hungarian_10x10_article_reproducer_report,
        write_tassadar_hungarian_10x10_article_reproducer_root,
    };
    use crate::TassadarHungarian10x10ArticleReproducerReport;

    #[test]
    fn hungarian_10x10_article_reproducer_root_and_report_are_machine_legible()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = write_tassadar_hungarian_10x10_article_reproducer_root(
            super::tassadar_hungarian_10x10_article_reproducer_root_path(),
        )?;
        let report = build_tassadar_hungarian_10x10_article_reproducer_report()?;
        assert_eq!(report.canonical_case_id, "hungarian_10x10_test_a");
        assert!(report.exact_trace_match);
        assert!(report.final_output_match);
        assert!(report.halt_match);
        assert!(!report.direct_execution_posture.fallback_observed);
        assert!(
            !report
                .direct_execution_posture
                .external_tool_surface_observed
        );
        assert!(repo_root().join(&root.readable_log.artifact_ref).exists());
        assert!(
            repo_root()
                .join(&root.token_trace_summary.artifact_ref)
                .exists()
        );
        Ok(())
    }

    #[test]
    fn hungarian_10x10_article_reproducer_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = write_tassadar_hungarian_10x10_article_reproducer_report(
            tassadar_hungarian_10x10_article_reproducer_report_path(),
        )?;
        let persisted: TassadarHungarian10x10ArticleReproducerReport = serde_json::from_slice(
            &std::fs::read(tassadar_hungarian_10x10_article_reproducer_report_path())?,
        )?;
        assert_eq!(persisted, expected);
        assert_eq!(
            TASSADAR_HUNGARIAN_10X10_ARTICLE_REPRODUCER_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json"
        );
        Ok(())
    }
}
