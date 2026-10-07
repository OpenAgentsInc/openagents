use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleUniversalMachineProofRebindingSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalMachineProofRebindingReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub proof_transport_boundary_id: String,
    pub proof_rebinding_status: String,
    pub proof_transport_audit_complete: bool,
    pub proof_rebinding_complete: bool,
    pub rebound_encoding_ids: Vec<String>,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleUniversalMachineProofRebindingReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleUniversalMachineProofRebindingSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_weight_artifact_id: summary.canonical_weight_artifact_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            proof_transport_boundary_id: summary.proof_transport_boundary_id.clone(),
            proof_rebinding_status: match summary.proof_rebinding_status {
                psionic_eval::TassadarPostArticleUniversalMachineProofRebindingStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleUniversalMachineProofRebindingStatus::Blocked => {
                    String::from("blocked")
                }
            },
            proof_transport_audit_complete: summary.proof_transport_audit_complete,
            proof_rebinding_complete: summary.proof_rebinding_complete,
            rebound_encoding_ids: summary.rebound_encoding_ids.clone(),
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article universal-machine proof rebinding summary `{}` keeps machine_identity_id=`{}`, canonical_model_id=`{}`, canonical_route_id=`{}`, rebound_encoding_ids={}, proof_transport_audit_complete={}, and proof_rebinding_complete={}.",
                summary.report_id,
                summary.machine_identity_id,
                summary.canonical_model_id,
                summary.canonical_route_id,
                summary.rebound_encoding_ids.len(),
                summary.proof_transport_audit_complete,
                summary.proof_rebinding_complete,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleUniversalMachineProofRebindingReceipt;
    use psionic_research::build_tassadar_post_article_universal_machine_proof_rebinding_summary;

    #[test]
    fn proof_rebinding_receipt_projects_summary() {
        let summary = build_tassadar_post_article_universal_machine_proof_rebinding_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleUniversalMachineProofRebindingReceipt::from_summary(&summary);

        assert_eq!(receipt.proof_rebinding_status, "green");
        assert_eq!(receipt.rebound_encoding_ids.len(), 2);
        assert!(receipt.proof_transport_audit_complete);
        assert!(receipt.proof_rebinding_complete);
        assert_eq!(receipt.deferred_issue_ids, vec![String::from("TAS-191")]);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
