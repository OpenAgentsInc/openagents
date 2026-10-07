use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_HULL_CACHE_RUNNER_ID, TassadarArticleRuntimeCloseoutBundle,
    TassadarArticleRuntimeCloseoutError, TassadarArticleRuntimeFloorStatus,
    TassadarExecutorDecodeMode,
    TassadarExecutorSelectionState, TassadarHullCacheRunner, TassadarProgram,
    TassadarTraceAbi, build_tassadar_article_runtime_closeout_bundle,
    execute_program_direct_summary, execution_summary_from_execution, tassadar_article_runtime_closeout_root_path,
    tassadar_hungarian_10x10_corpus, tassadar_sudoku_9x9_corpus, tassadar_trace_abi_for_profile_id,
    write_tassadar_article_runtime_closeout_bundle,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const SELECTED_CANDIDATE_KIND: &str = "hull_cache_runtime";
const INTERNAL_CPU_TOKEN_THROUGHPUT_FLOOR: f64 = 30_000.0;

const HUNGARIAN_TOKEN_TRACE_SUMMARY_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_article_reproducer_v1/token_trace_summary.json";
const HUNGARIAN_READABLE_LOG_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_article_reproducer_v1/readable_log.txt";
const SUDOKU_TOKEN_TRACE_SUMMARY_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/deployments/sudoku_9x9_test_a/token_trace_summary.json";
const SUDOKU_READABLE_LOG_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/deployments/sudoku_9x9_test_a/readable_log.txt";
const RUNTIME_CLOSEOUT_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/article_runtime_closeout_v1/article_runtime_closeout_bundle.json";

pub const TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF: &str =
    "fixtures/tassadar/runs/article_fast_route_throughput_v1";
