use std::{fs, path::Path};

use psionic_data::{
    TASSADAR_MIXED_TRAJECTORY_CONTRACT_REF, TassadarMixedTrajectoryTrainingCase,
    TassadarMixedTrajectoryTrainingSuite, tassadar_mixed_trajectory_contract,
};
use psionic_ir::{TassadarMixedTrajectoryEntryKind, TassadarMixedTrajectoryLaneKind};
use psionic_runtime::replay_tassadar_mixed_trajectory;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_MIXED_TRAJECTORY_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_mixed_trajectory_suite_v1";
pub const TASSADAR_MIXED_TRAJECTORY_SUITE_FILE: &str = "mixed_trajectory_suite.json";

/// Errors while materializing the mixed trajectory suite artifact.
#[derive(Debug, Error)]
pub enum TassadarMixedTrajectorySuiteError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write mixed trajectory suite `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Replay(#[from] psionic_runtime::TassadarMixedTrajectoryReplayError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Executes the committed mixed trajectory suite and writes the train-side artifact.
pub fn execute_tassadar_mixed_trajectory_suite(
    output_dir: &Path,
) -> Result<TassadarMixedTrajectoryTrainingSuite, TassadarMixedTrajectorySuiteError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarMixedTrajectorySuiteError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let contract = tassadar_mixed_trajectory_contract();
    let case_reports = contract
        .cases
        .iter()
        .map(|case| {
            let receipt = replay_tassadar_mixed_trajectory(&case.trajectory)?;
            let lane_sequence = receipt.lane_sequence.clone();
            let language_span_count = case
                .trajectory
                .entries
                .iter()
                .filter(|entry| {
                    entry.entry_kind == TassadarMixedTrajectoryEntryKind::LanguageSpan
                        && entry.lane_kind == TassadarMixedTrajectoryLaneKind::LanguageReasoning
                })
                .count() as u32;
            let exact_compute_span_count = case
                .trajectory
                .entries
                .iter()
                .filter(|entry| {
                    entry.entry_kind == TassadarMixedTrajectoryEntryKind::ExactComputeSpan
                        && entry.lane_kind == TassadarMixedTrajectoryLaneKind::InternalExactCompute
                })
                .count() as u32;
            let verifier_span_count = case
                .trajectory
                .entries
                .iter()
                .filter(|entry| {
                    entry.entry_kind == TassadarMixedTrajectoryEntryKind::VerifierEventSpan
                        && entry.lane_kind == TassadarMixedTrajectoryLaneKind::VerifierSearch
                })
                .count() as u32;
            let external_tool_span_count = case
                .trajectory
                .entries
                .iter()
                .filter(|entry| {
                    entry.entry_kind == TassadarMixedTrajectoryEntryKind::ExternalToolSpan
                        && entry.lane_kind == TassadarMixedTrajectoryLaneKind::ExternalTool
                })
                .count() as u32;
            Ok(TassadarMixedTrajectoryTrainingCase {
                case_id: case.case_id.clone(),
                workload_family: case.workload_family,
                lane_sequence,
                language_span_count,
                exact_compute_span_count,
                verifier_span_count,
                external_tool_span_count,
                receipt_boundary_count: receipt.receipt_boundary_count,
                handoff_count: receipt.handoff_count,
                schema_roundtrip_ok: receipt.schema_roundtrip_ok,
                lane_handoff_correct: receipt.lane_handoff_correct,
                trajectory_to_outcome_parity: receipt.trajectory_to_outcome_parity,
                note: case.note.clone(),
            })
        })
        .collect::<Result<Vec<_>, TassadarMixedTrajectorySuiteError>>()?;
    let mut suite = TassadarMixedTrajectoryTrainingSuite {
        contract,
        case_reports,
        summary: String::new(),
        report_digest: String::new(),
    };
    let parity_count = suite
        .case_reports
        .iter()
        .filter(|case| case.trajectory_to_outcome_parity)
        .count();
    suite.summary = format!(
        "Mixed trajectory suite now freezes {} seeded hybrid cases from `{}` with {} schema-roundtrip passes and {} trajectory-to-outcome parity passes.",
        suite.case_reports.len(),
        TASSADAR_MIXED_TRAJECTORY_CONTRACT_REF,
        suite
            .case_reports
            .iter()
            .filter(|case| case.schema_roundtrip_ok)
            .count(),
        parity_count,
    );
    suite.report_digest =
        stable_digest(b"psionic_tassadar_mixed_trajectory_training_suite|", &suite);

    let output_path = output_dir.join(TASSADAR_MIXED_TRAJECTORY_SUITE_FILE);
    let json = serde_json::to_string_pretty(&suite)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarMixedTrajectorySuiteError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(suite)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TASSADAR_MIXED_TRAJECTORY_SUITE_FILE, execute_tassadar_mixed_trajectory_suite};

    #[test]
    fn mixed_trajectory_suite_writes_machine_legible_training_artifact() {
        let directory = tempfile::tempdir().expect("tempdir");
        let suite = execute_tassadar_mixed_trajectory_suite(directory.path()).expect("suite");

        assert_eq!(suite.case_reports.len(), 3);
        assert!(
            suite
                .case_reports
                .iter()
                .all(|case| case.schema_roundtrip_ok && case.lane_handoff_correct)
        );
        assert!(
            directory
                .path()
                .join(TASSADAR_MIXED_TRAJECTORY_SUITE_FILE)
                .exists()
        );
    }
}
