//! The CPU prefill GEMM on AVX-512 hosts (Sapphire Rapids and later, the
//! c3 machines the sealed NIP-ATT lane runs on).
//!
//! `HostMatrix::matmul_rows` multiplies a chunk of `n` activation rows by a
//! weight matrix whose rows are decoded from the GGUF. The portable path
//! dots each decoded row with every activation row, which is bound by
//! loads and horizontal sums. Here:
//!
//! - the chunk's activations are packed once, k-major, with tokens in the
//!   vector lanes (`packed[(block * columns + k) * TOKENS + lane]`, zero
//!   past `n`);
//! - a task owns [`ROWS`] weight rows and decodes them [`KC`] columns at a
//!   time into a row-major buffer (no transpose of the weights);
//! - a register-blocked kernel runs [`MR`] weight rows by four 16-lane
//!   token vectors (24 zmm accumulators): per `k`, four vector loads, six
//!   broadcasts and 24 FMAs.
//!
//! Every output is one sequential f32 FMA chain over `k`, independent of the
//! blocking, the thread count and the chunk's token count, so a repeat is
//! bitwise identical. It differs from the portable path only in summation
//! order (about 1e-6 relative).
//!
//! The kernel is chosen at run time (`avx512f`), so the binary stays a
//! generic x86-64 build. `PSIONIC_CPU_GEMM=portable` turns it off.

use rayon::prelude::*;

/// Weight rows per kernel call.
const MR: usize = 6;
/// Token lanes per kernel call (four 16-lane vectors).
const TOKENS: usize = 64;
/// Weight rows per task: the activation slice is reused across eight
/// kernel row groups while it is in L2.
const ROWS: usize = 48;
/// Decode slice: a task decodes `ROWS x KC` weights (192 KB, in L2) at a
/// time.
const KC: usize = 1024;
/// Kernel slice: a 64-token activation slice of `KS` columns is 32 KB, so
/// it stays in L1 while the task's row groups walk it.
const KS: usize = 128;

/// Whether the AVX-512 GEMM runs on this host.
pub(super) fn available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        if std::env::var("PSIONIC_CPU_GEMM").is_ok_and(|value| value == "portable") {
            return false;
        }
        #[cfg(target_arch = "x86_64")]
        {
            std::arch::is_x86_feature_detected!("avx512f")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    })
}

/// `input` (`n x columns`, row-major) times the `rows x columns` matrix
/// whose columns `start..start + len` of row `r` `decode(r, start, len,
/// buffer)` appends to `buffer`: returns `n x rows`. `start` and `len` are
/// multiples of `granule` (a quantization block), which divides `columns`.
/// Callers check [`available`] first.
pub(super) fn matmul_rows<F, E>(
    rows: usize,
    columns: usize,
    granule: usize,
    input: &[f32],
    n: usize,
    decode: F,
) -> Result<Vec<f32>, E>
where
    F: Fn(usize, usize, usize, &mut Vec<f32>) -> Result<(), E> + Sync,
    E: Send,
{
    debug_assert_eq!(input.len(), n * columns);
    let granule = granule.max(1);
    let kc_max = KC.div_ceil(granule) * granule;
    let blocks = n.div_ceil(TOKENS);
    let mut packed = vec![0.0_f32; blocks * columns * TOKENS];
    // A packing task copies `span` columns of a block's tokens: row
    // segments read in order, one contiguous `span x TOKENS` write.
    let span = [256, 64, 16, 1]
        .into_iter()
        .find(|span| columns % span == 0)
        .unwrap_or(1);
    packed
        .par_chunks_mut(span * TOKENS)
        .enumerate()
        .for_each(|(index, region)| {
            let (block, k0) = (index * span / columns, index * span % columns);
            let first = block * TOKENS;
            for lane in 0..TOKENS.min(n - first) {
                let row = &input[(first + lane) * columns + k0..][..span];
                for (k, value) in row.iter().enumerate() {
                    region[k * TOKENS + lane] = *value;
                }
            }
        });
    let width = blocks * TOKENS;
    let tasks = rows.div_ceil(ROWS);
    let per_task: Vec<Vec<f32>> = (0..tasks)
        .into_par_iter()
        .map_init(
            || (Vec::<f32>::new(), Vec::<f32>::new()),
            |(weights, decoded), task| -> Result<Vec<f32>, E> {
                let first = task * ROWS;
                let count = ROWS.min(rows - first);
                let mut out = vec![0.0_f32; count * width];
                let mut k0 = 0;
                while k0 < columns {
                    let kc = kc_max.min(columns - k0);
                    // padded row stride: rows 4 KB apart would alias in L1
                    let ldw = kc + 16;
                    weights.clear();
                    for row in first..first + count {
                        decoded.clear();
                        decode(row, k0, kc, decoded)?;
                        decoded.resize(ldw, 0.0);
                        weights.extend_from_slice(decoded);
                    }
                    let mut s = 0;
                    while s < kc {
                        let sl = KS.min(kc - s);
                        for block in 0..blocks {
                            let lanes = TOKENS.min(n - block * TOKENS);
                            let x = &packed[(block * columns + k0 + s) * TOKENS..];
                            let mut r = 0;
                            while r < count {
                                let group = MR.min(count - r);
                                // SAFETY: the kernel reads `group` weight
                                // rows of `sl` floats at stride `ldw` from
                                // row `r`, column `s` of `weights`; `sl`
                                // steps of `TOKENS` floats from `x` (a
                                // k-slice of one packed block); and reads
                                // and writes `group` rows of
                                // `lanes.div_ceil(16) * 16 <= TOKENS` floats
                                // of `out` at stride `width`.
                                unsafe {
                                    kernel(
                                        group,
                                        lanes.div_ceil(16),
                                        sl,
                                        ldw,
                                        weights.as_ptr().add(r * ldw + s),
                                        x.as_ptr(),
                                        out.as_mut_ptr().add(r * width + block * TOKENS),
                                        width,
                                    );
                                }
                                r += group;
                            }
                        }
                        s += sl;
                    }
                    k0 += kc;
                }
                Ok(out)
            },
        )
        .collect::<Result<_, E>>()?;
    let mut output = vec![0.0_f32; n * rows];
    // 16 tokens at a time: each task row's 16 values are one contiguous
    // read.
    output
        .par_chunks_mut(rows * 16)
        .enumerate()
        .for_each(|(chunk, tokens)| {
            let t0 = chunk * 16;
            let count = tokens.len() / rows;
            for (task, values) in per_task.iter().enumerate() {
                for (offset, row) in values.chunks_exact(width).enumerate() {
                    let r = task * ROWS + offset;
                    for (i, value) in row[t0..t0 + count].iter().enumerate() {
                        tokens[i * rows + r] = *value;
                    }
                }
            }
        });
    Ok(output)
}

