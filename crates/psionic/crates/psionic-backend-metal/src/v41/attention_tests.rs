//! Parity tests for the V4.1 attention kernels on Metal against the CPU
//! path, ported from the CUDA backend's `v41_attention_tests.rs` (#12420).
//! The tolerances are the CUDA tests', fixed before the first GPU run there;
//! see that file for their derivation. Every kernel must equal its host
//! model bit for bit (results below 2^-126 flushed, as Apple GPUs flush
//! them). RoPE reads the host's cos and sin table for every position below
//! 2^20.

use super::*;
use crate::v41::matvec::{ftz, v41_round_bf16};
use crate::v41::{V41Buffer, V41Metal};
use psionic_models::deepseek_v41_cpu::attention::{
    CompressionState, candidate_mask, select_indices,
};
use psionic_models::deepseek_v41_cpu::projection::{
    AttentionProjections, Geometry, WeightSource, compress_ratio,
};
use psionic_models::deepseek_v41_cpu::quantization::{
    Arithmetic, MatrixPrecision, Quantization, inplace_bf16, round_bf16, sparse_attention_bf16,
};
use psionic_models::deepseek_v41_cpu::rope::{Rope, RopeConfig};
use psionic_models::deepseek_v41_cpu::shared_state::LayerOwners;
use psionic_models::deepseek_v41_cpu::{CpuError, MatrixRef, MatrixRows, rms_norm};
use psionic_models::deepseek_v41_weights::{decode_bf16, decode_f8_e4m3fn, decode_f8_e8m0fnu};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, path::PathBuf};

type TestResult = Result<(), Box<dyn Error>>;

const U: f64 = 1.0 / 16_777_216.0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Section {
    label: String,
    dtype: String,
    shape: Vec<u64>,
    offset: usize,
    bytes: usize,
    sha256: String,
}

#[derive(Deserialize)]
struct Fixture {
    blob_sha256: String,
    sections: Vec<Section>,
}

struct Rows {
    fixture: Fixture,
    blob: Vec<u8>,
}

impl Rows {
    fn load() -> Result<Self, Box<dyn Error>> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../psionic-backend-cuda/testdata/deepseek_v41_attention");
        let fixture: Fixture = serde_json::from_slice(&std::fs::read(dir.join("rows.json"))?)?;
        let blob = std::fs::read(dir.join("rows.bin"))?;
        assert_eq!(hex::encode(Sha256::digest(&blob)), fixture.blob_sha256);
        for s in &fixture.sections {
            assert_eq!(
                hex::encode(Sha256::digest(&blob[s.offset..s.offset + s.bytes])),
                s.sha256,
                "{}",
                s.label
            );
        }
        Ok(Self { fixture, blob })
    }

    fn section(&self, label: &str) -> Option<&Section> {
        self.fixture.sections.iter().find(|s| s.label == label)
    }

    fn has(&self, label: &str) -> bool {
        self.section(label).is_some()
    }

    fn raw(&self, label: &str) -> &[u8] {
        let s = self
            .section(label)
            .unwrap_or_else(|| panic!("no section {label}"));
        &self.blob[s.offset..s.offset + s.bytes]
    }

    fn shape(&self, label: &str) -> Vec<usize> {
        self.section(label)
            .map(|s| s.shape.iter().map(|d| *d as usize).collect())
            .unwrap_or_default()
    }

    /// Float values of a BF16 or F32 section.
    fn f32(&self, label: &str) -> Vec<f32> {
        let dtype = self.section(label).map(|s| s.dtype.as_str());
        let raw = self.raw(label);
        match dtype {
            Some("bfloat16" | "Bf16") => raw
                .chunks_exact(2)
                .map(|p| decode_bf16([p[0], p[1]]))
                .collect(),
            Some("float32" | "F32") => raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
            other => panic!("{label}: not a float section ({other:?})"),
        }
    }

    /// BF16 words of a BF16 section.
    fn words(&self, label: &str) -> Vec<u16> {
        assert!(matches!(
            self.section(label).map(|s| s.dtype.as_str()),
            Some("bfloat16" | "Bf16")
        ));
        self.raw(label)
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect()
    }

    fn ints(&self, label: &str) -> Vec<i64> {
        let dtype = self.section(label).map(|s| s.dtype.as_str());
        let raw = self.raw(label);
        match dtype {
            Some("int32") => raw
                .chunks_exact(4)
                .map(|c| i64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
            Some("int64") => raw
                .chunks_exact(8)
                .map(|c| i64::from_le_bytes(c.try_into().expect("8 bytes")))
                .collect(),
            other => panic!("{label}: not an integer section ({other:?})"),
        }
    }
}

/// The oracle prompts: name, prompt length, and the steps the fixture holds.
const PROMPTS: [(&str, usize, [(&str, usize); 4]); 7] = [
    (
        "p1-chat-thinking",
        41,
        [("last", 0), ("g1", 1), ("g2", 2), ("g32", 32)],
    ),
    (
        "p2-code",
        40,
        [("last", 0), ("g1", 1), ("g2", 2), ("g32", 32)],
    ),
    (
        "p3-2k",
        2124,
        [("last", 0), ("g1", 1), ("g2", 2), ("g32", 32)],
    ),
    (
        "p4-20k",
        19989,
        [("last", 0), ("g1", 1), ("g2", 2), ("g32", 32)],
    ),
    (
        "e-window-127",
        127,
        [("last", 0), ("g1", 1), ("g2", 2), ("g8", 8)],
    ),
    (
        "e-window-128",
        128,
        [("last", 0), ("g1", 1), ("g2", 2), ("g8", 8)],
    ),
    (
        "e-window-129",
        129,
        [("last", 0), ("g1", 1), ("g2", 2), ("g8", 8)],
    ),
];

/// Every recorded `(prompt, step, position)`; step `gK` is position
/// `L + K - 1` and `last` is `L - 1`.
fn oracle_steps() -> Vec<(&'static str, &'static str, usize)> {
    let mut steps = Vec::new();
    for (prompt, len, labels) in PROMPTS {
        for (step, k) in labels {
            steps.push((prompt, step, len + k - 1));
        }
    }
    steps
}

/// A matrix of fixture values with the checkpoint's precision.
struct FxMatrix {
    rows: usize,
    cols: usize,
    values: Vec<f32>,
    precision: MatrixPrecision,
}

impl MatrixRows for FxMatrix {
    fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    fn row(&self, i: usize) -> Result<Vec<f32>, CpuError> {
        Ok(self.values[i * self.cols..(i + 1) * self.cols].to_vec())
    }
    fn precision(&self) -> MatrixPrecision {
        self.precision
    }
}

/// Checkpoint tensors by name, for the CPU path's projection wiring.
#[derive(Default)]
struct FxWeights {
    tensors: BTreeMap<String, (Vec<usize>, Vec<f32>, MatrixPrecision)>,
}

impl FxWeights {
    fn from_rows(rows: &Rows) -> Self {
        let mut weights = Self::default();
        for s in &rows.fixture.sections {
            if s.label.starts_with("layers.") {
                let precision = if s.dtype == "Bf16" {
                    MatrixPrecision::Bf16
                } else {
                    MatrixPrecision::F32
                };
                weights.tensors.insert(
                    s.label.clone(),
                    (rows.shape(&s.label), rows.f32(&s.label), precision),
                );
            }
        }
        weights
    }
}

impl WeightSource for FxWeights {
    fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
        let (shape, values, precision) = self
            .tensors
            .get(name)
            .ok_or(CpuError::Invalid("missing fixture matrix"))?;
        Ok(Box::new(FxMatrix {
            rows: shape[0],
            cols: shape[1],
            values: values.clone(),
            precision: *precision,
        }))
    }
    fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
        Ok(self
            .tensors
            .get(name)
            .ok_or(CpuError::Invalid("missing fixture vector"))?
            .1
            .clone())
    }
}

fn projections(weights: &FxWeights, layer: usize) -> Result<AttentionProjections<'_>, CpuError> {
    AttentionProjections::new(weights, layer, Geometry::flash())?
        .with_arithmetic(Arithmetic::ReferenceBf16)
}

// ---------------------------------------------------------------------------
// Bounds and comparisons
// ---------------------------------------------------------------------------

fn gamma(n: usize) -> f64 {
    let nu = n as f64 * U;
    nu / (1.0 - nu)
}

fn bf16r(x: f32) -> f32 {
    v41_round_bf16(x)
}

