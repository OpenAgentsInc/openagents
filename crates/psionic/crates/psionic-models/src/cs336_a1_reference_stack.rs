use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
use psionic_nn::{
    Activation, ActivationKind, Embedding, LayerError, Linear, Module, ModuleParameter,
    ModuleStateDict, ModuleStateError, ModuleStateLoadError, ModuleStateLoadMode,
    ModuleStateLoadReport, NnTensor,
};
use psionic_transformer::{
    scaled_dot_product_attention, AttentionMask, AttentionMaskError, AttentionTensor4,
    AttentionTensorError, ScaledDotProductAttentionError,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1ReferenceConfig {
    pub vocab_size: usize,
    pub context_length: usize,
    pub d_model: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    pub d_ff: usize,
}

impl Cs336A1ReferenceConfig {
    pub fn validate(self) -> Result<Self, Cs336A1ReferenceError> {
        if self.vocab_size == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("vocab_size must be positive"),
            });
        }
        if self.context_length == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("context_length must be positive"),
            });
        }
        if self.d_model == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("d_model must be positive"),
            });
        }
        if self.num_layers == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("num_layers must be positive"),
            });
        }
        if self.num_heads == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("num_heads must be positive"),
            });
        }
        if self.d_ff == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("d_ff must be positive"),
            });
        }
        if !self.d_model.is_multiple_of(self.num_heads) {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: format!(
                    "d_model {} must be divisible by num_heads {}",
                    self.d_model, self.num_heads
                ),
            });
        }
        let head_dim = self.d_model / self.num_heads;
        if !head_dim.is_multiple_of(2) {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: format!("head_dim {head_dim} must be even for RoPE"),
            });
        }
        Ok(self)
    }

    #[must_use]
    pub const fn head_dim(self) -> usize {
        self.d_model / self.num_heads
    }
}

