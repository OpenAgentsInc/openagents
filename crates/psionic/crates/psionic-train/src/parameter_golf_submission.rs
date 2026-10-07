use std::{
    fmt::Write,
    fs,
    path::{Component, Path, PathBuf},
};

use psionic_eval::PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    parameter_golf_submission_runtime_payload_fixture_path,
    ParameterGolfLocalReferenceBenchmarkBundle, ParameterGolfLocalReferenceFixture,
    ParameterGolfReferenceTrainingConfig, ParameterGolfSubmissionRuntimeManifest,
    ParameterGolfTrainingArtifact,
};

/// Stable version identifier for the current non-record submission package.
pub const PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION: &str =
    "2026.03.18.non_record_submission.v2";

/// Public non-record track identifier used by Parameter Golf.
pub const PARAMETER_GOLF_NON_RECORD_TRACK_ID: &str = "non-record-unlimited-compute-16mb";

/// Public artifact cap in decimal bytes.
pub const PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES: u64 = 16_000_000;

/// Explicit claim boundary for the first non-record submission package.
pub const PARAMETER_GOLF_NON_RECORD_SUBMISSION_CLAIM_BOUNDARY: &str =
    "current honest non-record submission package only; the shipped train_gpt.py defaults to a bounded local-reference replay path, but the same exported folder now also ships a real single-H100 trainer payload plus the immutable PGOLF input-package descriptor and one Rust-owned distributed 8xH100 runtime lane for later remote execution, still without claiming record-track readiness";
pub const PARAMETER_GOLF_NON_RECORD_RUNTIME_VALIDATION_BATCH_SEQUENCES: u64 = 256;

const PARAMETER_GOLF_NON_RECORD_RECORDS_DIR: &str = "records/track_non_record_16mb";
const PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF: &str =
    "runtime/parameter_golf_submission_runtime";
const PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF: &str =
    "runtime/parameter_golf_single_h100_train";
const PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF: &str =
    "runtime/parameter_golf_submission_runtime.json";
const PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF: &str =
    "runtime/parameter_golf_real_execution_contract.json";
const PARAMETER_GOLF_RUNTIME_FIXTURE_ARTIFACT_REF: &str =
    "runtime/parameter_golf_local_reference_fixture.json";
const PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF: &str =
    "runtime/parameter_golf_google_input_package_descriptor_v1.json";
pub const PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF: &str =
    "parameter-golf-local-reference-run/benchmark/parameter_golf_submission_runtime_receipt.json";
pub const PARAMETER_GOLF_REAL_RUNTIME_REPORT_ARTIFACT_REF: &str =
    "parameter-golf-single-h100-run/benchmark/parameter_golf_single_h100_training.json";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_bringup.json";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_runtime_bootstrap.json";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_RECEIPTS_DIR_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/runtime_bootstrap_receipts";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_LOGS_DIR_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/runtime_bootstrap_logs";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RECEIPT_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_train_step.json";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RANK_RECEIPTS_DIR_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/runtime_train_step_receipts";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RANK_LOGS_DIR_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/runtime_train_step_logs";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_WINDOWS_DIR_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/runtime_train_step_windows";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_GRADIENTS_DIR_ARTIFACT_REF: &str =
    "parameter-golf-distributed-8xh100-run/benchmark/runtime_train_step_gradients";
pub const PARAMETER_GOLF_ACCOUNTING_COMPONENT_ENTRYPOINT: &str = "entrypoint_code_bytes";
pub const PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL: &str = "compressed_model_bytes";
pub const PARAMETER_GOLF_ACCOUNTING_COMPONENT_RUNTIME: &str = "shipped_runtime_code_bytes";
pub const PARAMETER_GOLF_ACCOUNTING_COMPONENT_WRAPPER: &str = "shipped_wrapper_code_bytes";
pub const PARAMETER_GOLF_ACCOUNTING_COMPONENT_BUILD_DEPS: &str = "required_build_dependency_bytes";
pub const PARAMETER_GOLF_EXECUTION_MODE_ENV_VAR: &str = "PSIONIC_PARAMETER_GOLF_EXECUTION_MODE";
pub const PARAMETER_GOLF_SINGLE_H100_DATASET_ROOT_ENV_VAR: &str =
    "PSIONIC_PARAMETER_GOLF_DATASET_ROOT";
pub const PARAMETER_GOLF_SINGLE_H100_TOKENIZER_PATH_ENV_VAR: &str =
    "PSIONIC_PARAMETER_GOLF_TOKENIZER_PATH";
const PARAMETER_GOLF_SINGLE_H100_OUTPUT_REPORT_ENV_VAR: &str =
    "PSIONIC_PARAMETER_GOLF_OUTPUT_REPORT";
const PARAMETER_GOLF_SINGLE_H100_MAX_STEPS_ENV_VAR: &str = "PSIONIC_PARAMETER_GOLF_MAX_STEPS";
pub const PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE: &str = "distributed_8xh100_train";

/// Machine-readable real execution contract shipped with the exported folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionRealExecutionContract {
    /// Stable schema version.
    pub schema_version: u32,
    /// Execution mode selected through the top-level entrypoint.
    pub execution_mode: String,
    /// Real trainer payload relative path.
    pub trainer_payload_path: String,
    /// Immutable input-package descriptor path shipped with the folder.
    pub input_package_descriptor_path: String,
    /// Environment variable that must point at a materialized dataset root.
    pub dataset_root_env_var: String,
    /// Environment variable that must point at the tokenizer file.
    pub tokenizer_path_env_var: String,
    /// Environment variable that can override the runtime report path.
    pub output_report_env_var: String,
    /// Default relative report path for the real trainer output.
    pub default_output_report_path: String,
    /// Environment variable that can override the trainer step cap.
    pub max_steps_env_var: String,
    /// Default bounded trainer step cap.
    pub default_max_steps: u64,
    /// Optional validation mode requested by this exact real execution lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_eval_mode: Option<crate::ParameterGolfValidationEvalMode>,
    /// Optional explicit validation batch geometry requested by this exact real execution lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_batch_sequences: Option<u64>,
    /// Honest claim boundary for this mode.
    pub claim_boundary: String,
}

/// Human-facing identity fields for one submission package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionIdentity {
    /// Submission author label.
    pub author: String,
    /// Public GitHub identifier.
    pub github_id: String,
    /// Run name surfaced in `submission.json`.
    pub name: String,
    /// Human-readable one-paragraph summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blurb: Option<String>,
    /// ISO-8601 UTC date string.
    pub date: String,
}

impl ParameterGolfSubmissionIdentity {
    /// Returns a repo-owned default identity for the first Psionic package lane.
    #[must_use]
    pub fn psionic_local_reference_defaults() -> Self {
        Self {
            author: String::from("Psionic"),
            github_id: String::from("OpenAgentsInc"),
            name: String::from("Psionic Local-Reference Runtime Replay"),
            blurb: None,
            date: String::from("2026-03-18T00:00:00Z"),
        }
    }
}

/// Config for one generated non-record submission package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfNonRecordSubmissionConfig {
    /// Stable submission folder identifier.
    pub submission_id: String,
    /// Human-facing identity information.
    pub identity: ParameterGolfSubmissionIdentity,
}

impl ParameterGolfNonRecordSubmissionConfig {
    /// Returns the canonical repo-owned config for the first package lane.
    #[must_use]
    pub fn local_reference_defaults() -> Self {
        Self {
            submission_id: String::from("2026-03-18_psionic_local_reference_runtime_replay_v2"),
            identity: ParameterGolfSubmissionIdentity::psionic_local_reference_defaults(),
        }
    }

