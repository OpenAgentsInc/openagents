use serde::{Deserialize, Serialize};

use psionic_serve::TassadarPostArticleRebasedUniversalityVerdictPublication;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalityVerdictReceipt {
    pub publication_id: String,
    pub report_ref: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub operator_allowed_profile_ids: Vec<String>,
    pub served_blocked_by: Vec<String>,
    pub detail: String,
}

impl TassadarPostArticleRebasedUniversalityVerdictReceipt {
    #[must_use]
    pub fn from_publication(
        publication: &TassadarPostArticleRebasedUniversalityVerdictPublication,
    ) -> Self {
        Self {
            publication_id: publication.publication_id.clone(),
            report_ref: publication.report_ref.clone(),
            machine_identity_id: publication.machine_identity_id.clone(),
            canonical_route_id: publication.canonical_route_id.clone(),
            current_served_internal_compute_profile_id: publication
                .current_served_internal_compute_profile_id
                .clone(),
            theory_green: publication.theory_green,
            operator_green: publication.operator_green,
            served_green: publication.served_green,
            rebase_claim_allowed: publication.rebase_claim_allowed,
            plugin_capability_claim_allowed: publication.plugin_capability_claim_allowed,
            served_public_universality_allowed: publication.served_public_universality_allowed,
            operator_allowed_profile_ids: publication.operator_allowed_profile_ids.clone(),
            served_blocked_by: publication.served_blocked_by.clone(),
            detail: format!(
                "post-article rebased universality verdict publication `{}` keeps theory_green={}, operator_green={}, served_green={}, rebase_claim_allowed={}, operator_profiles={}, served_blocked_by={}.",
                publication.publication_id,
                publication.theory_green,
                publication.operator_green,
                publication.served_green,
                publication.rebase_claim_allowed,
                publication.operator_allowed_profile_ids.len(),
                publication.served_blocked_by.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarPostArticleRebasedUniversalityVerdictReceipt;
    use psionic_serve::build_tassadar_post_article_rebased_universality_verdict_publication;

    #[test]
    fn post_article_rebased_universality_verdict_receipt_projects_publication() {
        let publication = build_tassadar_post_article_rebased_universality_verdict_publication()
            .expect("publication");
        let receipt =
            TassadarPostArticleRebasedUniversalityVerdictReceipt::from_publication(&publication);

        assert!(receipt.theory_green);
        assert!(receipt.operator_green);
        assert!(!receipt.served_green);
        assert!(receipt.rebase_claim_allowed);
        assert!(!receipt.plugin_capability_claim_allowed);
        assert!(!receipt.served_public_universality_allowed);
        assert_eq!(
            receipt.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(receipt.served_blocked_by.contains(&String::from(
            "kernel_policy_served_universality_authority_outside_psionic"
        )));
    }
}
