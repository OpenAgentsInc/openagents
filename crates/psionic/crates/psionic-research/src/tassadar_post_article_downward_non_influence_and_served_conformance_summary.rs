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
    build_tassadar_post_article_downward_non_influence_and_served_conformance_report,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError,
    TassadarPostArticleDownwardNonInfluenceStatus,
};

pub const TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_SUMMARY_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_downward_non_influence_and_served_conformance_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub contract_status: TassadarPostArticleDownwardNonInfluenceStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub lower_plane_truth_row_count: u32,
    pub served_deviation_row_count: u32,
    pub validation_row_count: u32,
    pub downward_non_influence_complete: bool,
    pub served_conformance_envelope_complete: bool,
    pub lower_plane_truth_rewrite_refused: bool,
    pub served_posture_narrower_than_operator_truth: bool,
    pub served_posture_fail_closed: bool,
    pub plugin_or_served_overclaim_refused: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError),
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

pub fn build_tassadar_post_article_downward_non_influence_and_served_conformance_summary(
) -> Result<
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError,
> {
    let report = build_tassadar_post_article_downward_non_influence_and_served_conformance_report(
    )?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
) -> TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary {
    let mut summary = TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        current_served_internal_compute_profile_id: report
            .machine_identity_binding
            .current_served_internal_compute_profile_id
            .clone(),
        contract_status: report.contract_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        lower_plane_truth_row_count: report.lower_plane_truth_rows.len() as u32,
        served_deviation_row_count: report.served_deviation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        downward_non_influence_complete: report.downward_non_influence_complete,
        served_conformance_envelope_complete: report.served_conformance_envelope_complete,
        lower_plane_truth_rewrite_refused: report.lower_plane_truth_rewrite_refused,
        served_posture_narrower_than_operator_truth: report
            .served_posture_narrower_than_operator_truth,
        served_posture_fail_closed: report.served_posture_fail_closed,
        plugin_or_served_overclaim_refused: report.plugin_or_served_overclaim_refused,
        next_stability_issue_id: report.next_stability_issue_id.clone(),
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        detail: format!(
            "post-article downward non-influence summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, lower_plane_truth_rows={}, served_deviation_rows={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.contract_status,
            report.lower_plane_truth_rows.len(),
            report.served_deviation_rows.len(),
            report.next_stability_issue_id,
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_downward_non_influence_and_served_conformance_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_downward_non_influence_and_served_conformance_summary_path(
) -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_SUMMARY_REF)
}

pub fn write_tassadar_post_article_downward_non_influence_and_served_conformance_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_downward_non_influence_and_served_conformance_summary(
    )?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError::Write {
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
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_downward_non_influence_and_served_conformance_summary,
        read_json,
        tassadar_post_article_downward_non_influence_and_served_conformance_summary_path,
        write_tassadar_post_article_downward_non_influence_and_served_conformance_summary,
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary,
        TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_SUMMARY_REF,
    };

    #[test]
    fn downward_non_influence_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_summary()
                .expect("summary");

        assert_eq!(
            summary.report_id,
            "tassadar.post_article_downward_non_influence_and_served_conformance.report.v1"
        );
        assert!(summary.downward_non_influence_complete);
        assert!(summary.served_conformance_envelope_complete);
        assert!(summary.lower_plane_truth_rewrite_refused);
        assert!(summary.served_posture_narrower_than_operator_truth);
        assert!(summary.served_posture_fail_closed);
        assert!(summary.plugin_or_served_overclaim_refused);
        assert_eq!(summary.lower_plane_truth_row_count, 6);
        assert_eq!(summary.served_deviation_row_count, 3);
        assert_eq!(summary.next_stability_issue_id, "TAS-215");
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn downward_non_influence_summary_matches_committed_truth() {
        let expected =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_summary()
                .expect("expected");
        let committed: TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary =
            read_json(
                tassadar_post_article_downward_non_influence_and_served_conformance_summary_path(),
            )
            .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_downward_non_influence_and_served_conformance_summary_path()
                .ends_with(
                    "tassadar_post_article_downward_non_influence_and_served_conformance_summary.json"
                )
        );
    }

    #[test]
    fn write_downward_non_influence_summary_persists_truth() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output_path = dir
            .path()
            .join("tassadar_post_article_downward_non_influence_and_served_conformance_summary.json");
        let written =
            write_tassadar_post_article_downward_non_influence_and_served_conformance_summary(
                &output_path,
            )
            .expect("written");
        let reread: TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary =
            read_json(&output_path).expect("reread");

        assert_eq!(written, reread);
        assert_eq!(
            tassadar_post_article_downward_non_influence_and_served_conformance_summary_path()
                .strip_prefix(super::repo_root())
                .expect("relative")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_SUMMARY_REF
        );
    }
}
