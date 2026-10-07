use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::TassadarSequenceSplit;
use psionic_eval::{
    TassadarExecutorBaselineComparisonError, TassadarExecutorBaselineComparisonReport,
    TassadarExecutorBaselineFamilyKind, TassadarExecutorBaselineFamilyReport,
    build_tassadar_executor_baseline_comparison_report, build_tassadar_sequence_dataset,
};
use psionic_models::{
    TassadarExecutorAttentionTransformer, TassadarExecutorTrainableSurface,
    TassadarExecutorTransformer,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the first four-family same-corpus learned
/// baseline comparison.
pub const TASSADAR_EXECUTOR_BASELINE_COMPARISON_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v12";
/// Canonical machine-readable report written at the comparison root.
pub const TASSADAR_EXECUTOR_BASELINE_COMPARISON_REPORT_FILE: &str =
    "architecture_comparison_report.json";

const FAMILY_REPORT_FILE: &str = "family_report.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const HULL_SPECIALIZED_LOOKUP_DIR: &str = "hull_specialized_lookup";
const SPARSE_LOOKUP_BASELINE_DIR: &str = "sparse_lookup_baseline";
const HYBRID_ATTENTION_BASELINE_DIR: &str = "hybrid_attention_baseline";
const RECURRENT_WINDOWED_BASELINE_DIR: &str = "recurrent_windowed_baseline";
const PROMPT_WINDOW_TOKEN_CAP: usize = 256;
const TARGET_TOKEN_CAP: usize = 32;

/// Persisted per-family bundle for the learned baseline comparison root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorBaselineRunBundle {
    /// Stable family kind under test.
    pub family_kind: TassadarExecutorBaselineFamilyKind,
    /// Stable run identifier for the family bundle.
    pub run_id: String,
    /// Relative family directory under the comparison root.
    pub run_directory: String,
    /// Stable model identifier.
    pub model_id: String,
    /// Stable descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable trained weight digest.
    pub trained_weight_digest: String,
    /// Explicit claim boundary for the family.
    pub claim_boundary: String,
    /// Relative family report path.
    pub family_report_file: String,
    /// Relative model descriptor path.
    pub model_descriptor_file: String,
    /// Stable digest of the family report.
    pub family_report_digest: String,
    /// Stable digest over the full bundle.
    pub bundle_digest: String,
}