#[derive(Debug, Error)]
pub enum Cs336A1ReferenceError {
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error(transparent)]
    ModuleStateLoad(#[from] ModuleStateLoadError),
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    AttentionTensor(#[from] AttentionTensorError),
    #[error(transparent)]
    AttentionMask(#[from] AttentionMaskError),
    #[error(transparent)]
    Attention(#[from] ScaledDotProductAttentionError),
    #[error("invalid CS336 A1 reference configuration for `{context}`: {detail}")]
    InvalidConfiguration {
        context: &'static str,
        detail: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1SwiGlu {
    module: Module,
    d_model: usize,
    d_ff: usize,
}

impl Cs336A1SwiGlu {
    pub fn new(
        module_id: impl Into<String>,
        d_model: usize,
        d_ff: usize,
    ) -> Result<Self, Cs336A1ReferenceError> {
        if d_model == 0 || d_ff == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "swiglu",
                detail: String::from("d_model and d_ff must be positive"),
            });
        }
        let mut module = Module::new(module_id, "cs336_a1_swiglu")?;
        module.insert_submodule("w1", linear_module("w1", d_model, d_ff)?)?;
        module.insert_submodule("w2", linear_module("w2", d_ff, d_model)?)?;
        module.insert_submodule("w3", linear_module("w3", d_model, d_ff)?)?;
        Ok(Self {
            module,
            d_model,
            d_ff,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn state_dict(&self) -> ModuleStateDict {
        self.module.state_dict()
    }

    pub fn load_state_dict(
        &mut self,
        state_dict: &ModuleStateDict,
        mode: ModuleStateLoadMode,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        self.module.load_state_dict(state_dict, mode)
    }

    pub fn forward(&self, input: &NnTensor) -> Result<NnTensor, Cs336A1ReferenceError> {
        let w1 = module_parameter_f32(&self.module, "w1.weight", &[self.d_ff, self.d_model])?;
        let w2 = module_parameter_f32(&self.module, "w2.weight", &[self.d_model, self.d_ff])?;
        let w3 = module_parameter_f32(&self.module, "w3.weight", &[self.d_ff, self.d_model])?;
        cs336_a1_swiglu(input, self.d_model, self.d_ff, w1, w2, w3)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1RotarySelfAttention {
    module: Module,
    d_model: usize,
    num_heads: usize,
    rope_theta: f32,
}

impl Cs336A1RotarySelfAttention {
    pub fn new(
        module_id: impl Into<String>,
        d_model: usize,
        num_heads: usize,
        rope_theta: f32,
    ) -> Result<Self, Cs336A1ReferenceError> {
        if d_model == 0 || num_heads == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "rotary_self_attention",
                detail: String::from("d_model and num_heads must be positive"),
            });
        }
        if !d_model.is_multiple_of(num_heads) {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "rotary_self_attention",
                detail: format!("d_model {d_model} must be divisible by num_heads {num_heads}"),
            });
        }
        let head_dim = d_model / num_heads;
        if !head_dim.is_multiple_of(2) {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "rotary_self_attention",
                detail: format!("head_dim {head_dim} must be even for RoPE"),
            });
        }
        let mut module = Module::new(module_id, "cs336_a1_rotary_self_attention")?;
        module.insert_submodule("q_proj", linear_module("q_proj", d_model, d_model)?)?;
        module.insert_submodule("k_proj", linear_module("k_proj", d_model, d_model)?)?;
        module.insert_submodule("v_proj", linear_module("v_proj", d_model, d_model)?)?;
        module.insert_submodule(
            "output_proj",
            linear_module("output_proj", d_model, d_model)?,
        )?;
        Ok(Self {
            module,
            d_model,
            num_heads,
            rope_theta,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn state_dict(&self) -> ModuleStateDict {
        self.module.state_dict()
    }

    pub fn load_state_dict(
        &mut self,
        state_dict: &ModuleStateDict,
        mode: ModuleStateLoadMode,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        self.module.load_state_dict(state_dict, mode)
    }

    pub fn forward(
        &self,
        input: &NnTensor,
        token_positions: Option<&[usize]>,
    ) -> Result<NnTensor, Cs336A1ReferenceError> {
        let q_proj =
            module_parameter_f32(&self.module, "q_proj.weight", &[self.d_model, self.d_model])?;
        let k_proj =
            module_parameter_f32(&self.module, "k_proj.weight", &[self.d_model, self.d_model])?;
        let v_proj =
            module_parameter_f32(&self.module, "v_proj.weight", &[self.d_model, self.d_model])?;
        let output_proj = module_parameter_f32(
            &self.module,
            "output_proj.weight",
            &[self.d_model, self.d_model],
        )?;
        cs336_a1_multihead_self_attention_with_rope(
            input,
            self.d_model,
            self.num_heads,
            self.rope_theta,
            q_proj,
            k_proj,
            v_proj,
            output_proj,
            token_positions,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1TransformerBlock {
    module: Module,
    d_model: usize,
    num_heads: usize,
    d_ff: usize,
    rope_theta: f32,
    rms_norm_eps: f32,
}

impl Cs336A1TransformerBlock {
    pub fn new(
        module_id: impl Into<String>,
        d_model: usize,
        num_heads: usize,
        d_ff: usize,
        rope_theta: f32,
        rms_norm_eps: f32,
    ) -> Result<Self, Cs336A1ReferenceError> {
        if rms_norm_eps <= 0.0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_block",
                detail: String::from("rms_norm_eps must be positive"),
            });
        }
        let mut module = Module::new(module_id, "cs336_a1_transformer_block")?;
        module.insert_submodule(
            "attn",
            Cs336A1RotarySelfAttention::new("attn", d_model, num_heads, rope_theta)?
                .module()
                .clone(),
        )?;
        module.insert_submodule("ln1", rms_norm_module("ln1", d_model)?)?;
        module.insert_submodule(
            "ffn",
            Cs336A1SwiGlu::new("ffn", d_model, d_ff)?.module().clone(),
        )?;
        module.insert_submodule("ln2", rms_norm_module("ln2", d_model)?)?;
        Ok(Self {
            module,
            d_model,
            num_heads,
            d_ff,
            rope_theta,
            rms_norm_eps,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn state_dict(&self) -> ModuleStateDict {
        self.module.state_dict()
    }

    pub fn load_state_dict(
        &mut self,
        state_dict: &ModuleStateDict,
        mode: ModuleStateLoadMode,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        self.module.load_state_dict(state_dict, mode)
    }

    pub fn forward(
        &self,
        input: &NnTensor,
        token_positions: Option<&[usize]>,
    ) -> Result<NnTensor, Cs336A1ReferenceError> {
        let normalized_attn_input = cs336_a1_rms_norm(
            input,
            self.d_model,
            self.rms_norm_eps,
            module_parameter_f32(&self.module, "ln1.weight", &[self.d_model])?,
        )?;
        let attention = Cs336A1RotarySelfAttention {
            module: self.module.submodule("attn")?.clone(),
            d_model: self.d_model,
            num_heads: self.num_heads,
            rope_theta: self.rope_theta,
        };
        let attention_output = attention.forward(&normalized_attn_input, token_positions)?;
        let first_residual = add_tensors(input, &attention_output)?;
        let normalized_ffn_input = cs336_a1_rms_norm(
            &first_residual,
            self.d_model,
            self.rms_norm_eps,
            module_parameter_f32(&self.module, "ln2.weight", &[self.d_model])?,
        )?;
        let feed_forward = Cs336A1SwiGlu {
            module: self.module.submodule("ffn")?.clone(),
            d_model: self.d_model,
            d_ff: self.d_ff,
        };
        let ffn_output = feed_forward.forward(&normalized_ffn_input)?;
        add_tensors(&first_residual, &ffn_output)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A1TransformerLm {
    module: Module,
    config: Cs336A1ReferenceConfig,
    rope_theta: f32,
    rms_norm_eps: f32,
}

impl Cs336A1TransformerLm {
    pub fn new(
        module_id: impl Into<String>,
        config: Cs336A1ReferenceConfig,
        rope_theta: f32,
        rms_norm_eps: f32,
    ) -> Result<Self, Cs336A1ReferenceError> {
        let config = config.validate()?;
        if rms_norm_eps <= 0.0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("rms_norm_eps must be positive"),
            });
        }
        let mut module = Module::new(module_id, "cs336_a1_transformer_lm")?;
        module.insert_submodule(
            "token_embeddings",
            embedding_module("token_embeddings", config.vocab_size, config.d_model)?,
        )?;
        let mut layers = Module::new("layers", "module_list")?;
        for layer_index in 0..config.num_layers {
            let block = Cs336A1TransformerBlock::new(
                layer_index.to_string(),
                config.d_model,
                config.num_heads,
                config.d_ff,
                rope_theta,
                rms_norm_eps,
            )?;
            layers.insert_submodule(layer_index.to_string(), block.module().clone())?;
        }
        module.insert_submodule("layers", layers)?;
        module.insert_submodule("ln_final", rms_norm_module("ln_final", config.d_model)?)?;
        module.insert_submodule(
            "lm_head",
            linear_module("lm_head", config.d_model, config.vocab_size)?,
        )?;
        Ok(Self {
            module,
            config,
            rope_theta,
            rms_norm_eps,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub const fn config(&self) -> Cs336A1ReferenceConfig {
        self.config
    }

    pub fn state_dict(&self) -> ModuleStateDict {
        self.module.state_dict()
    }

    pub fn load_state_dict(
        &mut self,
        state_dict: &ModuleStateDict,
        mode: ModuleStateLoadMode,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        self.module.load_state_dict(state_dict, mode)
    }

    pub fn forward_tokens(
        &self,
        token_shape: Shape,
        token_ids: &[usize],
    ) -> Result<NnTensor, Cs336A1ReferenceError> {
        let final_hidden = self.final_hidden_for_tokens(token_shape, token_ids)?;
        self.logits_from_final_hidden(&final_hidden)
    }

    /// Forward pass that returns both full-sequence logits and the
    /// last-token (penultimate-position) hidden state.
    ///
    /// This is the P1 hidden-state-extraction primitive for the learned
    /// coordinator (Khala M6 / TRINITY substrate). TRINITY routes on the
    /// hidden vector of the final prompt token, so the returned hidden tensor
    /// has shape `[batch, d_model]` (the sequence axis is collapsed to the
    /// last position). The logits tensor keeps the standard
    /// `[batch, seq, vocab]` shape so existing callers are unaffected.
    ///
    /// The forward is computed once; logits and hidden state are read off the
    /// same activations, so this costs the same as `forward_tokens` plus a
    /// cheap slice. Given identical weights and inputs it is deterministic,
    /// which is required for stable evolution-strategy fitness on the head.
    pub fn forward_with_hidden(
        &self,
        token_shape: Shape,
        token_ids: &[usize],
    ) -> Result<(NnTensor, NnTensor), Cs336A1ReferenceError> {
        let final_hidden = self.final_hidden_for_tokens(token_shape, token_ids)?;
        let last_token_hidden = self.last_token_hidden(&final_hidden)?;
        let logits = self.logits_from_final_hidden(&final_hidden)?;
        Ok((logits, last_token_hidden))
    }

    /// Extracts the hidden vector of the final sequence position for every
    /// batch row, collapsing a `[.., seq, d_model]` hidden tensor down to
    /// `[batch, d_model]` (batch is the product of all leading dimensions).
    fn last_token_hidden(
        &self,
        final_hidden: &NnTensor,
    ) -> Result<NnTensor, Cs336A1ReferenceError> {
        let dims = final_hidden.dims();
        if dims.len() < 2 || dims[dims.len() - 1] != self.config.d_model {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: format!(
                    "expected hidden state rank >= 2 with trailing width {}, found {:?}",
                    self.config.d_model, dims
                ),
            });
        }
        let d_model = self.config.d_model;
        let sequence_length = dims[dims.len() - 2];
        if sequence_length == 0 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: String::from("sequence dimension must be non-empty"),
            });
        }
        let batch = dims[..dims.len() - 2].iter().product::<usize>().max(1);
        let values = final_hidden.as_f32_slice()?;
        let mut output = vec![0.0_f32; batch * d_model];
        for row in 0..batch {
            let row_offset = row * sequence_length * d_model;
            let last_token_offset = row_offset + (sequence_length - 1) * d_model;
            output[row * d_model..(row + 1) * d_model]
                .copy_from_slice(&values[last_token_offset..last_token_offset + d_model]);
        }
        Ok(NnTensor::f32(Shape::new(vec![batch, d_model]), output)?)
    }

    pub fn final_hidden_for_tokens(
        &self,
        token_shape: Shape,
        token_ids: &[usize],
    ) -> Result<NnTensor, Cs336A1ReferenceError> {
        let dims = token_shape.dims();
        if dims.len() < 2 {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: format!("token input rank must be at least 2, found {}", dims.len()),
            });
        }
        let sequence_length =
            *dims
                .last()
                .ok_or_else(|| Cs336A1ReferenceError::InvalidConfiguration {
                    context: "transformer_lm",
                    detail: String::from("missing sequence dimension"),
                })?;
        if sequence_length > self.config.context_length {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: format!(
                    "sequence length {sequence_length} exceeds configured context length {}",
                    self.config.context_length
                ),
            });
        }

        let embedding_weight = module_parameter_f32(
            &self.module,
            "token_embeddings.weight",
            &[self.config.vocab_size, self.config.d_model],
        )?;
        let mut hidden = cs336_a1_embedding(
            token_shape.clone(),
            token_ids,
            self.config.vocab_size,
            self.config.d_model,
            embedding_weight,
        )?;
        let positions = default_token_positions(token_shape.dims())?;
        for layer_index in 0..self.config.num_layers {
            let block = Cs336A1TransformerBlock {
                module: self
                    .module
                    .submodule(format!("layers.{layer_index}").as_str())?
                    .clone(),
                d_model: self.config.d_model,
                num_heads: self.config.num_heads,
                d_ff: self.config.d_ff,
                rope_theta: self.rope_theta,
                rms_norm_eps: self.rms_norm_eps,
            };
            hidden = block.forward(&hidden, Some(positions.as_slice()))?;
        }
        cs336_a1_rms_norm(
            &hidden,
            self.config.d_model,
            self.rms_norm_eps,
            module_parameter_f32(&self.module, "ln_final.weight", &[self.config.d_model])?,
        )
    }

    pub fn logits_from_final_hidden(
        &self,
        final_hidden: &NnTensor,
    ) -> Result<NnTensor, Cs336A1ReferenceError> {
        let dims = final_hidden.dims();
        if dims.is_empty() || dims[dims.len() - 1] != self.config.d_model {
            return Err(Cs336A1ReferenceError::InvalidConfiguration {
                context: "transformer_lm",
                detail: format!(
                    "expected final hidden trailing width {}, found {:?}",
                    self.config.d_model, dims
                ),
            });
        }
        let lm_head = module_parameter_f32(
            &self.module,
            "lm_head.weight",
            &[self.config.vocab_size, self.config.d_model],
        )?;
        cs336_a1_linear(
            &final_hidden,
            self.config.d_model,
            self.config.vocab_size,
            lm_head,
        )
    }
}

