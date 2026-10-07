use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_approximate_attention_closure_matrix_report,
    TassadarApproximateAttentionClosureMatrixError,
    TassadarApproximateAttentionClosureMatrixReport,
};
use psionic_models::TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_SUMMARY_REPORT_REF;
use psionic_runtime::TassadarApproximateAttentionFamily;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Research summary over the approximate-attention closure matrix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarApproximateAttentionClosureSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Eval matrix consumed by the summary.
    pub matrix_report: TassadarApproximateAttentionClosureMatrixReport,
    /// Families with no degraded or refused workload rows on the current matrix.
    pub fully_direct_families: Vec<TassadarApproximateAttentionFamily>,
    /// Families that remain bounded to research analysis because they degrade or refuse somewhere.
    pub bounded_research_families: Vec<TassadarApproximateAttentionFamily>,
    /// Workloads refused by at least two non-dense families.
    pub refusal_hotspot_workloads: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarApproximateAttentionClosureSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarApproximateAttentionClosureMatrixError),
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

/// Builds the committed approximate-attention closure summary.
pub fn build_tassadar_approximate_attention_closure_summary_report(
) -> Result<
    TassadarApproximateAttentionClosureSummaryReport,
    TassadarApproximateAttentionClosureSummaryError,
> {
    let matrix_report = build_tassadar_approximate_attention_closure_matrix_report()?;
    let fully_direct_families = matrix_report
        .family_summaries
        .iter()
        .filter(|summary| summary.degraded_workload_count == 0 && summary.refused_workload_count == 0)
        .map(|summary| summary.attention_family)
        .collect::<Vec<_>>();
    let bounded_research_families = matrix_report
        .family_summaries
        .iter()
        .filter(|summary| summary.degraded_workload_count > 0 || summary.refused_workload_count > 0)
        .map(|summary| summary.attention_family)
        .collect::<Vec<_>>();
    let refusal_hotspot_workloads = matrix_report
        .workload_summaries
        .iter()
        .filter(|summary| summary.refused_families.len() >= 2)
        .map(|summary| summary.workload_target.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarApproximateAttentionClosureSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.approximate_attention_closure.summary.v1"),
        matrix_report,
        fully_direct_families,
        bounded_research_families,
        refusal_hotspot_workloads,
        claim_boundary: String::from(
            "this summary is a research-only interpretation layer over the committed closure matrix. It keeps fully-direct families, bounded research families, and refusal hotspots explicit instead of turning one approximate-attention win into a general executor claim",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Approximate-attention closure summary now marks {} fully-direct families, {} bounded research families, and {} refusal-hotspot workloads.",
        report.fully_direct_families.len(),
        report.bounded_research_families.len(),
        report.refusal_hotspot_workloads.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_approximate_attention_closure_summary_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed summary report.
#[must_use]
pub fn tassadar_approximate_attention_closure_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_SUMMARY_REPORT_REF)
}

/// Writes the committed approximate-attention closure summary.
pub fn write_tassadar_approximate_attention_closure_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarApproximateAttentionClosureSummaryReport,
    TassadarApproximateAttentionClosureSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarApproximateAttentionClosureSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_approximate_attention_closure_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarApproximateAttentionClosureSummaryError::Write {
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
) -> Result<T, TassadarApproximateAttentionClosureSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarApproximateAttentionClosureSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarApproximateAttentionClosureSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_approximate_attention_closure_summary_report, read_repo_json,
        tassadar_approximate_attention_closure_summary_report_path,
        write_tassadar_approximate_attention_closure_summary_report,
        TassadarApproximateAttentionClosureSummaryReport,
    };
    use psionic_models::TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_SUMMARY_REPORT_REF;
    use psionic_runtime::TassadarApproximateAttentionFamily;

    #[test]
    fn approximate_attention_closure_summary_marks_promotion_boundaries() {
        let report =
            build_tassadar_approximate_attention_closure_summary_report().expect("summary");

        assert!(report
            .fully_direct_families
            .contains(&TassadarApproximateAttentionFamily::DenseReferenceLinear));
        assert!(report
            .bounded_research_families
            .contains(&TassadarApproximateAttentionFamily::HardMaxRoutingProxy));
        assert!(report
            .refusal_hotspot_workloads
            .contains(&String::from("long_loop_kernel")));
    }

    #[test]
    fn approximate_attention_closure_summary_matches_committed_truth() {
        let generated =
            build_tassadar_approximate_attention_closure_summary_report().expect("summary");
        let committed: TassadarApproximateAttentionClosureSummaryReport =
            read_repo_json(TASSADAR_APPROXIMATE_ATTENTION_CLOSURE_SUMMARY_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_approximate_attention_closure_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_approximate_attention_closure_summary.json");
        let written = write_tassadar_approximate_attention_closure_summary_report(&output_path)
            .expect("write");
        let persisted: TassadarApproximateAttentionClosureSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_approximate_attention_closure_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_approximate_attention_closure_summary.json")
        );
    }
}
