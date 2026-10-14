//! ACT inference follows LeRobot v0.3.3 (Apache-2.0), with a zero latent
//! during inference. Training and stochastic VAE sampling remain offline.
use crate::{Result, require};
use candle_core::{DType, Device, Tensor};
use candle_nn::{Conv2d, LayerNorm, Linear, VarBuilder};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub dim_model: usize,
    pub n_heads: usize,
    pub dim_feedforward: usize,
    pub n_encoder_layers: usize,
    pub n_decoder_layers: usize,
    pub latent_dim: usize,
    pub chunk_size: usize,
    pub state_dim: usize,
    pub environment_dim: usize,
    pub action_dim: usize,
    pub cameras: usize,
    pub pre_norm: bool,
    pub activation: Activation,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    Relu,
    Gelu,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        require(
            (8..=512).contains(&self.dim_model)
                && self.dim_model.is_multiple_of(4)
                && (1..=16).contains(&self.n_heads)
                && self.dim_model.is_multiple_of(self.n_heads),
            "Choose a supported ACT attention size.",
        )?;
        require(
            (8..=4096).contains(&self.dim_feedforward)
                && (1..=8).contains(&self.n_encoder_layers)
                && (1..=8).contains(&self.n_decoder_layers)
                && (1..=256).contains(&self.latent_dim)
                && (1..=128).contains(&self.chunk_size),
            "Choose bounded ACT layers and action chunks.",
        )?;
        require(
            (1..=64).contains(&self.state_dim)
                && self.environment_dim <= 64
                && (1..=64).contains(&self.action_dim)
                && self.cameras <= 4
                && (self.cameras > 0 || self.environment_dim > 0),
            "Map ACT's robot state, environment or cameras, and actions.",
        )
    }
}
struct Attention {
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
    heads: usize,
}
impl Attention {
    fn load(config: &Config, vb: VarBuilder) -> candle_core::Result<Self> {
        let d = config.dim_model;
        let weight = vb.get((d * 3, d), "in_proj_weight")?;
        let bias = vb.get(d * 3, "in_proj_bias")?;
        let part = |index| -> candle_core::Result<Linear> {
            Ok(Linear::new(
                weight.narrow(0, index * d, d)?,
                Some(bias.narrow(0, index * d, d)?),
            ))
        };
        Ok(Self {
            q: part(0)?,
            k: part(1)?,
            v: part(2)?,
            out: candle_nn::linear(d, d, vb.pp("out_proj"))?,
            heads: config.n_heads,
        })
    }
    fn forward(&self, q: &Tensor, k: &Tensor, v: &Tensor) -> candle_core::Result<Tensor> {
        let (b, sq, d) = q.dims3()?;
        let sk = k.dim(1)?;
        let dh = d / self.heads;
        let q = q
            .apply(&self.q)?
            .reshape((b, sq, self.heads, dh))?
            .transpose(1, 2)?
            .contiguous()?;
        let k = k
            .apply(&self.k)?
            .reshape((b, sk, self.heads, dh))?
            .transpose(1, 2)?
            .contiguous()?;
        let v = v
            .apply(&self.v)?
            .reshape((b, sk, self.heads, dh))?
            .transpose(1, 2)?
            .contiguous()?;
        let scores = q
            .matmul(&k.transpose(2, 3)?)?
            .affine(1. / (dh as f64).sqrt(), 0.)?;
        candle_nn::ops::softmax_last_dim(&scores)?
            .matmul(&v)?
            .transpose(1, 2)?
            .contiguous()?
            .reshape((b, sq, d))?
            .apply(&self.out)
    }
}
struct Feed {
    first: Linear,
    second: Linear,
    activation: Activation,
}
impl Feed {
    fn load(config: &Config, vb: VarBuilder) -> candle_core::Result<Self> {
        Ok(Self {
            first: candle_nn::linear(config.dim_model, config.dim_feedforward, vb.pp("linear1"))?,
            second: candle_nn::linear(config.dim_feedforward, config.dim_model, vb.pp("linear2"))?,
            activation: config.activation,
        })
    }
    fn forward(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        let x = x.apply(&self.first)?;
        let x = match self.activation {
            Activation::Relu => x.relu()?,
            Activation::Gelu => x.gelu_erf()?,
        };
        x.apply(&self.second)
    }
}
struct Encoder {
    attention: Attention,
    feed: Feed,
    norm1: LayerNorm,
    norm2: LayerNorm,
    pre_norm: bool,
}
impl Encoder {
    fn load(config: &Config, vb: VarBuilder) -> candle_core::Result<Self> {
        Ok(Self {
            attention: Attention::load(config, vb.pp("self_attn"))?,
            feed: Feed::load(config, vb.clone())?,
            norm1: candle_nn::layer_norm(config.dim_model, 1e-5, vb.pp("norm1"))?,
            norm2: candle_nn::layer_norm(config.dim_model, 1e-5, vb.pp("norm2"))?,
            pre_norm: config.pre_norm,
        })
    }
    fn forward(&self, x: &Tensor, pos: &Tensor) -> candle_core::Result<Tensor> {
        let normalized = if self.pre_norm {
            x.apply(&self.norm1)?
        } else {
            x.clone()
        };
        let positioned = normalized.broadcast_add(pos)?;
        let x = (x + self
            .attention
            .forward(&positioned, &positioned, &normalized)?)?;
        if self.pre_norm {
            &x + self.feed.forward(&x.apply(&self.norm2)?)?
        } else {
            let x = x.apply(&self.norm1)?;
            (&x + self.feed.forward(&x)?)?.apply(&self.norm2)
        }
    }
}
struct Decoder {
    attention: Attention,
    cross: Attention,
    feed: Feed,
    norm1: LayerNorm,
    norm2: LayerNorm,
    norm3: LayerNorm,
    pre_norm: bool,
}
impl Decoder {
    fn load(config: &Config, vb: VarBuilder) -> candle_core::Result<Self> {
        Ok(Self {
            attention: Attention::load(config, vb.pp("self_attn"))?,
            cross: Attention::load(config, vb.pp("multihead_attn"))?,
            feed: Feed::load(config, vb.clone())?,
            norm1: candle_nn::layer_norm(config.dim_model, 1e-5, vb.pp("norm1"))?,
            norm2: candle_nn::layer_norm(config.dim_model, 1e-5, vb.pp("norm2"))?,
            norm3: candle_nn::layer_norm(config.dim_model, 1e-5, vb.pp("norm3"))?,
            pre_norm: config.pre_norm,
        })
    }
    fn forward(
        &self,
        x: &Tensor,
        memory: &Tensor,
        query_pos: &Tensor,
        memory_pos: &Tensor,
    ) -> candle_core::Result<Tensor> {
        let normalized = if self.pre_norm {
            x.apply(&self.norm1)?
        } else {
            x.clone()
        };
        let positioned = normalized.broadcast_add(query_pos)?;
        let x = (x + self
            .attention
            .forward(&positioned, &positioned, &normalized)?)?;
        let (skip, normalized) = if self.pre_norm {
            (x.clone(), x.apply(&self.norm2)?)
        } else {
            let x = x.apply(&self.norm1)?;
            (x.clone(), x)
        };
        let x = (&skip
            + self.cross.forward(
                &normalized.broadcast_add(query_pos)?,
                &memory.broadcast_add(memory_pos)?,
                memory,
            )?)?;
        if self.pre_norm {
            &x + self.feed.forward(&x.apply(&self.norm3)?)?
        } else {
            let x = x.apply(&self.norm2)?;
            (&x + self.feed.forward(&x)?)?.apply(&self.norm3)
        }
    }
}