pub fn cs336_a1_linear(
    input: &NnTensor,
    d_in: usize,
    d_out: usize,
    weight: &[f32],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let linear = Linear::from_f32_parts("cs336_a1_linear", d_in, d_out, weight.to_vec(), None)?;
    Ok(linear.forward(input)?)
}

pub fn cs336_a1_embedding(
    index_shape: Shape,
    token_ids: &[usize],
    vocab_size: usize,
    d_model: usize,
    weight: &[f32],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let embedding =
        Embedding::from_f32_table("cs336_a1_embedding", vocab_size, d_model, weight.to_vec())?;
    Ok(embedding.forward_with_shape(index_shape, token_ids)?)
}

pub fn cs336_a1_silu(input: &NnTensor) -> Result<NnTensor, Cs336A1ReferenceError> {
    let silu = Activation::new("cs336_a1_silu", ActivationKind::Silu)?;
    Ok(silu.forward(input)?)
}

pub fn cs336_a1_rms_norm(
    input: &NnTensor,
    d_model: usize,
    eps: f32,
    weight: &[f32],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    if eps <= 0.0 {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "rms_norm",
            detail: String::from("eps must be positive"),
        });
    }
    let dims = input.dims();
    if dims.is_empty() || dims[dims.len() - 1] != d_model {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "rms_norm",
            detail: format!("expected trailing dimension {d_model}, found {:?}", dims),
        });
    }
    if weight.len() != d_model {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "rms_norm",
            detail: format!("expected weight length {d_model}, found {}", weight.len()),
        });
    }
    let values = input.as_f32_slice()?;
    let rows = dims[..dims.len() - 1].iter().product::<usize>().max(1);
    let mut output = vec![0.0; values.len()];
    for row in 0..rows {
        let offset = row * d_model;
        let chunk = &values[offset..offset + d_model];
        let mean_square = chunk.iter().map(|value| value * value).sum::<f32>() / d_model as f32;
        let scale = 1.0 / (mean_square + eps).sqrt();
        for index in 0..d_model {
            output[offset + index] = chunk[index] * scale * weight[index];
        }
    }
    Ok(NnTensor::f32(Shape::new(dims.to_vec()), output)?)
}

