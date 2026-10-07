use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    tassadar_approximate_attention_closure_publication,
    TassadarApproximateAttentionClosurePublication,
    TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_MATRIX_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_approximate_attention_closure_runtime_report,
    TassadarApproximateAttentionClosureOutcome,
    TassadarApproximateAttentionClosureRuntimeError,
    TassadarApproximateAttentionClosureRuntimeReport, TassadarApproximateAttentionFamily,
    TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Aggregate summary for one approximate-attention family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionFamilySummary {
    /// Attention family being summarized.
    pub attention_family: TassadarApproximateAttentionFamily,
    /// Direct workload count.
    pub direct_workload_count: u32,
    /// Degraded-but-bounded workload count.
    pub degraded_workload_count: u32,
    /// Refused workload count.
    pub refused_workload_count: u32,
    /// Best direct speedup over dense reference on the current matrix.
    pub best_direct_speedup_over_dense_reference: f64,
    /// Plain-language note.
    pub note: String,
}

/// Aggregate summary for one workload target across families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionWorkloadSummary {
    /// Workload target being summarized.
    pub workload_target: String,
    /// Families that stay direct on the workload.
    pub direct_families: Vec<String>,
    /// Families that stay degraded-but-bounded on the workload.
    pub degraded_families: Vec<String>,
    /// Families that refuse on the workload.
    pub refused_families: Vec<String>,
    /// Plain-language note.
    pub note: String,
}

/// Committed eval matrix over approximate-attention closure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionClosureMatrixReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Publication anchoring the matrix.
    pub publication: TassadarApproximateAttentionClosurePublication,
    /// Runtime report consumed by the matrix.
    pub runtime_report: TassadarApproximateAttentionClosureRuntimeReport,
    /// Ordered family summaries.
    pub family_summaries: Vec<TassadarApproximateAttentionFamilySummary>,
    /// Ordered workload summaries.
    pub workload_summaries: Vec<TassadarApproximateAttentionWorkloadSummary>,
    /// Ordered refs used to generate the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarApproximateAttentionClosureMatrixReport {
    fn new(
        publication: TassadarApproximateAttentionClosurePublication,
        runtime_report: TassadarApproximateAttentionClosureRuntimeReport,
        family_summaries: Vec<TassadarApproximateAttentionFamilySummary>,
        workload_summaries: Vec<TassadarApproximateAttentionWorkloadSummary>,
    ) -> Self {
        let refusal_heavy_workload_count = workload_summaries
            .iter()
            .filter(|summary| summary.refused_families.len() >= 2)
            .count();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.approximate_attention_closure.matrix.v1"),
            publication,
            runtime_report,
            family_summaries,
            workload_summaries,
            generated_from_refs: vec![
                String::from(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_RUNTIME_REPORT_REF),
                String::from(
                    "fixtures/tassadar/reports/tassadar_efficient_attention_baseline_matrix.json",
                ),
            ],
            claim_boundary: String::from(
                "this eval report is a machine-legible closure matrix over the current bounded workload rows. It keeps direct, degraded_but_bounded, and refused posture explicit for each attention family, and it does not promote any approximate-attention family to served capability by itself",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Approximate-attention closure matrix now summarizes {} families across {} workloads, with {} workloads carrying refusal from at least two non-dense families.",
            report.family_summaries.len(),
            report.workload_summaries.len(),
            refusal_heavy_workload_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_approximate_attention_closure_matrix_report|",
            &report,
        );
        report
    }
}

/// Closure-matrix build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarApproximateAttentionClosureMatrixError {
    #[error(transparent)]
    Runtime(#[from] TassadarApproximateAttentionClosureRuntimeError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed approximate-attention closure matrix report.
pub fn build_tassadar_approximate_attention_closure_matrix_report(
) -> Result<
    TassadarApproximateAttentionClosureMatrixReport,
    TassadarApproximateAttentionClosureMatrixError,
> {
    let publication = tassadar_approximate_attention_closure_publication();
    let runtime_report = build_tassadar_approximate_attention_closure_runtime_report()?;
    let family_summaries = build_family_summaries(&runtime_report);
    let workload_summaries = build_workload_summaries(&runtime_report);
    Ok(TassadarApproximateAttentionClosureMatrixReport::new(
        publication,
        runtime_report,
        family_summaries,
        workload_summaries,
    ))
}

/// Returns the canonical absolute path for the committed matrix report.
#[must_use]
pub fn tassadar_approximate_attention_closure_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_MATRIX_REPORT_REF)
}

