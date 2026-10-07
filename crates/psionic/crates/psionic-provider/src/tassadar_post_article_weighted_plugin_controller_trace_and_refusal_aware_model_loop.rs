use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub control_trace_contract_id: String,
    pub control_trace_profile_id: String,
    pub determinism_profile_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub runtime_bundle_id: String,
    pub contract_status: String,
    pub controller_case_row_count: u32,
    pub control_trace_row_count: u32,
    pub host_negative_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub control_trace_contract_green: bool,
    pub determinism_profile_explicit: bool,
    pub typed_refusal_loop_closed: bool,
    pub host_not_planner_green: bool,
    pub adversarial_negative_rows_green: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopSummary,
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
            control_trace_contract_id: summary.control_trace_contract_id.clone(),
            control_trace_profile_id: summary.control_trace_profile_id.clone(),
            determinism_profile_id: summary.determinism_profile_id.clone(),
            closure_bundle_report_id: summary.closure_bundle_report_id.clone(),
            closure_bundle_report_digest: summary.closure_bundle_report_digest.clone(),
            closure_bundle_digest: summary.closure_bundle_digest.clone(),
            runtime_bundle_id: summary.runtime_bundle_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            controller_case_row_count: summary.controller_case_row_count,
            control_trace_row_count: summary.control_trace_row_count,
            host_negative_row_count: summary.host_negative_row_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            control_trace_contract_green: summary.control_trace_contract_green,
            determinism_profile_explicit: summary.determinism_profile_explicit,
            typed_refusal_loop_closed: summary.typed_refusal_loop_closed,
            host_not_planner_green: summary.host_not_planner_green,
            adversarial_negative_rows_green: summary.adversarial_negative_rows_green,
            closure_bundle_bound_by_digest: summary.closure_bundle_bound_by_digest,
            operator_internal_only_posture: summary.operator_internal_only_posture,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary
                .arbitrary_software_capability_allowed,
            detail: format!(
                "post-article weighted plugin controller summary `{}` keeps contract_status={:?}, control_trace_contract_id=`{}`, control_trace_rows={}, validation_rows={}, closure_bundle_digest=`{}`, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.control_trace_contract_id,
                summary.control_trace_row_count,
                summary.validation_row_count,
                summary.closure_bundle_digest,
                summary.weighted_plugin_control_allowed,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReceipt;
    use psionic_research::build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary;

    #[test]
    fn post_article_weighted_plugin_controller_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(receipt.controller_case_row_count, 6);
        assert_eq!(receipt.control_trace_row_count, 46);
        assert_eq!(receipt.host_negative_row_count, 10);
        assert_eq!(receipt.validation_row_count, 10);
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.control_trace_contract_green);
        assert!(receipt.determinism_profile_explicit);
        assert!(receipt.typed_refusal_loop_closed);
        assert!(receipt.host_not_planner_green);
        assert!(receipt.adversarial_negative_rows_green);
        assert!(receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
