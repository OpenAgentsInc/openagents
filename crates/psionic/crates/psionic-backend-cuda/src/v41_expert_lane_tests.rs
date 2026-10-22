//! The host expert lane against the device's host models and the CPU path.

use super::*;
use crate::v41_matvec::{V41MatvecFormat, v41_block_matvec_host};
use crate::v41_mix::{v41_round_bf16_all, v41_swiglu_host};
use psionic_models::deepseek_v41_cpu::quantization::{Quantization, quantize, round_bf16};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u64 << 24) as f32 * 2.0 - 1.0
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 56) as u8
    }
}

/// Random FP4 weights and UE8M0 scales near 2^-4, including exponent 0.
fn fp4_matrix(rng: &mut Rng, rows: usize, cols: usize) -> (Vec<u8>, Vec<u8>) {
    let weights = (0..rows * cols / 2).map(|_| rng.byte()).collect();
    let scales = (0..rows * cols / 32)
        .map(|i| {
            if i % 97 == 5 {
                0
            } else {
                120 + rng.byte() % 10
            }
        })
        .collect();
    (weights, scales)
}

fn activation(rng: &mut Rng, cols: usize, amplitude: f32) -> (Vec<u8>, Vec<u8>) {
    let x: Vec<f32> = (0..cols).map(|_| rng.unit() * amplitude).collect();
    let mut codes = vec![0; cols];
    let mut scales = vec![0; cols / 32];
    v41_act_quant_host(&x, &mut codes, &mut scales);
    (codes, scales)
}

fn isas() -> Vec<V41LaneIsa> {
    [V41LaneIsa::Scalar, V41LaneIsa::Avx2, V41LaneIsa::Avx512]
        .into_iter()
        .filter(|i| i.available())
        .collect()
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

/// Every path equals the device's host model bit for bit, for block counts
/// that leave every remainder against 8 and 16 lanes.
#[test]
fn fp4_matvec_equals_the_device_host_model() {
    let mut rng = Rng(0x5eed_1234_9876_0001);
    for blocks in [1, 2, 3, 7, 8, 9, 15, 16, 17, 31, 33, 72, 160] {
        let (rows, cols) = (37, blocks * 32);
        let (w, s) = fp4_matrix(&mut rng, rows, cols);
        let (codes, scales) = activation(&mut rng, cols, 3.0);
        let want = v41_block_matvec_host(
            V41MatvecFormat::Fp4E2M1Row32,
            &w,
            &s,
            rows,
            cols,
            &codes,
            &scales,
        );
        let x = V41DecodedActivation::new(&codes, &scales);
        for isa in isas() {
            let mut got = vec![0.0; rows];
            v41_fp4_matvec(isa, &w, &s, &x, 0..rows, &mut got);
            assert_eq!(bits(&got), bits(&want), "{isa:?} with {blocks} blocks");
            // A row range writes the same rows.
            let mut part = vec![0.0; 5];
            v41_fp4_matvec(isa, &w, &s, &x, 30..35, &mut part);
            assert_eq!(bits(&part), bits(&want[30..35]), "{isa:?} rows 30..35");
        }
    }
}

/// The quantizer equals the CPU path's `quantize(round_bf16(x), Fp8)`, the
/// rule the device kernel was checked against, including tiny, saturating,
/// and half-way values.
#[test]
fn act_quant_equals_the_cpu_path() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = Rng(77);
    for amplitude in [1e-9_f32, 1e-6, 1e-3, 0.5, 3.0, 448.0, 1e5] {
        let x: Vec<f32> = (0..256).map(|_| rng.unit() * amplitude).collect();
        let mut codes = vec![0; 256];
        let mut scales = vec![0; 8];
        v41_act_quant_host(&x, &mut codes, &mut scales);
        let want = quantize(&round_bf16(&x)?, Quantization::Fp8)?;
        assert_eq!(codes, want.codes(), "codes at amplitude {amplitude}");
        assert_eq!(scales, want.scales(), "scales at amplitude {amplitude}");
    }
    // A nonfinite block gets a NaN scale and NaN codes; its neighbours don't.
    let mut x = vec![1.0_f32; 64];
    x[40] = f32::INFINITY;
    let mut codes = vec![0; 64];
    let mut scales = vec![0; 2];
    v41_act_quant_host(&x, &mut codes, &mut scales);
    assert_eq!(scales[1], 0xff);
    assert!(codes[32..].iter().all(|c| *c == 0x7f));
    assert_ne!(scales[0], 0xff);
    Ok(())
}

