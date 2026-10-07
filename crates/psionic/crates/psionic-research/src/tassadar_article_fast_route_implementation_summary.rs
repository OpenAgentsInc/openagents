use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleFastRouteImplementationReport,
    TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_implementation_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteImplementationSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFastRouteImplementationReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub selected_candidate_kind: String,
    pub article_session_fast_path_integrated: bool,
    pub hybrid_fast_path_integrated: bool,
    pub direct_proof_descriptor_binding_green: bool,
    pub fast_route_implementation_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFastRouteImplementationSummary {
    fn new(report: TassadarArticleFastRouteImplementationReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_fast_route_implementation.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            selected_candidate_kind: report.selected_candidate_kind.clone(),
            article_session_fast_path_integrated: report.article_session_review.fast_path_integrated,
            hybrid_fast_path_integrated: report.hybrid_route_review.fast_path_integrated,
            direct_proof_descriptor_binding_green: report
                .direct_proof_review
                .descriptor_binding_green,
            fast_route_implementation_green: report.fast_route_implementation_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-173 fast-route implementation tranche. It keeps the canonical HullCache ownership, live session routing, and direct-proof descriptor binding operator-readable without pretending that TAS-174 or TAS-175 are already closed.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article fast-route implementation summary now records selected_candidate_kind=`{}`, tied_requirement_satisfied={}, article_session_fast_path_integrated={}, hybrid_fast_path_integrated={}, direct_proof_descriptor_binding_green={}, fast_route_implementation_green={}, and article_equivalence_green={}.",
            summary.selected_candidate_kind,
            summary.tied_requirement_satisfied,
            summary.article_session_fast_path_integrated,
            summary.hybrid_fast_path_integrated,
            summary.direct_proof_descriptor_binding_green,
            summary.fast_route_implementation_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_fast_route_implementation_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteImplementationSummaryError {
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

pub fn build_tassadar_article_fast_route_implementation_summary() -> Result<
    TassadarArticleFastRouteImplementationSummary,
    TassadarArticleFastRouteImplementationSummaryError,
> {
    let report: TassadarArticleFastRouteImplementationReport = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
        "article_fast_route_implementation_report",
    )?;
    Ok(TassadarArticleFastRouteImplementationSummary::new(report))
}

pub fn tassadar_article_fast_route_implementation_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_implementation_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFastRouteImplementationSummary,
    TassadarArticleFastRouteImplementationSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteImplementationSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_fast_route_implementation_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteImplementationSummaryError::Write {
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
) -> Result<T, TassadarArticleFastRouteImplementationSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteImplementationSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteImplementationSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_implementation_summary, read_repo_json,
        tassadar_article_fast_route_implementation_summary_path,
        write_tassadar_article_fast_route_implementation_summary,
        TassadarArticleFastRouteImplementationSummary,
        TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_SUMMARY_REPORT_REF,
    };

    #[test]
    fn fast_route_implementation_summary_tracks_closed_integration_tranche() {
        let summary =
            build_tassadar_article_fast_route_implementation_summary().expect("summary");

        assert_eq!(summary.tied_requirement_id, "TAS-173");
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.selected_candidate_kind, "hull_cache_runtime");
        assert!(summary.article_session_fast_path_integrated);
        assert!(summary.hybrid_fast_path_integrated);
        assert!(summary.direct_proof_descriptor_binding_green);
        assert!(summary.fast_route_implementation_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn fast_route_implementation_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_fast_route_implementation_summary().expect("summary");
        let committed: TassadarArticleFastRouteImplementationSummary =
            read_repo_json(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_SUMMARY_REPORT_REF, "summary")
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_implementation_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_implementation_summary.json");
        let written = write_tassadar_article_fast_route_implementation_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleFastRouteImplementationSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_implementation_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_implementation_summary.json")
        );
    }
}
