//! Flint's original BF16 checkpoint, executed in Rust on CPU or CUDA.
//!
//! Architecture reference: Recursive's Apache-2.0 Flint release and its
//! SGLang port at 37bc771d2b253e2d87d27ace71297154874ee3b4. This is a new
//! implementation using Candle, not a wrapper around an upstream server.

use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::Path,
    time::Instant,
};

use anyhow::{Context, Result, bail, ensure};
use candle::{D, DType, Device, Module, Tensor};
use candle_nn::{Embedding, Linear, VarBuilder};
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MODEL_ID: &str = "RecursiveSI/flint";
pub const REVISION: &str = "2a247be0c0aec895ccb0ac7ca92229ec6df37288";
pub const STOP_IDS: &[u32] = &[150001, 150003, 150005, 150018];

pub fn load_tokenizer(path: &Path) -> Result<tokenizers::Tokenizer> {
    let bytes = fs::read(path)?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes))
            == "a263cce0b7e2b03f7ee07ce809783ef4b47f160c3f268a4ea1524f638da45b68",
        "wrong Flint tokenizer checksum"
    );
    tokenizers::Tokenizer::from_bytes(bytes).map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Verify every checkpoint shard and tokenizer against the independent release pins.
pub fn verify_artifact(root: &Path) -> Result<()> {
    #[derive(Deserialize)]
    struct File {
        path: String,
        size: u64,
        sha256: String,
    }
    #[derive(Deserialize)]
    struct Artifact {
        files: Vec<File>,
    }
    let artifact: Artifact = serde_json::from_str(include_str!("../testdata/artifact.json"))?;
    let mut buffer = vec![0u8; 8 * 1024 * 1024];
    for file in artifact.files {
        let mut input = fs::File::open(root.join(&file.path))
            .with_context(|| format!("opening {}", file.path))?;
        ensure!(
            input.metadata()?.len() == file.size,
            "wrong size for {}",
            file.path
        );
        let mut hash = Sha256::new();
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        ensure!(
            format!("{:x}", hash.finalize()) == file.sha256,
            "checksum mismatch for {}",
            file.path
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub model_type: String,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub moe_intermediate_size: usize,
    pub shared_expert_intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub head_dim: usize,
    pub vocab_size: usize,
    pub num_experts: usize,
    pub num_experts_per_tok: usize,
    pub mlp_only_layers: Vec<usize>,
    pub no_rope_layer_indices: Vec<usize>,
    pub layer_types: Vec<String>,
    pub sliding_window: usize,
    pub max_position_embeddings: usize,
    pub rms_norm_eps: f64,
    pub rope_theta: f64,
    pub router_score_function: String,
    pub norm_topk_prob: bool,
    pub softmax_type: String,
    pub hidden_act: String,
    pub attention_bias: bool,
    pub tie_word_embeddings: bool,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.model_type == "flint", "expected a Flint checkpoint");
        ensure!(
            self.hidden_size > 0 && self.vocab_size > 0 && self.num_hidden_layers > 0,
            "empty model dimensions"
        );
        ensure!(
            self.num_key_value_heads > 0
                && self.num_attention_heads > 0
                && self.num_attention_heads % self.num_key_value_heads == 0,
            "invalid GQA heads"
        );
        ensure!(
            self.head_dim > 0 && self.head_dim % 2 == 0,
            "invalid rotary head dimension"
        );
        ensure!(
            self.num_experts_per_tok > 0 && self.num_experts_per_tok <= self.num_experts,
            "invalid expert selection"
        );
        ensure!(
            self.layer_types.len() == self.num_hidden_layers && self.sliding_window > 0,
            "invalid attention layout"
        );
        ensure!(
            self.layer_types
                .iter()
                .all(|t| t == "full_attention" || t == "sliding_attention"),
            "unsupported attention type"
        );
        ensure!(
            self.mlp_only_layers
                .iter()
                .chain(&self.no_rope_layer_indices)
                .all(|i| *i < self.num_hidden_layers),
            "layer index outside model"
        );
        ensure!(
            self.router_score_function == "sigmoid"
                && self.norm_topk_prob
                && self.softmax_type == "learnable"
                && self.hidden_act == "silu"
                && !self.attention_bias
                && !self.tie_word_embeddings,
            "unsupported Flint configuration"
        );
        ensure!(
            self.rms_norm_eps.is_finite()
                && self.rms_norm_eps > 0.0
                && self.rope_theta.is_finite()
                && self.rope_theta > 0.0,
            "invalid norm or rotary parameters"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

fn escaped_text(text: &str) -> String {
    let mut s = text
        .replace('\0', "")
        .replace("<think>", "")
        .replace("</think>", "");
    for role in [
        "begin_of_text",
        "end_of_text",
        "system",
        "developer",
        "user",
        "assistant",
        "observation",
    ] {
        s = s.replace(&format!("<|{role}|>"), &format!("&lt;|{role}|&gt;"));
    }
    s
}

/// Render the released template's text-only subset, preserving assistant reasoning.
pub fn render(messages: &[Message]) -> Result<String> {
    ensure!(!messages.is_empty(), "messages must not be empty");
    let mut text = String::new();
    for m in messages {
        match m.role.as_str() {
            "system" | "developer" | "user" => {
                text.push_str(&format!("<|{}|>{}", m.role, escaped_text(&m.content)));
            }
            "assistant" => {
                text.push_str("<|assistant|>");
                if let Some(reasoning) = &m.reasoning_content {
                    if !reasoning.is_empty() {
                        text.push_str(&format!("<think>{reasoning}</think>"));
                    }
                }
                text.push_str(&m.content);
            }
            // Tool execution is outside the initial text endpoint.
            _ => bail!("unsupported message role: {}", m.role),
        }
    }
    text.push_str("<|assistant|>");
    Ok(text)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Answer {
    pub content: String,
    pub reasoning_content: Option<String>,
}

pub fn split_answer(text: &str) -> Result<Answer> {
    ensure!(
        !text.contains("<tool_call>"),
        "the model requested a tool; this endpoint accepts text requests only"
    );
    if let Some(rest) = text.trim_start().strip_prefix("<think>") {
        return Ok(match rest.split_once("</think>") {
            Some((reasoning, content)) => Answer {
                content: content.to_string(),
                reasoning_content: Some(reasoning.to_string()),
            },
            None => Answer {
                content: String::new(),
                reasoning_content: Some(rest.to_string()),
            },
        });
    }
    Ok(Answer {
        content: text.to_string(),
        reasoning_content: None,
    })
}

struct Mlp {
    gate: Linear,
    up: Linear,
    down: Linear,
}

impl Mlp {
    fn load(vb: VarBuilder<'_>, h: usize, f: usize) -> candle::Result<Self> {
        Ok(Self {
            gate: candle_nn::linear_no_bias(h, f, vb.pp("gate_proj"))?,
            up: candle_nn::linear_no_bias(h, f, vb.pp("up_proj"))?,
            down: candle_nn::linear_no_bias(f, h, vb.pp("down_proj"))?,
        })
    }
    fn forward(&self, x: &Tensor) -> candle::Result<Tensor> {
        project(
            &self.down,
            &(candle_nn::ops::silu(&project(&self.gate, x)?)? * project(&self.up, x)?)?,
        )
    }
}

// Keep the checkpoint resident in BF16 and expand only the projection in use.
// FP32 activations avoid repeated BF16 rounding across the 49-layer MoE.
fn project(linear: &Linear, x: &Tensor) -> candle::Result<Tensor> {
    Linear::new(linear.weight().to_dtype(x.dtype())?, None).forward(x)
}

fn norm(x: &Tensor, weight: &Tensor, eps: f32) -> candle::Result<Tensor> {
    candle_nn::ops::rms_norm(x, &weight.to_dtype(x.dtype())?, eps)
}

struct Layer {
    input_norm: Tensor,
    post_norm: Tensor,
    q: Linear,
    k: Linear,
    v: Linear,
    o: Linear,
    q_norm: Tensor,
    k_norm: Tensor,
    sink: Tensor,
    dense: Option<Mlp>,
    gate: Option<Linear>,
    experts: Vec<Mlp>,
    shared: Option<Mlp>,
}

impl Layer {
    fn load(vb: VarBuilder<'_>, c: &Config, id: usize) -> Result<Self> {
        let h = c.hidden_size;
        let a = vb.pp("self_attn");
        let m = vb.pp("mlp");
        let dense = c.mlp_only_layers.contains(&id);
        Ok(Self {
            input_norm: vb.get(h, "input_layernorm.weight")?,
            post_norm: vb.get(h, "post_attention_layernorm.weight")?,
            q: candle_nn::linear_no_bias(h, c.num_attention_heads * c.head_dim, a.pp("q_proj"))?,
            k: candle_nn::linear_no_bias(h, c.num_key_value_heads * c.head_dim, a.pp("k_proj"))?,
            v: candle_nn::linear_no_bias(h, c.num_key_value_heads * c.head_dim, a.pp("v_proj"))?,
            o: candle_nn::linear_no_bias(c.num_attention_heads * c.head_dim, h, a.pp("o_proj"))?,
            q_norm: a.get(c.head_dim, "q_norm.weight")?,
            k_norm: a.get(c.head_dim, "k_norm.weight")?,
            sink: a
                .get(c.num_attention_heads, "softmax_offset")?
                .to_dtype(DType::F32)?,
            dense: if dense {
                Some(Mlp::load(m.clone(), h, c.intermediate_size)?)
            } else {
                None
            },
            gate: if dense {
                None
            } else {
                Some(candle_nn::linear_no_bias(h, c.num_experts, m.pp("gate"))?)
            },
            experts: if dense {
                Vec::new()
            } else {
                (0..c.num_experts)
                    .map(|e| Mlp::load(m.pp(format!("experts.{e}")), h, c.moe_intermediate_size))
                    .collect::<candle::Result<_>>()?
            },
            shared: if dense {
                None
            } else {
                Some(Mlp::load(
                    m.pp("shared_expert"),
                    h,
                    c.shared_expert_intermediate_size,
                )?)
            },
        })
    }
}

/// Request-owned KV state. A failed step must be discarded with the request.
pub struct Session {
    cache: Vec<Option<(Tensor, Tensor)>>,
    position: usize,
    failed: bool,
    capacity: usize,
}

pub struct Model {
    pub config: Config,
    pub device: Device,
    embed: Embedding,
    layers: Vec<Layer>,
    norm: Tensor,
    head: Linear,
    pub config_sha256: String,
}

impl Model {
    pub fn load(root: &Path, device: Device) -> Result<Self> {
        verify_artifact(root)?;
        let bytes = fs::read(root.join("config.json"))?;
        let c: Config = serde_json::from_slice(&bytes)?;
        c.validate()?;
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("model.safetensors.index.json"))?)?;
        let map = index["weight_map"]
            .as_object()
            .context("missing safetensors weight map")?;
        let names: BTreeSet<_> = map
            .values()
            .map(|v| v.as_str().context("invalid shard name"))
            .collect::<Result<_>>()?;
        let mut paths = Vec::new();
        for name in names {
            ensure!(
                Path::new(name).file_name().and_then(|p| p.to_str()) == Some(name),
                "shard path must be a file name"
            );
            paths.push(root.join(name));
        }
        // Files are read-only and must stay unchanged while this model is loaded.
        let dtype = if device.is_cpu() {
            DType::F32
        } else {
            DType::BF16
        };
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&paths, dtype, &device)? };
        let mut model = Self::from_builder(c, vb, device.clone())?;
        model.config_sha256 = format!("{:x}", Sha256::digest(bytes));
        Ok(model)
    }

    pub fn from_builder(c: Config, vb: VarBuilder<'_>, device: Device) -> Result<Self> {
        c.validate()?;
        let embed = candle_nn::embedding(c.vocab_size, c.hidden_size, vb.pp("model.embed_tokens"))?;
        let mut layers = Vec::new();
        for id in 0..c.num_hidden_layers {
            layers.push(
                Layer::load(vb.pp(format!("model.layers.{id}")), &c, id)
                    .with_context(|| format!("loading Flint layer {id}"))?,
            );
        }
        // The released serving kit computes the vocabulary projection in F32.
        let head = Linear::new(
            vb.get((c.vocab_size, c.hidden_size), "lm_head.weight")?
                .to_dtype(DType::F32)?,
            None,
        );
        Ok(Self {
            norm: vb.get(c.hidden_size, "model.norm.weight")?,
            config: c,
            device,
            embed,
            layers,
            head,
            config_sha256: String::new(),
        })
    }

    pub fn session(&self, capacity: usize) -> Result<Session> {
        ensure!(
            capacity > 0 && capacity <= self.config.max_position_embeddings,
            "context exceeds model capacity"
        );
        Ok(Session {
            cache: (0..self.layers.len()).map(|_| None).collect(),
            position: 0,
            failed: false,
            capacity,
        })
    }

    pub fn forward(&self, token: u32, session: &mut Session) -> Result<Vec<f32>> {
        ensure!(
            !session.failed
                && session.position < session.capacity
                && (token as usize) < self.config.vocab_size,
            "invalid token or request state"
        );
        session.failed = true;
        let result = self.step(token, session)?;
        session.position += 1;
        session.failed = false;
        Ok(result)
    }

    fn step(&self, token: u32, state: &mut Session) -> Result<Vec<f32>> {
        let c = &self.config;
        let trace = std::env::var_os("PSIONIC_FLINT_TRACE");
        let mut x = self
            .embed
            .forward(&Tensor::new(&[token], &self.device)?)?
            .to_dtype(DType::F32)?;
        for (id, l) in self.layers.iter().enumerate() {
            trace_tensor(&trace, state.position, id, "input", &x)?;
            let n = norm(&x, &l.input_norm, c.rms_norm_eps as f32)?;
            trace_tensor(&trace, state.position, id, "input_norm", &n)?;
            let q = project(&l.q, &n)?.reshape((c.num_attention_heads, c.head_dim))?;
            let k = project(&l.k, &n)?.reshape((c.num_key_value_heads, c.head_dim))?;
            let mut q = norm(&q.contiguous()?, &l.q_norm, c.rms_norm_eps as f32)?;
            let mut k = norm(&k.contiguous()?, &l.k_norm, c.rms_norm_eps as f32)?;
            if !c.no_rope_layer_indices.contains(&id) {
                q = rope(&q, state.position, c.rope_theta)?;
                k = rope(&k, state.position, c.rope_theta)?;
            }
            let v = project(&l.v, &n)?.reshape((c.num_key_value_heads, 1, c.head_dim))?;
            let k = k.unsqueeze(1)?;
            let (mut keys, mut values) = match &state.cache[id] {
                Some((pk, pv)) => (Tensor::cat(&[pk, &k], 1)?, Tensor::cat(&[pv, &v], 1)?),
                None => (k, v),
            };
            let len = keys.dim(1)?;
            if c.layer_types[id] == "sliding_attention" && len > c.sliding_window {
                keys = keys
                    .narrow(1, len - c.sliding_window, c.sliding_window)?
                    .contiguous()?;
                values = values
                    .narrow(1, len - c.sliding_window, c.sliding_window)?
                    .contiguous()?;
            }
            let attention = attend(
                &q,
                &keys,
                &values,
                &l.sink,
                c.num_attention_heads / c.num_key_value_heads,
            )?;
            trace_tensor(&trace, state.position, id, "attention", &attention)?;
            state.cache[id] = Some((keys, values));
            x = (&x
                + project(
                    &l.o,
                    &attention.reshape((1, c.num_attention_heads * c.head_dim))?,
                )?)?;
            let n = norm(&x, &l.post_norm, c.rms_norm_eps as f32)?;
            trace_tensor(&trace, state.position, id, "post_norm", &n)?;
            let mlp = match &l.dense {
                Some(dense) => dense.forward(&n)?,
                None => {
                    let gate = l.gate.as_ref().context("missing expert router")?;
                    let scores = project(gate, &n)?
                        .to_dtype(DType::F32)?
                        .flatten_all()?
                        .to_vec1::<f32>()?;
                    let selected = route(&scores, c.num_experts_per_tok)?;
                    let mut sum = Tensor::zeros((1, c.hidden_size), DType::F32, &self.device)?;
                    for (expert, weight) in selected {
                        sum = (sum
                            + (l.experts[expert].forward(&n)?.to_dtype(DType::F32)?
                                * weight as f64)?)?;
                    }
                    let shared = l
                        .shared
                        .as_ref()
                        .context("missing shared expert")?
                        .forward(&n)?;
                    (sum.to_dtype(x.dtype())? + shared)?
                }
            };
            trace_tensor(&trace, state.position, id, "mlp", &mlp)?;
            x = (&x + mlp)?;
        }
        let n = norm(&x, &self.norm, c.rms_norm_eps as f32)?;
        let logits = self
            .head
            .forward(&n.to_dtype(DType::F32)?)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        ensure!(
            logits.iter().all(|v| v.is_finite()),
            "model produced non-finite logits"
        );
        Ok(logits)
    }
}