fn same_bits(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_value(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// Checks device values against host-model values bit for bit.
/// The device equals the host model with subnormal results flushed.
fn assert_host(device: &[f32], host: &[f32], what: &str) {
    assert_eq!(device.len(), host.len(), "{what} length");
    for (i, (a, b)) in device.iter().zip(host).enumerate() {
        let b = ftz(*b);
        assert!(same_bits(*a, b), "{what}[{i}]: device {a} host {b}");
    }
}

/// Checks values against the CPU's as values (zero signs may differ).
fn assert_equal(gpu: &[f32], cpu: &[f32], what: &str) {
    assert_eq!(gpu.len(), cpu.len(), "{what} length");
    for (i, (a, b)) in gpu.iter().zip(cpu).enumerate() {
        assert!(same_value(*a, *b), "{what}[{i}]: gpu {a} cpu {b}");
    }
}

/// Checks |gpu - cpu| <= bound; returns the largest ratio.
fn assert_bound(gpu: &[f32], cpu: &[f32], bound: &[f64], what: &str) -> f64 {
    let mut worst = 0.0_f64;
    for i in 0..gpu.len() {
        let diff = (f64::from(gpu[i]) - f64::from(cpu[i])).abs();
        assert!(
            diff <= bound[i],
            "{what}[{i}]: |{} - {}| = {diff:e} > {:e}",
            gpu[i],
            cpu[i],
            bound[i]
        );
        if bound[i] > 0.0 {
            worst = worst.max(diff / bound[i]);
        }
    }
    worst
}

/// Whether a float32 value's bound reaches a BF16 rounding boundary.
fn reaches_boundary(value: f32, bound: f64) -> bool {
    bf16r((f64::from(value) - bound) as f32) != bf16r((f64::from(value) + bound) as f32)
}

/// BF16 outputs against the CPU's float32 values: equal after rounding, or a
/// rounding boundary within the bound. Returns the boundary count.
fn assert_bf16(gpu_bf16: &[f32], cpu_f32: &[f32], bound: &[f64], what: &str) -> usize {
    let mut boundary = 0;
    for i in 0..gpu_bf16.len() {
        if !same_value(gpu_bf16[i], bf16r(cpu_f32[i])) {
            assert!(
                reaches_boundary(cpu_f32[i], bound[i]),
                "{what}[{i}]: BF16 {} differs from {} with no boundary in reach",
                gpu_bf16[i],
                cpu_f32[i]
            );
            boundary += 1;
        }
    }
    boundary
}

/// The W1 tolerance for BF16 activations: 2^-6 absolute plus 2^-6 relative.
fn policy_bf16(values: &[f32], oracle: &[f32]) -> f64 {
    values
        .iter()
        .zip(oracle)
        .map(|(a, b)| f64::from((a - b).abs()) / (0.015625 + 0.015625 * f64::from(b.abs())))
        .fold(0.0, f64::max)
}

/// The CPU's tail rotation of one head (float32, no rounding) and the RoPE
/// bound of each value.
fn cpu_rope_bound(
    rope: &Rope,
    head: &[f32],
    position: usize,
    inverse: bool,
) -> Result<(Vec<f32>, Vec<f64>), CpuError> {
    let mut out = head.to_vec();
    rope.apply_tail(&mut out, head.len(), position, inverse)?;
    let prefix = head.len() - rope.dim();
    let mut bound = vec![0.0; head.len()];
    for i in (prefix..head.len()).step_by(2) {
        let ab = f64::from(head[i].abs()) + f64::from(head[i + 1].abs());
        bound[i] = ab * 4.0 * U + 2.0 * U * f64::from(out[i].abs());
        bound[i + 1] = ab * 4.0 * U + 2.0 * U * f64::from(out[i + 1].abs());
    }
    Ok((out, bound))
}

/// Compares a device cache form, produced from a rotated head, with the
/// CPU's `inplace_bf16(bf16(rotated))` block by block: equal unless a value
/// of the block has its RoPE bound at a BF16 boundary. Returns the number of
/// such blocks.
fn assert_rotated_blocks(
    device: &[f32],
    rotated: &[f32],
    bound: &[f64],
    kind: Quantization,
    block: usize,
    what: &str,
) -> Result<usize, CpuError> {
    let cpu = inplace_bf16(&round_bf16(rotated)?, kind)?;
    let mut reached = 0;
    for b in 0..device.len() / block {
        let range = b * block..(b + 1) * block;
        if range
            .clone()
            .any(|i| reaches_boundary(rotated[i], bound[i]))
        {
            reached += 1;
            continue;
        }
        assert_equal(
            &device[range.clone()],
            &cpu[range],
            &format!("{what} block {b}"),
        );
    }
    Ok(reached)
}

/// The CPU's `sparse_attention_bf16` for one head, transcribed with its
/// float `exp`: returns the float32 output before BF16 rounding and the
/// bound from the header.
fn cpu_head_bound(
    q: &[f32],
    kv: &[f32],
    indices: &[i32],
    sink: f32,
    scale: f32,
) -> (Vec<f32>, Vec<f64>) {
    let d = q.len();
    let q: Vec<f32> = q.iter().map(|x| bf16r(*x)).collect();
    let mut max = -1e30_f32;
    let mut sum = 0.0_f32;
    let mut out = vec![0.0_f32; d];
    let mut magnitude = vec![0.0_f64; d];
    let mut flips = vec![0.0_f64; d];
    let mut weight_total = 0.0_f64;
    let mut valid = 0;
    let mut chunks = 0;
    let eps = 4.0 * U;
    for chunk in indices.chunks(64) {
        chunks += 1;
        let previous = max;
        let mut scores = Vec::new();
        for id in chunk.iter().copied().filter(|i| *i >= 0) {
            let id = id as usize;
            let score = q
                .iter()
                .zip(&kv[id * d..(id + 1) * d])
                .map(|(q, k)| q * bf16r(*k))
                .sum::<f32>()
                * scale;
            max = max.max(score);
            scores.push((id, score));
        }
        let rescale = (previous - max).exp();
        sum *= rescale;
        for x in &mut out {
            *x *= rescale;
        }
        for (id, score) in scores {
            let w = (score - max).exp();
            sum += w;
            valid += 1;
            weight_total += f64::from(w);
            let wb = bf16r(w);
            let lo = bf16r((f64::from(w) * (1.0 - eps)) as f32);
            let hi = bf16r((f64::from(w) * (1.0 + eps)) as f32);
            let flip = f64::from((hi - lo).abs());
            for i in 0..d {
                let v = bf16r(kv[id * d + i]);
                out[i] += wb * v;
                magnitude[i] += f64::from(wb) * f64::from(v.abs());
                flips[i] += flip * f64::from(v.abs());
            }
        }
    }
    let sink_term = (sink - max).exp();
    let sum = sum + sink_term;
    let s = f64::from(sum);
    let k = chunks as f64;
    let ds = 4.0 * (k + 1.0) * U * weight_total
        + 4.0 * U * f64::from(sink_term)
        + 2.0 * gamma(valid + 2 * chunks + 1) * (weight_total + f64::from(sink_term));
    let result: Vec<f32> = out.iter().map(|x| x / sum).collect();
    let bound = (0..d)
        .map(|i| {
            let dacc =
                flips[i] + (4.0 * k * U + 2.0 * gamma(2 * valid + 2 * chunks)) * magnitude[i];
            let o = f64::from(result[i].abs());
            (dacc / s + o * ds / s + 2.0 * U * o) * (1.0 + 2.0 * ds / s)
        })
        .collect();
    (result, bound)
}

/// One token's attention on the CPU path: the BF16 output of
/// `sparse_attention_bf16`, plus the float32 values and bounds of the
/// transcription, which must round to that output.
fn cpu_attention(
    g: V41AttentionGeometry,
    query: &[f32],
    sinks: &[f32],
    kv: &[f32],
    indices: &[i32],
) -> Result<(Vec<f32>, Vec<f32>, Vec<f64>), CpuError> {
    let d = g.head_dim;
    let cpu = sparse_attention_bf16(query, g.heads, d, kv, indices, sinks, g.attention_scale())?;
    let mut values = Vec::new();
    let mut bounds = Vec::new();
    for h in 0..g.heads {
        let (v, b) = cpu_head_bound(
            &query[h * d..(h + 1) * d],
            kv,
            indices,
            sinks[h],
            g.attention_scale(),
        );
        values.extend(v);
        bounds.extend(b);
    }
    for (i, (a, b)) in cpu.iter().zip(&values).enumerate() {
        assert!(same_bits(*a, bf16r(*b)), "transcription [{i}]: {a} vs {b}");
    }
    Ok((cpu, values, bounds))
}

/// The CPU path's reference index scores (`shared_state.rs`'s private
/// `reference_index_scores`, transcribed).
fn cpu_index_scores(
    g: V41AttentionGeometry,
    query: &[f32],
    weights: &[f32],
    keys: &[f32],
) -> Result<Vec<f32>, CpuError> {
    let (heads, width) = (g.index_heads, g.index_dim);
    let scale = ((width as f64).powf(-0.5) * (heads as f64).powf(-0.5)) as f32;
    let weights = round_bf16(&weights.iter().map(|w| w * scale).collect::<Vec<_>>())?;
    let mut out = Vec::new();
    for key in keys.chunks_exact(width) {
        let mut score = 0.0;
        for h in 0..heads {
            let dot = query[h * width..(h + 1) * width]
                .iter()
                .zip(key)
                .map(|(q, k)| q * k)
                .sum::<f32>();
            if !dot.is_finite() {
                return Err(CpuError::NonFinite("reference index dot product"));
            }
            score += dot.max(0.0) * weights[h];
        }
        out.push(score);
    }
    Ok(out)
}

/// The CPU compressor's pooled values for one ratio-2 group with the
/// float `exp`, and their bounds from the header.
fn cpu_pool_bound(values: &[&[f32]], gates: &[&[f32]]) -> (Vec<f32>, Vec<f64>) {
    let width = values[0].len();
    let mut pooled = Vec::with_capacity(width);
    let mut bound = Vec::with_capacity(width);
    for d in 0..width {
        let max = gates.iter().map(|g| g[d]).fold(f32::NEG_INFINITY, f32::max);
        let denom: f32 = gates.iter().map(|g| (g[d] - max).exp()).sum();
        let mut value = 0.0_f32;
        let mut magnitude = 0.0_f64;
        for (v, g) in values.iter().zip(gates) {
            let q = (g[d] - max).exp() / denom;
            value += v[d] * q;
            magnitude += f64::from((v[d] * q).abs());
        }
        pooled.push(value);
        bound.push(17.0 * U * magnitude);
    }
    (pooled, bound)
}

/// The fallback bound on a latent whose BF16 pooled values may differ by
/// `dx` from the CPU's `x`: returns (CPU latent before rounding, bound).
fn rms_bound(
    x: &[f32],
    dx: &[f64],
    norm: &[f32],
    epsilon: f32,
) -> Result<(Vec<f32>, Vec<f64>), CpuError> {
    let w = round_bf16(norm)?;
    let out = rms_norm(x, &w, epsilon)?;
    let n = x.len();
    let squares: f64 = x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum();
    let variance = squares / n as f64 + f64::from(epsilon);
    let s = 1.0 / variance.sqrt();
    let change: f64 = x
        .iter()
        .zip(dx)
        .map(|(v, d)| 2.0 * f64::from(v.abs()) * d + d * d)
        .sum();
    let rel = change / (2.0 * squares.max(f64::MIN_POSITIVE)) + 2.0 * gamma(n) + 4.0 * U;
    let bound = (0..n)
        .map(|i| {
            f64::from(w[i].abs()) * (dx[i] * s + f64::from(x[i].abs()) * s * rel)
                + 4.0 * U * f64::from(out[i].abs())
        })
        .collect();
    Ok((out, bound))
}

// ---------------------------------------------------------------------------
// Device helpers
// ---------------------------------------------------------------------------

/// The Metal kernels; every Mac these tests run on has a Metal 3 GPU.
fn metal() -> Option<V41Metal> {
    Some(V41Metal::new().expect("the V4.1 Metal kernels compile"))
}

fn zeros(backend: &mut V41Metal, bytes: usize) -> Result<V41Buffer, RuntimeError> {
    backend.byte_buffer(&vec![0; bytes.max(4)])
}

fn filled_f32(backend: &mut V41Metal, len: usize, value: f32) -> Result<V41Buffer, RuntimeError> {
    upload_f32(backend, &vec![value; len.max(1)])
}

fn f32_buffer(backend: &mut V41Metal, len: usize) -> Result<V41Buffer, RuntimeError> {
    backend.f32_buffer(len.max(1))
}

fn upload_f32(backend: &mut V41Metal, values: &[f32]) -> Result<V41Buffer, RuntimeError> {
    backend.byte_buffer(
        &values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

fn upload_u16(backend: &mut V41Metal, values: &[u16]) -> Result<V41Buffer, RuntimeError> {
    let mut bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes.resize(bytes.len().max(4), 0);
    backend.byte_buffer(&bytes)
}

fn upload_i32(backend: &mut V41Metal, values: &[i32]) -> Result<V41Buffer, RuntimeError> {
    let mut bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes.resize(bytes.len().max(4), 0);
    backend.byte_buffer(&bytes)
}

fn read_f32(buffer: &V41Buffer, len: usize) -> Result<Vec<f32>, RuntimeError> {
    Ok(buffer.read_bytes()?[..len * 4]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn read_i32(buffer: &V41Buffer, len: usize) -> Result<Vec<i32>, RuntimeError> {
    Ok(buffer.read_bytes()?[..len * 4]
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn read_bf16(buffer: &V41Buffer, len: usize) -> Result<Vec<f32>, RuntimeError> {
    Ok(buffer.read_bytes()?[..len * 2]
        .chunks_exact(2)
        .map(|c| decode_bf16([c[0], c[1]]))
        .collect())
}

fn words(values: &[f32]) -> Vec<u16> {
    values.iter().map(|v| v41_bf16_bits(*v)).collect()
}

/// A small deterministic generator for synthetic cases.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform in [-1, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    /// A BF16 value, roughly normal times `scale`.
    fn bf16(&mut self, scale: f32) -> f32 {
        bf16r((self.unit() + self.unit() + self.unit()) * scale)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The frequencies and the device's cos and sin table for every position
/// below 2^20.
fn rope_table(
    backend: &mut V41Metal,
    config: V41RopeConfig,
) -> Result<(Vec<f32>, V41Buffer), Box<dyn Error>> {
    let frequencies = v41_rope_frequencies(config)?;
    let buffer = upload_f32(
        backend,
        &v41_rope_table_host(&frequencies, V41_MAX_POSITION),
    )?;
    Ok((frequencies, buffer))
}

fn cpu_rope(config: V41RopeConfig) -> Result<Rope, CpuError> {
    Rope::new(RopeConfig {
        dim: config.dim,
        base: config.base,
        original_context: config.original_context,
        factor: config.factor,
        beta_fast: config.beta_fast,
        beta_slow: config.beta_slow,
    })
}

/// A geometry with fewer heads, for synthetic cases.
fn small(heads: usize) -> V41AttentionGeometry {
    V41AttentionGeometry {
        heads,
        ..V41AttentionGeometry::flash()
    }
}

// ---------------------------------------------------------------------------
// CPU-only checks (every machine)
// ---------------------------------------------------------------------------

#[test]
fn host_decoders_and_schedule_match_the_cpu() -> TestResult {
    for dim in [2, 4, 64, 128] {
        for (config, cpu) in [
            (V41RopeConfig::window(dim), RopeConfig::window(dim)),
            (V41RopeConfig::compressed(dim), RopeConfig::compressed(dim)),
        ] {
            let host = v41_rope_frequencies(config)?;
            let cpu = Rope::new(cpu)?;
            assert_eq!(host.len(), cpu.frequencies().len());
            for (a, b) in host.iter().zip(cpu.frequencies()) {
                assert!(same_bits(*a, *b), "dim {dim}: {a} vs {b}");
            }
        }
    }
    assert!(v41_rope_frequencies(V41RopeConfig::window(3)).is_err());
    for layer in 0..40 {
        let cpu = LayerOwners::for_layer(layer)?;
        let host = V41LayerOwners::for_layer(layer).ok_or("owners")?;
        assert_eq!(
            (host.kv, host.index, host.candidates),
            (cpu.kv, cpu.index, cpu.candidates),
            "layer {layer}"
        );
        assert_eq!(v41_compress_ratio(layer), Some(compress_ratio(layer)?));
        assert_eq!(
            V41RopeConfig::for_layer(layer, 64).map(|c| c.base),
            Some(if layer < 2 { 10_000.0 } else { 160_000.0 })
        );
    }
    assert!(V41LayerOwners::for_layer(40).is_none());
    let g = V41AttentionGeometry::flash();
    assert!(check_geometry(g).is_ok());
    assert_eq!(g.attention_scale(), (512_f64).powf(-0.5) as f32);
    assert_eq!(
        g.index_scale(),
        ((128_f64).powf(-0.5) * (32_f64).powf(-0.5)) as f32
    );
    for bad in [
        V41AttentionGeometry {
            head_dim: 1024,
            ..g
        },
        V41AttentionGeometry { rope_dim: 3, ..g },
        V41AttentionGeometry { window: 0, ..g },
        V41AttentionGeometry {
            index_top_k: 0,
            ..g
        },
        V41AttentionGeometry {
            norm_epsilon: 0.0,
            ..g
        },
    ] {
        assert!(check_geometry(bad).is_err(), "{bad:?}");
    }
    assert!(
        check_compressor(V41Compressor {
            ratio: 9,
            width: 512,
            epsilon: 1e-20
        })
        .is_err()
    );
    Ok(())
}

/// Blocks that exercise every branch of the three cache quantizers: random
/// magnitudes from 2^-140 to 2^100, zeros and negative zeros, exact code
/// midpoints, saturation, and values that make the CPU refuse.
fn quant_blocks(kind: V41CacheQuant, rng: &mut Rng) -> Vec<Vec<f32>> {
    let n = kind.block();
    let mut blocks = Vec::new();
    for e in (-140..=100).step_by(4) {
        let scale = 2.0_f32.powi(e);
        blocks.push((0..n).map(|_| rng.unit() * scale).collect());
    }
    blocks.push(vec![0.0; n]);
    blocks.push(vec![-0.0; n]);
    // Midpoints between neighboring codes at scale 1 (the block max fixes
    // the scale), and saturation.
    let mut mid: Vec<f32> = (0..n).map(|i| 0.25 + 0.5 * i as f32 / n as f32).collect();
    mid[0] = 6.0;
    blocks.push(mid);
    let mut fp8_mid: Vec<f32> = (0..n).map(|i| (1.0 + 0.0625 * i as f32) * 4.0).collect();
    fp8_mid[0] = 448.0;
    blocks.push(fp8_mid);
    blocks.push((0..n).map(|i| if i == 0 { 3e38 } else { 1.0 }).collect());
    let mut nan = vec![1.0; n];
    nan[3] = f32::NAN;
    blocks.push(nan);
    let mut inf = vec![1.0; n];
    inf[5] = f32::NEG_INFINITY;
    blocks.push(inf);
    blocks
}

#[test]
fn host_quantizers_match_the_cpu() {
    let mut rng = Rng(7);
    for (kind, cpu_kind) in [
        (V41CacheQuant::Fp8, Quantization::Fp8),
        (V41CacheQuant::IndexFp4, Quantization::IndexFp4),
        (V41CacheQuant::CompressedFp4, Quantization::CompressedFp4),
    ] {
        let mut refused = 0;
        for block in quant_blocks(kind, &mut rng) {
            let host = v41_quant_block_host(kind, &block);
            match inplace_bf16(&block, cpu_kind) {
                Ok(cpu) => assert_host(&host, &cpu, &format!("{kind:?}")),
                Err(_) => {
                    refused += 1;
                    assert!(host.iter().all(|v| v.is_nan()), "{kind:?} refusal");
                }
            }
        }
        assert!(refused >= 2, "{kind:?}: {refused} refusals");
    }
}

fn cpu_select(scores: &[f32], top_k: usize, mask: Option<&[bool]>) -> Result<Vec<i32>, CpuError> {
    let mut out = select_indices(scores, scores.len(), top_k, 0, mask)?;
    out.resize(top_k, -1);
    Ok(out)
}

/// Scores with many exact ties, some -inf, at the given length.
fn tie_scores(rng: &mut Rng, n: usize) -> Vec<f32> {
    (0..n)
        .map(|_| match rng.below(16) {
            0 => f32::NEG_INFINITY,
            1..=6 => 1.5,
            _ => bf16r(rng.unit() * 4.0),
        })
        .collect()
}

#[test]
fn host_selection_matches_the_cpu() -> TestResult {
    let mut rng = Rng(11);
    for n in [1, 2, 7, 8, 9, 511, 512, 513, 2049, 16_384, 16_392, 20_001] {
        for scores in [
            (0..n).map(|_| rng.unit()).collect::<Vec<_>>(),
            tie_scores(&mut rng, n),
            (0..n).map(|i| -(i as f32)).collect(),
        ] {
            assert_eq!(
                v41_select_host(&scores, None, 512, false),
                cpu_select(&scores, 512, None)?,
                "n {n}"
            );
            let finite: Vec<f32> = scores
                .iter()
                .map(|s| if s.is_finite() { *s } else { -1e9 })
                .collect();
            let (_, flags, blocks) = v41_candidate_blocks_host(&finite, 2048, 8);
            let mask = candidate_mask(&finite, n, 2048, 8)?;
            let expanded: Vec<bool> = (0..n).map(|i| flags[i / 8] == 1).collect();
            assert_eq!(expanded, mask, "candidates n {n}");
            assert!(blocks.iter().filter(|b| **b >= 0).count() <= 2048);
            assert_eq!(
                v41_select_host(&scores, Some((&flags, 8)), 512, false),
                cpu_select(&scores, 512, Some(&mask))?,
                "masked n {n}"
            );
        }
    }
    let mut nan = vec![1.0; 9];
    nan[4] = f32::NAN;
    assert!(select_indices(&nan, 9, 4, 0, None).is_err());
    assert_eq!(v41_select_host(&nan, None, 4, false), vec![i32::MIN; 4]);
    let mut inf = vec![1.0; 9];
    inf[2] = f32::INFINITY;
    assert!(select_indices(&inf, 9, 4, 0, None).is_err());
    assert_eq!(v41_select_host(&inf, None, 4, false), vec![i32::MIN; 4]);
    assert_eq!(
        v41_select_host(&[], None, 4, false),
        cpu_select(&[], 4, None)?
    );
    Ok(())
}

#[test]
fn host_models_match_the_cpu_components() -> TestResult {
    let mut rng = Rng(13);
    // Dense products in all three formats, grouped and not.
    let (rows, cols) = (64, 96);
    let weights: Vec<f32> = (0..rows * cols).map(|_| rng.bf16(0.05)).collect();
    let input: Vec<f32> = (0..cols).map(|_| rng.unit() * 3.0).collect();
    let bf16_words = words(&weights);
    let matrix = FxMatrix {
        rows,
        cols,
        values: weights.clone(),
        precision: MatrixPrecision::Bf16,
    };
    let cpu = psionic_models::deepseek_v41_cpu::quantization::linear(
        &matrix,
        &input,
        Arithmetic::ReferenceBf16,
    )?;
    let host = v41_dense_host(
        V41DenseHost::Bf16(&bf16_words),
        &input,
        rows,
        cols,
        1,
        true,
        V41Output::Bf16,
    );
    assert_equal(&host, &cpu, "BF16 linear");
    let cpu =
        psionic_models::deepseek_v41_cpu::matvec(&MatrixRef::new(rows, cols, &weights)?, &input)?;
    let host = v41_dense_host(
        V41DenseHost::F32(&weights),
        &input,
        rows,
        cols,
        1,
        false,
        V41Output::F32,
    );
    assert_equal(&host, &cpu, "F32 matvec");
    let codes: Vec<u8> = (0..rows * cols)
        .map(|_| (rng.next() % 126) as u8 | ((rng.next() % 2) as u8) << 7)
        .collect();
    let scales: Vec<u8> = (0..(rows / 32) * (cols / 32))
        .map(|_| 120 + (rng.next() % 8) as u8)
        .collect();
    let decoded: Vec<f32> = (0..rows * cols)
        .map(|i| {
            decode_f8_e4m3fn(codes[i])
                * decode_f8_e8m0fnu(scales[(i / cols / 32) * (cols / 32) + (i % cols) / 32])
        })
        .collect();
    let grouped_input: Vec<f32> = (0..2 * cols).map(|_| rng.bf16(1.0)).collect();
    let cpu = MatrixRef::new(rows, cols, &decoded)?.multiply_grouped(&grouped_input, 2, true)?;
    let host = v41_dense_host(
        V41DenseHost::Fp8Block {
            codes: &codes,
            scales: &scales,
        },
        &grouped_input,
        rows,
        cols,
        2,
        false,
        V41Output::F32,
    );
    assert_equal(&host, &cpu, "grouped FP8");
    // RMS norm.
    let x: Vec<f32> = (0..512).map(|_| rng.unit() * 7.0).collect();
    let w: Vec<f32> = (0..512).map(|_| 1.0 + rng.unit() * 0.3).collect();
    let cpu = round_bf16(&rms_norm(&x, &round_bf16(&w)?, 1e-20)?)?;
    assert_equal(
        &v41_rms_norm_host(&x, &w, 1e-20, V41Output::Bf16),
        &cpu,
        "RMS norm",
    );
    // Compressors, ratio 1 exactly and ratio 2 within the bound.
    for ratio in [1, 2] {
        let width = 64;
        let compressor = V41Compressor {
            ratio,
            width,
            epsilon: 1e-20,
        };
        let tokens = 9;
        let values: Vec<f32> = (0..tokens * width).map(|_| rng.unit() * 2.0).collect();
        let gates: Vec<f32> = (0..tokens * width).map(|_| rng.unit() * 4.0).collect();
        let norm: Vec<f32> = (0..width).map(|_| 1.0 + rng.unit() * 0.2).collect();
        let gate = (ratio > 1).then_some(gates.as_slice());
        let mut host = V41CompressorHost::new(compressor, f32::NAN);
        let positions: Vec<i32> = (0..tokens as i32).collect();
        let latents = host.push(&values, gate, &norm, &positions);
        let mut cpu =
            CompressionState::new(ratio, width)?.with_arithmetic(Arithmetic::ReferenceBf16);
        let expected = cpu.push_chunk(0, &values, gate, &norm, 1e-20)?;
        let produced: Vec<_> = latents.into_iter().flatten().collect();
        assert_eq!(produced.len(), expected.len());
        for (k, (h, c)) in produced.iter().zip(&expected).enumerate() {
            if ratio == 1 {
                assert_equal(h, c, "ratio-1 latent");
                continue;
            }
            let t = 2 * k;
            let v: [&[f32]; 2] = [
                &values[t * width..(t + 1) * width],
                &values[(t + 1) * width..(t + 2) * width],
            ];
            let gs: [&[f32]; 2] = [
                &gates[t * width..(t + 1) * width],
                &gates[(t + 1) * width..(t + 2) * width],
            ];
            check_pooled_latent(h, c, &v, &gs, &norm, 1e-20, "host latent")?;
        }
    }
    // Attention.
    let g = small(2);
    for slots in [1, 63, 64, 65, 200] {
        let kv: Vec<f32> = (0..slots * 512).map(|_| rng.bf16(1.0)).collect();
        let indices: Vec<i32> = (0..slots as i32)
            .map(|i| if i % 7 == 3 { -1 } else { i })
            .collect();
        let query: Vec<f32> = (0..2 * 512).map(|_| rng.bf16(0.2)).collect();
        let sinks = [0.5, -3.0];
        let (cpu, values, bound) = cpu_attention(g, &query, &sinks, &kv, &indices)?;
        let host = v41_sparse_attention_host(g, &query, &sinks, &[], &kv, &indices, slots);
        assert_bf16(&host, &values, &bound, "host attention");
        let _ = cpu;
    }
    Ok(())
}

/// On real inputs, the host models agree with the CPU path and the CPU path
/// with the oracle: the compressor latents from the oracle's attention inputs
/// at layers 2 and 20, and the selections and candidate blocks from the
/// oracle's own scores.
#[test]
fn real_inputs_cpu_and_host_models_agree() -> TestResult {
    let rows = Rows::load()?;
    let weights = FxWeights::from_rows(&rows);
    let mut latents = 0;
    let mut worst = 0.0_f64;
    for layer in [2_usize, 20] {
        let ratio = compress_ratio(layer)?;
        let p = projections(&weights, layer)?;
        let norm = p.compressor_norm()?;
        for (prompt, step, position) in oracle_steps() {
            let label = format!("{prompt}.decode.{step}.layers.{layer}.attn.compressor.latent");
            if !rows.has(&label) {
                continue;
            }
            let Some(members) = oracle_group(&rows, prompt, position, layer, ratio) else {
                continue;
            };
            let mut values = Vec::new();
            let mut gates = Vec::new();
            for m in &members {
                let (v, g) = p.compressor_inputs(m)?;
                values.extend(v);
                gates.extend(g.unwrap_or_default());
            }
            let mut cpu =
                CompressionState::new(ratio, 512)?.with_arithmetic(Arithmetic::ReferenceBf16);
            let gate = (ratio > 1).then_some(gates.as_slice());
            let latent = cpu.push_chunk(0, &values, gate, &norm, 1e-20)?;
            let latent = latent.last().ok_or("latent")?;
            let policy = policy_bf16(latent, &rows.f32(&label));
            assert!(policy <= 1.0, "{label}: {policy}");
            worst = worst.max(policy);
            let mut host = V41CompressorHost::new(
                V41Compressor {
                    ratio,
                    width: 512,
                    epsilon: 1e-20,
                },
                f32::NAN,
            );
            let positions: Vec<i32> = (position + 1 - ratio..=position)
                .map(|q| q as i32)
                .collect();
            let host_latent = host.push(&values, gate, &norm, &positions);
            let host_latent = host_latent.last().and_then(|r| r.as_ref()).ok_or("host")?;
            if ratio == 1 {
                assert_equal(host_latent, latent, &label);
            } else {
                let v: Vec<&[f32]> = values.chunks_exact(512).collect();
                let g: Vec<&[f32]> = gates.chunks_exact(512).collect();
                check_pooled_latent(host_latent, latent, &v, &g, &norm, 1e-20, &label)?;
            }
            latents += 1;
        }
    }
    assert!(latents >= 10, "{latents} latents");
    let mut selections = 0;
    let mut ties = 0;
    for (prompt, step, position) in oracle_steps() {
        let base = format!("{prompt}.decode.{step}.layers");
        let score20 = format!("{base}.20.attn.indexer.score");
        if !rows.has(&score20) {
            continue;
        }
        let scores = rows.f32(&score20);
        let (_, flags, kept) = v41_candidate_blocks_host(&scores, 2048, 8);
        let mask = candidate_mask(&scores, scores.len(), 2048, 8)?;
        let candidates = format!("{base}.20.attn.indexer.candidate_blocks");
        if rows.has(&candidates) {
            let block_max: Vec<f32> = (0..scores.len().div_ceil(8))
                .map(|b| {
                    if b == position / 8 {
                        f32::INFINITY
                    } else {
                        scores[b * 8..((b + 1) * 8).min(scores.len())]
                            .iter()
                            .fold(f32::NEG_INFINITY, |a, v| a.max(*v))
                    }
                })
                .collect();
            ties += compare_with_oracle(&kept, &rows.ints(&candidates), 0, &block_max, &candidates);
        }
        for layer in V41_INDEX_SOURCES {
            let label = format!("{base}.{layer}.attn.indexer.score");
            let topk = format!("{base}.{layer}.attn.indexer.topk_idxs");
            if !rows.has(&label) || !rows.has(&topk) {
                continue;
            }
            let scores = rows.f32(&label);
            let ratio = compress_ratio(layer)?;
            assert_eq!(scores.len(), (position + 1) / ratio, "{label}");
            let flags = (layer > 20).then_some((flags.as_slice(), 8));
            let host = v41_select_host(&scores, flags, 512, false);
            assert_eq!(
                host,
                cpu_select(&scores, 512, (layer > 20).then_some(mask.as_slice()))?,
                "{label}"
            );
            ties += compare_with_oracle(
                &host,
                &rows.ints(&topk),
                topk_offset(step, position),
                &scores,
                &label,
            );
            selections += 1;
        }
    }
    assert!(selections >= 100, "{selections} selections");
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "cpu_real", "latents": latents, "latent_policy_ratio": worst,
               "selections": selections, "tied_differences": ties})
    );
    Ok(())
}

/// Checks a ratio-2 latent against the CPU's: exact when no pooled value's
/// bound reaches a BF16 boundary, else within the fallback bound. Returns
/// `(fallback used, latent differs from the CPU's)`.
fn check_pooled_latent(
    device: &[f32],
    cpu: &[f32],
    values: &[&[f32]],
    gates: &[&[f32]],
    norm: &[f32],
    epsilon: f32,
    what: &str,
) -> Result<(bool, bool), CpuError> {
    let (pooled, bound) = cpu_pool_bound(values, gates);
    let reach: Vec<bool> = pooled
        .iter()
        .zip(&bound)
        .map(|(p, b)| reaches_boundary(*p, *b))
        .collect();
    if !reach.contains(&true) {
        assert_equal(device, cpu, what);
        return Ok((false, false));
    }
    let x = round_bf16(&pooled)?;
    let dx: Vec<f64> = x
        .iter()
        .zip(&reach)
        .map(|(v, r)| {
            if *r {
                // One BF16 step at this magnitude.
                f64::from(v.abs()).max(f64::from(f32::MIN_POSITIVE)) * 2.0_f64.powi(-7)
            } else {
                0.0
            }
        })
        .collect();
    let (out, latent_bound) = rms_bound(&x, &dx, norm, epsilon)?;
    assert_bf16(device, &out, &latent_bound, what);
    let differs = device.iter().zip(cpu).any(|(a, b)| !same_value(*a, *b));
    Ok((true, differs))
}

// ---------------------------------------------------------------------------
// Mode 1: RoPE and YaRN
// ---------------------------------------------------------------------------

fn device_rope(
    backend: &mut V41Metal,
    table: &V41Buffer,
    dim: usize,
    values: &[f32],
    positions: &[i32],
    heads: usize,
    head_width: usize,
    back: usize,
    inverse: bool,
    output: V41Output,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let buffer = upload_f32(backend, values)?;
    let positions_buffer = upload_i32(backend, positions)?;
    let mut submission = backend.begin_submission();
    submission.v41_rope(
        V41RopeTable {
            table,
            dim,
            positions: V41_MAX_POSITION,
        },
        &buffer,
        V41Steps {
            positions: &positions_buffer,
            tokens: positions.len(),
        },
        heads,
        head_width,
        back,
        inverse,
        output,
    )?;
    submission.commit()?;
    Ok(read_f32(&buffer, values.len())?)
}

#[test]
fn rope_matches_the_cpu() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(21);
    let mut worst = 0.0_f64;
    let mut boundary = 0;
    let mut cases = 0;
    for config in [V41RopeConfig::window(64), V41RopeConfig::compressed(64)] {
        let (frequencies, table) = rope_table(&mut backend, config)?;
        let rope = cpu_rope(config)?;
        // (heads, head width): the query, the window KV, the index query.
        for (heads, head_width) in [(64, 512), (1, 512), (32, 128)] {
            let positions: Vec<i32> = vec![
                0, 1, 2, 127, 128, 129, 4095, 16_383, 16_384, 20_000, 65_535, 65_536, 65_537,
                1_048_575,
            ];
            for back in [0, 1] {
                for inverse in [false, true] {
                    let n = heads * head_width;
                    let values: Vec<f32> =
                        (0..positions.len() * n).map(|_| rng.bf16(2.0)).collect();
                    for output in [V41Output::F32, V41Output::Bf16] {
                        let device = device_rope(
                            &mut backend,
                            &table,
                            64,
                            &values,
                            &positions,
                            heads,
                            head_width,
                            back,
                            inverse,
                            output,
                        )?;
                        for (t, &p) in positions.iter().enumerate() {
                            let position = p as i64 - back as i64;
                            let mut host = values[t * n..(t + 1) * n].to_vec();
                            v41_rope_host(
                                &frequencies,
                                &mut host,
                                head_width,
                                position,
                                inverse,
                                output,
                            );
                            let got = &device[t * n..(t + 1) * n];
                            assert_host(got, &host, "RoPE host");
                            if position < 0 {
                                assert!(got.iter().all(|v| v.is_nan()));
                                continue;
                            }
                            for h in 0..heads {
                                let head =
                                    &values[t * n + h * head_width..t * n + (h + 1) * head_width];
                                let (cpu, bound) =
                                    cpu_rope_bound(&rope, head, position as usize, inverse)?;
                                let got = &got[h * head_width..(h + 1) * head_width];
                                if output == V41Output::F32 {
                                    worst = worst.max(assert_bound(got, &cpu, &bound, "RoPE"));
                                } else {
                                    boundary += assert_bf16(got, &cpu, &bound, "RoPE BF16");
                                }
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        // Out-of-range positions and NaN.
        let mut values: Vec<f32> = (0..512).map(|_| rng.bf16(1.0)).collect();
        let device = device_rope(
            &mut backend,
            &table,
            64,
            &values,
            &[1_048_576],
            1,
            512,
            0,
            false,
            V41Output::F32,
        )?;
        assert!(device.iter().all(|v| v.is_nan()));
        assert!(
            rope.apply_tail(&mut values.clone(), 512, 1_048_576, false)
                .is_err()
        );
        values[500] = f32::NAN;
        let device = device_rope(
            &mut backend,
            &table,
            64,
            &values,
            &[5],
            1,
            512,
            0,
            false,
            V41Output::F32,
        )?;
        assert!(device[500].is_nan() || device[501].is_nan());
    }
    // Real input: layer 20's oracle latent rotated at its own position
    // (ratio 1), as the compressed KV form is.
    let rows = Rows::load()?;
    let config = V41RopeConfig::compressed(64);
    let (_, table) = rope_table(&mut backend, config)?;
    let rope = cpu_rope(config)?;
    let mut real = 0;
    for (prompt, step, position) in oracle_steps() {
        let label = format!("{prompt}.decode.{step}.layers.20.attn.compressor.latent");
        if !rows.has(&label) {
            continue;
        }
        let latent = rows.f32(&label);
        let device = device_rope(
            &mut backend,
            &table,
            64,
            &latent,
            &[position as i32],
            1,
            512,
            0,
            false,
            V41Output::F32,
        )?;
        let (cpu, bound) = cpu_rope_bound(&rope, &latent, position, false)?;
        worst = worst.max(assert_bound(&device, &cpu, &bound, &label));
        real += 1;
    }
    assert!(real >= 10, "{real} real latents");
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "rope", "cases": cases, "real_latents": real,
               "max_error_over_bound": worst, "bf16_boundary_values": boundary})
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Cache quantizers
// ---------------------------------------------------------------------------

#[test]
fn cache_quantizers_match_the_cpu() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(23);
    for (kind, cpu_kind) in [
        (V41CacheQuant::Fp8, Quantization::Fp8),
        (V41CacheQuant::IndexFp4, Quantization::IndexFp4),
        (V41CacheQuant::CompressedFp4, Quantization::CompressedFp4),
    ] {
        let blocks = quant_blocks(kind, &mut rng);
        // The GPU reads a subnormal input as zero, and none reaches it from
        // another kernel, so the inputs are flushed for both sides.
        let input: Vec<f32> = blocks.iter().flatten().map(|v| ftz(*v)).collect();
        let width = kind.block() * 2;
        let rows = input.len() / width;
        let input = &input[..rows * width];
        let in_buffer = upload_f32(&mut backend, input)?;
        let out = f32_buffer(&mut backend, input.len())?;
        let mut submission = backend.begin_submission();
        submission.v41_cache_quant(kind, &in_buffer, &out, rows, width)?;
        submission.commit()?;
        let device = read_f32(&out, input.len())?;
        assert_host(
            &device,
            &v41_cache_quant_host(kind, input),
            "quantizer host",
        );
        for (b, block) in input.chunks_exact(kind.block()).enumerate() {
            let got = &device[b * kind.block()..(b + 1) * kind.block()];
            match inplace_bf16(block, cpu_kind) {
                Ok(cpu) => assert_equal(got, &cpu, &format!("{kind:?} block {b}")),
                Err(_) => assert!(got.iter().all(|v| v.is_nan())),
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Mode 2: window attention
// ---------------------------------------------------------------------------

/// Window KV rows in the cache form, as the CPU's `cache_form(Fp8)`.
fn window_rows(rng: &mut Rng, count: usize) -> Result<(Vec<f32>, Vec<f32>), CpuError> {
    let raw: Vec<f32> = (0..count * 512).map(|_| rng.bf16(1.0)).collect();
    let mut cached = Vec::with_capacity(raw.len());
    for row in raw.chunks_exact(512) {
        cached.extend(inplace_bf16(row, Quantization::Fp8)?);
    }
    Ok((raw, cached))
}

#[test]
fn window_attention_matches_the_cpu() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(31);
    let g = small(4);
    let d = g.head_dim;
    let sinks = [0.0_f32, 2.5, -4.0, 40.0];
    let sinks_buffer = upload_f32(&mut backend, &sinks)?;
    let total = 301;
    let (raw, cached) = window_rows(&mut rng, total)?;
    let raw_buffer = upload_f32(&mut backend, &raw)?;
    let queries: Vec<f32> = (0..total * g.heads * d).map(|_| rng.bf16(0.1)).collect();
    let query_buffer = upload_f32(&mut backend, &queries)?;
    let mut worst_boundary = 0;
    let mut checked = 0;
    let cpu_window = |p: usize| -> Vec<f32> {
        let first = (p + 1).saturating_sub(g.window);
        cached[first * d..(p + 1) * d].to_vec()
    };

    // Prefill: every position in one store and one attention launch, with a
    // ring long enough for the chunk.
    let ring_rows = g.window + total - 1;
    let ring = zeros(&mut backend, ring_rows * d * 2)?;
    let positions: Vec<i32> = (0..total as i32).collect();
    let positions_buffer = upload_i32(&mut backend, &positions)?;
    let out = f32_buffer(&mut backend, total * g.heads * d)?;
    let steps = V41Steps {
        positions: &positions_buffer,
        tokens: total,
    };
    let mut submission = backend.begin_submission();
    submission.v41_window_store(
        g,
        &raw_buffer,
        steps,
        V41Ring {
            buffer: &ring,
            rows: ring_rows,
        },
    )?;
    submission.v41_sparse_attention(
        g,
        &query_buffer,
        &sinks_buffer,
        V41Ring {
            buffer: &ring,
            rows: ring_rows,
        },
        None,
        steps,
        &out,
    )?;
    submission.commit()?;
    let stored = read_bf16(&ring, total * d)?;
    assert_equal(&stored, &cached, "window store");
    let device = read_f32(&out, total * g.heads * d)?;
    for p in 0..total {
        let kv = cpu_window(p);
        let n = kv.len() / d;
        let indices: Vec<i32> = (0..n as i32).collect();
        let q = &queries[p * g.heads * d..(p + 1) * g.heads * d];
        let got = &device[p * g.heads * d..(p + 1) * g.heads * d];
        let host = v41_sparse_attention_host(g, q, &sinks, &kv, &[], &[], 0);
        assert_host(got, &host, &format!("prefill host {p}"));
        let (_, values, bound) = cpu_attention(g, q, &sinks, &kv, &indices)?;
        worst_boundary += assert_bf16(got, &values, &bound, &format!("prefill {p}"));
        checked += 1;
    }

    // Decode: one token a step into a 128-row ring, which wraps after 128.
    let ring = zeros(&mut backend, g.window * d * 2)?;
    let mut position_buffer = upload_i32(&mut backend, &[0])?;
    let one = f32_buffer(&mut backend, g.heads * d)?;
    let row_buffer = f32_buffer(&mut backend, d)?;
    let query_one = f32_buffer(&mut backend, g.heads * d)?;
    let checks = [
        0, 1, 2, 63, 64, 65, 126, 127, 128, 129, 200, 255, 256, 257, 300,
    ];
    let mut row_in = row_buffer;
    let mut q_in = query_one;
    for p in 0..total {
        position_buffer.write_bytes(&(p as i32).to_le_bytes())?;
        row_in.write_bytes(
            &raw[p * d..(p + 1) * d]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        )?;
        let q = &queries[p * g.heads * d..(p + 1) * g.heads * d];
        q_in.write_bytes(&q.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>())?;
        let steps = V41Steps {
            positions: &position_buffer,
            tokens: 1,
        };
        let mut submission = backend.begin_submission();
        submission.v41_window_store(
            g,
            &row_in,
            steps,
            V41Ring {
                buffer: &ring,
                rows: g.window,
            },
        )?;
        submission.v41_sparse_attention(
            g,
            &q_in,
            &sinks_buffer,
            V41Ring {
                buffer: &ring,
                rows: g.window,
            },
            None,
            steps,
            &one,
        )?;
        submission.commit()?;
        if !checks.contains(&p) {
            continue;
        }
        let got = read_f32(&one, g.heads * d)?;
        let kv = cpu_window(p);
        let n = kv.len() / d;
        assert_eq!(n, (p + 1).min(128), "window rows at {p}");
        let host = v41_sparse_attention_host(g, q, &sinks, &kv, &[], &[], 0);
        assert_host(&got, &host, &format!("decode host {p}"));
        let indices: Vec<i32> = (0..n as i32).collect();
        let (_, values, bound) = cpu_attention(g, q, &sinks, &kv, &indices)?;
        worst_boundary += assert_bf16(&got, &values, &bound, &format!("decode {p}"));
        checked += 1;
        let ring_now = read_bf16(&ring, g.window * d)?;
        assert_equal(
            &ring_now[(p % 128) * d..(p % 128 + 1) * d],
            &cached[p * d..(p + 1) * d],
            "ring slot",
        );
    }

    // A sink that dominates, NaN queries, and a negative position.
    let q: Vec<f32> = (0..g.heads * d).map(|_| rng.bf16(0.1)).collect();
    let mut nan_q = q.clone();
    nan_q[d + 7] = f32::NAN;
    q_in.write_bytes(
        &nan_q
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )?;
    position_buffer.write_bytes(&(300_i32).to_le_bytes())?;
    let mut submission = backend.begin_submission();
    submission.v41_sparse_attention(
        g,
        &q_in,
        &sinks_buffer,
        V41Ring {
            buffer: &ring,
            rows: g.window,
        },
        None,
        V41Steps {
            positions: &position_buffer,
            tokens: 1,
        },
        &one,
    )?;
    submission.commit()?;
    let got = read_f32(&one, g.heads * d)?;
    assert!(got[d..2 * d].iter().all(|v| v.is_nan()), "NaN query head");
    assert!(
        got[..d].iter().all(|v| v.is_finite()),
        "other heads stay finite"
    );
    assert!(
        sparse_attention_bf16(
            &nan_q,
            g.heads,
            d,
            &cpu_window(300),
            &(0..128).collect::<Vec<_>>(),
            &sinks,
            0.04
        )
        .is_err()
    );
    position_buffer.write_bytes(&(-1_i32).to_le_bytes())?;
    let mut submission = backend.begin_submission();
    submission.v41_sparse_attention(
        g,
        &q_in,
        &sinks_buffer,
        V41Ring {
            buffer: &ring,
            rows: g.window,
        },
        None,
        V41Steps {
            positions: &position_buffer,
            tokens: 1,
        },
        &one,
    )?;
    submission.commit()?;
    assert!(read_f32(&one, g.heads * d)?.iter().all(|v| v.is_nan()));
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "window", "tokens_checked": checked, "bf16_boundary_values": worst_boundary})
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Mode 3: compression and compressed attention
// ---------------------------------------------------------------------------

/// Runs the compressor over `chunks` of `values` on the device, one
/// submission per chunk; returns every token's latent row (NaN where the
/// group is incomplete).
#[allow(clippy::too_many_arguments)]
fn device_compress(
    backend: &mut V41Metal,
    compressor: V41Compressor,
    values: &[f32],
    gates: Option<&[f32]>,
    norm: &[f32],
    start: usize,
    chunks: &[usize],
) -> Result<Vec<f32>, Box<dyn Error>> {
    let w = compressor.width;
    let norm_buffer = upload_f32(backend, norm)?;
    let state = filled_f32(backend, compressor.state_floats(), f32::NAN)?;
    let mut out = Vec::new();
    let mut offset = 0;
    for &count in chunks {
        let v = upload_f32(backend, &values[offset * w..(offset + count) * w])?;
        let g = match gates {
            Some(gates) => Some(upload_f32(
                backend,
                &gates[offset * w..(offset + count) * w],
            )?),
            None => None,
        };
        let positions: Vec<i32> = (0..count).map(|i| (start + offset + i) as i32).collect();
        let positions_buffer = upload_i32(backend, &positions)?;
        let latents = filled_f32(backend, count * w, f32::NAN)?;
        let mut submission = backend.begin_submission();
        submission.v41_compress(
            compressor,
            &v,
            g.as_ref(),
            &norm_buffer,
            &state,
            V41Steps {
                positions: &positions_buffer,
                tokens: count,
            },
            &latents,
        )?;
        submission.commit()?;
        out.extend(read_f32(&latents, count * w)?);
        offset += count;
    }
    Ok(out)
}

#[test]
fn compressor_chunks_and_tails_match_the_cpu() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(41);
    let width = 512;
    let mut fallbacks = 0;
    let mut differing = 0;
    let mut latents = 0;
    for ratio in [1, 2] {
        let compressor = V41Compressor {
            ratio,
            width,
            epsilon: 1e-20,
        };
        let tokens = 11;
        let values: Vec<f32> = (0..tokens * width).map(|_| rng.unit() * 3.0).collect();
        let gates: Vec<f32> = (0..tokens * width).map(|_| rng.unit() * 6.0).collect();
        let norm: Vec<f32> = (0..width).map(|_| 1.0 + rng.unit() * 0.3).collect();
        let gate = (ratio > 1).then_some(gates.as_slice());
        let mut cpu =
            CompressionState::new(ratio, width)?.with_arithmetic(Arithmetic::ReferenceBf16);
        let expected = cpu.push_chunk(0, &values, gate, &norm, 1e-20)?;
        assert_eq!(cpu.pending_tokens(), tokens % ratio);
        // Whole, token by token, and odd chunks including a chunk inside a
        // group: all give the same latents.
        let mut runs = Vec::new();
        for chunks in [vec![11], vec![1; 11], vec![3, 1, 2, 1, 4], vec![2, 5, 4]] {
            let device =
                device_compress(&mut backend, compressor, &values, gate, &norm, 0, &chunks)?;
            let mut host = V41CompressorHost::new(compressor, f32::NAN);
            let mut offset = 0;
            let mut host_rows = Vec::new();
            for &count in &chunks {
                let positions: Vec<i32> = (offset..offset + count).map(|p| p as i32).collect();
                let v = &values[offset * width..(offset + count) * width];
                let g = gate.map(|g| &g[offset * width..(offset + count) * width]);
                for row in host.push(v, g, &norm, &positions) {
                    host_rows.extend(row.unwrap_or_else(|| vec![f32::NAN; width]));
                }
                offset += count;
            }
            assert_host(
                &device,
                &host_rows,
                &format!("ratio {ratio} chunks {chunks:?}"),
            );
            runs.push(device);
        }
        for run in &runs[1..] {
            assert_host(run, &runs[0], "chunk replay");
        }
        let device = &runs[0];
        let mut k = 0;
        for t in 0..tokens {
            let row = &device[t * width..(t + 1) * width];
            if (t + 1) % ratio != 0 {
                assert!(
                    row.iter().all(|v| v.is_nan()),
                    "incomplete group writes nothing"
                );
                continue;
            }
            if ratio == 1 {
                assert_equal(row, &expected[k], "ratio-1 latent");
            } else {
                let v: Vec<&[f32]> = (0..2)
                    .map(|r| &values[(t - 1 + r) * width..(t + r) * width])
                    .collect();
                let g: Vec<&[f32]> = (0..2)
                    .map(|r| &gates[(t - 1 + r) * width..(t + r) * width])
                    .collect();
                let (fallback, differs) =
                    check_pooled_latent(row, &expected[k], &v, &g, &norm, 1e-20, "ratio-2 latent")?;
                fallbacks += usize::from(fallback);
                differing += usize::from(differs);
            }
            k += 1;
            latents += 1;
        }
    }
    // NaN input gives a NaN latent; the CPU refuses it.
    let compressor = V41Compressor {
        ratio: 2,
        width,
        epsilon: 1e-20,
    };
    let mut values: Vec<f32> = (0..2 * width).map(|_| rng.unit()).collect();
    values[width + 3] = f32::NAN;
    let gates: Vec<f32> = (0..2 * width).map(|_| rng.unit()).collect();
    let norm = vec![1.0; width];
    let device = device_compress(
        &mut backend,
        compressor,
        &values,
        Some(&gates),
        &norm,
        0,
        &[2],
    )?;
    assert!(device[width..].iter().all(|v| v.is_nan()));
    let mut cpu = CompressionState::new(2, width)?.with_arithmetic(Arithmetic::ReferenceBf16);
    assert!(
        cpu.push_chunk(0, &values, Some(&gates), &norm, 1e-20)
            .is_err()
    );
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "compressor_synthetic", "latents": latents, "ratio2_fallback_bounds": fallbacks,
               "ratio2_latents_differing": differing})
    );
    Ok(())
}

/// The compressor input rows of a group from the oracle, oldest first, and
/// the group's last position; `None` where a member isn't recorded.
fn oracle_group(
    rows: &Rows,
    prompt: &str,
    position: usize,
    layer: usize,
    ratio: usize,
) -> Option<Vec<Vec<f32>>> {
    let mut members = Vec::new();
    for p in position + 1 - ratio..=position {
        let (_, step, _) = oracle_steps()
            .into_iter()
            .find(|(pr, _, q)| *pr == prompt && *q == p)?;
        let label = format!("{prompt}.decode.{step}.layers.{layer}.attn.in");
        if !rows.has(&label) {
            return None;
        }
        members.push(rows.f32(&label));
    }
    Some(members)
}

#[test]
fn compressor_matches_the_cpu_on_real_inputs() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let weights = FxWeights::from_rows(&rows);
    let g = V41AttentionGeometry::flash();
    let config = V41RopeConfig::compressed(64);
    let (frequencies, table) = rope_table(&mut backend, config)?;
    let rope = cpu_rope(config)?;
    let mut report = Vec::new();
    for layer in [2_usize, 20] {
        let ratio = compress_ratio(layer)?;
        let p = projections(&weights, layer)?;
        let name = |part: &str| format!("layers.{layer}.attn.{part}");
        // The compressor weights on the device: F32 for ratio 2 (as
        // convert.py promotes them), BF16 for ratio 1.
        let wkv_values = rows.f32(&name("compressor.wkv.weight"));
        let (wkv, wgate, format) = if ratio > 1 {
            (
                upload_f32(&mut backend, &wkv_values)?,
                Some(upload_f32(
                    &mut backend,
                    &rows.f32(&name("compressor.wgate.weight")),
                )?),
                V41DenseFormat::F32,
            )
        } else {
            (
                upload_u16(&mut backend, &rows.words(&name("compressor.wkv.weight")))?,
                None,
                V41DenseFormat::Bf16,
            )
        };
        let norm = p.compressor_norm()?;
        let norm_buffer = upload_f32(&mut backend, &norm)?;
        let wk = rows.words(&name("indexer.wk.weight"));
        let wk_buffer = upload_u16(&mut backend, &wk)?;
        let k_norm = rows.f32(&name("indexer.k_norm.weight"));
        let k_norm_buffer = upload_f32(&mut backend, &k_norm)?;
        let wp = rows.words(&name("indexer.weights_proj.weight"));
        let wp_buffer = upload_u16(&mut backend, &wp)?;
        let mut cases = 0;
        let mut worst_policy = 0.0_f64;
        let mut fallback = 0;
        let mut differing = 0;
        let mut reached_key_blocks = 0;
        let mut reached_kv_blocks = 0;
        for (prompt, step, position) in oracle_steps() {
            let latent_label =
                format!("{prompt}.decode.{step}.layers.{layer}.attn.compressor.latent");
            if !rows.has(&latent_label) {
                continue;
            }
            let Some(members) = oracle_group(&rows, prompt, position, layer, ratio) else {
                eprintln!("skip {latent_label}: a group member's input isn't recorded");
                continue;
            };
            let x: Vec<f32> = members.iter().flatten().copied().collect();
            let x_buffer = upload_f32(&mut backend, &x)?;
            let tokens = ratio;
            let values = f32_buffer(&mut backend, tokens * 512)?;
            let gates = f32_buffer(&mut backend, tokens * 512)?;
            let index_weights = f32_buffer(&mut backend, tokens * 32)?;
            let state = filled_f32(&mut backend, 2 * ratio * 512, f32::NAN)?;
            let latents = filled_f32(&mut backend, tokens * 512, f32::NAN)?;
            let positions: Vec<i32> = (position + 1 - ratio..=position)
                .map(|q| q as i32)
                .collect();
            let positions_buffer = upload_i32(&mut backend, &positions)?;
            let kv = zeros(&mut backend, (position / ratio + 1) * 512 * 2)?;
            let keys = zeros(&mut backend, (position / ratio + 1) * 128 * 2)?;
            let source = V41Source {
                kv: &kv,
                index_keys: &keys,
                capacity: position / ratio + 1,
            };
            let steps = V41Steps {
                positions: &positions_buffer,
                tokens,
            };
            fn dense(w: &V41Buffer, format: V41DenseFormat) -> V41Dense<'_> {
                V41Dense {
                    weights: w,
                    scales: None,
                    format,
                    rows: 512,
                    cols: 5120,
                    groups: 1,
                }
            }
            let (round_input, output) = if ratio > 1 {
                (false, V41Output::F32)
            } else {
                (true, V41Output::Bf16)
            };
            let mut submission = backend.begin_submission();
            submission.v41_dense(
                dense(&wkv, format),
                &x_buffer,
                &values,
                tokens,
                round_input,
                output,
            )?;
            if let Some(wgate) = &wgate {
                submission.v41_dense(
                    dense(wgate, format),
                    &x_buffer,
                    &gates,
                    tokens,
                    round_input,
                    output,
                )?;
            }
            submission.v41_dense(
                V41Dense {
                    weights: &wp_buffer,
                    scales: None,
                    format: V41DenseFormat::Bf16,
                    rows: 32,
                    cols: 5120,
                    groups: 1,
                },
                &x_buffer,
                &index_weights,
                tokens,
                true,
                V41Output::Bf16,
            )?;
            submission.v41_compress(
                V41Compressor {
                    ratio,
                    width: 512,
                    epsilon: g.norm_epsilon,
                },
                &values,
                (ratio > 1).then_some(&gates),
                &norm_buffer,
                &state,
                steps,
                &latents,
            )?;
            submission.v41_compressed_store(
                g,
                ratio,
                V41IndexKeyWeights {
                    wk: &wk_buffer,
                    k_norm: &k_norm_buffer,
                },
                V41RopeTable {
                    table: &table,
                    dim: 64,
                    positions: V41_MAX_POSITION,
                },
                &latents,
                steps,
                source,
            )?;
            submission.commit()?;

            // Projections: equal to the CPU's.
            let device_values = read_f32(&values, tokens * 512)?;
            let device_gates = read_f32(&gates, tokens * 512)?;
            let mut cpu_values = Vec::new();
            let mut cpu_gates = Vec::new();
            for m in &members {
                let (v, gate) = p.compressor_inputs(m)?;
                cpu_values.extend(v);
                cpu_gates.extend(gate.unwrap_or_default());
                assert_equal(
                    &read_f32(&index_weights, tokens * 32)?
                        [cpu_values.len() / 512 * 32 - 32..cpu_values.len() / 512 * 32],
                    &p.index_weights(m)?,
                    "index weights",
                );
            }
            assert_equal(&device_values, &cpu_values, "compressor values");
            if ratio > 1 {
                assert_equal(&device_gates, &cpu_gates, "compressor gates");
            }

            // The latent: the host model exactly, the CPU's compressor, and
            // the oracle's within the W1 policy.
            let latent = read_f32(&latents, tokens * 512)?[(tokens - 1) * 512..].to_vec();
            let mut host = V41CompressorHost::new(
                V41Compressor {
                    ratio,
                    width: 512,
                    epsilon: g.norm_epsilon,
                },
                f32::NAN,
            );
            let host_rows = host.push(
                &device_values,
                (ratio > 1).then_some(device_gates.as_slice()),
                &norm,
                &positions,
            );
            assert_host(
                &latent,
                host_rows
                    .last()
                    .and_then(|r| r.as_ref())
                    .ok_or("host latent")?,
                "latent host",
            );
            let mut cpu =
                CompressionState::new(ratio, 512)?.with_arithmetic(Arithmetic::ReferenceBf16);
            let expected = cpu.push_chunk(
                0,
                &cpu_values,
                (ratio > 1).then_some(cpu_gates.as_slice()),
                &norm,
                g.norm_epsilon,
            )?;
            let cpu_latent = expected.last().ok_or("CPU latent")?;
            if ratio == 1 {
                assert_equal(&latent, cpu_latent, "latent");
            } else {
                let v: Vec<&[f32]> = cpu_values.chunks_exact(512).collect();
                let gs: Vec<&[f32]> = cpu_gates.chunks_exact(512).collect();
                let (used, differs) = check_pooled_latent(
                    &latent,
                    cpu_latent,
                    &v,
                    &gs,
                    &norm,
                    g.norm_epsilon,
                    "real latent",
                )?;
                fallback += usize::from(used);
                differing += usize::from(differs);
            }
            let oracle = rows.f32(&latent_label);
            let policy = policy_bf16(&latent, &oracle);
            assert!(policy <= 1.0, "{latent_label}: W1 policy ratio {policy}");
            worst_policy = worst_policy.max(policy);

            // The index key and compressed KV rows: the host model exactly,
            // the CPU chain block by block.
            let row = (position + 1) / ratio - 1;
            let device_key = read_bf16(&keys, (row + 1) * 128)?[row * 128..].to_vec();
            let device_kv = read_bf16(&kv, (row + 1) * 512)?[row * 512..].to_vec();
            let (host_key, host_kv) =
                v41_compressed_store_host(g, ratio, &wk, &k_norm, &frequencies, &latent, position);
            assert_host(&device_key, &host_key, "index key host");
            assert_host(&device_kv, &host_kv, "compressed KV host");
            let group_position = position + 1 - ratio;
            let key = p.index_key(&latent)?;
            let (rotated, bound) = cpu_rope_bound(&rope, &key, group_position, false)?;
            reached_key_blocks += assert_rotated_blocks(
                &device_key,
                &rotated,
                &bound,
                Quantization::IndexFp4,
                32,
                "index key",
            )?;
            let (rotated, bound) = cpu_rope_bound(&rope, &latent, group_position, false)?;
            reached_kv_blocks += assert_rotated_blocks(
                &device_kv,
                &rotated,
                &bound,
                Quantization::CompressedFp4,
                16,
                "compressed KV",
            )?;
            cases += 1;
        }
        assert!(cases >= 3, "layer {layer}: {cases} real groups");
        report.push(json!({"layer": layer, "ratio": ratio, "groups": cases,
            "oracle_policy_ratio": worst_policy, "fallback_bounds": fallback,
            "latents_differing": differing,
            "index_key_blocks_at_rope_boundary": reached_key_blocks,
            "kv_blocks_at_rope_boundary": reached_kv_blocks}));
    }
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "compressor_real", "layers": report})
    );
    Ok(())
}

/// Compressed source rows in their cache form, as BF16 words and floats.
fn source_rows(rng: &mut Rng, count: usize) -> Result<(Vec<u16>, Vec<f32>), CpuError> {
    let mut values = Vec::with_capacity(count * 512);
    for _ in 0..count {
        let row: Vec<f32> = (0..512).map(|_| rng.bf16(1.0)).collect();
        values.extend(inplace_bf16(&row, Quantization::CompressedFp4)?);
    }
    Ok((words(&values), values))
}

#[test]
fn compressed_attention_matches_the_cpu() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(43);
    let g = small(4);
    let d = g.head_dim;
    let sinks = [1.0_f32, -2.0, 0.25, 8.0];
    let sinks_buffer = upload_f32(&mut backend, &sinks)?;
    let max_rows = 700;
    let (kv_words, kv_values) = source_rows(&mut rng, max_rows)?;
    let kv_buffer = upload_u16(&mut backend, &kv_words)?;
    let (_, cached) = window_rows(&mut rng, 1500)?;
    let ring_rows = 128;
    let mut boundary = 0;
    let mut checked = 0;
    for ratio in [2_usize, 1] {
        for position in [
            0_usize, 1, 2, 3, 126, 127, 128, 129, 255, 256, 513, 699, 1023, 1399,
        ] {
            let visible = (position + 1) / ratio;
            if visible > max_rows {
                continue;
            }
            // The ring as the device holds it after position `position`.
            let mut ring = vec![0.0_f32; ring_rows * d];
            for q in (position + 1).saturating_sub(128)..=position {
                ring[(q % ring_rows) * d..(q % ring_rows + 1) * d]
                    .copy_from_slice(&cached[q * d..(q + 1) * d]);
            }
            let ring_buffer = upload_u16(&mut backend, &words(&ring))?;
            // Selection from random scores, with every 5th position masked
            // out as a candidate filter would.
            let scores: Vec<f32> = (0..visible).map(|_| rng.unit()).collect();
            let mask: Vec<bool> = (0..visible).map(|i| i % 5 != 2).collect();
            let selected = select_indices(&scores, visible, g.index_top_k, 0, Some(&mask))?;
            let mut row = selected.clone();
            row.resize(g.index_top_k, -1);
            let indices_buffer = upload_i32(&mut backend, &row)?;
            let q: Vec<f32> = (0..g.heads * d).map(|_| rng.bf16(0.1)).collect();
            let q_buffer = upload_f32(&mut backend, &q)?;
            let positions_buffer = upload_i32(&mut backend, &[position as i32])?;
            let out = f32_buffer(&mut backend, g.heads * d)?;
            let mut submission = backend.begin_submission();
            submission.v41_sparse_attention(
                g,
                &q_buffer,
                &sinks_buffer,
                V41Ring {
                    buffer: &ring_buffer,
                    rows: ring_rows,
                },
                Some(V41Compressed {
                    kv: &kv_buffer,
                    capacity: max_rows,
                    indices: V41Rows {
                        buffer: &indices_buffer,
                        stride: g.index_top_k,
                    },
                    ratio,
                }),
                V41Steps {
                    positions: &positions_buffer,
                    tokens: 1,
                },
                &out,
            )?;
            submission.commit()?;
            let got = read_f32(&out, g.heads * d)?;
            // The CPU runtime's slots: the window rows, then the selection.
            let first = (position + 1).saturating_sub(128);
            let window = cached[first * d..(position + 1) * d].to_vec();
            let n = window.len() / d;
            let host =
                v41_sparse_attention_host(g, &q, &sinks, &window, &kv_values, &selected, visible);
            assert_host(&got, &host, &format!("compressed host {ratio} {position}"));
            let mut kv = window.clone();
            kv.extend_from_slice(&kv_values[..visible * d]);
            let mut indices: Vec<i32> = (0..n as i32).collect();
            indices.extend(
                selected
                    .iter()
                    .map(|i| if *i < 0 { -1 } else { i + n as i32 }),
            );
            let (_, values, bound) = cpu_attention(g, &q, &sinks, &kv, &indices)?;
            boundary += assert_bf16(
                &got,
                &values,
                &bound,
                &format!("compressed {ratio} {position}"),
            );
            checked += 1;
        }
    }
    // An index past the visible rows poisons the output, as the CPU
    // refuses it.
    let mut row = vec![-1; g.index_top_k];
    row[0] = 10;
    let indices_buffer = upload_i32(&mut backend, &row)?;
    let positions_buffer = upload_i32(&mut backend, &[9])?;
    let ring_buffer = upload_u16(&mut backend, &words(&cached[..ring_rows * d]))?;
    let q_buffer = upload_f32(&mut backend, &vec![0.01; g.heads * d])?;
    let out = f32_buffer(&mut backend, g.heads * d)?;
    let mut submission = backend.begin_submission();
    submission.v41_sparse_attention(
        g,
        &q_buffer,
        &sinks_buffer,
        V41Ring {
            buffer: &ring_buffer,
            rows: ring_rows,
        },
        Some(V41Compressed {
            kv: &kv_buffer,
            capacity: max_rows,
            indices: V41Rows {
                buffer: &indices_buffer,
                stride: g.index_top_k,
            },
            ratio: 1,
        }),
        V41Steps {
            positions: &positions_buffer,
            tokens: 1,
        },
        &out,
    )?;
    submission.commit()?;
    assert!(read_f32(&out, g.heads * d)?.iter().all(|v| v.is_nan()));
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "compressed_attention", "tokens_checked": checked, "bf16_boundary_values": boundary})
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Mode 4: the indexer
// ---------------------------------------------------------------------------

