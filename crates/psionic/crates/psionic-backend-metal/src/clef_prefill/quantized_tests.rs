//! Packed projection weights checked against the independent CPU decoder.

use super::*;
use crate::clef_gemm::ClefMetalQ4Gemm;
use psionic_core::QuantizationMode;

#[test]
fn clef_packed_weight_lengths_reject_partial_blocks_and_overflow() {
    assert_eq!(ClefMetalWeightFormat::Q8_0.byte_len(64), Some(68));
    assert_eq!(ClefMetalWeightFormat::Q4K.byte_len(512), Some(288));
    assert_eq!(ClefMetalWeightFormat::Q6K.byte_len(512), Some(420));
    for format in [
        ClefMetalWeightFormat::Q8_0,
        ClefMetalWeightFormat::Q4K,
        ClefMetalWeightFormat::Q6K,
    ] {
        assert_eq!(format.byte_len(1), None);
        assert_eq!(format.byte_len(0), Some(0));
    }
    assert_eq!(ClefMetalWeightFormat::F32.byte_len(usize::MAX), None);
    assert_eq!(
        ClefMetalWeightFormat::Q8_0.byte_len(usize::MAX / 32 * 32),
        None
    );
}

fn weights(format: ClefMetalWeightFormat, elements: usize) -> Vec<u8> {
    let (values, stride) = match format {
        ClefMetalWeightFormat::Q8_0 => (32, 34),
        ClefMetalWeightFormat::Q4K => (256, 144),
        ClefMetalWeightFormat::Q6K => (256, 210),
        ClefMetalWeightFormat::F32 => panic!("packed test only"),
    };
    let mut bytes = vec![0; elements / values * stride];
    for (block, bytes) in bytes.chunks_mut(stride).enumerate() {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = ((index * 73 + block * 19 + index / 7) % 256) as u8;
        }
        // Dyadic scales keep these decoded weights exactly representable in
        // f16, so decoding can be checked without a rounding tolerance.
        let scale = if block % 3 == 0 { 0x0000u16 } else { 0x1c00u16 };
        match format {
            ClefMetalWeightFormat::Q8_0 => bytes[..2].copy_from_slice(&scale.to_le_bytes()),
            ClefMetalWeightFormat::Q4K => {
                bytes[..2].copy_from_slice(&scale.to_le_bytes());
                bytes[2..4].copy_from_slice(&0x1c00u16.to_le_bytes());
            }
            ClefMetalWeightFormat::Q6K => {
                bytes[208..210].copy_from_slice(&scale.to_le_bytes());
                for (index, byte) in bytes[192..208].iter_mut().enumerate() {
                    *byte = ((index as i8 + block as i8) % 16 - 8) as u8;
                }
            }
            ClefMetalWeightFormat::F32 => unreachable!(),
        }
    }
    bytes
}

fn half_value(bytes: &[u8]) -> f32 {
    let bits = u16::from_le_bytes([bytes[0], bytes[1]]);
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 31);
    let mantissa = f32::from(bits & 1023);
    if exponent == 0 {
        sign * mantissa * 2.0f32.powi(-24)
    } else {
        sign * (1.0 + mantissa / 1024.0) * 2.0f32.powi(exponent - 15)
    }
}

