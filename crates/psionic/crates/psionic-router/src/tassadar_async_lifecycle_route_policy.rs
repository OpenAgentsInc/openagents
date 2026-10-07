use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarBroadInternalComputeRouteDecisionStatus;

const TASSADAR_ASYNC_LIFECYCLE_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json";

pub const TASSADAR_ASYNC_LIFECYCLE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_async_lifecycle_route_policy_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleRoutePolicyRow {
    pub route_policy_id: String,
    pub target_profile_id: String,
    pub lifecycle_surface_id: String,
    pub decision_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_report_ref: String,
    pub rows: Vec<TassadarAsyncLifecycleRoutePolicyRow>,
    pub promoted_profile_specific_route_count: u32,
    pub suppressed_route_count: u32,
    pub refused_route_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct AsyncLifecycleProfileSource {
    profile_id: String,
    routeable_lifecycle_surface_ids: Vec<String>,
}

#[derive(Debug, Error)]
pub enum TassadarAsyncLifecycleRoutePolicyReportError {
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

pub fn build_tassadar_async_lifecycle_route_policy_report(
) -> Result<TassadarAsyncLifecycleRoutePolicyReport, TassadarAsyncLifecycleRoutePolicyReportError>
{
    let profile_report: AsyncLifecycleProfileSource =
        read_json(repo_root().join(TASSADAR_ASYNC_LIFECYCLE_PROFILE_REPORT_REF))?;
    let rows = vec![
        route_row(
            "route.async_lifecycle.interruptible_counter_job",
            &profile_report.profile_id,
            "interruptible_counter_job",
            profile_report
                .routeable_lifecycle_surface_ids
                .contains(&String::from("interruptible_counter_job")),
        ),
        route_row(
            "route.async_lifecycle.retryable_timeout_search_job",
            &profile_report.profile_id,
            "retryable_timeout_search_job",
            profile_report
                .routeable_lifecycle_surface_ids
                .contains(&String::from("retryable_timeout_search_job")),
        ),
        route_row(
            "route.async_lifecycle.safe_boundary_cancellation_job",
            &profile_report.profile_id,
            "safe_boundary_cancellation_job",
            profile_report
                .routeable_lifecycle_surface_ids
                .contains(&String::from("safe_boundary_cancellation_job")),
        ),
        refusal_row(
            "route.async_lifecycle.open_ended_external_callback",
            &profile_report.profile_id,
            "open_ended_external_callback",
        ),
        refusal_row(
            "route.async_lifecycle.mid_effect_cancellation",
            &profile_report.profile_id,
            "mid_effect_cancellation",
        ),
        refusal_row(
            "route.async_lifecycle.unbounded_retry_backoff",
            &profile_report.profile_id,
            "unbounded_retry_backoff",
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
    let mut report = TassadarAsyncLifecycleRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.async_lifecycle_route_policy.report.v1"),
        profile_report_ref: String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_REPORT_REF),
        rows,
        promoted_profile_specific_route_count,
        suppressed_route_count,
        refused_route_count,
        claim_boundary: String::from(
            "this router report promotes only the bounded deterministic async-lifecycle surfaces as profile-specific routes. It keeps open-ended callbacks, mid-effect cancellation, and unbounded retry explicitly refused and does not widen the async-lifecycle profile into a default served route",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Async-lifecycle route policy now records promoted_profile_specific_routes={}, suppressed_routes={}, refused_routes={}.",
        report.promoted_profile_specific_route_count,
        report.suppressed_route_count,
        report.refused_route_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_async_lifecycle_route_policy_report|", &report);
    Ok(report)
}

fn route_row(
    route_policy_id: &str,
    profile_id: &str,
    lifecycle_surface_id: &str,
    promoted: bool,
) -> TassadarAsyncLifecycleRoutePolicyRow {
    TassadarAsyncLifecycleRoutePolicyRow {
        route_policy_id: String::from(route_policy_id),
        target_profile_id: String::from(profile_id),
        lifecycle_surface_id: String::from(lifecycle_surface_id),
        decision_status: if promoted {
            TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        } else {
            TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        },
        note: if promoted {
            format!(
                "lifecycle surface `{lifecycle_surface_id}` is routeable only as a named profile-specific async-lifecycle lane"
            )
        } else {
            format!(
                "lifecycle surface `{lifecycle_surface_id}` stays suppressed because the bounded async-lifecycle profile did not keep it green"
            )
        },
    }
}

fn refusal_row(
    route_policy_id: &str,
    profile_id: &str,
    lifecycle_surface_id: &str,
) -> TassadarAsyncLifecycleRoutePolicyRow {
    TassadarAsyncLifecycleRoutePolicyRow {
        route_policy_id: String::from(route_policy_id),
        target_profile_id: String::from(profile_id),
        lifecycle_surface_id: String::from(lifecycle_surface_id),
        decision_status: TassadarBroadInternalComputeRouteDecisionStatus::Refused,
        note: format!(
            "lifecycle surface `{lifecycle_surface_id}` remains explicitly refused because it depends on async behavior outside the bounded interrupt/retry/cancel envelope"
        ),
    }
}

#[must_use]
pub fn tassadar_async_lifecycle_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ASYNC_LIFECYCLE_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_async_lifecycle_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarAsyncLifecycleRoutePolicyReport, TassadarAsyncLifecycleRoutePolicyReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarAsyncLifecycleRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_async_lifecycle_route_policy_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("async-lifecycle route policy should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarAsyncLifecycleRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_async_lifecycle_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarAsyncLifecycleRoutePolicyReport, TassadarAsyncLifecycleRoutePolicyReportError>
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarAsyncLifecycleRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarAsyncLifecycleRoutePolicyReportError::Decode {
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
) -> Result<T, TassadarAsyncLifecycleRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarAsyncLifecycleRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarAsyncLifecycleRoutePolicyReportError::Decode {
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
        build_tassadar_async_lifecycle_route_policy_report,
        load_tassadar_async_lifecycle_route_policy_report,
        tassadar_async_lifecycle_route_policy_report_path,
        write_tassadar_async_lifecycle_route_policy_report,
    };
    use crate::TassadarBroadInternalComputeRouteDecisionStatus;

    #[test]
    fn async_lifecycle_route_policy_promotes_bounded_lifecycle_surfaces_only() {
        let report = build_tassadar_async_lifecycle_route_policy_report().expect("report");

        assert_eq!(report.promoted_profile_specific_route_count, 3);
        assert_eq!(report.refused_route_count, 3);
        assert!(report.rows.iter().any(|row| {
            row.lifecycle_surface_id == "interruptible_counter_job"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.lifecycle_surface_id == "open_ended_external_callback"
                && row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Refused
        }));
    }

    #[test]
    fn async_lifecycle_route_policy_matches_committed_truth() {
        let generated = build_tassadar_async_lifecycle_route_policy_report().expect("report");
        let committed = load_tassadar_async_lifecycle_route_policy_report(
            tassadar_async_lifecycle_route_policy_report_path(),
        )
        .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_async_lifecycle_route_policy_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_async_lifecycle_route_policy_report.json");
        let report = write_tassadar_async_lifecycle_route_policy_report(&output_path)
            .expect("write report");
        let reloaded =
            load_tassadar_async_lifecycle_route_policy_report(&output_path).expect("reload");

        assert_eq!(report, reloaded);
    }
}
