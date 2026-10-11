//! Bounded device checks for batching evidence normalization with its consumer.

use super::*;

const WIDTH: usize = 64;
const TOKENS: usize = 17;
const ROWS: usize = 3;
const HEADS: usize = 2;
const EPS: f32 = 1e-5;
const SCALE: f32 = 0.25;
const MEMORY_BYTES: u64 = (TOKENS * WIDTH * 4) as u64;

#[derive(Clone, Copy, Debug)]
enum Consumer {
    Memory,
    Projected,
    Block,
}

struct Fixture {
    queries: Vec<f32>,
    matrices: [Vec<f32>; 4],
    biases: [Vec<f32>; 3],
    norms: [(Vec<f32>, Vec<f32>); 2],
}

impl Fixture {
    fn new() -> Self {
        Self {
            queries: (0..ROWS * WIDTH)
                .map(|i| ((i * 11 + i / WIDTH * 7) % 31) as f32 / 32.0 - 0.375)
                .collect(),
            matrices: std::array::from_fn(|which| {
                (0..WIDTH * WIDTH)
                    .map(|i| {
                        let row = i / WIDTH;
                        let column = i % WIDTH;
                        // Dense, nonsymmetric matrices expose transposes and head mapping.
                        ((row * 11 + column * 7 + row * column * 3 + which * 5) % 23) as f32 / 512.0
                            - 11.0 / 512.0
                            + if row == column { 0.125 } else { 0.0 }
                    })
                    .collect()
            }),
            biases: std::array::from_fn(|which| {
                (0..WIDTH)
                    .map(|i| ((i * 3 + which * 7) % 13) as f32 / 128.0 - 0.03125)
                    .collect()
            }),
            norms: std::array::from_fn(|layer| {
                (
                    (0..WIDTH)
                        .map(|i| 0.75 + layer as f32 * 0.5 + (i % 7) as f32 / 32.0)
                        .collect(),
                    (0..WIDTH)
                        .map(|i| (i % 9) as f32 / 32.0 - 0.125 + layer as f32 * 0.25)
                        .collect(),
                )
            }),
        }
    }

    fn memory(variant: usize) -> Vec<f32> {
        (0..TOKENS * WIDTH)
            .map(|i| {
                ((i * (13 + variant * 4) + i / WIDTH * 11 + variant * 19) % 43) as f32 / 32.0 - 0.5
                    + (i / WIDTH) as f32 / 64.0
            })
            .collect()
    }

    fn trunk(&self, memory: &[f32]) -> ClefMetalTrunk {
        // No model, trunk layers, or experimental kernels are loaded. The four
        // head matrices, memory, normalized views, and head scratch total <1 MiB.
        let metal = ClefMetal::new().expect("default Metal kernels");
        let device_name = metal.device_name().to_owned();
        let state = DeviceState {
            output_norm: metal.buffer(4),
            hidden_norm_weight: metal.buffer(4),
            hidden_norm_bias: metal.buffer(4),
            memory_projection: metal.buffer(4),
            evidence_norms: self
                .norms
                .iter()
                .map(|(weight, bias)| (metal.buffer_f32(weight), metal.buffer_f32(bias)))
                .collect(),
            head_matrices: self
                .matrices
                .iter()
                .map(|matrix| {
                    (
                        matrix.as_ptr() as usize,
                        (metal.buffer_f32(matrix), WIDTH, WIDTH),
                    )
                })
                .collect(),
            request: Some(request(&metal, memory)),
            metal,
            layers: Vec::new(),
            scratch: None,
            head_io: None,
            head_attention: None,
            weight_bytes: 0,
        };
        ClefMetalTrunk {
            device: Mutex::new(state),
            dims: Dims {
                hidden: WIDTH,
                ffn: 0,
                eps: EPS,
                conv_channels: 0,
                conv_kernel: 0,
                key_heads: 0,
                value_heads: 0,
                state: 0,
                v_head_reordered: false,
                heads: HEADS,
                kv_heads: HEADS,
                head_dim: WIDTH / HEADS,
                rotary: 0,
                attention_scale: SCALE,
                width: WIDTH,
                head_eps: EPS,
            },
            device_name,
        }
    }

    fn run(&self, trunk: &ClefMetalTrunk, consumer: Consumer, evidence: Option<usize>) -> Vec<f32> {
        // Each completed command buffer releases its temporary resources before
        // the next call. The fixture owns the matrix slices used as lookup keys.
        objc::rc::autoreleasepool(|| match consumer {
            Consumer::Memory => trunk
                .attend_memory(evidence, &self.queries, ROWS, SCALE)
                .unwrap(),
            Consumer::Projected => trunk
                .attend_memory_projected(
                    evidence,
                    &self.matrices[1],
                    &self.matrices[2],
                    &self.biases[1],
                    &self.queries,
                    ROWS,
                    HEADS,
                    SCALE,
                )
                .unwrap()
                .expect("resident matrices"),
            Consumer::Block => trunk
                .head_attend_block(
                    evidence,
                    (&self.matrices[0], &self.biases[0]),
                    &self.matrices[1],
                    &self.matrices[2],
                    &self.biases[1],
                    (&self.matrices[3], &self.biases[2]),
                    &self.queries,
                    ROWS,
                    HEADS,
                    SCALE,
                )
                .unwrap()
                .expect("resident matrices"),
        })
    }

