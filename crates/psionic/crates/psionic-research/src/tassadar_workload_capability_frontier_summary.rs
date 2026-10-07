use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_tassadar_workload_capability_frontier_report, TassadarWorkloadCapabilityFrontierReport,
    TassadarWorkloadCapabilityFrontierReportError, TassadarWorkloadFrontierObservationPosture,
};
use psionic_models::TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_SUMMARY_REPORT_REF;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Repo-facing summary over the current workload capability frontier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityFrontierSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Underlying eval-facing frontier report.
    pub frontier_report: TassadarWorkloadCapabilityFrontierReport,
    /// Preferred-lane recommendation counts copied into the research summary.
    pub preferred_lane_counts: BTreeMap<String, u32>,
    /// Observation posture counts across all preferred-lane observations.
    pub observation_posture_counts: BTreeMap<String, u32>,
    /// Workload families with at least one under-mapped preferred lane.
    pub under_mapped_workload_family_ids: Vec<String>,
    /// Workload families still marked refusal-first.
    pub refusal_first_workload_family_ids: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Summary sentence.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarWorkloadCapabilityFrontierSummaryReport {
    fn new(frontier_report: TassadarWorkloadCapabilityFrontierReport) -> Self {
        let preferred_lane_counts = frontier_report.preferred_lane_counts.clone();
        let under_mapped_workload_family_ids =
            frontier_report.under_mapped_workload_family_ids.clone();
        let refusal_first_workload_family_ids =
            frontier_report.refusal_first_workload_family_ids.clone();
        let observation_posture_counts = count_observation_postures(&frontier_report);
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.workload_capability_frontier.summary.v1"),
            frontier_report,
            preferred_lane_counts,
            observation_posture_counts,
            under_mapped_workload_family_ids,
            refusal_first_workload_family_ids,
            claim_boundary: String::from(
                "this summary keeps the workload-frontier result as a research and capability-truth surface. Benchmarked or artifact-backed rows here do not widen served capability posture by themselves, and refusal-first or under-mapped regions remain explicit rather than being absorbed into aggregate success claims",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Workload frontier summary keeps {} workload families explicit, with {} under-mapped families, {} refusal-first families, and posture counts {:?}.",
            report.frontier_report.frontier_rows.len(),
            report.under_mapped_workload_family_ids.len(),
            report.refusal_first_workload_family_ids.len(),
            report.observation_posture_counts,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_workload_capability_frontier_summary_report|",
            &report,
        );
        report
    }
}

/// Frontier-summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarWorkloadCapabilityFrontierSummaryReportError {
    /// Frontier report build failed.
    #[error(transparent)]
    Frontier(#[from] TassadarWorkloadCapabilityFrontierReportError),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed workload-frontier research summary.
pub fn build_tassadar_workload_capability_frontier_summary_report() -> Result<
    TassadarWorkloadCapabilityFrontierSummaryReport,
    TassadarWorkloadCapabilityFrontierSummaryReportError,
> {
    let frontier_report = build_tassadar_workload_capability_frontier_report()?;
    Ok(TassadarWorkloadCapabilityFrontierSummaryReport::new(
        frontier_report,
    ))
}

/// Returns the canonical absolute path for the committed workload-frontier summary.
#[must_use]
pub fn tassadar_workload_capability_frontier_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_SUMMARY_REPORT_REF)
}

/// Writes the committed workload-frontier research summary.
pub fn write_tassadar_workload_capability_frontier_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarWorkloadCapabilityFrontierSummaryReport,
    TassadarWorkloadCapabilityFrontierSummaryReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWorkloadCapabilityFrontierSummaryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_workload_capability_frontier_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWorkloadCapabilityFrontierSummaryReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn count_observation_postures(
    frontier_report: &TassadarWorkloadCapabilityFrontierReport,
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for row in &frontier_report.frontier_rows {
        for observation in &row.observed_lanes {
            *counts
                .entry(observation_posture_label(observation.posture).to_string())
                .or_insert(0) += 1;
        }
    }
    counts
}

fn observation_posture_label(posture: TassadarWorkloadFrontierObservationPosture) -> &'static str {
    match posture {
        TassadarWorkloadFrontierObservationPosture::Exact => "exact",
        TassadarWorkloadFrontierObservationPosture::FallbackExact => "fallback_exact",
        TassadarWorkloadFrontierObservationPosture::ResearchOnly => "research_only",
        TassadarWorkloadFrontierObservationPosture::RefuseFirst => "refuse_first",
        TassadarWorkloadFrontierObservationPosture::UnderMapped => "under_mapped",
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWorkloadCapabilityFrontierSummaryReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarWorkloadCapabilityFrontierSummaryReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWorkloadCapabilityFrontierSummaryReportError::Deserialize {
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
        build_tassadar_workload_capability_frontier_summary_report, read_repo_json,
        tassadar_workload_capability_frontier_summary_report_path,
        write_tassadar_workload_capability_frontier_summary_report,
        TassadarWorkloadCapabilityFrontierSummaryReport,
    };
    use psionic_models::TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_SUMMARY_REPORT_REF;

    #[test]
    fn workload_capability_frontier_summary_keeps_under_mapped_and_refusal_first_explicit() {
        let report = build_tassadar_workload_capability_frontier_summary_report().expect("report");

        assert!(report
            .under_mapped_workload_family_ids
            .contains(&String::from("micro_wasm_kernel")));
        assert!(report
            .refusal_first_workload_family_ids
            .contains(&String::from("sudoku_class")));
        assert!(
            report
                .observation_posture_counts
                .get("research_only")
                .copied()
                .unwrap_or(0)
                > 0
        );
    }

    #[test]
    fn workload_capability_frontier_summary_matches_committed_truth() {
        let generated =
            build_tassadar_workload_capability_frontier_summary_report().expect("summary report");
        let committed: TassadarWorkloadCapabilityFrontierSummaryReport =
            read_repo_json(TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_workload_capability_frontier_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_workload_capability_frontier_summary.json");
        let written = write_tassadar_workload_capability_frontier_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarWorkloadCapabilityFrontierSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_workload_capability_frontier_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_workload_capability_frontier_summary.json")
        );
    }
}
