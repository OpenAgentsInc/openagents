use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::TassadarSequenceSplit;
use psionic_eval::{
    TassadarExecutorArchitectureComparisonError, TassadarExecutorArchitectureComparisonReport,
    TassadarExecutorArchitectureFamilyKind, TassadarExecutorArchitectureFamilyReport,
    TassadarSequenceEvalError, build_tassadar_executor_architecture_comparison_report,
    build_tassadar_sequence_dataset, evaluate_attention_family_for_architecture_comparison,
};
use psionic_models::{TassadarExecutorAttentionTransformer, TassadarExecutorTransformerDescriptor};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the bounded Phase 15 same-corpus comparison.
pub const TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1";
/// Canonical output root for the follow-on comparison that swaps in the
/// trained attention-family checkpoint.
pub const TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_TRAINED_ATTENTION_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v2";
/// Canonical output root for the boundary-first attention-family comparison.
pub const TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_BOUNDARY_ATTENTION_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v7";
/// Canonical machine-readable architecture-comparison artifact.
pub const TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_REPORT_FILE: &str =
    "architecture_comparison_report.json";

const LOOKUP_BASELINE_DIR: &str = "lookup_baseline";
const EXECUTOR_ATTENTION_DIR: &str = "executor_attention_candidate";
const FAMILY_REPORT_FILE: &str = "family_report.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";
const LOOKUP_BASELINE_SOURCE_RUN_BUNDLE: &str = "crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1/output_head_embeddings_and_small_learned_mixer/run_bundle.json";
const LOOKUP_BASELINE_PRESERVED_FAMILY_REPORT: &str = "crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1/lookup_baseline/family_report.json";
const LOOKUP_BASELINE_PRESERVED_MODEL_DESCRIPTOR: &str = "crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1/lookup_baseline/model_descriptor.json";
const TRAINED_ATTENTION_CHECKPOINT_STATE: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1/checkpoint_state.json";
const TRAINED_ATTENTION_SOURCE_RUN_BUNDLE: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1/run_bundle.json";
const BOUNDARY_ATTENTION_CHECKPOINT_STATE: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v5/checkpoint_state.json";
const BOUNDARY_ATTENTION_SOURCE_RUN_BUNDLE: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v5/run_bundle.json";
const PROMPT_WINDOW_TOKEN_CAP: usize = 256;
const TARGET_TOKEN_CAP: usize = 32;

/// Persisted per-family bundle for the bounded architecture comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorArchitectureRunBundle {
    /// Stable family kind under test.
    pub family_kind: TassadarExecutorArchitectureFamilyKind,
    /// Stable run identifier for the family bundle.
    pub run_id: String,
    /// Relative family directory under the comparison root.
    pub run_directory: String,
    /// Stable model id.
    pub model_id: String,
    /// Stable descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable weight digest.
    pub trained_weight_digest: String,
    /// Explicit claim boundary for the family.
    pub claim_boundary: String,
    /// Prompt window used during bounded evaluation.
    pub prompt_window_token_cap: u32,
    /// Target-token cap used during bounded evaluation.
    pub target_token_cap: u32,
    /// Relative file containing the family report.
    pub family_report_file: String,
    /// Relative file containing the descriptor snapshot.
    pub model_descriptor_file: String,
    /// Optional repo-local source artifact reference when the bundle was built
    /// from a preserved baseline artifact rather than from a fresh run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_ref: Option<String>,
    /// Stable digest of the family report written beside the bundle.
    pub family_report_digest: String,
    /// Stable digest over the full run bundle.
    pub bundle_digest: String,
}

