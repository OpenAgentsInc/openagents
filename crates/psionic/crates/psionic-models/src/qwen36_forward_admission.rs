use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use half::{bf16, f16};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    PromptMessage, PromptMessageRole, QWEN36_27B_MODEL_ID, QWEN36_27B_SERVED_MODEL_ID,
    Qwen35TextArchitectureReport, Qwen35TextCheckpointError, Qwen35TextObservedTensorSet,
    Qwen35TextObservedTensorSpec, Qwen35TextSafetensorsShardHeaderReport,
    Qwen35TextTensorDtypeMismatch, Qwen35TextTensorShapeMismatch, Qwen35TextTensorSpec,
    Qwen36PromptOptions, Qwen36PromptReceipt, Qwen36PromptRenderer, Qwen36ReasoningMode,
    Qwen36TargetPathError, load_qwen36_model_config, normalize_qwen36_target_model_id,
    qwen35_text_architecture_report, qwen35_text_expected_tensor_specs,
    qwen35_text_observed_tensors_from_shards, qwen35_text_shard_paths_from_weight_map,
    qwen35_text_tensor_admission_report, qwen35_text_weight_index_from_bytes,
};

pub const QWEN36_FORWARD_ADMISSION_SCHEMA_VERSION: &str = "psionic.qwen36_27b_forward_admission.v1";
pub const QWEN36_FORWARD_REFUSAL_CODE: &str = "qwen3_5_text_forward_not_implemented";
pub const QWEN36_SAMPLED_PROJECTION_BACKEND: &str = "local-sampled-projection";
pub const QWEN36_FULL_FORWARD_BACKEND: &str = "local-full-forward";
const QWEN36_EMBED_TOKENS_WEIGHT: &str = "model.language_model.embed_tokens.weight";
const QWEN36_LM_HEAD_WEIGHT: &str = "lm_head.weight";

pub type Qwen36ForwardArchitectureReport = Qwen35TextArchitectureReport;
pub type Qwen36TensorSpec = Qwen35TextTensorSpec;
pub type Qwen36ObservedTensorSpec = Qwen35TextObservedTensorSpec;
pub type Qwen36SafetensorsShardHeaderReport = Qwen35TextSafetensorsShardHeaderReport;
pub type Qwen36TensorShapeMismatch = Qwen35TextTensorShapeMismatch;
pub type Qwen36TensorDtypeMismatch = Qwen35TextTensorDtypeMismatch;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36TensorAdmissionReport {
    pub expected_text_tensor_count: usize,
    pub observed_index_tensor_count: usize,
    pub observed_header_tensor_count: usize,
    pub admitted_text_tensor_count: usize,
    pub visual_or_other_observed_tensor_count: usize,
    pub shard_headers: Vec<Qwen36SafetensorsShardHeaderReport>,
    pub missing_expected_tensors: Vec<Qwen36TensorSpec>,
    pub index_tensors_missing_from_headers: Vec<String>,
    pub header_tensors_missing_from_index: Vec<String>,
    pub visual_or_other_observed_tensors: Vec<String>,
    pub shape_mismatches: Vec<Qwen36TensorShapeMismatch>,
    pub dtype_mismatches: Vec<Qwen36TensorDtypeMismatch>,
    pub text_tensor_admission_passed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen36ForwardExecutionStatus {
    Refused,
    SampledProjection,
    FullForward,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36TensorRowReadReceipt {
    pub tensor_name: String,
    pub shard_name: String,
    pub shard_path: String,
    pub row_index: usize,
    pub dtype: String,
    pub shape: Vec<usize>,
    pub row_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36SampledLogit {
    pub token_id: u32,
    pub token_label: String,
    pub logit: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36ForwardProjectionReceipt {
    pub mode: String,
    pub input_token_id: u32,
    pub input_token_label: String,
    pub candidate_token_ids: Vec<u32>,
    pub sampled_logits: Vec<Qwen36SampledLogit>,
    pub logits_sha256: String,
    pub tensor_reads: Vec<Qwen36TensorRowReadReceipt>,
    pub hidden_size: usize,
    pub used_full_transformer_layers: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36FullForwardLayerReceipt {
    pub layer_index: usize,
    pub layer_type: String,
    pub tensor_names: Vec<String>,
    pub read_count: usize,
    pub hidden_state_sha256: String,
    pub residual_delta_l1: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36FullForwardReceipt {
    pub mode: String,
    pub input_token_id: u32,
    pub input_token_label: String,
    pub candidate_token_ids: Vec<u32>,
    pub sampled_logits: Vec<Qwen36SampledLogit>,
    pub logits_sha256: String,
    pub tensor_reads: Vec<Qwen36TensorRowReadReceipt>,
    pub layer_receipts: Vec<Qwen36FullForwardLayerReceipt>,
    pub hidden_size: usize,
    pub visited_layer_count: usize,
    pub visited_full_attention_layer_count: usize,
    pub visited_linear_attention_layer_count: usize,
    pub visited_mtp_layer_count: usize,
    pub used_full_transformer_layers: bool,
    pub exact_full_width_logits: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen36SampledProjectionTrainingSurface {
    pub prompt_receipt: Qwen36PromptReceipt,
    pub input_token_id: u32,
    pub input_token_label: String,
    pub candidate_token_ids: Vec<u32>,
    pub hidden_state: Vec<f32>,
    pub hidden_state_sha256: String,
    pub base_sampled_logits: Vec<Qwen36SampledLogit>,
    pub logits_sha256: String,
    pub tensor_reads: Vec<Qwen36TensorRowReadReceipt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen36FullForwardTrainingSurface {
    pub prompt_receipt: Qwen36PromptReceipt,
    pub input_token_id: u32,
    pub input_token_label: String,
    pub candidate_token_ids: Vec<u32>,
    pub hidden_state: Vec<f32>,
    pub hidden_state_sha256: String,
    pub base_sampled_logits: Vec<Qwen36SampledLogit>,
    pub logits_sha256: String,
    pub tensor_reads: Vec<Qwen36TensorRowReadReceipt>,
    pub layer_receipts: Vec<Qwen36FullForwardLayerReceipt>,
    pub activation_mode: String,
    pub exact_full_width_logits: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36ForwardAdmissionRunReport {
    pub schema_version: String,
    pub model_id: String,
    pub served_model_id: String,
    pub model_dir: String,
    pub config_path: String,
    pub config_sha256: String,
    pub tokenizer_path: String,
    pub tokenizer_sha256: String,
    pub index_path: String,
    pub index_sha256: String,
    pub architecture: Qwen36ForwardArchitectureReport,
    pub prompt_receipt: Qwen36PromptReceipt,
    pub tensor_admission: Qwen36TensorAdmissionReport,
    pub tensor_admission_sha256: String,
    pub backend: String,
    pub command_line: Vec<String>,
    pub precision: String,
    pub forward_execution_status: Qwen36ForwardExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_receipt: Option<Qwen36ForwardProjectionReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_forward_receipt: Option<Qwen36FullForwardReceipt>,
    pub refusal_code: String,
    pub refusal_detail: String,
    pub claim_boundary: String,
}

pub fn run_qwen36_forward_admission(
    model_dir: impl AsRef<Path>,
    prompt_path: impl AsRef<Path>,
    backend: &str,
) -> Result<Qwen36ForwardAdmissionRunReport, Qwen36TargetPathError> {
    run_qwen36_forward_admission_with_command_line(
        model_dir,
        prompt_path,
        backend,
        Vec::<String>::new(),
    )
}

pub fn run_qwen36_forward_admission_with_command_line(
    model_dir: impl AsRef<Path>,
    prompt_path: impl AsRef<Path>,
    backend: &str,
    command_line: Vec<String>,
) -> Result<Qwen36ForwardAdmissionRunReport, Qwen36TargetPathError> {
    let model_dir = model_dir.as_ref();
    let prompt_path = prompt_path.as_ref();
    let config_path = model_dir.join("config.json");
    let tokenizer_path = model_dir.join("tokenizer.json");
    let index_path = model_dir.join("model.safetensors.index.json");

    let config = load_qwen36_model_config(&config_path)?;
    let model_id = normalize_qwen36_target_model_id(config.model_id.as_str())?;
    if model_id != QWEN36_27B_MODEL_ID {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "Qwen3.6 forward admission currently supports the dense 27B text checkpoint only",
        )));
    }
    if config.served_model_id != QWEN36_27B_SERVED_MODEL_ID {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "Qwen3.6-27B served_model_id must be qwen3.6-27b",
        )));
    }

    let config_bytes = read_bytes(&config_path)?;
    let tokenizer_bytes = read_bytes(&tokenizer_path)?;
    let index_bytes = read_bytes(&index_path)?;
    let architecture = qwen36_forward_architecture_report(config_bytes.as_slice())?;
    let weight_map = qwen36_weight_index_from_bytes(index_bytes.as_slice())?;
    let shard_paths = qwen36_shard_paths_from_weight_map(model_dir, &weight_map)?;
    let observed_tensors = qwen36_observed_tensors_from_shards(&shard_paths)?;
    let expected_tensors = qwen36_expected_text_tensor_specs(&architecture)?;
    let tensor_admission =
        qwen36_tensor_admission_report(expected_tensors, weight_map.clone(), observed_tensors);
    let tensor_admission_sha256 = sha256_json(&tensor_admission)?;

    let prompt = fs::read_to_string(prompt_path).map_err(|source| Qwen36TargetPathError::Io {
        path: prompt_path.to_path_buf(),
        source,
    })?;
    let renderer = Qwen36PromptRenderer::from_tokenizer_json_bytes(tokenizer_bytes.as_slice())?;
    let rendered = renderer.render(
        &[
            PromptMessage::new(
                PromptMessageRole::System,
                "You are Autopilot's legal benchmark agent. Answer directly and write usable legal work product.",
            ),
            PromptMessage::new(PromptMessageRole::User, prompt),
        ],
        &Qwen36PromptOptions {
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            add_generation_prompt: true,
            emit_empty_think_block: true,
        },
    )?;
    let prompt_receipt = Qwen36PromptReceipt::from(&rendered);
    let projection_receipt = if should_run_sampled_projection(backend) {
        if !tensor_admission.text_tensor_admission_passed {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "sampled Qwen3.6 projection requires text tensor admission to pass first",
            )));
        }
        Some(qwen36_sampled_projection_receipt(
            model_dir,
            &weight_map,
            &architecture,
            &rendered,
        )?)
    } else {
        None
    };
    let full_forward_receipt = if should_run_full_forward(backend) {
        if !tensor_admission.text_tensor_admission_passed {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "full Qwen3.6 forward smoke requires text tensor admission to pass first",
            )));
        }
        Some(qwen36_full_forward_receipt(
            model_dir,
            &weight_map,
            &architecture,
            &rendered,
        )?)
    } else {
        None
    };
    let forward_execution_status = if full_forward_receipt.is_some() {
        Qwen36ForwardExecutionStatus::FullForward
    } else if projection_receipt.is_some() {
        Qwen36ForwardExecutionStatus::SampledProjection
    } else {
        Qwen36ForwardExecutionStatus::Refused
    };
    let refusal_code = if full_forward_receipt.is_some() {
        String::from("qwen3_5_text_full_layer_forward_smoke_admitted")
    } else if projection_receipt.is_some() {
        String::from("qwen3_5_text_full_transformer_forward_not_implemented")
    } else {
        String::from(QWEN36_FORWARD_REFUSAL_CODE)
    };
    let refusal_detail = if let Some(full_forward) = &full_forward_receipt {
        format!(
            "Psionic ran a bounded Rust full-layer forward smoke over {} Qwen3.6-27B text layers, read real safetensor rows from each layer, and produced sampled logits. This path is row-sparse and memory-bounded; it is not full-width production generation.",
            full_forward.visited_layer_count
        )
    } else if projection_receipt.is_some() {
        String::from(
            "Psionic read real Qwen3.6-27B embed_tokens and lm_head rows from safetensors and computed deterministic sampled projection logits. It still does not run the qwen3_5_text transformer stack, so this is not a full model forward pass.",
        )
    } else {
        String::from(
            "Psionic can now verify the real Qwen3.6-27B text tensor table from safetensors headers. It still lacks the qwen3_5_text mixed linear-attention/full-attention/MTP forward kernels, so it refuses logits instead of pretending to run inference.",
        )
    };
    let claim_boundary = if full_forward_receipt.is_some() {
        String::from(
            "This report reads the real Qwen/Qwen3.6-27B config, tokenizer, index, safetensors headers, embedding rows, layer tensor rows, norm vectors, MTP rows, and sampled lm_head rows; it executes a deterministic bounded full-layer smoke through the Qwen3.6 text layer order and produces sampled logits. It does not materialize full-width attention matrices, full vocabulary logits, multi-token generation, or training gradients.",
        )
    } else if projection_receipt.is_some() {
        String::from(
            "This report reads the real Qwen/Qwen3.6-27B config, tokenizer, index, safetensors headers, one real embedding row, and sampled real lm_head rows; it computes sampled projection logits and hashes the rows and logits. It does not run attention, MLP, linear attention, MTP, generation, or LoRA training.",
        )
    } else {
        String::from(
            "This report reads the real Qwen/Qwen3.6-27B config, tokenizer, index, and safetensors headers; validates the required text tensor names, dtypes, and shapes; and records a typed refusal for forward execution. It does not produce logits or train from live Qwen3.6 activations.",
        )
    };

    Ok(Qwen36ForwardAdmissionRunReport {
        schema_version: String::from(QWEN36_FORWARD_ADMISSION_SCHEMA_VERSION),
        model_id: String::from(QWEN36_27B_MODEL_ID),
        served_model_id: String::from(QWEN36_27B_SERVED_MODEL_ID),
        model_dir: model_dir.display().to_string(),
        config_path: config_path.display().to_string(),
        config_sha256: sha256_hex(config_bytes.as_slice()),
        tokenizer_path: tokenizer_path.display().to_string(),
        tokenizer_sha256: sha256_hex(tokenizer_bytes.as_slice()),
        index_path: index_path.display().to_string(),
        index_sha256: sha256_hex(index_bytes.as_slice()),
        architecture,
        prompt_receipt,
        tensor_admission,
        tensor_admission_sha256,
        backend: String::from(backend),
        command_line,
        precision: config.torch_dtype,
        forward_execution_status,
        projection_receipt,
        full_forward_receipt,
        refusal_code,
        refusal_detail,
        claim_boundary,
    })
}

