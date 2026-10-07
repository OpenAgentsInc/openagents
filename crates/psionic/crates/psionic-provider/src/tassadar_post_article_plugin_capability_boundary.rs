use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticlePluginCapabilityBoundarySummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginCapabilityBoundaryReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub reserved_capability_plane_id: String,
    pub boundary_status: String,
    pub first_plugin_tranche_posture: String,
    pub dependency_row_count: u32,
    pub reserved_invariant_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticlePluginCapabilityBoundaryReceipt {
    #[must_use]
    pub fn from_summary(summary: &TassadarPostArticlePluginCapabilityBoundarySummary) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            reserved_capability_plane_id: summary.reserved_capability_plane_id.clone(),
            boundary_status: format!("{:?}", summary.boundary_status).to_lowercase(),
            first_plugin_tranche_posture: summary.first_plugin_tranche_posture.clone(),
            dependency_row_count: summary.dependency_row_count,
            reserved_invariant_count: summary.reserved_invariant_count,
            validation_row_count: summary.validation_row_count,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
            plugin_publication_allowed: summary.plugin_publication_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article plugin-capability boundary summary `{}` keeps boundary_status={:?}, reserved_capability_plane_id=`{}`, first_plugin_tranche_posture=`{}`, deferred_issue_ids={}, and plugin/publication claims blocked={}.",
                summary.report_id,
                summary.boundary_status,
                summary.reserved_capability_plane_id,
                summary.first_plugin_tranche_posture,
                summary.deferred_issue_ids.len(),
                !summary.plugin_capability_claim_allowed
                    && !summary.weighted_plugin_control_allowed
                    && !summary.plugin_publication_allowed,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticlePluginCapabilityBoundaryReceipt;
    use psionic_research::build_tassadar_post_article_plugin_capability_boundary_summary;

    #[test]
    fn post_article_plugin_capability_boundary_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_plugin_capability_boundary_summary().expect("summary");
        let receipt = TassadarPostArticlePluginCapabilityBoundaryReceipt::from_summary(&summary);

        assert_eq!(receipt.boundary_status, "green");
        assert_eq!(
            receipt.first_plugin_tranche_posture,
            "closed_world_operator_curated_only_until_audited"
        );
        assert!(receipt.deferred_issue_ids.is_empty());
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.weighted_plugin_control_allowed);
        assert!(!receipt.plugin_publication_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
