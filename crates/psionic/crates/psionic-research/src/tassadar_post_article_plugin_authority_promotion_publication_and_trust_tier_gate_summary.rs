use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_catalog::{
    build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_SUMMARY_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub control_trace_contract_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub contract_status:
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus,
    pub dependency_row_count: u32,
    pub trust_tier_row_count: u32,
    pub promotion_row_count: u32,
    pub publication_posture_row_count: u32,
    pub observer_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub trust_tier_gate_green: bool,
    pub promotion_receipts_explicit: bool,
    pub publication_posture_explicit: bool,
    pub observer_rights_explicit: bool,
    pub validator_hooks_explicit: bool,
    pub accepted_outcome_hooks_explicit: bool,
    pub operator_internal_only_posture: bool,
    pub profile_specific_named_routes_explicit: bool,
    pub broader_publication_refused: bool,
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
pub enum TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError {
    #[error(transparent)]
    Catalog(
        #[from] TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReportError,
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

pub fn build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary(
) -> Result<
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError,
> {
    let report =
        build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReport,
) -> TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary {
    let mut summary =
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary {
            schema_version: 1,
            report_id: report.report_id.clone(),
            machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
            canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
            canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
            computational_model_statement_id: report
                .machine_identity_binding
                .computational_model_statement_id
                .clone(),
            control_trace_contract_id: report
                .machine_identity_binding
                .control_trace_contract_id
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
            trust_tier_row_count: report.trust_tier_rows.len() as u32,
            promotion_row_count: report.promotion_rows.len() as u32,
            publication_posture_row_count: report.publication_posture_rows.len() as u32,
            observer_row_count: report.observer_rows.len() as u32,
            validation_row_count: report.validation_rows.len() as u32,
            deferred_issue_ids: report.deferred_issue_ids.clone(),
            trust_tier_gate_green: report.trust_tier_gate_green,
            promotion_receipts_explicit: report.promotion_receipts_explicit,
            publication_posture_explicit: report.publication_posture_explicit,
            observer_rights_explicit: report.observer_rights_explicit,
            validator_hooks_explicit: report.validator_hooks_explicit,
            accepted_outcome_hooks_explicit: report.accepted_outcome_hooks_explicit,
            operator_internal_only_posture: report.operator_internal_only_posture,
            profile_specific_named_routes_explicit: report.profile_specific_named_routes_explicit,
            broader_publication_refused: report.broader_publication_refused,
            closure_bundle_bound_by_digest: report.closure_bundle_bound_by_digest,
            rebase_claim_allowed: report.rebase_claim_allowed,
            plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
            plugin_publication_allowed: report.plugin_publication_allowed,
            served_public_universality_allowed: report.served_public_universality_allowed,
            arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin authority summary keeps machine_identity_id=`{}`, control_trace_contract_id=`{}`, contract_status={:?}, trust_tier_rows={}, publication_posture_rows={}, validation_rows={}, closure_bundle_digest=`{}`, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
                report.machine_identity_binding.machine_identity_id,
                report.machine_identity_binding.control_trace_contract_id,
                report.contract_status,
                report.trust_tier_rows.len(),
                report.publication_posture_rows.len(),
                report.validation_rows.len(),
                report.machine_identity_binding.closure_bundle_digest,
                report.weighted_plugin_control_allowed,
                report.deferred_issue_ids.len(),
            ),
            summary_digest: String::new(),
        };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_SUMMARY_REF,
    )
}

pub fn write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary,
    TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary =
        build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError::Write {
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
) -> Result<T, TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary,
        read_repo_json,
        tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary_path,
        write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary,
        TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary,
        TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_SUMMARY_REF,
    };
    use psionic_catalog::TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus;
    use tempfile::tempdir;

    #[test]
    fn post_article_plugin_authority_gate_summary_keeps_frontier_explicit() {
        let summary =
            build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary()
                .expect("summary");

        assert_eq!(
            summary.contract_status,
            TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateStatus::Green
        );
        assert_eq!(summary.dependency_row_count, 9);
        assert_eq!(summary.trust_tier_row_count, 5);
        assert_eq!(summary.promotion_row_count, 5);
        assert_eq!(summary.publication_posture_row_count, 6);
        assert_eq!(summary.observer_row_count, 4);
        assert_eq!(summary.validation_row_count, 10);
        assert!(summary.deferred_issue_ids.is_empty());
        assert!(summary.trust_tier_gate_green);
        assert!(summary.promotion_receipts_explicit);
        assert!(summary.publication_posture_explicit);
        assert!(summary.observer_rights_explicit);
        assert!(summary.validator_hooks_explicit);
        assert!(summary.accepted_outcome_hooks_explicit);
        assert!(summary.operator_internal_only_posture);
        assert!(summary.profile_specific_named_routes_explicit);
        assert!(summary.broader_publication_refused);
        assert!(summary.closure_bundle_bound_by_digest);
        assert!(summary.rebase_claim_allowed);
        assert!(summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_authority_gate_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary()
                .expect("summary");
        let committed: TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_authority_gate_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary.json",
        );
        let written =
            write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary(
                &output_path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
