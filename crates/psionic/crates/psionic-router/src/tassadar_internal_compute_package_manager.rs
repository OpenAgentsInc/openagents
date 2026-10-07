use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{
    TassadarInternalComputePackageSolverRefusalReason,
    TassadarInternalComputePackageSolverStatus,
    build_tassadar_internal_compute_package_manager_report,
};

pub const TASSADAR_INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_package_route_policy_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComputePackageRouteOutcome {
    Selected,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageRouteCaseReport {
    pub case_id: String,
    pub workload_family: String,
    pub outcome: TassadarInternalComputePackageRouteOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_package_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarInternalComputePackageSolverRefusalReason>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputePackageRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub selected_case_count: u32,
    pub refused_case_count: u32,
    pub named_public_package_ids: Vec<String>,
    pub default_served_package_ids: Vec<String>,
    pub routeable_package_ids: Vec<String>,
    pub refused_package_case_ids: Vec<String>,
    pub case_reports: Vec<TassadarInternalComputePackageRouteCaseReport>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalComputePackageRoutePolicyReportError {
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

#[must_use]
pub fn build_tassadar_internal_compute_package_route_policy_report(
) -> TassadarInternalComputePackageRoutePolicyReport {
    let manager = build_tassadar_internal_compute_package_manager_report();
    let case_reports = manager
        .solver_cases
        .iter()
        .map(|case| match case.status {
            TassadarInternalComputePackageSolverStatus::Exact => {
                TassadarInternalComputePackageRouteCaseReport {
                    case_id: case.case_id.clone(),
                    workload_family: case.workload_family.clone(),
                    outcome: TassadarInternalComputePackageRouteOutcome::Selected,
                    selected_package_id: case.selected_package_id.clone(),
                    refusal_reason: None,
                    note: case.note.clone(),
                }
            }
            TassadarInternalComputePackageSolverStatus::Refusal => {
                TassadarInternalComputePackageRouteCaseReport {
                    case_id: case.case_id.clone(),
                    workload_family: case.workload_family.clone(),
                    outcome: TassadarInternalComputePackageRouteOutcome::Refused,
                    selected_package_id: None,
                    refusal_reason: case.refusal_reason,
                    note: case.note.clone(),
                }
            }
        })
        .collect::<Vec<_>>();
    let routeable_package_ids = manager.public_package_ids.clone();
    let refused_package_case_ids = case_reports
        .iter()
        .filter(|case| case.outcome == TassadarInternalComputePackageRouteOutcome::Refused)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarInternalComputePackageRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.internal_compute_package_route_policy.report.v1"),
        selected_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarInternalComputePackageRouteOutcome::Selected)
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarInternalComputePackageRouteOutcome::Refused)
            .count() as u32,
        named_public_package_ids: manager.public_package_ids,
        default_served_package_ids: Vec::new(),
        routeable_package_ids,
        refused_package_case_ids,
        case_reports,
        overall_green: true,
        claim_boundary: String::from(
            "this router report freezes named-public and routeable internal-compute packages plus explicit refusal on ambiguous solver, insufficient-evidence, and portability-mismatch requests. It does not imply arbitrary package discovery or a default served package lane",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.selected_case_count == 3
        && report.refused_case_count == 3
        && report.default_served_package_ids.is_empty();
    report.summary = format!(
        "Internal compute package route policy freezes {} selected cases, {} refused cases, {} named-public packages, and {} default-served packages.",
        report.selected_case_count,
        report.refused_case_count,
        report.named_public_package_ids.len(),
        report.default_served_package_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_internal_compute_package_route_policy_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_internal_compute_package_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPUTE_PACKAGE_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_internal_compute_package_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalComputePackageRoutePolicyReport, TassadarInternalComputePackageRoutePolicyReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComputePackageRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_compute_package_route_policy_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalComputePackageRoutePolicyReportError::Write {
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
) -> Result<T, TassadarInternalComputePackageRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarInternalComputePackageRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalComputePackageRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_internal_compute_package_route_policy_report, read_json,
        tassadar_internal_compute_package_route_policy_report_path,
        write_tassadar_internal_compute_package_route_policy_report,
    };

    #[test]
    fn internal_compute_package_route_policy_report_keeps_named_public_and_refusals_explicit() {
        let report = build_tassadar_internal_compute_package_route_policy_report();

        assert_eq!(report.selected_case_count, 3);
        assert_eq!(report.refused_case_count, 3);
        assert_eq!(report.named_public_package_ids.len(), 3);
        assert!(report.default_served_package_ids.is_empty());
        assert!(report.overall_green);
    }

    #[test]
    fn internal_compute_package_route_policy_report_matches_committed_truth() {
        let generated = build_tassadar_internal_compute_package_route_policy_report();
        let committed = read_json(tassadar_internal_compute_package_route_policy_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_compute_package_route_policy_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_internal_compute_package_route_policy_report.json");
        let report = write_tassadar_internal_compute_package_route_policy_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
