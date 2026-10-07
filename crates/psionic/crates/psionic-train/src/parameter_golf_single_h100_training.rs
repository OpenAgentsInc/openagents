use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use half::bf16;
use psionic_backend_cuda::{CudaBackend, CudaBuffer};
use psionic_core::{
    DType, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, Shape, Tensor, TensorData,
    TensorId, TensorSpec,
};
use psionic_data::{
    load_parameter_golf_validation_tokens_from_paths, materialize_parameter_golf_token_window,
    parameter_golf_dataset_bundle_from_local_dir,
    parameter_golf_sentencepiece_byte_luts_from_tokenizer_path, DatasetIterationMode, DatasetKey,
    ParameterGolfDataError, ParameterGolfDatasetBundle, ParameterGolfSentencePieceByteLuts,
    ParameterGolfTokenStreamContract, ParameterGolfTokenStreamCursor,
    ParameterGolfTokenStreamWindow, PARAMETER_GOLF_TRAIN_SPLIT_NAME,
};
use psionic_ir::AutodiffBackwardResult;
use psionic_ir::{AutodiffError, Graph, GraphBuilder, GraphError, OpKind};
use psionic_models::{
    ModelDescriptor, ParameterGolfBankedWeights, ParameterGolfConfig, ParameterGolfExecutionError,
    ParameterGolfModelError, ParameterGolfReferenceModel, ParameterGolfWeights,
    PARAMETER_GOLF_BASELINE_MODEL_ID, PARAMETER_GOLF_BASELINE_REVISION,
};
use psionic_runtime::{
    BufferHandle, DeliveredExecutionContext, DeviceDescriptor, RuntimeError, RuntimeHealth,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    apply_parameter_golf_cuda_bf16_master_weight_optimizer_step,
    apply_parameter_golf_cuda_muon_step, bind_parameter_golf_baseline_training_graph_inputs,
    bind_parameter_golf_baseline_training_graph_inputs_with_banked_weights,
    build_parameter_golf_baseline_eval_graph_with_matrix_execution_mode,
    build_parameter_golf_baseline_training_graph_with_matrix_execution_mode,
    build_tokenizer_digest, builtin_parameter_golf_cuda_training_capability_report,
    device_matches_single_h100, export_parameter_golf_quantized_model_artifact,
    inspect_local_single_h100_machine_with_thresholds,
    materialize_parameter_golf_baseline_training_gradients, parameter_golf_optimizer_plan,
    parameter_golf_parameter_values_for_bindings,
    restore_parameter_golf_model_from_quantized_artifact, training_batch_from_window_tokens,
    ParameterGolfBaselineEvalGraph, ParameterGolfBaselineTrainingGraph, ParameterGolfBatchGeometry,
    ParameterGolfBf16MasterWeightStepReceipt, ParameterGolfFinalArtifactConfig,
    ParameterGolfMatrixExecutionMode, ParameterGolfOptimizerExecution,
    ParameterGolfOptimizerGroupKind, ParameterGolfOptimizerPlan,
    ParameterGolfReferenceTrainingError, ParameterGolfSingleH100BringupError,
    ParameterGolfSingleH100ChallengeThresholds, ParameterGolfSingleH100MachineObservation,
    ParameterGolfTrainError, ParameterGolfTrainingHyperparameters, TrainingOptimizerConfig,
    TrainingOptimizerState, TrainingPrecisionMode, PARAMETER_GOLF_SINGLE_H100_DATASET_REF,
    PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION, PARAMETER_GOLF_SINGLE_H100_VARIANT,
};
#[cfg(test)]
use crate::{
    build_parameter_golf_baseline_eval_graph, build_parameter_golf_baseline_training_graph,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleH100ModelVariant {
    #[default]
    BaselineSp1024_9x512,
    CompetitiveHomegolfV1,
}

impl ParameterGolfSingleH100ModelVariant {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselineSp1024_9x512 => "baseline_sp1024_9x512",
            Self::CompetitiveHomegolfV1 => "competitive_homegolf_v1",
        }
    }

    #[must_use]
    pub fn model_config(self) -> ParameterGolfConfig {
        match self {
            Self::BaselineSp1024_9x512 => ParameterGolfConfig::baseline_sp1024_9x512(),
            Self::CompetitiveHomegolfV1 => ParameterGolfConfig::competitive_homegolf_v1(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        match value {
            "baseline" | "baseline_sp1024_9x512" => Ok(Self::BaselineSp1024_9x512),
            "competitive" | "competitive_homegolf_v1" => Ok(Self::CompetitiveHomegolfV1),
            other => Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "unsupported parameter golf model variant `{other}`, expected baseline_sp1024_9x512 or competitive_homegolf_v1"
                ),
            }),
        }
    }
}

fn default_single_h100_model_config() -> ParameterGolfConfig {
    ParameterGolfSingleH100ModelVariant::default().model_config()
}

const HOMEGOLF_LOCAL_CUDA_GRAD_ACCUM_STEPS: usize = 32;
const HOMEGOLF_LOCAL_CUDA_VALIDATION_BATCH_SEQUENCES: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleDeviceMachineProfile {
    #[default]
    SingleH100,
    HomegolfLocalCuda,
}

impl ParameterGolfSingleDeviceMachineProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleH100 => "single_h100",
            Self::HomegolfLocalCuda => "homegolf_local_cuda",
        }
    }

    #[must_use]
    pub fn machine_thresholds(self) -> ParameterGolfSingleH100ChallengeThresholds {
        match self {
            Self::SingleH100 => ParameterGolfSingleH100ChallengeThresholds::challenge_h100(),
            Self::HomegolfLocalCuda => {
                ParameterGolfSingleH100ChallengeThresholds::homegolf_local_cuda()
            }
        }
    }

    #[must_use]
    const fn execution_summary_label(self) -> &'static str {
        match self {
            Self::SingleH100 => "single-H100",
            Self::HomegolfLocalCuda => "HOMEGOLF local-CUDA single-device",
        }
    }

    #[must_use]
    const fn claim_boundary(self) -> &'static str {
        match self {
            Self::SingleH100 => {
                "bounded_single_h100_accelerated_trainer; challenge geometry and optimizer contract are real, but this report does not claim 8xH100, record-track, or challenge-speed closure"
            }
            Self::HomegolfLocalCuda => {
                "homegolf_local_cuda_single_device_accelerated_trainer; challenge geometry, exact FineWeb/SP1024 data, and contest-style final scoring are real, but this report claims one HOMEGOLF local-CUDA 10-minute run rather than public single-H100, 8xH100, or leaderboard-equivalent closure"
            }
        }
    }

    #[must_use]
    const fn refusal_claim_boundary(self) -> &'static str {
        match self {
            Self::SingleH100 => {
                "single_h100_accelerated_trainer_refusal; dataset and model contracts are bound, but no training artifact is claimed when machine or CUDA blocker admission fails"
            }
            Self::HomegolfLocalCuda => {
                "homegolf_local_cuda_single_device_refusal; dataset and model contracts are bound, but no HOMEGOLF local-CUDA training artifact is claimed when machine or CUDA blocker admission fails"
            }
        }
    }
}

/// Config for the bounded Rust-owned single-H100 Parameter Golf trainer lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100TrainingConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Local dataset root containing cached FineWeb/SP1024 shards.
    pub dataset_root: PathBuf,
    /// Local tokenizer artifact path.
    pub tokenizer_path: PathBuf,
    /// Stable dataset identity expected by the run.
    pub dataset_key: DatasetKey,
    /// Stable tokenizer or dataset variant label.
    pub variant: String,
    /// Explicit model family variant surfaced into the exact HOMEGOLF lane.
    #[serde(default)]
    pub model_variant: ParameterGolfSingleH100ModelVariant,
    /// Explicit machine-admission profile for this single-device trainer run.
    #[serde(default)]
    pub machine_profile: ParameterGolfSingleDeviceMachineProfile,
    /// Exact model config for this trainer execution.
    #[serde(default = "default_single_h100_model_config")]
    pub model_config: ParameterGolfConfig,
    /// Public single-device challenge geometry.
    pub geometry: ParameterGolfBatchGeometry,
    /// Public baseline optimization contract.
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
    /// Explicit optimizer-step cap for this run.
    pub max_steps: u64,
    /// Warmup steps used to prime the runtime before measured training starts.
    pub warmup_steps: u64,
    /// Validation cadence in optimizer steps. Zero disables periodic validation.
    pub validation_loss_every: u64,
    /// Train-log cadence in optimizer steps. Zero disables train-step summaries.
    pub train_log_every: u64,
    /// Explicit final-validation posture for the live-model and roundtrip passes.
    #[serde(default)]
    pub final_validation_mode: ParameterGolfSingleH100ValidationMode,
    /// Explicit evaluation semantics for live and roundtrip validation.
    #[serde(default)]
    pub validation_eval_mode: ParameterGolfValidationEvalMode,
    /// Explicit rank-local evaluation batch geometry, independent from train batching.
    #[serde(default = "default_single_h100_validation_batch_sequences")]
    pub validation_batch_sequences: usize,
    /// Explicit matrix-execution posture for the banked PGOLF runtime surface.
    #[serde(default = "default_parameter_golf_single_h100_matrix_execution_mode")]
    pub matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    /// Optional legal score-first TTT overlay for the final validation passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_first_ttt: Option<ParameterGolfScoreFirstTttConfig>,
    /// Optional EMA posture for the final exported model surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ema: Option<ParameterGolfEmaConfig>,
    /// Optional SWA posture for the final exported model surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swa: Option<ParameterGolfSwaConfig>,
    /// Explicit final model surface to validate and export after training.
    #[serde(default)]
    pub final_model_surface: ParameterGolfFinalModelSurface,
    /// Explicit final-artifact quantization and compression posture.
    #[serde(default)]
    pub final_artifact_config: ParameterGolfFinalArtifactConfig,
}

impl ParameterGolfSingleH100TrainingConfig {
    /// Returns the canonical challenge-baseline single-H100 training defaults.
    #[must_use]
    pub fn challenge_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        Self {
            run_id: String::from("parameter-golf-single-h100-trainer"),
            dataset_root: dataset_root.into(),
            tokenizer_path: tokenizer_path.into(),
            dataset_key: DatasetKey::new(
                PARAMETER_GOLF_SINGLE_H100_DATASET_REF,
                PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION,
            ),
            variant: String::from(PARAMETER_GOLF_SINGLE_H100_VARIANT),
            model_variant: ParameterGolfSingleH100ModelVariant::BaselineSp1024_9x512,
            machine_profile: ParameterGolfSingleDeviceMachineProfile::SingleH100,
            model_config: ParameterGolfConfig::baseline_sp1024_9x512(),
            geometry: ParameterGolfBatchGeometry::challenge_single_device_defaults(),
            max_steps: hyperparameters.iterations,
            warmup_steps: 20,
            validation_loss_every: 1_000,
            train_log_every: 200,
            final_validation_mode: ParameterGolfSingleH100ValidationMode::Both,
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: parameter_golf_default_validation_batch_sequences(
                &ParameterGolfBatchGeometry::challenge_single_device_defaults(),
                &ParameterGolfValidationEvalMode::NonOverlapping,
            ),
            matrix_execution_mode: default_parameter_golf_single_h100_matrix_execution_mode(),
            score_first_ttt: None,
            ema: None,
            swa: None,
            final_model_surface: ParameterGolfFinalModelSurface::Raw,
            final_artifact_config: ParameterGolfFinalArtifactConfig::default(),
            hyperparameters,
        }
    }

    /// Returns the old explicit bounded proof posture for one short bring-up run.
    #[must_use]
    pub fn bounded_proof_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
        max_steps: u64,
    ) -> Self {
        let mut config = Self::challenge_defaults(dataset_root, tokenizer_path);
        config.max_steps = max_steps;
        config.warmup_steps = 0;
        config.validation_loss_every = 0;
        config.train_log_every = 1;
        config.final_validation_mode = ParameterGolfSingleH100ValidationMode::RoundtripOnly;
        config.validation_eval_mode = ParameterGolfValidationEvalMode::NonOverlapping;
        config.score_first_ttt = None;
        config.ema = None;
        config.swa = None;
        config.final_model_surface = ParameterGolfFinalModelSurface::Raw;
        config.final_artifact_config = ParameterGolfFinalArtifactConfig::default();
        config.hyperparameters.max_wallclock_seconds = None;
        config
    }

    #[must_use]
    pub fn challenge_competitive_homegolf_v1_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        let mut config = Self::challenge_defaults(dataset_root, tokenizer_path);
        config.apply_model_variant(ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1);
        config.validation_eval_mode = ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 };
        config.validation_batch_sequences = parameter_golf_default_validation_batch_sequences(
            &config.geometry,
            &config.validation_eval_mode,
        );
        config.score_first_ttt = Some(ParameterGolfScoreFirstTttConfig::leaderboard_defaults());
        config.ema = Some(ParameterGolfEmaConfig::default());
        config.swa = Some(ParameterGolfSwaConfig {
            source_surface: ParameterGolfSwaSourceSurface::Ema,
            ..Default::default()
        });
        config.final_model_surface = ParameterGolfFinalModelSurface::Swa;
        config.final_artifact_config = ParameterGolfFinalArtifactConfig::competitive_defaults();
        config
    }

    #[must_use]
    pub fn challenge_homegolf_local_cuda_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        let mut config = Self::challenge_defaults(dataset_root, tokenizer_path);
        config.apply_homegolf_local_cuda_profile();
        config
    }

    #[must_use]
    pub fn challenge_competitive_homegolf_v1_local_cuda_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        let mut config =
            Self::challenge_competitive_homegolf_v1_defaults(dataset_root, tokenizer_path);
        config.apply_homegolf_local_cuda_profile();
        config
    }

    pub fn apply_homegolf_local_cuda_profile(&mut self) {
        self.machine_profile = ParameterGolfSingleDeviceMachineProfile::HomegolfLocalCuda;
        self.warmup_steps = 0;
        self.geometry.grad_accum_steps = HOMEGOLF_LOCAL_CUDA_GRAD_ACCUM_STEPS;
        self.validation_batch_sequences = HOMEGOLF_LOCAL_CUDA_VALIDATION_BATCH_SEQUENCES;
    }

    pub fn apply_model_variant(&mut self, model_variant: ParameterGolfSingleH100ModelVariant) {
        self.model_variant = model_variant;
        self.model_config = model_variant.model_config();
    }

    fn validate(&self) -> Result<(), ParameterGolfSingleH100TrainingError> {
        if self.run_id.trim().is_empty() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("run_id must be non-empty"),
            });
        }
        if self.variant.trim().is_empty() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("variant must be non-empty"),
            });
        }
        if self.max_steps == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("max_steps must be positive"),
            });
        }
        if self.hyperparameters.iterations == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("hyperparameter iterations must be positive"),
            });
        }
        if self.max_steps > self.hyperparameters.iterations {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "max_steps={} must not exceed hyperparameter iterations={}",
                    self.max_steps, self.hyperparameters.iterations
                ),
            });
        }
        let expected_geometry = ParameterGolfBatchGeometry::challenge_single_device_defaults();
        match self.machine_profile {
            ParameterGolfSingleDeviceMachineProfile::SingleH100 => {
                if self.geometry != expected_geometry {
                    return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: String::from(
                            "single-H100 training requires challenge_single_device_defaults geometry",
                        ),
                    });
                }
            }
            ParameterGolfSingleDeviceMachineProfile::HomegolfLocalCuda => {
                if self.geometry.world_size != expected_geometry.world_size
                    || self.geometry.train_batch_tokens != expected_geometry.train_batch_tokens
                    || self.geometry.validation_batch_tokens
                        != expected_geometry.validation_batch_tokens
                    || self.geometry.train_sequence_length
                        != expected_geometry.train_sequence_length
                {
                    return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: String::from(
                            "homegolf_local_cuda training must preserve the challenge world_size, total train_batch_tokens, total validation_batch_tokens, and sequence_length while only changing the local grad_accum_steps fit profile",
                        ),
                    });
                }
            }
        }
        self.geometry.validate().map_err(|error| {
            ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!("invalid batch geometry for single-device trainer: {error}"),
            }
        })?;
        self.model_config.validate().map_err(|error| {
            ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!("model_config is invalid for the trainer lane: {error}"),
            }
        })?;
        if self.model_config != self.model_variant.model_config() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from(
                    "model_config drifted from the selected model_variant contract",
                ),
            });
        }
        if !self.dataset_root.is_dir() {
            return Err(ParameterGolfSingleH100TrainingError::MissingPath {
                path: self.dataset_root.display().to_string(),
                expected: String::from("dataset directory"),
            });
        }
        if !self.tokenizer_path.is_file() {
            return Err(ParameterGolfSingleH100TrainingError::MissingPath {
                path: self.tokenizer_path.display().to_string(),
                expected: String::from("tokenizer file"),
            });
        }
        self.validation_eval_mode
            .validate(self.geometry.train_sequence_length)?;
        if self.validation_batch_sequences == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("validation_batch_sequences must be positive"),
            });
        }
        if let Some(score_first_ttt) = self.score_first_ttt.as_ref() {
            score_first_ttt.validate(self.geometry.train_sequence_length)?;
            match self.validation_eval_mode {
                ParameterGolfValidationEvalMode::SlidingWindow { stride }
                    if stride == score_first_ttt.stride => {}
                ParameterGolfValidationEvalMode::SlidingWindow { stride } => {
                    return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: format!(
                            "score-first TTT requires validation_eval_mode=sliding_window:{} but observed sliding_window:{}",
                            score_first_ttt.stride, stride
                        ),
                    });
                }
                ParameterGolfValidationEvalMode::NonOverlapping => {
                    return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: String::from(
                            "score-first TTT requires sliding-window validation semantics",
                        ),
                    });
                }
            }
        }
        if let Some(ema) = self.ema.as_ref() {
            ema.validate()?;
        }
        if let Some(swa) = self.swa.as_ref() {
            swa.validate()?;
            if swa.source_surface == ParameterGolfSwaSourceSurface::Ema && self.ema.is_none() {
                return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: String::from("swa source_surface=ema requires an explicit ema config"),
                });
            }
        }
        if self.final_model_surface == ParameterGolfFinalModelSurface::Ema && self.ema.is_none() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("final_model_surface=ema requires an explicit ema config"),
            });
        }
        if self.final_model_surface == ParameterGolfFinalModelSurface::Swa && self.swa.is_none() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("final_model_surface=swa requires an explicit swa config"),
            });
        }
        Ok(())
    }
}

/// Explicit final-model surface evaluated and exported after training.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfFinalModelSurface {
    Raw,
    Ema,
    Swa,
}

impl Default for ParameterGolfFinalModelSurface {
    fn default() -> Self {
        Self::Raw
    }
}

impl ParameterGolfFinalModelSurface {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Ema => "ema",
            Self::Swa => "swa",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        match value {
            "raw" => Ok(Self::Raw),
            "ema" => Ok(Self::Ema),
            "swa" => Ok(Self::Swa),
            actual => Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "unsupported final model surface `{actual}`, expected one of raw, ema, or swa"
                ),
            }),
        }
    }
}

/// Explicit EMA posture for the PGOLF final exported model surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfEmaConfig {
    pub decay: f32,
}

impl Default for ParameterGolfEmaConfig {
    fn default() -> Self {
        Self { decay: 0.997 }
    }
}

impl ParameterGolfEmaConfig {
    fn validate(&self) -> Result<(), ParameterGolfSingleH100TrainingError> {
        if !self.decay.is_finite() || !(0.0..1.0).contains(&self.decay) {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "ema decay must be finite and inside [0, 1), observed {}",
                    self.decay
                ),
            });
        }
        Ok(())
    }
}

/// Explicit source surface used by SWA snapshot collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSwaSourceSurface {
    #[default]
    Raw,
    Ema,
}

impl ParameterGolfSwaSourceSurface {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Ema => "ema",
        }
    }
}

/// Explicit SWA posture for the PGOLF final exported model surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSwaConfig {
    pub every_steps: u64,
    pub max_learning_rate_multiplier: f32,
    #[serde(default)]
    pub source_surface: ParameterGolfSwaSourceSurface,
}

impl Default for ParameterGolfSwaConfig {
    fn default() -> Self {
        Self {
            every_steps: 50,
            max_learning_rate_multiplier: 0.2,
            source_surface: ParameterGolfSwaSourceSurface::Raw,
        }
    }
}

impl ParameterGolfSwaConfig {
    fn validate(&self) -> Result<(), ParameterGolfSingleH100TrainingError> {
        if self.every_steps == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("swa every_steps must be positive"),
            });
        }
        if !self.max_learning_rate_multiplier.is_finite()
            || !(0.0..=1.0).contains(&self.max_learning_rate_multiplier)
        {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "swa max_learning_rate_multiplier must be finite and inside [0, 1], observed {}",
                    self.max_learning_rate_multiplier
                ),
            });
        }
        Ok(())
    }
}

/// One machine-readable SWA outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSwaReceipt {
    pub config: ParameterGolfSwaConfig,
    pub sample_count: u64,
}

/// Observed current-state disposition for the bounded single-H100 trainer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleH100TrainingDisposition {
    RefusedMachineContract,
    RefusedCudaBlockers,
    RefusedWallclockProjection,
    TrainingExecuted,
}

/// Explicit final-validation posture for the single-H100 trainer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleH100ValidationMode {
    LiveOnly,
    RoundtripOnly,
    ArtifactOnly,
    #[default]
    Both,
}

impl ParameterGolfSingleH100ValidationMode {
    /// Parses one CLI-visible validation mode label.
    pub fn parse(value: &str) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        match value {
            "live_only" => Ok(Self::LiveOnly),
            "roundtrip_only" => Ok(Self::RoundtripOnly),
            "artifact_only" => Ok(Self::ArtifactOnly),
            "both" => Ok(Self::Both),
            actual => Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "unsupported final validation mode `{actual}`, expected one of live_only, roundtrip_only, artifact_only, or both"
                ),
            }),
        }
    }

    /// Stable string label for logs, reports, and CLI wiring.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LiveOnly => "live_only",
            Self::RoundtripOnly => "roundtrip_only",
            Self::ArtifactOnly => "artifact_only",
            Self::Both => "both",
        }
    }

    #[must_use]
    const fn runs_live_validation(self) -> bool {
        matches!(self, Self::LiveOnly | Self::Both)
    }

    #[must_use]
    const fn runs_roundtrip_validation(self) -> bool {
        matches!(self, Self::RoundtripOnly | Self::Both)
    }
}

#[must_use]
fn should_run_live_validation_checkpoint(
    step: u64,
    last_step: bool,
    validation_loss_every: u64,
    final_validation_mode: ParameterGolfSingleH100ValidationMode,
    final_model_surface: ParameterGolfFinalModelSurface,
) -> bool {
    if last_step {
        final_validation_mode.runs_live_validation()
            && final_model_surface == ParameterGolfFinalModelSurface::Raw
    } else {
        validation_loss_every > 0 && step > 0 && step % validation_loss_every == 0
    }
}

/// Explicit evaluation semantics for Parameter Golf validation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ParameterGolfValidationEvalMode {
    #[default]
    NonOverlapping,
    SlidingWindow {
        stride: usize,
    },
}

impl ParameterGolfValidationEvalMode {
    /// Parses one CLI-visible validation eval mode label.
    pub fn parse(value: &str) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        if value == "non_overlapping" {
            return Ok(Self::NonOverlapping);
        }
        if let Some(stride) = value.strip_prefix("sliding_window:") {
            return Ok(Self::SlidingWindow {
                stride: stride.parse::<usize>().map_err(|error| {
                    ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: format!("invalid sliding_window stride `{stride}`: {error}"),
                    }
                })?,
            });
        }
        Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: format!(
                "unsupported validation eval mode `{value}`, expected non_overlapping or sliding_window:<stride>"
            ),
        })
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NonOverlapping => "non_overlapping",
            Self::SlidingWindow { .. } => "sliding_window",
        }
    }

    pub(crate) fn validate(
        &self,
        sequence_length: usize,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        match self {
            Self::NonOverlapping => Ok(()),
            Self::SlidingWindow { stride } => {
                if *stride == 0 || *stride > sequence_length {
                    return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: format!(
                            "sliding-window validation stride must be in 1..={sequence_length}, observed {stride}"
                        ),
                    });
                }
                Ok(())
            }
        }
    }
}

pub const PARAMETER_GOLF_SCOREBOARD_VALIDATION_BATCH_SEQUENCES: usize = 1024;

#[must_use]
pub fn parameter_golf_default_validation_batch_sequences(
    geometry: &ParameterGolfBatchGeometry,
    eval_mode: &ParameterGolfValidationEvalMode,
) -> usize {
    match eval_mode {
        ParameterGolfValidationEvalMode::NonOverlapping => {
            geometry.local_validation_batch_sequences()
        }
        ParameterGolfValidationEvalMode::SlidingWindow { .. } => {
            PARAMETER_GOLF_SCOREBOARD_VALIDATION_BATCH_SEQUENCES
        }
    }
}

fn default_single_h100_validation_batch_sequences() -> usize {
    parameter_golf_default_validation_batch_sequences(
        &ParameterGolfBatchGeometry::challenge_single_device_defaults(),
        &ParameterGolfValidationEvalMode::NonOverlapping,
    )
}

fn default_parameter_golf_single_h100_matrix_execution_mode() -> ParameterGolfMatrixExecutionMode {
    ParameterGolfMatrixExecutionMode::DirectBanked
}

/// Explicit legal score-first TTT configuration layered on top of
/// sliding-window validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfScoreFirstTttConfig {
    /// Sliding-window stride used during the score phase.
    pub stride: usize,
    /// Contiguous validation tokens assigned to one TTT chunk.
    pub chunk_tokens: usize,
    /// Number of adaptation epochs per scored chunk.
    pub epochs: usize,
    /// Number of leading transformer blocks frozen during adaptation.
    pub freeze_blocks: usize,
    /// Scalar SGD learning rate.
    pub learning_rate: f32,
    /// SGD momentum.
    pub momentum: f32,
    /// Number of training sequences per adaptation batch.
    pub batch_sequences: usize,
    /// Global grad clip norm applied before each adaptation step.
    pub grad_clip_norm: f32,
}

impl ParameterGolfScoreFirstTttConfig {
    /// Returns the current leaderboard-facing legal score-first TTT defaults.
    #[must_use]
    pub fn leaderboard_defaults() -> Self {
        Self {
            stride: 64,
            chunk_tokens: 32_768,
            epochs: 3,
            freeze_blocks: 0,
            learning_rate: 0.002,
            momentum: 0.9,
            batch_sequences: 32,
            grad_clip_norm: 1.0,
        }
    }

