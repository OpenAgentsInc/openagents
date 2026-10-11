//! Opt-in correctness checks for GEMM tiles selected from Clef weight shapes.

use super::*;
use crate::clef_gemm::ClefMetalGemm;

const INPUT_OFFSET: usize = 32;
const OUTPUT_OFFSET: usize = 32;
const GUARD_ROWS: usize = 64;
const HALF_GUARD: u16 = 0x4400; // Exactly 4.0 in f16.
const FLOAT_GUARD: f32 = 777.5;
const UPLOAD_ELEMENTS: usize = 32_768;

// Exact f16 bits and the corresponding f32 values avoid a dependency or a
// full-size f32 copy of a 96 MiB weight matrix. Negative and mixed magnitudes
// exercise cancellation and residual accumulation.
const VALUES: [(u16, f32); 11] = [
    (0x0000, 0.0),
    (0x2800, 0.03125),
    (0xa800, -0.03125),
    (0x3000, 0.125),
    (0xb000, -0.125),
    (0x3800, 0.5),
    (0xb800, -0.5),
    (0x3c00, 1.0),
    (0xbc00, -1.0),
    (0x4000, 2.0),
    (0xc000, -2.0),
];

fn input_value(row: usize, column: usize) -> (u16, f32) {
    VALUES[((row * 11 + row / 3) ^ (column * 7 + column / 17)) % VALUES.len()]
}

fn weight_value(row: usize, column: usize) -> (u16, f32) {
    VALUES[((row * 13 + row / 5 + 3) ^ (column * 19 + column / 23)) % VALUES.len()]
}

fn residual(row: usize, column: usize) -> f32 {
    ((row * 3 + column * 5) % 7) as f32 / 8.0 - 0.375 + 0.03125
}

fn half_buffer(
    metal: &ClefMetal,
    elements: usize,
    value: impl Fn(usize) -> u16,
) -> ClefMetalBuffer {
    let buffer = metal.buffer(elements * 2);
    let mut bytes = Vec::with_capacity(UPLOAD_ELEMENTS * 2);
    for first in (0..elements).step_by(UPLOAD_ELEMENTS) {
        bytes.clear();
        for index in first..(first + UPLOAD_ELEMENTS).min(elements) {
            bytes.extend_from_slice(&value(index).to_le_bytes());
        }
        buffer
            .write_bytes(first * 2, &bytes)
            .expect("half input upload");
    }
    buffer
}

fn assert_float_guard(buffer: &ClefMetalBuffer, first: usize, count: usize, bits: u32) {
    for offset in (0..count).step_by(1024) {
        let values = buffer
            .read_f32(first + offset, (count - offset).min(1024))
            .expect("guard read");
        assert!(
            values.iter().all(|value| value.to_bits() == bits),
            "guard changed at or after {offset}"
        );
    }
}

fn assert_half_guard(buffer: &ClefMetalBuffer, first: usize, count: usize) {
    assert_eq!(first % 2, 0);
    assert_eq!(count % 2, 0);
    let pair = u32::from(HALF_GUARD) | (u32::from(HALF_GUARD) << 16);
    assert_float_guard(buffer, first / 2, count / 2, pair);
}

#[allow(clippy::too_many_arguments)]
fn run_gemm(
    metal: &ClefMetal,
    input: &ClefMetalBuffer,
    weight: &ClefMetalBuffer,
    n: usize,
    m: usize,
    k: usize,
    first_row: usize,
    accumulate: bool,
) -> Vec<f32> {
    // Drain each completed command's autoreleased objects before the next
    // output allocation, so the test's memory bound includes live buffers.
    objc::rc::autoreleasepool(|| {
        let count = n * m;
        let total = OUTPUT_OFFSET + count + GUARD_ROWS * m;
        let output = metal.buffer(total * 4);
        let mut values = Vec::with_capacity(UPLOAD_ELEMENTS);
        for first in (0..total).step_by(UPLOAD_ELEMENTS) {
            values.clear();
            for index in first..(first + UPLOAD_ELEMENTS).min(total) {
                values.push(if index < OUTPUT_OFFSET || index >= OUTPUT_OFFSET + count {
                    FLOAT_GUARD
                } else {
                    let index = index - OUTPUT_OFFSET;
                    residual(first_row + index / m, index % m)
                });
            }
            output
                .write_f32(first, &values)
                .expect("output initialization");
        }
        drop(values);
        let batch = metal.batch();
        batch
            .gemm(
                input,
                INPUT_OFFSET + first_row * k,
                weight,
                &output,
                OUTPUT_OFFSET,
                n,
                m,
                k,
                accumulate,
            )
            .expect("encode GEMM");
        batch.commit_wait().expect("complete GEMM");
        assert_float_guard(&output, 0, OUTPUT_OFFSET, FLOAT_GUARD.to_bits());
        assert_float_guard(
            &output,
            OUTPUT_OFFSET + count,
            GUARD_ROWS * m,
            FLOAT_GUARD.to_bits(),
        );
        output.read_f32(OUTPUT_OFFSET, count).expect("GEMM output")
    })
}

fn close(actual: f32, expected: f32, context: &str) {
    // These are proposed acceptance bounds, not results measured on a device.
    let error = (actual - expected).abs();
    let limit = 2e-3 + 2e-5 * expected.abs();
    assert!(
        actual.is_finite() && expected.is_finite() && error <= limit,
        "{context}: actual {actual}, expected {expected}, error {error}, limit {limit}"
    );
}

