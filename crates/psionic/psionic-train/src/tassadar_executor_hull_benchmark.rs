use std::{fs, path::Path};

use psionic_eval::{
    benchmark_tassadar_executor_neural_hull_decode, build_tassadar_sudoku_v0_sequence_dataset,
    EvalArtifact, TassadarExecutorHullBenchmarkError, TassadarSequenceEvalError,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarExecutorCheckpointState, TassadarExecutorReferenceRunBundle, TassadarExecutorRunError,
    TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_REPORT_FILE,
};

const CHECKPOINT_STATE_FILE: &str = "checkpoint_state.json";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";

/// Failure while materializing or persisting the neural hull benchmark.
#[derive(Debug, Error)]
pub enum TassadarExecutorHullBenchmarkPersistError {
    /// Dataset generation failed.
    #[error(transparent)]
    SequenceEval(#[from] TassadarSequenceEvalError),
    /// Loading one run artifact failed.
    #[error(transparent)]
    Run(#[from] TassadarExecutorRunError),
    /// Neural hull benchmarking failed.
    #[error(transparent)]
    Benchmark(#[from] TassadarExecutorHullBenchmarkError),
    /// Reading one persisted artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// Artifact path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Deserializing one persisted artifact failed.
    #[error("failed to deserialize `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Artifact path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Serializing one persisted artifact failed.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
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
}

/// Materializes the persisted trained model and writes the neural hull benchmark artifact.
pub fn materialize_tassadar_reference_run_hull_benchmark(
    output_dir: &Path,
    max_target_tokens_per_case: Option<u32>,
) -> Result<TassadarExecutorReferenceRunBundle, TassadarExecutorHullBenchmarkPersistError> {
    let mut run_bundle: TassadarExecutorReferenceRunBundle = read_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
    )?;
    let checkpoint_state: TassadarExecutorCheckpointState = read_json(
        output_dir.join(CHECKPOINT_STATE_FILE),
        "tassadar_executor_checkpoint_state",
    )?;
    let model = checkpoint_state.materialize_model()?;
    let dataset_bundle =
        build_tassadar_sudoku_v0_sequence_dataset(run_bundle.dataset_version.as_str())?;
    let report = benchmark_tassadar_executor_neural_hull_decode(
        &model,
        &dataset_bundle.dataset,
        None,
        max_target_tokens_per_case,
    )?;
    let artifact = write_json_artifact(
        output_dir,
        TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_REPORT_FILE,
        "tassadar_neural_hull_benchmark_report",
        &report,
    )?;

    run_bundle.artifacts.retain(|existing| {
        existing.artifact_ref != TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_REPORT_FILE
    });
    run_bundle.artifacts.push(artifact);
    run_bundle.neural_hull_benchmark_report_digest = Some(stable_digest(
        b"psionic_tassadar_executor_neural_hull_benchmark_report|",
        &report,
    ));
    run_bundle.bundle_digest.clear();
    run_bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_executor_reference_run_bundle|",
        &run_bundle,
    );
    write_json(
        output_dir.join(RUN_BUNDLE_FILE),
        "tassadar_reference_run_bundle",
        &run_bundle,
    )?;
    Ok(run_bundle)
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarExecutorHullBenchmarkPersistError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarExecutorHullBenchmarkPersistError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExecutorHullBenchmarkPersistError::Deserialize {
            artifact_kind: artifact_kind.to_string(),
            path: path.display().to_string(),
            error,
        }
    })
}

fn write_json_artifact<T>(
    output_dir: &Path,
    relative_path: &str,
    artifact_kind: &str,
    value: &T,
) -> Result<EvalArtifact, TassadarExecutorHullBenchmarkPersistError>
where
    T: Serialize,
{
    let path = output_dir.join(relative_path);
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorHullBenchmarkPersistError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(&path, &bytes).map_err(|error| TassadarExecutorHullBenchmarkPersistError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(EvalArtifact::new(
        artifact_kind,
        relative_path,
        bytes.as_slice(),
    ))
}

fn write_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
    value: &T,
) -> Result<(), TassadarExecutorHullBenchmarkPersistError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        TassadarExecutorHullBenchmarkPersistError::Serialize {
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })?;
    fs::write(path, &bytes).map_err(|error| TassadarExecutorHullBenchmarkPersistError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar executor hull benchmark value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        execute_tassadar_training_run, materialize_tassadar_reference_run_hull_benchmark,
        tassadar_executor_reference_run_config,
    };

    #[test]
    fn persisted_reference_run_accepts_neural_hull_benchmark_artifact(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        execute_tassadar_training_run(
            temp.path(),
            &tassadar_executor_reference_run_config(),
            None,
        )?;
        let bundle = materialize_tassadar_reference_run_hull_benchmark(temp.path(), Some(64))?;

        assert!(bundle.neural_hull_benchmark_report_digest.is_some());
        assert!(bundle.artifacts.iter().any(|artifact| {
            artifact.artifact_ref == super::TASSADAR_EXECUTOR_NEURAL_HULL_BENCHMARK_REPORT_FILE
        }));
        Ok(())
    }
}