    /// Parses one CLI-visible score-first TTT label.
    pub fn parse(value: &str) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        if value == "score_first_ttt" || value == "legal_score_first_ttt" {
            return Ok(Self::leaderboard_defaults());
        }
        let payload = value
            .strip_prefix("score_first_ttt:")
            .or_else(|| value.strip_prefix("legal_score_first_ttt:"))
            .ok_or_else(|| ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "unsupported score-first TTT label `{value}`, expected score_first_ttt or score_first_ttt:key=value,..."
                ),
            })?;
        let mut config = Self::leaderboard_defaults();
        for entry in payload.split(',').filter(|entry| !entry.trim().is_empty()) {
            let (key, raw_value) = entry.split_once('=').ok_or_else(|| {
                ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: format!("invalid score-first TTT entry `{entry}`, expected key=value"),
                }
            })?;
            match key.trim() {
                "stride" => {
                    config.stride = raw_value.parse::<usize>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT stride `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "chunk_tokens" => {
                    config.chunk_tokens = raw_value.parse::<usize>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT chunk_tokens `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "epochs" => {
                    config.epochs = raw_value.parse::<usize>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT epochs `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "freeze_blocks" => {
                    config.freeze_blocks = raw_value.parse::<usize>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT freeze_blocks `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "learning_rate" | "lr" => {
                    config.learning_rate = raw_value.parse::<f32>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT learning_rate `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "momentum" => {
                    config.momentum = raw_value.parse::<f32>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT momentum `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "batch_sequences" => {
                    config.batch_sequences = raw_value.parse::<usize>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT batch_sequences `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                "grad_clip_norm" => {
                    config.grad_clip_norm = raw_value.parse::<f32>().map_err(|error| {
                        ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "invalid score-first TTT grad_clip_norm `{raw_value}`: {error}"
                            ),
                        }
                    })?;
                }
                other => {
                    return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                        message: format!("unsupported score-first TTT key `{other}` in `{value}`"),
                    });
                }
            }
        }
        Ok(config)
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        "legal_score_first_ttt"
    }

    pub(crate) fn validate(
        &self,
        sequence_length: usize,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        if self.stride == 0 || self.stride > sequence_length {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "score-first TTT stride must be in 1..={sequence_length}, observed {}",
                    self.stride
                ),
            });
        }
        if self.chunk_tokens == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("score-first TTT chunk_tokens must be positive"),
            });
        }
        if self.batch_sequences == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("score-first TTT batch_sequences must be positive"),
            });
        }
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "score-first TTT learning_rate must be finite and positive, observed {}",
                    self.learning_rate
                ),
            });
        }
        if !self.momentum.is_finite() || !(0.0..=1.0).contains(&self.momentum) {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "score-first TTT momentum must be finite and within 0..=1, observed {}",
                    self.momentum
                ),
            });
        }
        if !self.grad_clip_norm.is_finite() || self.grad_clip_norm <= 0.0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "score-first TTT grad_clip_norm must be finite and positive, observed {}",
                    self.grad_clip_norm
                ),
            });
        }
        Ok(())
    }
}

/// One planned score-first TTT chunk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfScoreFirstTttChunkPlan {
    pub chunk_index: usize,
    pub chunk_start_token: usize,
    pub chunk_end_token: usize,
    pub first_window_start: Option<usize>,
    pub last_window_start: Option<usize>,
    pub score_window_count: usize,
}

/// One adaptation step executed inside one score-first TTT chunk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfScoreFirstTttAdaptationStepReceipt {
    pub epoch_index: usize,
    pub batch_index: usize,
    pub train_sequence_start: usize,
    pub train_sequence_count: usize,
    pub train_token_count: u64,
    pub mean_loss: f32,
    pub learning_rate: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient_norm_after_clip: Option<f32>,
    #[serde(default)]
    pub clip_applied: bool,
    #[serde(default)]
    pub non_finite_gradient_count: u32,
    pub observed_ms: u64,
}

/// One executed score-first TTT chunk receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfScoreFirstTttChunkReceipt {
    pub plan: ParameterGolfScoreFirstTttChunkPlan,
    pub score_summary: ParameterGolfSingleH100ValidationSummary,
    pub adaptation_applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptation_learning_rate: Option<f32>,
    #[serde(default)]
    pub adaptation_sequence_count: usize,
    #[serde(default)]
    pub adaptation_token_count: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adaptation_steps: Vec<ParameterGolfScoreFirstTttAdaptationStepReceipt>,
}

/// Machine-readable receipt for one legal score-first TTT validation pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfScoreFirstTttReceipt {
    pub path: String,
    pub config: ParameterGolfScoreFirstTttConfig,
    pub total_chunks: usize,
    pub total_score_window_count: usize,
    pub total_adaptation_step_count: usize,
    pub last_chunk_training_skipped: bool,
    pub chunk_receipts: Vec<ParameterGolfScoreFirstTttChunkReceipt>,
}

/// Machine-readable phase timings for one challenge-step aggregate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100PhaseTimings {
    pub window_planning_ms: u64,
    pub token_materialization_ms: u64,
    pub embedding_gather_ms: u64,
    pub forward_loss_cuda_ms: u64,
    pub retained_forward_readback_ms: u64,
    pub backward_cuda_ms: u64,
    pub gradient_readback_ms: u64,
    pub host_gradient_materialization_ms: u64,
    pub optimizer_step_ms: u64,
    pub retained_binding_tensor_count: u32,
    pub retained_binding_f32_count: u64,
    pub gradient_tensor_count: u32,
    pub gradient_f32_count: u64,
}

impl ParameterGolfSingleH100PhaseTimings {
    fn accumulate(&mut self, other: &Self) {
        self.window_planning_ms = self
            .window_planning_ms
            .saturating_add(other.window_planning_ms);
        self.token_materialization_ms = self
            .token_materialization_ms
            .saturating_add(other.token_materialization_ms);
        self.embedding_gather_ms = self
            .embedding_gather_ms
            .saturating_add(other.embedding_gather_ms);
        self.forward_loss_cuda_ms = self
            .forward_loss_cuda_ms
            .saturating_add(other.forward_loss_cuda_ms);
        self.retained_forward_readback_ms = self
            .retained_forward_readback_ms
            .saturating_add(other.retained_forward_readback_ms);
        self.backward_cuda_ms = self.backward_cuda_ms.saturating_add(other.backward_cuda_ms);
        self.gradient_readback_ms = self
            .gradient_readback_ms
            .saturating_add(other.gradient_readback_ms);
        self.host_gradient_materialization_ms = self
            .host_gradient_materialization_ms
            .saturating_add(other.host_gradient_materialization_ms);
        self.optimizer_step_ms = self
            .optimizer_step_ms
            .saturating_add(other.optimizer_step_ms);
        self.retained_binding_tensor_count = self
            .retained_binding_tensor_count
            .saturating_add(other.retained_binding_tensor_count);
        self.retained_binding_f32_count = self
            .retained_binding_f32_count
            .saturating_add(other.retained_binding_f32_count);
        self.gradient_tensor_count = self
            .gradient_tensor_count
            .saturating_add(other.gradient_tensor_count);
        self.gradient_f32_count = self
            .gradient_f32_count
            .saturating_add(other.gradient_f32_count);
    }

    fn host_materialization_ms(&self) -> u64 {
        self.embedding_gather_ms
            .saturating_add(self.retained_forward_readback_ms)
            .saturating_add(self.gradient_readback_ms)
            .saturating_add(self.host_gradient_materialization_ms)
    }
}

/// Reusable one-window gradient result for Parameter Golf training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ParameterGolfTrainingGradientBatchResult {
    pub window_id: String,
    pub loss: f32,
    pub phase_timings: ParameterGolfSingleH100PhaseTimings,
    pub parameter_gradients: BTreeMap<String, Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<ParameterGolfTrainingBatchRuntime>,
}

/// Per-step machine-readable training metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100TrainingStepMetrics {
    pub global_step: u64,
    pub train_window_ids: Vec<String>,
    pub mean_microbatch_loss: f32,
    pub learning_rate_multiplier: f32,
    pub muon_momentum: f32,
    pub observed_wallclock_ms: u64,
    pub phase_timings: ParameterGolfSingleH100PhaseTimings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_learning_rate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient_norm_after_clip: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_norm_after_step: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_norm: Option<f32>,
    #[serde(default)]
    pub clip_applied: bool,
    #[serde(default)]
    pub non_finite_gradient_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples_per_second_milli: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<ParameterGolfSingleH100TrainingRuntimeReceipt>,
}

/// Machine-readable runtime posture for one training step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100TrainingRuntimeReceipt {
    pub path: String,
    pub graph_surface: String,
    #[serde(default = "default_parameter_golf_single_h100_matrix_execution_mode")]
    pub matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    pub session_count: usize,
    pub persistent_parameter_buffer_count: usize,
    pub persistent_parameter_value_count: u64,
    pub resident_parameter_upload_us: u64,
    pub parameter_refresh_us: u64,
    pub reusable_input_token_buffer: bool,
    pub reusable_target_token_buffer: bool,
    pub total_input_token_write_us: u64,
    pub total_target_token_write_us: u64,
    pub resident_parameter_buffers_reused: bool,
}

/// Machine-readable validation summary for the accelerated single-H100 lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ValidationSummary {
    #[serde(default)]
    pub eval_mode: ParameterGolfValidationEvalMode,
    pub evaluated_sequence_count: usize,
    pub evaluated_token_count: u64,
    pub evaluated_byte_count: u64,
    pub mean_loss: f64,
    pub bits_per_byte: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<ParameterGolfSingleH100ValidationRuntimeReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_first_ttt_receipt: Option<ParameterGolfScoreFirstTttReceipt>,
}

/// Machine-readable runtime posture for one validation pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ValidationRuntimeReceipt {
    pub path: String,
    pub graph_surface: String,
    #[serde(default = "default_parameter_golf_single_h100_matrix_execution_mode")]
    pub matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    #[serde(default)]
    pub eval_mode: ParameterGolfValidationEvalMode,
    #[serde(default = "default_single_h100_validation_batch_sequences")]
    pub local_batch_sequences: usize,
    pub session_count: usize,
    pub total_batches: usize,
    #[serde(default)]
    pub total_units: usize,
    pub persistent_parameter_buffer_count: usize,
    pub persistent_parameter_value_count: u64,
    pub resident_parameter_upload_us: u64,
    pub per_batch_stable_parameter_buffer_allocations: u64,
    pub reusable_input_token_buffer: bool,
    pub reusable_target_token_buffer: bool,
    pub total_input_token_write_us: u64,
    pub total_target_token_write_us: u64,
    pub byte_accounting_mode: String,
    pub total_byte_accounting_us: u64,
}

/// One preserved validation observation from the widened single-H100 control loop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ValidationCheckpoint {
    pub stage_label: String,
    pub trigger_step: u64,
    pub observed_training_time_ms: u64,
    pub observed_validation_ms: u64,
    pub summary: ParameterGolfSingleH100ValidationSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100RoundtripReceipt {
    #[serde(default)]
    pub final_model_surface: ParameterGolfFinalModelSurface,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ema: Option<ParameterGolfEmaConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swa: Option<ParameterGolfSwaReceipt>,
    #[serde(default)]
    pub artifact_config: ParameterGolfFinalArtifactConfig,
    pub metric_source: String,
    pub validation: ParameterGolfSingleH100ValidationSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_ttt_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    pub observed_eval_ms: u64,
    pub compressed_model_bytes: u64,
    pub compressed_model_artifact_ref: String,
    pub compressed_model_artifact_digest: String,
}

/// Explicit stop reason for a measured single-H100 run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfSingleH100TrainingStopReason {
    StepBudgetReached,
    WallclockCapReached,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100GroupPrecisionReceipt {
    pub group_id: String,
    pub parameter_precision: TrainingPrecisionMode,
    pub gradient_precision: TrainingPrecisionMode,
    pub optimizer_state_precision: TrainingPrecisionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_weight_precision: Option<TrainingPrecisionMode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100PrecisionReceipt {
    pub graph_parameter_upload_precision: TrainingPrecisionMode,
    pub graph_execution_precision: TrainingPrecisionMode,
    pub retained_activation_precision: TrainingPrecisionMode,
    pub group_receipts: Vec<ParameterGolfSingleH100GroupPrecisionReceipt>,
    pub notes: Vec<String>,
}

/// End-to-end machine-readable trainer report for the bounded single-H100 lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100TrainingReport {
    pub schema_version: u32,
    pub scope_window: String,
    pub run_id: String,
    pub dataset_root: PathBuf,
    pub tokenizer_path: PathBuf,
    pub dataset_key: DatasetKey,
    pub variant: String,
    pub tokenizer_digest: psionic_data::TokenizerDigest,
    pub dataset_manifest_digest: String,
    pub train_shard_count: usize,
    pub validation_shard_count: usize,
    pub train_token_count: u64,
    pub validation_token_count: u64,
    pub geometry: ParameterGolfBatchGeometry,
    pub hyperparameters: ParameterGolfTrainingHyperparameters,
    pub max_steps: u64,
    pub warmup_steps: u64,
    pub completed_warmup_steps: u64,
    pub validation_loss_every: u64,
    pub train_log_every: u64,
    #[serde(default)]
    pub final_validation_mode: ParameterGolfSingleH100ValidationMode,
    #[serde(default)]
    pub validation_eval_mode: ParameterGolfValidationEvalMode,
    #[serde(default = "default_single_h100_validation_batch_sequences")]
    pub validation_batch_sequences: usize,
    #[serde(default = "default_parameter_golf_single_h100_matrix_execution_mode")]
    pub matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_first_ttt: Option<ParameterGolfScoreFirstTttConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ema: Option<ParameterGolfEmaConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swa: Option<ParameterGolfSwaConfig>,
    #[serde(default)]
    pub final_model_surface: ParameterGolfFinalModelSurface,
    #[serde(default)]
    pub final_artifact_config: ParameterGolfFinalArtifactConfig,
    pub executed_steps: u64,
    pub stop_reason: Option<ParameterGolfSingleH100TrainingStopReason>,
    pub delivered_execution: DeliveredExecutionContext,
    pub machine_thresholds: ParameterGolfSingleH100ChallengeThresholds,
    pub observed_cuda_health: RuntimeHealth,
    pub cuda_discovery_error: Option<String>,
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    pub matching_h100_device_count: usize,
    pub machine_contract_satisfied: bool,
    pub baseline_model_id: String,
    pub baseline_model_revision: String,
    #[serde(default)]
    pub model_variant: ParameterGolfSingleH100ModelVariant,
    #[serde(default)]
    pub machine_profile: ParameterGolfSingleDeviceMachineProfile,
    #[serde(default = "default_single_h100_model_config")]
    pub model_config: ParameterGolfConfig,
    pub baseline_model_descriptor_digest: String,
    pub optimizer_plan_digest: String,
    pub precision_receipt: ParameterGolfSingleH100PrecisionReceipt,
    pub cuda_training_capability_report_digest: String,
    pub challenge_kernel_blockers: Vec<String>,
    pub validation_checkpoints: Vec<ParameterGolfSingleH100ValidationCheckpoint>,
    pub initial_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    pub pre_export_final_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    pub final_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    pub warmup_observed_ms: u64,
    pub observed_training_time_ms: u64,
    pub pre_export_final_validation_observed_ms: Option<u64>,
    pub final_validation_observed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_swa_receipt: Option<ParameterGolfSwaReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_roundtrip_receipt: Option<ParameterGolfSingleH100RoundtripReceipt>,
    pub compressed_model_bytes: Option<u64>,
    pub compressed_model_artifact_ref: Option<String>,
    pub compressed_model_artifact_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed_model_artifact_path: Option<String>,
    pub step_metrics: Vec<ParameterGolfSingleH100TrainingStepMetrics>,
    pub aggregate_phase_timings: Option<ParameterGolfSingleH100PhaseTimings>,
    pub final_training_cursor: Option<ParameterGolfTokenStreamCursor>,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub observed_wallclock_ms: u64,
    pub disposition: ParameterGolfSingleH100TrainingDisposition,
    pub refusal: Option<PsionicRefusal>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl ParameterGolfSingleH100TrainingReport {
    /// Returns whether the report is an executed trainer proof.
    #[must_use]
    pub fn training_executed(&self) -> bool {
        self.disposition == ParameterGolfSingleH100TrainingDisposition::TrainingExecuted
    }
}

/// Config for a bounded same-node validation-runtime comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ValidationRuntimeComparisonConfig {
    pub run_id: String,
    pub dataset_root: PathBuf,
    pub tokenizer_path: PathBuf,
    pub dataset_key: DatasetKey,
    pub variant: String,
    pub batch_sequences: usize,
    pub sequence_length: usize,
    pub batch_limit: usize,
}

impl ParameterGolfSingleH100ValidationRuntimeComparisonConfig {
    #[must_use]
    pub fn bounded_local_defaults(
        dataset_root: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            run_id: String::from("parameter-golf-validation-runtime-comparison"),
            dataset_root: dataset_root.into(),
            tokenizer_path: tokenizer_path.into(),
            dataset_key: DatasetKey::new(
                PARAMETER_GOLF_SINGLE_H100_DATASET_REF,
                PARAMETER_GOLF_SINGLE_H100_DATASET_VERSION,
            ),
            variant: String::from(PARAMETER_GOLF_SINGLE_H100_VARIANT),
            batch_sequences: 8,
            sequence_length: ParameterGolfBatchGeometry::challenge_single_device_defaults()
                .train_sequence_length,
            batch_limit: 2,
        }
    }

    fn validate(&self) -> Result<(), ParameterGolfSingleH100TrainingError> {
        if self.run_id.trim().is_empty() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("run_id must be non-empty"),
            });
        }
        if self.batch_sequences == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("batch_sequences must be positive"),
            });
        }
        if self.sequence_length == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("sequence_length must be positive"),
            });
        }
        if self.batch_limit == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("batch_limit must be positive"),
            });
        }
        if !self.dataset_root.is_dir() {
            return Err(ParameterGolfSingleH100TrainingError::MissingPath {
                path: self.dataset_root.display().to_string(),
                expected: String::from("dataset directory"),
            });
        }
        if !self.tokenizer_path.is_file() {
            return Err(ParameterGolfSingleH100TrainingError::MissingPath {
                path: self.tokenizer_path.display().to_string(),
                expected: String::from("tokenizer file"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ValidationRuntimeComparisonLaneReceipt {
    pub mean_loss: f64,
    pub bits_per_byte: f64,
    pub observed_ms: u64,
    pub average_batch_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<ParameterGolfSingleH100ValidationRuntimeReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100ValidationRuntimeComparisonReceipt {
    pub schema_version: u32,
    pub scope_window: String,
    pub run_id: String,
    pub dataset_root: PathBuf,
    pub tokenizer_path: PathBuf,
    pub dataset_key: DatasetKey,
    pub variant: String,
    pub tokenizer_digest: psionic_data::TokenizerDigest,
    pub dataset_manifest_digest: String,
    pub batch_sequences: usize,
    pub sequence_length: usize,
    pub batch_limit: usize,
    pub observed_cuda_health: RuntimeHealth,
    pub observed_cuda_devices: Vec<DeviceDescriptor>,
    pub legacy: ParameterGolfSingleH100ValidationRuntimeComparisonLaneReceipt,
    pub device_resident: ParameterGolfSingleH100ValidationRuntimeComparisonLaneReceipt,
    pub summary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ParameterGolfSingleH100TrainerState {
    pub(crate) parameter_states: BTreeMap<String, ParameterGolfParameterState>,
}

#[derive(Clone, Debug)]
struct ParameterGolfEmaState {
    config: ParameterGolfEmaConfig,
    parameter_values: BTreeMap<String, Vec<f32>>,
}

impl ParameterGolfEmaState {
    fn new(config: ParameterGolfEmaConfig, state: &ParameterGolfSingleH100TrainerState) -> Self {
        Self {
            config,
            parameter_values: current_parameter_state_overrides(state),
        }
    }

    fn update(
        &mut self,
        state: &ParameterGolfSingleH100TrainerState,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        let current_values = current_parameter_state_overrides(state);
        for (parameter_id, values) in current_values {
            let ema_values = self
                .parameter_values
                .get_mut(&parameter_id)
                .ok_or_else(
                    || ParameterGolfSingleH100TrainingError::MissingParameterState {
                        parameter_id: parameter_id.clone(),
                    },
                )?;
            if ema_values.len() != values.len() {
                return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: format!(
                        "ema state length mismatch for `{parameter_id}`: {} vs {}",
                        ema_values.len(),
                        values.len()
                    ),
                });
            }
            for (ema_value, current_value) in ema_values.iter_mut().zip(values) {
                *ema_value =
                    (*ema_value * self.config.decay) + (current_value * (1.0 - self.config.decay));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ParameterGolfSwaState {
    config: ParameterGolfSwaConfig,
    parameter_sums: BTreeMap<String, Vec<f32>>,
    sample_count: u64,
}

impl ParameterGolfSwaState {
    fn new(config: ParameterGolfSwaConfig) -> Self {
        Self {
            config,
            parameter_sums: BTreeMap::new(),
            sample_count: 0,
        }
    }

    fn should_sample(&self, step: u64, learning_rate_multiplier: f32) -> bool {
        step > 0
            && step % self.config.every_steps == 0
            && learning_rate_multiplier < self.config.max_learning_rate_multiplier
    }

    fn update_from_overrides(
        &mut self,
        overrides: &BTreeMap<String, Vec<f32>>,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        if self.sample_count == 0 {
            self.parameter_sums = overrides.clone();
            self.sample_count = 1;
            return Ok(());
        }
        for (parameter_id, values) in overrides {
            let Some(sums) = self.parameter_sums.get_mut(parameter_id) else {
                return Err(
                    ParameterGolfSingleH100TrainingError::MissingParameterState {
                        parameter_id: parameter_id.clone(),
                    },
                );
            };
            if sums.len() != values.len() {
                return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: format!(
                        "swa state length mismatch for `{parameter_id}`: {} vs {}",
                        sums.len(),
                        values.len()
                    ),
                });
            }
            for (sum, value) in sums.iter_mut().zip(values) {
                *sum += *value;
            }
        }
        self.sample_count += 1;
        Ok(())
    }

    fn averaged_parameter_values(
        &self,
    ) -> Result<BTreeMap<String, Vec<f32>>, ParameterGolfSingleH100TrainingError> {
        if self.sample_count == 0 {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("swa final surface requested without any live swa samples"),
            });
        }
        Ok(self
            .parameter_sums
            .iter()
            .map(|(parameter_id, sums)| {
                (
                    parameter_id.clone(),
                    sums.iter()
                        .map(|value| *value / self.sample_count as f32)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>())
    }

    fn receipt(&self) -> Option<ParameterGolfSwaReceipt> {
        (self.sample_count > 0).then(|| ParameterGolfSwaReceipt {
            config: self.config.clone(),
            sample_count: self.sample_count,
        })
    }
}

#[derive(Clone, Debug)]
struct ParameterGolfValidationSequencePlan {
    valid_length: usize,
    score_start: usize,
}

#[derive(Clone, Debug)]
struct ParameterGolfValidationBatchPlan {
    batch_sequences: usize,
    evaluation_units: usize,
    input_tokens: Vec<i32>,
    target_tokens: Vec<i32>,
    sequence_plans: Vec<ParameterGolfValidationSequencePlan>,
    token_count: u64,
    byte_count: u64,
}

#[derive(Clone, Debug, Default)]
struct ParameterGolfValidationBatchRuntime {
    input_token_write_us: u64,
    target_token_write_us: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ParameterGolfScoreFirstTttChunkExecutionPlan {
    pub(crate) receipt_plan: ParameterGolfScoreFirstTttChunkPlan,
    pub(crate) window_starts: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct ParameterGolfScoreFirstTttParameterState {
    pub(crate) values: Vec<f32>,
    pub(crate) momentum_buffer: Vec<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ParameterGolfTrainingBatchRuntime {
    pub(crate) resident_parameter_upload_us: u64,
    pub(crate) parameter_refresh_us: u64,
    pub(crate) input_token_write_us: u64,
    pub(crate) target_token_write_us: u64,
    pub(crate) forward_loss_cuda_ms: u64,
    pub(crate) backward_cuda_ms: u64,
    pub(crate) persistent_parameter_buffer_count: usize,
    pub(crate) persistent_parameter_value_count: u64,
    pub(crate) resident_parameter_buffers_reused: bool,
}

#[derive(Clone, Debug)]
struct ParameterGolfCudaValidationSession {
    graph: ParameterGolfBaselineEvalGraph,
    model_config: ParameterGolfConfig,
    static_inputs: BTreeMap<TensorId, CudaBuffer>,
    input_token_buffer: CudaBuffer,
    bigram_token_buffer: Option<CudaBuffer>,
    target_token_buffer: CudaBuffer,
    input_token_staging: Vec<i32>,
    bigram_token_staging: Option<Vec<i32>>,
    target_token_staging: Vec<i32>,
    resident_parameter_upload_us: u64,
    persistent_parameter_buffer_count: usize,
    persistent_parameter_value_count: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ParameterGolfCudaTrainingSession {
    graph: ParameterGolfBaselineTrainingGraph,
    model_config: ParameterGolfConfig,
    backward_plan: psionic_ir::AutodiffBackwardPlan,
    retained_graph: psionic_ir::Graph,
    static_inputs: BTreeMap<TensorId, CudaBuffer>,
    input_token_buffer: CudaBuffer,
    bigram_token_buffer: Option<CudaBuffer>,
    target_token_buffer: CudaBuffer,
    input_token_staging: Vec<i32>,
    bigram_token_staging: Option<Vec<i32>>,
    target_token_staging: Vec<i32>,
    resident_parameter_upload_us_pending: Option<u64>,
    parameter_refresh_us_pending: Option<u64>,
    persistent_parameter_buffer_count: usize,
    persistent_parameter_value_count: u64,
}

#[derive(Clone, Debug)]
pub(crate) enum ParameterGolfParameterState {
    AdamBf16Master {
        shape: Vec<usize>,
        train_visible_values: Vec<f32>,
        train_visible_bf16_bits: Vec<u16>,
        master_weight_values: Vec<f32>,
        optimizer: TrainingOptimizerConfig,
        optimizer_state: TrainingOptimizerState,
        last_step_receipt: Option<ParameterGolfBf16MasterWeightStepReceipt>,
    },
    AdamFp32 {
        shape: Vec<usize>,
        values: Vec<f32>,
        optimizer: TrainingOptimizerConfig,
        optimizer_state: TrainingOptimizerState,
    },
    MuonBf16 {
        shape: Vec<usize>,
        values: Vec<f32>,
        bf16_bits: Vec<u16>,
        optimizer: crate::ParameterGolfMuonConfig,
        optimizer_state: crate::ParameterGolfMuonState,
    },
}

impl ParameterGolfParameterState {
    pub(crate) fn values(&self) -> &[f32] {
        match self {
            Self::AdamBf16Master {
                train_visible_values,
                ..
            } => train_visible_values.as_slice(),
            Self::AdamFp32 { values, .. } | Self::MuonBf16 { values, .. } => values.as_slice(),
        }
    }

    pub(crate) fn shape(&self) -> &[usize] {
        match self {
            Self::AdamBf16Master { shape, .. }
            | Self::AdamFp32 { shape, .. }
            | Self::MuonBf16 { shape, .. } => shape.as_slice(),
        }
    }

    fn precision_receipt(&self, group_id: String) -> ParameterGolfSingleH100GroupPrecisionReceipt {
        match self {
            Self::AdamBf16Master { .. } => ParameterGolfSingleH100GroupPrecisionReceipt {
                group_id,
                parameter_precision: TrainingPrecisionMode::Bf16,
                gradient_precision: TrainingPrecisionMode::Bf16,
                optimizer_state_precision: TrainingPrecisionMode::Fp32,
                master_weight_precision: Some(TrainingPrecisionMode::Fp32),
            },
            Self::AdamFp32 { .. } => ParameterGolfSingleH100GroupPrecisionReceipt {
                group_id,
                parameter_precision: TrainingPrecisionMode::Fp32,
                gradient_precision: TrainingPrecisionMode::Fp32,
                optimizer_state_precision: TrainingPrecisionMode::Fp32,
                master_weight_precision: None,
            },
            Self::MuonBf16 { .. } => ParameterGolfSingleH100GroupPrecisionReceipt {
                group_id,
                parameter_precision: TrainingPrecisionMode::Bf16,
                gradient_precision: TrainingPrecisionMode::Fp32,
                optimizer_state_precision: TrainingPrecisionMode::Fp32,
                master_weight_precision: None,
            },
        }
    }

    fn apply_gradients(
        &mut self,
        gradients: &[f32],
        learning_rate_multiplier: f32,
        muon_momentum: f32,
        step_number: u64,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        match self {
            Self::AdamBf16Master {
                train_visible_values,
                train_visible_bf16_bits,
                master_weight_values,
                optimizer,
                optimizer_state,
                last_step_receipt,
                ..
            } => {
                let mut optimizer = optimizer.clone();
                optimizer.learning_rate *= learning_rate_multiplier;
                let receipt = apply_parameter_golf_cuda_bf16_master_weight_optimizer_step(
                    train_visible_values.as_mut_slice(),
                    master_weight_values.as_mut_slice(),
                    gradients,
                    &optimizer,
                    optimizer_state,
                    step_number,
                )?;
                *train_visible_bf16_bits =
                    bf16_bits_from_f32_values(train_visible_values.as_slice());
                *last_step_receipt = Some(receipt);
                Ok(())
            }
            Self::AdamFp32 {
                values,
                optimizer,
                optimizer_state,
                ..
            } => {
                let mut optimizer = optimizer.clone();
                optimizer.learning_rate *= learning_rate_multiplier;
                optimizer.apply_step(
                    values.as_mut_slice(),
                    gradients,
                    optimizer_state,
                    step_number,
                )?;
                Ok(())
            }
            Self::MuonBf16 {
                shape,
                values,
                bf16_bits,
                optimizer,
                optimizer_state,
            } => {
                let mut optimizer = optimizer.clone();
                optimizer.learning_rate *= learning_rate_multiplier;
                optimizer.momentum = muon_momentum;
                apply_parameter_golf_cuda_muon_step(
                    values.as_mut_slice(),
                    shape.as_slice(),
                    gradients,
                    &optimizer,
                    optimizer_state,
                )?;
                round_values_to_bf16(values.as_mut_slice());
                *bf16_bits = bf16_bits_from_f32_values(values.as_slice());
                Ok(())
            }
        }
    }
}

/// Failure while executing the bounded single-H100 trainer.
#[derive(Debug, Error)]
pub enum ParameterGolfSingleH100TrainingError {
    #[error("parameter golf single-H100 training config is invalid: {message}")]
    InvalidConfig { message: String },
    #[error(
        "parameter golf single-H100 training projected optimizer step {global_step} to exceed the remaining wallclock budget after {observed_micro_steps}/{grad_accum_steps} micro-steps: observed_step_wallclock_ms={observed_step_wallclock_ms} projected_full_step_wallclock_ms={projected_full_step_wallclock_ms} remaining_training_wallclock_ms={remaining_training_wallclock_ms}"
    )]
    ProjectedWallclockCapExceeded {
        global_step: u64,
        observed_micro_steps: usize,
        grad_accum_steps: usize,
        observed_step_wallclock_ms: u64,
        projected_full_step_wallclock_ms: u64,
        remaining_training_wallclock_ms: u64,
    },
    #[error("parameter golf single-H100 training expected {expected} at `{path}`")]
    MissingPath { path: String, expected: String },
    #[error("parameter golf single-H100 training serialization failed: {message}")]
    Serialization { message: String },
    #[error("parameter golf single-H100 training could not select a qualifying H100 CUDA device")]
    MissingSelectedH100,
    #[error("parameter golf single-H100 training graph is missing tensor `{tensor_id}`")]
    MissingGraphTensor { tensor_id: TensorId },
    #[error("parameter golf single-H100 training graph output `{tensor_id}` was not materialized")]
    MissingGraphOutput { tensor_id: TensorId },
    #[error("parameter golf single-H100 training is missing parameter state for `{parameter_id}`")]
    MissingParameterState { parameter_id: String },
    #[error(transparent)]
    Data(#[from] ParameterGolfDataError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
    #[error(transparent)]
    Execution(#[from] ParameterGolfExecutionError),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
    #[error(transparent)]
    BaselineGraph(#[from] crate::ParameterGolfBaselineGraphError),
    #[error(transparent)]
    Bringup(#[from] ParameterGolfSingleH100BringupError),
    #[error(transparent)]
    ReferenceTraining(#[from] ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    Train(#[from] ParameterGolfTrainError),
    #[error(transparent)]
    Optimizer(#[from] crate::TrainingOptimizerError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Visualization(#[from] crate::ParameterGolfSingleH100VisualizationError),
}

/// Writes the bounded single-H100 training report to disk.
pub fn write_parameter_golf_single_h100_training_report(
    output_path: &Path,
    config: &ParameterGolfSingleH100TrainingConfig,
) -> Result<ParameterGolfSingleH100TrainingReport, ParameterGolfSingleH100TrainingError> {
    emit_progress_line(format!(
        "training_report_closeout_start report_path={}",
        output_path.display()
    ));
    let (report, persisted_artifact_path, mut live_visualization_writer) =
        build_parameter_golf_single_h100_training_report_inner(config, Some(output_path))?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(persisted_artifact_path) = persisted_artifact_path.as_ref() {
        emit_progress_line(format!(
            "training_report_closeout_artifact_ready path={}",
            persisted_artifact_path.display()
        ));
    }
    emit_progress_line(String::from("training_report_json_encode_start"));
    let encoded = serde_json::to_vec_pretty(&report).map_err(|error| {
        ParameterGolfSingleH100TrainingError::Serialization {
            message: error.to_string(),
        }
    })?;
    emit_progress_line(format!(
        "training_report_json_encode_complete bytes={}",
        encoded.len()
    ));
    fs::write(output_path, encoded)?;
    emit_progress_line(format!(
        "training_report_json_write_complete report_path={}",
        output_path.display()
    ));
    if let Some(writer) = live_visualization_writer.as_mut() {
        emit_progress_line(String::from("training_report_visualization_finish_start"));
        writer.finish_with_report(&report)?;
        emit_progress_line(String::from(
            "training_report_visualization_finish_complete",
        ));
    }
    emit_progress_line(String::from("training_report_closeout_complete"));
    Ok(report)
}

/// Writes one bounded same-node validation-runtime comparison receipt to disk.
pub fn write_parameter_golf_single_h100_validation_runtime_comparison_receipt(
    output_path: &Path,
    config: &ParameterGolfSingleH100ValidationRuntimeComparisonConfig,
) -> Result<
    ParameterGolfSingleH100ValidationRuntimeComparisonReceipt,
    ParameterGolfSingleH100TrainingError,
> {
    let receipt = build_parameter_golf_single_h100_validation_runtime_comparison_receipt(config)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_vec_pretty(&receipt).map_err(|error| {
        ParameterGolfSingleH100TrainingError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(output_path, encoded)?;
    Ok(receipt)
}

/// Builds one bounded same-node validation-runtime comparison receipt.
pub fn build_parameter_golf_single_h100_validation_runtime_comparison_receipt(
    config: &ParameterGolfSingleH100ValidationRuntimeComparisonConfig,
) -> Result<
    ParameterGolfSingleH100ValidationRuntimeComparisonReceipt,
    ParameterGolfSingleH100TrainingError,
> {
    config.validate()?;
    let tokenizer_bytes = fs::read(&config.tokenizer_path)?;
    let tokenizer_digest = build_tokenizer_digest(tokenizer_bytes.as_slice());
    let bundle = parameter_golf_dataset_bundle_from_local_dir(
        config.dataset_key.clone(),
        &config.dataset_root,
        config.variant.clone(),
        tokenizer_digest.clone(),
        config.tokenizer_path.display().to_string(),
        None,
    )?;
    let validation_tokens = load_parameter_golf_validation_tokens_from_paths(
        &bundle
            .validation_shards
            .iter()
            .map(|receipt| PathBuf::from(&receipt.path))
            .collect::<Vec<_>>(),
        config.sequence_length,
    )?;
    let total_sequences = (validation_tokens.len() - 1) / config.sequence_length;
    let selected_sequences = total_sequences.min(
        config
            .batch_sequences
            .saturating_mul(config.batch_limit.max(1)),
    );
    if selected_sequences == 0 {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: String::from(
                "validation runtime comparison requires at least one complete validation sequence",
            ),
        });
    }
    let selected_token_end = selected_sequences
        .saturating_mul(config.sequence_length)
        .saturating_add(1);
    let selected_validation_tokens = &validation_tokens[..selected_token_end];
    let byte_luts =
        parameter_golf_sentencepiece_byte_luts_from_tokenizer_path(&config.tokenizer_path)?;
    let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
    let machine_observation = inspect_local_single_h100_machine_with_thresholds(
        ParameterGolfSingleH100ChallengeThresholds::challenge_h100(),
    );
    let mut legacy_graph_cache = BTreeMap::new();
    let mut device_resident_graph_cache = BTreeMap::new();
    let mut cuda_backend = CudaBackend::new();
    let Some(selected_device) = cuda_backend.selected_device().cloned() else {
        return Err(ParameterGolfSingleH100TrainingError::MissingSelectedH100);
    };

    let legacy_started = Instant::now();
    let legacy_summary = evaluate_validation_on_cuda_legacy(
        &mut cuda_backend,
        &selected_device.device,
        model.descriptor(),
        &model,
        selected_validation_tokens,
        &byte_luts,
        config.sequence_length,
        config.batch_sequences,
        &mut legacy_graph_cache,
        "legacy_validation_runtime_comparison",
        None,
    )?;
    let legacy_observed_ms = duration_ms(legacy_started);

    let device_resident_started = Instant::now();
    let device_resident_summary = evaluate_validation_on_cuda(
        &mut cuda_backend,
        &selected_device.device,
        model.descriptor(),
        &model,
        selected_validation_tokens,
        &byte_luts,
        config.sequence_length,
        config.batch_sequences,
        &ParameterGolfValidationEvalMode::NonOverlapping,
        &mut device_resident_graph_cache,
        "device_resident_validation_runtime_comparison",
        None,
    )?;
    let device_resident_observed_ms = duration_ms(device_resident_started);
    let selected_batch_count = selected_sequences.div_ceil(config.batch_sequences.max(1));

    let legacy = ParameterGolfSingleH100ValidationRuntimeComparisonLaneReceipt {
        mean_loss: legacy_summary.mean_loss,
        bits_per_byte: legacy_summary.bits_per_byte,
        observed_ms: legacy_observed_ms,
        average_batch_ms: legacy_observed_ms as f64 / selected_batch_count as f64,
        runtime_receipt: legacy_summary.runtime_receipt,
    };
    let device_resident = ParameterGolfSingleH100ValidationRuntimeComparisonLaneReceipt {
        mean_loss: device_resident_summary.mean_loss,
        bits_per_byte: device_resident_summary.bits_per_byte,
        observed_ms: device_resident_observed_ms,
        average_batch_ms: device_resident_observed_ms as f64 / selected_batch_count as f64,
        runtime_receipt: device_resident_summary.runtime_receipt,
    };
    let summary = format!(
        "Compared the legacy host-rebind validation path against the device-resident validation path on the local CUDA node over {} batch(es) of {} sequence(s); average batch time moved from {:.2}ms to {:.2}ms.",
        selected_batch_count,
        config.batch_sequences,
        legacy.average_batch_ms,
        device_resident.average_batch_ms,
    );
    let mut receipt = ParameterGolfSingleH100ValidationRuntimeComparisonReceipt {
        schema_version: 1,
        scope_window: String::from("parameter_golf_validation_runtime_comparison_v1"),
        run_id: config.run_id.clone(),
        dataset_root: config.dataset_root.clone(),
        tokenizer_path: config.tokenizer_path.clone(),
        dataset_key: config.dataset_key.clone(),
        variant: config.variant.clone(),
        tokenizer_digest,
        dataset_manifest_digest: bundle.manifest.stable_digest(),
        batch_sequences: config.batch_sequences,
        sequence_length: config.sequence_length,
        batch_limit: config.batch_limit,
        observed_cuda_health: machine_observation.observed_cuda_health,
        observed_cuda_devices: machine_observation.observed_cuda_devices,
        legacy,
        device_resident,
        summary,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_parameter_golf_validation_runtime_comparison_receipt|",
        &receipt_without_digest(&receipt),
    );
    Ok(receipt)
}

/// Builds the bounded single-H100 training report.
pub fn build_parameter_golf_single_h100_training_report(
    config: &ParameterGolfSingleH100TrainingConfig,
) -> Result<ParameterGolfSingleH100TrainingReport, ParameterGolfSingleH100TrainingError> {
    let (report, _, _) = build_parameter_golf_single_h100_training_report_inner(config, None)?;
    Ok(report)
}

fn parameter_golf_single_h100_model_from_config(
    model_config: &ParameterGolfConfig,
    model_variant: ParameterGolfSingleH100ModelVariant,
) -> Result<ParameterGolfReferenceModel, ParameterGolfSingleH100TrainingError> {
    let weights = ParameterGolfWeights::from_initializer(model_config, Default::default())?;
    let model_id = match model_variant {
        ParameterGolfSingleH100ModelVariant::BaselineSp1024_9x512 => {
            PARAMETER_GOLF_BASELINE_MODEL_ID
        }
        ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1 => {
            "parameter-golf-homegolf-competitive-v1"
        }
    };
    Ok(ParameterGolfReferenceModel::new(
        ModelDescriptor::new(model_id, "parameter_golf_decoder", "v1"),
        model_config.clone(),
        weights,
    )?)
}

fn build_parameter_golf_single_h100_training_report_inner(
    config: &ParameterGolfSingleH100TrainingConfig,
    output_path: Option<&Path>,
) -> Result<
    (
        ParameterGolfSingleH100TrainingReport,
        Option<PathBuf>,
        Option<crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    ),
    ParameterGolfSingleH100TrainingError,
> {
    config.validate()?;
    let tokenizer_bytes = fs::read(&config.tokenizer_path)?;
    let tokenizer_digest = build_tokenizer_digest(tokenizer_bytes.as_slice());
    let bundle = parameter_golf_dataset_bundle_from_local_dir(
        config.dataset_key.clone(),
        &config.dataset_root,
        config.variant.clone(),
        tokenizer_digest.clone(),
        config.tokenizer_path.display().to_string(),
        None,
    )?;
    let machine_observation = inspect_local_single_h100_machine_with_thresholds(
        config.machine_profile.machine_thresholds(),
    );
    let initial_model =
        parameter_golf_single_h100_model_from_config(&config.model_config, config.model_variant)?;
    let runtime_descriptor = initial_model.banked_descriptor()?;
    let optimizer_plan =
        parameter_golf_optimizer_plan(&runtime_descriptor, &config.hyperparameters)?;
    let optimizer_plan_digest =
        stable_digest(b"psionic_parameter_golf_optimizer_plan|", &optimizer_plan);
    let precision_receipt = precision_receipt_from_optimizer_plan(&optimizer_plan);
    let capability_report = builtin_parameter_golf_cuda_training_capability_report()?;
    let started_at_ms = unix_time_ms();

    if !machine_observation.machine_contract_satisfied {
        return Ok((
            refusal_report(
                config,
                tokenizer_digest,
                &bundle,
                &machine_observation,
                &initial_model,
                optimizer_plan_digest,
                precision_receipt.clone(),
                capability_report.report_digest.clone(),
                capability_report.challenge_kernel_blockers().to_vec(),
                ParameterGolfSingleH100TrainingDisposition::RefusedMachineContract,
                machine_observation.refusal.clone(),
                started_at_ms,
                format!(
                    "The Rust-owned {} trainer path is now implemented, but this run still refused because the local machine contract does not satisfy the declared CUDA admission thresholds.",
                    config.machine_profile.execution_summary_label()
                ),
            ),
            None,
            None,
        ));
    }
    if !capability_report.challenge_kernel_blockers().is_empty() {
        return Ok((
            refusal_report(
                config,
                tokenizer_digest,
                &bundle,
                &machine_observation,
                &initial_model,
                optimizer_plan_digest,
                precision_receipt.clone(),
                capability_report.report_digest.clone(),
                capability_report.challenge_kernel_blockers().to_vec(),
                ParameterGolfSingleH100TrainingDisposition::RefusedCudaBlockers,
                Some(
                    PsionicRefusal::new(
                        PsionicRefusalCode::UnsupportedBackendCapability,
                        PsionicRefusalScope::Runtime,
                        format!(
                            "{} trainer requires an empty Parameter Golf CUDA blocker list, found {:?}",
                            config.machine_profile.execution_summary_label(),
                            capability_report.challenge_kernel_blockers()
                        ),
                    )
                    .with_subject(String::from("parameter_golf_single_h100_cuda_blockers")),
                ),
                started_at_ms,
                format!(
                    "The Rust-owned {} trainer path refuses explicitly when the committed Parameter Golf CUDA capability report still carries challenge blockers.",
                    config.machine_profile.execution_summary_label()
                ),
            ),
            None,
            None,
        ));
    }

    let mut cuda_backend = CudaBackend::new();
    let Some(selected_device) = cuda_backend.selected_device().cloned() else {
        return Err(ParameterGolfSingleH100TrainingError::MissingSelectedH100);
    };
    if !device_matches_single_h100(&selected_device, &machine_observation.thresholds) {
        return Err(ParameterGolfSingleH100TrainingError::MissingSelectedH100);
    }
    let delivered_execution =
        DeliveredExecutionContext::new("cuda", None, vec![selected_device.inventory_qualifiers()]);
    let mut live_visualization_writer = if let Some(output_path) = output_path {
        crate::ParameterGolfSingleH100LiveVisualizationWriter::try_start(
            config,
            output_path,
            started_at_ms,
        )?
    } else {
        None
    };

    emit_progress_line(format!(
        "single_device_train_start run_id={} machine_profile={} device={} max_steps={} iterations={} warmup_steps={} grad_accum_steps={} val_loss_every={} train_log_every={} final_validation_mode={} validation_eval_mode={} validation_batch_sequences={} matrix_execution_mode={} local_train_sequences={} local_validation_sequences={} max_wallclock_seconds={}",
        config.run_id,
        config.machine_profile.as_str(),
        selected_device.device_name.as_deref().unwrap_or("unknown"),
        config.max_steps,
        config.hyperparameters.iterations,
        config.warmup_steps,
        config.geometry.grad_accum_steps,
        config.validation_loss_every,
        config.train_log_every,
        config.final_validation_mode.as_str(),
        config.validation_eval_mode.as_str(),
        config.validation_batch_sequences,
        config.matrix_execution_mode.as_str(),
        config.geometry.local_train_batch_sequences(),
        config.validation_batch_sequences,
        config.hyperparameters.max_wallclock_seconds.unwrap_or(0.0),
    ));
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "training",
            Some(String::from("warmup")),
            format!(
                "The {} trainer started and entered warmup or measured training.",
                config.machine_profile.execution_summary_label()
            ),
            vec![String::from("dataloader"), String::from("trainer_boot")],
            None,
            None,
            true,
        )?;
    }

    let byte_luts =
        parameter_golf_sentencepiece_byte_luts_from_tokenizer_path(&config.tokenizer_path)?;
    let validation_tokens = load_parameter_golf_validation_tokens_from_paths(
        &bundle
            .validation_shards
            .iter()
            .map(|receipt| PathBuf::from(&receipt.path))
            .collect::<Vec<_>>(),
        config.geometry.train_sequence_length,
    )?;
    let mut train_graph_cache = BTreeMap::new();
    let mut train_session_cache = BTreeMap::new();
    let mut eval_graph_cache = BTreeMap::new();

    let mut trainer_state = seed_parameter_states(&initial_model, &optimizer_plan)?;
    let train_contract = ParameterGolfTokenStreamContract::new(
        bundle.manifest.key.clone(),
        PARAMETER_GOLF_TRAIN_SPLIT_NAME,
    )
    .with_mode(DatasetIterationMode::Repeat);
    let mut cursor = ParameterGolfTokenStreamCursor::new(PARAMETER_GOLF_TRAIN_SPLIT_NAME);
    let mut current_model = initial_model.clone();
    let mut ema_state = config
        .ema
        .clone()
        .map(|ema| ParameterGolfEmaState::new(ema, &trainer_state));
    let mut swa_state = config.swa.clone().map(ParameterGolfSwaState::new);
    let mut step_metrics = Vec::new();
    let mut validation_checkpoints = Vec::new();
    let mut aggregate_phase_timings = ParameterGolfSingleH100PhaseTimings::default();
    let requested_train_tokens =
        config.geometry.local_train_batch_tokens().saturating_add(1) as u64;

    let mut warmup_observed_ms = 0_u64;
    if config.warmup_steps > 0 {
        let warmup_started = Instant::now();
        let trainer_state_checkpoint = trainer_state.clone();
        let cursor_checkpoint = cursor.clone();
        let current_model_checkpoint = current_model.clone();
        for warmup_step in 0..config.warmup_steps {
            let learning_rate_multiplier = 1.0;
            let muon_momentum = config.hyperparameters.muon_momentum_at_step(warmup_step);
            let effective_learning_rate = Some(
                config
                    .hyperparameters
                    .token_learning_rate(current_model.descriptor().config.tie_embeddings)
                    * learning_rate_multiplier,
            );
            execute_training_step(
                &mut cuda_backend,
                &selected_device.device,
                &bundle,
                &train_contract,
                &mut cursor,
                &initial_model,
                &mut current_model,
                &mut trainer_state,
                &mut train_graph_cache,
                &mut train_session_cache,
                &config.geometry,
                requested_train_tokens,
                warmup_step + 1,
                config.warmup_steps,
                config.matrix_execution_mode,
                config.hyperparameters.grad_clip_norm,
                learning_rate_multiplier,
                muon_momentum,
                effective_learning_rate,
                None,
                false,
                &mut live_visualization_writer,
            )?;
            if config.warmup_steps <= 20
                || (warmup_step + 1) % 10 == 0
                || warmup_step + 1 == config.warmup_steps
            {
                emit_progress_line(format!(
                    "warmup_step:{}/{}",
                    warmup_step + 1,
                    config.warmup_steps
                ));
            }
            if let Some(writer) = live_visualization_writer.as_mut() {
                writer.record_phase(
                    "training",
                    Some(String::from("warmup")),
                    format!(
                        "Warmup step {} of {} completed.",
                        warmup_step + 1,
                        config.warmup_steps
                    ),
                    vec![String::from("warmup"), String::from("dataloader")],
                    Some(warmup_step + 1),
                    Some(config.geometry.grad_accum_steps as u32),
                    false,
                )?;
            }
        }
        warmup_observed_ms = duration_ms(warmup_started);
        trainer_state = trainer_state_checkpoint;
        cursor = cursor_checkpoint;
        current_model = current_model_checkpoint;
        train_session_cache.clear();
        emit_progress_line(format!(
            "warmup_restore_complete steps={} elapsed_ms={}",
            config.warmup_steps, warmup_observed_ms
        ));
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_event(
                crate::RemoteTrainingEventSeverity::Info,
                "warmup_completed",
                format!(
                    "Warmup completed and the trainer restored the measured-state checkpoint after {} ms.",
                    warmup_observed_ms
                ),
            );
            writer.record_phase(
                "training",
                Some(String::from("optimizer_step")),
                "Warmup completed and the trainer resumed measured optimizer steps.",
                vec![String::from("dataloader"), String::from("optimizer")],
                None,
                None,
                true,
            )?;
        }
    }

    let max_wallclock_ms = config
        .hyperparameters
        .max_wallclock_seconds
        .filter(|seconds| *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0) as u64);
    let mut training_time_ms = 0_u64;
    let mut step = 0_u64;
    let mut stop_reason = None;
    let mut initial_validation = None;
    let mut pre_export_final_validation = None;
    let mut pre_export_final_validation_observed_ms = None;

    loop {
        let last_step = step == config.max_steps || stop_reason.is_some();
        let should_validate = should_run_live_validation_checkpoint(
            step,
            last_step,
            config.validation_loss_every,
            config.final_validation_mode,
            config.final_model_surface,
        );
        if should_validate {
            let stage_label = if last_step {
                String::from("final_validation")
            } else if step == 0 {
                String::from("initial_validation")
            } else {
                format!("periodic_validation_step_{step}")
            };
            emit_progress_line(format!(
                "{}_start sequences={} batch_sequences={}",
                stage_label,
                (validation_tokens.len() - 1) / config.geometry.train_sequence_length,
                config.validation_batch_sequences,
            ));
            if let Some(writer) = live_visualization_writer.as_mut() {
                writer.record_phase(
                    "training",
                    Some(String::from("validation")),
                    format!(
                        "Validation stage `{stage_label}` started for step {}.",
                        step
                    ),
                    vec![String::from("validation"), String::from("gpu_sampling")],
                    Some(step.max(1)),
                    Some(0),
                    true,
                )?;
                writer.record_event(
                    crate::RemoteTrainingEventSeverity::Info,
                    "validation_started",
                    format!("Validation stage `{stage_label}` started."),
                );
            }
            let validation_started = Instant::now();
            let validation_summary =
                evaluate_validation_with_optional_score_first_ttt_on_cuda_and_matrix_execution_mode(
                &mut cuda_backend,
                &selected_device.device,
                current_model.descriptor(),
                &current_model,
                validation_tokens.as_slice(),
                &byte_luts,
                config.geometry.train_sequence_length,
                config.validation_batch_sequences,
                &config.validation_eval_mode,
                last_step
                    .then_some(config.score_first_ttt.as_ref())
                    .flatten(),
                &mut eval_graph_cache,
                &mut train_graph_cache,
                &stage_label,
                live_visualization_writer.as_mut(),
                config.matrix_execution_mode,
            )?;
            let observed_validation_ms = duration_ms(validation_started);
            if last_step {
                pre_export_final_validation_observed_ms = Some(observed_validation_ms);
                pre_export_final_validation = Some(validation_summary.clone());
                if let Some(writer) = live_visualization_writer.as_mut() {
                    writer.record_validation_checkpoint(
                        ParameterGolfSingleH100ValidationCheckpoint {
                            stage_label: stage_label.clone(),
                            trigger_step: step,
                            observed_training_time_ms: training_time_ms,
                            observed_validation_ms,
                            summary: validation_summary,
                        },
                        crate::ValidationSlot::PreExportFinal,
                    )?;
                }
            } else {
                if step == 0 {
                    initial_validation = Some(validation_summary.clone());
                    if let Some(writer) = live_visualization_writer.as_mut() {
                        writer.record_validation_checkpoint(
                            ParameterGolfSingleH100ValidationCheckpoint {
                                stage_label: stage_label.clone(),
                                trigger_step: step,
                                observed_training_time_ms: training_time_ms,
                                observed_validation_ms,
                                summary: validation_summary.clone(),
                            },
                            crate::ValidationSlot::Initial,
                        )?;
                    }
                }
                validation_checkpoints.push(ParameterGolfSingleH100ValidationCheckpoint {
                    stage_label,
                    trigger_step: step,
                    observed_training_time_ms: training_time_ms,
                    observed_validation_ms,
                    summary: validation_summary,
                });
                if let Some(writer) = live_visualization_writer.as_mut() {
                    writer.record_validation_checkpoint(
                        validation_checkpoints
                            .last()
                            .expect("validation checkpoint should exist")
                            .clone(),
                        crate::ValidationSlot::Periodic,
                    )?;
                }
            }
        } else if last_step {
            emit_progress_line(format!(
                "final_validation_skipped mode={} reason=explicit_final_validation_mode",
                config.final_validation_mode.as_str(),
            ));
        }

        if last_step {
            if stop_reason == Some(ParameterGolfSingleH100TrainingStopReason::WallclockCapReached)
                && step < config.max_steps
            {
                emit_progress_line(format!(
                    "stopping_early: wallclock_cap train_time:{}ms step:{}/{}",
                    training_time_ms, step, config.max_steps
                ));
            }
            break;
        }

        let learning_rate_multiplier = config
            .hyperparameters
            .learning_rate_multiplier(step, training_time_ms as f32);
        let muon_momentum = config.hyperparameters.muon_momentum_at_step(step);
        let effective_learning_rate = Some(
            config
                .hyperparameters
                .token_learning_rate(current_model.descriptor().config.tie_embeddings)
                * learning_rate_multiplier,
        );
        let remaining_training_wallclock_ms =
            max_wallclock_ms.map(|wallclock_ms| wallclock_ms.saturating_sub(training_time_ms));
        let step_metrics_next = match execute_training_step(
            &mut cuda_backend,
            &selected_device.device,
            &bundle,
            &train_contract,
            &mut cursor,
            &initial_model,
            &mut current_model,
            &mut trainer_state,
            &mut train_graph_cache,
            &mut train_session_cache,
            &config.geometry,
            requested_train_tokens,
            step + 1,
            config.max_steps,
            config.matrix_execution_mode,
            config.hyperparameters.grad_clip_norm,
            learning_rate_multiplier,
            muon_momentum,
            effective_learning_rate,
            remaining_training_wallclock_ms,
            true,
            &mut live_visualization_writer,
        ) {
            Ok(step_metrics) => step_metrics,
            Err(ParameterGolfSingleH100TrainingError::ProjectedWallclockCapExceeded {
                global_step,
                observed_micro_steps,
                grad_accum_steps,
                observed_step_wallclock_ms,
                projected_full_step_wallclock_ms,
                remaining_training_wallclock_ms,
            }) => {
                let detail = format!(
                    "{} trainer projected optimizer step {} to require about {}ms after {}/{} micro-steps, which exceeds the remaining {}ms wallclock budget on this machine posture.",
                    config.machine_profile.execution_summary_label(),
                    global_step,
                    projected_full_step_wallclock_ms,
                    observed_micro_steps,
                    grad_accum_steps,
                    remaining_training_wallclock_ms,
                );
                emit_progress_line(format!(
                    "wallclock_projection_refusal step={}/{} observed_micro_steps={}/{} observed_step_wallclock_ms={} projected_full_step_wallclock_ms={} remaining_training_wallclock_ms={}",
                    global_step,
                    config.max_steps,
                    observed_micro_steps,
                    grad_accum_steps,
                    observed_step_wallclock_ms,
                    projected_full_step_wallclock_ms,
                    remaining_training_wallclock_ms,
                ));
                if step == 0 {
                    return Ok((
                        refusal_report(
                            config,
                            tokenizer_digest,
                            &bundle,
                            &machine_observation,
                            &initial_model,
                            optimizer_plan_digest,
                            precision_receipt.clone(),
                            capability_report.report_digest.clone(),
                            capability_report.challenge_kernel_blockers().to_vec(),
                            ParameterGolfSingleH100TrainingDisposition::RefusedWallclockProjection,
                            Some(
                                PsionicRefusal::new(
                                    PsionicRefusalCode::UnsupportedBackendCapability,
                                    PsionicRefusalScope::Runtime,
                                    detail.clone(),
                                )
                                .with_subject(format!(
                                    "{}.wallclock_projection",
                                    config.machine_profile.as_str()
                                )),
                            ),
                            started_at_ms,
                            format!(
                                "The Rust-owned {} trainer refused before emitting a training artifact because the first measured optimizer step projected beyond the remaining declared wallclock cap on this machine posture.",
                                config.machine_profile.execution_summary_label()
                            ),
                        ),
                        None,
                        live_visualization_writer,
                    ));
                }
                emit_progress_line(format!(
                    "stopping_early: wallclock_projection train_time:{}ms completed_steps={} projected_next_step_ms={} remaining_training_wallclock_ms={}",
                    training_time_ms,
                    step,
                    projected_full_step_wallclock_ms,
                    remaining_training_wallclock_ms,
                ));
                stop_reason = Some(ParameterGolfSingleH100TrainingStopReason::WallclockCapReached);
                break;
            }
            Err(error) => return Err(error),
        };
        training_time_ms = training_time_ms.saturating_add(step_metrics_next.observed_wallclock_ms);
        aggregate_phase_timings.accumulate(&step_metrics_next.phase_timings);
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_step(step_metrics_next.clone())?;
        }
        step_metrics.push(step_metrics_next);
        step += 1;
        if let Some(ema_state) = ema_state.as_mut() {
            ema_state.update(&trainer_state)?;
        }
        if let Some(swa_state) = swa_state.as_mut() {
            if swa_state.should_sample(step, learning_rate_multiplier) {
                if swa_state.sample_count == 0 {
                    emit_progress_line(format!(
                        "swa:start step:{} source_surface={} every_steps={} max_learning_rate_multiplier={:.4}",
                        step,
                        swa_state.config.source_surface.as_str(),
                        swa_state.config.every_steps,
                        swa_state.config.max_learning_rate_multiplier,
                    ));
                }
                let snapshot = match swa_state.config.source_surface {
                    ParameterGolfSwaSourceSurface::Raw => {
                        current_parameter_state_overrides(&trainer_state)
                    }
                    ParameterGolfSwaSourceSurface::Ema => ema_state
                        .as_ref()
                        .ok_or_else(|| ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: String::from(
                                "swa source_surface=ema requested without one live ema state",
                            ),
                        })?
                        .parameter_values
                        .clone(),
                };
                swa_state.update_from_overrides(&snapshot)?;
            }
        }

        let should_log_train = config.train_log_every > 0
            && (step <= 10 || step % config.train_log_every == 0 || stop_reason.is_some());
        if should_log_train {
            let latest_step = step_metrics
                .last()
                .expect("step metrics should be present after a completed step");
            emit_progress_line(format!(
                "step:{}/{} train_loss:{:.4} train_time:{}ms step_avg:{:.2}ms",
                step,
                config.max_steps,
                latest_step.mean_microbatch_loss,
                training_time_ms,
                training_time_ms as f64 / step.max(1) as f64,
            ));
        }

        if stop_reason.is_none()
            && max_wallclock_ms.is_some_and(|wallclock_ms| training_time_ms >= wallclock_ms)
        {
            stop_reason = Some(ParameterGolfSingleH100TrainingStopReason::WallclockCapReached);
        }
    }

    let final_swa_receipt = swa_state.as_ref().and_then(ParameterGolfSwaState::receipt);
    let final_model = match config.final_model_surface {
        ParameterGolfFinalModelSurface::Raw => current_model.clone(),
        ParameterGolfFinalModelSurface::Ema => {
            let ema_state = ema_state.as_ref().ok_or_else(|| {
                ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: String::from(
                        "final_model_surface=ema requested without one live ema state",
                    ),
                }
            })?;
            emit_progress_line(format!(
                "final_model_surface_apply surface=ema decay={}",
                ema_state.config.decay
            ));
            materialize_model_from_overrides(&initial_model, &ema_state.parameter_values)?
        }
        ParameterGolfFinalModelSurface::Swa => {
            let swa_state = swa_state.as_ref().ok_or_else(|| {
                ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: String::from(
                        "final_model_surface=swa requested without one live swa state",
                    ),
                }
            })?;
            let averaged_values = swa_state.averaged_parameter_values()?;
            emit_progress_line(format!(
                "final_model_surface_apply surface=swa source_surface={} sample_count={}",
                swa_state.config.source_surface.as_str(),
                swa_state.sample_count,
            ));
            materialize_model_from_overrides(&initial_model, &averaged_values)?
        }
    };

    if config.final_validation_mode.runs_live_validation()
        && config.final_model_surface != ParameterGolfFinalModelSurface::Raw
    {
        let stage_label = format!("final_validation_{}", config.final_model_surface.as_str());
        emit_progress_line(format!(
            "{}_start sequences={} batch_sequences={} surface={}",
            stage_label,
            (validation_tokens.len() - 1) / config.geometry.train_sequence_length,
            config.validation_batch_sequences,
            config.final_model_surface.as_str(),
        ));
        let validation_started = Instant::now();
        let validation_summary =
            evaluate_validation_with_optional_score_first_ttt_on_cuda_and_matrix_execution_mode(
                &mut cuda_backend,
                &selected_device.device,
                final_model.descriptor(),
                &final_model,
                validation_tokens.as_slice(),
                &byte_luts,
                config.geometry.train_sequence_length,
                config.validation_batch_sequences,
                &config.validation_eval_mode,
                config.score_first_ttt.as_ref(),
                &mut eval_graph_cache,
                &mut train_graph_cache,
                &stage_label,
                live_visualization_writer.as_mut(),
                config.matrix_execution_mode,
            )?;
        let observed_validation_ms = duration_ms(validation_started);
        pre_export_final_validation_observed_ms = Some(observed_validation_ms);
        pre_export_final_validation = Some(validation_summary.clone());
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_validation_checkpoint(
                ParameterGolfSingleH100ValidationCheckpoint {
                    stage_label,
                    trigger_step: step,
                    observed_training_time_ms: training_time_ms,
                    observed_validation_ms,
                    summary: validation_summary,
                },
                crate::ValidationSlot::PreExportFinal,
            )?;
        }
    }

    let compressed_model_artifact = export_parameter_golf_quantized_model_artifact(
        &final_model,
        &config.final_artifact_config,
        &config.run_id,
        step,
    )?;
    emit_progress_line(format!(
        "final_artifact_export_complete bytes={} artifact_ref={} artifact_digest={}",
        compressed_model_artifact.bytes.len(),
        compressed_model_artifact.artifact_ref,
        compressed_model_artifact.artifact_digest,
    ));
    let persisted_artifact_path = if let Some(output_path) = output_path {
        let artifact_path = persisted_artifact_output_path(output_path);
        if let Some(parent) = artifact_path.parent() {
            fs::create_dir_all(parent)?;
        }
        emit_progress_line(format!(
            "final_artifact_persist_start path={} bytes={}",
            artifact_path.display(),
            compressed_model_artifact.bytes.len(),
        ));
        fs::write(&artifact_path, compressed_model_artifact.bytes.as_slice())?;
        emit_progress_line(format!(
            "final_artifact_persist_complete path={}",
            artifact_path.display()
        ));
        Some(artifact_path)
    } else {
        None
    };
    let mut final_validation = pre_export_final_validation.clone();
    let mut final_validation_observed_ms = pre_export_final_validation_observed_ms;
    let mut final_roundtrip_receipt = None;
    let final_roundtrip_metric_source = config.final_artifact_config.metric_source();
    let final_roundtrip_stage_label = format!("final_{final_roundtrip_metric_source}");
    if config.final_validation_mode.runs_roundtrip_validation() {
        emit_progress_line(format!(
            "{}_start sequences={} batch_sequences={} compressed_model_bytes={} artifact_ref={} artifact_digest={}",
            final_roundtrip_metric_source,
            (validation_tokens.len() - 1) / config.geometry.train_sequence_length,
            config.validation_batch_sequences,
            compressed_model_artifact.bytes.len(),
            compressed_model_artifact.artifact_ref,
            compressed_model_artifact.artifact_digest,
        ));
        let roundtrip_model = restore_parameter_golf_model_from_quantized_artifact(
            &initial_model,
            compressed_model_artifact.bytes.as_slice(),
        )?;
        let roundtrip_validation_started = Instant::now();
        let roundtrip_validation =
            evaluate_validation_with_optional_score_first_ttt_on_cuda_and_matrix_execution_mode(
                &mut cuda_backend,
                &selected_device.device,
                roundtrip_model.descriptor(),
                &roundtrip_model,
                validation_tokens.as_slice(),
                &byte_luts,
                config.geometry.train_sequence_length,
                config.validation_batch_sequences,
                &config.validation_eval_mode,
                config.score_first_ttt.as_ref(),
                &mut eval_graph_cache,
                &mut train_graph_cache,
                final_roundtrip_metric_source,
                live_visualization_writer.as_mut(),
                config.matrix_execution_mode,
            )?;
        let roundtrip_observed_ms = duration_ms(roundtrip_validation_started);
        emit_progress_line(format!(
            "{} val_loss:{:.4} val_bpb:{:.4} eval_time:{}ms compressed_model_bytes={} artifact_ref={} artifact_digest={}",
            final_roundtrip_metric_source,
            roundtrip_validation.mean_loss,
            roundtrip_validation.bits_per_byte,
            roundtrip_observed_ms,
            compressed_model_artifact.bytes.len(),
            compressed_model_artifact.artifact_ref,
            compressed_model_artifact.artifact_digest,
        ));
        emit_progress_line(format!(
            "{}_exact val_loss:{:.8} val_bpb:{:.8}",
            final_roundtrip_metric_source,
            roundtrip_validation.mean_loss,
            roundtrip_validation.bits_per_byte,
        ));
        final_validation_observed_ms = Some(roundtrip_observed_ms);
        final_validation = Some(roundtrip_validation.clone());
        final_roundtrip_receipt = Some(ParameterGolfSingleH100RoundtripReceipt {
            final_model_surface: config.final_model_surface,
            ema: match config.final_model_surface {
                ParameterGolfFinalModelSurface::Raw => None,
                ParameterGolfFinalModelSurface::Ema => config.ema.clone(),
                ParameterGolfFinalModelSurface::Swa => (config
                    .swa
                    .as_ref()
                    .is_some_and(|swa| swa.source_surface == ParameterGolfSwaSourceSurface::Ema))
                .then(|| config.ema.clone())
                .flatten(),
            },
            swa: (config.final_model_surface == ParameterGolfFinalModelSurface::Swa)
                .then(|| final_swa_receipt.clone())
                .flatten(),
            artifact_config: config.final_artifact_config.clone(),
            metric_source: String::from(final_roundtrip_metric_source),
            validation: roundtrip_validation,
            pre_ttt_validation: None,
            observed_eval_ms: roundtrip_observed_ms,
            compressed_model_bytes: compressed_model_artifact.bytes.len() as u64,
            compressed_model_artifact_ref: compressed_model_artifact.artifact_ref.clone(),
            compressed_model_artifact_digest: compressed_model_artifact.artifact_digest.clone(),
        });
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_roundtrip_receipt(
                final_roundtrip_receipt
                    .clone()
                    .expect("roundtrip receipt should be present"),
            )?;
            writer.record_validation_checkpoint(
                ParameterGolfSingleH100ValidationCheckpoint {
                    stage_label: final_roundtrip_stage_label.clone(),
                    trigger_step: step,
                    observed_training_time_ms: training_time_ms,
                    observed_validation_ms: roundtrip_observed_ms,
                    summary: final_validation
                        .clone()
                        .expect("final validation should exist for roundtrip"),
                },
                crate::ValidationSlot::FinalRoundtrip,
            )?;
        }
    } else {
        emit_progress_line(format!(
            "{}_skipped mode={} reason=explicit_final_validation_mode compressed_model_bytes={} artifact_ref={} artifact_digest={}",
            final_roundtrip_metric_source,
            config.final_validation_mode.as_str(),
            compressed_model_artifact.bytes.len(),
            compressed_model_artifact.artifact_ref,
            compressed_model_artifact.artifact_digest,
        ));
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_event(
                crate::RemoteTrainingEventSeverity::Info,
                &format!("{final_roundtrip_stage_label}_skipped"),
                format!(
                    "The final {} validation was skipped because final_validation_mode={}.",
                    final_roundtrip_metric_source,
                    config.final_validation_mode.as_str()
                ),
            );
        }
    }

    let finished_at_ms = unix_time_ms();
    let observed_wallclock_ms = finished_at_ms.saturating_sub(started_at_ms);
    let realized_stop_reason =
        stop_reason.unwrap_or(ParameterGolfSingleH100TrainingStopReason::StepBudgetReached);
    let final_metric_surface = match config.final_validation_mode {
        ParameterGolfSingleH100ValidationMode::LiveOnly => {
            "reported final live-model validation metrics without running the exported quantized roundtrip validation"
        }
        ParameterGolfSingleH100ValidationMode::RoundtripOnly => {
            "reported canonical final contest metrics from the exported quantized roundtrip artifact without also replaying the live-model final validation"
        }
        ParameterGolfSingleH100ValidationMode::ArtifactOnly => {
            "persisted the exported quantized artifact without running final live-model or quantized roundtrip validation, so detached score closeout must produce the final score receipt separately"
        }
        ParameterGolfSingleH100ValidationMode::Both => {
            "preserved the pre-export live-model validation separately and reported canonical final contest metrics from the exported quantized roundtrip artifact"
        }
    };
    let summary = format!(
        "The Rust-owned {} trainer executed {} optimizer step(s) with challenge single-device geometry on CUDA, used the widened train_gpt.py-style warmup, validation, and wallclock-stop control loop, ran with final_validation_mode={}, validation_eval_mode={}, validation_batch_sequences={}, matrix_execution_mode={}, and final_model_surface={}, {} before stopping via {:?}.",
        config.machine_profile.execution_summary_label(),
        step,
        config.final_validation_mode.as_str(),
        config.validation_eval_mode.as_str(),
        config.validation_batch_sequences,
        config.matrix_execution_mode.as_str(),
        config.final_model_surface.as_str(),
        final_metric_surface,
        realized_stop_reason
    );

    let mut report = ParameterGolfSingleH100TrainingReport {
        schema_version: 2,
        scope_window: String::from("parameter_golf_single_h100_training_v2"),
        run_id: config.run_id.clone(),
        dataset_root: config.dataset_root.clone(),
        tokenizer_path: config.tokenizer_path.clone(),
        dataset_key: config.dataset_key.clone(),
        variant: config.variant.clone(),
        tokenizer_digest,
        dataset_manifest_digest: bundle.manifest.stable_digest(),
        train_shard_count: bundle.train_shards.len(),
        validation_shard_count: bundle.validation_shards.len(),
        train_token_count: split_token_count(&bundle, PARAMETER_GOLF_TRAIN_SPLIT_NAME),
        validation_token_count: split_token_count(&bundle, "validation"),
        geometry: config.geometry.clone(),
        hyperparameters: config.hyperparameters.clone(),
        max_steps: config.max_steps,
        warmup_steps: config.warmup_steps,
        completed_warmup_steps: config.warmup_steps,
        validation_loss_every: config.validation_loss_every,
        train_log_every: config.train_log_every,
        final_validation_mode: config.final_validation_mode,
        validation_eval_mode: config.validation_eval_mode.clone(),
        validation_batch_sequences: config.validation_batch_sequences,
        matrix_execution_mode: config.matrix_execution_mode,
        score_first_ttt: config.score_first_ttt.clone(),
        ema: config.ema.clone(),
        swa: config.swa.clone(),
        final_model_surface: config.final_model_surface,
        final_artifact_config: config.final_artifact_config.clone(),
        executed_steps: step,
        stop_reason: Some(realized_stop_reason),
        delivered_execution,
        machine_thresholds: machine_observation.thresholds.clone(),
        observed_cuda_health: machine_observation.observed_cuda_health.clone(),
        cuda_discovery_error: machine_observation.cuda_discovery_error.clone(),
        observed_cuda_devices: machine_observation.observed_cuda_devices.clone(),
        matching_h100_device_count: machine_observation.matching_h100_device_count,
        machine_contract_satisfied: machine_observation.machine_contract_satisfied,
        baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
        baseline_model_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
        model_variant: config.model_variant,
        machine_profile: config.machine_profile,
        model_config: config.model_config.clone(),
        baseline_model_descriptor_digest: final_model.descriptor().stable_digest(),
        optimizer_plan_digest,
        precision_receipt: precision_receipt_from_trainer_state(&trainer_state),
        cuda_training_capability_report_digest: capability_report.report_digest.clone(),
        challenge_kernel_blockers: capability_report.challenge_kernel_blockers().to_vec(),
        validation_checkpoints,
        initial_validation,
        pre_export_final_validation,
        final_validation,
        warmup_observed_ms,
        observed_training_time_ms: training_time_ms,
        pre_export_final_validation_observed_ms,
        final_validation_observed_ms,
        final_swa_receipt,
        final_roundtrip_receipt,
        compressed_model_bytes: Some(compressed_model_artifact.bytes.len() as u64),
        compressed_model_artifact_ref: Some(compressed_model_artifact.artifact_ref.clone()),
        compressed_model_artifact_digest: Some(compressed_model_artifact.artifact_digest.clone()),
        compressed_model_artifact_path: persisted_artifact_path
            .as_ref()
            .map(|path| path.display().to_string()),
        step_metrics,
        aggregate_phase_timings: Some(aggregate_phase_timings),
        final_training_cursor: Some(cursor),
        started_at_ms,
        finished_at_ms,
        observed_wallclock_ms,
        disposition: ParameterGolfSingleH100TrainingDisposition::TrainingExecuted,
        refusal: None,
        claim_boundary: String::from(config.machine_profile.claim_boundary()),
        summary,
        report_digest: String::new(),
    };
    let report_digest_started = Instant::now();
    emit_progress_line(format!(
        "training_report_digest_start final_validation_bpb={} report_path={}",
        report.final_validation.as_ref().map_or_else(
            || String::from("none"),
            |summary| format!("{:.8}", summary.bits_per_byte)
        ),
        output_path
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("<memory_only>")),
    ));
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_single_h100_training_report|",
        &report_without_digest(&report),
    );
    emit_progress_line(format!(
        "training_report_digest_complete elapsed_ms={} report_digest={}",
        duration_ms(report_digest_started),
        report.report_digest,
    ));
    Ok((report, persisted_artifact_path, live_visualization_writer))
}

fn refusal_report(
    config: &ParameterGolfSingleH100TrainingConfig,
    tokenizer_digest: psionic_data::TokenizerDigest,
    bundle: &ParameterGolfDatasetBundle,
    machine_observation: &ParameterGolfSingleH100MachineObservation,
    initial_model: &ParameterGolfReferenceModel,
    optimizer_plan_digest: String,
    precision_receipt: ParameterGolfSingleH100PrecisionReceipt,
    capability_report_digest: String,
    challenge_kernel_blockers: Vec<String>,
    disposition: ParameterGolfSingleH100TrainingDisposition,
    refusal: Option<PsionicRefusal>,
    started_at_ms: u64,
    summary: String,
) -> ParameterGolfSingleH100TrainingReport {
    let finished_at_ms = unix_time_ms();
    let mut report = ParameterGolfSingleH100TrainingReport {
        schema_version: 2,
        scope_window: String::from("parameter_golf_single_h100_training_v2"),
        run_id: config.run_id.clone(),
        dataset_root: config.dataset_root.clone(),
        tokenizer_path: config.tokenizer_path.clone(),
        dataset_key: config.dataset_key.clone(),
        variant: config.variant.clone(),
        tokenizer_digest,
        dataset_manifest_digest: bundle.manifest.stable_digest(),
        train_shard_count: bundle.train_shards.len(),
        validation_shard_count: bundle.validation_shards.len(),
        train_token_count: split_token_count(bundle, PARAMETER_GOLF_TRAIN_SPLIT_NAME),
        validation_token_count: split_token_count(bundle, "validation"),
        geometry: config.geometry.clone(),
        hyperparameters: config.hyperparameters.clone(),
        max_steps: config.max_steps,
        warmup_steps: config.warmup_steps,
        completed_warmup_steps: 0,
        validation_loss_every: config.validation_loss_every,
        train_log_every: config.train_log_every,
        final_validation_mode: config.final_validation_mode,
        validation_eval_mode: config.validation_eval_mode.clone(),
        validation_batch_sequences: config.validation_batch_sequences,
        matrix_execution_mode: config.matrix_execution_mode,
        score_first_ttt: config.score_first_ttt.clone(),
        ema: config.ema.clone(),
        swa: config.swa.clone(),
        final_model_surface: config.final_model_surface,
        final_artifact_config: config.final_artifact_config.clone(),
        executed_steps: 0,
        stop_reason: None,
        delivered_execution: DeliveredExecutionContext::new("cuda", None, Vec::new()),
        machine_thresholds: machine_observation.thresholds.clone(),
        observed_cuda_health: machine_observation.observed_cuda_health.clone(),
        cuda_discovery_error: machine_observation.cuda_discovery_error.clone(),
        observed_cuda_devices: machine_observation.observed_cuda_devices.clone(),
        matching_h100_device_count: machine_observation.matching_h100_device_count,
        machine_contract_satisfied: machine_observation.machine_contract_satisfied,
        model_variant: config.model_variant,
        machine_profile: config.machine_profile,
        model_config: config.model_config.clone(),
        baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
        baseline_model_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
        baseline_model_descriptor_digest: initial_model.descriptor().stable_digest(),
        optimizer_plan_digest,
        precision_receipt,
        cuda_training_capability_report_digest: capability_report_digest,
        challenge_kernel_blockers,
        validation_checkpoints: Vec::new(),
        initial_validation: None,
        pre_export_final_validation: None,
        final_validation: None,
        warmup_observed_ms: 0,
        observed_training_time_ms: 0,
        pre_export_final_validation_observed_ms: None,
        final_validation_observed_ms: None,
        final_swa_receipt: None,
        final_roundtrip_receipt: None,
        compressed_model_bytes: None,
        compressed_model_artifact_ref: None,
        compressed_model_artifact_digest: None,
        compressed_model_artifact_path: None,
        step_metrics: Vec::new(),
        aggregate_phase_timings: None,
        final_training_cursor: None,
        started_at_ms,
        finished_at_ms,
        observed_wallclock_ms: finished_at_ms.saturating_sub(started_at_ms),
        disposition,
        refusal,
        claim_boundary: String::from(config.machine_profile.refusal_claim_boundary()),
        summary,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_single_h100_training_report|",
        &report_without_digest(&report),
    );
    report
}

fn persisted_artifact_output_path(report_path: &Path) -> PathBuf {
    let stem = report_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("parameter_golf_single_device_training");
    report_path.with_file_name(format!("{stem}.final_model.st"))
}

pub(crate) fn seed_parameter_states(
    model: &ParameterGolfReferenceModel,
    optimizer_plan: &crate::ParameterGolfOptimizerPlan,
) -> Result<ParameterGolfSingleH100TrainerState, ParameterGolfSingleH100TrainingError> {
    let parameter_vectors = model
        .all_parameter_vectors()?
        .into_iter()
        .map(|vector| (vector.parameter_id.clone(), vector))
        .collect::<BTreeMap<_, _>>();
    let mut parameter_states = BTreeMap::new();
    for group in &optimizer_plan.groups {
        for parameter_id in &group.tensor_names {
            let vector = parameter_vectors.get(parameter_id).ok_or_else(|| {
                ParameterGolfSingleH100TrainingError::MissingParameterState {
                    parameter_id: parameter_id.clone(),
                }
            })?;
            let state = match &group.execution {
                ParameterGolfOptimizerExecution::Adam { optimizer } => match group.kind {
                    ParameterGolfOptimizerGroupKind::TokenEmbeddingAdam
                    | ParameterGolfOptimizerGroupKind::UntiedLmHeadAdam => {
                        let mut train_visible_values = vector.values.clone();
                        round_values_to_bf16(train_visible_values.as_mut_slice());
                        let train_visible_bf16_bits =
                            bf16_bits_from_f32_values(train_visible_values.as_slice());
                        ParameterGolfParameterState::AdamBf16Master {
                            shape: vector.shape.dims().to_vec(),
                            train_visible_values,
                            train_visible_bf16_bits,
                            master_weight_values: vector.values.clone(),
                            optimizer: optimizer.clone(),
                            optimizer_state: optimizer.initialize_state(vector.values.len()),
                            last_step_receipt: None,
                        }
                    }
                    ParameterGolfOptimizerGroupKind::ScalarControlAdam => {
                        ParameterGolfParameterState::AdamFp32 {
                            shape: vector.shape.dims().to_vec(),
                            values: vector.values.clone(),
                            optimizer: optimizer.clone(),
                            optimizer_state: optimizer.initialize_state(vector.values.len()),
                        }
                    }
                    ParameterGolfOptimizerGroupKind::MatrixMuon => {
                        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                            message: format!(
                                "optimizer group `{}` declared Adam execution for matrix Muon posture",
                                group.group_id
                            ),
                        });
                    }
                },
                ParameterGolfOptimizerExecution::Muon { optimizer } => {
                    let mut values = vector.values.clone();
                    round_values_to_bf16(values.as_mut_slice());
                    let bf16_bits = bf16_bits_from_f32_values(values.as_slice());
                    ParameterGolfParameterState::MuonBf16 {
                        shape: vector.shape.dims().to_vec(),
                        values,
                        bf16_bits,
                        optimizer: optimizer.clone(),
                        optimizer_state: crate::ParameterGolfMuonState::zeros_for_len(
                            vector.values.len(),
                        ),
                    }
                }
            };
            parameter_states.insert(parameter_id.clone(), state);
        }
    }
    Ok(ParameterGolfSingleH100TrainerState { parameter_states })
}

fn precision_receipt_from_optimizer_plan(
    optimizer_plan: &ParameterGolfOptimizerPlan,
) -> ParameterGolfSingleH100PrecisionReceipt {
    ParameterGolfSingleH100PrecisionReceipt {
        graph_parameter_upload_precision: TrainingPrecisionMode::Bf16,
        graph_execution_precision: TrainingPrecisionMode::Fp32,
        retained_activation_precision: TrainingPrecisionMode::Fp32,
        group_receipts: optimizer_plan
            .groups
            .iter()
            .map(|group| match group.kind {
                ParameterGolfOptimizerGroupKind::TokenEmbeddingAdam
                | ParameterGolfOptimizerGroupKind::UntiedLmHeadAdam => {
                    ParameterGolfSingleH100GroupPrecisionReceipt {
                        group_id: group.group_id.clone(),
                        parameter_precision: TrainingPrecisionMode::Bf16,
                        gradient_precision: TrainingPrecisionMode::Bf16,
                        optimizer_state_precision: TrainingPrecisionMode::Fp32,
                        master_weight_precision: Some(TrainingPrecisionMode::Fp32),
                    }
                }
                ParameterGolfOptimizerGroupKind::MatrixMuon => {
                    ParameterGolfSingleH100GroupPrecisionReceipt {
                        group_id: group.group_id.clone(),
                        parameter_precision: TrainingPrecisionMode::Bf16,
                        gradient_precision: TrainingPrecisionMode::Fp32,
                        optimizer_state_precision: TrainingPrecisionMode::Fp32,
                        master_weight_precision: None,
                    }
                }
                ParameterGolfOptimizerGroupKind::ScalarControlAdam => {
                    ParameterGolfSingleH100GroupPrecisionReceipt {
                        group_id: group.group_id.clone(),
                        parameter_precision: TrainingPrecisionMode::Fp32,
                        gradient_precision: TrainingPrecisionMode::Fp32,
                        optimizer_state_precision: TrainingPrecisionMode::Fp32,
                        master_weight_precision: None,
                    }
                }
            })
            .collect(),
        notes: vec![String::from(
            "train-visible Parameter Golf weights now upload through BF16 graph inputs on the token-embedding, linear, and admitted attention hot path, while scalar/control tensors, optimizer math, and the wider retained graph surface stay explicit F32 where the bounded CUDA lane still requires it",
        )],
    }
}

fn precision_receipt_from_trainer_state(
    state: &ParameterGolfSingleH100TrainerState,
) -> ParameterGolfSingleH100PrecisionReceipt {
    ParameterGolfSingleH100PrecisionReceipt {
        graph_parameter_upload_precision: TrainingPrecisionMode::Bf16,
        graph_execution_precision: TrainingPrecisionMode::Fp32,
        retained_activation_precision: TrainingPrecisionMode::Fp32,
        group_receipts: state
            .parameter_states
            .iter()
            .map(|(parameter_id, state)| state.precision_receipt(parameter_id.clone()))
            .collect(),
        notes: vec![String::from(
            "train-visible Parameter Golf weights now upload through BF16 graph inputs on the token-embedding, linear, and admitted attention hot path, while scalar/control tensors, optimizer math, and the wider retained graph surface stay explicit F32 where the bounded CUDA lane still requires it",
        )],
    }
}

fn round_values_to_bf16(values: &mut [f32]) {
    for value in values {
        *value = bf16::from_f32(*value).to_f32();
    }
}

fn bf16_bits_from_f32_values(values: &[f32]) -> Vec<u16> {
    values
        .iter()
        .map(|value| bf16::from_f32(*value).to_bits())
        .collect()
}

pub(crate) fn zero_gradients(
    state: &ParameterGolfSingleH100TrainerState,
) -> Vec<(String, Vec<f32>)> {
    state
        .parameter_states
        .iter()
        .map(|(parameter_id, state)| (parameter_id.clone(), vec![0.0; state.values().len()]))
        .collect()
}

pub(crate) fn flatten_parameter_golf_optimizer_group_values(
    optimizer_plan: &ParameterGolfOptimizerPlan,
    values_by_parameter: &BTreeMap<String, Vec<f32>>,
) -> Result<BTreeMap<String, Vec<f32>>, ParameterGolfSingleH100TrainingError> {
    optimizer_plan
        .groups
        .iter()
        .map(|group| {
            let mut values = Vec::with_capacity(group.parameter_count);
            for parameter_id in &group.tensor_names {
                let parameter_values = values_by_parameter.get(parameter_id).ok_or_else(|| {
                    ParameterGolfSingleH100TrainingError::MissingParameterState {
                        parameter_id: parameter_id.clone(),
                    }
                })?;
                values.extend_from_slice(parameter_values);
            }
            if values.len() != group.parameter_count {
                return Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "optimizer group `{}` expected {} flattened values but produced {}",
                        group.group_id,
                        group.parameter_count,
                        values.len()
                    ),
                });
            }
            Ok((group.group_id.clone(), values))
        })
        .collect()
}

pub(crate) fn flatten_parameter_golf_optimizer_group_buffers(
    optimizer_plan: &ParameterGolfOptimizerPlan,
    values_by_parameter: &BTreeMap<String, Vec<f32>>,
) -> Result<BTreeMap<String, crate::TrainingTensorBuffer>, ParameterGolfSingleH100TrainingError> {
    flatten_parameter_golf_optimizer_group_values(optimizer_plan, values_by_parameter)?
        .into_iter()
        .map(|(group_id, values)| {
            let value_len = values.len();
            Ok((
                group_id.clone(),
                crate::TrainingTensorBuffer::from_f32(
                    group_id.clone(),
                    psionic_core::TensorSpec::new(
                        Shape::new(vec![value_len]),
                        DType::F32,
                        psionic_core::Device::cpu(),
                    ),
                    values,
                )
                .map_err(|error| ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "failed to build flattened optimizer group buffer `{group_id}` with {value_len} values: {error}"
                    ),
                })?,
            ))
        })
        .collect()
}

pub(crate) fn accumulate_gradients(
    accumulated: &mut [(String, Vec<f32>)],
    trainer_state: &ParameterGolfSingleH100TrainerState,
    gradients: &BTreeMap<String, Vec<f32>>,
    divisor: f32,
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    for (parameter_id, values) in accumulated {
        let gradient = gradients.get(parameter_id).ok_or_else(|| {
            ParameterGolfSingleH100TrainingError::MissingParameterState {
                parameter_id: parameter_id.clone(),
            }
        })?;
        let expected_len = trainer_state
            .parameter_states
            .get(parameter_id)
            .ok_or_else(
                || ParameterGolfSingleH100TrainingError::MissingParameterState {
                    parameter_id: parameter_id.clone(),
                },
            )?
            .values()
            .len();
        if gradient.len() != expected_len {
            return Err(ParameterGolfSingleH100TrainingError::Serialization {
                message: format!(
                    "gradient length mismatch for `{parameter_id}`: expected {expected_len}, found {}",
                    gradient.len()
                ),
            });
        }
        for (accumulated, gradient) in values.iter_mut().zip(gradient.iter()) {
            *accumulated += *gradient / divisor;
        }
    }
    Ok(())
}

pub(crate) fn apply_gradients_to_state(
    state: &mut ParameterGolfSingleH100TrainerState,
    gradients: &[(String, Vec<f32>)],
    learning_rate_multiplier: f32,
    muon_momentum: f32,
    step_number: u64,
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    for (parameter_id, gradient_values) in gradients {
        let parameter_state = state
            .parameter_states
            .get_mut(parameter_id)
            .ok_or_else(
                || ParameterGolfSingleH100TrainingError::MissingParameterState {
                    parameter_id: parameter_id.clone(),
                },
            )?;
        parameter_state.apply_gradients(
            gradient_values.as_slice(),
            learning_rate_multiplier,
            muon_momentum,
            step_number,
        )?;
    }
    Ok(())
}

pub(crate) fn parameter_golf_parameter_state_banked_muon_shape(
    state: &ParameterGolfParameterState,
) -> Option<(usize, usize, usize)> {
    match state {
        ParameterGolfParameterState::MuonBf16 { shape, .. } => match shape.as_slice() {
            [bank_len, rows, cols] => Some((*bank_len, *rows, *cols)),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn parameter_golf_parameter_state_values_slice(
    state: &ParameterGolfParameterState,
    value_offset: usize,
    value_count: usize,
) -> Result<Vec<f32>, ParameterGolfSingleH100TrainingError> {
    let values = state.values();
    let value_end = value_offset.saturating_add(value_count);
    if value_end > values.len() {
        return Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: format!(
                "parameter state slice [{value_offset}..{value_end}) exceeded {} values",
                values.len()
            ),
        });
    }
    Ok(values[value_offset..value_end].to_vec())
}

pub(crate) fn overwrite_parameter_golf_parameter_state_values(
    state: &mut ParameterGolfParameterState,
    values: &[f32],
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    match state {
        ParameterGolfParameterState::MuonBf16 {
            values: state_values,
            bf16_bits,
            ..
        } => {
            if state_values.len() != values.len() {
                return Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "parameter state overwrite expected {} values but received {}",
                        state_values.len(),
                        values.len()
                    ),
                });
            }
            state_values.copy_from_slice(values);
            round_values_to_bf16(state_values.as_mut_slice());
            *bf16_bits = bf16_bits_from_f32_values(state_values.as_slice());
            Ok(())
        }
        _ => Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: String::from(
                "parameter state overwrite currently supports only Muon banked tensors",
            ),
        }),
    }
}

pub(crate) fn apply_parameter_golf_muon_bank_shard_to_state(
    state: &mut ParameterGolfParameterState,
    owned_slice_start: usize,
    owned_slice_count: usize,
    gradients: &[f32],
    learning_rate_multiplier: f32,
    muon_momentum: f32,
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    match state {
        ParameterGolfParameterState::MuonBf16 {
            shape,
            values,
            bf16_bits,
            optimizer,
            optimizer_state,
        } => {
            let [bank_len, rows, cols] = shape.as_slice() else {
                return Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "bank-shard Muon update requires one rank-3 bank shape, found {:?}",
                        shape
                    ),
                });
            };
            let stride = rows.saturating_mul(*cols);
            let owned_slice_end = owned_slice_start.saturating_add(owned_slice_count);
            if owned_slice_end > *bank_len {
                return Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "bank-shard Muon update slice [{owned_slice_start}..{owned_slice_end}) exceeded bank length {}",
                        bank_len
                    ),
                });
            }
            let expected_gradient_len = owned_slice_count.saturating_mul(stride);
            if gradients.len() != expected_gradient_len {
                return Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "bank-shard Muon update expected {} gradient values but received {}",
                        expected_gradient_len,
                        gradients.len()
                    ),
                });
            }
            let value_offset = owned_slice_start.saturating_mul(stride);
            let value_end = value_offset.saturating_add(expected_gradient_len);
            let mut owned_optimizer_state = crate::ParameterGolfMuonState {
                momentum_buffer: optimizer_state.momentum_buffer[value_offset..value_end].to_vec(),
            };
            let mut optimizer = optimizer.clone();
            optimizer.learning_rate *= learning_rate_multiplier;
            optimizer.momentum = muon_momentum;
            apply_parameter_golf_cuda_muon_step(
                &mut values[value_offset..value_end],
                &[owned_slice_count, *rows, *cols],
                gradients,
                &optimizer,
                &mut owned_optimizer_state,
            )?;
            optimizer_state.momentum_buffer[value_offset..value_end]
                .copy_from_slice(owned_optimizer_state.momentum_buffer.as_slice());
            round_values_to_bf16(&mut values[value_offset..value_end]);
            bf16_bits[value_offset..value_end]
                .copy_from_slice(&bf16_bits_from_f32_values(&values[value_offset..value_end]));
            Ok(())
        }
        other => Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: format!(
                "bank-shard Muon update requires one Muon banked parameter state, found {other:?}"
            ),
        }),
    }
}

pub(crate) fn materialize_current_model(
    baseline: &ParameterGolfReferenceModel,
    state: &ParameterGolfSingleH100TrainerState,
) -> Result<ParameterGolfReferenceModel, ParameterGolfSingleH100TrainingError> {
    let overrides = current_parameter_state_overrides(state);
    materialize_model_from_overrides(baseline, &overrides)
}

fn materialize_model_from_overrides(
    baseline: &ParameterGolfReferenceModel,
    overrides: &BTreeMap<String, Vec<f32>>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfSingleH100TrainingError> {
    let weights = if uses_banked_runtime_surface(overrides) {
        baseline
            .banked_weights()?
            .with_parameter_overrides(&baseline.descriptor().config, overrides)?
            .to_split(&baseline.descriptor().config)?
    } else {
        baseline
            .weights()
            .with_parameter_overrides(&baseline.descriptor().config, overrides)?
    };
    Ok(ParameterGolfReferenceModel::new(
        baseline.descriptor().model.clone(),
        baseline.descriptor().config.clone(),
        weights,
    )?)
}

pub(crate) fn materialize_current_banked_weights(
    baseline: &ParameterGolfReferenceModel,
    state: &ParameterGolfSingleH100TrainerState,
) -> Result<psionic_models::ParameterGolfBankedWeights, ParameterGolfSingleH100TrainingError> {
    let config = &baseline.descriptor().config;
    let banked_weights = baseline.banked_weights()?;
    let allowed_parameter_ids = banked_weights
        .parameter_vectors(config)
        .into_iter()
        .map(|vector| vector.parameter_id)
        .collect::<std::collections::BTreeSet<_>>();
    let overrides = current_parameter_state_overrides(state)
        .into_iter()
        .filter(|(parameter_id, _)| allowed_parameter_ids.contains(parameter_id))
        .collect::<BTreeMap<_, _>>();
    banked_weights
        .with_parameter_overrides(config, &overrides)
        .map_err(Into::into)
}

fn current_parameter_state_overrides(
    state: &ParameterGolfSingleH100TrainerState,
) -> BTreeMap<String, Vec<f32>> {
    state
        .parameter_states
        .iter()
        .map(|(parameter_id, state)| (parameter_id.clone(), state.values().to_vec()))
        .collect::<BTreeMap<_, _>>()
}

fn uses_banked_runtime_surface(overrides: &BTreeMap<String, Vec<f32>>) -> bool {
    overrides.keys().any(|parameter_id| {
        psionic_models::PARAMETER_GOLF_MATRIX_BANK_NAMES.contains(&parameter_id.as_str())
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_parameter_golf_training_gradient_batch_from_examples(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    current_model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    training_session_cache: Option<&mut BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    sequence_length: usize,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
) -> Result<
    (
        f32,
        BTreeMap<String, Vec<f32>>,
        ParameterGolfSingleH100PhaseTimings,
        Option<ParameterGolfTrainingBatchRuntime>,
    ),
    ParameterGolfSingleH100TrainingError,
> {
    execute_parameter_golf_training_gradient_batch_from_examples_with_matrix_execution_mode(
        cuda_backend,
        device,
        current_model,
        explicit_banked_weights,
        graph_cache,
        training_session_cache,
        sequence_length,
        input_ids,
        target_ids,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_parameter_golf_training_gradient_batch_from_examples_with_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    current_model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    training_session_cache: Option<&mut BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    sequence_length: usize,
    input_ids: &[Vec<u32>],
    target_ids: &[Vec<u32>],
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<
    (
        f32,
        BTreeMap<String, Vec<f32>>,
        ParameterGolfSingleH100PhaseTimings,
        Option<ParameterGolfTrainingBatchRuntime>,
    ),
    ParameterGolfSingleH100TrainingError,
> {
    let batch_size = input_ids.len();
    let mut phase_timings = ParameterGolfSingleH100PhaseTimings::default();
    let (graph, backward_plan, loss, backward_outputs, runtime) =
        if let Some(training_session_cache) = training_session_cache {
            let session = training_session_for_batch(
                training_session_cache,
                cuda_backend,
                graph_cache,
                device.clone(),
                current_model,
                explicit_banked_weights,
                batch_size,
                sequence_length,
                matrix_execution_mode,
            )?;
            let (loss, backward_outputs, runtime) =
                session.execute_batch(cuda_backend, input_ids, target_ids)?;
            (
                session.graph.clone(),
                session.backward_plan.clone(),
                loss,
                backward_outputs,
                Some(runtime),
            )
        } else {
            let graph = training_graph_for_batch(
                graph_cache,
                device.clone(),
                current_model,
                batch_size,
                sequence_length,
                matrix_execution_mode,
            )?;
            let inputs = bind_parameter_golf_baseline_training_graph_inputs_with_banked_weights(
                graph,
                current_model,
                explicit_banked_weights,
                input_ids,
                target_ids,
            )?;
            let backward_plan = parameter_only_backward_plan(graph)?;
            let retained_graph = retained_forward_graph(graph, &backward_plan);
            let input_buffers = cuda_graph_input_buffers(cuda_backend, &retained_graph, &inputs)?;
            let forward_started = Instant::now();
            let forward_outputs = execute_cuda_graph_output_buffers_from_buffers(
                cuda_backend,
                &retained_graph,
                &input_buffers,
            )?;
            let forward_loss_cuda_ms = duration_ms(forward_started);
            let loss = scalar_float_cuda_buffer_output(&forward_outputs, graph.loss_tensor_id)?;
            let backward_started = Instant::now();
            let backward_outputs = execute_backward_plan(
                cuda_backend,
                graph.graph.graph(),
                &backward_plan,
                &input_buffers,
                &forward_outputs,
            )?;
            let backward_cuda_ms = duration_ms(backward_started);
            (
                graph.clone(),
                backward_plan,
                loss,
                backward_outputs,
                Some(ParameterGolfTrainingBatchRuntime {
                    resident_parameter_upload_us: 0,
                    parameter_refresh_us: 0,
                    input_token_write_us: 0,
                    target_token_write_us: 0,
                    forward_loss_cuda_ms,
                    backward_cuda_ms,
                    persistent_parameter_buffer_count: 0,
                    persistent_parameter_value_count: 0,
                    resident_parameter_buffers_reused: false,
                }),
            )
        };
    phase_timings.forward_loss_cuda_ms = runtime
        .as_ref()
        .map_or(0, |receipt| receipt.forward_loss_cuda_ms);
    phase_timings.backward_cuda_ms = runtime
        .as_ref()
        .map_or(0, |receipt| receipt.backward_cuda_ms);
    let retained_binding_tensor_ids = retained_forward_tensor_ids(&graph, &backward_plan);
    phase_timings.retained_binding_tensor_count = retained_binding_tensor_ids.len() as u32;
    phase_timings.retained_binding_f32_count =
        count_retained_tensor_elements(&graph, retained_binding_tensor_ids.as_slice())?;
    phase_timings.gradient_tensor_count = backward_plan.gradient_targets.len() as u32;
    phase_timings.gradient_f32_count = count_gradient_elements(&backward_plan)?;

    let materialize_started = Instant::now();
    let backward_result =
        backward_result_from_outputs(&backward_plan, backward_outputs.as_slice())?;
    let gradients = materialize_parameter_golf_baseline_training_gradients(
        &graph,
        &backward_result,
        &current_model.descriptor().config,
        input_ids,
    )?;
    phase_timings.host_gradient_materialization_ms = duration_ms(materialize_started);

    Ok((loss, gradients.parameter_gradients, phase_timings, runtime))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_parameter_golf_training_gradient_batch(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    bundle: &ParameterGolfDatasetBundle,
    current_model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    training_session_cache: Option<&mut BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    geometry: &ParameterGolfBatchGeometry,
    window: &ParameterGolfTokenStreamWindow,
) -> Result<ParameterGolfTrainingGradientBatchResult, ParameterGolfSingleH100TrainingError> {
    execute_parameter_golf_training_gradient_batch_with_matrix_execution_mode(
        cuda_backend,
        device,
        bundle,
        current_model,
        explicit_banked_weights,
        graph_cache,
        training_session_cache,
        geometry,
        window,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_parameter_golf_training_gradient_batch_with_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    bundle: &ParameterGolfDatasetBundle,
    current_model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    training_session_cache: Option<&mut BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    geometry: &ParameterGolfBatchGeometry,
    window: &ParameterGolfTokenStreamWindow,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfTrainingGradientBatchResult, ParameterGolfSingleH100TrainingError> {
    let mut phase_timings = ParameterGolfSingleH100PhaseTimings::default();

    let tokens_started = Instant::now();
    let tokens = materialize_parameter_golf_token_window(bundle, window)?;
    phase_timings.token_materialization_ms = duration_ms(tokens_started);
    let (input_ids, target_ids) = training_batch_from_window_tokens(tokens.as_slice(), geometry)?;
    let (loss, gradients, batch_phase_timings, runtime) =
        execute_parameter_golf_training_gradient_batch_from_examples_with_matrix_execution_mode(
            cuda_backend,
            device,
            current_model,
            explicit_banked_weights,
            graph_cache,
            training_session_cache,
            geometry.train_sequence_length,
            input_ids.as_slice(),
            target_ids.as_slice(),
            matrix_execution_mode,
        )?;
    phase_timings.accumulate(&batch_phase_timings);

    Ok(ParameterGolfTrainingGradientBatchResult {
        window_id: window.window_id.clone(),
        loss,
        phase_timings,
        parameter_gradients: gradients,
        runtime,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_training_step(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    bundle: &ParameterGolfDatasetBundle,
    train_contract: &ParameterGolfTokenStreamContract,
    cursor: &mut ParameterGolfTokenStreamCursor,
    baseline_model: &ParameterGolfReferenceModel,
    current_model: &mut ParameterGolfReferenceModel,
    trainer_state: &mut ParameterGolfSingleH100TrainerState,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    training_session_cache: &mut BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    geometry: &ParameterGolfBatchGeometry,
    requested_train_tokens: u64,
    global_step: u64,
    max_steps: u64,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    grad_clip_norm: f32,
    learning_rate_multiplier: f32,
    muon_momentum: f32,
    effective_learning_rate: Option<f32>,
    remaining_training_wallclock_ms: Option<u64>,
    emit_micro_step_logs: bool,
    live_visualization_writer: &mut Option<crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100TrainingStepMetrics, ParameterGolfSingleH100TrainingError> {
    let step_started = Instant::now();
    let current_banked_weights = current_model.banked_weights()?;
    if emit_micro_step_logs {
        emit_progress_line(format!(
            "train_step_start step={}/{} grad_accum_steps={}",
            global_step, max_steps, geometry.grad_accum_steps,
        ));
    }
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "training",
            Some(String::from("optimizer_step")),
            format!("Optimizer step {} of {} started.", global_step, max_steps),
            vec![
                String::from("dataloader"),
                String::from("forward"),
                String::from("optimizer"),
            ],
            Some(global_step),
            Some(0),
            true,
        )?;
    }

    let mut accumulated_gradients = zero_gradients(trainer_state);
    let mut microbatch_loss_sum = 0.0_f32;
    let mut window_ids = Vec::new();
    let mut step_profile = ParameterGolfSingleH100PhaseTimings::default();
    let mut batch_runtime_totals = ParameterGolfTrainingBatchRuntime::default();
    for micro_step in 0..geometry.grad_accum_steps {
        let plan_started = Instant::now();
        let window = train_contract
            .plan_window(&bundle.manifest, cursor, requested_train_tokens)?
            .ok_or_else(|| ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from("could not plan the next training token window"),
            })?;
        step_profile.window_planning_ms = step_profile
            .window_planning_ms
            .saturating_add(duration_ms(plan_started));
        *cursor = window.end_cursor.clone();
        let gradient_batch =
            execute_parameter_golf_training_gradient_batch_with_matrix_execution_mode(
                cuda_backend,
                device,
                bundle,
                current_model,
                Some(&current_banked_weights),
                graph_cache,
                Some(training_session_cache),
                geometry,
                &window,
                matrix_execution_mode,
            )?;
        window_ids.push(gradient_batch.window_id.clone());
        microbatch_loss_sum += gradient_batch.loss;
        step_profile.accumulate(&gradient_batch.phase_timings);
        if let Some(runtime) = gradient_batch.runtime.as_ref() {
            batch_runtime_totals.resident_parameter_upload_us = batch_runtime_totals
                .resident_parameter_upload_us
                .saturating_add(runtime.resident_parameter_upload_us);
            batch_runtime_totals.parameter_refresh_us = batch_runtime_totals
                .parameter_refresh_us
                .saturating_add(runtime.parameter_refresh_us);
            batch_runtime_totals.input_token_write_us = batch_runtime_totals
                .input_token_write_us
                .saturating_add(runtime.input_token_write_us);
            batch_runtime_totals.target_token_write_us = batch_runtime_totals
                .target_token_write_us
                .saturating_add(runtime.target_token_write_us);
            batch_runtime_totals.persistent_parameter_buffer_count = batch_runtime_totals
                .persistent_parameter_buffer_count
                .max(runtime.persistent_parameter_buffer_count);
            batch_runtime_totals.persistent_parameter_value_count = batch_runtime_totals
                .persistent_parameter_value_count
                .max(runtime.persistent_parameter_value_count);
            batch_runtime_totals.resident_parameter_buffers_reused |=
                runtime.resident_parameter_buffers_reused;
        }
        accumulate_gradients(
            accumulated_gradients.as_mut_slice(),
            trainer_state,
            &gradient_batch.parameter_gradients,
            geometry.grad_accum_steps as f32,
        )?;
        if emit_micro_step_logs {
            emit_progress_line(format!(
                "micro_step_complete step={}/{} micro_step={}/{} window_id={} train_loss={:.8} forward_ms={} backward_ms={} host_materialization_ms={} retained_binding_f32={} gradient_f32={}",
                global_step,
                max_steps,
                micro_step + 1,
                geometry.grad_accum_steps,
                gradient_batch.window_id,
                gradient_batch.loss,
                step_profile.forward_loss_cuda_ms,
                step_profile.backward_cuda_ms,
                step_profile.host_materialization_ms(),
                step_profile.retained_binding_f32_count,
                step_profile.gradient_f32_count,
            ));
        }
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_phase(
                "training",
                Some(String::from("optimizer_step")),
                format!(
                    "Micro-step {} of {} completed for optimizer step {}.",
                    micro_step + 1,
                    geometry.grad_accum_steps,
                    global_step
                ),
                vec![
                    String::from("forward"),
                    String::from("backward"),
                    String::from("optimizer"),
                ],
                Some(global_step),
                Some((micro_step + 1) as u32),
                false,
            )?;
        }
        if let Some(remaining_training_wallclock_ms) = remaining_training_wallclock_ms {
            let observed_step_wallclock_ms = duration_ms(step_started);
            let observed_micro_steps = micro_step + 1;
            let projected_full_step_wallclock_ms = project_full_step_wallclock_ms(
                observed_step_wallclock_ms,
                observed_micro_steps,
                geometry.grad_accum_steps,
            );
            if projected_full_step_wallclock_ms > remaining_training_wallclock_ms {
                return Err(
                    ParameterGolfSingleH100TrainingError::ProjectedWallclockCapExceeded {
                        global_step,
                        observed_micro_steps,
                        grad_accum_steps: geometry.grad_accum_steps,
                        observed_step_wallclock_ms,
                        projected_full_step_wallclock_ms,
                        remaining_training_wallclock_ms,
                    },
                );
            }
        }
    }

    let clip_observation = clip_gradients(accumulated_gradients.as_mut_slice(), grad_clip_norm);
    let optimizer_started = Instant::now();
    apply_gradients_to_state(
        trainer_state,
        accumulated_gradients.as_slice(),
        learning_rate_multiplier,
        muon_momentum,
        global_step,
    )?;
    *current_model = materialize_current_model(baseline_model, trainer_state)?;
    refresh_parameter_golf_cuda_training_sessions_from_state(
        training_session_cache,
        trainer_state,
    )?;
    step_profile.optimizer_step_ms = duration_ms(optimizer_started);
    let observed_wallclock_ms = duration_ms(step_started);
    let runtime_receipt = (!training_session_cache.is_empty()).then_some(
        ParameterGolfSingleH100TrainingRuntimeReceipt {
            path: String::from("device_resident_cuda_training_graph_v1"),
            graph_surface: String::from("parameter_golf_baseline_training_graph_v2"),
            matrix_execution_mode,
            session_count: training_session_cache.len(),
            persistent_parameter_buffer_count: batch_runtime_totals
                .persistent_parameter_buffer_count,
            persistent_parameter_value_count: batch_runtime_totals.persistent_parameter_value_count,
            resident_parameter_upload_us: batch_runtime_totals.resident_parameter_upload_us,
            parameter_refresh_us: batch_runtime_totals.parameter_refresh_us,
            reusable_input_token_buffer: true,
            reusable_target_token_buffer: true,
            total_input_token_write_us: batch_runtime_totals.input_token_write_us,
            total_target_token_write_us: batch_runtime_totals.target_token_write_us,
            resident_parameter_buffers_reused: batch_runtime_totals
                .resident_parameter_buffers_reused,
        },
    );
    let step_metrics = ParameterGolfSingleH100TrainingStepMetrics {
        global_step,
        train_window_ids: window_ids,
        mean_microbatch_loss: microbatch_loss_sum / geometry.grad_accum_steps as f32,
        learning_rate_multiplier,
        muon_momentum,
        observed_wallclock_ms,
        phase_timings: step_profile,
        effective_learning_rate,
        gradient_norm_after_clip: clip_observation.gradient_norm_after_clip,
        parameter_norm_after_step: crate::state_parameter_norm(trainer_state),
        update_norm: None,
        clip_applied: clip_observation.clip_applied,
        non_finite_gradient_count: clip_observation.non_finite_count,
        tokens_per_second: crate::throughput_tokens_per_second(
            geometry.local_train_batch_tokens(),
            observed_wallclock_ms,
        ),
        samples_per_second_milli: crate::throughput_samples_per_second_milli(
            geometry.local_train_batch_sequences(),
            observed_wallclock_ms,
        ),
        runtime_receipt,
    };
    if emit_micro_step_logs {
        emit_progress_line(format!(
            "train_step_complete step={} mean_microbatch_loss={:.8} lr_mult={:.8} muon_momentum={:.8} host_materialization_ms={} optimizer_step_ms={}",
            step_metrics.global_step,
            step_metrics.mean_microbatch_loss,
            step_metrics.learning_rate_multiplier,
            step_metrics.muon_momentum,
            step_metrics.phase_timings.host_materialization_ms(),
            step_metrics.phase_timings.optimizer_step_ms,
        ));
        if let Some(runtime_receipt) = step_metrics.runtime_receipt.as_ref() {
            emit_progress_line(format!(
                "train_runtime_receipt step={} path={} graph_surface={} matrix_execution_mode={} sessions={} stable_parameter_buffers={} stable_parameter_values={} resident_parameter_upload_us={} parameter_refresh_us={} input_token_write_us={} target_token_write_us={} resident_buffers_reused={}",
                step_metrics.global_step,
                runtime_receipt.path,
                runtime_receipt.graph_surface,
                runtime_receipt.matrix_execution_mode.as_str(),
                runtime_receipt.session_count,
                runtime_receipt.persistent_parameter_buffer_count,
                runtime_receipt.persistent_parameter_value_count,
                runtime_receipt.resident_parameter_upload_us,
                runtime_receipt.parameter_refresh_us,
                runtime_receipt.total_input_token_write_us,
                runtime_receipt.total_target_token_write_us,
                runtime_receipt.resident_parameter_buffers_reused,
            ));
        }
    }
    Ok(step_metrics)
}

pub(crate) fn evaluate_validation_on_cuda(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_on_cuda_with_matrix_execution_mode(
        cuda_backend,
        device,
        descriptor,
        model,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        eval_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_on_cuda_with_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_on_cuda_with_parameter_buffers(
        cuda_backend,
        device,
        descriptor,
        model,
        None,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        eval_mode,
        matrix_execution_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_on_cuda_with_resident_training_parameters(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: &BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_on_cuda_with_resident_training_parameters_and_matrix_execution_mode(
        cuda_backend,
        device,
        descriptor,
        model,
        resident_training_sessions,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        eval_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_on_cuda_with_resident_training_parameters_and_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: &BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_on_cuda_with_parameter_buffers(
        cuda_backend,
        device,
        descriptor,
        model,
        Some(resident_training_sessions),
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        eval_mode,
        matrix_execution_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_validation_on_cuda_with_parameter_buffers(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: Option<&BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    eval_mode.validate(sequence_length)?;
    if validation_tokens.len() <= sequence_length {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: String::from(
                "validation split is too short for the requested sequence length",
            ),
        });
    }
    let total_units = validation_unit_count(validation_tokens, sequence_length, eval_mode);
    let validation_batch_sequences = batch_sequences.max(1);
    let byte_accounting_started = Instant::now();
    let batch_plans = build_validation_batch_plans(
        validation_tokens,
        byte_luts,
        sequence_length,
        validation_batch_sequences,
        eval_mode,
    )?;
    let total_byte_accounting_us = duration_us(byte_accounting_started);
    evaluate_validation_batch_plans_on_cuda(
        cuda_backend,
        device,
        descriptor,
        model,
        sequence_length,
        eval_mode,
        matrix_execution_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
        total_units,
        total_byte_accounting_us,
        batch_plans,
        resident_training_sessions,
    )
}

pub(crate) fn evaluate_validation_window_starts_on_cuda(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
    window_starts: &[usize],
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_window_starts_on_cuda_with_matrix_execution_mode(
        cuda_backend,
        device,
        descriptor,
        model,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        stride,
        window_starts,
        graph_cache,
        stage_label,
        live_visualization_writer,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_window_starts_on_cuda_with_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
    window_starts: &[usize],
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_window_starts_on_cuda_with_parameter_buffers(
        cuda_backend,
        device,
        descriptor,
        model,
        None,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        stride,
        window_starts,
        matrix_execution_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_window_starts_on_cuda_with_resident_training_parameters(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: &BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
    window_starts: &[usize],
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_window_starts_on_cuda_with_resident_training_parameters_and_matrix_execution_mode(
        cuda_backend,
        device,
        descriptor,
        model,
        resident_training_sessions,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        stride,
        window_starts,
        graph_cache,
        stage_label,
        live_visualization_writer,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_window_starts_on_cuda_with_resident_training_parameters_and_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: &BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
    window_starts: &[usize],
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_window_starts_on_cuda_with_parameter_buffers(
        cuda_backend,
        device,
        descriptor,
        model,
        Some(resident_training_sessions),
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        stride,
        window_starts,
        matrix_execution_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_validation_window_starts_on_cuda_with_parameter_buffers(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: Option<&BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
    window_starts: &[usize],
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    let eval_mode = ParameterGolfValidationEvalMode::SlidingWindow { stride };
    eval_mode.validate(sequence_length)?;
    if validation_tokens.len() <= sequence_length {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: String::from(
                "validation split is too short for the requested sequence length",
            ),
        });
    }
    let validation_batch_sequences = batch_sequences.max(1);
    let byte_accounting_started = Instant::now();
    let batch_plans = build_sliding_window_validation_batch_plans_from_window_starts(
        validation_tokens,
        byte_luts,
        sequence_length,
        validation_batch_sequences,
        stride,
        window_starts,
    )?;
    let total_byte_accounting_us = duration_us(byte_accounting_started);
    evaluate_validation_batch_plans_on_cuda(
        cuda_backend,
        device,
        descriptor,
        model,
        sequence_length,
        &eval_mode,
        matrix_execution_mode,
        graph_cache,
        stage_label,
        live_visualization_writer,
        window_starts.len(),
        total_byte_accounting_us,
        batch_plans,
        resident_training_sessions,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_with_optional_score_first_ttt_on_cuda(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    score_first_ttt: Option<&ParameterGolfScoreFirstTttConfig>,
    eval_graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    train_graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    evaluate_validation_with_optional_score_first_ttt_on_cuda_and_matrix_execution_mode(
        cuda_backend,
        device,
        descriptor,
        model,
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        eval_mode,
        score_first_ttt,
        eval_graph_cache,
        train_graph_cache,
        stage_label,
        live_visualization_writer,
        ParameterGolfMatrixExecutionMode::DirectBanked,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_validation_with_optional_score_first_ttt_on_cuda_and_matrix_execution_mode(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    score_first_ttt: Option<&ParameterGolfScoreFirstTttConfig>,
    eval_graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    train_graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    match score_first_ttt {
        Some(score_first_ttt) => evaluate_score_first_ttt_on_cuda(
            cuda_backend,
            device,
            descriptor,
            model,
            validation_tokens,
            byte_luts,
            sequence_length,
            batch_sequences,
            eval_mode,
            score_first_ttt,
            eval_graph_cache,
            train_graph_cache,
            stage_label,
            live_visualization_writer,
            matrix_execution_mode,
        ),
        None => evaluate_validation_on_cuda_with_matrix_execution_mode(
            cuda_backend,
            device,
            descriptor,
            model,
            validation_tokens,
            byte_luts,
            sequence_length,
            batch_sequences,
            eval_mode,
            eval_graph_cache,
            stage_label,
            live_visualization_writer,
            matrix_execution_mode,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_score_first_ttt_on_cuda(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    _batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    score_first_ttt: &ParameterGolfScoreFirstTttConfig,
    eval_graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    train_graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    let stride = match eval_mode {
        ParameterGolfValidationEvalMode::SlidingWindow { stride } => *stride,
        ParameterGolfValidationEvalMode::NonOverlapping => {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: String::from(
                    "legal score-first TTT requires sliding-window validation semantics",
                ),
            });
        }
    };
    if stride != score_first_ttt.stride {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: format!(
                "legal score-first TTT stride {} did not match validation_eval_mode stride {}",
                score_first_ttt.stride, stride
            ),
        });
    }
    if validation_tokens.len() <= sequence_length {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: String::from(
                "validation split is too short for the requested sequence length",
            ),
        });
    }

    let chunk_plans = build_parameter_golf_score_first_ttt_chunk_plans(
        validation_tokens.len().saturating_sub(1),
        sequence_length,
        score_first_ttt,
    );
    let total_chunks = chunk_plans.len();
    let total_score_window_count = chunk_plans
        .iter()
        .map(|plan| plan.receipt_plan.score_window_count)
        .sum::<usize>();
    emit_progress_line(format!(
        "validation_score_first_ttt_start stage={} stride={} chunk_tokens={} chunks={} score_windows={} epochs={} freeze_blocks={} batch_sequences={}",
        stage_label,
        score_first_ttt.stride,
        score_first_ttt.chunk_tokens,
        chunk_plans.len(),
        total_score_window_count,
        score_first_ttt.epochs,
        score_first_ttt.freeze_blocks,
        score_first_ttt.batch_sequences,
    ));

    let base_model = model.clone();
    let mut current_model = model.clone();
    let mut trainable_states =
        seed_parameter_golf_score_first_ttt_states(&base_model, score_first_ttt.freeze_blocks);
    let mut training_session_cache = BTreeMap::new();
    let mut live_visualization_writer = live_visualization_writer;
    let mut total_loss_sum = 0.0_f64;
    let mut total_token_count = 0_u64;
    let mut total_byte_count = 0_u64;
    let mut chunk_receipts = Vec::with_capacity(chunk_plans.len());
    let mut total_adaptation_step_count = 0_usize;

    for chunk_plan in chunk_plans {
        let score_summary = if training_session_cache.is_empty() {
            evaluate_validation_window_starts_on_cuda_with_matrix_execution_mode(
                cuda_backend,
                device,
                descriptor,
                &current_model,
                validation_tokens,
                byte_luts,
                sequence_length,
                score_first_ttt.batch_sequences.max(1),
                score_first_ttt.stride,
                chunk_plan.window_starts.as_slice(),
                eval_graph_cache,
                stage_label,
                live_visualization_writer
                    .as_mut()
                    .map(|writer| &mut **writer),
                matrix_execution_mode,
            )?
        } else {
            evaluate_validation_window_starts_on_cuda_with_resident_training_parameters_and_matrix_execution_mode(
                cuda_backend,
                device,
                descriptor,
                &base_model,
                &training_session_cache,
                validation_tokens,
                byte_luts,
                sequence_length,
                score_first_ttt.batch_sequences.max(1),
                score_first_ttt.stride,
                chunk_plan.window_starts.as_slice(),
                eval_graph_cache,
                stage_label,
                live_visualization_writer
                    .as_mut()
                    .map(|writer| &mut **writer),
                matrix_execution_mode,
            )?
        };
        total_loss_sum += score_summary.mean_loss * score_summary.evaluated_token_count as f64;
        total_token_count = total_token_count.saturating_add(score_summary.evaluated_token_count);
        total_byte_count = total_byte_count.saturating_add(score_summary.evaluated_byte_count);

        let is_last_chunk = chunk_plan.receipt_plan.chunk_index + 1 == total_chunks.max(1);
        let mut adaptation_steps = Vec::new();
        let chunk_start = chunk_plan.receipt_plan.chunk_start_token;
        let chunk_end = chunk_plan.receipt_plan.chunk_end_token;
        let chunk_sequence_count = chunk_end.saturating_sub(chunk_start) / sequence_length;
        let chunk_learning_rate = parameter_golf_score_first_ttt_chunk_learning_rate(
            score_first_ttt,
            chunk_plan.receipt_plan.chunk_index,
            total_chunks,
        );
        if !is_last_chunk && score_first_ttt.epochs > 0 && chunk_sequence_count > 0 {
            for epoch_index in 0..score_first_ttt.epochs {
                for (batch_index, batch_sequence_start) in (0..chunk_sequence_count)
                    .step_by(score_first_ttt.batch_sequences)
                    .enumerate()
                {
                    let batch_sequence_end = (batch_sequence_start
                        + score_first_ttt.batch_sequences)
                        .min(chunk_sequence_count);
                    let token_start =
                        chunk_start + batch_sequence_start.saturating_mul(sequence_length);
                    let token_end =
                        chunk_start + batch_sequence_end.saturating_mul(sequence_length) + 1;
                    if token_end > validation_tokens.len() {
                        continue;
                    }
                    let (input_ids, target_ids) = training_batch_from_flat_tokens(
                        &validation_tokens[token_start..token_end],
                        sequence_length,
                    )?;
                    let step_started = Instant::now();
                    let (_loss, gradients, _, _) =
                        execute_parameter_golf_training_gradient_batch_from_examples_with_matrix_execution_mode(
                            cuda_backend,
                            device,
                            &current_model,
                            Some(&current_model.banked_weights()?),
                            train_graph_cache,
                            Some(&mut training_session_cache),
                            sequence_length,
                            input_ids.as_slice(),
                            target_ids.as_slice(),
                            matrix_execution_mode,
                        )?;
                    let mut trainable_gradients = gradients
                        .into_iter()
                        .filter(|(parameter_id, _)| trainable_states.contains_key(parameter_id))
                        .collect::<Vec<_>>();
                    let clip_observation = clip_gradients(
                        trainable_gradients.as_mut_slice(),
                        score_first_ttt.grad_clip_norm,
                    );
                    apply_parameter_golf_score_first_ttt_gradients(
                        &mut trainable_states,
                        trainable_gradients.as_slice(),
                        chunk_learning_rate,
                        score_first_ttt.momentum,
                    )?;
                    current_model = materialize_parameter_golf_score_first_ttt_model(
                        &base_model,
                        &trainable_states,
                    )?;
                    refresh_parameter_golf_cuda_training_sessions(
                        &mut training_session_cache,
                        &current_model,
                        Some(&current_model.banked_weights()?),
                    )?;
                    total_adaptation_step_count = total_adaptation_step_count.saturating_add(1);
                    let mean_loss =
                        current_model.loss(input_ids.as_slice(), target_ids.as_slice())?;
                    adaptation_steps.push(ParameterGolfScoreFirstTttAdaptationStepReceipt {
                        epoch_index,
                        batch_index,
                        train_sequence_start: batch_sequence_start,
                        train_sequence_count: batch_sequence_end
                            .saturating_sub(batch_sequence_start),
                        train_token_count: ((batch_sequence_end
                            .saturating_sub(batch_sequence_start))
                        .saturating_mul(sequence_length))
                            as u64,
                        mean_loss,
                        learning_rate: chunk_learning_rate,
                        gradient_norm_after_clip: clip_observation.gradient_norm_after_clip,
                        clip_applied: clip_observation.clip_applied,
                        non_finite_gradient_count: clip_observation.non_finite_count,
                        observed_ms: duration_ms(step_started),
                    });
                }
            }
        }

        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_phase(
                "training",
                Some(String::from("validation")),
                format!(
                    "Score-first TTT chunk {} of {} completed for stage `{stage_label}`.",
                    chunk_plan.receipt_plan.chunk_index + 1,
                    total_chunks.max(1)
                ),
                vec![String::from("validation"), String::from("optimizer")],
                None,
                None,
                false,
            )?;
        }

        chunk_receipts.push(ParameterGolfScoreFirstTttChunkReceipt {
            plan: chunk_plan.receipt_plan,
            score_summary,
            adaptation_applied: !adaptation_steps.is_empty(),
            adaptation_learning_rate: (!adaptation_steps.is_empty()).then_some(chunk_learning_rate),
            adaptation_sequence_count: chunk_sequence_count,
            adaptation_token_count: chunk_sequence_count.saturating_mul(sequence_length) as u64,
            adaptation_steps,
        });
    }

    let mean_loss = total_loss_sum / total_token_count.max(1) as f64;
    let bits_per_byte = (mean_loss / std::f64::consts::LN_2)
        * (total_token_count as f64 / total_byte_count.max(1) as f64);
    emit_progress_line(format!(
        "validation_score_first_ttt_complete stage={} mean_loss={:.8} val_bpb={:.8} score_windows={} adaptation_steps={} evaluated_tokens={} evaluated_bytes={}",
        stage_label,
        mean_loss,
        bits_per_byte,
        total_score_window_count,
        total_adaptation_step_count,
        total_token_count,
        total_byte_count,
    ));
    Ok(ParameterGolfSingleH100ValidationSummary {
        eval_mode: eval_mode.clone(),
        evaluated_sequence_count: total_score_window_count,
        evaluated_token_count: total_token_count,
        evaluated_byte_count: total_byte_count,
        mean_loss,
        bits_per_byte,
        runtime_receipt: None,
        score_first_ttt_receipt: Some(ParameterGolfScoreFirstTttReceipt {
            path: String::from("device_resident_cuda_score_first_ttt_eval_v1"),
            config: score_first_ttt.clone(),
            total_chunks,
            total_score_window_count,
            total_adaptation_step_count,
            last_chunk_training_skipped: true,
            chunk_receipts,
        }),
    })
}

pub(crate) fn build_parameter_golf_score_first_ttt_chunk_plans(
    total_tokens: usize,
    sequence_length: usize,
    score_first_ttt: &ParameterGolfScoreFirstTttConfig,
) -> Vec<ParameterGolfScoreFirstTttChunkExecutionPlan> {
    let num_chunks = total_tokens.div_ceil(score_first_ttt.chunk_tokens.max(1));
    let mut chunk_windows = vec![Vec::new(); num_chunks];
    for window_start in (0..total_tokens).step_by(score_first_ttt.stride.max(1)) {
        let valid_length = total_tokens
            .saturating_sub(window_start)
            .min(sequence_length);
        if valid_length < score_first_ttt.stride && window_start != 0 {
            continue;
        }
        let score_start = if window_start == 0 {
            0
        } else {
            valid_length.saturating_sub(score_first_ttt.stride)
        };
        let scored_start = window_start.saturating_add(score_start);
        let chunk_index =
            (scored_start / score_first_ttt.chunk_tokens.max(1)).min(num_chunks.saturating_sub(1));
        chunk_windows[chunk_index].push(window_start);
    }
    chunk_windows
        .into_iter()
        .enumerate()
        .map(|(chunk_index, window_starts)| {
            let chunk_start_token = chunk_index.saturating_mul(score_first_ttt.chunk_tokens);
            let chunk_end_token =
                ((chunk_index + 1).saturating_mul(score_first_ttt.chunk_tokens)).min(total_tokens);
            ParameterGolfScoreFirstTttChunkExecutionPlan {
                receipt_plan: ParameterGolfScoreFirstTttChunkPlan {
                    chunk_index,
                    chunk_start_token,
                    chunk_end_token,
                    first_window_start: window_starts.first().copied(),
                    last_window_start: window_starts.last().copied(),
                    score_window_count: window_starts.len(),
                },
                window_starts,
            }
        })
        .collect()
}

pub(crate) fn parameter_golf_score_first_ttt_chunk_learning_rate(
    score_first_ttt: &ParameterGolfScoreFirstTttConfig,
    chunk_index: usize,
    total_chunks: usize,
) -> f32 {
    score_first_ttt.learning_rate
        * 0.5
        * (1.0
            + (std::f32::consts::PI * chunk_index as f32
                / total_chunks.saturating_sub(1).max(1) as f32)
                .cos())
}

pub(crate) fn seed_parameter_golf_score_first_ttt_states(
    model: &ParameterGolfReferenceModel,
    freeze_blocks: usize,
) -> BTreeMap<String, ParameterGolfScoreFirstTttParameterState> {
    model
        .weights()
        .parameter_vectors(&model.descriptor().config)
        .into_iter()
        .filter(|vector| {
            !parameter_golf_score_first_ttt_parameter_is_frozen(
                vector.parameter_id.as_str(),
                freeze_blocks,
            )
        })
        .map(|vector| {
            (
                vector.parameter_id.clone(),
                ParameterGolfScoreFirstTttParameterState {
                    momentum_buffer: vec![0.0; vector.values.len()],
                    values: vector.values,
                },
            )
        })
        .collect()
}

fn parameter_golf_score_first_ttt_parameter_is_frozen(
    parameter_id: &str,
    freeze_blocks: usize,
) -> bool {
    (0..freeze_blocks)
        .any(|block_index| parameter_id.starts_with(&format!("blocks.{block_index}.")))
}

pub(crate) fn materialize_parameter_golf_score_first_ttt_model(
    baseline_model: &ParameterGolfReferenceModel,
    trainable_states: &BTreeMap<String, ParameterGolfScoreFirstTttParameterState>,
) -> Result<ParameterGolfReferenceModel, ParameterGolfSingleH100TrainingError> {
    let overrides = trainable_states
        .iter()
        .map(|(parameter_id, state)| (parameter_id.clone(), state.values.clone()))
        .collect::<BTreeMap<_, _>>();
    let weights = baseline_model
        .weights()
        .with_parameter_overrides(&baseline_model.descriptor().config, &overrides)?;
    Ok(ParameterGolfReferenceModel::new(
        baseline_model.descriptor().model.clone(),
        baseline_model.descriptor().config.clone(),
        weights,
    )?)
}

pub(crate) fn apply_parameter_golf_score_first_ttt_gradients(
    trainable_states: &mut BTreeMap<String, ParameterGolfScoreFirstTttParameterState>,
    gradients: &[(String, Vec<f32>)],
    learning_rate: f32,
    momentum: f32,
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    for (parameter_id, gradient_values) in gradients {
        let state = trainable_states.get_mut(parameter_id).ok_or_else(|| {
            ParameterGolfSingleH100TrainingError::MissingParameterState {
                parameter_id: parameter_id.clone(),
            }
        })?;
        if state.values.len() != gradient_values.len()
            || state.momentum_buffer.len() != gradient_values.len()
        {
            return Err(ParameterGolfSingleH100TrainingError::Serialization {
                message: format!(
                    "score-first TTT parameter `{parameter_id}` expected {} gradient values but observed {}",
                    state.values.len(),
                    gradient_values.len(),
                ),
            });
        }
        for ((value, momentum_buffer), gradient) in state
            .values
            .iter_mut()
            .zip(state.momentum_buffer.iter_mut())
            .zip(gradient_values.iter())
        {
            *momentum_buffer = (*momentum_buffer * momentum) + *gradient;
            *value -= learning_rate * *momentum_buffer;
        }
    }
    Ok(())
}

pub(crate) fn training_batch_from_flat_tokens(
    tokens: &[u16],
    sequence_length: usize,
) -> Result<(Vec<Vec<u32>>, Vec<Vec<u32>>), ParameterGolfSingleH100TrainingError> {
    if tokens.len() <= sequence_length {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: format!(
                "score-first TTT adaptation requires at least {} tokens, found {}",
                sequence_length + 1,
                tokens.len()
            ),
        });
    }
    let token_count = tokens.len().saturating_sub(1);
    if token_count % sequence_length != 0 {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: format!(
                "score-first TTT adaptation token count {} must divide sequence_length {}",
                token_count, sequence_length
            ),
        });
    }
    let batch_sequences = token_count / sequence_length;
    let mut input_ids = Vec::with_capacity(batch_sequences);
    let mut target_ids = Vec::with_capacity(batch_sequences);
    for sequence_index in 0..batch_sequences {
        let start = sequence_index * sequence_length;
        let end = start + sequence_length;
        input_ids.push(
            tokens[start..end]
                .iter()
                .map(|&token_id| u32::from(token_id))
                .collect(),
        );
        target_ids.push(
            tokens[start + 1..end + 1]
                .iter()
                .map(|&token_id| u32::from(token_id))
                .collect(),
        );
    }
    Ok((input_ids, target_ids))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_validation_batch_plans_on_cuda(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    _descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    sequence_length: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
    total_units: usize,
    total_byte_accounting_us: u64,
    batch_plans: Vec<ParameterGolfValidationBatchPlan>,
    resident_training_sessions: Option<&BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    let mut total_loss_sum = 0.0_f64;
    let mut total_token_count = 0_u64;
    let mut total_byte_count = 0_u64;
    let total_batches = batch_plans.len();
    let validation_started = Instant::now();
    let mut session_cache = BTreeMap::new();
    let mut live_visualization_writer = live_visualization_writer;
    let mut total_input_token_write_us = 0_u64;
    let mut total_target_token_write_us = 0_u64;
    let mut resident_parameter_upload_us = 0_u64;
    let mut persistent_parameter_buffer_count = 0_usize;
    let mut persistent_parameter_value_count = 0_u64;
    let mut processed_units = 0_usize;

    for (batch_index, batch_plan) in batch_plans.iter().enumerate() {
        emit_progress_line(format!(
            "validation_batch_start stage={} eval_mode={} batch={}/{} batch_sequences={} evaluated_tokens={} elapsed_ms={}",
            stage_label,
            eval_mode.as_str(),
            batch_index + 1,
            total_batches,
            batch_plan.batch_sequences,
            total_token_count,
            duration_ms(validation_started),
        ));
        let session = validation_session_for_batch(
            &mut session_cache,
            cuda_backend,
            graph_cache,
            device.clone(),
            model,
            resident_training_sessions,
            batch_plan.batch_sequences,
            sequence_length,
            matrix_execution_mode,
        )?;
        let (batch_token_losses, batch_runtime) =
            session.execute_batch(cuda_backend, batch_plan)?;
        total_loss_sum += scored_token_loss_sum(
            batch_token_losses.as_slice(),
            batch_plan.sequence_plans.as_slice(),
            sequence_length,
        );
        total_token_count = total_token_count.saturating_add(batch_plan.token_count);
        total_byte_count = total_byte_count.saturating_add(batch_plan.byte_count);
        total_input_token_write_us =
            total_input_token_write_us.saturating_add(batch_runtime.input_token_write_us);
        total_target_token_write_us =
            total_target_token_write_us.saturating_add(batch_runtime.target_token_write_us);
        processed_units = processed_units.saturating_add(batch_plan.evaluation_units);
        if batch_index == 0 || (batch_index + 1) % 32 == 0 || batch_index + 1 == total_batches {
            emit_progress_line(format!(
                "validation_progress stage={} eval_mode={} batch={}/{} units={} tokens={} elapsed_ms={}",
                stage_label,
                eval_mode.as_str(),
                batch_index + 1,
                total_batches,
                processed_units,
                total_token_count,
                duration_ms(validation_started),
            ));
        }
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_phase(
                "training",
                Some(String::from("validation")),
                format!(
                    "Validation stage `{stage_label}` processed batch {} of {}.",
                    batch_index + 1,
                    total_batches
                ),
                vec![String::from("validation"), String::from("gpu_sampling")],
                None,
                Some(0),
                false,
            )?;
        }
    }

    for session in session_cache.values() {
        resident_parameter_upload_us =
            resident_parameter_upload_us.saturating_add(session.resident_parameter_upload_us);
        persistent_parameter_buffer_count = persistent_parameter_buffer_count
            .saturating_add(session.persistent_parameter_buffer_count);
        persistent_parameter_value_count = persistent_parameter_value_count
            .saturating_add(session.persistent_parameter_value_count);
    }

    let mean_loss = total_loss_sum / total_token_count.max(1) as f64;
    let bits_per_byte = (mean_loss / std::f64::consts::LN_2)
        * (total_token_count as f64 / total_byte_count.max(1) as f64);
    let local_batch_sequences = batch_plans
        .first()
        .map_or(1, |batch_plan| batch_plan.batch_sequences.max(1));
    let runtime_receipt = ParameterGolfSingleH100ValidationRuntimeReceipt {
        path: String::from("device_resident_cuda_eval_graph_v1"),
        graph_surface: String::from("parameter_golf_baseline_eval_graph_v2"),
        matrix_execution_mode,
        eval_mode: eval_mode.clone(),
        local_batch_sequences,
        session_count: session_cache.len(),
        total_batches,
        total_units,
        persistent_parameter_buffer_count,
        persistent_parameter_value_count,
        resident_parameter_upload_us,
        per_batch_stable_parameter_buffer_allocations: 0,
        reusable_input_token_buffer: true,
        reusable_target_token_buffer: true,
        total_input_token_write_us,
        total_target_token_write_us,
        byte_accounting_mode: String::from("precomputed_batch_target_bytes"),
        total_byte_accounting_us,
    };
    emit_progress_line(format!(
        "validation_runtime_receipt stage={} eval_mode={} path={} graph_surface={} matrix_execution_mode={} batch_sequences={} sessions={} total_units={} stable_parameter_buffers={} stable_parameter_values={} resident_parameter_upload_us={} input_token_write_us={} target_token_write_us={} byte_accounting_us={}",
        stage_label,
        eval_mode.as_str(),
        runtime_receipt.path,
        runtime_receipt.graph_surface,
        runtime_receipt.matrix_execution_mode.as_str(),
        runtime_receipt.local_batch_sequences,
        runtime_receipt.session_count,
        runtime_receipt.total_units,
        runtime_receipt.persistent_parameter_buffer_count,
        runtime_receipt.persistent_parameter_value_count,
        runtime_receipt.resident_parameter_upload_us,
        runtime_receipt.total_input_token_write_us,
        runtime_receipt.total_target_token_write_us,
        runtime_receipt.total_byte_accounting_us,
    ));
    emit_progress_line(format!(
        "validation_complete stage={} eval_mode={} mean_loss={:.8} val_bpb={:.8} evaluated_tokens={} evaluated_bytes={} elapsed_ms={}",
        stage_label,
        eval_mode.as_str(),
        mean_loss,
        bits_per_byte,
        total_token_count,
        total_byte_count,
        duration_ms(validation_started),
    ));
    Ok(ParameterGolfSingleH100ValidationSummary {
        eval_mode: eval_mode.clone(),
        evaluated_sequence_count: total_units,
        evaluated_token_count: total_token_count,
        evaluated_byte_count: total_byte_count,
        mean_loss,
        bits_per_byte,
        runtime_receipt: Some(runtime_receipt),
        score_first_ttt_receipt: None,
    })
}

fn evaluate_validation_on_cuda_legacy(
    cuda_backend: &mut CudaBackend,
    device: &psionic_core::Device,
    _descriptor: &psionic_models::ParameterGolfModelDescriptor,
    model: &ParameterGolfReferenceModel,
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    stage_label: &str,
    live_visualization_writer: Option<&mut crate::ParameterGolfSingleH100LiveVisualizationWriter>,
) -> Result<ParameterGolfSingleH100ValidationSummary, ParameterGolfSingleH100TrainingError> {
    if validation_tokens.len() <= sequence_length {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: String::from(
                "validation split is too short for the requested sequence length",
            ),
        });
    }
    let total_sequences = (validation_tokens.len() - 1) / sequence_length;
    let mut total_loss_sum = 0.0_f64;
    let mut total_token_count = 0_u64;
    let mut total_byte_count = 0_u64;
    let validation_batch_sequences = batch_sequences.max(1);
    let total_batches = total_sequences.div_ceil(validation_batch_sequences);
    let validation_started = Instant::now();
    let mut live_visualization_writer = live_visualization_writer;

    for (batch_index, batch_start) in (0..total_sequences)
        .step_by(validation_batch_sequences)
        .enumerate()
    {
        let batch_end = (batch_start + validation_batch_sequences).min(total_sequences);
        let raw_start = batch_start * sequence_length;
        let raw_end = batch_end * sequence_length + 1;
        emit_progress_line(format!(
            "validation_batch_start stage={} batch={}/{} batch_sequences={} evaluated_tokens={} elapsed_ms={}",
            stage_label,
            batch_index + 1,
            total_batches,
            batch_end.saturating_sub(batch_start),
            total_token_count,
            duration_ms(validation_started),
        ));
        let local = &validation_tokens[raw_start..raw_end];
        let input_ids = local[..local.len() - 1]
            .chunks(sequence_length)
            .map(|row| {
                row.iter()
                    .map(|token| u32::from(*token))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let target_ids = local[1..]
            .chunks(sequence_length)
            .map(|row| {
                row.iter()
                    .map(|token| u32::from(*token))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let graph = training_graph_for_batch(
            graph_cache,
            device.clone(),
            model,
            input_ids.len(),
            sequence_length,
            ParameterGolfMatrixExecutionMode::DirectBanked,
        )?;
        let inputs = bind_parameter_golf_baseline_training_graph_inputs(
            graph,
            model,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        let outputs = execute_cuda_graph_outputs(cuda_backend, graph.graph.graph(), &inputs)?;
        let batch_loss = scalar_float_graph_output(&outputs, graph.loss_tensor_id)?;
        let batch_token_count = target_ids.iter().map(Vec::len).sum::<usize>() as u64;
        total_loss_sum += f64::from(batch_loss) * batch_token_count as f64;
        total_token_count = total_token_count.saturating_add(batch_token_count);
        total_byte_count = total_byte_count.saturating_add(
            byte_luts.count_target_bytes(
                input_ids
                    .iter()
                    .flat_map(|row| row.iter().copied())
                    .collect::<Vec<_>>()
                    .as_slice(),
                target_ids
                    .iter()
                    .flat_map(|row| row.iter().copied())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?,
        );
        if batch_index == 0 || (batch_index + 1) % 32 == 0 || batch_index + 1 == total_batches {
            emit_progress_line(format!(
                "validation_progress stage={} batch={}/{} sequences={} tokens={} elapsed_ms={}",
                stage_label,
                batch_index + 1,
                total_batches,
                batch_end,
                total_token_count,
                duration_ms(validation_started),
            ));
        }
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_phase(
                "training",
                Some(String::from("validation")),
                format!(
                    "Validation stage `{stage_label}` processed batch {} of {}.",
                    batch_index + 1,
                    total_batches
                ),
                vec![String::from("validation"), String::from("gpu_sampling")],
                None,
                Some(0),
                false,
            )?;
        }
    }

    let mean_loss = total_loss_sum / total_token_count.max(1) as f64;
    let bits_per_byte = (mean_loss / std::f64::consts::LN_2)
        * (total_token_count as f64 / total_byte_count.max(1) as f64);
    emit_progress_line(format!(
        "validation_complete stage={} eval_mode={} mean_loss={:.8} val_bpb={:.8} evaluated_tokens={} evaluated_bytes={} elapsed_ms={}",
        stage_label,
        ParameterGolfValidationEvalMode::NonOverlapping.as_str(),
        mean_loss,
        bits_per_byte,
        total_token_count,
        total_byte_count,
        duration_ms(validation_started),
    ));
    Ok(ParameterGolfSingleH100ValidationSummary {
        eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
        evaluated_sequence_count: total_sequences,
        evaluated_token_count: total_token_count,
        evaluated_byte_count: total_byte_count,
        mean_loss,
        bits_per_byte,
        runtime_receipt: None,
        score_first_ttt_receipt: None,
    })
}

fn build_validation_batch_plans(
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
) -> Result<Vec<ParameterGolfValidationBatchPlan>, ParameterGolfSingleH100TrainingError> {
    match eval_mode {
        ParameterGolfValidationEvalMode::NonOverlapping => {
            build_non_overlapping_validation_batch_plans(
                validation_tokens,
                byte_luts,
                sequence_length,
                batch_sequences,
            )
        }
        ParameterGolfValidationEvalMode::SlidingWindow { stride } => {
            build_sliding_window_validation_batch_plans(
                validation_tokens,
                byte_luts,
                sequence_length,
                batch_sequences,
                *stride,
            )
        }
    }
}

fn build_non_overlapping_validation_batch_plans(
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
) -> Result<Vec<ParameterGolfValidationBatchPlan>, ParameterGolfSingleH100TrainingError> {
    let total_sequences = (validation_tokens.len() - 1) / sequence_length;
    let mut plans = Vec::new();
    for batch_start in (0..total_sequences).step_by(batch_sequences.max(1)) {
        let batch_end = (batch_start + batch_sequences.max(1)).min(total_sequences);
        let batch_sequence_count = batch_end - batch_start;
        let mut input_tokens = vec![0_i32; batch_sequence_count * sequence_length];
        let mut target_tokens = vec![0_i32; batch_sequence_count * sequence_length];
        let mut previous_tokens = Vec::with_capacity(batch_sequence_count * sequence_length);
        let mut scored_targets = Vec::with_capacity(batch_sequence_count * sequence_length);
        let mut sequence_plans = Vec::with_capacity(batch_sequence_count);
        for (local_index, sequence_index) in (batch_start..batch_end).enumerate() {
            let raw_start = sequence_index * sequence_length;
            let local = &validation_tokens[raw_start..raw_start + sequence_length + 1];
            let row_offset = local_index * sequence_length;
            for position in 0..sequence_length {
                let previous = local[position];
                let target = local[position + 1];
                input_tokens[row_offset + position] = i32::from(previous);
                target_tokens[row_offset + position] = i32::from(target);
                previous_tokens.push(u32::from(previous));
                scored_targets.push(u32::from(target));
            }
            sequence_plans.push(ParameterGolfValidationSequencePlan {
                valid_length: sequence_length,
                score_start: 0,
            });
        }
        plans.push(ParameterGolfValidationBatchPlan {
            batch_sequences: batch_sequence_count,
            evaluation_units: batch_sequence_count,
            input_tokens,
            target_tokens,
            sequence_plans,
            token_count: (batch_sequence_count * sequence_length) as u64,
            byte_count: byte_luts
                .count_target_bytes(previous_tokens.as_slice(), scored_targets.as_slice())?,
        });
    }
    Ok(plans)
}

fn build_sliding_window_validation_batch_plans(
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
) -> Result<Vec<ParameterGolfValidationBatchPlan>, ParameterGolfSingleH100TrainingError> {
    let window_starts = build_parameter_golf_validation_window_starts(
        validation_tokens.len().saturating_sub(1),
        sequence_length,
        stride,
    );
    build_sliding_window_validation_batch_plans_from_window_starts(
        validation_tokens,
        byte_luts,
        sequence_length,
        batch_sequences,
        stride,
        window_starts.as_slice(),
    )
}

pub(crate) fn build_parameter_golf_validation_window_starts(
    total_tokens: usize,
    sequence_length: usize,
    stride: usize,
) -> Vec<usize> {
    (0..total_tokens)
        .step_by(stride.max(1))
        .filter(|window_start| {
            total_tokens
                .saturating_sub(*window_start)
                .min(sequence_length)
                >= 1
        })
        .collect()
}

fn build_sliding_window_validation_batch_plans_from_window_starts(
    validation_tokens: &[u16],
    byte_luts: &ParameterGolfSentencePieceByteLuts,
    sequence_length: usize,
    batch_sequences: usize,
    stride: usize,
    window_starts: &[usize],
) -> Result<Vec<ParameterGolfValidationBatchPlan>, ParameterGolfSingleH100TrainingError> {
    let total_tokens = validation_tokens.len().saturating_sub(1);
    let mut plans = Vec::new();
    for batch_start in (0..window_starts.len()).step_by(batch_sequences.max(1)) {
        let batch_end = (batch_start + batch_sequences.max(1)).min(window_starts.len());
        let batch_sequence_count = batch_end - batch_start;
        let mut input_tokens = vec![0_i32; batch_sequence_count * sequence_length];
        let mut target_tokens = vec![0_i32; batch_sequence_count * sequence_length];
        let mut previous_tokens = Vec::new();
        let mut scored_targets = Vec::new();
        let mut sequence_plans = Vec::with_capacity(batch_sequence_count);
        let mut token_count = 0_u64;
        for (local_index, window_start) in window_starts[batch_start..batch_end].iter().enumerate()
        {
            let window_end = (*window_start + sequence_length).min(total_tokens);
            let valid_length = window_end.saturating_sub(*window_start);
            let chunk = &validation_tokens[*window_start..window_end + 1];
            let row_offset = local_index * sequence_length;
            for position in 0..valid_length {
                input_tokens[row_offset + position] = i32::from(chunk[position]);
                target_tokens[row_offset + position] = i32::from(chunk[position + 1]);
            }
            let score_start = if *window_start == 0 {
                0
            } else {
                valid_length.saturating_sub(stride)
            };
            for position in score_start..valid_length {
                previous_tokens.push(u32::from(chunk[position]));
                scored_targets.push(u32::from(chunk[position + 1]));
            }
            token_count =
                token_count.saturating_add(valid_length.saturating_sub(score_start) as u64);
            sequence_plans.push(ParameterGolfValidationSequencePlan {
                valid_length,
                score_start,
            });
        }
        plans.push(ParameterGolfValidationBatchPlan {
            batch_sequences: batch_sequence_count,
            evaluation_units: batch_sequence_count,
            input_tokens,
            target_tokens,
            sequence_plans,
            token_count,
            byte_count: byte_luts
                .count_target_bytes(previous_tokens.as_slice(), scored_targets.as_slice())?,
        });
    }
    Ok(plans)
}

fn validation_unit_count(
    validation_tokens: &[u16],
    sequence_length: usize,
    eval_mode: &ParameterGolfValidationEvalMode,
) -> usize {
    match eval_mode {
        ParameterGolfValidationEvalMode::NonOverlapping => {
            (validation_tokens.len().saturating_sub(1)) / sequence_length
        }
        ParameterGolfValidationEvalMode::SlidingWindow { stride } => {
            (0..validation_tokens.len().saturating_sub(1))
                .step_by((*stride).max(1))
                .filter(|window_start| {
                    validation_tokens
                        .len()
                        .saturating_sub(1)
                        .saturating_sub(*window_start)
                        .min(sequence_length)
                        >= 1
                })
                .count()
        }
    }
}

fn scored_token_loss_sum(
    token_losses: &[f32],
    sequence_plans: &[ParameterGolfValidationSequencePlan],
    sequence_length: usize,
) -> f64 {
    sequence_plans
        .iter()
        .enumerate()
        .map(|(sequence_index, plan)| {
            let row_offset = sequence_index * sequence_length;
            token_losses[row_offset + plan.score_start..row_offset + plan.valid_length]
                .iter()
                .map(|value| f64::from(*value))
                .sum::<f64>()
        })
        .sum()
}

fn validation_session_for_batch<'a>(
    cache: &'a mut BTreeMap<usize, ParameterGolfCudaValidationSession>,
    cuda_backend: &mut CudaBackend,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    device: psionic_core::Device,
    model: &ParameterGolfReferenceModel,
    resident_training_sessions: Option<&BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
    batch_sequences: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<&'a mut ParameterGolfCudaValidationSession, ParameterGolfSingleH100TrainingError> {
    if !cache.contains_key(&batch_sequences) {
        let graph = eval_graph_for_batch(
            graph_cache,
            device,
            model,
            batch_sequences,
            sequence_length,
            matrix_execution_mode,
        )?
        .clone();
        let session = match resident_validation_parameter_source_session(resident_training_sessions)
        {
            Some(training_session) => {
                ParameterGolfCudaValidationSession::new_from_resident_training_session(
                    cuda_backend,
                    graph,
                    training_session,
                    batch_sequences,
                    sequence_length,
                )?
            }
            None => ParameterGolfCudaValidationSession::new(
                cuda_backend,
                graph,
                model,
                batch_sequences,
                sequence_length,
            )?,
        };
        cache.insert(batch_sequences, session);
    }
    cache.get_mut(&batch_sequences).ok_or_else(|| {
        ParameterGolfSingleH100TrainingError::Serialization {
            message: format!(
                "missing cached validation session for batch_sequences={batch_sequences}"
            ),
        }
    })
}

fn optional_bigram_token_input(
    model_config: &ParameterGolfConfig,
    bigram_token_ids_tensor_id: Option<TensorId>,
    bigram_token_buffer: Option<&mut CudaBuffer>,
    bigram_token_staging: Option<&mut Vec<i32>>,
    input_tokens: &[i32],
    batch_sequences: usize,
    sequence_length: usize,
) -> Result<Option<(TensorId, CudaBuffer)>, ParameterGolfSingleH100TrainingError> {
    let Some(bigram_token_ids_tensor_id) = bigram_token_ids_tensor_id else {
        return Ok(None);
    };
    let Some(bigram_token_buffer) = bigram_token_buffer else {
        return Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: String::from(
                "bigram graph input was requested without one allocated bigram token buffer",
            ),
        });
    };
    let Some(bigram_token_staging) = bigram_token_staging else {
        return Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: String::from(
                "bigram graph input was requested without one allocated bigram token staging buffer",
            ),
        });
    };
    if input_tokens.len() != batch_sequences.saturating_mul(sequence_length) {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: format!(
                "bigram token materialization expected {} flattened input tokens but observed {}",
                batch_sequences.saturating_mul(sequence_length),
                input_tokens.len(),
            ),
        });
    }
    if bigram_token_staging.len() != input_tokens.len() {
        return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
            message: format!(
                "bigram token staging expected {} values but observed {}",
                input_tokens.len(),
                bigram_token_staging.len(),
            ),
        });
    }
    let mut input_rows = Vec::with_capacity(batch_sequences);
    for row in input_tokens.chunks(sequence_length) {
        input_rows.push(
            row.iter()
                .map(|token_id| {
                    u32::try_from(*token_id).map_err(|_| {
                        ParameterGolfSingleH100TrainingError::Serialization {
                            message: format!(
                                "bigram token materialization observed one negative token id {}",
                                token_id
                            ),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let bigram_rows = model_config
        .bigram_hash_batch(input_rows.as_slice())?
        .ok_or_else(|| ParameterGolfSingleH100TrainingError::Serialization {
            message: String::from(
                "bigram graph input was requested but the model config has bigram hashing disabled",
            ),
        })?;
    for (destination, token_id) in bigram_token_staging.iter_mut().zip(
        bigram_rows
            .iter()
            .flat_map(|row| row.iter().map(|token_id| *token_id as i32)),
    ) {
        *destination = token_id;
    }
    bigram_token_buffer.write_i32(bigram_token_staging.as_slice())?;
    Ok(Some((
        bigram_token_ids_tensor_id,
        bigram_token_buffer.clone(),
    )))
}

fn resident_validation_parameter_source_session(
    resident_training_sessions: Option<&BTreeMap<usize, ParameterGolfCudaTrainingSession>>,
) -> Option<&ParameterGolfCudaTrainingSession> {
    resident_training_sessions.and_then(|sessions| sessions.values().next())
}

fn training_session_for_batch<'a>(
    cache: &'a mut BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    cuda_backend: &mut CudaBackend,
    graph_cache: &mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    device: psionic_core::Device,
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    batch_sequences: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<&'a mut ParameterGolfCudaTrainingSession, ParameterGolfSingleH100TrainingError> {
    if !cache.contains_key(&batch_sequences) {
        let graph = training_graph_for_batch(
            graph_cache,
            device,
            model,
            batch_sequences,
            sequence_length,
            matrix_execution_mode,
        )?
        .clone();
        let session = ParameterGolfCudaTrainingSession::new(
            cuda_backend,
            graph,
            model,
            explicit_banked_weights,
            batch_sequences,
            sequence_length,
        )?;
        cache.insert(batch_sequences, session);
    }
    cache.get_mut(&batch_sequences).ok_or_else(|| {
        ParameterGolfSingleH100TrainingError::Serialization {
            message: format!(
                "missing cached training session for batch_sequences={batch_sequences}"
            ),
        }
    })
}

pub(crate) fn refresh_parameter_golf_cuda_training_sessions(
    cache: &mut BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    model: &ParameterGolfReferenceModel,
    explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    for session in cache.values_mut() {
        session.refresh_parameters(model, explicit_banked_weights)?;
    }
    Ok(())
}

pub(crate) fn refresh_parameter_golf_cuda_training_sessions_from_state(
    cache: &mut BTreeMap<usize, ParameterGolfCudaTrainingSession>,
    state: &ParameterGolfSingleH100TrainerState,
) -> Result<(), ParameterGolfSingleH100TrainingError> {
    for session in cache.values_mut() {
        session.refresh_parameters_from_state(state)?;
    }
    Ok(())
}

fn training_graph_for_batch<'a>(
    cache: &'a mut BTreeMap<usize, ParameterGolfBaselineTrainingGraph>,
    device: psionic_core::Device,
    model: &ParameterGolfReferenceModel,
    batch_size: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<&'a ParameterGolfBaselineTrainingGraph, ParameterGolfSingleH100TrainingError> {
    if !cache.contains_key(&batch_size) {
        let descriptor = model.banked_descriptor()?;
        let graph = build_parameter_golf_baseline_training_graph_with_matrix_execution_mode(
            device,
            &descriptor,
            batch_size,
            sequence_length,
            matrix_execution_mode,
        )?;
        cache.insert(batch_size, graph);
    }
    cache
        .get(&batch_size)
        .ok_or_else(|| ParameterGolfSingleH100TrainingError::Serialization {
            message: format!("missing cached training graph for batch_size={batch_size}"),
        })
}

fn eval_graph_for_batch<'a>(
    cache: &'a mut BTreeMap<usize, ParameterGolfBaselineEvalGraph>,
    device: psionic_core::Device,
    model: &ParameterGolfReferenceModel,
    batch_size: usize,
    sequence_length: usize,
    matrix_execution_mode: ParameterGolfMatrixExecutionMode,
) -> Result<&'a ParameterGolfBaselineEvalGraph, ParameterGolfSingleH100TrainingError> {
    if !cache.contains_key(&batch_size) {
        let descriptor = model.banked_descriptor()?;
        let graph = build_parameter_golf_baseline_eval_graph_with_matrix_execution_mode(
            device,
            &descriptor,
            batch_size,
            sequence_length,
            matrix_execution_mode,
        )?;
        cache.insert(batch_size, graph);
    }
    cache
        .get(&batch_size)
        .ok_or_else(|| ParameterGolfSingleH100TrainingError::Serialization {
            message: format!("missing cached eval graph for batch_size={batch_size}"),
        })
}

impl ParameterGolfCudaValidationSession {
    fn new(
        cuda_backend: &mut CudaBackend,
        graph: ParameterGolfBaselineEvalGraph,
        model: &ParameterGolfReferenceModel,
        batch_sequences: usize,
        sequence_length: usize,
    ) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        let has_bigram_input = graph.bigram_token_ids_tensor_id.is_some();
        let parameter_vectors = model
            .all_parameter_vectors()?
            .into_iter()
            .map(|parameter| (parameter.parameter_id.clone(), parameter))
            .collect::<BTreeMap<_, _>>();
        let token_element_count = batch_sequences.saturating_mul(sequence_length);
        let token_shape = Shape::new(vec![batch_sequences, sequence_length]);
        let parameter_upload_started = Instant::now();
        let mut static_inputs = BTreeMap::new();
        let mut persistent_parameter_buffer_count = 0_usize;
        let mut persistent_parameter_value_count = 0_u64;
        for binding in &graph.parameter_bindings {
            let parameter = parameter_vectors
                .get(&binding.parameter_id)
                .ok_or_else(
                    || crate::ParameterGolfBaselineGraphError::MissingWeightVector {
                        parameter_id: binding.parameter_id.clone(),
                    },
                )?;
            let buffer = match binding.graph_input_dtype {
                DType::F32 => {
                    cuda_backend.input_buffer(binding.shape.clone(), parameter.values.clone())?
                }
                DType::BF16 => cuda_backend
                    .input_bf16_buffer(binding.shape.clone(), parameter.values.clone())?,
                actual => {
                    return Err(ParameterGolfSingleH100TrainingError::Serialization {
                        message: format!(
                            "validation session does not support graph input dtype {actual:?} for `{}`",
                            binding.parameter_id
                        ),
                    });
                }
            };
            persistent_parameter_buffer_count = persistent_parameter_buffer_count.saturating_add(1);
            persistent_parameter_value_count = persistent_parameter_value_count
                .saturating_add(buffer.spec().shape().element_count() as u64);
            static_inputs.insert(binding.graph_input_tensor_id, buffer);
        }
        let resident_parameter_upload_us = duration_us(parameter_upload_started);
        let input_token_buffer =
            cuda_backend.input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?;
        let bigram_token_buffer = if has_bigram_input {
            Some(
                cuda_backend
                    .input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?,
            )
        } else {
            None
        };
        let target_token_buffer =
            cuda_backend.input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?;
        Ok(Self {
            graph,
            model_config: model.descriptor().config.clone(),
            static_inputs,
            input_token_buffer,
            bigram_token_buffer,
            target_token_buffer,
            input_token_staging: vec![0_i32; token_element_count],
            bigram_token_staging: has_bigram_input.then(|| vec![0_i32; token_element_count]),
            target_token_staging: vec![0_i32; token_element_count],
            resident_parameter_upload_us,
            persistent_parameter_buffer_count,
            persistent_parameter_value_count,
        })
    }

    fn new_from_resident_training_session(
        cuda_backend: &mut CudaBackend,
        graph: ParameterGolfBaselineEvalGraph,
        training_session: &ParameterGolfCudaTrainingSession,
        batch_sequences: usize,
        sequence_length: usize,
    ) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        if graph.bigram_token_ids_tensor_id.is_some()
            != training_session.graph.bigram_token_ids_tensor_id.is_some()
        {
            return Err(ParameterGolfSingleH100TrainingError::Serialization {
                message: String::from(
                    "resident validation session expected training and eval graphs to agree on the optional bigram input posture",
                ),
            });
        }
        let has_bigram_input = graph.bigram_token_ids_tensor_id.is_some();
        let token_element_count = batch_sequences.saturating_mul(sequence_length);
        let token_shape = Shape::new(vec![batch_sequences, sequence_length]);
        let mut static_inputs = BTreeMap::new();
        let mut persistent_parameter_buffer_count = 0_usize;
        let mut persistent_parameter_value_count = 0_u64;
        for binding in &graph.parameter_bindings {
            let training_binding = training_session
                .graph
                .parameter_binding(&binding.parameter_id)
                .ok_or_else(|| ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "resident validation session could not find training binding for `{}`",
                        binding.parameter_id
                    ),
                })?;
            if training_binding.shape != binding.shape
                || training_binding.graph_input_dtype != binding.graph_input_dtype
            {
                return Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "resident validation session expected `{}` to match training binding shape {:?} dtype {:?} but observed shape {:?} dtype {:?}",
                        binding.parameter_id,
                        binding.shape,
                        binding.graph_input_dtype,
                        training_binding.shape,
                        training_binding.graph_input_dtype,
                    ),
                });
            }
            let buffer = training_session
                .static_inputs
                .get(&training_binding.graph_input_tensor_id)
                .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: training_binding.graph_input_tensor_id,
                })?
                .clone();
            persistent_parameter_buffer_count = persistent_parameter_buffer_count.saturating_add(1);
            persistent_parameter_value_count = persistent_parameter_value_count
                .saturating_add(buffer.spec().shape().element_count() as u64);
            static_inputs.insert(binding.graph_input_tensor_id, buffer);
        }
        let input_token_buffer =
            cuda_backend.input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?;
        let bigram_token_buffer = if has_bigram_input {
            Some(
                cuda_backend
                    .input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?,
            )
        } else {
            None
        };
        let target_token_buffer =
            cuda_backend.input_i32_buffer(token_shape, vec![0_i32; token_element_count])?;
        Ok(Self {
            graph,
            model_config: training_session.model_config.clone(),
            static_inputs,
            input_token_buffer,
            bigram_token_buffer,
            target_token_buffer,
            input_token_staging: vec![0_i32; token_element_count],
            bigram_token_staging: has_bigram_input.then(|| vec![0_i32; token_element_count]),
            target_token_staging: vec![0_i32; token_element_count],
            resident_parameter_upload_us: 0,
            persistent_parameter_buffer_count,
            persistent_parameter_value_count,
        })
    }

    fn execute_batch(
        &mut self,
        cuda_backend: &mut CudaBackend,
        batch_plan: &ParameterGolfValidationBatchPlan,
    ) -> Result<(Vec<f32>, ParameterGolfValidationBatchRuntime), ParameterGolfSingleH100TrainingError>
    {
        if batch_plan
            .batch_sequences
            .saturating_mul(self.input_token_staging.len().max(1))
            < batch_plan.input_tokens.len()
        {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "validation session staging capacity {} could not fit {} input tokens",
                    self.input_token_staging.len(),
                    batch_plan.input_tokens.len()
                ),
            });
        }
        if batch_plan.input_tokens.len() != self.input_token_staging.len()
            || batch_plan.target_tokens.len() != self.target_token_staging.len()
        {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "validation session expected {} tokens per batch but observed {} input and {} target tokens",
                    self.input_token_staging.len(),
                    batch_plan.input_tokens.len(),
                    batch_plan.target_tokens.len(),
                ),
            });
        }

        let input_write_started = Instant::now();
        self.input_token_staging
            .copy_from_slice(batch_plan.input_tokens.as_slice());
        self.input_token_buffer
            .write_i32(self.input_token_staging.as_slice())?;
        let input_token_write_us = duration_us(input_write_started);

        let target_write_started = Instant::now();
        self.target_token_staging
            .copy_from_slice(batch_plan.target_tokens.as_slice());
        self.target_token_buffer
            .write_i32(self.target_token_staging.as_slice())?;
        let target_token_write_us = duration_us(target_write_started);

        let mut inputs = self.static_inputs.clone();
        inputs.insert(
            self.graph.input_token_ids_tensor_id,
            self.input_token_buffer.clone(),
        );
        if let Some((bigram_tensor_id, bigram_buffer)) = optional_bigram_token_input(
            &self.model_config,
            self.graph.bigram_token_ids_tensor_id,
            self.bigram_token_buffer.as_mut(),
            self.bigram_token_staging.as_mut(),
            batch_plan.input_tokens.as_slice(),
            batch_plan.batch_sequences,
            self.input_token_staging.len() / batch_plan.batch_sequences.max(1),
        )? {
            inputs.insert(bigram_tensor_id, bigram_buffer);
        }
        inputs.insert(
            self.graph.target_ids_tensor_id,
            self.target_token_buffer.clone(),
        );
        let outputs =
            execute_cuda_graph_outputs_from_buffers(cuda_backend, &self.graph.graph, &inputs)?;
        let batch_token_losses =
            dense_f32_graph_output(&outputs, self.graph.token_losses_tensor_id)?;
        Ok((
            batch_token_losses,
            ParameterGolfValidationBatchRuntime {
                input_token_write_us,
                target_token_write_us,
            },
        ))
    }
}

impl ParameterGolfCudaTrainingSession {
    fn new(
        cuda_backend: &mut CudaBackend,
        graph: ParameterGolfBaselineTrainingGraph,
        model: &ParameterGolfReferenceModel,
        explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
        batch_sequences: usize,
        sequence_length: usize,
    ) -> Result<Self, ParameterGolfSingleH100TrainingError> {
        let has_bigram_input = graph.bigram_token_ids_tensor_id.is_some();
        let parameter_vectors = parameter_golf_parameter_values_for_bindings(
            graph.parameter_bindings.as_slice(),
            model,
            explicit_banked_weights,
        )?;
        let token_element_count = batch_sequences.saturating_mul(sequence_length);
        let token_shape = Shape::new(vec![batch_sequences, sequence_length]);
        let parameter_upload_started = Instant::now();
        let mut static_inputs = BTreeMap::new();
        let mut persistent_parameter_buffer_count = 0_usize;
        let mut persistent_parameter_value_count = 0_u64;
        for binding in &graph.parameter_bindings {
            let parameter = parameter_vectors
                .get(&binding.parameter_id)
                .ok_or_else(
                    || crate::ParameterGolfBaselineGraphError::MissingWeightVector {
                        parameter_id: binding.parameter_id.clone(),
                    },
                )?;
            let buffer = match binding.graph_input_dtype {
                DType::F32 => {
                    cuda_backend.input_buffer(binding.shape.clone(), parameter.clone())?
                }
                DType::BF16 => {
                    cuda_backend.input_bf16_buffer(binding.shape.clone(), parameter.clone())?
                }
                actual => {
                    return Err(ParameterGolfSingleH100TrainingError::Serialization {
                        message: format!(
                            "training session does not support graph input dtype {actual:?} for `{}`",
                            binding.parameter_id
                        ),
                    });
                }
            };
            persistent_parameter_buffer_count = persistent_parameter_buffer_count.saturating_add(1);
            persistent_parameter_value_count = persistent_parameter_value_count
                .saturating_add(buffer.spec().shape().element_count() as u64);
            static_inputs.insert(binding.graph_input_tensor_id, buffer);
        }
        let resident_parameter_upload_us = duration_us(parameter_upload_started);
        let input_token_buffer =
            cuda_backend.input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?;
        let target_token_buffer =
            cuda_backend.input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?;
        let backward_plan = parameter_only_backward_plan(&graph)?;
        let retained_graph = retained_forward_graph(&graph, &backward_plan);
        Ok(Self {
            graph,
            model_config: model.descriptor().config.clone(),
            backward_plan,
            retained_graph,
            static_inputs,
            input_token_buffer,
            bigram_token_buffer: if has_bigram_input {
                Some(
                    cuda_backend
                        .input_i32_buffer(token_shape.clone(), vec![0_i32; token_element_count])?,
                )
            } else {
                None
            },
            target_token_buffer,
            input_token_staging: vec![0_i32; token_element_count],
            bigram_token_staging: has_bigram_input.then(|| vec![0_i32; token_element_count]),
            target_token_staging: vec![0_i32; token_element_count],
            resident_parameter_upload_us_pending: Some(resident_parameter_upload_us),
            parameter_refresh_us_pending: None,
            persistent_parameter_buffer_count,
            persistent_parameter_value_count,
        })
    }

    fn refresh_parameters(
        &mut self,
        model: &ParameterGolfReferenceModel,
        explicit_banked_weights: Option<&ParameterGolfBankedWeights>,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        let parameter_vectors = parameter_golf_parameter_values_for_bindings(
            self.graph.parameter_bindings.as_slice(),
            model,
            explicit_banked_weights,
        )?;
        let refresh_started = Instant::now();
        for binding in &self.graph.parameter_bindings {
            let parameter = parameter_vectors
                .get(&binding.parameter_id)
                .ok_or_else(
                    || crate::ParameterGolfBaselineGraphError::MissingWeightVector {
                        parameter_id: binding.parameter_id.clone(),
                    },
                )?;
            let buffer = self
                .static_inputs
                .get_mut(&binding.graph_input_tensor_id)
                .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: binding.graph_input_tensor_id,
                })?;
            match binding.graph_input_dtype {
                DType::F32 => buffer.write_f32(parameter.as_slice())?,
                DType::BF16 => buffer.write_bf16_from_f32(parameter.as_slice())?,
                actual => {
                    return Err(ParameterGolfSingleH100TrainingError::Serialization {
                        message: format!(
                            "training session does not support refresh dtype {actual:?} for `{}`",
                            binding.parameter_id
                        ),
                    });
                }
            }
        }
        self.parameter_refresh_us_pending = Some(duration_us(refresh_started));
        Ok(())
    }

    fn refresh_parameters_from_state(
        &mut self,
        state: &ParameterGolfSingleH100TrainerState,
    ) -> Result<(), ParameterGolfSingleH100TrainingError> {
        let refresh_started = Instant::now();
        for binding in &self.graph.parameter_bindings {
            let parameter_state = state
                .parameter_states
                .get(&binding.parameter_id)
                .ok_or_else(
                    || ParameterGolfSingleH100TrainingError::MissingParameterState {
                        parameter_id: binding.parameter_id.clone(),
                    },
                )?;
            let buffer = self
                .static_inputs
                .get_mut(&binding.graph_input_tensor_id)
                .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: binding.graph_input_tensor_id,
                })?;
            match (binding.graph_input_dtype, parameter_state) {
                (DType::F32, _) => buffer.write_f32(parameter_state.values())?,
                (
                    DType::BF16,
                    ParameterGolfParameterState::AdamBf16Master {
                        train_visible_bf16_bits,
                        ..
                    },
                ) => buffer.write_bf16_bits(train_visible_bf16_bits.as_slice())?,
                (DType::BF16, ParameterGolfParameterState::MuonBf16 { bf16_bits, .. }) => {
                    buffer.write_bf16_bits(bf16_bits.as_slice())?
                }
                (DType::BF16, _) => buffer.write_bf16_from_f32(parameter_state.values())?,
                (actual, _) => {
                    return Err(ParameterGolfSingleH100TrainingError::Serialization {
                        message: format!(
                            "training session does not support state refresh dtype {actual:?} for `{}`",
                            binding.parameter_id
                        ),
                    });
                }
            }
        }
        self.parameter_refresh_us_pending = Some(duration_us(refresh_started));
        Ok(())
    }

    fn execute_batch(
        &mut self,
        cuda_backend: &mut CudaBackend,
        input_ids: &[Vec<u32>],
        target_ids: &[Vec<u32>],
    ) -> Result<
        (
            f32,
            Vec<(TensorId, TensorData)>,
            ParameterGolfTrainingBatchRuntime,
        ),
        ParameterGolfSingleH100TrainingError,
    > {
        let batch_sequences = input_ids.len();
        if batch_sequences == 0 || batch_sequences != target_ids.len() {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "training session expected matching non-zero batch sizes but observed {} input rows and {} target rows",
                    batch_sequences,
                    target_ids.len()
                ),
            });
        }
        let sequence_length = self.input_token_staging.len() / batch_sequences.max(1);
        let expected_token_count = batch_sequences.saturating_mul(sequence_length);
        let flattened_input_ids = input_ids
            .iter()
            .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
            .collect::<Vec<_>>();
        let flattened_target_ids = target_ids
            .iter()
            .flat_map(|row| row.iter().map(|token_id| *token_id as i32))
            .collect::<Vec<_>>();
        if flattened_input_ids.len() != expected_token_count
            || flattened_target_ids.len() != expected_token_count
        {
            return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                message: format!(
                    "training session expected {} tokens per batch but observed {} input and {} target tokens",
                    expected_token_count,
                    flattened_input_ids.len(),
                    flattened_target_ids.len(),
                ),
            });
        }

        let input_write_started = Instant::now();
        self.input_token_staging
            .copy_from_slice(flattened_input_ids.as_slice());
        self.input_token_buffer
            .write_i32(self.input_token_staging.as_slice())?;
        let input_token_write_us = duration_us(input_write_started);

        let target_write_started = Instant::now();
        self.target_token_staging
            .copy_from_slice(flattened_target_ids.as_slice());
        self.target_token_buffer
            .write_i32(self.target_token_staging.as_slice())?;
        let target_token_write_us = duration_us(target_write_started);

        let mut inputs = self.static_inputs.clone();
        inputs.insert(
            self.graph.input_token_ids_tensor_id,
            self.input_token_buffer.clone(),
        );
        if let Some((bigram_tensor_id, bigram_buffer)) = optional_bigram_token_input(
            &self.model_config,
            self.graph.bigram_token_ids_tensor_id,
            self.bigram_token_buffer.as_mut(),
            self.bigram_token_staging.as_mut(),
            flattened_input_ids.as_slice(),
            batch_sequences,
            sequence_length,
        )? {
            inputs.insert(bigram_tensor_id, bigram_buffer);
        }
        inputs.insert(
            self.graph.target_ids_tensor_id,
            self.target_token_buffer.clone(),
        );
        let forward_started = Instant::now();
        let forward_outputs = execute_cuda_graph_output_buffers_from_buffers(
            cuda_backend,
            &self.retained_graph,
            &inputs,
        )?;
        let forward_loss_cuda_ms = duration_ms(forward_started);
        let loss = scalar_float_cuda_buffer_output(&forward_outputs, self.graph.loss_tensor_id)?;
        let backward_started = Instant::now();
        let backward_outputs = execute_backward_plan(
            cuda_backend,
            self.graph.graph.graph(),
            &self.backward_plan,
            &inputs,
            &forward_outputs,
        )?;
        let backward_cuda_ms = duration_ms(backward_started);
        Ok((
            loss,
            backward_outputs,
            ParameterGolfTrainingBatchRuntime {
                resident_parameter_upload_us: self
                    .resident_parameter_upload_us_pending
                    .take()
                    .unwrap_or(0),
                parameter_refresh_us: self.parameter_refresh_us_pending.take().unwrap_or(0),
                input_token_write_us,
                target_token_write_us,
                forward_loss_cuda_ms,
                backward_cuda_ms,
                persistent_parameter_buffer_count: self.persistent_parameter_buffer_count,
                persistent_parameter_value_count: self.persistent_parameter_value_count,
                resident_parameter_buffers_reused: true,
            },
        ))
    }
}

fn retained_forward_graph(
    graph: &ParameterGolfBaselineTrainingGraph,
    backward_plan: &psionic_ir::AutodiffBackwardPlan,
) -> psionic_ir::Graph {
    let mut outputs = vec![graph.loss_tensor_id];
    outputs.extend(retained_forward_tensor_ids(graph, backward_plan));
    graph.graph.graph().with_outputs(unique_tensor_ids(outputs))
}

fn retained_forward_tensor_ids(
    graph: &ParameterGolfBaselineTrainingGraph,
    backward_plan: &psionic_ir::AutodiffBackwardPlan,
) -> Vec<TensorId> {
    let mut retained = std::collections::BTreeSet::new();
    let mut visited = std::collections::BTreeSet::new();
    for binding in &backward_plan.primal_bindings {
        collect_retained_forward_tensor_ids(
            graph.graph.graph(),
            binding.primal_tensor,
            &mut visited,
            &mut retained,
        );
    }
    retained.into_iter().collect()
}

fn collect_retained_forward_tensor_ids(
    graph: &psionic_ir::Graph,
    tensor_id: TensorId,
    visited: &mut std::collections::BTreeSet<TensorId>,
    retained: &mut std::collections::BTreeSet<TensorId>,
) {
    if !visited.insert(tensor_id) {
        return;
    }
    let Some(node) = graph.node(tensor_id) else {
        return;
    };
    if matches!(node.op(), psionic_ir::OpKind::Input { .. }) {
        return;
    }
    if is_forward_primal_rematerializable(node.op()) {
        for input in node.inputs() {
            collect_retained_forward_tensor_ids(graph, *input, visited, retained);
        }
        return;
    }
    retained.insert(tensor_id);
}

fn is_forward_primal_rematerializable(op: &psionic_ir::OpKind) -> bool {
    matches!(
        op,
        psionic_ir::OpKind::Reshape
            | psionic_ir::OpKind::Permute { .. }
            | psionic_ir::OpKind::Expand { .. }
            | psionic_ir::OpKind::Cast { .. }
    )
}

fn parameter_only_backward_plan(
    graph: &ParameterGolfBaselineTrainingGraph,
) -> Result<psionic_ir::AutodiffBackwardPlan, ParameterGolfSingleH100TrainingError> {
    let backward_plan = graph.graph.backward_plan(graph.loss_tensor_id)?;
    let parameter_targets = graph
        .parameter_bindings
        .iter()
        .map(|binding| binding.graph_input_tensor_id)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(filter_backward_plan_to_primal_targets(
        &backward_plan,
        &parameter_targets,
    ))
}

fn filter_backward_plan_to_primal_targets(
    backward_plan: &psionic_ir::AutodiffBackwardPlan,
    allowed_primal_tensors: &std::collections::BTreeSet<TensorId>,
) -> psionic_ir::AutodiffBackwardPlan {
    let gradient_targets = backward_plan
        .gradient_targets
        .iter()
        .filter(|target| allowed_primal_tensors.contains(&target.primal_tensor))
        .cloned()
        .collect::<Vec<_>>();
    let gradient_outputs = gradient_targets
        .iter()
        .map(|target| target.gradient_tensor)
        .collect::<Vec<_>>();
    let gradient_graph = backward_plan
        .gradient_graph
        .with_outputs(unique_tensor_ids(gradient_outputs));
    let live_tensor_ids = live_tensor_ids_for_outputs(&gradient_graph);
    let primal_bindings = backward_plan
        .primal_bindings
        .iter()
        .filter(|binding| live_tensor_ids.contains(&binding.gradient_graph_input))
        .cloned()
        .collect::<Vec<_>>();
    psionic_ir::AutodiffBackwardPlan {
        gradient_graph,
        primal_bindings,
        seed_input: backward_plan.seed_input,
        gradient_targets,
    }
}

fn live_tensor_ids_for_outputs(graph: &psionic_ir::Graph) -> std::collections::BTreeSet<TensorId> {
    let mut live = std::collections::BTreeSet::new();
    let mut stack = graph.outputs().to_vec();
    while let Some(tensor_id) = stack.pop() {
        if !live.insert(tensor_id) {
            continue;
        }
        if let Some(node) = graph.node(tensor_id) {
            stack.extend_from_slice(node.inputs());
        }
    }
    live
}

fn execute_backward_plan(
    cuda_backend: &mut CudaBackend,
    forward_graph: &psionic_ir::Graph,
    backward_plan: &psionic_ir::AutodiffBackwardPlan,
    forward_inputs: &BTreeMap<TensorId, CudaBuffer>,
    forward_outputs: &BTreeMap<TensorId, CudaBuffer>,
) -> Result<Vec<(TensorId, TensorData)>, ParameterGolfSingleH100TrainingError> {
    let mut available_values = forward_inputs.clone();
    available_values.extend(
        forward_outputs
            .iter()
            .map(|(tensor_id, buffer)| (*tensor_id, buffer.clone())),
    );
    let mut inputs = Vec::new();
    for binding in &backward_plan.primal_bindings {
        let values = match available_values.get(&binding.primal_tensor) {
            Some(values) => values.clone(),
            None => {
                let rematerialized = rematerialize_forward_primal_cuda_buffer(
                    cuda_backend,
                    forward_graph,
                    binding.primal_tensor,
                    &available_values,
                )?;
                available_values.insert(binding.primal_tensor, rematerialized.clone());
                rematerialized
            }
        };
        let input_dtype = backward_plan
            .gradient_graph
            .node(binding.gradient_graph_input)
            .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                tensor_id: binding.gradient_graph_input,
            })?
            .tensor()
            .spec()
            .dtype();
        let input_spec = backward_plan
            .gradient_graph
            .node(binding.gradient_graph_input)
            .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                tensor_id: binding.gradient_graph_input,
            })?
            .tensor()
            .spec()
            .clone();
        let input_buffer = if values.spec() == &input_spec {
            values
        } else {
            let host_values =
                materialize_cuda_buffer_for_dtype(&values, input_dtype, binding.primal_tensor)?;
            cuda_input_buffer_from_tensor_data(
                cuda_backend,
                &input_spec,
                binding.gradient_graph_input,
                &host_values,
            )?
        };
        inputs.push((binding.gradient_graph_input, input_buffer));
    }
    let seed_dtype = backward_plan
        .gradient_graph
        .node(backward_plan.seed_input)
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
            tensor_id: backward_plan.seed_input,
        })?
        .tensor()
        .spec()
        .dtype();
    inputs.push((
        backward_plan.seed_input,
        cuda_input_buffer_from_tensor_data(
            cuda_backend,
            backward_plan
                .gradient_graph
                .node(backward_plan.seed_input)
                .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: backward_plan.seed_input,
                })?
                .tensor()
                .spec(),
            backward_plan.seed_input,
            &floating_tensor_data_for_dtype(seed_dtype, vec![1.0_f32])?,
        )?,
    ));
    execute_cuda_graph_outputs_from_buffers(
        cuda_backend,
        &backward_plan.gradient_graph,
        &inputs.into_iter().collect(),
    )
}

struct ForwardPrimalRematerializationGraph {
    graph: Graph,
    output_tensor_id: TensorId,
    base_inputs: BTreeMap<TensorId, TensorId>,
}

fn rematerialize_forward_primal_cuda_buffer(
    cuda_backend: &mut CudaBackend,
    forward_graph: &psionic_ir::Graph,
    tensor_id: TensorId,
    available_values: &BTreeMap<TensorId, CudaBuffer>,
) -> Result<CudaBuffer, ParameterGolfSingleH100TrainingError> {
    let rematerialization =
        build_forward_primal_rematerialization_graph(forward_graph, tensor_id, available_values)?;
    let input_buffers = rematerialization
        .base_inputs
        .iter()
        .map(
            |(original_tensor_id, rematerialized_input_id)| -> Result<
                (TensorId, CudaBuffer),
                ParameterGolfSingleH100TrainingError,
            > {
                let buffer = available_values.get(original_tensor_id).ok_or(
                    ParameterGolfSingleH100TrainingError::MissingGraphOutput {
                        tensor_id: *original_tensor_id,
                    },
                )?;
                Ok((*rematerialized_input_id, buffer.clone()))
            },
        )
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let outputs = execute_cuda_graph_output_buffers_from_buffers(
        cuda_backend,
        &rematerialization.graph,
        &input_buffers,
    )?;
    outputs
        .get(&rematerialization.output_tensor_id)
        .cloned()
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphOutput {
            tensor_id: rematerialization.output_tensor_id,
        })
}

fn build_forward_primal_rematerialization_graph(
    forward_graph: &psionic_ir::Graph,
    tensor_id: TensorId,
    available_values: &BTreeMap<TensorId, CudaBuffer>,
) -> Result<ForwardPrimalRematerializationGraph, ParameterGolfSingleH100TrainingError> {
    let target_node = forward_graph
        .node(tensor_id)
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor { tensor_id })?;
    let mut builder = GraphBuilder::new(target_node.tensor().spec().device().clone());
    let mut base_inputs = BTreeMap::new();
    let mut cloned_tensors = BTreeMap::new();
    let output = clone_forward_primal_rematerialization_tensor(
        &mut builder,
        forward_graph,
        tensor_id,
        available_values,
        &mut base_inputs,
        &mut cloned_tensors,
    )?;
    let output_tensor_id = output.id();
    Ok(ForwardPrimalRematerializationGraph {
        graph: builder.finish(vec![output]),
        output_tensor_id,
        base_inputs,
    })
}

fn clone_forward_primal_rematerialization_tensor(
    builder: &mut GraphBuilder,
    forward_graph: &psionic_ir::Graph,
    tensor_id: TensorId,
    available_values: &BTreeMap<TensorId, CudaBuffer>,
    base_inputs: &mut BTreeMap<TensorId, TensorId>,
    cloned_tensors: &mut BTreeMap<TensorId, Tensor>,
) -> Result<Tensor, ParameterGolfSingleH100TrainingError> {
    if let Some(cloned) = cloned_tensors.get(&tensor_id) {
        return Ok(cloned.clone());
    }
    let node = forward_graph
        .node(tensor_id)
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor { tensor_id })?;
    let cloned = if available_values.contains_key(&tensor_id) {
        let rematerialized_input = builder.input(
            "rematerialized_forward_primal",
            node.tensor().spec().shape().clone(),
            node.tensor().spec().dtype(),
        );
        base_inputs.insert(tensor_id, rematerialized_input.id());
        rematerialized_input
    } else {
        match node.op() {
            OpKind::Reshape => {
                let input = clone_forward_primal_rematerialization_tensor(
                    builder,
                    forward_graph,
                    node.inputs()[0],
                    available_values,
                    base_inputs,
                    cloned_tensors,
                )?;
                builder.reshape(&input, node.tensor().spec().shape().clone())?
            }
            OpKind::Permute { axes } => {
                let input = clone_forward_primal_rematerialization_tensor(
                    builder,
                    forward_graph,
                    node.inputs()[0],
                    available_values,
                    base_inputs,
                    cloned_tensors,
                )?;
                builder.permute(&input, axes.clone())?
            }
            OpKind::Expand { shape } => {
                let input = clone_forward_primal_rematerialization_tensor(
                    builder,
                    forward_graph,
                    node.inputs()[0],
                    available_values,
                    base_inputs,
                    cloned_tensors,
                )?;
                builder.expand(&input, shape.clone())?
            }
            OpKind::Cast { dtype } => {
                let input = clone_forward_primal_rematerialization_tensor(
                    builder,
                    forward_graph,
                    node.inputs()[0],
                    available_values,
                    base_inputs,
                    cloned_tensors,
                )?;
                builder.cast(&input, *dtype)?
            }
            actual => {
                return Err(ParameterGolfSingleH100TrainingError::InvalidConfig {
                    message: format!(
                        "cannot rematerialize non-view forward primal tensor {tensor_id} through op {actual:?}"
                    ),
                });
            }
        }
    };
    cloned_tensors.insert(tensor_id, cloned.clone());
    Ok(cloned)
}

fn floating_tensor_data_for_dtype(
    dtype: DType,
    values: Vec<f32>,
) -> Result<TensorData, ParameterGolfSingleH100TrainingError> {
    match dtype {
        DType::F32 => Ok(TensorData::F32(values)),
        DType::BF16 => Ok(TensorData::BF16(values)),
        actual => Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: format!("unsupported floating tensor dtype {actual:?}"),
        }),
    }
}

fn tensor_data_for_dtype(
    dtype: DType,
    data: TensorData,
    tensor_id: TensorId,
) -> Result<TensorData, ParameterGolfSingleH100TrainingError> {
    match (dtype, data) {
        (DType::F32, TensorData::F32(values)) | (DType::F32, TensorData::BF16(values)) => {
            Ok(TensorData::F32(values))
        }
        (DType::BF16, TensorData::F32(values)) | (DType::BF16, TensorData::BF16(values)) => {
            Ok(TensorData::BF16(values))
        }
        (DType::I32, TensorData::I32(values)) => Ok(TensorData::I32(values)),
        (actual, observed) => Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: format!(
                "tensor {tensor_id} expected {actual:?} data but observed {:?}",
                match observed {
                    TensorData::F32(_) => DType::F32,
                    TensorData::BF16(_) => DType::BF16,
                    TensorData::I32(_) => DType::I32,
                    TensorData::QuantizedBlocks(_) => DType::I8,
                }
            ),
        }),
    }
}

fn backward_result_from_outputs(
    backward_plan: &psionic_ir::AutodiffBackwardPlan,
    outputs: &[(TensorId, TensorData)],
) -> Result<AutodiffBackwardResult, ParameterGolfSingleH100TrainingError> {
    let gradient_targets = backward_plan
        .gradient_targets
        .iter()
        .map(|target| (target.gradient_tensor, target.primal_tensor))
        .collect::<BTreeMap<_, _>>();
    let gradients = outputs
        .iter()
        .filter_map(|(tensor_id, values)| {
            gradient_targets
                .get(tensor_id)
                .copied()
                .map(|primal_tensor| (*tensor_id, primal_tensor, values.clone()))
        })
        .map(
            |(gradient_tensor_id, primal_tensor, values)| -> Result<
                (TensorId, TensorData),
                ParameterGolfSingleH100TrainingError,
            > {
            let dtype = backward_plan
                .gradient_graph
                .node(gradient_tensor_id)
                .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: gradient_tensor_id,
                })?
                .tensor()
                .spec()
                .dtype();
            Ok((
                primal_tensor,
                tensor_data_for_dtype(dtype, values, gradient_tensor_id)?,
            ))
        },
        )
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(AutodiffBackwardResult {
        forward_values: BTreeMap::new(),
        plan: backward_plan.clone(),
        gradients,
    })
}

fn execute_cuda_graph_outputs(
    cuda_backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &BTreeMap<TensorId, TensorData>,
) -> Result<Vec<(TensorId, TensorData)>, ParameterGolfSingleH100TrainingError> {
    let mut buffers = BTreeMap::new();
    for (tensor_id, data) in inputs {
        let spec = graph
            .node(*tensor_id)
            .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                tensor_id: *tensor_id,
            })?
            .tensor()
            .clone();
        let buffer =
            cuda_input_buffer_from_tensor_data(cuda_backend, spec.spec(), *tensor_id, data)?;
        buffers.insert(*tensor_id, buffer);
    }
    execute_cuda_graph_outputs_from_buffers(cuda_backend, graph, &buffers)
}

fn cuda_graph_input_buffers(
    cuda_backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &BTreeMap<TensorId, TensorData>,
) -> Result<BTreeMap<TensorId, CudaBuffer>, ParameterGolfSingleH100TrainingError> {
    let mut buffers = BTreeMap::new();
    for (tensor_id, data) in inputs {
        let spec = graph
            .node(*tensor_id)
            .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                tensor_id: *tensor_id,
            })?
            .tensor()
            .clone();
        let buffer =
            cuda_input_buffer_from_tensor_data(cuda_backend, spec.spec(), *tensor_id, data)?;
        buffers.insert(*tensor_id, buffer);
    }
    Ok(buffers)
}

fn execute_cuda_graph_output_buffers_from_buffers(
    cuda_backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &BTreeMap<TensorId, CudaBuffer>,
) -> Result<BTreeMap<TensorId, CudaBuffer>, ParameterGolfSingleH100TrainingError> {
    let result = cuda_backend.compile_and_execute(graph, inputs)?;
    graph
        .outputs()
        .iter()
        .map(|tensor_id| {
            let output = result.outputs.get(tensor_id).ok_or(
                ParameterGolfSingleH100TrainingError::MissingGraphOutput {
                    tensor_id: *tensor_id,
                },
            )?;
            Ok((*tensor_id, output.clone()))
        })
        .collect()
}

fn execute_cuda_graph_outputs_from_buffers(
    cuda_backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &BTreeMap<TensorId, CudaBuffer>,
) -> Result<Vec<(TensorId, TensorData)>, ParameterGolfSingleH100TrainingError> {
    let result = execute_cuda_graph_output_buffers_from_buffers(cuda_backend, graph, inputs)?;
    result
        .into_iter()
        .map(|(tensor_id, output)| {
            Ok((
                tensor_id,
                materialize_cuda_buffer_for_dtype(&output, output.spec().dtype(), tensor_id)?,
            ))
        })
        .collect()
}

fn scalar_float_cuda_buffer_output(
    outputs: &BTreeMap<TensorId, CudaBuffer>,
    tensor_id: TensorId,
) -> Result<f32, ParameterGolfSingleH100TrainingError> {
    outputs
        .get(&tensor_id)
        .map(|buffer| materialize_cuda_buffer_for_dtype(buffer, buffer.spec().dtype(), tensor_id))
        .transpose()?
        .and_then(|values| values.as_f32_slice().map(|slice| slice.to_vec()))
        .and_then(|values| values.first().copied())
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphOutput { tensor_id })
}

fn scalar_float_graph_output(
    outputs: &[(TensorId, TensorData)],
    tensor_id: TensorId,
) -> Result<f32, ParameterGolfSingleH100TrainingError> {
    outputs
        .iter()
        .find(|(current, _)| *current == tensor_id)
        .and_then(|(_, values)| values.as_f32_slice())
        .and_then(|values| values.first().copied())
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphOutput { tensor_id })
}

fn materialize_cuda_buffer_for_dtype(
    buffer: &CudaBuffer,
    dtype: DType,
    tensor_id: TensorId,
) -> Result<TensorData, ParameterGolfSingleH100TrainingError> {
    match dtype {
        psionic_core::DType::F32 => Ok(TensorData::F32(buffer.read_f32()?)),
        psionic_core::DType::BF16 => Ok(TensorData::BF16(buffer.read_bf16_to_f32()?)),
        psionic_core::DType::I32 => Ok(TensorData::I32(buffer.read_i32()?)),
        actual => Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: format!("unsupported graph output dtype {actual:?} for tensor {tensor_id}"),
        }),
    }
}

fn cuda_input_buffer_from_tensor_data(
    cuda_backend: &mut CudaBackend,
    spec: &TensorSpec,
    tensor_id: TensorId,
    data: &TensorData,
) -> Result<CudaBuffer, ParameterGolfSingleH100TrainingError> {
    match spec.dtype() {
        psionic_core::DType::F32 => match data {
            TensorData::F32(values) | TensorData::BF16(values) => {
                Ok(cuda_backend.input_buffer(spec.shape().clone(), values.clone())?)
            }
            TensorData::I32(_) | TensorData::QuantizedBlocks(_) => {
                Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "tensor {tensor_id} expected dense F32 host values for graph input"
                    ),
                })
            }
        },
        psionic_core::DType::BF16 => match data {
            TensorData::F32(values) | TensorData::BF16(values) => {
                Ok(cuda_backend.input_bf16_buffer(spec.shape().clone(), values.clone())?)
            }
            TensorData::I32(_) | TensorData::QuantizedBlocks(_) => {
                Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "tensor {tensor_id} expected dense BF16 host values for graph input"
                    ),
                })
            }
        },
        psionic_core::DType::I32 => match data {
            TensorData::I32(values) => {
                Ok(cuda_backend.input_i32_buffer(spec.shape().clone(), values.clone())?)
            }
            TensorData::F32(_) | TensorData::BF16(_) | TensorData::QuantizedBlocks(_) => {
                Err(ParameterGolfSingleH100TrainingError::Serialization {
                    message: format!(
                        "tensor {tensor_id} expected dense I32 host values for graph input"
                    ),
                })
            }
        },
        actual => Err(ParameterGolfSingleH100TrainingError::Serialization {
            message: format!("unsupported graph input dtype {actual:?} for tensor {tensor_id}"),
        }),
    }
}

fn dense_f32_graph_output(
    outputs: &[(TensorId, TensorData)],
    tensor_id: TensorId,
) -> Result<Vec<f32>, ParameterGolfSingleH100TrainingError> {
    outputs
        .iter()
        .find(|(current, _)| *current == tensor_id)
        .and_then(|(_, values)| values.as_f32_slice())
        .map(|values| values.to_vec())
        .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphOutput { tensor_id })
}

fn split_token_count(bundle: &ParameterGolfDatasetBundle, split_name: &str) -> u64 {
    bundle
        .manifest
        .split(split_name)
        .map(|split| split.shards.iter().map(|shard| shard.token_count).sum())
        .unwrap_or(0)
}

fn count_retained_tensor_elements(
    graph: &ParameterGolfBaselineTrainingGraph,
    retained_tensor_ids: &[TensorId],
) -> Result<u64, ParameterGolfSingleH100TrainingError> {
    retained_tensor_ids
        .iter()
        .try_fold(0_u64, |sum, tensor_id| {
            let node = graph.graph.graph().node(*tensor_id).ok_or(
                ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: *tensor_id,
                },
            )?;
            Ok(sum.saturating_add(node.tensor().spec().shape().element_count() as u64))
        })
}

fn count_gradient_elements(
    backward_plan: &psionic_ir::AutodiffBackwardPlan,
) -> Result<u64, ParameterGolfSingleH100TrainingError> {
    backward_plan
        .gradient_targets
        .iter()
        .try_fold(0_u64, |sum, target| {
            let node = backward_plan
                .gradient_graph
                .node(target.gradient_tensor)
                .ok_or(ParameterGolfSingleH100TrainingError::MissingGraphTensor {
                    tensor_id: target.gradient_tensor,
                })?;
            Ok(sum.saturating_add(node.tensor().spec().shape().element_count() as u64))
        })
}

pub(crate) fn clip_gradients(
    gradients: &mut [(String, Vec<f32>)],
    max_norm: f32,
) -> crate::GradientClipObservation {
    let non_finite_count = crate::non_finite_value_count(gradients);
    if !(max_norm.is_finite() && max_norm > 0.0) {
        return crate::GradientClipObservation {
            gradient_norm_after_clip: crate::finite_l2_norm(gradients),
            clip_applied: false,
            non_finite_count,
        };
    }
    let norm = gradients
        .iter()
        .flat_map(|(_, values)| values.iter())
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    let clip_applied = norm > max_norm && norm > f32::EPSILON;
    if clip_applied {
        let scale = max_norm / norm;
        for (_, values) in &mut *gradients {
            for value in values {
                *value *= scale;
            }
        }
    }
    crate::GradientClipObservation {
        gradient_norm_after_clip: crate::finite_l2_norm(gradients),
        clip_applied,
        non_finite_count,
    }
}

#[cfg(test)]
mod tests {
    use psionic_core::{DType, Device, Shape};
    use psionic_ir::{AutodiffContext, AutodiffGraphBuilder};
    use psionic_models::{ModelDescriptor, ParameterGolfReferenceModel};

    use super::*;

    fn competitive_homegolf_v1_fixture_model(
    ) -> Result<ParameterGolfReferenceModel, Box<dyn std::error::Error>> {
        let config = ParameterGolfConfig::competitive_homegolf_v1();
        let weights = ParameterGolfWeights::from_initializer(&config, Default::default())?;
        Ok(ParameterGolfReferenceModel::new(
            ModelDescriptor::new(
                "parameter-golf-competitive-homegolf-v1-fixture",
                "parameter_golf",
                "fixture",
            ),
            config,
            weights,
        )?)
    }

    #[test]
    fn backward_result_from_outputs_rekeys_gradient_outputs_to_primal_tensors(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 1]), DType::F32, true);
        let squared = builder.mul(&input, &input)?;
        let graph = builder.finish(vec![squared.clone()]);
        let backward_plan = graph.backward_plan(squared.id())?;
        let gradient_tensor_id = backward_plan
            .gradient_for(input.id())
            .ok_or("missing input gradient target")?;

        let backward_result = backward_result_from_outputs(
            &backward_plan,
            &[(gradient_tensor_id, TensorData::F32(vec![42.0_f32]))],
        )?;

        assert_eq!(
            backward_result
                .gradient(input.id())
                .and_then(TensorData::as_f32_slice),
            Some(&[42.0_f32][..])
        );
        Ok(())
    }

    #[test]
    fn backward_result_from_outputs_preserves_bf16_gradient_dtype(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut builder =
            AutodiffGraphBuilder::with_context(Device::cpu(), AutodiffContext::training());
        let input = builder.input("x", Shape::new(vec![1, 1]), DType::BF16, true);
        let squared = builder.mul(&input, &input)?;
        let graph = builder.finish(vec![squared.clone()]);
        let backward_plan = graph.backward_plan(squared.id())?;
        let gradient_tensor_id = backward_plan
            .gradient_for(input.id())
            .ok_or("missing input gradient target")?;

        let backward_result = backward_result_from_outputs(
            &backward_plan,
            &[(gradient_tensor_id, TensorData::BF16(vec![42.0_f32]))],
        )?;

        assert!(matches!(
            backward_result.gradient(input.id()),
            Some(TensorData::BF16(values)) if values == &vec![42.0_f32]
        ));
        Ok(())
    }

    #[test]
    fn tensor_data_for_dtype_preserves_i32_outputs() -> Result<(), Box<dyn std::error::Error>> {
        let tensor_id = TensorId(7);
        assert_eq!(
            tensor_data_for_dtype(DType::I32, TensorData::I32(vec![1, 2, 3]), tensor_id)?,
            TensorData::I32(vec![1, 2, 3])
        );
        Ok(())
    }

    #[test]
    fn challenge_defaults_use_widened_train_gpt_control_loop_defaults() {
        let config = ParameterGolfSingleH100TrainingConfig::challenge_defaults(
            "/tmp/dataset",
            "/tmp/tokenizer.model",
        );

        assert_eq!(config.max_steps, config.hyperparameters.iterations);
        assert_eq!(config.warmup_steps, 20);
        assert_eq!(config.validation_loss_every, 1_000);
        assert_eq!(config.train_log_every, 200);
        assert_eq!(
            config.final_validation_mode,
            ParameterGolfSingleH100ValidationMode::Both
        );
        assert_eq!(config.ema, None);
        assert_eq!(config.swa, None);
        assert_eq!(
            config.final_model_surface,
            ParameterGolfFinalModelSurface::Raw
        );
        assert_eq!(
            config.final_artifact_config,
            ParameterGolfFinalArtifactConfig::default()
        );
        assert_eq!(config.hyperparameters.max_wallclock_seconds, Some(600.0));
    }

    #[test]
    fn bounded_proof_defaults_disable_widened_loop_features() {
        let config = ParameterGolfSingleH100TrainingConfig::bounded_proof_defaults(
            "/tmp/dataset",
            "/tmp/tokenizer.model",
            3,
        );

        assert_eq!(config.max_steps, 3);
        assert_eq!(config.warmup_steps, 0);
        assert_eq!(config.validation_loss_every, 0);
        assert_eq!(config.train_log_every, 1);
        assert_eq!(
            config.final_validation_mode,
            ParameterGolfSingleH100ValidationMode::RoundtripOnly
        );
        assert_eq!(config.ema, None);
        assert_eq!(config.swa, None);
        assert_eq!(
            config.final_model_surface,
            ParameterGolfFinalModelSurface::Raw
        );
        assert_eq!(
            config.final_artifact_config,
            ParameterGolfFinalArtifactConfig::default()
        );
        assert_eq!(config.hyperparameters.max_wallclock_seconds, None);
    }

    #[test]
    fn live_validation_checkpoints_skip_step_zero_but_keep_later_and_final_validation() {
        assert!(!should_run_live_validation_checkpoint(
            0,
            false,
            1_000,
            ParameterGolfSingleH100ValidationMode::Both,
            ParameterGolfFinalModelSurface::Raw,
        ));
        assert!(!should_run_live_validation_checkpoint(
            999,
            false,
            1_000,
            ParameterGolfSingleH100ValidationMode::Both,
            ParameterGolfFinalModelSurface::Raw,
        ));
        assert!(should_run_live_validation_checkpoint(
            1_000,
            false,
            1_000,
            ParameterGolfSingleH100ValidationMode::Both,
            ParameterGolfFinalModelSurface::Raw,
        ));
        assert!(should_run_live_validation_checkpoint(
            0,
            true,
            1_000,
            ParameterGolfSingleH100ValidationMode::Both,
            ParameterGolfFinalModelSurface::Raw,
        ));
        assert!(!should_run_live_validation_checkpoint(
            1_000,
            true,
            1_000,
            ParameterGolfSingleH100ValidationMode::Both,
            ParameterGolfFinalModelSurface::Ema,
        ));
    }

    #[test]
    fn config_rejects_ema_surface_without_ema_config() -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("psionic-pgolf-ema-config-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let tokenizer_path = root.join("tokenizer.model");
        std::fs::write(&tokenizer_path, b"stub")?;

        let mut config =
            ParameterGolfSingleH100TrainingConfig::challenge_defaults(&root, &tokenizer_path);
        config.final_model_surface = ParameterGolfFinalModelSurface::Ema;
        let error = config
            .validate()
            .expect_err("ema export without ema config must fail");
        assert!(error
            .to_string()
            .contains("final_model_surface=ema requires an explicit ema config"));
        Ok(())
    }

    #[test]
    fn config_rejects_swa_surface_without_swa_config() -> Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("psionic-pgolf-swa-config-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let tokenizer_path = root.join("tokenizer.model");
        std::fs::write(&tokenizer_path, b"stub")?;

        let mut config =
            ParameterGolfSingleH100TrainingConfig::challenge_defaults(&root, &tokenizer_path);
        config.final_model_surface = ParameterGolfFinalModelSurface::Swa;
        let error = config
            .validate()
            .expect_err("swa export without swa config must fail");
        assert!(error
            .to_string()
            .contains("final_model_surface=swa requires an explicit swa config"));
        Ok(())
    }

    #[test]
    fn config_rejects_ema_backed_swa_without_ema_config() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = std::env::temp_dir().join(format!(
            "psionic-pgolf-swa-ema-config-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)?;
        let tokenizer_path = root.join("tokenizer.model");
        std::fs::write(&tokenizer_path, b"stub")?;

        let mut config =
            ParameterGolfSingleH100TrainingConfig::challenge_defaults(&root, &tokenizer_path);
        config.swa = Some(ParameterGolfSwaConfig {
            source_surface: ParameterGolfSwaSourceSurface::Ema,
            ..Default::default()
        });
        let error = config
            .validate()
            .expect_err("ema-backed swa without ema config must fail");
        assert!(error
            .to_string()
            .contains("swa source_surface=ema requires an explicit ema config"));
        Ok(())
    }

    #[test]
    fn ema_state_updates_parameter_values_and_materializes_model(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let optimizer_plan = parameter_golf_optimizer_plan(model.descriptor(), &hyperparameters)?;
        let mut trainer_state = seed_parameter_states(&model, &optimizer_plan)?;
        let mut ema_state =
            ParameterGolfEmaState::new(ParameterGolfEmaConfig { decay: 0.5 }, &trainer_state);
        let (parameter_id, original_value) = {
            let (parameter_id, state) = trainer_state
                .parameter_states
                .iter()
                .next()
                .expect("trainer state should contain at least one parameter");
            (parameter_id.clone(), state.values()[0])
        };

        match trainer_state
            .parameter_states
            .get_mut(&parameter_id)
            .expect("parameter state should still exist")
        {
            ParameterGolfParameterState::AdamBf16Master {
                train_visible_values,
                ..
            } => train_visible_values[0] += 2.0,
            ParameterGolfParameterState::AdamFp32 { values, .. }
            | ParameterGolfParameterState::MuonBf16 { values, .. } => values[0] += 2.0,
        }
        let updated_value = trainer_state
            .parameter_states
            .get(&parameter_id)
            .expect("updated parameter state should still exist")
            .values()[0];

        ema_state.update(&trainer_state)?;
        let ema_value = ema_state
            .parameter_values
            .get(&parameter_id)
            .expect("ema parameter should exist")[0];
        assert_eq!(ema_value, (original_value * 0.5) + (updated_value * 0.5));

        let ema_model = materialize_model_from_overrides(&model, &ema_state.parameter_values)?;
        let ema_vector = ema_model
            .all_parameter_vectors()?
            .into_iter()
            .find(|vector| vector.parameter_id == parameter_id)
            .expect("ema model should retain the updated parameter");
        assert_eq!(ema_vector.values[0], ema_value);
        Ok(())
    }

    #[test]
    fn swa_state_accumulates_parameter_values_and_materializes_model(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let optimizer_plan = parameter_golf_optimizer_plan(model.descriptor(), &hyperparameters)?;
        let mut trainer_state = seed_parameter_states(&model, &optimizer_plan)?;
        let mut swa_state = ParameterGolfSwaState::new(ParameterGolfSwaConfig {
            every_steps: 2,
            max_learning_rate_multiplier: 0.2,
            source_surface: ParameterGolfSwaSourceSurface::Raw,
        });
        let (parameter_id, original_value) = {
            let (parameter_id, state) = trainer_state
                .parameter_states
                .iter()
                .next()
                .expect("trainer state should contain at least one parameter");
            (parameter_id.clone(), state.values()[0])
        };

        let first_snapshot = current_parameter_state_overrides(&trainer_state);
        swa_state.update_from_overrides(&first_snapshot)?;

        match trainer_state
            .parameter_states
            .get_mut(&parameter_id)
            .expect("parameter state should still exist")
        {
            ParameterGolfParameterState::AdamBf16Master {
                train_visible_values,
                ..
            } => train_visible_values[0] += 2.0,
            ParameterGolfParameterState::AdamFp32 { values, .. }
            | ParameterGolfParameterState::MuonBf16 { values, .. } => values[0] += 2.0,
        }
        let second_snapshot = current_parameter_state_overrides(&trainer_state);
        let updated_value = second_snapshot
            .get(&parameter_id)
            .expect("updated parameter should exist")[0];
        swa_state.update_from_overrides(&second_snapshot)?;
        assert_eq!(swa_state.sample_count, 2);

        let averaged_values = swa_state.averaged_parameter_values()?;
        let swa_value = averaged_values
            .get(&parameter_id)
            .expect("swa parameter should exist")[0];
        assert_eq!(swa_value, (original_value + updated_value) / 2.0);

        let swa_model = materialize_model_from_overrides(&model, &averaged_values)?;
        let swa_vector = swa_model
            .all_parameter_vectors()?
            .into_iter()
            .find(|vector| vector.parameter_id == parameter_id)
            .expect("swa model should retain the averaged parameter");
        assert_eq!(swa_vector.values[0], swa_value);
        Ok(())
    }

    #[test]
    fn validation_mode_parser_accepts_all_supported_labels(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            ParameterGolfSingleH100ValidationMode::parse("live_only")?,
            ParameterGolfSingleH100ValidationMode::LiveOnly
        );
        assert_eq!(
            ParameterGolfSingleH100ValidationMode::parse("roundtrip_only")?,
            ParameterGolfSingleH100ValidationMode::RoundtripOnly
        );
        assert_eq!(
            ParameterGolfSingleH100ValidationMode::parse("both")?,
            ParameterGolfSingleH100ValidationMode::Both
        );
        assert_eq!(
            ParameterGolfSingleH100ValidationMode::parse("artifact_only")?,
            ParameterGolfSingleH100ValidationMode::ArtifactOnly
        );
        Ok(())
    }

    #[test]
    fn validation_eval_mode_parser_accepts_supported_labels(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            ParameterGolfValidationEvalMode::parse("non_overlapping")?,
            ParameterGolfValidationEvalMode::NonOverlapping
        );
        assert_eq!(
            ParameterGolfValidationEvalMode::parse("sliding_window:64")?,
            ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 }
        );
        Ok(())
    }

    #[test]
    fn execute_backward_plan_rematerializes_layout_mismatched_cuda_primals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let batch_size = 1;
        let sequence_length = 16;
        let graph = build_parameter_golf_baseline_training_graph(
            selected_device.device.clone(),
            model.descriptor(),
            batch_size,
            sequence_length,
        )?;
        let input_ids = vec![vec![0_u32; sequence_length]];
        let target_ids = vec![vec![1_u32; sequence_length]];
        let inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &graph,
            &model,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        let backward_plan = graph.graph.backward_plan(graph.loss_tensor_id)?;
        let retained_graph = retained_forward_graph(&graph, &backward_plan);
        let input_buffers = cuda_graph_input_buffers(&mut cuda_backend, &retained_graph, &inputs)?;
        let forward_outputs = execute_cuda_graph_output_buffers_from_buffers(
            &mut cuda_backend,
            &retained_graph,
            &input_buffers,
        )?;
        let backward_outputs = execute_backward_plan(
            &mut cuda_backend,
            graph.graph.graph(),
            &backward_plan,
            &input_buffers,
            &forward_outputs,
        )?;
        let backward_result =
            backward_result_from_outputs(&backward_plan, backward_outputs.as_slice())?;
        let gradients = materialize_parameter_golf_baseline_training_gradients(
            &graph,
            &backward_result,
            &model.descriptor().config,
            input_ids.as_slice(),
        )?;

        assert!(!gradients.parameter_gradients.is_empty());
        Ok(())
    }

    #[test]
    fn parameter_only_backward_plan_filters_non_parameter_targets(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let graph = build_parameter_golf_baseline_training_graph(
            psionic_core::Device::cpu(),
            model.descriptor(),
            1,
            16,
        )?;
        let full_plan = graph.graph.backward_plan(graph.loss_tensor_id)?;
        let filtered_plan = parameter_only_backward_plan(&graph)?;
        let parameter_targets = graph
            .parameter_bindings
            .iter()
            .map(|binding| binding.graph_input_tensor_id)
            .collect::<std::collections::BTreeSet<_>>();

        assert!(full_plan.gradient_targets.len() > filtered_plan.gradient_targets.len());
        assert_eq!(
            filtered_plan.gradient_targets.len(),
            graph.parameter_bindings.len()
        );
        assert!(filtered_plan
            .gradient_targets
            .iter()
            .all(|target| parameter_targets.contains(&target.primal_tensor)));
        assert_eq!(
            filtered_plan.gradient_graph.outputs().len(),
            filtered_plan.gradient_targets.len()
        );
        let live_tensor_ids = live_tensor_ids_for_outputs(&filtered_plan.gradient_graph);
        assert!(filtered_plan
            .primal_bindings
            .iter()
            .all(|binding| live_tensor_ids.contains(&binding.gradient_graph_input)));
        assert!(full_plan.primal_bindings.len() >= filtered_plan.primal_bindings.len());
        let retained_tensor_ids = retained_forward_tensor_ids(&graph, &filtered_plan);
        assert!(retained_tensor_ids.len() < filtered_plan.primal_bindings.len());
        assert!(retained_tensor_ids.iter().all(|tensor_id| {
            graph
                .graph
                .graph()
                .node(*tensor_id)
                .is_some_and(|node| !matches!(node.op(), psionic_ir::OpKind::Input { .. }))
        }));
        Ok(())
    }

    #[test]
    fn retained_forward_tensor_ids_skip_rematerializable_view_and_cast_primals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let graph = build_parameter_golf_baseline_training_graph(
            psionic_core::Device::cpu(),
            model.descriptor(),
            1,
            16,
        )?;
        let filtered_plan = parameter_only_backward_plan(&graph)?;
        let naive_retained_tensor_ids = filtered_plan
            .primal_bindings
            .iter()
            .filter_map(|binding| {
                graph
                    .graph
                    .graph()
                    .node(binding.primal_tensor)
                    .and_then(|node| {
                        (!matches!(node.op(), psionic_ir::OpKind::Input { .. }))
                            .then_some(binding.primal_tensor)
                    })
            })
            .collect::<std::collections::BTreeSet<_>>();
        let rematerializable_primal_tensor_ids = filtered_plan
            .primal_bindings
            .iter()
            .filter_map(|binding| {
                graph
                    .graph
                    .graph()
                    .node(binding.primal_tensor)
                    .and_then(|node| {
                        is_forward_primal_rematerializable(node.op())
                            .then_some(binding.primal_tensor)
                    })
            })
            .collect::<std::collections::BTreeSet<_>>();
        let retained_tensor_ids = retained_forward_tensor_ids(&graph, &filtered_plan)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();

        assert!(!rematerializable_primal_tensor_ids.is_empty());
        assert!(retained_tensor_ids.len() < naive_retained_tensor_ids.len());
        assert!(rematerializable_primal_tensor_ids
            .iter()
            .all(|tensor_id| !retained_tensor_ids.contains(tensor_id)));
        assert!(retained_tensor_ids.iter().all(|tensor_id| {
            graph.graph.graph().node(*tensor_id).is_some_and(|node| {
                !matches!(node.op(), psionic_ir::OpKind::Input { .. })
                    && !is_forward_primal_rematerializable(node.op())
            })
        }));
        Ok(())
    }

    #[test]
    fn pgolf_relu_squared_backward_binds_activation_outputs_not_preactivation_inputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let graph = build_parameter_golf_baseline_training_graph(
            psionic_core::Device::cpu(),
            model.descriptor(),
            1,
            16,
        )?;
        let filtered_plan = parameter_only_backward_plan(&graph)?;
        let retained_tensor_ids = retained_forward_tensor_ids(&graph, &filtered_plan)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let bound_primal_tensors = filtered_plan
            .primal_bindings
            .iter()
            .map(|binding| binding.primal_tensor)
            .collect::<std::collections::BTreeSet<_>>();
        let relu_squared_nodes = graph
            .graph
            .graph()
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.op(),
                    psionic_ir::OpKind::BackendExtension {
                        op: psionic_core::BackendExtensionOp::ReluSquared
                    }
                )
            })
            .collect::<Vec<_>>();

        assert!(!relu_squared_nodes.is_empty());
        for node in relu_squared_nodes {
            let output_id = node.tensor().id();
            let input_id = node.inputs()[0];
            assert!(bound_primal_tensors.contains(&output_id));
            assert!(!bound_primal_tensors.contains(&input_id));
            assert!(retained_tensor_ids.contains(&output_id));
            assert!(!retained_tensor_ids.contains(&input_id));
        }
        Ok(())
    }

    #[test]
    fn device_resident_training_session_matches_legacy_loss_and_gradients(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let batch_size = 1;
        let sequence_length = 16;
        let graph = build_parameter_golf_baseline_training_graph(
            selected_device.device.clone(),
            model.descriptor(),
            batch_size,
            sequence_length,
        )?;
        let input_ids = vec![vec![0_u32; sequence_length]];
        let target_ids = vec![vec![1_u32; sequence_length]];

        let legacy_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &graph,
            &model,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        let backward_plan = graph.graph.backward_plan(graph.loss_tensor_id)?;
        let retained_graph = retained_forward_graph(&graph, &backward_plan);
        let legacy_input_buffers =
            cuda_graph_input_buffers(&mut cuda_backend, &retained_graph, &legacy_inputs)?;
        let legacy_forward_outputs = execute_cuda_graph_output_buffers_from_buffers(
            &mut cuda_backend,
            &retained_graph,
            &legacy_input_buffers,
        )?;
        let legacy_loss =
            scalar_float_cuda_buffer_output(&legacy_forward_outputs, graph.loss_tensor_id)?;
        let legacy_backward_outputs = execute_backward_plan(
            &mut cuda_backend,
            graph.graph.graph(),
            &backward_plan,
            &legacy_input_buffers,
            &legacy_forward_outputs,
        )?;
        let legacy_backward_result =
            backward_result_from_outputs(&backward_plan, legacy_backward_outputs.as_slice())?;
        let legacy_gradients = materialize_parameter_golf_baseline_training_gradients(
            &graph,
            &legacy_backward_result,
            &model.descriptor().config,
            input_ids.as_slice(),
        )?;

        let mut resident_session = ParameterGolfCudaTrainingSession::new(
            &mut cuda_backend,
            graph.clone(),
            &model,
            Some(&model.banked_weights()?),
            batch_size,
            sequence_length,
        )?;
        let (resident_loss, resident_backward_outputs, resident_runtime) = resident_session
            .execute_batch(
                &mut cuda_backend,
                input_ids.as_slice(),
                target_ids.as_slice(),
            )?;
        let resident_backward_result = backward_result_from_outputs(
            &resident_session.backward_plan,
            resident_backward_outputs.as_slice(),
        )?;
        let resident_gradients = materialize_parameter_golf_baseline_training_gradients(
            &graph,
            &resident_backward_result,
            &model.descriptor().config,
            input_ids.as_slice(),
        )?;

        assert!((legacy_loss - resident_loss).abs() < 1.0e-5);
        assert_eq!(
            legacy_gradients
                .parameter_gradients
                .keys()
                .collect::<Vec<_>>(),
            resident_gradients
                .parameter_gradients
                .keys()
                .collect::<Vec<_>>()
        );
        for (parameter_id, legacy_values) in &legacy_gradients.parameter_gradients {
            let resident_values = resident_gradients
                .parameter_gradients
                .get(parameter_id)
                .ok_or("missing resident gradient parameter")?;
            assert_eq!(legacy_values.len(), resident_values.len());
            for (legacy, resident) in legacy_values.iter().zip(resident_values.iter()) {
                assert!((legacy - resident).abs() < 1.0e-4);
            }
        }
        assert!(resident_runtime.resident_parameter_buffers_reused);
        assert!(resident_runtime.resident_parameter_upload_us > 0);
        assert_eq!(resident_runtime.parameter_refresh_us, 0);

        resident_session.refresh_parameters(&model, Some(&model.banked_weights()?))?;
        let (_, _, refreshed_runtime) = resident_session.execute_batch(
            &mut cuda_backend,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        assert_eq!(refreshed_runtime.resident_parameter_upload_us, 0);
        assert!(refreshed_runtime.parameter_refresh_us > 0);

        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let optimizer_plan = parameter_golf_optimizer_plan(model.descriptor(), &hyperparameters)?;
        let trainer_state = seed_parameter_states(&model, &optimizer_plan)?;
        resident_session.refresh_parameters_from_state(&trainer_state)?;
        let (_, _, state_refreshed_runtime) = resident_session.execute_batch(
            &mut cuda_backend,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        assert_eq!(state_refreshed_runtime.resident_parameter_upload_us, 0);
        assert!(state_refreshed_runtime.parameter_refresh_us > 0);
        Ok(())
    }

    #[test]
    fn device_resident_banked_training_session_matches_reference_loss_and_gradients(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let descriptor = model.banked_descriptor()?;
        let batch_size = 1;
        let sequence_length = 16;
        let graph = build_parameter_golf_baseline_training_graph(
            selected_device.device.clone(),
            &descriptor,
            batch_size,
            sequence_length,
        )?;
        let input_ids = vec![vec![0_u32; sequence_length]];
        let target_ids = vec![vec![1_u32; sequence_length]];
        let reference_inputs = bind_parameter_golf_baseline_training_graph_inputs(
            &graph,
            &model,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        let backward_plan = graph.graph.backward_plan(graph.loss_tensor_id)?;
        let retained_graph = retained_forward_graph(&graph, &backward_plan);
        let reference_input_buffers =
            cuda_graph_input_buffers(&mut cuda_backend, &retained_graph, &reference_inputs)?;
        let reference_forward = execute_cuda_graph_output_buffers_from_buffers(
            &mut cuda_backend,
            &retained_graph,
            &reference_input_buffers,
        )?;
        let reference_loss =
            scalar_float_cuda_buffer_output(&reference_forward, graph.loss_tensor_id)?;
        let reference_backward_outputs = execute_backward_plan(
            &mut cuda_backend,
            graph.graph.graph(),
            &backward_plan,
            &reference_input_buffers,
            &reference_forward,
        )?;
        let reference_backward =
            backward_result_from_outputs(&backward_plan, reference_backward_outputs.as_slice())?;
        let reference_gradients = materialize_parameter_golf_baseline_training_gradients(
            &graph,
            &reference_backward,
            &descriptor.config,
            input_ids.as_slice(),
        )?;

        let mut resident_session = ParameterGolfCudaTrainingSession::new(
            &mut cuda_backend,
            graph.clone(),
            &model,
            Some(&model.banked_weights()?),
            batch_size,
            sequence_length,
        )?;
        let (resident_loss, resident_backward_outputs, _) = resident_session.execute_batch(
            &mut cuda_backend,
            input_ids.as_slice(),
            target_ids.as_slice(),
        )?;
        let resident_backward_result = backward_result_from_outputs(
            &resident_session.backward_plan,
            resident_backward_outputs.as_slice(),
        )?;
        let resident_gradients = materialize_parameter_golf_baseline_training_gradients(
            &graph,
            &resident_backward_result,
            &descriptor.config,
            input_ids.as_slice(),
        )?;

        assert!((reference_loss - resident_loss).abs() < 1.0e-5);
        assert_eq!(
            reference_gradients
                .parameter_gradients
                .keys()
                .collect::<Vec<_>>(),
            resident_gradients
                .parameter_gradients
                .keys()
                .collect::<Vec<_>>()
        );
        for (parameter_id, reference_values) in &reference_gradients.parameter_gradients {
            let resident_values = resident_gradients
                .parameter_gradients
                .get(parameter_id)
                .ok_or("missing resident gradient parameter")?;
            assert_eq!(reference_values.len(), resident_values.len());
            for (reference, resident) in reference_values.iter().zip(resident_values.iter()) {
                assert!((reference - resident).abs() < 1.0e-4);
            }
        }
        Ok(())
    }

    #[test]
    fn device_resident_validation_session_reuses_training_parameter_buffers_across_batch_sizes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let descriptor = model.banked_descriptor()?;
        let banked_weights = model.banked_weights()?;
        let sequence_length = 16;
        let training_graph = build_parameter_golf_baseline_training_graph(
            selected_device.device.clone(),
            &descriptor,
            1,
            sequence_length,
        )?;
        let training_session = ParameterGolfCudaTrainingSession::new(
            &mut cuda_backend,
            training_graph,
            &model,
            Some(&banked_weights),
            1,
            sequence_length,
        )?;
        let validation_graph = build_parameter_golf_baseline_eval_graph(
            selected_device.device.clone(),
            &descriptor,
            2,
            sequence_length,
        )?;
        let validation_session =
            ParameterGolfCudaValidationSession::new_from_resident_training_session(
                &mut cuda_backend,
                validation_graph,
                &training_session,
                2,
                sequence_length,
            )?;

        assert_eq!(validation_session.resident_parameter_upload_us, 0);
        assert_eq!(
            validation_session.persistent_parameter_buffer_count,
            training_session.persistent_parameter_buffer_count
        );
        for binding in &validation_session.graph.parameter_bindings {
            let training_binding = training_session
                .graph
                .parameter_binding(&binding.parameter_id)
                .ok_or("missing matching training binding")?;
            let training_buffer = training_session
                .static_inputs
                .get(&training_binding.graph_input_tensor_id)
                .ok_or("missing resident training buffer")?;
            let validation_buffer = validation_session
                .static_inputs
                .get(&binding.graph_input_tensor_id)
                .ok_or("missing resident validation buffer")?;
            assert_eq!(
                validation_buffer.allocation_identity(),
                training_buffer.allocation_identity()
            );
        }
        Ok(())
    }

    #[test]
    fn device_resident_validation_reuses_training_parameter_buffers_for_resident_eval(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let banked_weights = model.banked_weights()?;
        let sequence_length = 16;
        let mut training_graph_cache = BTreeMap::new();
        let mut training_session_cache = BTreeMap::new();
        training_session_for_batch(
            &mut training_session_cache,
            &mut cuda_backend,
            &mut training_graph_cache,
            selected_device.device.clone(),
            &model,
            Some(&banked_weights),
            1,
            sequence_length,
            ParameterGolfMatrixExecutionMode::DirectBanked,
        )?;

        let byte_luts = ParameterGolfSentencePieceByteLuts {
            base_bytes_lut: vec![1; 8],
            has_leading_space_lut: vec![false; 8],
            is_boundary_token_lut: vec![false; 8],
        };
        let validation_tokens = (0..33).map(|index| (index % 8) as u16).collect::<Vec<_>>();
        let mut eval_graph_cache = BTreeMap::new();
        let eager_summary = evaluate_validation_on_cuda(
            &mut cuda_backend,
            &selected_device.device,
            model.descriptor(),
            &model,
            validation_tokens.as_slice(),
            &byte_luts,
            sequence_length,
            2,
            &ParameterGolfValidationEvalMode::NonOverlapping,
            &mut eval_graph_cache,
            "resident_eval_eager",
            None,
        )?;
        let mut resident_eval_graph_cache = BTreeMap::new();
        let resident_summary = evaluate_validation_on_cuda_with_resident_training_parameters(
            &mut cuda_backend,
            &selected_device.device,
            model.descriptor(),
            &model,
            &training_session_cache,
            validation_tokens.as_slice(),
            &byte_luts,
            sequence_length,
            2,
            &ParameterGolfValidationEvalMode::NonOverlapping,
            &mut resident_eval_graph_cache,
            "resident_eval_reused",
            None,
        )?;

        let eager_runtime = eager_summary
            .runtime_receipt
            .ok_or("missing eager validation runtime receipt")?;
        let resident_runtime = resident_summary
            .runtime_receipt
            .ok_or("missing resident validation runtime receipt")?;
        assert_eq!(
            eager_summary.evaluated_token_count,
            resident_summary.evaluated_token_count
        );
        assert_eq!(
            eager_summary.evaluated_byte_count,
            resident_summary.evaluated_byte_count
        );
        assert!((eager_summary.mean_loss - resident_summary.mean_loss).abs() < 1.0e-6);
        assert!((eager_summary.bits_per_byte - resident_summary.bits_per_byte).abs() < 1.0e-6);
        assert!(eager_runtime.resident_parameter_upload_us > 0);
        assert_eq!(resident_runtime.resident_parameter_upload_us, 0);
        assert_eq!(
            eager_runtime.persistent_parameter_buffer_count,
            resident_runtime.persistent_parameter_buffer_count
        );
        Ok(())
    }

    #[test]
    fn competitive_homegolf_validation_session_binds_bigram_input_tensor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = competitive_homegolf_v1_fixture_model()?;
        let descriptor = model.banked_descriptor()?;
        let sequence_length = 16;
        let validation_graph = build_parameter_golf_baseline_eval_graph(
            selected_device.device.clone(),
            &descriptor,
            2,
            sequence_length,
        )?;
        assert!(validation_graph.bigram_token_ids_tensor_id.is_some());
        let mut validation_session = ParameterGolfCudaValidationSession::new(
            &mut cuda_backend,
            validation_graph,
            &model,
            2,
            sequence_length,
        )?;
        let byte_luts = ParameterGolfSentencePieceByteLuts {
            base_bytes_lut: vec![1; model.descriptor().config.vocab_size],
            has_leading_space_lut: vec![false; model.descriptor().config.vocab_size],
            is_boundary_token_lut: vec![false; model.descriptor().config.vocab_size],
        };
        let validation_tokens = (0..33).map(|index| (index % 16) as u16).collect::<Vec<_>>();
        let batch_plan = build_non_overlapping_validation_batch_plans(
            validation_tokens.as_slice(),
            &byte_luts,
            sequence_length,
            2,
        )?
        .into_iter()
        .next()
        .ok_or("missing validation batch plan")?;

        let (batch_token_losses, _) =
            validation_session.execute_batch(&mut cuda_backend, &batch_plan)?;
        assert_eq!(batch_token_losses.len(), 2 * sequence_length);
        Ok(())
    }

    #[test]
    fn competitive_homegolf_training_session_binds_bigram_input_tensor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = competitive_homegolf_v1_fixture_model()?;
        let descriptor = model.banked_descriptor()?;
        let banked_weights = model.banked_weights()?;
        let sequence_length = 16;
        let training_graph = build_parameter_golf_baseline_training_graph(
            selected_device.device.clone(),
            &descriptor,
            1,
            sequence_length,
        )?;
        assert!(training_graph.bigram_token_ids_tensor_id.is_some());
        let mut training_session = ParameterGolfCudaTrainingSession::new(
            &mut cuda_backend,
            training_graph,
            &model,
            Some(&banked_weights),
            1,
            sequence_length,
        )?;
        let input_ids = vec![(0..sequence_length)
            .map(|index| (index % 16) as u32)
            .collect::<Vec<_>>()];
        let target_ids = vec![(1..=sequence_length)
            .map(|index| (index % 16) as u32)
            .collect::<Vec<_>>()];

        let (_loss, gradients, _) =
            training_session.execute_batch(&mut cuda_backend, &input_ids, &target_ids)?;
        assert!(!gradients.is_empty());
        Ok(())
    }

    #[test]
    fn score_first_ttt_reuses_training_parameter_buffers_for_chunk_scoring_after_adaptation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cuda_backend = CudaBackend::new();
        let Some(selected_device) = cuda_backend.selected_device().cloned() else {
            return Ok(());
        };

        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let byte_luts = ParameterGolfSentencePieceByteLuts {
            base_bytes_lut: vec![1; 8],
            has_leading_space_lut: vec![false; 8],
            is_boundary_token_lut: vec![false; 8],
        };
        let sequence_length = 16;
        let validation_tokens = (0..97).map(|index| (index % 8) as u16).collect::<Vec<_>>();
        let score_first_ttt = ParameterGolfScoreFirstTttConfig {
            stride: 8,
            chunk_tokens: 32,
            epochs: 1,
            freeze_blocks: 0,
            learning_rate: 0.001,
            momentum: 0.9,
            batch_sequences: 1,
            grad_clip_norm: 1.0,
        };
        let mut eval_graph_cache = BTreeMap::new();
        let mut train_graph_cache = BTreeMap::new();
        let summary = evaluate_score_first_ttt_on_cuda(
            &mut cuda_backend,
            &selected_device.device,
            model.descriptor(),
            &model,
            validation_tokens.as_slice(),
            &byte_luts,
            sequence_length,
            1,
            &ParameterGolfValidationEvalMode::SlidingWindow {
                stride: score_first_ttt.stride,
            },
            &score_first_ttt,
            &mut eval_graph_cache,
            &mut train_graph_cache,
            "score_first_ttt_resident_eval_reuse",
            None,
            ParameterGolfMatrixExecutionMode::DirectBanked,
        )?;
        let receipt = summary
            .score_first_ttt_receipt
            .ok_or("missing score-first TTT receipt")?;
        assert!(receipt.chunk_receipts.len() >= 2);
        let first_runtime = receipt.chunk_receipts[0]
            .score_summary
            .runtime_receipt
            .as_ref()
            .ok_or("missing first chunk runtime receipt")?;
        assert!(first_runtime.resident_parameter_upload_us > 0);
        let later_runtimes = receipt
            .chunk_receipts
            .iter()
            .skip(1)
            .map(|chunk| {
                chunk
                    .score_summary
                    .runtime_receipt
                    .as_ref()
                    .ok_or("missing later chunk runtime receipt")
            })
            .collect::<Result<Vec<_>, _>>()?;
        assert!(!later_runtimes.is_empty());
        assert!(later_runtimes
            .iter()
            .any(|runtime| runtime.resident_parameter_upload_us == 0));
        Ok(())
    }

    #[test]
    fn sliding_window_validation_batch_plans_score_only_suffix_tokens(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let byte_luts = ParameterGolfSentencePieceByteLuts {
            base_bytes_lut: vec![1; 8],
            has_leading_space_lut: vec![false; 8],
            is_boundary_token_lut: vec![false; 8],
        };
        let validation_tokens = vec![0_u16, 1, 2, 3, 4, 5];
        let plans = build_validation_batch_plans(
            validation_tokens.as_slice(),
            &byte_luts,
            4,
            2,
            &ParameterGolfValidationEvalMode::SlidingWindow { stride: 2 },
        )?;

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].evaluation_units, 2);
        assert_eq!(plans[0].token_count, 6);
        assert_eq!(plans[0].byte_count, 6);
        assert_eq!(
            plans[0]
                .sequence_plans
                .iter()
                .map(|plan| (plan.valid_length, plan.score_start))
                .collect::<Vec<_>>(),
            vec![(4, 0), (3, 1)]
        );
        assert_eq!(plans[1].evaluation_units, 1);
        assert_eq!(plans[1].token_count, 1);
        assert_eq!(plans[1].byte_count, 1);
        assert_eq!(
            plans[1]
                .sequence_plans
                .iter()
                .map(|plan| (plan.valid_length, plan.score_start))
                .collect::<Vec<_>>(),
            vec![(1, 0)]
        );
        Ok(())
    }

    #[test]
    fn seed_parameter_states_matches_upstream_precision_split(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let optimizer_plan = parameter_golf_optimizer_plan(model.descriptor(), &hyperparameters)?;
        let trainer_state = seed_parameter_states(&model, &optimizer_plan)?;

        let token_embedding = trainer_state
            .parameter_states
            .get("tok_emb.weight")
            .ok_or("missing tok_emb.weight state")?;
        let token_embedding_source = model
            .weights()
            .parameter_vector(&model.descriptor().config, "tok_emb.weight")
            .ok_or("missing tok_emb.weight source vector")?;
        match token_embedding {
            ParameterGolfParameterState::AdamBf16Master {
                train_visible_values,
                train_visible_bf16_bits,
                master_weight_values,
                ..
            } => {
                for (source, rounded) in token_embedding_source
                    .values
                    .iter()
                    .zip(train_visible_values.iter())
                {
                    assert_eq!(*rounded, bf16::from_f32(*source).to_f32());
                }
                assert_eq!(
                    master_weight_values.as_slice(),
                    token_embedding_source.values
                );
                assert_eq!(
                    train_visible_bf16_bits.as_slice(),
                    bf16_bits_from_f32_values(train_visible_values.as_slice()).as_slice()
                );
            }
            _ => return Err("expected AdamBf16Master token embedding state".into()),
        }

        let matrix = trainer_state
            .parameter_states
            .get("blocks.0.attn.c_q.weight")
            .ok_or("missing matrix state")?;
        assert!(matches!(
            matrix,
            ParameterGolfParameterState::MuonBf16 { .. }
        ));

        let control = trainer_state
            .parameter_states
            .get("blocks.0.attn_scale")
            .ok_or("missing control state")?;
        assert!(matches!(
            control,
            ParameterGolfParameterState::AdamFp32 { .. }
        ));
        let control_source = model
            .weights()
            .parameter_vector(&model.descriptor().config, "blocks.0.attn_scale")
            .ok_or("missing blocks.0.attn_scale source vector")?;
        assert_eq!(control.values(), control_source.values.as_slice());

        Ok(())
    }

    #[test]
    fn seed_parameter_states_and_materialize_current_model_support_banked_optimizer_surface(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let runtime_descriptor = model.banked_descriptor()?;
        let optimizer_plan = parameter_golf_optimizer_plan(&runtime_descriptor, &hyperparameters)?;
        let mut trainer_state = seed_parameter_states(&model, &optimizer_plan)?;

        let qo_bank_state = trainer_state
            .parameter_states
            .get("qo_bank")
            .ok_or("missing qo_bank state")?;
        assert!(matches!(
            qo_bank_state,
            ParameterGolfParameterState::MuonBf16 { .. }
        ));

        let original_qo_bank = model
            .banked_weights()?
            .parameter_vectors(&model.descriptor().config)
            .into_iter()
            .find(|vector| vector.parameter_id == "qo_bank")
            .ok_or("missing qo_bank vector")?;

        let updated_qo_bank_first = {
            let ParameterGolfParameterState::MuonBf16 { values, .. } = trainer_state
                .parameter_states
                .get_mut("qo_bank")
                .ok_or("missing mutable qo_bank state")?
            else {
                return Err("expected MuonBf16 qo_bank state".into());
            };
            values[0] = bf16::from_f32(values[0] + 0.5).to_f32();
            values[0]
        };

        let materialized = materialize_current_model(&model, &trainer_state)?;
        let materialized_qo_bank = materialized
            .banked_weights()?
            .parameter_vectors(&materialized.descriptor().config)
            .into_iter()
            .find(|vector| vector.parameter_id == "qo_bank")
            .ok_or("missing materialized qo_bank vector")?;
        assert_ne!(materialized_qo_bank.values[0], original_qo_bank.values[0]);
        assert_eq!(materialized_qo_bank.values[0], updated_qo_bank_first);
        assert_eq!(
            materialized.weights().blocks[0].attention.q_proj.weight[0],
            updated_qo_bank_first
        );

        Ok(())
    }

    #[test]
    fn flatten_parameter_golf_optimizer_group_values_matches_group_parameter_counts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let optimizer_plan = parameter_golf_optimizer_plan(model.descriptor(), &hyperparameters)?;
        let trainer_state = seed_parameter_states(&model, &optimizer_plan)?;
        let values_by_parameter = trainer_state
            .parameter_states
            .iter()
            .map(|(parameter_id, state)| (parameter_id.clone(), state.values().to_vec()))
            .collect::<BTreeMap<_, _>>();

        let flattened =
            flatten_parameter_golf_optimizer_group_values(&optimizer_plan, &values_by_parameter)?;
        for group in &optimizer_plan.groups {
            let values = flattened
                .get(group.group_id.as_str())
                .ok_or("missing flattened optimizer group")?;
            assert_eq!(values.len(), group.parameter_count);
        }
        Ok(())
    }

    #[test]
    fn flatten_parameter_golf_optimizer_group_buffers_emits_cpu_f32_tensor_buffers(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        let hyperparameters = ParameterGolfTrainingHyperparameters::baseline_defaults();
        let optimizer_plan = parameter_golf_optimizer_plan(model.descriptor(), &hyperparameters)?;
        let trainer_state = seed_parameter_states(&model, &optimizer_plan)?;
        let values_by_parameter = trainer_state
            .parameter_states
            .iter()
            .map(|(parameter_id, state)| (parameter_id.clone(), state.values().to_vec()))
            .collect::<BTreeMap<_, _>>();

        let buffers =
            flatten_parameter_golf_optimizer_group_buffers(&optimizer_plan, &values_by_parameter)?;
        for group in &optimizer_plan.groups {
            let buffer = buffers
                .get(group.group_id.as_str())
                .ok_or("missing flattened optimizer buffer")?;
            assert_eq!(buffer.spec.dtype(), DType::F32);
            assert_eq!(buffer.spec.shape().element_count(), group.parameter_count);
            assert!(matches!(
                &buffer.data,
                TensorData::F32(values) if values.len() == group.parameter_count
            ));
        }
        Ok(())
    }

    #[test]
    fn score_first_ttt_config_parser_accepts_default_and_key_value_labels(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let defaults = ParameterGolfScoreFirstTttConfig::parse("score_first_ttt")?;
        assert_eq!(
            defaults,
            ParameterGolfScoreFirstTttConfig::leaderboard_defaults()
        );

        let parsed = ParameterGolfScoreFirstTttConfig::parse(
            "score_first_ttt:stride=32,chunk_tokens=2048,epochs=2,freeze_blocks=1,learning_rate=0.003,momentum=0.8,batch_sequences=16,grad_clip_norm=0.5",
        )?;
        assert_eq!(parsed.stride, 32);
        assert_eq!(parsed.chunk_tokens, 2_048);
        assert_eq!(parsed.epochs, 2);
        assert_eq!(parsed.freeze_blocks, 1);
        assert!((parsed.learning_rate - 0.003).abs() < f32::EPSILON);
        assert!((parsed.momentum - 0.8).abs() < f32::EPSILON);
        assert_eq!(parsed.batch_sequences, 16);
        assert!((parsed.grad_clip_norm - 0.5).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn score_first_ttt_chunk_plans_assign_windows_by_first_scored_token() {
        let config = ParameterGolfScoreFirstTttConfig {
            stride: 4,
            chunk_tokens: 8,
            epochs: 1,
            freeze_blocks: 0,
            learning_rate: 0.001,
            momentum: 0.9,
            batch_sequences: 2,
            grad_clip_norm: 1.0,
        };
        let plans = build_parameter_golf_score_first_ttt_chunk_plans(20, 8, &config);
        assert_eq!(plans.len(), 3);
        assert_eq!(plans[0].receipt_plan.chunk_start_token, 0);
        assert_eq!(plans[0].window_starts, vec![0]);
        assert_eq!(plans[1].receipt_plan.chunk_start_token, 8);
        assert_eq!(plans[1].window_starts, vec![4, 8]);
        assert_eq!(plans[2].receipt_plan.chunk_start_token, 16);
        assert_eq!(plans[2].window_starts, vec![12, 16]);
    }

    #[test]
    fn score_first_ttt_freeze_policy_only_freezes_requested_block_prefixes() {
        assert!(parameter_golf_score_first_ttt_parameter_is_frozen(
            "blocks.0.attn.c_q.weight",
            1
        ));
        assert!(parameter_golf_score_first_ttt_parameter_is_frozen(
            "blocks.1.mlp.fc.weight",
            2
        ));
        assert!(!parameter_golf_score_first_ttt_parameter_is_frozen(
            "blocks.2.attn.c_q.weight",
            2
        ));
        assert!(!parameter_golf_score_first_ttt_parameter_is_frozen(
            "tok_emb.weight",
            4
        ));
    }

    #[test]
    fn validation_runtime_receipt_defaults_total_units_for_older_reports(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let receipt: ParameterGolfSingleH100ValidationRuntimeReceipt = serde_json::from_str(
            r#"{
                "path": "device_resident_cuda_eval_graph_v1",
                "graph_surface": "parameter_golf_baseline_eval_graph_v1",
                "session_count": 2,
                "total_batches": 947,
                "persistent_parameter_buffer_count": 184,
                "persistent_parameter_value_count": 34119824,
                "resident_parameter_upload_us": 60515,
                "per_batch_stable_parameter_buffer_allocations": 0,
                "reusable_input_token_buffer": true,
                "reusable_target_token_buffer": true,
                "total_input_token_write_us": 213141,
                "total_target_token_write_us": 257596,
                "byte_accounting_mode": "precomputed_batch_target_bytes",
                "total_byte_accounting_us": 383945
            }"#,
        )?;
        assert_eq!(receipt.total_units, 0);
        assert_eq!(receipt.total_batches, 947);
        Ok(())
    }

    #[test]
    fn competitive_homegolf_defaults_enable_competitive_surfaces() {
        let config =
            ParameterGolfSingleH100TrainingConfig::challenge_competitive_homegolf_v1_defaults(
                PathBuf::from("."),
                PathBuf::from("tokenizer.model"),
            );
        assert_eq!(
            config.model_variant,
            ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1
        );
        assert_eq!(
            config.model_config,
            ParameterGolfConfig::competitive_homegolf_v1()
        );
        assert_eq!(
            config.validation_eval_mode,
            ParameterGolfValidationEvalMode::SlidingWindow { stride: 64 }
        );
        assert!(config.score_first_ttt.is_some());
        assert!(config.ema.is_some());
        assert!(config.swa.is_some());
        assert_eq!(
            config.final_model_surface,
            ParameterGolfFinalModelSurface::Swa
        );
        assert_eq!(
            config.final_artifact_config,
            ParameterGolfFinalArtifactConfig::competitive_defaults()
        );
    }

    #[test]
    fn homegolf_local_cuda_rejects_non_sequence_exact_grad_accum() {
        let mut config =
            ParameterGolfSingleH100TrainingConfig::challenge_homegolf_local_cuda_defaults(
                PathBuf::from("."),
                PathBuf::from("tokenizer.model"),
            );
        config.geometry.grad_accum_steps = 24;
        let error = config
            .validate()
            .expect_err("non-sequence-exact grad_accum should refuse");
        assert!(matches!(
            error,
            ParameterGolfSingleH100TrainingError::InvalidConfig { message }
                if message.contains("invalid batch geometry")
        ));
    }

    #[test]
    fn homegolf_local_cuda_defaults_drop_warmup_but_keep_wallclock_cap() {
        let config = ParameterGolfSingleH100TrainingConfig::challenge_homegolf_local_cuda_defaults(
            PathBuf::from("."),
            PathBuf::from("tokenizer.model"),
        );
        assert_eq!(
            config.machine_profile,
            ParameterGolfSingleDeviceMachineProfile::HomegolfLocalCuda
        );
        assert_eq!(config.warmup_steps, 0);
        assert_eq!(config.hyperparameters.max_wallclock_seconds, Some(600.0));
    }

    #[test]
    fn model_variant_parse_accepts_competitive_label() {
        assert_eq!(
            ParameterGolfSingleH100ModelVariant::parse("competitive_homegolf_v1")
                .expect("competitive label should parse"),
            ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1
        );
    }

    #[test]
    fn project_full_step_wallclock_ms_rounds_up_from_partial_microstep_progress() {
        assert_eq!(project_full_step_wallclock_ms(28_500, 1, 64), 1_824_000);
        assert_eq!(project_full_step_wallclock_ms(350_000, 12, 64), 1_866_667);
        assert_eq!(project_full_step_wallclock_ms(278_709, 64, 64), 278_709);
    }

    #[test]
    fn final_model_surface_parse_accepts_supported_labels() {
        assert_eq!(
            ParameterGolfFinalModelSurface::parse("raw").expect("raw should parse"),
            ParameterGolfFinalModelSurface::Raw
        );
        assert_eq!(
            ParameterGolfFinalModelSurface::parse("ema").expect("ema should parse"),
            ParameterGolfFinalModelSurface::Ema
        );
        assert_eq!(
            ParameterGolfFinalModelSurface::parse("swa").expect("swa should parse"),
            ParameterGolfFinalModelSurface::Swa
        );
    }
}

fn unique_tensor_ids(ids: Vec<TensorId>) -> Vec<TensorId> {
    let mut seen = std::collections::BTreeSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

fn project_full_step_wallclock_ms(
    observed_step_wallclock_ms: u64,
    observed_micro_steps: usize,
    total_micro_steps: usize,
) -> u64 {
    if observed_micro_steps == 0 || total_micro_steps == 0 {
        return 0;
    }
    let numerator = u128::from(observed_step_wallclock_ms)
        .saturating_mul(total_micro_steps as u128)
        .saturating_add(observed_micro_steps.saturating_sub(1) as u128);
    (numerator / observed_micro_steps as u128) as u64
}

fn duration_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

fn duration_us(started: Instant) -> u64 {
    started.elapsed().as_micros() as u64
}

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis() as u64
}

fn emit_progress_line(message: String) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{message}");
    let _ = stdout.flush();
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("digest serialization should succeed"));
    hex::encode(hasher.finalize())
}

fn report_without_digest(
    report: &ParameterGolfSingleH100TrainingReport,
) -> ParameterGolfSingleH100TrainingReport {
    let mut canonical = report.clone();
    canonical.report_digest.clear();
    canonical
}

fn receipt_without_digest(
    receipt: &ParameterGolfSingleH100ValidationRuntimeComparisonReceipt,
) -> ParameterGolfSingleH100ValidationRuntimeComparisonReceipt {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    canonical
}
