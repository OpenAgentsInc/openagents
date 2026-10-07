use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    execute_program_direct_summary, execute_tassadar_executor_request_summary,
    tassadar_article_hard_sudoku_suite, tassadar_trace_abi_for_profile_id,
    tassadar_wasm_profile_for_id, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarExecutorSelectionState, TASSADAR_CPU_REFERENCE_RUNNER_ID,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const HARD_SUDOKU_RUNTIME_CEILING_SECONDS: f64 = 180.0;
const HARD_SUDOKU_SUITE_MANIFEST_REF: &str =
    "fixtures/tassadar/sources/tassadar_article_hard_sudoku_suite_v1.json";

pub const TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_ROOT_REF: &str =
    "fixtures/tassadar/runs/article_hard_sudoku_benchmark_v1";
pub const TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_FILE: &str =
    "article_hard_sudoku_benchmark_bundle.json";
pub const TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/article_hard_sudoku_benchmark_v1/article_hard_sudoku_benchmark_bundle.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuBenchmarkCaseReceipt {
    pub case_id: String,
    pub case_role: String,
    pub split: String,
    pub given_count: usize,
    pub workload_family_id: String,
    pub program_profile_id: String,
    pub runtime_runner_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub selection_state: TassadarExecutorSelectionState,
    pub cpu_reference_behavior_digest: String,
    pub fast_route_behavior_digest: String,
    pub expected_outputs: Vec<i32>,
    pub actual_outputs: Vec<i32>,
    pub exact_output_match: bool,
    pub exact_behavior_match: bool,
    pub halt_reason_match: bool,
    pub exactness_green: bool,
    pub measured_run_time_seconds: f64,
    pub runtime_ceiling_seconds: f64,
    pub under_runtime_ceiling: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHardSudokuBenchmarkBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub bundle_root_ref: String,
    pub suite_manifest_ref: String,
    pub runtime_ceiling_seconds: f64,
    pub case_receipts: Vec<TassadarArticleHardSudokuBenchmarkCaseReceipt>,
    pub suite_exact_case_count: u32,
    pub suite_runtime_ceiling_pass_count: u32,
    pub named_arto_green: bool,
    pub hard_sudoku_suite_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl TassadarArticleHardSudokuBenchmarkBundle {
    fn new(case_receipts: Vec<TassadarArticleHardSudokuBenchmarkCaseReceipt>) -> Self {
        let suite_exact_case_count = case_receipts
            .iter()
            .filter(|receipt| receipt.exactness_green)
            .count() as u32;
        let suite_runtime_ceiling_pass_count = case_receipts
            .iter()
            .filter(|receipt| receipt.under_runtime_ceiling)
            .count() as u32;
        let named_arto_green = case_receipts.iter().any(|receipt| {
            receipt.case_id == "sudoku_9x9_arto_inkala"
                && receipt.exactness_green
                && receipt.under_runtime_ceiling
        });
        let hard_sudoku_suite_green = case_receipts.iter().all(|receipt| {
            receipt.selection_state == TassadarExecutorSelectionState::Direct
                && receipt.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
                && receipt.exactness_green
                && receipt.under_runtime_ceiling
        });
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.article_hard_sudoku_benchmark.bundle.v1"),
            bundle_root_ref: String::from(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_ROOT_REF),
            suite_manifest_ref: String::from(HARD_SUDOKU_SUITE_MANIFEST_REF),
            runtime_ceiling_seconds: HARD_SUDOKU_RUNTIME_CEILING_SECONDS,
            case_receipts,
            suite_exact_case_count,
            suite_runtime_ceiling_pass_count,
            named_arto_green,
            hard_sudoku_suite_green,
            claim_boundary: String::from(
                "this runtime bundle closes the TAS-181 hard-Sudoku runtime tranche only. It proves that the declared hard-Sudoku suite stays exact on the canonical HullCache fast route and that the named Arto Inkala fixture plus the committed hard-Sudoku stand-in remain under the article's stated three-minute ceiling. It does not yet claim the later unified demo-and-benchmark gate, no-spill single-run closure, or final article-equivalence green status.",
            ),
            summary: String::new(),
            bundle_digest: String::new(),
        };
        bundle.summary = format!(
            "Hard-Sudoku benchmark bundle now records suite_exact_case_count={}/{}, suite_runtime_ceiling_pass_count={}/{}, named_arto_green={}, and hard_sudoku_suite_green={}.",
            bundle.suite_exact_case_count,
            bundle.case_receipts.len(),
            bundle.suite_runtime_ceiling_pass_count,
            bundle.case_receipts.len(),
            bundle.named_arto_green,
            bundle.hard_sudoku_suite_green,
        );
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_article_hard_sudoku_benchmark_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleHardSudokuBenchmarkBundleError {
    #[error("missing trace ABI for profile `{profile_id}`")]
    MissingTraceAbi { profile_id: String },
    #[error("reference execution for `{case_id}` failed: {error}")]
    ReferenceExecution {
        case_id: String,
        error: TassadarExecutionRefusal,
    },
    #[error("fast-route execution for `{case_id}` failed: {detail}")]
    FastRouteExecution { case_id: String, detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[cfg(test)]
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[cfg(test)]
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn tassadar_article_hard_sudoku_benchmark_root_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_ROOT_REF)
}

#[must_use]
pub fn tassadar_article_hard_sudoku_benchmark_bundle_path() -> PathBuf {
    tassadar_article_hard_sudoku_benchmark_root_path()
        .join(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_FILE)
}

pub fn build_tassadar_article_hard_sudoku_benchmark_bundle(
) -> Result<TassadarArticleHardSudokuBenchmarkBundle, TassadarArticleHardSudokuBenchmarkBundleError>
{
    let case_receipts = tassadar_article_hard_sudoku_suite()
        .into_iter()
        .map(build_case_receipt)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TassadarArticleHardSudokuBenchmarkBundle::new(case_receipts))
}

pub fn write_tassadar_article_hard_sudoku_benchmark_bundle(
    output_root: impl AsRef<Path>,
) -> Result<TassadarArticleHardSudokuBenchmarkBundle, TassadarArticleHardSudokuBenchmarkBundleError>
{
    let output_root = output_root.as_ref();
    fs::create_dir_all(output_root).map_err(|error| {
        TassadarArticleHardSudokuBenchmarkBundleError::CreateDir {
            path: output_root.display().to_string(),
            error,
        }
    })?;
    let bundle = build_tassadar_article_hard_sudoku_benchmark_bundle()?;
    let output_path = output_root.join(TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleHardSudokuBenchmarkBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_case_receipt(
    case: crate::TassadarSudoku9x9CorpusCase,
) -> Result<
    TassadarArticleHardSudokuBenchmarkCaseReceipt,
    TassadarArticleHardSudokuBenchmarkBundleError,
> {
    let trace_abi =
        tassadar_trace_abi_for_profile_id(case.validation_case.program.profile_id.as_str())
            .ok_or_else(
                || TassadarArticleHardSudokuBenchmarkBundleError::MissingTraceAbi {
                    profile_id: case.validation_case.program.profile_id.clone(),
                },
            )?;
    let profile = tassadar_wasm_profile_for_id(case.validation_case.program.profile_id.as_str())
        .ok_or_else(
            || TassadarArticleHardSudokuBenchmarkBundleError::MissingTraceAbi {
                profile_id: case.validation_case.program.profile_id.clone(),
            },
        )?;
    let cpu_reference_summary = execute_program_direct_summary(
        &case.validation_case.program,
        &profile,
        &trace_abi,
        TASSADAR_CPU_REFERENCE_RUNNER_ID,
    )
    .map_err(
        |error| TassadarArticleHardSudokuBenchmarkBundleError::ReferenceExecution {
            case_id: case.case_id.clone(),
            error,
        },
    )?;

    let started = Instant::now();
    let execution_report = execute_tassadar_executor_request_summary(
        &case.validation_case.program,
        TassadarExecutorDecodeMode::HullCache,
        trace_abi.schema_version,
        None,
    )
    .map_err(|diagnostic| {
        TassadarArticleHardSudokuBenchmarkBundleError::FastRouteExecution {
            case_id: case.case_id.clone(),
            detail: diagnostic.detail,
        }
    })?;
    let measured_run_time_seconds = started.elapsed().as_secs_f64().max(1e-9);
    let fast_route_summary = execution_report.execution_summary;
    let exact_output_match = fast_route_summary.outputs == case.validation_case.expected_outputs;
    let exact_behavior_match =
        fast_route_summary.behavior_digest == cpu_reference_summary.behavior_digest;
    let halt_reason_match = fast_route_summary.halt_reason == cpu_reference_summary.halt_reason;
    let exactness_green = execution_report.selection.selection_state
        == TassadarExecutorSelectionState::Direct
        && execution_report.selection.effective_decode_mode
            == Some(TassadarExecutorDecodeMode::HullCache)
        && exact_output_match
        && exact_behavior_match
        && halt_reason_match;
    let under_runtime_ceiling = measured_run_time_seconds <= HARD_SUDOKU_RUNTIME_CEILING_SECONDS;

    Ok(TassadarArticleHardSudokuBenchmarkCaseReceipt {
        case_id: case.case_id.clone(),
        case_role: case_role_for_id(case.case_id.as_str()).to_string(),
        split: case.split.as_str().to_string(),
        given_count: case.given_count,
        workload_family_id: String::from("SudokuClass"),
        program_profile_id: case.validation_case.program.profile_id.clone(),
        runtime_runner_id: fast_route_summary.runner_id.clone(),
        requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
        effective_decode_mode: execution_report.selection.effective_decode_mode,
        selection_state: execution_report.selection.selection_state,
        cpu_reference_behavior_digest: cpu_reference_summary.behavior_digest,
        fast_route_behavior_digest: fast_route_summary.behavior_digest,
        expected_outputs: case.validation_case.expected_outputs.clone(),
        actual_outputs: fast_route_summary.outputs.clone(),
        exact_output_match,
        exact_behavior_match,
        halt_reason_match,
        exactness_green,
        measured_run_time_seconds,
        runtime_ceiling_seconds: HARD_SUDOKU_RUNTIME_CEILING_SECONDS,
        under_runtime_ceiling,
        note: format!(
            "case `{}` stays direct on HullCache and is checked against the CPU reference plus the declared {} second ceiling.",
            case.case_id, HARD_SUDOKU_RUNTIME_CEILING_SECONDS
        ),
    })
}

fn case_role_for_id(case_id: &str) -> &'static str {
    match case_id {
        "sudoku_9x9_arto_inkala" => "named_arto_inkala",
        _ => "hard_sudoku_stand_in",
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-runtime should live under <repo>/crates/psionic-runtime")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleHardSudokuBenchmarkBundleError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleHardSudokuBenchmarkBundleError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleHardSudokuBenchmarkBundleError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_hard_sudoku_benchmark_bundle, read_repo_json,
        tassadar_article_hard_sudoku_benchmark_bundle_path,
        write_tassadar_article_hard_sudoku_benchmark_bundle,
        TassadarArticleHardSudokuBenchmarkBundle,
        TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_REF,
    };
    use crate::{TassadarExecutorDecodeMode, TassadarExecutorSelectionState};

    fn normalized_bundle_value(
        bundle: &TassadarArticleHardSudokuBenchmarkBundle,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(bundle).expect("bundle serializes");
        value["bundle_digest"] = serde_json::Value::Null;
        for receipt in value["case_receipts"]
            .as_array_mut()
            .expect("case_receipts array")
        {
            receipt["measured_run_time_seconds"] = serde_json::Value::Null;
        }
        value
    }

    #[test]
    fn article_hard_sudoku_benchmark_bundle_closes_declared_suite() {
        let bundle = build_tassadar_article_hard_sudoku_benchmark_bundle().expect("bundle");

        assert_eq!(bundle.case_receipts.len(), 2);
        assert!(bundle.named_arto_green);
        assert!(bundle.hard_sudoku_suite_green);
        assert_eq!(bundle.suite_exact_case_count, 2);
        assert_eq!(bundle.suite_runtime_ceiling_pass_count, 2);
        for receipt in &bundle.case_receipts {
            assert_eq!(
                receipt.requested_decode_mode,
                TassadarExecutorDecodeMode::HullCache
            );
            assert_eq!(
                receipt.effective_decode_mode,
                Some(TassadarExecutorDecodeMode::HullCache)
            );
            assert_eq!(
                receipt.selection_state,
                TassadarExecutorSelectionState::Direct
            );
            assert!(receipt.exactness_green);
            assert!(receipt.under_runtime_ceiling);
        }
    }

    #[test]
    fn article_hard_sudoku_benchmark_bundle_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_hard_sudoku_benchmark_bundle().expect("bundle");
        let committed: TassadarArticleHardSudokuBenchmarkBundle = read_repo_json(
            TASSADAR_ARTICLE_HARD_SUDOKU_BENCHMARK_BUNDLE_REF,
            "article_hard_sudoku_benchmark_bundle",
        )?;
        assert_eq!(
            normalized_bundle_value(&generated),
            normalized_bundle_value(&committed)
        );
        Ok(())
    }

    #[test]
    fn write_article_hard_sudoku_benchmark_bundle_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let written = write_tassadar_article_hard_sudoku_benchmark_bundle(directory.path())?;
        let persisted: TassadarArticleHardSudokuBenchmarkBundle =
            serde_json::from_slice(&std::fs::read(
                directory
                    .path()
                    .join("article_hard_sudoku_benchmark_bundle.json"),
            )?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_hard_sudoku_benchmark_bundle_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("article_hard_sudoku_benchmark_bundle.json")
        );
        Ok(())
    }
}
