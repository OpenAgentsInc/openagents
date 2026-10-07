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
    build_tassadar_post_article_anti_drift_stability_closeout_audit_report,
    TassadarPostArticleAntiDriftCloseoutStatus,
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
    TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError,
};

pub const TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_anti_drift_stability_closeout_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftStabilityCloseoutSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub closeout_status: TassadarPostArticleAntiDriftCloseoutStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub lock_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub all_required_surface_locks_green: bool,
    pub machine_identity_lock_complete: bool,
    pub control_and_replay_posture_locked: bool,
    pub semantics_and_continuation_locked: bool,
    pub equivalent_choice_and_served_boundary_locked: bool,
    pub portability_and_minimality_locked: bool,
    pub plugin_capability_boundary_locked: bool,
    pub stronger_terminal_claims_require_closure_bundle: bool,
    pub stronger_plugin_platform_claims_require_closure_bundle: bool,
    pub closure_bundle_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleAntiDriftStabilityCloseoutSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleAntiDriftStabilityCloseoutAuditReportError),
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

pub fn build_tassadar_post_article_anti_drift_stability_closeout_summary() -> Result<
    TassadarPostArticleAntiDriftStabilityCloseoutSummary,
    TassadarPostArticleAntiDriftStabilityCloseoutSummaryError,
> {
    let report = build_tassadar_post_article_anti_drift_stability_closeout_audit_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleAntiDriftStabilityCloseoutAuditReport,
) -> TassadarPostArticleAntiDriftStabilityCloseoutSummary {
    let mut summary = TassadarPostArticleAntiDriftStabilityCloseoutSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        closeout_status: report.closeout_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        lock_row_count: report.lock_rows.len() as u32,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        all_required_surface_locks_green: report.all_required_surface_locks_green,
        machine_identity_lock_complete: report.machine_identity_lock_complete,
        control_and_replay_posture_locked: report.control_and_replay_posture_locked,
        semantics_and_continuation_locked: report.semantics_and_continuation_locked,
        equivalent_choice_and_served_boundary_locked: report
            .equivalent_choice_and_served_boundary_locked,
        portability_and_minimality_locked: report.portability_and_minimality_locked,
        plugin_capability_boundary_locked: report.plugin_capability_boundary_locked,
        stronger_terminal_claims_require_closure_bundle: report
            .stronger_terminal_claims_require_closure_bundle,
        stronger_plugin_platform_claims_require_closure_bundle: report
            .stronger_plugin_platform_claims_require_closure_bundle,
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        detail: format!(
            "post-article anti-drift closeout summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, closeout_status={:?}, lock_rows={}, invalidation_rows={}, validation_rows={}, and closure_bundle_issue_id=`{}`.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.closeout_status,
            report.lock_rows.len(),
            report.invalidation_rows.len(),
            report.validation_rows.len(),
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_anti_drift_stability_closeout_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_anti_drift_stability_closeout_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_anti_drift_stability_closeout_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleAntiDriftStabilityCloseoutSummary,
    TassadarPostArticleAntiDriftStabilityCloseoutSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleAntiDriftStabilityCloseoutSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_anti_drift_stability_closeout_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleAntiDriftStabilityCloseoutSummaryError::Write {
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
) -> Result<T, TassadarPostArticleAntiDriftStabilityCloseoutSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleAntiDriftStabilityCloseoutSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleAntiDriftStabilityCloseoutSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_anti_drift_stability_closeout_summary, read_json,
        tassadar_post_article_anti_drift_stability_closeout_summary_path,
        write_tassadar_post_article_anti_drift_stability_closeout_summary,
        TassadarPostArticleAntiDriftStabilityCloseoutSummary,
        TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_SUMMARY_REF,
    };

    #[test]
    fn anti_drift_summary_keeps_closure_boundary_explicit() {
        let summary =
            build_tassadar_post_article_anti_drift_stability_closeout_summary().expect("summary");

        assert_eq!(
            summary.report_id,
            "tassadar.post_article_anti_drift_stability_closeout_audit.report.v1"
        );
        assert!(summary.all_required_surface_locks_green);
        assert!(summary.machine_identity_lock_complete);
        assert!(summary.control_and_replay_posture_locked);
        assert!(summary.semantics_and_continuation_locked);
        assert!(summary.equivalent_choice_and_served_boundary_locked);
        assert!(summary.portability_and_minimality_locked);
        assert!(summary.plugin_capability_boundary_locked);
        assert!(summary.stronger_terminal_claims_require_closure_bundle);
        assert!(summary.stronger_plugin_platform_claims_require_closure_bundle);
        assert_eq!(summary.lock_row_count, 12);
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn anti_drift_summary_matches_committed_truth() {
        let expected =
            build_tassadar_post_article_anti_drift_stability_closeout_summary().expect("expected");
        let committed: TassadarPostArticleAntiDriftStabilityCloseoutSummary = read_json(
            tassadar_post_article_anti_drift_stability_closeout_summary_path(),
        )
        .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_anti_drift_stability_closeout_summary_path().ends_with(
                "tassadar_post_article_anti_drift_stability_closeout_summary.json"
            )
        );
    }

    #[test]
    fn write_anti_drift_summary_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_anti_drift_stability_closeout_summary.json");
        let written =
            write_tassadar_post_article_anti_drift_stability_closeout_summary(&output_path)
                .expect("written");
        let roundtrip: TassadarPostArticleAntiDriftStabilityCloseoutSummary =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            TASSADAR_POST_ARTICLE_ANTI_DRIFT_STABILITY_CLOSEOUT_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_anti_drift_stability_closeout_summary.json"
        );
    }
}