    fn reference(&self, consumer: Consumer, memory: &[f32], layer: Option<usize>) -> Vec<f64> {
        let mut memory: Vec<f64> = memory.iter().map(|&x| f64::from(x)).collect();
        if let Some(layer) = layer {
            let (weight, bias) = &self.norms[layer];
            for row in memory.chunks_exact_mut(WIDTH) {
                let mean = row.iter().sum::<f64>() / WIDTH as f64;
                let variance = row.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / WIDTH as f64;
                let denominator = (variance + f64::from(EPS)).sqrt();
                for (column, x) in row.iter_mut().enumerate() {
                    *x = (*x - mean) / denominator * f64::from(weight[column])
                        + f64::from(bias[column]);
                }
            }
        }
        let queries: Vec<f64> = self.queries.iter().map(|&x| f64::from(x)).collect();
        if matches!(consumer, Consumer::Memory) {
            return attention(&queries, &memory, &memory, 1);
        }
        let queries = if matches!(consumer, Consumer::Block) {
            linear(&queries, &self.matrices[0], Some(&self.biases[0]))
        } else {
            queries
        };
        // Form K and V explicitly, independently of the GPU's reassociated
        // query-side product and delayed value projection.
        let keys = linear(&memory, &self.matrices[1], None);
        let values = linear(&memory, &self.matrices[2], Some(&self.biases[1]));
        let context = attention(&queries, &keys, &values, HEADS);
        if matches!(consumer, Consumer::Block) {
            linear(&context, &self.matrices[3], Some(&self.biases[2]))
        } else {
            context
        }
    }
}

fn request(metal: &ClefMetal, memory: &[f32]) -> Request {
    assert_eq!(memory.len(), TOKENS * WIDTH);
    Request {
        tokens: TOKENS,
        conv: Vec::new(),
        delta: Vec::new(),
        key_cache: Vec::new(),
        value_cache: Vec::new(),
        memory: metal.buffer_f32(memory),
        normalized_memory: MemoryViews::new(2),
        attention: TrunkAttentionScratch::Unused,
        bytes: MEMORY_BYTES,
    }
}

fn reset_request(trunk: &ClefMetalTrunk, memory: &[f32]) {
    let mut state = trunk.device.lock().unwrap();
    state.request = Some(request(&state.metal, memory));
    let request = state.request.as_ref().unwrap();
    assert_eq!(request.bytes, MEMORY_BYTES);
    for layer in 0..2 {
        assert!(
            request
                .normalized_memory
                .select(&request.memory, Some(layer), None)
                .is_err()
        );
    }
}

fn bytes(trunk: &ClefMetalTrunk) -> u64 {
    trunk.device.lock().unwrap().request.as_ref().unwrap().bytes
}

fn publish_separate_normalization(trunk: &ClefMetalTrunk, layer: usize) {
    objc::rc::autoreleasepool(|| {
        let mut state = trunk.device.lock().unwrap();
        let request = state.request.as_ref().unwrap();
        let buffer = state.metal.buffer(MEMORY_BYTES as usize);
        let (weight, bias) = &state.evidence_norms[layer];
        let batch = state.metal.batch();
        batch
            .layer_norm_f32(
                &request.memory,
                Some((weight, bias)),
                &buffer,
                TOKENS,
                WIDTH,
                EPS,
            )
            .unwrap();
        // Reproduce the old submission boundary without using encode_view.
        batch.commit_wait().unwrap();
        let pending = PendingMemoryView {
            layer,
            byte_len: buffer.byte_len(),
            buffer,
        };
        let request = state.request.as_mut().unwrap();
        request.bytes += request
            .normalized_memory
            .complete(Some(pending), Ok(()))
            .unwrap() as u64;
    });
}

fn linear(input: &[f64], matrix: &[f32], bias: Option<&[f32]>) -> Vec<f64> {
    input
        .chunks_exact(WIDTH)
        .flat_map(|row| {
            (0..WIDTH).map(move |output| {
                row.iter()
                    .zip(&matrix[output * WIDTH..(output + 1) * WIDTH])
                    .map(|(x, w)| x * f64::from(*w))
                    .sum::<f64>()
                    + bias.map_or(0.0, |bias| f64::from(bias[output]))
            })
        })
        .collect()
}

