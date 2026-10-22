//! Parity tests for the V4.1 block-scaled kernels against the CPU path.
//!
//! Tolerance, fixed before the first GPU run. The CPU path
//! (`deepseek_v41_cpu::quantization::linear` before its BF16 rounding) and
//! the kernel add the same terms `t_i = (w_i * s_w) * (x_i * s_x)`: every
//! dequantized value is a code times a power of two, and a product of two
//! codes has at most 8 significant bits, so each term is exact in float32
//! unless it underflows or overflows. Only the association differs: the CPU
//! adds all K terms in one sequential sum (depth K - 1); the kernel adds 32
//! terms per block (depth 31), scales exactly, and adds the K / 32 block sums
//! in order (depth K / 32 - 1). With u = 2^-24 and
//! gamma(n) = n u / (1 - n u), each result is within
//! gamma(depth) * sum |t_i| of the exact sum (Higham, Accuracy and Stability
//! of Numerical Algorithms, 2nd ed., section 4.2), so
//!
//! |gpu - cpu| <= (gamma(K - 1) + gamma(K / 32 + 30)) * sum |t_i|.
//!
//! Fused multiply-adds don't change this: the products are exact either way.
//! Where the arithmetic is identical the kernel must match exactly: the host
//! model of the kernel's order ([`v41_block_matvec_host`]), activation
//! quantization (codes and scales), and inputs whose every partial sum is an
//! exact float32.

use super::*;
use crate::{CudaBackend, CudaCommandWait};
use psionic_core::{DType, Shape, TensorSpec};
use psionic_models::deepseek_v41_cpu::quantization::{
    Arithmetic, MatrixPrecision, Quantization, linear, quantize, round_bf16,
};
use psionic_models::deepseek_v41_cpu::{CpuError, MatrixRows};
use psionic_models::deepseek_v41_weights::{
    decode_bf16, decode_e2m1, decode_f8_e4m3fn, decode_f8_e8m0fnu, dequantize_fp4_row,
    dequantize_fp8_per32,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{error::Error, path::PathBuf};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Section {
    offset: usize,
    bytes: usize,
    sha256: String,
}

#[derive(Deserialize)]
struct FixtureMatrix {
    label: String,
    format: String,
    shape: [usize; 2],
    first_row: usize,
    rows: usize,
    weight: Section,
    scale: Section,
}

#[derive(Deserialize)]
struct FixtureVector {
    label: String,
    dtype: String,
    #[serde(flatten)]
    section: Section,
}

#[derive(Deserialize)]
struct Fixture {
    blob_sha256: String,
    matrices: Vec<FixtureMatrix>,
    vectors: Vec<FixtureVector>,
}

struct Rows {
    fixture: Fixture,
    blob: Vec<u8>,
}

impl Rows {
    fn load() -> Result<Self, Box<dyn Error>> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/deepseek_v41_matvec");
        let fixture: Fixture = serde_json::from_slice(&std::fs::read(dir.join("rows.json"))?)?;
        let blob = std::fs::read(dir.join("rows.bin"))?;
        assert_eq!(hex::encode(Sha256::digest(&blob)), fixture.blob_sha256);
        let rows = Self { fixture, blob };
        for m in &rows.fixture.matrices {
            assert_eq!(
                hex::encode(Sha256::digest(rows.bytes(&m.weight))),
                m.weight.sha256
            );
            assert_eq!(
                hex::encode(Sha256::digest(rows.bytes(&m.scale))),
                m.scale.sha256
            );
        }
        for v in &rows.fixture.vectors {
            assert_eq!(
                hex::encode(Sha256::digest(rows.bytes(&v.section))),
                v.section.sha256
            );
        }
        Ok(rows)
    }

    fn bytes(&self, s: &Section) -> &[u8] {
        &self.blob[s.offset..s.offset + s.bytes]
    }

    fn matrix(&self, label: &str) -> &FixtureMatrix {
        self.fixture
            .matrices
            .iter()
            .find(|m| m.label == label)
            .unwrap_or_else(|| panic!("no matrix {label}"))
    }

    fn raw(&self, label: &str) -> &[u8] {
        let v = self
            .fixture
            .vectors
            .iter()
            .find(|v| v.label == label)
            .unwrap_or_else(|| panic!("no vector {label}"));
        self.bytes(&v.section)
    }

    fn bf16(&self, label: &str) -> Vec<f32> {
        let v = self.fixture.vectors.iter().find(|v| v.label == label);
        assert_eq!(v.map(|v| v.dtype.as_str()), Some("BF16"));
        self.raw(label)
            .chunks_exact(2)
            .map(|p| decode_bf16([p[0], p[1]]))
            .collect()
    }
}

/// A packed matrix on the host: what the kernel reads.
struct Packed {
    format: V41MatvecFormat,
    rows: usize,
    cols: usize,
    weights: Vec<u8>,
    scales: Vec<u8>,
}

impl Packed {
    fn from_fixture(rows: &Rows, m: &FixtureMatrix) -> Self {
        let format = match m.format.as_str() {
            "fp4_e2m1_row32" => V41MatvecFormat::Fp4E2M1Row32,
            "fp8_block32x32" => V41MatvecFormat::Fp8Block32x32,
            other => panic!("unknown format {other}"),
        };
        assert_eq!(m.first_row % V41_BLOCK, 0);
        Self {
            format,
            rows: m.rows,
            cols: m.shape[1],
            weights: rows.bytes(&m.weight).to_vec(),
            scales: rows.bytes(&m.scale).to_vec(),
        }
    }

    /// One row dequantized by the #12373 loader.
    fn loader_row(&self, row: usize) -> Vec<f32> {
        let blocks = self.cols / V41_BLOCK;
        let result = match self.format {
            V41MatvecFormat::Fp4E2M1Row32 => dequantize_fp4_row(
                &self.weights[row * self.cols / 2..(row + 1) * self.cols / 2],
                &self.scales[row * blocks..(row + 1) * blocks],
            ),
            V41MatvecFormat::Fp8Block32x32 => {
                let block_row = row / V41_BLOCK;
                dequantize_fp8_per32(
                    &self.weights[row * self.cols..(row + 1) * self.cols],
                    &self.scales[block_row * blocks..(block_row + 1) * blocks],
                )
            }
            V41MatvecFormat::Q2Row32 | V41MatvecFormat::Q3Row32 => {
                unreachable!("the CPU path has no consumer-q matrices")
            }
        };
        result.expect("fixture rows fit their scales")
    }
}