    fn validate(&self) -> Result<(), ParameterGolfSubmissionError> {
        if self.submission_id.trim().is_empty() {
            return Err(ParameterGolfSubmissionError::InvalidConfig {
                message: String::from("submission_id must not be empty"),
            });
        }
        validate_relative_path(self.submission_id.as_str())?;
        if self.identity.author.trim().is_empty() {
            return Err(ParameterGolfSubmissionError::InvalidConfig {
                message: String::from("identity.author must not be empty"),
            });
        }
        if self.identity.github_id.trim().is_empty() {
            return Err(ParameterGolfSubmissionError::InvalidConfig {
                message: String::from("identity.github_id must not be empty"),
            });
        }
        if self.identity.name.trim().is_empty() {
            return Err(ParameterGolfSubmissionError::InvalidConfig {
                message: String::from("identity.name must not be empty"),
            });
        }
        if self.identity.date.trim().is_empty() {
            return Err(ParameterGolfSubmissionError::InvalidConfig {
                message: String::from("identity.date must not be empty"),
            });
        }
        Ok(())
    }
}

/// One counted component in the submission artifact accounting receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionAccountingComponent {
    /// Stable component identifier aligned to the accounting contract.
    pub component_id: String,
    /// Counted size in bytes.
    pub size_bytes: u64,
    /// Honest detail about what is or is not shipped for this component.
    pub detail: String,
}

/// Machine-readable counted-byte accounting for one submission package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionAccountingReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable package version.
    pub package_version: String,
    /// Submission track identifier.
    pub track: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Top-level entrypoint path.
    pub entrypoint_path: String,
    /// Ordered counted components.
    pub counted_components: Vec<ParameterGolfSubmissionAccountingComponent>,
    /// Total counted code bytes.
    pub counted_code_bytes: u64,
    /// Counted compressed-model bytes.
    pub compressed_model_bytes: u64,
    /// Total counted submission bytes.
    pub total_counted_bytes: u64,
    /// Whether the total stayed inside the public `16,000,000` byte cap.
    pub within_artifact_cap: bool,
    /// Explicit claim boundary for the package.
    pub claim_boundary: String,
    /// Stable digest over the accounting receipt.
    pub receipt_digest: String,
}

impl ParameterGolfSubmissionAccountingReceipt {
    fn new(
        run_id: impl Into<String>,
        entrypoint_path: impl Into<String>,
        counted_components: Vec<ParameterGolfSubmissionAccountingComponent>,
    ) -> Self {
        let run_id = run_id.into();
        let entrypoint_path = entrypoint_path.into();
        let compressed_model_bytes = counted_components
            .iter()
            .find(|component| component.component_id == PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL)
            .map_or(0, |component| component.size_bytes);
        let counted_code_bytes = counted_components
            .iter()
            .filter(|component| component.component_id != PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL)
            .map(|component| component.size_bytes)
            .sum();
        let total_counted_bytes = counted_components
            .iter()
            .map(|component| component.size_bytes)
            .sum();
        let mut receipt = Self {
            schema_version: 1,
            package_version: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION),
            track: String::from(PARAMETER_GOLF_NON_RECORD_TRACK_ID),
            run_id,
            benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
            entrypoint_path,
            counted_components,
            counted_code_bytes,
            compressed_model_bytes,
            total_counted_bytes,
            within_artifact_cap: total_counted_bytes
                <= PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES,
            claim_boundary: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_CLAIM_BOUNDARY),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.stable_digest();
        receipt
    }

    /// Returns a stable digest over the receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_submission_accounting_receipt|",
            &digestible,
        )
    }
}

/// Submission-facing `submission.json` contract for the non-record package lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfNonRecordSubmissionManifest {
    /// Human-facing author label.
    pub author: String,
    /// Public GitHub identifier.
    pub github_id: String,
    /// Human-facing run name.
    pub name: String,
    /// Submission summary.
    pub blurb: String,
    /// ISO-8601 UTC date string.
    pub date: String,
    /// Public non-record track identifier.
    pub track: String,
    /// Final roundtrip validation loss.
    pub val_loss: f64,
    /// Final roundtrip validation bits-per-byte.
    pub val_bpb: f64,
    /// Pre-quantized validation loss.
    pub pre_quant_val_loss: f64,
    /// Pre-quantized validation bits-per-byte.
    pub pre_quant_val_bpb: f64,
    /// Final completed training step.
    pub step_stop: u64,
    /// Observed training wallclock in seconds.
    pub wallclock_seconds: f64,
    /// Total counted submission bytes.
    pub bytes_total: u64,
    /// Counted compressed-model bytes.
    pub bytes_model_int8_zlib: u64,
    /// Counted code bytes.
    pub bytes_code: u64,
    /// Stable run identifier.
    pub run_id: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Canonical claim posture for the package.
    pub claim_posture: String,
    /// Explicit claim boundary for the package.
    pub claim_boundary: String,
    /// Top-level wrapper entrypoint path.
    pub entrypoint: String,
    /// Accounting receipt artifact path.
    pub accounting_receipt_artifact_ref: String,
    /// Accounting receipt artifact digest.
    pub accounting_receipt_artifact_digest: String,
    /// Benchmark receipt artifact path.
    pub benchmark_receipt_artifact_ref: String,
    /// Benchmark receipt artifact digest.
    pub benchmark_receipt_artifact_digest: String,
    /// Runtime manifest artifact path.
    pub runtime_manifest_artifact_ref: String,
    /// Runtime manifest artifact digest.
    pub runtime_manifest_artifact_digest: String,
    /// Runtime payload artifact path.
    pub runtime_payload_artifact_ref: String,
    /// Runtime payload artifact digest.
    pub runtime_payload_artifact_digest: String,
    /// Real single-H100 trainer payload artifact path.
    pub real_runtime_payload_artifact_ref: String,
    /// Real single-H100 trainer payload artifact digest.
    pub real_runtime_payload_artifact_digest: String,
    /// Real execution contract artifact path.
    pub real_execution_contract_artifact_ref: String,
    /// Real execution contract artifact digest.
    pub real_execution_contract_artifact_digest: String,
    /// Immutable input-package descriptor path shipped with the exported folder.
    pub runtime_input_package_descriptor_artifact_ref: String,
    /// Immutable input-package descriptor digest.
    pub runtime_input_package_descriptor_artifact_digest: String,
    /// Runtime receipt path written by the shipped payload.
    pub runtime_receipt_artifact_ref: String,
}

/// File role inside the generated submission folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSubmissionFileRole {
    Readme,
    SubmissionManifest,
    TrainLog,
    Entrypoint,
    RuntimePayload,
    RealRuntimePayload,
    RuntimeManifest,
    RealExecutionContract,
    RuntimeFixture,
    RuntimeInputPackageDescriptor,
    CompressedModel,
    BenchmarkPackage,
    ChallengeScoreReport,
    BenchmarkReceipt,
    AccountingReceipt,
    RunBundle,
}

/// One file in the generated submission folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionFileEntry {
    /// Relative path from the submission root.
    pub relative_path: String,
    /// Stable file role.
    pub role: ParameterGolfSubmissionFileRole,
    /// Artifact kind used inside Psionic.
    pub artifact_kind: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Whether this file counts toward the public artifact cap.
    pub counts_toward_artifact_cap: bool,
    /// Honest file detail.
    pub detail: String,
}

/// Top-level folder contract for the generated non-record submission package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfNonRecordSubmissionPackage {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable package version.
    pub package_version: String,
    /// Public record-folder root expected by the challenge repo.
    pub record_folder_relpath: String,
    /// Stable submission folder identifier.
    pub submission_id: String,
    /// Submission track identifier.
    pub track: String,
    /// Canonical claim posture for the package.
    pub claim_posture: String,
    /// Explicit claim boundary for the package.
    pub claim_boundary: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Top-level entrypoint path.
    pub entrypoint_path: String,
    /// Top-level `submission.json` path.
    pub submission_manifest_path: String,
    /// Accounting receipt path.
    pub accounting_receipt_path: String,
    /// Benchmark receipt path.
    pub benchmark_receipt_path: String,
    /// Runtime manifest path.
    pub runtime_manifest_path: String,
    /// Runtime payload path.
    pub runtime_payload_path: String,
    /// Ordered submission-folder files.
    pub files: Vec<ParameterGolfSubmissionFileEntry>,
    /// Stable digest over the package contract.
    pub package_digest: String,
}

