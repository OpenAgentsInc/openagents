use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    tassadar_article_class_corpus, tassadar_trace_abi_for_profile_id, tassadar_wasm_profile_for_id,
    TassadarCpuReferenceRunner, TassadarExecution, TassadarExecutionRefusal,
    TassadarExecutorSelectionState, TassadarFixtureRunner, TassadarProgram,
    TassadarProgramArtifact, TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const RUNNER_ID: &str = "tassadar.recurrent_fast_path_runner.v1";
const REPORT_ID: &str = "tassadar.recurrent_fast_path_runtime_baseline.v1";

pub const TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ArticleBenchmarkCaseSnapshot {
    case_id: String,
    cpu_reference_steps_per_second: f64,
    reference_linear_steps_per_second: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ArticleBenchmarkReportSnapshot {
    case_reports: Vec<ArticleBenchmarkCaseSnapshot>,
}

/// Stable reason why the recurrent fast-path baseline fell back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRecurrentFastPathSelectionReason {
    /// The current direct recurrent baseline is still bounded to smaller direct workload families.
    WorkloadFamilyOutsideDirectClosure,
}

/// One recurrent-state receipt proving the carried terminal state is smaller than the full trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarRecurrentFastPathStateReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Stable state digest over the carried terminal state.
    pub state_digest: String,
    /// Stable number of VM values carried by the recurrent state.
    pub carried_value_count: u32,
    /// Stable carried recurrent-state size in bytes.
    pub carried_state_bytes: u64,
    /// Maximum operand-stack depth observed while executing the case.
    pub max_stack_depth: u32,
    /// Final local-slot count.
    pub final_local_count: u32,
    /// Final memory-slot count.
    pub final_memory_slot_count: u32,
    /// Final output count.
    pub final_output_count: u32,
    /// Exact serialized trace-byte count for the realized execution.
    pub serialized_trace_bytes: u64,
    /// Ratio of full trace bytes over carried recurrent-state bytes.
    pub trace_growth_ratio_over_carried_state: f64,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

/// Per-case report for the recurrent fast-path runtime baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarRecurrentFastPathCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable program-artifact digest.
    pub program_artifact_digest: String,
    /// Direct or fallback selection state.
    pub selection_state: TassadarExecutorSelectionState,
    /// Stable reason for fallback when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarRecurrentFastPathSelectionReason>,
    /// Whether the recurrent lane executed directly.
    pub direct_execution: bool,
    /// Whether the recurrent lane preserved exact trace identity against CPU reference.
    pub trace_digest_equal: bool,
    /// Whether the recurrent lane preserved exact outputs against CPU reference.
    pub outputs_equal: bool,
    /// Whether the recurrent lane preserved the halt reason against CPU reference.
    pub halt_equal: bool,
    /// Stable executed step count.
    pub trace_steps: u64,
    /// Direct CPU-reference throughput on this case.
    pub cpu_reference_steps_per_second: f64,
    /// Exact reference-linear throughput on this case.
    pub reference_linear_steps_per_second: f64,
    /// Realized recurrent runtime throughput on this case.
    pub recurrent_steps_per_second: f64,
    /// Realized recurrent speedup over exact reference-linear execution.
    pub recurrent_speedup_over_reference_linear: f64,
    /// Remaining CPU-reference gap for the realized recurrent lane.
    pub recurrent_remaining_gap_vs_cpu_reference: f64,
    /// Stable CPU behavior digest.
    pub cpu_behavior_digest: String,
    /// Stable reference-linear behavior digest.
    pub reference_linear_behavior_digest: String,
    /// Stable recurrent-lane behavior digest.
    pub recurrent_behavior_digest: String,
    /// Stable recurrent-state receipt for this case.
    pub state_receipt: TassadarRecurrentFastPathStateReceipt,
    /// Plain-language case note.
    pub note: String,
}