/// Decodes whole Q4_K, Q6_K or Q8_0 blocks into `out`, value for value the
/// same f32 arithmetic as `psionic_backend_cpu::decode_quantized_row_into`
/// (bitwise identical; checked by a test), but into a slice, so the loops
/// vectorize. `None` for any other mode (the caller falls back).
pub(super) fn decode_blocks_into(
    mode: psionic_core::QuantizationMode,
    bytes: &[u8],
    out: &mut Vec<f32>,
) -> Option<()> {
    use psionic_core::QuantizationMode;
    let (elements, block_bytes) = mode.ggml_block_spec()?;
    if bytes.len() % block_bytes != 0 {
        return None;
    }
    let decode: fn(&[u8], &mut [f32]) = match mode {
        QuantizationMode::GgmlQ4K => decode_q4_k,
        QuantizationMode::GgmlQ6K => decode_q6_k,
        QuantizationMode::GgmlQ8_0 => decode_q8_0,
        _ => return None,
    };
    let start = out.len();
    out.resize(start + bytes.len() / block_bytes * elements, 0.0);
    for (block, values) in bytes
        .chunks_exact(block_bytes)
        .zip(out[start..].chunks_exact_mut(elements))
    {
        decode(block, values);
    }
    Some(())
}

fn f16_le(low: u8, high: u8) -> f32 {
    half::f16::from_bits(u16::from_le_bytes([low, high])).to_f32()
}

fn decode_q8_0(block: &[u8], out: &mut [f32]) {
    let scale = f16_le(block[0], block[1]);
    for (value, quant) in out.iter_mut().zip(&block[2..34]) {
        *value = f32::from(*quant as i8) * scale;
    }
}

fn q4_k_scale_min(index: usize, packed: &[u8]) -> (u8, u8) {
    if index < 4 {
        (packed[index] & 63, packed[index + 4] & 63)
    } else {
        (
            (packed[index + 4] & 0x0f) | ((packed[index - 4] >> 6) << 4),
            (packed[index + 4] >> 4) | ((packed[index] >> 6) << 4),
        )
    }
}

