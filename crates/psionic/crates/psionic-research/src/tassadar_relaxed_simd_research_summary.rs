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
    TassadarRelaxedSimdResearchLadderReport, TassadarRelaxedSimdResearchLadderReportError,
    build_tassadar_relaxed_simd_research_ladder_report,
};

pub const TASSADAR_RELAXED_SIMD_RESEARCH_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_relaxed_simd_research_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRelaxedSimdResearchSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub ladder_report: TassadarRelaxedSimdResearchLadderReport,
    pub exact_anchor_backend_ids: Vec<String>,
    pub approximate_backend_ids: Vec<String>,
    pub refused_backend_ids: Vec<String>,
    pub non_promotion_gate_reason_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarRelaxedSimdResearchSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarRelaxedSimdResearchLadderReportError),
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

pub fn build_tassadar_relaxed_simd_research_summary()
-> Result<TassadarRelaxedSimdResearchSummary, TassadarRelaxedSimdResearchSummaryError> {
    let ladder_report = build_tassadar_relaxed_simd_research_ladder_report()?;
    let mut report = TassadarRelaxedSimdResearchSummary {
        schema_version: 1,
        report_id: String::from("tassadar.relaxed_simd.research_summary.v1"),
        exact_anchor_backend_ids: ladder_report.exact_anchor_backend_ids.clone(),
        approximate_backend_ids: ladder_report.approximate_backend_ids.clone(),
        refused_backend_ids: ladder_report.refused_backend_ids.clone(),
        non_promotion_gate_reason_ids: ladder_report.non_promotion_gate_reason_ids.clone(),
        ladder_report,
        claim_boundary: String::from(
            "this summary keeps relaxed-SIMD and accelerator semantics as a research-only ladder. Bounded drift and explicit refusal stay visible, and the deterministic SIMD public profile remains separate from every relaxed-SIMD candidate",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Relaxed-SIMD research summary keeps exact_anchor_backends={}, approximate_backends={}, refused_backends={}, non_promotion_gates={}.",
        report.exact_anchor_backend_ids.len(),
        report.approximate_backend_ids.len(),
        report.refused_backend_ids.len(),
        report.non_promotion_gate_reason_ids.len(),
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_relaxed_simd_research_summary|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_relaxed_simd_research_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_RELAXED_SIMD_RESEARCH_SUMMARY_REF)
}

pub fn write_tassadar_relaxed_simd_research_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRelaxedSimdResearchSummary, TassadarRelaxedSimdResearchSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRelaxedSimdResearchSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_relaxed_simd_research_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRelaxedSimdResearchSummaryError::Write {
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
        .expect("repo root")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarRelaxedSimdResearchSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarRelaxedSimdResearchSummaryError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRelaxedSimdResearchSummaryError::Deserialize {
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
        TASSADAR_RELAXED_SIMD_RESEARCH_SUMMARY_REF, TassadarRelaxedSimdResearchSummary,
        build_tassadar_relaxed_simd_research_summary, read_repo_json,
    };

    #[test]
    fn relaxed_simd_research_summary_keeps_bounded_drift_and_refusal_visible() {
        let report = build_tassadar_relaxed_simd_research_summary().expect("summary");

        assert_eq!(report.exact_anchor_backend_ids.len(), 1);
        assert_eq!(report.approximate_backend_ids.len(), 2);
        assert_eq!(report.refused_backend_ids.len(), 2);
    }

    #[test]
    fn relaxed_simd_research_summary_matches_committed_truth() {
        let generated = build_tassadar_relaxed_simd_research_summary().expect("summary");
        let committed: TassadarRelaxedSimdResearchSummary =
            read_repo_json(TASSADAR_RELAXED_SIMD_RESEARCH_SUMMARY_REF).expect("committed summary");
        assert_eq!(generated, committed);
    }
}
