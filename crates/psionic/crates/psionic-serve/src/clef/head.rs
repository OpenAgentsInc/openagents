//! The Clef joint schema head in f32 on the CPU: a port of the reference
//! `JointSchemaHead.forward` (`joint_schema_model.py`).
//!
//! The head reads the backbone's final hidden rows only through three
//! quantities, so [`ClefHeadStream`] takes the rows one at a time and keeps
//! just those: the 1024-wide memory rows `LN(H) W_mem`, the span sums of
//! `LN(H)` over each question and option span, and the last normalized row.
//! The 4096-wide activations are dropped as they arrive.
//!
//! Memory attention (options and fields attending to every memory row) runs
//! through [`MemoryAttention`] in a query-side form: MHA has no mask, so a
//! head's score against memory row `m` is `(W_k,h^T q_h) . m` plus a
//! per-query constant that cancels in the softmax, and its output is
//! `W_v,h (sum_t p_t m_t) + b_v,h` because the weights sum to one. Nothing
//! is projected per memory row, so the cost is (queries x heads x L x width)
//! instead of (L x width^2) per layer, and the rows can stay on a device
//! ([`HostMemory`] is the CPU provider).

use std::collections::BTreeMap;
use std::path::Path;

use psionic_models::GgufBlobArtifact;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use super::encode::EncodedRecord;

/// LayerNorm epsilon (`torch.nn.LayerNorm` default; the GGUF records it as
/// `clef.attention.layer_norm_epsilon`).
pub const HEAD_NORM_EPSILON: f32 = 1e-5;

/// Head dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefHeadConfig {
    pub hidden_size: usize,
    pub width: usize,
    pub routing_layers: usize,
    pub layers: usize,
    pub heads: usize,
    pub feedforward: usize,
}

/// A dense row-major matrix `[rows, columns]` (`torch.nn.Linear.weight`).
#[derive(Clone, Debug)]
pub struct Matrix {
    pub rows: usize,
    pub columns: usize,
    pub values: Vec<f32>,
}

impl Matrix {
    fn row(&self, index: usize) -> &[f32] {
        &self.values[index * self.columns..(index + 1) * self.columns]
    }

    /// `W x (+ b)`.
    fn apply(&self, input: &[f32], bias: Option<&[f32]>) -> Vec<f32> {
        debug_assert_eq!(input.len(), self.columns);
        let compute = |row: usize| {
            let value = dot(self.row(row), input);
            bias.map_or(value, |bias| value + bias[row])
        };
        if self.rows * self.columns >= 1 << 18 {
            (0..self.rows).into_par_iter().map(compute).collect()
        } else {
            (0..self.rows).map(compute).collect()
        }
    }

    /// `X W^T (+ b)` for `n` rows of `X`.
    pub(crate) fn apply_rows(&self, input: &[f32], n: usize, bias: Option<&[f32]>) -> Vec<f32> {
        debug_assert_eq!(input.len(), n * self.columns);
        let mut out = vec![0.0f32; n * self.rows];
        out.par_chunks_mut(self.rows)
            .zip(input.par_chunks(self.columns))
            .for_each(|(out_row, in_row)| {
                for (index, slot) in out_row.iter_mut().enumerate() {
                    let value = dot(self.row(index), in_row);
                    *slot = bias.map_or(value, |bias| value + bias[index]);
                }
            });
        out
    }
}

/// LayerNorm weight and bias.
#[derive(Clone, Debug)]
pub struct Norm {
    pub weight: Vec<f32>,
    pub bias: Vec<f32>,
}

impl Norm {
    fn apply(&self, input: &[f32]) -> Vec<f32> {
        layer_norm(input, &self.weight, &self.bias, HEAD_NORM_EPSILON)
    }

    fn apply_rows(&self, input: &[f32], width: usize) -> Vec<f32> {
        input
            .par_chunks(width)
            .flat_map_iter(|row| self.apply(row))
            .collect()
    }
}

/// `torch.nn.MultiheadAttention` with packed q/k/v split into three linears.
#[derive(Clone, Debug)]
pub struct Attention {
    pub q: Matrix,
    pub q_bias: Vec<f32>,
    pub k: Matrix,
    pub k_bias: Vec<f32>,
    pub v: Matrix,
    pub v_bias: Vec<f32>,
    pub out: Matrix,
    pub out_bias: Vec<f32>,
}

/// One evidence-routing layer (options attend to memory).
#[derive(Clone, Debug)]
pub struct EvidenceLayer {
    pub query_norm: Norm,
    pub memory_norm: Norm,
    pub attention: Attention,
    pub feedforward_norm: Norm,
    pub up: Matrix,
    pub up_bias: Vec<f32>,
    pub down: Matrix,
    pub down_bias: Vec<f32>,
}

/// One field layer (`nn.TransformerDecoderLayer`, `norm_first`, GELU).
#[derive(Clone, Debug)]
pub struct FieldLayer {
    pub self_norm: Norm,
    pub self_attention: Attention,
    pub cross_norm: Norm,
    pub cross_attention: Attention,
    pub feedforward_norm: Norm,
    pub up: Matrix,
    pub up_bias: Vec<f32>,
    pub down: Matrix,
    pub down_bias: Vec<f32>,
}

/// The whole head in f32.
#[derive(Clone, Debug)]
pub struct ClefHeadWeights {
    pub config: ClefHeadConfig,
    pub hidden_norm: Norm,
    pub memory_projection: Matrix,
    pub question_projection: Matrix,
    pub option_question_projection: Matrix,
    pub global_projection: Matrix,
    pub option_context_projection: Matrix,
    pub option_lexical_projection: Matrix,
    /// `[3, width]`: noul, choice, score.
    pub type_embedding: Matrix,
    pub evidence_layers: Vec<EvidenceLayer>,
    pub option_summary_norm: Norm,
    pub layers: Vec<FieldLayer>,
    pub field_norm: Norm,
    pub option_norm: Norm,
    pub scorer: Matrix,
    pub scorer_bias: Vec<f32>,
    pub scorer_out: Vec<f32>,
    pub scorer_out_bias: f32,
    /// `exp(min(prior_logit_scale, ln 100))`.
    pub prior_scale: f32,
    /// `exp(min(joint_logit_scale, ln 100))`.
    pub joint_scale: f32,
    /// `sigmoid(residual_gate)`.
    pub gate: f32,
    /// Where the head came from: `gguf` or `safetensors`.
    pub source: &'static str,
    /// SHA-256 over the head's tensors (name, dtype, shape, bytes; sorted by
    /// name), or of the safetensors file.
    pub digest: String,
}

