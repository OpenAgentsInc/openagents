use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact,
};
use psionic_core::QuantizationMode;
use psionic_models::{
    QWEN36_27B_MODEL_ID, QWEN36_27B_REAL_MODEL_DIR, QWEN36_27B_SERVED_MODEL_ID,
    Qwen36FullForwardTrainingSurface, Qwen36PromptReceipt, Qwen36SampledLogit,
    Qwen36SampledProjectionTrainingSurface, qwen36_full_forward_training_surface,
    qwen36_sampled_projection_training_surface,
};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors, serialize, tensor::TensorView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN36_REAL_LORA_SFT_CONFIG_SCHEMA_VERSION: &str =
    "psionic.qwen36_real_lora_sft_config.v1";
pub const QWEN36_REAL_LORA_SFT_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen36_real_lora_sft_receipt.v1";
pub const QWEN36_REAL_LORA_SFT_LOSS_CURVE_SCHEMA_VERSION: &str =
    "psionic.qwen36_real_lora_sft_loss_curve.v1";
pub const QWEN36_REAL_LORA_SFT_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.qwen36_real_lora_sft_checkpoint.v1";
pub const QWEN36_REAL_LORA_STATE_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.qwen36_real_lora_state_checkpoint.v1";
pub const QWEN36_REAL_LORA_ACTIVE_TARGET: &str = "lm_head.weight";
pub const QWEN36_REAL_LORA_ADAPTER_FORMAT: &str = "lm_head_lora_safetensors";
pub const QWEN36_REAL_LORA_ACTIVATION_MODE: &str = "sampled_embed_lm_head_projection_v1";
pub const QWEN36_REAL_LORA_FULL_LAYER_ACTIVATION_MODE: &str =
    "bounded_full_layer_row_sparse_forward_v1";
