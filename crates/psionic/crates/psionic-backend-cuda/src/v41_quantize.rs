//! The `consumer-q` quantizer for DeepSeek-V4.1 routed experts (#12465).
//!
//! **Lossy.** The `consumer-q` profile is a separate, unqualified serving
//! profile with its own quality card (owner, 2026-10-21). It never replaces
//! the exact path and is never called qualified against the oracle.
//!
//! Each row of an expert matrix gets an `f32` scale, each 32-value block a
//! log-scale byte `u` (the block scale is `row × 2^((u - 255) / 32)`), and
//! each value a 2- or 3-bit code into the Lloyd–Max levels of a unit normal
//! ([`V41MatvecFormat::Q2Row32`], [`V41MatvecFormat::Q3Row32`]). For each
//! block the quantizer tries the scales near the block's own maximum and
//! the least-squares scale of the best codes, and keeps the one with the
//! smallest error weighted by an importance per input column (the mean
//! square of the activations the column sees, from calibration), as
//! importance-matrix quantization does. Without importances every column
//! weighs the same.
//!
//! Project Maya's V4.1 quants (`mw00/project-maya` at `451f9cb6a2`, MIT) use
//! ggml's i-quants with per-expert importance matrices and GPTQ error
//! feedback; this is a simpler reimplementation of the importance-weighted
//! part with formats of our own that our kernels and host lane decode.

use crate::v41_matvec::{
    V41_BLOCK, V41_Q2_LEVELS, V41_Q3_LEVELS, V41MatvecFormat, v41_q_block_scale,
};

/// The levels of a `consumer-q` format, ascending.
#[must_use]
pub fn v41_q_levels(format: V41MatvecFormat) -> &'static [f32] {
    match format.q_bits() {
        Some(2) => &V41_Q2_LEVELS,
        _ => &V41_Q3_LEVELS,
    }
}

/// The nearest level's code (ties to the lower code).
fn nearest(levels: &[f32], v: f32) -> usize {
    let mut best = 0;
    let mut err = (v - levels[0]).abs();
    for (q, l) in levels.iter().enumerate().skip(1) {
        let e = (v - l).abs();
        if e < err {
            err = e;
            best = q;
        }
    }
    best
}

/// The block scale byte nearest a wanted scale, at or below 255.
fn scale_byte(d: f32, wanted: f32) -> u8 {
    if d <= 0.0 || wanted <= 0.0 {
        return 0;
    }
    let u = 255.0 + 32.0 * (f64::from(wanted) / f64::from(d)).log2();
    u.round().clamp(0.0, 255.0) as u8
}

/// One block's codes and error at block scale `s`.
fn encode_block(levels: &[f32], values: &[f32], weight: &[f32], s: f32, codes: &mut [u8]) -> f64 {
    let mut err = 0.0_f64;
    for ((v, w), c) in values.iter().zip(weight).zip(codes.iter_mut()) {
        let q = if s > 0.0 { nearest(levels, v / s) } else { 0 };
        *c = q as u8;
        let e = f64::from(v - s * levels[q]);
        err += f64::from(*w) * e * e;
    }
    err
}