impl ClefHeadWeights {
    /// Every matrix the head multiplies through [`MemoryAttention::linear`]
    /// (a device backend uploads these once).
    #[must_use]
    pub fn linear_matrices(&self) -> Vec<&Matrix> {
        let mut out = vec![
            &self.question_projection,
            &self.option_question_projection,
            &self.global_projection,
            &self.option_context_projection,
            &self.option_lexical_projection,
            &self.scorer,
        ];
        for layer in &self.evidence_layers {
            let a = &layer.attention;
            out.extend([&a.q, &a.k, &a.v, &a.out, &layer.up, &layer.down]);
        }
        for layer in &self.layers {
            let (a, c) = (&layer.self_attention, &layer.cross_attention);
            out.extend([&a.q, &a.k, &a.v, &a.out, &c.q, &c.k, &c.v, &c.out, &layer.up, &layer.down]);
        }
        out
    }
}

/// Why a head could not be admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadLoadError(pub String);

impl std::fmt::Display for HeadLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HeadLoadError {}

fn err(message: impl Into<String>) -> HeadLoadError {
    HeadLoadError(message.into())
}

/// A source of named f32 tensors with shapes in torch order.
trait TensorSource {
    fn take(&mut self, name: &str) -> Result<(Vec<usize>, Vec<f32>), HeadLoadError>;
    /// Names still unread after loading: unknown tensors refuse the head.
    fn leftover(&self) -> Vec<String>;
}

struct Loader<'a> {
    source: &'a mut dyn TensorSource,
}

impl Loader<'_> {
    fn vector(&mut self, name: &str, len: usize) -> Result<Vec<f32>, HeadLoadError> {
        let (shape, values) = self.source.take(name)?;
        if shape.iter().product::<usize>() != len || values.len() != len {
            return Err(err(format!(
                "head tensor `{name}` has shape {shape:?}; expected [{len}]"
            )));
        }
        Ok(values)
    }

    fn matrix(&mut self, name: &str, rows: usize, columns: usize) -> Result<Matrix, HeadLoadError> {
        let (shape, values) = self.source.take(name)?;
        let ok = match shape.as_slice() {
            [r, c] => *r == rows && *c == columns,
            [n] => rows == 1 && *n == columns,
            _ => false,
        };
        if !ok || values.len() != rows * columns {
            return Err(err(format!(
                "head tensor `{name}` has shape {shape:?}; expected [{rows}, {columns}]"
            )));
        }
        Ok(Matrix {
            rows,
            columns,
            values,
        })
    }

    fn norm(&mut self, prefix: &str, width: usize) -> Result<Norm, HeadLoadError> {
        Ok(Norm {
            weight: self.vector(&format!("{prefix}.weight"), width)?,
            bias: self.vector(&format!("{prefix}.bias"), width)?,
        })
    }

    fn scalar(&mut self, name: &str) -> Result<f32, HeadLoadError> {
        let (shape, values) = self.source.take(name)?;
        if values.len() != 1 {
            return Err(err(format!(
                "head scalar `{name}` has shape {shape:?}; expected a scalar"
            )));
        }
        Ok(values[0])
    }

    /// GGUF names: `<prefix>_{q,k,v,o}.{weight,bias}`.
    fn split_attention(&mut self, prefix: &str, width: usize) -> Result<Attention, HeadLoadError> {
        Ok(Attention {
            q: self.matrix(&format!("{prefix}_q.weight"), width, width)?,
            q_bias: self.vector(&format!("{prefix}_q.bias"), width)?,
            k: self.matrix(&format!("{prefix}_k.weight"), width, width)?,
            k_bias: self.vector(&format!("{prefix}_k.bias"), width)?,
            v: self.matrix(&format!("{prefix}_v.weight"), width, width)?,
            v_bias: self.vector(&format!("{prefix}_v.bias"), width)?,
            out: self.matrix(&format!("{prefix}_o.weight"), width, width)?,
            out_bias: self.vector(&format!("{prefix}_o.bias"), width)?,
        })
    }

    /// torch names: `<prefix>.in_proj_{weight,bias}`, `<prefix>.out_proj.*`.
    fn packed_attention(&mut self, prefix: &str, width: usize) -> Result<Attention, HeadLoadError> {
        let packed = self.matrix(&format!("{prefix}.in_proj_weight"), 3 * width, width)?;
        let packed_bias = self.vector(&format!("{prefix}.in_proj_bias"), 3 * width)?;
        let part = |index: usize| Matrix {
            rows: width,
            columns: width,
            values: packed.values[index * width * width..(index + 1) * width * width].to_vec(),
        };
        let bias = |index: usize| packed_bias[index * width..(index + 1) * width].to_vec();
        Ok(Attention {
            q: part(0),
            q_bias: bias(0),
            k: part(1),
            k_bias: bias(1),
            v: part(2),
            v_bias: bias(2),
            out: self.matrix(&format!("{prefix}.out_proj.weight"), width, width)?,
            out_bias: self.vector(&format!("{prefix}.out_proj.bias"), width)?,
        })
    }
}

fn finish(source: &dyn TensorSource) -> Result<(), HeadLoadError> {
    let leftover = source.leftover();
    if leftover.is_empty() {
        Ok(())
    } else {
        Err(err(format!(
            "unknown head tensors: {}",
            leftover.join(", ")
        )))
    }
}

/// Head tensors read from a Clef GGUF (`decision.*`, `dec.blk.*`,
/// `token_types`), dequantized to f32.
struct GgufHeadSource<'a> {
    artifact: &'a GgufBlobArtifact,
    remaining: std::collections::BTreeSet<String>,
}

impl TensorSource for GgufHeadSource<'_> {
    fn take(&mut self, name: &str) -> Result<(Vec<usize>, Vec<f32>), HeadLoadError> {
        if !self.remaining.remove(name) {
            return Err(err(format!(
                "the Clef artifact is missing head tensor `{name}`"
            )));
        }
        let tensor = self
            .artifact
            .load_tensor(name)
            .map_err(|error| err(format!("head tensor `{name}`: {error}")))?;
        let shape = tensor.metadata().shape.dims().to_vec();
        let values = tensor
            .values()
            .map_err(|error| err(format!("head tensor `{name}`: {error}")))?
            .into_owned();
        Ok((shape, values))
    }

    fn leftover(&self) -> Vec<String> {
        self.remaining.iter().cloned().collect()
    }
}

