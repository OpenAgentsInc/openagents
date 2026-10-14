//! SmolVLA inference reimplemented from LeRobot v0.3.3 (Apache-2.0).
//! The SigLIP vision tower, pixel shuffle, SmolLM prefix, grouped-query
//! action expert, cached cross attention, and Euler flow steps use pinned
//! local weights. Noise is supplied explicitly for reproducible comparisons.
use crate::{Result, require};
use candle_core::{DType, Tensor};
use candle_nn::{Conv2d, Embedding, LayerNorm, Linear, RmsNorm, VarBuilder};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisionConfig {
    pub hidden: usize,
    pub intermediate: usize,
    pub layers: usize,
    pub heads: usize,
    pub image_size: usize,
    pub patch_size: usize,
    pub shuffle: usize,
    pub norm_epsilon: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub vision: VisionConfig,
    pub vocabulary: usize,
    pub hidden: usize,
    pub intermediate: usize,
    pub layers: usize,
    pub expert_hidden: usize,
    pub expert_intermediate: usize,
    pub expert_layers: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
    pub norm_epsilon: f64,
    pub attention_bias: bool,
    pub cross_attention: bool,
    pub self_attention_every: usize,
    pub max_state_dim: usize,
    pub max_action_dim: usize,
    pub chunk_size: usize,
    pub denoise_steps: usize,
    pub min_period: f64,
    pub max_period: f64,
    pub prefix_length: usize,
    pub image_start_tokens: Vec<u32>,
    pub image_end_tokens: Vec<u32>,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        let v = &self.vision;
        require(
            (8..=1152).contains(&v.hidden)
                && (1..=32).contains(&v.heads)
                && v.hidden.is_multiple_of(v.heads)
                && (8..=8192).contains(&v.intermediate)
                && (1..=32).contains(&v.layers)
                && (16..=512).contains(&v.image_size)
                && (2..=32).contains(&v.patch_size)
                && v.image_size.is_multiple_of(v.patch_size)
                && (1..=4).contains(&v.shuffle)
                && (v.image_size / v.patch_size).is_multiple_of(v.shuffle)
                && v.norm_epsilon.is_finite()
                && (1e-8..=1e-3).contains(&v.norm_epsilon),
            "Choose bounded SmolVLA vision settings",
        )?;
        require(
            (8..=2048).contains(&self.hidden)
                && (8..=2048).contains(&self.expert_hidden)
                && self.expert_hidden.is_multiple_of(2)
                && (8..=8192).contains(&self.intermediate)
                && (8..=8192).contains(&self.expert_intermediate)
                && (1..=32).contains(&self.layers)
                && (1..=self.layers).contains(&self.expert_layers)
                && self.layers.is_multiple_of(self.expert_layers),
            "Choose bounded SmolVLA transformer settings",
        )?;
        require(
            (1..=32).contains(&self.heads)
                && (1..=self.heads).contains(&self.kv_heads)
                && self.heads.is_multiple_of(self.kv_heads)
                && (2..=128).contains(&self.head_dim)
                && self.head_dim.is_multiple_of(2)
                && self.norm_epsilon.is_finite()
                && (1e-8..=1e-3).contains(&self.norm_epsilon)
                && (4..=65536).contains(&self.vocabulary),
            "Choose supported SmolVLA grouped attention",
        )?;
        require(
            (1..=64).contains(&self.max_state_dim)
                && (1..=64).contains(&self.max_action_dim)
                && (1..=128).contains(&self.chunk_size)
                && (1..=50).contains(&self.denoise_steps)
                && self.min_period.is_finite()
                && self.max_period.is_finite()
                && self.min_period > 0.
                && self.max_period >= self.min_period
                && self.max_period <= 100.
                && self.prefix_length <= 1024
                && self.image_start_tokens.len() <= 4
                && self.image_end_tokens.len() <= 4
                && self
                    .image_start_tokens
                    .iter()
                    .chain(&self.image_end_tokens)
                    .all(|t| (*t as usize) < self.vocabulary),
            "Choose bounded SmolVLA inputs and flow steps",
        )?;
        let parameters = self.vocabulary * self.hidden
            + self.layers
                * (4 * self.hidden * self.heads * self.head_dim
                    + 3 * self.hidden * self.intermediate)
            + self.expert_layers
                * (4 * self.expert_hidden * self.heads * self.head_dim
                    + 3 * self.expert_hidden * self.expert_intermediate)
            + v.layers * (4 * v.hidden * v.hidden + 2 * v.hidden * v.intermediate);
        require(
            parameters <= 850_000_000,
            "Choose a SmolVLA model within the supported memory size",
        )?;
        require(
            self.expert_layers == self.layers
                || (self.cross_attention && self.self_attention_every == 0),
            "Use cross attention without interleaved self attention for a reduced expert",
        )
    }
}
struct VisionLayer {
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
    first: Linear,
    second: Linear,
    norm1: LayerNorm,
    norm2: LayerNorm,
    heads: usize,
}
impl VisionLayer {
    fn load(c: &VisionConfig, vb: VarBuilder) -> candle_core::Result<Self> {
        let linear = |name| candle_nn::linear(c.hidden, c.hidden, vb.pp("self_attn").pp(name));
        Ok(Self {
            q: linear("q_proj")?,
            k: linear("k_proj")?,
            v: linear("v_proj")?,
            out: linear("out_proj")?,
            first: candle_nn::linear(c.hidden, c.intermediate, vb.pp("mlp.fc1"))?,
            second: candle_nn::linear(c.intermediate, c.hidden, vb.pp("mlp.fc2"))?,
            norm1: candle_nn::layer_norm(c.hidden, c.norm_epsilon, vb.pp("layer_norm1"))?,
            norm2: candle_nn::layer_norm(c.hidden, c.norm_epsilon, vb.pp("layer_norm2"))?,
            heads: c.heads,
        })
    }
    fn forward(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        let h = x.apply(&self.norm1)?;
        let (b, s, d) = h.dims3()?;
        let shape = |t: Tensor| t.reshape((b, s, self.heads, d / self.heads));
        let q = shape(h.apply(&self.q)?)?;
        let k = shape(h.apply(&self.k)?)?;
        let v = shape(h.apply(&self.v)?)?;
        let mask = Tensor::ones((1, s, s), DType::U8, x.device())?;
        let h = (x + attention(&q, &k, &v, &mask, self.heads)?.apply(&self.out)?)?;
        let feed = h
            .apply(&self.norm2)?
            .apply(&self.first)?
            .gelu()?
            .apply(&self.second)?;
        &h + feed
    }
}
struct Vision {
    config: VisionConfig,
    patch: Conv2d,
    position: Embedding,
    layers: Vec<VisionLayer>,
    norm: LayerNorm,
    connector: Linear,
}
impl Vision {
    fn load(c: &VisionConfig, text_dim: usize, vb: VarBuilder) -> candle_core::Result<Self> {
        let tower = vb.pp("vlm.model.vision_model");
        let patches = (c.image_size / c.patch_size).pow(2);
        Ok(Self {
            config: c.clone(),
            patch: candle_nn::conv2d(
                3,
                c.hidden,
                c.patch_size,
                candle_nn::Conv2dConfig {
                    stride: c.patch_size,
                    ..Default::default()
                },
                tower.pp("embeddings.patch_embedding"),
            )?,
            position: candle_nn::embedding(
                patches,
                c.hidden,
                tower.pp("embeddings.position_embedding"),
            )?,
            layers: (0..c.layers)
                .map(|i| VisionLayer::load(c, tower.pp(format!("encoder.layers.{i}"))))
                .collect::<candle_core::Result<_>>()?,
            norm: candle_nn::layer_norm(c.hidden, c.norm_epsilon, tower.pp("post_layernorm"))?,
            connector: candle_nn::linear_no_bias(
                c.hidden * c.shuffle * c.shuffle,
                text_dim,
                vb.pp("vlm.model.connector.modality_projection.proj"),
            )?,
        })
    }
    fn forward(&self, image: &Tensor) -> candle_core::Result<Tensor> {
        let x = image.apply(&self.patch)?;
        let (b, d, h, w) = x.dims4()?;
        let mut x = x.flatten_from(2)?.transpose(1, 2)?;
        let positions = Tensor::arange(0, (h * w) as u32, image.device())?;
        x = x.broadcast_add(&positions.apply(&self.position)?)?;
        for layer in &self.layers {
            x = layer.forward(&x)?;
        }
        let x = x.apply(&self.norm)?;
        let r = self.config.shuffle;
        // Transformers pixel_shuffle first folds width into channels, then
        // folds height, preserving its exact spatial/channel ordering.
        x.reshape((b, h, w, d))?
            .reshape((b, h, w / r, d * r))?
            .transpose(1, 2)?
            .contiguous()?
            .reshape((b, w / r, h / r, d * r * r))?
            .transpose(1, 2)?
            .contiguous()?
            .reshape((b, (h / r) * (w / r), d * r * r))?
            .apply(&self.connector)
    }
}
struct Layer {
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
    gate: Linear,
    up: Linear,
    down: Linear,
    norm1: RmsNorm,
    norm2: RmsNorm,
}
impl Layer {
    fn load(
        c: &Config,
        hidden: usize,
        intermediate: usize,
        cross: bool,
        vb: VarBuilder,
    ) -> candle_core::Result<Self> {
        let projection = |name: &str, input, output| {
            candle_nn::linear_b(input, output, c.attention_bias, vb.pp("self_attn").pp(name))
        };
        let kv = c.kv_heads * c.head_dim;
        Ok(Self {
            q: projection("q_proj", hidden, c.heads * c.head_dim)?,
            k: projection("k_proj", if cross { kv } else { hidden }, kv)?,
            v: projection("v_proj", if cross { kv } else { hidden }, kv)?,
            out: projection("o_proj", c.heads * c.head_dim, hidden)?,
            gate: candle_nn::linear_no_bias(hidden, intermediate, vb.pp("mlp.gate_proj"))?,
            up: candle_nn::linear_no_bias(hidden, intermediate, vb.pp("mlp.up_proj"))?,
            down: candle_nn::linear_no_bias(intermediate, hidden, vb.pp("mlp.down_proj"))?,
            norm1: candle_nn::rms_norm(hidden, c.norm_epsilon, vb.pp("input_layernorm"))?,
            norm2: candle_nn::rms_norm(hidden, c.norm_epsilon, vb.pp("post_attention_layernorm"))?,
        })
    }
    fn finish(&self, x: &Tensor, att: &Tensor) -> candle_core::Result<Tensor> {
        let x = (x + att.apply(&self.out)?)?;
        let h = x.apply(&self.norm2)?;
        let feed = (h.apply(&self.gate)?.silu()? * h.apply(&self.up)?)?.apply(&self.down)?;
        &x + feed
    }
}
fn rope(x: &Tensor, positions: &[f32]) -> candle_core::Result<Tensor> {
    let (b, s, h, d) = x.dims4()?;
    let mut sin = Vec::with_capacity(s * d / 2);
    let mut cos = Vec::with_capacity(s * d / 2);
    for p in positions {
        for i in 0..d / 2 {
            let angle = *p / 10000_f32.powf(2. * i as f32 / d as f32);
            sin.push(angle.sin());
            cos.push(angle.cos());
        }
    }
    let sin = Tensor::from_vec(sin, (1, s, 1, d / 2), x.device())?;
    let cos = Tensor::from_vec(cos, (1, s, 1, d / 2), x.device())?;
    let a = x.narrow(3, 0, d / 2)?;
    let z = x.narrow(3, d / 2, d / 2)?;
    Tensor::cat(
        &[
            (a.broadcast_mul(&cos)? - z.broadcast_mul(&sin)?)?,
            (z.broadcast_mul(&cos)? + a.broadcast_mul(&sin)?)?,
        ],
        3,
    )?
    .reshape((b, s, h, d))
}
fn attention(
    q: &Tensor,
    k: &Tensor,
    v: &Tensor,
    mask: &Tensor,
    heads: usize,
) -> candle_core::Result<Tensor> {
    let (b, sq, _, d) = q.dims4()?;
    let (_, sk, kv, _) = k.dims4()?;
    let repeated = |x: &Tensor| {
        x.unsqueeze(3)?
            .broadcast_as((b, sk, kv, heads / kv, d))?
            .contiguous()?
            .reshape((b, sk, heads, d))?
            .transpose(1, 2)?
            .contiguous()
    };
    let q = q.transpose(1, 2)?.contiguous()?;
    let k = repeated(k)?;
    let v = repeated(v)?;
    let scores = q
        .matmul(&k.transpose(2, 3)?)?
        .affine(1. / (d as f64).sqrt(), 0.)?;
    let mask = mask.unsqueeze(1)?.broadcast_as(scores.shape())?;
    let negative = Tensor::full(f32::MIN, scores.shape(), scores.device())?;
    let scores = mask.where_cond(&scores, &negative)?;
    candle_nn::ops::softmax_last_dim(&scores)?
        .matmul(&v)?
        .transpose(1, 2)?
        .contiguous()?
        .reshape((b, sq, heads * d))
}

