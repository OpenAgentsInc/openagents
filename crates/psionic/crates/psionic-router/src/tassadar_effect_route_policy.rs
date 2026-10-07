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
    TassadarEffectClass, TassadarEffectEvidenceRequirement, TassadarEffectExecutionBoundary,
    TassadarEffectReplayPosture, tassadar_effect_taxonomy,
};
use psionic_sandbox::{TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF, tassadar_sandbox_effect_boundary};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_EFFECT_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effect_route_policy_report.json";

/// Router-facing route kind for one effect class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectRouteKind {
    InternalExact,
    HostStateSnapshotBound,
    SandboxDelegation,
    ReceiptBoundInput,
    Refused,
}

/// One routeable effect row published by the router.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectRoutePolicyRow {
    pub effect_ref: String,
    pub effect_class: TassadarEffectClass,
    pub route_kind: TassadarEffectRouteKind,
    pub execution_boundary: TassadarEffectExecutionBoundary,
    pub replay_posture: TassadarEffectReplayPosture,
    pub evidence_requirement: TassadarEffectEvidenceRequirement,
    pub max_replays: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durable_state_profile_id: Option<String>,
    pub note: String,
}

/// Router-owned route-policy report for the widened effect taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub taxonomy_id: String,
    pub boundary_id: String,
    pub rows: Vec<TassadarEffectRoutePolicyRow>,
    pub internal_exact_row_count: u32,
    pub host_state_row_count: u32,
    pub sandbox_delegation_row_count: u32,
    pub receipt_bound_input_row_count: u32,
    pub refused_row_count: u32,
    pub generated_from_refs: Vec<String>,
    pub kernel_policy_dependency_marker: String,
    pub world_mount_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectRoutePolicyReportError {
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
pub fn build_tassadar_effect_route_policy_report() -> TassadarEffectRoutePolicyReport {
    let taxonomy = tassadar_effect_taxonomy();
    let boundary = tassadar_sandbox_effect_boundary();
    let rows = taxonomy
        .entries
        .iter()
        .map(|entry| TassadarEffectRoutePolicyRow {
            effect_ref: entry.effect_ref.clone(),
            effect_class: entry.effect_class,
            route_kind: route_kind_for_class(entry.effect_class),
            execution_boundary: entry.execution_boundary,
            replay_posture: entry.replay_posture,
            evidence_requirement: entry.evidence_requirement,
            max_replays: entry.max_replays,
            durable_state_profile_id: boundary
                .durable_state_profiles
                .iter()
                .find(|profile| profile.effect_ref == entry.effect_ref)
                .map(|profile| profile.profile_id.clone()),
            note: entry.note.clone(),
        })
        .collect::<Vec<_>>();
    let internal_exact_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarEffectRouteKind::InternalExact)
        .count() as u32;
    let host_state_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarEffectRouteKind::HostStateSnapshotBound)
        .count() as u32;
    let sandbox_delegation_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarEffectRouteKind::SandboxDelegation)
        .count() as u32;
    let receipt_bound_input_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarEffectRouteKind::ReceiptBoundInput)
        .count() as u32;
    let refused_row_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarEffectRouteKind::Refused)
        .count() as u32;
    let mut report = TassadarEffectRoutePolicyReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.effect_route_policy.report.v1"),
        taxonomy_id: taxonomy.taxonomy_id.clone(),
        boundary_id: boundary.boundary_id,
        rows,
        internal_exact_row_count,
        host_state_row_count,
        sandbox_delegation_row_count,
        receipt_bound_input_row_count,
        refused_row_count,
        generated_from_refs: vec![String::from(TASSADAR_IMPORT_POLICY_MATRIX_REPORT_REF)],
        kernel_policy_dependency_marker: taxonomy.kernel_policy_dependency_marker,
        world_mount_dependency_marker: taxonomy.world_mount_dependency_marker,
        claim_boundary: String::from(
            "this router policy report widens the import story into typed internal exact, host-state replay-bound, sandbox delegation, receipt-bound input, and refused effect routes. It does not collapse sandbox delegation or host-backed state into internal exact compute and it does not grant ambient side-effect authority inside standalone psionic",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Effect route policy exposes {} rows across internal_exact={}, host_state_snapshot_bound={}, sandbox_delegation={}, receipt_bound_input={}, refused={}.",
        report.rows.len(),
        report.internal_exact_row_count,
        report.host_state_row_count,
        report.sandbox_delegation_row_count,
        report.receipt_bound_input_row_count,
        report.refused_row_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_effect_route_policy_report|", &report);
    report
}

#[must_use]
pub fn tassadar_effect_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFECT_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_effect_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarEffectRoutePolicyReport, TassadarEffectRoutePolicyReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_effect_route_policy_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_effect_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<TassadarEffectRoutePolicyReport, TassadarEffectRoutePolicyReportError> {
    read_json(path)
}

#[must_use]
pub fn route_kind_for_class(effect_class: TassadarEffectClass) -> TassadarEffectRouteKind {
    match effect_class {
        TassadarEffectClass::DeterministicInternalStub => TassadarEffectRouteKind::InternalExact,
        TassadarEffectClass::DeterministicHostState => {
            TassadarEffectRouteKind::HostStateSnapshotBound
        }
        TassadarEffectClass::ExternalSandboxDelegation => {
            TassadarEffectRouteKind::SandboxDelegation
        }
        TassadarEffectClass::BoundedNondeterministicInput => {
            TassadarEffectRouteKind::ReceiptBoundInput
        }
        TassadarEffectClass::UnsafeSideEffect => TassadarEffectRouteKind::Refused,
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
) -> Result<T, TassadarEffectRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarEffectRoutePolicyReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEffectRoutePolicyReportError::Deserialize {
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
        TassadarEffectRouteKind, build_tassadar_effect_route_policy_report,
        load_tassadar_effect_route_policy_report, tassadar_effect_route_policy_report_path,
        write_tassadar_effect_route_policy_report,
    };

    #[test]
    fn effect_route_policy_report_covers_widened_split() {
        let report = build_tassadar_effect_route_policy_report();
        assert_eq!(report.rows.len(), 5);
        assert_eq!(report.internal_exact_row_count, 1);
        assert_eq!(report.host_state_row_count, 1);
        assert_eq!(report.sandbox_delegation_row_count, 1);
        assert_eq!(report.receipt_bound_input_row_count, 1);
        assert_eq!(report.refused_row_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.effect_ref == "state.counter_slot_read"
                && row.route_kind == TassadarEffectRouteKind::HostStateSnapshotBound
                && row.durable_state_profile_id.is_some()
        }));
    }

    #[test]
    fn effect_route_policy_report_matches_committed_truth() {
        let report = build_tassadar_effect_route_policy_report();
        let persisted =
            load_tassadar_effect_route_policy_report(tassadar_effect_route_policy_report_path())
                .expect("committed report");
        assert_eq!(persisted, report);
    }

    #[test]
    fn write_effect_route_policy_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_effect_route_policy_report.json");
        let report = write_tassadar_effect_route_policy_report(&output_path).expect("report");
        let persisted = load_tassadar_effect_route_policy_report(&output_path).expect("persisted");
        assert_eq!(persisted, report);
        std::fs::remove_file(output_path).expect("temp report should be removable");
    }
}
