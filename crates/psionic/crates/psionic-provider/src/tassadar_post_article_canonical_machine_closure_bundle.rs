use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleCanonicalMachineClosureBundleSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleReceipt {
    pub report_id: String,
    pub closure_bundle_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub closure_bundle_digest: String,
    pub bundle_status: String,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub proof_and_audit_classification_complete: bool,
    pub machine_subject_complete: bool,
    pub control_execution_and_continuation_bound: bool,
    pub hidden_state_and_observer_model_bound: bool,
    pub portability_and_minimality_bound: bool,
    pub anti_drift_closeout_inherited: bool,
    pub terminal_claims_must_reference_bundle_digest: bool,
    pub plugin_claims_must_reference_bundle_digest: bool,
    pub platform_claims_must_reference_bundle_digest: bool,
    pub closure_bundle_issue_id: String,
    pub next_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleCanonicalMachineClosureBundleReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleCanonicalMachineClosureBundleSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            closure_bundle_id: summary.closure_bundle_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            continuation_contract_id: summary.continuation_contract_id.clone(),
            closure_bundle_digest: summary.closure_bundle_digest.clone(),
            bundle_status: format!("{:?}", summary.bundle_status).to_lowercase(),
            supporting_material_row_count: summary.supporting_material_row_count,
            dependency_row_count: summary.dependency_row_count,
            invalidation_row_count: summary.invalidation_row_count,
            validation_row_count: summary.validation_row_count,
            proof_and_audit_classification_complete: summary.proof_and_audit_classification_complete,
            machine_subject_complete: summary.machine_subject_complete,
            control_execution_and_continuation_bound: summary
                .control_execution_and_continuation_bound,
            hidden_state_and_observer_model_bound: summary.hidden_state_and_observer_model_bound,
            portability_and_minimality_bound: summary.portability_and_minimality_bound,
            anti_drift_closeout_inherited: summary.anti_drift_closeout_inherited,
            terminal_claims_must_reference_bundle_digest: summary
                .terminal_claims_must_reference_bundle_digest,
            plugin_claims_must_reference_bundle_digest: summary
                .plugin_claims_must_reference_bundle_digest,
            platform_claims_must_reference_bundle_digest: summary
                .platform_claims_must_reference_bundle_digest,
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            next_issue_id: summary.next_issue_id.clone(),
            detail: format!(
                "post-article canonical machine closure bundle summary `{}` keeps bundle_status={:?}, machine_identity_id=`{}`, closure_bundle_digest=`{}`, and next_issue_id=`{}`.",
                summary.report_id,
                summary.bundle_status,
                summary.machine_identity_id,
                summary.closure_bundle_digest,
                summary.next_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleCanonicalMachineClosureBundleReceipt;
    use psionic_research::build_tassadar_post_article_canonical_machine_closure_bundle_summary;

    #[test]
    fn post_article_canonical_machine_closure_bundle_receipt_projects_summary() {
        let summary = build_tassadar_post_article_canonical_machine_closure_bundle_summary()
            .expect("summary");
        let receipt =
            TassadarPostArticleCanonicalMachineClosureBundleReceipt::from_summary(&summary);

        assert_eq!(receipt.bundle_status, "green");
        assert_eq!(
            receipt.closure_bundle_id,
            "tassadar.post_article.canonical_machine.closure_bundle.v1"
        );
        assert!(receipt.proof_and_audit_classification_complete);
        assert!(receipt.machine_subject_complete);
        assert!(receipt.control_execution_and_continuation_bound);
        assert!(receipt.hidden_state_and_observer_model_bound);
        assert!(receipt.portability_and_minimality_bound);
        assert!(receipt.anti_drift_closeout_inherited);
        assert!(receipt.terminal_claims_must_reference_bundle_digest);
        assert!(receipt.plugin_claims_must_reference_bundle_digest);
        assert!(receipt.platform_claims_must_reference_bundle_digest);
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
        assert_eq!(receipt.next_issue_id, "TAS-217");
        assert!(!receipt.closure_bundle_digest.is_empty());
    }
}
