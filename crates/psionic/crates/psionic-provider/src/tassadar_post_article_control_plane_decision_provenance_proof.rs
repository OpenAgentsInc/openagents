use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleControlPlaneDecisionProvenanceProofSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleControlPlaneDecisionProvenanceProofReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub control_plane_ownership_status: String,
    pub control_plane_ownership_green: bool,
    pub replay_posture_green: bool,
    pub decision_provenance_proof_complete: bool,
    pub carrier_split_publication_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleControlPlaneDecisionProvenanceProofReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleControlPlaneDecisionProvenanceProofSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            control_plane_ownership_status: match summary.control_plane_ownership_status {
                psionic_eval::TassadarPostArticleControlPlaneOwnershipStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleControlPlaneOwnershipStatus::Blocked => {
                    String::from("blocked")
                }
            },
            control_plane_ownership_green: summary.control_plane_ownership_green,
            replay_posture_green: summary.replay_posture_green,
            decision_provenance_proof_complete: summary.decision_provenance_proof_complete,
            carrier_split_publication_complete: summary.carrier_split_publication_complete,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article control-plane summary `{}` keeps machine_identity_id=`{}`, canonical_route_id=`{}`, control_plane_ownership_green={}, replay_posture_green={}, decision_provenance_proof_complete={}, and carrier_split_publication_complete={}.",
                summary.report_id,
                summary.machine_identity_id,
                summary.canonical_route_id,
                summary.control_plane_ownership_green,
                summary.replay_posture_green,
                summary.decision_provenance_proof_complete,
                summary.carrier_split_publication_complete,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleControlPlaneDecisionProvenanceProofReceipt;
    use psionic_research::build_tassadar_post_article_control_plane_decision_provenance_proof_summary;

    #[test]
    fn control_plane_receipt_projects_summary() {
        let summary = build_tassadar_post_article_control_plane_decision_provenance_proof_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleControlPlaneDecisionProvenanceProofReceipt::from_summary(&summary);

        assert_eq!(receipt.control_plane_ownership_status, "green");
        assert_eq!(
            receipt.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            receipt.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert!(receipt.control_plane_ownership_green);
        assert!(receipt.replay_posture_green);
        assert!(receipt.decision_provenance_proof_complete);
        assert!(!receipt.carrier_split_publication_complete);
        assert_eq!(receipt.deferred_issue_ids, vec![String::from("TAS-189")]);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
