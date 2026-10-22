//! Parity tests for the V4.1 mixing, Engram, and routing kernels against the
//! CPU path (`psionic_models::deepseek_v41_cpu`).
//!
//! Tolerances, fixed before the first GPU run. With u = 2^-24 and
//! gamma(n) = n u / (1 - n u):
//!
//! - **Exact.** Collapse, expand, the Engram gather, and the expert sum do the
//!   CPU's operations in the CPU's order, so they must equal it (as values;
//!   only the sign of a zero may differ). Every kernel must equal its host
//!   model bit for bit.
//! - **Reductions.** Where the device adds the same rounded terms `t_i` as
//!   the CPU in another order, |gpu - cpu| <= (gamma(K - 1) + gamma(d)) *
//!   sum |t_i|, d being the device's longest addition chain (Higham, Accuracy
//!   and Stability of Numerical Algorithms, 2nd ed., section 4.2). This
//!   covers the mixing projection and squares, the router logits, and the
//!   Engram sums.
//! - **Transcendentals.** The device rounds a double-precision `exp` or
//!   `log1p` once; the CPU calls the platform's float `expf` and `log1pf`,
//!   assumed within 1 ulp. They can differ by up to 2 ulp, so a sigmoid
//!   differs by at most 12 u relative after its addition and division on
//!   both sides, and a sqrtsoftplus score by 6 u relative.
//! - **Propagation.** Each later step adds its own rounding on both sides
//!   and carries the earlier difference through the step's exact derivative
//!   or, for monotone steps, through the step evaluated at both ends of the
//!   interval. The Sinkhorn iterations carry the comb's relative error in
//!   the norm ||e||_x = sqrt(sum x_ij e_ij^2): dividing a row or column by
//!   its sum s grows it by at most sqrt(1 / s), and each normalization adds
//!   10 u per entry, so |gpu - cpu| <= sqrt(comb) * E + E^2 at the end (first
//!   order plus the next term). The sums come from the CPU's own values.
//! - **BF16 outputs.** A BF16 value may differ from the CPU's only where the
//!   float32 bound reaches a BF16 rounding boundary.
//! - **Selections.** Expert ids must match exactly, except where the bound
//!   lets a selected and an unselected expert swap; such cases are reported
//!   and skipped. Oracle ids are compared except at the oracle's recorded
//!   exact ties (equal 6th and 7th biased scores), which are reported.