impl MatrixRows for Packed {
    fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        Ok(self.loader_row(index))
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

// ---------------------------------------------------------------------------
// The CPU path and the tolerance
// ---------------------------------------------------------------------------

/// The CPU path's float32 sums before BF16 rounding, with each row's
/// `sum |t_i|` in f64: the steps of `quantization::linear` for FP4/FP8
/// matrices, from the same public functions.
fn cpu_f32(matrix: &Packed, input: &[f32]) -> (Vec<f32>, Vec<f64>) {
    let activation = quantize(&round_bf16(input).expect("finite input"), Quantization::Fp8)
        .and_then(|q| q.dequantize())
        .expect("finite activation");
    (0..matrix.rows)
        .map(|row| {
            let w = matrix.loader_row(row);
            let sum: f32 = w.iter().zip(&activation).map(|(w, x)| w * x).sum();
            let magnitude: f64 = w
                .iter()
                .zip(&activation)
                .map(|(w, x)| (f64::from(*w) * f64::from(*x)).abs())
                .sum();
            (sum, magnitude)
        })
        .unzip()
}

fn gamma(n: usize) -> f64 {
    let nu = n as f64 * 2.0_f64.powi(-24);
    nu / (1.0 - nu)
}

/// The a priori bound on |gpu - cpu| for one output of a K-wide row.
fn bound(cols: usize, magnitude: f64) -> f64 {
    (gamma(cols - 1) + gamma(cols / V41_BLOCK + 30)) * magnitude
}

fn same_bits(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

// ---------------------------------------------------------------------------
// CPU-only checks (every machine)
// ---------------------------------------------------------------------------

#[test]
fn host_decoders_match_the_weight_loader_for_every_code() {
    for byte in 0..=255_u8 {
        assert!(
            same_bits(e4m3(byte), decode_f8_e4m3fn(byte)),
            "e4m3 {byte:#x}"
        );
        assert!(
            same_bits(e8m0(byte), decode_f8_e8m0fnu(byte)),
            "e8m0 {byte:#x}"
        );
    }
    for code in 0..16_u8 {
        assert!(same_bits(e2m1(code), decode_e2m1(code)), "e2m1 {code}");
    }
}

#[test]
fn dimension_checks_refuse_bad_shapes() {
    assert!(check_v41_matvec_dims(1280, 5120, 1, 1).is_ok());
    assert!(check_v41_matvec_dims(32768, 1280, 384, 6).is_ok());
    assert!(check_v41_matvec_dims(0, 5120, 1, 1).is_err());
    assert!(check_v41_matvec_dims(8, 0, 1, 1).is_err());
    assert!(check_v41_matvec_dims(8, 100, 1, 1).is_err());
    assert!(check_v41_matvec_dims(8, 64, 0, 1).is_err());
    assert!(check_v41_matvec_dims(8, 64, 1, 0).is_err());
    assert!(check_v41_matvec_dims(8, 64, 1, 65_536).is_err());
    assert!(check_v41_matvec_dims(1 << 20, 64, 1, 4096).is_err());
    assert_eq!(
        V41MatvecFormat::Fp8Block32x32.scale_bytes(1280, 5120),
        40 * 160
    );
    assert_eq!(V41MatvecFormat::Fp8Block32x32.scale_bytes(37, 96), 2 * 3);
    assert_eq!(
        V41MatvecFormat::Fp4E2M1Row32.weight_bytes(2304, 5120),
        2304 * 2560
    );
    assert_eq!(
        V41MatvecFormat::Fp4E2M1Row32.scale_bytes(2304, 5120),
        2304 * 160
    );
}

/// The committed rows decode, the CPU activation rule reproduces the
/// reference's own FP8 activations bit for bit, and the host model of the
/// kernel stays inside the tolerance of the CPU path on real rows.
#[test]
fn real_rows_cpu_rule_and_host_model_agree() -> TestResult {
    let rows = Rows::load()?;
    for (input, codes, scales) in [
        ("ffn_in", "ffn_in_fp8", "ffn_in_fp8_scale"),
        ("w2_in", "w2_in_fp8", "w2_in_fp8_scale"),
    ] {
        let q = quantize(&round_bf16(&rows.bf16(input))?, Quantization::Fp8)?;
        assert_eq!(q.codes(), rows.raw(codes), "{input} codes");
        assert_eq!(q.scales(), rows.raw(scales), "{input} scales");
    }
    for (label, input, _) in REAL_CASES {
        let matrix = Packed::from_fixture(&rows, rows.matrix(label));
        let x = rows.bf16(input);
        let q = quantize(&round_bf16(&x)?, Quantization::Fp8)?;
        let host = v41_block_matvec_host(
            matrix.format,
            &matrix.weights,
            &matrix.scales,
            matrix.rows,
            matrix.cols,
            q.codes(),
            q.scales(),
        );
        let (cpu, magnitude) = cpu_f32(&matrix, &x);
        // The f32 steps reproduce `linear` exactly once rounded.
        let linear = linear(&matrix, &x, Arithmetic::ReferenceBf16)?;
        for row in 0..matrix.rows {
            assert_eq!(v41_round_bf16(cpu[row]).to_bits(), linear[row].to_bits());
            let diff = f64::from((host[row] - cpu[row]).abs());
            assert!(
                diff <= bound(matrix.cols, magnitude[row]),
                "{label} row {row}"
            );
        }
    }
    Ok(())
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
    backend.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::F32, device))
}

fn upload_f32(backend: &mut CudaBackend, values: &[f32]) -> Result<CudaBuffer, RuntimeError> {
    let mut buffer = f32_buffer(backend, values.len())?;
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    buffer.write_bytes(&bytes)?;
    Ok(buffer)
}

