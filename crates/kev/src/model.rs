//! The Qwen backbone the decision model runs once per packed request.
//!
//! This is the `.model` half of `Qwen2ForCausalLM` / `Qwen3ForCausalLM` —
//! embeddings, the decoder layers, the final norm — with two changes from
//! the generation path: the caller supplies position ids (each question
//! branch restarts after the state) and an additive attention mask (the
//! block-causal branch mask), and there is no vocabulary head because the
//! model never generates text.
//!
//! Qwen3 differs from Qwen2 by declaring `head_dim`, dropping q/k/v
//! biases, and applying a per-head RMSNorm to q and k before rotary;
//! `Config` carries the differences and `load` reads them from the
//! checkpoint rather than hard-coding either family.
//!
//! Weights load from safetensors into the caller's dtype: `f32` for
//! conformance, `bf16` when serving a large backbone.

use std::path::Path;

use candle_core::{DType, Device, Result as CandleResult, Tensor};
use serde::Deserialize;

use crate::error::{Error, Result};

/// The fields of `config.json` the forward pass reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// The HF architecture the checkpoint declares; dispatch happens on it.
    #[serde(default)]
    pub architectures: Vec<String>,
    /// Embedding width.
    pub hidden_size: usize,
    /// SwiGLU intermediate width.
    pub intermediate_size: usize,
    /// Decoder layer count.
    pub num_hidden_layers: usize,
    /// Query head count.
    pub num_attention_heads: usize,
    /// Key/value head count; queries are grouped when it is smaller.
    pub num_key_value_heads: usize,
    /// Query head width. Qwen3 declares it; Qwen2 derives it.
    #[serde(default)]
    pub head_dim: Option<usize>,
    /// Whether q/k/v projections carry biases. Absent means the Qwen2
    /// convention, which is biased.
    #[serde(default)]
    pub attention_bias: Option<bool>,
    /// RMSNorm epsilon.
    pub rms_norm_eps: f64,
    /// Rotary base.
    pub rope_theta: f64,
    /// Vocabulary rows in the embedding table.
    pub vocab_size: usize,
    /// The backbone's position ceiling; the rotary table sizes to the request.
    pub max_position_embeddings: usize,
}

impl Config {
    /// Query head width: the declared `head_dim` when present (Qwen3), else
    /// `hidden_size / num_attention_heads` (Qwen2).
    #[must_use]
    pub fn head_dim(&self) -> usize {
        self.head_dim
            .unwrap_or(self.hidden_size / self.num_attention_heads)
    }

    /// Whether q/k/v projections carry biases (Qwen2 yes, Qwen3 no).
    #[must_use]
    pub fn attention_bias(&self) -> bool {
        self.attention_bias.unwrap_or(true)
    }

    /// Whether the architecture applies per-head RMSNorm to q and k before
    /// rotary (Qwen3 does, Qwen2 does not).
    #[must_use]
    pub fn qk_norm(&self) -> bool {
        self.architectures
            .iter()
            .any(|a| a == "Qwen3ForCausalLM")
    }
}

/// `y = x @ w.t() + b`, the `nn.Linear` convention.
#[derive(Debug, Clone)]
pub struct Linear {
    /// `[out, in]`.
    pub weight: Tensor,
    /// `[out]`.
    pub bias: Option<Tensor>,
}

impl Linear {
    /// Apply the projection to `[…, in]`.
    pub fn forward(&self, x: &Tensor) -> CandleResult<Tensor> {
        let y = x.matmul(&self.weight.t()?)?;
        match &self.bias {
            Some(b) => y.broadcast_add(b),
            None => Ok(y),
        }
    }
}

fn rms_norm(x: &Tensor, weight: &Tensor, eps: f64) -> CandleResult<Tensor> {
    let x_f32 = x.to_dtype(DType::F32)?;
    let var = x_f32.sqr()?.mean_keepdim(x_f32.rank() - 1)?;
    let normed = x_f32.broadcast_div(&(var + eps)?.sqrt()?)?;
    normed.to_dtype(x.dtype())?.broadcast_mul(weight)
}

/// Per-layer rotary cos/sin tables for positions up to `len`.
struct Rotary {
    cos: Tensor,
    sin: Tensor,
}

