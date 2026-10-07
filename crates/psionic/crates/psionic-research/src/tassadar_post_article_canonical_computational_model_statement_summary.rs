use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_post_article_canonical_computational_model_statement_report,
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
    TassadarPostArticleCanonicalComputationalModelStatus,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_computational_model_statement_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelStatementSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub statement_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub substrate_model_id: String,
    pub statement_status: TassadarPostArticleCanonicalComputationalModelStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub article_equivalent_compute_named: bool,
    pub tcm_v1_continuation_named: bool,
    pub declared_effect_boundary_named: bool,
    pub plugin_layer_scoped_above_machine: bool,
    pub proof_transport_complete: bool,
    pub proof_transport_audit_issue_id: String,
    pub next_stability_issue_id: String,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_issue_id: String,
    pub weighted_plugin_control_part_of_model: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalComputationalModelStatementSummaryError {
    #[error(transparent)]
    Runtime(#[from] TassadarPostArticleCanonicalComputationalModelStatementReportError),
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

pub fn build_tassadar_post_article_canonical_computational_model_statement_summary() -> Result<
    TassadarPostArticleCanonicalComputationalModelStatementSummary,
    TassadarPostArticleCanonicalComputationalModelStatementSummaryError,
> {
    let report = build_tassadar_post_article_canonical_computational_model_statement_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleCanonicalComputationalModelStatementReport,
) -> TassadarPostArticleCanonicalComputationalModelStatementSummary {
    let mut summary = TassadarPostArticleCanonicalComputationalModelStatementSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        statement_id: report.computational_model_statement.statement_id.clone(),
        machine_identity_id: report.computational_model_statement.machine_identity_id.clone(),
        canonical_model_id: report.computational_model_statement.canonical_model_id.clone(),
        canonical_route_id: report.computational_model_statement.canonical_route_id.clone(),
        continuation_contract_id: report
            .computational_model_statement
            .runtime_contract_id
            .clone(),
        substrate_model_id: report.computational_model_statement.substrate_model_id.clone(),
        statement_status: report.statement_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        article_equivalent_compute_named: report.article_equivalent_compute_named,
        tcm_v1_continuation_named: report.tcm_v1_continuation_named,
        declared_effect_boundary_named: report.declared_effect_boundary_named,
        plugin_layer_scoped_above_machine: report.plugin_layer_scoped_above_machine,
        proof_transport_complete: report.proof_transport_complete,
        proof_transport_audit_issue_id: report.proof_transport_audit_issue_id.clone(),
        next_stability_issue_id: report.next_stability_issue_id.clone(),
        closure_bundle_embedded_here: report.closure_bundle_embedded_here,
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        weighted_plugin_control_part_of_model: report.weighted_plugin_control_part_of_model,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article canonical computational-model summary keeps statement_id=`{}`, machine_identity_id=`{}`, statement_status={:?}, dependency_rows={}, invalidation_rows={}, validation_rows={}, proof_transport_complete={}, proof_transport_audit_issue_id=`{}`, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
            report.computational_model_statement.statement_id,
            report.computational_model_statement.machine_identity_id,
            report.statement_status,
            report.dependency_rows.len(),
            report.invalidation_rows.len(),
            report.validation_rows.len(),
            report.proof_transport_complete,
            report.proof_transport_audit_issue_id,
            report.next_stability_issue_id,
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_computational_model_statement_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_canonical_computational_model_statement_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_canonical_computational_model_statement_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalComputationalModelStatementSummary,
    TassadarPostArticleCanonicalComputationalModelStatementSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalComputationalModelStatementSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_canonical_computational_model_statement_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementSummaryError::Write {
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
) -> Result<T, TassadarPostArticleCanonicalComputationalModelStatementSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleCanonicalComputationalModelStatementSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_computational_model_statement_summary, read_json,
        read_repo_json, repo_root,
        tassadar_post_article_canonical_computational_model_statement_summary_path,
        write_tassadar_post_article_canonical_computational_model_statement_summary,
        TassadarPostArticleCanonicalComputationalModelStatementSummary,
        TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_SUMMARY_REF,
    };
    use psionic_runtime::TassadarPostArticleCanonicalComputationalModelStatus;
    use tempfile::tempdir;

    #[test]
    fn canonical_computational_model_statement_summary_keeps_scope_bounded() {
        let summary = build_tassadar_post_article_canonical_computational_model_statement_summary()
            .expect("summary");

        assert_eq!(
            summary.statement_status,
            TassadarPostArticleCanonicalComputationalModelStatus::Green
        );
        assert_eq!(
            summary.statement_id,
            "tassadar.post_article.canonical_computational_model.statement.v1"
        );
        assert_eq!(
            summary.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(summary.supporting_material_row_count, 11);
        assert_eq!(summary.dependency_row_count, 7);
        assert_eq!(summary.invalidation_row_count, 5);
        assert_eq!(summary.validation_row_count, 7);
        assert!(summary.article_equivalent_compute_named);
        assert!(summary.tcm_v1_continuation_named);
        assert!(summary.declared_effect_boundary_named);
        assert!(summary.plugin_layer_scoped_above_machine);
        assert!(summary.proof_transport_complete);
        assert_eq!(summary.proof_transport_audit_issue_id, "TAS-209");
        assert_eq!(summary.next_stability_issue_id, "TAS-215");
        assert!(!summary.closure_bundle_embedded_here);
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
        assert!(!summary.weighted_plugin_control_part_of_model);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn canonical_computational_model_statement_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_canonical_computational_model_statement_summary()
                .expect("summary");
        let committed: TassadarPostArticleCanonicalComputationalModelStatementSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_canonical_computational_model_statement_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_canonical_computational_model_statement_summary.json")
        );
    }

    #[test]
    fn canonical_computational_model_statement_summary_round_trips_to_disk() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_canonical_computational_model_statement_summary.json");
        let written = write_tassadar_post_article_canonical_computational_model_statement_summary(
            &output_path,
        )
        .expect("write summary");
        let persisted: TassadarPostArticleCanonicalComputationalModelStatementSummary =
            read_json(&output_path).expect("persisted summary");
        assert_eq!(written, persisted);
    }

    #[test]
    fn canonical_computational_model_statement_summary_path_resolves_inside_repo() {
        assert_eq!(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_canonical_computational_model_statement_summary.json"
        );
        assert!(
            tassadar_post_article_canonical_computational_model_statement_summary_path()
                .starts_with(repo_root())
        );
    }
}