fn trace_tensor(
    path: &Option<std::ffi::OsString>,
    position: usize,
    layer: usize,
    name: &str,
    tensor: &Tensor,
) -> Result<()> {
    if let Some(path) = path {
        let values = tensor
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(
            file,
            "{}",
            serde_json::json!({"position":position,"layer":layer,"name":name,"values":values})
        )?;
    }
    Ok(())
}

fn rope(x: &Tensor, position: usize, theta: f64) -> candle::Result<Tensor> {
    let half = x.dim(D::Minus1)? / 2;
    let angles: Vec<f32> = (0..half)
        .map(|i| (position as f64 / theta.powf(i as f64 / half as f64)) as f32)
        .collect();
    let angles = Tensor::new(angles, x.device())?;
    let cos = angles.cos()?.to_dtype(x.dtype())?;
    let sin = angles.sin()?.to_dtype(x.dtype())?;
    let a = x.narrow(D::Minus1, 0, half)?;
    let b = x.narrow(D::Minus1, half, half)?;
    Tensor::cat(
        &[
            &(a.broadcast_mul(&cos)? - b.broadcast_mul(&sin)?)?,
            &(a.broadcast_mul(&sin)? + b.broadcast_mul(&cos)?)?,
        ],
        D::Minus1,
    )
}

