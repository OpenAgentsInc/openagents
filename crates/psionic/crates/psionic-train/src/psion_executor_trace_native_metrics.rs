use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutorArticleCloseoutSetPacket, PsionExecutorLocalClusterCandidateStatus,
    PsionExecutorLocalClusterLedger, PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_TRACE_NATIVE_METRICS_SCHEMA_VERSION: &str =
    "psion.executor.trace_native_metrics.v1";
pub const PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_trace_native_metrics_v1.json";
pub const PSION_EXECUTOR_TRACE_NATIVE_METRICS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_TRACE_NATIVE_METRICS.md";

const PACKET_ID: &str = "psion_executor_trace_native_metrics_v1";
const BENCHMARK_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
const CLOSEOUT_WORKLOAD_IDS: [&str; 3] =
    ["hungarian_matching", "long_loop_kernel", "sudoku_v0_test_a"];
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD.md";
const PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET.md";

#[derive(Debug, Error)]
pub enum PsionExecutorTraceNativeMetricsError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error("missing closeout workload `{workload_id}` in benchmark report")]
    MissingWorkloadCase { workload_id: String },
    #[error("retained candidate rows must keep one shared model id")]
    SharedModelDrift,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleBenchmarkAggregateSummary {
    summary_digest: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct ArticleBenchmarkCaseReport {
    case_id: String,
    workload_target: String,
    status: String,
    score_bps: u64,
    final_output_exactness_bps: u64,
    step_exactness_bps: u64,
    halt_exactness_bps: u64,
    trace_digest_equal: bool,
    trace_steps: u64,
    reference_linear_steps_per_second: f64,
    hull_cache_steps_per_second: f64,
    hull_cache_speedup_over_reference_linear: f64,
    hull_cache_remaining_gap_vs_cpu_reference: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
struct ArticleBenchmarkReport {
    aggregate_summary: ArticleBenchmarkAggregateSummary,
    case_reports: Vec<ArticleBenchmarkCaseReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTraceNativeWorkloadMetricsRow {
    pub closeout_workload_id: String,
    pub benchmark_workload_target: String,
    pub benchmark_status: String,
    pub benchmark_score_bps: u64,
    pub trace_step_count: u64,
    pub final_output_exactness_bps: u64,
    pub step_exactness_bps: u64,
    pub halt_exactness_bps: u64,
    pub trace_digest_equal: bool,
    pub trace_digest_equal_bps: u64,
    pub reference_linear_steps_per_second: f64,
    pub hull_cache_steps_per_second: f64,
    pub hull_cache_speedup_over_reference_linear: f64,
    pub hull_cache_remaining_gap_vs_cpu_reference: f64,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTraceNativeCandidateRow {
    pub ledger_row_id: String,
    pub candidate_status: String,
    pub admitted_profile_id: String,
    pub model_id: String,
    pub run_id: String,
    pub shared_metric_source: String,
    pub workload_metrics: Vec<PsionExecutorTraceNativeWorkloadMetricsRow>,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTraceNativeMetricsPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub ledger_ref: String,
    pub ledger_digest: String,
    pub closeout_set_ref: String,
    pub closeout_set_digest: String,
    pub benchmark_report_ref: String,
    pub benchmark_report_sha256: String,
    pub benchmark_summary_digest: String,
    pub candidate_rows: Vec<PsionExecutorTraceNativeCandidateRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorTraceNativeWorkloadMetricsRow {
    fn validate(&self) -> Result<(), PsionExecutorTraceNativeMetricsError> {
        for (field, value) in [
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].closeout_workload_id",
                self.closeout_workload_id.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].benchmark_workload_target",
                self.benchmark_workload_target.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].benchmark_status",
                self.benchmark_status.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.trace_step_count == 0 {
            return Err(PsionExecutorTraceNativeMetricsError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].trace_step_count",
                ),
                detail: String::from("trace step count must stay positive"),
            });
        }
        for (field, value) in [
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].benchmark_score_bps",
                self.benchmark_score_bps,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].final_output_exactness_bps",
                self.final_output_exactness_bps,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].step_exactness_bps",
                self.step_exactness_bps,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].halt_exactness_bps",
                self.halt_exactness_bps,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].trace_digest_equal_bps",
                self.trace_digest_equal_bps,
            ),
        ] {
            if value > 10_000 {
                return Err(PsionExecutorTraceNativeMetricsError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("bps fields must stay within [0, 10000]"),
                });
            }
        }
        for (field, value) in [
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].reference_linear_steps_per_second",
                self.reference_linear_steps_per_second,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].hull_cache_steps_per_second",
                self.hull_cache_steps_per_second,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].hull_cache_speedup_over_reference_linear",
                self.hull_cache_speedup_over_reference_linear,
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].hull_cache_remaining_gap_vs_cpu_reference",
                self.hull_cache_remaining_gap_vs_cpu_reference,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorTraceNativeMetricsError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("metric must stay positive"),
                });
            }
        }
        if stable_workload_metrics_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTraceNativeMetricsError::DigestMismatch {
                field: String::from(
                    "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTraceNativeCandidateRow {
    fn validate(&self, closeout_workload_ids: &BTreeSet<String>) -> Result<(), PsionExecutorTraceNativeMetricsError> {
        for (field, value) in [
            (
                "psion_executor_trace_native_metrics.candidate_rows[].ledger_row_id",
                self.ledger_row_id.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].candidate_status",
                self.candidate_status.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].admitted_profile_id",
                self.admitted_profile_id.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].shared_metric_source",
                self.shared_metric_source.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.candidate_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.workload_metrics.len() != closeout_workload_ids.len() {
            return Err(PsionExecutorTraceNativeMetricsError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics",
                ),
                detail: String::from("candidate row must keep one metric row per closeout workload"),
            });
        }
        let mut seen = BTreeSet::new();
        for workload in &self.workload_metrics {
            workload.validate()?;
            seen.insert(workload.closeout_workload_id.clone());
        }
        if seen != *closeout_workload_ids {
            return Err(PsionExecutorTraceNativeMetricsError::InvalidValue {
                field: String::from(
                    "psion_executor_trace_native_metrics.candidate_rows[].workload_metrics.closeout_workload_id",
                ),
                detail: String::from("candidate row must keep the frozen closeout workload set"),
            });
        }
        if stable_candidate_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTraceNativeMetricsError::DigestMismatch {
                field: String::from("psion_executor_trace_native_metrics.candidate_rows[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTraceNativeMetricsPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorTraceNativeMetricsError> {
        if self.schema_version != PSION_EXECUTOR_TRACE_NATIVE_METRICS_SCHEMA_VERSION {
            return Err(PsionExecutorTraceNativeMetricsError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_trace_native_metrics.ledger_ref",
                self.ledger_ref.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.ledger_digest",
                self.ledger_digest.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.closeout_set_ref",
                self.closeout_set_ref.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.closeout_set_digest",
                self.closeout_set_digest.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.benchmark_report_ref",
                self.benchmark_report_ref.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.benchmark_report_sha256",
                self.benchmark_report_sha256.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.benchmark_summary_digest",
                self.benchmark_summary_digest.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_trace_native_metrics.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.candidate_rows.is_empty() {
            return Err(PsionExecutorTraceNativeMetricsError::MissingField {
                field: String::from("psion_executor_trace_native_metrics.candidate_rows"),
            });
        }
        let closeout_workload_ids = CLOSEOUT_WORKLOAD_IDS
            .into_iter()
            .map(String::from)
            .collect::<BTreeSet<_>>();
        let mut seen_rows = BTreeSet::new();
        for row in &self.candidate_rows {
            row.validate(&closeout_workload_ids)?;
            seen_rows.insert(row.ledger_row_id.clone());
        }
        if seen_rows.len() != self.candidate_rows.len() {
            return Err(PsionExecutorTraceNativeMetricsError::InvalidValue {
                field: String::from("psion_executor_trace_native_metrics.candidate_rows.ledger_row_id"),
                detail: String::from("candidate rows must stay unique"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorTraceNativeMetricsError::DigestMismatch {
                field: String::from("psion_executor_trace_native_metrics.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_trace_native_metrics_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorTraceNativeMetricsPacket, PsionExecutorTraceNativeMetricsError> {
    let closeout_packet: PsionExecutorArticleCloseoutSetPacket =
        read_json(workspace_root, PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH)?;
    let ledger: PsionExecutorLocalClusterLedger =
        read_json(workspace_root, PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH)?;
    let benchmark_report: ArticleBenchmarkReport = read_json(workspace_root, BENCHMARK_REPORT_PATH)?;

    let closeout_workload_ids = closeout_packet
        .workload_rows
        .iter()
        .map(|row| row.workload_id.clone())
        .collect::<BTreeSet<_>>();
    let candidate_rows = ledger
        .rows
        .iter()
        .map(|ledger_row| build_candidate_row(ledger_row, &closeout_workload_ids, &benchmark_report))
        .collect::<Result<Vec<_>, _>>()?;

    let shared_model_ids = candidate_rows
        .iter()
        .map(|row| row.model_id.clone())
        .collect::<BTreeSet<_>>();
    if shared_model_ids.len() != 1 {
        return Err(PsionExecutorTraceNativeMetricsError::SharedModelDrift);
    }

    let mut packet = PsionExecutorTraceNativeMetricsPacket {
        schema_version: String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        ledger_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        ledger_digest: ledger.ledger_digest.clone(),
        closeout_set_ref: String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH),
        closeout_set_digest: closeout_packet.packet_digest.clone(),
        benchmark_report_ref: String::from(BENCHMARK_REPORT_PATH),
        benchmark_report_sha256: sha256_for_path(workspace_root.join(BENCHMARK_REPORT_PATH))?,
        benchmark_summary_digest: benchmark_report.aggregate_summary.summary_digest.clone(),
        candidate_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_ARTICLE_CLOSEOUT_SET_FIXTURE_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
            String::from(BENCHMARK_REPORT_PATH),
        ],
        summary: String::from(
            "The executor lane now keeps the frozen bounded article closeout metrics visible per retained candidate row and per workload, binding trace length, exactness, and hull-cache throughput into the canonical ledger surface without pretending those benchmark facts are profile-specific when both rows still point at the same executor model.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    Ok(packet)
}

fn build_candidate_row(
    ledger_row: &crate::PsionExecutorLocalClusterLedgerRow,
    closeout_workload_ids: &BTreeSet<String>,
    benchmark_report: &ArticleBenchmarkReport,
) -> Result<PsionExecutorTraceNativeCandidateRow, PsionExecutorTraceNativeMetricsError> {
    let workload_metrics = closeout_workload_ids
        .iter()
        .map(|workload_id| -> Result<PsionExecutorTraceNativeWorkloadMetricsRow, PsionExecutorTraceNativeMetricsError> {
            let report_row = benchmark_report
                .case_reports
                .iter()
                .find(|row| row.case_id == *workload_id)
                .ok_or_else(|| PsionExecutorTraceNativeMetricsError::MissingWorkloadCase {
                    workload_id: workload_id.clone(),
                })?;
            let trace_digest_equal_bps = if report_row.trace_digest_equal { 10_000 } else { 0 };
            let detail = format!(
                "Ledger row `{}` now keeps closeout workload `{}` visible with trace-length, exactness, and hull-cache throughput facts lifted from `{}`. The benchmark workload target remains `{}`.",
                ledger_row.row_id, workload_id, BENCHMARK_REPORT_PATH, report_row.workload_target
            );
            let mut row = PsionExecutorTraceNativeWorkloadMetricsRow {
                closeout_workload_id: workload_id.clone(),
                benchmark_workload_target: report_row.workload_target.clone(),
                benchmark_status: report_row.status.clone(),
                benchmark_score_bps: report_row.score_bps,
                trace_step_count: report_row.trace_steps,
                final_output_exactness_bps: report_row.final_output_exactness_bps,
                step_exactness_bps: report_row.step_exactness_bps,
                halt_exactness_bps: report_row.halt_exactness_bps,
                trace_digest_equal: report_row.trace_digest_equal,
                trace_digest_equal_bps,
                reference_linear_steps_per_second: report_row.reference_linear_steps_per_second,
                hull_cache_steps_per_second: report_row.hull_cache_steps_per_second,
                hull_cache_speedup_over_reference_linear: report_row.hull_cache_speedup_over_reference_linear,
                hull_cache_remaining_gap_vs_cpu_reference: report_row.hull_cache_remaining_gap_vs_cpu_reference,
                detail,
                row_digest: String::new(),
            };
            row.row_digest = stable_workload_metrics_row_digest(&row);
            Ok(row)
        })
        .collect::<Result<Vec<_>, PsionExecutorTraceNativeMetricsError>>()?;

    let status = candidate_status_label(&ledger_row.candidate_status);
    let shared_metric_source = format!(
        "Retained candidate row `{}` shares the same executor model id `{}` as the other admitted ledger row, so the trace-native workload metrics stay model-bound and workload-specific rather than pretending to be per-profile training-throughput facts.",
        ledger_row.row_id, ledger_row.model_id
    );
    let mut row = PsionExecutorTraceNativeCandidateRow {
        ledger_row_id: ledger_row.row_id.clone(),
        candidate_status: String::from(status),
        admitted_profile_id: ledger_row.admitted_profile_id.clone(),
        model_id: ledger_row.model_id.clone(),
        run_id: ledger_row.run_id.clone(),
        shared_metric_source,
        workload_metrics,
        row_digest: String::new(),
    };
    row.row_digest = stable_candidate_row_digest(&row);
    Ok(row)
}

pub fn write_builtin_executor_trace_native_metrics_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorTraceNativeMetricsPacket, PsionExecutorTraceNativeMetricsError> {
    let packet = builtin_executor_trace_native_metrics_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn candidate_status_label(status: &PsionExecutorLocalClusterCandidateStatus) -> &'static str {
    match status {
        PsionExecutorLocalClusterCandidateStatus::CurrentBest => "current_best",
        PsionExecutorLocalClusterCandidateStatus::Candidate => "candidate",
    }
}

fn stable_workload_metrics_row_digest(row: &PsionExecutorTraceNativeWorkloadMetricsRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_digest(b"psion_executor_trace_native_workload_metrics_row|", &clone)
}

fn stable_candidate_row_digest(row: &PsionExecutorTraceNativeCandidateRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_digest(b"psion_executor_trace_native_candidate_row|", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorTraceNativeMetricsPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_digest(b"psion_executor_trace_native_metrics_packet|", &clone)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec_pretty(value).expect("serialize packet"));
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorTraceNativeMetricsError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorTraceNativeMetricsError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn sha256_for_path(path: PathBuf) -> Result<String, PsionExecutorTraceNativeMetricsError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorTraceNativeMetricsError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorTraceNativeMetricsError> {
    read_json_from_path(workspace_root.join(relative_path))
}

fn read_json_from_path<T: DeserializeOwned>(
    path: PathBuf,
) -> Result<T, PsionExecutorTraceNativeMetricsError> {
    let bytes = fs::read(&path).map_err(|error| PsionExecutorTraceNativeMetricsError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorTraceNativeMetricsError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorTraceNativeMetricsError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorTraceNativeMetricsError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorTraceNativeMetricsError::Write {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_trace_native_metrics_packet_is_valid(
    ) -> Result<(), PsionExecutorTraceNativeMetricsError> {
        let root = workspace_root();
        let packet = builtin_executor_trace_native_metrics_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_trace_native_metrics_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorTraceNativeMetricsError> {
        let root = workspace_root();
        let expected: PsionExecutorTraceNativeMetricsPacket =
            read_json(root.as_path(), PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH)?;
        let actual = builtin_executor_trace_native_metrics_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorTraceNativeMetricsError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_executor_trace_native_metrics_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorTraceNativeMetricsError> {
        let root = workspace_root();
        let packet = write_builtin_executor_trace_native_metrics_packet(root.as_path())?;
        let persisted: PsionExecutorTraceNativeMetricsPacket =
            read_json(root.as_path(), PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH)?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
