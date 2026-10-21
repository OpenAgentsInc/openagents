//! Device arithmetic of the eight-B200 (SM100) TP8 oracle, measured for #12404.
//!
//! The oracle's sparse attention kernel (TileLang, `inference/kernel.py`) uses
//! the legacy `mma.sync` BF16 instruction, CUDA's non-fast-math `expf`, and a
//! butterfly sum across warps. Probes on a B200 with the oracle's driver
//! (580.178.04), CUDA 12.9, and TileLang 0.1.12 measured each piece:
//!
//! - `mma.sync.m16n8k16` BF16 with an F32 accumulator matched this model in
//!   10,240,000 of 10,240,000 random outputs: the 16 exact products and the
//!   accumulator are aligned to the largest exponent, where a product's
//!   exponent is the sum of its factors' exponents, truncated toward zero 25
//!   bits below that exponent, summed exactly, and rounded toward zero.
//! - `MUFU.EX2` on `[0, 1)` reads the input as a fixed-point fraction with 23
//!   bits and returns one of 2^23 values. The table below holds all of them;
//!   it matched 2,000,000 of 2,000,000 random inputs.
//! - `expf` compiles to the sequence in [`cuda_expf`] (SASS for `sm_100`).
//!
//! The probes and raw results are kept under
//! `gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/w12404-capture/probe/`.
//! These functions reproduce that hardware's results; they are not general
//! IEEE arithmetic.

use super::CpuError;
use super::quantization::bf16;
use std::sync::OnceLock;

/// Decompose a finite nonzero F32 as `mantissa * 2^(exponent - 23)`.
fn parts(x: f32) -> (u64, i32) {
    let bits = x.to_bits();
    let field = ((bits >> 23) & 0xff) as i32;
    let frac = (bits & 0x7f_ffff) as u64;
    if field == 0 {
        (frac, -126)
    } else {
        (frac | 1 << 23, field - 127)
    }
}

/// Round an exact `units * 2^scale` toward zero to F32.
fn round_toward_zero(units: i64, scale: i32) -> f32 {
    if units == 0 {
        return 0.0;
    }
    let exact = units as f64 * 2f64.powi(scale);
    let rounded = exact as f32;
    if (rounded as f64).abs() > exact.abs() {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    }
}

/// One fused tensor-core sum: `c + sum(a[i] * b[i])` for up to 32 products of
/// BF16 or FP8 values held in F32. Infinite `c` passes through, as in the
/// kernel's masked score slots.
pub fn fused_dot(c: f32, a: &[f32], b: &[f32]) -> f32 {
    debug_assert!(a.len() <= 32 && a.len() == b.len());
    if !c.is_finite() {
        return c;
    }
    let mut emax = i32::MIN;
    let mut terms = [(0u64, 0i32, false); 32];
    let mut n = 0;
    for (x, y) in a.iter().zip(b) {
        if *x == 0.0 || *y == 0.0 {
            continue;
        }
        let (mx, ex) = parts(*x);
        let (my, ey) = parts(*y);
        // Exact product mx * my * 2^(ex + ey - 46); the hardware aligns on ex + ey.
        terms[n] = (mx * my, ex + ey, (*x < 0.0) != (*y < 0.0));
        emax = emax.max(ex + ey);
        n += 1;
    }
    let cp = (c != 0.0).then(|| parts(c));
    if let Some((_, ec)) = cp {
        emax = emax.max(ec);
    }
    if emax == i32::MIN {
        return c;
    }
    // Units of 2^(emax - 25); shifts below them truncate toward zero.
    let mut sum = 0i64;
    for (m, e, negative) in &terms[..n] {
        let shift = e - emax - 21;
        let units = if shift >= 0 {
            (*m << shift) as i64
        } else if -shift < 64 {
            (*m >> -shift) as i64
        } else {
            0
        };
        sum += if *negative { -units } else { units };
    }
    if let Some((mc, ec)) = cp {
        let shift = ec - emax + 2;
        let units = if shift >= 0 {
            (mc << shift) as i64
        } else if -shift < 64 {
            (mc >> -shift) as i64
        } else {
            0
        };
        sum += if c < 0.0 { -units } else { units };
    }
    round_toward_zero(sum, emax - 25)
}

