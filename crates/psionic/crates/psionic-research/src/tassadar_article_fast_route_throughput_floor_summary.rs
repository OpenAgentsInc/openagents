use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleFastRouteThroughputFloorReport,
    TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_throughput_floor_summary.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleFastRouteThroughputFloorSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFastRouteThroughputFloorReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub selection_prerequisite_green: bool,
    pub exactness_prerequisite_green: bool,
    pub drift_policy_green: bool,
    pub demo_public_floor_pass_count: u32,
    pub demo_internal_floor_pass_count: u32,
    pub kernel_floor_pass_count: u32,
    pub throughput_floor_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFastRouteThroughputFloorSummary {
    fn new(report: TassadarArticleFastRouteThroughputFloorReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_fast_route_throughput_floor.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            selection_prerequisite_green: report.selection_prerequisite.fast_route_selection_green,
            exactness_prerequisite_green: report.exactness_prerequisite.exactness_green,
            drift_policy_green: report.cross_machine_drift_review.drift_policy_green,
            demo_public_floor_pass_count: report.demo_public_floor_pass_count,
            demo_internal_floor_pass_count: report.demo_internal_floor_pass_count,
            kernel_floor_pass_count: report.kernel_floor_pass_count,
            throughput_floor_green: report.throughput_floor_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-175 throughput-floor tranche. It keeps the selected fast-route throughput floor, drift policy, and prerequisite posture operator-readable without pretending that TAS-176 onward or final article-equivalence green status are already closed.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Fast-route throughput-floor summary now records tied_requirement_satisfied={}, selection_prerequisite_green={}, exactness_prerequisite_green={}, drift_policy_green={}, demo_public_floor_passes={}, demo_internal_floor_passes={}, kernel_floor_passes={}, throughput_floor_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.selection_prerequisite_green,
            summary.exactness_prerequisite_green,
            summary.drift_policy_green,
            summary.demo_public_floor_pass_count,
            summary.demo_internal_floor_pass_count,
            summary.kernel_floor_pass_count,
            summary.throughput_floor_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_fast_route_throughput_floor_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFastRouteThroughputFloorSummaryError {
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

pub fn build_tassadar_article_fast_route_throughput_floor_summary(
) -> Result<TassadarArticleFastRouteThroughputFloorSummary, TassadarArticleFastRouteThroughputFloorSummaryError>
{
    let report: TassadarArticleFastRouteThroughputFloorReport = read_repo_json(
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_REF,
        "article_fast_route_throughput_floor_report",
    )?;
    Ok(TassadarArticleFastRouteThroughputFloorSummary::new(report))
}

pub fn tassadar_article_fast_route_throughput_floor_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_fast_route_throughput_floor_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleFastRouteThroughputFloorSummary, TassadarArticleFastRouteThroughputFloorSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFastRouteThroughputFloorSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_fast_route_throughput_floor_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFastRouteThroughputFloorSummaryError::Write {
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
) -> Result<T, TassadarArticleFastRouteThroughputFloorSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFastRouteThroughputFloorSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFastRouteThroughputFloorSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_SUMMARY_REPORT_REF,
        TassadarArticleFastRouteThroughputFloorSummary,
        build_tassadar_article_fast_route_throughput_floor_summary, read_repo_json,
        tassadar_article_fast_route_throughput_floor_summary_path,
        write_tassadar_article_fast_route_throughput_floor_summary,
    };

    #[test]
    fn throughput_floor_summary_tracks_green_runtime_floor_without_final_green() {
        let summary = build_tassadar_article_fast_route_throughput_floor_summary().expect("summary");

        assert_eq!(summary.tied_requirement_id, "TAS-175");
        assert!(summary.tied_requirement_satisfied);
        assert!(summary.selection_prerequisite_green);
        assert!(summary.exactness_prerequisite_green);
        assert!(summary.drift_policy_green);
        assert_eq!(summary.demo_public_floor_pass_count, 2);
        assert_eq!(summary.demo_internal_floor_pass_count, 2);
        assert_eq!(summary.kernel_floor_pass_count, 4);
        assert!(summary.throughput_floor_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn throughput_floor_summary_matches_committed_truth() {
        let generated = build_tassadar_article_fast_route_throughput_floor_summary().expect("summary");
        let committed: TassadarArticleFastRouteThroughputFloorSummary = read_repo_json(
            TASSADAR_ARTICLE_FAST_ROUTE_THROUGHPUT_FLOOR_SUMMARY_REPORT_REF,
            "summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_throughput_floor_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fast_route_throughput_floor_summary.json");
        let written = write_tassadar_article_fast_route_throughput_floor_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleFastRouteThroughputFloorSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fast_route_throughput_floor_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_fast_route_throughput_floor_summary.json")
        );
    }
}