fn read_f32(buffer: &CudaBuffer) -> Result<Vec<f32>, RuntimeError> {
    Ok(buffer
        .read_bytes()?
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Device-quantized activations: `(codes, scales)` buffers and host copies.
struct DeviceInput {
    codes: CudaBuffer,
    scales: CudaBuffer,
    host_codes: Vec<u8>,
    host_scales: Vec<u8>,
}

fn quantize_on_device(
    backend: &mut CudaBackend,
    input: &[f32],
    rows: usize,
) -> Result<DeviceInput, Box<dyn Error>> {
    let cols = input.len() / rows;
    let x = upload_f32(backend, input)?;
    let codes = backend.byte_buffer(&vec![0; input.len()])?;
    let scales = backend.byte_buffer(&vec![0; input.len() / V41_BLOCK])?;
    let mut submission = backend.begin_submission()?;
    submission.v41_act_quant_fp8(&x, &codes, &scales, rows, cols)?;
    submission.commit(CudaCommandWait::Completed)?;
    let host_codes = codes.read_bytes()?;
    let host_scales = scales.read_bytes()?;
    Ok(DeviceInput {
        codes,
        scales,
        host_codes,
        host_scales,
    })
}

/// Runs one matrix against one activation row; returns (f32, BF16) outputs.
fn matvec_on_device(
    backend: &mut CudaBackend,
    matrix: &Packed,
    input: &DeviceInput,
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn Error>> {
    let weights = backend.byte_buffer(&matrix.weights)?;
    let scales = backend.byte_buffer(&matrix.scales)?;
    let out = f32_buffer(backend, matrix.rows)?;
    let out_bf16 = f32_buffer(backend, matrix.rows)?;
    let device_matrix = V41Matrix {
        format: matrix.format,
        weights: &weights,
        scales: &scales,
        rows: matrix.rows,
        cols: matrix.cols,
        matrices: 1,
    };
    let mut submission = backend.begin_submission()?;
    for (buffer, output) in [(&out, V41Output::F32), (&out_bf16, V41Output::Bf16)] {
        submission.v41_block_matvec(
            device_matrix,
            &input.codes,
            &input.scales,
            V41Slots::single(),
            buffer,
            output,
        )?;
    }
    submission.commit(CudaCommandWait::Completed)?;
    Ok((read_f32(&out)?, read_f32(&out_bf16)?))
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
    /// Roughly normal with occasional outliers, like real activations.
    fn activation(&mut self) -> f32 {
        let u: f32 = (0..4)
            .map(|_| (self.next() >> 40) as f32 / (1u64 << 24) as f32)
            .sum();
        let x = (u - 2.0) * 1.7;
        if self.next().is_multiple_of(97) {
            x * 40.0
        } else {
            x
        }
    }
}

fn synthetic(rng: &mut Rng, format: V41MatvecFormat, rows: usize, cols: usize) -> Packed {
    let weights = (0..format.weight_bytes(rows, cols))
        .map(|_| {
            let byte = rng.byte();
            // No FP8 NaN codes here; NaN gets its own case.
            if format == V41MatvecFormat::Fp8Block32x32 && byte & 0x7f == 0x7f {
                byte ^ 1
            } else {
                byte
            }
        })
        .collect();
    // Real weight scales sit around 2^-10 to 2^-4.
    let scales = (0..format.scale_bytes(rows, cols))
        .map(|_| 115 + rng.byte() % 9)
        .collect();
    Packed {
        format,
        rows,
        cols,
        weights,
        scales,
    }
}

/// Whether the f32 outputs need the a priori bound or must match exactly.
#[derive(Clone, Copy, PartialEq)]
enum Expect {
    Bound,
    Exact,
}

/// Checks one matrix against the CPU path and the host model on the device;
/// returns the largest |gpu - cpu| / bound and the BF16 mismatches.
fn check_against_cpu(
    backend: &mut CudaBackend,
    matrix: &Packed,
    x: &[f32],
    expect: Expect,
) -> Result<(f64, usize), Box<dyn Error>> {
    let input = quantize_on_device(backend, x, 1)?;
    let cpu_q = quantize(&round_bf16(x)?, Quantization::Fp8)?;
    assert_eq!(input.host_codes, cpu_q.codes(), "activation codes");
    assert_eq!(input.host_scales, cpu_q.scales(), "activation scales");
    let (gpu, gpu_bf16) = matvec_on_device(backend, matrix, &input)?;
    let host = v41_block_matvec_host(
        matrix.format,
        &matrix.weights,
        &matrix.scales,
        matrix.rows,
        matrix.cols,
        &input.host_codes,
        &input.host_scales,
    );
    let (cpu, magnitude) = cpu_f32(matrix, x);
    let mut worst = 0.0_f64;
    let mut bf16_mismatches = 0;
    for row in 0..matrix.rows {
        assert!(
            same_bits(gpu[row], host[row]),
            "row {row}: device {} host {}",
            gpu[row],
            host[row]
        );
        assert!(
            same_bits(gpu_bf16[row], v41_round_bf16(gpu[row])),
            "row {row} BF16"
        );
        let diff = f64::from((gpu[row] - cpu[row]).abs());
        let limit = bound(matrix.cols, magnitude[row]);
        if expect == Expect::Exact {
            assert_eq!(gpu[row].to_bits(), cpu[row].to_bits(), "row {row} exact");
        }
        assert!(
            diff <= limit,
            "row {row}: |{} - {}| > {limit}",
            gpu[row],
            cpu[row]
        );
        if limit > 0.0 {
            worst = worst.max(diff / limit);
        }
        if gpu_bf16[row].to_bits() != v41_round_bf16(cpu[row]).to_bits() {
            // Allowed only when the bound reaches a BF16 rounding boundary.
            let lo = v41_round_bf16((f64::from(cpu[row]) - limit) as f32);
            let hi = v41_round_bf16((f64::from(cpu[row]) + limit) as f32);
            assert!(
                lo != hi,
                "row {row}: BF16 differs with no boundary in reach"
            );
            bf16_mismatches += 1;
        }
    }
    Ok((worst, bf16_mismatches))
}

// ---------------------------------------------------------------------------
// Device checks
// ---------------------------------------------------------------------------

/// `(matrix, activation, reference output)`. The reference outputs are the
/// oracle's BF16 `fp4_gemm` results for expert 222.
const REAL_CASES: [(&str, &str, Option<&str>); 5] = [
    ("wq_a", "attn_in", None),
    ("shared_w1", "ffn_in", None),
    ("shared_w2", "w2_in", None),
    ("expert_w1", "ffn_in", Some("expert_w1_out")),
    ("expert_w2", "w2_in", Some("expert_w2_out")),
];

#[test]
fn real_rows_match_the_cpu_path_on_device() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let rows = Rows::load()?;
    // The device activation rule against the reference's own codes.
    for (input, codes, scales) in [
        ("ffn_in", "ffn_in_fp8", "ffn_in_fp8_scale"),
        ("w2_in", "w2_in_fp8", "w2_in_fp8_scale"),
    ] {
        let q = quantize_on_device(&mut backend, &rows.bf16(input), 1)?;
        assert_eq!(q.host_codes, rows.raw(codes), "{input} device codes");
        assert_eq!(q.host_scales, rows.raw(scales), "{input} device scales");
    }
    for (label, input, reference) in REAL_CASES {
        let fixture = rows.matrix(label);
        let matrix = Packed::from_fixture(&rows, fixture);
        let x = rows.bf16(input);
        let (worst, bf16_mismatches) = check_against_cpu(&mut backend, &matrix, &x, Expect::Bound)?;
        let linear = linear(&matrix, &x, Arithmetic::ReferenceBf16)?;
        let q = quantize_on_device(&mut backend, &x, 1)?;
        let (_, gpu_bf16) = matvec_on_device(&mut backend, &matrix, &q)?;
        let linear_equal = gpu_bf16
            .iter()
            .zip(&linear)
            .filter(|(a, b)| a.to_bits() == b.to_bits())
            .count();
        let mut line = format!(
            "{label}: {} rows x {} K, max |gpu - cpu| / bound {worst:.3e}, \
             BF16 equal to CPU linear {linear_equal}/{} ({bf16_mismatches} at a rounding boundary)",
            matrix.rows, matrix.cols, matrix.rows
        );
        if let Some(reference) = reference {
            let reference = rows.bf16(reference);
            let slice = &reference[fixture.first_row..fixture.first_row + fixture.rows];
            let equal = gpu_bf16
                .iter()
                .zip(slice)
                .filter(|(a, b)| a.to_bits() == b.to_bits())
                .count();
            let max = gpu_bf16
                .iter()
                .zip(slice)
                .map(|(a, b)| (a - b).abs() / b.abs().max(f32::MIN_POSITIVE))
                .fold(0.0_f32, f32::max);
            line.push_str(&format!(
                "; BF16 equal to the reference {equal}/{}, max relative difference {max:.3e}",
                fixture.rows
            ));
        }
        eprintln!("{line}");
    }
    Ok(())
}

/// The model's shapes, plus odd row tails and K tails that end mid-warp and
/// mid-stage (`K / 32` of 3, 33, and 257 blocks).
const SHAPES: [(usize, usize); 13] = [
    (1280, 5120),
    (2304, 5120),
    (5120, 2304),
    (32768, 1280),
    (4096, 1280),
    (1024, 4096),
    (512, 5120),
    (1, 32),
    (7, 96),
    (37, 1056),
    (1003, 8224),
    (33, 4128),
    (5120, 8192),
];

#[test]
fn synthetic_shapes_match_the_cpu_path_on_device() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for (rows, cols) in SHAPES {
        for format in [
            V41MatvecFormat::Fp8Block32x32,
            V41MatvecFormat::Fp4E2M1Row32,
        ] {
            let matrix = synthetic(&mut rng, format, rows, cols);
            let x: Vec<f32> = (0..cols).map(|_| rng.activation()).collect();
            let (worst, mismatches) = check_against_cpu(&mut backend, &matrix, &x, Expect::Bound)?;
            eprintln!(
                "{format:?} {rows}x{cols}: max |gpu - cpu| / bound {worst:.3e}, \
                 BF16 boundary cases {mismatches}"
            );
        }
    }
    Ok(())
}

#[test]
fn exact_sums_match_the_cpu_path_bit_for_bit() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(7);
    let (rows, cols) = (64, 5120);
    // Integer activations up to 4 quantize exactly (scale 2^-6); weights are
    // small dyadic codes with unit scales, so every partial sum is exact.
    let x: Vec<f32> = (0..cols)
        .map(|i| {
            if i % 32 == 0 {
                4.0
            } else {
                (rng.next() % 9) as f32 - 4.0
            }
        })
        .collect();
    let fp8_codes = [0x30_u8, 0x38, 0x3c, 0x40, 0xb8, 0xc0, 0x00, 0x80];
    let fp8 = Packed {
        format: V41MatvecFormat::Fp8Block32x32,
        rows,
        cols,
        weights: (0..rows * cols)
            .map(|_| fp8_codes[(rng.next() % 8) as usize])
            .collect(),
        scales: vec![127; V41MatvecFormat::Fp8Block32x32.scale_bytes(rows, cols)],
    };
    let fp4 = Packed {
        format: V41MatvecFormat::Fp4E2M1Row32,
        rows,
        cols,
        weights: (0..rows * cols / 2).map(|_| rng.byte()).collect(),
        scales: vec![127; V41MatvecFormat::Fp4E2M1Row32.scale_bytes(rows, cols)],
    };
    for matrix in [&fp8, &fp4] {
        check_against_cpu(&mut backend, matrix, &x, Expect::Exact)?;
    }
    Ok(())
}