/// `mma.sync` BF16 over K in order, 16 products per instruction.
pub fn mma_bf16_dot(c: f32, a: &[f32], b: &[f32]) -> f32 {
    a.chunks(16)
        .zip(b.chunks(16))
        .fold(c, |acc, (x, y)| fused_dot(acc, x, y))
}

const EX2_TABLE_SHA256: &str = "b2a42c4a626469c986e33e43f16f41bde9d84de94347cd9ceaa1bd68d36bcdf0";

fn ex2_table() -> Result<&'static [u32], CpuError> {
    static TABLE: OnceLock<Result<Vec<u32>, &'static str>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            let mut packed = Vec::new();
            flate2::read::GzDecoder::new(
                &include_bytes!("../testdata/deepseek_v41_cpu/sm100_ex2.bin.gz")[..],
            )
            .read_to_end(&mut packed)
            .map_err(|_| "SM100 EX2 table does not decompress")?;
            if packed.len() != 1 << 21 {
                return Err("SM100 EX2 table length");
            }
            // Two-bit increments from 1.0, four per byte, low bits first.
            let mut table = Vec::with_capacity(1 << 23);
            let mut value = 1f32.to_bits();
            for byte in packed {
                for i in 0..4 {
                    value += u32::from(byte >> (2 * i) & 3);
                    table.push(value);
                }
            }
            let mut hash = Sha256::new();
            for v in &table {
                hash.update(v.to_le_bytes());
            }
            if hex::encode(hash.finalize()) != EX2_TABLE_SHA256 {
                return Err("SM100 EX2 table hash");
            }
            Ok(table)
        })
        .as_deref()
        .map_err(|e| CpuError::Invalid(e))
}

/// `MUFU.EX2`: exact on `[0, 1)`, the only range `expf` passes it for
/// results above the F32 normal range; elsewhere `2^x` rounded to nearest.
pub fn mufu_ex2(x: f32) -> Result<f32, CpuError> {
    if (0.0..1.0).contains(&x) {
        let index = (x as f64 * f64::from(1u32 << 23)) as usize;
        Ok(f32::from_bits(ex2_table()?[index]))
    } else {
        Ok((x as f64).exp2() as f32)
    }
}

/// CUDA 12.9 `expf` without fast math, from its `sm_100` SASS.
pub fn cuda_expf(x: f32) -> Result<f32, CpuError> {
    let t = x.mul_add(f32::from_bits(0x3bbb_989d), 0.5);
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    // FFMA.RM: every value here is an integer below 2^24, so floor is exact.
    let t = (t as f64 * 252.0 + 12_582_913.0).floor() as f32;
    let r = t - 12_583_039.0;
    let f = x.mul_add(f32::from_bits(0x3fb8_aa3b), -r);
    let f = x.mul_add(f32::from_bits(0x32a5_7060), f);
    let scale = f32::from_bits(t.to_bits().wrapping_shl(23));
    Ok(scale * mufu_ex2(f)?)
}

/// TileLang's `AllReduce<SumOp, 256, 32>` then `AllReduce<SumOp, 4, 1>` over
/// one 64-key block: each thread adds its two adjacent keys, then the warps
/// pair by XOR 4, 2, 1, then the quad lanes by XOR 2, 1.
fn block_sum(p: &[f32; 64]) -> f32 {
    let mut t = [[0.0f32; 4]; 8];
    for (w, lanes) in t.iter_mut().enumerate() {
        for (q, lane) in lanes.iter_mut().enumerate() {
            *lane = (0.0 + p[8 * w + 2 * q]) + p[8 * w + 2 * q + 1];
        }
    }
    let quad: Vec<f32> = (0..4)
        .map(|q| {
            ((t[0][q] + t[4][q]) + (t[2][q] + t[6][q]))
                + ((t[1][q] + t[5][q]) + (t[3][q] + t[7][q]))
        })
        .collect();
    (quad[0] + quad[2]) + (quad[1] + quad[3])
}