pub const QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER: &str = "full_forward_smoke";
pub const QWEN36_REAL_LORA_ACTIVATION_SOURCE_SAMPLED: &str = "sampled_projection";
pub const QWEN36_DENSE_REAL_LORA_TARGET_MODULES: &[&str] = &[
    "q_proj",
    "k_proj",
    "v_proj",
    "o_proj",
    "gate_proj",
    "up_proj",
    "down_proj",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraSftConfig {
    pub schema_version: String,
    pub run_id: String,
    #[serde(default = "default_real_qwen_model_dir")]
    pub model_dir: String,
    #[serde(default = "default_real_train_type")]
    pub train_type: String,
    #[serde(default = "default_activation_source")]
    pub activation_source: String,
    #[serde(default = "default_adapter_id")]
    pub adapter_id: String,
    #[serde(default = "default_adapter_revision")]
    pub adapter_revision: String,
    pub prompt: String,
    pub target_token_id: u32,
    #[serde(default)]
    pub candidate_token_ids: Vec<u32>,
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,
    #[serde(default)]
    pub resume_from_checkpoint_path: Option<String>,
    #[serde(default = "default_lora_rank")]
    pub lora_rank: usize,
    #[serde(default = "default_lora_alpha")]
    pub lora_alpha: f32,
    #[serde(default = "default_learning_rate")]
    pub learning_rate: f32,
    #[serde(default)]
    pub weight_decay: f32,
    #[serde(default = "default_beta1")]
    pub beta1: f32,
    #[serde(default = "default_beta2")]
    pub beta2: f32,
    #[serde(default = "default_epsilon")]
    pub epsilon: f32,
    #[serde(default = "default_max_steps")]
    pub max_steps: u64,
    #[serde(default)]
    pub gradient_clip_norm: Option<f32>,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

impl Default for Qwen36RealLoraSftConfig {
    fn default() -> Self {
        Self {
            schema_version: String::from(QWEN36_REAL_LORA_SFT_CONFIG_SCHEMA_VERSION),
            run_id: String::from("qwen36-27b-real-lora-sft-full-layer-smoke-001"),
            model_dir: default_real_qwen_model_dir(),
            train_type: default_real_train_type(),
            activation_source: default_activation_source(),
            adapter_id: default_adapter_id(),
            adapter_revision: default_adapter_revision(),
            prompt: String::from(
                "Draft a concise legal work product checklist for reviewing a vendor services agreement.",
            ),
            target_token_id: 271,
            candidate_token_ids: vec![0, 1, 2, 3, 4, 5, 271],
            target_modules: default_target_modules(),
            resume_from_checkpoint_path: None,
            lora_rank: default_lora_rank(),
            lora_alpha: default_lora_alpha(),
            learning_rate: default_learning_rate(),
            weight_decay: 0.0,
            beta1: default_beta1(),
            beta2: default_beta2(),
            epsilon: default_epsilon(),
            max_steps: default_max_steps(),
            gradient_clip_norm: Some(1.0),
            output_dir: default_output_dir(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealModelHashes {
    pub config_sha256: String,
    pub tokenizer_sha256: String,
    pub index_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraOptimizerReceipt {
    pub optimizer: String,
    pub learning_rate: f32,
    pub weight_decay: f32,
    pub beta1: f32,
    pub beta2: f32,
    pub epsilon: f32,
    pub gradient_clip_norm: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraQloraMetadata {
    pub enabled: bool,
    pub base_weight_quantization: String,
    pub adapter_weight_dtype: String,
    pub compatibility_note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraActivationReceipt {
    pub activation_source: String,
    pub activation_mode: String,
    pub exact_full_width_logits: bool,
    pub layer_receipt_count: usize,
    pub full_attention_layer_count: usize,
    pub linear_attention_layer_count: usize,
    pub mtp_layer_count: usize,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraLossPoint {
    pub step: u64,
    pub loss: f32,
    pub target_probability: f32,
    pub logits_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraLossCurve {
    pub schema_version: String,
    pub run_id: String,
    pub points: Vec<Qwen36RealLoraLossPoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraCheckpointSummary {
    pub schema_version: String,
    pub run_id: String,
    pub step: u64,
    pub adapter_artifact_path: String,
    pub adapter_artifact_sha256: String,
    pub state_checkpoint_path: String,
    pub state_checkpoint_sha256: String,
    pub optimizer_state_sha256: String,
    pub scheduler_cursor_state_sha256: String,
    pub state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealLoraSftReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub base_model: String,
    pub served_model_id: String,
    pub model_dir: String,
    pub model_hashes: Qwen36RealModelHashes,
    pub train_type: String,
    pub qlora: Qwen36RealLoraQloraMetadata,
    pub activation_mode: String,
    pub activation_receipt: Qwen36RealLoraActivationReceipt,
    pub prompt_receipt: Qwen36PromptReceipt,
    pub target_token_id: u32,
    pub candidate_token_ids: Vec<u32>,
    pub base_sampled_logits: Vec<Qwen36SampledLogit>,
    pub base_logits_sha256: String,
    pub hidden_state_sha256: String,
    pub active_trainable_target: String,
    pub adapter_format: String,
    pub dense_lora_target_modules: Vec<String>,
    pub requested_target_modules: Vec<String>,
    pub resolved_target_modules: Vec<String>,
    pub deferred_target_modules_reason: String,
    pub lora_rank: usize,
    pub lora_alpha: f32,
    pub trainable_parameter_count: u64,
    pub frozen_base_weights: bool,
    pub optimizer: Qwen36RealLoraOptimizerReceipt,
    pub completed_steps: u64,
    pub initial_loss: f32,
    pub final_loss: f32,
    pub loss_improved: bool,
    pub adapter_artifact_path: String,
    pub adapter_artifact_sha256: String,
    pub adapter_identity_digest: String,
    pub loss_curve_path: String,
    pub checkpoint_summary_path: String,
    pub state_checkpoint_path: String,
    pub state_checkpoint_sha256: String,
    pub dpo_grpo_adapter_compatibility: String,
    pub python_invoked: bool,
    pub claim_boundary: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen36RealLoraSftArtifacts {
    pub adapter_path: String,
    pub receipt_path: String,
    pub loss_curve_path: String,
    pub checkpoint_summary_path: String,
    pub state_checkpoint_path: String,
    pub receipt: Qwen36RealLoraSftReceipt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Qwen36RealLoraState {
    step: u64,
    lora_a: Vec<f32>,
    lora_b: Vec<f32>,
    adam_m_a: Vec<f32>,
    adam_v_a: Vec<f32>,
    adam_m_b: Vec<f32>,
    adam_v_b: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
struct Qwen36RealLoraStepOutput {
    loss: f32,
    target_probability: f32,
    logits_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Qwen36RealLoraActivationSurface {
    prompt_receipt: Qwen36PromptReceipt,
    candidate_token_ids: Vec<u32>,
    hidden_state: Vec<f32>,
    hidden_state_sha256: String,
    base_sampled_logits: Vec<Qwen36SampledLogit>,
    logits_sha256: String,
    tensor_reads: Vec<psionic_models::Qwen36TensorRowReadReceipt>,
    activation_receipt: Qwen36RealLoraActivationReceipt,
}

#[derive(Debug, Error)]
pub enum Qwen36RealLoraSftError {
    #[error("invalid Qwen3.6 real LoRA SFT config: {0}")]
    InvalidConfig(String),
    #[error("Qwen3.6 real LoRA SFT model error: {0}")]
    Model(#[from] psionic_models::Qwen36TargetPathError),
    #[error("Qwen3.6 real LoRA SFT I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("Qwen3.6 real LoRA SFT JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Qwen3.6 real LoRA SFT safetensors failed: {0}")]
    Safetensors(String),
    #[error("Qwen3.6 real LoRA adapter failed to reload: {0}")]
    Adapter(#[from] psionic_adapters::LmHeadLoraLoadError),
}

pub fn run_qwen36_real_lora_sft_default()
-> Result<Qwen36RealLoraSftArtifacts, Qwen36RealLoraSftError> {
    run_qwen36_real_lora_sft(&Qwen36RealLoraSftConfig::default())
}

pub fn run_qwen36_real_lora_sft_config_path(
    path: impl AsRef<Path>,
) -> Result<Qwen36RealLoraSftArtifacts, Qwen36RealLoraSftError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| Qwen36RealLoraSftError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let config = serde_json::from_slice::<Qwen36RealLoraSftConfig>(bytes.as_slice())?;
    run_qwen36_real_lora_sft(&config)
}

pub fn run_qwen36_real_lora_sft(
    config: &Qwen36RealLoraSftConfig,
) -> Result<Qwen36RealLoraSftArtifacts, Qwen36RealLoraSftError> {
    validate_config(config)?;
    let mut candidate_token_ids = config.candidate_token_ids.clone();
    candidate_token_ids.push(config.target_token_id);
    candidate_token_ids.sort_unstable();
    candidate_token_ids.dedup();
    let surface = activation_surface(
        &config.model_dir,
        config.prompt.as_str(),
        candidate_token_ids.as_slice(),
        config.activation_source.as_str(),
    )?;
    let resolved_target_modules = resolve_target_modules(config.target_modules.as_slice())?;
    let (vocab_size, hidden_size) = surface_shape(&surface)?;
    if config.target_token_id as usize >= vocab_size {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "target_token_id {} is outside vocab size {vocab_size}",
            config.target_token_id
        )));
    }
    let output_dir = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_dir).map_err(|error| Qwen36RealLoraSftError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;

    let model_hashes = model_hashes(Path::new(&config.model_dir))?;
    let mut state = if let Some(path) = &config.resume_from_checkpoint_path {
        load_state_checkpoint(Path::new(path), config, vocab_size, hidden_size)?
    } else {
        Qwen36RealLoraState::new(config, vocab_size, hidden_size)
    };
    let mut loss_points = Vec::new();
    for _ in state.step..config.max_steps {
        let point = train_one_step(config, &surface, vocab_size, hidden_size, &mut state)?;
        loss_points.push(Qwen36RealLoraLossPoint {
            step: state.step,
            loss: point.loss,
            target_probability: point.target_probability,
            logits_sha256: point.logits_sha256,
        });
    }
    let adapter_bytes = export_lm_head_lora_safetensors(
        config,
        hidden_size,
        vocab_size,
        &state.lora_a,
        &state.lora_b,
    )?;
    let adapter_artifact_sha256 = sha256_hex(adapter_bytes.as_slice());
    let adapter_path = output_dir.join("adapter.safetensors");
    fs::write(&adapter_path, adapter_bytes.as_slice()).map_err(|error| {
        Qwen36RealLoraSftError::Io {
            path: adapter_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let adapter_identity =
        adapter_identity(config, &adapter_artifact_sha256, hidden_size, vocab_size);
    let adapter_identity_digest = adapter_identity.stable_digest();
    LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        adapter_bytes.as_slice(),
        adapter_identity,
        config.lora_alpha,
    )?;

    let loss_curve = Qwen36RealLoraLossCurve {
        schema_version: String::from(QWEN36_REAL_LORA_SFT_LOSS_CURVE_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        points: loss_points,
    };
    let loss_curve_path = output_dir.join("loss_curve.json");
    write_json(&loss_curve_path, &loss_curve)?;
    let state_checkpoint_path = output_dir.join("state_checkpoint.safetensors");
    let state_checkpoint_bytes =
        export_state_checkpoint_safetensors(config, hidden_size, vocab_size, &state)?;
    fs::write(&state_checkpoint_path, state_checkpoint_bytes.as_slice()).map_err(|error| {
        Qwen36RealLoraSftError::Io {
            path: state_checkpoint_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let state_checkpoint_sha256 = sha256_hex(state_checkpoint_bytes.as_slice());
    let checkpoint = checkpoint_summary(
        config,
        &state,
        adapter_path.as_path(),
        adapter_artifact_sha256.as_str(),
        state_checkpoint_path.as_path(),
        state_checkpoint_sha256.as_str(),
    );
    let checkpoint_summary_path = output_dir.join("checkpoint_summary.json");
    write_json(&checkpoint_summary_path, &checkpoint)?;

    let initial_loss = loss_curve
        .points
        .first()
        .map(|point| point.loss)
        .unwrap_or_default();
    let final_loss = loss_curve
        .points
        .last()
        .map(|point| point.loss)
        .unwrap_or_default();
    let mut receipt = Qwen36RealLoraSftReceipt {
        schema_version: String::from(QWEN36_REAL_LORA_SFT_RECEIPT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        base_model: String::from(QWEN36_27B_MODEL_ID),
        served_model_id: String::from(QWEN36_27B_SERVED_MODEL_ID),
        model_dir: config.model_dir.clone(),
        model_hashes,
        train_type: config.train_type.clone(),
        qlora: qlora_metadata(config),
        activation_mode: surface.activation_receipt.activation_mode.clone(),
        activation_receipt: surface.activation_receipt.clone(),
        prompt_receipt: surface.prompt_receipt.clone(),
        target_token_id: config.target_token_id,
        candidate_token_ids: surface.candidate_token_ids.clone(),
        base_sampled_logits: surface.base_sampled_logits.clone(),
        base_logits_sha256: surface.logits_sha256.clone(),
        hidden_state_sha256: surface.hidden_state_sha256.clone(),
        active_trainable_target: String::from(QWEN36_REAL_LORA_ACTIVE_TARGET),
        adapter_format: String::from(QWEN36_REAL_LORA_ADAPTER_FORMAT),
        dense_lora_target_modules: QWEN36_DENSE_REAL_LORA_TARGET_MODULES
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        requested_target_modules: config.target_modules.clone(),
        resolved_target_modules,
        deferred_target_modules_reason: String::from(
            "The dense attention/MLP target set is now validated and the SFT surface comes from the bounded Rust full-layer activation path. The exported adapter remains LM-head LoRA because serving, DPO, and GRPO currently load that artifact shape; exact per-q_proj/k_proj/v_proj/o_proj/gate/up/down backward kernels remain the next implementation step.",
        ),
        lora_rank: config.lora_rank,
        lora_alpha: config.lora_alpha,
        trainable_parameter_count: trainable_parameter_count(
            config.lora_rank,
            hidden_size,
            vocab_size,
        ),
        frozen_base_weights: true,
        optimizer: optimizer_receipt(config),
        completed_steps: state.step,
        initial_loss,
        final_loss,
        loss_improved: final_loss < initial_loss,
        adapter_artifact_path: adapter_path.display().to_string(),
        adapter_artifact_sha256,
        adapter_identity_digest,
        loss_curve_path: loss_curve_path.display().to_string(),
        checkpoint_summary_path: checkpoint_summary_path.display().to_string(),
        state_checkpoint_path: state_checkpoint_path.display().to_string(),
        state_checkpoint_sha256,
        dpo_grpo_adapter_compatibility: String::from(
            "The artifact exports lm_head.lora_A.weight and lm_head.lora_B.weight in the same safetensors shape consumed by the existing legal DPO and GRPO parent-adapter paths.",
        ),
        python_invoked: false,
        claim_boundary: String::from(
            "This is a real Qwen3.6-27B bounded full-layer LoRA update: the hidden vector comes from the downloaded BF16 safetensors after the Rust row-sparse layer-stack smoke. It keeps all base weights frozen and trains a loadable LM-head LoRA adapter. It is not exact full-width transformer backprop through q_proj, k_proj, v_proj, o_proj, gate_proj, up_proj, or down_proj yet.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_json_digest(b"qwen36_real_lora_sft_receipt|", &receipt);
    let receipt_path = output_dir.join("training_receipt.json");
    write_json(&receipt_path, &receipt)?;

    Ok(Qwen36RealLoraSftArtifacts {
        adapter_path: adapter_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        loss_curve_path: loss_curve_path.display().to_string(),
        checkpoint_summary_path: checkpoint_summary_path.display().to_string(),
        state_checkpoint_path: state_checkpoint_path.display().to_string(),
        receipt,
    })
}

impl Qwen36RealLoraState {
    fn new(config: &Qwen36RealLoraSftConfig, vocab_size: usize, hidden_size: usize) -> Self {
        let lora_a = seeded_matrix(
            format!("{}|{}|lora_a", config.run_id, config.adapter_id).as_str(),
            config.lora_rank,
            hidden_size,
            0.002,
        );
        let lora_b = seeded_matrix(
            format!("{}|{}|lora_b", config.run_id, config.adapter_id).as_str(),
            vocab_size,
            config.lora_rank,
            0.002,
        );
        Self {
            step: 0,
            adam_m_a: vec![0.0; lora_a.len()],
            adam_v_a: vec![0.0; lora_a.len()],
            adam_m_b: vec![0.0; lora_b.len()],
            adam_v_b: vec![0.0; lora_b.len()],
            lora_a,
            lora_b,
        }
    }
}

fn activation_surface(
    model_dir: &str,
    prompt: &str,
    candidate_token_ids: &[u32],
    activation_source: &str,
) -> Result<Qwen36RealLoraActivationSurface, Qwen36RealLoraSftError> {
    match activation_source {
        QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER => {
            let surface =
                qwen36_full_forward_training_surface(model_dir, prompt, candidate_token_ids)?;
            Ok(Qwen36RealLoraActivationSurface::from(surface))
        }
        QWEN36_REAL_LORA_ACTIVATION_SOURCE_SAMPLED => {
            let surface =
                qwen36_sampled_projection_training_surface(model_dir, prompt, candidate_token_ids)?;
            Ok(Qwen36RealLoraActivationSurface::from(surface))
        }
        other => Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "unsupported activation_source `{other}`"
        ))),
    }
}

impl From<Qwen36FullForwardTrainingSurface> for Qwen36RealLoraActivationSurface {
    fn from(surface: Qwen36FullForwardTrainingSurface) -> Self {
        let full_attention_layer_count = surface
            .layer_receipts
            .iter()
            .filter(|receipt| receipt.layer_type == "full_attention")
            .count();
        let linear_attention_layer_count = surface
            .layer_receipts
            .iter()
            .filter(|receipt| receipt.layer_type == "linear_attention")
            .count();
        let mtp_layer_count = surface
            .layer_receipts
            .iter()
            .filter(|receipt| receipt.layer_type == "mtp_full_attention")
            .count();
        Self {
            prompt_receipt: surface.prompt_receipt,
            candidate_token_ids: surface.candidate_token_ids,
            hidden_state: surface.hidden_state,
            hidden_state_sha256: surface.hidden_state_sha256,
            base_sampled_logits: surface.base_sampled_logits,
            logits_sha256: surface.logits_sha256,
            tensor_reads: surface.tensor_reads,
            activation_receipt: Qwen36RealLoraActivationReceipt {
                activation_source: String::from(QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER),
                activation_mode: surface.activation_mode,
                exact_full_width_logits: surface.exact_full_width_logits,
                layer_receipt_count: surface.layer_receipts.len(),
                full_attention_layer_count,
                linear_attention_layer_count,
                mtp_layer_count,
                claim_boundary: surface.claim_boundary,
            },
        }
    }
}

impl From<Qwen36SampledProjectionTrainingSurface> for Qwen36RealLoraActivationSurface {
    fn from(surface: Qwen36SampledProjectionTrainingSurface) -> Self {
        Self {
            prompt_receipt: surface.prompt_receipt,
            candidate_token_ids: surface.candidate_token_ids,
            hidden_state: surface.hidden_state,
            hidden_state_sha256: surface.hidden_state_sha256,
            base_sampled_logits: surface.base_sampled_logits,
            logits_sha256: surface.logits_sha256,
            tensor_reads: surface.tensor_reads,
            activation_receipt: Qwen36RealLoraActivationReceipt {
                activation_source: String::from(QWEN36_REAL_LORA_ACTIVATION_SOURCE_SAMPLED),
                activation_mode: String::from(QWEN36_REAL_LORA_ACTIVATION_MODE),
                exact_full_width_logits: false,
                layer_receipt_count: 0,
                full_attention_layer_count: 0,
                linear_attention_layer_count: 0,
                mtp_layer_count: 0,
                claim_boundary: String::from(
                    "This fallback activation surface uses the embedding row directly and does not execute the Qwen3.6 layer stack.",
                ),
            },
        }
    }
}

fn train_one_step(
    config: &Qwen36RealLoraSftConfig,
    surface: &Qwen36RealLoraActivationSurface,
    vocab_size: usize,
    hidden_size: usize,
    state: &mut Qwen36RealLoraState,
) -> Result<Qwen36RealLoraStepOutput, Qwen36RealLoraSftError> {
    let target_position = surface
        .candidate_token_ids
        .iter()
        .position(|token| *token == config.target_token_id)
        .ok_or_else(|| {
            Qwen36RealLoraSftError::InvalidConfig(format!(
                "target token {} is missing from sampled candidate set",
                config.target_token_id
            ))
        })?;
    let scale = config.lora_alpha / config.lora_rank.max(1) as f32;
    let intermediate = lora_intermediate(
        &state.lora_a,
        &surface.hidden_state,
        config.lora_rank,
        hidden_size,
    );
    let mut logits = Vec::with_capacity(surface.candidate_token_ids.len());
    for (index, token_id) in surface.candidate_token_ids.iter().enumerate() {
        let base = surface.base_sampled_logits[index].logit as f32;
        let row_start = *token_id as usize * config.lora_rank;
        let adapter = dot(
            &state.lora_b[row_start..row_start + config.lora_rank],
            intermediate.as_slice(),
        ) * scale;
        logits.push(base + adapter);
    }
    let probabilities = softmax(logits.as_slice());
    let target_probability = probabilities[target_position].max(1e-30);
    let loss = -target_probability.ln();
    let logits_sha256 = sha256_f32_values(logits.as_slice());
    let mut grad_logits = probabilities;
    grad_logits[target_position] -= 1.0;
    let mut grad_a = vec![0.0_f32; state.lora_a.len()];
    let mut grad_b = vec![0.0_f32; state.lora_b.len()];
    for (candidate_index, token_id) in surface.candidate_token_ids.iter().enumerate() {
        let grad = grad_logits[candidate_index];
        let row_start = *token_id as usize * config.lora_rank;
        for rank_index in 0..config.lora_rank {
            grad_b[row_start + rank_index] += grad * intermediate[rank_index] * scale;
            let b_value = state.lora_b[row_start + rank_index];
            let a_row_start = rank_index * hidden_size;
            for hidden_index in 0..hidden_size {
                grad_a[a_row_start + hidden_index] +=
                    grad * b_value * surface.hidden_state[hidden_index] * scale;
            }
        }
    }
    if let Some(max_norm) = config.gradient_clip_norm {
        clip_gradients(grad_a.as_mut_slice(), grad_b.as_mut_slice(), max_norm);
    }
    state.step += 1;
    adamw_update(
        state.lora_a.as_mut_slice(),
        state.adam_m_a.as_mut_slice(),
        state.adam_v_a.as_mut_slice(),
        grad_a.as_slice(),
        config,
        state.step,
    );
    adamw_update(
        state.lora_b.as_mut_slice(),
        state.adam_m_b.as_mut_slice(),
        state.adam_v_b.as_mut_slice(),
        grad_b.as_slice(),
        config,
        state.step,
    );
    debug_assert_eq!(state.lora_b.len(), vocab_size * config.lora_rank);
    Ok(Qwen36RealLoraStepOutput {
        loss,
        target_probability,
        logits_sha256,
    })
}

fn validate_config(config: &Qwen36RealLoraSftConfig) -> Result<(), Qwen36RealLoraSftError> {
    if config.schema_version != QWEN36_REAL_LORA_SFT_CONFIG_SCHEMA_VERSION {
        return Err(Qwen36RealLoraSftError::InvalidConfig(String::from(
            "schema_version must be psionic.qwen36_real_lora_sft_config.v1",
        )));
    }
    if config.run_id.trim().is_empty()
        || config.model_dir.trim().is_empty()
        || config.adapter_id.trim().is_empty()
        || config.adapter_revision.trim().is_empty()
        || config.prompt.trim().is_empty()
        || config.output_dir.trim().is_empty()
    {
        return Err(Qwen36RealLoraSftError::InvalidConfig(String::from(
            "run_id, model_dir, adapter_id, adapter_revision, prompt, and output_dir must be non-empty",
        )));
    }
    if !matches!(config.train_type.as_str(), "lora" | "qlora") {
        return Err(Qwen36RealLoraSftError::InvalidConfig(String::from(
            "train_type must be `lora` or `qlora`",
        )));
    }
    if !matches!(
        config.activation_source.as_str(),
        QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER | QWEN36_REAL_LORA_ACTIVATION_SOURCE_SAMPLED
    ) {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "activation_source must be `{QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER}` or `{QWEN36_REAL_LORA_ACTIVATION_SOURCE_SAMPLED}`",
        )));
    }
    resolve_target_modules(config.target_modules.as_slice())?;
    if config.lora_rank == 0
        || !config.lora_alpha.is_finite()
        || config.lora_alpha <= 0.0
        || !config.learning_rate.is_finite()
        || config.learning_rate <= 0.0
        || !config.weight_decay.is_finite()
        || config.weight_decay < 0.0
        || !(0.0..1.0).contains(&config.beta1)
        || !(0.0..1.0).contains(&config.beta2)
        || !config.epsilon.is_finite()
        || config.epsilon <= 0.0
        || config.max_steps == 0
    {
        return Err(Qwen36RealLoraSftError::InvalidConfig(String::from(
            "rank, alpha, AdamW hyperparameters, and max_steps must be valid",
        )));
    }
    if config
        .gradient_clip_norm
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        return Err(Qwen36RealLoraSftError::InvalidConfig(String::from(
            "gradient_clip_norm must be positive when set",
        )));
    }
    Ok(())
}

fn resolve_target_modules(
    target_modules: &[String],
) -> Result<Vec<String>, Qwen36RealLoraSftError> {
    if target_modules.is_empty() || target_modules.iter().any(|value| value == "all-linear") {
        return Ok(QWEN36_DENSE_REAL_LORA_TARGET_MODULES
            .iter()
            .map(|value| String::from(*value))
            .collect());
    }
    let mut resolved = Vec::new();
    for module in target_modules {
        if module == "router" || module == "experts" || module.contains("moe") {
            return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
                "Qwen3.6-27B legal SFT refuses router/MoE target `{module}`; the dense 27B path supports attention and MLP linear modules only",
            )));
        }
        if !QWEN36_DENSE_REAL_LORA_TARGET_MODULES.contains(&module.as_str()) {
            return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
                "unsupported Qwen3.6 dense LoRA target `{module}`"
            )));
        }
        if !resolved.contains(module) {
            resolved.push(module.clone());
        }
    }
    Ok(resolved)
}

fn surface_shape(
    surface: &Qwen36RealLoraActivationSurface,
) -> Result<(usize, usize), Qwen36RealLoraSftError> {
    let read = surface.tensor_reads.first().ok_or_else(|| {
        Qwen36RealLoraSftError::InvalidConfig(String::from("missing tensor reads"))
    })?;
    let [vocab_size, hidden_size] = read.shape.as_slice() else {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "expected 2D embedding shape, found {:?}",
            read.shape
        )));
    };
    if surface.hidden_state.len() != *hidden_size {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "hidden state width {} did not match tensor width {hidden_size}",
            surface.hidden_state.len()
        )));
    }
    Ok((*vocab_size, *hidden_size))
}

fn lora_intermediate(lora_a: &[f32], hidden: &[f32], rank: usize, hidden_size: usize) -> Vec<f32> {
    let mut values = vec![0.0; rank];
    for rank_index in 0..rank {
        let row = &lora_a[rank_index * hidden_size..(rank_index + 1) * hidden_size];
        values[rank_index] = dot(row, hidden);
    }
    values
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, |left, right| left.max(right));
    let mut total = 0.0_f32;
    let mut values = Vec::with_capacity(logits.len());
    for logit in logits {
        let value = (*logit - max).exp();
        total += value;
        values.push(value);
    }
    for value in &mut values {
        *value /= total.max(1e-30);
    }
    values
}

fn clip_gradients(grad_a: &mut [f32], grad_b: &mut [f32], max_norm: f32) {
    let norm = grad_a
        .iter()
        .chain(grad_b.iter())
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm > max_norm {
        let scale = max_norm / norm;
        for value in grad_a.iter_mut().chain(grad_b.iter_mut()) {
            *value *= scale;
        }
    }
}

fn adamw_update(
    values: &mut [f32],
    moments: &mut [f32],
    velocities: &mut [f32],
    gradients: &[f32],
    config: &Qwen36RealLoraSftConfig,
    step: u64,
) {
    let step_i32 = i32::try_from(step).unwrap_or(i32::MAX);
    let bias1 = 1.0 - config.beta1.powi(step_i32);
    let bias2 = 1.0 - config.beta2.powi(step_i32);
    for (((value, moment), velocity), gradient) in values
        .iter_mut()
        .zip(moments.iter_mut())
        .zip(velocities.iter_mut())
        .zip(gradients.iter())
    {
        *moment = config.beta1 * *moment + (1.0 - config.beta1) * *gradient;
        *velocity = config.beta2 * *velocity + (1.0 - config.beta2) * gradient * gradient;
        let m_hat = *moment / bias1.max(1e-12);
        let v_hat = *velocity / bias2.max(1e-12);
        let decay = config.weight_decay * *value;
        *value -= config.learning_rate * (m_hat / (v_hat.sqrt() + config.epsilon) + decay);
    }
}

fn export_lm_head_lora_safetensors(
    config: &Qwen36RealLoraSftConfig,
    hidden_size: usize,
    vocab_size: usize,
    lora_a: &[f32],
    lora_b: &[f32],
) -> Result<Vec<u8>, Qwen36RealLoraSftError> {
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from("psionic.qwen36_real_lora_sft"),
        serde_json::json!({
            "run_id": config.run_id,
            "base_model": QWEN36_27B_MODEL_ID,
            "served_model_id": QWEN36_27B_SERVED_MODEL_ID,
            "train_type": config.train_type,
            "activation_source": config.activation_source,
            "active_trainable_target": QWEN36_REAL_LORA_ACTIVE_TARGET,
            "requested_target_modules": config.target_modules,
            "qlora": qlora_metadata(config),
            "frozen_base_weights": true,
        })
        .to_string(),
    );
    let lora_a_bytes = encode_f32_bytes(lora_a);
    let lora_b_bytes = encode_f32_bytes(lora_b);
    let view_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![config.lora_rank, hidden_size],
        lora_a_bytes.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let view_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, config.lora_rank],
        lora_b_bytes.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    serialize(
        [
            ("lm_head.lora_A.weight", view_a),
            ("lm_head.lora_B.weight", view_b),
        ],
        Some(metadata),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))
}

