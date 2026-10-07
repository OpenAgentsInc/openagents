use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleExecutionSemanticsProofTransportAuditSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsProofTransportAuditReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub audit_status: String,
    pub proof_transport_issue_id: String,
    pub proof_transport_complete: bool,
    pub plugin_execution_transport_bound: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleExecutionSemanticsProofTransportAuditReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleExecutionSemanticsProofTransportAuditSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            continuation_contract_id: summary.continuation_contract_id.clone(),
            computational_model_statement_id: summary.computational_model_statement_id.clone(),
            proof_transport_boundary_id: summary.proof_transport_boundary_id.clone(),
            audit_status: match summary.audit_status {
                psionic_eval::TassadarPostArticleExecutionSemanticsProofTransportStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleExecutionSemanticsProofTransportStatus::Blocked => {
                    String::from("blocked")
                }
            },
            proof_transport_issue_id: summary.proof_transport_issue_id.clone(),
            proof_transport_complete: summary.proof_transport_complete,
            plugin_execution_transport_bound: summary.plugin_execution_transport_bound,
            next_stability_issue_id: summary.next_stability_issue_id.clone(),
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article execution-semantics proof-transport summary `{}` keeps audit_status={:?}, machine_identity_id=`{}`, plugin_execution_transport_bound={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.audit_status,
                summary.machine_identity_id,
                summary.plugin_execution_transport_bound,
                summary.next_stability_issue_id,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleExecutionSemanticsProofTransportAuditReceipt;
    use psionic_research::build_tassadar_post_article_execution_semantics_proof_transport_audit_summary;

    #[test]
    fn post_article_execution_semantics_proof_transport_audit_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_execution_semantics_proof_transport_audit_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleExecutionSemanticsProofTransportAuditReceipt::from_summary(&summary);

        assert_eq!(receipt.audit_status, "green");
        assert_eq!(receipt.proof_transport_issue_id, "TAS-209");
        assert!(receipt.proof_transport_complete);
        assert!(receipt.plugin_execution_transport_bound);
        assert_eq!(receipt.next_stability_issue_id, "TAS-215");
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
    }
}
