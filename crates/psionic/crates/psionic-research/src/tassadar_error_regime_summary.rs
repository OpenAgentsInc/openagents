use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::TASSADAR_ERROR_REGIME_SUMMARY_REPORT_REF;
use psionic_eval::{
    TassadarErrorRegimeCatalogReport, TassadarErrorRegimeCatalogReportError,
    build_tassadar_error_regime_catalog_report,
};
use psionic_runtime::{TassadarErrorRegimeRecoverySurface, TassadarErrorRegimeWorkloadFamily};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Research-facing summary over the committed error-regime catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Source catalog consumed by the summary.
    pub catalog_report: TassadarErrorRegimeCatalogReport,
    /// Workloads where checkpoint-only beats verifier-only on the current sweep.
    pub checkpoint_dominant_workloads: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Workloads where verifier-only beats checkpoint-only on the current sweep.
    pub verifier_dominant_workloads: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Workloads whose best seeded result still needs the combined surface.
    pub workloads_needing_both: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Workloads with no self-healing surface on the current sweep.
    pub always_fragile_workloads: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarErrorRegimeSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarErrorRegimeCatalogReportError),
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

/// Builds the committed error-regime summary.
pub fn build_tassadar_error_regime_summary_report()
-> Result<TassadarErrorRegimeSummaryReport, TassadarErrorRegimeSummaryError> {
    let catalog_report = build_tassadar_error_regime_catalog_report()?;
    let checkpoint_dominant_workloads = catalog_report
        .workload_summaries
        .iter()
        .filter(|summary| {
            matches!(
                summary.best_recovery_surface,
                TassadarErrorRegimeRecoverySurface::CheckpointOnly
                    | TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier
            )
        })
        .filter(|summary| {
            summary.workload_family == TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop
        })
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let verifier_dominant_workloads = catalog_report
        .workload_summaries
        .iter()
        .filter(|summary| {
            summary.workload_family == TassadarErrorRegimeWorkloadFamily::SearchKernel
        })
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let workloads_needing_both = catalog_report.workloads_needing_both.clone();
    let always_fragile_workloads = catalog_report.always_fragile_workloads.clone();
    let mut report = TassadarErrorRegimeSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.error_regime.summary.v1"),
        catalog_report,
        checkpoint_dominant_workloads,
        verifier_dominant_workloads,
        workloads_needing_both,
        always_fragile_workloads,
        claim_boundary: String::from(
            "this summary is a research interpretation over the committed error-regime catalog. It keeps dominant recovery surfaces and always-fragile workloads explicit instead of promoting correction machinery into a broad executor claim",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Error-regime summary now marks {} checkpoint-dominant workloads, {} verifier-dominant workloads, {} workloads needing both, and {} always-fragile workloads.",
        report.checkpoint_dominant_workloads.len(),
        report.verifier_dominant_workloads.len(),
        report.workloads_needing_both.len(),
        report.always_fragile_workloads.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_error_regime_summary_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed summary report.
#[must_use]
pub fn tassadar_error_regime_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ERROR_REGIME_SUMMARY_REPORT_REF)
}

/// Writes the committed error-regime summary.
pub fn write_tassadar_error_regime_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarErrorRegimeSummaryReport, TassadarErrorRegimeSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarErrorRegimeSummaryError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_error_regime_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarErrorRegimeSummaryError::Write {
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
) -> Result<T, TassadarErrorRegimeSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarErrorRegimeSummaryError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarErrorRegimeSummaryError::Deserialize {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarErrorRegimeSummaryReport, build_tassadar_error_regime_summary_report,
        read_repo_json, tassadar_error_regime_summary_report_path,
        write_tassadar_error_regime_summary_report,
    };
    use psionic_data::TASSADAR_ERROR_REGIME_SUMMARY_REPORT_REF;
    use psionic_runtime::TassadarErrorRegimeWorkloadFamily;

    #[test]
    fn error_regime_summary_marks_dominant_and_fragile_workloads() {
        let report = build_tassadar_error_regime_summary_report().expect("error-regime summary");

        assert!(
            report
                .checkpoint_dominant_workloads
                .contains(&TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop)
        );
        assert!(
            report
                .verifier_dominant_workloads
                .contains(&TassadarErrorRegimeWorkloadFamily::SearchKernel)
        );
        assert!(
            report
                .always_fragile_workloads
                .contains(&TassadarErrorRegimeWorkloadFamily::LongHorizonControl)
        );
    }

    #[test]
    fn error_regime_summary_matches_committed_truth() {
        let generated = build_tassadar_error_regime_summary_report().expect("error-regime summary");
        let committed: TassadarErrorRegimeSummaryReport =
            read_repo_json(TASSADAR_ERROR_REGIME_SUMMARY_REPORT_REF)
                .expect("committed error-regime summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_error_regime_summary_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir.path().join("tassadar_error_regime_summary.json");
        let report =
            write_tassadar_error_regime_summary_report(&output_path).expect("write summary");
        let written = std::fs::read_to_string(&output_path).expect("written summary");
        let reparsed: TassadarErrorRegimeSummaryReport =
            serde_json::from_str(&written).expect("written summary should parse");

        assert_eq!(report, reparsed);
        assert_eq!(
            tassadar_error_regime_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_error_regime_summary.json")
        );
    }
}