/// Quantizes a row-major `rows × cols` matrix to a `consumer-q` format,
/// returning the format's weight and scale bytes. `importance` holds one
/// non-negative weight per column.
///
/// # Panics
///
/// Panics when `format` isn't a `consumer-q` format, `cols` isn't a
/// multiple of 32, or a slice has the wrong length.
#[must_use]
pub fn v41_quantize_matrix(
    format: V41MatvecFormat,
    values: &[f32],
    rows: usize,
    cols: usize,
    importance: Option<&[f32]>,
) -> (Vec<u8>, Vec<u8>) {
    let bits = format.q_bits().expect("a consumer-q format");
    assert!(cols.is_multiple_of(V41_BLOCK));
    assert_eq!(values.len(), rows * cols);
    let blocks = cols / V41_BLOCK;
    let levels = v41_q_levels(format);
    let top = levels[levels.len() - 1];
    let ones = vec![1.0_f32; cols];
    let importance = importance.unwrap_or(&ones);
    assert_eq!(importance.len(), cols);
    let mut weights = vec![0_u8; format.weight_bytes(rows, cols)];
    let mut scales = vec![0_u8; format.scale_bytes(rows, cols)];
    let rows_at = (rows * blocks).div_ceil(4) * 4;
    let block_bytes = 4 * bits;
    let mut codes = [0_u8; V41_BLOCK];
    let mut trial = [0_u8; V41_BLOCK];
    for r in 0..rows {
        let row = &values[r * cols..(r + 1) * cols];
        let amax = row.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
        let d = if amax > 0.0 && amax.is_finite() {
            amax / top
        } else {
            0.0
        };
        scales[rows_at + 4 * r..rows_at + 4 * r + 4].copy_from_slice(&d.to_le_bytes());
        for b in 0..blocks {
            let v = &row[b * V41_BLOCK..(b + 1) * V41_BLOCK];
            let w = &importance[b * V41_BLOCK..(b + 1) * V41_BLOCK];
            let bmax = v.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
            let start = scale_byte(d, bmax / top);
            let mut best = (f64::INFINITY, start);
            // The scales near the block maximum's, then the least-squares
            // scale of the first guess's codes; no allocation per block.
            let mut candidates = [0_u8; 18];
            for (slot, k) in candidates.iter_mut().zip(-12_i32..=4) {
                *slot = (i32::from(start) + k).clamp(0, 255) as u8;
            }
            let mut n = 17;
            let s0 = v41_q_block_scale(d, start);
            encode_block(levels, v, w, s0, &mut trial);
            let (num, den) =
                v.iter()
                    .zip(w)
                    .zip(&trial)
                    .fold((0.0_f64, 0.0_f64), |(n, m), ((x, wi), q)| {
                        let l = f64::from(levels[*q as usize]);
                        (
                            n + f64::from(*wi) * f64::from(*x) * l,
                            m + f64::from(*wi) * l * l,
                        )
                    });
            if den > 0.0 && num > 0.0 {
                candidates[n] = scale_byte(d, (num / den) as f32);
                n += 1;
            }
            let candidates = &mut candidates[..n];
            candidates.sort_unstable();
            let mut last = None;
            for &u in candidates.iter() {
                if last == Some(u) {
                    continue;
                }
                last = Some(u);
                let s = v41_q_block_scale(d, u);
                let err = encode_block(levels, v, w, s, &mut trial);
                if err < best.0 {
                    best = (err, u);
                    codes = trial;
                }
            }
            scales[r * blocks + b] = best.1;
            let out =
                &mut weights[(r * blocks + b) * block_bytes..(r * blocks + b + 1) * block_bytes];
            for (j, q) in codes.iter().enumerate() {
                let at = bits * j;
                let packed = u32::from(*q) << (at % 8);
                out[at / 8] |= packed as u8;
                if at % 8 + bits > 8 {
                    out[at / 8 + 1] |= (packed >> 8) as u8;
                }
            }
        }
    }
    (weights, scales)
}

/// The values a `consumer-q` matrix stands for, row-major.
///
/// # Panics
///
/// Panics when the slices don't fit the shape.
#[must_use]
pub fn v41_dequantize_matrix(
    format: V41MatvecFormat,
    weights: &[u8],
    scales: &[u8],
    rows: usize,
    cols: usize,
) -> Vec<f32> {
    let bits = format.q_bits().expect("a consumer-q format");
    let blocks = cols / V41_BLOCK;
    let levels = v41_q_levels(format);
    let rows_at = (rows * blocks).div_ceil(4) * 4;
    let block_bytes = 4 * bits;
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        let d = f32::from_le_bytes(
            scales[rows_at + 4 * r..rows_at + 4 * r + 4]
                .try_into()
                .unwrap_or([0; 4]),
        );
        for b in 0..blocks {
            let s = v41_q_block_scale(d, scales[r * blocks + b]);
            let block =
                &weights[(r * blocks + b) * block_bytes..(r * blocks + b + 1) * block_bytes];
            for j in 0..V41_BLOCK {
                out.push(s * levels[crate::v41_matvec::v41_q_code(bits, block, j)]);
            }
        }
    }
    out
}

