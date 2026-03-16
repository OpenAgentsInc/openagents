use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use psionic_data::TassadarSequenceSplit;
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind,
};
use psionic_eval::{
    EvalArtifact, TassadarExecutorBoundaryExactnessReport,
    TassadarExecutorDivergenceHistogramReport, TassadarExecutorFirstTokenConfusionReport,
    TassadarExecutorLinearBenchmarkReport, benchmark_tassadar_executor_linear_decode,
    build_tassadar_executor_boundary_exactness_report,
    build_tassadar_executor_divergence_histogram_report,
    build_tassadar_executor_first_token_confusion_report, build_tassadar_sequence_dataset,
};
use psionic_models::{
    TassadarExecutorTrainableSurface, TassadarExecutorTransformer,
    TassadarExecutorTransformerDescriptor, TassadarExecutorTransformerError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarExecutorTrainingConfig, TassadarExecutorTrainingError, TassadarExecutorTrainingReport,
    TassadarSequenceTrainingError, TassadarSequenceTrainingManifest,
    build_tassadar_sequence_training_manifest, train_tassadar_executor_transformer,
};

/// Stable schema version for persisted Tassadar reference-run bundles.
pub const TASSADAR_EXECUTOR_REFERENCE_RUN_SCHEMA_VERSION: u16 = 1;
/// Stable run identifier for the first committed Sudoku-v0 reference run.
pub const TASSADAR_EXECUTOR_REFERENCE_RUN_ID: &str =
    "tassadar-executor-transformer-sudoku-v0-reference-run-v0";
/// Stable run identifier for the first boundary-curriculum follow-on run.
pub const TASSADAR_EXECUTOR_BOUNDARY_RUN_ID: &str =
    "tassadar-executor-transformer-sudoku-v0-boundary-v1";
/// Stable run identifier for the first promotion-gate follow-on run.
pub const TASSADAR_EXECUTOR_PROMOTION_RUN_ID: &str =
    "tassadar-executor-transformer-sudoku-v0-promotion-v1";
/// Stable checkpoint family for persisted trained executor checkpoints.
pub const TASSADAR_EXECUTOR_CHECKPOINT_FAMILY: &str = "train.tassadar.executor_transformer";
/// Canonical repo path used for the first committed reference run.
pub const TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0";
/// Canonical repo path used for the first boundary-curriculum follow-on run.
pub const TASSADAR_EXECUTOR_BOUNDARY_RUN_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_boundary_v1";
/// Canonical repo path used for the first promotion-gate follow-on run.
pub const TASSADAR_EXECUTOR_PROMOTION_RUN_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1";

const TRAINING_MANIFEST_FILE: &str = "training_manifest.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const LINEAR_BENCHMARK_REPORT_FILE: &str = "linear_benchmark_report.json";
pub const TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE: &str = "boundary_exactness_report.json";
pub const TASSADAR_EXECUTOR_DIVERGENCE_HISTOGRAM_FILE: &str = "divergence_histogram.json";
pub const TASSADAR_EXECUTOR_FIRST_TOKEN_CONFUSION_FILE: &str = "first_token_confusion_report.json";
pub const TASSADAR_EXECUTOR_CHECKPOINT_LEADERBOARD_FILE: &str = "checkpoint_leaderboard.json";
pub const TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_REPORT_FILE: &str =
    "neural_hull_benchmark_report.json";
const CHECKPOINT_ARTIFACT_FILE: &str = "checkpoint_artifact.json";
const CHECKPOINT_STATE_FILE: &str = "checkpoint_state.json";
const CHECKPOINT_MANIFEST_FILE: &str = "checkpoint_manifest.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";

fn default_trainable_surface() -> TassadarExecutorTrainableSurface {
    TassadarExecutorTrainableSurface::OutputHeadOnly
}

fn tassadar_progress_updates_enabled() -> bool {
    match env::var("OPENAGENTS_TASSADAR_PROGRESS") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "no")
        }
        Err(_) => !cfg!(test),
    }
}

fn emit_tassadar_progress(message: impl AsRef<str>) {
    if tassadar_progress_updates_enabled() {
        eprintln!("{}", message.as_ref());
    }
}

/// One sparse embedding row override persisted inside a checkpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorEmbeddingRowOverride {
    /// Zero-based row index.
    pub row_index: u32,
    /// Full row values.
    pub values: Vec<f32>,
}

