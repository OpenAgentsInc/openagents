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
    TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF,
    build_tassadar_cross_profile_link_compatibility_report,
};
use psionic_runtime::{
    TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE,
    TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF, TassadarCrossProfileLinkRuntimeStatus,
    build_tassadar_cross_profile_link_compatibility_runtime_bundle,
};

pub const TASSADAR_CROSS_PROFILE_LINK_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_cross_profile_link_route_policy_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCrossProfileLinkRouteDecision {
    Selected,
    Downgraded,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkRouteCaseReport {
    pub case_id: String,
    pub decision: TassadarCrossProfileLinkRouteDecision,
    pub producer_profile_id: String,
    pub consumer_profile_id: String,
    pub effective_profile_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downgrade_target_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub compiler_report_ref: String,
    pub runtime_bundle_ref: String,
    pub selected_case_count: u32,
    pub downgraded_case_count: u32,
    pub refused_case_count: u32,
    pub routeable_case_ids: Vec<String>,
    pub downgraded_case_ids: Vec<String>,
    pub refused_case_ids: Vec<String>,
    pub case_reports: Vec<TassadarCrossProfileLinkRouteCaseReport>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarCrossProfileLinkRoutePolicyReportError {
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
pub fn build_tassadar_cross_profile_link_route_policy_report()
-> TassadarCrossProfileLinkRoutePolicyReport {
    let compiler_report = build_tassadar_cross_profile_link_compatibility_report();
    let runtime_bundle = build_tassadar_cross_profile_link_compatibility_runtime_bundle();
    let case_reports = runtime_bundle
        .case_receipts
        .iter()
        .map(|case| TassadarCrossProfileLinkRouteCaseReport {
            case_id: case.case_id.clone(),
            decision: match case.runtime_status {
                TassadarCrossProfileLinkRuntimeStatus::Exact => {
                    TassadarCrossProfileLinkRouteDecision::Selected
                }
                TassadarCrossProfileLinkRuntimeStatus::Downgraded => {
                    TassadarCrossProfileLinkRouteDecision::Downgraded
                }
                TassadarCrossProfileLinkRuntimeStatus::Refused => {
                    TassadarCrossProfileLinkRouteDecision::Refused
                }
            },
            producer_profile_id: case.producer_profile_id.clone(),
            consumer_profile_id: case.consumer_profile_id.clone(),
            effective_profile_ids: case.realized_profile_ids.clone(),
            downgrade_target_profile_id: case.downgrade_target_profile_id.clone(),
            refusal_reason_id: case.refusal_reason_id.clone(),
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let routeable_case_ids = case_reports
        .iter()
        .filter(|case| {
            matches!(
                case.decision,
                TassadarCrossProfileLinkRouteDecision::Selected
                    | TassadarCrossProfileLinkRouteDecision::Downgraded
            )
        })
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let downgraded_case_ids = case_reports
        .iter()
        .filter(|case| case.decision == TassadarCrossProfileLinkRouteDecision::Downgraded)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let refused_case_ids = case_reports
        .iter()
        .filter(|case| case.decision == TassadarCrossProfileLinkRouteDecision::Refused)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarCrossProfileLinkRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.cross_profile_link_route_policy.report.v1"),
        compiler_report_ref: String::from(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF),
        runtime_bundle_ref: format!(
            "{}/{}",
            TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF,
            TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE
        ),
        selected_case_count: 0,
        downgraded_case_count: 0,
        refused_case_count: 0,
        routeable_case_ids,
        downgraded_case_ids,
        refused_case_ids,
        case_reports,
        overall_green: false,
        claim_boundary: String::from(
            "this router report freezes bounded cross-profile linking route decisions over explicit exact selection, downgrade planning, and compatibility refusal. It does not imply generic profile flattening, automatic portability lifting, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.selected_case_count = report
        .case_reports
        .iter()
        .filter(|case| case.decision == TassadarCrossProfileLinkRouteDecision::Selected)
        .count() as u32;
    report.downgraded_case_count = report
        .case_reports
        .iter()
        .filter(|case| case.decision == TassadarCrossProfileLinkRouteDecision::Downgraded)
        .count() as u32;
    report.refused_case_count = report
        .case_reports
        .iter()
        .filter(|case| case.decision == TassadarCrossProfileLinkRouteDecision::Refused)
        .count() as u32;
    report.overall_green = report.selected_case_count == compiler_report.exact_case_count
        && report.downgraded_case_count == compiler_report.downgraded_case_count
        && report.refused_case_count == compiler_report.refused_case_count;
    report.summary = format!(
        "Cross-profile link route policy freezes selected_cases={}, downgraded_cases={}, refused_cases={}, routeable_cases={}.",
        report.selected_case_count,
        report.downgraded_case_count,
        report.refused_case_count,
        report.routeable_case_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_cross_profile_link_route_policy_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_cross_profile_link_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CROSS_PROFILE_LINK_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_cross_profile_link_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCrossProfileLinkRoutePolicyReport, TassadarCrossProfileLinkRoutePolicyReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCrossProfileLinkRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_cross_profile_link_route_policy_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCrossProfileLinkRoutePolicyReportError::Write {
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
) -> Result<T, TassadarCrossProfileLinkRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarCrossProfileLinkRoutePolicyReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCrossProfileLinkRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TassadarCrossProfileLinkRouteDecision,
        build_tassadar_cross_profile_link_route_policy_report, read_json,
        tassadar_cross_profile_link_route_policy_report_path,
        write_tassadar_cross_profile_link_route_policy_report,
    };

    #[test]
    fn cross_profile_link_route_policy_report_keeps_selected_downgraded_and_refused_routes_explicit()
     {
        let report = build_tassadar_cross_profile_link_route_policy_report();

        assert_eq!(report.selected_case_count, 1);
        assert_eq!(report.downgraded_case_count, 1);
        assert_eq!(report.refused_case_count, 2);
        assert!(report.overall_green);
        assert!(report.case_reports.iter().any(|case| {
            case.decision == TassadarCrossProfileLinkRouteDecision::Downgraded
                && case.downgrade_target_profile_id.as_deref()
                    == Some("tassadar.internal_compute.component_model_abi.v1")
        }));
    }

    #[test]
    fn cross_profile_link_route_policy_report_matches_committed_truth() {
        let generated = build_tassadar_cross_profile_link_route_policy_report();
        let committed = read_json(tassadar_cross_profile_link_route_policy_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_cross_profile_link_route_policy_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_cross_profile_link_route_policy_report.json");
        let report = write_tassadar_cross_profile_link_route_policy_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
