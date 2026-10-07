use std::time::Instant;

use psionic_eval::{
    build_parameter_golf_local_reference_benchmark_package, evaluate_parameter_golf_validation,
    BenchmarkPackage, ParameterGolfArtifactReceiptEntry, ParameterGolfArtifactSizeReceipt,
    ParameterGolfBundleRoot, ParameterGolfChallengeBenchmarkReceipt,
    ParameterGolfChallengeScoreReport, ParameterGolfMemoryReceipt,
    ParameterGolfValidationEvalError, ParameterGolfWallclockReceipt,
    PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF, PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    restore_parameter_golf_model_from_int8_zlib, restore_parameter_golf_model_from_safetensors,
    train_parameter_golf_local_reference, ParameterGolfCheckpointManifest,
    ParameterGolfLocalReferenceFixture, ParameterGolfReferenceOptimizerState,
    ParameterGolfReferenceTrainingConfig, ParameterGolfReferenceTrainingError,
    ParameterGolfReferenceTrainingOutcome, ParameterGolfTrainingArtifact,
};

/// Stable benchmark-package version for the bounded local-reference review lane.
pub const PARAMETER_GOLF_LOCAL_REFERENCE_BENCHMARK_VERSION: &str = "2026.03.18.local_reference.v1";
/// Frozen training wallclock used by the canonical exported submission bundle.
pub const PARAMETER_GOLF_LOCAL_REFERENCE_CANONICAL_TRAINING_OBSERVED_MS: u64 = 25_779;
/// Frozen raw-restore eval wallclock used by the canonical exported submission bundle.
pub const PARAMETER_GOLF_LOCAL_REFERENCE_CANONICAL_RAW_RESTORE_EVAL_OBSERVED_MS: u64 = 527;
/// Frozen int8+zlib restore eval wallclock used by the canonical exported submission bundle.
pub const PARAMETER_GOLF_LOCAL_REFERENCE_CANONICAL_INT8_ZLIB_RESTORE_EVAL_OBSERVED_MS: u64 = 544;

/// Aggregate benchmark bundle for one bounded local-reference run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfLocalReferenceBenchmarkBundle {
    /// Full bounded training outcome.
    pub training_outcome: ParameterGolfReferenceTrainingOutcome,
    /// Benchmark package used for review.
    pub benchmark_package: BenchmarkPackage,
    /// Score report derived from the run.
    pub challenge_score_report: ParameterGolfChallengeScoreReport,
    /// Benchmark receipt derived from the run.
    pub benchmark_receipt: ParameterGolfChallengeBenchmarkReceipt,
    /// Serialized benchmark-package artifact.
    pub benchmark_package_artifact: ParameterGolfTrainingArtifact,
    /// Serialized score-report artifact.
    pub challenge_score_report_artifact: ParameterGolfTrainingArtifact,
    /// Serialized benchmark-receipt artifact.
    pub benchmark_receipt_artifact: ParameterGolfTrainingArtifact,
    /// Top-level run bundle.
    pub run_bundle: ParameterGolfChallengeRunBundle,
    /// Serialized run-bundle artifact.
    pub run_bundle_artifact: ParameterGolfTrainingArtifact,
}

/// Top-level run bundle for Parameter Golf review artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfChallengeRunBundle {
    /// Stable run identifier.
    pub run_id: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Stable benchmark-package artifact reference.
    pub benchmark_package_artifact_ref: String,
    /// Stable benchmark-package artifact digest.
    pub benchmark_package_artifact_digest: String,
    /// Stable score-report artifact reference.
    pub challenge_score_report_artifact_ref: String,
    /// Stable score-report artifact digest.
    pub challenge_score_report_artifact_digest: String,
    /// Stable benchmark-receipt artifact reference.
    pub benchmark_receipt_artifact_ref: String,
    /// Stable benchmark-receipt artifact digest.
    pub benchmark_receipt_artifact_digest: String,
    /// Train-side review bundle root.
    pub train_bundle_root: ParameterGolfBundleRoot,
    /// Eval-side review bundle root.
    pub eval_bundle_root: ParameterGolfBundleRoot,
    /// Explicit claim boundary for the bundle.
    pub claim_boundary: String,
    /// Stable digest over the bundle payload.
    pub bundle_digest: String,
}