/// Frozen checkpoint payload for the first trained neural executor run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorCheckpointState {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable training run identifier.
    pub run_id: String,
    /// Base model identifier the checkpoint applies to.
    pub base_model_id: String,
    /// Frozen descriptor digest for the trained model.
    pub trained_model_descriptor_digest: String,
    /// Frozen trained weight digest.
    pub trained_weight_digest: String,
    /// Frozen training-manifest digest.
    pub training_manifest_digest: String,
    /// Active trainable surface for the checkpoint.
    #[serde(default = "default_trainable_surface")]
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Digest over the trained output projection only.
    pub output_projection_digest: String,
    /// Digest over the trained output bias only.
    pub output_bias_digest: String,
    /// Digest over the trained token embeddings when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_embeddings_digest: Option<String>,
    /// Digest over the trained position rows when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_embedding_rows_digest: Option<String>,
    /// Digest over the learned mixer projection when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_learned_mixer_projection_digest: Option<String>,
    /// Digest over the learned mixer bias when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_learned_mixer_bias_digest: Option<String>,
    /// Trained output projection.
    pub output_projection: Vec<f32>,
    /// Trained output bias.
    pub output_bias: Vec<f32>,
    /// Trained token embeddings when the surface enables them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_embeddings: Option<Vec<f32>>,
    /// Sparse trained position-embedding rows when the surface enables them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub position_embedding_rows: Vec<TassadarExecutorEmbeddingRowOverride>,
    /// Learned mixer projection when the surface enables it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_learned_mixer_projection: Option<Vec<f32>>,
    /// Learned mixer bias when the surface enables it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_learned_mixer_bias: Option<Vec<f32>>,
    /// Stable digest over the full checkpoint payload.
    pub state_digest: String,
}

impl TassadarExecutorCheckpointState {
    fn new(
        checkpoint_id: &str,
        run_id: &str,
        training_manifest: &TassadarSequenceTrainingManifest,
        model: &TassadarExecutorTransformer,
        observed_position_count: usize,
    ) -> Self {
        let output_projection = model.weights().output_projection().to_vec();
        let output_bias = model.weights().output_bias().to_vec();
        let token_embeddings = model
            .trainable_surface()
            .trains_token_embeddings()
            .then(|| model.weights().token_embeddings().to_vec());
        let position_embedding_rows = if model.trainable_surface().trains_position_embeddings() {
            capture_position_embedding_rows(model, observed_position_count)
        } else {
            Vec::new()
        };
        let small_learned_mixer_projection = model
            .trainable_surface()
            .trains_small_learned_mixer()
            .then(|| model.weights().small_learned_mixer_projection().to_vec());
        let small_learned_mixer_bias = model
            .trainable_surface()
            .trains_small_learned_mixer()
            .then(|| model.weights().small_learned_mixer_bias().to_vec());
        let output_projection_digest = stable_digest(
            b"psionic_tassadar_executor_output_projection|",
            &output_projection,
        );
        let output_bias_digest =
            stable_digest(b"psionic_tassadar_executor_output_bias|", &output_bias);
        let token_embeddings_digest = token_embeddings
            .as_ref()
            .map(|tensor| stable_digest(b"psionic_tassadar_executor_token_embeddings|", tensor));
        let position_embedding_rows_digest = (!position_embedding_rows.is_empty()).then(|| {
            stable_digest(
                b"psionic_tassadar_executor_position_embedding_rows|",
                &position_embedding_rows,
            )
        });
        let small_learned_mixer_projection_digest =
            small_learned_mixer_projection.as_ref().map(|tensor| {
                stable_digest(
                    b"psionic_tassadar_executor_small_learned_mixer_projection|",
                    tensor,
                )
            });
        let small_learned_mixer_bias_digest = small_learned_mixer_bias.as_ref().map(|tensor| {
            stable_digest(
                b"psionic_tassadar_executor_small_learned_mixer_bias|",
                tensor,
            )
        });
        let mut checkpoint = Self {
            checkpoint_id: checkpoint_id.to_string(),
            run_id: run_id.to_string(),
            base_model_id: model.descriptor().model.model_id.clone(),
            trained_model_descriptor_digest: model.descriptor().stable_digest(),
            trained_weight_digest: model.descriptor().weights.digest.clone(),
            training_manifest_digest: training_manifest.manifest_digest.clone(),
            trainable_surface: model.trainable_surface(),
            output_projection_digest,
            output_bias_digest,
            token_embeddings_digest,
            position_embedding_rows_digest,
            small_learned_mixer_projection_digest,
            small_learned_mixer_bias_digest,
            output_projection,
            output_bias,
            token_embeddings,
            position_embedding_rows,
            small_learned_mixer_projection,
            small_learned_mixer_bias,
            state_digest: String::new(),
        };
        checkpoint.state_digest =
            stable_digest(b"psionic_tassadar_executor_checkpoint_state|", &checkpoint);
        checkpoint
    }

