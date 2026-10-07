use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleFastRouteExactnessReport, TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_exactness_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteExactnessSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFastRouteExactnessReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub implementation_prerequisite_green: bool,
    pub all_article_workloads_exact: bool,
    pub article_session_direct_case_count: usize,
    pub hybrid_direct_case_count: usize,
    pub exactness_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFastRouteExactnessSummary {
    fn new(report: TassadarArticleFastRouteExactnessReport) -> Self {
        let article_session_direct_case_count = report
            .article_session_reviews
            .iter()
            .filter(|review| review.exact_direct_hull_cache)
            .count();
        let hybrid_direct_case_count = report
            .hybrid_route_reviews
            .iter()
            .filter(|review| review.exact_direct_hull_cache)
            .count();
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_fast_route_exactness.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            implementation_prerequisite_green: report
                .implementation_prerequisite
                .fast_route_implementation_green,
            all_article_workloads_exact: report.hull_cache_closure_review.all_article_workloads_exact,
            article_session_direct_case_count,
            hybrid_direct_case_count,
            exactness_green: report.exactness_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-174 fast-route exactness tranche. It keeps the zero-fallback article-workload closure operator-readable without pretending that TAS-175 throughput closure or final article-equivalence green status are already closed.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article fast-route exactness summary now records tied_requirement_satisfied={}, implementation_prerequisite_green={}, all_article_workloads_exact={}, article_session_direct_case_count={}, hybrid_direct_case_count={}, exactness_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.implementation_prerequisite_green,
            summary.all_article_workloads_exact,
            summary.article_session_direct_case_count,
            summary.hybrid_direct_case_count,
            summary.exactness_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_fast_route_exactness_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteExactnessSummaryError {
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

pub fn build_tassadar_article_fast_route_exactness_summary(
) -> Result<TassadarArticleFastRouteExactnessSummary, TassadarArticleFastRouteExactnessSummaryError>
{
    let report: TassadarArticleFastRouteExactnessReport = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_REPORT_REF,
        "article_fast_route_exactness_report",
    )?;
    Ok(TassadarArticleFastRouteExactnessSummary::new(report))
}

pub fn tassadar_article_fast_route_exactness_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_exactness_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleFastRouteExactnessSummary, TassadarArticleFastRouteExactnessSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteExactnessSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_fast_route_exactness_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteExactnessSummaryError::Write {
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
) -> Result<T, TassadarArticleFastRouteExactnessSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarArticleFastRouteExactnessSummaryError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteExactnessSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_exactness_summary, read_repo_json,
        tassadar_article_fast_route_exactness_summary_path,
        write_tassadar_article_fast_route_exactness_summary,
        TassadarArticleFastRouteExactnessSummary,
        TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SUMMARY_REPORT_REF,
    };

    #[test]
    fn fast_route_exactness_summary_tracks_no_fallback_article_closure() {
        let summary = build_tassadar_article_fast_route_exactness_summary().expect("summary");

        assert_eq!(summary.tied_requirement_id, "TAS-174");
        assert!(summary.implementation_prerequisite_green);
        assert!(summary.all_article_workloads_exact);
        assert_eq!(summary.article_session_direct_case_count, 3);
        assert_eq!(summary.hybrid_direct_case_count, 3);
        assert!(summary.exactness_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn fast_route_exactness_summary_matches_committed_truth() {
        let generated = build_tassadar_article_fast_route_exactness_summary().expect("summary");
        let committed: TassadarArticleFastRouteExactnessSummary = read_repo_json(
            TASSADAR_ARTICLE_FAST_ROUTE_EXACTNESS_SUMMARY_REPORT_REF,
            "summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_exactness_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_exactness_summary.json");
        let written = write_tassadar_article_fast_route_exactness_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleFastRouteExactnessSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_exactness_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_exactness_summary.json")
        );
    }
}
