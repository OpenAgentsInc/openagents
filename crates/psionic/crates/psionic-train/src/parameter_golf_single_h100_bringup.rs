use std::collections::BTreeMap;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use psionic_backend_cuda::CudaBackend;
use psionic_core::{DeviceKind, PsionicRefusal};
use psionic_data::{
    materialize_parameter_golf_token_window, parameter_golf_dataset_bundle_from_local_dir,
    DatasetIterationMode, DatasetKey, ParameterGolfDataError, ParameterGolfTokenStreamContract,
    ParameterGolfTokenStreamCursor, TokenizerDigest, TokenizerFamily,
    PARAMETER_GOLF_TRAIN_SPLIT_NAME,
};
use psionic_ir::GraphError;
use psionic_models::{
    ParameterGolfConfig, ParameterGolfExecutionError, ParameterGolfModelError,
    ParameterGolfReferenceModel, PARAMETER_GOLF_BASELINE_MODEL_ID,
    PARAMETER_GOLF_BASELINE_REVISION,
};
use psionic_runtime::{DeviceDescriptor, HealthStatus, RuntimeHealth};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_parameter_golf_cuda_training_capability_report, parameter_golf_optimizer_plan,
    ParameterGolfBatchGeometry, ParameterGolfCudaTrainingCapabilityReport, ParameterGolfTrainError,
    ParameterGolfTrainingHyperparameters,
};

/// Stable dataset reference for the public single-H100 Parameter Golf bring-up lane.
pub const PARAMETER_GOLF_SINGLE_H100_DATASET_REF: &str = "dataset://parameter-golf/fineweb-sp1024";
/// Stable dataset version for the current public single-H100 bring-up lane.
pub const PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION: &str = "2026.03.18";
/// Stable tokenizer variant label for the public single-H100 bring-up lane.
pub const PARAMETER_GOLF_SINGLE_H100_VARIANT: &str = "sp1024";
/// Maximum number of sequences included in the bounded CPU reference-loss probe.
pub const PARAMETER_GOLF_SINGLE_H100_REFERENCE_LOSS_PROBE_MAX_SEQUENCES: usize = 8;

/// Stable machine thresholds for the Rust-native single-H100 bring-up lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ChallengeThresholds {
    /// Required runtime backend.
    pub required_backend: String,
    /// Required device-family substring.
    pub required_device_name: String,
    /// Minimum matching-device count for the single-device lane.
    pub minimum_matching_device_count: usize,
    /// Whether MIG-partitioned devices are outside the current lane.
    pub require_non_mig: bool,
}

impl ParameterGolfSingleH100ChallengeThresholds {
    /// Returns the canonical public single-H100 thresholds.
    #[must_use]
    pub fn challenge_h100() -> Self {
        Self {
            required_backend: String::from("cuda"),
            required_device_name: String::from("H100"),
            minimum_matching_device_count: 1,
            require_non_mig: true,
        }
    }

    /// Returns the HOMEGOLF local-CUDA single-device thresholds.
    #[must_use]
    pub fn homegolf_local_cuda() -> Self {
        Self {
            required_backend: String::from("cuda"),
            required_device_name: String::new(),
            minimum_matching_device_count: 1,
            require_non_mig: true,
        }
    }
}

/// Config for the Rust-native Parameter Golf single-H100 bring-up seam.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100BringupConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Local dataset root containing `fineweb_train_*.bin` and `fineweb_val_*.bin`.
    pub dataset_root: PathBuf,
    /// Local tokenizer artifact path.
    pub tokenizer_path: PathBuf,
    /// Stable dataset identity expected by the bring-up.
    pub dataset_key: DatasetKey,
    /// Stable tokenizer or dataset variant label.
    pub variant: String,
    /// Challenge single-device batch geometry.
    pub geometry: ParameterGolfBatchGeometry,
    /// Baseline hyperparameter contract copied from the public script.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
}

impl ParameterGolfSingleH100BringupConfig {
    /// Returns the canonical single-H100 challenge bring-up config for one dataset root and tokenizer path.
    #[must_use]
    pub fn challenge_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            run_id: String::from("parameter-golf-single-h100-bringup"),
            dataset_root: dataset_root.into(),
            tokenizer_path: tokenizer_path.into(),
            dataset_key: DatasetKey::new(
                PARAMETER_GOLF_SINGLE_H100_DATASET_REF,
                PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION,
            ),
            variant: String::from(PARAMETER_GOLF_SINGLE_H100_VARIANT),
            geometry: ParameterGolfBatchGeometry::challenge_single_device_defaults(),
            hyperparameters: ParameterGolfTrainingHyperparameters::baseline_defaults(),
        }
    }

    fn validate(&self) -> Result<(), ParameterGolfSingleH100BringupError> {
        if self.run_id.trim().is_empty() {
            return Err(ParameterGolfSingleH100BringupError::InvalidConfig {
                message: String::from("run_id must be non-empty"),
            });
        }
        if self.variant.trim().is_empty() {
            return Err(ParameterGolfSingleH100BringupError::InvalidConfig {
                message: String::from("variant must be non-empty"),
            });
        }
        let expected_geometry = ParameterGolfBatchGeometry::challenge_single_device_defaults();
        if self.geometry != expected_geometry {
            return Err(ParameterGolfSingleH100BringupError::InvalidConfig {
                message: format!(
                    "single-H100 bring-up requires challenge_single_device_defaults geometry, found {:?}",
                    self.geometry
                ),
            });
        }
        let expected_hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        if self.hyperparameters != expected_hyperparameters {
            return Err(ParameterGolfSingleH100BringupError::InvalidConfig {
                message: String::from(
                    "single-H100 bring-up requires the public baseline hyperparameters",
                ),
            });
        }
        if !self.dataset_root.is_dir() {
            return Err(ParameterGolfSingleH100BringupError::MissingPath {
                path: self.dataset_root.display().to_string(),
                expected: String::from("dataset directory"),
            });
        }
        if !self.tokenizer_path.is_file() {
            return Err(ParameterGolfSingleH100BringupError::MissingPath {
                path: self.tokenizer_path.display().to_string(),
                expected: String::from("tokenizer file"),
            });
        }
        Ok(())
    }
}