/// Runs the index scores and the selection for one token on the device.
#[allow(clippy::too_many_arguments)]
fn device_index(
    backend: &mut V41Metal,
    g: V41AttentionGeometry,
    query: &[f32],
    weights: &[f32],
    keys: &[u16],
    position: usize,
    ratio: usize,
    flags: Option<(&[u8], usize)>,
) -> Result<(Vec<f32>, Vec<i32>), Box<dyn Error>> {
    let capacity = (keys.len() / g.index_dim).max(1);
    let q = upload_f32(backend, query)?;
    let w = upload_f32(backend, weights)?;
    let keys_buffer = upload_u16(backend, keys)?;
    let kv = zeros(backend, 4)?;
    let scores = filled_f32(backend, capacity, f32::NAN)?;
    let selected = zeros(backend, g.index_top_k * 4)?;
    let positions = upload_i32(backend, &[position as i32])?;
    let flag_buffer = match flags {
        Some((f, _)) => Some(backend.byte_buffer(&[f, &[0; 4]].concat())?),
        None => None,
    };
    let steps = V41Steps {
        positions: &positions,
        tokens: 1,
    };
    let mut submission = backend.begin_submission();
    submission.v41_index_scores(
        g,
        &q,
        &w,
        V41Source {
            kv: &kv,
            index_keys: &keys_buffer,
            capacity: keys.len() / g.index_dim,
        },
        ratio,
        steps,
        V41Rows {
            buffer: &scores,
            stride: capacity,
        },
    )?;
    submission.v41_select_indices(
        V41Rows {
            buffer: &scores,
            stride: capacity,
        },
        ratio,
        steps,
        flag_buffer.as_ref().zip(flags).map(|(buffer, (f, group))| {
            (
                V41Rows {
                    buffer,
                    stride: f.len().max(capacity.div_ceil(group)),
                },
                group,
            )
        }),
        g.index_top_k,
        V41Rows {
            buffer: &selected,
            stride: g.index_top_k,
        },
    )?;
    submission.commit()?;
    let visible = (position + 1) / ratio;
    Ok((
        read_f32(&scores, capacity)?[..visible].to_vec(),
        read_i32(&selected, g.index_top_k)?,
    ))
}