    /// Materializes a trained model from the checkpoint state.
    pub fn materialize_model(
        &self,
    ) -> Result<TassadarExecutorTransformer, TassadarExecutorRunError> {
        let mut model = match self.base_model_id.as_str() {
            TassadarExecutorTransformer::MODEL_ID => {
                TassadarExecutorTransformer::sudoku_v0_with_surface(self.trainable_surface)
            }
            TassadarExecutorTransformer::SUDOKU_9X9_MODEL_ID => {
                TassadarExecutorTransformer::sudoku_9x9_with_surface(self.trainable_surface)
            }
            actual => {
                return Err(TassadarExecutorRunError::UnexpectedBaseModel {
                    expected: format!(
                        "{}/{}",
                        TassadarExecutorTransformer::MODEL_ID,
                        TassadarExecutorTransformer::SUDOKU_9X9_MODEL_ID
                    ),
                    actual: actual.to_string(),
                });
            }
        };
        model.apply_trained_output_head(
            self.output_projection.as_slice(),
            self.output_bias.as_slice(),
        )?;
        if let Some(token_embeddings) = self.token_embeddings.as_ref() {
            if token_embeddings.len() != model.weights().token_embeddings().len() {
                return Err(TassadarExecutorRunError::Model(
                    TassadarExecutorTransformerError::WeightLengthMismatch {
                        tensor: String::from("token_embeddings"),
                        expected: model.weights().token_embeddings().len(),
                        actual: token_embeddings.len(),
                    },
                ));
            }
            model
                .weights_mut()
                .token_embeddings_mut()
                .copy_from_slice(token_embeddings.as_slice());
        }
        if !self.position_embedding_rows.is_empty() {
            let embedding_dim = model.descriptor().config.embedding_dim;
            let max_rows = model.descriptor().config.max_sequence_tokens;
            for row in &self.position_embedding_rows {
                let row_index = row.row_index as usize;
                if row.values.len() != embedding_dim || row_index >= max_rows {
                    return Err(TassadarExecutorRunError::Model(
                        TassadarExecutorTransformerError::WeightLengthMismatch {
                            tensor: String::from("position_embedding_row"),
                            expected: embedding_dim,
                            actual: row.values.len(),
                        },
                    ));
                }
                let start = row_index * embedding_dim;
                model.weights_mut().position_embeddings_mut()[start..start + embedding_dim]
                    .copy_from_slice(row.values.as_slice());
            }
        }
        if let Some(mixer_projection) = self.small_learned_mixer_projection.as_ref() {
            if mixer_projection.len() != model.weights().small_learned_mixer_projection().len() {
                return Err(TassadarExecutorRunError::Model(
                    TassadarExecutorTransformerError::WeightLengthMismatch {
                        tensor: String::from("small_learned_mixer_projection"),
                        expected: model.weights().small_learned_mixer_projection().len(),
                        actual: mixer_projection.len(),
                    },
                ));
            }
            model
                .weights_mut()
                .small_learned_mixer_projection_mut()
                .copy_from_slice(mixer_projection.as_slice());
        }
        if let Some(mixer_bias) = self.small_learned_mixer_bias.as_ref() {
            if mixer_bias.len() != model.weights().small_learned_mixer_bias().len() {
                return Err(TassadarExecutorRunError::Model(
                    TassadarExecutorTransformerError::WeightLengthMismatch {
                        tensor: String::from("small_learned_mixer_bias"),
                        expected: model.weights().small_learned_mixer_bias().len(),
                        actual: mixer_bias.len(),
                    },
                ));
            }
            model
                .weights_mut()
                .small_learned_mixer_bias_mut()
                .copy_from_slice(mixer_bias.as_slice());
        }
        model.refresh_after_training();
        if model.descriptor().stable_digest() != self.trained_model_descriptor_digest {
            return Err(TassadarExecutorRunError::DescriptorDigestMismatch {
                expected: self.trained_model_descriptor_digest.clone(),
                actual: model.descriptor().stable_digest(),
            });
        }
        if model.descriptor().weights.digest != self.trained_weight_digest {
            return Err(TassadarExecutorRunError::WeightDigestMismatch {
                expected: self.trained_weight_digest.clone(),
                actual: model.descriptor().weights.digest.clone(),
            });
        }
        Ok(model)
    }
}

/// Control-plane-friendly checkpoint artifact for the first trained run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorCheckpointArtifact {
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// File containing the checkpoint payload.
    pub checkpoint_state_file: String,
    /// Manifest reference for the checkpoint payload.
    pub checkpoint_manifest: DatastreamManifestRef,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Trained weight digest.
    pub trained_weight_digest: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl TassadarExecutorCheckpointArtifact {
    fn new(
        checkpoint_state: &TassadarExecutorCheckpointState,
        checkpoint_manifest: &DatastreamManifest,
    ) -> Self {
        let checkpoint_ref = format!("checkpoint://{}", checkpoint_state.checkpoint_id);
        let mut artifact = Self {
            checkpoint_id: checkpoint_state.checkpoint_id.clone(),
            checkpoint_family: String::from(TASSADAR_EXECUTOR_CHECKPOINT_FAMILY),
            checkpoint_ref,
            checkpoint_state_file: String::from(CHECKPOINT_STATE_FILE),
            checkpoint_manifest: checkpoint_manifest.manifest_ref(),
            trained_model_descriptor_digest: checkpoint_state
                .trained_model_descriptor_digest
                .clone(),
            trained_weight_digest: checkpoint_state.trained_weight_digest.clone(),
            artifact_digest: String::new(),
        };
        artifact.artifact_digest =
            stable_digest(b"psionic_tassadar_executor_checkpoint_artifact|", &artifact);
        artifact
    }
}

