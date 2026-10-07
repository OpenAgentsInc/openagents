use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginPacketAbiAndRustPdkSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiAndRustPdkReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub packet_abi_version: String,
    pub rust_first_pdk_id: String,
    pub contract_status: String,
    pub abi_row_count: u32,
    pub pdk_row_count: u32,
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

impl TassadarPostArticlePluginPacketAbiAndRustPdkReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticlePluginPacketAbiAndRustPdkSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            packet_abi_version: summary.packet_abi_version.clone(),
            rust_first_pdk_id: summary.rust_first_pdk_id.clone(),
            contract_status: format!("{:?}", summary.contract_status).to_lowercase(),
            abi_row_count: summary.abi_row_count,
            pdk_row_count: summary.pdk_row_count,
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
                "post-article plugin packet ABI summary `{}` keeps contract_status={:?}, packet_abi_version=`{}`, rust_first_pdk_id=`{}`, validation_rows={}, and deferred_issue_ids={}.",
                summary.report_id,
                summary.contract_status,
                summary.packet_abi_version,
                summary.rust_first_pdk_id,
                summary.validation_row_count,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginPacketAbiAndRustPdkReceipt;
    use psionic_research::build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_summary;

    #[test]
    fn post_article_plugin_packet_abi_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_summary().expect("summary");
        let receipt = TassadarPostArticlePluginPacketAbiAndRustPdkReceipt::from_summary(&summary);

        assert_eq!(receipt.contract_status, "green");
        assert_eq!(receipt.packet_abi_version, "packet.v1");
        assert_eq!(
            receipt.rust_first_pdk_id,
            "tassadar.plugin.rust_first_pdk.v1"
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
