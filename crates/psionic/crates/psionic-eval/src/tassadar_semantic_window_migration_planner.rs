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
    TASSADAR_SEMANTIC_WINDOW_COMPATIBILITY_DELTA_REPORT_REF,
    TassadarSemanticWindowCompatibilityDeltaReport,
    build_tassadar_semantic_window_compatibility_delta_report,
};
use psionic_router::{
    TASSADAR_SEMANTIC_WINDOW_ROUTE_POLICY_REPORT_REF, TassadarSemanticWindowRoutePolicyReport,
    build_tassadar_semantic_window_route_policy_report,
};
use psionic_runtime::{
    TASSADAR_SEMANTIC_WINDOW_MIGRATION_REPORT_REF, TassadarSemanticWindowMigrationReport,
    build_tassadar_semantic_window_migration_report,
};

pub const TASSADAR_SEMANTIC_WINDOW_MIGRATION_PLANNER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_semantic_window_migration_planner_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowMigrationPlannerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub migration_report_ref: String,
    pub migration_report: TassadarSemanticWindowMigrationReport,
    pub compatibility_delta_report_ref: String,
    pub compatibility_delta_report: TassadarSemanticWindowCompatibilityDeltaReport,
    pub route_policy_report_ref: String,
    pub route_policy_report: TassadarSemanticWindowRoutePolicyReport,
    pub active_window_id: String,
    pub selected_requested_window_ids: Vec<String>,
    pub downgraded_requested_window_ids: Vec<String>,
    pub refused_requested_window_ids: Vec<String>,
    pub served_publication_allowed_window_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSemanticWindowMigrationPlannerReportError {
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

pub fn build_tassadar_semantic_window_migration_planner_report() -> Result<
    TassadarSemanticWindowMigrationPlannerReport,
    TassadarSemanticWindowMigrationPlannerReportError,
> {
    let migration_report = build_tassadar_semantic_window_migration_report();
    let compatibility_delta_report = build_tassadar_semantic_window_compatibility_delta_report()
        .expect("compatibility delta should build");
    let route_policy_report = build_tassadar_semantic_window_route_policy_report();
    let active_window_id = migration_report.active_window_id.clone();
    let selected_requested_window_ids = route_policy_report.selected_requested_window_ids.clone();
    let downgraded_requested_window_ids =
        route_policy_report.downgraded_requested_window_ids.clone();
    let refused_requested_window_ids = route_policy_report.refused_requested_window_ids.clone();
    let served_publication_allowed_window_ids = Vec::new();
    let mut report = TassadarSemanticWindowMigrationPlannerReport {
        schema_version: 1,
        report_id: String::from("tassadar.semantic_window_migration_planner.report.v1"),
        migration_report_ref: String::from(TASSADAR_SEMANTIC_WINDOW_MIGRATION_REPORT_REF),
        migration_report,
        compatibility_delta_report_ref: String::from(
            TASSADAR_SEMANTIC_WINDOW_COMPATIBILITY_DELTA_REPORT_REF,
        ),
        compatibility_delta_report,
        route_policy_report_ref: String::from(TASSADAR_SEMANTIC_WINDOW_ROUTE_POLICY_REPORT_REF),
        route_policy_report,
        active_window_id,
        selected_requested_window_ids,
        downgraded_requested_window_ids,
        refused_requested_window_ids,
        served_publication_allowed_window_ids,
        overall_green: false,
        claim_boundary: String::from(
            "this eval report joins semantic-window revision, migration, and route posture into one downgrade-or-refuse planner view. It keeps the active window explicit, permits only bounded exact or downgrade paths, keeps served publication by semantic window empty, and refuses stale or evidence-bound requests instead of widening implicitly",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.selected_requested_window_ids.len() == 2
        && report.downgraded_requested_window_ids.len() == 1
        && report.refused_requested_window_ids.len() == 2
        && report
            .compatibility_delta_report
            .compatible_candidate_window_ids
            .len()
            == 1
        && report.served_publication_allowed_window_ids.is_empty()
        && report.route_policy_report.overall_green;
    report.summary = format!(
        "Semantic-window migration planner keeps active_window_id={}, selected_windows={}, downgraded_windows={}, refused_windows={}, served_publication_allowed_windows={}, overall_green={}.",
        report.active_window_id,
        report.selected_requested_window_ids.len(),
        report.downgraded_requested_window_ids.len(),
        report.refused_requested_window_ids.len(),
        report.served_publication_allowed_window_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_semantic_window_migration_planner_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_semantic_window_migration_planner_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SEMANTIC_WINDOW_MIGRATION_PLANNER_REPORT_REF)
}

pub fn write_tassadar_semantic_window_migration_planner_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarSemanticWindowMigrationPlannerReport,
    TassadarSemanticWindowMigrationPlannerReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSemanticWindowMigrationPlannerReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_semantic_window_migration_planner_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSemanticWindowMigrationPlannerReportError::Write {
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarSemanticWindowMigrationPlannerReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSemanticWindowMigrationPlannerReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSemanticWindowMigrationPlannerReportError::Decode {
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
        TassadarSemanticWindowMigrationPlannerReport,
        build_tassadar_semantic_window_migration_planner_report, read_json,
        tassadar_semantic_window_migration_planner_report_path,
    };

    #[test]
    fn semantic_window_migration_planner_keeps_selected_downgraded_and_refused_windows_explicit() {
        let report = build_tassadar_semantic_window_migration_planner_report().expect("report");

        assert_eq!(
            report.active_window_id,
            "tassadar.frozen_core_wasm.window.v1"
        );
        assert_eq!(report.selected_requested_window_ids.len(), 2);
        assert_eq!(report.downgraded_requested_window_ids.len(), 1);
        assert_eq!(report.refused_requested_window_ids.len(), 2);
        assert!(report.served_publication_allowed_window_ids.is_empty());
        assert!(report.overall_green);
    }

    #[test]
    fn semantic_window_migration_planner_matches_committed_truth() {
        let generated = build_tassadar_semantic_window_migration_planner_report().expect("report");
        let committed: TassadarSemanticWindowMigrationPlannerReport =
            read_json(tassadar_semantic_window_migration_planner_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