fn attend(
    q: &Tensor,
    k: &Tensor,
    v: &Tensor,
    sink: &Tensor,
    groups: usize,
) -> candle::Result<Tensor> {
    let (kv, len, dim) = k.dims3()?;
    let heads = kv * groups;
    let keys = k
        .unsqueeze(1)?
        .broadcast_as((kv, groups, len, dim))?
        .reshape((heads, len, dim))?
        .contiguous()?
        .to_dtype(DType::F32)?;
    let values = v
        .unsqueeze(1)?
        .broadcast_as((kv, groups, len, dim))?
        .reshape((heads, len, dim))?
        .contiguous()?
        .to_dtype(DType::F32)?;
    let scores = (q.to_dtype(DType::F32)?.unsqueeze(1)?.matmul(&keys.t()?)? / (dim as f64).sqrt())?;
    // The learned sink contributes to the denominator and has a zero value.
    let scores = Tensor::cat(&[&scores, &sink.reshape((heads, 1, 1))?], 2)?;
    let probs = candle_nn::ops::softmax_last_dim(&scores)?
        .narrow(2, 0, len)?
        .contiguous()?;
    probs.matmul(&values)?.squeeze(1)?.to_dtype(q.dtype())
}

pub fn route(logits: &[f32], count: usize) -> Result<Vec<(usize, f32)>> {
    ensure!(
        count > 0 && count <= logits.len() && logits.iter().all(|v| v.is_finite()),
        "invalid router scores"
    );
    let mut scores: Vec<_> = logits
        .iter()
        .enumerate()
        .map(|(i, x)| (i, 1.0 / (1.0 + (-x).exp())))
        .collect();
    scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    scores.truncate(count);
    let sum: f32 = scores.iter().map(|(_, s)| s).sum();
    ensure!(sum > 0.0 && sum.is_finite(), "invalid router normalization");
    for (_, score) in &mut scores {
        *score /= sum;
    }
    Ok(scores)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Sampling {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: usize,
    pub seed: u64,
}
impl Default for Sampling {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            top_p: 0.95,
            top_k: 20,
            seed: 42,
        }
    }
}
impl Sampling {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.temperature.is_finite()
                && self.temperature >= 0.0
                && self.top_p.is_finite()
                && self.top_p > 0.0
                && self.top_p <= 1.0
                && self.top_k > 0,
            "invalid sampling options"
        );
        Ok(())
    }
    /// Pick a token from vocabulary logits: greedy at temperature zero, else top-k then top-p.
    pub fn choose(&self, logits: &[f32], rng: &mut StdRng) -> Result<u32> {
        ensure!(
            !logits.is_empty() && logits.iter().all(|x| x.is_finite()),
            "invalid vocabulary logits"
        );
        let mut order: Vec<_> = logits.iter().copied().enumerate().collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        self.choose_ranked(&order, rng)
    }

    /// The same choice from the leading candidates of the ranked vocabulary
    /// (highest logit first, lower ID first on ties). `ranked` must hold at
    /// least `top_k` entries, or the whole vocabulary.
    pub fn choose_ranked(&self, ranked: &[(usize, f32)], rng: &mut StdRng) -> Result<u32> {
        ensure!(!ranked.is_empty(), "invalid vocabulary logits");
        let mut order = ranked.to_vec();
        if self.temperature == 0.0 {
            return Ok(order[0].0 as u32);
        }
        order.truncate(self.top_k.min(order.len()));
        let max = order[0].1 as f64;
        let mut weights: Vec<f64> = order
            .iter()
            .map(|(_, x)| ((*x as f64 - max) / self.temperature).exp())
            .collect();
        let total: f64 = weights.iter().sum();
        let mut cumulative = 0.0;
        let mut keep = 0;
        for w in &weights {
            cumulative += w / total;
            keep += 1;
            if cumulative >= self.top_p {
                break;
            }
        }
        weights.truncate(keep);
        let mut draw = rng.random::<f64>() * weights.iter().sum::<f64>();
        for (i, w) in weights.iter().enumerate() {
            draw -= w;
            if draw <= 0.0 {
                return Ok(order[i].0 as u32);
            }
        }
        Ok(order[keep - 1].0 as u32)
    }
}