impl ParameterGolfChallengeRunBundle {
    fn new(
        run_id: impl Into<String>,
        benchmark_package_artifact: &ParameterGolfTrainingArtifact,
        challenge_score_report_artifact: &ParameterGolfTrainingArtifact,
        benchmark_receipt_artifact: &ParameterGolfTrainingArtifact,
        train_bundle_root: ParameterGolfBundleRoot,
        eval_bundle_root: ParameterGolfBundleRoot,
    ) -> Self {
        let mut bundle = Self {
            run_id: run_id.into(),
            benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
            benchmark_package_artifact_ref: benchmark_package_artifact.artifact_ref.clone(),
            benchmark_package_artifact_digest: benchmark_package_artifact.artifact_digest.clone(),
            challenge_score_report_artifact_ref: challenge_score_report_artifact
                .artifact_ref
                .clone(),
            challenge_score_report_artifact_digest: challenge_score_report_artifact
                .artifact_digest
                .clone(),
            benchmark_receipt_artifact_ref: benchmark_receipt_artifact.artifact_ref.clone(),
            benchmark_receipt_artifact_digest: benchmark_receipt_artifact.artifact_digest.clone(),
            train_bundle_root,
            eval_bundle_root,
            claim_boundary: String::from(PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(b"psionic_parameter_golf_run_bundle|", &bundle);
        bundle
    }
}

/// Benchmark-bundle construction failure.
#[derive(Debug, Error)]
pub enum ParameterGolfBenchmarkBundleError {
    #[error("{context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error(transparent)]
    Eval(#[from] ParameterGolfValidationEvalError),
    #[error(transparent)]
    Training(#[from] ParameterGolfReferenceTrainingError),
}

/// Runs the bounded local-reference trainer and emits benchmark-package and receipt artifacts.
pub fn benchmark_parameter_golf_local_reference(
    fixture: &ParameterGolfLocalReferenceFixture,
    config: &ParameterGolfReferenceTrainingConfig,
) -> Result<ParameterGolfLocalReferenceBenchmarkBundle, ParameterGolfBenchmarkBundleError> {
    let training_started = Instant::now();
    let training_outcome = train_parameter_golf_local_reference(fixture, config)?;
    let training_observed_ms = training_started.elapsed().as_millis() as u64;

    let raw_eval_started = Instant::now();
    let raw_roundtrip_model = restore_parameter_golf_model_from_safetensors(
        &training_outcome.initial_model,
        training_outcome.raw_model_artifact.bytes.as_slice(),
    )?;
    let raw_roundtrip_eval = evaluate_parameter_golf_validation(
        &raw_roundtrip_model,
        fixture.validation_tokens.as_slice(),
        config.geometry.train_sequence_length,
        config.geometry.local_validation_batch_tokens(),
        &fixture.byte_luts()?,
    )?;
    let raw_restore_eval_observed_ms = raw_eval_started.elapsed().as_millis() as u64;

    let int8_eval_started = Instant::now();
    let int8_roundtrip_model = restore_parameter_golf_model_from_int8_zlib(
        &training_outcome.initial_model,
        training_outcome.int8_zlib_model_artifact.bytes.as_slice(),
    )?;
    let int8_roundtrip_eval = evaluate_parameter_golf_validation(
        &int8_roundtrip_model,
        fixture.validation_tokens.as_slice(),
        config.geometry.train_sequence_length,
        config.geometry.local_validation_batch_tokens(),
        &fixture.byte_luts()?,
    )?;
    let int8_zlib_restore_eval_observed_ms = int8_eval_started.elapsed().as_millis() as u64;

    let benchmark_package = build_parameter_golf_local_reference_benchmark_package(
        PARAMETER_GOLF_LOCAL_REFERENCE_BENCHMARK_VERSION,
        fixture.training_digest().as_str(),
        fixture.validation_digest().as_str(),
        config.geometry.train_sequence_length,
    );
    let challenge_score_report = ParameterGolfChallengeScoreReport::new(
        training_outcome.initial_validation_eval.clone(),
        training_outcome.final_validation_eval.clone(),
        raw_roundtrip_eval.clone(),
        int8_roundtrip_eval.clone(),
    );

    let benchmark_package_artifact = json_artifact(
        "parameter_golf_benchmark_package",
        format!(
            "{}/benchmark/parameter_golf_benchmark_package.json",
            config.run_id
        ),
        &benchmark_package,
    )?;
    let challenge_score_report_artifact = json_artifact(
        "parameter_golf_challenge_score_report",
        format!(
            "{}/benchmark/parameter_golf_challenge_score_report.json",
            config.run_id
        ),
        &challenge_score_report,
    )?;
    let initial_validation_artifact = json_artifact(
        "parameter_golf_validation_eval_report",
        format!("{}/eval/initial_validation_eval.json", config.run_id),
        &training_outcome.initial_validation_eval,
    )?;
    let final_validation_artifact = json_artifact(
        "parameter_golf_validation_eval_report",
        format!("{}/eval/final_validation_eval.json", config.run_id),
        &training_outcome.final_validation_eval,
    )?;
    let raw_roundtrip_validation_artifact = json_artifact(
        "parameter_golf_validation_eval_report",
        format!("{}/eval/raw_roundtrip_validation_eval.json", config.run_id),
        &raw_roundtrip_eval,
    )?;
    let int8_roundtrip_validation_artifact = json_artifact(
        "parameter_golf_validation_eval_report",
        format!(
            "{}/eval/int8_zlib_roundtrip_validation_eval.json",
            config.run_id
        ),
        &int8_roundtrip_eval,
    )?;

    let train_bundle_root = ParameterGolfBundleRoot {
        root_ref: format!("bundle://parameter_golf/{}/train", config.run_id),
        artifacts: vec![
            artifact_entry_from_training_artifact(
                &training_outcome.initial_checkpoint.manifest_artifact,
            ),
            artifact_entry_from_training_artifact(
                &training_outcome.initial_checkpoint.weights_artifact,
            ),
            artifact_entry_from_training_artifact(
                &training_outcome.final_checkpoint.manifest_artifact,
            ),
            artifact_entry_from_training_artifact(&training_outcome.raw_model_artifact),
            artifact_entry_from_training_artifact(&training_outcome.int8_zlib_model_artifact),
        ],
    };
    let eval_bundle_root = ParameterGolfBundleRoot {
        root_ref: format!("bundle://parameter_golf/{}/eval", config.run_id),
        artifacts: vec![
            artifact_entry_from_training_artifact(&benchmark_package_artifact),
            artifact_entry_from_training_artifact(&challenge_score_report_artifact),
            artifact_entry_from_training_artifact(&initial_validation_artifact),
            artifact_entry_from_training_artifact(&final_validation_artifact),
            artifact_entry_from_training_artifact(&raw_roundtrip_validation_artifact),
            artifact_entry_from_training_artifact(&int8_roundtrip_validation_artifact),
        ],
    };

    let tracked_artifacts = vec![
        artifact_entry_from_training_artifact(
            &training_outcome.initial_checkpoint.manifest_artifact,
        ),
        artifact_entry_from_training_artifact(
            &training_outcome.initial_checkpoint.weights_artifact,
        ),
        artifact_entry_from_training_artifact(&training_outcome.final_checkpoint.manifest_artifact),
        artifact_entry_from_training_artifact(&training_outcome.raw_model_artifact),
        artifact_entry_from_training_artifact(&training_outcome.int8_zlib_model_artifact),
        artifact_entry_from_training_artifact(&benchmark_package_artifact),
        artifact_entry_from_training_artifact(&challenge_score_report_artifact),
        artifact_entry_from_training_artifact(&initial_validation_artifact),
        artifact_entry_from_training_artifact(&final_validation_artifact),
        artifact_entry_from_training_artifact(&raw_roundtrip_validation_artifact),
        artifact_entry_from_training_artifact(&int8_roundtrip_validation_artifact),
    ];
    let artifact_size_receipt = ParameterGolfArtifactSizeReceipt {
        submission_artifact_ref: training_outcome
            .int8_zlib_model_artifact
            .artifact_ref
            .clone(),
        submission_artifact_digest: training_outcome
            .int8_zlib_model_artifact
            .artifact_digest
            .clone(),
        submission_artifact_size_bytes: training_outcome.int8_zlib_model_artifact.bytes.len()
            as u64,
        total_artifact_bytes: tracked_artifacts
            .iter()
            .map(|artifact| artifact.size_bytes)
            .sum(),
        artifacts: tracked_artifacts,
    };
    let wallclock_cap_ms = config
        .hyperparameters
        .max_wallclock_seconds
        .map(|seconds| (seconds * 1000.0).round() as u64);
    let wallclock_receipt = ParameterGolfWallclockReceipt {
        measurement_posture: String::from("observed_process_and_logical_step_budget"),
        training_observed_ms,
        training_logical_ms: config.max_steps.saturating_mul(config.step_duration_ms),
        raw_restore_eval_observed_ms,
        int8_zlib_restore_eval_observed_ms,
        within_wallclock_cap: wallclock_cap_ms.map(|cap| training_observed_ms <= cap),
        wallclock_cap_ms,
    };
    let final_manifest_bytes = training_outcome
        .final_checkpoint
        .manifest_artifact
        .bytes
        .len() as u64;
    let memory_receipt = ParameterGolfMemoryReceipt {
        measurement_posture: String::from("estimated_tensor_state_bytes"),
        model_parameter_bytes: model_parameter_bytes(&training_outcome.trained_model),
        optimizer_state_bytes: optimizer_state_bytes(&training_outcome.final_checkpoint.manifest),
        raw_model_artifact_bytes: training_outcome.raw_model_artifact.bytes.len() as u64,
        int8_zlib_artifact_bytes: training_outcome.int8_zlib_model_artifact.bytes.len() as u64,
        checkpoint_manifest_bytes: final_manifest_bytes,
        estimated_live_bytes_upper_bound: model_parameter_bytes(&training_outcome.trained_model)
            + optimizer_state_bytes(&training_outcome.final_checkpoint.manifest)
            + training_outcome.raw_model_artifact.bytes.len() as u64
            + training_outcome.int8_zlib_model_artifact.bytes.len() as u64
            + final_manifest_bytes,
    };

    let benchmark_receipt = ParameterGolfChallengeBenchmarkReceipt {
        benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
        benchmark_package: benchmark_package.key.clone(),
        run_id: config.run_id.clone(),
        trained_model_descriptor_digest: training_outcome
            .trained_model
            .descriptor()
            .stable_digest(),
        final_checkpoint_manifest_digest: training_outcome
            .final_checkpoint
            .manifest
            .stable_digest(),
        submission_artifact_ref: training_outcome
            .int8_zlib_model_artifact
            .artifact_ref
            .clone(),
        submission_artifact_digest: training_outcome
            .int8_zlib_model_artifact
            .artifact_digest
            .clone(),
        score_report: challenge_score_report.clone(),
        wallclock_receipt,
        memory_receipt,
        artifact_size_receipt,
        train_bundle_root: train_bundle_root.clone(),
        eval_bundle_root: eval_bundle_root.clone(),
        claim_boundary: String::from(PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY),
    };
    let benchmark_receipt_artifact = json_artifact(
        "parameter_golf_challenge_benchmark_receipt",
        format!(
            "{}/benchmark/parameter_golf_challenge_benchmark_receipt.json",
            config.run_id
        ),
        &benchmark_receipt,
    )?;
    let run_bundle = ParameterGolfChallengeRunBundle::new(
        config.run_id.as_str(),
        &benchmark_package_artifact,
        &challenge_score_report_artifact,
        &benchmark_receipt_artifact,
        train_bundle_root,
        eval_bundle_root,
    );
    let run_bundle_artifact = json_artifact(
        "parameter_golf_run_bundle",
        format!("{}/benchmark/run_bundle.json", config.run_id),
        &run_bundle,
    )?;
    Ok(ParameterGolfLocalReferenceBenchmarkBundle {
        training_outcome,
        benchmark_package,
        challenge_score_report,
        benchmark_receipt,
        benchmark_package_artifact,
        challenge_score_report_artifact,
        benchmark_receipt_artifact,
        run_bundle,
        run_bundle_artifact,
    })
}

/// Rewrites one bounded local-reference benchmark bundle onto the frozen canonical
/// wallclock facts used by the exported submission lane.
///
/// The exported-folder evidence lane is reviewed for replay and accounting
/// stability rather than local host speed, so the canonical submission path pins
/// these bounded wallclock fields to one committed reference measurement.
pub fn canonicalize_parameter_golf_local_reference_benchmark_bundle(
    bundle: &ParameterGolfLocalReferenceBenchmarkBundle,
) -> Result<ParameterGolfLocalReferenceBenchmarkBundle, ParameterGolfBenchmarkBundleError> {
    let mut canonical = bundle.clone();
    canonical
        .benchmark_receipt
        .wallclock_receipt
        .training_observed_ms = PARAMETER_GOLF_LOCAL_REFERENCE_CANONICAL_TRAINING_OBSERVED_MS;
    canonical
        .benchmark_receipt
        .wallclock_receipt
        .raw_restore_eval_observed_ms =
        PARAMETER_GOLF_LOCAL_REFERENCE_CANONICAL_RAW_RESTORE_EVAL_OBSERVED_MS;
    canonical
        .benchmark_receipt
        .wallclock_receipt
        .int8_zlib_restore_eval_observed_ms =
        PARAMETER_GOLF_LOCAL_REFERENCE_CANONICAL_INT8_ZLIB_RESTORE_EVAL_OBSERVED_MS;
    canonical.benchmark_receipt_artifact = json_artifact(
        "parameter_golf_challenge_benchmark_receipt",
        format!(
            "{}/benchmark/parameter_golf_challenge_benchmark_receipt.json",
            canonical.run_bundle.run_id
        ),
        &canonical.benchmark_receipt,
    )?;
    canonical.run_bundle = ParameterGolfChallengeRunBundle::new(
        canonical.run_bundle.run_id.clone(),
        &canonical.benchmark_package_artifact,
        &canonical.challenge_score_report_artifact,
        &canonical.benchmark_receipt_artifact,
        canonical.run_bundle.train_bundle_root.clone(),
        canonical.run_bundle.eval_bundle_root.clone(),
    );
    canonical.run_bundle_artifact = json_artifact(
        "parameter_golf_run_bundle",
        format!("{}/benchmark/run_bundle.json", canonical.run_bundle.run_id),
        &canonical.run_bundle,
    )?;
    Ok(canonical)
}

fn json_artifact<T: Serialize>(
    artifact_kind: &'static str,
    artifact_ref: String,
    value: &T,
) -> Result<ParameterGolfTrainingArtifact, ParameterGolfBenchmarkBundleError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        ParameterGolfBenchmarkBundleError::Serialization {
            context: "parameter golf benchmark artifact serialization",
            message: error.to_string(),
        }
    })?;
    Ok(ParameterGolfTrainingArtifact::new(
        artifact_kind,
        artifact_ref,
        bytes,
    ))
}

fn artifact_entry_from_training_artifact(
    artifact: &ParameterGolfTrainingArtifact,
) -> ParameterGolfArtifactReceiptEntry {
    ParameterGolfArtifactReceiptEntry {
        artifact_kind: artifact.artifact_kind.clone(),
        artifact_ref: artifact.artifact_ref.clone(),
        artifact_digest: artifact.artifact_digest.clone(),
        size_bytes: artifact.bytes.len() as u64,
    }
}

fn model_parameter_bytes(model: &psionic_models::ParameterGolfReferenceModel) -> u64 {
    model
        .weights()
        .parameter_vectors(&model.descriptor().config)
        .iter()
        .map(|parameter| parameter.values.len() as u64 * 4)
        .sum()
}

fn optimizer_state_bytes(manifest: &ParameterGolfCheckpointManifest) -> u64 {
    manifest
        .trainable_tensors
        .iter()
        .map(|tensor| match &tensor.optimizer_state {
            ParameterGolfReferenceOptimizerState::Adam { state } => match state {
                crate::TrainingOptimizerState::Sgd { momentum_buffer }
                | crate::TrainingOptimizerState::Lars { momentum_buffer } => momentum_buffer
                    .as_ref()
                    .map_or(0, |buffer| buffer.len() as u64 * 4),
                crate::TrainingOptimizerState::Adam {
                    first_moment,
                    second_moment,
                }
                | crate::TrainingOptimizerState::AdamW {
                    first_moment,
                    second_moment,
                }
                | crate::TrainingOptimizerState::Lamb {
                    first_moment,
                    second_moment,
                } => (first_moment.len() as u64 + second_moment.len() as u64) * 4,
            },
            ParameterGolfReferenceOptimizerState::Muon { state } => {
                state.momentum_buffer.len() as u64 * 4
            }
        })
        .sum()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        benchmark_parameter_golf_local_reference, PARAMETER_GOLF_LOCAL_REFERENCE_BENCHMARK_VERSION,
    };
    use crate::{ParameterGolfLocalReferenceFixture, ParameterGolfReferenceTrainingConfig};