pub const TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE: &str =
    "article_fast_route_throughput_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFastRouteThroughputFloorStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteTokenTraceCounts {
    pub token_trace_summary_ref: String,
    pub readable_log_ref: String,
    pub prompt_token_count: u64,
    pub target_token_count: u64,
    pub total_token_count: u64,
    pub readable_log_line_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRoutePublicThroughputAnchor {
    pub token_throughput_per_second: u64,
    pub line_throughput_per_second: u64,
    pub total_token_count: u64,
    pub source_note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteDemoReceipt {
    pub workload_id: String,
    pub case_id: String,
    pub program_profile_id: String,
    pub runtime_runner_id: String,
    pub token_trace_counts: TassadarArticleFastRouteTokenTraceCounts,
    pub public_anchor: TassadarArticleFastRoutePublicThroughputAnchor,
    pub exact_step_count: u64,
    pub exactness_bps: u32,
    pub selection_state: TassadarExecutorSelectionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub measured_run_time_seconds: f64,
    pub steps_per_second: f64,
    pub tokens_per_second: f64,
    pub lines_per_second: f64,
    pub internal_token_throughput_floor: f64,
    pub public_token_anchor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub public_line_anchor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub internal_token_floor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteKernelReceipt {
    pub workload_horizon_id: String,
    pub runtime_closeout_bundle_ref: String,
    pub runtime_runner_id: String,
    pub exact_step_count: u64,
    pub exactness_bps: u32,
    pub measured_run_time_seconds: f64,
    pub steps_per_second: f64,
    pub internal_steps_per_second_floor: f64,
    pub floor_status: TassadarArticleFastRouteThroughputFloorStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub bundle_root_ref: String,
    pub runtime_closeout_bundle_ref: String,
    pub selected_candidate_kind: String,
    pub selected_decode_mode: TassadarExecutorDecodeMode,
    pub generated_from_refs: Vec<String>,
    pub demo_receipts: Vec<TassadarArticleFastRouteDemoReceipt>,
    pub kernel_receipts: Vec<TassadarArticleFastRouteKernelReceipt>,
    pub demo_public_floor_pass_count: u32,
    pub demo_internal_floor_pass_count: u32,
    pub kernel_floor_pass_count: u32,
    pub throughput_floor_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl TassadarArticleFastRouteThroughputBundle {
    fn new(
        demo_receipts: Vec<TassadarArticleFastRouteDemoReceipt>,
        kernel_receipts: Vec<TassadarArticleFastRouteKernelReceipt>,
    ) -> Self {
        let generated_from_refs = demo_receipts
            .iter()
            .flat_map(|receipt| {
                [
                    receipt.token_trace_counts.token_trace_summary_ref.clone(),
                    receipt.token_trace_counts.readable_log_ref.clone(),
                ]
            })
            .chain(std::iter::once(String::from(RUNTIME_CLOSEOUT_BUNDLE_REF)))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let demo_public_floor_pass_count = demo_receipts
            .iter()
            .filter(|receipt| {
                receipt.public_token_anchor_status == TassadarArticleFastRouteThroughputFloorStatus::Passed
                    && receipt.public_line_anchor_status
                        == TassadarArticleFastRouteThroughputFloorStatus::Passed
            })
            .count() as u32;
        let demo_internal_floor_pass_count = demo_receipts
            .iter()
            .filter(|receipt| {
                receipt.internal_token_floor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
            })
            .count() as u32;
        let kernel_floor_pass_count = kernel_receipts
            .iter()
            .filter(|receipt| {
                receipt.floor_status == TassadarArticleFastRouteThroughputFloorStatus::Passed
            })
            .count() as u32;
        let throughput_floor_green = demo_receipts.iter().all(|receipt| {
            receipt.selection_state == TassadarExecutorSelectionState::Direct
                && receipt.effective_decode_mode == Some(TassadarExecutorDecodeMode::HullCache)
                && receipt.exactness_bps == 10_000
                && receipt.public_token_anchor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
                && receipt.public_line_anchor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
                && receipt.internal_token_floor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
        }) && kernel_receipts.iter().all(|receipt| {
            receipt.exactness_bps == 10_000
                && receipt.floor_status == TassadarArticleFastRouteThroughputFloorStatus::Passed
        });
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.article_fast_route_throughput.bundle.v1"),
            bundle_root_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF),
            runtime_closeout_bundle_ref: String::from(RUNTIME_CLOSEOUT_BUNDLE_REF),
            selected_candidate_kind: String::from(SELECTED_CANDIDATE_KIND),
            selected_decode_mode: TassadarExecutorDecodeMode::HullCache,
            generated_from_refs,
            demo_receipts,
            kernel_receipts,
            demo_public_floor_pass_count,
            demo_internal_floor_pass_count,
            kernel_floor_pass_count,
            throughput_floor_green,
            claim_boundary: String::from(
                "this runtime bundle closes TAS-175 only for the selected HullCache fast path on the committed Hungarian article run, the committed Sudoku-9x9 hard-Sudoku stand-in run, and the bounded million-step and multi-million-step kernel set. It does not claim final Hungarian demo parity, canonical Arto Inkala closure, benchmark-wide hard-Sudoku closure, or final article-equivalence green status.",
            ),
            summary: String::new(),
            bundle_digest: String::new(),
        };
        bundle.summary = format!(
            "Fast-route throughput bundle now records demo_public_floor_passes={}/{}, demo_internal_floor_passes={}/{}, kernel_floor_passes={}/{}, and throughput_floor_green={}.",
            bundle.demo_public_floor_pass_count,
            bundle.demo_receipts.len(),
            bundle.demo_internal_floor_pass_count,
            bundle.demo_receipts.len(),
            bundle.kernel_floor_pass_count,
            bundle.kernel_receipts.len(),
            bundle.throughput_floor_green,
        );
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_article_fast_route_throughput_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TokenTraceSummaryView {
    case_id: String,
    prompt_token_count: u64,
    target_token_count: u64,
    total_token_count: u64,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteThroughputBundleError {
    #[error(transparent)]
    RuntimeCloseout(#[from] TassadarArticleRuntimeCloseoutError),
    #[error(transparent)]
    Execution(#[from] crate::TassadarExecutionRefusal),
    #[error("missing throughput demo case `{case_id}`")]
    MissingCase { case_id: String },
    #[error("missing trace ABI for program profile `{profile_id}`")]
    MissingTraceAbi { profile_id: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn tassadar_article_fast_route_throughput_root_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF)
}

#[must_use]
pub fn tassadar_article_fast_route_throughput_bundle_path() -> PathBuf {
    tassadar_article_fast_route_throughput_root_path()
        .join(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE)
}

pub fn build_tassadar_article_fast_route_throughput_bundle(
) -> Result<TassadarArticleFastRouteThroughputBundle, TassadarArticleFastRouteThroughputBundleError>
{
    let demo_receipts = vec![
        build_hungarian_demo_receipt()?,
        build_hard_sudoku_demo_receipt()?,
    ];
    let runtime_closeout_bundle = build_tassadar_article_runtime_closeout_bundle()?;
    let kernel_receipts = build_kernel_receipts(&runtime_closeout_bundle)?;
    Ok(TassadarArticleFastRouteThroughputBundle::new(
        demo_receipts,
        kernel_receipts,
    ))
}

pub fn write_tassadar_article_fast_route_throughput_bundle(
    output_root: impl AsRef<Path>,
) -> Result<TassadarArticleFastRouteThroughputBundle, TassadarArticleFastRouteThroughputBundleError>
{
    write_tassadar_article_runtime_closeout_bundle(tassadar_article_runtime_closeout_root_path())?;
    let output_root = output_root.as_ref();
    fs::create_dir_all(output_root).map_err(|error| {
        TassadarArticleFastRouteThroughputBundleError::CreateDir {
            path: output_root.display().to_string(),
            error,
        }
    })?;
    let bundle = build_tassadar_article_fast_route_throughput_bundle()?;
    let output_path = output_root.join(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteThroughputBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_hungarian_demo_receipt(
) -> Result<TassadarArticleFastRouteDemoReceipt, TassadarArticleFastRouteThroughputBundleError> {
    let case = tassadar_hungarian_10x10_corpus()
        .into_iter()
        .find(|case| case.case_id == "hungarian_10x10_test_a")
        .ok_or_else(|| TassadarArticleFastRouteThroughputBundleError::MissingCase {
            case_id: String::from("hungarian_10x10_test_a"),
        })?;
    let public_anchor = TassadarArticleFastRoutePublicThroughputAnchor {
        token_throughput_per_second: 33_409,
        line_throughput_per_second: 7_263,
        total_token_count: 158_396,
        source_note: String::from(
            "Percepta article public Hungarian row with concrete token, line, and total-token figures",
        ),
    };
    build_demo_receipt(
        String::from("hungarian_article_run"),
        case.case_id,
        case.validation_case.program,
        HUNGARIAN_TOKEN_TRACE_SUMMARY_REF,
        HUNGARIAN_READABLE_LOG_REF,
        public_anchor,
        "this row closes the public Hungarian CPU throughput story only for the committed `hungarian_10x10_test_a` article reproducer on direct HullCache; it does not yet claim final canonical article-demo parity beyond that bounded reproducer lane",
    )
}

fn build_hard_sudoku_demo_receipt(
) -> Result<TassadarArticleFastRouteDemoReceipt, TassadarArticleFastRouteThroughputBundleError> {
    let case = tassadar_sudoku_9x9_corpus()
        .into_iter()
        .find(|case| case.case_id == "sudoku_9x9_test_a")
        .ok_or_else(|| TassadarArticleFastRouteThroughputBundleError::MissingCase {
            case_id: String::from("sudoku_9x9_test_a"),
        })?;
    let public_anchor = TassadarArticleFastRoutePublicThroughputAnchor {
        token_throughput_per_second: 34_585,
        line_throughput_per_second: 7_618,
        total_token_count: 2_386_560,
        source_note: String::from(
            "Percepta article public Sudoku row with concrete token, line, and total-token figures",
        ),
    };
    build_demo_receipt(
        String::from("hard_sudoku_article_run"),
        case.case_id,
        case.validation_case.program,
        SUDOKU_TOKEN_TRACE_SUMMARY_REF,
        SUDOKU_READABLE_LOG_REF,
        public_anchor,
        "this row uses the committed `sudoku_9x9_test_a` hard-Sudoku article-shaped executor as the TAS-175 runtime anchor; TAS-181 still owns the named Arto Inkala fixture and benchmark-wide hard-Sudoku closure",
    )
}

fn build_demo_receipt(
    workload_id: String,
    case_id: String,
    program: TassadarProgram,
    token_trace_summary_ref: &str,
    readable_log_ref: &str,
    public_anchor: TassadarArticleFastRoutePublicThroughputAnchor,
    note: &str,
) -> Result<TassadarArticleFastRouteDemoReceipt, TassadarArticleFastRouteThroughputBundleError> {
    let trace_abi = trace_abi_for_program(&program)?;
    let cpu_reference_summary = execute_program_direct_summary(
        &program,
        &crate::tassadar_wasm_profile_for_id(program.profile_id.as_str()).expect("supported profile"),
        &trace_abi,
        crate::TASSADAR_CPU_REFERENCE_RUNNER_ID,
    )?;
    let hull_execution = TassadarHullCacheRunner::for_program(&program)?.execute(&program)?;
    let hull_summary = execution_summary_from_execution(&hull_execution);
    let exactness_bps = u32::from(
        cpu_reference_summary.trace_digest == hull_summary.trace_digest
            && cpu_reference_summary.behavior_digest == hull_summary.behavior_digest
            && cpu_reference_summary.outputs == hull_summary.outputs
            && cpu_reference_summary.halt_reason == hull_summary.halt_reason,
    ) * 10_000;
    let started = Instant::now();
    let benchmark_execution = TassadarHullCacheRunner::for_program(&program)?.execute(&program)?;
    let elapsed = started.elapsed().as_secs_f64().max(1e-9);
    let benchmark_summary = execution_summary_from_execution(&benchmark_execution);
    let steps_per_second = benchmark_summary.step_count as f64 / elapsed;
    let token_trace_counts = read_token_trace_counts(token_trace_summary_ref, readable_log_ref)?;
    let measured_run_time_seconds = benchmark_summary.step_count as f64 / steps_per_second.max(1e-9);
    let tokens_per_second =
        token_trace_counts.total_token_count as f64 * steps_per_second / benchmark_summary.step_count as f64;
    let lines_per_second = token_trace_counts.readable_log_line_count as f64
        * steps_per_second
        / benchmark_summary.step_count as f64;
    Ok(TassadarArticleFastRouteDemoReceipt {
        workload_id,
        case_id,
        program_profile_id: program.profile_id.clone(),
        runtime_runner_id: String::from(TASSADAR_HULL_CACHE_RUNNER_ID),
        token_trace_counts,
        public_token_anchor_status: floor_status(
            tokens_per_second,
            public_anchor.token_throughput_per_second as f64,
        ),
        public_line_anchor_status: floor_status(
            lines_per_second,
            public_anchor.line_throughput_per_second as f64,
        ),
        internal_token_floor_status: floor_status(
            tokens_per_second,
            INTERNAL_CPU_TOKEN_THROUGHPUT_FLOOR,
        ),
        public_anchor,
        exact_step_count: benchmark_summary.step_count,
        exactness_bps,
        selection_state: TassadarExecutorSelectionState::Direct,
        effective_decode_mode: Some(TassadarExecutorDecodeMode::HullCache),
        measured_run_time_seconds,
        steps_per_second,
        tokens_per_second,
        lines_per_second,
        internal_token_throughput_floor: INTERNAL_CPU_TOKEN_THROUGHPUT_FLOOR,
        note: String::from(note),
    })
}

fn build_kernel_receipts(
    runtime_closeout_bundle: &TassadarArticleRuntimeCloseoutBundle,
) -> Result<Vec<TassadarArticleFastRouteKernelReceipt>, TassadarArticleFastRouteThroughputBundleError>
{
    runtime_closeout_bundle
        .horizon_receipts
        .iter()
        .map(|receipt| {
            let steps_per_second = receipt
                .hull_cache
                .steps_per_second
                .ok_or_else(|| TassadarArticleFastRouteThroughputBundleError::MissingCase {
                    case_id: receipt.horizon_id.clone(),
                })?;
            Ok(TassadarArticleFastRouteKernelReceipt {
                workload_horizon_id: receipt.horizon_id.clone(),
                runtime_closeout_bundle_ref: String::from(RUNTIME_CLOSEOUT_BUNDLE_REF),
                runtime_runner_id: String::from(TASSADAR_HULL_CACHE_RUNNER_ID),
                exact_step_count: receipt.exact_step_count,
                exactness_bps: receipt.hull_cache_exactness_bps,
                measured_run_time_seconds: receipt.exact_step_count as f64 / steps_per_second.max(1e-9),
                steps_per_second,
                internal_steps_per_second_floor: receipt.throughput_floor_steps_per_second,
                floor_status: if receipt.hull_cache_floor_status
                    == TassadarArticleRuntimeFloorStatus::Passed
                {
                    TassadarArticleFastRouteThroughputFloorStatus::Passed
                } else {
                    TassadarArticleFastRouteThroughputFloorStatus::Failed
                },
                note: String::from(
                    "this row is projected from the bounded article runtime closeout bundle so TAS-175 can publish fast-route million-step and multi-million-step throughput floors without widening the later single-run no-spill claim surface",
                ),
            })
        })
        .collect::<Result<Vec<_>, _>>()
}

fn read_token_trace_counts(
    token_trace_summary_ref: &str,
    readable_log_ref: &str,
) -> Result<TassadarArticleFastRouteTokenTraceCounts, TassadarArticleFastRouteThroughputBundleError>
{
    let summary: TokenTraceSummaryView = read_repo_json(
        token_trace_summary_ref,
        "article_fast_route_token_trace_summary",
    )?;
    let readable_log = repo_root().join(readable_log_ref);
    let readable_log_bytes = fs::read(&readable_log).map_err(|error| {
        TassadarArticleFastRouteThroughputBundleError::Read {
            path: readable_log.display().to_string(),
            error,
        }
    })?;
    let readable_log_line_count = String::from_utf8_lossy(&readable_log_bytes)
        .lines()
        .count() as u64;
    Ok(TassadarArticleFastRouteTokenTraceCounts {
        token_trace_summary_ref: String::from(token_trace_summary_ref),
        readable_log_ref: String::from(readable_log_ref),
        prompt_token_count: summary.prompt_token_count,
        target_token_count: summary.target_token_count,
        total_token_count: summary.total_token_count,
        readable_log_line_count,
    })
}

fn trace_abi_for_program(
    program: &TassadarProgram,
) -> Result<TassadarTraceAbi, TassadarArticleFastRouteThroughputBundleError> {
    tassadar_trace_abi_for_profile_id(program.profile_id.as_str()).ok_or_else(|| {
        TassadarArticleFastRouteThroughputBundleError::MissingTraceAbi {
            profile_id: program.profile_id.clone(),
        }
    })
}

fn floor_status(
    measured_value: f64,
    floor: f64,
) -> TassadarArticleFastRouteThroughputFloorStatus {
    if measured_value >= floor {
        TassadarArticleFastRouteThroughputFloorStatus::Passed
    } else {
        TassadarArticleFastRouteThroughputFloorStatus::Failed
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

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFastRouteThroughputBundleError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarArticleFastRouteThroughputBundleError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteThroughputBundleError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE,
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF,
        TassadarArticleFastRouteThroughputBundle,
        TassadarArticleFastRouteThroughputFloorStatus,
        build_tassadar_article_fast_route_throughput_bundle, read_repo_json,
        tassadar_article_fast_route_throughput_bundle_path,
        write_tassadar_article_fast_route_throughput_bundle,
    };

    fn normalized_bundle_value(
        bundle: &TassadarArticleFastRouteThroughputBundle,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(bundle).expect("bundle serializes");
        value["bundle_digest"] = serde_json::Value::Null;
        for receipt in value["demo_receipts"]
            .as_array_mut()
            .expect("demo_receipts")
        {
            receipt["measured_run_time_seconds"] = serde_json::Value::Null;
            receipt["steps_per_second"] = serde_json::Value::Null;
            receipt["tokens_per_second"] = serde_json::Value::Null;
            receipt["lines_per_second"] = serde_json::Value::Null;
        }
        for receipt in value["kernel_receipts"]
            .as_array_mut()
            .expect("kernel_receipts")
        {
            receipt["measured_run_time_seconds"] = serde_json::Value::Null;
            receipt["steps_per_second"] = serde_json::Value::Null;
        }
        value
    }

    #[test]
    fn fast_route_throughput_bundle_closes_declared_floor_rows() {
        let bundle = build_tassadar_article_fast_route_throughput_bundle().expect("bundle");

        assert_eq!(
            bundle.bundle_root_ref,
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF
        );
        assert_eq!(bundle.selected_candidate_kind, "hull_cache_runtime");
        assert_eq!(bundle.demo_receipts.len(), 2);
        assert_eq!(bundle.kernel_receipts.len(), 4);
        assert_eq!(bundle.demo_public_floor_pass_count, 2);
        assert_eq!(bundle.demo_internal_floor_pass_count, 2);
        assert_eq!(bundle.kernel_floor_pass_count, 4);
        assert!(bundle.throughput_floor_green);
        assert!(bundle
            .demo_receipts
            .iter()
            .all(|receipt| receipt.exactness_bps == 10_000));
        assert!(bundle.demo_receipts.iter().all(|receipt| {
            receipt.public_token_anchor_status
                == TassadarArticleFastRouteThroughputFloorStatus::Passed
                && receipt.public_line_anchor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
                && receipt.internal_token_floor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
        }));
        assert!(bundle.kernel_receipts.iter().all(|receipt| {
            receipt.exactness_bps == 10_000
                && receipt.floor_status
                    == TassadarArticleFastRouteThroughputFloorStatus::Passed
        }));
    }

    #[test]
    fn fast_route_throughput_bundle_matches_committed_truth() {
        let generated = build_tassadar_article_fast_route_throughput_bundle().expect("bundle");
        let committed: TassadarArticleFastRouteThroughputBundle = read_repo_json(
            &format!(
                "{}/{}",
                TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_ROOT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE
            ),
            "article_fast_route_throughput_bundle",
        )
        .expect("committed bundle");
        assert_eq!(
            normalized_bundle_value(&generated),
            normalized_bundle_value(&committed)
        );
    }

    #[test]
    fn write_fast_route_throughput_bundle_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let written =
            write_tassadar_article_fast_route_throughput_bundle(directory.path()).expect("write");
        let persisted: TassadarArticleFastRouteThroughputBundle = serde_json::from_slice(
            &std::fs::read(
                directory
                    .path()
                    .join(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE),
            )
            .expect("read"),
        )
        .expect("decode");
        assert_eq!(
            normalized_bundle_value(&written),
            normalized_bundle_value(&persisted)
        );
        assert_eq!(
            tassadar_article_fast_route_throughput_bundle_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_BUNDLE_FILE)
        );
    }
}
