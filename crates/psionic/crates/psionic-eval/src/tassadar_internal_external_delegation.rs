use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_environments::tassadar_delegation_benchmark_suite;
use psionic_router::{
    TASSADAR_INTERNAL_EXTERNAL_DELEGATION_ROUTE_MATRIX_REF, TassadarDelegationLane,
    TassadarDelegationLaneMeasurement, TassadarDelegationRefusalPosture,
    TassadarInternalExternalDelegationRouteMatrix,
    build_tassadar_internal_external_delegation_route_matrix,
};
use psionic_sandbox::tassadar_external_delegation_baseline;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_INTERNAL_EXTERNAL_DELEGATION_BENCHMARK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_external_delegation_benchmark_report.json";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalExternalDelegationBenchmarkReport {
    pub schema_version: u16,
    pub report_id: String,
    pub suite_id: String,
    pub sandbox_baseline_id: String,
    pub route_matrix_ref: String,
    pub internal_win_count: u32,
    pub cpu_reference_win_count: u32,
    pub external_sandbox_win_count: u32,
    pub hybrid_only_count: u32,
    pub internal_average_evidence_completeness_bps: u32,
    pub cpu_reference_average_evidence_completeness_bps: u32,
    pub external_sandbox_average_evidence_completeness_bps: u32,
    pub internal_refusal_or_degraded_count: u32,
    pub cpu_reference_refusal_or_degraded_count: u32,
    pub external_sandbox_refusal_or_degraded_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalExternalDelegationBenchmarkReportError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_internal_external_delegation_benchmark_report() -> Result<
    TassadarInternalExternalDelegationBenchmarkReport,
    TassadarInternalExternalDelegationBenchmarkReportError,
> {
    let suite = tassadar_delegation_benchmark_suite();
    let sandbox_baseline = tassadar_external_delegation_baseline();
    let route_matrix = build_tassadar_internal_external_delegation_route_matrix().map_err(
        |error| match error {
            psionic_router::TassadarInternalExternalDelegationRouteMatrixError::Read {
                path,
                error,
            } => TassadarInternalExternalDelegationBenchmarkReportError::Read { path, error },
            psionic_router::TassadarInternalExternalDelegationRouteMatrixError::Deserialize {
                path,
                error,
            } => {
                TassadarInternalExternalDelegationBenchmarkReportError::Deserialize { path, error }
            }
            psionic_router::TassadarInternalExternalDelegationRouteMatrixError::CreateDir {
                path,
                error,
            } => TassadarInternalExternalDelegationBenchmarkReportError::CreateDir { path, error },
            psionic_router::TassadarInternalExternalDelegationRouteMatrixError::Write {
                path,
                error,
            } => TassadarInternalExternalDelegationBenchmarkReportError::Write { path, error },
            psionic_router::TassadarInternalExternalDelegationRouteMatrixError::Json(error) => {
                TassadarInternalExternalDelegationBenchmarkReportError::Json(error)
            }
        },
    )?;
    let mut report = TassadarInternalExternalDelegationBenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.internal_external_delegation.benchmark_report.v1"),
        suite_id: suite.suite_id,
        sandbox_baseline_id: sandbox_baseline.baseline_id,
        route_matrix_ref: String::from(TASSADAR_INTERNAL_EXTERNAL_DELEGATION_ROUTE_MATRIX_REF),
        internal_win_count: route_matrix.internal_win_count,
        cpu_reference_win_count: route_matrix.cpu_reference_win_count,
        external_sandbox_win_count: route_matrix.external_sandbox_win_count,
        hybrid_only_count: route_matrix.hybrid_only_count,
        internal_average_evidence_completeness_bps: average_lane_metric(
            &route_matrix,
            TassadarDelegationLane::InternalExactCompute,
            |measurement| measurement.evidence_completeness_bps,
        ),
        cpu_reference_average_evidence_completeness_bps: average_lane_metric(
            &route_matrix,
            TassadarDelegationLane::CpuReference,
            |measurement| measurement.evidence_completeness_bps,
        ),
        external_sandbox_average_evidence_completeness_bps: average_lane_metric(
            &route_matrix,
            TassadarDelegationLane::ExternalSandbox,
            |measurement| measurement.evidence_completeness_bps,
        ),
        internal_refusal_or_degraded_count: degraded_or_refused_count(
            &route_matrix,
            TassadarDelegationLane::InternalExactCompute,
        ),
        cpu_reference_refusal_or_degraded_count: degraded_or_refused_count(
            &route_matrix,
            TassadarDelegationLane::CpuReference,
        ),
        external_sandbox_refusal_or_degraded_count: degraded_or_refused_count(
            &route_matrix,
            TassadarDelegationLane::ExternalSandbox,
        ),
        claim_boundary: String::from(
            "this report is a benchmark-bound comparison across matched workloads. It keeps internal exact-compute, cpu-reference, external sandbox, and hybrid-only honest postures explicit instead of collapsing route rankings into product or authority claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Delegation benchmark report covers {} matched cases with wins internal={}, cpu_reference={}, external_sandbox={}, hybrid={}, and evidence completeness internal/cpu/external = {}/{}/{} bps.",
        route_matrix.cases.len(),
        report.internal_win_count,
        report.cpu_reference_win_count,
        report.external_sandbox_win_count,
        report.hybrid_only_count,
        report.internal_average_evidence_completeness_bps,
        report.cpu_reference_average_evidence_completeness_bps,
        report.external_sandbox_average_evidence_completeness_bps,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_internal_external_delegation_benchmark_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_internal_external_delegation_benchmark_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_EXTERNAL_DELEGATION_BENCHMARK_REPORT_REF)
}