impl ParameterGolfNonRecordSubmissionPackage {
    fn new(
        submission_id: impl Into<String>,
        run_id: impl Into<String>,
        accounting_receipt_path: impl Into<String>,
        benchmark_receipt_path: impl Into<String>,
        runtime_manifest_path: impl Into<String>,
        runtime_payload_path: impl Into<String>,
        files: Vec<ParameterGolfSubmissionFileEntry>,
    ) -> Self {
        let submission_id = submission_id.into();
        let run_id = run_id.into();
        let accounting_receipt_path = accounting_receipt_path.into();
        let benchmark_receipt_path = benchmark_receipt_path.into();
        let runtime_manifest_path = runtime_manifest_path.into();
        let runtime_payload_path = runtime_payload_path.into();
        let mut package = Self {
            schema_version: 1,
            package_version: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION),
            record_folder_relpath: format!(
                "{}/{}",
                PARAMETER_GOLF_NON_RECORD_RECORDS_DIR, submission_id
            ),
            submission_id,
            track: String::from(PARAMETER_GOLF_NON_RECORD_TRACK_ID),
            claim_posture: String::from("non_record_submission"),
            claim_boundary: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_CLAIM_BOUNDARY),
            benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
            run_id,
            entrypoint_path: String::from("train_gpt.py"),
            submission_manifest_path: String::from("submission.json"),
            accounting_receipt_path,
            benchmark_receipt_path,
            runtime_manifest_path,
            runtime_payload_path,
            files,
            package_digest: String::new(),
        };
        package.package_digest = package.stable_digest();
        package
    }

    /// Returns a stable digest over the package payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.package_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_non_record_submission_package|",
            &digestible,
        )
    }
}

/// In-memory bundle for one generated non-record submission package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfNonRecordSubmissionBundle {
    /// Typed folder contract.
    pub package: ParameterGolfNonRecordSubmissionPackage,
    /// Top-level `submission.json` payload.
    pub submission_manifest: ParameterGolfNonRecordSubmissionManifest,
    /// Machine-readable counted-byte receipt.
    pub accounting_receipt: ParameterGolfSubmissionAccountingReceipt,
    /// All file artifacts that should be materialized into the submission root.
    pub file_artifacts: Vec<ParameterGolfTrainingArtifact>,
}

impl ParameterGolfNonRecordSubmissionBundle {
    /// Returns one file artifact by its relative path.
    #[must_use]
    pub fn artifact(&self, relative_path: &str) -> Option<&ParameterGolfTrainingArtifact> {
        self.file_artifacts
            .iter()
            .find(|artifact| artifact.artifact_ref == relative_path)
    }
}