/// Persisted model artifact identity for the first trained neural executor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorModelArtifact {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable run identifier that produced the artifact.
    pub run_id: String,
    /// Full trained model descriptor.
    pub descriptor: TassadarExecutorTransformerDescriptor,
    /// Training report digest backing this artifact.
    pub training_report_digest: String,
    /// Linear benchmark report digest backing this artifact.
    pub linear_benchmark_report_digest: String,
    /// Checkpoint manifest backing the artifact.
    pub checkpoint_manifest: DatastreamManifestRef,
    /// Stable artifact digest.
    pub artifact_digest: String,
}

impl TassadarExecutorModelArtifact {
    fn new(
        run_id: &str,
        model: &TassadarExecutorTransformer,
        training_report: &TassadarExecutorTrainingReport,
        benchmark_report: &TassadarExecutorLinearBenchmarkReport,
        checkpoint_manifest: &DatastreamManifest,
    ) -> Self {
        let mut artifact = Self {
            artifact_id: format!("{}.model_artifact", model.descriptor().model.model_id),
            run_id: run_id.to_string(),
            descriptor: model.descriptor().clone(),
            training_report_digest: training_report.report_digest.clone(),
            linear_benchmark_report_digest: stable_digest(
                b"psionic_tassadar_executor_linear_benchmark_report|",
                benchmark_report,
            ),
            checkpoint_manifest: checkpoint_manifest.manifest_ref(),
            artifact_digest: String::new(),
        };
        artifact.artifact_digest =
            stable_digest(b"psionic_tassadar_executor_model_artifact|", &artifact);
        artifact
    }
}