pub fn write_tassadar_internal_external_delegation_benchmark_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarInternalExternalDelegationBenchmarkReport,
    TassadarInternalExternalDelegationBenchmarkReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalExternalDelegationBenchmarkReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_external_delegation_benchmark_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalExternalDelegationBenchmarkReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn average_lane_metric<F>(
    route_matrix: &TassadarInternalExternalDelegationRouteMatrix,
    lane: TassadarDelegationLane,
    metric: F,
) -> u32
where
    F: Fn(&TassadarDelegationLaneMeasurement) -> u32,
{
    let values = route_matrix
        .cases
        .iter()
        .filter_map(|case| {
            case.lane_measurements
                .iter()
                .find(|measurement| measurement.lane == lane)
        })
        .map(metric)
        .collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u32>() / values.len() as u32
    }
}

fn degraded_or_refused_count(
    route_matrix: &TassadarInternalExternalDelegationRouteMatrix,
    lane: TassadarDelegationLane,
) -> u32 {
    route_matrix
        .cases
        .iter()
        .filter_map(|case| {
            case.lane_measurements
                .iter()
                .find(|measurement| measurement.lane == lane)
        })
        .filter(|measurement| {
            measurement.refusal_posture != TassadarDelegationRefusalPosture::Exact
        })
        .count() as u32
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarInternalExternalDelegationBenchmarkReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarInternalExternalDelegationBenchmarkReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalExternalDelegationBenchmarkReportError::Deserialize {
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
        TASSADAR_INTERNAL_EXTERNAL_DELEGATION_BENCHMARK_REPORT_REF,
        TassadarInternalExternalDelegationBenchmarkReport,
        build_tassadar_internal_external_delegation_benchmark_report, read_repo_json,
        tassadar_internal_external_delegation_benchmark_report_path,
        write_tassadar_internal_external_delegation_benchmark_report,
    };

    #[test]
    fn internal_external_delegation_benchmark_report_keeps_lane_boundaries_explicit() {
        let report =
            build_tassadar_internal_external_delegation_benchmark_report().expect("report");

        assert_eq!(report.internal_win_count, 2);
        assert_eq!(report.cpu_reference_win_count, 2);
        assert_eq!(report.external_sandbox_win_count, 1);
        assert_eq!(report.hybrid_only_count, 1);
        assert!(
            report.cpu_reference_average_evidence_completeness_bps
                > report.internal_average_evidence_completeness_bps
        );
    }

    #[test]
    fn internal_external_delegation_benchmark_report_matches_committed_truth() {
        let generated =
            build_tassadar_internal_external_delegation_benchmark_report().expect("report");
        let committed: TassadarInternalExternalDelegationBenchmarkReport =
            read_repo_json(TASSADAR_INTERNAL_EXTERNAL_DELEGATION_BENCHMARK_REPORT_REF)
                .expect("committed");
        assert_eq!(generated, committed);
    }

    #[test]
    fn internal_external_delegation_benchmark_report_writer_round_trips() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let output_path = temp_dir
            .path()
            .join("tassadar_internal_external_delegation_benchmark_report.json");
        let written = write_tassadar_internal_external_delegation_benchmark_report(&output_path)
            .expect("write");
        let persisted: TassadarInternalExternalDelegationBenchmarkReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }

    #[test]
    fn internal_external_delegation_benchmark_report_writer_uses_committed_path() {
        let path = tassadar_internal_external_delegation_benchmark_report_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("tassadar_internal_external_delegation_benchmark_report.json")
        );
    }
}
