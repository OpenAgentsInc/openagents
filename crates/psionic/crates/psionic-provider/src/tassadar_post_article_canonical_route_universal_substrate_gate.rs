use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstrateGateReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub gate_status: String,
    pub bounded_universality_story_carried: bool,
    pub proof_rebinding_complete: bool,
    pub witness_suite_reissued: bool,
    pub deferred_issue_ids: Vec<String>,
    pub universal_substrate_gate_allowed: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleCanonicalRouteUniversalSubstrateGateReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            gate_status: format!("{:?}", summary.gate_status).to_lowercase(),
            bounded_universality_story_carried: summary.bounded_universality_story_carried,
            proof_rebinding_complete: summary.proof_rebinding_complete,
            witness_suite_reissued: summary.witness_suite_reissued,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            universal_substrate_gate_allowed: summary.universal_substrate_gate_allowed,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article canonical-route universal-substrate gate receipt keeps machine_identity_id=`{}`, canonical_model_id=`{}`, canonical_route_id=`{}`, gate_status={:?}, bounded_universality_story_carried={}, and deferred_issue_ids={}.",
                summary.machine_identity_id,
                summary.canonical_model_id,
                summary.canonical_route_id,
                summary.gate_status,
                summary.bounded_universality_story_carried,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleCanonicalRouteUniversalSubstrateGateReceipt;
    use psionic_research::build_tassadar_post_article_canonical_route_universal_substrate_gate_summary;

    #[test]
    fn canonical_route_universal_substrate_gate_receipt_projects_summary() {
        let summary = build_tassadar_post_article_canonical_route_universal_substrate_gate_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleCanonicalRouteUniversalSubstrateGateReceipt::from_summary(&summary);

        assert_eq!(receipt.gate_status, "green");
        assert!(receipt.bounded_universality_story_carried);
        assert!(receipt.proof_rebinding_complete);
        assert!(receipt.witness_suite_reissued);
        assert_eq!(receipt.deferred_issue_ids, vec![String::from("TAS-193")]);
        assert!(receipt.universal_substrate_gate_allowed);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