/// The oracle's `sparse_attn` kernel for one query row: BF16 query heads and
/// KV rows, `-1` for masked index slots, F32 sinks. Index slots run in
/// blocks of 64 exactly as listed.
pub fn sparse_attention(
    query: &[f32],
    heads: usize,
    width: usize,
    kv: &[f32],
    indices: &[i32],
    sinks: &[f32],
    scale: f32,
) -> Result<Vec<f32>, CpuError> {
    if width == 0 || !width.is_multiple_of(16) || query.len() != heads * width {
        return Err(CpuError::Invalid("SM100 attention dimensions"));
    }
    let rows = kv.len() / width;
    if indices
        .iter()
        .any(|i| *i < -1 || (*i >= 0 && *i as usize >= rows))
    {
        return Err(CpuError::Invalid("SM100 attention index"));
    }
    let mut out = vec![0.0; query.len()];
    for h in 0..heads {
        let q = &query[h * width..(h + 1) * width];
        let mut max = -1e30_f32;
        let mut sum = 0.0f32;
        let mut acc = vec![0.0f32; width];
        for chunk in indices.chunks(64) {
            let mut ids = [-1i32; 64];
            ids[..chunk.len()].copy_from_slice(chunk);
            let mut scores = [f32::NEG_INFINITY; 64];
            for (slot, id) in ids.iter().enumerate() {
                if *id >= 0 {
                    let key = &kv[*id as usize * width..(*id as usize + 1) * width];
                    scores[slot] = mma_bf16_dot(0.0, q, key) * scale;
                }
            }
            let previous = max;
            max = scores.iter().fold(max, |m, s| m.max(*s));
            let rescale = cuda_expf(previous - max)?;
            let mut weights = [0.0f32; 64];
            for (w, s) in weights.iter_mut().zip(&scores) {
                *w = cuda_expf(*s - max)?;
            }
            sum = sum.mul_add(rescale, block_sum(&weights));
            let rounded: Vec<f32> = weights.iter().map(|w| bf16(*w)).collect();
            for x in &mut acc {
                *x *= rescale;
            }
            let mut values = [0.0f32; 64];
            for (d, x) in acc.iter_mut().enumerate() {
                for (slot, id) in ids.iter().enumerate() {
                    values[slot] = if *id >= 0 {
                        kv[*id as usize * width + d]
                    } else {
                        0.0
                    };
                }
                *x = mma_bf16_dot(*x, &rounded, &values);
            }
        }
        sum += cuda_expf(sinks[h] - max)?;
        for (o, x) in out[h * width..(h + 1) * width].iter_mut().zip(&acc) {
            *o = bf16(*x / sum);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fused_dot_matches_handcrafted_b200_results() {
        let one = 1.0f32;
        // Terms 25 bits below the leading exponent survive; 26 bits below are dropped.
        assert_eq!(
            fused_dot(one, &[2f32.powi(-25); 4], &[1.0; 4]),
            1.0 + 2f32.powi(-23)
        );
        assert_eq!(fused_dot(one, &[2f32.powi(-26); 8], &[1.0; 8]), 1.0);
        // Truncation is toward zero, also for negative terms.
        assert_eq!(
            fused_dot(one, &[-(2f32.powi(-25))], &[1.0]),
            1.0 - 2f32.powi(-24)
        );
        assert_eq!(fused_dot(one, &[-(2f32.powi(-26))], &[1.0]), 1.0);
        // Alignment uses the largest term before cancellation.
        assert_eq!(
            fused_dot(one, &[1024.0, -1024.0, 2f32.powi(-20)], &[1.0; 3]),
            1.0
        );
        assert_eq!(
            fused_dot(2f32.powi(30), &[-(2f32.powi(30)), 1.0], &[1.0; 2]),
            0.0
        );
        // The result rounds toward zero.
        assert_eq!(
            fused_dot(0.0, &[1.0, 1.5 * 2f32.powi(-23)], &[1.0; 2]),
            1.0 + 2f32.powi(-23)
        );
        assert_eq!(
            fused_dot(f32::NEG_INFINITY, &[1.0], &[1.0]),
            f32::NEG_INFINITY
        );
    }

    #[test]
    fn fused_dot_aligns_products_on_factor_exponents() {
        // From the B200 random probe: 1.5 * 1.5 = 2.25 still aligns on 2^0, so a
        // 2^-25 term survives where alignment on 2^1 would drop it.
        let mut a = [2f32.powi(-25); 9];
        a[0] = 1.5;
        let mut b = [1.0f32; 9];
        b[0] = 1.5;
        assert_eq!(fused_dot(0.0, &a, &b), 2.25 + 2f32.powi(-22));
    }

    #[test]
    fn ex2_table_and_expf_follow_the_device_sequence() -> Result<(), CpuError> {
        assert_eq!(mufu_ex2(0.0)?, 1.0);
        assert_eq!(mufu_ex2(0.5)?.to_bits(), 0x3fb5_04f3);
        // Inputs share a value per 2^-23 step.
        assert_eq!(mufu_ex2(0.5 + 2f32.powi(-24))?, mufu_ex2(0.5)?);
        assert_eq!(cuda_expf(0.0)?, 1.0);
        assert_eq!(cuda_expf(f32::NEG_INFINITY)?, 0.0);
        assert_eq!(cuda_expf(-1e30)?, 0.0);
        let x = -1.0f32;
        assert!((cuda_expf(x)? - x.exp()).abs() <= 2.0 * f32::EPSILON * x.exp());
        Ok(())
    }

    fn bf16_hex(text: &str) -> Vec<f32> {
        hex::decode(text)
            .expect("fixture hex")
            .chunks_exact(2)
            .map(|b| half::bf16::from_le_bytes([b[0], b[1]]).to_f32())
            .collect()
    }

    /// Original-weight B200 capture: layer 0, chat prompt position 2, TP8 rank 6.
    /// The earlier sequential F32 model differs from the oracle in one BF16 value.
    #[test]
    fn window_attention_reproduces_the_b200_oracle_bit_for_bit() -> Result<(), CpuError> {
        let f: serde_json::Value = serde_json::from_str(include_str!(
            "../testdata/deepseek_v41_cpu/sm100_attention.json"
        ))
        .expect("fixture json");
        let text = |k: &str| f[k].as_str().expect("fixture field").to_string();
        let query = bf16_hex(&text("query_bf16"));
        let kv = bf16_hex(&text("kv_bf16"));
        let expected = bf16_hex(&text("expected_bf16"));
        let indices: Vec<i32> = serde_json::from_value(f["indices"].clone()).expect("indices");
        let sinks: Vec<f32> = serde_json::from_value(f["sinks_f32"].clone()).expect("sinks");
        let scale = (512f64).powf(-0.5) as f32;
        let out = sparse_attention(&query, 8, 512, &kv, &indices, &sinks, scale)?;
        assert_eq!(out, expected);
        let old = super::super::quantization::sparse_attention_bf16_tp8_sequential(
            &query, 8, 512, &kv, &indices, &sinks, scale,
        )?;
        assert_eq!(old.iter().zip(&expected).filter(|(a, b)| a != b).count(), 1);
        Ok(())
    }

    #[test]
    fn block_sum_uses_the_warp_butterfly_order() {
        let mut p = [0.0f32; 64];
        p[0] = 1.0;
        p[32] = 2f32.powi(-24);
        p[33] = 2f32.powi(-24);
        // Keys 32 and 33 share a thread, so their sum survives next to 1.0.
        assert_eq!(block_sum(&p), 1.0 + 2f32.powi(-23));
    }
}
