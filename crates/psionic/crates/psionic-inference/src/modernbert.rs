//! Native float32 ModernBERT execution, independent of ONNX Runtime.

use std::{collections::BTreeMap, path::Path};

use rayon::{ThreadPool, ThreadPoolBuilder, prelude::*};
use safetensors::{Dtype, SafeTensors};
use serde::{Deserialize, Serialize};

use crate::{
    EngineKind, Error, ExecutionIdentity, ModelEngine, Result, Tensor, TensorData, Tensors,
    router::file_sha256,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LayerConfig {
    pub window: Option<usize>,
    pub inverse_frequencies: Vec<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModernBertConfig {
    pub schema: String,
    pub hidden: usize,
    pub heads: usize,
    pub intermediate: usize,
    pub epsilon: f32,
    pub embedding_scale: f32,
    pub embedding_zero: u8,
    pub layers: Vec<LayerConfig>,
    pub source_sha256: String,
    pub weights_sha256: String,
}

struct Matrix {
    rows: usize,
    cols: usize,
    values: Vec<f32>,
}

struct Layer {
    config: LayerConfig,
    attn_norm: Option<Vec<f32>>,
    mlp_norm: Vec<f32>,
    qkv: Matrix,
    attn_out: Matrix,
    up: Matrix,
    down: Matrix,
}

struct Head {
    weight: Matrix,
    bias: Vec<f32>,
}

pub struct ModernBertEngine {
    config: ModernBertConfig,
    embedding: Vec<u8>,
    embedding_norm: Vec<f32>,
    final_norm: Vec<f32>,
    layers: Vec<Layer>,
    heads: BTreeMap<String, Head>,
    pool: ThreadPool,
    identity: ExecutionIdentity,
}

fn tensor_error(error: impl std::fmt::Display) -> Error {
    Error(format!("Cannot load native model: {error}"))
}

fn floats(tensors: &SafeTensors<'_>, name: &str, shape: &[usize]) -> Result<Vec<f32>> {
    let t = tensors.tensor(name).map_err(tensor_error)?;
    if t.dtype() != Dtype::F32 || t.shape() != shape {
        return Err(Error(format!(
            "Unexpected native weight type or dimensions: {name}"
        )));
    }
    let values: Vec<f32> = t
        .data()
        .chunks_exact(4)
        .map(|v| f32::from_le_bytes([v[0], v[1], v[2], v[3]]))
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err(Error(format!("Non-finite native weight: {name}")));
    }
    Ok(values)
}

fn matrix(tensors: &SafeTensors<'_>, name: &str, rows: usize, cols: usize) -> Result<Matrix> {
    Ok(Matrix {
        rows,
        cols,
        values: floats(tensors, name, &[rows, cols])?,
    })
}

impl ModernBertEngine {
    pub fn load(root: &Path, threads: usize) -> Result<Self> {
        if !(1..=256).contains(&threads) {
            return Err(Error("Choose between 1 and 256 CPU threads".into()));
        }
        let config: ModernBertConfig =
            serde_json::from_slice(&std::fs::read(root.join("native.json"))?)?;
        let h = config.hidden;
        let f = config.intermediate;
        if config.schema != "psionic.modernbert.v1"
            || h == 0
            || h > 4096
            || config.heads == 0
            || h % config.heads != 0
            || (h / config.heads) % 2 != 0
            || f == 0
            || f > 16384
            || config.layers.is_empty()
            || config.layers.len() > 64
            || !config.epsilon.is_finite()
            || config.epsilon <= 0.0
            || !config.embedding_scale.is_finite()
            || config.embedding_scale <= 0.0
        {
            return Err(Error("Invalid native encoder configuration".into()));
        }
        for layer in &config.layers {
            if layer.inverse_frequencies.len() != h / config.heads / 2
                || layer
                    .inverse_frequencies
                    .iter()
                    .any(|v| !v.is_finite() || *v <= 0.0)
            {
                return Err(Error("Invalid native rotary frequencies".into()));
            }
        }
        if file_sha256(&root.join("model.gq.onnx"))? != config.source_sha256
            || file_sha256(&root.join("native.safetensors"))? != config.weights_sha256
        {
            return Err(Error(
                "Native model files do not match their expected checksums".into(),
            ));
        }
        let bytes = std::fs::read(root.join("native.safetensors"))?;
        let tensors = SafeTensors::deserialize(&bytes).map_err(tensor_error)?;
        let embedding = tensors.tensor("embedding").map_err(tensor_error)?;
        if embedding.dtype() != Dtype::U8
            || embedding.shape().len() != 2
            || embedding.shape()[1] != h
            || embedding.shape()[0] == 0
        {
            return Err(Error("Invalid native embedding table".into()));
        }
        let embedding = embedding.data().to_vec();
        let mut layers = Vec::new();
        for (i, layer_config) in config.layers.iter().enumerate() {
            let prefix = format!("layers.{i}");
            layers.push(Layer {
                config: layer_config.clone(),
                attn_norm: if i == 0 {
                    None
                } else {
                    Some(floats(&tensors, &format!("{prefix}.attn_norm"), &[h])?)
                },
                mlp_norm: floats(&tensors, &format!("{prefix}.mlp_norm"), &[h])?,
                qkv: matrix(&tensors, &format!("{prefix}.qkv"), h, 3 * h)?,
                attn_out: matrix(&tensors, &format!("{prefix}.attn_out"), h, h)?,
                up: matrix(&tensors, &format!("{prefix}.up"), h, 2 * f)?,
                down: matrix(&tensors, &format!("{prefix}.down"), f, h)?,
            });
        }
        let mut heads = BTreeMap::new();
        for (name, width) in [
            ("category", 9),
            ("difficulty", 5),
            ("stakes", 4),
            ("flags", 4),
        ] {
            heads.insert(
                name.into(),
                Head {
                    weight: matrix(&tensors, &format!("heads.{name}.weight"), h, width)?,
                    bias: floats(&tensors, &format!("heads.{name}.bias"), &[width])?,
                },
            );
        }
        Ok(Self {
            embedding,
            embedding_norm: floats(&tensors, "embedding_norm", &[h])?,
            final_norm: floats(&tensors, "final_norm", &[h])?,
            layers,
            heads,
            pool: ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .map_err(tensor_error)?,
            identity: ExecutionIdentity {
                engine: EngineKind::Native,
                provider: "psionic-cpu".into(),
                device: "cpu".into(),
                precision: "uint8-embedding/f32".into(),
                threads,
                runtime_version: env!("CARGO_PKG_VERSION").into(),
                runtime_sha256: None,
                cpu_fallback: Some(false),
            },
            config,
        })
    }

    fn forward(&self, inputs: &Tensors) -> Result<Tensors> {
        if inputs.len() != 2 {
            return Err(Error(
                "The encoder requires input_ids and attention_mask".into(),
            ));
        }
        let ids = inputs
            .get("input_ids")
            .ok_or_else(|| Error("Missing input_ids".into()))?;
        let mask = inputs
            .get("attention_mask")
            .ok_or_else(|| Error("Missing attention_mask".into()))?;
        if ids.shape.len() != 2
            || ids.shape[0] != 1
            || !(1..=512).contains(&ids.shape[1])
            || mask.shape != ids.shape
        {
            return Err(Error(
                "The native encoder expects one sequence of 1 to 512 tokens".into(),
            ));
        }
        let ids = ids.integers()?;
        let mask = mask.integers()?;
        if mask.iter().any(|v| ![0, 1].contains(v)) {
            return Err(Error("Attention masks must contain zero or one".into()));
        }
        let h = self.config.hidden;
        let n = ids.len();
        let mut hidden = Vec::with_capacity(n * h);
        for id in ids {
            let id =
                usize::try_from(*id).map_err(|_| Error("Token ID must be non-negative".into()))?;
            if id >= self.embedding.len() / h {
                return Err(Error("Token ID is outside the model vocabulary".into()));
            }
            hidden.extend(self.embedding[id * h..(id + 1) * h].iter().map(|v| {
                (f32::from(*v) - f32::from(self.config.embedding_zero))
                    * self.config.embedding_scale
            }));
        }
        normalize(&mut hidden, &self.embedding_norm, self.config.epsilon);
        for layer in &self.layers {
            let mut normalized = hidden.clone();
            if let Some(weight) = &layer.attn_norm {
                normalize(&mut normalized, weight, self.config.epsilon);
            }
            let qkv = linear(&normalized, &layer.qkv);
            let attention = attention(&qkv, n, h, self.config.heads, mask, &layer.config);
            let output = linear(&attention, &layer.attn_out);
            add(&mut hidden, &output);
            let mut normalized = hidden.clone();
            normalize(&mut normalized, &layer.mlp_norm, self.config.epsilon);
            let up = linear(&normalized, &layer.up);
            let f = self.config.intermediate;
            let mut gated = vec![0.0; n * f];
            gated
                .par_chunks_mut(f)
                .zip(up.par_chunks(2 * f))
                .for_each(|(out, input)| {
                    for i in 0..f {
                        out[i] = (input[i]
                            * (1.0 + libm::erff(input[i] / std::f32::consts::SQRT_2))
                            * 0.5)
                            * input[f + i];
                    }
                });
            add(&mut hidden, &linear(&gated, &layer.down));
        }
        let mut pooled = hidden[..h].to_vec();
        normalize(&mut pooled, &self.final_norm, self.config.epsilon);
        let mut outputs = Tensors::new();
        for (name, head) in &self.heads {
            let mut values = linear(&pooled, &head.weight);
            add(&mut values, &head.bias);
            outputs.insert(
                name.clone(),
                Tensor {
                    shape: vec![1, values.len()],
                    data: TensorData::F32(values),
                },
            );
        }
        Ok(outputs)
    }
}

impl ModelEngine for ModernBertEngine {
    fn identity(&self) -> &ExecutionIdentity {
        &self.identity
    }
    fn run(&mut self, inputs: &Tensors) -> Result<Tensors> {
        self.pool.install(|| self.forward(inputs))
    }
}

fn add(a: &mut [f32], b: &[f32]) {
    for (a, b) in a.iter_mut().zip(b) {
        *a += b;
    }
}

fn normalize(values: &mut [f32], weight: &[f32], epsilon: f32) {
    values.par_chunks_mut(weight.len()).for_each(|row| {
        let mean = row.iter().sum::<f32>() / row.len() as f32;
        let variance =
            row.iter().map(|v| (*v - mean) * (*v - mean)).sum::<f32>() / row.len() as f32;
        let scale = 1.0 / (variance + epsilon).sqrt();
        for (value, weight) in row.iter_mut().zip(weight) {
            *value = (*value - mean) * scale * weight;
        }
    });
}

fn linear(input: &[f32], weight: &Matrix) -> Vec<f32> {
    let rows = input.len() / weight.rows;
    let mut output = vec![0.0; rows * weight.cols];
    let chunk = rows.div_ceil(rayon::current_num_threads()).max(1);
    output
        .par_chunks_mut(chunk * weight.cols)
        .enumerate()
        .for_each(|(i, out)| {
            let a = &input[i * chunk * weight.rows..];
            gemm(
                out.len() / weight.cols,
                weight.cols,
                weight.rows,
                a,
                weight.rows,
                1,
                &weight.values,
                weight.cols,
                1,
                out,
            );
        });
    output
}

#[allow(clippy::too_many_arguments)]
fn gemm(
    m: usize,
    n: usize,
    k: usize,
    a: &[f32],
    ars: usize,
    acs: usize,
    b: &[f32],
    brs: usize,
    bcs: usize,
    out: &mut [f32],
) {
    debug_assert!(m > 0 && n > 0 && k > 0);
    debug_assert!(a.len() > (m - 1) * ars + (k - 1) * acs);
    debug_assert!(b.len() > (k - 1) * brs + (n - 1) * bcs);
    debug_assert_eq!(out.len(), m * n);
    // SAFETY: all dimensions and strides describe the checked slices above;
    // output is a distinct contiguous m-by-n allocation.
    unsafe {
        matrixmultiply::sgemm(
            m,
            k,
            n,
            1.0,
            a.as_ptr(),
            ars as isize,
            acs as isize,
            b.as_ptr(),
            brs as isize,
            bcs as isize,
            0.0,
            out.as_mut_ptr(),
            n as isize,
            1,
        );
    }
}

fn attention(
    qkv: &[f32],
    n: usize,
    h: usize,
    heads: usize,
    mask: &[i64],
    config: &LayerConfig,
) -> Vec<f32> {
    let dim = h / heads;
    let mut per_head = vec![0.0; heads * n * dim];
    per_head
        .par_chunks_mut(n * dim)
        .enumerate()
        .for_each(|(head, out)| {
            let mut q = vec![0.0; n * dim];
            let mut k = vec![0.0; n * dim];
            let mut v = vec![0.0; n * dim];
            // The ONNX graph scales Q and K independently before their product.
            let scale = (1.0 / (dim as f32).sqrt()).sqrt();
            for t in 0..n {
                for d in 0..dim {
                    q[t * dim + d] = qkv[t * 3 * h + head * dim + d];
                    k[t * dim + d] = qkv[t * 3 * h + h + head * dim + d];
                    v[t * dim + d] = qkv[t * 3 * h + 2 * h + head * dim + d];
                }
                for d in 0..dim / 2 {
                    let angle = t as f32 * config.inverse_frequencies[d];
                    let (sin, cos) = angle.sin_cos();
                    for values in [&mut q, &mut k] {
                        let a = values[t * dim + d];
                        let b = values[t * dim + dim / 2 + d];
                        values[t * dim + d] = (a * cos - b * sin) * scale;
                        values[t * dim + dim / 2 + d] = (b * cos + a * sin) * scale;
                    }
                }
            }
            let mut scores = vec![0.0; n * n];
            gemm(n, n, dim, &q, dim, 1, &k, 1, dim, &mut scores);
            for (t, row) in scores.chunks_mut(n).enumerate() {
                let mut max = f32::NEG_INFINITY;
                for (s, value) in row.iter_mut().enumerate() {
                    if mask[s] == 0 || config.window.is_some_and(|w| t.abs_diff(s) > w) {
                        *value = f32::NEG_INFINITY;
                    }
                    max = max.max(*value);
                }
                if max == f32::NEG_INFINITY {
                    row.fill(0.0);
                    continue;
                }
                let mut sum = 0.0;
                for value in row.iter_mut() {
                    *value = (*value - max).exp();
                    sum += *value;
                }
                for value in row {
                    *value /= sum;
                }
            }
            gemm(n, dim, n, &scores, n, 1, &v, dim, 1, out);
        });
    let mut output = vec![0.0; n * h];
    for t in 0..n {
        for head in 0..heads {
            output[t * h + head * dim..t * h + (head + 1) * dim].copy_from_slice(
                &per_head[head * n * dim + t * dim..head * n * dim + (t + 1) * dim],
            );
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_attention_respects_padding_and_window_edges() {
        // Zero Q/K gives uniform attention over permitted keys.
        let mut qkv = vec![0.0; 4 * 3 * 2];
        for t in 0..4 {
            qkv[t * 6 + 4] = t as f32;
            qkv[t * 6 + 5] = 10.0 * t as f32;
        }
        let cfg = LayerConfig {
            window: Some(1),
            inverse_frequencies: vec![1.0],
        };
        let actual = attention(&qkv, 4, 2, 1, &[1, 1, 0, 1], &cfg);
        assert_eq!(actual, vec![0.5, 5.0, 0.5, 5.0, 2.0, 20.0, 3.0, 30.0]);
        assert_eq!(attention(&qkv, 4, 2, 1, &[0, 0, 0, 0], &cfg), vec![0.0; 8]);
    }

    #[test]
    fn normalization_subtracts_mean_and_uses_population_variance() {
        let mut values = vec![1.0, 3.0];
        normalize(&mut values, &[2.0, 3.0], 1e-5);
        assert!((values[0] + 2.0).abs() < 0.0001);
        assert!((values[1] - 3.0).abs() < 0.0001);
    }
}