/// Writes the committed approximate-attention closure matrix report.
pub fn write_tassadar_approximate_attention_closure_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarApproximateAttentionClosureMatrixReport,
    TassadarApproximateAttentionClosureMatrixError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarApproximateAttentionClosureMatrixError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_approximate_attention_closure_matrix_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarApproximateAttentionClosureMatrixError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_family_summaries(
    runtime_report: &TassadarApproximateAttentionClosureRuntimeReport,
) -> Vec<TassadarApproximateAttentionFamilySummary> {
    let mut grouped = BTreeMap::<TassadarApproximateAttentionFamily, Vec<_>>::new();
    for receipt in &runtime_report.closure_receipts {
        grouped
            .entry(receipt.attention_family)
            .or_default()
            .push(receipt);
    }
    grouped
        .into_iter()
        .map(|(attention_family, receipts)| TassadarApproximateAttentionFamilySummary {
            attention_family,
            direct_workload_count: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
                })
                .count() as u32,
            degraded_workload_count: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome
                        == TassadarApproximateAttentionClosureOutcome::DegradedButBounded
                })
                .count() as u32,
            refused_workload_count: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Refused
                })
                .count() as u32,
            best_direct_speedup_over_dense_reference: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
                })
                .map(|receipt| receipt.average_speedup_over_dense_reference)
                .fold(0.0, f64::max),
            note: format!(
                "family `{}` now has {} direct, {} degraded, and {} refused workload rows",
                attention_family.as_str(),
                receipts
                    .iter()
                    .filter(|receipt| receipt.closure_outcome
                        == TassadarApproximateAttentionClosureOutcome::Direct)
                    .count(),
                receipts
                    .iter()
                    .filter(|receipt| receipt.closure_outcome
                        == TassadarApproximateAttentionClosureOutcome::DegradedButBounded)
                    .count(),
                receipts
                    .iter()
                    .filter(|receipt| receipt.closure_outcome
                        == TassadarApproximateAttentionClosureOutcome::Refused)
                    .count(),
            ),
        })
        .collect()
}

fn build_workload_summaries(
    runtime_report: &TassadarApproximateAttentionClosureRuntimeReport,
) -> Vec<TassadarApproximateAttentionWorkloadSummary> {
    let mut grouped = BTreeMap::<String, Vec<_>>::new();
    for receipt in &runtime_report.closure_receipts {
        grouped
            .entry(receipt.workload_target.clone())
            .or_default()
            .push(receipt);
    }
    grouped
        .into_iter()
        .map(|(workload_target, receipts)| TassadarApproximateAttentionWorkloadSummary {
            workload_target: workload_target.clone(),
            direct_families: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Direct
                })
                .map(|receipt| String::from(receipt.attention_family.as_str()))
                .collect(),
            degraded_families: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome
                        == TassadarApproximateAttentionClosureOutcome::DegradedButBounded
                })
                .map(|receipt| String::from(receipt.attention_family.as_str()))
                .collect(),
            refused_families: receipts
                .iter()
                .filter(|receipt| {
                    receipt.closure_outcome == TassadarApproximateAttentionClosureOutcome::Refused
                })
                .map(|receipt| String::from(receipt.attention_family.as_str()))
                .collect(),
            note: format!(
                "workload `{}` keeps approximate-attention closure split into direct, degraded, and refused families instead of one blended score",
                workload_target,
            ),
        })
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
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
) -> Result<T, TassadarApproximateAttentionClosureMatrixError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarApproximateAttentionClosureMatrixError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarApproximateAttentionClosureMatrixError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_approximate_attention_closure_matrix_report, read_repo_json,
        tassadar_approximate_attention_closure_matrix_report_path,
        write_tassadar_approximate_attention_closure_matrix_report,
        TassadarApproximateAttentionClosureMatrixReport,
    };
    use psionic_models::TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_MATRIX_REPORT_REF;
    use psionic_runtime::TassadarApproximateAttentionFamily;

    #[test]
    fn approximate_attention_closure_matrix_tracks_refusal_hotspots() {
        let report = build_tassadar_approximate_attention_closure_matrix_report()
            .expect("matrix report");
        let sudoku = report
            .workload_summaries
            .iter()
            .find(|summary| summary.workload_target == "sudoku_class")
            .expect("sudoku summary");
        let lsh = report
            .family_summaries
            .iter()
            .find(|summary| {
                summary.attention_family == TassadarApproximateAttentionFamily::LshBucketedProxy
            })
            .expect("lsh summary");

        assert!(sudoku
            .refused_families
            .contains(&String::from("lsh_bucketed_proxy")));
        assert_eq!(lsh.refused_workload_count, 2);
    }

    #[test]
    fn approximate_attention_closure_matrix_matches_committed_truth() {
        let generated = build_tassadar_approximate_attention_closure_matrix_report()
            .expect("matrix report");
        let committed: TassadarApproximateAttentionClosureMatrixReport =
            read_repo_json(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_MATRIX_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_approximate_attention_closure_matrix_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_approximate_attention_closure_matrix.json");
        let written = write_tassadar_approximate_attention_closure_matrix_report(&output_path)
            .expect("write");
        let persisted: TassadarApproximateAttentionClosureMatrixReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_approximate_attention_closure_matrix_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_approximate_attention_closure_matrix.json")
        );
    }
}
