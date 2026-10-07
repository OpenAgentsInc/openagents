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
    build_tassadar_post_article_starter_plugin_catalog_eval_report,
    TassadarPostArticleStarterPluginCatalogEvalReport,
    TassadarPostArticleStarterPluginCatalogEvalReportError,
    TassadarPostArticleStarterPluginCatalogEvalStatus,
};

pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_starter_plugin_catalog_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub closure_bundle_digest: String,
    pub eval_status: TassadarPostArticleStarterPluginCatalogEvalStatus,
    pub starter_plugin_count: u32,
    pub local_deterministic_plugin_count: u32,
    pub read_only_network_plugin_count: u32,
    pub bounded_flow_count: u32,
    pub operator_internal_only_posture: bool,
    pub public_marketplace_language_suppressed: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub next_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleStarterPluginCatalogSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleStarterPluginCatalogEvalReportError),
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

pub fn build_tassadar_post_article_starter_plugin_catalog_summary() -> Result<
    TassadarPostArticleStarterPluginCatalogSummary,
    TassadarPostArticleStarterPluginCatalogSummaryError,
> {
    let report = build_tassadar_post_article_starter_plugin_catalog_eval_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleStarterPluginCatalogEvalReport,
) -> TassadarPostArticleStarterPluginCatalogSummary {
    let mut summary = TassadarPostArticleStarterPluginCatalogSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        closure_bundle_digest: report.machine_identity_binding.closure_bundle_digest.clone(),
        eval_status: report.eval_status,
        starter_plugin_count: report.starter_plugin_count,
        local_deterministic_plugin_count: report.local_deterministic_plugin_count,
        read_only_network_plugin_count: report.read_only_network_plugin_count,
        bounded_flow_count: report.bounded_flow_count,
        operator_internal_only_posture: report.operator_internal_only_posture,
        public_marketplace_language_suppressed: report.public_marketplace_language_suppressed,
        closure_bundle_bound_by_digest: report.closure_bundle_bound_by_digest,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        next_issue_id: report.next_issue_id.clone(),
        detail: format!(
            "starter catalog summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, starter_plugin_count={}, bounded_flow_count={}, operator_internal_only_posture={}, closure_bundle_digest=`{}`, and next_issue_id=`{}` explicit.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.starter_plugin_count,
            report.bounded_flow_count,
            report.operator_internal_only_posture,
            report.machine_identity_binding.closure_bundle_digest,
            report.next_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_catalog_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_starter_plugin_catalog_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_SUMMARY_REF)
}

pub fn write_tassadar_post_article_starter_plugin_catalog_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleStarterPluginCatalogSummary,
    TassadarPostArticleStarterPluginCatalogSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleStarterPluginCatalogSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_starter_plugin_catalog_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogSummaryError::Write {
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
) -> Result<T, TassadarPostArticleStarterPluginCatalogSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_starter_plugin_catalog_summary, read_json,
        tassadar_post_article_starter_plugin_catalog_summary_path,
        write_tassadar_post_article_starter_plugin_catalog_summary,
        TassadarPostArticleStarterPluginCatalogSummary,
        TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_SUMMARY_REF,
    };

    #[test]
    fn starter_plugin_catalog_summary_keeps_bounded_counts_and_posture_explicit() {
        let summary =
            build_tassadar_post_article_starter_plugin_catalog_summary().expect("summary");

        assert_eq!(
            summary.report_id,
            "tassadar.post_article.starter_plugin_catalog.eval_report.v1"
        );
        assert_eq!(summary.starter_plugin_count, 6);
        assert_eq!(summary.local_deterministic_plugin_count, 5);
        assert_eq!(summary.read_only_network_plugin_count, 1);
        assert_eq!(summary.bounded_flow_count, 2);
        assert!(summary.operator_internal_only_posture);
        assert!(summary.public_marketplace_language_suppressed);
        assert!(summary.closure_bundle_bound_by_digest);
        assert!(summary.plugin_capability_claim_allowed);
        assert!(summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert_eq!(summary.next_issue_id, "TAS-217");
    }

    #[test]
    fn starter_plugin_catalog_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_starter_plugin_catalog_summary().expect("summary");
        let committed: TassadarPostArticleStarterPluginCatalogSummary =
            read_json(tassadar_post_article_starter_plugin_catalog_summary_path())
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_starter_plugin_catalog_summary_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_starter_plugin_catalog_summary.json");
        let written = write_tassadar_post_article_starter_plugin_catalog_summary(&output_path)
            .expect("write");
        let roundtrip: TassadarPostArticleStarterPluginCatalogSummary =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_post_article_starter_plugin_catalog_summary_path()
                .strip_prefix(super::repo_root())
                .expect("starter catalog summary should live under repo root")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_SUMMARY_REF
        );
    }
}