/// Index keys in their cache form (BF16 words and floats).
fn index_keys(rng: &mut Rng, count: usize, ties: bool) -> Result<(Vec<u16>, Vec<f32>), CpuError> {
    let mut values = Vec::with_capacity(count * 128);
    let shared: Vec<f32> = (0..128).map(|_| rng.bf16(1.0)).collect();
    for i in 0..count {
        let row: Vec<f32> = if ties && i % 3 == 0 {
            shared.clone()
        } else {
            (0..128).map(|_| rng.bf16(1.0)).collect()
        };
        values.extend(inplace_bf16(&row, Quantization::IndexFp4)?);
    }
    Ok((words(&values), values))
}

#[test]
fn indexer_matches_the_cpu() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(51);
    let g = V41AttentionGeometry::flash();
    let config = V41RopeConfig::compressed(64);
    let (frequencies, table) = rope_table(&mut backend, config)?;
    let rope = cpu_rope(config)?;
    let mut cases = 0;
    let mut reached = 0;
    // The index query: RoPE then the FP4 form, against the CPU chain.
    for position in [0_usize, 128, 20_000, 65_536] {
        let raw: Vec<f32> = (0..32 * 128).map(|_| rng.bf16(1.0)).collect();
        let buffer = upload_f32(&mut backend, &raw)?;
        let positions = upload_i32(&mut backend, &[position as i32])?;
        let mut submission = backend.begin_submission();
        submission.v41_rope(
            V41RopeTable {
                table: &table,
                dim: 64,
                positions: V41_MAX_POSITION,
            },
            &buffer,
            V41Steps {
                positions: &positions,
                tokens: 1,
            },
            32,
            128,
            0,
            false,
            V41Output::Bf16,
        )?;
        submission.v41_cache_quant(V41CacheQuant::IndexFp4, &buffer, &buffer, 32, 128)?;
        submission.commit()?;
        let device = read_f32(&buffer, 32 * 128)?;
        let mut host = raw.clone();
        v41_rope_host(
            &frequencies,
            &mut host,
            128,
            position as i64,
            false,
            V41Output::Bf16,
        );
        assert_host(
            &device,
            &v41_cache_quant_host(V41CacheQuant::IndexFp4, &host),
            "index query host",
        );
        for h in 0..32 {
            let (rotated, bound) =
                cpu_rope_bound(&rope, &raw[h * 128..(h + 1) * 128], position, false)?;
            reached += assert_rotated_blocks(
                &device[h * 128..(h + 1) * 128],
                &rotated,
                &bound,
                Quantization::IndexFp4,
                32,
                "index query",
            )?;
        }
    }
    // Scores and selection: short rows, ties, the 127/128/129 edges, both
    // ratios, and 20,001 positions.
    let (keys, key_values) = index_keys(&mut rng, 20_001, true)?;
    for (ratio, position) in [
        (1, 0),
        (2, 0),
        (2, 1),
        (1, 126),
        (1, 127),
        (1, 128),
        (2, 255),
        (2, 257),
        (1, 511),
        (1, 512),
        (1, 2_000),
        (2, 20_000),
        (1, 20_000),
    ] {
        let visible = (position + 1) / ratio;
        let query: Vec<f32> = v41_cache_quant_host(
            V41CacheQuant::IndexFp4,
            &(0..32 * 128).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
        );
        let weights: Vec<f32> = (0..32).map(|_| rng.bf16(1.0)).collect();
        let (scores, selected) = device_index(
            &mut backend,
            g,
            &query,
            &weights,
            &keys[..visible.max(1) * 128],
            position,
            ratio,
            None,
        )?;
        let cpu_scores = cpu_index_scores(g, &query, &weights, &key_values[..visible * 128])?;
        assert_equal(&scores, &cpu_scores, &format!("scores {ratio} {position}"));
        assert_host(
            &scores,
            &v41_index_scores_host(g, &query, &weights, &keys, visible),
            "scores host",
        );
        assert_eq!(
            selected,
            cpu_select(&cpu_scores, 512, None)?,
            "selection {ratio} {position}"
        );
        cases += 1;
    }
    // Ties: many equal scores across the 512th place go to the lower index.
    let (tie_keys, _) = index_keys(&mut rng, 3_000, false)?;
    let tie_scores_values = tie_scores(&mut rng, 3_000);
    let selected = device_select_scores(&mut backend, &tie_scores_values, 1, 2_999, None)?;
    assert_eq!(selected, cpu_select(&tie_scores_values, 512, None)?, "ties");
    let _ = tie_keys;
    // NaN and +inf poison every slot; an empty history selects nothing.
    let mut nan = tie_scores(&mut rng, 600);
    nan[17] = f32::NAN;
    assert_eq!(
        device_select_scores(&mut backend, &nan, 1, 599, None)?,
        vec![i32::MIN; 512]
    );
    nan[17] = f32::INFINITY;
    assert_eq!(
        device_select_scores(&mut backend, &nan, 1, 599, None)?,
        vec![i32::MIN; 512]
    );
    assert_eq!(
        device_select_scores(&mut backend, &[0.0], 2, 0, None)?,
        vec![-1; 512]
    );
    let all_inf = vec![f32::NEG_INFINITY; 40];
    assert_eq!(
        device_select_scores(&mut backend, &all_inf, 1, 39, None)?,
        cpu_select(&all_inf, 512, None)?
    );
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "indexer", "score_cases": cases, "index_query_blocks_at_rope_boundary": reached})
    );
    Ok(())
}