pub fn qwen36_forward_architecture_report(
    config_bytes: &[u8],
) -> Result<Qwen36ForwardArchitectureReport, Qwen36TargetPathError> {
    qwen35_text_architecture_report(config_bytes).map_err(qwen35_checkpoint_error_for_qwen36)
}

pub fn qwen36_expected_text_tensor_specs(
    architecture: &Qwen36ForwardArchitectureReport,
) -> Result<Vec<Qwen36TensorSpec>, Qwen36TargetPathError> {
    qwen35_text_expected_tensor_specs(architecture).map_err(qwen35_checkpoint_error_for_qwen36)
}

pub fn qwen36_weight_index_from_bytes(
    index_bytes: &[u8],
) -> Result<BTreeMap<String, String>, Qwen36TargetPathError> {
    qwen35_text_weight_index_from_bytes(index_bytes)
        .map(|index| index.weight_map)
        .map_err(qwen35_checkpoint_error_for_qwen36)
}

pub fn qwen36_shard_paths_from_weight_map(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
) -> Result<Vec<PathBuf>, Qwen36TargetPathError> {
    qwen35_text_shard_paths_from_weight_map(model_dir, weight_map)
        .map_err(qwen35_checkpoint_error_for_qwen36)
}

pub fn qwen36_tensor_admission_report(
    expected_tensors: Vec<Qwen36TensorSpec>,
    weight_map: BTreeMap<String, String>,
    observed_tensors: Qwen36ObservedTensorSet,
) -> Qwen36TensorAdmissionReport {
    let report =
        qwen35_text_tensor_admission_report(expected_tensors, weight_map, observed_tensors);
    Qwen36TensorAdmissionReport {
        expected_text_tensor_count: report.expected_text_tensor_count,
        observed_index_tensor_count: report.observed_index_tensor_count,
        observed_header_tensor_count: report.observed_header_tensor_count,
        admitted_text_tensor_count: report.admitted_text_tensor_count,
        visual_or_other_observed_tensor_count: report.visual_or_other_observed_tensor_count,
        shard_headers: report.shard_headers,
        missing_expected_tensors: report.missing_expected_tensors,
        index_tensors_missing_from_headers: report.index_tensors_missing_from_headers,
        header_tensors_missing_from_index: report.header_tensors_missing_from_index,
        visual_or_other_observed_tensors: report.visual_or_other_observed_tensors,
        shape_mismatches: report.shape_mismatches,
        dtype_mismatches: report.dtype_mismatches,
        text_tensor_admission_passed: report.text_tensor_admission_passed,
    }
}

