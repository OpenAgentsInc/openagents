use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarExecutorBoundaryExactnessReport, TassadarExecutorFirstTokenConfusionReport,
};
use psionic_models::TassadarExecutorTrainableSurface;
use psionic_train::{
    TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE,
    TASSADAR_EXECUTOR_FIRST_TOKEN_CONFUSION_FILE, TassadarExecutorReferenceRunBundle,
    TassadarExecutorRunError, TassadarExecutorTrainingConfig, TassadarExecutorTrainingReport,
    execute_tassadar_training_run,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the Phase 13 same-corpus ablation.
pub const TASSADAR_TRAINABLE_SURFACE_ABLATION_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1";
/// Canonical machine-readable ablation artifact.
pub const TASSADAR_TRAINABLE_SURFACE_ABLATION_REPORT_FILE: &str =
    "trainable_surface_ablation.json";

const TRAINING_REPORT_FILE: &str = "training_report.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";

/// One same-corpus trainable-surface result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrainableSurfaceAblationEntry {
    /// Stable trainable surface label.
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Relative run directory under the ablation root.
    pub run_directory: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable run-bundle digest.
    pub run_bundle_digest: String,
    /// Selected checkpoint identifier.
    pub selected_checkpoint_id: String,
    /// Stage that produced the selected checkpoint.
    pub selected_stage_id: String,
    /// Aggregate validation exactness.
    pub aggregate_target_token_exactness_bps: u32,
    /// First-target exactness.
    pub first_target_exactness_bps: u32,
    /// First-8 exactness.
    pub first_8_token_exactness_bps: u32,
    /// First-32 exactness.
    pub first_32_token_exactness_bps: u32,
    /// Exact validation trace count.
    pub exact_trace_case_count: u32,
    /// Token-zero divergence count.
    pub token_zero_divergence_case_count: u32,
    /// Whether this surface beats the baseline on the boundary rank tuple.
    pub materially_better_than_baseline: bool,
}

/// Machine-readable Phase 13 ablation report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrainableSurfaceAblationReport {
    /// Stable workload family.
    pub workload: String,
    /// Frozen dataset version.
    pub dataset_version: String,
    /// Preserved baseline surface.
    pub baseline_surface: TassadarExecutorTrainableSurface,
    /// Ordered evaluated entries.
    pub entries: Vec<TassadarTrainableSurfaceAblationEntry>,
    /// Recommended next surface when one materially beat the baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_surface: Option<TassadarExecutorTrainableSurface>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarTrainableSurfaceAblationReport {
    fn new(
        entries: Vec<TassadarTrainableSurfaceAblationEntry>,
        recommended_surface: Option<TassadarExecutorTrainableSurface>,
    ) -> Self {
        let mut report = Self {
            workload: String::from("sudoku_v0"),
            dataset_version: String::from("train-v0"),
            baseline_surface: TassadarExecutorTrainableSurface::OutputHeadOnly,
            entries,
            recommended_surface,
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_trainable_surface_ablation_report|", &report);
        report
    }
}

/// Errors while running or writing the Phase 13 ablation.
#[derive(Debug, Error)]
pub enum TassadarTrainableSurfaceAblationError {
    /// One training run failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Reading one persisted artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding one persisted artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// File path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Creating one output directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Writing the ablation report failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// No baseline entry was produced.
    #[error("trainable surface ablation did not produce the output-head-only baseline")]
    MissingBaseline,
}

