use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID, TASSADAR_ASYNC_LIFECYCLE_PROFILE_RUNTIME_REPORT_REF,
    TassadarAsyncLifecycleCaseStatus,
    TassadarAsyncLifecycleProfileRuntimeReport as TassadarAsyncLifecycleRuntimeReport,
    build_tassadar_async_lifecycle_profile_runtime_report,
};

pub const TASSADAR_ASYNC_LIFECYCLE_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleProfileCaseAudit {
    pub case_id: String,
    pub process_id: String,
    pub lifecycle_surface_id: String,
    pub state_shape_id: String,
    pub runtime_status: TassadarAsyncLifecycleCaseStatus,
    pub exact_interrupt_parity: bool,
    pub exact_retry_parity: bool,
    pub exact_cancellation_parity: bool,
    pub retry_attempt_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub route_eligible: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_report_ref: String,
    pub runtime_report: TassadarAsyncLifecycleRuntimeReport,
    pub case_audits: Vec<TassadarAsyncLifecycleProfileCaseAudit>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub overall_green: bool,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub routeable_lifecycle_surface_ids: Vec<String>,
    pub refused_lifecycle_surface_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarAsyncLifecycleProfileReportError {
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

pub fn build_tassadar_async_lifecycle_profile_report(
) -> Result<TassadarAsyncLifecycleProfileReport, TassadarAsyncLifecycleProfileReportError> {
    let runtime_report = build_tassadar_async_lifecycle_profile_runtime_report();
    let case_audits = runtime_report
        .rows
        .iter()
        .map(|row| TassadarAsyncLifecycleProfileCaseAudit {
            case_id: row.case_id.clone(),
            process_id: row.process_id.clone(),
            lifecycle_surface_id: row.lifecycle_surface_id.clone(),
            state_shape_id: row.state_shape_id.clone(),
            runtime_status: row.status,
            exact_interrupt_parity: row.exact_interrupt_parity,
            exact_retry_parity: row.exact_retry_parity,
            exact_cancellation_parity: row.exact_cancellation_parity,
            retry_attempt_count: row.retry_attempt_count,
            refusal_reason_id: row.refusal_reason_id.clone(),
            route_eligible: row.status == TassadarAsyncLifecycleCaseStatus::ExactDeterministicParity,
            note: row.note.clone(),
        })
        .collect::<Vec<_>>();
    let exact_case_count = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarAsyncLifecycleCaseStatus::ExactDeterministicParity)
        .count() as u32;
    let refusal_case_count = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarAsyncLifecycleCaseStatus::ExactRefusalParity)
        .count() as u32;
    let routeable_lifecycle_surface_ids = case_audits
        .iter()
        .filter(|case| case.route_eligible)
        .map(|case| case.lifecycle_surface_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let refused_lifecycle_surface_ids = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarAsyncLifecycleCaseStatus::ExactRefusalParity)
        .map(|case| case.lifecycle_surface_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let overall_green = runtime_report.overall_green
        && exact_case_count == runtime_report.exact_case_count
        && refusal_case_count == runtime_report.refusal_case_count
        && !routeable_lifecycle_surface_ids.is_empty();
    let mut report = TassadarAsyncLifecycleProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.async_lifecycle_profile.report.v1"),
        profile_id: String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID),
        runtime_report_ref: String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_RUNTIME_REPORT_REF),
        runtime_report,
        case_audits,
        exact_case_count,
        refusal_case_count,
        overall_green,
        public_profile_allowed_profile_ids: if overall_green {
            vec![String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID)]
        } else {
            Vec::new()
        },
        default_served_profile_allowed_profile_ids: Vec::new(),
        routeable_lifecycle_surface_ids,
        refused_lifecycle_surface_ids,
        claim_boundary: String::from(
            "this eval report covers one bounded async-lifecycle profile with deterministic interrupt, bounded retry, and safe-boundary cancellation semantics. It admits named public posture only for the deterministic lifecycle surfaces in this report and keeps open-ended callbacks, mid-effect cancellation, and unbounded retry on explicit refusal paths instead of implying broad async execution or default served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Async-lifecycle profile report covers exact_cases={}, refusal_cases={}, routeable_surfaces={}, refused_surfaces={}, overall_green={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.routeable_lifecycle_surface_ids.len(),
        report.refused_lifecycle_surface_ids.len(),
        report.overall_green,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_async_lifecycle_profile_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_async_lifecycle_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ASYNC_LIFECYCLE_PROFILE_REPORT_REF)
}

pub fn write_tassadar_async_lifecycle_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarAsyncLifecycleProfileReport, TassadarAsyncLifecycleProfileReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarAsyncLifecycleProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_async_lifecycle_profile_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarAsyncLifecycleProfileReportError::Write {
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
) -> Result<T, TassadarAsyncLifecycleProfileReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarAsyncLifecycleProfileReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarAsyncLifecycleProfileReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_async_lifecycle_profile_report, read_json,
        tassadar_async_lifecycle_profile_report_path,
        write_tassadar_async_lifecycle_profile_report,
    };
    use psionic_runtime::TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID;
    use tempfile::tempdir;

    #[test]
    fn async_lifecycle_profile_report_keeps_named_public_routeability_bounded() {
        let report = build_tassadar_async_lifecycle_profile_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.profile_id, TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID);
        assert_eq!(report.exact_case_count, 3);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(
            report.public_profile_allowed_profile_ids,
            vec![String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID)]
        );
        assert!(report.default_served_profile_allowed_profile_ids.is_empty());
        assert!(report
            .routeable_lifecycle_surface_ids
            .contains(&String::from("interruptible_counter_job")));
        assert!(report
            .routeable_lifecycle_surface_ids
            .contains(&String::from("retryable_timeout_search_job")));
        assert!(report
            .routeable_lifecycle_surface_ids
            .contains(&String::from("safe_boundary_cancellation_job")));
        assert!(report
            .refused_lifecycle_surface_ids
            .contains(&String::from("open_ended_external_callback")));
    }

    #[test]
    fn async_lifecycle_profile_report_matches_committed_truth() {
        let generated = build_tassadar_async_lifecycle_profile_report().expect("report");
        let committed = read_json(tassadar_async_lifecycle_profile_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_async_lifecycle_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_async_lifecycle_profile_report.json");
        let report =
            write_tassadar_async_lifecycle_profile_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_async_lifecycle_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_async_lifecycle_profile_report.json")
        );
    }
}
