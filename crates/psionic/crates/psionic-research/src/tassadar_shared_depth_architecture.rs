use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_tassadar_shared_depth_halting_calibration_report,
    TassadarSharedDepthHaltingCalibrationReport,
};
use psionic_models::{
    tassadar_shared_depth_executor_publication, TassadarSharedDepthExecutorPublication,
    TassadarSharedDepthWorkloadFamily, TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF,
};
use psionic_train::{
    build_tassadar_shared_depth_curriculum_suite, TassadarSharedDepthCurriculumSuite,
    TassadarSharedDepthCurriculumVariantId,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_SHARED_DEPTH_ARCHITECTURE_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_shared_depth_architecture_report";
pub const TASSADAR_SHARED_DEPTH_ARCHITECTURE_TEST_COMMAND: &str =
    "cargo test -p psionic-research shared_depth_architecture_report_matches_committed_truth -- --nocapture";

/// Held-out family delta between the flat-prefix baseline and the dynamic-halting candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthHeldOutDelta {
    /// Stable workload family.
    pub family: TassadarSharedDepthWorkloadFamily,
    /// Baseline later-window exactness.
    pub baseline_later_window_exactness_bps: u32,
    /// Candidate later-window exactness.
    pub candidate_later_window_exactness_bps: u32,
    /// Baseline budget exhaustion rate.
    pub baseline_budget_exhaustion_rate_bps: u32,
    /// Candidate budget exhaustion rate.
    pub candidate_budget_exhaustion_rate_bps: u32,
    /// Candidate minus baseline later-window exactness.
    pub later_window_delta_bps: i32,
    /// Baseline minus candidate budget exhaustion.
    pub exhaustion_reduction_bps: i32,
}

/// Aggregate ablation row for one shared-depth curriculum variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthVariantSummary {
    /// Stable variant identifier.
    pub variant_id: String,
    /// Human-readable variant summary.
    pub description: String,
    /// Average later-window exactness across all families.
    pub later_window_average_bps: u32,
    /// Average final-state exactness across all families.
    pub final_state_average_bps: u32,
    /// Average budget exhaustion rate across all families.
    pub average_budget_exhaustion_rate_bps: u32,
}

/// Committed research report for the shared-depth executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedDepthArchitectureReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Repo-facing model publication for the lane.
    pub publication: TassadarSharedDepthExecutorPublication,
    /// Training-facing curriculum suite for the lane.
    pub curriculum_suite: TassadarSharedDepthCurriculumSuite,
    /// Eval-facing halting calibration report.
    pub halting_report: TassadarSharedDepthHaltingCalibrationReport,
    /// Held-out family deltas against the baseline.
    pub held_out_family_deltas: Vec<TassadarSharedDepthHeldOutDelta>,
    /// Ordered variant summaries.
    pub variant_summaries: Vec<TassadarSharedDepthVariantSummary>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Summary sentence for the current report.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarSharedDepthArchitectureReport {
    fn new(
        publication: TassadarSharedDepthExecutorPublication,
        curriculum_suite: TassadarSharedDepthCurriculumSuite,
        halting_report: TassadarSharedDepthHaltingCalibrationReport,
        held_out_family_deltas: Vec<TassadarSharedDepthHeldOutDelta>,
        variant_summaries: Vec<TassadarSharedDepthVariantSummary>,
    ) -> Self {
        let cumulative_later_window_delta_bps = held_out_family_deltas
            .iter()
            .map(|delta| delta.later_window_delta_bps)
            .sum::<i32>();
        let cumulative_exhaustion_reduction_bps = held_out_family_deltas
            .iter()
            .map(|delta| delta.exhaustion_reduction_bps)
            .sum::<i32>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.shared_depth_architecture.report.v1"),
            publication,
            curriculum_suite,
            halting_report,
            held_out_family_deltas,
            variant_summaries,
            claim_boundary: String::from(
                "this report freezes one research-only shared-depth architecture comparison for loop-heavy kernel traces and call-heavy module traces; it compares a flat-prefix baseline, a shared-depth fixed-budget lane, and a dynamic-halting shared-depth lane, and does not claim served exactness, arbitrary Wasm closure, or benchmark-gated promotion",
            ),
            summary: format!(
                "Shared-depth recurrent refinement now has one explicit repo-facing architecture report: cumulative held-out later-window gain={} bps and cumulative budget-exhaustion reduction={} bps for the dynamic-halting lane relative to the flat-prefix baseline.",
                cumulative_later_window_delta_bps,
                cumulative_exhaustion_reduction_bps,
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_shared_depth_architecture_report|",
            &report,
        );
        report
    }
}