fn export_state_checkpoint_safetensors(
    config: &Qwen36RealLoraSftConfig,
    hidden_size: usize,
    vocab_size: usize,
    state: &Qwen36RealLoraState,
) -> Result<Vec<u8>, Qwen36RealLoraSftError> {
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from("psionic.qwen36_real_lora_state_checkpoint"),
        serde_json::json!({
            "schema_version": QWEN36_REAL_LORA_STATE_CHECKPOINT_SCHEMA_VERSION,
            "run_id": config.run_id,
            "step": state.step,
            "lora_rank": config.lora_rank,
            "hidden_size": hidden_size,
            "vocab_size": vocab_size,
            "activation_source": config.activation_source,
            "train_type": config.train_type,
            "frozen_base_weights": true,
        })
        .to_string(),
    );
    let lora_a = encode_f32_bytes(&state.lora_a);
    let lora_b = encode_f32_bytes(&state.lora_b);
    let adam_m_a = encode_f32_bytes(&state.adam_m_a);
    let adam_v_a = encode_f32_bytes(&state.adam_v_a);
    let adam_m_b = encode_f32_bytes(&state.adam_m_b);
    let adam_v_b = encode_f32_bytes(&state.adam_v_b);
    let view_lora_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![config.lora_rank, hidden_size],
        lora_a.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let view_lora_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, config.lora_rank],
        lora_b.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let view_adam_m_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![config.lora_rank, hidden_size],
        adam_m_a.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let view_adam_v_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![config.lora_rank, hidden_size],
        adam_v_a.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let view_adam_m_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, config.lora_rank],
        adam_m_b.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let view_adam_v_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, config.lora_rank],
        adam_v_b.as_slice(),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    serialize(
        [
            ("lora_A.weight", view_lora_a),
            ("lora_B.weight", view_lora_b),
            ("adam_m_A", view_adam_m_a),
            ("adam_v_A", view_adam_v_a),
            ("adam_m_B", view_adam_m_b),
            ("adam_v_B", view_adam_v_b),
        ],
        Some(metadata),
    )
    .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))
}