/// Frozen BatchNorm is folded once into a scale and bias, as torchvision's
/// inference backbone does. No running statistics change during a call.
struct FrozenNorm {
    scale: Tensor,
    bias: Tensor,
}
impl FrozenNorm {
    fn load(channels: usize, vb: VarBuilder) -> candle_core::Result<Self> {
        let weight = vb.get(channels, "weight")?;
        let bias = vb.get(channels, "bias")?;
        let mean = vb.get(channels, "running_mean")?;
        let variance = vb.get(channels, "running_var")?;
        let scale = weight.div(&variance.affine(1., 1e-5)?.sqrt()?)?;
        let bias = (bias - mean.mul(&scale)?)?;
        Ok(Self {
            scale: scale.reshape((1, channels, 1, 1))?,
            bias: bias.reshape((1, channels, 1, 1))?,
        })
    }
    fn forward(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        x.broadcast_mul(&self.scale)?.broadcast_add(&self.bias)
    }
}
fn conv(
    input: usize,
    output: usize,
    kernel: usize,
    padding: usize,
    stride: usize,
    vb: VarBuilder,
) -> candle_core::Result<Conv2d> {
    candle_nn::conv2d_no_bias(
        input,
        output,
        kernel,
        candle_nn::Conv2dConfig {
            padding,
            stride,
            ..Default::default()
        },
        vb,
    )
}
struct Block {
    first: Conv2d,
    norm1: FrozenNorm,
    second: Conv2d,
    norm2: FrozenNorm,
    downsample: Option<(Conv2d, FrozenNorm)>,
}
impl Block {
    fn load(
        input: usize,
        output: usize,
        stride: usize,
        vb: VarBuilder,
    ) -> candle_core::Result<Self> {
        Ok(Self {
            first: conv(input, output, 3, 1, stride, vb.pp("conv1"))?,
            norm1: FrozenNorm::load(output, vb.pp("bn1"))?,
            second: conv(output, output, 3, 1, 1, vb.pp("conv2"))?,
            norm2: FrozenNorm::load(output, vb.pp("bn2"))?,
            downsample: if input != output || stride != 1 {
                Some((
                    conv(input, output, 1, 0, stride, vb.pp("downsample.0"))?,
                    FrozenNorm::load(output, vb.pp("downsample.1"))?,
                ))
            } else {
                None
            },
        })
    }
    fn forward(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        let y = self.norm1.forward(&x.apply(&self.first)?)?.relu()?;
        let y = self.norm2.forward(&y.apply(&self.second)?)?;
        let skip = if let Some((conv, norm)) = &self.downsample {
            norm.forward(&x.apply(conv)?)?
        } else {
            x.clone()
        };
        (skip + y)?.relu()
    }
}
struct Resnet {
    first: Conv2d,
    norm: FrozenNorm,
    blocks: Vec<Block>,
}
impl Resnet {
    fn load(vb: VarBuilder) -> candle_core::Result<Self> {
        let first = conv(3, 64, 7, 3, 2, vb.pp("conv1"))?;
        let norm = FrozenNorm::load(64, vb.pp("bn1"))?;
        let mut blocks = Vec::new();
        let mut input = 64;
        for (layer, output) in [64, 128, 256, 512].into_iter().enumerate() {
            for block in 0..2 {
                let stride = if layer > 0 && block == 0 { 2 } else { 1 };
                blocks.push(Block::load(
                    input,
                    output,
                    stride,
                    vb.pp(format!("layer{}.{}", layer + 1, block)),
                )?);
                input = output;
            }
        }
        Ok(Self {
            first,
            norm,
            blocks,
        })
    }
    fn forward(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        let mut x = self
            .norm
            .forward(&x.apply(&self.first)?)?
            .relu()?
            .pad_with_zeros(2, 1, 1)?
            .pad_with_zeros(3, 1, 1)?
            .max_pool2d_with_stride(3, 2)?;
        for block in &self.blocks {
            x = block.forward(&x)?;
        }
        Ok(x)
    }
}