/// A measured B200 row where sequential F32 block sums round to the wrong
/// BF16 value, plus the independent CPU model's eight-rank partial sum.
#[test]
fn reference_fp8_order_matches_the_measured_b200_row() -> TestResult {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../psionic-models/src/testdata/deepseek_v41_cpu/sm100_gemm.json"
    ))?;
    let row = &fixture["wq_b"];
    let decode = |v: &serde_json::Value| -> Result<Vec<f32>, Box<dyn Error>> {
        Ok(hex::decode(v.as_str().ok_or("missing BF16 fixture")?)?
            .chunks_exact(2)
            .map(|b| decode_bf16([b[0], b[1]]))
            .collect())
    };
    let input = decode(&row["input_bf16"])?;
    let original_weights = decode(&row["weights_bf16"])?;
    let quant_weights = quantize(&original_weights, Quantization::Fp8)?;
    assert_eq!(quant_weights.dequantize()?, original_weights);
    let packed = Packed {
        format: V41MatvecFormat::Fp8Block32x32,
        rows: 1,
        cols: input.len(),
        weights: quant_weights.codes().to_vec(),
        scales: quant_weights.scales().to_vec(),
    };
    let expected = row["expected"].as_f64().ok_or("missing expected value")? as f32;
    assert_eq!(
        linear(&packed, &input, Arithmetic::ReferenceBf16Tp8)?,
        vec![expected]
    );
    assert_ne!(
        linear(&packed, &input, Arithmetic::ReferenceBf16)?,
        vec![expected]
    );
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let w = backend.byte_buffer(&packed.weights)?;
    let ws = backend.byte_buffer(&packed.scales)?;
    let quant_input = quantize(&round_bf16(&input)?, Quantization::Fp8)?;
    let x = backend.byte_buffer(quant_input.codes())?;
    let xs = backend.byte_buffer(quant_input.scales())?;
    let out = f32_buffer(&mut backend, 1)?;
    let matrix = V41Matrix {
        format: packed.format,
        weights: &w,
        scales: &ws,
        rows: 1,
        cols: packed.cols,
        matrices: 1,
    };
    let slots = V41Slots {
        count: 1,
        ids: None,
        shared_input: true,
    };
    for shards in [1, 8] {
        let mut submission = backend.begin_submission()?;
        submission.v41_block_matvec_reference(
            matrix,
            &x,
            &xs,
            slots,
            &out,
            V41Output::Bf16,
            shards,
        )?;
        submission.commit(CudaCommandWait::Completed)?;
        let want = if shards == 1 {
            vec![expected]
        } else {
            psionic_models::deepseek_v41_cpu::quantization::linear_row_parallel(
                &packed,
                &input,
                Arithmetic::ReferenceBf16Tp8,
            )?
        };
        assert_eq!(
            read_f32(&out)?,
            want,
            "reference FP8 with {shards} rank partials"
        );
    }
    Ok(())
}

/// The bounded diagnostic agrees with the independently implemented CPU
/// integer sum for mixed signs, zeros, subnormal FP8 codes, and ordinary
/// weight scales at both real projection widths. Comparison is exact.
#[test]
fn reference_fp8_random_rows_match_independent_cpu_sums() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(0x987123);
    for cols in [1280, 5120] {
        let mut packed = synthetic(&mut rng, V41MatvecFormat::Fp8Block32x32, 37, cols);
        packed.weights[..cols].fill(0);
        let input: Vec<f32> = (0..cols).map(|_| rng.activation()).collect();
        let q = quantize(&round_bf16(&input)?, Quantization::Fp8)?;
        let w = backend.byte_buffer(&packed.weights)?;
        let ws = backend.byte_buffer(&packed.scales)?;
        let x = backend.byte_buffer(q.codes())?;
        let xs = backend.byte_buffer(q.scales())?;
        let out = f32_buffer(&mut backend, packed.rows)?;
        let matrix = V41Matrix {
            format: packed.format,
            weights: &w,
            scales: &ws,
            rows: packed.rows,
            cols,
            matrices: 1,
        };
        for shards in [1, 8] {
            let mut submission = backend.begin_submission()?;
            submission.v41_block_matvec_reference(
                matrix,
                &x,
                &xs,
                V41Slots::single(),
                &out,
                V41Output::Bf16,
                shards,
            )?;
            submission.commit(CudaCommandWait::Completed)?;
            let want = if shards == 1 {
                linear(&packed, &input, Arithmetic::ReferenceBf16Tp8)?
            } else {
                psionic_models::deepseek_v41_cpu::quantization::linear_row_parallel(
                    &packed,
                    &input,
                    Arithmetic::ReferenceBf16Tp8,
                )?
            };
            assert_eq!(read_f32(&out)?, want, "K={cols}, ranks={shards}");
        }
    }
    Ok(())
}