/// Runs only the selection over given scores for one token.
fn device_select_scores(
    backend: &mut V41Metal,
    scores: &[f32],
    ratio: usize,
    position: usize,
    flags: Option<(&[u8], usize)>,
) -> Result<Vec<i32>, Box<dyn Error>> {
    let stride = scores.len().max(1);
    let buffer = upload_f32(backend, scores)?;
    let positions = upload_i32(backend, &[position as i32])?;
    let out = zeros(backend, 512 * 4)?;
    let flag_buffer = match flags {
        Some((f, _)) => Some(backend.byte_buffer(&[f, &[0; 4]].concat())?),
        None => None,
    };
    let mut submission = backend.begin_submission();
    submission.v41_select_indices(
        V41Rows {
            buffer: &buffer,
            stride,
        },
        ratio,
        V41Steps {
            positions: &positions,
            tokens: 1,
        },
        flag_buffer.as_ref().zip(flags).map(|(buffer, (f, group))| {
            (
                V41Rows {
                    buffer,
                    stride: f.len().max(stride.div_ceil(group)),
                },
                group,
            )
        }),
        512,
        V41Rows {
            buffer: &out,
            stride: 512,
        },
    )?;
    submission.commit()?;
    Ok(read_i32(&out, 512)?)
}

/// Runs layer 20's candidate blocks over given scores for one token: returns
/// (flags, kept blocks).
fn device_candidates(
    backend: &mut V41Metal,
    scores: &[f32],
    position: usize,
) -> Result<(Vec<u8>, Vec<i32>), Box<dyn Error>> {
    let stride = scores.len().max(1);
    let blocks = stride.div_ceil(8);
    let buffer = upload_f32(backend, scores)?;
    let positions = upload_i32(backend, &[position as i32])?;
    let block_scores = f32_buffer(backend, blocks)?;
    let flags = zeros(backend, blocks)?;
    let kept = zeros(backend, 2048 * 4)?;
    let mut submission = backend.begin_submission();
    submission.v41_candidate_blocks(
        V41Rows {
            buffer: &buffer,
            stride,
        },
        1,
        V41Steps {
            positions: &positions,
            tokens: 1,
        },
        V41Candidates {
            block_scores: &block_scores,
            flags: &flags,
            blocks: &kept,
            stride: blocks,
            blocks_stride: 2048,
        },
        2048,
        8,
    )?;
    submission.commit()?;
    let visible_blocks = (position + 1).div_ceil(8);
    Ok((
        flags.read_bytes()?[..visible_blocks].to_vec(),
        read_i32(&kept, 2048)?,
    ))
}

