//! Same-input field and complete-head comparisons without a backbone model.

use super::super::head_blocks::{FieldMode, validate};
use super::*;
use crate::clef::encode::{EncodedQuestion, EncodedRecord};
use crate::clef::head::{self, ClefHeadWeights, HostMemory, MemoryAttention, MemoryView};

fn weights() -> ClefHeadWeights {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/clef/head/tiny");
    ClefHeadWeights::from_safetensors(&path, 64).unwrap()
}

#[test]
fn field_mode_is_explicit_and_shapes_are_checked_without_device_allocations() {
    assert_eq!(FieldMode::default(), FieldMode::Staged);
    assert_eq!("staged".parse::<FieldMode>().unwrap(), FieldMode::Staged);
    assert_eq!(
        "fused-fields".parse::<FieldMode>().unwrap(),
        FieldMode::FusedFields
    );
    for invalid in ["", "true", "fused", "automatic"] {
        assert!(invalid.parse::<FieldMode>().is_err());
    }
    let weights = weights();
    assert!(validate(&weights).is_ok());
    let mut bad = weights.clone();
    bad.layers[0].self_norm.bias.pop();
    assert!(validate(&bad).is_err());
    let mut bad = weights.clone();
    bad.layers[0].cross_attention.out.columns += 1;
    assert!(validate(&bad).is_err());
    let mut bad = weights.clone();
    bad.layers[0].up_bias[0] = f32::NAN;
    assert!(validate(&bad).is_err());
    let mut bad = weights.clone();
    bad.layers.pop();
    assert!(validate(&bad).is_err());
}

fn fixture(weights: &ClefHeadWeights, memory: &[f32], tokens: usize) -> ClefMetalTrunk {
    let source = Fixture::new();
    let mut trunk = source.trunk(&Fixture::memory(0));
    trunk.dims.width = weights.config.width;
    let mut state = trunk.device.lock().unwrap();
    state.head_matrices = weights
        .linear_matrices()
        .into_iter()
        .map(|m| {
            (
                m.values.as_ptr() as usize,
                (state.metal.buffer_f32(&m.values), m.rows, m.columns),
            )
        })
        .collect();
    state.evidence_norms = weights
        .evidence_layers
        .iter()
        .map(|layer| {
            (
                state.metal.buffer_f32(&layer.memory_norm.weight),
                state.metal.buffer_f32(&layer.memory_norm.bias),
            )
        })
        .collect();
    state.request = Some(Request {
        tokens,
        conv: Vec::new(),
        delta: Vec::new(),
        key_cache: Vec::new(),
        value_cache: Vec::new(),
        memory: state.metal.buffer_f32(memory),
        normalized_memory: MemoryViews::new(weights.config.routing_layers),
        attention: TrunkAttentionScratch::Unused,
        bytes: (memory.len() * 4) as u64,
    });
    drop(state);
    trunk
}

struct FieldMemory<'a> {
    host: HostMemory<'a>,
    trunk: &'a ClefMetalTrunk,
}

impl MemoryAttention for FieldMemory<'_> {
    fn attend(
        &mut self,
        view: MemoryView,
        queries: &[f32],
        rows: usize,
        scale: f32,
    ) -> Result<Vec<f32>, String> {
        self.host.attend(view, queries, rows, scale)
    }

    fn field_layers(
        &mut self,
        weights: &ClefHeadWeights,
        fields: &[f32],
        rows: usize,
    ) -> Result<Option<Vec<f32>>, String> {
        self.trunk.head_field_layers(weights, fields, rows)
    }
}

fn compare(got: &[f32], expected: &[f32], label: &str) {
    assert_eq!(got.len(), expected.len());
    for (index, (a, b)) in got.iter().zip(expected).enumerate() {
        assert!(
            a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-3,
            "{label}[{index}]: {a} vs {b}"
        );
    }
}

