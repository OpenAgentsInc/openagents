use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_eval::{
    TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF, TassadarUniversalityVerdictLevel,
    TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError,
    build_tassadar_universality_verdict_split_report,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityVerdictPublication {
    pub publication_id: String,
    pub report_ref: String,
    pub current_served_internal_compute_profile_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub operator_allowed_profile_ids: Vec<String>,
    pub operator_route_constraint_ids: Vec<String>,
    pub served_allowed_profile_ids: Vec<String>,
    pub served_route_constraint_ids: Vec<String>,
    pub blocked_by: Vec<String>,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarUniversalityVerdictPublicationError {
    #[error("failed to build universality verdict split report: {detail}")]
    InvalidVerdictReport { detail: String },
    #[error("missing `{verdict_level}` universality verdict row")]
    MissingVerdictRow { verdict_level: String },
    #[error("served universality publication is suppressed: {detail}")]
    ServedSuppressed { detail: String },
}

pub fn build_tassadar_universality_verdict_publication()
-> Result<TassadarUniversalityVerdictPublication, TassadarUniversalityVerdictPublicationError> {
    let report = build_tassadar_universality_verdict_split_report().map_err(
        |error: TassadarUniversalityVerdictSplitReportError| {
            TassadarUniversalityVerdictPublicationError::InvalidVerdictReport {
                detail: error.to_string(),
            }
        },
    )?;
    build_publication_from_report(report)
}

pub fn require_tassadar_served_universality_publication()
-> Result<TassadarUniversalityVerdictPublication, TassadarUniversalityVerdictPublicationError> {
    let publication = build_tassadar_universality_verdict_publication()?;
    if publication.served_green {
        Ok(publication)
    } else {
        Err(
            TassadarUniversalityVerdictPublicationError::ServedSuppressed {
                detail: String::from(
                    "the served universality verdict remains suppressed because no named served universality profile is published and authority-bearing closure still lives outside standalone psionic",
                ),
            },
        )
    }
}

fn build_publication_from_report(
    report: TassadarUniversalityVerdictSplitReport,
) -> Result<TassadarUniversalityVerdictPublication, TassadarUniversalityVerdictPublicationError> {
    if !report.overall_green || !report.theory_green || !report.operator_green {
        return Err(
            TassadarUniversalityVerdictPublicationError::InvalidVerdictReport {
                detail: format!(
                    "theory/operator verdict report must stay green before publication can project it (theory_green={}, operator_green={}, overall_green={})",
                    report.theory_green, report.operator_green, report.overall_green,
                ),
            },
        );
    }
    let operator_row = report
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Operator)
        .ok_or_else(
            || TassadarUniversalityVerdictPublicationError::MissingVerdictRow {
                verdict_level: String::from("operator"),
            },
        )?;
    let served_row = report
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Served)
        .ok_or_else(
            || TassadarUniversalityVerdictPublicationError::MissingVerdictRow {
                verdict_level: String::from("served"),
            },
        )?;

    Ok(TassadarUniversalityVerdictPublication {
        publication_id: String::from("psionic.tassadar_universality_verdict_publication.v1"),
        report_ref: String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
        current_served_internal_compute_profile_id: String::from(
            "tassadar.internal_compute.article_closeout.v1",
        ),
        theory_green: report.theory_green,
        operator_green: report.operator_green,
        served_green: report.served_green,
        operator_allowed_profile_ids: operator_row.allowed_profile_ids.clone(),
        operator_route_constraint_ids: operator_row.route_constraint_ids.clone(),
        served_allowed_profile_ids: served_row.allowed_profile_ids.clone(),
        served_route_constraint_ids: served_row.route_constraint_ids.clone(),
        blocked_by: served_row.blocked_by.clone(),
        kernel_policy_dependency_marker: report.kernel_policy_dependency_marker,
        nexus_dependency_marker: report.nexus_dependency_marker,
        claim_boundary: String::from(
            "this served publication projects the final universality verdict split without widening the current served executor lane. Theory and operator truth may be green while served publication stays suppressed.",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarUniversalityVerdictPublicationError,
        build_tassadar_universality_verdict_publication,
        require_tassadar_served_universality_publication,
    };

    #[test]
    fn universality_verdict_publication_keeps_served_posture_suppressed() {
        let publication = build_tassadar_universality_verdict_publication().expect("publication");

        assert!(publication.theory_green);
        assert!(publication.operator_green);
        assert!(!publication.served_green);
        assert_eq!(
            publication.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(
            publication
                .blocked_by
                .contains(&String::from("named_served_universal_profile_missing"))
        );
        assert!(
            publication
                .served_route_constraint_ids
                .contains(&String::from(
                    "served_universality_route_publication_suppressed"
                ))
        );
    }

    #[test]
    fn served_universality_publication_requirement_fails_closed() {
        let error = require_tassadar_served_universality_publication()
            .expect_err("served universality should stay suppressed");
        assert!(matches!(
            error,
            TassadarUniversalityVerdictPublicationError::ServedSuppressed { .. }
        ));
    }
}
