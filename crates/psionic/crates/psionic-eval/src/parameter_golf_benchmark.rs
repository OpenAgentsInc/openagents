use psionic_data::DatasetKey;
use psionic_environments::EnvironmentPackageKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy, EvalArtifact, ParameterGolfValidationEvalReport,
};

/// Canonical benchmark reference for the Parameter Golf review lane.
pub const PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF: &str =
    "benchmark://openagents/psionic/parameter_golf/challenge_review";

/// Stable metric identifier used for leaderboard-facing Parameter Golf review.
pub const PARAMETER_GOLF_SUBMISSION_METRIC_ID: &str = "parameter_golf.validation_bits_per_byte";

/// Claim boundary carried by the bounded local-reference benchmark lane.
pub const PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY: &str =
    "bounded local-reference benchmark receipt lane only; not measured 8xH100 challenge closure";

/// One score report for a Parameter Golf training run and its roundtrips.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfChallengeScoreReport {
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Initial baseline validation report.
    pub initial_validation: ParameterGolfValidationEvalReport,
    /// Final trained-model validation report.
    pub trained_validation: ParameterGolfValidationEvalReport,
    /// Raw full-precision roundtrip validation report.
    pub raw_roundtrip_validation: ParameterGolfValidationEvalReport,
    /// Int8+zlib roundtrip validation report.
    pub int8_zlib_roundtrip_validation: ParameterGolfValidationEvalReport,
    /// Stable metric identifier used for submission review.
    pub submission_metric_id: String,
    /// Submission-facing `bits per byte` score.
    pub submission_bits_per_byte: f64,
    /// Submission-facing mean loss.
    pub submission_mean_loss: f64,
    /// Improvement over the initial baseline in `bits per byte`.
    pub bits_per_byte_improvement_vs_initial: f64,
}

impl ParameterGolfChallengeScoreReport {
    /// Builds one score report from the current validation reports.
    #[must_use]
    pub fn new(
        initial_validation: ParameterGolfValidationEvalReport,
        trained_validation: ParameterGolfValidationEvalReport,
        raw_roundtrip_validation: ParameterGolfValidationEvalReport,
        int8_zlib_roundtrip_validation: ParameterGolfValidationEvalReport,
    ) -> Self {
        Self {
            benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
            submission_metric_id: String::from(PARAMETER_GOLF_SUBMISSION_METRIC_ID),
            submission_bits_per_byte: int8_zlib_roundtrip_validation.bits_per_byte,
            submission_mean_loss: int8_zlib_roundtrip_validation.mean_loss,
            bits_per_byte_improvement_vs_initial: initial_validation.bits_per_byte
                - int8_zlib_roundtrip_validation.bits_per_byte,
            initial_validation,
            trained_validation,
            raw_roundtrip_validation,
            int8_zlib_roundtrip_validation,
        }
    }

    /// Returns a stable digest over the score report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_score_report|", self)
    }

    /// Returns the score report as a Psionic eval artifact.
    #[must_use]
    pub fn as_artifact(&self) -> EvalArtifact {
        let bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        EvalArtifact::new(
            "parameter_golf_challenge_score_report",
            "parameter_golf_challenge_score_report.json",
            &bytes,
        )
    }
}

/// One artifact receipt entry used by bundle and size receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfArtifactReceiptEntry {
    /// Stable artifact kind.
    pub artifact_kind: String,
    /// Stable artifact reference.
    pub artifact_ref: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Serialized size in bytes.
    pub size_bytes: u64,
}

/// Artifact-size receipt for one Parameter Golf benchmark bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfArtifactSizeReceipt {
    /// Submission-facing artifact reference.
    pub submission_artifact_ref: String,
    /// Submission-facing artifact digest.
    pub submission_artifact_digest: String,
    /// Submission-facing artifact size.
    pub submission_artifact_size_bytes: u64,
    /// All tracked artifacts in the bundle.
    pub artifacts: Vec<ParameterGolfArtifactReceiptEntry>,
    /// Total tracked artifact bytes.
    pub total_artifact_bytes: u64,
}