fn load_state_checkpoint(
    path: &Path,
    config: &Qwen36RealLoraSftConfig,
    vocab_size: usize,
    hidden_size: usize,
) -> Result<Qwen36RealLoraState, Qwen36RealLoraSftError> {
    let bytes = fs::read(path).map_err(|error| Qwen36RealLoraSftError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let tensors = SafeTensors::deserialize(bytes.as_slice())
        .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    let metadata = read_state_checkpoint_metadata(bytes.as_slice())?;
    let step = metadata
        .get("step")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            Qwen36RealLoraSftError::InvalidConfig(String::from(
                "state checkpoint metadata is missing step",
            ))
        })?;
    let checkpoint_rank = metadata
        .get("lora_rank")
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            Qwen36RealLoraSftError::InvalidConfig(String::from(
                "state checkpoint metadata is missing lora_rank",
            ))
        })?;
    if checkpoint_rank != config.lora_rank {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "state checkpoint rank {checkpoint_rank} does not match config rank {}",
            config.lora_rank
        )));
    }
    Ok(Qwen36RealLoraState {
        step,
        lora_a: decode_state_tensor(&tensors, "lora_A.weight", &[config.lora_rank, hidden_size])?,
        lora_b: decode_state_tensor(&tensors, "lora_B.weight", &[vocab_size, config.lora_rank])?,
        adam_m_a: decode_state_tensor(&tensors, "adam_m_A", &[config.lora_rank, hidden_size])?,
        adam_v_a: decode_state_tensor(&tensors, "adam_v_A", &[config.lora_rank, hidden_size])?,
        adam_m_b: decode_state_tensor(&tensors, "adam_m_B", &[vocab_size, config.lora_rank])?,
        adam_v_b: decode_state_tensor(&tensors, "adam_v_B", &[vocab_size, config.lora_rank])?,
    })
}

