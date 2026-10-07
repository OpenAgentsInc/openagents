use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TassadarModuleStateExecutorPublication, TassadarModuleStateProgramFamily,
    TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF, tassadar_module_state_executor_publication,
};
use psionic_train::{
    TassadarModuleStateCurriculumSuite, TassadarModuleStateCurriculumVariantId,
    build_tassadar_module_state_curriculum_suite,
};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MODULE_STATE_ARCHITECTURE_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_module_state_architecture_report";
pub const TASSADAR_MODULE_STATE_ARCHITECTURE_TEST_COMMAND: &str =
    "cargo test -p psionic-research module_state_architecture_report_matches_committed_truth -- --nocapture";

/// Held-out family delta between the flat-prefix baseline and the full module curriculum.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateHeldOutDelta {
    /// Stable module family.
    pub family: TassadarModuleStateProgramFamily,
    /// Baseline later-window exactness.
    pub baseline_later_window_exactness_bps: u32,
    /// Candidate later-window exactness.
    pub candidate_later_window_exactness_bps: u32,
    /// Baseline final-state exactness.
    pub baseline_final_state_exactness_bps: u32,
    /// Candidate final-state exactness.
    pub candidate_final_state_exactness_bps: u32,
    /// Candidate minus baseline later-window exactness.
    pub later_window_delta_bps: i32,
    /// Candidate minus baseline final-state exactness.
    pub final_state_delta_bps: i32,
    /// Baseline gap minus candidate gap.
    pub trace_to_final_state_gap_reduction_bps: i32,
}

/// Aggregate ablation row for one curriculum variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateCurriculumAblationSummary {
    /// Stable variant identifier.
    pub variant_id: String,
    /// Human-readable variant summary.
    pub description: String,
    /// Average later-window exactness across all families.
    pub later_window_average_bps: u32,
    /// Average final-state exactness across all families.
    pub final_state_average_bps: u32,
    /// Average held-out later-window exactness.
    pub held_out_later_window_average_bps: u32,
    /// Average held-out final-state exactness.
    pub held_out_final_state_average_bps: u32,
    /// Average trace-to-final-state gap.
    pub average_trace_to_final_state_gap_bps: u32,
}

/// Committed research report for the module-state executor redesign lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateArchitectureReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Repo-facing model publication for the lane.
    pub publication: TassadarModuleStateExecutorPublication,
    /// Training-facing curriculum suite for the lane.
    pub curriculum_suite: TassadarModuleStateCurriculumSuite,
    /// Held-out family deltas against the baseline.
    pub held_out_family_deltas: Vec<TassadarModuleStateHeldOutDelta>,
    /// Ordered curriculum ablation summaries.
    pub curriculum_ablations: Vec<TassadarModuleStateCurriculumAblationSummary>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Summary sentence for the current report.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarModuleStateArchitectureReport {
    fn new(
        publication: TassadarModuleStateExecutorPublication,
        curriculum_suite: TassadarModuleStateCurriculumSuite,
        held_out_family_deltas: Vec<TassadarModuleStateHeldOutDelta>,
        curriculum_ablations: Vec<TassadarModuleStateCurriculumAblationSummary>,
    ) -> Self {
        let total_held_out_later_window_delta_bps =
            held_out_family_deltas
                .iter()
                .map(|delta| delta.later_window_delta_bps)
                .sum::<i32>();
        let total_held_out_gap_reduction_bps = held_out_family_deltas
            .iter()
            .map(|delta| delta.trace_to_final_state_gap_reduction_bps)
            .sum::<i32>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.module_state_architecture.report.v1"),
            publication,
            curriculum_suite,
            held_out_family_deltas,
            curriculum_ablations,
            claim_boundary: String::from(
                "this report freezes one learned bounded research-only architecture comparison for module-scale Wasm workloads; it compares a flat-prefix baseline against a frame-aware module-state candidate over deterministic curriculum and held-out-family metrics, and does not claim served exactness, arbitrary Wasm closure, or benchmark-gated promotion",
            ),
            summary: format!(
                "Held-out module-family deltas are now explicit for the module-state redesign lane: cumulative later-window gain={} bps and cumulative trace-to-final-state gap reduction={} bps across the held-out parsing and vm_style families.",
                total_held_out_later_window_delta_bps,
                total_held_out_gap_reduction_bps,
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_module_state_architecture_report|", &report);
        report
    }
}

/// Report build failures for the module-state redesign lane.
#[derive(Debug, Error)]
pub enum TassadarModuleStateArchitectureReportError {
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

/// Builds the committed research report for the module-state executor redesign lane.
#[must_use]
pub fn build_tassadar_module_state_architecture_report() -> TassadarModuleStateArchitectureReport {
    let publication = tassadar_module_state_executor_publication();
    let curriculum_suite = build_tassadar_module_state_curriculum_suite();
    let held_out_family_deltas = build_held_out_family_deltas(&curriculum_suite);
    let curriculum_ablations = curriculum_suite
        .variants
        .iter()
        .map(|variant| TassadarModuleStateCurriculumAblationSummary {
            variant_id: String::from(variant.variant_id.as_str()),
            description: variant.description.clone(),
            later_window_average_bps: variant.later_window_average_bps,
            final_state_average_bps: variant.final_state_average_bps,
            held_out_later_window_average_bps: variant.held_out_later_window_average_bps,
            held_out_final_state_average_bps: variant.held_out_final_state_average_bps,
            average_trace_to_final_state_gap_bps: variant.average_trace_to_final_state_gap_bps,
        })
        .collect::<Vec<_>>();
    TassadarModuleStateArchitectureReport::new(
        publication,
        curriculum_suite,
        held_out_family_deltas,
        curriculum_ablations,
    )
}

/// Returns the canonical absolute path for the committed module-state report.
#[must_use]
pub fn tassadar_module_state_architecture_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF)
}