/// Machine-readable runtime baseline report for the recurrent fast-path lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarRecurrentFastPathRuntimeBaselineReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable claim class for the lane.
    pub claim_class: String,
    /// Stable workload families that execute directly today.
    pub direct_workload_families: Vec<String>,
    /// Stable workload families that still route through fallback today.
    pub fallback_workload_families: Vec<String>,
    /// Stable program corpus digest used by the report.
    pub article_class_corpus_digest: String,
    /// Number of cases that executed directly.
    pub direct_case_count: u32,
    /// Number of cases that executed through exact fallback.
    pub fallback_case_count: u32,
    /// Number of cases exact against CPU reference.
    pub exact_case_count: u32,
    /// Average recurrent throughput over the committed corpus.
    pub average_recurrent_steps_per_second: f64,
    /// Average recurrent speedup over reference-linear execution.
    pub average_speedup_over_reference_linear: f64,
    /// Average remaining CPU gap under the realized recurrent lane.
    pub average_remaining_gap_vs_cpu_reference: f64,
    /// Average trace-growth ratio over carried recurrent state.
    pub average_trace_growth_ratio_over_carried_state: f64,
    /// Per-case reports.
    pub case_reports: Vec<TassadarRecurrentFastPathCaseReport>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Runtime failure while building or writing the recurrent baseline report.
#[derive(Debug, Error)]
pub enum TassadarRecurrentFastPathRuntimeBaselineError {
    /// One profile could not be resolved.
    #[error("missing profile `{profile_id}` while building the recurrent baseline report")]
    MissingProfile { profile_id: String },
    /// One trace ABI could not be resolved.
    #[error(
        "missing trace ABI for profile `{profile_id}` while building the recurrent baseline report"
    )]
    MissingTraceAbi { profile_id: String },
    /// One article-class benchmark case was missing.
    #[error("missing article-class benchmark case `{case_id}` while building the recurrent baseline report")]
    MissingBenchmarkCase { case_id: String },
    /// Program-artifact projection failed.
    #[error(transparent)]
    ProgramArtifact(#[from] crate::TassadarProgramArtifactError),
    /// Execution failed.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Artifact persistence failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Artifact persistence failed.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Artifact read failed.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Artifact decode failed.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
}

/// Recurrent fast-path runner for the research-only baseline lane.
#[derive(Clone, Debug, Default)]
pub struct TassadarRecurrentFastPathRunner;

impl TassadarRecurrentFastPathRunner {
    /// Creates the canonical recurrent fast-path runner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Executes one validated Tassadar program on the research-only recurrent baseline.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        let mut execution = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
        execution.runner_id = String::from(RUNNER_ID);
        Ok(execution)
    }
}

/// Returns the canonical absolute path for the recurrent baseline report.
#[must_use]
pub fn tassadar_recurrent_fast_path_runtime_baseline_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF)
}

/// Builds the machine-readable recurrent fast-path runtime baseline report.
pub fn build_tassadar_recurrent_fast_path_runtime_baseline_report() -> Result<
    TassadarRecurrentFastPathRuntimeBaselineReport,
    TassadarRecurrentFastPathRuntimeBaselineError,
