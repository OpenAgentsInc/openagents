use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleUniversalityBridgeContractSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityBridgeContractReceipt {
    pub report_id: String,
    pub bridge_status: String,
    pub bridge_machine_identity_id: String,
    pub carrier_topology: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub reserved_capability_issue_ids: Vec<String>,
    pub bridge_contract_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleUniversalityBridgeContractReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticleUniversalityBridgeContractSummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            bridge_status: match summary.bridge_status {
                psionic_eval::TassadarPostArticleUniversalityBridgeStatus::Green => {
                    String::from("green")
                }
                psionic_eval::TassadarPostArticleUniversalityBridgeStatus::Blocked => {
                    String::from("blocked")
                }
            },
            bridge_machine_identity_id: summary.bridge_machine_identity_id.clone(),
            carrier_topology: String::from("explicit_split_across_direct_and_resumable_lanes"),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            continuation_contract_id: summary.continuation_contract_id.clone(),
            reserved_capability_issue_ids: summary.reserved_capability_issue_ids.clone(),
            bridge_contract_green: summary.bridge_contract_green,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            detail: format!(
                "post-article universality bridge summary `{}` keeps bridge_status={:?}, carrier_topology={:?}, canonical_route_id=`{}`, reserved_capability_issue_ids={}, bridge_contract_green={}, rebase_claim_allowed={}, plugin_capability_claim_allowed={}, and served_public_universality_allowed={}.",
                summary.report_id,
                summary.bridge_status,
                summary.carrier_topology,
                summary.canonical_route_id,
                summary.reserved_capability_issue_ids.len(),
                summary.bridge_contract_green,
                summary.rebase_claim_allowed,
                summary.plugin_capability_claim_allowed,
                summary.served_public_universality_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleUniversalityBridgeContractReceipt;
    use psionic_research::build_tassadar_post_article_universality_bridge_contract_summary;

    #[test]
    fn post_article_universality_bridge_contract_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_universality_bridge_contract_summary().expect("summary");
        let receipt = TassadarPostArticleUniversalityBridgeContractReceipt::from_summary(&summary);

        assert_eq!(receipt.bridge_status, "green");
        assert_eq!(
            receipt.carrier_topology,
            "explicit_split_across_direct_and_resumable_lanes"
        );
        assert_eq!(
            receipt.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert!(receipt
            .reserved_capability_issue_ids
            .contains(&String::from("TAS-195")));
        assert!(receipt
            .reserved_capability_issue_ids
            .contains(&String::from("TAS-216")));
        assert!(receipt.bridge_contract_green);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
    }
}