/// Activation quantization on edge values: zero blocks (scale floor),
/// signed zeros, ties between E4M3 codes, subnormal codes, saturation, large
/// and tiny magnitudes, BF16 rounding of the input, and nonfinite blocks.
#[test]
fn activation_quantization_edges_match_the_cpu_rule() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut blocks: Vec<Vec<f32>> = vec![
        vec![0.0; 32],
        (0..32)
            .map(|i| if i % 2 == 0 { -0.0 } else { 0.0 })
            .collect(),
        (0..32).map(|i| 1.0 + i as f32 / 16.0).collect(),
        (0..32).map(|i| 448.0 - i as f32).collect(),
        (0..32).map(|i| (i as f32 - 16.0) * 1e-30).collect(),
        (0..32).map(|i| (i as f32 - 16.0) * 1e30).collect(),
        (0..32).map(|i| 1.003_906_3 + i as f32 * 0.001).collect(),
        (0..32)
            .map(|i| if i == 0 { 1e-4 } else { 1e-7 * i as f32 })
            .collect(),
        // Ties: with scale 1, 1.0625 and 1.1875 sit halfway between codes.
        (0..32)
            .map(|i| [1.0625, 1.1875, -1.0625, 448.0][i % 4])
            .collect(),
        (0..32).map(|i| if i == 3 { 3.0e38 } else { 1.0 }).collect(),
    ];
    let mut rng = Rng(11);
    for _ in 0..64 {
        let scale = 2.0_f32.powi((rng.next() % 60) as i32 - 30);
        blocks.push((0..32).map(|_| rng.activation() * scale).collect());
    }
    let finite: Vec<f32> = blocks.concat();
    let q = quantize_on_device(&mut backend, &finite, 1)?;
    let cpu = quantize(&round_bf16(&finite)?, Quantization::Fp8)?;
    assert_eq!(q.host_codes, cpu.codes());
    assert_eq!(q.host_scales, cpu.scales());
    // Two rows: the device's rows are independent.
    let q2 = quantize_on_device(&mut backend, &[finite.clone(), finite.clone()].concat(), 2)?;
    assert_eq!(q2.host_codes, [cpu.codes(), cpu.codes()].concat());
    // Nonfinite blocks: the CPU refuses; the device writes NaN scales.
    let mut bad = vec![1.0_f32; 32 * 4];
    bad[5] = f32::NAN;
    bad[32 + 7] = f32::INFINITY;
    bad[64 + 9] = f32::MAX; // rounds to BF16 infinity
    assert!(round_bf16(&bad).is_err());
    let q = quantize_on_device(&mut backend, &bad, 1)?;
    assert_eq!(q.host_scales[..3], [0xff, 0xff, 0xff]);
    assert!(q.host_codes[..96].iter().all(|c| *c == 0x7f));
    assert_eq!(
        q.host_scales[3],
        quantize(&[1.0; 32], Quantization::Fp8)?.scales()[0]
    );
    Ok(())
}

/// Extreme and NaN scales, saturated codes, and NaN codes: the device must
/// match the host model of its arithmetic exactly, overflow and NaN included.
#[test]
fn extreme_scales_saturation_and_nan_match_the_host_model() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(23);
    let (rows, cols) = (96, 1280);
    for format in [
        V41MatvecFormat::Fp8Block32x32,
        V41MatvecFormat::Fp4E2M1Row32,
    ] {
        let mut matrix = synthetic(&mut rng, format, rows, cols);
        let blocks = cols / V41_BLOCK;
        let scale_row = |row: usize| match format {
            V41MatvecFormat::Fp8Block32x32 => row / V41_BLOCK * blocks,
            _ => row * blocks,
        };
        // Row 0: smallest scales (2^-127, subnormal); row 33: largest
        // (2^127) with saturated codes, which overflows; row 70: a NaN scale.
        for b in 0..blocks {
            matrix.scales[scale_row(0) + b] = 0;
            matrix.scales[scale_row(33) + b] = 254;
        }
        matrix.scales[scale_row(70) + 3] = 0xff;
        let row_bytes = format.weight_bytes(1, cols);
        let saturated = match format {
            V41MatvecFormat::Fp8Block32x32 => 0x7e,
            V41MatvecFormat::Fp4E2M1Row32 => 0x77,
            V41MatvecFormat::Q2Row32 | V41MatvecFormat::Q3Row32 => 0xff,
        };
        matrix.weights[33 * row_bytes..34 * row_bytes].fill(saturated);
        if format == V41MatvecFormat::Fp8Block32x32 {
            matrix.weights[95 * row_bytes + 17] = 0xff; // a NaN weight code
        }
        // Positive activations keep every saturated block sum positive, so
        // row 33 overflows to +infinity rather than NaN.
        let mut x: Vec<f32> = (0..cols).map(|_| rng.activation().abs()).collect();
        x[0] = 448.0;
        x[32..64].fill(1e30);
        x[64..96].fill(1e-30);
        let input = quantize_on_device(&mut backend, &x, 1)?;
        let (gpu, gpu_bf16) = matvec_on_device(&mut backend, &matrix, &input)?;
        let host = v41_block_matvec_host(
            format,
            &matrix.weights,
            &matrix.scales,
            rows,
            cols,
            &input.host_codes,
            &input.host_scales,
        );
        for row in 0..rows {
            assert!(same_bits(gpu[row], host[row]), "{format:?} row {row}");
            assert!(
                same_bits(gpu_bf16[row], v41_round_bf16(host[row])),
                "{format:?} row {row}"
            );
        }
        assert!(gpu[33].is_infinite());
        assert!(gpu[70].is_nan());
        if format == V41MatvecFormat::Fp8Block32x32 {
            assert!(gpu[95].is_nan());
        }
        // A NaN activation block makes every output NaN.
        x[200] = f32::NAN;
        let input = quantize_on_device(&mut backend, &x, 1)?;
        let (gpu, _) = matvec_on_device(&mut backend, &matrix, &input)?;
        assert!(gpu.iter().all(|v| v.is_nan()));
    }
    Ok(())
}

