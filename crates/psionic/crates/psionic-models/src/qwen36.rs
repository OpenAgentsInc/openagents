use std::{
    fs,
    path::{Path, PathBuf},
};

use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokenizers::Tokenizer;

use crate::{PromptMessage, PromptMessageRole};

pub const QWEN36_TEMPLATE_ID: &str = "qwen3.6.chat_template.v1";
pub const QWEN36_EMPTY_THINK_BLOCK: &str = "<think>\n\n</think>";
pub const QWEN36_27B_MODEL_ID: &str = "Qwen/Qwen3.6-27B";
pub const QWEN36_27B_SHORT_MODEL_ID: &str = "Qwen3.6-27B";
pub const QWEN36_27B_SERVED_MODEL_ID: &str = "qwen3.6-27b";
pub const QWEN36_27B_SMOKE_CONFIG_PATH: &str = "fixtures/qwen36_27b_smoke/config.json";
pub const QWEN36_27B_SMOKE_TOKENIZER_PATH: &str = "fixtures/qwen36_27b_smoke/tokenizer.json";
pub const QWEN36_27B_SMOKE_SHARD_PATH: &str =
    "target/legal/qwen36_27b_prompt_smoke/model-00001-of-00001.safetensors";
pub const QWEN36_27B_REAL_MODEL_DIR: &str = "target/models/qwen/Qwen3.6-27B";
pub const QWEN36_35B_A3B_MODEL_ID: &str = "Qwen/Qwen3.6-35B-A3B";
pub const QWEN36_35B_A3B_SHORT_MODEL_ID: &str = "Qwen3.6-35B-A3B";
pub const QWEN36_35B_A3B_SERVED_MODEL_ID: &str = "qwen3.6-35b-a3b";
pub const QWEN36_35B_A3B_SMOKE_CONFIG_PATH: &str = "fixtures/qwen36_35b_a3b_smoke/config.json";
pub const QWEN36_35B_A3B_SMOKE_TOKENIZER_PATH: &str = "fixtures/qwen36_27b_smoke/tokenizer.json";
pub const QWEN36_35B_A3B_SMOKE_SHARD_PATH: &str =
    "target/legal/qwen36_35b_a3b_prompt_smoke/model-00001-of-00001.safetensors";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen36TargetMemoryStrategy {
    CpuOffloadSmoke,
    SingleHighMemoryGpu,
    PylonMultiWorker,
    QuantizedBaseAdapterTraining,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36ModelConfig {
    pub model_id: String,
    pub served_model_id: String,
    pub model_type: String,
    pub architectures: Vec<String>,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub vocab_size: usize,
    pub max_position_embeddings: usize,
    pub torch_dtype: String,
    pub tokenizer_path: String,
    pub chat_template_id: String,
    pub memory_strategy: Qwen36TargetMemoryStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_experts: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_experts_per_tok: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moe_intermediate_size: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LoadedShardReport {
    pub path: String,
    pub sha256: String,
    pub byte_len: u64,
    pub tensor_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36LegalPromptSmokeReport {
    pub schema_version: String,
    pub model_id: String,
    pub served_model_id: String,
    pub config_path: String,
    pub tokenizer_path: String,
    pub tokenizer_sha256: String,
    pub loaded_shards: Vec<Qwen36LoadedShardReport>,
    pub prompt_receipt: Qwen36PromptReceipt,
    pub rendered_prompt: String,
    pub deterministic_output: String,
    pub memory_strategy: Qwen36TargetMemoryStrategy,
    pub claim_boundary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen36ReasoningMode {
    Thinking,
    DirectAnswer,
    MixedExplicit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36PromptOptions {
    pub reasoning_mode: Qwen36ReasoningMode,
    pub add_generation_prompt: bool,
    pub emit_empty_think_block: bool,
}

impl Default for Qwen36PromptOptions {
    fn default() -> Self {
        Self {
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            add_generation_prompt: true,
            emit_empty_think_block: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RenderedPrompt {
    pub template_id: String,
    pub reasoning_mode: Qwen36ReasoningMode,
    pub text: String,
    pub prompt_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_ids: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Qwen36PromptReceipt {
    pub template_id: String,
    pub reasoning_mode: Qwen36ReasoningMode,
    pub prompt_hash: String,
    pub token_count: usize,
}

impl From<&Qwen36RenderedPrompt> for Qwen36PromptReceipt {
    fn from(prompt: &Qwen36RenderedPrompt) -> Self {
        Self {
            template_id: prompt.template_id.clone(),
            reasoning_mode: prompt.reasoning_mode,
            prompt_hash: prompt.prompt_hash.clone(),
            token_count: prompt.token_ids.len(),
        }
    }
}

#[derive(Debug, Error)]
pub enum Qwen36TemplateError {
    #[error("failed to load Qwen3.6 tokenizer: {0}")]
    TokenizerLoad(String),
    #[error("failed to tokenize Qwen3.6 prompt: {0}")]
    TokenizerEncode(String),
    #[error("invalid Qwen3.6 chat messages: {0}")]
    InvalidMessages(String),
}

pub struct Qwen36PromptRenderer {
    tokenizer: Option<Tokenizer>,
}

impl Qwen36PromptRenderer {
    pub fn without_tokenizer() -> Self {
        Self { tokenizer: None }
    }

    pub fn from_tokenizer_file(path: impl AsRef<Path>) -> Result<Self, Qwen36TemplateError> {
        let tokenizer = Tokenizer::from_file(path.as_ref())
            .map_err(|error| Qwen36TemplateError::TokenizerLoad(error.to_string()))?;
        Ok(Self {
            tokenizer: Some(tokenizer),
        })
    }

    pub fn from_tokenizer_json_bytes(bytes: &[u8]) -> Result<Self, Qwen36TemplateError> {
        let tokenizer = Tokenizer::from_bytes(bytes)
            .map_err(|error| Qwen36TemplateError::TokenizerLoad(error.to_string()))?;
        Ok(Self {
            tokenizer: Some(tokenizer),
        })
    }

    pub fn render(
        &self,
        messages: &[PromptMessage],
        options: &Qwen36PromptOptions,
    ) -> Result<Qwen36RenderedPrompt, Qwen36TemplateError> {
        let text = render_qwen36_prompt_text(messages, options)?;
        let prompt_hash = qwen36_prompt_hash(text.as_str(), options.reasoning_mode);
        let token_ids = if let Some(tokenizer) = &self.tokenizer {
            tokenizer
                .encode(text.as_str(), false)
                .map(|encoding| encoding.get_ids().to_vec())
                .map_err(|error| Qwen36TemplateError::TokenizerEncode(error.to_string()))?
        } else {
            Vec::new()
        };
        Ok(Qwen36RenderedPrompt {
            template_id: String::from(QWEN36_TEMPLATE_ID),
            reasoning_mode: options.reasoning_mode,
            text,
            prompt_hash,
            token_ids,
        })
    }
}

pub fn normalize_qwen36_27b_model_id(model: &str) -> Result<String, Qwen36TargetPathError> {
    match model {
        QWEN36_27B_MODEL_ID | QWEN36_27B_SHORT_MODEL_ID => Ok(String::from(QWEN36_27B_MODEL_ID)),
        other => Err(Qwen36TargetPathError::InvalidTargetModel(String::from(
            other,
        ))),
    }
}

pub fn normalize_qwen36_35b_a3b_model_id(model: &str) -> Result<String, Qwen36TargetPathError> {
    match model {
        QWEN36_35B_A3B_MODEL_ID | QWEN36_35B_A3B_SHORT_MODEL_ID => {
            Ok(String::from(QWEN36_35B_A3B_MODEL_ID))
        }
        other => Err(Qwen36TargetPathError::InvalidTargetModel(String::from(
            other,
        ))),
    }
}

pub fn normalize_qwen36_target_model_id(model: &str) -> Result<String, Qwen36TargetPathError> {
    match model {
        QWEN36_27B_MODEL_ID | QWEN36_27B_SHORT_MODEL_ID => Ok(String::from(QWEN36_27B_MODEL_ID)),
        QWEN36_35B_A3B_MODEL_ID | QWEN36_35B_A3B_SHORT_MODEL_ID => {
            Ok(String::from(QWEN36_35B_A3B_MODEL_ID))
        }
        other => Err(Qwen36TargetPathError::InvalidTargetModel(String::from(
            other,
        ))),
    }
}

pub fn load_qwen36_model_config(
    path: impl AsRef<Path>,
) -> Result<Qwen36ModelConfig, Qwen36TargetPathError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| Qwen36TargetPathError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let value = serde_json::from_slice::<Value>(bytes.as_slice())?;
    let config = if value.get("model_id").is_some() {
        serde_json::from_value::<Qwen36ModelConfig>(value)?
    } else {
        qwen36_model_config_from_hf_value(path, value)?
    };
    validate_qwen36_model_config(&config)?;
    Ok(config)
}

#[derive(Clone, Debug, Deserialize)]
struct Qwen36HfRootConfig {
    model_type: String,
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default)]
    text_config: Option<Qwen36HfTextConfig>,
}

#[derive(Clone, Debug, Deserialize)]
struct Qwen36HfTextConfig {
    model_type: String,
    hidden_size: usize,
    intermediate_size: usize,
    num_hidden_layers: usize,
    num_attention_heads: usize,
    num_key_value_heads: usize,
    vocab_size: usize,
    max_position_embeddings: usize,
    #[serde(default)]
    dtype: Option<String>,
    #[serde(default)]
    torch_dtype: Option<String>,
}

fn qwen36_model_config_from_hf_value(
    path: &Path,
    value: Value,
) -> Result<Qwen36ModelConfig, Qwen36TargetPathError> {
    let root = serde_json::from_value::<Qwen36HfRootConfig>(value)?;
    let Some(text) = root.text_config else {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "Hugging Face Qwen3.6 config must contain text_config",
        )));
    };
    if root.model_type != "qwen3_5" || text.model_type != "qwen3_5_text" {
        return Err(Qwen36TargetPathError::InvalidConfig(format!(
            "unsupported Hugging Face Qwen3.6 config model_type `{}` / `{}`",
            root.model_type, text.model_type
        )));
    }
    let tokenizer_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("tokenizer.json")
        .display()
        .to_string();
    Ok(Qwen36ModelConfig {
        model_id: String::from(QWEN36_27B_MODEL_ID),
        served_model_id: String::from(QWEN36_27B_SERVED_MODEL_ID),
        model_type: text.model_type,
        architectures: root.architectures,
        hidden_size: text.hidden_size,
        intermediate_size: text.intermediate_size,
        num_hidden_layers: text.num_hidden_layers,
        num_attention_heads: text.num_attention_heads,
        num_key_value_heads: text.num_key_value_heads,
        vocab_size: text.vocab_size,
        max_position_embeddings: text.max_position_embeddings,
        torch_dtype: text
            .torch_dtype
            .or(text.dtype)
            .unwrap_or_else(|| String::from("unknown")),
        tokenizer_path,
        chat_template_id: String::from(QWEN36_TEMPLATE_ID),
        memory_strategy: Qwen36TargetMemoryStrategy::SingleHighMemoryGpu,
        num_experts: None,
        num_experts_per_tok: None,
        moe_intermediate_size: None,
    })
}

pub fn validate_qwen36_model_config(
    config: &Qwen36ModelConfig,
) -> Result<(), Qwen36TargetPathError> {
    let model_id = normalize_qwen36_target_model_id(config.model_id.as_str())?;
    if model_id == QWEN36_27B_MODEL_ID && config.served_model_id != QWEN36_27B_SERVED_MODEL_ID {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "served_model_id must be qwen3.6-27b",
        )));
    }
    if model_id == QWEN36_35B_A3B_MODEL_ID
        && config.served_model_id != QWEN36_35B_A3B_SERVED_MODEL_ID
    {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "served_model_id must be qwen3.6-35b-a3b",
        )));
    }
    if !matches!(config.model_type.as_str(), "qwen3" | "qwen3_5_text") {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "model_type must be qwen3 or qwen3_5_text",
        )));
    }
    if model_id == QWEN36_27B_MODEL_ID {
        if !config.architectures.iter().any(|architecture| {
            matches!(
                architecture.as_str(),
                "Qwen3ForCausalLM" | "Qwen3_5ForCausalLM" | "Qwen3_5ForConditionalGeneration"
            )
        }) {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "27B architectures must include a Qwen3/Qwen3.5 causal or conditional generation architecture",
            )));
        }
    } else if !config
        .architectures
        .iter()
        .any(|architecture| architecture == "Qwen3MoeForCausalLM")
    {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "35B-A3B architectures must include Qwen3MoeForCausalLM",
        )));
    }
    if config.hidden_size == 0
        || config.intermediate_size == 0
        || config.num_hidden_layers == 0
        || config.num_attention_heads == 0
        || config.num_key_value_heads == 0
        || config.vocab_size == 0
        || config.max_position_embeddings == 0
    {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "Qwen3.6 config dimensions must be non-zero",
        )));
    }
    if config.num_key_value_heads > config.num_attention_heads {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "num_key_value_heads cannot exceed num_attention_heads",
        )));
    }
    if config.chat_template_id != QWEN36_TEMPLATE_ID {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "chat_template_id must match qwen3.6.chat_template.v1",
        )));
    }
    if model_id == QWEN36_35B_A3B_MODEL_ID {
        let Some(num_experts) = config.num_experts else {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "35B-A3B MoE config must declare num_experts",
            )));
        };
        let Some(num_experts_per_tok) = config.num_experts_per_tok else {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "35B-A3B MoE config must declare num_experts_per_tok",
            )));
        };
        let Some(moe_intermediate_size) = config.moe_intermediate_size else {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "35B-A3B MoE config must declare moe_intermediate_size",
            )));
        };
        if num_experts == 0 || num_experts_per_tok == 0 || moe_intermediate_size == 0 {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "35B-A3B MoE dimensions must be non-zero",
            )));
        }
        if num_experts_per_tok > num_experts {
            return Err(Qwen36TargetPathError::InvalidConfig(String::from(
                "num_experts_per_tok cannot exceed num_experts",
            )));
        }
    }
    Ok(())
}