/// Current readiness posture for the Rust-native Parameter Golf single-H100 bring-up seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleH100BringupDisposition {
    /// The local challenge data and the current repo posture are ready for a real attempt.
    ReadyToAttempt,
    /// The local machine does not expose the required single-H100 CUDA posture.
    RefusedMachineContract,
    /// The command can bind dataset and model truth, but the current CUDA blocker list still forces refusal.
    RefusedCudaBlockers,
}

/// Observed execution posture for the Rust-native single-H100 bring-up command itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleH100ExecutionPosture {
    /// The command validated dataset, tokenizer, model, and blocker truth, but did not execute training.
    ContractValidationOnly,
    /// The command executed the actual training path.
    TrainingExecuted,
}

/// One real challenge microbatch probe materialized by the single-H100 bring-up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ReferenceMicrobatchProbe {
    /// Stable token-window identifier.
    pub window_id: String,
    /// Stable contract digest for the planned training window.
    pub contract_digest: String,
    /// Number of raw tokens materialized for the first training microbatch plus one next-token target.
    pub token_count: u64,
    /// Stable digest over the raw token window.
    pub token_digest: String,
    /// Number of sequences in the probe microbatch.
    pub batch_size: usize,
    /// Sequence length per row.
    pub sequence_length: usize,
    /// Number of leading sequences included in the bounded CPU reference-loss probe.
    pub loss_probe_sequence_count: usize,
    /// Number of tokens included in the bounded CPU reference-loss probe.
    pub loss_probe_token_count: u64,
    /// CPU-reference mean loss over the bounded prefix of the materialized microbatch.
    pub mean_loss: f32,
}

/// Machine-readable report for the Rust-native Parameter Golf single-H100 bring-up seam.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100BringupReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Local dataset root used for the report.
    pub dataset_root: PathBuf,
    /// Local tokenizer artifact used for the report.
    pub tokenizer_path: PathBuf,
    /// Stable dataset identity expected by the command.
    pub dataset_key: DatasetKey,
    /// Stable variant label.
    pub variant: String,
    /// Machine-readable tokenizer digest summary for the supplied tokenizer file.
    pub tokenizer_digest: TokenizerDigest,
    /// Stable dataset manifest digest for the selected local shard directory.
    pub dataset_manifest_digest: String,
    /// Selected train-shard count in the dataset bundle.
    pub train_shard_count: usize,
    /// Validation-shard count in the dataset bundle.
    pub validation_shard_count: usize,
    /// Total train tokens in the selected train prefix.
    pub train_token_count: u64,
    /// Total validation tokens in the fixed validation split.
    pub validation_token_count: u64,
    /// Current public train-selection posture from the manifest metadata.
    pub train_selection_posture: String,
    /// Current fixed validation identity from the manifest metadata.
    pub validation_identity: String,
    /// Public single-device batch geometry for the run.
    pub geometry: ParameterGolfBatchGeometry,
    /// Public baseline hyperparameters for the run.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
    /// Optional real first-microbatch probe when the machine contract is satisfied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_microbatch_probe: Option<ParameterGolfSingleH100ReferenceMicrobatchProbe>,
    /// Declared local-machine admission thresholds for the single-H100 lane.
    pub machine_thresholds: ParameterGolfSingleH100ChallengeThresholds,
    /// Observed local CUDA backend health when the command ran.
    pub observed_cuda_health: RuntimeHealth,
    /// Optional CUDA discovery error when backend probing could not enumerate inventory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_discovery_error: Option<String>,
    /// Observed CUDA inventory visible to the bring-up command.
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    /// Count of observed devices that satisfy the single-H100 machine contract.
    pub matching_h100_device_count: usize,
    /// Whether the local machine satisfies the single-H100 CUDA contract.
    pub machine_contract_satisfied: bool,
    /// Baseline model id bound to the bring-up.
    pub baseline_model_id: String,
    /// Baseline model revision bound to the bring-up.
    pub baseline_model_revision: String,
    /// Baseline model config bound to the bring-up.
    pub baseline_model_config: ParameterGolfConfig,
    /// Stable baseline model descriptor digest.
    pub baseline_model_descriptor_digest: String,
    /// Stable optimizer-plan digest for the baseline model.
    pub optimizer_plan_digest: String,
    /// Human-readable Psionic-native command for the bring-up.
    pub psionic_entrypoint: String,
    /// Human-readable upstream reference command for the same dataset or tokenizer contract.
    pub upstream_reference_entrypoint: String,
    /// Current CUDA capability report digest reused by the bring-up.
    pub cuda_training_capability_report_digest: String,
    /// Explicit current challenge blocker ids.
    pub challenge_kernel_blockers: Vec<String>,
    /// Explicit CUDA-blocker refusal kept separate from machine admission refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_blocker_refusal: Option<PsionicRefusal>,
    /// Observed bring-up start time.
    pub started_at_ms: u64,
    /// Observed bring-up finish time.
    pub finished_at_ms: u64,
    /// Observed wallclock for the bring-up command itself.
    pub observed_wallclock_ms: u64,
    /// Whether the command only validated contracts or actually executed training.
    pub execution_posture: ParameterGolfSingleH100ExecutionPosture,
    /// Final validation loss when training actually executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_val_loss: Option<f64>,
    /// Final validation bits-per-byte when training actually executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_val_bpb: Option<f64>,
    /// Final compressed model bytes when training actually executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed_model_bytes: Option<u64>,
    /// Current bring-up disposition.
    pub disposition: ParameterGolfSingleH100BringupDisposition,
    /// Primary refusal for the current command posture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Explicit drift or refusal notes preserved from the current run.
    pub drift_notes: Vec<String>,
    /// Honest claim boundary for the report.
    pub claim_boundary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl ParameterGolfSingleH100BringupReport {
    /// Returns whether the current report is green for a real single-H100 attempt.
    #[must_use]
    pub const fn ready_to_attempt(&self) -> bool {
        matches!(
            self.disposition,
            ParameterGolfSingleH100BringupDisposition::ReadyToAttempt
        )
    }
}

