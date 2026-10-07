use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::build_tassadar_broad_internal_compute_profile_publication_report;
use psionic_models::{
    TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    TassadarBroadInternalComputeWorldMountBindingStatus,
};
use psionic_router::{
    build_tassadar_broad_internal_compute_route_policy_report,
    TassadarBroadInternalComputeRouteDecisionStatus,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeProfilePublication {
    pub publication_id: String,
    pub report_ref: String,
    pub route_policy_report_ref: String,
    pub current_served_profile_id: String,
    pub published_profile_ids: Vec<String>,
    pub public_profile_specific_route_ids: Vec<String>,
    pub profile_specific_world_mount_template_ids: Vec<String>,
    pub profile_specific_accepted_outcome_template_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub current_served_route_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub current_served_world_mount_binding_status:
        TassadarBroadInternalComputeWorldMountBindingStatus,
    pub current_served_accepted_outcome_binding_status:
        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarBroadInternalComputeProfilePublicationError {
    #[error("failed to build broad internal-compute publication report: {detail}")]
    InvalidPublicationReport { detail: String },
    #[error("failed to build broad internal-compute route policy report: {detail}")]
    InvalidRoutePolicyReport { detail: String },
    #[error("current served broad profile `{profile_id}` was missing from the publication report")]
    MissingCurrentServedProfile { profile_id: String },
    #[error("current served broad profile `{profile_id}` was missing from route policy")]
    MissingCurrentServedRoute { profile_id: String },
    #[error("current served broad profile `{profile_id}` was not selected by route policy")]
    CurrentServedProfileNotSelected { profile_id: String },
}

pub fn build_tassadar_broad_internal_compute_profile_publication() -> Result<
    TassadarBroadInternalComputeProfilePublication,
    TassadarBroadInternalComputeProfilePublicationError,
> {
    let report =
        build_tassadar_broad_internal_compute_profile_publication_report().map_err(|error| {
            TassadarBroadInternalComputeProfilePublicationError::InvalidPublicationReport {
                detail: error.to_string(),
            }
        })?;
    let route_policy =
        build_tassadar_broad_internal_compute_route_policy_report().map_err(|error| {
            TassadarBroadInternalComputeProfilePublicationError::InvalidRoutePolicyReport {
                detail: error.to_string(),
            }
        })?;
    let publication_row = report
        .profile_rows
        .iter()
        .find(|row| row.profile_id == report.current_served_profile_id)
        .ok_or_else(|| {
            TassadarBroadInternalComputeProfilePublicationError::MissingCurrentServedProfile {
                profile_id: report.current_served_profile_id.clone(),
            }
        })?;
    let route_row = route_policy
        .rows
        .iter()
        .find(|row| row.target_profile_id == report.current_served_profile_id)
        .ok_or_else(|| {
            TassadarBroadInternalComputeProfilePublicationError::MissingCurrentServedRoute {
                profile_id: report.current_served_profile_id.clone(),
            }
        })?;
    if route_row.decision_status != TassadarBroadInternalComputeRouteDecisionStatus::Selected {
        return Err(
            TassadarBroadInternalComputeProfilePublicationError::CurrentServedProfileNotSelected {
                profile_id: report.current_served_profile_id.clone(),
            },
        );
    }
    let public_profile_specific_route_ids = route_policy
        .rows
        .iter()
        .filter(|row| {
            row.decision_status
                == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        })
        .map(|row| row.target_profile_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let profile_specific_world_mount_template_ids = report
        .profile_rows
        .iter()
        .filter(|row| {
            row.world_mount_binding_status
                == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
        })
        .map(|row| row.profile_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let profile_specific_accepted_outcome_template_ids = report
        .profile_rows
        .iter()
        .filter(|row| {
            row.accepted_outcome_binding_status
                == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable
        })
        .map(|row| row.profile_id.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(TassadarBroadInternalComputeProfilePublication {
        publication_id: report.report_id,
        report_ref: String::from(
            psionic_models::TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
        ),
        route_policy_report_ref: String::from(
            psionic_router::TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
        ),
        current_served_profile_id: report.current_served_profile_id,
        published_profile_ids: report.published_profile_ids,
        public_profile_specific_route_ids,
        profile_specific_world_mount_template_ids,
        profile_specific_accepted_outcome_template_ids,
        suppressed_profile_ids: report.suppressed_profile_ids,
        failed_profile_ids: report.failed_profile_ids,
        current_served_route_status: route_row.decision_status,
        current_served_world_mount_binding_status: publication_row.world_mount_binding_status,
        current_served_accepted_outcome_binding_status: publication_row
            .accepted_outcome_binding_status,
        claim_boundary: String::from(
            "this served publication carries the current selected broad internal-compute profile id, the committed publication report refs, and the current route-policy selection explicitly. It does not widen the served executor lane beyond the current selected profile or treat suppressed broader profiles as served capabilities",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::build_tassadar_broad_internal_compute_profile_publication;
    use psionic_models::{
        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
        TassadarBroadInternalComputeWorldMountBindingStatus,
    };
    use psionic_router::TassadarBroadInternalComputeRouteDecisionStatus;

    #[test]
    fn broad_internal_compute_profile_publication_keeps_current_served_profile_selected() {
        let publication =
            build_tassadar_broad_internal_compute_profile_publication().expect("publication");
        assert_eq!(
            publication.current_served_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert_eq!(
            publication.current_served_route_status,
            TassadarBroadInternalComputeRouteDecisionStatus::Selected
        );
        assert_eq!(
            publication.current_served_world_mount_binding_status,
            TassadarBroadInternalComputeWorldMountBindingStatus::CompatibleWithCurrentExactComputeMount
        );
        assert_eq!(
            publication.current_served_accepted_outcome_binding_status,
            TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ExactComputeEnvelopeAvailable
        );
        assert!(publication.suppressed_profile_ids.contains(&String::from(
            "tassadar.internal_compute.public_broad_family.v1"
        )));
        assert!(publication
            .public_profile_specific_route_ids
            .contains(&String::from(
                "tassadar.internal_compute.deterministic_import_subset.v1"
            )));
        assert!(publication
            .public_profile_specific_route_ids
            .contains(&String::from(
                "tassadar.internal_compute.runtime_support_subset.v1"
            )));
    }
}