fn compare_all(actual: &[f32], expected: &[f32], context: &str) {
    assert_eq!(actual.len(), expected.len(), "{context}");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let error = (actual - expected).abs();
        let limit = 2e-3 + 2e-5 * expected.abs();
        assert!(
            actual.is_finite() && expected.is_finite() && error <= limit,
            "{context}, element {index}: actual {actual}, expected {expected}, error {error}, limit {limit}"
        );
    }
}

fn sample_indices(length: usize) -> Vec<usize> {
    let mut indices = vec![0, 1, 31, 32, 33, 63, 64, length / 2, length - 1];
    indices.retain(|index| *index < length);
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn compare_dense_samples(output: &[f32], n: usize, m: usize, k: usize, accumulate: bool) {
    for row in sample_indices(n) {
        for column in sample_indices(m) {
            let mut expected: f64 = (0..k)
                .map(|inner| {
                    f64::from(input_value(row, inner).1) * f64::from(weight_value(column, inner).1)
                })
                .sum();
            if accumulate {
                expected += f64::from(residual(row, column));
            }
            close(
                output[row * m + column],
                expected as f32,
                &format!(
                    "dense reference n={n}, W={m}x{k}, row={row}, column={column}, accumulate={accumulate}"
                ),
            );
        }
    }
}

fn exercise_shape(baseline: &ClefMetal, selected: &ClefMetal, n: usize, m: usize, k: usize) {
    let weight_elements = (m + GUARD_ROWS) * k;
    let input_elements = INPUT_OFFSET + (n + GUARD_ROWS) * k;
    // Weight and input buffers, the largest output buffer, at most three CPU
    // output vectors, and one upload block fit below 120 MB of test data.
    // Shader libraries and driver allocations are outside this accounting.
    let maximum_data_bytes = weight_elements * 2
        + input_elements * 2
        + (OUTPUT_OFFSET + (n + GUARD_ROWS) * m) * 4
        + 3 * n * m * 4
        + UPLOAD_ELEMENTS * 4;
    assert!(
        maximum_data_bytes < 120_000_000,
        "case needs {maximum_data_bytes} bytes"
    );
    let weights = half_buffer(selected, weight_elements, |index| {
        if index < m * k {
            weight_value(index / k, index % k).0
        } else {
            HALF_GUARD
        }
    });
    let input = half_buffer(selected, input_elements, |index| {
        if index < INPUT_OFFSET || index >= INPUT_OFFSET + n * k {
            HALF_GUARD
        } else {
            let index = index - INPUT_OFFSET;
            input_value(index / k, index % k).0
        }
    });
    for accumulate in [false, true] {
        let reference = run_gemm(baseline, &input, &weights, n, m, k, 0, accumulate);
        let whole = run_gemm(selected, &input, &weights, n, m, k, 0, accumulate);
        compare_all(
            &whole,
            &reference,
            &format!("selected vs baseline n={n}, W={m}x{k}, accumulate={accumulate}"),
        );
        compare_dense_samples(&reference, n, m, k, accumulate);
        compare_dense_samples(&whole, n, m, k, accumulate);
        drop(reference);
        if n == 65 {
            // Changing 65 rows into 33 + 32 exercises a different token tile.
            // Compare within a bound; bitwise chunk equivalence is not assumed.
            let mut chunks = Vec::with_capacity(whole.len());
            chunks.extend(run_gemm(
                selected, &input, &weights, 33, m, k, 0, accumulate,
            ));
            chunks.extend(run_gemm(
                selected, &input, &weights, 32, m, k, 33, accumulate,
            ));
            compare_all(
                &chunks,
                &whole,
                &format!("33+32 vs whole, W={m}x{k}, accumulate={accumulate}"),
            );
        }
    }
    assert_half_guard(&input, 0, INPUT_OFFSET);
    assert_half_guard(&input, INPUT_OFFSET + n * k, GUARD_ROWS * k);
    assert_half_guard(&weights, m * k, GUARD_ROWS * k);
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn selected_gemm_tiles_match_baseline_and_dense_samples_for_real_clef_weights() {
    let baseline = ClefMetal::new().expect("baseline Metal device");
    let selected =
        ClefMetal::new_with_options(ClefMetalAttention::Staged, ClefMetalGemm::WeightShapes)
            .expect("shape-selected Metal device");
    // These are the six distinct trunk projection shapes in Clef-Flash Q4_K_M.
    // Each case releases its buffers before allocating the next weight shape.
    for (m, k) in [
        (12288, 4096),
        (4096, 12288),
        (8192, 4096),
        (4096, 4096),
        (1024, 4096),
        (32, 4096),
    ] {
        objc::rc::autoreleasepool(|| exercise_shape(&baseline, &selected, 65, m, k));
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn selected_narrow_gemm_handles_single_rows_and_token_tile_boundaries() {
    let baseline = ClefMetal::new().expect("baseline Metal device");
    let selected =
        ClefMetal::new_with_options(ClefMetalAttention::Staged, ClefMetalGemm::WeightShapes)
            .expect("shape-selected Metal device");
    for n in [1, 63, 64, 127, 129] {
        objc::rc::autoreleasepool(|| exercise_shape(&baseline, &selected, n, 32, 4096));
    }
}