    #[test]
    fn parameter_golf_local_reference_benchmark_bundle_is_machine_readable(
    ) -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let bundle = benchmark_parameter_golf_local_reference(&fixture, &config)?;

        assert_eq!(
            bundle.benchmark_package.key.benchmark_ref,
            psionic_eval::PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF
        );
        assert_eq!(
            bundle.benchmark_package.key.version,
            PARAMETER_GOLF_LOCAL_REFERENCE_BENCHMARK_VERSION
        );
        assert_eq!(
            bundle.challenge_score_report.trained_validation,
            bundle.training_outcome.final_validation_eval
        );
        assert!(
            bundle
                .benchmark_receipt
                .wallclock_receipt
                .training_observed_ms
                > 0
        );
        assert!(
            bundle
                .benchmark_receipt
                .artifact_size_receipt
                .submission_artifact_size_bytes
                > 0
        );
        assert!(
            bundle
                .benchmark_receipt
                .memory_receipt
                .estimated_live_bytes_upper_bound
                > bundle
                    .benchmark_receipt
                    .memory_receipt
                    .model_parameter_bytes
        );
        assert!(!bundle.run_bundle.bundle_digest.is_empty());
        assert!(bundle
            .run_bundle
            .train_bundle_root
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact_ref.ends_with("final_model.int8.ptz")));
        assert!(bundle
            .run_bundle
            .eval_bundle_root
            .artifacts
            .iter()
            .any(|artifact| artifact
                .artifact_ref
                .ends_with("parameter_golf_challenge_score_report.json")));
        let roundtrip: super::ParameterGolfChallengeRunBundle =
            serde_json::from_slice(&serde_json::to_vec_pretty(&bundle.run_bundle)?)?;
        assert_eq!(roundtrip, bundle.run_bundle);
        Ok(())
    }
}