impl ParameterGolfArtifactSizeReceipt {
    /// Returns a stable digest over the artifact-size receipt.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_artifact_size_receipt|", self)
    }
}

/// Wallclock receipt for one bounded Parameter Golf benchmark run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfWallclockReceipt {
    /// Stable description of how the wallclock facts were collected.
    pub measurement_posture: String,
    /// Observed wallclock for the bounded training run.
    pub training_observed_ms: u64,
    /// Logical wallclock implied by the step contract.
    pub training_logical_ms: u64,
    /// Observed wallclock for the raw-restore eval path.
    pub raw_restore_eval_observed_ms: u64,
    /// Observed wallclock for the int8+zlib restore eval path.
    pub int8_zlib_restore_eval_observed_ms: u64,
    /// Optional wallclock cap derived from the current schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallclock_cap_ms: Option<u64>,
    /// Optional within-cap posture when a cap exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_wallclock_cap: Option<bool>,
}

/// Memory receipt for one bounded Parameter Golf benchmark run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfMemoryReceipt {
    /// Stable description of how the memory facts were collected.
    pub measurement_posture: String,
    /// Estimated full-precision model parameter bytes.
    pub model_parameter_bytes: u64,
    /// Estimated live optimizer-state bytes for the current trainable surface.
    pub optimizer_state_bytes: u64,
    /// Final raw-model artifact bytes.
    pub raw_model_artifact_bytes: u64,
    /// Final int8+zlib artifact bytes.
    pub int8_zlib_artifact_bytes: u64,
    /// Final checkpoint-manifest bytes.
    pub checkpoint_manifest_bytes: u64,
    /// Estimated live upper bound across the tracked components.
    pub estimated_live_bytes_upper_bound: u64,
}

/// One bundle root that groups train-side or eval-side artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfBundleRoot {
    /// Stable bundle-root reference.
    pub root_ref: String,
    /// Artifacts attached to the root.
    pub artifacts: Vec<ParameterGolfArtifactReceiptEntry>,
}

impl ParameterGolfBundleRoot {
    /// Returns the total tracked bytes inside the root.
    #[must_use]
    pub fn total_artifact_bytes(&self) -> u64 {
        self.artifacts.iter().map(|artifact| artifact.size_bytes).sum()
    }
}

/// Aggregate benchmark receipt for the Parameter Golf review lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfChallengeBenchmarkReceipt {
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Stable benchmark package key.
    pub benchmark_package: BenchmarkPackageKey,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable descriptor digest for the trained model.
    pub trained_model_descriptor_digest: String,
    /// Stable digest for the final checkpoint manifest.
    pub final_checkpoint_manifest_digest: String,
    /// Submission-facing artifact reference.
    pub submission_artifact_ref: String,
    /// Submission-facing artifact digest.
    pub submission_artifact_digest: String,
    /// Score report for the run.
    pub score_report: ParameterGolfChallengeScoreReport,
    /// Wallclock receipt for the run.
    pub wallclock_receipt: ParameterGolfWallclockReceipt,
    /// Memory receipt for the run.
    pub memory_receipt: ParameterGolfMemoryReceipt,
    /// Artifact-size receipt for the run.
    pub artifact_size_receipt: ParameterGolfArtifactSizeReceipt,
    /// Train-side bundle root.
    pub train_bundle_root: ParameterGolfBundleRoot,
    /// Eval-side bundle root.
    pub eval_bundle_root: ParameterGolfBundleRoot,
    /// Explicit claim boundary for the receipt.
    pub claim_boundary: String,
}

impl ParameterGolfChallengeBenchmarkReceipt {
    /// Returns a stable digest over the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_parameter_golf_challenge_benchmark_receipt|", self)
    }

    /// Returns the receipt as a Psionic eval artifact.
    #[must_use]
    pub fn as_artifact(&self) -> EvalArtifact {
        let bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => error.to_string().into_bytes(),
        };
        EvalArtifact::new(
            "parameter_golf_challenge_benchmark_receipt",
            "parameter_golf_challenge_benchmark_receipt.json",
            &bytes,
        )
    }
}

