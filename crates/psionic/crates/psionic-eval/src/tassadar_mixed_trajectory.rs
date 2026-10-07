use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TASSADAR_MIXED_TRAJECTORY_REPORT_REF, TASSADAR_MIXED_TRAJECTORY_SUITE_REF,
    TassadarMixedTrajectoryTrainingSuite, TassadarMixedTrajectoryWorkloadFamily,
    tassadar_mixed_trajectory_contract,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Case-level summary in the mixed trajectory eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryCaseSummary {
    /// Compared workload family.
    pub workload_family: TassadarMixedTrajectoryWorkloadFamily,
    /// Number of explicit lane handoffs.
    pub handoff_count: u32,
    /// Number of explicit receipt boundaries.
    pub receipt_boundary_count: u32,
    /// Whether schema roundtrip stayed exact.
    pub schema_roundtrip_ok: bool,
    /// Whether lane handoffs stayed correct.
    pub lane_handoff_correct: bool,
    /// Whether final outputs matched the replayed outcome.
    pub trajectory_to_outcome_parity: bool,
    /// Ordered lane sequence.
    pub lane_sequence: Vec<psionic_ir::TassadarMixedTrajectoryLaneKind>,
    /// Plain-language note.
    pub note: String,
}

/// Eval-side report for the mixed trajectory suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Source contract ref.
    pub contract_ref: String,
    /// Source contract digest.
    pub contract_digest: String,
    /// Source suite ref.
    pub suite_ref: String,
    /// Source suite digest.
    pub suite_digest: String,
    /// Case summaries.
    pub case_summaries: Vec<TassadarMixedTrajectoryCaseSummary>,
    /// Share of cases with exact schema roundtrip.
    pub schema_roundtrip_rate_bps: u32,
    /// Share of cases with correct lane handoffs.
    pub lane_handoff_correctness_bps: u32,
    /// Share of cases with trajectory-to-outcome parity.
    pub trajectory_to_outcome_parity_bps: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarMixedTrajectoryReportError {
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

/// Builds the committed mixed trajectory report.
pub fn build_tassadar_mixed_trajectory_report()
-> Result<TassadarMixedTrajectoryReport, TassadarMixedTrajectoryReportError> {
    let contract = tassadar_mixed_trajectory_contract();
    let suite: TassadarMixedTrajectoryTrainingSuite =
        read_repo_json(TASSADAR_MIXED_TRAJECTORY_SUITE_REF)?;
    let case_summaries = suite
        .case_reports
        .iter()
        .map(|case| TassadarMixedTrajectoryCaseSummary {
            workload_family: case.workload_family,
            handoff_count: case.handoff_count,
            receipt_boundary_count: case.receipt_boundary_count,
            schema_roundtrip_ok: case.schema_roundtrip_ok,
            lane_handoff_correct: case.lane_handoff_correct,
            trajectory_to_outcome_parity: case.trajectory_to_outcome_parity,
            lane_sequence: case.lane_sequence.clone(),
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let case_count = case_summaries.len() as u32;
    let mut report = TassadarMixedTrajectoryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.mixed_trajectory.report.v1"),
        contract_ref: contract.contract_ref.clone(),
        contract_digest: contract.contract_digest.clone(),
        suite_ref: String::from(TASSADAR_MIXED_TRAJECTORY_SUITE_REF),
        suite_digest: suite.report_digest.clone(),
        case_summaries,
        schema_roundtrip_rate_bps: ratio_bps(
            suite
                .case_reports
                .iter()
                .filter(|case| case.schema_roundtrip_ok)
                .count() as u32,
            case_count,
        ),
        lane_handoff_correctness_bps: ratio_bps(
            suite
                .case_reports
                .iter()
                .filter(|case| case.lane_handoff_correct)
                .count() as u32,
            case_count,
        ),
        trajectory_to_outcome_parity_bps: ratio_bps(
            suite
                .case_reports
                .iter()
                .filter(|case| case.trajectory_to_outcome_parity)
                .count() as u32,
            case_count,
        ),
        claim_boundary: String::from(
            "this report checks schema roundtrip, lane handoff correctness, receipt-boundary explicitness, and outcome parity over the seeded mixed trajectory suite. It remains an execution-truth artifact family and does not imply accepted-outcome closure or market settlement",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Mixed trajectory report now covers {} seeded hybrid cases with schema_roundtrip={}bps, lane_handoff_correctness={}bps, and trajectory_to_outcome_parity={}bps.",
        report.case_summaries.len(),
        report.schema_roundtrip_rate_bps,
        report.lane_handoff_correctness_bps,
        report.trajectory_to_outcome_parity_bps,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_mixed_trajectory_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_mixed_trajectory_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MIXED_TRAJECTORY_REPORT_REF)
}

/// Writes the committed mixed trajectory report.
pub fn write_tassadar_mixed_trajectory_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMixedTrajectoryReport, TassadarMixedTrajectoryReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMixedTrajectoryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_mixed_trajectory_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMixedTrajectoryReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        numerator.saturating_mul(10_000) / denominator
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarMixedTrajectoryReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarMixedTrajectoryReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarMixedTrajectoryReportError::Deserialize {
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
        TassadarMixedTrajectoryReport, build_tassadar_mixed_trajectory_report, read_repo_json,
        tassadar_mixed_trajectory_report_path, write_tassadar_mixed_trajectory_report,
    };
    use psionic_data::TASSADAR_MIXED_TRAJECTORY_REPORT_REF;

    #[test]
    fn mixed_trajectory_report_keeps_roundtrip_handoff_and_parity_explicit() {
        let report = build_tassadar_mixed_trajectory_report().expect("report");

        assert_eq!(report.case_summaries.len(), 3);
        assert_eq!(report.schema_roundtrip_rate_bps, 10_000);
        assert_eq!(report.lane_handoff_correctness_bps, 10_000);
        assert_eq!(report.trajectory_to_outcome_parity_bps, 10_000);
    }

    #[test]
    fn mixed_trajectory_report_matches_committed_truth() {
        let generated = build_tassadar_mixed_trajectory_report().expect("report");
        let committed: TassadarMixedTrajectoryReport =
            read_repo_json(TASSADAR_MIXED_TRAJECTORY_REPORT_REF).expect("committed");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_mixed_trajectory_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_mixed_trajectory_report.json");
        let written = write_tassadar_mixed_trajectory_report(&output_path).expect("write");
        let persisted: TassadarMixedTrajectoryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert!(
            tassadar_mixed_trajectory_report_path()
                .ends_with("fixtures/tassadar/reports/tassadar_mixed_trajectory_report.json")
        );
    }
}