/// The offset the reference adds to its selected indices, which address its
/// KV array: the prompt length in the prefill forward (`last`), the window
/// in decode steps.
fn topk_offset(step: &str, position: usize) -> i64 {
    if step == "last" {
        position as i64 + 1
    } else {
        128
    }
}

/// Compares a selection with the oracle's (minus `offset`; candidate blocks
/// have none): equal except among values equal to the last selected one.
/// Returns the number of tied indices that differ.
fn compare_with_oracle(
    selected: &[i32],
    oracle: &[i64],
    offset: i64,
    scores: &[f32],
    what: &str,
) -> usize {
    let ours: std::collections::BTreeSet<i64> = selected
        .iter()
        .filter(|i| **i >= 0)
        .map(|i| i64::from(*i))
        .collect();
    let theirs: std::collections::BTreeSet<i64> = oracle
        .iter()
        .filter(|i| **i >= 0)
        .map(|i| i - offset)
        .collect();
    assert_eq!(ours.len(), theirs.len(), "{what}: selection sizes");
    let last = ours
        .iter()
        .map(|i| scores[*i as usize])
        .fold(f32::INFINITY, f32::min);
    let mut ties = 0;
    for i in ours.symmetric_difference(&theirs) {
        assert!(
            scores[*i as usize] == last,
            "{what}: index {i} (score {}) differs and isn't tied with {last}",
            scores[*i as usize]
        );
        ties += 1;
    }
    ties
}

