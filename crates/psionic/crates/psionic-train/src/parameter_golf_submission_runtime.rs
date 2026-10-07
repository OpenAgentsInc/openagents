use std::{
    env, fs,
    path::{Path, PathBuf},
};

use psionic_eval::{evaluate_parameter_golf_validation, ParameterGolfValidationEvalReport};
use psionic_models::ParameterGolfReferenceModel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    execute_parameter_golf_backed_dense_rank_runtime,
    execute_parameter_golf_distributed_8xh100_runtime_bootstrap_child,
    execute_parameter_golf_distributed_8xh100_train_step_child,
    execute_parameter_golf_distributed_8xh100_validation_child,
    parameter_golf_distributed_8xh100_runtime_bootstrap_child_enabled,
    parameter_golf_distributed_8xh100_train_step_child_enabled,
    parameter_golf_distributed_8xh100_validation_child_enabled,
    restore_parameter_golf_model_from_int8_zlib,
    write_parameter_golf_distributed_8xh100_bringup_report, DenseRankRuntimeError,
    DenseRankRuntimeExecutionReceipt, ParameterGolfDistributed8xH100BringupConfig,
    ParameterGolfDistributed8xH100BringupError, ParameterGolfDistributed8xH100BringupReport,
    ParameterGolfDistributed8xH100RuntimeBootstrapError,
    ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt,
    ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    ParameterGolfDistributed8xH100TrainStepError,
    ParameterGolfDistributed8xH100TrainStepRankReceipt,
    ParameterGolfDistributed8xH100TrainStepReceipt,
    ParameterGolfDistributed8xH100ValidationRankReceipt,
    ParameterGolfDistributedLiveVisualizationWriter, ParameterGolfDistributedVisualizationError,
    ParameterGolfDistributedVisualizationMetadata, ParameterGolfLocalReferenceFixture,
    ParameterGolfNonRecordSubmissionManifest, ParameterGolfScoreFirstTttConfig,
    ParameterGolfSubmissionAccountingReceipt, ParameterGolfSubmissionRealExecutionContract,
    ParameterGolfValidationEvalMode, RemoteTrainingResultClassification,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE, PARAMETER_GOLF_EXECUTION_MODE_ENV_VAR,
};

/// Machine-readable runtime manifest shipped with the submission folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionRuntimeManifest {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable package version.
    pub package_version: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Canonical benchmark reference.
    pub benchmark_ref: String,
    /// Top-level entrypoint path.
    pub entrypoint_path: String,
    /// Shipped runtime payload path.
    pub runtime_payload_path: String,
    /// Top-level `submission.json` path.
    pub submission_manifest_path: String,
    /// Shipped accounting receipt path.
    pub accounting_receipt_path: String,
    /// Shipped local-reference fixture path.
    pub fixture_path: String,
    /// Shipped counted model artifact path.
    pub model_artifact_path: String,
    /// Runtime receipt path written by the payload.
    pub runtime_receipt_path: String,
    /// Distributed bring-up report path written by the shipped payload in `distributed_8xh100_train` mode.
    pub distributed_bringup_report_path: String,
    /// Sequence length used by the bounded local-reference eval replay.
    pub sequence_length: usize,
    /// Validation batch tokens used by the bounded local-reference eval replay.
    pub validation_batch_tokens: usize,
    /// Requested base validation-eval mode for the shipped runtime.
    #[serde(default)]
    pub validation_eval_mode: ParameterGolfValidationEvalMode,
    /// Explicit validation batch geometry for the shipped runtime.
    #[serde(default)]
    pub validation_batch_sequences: u64,
    /// Optional legal score-first TTT overlay requested by the shipped runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_first_ttt: Option<ParameterGolfScoreFirstTttConfig>,
    /// Expected final roundtrip validation loss from `submission.json`.
    pub expected_val_loss: f64,
    /// Expected final roundtrip validation bits-per-byte from `submission.json`.
    pub expected_val_bpb: f64,
    /// Default exported-folder execution mode.
    #[serde(default = "default_local_reference_execution_mode")]
    pub default_execution_mode: String,
    /// Additional explicit real execution contracts shipped with the folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub real_execution_contracts: Vec<ParameterGolfSubmissionRealExecutionContract>,
    /// Explicit runtime posture for the package.
    pub runtime_posture: String,
    /// Explicit claim boundary for the runtime.
    pub claim_boundary: String,
    /// Stable digest over the manifest.
    pub manifest_digest: String,
}

impl ParameterGolfSubmissionRuntimeManifest {
    /// Returns a stable digest over the manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.manifest_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_submission_runtime_manifest|",
            &digestible,
        )
    }

    fn execution_contract(
        &self,
        execution_mode: &str,
    ) -> Option<&ParameterGolfSubmissionRealExecutionContract> {
        self.real_execution_contracts
            .iter()
            .find(|contract| contract.execution_mode == execution_mode)
    }

    fn validation_eval_mode_for_execution_mode(
        &self,
        execution_mode: &str,
    ) -> &ParameterGolfValidationEvalMode {
        self.execution_contract(execution_mode)
            .and_then(|contract| contract.validation_eval_mode.as_ref())
            .unwrap_or(&self.validation_eval_mode)
    }

    fn validation_batch_sequences_for_execution_mode(&self, execution_mode: &str) -> u64 {
        self.execution_contract(execution_mode)
            .and_then(|contract| contract.validation_batch_sequences)
            .unwrap_or(self.validation_batch_sequences)
    }
}

/// Runtime result emitted by the shipped submission payload.
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterGolfSubmissionRuntimeOutcome {
    /// Bounded local-reference restore-and-eval replay.
    LocalReference(ParameterGolfSubmissionRuntimeReceipt),
    /// Rust-owned distributed `8xH100` bring-up report.
    Distributed8xH100Bringup {
        report_path: String,
        report: ParameterGolfDistributed8xH100BringupReport,
    },
    /// Rust-owned distributed `8xH100` runtime bootstrap receipt above the bring-up gate.
    Distributed8xH100Bootstrap {
        report_path: String,
        report: ParameterGolfDistributed8xH100BringupReport,
        receipt_path: String,
        receipt: ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    },
    /// Rust-owned distributed `8xH100` train-step receipt above the bootstrap gate.
    Distributed8xH100TrainStep {
        report_path: String,
        report: ParameterGolfDistributed8xH100BringupReport,
        bootstrap_receipt_path: String,
        bootstrap_receipt: ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
        train_step_receipt_path: String,
        train_step_receipt: ParameterGolfDistributed8xH100TrainStepReceipt,
        dense_rank_execution_receipt_path: String,
        dense_rank_execution_receipt: DenseRankRuntimeExecutionReceipt,
    },
    /// Rust-owned distributed `8xH100` completion receipt above validation-backed execution.
    Distributed8xH100Completed {
        receipt_path: String,
        receipt: ParameterGolfDistributed8xH100CompletionReceipt,
    },
    /// Internal child-rank bootstrap receipt used by the shipped runtime fanout.
    Distributed8xH100BootstrapChild {
        receipt_path: String,
        receipt: ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt,
    },
    /// Internal child-rank train-step receipt used by the shipped runtime fanout.
    Distributed8xH100TrainStepChild {
        receipt_path: String,
        receipt: ParameterGolfDistributed8xH100TrainStepRankReceipt,
    },
    /// Internal child-rank validation receipt used by the shipped runtime fanout.
    Distributed8xH100ValidationChild {
        receipt_path: String,
        receipt: ParameterGolfDistributed8xH100ValidationRankReceipt,
    },
}

