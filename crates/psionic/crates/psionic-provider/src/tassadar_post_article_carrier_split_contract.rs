use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleCarrierSplitContractSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCarrierSplitContractReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub direct_carrier_id: String,
    pub resumable_carrier_id: String,
    pub reserved_capability_plane_id: String,
    pub carrier_split_status: String,
    pub carrier_split_publication_complete: bool,
    pub carrier_collapse_refused: bool,
    pub reserved_capability_plane_explicit: bool,
    pub decision_provenance_proof_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleCarrierSplitContractReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleCarrierSplitContractSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            direct_carrier_id: summary.direct_carrier_id.clone(),
            resumable_carrier_id: summary.resumable_carrier_id.clone(),
            reserved_capability_plane_id: summary.reserved_capability_plane_id.clone(),
            carrier_split_status: match summary.carrier_split_status {
                psionic_eval::TassadarPostArticleCarrierSplitStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleCarrierSplitStatus::Blocked => {
                    String::from("blocked")
                }
            },
            carrier_split_publication_complete: summary.carrier_split_publication_complete,
            carrier_collapse_refused: summary.carrier_collapse_refused,
            reserved_capability_plane_explicit: summary.reserved_capability_plane_explicit,
            decision_provenance_proof_complete: summary.decision_provenance_proof_complete,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article carrier-split summary `{}` keeps direct_carrier_id=`{}`, resumable_carrier_id=`{}`, reserved_capability_plane_id=`{}`, carrier_split_publication_complete={}, carrier_collapse_refused={}, and decision_provenance_proof_complete={}.",
                summary.report_id,
                summary.direct_carrier_id,
                summary.resumable_carrier_id,
                summary.reserved_capability_plane_id,
                summary.carrier_split_publication_complete,
                summary.carrier_collapse_refused,
                summary.decision_provenance_proof_complete,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleCarrierSplitContractReceipt;
    use psionic_research::build_tassadar_post_article_carrier_split_contract_summary;

    #[test]
    fn carrier_split_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_carrier_split_contract_summary().expect("summary");
        let receipt = TassadarPostArticleCarrierSplitContractReceipt::from_summary(&summary);

        assert_eq!(receipt.carrier_split_status, "green");
        assert!(receipt.carrier_split_publication_complete);
        assert!(receipt.carrier_collapse_refused);
        assert!(receipt.reserved_capability_plane_explicit);
        assert!(receipt.decision_provenance_proof_complete);
        assert_eq!(receipt.deferred_issue_ids, vec![String::from("TAS-190")]);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
