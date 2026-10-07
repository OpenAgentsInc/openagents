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
    build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub contract_status: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus,
    pub dependency_row_count: u32,
    pub receipt_identity_row_count: u32,
    pub replay_class_row_count: u32,
    pub failure_class_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub operator_internal_only_posture: bool,
    pub closure_bundle_bound_by_digest: bool,
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
pub enum TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError),
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

pub fn build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary() -> Result<
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError,
> {
    let report =
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport,
) -> TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary {
    let mut summary = TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        computational_model_statement_id: report
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        packet_abi_version: report.machine_identity_binding.packet_abi_version.clone(),
        host_owned_runtime_api_id: report
            .machine_identity_binding
            .host_owned_runtime_api_id
            .clone(),
        engine_abstraction_id: report
            .machine_identity_binding
            .engine_abstraction_id
            .clone(),
        invocation_receipt_profile_id: report
            .machine_identity_binding
            .invocation_receipt_profile_id
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
        contract_status: report.contract_status,
        dependency_row_count: report.dependency_rows.len() as u32,
        receipt_identity_row_count: report.receipt_identity_rows.len() as u32,
        replay_class_row_count: report.replay_class_rows.len() as u32,
        failure_class_row_count: report.failure_class_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        operator_internal_only_posture: report.operator_internal_only_posture,
        closure_bundle_bound_by_digest: report.closure_bundle_bound_by_digest,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article plugin invocation-receipt summary keeps machine_identity_id=`{}`, invocation_receipt_profile_id=`{}`, contract_status={:?}, receipt_identity_rows={}, replay_class_rows={}, closure_bundle_digest=`{}`, and deferred_issue_ids={}.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.invocation_receipt_profile_id,
            report.contract_status,
            report.receipt_identity_rows.len(),
            report.replay_class_rows.len(),
            report.machine_identity_binding.closure_bundle_digest,
            report.deferred_issue_ids.len(),
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary_path() -> PathBuf
{
    repo_root()
        .join(TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_SUMMARY_REF)
}

pub fn write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary =
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError::Write {
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
) -> Result<T, TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary,
        read_repo_json,
        tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary_path,
        write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary,
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary,
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus;

    #[test]
    fn post_article_plugin_invocation_receipts_summary_keeps_frontier_explicit() {
        let summary =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary()
                .expect("summary");

        assert_eq!(
            summary.contract_status,
            TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus::Green
        );
        assert_eq!(summary.dependency_row_count, 6);
        assert_eq!(summary.receipt_identity_row_count, 18);
        assert_eq!(summary.replay_class_row_count, 4);
        assert_eq!(summary.failure_class_row_count, 12);
        assert_eq!(summary.validation_row_count, 10);
        assert!(summary.deferred_issue_ids.is_empty());
        assert!(summary.closure_bundle_bound_by_digest);
        assert!(summary.operator_internal_only_posture);
        assert!(summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_invocation_receipts_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary()
                .expect("summary");
        let committed: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_invocation_receipts_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary.json",
        );
        let written =
            write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