pub fn cs336_a1_swiglu(
    input: &NnTensor,
    d_model: usize,
    d_ff: usize,
    w1_weight: &[f32],
    w2_weight: &[f32],
    w3_weight: &[f32],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let gate = cs336_a1_silu(&cs336_a1_linear(input, d_model, d_ff, w1_weight)?)?;
    let value = cs336_a1_linear(input, d_model, d_ff, w3_weight)?;
    let gated = mul_tensors(&gate, &value)?;
    cs336_a1_linear(&gated, d_ff, d_model, w2_weight)
}

pub fn cs336_a1_rope(
    input: &NnTensor,
    theta: f32,
    token_positions: &[usize],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let dims = input.dims();
    if dims.len() < 2 {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "rope",
            detail: format!("expected rank >= 2, found {}", dims.len()),
        });
    }
    let sequence_length = dims[dims.len() - 2];
    let d_model = dims[dims.len() - 1];
    if !d_model.is_multiple_of(2) {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "rope",
            detail: format!("embedding width {d_model} must be even"),
        });
    }
    let batch = dims[..dims.len() - 2].iter().product::<usize>().max(1);
    let positions = normalize_positions(batch, sequence_length, token_positions)?;
    let values = input.as_f32_slice()?;
    let mut output = values.to_vec();
    let half = d_model / 2;
    for batch_index in 0..batch {
        for seq_index in 0..sequence_length {
            let position = positions[batch_index * sequence_length + seq_index] as f32;
            let base = (batch_index * sequence_length + seq_index) * d_model;
            for pair in 0..half {
                let exponent = (2 * pair) as f32 / d_model as f32;
                let angle = position / theta.powf(exponent);
                let cosine = angle.cos();
                let sine = angle.sin();
                let even = values[base + 2 * pair];
                let odd = values[base + 2 * pair + 1];
                output[base + 2 * pair] = even * cosine - odd * sine;
                output[base + 2 * pair + 1] = even * sine + odd * cosine;
            }
        }
    }
    Ok(NnTensor::f32(Shape::new(dims.to_vec()), output)?)
}

pub fn cs336_a1_scaled_dot_product_attention(
    query: &NnTensor,
    key: &NnTensor,
    value: &NnTensor,
    mask: Option<&AttentionMask>,
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let query_shape = normalize_attention_tensor(query)?;
    let key_shape = normalize_attention_tensor(key)?;
    let value_shape = normalize_attention_tensor(value)?;
    let output = scaled_dot_product_attention(
        &query_shape.tensor,
        &key_shape.tensor,
        &value_shape.tensor,
        mask,
    )?;
    attention_tensor4_to_nn_tensor(&output.context, &query_shape.output_prefix)
}

pub fn cs336_a1_multihead_self_attention(
    input: &NnTensor,
    d_model: usize,
    num_heads: usize,
    q_proj_weight: &[f32],
    k_proj_weight: &[f32],
    v_proj_weight: &[f32],
    o_proj_weight: &[f32],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    cs336_a1_multihead_self_attention_impl(
        input,
        d_model,
        num_heads,
        q_proj_weight,
        k_proj_weight,
        v_proj_weight,
        o_proj_weight,
        None,
        None,
    )
}

pub fn cs336_a1_multihead_self_attention_with_rope(
    input: &NnTensor,
    d_model: usize,
    num_heads: usize,
    rope_theta: f32,
    q_proj_weight: &[f32],
    k_proj_weight: &[f32],
    v_proj_weight: &[f32],
    o_proj_weight: &[f32],
    token_positions: Option<&[usize]>,
) -> Result<NnTensor, Cs336A1ReferenceError> {
    cs336_a1_multihead_self_attention_impl(
        input,
        d_model,
        num_heads,
        q_proj_weight,
        k_proj_weight,
        v_proj_weight,
        o_proj_weight,
        Some(rope_theta),
        token_positions,
    )
}