/// Submission-package construction failure.
#[derive(Debug, Error)]
pub enum ParameterGolfSubmissionError {
    #[error("parameter golf submission config is invalid: {message}")]
    InvalidConfig { message: String },
    #[error("parameter golf submission artifact path is invalid: `{path}`")]
    InvalidArtifactPath { path: String },
    #[error("{context}: {message}")]
    Serialization {
        context: &'static str,
        message: String,
    },
    #[error(transparent)]
    ReferenceTraining(#[from] crate::ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Builds the first honest non-record submission package from a local-reference benchmark bundle.
pub fn build_parameter_golf_non_record_submission_bundle(
    benchmark_bundle: &ParameterGolfLocalReferenceBenchmarkBundle,
    config: &ParameterGolfNonRecordSubmissionConfig,
) -> Result<ParameterGolfNonRecordSubmissionBundle, ParameterGolfSubmissionError> {
    config.validate()?;

    let run_id = benchmark_bundle.run_bundle.run_id.clone();
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let runtime_config = ParameterGolfReferenceTrainingConfig::local_reference();
    let model_artifact = benchmark_bundle
        .training_outcome
        .int8_zlib_model_artifact
        .clone();
    validate_relative_path(model_artifact.artifact_ref.as_str())?;
    validate_relative_path(
        benchmark_bundle
            .benchmark_package_artifact
            .artifact_ref
            .as_str(),
    )?;
    validate_relative_path(
        benchmark_bundle
            .challenge_score_report_artifact
            .artifact_ref
            .as_str(),
    )?;
    validate_relative_path(
        benchmark_bundle
            .benchmark_receipt_artifact
            .artifact_ref
            .as_str(),
    )?;
    validate_relative_path(benchmark_bundle.run_bundle_artifact.artifact_ref.as_str())?;
    validate_relative_path(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_RUNTIME_FIXTURE_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_REAL_RUNTIME_REPORT_ARTIFACT_REF)?;
    validate_relative_path(PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF)?;

    let runtime_payload_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_submission_runtime_payload",
        String::from(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF),
        fs::read(parameter_golf_submission_runtime_payload_fixture_path())?,
    );
    let real_runtime_payload_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_single_h100_runtime_payload",
        String::from(PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF),
        fs::read(parameter_golf_single_h100_runtime_payload_fixture_path())?,
    );
    let runtime_fixture_artifact = json_artifact(
        "parameter_golf_local_reference_fixture",
        String::from(PARAMETER_GOLF_RUNTIME_FIXTURE_ARTIFACT_REF),
        &fixture,
    )?;
    let runtime_input_package_descriptor_artifact = ParameterGolfTrainingArtifact::new(
        "parameter_golf_google_input_package_descriptor",
        String::from(PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF),
        fs::read(parameter_golf_google_input_package_descriptor_fixture_path())?,
    );
    let single_h100_execution_contract = ParameterGolfSubmissionRealExecutionContract {
        schema_version: 1,
        execution_mode: String::from("single_h100_train"),
        trainer_payload_path: String::from(PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF),
        input_package_descriptor_path: String::from(
            PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF,
        ),
        dataset_root_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_DATASET_ROOT_ENV_VAR),
        tokenizer_path_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_TOKENIZER_PATH_ENV_VAR),
        output_report_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_OUTPUT_REPORT_ENV_VAR),
        default_output_report_path: String::from(PARAMETER_GOLF_REAL_RUNTIME_REPORT_ARTIFACT_REF),
        max_steps_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_MAX_STEPS_ENV_VAR),
        default_max_steps: 1,
        validation_eval_mode: None,
        validation_batch_sequences: None,
        claim_boundary: String::from(
            "This exported folder now ships the real single-H100 trainer payload and the immutable PGOLF input-package descriptor. When the explicit environment contract is supplied, train_gpt.py can invoke the actual bounded Rust-owned single-H100 trainer path and preserve its machine-readable report inside the folder.",
        ),
    };
    let distributed_8xh100_execution_contract = ParameterGolfSubmissionRealExecutionContract {
        schema_version: 1,
        execution_mode: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE),
        trainer_payload_path: String::from(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF),
        input_package_descriptor_path: String::from(
            PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF,
        ),
        dataset_root_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_DATASET_ROOT_ENV_VAR),
        tokenizer_path_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_TOKENIZER_PATH_ENV_VAR),
        output_report_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_OUTPUT_REPORT_ENV_VAR),
        default_output_report_path: String::from(
            PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RECEIPT_ARTIFACT_REF,
        ),
        max_steps_env_var: String::from(PARAMETER_GOLF_SINGLE_H100_MAX_STEPS_ENV_VAR),
        default_max_steps: 0,
        validation_eval_mode: Some(crate::ParameterGolfValidationEvalMode::SlidingWindow {
            stride: 64,
        }),
        validation_batch_sequences: Some(1024),
        claim_boundary: String::from(
            "This exported folder now ships the Rust-owned distributed 8xH100 runtime lane through the same runtime payload. When the explicit environment contract is supplied and train_gpt.py is invoked in distributed_8xh100_train mode, the shipped runtime can admit the real 8xH100 machine contract, execute the retained bootstrap and train-step path, and preserve its machine-readable receipts inside the folder.",
        ),
    };
    let real_execution_contract_artifact = json_artifact(
        "parameter_golf_real_execution_contract",
        String::from(PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF),
        &single_h100_execution_contract,
    )?;
    let mut runtime_manifest = ParameterGolfSubmissionRuntimeManifest {
        schema_version: 1,
        package_version: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION),
        run_id: run_id.clone(),
        benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
        entrypoint_path: String::from("train_gpt.py"),
        runtime_payload_path: String::from(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF),
        submission_manifest_path: String::from("submission.json"),
        accounting_receipt_path: format!("{run_id}/benchmark/parameter_golf_submission_accounting.json"),
        fixture_path: String::from(PARAMETER_GOLF_RUNTIME_FIXTURE_ARTIFACT_REF),
        model_artifact_path: model_artifact.artifact_ref.clone(),
        runtime_receipt_path: String::from(PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF),
        distributed_bringup_report_path: String::from(
            PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF,
        ),
        sequence_length: runtime_config.geometry.train_sequence_length,
        validation_batch_tokens: runtime_config.geometry.local_validation_batch_tokens(),
        validation_eval_mode: crate::ParameterGolfValidationEvalMode::NonOverlapping,
        validation_batch_sequences: PARAMETER_GOLF_NON_RECORD_RUNTIME_VALIDATION_BATCH_SEQUENCES,
        score_first_ttt: None,
        expected_val_loss: benchmark_bundle
            .challenge_score_report
            .int8_zlib_roundtrip_validation
            .mean_loss,
        expected_val_bpb: benchmark_bundle
            .challenge_score_report
            .int8_zlib_roundtrip_validation
            .bits_per_byte,
        default_execution_mode: String::from("local_reference_validation"),
        real_execution_contracts: vec![
            single_h100_execution_contract.clone(),
            distributed_8xh100_execution_contract.clone(),
        ],
        runtime_posture: String::from(
            "shipped_folder_defaults to bounded local-reference replay through one prebuilt runtime payload, while also carrying the real single-H100 trainer payload plus input-package descriptor and one Rust-owned distributed 8xH100 runtime lane for explicit later remote execution",
        ),
        claim_boundary: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_CLAIM_BOUNDARY),
        manifest_digest: String::new(),
    };
    runtime_manifest.manifest_digest = runtime_manifest.stable_digest();
    let runtime_manifest_artifact = json_artifact(
        "parameter_golf_submission_runtime_manifest",
        String::from(PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF),
        &runtime_manifest,
    )?;

    let entrypoint_artifact = text_artifact(
        "parameter_golf_submission_entrypoint",
        String::from("train_gpt.py"),
        render_entrypoint_launcher(
            runtime_payload_artifact.artifact_ref.as_str(),
            runtime_manifest_artifact.artifact_ref.as_str(),
            real_execution_contract_artifact.artifact_ref.as_str(),
        ),
    );

    let accounting_receipt = ParameterGolfSubmissionAccountingReceipt::new(
        run_id.as_str(),
        entrypoint_artifact.artifact_ref.as_str(),
        vec![
            ParameterGolfSubmissionAccountingComponent {
                component_id: String::from(PARAMETER_GOLF_ACCOUNTING_COMPONENT_ENTRYPOINT),
                size_bytes: entrypoint_artifact.bytes.len() as u64,
                detail: String::from(
                    "the package ships one Python train_gpt.py launcher at the submission root that execs the shipped Psionic runtime payload",
                ),
            },
            ParameterGolfSubmissionAccountingComponent {
                component_id: String::from(PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL),
                size_bytes: model_artifact.bytes.len() as u64,
                detail: String::from(
                    "the counted model artifact is the final int8+zlib roundtrip export emitted by the Psionic local-reference lane",
                ),
            },
            ParameterGolfSubmissionAccountingComponent {
                component_id: String::from(PARAMETER_GOLF_ACCOUNTING_COMPONENT_RUNTIME),
                size_bytes: runtime_payload_artifact.bytes.len() as u64
                    + real_runtime_payload_artifact.bytes.len() as u64,
                detail: String::from(
                    "the package ships two prebuilt Psionic runtime payloads: one lightweight local-reference replay binary for the default challenge-clone dry run, plus the real single-H100 trainer binary used by the exported-folder remote execution path",
                ),
            },
            ParameterGolfSubmissionAccountingComponent {
                component_id: String::from(PARAMETER_GOLF_ACCOUNTING_COMPONENT_WRAPPER),
                size_bytes: 0,
                detail: String::from(
                    "no helper wrapper code beyond the top-level train_gpt.py launcher is shipped in this package",
                ),
            },
            ParameterGolfSubmissionAccountingComponent {
                component_id: String::from(PARAMETER_GOLF_ACCOUNTING_COMPONENT_BUILD_DEPS),
                size_bytes: 0,
                detail: String::from(
                    "the package ships a prebuilt runtime payload and requires no build step or vendored dependency tree during execution",
                ),
            },
        ],
    );
    let accounting_receipt_artifact = json_artifact(
        "parameter_golf_submission_accounting_receipt",
        format!("{run_id}/benchmark/parameter_golf_submission_accounting.json"),
        &accounting_receipt,
    )?;

    let step_stop = benchmark_bundle
        .training_outcome
        .step_metrics
        .last()
        .map_or(0, |metrics| metrics.global_step);
    let wallclock_seconds = benchmark_bundle
        .benchmark_receipt
        .wallclock_receipt
        .training_observed_ms as f64
        / 1_000.0;
    let blurb = config.identity.blurb.clone().unwrap_or_else(|| {
        format!(
            "Psionic non-record submission package for the bounded local-reference lane; it preserves benchmark and accounting receipts, ships a train_gpt.py launcher plus both the lightweight replay runtime and the real single-H100 trainer payload, and binds the later remote execution path to the committed immutable PGOLF input-package descriptor."
        )
    });
    let submission_manifest = ParameterGolfNonRecordSubmissionManifest {
        author: config.identity.author.clone(),
        github_id: config.identity.github_id.clone(),
        name: config.identity.name.clone(),
        blurb,
        date: config.identity.date.clone(),
        track: String::from(PARAMETER_GOLF_NON_RECORD_TRACK_ID),
        val_loss: benchmark_bundle
            .challenge_score_report
            .int8_zlib_roundtrip_validation
            .mean_loss,
        val_bpb: benchmark_bundle
            .challenge_score_report
            .int8_zlib_roundtrip_validation
            .bits_per_byte,
        pre_quant_val_loss: benchmark_bundle
            .challenge_score_report
            .trained_validation
            .mean_loss,
        pre_quant_val_bpb: benchmark_bundle
            .challenge_score_report
            .trained_validation
            .bits_per_byte,
        step_stop,
        wallclock_seconds,
        bytes_total: accounting_receipt.total_counted_bytes,
        bytes_model_int8_zlib: accounting_receipt.compressed_model_bytes,
        bytes_code: accounting_receipt.counted_code_bytes,
        run_id: run_id.clone(),
        benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
        claim_posture: String::from("non_record_submission"),
        claim_boundary: String::from(PARAMETER_GOLF_NON_RECORD_SUBMISSION_CLAIM_BOUNDARY),
        entrypoint: entrypoint_artifact.artifact_ref.clone(),
        accounting_receipt_artifact_ref: accounting_receipt_artifact.artifact_ref.clone(),
        accounting_receipt_artifact_digest: accounting_receipt_artifact.artifact_digest.clone(),
        benchmark_receipt_artifact_ref: benchmark_bundle
            .benchmark_receipt_artifact
            .artifact_ref
            .clone(),
        benchmark_receipt_artifact_digest: benchmark_bundle
            .benchmark_receipt_artifact
            .artifact_digest
            .clone(),
        runtime_manifest_artifact_ref: runtime_manifest_artifact.artifact_ref.clone(),
        runtime_manifest_artifact_digest: runtime_manifest_artifact.artifact_digest.clone(),
        runtime_payload_artifact_ref: runtime_payload_artifact.artifact_ref.clone(),
        runtime_payload_artifact_digest: runtime_payload_artifact.artifact_digest.clone(),
        real_runtime_payload_artifact_ref: real_runtime_payload_artifact.artifact_ref.clone(),
        real_runtime_payload_artifact_digest: real_runtime_payload_artifact.artifact_digest.clone(),
        real_execution_contract_artifact_ref: real_execution_contract_artifact.artifact_ref.clone(),
        real_execution_contract_artifact_digest: real_execution_contract_artifact
            .artifact_digest
            .clone(),
        runtime_input_package_descriptor_artifact_ref: runtime_input_package_descriptor_artifact
            .artifact_ref
            .clone(),
        runtime_input_package_descriptor_artifact_digest: runtime_input_package_descriptor_artifact
            .artifact_digest
            .clone(),
        runtime_receipt_artifact_ref: String::from(PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF),
    };
    let submission_manifest_artifact = json_artifact(
        "parameter_golf_submission_manifest",
        String::from("submission.json"),
        &submission_manifest,
    )?;
    let train_log_artifact = text_artifact(
        "parameter_golf_submission_train_log",
        String::from("train.log"),
        render_train_log(benchmark_bundle, &submission_manifest),
    );
    let readme_artifact = text_artifact(
        "parameter_golf_submission_readme",
        String::from("README.md"),
        render_readme(
            benchmark_bundle,
            &submission_manifest,
            &accounting_receipt,
            config.submission_id.as_str(),
        ),
    );

    let file_artifacts = vec![
        readme_artifact,
        submission_manifest_artifact,
        train_log_artifact,
        entrypoint_artifact,
        runtime_payload_artifact,
        real_runtime_payload_artifact,
        runtime_manifest_artifact,
        real_execution_contract_artifact,
        runtime_fixture_artifact,
        runtime_input_package_descriptor_artifact,
        model_artifact,
        benchmark_bundle.benchmark_package_artifact.clone(),
        benchmark_bundle.challenge_score_report_artifact.clone(),
        benchmark_bundle.benchmark_receipt_artifact.clone(),
        accounting_receipt_artifact,
        benchmark_bundle.run_bundle_artifact.clone(),
    ];
    for artifact in &file_artifacts {
        validate_relative_path(artifact.artifact_ref.as_str())?;
    }

    let files = file_artifacts
        .iter()
        .map(|artifact| {
            let (role, counts_toward_artifact_cap, detail) =
                submission_role_and_detail(artifact.artifact_ref.as_str());
            ParameterGolfSubmissionFileEntry {
                relative_path: artifact.artifact_ref.clone(),
                role,
                artifact_kind: artifact.artifact_kind.clone(),
                artifact_digest: artifact.artifact_digest.clone(),
                size_bytes: artifact.bytes.len() as u64,
                counts_toward_artifact_cap,
                detail,
            }
        })
        .collect::<Vec<_>>();

    let package = ParameterGolfNonRecordSubmissionPackage::new(
        config.submission_id.as_str(),
        run_id,
        format!(
            "{}/benchmark/parameter_golf_submission_accounting.json",
            benchmark_bundle.run_bundle.run_id
        ),
        benchmark_bundle
            .benchmark_receipt_artifact
            .artifact_ref
            .as_str(),
        String::from(PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF),
        String::from(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF),
        files,
    );

    Ok(ParameterGolfNonRecordSubmissionBundle {
        package,
        submission_manifest,
        accounting_receipt,
        file_artifacts,
    })
}