/// Error while building or writing the Rust-native Parameter Golf single-H100 bring-up report.
#[derive(Debug, Error)]
pub enum ParameterGolfSingleH100BringupError {
    #[error("parameter golf single-H100 bring-up config is invalid: {message}")]
    InvalidConfig { message: String },
    #[error("parameter golf single-H100 bring-up is missing {expected} at `{path}`")]
    MissingPath { path: String, expected: String },
    #[error("parameter golf single-H100 bring-up failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("parameter golf single-H100 bring-up failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Data(#[from] ParameterGolfDataError),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
    #[error(transparent)]
    Execution(#[from] ParameterGolfExecutionError),
    #[error(transparent)]
    Train(#[from] ParameterGolfTrainError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

/// Builds the Rust-native Parameter Golf single-H100 bring-up report for one local dataset or tokenizer contract.
pub fn build_parameter_golf_single_h100_bringup_report(
    config: &ParameterGolfSingleH100BringupConfig,
) -> Result<ParameterGolfSingleH100BringupReport, ParameterGolfSingleH100BringupError> {
    let started_at_ms = now_ms();
    let started = Instant::now();
    config.validate()?;
    let tokenizer_bytes = fs::read(&config.tokenizer_path).map_err(|error| {
        ParameterGolfSingleH100BringupError::Read {
            path: config.tokenizer_path.display().to_string(),
            error,
        }
    })?;
    let tokenizer_digest = build_tokenizer_digest(&tokenizer_bytes);
    let bundle = parameter_golf_dataset_bundle_from_local_dir(
        config.dataset_key.clone(),
        &config.dataset_root,
        config.variant.clone(),
        tokenizer_digest.clone(),
        config.tokenizer_path.display().to_string(),
        None,
    )?;
    let baseline_model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
    let optimizer_plan =
        parameter_golf_optimizer_plan(baseline_model.descriptor(), &config.hyperparameters)?;
    let optimizer_plan_digest =
        stable_digest(b"psionic_parameter_golf_optimizer_plan|", &optimizer_plan);
    let cuda_training_report = builtin_parameter_golf_cuda_training_capability_report()?;
    let machine_observation = inspect_local_single_h100_machine();
    let reference_microbatch_probe =
        build_reference_microbatch_probe(config, &bundle, &baseline_model, &machine_observation)?;
    let observed_wallclock_ms = started.elapsed().as_millis() as u64;
    let finished_at_ms = now_ms();
    Ok(build_report(
        config,
        &bundle,
        baseline_model,
        reference_microbatch_probe,
        &machine_observation,
        optimizer_plan_digest,
        &cuda_training_report,
        started_at_ms,
        finished_at_ms,
        observed_wallclock_ms,
    ))
}

/// Writes the Rust-native Parameter Golf single-H100 bring-up report to one JSON path.
pub fn write_parameter_golf_single_h100_bringup_report(
    output_path: &Path,
    config: &ParameterGolfSingleH100BringupConfig,
) -> Result<ParameterGolfSingleH100BringupReport, ParameterGolfSingleH100BringupError> {
    let report = build_parameter_golf_single_h100_bringup_report(config)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| ParameterGolfSingleH100BringupError::Write {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let encoded = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        ParameterGolfSingleH100BringupError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_report(
    config: &ParameterGolfSingleH100BringupConfig,
    bundle: &psionic_data::ParameterGolfDatasetBundle,
    baseline_model: ParameterGolfReferenceModel,
    reference_microbatch_probe: Option<ParameterGolfSingleH100ReferenceMicrobatchProbe>,
    machine_observation: &ParameterGolfSingleH100MachineObservation,
    optimizer_plan_digest: String,
    cuda_training_report: &ParameterGolfCudaTrainingCapabilityReport,
    started_at_ms: u64,
    finished_at_ms: u64,
    observed_wallclock_ms: u64,
) -> ParameterGolfSingleH100BringupReport {
    let cuda_blocker_refusal = cuda_training_report.challenge_readiness_refusal();
    let refusal = machine_observation
        .refusal
        .clone()
        .or_else(|| cuda_blocker_refusal.clone());
    let disposition = if machine_observation.refusal.is_some() {
        ParameterGolfSingleH100BringupDisposition::RefusedMachineContract
    } else if cuda_blocker_refusal.is_some() {
        ParameterGolfSingleH100BringupDisposition::RefusedCudaBlockers
    } else {
        ParameterGolfSingleH100BringupDisposition::ReadyToAttempt
    };
    let mut drift_notes = cuda_training_report.boundary_notes();
    if let Some(error) = &machine_observation.cuda_discovery_error {
        drift_notes.push(format!("CUDA inventory discovery failed while evaluating the local single-H100 contract: {error}"));
    }
    drift_notes.push(format!(
        "Observed CUDA backend health was {:?} with {} discovered CUDA device(s); {} matched the single-H100 contract.",
        machine_observation.observed_cuda_health.status,
        machine_observation.observed_cuda_devices.len(),
        machine_observation.matching_h100_device_count,
    ));
    if let Some(refusal) = &machine_observation.refusal {
        drift_notes.push(format!(
            "The current local machine does not satisfy the single-H100 contract: {}",
            refusal.detail
        ));
    }
    if let Some(probe) = &reference_microbatch_probe {
        drift_notes.push(format!(
            "The current single-H100 bring-up command materialized the first real training window ({}) and computed a bounded CPU reference mean loss of {:.8} over the first {} sequence(s), but it does not yet execute the CUDA baseline training loop.",
            probe.window_id,
            probe.mean_loss,
            probe.loss_probe_sequence_count,
        ));
    } else {
        drift_notes.push(String::from(
            "The current single-H100 bring-up command validates dataset, tokenizer, model, optimizer, and blocker truth only; it does not yet execute the real baseline training loop.",
        ));
    }
    drift_notes.push(String::from(
        "final_val_loss, final_val_bpb, and compressed_model_bytes are absent because no training artifact was produced by this command.",
    ));
    let train_shard_count = bundle.train_shards.len();
    let validation_shard_count = bundle.validation_shards.len();
    let train_token_count = bundle
        .train_shards
        .iter()
        .map(|shard| shard.header.token_count as u64)
        .sum::<u64>();
    let validation_token_count = bundle
        .validation_shards
        .iter()
        .map(|shard| shard.header.token_count as u64)
        .sum::<u64>();
    let train_selection_posture = metadata_string(
        &bundle.manifest.metadata,
        "parameter_golf_train_selection_posture",
    );
    let validation_identity = metadata_string(
        &bundle.manifest.metadata,
        "parameter_golf_validation_identity",
    );
    let psionic_entrypoint = format!(
        "cargo run -q -p psionic-train --bin parameter_golf_single_h100_bringup -- {} {}",
        config.dataset_root.display(),
        config.tokenizer_path.display()
    );
    let upstream_reference_entrypoint = format!(
        "RUN_ID={} DATA_PATH={} TOKENIZER_PATH={} VOCAB_SIZE=1024 torchrun --standalone --nproc_per_node=1 train_gpt.py",
        config.run_id,
        config.dataset_root.display(),
        config.tokenizer_path.display()
    );
    let claim_boundary = if report_claims_reference_probe(
        &machine_observation.refusal,
        &reference_microbatch_probe,
    ) {
        String::from(
            "This report proves the Rust-native single-H100 bring-up command owns the challenge dataset, tokenizer identity, local CUDA machine-admission truth, baseline model contract, one real first-microbatch materialization plus a bounded CPU reference-loss probe over its leading sequences, observed bring-up wallclock, and explicit CUDA blocker truth. It does not claim successful Psionic CUDA training execution or a produced model artifact while the current command remains a validation-and-refusal seam.",
        )
    } else {
        String::from(
            "This report proves the Rust-native single-H100 bring-up command owns the challenge dataset, tokenizer identity, local CUDA machine-admission truth, baseline model contract, observed bring-up wallclock, and explicit CUDA blocker truth. It does not claim successful Psionic training execution or a produced model artifact while the current command remains a validation-and-refusal seam.",
        )
    };
    let mut report = ParameterGolfSingleH100BringupReport {
        schema_version: 1,
        scope_window: String::from("parameter_golf_single_h100_bringup_v1"),
        run_id: config.run_id.clone(),
        dataset_root: config.dataset_root.clone(),
        tokenizer_path: config.tokenizer_path.clone(),
        dataset_key: config.dataset_key.clone(),
        variant: config.variant.clone(),
        tokenizer_digest: bundle.manifest.tokenizer.clone(),
        dataset_manifest_digest: bundle.manifest.stable_digest(),
        train_shard_count,
        validation_shard_count,
        train_token_count,
        validation_token_count,
        train_selection_posture,
        validation_identity,
        geometry: config.geometry.clone(),
        hyperparameters: config.hyperparameters.clone(),
        reference_microbatch_probe,
        machine_thresholds: machine_observation.thresholds.clone(),
        observed_cuda_health: machine_observation.observed_cuda_health.clone(),
        cuda_discovery_error: machine_observation.cuda_discovery_error.clone(),
        observed_cuda_devices: machine_observation.observed_cuda_devices.clone(),
        matching_h100_device_count: machine_observation.matching_h100_device_count,
        machine_contract_satisfied: machine_observation.machine_contract_satisfied,
        baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
        baseline_model_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
        baseline_model_config: baseline_model.descriptor().config.clone(),
        baseline_model_descriptor_digest: baseline_model.descriptor().stable_digest(),
        optimizer_plan_digest,
        psionic_entrypoint,
        upstream_reference_entrypoint,
        cuda_training_capability_report_digest: cuda_training_report.report_digest.clone(),
        challenge_kernel_blockers: cuda_training_report.challenge_kernel_blockers().to_vec(),
        cuda_blocker_refusal,
        started_at_ms,
        finished_at_ms,
        observed_wallclock_ms,
        execution_posture: ParameterGolfSingleH100ExecutionPosture::ContractValidationOnly,
        final_val_loss: None,
        final_val_bpb: None,
        compressed_model_bytes: None,
        disposition,
        refusal,
        drift_notes,
        claim_boundary,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_single_h100_bringup_report|",
        &report,
    );
    report
}

fn build_reference_microbatch_probe(
    config: &ParameterGolfSingleH100BringupConfig,
    bundle: &psionic_data::ParameterGolfDatasetBundle,
    baseline_model: &ParameterGolfReferenceModel,
    machine_observation: &ParameterGolfSingleH100MachineObservation,
) -> Result<
    Option<ParameterGolfSingleH100ReferenceMicrobatchProbe>,
    ParameterGolfSingleH100BringupError,
> {
    if !machine_observation.machine_contract_satisfied {
        return Ok(None);
    }
    let train_contract = ParameterGolfTokenStreamContract::new(
        bundle.manifest.key.clone(),
        PARAMETER_GOLF_TRAIN_SPLIT_NAME,
    )
    .with_mode(DatasetIterationMode::Repeat);
    let cursor = ParameterGolfTokenStreamCursor::new(PARAMETER_GOLF_TRAIN_SPLIT_NAME);
    let requested_token_count = config.geometry.local_train_batch_tokens().saturating_add(1) as u64;
    let window = train_contract
        .plan_window(&bundle.manifest, &cursor, requested_token_count)?
        .ok_or(ParameterGolfSingleH100BringupError::InvalidConfig {
            message: String::from(
                "single-H100 reference microbatch probe could not plan the first training window",
            ),
        })?;
    let tokens = materialize_parameter_golf_token_window(bundle, &window)?;
    let expected_token_count = config.geometry.local_train_batch_tokens().saturating_add(1);
    if tokens.len() != expected_token_count {
        return Err(ParameterGolfSingleH100BringupError::InvalidConfig {
            message: format!(
                "single-H100 reference microbatch probe expected {expected_token_count} tokens but materialized {}",
                tokens.len()
            ),
        });
    }
    let (input_ids, target_ids) =
        training_batch_from_window_tokens(tokens.as_slice(), &config.geometry)?;
    let (loss_probe_inputs, loss_probe_targets) =
        bounded_reference_loss_probe_batch(input_ids.as_slice(), target_ids.as_slice());
    let loss_probe_sequence_count = loss_probe_inputs.len();
    let mean_loss =
        baseline_model.loss(loss_probe_inputs.as_slice(), loss_probe_targets.as_slice())?;
    Ok(Some(ParameterGolfSingleH100ReferenceMicrobatchProbe {
        window_id: window.window_id,
        contract_digest: window.contract_digest,
        token_count: tokens.len() as u64,
        token_digest: stable_digest(
            b"psionic_parameter_golf_single_h100_reference_window_tokens|",
            &tokens,
        ),
        batch_size: input_ids.len(),
        sequence_length: config.geometry.train_sequence_length,
        loss_probe_sequence_count,
        loss_probe_token_count: (loss_probe_sequence_count * config.geometry.train_sequence_length)
            as u64,
        mean_loss,
    }))
}

pub(crate) fn training_batch_from_window_tokens(
    tokens: &[u16],
    geometry: &ParameterGolfBatchGeometry,
) -> Result<(Vec<Vec<u32>>, Vec<Vec<u32>>), ParameterGolfSingleH100BringupError> {
    let expected_token_count = geometry.local_train_batch_tokens().saturating_add(1);
    if tokens.len() != expected_token_count {
        return Err(ParameterGolfSingleH100BringupError::InvalidConfig {
            message: format!(
                "training token window must contain exactly {expected_token_count} tokens, found {}",
                tokens.len()
            ),
        });
    }
    let batch_size = geometry.local_train_batch_sequences();
    let sequence_length = geometry.train_sequence_length;
    let mut input_ids = Vec::with_capacity(batch_size);
    let mut target_ids = Vec::with_capacity(batch_size);
    for sequence_index in 0..batch_size {
        let start = sequence_index * sequence_length;
        let end = start + sequence_length;
        input_ids.push(
            tokens[start..end]
                .iter()
                .map(|&token_id| token_id as u32)
                .collect(),
        );
        target_ids.push(
            tokens[start + 1..end + 1]
                .iter()
                .map(|&token_id| token_id as u32)
                .collect(),
        );
    }
    Ok((input_ids, target_ids))
}

fn bounded_reference_loss_probe_batch(
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> (Vec<Vec<u32>>, Vec<Vec<u32>>) {
    let loss_probe_sequence_count = input_ids
        .len()
        .min(PARAMETER_GOLF_SINGLE_H100_REFERENCE_LOSS_PROBE_MAX_SEQUENCES);
    (
        input_ids[..loss_probe_sequence_count].to_vec(),
        target_ids[..loss_probe_sequence_count].to_vec(),
    )
}

fn report_claims_reference_probe(
    machine_refusal: &Option<PsionicRefusal>,
    reference_microbatch_probe: &Option<ParameterGolfSingleH100ReferenceMicrobatchProbe>,
) -> bool {
    machine_refusal.is_none() && reference_microbatch_probe.is_some()
}

pub(crate) fn build_tokenizer_digest(tokenizer_bytes: &[u8]) -> TokenizerDigest {
    TokenizerDigest::new(
        TokenizerFamily::SentencePiece,
        sha256_hex(tokenizer_bytes),
        1024,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParameterGolfSingleH100MachineObservation {
    pub thresholds: ParameterGolfSingleH100ChallengeThresholds,
    pub observed_cuda_health: RuntimeHealth,
    pub cuda_discovery_error: Option<String>,
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    pub matching_h100_device_count: usize,
    pub machine_contract_satisfied: bool,
    pub refusal: Option<PsionicRefusal>,
}

pub(crate) fn inspect_local_single_h100_machine() -> ParameterGolfSingleH100MachineObservation {
    inspect_local_single_h100_machine_with_thresholds(
        ParameterGolfSingleH100ChallengeThresholds::challenge_h100(),
    )
}

pub(crate) fn inspect_local_single_h100_machine_with_thresholds(
    thresholds: ParameterGolfSingleH100ChallengeThresholds,
) -> ParameterGolfSingleH100MachineObservation {
    let backend = CudaBackend::new();
    match backend.discovery_report() {
        Ok(report) => {
            machine_observation_from_inventory(thresholds, report.health, None, report.devices)
        }
        Err(error) => machine_observation_from_inventory(
            thresholds,
            RuntimeHealth {
                status: HealthStatus::Offline,
                message: error.to_string(),
            },
            Some(error.to_string()),
            Vec::new(),
        ),
    }
}

pub(crate) fn machine_observation_from_inventory(
    thresholds: ParameterGolfSingleH100ChallengeThresholds,
    observed_cuda_health: RuntimeHealth,
    cuda_discovery_error: Option<String>,
    observed_cuda_devices: Vec<DeviceDescriptor>,
) -> ParameterGolfSingleH100MachineObservation {
    let matching_h100_device_count = observed_cuda_devices
        .iter()
        .filter(|device| device_matches_single_h100(device, &thresholds))
        .count();
    let machine_contract_satisfied = observed_cuda_health.status != HealthStatus::Offline
        && matching_h100_device_count >= thresholds.minimum_matching_device_count;
    let refusal = (!machine_contract_satisfied).then(|| {
        let observed_names = if observed_cuda_devices.is_empty() {
            String::from("no discovered CUDA devices")
        } else {
            observed_cuda_devices
                .iter()
                .map(observed_device_label)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let expected_shape = if thresholds.require_non_mig {
            format!(
                "at least {} non-MIG `{}` device(s)",
                thresholds.minimum_matching_device_count, thresholds.required_device_name
            )
        } else {
            format!(
                "at least {} `{}` device(s)",
                thresholds.minimum_matching_device_count, thresholds.required_device_name
            )
        };
        let detail = match &cuda_discovery_error {
            Some(error) => format!(
                "single-H100 bring-up requires {expected_shape} on backend `{}` but CUDA discovery failed: {error}",
                thresholds.required_backend
            ),
            None => format!(
                "single-H100 bring-up requires {expected_shape} on backend `{}` but found {} matching device(s) among: {observed_names}",
                thresholds.required_backend, matching_h100_device_count
            ),
        };
        PsionicRefusal::new(
            psionic_core::PsionicRefusalCode::UnsupportedBackendCapability,
            psionic_core::PsionicRefusalScope::Runtime,
            detail,
        )
        .with_subject(String::from("parameter_golf_single_h100_machine"))
    });
    ParameterGolfSingleH100MachineObservation {
        thresholds,
        observed_cuda_health,
        cuda_discovery_error,
        observed_cuda_devices,
        matching_h100_device_count,
        machine_contract_satisfied,
        refusal,
    }
}

pub(crate) fn device_matches_single_h100(
    device: &DeviceDescriptor,
    thresholds: &ParameterGolfSingleH100ChallengeThresholds,
) -> bool {
    if device.backend != thresholds.required_backend || device.device.kind() != DeviceKind::Cuda {
        return false;
    }
    let name_matches = thresholds.required_device_name.is_empty()
        || device
            .device_name
            .as_deref()
            .is_some_and(|name| name.contains(thresholds.required_device_name.as_str()));
    if !name_matches {
        return false;
    }
    if !thresholds.require_non_mig {
        return true;
    }
    device
        .nvidia_metadata
        .as_ref()
        .is_some_and(|metadata| !metadata.risk.mig_partitioned)
}

fn observed_device_label(device: &DeviceDescriptor) -> String {
    device
        .device_name
        .clone()
        .unwrap_or_else(|| device.device.label().unwrap_or("unknown").to_string())
}

fn metadata_string(metadata: &BTreeMap<String, Value>, key: &str) -> String {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use psionic_core::{DType, Device, DeviceKind, QuantizationMode};
    use psionic_runtime::{
        HealthStatus, NvidiaDeviceMetadata, NvidiaRecoveryAction, NvidiaRecoveryProfile,
        NvidiaRiskLevel, NvidiaRiskProfile, NvidiaTopologyInfo, QuantizationExecution,
        QuantizationLoadPath, QuantizationSupport, RuntimeHealth,
    };

    use super::{
        bounded_reference_loss_probe_batch, build_parameter_golf_single_h100_bringup_report,
        device_matches_single_h100, machine_observation_from_inventory,
        training_batch_from_window_tokens, write_parameter_golf_single_h100_bringup_report,
        ParameterGolfSingleH100BringupConfig, ParameterGolfSingleH100BringupDisposition,
        ParameterGolfSingleH100ChallengeThresholds, ParameterGolfSingleH100ExecutionPosture,
        PARAMETER_GOLF_SINGLE_H100_DATASET_REF, PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION,
    };
    use crate::ParameterGolfBatchGeometry;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let unique = format!(
                "psionic_parameter_golf_single_h100_{}_{}_{}",
                label,
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time should be monotonic")
                    .as_nanos()
            );
            path.push(unique);
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_shard(path: &Path, tokens: &[u16]) {
        const SHARD_HEADER_BYTES: usize = 256 * 4;
        const SHARD_MAGIC: i32 = 20240520;
        const SHARD_VERSION: i32 = 1;

        let mut bytes = vec![0_u8; SHARD_HEADER_BYTES];
        bytes[0..4].copy_from_slice(&SHARD_MAGIC.to_le_bytes());
        bytes[4..8].copy_from_slice(&SHARD_VERSION.to_le_bytes());
        bytes[8..12].copy_from_slice(&(tokens.len() as i32).to_le_bytes());
        for token in tokens {
            bytes.extend_from_slice(&token.to_le_bytes());
        }
        fs::write(path, bytes).expect("shard should be written");
    }

    fn sample_dataset_root() -> TempDirGuard {
        let temp = TempDirGuard::new("dataset");
        write_shard(&temp.path.join("fineweb_train_000000.bin"), &[1, 2, 3, 4]);
        write_shard(
            &temp.path.join("fineweb_train_000001.bin"),
            &[5, 6, 7, 8, 9],
        );
        write_shard(
            &temp.path.join("fineweb_val_000000.bin"),
            &[10, 11, 12, 13, 14],
        );
        temp
    }

    #[test]
    fn single_h100_bringup_report_surfaces_dataset_and_current_readiness_posture(
    ) -> Result<(), Box<dyn Error>> {
        let dataset = sample_dataset_root();
        let tokenizer_path = dataset.path.join("fineweb_1024_bpe.model");
        fs::write(&tokenizer_path, b"sentencepiece-placeholder")?;
        let config = ParameterGolfSingleH100BringupConfig::challenge_defaults(
            &dataset.path,
            &tokenizer_path,
        );

        let report = build_parameter_golf_single_h100_bringup_report(&config)?;

        assert_eq!(
            report.dataset_key.dataset_ref,
            PARAMETER_GOLF_SINGLE_H100_DATASET_REF
        );
        assert_eq!(
            report.dataset_key.version,
            PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION
        );
        assert_eq!(report.train_shard_count, 2);
        assert_eq!(report.validation_shard_count, 1);
        assert_eq!(report.train_token_count, 9);
        assert_eq!(report.validation_token_count, 5);
        assert_eq!(
            report.execution_posture,
            ParameterGolfSingleH100ExecutionPosture::ContractValidationOnly
        );
        assert!(matches!(
            report.disposition,
            ParameterGolfSingleH100BringupDisposition::RefusedMachineContract
                | ParameterGolfSingleH100BringupDisposition::ReadyToAttempt
        ));
        assert!(report.challenge_kernel_blockers.is_empty());
        assert!(report.cuda_blocker_refusal.is_none());
        assert_eq!(
            report.machine_thresholds,
            ParameterGolfSingleH100ChallengeThresholds::challenge_h100()
        );
        assert!(report.matching_h100_device_count <= report.observed_cuda_devices.len());
        if report.machine_contract_satisfied {
            assert!(report.reference_microbatch_probe.is_some());
            assert_eq!(
                report.disposition,
                ParameterGolfSingleH100BringupDisposition::ReadyToAttempt
            );
            assert!(report.refusal.is_none());
            assert!(report.ready_to_attempt());
        } else {
            assert!(report.reference_microbatch_probe.is_none());
            assert_eq!(
                report.disposition,
                ParameterGolfSingleH100BringupDisposition::RefusedMachineContract
            );
            assert!(report.refusal.is_some());
            assert!(report
                .refusal
                .as_ref()
                .is_some_and(|refusal| refusal.subject.as_deref()
                    == Some("parameter_golf_single_h100_machine")));
            assert!(!report.ready_to_attempt());
        }
        assert!(report.observed_wallclock_ms > 0);
        assert!(report.finished_at_ms >= report.started_at_ms);
        assert!(report.final_val_loss.is_none());
        assert!(report.final_val_bpb.is_none());
        assert!(report.compressed_model_bytes.is_none());
        assert!(report
            .drift_notes
            .iter()
            .any(|note| note.contains("does not yet execute the real baseline training loop")));
        assert_eq!(report.baseline_model_config.vocab_size, 1024);
        assert_eq!(report.geometry, config.geometry);
        assert_eq!(report.hyperparameters, config.hyperparameters);
        assert!(!report.report_digest.is_empty());
        Ok(())
    }

    #[test]
    fn single_h100_bringup_report_writer_persists_machine_readable_json(
    ) -> Result<(), Box<dyn Error>> {
        let dataset = sample_dataset_root();
        let tokenizer_path = dataset.path.join("fineweb_1024_bpe.model");
        fs::write(&tokenizer_path, b"sentencepiece-placeholder")?;
        let config = ParameterGolfSingleH100BringupConfig::challenge_defaults(
            &dataset.path,
            &tokenizer_path,
        );
        let output_path = dataset.path.join("report.json");

        let report = write_parameter_golf_single_h100_bringup_report(&output_path, &config)?;
        let loaded = serde_json::from_slice::<super::ParameterGolfSingleH100BringupReport>(
            fs::read(&output_path)?.as_slice(),
        )?;

        assert_eq!(loaded.report_digest, report.report_digest);
        assert_eq!(loaded.train_shard_count, 2);
        assert_eq!(loaded.machine_thresholds, report.machine_thresholds);
        assert_eq!(
            loaded.machine_contract_satisfied,
            report.machine_contract_satisfied
        );
        Ok(())
    }

    #[test]
    fn single_h100_machine_observation_refuses_non_matching_inventory() {
        let thresholds = ParameterGolfSingleH100ChallengeThresholds::challenge_h100();
        let observation = machine_observation_from_inventory(
            thresholds.clone(),
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("cuda ready on 1 NVIDIA device"),
            },
            None,
            vec![sample_cuda_device(
                "NVIDIA GeForce RTX 4080",
                false,
                Some(16 * 1024 * 1024 * 1024),
            )],
        );

        assert_eq!(observation.thresholds, thresholds);
        assert_eq!(observation.matching_h100_device_count, 0);
        assert!(!observation.machine_contract_satisfied);
        assert!(observation.refusal.as_ref().is_some_and(
            |refusal| refusal.subject.as_deref() == Some("parameter_golf_single_h100_machine")
        ));
    }

    #[test]
    fn single_h100_machine_observation_accepts_non_mig_h100_inventory() {
        let thresholds = ParameterGolfSingleH100ChallengeThresholds::challenge_h100();
        let device = sample_cuda_device(
            "NVIDIA H100 80GB HBM3",
            false,
            Some(80 * 1024 * 1024 * 1024),
        );
        assert!(device_matches_single_h100(&device, &thresholds));

        let observation = machine_observation_from_inventory(
            thresholds,
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("cuda ready on 1 NVIDIA device"),
            },
            None,
            vec![device],
        );

        assert_eq!(observation.matching_h100_device_count, 1);
        assert!(observation.machine_contract_satisfied);
        assert!(observation.refusal.is_none());
    }

    #[test]
    fn training_batch_from_window_tokens_slices_inputs_and_targets() -> Result<(), Box<dyn Error>> {
        let tokens = (1_u16..=33_u16).collect::<Vec<_>>();
        let geometry = ParameterGolfBatchGeometry {
            world_size: 1,
            train_batch_tokens: 32,
            validation_batch_tokens: 32,
            train_sequence_length: 4,
            grad_accum_steps: 1,
        };

        let (input_ids, target_ids) = training_batch_from_window_tokens(&tokens, &geometry)?;

        assert_eq!(input_ids.len(), geometry.local_train_batch_sequences());
        assert_eq!(target_ids.len(), geometry.local_train_batch_sequences());
        assert_eq!(input_ids[0], vec![1, 2, 3, 4]);
        assert_eq!(target_ids[0], vec![2, 3, 4, 5]);
        assert_eq!(input_ids.last(), Some(&vec![29, 30, 31, 32]));
        assert_eq!(target_ids.last(), Some(&vec![30, 31, 32, 33]));
        Ok(())
    }

    #[test]
    fn bounded_reference_loss_probe_batch_caps_the_cpu_probe_prefix() {
        let input_ids = (0_u32..12_u32)
            .map(|row| vec![row, row + 100])
            .collect::<Vec<_>>();
        let target_ids = (0_u32..12_u32)
            .map(|row| vec![row + 1, row + 101])
            .collect::<Vec<_>>();

        let (probe_inputs, probe_targets) =
            bounded_reference_loss_probe_batch(input_ids.as_slice(), target_ids.as_slice());

        assert_eq!(
            probe_inputs.len(),
            super::PARAMETER_GOLF_SINGLE_H100_REFERENCE_LOSS_PROBE_MAX_SEQUENCES
        );
        assert_eq!(probe_targets.len(), probe_inputs.len());
        assert_eq!(probe_inputs.first(), Some(&vec![0, 100]));
        assert_eq!(probe_inputs.last(), Some(&vec![7, 107]));
        assert_eq!(probe_targets.last(), Some(&vec![8, 108]));
    }

    fn sample_cuda_device(
        name: &str,
        mig_partitioned: bool,
        memory_capacity_bytes: Option<u64>,
    ) -> psionic_runtime::DeviceDescriptor {
        psionic_runtime::DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from(name)),
            supported_dtypes: vec![DType::F32, DType::F16, DType::BF16],
            supported_quantization: vec![QuantizationSupport {
                mode: QuantizationMode::Int8Symmetric,
                load_path: QuantizationLoadPath::BackendQuantized,
                execution: QuantizationExecution::Native,
            }],
            memory_capacity_bytes,
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("hopper")),
                    compute_capability: Some(String::from("9.0")),
                    pci_bdf: Some(String::from("00000000:01:00.0")),
                    sm_count: Some(132),
                    vram_bytes: memory_capacity_bytes,
                    mig_profile: mig_partitioned.then(|| String::from("1g.10gb")),
                },
                risk: NvidiaRiskProfile {
                    level: if mig_partitioned {
                        NvidiaRiskLevel::Elevated
                    } else {
                        NvidiaRiskLevel::Standard
                    },
                    display_attached: Some(false),
                    mig_partitioned,
                    warnings: Vec::new(),
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                    ],
                },
            }),
        }
    }
}
