use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::TassadarWeakSupervisionWorkloadFamily;
use psionic_data::TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF;
use psionic_eval::{
    build_tassadar_weak_supervision_executor_report, TassadarWeakSupervisionExecutorReport,
    TassadarWeakSupervisionExecutorReportError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Research-facing summary over the weak-supervision executor report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionExecutorSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Source eval report.
    pub eval_report: TassadarWeakSupervisionExecutorReport,
    /// Workloads where mixed supervision recovers most of the full-trace benefit.
    pub mixed_viable_workloads: Vec<TassadarWeakSupervisionWorkloadFamily>,
    /// Workloads where full trace still looks necessary.
    pub full_trace_only_workloads: Vec<TassadarWeakSupervisionWorkloadFamily>,
    /// Workloads where io-only remains fragile.
    pub io_only_fragile_workloads: Vec<TassadarWeakSupervisionWorkloadFamily>,
    /// Search-heavy workloads that still need structure-aware supervision.
    pub search_family_needing_structure: Vec<TassadarWeakSupervisionWorkloadFamily>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarWeakSupervisionExecutorSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarWeakSupervisionExecutorReportError),
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

/// Builds the committed weak-supervision summary.
pub fn build_tassadar_weak_supervision_executor_summary_report(
) -> Result<TassadarWeakSupervisionExecutorSummaryReport, TassadarWeakSupervisionExecutorSummaryError>
{
    let eval_report = build_tassadar_weak_supervision_executor_report()?;
    let mixed_viable_workloads = eval_report.mixed_viable_workloads.clone();
    let io_only_fragile_workloads = eval_report.io_only_fragile_workloads.clone();
    let full_trace_only_workloads = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| summary.mixed_gap_vs_full_trace_bps < -1_000)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let search_family_needing_structure = eval_report
        .workload_summaries
        .iter()
        .filter(|summary| {
            summary.workload_family == TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel
                && (summary.mixed_gap_vs_full_trace_bps < -500
                    || io_only_fragile_workloads.contains(&summary.workload_family))
        })
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let mut report = TassadarWeakSupervisionExecutorSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.weak_supervision.summary.v1"),
        eval_report,
        mixed_viable_workloads,
        full_trace_only_workloads,
        io_only_fragile_workloads,
        search_family_needing_structure,
        claim_boundary: String::from(
            "this summary is a research interpretation over the committed weak-supervision report. It keeps mixed-viable, full-trace-only, and io-only-fragile workloads explicit instead of promoting weaker supervision into broad learned closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Weak-supervision summary now marks {} mixed-viable workloads, {} full-trace-only workloads, {} io-only fragile workloads, and {} search-family workloads still needing structure-aware supervision.",
        report.mixed_viable_workloads.len(),
        report.full_trace_only_workloads.len(),
        report.io_only_fragile_workloads.len(),
        report.search_family_needing_structure.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_weak_supervision_executor_summary_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed summary report.
#[must_use]
pub fn tassadar_weak_supervision_executor_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF)
}

/// Writes the committed weak-supervision summary.
pub fn write_tassadar_weak_supervision_executor_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWeakSupervisionExecutorSummaryReport, TassadarWeakSupervisionExecutorSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWeakSupervisionExecutorSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_weak_supervision_executor_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWeakSupervisionExecutorSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
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
) -> Result<T, TassadarWeakSupervisionExecutorSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarWeakSupervisionExecutorSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWeakSupervisionExecutorSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_weak_supervision_executor_summary_report, read_repo_json,
        tassadar_weak_supervision_executor_summary_report_path,
        write_tassadar_weak_supervision_executor_summary_report,
        TassadarWeakSupervisionExecutorSummaryReport,
    };
    use psionic_data::{
        TassadarWeakSupervisionWorkloadFamily, TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF,
    };

    #[test]
    fn weak_supervision_summary_marks_mixed_viable_and_full_trace_only_workloads() {
        let report = build_tassadar_weak_supervision_executor_summary_report()
            .expect("weak-supervision summary");

        assert!(report
            .mixed_viable_workloads
            .contains(&TassadarWeakSupervisionWorkloadFamily::HungarianModule));
        assert!(report
            .full_trace_only_workloads
            .contains(&TassadarWeakSupervisionWorkloadFamily::ModuleStateControl));
        assert!(report
            .io_only_fragile_workloads
            .contains(&TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel));
        assert!(report
            .search_family_needing_structure
            .contains(&TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel));
    }

    #[test]
    fn weak_supervision_summary_matches_committed_truth() {
        let generated = build_tassadar_weak_supervision_executor_summary_report()
            .expect("weak-supervision summary");
        let committed: TassadarWeakSupervisionExecutorSummaryReport =
            read_repo_json(TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF)
                .expect("committed weak-supervision summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_weak_supervision_summary_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_weak_supervision_executor_summary.json");
        let report = write_tassadar_weak_supervision_executor_summary_report(&output_path)
            .expect("write weak-supervision summary");
        let written = std::fs::read_to_string(&output_path).expect("written summary");
        let reparsed: TassadarWeakSupervisionExecutorSummaryReport =
            serde_json::from_str(&written).expect("written summary should parse");

        assert_eq!(report, reparsed);
        assert_eq!(
            tassadar_weak_supervision_executor_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_weak_supervision_executor_summary.json")
        );
    }
}
