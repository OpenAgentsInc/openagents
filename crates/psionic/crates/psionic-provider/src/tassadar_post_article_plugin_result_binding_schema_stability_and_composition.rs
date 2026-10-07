use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub runtime_bundle_id: String,
    pub contract_status: String,
    pub binding_row_count: u32,
    pub evidence_boundary_row_count: u32,
    pub composition_row_count: u32,
    pub negative_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub result_binding_contract_green: bool,
    pub semantic_composition_closure_green: bool,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            packet_abi_version: summary.packet_abi_version.clone(),
            host_owned_runtime_api_id: summary.host_owned_runtime_api_id.clone(),
            engine_abstraction_id: summary.engine_abstraction_id.clone(),
            invocation_receipt_profile_id: summary.invocation_receipt_profile_id.clone(),
            result_binding_contract_id: summary.result_binding_contract_id.clone(),
            model_loop_return_profile_id: summary.model_loop_return_profile_id.clone(),
            runtime_bundle_id: summary.runtime_bundle_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            binding_row_count: summary.binding_row_count,
            evidence_boundary_row_count: summary.evidence_boundary_row_count,
            composition_row_count: summary.composition_row_count,
            negative_row_count: summary.negative_row_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            result_binding_contract_green: summary.result_binding_contract_green,
            semantic_composition_closure_green: summary.semantic_composition_closure_green,
            operator_internal_only_posture: summary.operator_internal_only_posture,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin result-binding summary `{}` keeps contract_status={:?}, result_binding_contract_id=`{}`, binding_rows={}, composition_rows={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.result_binding_contract_id,
                summary.binding_row_count,
                summary.composition_row_count,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionReceipt;
    use psionic_research::build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary;

    #[test]
    fn post_article_plugin_result_binding_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(receipt.packet_abi_version, "packet.v1");
        assert_eq!(
            receipt.host_owned_runtime_api_id,
            "tassadar.plugin_runtime.host_owned_api.v1"
        );
        assert_eq!(
            receipt.engine_abstraction_id,
            "tassadar.plugin_runtime.engine_abstraction.v1"
        );
        assert_eq!(
            receipt.invocation_receipt_profile_id,
            "tassadar.plugin_runtime.invocation_receipts.v1"
        );
        assert_eq!(
            receipt.result_binding_contract_id,
            "tassadar.weighted_plugin.result_binding_contract.v1"
        );
        assert_eq!(
            receipt.model_loop_return_profile_id,
            "tassadar.weighted_plugin.model_loop_return_profile.v1"
        );
        assert_eq!(receipt.binding_row_count, 6);
        assert_eq!(receipt.evidence_boundary_row_count, 4);
        assert_eq!(receipt.composition_row_count, 4);
        assert_eq!(receipt.negative_row_count, 4);
        assert_eq!(receipt.validation_row_count, 12);
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.result_binding_contract_green);
        assert!(receipt.semantic_composition_closure_green);
        assert!(receipt.operator_internal_only_posture);
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
