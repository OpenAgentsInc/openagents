use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::build_tassadar_proposal_profile_ladder_claim_checker_report;
use psionic_router::{
    build_tassadar_proposal_profile_route_policy_report,
    TassadarBroadInternalComputeRouteDecisionStatus,
};

pub const PROPOSAL_PROFILE_PUBLICATION_POLICY_ID: &str = "psionic.proposal_profile_publication";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProposalProfilePublicationPolicy {
    pub publication_id: String,
    pub claim_checker_report_ref: String,
    pub route_policy_report_ref: String,
    pub public_profile_ids: Vec<String>,
    pub served_publication_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub research_only_profile_ids: Vec<String>,
    pub default_served_profile_ids: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarProposalProfilePublicationPolicyError {
    #[error("proposal-profile claim checker was not green")]
    InvalidClaimChecker,
    #[error("proposal-profile publication widened the default served lane")]
    DefaultServedProfilesMustStayEmpty,
    #[error("proposal-profile route policy drifted from claim checker")]
    RoutePolicyDrift,
}

pub fn build_tassadar_proposal_profile_publication_policy(
) -> Result<TassadarProposalProfilePublicationPolicy, TassadarProposalProfilePublicationPolicyError>
{
    let report = build_tassadar_proposal_profile_ladder_claim_checker_report()
        .map_err(|_| TassadarProposalProfilePublicationPolicyError::InvalidClaimChecker)?;
    let route_policy = build_tassadar_proposal_profile_route_policy_report()
        .map_err(|_| TassadarProposalProfilePublicationPolicyError::RoutePolicyDrift)?;
    if !report.overall_green {
        return Err(TassadarProposalProfilePublicationPolicyError::InvalidClaimChecker);
    }
    if !report.default_served_profile_ids.is_empty() {
        return Err(
            TassadarProposalProfilePublicationPolicyError::DefaultServedProfilesMustStayEmpty,
        );
    }
    let promoted_route_profile_ids = route_policy
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
    if promoted_route_profile_ids != report.served_publication_allowed_profile_ids {
        return Err(TassadarProposalProfilePublicationPolicyError::RoutePolicyDrift);
    }
    Ok(TassadarProposalProfilePublicationPolicy {
        publication_id: String::from(PROPOSAL_PROFILE_PUBLICATION_POLICY_ID),
        claim_checker_report_ref: String::from(
            psionic_eval::TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF,
        ),
        route_policy_report_ref: String::from(
            psionic_router::TASSADAR_PROPOSAL_PROFILE_ROUTE_POLICY_REPORT_REF,
        ),
        public_profile_ids: report.public_profile_ids,
        served_publication_allowed_profile_ids: report.served_publication_allowed_profile_ids,
        suppressed_profile_ids: report.suppressed_profile_ids,
        research_only_profile_ids: report.research_only_profile_ids,
        default_served_profile_ids: report.default_served_profile_ids,
        claim_boundary: String::from(
            "this served publication policy promotes only the proposal families that the proposal-profile ladder claim checker marks as named public profiles and that the router marks as profile-specific routes. It keeps operator-only families suppressed, research-only families non-served, and the default served lane empty.",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_proposal_profile_publication_policy, PROPOSAL_PROFILE_PUBLICATION_POLICY_ID,
    };

    #[test]
    fn proposal_profile_publication_policy_keeps_only_public_profiles_routeable() {
        let policy = build_tassadar_proposal_profile_publication_policy().expect("policy");

        assert_eq!(
            policy.publication_id,
            PROPOSAL_PROFILE_PUBLICATION_POLICY_ID
        );
        assert_eq!(
            policy.served_publication_allowed_profile_ids,
            vec![
                String::from("tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"),
                String::from("tassadar.proposal_profile.simd_deterministic.v1"),
            ]
        );
        assert!(policy.default_served_profile_ids.is_empty());
        assert!(policy.suppressed_profile_ids.contains(&String::from(
            "tassadar.proposal_profile.memory64_continuation.v1"
        )));
        assert!(policy.research_only_profile_ids.contains(&String::from(
            "tassadar.research_profile.threads_deterministic_scheduler.v1"
        )));
    }
}