/// Executes the Phase 13 same-corpus trainable-surface ablation.
pub fn run_tassadar_trainable_surface_ablation(
    output_dir: &Path,
) -> Result<TassadarTrainableSurfaceAblationReport, TassadarTrainableSurfaceAblationError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarTrainableSurfaceAblationError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let surfaces = [
        TassadarExecutorTrainableSurface::OutputHeadOnly,
        TassadarExecutorTrainableSurface::OutputHeadAndTokenEmbeddings,
        TassadarExecutorTrainableSurface::OutputHeadAndEmbeddings,
        TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
    ];
    let mut entries = Vec::new();

    for surface in surfaces {
        let run_dir = output_dir.join(surface.label());
        let config = phase13_surface_config(surface);
        execute_tassadar_training_run(
            run_dir.as_path(),
            &config,
            None,
        )?;
        let bundle: TassadarExecutorReferenceRunBundle =
            read_json(run_dir.join(RUN_BUNDLE_FILE), "tassadar_reference_run_bundle")?;
        let training_report: TassadarExecutorTrainingReport =
            read_json(run_dir.join(TRAINING_REPORT_FILE), "tassadar_training_report")?;
        let boundary_report: TassadarExecutorBoundaryExactnessReport = read_json(
            run_dir.join(TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE),
            "tassadar_boundary_exactness_report",
        )?;
        let first_token_confusion_report: TassadarExecutorFirstTokenConfusionReport = read_json(
            run_dir.join(TASSADAR_EXECUTOR_FIRST_TOKEN_CONFUSION_FILE),
            "tassadar_first_token_confusion_report",
        )?;
        let selected_stage_id = training_report
            .epoch_reports
            .iter()
            .find(|epoch| epoch.checkpoint_id == training_report.best_checkpoint_id)
            .map(|epoch| epoch.stage_id.clone())
            .unwrap_or_else(|| String::from("unknown"));
        entries.push(TassadarTrainableSurfaceAblationEntry {
            trainable_surface: surface,
            run_directory: surface.label().to_string(),
            run_id: bundle.run_id.clone(),
            run_bundle_digest: bundle.bundle_digest.clone(),
            selected_checkpoint_id: training_report.best_checkpoint_id.clone(),
            selected_stage_id,
            aggregate_target_token_exactness_bps: boundary_report
                .aggregate_target_token_exactness_bps,
            first_target_exactness_bps: boundary_report.first_target_exactness_bps,
            first_8_token_exactness_bps: boundary_report.first_8_token_exactness_bps,
            first_32_token_exactness_bps: boundary_report.first_32_token_exactness_bps,
            exact_trace_case_count: boundary_report.exact_trace_case_count,
            token_zero_divergence_case_count: first_token_confusion_report
                .token_zero_divergence_case_count,
            materially_better_than_baseline: false,
        });
    }

    let baseline_entry = entries
        .iter()
        .find(|entry| {
            entry.trainable_surface == TassadarExecutorTrainableSurface::OutputHeadOnly
        })
        .cloned()
        .ok_or(TassadarTrainableSurfaceAblationError::MissingBaseline)?;
    for entry in &mut entries {
        entry.materially_better_than_baseline =
            surface_rank(entry) > surface_rank(&baseline_entry);
    }
    let recommended_surface = entries
        .iter()
        .filter(|entry| entry.materially_better_than_baseline)
        .max_by_key(|entry| surface_rank(entry))
        .map(|entry| entry.trainable_surface);
    let report = TassadarTrainableSurfaceAblationReport::new(entries, recommended_surface);
    let report_path = output_dir.join(TASSADAR_TRAINABLE_SURFACE_ABLATION_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report).expect("ablation report should serialize");
    fs::write(&report_path, bytes).map_err(|error| TassadarTrainableSurfaceAblationError::Write {
        path: report_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn phase13_surface_config(
    trainable_surface: TassadarExecutorTrainableSurface,
) -> TassadarExecutorTrainingConfig {
    let mut config = TassadarExecutorTrainingConfig::boundary_curriculum_reference()
        .with_trainable_surface(trainable_surface);
    config.run_id = format!(
        "tassadar-executor-transformer-sudoku-v0-surface-{}-v1",
        trainable_surface.label()
    );
    config
}

fn surface_rank(entry: &TassadarTrainableSurfaceAblationEntry) -> (u32, u32, u32, u32, u32) {
    (
        entry.first_target_exactness_bps,
        entry.first_32_token_exactness_bps,
        entry.first_8_token_exactness_bps,
        entry.exact_trace_case_count,
        entry.aggregate_target_token_exactness_bps,
    )
}

fn read_json<T>(
    path: PathBuf,
    artifact_kind: &str,
) -> Result<T, TassadarTrainableSurfaceAblationError>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(&path).map_err(|error| TassadarTrainableSurfaceAblationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarTrainableSurfaceAblationError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar trainable surface ablation should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_TRAINABLE_SURFACE_ABLATION_REPORT_FILE, run_tassadar_trainable_surface_ablation,
    };

    #[test]
    fn trainable_surface_ablation_writes_report_and_surface_runs()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let report = run_tassadar_trainable_surface_ablation(temp.path())?;

        assert_eq!(report.entries.len(), 4);
        assert!(
            temp.path()
                .join(TASSADAR_TRAINABLE_SURFACE_ABLATION_REPORT_FILE)
                .exists()
        );
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.trainable_surface
                    == psionic_models::TassadarExecutorTrainableSurface::OutputHeadOnly)
        );
        Ok(())
    }
}