impl Rotary {
    fn new(head_dim: usize, theta: f64, len: usize, device: &Device) -> CandleResult<Self> {
        let half = head_dim / 2;
        let inv_freq: Vec<f32> = (0..half)
            .map(|i| (theta as f32).powf(-2.0 * i as f32 / head_dim as f32))
            .collect();
        let inv_freq = Tensor::from_vec(inv_freq, half, device)?;
        let pos = Tensor::arange(0u32, len as u32, device)?.to_dtype(DType::F32)?;
        let freqs = pos.unsqueeze(1)?.matmul(&inv_freq.unsqueeze(0)?)?; // [len, half]
        Ok(Self {
            cos: freqs.cos()?,
            sin: freqs.sin()?,
        })
    }

    /// Rotate `[len, heads, head_dim]` by the per-position tables.
    fn apply(&self, x: &Tensor, pos: &[i64]) -> CandleResult<Tensor> {
        let head_dim = x.dim(x.rank() - 1)?;
        let half = head_dim / 2;
        let pos_t = Tensor::from_vec(pos.to_vec(), pos.len(), x.device())?;
        let cos = self
            .cos
            .index_select(&pos_t, 0)?
            .unsqueeze(1)?
            .to_dtype(x.dtype())?; // [len, 1, half]
        let sin = self
            .sin
            .index_select(&pos_t, 0)?
            .unsqueeze(1)?
            .to_dtype(x.dtype())?;
        let x1 = x.narrow(x.rank() - 1, 0, half)?;
        let x2 = x.narrow(x.rank() - 1, half, half)?;
        let rot1 = (x1.broadcast_mul(&cos)? - x2.broadcast_mul(&sin)?)?;
        let rot2 = (x2.broadcast_mul(&cos)? + x1.broadcast_mul(&sin)?)?;
        Tensor::cat(&[&rot1, &rot2], x.rank() - 1)
    }
}

struct Attention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    /// Per-head RMSNorm on queries before rotary; Qwen3 only.
    q_norm: Option<Tensor>,
    /// Per-head RMSNorm on keys before rotary; Qwen3 only.
    k_norm: Option<Tensor>,
    rms_eps: f64,
    n_heads: usize,
    n_kv_heads: usize,
    head_dim: usize,
}

impl Attention {
    fn forward(
        &self,
        x: &Tensor,
        rotary: &Rotary,
        pos: &[i64],
        mask: &Tensor,
    ) -> CandleResult<Tensor> {
        let len = x.dim(0)?;
        let mut q = self
            .q_proj
            .forward(x)?
            .reshape((len, self.n_heads, self.head_dim))?;
        let mut k = self
            .k_proj
            .forward(x)?
            .reshape((len, self.n_kv_heads, self.head_dim))?;
        if let (Some(qn), Some(kn)) = (&self.q_norm, &self.k_norm) {
            q = rms_norm(&q, qn, self.rms_eps)?;
            k = rms_norm(&k, kn, self.rms_eps)?;
        }
        let q = q;
        let k = k;
        let v = self
            .v_proj
            .forward(x)?
            .reshape((len, self.n_kv_heads, self.head_dim))?;
        let q = rotary.apply(&q, pos)?.transpose(0, 1)?.contiguous()?; // [n_h, len, hd]
        let k = rotary.apply(&k, pos)?.transpose(0, 1)?.contiguous()?; // [n_kv, len, hd]
        let v = v.transpose(0, 1)?.contiguous()?; // [n_kv, len, hd]
        let groups = self.n_heads / self.n_kv_heads;
        let k = repeat_kv(&k, groups)?;
        let v = repeat_kv(&v, groups)?;
        // Metal's batched matmul requires contiguous operands.
        let scores = q
            .matmul(&k.transpose(1, 2)?.contiguous()?)?
            .affine(1.0 / (self.head_dim as f64).sqrt(), 0.0)?; // [n_h, len, len]
        let scores = scores.broadcast_add(mask)?;
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        let out = probs.matmul(&v)?; // [n_h, len, hd]
        let out = out
            .transpose(0, 1)?
            .contiguous()?
            .reshape((len, self.n_heads * self.head_dim))?;
        self.o_proj.forward(&out)
    }
}

