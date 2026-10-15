//! Native float32 ModernBERT execution, independent of ONNX Runtime.

use std::{collections::BTreeMap, path::Path};

use rayon::{ThreadPool, prelude::*};
use safetensors::{Dtype, SafeTensors};
use serde::{Deserialize, Serialize};

use crate::buffers::Buffers;
use crate::{
    EngineKind, Error, ExecutionIdentity, ModelEngine, Result, Tensor, TensorData, Tensors,
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

#[cfg(target_arch = "x86_64")]
mod simd;

struct Matrix {
    #[cfg(target_arch = "x86_64")]
    packed: Option<simd::Packed>,
    rows: usize,
    cols: usize,
    values: Vec<f32>,
}

struct Layer {
    config: LayerConfig,
    rotary: std::sync::Arc<Vec<f32>>,
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

#[cfg(feature = "native-cuda")]
mod cuda;
#[cfg(feature = "native-metal")]
mod metal;

pub struct ModernBertEngine {
    buffers: Buffers,
    #[cfg(feature = "native-cuda")]
    cuda: Option<cuda::Encoder>,
    #[cfg(feature = "native-metal")]
    metal: Option<metal::Encoder>,
    accelerator: Option<std::sync::Mutex<crate::dense_accelerator::DenseAccelerator>>,
    config: ModernBertConfig,
    embedding: Vec<u8>,
    vocabulary: usize,
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
        #[cfg(target_arch = "x86_64")]
        packed: None,
        rows,
        cols,
        values: floats(tensors, name, &[rows, cols])?,
    })
}

impl ModernBertEngine {
    pub fn load(root: &Path, threads: usize) -> Result<Self> {
        Self::load_inner(root, threads, true)
    }

