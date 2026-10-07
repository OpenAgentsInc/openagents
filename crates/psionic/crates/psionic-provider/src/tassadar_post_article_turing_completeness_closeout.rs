use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleTuringCompletenessCloseoutSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessCloseoutReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub closeout_status: String,
    pub historical_tas_156_still_stands: bool,
    pub canonical_route_truth_carrier: bool,
    pub control_plane_proof_part_of_truth_carrier: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub closure_bundle_issue_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleTuringCompletenessCloseoutReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleTuringCompletenessCloseoutSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            closure_bundle_report_id: summary.closure_bundle_report_id.clone(),
            closure_bundle_report_digest: summary.closure_bundle_report_digest.clone(),
            closure_bundle_digest: summary.closure_bundle_digest.clone(),
            closeout_status: match summary.closeout_status {
                psionic_eval::TassadarPostArticleTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed => {
                    String::from("theory_green_operator_green_served_suppressed")
                }
                psionic_eval::TassadarPostArticleTuringCompletenessCloseoutStatus::Incomplete => {
                    String::from("incomplete")
                }
            },
            historical_tas_156_still_stands: summary.historical_tas_156_still_stands,
            canonical_route_truth_carrier: summary.canonical_route_truth_carrier,
            control_plane_proof_part_of_truth_carrier: summary
                .control_plane_proof_part_of_truth_carrier,
            closure_bundle_bound_by_digest: summary.closure_bundle_bound_by_digest,
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            theory_green: summary.theory_green,
            operator_green: summary.operator_green,
            served_green: summary.served_green,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            detail: format!(
                "post-article turing-completeness closeout summary `{}` keeps closeout_status={:?}, historical_tas_156_still_stands={}, canonical_route_truth_carrier={}, control_plane_truth_carrier={}, closure_bundle_digest=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.closeout_status,
                summary.historical_tas_156_still_stands,
                summary.canonical_route_truth_carrier,
                summary.control_plane_proof_part_of_truth_carrier,
                summary.closure_bundle_digest,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleTuringCompletenessCloseoutReceipt;
    use psionic_research::build_tassadar_post_article_turing_completeness_closeout_summary;

    #[test]
    fn post_article_turing_completeness_closeout_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_turing_completeness_closeout_summary().expect("summary");
        let receipt = TassadarPostArticleTuringCompletenessCloseoutReceipt::from_summary(&summary);

        assert_eq!(
            receipt.closeout_status,
            "theory_green_operator_green_served_suppressed"
        );
        assert!(receipt.historical_tas_156_still_stands);
        assert!(receipt.canonical_route_truth_carrier);
        assert!(receipt.control_plane_proof_part_of_truth_carrier);
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
        assert!(receipt.theory_green);
        assert!(receipt.operator_green);
        assert!(!receipt.served_green);
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
    }
}
