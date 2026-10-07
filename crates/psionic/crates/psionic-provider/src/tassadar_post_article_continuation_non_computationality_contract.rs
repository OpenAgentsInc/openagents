use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleContinuationNonComputationalityContractSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationNonComputationalityContractReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub contract_status: String,
    pub continuation_extends_execution_without_second_machine: bool,
    pub hidden_workflow_logic_refused: bool,
    pub continuation_expressivity_extension_blocked: bool,
    pub plugin_resume_hidden_compute_refused: bool,
    pub continuation_non_computationality_complete: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleContinuationNonComputationalityContractReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleContinuationNonComputationalityContractSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            continuation_contract_id: summary.continuation_contract_id.clone(),
            computational_model_statement_id: summary.computational_model_statement_id.clone(),
            proof_transport_boundary_id: summary.proof_transport_boundary_id.clone(),
            contract_status: match summary.contract_status {
                psionic_eval::TassadarPostArticleContinuationNonComputationalityStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleContinuationNonComputationalityStatus::Blocked => {
                    String::from("blocked")
                }
            },
            continuation_extends_execution_without_second_machine: summary
                .continuation_extends_execution_without_second_machine,
            hidden_workflow_logic_refused: summary.hidden_workflow_logic_refused,
            continuation_expressivity_extension_blocked: summary
                .continuation_expressivity_extension_blocked,
            plugin_resume_hidden_compute_refused: summary.plugin_resume_hidden_compute_refused,
            continuation_non_computationality_complete: summary
                .continuation_non_computationality_complete,
            next_stability_issue_id: summary.next_stability_issue_id.clone(),
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article continuation non-computationality summary `{}` keeps contract_status={:?}, machine_identity_id=`{}`, continuation_non_computationality_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.contract_status,
                summary.machine_identity_id,
                summary.continuation_non_computationality_complete,
                summary.next_stability_issue_id,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleContinuationNonComputationalityContractReceipt;
    use psionic_research::build_tassadar_post_article_continuation_non_computationality_contract_summary;

    #[test]
    fn post_article_continuation_non_computationality_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_continuation_non_computationality_contract_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleContinuationNonComputationalityContractReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert!(receipt.continuation_extends_execution_without_second_machine);
        assert!(receipt.hidden_workflow_logic_refused);
        assert!(receipt.continuation_expressivity_extension_blocked);
        assert!(receipt.plugin_resume_hidden_compute_refused);
        assert!(receipt.continuation_non_computationality_complete);
        assert_eq!(receipt.next_stability_issue_id, "TAS-215");
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
    }
}