fn decode_q4_k(block: &[u8], out: &mut [f32]) {
    let scale = f16_le(block[0], block[1]);
    let minimum = f16_le(block[2], block[3]);
    let scales = &block[4..16];
    for (pair, (quants, values)) in block[16..144]
        .chunks_exact(32)
        .zip(out.chunks_exact_mut(64))
        .enumerate()
    {
        let (low_scale, low_min) = q4_k_scale_min(2 * pair, scales);
        let (high_scale, high_min) = q4_k_scale_min(2 * pair + 1, scales);
        let (low_scale, low_min) = (scale * f32::from(low_scale), minimum * f32::from(low_min));
        let (high_scale, high_min) = (scale * f32::from(high_scale), minimum * f32::from(high_min));
        let (low, high) = values.split_at_mut(32);
        for ((low, high), quant) in low.iter_mut().zip(high.iter_mut()).zip(quants) {
            *low = low_scale * f32::from(quant & 0x0f) - low_min;
            *high = high_scale * f32::from(quant >> 4) - high_min;
        }
    }
}

fn decode_q6_k(block: &[u8], out: &mut [f32]) {
    let (ql, qh, scales) = (&block[0..128], &block[128..192], &block[192..208]);
    let scale = f16_le(block[208], block[209]);
    for chunk in 0..2 {
        let ql = &ql[chunk * 64..(chunk + 1) * 64];
        let qh = &qh[chunk * 32..(chunk + 1) * 32];
        let sc = &scales[chunk * 8..(chunk + 1) * 8];
        let out = &mut out[chunk * 128..(chunk + 1) * 128];
        for l in 0..32 {
            let is = l / 16;
            let q1 = (((ql[l] & 0x0f) | ((qh[l] & 0x03) << 4)) as i8) - 32;
            let q2 = (((ql[l + 32] & 0x0f) | (((qh[l] >> 2) & 0x03) << 4)) as i8) - 32;
            let q3 = (((ql[l] >> 4) | (((qh[l] >> 4) & 0x03) << 4)) as i8) - 32;
            let q4 = (((ql[l + 32] >> 4) | (((qh[l] >> 6) & 0x03) << 4)) as i8) - 32;
            out[l] = scale * f32::from(sc[is] as i8) * f32::from(q1);
            out[l + 32] = scale * f32::from(sc[is + 2] as i8) * f32::from(q2);
            out[l + 64] = scale * f32::from(sc[is + 4] as i8) * f32::from(q3);
            out[l + 96] = scale * f32::from(sc[is + 6] as i8) * f32::from(q4);
        }
    }
}

/// The register-blocked kernel; only reached when [`available`] is true.
unsafe fn kernel(
    rows: usize,
    vectors: usize,
    kc: usize,
    ldw: usize,
    w: *const f32,
    x: *const f32,
    c: *mut f32,
    ldc: usize,
) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: `available()` checked avx512f; the caller bounds the pointers.
    unsafe {
        avx512::dispatch(rows, vectors, kc, ldw, w, x, c, ldc);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (rows, vectors, kc, ldw, w, x, c, ldc);
        unreachable!("the AVX-512 GEMM is x86_64 only");
    }
}

#[cfg(target_arch = "x86_64")]
mod avx512 {
    use super::TOKENS;
    use std::arch::x86_64::{
        __m512, _mm512_fmadd_ps, _mm512_loadu_ps, _mm512_set1_ps, _mm512_setzero_ps,
        _mm512_storeu_ps,
    };

    /// Runs the kernel for `rows` (`1..=MR`) weight rows and `vectors`
    /// (`1..=4`) 16-token vectors.
    #[target_feature(enable = "avx512f")]
    pub(super) unsafe fn dispatch(
        rows: usize,
        vectors: usize,
        kc: usize,
        ldw: usize,
        w: *const f32,
        x: *const f32,
        c: *mut f32,
        ldc: usize,
    ) {
        macro_rules! by_vectors {
            ($r:literal) => {
                match vectors {
                    1 => kernel::<$r, 1>(kc, ldw, w, x, c, ldc),
                    2 => kernel::<$r, 2>(kc, ldw, w, x, c, ldc),
                    3 => kernel::<$r, 3>(kc, ldw, w, x, c, ldc),
                    _ => kernel::<$r, 4>(kc, ldw, w, x, c, ldc),
                }
            };
        }
        unsafe {
            match rows {
                1 => by_vectors!(1),
                2 => by_vectors!(2),
                3 => by_vectors!(3),
                4 => by_vectors!(4),
                5 => by_vectors!(5),
                _ => by_vectors!(6),
            }
        }
    }