/// The whole expert equals the device's host models chained as the stage's
/// MoE records them.
#[test]
fn expert_forward_equals_the_chained_host_models() {
    let mut rng = Rng(9);
    for (hidden, width) in [(128, 64), (256, 96), (5120 / 8, 2304 / 8)] {
        let dims = V41ExpertDims::fp4(hidden, width, 10.0);
        let (w1, s1) = fp4_matrix(&mut rng, width, hidden);
        let (w3, s3) = fp4_matrix(&mut rng, width, hidden);
        let (w2, s2) = fp4_matrix(&mut rng, hidden, width);
        let (codes, scales) = activation(&mut rng, hidden, 4.0);
        let route = 0.37_f32;
        let fp4 = V41MatvecFormat::Fp4E2M1Row32;
        let gate = v41_round_bf16_all(&v41_block_matvec_host(
            fp4, &w1, &s1, width, hidden, &codes, &scales,
        ));
        let up = v41_round_bf16_all(&v41_block_matvec_host(
            fp4, &w3, &s3, width, hidden, &codes, &scales,
        ));
        let h = v41_round_bf16_all(&v41_swiglu_host(&gate, &up, Some(&[route]), width, 10.0));
        let mut h_codes = vec![0; width];
        let mut h_scales = vec![0; width / 32];
        v41_act_quant_host(&h, &mut h_codes, &mut h_scales);
        let want = v41_round_bf16_all(&v41_block_matvec_host(
            fp4, &w2, &s2, hidden, width, &h_codes, &h_scales,
        ));
        let weights = V41ExpertWeights {
            w1: (&w1, &s1),
            w2: (&w2, &s2),
            w3: (&w3, &s3),
        };
        let x = V41DecodedActivation::new(&codes, &scales);
        for isa in isas() {
            let mut out = vec![0.0; hidden];
            let mut scratch = V41ExpertScratch::default();
            v41_expert_forward(isa, dims, &weights, &x, route, &mut scratch, &mut out);
            assert_eq!(bits(&out), bits(&want), "{isa:?} {hidden}x{width}");
        }
    }
}

/// The decoders match the device's bit patterns, NaN included.
#[test]
fn decoders_match_the_device() {
    assert_eq!(v41_e8m0(0).to_bits(), 0x0040_0000);
    assert_eq!(v41_e8m0(0xff).to_bits(), 0x7fff_ffff);
    assert_eq!(v41_e8m0(127), 1.0);
    assert_eq!(v41_e4m3(0x7f).to_bits(), 0x7fff_ffff);
    assert_eq!(v41_e4m3(0xff).to_bits(), 0xffff_ffff);
    assert_eq!(v41_e4m3(0x38), 1.0);
    assert_eq!(v41_e4m3(0x7e), 448.0);
    assert_eq!(v41_e4m3(0x01), 2.0_f32.powi(-9));
    assert_eq!(
        v41_bf16_round(f32::from_bits(0x7fff_ffff)).to_bits(),
        0x7fff_0000
    );
    assert_eq!(v41_bf16_round(1.0 + 2.0_f32.powi(-8)), 1.0);
    for byte in 0..=255_u8 {
        let v = v41_e4m3(byte);
        if !v.is_nan() {
            assert_eq!(
                v,
                psionic_models::deepseek_v41_weights::decode_f8_e4m3fn(byte)
            );
        }
    }
}

/// The `consumer-q` formats (#12465): every path equals the device's host
/// model bit for bit, on matrices the quantizer wrote.
#[test]
fn consumer_q_matvec_equals_the_device_host_model() {
    use crate::v41_quantize::v41_quantize_matrix;
    let mut rng = Rng(0xfeed_0042);
    for format in [V41MatvecFormat::Q2Row32, V41MatvecFormat::Q3Row32] {
        for blocks in [1, 2, 7, 8, 9, 16, 17, 33, 72] {
            let (rows, cols) = (21, blocks * 32);
            let values: Vec<f32> = (0..rows * cols).map(|_| rng.unit() * 0.03).collect();
            let (w, s) = v41_quantize_matrix(format, &values, rows, cols, None);
            let (codes, scales) = activation(&mut rng, cols, 3.0);
            let want = v41_block_matvec_host(format, &w, &s, rows, cols, &codes, &scales);
            let x = V41DecodedActivation::new(&codes, &scales);
            for isa in isas() {
                let mut got = vec![0.0; rows];
                v41_expert_matvec(isa, format, &w, &s, rows, &x, 0..rows, &mut got);
                assert_eq!(bits(&got), bits(&want), "{format:?} {isa:?} {blocks} blocks");
                let mut part = vec![0.0; 4];
                v41_expert_matvec(isa, format, &w, &s, rows, &x, 13..17, &mut part);
                assert_eq!(bits(&part), bits(&want[13..17]));
            }
        }
    }
}
