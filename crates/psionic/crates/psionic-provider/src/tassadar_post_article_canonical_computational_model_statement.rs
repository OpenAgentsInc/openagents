use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleCanonicalComputationalModelStatementSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelStatementReceipt {
    pub report_id: String,
    pub statement_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub substrate_model_id: String,
    pub statement_status: String,
    pub article_equivalent_compute_named: bool,
    pub tcm_v1_continuation_named: bool,
    pub declared_effect_boundary_named: bool,
    pub plugin_layer_scoped_above_machine: bool,
    pub proof_transport_complete: bool,
    pub proof_transport_audit_issue_id: String,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub weighted_plugin_control_part_of_model: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleCanonicalComputationalModelStatementReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleCanonicalComputationalModelStatementSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            statement_id: summary.statement_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            continuation_contract_id: summary.continuation_contract_id.clone(),
            substrate_model_id: summary.substrate_model_id.clone(),
            statement_status: match summary.statement_status {
                psionic_runtime::TassadarPostArticleCanonicalComputationalModelStatus::Green => {
                    String::from("green")
                }
                psionic_runtime::TassadarPostArticleCanonicalComputationalModelStatus::Blocked => {
                    String::from("blocked")
                }
            },
            article_equivalent_compute_named: summary.article_equivalent_compute_named,
            tcm_v1_continuation_named: summary.tcm_v1_continuation_named,
            declared_effect_boundary_named: summary.declared_effect_boundary_named,
            plugin_layer_scoped_above_machine: summary.plugin_layer_scoped_above_machine,
            proof_transport_complete: summary.proof_transport_complete,
            proof_transport_audit_issue_id: summary.proof_transport_audit_issue_id.clone(),
            next_stability_issue_id: summary.next_stability_issue_id.clone(),
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            weighted_plugin_control_part_of_model: summary.weighted_plugin_control_part_of_model,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article canonical computational-model summary `{}` keeps statement_status={:?}, machine_identity_id=`{}`, plugin_layer_scoped_above_machine={}, proof_transport_complete={}, proof_transport_audit_issue_id=`{}`, and next_stability_issue_id=`{}`.",
                summary.report_id,
                summary.statement_status,
                summary.machine_identity_id,
                summary.plugin_layer_scoped_above_machine,
                summary.proof_transport_complete,
                summary.proof_transport_audit_issue_id,
                summary.next_stability_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleCanonicalComputationalModelStatementReceipt;
    use psionic_research::build_tassadar_post_article_canonical_computational_model_statement_summary;

    #[test]
    fn post_article_canonical_computational_model_statement_receipt_projects_summary() {
        let summary = build_tassadar_post_article_canonical_computational_model_statement_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleCanonicalComputationalModelStatementReceipt::from_summary(&summary);

        assert_eq!(receipt.statement_status, "green");
        assert!(receipt.article_equivalent_compute_named);
        assert!(receipt.tcm_v1_continuation_named);
        assert!(receipt.declared_effect_boundary_named);
        assert!(receipt.plugin_layer_scoped_above_machine);
        assert!(receipt.proof_transport_complete);
        assert_eq!(receipt.proof_transport_audit_issue_id, "TAS-209");
        assert_eq!(receipt.next_stability_issue_id, "TAS-215");
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
        assert!(!receipt.weighted_plugin_control_part_of_model);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