fn attention(queries: &[f64], keys: &[f64], values: &[f64], heads: usize) -> Vec<f64> {
    let mut output = vec![0.0; ROWS * WIDTH];
    let head_dim = WIDTH / heads;
    for row in 0..ROWS {
        for head in 0..heads {
            let columns = head * head_dim..(head + 1) * head_dim;
            let scores: Vec<f64> = (0..TOKENS)
                .map(|token| {
                    columns
                        .clone()
                        .map(|c| queries[row * WIDTH + c] * keys[token * WIDTH + c])
                        .sum::<f64>()
                        * f64::from(SCALE)
                })
                .collect();
            let max = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let weights: Vec<f64> = scores.iter().map(|score| (score - max).exp()).collect();
            let sum = weights.iter().sum::<f64>();
            for column in columns {
                output[row * WIDTH + column] = (0..TOKENS)
                    .map(|token| weights[token] / sum * values[token * WIDTH + column])
                    .sum();
            }
        }
    }
    output
}

fn assert_reference(actual: &[f32], expected: &[f64], label: &str) {
    assert_eq!(actual.len(), expected.len());
    for (i, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        let tolerance = 2e-5 + 2e-5 * expected.abs();
        assert!(
            actual.is_finite() && (f64::from(actual) - expected).abs() <= tolerance,
            "{label} element {i}: {actual} != {expected} within {tolerance}"
        );
    }
}

fn assert_same(actual: &[f32], expected: &[f32], label: &str) {
    assert_eq!(actual.len(), expected.len());
    for (i, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            actual.is_finite() && expected.is_finite(),
            "{label} element {i} is not finite"
        );
        assert_eq!(actual.to_bits(), expected.to_bits(), "{label} element {i}");
    }
}

fn exercise(consumer: Consumer) {
    objc::rc::autoreleasepool(|| {
        let fixture = Fixture::new();
        let memory = Fixture::memory(0);
        let trunk = fixture.trunk(&memory);
        let raw = fixture.run(&trunk, consumer, None);
        assert_reference(
            &raw,
            &fixture.reference(consumer, &memory, None),
            "raw memory",
        );
        assert_eq!(bytes(&trunk), MEMORY_BYTES);

        let mut combined = Vec::new();
        for layer in 0..2 {
            let cold = fixture.run(&trunk, consumer, Some(layer));
            assert_reference(
                &cold,
                &fixture.reference(consumer, &memory, Some(layer)),
                "cold evidence",
            );
            assert_eq!(bytes(&trunk), MEMORY_BYTES * (layer as u64 + 2));
            assert_same(
                &fixture.run(&trunk, consumer, Some(layer)),
                &cold,
                "warm evidence",
            );
            assert_same(
                &fixture.run(&trunk, consumer, None),
                &raw,
                "raw after evidence",
            );
            assert_eq!(bytes(&trunk), MEMORY_BYTES * (layer as u64 + 2));
            combined.push(cold);
        }
        assert_same(
            &fixture.run(&trunk, consumer, Some(0)),
            &combined[0],
            "first layer after second",
        );
        assert!(
            combined[0]
                .iter()
                .zip(&combined[1])
                .any(|(a, b)| (a - b).abs() > 1e-4),
            "the two evidence layers must produce distinct fixture outputs"
        );
        assert_eq!(bytes(&trunk), MEMORY_BYTES * 3);

        // Reset the request on the same device to test that old views cannot be
        // reused, then compare against the previous two-batch sequence.
        reset_request(&trunk, &memory);
        for (layer, expected) in combined.iter().enumerate() {
            publish_separate_normalization(&trunk, layer);
            assert_same(
                &fixture.run(&trunk, consumer, Some(layer)),
                expected,
                "separate normalization batch",
            );
            assert_eq!(bytes(&trunk), MEMORY_BYTES * (layer as u64 + 2));
        }

        let next_memory = Fixture::memory(1);
        reset_request(&trunk, &next_memory);
        let next = fixture.run(&trunk, consumer, Some(0));
        assert_reference(
            &next,
            &fixture.reference(consumer, &next_memory, Some(0)),
            "next request",
        );
        assert!(
            next.iter()
                .zip(&combined[0])
                .any(|(a, b)| (a - b).abs() > 1e-5),
            "the next request must produce a distinct fixture output"
        );
        assert_eq!(bytes(&trunk), MEMORY_BYTES * 2);
    });
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn evidence_batch_memory_matches_separate_normalization() {
    exercise(Consumer::Memory);
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn evidence_batch_projected_matches_separate_normalization() {
    exercise(Consumer::Projected);
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn evidence_batch_block_matches_separate_normalization() {
    exercise(Consumer::Block);
}
