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
    build_tassadar_post_article_universality_portability_minimality_matrix_report,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_portability_minimality_matrix_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub matrix_status: TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus,
    pub bounded_universality_story_carried: bool,
    pub machine_row_count: u32,
    pub route_row_count: u32,
    pub minimality_row_count: u32,
    pub validation_row_count: u32,
    pub machine_matrix_green: bool,
    pub route_classification_green: bool,
    pub minimality_green: bool,
    pub served_suppression_boundary_preserved: bool,
    pub served_conformance_envelope_defined: bool,
    pub deferred_issue_ids: Vec<String>,
    pub universal_substrate_gate_allowed: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError),
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

pub fn build_tassadar_post_article_universality_portability_minimality_matrix_summary() -> Result<
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError,
> {
    let report = build_tassadar_post_article_universality_portability_minimality_matrix_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
) -> TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary {
    let mut summary = TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_model_id: report.canonical_model_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        matrix_status: report.matrix_status,
        bounded_universality_story_carried: report.bounded_universality_story_carried,
        machine_row_count: report.machine_matrix_rows.len() as u32,
        route_row_count: report.route_classification_rows.len() as u32,
        minimality_row_count: report.minimality_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        machine_matrix_green: report.machine_matrix_green,
        route_classification_green: report.route_classification_green,
        minimality_green: report.minimality_green,
        served_suppression_boundary_preserved: report.served_suppression_boundary_preserved,
        served_conformance_envelope_defined: report.served_conformance_envelope_defined,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        universal_substrate_gate_allowed: report.universal_substrate_gate_allowed,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article universality portability/minimality summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, machine_rows={}, route_rows={}, minimality_rows={}, validation_rows={}, matrix_status={:?}, and served_conformance_envelope_defined={}.",
            report.machine_identity_id,
            report.canonical_route_id,
            report.machine_matrix_rows.len(),
            report.route_classification_rows.len(),
            report.minimality_rows.len(),
            report.validation_rows.len(),
            report.matrix_status,
            report.served_conformance_envelope_defined,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_universality_portability_minimality_matrix_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_universality_portability_minimality_matrix_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_SUMMARY_REF)
}

pub fn write_tassadar_post_article_universality_portability_minimality_matrix_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_universality_portability_minimality_matrix_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError::Write {
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
) -> Result<T, TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universality_portability_minimality_matrix_summary,
        read_repo_json,
        tassadar_post_article_universality_portability_minimality_matrix_summary_path,
        write_tassadar_post_article_universality_portability_minimality_matrix_summary,
        TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary,
        TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus;
    use tempfile::tempdir;

    #[test]
    fn universality_portability_minimality_matrix_summary_keeps_next_frontier_visible() {
        let summary =
            build_tassadar_post_article_universality_portability_minimality_matrix_summary()
                .expect("summary");

        assert_eq!(
            summary.matrix_status,
            TassadarPostArticleUniversalityPortabilityMinimalityMatrixStatus::Green
        );
        assert!(summary.bounded_universality_story_carried);
        assert_eq!(summary.machine_row_count, 3);
        assert_eq!(summary.route_row_count, 4);
        assert_eq!(summary.minimality_row_count, 3);
        assert_eq!(summary.validation_row_count, 8);
        assert!(summary.machine_matrix_green);
        assert!(summary.route_classification_green);
        assert!(summary.minimality_green);
        assert!(summary.served_suppression_boundary_preserved);
        assert!(summary.served_conformance_envelope_defined);
        assert_eq!(summary.deferred_issue_ids, vec![String::from("TAS-194")]);
        assert!(summary.universal_substrate_gate_allowed);
        assert!(!summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn universality_portability_minimality_matrix_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_universality_portability_minimality_matrix_summary()
                .expect("summary");
        let committed: TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_universality_portability_minimality_matrix_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universality_portability_minimality_matrix_summary.json")
        );
    }

    #[test]
    fn write_universality_portability_minimality_matrix_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universality_portability_minimality_matrix_summary.json");
        let written =
            write_tassadar_post_article_universality_portability_minimality_matrix_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