/// Writes the generated submission package to one folder root.
pub fn write_parameter_golf_non_record_submission_bundle(
    bundle: &ParameterGolfNonRecordSubmissionBundle,
    output_dir: &Path,
) -> Result<(), ParameterGolfSubmissionError> {
    fs::create_dir_all(output_dir)?;
    for artifact in &bundle.file_artifacts {
        validate_relative_path(artifact.artifact_ref.as_str())?;
        let path = output_dir.join(artifact.artifact_ref.as_str());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, artifact.bytes.as_slice())?;
        maybe_make_executable(artifact.artifact_ref.as_str(), &path)?;
    }
    Ok(())
}

fn maybe_make_executable(
    relative_path: &str,
    path: &Path,
) -> Result<(), ParameterGolfSubmissionError> {
    #[cfg(unix)]
    {
        if matches!(
            relative_path,
            "train_gpt.py"
                | PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF
                | PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF
        ) {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions)?;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (relative_path, path);
    }
    Ok(())
}

fn submission_role_and_detail(
    relative_path: &str,
) -> (ParameterGolfSubmissionFileRole, bool, String) {
    match relative_path {
        "README.md" => (
            ParameterGolfSubmissionFileRole::Readme,
            false,
            String::from("human-readable non-record submission overview"),
        ),
        "submission.json" => (
            ParameterGolfSubmissionFileRole::SubmissionManifest,
            false,
            String::from("leaderboard-facing metadata plus explicit Psionic claim boundary"),
        ),
        "train.log" => (
            ParameterGolfSubmissionFileRole::TrainLog,
            false,
            String::from("preserved training-step log synthesized from the bounded Psionic run"),
        ),
        "train_gpt.py" => (
            ParameterGolfSubmissionFileRole::Entrypoint,
            true,
            String::from(
                "top-level Python launcher that defaults to the lightweight replay runtime and can also dispatch to the shipped single-H100 trainer payload through an explicit execution-mode environment contract",
            ),
        ),
        PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF => (
            ParameterGolfSubmissionFileRole::RuntimePayload,
            true,
            String::from(
                "shipped Psionic runtime payload used by the default local-reference replay path",
            ),
        ),
        PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF => (
            ParameterGolfSubmissionFileRole::RealRuntimePayload,
            true,
            String::from(
                "shipped Psionic single-H100 trainer payload used when the exported folder is run in explicit real-execution mode",
            ),
        ),
        PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF => (
            ParameterGolfSubmissionFileRole::RuntimeManifest,
            false,
            String::from(
                "machine-readable runtime manifest consumed by the shipped Psionic payload",
            ),
        ),
        PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF => (
            ParameterGolfSubmissionFileRole::RealExecutionContract,
            false,
            String::from(
                "machine-readable contract that binds the exported folder to the real single-H100 trainer payload and the required execution-mode environment variables",
            ),
        ),
        PARAMETER_GOLF_RUNTIME_FIXTURE_ARTIFACT_REF => (
            ParameterGolfSubmissionFileRole::RuntimeFixture,
            false,
            String::from(
                "shipped bounded local-reference fixture reused by the runtime replay path",
            ),
        ),
        PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF => (
            ParameterGolfSubmissionFileRole::RuntimeInputPackageDescriptor,
            false,
            String::from(
                "immutable PGOLF input-package descriptor shipped with the folder so later remote runs stay bound to the same dataset and tokenizer contract",
            ),
        ),
        _ if relative_path.ends_with("final_model.int8.ptz") => (
            ParameterGolfSubmissionFileRole::CompressedModel,
            true,
            String::from("final int8+zlib model artifact counted toward the public byte cap"),
        ),
        _ if relative_path.ends_with("parameter_golf_benchmark_package.json") => (
            ParameterGolfSubmissionFileRole::BenchmarkPackage,
            false,
            String::from("preserved benchmark-package contract for offline review"),
        ),
        _ if relative_path.ends_with("parameter_golf_challenge_score_report.json") => (
            ParameterGolfSubmissionFileRole::ChallengeScoreReport,
            false,
            String::from("preserved final score report for the packaged run"),
        ),
        _ if relative_path.ends_with("parameter_golf_challenge_benchmark_receipt.json") => (
            ParameterGolfSubmissionFileRole::BenchmarkReceipt,
            false,
            String::from("preserved benchmark receipt for the packaged run"),
        ),
        _ if relative_path.ends_with("parameter_golf_submission_accounting.json") => (
            ParameterGolfSubmissionFileRole::AccountingReceipt,
            false,
            String::from("machine-readable counted-byte accounting receipt"),
        ),
        _ => (
            ParameterGolfSubmissionFileRole::RunBundle,
            false,
            String::from("preserved run-bundle artifact tying train and eval evidence together"),
        ),
    }
}

