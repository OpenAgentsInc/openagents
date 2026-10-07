use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_SEMANTIC_WINDOW_MIGRATION_REPORT_REF, TassadarSemanticWindowMigrationStatus,
    build_tassadar_semantic_window_migration_report,
};

pub const TASSADAR_SEMANTIC_WINDOW_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_semantic_window_route_policy_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSemanticWindowRouteDecision {
    SelectedActiveWindow,
    DowngradedNamedProfiles,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowRouteCase {
    pub case_id: String,
    pub requested_window_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_window_id: Option<String>,
    pub effective_profile_ids: Vec<String>,
    pub decision: TassadarSemanticWindowRouteDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub migration_report_ref: String,
    pub selected_requested_window_ids: Vec<String>,
    pub downgraded_requested_window_ids: Vec<String>,
    pub refused_requested_window_ids: Vec<String>,
    pub case_reports: Vec<TassadarSemanticWindowRouteCase>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSemanticWindowRoutePolicyReportError {
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
pub fn build_tassadar_semantic_window_route_policy_report()
-> TassadarSemanticWindowRoutePolicyReport {
    let migration_report = build_tassadar_semantic_window_migration_report();
    let mut case_reports = migration_report
        .case_receipts
        .iter()
        .map(|case| TassadarSemanticWindowRouteCase {
            case_id: case.case_id.clone(),
            requested_window_id: case.requested_window_id.clone(),
            effective_window_id: case.effective_window_id.clone(),
            effective_profile_ids: case.effective_profile_ids.clone(),
            decision: match case.status {
                TassadarSemanticWindowMigrationStatus::Exact => {
                    TassadarSemanticWindowRouteDecision::SelectedActiveWindow
                }
                TassadarSemanticWindowMigrationStatus::Downgraded => {
                    TassadarSemanticWindowRouteDecision::DowngradedNamedProfiles
                }
                TassadarSemanticWindowMigrationStatus::Refused => {
                    TassadarSemanticWindowRouteDecision::Refused
                }
            },
            refusal_reason_id: case.refusal_reason_id.clone(),
            detail: case.detail.clone(),
        })
        .collect::<Vec<_>>();
    case_reports.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let selected_requested_window_ids = case_reports
        .iter()
        .filter(|case| case.decision == TassadarSemanticWindowRouteDecision::SelectedActiveWindow)
        .map(|case| case.requested_window_id.clone())
        .collect::<Vec<_>>();
    let downgraded_requested_window_ids = case_reports
        .iter()
        .filter(|case| {
            case.decision == TassadarSemanticWindowRouteDecision::DowngradedNamedProfiles
        })
        .map(|case| case.requested_window_id.clone())
        .collect::<Vec<_>>();
    let refused_requested_window_ids = case_reports
        .iter()
        .filter(|case| case.decision == TassadarSemanticWindowRouteDecision::Refused)
        .map(|case| case.requested_window_id.clone())
        .collect::<Vec<_>>();

    let mut report = TassadarSemanticWindowRoutePolicyReport {
        schema_version: 1,
        report_id: String::from("tassadar.semantic_window_route_policy.report.v1"),
        migration_report_ref: String::from(TASSADAR_SEMANTIC_WINDOW_MIGRATION_REPORT_REF),
        selected_requested_window_ids,
        downgraded_requested_window_ids,
        refused_requested_window_ids,
        case_reports,
        overall_green: false,
        claim_boundary: String::from(
            "this router report freezes active-window selection, named-profile downgrade, and typed refusal for declared semantic-window requests. It does not flatten all windows into a single superset or widen served posture implicitly",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.selected_requested_window_ids.len() == 2
        && report.downgraded_requested_window_ids.len() == 1
        && report.refused_requested_window_ids.len() == 2;
    report.summary = format!(
        "Semantic-window route policy keeps selected_windows={}, downgraded_windows={}, refused_windows={}, overall_green={}.",
        report.selected_requested_window_ids.len(),
        report.downgraded_requested_window_ids.len(),
        report.refused_requested_window_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_semantic_window_route_policy_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_semantic_window_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SEMANTIC_WINDOW_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_semantic_window_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSemanticWindowRoutePolicyReport, TassadarSemanticWindowRoutePolicyReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSemanticWindowRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_semantic_window_route_policy_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSemanticWindowRoutePolicyReportError::Write {
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
) -> Result<T, TassadarSemanticWindowRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarSemanticWindowRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSemanticWindowRoutePolicyReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarSemanticWindowRouteDecision, TassadarSemanticWindowRoutePolicyReport,
        build_tassadar_semantic_window_route_policy_report, read_json,
        tassadar_semantic_window_route_policy_report_path,
    };

    #[test]
    fn semantic_window_route_policy_keeps_selected_downgraded_and_refused_requests_separate() {
        let report = build_tassadar_semantic_window_route_policy_report();

        assert_eq!(report.selected_requested_window_ids.len(), 2);
        assert_eq!(report.downgraded_requested_window_ids.len(), 1);
        assert_eq!(report.refused_requested_window_ids.len(), 2);
        assert!(report.case_reports.iter().any(|case| {
            case.requested_window_id == "tassadar.frozen_core_wasm.window.v1_plus_public_proposals"
                && case.decision == TassadarSemanticWindowRouteDecision::DowngradedNamedProfiles
        }));
    }

    #[test]
    fn semantic_window_route_policy_matches_committed_truth() {
        let generated = build_tassadar_semantic_window_route_policy_report();
        let committed: TassadarSemanticWindowRoutePolicyReport =
            read_json(tassadar_semantic_window_route_policy_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
