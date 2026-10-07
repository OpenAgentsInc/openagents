use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_tassadar_precision_attention_robustness_audit_report,
    TassadarPrecisionAttentionRobustnessAuditError,
    TassadarPrecisionAttentionRobustnessAuditReport,
};
use psionic_models::{
    TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_SUMMARY_REPORT_REF,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Research-facing summary over the numeric and attention robustness audit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionRobustnessSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Underlying eval-facing robustness audit report.
    pub audit_report: TassadarPrecisionAttentionRobustnessAuditReport,
    /// Workload families that stay exact under at most three regimes.
    pub fragile_workload_family_ids: Vec<String>,
    /// Workload families that refuse in at least half the audited regimes.
    pub refusal_hotspot_workload_family_ids: Vec<String>,
    /// Workload families with no refused regimes.
    pub stable_workload_family_ids: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Summary sentence.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarPrecisionAttentionRobustnessSummaryReport {
    fn new(audit_report: TassadarPrecisionAttentionRobustnessAuditReport) -> Self {
        let total_regimes = audit_report.regime_summaries.len() as u32;
        let fragile_workload_family_ids = audit_report
            .workload_summaries
            .iter()
            .filter(|summary| summary.exact_regime_count <= 3)
            .map(|summary| summary.workload_family_id.clone())
            .collect::<Vec<_>>();
        let refusal_hotspot_workload_family_ids = audit_report
            .workload_summaries
            .iter()
            .filter(|summary| summary.refused_regime_count * 2 >= total_regimes)
            .map(|summary| summary.workload_family_id.clone())
            .collect::<Vec<_>>();
        let stable_workload_family_ids = audit_report
            .workload_summaries
            .iter()
            .filter(|summary| summary.refused_regime_count == 0)
            .map(|summary| summary.workload_family_id.clone())
            .collect::<Vec<_>>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.precision_attention_robustness.summary.v1"),
            audit_report,
            fragile_workload_family_ids,
            refusal_hotspot_workload_family_ids,
            stable_workload_family_ids,
            claim_boundary: String::from(
                "this summary keeps finite-precision and attention-semantics robustness as a research-only execution-truth surface. Fragile or refusal-heavy workloads remain explicit, and approximate_bounded outcomes do not widen served capability or erase refusal requirements",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Precision/attention robustness summary now marks {} fragile workloads, {} refusal hotspots, and {} stable workloads across the current audited regimes.",
            report.fragile_workload_family_ids.len(),
            report.refusal_hotspot_workload_family_ids.len(),
            report.stable_workload_family_ids.len(),
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_precision_attention_robustness_summary_report|",
            &report,
        );
        report
    }
}

/// Robustness-summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarPrecisionAttentionRobustnessSummaryError {
    /// Building the eval report failed.
    #[error(transparent)]
    Eval(#[from] TassadarPrecisionAttentionRobustnessAuditError),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed robustness summary.
pub fn build_tassadar_precision_attention_robustness_summary_report() -> Result<
    TassadarPrecisionAttentionRobustnessSummaryReport,
    TassadarPrecisionAttentionRobustnessSummaryError,
> {
    let audit_report = build_tassadar_precision_attention_robustness_audit_report()?;
    Ok(TassadarPrecisionAttentionRobustnessSummaryReport::new(
        audit_report,
    ))
}

/// Returns the canonical absolute path for the committed robustness summary.
#[must_use]
pub fn tassadar_precision_attention_robustness_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_SUMMARY_REPORT_REF)
}

/// Writes the committed robustness summary.
pub fn write_tassadar_precision_attention_robustness_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPrecisionAttentionRobustnessSummaryReport,
    TassadarPrecisionAttentionRobustnessSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPrecisionAttentionRobustnessSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_precision_attention_robustness_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPrecisionAttentionRobustnessSummaryError::Write {
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

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPrecisionAttentionRobustnessSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPrecisionAttentionRobustnessSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPrecisionAttentionRobustnessSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_precision_attention_robustness_summary_report, read_repo_json,
        tassadar_precision_attention_robustness_summary_report_path,
        write_tassadar_precision_attention_robustness_summary_report,
        TassadarPrecisionAttentionRobustnessSummaryReport,
    };
    use psionic_models::TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_SUMMARY_REPORT_REF;

    #[test]
    fn precision_attention_robustness_summary_keeps_fragile_and_refusal_hotspots_explicit() {
        let report = build_tassadar_precision_attention_robustness_summary_report()
            .expect("summary report");

        assert!(report
            .fragile_workload_family_ids
            .contains(&String::from("sudoku_class")));
        assert!(report
            .refusal_hotspot_workload_family_ids
            .contains(&String::from("sudoku_class")));
        assert!(report
            .stable_workload_family_ids
            .contains(&String::from("micro_wasm_kernel")));
    }

    #[test]
    fn precision_attention_robustness_summary_matches_committed_truth() {
        let generated = build_tassadar_precision_attention_robustness_summary_report()
            .expect("summary report");
        let committed: TassadarPrecisionAttentionRobustnessSummaryReport =
            read_repo_json(TASSADAR_PRECISION_ATTENTION_ROBUSTNESS_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_precision_attention_robustness_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_precision_attention_robustness_summary.json");
        let written = write_tassadar_precision_attention_robustness_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarPrecisionAttentionRobustnessSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_precision_attention_robustness_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_precision_attention_robustness_summary.json")
        );
    }
}
