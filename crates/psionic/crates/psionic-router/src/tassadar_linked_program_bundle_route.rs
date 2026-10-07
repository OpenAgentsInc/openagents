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
    TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF, TassadarLinkedProgramBundlePosture,
    TassadarRuntimeSupportModuleClass, build_tassadar_linked_program_bundle_runtime_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LINKED_PROGRAM_BUNDLE_ROUTE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_route_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramBundleRouteKind {
    InternalExact,
    SharedStateReceiptBound,
    RollbackPinnedHelper,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleRouteRow {
    pub bundle_id: String,
    pub consumer_family: String,
    pub route_kind: TassadarLinkedProgramBundleRouteKind,
    pub runtime_posture: TassadarLinkedProgramBundlePosture,
    pub helper_module_refs: Vec<String>,
    pub runtime_support_classes: Vec<TassadarRuntimeSupportModuleClass>,
    pub shared_state_module_refs: Vec<String>,
    pub benchmark_lineage_complete: bool,
    pub dependency_graph_digest: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleRouteReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_report_ref: String,
    pub rows: Vec<TassadarLinkedProgramBundleRouteRow>,
    pub internal_exact_row_count: u32,
    pub shared_state_row_count: u32,
    pub rollback_row_count: u32,
    pub refused_row_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarLinkedProgramBundleRouteReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_linked_program_bundle_route_report() -> TassadarLinkedProgramBundleRouteReport
{
    let runtime_report = build_tassadar_linked_program_bundle_runtime_report();
    let rows = runtime_report
        .case_reports
        .iter()
        .map(|case| TassadarLinkedProgramBundleRouteRow {
            bundle_id: case.bundle_descriptor.bundle_id.clone(),
            consumer_family: case.bundle_descriptor.consumer_family.clone(),
            route_kind: route_kind_for_case(case),
            runtime_posture: case.posture,
            helper_module_refs: case.helper_module_refs.clone(),
            runtime_support_classes: case.runtime_support_classes.clone(),
            shared_state_module_refs: case.shared_state_module_refs.clone(),
            benchmark_lineage_complete: case.benchmark_lineage_complete,
            dependency_graph_digest: case.dependency_graph_digest.clone(),
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let internal_exact_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::InternalExact)
        .count() as u32;
    let shared_state_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::SharedStateReceiptBound)
        .count() as u32;
    let rollback_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::RollbackPinnedHelper)
        .count() as u32;
    let refused_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::Refused)
        .count() as u32;
    let mut report = TassadarLinkedProgramBundleRouteReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.linked_program_bundle_route.report.v1"),
        runtime_report_ref: String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF),
        rows,
        internal_exact_row_count,
        shared_state_row_count,
        rollback_row_count,
        refused_row_count,
        claim_boundary: String::from(
            "this router report freezes bounded route posture for linked-program bundles with explicit shared-state, helper rollback, and refusal boundaries. It does not imply arbitrary bundle composition or unrestricted runtime-support installation",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Linked-program bundle route report exposes {} rows with internal_exact={}, shared_state_receipt_bound={}, rollback_pinned_helper={}, refused={}.",
        report.rows.len(),
        report.internal_exact_row_count,
        report.shared_state_row_count,
        report.rollback_row_count,
        report.refused_row_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_linked_program_bundle_route_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_linked_program_bundle_route_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LINKED_PROGRAM_BUNDLE_ROUTE_REPORT_REF)
}

pub fn write_tassadar_linked_program_bundle_route_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleRouteReport, TassadarLinkedProgramBundleRouteReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLinkedProgramBundleRouteReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_linked_program_bundle_route_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLinkedProgramBundleRouteReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_linked_program_bundle_route_report(
    path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleRouteReport, TassadarLinkedProgramBundleRouteReportError>
{
    read_json(path)
}

fn route_kind_for_case(
    case: &psionic_runtime::TassadarLinkedProgramBundleCaseReport,
) -> TassadarLinkedProgramBundleRouteKind {
    match case.posture {
        TassadarLinkedProgramBundlePosture::Exact if case.shared_state_module_refs.is_empty() => {
            TassadarLinkedProgramBundleRouteKind::InternalExact
        }
        TassadarLinkedProgramBundlePosture::Exact => {
            TassadarLinkedProgramBundleRouteKind::SharedStateReceiptBound
        }
        TassadarLinkedProgramBundlePosture::RolledBack => {
            TassadarLinkedProgramBundleRouteKind::RollbackPinnedHelper
        }
        TassadarLinkedProgramBundlePosture::Refused => TassadarLinkedProgramBundleRouteKind::Refused,
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-router crate dir")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarLinkedProgramBundleRouteReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarLinkedProgramBundleRouteReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLinkedProgramBundleRouteReportError::Deserialize {
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
        TassadarLinkedProgramBundleRouteKind, build_tassadar_linked_program_bundle_route_report,
        load_tassadar_linked_program_bundle_route_report,
        tassadar_linked_program_bundle_route_report_path,
    };

    #[test]
    fn linked_program_bundle_route_report_tracks_exact_shared_rollback_and_refusal_routes() {
        let report = build_tassadar_linked_program_bundle_route_report();
        assert_eq!(report.internal_exact_row_count, 1);
        assert_eq!(report.shared_state_row_count, 1);
        assert_eq!(report.rollback_row_count, 1);
        assert_eq!(report.refused_row_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.bundle_id == "tassadar.linked_program_bundle.checkpoint_backtrack.v1"
                && row.route_kind == TassadarLinkedProgramBundleRouteKind::SharedStateReceiptBound
        }));
    }

    #[test]
    fn linked_program_bundle_route_report_matches_committed_truth() {
        let expected = build_tassadar_linked_program_bundle_route_report();
        let committed = load_tassadar_linked_program_bundle_route_report(
            tassadar_linked_program_bundle_route_report_path(),
        )
        .expect("committed report");
        assert_eq!(committed, expected);
    }
}
