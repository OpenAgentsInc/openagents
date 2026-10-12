//! Packed projection weights checked against the independent CPU decoder.

use super::*;
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
