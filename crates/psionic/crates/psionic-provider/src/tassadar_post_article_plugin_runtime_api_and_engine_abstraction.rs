use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginRuntimeApiAndEngineAbstractionSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub contract_status: String,
    pub runtime_api_row_count: u32,
    pub engine_row_count: u32,
    pub bound_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticlePluginRuntimeApiAndEngineAbstractionSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            packet_abi_version: summary.packet_abi_version.clone(),
            host_owned_runtime_api_id: summary.host_owned_runtime_api_id.clone(),
            engine_abstraction_id: summary.engine_abstraction_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            runtime_api_row_count: summary.runtime_api_row_count,
            engine_row_count: summary.engine_row_count,
            bound_row_count: summary.bound_row_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            operator_internal_only_posture: summary.operator_internal_only_posture,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin runtime API summary `{}` keeps contract_status={:?}, host_owned_runtime_api_id=`{}`, engine_abstraction_id=`{}`, validation_rows={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.host_owned_runtime_api_id,
                summary.engine_abstraction_id,
                summary.validation_row_count,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReceipt;
    use psionic_research::build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary;

    #[test]
    fn post_article_plugin_runtime_api_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReceipt::from_summary(&summary);

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(
            receipt.host_owned_runtime_api_id,
            "tassadar.plugin_runtime.host_owned_api.v1"
        );
        assert_eq!(
            receipt.engine_abstraction_id,
            "tassadar.plugin_runtime.engine_abstraction.v1"
        );
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.operator_internal_only_posture);
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
