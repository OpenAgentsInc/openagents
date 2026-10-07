use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarBroadInternalComputeRouteDecisionStatus;

const TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_proposal_profile_ladder_claim_checker_report.json";

pub const TASSADAR_PROPOSAL_PROFILE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_proposal_profile_route_policy_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProposalProfileRoutePolicyRow {
    pub route_policy_id: String,
    pub target_profile_id: String,
    pub decision_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProposalProfileRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub claim_checker_report_ref: String,
    pub rows: Vec<TassadarProposalProfileRoutePolicyRow>,
    pub promoted_profile_specific_route_count: u32,
    pub suppressed_route_count: u32,
    pub refused_route_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ClaimCheckerSource {
    public_profile_ids: Vec<String>,
    suppressed_profile_ids: Vec<String>,
    research_only_profile_ids: Vec<String>,
}

#[derive(Debug, Error)]
pub enum TassadarProposalProfileRoutePolicyReportError {
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

pub fn build_tassadar_proposal_profile_route_policy_report(
) -> Result<TassadarProposalProfileRoutePolicyReport, TassadarProposalProfileRoutePolicyReportError>
{
    let source: ClaimCheckerSource =
        read_json(repo_root().join(TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF))?;
    let rows = vec![
        route_row(
            "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1",
            &source,
        ),
        route_row(
            "tassadar.proposal_profile.memory64_continuation.v1",
            &source,
        ),
        route_row("tassadar.proposal_profile.multi_memory_routing.v1", &source),
        route_row(
            "tassadar.proposal_profile.component_linking_interface_types.v1",
            &source,
        ),
        route_row("tassadar.proposal_profile.simd_deterministic.v1", &source),
        route_row(
            "tassadar.research_profile.threads_deterministic_scheduler.v1",
            &source,
        ),
    ];
    let promoted_profile_specific_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status
                == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        })
        .count() as u32;
    let suppressed_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        })
        .count() as u32;
    let refused_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Refused
        })
        .count() as u32;
    let mut report = TassadarProposalProfileRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.proposal_profile_route_policy.report.v1"),
        claim_checker_report_ref: String::from(
            TASSADAR_PROPOSAL_PROFILE_LADDER_CLAIM_CHECKER_REPORT_REF,
        ),
        rows,
        promoted_profile_specific_route_count,
        suppressed_route_count,
        refused_route_count,
        claim_boundary: String::from(
            "this router report promotes only the proposal profiles that the claim checker marks as named public profiles. Operator-only proposal families stay suppressed, and research-only families stay refused. This does not create a default served proposal lane or implicit inheritance across proposal families.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Proposal-profile route policy now records promoted_profile_specific_routes={}, suppressed_routes={}, refused_routes={}.",
        report.promoted_profile_specific_route_count,
        report.suppressed_route_count,
        report.refused_route_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_proposal_profile_route_policy_report|",
        &report,
    );
    Ok(report)
}

fn route_row(
    profile_id: &str,
    source: &ClaimCheckerSource,
) -> TassadarProposalProfileRoutePolicyRow {
    let (decision_status, note) = if source.public_profile_ids.iter().any(|id| id == profile_id) {
        (
            TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific,
            format!(
                "proposal profile `{profile_id}` is routeable only as a named profile-specific lane because the proposal-profile claim checker marks it public"
            ),
        )
    } else if source
        .suppressed_profile_ids
        .iter()
        .any(|id| id == profile_id)
    {
        (
            TassadarBroadInternalComputeRouteDecisionStatus::Suppressed,
            format!(
                "proposal profile `{profile_id}` stays suppressed because it is operator-only even though the underlying bounded artifact is green"
            ),
        )
    } else {
        (
            TassadarBroadInternalComputeRouteDecisionStatus::Refused,
            format!(
                "proposal profile `{profile_id}` stays refused because it remains research-only and must not widen served posture"
            ),
        )
    };
    TassadarProposalProfileRoutePolicyRow {
        route_policy_id: format!("route.{profile_id}.proposal_profile"),
        target_profile_id: String::from(profile_id),
        decision_status,
        note,
    }
}

#[must_use]
pub fn tassadar_proposal_profile_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PROPOSAL_PROFILE_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_proposal_profile_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarProposalProfileRoutePolicyReport, TassadarProposalProfileRoutePolicyReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarProposalProfileRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_proposal_profile_route_policy_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("proposal-profile route policy serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarProposalProfileRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_proposal_profile_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarProposalProfileRoutePolicyReport, TassadarProposalProfileRoutePolicyReportError>
{
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarProposalProfileRoutePolicyReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarProposalProfileRoutePolicyReportError::Decode {
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

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarProposalProfileRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarProposalProfileRoutePolicyReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarProposalProfileRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
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
        build_tassadar_proposal_profile_route_policy_report,
        load_tassadar_proposal_profile_route_policy_report,
        tassadar_proposal_profile_route_policy_report_path,
        TassadarProposalProfileRoutePolicyReport,
    };
    use crate::TassadarBroadInternalComputeRouteDecisionStatus;

    #[test]
    fn proposal_profile_route_policy_promotes_only_named_public_profiles() {
        let report = build_tassadar_proposal_profile_route_policy_report().expect("report");

        assert_eq!(report.promoted_profile_specific_route_count, 2);
        assert_eq!(report.suppressed_route_count, 3);
        assert_eq!(report.refused_route_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.proposal_profile.simd_deterministic.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.proposal_profile.memory64_continuation.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.research_profile.threads_deterministic_scheduler.v1"
                && row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Refused
        }));
    }

    #[test]
    fn proposal_profile_route_policy_matches_committed_truth() {
        let generated = build_tassadar_proposal_profile_route_policy_report().expect("report");
        let committed: TassadarProposalProfileRoutePolicyReport =
            load_tassadar_proposal_profile_route_policy_report(
                tassadar_proposal_profile_route_policy_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
    }
}