#[test]
#[ignore = "requires a Metal device; synthetic buffers under 128 KiB"]
fn clef_packed_projection_formats_match_cpu_decode_and_products() {
    let metal = ClefMetal::new().expect("Metal");
    let n = 17;
    let m = 7;
    let k = 512;
    for (format, mode) in [
        (ClefMetalWeightFormat::Q8_0, QuantizationMode::GgmlQ8_0),
        (ClefMetalWeightFormat::Q4K, QuantizationMode::GgmlQ4K),
        (ClefMetalWeightFormat::Q6K, QuantizationMode::GgmlQ6K),
    ] {
        let packed = weights(format, m * k);
        let mut reference = Vec::new();
        psionic_backend_cpu::decode_quantized_row_into(mode, &packed, &mut reference)
            .expect("independent CPU decoder");
        assert_eq!(reference.len(), m * k);
        let source = metal.buffer_with_bytes(&packed);
        let guarded = metal.buffer_with_bytes(&vec![0x44; m * k * 2 + 64]);
        let decoded = guarded.view(32, m * k * 2).expect("guarded view");
        let input: Vec<f32> = (0..n * k)
            .map(|i| ((i * 7 + i / k * 3) % 19) as f32 / 64.0 - 0.125)
            .collect();
        let input_f32 = metal.buffer_f32(&input);
        let input_f16 = metal.buffer(n * k * 2);
        let output = metal.buffer_f32(&vec![123.0; 32 + n * m + 64 * m]);
        let batch = metal.batch();
        batch
            .dequantize_to_f16(format, &source, &decoded, m * k)
            .expect("decode");
        batch
            .dequantize_to_f16(ClefMetalWeightFormat::F32, &input_f32, &input_f16, n * k)
            .expect("input conversion");
        batch
            .gemm(&input_f16, 0, &decoded, &output, 32, n, m, k, false)
            .expect("projection");
        batch.commit_wait().expect("completion");
        let actual: Vec<f32> = decoded
            .read_bytes(0, m * k * 2)
            .expect("decoded bytes")
            .chunks_exact(2)
            .map(half_value)
            .collect();
        assert_eq!(actual, reference, "{format:?} decoded values");
        assert_eq!(guarded.read_bytes(0, 32).unwrap(), vec![0x44; 32]);
        assert_eq!(
            guarded.read_bytes(32 + m * k * 2, 32).unwrap(),
            vec![0x44; 32]
        );
        assert_eq!(source.read_bytes(0, packed.len()).unwrap(), packed);
        let product = output.read_f32(32, n * m).expect("products");
        for row in 0..n {
            for column in 0..m {
                let expected: f64 = (0..k)
                    .map(|inner| {
                        f64::from(input[row * k + inner]) * f64::from(reference[column * k + inner])
                    })
                    .sum();
                assert!(
                    (f64::from(product[row * m + column]) - expected).abs() <= 2e-5,
                    "{format:?} row {row} column {column}"
                );
            }
        }
        assert!(
            output
                .read_f32(0, 32)
                .unwrap()
                .iter()
                .all(|value| *value == 123.0)
        );
        assert!(
            output
                .read_f32(32 + n * m, 64 * m)
                .unwrap()
                .iter()
                .all(|value| *value == 123.0)
        );
    }
}