/// Slots that pick matrices from a stack by device id, with shared and
/// per-slot activations; ids outside the stack write NaN.
#[test]
fn stacked_matrices_follow_device_ids() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(31);
    let (rows, cols, stack) = (40, 96, 3);
    for format in [
        V41MatvecFormat::Fp8Block32x32,
        V41MatvecFormat::Fp4E2M1Row32,
    ] {
        let matrices: Vec<Packed> = (0..stack)
            .map(|_| synthetic(&mut rng, format, rows, cols))
            .collect();
        let weights = backend.byte_buffer(
            &matrices
                .iter()
                .flat_map(|m| m.weights.clone())
                .collect::<Vec<_>>(),
        )?;
        let scales = backend.byte_buffer(
            &matrices
                .iter()
                .flat_map(|m| m.scales.clone())
                .collect::<Vec<_>>(),
        )?;
        let ids: [i32; 5] = [2, 0, -1, 2, 3];
        let id_buffer =
            backend.byte_buffer(&ids.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<_>>())?;
        let x: Vec<f32> = (0..cols * ids.len()).map(|_| rng.activation()).collect();
        let input = quantize_on_device(&mut backend, &x, ids.len())?;
        for shared_input in [false, true] {
            let out = f32_buffer(&mut backend, rows * ids.len())?;
            let mut submission = backend.begin_submission()?;
            submission.v41_block_matvec(
                V41Matrix {
                    format,
                    weights: &weights,
                    scales: &scales,
                    rows,
                    cols,
                    matrices: stack,
                },
                &input.codes,
                &input.scales,
                V41Slots {
                    count: ids.len(),
                    ids: Some(&id_buffer),
                    shared_input,
                },
                &out,
                V41Output::F32,
            )?;
            submission.commit(CudaCommandWait::Completed)?;
            let gpu = read_f32(&out)?;
            for (slot, id) in ids.iter().enumerate() {
                let got = &gpu[slot * rows..(slot + 1) * rows];
                let Some(m) = usize::try_from(*id).ok().and_then(|id| matrices.get(id)) else {
                    assert!(got.iter().all(|v| v.is_nan()), "slot {slot}");
                    continue;
                };
                let row = if shared_input { 0 } else { slot };
                let host = v41_block_matvec_host(
                    format,
                    &m.weights,
                    &m.scales,
                    rows,
                    cols,
                    &input.host_codes[row * cols..(row + 1) * cols],
                    &input.host_scales[row * cols / 32..(row + 1) * cols / 32],
                );
                for (a, b) in got.iter().zip(&host) {
                    assert!(same_bits(*a, *b), "{format:?} slot {slot}");
                }
            }
        }
    }
    Ok(())
}

