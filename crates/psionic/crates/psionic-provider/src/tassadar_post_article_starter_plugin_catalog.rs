use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleStarterPluginCatalogSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub closure_bundle_digest: String,
    pub eval_status: String,
    pub starter_plugin_count: u32,
    pub local_deterministic_plugin_count: u32,
    pub read_only_network_plugin_count: u32,
    pub bounded_flow_count: u32,
    pub operator_internal_only_posture: bool,
    pub public_marketplace_language_suppressed: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub next_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleStarterPluginCatalogReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleStarterPluginCatalogSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            closure_bundle_digest: summary.closure_bundle_digest.clone(),
            eval_status: format!("{:?}", summary.eval_status).to_lowercase(),
            starter_plugin_count: summary.starter_plugin_count,
            local_deterministic_plugin_count: summary.local_deterministic_plugin_count,
            read_only_network_plugin_count: summary.read_only_network_plugin_count,
            bounded_flow_count: summary.bounded_flow_count,
            operator_internal_only_posture: summary.operator_internal_only_posture,
            public_marketplace_language_suppressed: summary
                .public_marketplace_language_suppressed,
            closure_bundle_bound_by_digest: summary.closure_bundle_bound_by_digest,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            next_issue_id: summary.next_issue_id.clone(),
            detail: format!(
                "starter plugin catalog summary `{}` keeps eval_status={:?}, starter_plugin_count={}, bounded_flow_count={}, closure_bundle_digest=`{}`, and next_issue_id=`{}`.",
                summary.report_id,
                summary.eval_status,
                summary.starter_plugin_count,
                summary.bounded_flow_count,
                summary.closure_bundle_digest,
                summary.next_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleStarterPluginCatalogReceipt;
    use psionic_research::build_tassadar_post_article_starter_plugin_catalog_summary;

    #[test]
    fn post_article_starter_plugin_catalog_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_starter_plugin_catalog_summary().expect("summary");
        let receipt = TassadarPostArticleStarterPluginCatalogReceipt::from_summary(&summary);

        assert_eq!(receipt.eval_status, "green");
        assert_eq!(receipt.starter_plugin_count, 6);
        assert_eq!(receipt.local_deterministic_plugin_count, 5);
        assert_eq!(receipt.read_only_network_plugin_count, 1);
        assert_eq!(receipt.bounded_flow_count, 2);
        assert!(receipt.operator_internal_only_posture);
        assert!(receipt.public_marketplace_language_suppressed);
        assert!(receipt.closure_bundle_bound_by_digest);
        assert!(receipt.plugin_capability_claim_allowed);
        assert!(receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert_eq!(receipt.next_issue_id, "TAS-217");
    }
}