fn read_state_checkpoint_metadata(
    bytes: &[u8],
) -> Result<serde_json::Value, Qwen36RealLoraSftError> {
    let header_len_bytes: [u8; 8] = bytes
        .get(..8)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| {
            Qwen36RealLoraSftError::InvalidConfig(String::from(
                "state checkpoint safetensors header is missing length prefix",
            ))
        })?;
    let header_len = usize::try_from(u64::from_le_bytes(header_len_bytes)).map_err(|_| {
        Qwen36RealLoraSftError::InvalidConfig(String::from(
            "state checkpoint safetensors header length is too large",
        ))
    })?;
    let header = bytes.get(8..8 + header_len).ok_or_else(|| {
        Qwen36RealLoraSftError::InvalidConfig(String::from(
            "state checkpoint safetensors header is truncated",
        ))
    })?;
    let header = serde_json::from_slice::<serde_json::Value>(header)?;
    let metadata = header
        .get("__metadata__")
        .and_then(|value| value.as_object())
        .and_then(|metadata| metadata.get("psionic.qwen36_real_lora_state_checkpoint"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            Qwen36RealLoraSftError::InvalidConfig(String::from(
                "state checkpoint is missing psionic.qwen36_real_lora_state_checkpoint metadata",
            ))
        })?;
    Ok(serde_json::from_str::<serde_json::Value>(metadata)?)
}