/// Quantizes with GPTQ error feedback inside each 32-column scale group.
/// `hessians` holds the uncentered activation second moments as
/// `[cols / 32, 32, 32]`, in column order. A one-percent diagonal damping
/// makes low-rank calibration groups invertible. Correlations between
/// different scale groups are not modeled.
///
/// # Panics
///
/// Panics for the same invalid shapes as [`v41_quantize_matrix`], or an
/// incorrectly sized Hessian.
#[must_use]
pub fn v41_quantize_matrix_gptq(
    format: V41MatvecFormat,
    values: &[f32],
    rows: usize,
    cols: usize,
    hessians: &[f32],
) -> (Vec<u8>, Vec<u8>) {
    const N: usize = V41_BLOCK;
    assert_eq!(hessians.len(), cols * N);
    let diagonal: Vec<f32> = (0..cols)
        .map(|c| hessians[(c / N * N + c % N) * N + c % N].max(1e-12))
        .collect();
    let (mut weights, scales) = v41_quantize_matrix(format, values, rows, cols, Some(&diagonal));
    let bits = format.q_bits().expect("a consumer-q format");
    let levels = v41_q_levels(format);
    let blocks = cols / N;
    let rows_at = (rows * blocks).div_ceil(4) * 4;
    for b in 0..blocks {
        let h = &hessians[b * N * N..(b + 1) * N * N];
        let Some(upper) = inverse_cholesky(h) else {
            continue;
        };
        for r in 0..rows {
            let d = f32::from_le_bytes(
                scales[rows_at + 4 * r..rows_at + 4 * r + 4]
                    .try_into()
                    .unwrap(),
            );
            let scale = v41_q_block_scale(d, scales[r * blocks + b]);
            if scale <= 0.0 {
                continue;
            }
            let original = &values[r * cols + b * N..r * cols + (b + 1) * N];
            let at = (r * blocks + b) * 4 * bits;
            let encoded = &mut weights[at..at + 4 * bits];
            let mut adjusted = [0.0_f64; N];
            for (v, x) in adjusted.iter_mut().zip(original) {
                *v = f64::from(*x);
            }
            let mut codes = [0_u8; N];
            let mut errors = [0.0_f64; N];
            let mut initial_errors = [0.0_f64; N];
            for j in 0..N {
                let q = nearest(levels, (adjusted[j] / f64::from(scale)) as f32);
                codes[j] = q as u8;
                let quantized = f64::from(scale) * f64::from(levels[q]);
                errors[j] = f64::from(original[j]) - quantized;
                initial_errors[j] = f64::from(original[j])
                    - f64::from(scale)
                        * f64::from(levels[crate::v41_matvec::v41_q_code(bits, encoded, j)]);
                let error = (adjusted[j] - quantized) / upper[j * N + j];
                for k in j + 1..N {
                    adjusted[k] -= error * upper[j * N + k];
                }
            }
            // Feedback must improve the measured activation objective,
            // including when damping was needed for a sparse expert.
            let loss = |e: &[f64; N]| -> f64 {
                (0..N)
                    .map(|j| {
                        (0..N)
                            .map(|k| e[j] * f64::from(h[j * N + k]) * e[k])
                            .sum::<f64>()
                    })
                    .sum()
            };
            if loss(&errors) >= loss(&initial_errors) {
                continue;
            }
            encoded.fill(0);
            for (j, q) in codes.iter().enumerate() {
                let bit = bits * j;
                let word = u16::from(*q) << (bit % 8);
                encoded[bit / 8] |= word as u8;
                if bit % 8 + bits > 8 {
                    encoded[bit / 8 + 1] |= (word >> 8) as u8;
                }
            }
        }
    }
    (weights, scales)
}