pub struct SmolVla {
    config: Config,
    vision: Vision,
    embedding: Embedding,
    prefix: Vec<Layer>,
    expert: Vec<Layer>,
    expert_norm: RmsNorm,
    state: Linear,
    action_in: Linear,
    action_out: Linear,
    time_in: Linear,
    time_out: Linear,
}
impl SmolVla {
    pub fn load(config: Config, vb: VarBuilder) -> Result<Self> {
        config.validate()?;
        let c = &config;
        let expert = vb.pp("vlm_with_expert");
        let multiple = c.layers / c.expert_layers;
        let model = Self {
            vision: Vision::load(&c.vision, c.hidden, expert.clone())?,
            embedding: candle_nn::embedding(
                c.vocabulary,
                c.hidden,
                expert.pp("vlm.model.text_model.embed_tokens"),
            )?,
            prefix: (0..c.layers)
                .map(|i| {
                    Layer::load(
                        c,
                        c.hidden,
                        c.intermediate,
                        false,
                        expert.pp(format!("vlm.model.text_model.layers.{i}")),
                    )
                })
                .collect::<candle_core::Result<_>>()?,
            expert: (0..c.expert_layers)
                .map(|i| {
                    Layer::load(
                        c,
                        c.expert_hidden,
                        c.expert_intermediate,
                        c.cross_attention
                            && !(c.self_attention_every > 0
                                && (i * multiple).is_multiple_of(c.self_attention_every)),
                        expert.pp(format!("lm_expert.layers.{i}")),
                    )
                })
                .collect::<candle_core::Result<_>>()?,
            expert_norm: candle_nn::rms_norm(
                c.expert_hidden,
                c.norm_epsilon,
                expert.pp("lm_expert.norm"),
            )?,
            state: candle_nn::linear(c.max_state_dim, c.hidden, vb.pp("state_proj"))?,
            action_in: candle_nn::linear(
                c.max_action_dim,
                c.expert_hidden,
                vb.pp("action_in_proj"),
            )?,
            action_out: candle_nn::linear(
                c.expert_hidden,
                c.max_action_dim,
                vb.pp("action_out_proj"),
            )?,
            time_in: candle_nn::linear(
                c.expert_hidden * 2,
                c.expert_hidden,
                vb.pp("action_time_mlp_in"),
            )?,
            time_out: candle_nn::linear(
                c.expert_hidden,
                c.expert_hidden,
                vb.pp("action_time_mlp_out"),
            )?,
            config,
        };
        Ok(model)
    }
    pub fn forward(
        &self,
        state: &Tensor,
        images: &[Tensor],
        image_valid: &[bool],
        tokens: &[u32],
        language_valid: &[bool],
        noise: &Tensor,
    ) -> Result<Tensor> {
        let c = &self.config;
        require(
            state.dims() == [1, c.max_state_dim]
                && noise.dims() == [1, c.chunk_size, c.max_action_dim]
                && images.len() <= 4
                && images.len() == image_valid.len()
                && !tokens.is_empty()
                && tokens.len() <= 128
                && tokens.len() == language_valid.len()
                && language_valid.iter().any(|v| *v)
                && tokens.iter().all(|t| (*t as usize) < c.vocabulary),
            "Use the pinned SmolVLA state, language, images, and noise shapes",
        )?;
        let device = state.device();
        let mut embeddings = Vec::new();
        let mut valid = Vec::new();
        let mut groups = Vec::new();
        let language = |ids: &[u32]| -> candle_core::Result<Tensor> {
            Tensor::from_vec(ids.to_vec(), (1, ids.len()), device)?.apply(&self.embedding)
        };
        for (image, is_valid) in images.iter().zip(image_valid) {
            require(
                image.dims() == [1, 3, c.vision.image_size, c.vision.image_size],
                "Use the checkpoint's square policy images",
            )?;
            if !c.image_start_tokens.is_empty() {
                embeddings.push(language(&c.image_start_tokens)?);
                valid.extend(std::iter::repeat_n(true, c.image_start_tokens.len()));
                groups.extend(std::iter::repeat_n(0, c.image_start_tokens.len()));
            }
            let image = self
                .vision
                .forward(image)?
                .affine((c.hidden as f64).sqrt(), 0.)?;
            let n = image.dim(1)?;
            embeddings.push(image);
            valid.extend(std::iter::repeat_n(*is_valid, n));
            groups.extend(std::iter::repeat_n(0, n));
            if !c.image_end_tokens.is_empty() {
                embeddings.push(language(&c.image_end_tokens)?);
                valid.extend(std::iter::repeat_n(true, c.image_end_tokens.len()));
                groups.extend(std::iter::repeat_n(0, c.image_end_tokens.len()));
            }
        }
        embeddings.push(language(tokens)?.affine((c.hidden as f64).sqrt(), 0.)?);
        valid.extend_from_slice(language_valid);
        groups.extend(std::iter::repeat_n(0, tokens.len()));
        embeddings.push(state.apply(&self.state)?.unsqueeze(1)?);
        valid.push(true);
        groups.push(1);
        let mut prefix = Tensor::cat(&embeddings, 1)?;
        if valid.len() < c.prefix_length {
            let n = c.prefix_length - valid.len();
            prefix = Tensor::cat(
                &[prefix, Tensor::zeros((1, n, c.hidden), DType::F32, device)?],
                1,
            )?;
            valid.extend(std::iter::repeat_n(false, n));
            groups.extend(std::iter::repeat_n(1, n));
        }
        require(
            valid.len() <= 2048,
            "Use a bounded SmolVLA observation prefix",
        )?;
        let n = valid.len();
        let mut position = -1_f32;
        let positions = valid
            .iter()
            .map(|v| {
                if *v {
                    position += 1.;
                }
                position
            })
            .collect::<Vec<_>>();
        let mut mask_values = Vec::with_capacity(n * n);
        for q in 0..n {
            for k in 0..n {
                mask_values.push((valid[q] && valid[k] && groups[k] <= groups[q]) as u8);
            }
        }
        let mask = Tensor::from_vec(mask_values, (1, n, n), device)?;
        let mut cache = Vec::new();
        for layer in &self.prefix {
            let h = prefix.apply(&layer.norm1)?;
            let q = rope(
                &h.apply(&layer.q)?.reshape((1, n, c.heads, c.head_dim))?,
                &positions,
            )?;
            let k = rope(
                &h.apply(&layer.k)?.reshape((1, n, c.kv_heads, c.head_dim))?,
                &positions,
            )?;
            let v = h.apply(&layer.v)?.reshape((1, n, c.kv_heads, c.head_dim))?;
            prefix = layer.finish(&prefix, &attention(&q, &k, &v, &mask, c.heads)?)?;
            cache.push((k, v));
        }
        let offset = valid.iter().filter(|v| **v).count() as f32;
        let dt = -1_f32 / c.denoise_steps as f32;
        let mut time = 1_f32;
        let mut x = noise.clone();
        for _ in 0..c.denoise_steps {
            let half = c.expert_hidden / 2;
            let periods = (0..half)
                .map(|i| {
                    c.min_period * (c.max_period / c.min_period).powf(i as f64 / (half - 1) as f64)
                })
                .collect::<Vec<_>>();
            let angles = periods
                .iter()
                .map(|p| f64::from(time) * std::f64::consts::TAU / p)
                .collect::<Vec<_>>();
            let time_embedding = Tensor::from_vec(
                angles
                    .iter()
                    .map(|a| a.sin() as f32)
                    .chain(angles.iter().map(|a| a.cos() as f32))
                    .collect::<Vec<_>>(),
                (1, 1, c.expert_hidden),
                device,
            )?
            .broadcast_as((1, c.chunk_size, c.expert_hidden))?;
            let mut suffix = Tensor::cat(&[x.apply(&self.action_in)?, time_embedding], 2)?
                .apply(&self.time_in)?
                .silu()?
                .apply(&self.time_out)?;
            let multiple = c.layers / c.expert_layers;
            for (i, (prefix_k, prefix_v)) in cache.iter().enumerate() {
                if !i.is_multiple_of(multiple) {
                    continue;
                }
                let layer = &self.expert[i / multiple];
                let h = suffix.apply(&layer.norm1)?;
                let q = h
                    .apply(&layer.q)?
                    .reshape((1, c.chunk_size, c.heads, c.head_dim))?;
                let cross = c.cross_attention
                    && !(c.self_attention_every > 0 && i.is_multiple_of(c.self_attention_every));
                let (q, k, v, mask) = if cross {
                    let positions = (0..c.chunk_size).map(|j| j as f32).collect::<Vec<_>>();
                    let k = prefix_k
                        .flatten_from(2)?
                        .apply(&layer.k)?
                        .reshape((1, n, c.kv_heads, c.head_dim))?;
                    let v = prefix_v
                        .flatten_from(2)?
                        .apply(&layer.v)?
                        .reshape((1, n, c.kv_heads, c.head_dim))?;
                    let mask = Tensor::from_vec(
                        (0..c.chunk_size)
                            .flat_map(|_| valid.iter().map(|v| *v as u8))
                            .collect::<Vec<_>>(),
                        (1, c.chunk_size, n),
                        device,
                    )?;
                    (rope(&q, &positions)?, k, v, mask)
                } else {
                    let positions = (0..c.chunk_size)
                        .map(|j| offset + j as f32)
                        .collect::<Vec<_>>();
                    let k = rope(
                        &h.apply(&layer.k)?
                            .reshape((1, c.chunk_size, c.kv_heads, c.head_dim))?,
                        &positions,
                    )?;
                    let v =
                        h.apply(&layer.v)?
                            .reshape((1, c.chunk_size, c.kv_heads, c.head_dim))?;
                    let mask = Tensor::from_vec(
                        (0..c.chunk_size)
                            .flat_map(|j| {
                                valid
                                    .iter()
                                    .map(|v| *v as u8)
                                    .chain((0..c.chunk_size).map(move |k| (k <= j) as u8))
                            })
                            .collect::<Vec<_>>(),
                        (1, c.chunk_size, n + c.chunk_size),
                        device,
                    )?;
                    (
                        rope(&q, &positions)?,
                        Tensor::cat(&[prefix_k, &k], 1)?,
                        Tensor::cat(&[prefix_v, &v], 1)?,
                        mask,
                    )
                };
                suffix = layer.finish(&suffix, &attention(&q, &k, &v, &mask, c.heads)?)?;
            }
            let velocity = suffix.apply(&self.expert_norm)?.apply(&self.action_out)?;
            x = (&x + velocity.affine(f64::from(dt), 0.)?)?;
            time += dt;
        }
        Ok(x)
    }
}
