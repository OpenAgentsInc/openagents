use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_catalog::{
    build_tassadar_post_article_plugin_manifest_identity_contract_report,
    TassadarPostArticlePluginManifestIdentityContractReport,
    TassadarPostArticlePluginManifestIdentityContractReportError,
    TassadarPostArticlePluginManifestIdentityContractStatus,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestIdentityContractSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub contract_status: TassadarPostArticlePluginManifestIdentityContractStatus,
    pub dependency_row_count: u32,
    pub manifest_field_row_count: u32,
    pub invocation_identity_row_count: u32,
    pub hot_swap_rule_row_count: u32,
    pub packaging_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub operator_internal_only_posture: bool,
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
pub enum TassadarPostArticlePluginManifestIdentityContractSummaryError {
    #[error(transparent)]
    Catalog(#[from] TassadarPostArticlePluginManifestIdentityContractReportError),
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

pub fn build_tassadar_post_article_plugin_manifest_identity_contract_summary() -> Result<
    TassadarPostArticlePluginManifestIdentityContractSummary,
    TassadarPostArticlePluginManifestIdentityContractSummaryError,
> {
    let report = build_tassadar_post_article_plugin_manifest_identity_contract_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticlePluginManifestIdentityContractReport,
) -> TassadarPostArticlePluginManifestIdentityContractSummary {
    let mut summary = TassadarPostArticlePluginManifestIdentityContractSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        computational_model_statement_id: report
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        contract_status: report.contract_status,
        dependency_row_count: report.dependency_rows.len() as u32,
        manifest_field_row_count: report.manifest_field_rows.len() as u32,
        invocation_identity_row_count: report.invocation_identity_rows.len() as u32,
        hot_swap_rule_row_count: report.hot_swap_rule_rows.len() as u32,
        packaging_row_count: report.packaging_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        operator_internal_only_posture: report.operator_internal_only_posture,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article plugin-manifest summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, computational_model_statement_id=`{}`, contract_status={:?}, manifest_field_rows={}, and deferred_issue_ids={}.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.machine_identity_binding.computational_model_statement_id,
            report.contract_status,
            report.manifest_field_rows.len(),
            report.deferred_issue_ids.len(),
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_manifest_identity_contract_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_plugin_manifest_identity_contract_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_plugin_manifest_identity_contract_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginManifestIdentityContractSummary,
    TassadarPostArticlePluginManifestIdentityContractSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginManifestIdentityContractSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_plugin_manifest_identity_contract_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractSummaryError::Write {
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
) -> Result<T, TassadarPostArticlePluginManifestIdentityContractSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginManifestIdentityContractSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_manifest_identity_contract_summary, read_repo_json,
        tassadar_post_article_plugin_manifest_identity_contract_summary_path,
        write_tassadar_post_article_plugin_manifest_identity_contract_summary,
        TassadarPostArticlePluginManifestIdentityContractSummary,
        TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_SUMMARY_REF,
    };
    use psionic_catalog::TassadarPostArticlePluginManifestIdentityContractStatus;
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_manifest_summary_keeps_frontier_explicit() {
        let summary =
            build_tassadar_post_article_plugin_manifest_identity_contract_summary().expect("summary");

        assert_eq!(
            summary.contract_status,
            TassadarPostArticlePluginManifestIdentityContractStatus::Green
        );
        assert_eq!(summary.dependency_row_count, 6);
        assert_eq!(summary.manifest_field_row_count, 12);
        assert_eq!(summary.invocation_identity_row_count, 3);
        assert_eq!(summary.hot_swap_rule_row_count, 4);
        assert_eq!(summary.packaging_row_count, 3);
        assert_eq!(summary.validation_row_count, 8);
        assert!(summary.deferred_issue_ids.is_empty());
        assert!(summary.operator_internal_only_posture);
        assert!(summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_manifest_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_manifest_identity_contract_summary().expect("summary");
        let committed: TassadarPostArticlePluginManifestIdentityContractSummary = read_repo_json(
            TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_SUMMARY_REF,
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_manifest_identity_contract_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_plugin_manifest_identity_contract_summary.json")
        );
    }

    #[test]
    fn write_post_article_plugin_manifest_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_manifest_identity_contract_summary.json");
        let written = write_tassadar_post_article_plugin_manifest_identity_contract_summary(
            &output_path,
        )
        .expect("write summary");
        let persisted: TassadarPostArticlePluginManifestIdentityContractSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