#[test]
#[ignore = "requires a Metal device; synthetic buffers under 1 MiB"]
fn clef_q4k_direct_gemm_matches_decode_then_gemm() {
    let metal = ClefMetal::new().expect("Metal");
    // Partial token and weight-row tiles on both edges, two super-blocks of k;
    // 300 x 200 spans more than one weight-stationary tile on each axis.
    for kernel in ClefMetalQ4Gemm::ALL {
        for (n, m, k) in [
            (130usize, 70usize, 512usize),
            (64, 64, 256),
            (1, 3, 768),
            (300, 200, 1280),
        ] {
            let packed = weights(ClefMetalWeightFormat::Q4K, m * k);
            let source = metal.buffer_with_bytes(&packed);
            let decoded = metal.buffer(m * k * 2);
            let input: Vec<f32> = (0..n * k)
                .map(|i| ((i * 7 + i / k * 3) % 19) as f32 / 64.0 - 0.125)
                .collect();
            let input_f32 = metal.buffer_f32(&input);
            let input_f16 = metal.buffer(n * k * 2);
            let residual: Vec<f32> = (0..n * m).map(|i| (i % 5) as f32 / 4.0 - 0.5).collect();
            let reference = metal.buffer_f32(&residual);
            let direct = metal.buffer_f32(&residual);
            let fresh = metal.buffer_f32(&vec![9.0; n * m]);
            let batch = metal.batch();
            batch
                .dequantize_to_f16(ClefMetalWeightFormat::F32, &input_f32, &input_f16, n * k)
                .expect("input conversion");
            batch
                .dequantize_to_f16(ClefMetalWeightFormat::Q4K, &source, &decoded, m * k)
                .expect("decode");
            batch
                .gemm(&input_f16, 0, &decoded, &reference, 0, n, m, k, true)
                .expect("decode-then-gemm");
            batch
                .gemm_q4k_with(kernel, &input_f16, 0, &source, &direct, 0, n, m, k, true)
                .expect("direct q4k gemm, accumulate");
            batch
                .gemm_q4k_with(kernel, &input_f16, 0, &source, &fresh, 0, n, m, k, false)
                .expect("direct q4k gemm");
            batch.commit_wait().expect("completion");
            let reference = reference.read_f32(0, n * m).unwrap();
            let direct = direct.read_f32(0, n * m).unwrap();
            let fresh = fresh.read_f32(0, n * m).unwrap();
            for i in 0..n * m {
                let tolerance = 1e-4 * reference[i].abs().max(1.0);
                assert!(
                    (direct[i] - reference[i]).abs() <= tolerance,
                    "{kernel:?} n={n} m={m} k={k} element {i}: direct {} reference {}",
                    direct[i],
                    reference[i]
                );
                let product = reference[i] - residual[i];
                assert!(
                    (fresh[i] - product).abs() <= 1e-4 * product.abs().max(1.0),
                    "{kernel:?} n={n} m={m} k={k} element {i}: fresh {} product {product}",
                    fresh[i]
                );
            }
        }
    }
}

#[test]
#[ignore = "timing on a Metal device; about 1 GiB of synthetic buffers"]
fn clef_q4k_direct_gemm_timing_against_f16() {
    // The f16 reference uses the weight-shape tiles the benchmarked 27B
    // settings select (PSIONIC_CLEF_METAL_GEMM=weight-shapes).
    let metal = ClefMetal::new_with_options(
        crate::clef_attention::ClefMetalAttention::default(),
        crate::clef_gemm::ClefMetalGemm::WeightShapes,
    )
    .expect("Metal");
    // CLEF_Q4_REPS sets the timed repeats (8 by default; ~100 sustains the GPU
    // for over a second, where f16 throughput has been seen to fall).
    let reps: usize = std::env::var("CLEF_Q4_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    for (m, k) in [
        (12288usize, 4096usize),
        (4096, 12288),
        (17408, 5120),
        (5120, 17408),
    ] {
        let packed = weights(ClefMetalWeightFormat::Q4K, m * k);
        let source = metal.buffer_with_bytes(&packed);
        let decoded = metal.buffer(m * k * 2);
        let batch = metal.batch();
        batch
            .dequantize_to_f16(ClefMetalWeightFormat::Q4K, &source, &decoded, m * k)
            .unwrap();
        batch.commit_wait().unwrap();
        for n in [1024usize, 2048, 4096] {
            let input = metal.buffer(n * k * 2);
            let out = metal.buffer(n * m * 4);
            let time = |kernel: Option<ClefMetalQ4Gemm>| {
                let encode = |batch: &ClefMetalBatch<'_>| match kernel {
                    Some(kernel) => batch
                        .gemm_q4k_with(kernel, &input, 0, &source, &out, 0, n, m, k, false)
                        .unwrap(),
                    None => batch
                        .gemm(&input, 0, &decoded, &out, 0, n, m, k, false)
                        .unwrap(),
                };
                // one warm-up, then `reps` timed in one command buffer
                let warm = metal.batch();
                encode(&warm);
                warm.commit_wait().unwrap();
                let started = std::time::Instant::now();
                let batch = metal.batch();
                for _ in 0..reps {
                    encode(&batch);
                }
                batch.commit_wait().unwrap();
                started.elapsed().as_secs_f64() * 1000.0 / reps as f64
            };
            let tflops = |ms: f64| 2.0 * (n * m * k) as f64 / (ms / 1000.0) / 1e12;
            let f16 = time(None);
            let mut line = format!("m={m} k={k} n={n}: f16 {f16:.2} ms ({:.1} TF)", tflops(f16));
            for kernel in ClefMetalQ4Gemm::ALL {
                let q4 = time(Some(kernel));
                line += &format!(
                    ", {kernel:?} {q4:.2} ms ({:.1} TF, x{:.2})",
                    tflops(q4),
                    f16 / q4
                );
            }
            eprintln!("{line}");
        }
    }
}

