//! Opt-in device checks for the experimental attention implementation.

use super::*;

const DIM: usize = 256;
const GUARD_ROWS: usize = 64;
const GUARD: f32 = 777.5;

struct Inputs {
    n: usize,
    heads: usize,
    kv_heads: usize,
    first: usize,
    query: Vec<f32>,
    key: Vec<f32>,
    value: Vec<f32>,
}

impl Inputs {
    fn patterned(n: usize, heads: usize, kv_heads: usize, first: usize) -> Self {
        // Small dyadic inputs are represented exactly in f16. Different heads,
        // rows, and channels expose incorrect strides and grouped-head mapping.
        let query = (0..n * heads * DIM)
            .map(|i| ((i * 7 + i / DIM * 3) % 17) as f32 / 64.0 - 0.125)
            .collect();
        let key = (0..(first + n) * kv_heads * DIM)
            .map(|i| ((i * 11 + i / DIM * 5) % 19) as f32 / 32.0 - 0.25)
            .collect();
        let value = (0..(first + n) * kv_heads * DIM)
            .map(|i| {
                let head = i / DIM % kv_heads;
                ((i * 13 + i / DIM * 7) % 23) as f32 / 64.0 - 0.125 + head as f32 / 4.0
            })
            .collect();
        Self {
            n,
            heads,
            kv_heads,
            first,
            query,
            key,
            value,
        }
    }

