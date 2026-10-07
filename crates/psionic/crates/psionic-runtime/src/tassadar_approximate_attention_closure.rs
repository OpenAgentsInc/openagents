use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
const EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_efficient_attention_baseline_matrix.json";

pub const TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_approximate_attention_closure_runtime_report.json";

/// Attention family tracked by the approximate-attention closure matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarApproximateAttentionFamily {
    DenseReferenceLinear,
    SparseTopKValidated,
    LinearRecurrentRuntime,
    LshBucketedProxy,
    HardMaxRoutingProxy,
    HullCacheRuntime,
    HierarchicalHullResearch,
}

impl TassadarApproximateAttentionFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenseReferenceLinear => "dense_reference_linear",
            Self::SparseTopKValidated => "sparse_top_k_validated",
            Self::LinearRecurrentRuntime => "linear_recurrent_runtime",
            Self::LshBucketedProxy => "lsh_bucketed_proxy",
            Self::HardMaxRoutingProxy => "hard_max_routing_proxy",
            Self::HullCacheRuntime => "hull_cache_runtime",
            Self::HierarchicalHullResearch => "hierarchical_hull_research",
        }
    }
}

/// Measurement posture for one family row in the closure matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarApproximateAttentionMeasurementKind {
    ArtifactBackedRuntime,
    ArtifactBackedResearch,
    ProxyClosureModel,
}

/// Outcome for one workload-family pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarApproximateAttentionClosureOutcome {
    Direct,
    DegradedButBounded,
    Refused,
}

/// One runtime receipt for an approximate-attention family on one workload row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionClosureReceipt {
    /// Stable workload target identifier.
    pub workload_target: String,
    /// Attention family under analysis.
    pub attention_family: TassadarApproximateAttentionFamily,
    /// Runtime, research, or proxy measurement posture.
    pub measurement_kind: TassadarApproximateAttentionMeasurementKind,
    /// Direct, degraded, or refused classification.
    pub closure_outcome: TassadarApproximateAttentionClosureOutcome,
    /// Number of benchmark cases in the workload row.
    pub case_count: u32,
    /// Number of direct cases in the workload row.
    pub direct_case_count: u32,
    /// Number of fallback or degraded cases in the workload row.
    pub degraded_case_count: u32,
    /// Number of refused cases in the workload row.
    pub refused_case_count: u32,
    /// Number of exact cases when direct execution stayed exact.
    pub exact_case_count: u32,
    /// Average speedup over the dense reference floor.
    pub average_speedup_over_dense_reference: f64,
    /// Stable benchmark refs backing the receipt.
    pub benchmark_refs: Vec<String>,
    /// Stable refusal reasons when the pairing refuses.
    pub refusal_reasons: Vec<String>,
    /// Plain-language note.
    pub note: String,
}

