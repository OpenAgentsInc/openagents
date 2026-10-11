//! Opt-in field decoder stack with one device submission per request.

use super::*;
use crate::clef::head::{ClefHeadConfig, ClefHeadWeights, FieldLayer, Matrix, Norm};
use psionic_backend_metal::clef_prefill::head_blocks::ClefFieldPlan;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum FieldMode {
    #[default]
    Staged,
    FusedFields,
}

impl std::str::FromStr for FieldMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "staged" => Ok(Self::Staged),
            "fused-fields" => Ok(Self::FusedFields),
            _ => Err(String::from(
                "PSIONIC_CLEF_METAL_HEAD must be staged or fused-fields",
            )),
        }
    }
}

struct DeviceNorm {
    weight: ClefMetalBuffer,
    bias: ClefMetalBuffer,
}

struct DeviceField {
    // Q, K, V, output; cross Q, K, V, output; FFN up, down.
    matrices: [usize; 10],
    norms: [DeviceNorm; 3],
    self_biases: [ClefMetalBuffer; 4],
    cross_biases: [ClefMetalBuffer; 3],
    up_bias: ClefMetalBuffer,
    down_bias: ClefMetalBuffer,
}

pub(super) struct FieldStack {
    config: ClefHeadConfig,
    layers: Vec<DeviceField>,
    bytes: usize,
}

fn matrices(layer: &FieldLayer) -> [&Matrix; 10] {
    let a = &layer.self_attention;
    let b = &layer.cross_attention;
    [
        &a.q,
        &a.k,
        &a.v,
        &a.out,
        &b.q,
        &b.k,
        &b.v,
        &b.out,
        &layer.up,
        &layer.down,
    ]
}

pub(super) fn validate(weights: &ClefHeadWeights) -> Result<ClefFieldPlan, String> {
    let c = weights.config;
    let plan = ClefFieldPlan::new(1, c.width, c.heads, c.feedforward)?;
    plan.vector_bytes(c.layers)?;
    if weights.layers.len() != c.layers {
        return Err(String::from("field layer count mismatch"));
    }
    let vector = |values: &[f32], width: usize| {
        if values.len() != width || values.iter().any(|x| !x.is_finite()) {
            Err(String::from("invalid field vector"))
        } else {
            Ok(())
        }
    };
    for layer in &weights.layers {
        for norm in [&layer.self_norm, &layer.cross_norm, &layer.feedforward_norm] {
            vector(&norm.weight, c.width)?;
            vector(&norm.bias, c.width)?;
        }
        for a in [&layer.self_attention, &layer.cross_attention] {
            for bias in [&a.q_bias, &a.k_bias, &a.v_bias, &a.out_bias] {
                vector(bias, c.width)?;
            }
        }
        vector(&layer.up_bias, c.feedforward)?;
        vector(&layer.down_bias, c.width)?;
        for (index, matrix) in matrices(layer).into_iter().enumerate() {
            let (rows, columns) = match index {
                8 => (c.feedforward, c.width),
                9 => (c.width, c.feedforward),
                _ => (c.width, c.width),
            };
            if matrix.rows != rows
                || matrix.columns != columns
                || matrix.values.len() != rows * columns
            {
                return Err(String::from("field matrix shape mismatch"));
            }
        }
    }
    Ok(plan)
}

impl FieldStack {
    pub(super) fn workspace_bytes(&self, rows: usize) -> Result<usize, String> {
        ClefFieldPlan::new(
            rows,
            self.config.width,
            self.config.heads,
            self.config.feedforward,
        )
        .map(|plan| plan.workspace_bytes)
    }
}

