use serde::{Deserialize, Serialize};

use psionic_serve::TassadarUniversalityVerdictPublication;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityVerdictReceipt {
    pub publication_id: String,
    pub report_ref: String,
    pub current_served_internal_compute_profile_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub operator_allowed_profile_ids: Vec<String>,
    pub served_allowed_profile_ids: Vec<String>,
    pub blocked_by: Vec<String>,
    pub detail: String,
}

impl TassadarUniversalityVerdictReceipt {
    #[must_use]
    pub fn from_publication(publication: &TassadarUniversalityVerdictPublication) -> Self {
        Self {
            publication_id: publication.publication_id.clone(),
            report_ref: publication.report_ref.clone(),
            current_served_internal_compute_profile_id: publication
                .current_served_internal_compute_profile_id
                .clone(),
            theory_green: publication.theory_green,
            operator_green: publication.operator_green,
            served_green: publication.served_green,
            operator_allowed_profile_ids: publication.operator_allowed_profile_ids.clone(),
            served_allowed_profile_ids: publication.served_allowed_profile_ids.clone(),
            blocked_by: publication.blocked_by.clone(),
            detail: format!(
                "universality verdict publication `{}` keeps theory_green={}, operator_green={}, served_green={}, operator_profiles={}, served_profiles={}, blocked_by={}",
                publication.publication_id,
                publication.theory_green,
                publication.operator_green,
                publication.served_green,
                publication.operator_allowed_profile_ids.len(),
                publication.served_allowed_profile_ids.len(),
                publication.blocked_by.len(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarUniversalityVerdictReceipt;
    use psionic_serve::build_tassadar_universality_verdict_publication;

    #[test]
    fn universality_verdict_receipt_projects_publication() {
        let publication = build_tassadar_universality_verdict_publication().expect("publication");
        let receipt = TassadarUniversalityVerdictReceipt::from_publication(&publication);

        assert!(receipt.theory_green);
        assert!(receipt.operator_green);
        assert!(!receipt.served_green);
        assert_eq!(
            receipt.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert_eq!(receipt.operator_allowed_profile_ids.len(), 3);
        assert!(receipt.served_allowed_profile_ids.is_empty());
        assert!(receipt.blocked_by.contains(&String::from(
            "nexus_accepted_outcome_closure_outside_psionic"
        )));
    }
}