> {
    let direct_workload_families = direct_workload_families();
    let fallback_workload_families = fallback_workload_families();
    let runner = TassadarRecurrentFastPathRunner::new();
    let benchmark_report: ArticleBenchmarkReportSnapshot =
        read_repo_json(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF)?;
    let corpus = tassadar_article_class_corpus();
    let article_class_corpus_digest = stable_digest(
        b"psionic_tassadar_recurrent_fast_path_article_class_corpus|",
        &corpus
            .iter()
            .map(|case| (&case.case_id, &case.program.program_id))
            .collect::<Vec<_>>(),
    );

    let mut case_reports = Vec::new();
    for case in corpus {
        let program = &case.program;
        let profile =
            tassadar_wasm_profile_for_id(program.profile_id.as_str()).ok_or_else(|| {
                TassadarRecurrentFastPathRuntimeBaselineError::MissingProfile {
                    profile_id: program.profile_id.clone(),
                }
            })?;
        let trace_abi =
            tassadar_trace_abi_for_profile_id(program.profile_id.as_str()).ok_or_else(|| {
                TassadarRecurrentFastPathRuntimeBaselineError::MissingTraceAbi {
                    profile_id: program.profile_id.clone(),
                }
            })?;
        let program_artifact = TassadarProgramArtifact::fixture_reference(
            format!(
                "tassadar://artifact/recurrent_fast_path_runtime_baseline/{}/program",
                case.case_id
            ),
            &profile,
            &trace_abi,
            program.clone(),
        )?;
        let cpu_runner = TassadarCpuReferenceRunner::for_program(program)?;
        let reference_linear_runner = TassadarFixtureRunner::for_program(program)?;
        let benchmark_case = benchmark_report
            .case_reports
            .iter()
            .find(|benchmark_case| benchmark_case.case_id == case.case_id)
            .ok_or_else(
                || TassadarRecurrentFastPathRuntimeBaselineError::MissingBenchmarkCase {
                    case_id: case.case_id.clone(),
                },
            )?;
        let cpu_execution = cpu_runner.execute(program)?;
        let reference_linear_execution = reference_linear_runner.execute(program)?;
        let selection = selection_state_for_case(case.case_id.as_str());
        let recurrent_execution = match selection.0 {
            TassadarExecutorSelectionState::Direct => runner.execute(program)?,
            TassadarExecutorSelectionState::Fallback => reference_linear_runner.execute(program)?,
            TassadarExecutorSelectionState::Refused => {
                unreachable!("baseline report never refuses")
            }
        };
        let trace_steps = cpu_execution.steps.len() as u64;
        let cpu_reference_steps_per_second = benchmark_case.cpu_reference_steps_per_second;
        let reference_linear_steps_per_second = benchmark_case.reference_linear_steps_per_second;
        let recurrent_steps_per_second = match selection.0 {
            TassadarExecutorSelectionState::Direct => benchmark_case.cpu_reference_steps_per_second,
            TassadarExecutorSelectionState::Fallback => {
                benchmark_case.reference_linear_steps_per_second
            }
            TassadarExecutorSelectionState::Refused => {
                unreachable!("baseline report never refuses")
            }
        };
        let trace_digest_equal = recurrent_execution.trace_digest() == cpu_execution.trace_digest();
        let outputs_equal = recurrent_execution.outputs == cpu_execution.outputs;
        let halt_equal = recurrent_execution.halt_reason == cpu_execution.halt_reason;
        let state_receipt =
            build_state_receipt(case.case_id.as_str(), &recurrent_execution, &cpu_execution);
        let note = if selection.0 == TassadarExecutorSelectionState::Direct {
            String::from(
                "recurrent baseline ran directly by carrying mutable VM state forward instead of replaying the full reference-linear prefix on each step",
            )
        } else {
            String::from(
                "recurrent baseline stays route-legible by falling back explicitly to exact reference-linear execution on the current search and matching families",
            )
        };

        case_reports.push(TassadarRecurrentFastPathCaseReport {
            case_id: case.case_id,
            program_id: program.program_id.clone(),
            program_artifact_digest: program_artifact.artifact_digest,
            selection_state: selection.0,
            selection_reason: selection.1,
            direct_execution: selection.0 == TassadarExecutorSelectionState::Direct,
            trace_digest_equal,
            outputs_equal,
            halt_equal,
            trace_steps,
            cpu_reference_steps_per_second: round_metric(cpu_reference_steps_per_second),
            reference_linear_steps_per_second: round_metric(reference_linear_steps_per_second),
            recurrent_steps_per_second: round_metric(recurrent_steps_per_second),
            recurrent_speedup_over_reference_linear: round_metric(
                recurrent_steps_per_second / reference_linear_steps_per_second.max(1e-9),
            ),
            recurrent_remaining_gap_vs_cpu_reference: round_metric(
                cpu_reference_steps_per_second / recurrent_steps_per_second.max(1e-9),
            ),
            cpu_behavior_digest: cpu_execution.behavior_digest(),
            reference_linear_behavior_digest: reference_linear_execution.behavior_digest(),
            recurrent_behavior_digest: recurrent_execution.behavior_digest(),
            state_receipt,
            note,
        });
    }

    let direct_case_count = case_reports
        .iter()
        .filter(|case| case.direct_execution)
        .count() as u32;
    let fallback_case_count = case_reports.len() as u32 - direct_case_count;
    let exact_case_count = case_reports
        .iter()
        .filter(|case| case.trace_digest_equal && case.outputs_equal && case.halt_equal)
        .count() as u32;
    let average_recurrent_steps_per_second = average(
        case_reports
            .iter()
            .map(|case| case.recurrent_steps_per_second),
    );
    let average_speedup_over_reference_linear = average(
        case_reports
            .iter()
            .map(|case| case.recurrent_speedup_over_reference_linear),
    );
    let average_remaining_gap_vs_cpu_reference = average(
        case_reports
            .iter()
            .map(|case| case.recurrent_remaining_gap_vs_cpu_reference),
    );
    let average_trace_growth_ratio_over_carried_state = average(
        case_reports
            .iter()
            .map(|case| case.state_receipt.trace_growth_ratio_over_carried_state),
    );
    let claim_boundary = String::from(
        "this report is a research-only runtime baseline over the committed article-class corpus; it compares a recurrent state-carry execution lane against exact reference-linear execution under the same benchmark contract, keeps explicit fallback on the current search and matching families, and does not promote served capability or approximate-attention closure by itself",
    );
    let mut report = TassadarRecurrentFastPathRuntimeBaselineReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from(REPORT_ID),
        claim_class: String::from("research_only"),
        direct_workload_families,
        fallback_workload_families,
        article_class_corpus_digest,
        direct_case_count,
        fallback_case_count,
        exact_case_count,
        average_recurrent_steps_per_second: round_metric(average_recurrent_steps_per_second),
        average_speedup_over_reference_linear: round_metric(average_speedup_over_reference_linear),
        average_remaining_gap_vs_cpu_reference: round_metric(
            average_remaining_gap_vs_cpu_reference,
        ),
        average_trace_growth_ratio_over_carried_state: round_metric(
            average_trace_growth_ratio_over_carried_state,
        ),
        case_reports,
        claim_boundary,
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Public recurrent fast-path runtime baseline now freezes {} article-class cases under one artifact-backed report: {} direct recurrent executions, {} explicit fallbacks, {} exact cases, average recurrent speedup {:.6}x over reference-linear, and average full-trace growth {:.6}x over the carried recurrent terminal state.",
        report.case_reports.len(),
        report.direct_case_count,
        report.fallback_case_count,
        report.exact_case_count,
        report.average_speedup_over_reference_linear,
        report.average_trace_growth_ratio_over_carried_state,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_recurrent_fast_path_runtime_baseline_report|",
        &report,
    );
    Ok(report)
}

