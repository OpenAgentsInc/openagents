use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleBoundedWeightedPluginPlatformCloseoutSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBoundedWeightedPluginPlatformCloseoutReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub control_trace_contract_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub closeout_status: String,
    pub operator_internal_only_posture: bool,
    pub served_plugin_envelope_published: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub closure_bundle_issue_id: String,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleBoundedWeightedPluginPlatformCloseoutReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleBoundedWeightedPluginPlatformCloseoutSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            computational_model_statement_id: summary.computational_model_statement_id.clone(),
            control_trace_contract_id: summary.control_trace_contract_id.clone(),
            closure_bundle_report_id: summary.closure_bundle_report_id.clone(),
            closure_bundle_report_digest: summary.closure_bundle_report_digest.clone(),
            closure_bundle_digest: summary.closure_bundle_digest.clone(),
            closeout_status: match summary.closeout_status {
                psionic_eval::TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus::OperatorGreenServedSuppressed => {
                    String::from("operator_green_served_suppressed")
                }
                psionic_eval::TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus::Incomplete => {
                    String::from("incomplete")
                }
            },
            operator_internal_only_posture: summary.operator_internal_only_posture,
            served_plugin_envelope_published: summary.served_plugin_envelope_published,
            closure_bundle_bound_by_digest: summary.closure_bundle_bound_by_digest,
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article bounded weighted plugin-platform closeout summary `{}` keeps closeout_status={:?}, control_trace_contract_id=`{}`, operator_internal_only_posture={}, closure_bundle_digest=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.closeout_status,
                summary.control_trace_contract_id,
                summary.operator_internal_only_posture,
                summary.closure_bundle_digest,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleBoundedWeightedPluginPlatformCloseoutReceipt;
    use psionic_research::build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_summary;

    #[test]
    fn post_article_bounded_weighted_plugin_platform_closeout_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleBoundedWeightedPluginPlatformCloseoutReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.closeout_status, "operator_green_served_suppressed");
        assert!(receipt.operator_internal_only_posture);
        assert!(!receipt.served_plugin_envelope_published);
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
        assert!(receipt.rebase_claim_allowed);
        assert!(receipt.plugin_capability_claim_allowed);
        assert!(receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