fn render_entrypoint_launcher(
    runtime_payload_ref: &str,
    runtime_manifest_ref: &str,
    real_execution_contract_ref: &str,
) -> String {
    format!(
        r#"#!/usr/bin/env python3
from pathlib import Path
import json
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
RUNTIME_PAYLOAD = ROOT / "{runtime_payload_ref}"
RUNTIME_MANIFEST = ROOT / "{runtime_manifest_ref}"
REAL_EXECUTION_CONTRACT = ROOT / "{real_execution_contract_ref}"
EXECUTION_MODE = os.environ.get("{execution_mode_env_var}", "local_reference_validation")

if EXECUTION_MODE == "local_reference_validation":
    completed = subprocess.run([str(RUNTIME_PAYLOAD), str(RUNTIME_MANIFEST)], cwd=ROOT, check=False)
    sys.exit(completed.returncode)

if EXECUTION_MODE == "single_h100_train":
    contract = json.loads(REAL_EXECUTION_CONTRACT.read_text(encoding="utf-8"))
    dataset_root = os.environ.get(contract["dataset_root_env_var"])
    tokenizer_path = os.environ.get(contract["tokenizer_path_env_var"])
    missing = []
    if not dataset_root:
        missing.append(contract["dataset_root_env_var"])
    if not tokenizer_path:
        missing.append(contract["tokenizer_path_env_var"])
    if missing:
        print(
            "parameter golf submission runtime missing required environment variable(s) for real execution mode: "
            + ", ".join(missing),
            file=sys.stderr,
        )
        sys.exit(1)
    output_value = os.environ.get(
        contract["output_report_env_var"],
        contract["default_output_report_path"],
    )
    output_path = Path(output_value)
    if not output_path.is_absolute():
        output_path = ROOT / output_path
    max_steps = os.environ.get(
        contract["max_steps_env_var"],
        str(contract["default_max_steps"]),
    )
    trainer_payload = ROOT / contract["trainer_payload_path"]
    completed = subprocess.run(
        [str(trainer_payload), dataset_root, tokenizer_path, str(output_path), max_steps],
        cwd=ROOT,
        check=False,
    )
    sys.exit(completed.returncode)

if EXECUTION_MODE == "{distributed_8xh100_execution_mode}":
    completed = subprocess.run([str(RUNTIME_PAYLOAD), str(RUNTIME_MANIFEST)], cwd=ROOT, check=False)
    sys.exit(completed.returncode)

print(f"unsupported execution mode: {{EXECUTION_MODE}}", file=sys.stderr)
sys.exit(1)
"#,
        runtime_payload_ref = runtime_payload_ref,
        runtime_manifest_ref = runtime_manifest_ref,
        real_execution_contract_ref = real_execution_contract_ref,
        execution_mode_env_var = PARAMETER_GOLF_EXECUTION_MODE_ENV_VAR,
        distributed_8xh100_execution_mode = PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE,
    )
}

fn render_train_log(
    benchmark_bundle: &ParameterGolfLocalReferenceBenchmarkBundle,
    submission_manifest: &ParameterGolfNonRecordSubmissionManifest,
) -> String {
    let mut log = String::new();
    let _ = writeln!(
        log,
        "psionic_parameter_golf_submission run_id={} track={} benchmark_ref={}",
        submission_manifest.run_id, submission_manifest.track, submission_manifest.benchmark_ref
    );
    for step in &benchmark_bundle.training_outcome.step_metrics {
        let _ = writeln!(
            log,
            "step={} mean_microbatch_loss={:.8} validation_loss={:.8} validation_bpb={:.8} learning_rate_multiplier={:.8} muon_momentum={:.8}",
            step.global_step,
            step.mean_microbatch_loss,
            step.validation_mean_loss,
            step.validation_bits_per_byte,
            step.learning_rate_multiplier,
            step.muon_momentum,
        );
    }
    let _ = writeln!(
        log,
        "trained_validation_exact val_loss:{:.8} val_bpb:{:.8}",
        benchmark_bundle
            .challenge_score_report
            .trained_validation
            .mean_loss,
        benchmark_bundle
            .challenge_score_report
            .trained_validation
            .bits_per_byte,
    );
    let _ = writeln!(
        log,
        "final_int8_zlib_roundtrip_exact val_loss:{:.8} val_bpb:{:.8}",
        benchmark_bundle
            .challenge_score_report
            .int8_zlib_roundtrip_validation
            .mean_loss,
        benchmark_bundle
            .challenge_score_report
            .int8_zlib_roundtrip_validation
            .bits_per_byte,
    );
    log
}