#[test]
fn selection_and_candidates_match_the_oracle() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let mut steps_checked = 0;
    let mut selections = 0;
    let mut selection_ties = 0;
    let mut candidate_ties = 0;
    let mut largest = 0;
    for (prompt, step, position) in oracle_steps() {
        let base = format!("{prompt}.decode.{step}.layers");
        let candidate_label = format!("{base}.20.attn.indexer.candidate_blocks");
        let score20 = format!("{base}.20.attn.indexer.score");
        if !rows.has(&score20) {
            continue;
        }
        // Layer 20's candidate blocks.
        let scores = rows.f32(&score20);
        assert_eq!(scores.len(), position + 1, "{score20}");
        let (flags, kept) = device_candidates(&mut backend, &scores, position)?;
        let (_, host_flags, host_kept) = v41_candidate_blocks_host(&scores, 2048, 8);
        assert_eq!(flags, host_flags, "{candidate_label} flags host");
        assert_eq!(kept, host_kept, "{candidate_label} host");
        let mask = candidate_mask(&scores, scores.len(), 2048, 8)?;
        assert_eq!(
            (0..scores.len())
                .map(|i| flags[i / 8] == 1)
                .collect::<Vec<_>>(),
            mask,
            "{candidate_label} CPU"
        );
        if rows.has(&candidate_label) {
            let block_max: Vec<f32> = (0..scores.len().div_ceil(8))
                .map(|b| {
                    if b == position / 8 {
                        f32::INFINITY
                    } else {
                        scores[b * 8..((b + 1) * 8).min(scores.len())]
                            .iter()
                            .fold(f32::NEG_INFINITY, |a, v| a.max(*v))
                    }
                })
                .collect();
            candidate_ties += compare_with_oracle(
                &kept,
                &rows.ints(&candidate_label),
                0,
                &block_max,
                &candidate_label,
            );
        }
        // Every index source's selection.
        for layer in V41_INDEX_SOURCES {
            let label = format!("{base}.{layer}.attn.indexer.score");
            let topk = format!("{base}.{layer}.attn.indexer.topk_idxs");
            if !rows.has(&label) {
                continue;
            }
            let ratio = compress_ratio(layer)?;
            let scores = rows.f32(&label);
            assert_eq!(scores.len(), (position + 1) / ratio, "{label}");
            let candidates = (layer > 20).then_some((flags.as_slice(), 8));
            let selected =
                device_select_scores(&mut backend, &scores, ratio, position, candidates)?;
            let cpu_mask = (layer > 20).then_some(mask.as_slice());
            assert_eq!(selected, cpu_select(&scores, 512, cpu_mask)?, "{label} CPU");
            assert_eq!(
                selected,
                v41_select_host(&scores, candidates, 512, false),
                "{label} host"
            );
            if rows.has(&topk) {
                selection_ties += compare_with_oracle(
                    &selected,
                    &rows.ints(&topk),
                    topk_offset(step, position),
                    &scores,
                    &label,
                );
            }
            selections += 1;
            largest = largest.max(scores.len());
        }
        steps_checked += 1;
    }
    assert!(steps_checked >= 20, "{steps_checked} oracle steps");
    // Synthetic: 20,001 positions with ties, the candidate budget, and a
    // later indexer that reads the flags.
    let mut rng = Rng(61);
    let scores = tie_scores(&mut rng, 20_001)
        .into_iter()
        .map(|s| if s.is_finite() { s } else { -7.0 })
        .collect::<Vec<_>>();
    let (flags, kept) = device_candidates(&mut backend, &scores, 20_000)?;
    let mask = candidate_mask(&scores, 20_001, 2048, 8)?;
    assert_eq!(
        (0..20_001).map(|i| flags[i / 8] == 1).collect::<Vec<_>>(),
        mask
    );
    assert_eq!(kept.iter().filter(|b| **b >= 0).count(), 2048);
    assert!(flags[20_000 / 8] == 1, "the newest block is pinned");
    let later = tie_scores(&mut rng, 20_001);
    let selected = device_select_scores(&mut backend, &later, 1, 20_000, Some((&flags, 8)))?;
    assert_eq!(selected, cpu_select(&later, 512, Some(&mask))?);
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "selection_oracle", "steps": steps_checked, "selections": selections,
               "largest_visible": largest, "selection_tied_indices_differing": selection_ties,
               "candidate_tied_blocks_differing": candidate_ties})
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Mode 6: shared sources read on the device
// ---------------------------------------------------------------------------

