use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::TassadarSequenceSplit;
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind,
};
use psionic_eval::{
    EvalArtifact, TassadarExecutorLinearBenchmarkReport, benchmark_tassadar_executor_linear_decode,
    build_tassadar_sudoku_v0_sequence_dataset,
};
use psionic_models::{
    TassadarExecutorTransformer, TassadarExecutorTransformerDescriptor,
    TassadarExecutorTransformerError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarExecutorTrainingConfig, TassadarExecutorTrainingError, TassadarExecutorTrainingReport,
    TassadarSequenceTrainingError, TassadarSequenceTrainingManifest,
    build_tassadar_sudoku_v0_sequence_training_manifest, train_tassadar_executor_transformer,
};

/// Stable schema version for persisted Tassadar reference-run bundles.
pub const TASSADAR_EXECUTOR_REFERENCE_RUN_SCHEMA_VERSION: u16 = 1;
/// Stable run identifier for the first committed Sudoku-v0 reference run.
pub const TASSADAR_EXECUTOR_REFERENCE_RUN_ID: &str =
    "tassadar-executor-transformer-sudoku-v0-reference-run-v0";
/// Stable checkpoint family for persisted trained executor checkpoints.
pub const TASSADAR_EXECUTOR_CHECKPOINT_FAMILY: &str = "train.tassadar.executor_transformer";
/// Canonical repo path used for the first committed reference run.
pub const TASSADAR_EXECUTOR_REFERENCE_RUN_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0";

const TRAINING_MANIFEST_FILE: &str = "training_manifest.json";
const TRAINING_REPORT_FILE: &str = "training_report.json";
const LINEAR_BENCHMARK_REPORT_FILE: &str = "linear_benchmark_report.json";
const CHECKPOINT_ARTIFACT_FILE: &str = "checkpoint_artifact.json";
const CHECKPOINT_STATE_FILE: &str = "checkpoint_state.json";
const CHECKPOINT_MANIFEST_FILE: &str = "checkpoint_manifest.json";
const MODEL_ARTIFACT_FILE: &str = "model_artifact.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";

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
    /// Digest over the trained output projection only.
    pub output_projection_digest: String,
    /// Digest over the trained output bias only.
    pub output_bias_digest: String,
    /// Trained output projection.
    pub output_projection: Vec<f32>,
    /// Trained output bias.
    pub output_bias: Vec<f32>,
    /// Stable digest over the full checkpoint payload.
    pub state_digest: String,
}

impl TassadarExecutorCheckpointState {
    fn new(
        run_id: &str,
        training_manifest: &TassadarSequenceTrainingManifest,
        model: &TassadarExecutorTransformer,
    ) -> Self {
        let checkpoint_id = format!("{run_id}.checkpoint.epoch_0001");
        let output_projection = model.weights().output_projection().to_vec();
        let output_bias = model.weights().output_bias().to_vec();
        let output_projection_digest = stable_digest(
            b"psionic_tassadar_executor_output_projection|",
            &output_projection,
        );
        let output_bias_digest =
            stable_digest(b"psionic_tassadar_executor_output_bias|", &output_bias);
        let mut checkpoint = Self {
            checkpoint_id,
            run_id: run_id.to_string(),
            base_model_id: model.descriptor().model.model_id.clone(),
            trained_model_descriptor_digest: model.descriptor().stable_digest(),
            trained_weight_digest: model.descriptor().weights.digest.clone(),
            training_manifest_digest: training_manifest.manifest_digest.clone(),
            output_projection_digest,
            output_bias_digest,
            output_projection,
            output_bias,
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
        if self.base_model_id != TassadarExecutorTransformer::MODEL_ID {
            return Err(TassadarExecutorRunError::UnexpectedBaseModel {
                expected: String::from(TassadarExecutorTransformer::MODEL_ID),
                actual: self.base_model_id.clone(),
            });
        }

        let mut model = TassadarExecutorTransformer::sudoku_v0();
        model.apply_trained_output_head(
            self.output_projection.as_slice(),
            self.output_bias.as_slice(),
        )?;
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

/// Executes one persisted Tassadar training run with an optional benchmark split filter.
pub fn execute_tassadar_training_run(
    output_dir: &Path,
    config: &TassadarExecutorTrainingConfig,
    benchmark_split_filter: Option<TassadarSequenceSplit>,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorRunError> {
    fs::create_dir_all(output_dir).map_err(|error| TassadarExecutorRunError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;

    let training_manifest =
        build_tassadar_sudoku_v0_sequence_training_manifest(config.dataset_version.as_str())?;
    let outcome = train_tassadar_executor_transformer(config)?;
    let dataset_bundle = build_tassadar_sudoku_v0_sequence_dataset(config.dataset_version.as_str())
        .map_err(TassadarExecutorTrainingError::from)?;
    let benchmark_report = benchmark_tassadar_executor_linear_decode(
        &outcome.model,
        &dataset_bundle.dataset,
        benchmark_split_filter,
    )
    .map_err(TassadarExecutorTrainingError::from)?;
    let checkpoint_state = TassadarExecutorCheckpointState::new(
        config.run_id.as_str(),
        &training_manifest,
        &outcome.model,
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
        &checkpoint_artifact,
        &model_artifact,
        artifacts,
    );
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        &bundle,
        "tassadar_reference_run_bundle",
    )?;
    Ok(bundle)
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
        CHECKPOINT_STATE_FILE, MODEL_ARTIFACT_FILE, RUN_BUNDLE_FILE, TRAINING_REPORT_FILE,
        TassadarExecutorCheckpointState, execute_tassadar_training_run,
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
}