impl TassadarExecutorBaselineRunBundle {
    fn new(
        family_kind: TassadarExecutorBaselineFamilyKind,
        run_directory: &str,
        report: &TassadarExecutorBaselineFamilyReport,
    ) -> Self {
        let run_id = format!("tassadar-executor-baseline-{}-v1", family_kind.label());
        let mut bundle = Self {
            family_kind,
            run_id,
            run_directory: run_directory.to_string(),
            model_id: report.model_id.clone(),
            model_descriptor_digest: report.model_descriptor_digest.clone(),
            trained_weight_digest: report.trained_weight_digest.clone(),
            claim_boundary: report.claim_boundary.clone(),
            family_report_file: String::from(FAMILY_REPORT_FILE),
            model_descriptor_file: String::from(MODEL_DESCRIPTOR_FILE),
            family_report_digest: report.report_digest.clone(),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_executor_baseline_run_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Errors while persisting the learned baseline comparison root.
#[derive(Debug, Error)]
pub enum TassadarExecutorBaselineComparisonPersistError {
    /// Dataset build or validation failed.
    #[error(transparent)]
    Eval(#[from] TassadarExecutorBaselineComparisonError),
    /// Dataset generation failed.
    #[error(transparent)]
    Dataset(#[from] psionic_eval::TassadarSequenceEvalError),
    /// Creating one output directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Writing one artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

/// Executes the first four-family same-corpus learned baseline comparison and
/// writes the top-level report plus per-family bundles.
pub fn run_tassadar_executor_baseline_comparison(
    output_dir: &Path,
) -> Result<TassadarExecutorBaselineComparisonReport, TassadarExecutorBaselineComparisonPersistError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarExecutorBaselineComparisonPersistError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let dataset = build_tassadar_sequence_dataset(
        psionic_eval::TassadarSequenceWorkload::SudokuV0,
        "train-v0",
    )?;
    let hull_specialized_lookup = TassadarExecutorTransformer::sudoku_v0_with_surface(
        TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
    );
    let sparse_lookup_baseline = TassadarExecutorTransformer::sudoku_v0_sparse_with_surface(
        TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
    );
    let hybrid_attention_baseline = TassadarExecutorAttentionTransformer::sudoku_v0();
    let recurrent_windowed_baseline = TassadarExecutorTransformer::sudoku_v0_windowed_with_surface(
        TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
    );

    let comparison = build_tassadar_executor_baseline_comparison_report(
        &dataset.dataset,
        TassadarSequenceSplit::Validation,
        PROMPT_WINDOW_TOKEN_CAP,
        TARGET_TOKEN_CAP,
        &hull_specialized_lookup,
        &sparse_lookup_baseline,
        &hybrid_attention_baseline,
        &recurrent_windowed_baseline,
    )?;

    persist_family_bundle(
        output_dir,
        HULL_SPECIALIZED_LOOKUP_DIR,
        &comparison.hull_specialized_lookup,
        hull_specialized_lookup.descriptor(),
    )?;
    persist_family_bundle(
        output_dir,
        SPARSE_LOOKUP_BASELINE_DIR,
        &comparison.sparse_lookup_baseline,
        sparse_lookup_baseline.descriptor(),
    )?;
    persist_family_bundle(
        output_dir,
        HYBRID_ATTENTION_BASELINE_DIR,
        &comparison.hybrid_attention_baseline,
        hybrid_attention_baseline.descriptor(),
    )?;
    persist_family_bundle(
        output_dir,
        RECURRENT_WINDOWED_BASELINE_DIR,
        &comparison.recurrent_windowed_baseline,
        recurrent_windowed_baseline.descriptor(),
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_BASELINE_COMPARISON_REPORT_FILE),
        &comparison,
    )?;

    Ok(comparison)
}

fn persist_family_bundle<T>(
    output_dir: &Path,
    family_dir_name: &str,
    report: &TassadarExecutorBaselineFamilyReport,
    descriptor: &T,
) -> Result<(), TassadarExecutorBaselineComparisonPersistError>
where
    T: Serialize,
{
    let family_dir = output_dir.join(family_dir_name);
    fs::create_dir_all(&family_dir).map_err(|error| {
        TassadarExecutorBaselineComparisonPersistError::CreateDir {
            path: family_dir.display().to_string(),
            error,
        }
    })?;
    let bundle = TassadarExecutorBaselineRunBundle::new(
        report.family_kind,
        family_dir_name,
        report,
    );
    write_json(family_dir.join(FAMILY_REPORT_FILE), report)?;
    write_json(family_dir.join(MODEL_DESCRIPTOR_FILE), descriptor)?;
    write_json(family_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(())
}

fn write_json<T>(
    path: PathBuf,
    value: &T,
) -> Result<(), TassadarExecutorBaselineComparisonPersistError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)
        .expect("Tassadar learned baseline comparison artifact should serialize");
    fs::write(&path, bytes).map_err(|error| {
        TassadarExecutorBaselineComparisonPersistError::Write {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("Tassadar learned baseline comparison bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_EXECUTOR_BASELINE_COMPARISON_REPORT_FILE,
        run_tassadar_executor_baseline_comparison,
    };

    #[test]
    fn baseline_comparison_writes_all_family_bundles_and_top_level_report()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let report = run_tassadar_executor_baseline_comparison(temp.path())?;

        assert!(report.recurrent_changes_long_trace_contract);
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_BASELINE_COMPARISON_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join("hull_specialized_lookup")
                .join("run_bundle.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("sparse_lookup_baseline")
                .join("run_bundle.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("hybrid_attention_baseline")
                .join("run_bundle.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("recurrent_windowed_baseline")
                .join("run_bundle.json")
                .exists()
        );
        Ok(())
    }
}