fn render_readme(
    benchmark_bundle: &ParameterGolfLocalReferenceBenchmarkBundle,
    submission_manifest: &ParameterGolfNonRecordSubmissionManifest,
    accounting_receipt: &ParameterGolfSubmissionAccountingReceipt,
    submission_id: &str,
) -> String {
    let mut readme = String::new();
    let _ = writeln!(
        readme,
        "This record captures a Psionic-owned non-record submission package for the bounded local-reference Parameter Golf lane.\n"
    );
    let _ = writeln!(
        readme,
        "This package is intentionally **not** a record-track runtime claim. The shipped `train_gpt.py` still defaults to a prebuilt Psionic runtime payload that restores the included int8+zlib model artifact and re-runs the bounded local-reference validation path on the shipped fixture, but the same folder now also carries the real single-H100 trainer payload plus the immutable PGOLF input-package descriptor for explicit remote execution.\n"
    );
    let _ = writeln!(readme, "Configuration:");
    let _ = writeln!(readme, "- Track: `{}`", submission_manifest.track);
    let _ = writeln!(readme, "- Run ID: `{}`", submission_manifest.run_id);
    let _ = writeln!(
        readme,
        "- Claim posture: `{}`",
        submission_manifest.claim_posture
    );
    let _ = writeln!(
        readme,
        "- Claim boundary: `{}`",
        submission_manifest.claim_boundary
    );
    let _ = writeln!(
        readme,
        "- Wrapper entrypoint: `{}`",
        submission_manifest.entrypoint
    );
    let _ = writeln!(
        readme,
        "- Runtime payload: `{}`",
        submission_manifest.runtime_payload_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- Real trainer payload: `{}`",
        submission_manifest.real_runtime_payload_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- Runtime manifest: `{}`",
        submission_manifest.runtime_manifest_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- Real execution contract: `{}`",
        submission_manifest.real_execution_contract_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- Input-package descriptor: `{}`",
        submission_manifest.runtime_input_package_descriptor_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- Runtime receipt path: `{}`",
        submission_manifest.runtime_receipt_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- Validation oracle: `{}`\n",
        submission_manifest.benchmark_ref
    );
    let _ = writeln!(readme, "Key metrics:");
    let _ = writeln!(
        readme,
        "- Trained validation: `val_loss = {:.8}`, `val_bpb = {:.8}`",
        benchmark_bundle
            .challenge_score_report
            .trained_validation
            .mean_loss,
        benchmark_bundle
            .challenge_score_report
            .trained_validation
            .bits_per_byte,
    );
    let _ = writeln!(
        readme,
        "- Final int8+zlib roundtrip: `val_loss = {:.8}`, `val_bpb = {:.8}`",
        submission_manifest.val_loss, submission_manifest.val_bpb
    );
    let _ = writeln!(
        readme,
        "- Training wallclock: `{:.3}s`",
        submission_manifest.wallclock_seconds
    );
    let _ = writeln!(
        readme,
        "- Counted bytes: `{}` total = `{}` code + `{}` compressed model\n",
        submission_manifest.bytes_total,
        submission_manifest.bytes_code,
        submission_manifest.bytes_model_int8_zlib,
    );
    let _ = writeln!(readme, "Artifact accounting:");
    for component in &accounting_receipt.counted_components {
        let _ = writeln!(
            readme,
            "- `{}`: `{}` bytes; {}",
            component.component_id, component.size_bytes, component.detail
        );
    }
    let _ = writeln!(
        readme,
        "- Within 16,000,000-byte cap: `{}`\n",
        accounting_receipt.within_artifact_cap
    );
    let _ = writeln!(readme, "Included files:");
    let _ = writeln!(readme, "- `README.md`");
    let _ = writeln!(readme, "- `submission.json`");
    let _ = writeln!(readme, "- `train.log`");
    let _ = writeln!(readme, "- `train_gpt.py`");
    let _ = writeln!(
        readme,
        "- `{}`",
        submission_manifest.runtime_payload_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        submission_manifest.runtime_manifest_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        submission_manifest.real_execution_contract_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        PARAMETER_GOLF_RUNTIME_FIXTURE_ARTIFACT_REF
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        submission_manifest.runtime_input_package_descriptor_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        submission_manifest.real_runtime_payload_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        benchmark_bundle
            .training_outcome
            .int8_zlib_model_artifact
            .artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        benchmark_bundle.benchmark_package_artifact.artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        benchmark_bundle
            .challenge_score_report_artifact
            .artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        benchmark_bundle.benchmark_receipt_artifact.artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}/benchmark/parameter_golf_submission_accounting.json`",
        submission_manifest.run_id
    );
    let _ = writeln!(
        readme,
        "- `{}`",
        submission_manifest.runtime_receipt_artifact_ref
    );
    let _ = writeln!(
        readme,
        "- `{}`\n",
        benchmark_bundle.run_bundle_artifact.artifact_ref
    );
    let _ = writeln!(readme, "\nReal execution mode:");
    let _ = writeln!(
        readme,
        "- Set `{}` to `single_h100_train` to dispatch the top-level `train_gpt.py` entrypoint to the shipped single-H100 trainer payload.",
        PARAMETER_GOLF_EXECUTION_MODE_ENV_VAR
    );
    let _ = writeln!(
        readme,
        "- Set `{}` and `{}` to the materialized FineWeb SP1024 dataset root and tokenizer path.",
        PARAMETER_GOLF_SINGLE_H100_DATASET_ROOT_ENV_VAR,
        PARAMETER_GOLF_SINGLE_H100_TOKENIZER_PATH_ENV_VAR
    );
    let _ = writeln!(
        readme,
        "- Optional overrides: `{}` for the trainer report path and `{}` for the trainer step cap.\n",
        PARAMETER_GOLF_SINGLE_H100_OUTPUT_REPORT_ENV_VAR,
        PARAMETER_GOLF_SINGLE_H100_MAX_STEPS_ENV_VAR
    );
    let _ = writeln!(
        readme,
        "- Set `{}` to `{}` on the exported-folder `8xH100` lane to dispatch into the shipped Rust-owned distributed runtime path. That mode now writes the machine-readable bring-up report, one aggregate runtime-bootstrap receipt at `{}`, retained per-rank bootstrap receipts under `{}`, retained per-rank bootstrap logs under `{}`, one aggregate train-step receipt at `{}`, retained per-rank train-step receipts under `{}`, retained per-rank train-step logs under `{}`, retained train-step window plans under `{}`, retained per-rank gradient artifacts under `{}`, retained runtime-owned post-step model artifacts beside the same benchmark root, retained per-rank validation receipts and logs beside the same benchmark root, one measured distributed receipt at `parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_receipt.json`, and one completion receipt bound to the trained runtime-produced final artifact when the distributed validation-backed lane succeeds.\n",
        PARAMETER_GOLF_EXECUTION_MODE_ENV_VAR,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_RECEIPTS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RANK_LOGS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RECEIPT_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RANK_RECEIPTS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RANK_LOGS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_WINDOWS_DIR_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_GRADIENTS_DIR_ARTIFACT_REF
    );
    let _ = writeln!(
        readme,
        "The package root for challenge-repo publication is `{}/{}`.",
        PARAMETER_GOLF_NON_RECORD_RECORDS_DIR, submission_id
    );
    readme
}

fn parameter_golf_single_h100_runtime_payload_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
        .join("fixtures/parameter_golf/runtime/parameter_golf_single_h100_train.x86_64-unknown-linux-gnu")
}

fn parameter_golf_google_input_package_descriptor_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
        .join(
            "fixtures/parameter_golf/google/parameter_golf_google_input_package_descriptor_v1.json",
        )
}

fn text_artifact(
    artifact_kind: &'static str,
    artifact_ref: String,
    text: String,
) -> ParameterGolfTrainingArtifact {
    ParameterGolfTrainingArtifact::new(artifact_kind, artifact_ref, text.into_bytes())
}

fn json_artifact<T: Serialize>(
    artifact_kind: &'static str,
    artifact_ref: String,
    value: &T,
) -> Result<ParameterGolfTrainingArtifact, ParameterGolfSubmissionError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        ParameterGolfSubmissionError::Serialization {
            context: "parameter golf submission artifact serialization",
            message: error.to_string(),
        }
    })?;
    Ok(ParameterGolfTrainingArtifact::new(
        artifact_kind,
        artifact_ref,
        bytes,
    ))
}

fn validate_relative_path(path: &str) -> Result<(), ParameterGolfSubmissionError> {
    if path.trim().is_empty() {
        return Err(ParameterGolfSubmissionError::InvalidArtifactPath {
            path: String::from(path),
        });
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(ParameterGolfSubmissionError::InvalidArtifactPath {
            path: String::from(path),
        });
    }
    for component in candidate.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => {
                return Err(ParameterGolfSubmissionError::InvalidArtifactPath {
                    path: String::from(path),
                });
            }
        }
    }
    Ok(())
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
    use std::process::Command;
    use std::{
        error::Error,
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    use super::{
        build_parameter_golf_non_record_submission_bundle,
        write_parameter_golf_non_record_submission_bundle,
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_BUILD_DEPS,
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_ENTRYPOINT, PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL,
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_RUNTIME, PARAMETER_GOLF_ACCOUNTING_COMPONENT_WRAPPER,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF,
        PARAMETER_GOLF_NON_RECORD_RUNTIME_VALIDATION_BATCH_SEQUENCES,
        PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION, PARAMETER_GOLF_NON_RECORD_TRACK_ID,
        PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF,
        PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF,
        PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF,
        PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF, PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF,
        PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF,
    };
    use crate::{
        benchmark_parameter_golf_local_reference, ParameterGolfDistributed8xH100BringupReport,
        ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionConfig,
        ParameterGolfReferenceTrainingConfig, ParameterGolfSubmissionRuntimeManifest,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .expect("repo root should resolve from psionic-train crate dir")
    }

    fn install_current_runtime_wrapper(submission_root: &Path) -> Result<(), Box<dyn Error>> {
        let runtime_path = submission_root.join(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF);
        let manifest_path = repo_root().join("crates/psionic-train/Cargo.toml");
        let wrapper = format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nexec cargo run -q --manifest-path '{}' --bin parameter_golf_submission_runtime -- \"$@\"\n",
            manifest_path.display()
        );
        fs::write(&runtime_path, wrapper)?;
        let mut permissions = fs::metadata(&runtime_path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&runtime_path, permissions)?;
        Ok(())
    }

    #[test]
    fn parameter_golf_non_record_submission_bundle_is_machine_readable_and_writes_folder_contract(
    ) -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &config)?;
        let submission_bundle = build_parameter_golf_non_record_submission_bundle(
            &benchmark_bundle,
            &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
        )?;

        assert_eq!(
            submission_bundle.package.package_version,
            PARAMETER_GOLF_NON_RECORD_SUBMISSION_VERSION
        );
        assert_eq!(
            submission_bundle.package.track,
            PARAMETER_GOLF_NON_RECORD_TRACK_ID
        );
        assert_eq!(
            submission_bundle.package.claim_posture,
            "non_record_submission"
        );
        assert_eq!(submission_bundle.package.entrypoint_path, "train_gpt.py");
        assert!(submission_bundle.accounting_receipt.within_artifact_cap);
        assert!(
            submission_bundle.submission_manifest.bytes_total
                == submission_bundle.accounting_receipt.total_counted_bytes
        );
        assert!(submission_bundle
            .artifact("README.md")
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact("submission.json")
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact("train.log")
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact("train_gpt.py")
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF)
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact(PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF)
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact(PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF)
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle
            .artifact(PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF)
            .is_some_and(|artifact| !artifact.bytes.is_empty()));
        assert!(submission_bundle.package.files.iter().any(|file| {
            file.relative_path.ends_with("final_model.int8.ptz") && file.counts_toward_artifact_cap
        }));

        let temp_dir = tempfile::tempdir()?;
        write_parameter_golf_non_record_submission_bundle(&submission_bundle, temp_dir.path())?;
        assert!(temp_dir.path().join("README.md").is_file());
        assert!(temp_dir.path().join("submission.json").is_file());
        assert!(temp_dir.path().join("train.log").is_file());
        assert!(temp_dir.path().join("train_gpt.py").is_file());
        assert!(temp_dir
            .path()
            .join(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF)
            .is_file());
        assert!(temp_dir
            .path()
            .join(PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF)
            .is_file());
        assert!(temp_dir
            .path()
            .join(PARAMETER_GOLF_REAL_RUNTIME_CONTRACT_ARTIFACT_REF)
            .is_file());
        assert!(temp_dir
            .path()
            .join(PARAMETER_GOLF_RUNTIME_INPUT_PACKAGE_DESCRIPTOR_ARTIFACT_REF)
            .is_file());
        assert!(temp_dir
            .path()
            .join(format!(
                "{}/benchmark/parameter_golf_challenge_benchmark_receipt.json",
                benchmark_bundle.run_bundle.run_id
            ))
            .is_file());
        let submission_json = fs::read_to_string(temp_dir.path().join("submission.json"))?;
        assert!(submission_json.contains("\"track\": \"non-record-unlimited-compute-16mb\""));
        let readme = fs::read_to_string(temp_dir.path().join("README.md"))?;
        assert!(readme.contains("non-record submission package"));
        let runtime_manifest: ParameterGolfSubmissionRuntimeManifest = serde_json::from_slice(
            &submission_bundle
                .artifact(PARAMETER_GOLF_RUNTIME_MANIFEST_ARTIFACT_REF)
                .expect("runtime manifest artifact should exist")
                .bytes,
        )?;
        assert_eq!(
            runtime_manifest
                .real_execution_contracts
                .iter()
                .map(|contract| contract.execution_mode.as_str())
                .collect::<Vec<_>>(),
            vec![
                "single_h100_train",
                PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE
            ]
        );
        assert_eq!(
            runtime_manifest.validation_eval_mode,
            crate::ParameterGolfValidationEvalMode::NonOverlapping
        );
        assert_eq!(
            runtime_manifest.validation_batch_sequences,
            PARAMETER_GOLF_NON_RECORD_RUNTIME_VALIDATION_BATCH_SEQUENCES
        );
        let distributed_contract = runtime_manifest
            .real_execution_contracts
            .iter()
            .find(|contract| {
                contract.execution_mode == PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE
            })
            .expect("distributed execution contract should exist");
        assert_eq!(
            distributed_contract.validation_eval_mode,
            Some(crate::ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 })
        );
        assert_eq!(distributed_contract.validation_batch_sequences, Some(1024));
        assert!(runtime_manifest
            .runtime_posture
            .contains("distributed 8xH100 runtime lane"));
        Ok(())
    }

    #[test]
    fn parameter_golf_non_record_submission_entrypoint_runs_shipped_runtime(
    ) -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &config)?;
        let submission_bundle = build_parameter_golf_non_record_submission_bundle(
            &benchmark_bundle,
            &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
        )?;

        let temp_dir = tempfile::tempdir()?;
        write_parameter_golf_non_record_submission_bundle(&submission_bundle, temp_dir.path())?;
        install_current_runtime_wrapper(temp_dir.path())?;

        let completed = Command::new("python3")
            .arg("train_gpt.py")
            .current_dir(temp_dir.path())
            .output()?;
        assert!(
            completed.status.success(),
            "entrypoint failed: stdout=`{}` stderr=`{}`",
            String::from_utf8_lossy(&completed.stdout),
            String::from_utf8_lossy(&completed.stderr),
        );
        let stdout = String::from_utf8_lossy(&completed.stdout);
        assert!(stdout.contains("psionic_non_record_submission_runtime"));
        assert!(stdout.contains("final_int8_zlib_roundtrip_exact"));
        assert!(temp_dir
            .path()
            .join(PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF)
            .is_file());
        Ok(())
    }

    #[test]
    fn parameter_golf_non_record_submission_entrypoint_refuses_distributed_8xh100_machine_contract(
    ) -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &config)?;
        let submission_bundle = build_parameter_golf_non_record_submission_bundle(
            &benchmark_bundle,
            &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
        )?;

        let temp_dir = tempfile::tempdir()?;
        write_parameter_golf_non_record_submission_bundle(&submission_bundle, temp_dir.path())?;
        install_current_runtime_wrapper(temp_dir.path())?;

        let completed = Command::new("python3")
            .arg("train_gpt.py")
            .env(
                "PSIONIC_PARAMETER_GOLF_EXECUTION_MODE",
                PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE,
            )
            .current_dir(temp_dir.path())
            .output()?;
        assert!(
            !completed.status.success(),
            "distributed 8xH100 mode should refuse when the local machine contract cannot satisfy the lane"
        );
        let stderr = String::from_utf8_lossy(&completed.stderr);
        assert!(
            stderr.contains(
                "refused the distributed 8xH100 machine contract before runtime bootstrap"
            ),
            "unexpected stderr=`{stderr}`",
        );
        assert!(stderr.contains("wrote distributed bring-up report"));
        assert!(!temp_dir
            .path()
            .join(PARAMETER_GOLF_RUNTIME_RECEIPT_ARTIFACT_REF)
            .exists());
        assert!(!temp_dir
            .path()
            .join(PARAMETER_GOLF_DISTRIBUTED_8XH100_RUNTIME_BOOTSTRAP_RECEIPT_ARTIFACT_REF)
            .exists());
        let report_path = temp_dir
            .path()
            .join(PARAMETER_GOLF_DISTRIBUTED_8XH100_BRINGUP_REPORT_ARTIFACT_REF);
        assert!(report_path.is_file());
        let report: ParameterGolfDistributed8xH100BringupReport =
            serde_json::from_slice(&fs::read(&report_path)?)?;
        assert_eq!(
            report.disposition,
            crate::ParameterGolfDistributed8xH100BringupDisposition::RefusedMachineContract
        );
        assert!(!report.machine_contract_satisfied);
        Ok(())
    }

    #[test]
    fn parameter_golf_non_record_submission_accounting_counts_real_runtime_payload(
    ) -> Result<(), Box<dyn Error>> {
        let fixture = ParameterGolfLocalReferenceFixture::reference()?;
        let config = ParameterGolfReferenceTrainingConfig::local_reference();
        let benchmark_bundle = benchmark_parameter_golf_local_reference(&fixture, &config)?;
        let submission_bundle = build_parameter_golf_non_record_submission_bundle(
            &benchmark_bundle,
            &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
        )?;

        let runtime_payload_size = submission_bundle
            .artifact(PARAMETER_GOLF_RUNTIME_PAYLOAD_ARTIFACT_REF)
            .expect("runtime payload artifact should exist")
            .bytes
            .len() as u64;
        let real_runtime_payload_size = submission_bundle
            .artifact(PARAMETER_GOLF_REAL_RUNTIME_PAYLOAD_ARTIFACT_REF)
            .expect("real runtime payload artifact should exist")
            .bytes
            .len() as u64;
        let entrypoint_size = submission_bundle
            .artifact("train_gpt.py")
            .expect("entrypoint artifact should exist")
            .bytes
            .len() as u64;
        let model_size = benchmark_bundle
            .training_outcome
            .int8_zlib_model_artifact
            .bytes
            .len() as u64;

        let receipt = &submission_bundle.accounting_receipt;
        let component_size = |component_id: &str| -> u64 {
            receipt
                .counted_components
                .iter()
                .find(|component| component.component_id == component_id)
                .map(|component| component.size_bytes)
                .expect("counted component should exist")
        };

        assert_eq!(
            component_size(PARAMETER_GOLF_ACCOUNTING_COMPONENT_ENTRYPOINT),
            entrypoint_size
        );
        assert_eq!(
            component_size(PARAMETER_GOLF_ACCOUNTING_COMPONENT_RUNTIME),
            runtime_payload_size + real_runtime_payload_size
        );
        assert_eq!(
            component_size(PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL),
            model_size
        );
        assert_eq!(
            component_size(PARAMETER_GOLF_ACCOUNTING_COMPONENT_WRAPPER),
            0
        );
        assert_eq!(
            component_size(PARAMETER_GOLF_ACCOUNTING_COMPONENT_BUILD_DEPS),
            0
        );
        assert_eq!(
            receipt.total_counted_bytes,
            entrypoint_size + runtime_payload_size + real_runtime_payload_size + model_size
        );
        assert!(receipt.within_artifact_cap);
        Ok(())
    }
}