/// Whether a GGUF tensor name belongs to the decision head.
#[must_use]
pub fn is_gguf_head_tensor(name: &str) -> bool {
    name.starts_with("decision.") || name.starts_with("dec.") || name.starts_with("token_types")
}

fn read_u32(metadata: &psionic_models::GgufContent, key: &str) -> Result<usize, HeadLoadError> {
    metadata
        .metadata()
        .get(key)
        .and_then(psionic_models::GgufMetadataValue::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| err(format!("the Clef artifact is missing metadata `{key}`")))
}

impl ClefHeadWeights {
    /// Loads the head from a Clef GGUF (`ggml-org/Clef-Flash-GGUF` layout).
    pub fn from_gguf(
        artifact: &GgufBlobArtifact,
        hidden_size: usize,
    ) -> Result<Self, HeadLoadError> {
        let content = artifact.content();
        let decision_type = content
            .metadata()
            .get("clef.decision.type")
            .and_then(|value| match value {
                psionic_models::GgufMetadataValue::String(value) => Some(value.as_str()),
                _ => None,
            });
        if decision_type != Some("clef") {
            return Err(err(
                "the artifact has no Clef decision head (`clef.decision.type` is not `clef`)",
            ));
        }
        let routing_layers = read_u32(content, "clef.decision.routing_block_count")?;
        let layers = read_u32(content, "clef.decision.block_count")?;
        let heads = read_u32(content, "clef.decision.head_count")?;
        if let Some(epsilon) = content
            .metadata()
            .get("clef.attention.layer_norm_epsilon")
            .and_then(psionic_models::GgufMetadataValue::as_f32)
            && (epsilon - HEAD_NORM_EPSILON).abs() > 1e-9
        {
            return Err(err(format!(
                "the Clef head LayerNorm epsilon is {epsilon}; this server implements {HEAD_NORM_EPSILON}"
            )));
        }
        let memory = content
            .tensor_info("decision.proj_memory.weight")
            .ok_or_else(|| err("the Clef artifact is missing `decision.proj_memory.weight`"))?;
        let [width, memory_input] = memory.shape.dims() else {
            return Err(err("`decision.proj_memory.weight` is not a matrix"));
        };
        if *memory_input != hidden_size {
            return Err(err(format!(
                "the Clef head reads hidden size {memory_input}, the backbone has {hidden_size}"
            )));
        }
        let feedforward = content
            .tensor_info("dec.blk.0.ffn_up.weight")
            .and_then(|info| info.shape.dims().first().copied())
            .ok_or_else(|| err("the Clef artifact is missing `dec.blk.0.ffn_up.weight`"))?;
        let config = ClefHeadConfig {
            hidden_size,
            width: *width,
            routing_layers,
            layers,
            heads,
            feedforward,
        };
        let mut remaining: std::collections::BTreeSet<String> = content
            .tensor_infos()
            .map(|info| info.name.clone())
            .filter(|name| is_gguf_head_tensor(name))
            .collect();
        let digest = {
            let mut hasher = Sha256::new();
            for name in &remaining {
                let info = content
                    .tensor_info(name)
                    .ok_or_else(|| err("tensor table changed"))?;
                hasher.update(name.as_bytes());
                hasher.update(format!("{:?}{:?}", info.tensor_type, info.shape.dims()).as_bytes());
                let storage = artifact
                    .paged_tensor(name)
                    .map_err(|error| err(format!("head tensor `{name}`: {error}")))?;
                hasher.update(
                    storage
                        .bytes()
                        .map_err(|error| err(format!("head tensor `{name}`: {error}")))?,
                );
            }
            format!("sha256:{}", hex::encode(hasher.finalize()))
        };
        // `remaining` is consumed by the loader; keep the set for it.
        let mut source = GgufHeadSource {
            artifact,
            remaining: std::mem::take(&mut remaining),
        };
        let weights = load_gguf_layout(&mut source, config, digest)?;
        finish(&source)?;
        Ok(weights)
    }

    /// Loads the Hugging Face head (`joint_head.safetensors` and
    /// `joint_head_config.json`, bf16 or f32) from a directory or from the
    /// safetensors path (the config is read beside it).
    pub fn from_safetensors(path: &Path, hidden_size: usize) -> Result<Self, HeadLoadError> {
        let (file, directory) = if path.is_dir() {
            (path.join("joint_head.safetensors"), path.to_path_buf())
        } else {
            (
                path.to_path_buf(),
                path.parent().map(Path::to_path_buf).unwrap_or_default(),
            )
        };
        let config_path = directory.join("joint_head_config.json");
        let config_text = std::fs::read_to_string(&config_path)
            .map_err(|error| err(format!("{}: {error}", config_path.display())))?;
        let config_json: serde_json::Value = serde_json::from_str(&config_text)
            .map_err(|error| err(format!("{}: {error}", config_path.display())))?;
        let field = |key: &str| {
            config_json
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize)
                .ok_or_else(|| err(format!("{}: `{key}` is missing", config_path.display())))
        };
        let config = ClefHeadConfig {
            hidden_size: field("hidden_size")?,
            width: field("width")?,
            routing_layers: field("routing_layers")?,
            layers: field("layers")?,
            heads: field("heads")?,
            feedforward: field("feedforward")?,
        };
        if config.hidden_size != hidden_size {
            return Err(err(format!(
                "the Clef head reads hidden size {}, the backbone has {hidden_size}",
                config.hidden_size
            )));
        }
        let bytes =
            std::fs::read(&file).map_err(|error| err(format!("{}: {error}", file.display())))?;
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        let tensors = safetensors::SafeTensors::deserialize(&bytes)
            .map_err(|error| err(format!("{}: {error}", file.display())))?;
        let mut source = SafetensorsSource {
            tensors: BTreeMap::new(),
        };
        for (name, view) in tensors.tensors() {
            let values = safetensors_to_f32(&name, &view)?;
            source
                .tensors
                .insert(name.clone(), (view.shape().to_vec(), values));
        }
        let weights = load_torch_layout(&mut source, config, digest)?;
        finish(&source)?;
        Ok(weights)
    }
}

struct SafetensorsSource {
    tensors: BTreeMap<String, (Vec<usize>, Vec<f32>)>,
}

impl TensorSource for SafetensorsSource {
    fn take(&mut self, name: &str) -> Result<(Vec<usize>, Vec<f32>), HeadLoadError> {
        self.tensors
            .remove(name)
            .ok_or_else(|| err(format!("the head file is missing tensor `{name}`")))
    }

    fn leftover(&self) -> Vec<String> {
        self.tensors.keys().cloned().collect()
    }
}