/// Repeat each key/value head `groups` times for grouped-query attention.
fn repeat_kv(x: &Tensor, groups: usize) -> CandleResult<Tensor> {
    if groups == 1 {
        return Ok(x.clone());
    }
    let (n_kv, len, hd) = x.dims3()?;
    x.unsqueeze(1)?
        .expand((n_kv, groups, len, hd))?
        .reshape((n_kv * groups, len, hd))
}

struct Mlp {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
}

impl Mlp {
    fn forward(&self, x: &Tensor) -> CandleResult<Tensor> {
        self.down_proj
            .forward(&(self.gate_proj.forward(x)?.silu()? * self.up_proj.forward(x)?)?)
    }
}

struct Layer {
    attn: Attention,
    mlp: Mlp,
    input_layernorm: Tensor,
    post_attention_layernorm: Tensor,
}

/// The decoder trunk: embeddings in, last hidden state out.
pub struct Backbone {
    config: Config,
    embed: Tensor,
    layers: Vec<Layer>,
    norm: Tensor,
    device: Device,
    dtype: DType,
}

fn take(map: &mut std::collections::HashMap<String, Tensor>, name: &str) -> Result<Tensor> {
    map.remove(name)
        .ok_or_else(|| Error::Artifact(format!("missing tensor {name}")))
}

fn linear(
    map: &mut std::collections::HashMap<String, Tensor>,
    name: &str,
    bias: bool,
) -> Result<Linear> {
    let weight = take(map, &format!("{name}.weight"))?;
    let bias = if bias {
        Some(take(map, &format!("{name}.bias"))?)
    } else {
        None
    };
    Ok(Linear { weight, bias })
}

