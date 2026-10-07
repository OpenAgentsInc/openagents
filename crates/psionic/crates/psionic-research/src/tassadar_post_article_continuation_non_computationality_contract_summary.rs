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
    build_tassadar_post_article_continuation_non_computationality_contract_report,
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
    TassadarPostArticleContinuationNonComputationalityStatus,
};

pub const TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_continuation_non_computationality_contract_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationNonComputationalityContractSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub contract_status: TassadarPostArticleContinuationNonComputationalityStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub continuation_surface_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub continuation_extends_execution_without_second_machine: bool,
    pub hidden_workflow_logic_refused: bool,
    pub continuation_expressivity_extension_blocked: bool,
    pub plugin_resume_hidden_compute_refused: bool,
    pub continuation_non_computationality_complete: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleContinuationNonComputationalityContractSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleContinuationNonComputationalityContractReportError),
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

pub fn build_tassadar_post_article_continuation_non_computationality_contract_summary() -> Result<
    TassadarPostArticleContinuationNonComputationalityContractSummary,
    TassadarPostArticleContinuationNonComputationalityContractSummaryError,
> {
    let report = build_tassadar_post_article_continuation_non_computationality_contract_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleContinuationNonComputationalityContractReport,
) -> TassadarPostArticleContinuationNonComputationalityContractSummary {
    let mut summary = TassadarPostArticleContinuationNonComputationalityContractSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        continuation_contract_id: report.machine_identity_binding.continuation_contract_id.clone(),
        computational_model_statement_id: report
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        proof_transport_boundary_id: report
            .machine_identity_binding
            .proof_transport_boundary_id
            .clone(),
        contract_status: report.contract_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        continuation_surface_row_count: report.continuation_surface_rows.len() as u32,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        continuation_extends_execution_without_second_machine: report
            .continuation_extends_execution_without_second_machine,
        hidden_workflow_logic_refused: report.hidden_workflow_logic_refused,
        continuation_expressivity_extension_blocked: report
            .continuation_expressivity_extension_blocked,
        plugin_resume_hidden_compute_refused: report.plugin_resume_hidden_compute_refused,
        continuation_non_computationality_complete: report
            .continuation_non_computationality_complete,
        next_stability_issue_id: report.next_stability_issue_id.clone(),
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        detail: format!(
            "post-article continuation non-computationality summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, continuation_surface_rows={}, continuation_non_computationality_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.contract_status,
            report.continuation_surface_rows.len(),
            report.continuation_non_computationality_complete,
            report.next_stability_issue_id,
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_continuation_non_computationality_contract_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_continuation_non_computationality_contract_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_continuation_non_computationality_contract_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleContinuationNonComputationalityContractSummary,
    TassadarPostArticleContinuationNonComputationalityContractSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleContinuationNonComputationalityContractSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_continuation_non_computationality_contract_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleContinuationNonComputationalityContractSummaryError::Write {
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
) -> Result<T, TassadarPostArticleContinuationNonComputationalityContractSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleContinuationNonComputationalityContractSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleContinuationNonComputationalityContractSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_continuation_non_computationality_contract_summary, read_json,
        tassadar_post_article_continuation_non_computationality_contract_summary_path,
        write_tassadar_post_article_continuation_non_computationality_contract_summary,
        TassadarPostArticleContinuationNonComputationalityContractSummary,
        TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleContinuationNonComputationalityStatus;
    use tempfile::tempdir;

    #[test]
    fn continuation_non_computationality_contract_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_continuation_non_computationality_contract_summary()
                .expect("summary");

        assert_eq!(
            summary.contract_status,
            TassadarPostArticleContinuationNonComputationalityStatus::Green
        );
        assert_eq!(summary.supporting_material_row_count, 10);
        assert_eq!(summary.dependency_row_count, 6);
        assert_eq!(summary.continuation_surface_row_count, 6);
        assert_eq!(summary.invalidation_row_count, 7);
        assert_eq!(summary.validation_row_count, 9);
        assert!(summary.continuation_extends_execution_without_second_machine);
        assert!(summary.hidden_workflow_logic_refused);
        assert!(summary.continuation_expressivity_extension_blocked);
        assert!(summary.plugin_resume_hidden_compute_refused);
        assert!(summary.continuation_non_computationality_complete);
        assert_eq!(summary.next_stability_issue_id, "TAS-215");
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn continuation_non_computationality_contract_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_continuation_non_computationality_contract_summary()
                .expect("summary");
        let committed: TassadarPostArticleContinuationNonComputationalityContractSummary =
            read_json(
                tassadar_post_article_continuation_non_computationality_contract_summary_path(),
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_continuation_non_computationality_contract_summary.json"
        );
    }

    #[test]
    fn continuation_non_computationality_contract_summary_round_trips_to_disk() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_continuation_non_computationality_contract_summary.json");
        let written =
            write_tassadar_post_article_continuation_non_computationality_contract_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticleContinuationNonComputationalityContractSummary =
            read_json(&output_path).expect("persisted summary");
        assert_eq!(written, persisted);
    }
}