/// Aggregate persisted bundle for one first-run Sudoku-v0 training execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorReferenceRunBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable run identifier.
    pub run_id: String,
    /// Active trainable surface for the run.
    #[serde(default = "default_trainable_surface")]
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Frozen dataset version.
    pub dataset_version: String,
    /// Frozen dataset storage key.
    pub dataset_storage_key: String,
    /// Frozen dataset digest.
    pub dataset_digest: String,
    /// Frozen tokenizer digest.
    pub tokenizer_digest: String,
    /// Frozen vocabulary digest.
    pub vocabulary_digest: String,
    /// Frozen training-manifest digest.
    pub training_manifest_digest: String,
    /// Trained model descriptor digest.
    pub trained_model_descriptor_digest: String,
    /// Trained weight digest.
    pub trained_weight_digest: String,
    /// Persisted training report digest.
    pub training_report_digest: String,
    /// Persisted benchmark report digest.
    pub linear_benchmark_report_digest: String,
    /// Persisted boundary exactness report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_exactness_report_digest: Option<String>,
    /// Persisted divergence histogram report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divergence_histogram_report_digest: Option<String>,
    /// Persisted first-token confusion report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_token_confusion_report_digest: Option<String>,
    /// Persisted checkpoint leaderboard digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_leaderboard_report_digest: Option<String>,
    /// Persisted exactness-curve report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exactness_curve_report_digest: Option<String>,
    /// Persisted failure-sample report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_samples_report_digest: Option<String>,
    /// Persisted best-checkpoint manifest digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_checkpoint_manifest_digest: Option<String>,
    /// Persisted exact-trace-sample report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_trace_samples_report_digest: Option<String>,
    /// Persisted promotion-gate report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_gate_report_digest: Option<String>,
    /// Persisted neural hull benchmark report digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural_hull_benchmark_report_digest: Option<String>,
    /// Persisted checkpoint artifact digest.
    pub checkpoint_artifact_digest: String,
    /// Persisted model artifact digest.
    pub model_artifact_digest: String,
    /// Persisted file inventory.
    pub artifacts: Vec<EvalArtifact>,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl TassadarExecutorReferenceRunBundle {
    fn new(
        config: &TassadarExecutorTrainingConfig,
        training_manifest: &TassadarSequenceTrainingManifest,
        training_report: &TassadarExecutorTrainingReport,
        benchmark_report: &TassadarExecutorLinearBenchmarkReport,
        boundary_exactness_report: Option<&TassadarExecutorBoundaryExactnessReport>,
        divergence_histogram_report: Option<&TassadarExecutorDivergenceHistogramReport>,
        first_token_confusion_report: Option<&TassadarExecutorFirstTokenConfusionReport>,
        checkpoint_artifact: &TassadarExecutorCheckpointArtifact,
        model_artifact: &TassadarExecutorModelArtifact,
        artifacts: Vec<EvalArtifact>,
    ) -> Self {
        let benchmark_digest = stable_digest(
            b"psionic_tassadar_executor_linear_benchmark_report|",
            benchmark_report,
        );
        let mut bundle = Self {
            schema_version: TASSADAR_EXECUTOR_REFERENCE_RUN_SCHEMA_VERSION,
            run_id: config.run_id.clone(),
            trainable_surface: config.trainable_surface,
            dataset_version: config.dataset_version.clone(),
            dataset_storage_key: training_manifest.dataset_storage_key.clone(),
            dataset_digest: training_manifest.dataset_digest.clone(),
            tokenizer_digest: training_manifest.tokenizer_digest.clone(),
            vocabulary_digest: training_manifest.vocabulary_digest.clone(),
            training_manifest_digest: training_manifest.manifest_digest.clone(),
            trained_model_descriptor_digest: training_report
                .trained_model_descriptor_digest
                .clone(),
            trained_weight_digest: training_report.trained_weight_digest.clone(),
            training_report_digest: training_report.report_digest.clone(),
            linear_benchmark_report_digest: benchmark_digest,
            boundary_exactness_report_digest: boundary_exactness_report
                .map(|report| report.report_digest.clone()),
            divergence_histogram_report_digest: divergence_histogram_report
                .map(|report| report.report_digest.clone()),
            first_token_confusion_report_digest: first_token_confusion_report
                .map(|report| report.report_digest.clone()),
            checkpoint_leaderboard_report_digest: Some(stable_digest(
                b"psionic_tassadar_executor_checkpoint_leaderboard|",
                &training_report.checkpoint_leaderboard,
            )),
            exactness_curve_report_digest: None,
            failure_samples_report_digest: None,
            best_checkpoint_manifest_digest: None,
            exact_trace_samples_report_digest: None,
            promotion_gate_report_digest: None,
            neural_hull_benchmark_report_digest: None,
            checkpoint_artifact_digest: checkpoint_artifact.artifact_digest.clone(),
            model_artifact_digest: model_artifact.artifact_digest.clone(),
            artifacts,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest =
            stable_digest(b"psionic_tassadar_executor_reference_run_bundle|", &bundle);
        bundle
    }
}

/// Failure while materializing or persisting the first trained executor run.
#[derive(Debug, Error)]
pub enum TassadarExecutorRunError {
    /// Training-manifest freezing failed.
    #[error(transparent)]
    SequenceTraining(#[from] TassadarSequenceTrainingError),
    /// Training failed.
    #[error(transparent)]
    Training(#[from] TassadarExecutorTrainingError),
    /// Model checkpoint application failed.
    #[error(transparent)]
    Model(#[from] TassadarExecutorTransformerError),
    /// Run-directory creation failed.
    #[error("failed to create run directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// JSON serialization failed.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind being serialized.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Writing one persisted artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// Artifact path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// The checkpoint targeted a different base model than expected.
    #[error("checkpoint base model mismatch: expected `{expected}`, found `{actual}`")]
    UnexpectedBaseModel {
        /// Expected base model identifier.
        expected: String,
        /// Actual base model identifier.
        actual: String,
    },
    /// The reconstructed model descriptor digest did not match the checkpoint.
    #[error("checkpoint descriptor digest mismatch: expected `{expected}`, found `{actual}`")]
    DescriptorDigestMismatch {
        /// Expected digest.
        expected: String,
        /// Actual digest.
        actual: String,
    },
    /// The reconstructed model weight digest did not match the checkpoint.
    #[error("checkpoint weight digest mismatch: expected `{expected}`, found `{actual}`")]
    WeightDigestMismatch {
        /// Expected digest.
        expected: String,
        /// Actual digest.
        actual: String,
    },
}

/// Returns the canonical config for the first committed Sudoku-v0 reference run.
#[must_use]
pub fn tassadar_executor_reference_run_config() -> TassadarExecutorTrainingConfig {
    let mut config = TassadarExecutorTrainingConfig::reference();
    config.run_id = String::from(TASSADAR_EXECUTOR_REFERENCE_RUN_ID);
    config
}

/// Executes the first committed Sudoku-v0 reference run and persists its artifacts.
pub fn execute_tassadar_reference_training_run(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorRunError> {
    execute_tassadar_training_run(output_dir, &tassadar_executor_reference_run_config(), None)
}

/// Returns the canonical config for the first boundary-curriculum follow-on run.
#[must_use]
pub fn tassadar_executor_boundary_run_config() -> TassadarExecutorTrainingConfig {
    let mut config = TassadarExecutorTrainingConfig::boundary_curriculum_reference();
    config.run_id = String::from(TASSADAR_EXECUTOR_BOUNDARY_RUN_ID);
    config
}

/// Returns the canonical config for the first promotion-gate follow-on run.
#[must_use]
pub fn tassadar_executor_promotion_run_config() -> TassadarExecutorTrainingConfig {
    TassadarExecutorTrainingConfig {
        run_id: String::from(TASSADAR_EXECUTOR_PROMOTION_RUN_ID),
        workload: psionic_eval::TassadarSequenceWorkload::SudokuV0,
        dataset_version: String::from("train-v0"),
        epochs: 1,
        learning_rate: 0.05,
        max_train_target_tokens_per_example: None,
        max_eval_target_tokens_per_example: None,
        terminal_stage_learning_rate_scale: Some(0.005),
        trainable_surface:
            TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
        curriculum_stages: vec![
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_token", Some(1), 1),
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_2_tokens", Some(2), 1),
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_4_tokens", Some(4), 1),
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_8_tokens", Some(8), 2),
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_16_tokens", Some(16), 2),
            crate::TassadarExecutorCurriculumStage::new(
                "prompt_to_first_16_tokens_refine",
                Some(16),
                8,
            )
            .with_learning_rate_scale(0.1)
            .with_prefix_mode(crate::TassadarExecutorStagePrefixMode::GreedyRollout),
            crate::TassadarExecutorCurriculumStage::new("prompt_to_first_32_tokens", Some(32), 8)
                .with_learning_rate_scale(0.02)
                .with_prefix_mode(crate::TassadarExecutorStagePrefixMode::GreedyRollout),
        ],
        validate_every_epoch: true,
        select_best_checkpoint_by_boundary: true,
    }
}

/// Executes the first boundary-curriculum follow-on run and persists its artifacts.
pub fn execute_tassadar_boundary_training_run(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorRunError> {
    execute_tassadar_training_run(output_dir, &tassadar_executor_boundary_run_config(), None)
}

/// Executes the first promotion-gate follow-on run and persists its base artifacts.
pub fn execute_tassadar_promotion_training_run(
    output_dir: &Path,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorRunError> {
    execute_tassadar_training_run(output_dir, &tassadar_executor_promotion_run_config(), None)
}

/// Executes one persisted Tassadar training run with an optional benchmark split filter.
pub fn execute_tassadar_training_run(
    output_dir: &Path,
    config: &TassadarExecutorTrainingConfig,
    benchmark_split_filter: Option<TassadarSequenceSplit>,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorRunError> {
    let run_started_at = Instant::now();
    emit_tassadar_progress(format!(
        "tassadar_progress phase=run_prepare run={} output_dir={} workload={} surface={} dataset={} benchmark_split={} elapsed_ms=0",
        config.run_id,
        output_dir.display(),
        config.workload.dataset_ref(),
        config.trainable_surface.label(),
        config.dataset_version,
        benchmark_split_filter
            .map(|split| split.as_str().to_string())
            .unwrap_or_else(|| String::from("all")),
    ));
    fs::create_dir_all(output_dir).map_err(|error| TassadarExecutorRunError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;

    let training_manifest = build_tassadar_sequence_training_manifest(
        config.workload,
        config.dataset_version.as_str(),
        config.trainable_surface,
    )?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=manifest_ready run={} manifest_digest={} dataset_storage_key={} train_batches={} elapsed_ms={}",
        config.run_id,
        training_manifest.manifest_digest,
        training_manifest.dataset_storage_key,
        training_manifest.train_plan.batches.len(),
        run_started_at.elapsed().as_millis(),
    ));
    let outcome = train_tassadar_executor_transformer(config)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=training_ready run={} best_checkpoint={} first_target_bps={} first_32_bps={} exact_traces={} elapsed_ms={}",
        config.run_id,
        outcome.report.best_checkpoint_id,
        outcome.report.evaluation.first_target_exactness_bps,
        outcome.report.evaluation.first_32_token_exactness_bps,
        outcome.report.evaluation.exact_trace_case_count,
        run_started_at.elapsed().as_millis(),
    ));
    let dataset_bundle =
        build_tassadar_sequence_dataset(config.workload, config.dataset_version.as_str())
            .map_err(TassadarExecutorTrainingError::from)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=benchmark_start run={} benchmark_split={} eval_examples={} elapsed_ms={}",
        config.run_id,
        benchmark_split_filter
            .map(|split| split.as_str().to_string())
            .unwrap_or_else(|| String::from("all")),
        dataset_bundle.dataset.examples.len(),
        run_started_at.elapsed().as_millis(),
    ));
    let benchmark_report = benchmark_tassadar_executor_linear_decode(
        &outcome.model,
        &dataset_bundle.dataset,
        benchmark_split_filter,
    )
    .map_err(TassadarExecutorTrainingError::from)?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=benchmark_complete run={} decode_mode={} neural_tok_s={} cpu_tok_s={} exact_cases={} elapsed_ms={}",
        config.run_id,
        benchmark_report.decode_mode,
        benchmark_report.neural_tokens_per_second,
        benchmark_report.cpu_tokens_per_second,
        benchmark_report
            .case_reports
            .iter()
            .filter(|case| case.exact_trace_match)
            .count(),
        run_started_at.elapsed().as_millis(),
    ));
    let boundary_exactness_report =
        build_tassadar_executor_boundary_exactness_report(&outcome.report.evaluation);
    let divergence_histogram_report =
        build_tassadar_executor_divergence_histogram_report(&outcome.report.evaluation);
    let first_token_confusion_report =
        build_tassadar_executor_first_token_confusion_report(&outcome.report.evaluation);
    let checkpoint_state = TassadarExecutorCheckpointState::new(
        outcome.report.best_checkpoint_id.as_str(),
        config.run_id.as_str(),
        &training_manifest,
        &outcome.model,
        dataset_bundle
            .dataset
            .examples
            .iter()
            .map(|example| example.token_ids.len())
            .max()
            .unwrap_or(1),
    );
    let checkpoint_state_bytes =
        serialize_json("tassadar_executor_checkpoint_state", &checkpoint_state)?;
    let checkpoint_manifest = DatastreamManifest::from_bytes(
        format!("checkpoint://{}", checkpoint_state.checkpoint_id),
        DatastreamSubjectKind::Checkpoint,
        checkpoint_state_bytes.as_slice(),
        4_096,
        DatastreamEncoding::RawBinary,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new(TASSADAR_EXECUTOR_CHECKPOINT_FAMILY)
            .with_checkpoint_ref(format!("checkpoint://{}", checkpoint_state.checkpoint_id))
            .with_step(u64::from(config.epochs)),
    )
    .with_provenance_digest(checkpoint_state.state_digest.clone());
    let checkpoint_artifact =
        TassadarExecutorCheckpointArtifact::new(&checkpoint_state, &checkpoint_manifest);
    let model_artifact = TassadarExecutorModelArtifact::new(
        config.run_id.as_str(),
        &outcome.model,
        &outcome.report,
        &benchmark_report,
        &checkpoint_manifest,
    );

    checkpoint_state.materialize_model()?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=persist_start run={} checkpoint_id={} artifact_dir={} elapsed_ms={}",
        config.run_id,
        checkpoint_state.checkpoint_id,
        output_dir.display(),
        run_started_at.elapsed().as_millis(),
    ));

    let mut artifacts = Vec::new();
    artifacts.push(write_json_artifact(
        output_dir,
        TRAINING_MANIFEST_FILE,
        "tassadar_training_manifest",
        &training_manifest,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        TRAINING_REPORT_FILE,
        "tassadar_training_report",
        &outcome.report,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        LINEAR_BENCHMARK_REPORT_FILE,
        "tassadar_linear_benchmark_report",
        &benchmark_report,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE,
        "tassadar_boundary_exactness_report",
        &boundary_exactness_report,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_DIVERGENCE_HISTOGRAM_FILE,
        "tassadar_divergence_histogram_report",
        &divergence_histogram_report,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_FIRST_TOKEN_CONFUSION_FILE,
        "tassadar_first_token_confusion_report",
        &first_token_confusion_report,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_CHECKPOINT_LEADERBOARD_FILE,
        "tassadar_checkpoint_leaderboard",
        &outcome.report.checkpoint_leaderboard,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        CHECKPOINT_ARTIFACT_FILE,
        "tassadar_checkpoint_artifact",
        &checkpoint_artifact,
    )?);
    write_bytes(
        output_dir.join(CHECKPOINT_STATE_FILE),
        checkpoint_state_bytes.as_slice(),
    )?;
    artifacts.push(EvalArtifact::new(
        "tassadar_checkpoint_state",
        CHECKPOINT_STATE_FILE,
        checkpoint_state_bytes.as_slice(),
    ));
    artifacts.push(write_json_artifact(
        output_dir,
        CHECKPOINT_MANIFEST_FILE,
        "tassadar_checkpoint_manifest",
        &checkpoint_manifest,
    )?);
    artifacts.push(write_json_artifact(
        output_dir,
        MODEL_ARTIFACT_FILE,
        "tassadar_model_artifact",
        &model_artifact,
    )?);

    let bundle = TassadarExecutorReferenceRunBundle::new(
        config,
        &training_manifest,
        &outcome.report,
        &benchmark_report,
        Some(&boundary_exactness_report),
        Some(&divergence_histogram_report),
        Some(&first_token_confusion_report),
        &checkpoint_artifact,
        &model_artifact,
        artifacts,
    );
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        &bundle,
        "tassadar_reference_run_bundle",
    )?;
    emit_tassadar_progress(format!(
        "tassadar_progress phase=run_complete run={} bundle_digest={} artifacts={} elapsed_ms={}",
        config.run_id,
        bundle.bundle_digest,
        bundle.artifacts.len(),
        run_started_at.elapsed().as_millis(),
    ));
    Ok(bundle)
}