impl Backbone {
    /// Load `config.json` and every `*.safetensors` shard in `dir`, cast to
    /// `dtype`.
    ///
    /// `dtype` is the compute dtype: `F32` for fixture conformance, `BF16`
    /// when serving a large backbone (third-decimal drift, the same caveat
    /// the reference attaches to `KEV_DTYPE`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] when a file or tensor is missing or malformed.
    pub fn load(dir: &Path, device: &Device, dtype: DType) -> Result<Self> {
        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(dir.join("config.json"))
                .map_err(|e| Error::Artifact(format!("read config.json: {e}")))?,
        )?;
        let mut shard_paths: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| Error::Artifact(format!("read_dir {}: {e}", dir.display())))?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|x| x == "safetensors")
                    && p.file_name().is_some_and(|n| {
                        n.to_string_lossy().starts_with("model")
                    })
            })
            .collect();
        shard_paths.sort();
        if shard_paths.is_empty() {
            return Err(Error::Artifact(format!(
                "no model safetensors in {}",
                dir.display()
            )));
        }
        let mut tensors = std::collections::HashMap::new();
        for path in &shard_paths {
            for (k, v) in candle_core::safetensors::load(path, device)
                .map_err(|e| Error::Artifact(format!("load {}: {e}", path.display())))?
            {
                let name = k.strip_prefix("model.").unwrap_or(&k).to_string();
                tensors.insert(name, v.to_dtype(dtype)?);
            }
        }
        let embed = take(&mut tensors, "embed_tokens.weight")?;
        let norm = take(&mut tensors, "norm.weight")?;
        let attn_bias = config.attention_bias();
        let qk_norm = config.qk_norm();
        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for i in 0..config.num_hidden_layers {
            let p = format!("layers.{i}");
            layers.push(Layer {
                attn: Attention {
                    q_proj: linear(&mut tensors, &format!("{p}.self_attn.q_proj"), attn_bias)?,
                    k_proj: linear(&mut tensors, &format!("{p}.self_attn.k_proj"), attn_bias)?,
                    v_proj: linear(&mut tensors, &format!("{p}.self_attn.v_proj"), attn_bias)?,
                    o_proj: linear(&mut tensors, &format!("{p}.self_attn.o_proj"), false)?,
                    q_norm: qk_norm
                        .then(|| take(&mut tensors, &format!("{p}.self_attn.q_norm.weight")))
                        .transpose()?,
                    k_norm: qk_norm
                        .then(|| take(&mut tensors, &format!("{p}.self_attn.k_norm.weight")))
                        .transpose()?,
                    rms_eps: config.rms_norm_eps,
                    n_heads: config.num_attention_heads,
                    n_kv_heads: config.num_key_value_heads,
                    head_dim: config.head_dim(),
                },
                mlp: Mlp {
                    gate_proj: linear(&mut tensors, &format!("{p}.mlp.gate_proj"), false)?,
                    up_proj: linear(&mut tensors, &format!("{p}.mlp.up_proj"), false)?,
                    down_proj: linear(&mut tensors, &format!("{p}.mlp.down_proj"), false)?,
                },
                input_layernorm: take(&mut tensors, &format!("{p}.input_layernorm.weight"))?,
                post_attention_layernorm: take(
                    &mut tensors,
                    &format!("{p}.post_attention_layernorm.weight"),
                )?,
            });
        }
        Ok(Self {
            config,
            embed,
            layers,
            norm,
            device: device.clone(),
            dtype,
        })
    }

    /// The compute dtype the weights were loaded in.
    #[must_use]
    pub fn dtype(&self) -> DType {
        self.dtype
    }

    /// The loaded configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Fold peft `lora_A`/`lora_B` pairs into the targeted projections, with
    /// `scale = alpha / r` already applied by the caller.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] when a targeted pair is missing or malformed.
    pub fn merge_lora(
        &mut self,
        tensors: &std::collections::HashMap<String, Tensor>,
        targets: &[String],
        scale: f64,
    ) -> Result<()> {
        for (i, layer) in self.layers.iter_mut().enumerate() {
            for target in targets {
                let (group, linear) = match target.as_str() {
                    "q_proj" => ("self_attn", &mut layer.attn.q_proj),
                    "k_proj" => ("self_attn", &mut layer.attn.k_proj),
                    "v_proj" => ("self_attn", &mut layer.attn.v_proj),
                    "o_proj" => ("self_attn", &mut layer.attn.o_proj),
                    "gate_proj" => ("mlp", &mut layer.mlp.gate_proj),
                    "up_proj" => ("mlp", &mut layer.mlp.up_proj),
                    "down_proj" => ("mlp", &mut layer.mlp.down_proj),
                    other => {
                        return Err(Error::Artifact(format!(
                            "adapter targets unknown module {other}"
                        )));
                    }
                };
                let a = tensors
                    .get(&format!("layers.{i}.{group}.{target}.lora_A.weight"))
                    .ok_or_else(|| {
                        Error::Artifact(format!(
                            "missing lora_A for layers.{i}.{group}.{target}"
                        ))
                    })?;
                let b = tensors
                    .get(&format!("layers.{i}.{group}.{target}.lora_B.weight"))
                    .ok_or_else(|| {
                        Error::Artifact(format!(
                            "missing lora_B for layers.{i}.{group}.{target}"
                        ))
                    })?;
                crate::lora::merge(linear, a, b, scale)?;
            }
        }
        Ok(())
    }

    /// The last hidden state for one packed sequence: `[len, hidden_size]`.
    /// `pos` carries the encoding's position ids and `mask` is the additive
    /// `[len, len]` block-causal mask.
    ///
    /// # Errors
    ///
    /// Propagates candle errors from the forward pass.
    pub fn hidden(&self, ids: &[u32], pos: &[i64], mask: &Tensor) -> Result<Tensor> {
        let len = ids.len();
        let ids_t = Tensor::from_vec(ids.to_vec(), len, &self.device)?;
        let mut x = self.embed.index_select(&ids_t, 0)?; // [len, d]
        let rope_len = pos.iter().copied().max().unwrap_or(0) as usize + 1;
        let rotary = Rotary::new(
            self.config.head_dim(),
            self.config.rope_theta,
            rope_len,
            &self.device,
        )?;
        for layer in &self.layers {
            let h = rms_norm(&x, &layer.input_layernorm, self.config.rms_norm_eps)?;
            x = (x + layer.attn.forward(&h, &rotary, pos, mask)?)?;
            let h = rms_norm(&x, &layer.post_attention_layernorm, self.config.rms_norm_eps)?;
            x = (x + layer.mlp.forward(&h)?)?;
        }
        Ok(rms_norm(&x, &self.norm, self.config.rms_norm_eps)?)
    }
}
