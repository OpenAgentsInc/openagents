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
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    TassadarPostArticleCanonicalRouteSemanticPreservationStatus,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_route_semantic_preservation_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteSemanticPreservationSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub semantic_preservation_status: TassadarPostArticleCanonicalRouteSemanticPreservationStatus,
    pub semantic_preservation_audit_green: bool,
    pub state_ownership_green: bool,
    pub control_ownership_rule_green: bool,
    pub semantic_preservation_green: bool,
    pub decision_provenance_proof_complete: bool,
    pub carrier_split_publication_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub state_class_ids: Vec<String>,
    pub continuation_mechanism_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError),
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

pub fn build_tassadar_post_article_canonical_route_semantic_preservation_summary() -> Result<
    TassadarPostArticleCanonicalRouteSemanticPreservationSummary,
    TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError,
> {
    let report = build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
) -> TassadarPostArticleCanonicalRouteSemanticPreservationSummary {
    let mut summary = TassadarPostArticleCanonicalRouteSemanticPreservationSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report
            .canonical_identity_review
            .machine_identity_id
            .clone(),
        canonical_route_id: report.canonical_identity_review.canonical_route_id.clone(),
        semantic_preservation_status: report.semantic_preservation_status,
        semantic_preservation_audit_green: report.semantic_preservation_audit_green,
        state_ownership_green: report.state_ownership_green,
        control_ownership_rule_green: report
            .control_ownership_boundary_review
            .control_ownership_rule_green,
        semantic_preservation_green: report.semantic_preservation_green,
        decision_provenance_proof_complete: report.decision_provenance_proof_complete,
        carrier_split_publication_complete: report.carrier_split_publication_complete,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        state_class_ids: report
            .state_class_rows
            .iter()
            .map(|row| row.state_class_id.clone())
            .collect(),
        continuation_mechanism_ids: report
            .continuation_mechanism_rows
            .iter()
            .map(|row| row.mechanism_id.clone())
            .collect(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article semantic-preservation summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, state_classes={}, continuation_mechanisms={}, semantic_preservation_status={:?}, decision_provenance_proof_complete={}, and carrier_split_publication_complete={}.",
            report.canonical_identity_review.machine_identity_id,
            report.canonical_identity_review.canonical_route_id,
            report.state_class_rows.len(),
            report.continuation_mechanism_rows.len(),
            report.semantic_preservation_status,
            report.decision_provenance_proof_complete,
            report.carrier_split_publication_complete,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_route_semantic_preservation_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_canonical_route_semantic_preservation_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_SUMMARY_REF)
}

pub fn write_tassadar_post_article_canonical_route_semantic_preservation_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalRouteSemanticPreservationSummary,
    TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_canonical_route_semantic_preservation_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError::Write {
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
) -> Result<T, TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalRouteSemanticPreservationSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_route_semantic_preservation_summary, read_repo_json,
        tassadar_post_article_canonical_route_semantic_preservation_summary_path,
        write_tassadar_post_article_canonical_route_semantic_preservation_summary,
        TassadarPostArticleCanonicalRouteSemanticPreservationSummary,
        TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_SUMMARY_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn semantic_preservation_summary_keeps_deferrals_visible() {
        let summary = build_tassadar_post_article_canonical_route_semantic_preservation_summary()
            .expect("summary");

        assert!(summary.semantic_preservation_audit_green);
        assert!(summary.state_ownership_green);
        assert!(summary.control_ownership_rule_green);
        assert!(summary.semantic_preservation_green);
        assert_eq!(
            summary.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            summary.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            summary.deferred_issue_ids,
            vec![String::from("TAS-188A"), String::from("TAS-189")]
        );
        assert!(!summary.decision_provenance_proof_complete);
        assert!(!summary.carrier_split_publication_complete);
        assert!(!summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn semantic_preservation_summary_matches_committed_truth() {
        let generated = build_tassadar_post_article_canonical_route_semantic_preservation_summary()
            .expect("summary");
        let committed: TassadarPostArticleCanonicalRouteSemanticPreservationSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_canonical_route_semantic_preservation_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_canonical_route_semantic_preservation_summary.json")
        );
    }

    #[test]
    fn write_semantic_preservation_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_canonical_route_semantic_preservation_summary.json");
        let written =
            write_tassadar_post_article_canonical_route_semantic_preservation_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarPostArticleCanonicalRouteSemanticPreservationSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