fn cs336_a1_multihead_self_attention_impl(
    input: &NnTensor,
    d_model: usize,
    num_heads: usize,
    q_proj_weight: &[f32],
    k_proj_weight: &[f32],
    v_proj_weight: &[f32],
    o_proj_weight: &[f32],
    rope_theta: Option<f32>,
    token_positions: Option<&[usize]>,
) -> Result<NnTensor, Cs336A1ReferenceError> {
    if input.dims().len() < 3 {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "multihead_self_attention",
            detail: format!("expected rank >= 3, found {}", input.dims().len()),
        });
    }
    if input.dims()[input.dims().len() - 1] != d_model {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "multihead_self_attention",
            detail: format!(
                "expected trailing model width {d_model}, found {:?}",
                input.dims()
            ),
        });
    }
    if !d_model.is_multiple_of(num_heads) {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "multihead_self_attention",
            detail: format!("d_model {d_model} must be divisible by num_heads {num_heads}"),
        });
    }
    let projected_q = cs336_a1_linear(input, d_model, d_model, q_proj_weight)?;
    let projected_k = cs336_a1_linear(input, d_model, d_model, k_proj_weight)?;
    let projected_v = cs336_a1_linear(input, d_model, d_model, v_proj_weight)?;
    let head_dim = d_model / num_heads;
    let query_shape = reshape_for_attention(&projected_q, num_heads, head_dim)?;
    let key_shape = reshape_for_attention(&projected_k, num_heads, head_dim)?;
    let mut query_tensor = query_shape.tensor;
    let mut key_tensor = key_shape.tensor.clone();

    if let Some(theta) = rope_theta {
        let positions = normalize_positions(
            query_shape.batch_size,
            query_shape.sequence_length,
            token_positions.unwrap_or(&[]),
        )?;
        query_tensor = apply_rope_to_attention_tensor(&query_tensor, theta, positions.as_slice())?;
        key_tensor = apply_rope_to_attention_tensor(&key_tensor, theta, positions.as_slice())?;
    }

    let value_shape = reshape_for_attention(&projected_v, num_heads, head_dim)?;
    let mask = AttentionMask::causal(
        query_shape.batch_size,
        query_shape.sequence_length,
        query_shape.sequence_length,
    );
    let attended =
        scaled_dot_product_attention(&query_tensor, &key_tensor, &value_shape.tensor, Some(&mask))?;
    let merged = merge_attention_output(
        &attended.context,
        query_shape.batch_size,
        query_shape.sequence_length,
        num_heads,
        head_dim,
        query_shape.output_prefix.as_slice(),
    )?;
    cs336_a1_linear(&merged, d_model, d_model, o_proj_weight)
}

struct NormalizedAttentionTensor {
    tensor: AttentionTensor4,
    output_prefix: Vec<usize>,
}

struct ReshapedSelfAttention {
    tensor: AttentionTensor4,
    batch_size: usize,
    sequence_length: usize,
    output_prefix: Vec<usize>,
}

fn normalize_attention_tensor(
    tensor: &NnTensor,
) -> Result<NormalizedAttentionTensor, Cs336A1ReferenceError> {
    let dims = tensor.dims();
    let values = tensor.as_f32_slice()?;
    match dims.len() {
        3 => Ok(NormalizedAttentionTensor {
            tensor: AttentionTensor4::new([dims[0], 1, dims[1], dims[2]], values.to_vec())?,
            output_prefix: vec![dims[0], dims[1]],
        }),
        4 => Ok(NormalizedAttentionTensor {
            tensor: AttentionTensor4::new([dims[0], dims[1], dims[2], dims[3]], values.to_vec())?,
            output_prefix: vec![dims[0], dims[1], dims[2]],
        }),
        _ => Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "scaled_dot_product_attention",
            detail: format!("expected rank 3 or 4, found {}", dims.len()),
        }),
    }
}

fn attention_tensor4_to_nn_tensor(
    tensor: &AttentionTensor4,
    output_prefix: &[usize],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let mut dims = output_prefix.to_vec();
    dims.push(tensor.col_count());
    Ok(NnTensor::f32(Shape::new(dims), tensor.values().to_vec())?)
}

fn reshape_for_attention(
    tensor: &NnTensor,
    num_heads: usize,
    head_dim: usize,
) -> Result<ReshapedSelfAttention, Cs336A1ReferenceError> {
    let dims = tensor.dims();
    let batch_size = dims[..dims.len() - 2].iter().product::<usize>().max(1);
    let sequence_length = dims[dims.len() - 2];
    let values = tensor.as_f32_slice()?;
    let mut reshaped = AttentionTensor4::zeros([batch_size, num_heads, sequence_length, head_dim]);
    for batch in 0..batch_size {
        for seq in 0..sequence_length {
            let src_offset = (batch * sequence_length + seq) * (num_heads * head_dim);
            for head in 0..num_heads {
                let head_offset = src_offset + head * head_dim;
                for feature in 0..head_dim {
                    reshaped.set(batch, head, seq, feature, values[head_offset + feature]);
                }
            }
        }
    }
    Ok(ReshapedSelfAttention {
        tensor: reshaped,
        batch_size,
        sequence_length,
        output_prefix: dims[..dims.len() - 1].to_vec(),
    })
}

fn apply_rope_to_attention_tensor(
    tensor: &AttentionTensor4,
    theta: f32,
    token_positions: &[usize],
) -> Result<AttentionTensor4, Cs336A1ReferenceError> {
    let batch_size = tensor.batch_size();
    let sequence_length = tensor.row_count();
    let head_dim = tensor.col_count();
    let positions = normalize_positions(batch_size, sequence_length, token_positions)?;
    let mut output = tensor.clone();
    let half = head_dim / 2;
    for batch in 0..batch_size {
        for head in 0..tensor.head_count() {
            for seq in 0..sequence_length {
                let position = positions[batch * sequence_length + seq] as f32;
                for pair in 0..half {
                    let exponent = (2 * pair) as f32 / head_dim as f32;
                    let angle = position / theta.powf(exponent);
                    let cosine = angle.cos();
                    let sine = angle.sin();
                    let even = tensor.get(batch, head, seq, 2 * pair);
                    let odd = tensor.get(batch, head, seq, 2 * pair + 1);
                    output.set(batch, head, seq, 2 * pair, even * cosine - odd * sine);
                    output.set(batch, head, seq, 2 * pair + 1, even * sine + odd * cosine);
                }
            }
        }
    }
    Ok(output)
}