    fn load_inner(root: &Path, threads: usize, pack: bool) -> Result<Self> {
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
        // The pinned conversion receipt binds source identity and every converted value.
        // Serving the native package does not need the original ONNX weights as well.
        crate::router::verify_native_router(root)?;
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
        let mut rotary_tables: BTreeMap<Vec<u32>, std::sync::Arc<Vec<f32>>> = BTreeMap::new();
        for (i, layer_config) in config.layers.iter().enumerate() {
            let prefix = format!("layers.{i}");
            layers.push(Layer {
                config: layer_config.clone(),
                rotary: rotary_tables
                    .entry(
                        layer_config
                            .inverse_frequencies
                            .iter()
                            .map(|f| f.to_bits())
                            .collect(),
                    )
                    .or_insert_with(|| std::sync::Arc::new(rotary_table(layer_config, 512)))
                    .clone(),
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
        #[allow(unused_mut)]
        let mut model = Self {
            buffers: Buffers::default(),
            #[cfg(feature = "native-cuda")]
            cuda: None,
            #[cfg(feature = "native-metal")]
            metal: None,
            accelerator: None,
            vocabulary: embedding.len() / h,
            embedding,
            embedding_norm: floats(&tensors, "embedding_norm", &[h])?,
            final_norm: floats(&tensors, "final_norm", &[h])?,
            layers,
            heads,
            pool: crate::cpu::pool(threads)?,
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
        };
        #[cfg(target_arch = "x86_64")]
        if pack && simd::available() {
            for layer in &mut model.layers {
                for weight in [
                    &mut layer.qkv,
                    &mut layer.attn_out,
                    &mut layer.up,
                    &mut layer.down,
                ] {
                    weight.packed =
                        Some(simd::Packed::new(&weight.values, weight.rows, weight.cols));
                    weight.values = Vec::new();
                }
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        let _ = pack;
        Ok(model)
    }

    pub fn load_on(root: &Path, threads: usize, device: crate::DeviceChoice) -> Result<Self> {
        let mut model = Self::load_inner(root, threads, device == crate::DeviceChoice::Cpu)?;
        #[cfg(feature = "native-cuda")]
        if device == crate::DeviceChoice::Cuda {
            let encoder = cuda::Encoder::load(&mut model)?;
            model.identity.provider = "psionic-cuda-encoder".into();
            model.identity.device = encoder.device.clone();
            model.identity.cpu_fallback = Some(false);
            model.cuda = Some(encoder);
            return Ok(model);
        }
        #[cfg(feature = "native-metal")]
        if device == crate::DeviceChoice::Metal {
            let gpu = metal::Encoder::load(&mut model)?;
            model.identity.provider = "psionic-metal-encoder".into();
            model.identity.device = gpu.device.clone();
            model.identity.cpu_fallback = Some(false);
            model.metal = Some(gpu);
            return Ok(model);
        }
        if device != crate::DeviceChoice::Cpu {
            let weights: Vec<_> = model
                .layers
                .iter()
                .flat_map(|l| [&l.qkv, &l.attn_out, &l.up, &l.down])
                .chain(model.heads.values().map(|h| &h.weight))
                .map(|w| {
                    (
                        w.values.as_ptr() as usize,
                        w.values.as_slice(),
                        w.rows,
                        w.cols,
                    )
                })
                .collect();
            let accelerator = crate::dense_accelerator::DenseAccelerator::load(device, &weights)?;
            model.identity.provider = accelerator.provider.clone();
            model.identity.device = accelerator.device.clone();
            model.identity.cpu_fallback = Some(true);
            model.accelerator = Some(std::sync::Mutex::new(accelerator));
        }
        Ok(model)
    }

    fn linear(&self, input: &[f32], weight: &Matrix) -> Result<Vec<f32>> {
        if let Some(accelerator) = &self.accelerator {
            accelerator
                .lock()
                .map_err(|_| Error("Cannot access native accelerator".into()))?
                .matmul(
                    weight.values.as_ptr() as usize,
                    input,
                    input.len() / weight.rows,
                    weight.rows,
                    weight.cols,
                )
        } else {
            Ok(linear(input, weight, &self.buffers))
        }
    }

    fn inputs<'a>(&self, inputs: &'a Tensors) -> Result<(&'a [i64], &'a [i64])> {
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
        if ids
            .iter()
            .any(|id| usize::try_from(*id).map_or(true, |id| id >= self.vocabulary))
        {
            return Err(Error("Token ID is outside the model vocabulary".into()));
        }
        Ok((ids, mask))
    }

    fn forward(&self, inputs: &Tensors) -> Result<Tensors> {
        let (ids, mask) = self.inputs(inputs)?;
        let h = self.config.hidden;
        let n = ids.len();
        let mut hidden = self.buffers.take(n * h);
        hidden.clear();
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
        for (index, layer) in self.layers.iter().enumerate() {
            let rows = if index + 1 == self.layers.len() { 1 } else { n };
            let mut normalized = self.buffers.take(hidden.len());
            normalized.copy_from_slice(&hidden);
            if let Some(weight) = &layer.attn_norm {
                normalize(&mut normalized, weight, self.config.epsilon);
            }
            let qkv = self.linear(&normalized, &layer.qkv)?;
            self.buffers.give(normalized);
            let attention = attention_prepared(
                &qkv,
                n,
                h,
                self.config.heads,
                mask,
                &layer.config,
                &layer.rotary,
                rows,
                &self.buffers,
            );
            self.buffers.give(qkv);
            let output = self.linear(&attention, &layer.attn_out)?;
            self.buffers.give(attention);
            add(&mut hidden, &output);
            self.buffers.give(output);
            let mut normalized = self.buffers.take(rows * h);
            normalized.copy_from_slice(&hidden[..rows * h]);
            normalize(&mut normalized, &layer.mlp_norm, self.config.epsilon);
            let up = self.linear(&normalized, &layer.up)?;
            self.buffers.give(normalized);
            let f = self.config.intermediate;
            let mut gated = self.buffers.take(rows * f);
            gated
                .par_chunks_mut(f)
                .zip(up.par_chunks(2 * f))
                .for_each(|(out, input)| {
                    #[cfg(target_arch = "x86_64")]
                    if simd::available() {
                        // SAFETY: runtime feature checks and the row slices provide the required extents.
                        unsafe {
                            simd::geglu_row(input, out);
                        }
                        return;
                    }
                    for i in 0..f {
                        out[i] = (input[i]
                            * (1.0 + libm::erff(input[i] / std::f32::consts::SQRT_2))
                            * 0.5)
                            * input[f + i];
                    }
                });
            self.buffers.give(up);
            let down = self.linear(&gated, &layer.down)?;
            self.buffers.give(gated);
            add(&mut hidden, &down);
            self.buffers.give(down);
        }
        let mut pooled = self.buffers.take(h);
        pooled.copy_from_slice(&hidden[..h]);
        self.buffers.give(hidden);
        normalize(&mut pooled, &self.final_norm, self.config.epsilon);
        let mut outputs = Tensors::new();
        for (name, head) in &self.heads {
            let mut values = self.linear(&pooled, &head.weight)?;
            add(&mut values, &head.bias);
            outputs.insert(
                name.clone(),
                Tensor {
                    shape: vec![1, values.len()],
                    data: TensorData::F32(values),
                },
            );
        }
        self.buffers.give(pooled);
        Ok(outputs)
    }
}

impl ModelEngine for ModernBertEngine {
    fn execution_report(&self) -> serde_json::Value {
        #[cfg(feature = "native-metal")]
        if let Some(metal) = &self.metal {
            return metal.report();
        }
        #[cfg(feature = "native-cuda")]
        if let Some(cuda) = &self.cuda {
            return cuda.report();
        }
        if let Some(accelerator) = self.accelerator.as_ref().and_then(|a| a.lock().ok()) {
            let mut report = accelerator.report();
            report["cpu_operations"] = serde_json::json!(
                "embedding gather/dequantization, norms, RoPE, attention, GeGLU, residuals, and head biases"
            );
            return report;
        }
        let mut report = serde_json::json!({"placement":"CPU","activation_cache_limit_bytes":64*1024*1024,"activation_cache_max_buffers":128,"cached_activation_bytes":self.buffers.cached_bytes(),"cached_activation_buffers":self.buffers.cached_buffers(),"worker_cpus":self.pool.broadcast(|_|crate::cpu::affinity()),"rotary":"cached position tables","final_layer_query_rows":1,"projection":"gemm 0.19 f32","erf":"libm f32"});
        #[cfg(target_arch = "x86_64")]
        if simd::available() {
            report["projection"] = serde_json::json!(
                "AVX2/FMA packed 3x24 short and 16x6 long kernels; cache-blocked large activations"
            );
            report["erf"] = serde_json::json!(
                "f32 degree-10 piecewise polynomial; dense-domain and model-reference checks"
            );
            report["packed_weights_bytes"] = serde_json::json!(
                self.layers
                    .iter()
                    .flat_map(|l| [&l.qkv, &l.attn_out, &l.up, &l.down])
                    .filter_map(|w| w.packed.as_ref())
                    .map(|w| w.byte_len())
                    .sum::<usize>()
            );
        }
        report
    }

    fn identity(&self) -> &ExecutionIdentity {
        &self.identity
    }
    fn run(&mut self, inputs: &Tensors) -> Result<Tensors> {
        #[cfg(feature = "native-metal")]
        {
            let (ids, mask) = self.inputs(inputs)?;
            if let Some(metal) = &mut self.metal {
                return metal.run(ids, mask);
            }
        }
        #[cfg(feature = "native-cuda")]
        {
            let (ids, mask) = self.inputs(inputs)?;
            if let Some(cuda) = &mut self.cuda {
                return cuda.run(ids, mask);
            }
        }
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
        #[cfg(target_arch = "x86_64")]
        if simd::available() {
            // SAFETY: features were checked and the model supplies one complete weight per value.
            unsafe {
                simd::normalize_row(row, weight, epsilon);
            }
            return;
        }
        let mean = row.iter().sum::<f32>() / row.len() as f32;
        let variance =
            row.iter().map(|v| (*v - mean) * (*v - mean)).sum::<f32>() / row.len() as f32;
        let scale = 1.0 / (variance + epsilon).sqrt();
        for (value, weight) in row.iter_mut().zip(weight) {
            *value = (*value - mean) * scale * weight;
        }
    });
}

fn linear(input: &[f32], weight: &Matrix, buffers: &Buffers) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    if let Some(packed) = &weight.packed {
        return packed.multiply(input, buffers);
    }
    let rows = input.len() / weight.rows;
    let mut output = buffers.take(rows * weight.cols);
    gemm_parallel(
        rows,
        weight.cols,
        weight.rows,
        input,
        weight.rows,
        1,
        &weight.values,
        weight.cols,
        1,
        &mut output,
        gemm::Parallelism::Rayon(rayon::current_num_threads()),
    );
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
    gemm_parallel(
        m,
        n,
        k,
        a,
        ars,
        acs,
        b,
        brs,
        bcs,
        out,
        gemm::Parallelism::None,
    );
}

#[allow(clippy::too_many_arguments)]
fn gemm_parallel(
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
    parallelism: gemm::Parallelism,
) {
    debug_assert!(m > 0 && n > 0 && k > 0);
    debug_assert!(a.len() > (m - 1) * ars + (k - 1) * acs);
    debug_assert!(b.len() > (k - 1) * brs + (n - 1) * bcs);
    debug_assert_eq!(out.len(), m * n);
    // SAFETY: all dimensions and strides describe the checked slices above;
    // output is a distinct contiguous m-by-n allocation.
    unsafe {
        gemm::gemm(
            m,
            n,
            k,
            out.as_mut_ptr(),
            1,
            n as isize,
            false,
            a.as_ptr(),
            acs as isize,
            ars as isize,
            b.as_ptr(),
            bcs as isize,
            brs as isize,
            0.0_f32,
            1.0,
            false,
            false,
            false,
            parallelism,
        );
    }
}

#[cfg(test)]
fn attention(
    qkv: &[f32],
    n: usize,
    h: usize,
    heads: usize,
    mask: &[i64],
    config: &LayerConfig,
) -> Vec<f32> {
    let rotary = rotary_table(config, n);
    attention_prepared(
        qkv,
        n,
        h,
        heads,
        mask,
        config,
        &rotary,
        n,
        &Buffers::default(),
    )
}

fn rotary_table(config: &LayerConfig, n: usize) -> Vec<f32> {
    let half = config.inverse_frequencies.len();
    let mut values = vec![0.0; n * half * 2];
    for t in 0..n {
        for (d, f) in config.inverse_frequencies.iter().enumerate() {
            let (sin, cos) = (t as f32 * f).sin_cos();
            values[t * half * 2 + d] = sin;
            values[t * half * 2 + half + d] = cos;
        }
    }
    values
}

#[allow(clippy::too_many_arguments)]
fn attention_prepared(
    qkv: &[f32],
    n: usize,
    h: usize,
    heads: usize,
    mask: &[i64],
    config: &LayerConfig,
    rotary: &[f32],
    queries: usize,
    buffers: &Buffers,
) -> Vec<f32> {
    let dim = h / heads;
    let mut per_head = buffers.take(heads * queries * dim);
    per_head
        .par_chunks_mut(queries * dim)
        .enumerate()
        .for_each(|(head, out)| {
            let mut q = buffers.take(queries * dim);
            let mut k = buffers.take(n * dim);
            let mut v = buffers.take(n * dim);
            // The ONNX graph scales Q and K independently before their product.
            let scale = (1.0 / (dim as f32).sqrt()).sqrt();
            for t in 0..n {
                let offset = t * 3 * h + head * dim;
                if t < queries {
                    q[t * dim..(t + 1) * dim].copy_from_slice(&qkv[offset..offset + dim]);
                }
                k[t * dim..(t + 1) * dim].copy_from_slice(&qkv[offset + h..offset + h + dim]);
                v[t * dim..(t + 1) * dim]
                    .copy_from_slice(&qkv[offset + 2 * h..offset + 2 * h + dim]);
            }
            #[cfg(target_arch = "x86_64")]
            let rotated = if simd::available() {
                // SAFETY: CPU features and model dimensions were validated; each table covers all positions.
                unsafe {
                    simd::rotate(&mut q, dim, rotary, scale);
                    simd::rotate(&mut k, dim, rotary, scale);
                }
                true
            } else {
                false
            };
            #[cfg(not(target_arch = "x86_64"))]
            let rotated = false;
            if !rotated {
                for values in [&mut q, &mut k] {
                    for (t, row) in values.chunks_mut(dim).enumerate() {
                        for d in 0..dim / 2 {
                            let sin = rotary[t * dim + d];
                            let cos = rotary[t * dim + dim / 2 + d];
                            let a = row[d];
                            let b = row[dim / 2 + d];
                            row[d] = (a * cos - b * sin) * scale;
                            row[dim / 2 + d] = (b * cos + a * sin) * scale;
                        }
                    }
                }
            }
            let mut scores = buffers.take(queries * n);
            gemm(queries, n, dim, &q, dim, 1, &k, 1, dim, &mut scores);
            for (t, row) in scores.chunks_mut(n).enumerate() {
                let first = config.window.map_or(0, |w| t.saturating_sub(w));
                let end = config
                    .window
                    .map_or(n, |w| t.saturating_add(w).saturating_add(1).min(n));
                row[..first].fill(0.0);
                row[end..].fill(0.0);
                let row = &mut row[first..end];
                let mut max = f32::NEG_INFINITY;
                for (i, value) in row.iter_mut().enumerate() {
                    if mask[first + i] == 0 {
                        *value = f32::NEG_INFINITY;
                    }
                    max = max.max(*value);
                }
                if max == f32::NEG_INFINITY {
                    row.fill(0.0);
                    continue;
                }
                #[cfg(target_arch = "x86_64")]
                if simd::available() {
                    // SAFETY: AVX2/FMA were checked and the maximum is finite.
                    unsafe {
                        simd::softmax_row(row, max);
                    }
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
            gemm(queries, dim, n, &scores, n, 1, &v, dim, 1, out);
            for values in [q, k, v, scores] {
                buffers.give(values);
            }
        });
    let mut output = buffers.take(queries * h);
    for t in 0..queries {
        for head in 0..heads {
            output[t * h + head * dim..t * h + (head + 1) * dim].copy_from_slice(
                &per_head[head * queries * dim + t * dim..head * queries * dim + (t + 1) * dim],
            );
        }
    }
    buffers.give(per_head);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_cache_evicts_old_buffers_and_retains_recent_sizes() {
        let buffers = Buffers::default();
        for len in 1..=129 {
            buffers.give(vec![7.0; len]);
        }
        assert_eq!(buffers.take(1), vec![0.0]);
        assert_eq!(buffers.take(129), vec![7.0; 129]);
        assert!(buffers.cached_bytes() <= 64 * 1024 * 1024 && buffers.cached_buffers() <= 128);
    }
    #[test]
    fn first_query_pruning_preserves_attention_output() {
        let n = 129;
        let h = 8;
        let heads = 2;
        let qkv: Vec<_> = (0..n * 3 * h)
            .map(|i| ((i * 17 % 47) as f32 - 23.0) / 53.0)
            .collect();
        let mask: Vec<_> = (0..n).map(|i| i64::from(i % 3 != 2)).collect();
        for window in [None, Some(1), Some(64)] {
            let config = LayerConfig {
                window,
                inverse_frequencies: vec![1.0, 0.01],
            };
            let all = attention(&qkv, n, h, heads, &mask, &config);
            let pruned = attention_prepared(
                &qkv,
                n,
                h,
                heads,
                &mask,
                &config,
                &rotary_table(&config, n),
                1,
                &Buffers::default(),
            );
            assert!(
                all[..h]
                    .iter()
                    .zip(&pruned)
                    .all(|(a, b)| (a - b).abs() < 1e-6)
            );
        }
    }
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
