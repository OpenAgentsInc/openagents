use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleFastRouteArchitectureSelectionReport,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_architecture_selection_summary.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteArchitectureSelectionSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFastRouteArchitectureSelectionReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub selected_candidate_kind: String,
    pub routeable_candidate_count: usize,
    pub exact_article_matrix_candidate_count: usize,
    pub selected_direct_module_class_count: usize,
    pub selected_fallback_module_class_count: usize,
    pub fast_route_selection_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFastRouteArchitectureSelectionSummary {
    fn new(report: TassadarArticleFastRouteArchitectureSelectionReport) -> Self {
        let selected_routeability = report
            .routeability_checks
            .iter()
            .find(|row| row.candidate_kind == report.selected_candidate_kind)
            .expect("selected candidate routeability");
        let routeable_candidate_count = report
            .candidate_verdicts
            .iter()
            .filter(|row| row.routeability_green)
            .count();
        let exact_article_matrix_candidate_count = report
            .candidate_verdicts
            .iter()
            .filter(|row| row.article_scale_exactness_green)
            .count();
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_fast_route_architecture_selection.summary.v1"),
            report_ref: String::from(
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            ),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            selected_candidate_kind: report.selected_candidate_kind.label().to_string(),
            routeable_candidate_count,
            exact_article_matrix_candidate_count,
            selected_direct_module_class_count: selected_routeability.direct_module_class_count,
            selected_fallback_module_class_count: selected_routeability.fallback_module_class_count,
            fast_route_selection_green: report.fast_route_selection_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-172 fast-route architecture selection. It keeps the chosen fast family, candidate counts, and projected routeability split operator-readable without pretending that TAS-173 through TAS-175 are already closed.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article fast-route selection summary now records selected_candidate_kind=`{}`, routeable_candidate_count={}, exact_article_matrix_candidate_count={}, selected_direct_module_class_count={}, selected_fallback_module_class_count={}, fast_route_selection_green={}, and article_equivalence_green={}.",
            summary.selected_candidate_kind,
            summary.routeable_candidate_count,
            summary.exact_article_matrix_candidate_count,
            summary.selected_direct_module_class_count,
            summary.selected_fallback_module_class_count,
            summary.fast_route_selection_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_fast_route_architecture_selection_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteArchitectureSelectionSummaryError {
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

pub fn build_tassadar_article_fast_route_architecture_selection_summary() -> Result<
    TassadarArticleFastRouteArchitectureSelectionSummary,
    TassadarArticleFastRouteArchitectureSelectionSummaryError,
> {
    let report: TassadarArticleFastRouteArchitectureSelectionReport = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        "article_fast_route_architecture_selection_report",
    )?;
    Ok(TassadarArticleFastRouteArchitectureSelectionSummary::new(
        report,
    ))
}

pub fn tassadar_article_fast_route_architecture_selection_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_architecture_selection_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFastRouteArchitectureSelectionSummary,
    TassadarArticleFastRouteArchitectureSelectionSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteArchitectureSelectionSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_fast_route_architecture_selection_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteArchitectureSelectionSummaryError::Write {
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
) -> Result<T, TassadarArticleFastRouteArchitectureSelectionSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteArchitectureSelectionSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteArchitectureSelectionSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fast_route_architecture_selection_summary, read_repo_json,
        tassadar_article_fast_route_architecture_selection_summary_path,
        write_tassadar_article_fast_route_architecture_selection_summary,
        TassadarArticleFastRouteArchitectureSelectionSummary,
        TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_SUMMARY_REPORT_REF,
    };

    #[test]
    fn fast_route_selection_summary_tracks_hull_cache_choice_without_final_green() {
        let summary =
            build_tassadar_article_fast_route_architecture_selection_summary().expect("summary");

        assert_eq!(summary.selected_candidate_kind, "hull_cache_runtime");
        assert_eq!(summary.routeable_candidate_count, 1);
        assert_eq!(summary.exact_article_matrix_candidate_count, 3);
        assert_eq!(summary.selected_direct_module_class_count, 6);
        assert_eq!(summary.selected_fallback_module_class_count, 0);
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.acceptance_status, "green");
        assert!(summary.fast_route_selection_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn fast_route_selection_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_fast_route_architecture_selection_summary().expect("summary");
        let committed: TassadarArticleFastRouteArchitectureSelectionSummary = read_repo_json(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_SUMMARY_REPORT_REF,
            "article_fast_route_architecture_selection_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_selection_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_architecture_selection_summary.json");
        let written =
            write_tassadar_article_fast_route_architecture_selection_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarArticleFastRouteArchitectureSelectionSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_architecture_selection_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_architecture_selection_summary.json")
        );
    }
}
