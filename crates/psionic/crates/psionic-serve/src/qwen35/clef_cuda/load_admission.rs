//! Reads tensor metadata and resident f32 slice lengths before CUDA uploads.

use super::memory_admission::{BackboneShape, Estimate, Inventory, MatrixFormat, bytes};
use super::{
    ClefCudaHeadParams, CpuGgufQwen35TextGenerationService, CpuQwen35LayerKind, Fused, HostMatrix,
    HostMatrixKind, QuantizationMode, knobs,
};

pub(super) fn estimate(
    cpu: &CpuGgufQwen35TextGenerationService,
    head: &ClefCudaHeadParams<'_>,
    max_tokens: usize,
    chunk: usize,
) -> Result<Estimate, String> {
    let model = &cpu.model;
    let config = &model.descriptor.config;
    let mut shape = BackboneShape {
        hidden: config.hidden_size,
        ffn: 0,
        conv_channels: 0,
        conv_kernel: 0,
        key_heads: 0,
        value_heads: 0,
        state: 0,
        heads: config.block.attention.head_count,
        kv_heads: 0,
        head_dim: config.block.attention.head_dim,
        rotary: config.block.attention.rotary_dim,
        width: head.width,
        hybrid_layers: 0,
        attention_layers: 0,
        flash: knobs().flash,
    };
    if bytes(&[shape.heads, shape.head_dim], 1)? > 1024 * 64 || shape.head_dim > 1024 {
        return Err(String::from(
            "clef cuda prefill: attention head dim above 1024",
        ));
    }
    if head.memory_projection.len() as u64 != bytes(&[head.width, shape.hidden], 1)? {
        return Err(String::from(
            "clef cuda prefill: W_mem does not match the hidden size",
        ));
    }
    let mut inventory = Inventory::default();
    let fused_always = knobs().fused == Fused::Always;
    for (index, layer) in model.layers.iter().enumerate() {
        let mut matrix = |part: &str, weight: &HostMatrix| {
            add_matrix(&mut inventory, weight, fused_always)
                .map_err(|error| format!("blk.{index}.{part}: {error}"))
        };
        let gate = layer.ffn_gate_up.parts.first().ok_or("missing FFN gate")?;
        let up = layer.ffn_gate_up.parts.get(1).ok_or("missing FFN up")?;
        shape.ffn = gate.rows();
        matrix("ffn_gate", gate)?;
        matrix("ffn_up", up)?;
        matrix("ffn_down", &layer.ffn_down)?;
        match &layer.kind {
            CpuQwen35LayerKind::Hybrid(hybrid) => {
                shape.hybrid_layers += 1;
                let parts = &hybrid.qkv_gate_alpha_beta.parts;
                if parts.len() != 4 {
                    return Err(String::from("hybrid layer requires four projections"));
                }
                for (name, weight) in ["attn_qkv", "attn_gate", "ssm_alpha", "ssm_beta"]
                    .into_iter()
                    .zip(parts)
                {
                    matrix(name, weight)?;
                }
                matrix("ssm_out", &hybrid.ssm_out)?;
                shape.conv_channels = parts[0].rows();
                shape.conv_kernel = hybrid.conv_kernel;
                shape.key_heads = hybrid.group_count;
                shape.value_heads = hybrid.time_step_rank;
                shape.state = hybrid.state_size;
                if hybrid.ssm_conv1d.rows != shape.conv_channels
                    || hybrid.ssm_conv1d.columns != shape.conv_kernel
                {
                    return Err(format!(
                        "blk.{index}.ssm_conv1d: conv1d shape does not match attn_qkv"
                    ));
                }
                if shape.conv_kernel > 4 || !matches!(shape.state, 64 | 128 | 256) {
                    return Err(format!(
                        "blk.{index}.ssm: conv kernel {} / state {} are outside this lane",
                        shape.conv_kernel, shape.state
                    ));
                }
                for values in [
                    &hybrid.ssm_conv1d.values,
                    &hybrid.ssm_a,
                    &hybrid.ssm_dt,
                    &hybrid.ssm_norm,
                ] {
                    inventory.f32(values.len())?;
                }
            }
            CpuQwen35LayerKind::FullAttention(attention) => {
                shape.attention_layers += 1;
                let parts = &attention.qkv.parts;
                if parts.len() != 3 {
                    return Err(String::from("attention layer requires three projections"));
                }
                for (name, weight) in ["attn_q", "attn_k", "attn_v"].into_iter().zip(parts) {
                    matrix(name, weight)?;
                }
                matrix("attn_output", &attention.output)?;
                shape.kv_heads = attention.kv_width / shape.head_dim.max(1);
                if parts[0].rows() as u64 != bytes(&[shape.heads, shape.head_dim], 2)? {
                    return Err(format!(
                        "blk.{index}.attn_q: expected an interleaved query/gate projection"
                    ));
                }
                inventory.f32(attention.query_norm.len())?;
                inventory.f32(attention.key_norm.len())?;
            }
        }
        inventory.f32(layer.attention_norm.len())?;
        inventory.f32(layer.post_attention_norm.len())?;
    }
    for values in [
        model.output_norm.as_slice(),
        head.hidden_norm_weight,
        head.hidden_norm_bias,
        head.memory_projection,
    ] {
        inventory.f32(values.len())?;
    }
    for (weight, bias) in &head.evidence_norms {
        inventory.f32(weight.len())?;
        inventory.f32(bias.len())?;
    }
    for (values, rows, columns) in &head.linear_matrices {
        if values.len() as u64 != bytes(&[*rows, *columns], 1)? {
            return Err(String::from("clef cuda head: matrix shape mismatch"));
        }
        inventory.f32(values.len())?;
    }
    Estimate::new(&inventory, shape, max_tokens, chunk)
}

fn add_matrix(
    inventory: &mut Inventory,
    matrix: &HostMatrix,
    fused_always: bool,
) -> Result<(), String> {
    match &matrix.kind {
        HostMatrixKind::Dense(dense) => {
            if dense.values.len() as u64 != bytes(&[dense.rows, dense.columns], 1)? {
                return Err(String::from("dense matrix shape does not match its values"));
            }
            inventory.matrix(dense.rows, dense.columns, MatrixFormat::F32, fused_always)?;
        }
        HostMatrixKind::Quantized(quantized) => {
            let format = match quantized.mode {
                QuantizationMode::GgmlQ8_0 => MatrixFormat::Q8_0,
                QuantizationMode::GgmlQ4K => MatrixFormat::Q4K,
                other => {
                    return Err(format!(
                        "{other:?} is unsupported; this lane dequantizes Q8_0, Q4_K and f32"
                    ));
                }
            };
            let resident =
                inventory.matrix(quantized.rows, quantized.columns, format, fused_always)?;
            if resident != bytes(&[quantized.rows, quantized.row_byte_len], 1)? {
                return Err(String::from(
                    "quantized matrix row size does not match its format",
                ));
            }
        }
    }
    Ok(())
}
