use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginManifestIdentityContractSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginManifestIdentityContractReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub contract_status: String,
    pub manifest_field_row_count: u32,
    pub hot_swap_rule_row_count: u32,
    pub packaging_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticlePluginManifestIdentityContractReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticlePluginManifestIdentityContractSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            computational_model_statement_id: summary.computational_model_statement_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            manifest_field_row_count: summary.manifest_field_row_count,
            hot_swap_rule_row_count: summary.hot_swap_rule_row_count,
            packaging_row_count: summary.packaging_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            operator_internal_only_posture: summary.operator_internal_only_posture,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin-manifest summary `{}` keeps contract_status={:?}, manifest_field_rows={}, packaging_rows={}, deferred_issue_ids={}, and plugin/publication claims blocked={}.",
                summary.report_id,
                summary.contract_status,
                summary.manifest_field_row_count,
                summary.packaging_row_count,
                summary.deferred_issue_ids.len(),
                !summary.plugin_capability_claim_allowed
                    && !summary.weighted_plugin_control_allowed
                    && !summary.plugin_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginManifestIdentityContractReceipt;
    use psionic_research::build_tassadar_post_article_plugin_manifest_identity_contract_summary;

    #[test]
    fn post_article_plugin_manifest_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_manifest_identity_contract_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginManifestIdentityContractReceipt::from_summary(&summary);

        assert_eq!(receipt.contract_status, "green");
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.operator_internal_only_posture);
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