fn default_local_reference_execution_mode() -> String {
    String::from("local_reference_validation")
}

/// Completion receipt emitted when the shipped distributed runtime finishes the
/// current exported-folder execution contract without the old refusal posture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfDistributed8xH100CompletionReceipt {
    pub schema_version: u32,
    pub run_id: String,
    pub bringup_report_path: String,
    pub bringup_report_digest: String,
    pub runtime_bootstrap_receipt_path: String,
    pub runtime_bootstrap_receipt_digest: String,
    pub train_step_receipt_path: String,
    pub train_step_receipt_digest: String,
    pub distributed_receipt_path: String,
    pub distributed_receipt_digest: String,
    pub final_model_artifact_path: String,
    pub final_model_artifact_digest: String,
    pub final_model_artifact_size_bytes: u64,
    pub submission_manifest_path: String,
    pub submission_manifest_digest: String,
    pub distributed_validation_mean_loss: f64,
    pub distributed_validation_bits_per_byte: f64,
    pub distributed_validation_observed_ms: u64,
    pub claim_boundary: String,
    pub receipt_digest: String,
}

impl ParameterGolfDistributed8xH100CompletionReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_distributed_8xh100_completion_receipt|",
            &digestible,
        )
    }
}

/// Runtime receipt emitted by the shipped submission payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionRuntimeReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable run identifier.
    pub run_id: String,
    /// Runtime posture executed by the payload.
    pub runtime_posture: String,
    /// Top-level entrypoint path.
    pub entrypoint_path: String,
    /// Runtime payload path.
    pub runtime_payload_path: String,
    /// Top-level `submission.json` path.
    pub submission_manifest_path: String,
    /// Accounting receipt path.
    pub accounting_receipt_path: String,
    /// Fixture path used for the eval replay.
    pub fixture_path: String,
    /// Counted model artifact path.
    pub model_artifact_path: String,
    /// Executed validation report from the shipped model artifact.
    pub executed_validation: ParameterGolfValidationEvalReport,
    /// Whether the executed validation loss matched `submission.json`.
    pub matches_submission_val_loss: bool,
    /// Whether the executed validation bits-per-byte matched `submission.json`.
    pub matches_submission_val_bpb: bool,
    /// Whether the counted-code bytes in `submission.json` matched the shipped accounting receipt.
    pub matches_accounting_code_bytes: bool,
    /// Whether the total counted bytes in `submission.json` matched the shipped accounting receipt.
    pub matches_accounting_total_bytes: bool,
    /// Whether the counted model size in `submission.json` matched the shipped artifact bytes.
    pub matches_submission_model_bytes: bool,
    /// Whether the counted model size in the accounting receipt matched the shipped artifact bytes.
    pub matches_accounting_model_bytes: bool,
    /// Stable digest over the receipt payload.
    pub receipt_digest: String,
}

impl ParameterGolfSubmissionRuntimeReceipt {
    fn new(
        manifest: &ParameterGolfSubmissionRuntimeManifest,
        executed_validation: ParameterGolfValidationEvalReport,
        submission_manifest: &ParameterGolfNonRecordSubmissionManifest,
        accounting_receipt: &ParameterGolfSubmissionAccountingReceipt,
        model_artifact_size_bytes: u64,
    ) -> Self {
        let mut receipt = Self {
            schema_version: 1,
            run_id: manifest.run_id.clone(),
            runtime_posture: manifest.runtime_posture.clone(),
            entrypoint_path: manifest.entrypoint_path.clone(),
            runtime_payload_path: manifest.runtime_payload_path.clone(),
            submission_manifest_path: manifest.submission_manifest_path.clone(),
            accounting_receipt_path: manifest.accounting_receipt_path.clone(),
            fixture_path: manifest.fixture_path.clone(),
            model_artifact_path: manifest.model_artifact_path.clone(),
            matches_submission_val_loss: metric_matches(
                executed_validation.mean_loss,
                submission_manifest.val_loss,
            ),
            matches_submission_val_bpb: metric_matches(
                executed_validation.bits_per_byte,
                submission_manifest.val_bpb,
            ),
            matches_accounting_code_bytes: submission_manifest.bytes_code
                == accounting_receipt.counted_code_bytes,
            matches_accounting_total_bytes: submission_manifest.bytes_total
                == accounting_receipt.total_counted_bytes,
            matches_submission_model_bytes: submission_manifest.bytes_model_int8_zlib
                == model_artifact_size_bytes,
            matches_accounting_model_bytes: accounting_receipt.compressed_model_bytes
                == model_artifact_size_bytes,
            executed_validation,
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
            b"psionic_parameter_golf_submission_runtime_receipt|",
            &digestible,
        )
    }

    /// Returns whether the runtime result is consistent with the shipped manifest and accounting.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.matches_submission_val_loss
            && self.matches_submission_val_bpb
            && self.matches_accounting_code_bytes
            && self.matches_accounting_total_bytes
            && self.matches_submission_model_bytes
            && self.matches_accounting_model_bytes
    }
}

