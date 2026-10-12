use super::*;

#[test]
fn ffn_placement_is_explicit_and_conversion_is_bounded() {
    assert_eq!(FfnWeightMode::default(), FfnWeightMode::ResidentF16);
    assert_eq!("resident-f16".parse(), Ok(FfnWeightMode::ResidentF16));
    assert_eq!("q4k-on-demand".parse(), Ok(FfnWeightMode::Q4KOnDemand));
    for value in ["", "q4k", "Q4K-ON-DEMAND", " q4k-on-demand"] {
        assert!(value.parse::<FfnWeightMode>().is_err());
    }
    assert_eq!(ffn_conversion_bytes(17408, 5120), Ok(178_257_920));
    assert_eq!(ffn_conversion_bytes(12288, 4096), Ok(100_663_296));
    for (rows, columns) in [(0, 256), (usize::MAX, 2), (65536, 65536)] {
        assert!(ffn_conversion_bytes(rows, columns).is_err());
    }
}

fn packed_weights(salt: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; 256 * 256 / 256 * 144];
    for (block, bytes) in bytes.chunks_mut(144).enumerate() {
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = ((block * 17 + index * 37 + salt * 19) % 256) as u8;
        }
        bytes[..2].copy_from_slice(&0x1000u16.to_le_bytes());
        bytes[2..4].copy_from_slice(&0x1000u16.to_le_bytes());
    }
    bytes
}

fn layer(metal: &ClefMetal, compressed: bool) -> DeviceLayer {
    let weight = |salt| {
        let source = metal.buffer_with_bytes(&packed_weights(salt));
        let (buffer, format) = if compressed {
            (source, Some(ClefMetalWeightFormat::Q4K))
        } else {
            let decoded = metal.buffer(256 * 256 * 2);
            let batch = metal.batch();
            batch
                .dequantize_to_f16(ClefMetalWeightFormat::Q4K, &source, &decoded, 256 * 256)
                .unwrap();
            batch.commit_wait().unwrap();
            (decoded, None)
        };
        DeviceWeight {
            buffer,
            rows: 256,
            columns: 256,
            format,
        }
    };
    let unused = || DeviceWeight {
        buffer: metal.buffer(16),
        rows: 1,
        columns: 1,
        format: None,
    };
    DeviceLayer {
        attention_norm: metal.buffer_f32(&vec![1.0; 256]),
        post_attention_norm: metal.buffer_f32(&vec![1.0; 256]),
        gate: weight(1),
        up: weight(2),
        down: weight(3),
        mixer: DeviceMixer::Attention {
            query_gate: unused(),
            key: unused(),
            value: unused(),
            out: unused(),
            query_norm: metal.buffer(16),
            key_norm: metal.buffer(16),
        },
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn q4_ffn_reuses_one_conversion_slot_without_changing_outputs() {
    let fixture = Fixture::new();
    let mut trunk = fixture.trunk(&Fixture::memory(0));
    trunk.dims.hidden = 256;
    trunk.dims.ffn = 256;
    let dims = trunk.dims;
    let mut state = trunk.device.lock().unwrap();
    let resident = layer(&state.metal, false);
    let packed = layer(&state.metal, true);
    let packed_bytes = [
        packed.gate.buffer.read_bytes(0, 36864).unwrap(),
        packed.up.buffer.read_bytes(0, 36864).unwrap(),
        packed.down.buffer.read_bytes(0, 36864).unwrap(),
    ];
    state.weight_workspace_bytes = 256 * 256 * 2;
    for n in [1, 17, 33, 65] {
        ensure_scratch(&mut state, dims, n).unwrap();
        let mut scratch = state.scratch.take().unwrap();
        let guard = state
            .metal
            .buffer_with_bytes(&vec![0x45; state.weight_workspace_bytes + 64]);
        scratch.weight16 = Some(guard.view(32, state.weight_workspace_bytes).unwrap());
        let input = (0..n * 256)
            .map(|i| ((i * 7 + i / 11) % 31) as f32 / 32.0 - 0.5)
            .collect::<Vec<_>>();
        let mut results = Vec::new();
        for layer in [&resident, &packed] {
            scratch.x.write_f32(0, &input).unwrap();
            let batch = state.metal.batch();
            // Six different projection uses share one slot in a single batch.
            // The second layer also checks residual accumulation.
            encode_ffn(&batch, layer, &scratch, dims, n).unwrap();
            encode_ffn(&batch, layer, &scratch, dims, n).unwrap();
            batch.commit_wait().unwrap();
            results.push(scratch.x.read_f32(0, input.len()).unwrap());
        }
        assert_eq!(results[0].len(), results[1].len());
        for (&a, &b) in results[0].iter().zip(&results[1]) {
            assert!(a.is_finite() && b.is_finite());
            assert_eq!(a.to_bits(), b.to_bits(), "tokens={n}: {a} vs {b}");
        }
        assert_eq!(guard.read_bytes(0, 32).unwrap(), vec![0x45; 32]);
        assert_eq!(
            guard
                .read_bytes(state.weight_workspace_bytes + 32, 32)
                .unwrap(),
            vec![0x45; 32]
        );
        for (weight, expected) in [&packed.gate, &packed.up, &packed.down]
            .iter()
            .zip(&packed_bytes)
        {
            assert_eq!(
                &weight.buffer.read_bytes(0, expected.len()).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn prefill_order_is_explicit_and_layer_major_only_spans_chunks() {
    assert_eq!(PrefillOrder::default(), PrefillOrder::Auto);
    assert_eq!("auto".parse(), Ok(PrefillOrder::Auto));
    assert_eq!("chunk-major".parse(), Ok(PrefillOrder::ChunkMajor));
    assert_eq!("layer-major".parse(), Ok(PrefillOrder::LayerMajor));
    for value in ["", "layer", "Layer-Major", " layer-major"] {
        assert!(value.parse::<PrefillOrder>().is_err());
    }
    // Auto: layer-major exactly for on-demand weights over several chunks.
    assert!(PrefillOrder::Auto.layer_major(2, true, false));
    assert!(!PrefillOrder::Auto.layer_major(8, false, false));
    assert!(!PrefillOrder::Auto.layer_major(1, true, false));
    // Forced orders, but never for a single chunk or with per-chunk hooks.
    assert!(PrefillOrder::LayerMajor.layer_major(2, false, false));
    assert!(!PrefillOrder::LayerMajor.layer_major(1, false, false));
    assert!(!PrefillOrder::LayerMajor.layer_major(8, true, true));
    assert!(!PrefillOrder::ChunkMajor.layer_major(8, true, false));
}
