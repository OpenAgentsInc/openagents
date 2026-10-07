use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::build_tassadar_general_internal_compute_red_team_audit_report;
use psionic_router::TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF;

pub const GENERAL_INTERNAL_COMPUTE_RED_TEAM_PUBLICATION_ID: &str =
    "psionic.general_internal_compute_red_team_audit";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralInternalComputeRedTeamPublicationStatus {
    Published,
    Suppressed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamPublicationDecision {
    pub publication_id: String,
    pub audit_report_ref: String,
    pub route_exercises_report_ref: String,
    pub status: TassadarGeneralInternalComputeRedTeamPublicationStatus,
    pub blocked_finding_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarGeneralInternalComputeRedTeamPublicationDecisionError {
    #[error("general internal-compute red-team audit is not publication-safe: {detail}")]
    NotPublicationSafe { detail: String },
}

pub fn tassadar_general_internal_compute_red_team_publication_decision(
) -> TassadarGeneralInternalComputeRedTeamPublicationDecision {
    let report = build_tassadar_general_internal_compute_red_team_audit_report()
        .expect("general internal-compute red-team audit should build");
    let status = if report.publication_safe {
        TassadarGeneralInternalComputeRedTeamPublicationStatus::Published
    } else {
        TassadarGeneralInternalComputeRedTeamPublicationStatus::Suppressed
    };
    TassadarGeneralInternalComputeRedTeamPublicationDecision {
        publication_id: String::from(GENERAL_INTERNAL_COMPUTE_RED_TEAM_PUBLICATION_ID),
        audit_report_ref: String::from(
            psionic_eval::TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_AUDIT_REPORT_REF,
        ),
        route_exercises_report_ref: String::from(
            TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF,
        ),
        status,
        blocked_finding_ids: report.failed_finding_ids,
        detail: report.summary,
    }
}

pub fn require_tassadar_general_internal_compute_red_team_publication() -> Result<
    TassadarGeneralInternalComputeRedTeamPublicationDecision,
    TassadarGeneralInternalComputeRedTeamPublicationDecisionError,
> {
    let decision = tassadar_general_internal_compute_red_team_publication_decision();
    match decision.status {
        TassadarGeneralInternalComputeRedTeamPublicationStatus::Published => Ok(decision),
        TassadarGeneralInternalComputeRedTeamPublicationStatus::Suppressed => Err(
            TassadarGeneralInternalComputeRedTeamPublicationDecisionError::NotPublicationSafe {
                detail: decision.detail.clone(),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        require_tassadar_general_internal_compute_red_team_publication,
        tassadar_general_internal_compute_red_team_publication_decision,
        TassadarGeneralInternalComputeRedTeamPublicationStatus,
    };

    #[test]
    fn red_team_publication_decision_is_publishable_when_findings_are_resolved() {
        let decision = tassadar_general_internal_compute_red_team_publication_decision();

        assert_eq!(
            decision.status,
            TassadarGeneralInternalComputeRedTeamPublicationStatus::Published
        );
        assert!(decision.blocked_finding_ids.is_empty());
    }

    #[test]
    fn red_team_publication_requirement_returns_decision_when_audit_is_clean() {
        let decision = require_tassadar_general_internal_compute_red_team_publication()
            .expect("audit is publication-safe");
        assert_eq!(
            decision.status,
            TassadarGeneralInternalComputeRedTeamPublicationStatus::Published
        );
    }
}