impl TassadarExecutorArchitectureRunBundle {
    fn new(
        family_kind: TassadarExecutorArchitectureFamilyKind,
        run_directory: &str,
        run_revision: &str,
        report: &TassadarExecutorArchitectureFamilyReport,
        source_artifact_ref: Option<String>,
    ) -> Self {
        let run_id = format!(
            "tassadar-executor-architecture-{}-{}",
            family_kind.label(),
            run_revision
        );
        let mut bundle = Self {
            family_kind,
            run_id,
            run_directory: run_directory.to_string(),
            model_id: report.model_id.clone(),
            model_descriptor_digest: report.model_descriptor_digest.clone(),
            trained_weight_digest: report.trained_weight_digest.clone(),
            claim_boundary: report.claim_boundary.clone(),
            prompt_window_token_cap: report.prompt_window_token_cap,
            target_token_cap: report.target_token_cap,
            family_report_file: String::from(FAMILY_REPORT_FILE),
            model_descriptor_file: String::from(MODEL_DESCRIPTOR_FILE),
            source_artifact_ref,
            family_report_digest: report.report_digest.clone(),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_executor_architecture_run_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Errors while writing the bounded Phase 15 research artifacts.
#[derive(Debug, Error)]
pub enum TassadarExecutorArchitectureComparisonPersistError {
    /// Building the bounded sequence dataset failed.
    #[error(transparent)]
    DatasetBuild(#[from] TassadarSequenceEvalError),
    /// One dataset or evaluation step failed.
    #[error(transparent)]
    Eval(#[from] TassadarExecutorArchitectureComparisonError),
    /// Loading the preserved lookup baseline failed.
    #[error("failed to load lookup baseline from `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding the preserved lookup checkpoint failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// File path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Materializing the trained attention checkpoint failed.
    #[error("failed to materialize trained attention checkpoint `{checkpoint_id}`: {error}")]
    MaterializeAttention {
        /// Stable checkpoint identifier.
        checkpoint_id: String,
        /// Source error.
        error: crate::TassadarExecutorAttentionTrainingError,
    },
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

/// Executes the bounded same-corpus architecture comparison and writes the
/// resulting family bundles plus the top-level report.
pub fn run_tassadar_executor_architecture_comparison(
    output_dir: &Path,
) -> Result<
    TassadarExecutorArchitectureComparisonReport,
    TassadarExecutorArchitectureComparisonPersistError,
> {
    run_tassadar_executor_architecture_comparison_with_attention_candidate(
        output_dir,
        "v1",
        &TassadarExecutorAttentionTransformer::sudoku_v0(),
        None,
    )
}

/// Executes the bounded same-corpus architecture comparison with the trained
/// attention-family checkpoint from the canonical attention-training bundle.
pub fn run_tassadar_executor_architecture_comparison_with_trained_attention(
    output_dir: &Path,
) -> Result<
    TassadarExecutorArchitectureComparisonReport,
    TassadarExecutorArchitectureComparisonPersistError,
> {
    let trained_attention =
        load_attention_candidate_from_checkpoint(TRAINED_ATTENTION_CHECKPOINT_STATE)?;
    run_tassadar_executor_architecture_comparison_with_attention_candidate(
        output_dir,
        "v2",
        &trained_attention,
        Some(String::from(TRAINED_ATTENTION_SOURCE_RUN_BUNDLE)),
    )
}

/// Executes the bounded same-corpus architecture comparison with the
/// boundary-first trained attention-family checkpoint.
pub fn run_tassadar_executor_architecture_comparison_with_boundary_attention(
    output_dir: &Path,
) -> Result<
    TassadarExecutorArchitectureComparisonReport,
    TassadarExecutorArchitectureComparisonPersistError,
> {
    let trained_attention =
        load_attention_candidate_from_checkpoint(BOUNDARY_ATTENTION_CHECKPOINT_STATE)?;
    run_tassadar_executor_architecture_comparison_with_attention_candidate(
        output_dir,
        "v7",
        &trained_attention,
        Some(String::from(BOUNDARY_ATTENTION_SOURCE_RUN_BUNDLE)),
    )
}

fn load_lookup_baseline() -> Result<
    (
        TassadarExecutorArchitectureFamilyReport,
        TassadarExecutorTransformerDescriptor,
    ),
    TassadarExecutorArchitectureComparisonPersistError,
> {
    let report = read_json(
        repo_root().join(LOOKUP_BASELINE_PRESERVED_FAMILY_REPORT),
        "tassadar_executor_architecture_family_report",
    )?;
    let descriptor = read_json(
        repo_root().join(LOOKUP_BASELINE_PRESERVED_MODEL_DESCRIPTOR),
        "tassadar_executor_transformer_descriptor",
    )?;
    Ok((report, descriptor))
}

fn load_attention_candidate_from_checkpoint(
    checkpoint_path: &str,
) -> Result<TassadarExecutorAttentionTransformer, TassadarExecutorArchitectureComparisonPersistError>
{
    let checkpoint: crate::TassadarExecutorAttentionCheckpointState = read_json(
        repo_root().join(checkpoint_path),
        "tassadar_executor_attention_checkpoint_state",
    )?;
    checkpoint.materialize_model().map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::MaterializeAttention {
            checkpoint_id: checkpoint.checkpoint_id,
            error,
        }
    })
}

fn run_tassadar_executor_architecture_comparison_with_attention_candidate(
    output_dir: &Path,
    run_revision: &str,
    executor_attention_candidate: &TassadarExecutorAttentionTransformer,
    attention_source_artifact_ref: Option<String>,
) -> Result<
    TassadarExecutorArchitectureComparisonReport,
    TassadarExecutorArchitectureComparisonPersistError,
> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let dataset = build_tassadar_sequence_dataset(
        psionic_eval::TassadarSequenceWorkload::SudokuV0,
        "train-v0",
    )?;
    let (lookup_report, lookup_descriptor) = load_lookup_baseline()?;
    let candidate_report = evaluate_attention_family_for_architecture_comparison(
        executor_attention_candidate,
        &dataset.dataset,
        TassadarSequenceSplit::Validation,
        PROMPT_WINDOW_TOKEN_CAP,
        TARGET_TOKEN_CAP,
    )?;
    let comparison = build_tassadar_executor_architecture_comparison_report(
        &dataset.dataset,
        TassadarSequenceSplit::Validation,
        PROMPT_WINDOW_TOKEN_CAP,
        TARGET_TOKEN_CAP,
        lookup_report.clone(),
        candidate_report.clone(),
    )?;

    persist_lookup_bundle(output_dir, run_revision, &lookup_descriptor, &lookup_report)?;
    persist_attention_bundle(
        output_dir,
        run_revision,
        executor_attention_candidate,
        &candidate_report,
        attention_source_artifact_ref,
    )?;
    write_json(
        output_dir.join(TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_REPORT_FILE),
        &comparison,
    )?;

    Ok(comparison)
}

fn persist_lookup_bundle(
    output_dir: &Path,
    run_revision: &str,
    descriptor: &TassadarExecutorTransformerDescriptor,
    report: &TassadarExecutorArchitectureFamilyReport,
) -> Result<(), TassadarExecutorArchitectureComparisonPersistError> {
    let family_dir = output_dir.join(LOOKUP_BASELINE_DIR);
    fs::create_dir_all(&family_dir).map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::CreateDir {
            path: family_dir.display().to_string(),
            error,
        }
    })?;
    let bundle = TassadarExecutorArchitectureRunBundle::new(
        TassadarExecutorArchitectureFamilyKind::LookupBaseline,
        LOOKUP_BASELINE_DIR,
        run_revision,
        report,
        Some(String::from(LOOKUP_BASELINE_SOURCE_RUN_BUNDLE)),
    );
    write_json(family_dir.join(FAMILY_REPORT_FILE), report)?;
    write_json(family_dir.join(MODEL_DESCRIPTOR_FILE), descriptor)?;
    write_json(family_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(())
}

fn persist_attention_bundle(
    output_dir: &Path,
    run_revision: &str,
    model: &TassadarExecutorAttentionTransformer,
    report: &TassadarExecutorArchitectureFamilyReport,
    source_artifact_ref: Option<String>,
) -> Result<(), TassadarExecutorArchitectureComparisonPersistError> {
    let family_dir = output_dir.join(EXECUTOR_ATTENTION_DIR);
    fs::create_dir_all(&family_dir).map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::CreateDir {
            path: family_dir.display().to_string(),
            error,
        }
    })?;
    let bundle = TassadarExecutorArchitectureRunBundle::new(
        TassadarExecutorArchitectureFamilyKind::ExecutorAttentionCandidate,
        EXECUTOR_ATTENTION_DIR,
        run_revision,
        report,
        source_artifact_ref,
    );
    write_json(family_dir.join(FAMILY_REPORT_FILE), report)?;
    write_json(family_dir.join(MODEL_DESCRIPTOR_FILE), model.descriptor())?;
    write_json(family_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(())
}

fn read_json<T>(
    path: PathBuf,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorArchitectureComparisonPersistError>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(&path).map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json<T>(
    path: PathBuf,
    value: &T,
) -> Result<(), TassadarExecutorArchitectureComparisonPersistError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).expect("research artifact should serialize");
    fs::write(&path, bytes).map_err(|error| {
        TassadarExecutorArchitectureComparisonPersistError::Write {
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
        .expect("Tassadar executor architecture comparison bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_BOUNDARY_ATTENTION_OUTPUT_DIR,
        TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_REPORT_FILE,
        TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_TRAINED_ATTENTION_OUTPUT_DIR,
        run_tassadar_executor_architecture_comparison,
        run_tassadar_executor_architecture_comparison_with_boundary_attention,
        run_tassadar_executor_architecture_comparison_with_trained_attention,
    };

    #[test]
    fn architecture_comparison_writes_both_family_bundles_and_top_level_report()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let report = run_tassadar_executor_architecture_comparison(temp.path())?;

        assert!(report.candidate_closer_to_article_fidelity);
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join("lookup_baseline")
                .join("run_bundle.json")
                .exists()
        );
        assert!(
            temp.path()
                .join("executor_attention_candidate")
                .join("run_bundle.json")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn trained_attention_architecture_comparison_writes_top_level_report()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let report =
            run_tassadar_executor_architecture_comparison_with_trained_attention(temp.path())?;

        assert!(report.candidate_closer_to_article_fidelity);
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join("executor_attention_candidate")
                .join("run_bundle.json")
                .exists()
        );
        assert_ne!(
            TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_TRAINED_ATTENTION_OUTPUT_DIR,
            ""
        );
        Ok(())
    }

    #[test]
    fn boundary_attention_architecture_comparison_writes_top_level_report()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let report =
            run_tassadar_executor_architecture_comparison_with_boundary_attention(temp.path())?;

        assert!(report.candidate_closer_to_article_fidelity);
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join("executor_attention_candidate")
                .join("run_bundle.json")
                .exists()
        );
        assert_ne!(
            TASSADAR_EXECUTOR_ARCHITECTURE_COMPARISON_BOUNDARY_ATTENTION_OUTPUT_DIR,
            ""
        );
        Ok(())
    }
}