    /// `c[i * ldc + lane] += sum_{k < kc} w[i * ldw + k] * x[k * TOKENS +
    /// lane]` for `i < R`, `lane < 16 V`: one FMA chain per output, in `k`
    /// order.
    #[target_feature(enable = "avx512f")]
    #[inline]
    unsafe fn kernel<const R: usize, const V: usize>(
        kc: usize,
        ldw: usize,
        w: *const f32,
        x: *const f32,
        c: *mut f32,
        ldc: usize,
    ) {
        unsafe {
            let mut acc: [[__m512; V]; R] = [[_mm512_setzero_ps(); V]; R];
            for i in 0..R {
                for v in 0..V {
                    acc[i][v] = _mm512_loadu_ps(c.add(i * ldc + v * 16));
                }
            }
            for k in 0..kc {
                let mut xv: [__m512; V] = [_mm512_setzero_ps(); V];
                for v in 0..V {
                    xv[v] = _mm512_loadu_ps(x.add(k * TOKENS + v * 16));
                }
                for i in 0..R {
                    let a = _mm512_set1_ps(*w.add(i * ldw + k));
                    for v in 0..V {
                        acc[i][v] = _mm512_fmadd_ps(a, xv[v], acc[i][v]);
                    }
                }
            }
            for i in 0..R {
                for v in 0..V {
                    _mm512_storeu_ps(c.add(i * ldc + v * 16), acc[i][v]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(
        rows: usize,
        columns: usize,
        weights: &[f32],
        input: &[f32],
        n: usize,
    ) -> Vec<f64> {
        let mut out = vec![0.0_f64; n * rows];
        for t in 0..n {
            for r in 0..rows {
                out[t * rows + r] = (0..columns)
                    .map(|k| {
                        f64::from(weights[r * columns + k]) * f64::from(input[t * columns + k])
                    })
                    .sum();
            }
        }
        out
    }

    /// The slice decoders against the backend's row decoder, bitwise.
    #[test]
    fn block_decoders_match_the_backend_bitwise() {
        use psionic_core::QuantizationMode;
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut byte = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 32) as u8
        };
        for mode in [
            QuantizationMode::GgmlQ4K,
            QuantizationMode::GgmlQ6K,
            QuantizationMode::GgmlQ8_0,
        ] {
            let (_, block_bytes) = mode.ggml_block_spec().unwrap();
            let mut bytes: Vec<u8> = (0..block_bytes * 9).map(|_| byte()).collect();
            // finite f16 scales (exponent below 0x1f)
            for block in bytes.chunks_exact_mut(block_bytes) {
                let at: &[usize] = match mode {
                    QuantizationMode::GgmlQ4K => &[1, 3],
                    QuantizationMode::GgmlQ6K => &[209],
                    _ => &[1],
                };
                for &index in at {
                    block[index] &= 0xbb;
                }
            }
            let mut want = Vec::new();
            psionic_backend_cpu::decode_quantized_row_into(mode, &bytes, &mut want).unwrap();
            let mut got = vec![7.0];
            decode_blocks_into(mode, &bytes, &mut got).unwrap();
            assert_eq!(got[0], 7.0);
            let want: Vec<u32> = want.iter().map(|value| value.to_bits()).collect();
            let got: Vec<u32> = got[1..].iter().map(|value| value.to_bits()).collect();
            assert_eq!(got, want, "{mode:?}");
        }
    }

    /// The packed kernel against an f64 reference over panel, token and
    /// `k`-slice tails, and a repeat against itself (bitwise).
    #[test]
    fn packed_gemm_matches_reference_and_repeats() {
        if !available() {
            eprintln!("skipped: no avx512f on this host");
            return;
        }
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
        };
        for &(rows, columns, n) in &[
            (1, 16, 1),
            (7, 2048, 64),
            (49, 1028, 70),
            (5, 264, 3),
            (31, 40, 5),
            (40, 520, 2),
            (32, 1024, 12),
            (70, 1100, 13),
            (96, 2304, 27),
            (33, 4096, 1),
        ] {
            let weights: Vec<f32> = (0..rows * columns).map(|_| next()).collect();
            let input: Vec<f32> = (0..n * columns).map(|_| next()).collect();
            let decode = |row: usize, start: usize, len: usize, buffer: &mut Vec<f32>| {
                buffer.extend_from_slice(
                    &weights[row * columns + start..row * columns + start + len],
                );
                Ok::<(), ()>(())
            };
            let got = matmul_rows(rows, columns, 4, &input, n, decode).unwrap();
            let again = matmul_rows(rows, columns, 4, &input, n, decode).unwrap();
            assert_eq!(got, again, "repeat differs at {rows}x{columns} n={n}");
            let want = reference(rows, columns, &weights, &input, n);
            let scale = (columns as f64).sqrt();
            for (index, (got, want)) in got.iter().zip(&want).enumerate() {
                let error = (f64::from(*got) - want).abs() / scale;
                assert!(
                    error < 2e-6,
                    "{rows}x{columns} n={n} output {index}: {got} vs {want}"
                );
            }
        }
    }
}