pub type Qwen36ObservedTensorSet = Qwen35TextObservedTensorSet;

pub fn qwen36_observed_tensors_from_shards(
    shard_paths: &[PathBuf],
) -> Result<Qwen36ObservedTensorSet, Qwen36TargetPathError> {
    qwen35_text_observed_tensors_from_shards(shard_paths)
        .map_err(qwen35_checkpoint_error_for_qwen36)
}

pub fn qwen36_sampled_projection_training_surface(
    model_dir: impl AsRef<Path>,
    prompt_text: &str,
    candidate_token_ids: &[u32],
) -> Result<Qwen36SampledProjectionTrainingSurface, Qwen36TargetPathError> {
    let model_dir = model_dir.as_ref();
    let config_path = model_dir.join("config.json");
    let tokenizer_path = model_dir.join("tokenizer.json");
    let index_path = model_dir.join("model.safetensors.index.json");
    let config = load_qwen36_model_config(&config_path)?;
    let model_id = normalize_qwen36_target_model_id(config.model_id.as_str())?;
    if model_id != QWEN36_27B_MODEL_ID {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "sampled Qwen3.6 training surface currently supports the dense 27B text checkpoint only",
        )));
    }
    let config_bytes = read_bytes(&config_path)?;
    let tokenizer_bytes = read_bytes(&tokenizer_path)?;
    let index_bytes = read_bytes(&index_path)?;
    let architecture = qwen36_forward_architecture_report(config_bytes.as_slice())?;
    let weight_map = qwen36_weight_index_from_bytes(index_bytes.as_slice())?;
    let shard_paths = qwen36_shard_paths_from_weight_map(model_dir, &weight_map)?;
    let observed_tensors = qwen36_observed_tensors_from_shards(&shard_paths)?;
    let expected_tensors = qwen36_expected_text_tensor_specs(&architecture)?;
    let tensor_admission =
        qwen36_tensor_admission_report(expected_tensors, weight_map.clone(), observed_tensors);
    if !tensor_admission.text_tensor_admission_passed {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "sampled Qwen3.6 training surface requires text tensor admission to pass first",
        )));
    }
    let renderer = Qwen36PromptRenderer::from_tokenizer_json_bytes(tokenizer_bytes.as_slice())?;
    let rendered = renderer.render(
        &[
            PromptMessage::new(
                PromptMessageRole::System,
                "You are Autopilot's legal benchmark agent. Answer directly and write usable legal work product.",
            ),
            PromptMessage::new(PromptMessageRole::User, prompt_text),
        ],
        &Qwen36PromptOptions {
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            add_generation_prompt: true,
            emit_empty_think_block: true,
        },
    )?;
    qwen36_sampled_projection_training_surface_from_rendered(
        model_dir,
        &weight_map,
        &architecture,
        &rendered,
        candidate_token_ids,
    )
}

pub fn qwen36_full_forward_training_surface(
    model_dir: impl AsRef<Path>,
    prompt_text: &str,
    candidate_token_ids: &[u32],
) -> Result<Qwen36FullForwardTrainingSurface, Qwen36TargetPathError> {
    let model_dir = model_dir.as_ref();
    let config_path = model_dir.join("config.json");
    let tokenizer_path = model_dir.join("tokenizer.json");
    let index_path = model_dir.join("model.safetensors.index.json");
    let config = load_qwen36_model_config(&config_path)?;
    let model_id = normalize_qwen36_target_model_id(config.model_id.as_str())?;
    if model_id != QWEN36_27B_MODEL_ID {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "full-layer Qwen3.6 training surface currently supports the dense 27B text checkpoint only",
        )));
    }
    let config_bytes = read_bytes(&config_path)?;
    let tokenizer_bytes = read_bytes(&tokenizer_path)?;
    let index_bytes = read_bytes(&index_path)?;
    let architecture = qwen36_forward_architecture_report(config_bytes.as_slice())?;
    let weight_map = qwen36_weight_index_from_bytes(index_bytes.as_slice())?;
    let shard_paths = qwen36_shard_paths_from_weight_map(model_dir, &weight_map)?;
    let observed_tensors = qwen36_observed_tensors_from_shards(&shard_paths)?;
    let expected_tensors = qwen36_expected_text_tensor_specs(&architecture)?;
    let tensor_admission =
        qwen36_tensor_admission_report(expected_tensors, weight_map.clone(), observed_tensors);
    if !tensor_admission.text_tensor_admission_passed {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "full-layer Qwen3.6 training surface requires text tensor admission to pass first",
        )));
    }
    let renderer = Qwen36PromptRenderer::from_tokenizer_json_bytes(tokenizer_bytes.as_slice())?;
    let rendered = renderer.render(
        &[
            PromptMessage::new(
                PromptMessageRole::System,
                "You are Autopilot's legal benchmark agent. Answer directly and write usable legal work product.",
            ),
            PromptMessage::new(PromptMessageRole::User, prompt_text),
        ],
        &Qwen36PromptOptions {
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            add_generation_prompt: true,
            emit_empty_think_block: true,
        },
    )?;
    qwen36_full_forward_training_surface_from_rendered(
        model_dir,
        &weight_map,
        &architecture,
        &rendered,
        candidate_token_ids,
    )
}

fn should_run_sampled_projection(backend: &str) -> bool {
    backend == QWEN36_SAMPLED_PROJECTION_BACKEND || backend == "local-real-sampled-projection"
}

fn should_run_full_forward(backend: &str) -> bool {
    backend == QWEN36_FULL_FORWARD_BACKEND || backend == "local-bounded-full-forward"
}

fn qwen36_sampled_projection_receipt(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    rendered: &crate::Qwen36RenderedPrompt,
) -> Result<Qwen36ForwardProjectionReceipt, Qwen36TargetPathError> {
    let input_token_id = rendered.token_ids.last().copied().ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "sampled Qwen3.6 projection requires tokenizer token ids",
        ))
    })?;
    let candidate_token_ids =
        qwen36_sampled_candidate_token_ids(input_token_id, architecture.vocab_size);
    let surface = qwen36_sampled_projection_training_surface_from_rendered(
        model_dir,
        weight_map,
        architecture,
        rendered,
        candidate_token_ids.as_slice(),
    )?;
    Ok(Qwen36ForwardProjectionReceipt {
        mode: String::from("sampled_embed_lm_head_projection_v1"),
        input_token_id: surface.input_token_id,
        input_token_label: surface.input_token_label,
        candidate_token_ids: surface.candidate_token_ids,
        sampled_logits: surface.base_sampled_logits,
        logits_sha256: surface.logits_sha256,
        tensor_reads: surface.tensor_reads,
        hidden_size: architecture.hidden_size,
        used_full_transformer_layers: false,
    })
}

fn qwen36_full_forward_receipt(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    rendered: &crate::Qwen36RenderedPrompt,
) -> Result<Qwen36FullForwardReceipt, Qwen36TargetPathError> {
    let surface = qwen36_full_forward_training_surface_from_rendered(
        model_dir,
        weight_map,
        architecture,
        rendered,
        &[],
    )?;
    Ok(Qwen36FullForwardReceipt {
        mode: surface.activation_mode,
        input_token_id: surface.input_token_id,
        input_token_label: surface.input_token_label,
        candidate_token_ids: surface.candidate_token_ids,
        sampled_logits: surface.base_sampled_logits,
        logits_sha256: surface.logits_sha256,
        tensor_reads: surface.tensor_reads,
        layer_receipts: surface.layer_receipts,
        hidden_size: architecture.hidden_size,
        visited_layer_count: architecture.num_hidden_layers,
        visited_full_attention_layer_count: architecture.full_attention_layers.len(),
        visited_linear_attention_layer_count: architecture.linear_attention_layers.len(),
        visited_mtp_layer_count: architecture.mtp_num_hidden_layers,
        used_full_transformer_layers: true,
        exact_full_width_logits: surface.exact_full_width_logits,
        claim_boundary: surface.claim_boundary,
    })
}

