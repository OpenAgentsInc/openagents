//! Numerical boundaries from `kernel.py:22-199,311-389` and `model.py:181-207`.
//! Conversion uses nearest-even finite codes. Device conversion instructions
//! and GEMM reduction order still need the original-weight oracle comparison.

use super::{CpuError, MatrixRows, finite, matvec, product, shape};
use crate::deepseek_v41_weights::{decode_e2m1, decode_f8_e4m3fn, decode_f8_e8m0fnu};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    Float32,
    ReferenceBf16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixPrecision {
    F32,
    Bf16,
    Fp8,
    Fp4,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantization {
    Fp8,
    IndexFp4,
    CompressedFp4,
}

pub(crate) fn bf16(x: f32) -> f32 {
    half::bf16::from_f32(x).to_f32()
}
pub fn round_bf16(input: &[f32]) -> Result<Vec<f32>, CpuError> {
    finite("BF16 input", input)?;
    let out: Vec<_> = input.iter().map(|x| bf16(*x)).collect();
    finite("BF16 output", &out)?;
    Ok(out)
}

fn nearest(x: f32, values: &[f32]) -> u8 {
    let hi = values.partition_point(|v| *v < x).min(values.len() - 1);
    if hi == 0 {
        return 0;
    }
    let lo = hi - 1;
    let a = x - values[lo];
    let b = values[hi] - x;
    if a < b || (a == b && lo % 2 == 0) {
        lo as u8
    } else {
        hi as u8
    }
}

pub fn encode_fp8(value: f32) -> Result<u8, CpuError> {
    if !value.is_finite() {
        return Err(CpuError::NonFinite("FP8 input"));
    }
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    let values = TABLE.get_or_init(|| (0..127).map(decode_f8_e4m3fn).collect());
    Ok(nearest(value.abs().min(448.0), values) | if value.is_sign_negative() { 0x80 } else { 0 })
}
pub fn encode_fp4(value: f32) -> Result<u8, CpuError> {
    if !value.is_finite() {
        return Err(CpuError::NonFinite("FP4 input"));
    }
    Ok(nearest(
        value.abs().min(6.0),
        &[0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0],
    ) | if value.is_sign_negative() { 8 } else { 0 })
}

fn ceil_power_scale(value: f32) -> Result<(f32, u8), CpuError> {
    if !value.is_finite() || value < f32::MIN_POSITIVE {
        return Err(CpuError::Invalid("power-of-two scale range"));
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 255) + u32::from(bits & 0x7fffff != 0);
    if exponent >= 255 {
        return Err(CpuError::Invalid("power-of-two scale overflow"));
    }
    Ok((f32::from_bits(exponent << 23), exponent as u8))
}

#[derive(Debug, Clone)]
pub struct Quantized {
    kind: Quantization,
    block: usize,
    elements: usize,
    codes: Vec<u8>,
    scales: Vec<u8>,
}
impl Quantized {
    pub fn codes(&self) -> &[u8] {
        &self.codes
    }
    pub fn scales(&self) -> &[u8] {
        &self.scales
    }
    pub fn dequantize(&self) -> Result<Vec<f32>, CpuError> {
        let mut out = Vec::with_capacity(self.elements);
        for i in 0..self.elements {
            let scale = if self.kind == Quantization::CompressedFp4 {
                decode_f8_e4m3fn(self.scales[i / self.block])
            } else {
                decode_f8_e8m0fnu(self.scales[i / self.block])
            };
            let value = if self.kind == Quantization::Fp8 {
                decode_f8_e4m3fn(self.codes[i])
            } else {
                decode_e2m1((self.codes[i / 2] >> ((i % 2) * 4)) & 15)
            };
            out.push(value * scale);
        }
        finite("dequantized activation", &out)?;
        Ok(out)
    }
}

/// One or more complete blocks; callers split head/row boundaries explicitly.
pub fn quantize(input: &[f32], kind: Quantization) -> Result<Quantized, CpuError> {
    let block = if kind == Quantization::CompressedFp4 {
        16
    } else {
        32
    };
    if input.is_empty() || !input.len().is_multiple_of(block) {
        return Err(CpuError::Invalid("activation block dimensions"));
    }
    finite("activation input", input)?;
    let mut codes = Vec::with_capacity(input.len());
    let mut scales = Vec::new();
    for values in input.chunks_exact(block) {
        let amax = values.iter().map(|x| x.abs()).fold(0.0, f32::max);
        let (scale, byte) = match kind {
            Quantization::Fp8 => ceil_power_scale(amax.max(1e-4) * (1.0 / 448.0))?,
            Quantization::IndexFp4 => {
                ceil_power_scale(amax.max(6.0 * f32::MIN_POSITIVE) * (1.0 / 6.0))?
            }
            Quantization::CompressedFp4 => {
                let value = amax.max(6.0 * 2.0_f32.powi(-9)) / 6.0;
                if value > 448.0 {
                    return Err(CpuError::Invalid("compressed KV scale exceeds E4M3 range"));
                }
                let byte = encode_fp8(value)?;
                (decode_f8_e4m3fn(byte), byte)
            }
        };
        scales.push(byte);
        for x in values {
            let value = *x / scale;
            codes.push(if kind == Quantization::Fp8 {
                encode_fp8(value)?
            } else {
                encode_fp4(value)?
            });
        }
    }
    if kind != Quantization::Fp8 {
        codes = codes.chunks_exact(2).map(|p| p[0] | p[1] << 4).collect();
    }
    Ok(Quantized {
        kind,
        block,
        elements: input.len(),
        codes,
        scales,
    })
}

pub fn inplace_bf16(input: &[f32], kind: Quantization) -> Result<Vec<f32>, CpuError> {
    round_bf16(&quantize(&round_bf16(input)?, kind)?.dequantize()?)
}

/// FP4 and FP8 matrices both consume per-32 FP8 activations. F32 is also
/// used for promoted compressor weights; the final head uses matvec directly.
pub fn linear(
    matrix: &dyn MatrixRows,
    input: &[f32],
    arithmetic: Arithmetic,
) -> Result<Vec<f32>, CpuError> {
    if arithmetic == Arithmetic::Float32 || matrix.precision() == MatrixPrecision::F32 {
        return matvec(matrix, input);
    }
    let input = round_bf16(input)?;
    let activation = if matches!(
        matrix.precision(),
        MatrixPrecision::Fp8 | MatrixPrecision::Fp4
    ) {
        quantize(&input, Quantization::Fp8)?.dequantize()?
    } else {
        input
    };
    let (rows, cols) = matrix.dimensions();
    shape("linear input", &activation, cols)?;
    if rows == 0 {
        return Err(CpuError::Invalid("empty linear matrix"));
    }
    let mut out = Vec::with_capacity(rows);
    for i in 0..rows {
        let mut row = matrix.row(i)?;
        shape("linear weight row", &row, cols)?;
        if matrix.precision() == MatrixPrecision::Bf16 {
            row = round_bf16(&row)?;
        }
        out.push(row.iter().zip(&activation).map(|(w, x)| w * x).sum());
    }
    round_bf16(&out)
}

/// The device kernel rounds unnormalized softmax weights to BF16 before the
/// value product, updating the running maximum in blocks of 64 indices.
pub fn sparse_attention_bf16(
    query: &[f32],
    heads: usize,
    width: usize,
    kv: &[f32],
    indices: &[i32],
    sinks: &[f32],
    scale: f32,
) -> Result<Vec<f32>, CpuError> {
    if heads == 0 || width == 0 || !kv.len().is_multiple_of(width) || !super::positive(scale) {
        return Err(CpuError::Invalid("BF16 attention dimensions"));
    }
    shape("BF16 attention query", query, product(heads, width)?)?;
    shape("BF16 sinks", sinks, heads)?;
    let query = round_bf16(query)?;
    let kv = round_bf16(kv)?;
    if indices
        .iter()
        .any(|i| *i < -1 || (*i >= 0 && *i as usize >= kv.len() / width))
    {
        return Err(CpuError::Invalid("BF16 attention index"));
    }
    let mut out = vec![0.0; query.len()];
    for h in 0..heads {
        let mut max = -1e30_f32;
        let mut sum = 0.0_f32;
        for chunk in indices.chunks(64) {
            let mut scores = Vec::new();
            let previous = max;
            for id in chunk.iter().copied().filter(|i| *i >= 0) {
                let id = id as usize;
                let score = query[h * width..(h + 1) * width]
                    .iter()
                    .zip(&kv[id * width..(id + 1) * width])
                    .map(|(q, k)| q * k)
                    .sum::<f32>()
                    * scale;
                if !score.is_finite() {
                    return Err(CpuError::NonFinite("BF16 attention score"));
                }
                max = max.max(score);
                scores.push((id, score));
            }
            let rescale = (previous - max).exp();
            sum *= rescale;
            for x in &mut out[h * width..(h + 1) * width] {
                *x *= rescale;
            }
            for (id, score) in scores {
                let weight = (score - max).exp();
                sum += weight;
                for d in 0..width {
                    out[h * width + d] += bf16(weight) * kv[id * width + d];
                }
            }
        }
        sum += (sinks[h] - max).exp();
        for x in &mut out[h * width..(h + 1) * width] {
            *x /= sum;
        }
    }
    round_bf16(&out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct Case {
        kind: String,
        input: Vec<f32>,
        codes: Vec<u8>,
        scales: Vec<u8>,
        decoded: Vec<f32>,
    }
    #[derive(Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
        query: Vec<f32>,
        kv: Vec<f32>,
        indices: Vec<i32>,
        sinks: Vec<f32>,
        attention: Vec<f32>,
    }
    #[test]
    fn codes_scales_and_decoded_values_match_independent_fixture()
    -> Result<(), Box<dyn std::error::Error>> {
        let f: Fixture = serde_json::from_str(include_str!(
            "../testdata/deepseek_v41_cpu/quantization.json"
        ))?;
        for c in f.cases {
            let kind = match c.kind.as_str() {
                "fp8" => Quantization::Fp8,
                "index_fp4" => Quantization::IndexFp4,
                _ => Quantization::CompressedFp4,
            };
            let q = quantize(&c.input, kind)?;
            assert_eq!(q.codes(), c.codes);
            assert_eq!(q.scales(), c.scales);
            assert_eq!(q.dequantize()?, c.decoded);
        }
        assert_eq!(
            sparse_attention_bf16(&f.query, 1, 2, &f.kv, &f.indices, &f.sinks, 0.5)?,
            f.attention
        );
        Ok(())
    }
    #[test]
    fn nearest_even_signed_zero_floors_and_saturation() -> Result<(), CpuError> {
        assert_eq!(encode_fp8(1.0625)?, 0x38);
        assert_eq!(encode_fp8(1.1875)?, 0x3a);
        assert_eq!(encode_fp4(0.25)?, 0);
        assert_eq!(encode_fp4(0.75)?, 2);
        assert_eq!(encode_fp4(5.0)?, 6);
        assert_eq!(encode_fp8(-0.0)?, 128);
        assert_eq!(encode_fp4(-0.0)?, 8);
        assert_eq!(encode_fp8(1000.0)?, 126);
        assert_eq!(encode_fp4(1000.0)?, 7);
        assert_eq!(quantize(&[0.0; 32], Quantization::Fp8)?.scales(), &[105]);
        assert_eq!(quantize(&[0.0; 32], Quantization::IndexFp4)?.scales(), &[1]);
        assert_eq!(
            quantize(&[0.0; 16], Quantization::CompressedFp4)?.scales(),
            &[1]
        );
        assert_eq!(
            round_bf16(&[1.00390625, 1.01171875, -0.0])?
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            vec![
                1.0_f32.to_bits(),
                1.015625_f32.to_bits(),
                (-0.0_f32).to_bits()
            ]
        );
        Ok(())
    }
    struct Typed {
        precision: MatrixPrecision,
    }
    impl MatrixRows for Typed {
        fn dimensions(&self) -> (usize, usize) {
            (1, 32)
        }
        fn row(&self, _: usize) -> Result<Vec<f32>, CpuError> {
            Ok(vec![0.5; 32])
        }
        fn precision(&self) -> MatrixPrecision {
            self.precision
        }
    }
    #[test]
    fn fp4_and_fp8_weight_paths_both_use_fp8_activations() -> Result<(), CpuError> {
        let mut x = vec![1.0625; 32];
        x[0] = 448.0;
        let expected = round_bf16(&[quantize(&x, Quantization::Fp8)?
            .dequantize()?
            .iter()
            .sum::<f32>()
            * 0.5])?;
        for precision in [MatrixPrecision::Fp4, MatrixPrecision::Fp8] {
            assert_eq!(
                linear(&Typed { precision }, &x, Arithmetic::ReferenceBf16)?,
                expected
            );
        }
        assert_ne!(
            linear(
                &Typed {
                    precision: MatrixPrecision::Fp8
                },
                &x,
                Arithmetic::Float32
            )?,
            expected
        );
        Ok(())
    }
    #[test]
    fn invalid_quantization_and_attention_inputs_are_refused() {
        assert!(quantize(&[1.0; 31], Quantization::Fp8).is_err());
        assert!(quantize(&[f32::NAN; 32], Quantization::IndexFp4).is_err());
        assert!(quantize(&[f32::MAX; 16], Quantization::CompressedFp4).is_err());
        assert!(round_bf16(&[f32::MAX]).is_err());
        assert!(sparse_attention_bf16(&[1.0], 1, 1, &[], &[0], &[0.0], 1.0).is_err());
    }

    struct QueryWeights;
    struct QueryMatrix {
        expanded: bool,
    }
    impl MatrixRows for QueryMatrix {
        fn dimensions(&self) -> (usize, usize) {
            (if self.expanded { 64 } else { 32 }, 32)
        }
        fn precision(&self) -> MatrixPrecision {
            MatrixPrecision::Fp8
        }
        fn row(&self, i: usize) -> Result<Vec<f32>, CpuError> {
            if self.expanded {
                let mut r = vec![0.0; 32];
                r[i % 32] = 1.0;
                Ok(r)
            } else {
                Ok(vec![if i % 2 == 0 { 0.5 } else { 1.0 }; 32])
            }
        }
    }
    impl super::super::projection::WeightSource for QueryWeights {
        fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            Ok(Box::new(QueryMatrix {
                expanded: name.ends_with("wq_b.weight"),
            }))
        }
        fn vector(&self, _: &str) -> Result<Vec<f32>, CpuError> {
            Ok(vec![1.0; 32])
        }
    }
    #[test]
    fn precision_mode_reaches_the_named_projection_path() -> Result<(), CpuError> {
        use super::super::projection::{AttentionProjections, Geometry};
        let g = Geometry {
            hidden: 32,
            heads: 2,
            head_dim: 32,
            q_rank: 32,
            o_groups: 2,
            o_rank: 32,
            index_heads: 2,
            index_dim: 32,
            rope_dim: 8,
            ..Geometry::flash()
        };
        let p = AttentionProjections::new(&QueryWeights, 2, g)?
            .with_arithmetic(Arithmetic::ReferenceBf16)?;
        let mut x = vec![1.0625; 32];
        x[0] = 448.0;
        let q = p.query(&x)?;
        let (a, b) = (bf16(239.5), bf16(479.0));
        let norm = ((a * a + b * b) / 2.0 + 1e-20).sqrt().recip();
        let expected: Vec<_> = (0..32)
            .map(|i| bf16(if i % 2 == 0 { a * norm } else { b * norm }))
            .collect();
        assert_eq!(q.low_rank, expected);
        // wq_b consumes FP8-quantized normalized activations as well.
        let expected = round_bf16(&quantize(&expected, Quantization::Fp8)?.dequantize()?)?;
        assert_eq!(q.heads, [expected.clone(), expected].concat());
        Ok(())
    }

    #[test]
    fn subnormal_scales_and_bf16_compression_boundary_are_explicit() -> Result<(), CpuError> {
        let q = quantize(&[f32::from_bits(1); 32], Quantization::IndexFp4)?;
        assert_eq!(q.scales(), &[1]);
        assert!(q.dequantize()?.iter().all(|v| *v == 0.0));
        use super::super::attention::CompressionState;
        let mut c = CompressionState::new(2, 2)?.with_arithmetic(Arithmetic::ReferenceBf16);
        let (a, b) = ([1.003, 2.003], [3.007, 4.007]);
        assert!(c.push(0, &a, Some(&[0.0; 2]), &[1.0; 2], 1e-20)?.is_none());
        let got = c.push(1, &b, Some(&[0.0; 2]), &[1.0; 2], 1e-20)?;
        let pooled = round_bf16(&[(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])?;
        assert_eq!(
            got,
            Some(round_bf16(&super::super::rms_norm(
                &pooled, &[1.0; 2], 1e-20
            )?)?)
        );
        Ok(())
    }
}