/// Single-row (decode) bandwidth on the model's shapes. Run explicitly:
/// `PSIONIC_V41_MATVEC_BENCH=1 cargo test ... -- --ignored v41_decode_bandwidth`.
/// Each shape rotates over enough copies to exceed the L2 cache, so reads
/// come from device memory.
#[test]
#[ignore = "measurement; needs a CUDA device"]
fn v41_decode_bandwidth() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(5);
    let cases: [(&str, V41MatvecFormat, usize, usize, usize); 7] = [
        ("wq_a", V41MatvecFormat::Fp8Block32x32, 1280, 5120, 1),
        ("shared_w1", V41MatvecFormat::Fp8Block32x32, 2304, 5120, 1),
        ("shared_w2", V41MatvecFormat::Fp8Block32x32, 5120, 2304, 1),
        ("wq_b", V41MatvecFormat::Fp8Block32x32, 32768, 1280, 1),
        ("wo_b", V41MatvecFormat::Fp8Block32x32, 5120, 8192, 1),
        ("expert_w1", V41MatvecFormat::Fp4E2M1Row32, 2304, 5120, 1),
        ("6_experts_w2", V41MatvecFormat::Fp4E2M1Row32, 5120, 2304, 6),
    ];
    let budget = 320 << 20;
    let mut report = Vec::new();
    for (label, format, rows, cols, slots) in cases {
        let per_matrix = format.weight_bytes(rows, cols) + format.scale_bytes(rows, cols);
        let copies = (budget / per_matrix).clamp(slots, 64);
        let stack = synthetic(&mut rng, format, rows, cols);
        let weights = backend.byte_buffer(&stack.weights.repeat(copies))?;
        let scales = backend.byte_buffer(&stack.scales.repeat(copies))?;
        let x: Vec<f32> = (0..cols * slots).map(|_| rng.activation()).collect();
        let input = quantize_on_device(&mut backend, &x, slots)?;
        let out = f32_buffer(&mut backend, rows * slots)?;
        // One id buffer per launch position so consecutive launches read
        // different copies.
        let rounds = copies / slots;
        let id_buffers = (0..rounds)
            .map(|r| {
                let ids: Vec<u8> = (0..slots)
                    .flat_map(|s| ((r * slots + s) as i32).to_le_bytes())
                    .collect();
                backend.byte_buffer(&ids)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let matrix = V41Matrix {
            format,
            weights: &weights,
            scales: &scales,
            rows,
            cols,
            matrices: copies,
        };
        let launch = |backend: &mut CudaBackend, n: usize| -> Result<f64, Box<dyn Error>> {
            let mut submission = backend.begin_submission()?;
            let start = std::time::Instant::now();
            for i in 0..n {
                submission.v41_block_matvec(
                    matrix,
                    &input.codes,
                    &input.scales,
                    V41Slots {
                        count: slots,
                        ids: Some(&id_buffers[i % rounds]),
                        shared_input: false,
                    },
                    &out,
                    V41Output::Bf16,
                )?;
            }
            submission.commit(CudaCommandWait::Completed)?;
            Ok(start.elapsed().as_secs_f64())
        };
        launch(&mut backend, 20)?;
        let iterations = 400;
        let seconds = launch(&mut backend, iterations)? / iterations as f64;
        let bytes = slots * (per_matrix + cols + cols / V41_BLOCK) + slots * rows * 4;
        let gbps = bytes as f64 / seconds / 1e9;
        eprintln!(
            "{label}: {format:?} {rows}x{cols} x{slots}: {:.2} us, {:.1} GB/s",
            seconds * 1e6,
            gbps
        );
        report.push(serde_json::json!({
            "case": label, "rows": rows, "cols": cols, "slots": slots,
            "bytes_per_launch": bytes, "microseconds": seconds * 1e6, "gb_per_s": gbps,
        }));
    }
    eprintln!("{}", serde_json::to_string(&report)?);
    Ok(())
}

/// The batched product (#12464) equals one matrix-vector product per entry
/// bit for bit: a dense matrix over several activation rows, and routed
/// stacks grouped by expert with shared and per-entry inputs, including an
/// invalid id, uneven tiles, and K blocks that leave a partial pass.
#[test]
fn batched_products_equal_the_host_model_per_row() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(77);
    let words = |v: &[i32]| v.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>();
    for format in [
        V41MatvecFormat::Fp8Block32x32,
        V41MatvecFormat::Fp4E2M1Row32,
    ] {
        for (rows, cols) in [(40, 96), (64, 32 * 37)] {
            let stack = 5;
            let matrices: Vec<Packed> = (0..stack)
                .map(|_| synthetic(&mut rng, format, rows, cols))
                .collect();
            let weights = backend.byte_buffer(
                &matrices
                    .iter()
                    .flat_map(|m| m.weights.clone())
                    .collect::<Vec<_>>(),
            )?;
            let scales = backend.byte_buffer(
                &matrices
                    .iter()
                    .flat_map(|m| m.scales.clone())
                    .collect::<Vec<_>>(),
            )?;
            let matrix = V41Matrix {
                format,
                weights: &weights,
                scales: &scales,
                rows,
                cols,
                matrices: stack,
            };
            // 11 tokens, top-2 routing with one invalid id.
            let (tokens, top_k) = (11, 2);
            let mut ids: Vec<i32> = (0..tokens * top_k)
                .map(|i| ((i * 7 + 3) % stack) as i32)
                .collect();
            ids[5] = -1;
            let x: Vec<f32> = (0..cols * tokens * top_k)
                .map(|_| rng.activation())
                .collect();
            let input = quantize_on_device(&mut backend, &x, tokens * top_k)?;
            let count = ids.len();
            let max_tiles = V41Batch::max_tiles(count, stack);
            let id_buffer = backend.byte_buffer(&words(&ids))?;
            let scratch = backend.byte_buffer(&vec![0; 8 * (stack + 1)])?;
            let entries = backend.byte_buffer(&vec![0; 4 * count])?;
            let tiles = backend.byte_buffer(&vec![0; 16 * max_tiles])?;
            let tile_count = backend.byte_buffer(&[0; 4])?;
            for shared_input in [true, false] {
                let out = f32_buffer(&mut backend, rows * count)?;
                let mut s = backend.begin_submission()?;
                s.v41_group_entries(
                    &id_buffer,
                    count,
                    stack,
                    &scratch,
                    &entries,
                    &tiles,
                    &tile_count,
                )?;
                s.v41_block_matmul(
                    format,
                    Some(matrix),
                    None,
                    rows,
                    cols,
                    stack,
                    V41Batch {
                        entries: &entries,
                        tiles: &tiles,
                        tile_count: &tile_count,
                        max_tiles,
                        count,
                        top_k,
                        shared_input,
                    },
                    &input.codes,
                    &input.scales,
                    count,
                    &out,
                    count,
                    V41Output::F32,
                )?;
                s.commit(CudaCommandWait::Completed)?;
                let gpu = read_f32(&out)?;
                for (e, id) in ids.iter().enumerate() {
                    let got = &gpu[e * rows..(e + 1) * rows];
                    let Some(m) = usize::try_from(*id).ok().and_then(|id| matrices.get(id)) else {
                        assert!(got.iter().all(|v| v.is_nan()), "entry {e}");
                        continue;
                    };
                    let r = if shared_input { e / top_k } else { e };
                    let host = v41_block_matvec_host(
                        format,
                        &m.weights,
                        &m.scales,
                        rows,
                        cols,
                        &input.host_codes[r * cols..(r + 1) * cols],
                        &input.host_scales[r * cols / 32..(r + 1) * cols / 32],
                    );
                    for (row, (a, b)) in got.iter().zip(&host).enumerate() {
                        assert!(
                            same_bits(*a, *b),
                            "{format:?} {rows}x{cols} entry {e} row {row}: {a} vs {b}"
                        );
                    }
                }
            }
            // A dense product: one matrix, entries 0..T in order, one tile
            // per eight rows.
            let dense_tiles: Vec<i32> = (0..count.div_ceil(V41_TILE_ROWS))
                .flat_map(|t| {
                    [
                        0,
                        (t * V41_TILE_ROWS) as i32,
                        (count - t * V41_TILE_ROWS).min(V41_TILE_ROWS) as i32,
                        0,
                    ]
                })
                .collect();
            let identity: Vec<i32> = (0..count as i32).collect();
            let entries = backend.byte_buffer(&words(&identity))?;
            let tiles = backend.byte_buffer(&words(&dense_tiles))?;
            let tile_count = backend.byte_buffer(&words(&[(dense_tiles.len() / 4) as i32]))?;
            let out = f32_buffer(&mut backend, rows * count)?;
            let mut s = backend.begin_submission()?;
            s.v41_block_matmul(
                format,
                Some(V41Matrix {
                    matrices: 1,
                    ..matrix
                }),
                None,
                rows,
                cols,
                1,
                V41Batch {
                    entries: &entries,
                    tiles: &tiles,
                    tile_count: &tile_count,
                    max_tiles: dense_tiles.len() / 4,
                    count,
                    top_k: 1,
                    shared_input: false,
                },
                &input.codes,
                &input.scales,
                count,
                &out,
                count,
                V41Output::Bf16,
            )?;
            s.commit(CudaCommandWait::Completed)?;
            let gpu = read_f32(&out)?;
            for e in 0..count {
                let host = v41_block_matvec_host(
                    format,
                    &matrices[0].weights,
                    &matrices[0].scales,
                    rows,
                    cols,
                    &input.host_codes[e * cols..(e + 1) * cols],
                    &input.host_scales[e * cols / 32..(e + 1) * cols / 32],
                );
                for (row, (a, b)) in gpu[e * rows..(e + 1) * rows].iter().zip(&host).enumerate() {
                    assert!(
                        same_bits(*a, v41_round_bf16(*b)),
                        "dense entry {e} row {row}"
                    );
                }
            }
        }
    }
    Ok(())
}