fn safetensors_to_f32(
    name: &str,
    view: &safetensors::tensor::TensorView<'_>,
) -> Result<Vec<f32>, HeadLoadError> {
    let data = view.data();
    Ok(match view.dtype() {
        safetensors::Dtype::F32 => data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        safetensors::Dtype::BF16 => data
            .chunks_exact(2)
            .map(|b| f32::from_bits(u32::from(u16::from_le_bytes([b[0], b[1]])) << 16))
            .collect(),
        safetensors::Dtype::F16 => data
            .chunks_exact(2)
            .map(|b| f16_to_f32(u16::from_le_bytes([b[0], b[1]])))
            .collect(),
        other => {
            return Err(err(format!(
                "head tensor `{name}` has dtype {other:?}; expected F32, BF16 or F16"
            )));
        }
    })
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits >> 15) << 31;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let mantissa = u32::from(bits & 0x3ff);
    let value = if exponent == 0 {
        if mantissa == 0 {
            sign
        } else {
            // subnormal
            let magnitude = mantissa as f32 * 2f32.powi(-24);
            return if sign != 0 { -magnitude } else { magnitude };
        }
    } else if exponent == 31 {
        sign | 0x7f80_0000 | (mantissa << 13)
    } else {
        sign | ((exponent + 112) << 23) | (mantissa << 13)
    };
    f32::from_bits(value)
}

fn load_gguf_layout(
    source: &mut dyn TensorSource,
    config: ClefHeadConfig,
    digest: String,
) -> Result<ClefHeadWeights, HeadLoadError> {
    let ClefHeadConfig {
        hidden_size: d,
        width: w,
        routing_layers,
        layers,
        feedforward: ff,
        ..
    } = config;
    let mut l = Loader { source };
    let hidden_norm = l.norm("decision.hidden_norm", d)?;
    let memory_projection = l.matrix("decision.proj_memory.weight", w, d)?;
    let question_projection = l.matrix("decision.proj_question.weight", w, d)?;
    let option_question_projection = l.matrix("decision.proj_option_question.weight", w, d)?;
    let global_projection = l.matrix("decision.proj_global.weight", w, d)?;
    let option_context_projection = l.matrix("decision.proj_option_context.weight", w, d)?;
    let option_lexical_projection = l.matrix("decision.proj_option_lexical.weight", w, d)?;
    let type_embedding = l.matrix("token_types.weight", 3, w)?;
    let mut evidence_layers = Vec::with_capacity(routing_layers);
    for index in 0..routing_layers {
        let p = format!("dec.blk.{index}");
        evidence_layers.push(EvidenceLayer {
            query_norm: l.norm(&format!("{p}.cross_attn_norm"), w)?,
            memory_norm: l.norm(&format!("{p}.cross_attn_norm_kv"), w)?,
            attention: l.split_attention(&format!("{p}.cross_attn"), w)?,
            feedforward_norm: l.norm(&format!("{p}.ffn_norm"), w)?,
            up: l.matrix(&format!("{p}.ffn_up.weight"), ff, w)?,
            up_bias: l.vector(&format!("{p}.ffn_up.bias"), ff)?,
            down: l.matrix(&format!("{p}.ffn_down.weight"), w, ff)?,
            down_bias: l.vector(&format!("{p}.ffn_down.bias"), w)?,
        });
    }
    let mut field_layers = Vec::with_capacity(layers);
    for index in routing_layers..routing_layers + layers {
        let p = format!("dec.blk.{index}");
        field_layers.push(FieldLayer {
            self_norm: l.norm(&format!("{p}.attn_norm"), w)?,
            self_attention: l.split_attention(&format!("{p}.attn"), w)?,
            cross_norm: l.norm(&format!("{p}.cross_attn_norm"), w)?,
            cross_attention: l.split_attention(&format!("{p}.cross_attn"), w)?,
            feedforward_norm: l.norm(&format!("{p}.ffn_norm"), w)?,
            up: l.matrix(&format!("{p}.ffn_up.weight"), ff, w)?,
            up_bias: l.vector(&format!("{p}.ffn_up.bias"), ff)?,
            down: l.matrix(&format!("{p}.ffn_down.weight"), w, ff)?,
            down_bias: l.vector(&format!("{p}.ffn_down.bias"), w)?,
        });
    }
    let option_summary_norm = l.norm("decision.option_summary_norm", w)?;
    let field_norm = l.norm("decision.field_norm", w)?;
    let option_norm = l.norm("decision.option_norm", w)?;
    let scorer = l.matrix("decision.scorer.weight", w, 4 * w)?;
    let scorer_bias = l.vector("decision.scorer.bias", w)?;
    let scorer_out = l.matrix("decision.scorer_out.weight", 1, w)?.values;
    let scorer_out_bias = l.vector("decision.scorer_out.bias", 1)?[0];
    // The converter stores the scales as used at inference.
    let scales = l.vector("decision.scales", 3)?;
    validate_config(&config)?;
    Ok(ClefHeadWeights {
        config,
        hidden_norm,
        memory_projection,
        question_projection,
        option_question_projection,
        global_projection,
        option_context_projection,
        option_lexical_projection,
        type_embedding,
        evidence_layers,
        option_summary_norm,
        layers: field_layers,
        field_norm,
        option_norm,
        scorer,
        scorer_bias,
        scorer_out,
        scorer_out_bias,
        prior_scale: scales[0],
        joint_scale: scales[1],
        gate: scales[2],
        source: "gguf",
        digest,
    })
}