/// Sustained alternating windows for macmon joins (#11481): one JSON line per
/// window (`kernel`, `start`, `seconds`, `tflops`). CLEF_ALT="f16,Direct,Stationary"
/// picks kernels, CLEF_ALT_WINDOW seconds per window, CLEF_ALT_CYCLES cycles
/// (order reversed on odd cycles). Eight distinct 17408 x 5120 weights so the
/// weights cannot stay in the system cache; 2,048 tokens per GEMM.
#[test]
#[ignore = "sustained timing on a Metal device; about 2 GiB of synthetic buffers"]
fn clef_q4k_sustained_alternation() {
    let metal = ClefMetal::new_with_options(
        crate::clef_attention::ClefMetalAttention::default(),
        crate::clef_gemm::ClefMetalGemm::WeightShapes,
    )
    .expect("Metal");
    let names: Vec<String> = std::env::var("CLEF_ALT")
        .unwrap_or_else(|_| String::from("f16,Direct,Stationary"))
        .split(',')
        .map(String::from)
        .collect();
    let window: f64 = std::env::var("CLEF_ALT_WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20.0);
    let cycles: usize = std::env::var("CLEF_ALT_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let (m, k, n, copies) = (17408usize, 5120usize, 2048usize, 8usize);
    let mut packed = Vec::new();
    let mut decoded = Vec::new();
    for copy in 0..copies {
        let mut bytes = weights(ClefMetalWeightFormat::Q4K, m * k);
        bytes[100] = copy as u8;
        let source = metal.buffer_with_bytes(&bytes);
        let f16 = metal.buffer(m * k * 2);
        let batch = metal.batch();
        batch
            .dequantize_to_f16(ClefMetalWeightFormat::Q4K, &source, &f16, m * k)
            .unwrap();
        batch.commit_wait().unwrap();
        packed.push(source);
        decoded.push(f16);
    }
    let input = metal.buffer(n * k * 2);
    let out = metal.buffer(n * m * 4);
    let kernel_of = |name: &str| {
        ClefMetalQ4Gemm::ALL
            .into_iter()
            .find(|kernel| format!("{kernel:?}") == name)
    };
    for cycle in 0..cycles {
        let mut order = names.clone();
        if cycle % 2 == 1 {
            order.reverse();
        }
        for name in &order {
            let kernel = if name == "f16" {
                None
            } else {
                Some(kernel_of(name).expect("known kernel"))
            };
            let start = std::time::SystemTime::now();
            let started = std::time::Instant::now();
            let mut gemms = 0usize;
            while started.elapsed().as_secs_f64() < window {
                let batch = metal.batch();
                for copy in 0..copies {
                    match kernel {
                        Some(kernel) => batch
                            .gemm_q4k_with(
                                kernel,
                                &input,
                                0,
                                &packed[copy],
                                &out,
                                0,
                                n,
                                m,
                                k,
                                false,
                            )
                            .unwrap(),
                        None => batch
                            .gemm(&input, 0, &decoded[copy], &out, 0, n, m, k, false)
                            .unwrap(),
                    }
                }
                batch.commit_wait().unwrap();
                gemms += copies;
            }
            let seconds = started.elapsed().as_secs_f64();
            let unix = start
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();
            println!(
                "{{\"kernel\":\"{name}\",\"unix\":{unix:.3},\"seconds\":{seconds:.3},\"gemms\":{gemms},\"tflops\":{:.3}}}",
                2.0 * (gemms * n * m * k) as f64 / seconds / 1e12
            );
        }
    }
}