pub fn write_qwen36_27b_smoke_safetensors(
    path: impl AsRef<Path>,
) -> Result<Qwen36LoadedShardReport, Qwen36TargetPathError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Qwen36TargetPathError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let embed = encode_f32_bytes(&[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]);
    let lm_head = encode_f32_bytes(&[0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0]);
    let embed_view = TensorView::new(SafeTensorsDType::F32, vec![2, 4], embed.as_slice())
        .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let head_view = TensorView::new(SafeTensorsDType::F32, vec![2, 4], lm_head.as_slice())
        .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let bytes = serialize(
        [
            ("model.embed_tokens.weight", embed_view),
            ("lm_head.weight", head_view),
        ],
        None,
    )
    .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    fs::write(path, bytes).map_err(|source| Qwen36TargetPathError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    load_qwen36_safetensors_shard(path)
}

pub fn load_qwen36_safetensors_shard(
    path: impl AsRef<Path>,
) -> Result<Qwen36LoadedShardReport, Qwen36TargetPathError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| Qwen36TargetPathError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let tensors = SafeTensors::deserialize(bytes.as_slice())
        .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let mut tensor_names = tensors
        .names()
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    tensor_names.sort();
    Ok(Qwen36LoadedShardReport {
        path: path.display().to_string(),
        sha256: sha256_hex(bytes.as_slice()),
        byte_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        tensor_names,
    })
}