fn load_torch_layout(
    source: &mut dyn TensorSource,
    config: ClefHeadConfig,
    digest: String,
) -> Result<ClefHeadWeights, HeadLoadError> {
    let ClefHeadConfig {
        hidden_size: d,
        width: w,
        routing_layers,
        layers,
        feedforward: ff,
        ..
    } = config;
    let mut l = Loader { source };
    let hidden_norm = l.norm("hidden_norm", d)?;
    let memory_projection = l.matrix("memory_projection.weight", w, d)?;
    let question_projection = l.matrix("question_projection.weight", w, d)?;
    let option_question_projection = l.matrix("option_question_projection.weight", w, d)?;
    let global_projection = l.matrix("global_projection.weight", w, d)?;
    let option_context_projection = l.matrix("option_context_projection.weight", w, d)?;
    let option_lexical_projection = l.matrix("option_lexical_projection.weight", w, d)?;
    let type_embedding = l.matrix("type_embedding.weight", 3, w)?;
    let mut evidence_layers = Vec::with_capacity(routing_layers);
    for index in 0..routing_layers {
        let p = format!("evidence_layers.{index}");
        evidence_layers.push(EvidenceLayer {
            query_norm: l.norm(&format!("{p}.query_norm"), w)?,
            memory_norm: l.norm(&format!("{p}.memory_norm"), w)?,
            attention: l.packed_attention(&format!("{p}.attention"), w)?,
            feedforward_norm: l.norm(&format!("{p}.feedforward_norm"), w)?,
            up: l.matrix(&format!("{p}.feedforward.0.weight"), ff, w)?,
            up_bias: l.vector(&format!("{p}.feedforward.0.bias"), ff)?,
            down: l.matrix(&format!("{p}.feedforward.3.weight"), w, ff)?,
            down_bias: l.vector(&format!("{p}.feedforward.3.bias"), w)?,
        });
    }
    let mut field_layers = Vec::with_capacity(layers);
    for index in 0..layers {
        let p = format!("layers.{index}");
        field_layers.push(FieldLayer {
            self_norm: l.norm(&format!("{p}.norm1"), w)?,
            self_attention: l.packed_attention(&format!("{p}.self_attn"), w)?,
            cross_norm: l.norm(&format!("{p}.norm2"), w)?,
            cross_attention: l.packed_attention(&format!("{p}.multihead_attn"), w)?,
            feedforward_norm: l.norm(&format!("{p}.norm3"), w)?,
            up: l.matrix(&format!("{p}.linear1.weight"), ff, w)?,
            up_bias: l.vector(&format!("{p}.linear1.bias"), ff)?,
            down: l.matrix(&format!("{p}.linear2.weight"), w, ff)?,
            down_bias: l.vector(&format!("{p}.linear2.bias"), w)?,
        });
    }
    let option_summary_norm = l.norm("option_summary_norm", w)?;
    let field_norm = l.norm("field_norm", w)?;
    let option_norm = l.norm("option_norm", w)?;
    let scorer = l.matrix("residual_scorer.0.weight", w, 4 * w)?;
    let scorer_bias = l.vector("residual_scorer.0.bias", w)?;
    let scorer_out = l.matrix("residual_scorer.3.weight", 1, w)?.values;
    let scorer_out_bias = l.vector("residual_scorer.3.bias", 1)?[0];
    let prior = l.scalar("prior_logit_scale")?;
    let joint = l.scalar("joint_logit_scale")?;
    let gate = l.scalar("residual_gate")?;
    validate_config(&config)?;
    let cap = 100f32.ln();
    Ok(ClefHeadWeights {
        config,
        hidden_norm,
        memory_projection,
        question_projection,
        option_question_projection,
        global_projection,
        option_context_projection,
        option_lexical_projection,
        type_embedding,
        evidence_layers,
        option_summary_norm,
        layers: field_layers,
        field_norm,
        option_norm,
        scorer,
        scorer_bias,
        scorer_out,
        scorer_out_bias,
        prior_scale: prior.min(cap).exp(),
        joint_scale: joint.min(cap).exp(),
        gate: 1.0 / (1.0 + (-gate).exp()),
        source: "safetensors",
        digest,
    })
}

fn validate_config(config: &ClefHeadConfig) -> Result<(), HeadLoadError> {
    if config.heads == 0 || config.width % config.heads != 0 {
        return Err(err(format!(
            "head width {} is not divisible by {} attention heads",
            config.width, config.heads
        )));
    }
    Ok(())
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    // Eight independent lanes so the compiler can vectorize; the order is
    // fixed, so a repeated call is bitwise identical.
    let mut lanes = [0.0f32; 8];
    let chunks = left.len() / 8;
    for chunk in 0..chunks {
        let base = chunk * 8;
        for lane in 0..8 {
            lanes[lane] += left[base + lane] * right[base + lane];
        }
    }
    let mut sum = 0.0f32;
    for index in chunks * 8..left.len() {
        sum += left[index] * right[index];
    }
    lanes.iter().sum::<f32>() + sum
}

fn layer_norm(input: &[f32], weight: &[f32], bias: &[f32], epsilon: f32) -> Vec<f32> {
    let n = input.len() as f64;
    let mean = input.iter().map(|value| f64::from(*value)).sum::<f64>() / n;
    let variance = input
        .iter()
        .map(|value| {
            let centered = f64::from(*value) - mean;
            centered * centered
        })
        .sum::<f64>()
        / n;
    let inverse = 1.0 / (variance + f64::from(epsilon)).sqrt();
    input
        .iter()
        .zip(weight.iter().zip(bias))
        .map(|(value, (weight, bias))| {
            ((f64::from(*value) - mean) * inverse) as f32 * weight + bias
        })
        .collect()
}

/// Exact (erf) GELU, `torch.nn.GELU()`.
fn gelu(value: f32) -> f32 {
    let x = f64::from(value);
    (0.5 * x * (1.0 + libm::erf(x / std::f64::consts::SQRT_2))) as f32
}

fn add_into(target: &mut [f32], other: &[f32]) {
    for (target, other) in target.iter_mut().zip(other) {
        *target += other;
    }
}

fn l2_normalize(input: &[f32], epsilon: f32) -> Vec<f32> {
    let norm = input
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt() as f32;
    let scale = 1.0 / norm.max(epsilon);
    input.iter().map(|value| value * scale).collect()
}

fn softmax(values: &[f32]) -> Vec<f32> {
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f64> = values
        .iter()
        .map(|value| f64::from(value - max).exp())
        .collect();
    let total: f64 = exps.iter().sum();
    exps.iter().map(|value| (value / total) as f32).collect()
}

/// Multi-head attention of `n_query` rows against projected keys and values.
#[allow(clippy::too_many_arguments)]
fn attend(
    backend: &mut dyn MemoryAttention,
    attention: &Attention,
    heads: usize,
    queries: &[f32],
    n_query: usize,
    keys: &[f32],
    values: &[f32],
    n_key: usize,
    width: usize,
) -> Result<Vec<f32>, String> {
    let projected = backend.linear(&attention.q, queries, n_query, Some(&attention.q_bias))?;
    let head_dim = width / heads;
    let scale = 1.0 / (head_dim as f32).sqrt();
    let mut context = vec![0.0f32; n_query * width];
    context
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, out)| {
            let query = &projected[row * width..(row + 1) * width];
            let mut scores = vec![0.0f32; n_key];
            for head in 0..heads {
                let range = head * head_dim..(head + 1) * head_dim;
                for (key_index, score) in scores.iter_mut().enumerate() {
                    let key = &keys[key_index * width..(key_index + 1) * width];
                    *score = dot(&query[range.clone()], &key[range.clone()]) * scale;
                }
                let weights = softmax(&scores);
                let slot = &mut out[range.clone()];
                for (key_index, weight) in weights.iter().enumerate() {
                    let value = &values[key_index * width..(key_index + 1) * width];
                    for (target, value) in slot.iter_mut().zip(&value[range.clone()]) {
                        *target += weight * value;
                    }
                }
            }
        });
    backend.linear(&attention.out, &context, n_query, Some(&attention.out_bias))
}

