use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub computational_model_statement_id: String,
    pub control_trace_contract_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub contract_status: String,
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
}

impl TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateSummary,
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
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            trust_tier_row_count: summary.trust_tier_row_count,
            promotion_row_count: summary.promotion_row_count,
            publication_posture_row_count: summary.publication_posture_row_count,
            observer_row_count: summary.observer_row_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            trust_tier_gate_green: summary.trust_tier_gate_green,
            promotion_receipts_explicit: summary.promotion_receipts_explicit,
            publication_posture_explicit: summary.publication_posture_explicit,
            observer_rights_explicit: summary.observer_rights_explicit,
            validator_hooks_explicit: summary.validator_hooks_explicit,
            accepted_outcome_hooks_explicit: summary.accepted_outcome_hooks_explicit,
            operator_internal_only_posture: summary.operator_internal_only_posture,
            profile_specific_named_routes_explicit: summary.profile_specific_named_routes_explicit,
            broader_publication_refused: summary.broader_publication_refused,
            closure_bundle_bound_by_digest: summary.closure_bundle_bound_by_digest,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin authority summary `{}` keeps contract_status={:?}, control_trace_contract_id=`{}`, trust_tier_rows={}, publication_posture_rows={}, closure_bundle_digest=`{}`, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.control_trace_contract_id,
                summary.trust_tier_row_count,
                summary.publication_posture_row_count,
                summary.closure_bundle_digest,
                summary.weighted_plugin_control_allowed,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReceipt;
    use psionic_research::build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary;

    #[test]
    fn post_article_plugin_authority_promotion_publication_trust_tier_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginAuthorityPromotionPublicationAndTrustTierGateReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(
            receipt.control_trace_contract_id,
            "tassadar.weighted_plugin.controller_trace_contract.v1"
        );
        assert_eq!(receipt.trust_tier_row_count, 5);
        assert_eq!(receipt.promotion_row_count, 5);
        assert_eq!(receipt.publication_posture_row_count, 6);
        assert_eq!(receipt.observer_row_count, 4);
        assert_eq!(receipt.validation_row_count, 10);
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.trust_tier_gate_green);
        assert!(receipt.promotion_receipts_explicit);
        assert!(receipt.publication_posture_explicit);
        assert!(receipt.observer_rights_explicit);
        assert!(receipt.validator_hooks_explicit);
        assert!(receipt.accepted_outcome_hooks_explicit);
        assert!(receipt.operator_internal_only_posture);
        assert!(receipt.profile_specific_named_routes_explicit);
        assert!(receipt.broader_publication_refused);
        assert!(receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