/// One submission runs a prefill chunk through two KV sources (layer 2 at
/// ratio 2 and layer 20 at ratio 1, with their real compressor norms and
/// indexer key weights) and the consumers that read them in place: layer 2's
/// and layer 20's own indexers, layer 24's indexer over layer 20's keys with
/// layer 20's candidates, layer 3 and layer 21 attending through their
/// sources' selections, and layer 24 attending through its own. Nothing is
/// copied to the host until the end; every stage then equals its host model
/// on the device's own inputs and the CPU component on the same inputs.
#[test]
fn shared_sources_are_read_on_the_device() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let mut rng = Rng(71);
    let g = small(4);
    let d = g.head_dim;
    let tokens = 600;
    let config = V41RopeConfig::compressed(64);
    let (frequencies, table) = rope_table(&mut backend, config)?;
    let rope_table_ref = V41RopeTable {
        table: &table,
        dim: 64,
        positions: V41_MAX_POSITION,
    };
    let positions: Vec<i32> = (0..tokens as i32).collect();
    let positions_buffer = upload_i32(&mut backend, &positions)?;
    let steps = V41Steps {
        positions: &positions_buffer,
        tokens,
    };
    let sinks: Vec<f32> = rows.f32("layers.20.attn.attn_sink")[..g.heads].to_vec();
    let sinks_buffer = upload_f32(&mut backend, &sinks)?;
    let ring_rows = g.window + tokens - 1;
    let mut source_report = Vec::new();

    for (source_layer, consumers) in [(2_usize, vec![2_usize, 3]), (20, vec![20, 21, 24])] {
        let ratio = compress_ratio(source_layer)?;
        let capacity = tokens / ratio;
        let name = |part: &str| format!("layers.{source_layer}.attn.{part}");
        let norm = rows.f32(&name("compressor.norm.weight"));
        let wk = rows.words(&name("indexer.wk.weight"));
        let k_norm = rows.f32(&name("indexer.k_norm.weight"));
        let values: Vec<f32> = (0..tokens * 512).map(|_| rng.unit() * 2.0).collect();
        let gates: Vec<f32> = (0..tokens * 512).map(|_| rng.unit() * 3.0).collect();
        let values_buffer = upload_f32(&mut backend, &values)?;
        let gates_buffer = upload_f32(&mut backend, &gates)?;
        let norm_buffer = upload_f32(&mut backend, &norm)?;
        let wk_buffer = upload_u16(&mut backend, &wk)?;
        let k_norm_buffer = upload_f32(&mut backend, &k_norm)?;
        let state = filled_f32(&mut backend, 2 * ratio * 512, f32::NAN)?;
        let latents = filled_f32(&mut backend, tokens * 512, f32::NAN)?;
        let kv = zeros(&mut backend, capacity * 512 * 2)?;
        let keys = zeros(&mut backend, capacity * 128 * 2)?;
        let source = V41Source {
            kv: &kv,
            index_keys: &keys,
            capacity,
        };
        // Per consumer: a ring with its window rows, a query, index
        // inputs, and outputs.
        struct Consumer {
            layer: usize,
            ring: V41Buffer,
            raw_window: Vec<f32>,
            window_in: V41Buffer,
            query: Vec<f32>,
            query_buffer: V41Buffer,
            index_query: Vec<f32>,
            index_query_buffer: V41Buffer,
            index_weights: Vec<f32>,
            index_weights_buffer: V41Buffer,
            scores: V41Buffer,
            selected: V41Buffer,
            out: V41Buffer,
        }
        let mut layers = Vec::new();
        for &layer in &consumers {
            let raw_window: Vec<f32> = (0..tokens * d).map(|_| rng.bf16(1.0)).collect();
            let query: Vec<f32> = (0..tokens * g.heads * d).map(|_| rng.bf16(0.1)).collect();
            let index_query: Vec<f32> = (0..tokens * 32 * 128).map(|_| rng.bf16(1.0)).collect();
            let index_weights: Vec<f32> = (0..tokens * 32).map(|_| rng.bf16(1.0)).collect();
            layers.push(Consumer {
                layer,
                ring: zeros(&mut backend, ring_rows * d * 2)?,
                window_in: upload_f32(&mut backend, &raw_window)?,
                raw_window,
                query_buffer: upload_f32(&mut backend, &query)?,
                query,
                index_query_buffer: upload_f32(&mut backend, &index_query)?,
                index_query,
                index_weights_buffer: upload_f32(&mut backend, &index_weights)?,
                index_weights,
                scores: filled_f32(&mut backend, tokens * capacity, f32::NAN)?,
                selected: zeros(&mut backend, tokens * g.index_top_k * 4)?,
                out: f32_buffer(&mut backend, tokens * g.heads * d)?,
            });
        }
        let block_capacity = capacity.div_ceil(8);
        let block_scores = f32_buffer(&mut backend, tokens * block_capacity)?;
        let flags = zeros(&mut backend, tokens * block_capacity)?;
        let kept = zeros(&mut backend, tokens * 2048 * 4)?;

        let mut submission = backend.begin_submission();
        submission.v41_compress(
            V41Compressor {
                ratio,
                width: 512,
                epsilon: g.norm_epsilon,
            },
            &values_buffer,
            (ratio > 1).then_some(&gates_buffer),
            &norm_buffer,
            &state,
            steps,
            &latents,
        )?;
        submission.v41_compressed_store(
            g,
            ratio,
            V41IndexKeyWeights {
                wk: &wk_buffer,
                k_norm: &k_norm_buffer,
            },
            rope_table_ref,
            &latents,
            steps,
            source,
        )?;
        for c in &layers {
            let owners = V41LayerOwners::for_layer(c.layer).ok_or("owners")?;
            assert_eq!(owners.kv, Some(source_layer));
            let ring = V41Ring {
                buffer: &c.ring,
                rows: ring_rows,
            };
            submission.v41_window_store(g, &c.window_in, steps, ring)?;
            if owners.index == Some(c.layer) {
                submission.v41_rope(
                    rope_table_ref,
                    &c.index_query_buffer,
                    steps,
                    32,
                    128,
                    0,
                    false,
                    V41Output::Bf16,
                )?;
                submission.v41_cache_quant(
                    V41CacheQuant::IndexFp4,
                    &c.index_query_buffer,
                    &c.index_query_buffer,
                    tokens * 32,
                    128,
                )?;
                submission.v41_index_scores(
                    g,
                    &c.index_query_buffer,
                    &c.index_weights_buffer,
                    source,
                    ratio,
                    steps,
                    V41Rows {
                        buffer: &c.scores,
                        stride: capacity,
                    },
                )?;
                if c.layer == 20 {
                    submission.v41_candidate_blocks(
                        V41Rows {
                            buffer: &c.scores,
                            stride: capacity,
                        },
                        ratio,
                        steps,
                        V41Candidates {
                            block_scores: &block_scores,
                            flags: &flags,
                            blocks: &kept,
                            stride: block_capacity,
                            blocks_stride: 2048,
                        },
                        2048,
                        8,
                    )?;
                }
                submission.v41_select_indices(
                    V41Rows {
                        buffer: &c.scores,
                        stride: capacity,
                    },
                    ratio,
                    steps,
                    (c.layer > 20).then_some((
                        V41Rows {
                            buffer: &flags,
                            stride: block_capacity,
                        },
                        8,
                    )),
                    g.index_top_k,
                    V41Rows {
                        buffer: &c.selected,
                        stride: g.index_top_k,
                    },
                )?;
            }
            let owner = layers
                .iter()
                .find(|o| Some(o.layer) == owners.index)
                .ok_or("index owner")?;
            submission.v41_sparse_attention(
                g,
                &c.query_buffer,
                &sinks_buffer,
                ring,
                Some(V41Compressed {
                    kv: &kv,
                    capacity,
                    indices: V41Rows {
                        buffer: &owner.selected,
                        stride: g.index_top_k,
                    },
                    ratio,
                }),
                steps,
                &c.out,
            )?;
        }
        submission.commit()?;

        // The source rows the consumers read.
        let device_latents = read_f32(&latents, tokens * 512)?;
        let device_kv = read_bf16(&kv, capacity * 512)?;
        let device_keys = read_bf16(&keys, capacity * 128)?;
        let mut host = V41CompressorHost::new(
            V41Compressor {
                ratio,
                width: 512,
                epsilon: g.norm_epsilon,
            },
            f32::NAN,
        );
        let host_latents = host.push(
            &values,
            (ratio > 1).then_some(gates.as_slice()),
            &norm,
            &positions,
        );
        for (t, row) in host_latents.iter().enumerate() {
            let got = &device_latents[t * 512..(t + 1) * 512];
            match row {
                Some(row) => {
                    assert_host(got, row, "source latent");
                    let r = (t + 1) / ratio - 1;
                    let (key, kv_row) =
                        v41_compressed_store_host(g, ratio, &wk, &k_norm, &frequencies, got, t);
                    assert_host(&device_keys[r * 128..(r + 1) * 128], &key, "source key");
                    assert_host(&device_kv[r * 512..(r + 1) * 512], &kv_row, "source KV");
                }
                None => assert!(got.iter().all(|v| v.is_nan())),
            }
        }
        let mut cpu = CompressionState::new(ratio, 512)?.with_arithmetic(Arithmetic::ReferenceBf16);
        let cpu_latents = cpu.push_chunk(
            0,
            &values,
            (ratio > 1).then_some(gates.as_slice()),
            &norm,
            g.norm_epsilon,
        )?;
        let mut fallback = 0;
        let mut differing = 0;
        for (r, cpu_latent) in cpu_latents.iter().enumerate() {
            let t = (r + 1) * ratio - 1;
            let got = &device_latents[t * 512..(t + 1) * 512];
            if ratio == 1 {
                assert_equal(got, cpu_latent, "source latent CPU");
            } else {
                let v: Vec<&[f32]> = (0..2)
                    .map(|k| &values[(t - 1 + k) * 512..(t + k) * 512])
                    .collect();
                let gs: Vec<&[f32]> = (0..2)
                    .map(|k| &gates[(t - 1 + k) * 512..(t + k) * 512])
                    .collect();
                let (used, differs) = check_pooled_latent(
                    got,
                    cpu_latent,
                    &v,
                    &gs,
                    &norm,
                    g.norm_epsilon,
                    "source latent CPU",
                )?;
                fallback += usize::from(used);
                differing += usize::from(differs);
            }
        }

        // Each consumer, checked token by token on the device's own inputs.
        let mut selections = BTreeMap::new();
        let mut boundary = 0;
        for c in &layers {
            let owners = V41LayerOwners::for_layer(c.layer).ok_or("owners")?;
            let stored = read_bf16(&c.ring, ring_rows * d)?;
            let mut cached = Vec::with_capacity(tokens * d);
            for row in c.raw_window.chunks_exact(d) {
                cached.extend(inplace_bf16(row, Quantization::Fp8)?);
            }
            assert_equal(&stored[..tokens * d], &cached, "consumer ring");
            if owners.index == Some(c.layer) {
                let scores = read_f32(&c.scores, tokens * capacity)?;
                let selected = read_i32(&c.selected, tokens * g.index_top_k)?;
                let flag_rows = flags.read_bytes()?;
                let mut device_query = c.index_query.clone();
                for t in 0..tokens {
                    let q = &mut device_query[t * 4096..(t + 1) * 4096];
                    v41_rope_host(&frequencies, q, 128, t as i64, false, V41Output::Bf16);
                    let quantized = v41_cache_quant_host(V41CacheQuant::IndexFp4, q);
                    q.copy_from_slice(&quantized);
                }
                for t in 0..tokens {
                    let visible = (t + 1) / ratio;
                    let q = &device_query[t * 4096..(t + 1) * 4096];
                    let w = &c.index_weights[t * 32..(t + 1) * 32];
                    let row_scores = &scores[t * capacity..t * capacity + visible];
                    let key_values = &device_keys[..visible * 128];
                    assert_equal(
                        row_scores,
                        &cpu_index_scores(g, q, w, key_values)?,
                        "consumer scores",
                    );
                    let mask = if c.layer > 20 {
                        let f = &flag_rows[t * block_capacity..(t + 1) * block_capacity];
                        Some((0..visible).map(|i| f[i / 8] == 1).collect::<Vec<_>>())
                    } else {
                        None
                    };
                    if c.layer == 20 {
                        let cpu_mask = candidate_mask(row_scores, visible, 2048, 8)?;
                        let f = &flag_rows[t * block_capacity..(t + 1) * block_capacity];
                        assert_eq!(
                            (0..visible).map(|i| f[i / 8] == 1).collect::<Vec<_>>(),
                            cpu_mask,
                            "candidates"
                        );
                    }
                    let row = &selected[t * 512..(t + 1) * 512];
                    assert_eq!(
                        row,
                        cpu_select(row_scores, 512, mask.as_deref())?.as_slice(),
                        "consumer selection {t}"
                    );
                }
                selections.insert(c.layer, selected);
            }
            let selected = selections
                .get(&owners.index.ok_or("index")?)
                .ok_or("owner selection")?;
            let out = read_f32(&c.out, tokens * g.heads * d)?;
            for t in (0..tokens).step_by(37).chain([127, 128, 129, tokens - 1]) {
                let visible = (t + 1) / ratio;
                let count = visible.min(g.index_top_k);
                let first = (t + 1).saturating_sub(g.window);
                let window = cached[first * d..(t + 1) * d].to_vec();
                let n = window.len() / d;
                let token_selection = &selected[t * 512..t * 512 + count];
                let q = &c.query[t * g.heads * d..(t + 1) * g.heads * d];
                let got = &out[t * g.heads * d..(t + 1) * g.heads * d];
                let host = v41_sparse_attention_host(
                    g,
                    q,
                    &sinks,
                    &window,
                    &device_kv,
                    token_selection,
                    visible,
                );
                assert_host(got, &host, "consumer attention host");
                let mut kv_all = window.clone();
                kv_all.extend_from_slice(&device_kv[..visible * d]);
                let mut indices: Vec<i32> = (0..n as i32).collect();
                indices.extend(
                    token_selection
                        .iter()
                        .map(|i| if *i < 0 { -1 } else { i + n as i32 }),
                );
                let (_, values, bound) = cpu_attention(g, q, &sinks, &kv_all, &indices)?;
                boundary += assert_bf16(
                    got,
                    &values,
                    &bound,
                    &format!("layer {} token {t}", c.layer),
                );
            }
        }
        source_report.push(json!({"source": source_layer, "ratio": ratio, "consumers": consumers,
            "tokens": tokens, "latent_fallback_bounds": fallback, "latents_differing": differing, "attention_bf16_boundary_values": boundary}));
    }
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "shared_sources", "sources": source_report})
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Decode latency (observation)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "measurement"]
fn v41_attention_decode_latency() -> TestResult {
    let Some(mut backend) = metal() else {
        return Ok(());
    };
    let mut rng = Rng(81);
    let g = V41AttentionGeometry::flash();
    let d = g.head_dim;
    let mut report = Vec::new();
    let (_, table) = rope_table(&mut backend, V41RopeConfig::compressed(64))?;
    let rope = V41RopeTable {
        table: &table,
        dim: 64,
        positions: V41_MAX_POSITION,
    };
    let query = upload_f32(
        &mut backend,
        &(0..g.heads * d).map(|_| rng.bf16(0.1)).collect::<Vec<_>>(),
    )?;
    let index_query = upload_f32(
        &mut backend,
        &(0..4096).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let index_weights = upload_f32(
        &mut backend,
        &(0..32).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let row = upload_f32(
        &mut backend,
        &(0..d).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let x = upload_f32(
        &mut backend,
        &(0..5120).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let sinks = upload_f32(&mut backend, &vec![0.5; g.heads])?;
    let ring = zeros(&mut backend, g.window * d * 2)?;
    let out = f32_buffer(&mut backend, g.heads * d)?;
    let values = f32_buffer(&mut backend, d)?;
    let latents = f32_buffer(&mut backend, d)?;
    let w_bf16 = upload_u16(
        &mut backend,
        &(0..512 * 5120)
            .map(|_| v41_bf16_bits(rng.bf16(0.02)))
            .collect::<Vec<_>>(),
    )?;
    let w_f32 = upload_f32(
        &mut backend,
        &(0..512 * 5120).map(|_| rng.bf16(0.02)).collect::<Vec<_>>(),
    )?;
    let wk = upload_u16(
        &mut backend,
        &(0..128 * 512)
            .map(|_| v41_bf16_bits(rng.bf16(0.05)))
            .collect::<Vec<_>>(),
    )?;
    let norm = upload_f32(&mut backend, &vec![1.0; 512])?;
    let state = filled_f32(&mut backend, 2 * 2 * 512, 0.5)?;
    let mut run = |label: &str,
                   backend: &mut V41Metal,
                   position: i32,
                   launch: &mut dyn FnMut(
        &mut crate::v41::V41Submission,
        V41Steps<'_>,
    ) -> Result<(), RuntimeError>|
     -> TestResult {
        let positions = upload_i32(backend, &[position])?;
        let steps = V41Steps {
            positions: &positions,
            tokens: 1,
        };
        let mut time = |n: usize| -> Result<f64, Box<dyn Error>> {
            let mut submission = backend.begin_submission();
            let start = std::time::Instant::now();
            for _ in 0..n {
                launch(&mut submission, steps)?;
            }
            submission.commit()?;
            Ok(start.elapsed().as_secs_f64() / n as f64)
        };
        time(5)?;
        let us = time(100)? * 1e6;
        eprintln!("{label}: {us:.2} us");
        report.push(json!({"kernel": label, "microseconds": us}));
        Ok(())
    };
    run(
        "RoPE (query, 64x512)",
        &mut backend,
        20_000,
        &mut |s, st| s.v41_rope(rope, &query, st, 64, 512, 0, false, V41Output::Bf16),
    )?;
    run(
        "window store (512, FP8 form)",
        &mut backend,
        20_000,
        &mut |s, st| {
            s.v41_window_store(
                g,
                &row,
                st,
                V41Ring {
                    buffer: &ring,
                    rows: 128,
                },
            )
        },
    )?;
    run(
        "dense 512x5120 F32 (ratio-2 compressor)",
        &mut backend,
        0,
        &mut |s, _| {
            s.v41_dense(
                V41Dense {
                    weights: &w_f32,
                    scales: None,
                    format: V41DenseFormat::F32,
                    rows: 512,
                    cols: 5120,
                    groups: 1,
                },
                &x,
                &values,
                1,
                false,
                V41Output::F32,
            )
        },
    )?;
    run(
        "dense 512x5120 BF16 (ratio-1 compressor)",
        &mut backend,
        0,
        &mut |s, _| {
            s.v41_dense(
                V41Dense {
                    weights: &w_bf16,
                    scales: None,
                    format: V41DenseFormat::Bf16,
                    rows: 512,
                    cols: 5120,
                    groups: 1,
                },
                &x,
                &values,
                1,
                true,
                V41Output::Bf16,
            )
        },
    )?;
    run(
        "dense 32x5120 BF16 (index weights)",
        &mut backend,
        0,
        &mut |s, _| {
            s.v41_dense(
                V41Dense {
                    weights: &w_bf16,
                    scales: None,
                    format: V41DenseFormat::Bf16,
                    rows: 32,
                    cols: 5120,
                    groups: 1,
                },
                &x,
                &index_weights,
                1,
                true,
                V41Output::Bf16,
            )
        },
    )?;
    run("RMS norm (1280)", &mut backend, 0, &mut |s, _| {
        s.v41_rms_norm(&x, &x, &out, 1, 1280, 1e-20, V41Output::Bf16)
    })?;
    run(
        "compressor (ratio 2, completing)",
        &mut backend,
        20_001,
        &mut |s, st| {
            s.v41_compress(
                V41Compressor {
                    ratio: 2,
                    width: 512,
                    epsilon: 1e-20,
                },
                &values,
                Some(&values),
                &norm,
                &state,
                st,
                &latents,
            )
        },
    )?;
    run(
        "index cache form (32x128 FP4)",
        &mut backend,
        0,
        &mut |s, _| s.v41_cache_quant(V41CacheQuant::IndexFp4, &index_query, &index_query, 32, 128),
    )?;
    for visible in [2_048_usize, 20_001, 65_536] {
        let keys = upload_u16(
            &mut backend,
            &(0..visible * 128)
                .map(|_| v41_bf16_bits(rng.bf16(1.0)))
                .collect::<Vec<_>>(),
        )?;
        let kv = upload_u16(
            &mut backend,
            &(0..visible * 512)
                .map(|_| v41_bf16_bits(rng.bf16(1.0)))
                .collect::<Vec<_>>(),
        )?;
        let source = V41Source {
            kv: &kv,
            index_keys: &keys,
            capacity: visible,
        };
        let scores = f32_buffer(&mut backend, visible)?;
        let blocks = visible.div_ceil(8);
        let block_scores = f32_buffer(&mut backend, blocks)?;
        let flags = zeros(&mut backend, blocks)?;
        let kept = zeros(&mut backend, 2048 * 4)?;
        let selected = zeros(&mut backend, 512 * 4)?;
        let position = visible as i32 - 1;
        run(
            &format!("compressed store ({visible} rows)"),
            &mut backend,
            position,
            &mut |s, st| {
                s.v41_compressed_store(
                    g,
                    1,
                    V41IndexKeyWeights {
                        wk: &wk,
                        k_norm: &norm,
                    },
                    rope,
                    &latents,
                    st,
                    source,
                )
            },
        )?;
        run(
            &format!("index scores ({visible} keys)"),
            &mut backend,
            position,
            &mut |s, st| {
                s.v41_index_scores(
                    g,
                    &index_query,
                    &index_weights,
                    source,
                    1,
                    st,
                    V41Rows {
                        buffer: &scores,
                        stride: visible,
                    },
                )
            },
        )?;
        run(
            &format!("candidate blocks ({visible} keys)"),
            &mut backend,
            position,
            &mut |s, st| {
                s.v41_candidate_blocks(
                    V41Rows {
                        buffer: &scores,
                        stride: visible,
                    },
                    1,
                    st,
                    V41Candidates {
                        block_scores: &block_scores,
                        flags: &flags,
                        blocks: &kept,
                        stride: blocks,
                        blocks_stride: 2048,
                    },
                    2048,
                    8,
                )
            },
        )?;
        run(
            &format!("top-512 selection ({visible} keys)"),
            &mut backend,
            position,
            &mut |s, st| {
                s.v41_select_indices(
                    V41Rows {
                        buffer: &scores,
                        stride: visible,
                    },
                    1,
                    st,
                    Some((
                        V41Rows {
                            buffer: &flags,
                            stride: blocks,
                        },
                        8,
                    )),
                    512,
                    V41Rows {
                        buffer: &selected,
                        stride: 512,
                    },
                )
            },
        )?;
        run(
            &format!("attention, 128 window + 512 selected ({visible} rows)"),
            &mut backend,
            position,
            &mut |s, st| {
                s.v41_sparse_attention(
                    g,
                    &query,
                    &sinks,
                    V41Ring {
                        buffer: &ring,
                        rows: 128,
                    },
                    Some(V41Compressed {
                        kv: &kv,
                        capacity: visible,
                        indices: V41Rows {
                            buffer: &selected,
                            stride: 512,
                        },
                        ratio: 1,
                    }),
                    st,
                    &out,
                )
            },
        )?;
    }
    run(
        "attention, window only (128)",
        &mut backend,
        20_000,
        &mut |s, st| {
            s.v41_sparse_attention(
                g,
                &query,
                &sinks,
                V41Ring {
                    buffer: &ring,
                    rows: 128,
                },
                None,
                st,
                &out,
            )
        },
    )?;
    eprintln!("{}", serde_json::to_string(&report)?);
    Ok(())
}