/// Keys and values of one attention against `n` rows.
fn project_memory(
    backend: &mut dyn MemoryAttention,
    attention: &Attention,
    memory: &[f32],
    n: usize,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    Ok((
        backend.linear(&attention.k, memory, n, Some(&attention.k_bias))?,
        backend.linear(&attention.v, memory, n, Some(&attention.v_bias))?,
    ))
}

/// Which memory the attention reads: an evidence layer's `LN_m(M)` or the
/// raw memory rows (field layers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryView {
    Evidence(usize),
    Raw,
}

/// Softmax-weighted memory rows for query-side vectors.
pub trait MemoryAttention {
    /// For each of `rows` query vectors `u` (`rows x width`), returns
    /// `sum_t softmax_t(scale * u . m_t) m_t` over the rows `m_t` of `view`.
    fn attend(
        &mut self,
        view: MemoryView,
        queries: &[f32],
        rows: usize,
        scale: f32,
    ) -> Result<Vec<f32>, String>;

    /// The memory attention between the query and output projections of
    /// `attention`: for each of `n_query` projected queries, the attended
    /// context before `W_out`. The default runs the per-head products on
    /// the host ([`attend_projected_host`]); a device backend can keep them
    /// on the device.
    fn attend_projected(
        &mut self,
        attention: &Attention,
        heads: usize,
        projected: &[f32],
        n_query: usize,
        view: MemoryView,
        width: usize,
    ) -> Result<Vec<f32>, String> {
        attend_projected_host(self, attention, heads, projected, n_query, view, width)
    }

    /// `X W^T (+ b)` for `n` rows of `X`: every dense matrix product of the
    /// head goes through here, so a device backend can keep the head's
    /// weights resident. The default runs on the CPU.
    fn linear(
        &mut self,
        matrix: &Matrix,
        input: &[f32],
        n: usize,
        bias: Option<&[f32]>,
    ) -> Result<Vec<f32>, String> {
        Ok(matrix.apply_rows(input, n, bias))
    }
}

/// Memory rows held on the host.
pub struct HostMemory<'a> {
    weights: &'a ClefHeadWeights,
    memory: &'a [f32],
    normalized: BTreeMap<usize, Vec<f32>>,
}

impl<'a> HostMemory<'a> {
    #[must_use]
    pub fn new(weights: &'a ClefHeadWeights, memory: &'a [f32]) -> Self {
        Self {
            weights,
            memory,
            normalized: BTreeMap::new(),
        }
    }
}

impl MemoryAttention for HostMemory<'_> {
    fn attend(
        &mut self,
        view: MemoryView,
        queries: &[f32],
        rows: usize,
        scale: f32,
    ) -> Result<Vec<f32>, String> {
        let w = self.weights.config.width;
        let memory: &[f32] = match view {
            MemoryView::Raw => self.memory,
            MemoryView::Evidence(layer) => {
                let norm = &self
                    .weights
                    .evidence_layers
                    .get(layer)
                    .ok_or_else(|| format!("no evidence layer {layer}"))?
                    .memory_norm;
                let memory = self.memory;
                self.normalized
                    .entry(layer)
                    .or_insert_with(|| norm.apply_rows(memory, w))
                    .as_slice()
            }
        };
        let n = memory.len() / w;
        let mut out = vec![0.0f32; rows * w];
        out.par_chunks_mut(w)
            .zip(queries.par_chunks(w))
            .for_each(|(slot, query)| {
                let scores: Vec<f32> = memory
                    .chunks(w)
                    .map(|row| dot(query, row) * scale)
                    .collect();
                let weights = softmax(&scores);
                for (weight, row) in weights.iter().zip(memory.chunks(w)) {
                    for (target, value) in slot.iter_mut().zip(row) {
                        *target += weight * value;
                    }
                }
                debug_assert_eq!(weights.len(), n);
            });
        Ok(out)
    }
}

/// Multi-head attention of `n_query` (already normalized) rows against
/// memory, in the query-side form ([module docs](self)).
fn attend_memory(
    attention: &Attention,
    heads: usize,
    queries: &[f32],
    n_query: usize,
    memory: &mut dyn MemoryAttention,
    view: MemoryView,
    width: usize,
) -> Result<Vec<f32>, String> {
    let projected = memory.linear(&attention.q, queries, n_query, Some(&attention.q_bias))?;
    let context = memory.attend_projected(attention, heads, &projected, n_query, view, width)?;
    memory.linear(&attention.out, &context, n_query, Some(&attention.out_bias))
}

/// The memory attention between the query and output projections, on the
/// host: `u[i, h] = W_k,h^T q[i, h]`, the attention of each `u` over the
/// view's rows, then `W_v,h z[i, h] + b_v` ([`MemoryAttention::attend_projected`]).
pub fn attend_projected_host<M: MemoryAttention + ?Sized>(
    memory: &mut M,
    attention: &Attention,
    heads: usize,
    projected: &[f32],
    n_query: usize,
    view: MemoryView,
    width: usize,
) -> Result<Vec<f32>, String> {
    let head_dim = width / heads;
    // u[i, h] = W_k,h^T q[i, h]   (rows of W_k for head h are its outputs)
    let mut side = vec![0.0f32; n_query * heads * width];
    side.par_chunks_mut(width)
        .enumerate()
        .for_each(|(index, slot)| {
            let (row, head) = (index / heads, index % heads);
            let query = &projected[row * width..(row + 1) * width];
            for d in head * head_dim..(head + 1) * head_dim {
                let coefficient = query[d];
                for (target, weight) in slot.iter_mut().zip(attention.k.row(d)) {
                    *target += coefficient * weight;
                }
            }
        });
    let scale = 1.0 / (head_dim as f32).sqrt();
    let mixed = memory.attend(view, &side, n_query * heads, scale)?;
    // context[i, h*hd + e] = W_v[h*hd + e] . z[i, h] + b_v
    let mut context = vec![0.0f32; n_query * width];
    context
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, out)| {
            for head in 0..heads {
                let z = &mixed[(row * heads + head) * width..(row * heads + head + 1) * width];
                for e in head * head_dim..(head + 1) * head_dim {
                    out[e] = dot(attention.v.row(e), z) + attention.v_bias[e];
                }
            }
        });
    Ok(context)
}

