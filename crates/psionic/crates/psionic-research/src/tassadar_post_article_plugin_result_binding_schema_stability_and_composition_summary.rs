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
    build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionStatus,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_SUMMARY_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary {
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
    pub runtime_bundle_id: String,
    pub contract_status: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionStatus,
    pub dependency_row_count: u32,
    pub binding_row_count: u32,
    pub evidence_boundary_row_count: u32,
    pub composition_row_count: u32,
    pub negative_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub result_binding_contract_green: bool,
    pub semantic_composition_closure_green: bool,
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
pub enum TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError {
    #[error(transparent)]
    Eval(
        #[from] TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError,
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

pub fn build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary(
) -> Result<
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError,
> {
    let report =
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport,
) -> TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary {
    let mut summary = TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary {
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
        runtime_bundle_id: report.machine_identity_binding.runtime_bundle_id.clone(),
        contract_status: report.contract_status,
        dependency_row_count: report.dependency_rows.len() as u32,
        binding_row_count: report.binding_rows.len() as u32,
        evidence_boundary_row_count: report.evidence_boundary_rows.len() as u32,
        composition_row_count: report.composition_rows.len() as u32,
        negative_row_count: report.negative_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        result_binding_contract_green: report.result_binding_contract_green,
        semantic_composition_closure_green: report.semantic_composition_closure_green,
        operator_internal_only_posture: report.operator_internal_only_posture,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article plugin result-binding summary keeps machine_identity_id=`{}`, result_binding_contract_id=`{}`, contract_status={:?}, binding_rows={}, composition_rows={}, and deferred_issue_ids={}.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.result_binding_contract_id,
            report.contract_status,
            report.binding_rows.len(),
            report.composition_rows.len(),
            report.deferred_issue_ids.len(),
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_SUMMARY_REF,
    )
}

pub fn write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary =
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary(
        )?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError::Write {
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
) -> Result<T, TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary,
        read_repo_json,
        tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary_path,
        write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary,
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary,
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_SUMMARY_REF,
    };

    #[test]
    fn post_article_plugin_result_binding_summary_covers_declared_rows() {
        let summary =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary()
                .expect("summary");

        assert_eq!(
            summary.result_binding_contract_id,
            "tassadar.weighted_plugin.result_binding_contract.v1"
        );
        assert_eq!(
            summary.model_loop_return_profile_id,
            "tassadar.weighted_plugin.model_loop_return_profile.v1"
        );
        assert_eq!(summary.dependency_row_count, 7);
        assert_eq!(summary.binding_row_count, 6);
        assert_eq!(summary.evidence_boundary_row_count, 4);
        assert_eq!(summary.composition_row_count, 4);
        assert_eq!(summary.negative_row_count, 4);
        assert_eq!(summary.validation_row_count, 12);
        assert!(summary.deferred_issue_ids.is_empty());
        assert!(summary.result_binding_contract_green);
        assert!(summary.semantic_composition_closure_green);
        assert!(summary.operator_internal_only_posture);
        assert!(summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_result_binding_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary()
                .expect("summary");
        let committed: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary.json")
        );
    }

    #[test]
    fn write_post_article_plugin_result_binding_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary.json");
        let written =
            write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
