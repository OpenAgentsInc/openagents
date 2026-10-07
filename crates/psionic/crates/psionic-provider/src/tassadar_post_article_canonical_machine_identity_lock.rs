use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleCanonicalMachineIdentityLockSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineIdentityLockReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub canonical_weight_bundle_digest: String,
    pub carrier_class_id: String,
    pub canonical_machine_lock_contract_id: String,
    pub lock_status: String,
    pub one_canonical_machine_named: bool,
    pub mixed_carrier_evidence_bundle_refused: bool,
    pub legacy_projection_binding_complete: bool,
    pub closure_bundle_issue_id: String,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleCanonicalMachineIdentityLockReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleCanonicalMachineIdentityLockSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            canonical_weight_bundle_digest: summary.canonical_weight_bundle_digest.clone(),
            carrier_class_id: summary.carrier_class_id.clone(),
            canonical_machine_lock_contract_id: summary
                .canonical_machine_lock_contract_id
                .clone(),
            lock_status: match summary.lock_status {
                psionic_eval::TassadarPostArticleCanonicalMachineIdentityLockStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleCanonicalMachineIdentityLockStatus::Blocked => {
                    String::from("blocked")
                }
            },
            one_canonical_machine_named: summary.one_canonical_machine_named,
            mixed_carrier_evidence_bundle_refused: summary
                .mixed_carrier_evidence_bundle_refused,
            legacy_projection_binding_complete: summary.legacy_projection_binding_complete,
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article canonical machine identity lock summary `{}` keeps lock_status={:?}, carrier_class_id=`{}`, mixed_carrier_evidence_bundle_refused={}, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.lock_status,
                summary.carrier_class_id,
                summary.mixed_carrier_evidence_bundle_refused,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleCanonicalMachineIdentityLockReceipt;
    use psionic_research::build_tassadar_post_article_canonical_machine_identity_lock_summary;

    #[test]
    fn post_article_canonical_machine_identity_lock_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_canonical_machine_identity_lock_summary().expect("summary");
        let receipt =
            TassadarPostArticleCanonicalMachineIdentityLockReceipt::from_summary(&summary);

        assert_eq!(receipt.lock_status, "green");
        assert!(receipt.one_canonical_machine_named);
        assert!(receipt.mixed_carrier_evidence_bundle_refused);
        assert!(receipt.legacy_projection_binding_complete);
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
        assert!(receipt.rebase_claim_allowed);
        assert!(receipt.plugin_capability_claim_allowed);
        assert!(receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