fn feedforward(
    backend: &mut dyn MemoryAttention,
    up: &Matrix,
    up_bias: &[f32],
    down: &Matrix,
    down_bias: &[f32],
    input: &[f32],
    n: usize,
) -> Result<Vec<f32>, String> {
    let mut hidden = backend.linear(up, input, n, Some(up_bias))?;
    hidden
        .par_iter_mut()
        .for_each(|value| *value = gelu(*value));
    backend.linear(down, &hidden, n, Some(down_bias))
}

/// Streamed head input for one encoded record ([module docs](self)).
pub struct ClefHeadStream<'a> {
    weights: &'a ClefHeadWeights,
    length: usize,
    received: usize,
    /// `[length, width]`: `LN(H) W_mem`.
    memory: Vec<f32>,
    /// Which span sum each position feeds, if any.
    span_of: Vec<Option<usize>>,
    /// `(start, end)` and the running sum of `LN(H)` for each span; spans
    /// are questions first, then each question's options in order.
    spans: Vec<((usize, usize), Vec<f32>)>,
    last: Vec<f32>,
}

impl<'a> ClefHeadStream<'a> {
    /// Prepares to receive `record.input_ids.len()` hidden rows.
    #[must_use]
    pub fn new(weights: &'a ClefHeadWeights, record: &EncodedRecord) -> Self {
        let length = record.input_ids.len();
        let d = weights.config.hidden_size;
        let mut spans = Vec::new();
        for question in &record.questions {
            spans.push((question.question_span, vec![0.0f32; d]));
        }
        for question in &record.questions {
            for span in &question.option_spans {
                spans.push((*span, vec![0.0f32; d]));
            }
        }
        let mut span_of = vec![None; length];
        for (index, ((start, end), _)) in spans.iter().enumerate() {
            for slot in span_of.iter_mut().take(*end).skip(*start) {
                *slot = Some(index);
            }
        }
        Self {
            weights,
            length,
            received: 0,
            memory: vec![0.0f32; length * weights.config.width],
            span_of,
            spans,
            last: Vec::new(),
        }
    }

    /// Takes the final hidden row of position `index` (rows arrive in order).
    pub fn push(&mut self, index: usize, hidden: &[f32]) -> Result<(), String> {
        if index != self.received || index >= self.length {
            return Err(format!(
                "hidden row {index} arrived out of order (expected {})",
                self.received
            ));
        }
        if hidden.len() != self.weights.config.hidden_size {
            return Err(format!(
                "hidden row width {} does not match the head's {}",
                hidden.len(),
                self.weights.config.hidden_size
            ));
        }
        let normalized = self.weights.hidden_norm.apply(hidden);
        let width = self.weights.config.width;
        let row = self.weights.memory_projection.apply(&normalized, None);
        self.memory[index * width..(index + 1) * width].copy_from_slice(&row);
        if let Some(span) = self.span_of[index] {
            add_into(&mut self.spans[span].1, &normalized);
        }
        if index + 1 == self.length {
            self.last = normalized;
        }
        self.received += 1;
        Ok(())
    }

    /// The head's pooled inputs so far: every span mean (questions, then
    /// options in prompt order) and the normalized last row. Complete once
    /// every row has been pushed.
    #[must_use]
    pub fn pooled(&self) -> (Vec<Vec<f32>>, Vec<f32>) {
        (
            (0..self.spans.len()).map(|index| self.span_mean(index)).collect(),
            self.last.clone(),
        )
    }

    fn span_mean(&self, index: usize) -> Vec<f32> {
        let ((start, end), sum) = &self.spans[index];
        let count = end.saturating_sub(*start).max(1) as f32;
        sum.iter().map(|value| value / count).collect()
    }

    /// Runs the head. `lexical_rows` returns the untied LM-head rows of the
    /// given token ids. Returns one logit per option of every question, in
    /// prompt option order.
    pub fn finish(
        self,
        record: &EncodedRecord,
        lexical_rows: &dyn Fn(&[u32]) -> Result<Vec<Vec<f32>>, String>,
    ) -> Result<Vec<Vec<f32>>, String> {
        if self.received != self.length {
            return Err(format!(
                "the head received {} of {} hidden rows",
                self.received, self.length
            ));
        }
        let span_means: Vec<Vec<f32>> = (0..self.spans.len())
            .map(|index| self.span_mean(index))
            .collect();
        let mut memory = HostMemory::new(self.weights, &self.memory);
        run_head(
            self.weights,
            record,
            &span_means,
            &self.last,
            &mut memory,
            lexical_rows,
        )
    }
}

/// The head spans of a record, in the order [`run_head`] reads their means:
/// every question span, then each question's option spans in order.
#[must_use]
pub fn head_spans(record: &EncodedRecord) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = record.questions.iter().map(|q| q.question_span).collect();
    for question in &record.questions {
        spans.extend(question.option_spans.iter().copied());
    }
    spans
}

