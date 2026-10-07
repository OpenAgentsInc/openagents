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
    build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
    TassadarPostArticleEquivalentChoiceNeutralityStatus,
};

pub const TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_SUMMARY_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub control_plane_equivalent_choice_relation_id: String,
    pub admissibility_contract_id: String,
    pub contract_status: TassadarPostArticleEquivalentChoiceNeutralityStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub equivalent_choice_class_row_count: u32,
    pub case_binding_row_count: u32,
    pub validation_row_count: u32,
    pub equivalent_choice_neutrality_complete: bool,
    pub admissibility_narrowing_receipt_visible: bool,
    pub hidden_ordering_or_ranking_quarantined: bool,
    pub latency_cost_and_soft_failure_channels_blocked: bool,
    pub served_or_plugin_equivalence_overclaim_refused: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError),
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

pub fn build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary(
) -> Result<
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError,
> {
    let report =
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
        )?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
) -> TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary {
    let mut summary =
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary {
            schema_version: 1,
            report_id: report.report_id.clone(),
            machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
            canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
            canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
            control_plane_equivalent_choice_relation_id: report
                .machine_identity_binding
                .control_plane_equivalent_choice_relation_id
                .clone(),
            admissibility_contract_id: report
                .machine_identity_binding
                .admissibility_contract_id
                .clone(),
            contract_status: report.contract_status,
            supporting_material_row_count: report.supporting_material_rows.len() as u32,
            dependency_row_count: report.dependency_rows.len() as u32,
            equivalent_choice_class_row_count: report.equivalent_choice_class_rows.len() as u32,
            case_binding_row_count: report.case_binding_rows.len() as u32,
            validation_row_count: report.validation_rows.len() as u32,
            equivalent_choice_neutrality_complete: report.equivalent_choice_neutrality_complete,
            admissibility_narrowing_receipt_visible: report
                .admissibility_narrowing_receipt_visible,
            hidden_ordering_or_ranking_quarantined: report
                .hidden_ordering_or_ranking_quarantined,
            latency_cost_and_soft_failure_channels_blocked: report
                .latency_cost_and_soft_failure_channels_blocked,
            served_or_plugin_equivalence_overclaim_refused: report
                .served_or_plugin_equivalence_overclaim_refused,
            next_stability_issue_id: report.next_stability_issue_id.clone(),
            closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article equivalent-choice neutrality summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, equivalent_choice_class_rows={}, case_binding_rows={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
                report.machine_identity_binding.machine_identity_id,
                report.machine_identity_binding.canonical_route_id,
                report.contract_status,
                report.equivalent_choice_class_rows.len(),
                report.case_binding_rows.len(),
                report.next_stability_issue_id,
                report.closure_bundle_issue_id,
            ),
            summary_digest: String::new(),
        };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_SUMMARY_REF,
    )
}

pub fn write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary =
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary(
        )?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError::Write {
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
) -> Result<T, TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError>
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary,
        read_json,
        tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary_path,
        write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary,
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary,
        TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_SUMMARY_REF,
    };

    #[test]
    fn equivalent_choice_neutrality_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary()
                .expect("summary");

        assert_eq!(
            summary.report_id,
            "tassadar.post_article_equivalent_choice_neutrality_and_admissibility.report.v1"
        );
        assert!(summary.equivalent_choice_neutrality_complete);
        assert!(summary.admissibility_narrowing_receipt_visible);
        assert!(summary.hidden_ordering_or_ranking_quarantined);
        assert!(summary.latency_cost_and_soft_failure_channels_blocked);
        assert!(summary.served_or_plugin_equivalence_overclaim_refused);
        assert_eq!(summary.equivalent_choice_class_row_count, 5);
        assert_eq!(summary.next_stability_issue_id, "TAS-215");
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn equivalent_choice_neutrality_summary_matches_committed_truth() {
        let expected =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary()
                .expect("expected");
        let committed: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary =
            read_json(
                tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary_path(),
            )
            .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary_path()
                .ends_with(
                    "tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary.json"
                )
        );
    }

    #[test]
    fn write_equivalent_choice_neutrality_summary_persists_truth() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output_path = dir.path().join(
            "tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary.json",
        );

        let written =
            write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary(
                &output_path,
            )
            .expect("written");
        let reread: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary =
            read_json(&output_path).expect("reread");

        assert_eq!(written, reread);
        assert_eq!(
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary.json"
        );
    }
}
