use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_environments::tassadar_delegation_benchmark_suite;
use psionic_models::TassadarWorkloadClass;
use psionic_sandbox::tassadar_external_delegation_baseline;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_INTERNAL_EXTERNAL_DELEGATION_ROUTE_MATRIX_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_external_delegation_route_matrix.json";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDelegationLane {
    InternalExactCompute,
    CpuReference,
    ExternalSandbox,
    Hybrid,
}

impl TassadarDelegationLane {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InternalExactCompute => "internal_exact_compute",
            Self::CpuReference => "cpu_reference",
            Self::ExternalSandbox => "external_sandbox",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDelegationRefusalPosture {
    Exact,
    Degraded,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDelegationLaneMeasurement {
    pub lane: TassadarDelegationLane,
    pub expected_correctness_bps: u32,
    pub evidence_completeness_bps: u32,
    pub estimated_latency_millis: u32,
    pub estimated_cost_milliunits: u32,
    pub refusal_posture: TassadarDelegationRefusalPosture,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDelegationBenchmarkCase {
    pub case_id: String,
    pub workload_class: TassadarWorkloadClass,
    pub lane_measurements: Vec<TassadarDelegationLaneMeasurement>,
    pub honest_winner: TassadarDelegationLane,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalExternalDelegationRouteMatrix {
    pub schema_version: u16,
    pub matrix_id: String,
    pub suite_id: String,
    pub sandbox_baseline_id: String,
    pub cases: Vec<TassadarDelegationBenchmarkCase>,
    pub internal_win_count: u32,
    pub cpu_reference_win_count: u32,
    pub external_sandbox_win_count: u32,
    pub hybrid_only_count: u32,
    pub internal_cost_per_correct_job_milliunits: u32,
    pub cpu_reference_cost_per_correct_job_milliunits: u32,
    pub external_sandbox_cost_per_correct_job_milliunits: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub matrix_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalExternalDelegationRouteMatrixError {
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

pub fn build_tassadar_internal_external_delegation_route_matrix() -> Result<
    TassadarInternalExternalDelegationRouteMatrix,
    TassadarInternalExternalDelegationRouteMatrixError,
> {
    let suite = tassadar_delegation_benchmark_suite();
    let sandbox_baseline = tassadar_external_delegation_baseline();
    let cases = seeded_cases();
    let mut matrix = TassadarInternalExternalDelegationRouteMatrix {
        schema_version: REPORT_SCHEMA_VERSION,
        matrix_id: String::from("tassadar.internal_external_delegation.route_matrix.v1"),
        suite_id: suite.suite_id,
        sandbox_baseline_id: sandbox_baseline.baseline_id,
        internal_win_count: cases
            .iter()
            .filter(|case| case.honest_winner == TassadarDelegationLane::InternalExactCompute)
            .count() as u32,
        cpu_reference_win_count: cases
            .iter()
            .filter(|case| case.honest_winner == TassadarDelegationLane::CpuReference)
            .count() as u32,
        external_sandbox_win_count: cases
            .iter()
            .filter(|case| case.honest_winner == TassadarDelegationLane::ExternalSandbox)
            .count() as u32,
        hybrid_only_count: cases
            .iter()
            .filter(|case| case.honest_winner == TassadarDelegationLane::Hybrid)
            .count() as u32,
        internal_cost_per_correct_job_milliunits: average_lane_cost(
            &cases,
            TassadarDelegationLane::InternalExactCompute,
        ),
        cpu_reference_cost_per_correct_job_milliunits: average_lane_cost(
            &cases,
            TassadarDelegationLane::CpuReference,
        ),
        external_sandbox_cost_per_correct_job_milliunits: average_lane_cost(
            &cases,
            TassadarDelegationLane::ExternalSandbox,
        ),
        cases,
        claim_boundary: String::from(
            "this matrix is a benchmark-bound route-comparison surface over matched workloads. It keeps internal exact-compute, cpu-reference, external sandbox, and hybrid postures explicit instead of promoting one lane into general closure",
        ),
        summary: String::new(),
        matrix_digest: String::new(),
    };
    matrix.summary = format!(
        "Delegation route matrix covers {} matched cases with wins internal={}, cpu_reference={}, external_sandbox={}, hybrid={}.",
        matrix.cases.len(),
        matrix.internal_win_count,
        matrix.cpu_reference_win_count,
        matrix.external_sandbox_win_count,
        matrix.hybrid_only_count,
    );
    matrix.matrix_digest = stable_digest(
        b"psionic_tassadar_internal_external_delegation_route_matrix|",
        &matrix,
    );
    Ok(matrix)
}

#[must_use]
pub fn tassadar_internal_external_delegation_route_matrix_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_EXTERNAL_DELEGATION_ROUTE_MATRIX_REF)
}

pub fn write_tassadar_internal_external_delegation_route_matrix(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarInternalExternalDelegationRouteMatrix,
    TassadarInternalExternalDelegationRouteMatrixError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalExternalDelegationRouteMatrixError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let matrix = build_tassadar_internal_external_delegation_route_matrix()?;
    let json = serde_json::to_string_pretty(&matrix)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalExternalDelegationRouteMatrixError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(matrix)
}

fn seeded_cases() -> Vec<TassadarDelegationBenchmarkCase> {
    vec![
        case(
            "arithmetic_microprogram_exact_transform",
            TassadarWorkloadClass::ArithmeticMicroprogram,
            TassadarDelegationLane::InternalExactCompute,
            vec![
                lane(
                    TassadarDelegationLane::InternalExactCompute,
                    9_900,
                    8_900,
                    180,
                    900,
                    TassadarDelegationRefusalPosture::Exact,
                    "internal exact-compute is fastest while staying exact enough on this bounded arithmetic row",
                ),
                lane(
                    TassadarDelegationLane::CpuReference,
                    10_000,
                    10_000,
                    620,
                    1_400,
                    TassadarDelegationRefusalPosture::Exact,
                    "cpu-reference stays exact and authoritative but slower and costlier",
                ),
                lane(
                    TassadarDelegationLane::ExternalSandbox,
                    10_000,
                    9_500,
                    780,
                    3_100,
                    TassadarDelegationRefusalPosture::Exact,
                    "external sandbox is exact but expensive for this small bounded row",
                ),
            ],
            "internal execution wins honestly on small exact arithmetic workloads",
        ),
        case(
            "memory_heavy_kernel_patch",
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarDelegationLane::InternalExactCompute,
            vec![
                lane(
                    TassadarDelegationLane::InternalExactCompute,
                    9_880,
                    9_100,
                    320,
                    1_900,
                    TassadarDelegationRefusalPosture::Exact,
                    "internal lane keeps the best cost-latency profile on this bounded memory-heavy kernel",
                ),
                lane(
                    TassadarDelegationLane::CpuReference,
                    10_000,
                    10_000,
                    780,
                    2_500,
                    TassadarDelegationRefusalPosture::Exact,
                    "cpu-reference remains strongest on evidence completeness but costs more",
                ),
                lane(
                    TassadarDelegationLane::ExternalSandbox,
                    9_950,
                    9_400,
                    900,
                    4_200,
                    TassadarDelegationRefusalPosture::Exact,
                    "external sandbox stays exact but is not the best current tradeoff",
                ),
            ],
            "internal execution remains the honest winner on the public bounded memory-heavy lane",
        ),
        case(
            "long_loop_kernel_robust",
            TassadarWorkloadClass::LongLoopKernel,
            TassadarDelegationLane::ExternalSandbox,
            vec![
                lane(
                    TassadarDelegationLane::InternalExactCompute,
                    9_100,
                    7_200,
                    640,
                    3_600,
                    TassadarDelegationRefusalPosture::Degraded,
                    "internal lane remains faster than sandbox but still degrades through fallback churn",
                ),
                lane(
                    TassadarDelegationLane::CpuReference,
                    10_000,
                    10_000,
                    1_450,
                    3_200,
                    TassadarDelegationRefusalPosture::Exact,
                    "cpu-reference stays exact but slower than the current sandbox baseline",
                ),
                lane(
                    TassadarDelegationLane::ExternalSandbox,
                    9_950,
                    9_300,
                    980,
                    2_700,
                    TassadarDelegationRefusalPosture::Exact,
                    "external sandbox is the strongest robust public baseline on the long-loop row",
                ),
            ],
            "external sandbox wins when the public internal lane still degrades on long-loop robustness",
        ),
        case(
            "sudoku_candidate_check",
            TassadarWorkloadClass::SudokuClass,
            TassadarDelegationLane::CpuReference,
            vec![
                lane(
                    TassadarDelegationLane::InternalExactCompute,
                    9_250,
                    7_800,
                    410,
                    2_950,
                    TassadarDelegationRefusalPosture::Degraded,
                    "internal lane is strong but not the cleanest current exact authority for this search row",
                ),
                lane(
                    TassadarDelegationLane::CpuReference,
                    10_000,
                    10_000,
                    730,
                    2_200,
                    TassadarDelegationRefusalPosture::Exact,
                    "cpu-reference remains the honest exact authority on the seeded Sudoku row",
                ),
                lane(
                    TassadarDelegationLane::ExternalSandbox,
                    9_900,
                    9_300,
                    1_020,
                    4_900,
                    TassadarDelegationRefusalPosture::Exact,
                    "external sandbox stays exact but costlier than CPU-reference here",
                ),
            ],
            "cpu-reference remains the cleanest exact winner on the public Sudoku benchmark row",
        ),
        case(
            "branch_heavy_control_repair",
            TassadarWorkloadClass::BranchHeavyKernel,
            TassadarDelegationLane::Hybrid,
            vec![
                lane(
                    TassadarDelegationLane::InternalExactCompute,
                    9_780,
                    8_100,
                    300,
                    2_200,
                    TassadarDelegationRefusalPosture::Degraded,
                    "internal lane is attractive on speed but not sufficient alone for the current evidence bar",
                ),
                lane(
                    TassadarDelegationLane::CpuReference,
                    10_000,
                    10_000,
                    1_150,
                    2_700,
                    TassadarDelegationRefusalPosture::Exact,
                    "cpu-reference is exact but too slow to be the only honest operational path here",
                ),
                lane(
                    TassadarDelegationLane::ExternalSandbox,
                    9_920,
                    9_100,
                    1_040,
                    4_600,
                    TassadarDelegationRefusalPosture::Exact,
                    "external sandbox is exact but still costlier than the hybrid validation posture",
                ),
            ],
            "hybrid routing is the only honest posture on this row: internal speed plus CPU-reference validation",
        ),
        case(
            "clrs_shortest_path_verification",
            TassadarWorkloadClass::ClrsShortestPath,
            TassadarDelegationLane::CpuReference,
            vec![
                lane(
                    TassadarDelegationLane::InternalExactCompute,
                    8_900,
                    7_600,
                    360,
                    2_400,
                    TassadarDelegationRefusalPosture::Degraded,
                    "internal CLRS bridge remains benchmarked but not yet the strongest public exact path",
                ),
                lane(
                    TassadarDelegationLane::CpuReference,
                    10_000,
                    10_000,
                    810,
                    2_100,
                    TassadarDelegationRefusalPosture::Exact,
                    "cpu-reference remains the exact authority for the CLRS bridge row",
                ),
                lane(
                    TassadarDelegationLane::ExternalSandbox,
                    9_950,
                    9_350,
                    1_100,
                    4_300,
                    TassadarDelegationRefusalPosture::Exact,
                    "external sandbox is viable but does not beat CPU-reference on this bridge workload",
                ),
            ],
            "cpu-reference wins where the internal CLRS bridge is still only partially closed publicly",
        ),
    ]
}

fn case(
    case_id: &str,
    workload_class: TassadarWorkloadClass,
    honest_winner: TassadarDelegationLane,
    mut lane_measurements: Vec<TassadarDelegationLaneMeasurement>,
    note: &str,
) -> TassadarDelegationBenchmarkCase {
    lane_measurements.sort_by_key(|lane| lane.lane.as_str());
    TassadarDelegationBenchmarkCase {
        case_id: String::from(case_id),
        workload_class,
        lane_measurements,
        honest_winner,
        note: String::from(note),
    }
}

fn lane(
    lane: TassadarDelegationLane,
    expected_correctness_bps: u32,
    evidence_completeness_bps: u32,
    estimated_latency_millis: u32,
    estimated_cost_milliunits: u32,
    refusal_posture: TassadarDelegationRefusalPosture,
    note: &str,
) -> TassadarDelegationLaneMeasurement {
    TassadarDelegationLaneMeasurement {
        lane,
        expected_correctness_bps,
        evidence_completeness_bps,
        estimated_latency_millis,
        estimated_cost_milliunits,
        refusal_posture,
        note: String::from(note),
    }
}

fn average_lane_cost(
    cases: &[TassadarDelegationBenchmarkCase],
    lane: TassadarDelegationLane,
) -> u32 {
    let exact_rows = cases
        .iter()
        .filter_map(|case| {
            case.lane_measurements.iter().find(|measurement| {
                measurement.lane == lane
                    && measurement.refusal_posture == TassadarDelegationRefusalPosture::Exact
            })
        })
        .map(|measurement| measurement.estimated_cost_milliunits)
        .collect::<Vec<_>>();
    if exact_rows.is_empty() {
        0
    } else {
        exact_rows.iter().sum::<u32>() / exact_rows.len() as u32
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
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarInternalExternalDelegationRouteMatrixError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarInternalExternalDelegationRouteMatrixError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalExternalDelegationRouteMatrixError::Deserialize {
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
        TASSADAR_INTERNAL_EXTERNAL_DELEGATION_ROUTE_MATRIX_REF, TassadarDelegationLane,
        TassadarInternalExternalDelegationRouteMatrix,
        build_tassadar_internal_external_delegation_route_matrix, read_repo_json,
        tassadar_internal_external_delegation_route_matrix_path,
        write_tassadar_internal_external_delegation_route_matrix,
    };

    #[test]
    fn internal_external_delegation_route_matrix_keeps_winner_boundaries_explicit() {
        let matrix = build_tassadar_internal_external_delegation_route_matrix().expect("matrix");

        assert_eq!(matrix.cases.len(), 6);
        assert_eq!(matrix.internal_win_count, 2);
        assert_eq!(matrix.cpu_reference_win_count, 2);
        assert_eq!(matrix.external_sandbox_win_count, 1);
        assert_eq!(matrix.hybrid_only_count, 1);
        assert!(
            matrix
                .cases
                .iter()
                .any(|case| case.honest_winner == TassadarDelegationLane::Hybrid)
        );
    }

    #[test]
    fn internal_external_delegation_route_matrix_matches_committed_truth() {
        let generated = build_tassadar_internal_external_delegation_route_matrix().expect("matrix");
        let committed: TassadarInternalExternalDelegationRouteMatrix =
            read_repo_json(TASSADAR_INTERNAL_EXTERNAL_DELEGATION_ROUTE_MATRIX_REF)
                .expect("committed matrix");
        assert_eq!(generated, committed);
    }

    #[test]
    fn internal_external_delegation_route_matrix_writer_round_trips() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let output_path = temp_dir
            .path()
            .join("tassadar_internal_external_delegation_route_matrix.json");
        let written = write_tassadar_internal_external_delegation_route_matrix(&output_path)
            .expect("write matrix");
        let persisted: TassadarInternalExternalDelegationRouteMatrix =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read matrix"))
                .expect("decode matrix");
        assert_eq!(written, persisted);
    }

    #[test]
    fn internal_external_delegation_route_matrix_writer_uses_committed_path() {
        let path = tassadar_internal_external_delegation_route_matrix_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("tassadar_internal_external_delegation_route_matrix.json")
        );
    }
}