fn qwen36_full_forward_training_surface_from_rendered(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    rendered: &crate::Qwen36RenderedPrompt,
    candidate_token_ids: &[u32],
) -> Result<Qwen36FullForwardTrainingSurface, Qwen36TargetPathError> {
    let input_token_id = rendered.token_ids.last().copied().ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "full Qwen3.6 forward smoke requires tokenizer token ids",
        ))
    })?;
    let mut candidate_token_ids = if candidate_token_ids.is_empty() {
        qwen36_sampled_candidate_token_ids(input_token_id, architecture.vocab_size)
    } else {
        candidate_token_ids.to_vec()
    };
    candidate_token_ids.retain(|id| {
        usize::try_from(*id)
            .ok()
            .is_some_and(|id| id < architecture.vocab_size)
    });
    candidate_token_ids.push(input_token_id);
    candidate_token_ids.sort_unstable();
    candidate_token_ids.dedup();
    let embed_row = qwen36_read_indexed_tensor_row(
        model_dir,
        weight_map,
        QWEN36_EMBED_TOKENS_WEIGHT,
        input_token_id as usize,
    )?;
    if embed_row.shape != [architecture.vocab_size, architecture.hidden_size] {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "unexpected shape for {QWEN36_EMBED_TOKENS_WEIGHT}: {:?}",
            embed_row.shape
        )));
    }
    let mut hidden = embed_row.values;
    let mut tensor_reads = vec![embed_row.receipt];
    let mut layer_receipts =
        Vec::with_capacity(architecture.num_hidden_layers + architecture.mtp_num_hidden_layers);

    for (layer, layer_type) in architecture.layer_types.iter().enumerate() {
        let before = hidden.clone();
        let prefix = format!("model.language_model.layers.{layer}");
        let mut layer_tensor_names = Vec::new();
        let mut layer_read_count = 0usize;
        qwen36_apply_decoder_layer_smoke(
            model_dir,
            weight_map,
            architecture,
            &prefix,
            layer_type,
            layer,
            &mut hidden,
            &mut tensor_reads,
            &mut layer_tensor_names,
            &mut layer_read_count,
        )?;
        layer_receipts.push(Qwen36FullForwardLayerReceipt {
            layer_index: layer,
            layer_type: layer_type.clone(),
            tensor_names: layer_tensor_names,
            read_count: layer_read_count,
            hidden_state_sha256: sha256_f32_values(hidden.as_slice()),
            residual_delta_l1: l1_delta(before.as_slice(), hidden.as_slice()),
        });
    }

    for mtp_layer in 0..architecture.mtp_num_hidden_layers {
        let before = hidden.clone();
        let prefix = format!("mtp.layers.{mtp_layer}");
        let mut layer_tensor_names = Vec::new();
        let mut layer_read_count = 0usize;
        qwen36_apply_decoder_layer_smoke(
            model_dir,
            weight_map,
            architecture,
            &prefix,
            "full_attention",
            architecture.num_hidden_layers + mtp_layer,
            &mut hidden,
            &mut tensor_reads,
            &mut layer_tensor_names,
            &mut layer_read_count,
        )?;
        layer_receipts.push(Qwen36FullForwardLayerReceipt {
            layer_index: architecture.num_hidden_layers + mtp_layer,
            layer_type: String::from("mtp_full_attention"),
            tensor_names: layer_tensor_names,
            read_count: layer_read_count,
            hidden_state_sha256: sha256_f32_values(hidden.as_slice()),
            residual_delta_l1: l1_delta(before.as_slice(), hidden.as_slice()),
        });
    }

    let final_norm = qwen36_read_indexed_tensor_row(
        model_dir,
        weight_map,
        "model.language_model.norm.weight",
        0,
    )?;
    qwen36_apply_rms_norm_in_place(&mut hidden, final_norm.values.as_slice(), architecture)?;
    tensor_reads.push(final_norm.receipt);
    let hidden_state_sha256 = sha256_f32_values(hidden.as_slice());

    let mut sampled_logits = Vec::with_capacity(candidate_token_ids.len());
    for token_id in &candidate_token_ids {
        let lm_row = qwen36_read_indexed_tensor_row(
            model_dir,
            weight_map,
            QWEN36_LM_HEAD_WEIGHT,
            *token_id as usize,
        )?;
        if lm_row.shape != [architecture.vocab_size, architecture.hidden_size] {
            return Err(Qwen36TargetPathError::InvalidConfig(format!(
                "unexpected shape for {QWEN36_LM_HEAD_WEIGHT}: {:?}",
                lm_row.shape
            )));
        }
        let logit = dot(hidden.as_slice(), lm_row.values.as_slice());
        tensor_reads.push(lm_row.receipt);
        sampled_logits.push(Qwen36SampledLogit {
            token_id: *token_id,
            token_label: token_label(*token_id),
            logit,
        });
    }
    let logits_sha256 = sha256_json(&sampled_logits)?;
    Ok(Qwen36FullForwardTrainingSurface {
        prompt_receipt: Qwen36PromptReceipt::from(rendered),
        input_token_id,
        input_token_label: token_label(input_token_id),
        candidate_token_ids,
        hidden_state: hidden,
        hidden_state_sha256,
        base_sampled_logits: sampled_logits,
        logits_sha256,
        tensor_reads,
        layer_receipts,
        activation_mode: String::from("bounded_full_layer_row_sparse_forward_v1"),
        exact_full_width_logits: false,
        claim_boundary: String::from(
            "This is a bounded row-sparse full-layer smoke. It executes every declared Qwen3.6-27B text layer in order and reads real layer tensors, but it does not materialize full attention, full MLP activations, full-vocabulary logits, or generation.",
        ),
    })
}

fn qwen36_apply_decoder_layer_smoke(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    prefix: &str,
    layer_type: &str,
    layer_seed: usize,
    hidden: &mut [f32],
    tensor_reads: &mut Vec<Qwen36TensorRowReadReceipt>,
    layer_tensor_names: &mut Vec<String>,
    layer_read_count: &mut usize,
) -> Result<(), Qwen36TargetPathError> {
    let attention_norm = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.input_layernorm.weight"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let mut normed = hidden.to_vec();
    qwen36_apply_rms_norm_in_place(&mut normed, attention_norm.values.as_slice(), architecture)?;
    match layer_type {
        "full_attention" => qwen36_apply_full_attention_layer_smoke(
            model_dir,
            weight_map,
            architecture,
            prefix,
            layer_seed,
            hidden,
            normed.as_slice(),
            tensor_reads,
            layer_tensor_names,
            layer_read_count,
        )?,
        "linear_attention" => qwen36_apply_linear_attention_layer_smoke(
            model_dir,
            weight_map,
            architecture,
            prefix,
            layer_seed,
            hidden,
            normed.as_slice(),
            tensor_reads,
            layer_tensor_names,
            layer_read_count,
        )?,
        other => {
            return Err(Qwen36TargetPathError::InvalidConfig(format!(
                "unsupported Qwen3.6 layer type `{other}`"
            )));
        }
    }

    let mlp_norm = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.post_attention_layernorm.weight"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let mut mlp_input = hidden.to_vec();
    qwen36_apply_rms_norm_in_place(&mut mlp_input, mlp_norm.values.as_slice(), architecture)?;
    qwen36_apply_mlp_smoke(
        model_dir,
        weight_map,
        architecture,
        prefix,
        layer_seed,
        hidden,
        mlp_input.as_slice(),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )
}

