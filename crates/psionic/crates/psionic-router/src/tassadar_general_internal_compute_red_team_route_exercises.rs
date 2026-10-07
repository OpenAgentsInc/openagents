use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_broad_general_compute_validator_route_policy_report,
    build_tassadar_proposal_profile_route_policy_report,
    TassadarBroadGeneralComputeAuthorityStatus,
    TassadarBroadGeneralComputeValidatorRoutePolicyReportError,
    TassadarBroadInternalComputeRouteDecisionStatus, TassadarProposalProfileRoutePolicyReportError,
};

pub const TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_general_internal_compute_red_team_route_exercises_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralInternalComputeRedTeamRouteOutcome {
    BlockedAsExpected,
    UnexpectedlyRouteable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamRouteExerciseRow {
    pub case_id: String,
    pub target_profile_id: String,
    pub requested_posture: String,
    pub route_outcome: TassadarGeneralInternalComputeRedTeamRouteOutcome,
    pub source_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralInternalComputeRedTeamRouteExercisesReport {
    pub schema_version: u16,
    pub report_id: String,
    pub cases: Vec<TassadarGeneralInternalComputeRedTeamRouteExerciseRow>,
    pub blocked_case_ids: Vec<String>,
    pub failed_case_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarGeneralInternalComputeRedTeamRouteExercisesReportError {
    #[error(transparent)]
    BroadGeneralCompute(#[from] TassadarBroadGeneralComputeValidatorRoutePolicyReportError),
    #[error(transparent)]
    ProposalProfiles(#[from] TassadarProposalProfileRoutePolicyReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_general_internal_compute_red_team_route_exercises_report() -> Result<
    TassadarGeneralInternalComputeRedTeamRouteExercisesReport,
    TassadarGeneralInternalComputeRedTeamRouteExercisesReportError,
> {
    let broad_report = build_tassadar_broad_general_compute_validator_route_policy_report()?;
    let proposal_report = build_tassadar_proposal_profile_route_policy_report()?;

    let deterministic_import_row = broad_report
        .rows
        .iter()
        .find(|row| {
            row.target_profile_id == "tassadar.internal_compute.deterministic_import_subset.v1"
        })
        .expect("deterministic import subset row should exist");
    let portable_broad_row = broad_report
        .rows
        .iter()
        .find(|row| row.target_profile_id == "tassadar.internal_compute.portable_broad_family.v1")
        .expect("portable broad family row should exist");
    let memory64_row = proposal_report
        .rows
        .iter()
        .find(|row| row.target_profile_id == "tassadar.proposal_profile.memory64_continuation.v1")
        .expect("memory64 row should exist");
    let threads_row = proposal_report
        .rows
        .iter()
        .find(|row| {
            row.target_profile_id == "tassadar.research_profile.threads_deterministic_scheduler.v1"
        })
        .expect("threads row should exist");

    let cases = vec![
        route_case(
            "candidate_only_profile_cannot_skip_challenge_window",
            "tassadar.internal_compute.deterministic_import_subset.v1",
            "accepted_outcome_ready_route",
            deterministic_import_row.authority_status
                == TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow,
            &[crate::TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF],
            "candidate-only internal-compute profiles must stay challenge-window bounded instead of routing as accepted-outcome-ready immediately",
        ),
        route_case(
            "portable_broad_family_cannot_route_without_evidence",
            "tassadar.internal_compute.portable_broad_family.v1",
            "broad_general_compute_route",
            portable_broad_row.authority_status
                == TassadarBroadGeneralComputeAuthorityStatus::RefusedPendingEvidence,
            &[crate::TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF],
            "portable broad-family requests stay refused until portability evidence and authority policy are complete",
        ),
        route_case(
            "memory64_cannot_inherit_public_route",
            "tassadar.proposal_profile.memory64_continuation.v1",
            "public_proposal_route",
            memory64_row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed,
            &[crate::TASSADAR_PROPOSAL_PROFILE_ROUTE_POLICY_REPORT_REF],
            "memory64 remains operator-only and cannot inherit public routeability from the frozen core lane",
        ),
        route_case(
            "threads_cannot_route_as_public_profile",
            "tassadar.research_profile.threads_deterministic_scheduler.v1",
            "public_threads_route",
            threads_row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Refused,
            &[crate::TASSADAR_PROPOSAL_PROFILE_ROUTE_POLICY_REPORT_REF],
            "threads stays research-only and cannot surface as a public profile-specific route",
        ),
    ];
    let blocked_case_ids = cases
        .iter()
        .filter(|case| {
            case.route_outcome
                == TassadarGeneralInternalComputeRedTeamRouteOutcome::BlockedAsExpected
        })
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let failed_case_ids = cases
        .iter()
        .filter(|case| {
            case.route_outcome
                == TassadarGeneralInternalComputeRedTeamRouteOutcome::UnexpectedlyRouteable
        })
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarGeneralInternalComputeRedTeamRouteExercisesReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.general_internal_compute.red_team_route_exercises.report.v1",
        ),
        cases,
        blocked_case_ids,
        failed_case_ids,
        overall_green: false,
        claim_boundary: String::from(
            "this router report red-teams the served and authority-facing route surfaces for broad internal-compute and proposal profiles. It exists to prove that candidate-only, operator-only, and research-only profiles stay blocked instead of becoming implicitly routeable.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.failed_case_ids.is_empty() && report.blocked_case_ids.len() == 4;
    report.summary = format!(
        "General internal-compute red-team route exercises keep blocked_cases={}, failed_cases={}, overall_green={}.",
        report.blocked_case_ids.len(),
        report.failed_case_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_general_internal_compute_red_team_route_exercises_report|",
        &report,
    );
    Ok(report)
}

fn route_case(
    case_id: &str,
    target_profile_id: &str,
    requested_posture: &str,
    blocked_as_expected: bool,
    source_refs: &[&str],
    note: &str,
) -> TassadarGeneralInternalComputeRedTeamRouteExerciseRow {
    TassadarGeneralInternalComputeRedTeamRouteExerciseRow {
        case_id: String::from(case_id),
        target_profile_id: String::from(target_profile_id),
        requested_posture: String::from(requested_posture),
        route_outcome: if blocked_as_expected {
            TassadarGeneralInternalComputeRedTeamRouteOutcome::BlockedAsExpected
        } else {
            TassadarGeneralInternalComputeRedTeamRouteOutcome::UnexpectedlyRouteable
        },
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

#[must_use]
pub fn tassadar_general_internal_compute_red_team_route_exercises_report_path() -> PathBuf {
    repo_root().join(TASSADAR_GENERAL_INTERNAL_COMPUTE_RED_TEAM_ROUTE_EXERCISES_REPORT_REF)
}

pub fn write_tassadar_general_internal_compute_red_team_route_exercises_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarGeneralInternalComputeRedTeamRouteExercisesReport,
    TassadarGeneralInternalComputeRedTeamRouteExercisesReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarGeneralInternalComputeRedTeamRouteExercisesReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_general_internal_compute_red_team_route_exercises_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamRouteExercisesReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarGeneralInternalComputeRedTeamRouteExercisesReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamRouteExercisesReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarGeneralInternalComputeRedTeamRouteExercisesReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_general_internal_compute_red_team_route_exercises_report, read_json,
        tassadar_general_internal_compute_red_team_route_exercises_report_path,
        TassadarGeneralInternalComputeRedTeamRouteExercisesReport,
        TassadarGeneralInternalComputeRedTeamRouteOutcome,
    };

    #[test]
    fn red_team_route_exercises_keep_non_public_profiles_blocked() {
        let report = build_tassadar_general_internal_compute_red_team_route_exercises_report()
            .expect("report");

        assert!(report.overall_green);
        assert_eq!(report.blocked_case_ids.len(), 4);
        assert!(report.failed_case_ids.is_empty());
        assert!(report.cases.iter().all(|case| {
            case.route_outcome
                == TassadarGeneralInternalComputeRedTeamRouteOutcome::BlockedAsExpected
        }));
    }

    #[test]
    fn red_team_route_exercises_match_committed_truth() {
        let generated = build_tassadar_general_internal_compute_red_team_route_exercises_report()
            .expect("report");
        let committed: TassadarGeneralInternalComputeRedTeamRouteExercisesReport =
            read_json(tassadar_general_internal_compute_red_team_route_exercises_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