use super::*;
use crate::v41_matvec::{V41_BLOCK, V41Matrix, V41MatvecFormat, V41Slots, v41_block_matvec_host};
use crate::{CudaBackend, CudaCommandWait};
use psionic_core::{DType, Shape, TensorSpec};
use psionic_models::deepseek_v41_cpu::engram::EngramGate;
use psionic_models::deepseek_v41_cpu::hyperconnections::{HyperConnections, Mixes};
use psionic_models::deepseek_v41_cpu::moe::{self, ExpertProvider, Router};
use psionic_models::deepseek_v41_cpu::quantization::{
    Arithmetic, MatrixPrecision, Quantization, linear, quantize, round_bf16,
};
use psionic_models::deepseek_v41_cpu::{CpuError, MatrixRef, MatrixRows, matvec};
use psionic_models::deepseek_v41_weights::{
    decode_bf16, decode_e2m1, decode_f8_e4m3fn, decode_f8_e8m0fnu, dequantize_fp8_per32,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{error::Error, path::PathBuf};

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
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/deepseek_v41_mix");
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

    /// Float values of a BF16 or F32 section.
    fn f32(&self, label: &str) -> Vec<f32> {
        let section = self.section(label);
        let dtype = section.map(|s| s.dtype.as_str());
        let raw = self.raw(label);
        let elements: u64 = section.map_or(0, |s| s.shape.iter().product());
        let values: Vec<f32> = match dtype {
            Some("bfloat16" | "Bf16") => raw
                .chunks_exact(2)
                .map(|p| decode_bf16([p[0], p[1]]))
                .collect(),
            Some("float32" | "F32") => raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
            other => panic!("{label}: not a float section ({other:?})"),
        };
        assert_eq!(values.len() as u64, elements, "{label} shape");
        values
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

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

fn gamma(n: usize) -> f64 {
    let nu = n as f64 * U;
    nu / (1.0 - nu)
}

/// Longest addition chain of a 256-thread block sum of `n` terms.
fn block_depth(n: usize) -> usize {
    n.div_ceil(V41_REDUCE_THREADS) + 5 + 7
}

/// Bound factor for one reduction of `k` identical terms: the CPU adds them
/// sequentially, the device with chain length `depth`.
fn reorder(k: usize, depth: usize) -> f64 {
    gamma(k.saturating_sub(1)) + gamma(depth)
}

fn hc_depth(hc: V41HyperConfig) -> usize {
    block_depth(hc.elements().div_ceil(V41_HC_SPLITS)) + V41_HC_SPLITS - 1
}

fn gate_depth(cols: usize) -> usize {
    cols.div_ceil(256) * 8 + 5
}

fn sigmoid64(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn softplus_root(x: f64) -> f64 {
    let softplus = if x > 20.0 { x } else { x.exp().ln_1p() };
    softplus.sqrt()
}

/// Bounds on |gpu - cpu| for each mix value given the CPU's logits and a
/// bound on each logit's difference.
fn mix_bounds(hc: V41HyperConfig, logits: &[f32], dl: &[f64]) -> Vec<f64> {
    let h = hc.streams;
    let eps = f64::from(hc.mix_epsilon);
    let mut out = Vec::with_capacity(hc.mixes());
    for i in 0..2 * h {
        let l = f64::from(logits[i]);
        let s = sigmoid64(l);
        let slope = s * (1.0 - s);
        out.push(if i < h {
            slope * dl[i] + 12.0 * U * s + 2.0 * U * (s + eps)
        } else {
            2.0 * slope * dl[i] + 24.0 * U * s
        });
    }
    // Initial comb: row softmax plus eps, relative error per entry.
    let comb_logits: Vec<f64> = logits[2 * h..].iter().map(|x| f64::from(*x)).collect();
    let comb_dl = &dl[2 * h..];
    let mut x = vec![0.0_f64; h * h];
    let mut rel = vec![0.0_f64; h * h];
    for r in 0..h {
        let row = &comb_logits[r * h..(r + 1) * h];
        let m = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let dmax = comb_dl[r * h..(r + 1) * h]
            .iter()
            .copied()
            .fold(0.0, f64::max);
        let z: Vec<f64> = row.iter().map(|l| (l - m).exp()).collect();
        let sum: f64 = z.iter().sum();
        let rz: Vec<f64> = (0..h)
            .map(|j| comb_dl[r * h + j] + dmax + 2.0 * U * (row[j] - m).abs() + 3.0 * U)
            .collect();
        let rz_max = rz.iter().copied().fold(0.0, f64::max);
        for j in 0..h {
            x[r * h + j] = z[j] / sum + eps;
            rel[r * h + j] = rz[j] + rz_max + 10.0 * U;
        }
    }
    let mut e = x
        .iter()
        .zip(&rel)
        .map(|(x, r)| x * r * r)
        .sum::<f64>()
        .sqrt();
    let normalize = |x: &mut [f64], e: &mut f64, rows: bool| {
        let mut worst = 0.0_f64;
        for a in 0..h {
            let index = |b: usize| if rows { a * h + b } else { b * h + a };
            let sum = (0..h).map(|b| x[index(b)]).sum::<f64>() + eps;
            worst = worst.max(1.0 / sum);
            for b in 0..h {
                x[index(b)] /= sum;
            }
        }
        let mass: f64 = x.iter().sum();
        *e = *e * worst.sqrt() + 10.0 * U * mass.sqrt();
    };
    for iteration in 0..hc.sinkhorn_iterations {
        if iteration > 0 {
            normalize(&mut x, &mut e, true);
        }
        normalize(&mut x, &mut e, false);
    }
    out.extend(x.iter().map(|x| x.sqrt() * e + e * e));
    out
}

/// The CPU's mixing logits and a bound on each projected value's
/// difference, for a residual and an F32 projection.
fn projection_bounds(
    hc: V41HyperConfig,
    projection: &[f32],
    scale: &[f32],
    base: &[f32],
    residual: &[f32],
) -> (Vec<f32>, Vec<f64>) {
    let (m, k) = (hc.mixes(), hc.elements());
    let factor = reorder(k, hc_depth(hc));
    let squares: f32 = residual.iter().map(|x| x * x).sum();
    let variance = squares / k as f32;
    let inv = (variance + hc.norm_epsilon).sqrt().recip();
    let rel_squares = if squares > 0.0 {
        factor * residual.iter().map(|x| f64::from(x * x)).sum::<f64>() / f64::from(squares)
    } else {
        0.0
    };
    let rel_inv = 0.5 * (rel_squares + 4.0 * U) + 4.0 * U;
    let mut logits = Vec::with_capacity(m);
    let mut dl = Vec::with_capacity(m);
    for i in 0..m {
        let row = &projection[i * k..(i + 1) * k];
        let dot: f32 = row.iter().zip(residual).map(|(w, x)| w * x).sum();
        let magnitude: f64 = row
            .iter()
            .zip(residual)
            .map(|(w, x)| f64::from((w * x).abs()))
            .sum();
        let p = dot * inv;
        let dp = factor * magnitude * f64::from(inv)
            + f64::from((dot * inv).abs()) * rel_inv
            + 2.0 * U * f64::from(p.abs());
        let s = scale[if i < hc.streams {
            0
        } else if i < 2 * hc.streams {
            1
        } else {
            2
        }];
        let l = p * s + base[i];
        logits.push(l);
        dl.push(
            f64::from(s.abs()) * dp + 2.0 * U * (f64::from((p * s).abs()) + f64::from(l.abs())),
        );
    }
    (logits, dl)
}

fn same_bits(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same_value(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// Checks device values against host-model values bit for bit.
fn assert_host(device: &[f32], host: &[f32], what: &str) {
    assert_eq!(device.len(), host.len(), "{what} length");
    for (i, (a, b)) in device.iter().zip(host).enumerate() {
        assert!(same_bits(*a, *b), "{what}[{i}]: device {a} host {b}");
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

/// BF16 outputs against the CPU's rounded float32 values: equal, or a
/// rounding boundary within the float32 bound. Returns the boundary count.
fn assert_bf16(gpu_bf16: &[f32], cpu_f32: &[f32], bound: &[f64], what: &str) -> usize {
    let mut boundary = 0;
    for i in 0..gpu_bf16.len() {
        if !same_value(gpu_bf16[i], v41_round_bf16(cpu_f32[i])) {
            let lo = v41_round_bf16((f64::from(cpu_f32[i]) - bound[i]) as f32);
            let hi = v41_round_bf16((f64::from(cpu_f32[i]) + bound[i]) as f32);
            assert!(
                lo != hi,
                "{what}[{i}]: BF16 differs with no boundary in reach"
            );
            boundary += 1;
        }
    }
    boundary
}

/// The W1 tolerance for float32 mixing and router values against the
/// oracle: 2e-4 absolute plus 2e-4 relative. Returns the largest ratio.
fn policy_f32(values: &[f32], oracle: &[f32]) -> f64 {
    values
        .iter()
        .zip(oracle)
        .map(|(a, b)| f64::from((a - b).abs()) / (2e-4 + 2e-4 * f64::from(b.abs())))
        .fold(0.0, f64::max)
}

/// The W1 tolerance for BF16 activations: 2^-6 absolute plus 2^-6 relative.
fn policy_bf16(values: &[f32], oracle: &[f32]) -> f64 {
    values
        .iter()
        .zip(oracle)
        .map(|(a, b)| f64::from((a - b).abs()) / (0.015625 + 0.015625 * f64::from(b.abs())))
        .fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------
// Device helpers
// ---------------------------------------------------------------------------

/// The CUDA backend, or `None` to skip. `PSIONIC_REQUIRE_CUDA=1` turns a
/// missing device or unbuilt kernels into a failure.
fn cuda() -> Option<CudaBackend> {
    let backend = CudaBackend::new();
    if backend.selected_device().is_some() && backend.quantized_kernels_available() {
        return Some(backend);
    }
    assert!(
        std::env::var_os("PSIONIC_REQUIRE_CUDA").is_none(),
        "CUDA required but the device or the kernels are unavailable"
    );
    None
}

fn f32_buffer(backend: &mut CudaBackend, len: usize) -> Result<CudaBuffer, RuntimeError> {
    let device = backend
        .selected_device()
        .map(|d| d.device.clone())
        .ok_or_else(|| RuntimeError::Backend(String::from("no device")))?;
    backend.allocate_buffer(&TensorSpec::new(
        Shape::new(vec![len.max(1)]),
        DType::F32,
        device,
    ))
}

fn upload_f32(backend: &mut CudaBackend, values: &[f32]) -> Result<CudaBuffer, RuntimeError> {
    backend.byte_buffer(
        &values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

fn upload_i64(backend: &mut CudaBackend, values: &[i64]) -> Result<CudaBuffer, RuntimeError> {
    backend.byte_buffer(
        &values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

fn read_f32(buffer: &CudaBuffer, len: usize) -> Result<Vec<f32>, RuntimeError> {
    Ok(buffer.read_bytes()?[..len * 4]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn read_i32(buffer: &CudaBuffer, len: usize) -> Result<Vec<i32>, RuntimeError> {
    Ok(buffer.read_bytes()?[..len * 4]
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn bf16_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|v| half::bf16::from_f32(*v).to_le_bytes())
        .collect()
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
    fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
    /// Uniform in [-1, 1).
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    /// A BF16 value, roughly normal times `scale`.
    fn bf16(&mut self, scale: f32) -> f32 {
        let x = (self.unit() + self.unit() + self.unit()) * scale;
        v41_round_bf16(x)
    }
}

/// Runs the mixes on the device: returns (projected, mixes).
fn device_mixes(
    backend: &mut CudaBackend,
    hc: V41HyperConfig,
    projection: &[f32],
    scale: &[f32],
    base: &[f32],
    residual: &[f32],
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn Error>> {
    let projection_buffer = upload_f32(backend, projection)?;
    let scale_buffer = upload_f32(backend, scale)?;
    let base_buffer = upload_f32(backend, base)?;
    let residual_buffer = upload_f32(backend, residual)?;
    let scratch = f32_buffer(backend, hc.scratch_floats())?;
    let mixes = f32_buffer(backend, hc.mixes())?;
    let mut submission = backend.begin_submission()?;
    submission.v41_hc_mixes(
        hc,
        V41HcWeights {
            projection: &projection_buffer,
            scale: &scale_buffer,
            base: &base_buffer,
        },
        &residual_buffer,
        &scratch,
        &mixes,
    )?;
    submission.commit(CudaCommandWait::Completed)?;
    let scratch = read_f32(&scratch, hc.scratch_floats())?;
    let start = (hc.mixes() + 1) * V41_HC_SPLITS;
    Ok((scratch[start..].to_vec(), read_f32(&mixes, hc.mixes())?))
}

fn device_split(
    backend: &mut CudaBackend,
    hc: V41HyperConfig,
    projected: &[f32],
    scale: &[f32],
    base: &[f32],
) -> Result<Vec<f32>, Box<dyn Error>> {
    let projected = upload_f32(backend, projected)?;
    let scale = upload_f32(backend, scale)?;
    let base = upload_f32(backend, base)?;
    let mixes = f32_buffer(backend, hc.mixes())?;
    let mut submission = backend.begin_submission()?;
    submission.v41_hc_split_sinkhorn(hc, &projected, &scale, &base, &mixes)?;
    submission.commit(CudaCommandWait::Completed)?;
    Ok(read_f32(&mixes, hc.mixes())?)
}

fn cpu_mixes(hc: V41HyperConfig) -> HyperConnections {
    HyperConnections {
        streams: hc.streams,
        width: hc.width,
        norm_epsilon: hc.norm_epsilon,
        mix_epsilon: hc.mix_epsilon,
        sinkhorn_iterations: hc.sinkhorn_iterations,
    }
}

fn flat(m: &Mixes) -> Vec<f32> {
    m.pre
        .iter()
        .chain(&m.post)
        .chain(&m.comb)
        .copied()
        .collect()
}

/// Checks the device mixes for one case against the host model and the CPU
/// path; returns the largest |gpu - cpu| / bound and the device mixes.
fn check_mixes(
    backend: &mut CudaBackend,
    hc: V41HyperConfig,
    projection: &[f32],
    scale: &[f32],
    base: &[f32],
    residual: &[f32],
) -> Result<(f64, Vec<f32>), Box<dyn Error>> {
    let (projected, mixes) = device_mixes(backend, hc, projection, scale, base, residual)?;
    let (host_projected, host_mixes) = v41_hc_mixes_host(hc, projection, scale, base, residual);
    assert_host(&projected, &host_projected, "projected");
    assert_host(&mixes, &host_mixes, "mixes");
    let matrix = MatrixRef::new(hc.mixes(), hc.elements(), projection)?;
    let cpu = flat(&cpu_mixes(hc).mixes(residual, &matrix, scale, base)?);
    let (logits, dl) = projection_bounds(hc, projection, scale, base, residual);
    let bound = mix_bounds(hc, &logits, &dl);
    let worst = assert_bound(&mixes, &cpu, &bound, "mixes");
    Ok((worst, mixes))
}

// ---------------------------------------------------------------------------
// CPU-only checks (every machine)
// ---------------------------------------------------------------------------

#[test]
fn host_decoders_match_the_weight_loader_for_every_code() {
    for byte in 0..=255_u8 {
        assert!(same_bits(e4m3(byte), decode_f8_e4m3fn(byte)), "{byte:#x}");
        assert!(same_bits(e8m0(byte), decode_f8_e8m0fnu(byte)), "{byte:#x}");
    }
    let _ = decode_e2m1;
}

#[test]
fn configurations_refuse_bad_shapes() {
    assert!(V41HyperConfig::flash().check().is_ok());
    assert_eq!(V41HyperConfig::flash().mixes(), 24);
    assert_eq!(V41HyperConfig::flash().scratch_floats(), 25 * 16 + 24);
    for bad in [
        V41HyperConfig {
            streams: 5,
            ..V41HyperConfig::flash()
        },
        V41HyperConfig {
            streams: 0,
            ..V41HyperConfig::flash()
        },
        V41HyperConfig {
            sinkhorn_iterations: 0,
            ..V41HyperConfig::flash()
        },
        V41HyperConfig {
            mix_epsilon: 0.0,
            ..V41HyperConfig::flash()
        },
    ] {
        assert!(bad.check().is_err(), "{bad:?}");
    }
    assert!(V41Router::flash().check().is_ok());
    for bad in [
        V41Router {
            top_k: 0,
            ..V41Router::flash()
        },
        V41Router {
            top_k: 33,
            ..V41Router::flash()
        },
        V41Router {
            experts: 1025,
            ..V41Router::flash()
        },
        V41Router {
            experts: 4,
            top_k: 6,
            ..V41Router::flash()
        },
        V41Router {
            temperature: 0.0,
            ..V41Router::flash()
        },
    ] {
        assert!(bad.check().is_err(), "{bad:?}");
    }
    assert_eq!(
        V41EngramGate::flash().inv_sqrt_width(),
        (5120_f64).powf(-0.5) as f32
    );
}

/// The committed fixture decodes; the CPU loader turns the fetched Engram
/// rows into the oracle's own BF16 rows; the CPU router on logits rebuilt
/// from the oracle's scores picks the oracle's experts; and the host models
/// stay inside the tolerances of the CPU path on real inputs.
#[test]
fn real_inputs_cpu_and_host_models_agree() -> TestResult {
    let rows = Rows::load()?;
    // Engram rows: loader dequantization, then BF16, equals the oracle.
    for layer in [1, 14] {
        let codes = rows.raw(&format!("layers.{layer}.engram.embed.weight.rows"));
        let scales = rows.raw(&format!("layers.{layer}.engram.embed.scale.rows"));
        let oracle = rows.f32(&format!("layers.{layer}.engram.rows"));
        let mut loader = Vec::new();
        for r in 0..24 {
            loader.extend(dequantize_fp8_per32(
                &codes[r * 256..(r + 1) * 256],
                &scales[r * 8..(r + 1) * 8],
            )?);
        }
        assert_eq!(round_bf16(&loader)?, oracle, "layer {layer} rows");
        let ids: Vec<i64> = (0..24).collect();
        let host = v41_engram_gather_host(codes, scales, 24, &ids, 256);
        assert_host(&host, &loader, "gather host model");
    }
    // Mixes: the host model against the CPU path.
    for (layer, part, input) in mix_cases() {
        let (projection, scale, base) = mix_weights(&rows, layer, part);
        let residual = rows.f32(&input);
        let hc = V41HyperConfig::flash();
        let (_, host) = v41_hc_mixes_host(hc, &projection, &scale, &base, &residual);
        let matrix = MatrixRef::new(24, 20480, &projection)?;
        let cpu = flat(&cpu_mixes(hc).mixes(&residual, &matrix, &scale, &base)?);
        let (logits, dl) = projection_bounds(hc, &projection, &scale, &base, &residual);
        assert_bound(&host, &cpu, &mix_bounds(hc, &logits, &dl), "host mixes");
    }
    Ok(())
}

fn mix_cases() -> [(u32, &'static str, String); 2] {
    [
        (0, "attn", String::from("layers.0.hc.in")),
        (39, "ffn", String::from("layers.39.hc.attn.expanded")),
    ]
}

fn mix_weights(rows: &Rows, layer: u32, part: &str) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    (
        rows.f32(&format!("layers.{layer}.hc_{part}_fn")),
        rows.f32(&format!("layers.{layer}.hc_{part}_scale")),
        rows.f32(&format!("layers.{layer}.hc_{part}_base")),
    )
}

// ---------------------------------------------------------------------------
// Hyper-connections on the device
// ---------------------------------------------------------------------------

/// Real projection weights (layer 0 attention, layer 39 FFN) on the
/// oracle's own residuals.
#[test]
fn real_mixes_match_the_cpu_path() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    for (layer, part, input) in mix_cases() {
        let (projection, scale, base) = mix_weights(&rows, layer, part);
        let residual = rows.f32(&input);
        let (worst, mixes) = check_mixes(
            &mut backend,
            V41HyperConfig::flash(),
            &projection,
            &scale,
            &base,
            &residual,
        )?;
        let oracle: Vec<f32> = ["pre", "post", "comb"]
            .iter()
            .flat_map(|name| rows.f32(&format!("layers.{layer}.hc.{part}.{name}")))
            .collect();
        eprintln!(
            "layer {layer} {part} mixes: max |gpu - cpu| / bound {worst:.3e}; \
             against the oracle, max error / (2e-4 + 2e-4 |x|) {:.3e}",
            policy_f32(&mixes, &oracle)
        );
    }
    Ok(())
}

/// Collapse and expand at every recorded layer, from the oracle's inputs and
/// mixes: equal to the CPU path, and compared with the oracle's outputs.
#[test]
fn collapse_and_expand_match_the_cpu_path_at_every_recorded_layer() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let hc = V41HyperConfig::flash();
    let cpu = cpu_mixes(hc);
    let mut worst_policy = 0.0_f64;
    for layer in [0, 1, 2, 14, 20, 21, 39] {
        let site = |s: &str| format!("layers.{layer}.{s}");
        let mixes = |part: &str| -> Vec<f32> {
            ["pre", "post", "comb"]
                .iter()
                .flat_map(|n| rows.f32(&site(&format!("hc.{part}.{n}"))))
                .collect()
        };
        let attn_in = rows.f32(&site("hc.in"));
        let pre_in = rows.f32(&site("hc.pre_mix_in"));
        let attn_mixes = mixes("attn");
        let ffn_in = rows.f32(&site("hc.attn.expanded"));
        let ffn_mixes = mixes("ffn");
        let mut cases: Vec<(&str, Vec<f32>, Vec<f32>, Vec<f32>, String)> = vec![
            (
                "attn collapse",
                attn_in.clone(),
                pre_in,
                Vec::new(),
                site("hc.attn.collapsed"),
            ),
            (
                "ffn collapse",
                ffn_in.clone(),
                attn_mixes.clone(),
                Vec::new(),
                site("hc.ffn.collapsed"),
            ),
            (
                "attn expand",
                attn_in,
                attn_mixes,
                rows.f32(&site("attn.out")),
                site("hc.attn.expanded"),
            ),
        ];
        if rows.has(&site("ffn.out")) {
            cases.push((
                "ffn expand",
                ffn_in,
                ffn_mixes,
                rows.f32(&site("ffn.out")),
                site("hc.ffn.expanded"),
            ));
        }
        for (what, residual, mix, sublayer, expected) in cases {
            let residual_buffer = upload_f32(&mut backend, &residual)?;
            let mix_buffer = upload_f32(&mut backend, &mix)?;
            let expand = !sublayer.is_empty();
            let len = if expand { hc.elements() } else { hc.width };
            let out = f32_buffer(&mut backend, len)?;
            let out_bf16 = f32_buffer(&mut backend, len)?;
            let sub_buffer = upload_f32(&mut backend, if expand { &sublayer } else { &mix })?;
            let mut submission = backend.begin_submission()?;
            for (buffer, output) in [(&out, V41Output::F32), (&out_bf16, V41Output::Bf16)] {
                if expand {
                    submission.v41_hc_expand(
                        hc,
                        &sub_buffer,
                        &residual_buffer,
                        &mix_buffer,
                        buffer,
                        output,
                    )?;
                } else {
                    submission.v41_hc_collapse(
                        hc,
                        &residual_buffer,
                        &mix_buffer,
                        buffer,
                        output,
                    )?;
                }
            }
            submission.commit(CudaCommandWait::Completed)?;
            let gpu = read_f32(&out, len)?;
            let gpu_bf16 = read_f32(&out_bf16, len)?;
            let (host, cpu_values) = if expand {
                let m = Mixes {
                    pre: mix[..4].to_vec(),
                    post: mix[4..8].to_vec(),
                    comb: mix[8..].to_vec(),
                };
                (
                    v41_hc_expand_host(hc, &sublayer, &residual, &mix),
                    cpu.expand(&sublayer, &residual, &m)?,
                )
            } else {
                (
                    v41_hc_collapse_host(hc, &residual, &mix),
                    cpu.collapse(&residual, &mix[..4])?,
                )
            };
            assert_host(&gpu, &host, what);
            assert_host(&gpu_bf16, &v41_round_bf16_all(&host), what);
            for (i, (a, b)) in gpu.iter().zip(&cpu_values).enumerate() {
                assert!(same_value(*a, *b), "layer {layer} {what}[{i}]: {a} != {b}");
            }
            let policy = policy_bf16(&gpu_bf16, &rows.f32(&expected));
            worst_policy = worst_policy.max(policy);
            eprintln!(
                "layer {layer} {what}: equal to the CPU; against the oracle, max error / (2^-6 + 2^-6 |x|) {policy:.3e}"
            );
        }
    }
    eprintln!("collapse and expand: largest oracle ratio {worst_policy:.3e}");
    Ok(())
}

/// The mixes on synthetic inputs: the model's shape, smaller stream counts,
/// a zero residual, large and tiny residuals, and nonfinite input.
#[test]
fn synthetic_mixes_match_the_cpu_path() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(0x5eed_1234);
    for (streams, width, residual_scale) in [
        (4, 5120, 1.0_f32),
        (4, 5120, 1e-12),
        (4, 5120, 1e12),
        (4, 32, 1.0),
        (3, 1000, 1.0),
        (2, 7, 1.0),
        (1, 64, 1.0),
    ] {
        let hc = V41HyperConfig {
            streams,
            width,
            ..V41HyperConfig::flash()
        };
        let projection: Vec<f32> = (0..hc.mixes() * hc.elements())
            .map(|_| rng.unit() * 0.02)
            .collect();
        let scale = [rng.unit() * 0.5, rng.unit() * 0.5, rng.unit() * 2.0];
        let base: Vec<f32> = (0..hc.mixes()).map(|_| rng.unit()).collect();
        let residual: Vec<f32> = (0..hc.elements())
            .map(|_| rng.bf16(residual_scale))
            .collect();
        let (worst, _) = check_mixes(&mut backend, hc, &projection, &scale, &base, &residual)?;
        eprintln!(
            "{streams}x{width} residual scale {residual_scale:e}: max |gpu - cpu| / bound {worst:.3e}"
        );
    }
    // A zero residual: variance 0, every projection 0, logits = base.
    let hc = V41HyperConfig::flash();
    let projection: Vec<f32> = (0..24 * 20480).map(|_| rng.unit()).collect();
    let base: Vec<f32> = (0..24).map(|_| rng.unit()).collect();
    let (worst, mixes) = check_mixes(
        &mut backend,
        hc,
        &projection,
        &[1.0; 3],
        &base,
        &vec![0.0; 20480],
    )?;
    let cpu = flat(&cpu_mixes(hc).split_sinkhorn(&[0.0; 24], &[1.0; 3], &base)?);
    assert!(worst <= 1.0);
    assert_eq!(mixes[..8], cpu[..8]);
    // Nonfinite residual: the CPU refuses; the device writes NaN mixes.
    let mut residual = vec![1.0_f32; 20480];
    residual[77] = f32::NAN;
    let (_, mixes) = device_mixes(&mut backend, hc, &projection, &[1.0; 3], &base, &residual)?;
    assert!(mixes.iter().all(|x| x.is_nan()));
    Ok(())
}

/// The split and Sinkhorn step alone on identical projected inputs: extreme
/// logits, ties, zeros, and NaN.
#[test]
fn sinkhorn_edges_match_the_cpu_path() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let hc = V41HyperConfig::flash();
    let cpu = cpu_mixes(hc);
    let mut rng = Rng(99);
    let mut cases: Vec<(String, Vec<f32>, [f32; 3], Vec<f32>)> = vec![
        ("zeros".into(), vec![0.0; 24], [1.0; 3], vec![0.0; 24]),
        (
            "extreme".into(),
            (0..24)
                .map(|i| if i % 3 == 0 { 1000.0 } else { -1000.0 })
                .collect(),
            [1.0; 3],
            vec![0.0; 24],
        ),
        (
            "one hot rows".into(),
            (0..24)
                .map(|i| {
                    if i >= 8 && (i - 8) % 5 == 0 {
                        80.0
                    } else {
                        -80.0
                    }
                })
                .collect(),
            [1.0; 3],
            vec![0.0; 24],
        ),
        (
            "equal logits".into(),
            vec![3.5; 24],
            [1.0; 3],
            vec![0.0; 24],
        ),
        (
            "large scale".into(),
            (0..24).map(|_| rng.unit()).collect(),
            [30.0, 30.0, 60.0],
            (0..24).map(|_| rng.unit()).collect(),
        ),
    ];
    for i in 0..32 {
        let spread = [0.1_f32, 1.0, 5.0, 20.0][i % 4];
        cases.push((
            format!("random {i}"),
            (0..24).map(|_| rng.unit() * spread).collect(),
            [rng.unit(), rng.unit(), rng.unit() * 4.0],
            (0..24).map(|_| rng.unit() * spread).collect(),
        ));
    }
    let mut worst = 0.0_f64;
    for (name, projected, scale, base) in &cases {
        let gpu = device_split(&mut backend, hc, projected, scale, base)?;
        assert_host(
            &gpu,
            &v41_hc_split_sinkhorn_host(hc, projected, scale, base),
            name,
        );
        let cpu_values = flat(&cpu.split_sinkhorn(projected, scale, base)?);
        let logits: Vec<f32> = (0..24)
            .map(|i| {
                projected[i]
                    * scale[if i < 4 {
                        0
                    } else if i < 8 {
                        1
                    } else {
                        2
                    }]
                    + base[i]
            })
            .collect();
        let ratio = assert_bound(
            &gpu,
            &cpu_values,
            &mix_bounds(hc, &logits, &[0.0; 24]),
            name,
        );
        worst = worst.max(ratio);
        for row in gpu[8..].chunks_exact(4) {
            assert!(row.iter().all(|x| x.is_finite() && *x >= 0.0), "{name}");
        }
    }
    eprintln!(
        "Sinkhorn on identical inputs, {} cases: max |gpu - cpu| / bound {worst:.3e}",
        cases.len()
    );
    // Fewer streams.
    for streams in 1..=3 {
        let hc = V41HyperConfig {
            streams,
            width: 8,
            ..V41HyperConfig::flash()
        };
        let m = hc.mixes();
        let projected: Vec<f32> = (0..m).map(|_| rng.unit() * 3.0).collect();
        let base: Vec<f32> = (0..m).map(|_| rng.unit()).collect();
        let gpu = device_split(&mut backend, hc, &projected, &[1.0; 3], &base)?;
        assert_host(
            &gpu,
            &v41_hc_split_sinkhorn_host(hc, &projected, &[1.0; 3], &base),
            "streams",
        );
    }
    // NaN and infinite logits: the CPU refuses, the device writes NaN.
    for bad in [f32::NAN, f32::INFINITY] {
        let mut projected = vec![0.5_f32; 24];
        projected[13] = bad;
        assert!(
            cpu.split_sinkhorn(&projected, &[1.0; 3], &[0.0; 24])
                .is_err()
        );
        let gpu = device_split(&mut backend, hc, &projected, &[1.0; 3], &[0.0; 24])?;
        assert!(gpu.iter().all(|x| x.is_nan()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Engram on the device
// ---------------------------------------------------------------------------

/// Rows of a synthetic host table that nobody asks for hold NaN codes and
/// NaN scales, so any read of them would show in the output.
struct HostTable {
    codes: CudaHostBuffer,
    scales: CudaHostBuffer,
    host_codes: Vec<u8>,
    host_scales: Vec<u8>,
    rows: u64,
}

fn poisoned_table(
    backend: &mut CudaBackend,
    rows: usize,
    placed: &[(usize, &[u8], &[u8])],
) -> Result<HostTable, Box<dyn Error>> {
    let mut host_codes = vec![0x7f_u8; rows * 256];
    let mut host_scales = vec![0xff_u8; rows * 8];
    for (row, codes, scales) in placed {
        host_codes[row * 256..(row + 1) * 256].copy_from_slice(codes);
        host_scales[row * 8..(row + 1) * 8].copy_from_slice(scales);
    }
    let mut codes = backend.host_buffer(host_codes.len())?;
    codes.write_bytes(&host_codes)?;
    let mut scales = backend.host_buffer(host_scales.len())?;
    scales.write_bytes(&host_scales)?;
    Ok(HostTable {
        codes,
        scales,
        host_codes,
        host_scales,
        rows: rows as u64,
    })
}

fn gather_on_device(
    backend: &mut CudaBackend,
    table: &HostTable,
    ids: &[i64],
    output: V41Output,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let id_buffer = upload_i64(backend, ids)?;
    let out = f32_buffer(backend, ids.len() * 256)?;
    let mut submission = backend.begin_submission()?;
    submission.v41_engram_gather(
        V41EngramTable {
            codes: &table.codes,
            scales: &table.scales,
            rows: table.rows,
            width: 256,
        },
        &id_buffer,
        ids.len(),
        &out,
        output,
    )?;
    submission.commit(CudaCommandWait::Completed)?;
    Ok(read_f32(&out, ids.len() * 256)?)
}

/// The oracle's 24 rows at layers 1 and 14, placed in a poisoned host table:
/// the device gathers exactly the oracle's BF16 rows and the CPU loader's
/// values, and ids outside the table give NaN.
#[test]
fn engram_gather_reads_only_the_requested_rows() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    for layer in [1, 14] {
        let codes = rows.raw(&format!("layers.{layer}.engram.embed.weight.rows"));
        let scales = rows.raw(&format!("layers.{layer}.engram.embed.scale.rows"));
        let hash_ids = rows.ints(&format!("layers.{layer}.engram.hash_ids"));
        let oracle = rows.f32(&format!("layers.{layer}.engram.rows"));
        // Spread the rows over a 65,536-row table, in the oracle's id order.
        let table_rows = 65_536;
        let positions: Vec<usize> = hash_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id as usize % 2048) * 32 + i)
            .collect();
        let placed: Vec<(usize, &[u8], &[u8])> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    *p,
                    &codes[i * 256..(i + 1) * 256],
                    &scales[i * 8..(i + 1) * 8],
                )
            })
            .collect();
        let table = poisoned_table(&mut backend, table_rows, &placed)?;
        let ids: Vec<i64> = positions.iter().map(|p| *p as i64).collect();
        let gpu = gather_on_device(&mut backend, &table, &ids, V41Output::F32)?;
        let gpu_bf16 = gather_on_device(&mut backend, &table, &ids, V41Output::Bf16)?;
        let host =
            v41_engram_gather_host(&table.host_codes, &table.host_scales, table.rows, &ids, 256);
        assert_host(&gpu, &host, "gather");
        assert!(gpu.iter().all(|x| x.is_finite()));
        let mut loader = Vec::new();
        for r in 0..24 {
            loader.extend(dequantize_fp8_per32(
                &codes[r * 256..(r + 1) * 256],
                &scales[r * 8..(r + 1) * 8],
            )?);
        }
        assert_host(&gpu, &loader, "gather against the loader");
        assert_eq!(gpu_bf16, oracle, "layer {layer} rows against the oracle");
        eprintln!(
            "layer {layer}: 24 rows (hash ids {}..) equal the oracle's BF16 rows and the CPU loader",
            hash_ids[0]
        );
        // Out of range ids write NaN rows; the others are untouched.
        let ids = [ids[0], -1, table_rows as i64, i64::MAX, ids[5]];
        let gpu = gather_on_device(&mut backend, &table, &ids, V41Output::F32)?;
        assert!(gpu[256..4 * 256].iter().all(|x| x.is_nan()));
        assert_host(&gpu[..256], &loader[..256], "first row");
        assert_host(&gpu[4 * 256..], &loader[5 * 256..6 * 256], "last row");
    }
    // Edge scales and codes: 2^-127 (subnormal products), 2^127, NaN.
    let mut rng = Rng(3);
    let codes: Vec<u8> = (0..4 * 256).map(|_| rng.byte()).collect();
    let mut scales: Vec<u8> = (0..4 * 8).map(|_| 110 + rng.byte() % 20).collect();
    scales[0] = 0;
    scales[9] = 254;
    scales[17] = 0xff;
    let placed: Vec<(usize, &[u8], &[u8])> = (0..4)
        .map(|r| {
            (
                r * 3,
                &codes[r * 256..(r + 1) * 256],
                &scales[r * 8..(r + 1) * 8],
            )
        })
        .collect();
    let table = poisoned_table(&mut backend, 16, &placed)?;
    let ids = [0_i64, 3, 6, 9];
    for output in [V41Output::F32, V41Output::Bf16] {
        let gpu = gather_on_device(&mut backend, &table, &ids, output)?;
        let host = v41_engram_gather_host(&table.host_codes, &table.host_scales, 16, &ids, 256);
        let host = if output == V41Output::Bf16 {
            v41_round_bf16_all(&host)
        } else {
            host
        };
        assert_host(&gpu, &host, "edge gather");
    }
    Ok(())
}

/// The gate on the oracle's residuals at layers 1 and 14 with the real query
/// and key weights; keys and value are synthetic, since the oracle records no
/// projection output. Also zero streams, a zero dot, and NaN.
#[test]
fn engram_gate_matches_the_cpu_path() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let gate = V41EngramGate::flash();
    let (s, w) = (gate.streams, gate.width);
    let mut rng = Rng(77);
    let mut cases: Vec<(String, Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>)> = Vec::new();
    for layer in [1, 14] {
        let residual = rows.f32(&format!("layers.{layer}.engram.in"));
        let q = rows.f32(&format!("layers.{layer}.engram.q_weight"));
        let k = rows.f32(&format!("layers.{layer}.engram.k_weight"));
        for key_scale in [0.05_f32, 1.0, 30.0] {
            let projected: Vec<f32> = (0..(s + 1) * w).map(|_| rng.bf16(key_scale)).collect();
            cases.push((
                format!("layer {layer} keys x{key_scale}"),
                residual.clone(),
                projected,
                q.clone(),
                k.clone(),
            ));
        }
        // Keys equal to the residual: a large positive dot.
        let mut projected = residual.clone();
        projected.extend((0..w).map(|_| rng.bf16(1.0)));
        cases.push((
            format!("layer {layer} keys = residual"),
            residual.clone(),
            projected,
            q.clone(),
            k.clone(),
        ));
    }
    // A zero stream and zero keys (dot 0, the 1e-6 floor), and a stream whose
    // dot is negative.
    let mut residual: Vec<f32> = (0..s * w).map(|_| rng.bf16(1.0)).collect();
    residual[..w].fill(0.0);
    let mut projected: Vec<f32> = (0..(s + 1) * w).map(|_| rng.bf16(1.0)).collect();
    projected[w..2 * w].fill(0.0);
    for d in 0..w {
        projected[2 * w + d] = -residual[2 * w + d];
    }
    let ones = vec![1.0_f32; s * w];
    cases.push((
        "zero stream, zero keys, negative dot".into(),
        residual,
        projected,
        ones.clone(),
        ones.clone(),
    ));
    let cpu = EngramGate {
        streams: s,
        width: w,
        epsilon: gate.epsilon,
    };
    let depth = block_depth(w);
    for (name, residual, projected, q, k) in &cases {
        let (gpu, gpu_bf16) = engram_gate_on_device(&mut backend, gate, residual, projected, q, k)?;
        let host = v41_engram_gate_host(gate, residual, projected, q, k);
        assert_host(&gpu, &host, name);
        assert_host(&gpu_bf16, &v41_round_bf16_all(&host), name);
        let cpu_values = cpu.apply(
            residual,
            &projected[..s * w],
            &projected[s * w..],
            q,
            k,
            true,
        )?;
        // The bound, stream by stream.
        let value = &projected[s * w..];
        let mut bound = Vec::with_capacity(s * w);
        for h in 0..s {
            let x = &residual[h * w..(h + 1) * w];
            let key = &projected[h * w..(h + 1) * w];
            let qk: Vec<f32> = (0..w).map(|d| q[h * w + d] * k[h * w + d]).collect();
            let factor = reorder(w, depth);
            let xs: f32 = x.iter().map(|v| v * v).sum();
            let ks: f32 = key.iter().map(|v| v * v).sum();
            let rel = |sum: f32, terms: f64| if sum > 0.0 { factor * terms / f64::from(sum) } else { 0.0 } + 2.0 * U;
            let rel_x = rel(xs, x.iter().map(|v| f64::from(v * v)).sum());
            let rel_k = rel(ks, key.iter().map(|v| f64::from(v * v)).sum());
            let xvar = xs / w as f32;
            let kvar = ks / w as f32;
            let rstd = (xvar + gate.epsilon).sqrt().recip() * (kvar + gate.epsilon).sqrt().recip();
            let rel_rstd =
                0.5 * (rel_x + 2.0 * U) + 4.0 * U + 0.5 * (rel_k + 2.0 * U) + 4.0 * U + 2.0 * U;
            let terms: Vec<f32> = (0..w).map(|d| x[d] * qk[d] * key[d]).collect();
            let dot: f32 = terms.iter().sum();
            let c = gate.inv_sqrt_width();
            let scaled = dot * rstd * c;
            let d_scaled = factor
                * terms.iter().map(|t| f64::from(t.abs())).sum::<f64>()
                * f64::from(rstd)
                * f64::from(c)
                + f64::from(scaled.abs()) * (rel_rstd + 4.0 * U);
            let root = |z: f64| z.abs().max(1e-6).sqrt().copysign(z);
            let z = f64::from(scaled);
            let y = root(z);
            let dy = root(z + d_scaled) - root(z - d_scaled) + 2.0 * U * y.abs();
            let g = sigmoid64(y);
            let dg = sigmoid64(y + dy) - sigmoid64(y - dy) + 12.0 * U * g;
            for d in 0..w {
                let v = f64::from(value[d]);
                let out = f64::from(cpu_values[h * w + d]);
                bound.push(dg * v.abs() + 2.0 * U * ((g * v).abs() + out.abs()));
            }
        }
        let worst = assert_bound(&gpu, &cpu_values, &bound, name);
        let boundary = assert_bf16(&gpu_bf16, &cpu_values, &bound, name);
        eprintln!(
            "Engram gate, {name}: max |gpu - cpu| / bound {worst:.3e}, BF16 boundary cases {boundary}"
        );
    }
    // NaN in one stream: the CPU refuses; the device writes NaN there only.
    let mut residual: Vec<f32> = (0..s * w).map(|_| rng.bf16(1.0)).collect();
    residual[3 * w + 5] = f32::NAN;
    let projected: Vec<f32> = (0..(s + 1) * w).map(|_| rng.bf16(1.0)).collect();
    assert!(
        cpu.apply(
            &residual,
            &projected[..s * w],
            &projected[s * w..],
            &ones,
            &ones,
            true
        )
        .is_err()
    );
    let (gpu, _) = engram_gate_on_device(&mut backend, gate, &residual, &projected, &ones, &ones)?;
    assert!(gpu[..3 * w].iter().all(|x| x.is_finite()));
    assert!(gpu[3 * w..].iter().all(|x| x.is_nan()));
    Ok(())
}

fn engram_gate_on_device(
    backend: &mut CudaBackend,
    gate: V41EngramGate,
    residual: &[f32],
    projected: &[f32],
    q: &[f32],
    k: &[f32],
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn Error>> {
    let n = gate.streams * gate.width;
    let residual = upload_f32(backend, residual)?;
    let projected = upload_f32(backend, projected)?;
    let q = upload_f32(backend, q)?;
    let k = upload_f32(backend, k)?;
    let out = f32_buffer(backend, n)?;
    let out_bf16 = f32_buffer(backend, n)?;
    let mut submission = backend.begin_submission()?;
    for (buffer, output) in [(&out, V41Output::F32), (&out_bf16, V41Output::Bf16)] {
        submission.v41_engram_gate(gate, &residual, &projected, &q, &k, buffer, output)?;
    }
    submission.commit(CudaCommandWait::Completed)?;
    Ok((read_f32(&out, n)?, read_f32(&out_bf16, n)?))
}

// ---------------------------------------------------------------------------
// Routing on the device
// ---------------------------------------------------------------------------

/// Bounds on each biased score's difference given bounds on the logits.
fn score_bounds(
    router: V41Router,
    logits: &[f32],
    bias: &[f32],
    dl: &[f64],
) -> (Vec<f32>, Vec<f32>, Vec<f64>, Vec<f64>) {
    let mut scores = Vec::new();
    let mut ranked = Vec::new();
    let mut ds = Vec::new();
    let mut dr = Vec::new();
    for e in 0..router.experts {
        let x = logits[e] / router.temperature;
        let s = if x > 20.0 {
            x.sqrt()
        } else {
            x.exp().ln_1p().sqrt()
        };
        let r = s + bias[e];
        let dx = dl[e] / f64::from(router.temperature);
        let xf = f64::from(x);
        let mut bound = softplus_root(xf + dx) - softplus_root(xf - dx)
            + 6.0 * U * f64::from(s)
            + 2f64.powi(-60);
        if (xf - 20.0).abs() <= dx {
            bound += 1e-9;
        }
        scores.push(s);
        ranked.push(r);
        ds.push(bound);
        dr.push(bound + 2.0 * U * f64::from(r.abs()));
    }
    (scores, ranked, ds, dr)
}

/// Compares the device's routing with the CPU's. Returns `false` (and
/// checks nothing more) when the bound lets a selected and an unselected
/// expert swap.
fn check_route(
    router: V41Router,
    logits: &[f32],
    bias: &[f32],
    dl: &[f64],
    ids: &[i32],
    weights: &[f32],
    what: &str,
) -> Result<bool, Box<dyn Error>> {
    let cpu = Router {
        top_k: router.top_k,
        temperature: router.temperature,
        normalize: router.normalize,
        route_scale: router.route_scale,
    }
    .select(logits, bias)?;
    let (scores, ranked, ds, dr) = score_bounds(router, logits, bias, dl);
    let selected: Vec<usize> = cpu.indices.clone();
    let near = selected.iter().any(|i| {
        (0..router.experts)
            .filter(|j| !selected.contains(j))
            .any(|j| f64::from(ranked[*i]) - f64::from(ranked[j]) <= dr[*i] + dr[j])
    });
    let mut cpu_ids: Vec<i32> = cpu.indices.iter().map(|i| *i as i32).collect();
    cpu_ids.sort_unstable();
    if near {
        eprintln!(
            "{what}: a selected and an unselected expert are within the bound, so ids aren't \
             required to match; they {}",
            if ids == cpu_ids.as_slice() {
                "do"
            } else {
                "don't"
            }
        );
        return Ok(false);
    }
    let mut pairs: Vec<(usize, f32)> = cpu
        .indices
        .iter()
        .copied()
        .zip(cpu.weights.iter().copied())
        .collect();
    pairs.sort_by_key(|(id, _)| *id);
    let expected: Vec<i32> = pairs.iter().map(|(id, _)| *id as i32).collect();
    assert_eq!(ids, expected.as_slice(), "{what} ids");
    let total: f64 = selected.iter().map(|i| f64::from(scores[*i])).sum();
    let dtotal: f64 = selected.iter().map(|i| ds[*i]).sum();
    let scale = f64::from(router.route_scale);
    let bound: Vec<f64> = pairs
        .iter()
        .map(|(id, w)| {
            if router.normalize && router.top_k > 1 {
                scale * ds[*id] / total + f64::from(*w) * (dtotal / total + 16.0 * U)
            } else {
                scale * ds[*id] + f64::from(*w) * 4.0 * U
            }
        })
        .collect();
    let cpu_weights: Vec<f32> = pairs.iter().map(|(_, w)| *w).collect();
    assert_bound(weights, &cpu_weights, &bound, what);
    Ok(true)
}

fn route_on_device(
    backend: &mut CudaBackend,
    router: V41Router,
    logits: &[f32],
    bias: &[f32],
) -> Result<(Vec<i32>, Vec<f32>), Box<dyn Error>> {
    let logits = upload_f32(backend, logits)?;
    let bias = upload_f32(backend, bias)?;
    let ids = backend.byte_buffer(&vec![0; router.top_k * 4])?;
    let weights = f32_buffer(backend, router.top_k)?;
    let mut submission = backend.begin_submission()?;
    submission.v41_route(router, &logits, &bias, &ids, &weights)?;
    submission.commit(CudaCommandWait::Completed)?;
    Ok((
        read_i32(&ids, router.top_k)?,
        read_f32(&weights, router.top_k)?,
    ))
}

/// The oracle's ids, unless its 6th and 7th biased scores are exactly equal.
fn oracle_ids(rows: &Rows, layer: u32) -> Option<Vec<i32>> {
    let biased = rows.f32(&format!("layers.{layer}.ffn.gate.biased_scores"));
    let mut sorted = biased.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    if sorted[5] == sorted[6] {
        return None;
    }
    let mut ids: Vec<i32> = rows
        .ints(&format!("layers.{layer}.ffn.gate.ids"))
        .iter()
        .map(|i| *i as i32)
        .collect();
    ids.sort_unstable();
    Some(ids)
}

/// Layer 14's real BF16 router on the oracle's gate input: logits within
/// the reduction bound, then the routing, against the CPU and the oracle.
#[test]
fn real_router_matches_the_cpu_path() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let router = V41Router::flash();
    let (experts, cols) = (384, 5120);
    let weight = rows.f32("layers.14.ffn.gate.weight");
    let weight_bytes = rows.raw("layers.14.ffn.gate.weight").to_vec();
    let input = rows.f32("layers.14.ffn.gate.in");
    let bias = rows.f32("layers.14.ffn.gate.bias");
    let weight_buffer = backend.byte_buffer(&weight_bytes)?;
    let input_buffer = upload_f32(&mut backend, &input)?;
    let logits_buffer = f32_buffer(&mut backend, experts)?;
    let bias_buffer = upload_f32(&mut backend, &bias)?;
    let ids_buffer = backend.byte_buffer(&[0; 24])?;
    let weights_buffer = f32_buffer(&mut backend, 6)?;
    let mut submission = backend.begin_submission()?;
    submission.v41_gate_logits(&weight_buffer, &input_buffer, &logits_buffer, experts, cols)?;
    submission.v41_route(
        router,
        &logits_buffer,
        &bias_buffer,
        &ids_buffer,
        &weights_buffer,
    )?;
    submission.commit(CudaCommandWait::Completed)?;
    let gpu_logits = read_f32(&logits_buffer, experts)?;
    assert_host(
        &gpu_logits,
        &v41_gate_logits_host(&weight_bytes, &input, experts, cols),
        "logits",
    );
    let cpu_logits = matvec(&MatrixRef::new(experts, cols, &weight)?, &input)?;
    let factor = reorder(cols, gate_depth(cols));
    let dl: Vec<f64> = (0..experts)
        .map(|e| {
            factor
                * weight[e * cols..(e + 1) * cols]
                    .iter()
                    .zip(&input)
                    .map(|(w, x)| f64::from((w * x).abs()))
                    .sum::<f64>()
        })
        .collect();
    let worst = assert_bound(&gpu_logits, &cpu_logits, &dl, "logits");
    let ids = read_i32(&ids_buffer, 6)?;
    let weights = read_f32(&weights_buffer, 6)?;
    let (host_ids, host_weights) = v41_route_host(router, &gpu_logits, &bias);
    assert_eq!(ids, host_ids);
    assert_host(&weights, &host_weights, "weights");
    let compared = check_route(router, &cpu_logits, &bias, &dl, &ids, &weights, "layer 14")?;
    let oracle = oracle_ids(&rows, 14);
    if let Some(expected) = &oracle {
        assert_eq!(&ids, expected, "layer 14 against the oracle");
    }
    let mut oracle_weights: Vec<(i64, f32)> = rows
        .ints("layers.14.ffn.gate.ids")
        .into_iter()
        .zip(rows.f32("layers.14.ffn.gate.weights"))
        .collect();
    oracle_weights.sort_by_key(|(id, _)| *id);
    let oracle_weights: Vec<f32> = oracle_weights.iter().map(|(_, w)| *w).collect();
    let scores = rows.f32("layers.14.ffn.gate.scores");
    let (gpu_scores, _, _, _) = score_bounds(router, &gpu_logits, &bias, &vec![0.0; experts]);
    eprintln!(
        "layer 14 router: logits max |gpu - cpu| / bound {worst:.3e}; ids {ids:?} {} the CPU, {} the oracle; \
         against the oracle, scores {:.3e} and weights {:.3e} of (2e-4 + 2e-4 |x|)",
        if compared {
            "equal"
        } else {
            "not compared with"
        },
        if oracle.is_some() {
            "equal"
        } else {
            "(exact tie) not compared with"
        },
        policy_f32(&gpu_scores, &scores),
        policy_f32(&weights, &oracle_weights),
    );
    Ok(())
}

/// Routing at layers 0, 2, 14, 20, and 39 from logits rebuilt from the
/// oracle's scores (the device and the CPU get identical logits).
#[test]
fn routing_matches_the_cpu_path_on_oracle_scores() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    let router = V41Router::flash();
    for layer in [0, 2, 14, 20, 39] {
        let scores = rows.f32(&format!("layers.{layer}.ffn.gate.scores"));
        let bias = rows.f32(&format!("layers.{layer}.ffn.gate.bias"));
        // score = sqrt(softplus(logit)), so logit = log(expm1(score^2)), or
        // score^2 above 20.
        let logits: Vec<f32> = scores
            .iter()
            .map(|s| {
                let y = f64::from(*s) * f64::from(*s);
                (if y > 20.0 { y } else { y.exp_m1().ln() }) as f32
            })
            .collect();
        let (ids, weights) = route_on_device(&mut backend, router, &logits, &bias)?;
        let (host_ids, host_weights) = v41_route_host(router, &logits, &bias);
        assert_eq!(ids, host_ids, "layer {layer}");
        assert_host(&weights, &host_weights, "weights");
        let compared = check_route(
            router,
            &logits,
            &bias,
            &[0.0; 384],
            &ids,
            &weights,
            &format!("layer {layer}"),
        )?;
        let oracle = oracle_ids(&rows, layer);
        if let Some(expected) = &oracle {
            assert_eq!(&ids, expected, "layer {layer} against the oracle");
        }
        eprintln!(
            "layer {layer}: ids {ids:?} {} the CPU, {} the oracle",
            if compared {
                "equal"
            } else {
                "not compared with"
            },
            if oracle.is_some() {
                "equal"
            } else {
                "(exact tie) not compared with"
            },
        );
    }
    Ok(())
}

/// Ties (lowest id wins), equal logits, extreme logits, the softplus
/// threshold, large biases, NaN and infinite input, top-1, and the largest
/// expert count.
#[test]
fn routing_edges_match_the_cpu_path() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let flash = V41Router::flash();
    let mut rng = Rng(5);
    let mut cases: Vec<(String, V41Router, Vec<f32>, Vec<f32>)> = vec![
        ("equal".into(), flash, vec![0.0; 384], vec![0.0; 384]),
        (
            "ties at the boundary".into(),
            flash,
            (0..384)
                .map(|e| if e % 50 == 7 { 2.0 } else { -1.0 })
                .collect(),
            vec![0.0; 384],
        ),
        (
            "extreme".into(),
            flash,
            (0..384)
                .map(|e| [1e30, -1e30, 1e4, -1e4, 0.0][e % 5])
                .collect(),
            vec![0.0; 384],
        ),
        (
            "threshold".into(),
            flash,
            (0..384).map(|e| 20.0 + (e as f32 - 192.0) * 1e-3).collect(),
            vec![0.0; 384],
        ),
        (
            "bias decides".into(),
            flash,
            (0..384).map(|_| rng.unit()).collect(),
            (0..384)
                .map(|e| if e % 61 == 0 { 50.0 } else { 0.0 })
                .collect(),
        ),
        (
            "top-1".into(),
            V41Router { top_k: 1, ..flash },
            (0..384).map(|_| rng.unit() * 4.0).collect(),
            (0..384).map(|_| rng.unit() * 0.1).collect(),
        ),
        (
            "seven experts".into(),
            V41Router {
                experts: 7,
                ..flash
            },
            (0..7).map(|_| rng.unit()).collect(),
            vec![0.0; 7],
        ),
        (
            "unnormalized".into(),
            V41Router {
                normalize: false,
                ..flash
            },
            (0..384).map(|_| rng.unit() * 3.0).collect(),
            vec![0.0; 384],
        ),
        (
            "temperature".into(),
            V41Router {
                temperature: 0.7,
                ..flash
            },
            (0..384).map(|_| rng.unit() * 3.0).collect(),
            (0..384).map(|_| rng.unit() * 0.01).collect(),
        ),
        (
            "1024 experts, top 32".into(),
            V41Router {
                experts: 1024,
                top_k: 32,
                ..flash
            },
            (0..1024).map(|_| rng.unit() * 5.0).collect(),
            (0..1024).map(|_| rng.unit() * 0.1).collect(),
        ),
    ];
    for i in 0..16 {
        cases.push((
            format!("random {i}"),
            flash,
            (0..384).map(|_| rng.unit() * 6.0).collect(),
            (0..384).map(|_| rng.unit() * 0.05).collect(),
        ));
    }
    for (name, router, logits, bias) in &cases {
        let (ids, weights) = route_on_device(&mut backend, *router, logits, bias)?;
        let (host_ids, host_weights) = v41_route_host(*router, logits, bias);
        assert_eq!(ids, host_ids, "{name}");
        assert_host(&weights, &host_weights, name);
        let compared = check_route(
            *router,
            logits,
            bias,
            &vec![0.0; router.experts],
            &ids,
            &weights,
            name,
        )?;
        if name == "equal" {
            assert_eq!(ids, [0, 1, 2, 3, 4, 5]);
        }
        if name == "ties at the boundary" {
            // Eight experts tie at the top; the six lowest ids win.
            assert_eq!(ids, [7, 57, 107, 157, 207, 257]);
        }
        eprintln!(
            "{name}: ids {} the CPU",
            if compared {
                "equal"
            } else {
                "not compared with"
            }
        );
    }
    // Nonfinite input: the CPU refuses; the device writes -1 and NaN.
    for (logit, bias) in [
        (f32::NAN, 0.0),
        (f32::INFINITY, 0.0),
        (0.0, f32::INFINITY),
        (0.0, f32::NAN),
    ] {
        let mut logits = vec![0.5_f32; 384];
        let mut biases = vec![0.0_f32; 384];
        logits[100] = logit;
        biases[200] = bias;
        assert!(Router::flash().select(&logits, &biases).is_err());
        let (ids, weights) = route_on_device(&mut backend, flash, &logits, &biases)?;
        assert_eq!(ids, [-1; 6]);
        assert!(weights.iter().all(|w| w.is_nan()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Experts on the device
// ---------------------------------------------------------------------------

/// A diagonal matrix: with an all-ones input, `linear` returns the diagonal
/// exactly, which exposes the CPU expert's element-wise SwiGLU.
struct Diagonal(Vec<f32>);

impl MatrixRows for Diagonal {
    fn dimensions(&self) -> (usize, usize) {
        (self.0.len(), self.0.len())
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        let mut row = vec![0.0; self.0.len()];
        row[index] = self.0[index];
        Ok(row)
    }
}

/// The CPU expert's SwiGLU output for given w1 and w3 outputs (float32,
/// before its BF16 cast).
fn cpu_swiglu(gate: &[f32], up: &[f32], weight: f32, limit: f32) -> Result<Vec<f32>, CpuError> {
    let n = gate.len();
    moe::expert_with_arithmetic(
        &vec![1.0; n],
        &Diagonal(gate.to_vec()),
        &Diagonal(vec![1.0; n]),
        &Diagonal(up.to_vec()),
        weight,
        limit,
        Arithmetic::Float32,
    )
}

fn swiglu_bound(cpu: &[f32]) -> Vec<f64> {
    cpu.iter()
        .map(|h| 20.0 * U * f64::from(h.abs()) + 2f64.powi(-126))
        .collect()
}

fn swiglu_on_device(
    backend: &mut CudaBackend,
    gate: &[f32],
    up: &[f32],
    weights: Option<&[f32]>,
    width: usize,
    limit: f32,
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn Error>> {
    let n = gate.len();
    let slots = n / width;
    let gate = upload_f32(backend, gate)?;
    let up = upload_f32(backend, up)?;
    let weights = weights.map(|w| upload_f32(backend, w)).transpose()?;
    let out = f32_buffer(backend, n)?;
    let out_bf16 = f32_buffer(backend, n)?;
    let mut submission = backend.begin_submission()?;
    for (buffer, output) in [(&out, V41Output::F32), (&out_bf16, V41Output::Bf16)] {
        submission.v41_swiglu(
            &gate,
            &up,
            weights.as_ref(),
            slots,
            width,
            limit,
            buffer,
            output,
        )?;
    }
    submission.commit(CudaCommandWait::Completed)?;
    Ok((read_f32(&out, n)?, read_f32(&out_bf16, n)?))
}

/// The oracle's routed expert at layers 2 and 20: its w1 and w3 outputs and
/// combine weight give its BF16 down-projection input.
#[test]
fn swiglu_matches_the_cpu_path_and_the_oracle() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    for layer in [2, 20] {
        let gate = rows.f32(&format!("layers.{layer}.ffn.expert.w1_out"));
        let up = rows.f32(&format!("layers.{layer}.ffn.expert.w3_out"));
        let weight = rows.f32(&format!("layers.{layer}.ffn.expert.combine_weight"))[0];
        let oracle = rows.f32(&format!("layers.{layer}.ffn.expert.w2_in"));
        let (gpu, gpu_bf16) =
            swiglu_on_device(&mut backend, &gate, &up, Some(&[weight]), 2304, 10.0)?;
        assert_host(
            &gpu,
            &v41_swiglu_host(&gate, &up, Some(&[weight]), 2304, 10.0),
            "SwiGLU",
        );
        let cpu = cpu_swiglu(&gate, &up, weight, 10.0)?;
        let bound = swiglu_bound(&cpu);
        let worst = assert_bound(&gpu, &cpu, &bound, "SwiGLU");
        let boundary = assert_bf16(&gpu_bf16, &cpu, &bound, "SwiGLU");
        let equal = gpu_bf16.iter().zip(&oracle).filter(|(a, b)| a == b).count();
        eprintln!(
            "layer {layer} expert {}: max |gpu - cpu| / bound {worst:.3e}, BF16 boundary cases {boundary}; \
             BF16 equal to the oracle's w2 input {equal}/2304, max error / (2^-6 + 2^-6 |x|) {:.3e}",
            rows.ints(&format!("layers.{layer}.ffn.expert.id"))[0],
            policy_bf16(&gpu_bf16, &oracle)
        );
    }
    // Clamps, extreme gates, several slots with their own weights, no clamp,
    // and NaN.
    let mut rng = Rng(8);
    let width = 512;
    let slots = 6;
    let gate: Vec<f32> = (0..slots * width)
        .map(|i| match i % 8 {
            0 => 25.0,
            1 => -120.0,
            2 => -90.0,
            3 => 10.0,
            _ => rng.bf16(6.0),
        })
        .collect();
    let up: Vec<f32> = (0..slots * width)
        .map(|i| match i % 7 {
            0 => 40.0,
            1 => -40.0,
            _ => rng.bf16(6.0),
        })
        .collect();
    let weights: Vec<f32> = (0..slots).map(|_| rng.unit().abs() * 1.5).collect();
    for limit in [10.0_f32, 0.0] {
        let (gpu, gpu_bf16) =
            swiglu_on_device(&mut backend, &gate, &up, Some(&weights), width, limit)?;
        assert_host(
            &gpu,
            &v41_swiglu_host(&gate, &up, Some(&weights), width, limit),
            "synthetic",
        );
        for s in 0..slots {
            let range = s * width..(s + 1) * width;
            let cpu = cpu_swiglu(&gate[range.clone()], &up[range.clone()], weights[s], limit)?;
            let bound = swiglu_bound(&cpu);
            assert_bound(&gpu[range.clone()], &cpu, &bound, "synthetic");
            assert_bf16(&gpu_bf16[range], &cpu, &bound, "synthetic");
        }
    }
    let (shared, _) = swiglu_on_device(
        &mut backend,
        &gate[..width],
        &up[..width],
        None,
        width,
        10.0,
    )?;
    assert_host(
        &shared,
        &v41_swiglu_host(&gate[..width], &up[..width], None, width, 10.0),
        "shared",
    );
    let mut bad = gate[..width].to_vec();
    bad[3] = f32::NAN;
    let (gpu, _) = swiglu_on_device(&mut backend, &bad, &up[..width], None, width, 10.0)?;
    assert!(gpu[3].is_nan());
    assert_eq!(gpu.iter().filter(|x| x.is_nan()).count(), 1);
    Ok(())
}

/// Canned expert outputs keyed by id (`None` is the shared expert).
struct Canned {
    outputs: Vec<Vec<f32>>,
    shared: Vec<f32>,
}

impl ExpertProvider for Canned {
    fn forward(&self, id: Option<usize>, _: &[f32], _: f32, _: f32) -> Result<Vec<f32>, CpuError> {
        Ok(match id {
            Some(id) => self.outputs[id].clone(),
            None => self.shared.clone(),
        })
    }
}

/// Routing from logits and the expert sum, against `moe::forward` with the
/// same expert outputs. Integer-valued BF16 gate inputs keep every logit
/// exact, so ids and sums must match exactly.
#[test]
fn moe_sum_matches_the_cpu_forward() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(21);
    let (experts, cols, width) = (384, 5120, 5120);
    for case in 0..4 {
        let gate: Vec<f32> = (0..experts * cols)
            .map(|_| (rng.next() % 5) as f32 - 2.0)
            .collect();
        let input: Vec<f32> = (0..cols).map(|_| (rng.next() % 3) as f32 - 1.0).collect();
        let bias: Vec<f32> = (0..experts).map(|_| rng.unit() * 0.25).collect();
        let canned = Canned {
            outputs: (0..experts)
                .map(|_| (0..width).map(|_| rng.bf16(0.5)).collect())
                .collect(),
            shared: (0..width).map(|_| rng.bf16(0.5)).collect(),
        };
        let (routing, cpu_out) = moe::forward(
            &input,
            &MatrixRef::new(experts, cols, &gate)?,
            &bias,
            Router::flash(),
            &canned,
            10.0,
        )?;
        let gate_buffer = backend.byte_buffer(&bf16_bytes(&gate))?;
        let input_buffer = upload_f32(&mut backend, &input)?;
        let bias_buffer = upload_f32(&mut backend, &bias)?;
        let logits = f32_buffer(&mut backend, experts)?;
        let ids = backend.byte_buffer(&[0; 24])?;
        let weights = f32_buffer(&mut backend, 6)?;
        let mut submission = backend.begin_submission()?;
        submission.v41_gate_logits(&gate_buffer, &input_buffer, &logits, experts, cols)?;
        submission.v41_route(V41Router::flash(), &logits, &bias_buffer, &ids, &weights)?;
        submission.commit(CudaCommandWait::Completed)?;
        let ids = read_i32(&ids, 6)?;
        let mut expected: Vec<usize> = routing.indices.clone();
        expected.sort_unstable();
        assert_eq!(
            ids,
            expected.iter().map(|i| *i as i32).collect::<Vec<_>>(),
            "case {case}"
        );
        let routed: Vec<f32> = ids
            .iter()
            .flat_map(|id| canned.outputs[*id as usize].clone())
            .collect();
        let routed_buffer = upload_f32(&mut backend, &routed)?;
        let shared_buffer = upload_f32(&mut backend, &canned.shared)?;
        let out = f32_buffer(&mut backend, width)?;
        let mut submission = backend.begin_submission()?;
        submission.v41_moe_combine(
            &routed_buffer,
            6,
            Some(&shared_buffer),
            width,
            &out,
            V41Output::F32,
        )?;
        submission.commit(CudaCommandWait::Completed)?;
        let gpu = read_f32(&out, width)?;
        assert_host(
            &gpu,
            &v41_moe_combine_host(&routed, 6, Some(&canned.shared), width),
            "sum",
        );
        for (i, (a, b)) in gpu.iter().zip(&cpu_out).enumerate() {
            assert!(same_value(*a, *b), "case {case} [{i}]: {a} != {b}");
        }
    }
    Ok(())
}

/// A packed expert matrix on the host, read by the CPU path through the
/// weight loader's dequantization.
struct Packed {
    format: V41MatvecFormat,
    rows: usize,
    cols: usize,
    weights: Vec<u8>,
    scales: Vec<u8>,
}

impl Packed {
    fn synthetic(rng: &mut Rng, format: V41MatvecFormat, rows: usize, cols: usize) -> Self {
        let weights = (0..format.weight_bytes(rows, cols))
            .map(|_| {
                let byte = rng.byte();
                if format == V41MatvecFormat::Fp8Block32x32 && byte & 0x7f == 0x7f {
                    byte ^ 1
                } else {
                    byte
                }
            })
            .collect();
        let scales = (0..format.scale_bytes(rows, cols))
            .map(|_| 118 + rng.byte() % 6)
            .collect();
        Self {
            format,
            rows,
            cols,
            weights,
            scales,
        }
    }
}

impl MatrixRows for Packed {
    fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        let blocks = self.cols / V41_BLOCK;
        Ok(match self.format {
            V41MatvecFormat::Fp4E2M1Row32 => {
                psionic_models::deepseek_v41_weights::dequantize_fp4_row(
                    &self.weights[index * self.cols / 2..(index + 1) * self.cols / 2],
                    &self.scales[index * blocks..(index + 1) * blocks],
                )?
            }
            V41MatvecFormat::Fp8Block32x32 => {
                let b = index / V41_BLOCK;
                dequantize_fp8_per32(
                    &self.weights[index * self.cols..(index + 1) * self.cols],
                    &self.scales[b * blocks..(b + 1) * blocks],
                )?
            }
            V41MatvecFormat::Q2Row32 | V41MatvecFormat::Q3Row32 => {
                unreachable!("the CPU path has no consumer-q matrices")
            }
        })
    }
    fn precision(&self) -> MatrixPrecision {
        match self.format {
            V41MatvecFormat::Fp4E2M1Row32 => MatrixPrecision::Fp4,
            V41MatvecFormat::Fp8Block32x32 => MatrixPrecision::Fp8,
            V41MatvecFormat::Q2Row32 | V41MatvecFormat::Q3Row32 => {
                unreachable!("the CPU path has no consumer-q matrices")
            }
        }
    }
}

struct Bank<'a> {
    w1: &'a [Packed],
    w2: &'a [Packed],
    w3: &'a [Packed],
    shared: &'a [Packed; 3],
}

impl ExpertProvider for Bank<'_> {
    fn forward(
        &self,
        id: Option<usize>,
        input: &[f32],
        weight: f32,
        limit: f32,
    ) -> Result<Vec<f32>, CpuError> {
        let (w1, w2, w3) = match id {
            Some(i) => (&self.w1[i], &self.w2[i], &self.w3[i]),
            None => (&self.shared[0], &self.shared[1], &self.shared[2]),
        };
        moe::expert_with_arithmetic(input, w1, w2, w3, weight, limit, Arithmetic::ReferenceBf16)
    }
}

fn matrix<'a>(
    weights: &'a CudaBuffer,
    scales: &'a CudaBuffer,
    format: V41MatvecFormat,
    rows: usize,
    cols: usize,
    matrices: usize,
) -> V41Matrix<'a> {
    V41Matrix {
        format,
        weights,
        scales,
        rows,
        cols,
        matrices,
    }
}

fn stack(
    backend: &mut CudaBackend,
    matrices: &[Packed],
) -> Result<(CudaBuffer, CudaBuffer), RuntimeError> {
    let w: Vec<u8> = matrices.iter().flat_map(|m| m.weights.clone()).collect();
    let s: Vec<u8> = matrices.iter().flat_map(|m| m.scales.clone()).collect();
    Ok((backend.byte_buffer(&w)?, backend.byte_buffer(&s)?))
}

/// One whole MoE step on the device in one submission, with no host round
/// trip: router logits, routing, then the six FP4 experts picked by the
/// device ids (W4's `V41Slots`), SwiGLU with the device route weights, the
/// FP8 shared expert, and the sum. Equal to the host models' chain bit for
/// bit; ids equal to `moe::forward`'s, and the output within the W1 BF16
/// tolerance of it.
#[test]
fn moe_step_runs_from_device_ids() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(1234);
    let (experts, hidden, inter, top_k) = (16, 256, 128, 6);
    let router = V41Router {
        experts,
        ..V41Router::flash()
    };
    let fp4 = V41MatvecFormat::Fp4E2M1Row32;
    let fp8 = V41MatvecFormat::Fp8Block32x32;
    let w1: Vec<Packed> = (0..experts)
        .map(|_| Packed::synthetic(&mut rng, fp4, inter, hidden))
        .collect();
    let w3: Vec<Packed> = (0..experts)
        .map(|_| Packed::synthetic(&mut rng, fp4, inter, hidden))
        .collect();
    let w2: Vec<Packed> = (0..experts)
        .map(|_| Packed::synthetic(&mut rng, fp4, hidden, inter))
        .collect();
    let shared = [
        Packed::synthetic(&mut rng, fp8, inter, hidden),
        Packed::synthetic(&mut rng, fp8, hidden, inter),
        Packed::synthetic(&mut rng, fp8, inter, hidden),
    ];
    let gate: Vec<f32> = (0..experts * hidden).map(|_| rng.bf16(0.05)).collect();
    let bias: Vec<f32> = (0..experts).map(|_| rng.unit() * 0.01).collect();
    let input: Vec<f32> = (0..hidden).map(|_| rng.bf16(1.0)).collect();

    // Device chain.
    let gate_buffer = backend.byte_buffer(&bf16_bytes(&gate))?;
    let bias_buffer = upload_f32(&mut backend, &bias)?;
    let input_buffer = upload_f32(&mut backend, &input)?;
    let (w1_w, w1_s) = stack(&mut backend, &w1)?;
    let (w3_w, w3_s) = stack(&mut backend, &w3)?;
    let (w2_w, w2_s) = stack(&mut backend, &w2)?;
    let (sw1_w, sw1_s) = stack(&mut backend, std::slice::from_ref(&shared[0]))?;
    let (sw2_w, sw2_s) = stack(&mut backend, std::slice::from_ref(&shared[1]))?;
    let (sw3_w, sw3_s) = stack(&mut backend, std::slice::from_ref(&shared[2]))?;
    let logits = f32_buffer(&mut backend, experts)?;
    let ids = backend.byte_buffer(&vec![0; top_k * 4])?;
    let route_weights = f32_buffer(&mut backend, top_k)?;
    let x_codes = backend.byte_buffer(&vec![0; hidden])?;
    let x_scales = backend.byte_buffer(&vec![0; hidden / 32])?;
    let g = f32_buffer(&mut backend, top_k * inter)?;
    let u = f32_buffer(&mut backend, top_k * inter)?;
    let h = f32_buffer(&mut backend, top_k * inter)?;
    let h_codes = backend.byte_buffer(&vec![0; top_k * inter])?;
    let h_scales = backend.byte_buffer(&vec![0; top_k * inter / 32])?;
    let y = f32_buffer(&mut backend, top_k * hidden)?;
    let sg = f32_buffer(&mut backend, inter)?;
    let su = f32_buffer(&mut backend, inter)?;
    let sh = f32_buffer(&mut backend, inter)?;
    let sh_codes = backend.byte_buffer(&vec![0; inter])?;
    let sh_scales = backend.byte_buffer(&vec![0; inter / 32])?;
    let sy = f32_buffer(&mut backend, hidden)?;
    let out = f32_buffer(&mut backend, hidden)?;
    let routed = |shared_input| V41Slots {
        count: top_k,
        ids: Some(&ids),
        shared_input,
    };
    let mut submission = backend.begin_submission()?;
    submission.v41_gate_logits(&gate_buffer, &input_buffer, &logits, experts, hidden)?;
    submission.v41_route(router, &logits, &bias_buffer, &ids, &route_weights)?;
    submission.v41_act_quant_fp8(&input_buffer, &x_codes, &x_scales, 1, hidden)?;
    submission.v41_block_matvec(
        matrix(&w1_w, &w1_s, fp4, inter, hidden, experts),
        &x_codes,
        &x_scales,
        routed(true),
        &g,
        V41Output::Bf16,
    )?;
    submission.v41_block_matvec(
        matrix(&w3_w, &w3_s, fp4, inter, hidden, experts),
        &x_codes,
        &x_scales,
        routed(true),
        &u,
        V41Output::Bf16,
    )?;
    submission.v41_swiglu(
        &g,
        &u,
        Some(&route_weights),
        top_k,
        inter,
        10.0,
        &h,
        V41Output::Bf16,
    )?;
    submission.v41_act_quant_fp8(&h, &h_codes, &h_scales, top_k, inter)?;
    submission.v41_block_matvec(
        matrix(&w2_w, &w2_s, fp4, hidden, inter, experts),
        &h_codes,
        &h_scales,
        routed(false),
        &y,
        V41Output::Bf16,
    )?;
    submission.v41_block_matvec(
        matrix(&sw1_w, &sw1_s, fp8, inter, hidden, 1),
        &x_codes,
        &x_scales,
        V41Slots::single(),
        &sg,
        V41Output::Bf16,
    )?;
    submission.v41_block_matvec(
        matrix(&sw3_w, &sw3_s, fp8, inter, hidden, 1),
        &x_codes,
        &x_scales,
        V41Slots::single(),
        &su,
        V41Output::Bf16,
    )?;
    submission.v41_swiglu(&sg, &su, None, 1, inter, 10.0, &sh, V41Output::Bf16)?;
    submission.v41_act_quant_fp8(&sh, &sh_codes, &sh_scales, 1, inter)?;
    submission.v41_block_matvec(
        matrix(&sw2_w, &sw2_s, fp8, hidden, inter, 1),
        &sh_codes,
        &sh_scales,
        V41Slots::single(),
        &sy,
        V41Output::Bf16,
    )?;
    submission.v41_moe_combine(&y, top_k, Some(&sy), hidden, &out, V41Output::Bf16)?;
    submission.commit(CudaCommandWait::Completed)?;
    let gpu_ids = read_i32(&ids, top_k)?;
    let gpu_out = read_f32(&out, hidden)?;

    // The host models' chain.
    let host_logits = v41_gate_logits_host(&bf16_bytes(&gate), &input, experts, hidden);
    let (host_ids, host_weights) = v41_route_host(router, &host_logits, &bias);
    assert_eq!(gpu_ids, host_ids);
    let q = quantize(&round_bf16(&input)?, Quantization::Fp8)?;
    let mv = |m: &Packed, codes: &[u8], scales: &[u8]| {
        v41_round_bf16_all(&v41_block_matvec_host(
            m.format, &m.weights, &m.scales, m.rows, m.cols, codes, scales,
        ))
    };
    let mut routed_out = Vec::new();
    for (slot, id) in host_ids.iter().enumerate() {
        let e = *id as usize;
        let gh = mv(&w1[e], q.codes(), q.scales());
        let uh = mv(&w3[e], q.codes(), q.scales());
        let hh = v41_round_bf16_all(&v41_swiglu_host(
            &gh,
            &uh,
            Some(&host_weights[slot..=slot]),
            inter,
            10.0,
        ));
        let hq = quantize(&hh, Quantization::Fp8)?;
        routed_out.extend(mv(&w2[e], hq.codes(), hq.scales()));
    }
    let sgh = mv(&shared[0], q.codes(), q.scales());
    let suh = mv(&shared[2], q.codes(), q.scales());
    let shh = v41_round_bf16_all(&v41_swiglu_host(&sgh, &suh, None, inter, 10.0));
    let shq = quantize(&shh, Quantization::Fp8)?;
    let syh = mv(&shared[1], shq.codes(), shq.scales());
    let host_out = v41_round_bf16_all(&v41_moe_combine_host(
        &routed_out,
        top_k,
        Some(&syh),
        hidden,
    ));
    assert_host(&gpu_out, &host_out, "MoE step");

    // The CPU path.
    let (routing, cpu_out) = moe::forward(
        &input,
        &MatrixRef::new(experts, hidden, &gate)?,
        &bias,
        Router {
            top_k,
            ..Router::flash()
        },
        &Bank {
            w1: &w1,
            w2: &w2,
            w3: &w3,
            shared: &shared,
        },
        10.0,
    )?;
    let mut expected = routing.indices.clone();
    expected.sort_unstable();
    assert_eq!(
        gpu_ids,
        expected.iter().map(|i| *i as i32).collect::<Vec<_>>()
    );
    let cpu_out = round_bf16(&cpu_out)?;
    let ratio = policy_bf16(&gpu_out, &cpu_out);
    let equal = gpu_out.iter().zip(&cpu_out).filter(|(a, b)| a == b).count();
    assert!(ratio <= 1.0, "MoE step against the CPU: {ratio}");
    eprintln!(
        "MoE step from device ids {gpu_ids:?}: equal to the host chain; against moe::forward, \
         BF16 equal {equal}/{hidden}, max error / (2^-6 + 2^-6 |x|) {ratio:.3e}"
    );
    let _ = linear;
    Ok(())
}

// ---------------------------------------------------------------------------
// Decode latency (observation)
// ---------------------------------------------------------------------------

/// One decode step's latency per kernel at the model's shapes. Run
/// explicitly: `cargo test ... -- --ignored v41_mix_decode_latency`.
#[test]
#[ignore = "measurement; needs a CUDA device"]
fn v41_mix_decode_latency() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(42);
    let hc = V41HyperConfig::flash();
    let n = hc.elements();
    let projection = upload_f32(
        &mut backend,
        &(0..24 * n).map(|_| rng.unit() * 0.01).collect::<Vec<_>>(),
    )?;
    let scale = upload_f32(&mut backend, &[0.1, 0.1, 1.0])?;
    let base = upload_f32(
        &mut backend,
        &(0..24).map(|_| rng.unit()).collect::<Vec<_>>(),
    )?;
    let residual = upload_f32(
        &mut backend,
        &(0..n).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let sublayer = upload_f32(
        &mut backend,
        &(0..5120).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let scratch = f32_buffer(&mut backend, hc.scratch_floats())?;
    let mixes = f32_buffer(&mut backend, 24)?;
    let collapsed = f32_buffer(&mut backend, 5120)?;
    let expanded = f32_buffer(&mut backend, n)?;
    let projected = upload_f32(
        &mut backend,
        &(0..(4 + 1) * 5120)
            .map(|_| rng.bf16(1.0))
            .collect::<Vec<_>>(),
    )?;
    let qk = upload_f32(
        &mut backend,
        &(0..n).map(|_| rng.bf16(1.0)).collect::<Vec<_>>(),
    )?;
    let gated = f32_buffer(&mut backend, n)?;
    let gate_weight = backend.byte_buffer(&bf16_bytes(
        &(0..384 * 5120).map(|_| rng.bf16(0.02)).collect::<Vec<_>>(),
    ))?;
    let logits = f32_buffer(&mut backend, 384)?;
    let bias = upload_f32(
        &mut backend,
        &(0..384).map(|_| rng.unit() * 0.01).collect::<Vec<_>>(),
    )?;
    let ids = backend.byte_buffer(&[0; 24])?;
    let route_weights = f32_buffer(&mut backend, 6)?;
    let w13 = upload_f32(
        &mut backend,
        &(0..6 * 2304).map(|_| rng.bf16(3.0)).collect::<Vec<_>>(),
    )?;
    let hidden = f32_buffer(&mut backend, 6 * 2304)?;
    let routed = upload_f32(
        &mut backend,
        &(0..6 * 5120).map(|_| rng.bf16(0.1)).collect::<Vec<_>>(),
    )?;
    let moe_out = f32_buffer(&mut backend, 5120)?;
    let iterations = 200;
    let mut report = Vec::new();
    let mut time = |label: &str,
                    backend: &mut CudaBackend,
                    launch: &mut dyn FnMut(&mut CudaSubmission) -> Result<(), RuntimeError>|
     -> TestResult {
        let mut run = |n: usize| -> Result<f64, Box<dyn Error>> {
            let mut submission = backend.begin_submission()?;
            let start = std::time::Instant::now();
            for _ in 0..n {
                launch(&mut submission)?;
            }
            submission.commit(CudaCommandWait::Completed)?;
            Ok(start.elapsed().as_secs_f64() / n as f64)
        };
        run(10)?;
        let us = run(iterations)? * 1e6;
        eprintln!("{label}: {us:.2} us");
        report.push(serde_json::json!({"kernel": label, "microseconds": us}));
        Ok(())
    };
    time(
        "hc mixes (24x20480 projection, RMS, Sinkhorn)",
        &mut backend,
        &mut |s| {
            s.v41_hc_mixes(
                hc,
                V41HcWeights {
                    projection: &projection,
                    scale: &scale,
                    base: &base,
                },
                &residual,
                &scratch,
                &mixes,
            )
        },
    )?;
    time("hc collapse (4x5120 to 5120)", &mut backend, &mut |s| {
        s.v41_hc_collapse(hc, &residual, &mixes, &collapsed, V41Output::Bf16)
    })?;
    time("hc expand (5120 to 4x5120)", &mut backend, &mut |s| {
        s.v41_hc_expand(hc, &sublayer, &residual, &mixes, &expanded, V41Output::Bf16)
    })?;
    time("Engram gate (4x5120)", &mut backend, &mut |s| {
        s.v41_engram_gate(
            V41EngramGate::flash(),
            &residual,
            &projected,
            &qk,
            &qk,
            &gated,
            V41Output::Bf16,
        )
    })?;
    time("router logits (384x5120 BF16)", &mut backend, &mut |s| {
        s.v41_gate_logits(&gate_weight, &collapsed, &logits, 384, 5120)
    })?;
    time("routing (top 6 of 384)", &mut backend, &mut |s| {
        s.v41_route(V41Router::flash(), &logits, &bias, &ids, &route_weights)
    })?;
    time("SwiGLU (6x2304)", &mut backend, &mut |s| {
        s.v41_swiglu(
            &w13,
            &w13,
            Some(&route_weights),
            6,
            2304,
            10.0,
            &hidden,
            V41Output::Bf16,
        )
    })?;
    time("MoE sum (6x5120 + shared)", &mut backend, &mut |s| {
        s.v41_moe_combine(&routed, 6, Some(&sublayer), 5120, &moe_out, V41Output::Bf16)
    })?;
    // The Engram gather from tables of 16 MiB and 1 GiB: the time doesn't
    // depend on the table size, since only the 24 requested rows are read.
    let row_ids: Vec<i64> = (0..24).map(|i| i * 2731 + 17).collect();
    let id_buffer = upload_i64(&mut backend, &row_ids)?;
    let gathered = f32_buffer(&mut backend, 24 * 256)?;
    for rows in [1_usize << 16, 1 << 22] {
        let mut codes = backend.host_buffer(rows * 256)?;
        codes.write_bytes(&vec![0x38; rows * 256])?;
        let mut scales = backend.host_buffer(rows * 8)?;
        scales.write_bytes(&vec![127; rows * 8])?;
        let ids: Vec<i64> = row_ids.iter().map(|i| i * (rows as i64 >> 16)).collect();
        let id_buffer = upload_i64(&mut backend, &ids)?;
        time(
            &format!(
                "Engram gather (24 rows, {} MiB host table)",
                rows * 264 >> 20
            ),
            &mut backend,
            &mut |s| {
                s.v41_engram_gather(
                    V41EngramTable {
                        codes: &codes,
                        scales: &scales,
                        rows: rows as u64,
                        width: 256,
                    },
                    &id_buffer,
                    24,
                    &gathered,
                    V41Output::Bf16,
                )
            },
        )?;
    }
    let _ = id_buffer;
    eprintln!("{}", serde_json::to_string(&report)?);
    Ok(())
}