fn capture_position_embedding_rows(
    model: &TassadarExecutorTransformer,
    observed_position_count: usize,
) -> Vec<TassadarExecutorEmbeddingRowOverride> {
    let embedding_dim = model.descriptor().config.embedding_dim;
    let row_count = observed_position_count.min(model.descriptor().config.max_sequence_tokens);
    (0..row_count)
        .map(|row_index| {
            let start = row_index * embedding_dim;
            TassadarExecutorEmbeddingRowOverride {
                row_index: row_index as u32,
                values: model.weights().position_embeddings()[start..start + embedding_dim]
                    .to_vec(),
            }
        })
        .collect()
}

fn write_json_artifact<T>(
    output_dir: &Path,
    relative_path: &str,
    artifact_kind: &str,
    value: &T,
) -> Result<EvalArtifact, TassadarExecutorRunError>
where
    T: Serialize,
{
    let bytes = serialize_json(artifact_kind, value)?;
    write_bytes(output_dir.join(relative_path), bytes.as_slice())?;
    Ok(EvalArtifact::new(
        artifact_kind,
        relative_path,
        bytes.as_slice(),
    ))
}

fn serialize_json<T>(artifact_kind: &str, value: &T) -> Result<Vec<u8>, TassadarExecutorRunError>
where
    T: Serialize,
{
    serde_json::to_vec_pretty(value).map_err(|error| TassadarExecutorRunError::Serialize {
        artifact_kind: artifact_kind.to_string(),
        error,
    })
}

