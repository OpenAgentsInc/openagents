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
    build_tassadar_hybrid_process_controller_runtime_bundle,
    TassadarHybridProcessControllerRuntimeStatus,
    TASSADAR_HYBRID_PROCESS_CONTROLLER_RUNTIME_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_ROUTE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hybrid_process_controller_route_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHybridProcessControllerRouteKind {
    CompiledExact,
    HybridVerifierAttached,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerRouteRow {
    pub case_id: String,
    pub workload_family: String,
    pub route_kind: TassadarHybridProcessControllerRouteKind,
    pub runtime_status: TassadarHybridProcessControllerRuntimeStatus,
    pub verifier_attached: bool,
    pub challenge_path_green: bool,
    pub verifier_gain_bps: i32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerRouteReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_report_ref: String,
    pub rows: Vec<TassadarHybridProcessControllerRouteRow>,
    pub compiled_exact_route_count: u32,
    pub hybrid_route_count: u32,
    pub refused_route_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarHybridProcessControllerRouteReportError {
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
pub fn build_tassadar_hybrid_process_controller_route_report(
) -> TassadarHybridProcessControllerRouteReport {
    let runtime_report = build_tassadar_hybrid_process_controller_runtime_bundle();
    let rows = runtime_report
        .case_receipts
        .iter()
        .map(|case| TassadarHybridProcessControllerRouteRow {
            case_id: case.case_id.clone(),
            workload_family: case.workload_family.clone(),
            route_kind: match case.runtime_status {
                TassadarHybridProcessControllerRuntimeStatus::CompiledExact => {
                    TassadarHybridProcessControllerRouteKind::CompiledExact
                }
                TassadarHybridProcessControllerRuntimeStatus::HybridVerifierAttached => {
                    TassadarHybridProcessControllerRouteKind::HybridVerifierAttached
                }
                TassadarHybridProcessControllerRuntimeStatus::Refused => {
                    TassadarHybridProcessControllerRouteKind::Refused
                }
            },
            runtime_status: case.runtime_status,
            verifier_attached: case.verifier_attached,
            challenge_path_green: case.challenge_path_green,
            verifier_gain_bps: case.verifier_on_exactness_bps as i32
                - case.verifier_off_exactness_bps as i32,
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let compiled_exact_route_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarHybridProcessControllerRouteKind::CompiledExact)
        .count() as u32;
    let hybrid_route_count = rows
        .iter()
        .filter(|row| {
            row.route_kind == TassadarHybridProcessControllerRouteKind::HybridVerifierAttached
        })
        .count() as u32;
    let refused_route_count = rows
        .iter()
        .filter(|row| row.route_kind == TassadarHybridProcessControllerRouteKind::Refused)
        .count() as u32;
    let mut report = TassadarHybridProcessControllerRouteReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.hybrid_process_controller.route.report.v1"),
        runtime_report_ref: String::from(TASSADAR_HYBRID_PROCESS_CONTROLLER_RUNTIME_REPORT_REF),
        rows,
        compiled_exact_route_count,
        hybrid_route_count,
        refused_route_count,
        claim_boundary: String::from(
            "this router report freezes bounded route posture for the verifier-attached hybrid controller. It does not imply default hybrid routing, arbitrary lane switching, or served publication widening",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Hybrid process controller route report exposes {} rows with compiled_exact={}, hybrid_verifier_attached={}, refused={}.",
        report.rows.len(),
        report.compiled_exact_route_count,
        report.hybrid_route_count,
        report.refused_route_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_hybrid_process_controller_route_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_hybrid_process_controller_route_report_path() -> PathBuf {
    repo_root().join(TASSADAR_HYBRID_PROCESS_CONTROLLER_ROUTE_REPORT_REF)
}

pub fn write_tassadar_hybrid_process_controller_route_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarHybridProcessControllerRouteReport,
    TassadarHybridProcessControllerRouteReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarHybridProcessControllerRouteReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_hybrid_process_controller_route_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarHybridProcessControllerRouteReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
) -> Result<T, TassadarHybridProcessControllerRouteReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarHybridProcessControllerRouteReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarHybridProcessControllerRouteReportError::Deserialize {
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
        build_tassadar_hybrid_process_controller_route_report, read_json,
        tassadar_hybrid_process_controller_route_report_path,
        TassadarHybridProcessControllerRouteKind,
    };

    #[test]
    fn hybrid_process_controller_route_report_tracks_hybrid_route_and_refusal_posture() {
        let report = build_tassadar_hybrid_process_controller_route_report();

        assert_eq!(report.compiled_exact_route_count, 1);
        assert_eq!(report.hybrid_route_count, 2);
        assert_eq!(report.refused_route_count, 1);
        assert!(report.rows.iter().any(|row| {
            row.workload_family == "search_frontier_resume"
                && row.route_kind
                    == TassadarHybridProcessControllerRouteKind::HybridVerifierAttached
                && row.verifier_gain_bps > 0
        }));
    }

    #[test]
    fn hybrid_process_controller_route_report_matches_committed_truth() {
        let generated = build_tassadar_hybrid_process_controller_route_report();
        let committed: super::TassadarHybridProcessControllerRouteReport =
            read_json(tassadar_hybrid_process_controller_route_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