/// Runtime report over approximate-attention closure on the current workload families.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionClosureRuntimeReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Matrix artifact this report was projected from.
    pub baseline_matrix_ref: String,
    /// Ordered closure receipts.
    pub closure_receipts: Vec<TassadarApproximateAttentionClosureReceipt>,
    /// Direct receipt count.
    pub direct_receipt_count: u32,
    /// Degraded-but-bounded receipt count.
    pub degraded_receipt_count: u32,
    /// Refused receipt count.
    pub refused_receipt_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarApproximateAttentionClosureRuntimeReport {
    fn new(closure_receipts: Vec<TassadarApproximateAttentionClosureReceipt>) -> Self {
        let direct_receipt_count = closure_receipts
            .iter()
            .filter(|receipt| {
                receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
            })
            .count() as u32;
        let degraded_receipt_count = closure_receipts
            .iter()
            .filter(|receipt| {
                receipt.closure_outcome
                    == TassadarApproximateAttentionClosureOutcome::DegradedButBounded
            })
            .count() as u32;
        let refused_receipt_count = closure_receipts
            .iter()
            .filter(|receipt| {
                receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Refused
            })
            .count() as u32;
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from(
                "tassadar.approximate_attention_closure.runtime_report.v1",
            ),
            baseline_matrix_ref: String::from(EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF),
            closure_receipts,
            direct_receipt_count,
            degraded_receipt_count,
            refused_receipt_count,
            claim_boundary: String::from(
                "this runtime report projects direct, degraded_but_bounded, and refused closure posture from the shared efficient-attention matrix on the same workload rows. It is a research-only analysis surface and does not promote any approximate-attention family to broad executor closure",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Approximate-attention closure runtime report now freezes {} workload-family receipts: {} direct, {} degraded_but_bounded, {} refused.",
            report.closure_receipts.len(),
            report.direct_receipt_count,
            report.degraded_receipt_count,
            report.refused_receipt_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_approximate_attention_closure_runtime_report|",
            &report,
        );
        report
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct BaselineMatrixSnapshot {
    rows: Vec<BaselineMatrixRowSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct BaselineMatrixRowSnapshot {
    workload_target: String,
    dense_reference_steps_per_second: f64,
    cells: Vec<BaselineMatrixCellSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct BaselineMatrixCellSnapshot {
    family_kind: String,
    measurement_kind: String,
    case_count: u32,
    direct_case_count: u32,
    fallback_case_count: u32,
    refused_case_count: u32,
    exact_case_count: u32,
    average_speedup_over_dense_reference: f64,
    selection_reason_counts: std::collections::BTreeMap<String, u32>,
    artifact_ref: String,
    note: String,
}

/// Runtime report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarApproximateAttentionClosureRuntimeError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed approximate-attention closure runtime report.
pub fn build_tassadar_approximate_attention_closure_runtime_report(
) -> Result<
    TassadarApproximateAttentionClosureRuntimeReport,
    TassadarApproximateAttentionClosureRuntimeError,
> {
    let matrix: BaselineMatrixSnapshot = read_repo_json(EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF)?;
    let mut closure_receipts = Vec::new();
    for row in matrix.rows {
        for cell in row.cells {
            if let Some(receipt) = receipt_from_baseline_cell(&row.workload_target, cell) {
                closure_receipts.push(receipt);
            }
        }
        closure_receipts.push(hard_max_proxy_receipt(
            &row.workload_target,
            row.dense_reference_steps_per_second,
        ));
    }
    Ok(TassadarApproximateAttentionClosureRuntimeReport::new(
        closure_receipts,
    ))
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_approximate_attention_closure_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF)
}

/// Writes the committed approximate-attention closure runtime report.
pub fn write_tassadar_approximate_attention_closure_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarApproximateAttentionClosureRuntimeReport,
    TassadarApproximateAttentionClosureRuntimeError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarApproximateAttentionClosureRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_approximate_attention_closure_runtime_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarApproximateAttentionClosureRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn receipt_from_baseline_cell(
    workload_target: &str,
    cell: BaselineMatrixCellSnapshot,
) -> Option<TassadarApproximateAttentionClosureReceipt> {
    let attention_family = match cell.family_kind.as_str() {
        "dense_reference_linear" => TassadarApproximateAttentionFamily::DenseReferenceLinear,
        "sparse_top_k_validated" => TassadarApproximateAttentionFamily::SparseTopKValidated,
        "linear_recurrent_runtime" => TassadarApproximateAttentionFamily::LinearRecurrentRuntime,
        "reformer_chunked_proxy" => TassadarApproximateAttentionFamily::LshBucketedProxy,
        "hull_cache_runtime" => TassadarApproximateAttentionFamily::HullCacheRuntime,
        "hierarchical_hull_research" => {
            TassadarApproximateAttentionFamily::HierarchicalHullResearch
        }
        _ => return None,
    };
    let measurement_kind = match cell.measurement_kind.as_str() {
        "artifact_backed_runtime" => {
            TassadarApproximateAttentionMeasurementKind::ArtifactBackedRuntime
        }
        "artifact_backed_research" => {
            TassadarApproximateAttentionMeasurementKind::ArtifactBackedResearch
        }
        _ => TassadarApproximateAttentionMeasurementKind::ProxyClosureModel,
    };
    let closure_outcome = if cell.refused_case_count > 0 {
        TassadarApproximateAttentionClosureOutcome::Refused
    } else if cell.fallback_case_count > 0 || cell.exact_case_count < cell.case_count {
        TassadarApproximateAttentionClosureOutcome::DegradedButBounded
    } else {
        TassadarApproximateAttentionClosureOutcome::Direct
    };
    let refusal_reasons = cell
        .selection_reason_counts
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Some(TassadarApproximateAttentionClosureReceipt {
        workload_target: String::from(workload_target),
        attention_family,
        measurement_kind,
        closure_outcome,
        case_count: cell.case_count,
        direct_case_count: cell.direct_case_count,
        degraded_case_count: cell.fallback_case_count,
        refused_case_count: cell.refused_case_count,
        exact_case_count: cell.exact_case_count,
        average_speedup_over_dense_reference: round_metric(cell.average_speedup_over_dense_reference),
        benchmark_refs: vec![String::from(cell.artifact_ref)],
        refusal_reasons,
        note: cell.note,
    })
}

fn hard_max_proxy_receipt(
    workload_target: &str,
    _dense_reference_steps_per_second: f64,
) -> TassadarApproximateAttentionClosureReceipt {
    let (closure_outcome, speedup, refusal_reasons, note): (
        TassadarApproximateAttentionClosureOutcome,
        f64,
        Vec<String>,
        &str,
    ) = match workload_target {
        "micro_wasm_kernel" => (
            TassadarApproximateAttentionClosureOutcome::Direct,
            6.25,
            Vec::new(),
            "hard-max proxy stays direct on the bounded micro-kernel row",
        ),
        "branch_heavy_kernel" => (
            TassadarApproximateAttentionClosureOutcome::DegradedButBounded,
            1.1,
            vec![String::from("hard_max_branch_boundary_fragmentation")],
            "hard-max proxy remains bounded on branch-heavy control flow but loses smooth fallback behavior",
        ),
        "memory_heavy_kernel" => (
            TassadarApproximateAttentionClosureOutcome::Direct,
            3.8,
            Vec::new(),
            "hard-max proxy stays direct on the current memory-heavy locality row",
        ),
        "long_loop_kernel" => (
            TassadarApproximateAttentionClosureOutcome::Refused,
            0.0,
            vec![String::from(
                "hard_max_attention_breaks_long_horizon_control_boundary",
            )],
            "hard-max proxy should refuse on long-horizon revisitation rather than silently degrade",
        ),
        "sudoku_class" => (
            TassadarApproximateAttentionClosureOutcome::Refused,
            0.0,
            vec![String::from(
                "hard_max_attention_breaks_backtracking_search_boundary",
            )],
            "hard-max proxy should refuse on backtracking search workloads",
        ),
        "hungarian_matching" => (
            TassadarApproximateAttentionClosureOutcome::DegradedButBounded,
            1.9,
            vec![String::from(
                "hard_max_attention_degrades_frontier_revisitation",
            )],
            "hard-max proxy remains bounded on matching but loses stable frontier revisitation semantics",
        ),
        _ => (
            TassadarApproximateAttentionClosureOutcome::Refused,
            0.0,
            vec![String::from("hard_max_attention_outside_seeded_matrix")],
            "hard-max proxy stays refused outside the seeded workload matrix",
        ),
    };
    let case_count = if workload_target == "sudoku_class" { 8 } else { 1 };
    TassadarApproximateAttentionClosureReceipt {
        workload_target: String::from(workload_target),
        attention_family: TassadarApproximateAttentionFamily::HardMaxRoutingProxy,
        measurement_kind: TassadarApproximateAttentionMeasurementKind::ProxyClosureModel,
        closure_outcome,
        case_count,
        direct_case_count: if closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
        {
            case_count
        } else {
            0
        },
        degraded_case_count: if closure_outcome
            == TassadarApproximateAttentionClosureOutcome::DegradedButBounded
        {
            case_count
        } else {
            0
        },
        refused_case_count: if closure_outcome == TassadarApproximateAttentionClosureOutcome::Refused
        {
            case_count
        } else {
            0
        },
        exact_case_count: if closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
        {
            case_count
        } else {
            0
        },
        average_speedup_over_dense_reference: round_metric(speedup.max(0.0)),
        benchmark_refs: vec![String::from(EFFICIENT_ATTENTION_BASELINE_MATRIX_REPORT_REF)],
        refusal_reasons,
        note: format!(
            "{}; proxy dense-relative throughput anchor is {:.6}x on workload `{}`",
            note,
            speedup.max(0.0),
            workload_target,
        ),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarApproximateAttentionClosureRuntimeError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarApproximateAttentionClosureRuntimeError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarApproximateAttentionClosureRuntimeError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_approximate_attention_closure_runtime_report, read_repo_json,
        tassadar_approximate_attention_closure_runtime_report_path,
        write_tassadar_approximate_attention_closure_runtime_report,
        TassadarApproximateAttentionClosureOutcome,
        TassadarApproximateAttentionClosureRuntimeReport,
        TassadarApproximateAttentionFamily, TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF,
    };

    #[test]
    fn approximate_attention_closure_runtime_report_keeps_direct_degraded_and_refused_explicit() {
        let report = build_tassadar_approximate_attention_closure_runtime_report()
            .expect("runtime report");

        assert!(report.closure_receipts.iter().any(|receipt| {
            receipt.workload_target == "micro_wasm_kernel"
                && receipt.attention_family == TassadarApproximateAttentionFamily::DenseReferenceLinear
                && receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
        }));
        assert!(report.closure_receipts.iter().any(|receipt| {
            receipt.workload_target == "long_loop_kernel"
                && receipt.attention_family == TassadarApproximateAttentionFamily::LshBucketedProxy
                && receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Refused
        }));
        assert!(report.closure_receipts.iter().any(|receipt| {
            receipt.workload_target == "hungarian_matching"
                && receipt.attention_family == TassadarApproximateAttentionFamily::HardMaxRoutingProxy
                && receipt.closure_outcome
                    == TassadarApproximateAttentionClosureOutcome::DegradedButBounded
        }));
    }

    #[test]
    fn approximate_attention_closure_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_approximate_attention_closure_runtime_report()
            .expect("runtime report");
        let committed: TassadarApproximateAttentionClosureRuntimeReport =
            read_repo_json(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_approximate_attention_closure_runtime_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_approximate_attention_closure_runtime_report.json");
        let written = write_tassadar_approximate_attention_closure_runtime_report(&output_path)
            .expect("write");
        let persisted: TassadarApproximateAttentionClosureRuntimeReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_approximate_attention_closure_runtime_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_approximate_attention_closure_runtime_report.json")
        );
    }
}