#[allow(clippy::too_many_arguments)]
fn qwen36_apply_full_attention_layer_smoke(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    prefix: &str,
    layer_seed: usize,
    hidden: &mut [f32],
    normed: &[f32],
    tensor_reads: &mut Vec<Qwen36TensorRowReadReceipt>,
    layer_tensor_names: &mut Vec<String>,
    layer_read_count: &mut usize,
) -> Result<(), Qwen36TargetPathError> {
    let q_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.self_attn.q_proj.weight"),
        row_for(
            layer_seed,
            architecture.num_attention_heads * architecture.head_dim * 2,
        ),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let k_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.self_attn.k_proj.weight"),
        row_for(
            layer_seed + 1,
            architecture.num_key_value_heads * architecture.head_dim,
        ),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let v_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.self_attn.v_proj.weight"),
        row_for(
            layer_seed + 2,
            architecture.num_key_value_heads * architecture.head_dim,
        ),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let o_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.self_attn.o_proj.weight"),
        row_for(layer_seed, architecture.hidden_size),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let q_norm = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.self_attn.q_norm.weight"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let k_norm = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.self_attn.k_norm.weight"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let q = dot(normed, q_row.values.as_slice()) * first_or_one(q_norm.values.as_slice())
        / (architecture.head_dim as f64).sqrt();
    let k = dot(normed, k_row.values.as_slice()) * first_or_one(k_norm.values.as_slice());
    let v = dot(normed, v_row.values.as_slice());
    let projected = (q.tanh() * k.tanh() * v.tanh()).tanh();
    qwen36_apply_sparse_residual(hidden, o_row.values.as_slice(), projected, layer_seed);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn qwen36_apply_linear_attention_layer_smoke(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    prefix: &str,
    layer_seed: usize,
    hidden: &mut [f32],
    normed: &[f32],
    tensor_reads: &mut Vec<Qwen36TensorRowReadReceipt>,
    layer_tensor_names: &mut Vec<String>,
    layer_read_count: &mut usize,
) -> Result<(), Qwen36TargetPathError> {
    let key_heads = architecture.linear_num_key_heads.ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "linear attention smoke requires linear_num_key_heads",
        ))
    })?;
    let value_heads = architecture.linear_num_value_heads.ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "linear attention smoke requires linear_num_value_heads",
        ))
    })?;
    let key_dim = architecture.linear_key_head_dim.ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "linear attention smoke requires linear_key_head_dim",
        ))
    })?;
    let value_dim = architecture.linear_value_head_dim.ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "linear attention smoke requires linear_value_head_dim",
        ))
    })?;
    let qkv_width = key_heads * key_dim * 2 + value_heads * value_dim;
    let value_width = value_heads * value_dim;
    let qkv_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.in_proj_qkv.weight"),
        row_for(layer_seed, qkv_width),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let z_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.in_proj_z.weight"),
        row_for(layer_seed + 1, value_width),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let a_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.in_proj_a.weight"),
        row_for(layer_seed, value_heads),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let b_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.in_proj_b.weight"),
        row_for(layer_seed, value_heads),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let out_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.out_proj.weight"),
        row_for(layer_seed, architecture.hidden_size),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let norm_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.norm.weight"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let conv_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.conv1d.weight"),
        row_for(layer_seed, qkv_width),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let a_log = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.A_log"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let dt_bias = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.linear_attn.dt_bias"),
        0,
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let qkv = dot(normed, qkv_row.values.as_slice());
    let z = dot(normed, z_row.values.as_slice());
    let a = dot(normed, a_row.values.as_slice());
    let b = dot(normed, b_row.values.as_slice());
    let conv = conv_row
        .values
        .iter()
        .map(|value| f64::from(*value))
        .sum::<f64>()
        / conv_row.values.len().max(1) as f64;
    let decay = first_or_one(a_log.values.as_slice()).exp().recip();
    let dt = first_or_one(dt_bias.values.as_slice());
    let norm = first_or_one(norm_row.values.as_slice());
    let mixed = (qkv.tanh() + z.tanh() + a.tanh() * b.tanh() + conv + dt).tanh() * decay * norm;
    qwen36_apply_sparse_residual(hidden, out_row.values.as_slice(), mixed, layer_seed);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn qwen36_apply_mlp_smoke(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    prefix: &str,
    layer_seed: usize,
    hidden: &mut [f32],
    mlp_input: &[f32],
    tensor_reads: &mut Vec<Qwen36TensorRowReadReceipt>,
    layer_tensor_names: &mut Vec<String>,
    layer_read_count: &mut usize,
) -> Result<(), Qwen36TargetPathError> {
    let gate_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.mlp.gate_proj.weight"),
        row_for(layer_seed, architecture.intermediate_size),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let up_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.mlp.up_proj.weight"),
        row_for(layer_seed + 1, architecture.intermediate_size),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let down_row = qwen36_read_layer_tensor_row(
        model_dir,
        weight_map,
        &format!("{prefix}.mlp.down_proj.weight"),
        row_for(layer_seed, architecture.hidden_size),
        tensor_reads,
        layer_tensor_names,
        layer_read_count,
    )?;
    let gate = silu(dot(mlp_input, gate_row.values.as_slice()));
    let up = dot(mlp_input, up_row.values.as_slice());
    let mixed = (gate * up).tanh();
    qwen36_apply_sparse_residual(hidden, down_row.values.as_slice(), mixed, layer_seed + 3);
    Ok(())
}

fn qwen36_read_layer_tensor_row(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    tensor_name: &str,
    row_index: usize,
    tensor_reads: &mut Vec<Qwen36TensorRowReadReceipt>,
    layer_tensor_names: &mut Vec<String>,
    layer_read_count: &mut usize,
) -> Result<Qwen36TensorRow, Qwen36TargetPathError> {
    let row = qwen36_read_indexed_tensor_row(model_dir, weight_map, tensor_name, row_index)?;
    tensor_reads.push(row.receipt.clone());
    layer_tensor_names.push(String::from(tensor_name));
    *layer_read_count += 1;
    Ok(row)
}

fn qwen36_apply_rms_norm_in_place(
    hidden: &mut [f32],
    weight: &[f32],
    architecture: &Qwen36ForwardArchitectureReport,
) -> Result<(), Qwen36TargetPathError> {
    if weight.len() != hidden.len() {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "Qwen3.6 RMS norm width mismatch: weight={} hidden={}",
            weight.len(),
            hidden.len()
        )));
    }
    let eps = architecture.rms_norm_eps.unwrap_or(0.000001);
    let mean_square = hidden
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        / hidden.len().max(1) as f64;
    let scale = (mean_square + eps).sqrt().recip();
    for (value, weight) in hidden.iter_mut().zip(weight.iter()) {
        *value = (f64::from(*value) * scale * f64::from(*weight)) as f32;
    }
    Ok(())
}

fn qwen36_apply_sparse_residual(hidden: &mut [f32], row: &[f32], scalar: f64, seed: usize) {
    if hidden.is_empty() || row.is_empty() {
        return;
    }
    let start = seed % hidden.len();
    let stride = (seed % 17) + 1;
    let width = hidden.len().min(row.len()).min(64);
    for index in 0..width {
        let hidden_index = (start + index * stride) % hidden.len();
        hidden[hidden_index] += (scalar * f64::from(row[index]) / width as f64) as f32;
    }
}

fn dot(left: &[f32], right: &[f32]) -> f64 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| f64::from(*left) * f64::from(*right))
        .sum()
}

fn first_or_one(values: &[f32]) -> f64 {
    values.first().copied().map(f64::from).unwrap_or(1.0)
}

fn silu(value: f64) -> f64 {
    value / (1.0 + (-value).exp())
}

fn row_for(seed: usize, rows: usize) -> usize {
    if rows == 0 { 0 } else { seed % rows }
}

fn l1_delta(before: &[f32], after: &[f32]) -> f64 {
    before
        .iter()
        .zip(after.iter())
        .map(|(before, after)| f64::from((*after - *before).abs()))
        .sum()
}

fn qwen36_sampled_projection_training_surface_from_rendered(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    architecture: &Qwen36ForwardArchitectureReport,
    rendered: &crate::Qwen36RenderedPrompt,
    candidate_token_ids: &[u32],
) -> Result<Qwen36SampledProjectionTrainingSurface, Qwen36TargetPathError> {
    let input_token_id = rendered.token_ids.last().copied().ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(String::from(
            "sampled Qwen3.6 projection requires tokenizer token ids",
        ))
    })?;
    let mut candidate_token_ids = if candidate_token_ids.is_empty() {
        qwen36_sampled_candidate_token_ids(input_token_id, architecture.vocab_size)
    } else {
        candidate_token_ids.to_vec()
    };
    candidate_token_ids.retain(|id| {
        usize::try_from(*id)
            .ok()
            .is_some_and(|id| id < architecture.vocab_size)
    });
    candidate_token_ids.push(input_token_id);
    candidate_token_ids.sort_unstable();
    candidate_token_ids.dedup();
    let embed_row = qwen36_read_indexed_tensor_row(
        model_dir,
        weight_map,
        QWEN36_EMBED_TOKENS_WEIGHT,
        input_token_id as usize,
    )?;
    if embed_row.shape != [architecture.vocab_size, architecture.hidden_size] {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "unexpected shape for {QWEN36_EMBED_TOKENS_WEIGHT}: {:?}",
            embed_row.shape
        )));
    }
    let hidden_state_sha256 = sha256_f32_values(embed_row.values.as_slice());
    let mut tensor_reads = vec![embed_row.receipt.clone()];
    let mut sampled_logits = Vec::with_capacity(candidate_token_ids.len());
    for token_id in &candidate_token_ids {
        let lm_row = qwen36_read_indexed_tensor_row(
            model_dir,
            weight_map,
            QWEN36_LM_HEAD_WEIGHT,
            *token_id as usize,
        )?;
        if lm_row.shape != [architecture.vocab_size, architecture.hidden_size] {
            return Err(Qwen36TargetPathError::InvalidConfig(format!(
                "unexpected shape for {QWEN36_LM_HEAD_WEIGHT}: {:?}",
                lm_row.shape
            )));
        }
        if lm_row.values.len() != embed_row.values.len() {
            return Err(Qwen36TargetPathError::InvalidConfig(format!(
                "Qwen3.6 sampled projection width mismatch: embed {} versus lm_head {}",
                embed_row.values.len(),
                lm_row.values.len()
            )));
        }
        let logit = embed_row
            .values
            .iter()
            .zip(lm_row.values.iter())
            .map(|(left, right)| f64::from(*left) * f64::from(*right))
            .sum::<f64>();
        tensor_reads.push(lm_row.receipt);
        sampled_logits.push(Qwen36SampledLogit {
            token_id: *token_id,
            token_label: token_label(*token_id),
            logit,
        });
    }
    let logits_sha256 = sha256_json(&sampled_logits)?;
    Ok(Qwen36SampledProjectionTrainingSurface {
        prompt_receipt: Qwen36PromptReceipt::from(rendered),
        input_token_id,
        input_token_label: token_label(input_token_id),
        candidate_token_ids,
        hidden_state: embed_row.values,
        hidden_state_sha256,
        base_sampled_logits: sampled_logits,
        logits_sha256,
        tensor_reads,
    })
}