/// Report build failures for the shared-depth executor lane.
#[derive(Debug, Error)]
pub enum TassadarSharedDepthArchitectureReportError {
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed research report for the shared-depth executor lane.
#[must_use]
pub fn build_tassadar_shared_depth_architecture_report() -> TassadarSharedDepthArchitectureReport {
    let publication = tassadar_shared_depth_executor_publication();
    let curriculum_suite = build_tassadar_shared_depth_curriculum_suite();
    let halting_report = build_tassadar_shared_depth_halting_calibration_report();
    let held_out_family_deltas = build_held_out_family_deltas(&curriculum_suite);
    let variant_summaries = curriculum_suite
        .variants
        .iter()
        .map(|variant| TassadarSharedDepthVariantSummary {
            variant_id: String::from(variant.variant_id.as_str()),
            description: variant.description.clone(),
            later_window_average_bps: variant.later_window_average_bps,
            final_state_average_bps: variant.final_state_average_bps,
            average_budget_exhaustion_rate_bps: variant.average_budget_exhaustion_rate_bps,
        })
        .collect::<Vec<_>>();
    TassadarSharedDepthArchitectureReport::new(
        publication,
        curriculum_suite,
        halting_report,
        held_out_family_deltas,
        variant_summaries,
    )
}

/// Returns the canonical absolute path for the committed shared-depth report.
#[must_use]
pub fn tassadar_shared_depth_architecture_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF)
}

/// Writes the committed research report for the shared-depth executor lane.
pub fn write_tassadar_shared_depth_architecture_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSharedDepthArchitectureReport, TassadarSharedDepthArchitectureReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedDepthArchitectureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_shared_depth_architecture_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedDepthArchitectureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_held_out_family_deltas(
    suite: &TassadarSharedDepthCurriculumSuite,
) -> Vec<TassadarSharedDepthHeldOutDelta> {
    let variants = suite
        .variants
        .iter()
        .map(|variant| (variant.variant_id, variant))
        .collect::<std::collections::BTreeMap<_, _>>();
    let baseline = variants
        .get(&TassadarSharedDepthCurriculumVariantId::FlatPrefixBaseline)
        .expect("baseline variant should exist");
    let candidate = variants
        .get(&TassadarSharedDepthCurriculumVariantId::SharedDepthDynamicHalting)
        .expect("candidate variant should exist");
    baseline
        .family_evals
        .iter()
        .filter(|family| family.held_out)
        .map(|baseline_eval| {
            let candidate_eval = candidate
                .family_evals
                .iter()
                .find(|family| family.family == baseline_eval.family)
                .expect("candidate family should exist");
            TassadarSharedDepthHeldOutDelta {
                family: baseline_eval.family,
                baseline_later_window_exactness_bps: baseline_eval.later_window_exactness_bps,
                candidate_later_window_exactness_bps: candidate_eval.later_window_exactness_bps,
                baseline_budget_exhaustion_rate_bps: baseline_eval.budget_exhaustion_rate_bps,
                candidate_budget_exhaustion_rate_bps: candidate_eval.budget_exhaustion_rate_bps,
                later_window_delta_bps: candidate_eval.later_window_exactness_bps as i32
                    - baseline_eval.later_window_exactness_bps as i32,
                exhaustion_reduction_bps: baseline_eval.budget_exhaustion_rate_bps as i32
                    - candidate_eval.budget_exhaustion_rate_bps as i32,
            }
        })
        .collect()
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
) -> Result<T, TassadarSharedDepthArchitectureReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarSharedDepthArchitectureReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedDepthArchitectureReportError::Deserialize {
            artifact_kind: String::from("tassadar_shared_depth_architecture_report"),
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
        build_tassadar_shared_depth_architecture_report, read_repo_json,
        tassadar_shared_depth_architecture_report_path,
        write_tassadar_shared_depth_architecture_report, TassadarSharedDepthArchitectureReport,
        TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF,
    };

    #[test]
    fn shared_depth_architecture_report_improves_held_out_family() {
        let report = build_tassadar_shared_depth_architecture_report();

        assert_eq!(report.held_out_family_deltas.len(), 1);
        assert!(report
            .held_out_family_deltas
            .iter()
            .all(|delta| delta.later_window_delta_bps > 0));
        assert!(report
            .held_out_family_deltas
            .iter()
            .all(|delta| delta.exhaustion_reduction_bps > 0));
        assert!(
            report
                .halting_report
                .dynamic_halting_beats_fixed_budget_on_call_exhaustion
        );
    }

    #[test]
    fn shared_depth_architecture_report_matches_committed_truth() {
        let generated = build_tassadar_shared_depth_architecture_report();
        let committed: TassadarSharedDepthArchitectureReport =
            read_repo_json(TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_depth_architecture_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_shared_depth_architecture_report.json");
        let written =
            write_tassadar_shared_depth_architecture_report(&output_path).expect("write report");
        let persisted: TassadarSharedDepthArchitectureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_shared_depth_architecture_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_shared_depth_architecture_report.json")
        );
    }
}
