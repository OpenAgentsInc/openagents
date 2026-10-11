//! Input checks and bounded device checks for Metal prefill and its consumers.

use super::*;

mod ffn_weights;
mod head_blocks;

#[test]
fn prefill_span_validation_accepts_boundaries_and_overlapping_spans() {
    let maximum = i32::MAX as usize;
    for (length, spans) in [
        (1, vec![(0, 1)]),
        (17, vec![(0, 17), (3, 8), (7, 9), (16, 17)]),
        (maximum, vec![(0, maximum), (maximum - 1, maximum)]),
    ] {
        assert!(validate_prefill_spans(length, &spans).is_ok());
    }
}

#[test]
fn prefill_span_validation_allows_observers_without_spans() {
    assert!(validate_prefill_spans(1, &[]).is_ok());
    assert!(validate_prefill_spans(i32::MAX as usize, &[]).is_ok());
}

#[test]
fn prefill_span_validation_rejects_invalid_prompt_lengths_without_allocating_tokens() {
    for length in [0, i32::MAX as usize + 1, usize::MAX] {
        assert!(validate_prefill_spans(length, &[]).is_err());
        assert!(validate_prefill_spans(length, &[(0, 1)]).is_err());
    }
}

#[test]
fn prefill_span_validation_rejects_empty_reversed_and_outside_spans() {
    for span in [(0, 0), (5, 5), (9, 8), (0, 18), (17, 18), (0, usize::MAX)] {
        assert!(validate_prefill_spans(17, &[span]).is_err());
        assert!(validate_prefill_spans(17, &[(0, 1), span]).is_err());
    }
}

