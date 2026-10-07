use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceAndServedConformanceReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub contract_status: String,
    pub lower_plane_truth_row_count: u32,
    pub served_deviation_row_count: u32,
    pub downward_non_influence_complete: bool,
    pub served_conformance_envelope_complete: bool,
    pub lower_plane_truth_rewrite_refused: bool,
    pub served_posture_narrower_than_operator_truth: bool,
    pub served_posture_fail_closed: bool,
    pub plugin_or_served_overclaim_refused: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleDownwardNonInfluenceAndServedConformanceReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleDownwardNonInfluenceAndServedConformanceSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            current_served_internal_compute_profile_id: summary
                .current_served_internal_compute_profile_id
                .clone(),
            contract_status: match summary.contract_status {
                psionic_eval::TassadarPostArticleDownwardNonInfluenceStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleDownwardNonInfluenceStatus::Blocked => {
                    String::from("blocked")
                }
            },
            lower_plane_truth_row_count: summary.lower_plane_truth_row_count,
            served_deviation_row_count: summary.served_deviation_row_count,
            downward_non_influence_complete: summary.downward_non_influence_complete,
            served_conformance_envelope_complete: summary.served_conformance_envelope_complete,
            lower_plane_truth_rewrite_refused: summary.lower_plane_truth_rewrite_refused,
            served_posture_narrower_than_operator_truth: summary
                .served_posture_narrower_than_operator_truth,
            served_posture_fail_closed: summary.served_posture_fail_closed,
            plugin_or_served_overclaim_refused: summary.plugin_or_served_overclaim_refused,
            next_stability_issue_id: summary.next_stability_issue_id.clone(),
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article downward non-influence summary `{}` keeps contract_status={:?}, machine_identity_id=`{}`, served_profile_id=`{}`, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.contract_status,
                summary.machine_identity_id,
                summary.current_served_internal_compute_profile_id,
                summary.next_stability_issue_id,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleDownwardNonInfluenceAndServedConformanceReceipt;
    use psionic_research::build_tassadar_post_article_downward_non_influence_and_served_conformance_summary;

    #[test]
    fn post_article_downward_non_influence_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleDownwardNonInfluenceAndServedConformanceReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(
            receipt.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(receipt.downward_non_influence_complete);
        assert!(receipt.served_conformance_envelope_complete);
        assert!(receipt.lower_plane_truth_rewrite_refused);
        assert!(receipt.served_posture_narrower_than_operator_truth);
        assert!(receipt.served_posture_fail_closed);
        assert!(receipt.plugin_or_served_overclaim_refused);
        assert_eq!(receipt.next_stability_issue_id, "TAS-215");
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
    }
}