fn checkpoint_summary(
    config: &Qwen36RealLoraSftConfig,
    state: &Qwen36RealLoraState,
    adapter_path: &Path,
    adapter_artifact_sha256: &str,
    state_checkpoint_path: &Path,
    state_checkpoint_sha256: &str,
) -> Qwen36RealLoraCheckpointSummary {
    let optimizer_state_sha256 = stable_json_digest(
        b"qwen36_real_lora_optimizer_state|",
        &serde_json::json!({
            "step": state.step,
            "adam_m_a": sha256_f32_values(&state.adam_m_a),
            "adam_v_a": sha256_f32_values(&state.adam_v_a),
            "adam_m_b": sha256_f32_values(&state.adam_m_b),
            "adam_v_b": sha256_f32_values(&state.adam_v_b),
        }),
    );
    let mut checkpoint = Qwen36RealLoraCheckpointSummary {
        schema_version: String::from(QWEN36_REAL_LORA_SFT_CHECKPOINT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        step: state.step,
        adapter_artifact_path: adapter_path.display().to_string(),
        adapter_artifact_sha256: String::from(adapter_artifact_sha256),
        state_checkpoint_path: state_checkpoint_path.display().to_string(),
        state_checkpoint_sha256: String::from(state_checkpoint_sha256),
        optimizer_state_sha256,
        scheduler_cursor_state_sha256: stable_json_digest(
            b"qwen36_real_lora_scheduler_cursor|",
            &serde_json::json!({
                "run_id": config.run_id,
                "completed_step": state.step,
                "resume_next_step": state.step.saturating_add(1),
                "max_steps": config.max_steps,
            }),
        ),
        state_digest: String::new(),
    };
    checkpoint.state_digest = stable_json_digest(b"qwen36_real_lora_checkpoint|", &checkpoint);
    checkpoint
}

fn adapter_identity(
    config: &Qwen36RealLoraSftConfig,
    adapter_digest: &str,
    hidden_size: usize,
    vocab_size: usize,
) -> AdapterArtifactIdentity {
    AdapterArtifactIdentity::new(
        config.adapter_id.clone(),
        config.adapter_revision.clone(),
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        QWEN36_27B_MODEL_ID,
        "main",
        "sha256:real-qwen36-27b-local-safetensors",
        adapter_digest,
        if config.train_type == "qlora" {
            QuantizationMode::GgmlQ4K
        } else {
            QuantizationMode::None
        },
        AdapterTargetFamily::DecoderComposite,
        trainable_parameter_count(config.lora_rank, hidden_size, vocab_size),
    )
}

fn optimizer_receipt(config: &Qwen36RealLoraSftConfig) -> Qwen36RealLoraOptimizerReceipt {
    Qwen36RealLoraOptimizerReceipt {
        optimizer: String::from("adamw"),
        learning_rate: config.learning_rate,
        weight_decay: config.weight_decay,
        beta1: config.beta1,
        beta2: config.beta2,
        epsilon: config.epsilon,
        gradient_clip_norm: config.gradient_clip_norm,
    }
}

fn qlora_metadata(config: &Qwen36RealLoraSftConfig) -> Qwen36RealLoraQloraMetadata {
    if config.train_type == "qlora" {
        Qwen36RealLoraQloraMetadata {
            enabled: true,
            base_weight_quantization: String::from("ggml_q4_k_compatible_control_plane"),
            adapter_weight_dtype: String::from("f32"),
            compatibility_note: String::from(
                "QLoRA jobs keep the base checkpoint frozen and record q4_k compatibility for lower-memory Pylon placement. The current Rust trainer still writes F32 adapter deltas; exact 4-bit base matmul kernels are not part of this SFT command.",
            ),
        }
    } else {
        Qwen36RealLoraQloraMetadata {
            enabled: false,
            base_weight_quantization: String::from("none"),
            adapter_weight_dtype: String::from("f32"),
            compatibility_note: String::from(
                "LoRA jobs train F32 adapter deltas over frozen BF16 Qwen3.6 base weights.",
            ),
        }
    }
}

fn model_hashes(model_dir: &Path) -> Result<Qwen36RealModelHashes, Qwen36RealLoraSftError> {
    Ok(Qwen36RealModelHashes {
        config_sha256: sha256_file(&model_dir.join("config.json"))?,
        tokenizer_sha256: sha256_file(&model_dir.join("tokenizer.json"))?,
        index_sha256: sha256_file(&model_dir.join("model.safetensors.index.json"))?,
    })
}

fn decode_state_tensor(
    tensors: &SafeTensors<'_>,
    name: &str,
    expected_shape: &[usize],
) -> Result<Vec<f32>, Qwen36RealLoraSftError> {
    let tensor = tensors
        .tensor(name)
        .map_err(|error| Qwen36RealLoraSftError::Safetensors(error.to_string()))?;
    if tensor.dtype() != SafeTensorsDType::F32 {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "state checkpoint tensor `{name}` must be F32, found {:?}",
            tensor.dtype()
        )));
    }
    if tensor.shape() != expected_shape {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "state checkpoint tensor `{name}` shape {:?} did not match {:?}",
            tensor.shape(),
            expected_shape
        )));
    }
    let data = tensor.data();
    if data.len() % 4 != 0 {
        return Err(Qwen36RealLoraSftError::InvalidConfig(format!(
            "state checkpoint tensor `{name}` byte length is not divisible by 4"
        )));
    }
    Ok(data
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn trainable_parameter_count(rank: usize, hidden_size: usize, vocab_size: usize) -> u64 {
    u64::try_from(rank.saturating_mul(hidden_size.saturating_add(vocab_size))).unwrap_or(u64::MAX)
}

fn seeded_matrix(seed: &str, rows: usize, cols: usize, scale: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(rows.saturating_mul(cols));
    for index in 0..rows.saturating_mul(cols) {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(b"|");
        hasher.update(index.to_le_bytes());
        let digest = hasher.finalize();
        let raw = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
        let unit = raw as f32 / u32::MAX as f32;
        values.push((unit - 0.5) * 2.0 * scale);
    }
    values
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum()
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Qwen36RealLoraSftError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|error| Qwen36RealLoraSftError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn sha256_file(path: &Path) -> Result<String, Qwen36RealLoraSftError> {
    let bytes = fs::read(path).map_err(|error| Qwen36RealLoraSftError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn stable_json_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn sha256_f32_values(values: &[f32]) -> String {
    sha256_hex(encode_f32_bytes(values).as_slice())
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn default_real_qwen_model_dir() -> String {
    String::from(QWEN36_27B_REAL_MODEL_DIR)
}

fn default_real_train_type() -> String {
    String::from("lora")
}

fn default_activation_source() -> String {
    String::from(QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER)
}

fn default_adapter_id() -> String {
    String::from("qwen36-27b-real-lora-full-layer-smoke-001")
}

fn default_adapter_revision() -> String {
    String::from("real-full-layer-smoke-001")
}

fn default_target_modules() -> Vec<String> {
    vec![String::from("all-linear")]
}

fn default_lora_rank() -> usize {
    4
}

fn default_lora_alpha() -> f32 {
    8.0
}

fn default_learning_rate() -> f32 {
    0.01
}

fn default_beta1() -> f32 {
    0.9
}

fn default_beta2() -> f32 {
    0.999
}

fn default_epsilon() -> f32 {
    1e-8
}

fn default_max_steps() -> u64 {
    3
}

fn default_output_dir() -> String {
    String::from("target/legal/qwen36_27b_real_lora_sft_full_layer")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_surface() -> Qwen36RealLoraActivationSurface {
        Qwen36RealLoraActivationSurface {
            prompt_receipt: Qwen36PromptReceipt {
                template_id: String::from("test"),
                reasoning_mode: psionic_models::Qwen36ReasoningMode::DirectAnswer,
                prompt_hash: String::from("hash"),
                token_count: 1,
            },
            candidate_token_ids: vec![0, 1, 2],
            hidden_state: vec![0.5, -0.25, 0.75, 0.1],
            hidden_state_sha256: String::from("hidden"),
            base_sampled_logits: vec![
                Qwen36SampledLogit {
                    token_id: 0,
                    token_label: String::from("token:0"),
                    logit: -0.2,
                },
                Qwen36SampledLogit {
                    token_id: 1,
                    token_label: String::from("token:1"),
                    logit: 0.1,
                },
                Qwen36SampledLogit {
                    token_id: 2,
                    token_label: String::from("token:2"),
                    logit: 0.0,
                },
            ],
            logits_sha256: String::from("logits"),
            tensor_reads: vec![psionic_models::Qwen36TensorRowReadReceipt {
                tensor_name: String::from("model.language_model.embed_tokens.weight"),
                shard_name: String::from("test.safetensors"),
                shard_path: String::from("test.safetensors"),
                row_index: 2,
                dtype: String::from("BF16"),
                shape: vec![3, 4],
                row_sha256: String::from("row"),
            }],
            activation_receipt: Qwen36RealLoraActivationReceipt {
                activation_source: String::from(QWEN36_REAL_LORA_ACTIVATION_SOURCE_FULL_LAYER),
                activation_mode: String::from(QWEN36_REAL_LORA_FULL_LAYER_ACTIVATION_MODE),
                exact_full_width_logits: false,
                layer_receipt_count: 1,
                full_attention_layer_count: 1,
                linear_attention_layer_count: 0,
                mtp_layer_count: 0,
                claim_boundary: String::from("test surface"),
            },
        }
    }

    #[test]
    fn qwen36_real_lora_step_changes_weights_from_live_surface() {
        let config = Qwen36RealLoraSftConfig {
            lora_rank: 2,
            lora_alpha: 4.0,
            learning_rate: 0.01,
            max_steps: 1,
            target_token_id: 2,
            candidate_token_ids: vec![0, 1, 2],
            ..Qwen36RealLoraSftConfig::default()
        };
        let surface = tiny_surface();
        let mut state = Qwen36RealLoraState::new(&config, 3, 4);
        let before_a = state.lora_a.clone();
        let before_b = state.lora_b.clone();

        let result = train_one_step(&config, &surface, 3, 4, &mut state).expect("step");

        assert!(result.loss.is_finite());
        assert_ne!(before_a, state.lora_a);
        assert_ne!(before_b, state.lora_b);
        assert_eq!(state.step, 1);
    }

    #[test]
    fn qwen36_real_lora_resume_matches_uninterrupted_steps() {
        let config = Qwen36RealLoraSftConfig {
            lora_rank: 2,
            lora_alpha: 4.0,
            learning_rate: 0.01,
            max_steps: 2,
            target_token_id: 2,
            candidate_token_ids: vec![0, 1, 2],
            ..Qwen36RealLoraSftConfig::default()
        };
        let surface = tiny_surface();
        let mut uninterrupted = Qwen36RealLoraState::new(&config, 3, 4);
        train_one_step(&config, &surface, 3, 4, &mut uninterrupted).expect("step 1");
        train_one_step(&config, &surface, 3, 4, &mut uninterrupted).expect("step 2");

        let mut resumed = Qwen36RealLoraState::new(&config, 3, 4);
        train_one_step(&config, &surface, 3, 4, &mut resumed).expect("step 1");
        let checkpoint = resumed.clone();
        resumed = checkpoint;
        train_one_step(&config, &surface, 3, 4, &mut resumed).expect("step 2");

        assert_eq!(uninterrupted, resumed);
    }

    #[test]
    fn qwen36_real_lora_state_checkpoint_roundtrips_resume_state() {
        let config = Qwen36RealLoraSftConfig {
            lora_rank: 2,
            lora_alpha: 4.0,
            learning_rate: 0.01,
            max_steps: 2,
            target_token_id: 2,
            candidate_token_ids: vec![0, 1, 2],
            ..Qwen36RealLoraSftConfig::default()
        };
        let surface = tiny_surface();
        let mut uninterrupted = Qwen36RealLoraState::new(&config, 3, 4);
        train_one_step(&config, &surface, 3, 4, &mut uninterrupted).expect("step 1");
        let checkpoint_bytes =
            export_state_checkpoint_safetensors(&config, 4, 3, &uninterrupted).expect("checkpoint");
        let temp = tempfile::tempdir().expect("tempdir");
        let checkpoint_path = temp.path().join("state_checkpoint.safetensors");
        fs::write(&checkpoint_path, checkpoint_bytes).expect("write checkpoint");

        let mut resumed =
            load_state_checkpoint(&checkpoint_path, &config, 3, 4).expect("load checkpoint");
        train_one_step(&config, &surface, 3, 4, &mut uninterrupted).expect("step 2");
        train_one_step(&config, &surface, 3, 4, &mut resumed).expect("resumed step 2");

        assert_eq!(uninterrupted, resumed);
    }

    #[test]
    fn qwen36_real_lora_target_module_checks_refuse_router_and_resolve_all_linear() {
        let all = resolve_target_modules(&[String::from("all-linear")]).expect("all-linear");
        assert_eq!(
            all,
            QWEN36_DENSE_REAL_LORA_TARGET_MODULES
                .iter()
                .map(|value| String::from(*value))
                .collect::<Vec<_>>()
        );

        let error = resolve_target_modules(&[String::from("router")]).expect_err("router refused");
        assert!(error.to_string().contains("refuses router/MoE"));
    }

    #[test]
    fn qwen36_real_lora_qlora_mode_records_lower_memory_metadata() {
        let config = Qwen36RealLoraSftConfig {
            train_type: String::from("qlora"),
            ..Qwen36RealLoraSftConfig::default()
        };
        validate_config(&config).expect("qlora config");
        let metadata = qlora_metadata(&config);

        assert!(metadata.enabled);
        assert_eq!(
            metadata.base_weight_quantization,
            "ggml_q4_k_compatible_control_plane"
        );
        assert_eq!(metadata.adapter_weight_dtype, "f32");
    }

    #[test]
    fn qwen36_real_lora_exports_loadable_adapter_format() {
        let config = Qwen36RealLoraSftConfig {
            lora_rank: 2,
            lora_alpha: 4.0,
            ..Qwen36RealLoraSftConfig::default()
        };
        let state = Qwen36RealLoraState::new(&config, 3, 4);
        let bytes = export_lm_head_lora_safetensors(
            &config,
            4,
            3,
            state.lora_a.as_slice(),
            state.lora_b.as_slice(),
        )
        .expect("export");
        let digest = sha256_hex(bytes.as_slice());
        let identity = adapter_identity(&config, digest.as_str(), 4, 3);

        let adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
            bytes.as_slice(),
            identity,
            config.lora_alpha,
        )
        .expect("adapter loads");

        assert_eq!(adapter.rank, 2);
        assert_eq!(adapter.hidden_size, 4);
        assert_eq!(adapter.vocab_size, 3);
    }
}
