use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_sandbox::{
    build_tassadar_post_article_plugin_charter_authority_boundary_report,
    TassadarPostArticlePluginCharterAuthorityBoundaryReport,
    TassadarPostArticlePluginCharterAuthorityBoundaryReportError,
    TassadarPostArticlePluginCharterAuthorityBoundaryStatus,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_charter_authority_boundary_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCharterAuthorityBoundarySummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub charter_status: TassadarPostArticlePluginCharterAuthorityBoundaryStatus,
    pub dependency_row_count: u32,
    pub law_row_count: u32,
    pub state_class_row_count: u32,
    pub governance_row_count: u32,
    pub validation_row_count: u32,
    pub current_publication_posture: String,
    pub first_plugin_tranche_posture: String,
    pub deferred_issue_ids: Vec<String>,
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
pub enum TassadarPostArticlePluginCharterAuthorityBoundarySummaryError {
    #[error(transparent)]
    Sandbox(#[from] TassadarPostArticlePluginCharterAuthorityBoundaryReportError),
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

pub fn build_tassadar_post_article_plugin_charter_authority_boundary_summary() -> Result<
    TassadarPostArticlePluginCharterAuthorityBoundarySummary,
    TassadarPostArticlePluginCharterAuthorityBoundarySummaryError,
> {
    let report = build_tassadar_post_article_plugin_charter_authority_boundary_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticlePluginCharterAuthorityBoundaryReport,
) -> TassadarPostArticlePluginCharterAuthorityBoundarySummary {
    let mut summary = TassadarPostArticlePluginCharterAuthorityBoundarySummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        computational_model_statement_id: report
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        charter_status: report.charter_status,
        dependency_row_count: report.dependency_rows.len() as u32,
        law_row_count: report.law_rows.len() as u32,
        state_class_row_count: report.state_class_rows.len() as u32,
        governance_row_count: report.governance_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        current_publication_posture: report.current_publication_posture.clone(),
        first_plugin_tranche_posture: report.first_plugin_tranche_posture.clone(),
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article plugin-charter summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, computational_model_statement_id=`{}`, charter_status={:?}, current_publication_posture=`{}`, and deferred_issue_ids={}.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.machine_identity_binding.computational_model_statement_id,
            report.charter_status,
            report.current_publication_posture,
            report.deferred_issue_ids.len(),
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_charter_authority_boundary_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_plugin_charter_authority_boundary_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_SUMMARY_REF)
}

pub fn write_tassadar_post_article_plugin_charter_authority_boundary_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginCharterAuthorityBoundarySummary,
    TassadarPostArticlePluginCharterAuthorityBoundarySummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginCharterAuthorityBoundarySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_plugin_charter_authority_boundary_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundarySummaryError::Write {
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
) -> Result<T, TassadarPostArticlePluginCharterAuthorityBoundarySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundarySummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginCharterAuthorityBoundarySummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_charter_authority_boundary_summary, read_repo_json,
        tassadar_post_article_plugin_charter_authority_boundary_summary_path,
        write_tassadar_post_article_plugin_charter_authority_boundary_summary,
        TassadarPostArticlePluginCharterAuthorityBoundarySummary,
        TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_SUMMARY_REF,
    };
    use psionic_sandbox::TassadarPostArticlePluginCharterAuthorityBoundaryStatus;
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_charter_summary_keeps_frontier_explicit() {
        let summary = build_tassadar_post_article_plugin_charter_authority_boundary_summary()
            .expect("summary");

        assert_eq!(
            summary.charter_status,
            TassadarPostArticlePluginCharterAuthorityBoundaryStatus::Green
        );
        assert_eq!(summary.dependency_row_count, 11);
        assert_eq!(summary.law_row_count, 17);
        assert_eq!(summary.state_class_row_count, 4);
        assert_eq!(summary.governance_row_count, 4);
        assert_eq!(summary.validation_row_count, 10);
        assert_eq!(
            summary.current_publication_posture,
            "internal_only_until_later_plugin_platform_gates"
        );
        assert_eq!(
            summary.first_plugin_tranche_posture,
            "closed_world_operator_curated_only_until_audited"
        );
        assert!(summary.deferred_issue_ids.is_empty());
        assert!(summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_charter_summary_matches_committed_truth() {
        let generated = build_tassadar_post_article_plugin_charter_authority_boundary_summary()
            .expect("summary");
        let committed: TassadarPostArticlePluginCharterAuthorityBoundarySummary =
            read_repo_json(TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_charter_authority_boundary_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_plugin_charter_authority_boundary_summary.json")
        );
    }

    #[test]
    fn write_post_article_plugin_charter_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_charter_authority_boundary_summary.json");
        let written =
            write_tassadar_post_article_plugin_charter_authority_boundary_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarPostArticlePluginCharterAuthorityBoundarySummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
