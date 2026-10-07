use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleUniversalityWitnessSuiteReissueSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityWitnessSuiteReissueReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub witness_suite_status: String,
    pub proof_rebinding_complete: bool,
    pub witness_suite_reissued: bool,
    pub exact_family_count: u32,
    pub refusal_boundary_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub universal_substrate_gate_allowed: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleUniversalityWitnessSuiteReissueReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleUniversalityWitnessSuiteReissueSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            witness_suite_status: match summary.witness_suite_status {
                psionic_eval::TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleUniversalityWitnessSuiteReissueStatus::Blocked => {
                    String::from("blocked")
                }
            },
            proof_rebinding_complete: summary.proof_rebinding_complete,
            witness_suite_reissued: summary.witness_suite_reissued,
            exact_family_count: summary.exact_family_count,
            refusal_boundary_count: summary.refusal_boundary_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            universal_substrate_gate_allowed: summary.universal_substrate_gate_allowed,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article universality witness-suite reissue summary `{}` keeps machine_identity_id=`{}`, canonical_model_id=`{}`, canonical_route_id=`{}`, exact_family_count={}, refusal_boundary_count={}, and witness_suite_reissued={}.",
                summary.report_id,
                summary.machine_identity_id,
                summary.canonical_model_id,
                summary.canonical_route_id,
                summary.exact_family_count,
                summary.refusal_boundary_count,
                summary.witness_suite_reissued,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleUniversalityWitnessSuiteReissueReceipt;
    use psionic_research::build_tassadar_post_article_universality_witness_suite_reissue_summary;

    #[test]
    fn witness_suite_reissue_receipt_projects_summary() {
        let summary = build_tassadar_post_article_universality_witness_suite_reissue_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleUniversalityWitnessSuiteReissueReceipt::from_summary(&summary);

        assert_eq!(receipt.witness_suite_status, "green");
        assert!(receipt.proof_rebinding_complete);
        assert!(receipt.witness_suite_reissued);
        assert_eq!(receipt.exact_family_count, 5);
        assert_eq!(receipt.refusal_boundary_count, 2);
        assert_eq!(receipt.deferred_issue_ids, vec![String::from("TAS-192")]);
        assert!(!receipt.universal_substrate_gate_allowed);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