/// Builds the benchmark package used by the bounded local-reference review lane.
#[must_use]
pub fn build_parameter_golf_local_reference_benchmark_package(
    version: &str,
    training_dataset_digest: &str,
    validation_dataset_digest: &str,
    sequence_length: usize,
) -> BenchmarkPackage {
    let mut benchmark_case = BenchmarkCase::new("parameter_golf.local_reference.validation");
    benchmark_case.ordinal = Some(0);
    benchmark_case.input_ref = Some(format!(
        "parameter_golf://validation/{validation_dataset_digest}"
    ));
    benchmark_case.expected_output_ref = Some(String::from(PARAMETER_GOLF_SUBMISSION_METRIC_ID));
    benchmark_case.metadata = json!({
        "sequence_length": sequence_length,
        "training_dataset_digest": training_dataset_digest,
        "validation_dataset_digest": validation_dataset_digest,
        "submission_metric_id": PARAMETER_GOLF_SUBMISSION_METRIC_ID,
        "claim_boundary": PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY
    });

    let mut package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF, version),
        "Parameter Golf Challenge Review",
        EnvironmentPackageKey::new("env.openagents.parameter_golf.challenge_eval", version),
        1,
        BenchmarkAggregationKind::MeanScore,
    )
    .with_dataset(
        DatasetKey::new("dataset://openagents/parameter_golf/local_reference_fixture", version),
        Some(String::from("validation")),
    )
    .with_verification_policy(BenchmarkVerificationPolicy {
        require_timer_integrity: true,
        require_token_accounting: true,
        require_final_state_capture: true,
        require_execution_strategy: true,
    })
    .with_cases(vec![benchmark_case]);
    package.metadata.insert(
        String::from("parameter_golf.training_dataset_digest"),
        Value::String(String::from(training_dataset_digest)),
    );
    package.metadata.insert(
        String::from("parameter_golf.validation_dataset_digest"),
        Value::String(String::from(validation_dataset_digest)),
    );
    package.metadata.insert(
        String::from("parameter_golf.sequence_length"),
        Value::from(sequence_length as u64),
    );
    package.metadata.insert(
        String::from("parameter_golf.submission_metric_id"),
        Value::String(String::from(PARAMETER_GOLF_SUBMISSION_METRIC_ID)),
    );
    package.metadata.insert(
        String::from("parameter_golf.claim_boundary"),
        Value::String(String::from(PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY)),
    );
    package.metadata.insert(
        String::from("parameter_golf.bundle_roots"),
        json!(["train_bundle_root", "eval_bundle_root"]),
    );
    package
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

    use serde_json::Value;

    use super::{
        PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF,
        PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY, PARAMETER_GOLF_SUBMISSION_METRIC_ID,
        ParameterGolfArtifactReceiptEntry, ParameterGolfArtifactSizeReceipt, ParameterGolfBundleRoot,
        ParameterGolfChallengeBenchmarkReceipt, ParameterGolfChallengeScoreReport,
        ParameterGolfMemoryReceipt, ParameterGolfValidationEvalReport, ParameterGolfWallclockReceipt,
        build_parameter_golf_local_reference_benchmark_package,
    };

    fn eval_report(label: &str, mean_loss: f64, bits_per_byte: f64) -> ParameterGolfValidationEvalReport {
        ParameterGolfValidationEvalReport {
            eval_ref: String::from(label),
            model_descriptor_digest: format!("{label}-model"),
            sequence_length: 4,
            batch_token_budget: 32,
            evaluated_sequence_count: 4,
            evaluated_token_count: 16,
            evaluated_byte_count: 16,
            mean_loss,
            bits_per_byte,
        }
    }

    #[test]
    fn parameter_golf_benchmark_package_is_machine_readable() {
        let package = build_parameter_golf_local_reference_benchmark_package(
            "2026.03.18.local_reference.v1",
            "train-digest",
            "val-digest",
            4,
        );
        assert_eq!(
            package.key.benchmark_ref,
            PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF
        );
        assert_eq!(
            package
                .metadata
                .get("parameter_golf.submission_metric_id")
                .and_then(Value::as_str),
            Some(PARAMETER_GOLF_SUBMISSION_METRIC_ID)
        );
        assert_eq!(
            package
                .metadata
                .get("parameter_golf.claim_boundary")
                .and_then(Value::as_str),
            Some(PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY)
        );
        assert_eq!(package.cases.len(), 1);
    }

    #[test]
    fn parameter_golf_challenge_benchmark_receipt_round_trips() -> Result<(), Box<dyn Error>> {
        let score_report = ParameterGolfChallengeScoreReport::new(
            eval_report("initial", 3.0, 1.5),
            eval_report("trained", 2.5, 1.3),
            eval_report("raw", 2.5, 1.3),
            eval_report("int8", 2.6, 1.31),
        );
        let train_root = ParameterGolfBundleRoot {
            root_ref: String::from("bundle://parameter_golf/run/train"),
            artifacts: vec![ParameterGolfArtifactReceiptEntry {
                artifact_kind: String::from("parameter_golf_model_int8_zlib"),
                artifact_ref: String::from("final_model.int8.ptz"),
                artifact_digest: String::from("digest"),
                size_bytes: 123,
            }],
        };
        let receipt = ParameterGolfChallengeBenchmarkReceipt {
            benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
            benchmark_package: build_parameter_golf_local_reference_benchmark_package(
                "2026.03.18.local_reference.v1",
                "train-digest",
                "val-digest",
                4,
            )
            .key,
            run_id: String::from("parameter-golf-local-reference-run"),
            trained_model_descriptor_digest: String::from("trained-model"),
            final_checkpoint_manifest_digest: String::from("checkpoint"),
            submission_artifact_ref: String::from("final_model.int8.ptz"),
            submission_artifact_digest: String::from("submission"),
            score_report,
            wallclock_receipt: ParameterGolfWallclockReceipt {
                measurement_posture: String::from("observed_process_and_logical_step_budget"),
                training_observed_ms: 100,
                training_logical_ms: 100,
                raw_restore_eval_observed_ms: 11,
                int8_zlib_restore_eval_observed_ms: 13,
                wallclock_cap_ms: Some(600_000),
                within_wallclock_cap: Some(true),
            },
            memory_receipt: ParameterGolfMemoryReceipt {
                measurement_posture: String::from("estimated_tensor_state_bytes"),
                model_parameter_bytes: 1024,
                optimizer_state_bytes: 128,
                raw_model_artifact_bytes: 900,
                int8_zlib_artifact_bytes: 400,
                checkpoint_manifest_bytes: 200,
                estimated_live_bytes_upper_bound: 2652,
            },
            artifact_size_receipt: ParameterGolfArtifactSizeReceipt {
                submission_artifact_ref: String::from("final_model.int8.ptz"),
                submission_artifact_digest: String::from("submission"),
                submission_artifact_size_bytes: 400,
                artifacts: train_root.artifacts.clone(),
                total_artifact_bytes: 123,
            },
            train_bundle_root: train_root.clone(),
            eval_bundle_root: ParameterGolfBundleRoot {
                root_ref: String::from("bundle://parameter_golf/run/eval"),
                artifacts: vec![],
            },
            claim_boundary: String::from(PARAMETER_GOLF_LOCAL_REFERENCE_CLAIM_BOUNDARY),
        };
        let encoded = serde_json::to_vec_pretty(&receipt)?;
        let decoded: ParameterGolfChallengeBenchmarkReceipt = serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, receipt);
        assert!(!receipt.stable_digest().is_empty());
        let artifact = receipt.as_artifact();
        assert_eq!(
            artifact.artifact_kind,
            "parameter_golf_challenge_benchmark_receipt"
        );
        Ok(())
    }
}