#[derive(Debug, Serialize)]
pub struct Generation {
    pub answer: Answer,
    pub token_ids: Vec<u32>,
    pub prompt_tokens: usize,
    pub finish_reason: String,
    pub elapsed_seconds: f64,
    pub prefill_seconds: f64,
}

pub fn generate(
    model: &Model,
    tokenizer: &tokenizers::Tokenizer,
    prompt: &[u32],
    max_tokens: usize,
    capacity: usize,
    sampling: &Sampling,
    deadline_seconds: f64,
) -> Result<Generation> {
    sampling.validate()?;
    ensure!(
        !prompt.is_empty()
            && max_tokens > 0
            && prompt
                .len()
                .checked_add(max_tokens)
                .is_some_and(|n| n <= capacity),
        "prompt and output exceed context limit"
    );
    ensure!(
        deadline_seconds.is_finite() && deadline_seconds > 0.0,
        "invalid request deadline"
    );
    let start = Instant::now();
    let mut state = model.session(capacity)?;
    let mut logits = Vec::new();
    for &token in prompt {
        ensure!(
            start.elapsed().as_secs_f64() < deadline_seconds,
            "request timed out"
        );
        logits = model.forward(token, &mut state)?;
    }
    let prefill_seconds = start.elapsed().as_secs_f64();
    let mut rng = StdRng::seed_from_u64(sampling.seed);
    let mut ids = Vec::new();
    let mut finish = "length";
    for step in 0..max_tokens {
        ensure!(
            start.elapsed().as_secs_f64() < deadline_seconds,
            "request timed out"
        );
        let token = sampling.choose(&logits, &mut rng)?;
        if STOP_IDS.contains(&token) {
            finish = "stop";
            break;
        }
        ids.push(token);
        if step + 1 < max_tokens {
            logits = model.forward(token, &mut state)?;
        }
    }
    let text = tokenizer
        .decode(&ids, false)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(Generation {
        answer: split_answer(&text)?,
        token_ids: ids,
        prompt_tokens: prompt.len(),
        finish_reason: finish.to_string(),
        elapsed_seconds: start.elapsed().as_secs_f64(),
        prefill_seconds,
    })
}

#[cfg(all(feature = "metal", target_os = "macos"))]
pub mod metal;

#[cfg(test)]
mod tests;
