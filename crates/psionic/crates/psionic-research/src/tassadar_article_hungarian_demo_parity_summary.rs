use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleHungarianDemoParityReport, TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hungarian_demo_parity_summary.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleHungarianDemoParitySummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleHungarianDemoParityReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: Option<String>,
    pub session_fast_route_green: bool,
    pub hybrid_fast_route_green: bool,
    pub throughput_floor_green: bool,
    pub no_tool_proof_green: bool,
    pub binding_green: bool,
    pub hungarian_demo_parity_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleHungarianDemoParitySummary {
    fn new(report: TassadarArticleHungarianDemoParityReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_hungarian_demo_parity.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            blocked_issue_frontier: report.acceptance_gate_tie.blocked_issue_ids.first().cloned(),
            session_fast_route_green: report.fast_route_session_review.fast_route_direct_green,
            hybrid_fast_route_green: report.fast_route_hybrid_review.fast_route_direct_green,
            throughput_floor_green: report.throughput_review.declared_floor_green,
            no_tool_proof_green: report.no_tool_proof_review.no_tool_proof_green,
            binding_green: report.binding_review.binding_green,
            hungarian_demo_parity_green: report.hungarian_demo_parity_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-180 Hungarian article demo parity tranche. It keeps the 10x10 Hungarian fast-route, throughput-floor, and no-tool-proof binding operator-readable without pretending that the hard-Sudoku benchmark or final article-equivalence tranches are already green.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Hungarian article demo parity summary now records tied_requirement_satisfied={}, session_fast_route_green={}, hybrid_fast_route_green={}, throughput_floor_green={}, no_tool_proof_green={}, binding_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.session_fast_route_green,
            summary.hybrid_fast_route_green,
            summary.throughput_floor_green,
            summary.no_tool_proof_green,
            summary.binding_green,
            summary.blocked_issue_frontier,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_hungarian_demo_parity_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleHungarianDemoParitySummaryError {
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

pub fn build_tassadar_article_hungarian_demo_parity_summary(
) -> Result<TassadarArticleHungarianDemoParitySummary, TassadarArticleHungarianDemoParitySummaryError>
{
    let report: TassadarArticleHungarianDemoParityReport = read_repo_json(
        TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_REPORT_REF,
        "article_hungarian_demo_parity_report",
    )?;
    Ok(TassadarArticleHungarianDemoParitySummary::new(report))
}

pub fn tassadar_article_hungarian_demo_parity_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_hungarian_demo_parity_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleHungarianDemoParitySummary, TassadarArticleHungarianDemoParitySummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleHungarianDemoParitySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_hungarian_demo_parity_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleHungarianDemoParitySummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleHungarianDemoParitySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleHungarianDemoParitySummaryError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleHungarianDemoParitySummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_hungarian_demo_parity_summary, read_repo_json,
        tassadar_article_hungarian_demo_parity_summary_path,
        write_tassadar_article_hungarian_demo_parity_summary,
        TassadarArticleHungarianDemoParitySummary,
        TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_hungarian_demo_parity_summary_tracks_green_hungarian_demo() {
        let summary = build_tassadar_article_hungarian_demo_parity_summary().expect("summary");

        assert!(summary.tied_requirement_satisfied);
        assert!(summary.session_fast_route_green);
        assert!(summary.hybrid_fast_route_green);
        assert!(summary.throughput_floor_green);
        assert!(summary.no_tool_proof_green);
        assert!(summary.binding_green);
        assert!(summary.hungarian_demo_parity_green);
        assert_eq!(summary.blocked_issue_frontier, None);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_hungarian_demo_parity_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_hungarian_demo_parity_summary().expect("summary");
        let committed: TassadarArticleHungarianDemoParitySummary = read_repo_json(
            TASSADAR_ARTICLE_HUNGARIAN_DEMO_PARITY_SUMMARY_REPORT_REF,
            "article_hungarian_demo_parity_summary",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_hungarian_demo_parity_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_hungarian_demo_parity_summary.json");
        let written = write_tassadar_article_hungarian_demo_parity_summary(&output_path)?;
        let persisted: TassadarArticleHungarianDemoParitySummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_hungarian_demo_parity_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_hungarian_demo_parity_summary.json")
        );
        Ok(())
    }
}