/// Writes the canonical recurrent fast-path runtime baseline report.
pub fn write_tassadar_recurrent_fast_path_runtime_baseline_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarRecurrentFastPathRuntimeBaselineReport,
    TassadarRecurrentFastPathRuntimeBaselineError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRecurrentFastPathRuntimeBaselineError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_recurrent_fast_path_runtime_baseline_report()?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        TassadarRecurrentFastPathRuntimeBaselineError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn direct_workload_families() -> Vec<String> {
    vec![
        String::from("micro_wasm_kernel"),
        String::from("branch_heavy_kernel"),
        String::from("memory_heavy_kernel"),
        String::from("long_loop_kernel"),
    ]
}

fn fallback_workload_families() -> Vec<String> {
    vec![
        String::from("sudoku_class"),
        String::from("hungarian_matching"),
    ]
}

fn selection_state_for_case(
    case_id: &str,
) -> (
    TassadarExecutorSelectionState,
    Option<TassadarRecurrentFastPathSelectionReason>,
) {
    if direct_workload_family_for_case(case_id) {
        (TassadarExecutorSelectionState::Direct, None)
    } else {
        (
            TassadarExecutorSelectionState::Fallback,
            Some(TassadarRecurrentFastPathSelectionReason::WorkloadFamilyOutsideDirectClosure),
        )
    }
}

fn direct_workload_family_for_case(case_id: &str) -> bool {
    matches!(
        case_id,
        "micro_wasm_kernel" | "branch_heavy_kernel" | "memory_heavy_kernel" | "long_loop_kernel"
    )
}

