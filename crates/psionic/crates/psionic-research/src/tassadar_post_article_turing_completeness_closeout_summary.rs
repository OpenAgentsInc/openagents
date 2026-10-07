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
    build_tassadar_post_article_turing_completeness_closeout_audit_report,
    TassadarPostArticleTuringCompletenessCloseoutAuditReport,
    TassadarPostArticleTuringCompletenessCloseoutAuditReportError,
    TassadarPostArticleTuringCompletenessCloseoutStatus,
};

pub const TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_turing_completeness_closeout_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessCloseoutSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_architecture_anchor_crate: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub closeout_status: TassadarPostArticleTuringCompletenessCloseoutStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub validation_row_count: u32,
    pub historical_tas_156_still_stands: bool,
    pub canonical_route_truth_carrier: bool,
    pub control_plane_proof_part_of_truth_carrier: bool,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub closure_bundle_issue_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleTuringCompletenessCloseoutSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleTuringCompletenessCloseoutAuditReportError),
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

pub fn build_tassadar_post_article_turing_completeness_closeout_summary() -> Result<
    TassadarPostArticleTuringCompletenessCloseoutSummary,
    TassadarPostArticleTuringCompletenessCloseoutSummaryError,
> {
    let report = build_tassadar_post_article_turing_completeness_closeout_audit_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleTuringCompletenessCloseoutAuditReport,
) -> TassadarPostArticleTuringCompletenessCloseoutSummary {
    let mut summary = TassadarPostArticleTuringCompletenessCloseoutSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        canonical_architecture_anchor_crate: report
            .machine_identity_binding
            .canonical_architecture_anchor_crate
            .clone(),
        closure_bundle_report_id: report
            .machine_identity_binding
            .closure_bundle_report_id
            .clone(),
        closure_bundle_report_digest: report
            .machine_identity_binding
            .closure_bundle_report_digest
            .clone(),
        closure_bundle_digest: report
            .machine_identity_binding
            .closure_bundle_digest
            .clone(),
        closeout_status: report.closeout_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        historical_tas_156_still_stands: report.historical_tas_156_still_stands,
        canonical_route_truth_carrier: report.canonical_route_truth_carrier,
        control_plane_proof_part_of_truth_carrier: report
            .control_plane_proof_part_of_truth_carrier,
        closure_bundle_embedded_here: report.closure_bundle_embedded_here,
        closure_bundle_bound_by_digest: report.closure_bundle_bound_by_digest,
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        theory_green: report.theory_green,
        operator_green: report.operator_green,
        served_green: report.served_green,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article turing-completeness closeout summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, closeout_status={:?}, historical_tas_156_still_stands={}, closure_bundle_digest=`{}`, and closure_bundle_issue_id=`{}`.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.closeout_status,
            report.historical_tas_156_still_stands,
            report.machine_identity_binding.closure_bundle_digest,
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_turing_completeness_closeout_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_turing_completeness_closeout_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_turing_completeness_closeout_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleTuringCompletenessCloseoutSummary,
    TassadarPostArticleTuringCompletenessCloseoutSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleTuringCompletenessCloseoutSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_turing_completeness_closeout_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutSummaryError::Write {
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
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleTuringCompletenessCloseoutSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_turing_completeness_closeout_summary, read_repo_json,
        tassadar_post_article_turing_completeness_closeout_summary_path,
        write_tassadar_post_article_turing_completeness_closeout_summary,
        TassadarPostArticleTuringCompletenessCloseoutSummary,
        TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleTuringCompletenessCloseoutStatus;
    use tempfile::tempdir;

    #[test]
    fn post_article_turing_completeness_closeout_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_turing_completeness_closeout_summary().expect("summary");

        assert_eq!(
            summary.closeout_status,
            TassadarPostArticleTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed
        );
        assert_eq!(summary.supporting_material_row_count, 14);
        assert_eq!(summary.dependency_row_count, 12);
        assert_eq!(summary.validation_row_count, 11);
        assert!(summary.historical_tas_156_still_stands);
        assert!(summary.canonical_route_truth_carrier);
        assert!(summary.control_plane_proof_part_of_truth_carrier);
        assert!(summary.closure_bundle_bound_by_digest);
        assert!(!summary.closure_bundle_embedded_here);
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
        assert!(summary.theory_green);
        assert!(summary.operator_green);
        assert!(!summary.served_green);
        assert!(summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_turing_completeness_closeout_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_turing_completeness_closeout_summary().expect("summary");
        let committed: TassadarPostArticleTuringCompletenessCloseoutSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_turing_completeness_closeout_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_turing_completeness_closeout_summary.json")
        );
    }

    #[test]
    fn write_post_article_turing_completeness_closeout_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_turing_completeness_closeout_summary.json");
        let written =
            write_tassadar_post_article_turing_completeness_closeout_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarPostArticleTuringCompletenessCloseoutSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