pub struct Act {
    config: Config,
    latent: Linear,
    state: Linear,
    environment: Option<Linear>,
    backbone: Option<Resnet>,
    image: Option<Conv2d>,
    position: Tensor,
    query: Tensor,
    encoder: Vec<Encoder>,
    encoder_norm: Option<LayerNorm>,
    decoder: Vec<Decoder>,
    decoder_norm: LayerNorm,
    head: Linear,
}
impl Act {
    pub fn load(config: Config, vb: VarBuilder) -> Result<Self> {
        config.validate()?;
        let d = config.dim_model;
        let encoder = (0..config.n_encoder_layers)
            .map(|i| Encoder::load(&config, vb.pp(format!("encoder.layers.{i}"))))
            .collect::<candle_core::Result<Vec<_>>>()?;
        let decoder = (0..config.n_decoder_layers)
            .map(|i| Decoder::load(&config, vb.pp(format!("decoder.layers.{i}"))))
            .collect::<candle_core::Result<Vec<_>>>()?;
        let has_environment = config.environment_dim > 0;
        let has_camera = config.cameras > 0;
        let model = Self {
            latent: candle_nn::linear(config.latent_dim, d, vb.pp("encoder_latent_input_proj"))?,
            state: candle_nn::linear(config.state_dim, d, vb.pp("encoder_robot_state_input_proj"))?,
            environment: if has_environment {
                Some(candle_nn::linear(
                    config.environment_dim,
                    d,
                    vb.pp("encoder_env_state_input_proj"),
                )?)
            } else {
                None
            },
            backbone: if has_camera {
                Some(Resnet::load(vb.pp("backbone"))?)
            } else {
                None
            },
            image: if has_camera {
                Some(candle_nn::conv2d(
                    512,
                    d,
                    1,
                    Default::default(),
                    vb.pp("encoder_img_feat_input_proj"),
                )?)
            } else {
                None
            },
            position: vb
                .get(
                    (2 + usize::from(has_environment), d),
                    "encoder_1d_feature_pos_embed.weight",
                )?
                .unsqueeze(0)?,
            query: vb
                .get((config.chunk_size, d), "decoder_pos_embed.weight")?
                .unsqueeze(0)?,
            encoder,
            encoder_norm: if config.pre_norm {
                Some(candle_nn::layer_norm(d, 1e-5, vb.pp("encoder.norm"))?)
            } else {
                None
            },
            decoder,
            decoder_norm: candle_nn::layer_norm(d, 1e-5, vb.pp("decoder.norm"))?,
            head: candle_nn::linear(d, config.action_dim, vb.pp("action_head"))?,
            config,
        };
        Ok(model)
    }
    pub fn config(&self) -> &Config {
        &self.config
    }
    pub fn forward(
        &self,
        state: &Tensor,
        environment: Option<&Tensor>,
        images: &[Tensor],
    ) -> Result<Tensor> {
        let (batch, width) = state.dims2()?;
        require(
            batch == 1
                && width == self.config.state_dim
                && state.dtype() == DType::F32
                && images.len() == self.config.cameras,
            "Use one mapped ACT observation with the expected state and cameras.",
        )?;
        let latent = Tensor::zeros((batch, self.config.latent_dim), DType::F32, state.device())?
            .apply(&self.latent)?
            .unsqueeze(1)?;
        let mut tokens = vec![latent, state.apply(&self.state)?.unsqueeze(1)?];
        if let Some(project) = &self.environment {
            let env = environment.ok_or_else(|| {
                crate::Error::Invalid("This ACT policy needs the mapped environment state.".into())
            })?;
            require(
                env.dims() == [batch, self.config.environment_dim],
                "Use the expected ACT environment state.",
            )?;
            tokens.push(env.apply(project)?.unsqueeze(1)?);
        } else {
            require(
                environment.is_none(),
                "This ACT policy doesn't use environment state.",
            )?;
        }
        let mut positions = vec![self.position.clone()];
        let mut length = tokens.len();
        if let (Some(backbone), Some(project)) = (&self.backbone, &self.image) {
            for image in images {
                let (b, c, h, w) = image.dims4()?;
                require(
                    b == batch
                        && c == 3
                        && (16..=640).contains(&h)
                        && (16..=640).contains(&w)
                        && image.dtype() == DType::F32,
                    "Use bounded three-channel ACT camera observations.",
                )?;
                let features = backbone.forward(image)?;
                let (_, _, h, w) = features.dims4()?;
                length += h * w;
                require(
                    length <= 1600,
                    "This ACT observation has too many image features.",
                )?;
                positions.push(image_position(h, w, self.config.dim_model, state.device())?);
                tokens.push(
                    features
                        .apply(project)?
                        .flatten_from(2)?
                        .transpose(1, 2)?
                        .contiguous()?,
                );
            }
        }
        let position = Tensor::cat(&positions, 1)?;
        let mut memory = Tensor::cat(&tokens, 1)?;
        for layer in &self.encoder {
            memory = layer.forward(&memory, &position)?;
        }
        if let Some(norm) = &self.encoder_norm {
            memory = memory.apply(norm)?;
        }
        let mut output = Tensor::zeros(
            (batch, self.config.chunk_size, self.config.dim_model),
            DType::F32,
            state.device(),
        )?;
        for layer in &self.decoder {
            output = layer.forward(&output, &memory, &self.query, &position)?;
        }
        Ok(output.apply(&self.decoder_norm)?.apply(&self.head)?)
    }
}
fn image_position(
    height: usize,
    width: usize,
    dimension: usize,
    device: &Device,
) -> candle_core::Result<Tensor> {
    let half = dimension / 2;
    let mut values = Vec::with_capacity(height * width * dimension);
    for y in 1..=height {
        for x in 1..=width {
            for coordinate in [
                y as f32 / (height as f32 + 1e-6) * std::f32::consts::TAU,
                x as f32 / (width as f32 + 1e-6) * std::f32::consts::TAU,
            ] {
                for i in 0..half {
                    let inverse = 10000_f32.powf(2. * (i / 2) as f32 / half as f32);
                    let angle = coordinate / inverse;
                    values.push(if i % 2 == 0 { angle.sin() } else { angle.cos() });
                }
            }
        }
    }
    Tensor::from_vec(values, (1, height * width, dimension), device)
}
