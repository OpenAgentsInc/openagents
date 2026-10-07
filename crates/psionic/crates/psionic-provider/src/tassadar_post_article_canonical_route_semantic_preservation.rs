use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleCanonicalRouteSemanticPreservationSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteSemanticPreservationReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub semantic_preservation_status: String,
    pub semantic_preservation_audit_green: bool,
    pub state_ownership_green: bool,
    pub control_ownership_rule_green: bool,
    pub semantic_preservation_green: bool,
    pub decision_provenance_proof_complete: bool,
    pub carrier_split_publication_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleCanonicalRouteSemanticPreservationReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleCanonicalRouteSemanticPreservationSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            semantic_preservation_status: match summary.semantic_preservation_status {
                psionic_eval::TassadarPostArticleCanonicalRouteSemanticPreservationStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleCanonicalRouteSemanticPreservationStatus::Blocked => {
                    String::from("blocked")
                }
            },
            semantic_preservation_audit_green: summary.semantic_preservation_audit_green,
            state_ownership_green: summary.state_ownership_green,
            control_ownership_rule_green: summary.control_ownership_rule_green,
            semantic_preservation_green: summary.semantic_preservation_green,
            decision_provenance_proof_complete: summary.decision_provenance_proof_complete,
            carrier_split_publication_complete: summary.carrier_split_publication_complete,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article semantic-preservation summary `{}` keeps machine_identity_id=`{}`, canonical_route_id=`{}`, semantic_preservation_audit_green={}, state_ownership_green={}, control_ownership_rule_green={}, semantic_preservation_green={}, decision_provenance_proof_complete={}, and carrier_split_publication_complete={}.",
                summary.report_id,
                summary.machine_identity_id,
                summary.canonical_route_id,
                summary.semantic_preservation_audit_green,
                summary.state_ownership_green,
                summary.control_ownership_rule_green,
                summary.semantic_preservation_green,
                summary.decision_provenance_proof_complete,
                summary.carrier_split_publication_complete,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleCanonicalRouteSemanticPreservationReceipt;
    use psionic_research::build_tassadar_post_article_canonical_route_semantic_preservation_summary;

    #[test]
    fn semantic_preservation_receipt_projects_summary() {
        let summary = build_tassadar_post_article_canonical_route_semantic_preservation_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleCanonicalRouteSemanticPreservationReceipt::from_summary(&summary);

        assert_eq!(receipt.semantic_preservation_status, "green");
        assert_eq!(
            receipt.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            receipt.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert!(receipt.semantic_preservation_audit_green);
        assert!(receipt.state_ownership_green);
        assert!(receipt.control_ownership_rule_green);
        assert!(receipt.semantic_preservation_green);
        assert!(!receipt.decision_provenance_proof_complete);
        assert!(!receipt.carrier_split_publication_complete);
        assert_eq!(
            receipt.deferred_issue_ids,
            vec![String::from("TAS-188A"), String::from("TAS-189")]
        );
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