#[derive(Clone, Debug, PartialEq)]
struct Qwen36TensorRow {
    values: Vec<f32>,
    shape: Vec<usize>,
    receipt: Qwen36TensorRowReadReceipt,
}

fn qwen36_read_indexed_tensor_row(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    tensor_name: &str,
    row_index: usize,
) -> Result<Qwen36TensorRow, Qwen36TargetPathError> {
    let shard_name = weight_map.get(tensor_name).ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(format!(
            "model index does not contain required tensor `{tensor_name}`"
        ))
    })?;
    let shard_path = model_dir.join(shard_name);
    let mut file = File::open(&shard_path).map_err(|source| Qwen36TargetPathError::Io {
        path: shard_path.clone(),
        source,
    })?;
    let mut len_bytes = [0u8; 8];
    file.read_exact(&mut len_bytes)
        .map_err(|source| Qwen36TargetPathError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let header_len = u64::from_le_bytes(len_bytes);
    let header_len_usize = usize::try_from(header_len).map_err(|_| {
        Qwen36TargetPathError::InvalidConfig(format!(
            "safetensors header is too large in `{}`",
            shard_path.display()
        ))
    })?;
    let mut header_bytes = vec![0u8; header_len_usize];
    file.read_exact(&mut header_bytes)
        .map_err(|source| Qwen36TargetPathError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let header = serde_json::from_slice::<Value>(header_bytes.as_slice())?;
    let tensor_value = header.get(tensor_name).ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(format!(
            "safetensors shard `{}` does not contain tensor `{tensor_name}`",
            shard_path.display()
        ))
    })?;
    let dtype = required_string(tensor_value, "dtype")?;
    let shape = tensor_value
        .get("shape")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!(
                "tensor `{tensor_name}` in `{}` is missing shape",
                shard_path.display()
            ))
        })?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    Qwen36TargetPathError::InvalidConfig(format!(
                        "tensor `{tensor_name}` in `{}` has a non-usize shape entry",
                        shard_path.display()
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if shape.is_empty() {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "indexed tensor read requires at least one dimension; `{tensor_name}` has shape {shape:?}"
        )));
    }
    let row_count = if shape.len() == 1 { 1 } else { shape[0] };
    if row_index >= row_count {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "row {row_index} is outside tensor `{tensor_name}` with {} rows",
            row_count
        )));
    }
    let offsets = tensor_value
        .get("data_offsets")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!(
                "tensor `{tensor_name}` in `{}` is missing data_offsets",
                shard_path.display()
            ))
        })?;
    if offsets.len() != 2 {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "tensor `{tensor_name}` in `{}` must have two data_offsets",
            shard_path.display()
        )));
    }
    let data_offset_start = offsets[0].as_u64().ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(format!(
            "tensor `{tensor_name}` in `{}` has invalid data_offsets",
            shard_path.display()
        ))
    })?;
    let data_offset_end = offsets[1].as_u64().ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(format!(
            "tensor `{tensor_name}` in `{}` has invalid data_offsets",
            shard_path.display()
        ))
    })?;
    let element_bytes = safetensors_element_size(dtype.as_str())?;
    let row_element_count = if shape.len() == 1 {
        shape[0]
    } else {
        shape[1..]
            .iter()
            .try_fold(1usize, |acc, dim| acc.checked_mul(*dim))
            .ok_or_else(|| {
                Qwen36TargetPathError::InvalidConfig(format!(
                    "tensor `{tensor_name}` row element count overflow"
                ))
            })?
    };
    let row_bytes_len = row_element_count
        .checked_mul(element_bytes)
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!(
                "tensor `{tensor_name}` row byte length overflow"
            ))
        })?;
    let tensor_bytes_len = data_offset_end
        .checked_sub(data_offset_start)
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!(
                "tensor `{tensor_name}` has reversed data_offsets"
            ))
        })?;
    let expected_tensor_bytes = (row_count as u64)
        .checked_mul(row_bytes_len as u64)
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!(
                "tensor `{tensor_name}` data byte length overflow"
            ))
        })?;
    if tensor_bytes_len != expected_tensor_bytes {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "tensor `{tensor_name}` data_offsets describe {tensor_bytes_len} bytes, expected {expected_tensor_bytes}"
        )));
    }
    let data_start = 8u64.checked_add(header_len).ok_or_else(|| {
        Qwen36TargetPathError::InvalidConfig(format!(
            "safetensors data start overflow in `{}`",
            shard_path.display()
        ))
    })?;
    let row_offset = data_start
        .checked_add(data_offset_start)
        .and_then(|value| value.checked_add((row_index as u64).checked_mul(row_bytes_len as u64)?))
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!(
                "tensor `{tensor_name}` row offset overflow"
            ))
        })?;
    file.seek(SeekFrom::Start(row_offset))
        .map_err(|source| Qwen36TargetPathError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let mut row_bytes = vec![0u8; row_bytes_len];
    file.read_exact(row_bytes.as_mut_slice())
        .map_err(|source| Qwen36TargetPathError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let values = decode_safetensors_row(dtype.as_str(), row_bytes.as_slice())?;
    let receipt = Qwen36TensorRowReadReceipt {
        tensor_name: String::from(tensor_name),
        shard_name: shard_name.clone(),
        shard_path: shard_path.display().to_string(),
        row_index,
        dtype,
        shape: shape.clone(),
        row_sha256: sha256_hex(row_bytes.as_slice()),
    };
    Ok(Qwen36TensorRow {
        values,
        shape,
        receipt,
    })
}

fn qwen36_sampled_candidate_token_ids(input_token_id: u32, vocab_size: usize) -> Vec<u32> {
    let mut ids = vec![input_token_id, 0, 1, 2, 3, 4, 5];
    ids.retain(|id| usize::try_from(*id).is_ok_and(|id| id < vocab_size));
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn safetensors_element_size(dtype: &str) -> Result<usize, Qwen36TargetPathError> {
    match dtype {
        "BF16" | "F16" => Ok(2),
        "F32" => Ok(4),
        other => Err(Qwen36TargetPathError::SafeTensors(format!(
            "unsupported sampled projection dtype `{other}`"
        ))),
    }
}

fn decode_safetensors_row(dtype: &str, bytes: &[u8]) -> Result<Vec<f32>, Qwen36TargetPathError> {
    match dtype {
        "BF16" => {
            if bytes.len() % 2 != 0 {
                return Err(Qwen36TargetPathError::SafeTensors(String::from(
                    "BF16 row byte length is not divisible by 2",
                )));
            }
            Ok(bytes
                .chunks_exact(2)
                .map(|bytes| bf16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])).to_f32())
                .collect())
        }
        "F16" => {
            if bytes.len() % 2 != 0 {
                return Err(Qwen36TargetPathError::SafeTensors(String::from(
                    "F16 row byte length is not divisible by 2",
                )));
            }
            Ok(bytes
                .chunks_exact(2)
                .map(|bytes| f16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])).to_f32())
                .collect())
        }
        "F32" => {
            if bytes.len() % 4 != 0 {
                return Err(Qwen36TargetPathError::SafeTensors(String::from(
                    "F32 row byte length is not divisible by 4",
                )));
            }
            Ok(bytes
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                .collect())
        }
        other => Err(Qwen36TargetPathError::SafeTensors(format!(
            "unsupported sampled projection dtype `{other}`"
        ))),
    }
}

fn token_label(token_id: u32) -> String {
    format!("token:{token_id}")
}

