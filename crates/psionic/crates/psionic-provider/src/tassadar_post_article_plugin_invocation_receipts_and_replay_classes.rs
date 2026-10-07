use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub contract_status: String,
    pub receipt_identity_row_count: u32,
    pub replay_class_row_count: u32,
    pub failure_class_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub operator_internal_only_posture: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticlePluginInvocationReceiptsAndReplayClassesSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            packet_abi_version: summary.packet_abi_version.clone(),
            host_owned_runtime_api_id: summary.host_owned_runtime_api_id.clone(),
            engine_abstraction_id: summary.engine_abstraction_id.clone(),
            invocation_receipt_profile_id: summary.invocation_receipt_profile_id.clone(),
            closure_bundle_report_id: summary.closure_bundle_report_id.clone(),
            closure_bundle_report_digest: summary.closure_bundle_report_digest.clone(),
            closure_bundle_digest: summary.closure_bundle_digest.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            receipt_identity_row_count: summary.receipt_identity_row_count,
            replay_class_row_count: summary.replay_class_row_count,
            failure_class_row_count: summary.failure_class_row_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            operator_internal_only_posture: summary.operator_internal_only_posture,
            closure_bundle_bound_by_digest: summary.closure_bundle_bound_by_digest,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin invocation-receipt summary `{}` keeps contract_status={:?}, invocation_receipt_profile_id=`{}`, replay_class_rows={}, validation_rows={}, closure_bundle_digest=`{}`, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.invocation_receipt_profile_id,
                summary.replay_class_row_count,
                summary.validation_row_count,
                summary.closure_bundle_digest,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReceipt;
    use psionic_research::build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary;

    #[test]
    fn post_article_plugin_invocation_receipts_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReceipt::from_summary(
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