    fn chunk(&self, offset: usize, n: usize) -> Self {
        let stride = self.heads * DIM;
        Self {
            n,
            heads: self.heads,
            kv_heads: self.kv_heads,
            first: self.first + offset,
            query: self.query[offset * stride..(offset + n) * stride].to_vec(),
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

fn upload_half(metal: &ClefMetal, values: &[f32]) -> ClefMetalBuffer {
    let source = metal.buffer_f32(values);
    let destination = metal.buffer(values.len() * 2);
    let batch = metal.batch();
    batch
        .dequantize_to_f16(
            ClefMetalWeightFormat::F32,
            &source,
            &destination,
            values.len(),
        )
        .expect("convert exact dyadic inputs to f16");
    batch.commit_wait().expect("input conversion");
    destination
}

fn run_attention(metal: &ClefMetal, inputs: &Inputs, fused: bool) -> Vec<f32> {
    let query = upload_half(metal, &inputs.query);
    let key = upload_half(metal, &inputs.key);
    let value = upload_half(metal, &inputs.value);
    let plan = ClefAttentionPlan::new(inputs.n, inputs.heads, inputs.kv_heads, DIM, inputs.first)
        .expect("test shape");
    let count = plan.output_bytes / 4;
    // Cover a full staged query tile so a missing tail mask remains inside the
    // test allocation and is reported as a changed sentinel.
    let guard_elements = GUARD_ROWS * inputs.heads * DIM;
    let output = metal.buffer_f32(&vec![GUARD; count + guard_elements]);
    let batch = metal.batch();
    if fused {
        batch
            .attention_fused(
                &query,
                &key,
                &value,
                &output,
                inputs.n,
                inputs.heads,
                inputs.kv_heads,
                DIM,
                inputs.first,
            )
            .expect("encode fused attention");
        batch.commit_wait().expect("fused attention");
    } else {
        let scores = metal.buffer(plan.score_bytes);
        let probabilities = metal.buffer(plan.probability_bytes);
        batch
            .attention(
                &query,
                &key,
                &value,
                &output,
                &scores,
                &probabilities,
                inputs.n,
                inputs.heads,
                inputs.kv_heads,
                DIM,
                inputs.first,
            )
            .expect("encode staged attention");
        batch.commit_wait().expect("staged attention");
    }
    let tail = output
        .read_f32(count, guard_elements)
        .expect("output guard");
    assert!(
        tail.iter().all(|value| value.to_bits() == GUARD.to_bits()),
        "output tail was overwritten"
    );
    let output = output.read_f32(0, count).expect("attention output");
    assert!(
        output.iter().all(|value| value.is_finite()),
        "nonfinite output"
    );
    output
}

/// Independent dense attention in f64, without the GPU's half-probability
/// approximation. Causality uses absolute positions and grouped-head mapping.
fn dense_attention(inputs: &Inputs) -> Vec<f32> {
    let mut output = vec![0.0; inputs.n * inputs.heads * DIM];
    for row in 0..inputs.n {
        let visible = inputs.first + row + 1;
        for head in 0..inputs.heads {
            let kv_head = head / (inputs.heads / inputs.kv_heads);
            let query_start = (row * inputs.heads + head) * DIM;
            let mut scores = Vec::with_capacity(visible);
            for key in 0..visible {
                let key_start = (key * inputs.kv_heads + kv_head) * DIM;
                scores.push(
                    (0..DIM)
                        .map(|d| {
                            f64::from(inputs.query[query_start + d])
                                * f64::from(inputs.key[key_start + d])
                        })
                        .sum::<f64>(),
                );
            }
            let maximum = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let weights: Vec<_> = scores.iter().map(|score| (score - maximum).exp()).collect();
            let sum: f64 = weights.iter().sum();
            for d in 0..DIM {
                let result: f64 = weights
                    .iter()
                    .enumerate()
                    .map(|(key, weight)| {
                        let value = inputs.value[(key * inputs.kv_heads + kv_head) * DIM + d];
                        weight * f64::from(value) / sum
                    })
                    .sum();
                output[query_start + d] = result as f32;
            }
        }
    }
    output
}

fn assert_close(actual: &[f32], expected: &[f32], absolute: f32, relative: f32, context: &str) {
    assert_eq!(actual.len(), expected.len(), "{context}");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let error = (actual - expected).abs();
        let limit = absolute + relative * expected.abs();
        assert!(
            actual.is_finite() && expected.is_finite() && error <= limit,
            "{context}: element {index}, actual {actual}, expected {expected}, error {error}, limit {limit}"
        );
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn fused_attention_matches_dense_and_staged_at_tails_and_absolute_positions() {
    let metal = ClefMetal::new_with_attention(ClefMetalAttention::FusedTensorOps)
        .expect("fused Metal device");
    for (heads, kv_heads) in [(1, 1), (6, 2)] {
        for first in [0, 31, 32, 33, 65] {
            for n in [1, 15, 16, 17, 33] {
                let inputs = Inputs::patterned(n, heads, kv_heads, first);
                let expected = dense_attention(&inputs);
                let fused = run_attention(&metal, &inputs, true);
                let staged = run_attention(&metal, &inputs, false);
                let context = format!("n={n} heads={heads}/{kv_heads} first={first}");
                // These are acceptance bounds, not measured claims. The two
                // implementations round probabilities at different stages.
                assert_close(
                    &fused,
                    &expected,
                    7.5e-4,
                    1.5e-3,
                    &format!("fused vs dense {context}"),
                );
                assert_close(
                    &staged,
                    &expected,
                    7.5e-4,
                    1.5e-3,
                    &format!("staged vs dense {context}"),
                );
                assert_close(
                    &fused,
                    &staged,
                    7.5e-4,
                    1.5e-3,
                    &format!("fused vs staged {context}"),
                );
            }
        }
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn fused_attention_masks_finite_future_values_and_handles_uniform_probabilities() {
    let metal = ClefMetal::new_with_attention(ClefMetalAttention::FusedTensorOps)
        .expect("fused Metal device");
    let mut inputs = Inputs::patterned(17, 6, 2, 31);
    let original = dense_attention(&inputs);
    // The first query must not see these large finite future values. NaNs are
    // unsuitable sentinels: even a correctly masked staged PV has 0 * NaN.
    let future_start = (inputs.first + 1) * inputs.kv_heads * DIM;
    inputs.key[future_start..].fill(16.0);
    inputs.value[future_start..].fill(16.0);
    let expected = dense_attention(&inputs);
    assert_eq!(
        &expected[..inputs.heads * DIM],
        &original[..inputs.heads * DIM]
    );
    for fused in [false, true] {
        let actual = run_attention(&metal, &inputs, fused);
        assert_close(
            &actual,
            &expected,
            7.5e-4,
            1.5e-3,
            "finite future sentinels",
        );
    }

    // At three visible keys, the staged path rounds each normalized 1/3 to
    // f16. The online path sums unnormalized ones before dividing by three.
    // Both must stay close to the independent oracle; exact equality between
    // implementations would assert a different numerical contract.
    for visible in [3, 65, 97] {
        let mut uniform = Inputs::patterned(1, 6, 2, visible - 1);
        uniform.query.fill(0.0);
        uniform.key.fill(0.0);
        uniform.value.fill(1.0);
        let expected = dense_attention(&uniform);
        let fused = run_attention(&metal, &uniform, true);
        let staged = run_attention(&metal, &uniform, false);
        assert_close(&fused, &expected, 2e-5, 0.0, "uniform fused probabilities");
        assert_close(
            &staged,
            &expected,
            7.5e-4,
            0.0,
            "uniform staged probabilities",
        );
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn fused_attention_rescales_across_large_tile_maximum_changes() {
    let metal = ClefMetal::new_with_attention(ClefMetalAttention::FusedTensorOps)
        .expect("fused Metal device");
    for tile_scores in [
        [96.0, 104.0, 112.0],
        [112.0, 104.0, 96.0],
        [-96.0, 0.0, 96.0],
        [96.0, 0.0, -96.0],
    ] {
        // One nonzero query component makes the score exactly the first key
        // component. Scores and values remain exactly representable in f16.
        // The first two key tiles are full; queries see 1..17 keys of tile 3.
        let mut inputs = Inputs::patterned(17, 6, 2, 64);
        inputs.query.fill(0.0);
        inputs.key.fill(0.0);
        for row in 0..inputs.n {
            for head in 0..inputs.heads {
                inputs.query[(row * inputs.heads + head) * DIM] = 1.0;
            }
        }
        for key in 0..inputs.first + inputs.n {
            let tile = key / 32;
            for head in 0..inputs.kv_heads {
                let start = (key * inputs.kv_heads + head) * DIM;
                inputs.key[start] = tile_scores[tile] + (key % 32) as f32 / 8.0 + head as f32 / 4.0;
                for d in 0..DIM {
                    // Different tile values make stale output accumulators
                    // visible when a later score maximum rescales the sum.
                    inputs.value[start + d] = [-0.75, 0.25, 0.875][tile]
                        + (key % 5) as f32 / 16.0
                        + (d % 7) as f32 / 128.0
                        + head as f32 / 8.0;
                }
            }
        }
        let expected = dense_attention(&inputs);
        let fused = run_attention(&metal, &inputs, true);
        let staged = run_attention(&metal, &inputs, false);
        let context = format!("tile score bases {tile_scores:?}");
        // Acceptance bounds are unmeasured. They allow half-probability
        // rounding while rejecting unstable softmax or missing rescaling.
        assert_close(
            &fused,
            &expected,
            7.5e-4,
            1.5e-3,
            &format!("fused vs dense {context}"),
        );
        assert_close(
            &staged,
            &expected,
            7.5e-4,
            1.5e-3,
            &format!("staged vs dense {context}"),
        );
        assert_close(
            &fused,
            &staged,
            7.5e-4,
            1.5e-3,
            &format!("fused vs staged {context}"),
        );
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn fused_attention_repeats_exactly_and_matches_non_aligned_chunks() {
    let metal = ClefMetal::new_with_attention(ClefMetalAttention::FusedTensorOps)
        .expect("fused Metal device");
    let inputs = Inputs::patterned(65, 6, 2, 65);
    let whole = run_attention(&metal, &inputs, true);
    let repeated = run_attention(&metal, &inputs, true);
    let bits = |values: &[f32]| {
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        bits(&whole),
        bits(&repeated),
        "identical inputs must repeat bit for bit"
    );
    for chunk in [15, 17, 31, 33] {
        let mut combined = Vec::with_capacity(whole.len());
        for first in (0..inputs.n).step_by(chunk) {
            let part = inputs.chunk(first, chunk.min(inputs.n - first));
            combined.extend(run_attention(&metal, &part, true));
        }
        // Chunk grouping can change the number of masked columns in the last
        // TensorOps tile. Require a tight bound without claiming bitwise parity.
        assert_close(&combined, &whole, 2e-5, 5e-5, &format!("chunk {chunk}"));
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn attention_rejects_undersized_buffers_before_dispatch() {
    let metal = ClefMetal::new_with_attention(ClefMetalAttention::FusedTensorOps)
        .expect("fused Metal device");
    let plan = ClefAttentionPlan::new(17, 6, 2, DIM, 33).expect("test shape");
    let required = [
        plan.query_bytes,
        plan.key_value_bytes,
        plan.key_value_bytes,
        plan.output_bytes,
    ];
    let scores = metal.buffer(plan.score_bytes);
    let probabilities = metal.buffer(plan.probability_bytes);
    for fused in [false, true] {
        for short in 0..required.len() {
            let buffers: Vec<_> = required
                .iter()
                .enumerate()
                .map(|(index, bytes)| metal.buffer(if index == short { bytes - 1 } else { *bytes }))
                .collect();
            let batch = metal.batch();
            let result = if fused {
                batch.attention_fused(
                    &buffers[0],
                    &buffers[1],
                    &buffers[2],
                    &buffers[3],
                    17,
                    6,
                    2,
                    DIM,
                    33,
                )
            } else {
                batch.attention(
                    &buffers[0],
                    &buffers[1],
                    &buffers[2],
                    &buffers[3],
                    &scores,
                    &probabilities,
                    17,
                    6,
                    2,
                    DIM,
                    33,
                )
            };
            assert!(
                result.is_err(),
                "accepted undersized buffer {short}, fused={fused}"
            );
            assert!(
                !batch.dispatched.get(),
                "encoded work before rejecting buffer {short}"
            );
            batch.encoder.end_encoding();
        }
    }
}