const WIDTH: usize = 64;
const TOKENS: usize = 17;
const ROWS: usize = 3;
const HEADS: usize = 2;
const EPS: f32 = 1e-5;
const SCALE: f32 = 0.25;
const MEMORY_BYTES: u64 = (TOKENS * WIDTH * 4) as u64;

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn grouped_ffn_preserves_independent_request_rows() {
    let fixture = Fixture::new();
    let mut trunk = fixture.trunk(&Fixture::memory(0));
    trunk.dims.ffn = WIDTH;
    let dims = trunk.dims;
    let mut state = trunk.device.lock().unwrap();
    let weight = |salt: usize| {
        let values: Vec<f32> = (0..WIDTH * WIDTH)
            .map(|i| ((i * 7 + i / WIDTH * 11 + salt) % 23) as f32 / 256.0 - 0.04)
            .collect();
        let source = state.metal.buffer_f32(&values);
        let buffer = state.metal.buffer(WIDTH * WIDTH * 2);
        let batch = state.metal.batch();
        batch
            .dequantize_to_f16(ClefMetalWeightFormat::F32, &source, &buffer, values.len())
            .unwrap();
        batch.commit_wait().unwrap();
        DeviceWeight {
            buffer,
            rows: WIDTH,
            columns: WIDTH,
            format: None,
        }
    };
    let layer = DeviceLayer {
        attention_norm: state.metal.buffer_f32(&vec![1.0; WIDTH]),
        post_attention_norm: state.metal.buffer_f32(&vec![1.0; WIDTH]),
        gate: weight(1),
        up: weight(9),
        down: weight(17),
        // This fixture exercises FFN grouping only; no mixer runs.
        mixer: DeviceMixer::Attention {
            query_gate: weight(0),
            key: weight(0),
            value: weight(0),
            out: weight(0),
            query_norm: state.metal.buffer(16),
            key_norm: state.metal.buffer(16),
        },
    };
    ensure_scratch(&mut state, dims, 3).unwrap();
    let first = state.scratch.take().unwrap();
    ensure_scratch(&mut state, dims, 5).unwrap();
    let second = state.scratch.take().unwrap();
    ensure_scratch(&mut state, dims, 8).unwrap();
    let packed = state.scratch.take().unwrap();
    let a: Vec<_> = (0..3 * WIDTH)
        .map(|i| (i % 29) as f32 / 32.0 - 0.2)
        .collect();
    let b: Vec<_> = (0..5 * WIDTH)
        .map(|i| (i % 37) as f32 / 64.0 + 0.3)
        .collect();
    first.x.write_f32(0, &a).unwrap();
    second.x.write_f32(0, &b).unwrap();
    let batch = state.metal.batch();
    encode_ffn(&batch, &layer, &first, dims, 3).unwrap();
    encode_ffn(&batch, &layer, &second, dims, 5).unwrap();
    batch.commit_wait().unwrap();
    let expected_a = first.x.read_f32(0, a.len()).unwrap();
    let expected_b = second.x.read_f32(0, b.len()).unwrap();
    first.x.write_f32(0, &a).unwrap();
    second.x.write_f32(0, &b).unwrap();
    let batch = state.metal.batch();
    batch.copy_f32(&first.x, 0, &packed.x, 0, a.len()).unwrap();
    batch
        .copy_f32(&second.x, 0, &packed.x, a.len(), b.len())
        .unwrap();
    encode_ffn(&batch, &layer, &packed, dims, 8).unwrap();
    batch.copy_f32(&packed.x, 0, &first.x, 0, a.len()).unwrap();
    batch
        .copy_f32(&packed.x, a.len(), &second.x, 0, b.len())
        .unwrap();
    batch.commit_wait().unwrap();
    for (actual, expected) in first
        .x
        .read_f32(0, a.len())
        .unwrap()
        .iter()
        .zip(&expected_a)
        .chain(
            second
                .x
                .read_f32(0, b.len())
                .unwrap()
                .iter()
                .zip(&expected_b),
        )
    {
        assert!(
            actual.is_finite() && (actual - expected).abs() <= 1e-5,
            "grouped FFN differs: {actual} versus {expected}"
        );
    }
}

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
            prefix: PrefixCache::new(0),
            batch_prefix: None,
            metal,
            layers: Vec::new(),
            scratch: None,
            head_io: None,
            head_attention: None,
            head_fields: None,
            weight_bytes: 0,
            weight_workspace_bytes: 0,
        };
        ClefMetalTrunk {
            device: Arc::new(Mutex::new(state)),
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

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn prefix_snapshot_restores_state_and_memory_without_suffix_bytes() {
    objc::rc::autoreleasepool(|| {
        let fixture = Fixture::new();
        let trunk = fixture.trunk(&Fixture::memory(0));
        let state = trunk.device.lock().unwrap();
        let metal = &state.metal;
        let mut cold = request(metal, &Fixture::memory(0));
        cold.conv = vec![Some(metal.buffer_with_bytes(&[7u8; 32])), None];
        cold.delta = vec![Some(metal.buffer_with_bytes(&[13u8; 64])), None];
        cold.key_cache = vec![
            None,
            Some(metal.buffer_with_bytes(&vec![19u8; TOKENS * WIDTH * 2])),
        ];
        cold.value_cache = vec![
            None,
            Some(metal.buffer_with_bytes(&vec![23u8; TOKENS * WIDTH * 2])),
        ];
        let prefix = 5;
        let snapshot = Snapshot::capture(&cold, trunk.dims, prefix).unwrap();
        let mut warm = request(metal, &vec![-2.0; TOKENS * WIDTH]);
        warm.conv = vec![Some(metal.buffer(32)), None];
        warm.delta = vec![Some(metal.buffer(64)), None];
        warm.key_cache = vec![None, Some(metal.buffer(TOKENS * WIDTH * 2))];
        warm.value_cache = vec![None, Some(metal.buffer(TOKENS * WIDTH * 2))];
        snapshot.restore(&warm).unwrap();
        assert_eq!(
            warm.conv[0].as_ref().unwrap().read_bytes(0, 32).unwrap(),
            vec![7u8; 32]
        );
        assert_eq!(
            warm.delta[0].as_ref().unwrap().read_bytes(0, 64).unwrap(),
            vec![13u8; 64]
        );
        for (cache, byte) in [(&warm.key_cache, 19u8), (&warm.value_cache, 23u8)] {
            let bytes = cache[1]
                .as_ref()
                .unwrap()
                .read_bytes(0, TOKENS * WIDTH * 2)
                .unwrap();
            assert_eq!(
                &bytes[..prefix * WIDTH * 2],
                &vec![byte; prefix * WIDTH * 2]
            );
            assert!(bytes[prefix * WIDTH * 2..].iter().all(|&value| value == 0));
        }
        let memory = warm.memory.read_f32(0, TOKENS * WIDTH).unwrap();
        assert_eq!(
            &memory[..prefix * WIDTH],
            &Fixture::memory(0)[..prefix * WIDTH]
        );
        assert!(memory[prefix * WIDTH..].iter().all(|&value| value == -2.0));
        assert!(
            warm.normalized_memory
                .select(&warm.memory, Some(0), None)
                .is_err()
        );

        // Validate every destination before writing any state.
        let mut wrong = request(metal, &vec![-3.0; TOKENS * WIDTH]);
        wrong.conv = vec![Some(metal.buffer(32)), None];
        wrong.delta = vec![Some(metal.buffer(8)), None];
        wrong.key_cache = vec![None, Some(metal.buffer(TOKENS * WIDTH * 2))];
        wrong.value_cache = vec![None, Some(metal.buffer(TOKENS * WIDTH * 2))];
        assert!(snapshot.restore(&wrong).is_err());
        assert_eq!(
            wrong.conv[0].as_ref().unwrap().read_bytes(0, 32).unwrap(),
            vec![0; 32]
        );
        assert!(
            wrong
                .memory
                .read_f32(0, TOKENS * WIDTH)
                .unwrap()
                .iter()
                .all(|&value| value == -3.0)
        );
        assert!(Snapshot::byte_len(&cold, trunk.dims, usize::MAX).is_err());
    });
}