impl ClefMetalTrunk {
    /// Uploads the additional field vectors only for the experimental mode.
    pub(crate) fn prepare_field_head(&self, weights: &ClefHeadWeights) -> Result<(), String> {
        let mode = match std::env::var("PSIONIC_CLEF_METAL_HEAD") {
            Ok(value) => value.parse()?,
            Err(std::env::VarError::NotPresent) => FieldMode::Staged,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(String::from("PSIONIC_CLEF_METAL_HEAD must be valid text"));
            }
        };
        self.configure_field_head(weights, mode)
    }

    pub(super) fn configure_field_head(
        &self,
        weights: &ClefHeadWeights,
        mode: FieldMode,
    ) -> Result<(), String> {
        let mut state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if mode == FieldMode::Staged {
            if let Some(old) = state.head_fields.take() {
                state.weight_bytes -= old.bytes as u64;
            }
            return Ok(());
        }
        let plan = validate(weights)?;
        if weights.config.width != self.dims.width {
            return Err(String::from("field width differs from memory"));
        }
        // Validate the entire stack before allocating vectors or recording work.
        for layer in &weights.layers {
            for matrix in matrices(layer) {
                let entry = state
                    .head_matrices
                    .get(&(matrix.values.as_ptr() as usize))
                    .ok_or("field matrix is not on the device")?;
                if (entry.1, entry.2) != (matrix.rows, matrix.columns)
                    || entry.0.byte_len() < matrix.values.len() * 4
                {
                    return Err(String::from("field device matrix shape mismatch"));
                }
            }
        }
        let bytes = plan.vector_bytes(weights.config.layers)?;
        let previous = state
            .head_fields
            .as_ref()
            .map_or(0, |stack| stack.bytes as u64);
        let weight_bytes = state
            .weight_bytes
            .checked_sub(previous)
            .and_then(|value| value.checked_add(bytes as u64))
            .ok_or("field weight size overflow")?;
        let norm = |norm: &Norm| DeviceNorm {
            weight: state.metal.buffer_f32(&norm.weight),
            bias: state.metal.buffer_f32(&norm.bias),
        };
        let layers = weights
            .layers
            .iter()
            .map(|layer| {
                let a = &layer.self_attention;
                let b = &layer.cross_attention;
                DeviceField {
                    matrices: matrices(layer).map(|m| m.values.as_ptr() as usize),
                    norms: [
                        norm(&layer.self_norm),
                        norm(&layer.cross_norm),
                        norm(&layer.feedforward_norm),
                    ],
                    self_biases: [&a.q_bias, &a.k_bias, &a.v_bias, &a.out_bias]
                        .map(|b| state.metal.buffer_f32(b)),
                    // The cross-attention key bias is constant over memory rows and
                    // cancels in softmax, as in the existing query-side path.
                    cross_biases: [&b.q_bias, &b.v_bias, &b.out_bias]
                        .map(|b| state.metal.buffer_f32(b)),
                    up_bias: state.metal.buffer_f32(&layer.up_bias),
                    down_bias: state.metal.buffer_f32(&layer.down_bias),
                }
            })
            .collect();
        state.head_fields = Some(FieldStack {
            config: weights.config,
            layers,
            bytes,
        });
        state.weight_bytes = weight_bytes;
        Ok(())
    }

    /// Runs all field layers, or returns `None` for the default staged mode.
    pub(crate) fn head_field_layers(
        &self,
        weights: &ClefHeadWeights,
        input: &[f32],
        rows: usize,
    ) -> Result<Option<Vec<f32>>, String> {
        let mut state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(stack) = state.head_fields.as_ref() else {
            return Ok(None);
        };
        if stack.config != weights.config
            || stack.layers.len() != weights.layers.len()
            || stack
                .layers
                .iter()
                .zip(&weights.layers)
                .any(|(device, layer)| {
                    device.matrices != matrices(layer).map(|m| m.values.as_ptr() as usize)
                })
        {
            return Err(String::from("field head changed after upload"));
        }
        let c = stack.config;
        let plan = ClefFieldPlan::new(rows, c.width, c.heads, c.feedforward)?;
        if input.len() != rows * c.width || input.iter().any(|x| !x.is_finite()) {
            return Err(String::from("invalid field inputs"));
        }
        let request = state.request.as_ref().ok_or("no prefill on the device")?;
        let length = request.tokens;
        let side_rows = rows * c.heads;
        let capacity = side_rows
            .checked_next_power_of_two()
            .ok_or("field attention rows overflow")?
            .max(64);
        // Existing kernels use 32-bit element indexing. Check the largest
        // rounded scratch extents before allocation or command encoding.
        for columns in [length, c.width] {
            let count = capacity
                .checked_mul(columns)
                .ok_or("field attention size overflow")?;
            if count > i32::MAX as usize {
                return Err(String::from("field attention is too large"));
            }
        }
        let memory_bytes = length
            .checked_mul(c.width)
            .and_then(|x| x.checked_mul(4))
            .ok_or("field memory size overflow")?;
        if length == 0 || request.memory.byte_len() < memory_bytes {
            return Err(String::from("invalid field memory"));
        }
        ensure_head_attention(&mut state, side_rows, length, c.width);
        let stack = state
            .head_fields
            .as_ref()
            .ok_or("field stack unavailable")?;
        let attention = state
            .head_attention
            .as_ref()
            .ok_or("field attention scratch unavailable")?;
        let memory = &state.request.as_ref().ok_or("no prefill")?.memory;
        let fields = state.metal.buffer_f32(input);
        let normalized = state.metal.buffer(plan.row_bytes);
        let q = state.metal.buffer(plan.row_bytes);
        let k = state.metal.buffer(plan.row_bytes);
        let v = state.metal.buffer(plan.row_bytes);
        let context = state.metal.buffer(plan.row_bytes);
        let update = state.metal.buffer(plan.row_bytes);
        let ff = state.metal.buffer(plan.feedforward_bytes);
        let batch = state.metal.batch();
        let width = c.width;
        let scale = 1.0 / ((width / c.heads) as f32).sqrt();
        for layer in &stack.layers {
            let matrix = |index: usize| {
                state
                    .head_matrices
                    .get(&layer.matrices[index])
                    .map(|m| &m.0)
                    .ok_or("field matrix unavailable")
            };
            let ln = |index: usize| {
                batch.layer_norm_f32(
                    &fields,
                    Some((&layer.norms[index].weight, &layer.norms[index].bias)),
                    &normalized,
                    rows,
                    width,
                    self.dims.head_eps,
                )
            };
            ln(0)?;
            for (index, output) in [&q, &k, &v].into_iter().enumerate() {
                batch.gemm_f32(&normalized, matrix(index)?, output, rows, width, width)?;
                batch.bias_act(output, &layer.self_biases[index], rows, width, false)?;
            }
            batch.field_self_attention(&q, &k, &v, &context, rows, c.heads, width)?;
            batch.gemm_f32(&context, matrix(3)?, &update, rows, width, width)?;
            batch.bias_act(&update, &layer.self_biases[3], rows, width, false)?;
            batch.add_f32(&fields, &update, rows * width)?;
            ln(1)?;
            batch.gemm_f32(&normalized, matrix(4)?, &q, rows, width, width)?;
            batch.bias_act(&q, &layer.cross_biases[0], rows, width, false)?;
            batch.head_side(&q, matrix(5)?, &attention.side, rows, c.heads, width)?;
            batch.gemm_f32(
                &attention.side,
                memory,
                &attention.scores,
                side_rows,
                length,
                width,
            )?;
            batch.softmax_rows_f32(&attention.scores, side_rows, length, scale)?;
            batch.gemm_f32_nn(
                &attention.scores,
                memory,
                &attention.mixed,
                side_rows,
                width,
                length,
            )?;
            batch.head_context(
                &attention.mixed,
                matrix(6)?,
                &layer.cross_biases[1],
                &context,
                rows,
                c.heads,
                width,
            )?;
            batch.gemm_f32(&context, matrix(7)?, &update, rows, width, width)?;
            batch.bias_act(&update, &layer.cross_biases[2], rows, width, false)?;
            batch.add_f32(&fields, &update, rows * width)?;
            ln(2)?;
            batch.gemm_f32(&normalized, matrix(8)?, &ff, rows, c.feedforward, width)?;
            batch.bias_act(&ff, &layer.up_bias, rows, c.feedforward, true)?;
            batch.gemm_f32(&ff, matrix(9)?, &update, rows, width, c.feedforward)?;
            batch.bias_act(&update, &layer.down_bias, rows, width, false)?;
            batch.add_f32(&fields, &update, rows * width)?;
        }
        batch.commit_wait()?;
        fields.read_f32(0, rows * width).map(Some)
    }
}