fn values(length: usize, salt: usize) -> Vec<f32> {
    (0..length)
        .map(|i| ((i * 19 + salt) % 97) as f32 / 61.0 - 0.7)
        .collect()
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn fused_fields_preserve_same_input_logits_tops_and_request_memory() {
    objc::rc::autoreleasepool(|| {
        let weights = weights();
        let width = weights.config.width;
        let memory = values(TOKENS * width, 17);
        let trunk = fixture(&weights, &memory, TOKENS);
        assert!(trunk.head_field_layers(&weights, &[], 0).unwrap().is_none());
        let staged_bytes = trunk
            .session_bytes(TOKENS, 3, 12, weights.config.heads)
            .unwrap();
        trunk
            .configure_field_head(&weights, FieldMode::FusedFields)
            .unwrap();
        let fused_bytes = trunk
            .session_bytes(TOKENS, 3, 12, weights.config.heads)
            .unwrap();
        let expected = psionic_backend_metal::clef_prefill::head_blocks::ClefFieldPlan::new(
            12,
            width,
            weights.config.heads,
            weights.config.feedforward,
        )
        .unwrap();
        assert_eq!(fused_bytes - staged_bytes, expected.workspace_bytes as u64);
        assert_eq!(
            trunk.device.lock().unwrap().weight_bytes,
            expected.vector_bytes(weights.config.layers).unwrap() as u64
        );
        for rows in [1, 3, 9, 17, 63, 64] {
            let fields = values(rows * width, rows);
            let expected = head::field_layers_host(
                &weights,
                fields.clone(),
                rows,
                &mut HostMemory::new(&weights, &memory),
            )
            .unwrap();
            let got = trunk
                .head_field_layers(&weights, &fields, rows)
                .unwrap()
                .unwrap();
            compare(&got, &expected, "field stack");
            assert_eq!(fields, values(rows * width, rows));
            assert_eq!(
                trunk
                    .device
                    .lock()
                    .unwrap()
                    .request
                    .as_ref()
                    .unwrap()
                    .memory
                    .read_f32(0, memory.len())
                    .unwrap(),
                memory
            );
            let record = EncodedRecord {
                input_ids: (0..TOKENS as u32).collect(),
                questions: (0..rows)
                    .map(|i| EncodedQuestion {
                        question_id: format!("q{i}"),
                        question_type: i % 3,
                        question_span: (0, 1),
                        option_spans: vec![(1, 2), (2, 3), (3, 5)],
                        option_ids: vec![String::from("a"), String::from("b"), String::from("c")],
                    })
                    .collect(),
                truncated_state_tokens: 0,
                prefix_tokens: 0,
                media: None,
            };
            let spans: Vec<_> = (0..rows * 4)
                .map(|i| values(weights.config.hidden_size, i + 7))
                .collect();
            let global = values(weights.config.hidden_size, 81);
            let lexical = |ids: &[u32]| {
                Ok(ids
                    .iter()
                    .map(|id| values(weights.config.hidden_size, *id as usize))
                    .collect())
            };
            let reference = head::run_head(
                &weights,
                &record,
                &spans,
                &global,
                &mut HostMemory::new(&weights, &memory),
                &lexical,
            )
            .unwrap();
            let actual = head::run_head(
                &weights,
                &record,
                &spans,
                &global,
                &mut FieldMemory {
                    host: HostMemory::new(&weights, &memory),
                    trunk: &trunk,
                },
                &lexical,
            )
            .unwrap();
            for (got, expected) in actual.iter().zip(&reference) {
                compare(got, expected, "complete head");
                let top = |xs: &[f32]| {
                    xs.iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| a.total_cmp(b))
                        .unwrap()
                        .0
                };
                assert_eq!(top(got), top(expected));
            }
        }
        // A different prompt length and nearly constant field rows exercise
        // scratch replacement and the two-pass normalization after reuse.
        let next_tokens = 11;
        let next_memory = values(next_tokens * width, 71);
        {
            let mut state = trunk.device.lock().unwrap();
            let next = state.metal.buffer_f32(&next_memory);
            let request = state.request.as_mut().unwrap();
            request.tokens = next_tokens;
            request.memory = next;
            request.normalized_memory = MemoryViews::new(weights.config.routing_layers);
            request.bytes = (next_memory.len() * 4) as u64;
        }
        let fields: Vec<f32> = (0..3 * width)
            .map(|i| 8.0 + ((i * 7) % 11) as f32 * 0.001)
            .collect();
        let reference = head::field_layers_host(
            &weights,
            fields.clone(),
            3,
            &mut HostMemory::new(&weights, &next_memory),
        )
        .unwrap();
        let actual = trunk
            .head_field_layers(&weights, &fields, 3)
            .unwrap()
            .unwrap();
        compare(&actual, &reference, "changed memory and near-constant rows");
        let state = trunk.device.lock().unwrap();
        for matrix in weights.linear_matrices() {
            let resident = &state.head_matrices[&(matrix.values.as_ptr() as usize)].0;
            assert_eq!(
                resident.read_f32(0, matrix.values.len()).unwrap(),
                matrix.values
            );
        }
        drop(state);
        let changed = weights.clone();
        assert!(
            trunk
                .head_field_layers(&changed, &values(width, 1), 1)
                .is_err()
        );
        assert!(
            trunk
                .head_field_layers(&weights, &values(65 * width, 1), 65)
                .is_err()
        );
        assert!(
            trunk
                .head_field_layers(&weights, &vec![f32::NAN; width], 1)
                .is_err()
        );
        trunk
            .configure_field_head(&weights, FieldMode::Staged)
            .unwrap();
        assert_eq!(trunk.device.lock().unwrap().weight_bytes, 0);
        assert!(trunk.head_field_layers(&weights, &[], 0).unwrap().is_none());
        assert_eq!(
            trunk
                .session_bytes(TOKENS, 3, 12, weights.config.heads)
                .unwrap(),
            staged_bytes
        );
    });
}
