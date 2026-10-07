use serde::{Deserialize, Serialize};

use psionic_research::TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityPortabilityMinimalityMatrixReceipt {
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub matrix_status: String,
    pub bounded_universality_story_carried: bool,
    pub machine_row_count: u32,
    pub route_row_count: u32,
    pub minimality_row_count: u32,
    pub validation_row_count: u32,
    pub served_suppression_boundary_preserved: bool,
    pub served_conformance_envelope_defined: bool,
    pub deferred_issue_ids: Vec<String>,
    pub universal_substrate_gate_allowed: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
}

impl TassadarPostArticleUniversalityPortabilityMinimalityMatrixReceipt {
    #[must_use]
    pub fn from_summary(
        summary: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixSummary,
    ) -> Self {
        Self {
            report_id: summary.report_id.clone(),
            machine_identity_id: summary.machine_identity_id.clone(),
            canonical_model_id: summary.canonical_model_id.clone(),
            canonical_route_id: summary.canonical_route_id.clone(),
            matrix_status: format!("{:?}", summary.matrix_status).to_lowercase(),
            bounded_universality_story_carried: summary.bounded_universality_story_carried,
            machine_row_count: summary.machine_row_count,
            route_row_count: summary.route_row_count,
            minimality_row_count: summary.minimality_row_count,
            validation_row_count: summary.validation_row_count,
            served_suppression_boundary_preserved: summary.served_suppression_boundary_preserved,
            served_conformance_envelope_defined: summary.served_conformance_envelope_defined,
            deferred_issue_ids: summary.deferred_issue_ids.clone(),
            universal_substrate_gate_allowed: summary.universal_substrate_gate_allowed,
            rebase_claim_allowed: summary.rebase_claim_allowed,
            plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
            served_public_universality_allowed: summary.served_public_universality_allowed,
            arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
            detail: format!(
                "post-article universality portability/minimality matrix receipt keeps machine_identity_id=`{}`, canonical_route_id=`{}`, matrix_status={:?}, served_conformance_envelope_defined={}, and deferred_issue_ids={}.",
                summary.machine_identity_id,
                summary.canonical_route_id,
                summary.matrix_status,
                summary.served_conformance_envelope_defined,
                summary.deferred_issue_ids.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleUniversalityPortabilityMinimalityMatrixReceipt;
    use psionic_research::build_tassadar_post_article_universality_portability_minimality_matrix_summary;

    #[test]
    fn universality_portability_minimality_matrix_receipt_projects_summary() {
        let summary =
            build_tassadar_post_article_universality_portability_minimality_matrix_summary()
                .expect("summary");
        let receipt =
            TassadarPostArticleUniversalityPortabilityMinimalityMatrixReceipt::from_summary(
                &summary,
            );

        assert_eq!(receipt.matrix_status, "green");
        assert!(receipt.bounded_universality_story_carried);
        assert_eq!(receipt.machine_row_count, 3);
        assert_eq!(receipt.route_row_count, 4);
        assert_eq!(receipt.minimality_row_count, 3);
        assert_eq!(receipt.validation_row_count, 8);
        assert!(receipt.served_suppression_boundary_preserved);
        assert!(receipt.served_conformance_envelope_defined);
        assert_eq!(receipt.deferred_issue_ids, vec![String::from("TAS-194")]);
        assert!(receipt.universal_substrate_gate_allowed);
        assert!(!receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert!(!receipt.arbitrary_software_capability_allowed);
    }
}