/// The head after the backbone: `span_means` are the means of `LN(H)` over
/// [`head_spans`], `global` is `LN(H)` of the last token, and `memory`
/// attends over `LN(H) W_mem`.
pub fn run_head(
    weights: &ClefHeadWeights,
    record: &EncodedRecord,
    span_means: &[Vec<f32>],
    global: &[f32],
    memory: &mut dyn MemoryAttention,
    lexical_rows: &dyn Fn(&[u32]) -> Result<Vec<Vec<f32>>, String>,
) -> Result<Vec<Vec<f32>>, String> {
    let config = weights.config;
    let (d, w) = (config.hidden_size, config.width);
    let question_count = record.questions.len();
    let question_vectors: Vec<Vec<f32>> = span_means[..question_count].to_vec();
    let global = global.to_vec();

    // Option context and lexical vectors.
    let mut option_context = Vec::new();
    let mut lexical = Vec::new();
    let mut owner = Vec::new();
    let mut span_index = question_count;
    for (question_index, question) in record.questions.iter().enumerate() {
        for (start, end) in &question.option_spans {
            option_context.push(span_means[span_index].clone());
            span_index += 1;
            let ids = &record.input_ids[*start..*end];
            let rows = lexical_rows(ids)?;
            let mut mean = vec![0.0f32; d];
            for row in &rows {
                if row.len() != d {
                    return Err(format!(
                        "lexical row width {} does not match hidden size {d}",
                        row.len()
                    ));
                }
                add_into(&mut mean, row);
            }
            let count = rows.len().max(1) as f32;
            mean.iter_mut().for_each(|value| *value /= count);
            lexical.push(mean);
            owner.push(question_index);
        }
    }
    let n_options = owner.len();
    let flat = |rows: &[Vec<f32>]| rows.concat();

    // R = W_oc c + W_ol l + W_oq q
    let option_question = memory.linear(
        &weights.option_question_projection,
        &flat(&question_vectors),
        question_count,
        None,
    )?;
    let context = memory.linear(
        &weights.option_context_projection,
        &flat(&option_context),
        n_options,
        None,
    )?;
    let lexical_part = memory.linear(
        &weights.option_lexical_projection,
        &flat(&lexical),
        n_options,
        None,
    )?;
    let mut routed = vec![0.0f32; n_options * w];
    for (index, row) in routed.chunks_mut(w).enumerate() {
        let question = &option_question[owner[index] * w..(owner[index] + 1) * w];
        for column in 0..w {
            row[column] = context[index * w + column]
                + lexical_part[index * w + column]
                + question[column];
        }
    }

    for (layer_index, layer) in weights.evidence_layers.iter().enumerate() {
        let queries = layer.query_norm.apply_rows(&routed, w);
        let attended = attend_memory(
            &layer.attention,
            config.heads,
            &queries,
            n_options,
            memory,
            MemoryView::Evidence(layer_index),
            w,
        )?;
        add_into(&mut routed, &attended);
        let normalized = layer.feedforward_norm.apply_rows(&routed, w);
        let ff = feedforward(
            memory,
            &layer.up,
            &layer.up_bias,
            &layer.down,
            &layer.down_bias,
            &normalized,
            n_options,
        )?;
        add_into(&mut routed, &ff);
    }

    // Fields.
    let base_fields = memory.linear(
        &weights.question_projection,
        &flat(&question_vectors),
        question_count,
        None,
    )?;
    let global_projected = memory.linear(&weights.global_projection, &global, 1, None)?;
    let mut fields = vec![0.0f32; question_count * w];
    let mut first_option = 0;
    let mut option_ranges = Vec::with_capacity(question_count);
    for (question_index, question) in record.questions.iter().enumerate() {
        let count = question.option_spans.len();
        option_ranges.push(first_option..first_option + count);
        let field = &base_fields[question_index * w..(question_index + 1) * w];
        let scores: Vec<f32> = (first_option..first_option + count)
            .map(|option| dot(&routed[option * w..(option + 1) * w], field) / (w as f32).sqrt())
            .collect();
        let routing = softmax(&scores);
        let mut summary = vec![0.0f32; w];
        for (offset, weight) in routing.iter().enumerate() {
            let option = first_option + offset;
            for (target, value) in summary
                .iter_mut()
                .zip(&routed[option * w..(option + 1) * w])
            {
                *target += weight * value;
            }
        }
        let summary = weights.option_summary_norm.apply(&summary);
        let type_row = weights.type_embedding.row(question.question_type);
        let slot = &mut fields[question_index * w..(question_index + 1) * w];
        for column in 0..w {
            slot[column] =
                field[column] + summary[column] + global_projected[column] + type_row[column];
        }
        first_option += count;
    }

    for layer in &weights.layers {
        let normalized = layer.self_norm.apply_rows(&fields, w);
        let (keys, values) =
            project_memory(memory, &layer.self_attention, &normalized, question_count)?;
        let attended = attend(
            memory,
            &layer.self_attention,
            config.heads,
            &normalized,
            question_count,
            &keys,
            &values,
            question_count,
            w,
        )?;
        add_into(&mut fields, &attended);
        // Memory is not normalized in the field layers.
        let normalized = layer.cross_norm.apply_rows(&fields, w);
        let attended = attend_memory(
            &layer.cross_attention,
            config.heads,
            &normalized,
            question_count,
            memory,
            MemoryView::Raw,
            w,
        )?;
        add_into(&mut fields, &attended);
        let normalized = layer.feedforward_norm.apply_rows(&fields, w);
        let ff = feedforward(
            memory,
            &layer.up,
            &layer.up_bias,
            &layer.down,
            &layer.down_bias,
            &normalized,
            question_count,
        )?;
        add_into(&mut fields, &ff);
    }
    let fields = weights.field_norm.apply_rows(&fields, w);

    // Scores: the residual scorer runs over every option of every question
    // in one product.
    let options_normed = weights.option_norm.apply_rows(&routed, w);
    let mut features = Vec::with_capacity(n_options * 4 * w);
    for (option, owner_question) in owner.iter().enumerate() {
        let field = &fields[owner_question * w..(owner_question + 1) * w];
        let options = &options_normed[option * w..(option + 1) * w];
        features.extend_from_slice(field);
        features.extend_from_slice(options);
        features.extend(field.iter().zip(options).map(|(f, o)| f * o));
        features.extend(field.iter().zip(options).map(|(f, o)| (f - o).abs()));
    }
    let hidden = memory.linear(&weights.scorer, &features, n_options, Some(&weights.scorer_bias))?;
    let mut logits = Vec::with_capacity(question_count);
    for (question_index, range) in option_ranges.into_iter().enumerate() {
        let field = &fields[question_index * w..(question_index + 1) * w];
        let mut anchor = question_vectors[question_index].clone();
        add_into(&mut anchor, &global);
        let anchor = l2_normalize(&anchor, 1e-12);
        let field_unit = l2_normalize(field, 1e-8);
        let question_logits: Vec<f32> = range
            .map(|option| {
                let lexical_unit = l2_normalize(&lexical[option], 1e-12);
                let prior = weights.prior_scale * dot(&lexical_unit, &anchor);
                let options = &options_normed[option * w..(option + 1) * w];
                let cosine = dot(&field_unit, &l2_normalize(options, 1e-8));
                let residual = hidden[option * w..(option + 1) * w]
                    .iter()
                    .zip(&weights.scorer_out)
                    .map(|(value, weight)| gelu(*value) * weight)
                    .sum::<f32>()
                    + weights.scorer_out_bias;
                let joint = weights.joint_scale * cosine + residual;
                prior + weights.gate * joint
            })
            .collect();
        logits.push(question_logits);
    }
    Ok(logits)
}

/// `softmax` over one question's logits.
#[must_use]
pub fn probabilities(logits: &[f32]) -> Vec<f64> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f64> = logits
        .iter()
        .map(|value| f64::from(value - max).exp())
        .collect();
    let total: f64 = exps.iter().sum();
    exps.iter().map(|value| value / total).collect()
}
