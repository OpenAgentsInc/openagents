use std::{
    collections::BTreeSet,
    fs,
    path::Path,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionExecutorTraceNativeCandidateRow, PsionExecutorTraceNativeMetricsPacket,
    PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_HULL_CACHE_BENCHMARK_SCHEMA_VERSION: &str =
    "psion.executor.hull_cache_benchmark.v1";
pub const PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_hull_cache_benchmark_v1.json";
pub const PSION_EXECUTOR_HULL_CACHE_BENCHMARK_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_HULL_CACHE_BENCHMARK.md";

const PACKET_ID: &str = "psion_executor_hull_cache_benchmark_v1";
const BENCHMARK_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_TRACE_NATIVE_METRICS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_TRACE_NATIVE_METRICS.md";

#[derive(Debug, Error)]
pub enum PsionExecutorHullCacheBenchmarkError {
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
    #[error("candidate row `{row_id}` has no workload benchmarks")]
    MissingWorkloadBenchmarks { row_id: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorHullCacheBenchmarkWorkloadRow {
    pub closeout_workload_id: String,
    pub benchmark_workload_target: String,
    pub reference_linear_steps_per_second: f64,
    pub hull_cache_steps_per_second: f64,
    pub hull_cache_speedup_over_reference_linear: f64,
    pub hull_cache_remaining_gap_vs_cpu_reference: f64,
    pub exactness_green: bool,
    pub trace_digest_green: bool,
    pub serving_truth_status: String,
    pub promotion_blocker: bool,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorHullCacheBenchmarkCandidateRow {
    pub trace_native_candidate_row_id: String,
    pub candidate_status: String,
    pub model_id: String,
    pub run_id: String,
    pub workload_rows: Vec<PsionExecutorHullCacheBenchmarkWorkloadRow>,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorHullCacheBenchmarkPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub trace_native_metrics_ref: String,
    pub trace_native_metrics_digest: String,
    pub benchmark_report_ref: String,
    pub benchmark_report_sha256: String,
    pub candidate_rows: Vec<PsionExecutorHullCacheBenchmarkCandidateRow>,
    pub min_speedup_over_reference_linear: f64,
    pub max_speedup_over_reference_linear: f64,
    pub max_remaining_gap_vs_cpu_reference: f64,
    pub all_serving_truth_green: bool,
    pub promotion_blocked: bool,
    pub promotion_block_reason: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorHullCacheBenchmarkWorkloadRow {
    fn validate(&self) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
        for (field, value) in [
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].closeout_workload_id",
                self.closeout_workload_id.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].benchmark_workload_target",
                self.benchmark_workload_target.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].serving_truth_status",
                self.serving_truth_status.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        for (field, value) in [
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].reference_linear_steps_per_second",
                self.reference_linear_steps_per_second,
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].hull_cache_steps_per_second",
                self.hull_cache_steps_per_second,
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].hull_cache_speedup_over_reference_linear",
                self.hull_cache_speedup_over_reference_linear,
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].hull_cache_remaining_gap_vs_cpu_reference",
                self.hull_cache_remaining_gap_vs_cpu_reference,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorHullCacheBenchmarkError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("benchmark metric must stay positive"),
                });
            }
        }
        if stable_workload_row_digest(self) != self.row_digest {
            return Err(PsionExecutorHullCacheBenchmarkError::DigestMismatch {
                field: String::from(
                    "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorHullCacheBenchmarkCandidateRow {
    fn validate(&self) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
        for (field, value) in [
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].trace_native_candidate_row_id",
                self.trace_native_candidate_row_id.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].candidate_status",
                self.candidate_status.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.candidate_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.workload_rows.is_empty() {
            return Err(PsionExecutorHullCacheBenchmarkError::MissingWorkloadBenchmarks {
                row_id: self.trace_native_candidate_row_id.clone(),
            });
        }
        let mut seen = BTreeSet::new();
        for row in &self.workload_rows {
            row.validate()?;
            seen.insert(row.closeout_workload_id.clone());
        }
        if seen.len() != self.workload_rows.len() {
            return Err(PsionExecutorHullCacheBenchmarkError::InvalidValue {
                field: String::from(
                    "psion_executor_hull_cache_benchmark.candidate_rows[].workload_rows.closeout_workload_id",
                ),
                detail: String::from("workload benchmarks must stay unique per candidate row"),
            });
        }
        if stable_candidate_row_digest(self) != self.row_digest {
            return Err(PsionExecutorHullCacheBenchmarkError::DigestMismatch {
                field: String::from("psion_executor_hull_cache_benchmark.candidate_rows[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorHullCacheBenchmarkPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
        if self.schema_version != PSION_EXECUTOR_HULL_CACHE_BENCHMARK_SCHEMA_VERSION {
            return Err(PsionExecutorHullCacheBenchmarkError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_hull_cache_benchmark.trace_native_metrics_ref",
                self.trace_native_metrics_ref.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.trace_native_metrics_digest",
                self.trace_native_metrics_digest.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.benchmark_report_ref",
                self.benchmark_report_ref.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.benchmark_report_sha256",
                self.benchmark_report_sha256.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.promotion_block_reason",
                self.promotion_block_reason.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_hull_cache_benchmark.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.candidate_rows.is_empty() {
            return Err(PsionExecutorHullCacheBenchmarkError::MissingField {
                field: String::from("psion_executor_hull_cache_benchmark.candidate_rows"),
            });
        }
        let mut seen = BTreeSet::new();
        for row in &self.candidate_rows {
            row.validate()?;
            seen.insert(row.trace_native_candidate_row_id.clone());
        }
        if seen.len() != self.candidate_rows.len() {
            return Err(PsionExecutorHullCacheBenchmarkError::InvalidValue {
                field: String::from("psion_executor_hull_cache_benchmark.candidate_rows.trace_native_candidate_row_id"),
                detail: String::from("candidate row ids must stay unique"),
            });
        }
        if self.min_speedup_over_reference_linear <= 0.0
            || self.max_speedup_over_reference_linear <= 0.0
            || self.max_remaining_gap_vs_cpu_reference <= 0.0
        {
            return Err(PsionExecutorHullCacheBenchmarkError::InvalidValue {
                field: String::from("psion_executor_hull_cache_benchmark.aggregate_metrics"),
                detail: String::from("aggregate benchmark metrics must stay positive"),
            });
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorHullCacheBenchmarkError::DigestMismatch {
                field: String::from("psion_executor_hull_cache_benchmark.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_hull_cache_benchmark_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorHullCacheBenchmarkPacket, PsionExecutorHullCacheBenchmarkError> {
    let trace_native: PsionExecutorTraceNativeMetricsPacket =
        read_json(workspace_root, PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH)?;

    let candidate_rows = trace_native
        .candidate_rows
        .iter()
        .map(build_candidate_row)
        .collect::<Result<Vec<_>, _>>()?;

    let workload_rows = candidate_rows
        .iter()
        .flat_map(|row| row.workload_rows.iter())
        .collect::<Vec<_>>();
    let min_speedup_over_reference_linear = workload_rows
        .iter()
        .map(|row| row.hull_cache_speedup_over_reference_linear)
        .fold(f64::INFINITY, f64::min);
    let max_speedup_over_reference_linear = workload_rows
        .iter()
        .map(|row| row.hull_cache_speedup_over_reference_linear)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_remaining_gap_vs_cpu_reference = workload_rows
        .iter()
        .map(|row| row.hull_cache_remaining_gap_vs_cpu_reference)
        .fold(f64::NEG_INFINITY, f64::max);
    let all_serving_truth_green = workload_rows
        .iter()
        .all(|row| row.serving_truth_status == "green");
    let promotion_blocked = workload_rows.iter().any(|row| row.promotion_blocker);
    let promotion_block_reason = if promotion_blocked {
        String::from("fast_route_serving_truth_red")
    } else {
        String::from("none_all_closeout_workloads_green")
    };

    let mut packet = PsionExecutorHullCacheBenchmarkPacket {
        schema_version: String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        trace_native_metrics_ref: String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH),
        trace_native_metrics_digest: trace_native.packet_digest.clone(),
        benchmark_report_ref: trace_native.benchmark_report_ref.clone(),
        benchmark_report_sha256: trace_native.benchmark_report_sha256.clone(),
        candidate_rows,
        min_speedup_over_reference_linear,
        max_speedup_over_reference_linear,
        max_remaining_gap_vs_cpu_reference,
        all_serving_truth_green,
        promotion_blocked,
        promotion_block_reason,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_DOC_PATH),
            String::from(PSION_EXECUTOR_TRACE_NATIVE_METRICS_FIXTURE_PATH),
            String::from(BENCHMARK_REPORT_PATH),
        ],
        summary: String::from(
            "The executor lane now keeps one explicit `HullKVCache` versus `reference_linear` benchmark packet over the frozen bounded article closeout trio, including speedup ratios and an explicit promotion-block rule when fast-route serving truth turns red.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    Ok(packet)
}

fn build_candidate_row(
    source: &PsionExecutorTraceNativeCandidateRow,
) -> Result<PsionExecutorHullCacheBenchmarkCandidateRow, PsionExecutorHullCacheBenchmarkError> {
    let workload_rows = source
        .workload_metrics
        .iter()
        .map(|metric| -> Result<PsionExecutorHullCacheBenchmarkWorkloadRow, PsionExecutorHullCacheBenchmarkError> {
            let exactness_green = metric.final_output_exactness_bps == 10_000
                && metric.step_exactness_bps == 10_000
                && metric.halt_exactness_bps == 10_000;
            let trace_digest_green = metric.trace_digest_equal_bps == 10_000;
            let serving_truth_status = if exactness_green && trace_digest_green {
                "green"
            } else {
                "red"
            };
            let promotion_blocker = serving_truth_status == "red";
            let detail = format!(
                "Closeout workload `{}` on candidate row `{}` keeps `reference_linear` at {:.6} steps/s and `hull_cache` at {:.6} steps/s for speedup {:.6}. Serving truth stays `{}`.",
                metric.closeout_workload_id,
                source.ledger_row_id,
                metric.reference_linear_steps_per_second,
                metric.hull_cache_steps_per_second,
                metric.hull_cache_speedup_over_reference_linear,
                serving_truth_status
            );
            let mut row = PsionExecutorHullCacheBenchmarkWorkloadRow {
                closeout_workload_id: metric.closeout_workload_id.clone(),
                benchmark_workload_target: metric.benchmark_workload_target.clone(),
                reference_linear_steps_per_second: metric.reference_linear_steps_per_second,
                hull_cache_steps_per_second: metric.hull_cache_steps_per_second,
                hull_cache_speedup_over_reference_linear: metric.hull_cache_speedup_over_reference_linear,
                hull_cache_remaining_gap_vs_cpu_reference: metric.hull_cache_remaining_gap_vs_cpu_reference,
                exactness_green,
                trace_digest_green,
                serving_truth_status: String::from(serving_truth_status),
                promotion_blocker,
                detail,
                row_digest: String::new(),
            };
            row.row_digest = stable_workload_row_digest(&row);
            Ok(row)
        })
        .collect::<Result<Vec<_>, PsionExecutorHullCacheBenchmarkError>>()?;

    let mut row = PsionExecutorHullCacheBenchmarkCandidateRow {
        trace_native_candidate_row_id: source.ledger_row_id.clone(),
        candidate_status: source.candidate_status.clone(),
        model_id: source.model_id.clone(),
        run_id: source.run_id.clone(),
        workload_rows,
        row_digest: String::new(),
    };
    row.row_digest = stable_candidate_row_digest(&row);
    Ok(row)
}

pub fn write_builtin_executor_hull_cache_benchmark_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorHullCacheBenchmarkPacket, PsionExecutorHullCacheBenchmarkError> {
    let packet = builtin_executor_hull_cache_benchmark_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn stable_workload_row_digest(row: &PsionExecutorHullCacheBenchmarkWorkloadRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_digest(b"psion_executor_hull_cache_benchmark_workload_row|", &clone)
}

fn stable_candidate_row_digest(row: &PsionExecutorHullCacheBenchmarkCandidateRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_digest(b"psion_executor_hull_cache_benchmark_candidate_row|", &clone)
}

fn stable_packet_digest(packet: &PsionExecutorHullCacheBenchmarkPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_digest(b"psion_executor_hull_cache_benchmark_packet|", &clone)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec_pretty(value).expect("serialize packet"));
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorHullCacheBenchmarkError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorHullCacheBenchmarkError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorHullCacheBenchmarkError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorHullCacheBenchmarkError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorHullCacheBenchmarkError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorHullCacheBenchmarkError::Write {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_hull_cache_benchmark_packet_is_valid(
    ) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
        let root = workspace_root();
        let packet = builtin_executor_hull_cache_benchmark_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_hull_cache_benchmark_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
        let root = workspace_root();
        let expected: PsionExecutorHullCacheBenchmarkPacket =
            read_json(root.as_path(), PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH)?;
        let actual = builtin_executor_hull_cache_benchmark_packet(root.as_path())?;
        if expected != actual {
            return Err(PsionExecutorHullCacheBenchmarkError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn write_executor_hull_cache_benchmark_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorHullCacheBenchmarkError> {
        let root = workspace_root();
        let packet = write_builtin_executor_hull_cache_benchmark_packet(root.as_path())?;
        let persisted: PsionExecutorHullCacheBenchmarkPacket =
            read_json(root.as_path(), PSION_EXECUTOR_HULL_CACHE_BENCHMARK_FIXTURE_PATH)?;
        assert_eq!(packet, persisted);
        Ok(())
    }
}
