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
    build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalStatus,
};

pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_SUMMARY_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub control_trace_contract_id: String,
    pub control_trace_profile_id: String,
    pub determinism_profile_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub runtime_bundle_id: String,
    pub contract_status:
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalStatus,
    pub dependency_row_count: u32,
    pub controller_case_row_count: u32,
    pub control_trace_row_count: u32,
    pub host_negative_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub control_trace_contract_green: bool,
    pub determinism_profile_explicit: bool,
    pub typed_refusal_loop_closed: bool,
    pub host_not_planner_green: bool,
    pub adversarial_negative_rows_green: bool,
    pub closure_bundle_bound_by_digest: bool,
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
pub enum TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError {
    #[error(transparent)]
    Eval(
        #[from]
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
    ),
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

pub fn build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary(
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError,
> {
    let report =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
) -> TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary {
    let mut summary =
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary {
            schema_version: 1,
            report_id: report.report_id.clone(),
            machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
            canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
            canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
            packet_abi_version: report.machine_identity_binding.packet_abi_version.clone(),
            host_owned_runtime_api_id: report
                .machine_identity_binding
                .host_owned_runtime_api_id
                .clone(),
            engine_abstraction_id: report.machine_identity_binding.engine_abstraction_id.clone(),
            invocation_receipt_profile_id: report
                .machine_identity_binding
                .invocation_receipt_profile_id
                .clone(),
            result_binding_contract_id: report
                .machine_identity_binding
                .result_binding_contract_id
                .clone(),
            model_loop_return_profile_id: report
                .machine_identity_binding
                .model_loop_return_profile_id
                .clone(),
            control_trace_contract_id: report
                .machine_identity_binding
                .control_trace_contract_id
                .clone(),
            control_trace_profile_id: report
                .machine_identity_binding
                .control_trace_profile_id
                .clone(),
            determinism_profile_id: report
                .machine_identity_binding
                .determinism_profile_id
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
            runtime_bundle_id: report.machine_identity_binding.runtime_bundle_id.clone(),
            contract_status: report.contract_status,
            dependency_row_count: report.dependency_rows.len() as u32,
            controller_case_row_count: report.controller_case_rows.len() as u32,
            control_trace_row_count: report.control_trace_rows.len() as u32,
            host_negative_row_count: report.host_negative_rows.len() as u32,
            validation_row_count: report.validation_rows.len() as u32,
            deferred_issue_ids: report.deferred_issue_ids.clone(),
            control_trace_contract_green: report.control_trace_contract_green,
            determinism_profile_explicit: report.determinism_profile_explicit,
            typed_refusal_loop_closed: report.typed_refusal_loop_closed,
            host_not_planner_green: report.host_not_planner_green,
            adversarial_negative_rows_green: report.adversarial_negative_rows_green,
            closure_bundle_bound_by_digest: report.closure_bundle_bound_by_digest,
            operator_internal_only_posture: report.operator_internal_only_posture,
            rebase_claim_allowed: report.rebase_claim_allowed,
            plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
            plugin_publication_allowed: report.plugin_publication_allowed,
            served_public_universality_allowed: report.served_public_universality_allowed,
            arbitrary_software_capability_allowed: report
                .arbitrary_software_capability_allowed,
            detail: format!(
                "post-article weighted plugin controller summary keeps machine_identity_id=`{}`, control_trace_contract_id=`{}`, contract_status={:?}, controller_case_rows={}, control_trace_rows={}, validation_rows={}, closure_bundle_digest=`{}`, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
                report.machine_identity_binding.machine_identity_id,
                report.machine_identity_binding.control_trace_contract_id,
                report.contract_status,
                report.controller_case_rows.len(),
                report.control_trace_rows.len(),
                report.validation_rows.len(),
                report.machine_identity_binding.closure_bundle_digest,
                report.weighted_plugin_control_allowed,
                report.deferred_issue_ids.len(),
            ),
            summary_digest: String::new(),
        };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_SUMMARY_REF,
    )
}

pub fn write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError::Write {
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
) -> Result<T, TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError>
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary,
        read_repo_json,
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary_path,
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary,
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_SUMMARY_REF,
    };

    #[test]
    fn post_article_weighted_plugin_controller_summary_covers_declared_rows() {
        let summary =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary()
                .expect("summary");

        assert_eq!(summary.dependency_row_count, 5);
        assert_eq!(summary.controller_case_row_count, 5);
        assert_eq!(summary.control_trace_row_count, 40);
        assert_eq!(summary.host_negative_row_count, 10);
        assert_eq!(summary.validation_row_count, 10);
        assert!(summary.closure_bundle_bound_by_digest);
        assert!(summary.control_trace_contract_green);
        assert!(summary.determinism_profile_explicit);
        assert!(summary.typed_refusal_loop_closed);
        assert!(summary.host_not_planner_green);
        assert!(summary.adversarial_negative_rows_green);
        assert!(summary.weighted_plugin_control_allowed);
        assert!(summary.deferred_issue_ids.is_empty());
    }

    #[test]
    fn post_article_weighted_plugin_controller_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary()
                .expect("summary");
        let committed: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary.json"
            )
        );
    }

    #[test]
    fn write_post_article_weighted_plugin_controller_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary.json",
        );
        let written =
            write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