/// The `consumer-q` formats (#12465) on the device: the matrix-vector
/// kernel over a stack with device ids, the cached form, and the batched
/// product all equal the host model bit for bit.
#[test]
fn consumer_q_kernels_equal_the_host_model() -> TestResult {
    use crate::v41_expert_cache::{V41_LANE_BUS, V41CachedStack};
    use crate::v41_quantize::v41_quantize_matrix;
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(5150);
    let words = |v: &[i32]| v.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>();
    for format in [V41MatvecFormat::Q2Row32, V41MatvecFormat::Q3Row32] {
        for (rows, cols) in [(40, 96), (72, 32 * 37)] {
            let stack = 4;
            let mut w = Vec::new();
            let mut s = Vec::new();
            for _ in 0..stack {
                let values: Vec<f32> = (0..rows * cols).map(|_| rng.activation() * 0.02).collect();
                let (wm, sm) = v41_quantize_matrix(format, &values, rows, cols, None);
                w.push(wm);
                s.push(sm);
            }
            let weights = backend.byte_buffer(&w.concat())?;
            let scales = backend.byte_buffer(&s.concat())?;
            let ids: [i32; 5] = [3, 0, 2, 2, 1];
            let id_buffer = backend.byte_buffer(&words(&ids))?;
            let x: Vec<f32> = (0..cols * ids.len()).map(|_| rng.activation()).collect();
            let input = quantize_on_device(&mut backend, &x, ids.len())?;
            let host = |e: usize, slot: usize| {
                v41_block_matvec_host(
                    format,
                    &w[ids[e] as usize],
                    &s[ids[e] as usize],
                    rows,
                    cols,
                    &input.host_codes[slot * cols..(slot + 1) * cols],
                    &input.host_scales[slot * cols / 32..(slot + 1) * cols / 32],
                )
            };
            // Matrix-vector over the stack.
            let out = f32_buffer(&mut backend, rows * ids.len())?;
            let mut sub = backend.begin_submission()?;
            sub.v41_block_matvec(
                V41Matrix {
                    format,
                    weights: &weights,
                    scales: &scales,
                    rows,
                    cols,
                    matrices: stack,
                },
                &input.codes,
                &input.scales,
                V41Slots {
                    count: ids.len(),
                    ids: Some(&id_buffer),
                    shared_input: false,
                },
                &out,
                V41Output::F32,
            )?;
            sub.commit(CudaCommandWait::Completed)?;
            let gpu = read_f32(&out)?;
            for e in 0..ids.len() {
                let want = host(e, e);
                for (r, (a, b)) in gpu[e * rows..(e + 1) * rows].iter().zip(&want).enumerate() {
                    assert!(
                        same_bits(*a, *b),
                        "{format:?} matvec slot {e} row {r}: {a} vs {b}"
                    );
                }
            }
            // The cached form: matrices 1 and 3 in device slots 0 and 1, the
            // rest read from the stack over the bus lane.
            let cache_w = backend.byte_buffer(&[w[1].clone(), w[3].clone()].concat())?;
            let cache_s = backend.byte_buffer(&[s[1].clone(), s[3].clone()].concat())?;
            let map = backend.byte_buffer(&words(&[V41_LANE_BUS, 0, V41_LANE_BUS, 1]))?;
            let out = f32_buffer(&mut backend, rows * ids.len())?;
            let mut sub = backend.begin_submission()?;
            sub.v41_block_matvec_cached(
                V41CachedStack {
                    format,
                    rows,
                    cols,
                    experts: stack,
                    host: Some((&weights, &scales)),
                    cache: (&cache_w, &cache_s),
                    cache_slots: 2,
                    slot_map: &map,
                },
                &input.codes,
                &input.scales,
                V41Slots {
                    count: ids.len(),
                    ids: Some(&id_buffer),
                    shared_input: false,
                },
                &out,
                V41Output::F32,
            )?;
            sub.commit(CudaCommandWait::Completed)?;
            let gpu = read_f32(&out)?;
            for e in 0..ids.len() {
                let want = host(e, e);
                assert!(
                    gpu[e * rows..(e + 1) * rows]
                        .iter()
                        .zip(&want)
                        .all(|(a, b)| same_bits(*a, *b)),
                    "{format:?} cached slot {e}"
                );
            }
            // The batched product, grouped by matrix.
            let count = ids.len();
            let max_tiles = V41Batch::max_tiles(count, stack);
            let scratch = backend.byte_buffer(&vec![0; 8 * (stack + 1)])?;
            let entries = backend.byte_buffer(&vec![0; 4 * count])?;
            let tiles = backend.byte_buffer(&vec![0; 16 * max_tiles])?;
            let tile_count = backend.byte_buffer(&[0; 4])?;
            let out = f32_buffer(&mut backend, rows * count)?;
            let mut sub = backend.begin_submission()?;
            sub.v41_group_entries(
                &id_buffer,
                count,
                stack,
                &scratch,
                &entries,
                &tiles,
                &tile_count,
            )?;
            sub.v41_block_matmul(
                format,
                Some(V41Matrix {
                    format,
                    weights: &weights,
                    scales: &scales,
                    rows,
                    cols,
                    matrices: stack,
                }),
                None,
                rows,
                cols,
                stack,
                V41Batch {
                    entries: &entries,
                    tiles: &tiles,
                    tile_count: &tile_count,
                    max_tiles,
                    count,
                    top_k: 1,
                    shared_input: false,
                },
                &input.codes,
                &input.scales,
                count,
                &out,
                count,
                V41Output::F32,
            )?;
            sub.commit(CudaCommandWait::Completed)?;
            let gpu = read_f32(&out)?;
            for e in 0..count {
                let want = host(e, e);
                assert!(
                    gpu[e * rows..(e + 1) * rows]
                        .iter()
                        .zip(&want)
                        .all(|(a, b)| same_bits(*a, *b)),
                    "{format:?} batched entry {e}"
                );
            }
        }
    }
    Ok(())
}

/// Times the routed-expert product at V4.1's shape: 2,048 tokens, top-6 of
/// 384 FP4 experts, gate rows 2,048 by 4,096 columns (#12464). Run with
/// `--ignored --nocapture`.
#[test]
#[ignore = "timing on a CUDA device"]
fn batched_expert_product_timing() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let mut rng = Rng(5);
    let words = |v: &[i32]| v.iter().flat_map(|i| i.to_le_bytes()).collect::<Vec<u8>>();
    let format = V41MatvecFormat::Fp4E2M1Row32;
    let (rows, cols, stack, tokens, top_k) = (2048, 4096, 384, 2048, 6);
    let one = synthetic(&mut rng, format, rows, cols);
    let weights = backend.byte_buffer(&one.weights.repeat(stack))?;
    let scales = backend.byte_buffer(&one.scales.repeat(stack))?;
    let matrix = V41Matrix {
        format,
        weights: &weights,
        scales: &scales,
        rows,
        cols,
        matrices: stack,
    };
    let ids: Vec<i32> = (0..tokens * top_k)
        .map(|_| (rng.next() % stack as u64) as i32)
        .collect();
    let x: Vec<f32> = (0..cols * tokens).map(|_| rng.activation()).collect();
    let input = quantize_on_device(&mut backend, &x, tokens)?;
    let count = ids.len();
    let max_tiles = V41Batch::max_tiles(count, stack);
    let id_buffer = backend.byte_buffer(&words(&ids))?;
    let scratch = backend.byte_buffer(&vec![0; 8 * (stack + 1)])?;
    let entries = backend.byte_buffer(&vec![0; 4 * count])?;
    let tiles = backend.byte_buffer(&vec![0; 16 * max_tiles])?;
    let tile_count = backend.byte_buffer(&[0; 4])?;
    let out = f32_buffer(&mut backend, rows * count)?;
    for round in 0..3 {
        let start = std::time::Instant::now();
        let mut s = backend.begin_submission()?;
        s.v41_group_entries(
            &id_buffer,
            count,
            stack,
            &scratch,
            &entries,
            &tiles,
            &tile_count,
        )?;
        s.v41_block_matmul(
            format,
            Some(matrix),
            None,
            rows,
            cols,
            stack,
            V41Batch {
                entries: &entries,
                tiles: &tiles,
                tile_count: &tile_count,
                max_tiles,
                count,
                top_k,
                shared_input: true,
            },
            &input.codes,
            &input.scales,
            tokens,
            &out,
            count,
            V41Output::F32,
        )?;
        s.commit(CudaCommandWait::Completed)?;
        let seconds = start.elapsed().as_secs_f64();
        let fma = (count * rows * cols) as f64;
        eprintln!(
            "TIMING round {round}: {:.1} ms, {:.2} TFMA/s",
            seconds * 1e3,
            fma / seconds / 1e12
        );
    }
    Ok(())
}