fn merge_attention_output(
    tensor: &AttentionTensor4,
    batch_size: usize,
    sequence_length: usize,
    num_heads: usize,
    head_dim: usize,
    output_prefix: &[usize],
) -> Result<NnTensor, Cs336A1ReferenceError> {
    let mut output = vec![0.0; batch_size * sequence_length * num_heads * head_dim];
    for batch in 0..batch_size {
        for seq in 0..sequence_length {
            let dst_offset = (batch * sequence_length + seq) * num_heads * head_dim;
            for head in 0..num_heads {
                for feature in 0..head_dim {
                    output[dst_offset + head * head_dim + feature] =
                        tensor.get(batch, head, seq, feature);
                }
            }
        }
    }
    let mut dims = output_prefix.to_vec();
    dims.push(num_heads * head_dim);
    Ok(NnTensor::f32(Shape::new(dims), output)?)
}

fn normalize_positions(
    batch_size: usize,
    sequence_length: usize,
    token_positions: &[usize],
) -> Result<Vec<usize>, Cs336A1ReferenceError> {
    if token_positions.is_empty() {
        let mut positions = Vec::with_capacity(batch_size * sequence_length);
        for _ in 0..batch_size {
            positions.extend(0..sequence_length);
        }
        return Ok(positions);
    }
    if token_positions.len() == sequence_length {
        let mut positions = Vec::with_capacity(batch_size * sequence_length);
        for _ in 0..batch_size {
            positions.extend_from_slice(token_positions);
        }
        return Ok(positions);
    }
    if token_positions.len() == batch_size * sequence_length {
        return Ok(token_positions.to_vec());
    }
    Err(Cs336A1ReferenceError::InvalidConfiguration {
        context: "rope",
        detail: format!(
            "token position length {} does not match sequence length {} or batch*sequence {}",
            token_positions.len(),
            sequence_length,
            batch_size * sequence_length
        ),
    })
}

fn default_token_positions(token_dims: &[usize]) -> Result<Vec<usize>, Cs336A1ReferenceError> {
    if token_dims.len() < 2 {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "transformer_lm",
            detail: String::from("token ids must have rank at least 2"),
        });
    }
    let batch_size = token_dims[..token_dims.len() - 1]
        .iter()
        .product::<usize>()
        .max(1);
    let sequence_length = token_dims[token_dims.len() - 1];
    normalize_positions(batch_size, sequence_length, &[])
}

fn add_tensors(left: &NnTensor, right: &NnTensor) -> Result<NnTensor, Cs336A1ReferenceError> {
    if left.dims() != right.dims() {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "tensor_add",
            detail: format!(
                "left shape {:?} does not match right shape {:?}",
                left.dims(),
                right.dims()
            ),
        });
    }
    let values = left
        .as_f32_slice()?
        .iter()
        .zip(right.as_f32_slice()?.iter())
        .map(|(left, right)| left + right)
        .collect::<Vec<_>>();
    Ok(NnTensor::f32(Shape::new(left.dims().to_vec()), values)?)
}

fn mul_tensors(left: &NnTensor, right: &NnTensor) -> Result<NnTensor, Cs336A1ReferenceError> {
    if left.dims() != right.dims() {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "tensor_mul",
            detail: format!(
                "left shape {:?} does not match right shape {:?}",
                left.dims(),
                right.dims()
            ),
        });
    }
    let values = left
        .as_f32_slice()?
        .iter()
        .zip(right.as_f32_slice()?.iter())
        .map(|(left, right)| left * right)
        .collect::<Vec<_>>();
    Ok(NnTensor::f32(Shape::new(left.dims().to_vec()), values)?)
}

fn linear_module(
    module_id: impl Into<String>,
    in_features: usize,
    out_features: usize,
) -> Result<Module, Cs336A1ReferenceError> {
    let mut module = Module::new(module_id, "linear")?;
    module.insert_parameter(
        "weight",
        dense_trainable_parameter(
            &[out_features, in_features],
            vec![0.0; out_features * in_features],
        )?,
    )?;
    Ok(module)
}

fn embedding_module(
    module_id: impl Into<String>,
    vocab_size: usize,
    embedding_dim: usize,
) -> Result<Module, Cs336A1ReferenceError> {
    let mut module = Module::new(module_id, "embedding")?;
    module.insert_parameter(
        "weight",
        dense_trainable_parameter(
            &[vocab_size, embedding_dim],
            vec![0.0; vocab_size * embedding_dim],
        )?,
    )?;
    Ok(module)
}

fn rms_norm_module(
    module_id: impl Into<String>,
    d_model: usize,
) -> Result<Module, Cs336A1ReferenceError> {
    let mut module = Module::new(module_id, "rms_norm")?;
    module.insert_parameter(
        "weight",
        dense_trainable_parameter(&[d_model], vec![1.0; d_model])?,
    )?;
    Ok(module)
}

fn dense_trainable_parameter(
    shape: &[usize],
    values: Vec<f32>,
) -> Result<ModuleParameter, Cs336A1ReferenceError> {
    Ok(ModuleParameter::new(
        TensorSpec::new(Shape::new(shape.to_vec()), DType::F32, Device::cpu()),
        TensorData::F32(values),
        true,
    )?)
}