fn write_json<T>(
    path: PathBuf,
    value: &T,
    artifact_kind: &str,
) -> Result<(), TassadarExecutorRunError>
where
    T: Serialize,
{
    let bytes = serialize_json(artifact_kind, value)?;
    write_bytes(path, bytes.as_slice())
}

fn write_bytes(path: PathBuf, bytes: &[u8]) -> Result<(), TassadarExecutorRunError> {
    fs::write(&path, bytes).map_err(|error| TassadarExecutorRunError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar executor run value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        CHECKPOINT_STATE_FILE, MODEL_ARTIFACT_FILE, RUN_BUNDLE_FILE,
        TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE,
        TASSADAR_EXECUTOR_CHECKPOINT_LEADERBOARD_FILE, TASSADAR_EXECUTOR_DIVERGENCE_HISTOGRAM_FILE,
        TASSADAR_EXECUTOR_FIRST_TOKEN_CONFUSION_FILE, TRAINING_REPORT_FILE,
        TassadarExecutorCheckpointState, execute_tassadar_boundary_training_run,
        execute_tassadar_training_run, tassadar_executor_boundary_run_config,
        tassadar_executor_reference_run_config,
    };

    #[test]
    fn persisted_reference_run_writes_reconstructable_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bundle = execute_tassadar_training_run(
            temp.path(),
            &tassadar_executor_reference_run_config(),
            Some(psionic_data::TassadarSequenceSplit::Validation),
        )?;

        assert!(temp.path().join(TRAINING_REPORT_FILE).exists());
        assert!(temp.path().join(CHECKPOINT_STATE_FILE).exists());
        assert!(temp.path().join(MODEL_ARTIFACT_FILE).exists());
        assert!(temp.path().join(RUN_BUNDLE_FILE).exists());
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_BOUNDARY_EXACTNESS_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_DIVERGENCE_HISTOGRAM_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_FIRST_TOKEN_CONFUSION_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_EXECUTOR_CHECKPOINT_LEADERBOARD_FILE)
                .exists()
        );
        assert!(!bundle.bundle_digest.is_empty());

        let checkpoint: TassadarExecutorCheckpointState =
            serde_json::from_slice(&fs::read(temp.path().join(CHECKPOINT_STATE_FILE))?)?;
        let restored = checkpoint.materialize_model()?;
        assert_eq!(
            restored.descriptor().stable_digest(),
            checkpoint.trained_model_descriptor_digest
        );
        assert_eq!(
            restored.descriptor().weights.digest,
            checkpoint.trained_weight_digest
        );
        Ok(())
    }

    #[test]
    fn persisted_boundary_run_uses_boundary_curriculum_config()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bundle = execute_tassadar_boundary_training_run(temp.path())?;

        assert_eq!(
            bundle.run_id,
            tassadar_executor_boundary_run_config().run_id
        );
        assert_eq!(bundle.dataset_version, "train-v0");
        assert!(bundle.boundary_exactness_report_digest.is_some());
        assert!(bundle.divergence_histogram_report_digest.is_some());
        assert!(bundle.first_token_confusion_report_digest.is_some());
        assert!(bundle.checkpoint_leaderboard_report_digest.is_some());
        Ok(())
    }
}
