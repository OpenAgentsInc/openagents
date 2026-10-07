use std::{
    fs,
    path::Path,
};

use psionic_models::TassadarExecutorSubroutineWorkloadFamily;
use psionic_train::{
    TassadarExecutorSubroutineOodProxyConfig, TassadarExecutorSubroutineOodProxyReport,
    TassadarExecutorSupervisionTargetMode, TassadarExecutorSubroutineDatasetError,
    build_tassadar_executor_subroutine_dataset_manifest,
    build_tassadar_executor_subroutine_ood_proxy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SUBROUTINE_LIBRARY_ABLATION_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_FILE: &str =
    "tassadar_subroutine_library_ablation_report.json";
pub const TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json";
pub const TASSADAR_SUBROUTINE_LIBRARY_ABLATION_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_subroutine_library_ablation";
pub const TASSADAR_SUBROUTINE_LIBRARY_ABLATION_TEST_COMMAND: &str =
    "cargo test -p psionic-research subroutine_library_ablation_report_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSubroutineLibraryAblationDelta {
    pub held_out_workload_family: TassadarExecutorSubroutineWorkloadFamily,
    pub full_trace_reuse_bps: u32,
    pub subroutine_library_reuse_bps: u32,
    pub delta_bps: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSubroutineLibraryAblationReport {
    pub schema_version: u16,
    pub comparison_id: String,
    pub report_ref: String,
    pub regeneration_commands: Vec<String>,
    pub full_trace_manifest_digest: String,
    pub subroutine_manifest_digest: String,
    pub mode_reports: Vec<TassadarExecutorSubroutineOodProxyReport>,
    pub workload_deltas: Vec<TassadarSubroutineLibraryAblationDelta>,
    pub claim_class: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarSubroutineLibraryAblationReport {
    fn new(mode_reports: Vec<TassadarExecutorSubroutineOodProxyReport>) -> Result<Self, TassadarSubroutineLibraryAblationError> {
        let full_trace_manifest = build_tassadar_executor_subroutine_dataset_manifest(
            TassadarExecutorSupervisionTargetMode::FullTrace,
        )?;
        let subroutine_manifest = build_tassadar_executor_subroutine_dataset_manifest(
            TassadarExecutorSupervisionTargetMode::SubroutineLibrary,
        )?;
        let workload_deltas = [
            TassadarExecutorSubroutineWorkloadFamily::Sort,
            TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
        ]
        .into_iter()
        .map(|held_out_workload_family| {
            let full_trace = mode_reports
                .iter()
                .find(|report| {
                    report.held_out_workload_family == held_out_workload_family
                        && report.supervision_mode == TassadarExecutorSupervisionTargetMode::FullTrace
                })
                .expect("full-trace OOD report should exist");
            let subroutine = mode_reports
                .iter()
                .find(|report| {
                    report.held_out_workload_family == held_out_workload_family
                        && report.supervision_mode
                            == TassadarExecutorSupervisionTargetMode::SubroutineLibrary
                })
                .expect("subroutine OOD report should exist");
            TassadarSubroutineLibraryAblationDelta {
                held_out_workload_family,
                full_trace_reuse_bps: full_trace.held_out_reuse_bps,
                subroutine_library_reuse_bps: subroutine.held_out_reuse_bps,
                delta_bps: subroutine.held_out_reuse_bps as i32
                    - full_trace.held_out_reuse_bps as i32,
            }
        })
        .collect::<Vec<_>>();
        let improved_workloads = workload_deltas
            .iter()
            .filter(|delta| delta.delta_bps > 0)
            .count();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            comparison_id: String::from("tassadar.subroutine_library_ablation.v0"),
            report_ref: String::from(TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_REF),
            regeneration_commands: vec![
                String::from(TASSADAR_SUBROUTINE_LIBRARY_ABLATION_EXAMPLE_COMMAND),
                String::from(TASSADAR_SUBROUTINE_LIBRARY_ABLATION_TEST_COMMAND),
            ],
            full_trace_manifest_digest: full_trace_manifest.manifest_digest,
            subroutine_manifest_digest: subroutine_manifest.manifest_digest,
            mode_reports,
            workload_deltas,
            claim_class: String::from("learned_bounded_success"),
            claim_boundary: String::from(
                "this report compares held-out target-label reuse under full-trace versus reusable-subroutine supervision on the seeded sort, shortest-path, and sudoku-style corpus only; it is an OOD supervision proxy for research planning, not a trained-model exactness claim",
            ),
            summary: format!(
                "Bounded subroutine-library ablation now publishes held-out label-reuse deltas across 3 workload families; reusable subroutine targets improve OOD reuse on {} of 3 held-out families while remaining explicitly a research-only supervision proxy rather than a trained exactness result.",
                improved_workloads
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_subroutine_library_ablation_report|", &report);
        Ok(report)
    }
}

#[derive(Debug, Error)]
pub enum TassadarSubroutineLibraryAblationError {
    #[error(transparent)]
    Dataset(#[from] TassadarExecutorSubroutineDatasetError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_subroutine_library_ablation_report()
-> Result<TassadarSubroutineLibraryAblationReport, TassadarSubroutineLibraryAblationError> {
    let mode_reports = [
        TassadarExecutorSupervisionTargetMode::FullTrace,
        TassadarExecutorSupervisionTargetMode::SubroutineLibrary,
    ]
    .into_iter()
    .flat_map(|supervision_mode| {
        [
            TassadarExecutorSubroutineWorkloadFamily::Sort,
            TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
        ]
        .into_iter()
        .map(move |held_out_workload_family| {
            build_tassadar_executor_subroutine_ood_proxy(&TassadarExecutorSubroutineOodProxyConfig {
                supervision_mode,
                held_out_workload_family,
            })
        })
    })
    .collect::<Result<Vec<_>, _>>()?;
    TassadarSubroutineLibraryAblationReport::new(mode_reports)
}

pub fn run_tassadar_subroutine_library_ablation(
    output_dir: &Path,
) -> Result<TassadarSubroutineLibraryAblationReport, TassadarSubroutineLibraryAblationError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarSubroutineLibraryAblationError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_subroutine_library_ablation_report()?;
    let report_path = output_dir.join(TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarSubroutineLibraryAblationError::Write {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded =
        serde_json::to_vec(value).expect("subroutine library ablation report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_REF,
        build_tassadar_subroutine_library_ablation_report,
        run_tassadar_subroutine_library_ablation,
        TassadarSubroutineLibraryAblationReport,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn subroutine_library_ablation_improves_ood_reuse_on_all_seeded_workloads()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_subroutine_library_ablation_report()?;
        assert_eq!(report.mode_reports.len(), 6);
        assert!(report
            .workload_deltas
            .iter()
            .all(|delta| delta.delta_bps > 0));
        Ok(())
    }

    #[test]
    fn subroutine_library_ablation_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_subroutine_library_ablation_report()?;
        let persisted: TassadarSubroutineLibraryAblationReport =
            read_repo_json(TASSADAR_SUBROUTINE_LIBRARY_ABLATION_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn subroutine_library_ablation_report_writes_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report = run_tassadar_subroutine_library_ablation(temp_dir.path())?;
        let persisted: TassadarSubroutineLibraryAblationReport = serde_json::from_slice(
            &std::fs::read(
                temp_dir
                    .path()
                    .join("tassadar_subroutine_library_ablation_report.json"),
            )?,
        )?;
        assert_eq!(persisted, report);
        Ok(())
    }
}
