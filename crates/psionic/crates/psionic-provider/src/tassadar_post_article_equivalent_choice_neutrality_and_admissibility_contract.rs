use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub control_plane_equivalent_choice_relation_id: String,
    pub admissibility_contract_id: String,
    pub contract_status: String,
    pub equivalent_choice_class_row_count: u32,
    pub case_binding_row_count: u32,
    pub equivalent_choice_neutrality_complete: bool,
    pub admissibility_narrowing_receipt_visible: bool,
    pub hidden_ordering_or_ranking_quarantined: bool,
    pub latency_cost_and_soft_failure_channels_blocked: bool,
    pub served_or_plugin_equivalence_overclaim_refused: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            control_plane_equivalent_choice_relation_id: summary
                .control_plane_equivalent_choice_relation_id
                .clone(),
            admissibility_contract_id: summary.admissibility_contract_id.clone(),
            contract_status: match summary.contract_status {
                psionic_eval::TassadarPostArticleEquivalentChoiceNeutralityStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleEquivalentChoiceNeutralityStatus::Blocked => {
                    String::from("blocked")
                }
            },
            equivalent_choice_class_row_count: summary.equivalent_choice_class_row_count,
            case_binding_row_count: summary.case_binding_row_count,
            equivalent_choice_neutrality_complete: summary.equivalent_choice_neutrality_complete,
            admissibility_narrowing_receipt_visible: summary
                .admissibility_narrowing_receipt_visible,
            hidden_ordering_or_ranking_quarantined: summary
                .hidden_ordering_or_ranking_quarantined,
            latency_cost_and_soft_failure_channels_blocked: summary
                .latency_cost_and_soft_failure_channels_blocked,
            served_or_plugin_equivalence_overclaim_refused: summary
                .served_or_plugin_equivalence_overclaim_refused,
            next_stability_issue_id: summary.next_stability_issue_id.clone(),
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article equivalent-choice neutrality summary `{}` keeps contract_status={:?}, machine_identity_id=`{}`, equivalent_choice_neutrality_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.contract_status,
                summary.machine_identity_id,
                summary.equivalent_choice_neutrality_complete,
                summary.next_stability_issue_id,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReceipt;
    use psionic_research::build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary;

    #[test]
    fn post_article_equivalent_choice_neutrality_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(
            receipt.control_plane_equivalent_choice_relation_id,
            "singleton_exact_control_trace.v1"
        );
        assert!(receipt.equivalent_choice_neutrality_complete);
        assert!(receipt.admissibility_narrowing_receipt_visible);
        assert!(receipt.hidden_ordering_or_ranking_quarantined);
        assert!(receipt.latency_cost_and_soft_failure_channels_blocked);
        assert!(receipt.served_or_plugin_equivalence_overclaim_refused);
        assert_eq!(receipt.next_stability_issue_id, "TAS-215");
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
    }
}
