use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub reference_linear_proof_route_descriptor_digest: String,
    pub proof_transport_boundary_id: String,
    pub contract_status: String,
    pub carrier_binding_complete: bool,
    pub unproven_fast_routes_quarantined: bool,
    pub resumable_family_not_presented_as_direct_machine: bool,
    pub served_or_plugin_machine_overclaim_refused: bool,
    pub fast_route_legitimacy_complete: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
}

impl TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            canonical_route_descriptor_digest: summary.canonical_route_descriptor_digest.clone(),
            reference_linear_proof_route_descriptor_digest: summary
                .reference_linear_proof_route_descriptor_digest
                .clone(),
            proof_transport_boundary_id: summary.proof_transport_boundary_id.clone(),
            contract_status: match summary.contract_status {
                psionic_eval::TassadarPostArticleFastRouteLegitimacyStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleFastRouteLegitimacyStatus::Blocked => {
                    String::from("blocked")
                }
            },
            carrier_binding_complete: summary.carrier_binding_complete,
            unproven_fast_routes_quarantined: summary.unproven_fast_routes_quarantined,
            resumable_family_not_presented_as_direct_machine: summary
                .resumable_family_not_presented_as_direct_machine,
            served_or_plugin_machine_overclaim_refused: summary
                .served_or_plugin_machine_overclaim_refused,
            fast_route_legitimacy_complete: summary.fast_route_legitimacy_complete,
            next_stability_issue_id: summary.next_stability_issue_id.clone(),
            closure_bundle_issue_id: summary.closure_bundle_issue_id.clone(),
            detail: format!(
                "post-article fast-route legitimacy summary `{}` keeps contract_status={:?}, machine_identity_id=`{}`, fast_route_legitimacy_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
                summary.report_id,
                summary.contract_status,
                summary.machine_identity_id,
                summary.fast_route_legitimacy_complete,
                summary.next_stability_issue_id,
                summary.closure_bundle_issue_id,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReceipt;
    use psionic_research::build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary;

    #[test]
    fn post_article_fast_route_legitimacy_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert!(receipt.carrier_binding_complete);
        assert!(receipt.unproven_fast_routes_quarantined);
        assert!(receipt.resumable_family_not_presented_as_direct_machine);
        assert!(receipt.served_or_plugin_machine_overclaim_refused);
        assert!(receipt.fast_route_legitimacy_complete);
        assert_eq!(receipt.next_stability_issue_id, "TAS-215");
        assert_eq!(receipt.closure_bundle_issue_id, "TAS-215");
    }
}