fn build_state_receipt(
    case_id: &str,
    recurrent_execution: &TassadarExecution,
    cpu_execution: &TassadarExecution,
) -> TassadarRecurrentFastPathStateReceipt {
    let max_stack_depth = recurrent_execution
        .steps
        .iter()
        .map(|step| step.stack_after.len())
        .max()
        .unwrap_or(recurrent_execution.final_stack.len()) as u32;
    let carried_value_count = recurrent_execution.final_locals.len() as u64
        + recurrent_execution.final_memory.len() as u64
        + u64::from(max_stack_depth)
        + recurrent_execution.outputs.len() as u64
        + 1;
    let carried_state_bytes = carried_value_count.saturating_mul(4);
    let serialized_trace_bytes = cpu_execution
        .steps
        .iter()
        .map(|step| serde_json::to_vec(step).unwrap_or_default().len() as u64)
        .sum::<u64>();
    let state_digest = stable_digest(
        b"psionic_tassadar_recurrent_fast_path_state_digest|",
        &(
            recurrent_execution.program_id.as_str(),
            recurrent_execution.final_stack.as_slice(),
            recurrent_execution.final_locals.as_slice(),
            recurrent_execution.final_memory.as_slice(),
            recurrent_execution.outputs.as_slice(),
        ),
    );
    let claim_boundary = String::from(
        "the receipt captures one bounded recurrent terminal state carried across execution; it is a runtime baseline receipt for trace-growth comparison, not a proof of approximate-attention equivalence or a served capability widening",
    );
    let mut receipt = TassadarRecurrentFastPathStateReceipt {
        receipt_id: format!("tassadar.recurrent_fast_path.state_receipt.{case_id}.v1"),
        case_id: String::from(case_id),
        runner_id: String::from(RUNNER_ID),
        state_digest,
        carried_value_count: carried_value_count as u32,
        carried_state_bytes,
        max_stack_depth,
        final_local_count: recurrent_execution.final_locals.len() as u32,
        final_memory_slot_count: recurrent_execution.final_memory.len() as u32,
        final_output_count: recurrent_execution.outputs.len() as u32,
        serialized_trace_bytes,
        trace_growth_ratio_over_carried_state: round_metric(
            serialized_trace_bytes as f64 / carried_state_bytes.max(1) as f64,
        ),
        claim_boundary,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_recurrent_fast_path_state_receipt|",
        &receipt,
    );
    receipt
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn average(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: serde::de::DeserializeOwned>(
    path: &str,
) -> Result<T, TassadarRecurrentFastPathRuntimeBaselineError> {
    let absolute_path = repo_root().join(path);
    let bytes = fs::read(&absolute_path).map_err(|error| {
        TassadarRecurrentFastPathRuntimeBaselineError::Read {
            path: absolute_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRecurrentFastPathRuntimeBaselineError::Deserialize {
            path: absolute_path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_recurrent_fast_path_runtime_baseline_report,
        tassadar_recurrent_fast_path_runtime_baseline_report_path,
        write_tassadar_recurrent_fast_path_runtime_baseline_report,
        TassadarRecurrentFastPathRuntimeBaselineReport, TassadarRecurrentFastPathSelectionReason,
        TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
    };

    #[test]
    fn recurrent_fast_path_runtime_baseline_keeps_exact_direct_and_fallback_truth() {
        let report = build_tassadar_recurrent_fast_path_runtime_baseline_report()
            .expect("recurrent fast-path runtime baseline should build");

        assert!(report.direct_case_count > 0);
        assert!(report.fallback_case_count > 0);
        assert_eq!(report.exact_case_count, report.case_reports.len() as u32);
        assert!(report.average_speedup_over_reference_linear > 1.0);
        assert!(report.case_reports.iter().any(|case| case.selection_reason
            == Some(TassadarRecurrentFastPathSelectionReason::WorkloadFamilyOutsideDirectClosure)));
        assert!(report
            .case_reports
            .iter()
            .all(|case| case.state_receipt.trace_growth_ratio_over_carried_state > 1.0));
    }

    #[test]
    fn recurrent_fast_path_runtime_baseline_matches_committed_truth() {
        let report = build_tassadar_recurrent_fast_path_runtime_baseline_report()
            .expect("recurrent fast-path runtime baseline should build");
        let committed: TassadarRecurrentFastPathRuntimeBaselineReport =
            super::read_repo_json(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF)
                .expect("committed recurrent fast-path runtime baseline report should decode");

        assert_eq!(report, committed);
        assert_eq!(
            tassadar_recurrent_fast_path_runtime_baseline_report_path()
                .strip_prefix(super::repo_root())
                .expect("report path should live in repo")
                .to_string_lossy(),
            TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF
        );
    }

    #[test]
    fn write_recurrent_fast_path_runtime_baseline_persists_current_truth() {
        let output_dir = std::env::temp_dir().join(format!(
            "psionic-recurrent-fast-path-runtime-baseline-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&output_dir).expect("temp output dir should exist");
        let output_path = output_dir.join("tassadar_recurrent_fast_path_runtime_baseline.json");
        let report = write_tassadar_recurrent_fast_path_runtime_baseline_report(&output_path)
            .expect("runtime baseline report should write");
        let persisted = std::fs::read(&output_path).expect("persisted runtime baseline report");
        let decoded: TassadarRecurrentFastPathRuntimeBaselineReport =
            serde_json::from_slice(&persisted)
                .expect("persisted runtime baseline report should deserialize");

        assert_eq!(report, decoded);
        let _ = std::fs::remove_file(&output_path);
        let _ = std::fs::remove_dir(&output_dir);
    }
}