pub fn write_qwen36_35b_a3b_moe_smoke_safetensors(
    path: impl AsRef<Path>,
) -> Result<Qwen36LoadedShardReport, Qwen36TargetPathError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Qwen36TargetPathError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let router = encode_f32_bytes(&[0.2, 0.8, 0.6, 0.4, 0.3, 0.7, 0.9, 0.1]);
    let expert_0_gate = encode_f32_bytes(&[0.1, 0.2, 0.3, 0.4]);
    let expert_0_up = encode_f32_bytes(&[0.4, 0.3, 0.2, 0.1]);
    let expert_0_down = encode_f32_bytes(&[0.5, 0.6, 0.7, 0.8]);
    let expert_1_gate = encode_f32_bytes(&[0.8, 0.7, 0.6, 0.5]);
    let expert_1_up = encode_f32_bytes(&[0.5, 0.6, 0.7, 0.8]);
    let expert_1_down = encode_f32_bytes(&[0.4, 0.3, 0.2, 0.1]);
    let router_view = TensorView::new(SafeTensorsDType::F32, vec![2, 4], router.as_slice())
        .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let expert_0_gate_view =
        TensorView::new(SafeTensorsDType::F32, vec![2, 2], expert_0_gate.as_slice())
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let expert_0_up_view =
        TensorView::new(SafeTensorsDType::F32, vec![2, 2], expert_0_up.as_slice())
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let expert_0_down_view =
        TensorView::new(SafeTensorsDType::F32, vec![2, 2], expert_0_down.as_slice())
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let expert_1_gate_view =
        TensorView::new(SafeTensorsDType::F32, vec![2, 2], expert_1_gate.as_slice())
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let expert_1_up_view =
        TensorView::new(SafeTensorsDType::F32, vec![2, 2], expert_1_up.as_slice())
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let expert_1_down_view =
        TensorView::new(SafeTensorsDType::F32, vec![2, 2], expert_1_down.as_slice())
            .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    let bytes = serialize(
        [
            ("model.layers.0.mlp.gate.weight", router_view),
            (
                "model.layers.0.mlp.experts.0.gate_proj.weight",
                expert_0_gate_view,
            ),
            (
                "model.layers.0.mlp.experts.0.up_proj.weight",
                expert_0_up_view,
            ),
            (
                "model.layers.0.mlp.experts.0.down_proj.weight",
                expert_0_down_view,
            ),
            (
                "model.layers.0.mlp.experts.1.gate_proj.weight",
                expert_1_gate_view,
            ),
            (
                "model.layers.0.mlp.experts.1.up_proj.weight",
                expert_1_up_view,
            ),
            (
                "model.layers.0.mlp.experts.1.down_proj.weight",
                expert_1_down_view,
            ),
        ],
        None,
    )
    .map_err(|error| Qwen36TargetPathError::SafeTensors(error.to_string()))?;
    fs::write(path, bytes).map_err(|source| Qwen36TargetPathError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    load_qwen36_safetensors_shard(path)
}