/// Writes the committed research report for the module-state executor redesign lane.
pub fn write_tassadar_module_state_architecture_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleStateArchitectureReport, TassadarModuleStateArchitectureReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleStateArchitectureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_state_architecture_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleStateArchitectureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_held_out_family_deltas(
    suite: &TassadarModuleStateCurriculumSuite,
) -> Vec<TassadarModuleStateHeldOutDelta> {
    let variants = suite
        .variants
        .iter()
        .map(|variant| (variant.variant_id, variant))
        .collect::<BTreeMap<_, _>>();
    let baseline = variants
        .get(&TassadarModuleStateCurriculumVariantId::FlatPrefixBaseline)
        .expect("baseline variant should exist");
    let candidate = variants
        .get(&TassadarModuleStateCurriculumVariantId::FullModuleCurriculum)
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
            TassadarModuleStateHeldOutDelta {
                family: baseline_eval.family,
                baseline_later_window_exactness_bps: baseline_eval.later_window_exactness_bps,
                candidate_later_window_exactness_bps: candidate_eval.later_window_exactness_bps,
                baseline_final_state_exactness_bps: baseline_eval.final_state_exactness_bps,
                candidate_final_state_exactness_bps: candidate_eval.final_state_exactness_bps,
                later_window_delta_bps: candidate_eval.later_window_exactness_bps as i32
                    - baseline_eval.later_window_exactness_bps as i32,
                final_state_delta_bps: candidate_eval.final_state_exactness_bps as i32
                    - baseline_eval.final_state_exactness_bps as i32,
                trace_to_final_state_gap_reduction_bps:
                    baseline_eval.trace_to_final_state_gap_bps as i32
                        - candidate_eval.trace_to_final_state_gap_bps as i32,
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
) -> Result<T, TassadarModuleStateArchitectureReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarModuleStateArchitectureReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModuleStateArchitectureReportError::Deserialize {
            artifact_kind: String::from("tassadar_module_state_architecture_report"),
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
        TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF, TassadarModuleStateArchitectureReport,
        build_tassadar_module_state_architecture_report,
        tassadar_module_state_architecture_report_path, write_tassadar_module_state_architecture_report,
        read_repo_json,
    };

    #[test]
    fn module_state_architecture_report_improves_held_out_families() {
        let report = build_tassadar_module_state_architecture_report();
        assert_eq!(report.held_out_family_deltas.len(), 2);
        assert!(report
            .held_out_family_deltas
            .iter()
            .all(|delta| delta.later_window_delta_bps > 0));
        assert!(report
            .held_out_family_deltas
            .iter()
            .all(|delta| delta.trace_to_final_state_gap_reduction_bps > 0));
    }

    #[test]
    fn module_state_architecture_report_matches_committed_truth() {
        let generated = build_tassadar_module_state_architecture_report();
        let committed: TassadarModuleStateArchitectureReport =
            read_repo_json(TASSADAR_MODULE_STATE_ARCHITECTURE_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_module_state_architecture_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_module_state_architecture_report.json");
        let written = write_tassadar_module_state_architecture_report(&output_path)
            .expect("write report");
        let persisted: TassadarModuleStateArchitectureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(tassadar_module_state_architecture_report_path().file_name().and_then(|name| name.to_str()), Some("tassadar_module_state_architecture_report.json"));
    }
}
