use serde::{Deserialize, Serialize};

use psionic_serve::TassadarBroadInternalComputeProfilePublication;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeProfilePublicationReceipt {
    pub publication_id: String,
    pub current_served_profile_id: String,
    pub published_profile_ids: Vec<String>,
    pub public_profile_specific_route_ids: Vec<String>,
    pub profile_specific_world_mount_template_ids: Vec<String>,
    pub profile_specific_accepted_outcome_template_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub route_policy_report_ref: String,
    pub current_served_world_mount_binding_status:
        psionic_models::TassadarBroadInternalComputeWorldMountBindingStatus,
    pub current_served_accepted_outcome_binding_status:
        psionic_models::TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    pub detail: String,
}

impl TassadarBroadInternalComputeProfilePublicationReceipt {
    #[must_use]
    pub fn from_publication(publication: &TassadarBroadInternalComputeProfilePublication) -> Self {
        Self {
            publication_id: publication.publication_id.clone(),
            current_served_profile_id: publication.current_served_profile_id.clone(),
            published_profile_ids: publication.published_profile_ids.clone(),
            public_profile_specific_route_ids: publication.public_profile_specific_route_ids.clone(),
            profile_specific_world_mount_template_ids: publication
                .profile_specific_world_mount_template_ids
                .clone(),
            profile_specific_accepted_outcome_template_ids: publication
                .profile_specific_accepted_outcome_template_ids
                .clone(),
            suppressed_profile_ids: publication.suppressed_profile_ids.clone(),
            failed_profile_ids: publication.failed_profile_ids.clone(),
            route_policy_report_ref: publication.route_policy_report_ref.clone(),
            current_served_world_mount_binding_status: publication
                .current_served_world_mount_binding_status,
            current_served_accepted_outcome_binding_status: publication
                .current_served_accepted_outcome_binding_status,
            detail: format!(
                "broad internal-compute publication `{}` keeps current_served_profile=`{}`, published_profiles={}, suppressed_profiles={}, failed_profiles={}, route_policy_ref=`{}`",
                publication.publication_id,
                publication.current_served_profile_id,
                publication.published_profile_ids.len(),
                publication.suppressed_profile_ids.len(),
                publication.failed_profile_ids.len(),
                publication.route_policy_report_ref,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarBroadInternalComputeProfilePublicationReceipt;
    use psionic_serve::build_tassadar_broad_internal_compute_profile_publication;

    #[test]
    fn broad_internal_compute_profile_publication_receipt_projects_serve_publication() {
        let publication =
            build_tassadar_broad_internal_compute_profile_publication().expect("publication");
        let receipt =
            TassadarBroadInternalComputeProfilePublicationReceipt::from_publication(&publication);

        assert_eq!(
            receipt.current_served_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(receipt.suppressed_profile_ids.contains(&String::from(
            "tassadar.internal_compute.public_broad_family.v1"
        )));
        assert!(receipt
            .public_profile_specific_route_ids
            .contains(&String::from(
                "tassadar.internal_compute.deterministic_import_subset.v1"
            )));
        assert!(receipt
            .profile_specific_world_mount_template_ids
            .contains(&String::from(
                "tassadar.internal_compute.runtime_support_subset.v1"
            )));
    }
}