/// Failure while executing the shipped submission runtime.
#[derive(Debug, Error)]
pub enum ParameterGolfSubmissionRuntimeError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("parameter golf submission runtime consistency error: {message}")]
    Consistency { message: String },
    #[error("parameter golf submission runtime execution-mode error: {message}")]
    ExecutionMode { message: String },
    #[error(
        "parameter golf submission runtime does not support validation_eval_mode={validation_eval_mode} score_first_ttt={score_first_ttt:?} on posture `{runtime_posture}`"
    )]
    UnsupportedValidationMode {
        runtime_posture: String,
        validation_eval_mode: String,
        score_first_ttt: Option<String>,
    },
    #[error(transparent)]
    ReferenceTraining(#[from] crate::ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Eval(#[from] psionic_eval::ParameterGolfValidationEvalError),
    #[error(transparent)]
    Model(#[from] psionic_models::ParameterGolfModelError),
    #[error(transparent)]
    Execution(#[from] psionic_models::ParameterGolfExecutionError),
    #[error(transparent)]
    Data(#[from] psionic_data::ParameterGolfDataError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    DistributedBringup(#[from] ParameterGolfDistributed8xH100BringupError),
    #[error(transparent)]
    DistributedRuntimeBootstrap(#[from] ParameterGolfDistributed8xH100RuntimeBootstrapError),
    #[error(transparent)]
    DistributedTrainStep(#[from] ParameterGolfDistributed8xH100TrainStepError),
    #[error(transparent)]
    DenseRankRuntime(#[from] DenseRankRuntimeError),
    #[error(transparent)]
    Visualization(#[from] ParameterGolfDistributedVisualizationError),
}

/// Executes the shipped submission runtime manifest, writes the runtime receipt, and returns it.
pub fn execute_parameter_golf_submission_runtime_manifest(
    root: &Path,
    manifest_path: &Path,
) -> Result<ParameterGolfSubmissionRuntimeReceipt, ParameterGolfSubmissionRuntimeError> {
    let manifest = read_json::<ParameterGolfSubmissionRuntimeManifest>(
        manifest_path,
        "parameter_golf_submission_runtime_manifest",
    )?;
    let receipt = execute_parameter_golf_submission_runtime(root, &manifest)?;
    let receipt_path = root.join(&manifest.runtime_receipt_path);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfSubmissionRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let encoded = serde_json::to_string_pretty(&receipt)?;
    fs::write(&receipt_path, format!("{encoded}\n")).map_err(|error| {
        ParameterGolfSubmissionRuntimeError::Write {
            path: receipt_path.display().to_string(),
            error,
        }
    })?;
    Ok(receipt)
}

/// Executes the shipped submission runtime entrypoint for the current execution mode.
pub fn execute_parameter_golf_submission_runtime_entrypoint(
    root: &Path,
    manifest_path: &Path,
) -> Result<ParameterGolfSubmissionRuntimeOutcome, ParameterGolfSubmissionRuntimeError> {
    let manifest = read_json::<ParameterGolfSubmissionRuntimeManifest>(
        manifest_path,
        "parameter_golf_submission_runtime_manifest",
    )?;
    let execution_mode = env::var(PARAMETER_GOLF_EXECUTION_MODE_ENV_VAR)
        .unwrap_or(manifest.default_execution_mode.clone());
    let resolved_validation_eval_mode = manifest
        .validation_eval_mode_for_execution_mode(execution_mode.as_str())
        .clone();
    let resolved_validation_batch_sequences =
        manifest.validation_batch_sequences_for_execution_mode(execution_mode.as_str());
    match execution_mode.as_str() {
        "local_reference_validation" => Ok(ParameterGolfSubmissionRuntimeOutcome::LocalReference(
            execute_parameter_golf_submission_runtime_manifest(root, manifest_path)?,
        )),
        PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE => {
            if parameter_golf_distributed_8xh100_validation_child_enabled() {
                let receipt =
                    execute_parameter_golf_submission_distributed_8xh100_validation_child(
                        &manifest,
                    )?;
                return Ok(
                    ParameterGolfSubmissionRuntimeOutcome::Distributed8xH100ValidationChild {
                        receipt_path: env::var(
                            "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_VALIDATION_RECEIPT_PATH",
                        )
                        .unwrap_or_default(),
                        receipt,
                    },
                );
            }
            if parameter_golf_distributed_8xh100_train_step_child_enabled() {
                let receipt =
                    execute_parameter_golf_submission_distributed_8xh100_train_step_child(
                        &manifest,
                    )?;
                return Ok(
                    ParameterGolfSubmissionRuntimeOutcome::Distributed8xH100TrainStepChild {
                        receipt_path: env::var(
                            "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_TRAIN_STEP_RECEIPT_PATH",
                        )
                        .unwrap_or_default(),
                        receipt,
                    },
                );
            }
            if parameter_golf_distributed_8xh100_runtime_bootstrap_child_enabled() {
                let receipt = execute_parameter_golf_submission_distributed_8xh100_bootstrap_child(
                    &manifest,
                )?;
                return Ok(
                    ParameterGolfSubmissionRuntimeOutcome::Distributed8xH100BootstrapChild {
                        receipt_path: env::var(
                            "PSIONIC_PARAMETER_GOLF_DISTRIBUTED_8XH100_BOOTSTRAP_RECEIPT_PATH",
                        )
                        .unwrap_or_default(),
                        receipt,
                    },
                );
            }
            execute_parameter_golf_submission_distributed_8xh100_bootstrap(
                root,
                manifest_path,
                &manifest,
                &resolved_validation_eval_mode,
                resolved_validation_batch_sequences,
            )
        }
        other => Err(ParameterGolfSubmissionRuntimeError::ExecutionMode {
            message: format!("unsupported execution mode `{other}`"),
        }),
    }
}

/// Executes the actual shipped restore-and-eval path described by the runtime manifest.
pub fn execute_parameter_golf_submission_runtime(
    root: &Path,
    manifest: &ParameterGolfSubmissionRuntimeManifest,
) -> Result<ParameterGolfSubmissionRuntimeReceipt, ParameterGolfSubmissionRuntimeError> {
    if manifest.score_first_ttt.is_some() {
        return Err(
            ParameterGolfSubmissionRuntimeError::UnsupportedValidationMode {
                runtime_posture: manifest.runtime_posture.clone(),
                validation_eval_mode: manifest.validation_eval_mode.as_str().to_string(),
                score_first_ttt: manifest
                    .score_first_ttt
                    .as_ref()
                    .map(|config| config.label().to_string()),
            },
        );
    }
    let submission_manifest = read_json::<ParameterGolfNonRecordSubmissionManifest>(
        root.join(&manifest.submission_manifest_path),
        "parameter_golf_submission_manifest",
    )?;
    let accounting_receipt = read_json::<ParameterGolfSubmissionAccountingReceipt>(
        root.join(&manifest.accounting_receipt_path),
        "parameter_golf_submission_accounting_receipt",
    )?;
    let fixture = read_json::<ParameterGolfLocalReferenceFixture>(
        root.join(&manifest.fixture_path),
        "parameter_golf_local_reference_fixture",
    )?;
    let model_artifact_path = root.join(&manifest.model_artifact_path);
    let model_artifact_bytes = fs::read(&model_artifact_path).map_err(|error| {
        ParameterGolfSubmissionRuntimeError::Read {
            path: model_artifact_path.display().to_string(),
            error,
        }
    })?;
    let baseline_model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
    let restored_model = restore_parameter_golf_model_from_int8_zlib(
        &baseline_model,
        model_artifact_bytes.as_slice(),
    )?;
    let executed_validation = evaluate_parameter_golf_validation(
        &restored_model,
        fixture.validation_tokens.as_slice(),
        manifest.sequence_length,
        manifest.validation_batch_tokens,
        &fixture.byte_luts()?,
    )?;
    let receipt = ParameterGolfSubmissionRuntimeReceipt::new(
        manifest,
        executed_validation,
        &submission_manifest,
        &accounting_receipt,
        model_artifact_bytes.len() as u64,
    );
    if !receipt.is_consistent() {
        return Err(ParameterGolfSubmissionRuntimeError::Consistency {
            message: String::from(
                "executed runtime receipt did not match submission.json or the shipped accounting receipt",
            ),
        });
    }
    Ok(receipt)
}

fn execute_parameter_golf_submission_distributed_8xh100_bootstrap(
    root: &Path,
    manifest_path: &Path,
    manifest: &ParameterGolfSubmissionRuntimeManifest,
    validation_eval_mode: &ParameterGolfValidationEvalMode,
    validation_batch_sequences: u64,
) -> Result<ParameterGolfSubmissionRuntimeOutcome, ParameterGolfSubmissionRuntimeError> {
    let output_path = root.join(&manifest.distributed_bringup_report_path);
    let config = ParameterGolfDistributed8xH100BringupConfig::challenge_defaults();
    let report =
        write_parameter_golf_distributed_8xh100_bringup_report(&output_path, &config, None)?;
    if !report.ready_to_attempt() {
        return Ok(
            ParameterGolfSubmissionRuntimeOutcome::Distributed8xH100Bringup {
                report_path: output_path.display().to_string(),
                report,
            },
        );
    }
    let mut live_visualization_writer = ParameterGolfDistributedLiveVisualizationWriter::start(
        ParameterGolfDistributedVisualizationMetadata::new_for_runpod_runtime(
            root,
            &output_path,
            manifest_path,
        ),
        &manifest.run_id,
        "The RunPod 8xH100 distributed runtime started and is emitting one-second live visualization snapshots.",
    )?;
    let runtime = execute_parameter_golf_backed_dense_rank_runtime(
        root,
        manifest_path,
        &manifest.run_id,
        &output_path,
        &report,
        validation_eval_mode,
        validation_batch_sequences,
        manifest.score_first_ttt.as_ref(),
        Some(&mut live_visualization_writer),
    )?;
    let bootstrap_receipt_path = PathBuf::from(&runtime.bootstrap_receipt_path);
    let train_step_receipt_path = PathBuf::from(&runtime.train_step_receipt_path);
    if runtime
        .train_step_receipt
        .distributed_receipt
        .validation_aggregation
        .is_none()
    {
        live_visualization_writer.finish(
            RemoteTrainingResultClassification::CompletedFailure,
            "The RunPod 8xH100 runtime stopped after the retained train-step proof without one final validation-backed distributed completion receipt.",
        )?;
        return Ok(
            ParameterGolfSubmissionRuntimeOutcome::Distributed8xH100TrainStep {
                report_path: output_path.display().to_string(),
                report,
                bootstrap_receipt_path: runtime.bootstrap_receipt_path,
                bootstrap_receipt: runtime.bootstrap_receipt,
                train_step_receipt_path: runtime.train_step_receipt_path,
                train_step_receipt: runtime.train_step_receipt,
                dense_rank_execution_receipt_path: runtime.dense_rank_execution_receipt_path,
                dense_rank_execution_receipt: runtime.dense_rank_execution_receipt,
            },
        );
    }
    let completion_receipt_path = parameter_golf_distributed_8xh100_completion_receipt_path(
        root,
        &manifest.distributed_bringup_report_path,
    );
    let completion_receipt = build_parameter_golf_distributed_8xh100_completion_receipt(
        root,
        manifest,
        &output_path,
        &report,
        &bootstrap_receipt_path,
        &runtime.bootstrap_receipt,
        &train_step_receipt_path,
        &runtime.train_step_receipt,
    )?;
    if let Some(parent) = completion_receipt_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfSubmissionRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(
        &completion_receipt_path,
        format!("{}\n", serde_json::to_string_pretty(&completion_receipt)?),
    )
    .map_err(|error| ParameterGolfSubmissionRuntimeError::Write {
        path: completion_receipt_path.display().to_string(),
        error,
    })?;
    live_visualization_writer.finish(
        RemoteTrainingResultClassification::CompletedSuccess,
        "The RunPod 8xH100 runtime sealed its validation-backed distributed completion receipt and retained the provider-neutral live training series.",
    )?;
    Ok(
        ParameterGolfSubmissionRuntimeOutcome::Distributed8xH100Completed {
            receipt_path: completion_receipt_path.display().to_string(),
            receipt: completion_receipt,
        },
    )
}

fn parameter_golf_distributed_8xh100_completion_receipt_path(
    root: &Path,
    bringup_report_path: &str,
) -> PathBuf {
    let resolved = root.join(bringup_report_path);
    match resolved.parent() {
        Some(parent) => parent.join("parameter_golf_distributed_8xh100_completion.json"),
        None => root.join("parameter_golf_distributed_8xh100_completion.json"),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_parameter_golf_distributed_8xh100_completion_receipt(
    root: &Path,
    manifest: &ParameterGolfSubmissionRuntimeManifest,
    bringup_report_path: &Path,
    bringup_report: &ParameterGolfDistributed8xH100BringupReport,
    bootstrap_receipt_path: &Path,
    bootstrap_receipt: &ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    train_step_receipt_path: &Path,
    train_step_receipt: &ParameterGolfDistributed8xH100TrainStepReceipt,
) -> Result<ParameterGolfDistributed8xH100CompletionReceipt, ParameterGolfSubmissionRuntimeError> {
    let validation = train_step_receipt
        .distributed_receipt
        .validation_aggregation
        .as_ref()
        .ok_or_else(|| ParameterGolfSubmissionRuntimeError::Consistency {
            message: String::from(
                "distributed completion receipt requires one validation-backed distributed receipt",
            ),
        })?;
    let final_model_artifact_path =
        PathBuf::from(&train_step_receipt.current_model_int8_zlib_artifact_path);
    let final_model_artifact_bytes = fs::read(&final_model_artifact_path).map_err(|error| {
        ParameterGolfSubmissionRuntimeError::Read {
            path: final_model_artifact_path.display().to_string(),
            error,
        }
    })?;
    let submission_manifest_path = root.join(&manifest.submission_manifest_path);
    let submission_manifest_bytes = fs::read(&submission_manifest_path).map_err(|error| {
        ParameterGolfSubmissionRuntimeError::Read {
            path: submission_manifest_path.display().to_string(),
            error,
        }
    })?;
    let mut receipt = ParameterGolfDistributed8xH100CompletionReceipt {
        schema_version: 1,
        run_id: manifest.run_id.clone(),
        bringup_report_path: bringup_report_path.display().to_string(),
        bringup_report_digest: bringup_report.report_digest.clone(),
        runtime_bootstrap_receipt_path: bootstrap_receipt_path.display().to_string(),
        runtime_bootstrap_receipt_digest: bootstrap_receipt.receipt_digest.clone(),
        train_step_receipt_path: train_step_receipt_path.display().to_string(),
        train_step_receipt_digest: train_step_receipt.receipt_digest.clone(),
        distributed_receipt_path: train_step_receipt.distributed_receipt_path.clone(),
        distributed_receipt_digest: train_step_receipt.distributed_receipt.receipt_digest.clone(),
        final_model_artifact_path: final_model_artifact_path.display().to_string(),
        final_model_artifact_digest: sha256_bytes(final_model_artifact_bytes.as_slice()),
        final_model_artifact_size_bytes: final_model_artifact_bytes.len() as u64,
        submission_manifest_path: submission_manifest_path.display().to_string(),
        submission_manifest_digest: sha256_bytes(submission_manifest_bytes.as_slice()),
        distributed_validation_mean_loss: validation.mean_loss,
        distributed_validation_bits_per_byte: validation.bits_per_byte,
        distributed_validation_observed_ms: validation.observed_ms,
        claim_boundary: String::from(
            "This completion receipt proves the exported-folder distributed runtime finished the current 8xH100 execution contract with one measured distributed validation aggregation and bound that execution to the exact trained post-step int8+zlib artifact produced by the live runtime. It does not by itself claim record-track promotion.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    Ok(receipt)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn execute_parameter_golf_submission_distributed_8xh100_bootstrap_child(
    manifest: &ParameterGolfSubmissionRuntimeManifest,
) -> Result<
    ParameterGolfDistributed8xH100RuntimeBootstrapRankReceipt,
    ParameterGolfSubmissionRuntimeError,
> {
    Ok(execute_parameter_golf_distributed_8xh100_runtime_bootstrap_child(&manifest.run_id)?)
}

fn execute_parameter_golf_submission_distributed_8xh100_train_step_child(
    manifest: &ParameterGolfSubmissionRuntimeManifest,
) -> Result<ParameterGolfDistributed8xH100TrainStepRankReceipt, ParameterGolfSubmissionRuntimeError>
{
    Ok(execute_parameter_golf_distributed_8xh100_train_step_child(
        &manifest.run_id,
    )?)
}

fn execute_parameter_golf_submission_distributed_8xh100_validation_child(
    manifest: &ParameterGolfSubmissionRuntimeManifest,
) -> Result<ParameterGolfDistributed8xH100ValidationRankReceipt, ParameterGolfSubmissionRuntimeError>
{
    Ok(execute_parameter_golf_distributed_8xh100_validation_child(
        &manifest.run_id,
    )?)
}

fn metric_matches(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-9
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: impl AsRef<Path>,
    artifact_kind: &'static str,
) -> Result<T, ParameterGolfSubmissionRuntimeError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| ParameterGolfSubmissionRuntimeError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ParameterGolfSubmissionRuntimeError::Deserialize {
            artifact_kind: String::from(artifact_kind),
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

/// True when the committed `x86_64-unknown-linux-gnu` runtime payload can execute on this host.
///
/// The non-record submission package intentionally ships one committed Linux
/// x86_64 payload (the challenge replay host target). Hosts with any other
/// architecture or OS cannot exec that payload, so payload-executing tests
/// skip with an explicit printed reason instead of failing on
/// `Exec format error`.
#[must_use]
pub fn parameter_golf_submission_runtime_payload_matches_host() -> bool {
    cfg!(all(target_os = "linux", target_arch = "x86_64"))
}

/// Returns the committed runtime payload path used by the current non-record package.
#[must_use]
pub fn parameter_golf_submission_runtime_payload_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
        .join("fixtures/parameter_golf/runtime/parameter_golf_submission_runtime.x86_64-unknown-linux-gnu")
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs, path::Path};

    use psionic_eval::{
        ParameterGolfDistributedChallengeThresholds, ParameterGolfDistributedCommunicationReceipt,
        ParameterGolfDistributedCommunicationStageReceipt, ParameterGolfDistributedLaneDisposition,
        ParameterGolfDistributedThroughputReceipt, ParameterGolfDistributedTimingReceipt,
        ParameterGolfDistributedTopologyReceipt,
        ParameterGolfDistributedValidationAggregationReceipt,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY,
    };
    use psionic_runtime::{
        BackendSelection, ClusterCommunicationClass, ClusterTransportClass, HealthStatus,
        RuntimeHealth, TrainingCollectiveKind, TrainingCollectiveQuantization,
        TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind,
    };
    use tempfile::tempdir;

    use crate::{
        ParameterGolfBatchGeometry, ParameterGolfDistributed8xH100BringupDisposition,
        ParameterGolfDistributed8xH100BringupExecutionPosture,
        ParameterGolfDistributed8xH100BringupReport,
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition,
        ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
        ParameterGolfDistributed8xH100RuntimeRequestedBackend,
        ParameterGolfDistributed8xH100TrainStepReceipt, ParameterGolfDistributedStepObservation,
        ParameterGolfTrainingHyperparameters,
    };

    use super::*;

    fn sample_distributed_receipt() -> ParameterGolfDistributedThroughputReceipt {
        ParameterGolfDistributedThroughputReceipt {
            benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
            run_id: String::from("parameter-golf-distributed-runtime-test"),
            model_descriptor_digest: String::from("model-digest"),
            optimizer_plan_digest: String::from("optimizer-digest"),
            thresholds: ParameterGolfDistributedChallengeThresholds::challenge_8xh100(),
            topology: ParameterGolfDistributedTopologyReceipt {
                backend_selection: BackendSelection::direct(
                    "cuda",
                    None,
                    vec![String::from("parameter_golf_distributed_train")],
                ),
                topology_digest: String::from("topology-digest"),
                selected_device_names: vec![String::from("NVIDIA H100 80GB HBM3"); 8],
                all_devices_match_required_model: true,
            },
            communication: ParameterGolfDistributedCommunicationReceipt {
                communication_class: ClusterCommunicationClass::TensorCollectiveMesh,
                transport: ClusterTransportClass::Loopback,
                mesh_id: String::from("mesh.parameter_golf.8xh100"),
                axes: vec![TrainingDeviceMeshAxis::new(
                    "dp",
                    TrainingDeviceMeshAxisKind::DataParallel,
                    8,
                )
                .with_collective_group_size(8)],
                stages: vec![ParameterGolfDistributedCommunicationStageReceipt {
                    stage_id: String::from("ddp_gradient_all_reduce"),
                    collective_kind: TrainingCollectiveKind::AllReduce,
                    quantization: TrainingCollectiveQuantization::None,
                    payload_bytes: 1024,
                    estimated_wire_bytes: 2048,
                    worker_count: 8,
                    detail: String::from("DDP gradient synchronization"),
                }],
            },
            training_capability_report_digest: String::from("coverage-digest"),
            challenge_kernel_blockers: Vec::new(),
            disposition: ParameterGolfDistributedLaneDisposition::Measured,
            timing: Some(ParameterGolfDistributedTimingReceipt {
                measurement_posture: String::from("observed_step_wallclock"),
                step_count: 4,
                total_train_tokens: 1_048_576,
                training_step_observed_ms: 400,
                validation_observed_ms: 20,
                export_observed_ms: 10,
                total_observed_ms: 430,
                mean_step_duration_ms: 100,
                tail_step_duration_ms: 112,
                train_tokens_per_second: 2_621_440,
                wallclock_cap_ms: 600_000,
                within_wallclock_cap: true,
            }),
            validation_aggregation: Some(ParameterGolfDistributedValidationAggregationReceipt {
                measurement_posture: String::from("distributed_validation"),
                eval_mode: String::from("non_overlapping"),
                world_size: 8,
                total_sequence_count: 1024,
                total_evaluation_unit_count: 1024,
                local_batch_sequences: 128,
                aggregated_loss_sum: 3_814.4,
                aggregated_token_count: 65_536,
                aggregated_byte_count: 131_072,
                mean_loss: 3.82,
                bits_per_byte: 1.91,
                observed_ms: 20,
                shards: Vec::new(),
            }),
            memory: None,
            refusal: None,
            boundary_notes: vec![String::from("boundary")],
            claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
            receipt_digest: String::new(),
        }
        .with_stable_digest()
    }

    fn sample_bringup_report() -> ParameterGolfDistributed8xH100BringupReport {
        let mut report = ParameterGolfDistributed8xH100BringupReport {
            schema_version: 1,
            run_id: String::from("parameter-golf-distributed-runtime-test"),
            geometry: ParameterGolfBatchGeometry::challenge_distributed_8xh100_defaults(),
            hyperparameters: ParameterGolfTrainingHyperparameters::baseline_defaults(),
            machine_thresholds: ParameterGolfDistributedChallengeThresholds::challenge_8xh100(),
            observed_cuda_health: RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("cuda online"),
            },
            cuda_discovery_error: None,
            observed_cuda_devices: Vec::new(),
            matching_h100_device_count: 8,
            machine_contract_satisfied: true,
            psionic_entrypoint: String::from("train_gpt.py"),
            execution_posture:
                ParameterGolfDistributed8xH100BringupExecutionPosture::ContractValidationOnly,
            distributed_receipt: None,
            disposition: ParameterGolfDistributed8xH100BringupDisposition::ContractReady,
            refusal: None,
            drift_notes: Vec::new(),
            claim_boundary: String::from("bringup boundary"),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_parameter_golf_distributed_8xh100_bringup_report|",
            &{
                let mut digestible = report.clone();
                digestible.report_digest.clear();
                digestible
            },
        );
        report
    }

    fn sample_runtime_bootstrap_receipt(
        bringup_report_path: &Path,
        bringup_report: &ParameterGolfDistributed8xH100BringupReport,
    ) -> ParameterGolfDistributed8xH100RuntimeBootstrapReceipt {
        let mut receipt = ParameterGolfDistributed8xH100RuntimeBootstrapReceipt {
            schema_version: 1,
            run_id: String::from("parameter-golf-distributed-runtime-test"),
            requested_backend: ParameterGolfDistributed8xH100RuntimeRequestedBackend::Nccl,
            world_size: 8,
            bringup_report_path: bringup_report_path.display().to_string(),
            bringup_report_digest: bringup_report.report_digest.clone(),
            runtime_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
            runtime_manifest_path: String::from("runtime/parameter_golf_submission_runtime.json"),
            rank_launches: Vec::new(),
            successful_rank_count: 8,
            disposition: ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::Bootstrapped,
            refusal: None,
            drift_notes: Vec::new(),
            claim_boundary: String::from("bootstrap boundary"),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.stable_digest();
        receipt
    }

    fn sample_train_step_receipt(
        root: &Path,
        distributed_receipt: ParameterGolfDistributedThroughputReceipt,
    ) -> Result<ParameterGolfDistributed8xH100TrainStepReceipt, Box<dyn Error>> {
        let benchmark_root = root.join("parameter-golf-distributed-8xh100-run/benchmark");
        fs::create_dir_all(&benchmark_root)?;
        let final_model_path = benchmark_root.join("current_model.int8.zlib");
        fs::write(&final_model_path, b"model-int8")?;
        let mut receipt = ParameterGolfDistributed8xH100TrainStepReceipt {
            schema_version: 1,
            run_id: String::from("parameter-golf-distributed-runtime-test"),
            world_size: 8,
            bringup_report_path: benchmark_root
                .join("parameter_golf_distributed_8xh100_bringup.json")
                .display()
                .to_string(),
            bringup_report_digest: String::from("bringup-digest"),
            runtime_bootstrap_receipt_path: benchmark_root
                .join("parameter_golf_distributed_8xh100_runtime_bootstrap.json")
                .display()
                .to_string(),
            runtime_bootstrap_receipt_digest: String::from("bootstrap-digest"),
            runtime_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
            runtime_manifest_path: String::from("runtime/parameter_golf_submission_runtime.json"),
            measurements_path: benchmark_root
                .join("parameter_golf_distributed_8xh100_measurements.json")
                .display()
                .to_string(),
            distributed_receipt_path: benchmark_root
                .join("parameter_golf_distributed_8xh100_receipt.json")
                .display()
                .to_string(),
            train_step_receipt_path: benchmark_root
                .join("parameter_golf_distributed_8xh100_train_step.json")
                .display()
                .to_string(),
            step_scope_root_dir: benchmark_root
                .join("runtime_step_scopes")
                .display()
                .to_string(),
            executed_step_count: 4,
            observed_training_time_ms: 400,
            step_observations: vec![ParameterGolfDistributedStepObservation::new(
                1,
                1_742_846_401_000,
                1_742_846_401_400,
                1_048_576,
            )],
            stop_reason: Some(String::from("validation_completed")),
            mean_train_loss: 4.12,
            train_tokens: 1_048_576,
            observed_step_ms: 400,
            gradient_sync_ms: 28,
            optimizer_step_ms: 12,
            gradient_norm_after_clip: 0.91,
            clip_applied: true,
            non_finite_gradient_count: 0,
            rank_launches: Vec::new(),
            aggregated_gradient_artifact_path: benchmark_root
                .join("aggregated_gradients.safetensors")
                .display()
                .to_string(),
            aggregated_gradient_artifact_sha256: String::from("aggregated-gradient-sha"),
            current_model_artifact_path: benchmark_root
                .join("current_model.runtime_surface.safetensors")
                .display()
                .to_string(),
            current_model_artifact_sha256: String::from("runtime-surface-sha"),
            current_model_artifact_surface: String::from("banked_full_precision_v1"),
            current_model_int8_zlib_artifact_path: final_model_path.display().to_string(),
            current_model_int8_zlib_artifact_sha256: sha256_bytes(b"model-int8"),
            current_model_int8_zlib_artifact_size_bytes: b"model-int8".len() as u64,
            validation_rank_launches: Vec::new(),
            step_observation: ParameterGolfDistributedStepObservation::new(
                4,
                1_742_846_401_000,
                1_742_846_401_400,
                1_048_576,
            ),
            validation_observed_ms: 20,
            validation_total_sequence_count: 1024,
            validation_shard_observations: Vec::new(),
            score_first_ttt_receipt: None,
            distributed_receipt,
            claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.stable_digest();
        Ok(receipt)
    }

    #[test]
    fn local_reference_runtime_refuses_score_first_ttt_requests() {
        let manifest = ParameterGolfSubmissionRuntimeManifest {
            schema_version: 1,
            package_version: String::from("test"),
            run_id: String::from("test-run"),
            benchmark_ref: String::from("pgolf:test"),
            entrypoint_path: String::from("train_gpt.py"),
            runtime_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
            submission_manifest_path: String::from("submission.json"),
            accounting_receipt_path: String::from("parameter_golf_submission_accounting.json"),
            fixture_path: String::from("runtime/parameter_golf_local_reference_fixture.json"),
            model_artifact_path: String::from("submission_model.bin.zlib"),
            runtime_receipt_path: String::from("parameter_golf_submission_runtime_receipt.json"),
            distributed_bringup_report_path: String::from(
                "parameter_golf_distributed_8xh100_bringup_report.json",
            ),
            sequence_length: 1024,
            validation_batch_tokens: 16_384,
            validation_eval_mode: ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 },
            validation_batch_sequences: 1024,
            score_first_ttt: Some(ParameterGolfScoreFirstTttConfig::leaderboard_defaults()),
            expected_val_loss: 1.0,
            expected_val_bpb: 1.0,
            default_execution_mode: default_local_reference_execution_mode(),
            real_execution_contracts: Vec::new(),
            runtime_posture: String::from("bounded_local_reference"),
            claim_boundary: String::from("bounded_local_reference_replay_only"),
            manifest_digest: String::new(),
        };

        let error = execute_parameter_golf_submission_runtime(Path::new("."), &manifest)
            .expect_err("score-first TTT should refuse on the bounded local-reference runtime");
        match error {
            ParameterGolfSubmissionRuntimeError::UnsupportedValidationMode {
                runtime_posture,
                validation_eval_mode,
                score_first_ttt,
            } => {
                assert_eq!(runtime_posture, "bounded_local_reference");
                assert_eq!(validation_eval_mode, "sliding_window");
                assert_eq!(score_first_ttt.as_deref(), Some("legal_score_first_ttt"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn execution_mode_specific_validation_contract_overrides_manifest_defaults() {
        let manifest = ParameterGolfSubmissionRuntimeManifest {
            schema_version: 1,
            package_version: String::from("test"),
            run_id: String::from("test-run"),
            benchmark_ref: String::from("pgolf:test"),
            entrypoint_path: String::from("train_gpt.py"),
            runtime_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
            submission_manifest_path: String::from("submission.json"),
            accounting_receipt_path: String::from("parameter_golf_submission_accounting.json"),
            fixture_path: String::from("runtime/parameter_golf_local_reference_fixture.json"),
            model_artifact_path: String::from("submission_model.bin.zlib"),
            runtime_receipt_path: String::from("parameter_golf_submission_runtime_receipt.json"),
            distributed_bringup_report_path: String::from(
                "parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_bringup.json",
            ),
            sequence_length: 1024,
            validation_batch_tokens: 16_384,
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: 256,
            score_first_ttt: None,
            expected_val_loss: 1.0,
            expected_val_bpb: 1.0,
            default_execution_mode: default_local_reference_execution_mode(),
            real_execution_contracts: vec![ParameterGolfSubmissionRealExecutionContract {
                schema_version: 1,
                execution_mode: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE),
                trainer_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
                input_package_descriptor_path: String::from(
                    "runtime/parameter_golf_input_package_descriptor.json",
                ),
                dataset_root_env_var: String::from("PGOLF_DATASET_ROOT"),
                tokenizer_path_env_var: String::from("PGOLF_TOKENIZER_PATH"),
                output_report_env_var: String::from("PGOLF_OUTPUT_REPORT"),
                default_output_report_path: String::from(
                    "parameter_golf_distributed_8xh100_train_step.json",
                ),
                max_steps_env_var: String::from("PGOLF_MAX_STEPS"),
                default_max_steps: 0,
                validation_eval_mode: Some(ParameterGolfValidationEvalMode::SlidingWindow {
                    stride: 64,
                }),
                validation_batch_sequences: Some(1024),
                claim_boundary: String::from("distributed contract"),
            }],
            runtime_posture: String::from("test"),
            claim_boundary: String::from("test"),
            manifest_digest: String::new(),
        };
        assert_eq!(
            manifest.validation_eval_mode_for_execution_mode("local_reference_validation"),
            &ParameterGolfValidationEvalMode::NonOverlapping
        );
        assert_eq!(
            manifest.validation_batch_sequences_for_execution_mode("local_reference_validation"),
            256
        );
        assert_eq!(
            manifest.validation_eval_mode_for_execution_mode(
                PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE
            ),
            &ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 }
        );
        assert_eq!(
            manifest.validation_batch_sequences_for_execution_mode(
                PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE
            ),
            1024
        );
    }

    #[test]
    fn distributed_completion_receipt_binds_validation_and_final_artifact(
    ) -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let root = tempdir.path();
        let submission_manifest_bytes = b"{\"track\":\"non-record\"}\n";
        fs::write(root.join("submission.json"), submission_manifest_bytes)?;

        let mut manifest = ParameterGolfSubmissionRuntimeManifest {
            schema_version: 1,
            package_version: String::from("test"),
            run_id: String::from("parameter-golf-distributed-runtime-test"),
            benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
            entrypoint_path: String::from("train_gpt.py"),
            runtime_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
            submission_manifest_path: String::from("submission.json"),
            accounting_receipt_path: String::from("parameter_golf_submission_accounting.json"),
            fixture_path: String::from("runtime/parameter_golf_local_reference_fixture.json"),
            model_artifact_path: String::from("submission_model.bin.zlib"),
            runtime_receipt_path: String::from("parameter_golf_submission_runtime_receipt.json"),
            distributed_bringup_report_path: String::from(
                "parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_bringup.json",
            ),
            sequence_length: 1024,
            validation_batch_tokens: 524_288,
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: 64,
            score_first_ttt: None,
            expected_val_loss: 1.0,
            expected_val_bpb: 1.0,
            default_execution_mode: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE),
            real_execution_contracts: Vec::new(),
            runtime_posture: String::from("distributed_runtime"),
            claim_boundary: String::from("distributed runtime boundary"),
            manifest_digest: String::new(),
        };
        manifest.manifest_digest = manifest.stable_digest();

        let bringup_report = sample_bringup_report();
        let bringup_report_path = root.join(&manifest.distributed_bringup_report_path);
        let bootstrap_receipt =
            sample_runtime_bootstrap_receipt(&bringup_report_path, &bringup_report);
        let train_step_receipt = sample_train_step_receipt(root, sample_distributed_receipt())?;
        let train_step_receipt_path =
            root.join("parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_train_step.json");
        let bootstrap_receipt_path =
            root.join("parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_runtime_bootstrap.json");

        let receipt = build_parameter_golf_distributed_8xh100_completion_receipt(
            root,
            &manifest,
            &bringup_report_path,
            &bringup_report,
            &bootstrap_receipt_path,
            &bootstrap_receipt,
            &train_step_receipt_path,
            &train_step_receipt,
        )?;

        assert_eq!(
            receipt.final_model_artifact_digest,
            sha256_bytes(b"model-int8")
        );
        assert_eq!(
            receipt.submission_manifest_digest,
            sha256_bytes(submission_manifest_bytes)
        );
        assert_eq!(receipt.distributed_validation_mean_loss, 3.82);
        assert_eq!(receipt.distributed_validation_bits_per_byte, 1.91);
        assert_eq!(receipt.distributed_validation_observed_ms, 20);
        assert_eq!(receipt.receipt_digest, receipt.stable_digest());
        Ok(())
    }

    #[test]
    fn distributed_completion_receipt_requires_validation_aggregation() -> Result<(), Box<dyn Error>>
    {
        let tempdir = tempdir()?;
        let root = tempdir.path();
        fs::write(
            root.join("submission.json"),
            b"{\"track\":\"non-record\"}\n",
        )?;
        let mut manifest = ParameterGolfSubmissionRuntimeManifest {
            schema_version: 1,
            package_version: String::from("test"),
            run_id: String::from("parameter-golf-distributed-runtime-test"),
            benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
            entrypoint_path: String::from("train_gpt.py"),
            runtime_payload_path: String::from("runtime/parameter_golf_submission_runtime"),
            submission_manifest_path: String::from("submission.json"),
            accounting_receipt_path: String::from("parameter_golf_submission_accounting.json"),
            fixture_path: String::from("runtime/parameter_golf_local_reference_fixture.json"),
            model_artifact_path: String::from("submission_model.bin.zlib"),
            runtime_receipt_path: String::from("parameter_golf_submission_runtime_receipt.json"),
            distributed_bringup_report_path: String::from(
                "parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_bringup.json",
            ),
            sequence_length: 1024,
            validation_batch_tokens: 524_288,
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: 64,
            score_first_ttt: None,
            expected_val_loss: 1.0,
            expected_val_bpb: 1.0,
            default_execution_mode: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_EXECUTION_MODE),
            real_execution_contracts: Vec::new(),
            runtime_posture: String::from("distributed_runtime"),
            claim_boundary: String::from("distributed runtime boundary"),
            manifest_digest: String::new(),
        };
        manifest.manifest_digest = manifest.stable_digest();

        let bringup_report = sample_bringup_report();
        let bringup_report_path = root.join(&manifest.distributed_bringup_report_path);
        let bootstrap_receipt =
            sample_runtime_bootstrap_receipt(&bringup_report_path, &bringup_report);
        let mut train_step_receipt = sample_train_step_receipt(root, sample_distributed_receipt())?;
        train_step_receipt
            .distributed_receipt
            .validation_aggregation = None;
        train_step_receipt.distributed_receipt.receipt_digest =
            train_step_receipt.distributed_receipt.stable_digest();
        let train_step_receipt_path =
            root.join("parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_train_step.json");
        let bootstrap_receipt_path =
            root.join("parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_runtime_bootstrap.json");

        let error = build_parameter_golf_distributed_8xh100_completion_receipt(
            root,
            &manifest,
            &bringup_report_path,
            &bringup_report,
            &bootstrap_receipt_path,
            &bootstrap_receipt,
            &train_step_receipt_path,
            &train_step_receipt,
        )
        .expect_err("completion receipt should fail closed without validation aggregation");
        match error {
            ParameterGolfSubmissionRuntimeError::Consistency { message } => {
                assert_eq!(
                    message,
                    "distributed completion receipt requires one validation-backed distributed receipt"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
        Ok(())
    }
}
