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
    build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    TassadarPostArticleExecutionSemanticsProofTransportStatus,
};

pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_execution_semantics_proof_transport_audit_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsProofTransportAuditSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub audit_status: TassadarPostArticleExecutionSemanticsProofTransportStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub plugin_surface_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub proof_transport_issue_id: String,
    pub proof_transport_complete: bool,
    pub plugin_execution_transport_bound: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleExecutionSemanticsProofTransportAuditReportError),
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

pub fn build_tassadar_post_article_execution_semantics_proof_transport_audit_summary() -> Result<
    TassadarPostArticleExecutionSemanticsProofTransportAuditSummary,
    TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError,
> {
    let report = build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
) -> TassadarPostArticleExecutionSemanticsProofTransportAuditSummary {
    let mut summary = TassadarPostArticleExecutionSemanticsProofTransportAuditSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_model_id: report.canonical_model_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        continuation_contract_id: report.continuation_contract_id.clone(),
        computational_model_statement_id: report.computational_model_statement_id.clone(),
        proof_transport_boundary_id: report.transport_boundary.boundary_id.clone(),
        audit_status: report.audit_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        plugin_surface_row_count: report.plugin_surface_rows.len() as u32,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        proof_transport_issue_id: report.proof_transport_issue_id.clone(),
        proof_transport_complete: report.proof_transport_complete,
        plugin_execution_transport_bound: report.plugin_execution_transport_bound,
        next_stability_issue_id: report.next_stability_issue_id.clone(),
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        detail: format!(
            "post-article execution-semantics proof-transport summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, audit_status={:?}, plugin_surface_rows={}, proof_transport_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
            report.machine_identity_id,
            report.canonical_route_id,
            report.audit_status,
            report.plugin_surface_rows.len(),
            report.proof_transport_complete,
            report.next_stability_issue_id,
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_execution_semantics_proof_transport_audit_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_execution_semantics_proof_transport_audit_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_execution_semantics_proof_transport_audit_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleExecutionSemanticsProofTransportAuditSummary,
    TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_execution_semantics_proof_transport_audit_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError::Write {
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
) -> Result<T, TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_execution_semantics_proof_transport_audit_summary, read_json,
        read_repo_json, repo_root,
        tassadar_post_article_execution_semantics_proof_transport_audit_summary_path,
        write_tassadar_post_article_execution_semantics_proof_transport_audit_summary,
        TassadarPostArticleExecutionSemanticsProofTransportAuditSummary,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleExecutionSemanticsProofTransportStatus;
    use tempfile::tempdir;

    #[test]
    fn execution_semantics_proof_transport_audit_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_execution_semantics_proof_transport_audit_summary()
                .expect("summary");

        assert_eq!(
            summary.audit_status,
            TassadarPostArticleExecutionSemanticsProofTransportStatus::Green
        );
        assert_eq!(
            summary.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            summary.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(summary.supporting_material_row_count, 11);
        assert_eq!(summary.dependency_row_count, 9);
        assert_eq!(summary.plugin_surface_row_count, 3);
        assert_eq!(summary.invalidation_row_count, 6);
        assert_eq!(summary.validation_row_count, 9);
        assert_eq!(summary.proof_transport_issue_id, "TAS-209");
        assert!(summary.proof_transport_complete);
        assert!(summary.plugin_execution_transport_bound);
        assert_eq!(summary.next_stability_issue_id, "TAS-215");
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn execution_semantics_proof_transport_audit_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_execution_semantics_proof_transport_audit_summary()
                .expect("summary");
        let committed: TassadarPostArticleExecutionSemanticsProofTransportAuditSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_execution_semantics_proof_transport_audit_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_execution_semantics_proof_transport_audit_summary.json")
        );
    }

    #[test]
    fn execution_semantics_proof_transport_audit_summary_round_trips_to_disk() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_execution_semantics_proof_transport_audit_summary.json");
        let written =
            write_tassadar_post_article_execution_semantics_proof_transport_audit_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticleExecutionSemanticsProofTransportAuditSummary =
            read_json(&output_path).expect("persisted summary");
        assert_eq!(written, persisted);
    }

    #[test]
    fn execution_semantics_proof_transport_audit_summary_path_resolves_inside_repo() {
        assert_eq!(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_execution_semantics_proof_transport_audit_summary.json"
        );
        assert!(
            tassadar_post_article_execution_semantics_proof_transport_audit_summary_path()
                .starts_with(repo_root())
        );
    }
}