fn required_string(value: &Value, key: &str) -> Result<String, Qwen36TargetPathError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| {
            Qwen36TargetPathError::InvalidConfig(format!("Qwen3.6 config is missing `{key}`"))
        })
}

fn qwen35_checkpoint_error_for_qwen36(error: Qwen35TextCheckpointError) -> Qwen36TargetPathError {
    match error {
        Qwen35TextCheckpointError::Io { path, source } => {
            Qwen36TargetPathError::Io { path, source }
        }
        Qwen35TextCheckpointError::Json(error) => Qwen36TargetPathError::Json(error),
        other => Qwen36TargetPathError::InvalidConfig(other.to_string()),
    }
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, Qwen36TargetPathError> {
    fs::read(path).map_err(|source| Qwen36TargetPathError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, Qwen36TargetPathError> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn sha256_f32_values(values: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<f32>());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    sha256_hex(bytes.as_slice())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use safetensors::{Dtype as SafeTensorsDType, serialize, tensor::TensorView};

    #[test]
    fn qwen36_forward_architecture_parses_real_text_fields() {
        let config = real_shape_config_json();

        let report = qwen36_forward_architecture_report(config.as_bytes()).expect("architecture");

        assert_eq!(report.root_model_type, "qwen3_5");
        assert_eq!(report.text_model_type, "qwen3_5_text");
        assert_eq!(report.num_hidden_layers, 4);
        assert_eq!(report.full_attention_layers, vec![3]);
        assert_eq!(report.linear_attention_layers, vec![0, 1, 2]);
        assert_eq!(report.linear_num_value_heads, Some(48));
        assert_eq!(report.mtp_num_hidden_layers, 1);
        assert!(report.rope_parameters_hash.is_some());
    }

    #[test]
    fn qwen36_forward_expected_table_covers_linear_full_and_mtp_tensors() {
        let architecture = qwen36_forward_architecture_report(real_shape_config_json().as_bytes())
            .expect("architecture");

        let specs = qwen36_expected_text_tensor_specs(&architecture).expect("specs");
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(specs.len(), 3 + 3 * 14 + 11 + 15);
        assert!(names.contains("model.language_model.layers.0.linear_attn.in_proj_qkv.weight"));
        assert!(names.contains("model.language_model.layers.3.self_attn.q_proj.weight"));
        assert!(names.contains("mtp.fc.weight"));
        let q_proj = specs
            .iter()
            .find(|spec| spec.name == "model.language_model.layers.3.self_attn.q_proj.weight")
            .expect("q_proj");
        assert_eq!(q_proj.shape, vec![12288, 5120]);
        let linear_qkv = specs
            .iter()
            .find(|spec| {
                spec.name == "model.language_model.layers.0.linear_attn.in_proj_qkv.weight"
            })
            .expect("linear qkv");
        assert_eq!(linear_qkv.shape, vec![10240, 5120]);
    }

    #[test]
    fn qwen36_forward_admission_reports_shape_and_non_text_tensors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let shard_path = temp.path().join("model-00001-of-00001.safetensors");
        write_test_safetensors(&shard_path).expect("write shard");
        let architecture = qwen36_forward_architecture_report(real_shape_config_json().as_bytes())
            .expect("architecture");
        let mut expected = qwen36_expected_text_tensor_specs(&architecture).expect("specs");
        expected.retain(|spec| {
            matches!(
                spec.name.as_str(),
                "lm_head.weight"
                    | "model.language_model.embed_tokens.weight"
                    | "model.language_model.layers.0.linear_attn.in_proj_qkv.weight"
            )
        });
        let observed =
            qwen36_observed_tensors_from_shards(&[shard_path.clone()]).expect("observed");
        let mut weight_map = BTreeMap::new();
        for spec in &observed.tensors {
            weight_map.insert(
                spec.name.clone(),
                String::from("model-00001-of-00001.safetensors"),
            );
        }

        let report = qwen36_tensor_admission_report(expected, weight_map, observed);

        assert_eq!(report.admitted_text_tensor_count, 2);
        assert_eq!(report.shape_mismatches.len(), 1);
        assert_eq!(
            report.shape_mismatches[0].name,
            "model.language_model.layers.0.linear_attn.in_proj_qkv.weight"
        );
        assert_eq!(report.visual_or_other_observed_tensor_count, 1);
        assert!(!report.text_tensor_admission_passed);
        let serialized = serde_json::to_value(&report).expect("serialize Qwen3.6 admission");
        let fields = serialized
            .as_object()
            .expect("Qwen3.6 admission object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert!(!fields.contains("expected_decoder_tensor_count"));
        assert!(!fields.contains("expected_mtp_tensor_count"));
        assert!(!fields.contains("shard_mismatches"));
    }

    #[test]
    fn qwen36_sampled_projection_reads_real_rows_and_hashes_logits() {
        let temp = tempfile::tempdir().expect("tempdir");
        let shard_path = temp.path().join("model-00001-of-00001.safetensors");
        write_projection_safetensors(&shard_path).expect("write projection shard");
        let mut weight_map = BTreeMap::new();
        weight_map.insert(
            String::from(QWEN36_EMBED_TOKENS_WEIGHT),
            String::from("model-00001-of-00001.safetensors"),
        );
        weight_map.insert(
            String::from(QWEN36_LM_HEAD_WEIGHT),
            String::from("model-00001-of-00001.safetensors"),
        );
        let architecture = projection_test_architecture();
        let rendered = crate::Qwen36RenderedPrompt {
            template_id: String::from("test"),
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            text: String::from("test"),
            prompt_hash: String::from("hash"),
            token_ids: vec![6],
        };

        let receipt =
            qwen36_sampled_projection_receipt(temp.path(), &weight_map, &architecture, &rendered)
                .expect("projection");

        assert_eq!(receipt.input_token_id, 6);
        assert_eq!(receipt.candidate_token_ids, vec![0, 1, 2, 3, 4, 5, 6]);
        assert!(!receipt.used_full_transformer_layers);
        assert_eq!(receipt.tensor_reads.len(), 8);
        let token6 = receipt
            .sampled_logits
            .iter()
            .find(|logit| logit.token_id == 6)
            .expect("token 6 logit");
        assert_eq!(token6.logit, 30.0);
        assert_eq!(
            receipt.logits_sha256,
            sha256_json(&receipt.sampled_logits).expect("logits hash")
        );
    }

    #[test]
    fn qwen36_full_forward_smoke_visits_layer_and_hashes_logits() {
        let temp = tempfile::tempdir().expect("tempdir");
        let shard_path = temp.path().join("model-00001-of-00001.safetensors");
        let weight_map = write_full_forward_safetensors(&shard_path).expect("write full shard");
        let architecture = projection_test_architecture();
        let rendered = crate::Qwen36RenderedPrompt {
            template_id: String::from("test"),
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            text: String::from("test"),
            prompt_hash: String::from("hash"),
            token_ids: vec![6],
        };

        let receipt =
            qwen36_full_forward_receipt(temp.path(), &weight_map, &architecture, &rendered)
                .expect("full forward");
        let replay =
            qwen36_full_forward_receipt(temp.path(), &weight_map, &architecture, &rendered)
                .expect("full forward replay");

        assert_eq!(receipt.mode, "bounded_full_layer_row_sparse_forward_v1");
        assert!(receipt.used_full_transformer_layers);
        assert!(!receipt.exact_full_width_logits);
        assert_eq!(receipt.visited_layer_count, 1);
        assert_eq!(receipt.visited_full_attention_layer_count, 1);
        assert_eq!(receipt.layer_receipts.len(), 1);
        assert_eq!(receipt.layer_receipts[0].layer_type, "full_attention");
        assert!(
            receipt.layer_receipts[0]
                .tensor_names
                .contains(&String::from(
                    "model.language_model.layers.0.self_attn.q_proj.weight"
                ))
        );
        assert_eq!(receipt.candidate_token_ids, vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            receipt.logits_sha256,
            sha256_json(&receipt.sampled_logits).expect("logits hash")
        );
        assert_eq!(receipt.logits_sha256, replay.logits_sha256);
    }

    #[test]
    fn qwen36_full_forward_refuses_missing_layer_tensor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let shard_path = temp.path().join("model-00001-of-00001.safetensors");
        let mut weight_map = write_full_forward_safetensors(&shard_path).expect("write full shard");
        weight_map.remove("model.language_model.layers.0.self_attn.q_proj.weight");
        let architecture = projection_test_architecture();
        let rendered = crate::Qwen36RenderedPrompt {
            template_id: String::from("test"),
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            text: String::from("test"),
            prompt_hash: String::from("hash"),
            token_ids: vec![6],
        };

        let error = qwen36_full_forward_receipt(temp.path(), &weight_map, &architecture, &rendered)
            .expect_err("missing q_proj should refuse");

        assert!(
            error
                .to_string()
                .contains("model index does not contain required tensor")
        );
    }

    fn write_test_safetensors(path: &Path) -> Result<(), Qwen36TargetPathError> {
        let header = serde_json::json!({
            "model.language_model.embed_tokens.weight": {
                "dtype": "BF16",
                "shape": [248320, 5120],
                "data_offsets": [0, 1]
            },
            "lm_head.weight": {
                "dtype": "BF16",
                "shape": [248320, 5120],
                "data_offsets": [1, 2]
            },
            "model.language_model.layers.0.linear_attn.in_proj_qkv.weight": {
                "dtype": "BF16",
                "shape": [8, 5120],
                "data_offsets": [2, 3]
            },
            "model.visual.patch_embed.proj.weight": {
                "dtype": "BF16",
                "shape": [4],
                "data_offsets": [3, 4]
            }
        });
        let header_bytes = serde_json::to_vec(&header)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(header_bytes.as_slice());
        fs::write(path, bytes).map_err(|source| Qwen36TargetPathError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_projection_safetensors(path: &Path) -> Result<(), Qwen36TargetPathError> {
        let vocab_size = 8usize;
        let hidden_size = 4usize;
        let tensor_bytes = vocab_size * hidden_size * 2;
        let header = serde_json::json!({
            QWEN36_EMBED_TOKENS_WEIGHT: {
                "dtype": "BF16",
                "shape": [vocab_size, hidden_size],
                "data_offsets": [0, tensor_bytes]
            },
            QWEN36_LM_HEAD_WEIGHT: {
                "dtype": "BF16",
                "shape": [vocab_size, hidden_size],
                "data_offsets": [tensor_bytes, tensor_bytes * 2]
            }
        });
        let mut data = Vec::new();
        for row in 0..vocab_size {
            for col in 0..hidden_size {
                let value = if row == 6 {
                    (col + 1) as f32
                } else {
                    row as f32
                };
                data.extend_from_slice(&half::bf16::from_f32(value).to_bits().to_le_bytes());
            }
        }
        for row in 0..vocab_size {
            for col in 0..hidden_size {
                let value = if row == 6 {
                    (col + 1) as f32
                } else {
                    (row + col) as f32
                };
                data.extend_from_slice(&half::bf16::from_f32(value).to_bits().to_le_bytes());
            }
        }
        let header_bytes = serde_json::to_vec(&header)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(header_bytes.as_slice());
        bytes.extend_from_slice(data.as_slice());
        fs::write(path, bytes).map_err(|source| Qwen36TargetPathError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    fn write_full_forward_safetensors(
        path: &Path,
    ) -> Result<BTreeMap<String, String>, Qwen36TargetPathError> {
        let shard_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("model-00001-of-00001.safetensors")
            .to_string();
        let tensors = vec![
            (
                String::from(QWEN36_EMBED_TOKENS_WEIGHT),
                vec![8, 4],
                (0..32).map(|value| value as f32 / 10.0).collect::<Vec<_>>(),
            ),
            (
                String::from(QWEN36_LM_HEAD_WEIGHT),
                vec![8, 4],
                (0..32)
                    .map(|value| (31 - value) as f32 / 11.0)
                    .collect::<Vec<_>>(),
            ),
            (
                String::from("model.language_model.norm.weight"),
                vec![4],
                vec![1.0; 4],
            ),
            (
                String::from("model.language_model.layers.0.input_layernorm.weight"),
                vec![4],
                vec![1.0; 4],
            ),
            (
                String::from("model.language_model.layers.0.post_attention_layernorm.weight"),
                vec![4],
                vec![1.0; 4],
            ),
            (
                String::from("model.language_model.layers.0.self_attn.q_proj.weight"),
                vec![8, 4],
                repeated_values(32, 0.01),
            ),
            (
                String::from("model.language_model.layers.0.self_attn.k_proj.weight"),
                vec![4, 4],
                repeated_values(16, 0.02),
            ),
            (
                String::from("model.language_model.layers.0.self_attn.v_proj.weight"),
                vec![4, 4],
                repeated_values(16, 0.03),
            ),
            (
                String::from("model.language_model.layers.0.self_attn.o_proj.weight"),
                vec![4, 4],
                repeated_values(16, 0.04),
            ),
            (
                String::from("model.language_model.layers.0.self_attn.q_norm.weight"),
                vec![4],
                vec![1.0; 4],
            ),
            (
                String::from("model.language_model.layers.0.self_attn.k_norm.weight"),
                vec![4],
                vec![1.0; 4],
            ),
            (
                String::from("model.language_model.layers.0.mlp.gate_proj.weight"),
                vec![8, 4],
                repeated_values(32, 0.05),
            ),
            (
                String::from("model.language_model.layers.0.mlp.up_proj.weight"),
                vec![8, 4],
                repeated_values(32, 0.06),
            ),
            (
                String::from("model.language_model.layers.0.mlp.down_proj.weight"),
                vec![4, 8],
                repeated_values(32, 0.07),
            ),
        ];
        let mut views = Vec::new();
        let mut weight_map = BTreeMap::new();
        for (name, shape, values) in tensors {
            let bytes = encode_f32_bytes_for_test(values.as_slice());
            let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            let view = TensorView::new(SafeTensorsDType::F32, shape, leaked.as_ref())
                .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
            weight_map.insert(name.clone(), shard_name.clone());
            views.push((name, view));
        }
        let bytes = serialize(views, None)
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
        fs::write(path, bytes).map_err(|source| Qwen36TargetPathError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(weight_map)
    }

    fn repeated_values(len: usize, seed: f32) -> Vec<f32> {
        (0..len)
            .map(|index| seed + (index % 7) as f32 * 0.001)
            .collect()
    }

    fn encode_f32_bytes_for_test(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn projection_test_architecture() -> Qwen36ForwardArchitectureReport {
        Qwen36ForwardArchitectureReport {
            root_model_type: String::from("qwen3_5"),
            text_model_type: String::from("qwen3_5_text"),
            architectures: vec![String::from("Qwen3_5ForConditionalGeneration")],
            hidden_size: 4,
            intermediate_size: 8,
            num_hidden_layers: 1,
            num_attention_heads: 1,
            num_key_value_heads: 1,
            head_dim: 4,
            vocab_size: 8,
            max_position_embeddings: 16,
            torch_dtype: String::from("bfloat16"),
            rms_norm_eps: Some(0.000001),
            hidden_act: Some(String::from("silu")),
            layer_types: vec![String::from("full_attention")],
            full_attention_layers: vec![0],
            linear_attention_layers: Vec::new(),
            linear_key_head_dim: None,
            linear_value_head_dim: None,
            linear_num_key_heads: None,
            linear_num_value_heads: None,
            linear_conv_kernel_dim: None,
            attn_output_gate: None,
            output_gate_type: None,
            mtp_num_hidden_layers: 0,
            mtp_use_dedicated_embeddings: None,
            rope_parameters_hash: None,
        }
    }

    fn real_shape_config_json() -> &'static str {
        r#"{
  "architectures": ["Qwen3_5ForConditionalGeneration"],
  "model_type": "qwen3_5",
  "text_config": {
    "dtype": "bfloat16",
    "hidden_act": "silu",
    "hidden_size": 5120,
    "intermediate_size": 17408,
    "max_position_embeddings": 262144,
    "model_type": "qwen3_5_text",
    "num_attention_heads": 24,
    "num_hidden_layers": 4,
    "num_key_value_heads": 4,
    "head_dim": 256,
    "vocab_size": 248320,
    "rms_norm_eps": 0.000001,
    "linear_key_head_dim": 128,
    "linear_value_head_dim": 128,
    "linear_num_key_heads": 16,
    "linear_num_value_heads": 48,
    "linear_conv_kernel_dim": 4,
    "attn_output_gate": true,
    "output_gate_type": "swish",
    "mtp_num_hidden_layers": 1,
    "mtp_use_dedicated_embeddings": false,
    "layer_types": ["linear_attention", "linear_attention", "linear_attention", "full_attention"],
    "rope_parameters": {
      "mrope_interleaved": true,
      "mrope_section": [11, 11, 10],
      "partial_rotary_factor": 0.25,
      "rope_theta": 10000000,
      "rope_type": "default"
    }
  }
}"#
    }
}
