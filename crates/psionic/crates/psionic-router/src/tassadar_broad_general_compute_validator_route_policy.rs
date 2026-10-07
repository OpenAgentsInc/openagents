use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_broad_internal_compute_route_policy_report,
    TassadarBroadInternalComputeRouteDecisionStatus, TassadarBroadInternalComputeRoutePolicyRow,
    TassadarBroadInternalComputeRoutePolicyReportError,
};

pub const TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_general_compute_validator_route_policy_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadGeneralComputeAuthorityStatus {
    AcceptedOutcomeReady,
    CandidateOnlyChallengeWindow,
    SuppressedPendingPolicy,
    RefusedPendingEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeValidatorRoutePolicyRow {
    pub route_policy_id: String,
    pub target_profile_id: String,
    pub base_route_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub authority_status: TassadarBroadGeneralComputeAuthorityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_policy_ref: Option<String>,
    pub challenge_window_minutes: u32,
    pub economic_receipt_allowed: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeValidatorRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub base_route_policy_report_ref: String,
    pub rows: Vec<TassadarBroadGeneralComputeValidatorRoutePolicyRow>,
    pub accepted_outcome_ready_route_count: u32,
    pub candidate_only_route_count: u32,
    pub suppressed_route_count: u32,
    pub refused_route_count: u32,
    pub accepted_outcome_ready_profile_ids: Vec<String>,
    pub candidate_only_profile_ids: Vec<String>,
    pub economic_receipt_allowed_profile_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadGeneralComputeValidatorRoutePolicyReportError {
    #[error(transparent)]
    BaseRoutePolicy(#[from] TassadarBroadInternalComputeRoutePolicyReportError),
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
}

pub fn build_tassadar_broad_general_compute_validator_route_policy_report() -> Result<
    TassadarBroadGeneralComputeValidatorRoutePolicyReport,
    TassadarBroadGeneralComputeValidatorRoutePolicyReportError,
> {
    let base_report = build_tassadar_broad_internal_compute_route_policy_report()?;
    let rows = base_report
        .rows
        .iter()
        .map(authority_row)
        .collect::<Vec<_>>();

    let accepted_outcome_ready_profile_ids = rows
        .iter()
        .filter(|row| row.authority_status == TassadarBroadGeneralComputeAuthorityStatus::AcceptedOutcomeReady)
        .map(|row| row.target_profile_id.clone())
        .collect::<Vec<_>>();
    let candidate_only_profile_ids = rows
        .iter()
        .filter(|row| row.authority_status == TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow)
        .map(|row| row.target_profile_id.clone())
        .collect::<Vec<_>>();
    let economic_receipt_allowed_profile_ids = rows
        .iter()
        .filter(|row| row.economic_receipt_allowed)
        .map(|row| row.target_profile_id.clone())
        .collect::<Vec<_>>();
    let accepted_outcome_ready_route_count = accepted_outcome_ready_profile_ids.len() as u32;
    let candidate_only_route_count = candidate_only_profile_ids.len() as u32;
    let suppressed_route_count = rows
        .iter()
        .filter(|row| row.authority_status == TassadarBroadGeneralComputeAuthorityStatus::SuppressedPendingPolicy)
        .count() as u32;
    let refused_route_count = rows
        .iter()
        .filter(|row| row.authority_status == TassadarBroadGeneralComputeAuthorityStatus::RefusedPendingEvidence)
        .count() as u32;

    let mut report = TassadarBroadGeneralComputeValidatorRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_general_compute_validator_route_policy.report.v1"),
        base_route_policy_report_ref: String::from(
            crate::TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
        ),
        rows,
        accepted_outcome_ready_route_count,
        candidate_only_route_count,
        suppressed_route_count,
        refused_route_count,
        accepted_outcome_ready_profile_ids,
        candidate_only_profile_ids,
        economic_receipt_allowed_profile_ids,
        claim_boundary: String::from(
            "this router report is the authority-facing and economic-facing overlay for named broad internal-compute profiles. It keeps accepted-outcome-ready, candidate-only, suppressed, and refused profiles explicit instead of turning broader internal compute into one generic validator-safe or market-safe lane.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad general-compute validator route policy now records accepted_outcome_ready_routes={}, candidate_only_routes={}, suppressed_routes={}, refused_routes={}.",
        report.accepted_outcome_ready_route_count,
        report.candidate_only_route_count,
        report.suppressed_route_count,
        report.refused_route_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_general_compute_validator_route_policy_report|",
        &report,
    );
    Ok(report)
}

fn authority_row(
    row: &TassadarBroadInternalComputeRoutePolicyRow,
) -> TassadarBroadGeneralComputeValidatorRoutePolicyRow {
    let (authority_status, validator_policy_ref, challenge_window_minutes, economic_receipt_allowed, note) =
        match row.target_profile_id.as_str() {
            "tassadar.internal_compute.article_closeout.v1" => (
                TassadarBroadGeneralComputeAuthorityStatus::AcceptedOutcomeReady,
                Some(String::from(
                    "validator-policies.internal_compute.article_closeout.v1",
                )),
                0,
                true,
                String::from(
                    "current served article-closeout stays accepted-outcome-ready and can issue validator-bound economic receipts under the existing exact-compute envelope",
                ),
            ),
            "tassadar.internal_compute.deterministic_import_subset.v1" => (
                TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow,
                Some(String::from(
                    "validator-policies.internal_compute.deterministic_import_subset.v1",
                )),
                120,
                true,
                String::from(
                    "deterministic import subset is now profile-specific and validator-attachable, but it stays candidate-only until the challenge window closes and the profile-specific authority envelope is satisfied",
                ),
            ),
            "tassadar.internal_compute.runtime_support_subset.v1" => (
                TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow,
                Some(String::from(
                    "validator-policies.internal_compute.runtime_support_subset.v1",
                )),
                90,
                true,
                String::from(
                    "runtime-support subset is now profile-specific and validator-attachable, but it stays candidate-only until the challenge window closes and the profile-specific authority envelope is satisfied",
                ),
            ),
            "tassadar.internal_compute.portable_broad_family.v1" => (
                TassadarBroadGeneralComputeAuthorityStatus::RefusedPendingEvidence,
                None,
                0,
                false,
                String::from(
                    "portable broad family stays refused because portability evidence and public authority posture are still incomplete",
                ),
            ),
            _ => (
                TassadarBroadGeneralComputeAuthorityStatus::SuppressedPendingPolicy,
                None,
                0,
                false,
                String::from(
                    "profile remains suppressed pending explicit profile-specific mount, accepted-outcome, and economic policy artifacts",
                ),
            ),
        };

    TassadarBroadGeneralComputeValidatorRoutePolicyRow {
        route_policy_id: format!("validator_route.{}", row.target_profile_id),
        target_profile_id: row.target_profile_id.clone(),
        base_route_status: row.decision_status,
        authority_status,
        validator_policy_ref,
        challenge_window_minutes,
        economic_receipt_allowed,
        note,
    }
}

#[must_use]
pub fn tassadar_broad_general_compute_validator_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_broad_general_compute_validator_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadGeneralComputeValidatorRoutePolicyReport,
    TassadarBroadGeneralComputeValidatorRoutePolicyReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadGeneralComputeValidatorRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_general_compute_validator_route_policy_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("broad general-compute validator route policy serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarBroadGeneralComputeValidatorRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_broad_general_compute_validator_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarBroadGeneralComputeValidatorRoutePolicyReport,
    TassadarBroadGeneralComputeValidatorRoutePolicyReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadGeneralComputeValidatorRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadGeneralComputeValidatorRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_general_compute_validator_route_policy_report,
        load_tassadar_broad_general_compute_validator_route_policy_report,
        tassadar_broad_general_compute_validator_route_policy_report_path,
        TassadarBroadGeneralComputeAuthorityStatus,
        TassadarBroadGeneralComputeValidatorRoutePolicyReport,
    };

    #[test]
    fn broad_general_compute_validator_route_policy_keeps_ready_candidate_and_refused_profiles_distinct() {
        let report =
            build_tassadar_broad_general_compute_validator_route_policy_report().expect("report");

        assert_eq!(report.accepted_outcome_ready_route_count, 1);
        assert_eq!(report.candidate_only_route_count, 2);
        assert_eq!(report.suppressed_route_count, 4);
        assert_eq!(report.refused_route_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.article_closeout.v1"
                && row.authority_status
                    == TassadarBroadGeneralComputeAuthorityStatus::AcceptedOutcomeReady
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.deterministic_import_subset.v1"
                && row.authority_status
                    == TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.runtime_support_subset.v1"
                && row.authority_status
                    == TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.portable_broad_family.v1"
                && row.authority_status
                    == TassadarBroadGeneralComputeAuthorityStatus::RefusedPendingEvidence
        }));
    }

    #[test]
    fn broad_general_compute_validator_route_policy_matches_committed_truth() {
        let generated =
            build_tassadar_broad_general_compute_validator_route_policy_report().expect("report");
        let committed: TassadarBroadGeneralComputeValidatorRoutePolicyReport =
            load_tassadar_broad_general_compute_validator_route_policy_report(
                tassadar_broad_general_compute_validator_route_policy_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
    }
}
