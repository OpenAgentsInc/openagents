use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(not(test))]
use serde::de::DeserializeOwned;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarBroadInternalComputeRouteDecisionStatus;

const TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_session_process_profile_report.json";

pub const TASSADAR_SESSION_PROCESS_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_session_process_route_policy_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessRoutePolicyRow {
    pub route_policy_id: String,
    pub target_profile_id: String,
    pub interaction_surface_id: String,
    pub decision_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_report_ref: String,
    pub rows: Vec<TassadarSessionProcessRoutePolicyRow>,
    pub promoted_profile_specific_route_count: u32,
    pub suppressed_route_count: u32,
    pub refused_route_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SessionProcessProfileSource {
    profile_id: String,
    routeable_interaction_surface_ids: Vec<String>,
}

#[derive(Debug, Error)]
pub enum TassadarSessionProcessRoutePolicyReportError {
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

pub fn build_tassadar_session_process_route_policy_report(
) -> Result<TassadarSessionProcessRoutePolicyReport, TassadarSessionProcessRoutePolicyReportError>
{
    let profile_report: SessionProcessProfileSource =
        read_json(repo_root().join(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF))?;
    let rows = vec![
        route_row(
            "route.session_process.echo_turn_loop",
            &profile_report.profile_id,
            "deterministic_echo_turn_loop",
            profile_report
                .routeable_interaction_surface_ids
                .contains(&String::from("deterministic_echo_turn_loop")),
        ),
        route_row(
            "route.session_process.stateful_counter_turn_loop",
            &profile_report.profile_id,
            "stateful_counter_turn_loop",
            profile_report
                .routeable_interaction_surface_ids
                .contains(&String::from("stateful_counter_turn_loop")),
        ),
        refusal_row(
            "route.session_process.open_ended_external",
            &profile_report.profile_id,
            "open_ended_external_event_stream",
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
        .filter(|row| row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Refused)
        .count() as u32;
    let mut report = TassadarSessionProcessRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.session_process_route_policy.report.v1"),
        profile_report_ref: String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
        rows,
        promoted_profile_specific_route_count,
        suppressed_route_count,
        refused_route_count,
        claim_boundary: String::from(
            "this router report promotes only the bounded deterministic session-process interaction surfaces as profile-specific routes. It keeps open-ended external event streams explicitly refused and does not widen the session-process profile into a default served route",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Session-process route policy now records promoted_profile_specific_routes={}, suppressed_routes={}, refused_routes={}.",
        report.promoted_profile_specific_route_count,
        report.suppressed_route_count,
        report.refused_route_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_session_process_route_policy_report|", &report);
    Ok(report)
}

fn route_row(
    route_policy_id: &str,
    profile_id: &str,
    interaction_surface_id: &str,
    promoted: bool,
) -> TassadarSessionProcessRoutePolicyRow {
    TassadarSessionProcessRoutePolicyRow {
        route_policy_id: String::from(route_policy_id),
        target_profile_id: String::from(profile_id),
        interaction_surface_id: String::from(interaction_surface_id),
        decision_status: if promoted {
            TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        } else {
            TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        },
        note: if promoted {
            format!(
                "interactive surface `{interaction_surface_id}` is routeable only as a named profile-specific session-process lane"
            )
        } else {
            format!(
                "interactive surface `{interaction_surface_id}` stays suppressed because the bounded session-process profile did not keep it green"
            )
        },
    }
}

fn refusal_row(
    route_policy_id: &str,
    profile_id: &str,
    interaction_surface_id: &str,
) -> TassadarSessionProcessRoutePolicyRow {
    TassadarSessionProcessRoutePolicyRow {
        route_policy_id: String::from(route_policy_id),
        target_profile_id: String::from(profile_id),
        interaction_surface_id: String::from(interaction_surface_id),
        decision_status: TassadarBroadInternalComputeRouteDecisionStatus::Refused,
        note: format!(
            "interactive surface `{interaction_surface_id}` remains explicitly refused because it depends on open-ended external events outside the bounded session-process envelope"
        ),
    }
}

#[must_use]
pub fn tassadar_session_process_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SESSION_PROCESS_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_session_process_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSessionProcessRoutePolicyReport, TassadarSessionProcessRoutePolicyReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSessionProcessRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_session_process_route_policy_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("session-process route policy serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarSessionProcessRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_session_process_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSessionProcessRoutePolicyReport, TassadarSessionProcessRoutePolicyReportError>
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSessionProcessRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSessionProcessRoutePolicyReportError::Decode {
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
) -> Result<T, TassadarSessionProcessRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSessionProcessRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSessionProcessRoutePolicyReportError::Decode {
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
        build_tassadar_session_process_route_policy_report,
        load_tassadar_session_process_route_policy_report,
        tassadar_session_process_route_policy_report_path,
        write_tassadar_session_process_route_policy_report,
    };
    use crate::TassadarBroadInternalComputeRouteDecisionStatus;

    #[test]
    fn session_process_route_policy_promotes_bounded_interactive_surfaces_only() {
        let report = build_tassadar_session_process_route_policy_report().expect("report");

        assert_eq!(report.promoted_profile_specific_route_count, 2);
        assert_eq!(report.refused_route_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.interaction_surface_id == "deterministic_echo_turn_loop"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.interaction_surface_id == "open_ended_external_event_stream"
                && row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Refused
        }));
    }

    #[test]
    fn session_process_route_policy_matches_committed_truth() {
        let generated = build_tassadar_session_process_route_policy_report().expect("report");
        let committed = load_tassadar_session_process_route_policy_report(
            tassadar_session_process_route_policy_report_path(),
        )
        .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_session_process_route_policy_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_session_process_route_policy_report.json");
        let generated =
            write_tassadar_session_process_route_policy_report(&output_path).expect("report");
        let reloaded =
            load_tassadar_session_process_route_policy_report(&output_path).expect("reloaded");

        assert_eq!(generated, reloaded);
    }
}