// Upper Cholesky factor of the inverse damped activation Hessian: GPTQ's
// update uses U[j, k] / U[j, j] to compensate later columns for column j.
fn inverse_cholesky(h: &[f32]) -> Option<Vec<f64>> {
    const N: usize = V41_BLOCK;
    let trace: f64 = (0..N).map(|j| f64::from(h[j * N + j])).sum();
    if !trace.is_finite() || trace <= 0.0 || h.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let cholesky = |a: &[f64]| -> Option<Vec<f64>> {
        let mut l = vec![0.0; N * N];
        for j in 0..N {
            for k in 0..=j {
                let v = a[j * N + k] - (0..k).map(|m| l[j * N + m] * l[k * N + m]).sum::<f64>();
                if j == k {
                    if v <= 0.0 || !v.is_finite() {
                        return None;
                    }
                    l[j * N + k] = v.sqrt();
                } else {
                    l[j * N + k] = v / l[k * N + k];
                }
            }
        }
        Some(l)
    };
    let mut a: Vec<f64> = h.iter().map(|v| f64::from(*v)).collect();
    for j in 0..N {
        a[j * N + j] += 0.01 * trace / N as f64;
    }
    let l = cholesky(&a)?;
    let mut inverse_l = vec![0.0; N * N];
    for col in 0..N {
        for row in col..N {
            let target = if row == col { 1.0 } else { 0.0 };
            inverse_l[row * N + col] = (target
                - (col..row)
                    .map(|k| l[row * N + k] * inverse_l[k * N + col])
                    .sum::<f64>())
                / l[row * N + row];
        }
    }
    let mut inverse = vec![0.0; N * N];
    for j in 0..N {
        for k in 0..N {
            inverse[j * N + k] = (j.max(k)..N)
                .map(|m| inverse_l[m * N + j] * inverse_l[m * N + k])
                .sum();
        }
    }
    let lower = cholesky(&inverse)?;
    Some((0..N * N).map(|i| lower[(i % N) * N + i / N]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gaussian(seed: &mut u64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|_| {
                let mut u = || {
                    *seed ^= *seed << 13;
                    *seed ^= *seed >> 7;
                    *seed ^= *seed << 17;
                    ((*seed >> 11) as f64 + 0.5) / (1_u64 << 53) as f64
                };
                ((-2.0 * u().ln()).sqrt() * (std::f64::consts::TAU * u()).cos()) as f32
            })
            .collect()
    }

    /// Quantizing Gaussian rows gives the error a Lloyd–Max quantizer of
    /// that many bits gets, near enough, and more importance on a column
    /// lowers its own error.
    #[test]
    fn quantizer_error_and_importance() {
        let mut seed = 0x1234_5678_9abc_def1;
        let (rows, cols) = (64, 256);
        let values: Vec<f32> = gaussian(&mut seed, rows * cols)
            .iter()
            .map(|v| v * 0.02)
            .collect();
        let power: f64 = values.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
        for (format, ceiling) in [
            (V41MatvecFormat::Q2Row32, 0.15),
            (V41MatvecFormat::Q3Row32, 0.05),
        ] {
            let (w, s) = v41_quantize_matrix(format, &values, rows, cols, None);
            assert_eq!(w.len(), format.weight_bytes(rows, cols));
            assert_eq!(s.len(), format.scale_bytes(rows, cols));
            let back = v41_dequantize_matrix(format, &w, &s, rows, cols);
            let err: f64 = values
                .iter()
                .zip(&back)
                .map(|(a, b)| f64::from(a - b).powi(2))
                .sum();
            let relative = err / power;
            assert!(relative < ceiling, "{format:?}: relative error {relative}");
            // Column 5 weighted heavily: its error drops.
            let mut imp = vec![1.0_f32; cols];
            imp[5] = 1000.0;
            let (w2, s2) = v41_quantize_matrix(format, &values, rows, cols, Some(&imp));
            let back2 = v41_dequantize_matrix(format, &w2, &s2, rows, cols);
            let col = |b: &[f32]| -> f64 {
                (0..rows)
                    .map(|r| f64::from(values[r * cols + 5] - b[r * cols + 5]).powi(2))
                    .sum()
            };
            assert!(col(&back2) <= col(&back), "{format:?}");
        }
    }

    #[test]
    fn gptq_feedback_uses_correlations_and_reduces_activation_error() {
        let (rows, cols) = (32, 64);
        let mut seed = 0x1234_5678_9abc_def1;
        let values = gaussian(&mut seed, rows * cols);
        let mut h = vec![0.0; cols * V41_BLOCK];
        for block in h.chunks_exact_mut(V41_BLOCK * V41_BLOCK) {
            for j in 0..V41_BLOCK {
                block[j * V41_BLOCK + j] = 1.0;
                block[j * V41_BLOCK + (j ^ 1)] = 0.98;
            }
        }
        for format in [V41MatvecFormat::Q2Row32, V41MatvecFormat::Q3Row32] {
            let (w, s) = v41_quantize_matrix(format, &values, rows, cols, None);
            let ordinary = v41_dequantize_matrix(format, &w, &s, rows, cols);
            let (qw, qs) = v41_quantize_matrix_gptq(format, &values, rows, cols, &h);
            let feedback = v41_dequantize_matrix(format, &qw, &qs, rows, cols);
            let loss = |quantized: &[f32]| -> f64 {
                values
                    .chunks_exact(2)
                    .zip(quantized.chunks_exact(2))
                    .map(|(v, q)| {
                        let (a, b) = (f64::from(v[0] - q[0]), f64::from(v[1] - q[1]));
                        a * a + b * b + 1.96 * a * b
                    })
                    .sum()
            };
            assert!(loss(&feedback) < 0.8 * loss(&ordinary), "{format:?}");
            // The diagonal is identical to ordinary importance weighting;
            // only the cross-column moments can explain the improvement.
            let mut identity = h.clone();
            for block in identity.chunks_exact_mut(V41_BLOCK * V41_BLOCK) {
                for j in 0..V41_BLOCK {
                    block[j * V41_BLOCK + (j ^ 1)] = 0.0;
                }
            }
            let (iw, is) = v41_quantize_matrix_gptq(format, &values, rows, cols, &identity);
            assert_eq!(w, iw);
            assert_eq!(s, is);
            let zero = vec![0.0; cols * V41_BLOCK];
            let (zw, zs) = v41_quantize_matrix_gptq(format, &values, rows, cols, &zero);
            assert_eq!(zw.len(), w.len());
            assert!(
                v41_dequantize_matrix(format, &zw, &zs, rows, cols)
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }
}