pub fn run_qwen36_legal_prompt_smoke(
    model: &str,
    prompt_path: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
    tokenizer_path: impl AsRef<Path>,
    shard_paths: &[PathBuf],
) -> Result<Qwen36LegalPromptSmokeReport, Qwen36TargetPathError> {
    let model_id = normalize_qwen36_target_model_id(model)?;
    let prompt_path = prompt_path.as_ref();
    let config_path = config_path.as_ref();
    let tokenizer_path = tokenizer_path.as_ref();
    let config = load_qwen36_model_config(config_path)?;
    if normalize_qwen36_target_model_id(config.model_id.as_str())? != model_id {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "requested model and config model_id differ",
        )));
    }
    let prompt = fs::read_to_string(prompt_path).map_err(|source| Qwen36TargetPathError::Io {
        path: prompt_path.to_path_buf(),
        source,
    })?;
    let tokenizer_bytes = fs::read(tokenizer_path).map_err(|source| Qwen36TargetPathError::Io {
        path: tokenizer_path.to_path_buf(),
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
    let loaded_shards = shard_paths
        .iter()
        .map(load_qwen36_safetensors_shard)
        .collect::<Result<Vec<_>, _>>()?;
    if loaded_shards.is_empty() {
        return Err(Qwen36TargetPathError::InvalidConfig(String::from(
            "at least one safetensors shard must be loaded",
        )));
    }
    let deterministic_output = deterministic_qwen36_legal_smoke_output(
        model_id.as_str(),
        rendered.prompt_hash.as_str(),
        loaded_shards.as_slice(),
    );
    let prompt_receipt = Qwen36PromptReceipt::from(&rendered);
    let rendered_prompt = rendered.text;
    let claim_boundary = if model_id == QWEN36_35B_A3B_MODEL_ID {
        String::from(
            "This Rust run loads the Qwen3.6-35B-A3B MoE config, tokenizer, and expert safetensors shards, then renders the Qwen3.6 direct-answer chat template. It does not run a full 35B-A3B forward pass, train the router, or measure private Harvey tasks.",
        )
    } else {
        String::from(
            "This Rust run loads the Qwen3.6-27B config, tokenizer, and safetensors shards, then renders the Qwen3.6 direct-answer chat template. It does not run a full 27B forward pass yet.",
        )
    };
    Ok(Qwen36LegalPromptSmokeReport {
        schema_version: if model_id == QWEN36_35B_A3B_MODEL_ID {
            String::from("psionic.qwen36_35b_a3b_legal_prompt_smoke.v1")
        } else {
            String::from("psionic.qwen36_27b_legal_prompt_smoke.v1")
        },
        model_id,
        served_model_id: config.served_model_id,
        config_path: config_path.display().to_string(),
        tokenizer_path: tokenizer_path.display().to_string(),
        tokenizer_sha256: sha256_hex(tokenizer_bytes.as_slice()),
        loaded_shards,
        prompt_receipt,
        rendered_prompt,
        deterministic_output,
        memory_strategy: config.memory_strategy,
        claim_boundary,
    })
}

fn deterministic_qwen36_legal_smoke_output(
    model_id: &str,
    prompt_hash: &str,
    loaded_shards: &[Qwen36LoadedShardReport],
) -> String {
    let shard_hash = loaded_shards
        .first()
        .map(|shard| shard.sha256.as_str())
        .unwrap_or("missing");
    format!(
        "{model_id} legal target path loaded {} shard(s), rendered prompt {}, and is ready for adapter evaluation. First shard hash: {}.",
        loaded_shards.len(),
        &prompt_hash[..12.min(prompt_hash.len())],
        &shard_hash[..12.min(shard_hash.len())]
    )
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

#[derive(Debug, Error)]
pub enum Qwen36TargetPathError {
    #[error("unsupported Qwen3.6 target model `{0}`")]
    InvalidTargetModel(String),
    #[error("invalid Qwen3.6 target config: {0}")]
    InvalidConfig(String),
    #[error("Qwen3.6 target path I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Qwen3.6 target path JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Qwen3.6 target path safetensors failed: {0}")]
    SafeTensors(String),
    #[error(transparent)]
    Template(#[from] Qwen36TemplateError),
}

pub fn render_qwen36_prompt_text(
    messages: &[PromptMessage],
    options: &Qwen36PromptOptions,
) -> Result<String, Qwen36TemplateError> {
    if messages.is_empty() {
        return Err(Qwen36TemplateError::InvalidMessages(String::from(
            "Qwen3.6 prompt rendering requires at least one message",
        )));
    }
    let mut rendered = String::new();
    let mut saw_non_instruction = false;
    let mut index = 0usize;
    while index < messages.len() {
        let message = &messages[index];
        match message.role {
            PromptMessageRole::System | PromptMessageRole::Developer => {
                if saw_non_instruction {
                    return Err(Qwen36TemplateError::InvalidMessages(String::from(
                        "Qwen3.6 system/developer messages must precede user, assistant, and tool messages",
                    )));
                }
                rendered.push_str("<|im_start|>system\n");
                rendered.push_str(message.content.trim());
                rendered.push_str("<|im_end|>\n");
            }
            PromptMessageRole::User => {
                saw_non_instruction = true;
                rendered.push_str("<|im_start|>user\n");
                rendered.push_str(message.content.trim());
                rendered.push_str("<|im_end|>\n");
            }
            PromptMessageRole::Assistant => {
                saw_non_instruction = true;
                rendered.push_str("<|im_start|>assistant\n");
                rendered
                    .push_str(qwen36_assistant_content(message, options.reasoning_mode)?.as_str());
                rendered.push_str("<|im_end|>\n");
            }
            PromptMessageRole::Tool => {
                saw_non_instruction = true;
                let start = index;
                while index < messages.len() && messages[index].role == PromptMessageRole::Tool {
                    index += 1;
                }
                rendered.push_str("<|im_start|>user\n");
                for tool_message in &messages[start..index] {
                    rendered.push_str("<tool_response>");
                    if let Some(author) = tool_message.author_name.as_deref() {
                        rendered.push_str("\nname: ");
                        rendered.push_str(author);
                    }
                    rendered.push('\n');
                    rendered.push_str(tool_message.content.trim());
                    rendered.push_str("\n</tool_response>\n");
                }
                rendered.push_str("<|im_end|>\n");
                continue;
            }
        }
        index += 1;
    }
    if options.add_generation_prompt {
        rendered.push_str("<|im_start|>assistant\n");
        if options.reasoning_mode == Qwen36ReasoningMode::DirectAnswer
            && options.emit_empty_think_block
        {
            rendered.push_str(QWEN36_EMPTY_THINK_BLOCK);
            rendered.push_str("\n\n");
        }
    }
    Ok(rendered)
}

pub fn qwen36_prompt_hash(text: &str, reasoning_mode: Qwen36ReasoningMode) -> String {
    let mut hasher = Sha256::new();
    hasher.update(QWEN36_TEMPLATE_ID.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{reasoning_mode:?}").as_bytes());
    hasher.update(b"|");
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn qwen36_assistant_content(
    message: &PromptMessage,
    mode: Qwen36ReasoningMode,
) -> Result<String, Qwen36TemplateError> {
    if message.content.contains("/think") || message.content.contains("/nothink") {
        return Err(Qwen36TemplateError::InvalidMessages(String::from(
            "Qwen3.6 renderer does not accept /think or /nothink control tokens",
        )));
    }
    let answer_content = strip_qwen36_think_block(message.content.trim());
    let reasoning = message
        .reasoning_content
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    Ok(match mode {
        Qwen36ReasoningMode::Thinking => {
            let mut content = String::new();
            content.push_str("<think>\n");
            content.push_str(reasoning.unwrap_or(""));
            content.push_str("\n</think>\n\n");
            content.push_str(answer_content.trim());
            content
        }
        Qwen36ReasoningMode::DirectAnswer => answer_content.trim().to_string(),
        Qwen36ReasoningMode::MixedExplicit => {
            if let Some(reasoning) = reasoning {
                let mut content = String::new();
                content.push_str("<think>\n");
                content.push_str(reasoning);
                content.push_str("\n</think>\n\n");
                content.push_str(answer_content.trim());
                content
            } else {
                answer_content.trim().to_string()
            }
        }
    })
}

fn strip_qwen36_think_block(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("<think>") {
        return content.to_string();
    }
    if let Some(end) = trimmed.find("</think>") {
        return trimmed[end + "</think>".len()..].trim_start().to_string();
    }
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PromptMessage, PromptMessageRole};

    fn messages() -> Vec<PromptMessage> {
        vec![
            PromptMessage::new(PromptMessageRole::System, "Use tools carefully."),
            PromptMessage::new(PromptMessageRole::User, "Write memo.md."),
            PromptMessage::new(PromptMessageRole::Tool, "{\"ok\":true}")
                .with_author_name("validate_deliverables"),
            PromptMessage::new(PromptMessageRole::Assistant, "Done.")
                .with_reasoning_content("Need to verify memo.md first."),
        ]
    }

    #[test]
    fn qwen36_template_direct_answer_is_stable_and_has_no_fake_reasoning() {
        let renderer = Qwen36PromptRenderer::without_tokenizer();
        let options = Qwen36PromptOptions {
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            add_generation_prompt: true,
            emit_empty_think_block: false,
        };
        let first = renderer.render(&messages(), &options).expect("render");
        let second = renderer.render(&messages(), &options).expect("render");

        assert_eq!(first.text, second.text);
        assert_eq!(first.prompt_hash, second.prompt_hash);
        assert!(!first.text.contains("/think"));
        assert!(!first.text.contains("/nothink"));
        assert!(!first.text.contains("Need to verify memo.md first."));
        assert!(!first.text.contains("<think>"));
        assert!(first.text.contains("<tool_response>"));
    }

    #[test]
    fn qwen36_template_thinking_mode_renders_explicit_reasoning() {
        let renderer = Qwen36PromptRenderer::without_tokenizer();
        let options = Qwen36PromptOptions {
            reasoning_mode: Qwen36ReasoningMode::Thinking,
            add_generation_prompt: false,
            emit_empty_think_block: false,
        };
        let rendered = renderer.render(&messages(), &options).expect("render");

        assert!(rendered
            .text
            .contains("<think>\nNeed to verify memo.md first.\n</think>"));
        assert!(rendered.text.contains("Done."));
    }

    #[test]
    fn qwen36_template_direct_generation_can_emit_empty_think_block() {
        let renderer = Qwen36PromptRenderer::without_tokenizer();
        let options = Qwen36PromptOptions {
            reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
            add_generation_prompt: true,
            emit_empty_think_block: true,
        };
        let rendered = renderer
            .render(
                &[PromptMessage::new(
                    PromptMessageRole::User,
                    "Answer directly.",
                )],
                &options,
            )
            .expect("render");

        assert!(rendered
            .text
            .ends_with("<|im_start|>assistant\n<think>\n\n</think>\n\n"));
    }

    #[test]
    fn qwen36_template_rejects_old_soft_switch_tokens() {
        let renderer = Qwen36PromptRenderer::without_tokenizer();
        let error = renderer
            .render(
                &[PromptMessage::new(
                    PromptMessageRole::Assistant,
                    "/nothink answer",
                )],
                &Qwen36PromptOptions::default(),
            )
            .expect_err("soft switch tokens are rejected");
        assert!(error.to_string().contains("/think or /nothink"));
    }

    #[test]
    fn qwen36_27b_target_config_admits_dense_smoke_path() {
        let config = Qwen36ModelConfig {
            model_id: String::from(QWEN36_27B_MODEL_ID),
            served_model_id: String::from(QWEN36_27B_SERVED_MODEL_ID),
            model_type: String::from("qwen3"),
            architectures: vec![String::from("Qwen3ForCausalLM")],
            hidden_size: 5120,
            intermediate_size: 27648,
            num_hidden_layers: 64,
            num_attention_heads: 40,
            num_key_value_heads: 8,
            vocab_size: 151936,
            max_position_embeddings: 131072,
            torch_dtype: String::from("bfloat16"),
            tokenizer_path: String::from(QWEN36_27B_SMOKE_TOKENIZER_PATH),
            chat_template_id: String::from(QWEN36_TEMPLATE_ID),
            memory_strategy: Qwen36TargetMemoryStrategy::CpuOffloadSmoke,
            num_experts: None,
            num_experts_per_tok: None,
            moe_intermediate_size: None,
        };

        validate_qwen36_model_config(&config).expect("Qwen3.6-27B target config");
        assert_eq!(
            normalize_qwen36_27b_model_id(QWEN36_27B_SHORT_MODEL_ID).expect("normalize"),
            QWEN36_27B_MODEL_ID
        );
    }

    #[test]
    fn qwen36_27b_target_config_loads_real_huggingface_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("config.json");
        let hf_config = serde_json::json!({
            "architectures": ["Qwen3_5ForConditionalGeneration"],
            "model_type": "qwen3_5",
            "tie_word_embeddings": false,
            "text_config": {
                "dtype": "bfloat16",
                "hidden_size": 5120,
                "intermediate_size": 17408,
                "max_position_embeddings": 262144,
                "model_type": "qwen3_5_text",
                "num_attention_heads": 24,
                "num_hidden_layers": 64,
                "num_key_value_heads": 4,
                "vocab_size": 248320
            }
        });
        fs::write(
            &config_path,
            serde_json::to_vec_pretty(&hf_config).expect("config json"),
        )
        .expect("write config");

        let config = load_qwen36_model_config(&config_path).expect("load real HF config");

        assert_eq!(config.model_id, QWEN36_27B_MODEL_ID);
        assert_eq!(config.served_model_id, QWEN36_27B_SERVED_MODEL_ID);
        assert_eq!(config.model_type, "qwen3_5_text");
        assert_eq!(config.hidden_size, 5120);
        assert_eq!(config.intermediate_size, 17408);
        assert_eq!(config.num_attention_heads, 24);
        assert_eq!(config.num_key_value_heads, 4);
        assert_eq!(config.vocab_size, 248320);
        assert_eq!(config.torch_dtype, "bfloat16");
        assert!(config.tokenizer_path.ends_with("tokenizer.json"));
    }

    #[test]
    fn qwen36_35b_a3b_target_config_requires_moe_facts() {
        let config = Qwen36ModelConfig {
            model_id: String::from(QWEN36_35B_A3B_MODEL_ID),
            served_model_id: String::from(QWEN36_35B_A3B_SERVED_MODEL_ID),
            model_type: String::from("qwen3"),
            architectures: vec![String::from("Qwen3MoeForCausalLM")],
            hidden_size: 5120,
            intermediate_size: 27648,
            num_hidden_layers: 64,
            num_attention_heads: 40,
            num_key_value_heads: 8,
            vocab_size: 151936,
            max_position_embeddings: 131072,
            torch_dtype: String::from("bfloat16"),
            tokenizer_path: String::from(QWEN36_35B_A3B_SMOKE_TOKENIZER_PATH),
            chat_template_id: String::from(QWEN36_TEMPLATE_ID),
            memory_strategy: Qwen36TargetMemoryStrategy::PylonMultiWorker,
            num_experts: Some(128),
            num_experts_per_tok: Some(8),
            moe_intermediate_size: Some(768),
        };

        validate_qwen36_model_config(&config).expect("Qwen3.6-35B-A3B MoE target config");
        assert_eq!(
            normalize_qwen36_35b_a3b_model_id(QWEN36_35B_A3B_SHORT_MODEL_ID).expect("normalize"),
            QWEN36_35B_A3B_MODEL_ID
        );
    }

    #[test]
    fn qwen36_35b_a3b_moe_smoke_loads_router_and_expert_tensors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let shard_path = temp.path().join("model-00001-of-00001.safetensors");

        let report =
            write_qwen36_35b_a3b_moe_smoke_safetensors(&shard_path).expect("write MoE shard");

        assert!(report
            .tensor_names
            .contains(&String::from("model.layers.0.mlp.gate.weight")));
        assert!(report.tensor_names.contains(&String::from(
            "model.layers.0.mlp.experts.0.gate_proj.weight"
        )));
        assert!(report.tensor_names.contains(&String::from(
            "model.layers.0.mlp.experts.1.down_proj.weight"
        )));
    }

    #[test]
    fn qwen36_27b_smoke_loads_tokenizer_shard_and_renders_legal_prompt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("config.json");
        let tokenizer_path = temp.path().join("tokenizer.json");
        let prompt_path = temp.path().join("smoke.prompt");
        let shard_path = temp.path().join("model-00001-of-00001.safetensors");
        let config = serde_json::json!({
            "model_id": QWEN36_27B_MODEL_ID,
            "served_model_id": QWEN36_27B_SERVED_MODEL_ID,
            "model_type": "qwen3",
            "architectures": ["Qwen3ForCausalLM"],
            "hidden_size": 5120,
            "intermediate_size": 27648,
            "num_hidden_layers": 64,
            "num_attention_heads": 40,
            "num_key_value_heads": 8,
            "vocab_size": 151936,
            "max_position_embeddings": 131072,
            "torch_dtype": "bfloat16",
            "tokenizer_path": QWEN36_27B_SMOKE_TOKENIZER_PATH,
            "chat_template_id": QWEN36_TEMPLATE_ID,
            "memory_strategy": "cpu_offload_smoke"
        });
        fs::write(
            &config_path,
            serde_json::to_vec_pretty(&config).expect("config json"),
        )
        .expect("write config");
        fs::write(&tokenizer_path, minimal_qwen36_tokenizer_json()).expect("write tokenizer");
        fs::write(
            &prompt_path,
            "Prepare a concise legal risk memo from the provided contract facts.",
        )
        .expect("write prompt");
        write_qwen36_27b_smoke_safetensors(&shard_path).expect("write shard");

        let report = run_qwen36_legal_prompt_smoke(
            QWEN36_27B_SHORT_MODEL_ID,
            &prompt_path,
            &config_path,
            &tokenizer_path,
            &[shard_path],
        )
        .expect("run smoke");

        assert_eq!(report.model_id, QWEN36_27B_MODEL_ID);
        assert_eq!(report.served_model_id, QWEN36_27B_SERVED_MODEL_ID);
        assert_eq!(report.loaded_shards.len(), 1);
        assert!(report
            .loaded_shards
            .first()
            .expect("shard")
            .tensor_names
            .contains(&String::from("lm_head.weight")));
        assert!(report.rendered_prompt.contains("<|im_start|>system"));
        assert!(report.rendered_prompt.contains(QWEN36_EMPTY_THINK_BLOCK));
        assert!(report.prompt_receipt.token_count > 0);
        assert!(report
            .deterministic_output
            .contains("Qwen3.6-27B legal target path"));
        assert!(report
            .claim_boundary
            .contains("does not run a full 27B forward pass"));
    }

    fn minimal_qwen36_tokenizer_json() -> &'static str {
        r#"{
  "version": "1.0",
  "truncation": null,
  "padding": null,
  "added_tokens": [
    {"id":0,"content":"<unk>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
    {"id":1,"content":"<|im_start|>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
    {"id":2,"content":"<|im_end|>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
    {"id":3,"content":"<think>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
    {"id":4,"content":"</think>","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}
  ],
  "normalizer": null,
  "pre_tokenizer": {"type": "Whitespace"},
  "post_processor": null,
  "decoder": null,
  "model": {
    "type": "WordLevel",
    "vocab": {
      "<unk>": 0,
      "<|im_start|>": 1,
      "<|im_end|>": 2,
      "<think>": 3,
      "</think>": 4,
      "system": 5,
      "user": 6,
      "assistant": 7,
      "You": 8,
      "are": 9,
      "Autopilot": 10,
      "legal": 11,
      "benchmark": 12,
      "agent": 13,
      "Answer": 14,
      "directly": 15,
      "write": 16,
      "usable": 17,
      "work": 18,
      "product": 19,
      "Prepare": 20,
      "a": 21,
      "concise": 22,
      "risk": 23,
      "memo": 24,
      "from": 25,
      "the": 26,
      "provided": 27,
      "contract": 28,
      "facts": 29
    },
    "unk_token": "<unk>"
  }
}"#
    }
}