fn module_parameter_f32<'a>(
    module: &'a Module,
    path: &str,
    expected_shape: &[usize],
) -> Result<&'a [f32], Cs336A1ReferenceError> {
    let parameter = module.parameter(path)?;
    if parameter.spec.shape().dims() != expected_shape {
        return Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "state_dict",
            detail: format!(
                "parameter `{path}` expected shape {:?}, found {:?}",
                expected_shape,
                parameter.spec.shape().dims()
            ),
        });
    }
    match &parameter.data {
        TensorData::F32(values) => Ok(values.as_slice()),
        _ => Err(Cs336A1ReferenceError::InvalidConfiguration {
            context: "state_dict",
            detail: format!("parameter `{path}` must be dense f32"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cs336_a1_embedding, cs336_a1_linear, cs336_a1_multihead_self_attention,
        cs336_a1_multihead_self_attention_with_rope, cs336_a1_rms_norm, cs336_a1_rope,
        cs336_a1_scaled_dot_product_attention, cs336_a1_silu, cs336_a1_swiglu,
        Cs336A1ReferenceConfig, Cs336A1TransformerBlock, Cs336A1TransformerLm,
    };
    use psionic_core::{Shape, TensorData};
    use psionic_nn::{ModuleStateLoadMode, NnTensor};
    use psionic_transformer::AttentionMask;

    #[test]
    fn linear_matches_manual_projection() {
        let input =
            NnTensor::f32(Shape::new(vec![1, 2, 2]), vec![1.0, 2.0, 3.0, 4.0]).expect("tensor");
        let output = cs336_a1_linear(&input, 2, 2, &[1.0, 0.0, 0.0, 1.0]).expect("linear");
        assert_eq!(
            output.as_f32_slice().expect("output"),
            &[1.0, 2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn embedding_matches_table_lookup() {
        let output = cs336_a1_embedding(
            Shape::new(vec![2]),
            &[2, 0],
            3,
            2,
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .expect("embedding");
        assert_eq!(
            output.as_f32_slice().expect("output"),
            &[5.0, 6.0, 1.0, 2.0]
        );
    }

    #[test]
    fn silu_matches_reference_formula() {
        let input = NnTensor::f32(Shape::new(vec![2]), vec![0.0, 1.0]).expect("tensor");
        let output = cs336_a1_silu(&input).expect("silu");
        let values = output.as_f32_slice().expect("output");
        assert!((values[0] - 0.0).abs() < 1e-6);
        assert!((values[1] - (1.0 / (1.0 + (-1.0f32).exp()))).abs() < 1e-6);
    }

    #[test]
    fn rms_norm_scales_by_root_mean_square() {
        let input = NnTensor::f32(Shape::new(vec![1, 2]), vec![3.0, 4.0]).expect("tensor");
        let output = cs336_a1_rms_norm(&input, 2, 1e-6, &[1.0, 2.0]).expect("rmsnorm");
        let scale = 1.0 / ((12.5_f32 + 1e-6).sqrt());
        let values = output.as_f32_slice().expect("output");
        assert!((values[0] - 3.0 * scale).abs() < 1e-6);
        assert!((values[1] - 8.0 * scale).abs() < 1e-6);
    }

    #[test]
    fn swiglu_composes_gate_and_value_paths() {
        let input = NnTensor::f32(Shape::new(vec![1, 2]), vec![1.0, 2.0]).expect("tensor");
        let output = cs336_a1_swiglu(
            &input,
            2,
            2,
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
        )
        .expect("swiglu");
        let values = output.as_f32_slice().expect("output");
        assert!(values[0] > 0.73 && values[0] < 0.74);
        assert!(values[1] > 3.52 && values[1] < 3.53);
    }

    #[test]
    fn rope_rotates_even_odd_pairs() {
        let input = NnTensor::f32(
            Shape::new(vec![1, 2, 4]),
            vec![1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0],
        )
        .expect("tensor");
        let output = cs336_a1_rope(&input, 10_000.0, &[0, 1]).expect("rope");
        let values = output.as_f32_slice().expect("output");
        assert!((values[0] - 1.0).abs() < 1e-6);
        assert!((values[1] - 0.0).abs() < 1e-6);
        assert!((values[4] - 1.0f32.cos()).abs() < 1e-6);
        assert!((values[5] - 1.0f32.sin()).abs() < 1e-6);
    }

    #[test]
    fn scaled_dot_product_attention_matches_manual_example() {
        let query =
            NnTensor::f32(Shape::new(vec![1, 2, 2]), vec![1.0, 0.0, 0.0, 1.0]).expect("query");
        let key = query.clone();
        let value =
            NnTensor::f32(Shape::new(vec![1, 2, 2]), vec![10.0, 0.0, 0.0, 20.0]).expect("value");
        let output = cs336_a1_scaled_dot_product_attention(
            &query,
            &key,
            &value,
            Some(&AttentionMask::causal(1, 2, 2)),
        )
        .expect("attention");
        let values = output.as_f32_slice().expect("output");
        assert!((values[0] - 10.0).abs() < 1e-6);
        assert!((values[1] - 0.0).abs() < 1e-6);
        assert!(values[3] > 13.3 && values[3] < 13.5);
    }

    #[test]
    fn multihead_self_attention_supports_identity_projection_path() {
        let input =
            NnTensor::f32(Shape::new(vec![1, 2, 2]), vec![1.0, 0.0, 0.0, 1.0]).expect("tensor");
        let output = cs336_a1_multihead_self_attention(
            &input,
            2,
            1,
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
        )
        .expect("attention");
        assert_eq!(output.dims(), &[1, 2, 2]);
    }

    #[test]
    fn multihead_self_attention_with_rope_executes_end_to_end() {
        let input =
            NnTensor::f32(Shape::new(vec![1, 2, 2]), vec![1.0, 0.0, 0.0, 1.0]).expect("tensor");
        let output = cs336_a1_multihead_self_attention_with_rope(
            &input,
            2,
            1,
            10_000.0,
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0, 1.0],
            Some(&[0, 1]),
        )
        .expect("attention");
        assert_eq!(output.dims(), &[1, 2, 2]);
    }

    #[test]
    fn transformer_block_with_zero_submodules_is_identity() {
        let mut block =
            Cs336A1TransformerBlock::new("block", 2, 1, 2, 10_000.0, 1e-6).expect("block");
        let mut weights = block.state_dict();
        for (path, entry) in &mut weights.entries {
            match path.as_str() {
                "ln1.weight" | "ln2.weight" => {
                    entry.data = TensorData::F32(vec![1.0, 1.0]);
                }
                _ if path.ends_with(".weight") => {
                    let len = entry.spec.shape().element_count();
                    entry.data = TensorData::F32(vec![0.0; len]);
                }
                _ => {}
            }
        }
        block
            .load_state_dict(&weights, ModuleStateLoadMode::Strict)
            .expect("load");
        let input =
            NnTensor::f32(Shape::new(vec![1, 2, 2]), vec![1.0, 2.0, 3.0, 4.0]).expect("tensor");
        let output = block.forward(&input, Some(&[0, 1])).expect("forward");
        assert_eq!(
            output.as_f32_slice().expect("output"),
            input.as_f32_slice().expect("input")
        );
    }

    #[test]
    fn transformer_lm_executes_end_to_end_and_exposes_expected_state_dict_keys() {
        let config = Cs336A1ReferenceConfig {
            vocab_size: 3,
            context_length: 4,
            d_model: 2,
            num_layers: 1,
            num_heads: 1,
            d_ff: 2,
        };
        let mut model = Cs336A1TransformerLm::new("lm", config, 10_000.0, 1e-6).expect("model");
        let mut weights = model.state_dict();
        for (path, entry) in &mut weights.entries {
            let len = entry.spec.shape().element_count();
            let values = match path.as_str() {
                "token_embeddings.weight" => vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "lm_head.weight" => vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "ln_final.weight" | "layers.0.ln1.weight" | "layers.0.ln2.weight" => {
                    vec![1.0, 1.0]
                }
                _ if path.ends_with(".weight") => vec![0.0; len],
                _ => vec![0.0; len],
            };
            entry.data = TensorData::F32(values);
        }
        model
            .load_state_dict(&weights, ModuleStateLoadMode::Strict)
            .expect("load");
        let output = model
            .forward_tokens(Shape::new(vec![1, 2]), &[0, 1])
            .expect("forward");
        assert_eq!(output.dims(), &[1, 2, 3]);
        let keys = model.state_dict().keys();
        assert!(keys.contains(&String::from("token_embeddings.weight")));
        assert!(keys.contains(&String::from("layers.0.attn.q_proj.weight")));
        assert!(keys.contains(&String::from("layers.0.ffn.w1.weight")));
        assert!(keys.contains(&String::from("layers.0.ln1.weight")));
        assert!(keys.contains(&String::from("ln_final.weight")));
        assert!(keys.contains(&String::from("lm_head.weight")));
    }

    fn build_two_dim_smoke_lm() -> Cs336A1TransformerLm {
        let config = Cs336A1ReferenceConfig {
            vocab_size: 3,
            context_length: 4,
            d_model: 2,
            num_layers: 1,
            num_heads: 1,
            d_ff: 2,
        };
        let mut model = Cs336A1TransformerLm::new("lm", config, 10_000.0, 1e-6).expect("model");
        let mut weights = model.state_dict();
        for (path, entry) in &mut weights.entries {
            let len = entry.spec.shape().element_count();
            let values = match path.as_str() {
                "token_embeddings.weight" => vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "lm_head.weight" => vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "ln_final.weight" | "layers.0.ln1.weight" | "layers.0.ln2.weight" => {
                    vec![1.0, 1.0]
                }
                _ => vec![0.0; len],
            };
            entry.data = TensorData::F32(values);
        }
        model
            .load_state_dict(&weights, ModuleStateLoadMode::Strict)
            .expect("load");
        model
    }

    #[test]
    fn forward_with_hidden_shapes_and_matches_logit_path() {
        let model = build_two_dim_smoke_lm();
        let (logits, hidden) = model
            .forward_with_hidden(Shape::new(vec![1, 2]), &[0, 1])
            .expect("forward_with_hidden");
        // Logits keep the standard [batch, seq, vocab] shape.
        assert_eq!(logits.dims(), &[1, 2, 3]);
        // Hidden state is collapsed to [batch, d_model].
        assert_eq!(hidden.dims(), &[1, 2]);

        // forward_with_hidden must return exactly the same logits as the
        // existing forward_tokens path (no behavioral drift).
        let reference = model
            .forward_tokens(Shape::new(vec![1, 2]), &[0, 1])
            .expect("forward_tokens");
        assert_eq!(
            logits.as_f32_slice().expect("logits"),
            reference.as_f32_slice().expect("reference"),
        );

        // The returned hidden state must be the last-position slice of the
        // full final-hidden tensor.
        let final_hidden = model
            .final_hidden_for_tokens(Shape::new(vec![1, 2]), &[0, 1])
            .expect("final_hidden");
        let full = final_hidden.as_f32_slice().expect("full");
        let last = &full[(2 - 1) * 2..2 * 2];
        assert_eq!(hidden.as_f32_slice().expect("hidden"), last);
    }

    #[test]
    fn forward_with_hidden_is_deterministic() {
        let model = build_two_dim_smoke_lm();
        let (logits_a, hidden_a) = model
            .forward_with_hidden(Shape::new(vec![1, 2]), &[0, 1])
            .expect("first");
        let (logits_b, hidden_b) = model
            .forward_with_hidden(Shape::new(vec![1, 2]), &[0, 1])
            .expect("second");
        // Determinism is required for stable evolution-strategy fitness on the
        // coordinator head: identical weights + inputs => identical features.
        assert_eq!(
            hidden_a.as_f32_slice().expect("a"),
            hidden_b.as_f32_slice().expect("b"),
        );
        assert_eq!(
            logits_a.as_f32_slice().expect("a"),
            logits_b.as_f32_slice().expect("b"),
        );
    }
}
