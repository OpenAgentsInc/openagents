use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilitySummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub world_mount_envelope_compiler_id: String,
    pub admissibility_contract_id: String,
    pub contract_status: String,
    pub candidate_set_row_count: u32,
    pub equivalent_choice_row_count: u32,
    pub envelope_row_count: u32,
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

impl TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilitySummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            packet_abi_version: summary.packet_abi_version.clone(),
            host_owned_runtime_api_id: summary.host_owned_runtime_api_id.clone(),
            engine_abstraction_id: summary.engine_abstraction_id.clone(),
            invocation_receipt_profile_id: summary.invocation_receipt_profile_id.clone(),
            world_mount_envelope_compiler_id: summary.world_mount_envelope_compiler_id.clone(),
            admissibility_contract_id: summary.admissibility_contract_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            candidate_set_row_count: summary.candidate_set_row_count,
            equivalent_choice_row_count: summary.equivalent_choice_row_count,
            envelope_row_count: summary.envelope_row_count,
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
                "post-article plugin world-mount admissibility summary `{}` keeps contract_status={:?}, world_mount_envelope_compiler_id=`{}`, candidate_set_rows={}, envelope_rows={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.world_mount_envelope_compiler_id,
                summary.candidate_set_row_count,
                summary.envelope_row_count,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReceipt;
    use psionic_research::build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_summary;

    #[test]
    fn post_article_plugin_world_mount_admissibility_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReceipt::from_summary(
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
            receipt.world_mount_envelope_compiler_id,
            "tassadar.plugin_runtime.world_mount_envelope_compiler.v1"
        );
        assert_eq!(
            receipt.admissibility_contract_id,
            "tassadar.plugin_runtime.admissibility.v1"
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
