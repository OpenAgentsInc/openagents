//! Metal sequence prefill for the Clef decision lane (OpenAgentsInc/openagents
//! #11196): the M2 design ([`super::clef_cuda`]) on Apple silicon.
//!
//! - **Projections.** Every trunk weight is dequantized to f16 once at load
//!   and stays resident in unified memory; each projection is one f16 x f16
//!   -> f32 GEMM on the Metal Performance Primitives `matmul2d` tensor op.
//!   The output projections accumulate into the f32 residual in the GEMM.
//! - **Gated DeltaNet.** A sequence causal conv1d with carried inputs, then
//!   the delta rule as a scan over the chunk's tokens with token inputs
//!   staged through threadgroup memory.
//! - **Full attention.** q/k RMSNorm, rotary and the q scale in one kernel
//!   that appends k and v to f16 caches; scores and `P V` on the tensor op,
//!   one query head at a time, with a causal softmax between.
//! - **Head inputs.** As on CUDA: the chunk's rows go through the output
//!   norm, the head's `hidden_norm` and `W_mem` (fixed order) on the device,
//!   span sums accumulate there, and only the span sums and the last row
//!   come back. The head's memory attention reads the device rows.
//!
//! A chunk's whole layer stack is one command buffer. Every reduction has a
//! fixed order, so a token's rows do not depend on the chunk it came in.

use std::sync::Mutex;

use psionic_backend_metal::clef_prefill::{ClefMetal, ClefMetalBatch, ClefMetalBuffer, ClefMetalWeightFormat};
use rayon::prelude::*;

use super::clef_cuda::{RopeTable, rope_table_params};
use super::{
    ClefCudaHeadParams as ClefHeadParams, ClefCudaPrefill as ClefDevicePrefill, ClefLayerObserver,
    CpuGgufQwen35TextGenerationService, CpuQwen35LayerKind, HostMatrix, HostMatrixKind, QuantizationMode, TokenId,
    qwen35_attention_scale,
};

struct DeviceWeight {
    buffer: ClefMetalBuffer,
    rows: usize,
    columns: usize,
}

enum DeviceMixer {
    Hybrid {
        qkv: DeviceWeight,
        z: DeviceWeight,
        alpha: DeviceWeight,
        beta: DeviceWeight,
        out: DeviceWeight,
        conv: ClefMetalBuffer,
        ssm_a: ClefMetalBuffer,
        ssm_dt: ClefMetalBuffer,
        ssm_norm: ClefMetalBuffer,
    },
    Attention {
        query_gate: DeviceWeight,
        key: DeviceWeight,
        value: DeviceWeight,
        out: DeviceWeight,
        query_norm: ClefMetalBuffer,
        key_norm: ClefMetalBuffer,
    },
}

struct DeviceLayer {
    attention_norm: ClefMetalBuffer,
    post_attention_norm: ClefMetalBuffer,
    gate: DeviceWeight,
    up: DeviceWeight,
    down: DeviceWeight,
    mixer: DeviceMixer,
}

#[derive(Clone, Copy, Debug)]
struct Dims {
    hidden: usize,
    ffn: usize,
    eps: f32,
    conv_channels: usize,
    conv_kernel: usize,
    key_heads: usize,
    value_heads: usize,
    state: usize,
    v_head_reordered: bool,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    rotary: usize,
    attention_scale: f32,
    width: usize,
    head_eps: f32,
}

/// Activations for one chunk of up to `capacity` tokens.
struct Scratch {
    capacity: usize,
    x: ClefMetalBuffer,
    act16: ClefMetalBuffer,
    big_a: ClefMetalBuffer,
    big_b: ClefMetalBuffer,
    conv_out: ClefMetalBuffer,
    mid_a: ClefMetalBuffer,
    mid_b: ClefMetalBuffer,
    small_a: ClefMetalBuffer,
    small_b: ClefMetalBuffer,
    small_c: ClefMetalBuffer,
    small_d: ClefMetalBuffer,
    qn: ClefMetalBuffer,
    kn: ClefMetalBuffer,
    key_dot_query: ClefMetalBuffer,
    query16: ClefMetalBuffer,
    cos_sin: ClefMetalBuffer,
    final_rows: ClefMetalBuffer,
    norm_rows: ClefMetalBuffer,
    bytes: u64,
}

/// Per-request device state.
struct Request {
    tokens: usize,
    conv: Vec<Option<ClefMetalBuffer>>,
    delta: Vec<Option<ClefMetalBuffer>>,
    key_cache: Vec<Option<ClefMetalBuffer>>,
    value_cache: Vec<Option<ClefMetalBuffer>>,
    memory: ClefMetalBuffer,
    normalized_memory: Vec<Option<ClefMetalBuffer>>,
    /// Attention scores (f32) and probabilities (f16): chunk x tokens.
    scores: ClefMetalBuffer,
    probs: ClefMetalBuffer,
    bytes: u64,
}

/// Buffers for the head's memory attention.
struct HeadAttention {
    rows: usize,
    length: usize,
    query: ClefMetalBuffer,
    side: ClefMetalBuffer,
    scores: ClefMetalBuffer,
    mixed: ClefMetalBuffer,
    context: ClefMetalBuffer,
    bias: ClefMetalBuffer,
}

struct DeviceState {
    metal: ClefMetal,
    layers: Vec<DeviceLayer>,
    output_norm: ClefMetalBuffer,
    hidden_norm_weight: ClefMetalBuffer,
    hidden_norm_bias: ClefMetalBuffer,
    memory_projection: ClefMetalBuffer,
    evidence_norms: Vec<(ClefMetalBuffer, ClefMetalBuffer)>,
    scratch: Option<Scratch>,
    request: Option<Request>,
    head_matrices: std::collections::HashMap<usize, (ClefMetalBuffer, usize, usize)>,
    head_io: Option<(usize, ClefMetalBuffer, usize, ClefMetalBuffer)>,
    head_attention: Option<HeadAttention>,
    weight_bytes: u64,
}

/// The Clef trunk on the Metal device.
pub struct ClefMetalTrunk {
    device: Mutex<DeviceState>,
    dims: Dims,
    device_name: String,
}

impl std::fmt::Debug for ClefMetalTrunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClefMetalTrunk")
            .field("device", &self.device_name)
            .finish_non_exhaustive()
    }
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding; the slice covers exactly its values.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) }
}

/// Uploads a weight and dequantizes it to resident f16.
fn upload_matrix(metal: &ClefMetal, matrix: &HostMatrix, name: &str) -> Result<DeviceWeight, String> {
    let (format, rows, columns, bytes): (ClefMetalWeightFormat, usize, usize, Vec<u8>) = match &matrix.kind {
        HostMatrixKind::Dense(dense) => (
            ClefMetalWeightFormat::F32,
            dense.rows,
            dense.columns,
            f32_bytes(&dense.values).to_vec(),
        ),
        HostMatrixKind::Quantized(quantized) => {
            let format = match quantized.mode {
                QuantizationMode::GgmlQ8_0 => ClefMetalWeightFormat::Q8_0,
                QuantizationMode::GgmlQ4K => ClefMetalWeightFormat::Q4K,
                other => {
                    return Err(format!(
                        "clef metal prefill: `{name}` is {other:?}; this lane takes Q8_0, Q4_K and f32"
                    ));
                }
            };
            let elements = quantized.rows * quantized.columns;
            let len = format
                .byte_len(elements)
                .filter(|len| *len == quantized.rows * quantized.row_byte_len)
                .ok_or_else(|| format!("clef metal prefill: `{name}` has an unexpected {format:?} size"))?;
            let bytes = quantized.storage.read_range(0, len).map_err(err)?;
            (format, quantized.rows, quantized.columns, bytes.to_vec())
        }
    };
    let source = metal.buffer_with_bytes(&bytes);
    let buffer = metal.buffer(rows * columns * 2);
    let batch = metal.batch();
    batch.dequantize_to_f16(format, &source, &buffer, rows * columns)?;
    batch.commit_wait()?;
    Ok(DeviceWeight { buffer, rows, columns })
}

impl ClefMetalTrunk {
    /// Uploads the trunk of a loaded CPU qwen35 model (the CPU copy keeps
    /// `token_embd` and `output`) and the head's device-side parameters.
    pub fn load(cpu: &CpuGgufQwen35TextGenerationService, head: &ClefHeadParams<'_>) -> Result<Self, String> {
        let metal = ClefMetal::new()?;
        let device_name = metal.device_name().to_string();
        let model = &cpu.model;
        let config = &model.descriptor.config;
        let metadata = &model.family_metadata;
        let mut dims = Dims {
            hidden: config.hidden_size,
            ffn: 0,
            eps: metadata.rms_norm_epsilon,
            conv_channels: 0,
            conv_kernel: 0,
            key_heads: 0,
            value_heads: 0,
            state: 0,
            v_head_reordered: false,
            heads: config.block.attention.head_count,
            kv_heads: 0,
            head_dim: config.block.attention.head_dim,
            rotary: config.block.attention.rotary_dim,
            attention_scale: qwen35_attention_scale(metadata, config.block.attention.head_dim),
            width: head.width,
            head_eps: head.norm_epsilon,
        };
        let mut layers = Vec::with_capacity(model.layers.len());
        let mut weight_bytes = 0u64;
        for (index, layer) in model.layers.iter().enumerate() {
            let name = |part: &str| format!("blk.{index}.{part}");
            let gate = upload_matrix(&metal, &layer.ffn_gate_up.parts[0], &name("ffn_gate"))?;
            let up = upload_matrix(&metal, &layer.ffn_gate_up.parts[1], &name("ffn_up"))?;
            let down = upload_matrix(&metal, &layer.ffn_down, &name("ffn_down"))?;
            dims.ffn = gate.rows;
            let mixer = match &layer.kind {
                CpuQwen35LayerKind::Hybrid(hybrid) => {
                    let parts = &hybrid.qkv_gate_alpha_beta.parts;
                    let qkv = upload_matrix(&metal, &parts[0], &name("attn_qkv"))?;
                    let z = upload_matrix(&metal, &parts[1], &name("attn_gate"))?;
                    let alpha = upload_matrix(&metal, &parts[2], &name("ssm_alpha"))?;
                    let beta = upload_matrix(&metal, &parts[3], &name("ssm_beta"))?;
                    let out = upload_matrix(&metal, &hybrid.ssm_out, &name("ssm_out"))?;
                    dims.conv_channels = qkv.rows;
                    dims.conv_kernel = hybrid.conv_kernel;
                    dims.key_heads = hybrid.group_count;
                    dims.value_heads = hybrid.time_step_rank;
                    dims.state = hybrid.state_size;
                    dims.v_head_reordered = hybrid.v_head_reordered;
                    if hybrid.conv_kernel > 4 || hybrid.state_size != 128 {
                        return Err(format!(
                            "{}: conv kernel {} / state {} are outside this lane",
                            name("ssm"),
                            hybrid.conv_kernel,
                            hybrid.state_size
                        ));
                    }
                    DeviceMixer::Hybrid {
                        conv: metal.buffer_f32(&hybrid.ssm_conv1d.values),
                        ssm_a: metal.buffer_f32(&hybrid.ssm_a),
                        ssm_dt: metal.buffer_f32(&hybrid.ssm_dt),
                        ssm_norm: metal.buffer_f32(&hybrid.ssm_norm),
                        qkv,
                        z,
                        alpha,
                        beta,
                        out,
                    }
                }
                CpuQwen35LayerKind::FullAttention(attention) => {
                    let parts = &attention.qkv.parts;
                    let query_gate = upload_matrix(&metal, &parts[0], &name("attn_q"))?;
                    let key = upload_matrix(&metal, &parts[1], &name("attn_k"))?;
                    let value = upload_matrix(&metal, &parts[2], &name("attn_v"))?;
                    let out = upload_matrix(&metal, &attention.output, &name("attn_output"))?;
                    dims.kv_heads = attention.kv_width / dims.head_dim.max(1);
                    if query_gate.rows != dims.heads * dims.head_dim * 2 || dims.head_dim != 256 {
                        return Err(format!("{}: expected an interleaved query/gate projection of head dim 256", name("attn_q")));
                    }
                    DeviceMixer::Attention {
                        query_norm: metal.buffer_f32(&attention.query_norm),
                        key_norm: metal.buffer_f32(&attention.key_norm),
                        query_gate,
                        key,
                        value,
                        out,
                    }
                }
            };
            let mut weights: Vec<&DeviceWeight> = vec![&gate, &up, &down];
            match &mixer {
                DeviceMixer::Hybrid { qkv, z, alpha, beta, out, .. } => weights.extend([qkv, z, alpha, beta, out]),
                DeviceMixer::Attention { query_gate, key, value, out, .. } => {
                    weights.extend([query_gate, key, value, out]);
                }
            }
            weight_bytes += weights.iter().map(|w| w.buffer.byte_len() as u64).sum::<u64>();
            layers.push(DeviceLayer {
                attention_norm: metal.buffer_f32(&layer.attention_norm),
                post_attention_norm: metal.buffer_f32(&layer.post_attention_norm),
                gate,
                up,
                down,
                mixer,
            });
        }
        if head.memory_projection.len() != head.width * dims.hidden {
            return Err(String::from("clef metal prefill: W_mem does not match the hidden size"));
        }
        let mut head_matrices = std::collections::HashMap::new();
        for (values, rows, columns) in &head.linear_matrices {
            if values.len() != rows * columns {
                return Err(String::from("clef metal head: matrix shape mismatch"));
            }
            let buffer = metal.buffer_f32(values);
            weight_bytes += buffer.byte_len() as u64;
            head_matrices.insert(values.as_ptr() as usize, (buffer, *rows, *columns));
        }
        let state = DeviceState {
            output_norm: metal.buffer_f32(&model.output_norm),
            hidden_norm_weight: metal.buffer_f32(head.hidden_norm_weight),
            hidden_norm_bias: metal.buffer_f32(head.hidden_norm_bias),
            memory_projection: metal.buffer_f32(head.memory_projection),
            evidence_norms: head
                .evidence_norms
                .iter()
                .map(|(w, b)| (metal.buffer_f32(w), metal.buffer_f32(b)))
                .collect(),
            layers,
            scratch: None,
            request: None,
            head_matrices,
            head_io: None,
            head_attention: None,
            weight_bytes,
            metal,
        };
        Ok(Self {
            device: Mutex::new(state),
            dims,
            device_name,
        })
    }

    /// The Metal device name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Bytes of resident trunk weights (f16) and head matrices.
    #[must_use]
    pub fn weight_device_bytes(&self) -> u64 {
        self.device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .weight_bytes
    }

    /// Runs `tokens` through the trunk in chunks of `chunk` (see
    /// [`super::ClefCudaTrunk::prefill`], the same contract).
    pub fn prefill(
        &self,
        cpu: &CpuGgufQwen35TextGenerationService,
        tokens: &[TokenId],
        spans: &[(usize, usize)],
        chunk: usize,
        mut layer_observer: Option<ClefLayerObserver<'_>>,
        mut final_observer: Option<ClefLayerObserver<'_>>,
    ) -> Result<ClefDevicePrefill, String> {
        let dims = self.dims;
        let length = tokens.len();
        if length == 0 {
            return Err(String::from("empty prompt"));
        }
        let chunk = chunk.clamp(1, length);
        let model = &cpu.model;
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        state.request = None;
        ensure_scratch(state, dims, chunk);
        let mut request = new_request(state, dims, length, chunk);
        let span_values: Vec<i32> = spans
            .iter()
            .flat_map(|(start, end)| [*start as i32, *end as i32])
            .collect();
        let span_buffer = state.metal.buffer(span_values.len().max(2) * 4);
        span_buffer.write_i32(0, &span_values)?;
        let span_sums = state.metal.buffer((spans.len() * dims.hidden).max(1) * 4);
        request.bytes += span_sums.byte_len() as u64 + span_buffer.byte_len() as u64;

        let rope = rope_table_params(model);
        let profile = std::env::var_os("PSIONIC_CLEF_PROFILE").is_some();
        let began = std::time::Instant::now();
        let mut device_time = 0.0f64;
        let mut last = Vec::new();
        let build_inputs = |first: usize, n: usize| -> Result<(Vec<f32>, Vec<f32>), String> {
            let piece = &tokens[first..first + n];
            let mut embedded = vec![0.0f32; n * dims.hidden];
            embedded
                .par_chunks_mut(dims.hidden)
                .zip(piece.par_iter())
                .try_for_each(|(slot, token)| -> Result<(), String> {
                    let index = token.as_u32() as usize;
                    if index >= model.descriptor.config.vocab_size {
                        return Err(format!("token {index} is outside the vocabulary"));
                    }
                    slot.copy_from_slice(&model.token_embedding.decode_row(index).map_err(err)?);
                    Ok(())
                })?;
            Ok((embedded, rope_table(&rope, first, n, dims.rotary)))
        };
        let mut prepared = Some(build_inputs(0, chunk.min(length))?);
        let mut first = 0usize;
        while first < length {
            let n = chunk.min(length - first);
            let (embedded, table) = prepared.take().ok_or("chunk inputs")?;
            {
                let scratch = state.scratch.as_ref().ok_or("scratch")?;
                scratch.x.write_f32(0, &embedded)?;
                scratch.cos_sin.write_f32(0, &table)?;
            }
            let next_first = first + n;
            let next_n = chunk.min(length.saturating_sub(next_first));
            let device_began = std::time::Instant::now();
            let (device, next) = std::thread::scope(|scope| {
                let next = (next_first < length).then(|| scope.spawn(|| build_inputs(next_first, next_n)));
                let device = (|| -> Result<(), String> {
                    let scratch = state.scratch.as_ref().ok_or("scratch")?;
                    if layer_observer.is_none() {
                        // one command buffer for the chunk's layer stack
                        let batch = state.metal.batch();
                        for (layer_index, layer) in state.layers.iter().enumerate() {
                            encode_layer(&batch, layer, scratch, &request, layer_index, dims, first, n)?;
                        }
                        batch.commit_wait()?;
                    } else {
                        for (layer_index, layer) in state.layers.iter().enumerate() {
                            let batch = state.metal.batch();
                            encode_layer(&batch, layer, scratch, &request, layer_index, dims, first, n)?;
                            batch.commit_wait()?;
                            if let Some(observer) = layer_observer.as_mut() {
                                let rows = scratch.x.read_f32(0, n * dims.hidden)?;
                                observer(layer_index, first, &rows);
                            }
                        }
                    }
                    // output norm, hidden LayerNorm, memory rows, span sums
                    let batch = state.metal.batch();
                    batch.rms_norm_f32(&scratch.x, &state.output_norm, &scratch.final_rows, n, dims.hidden, dims.eps)?;
                    batch.layer_norm_f32(
                        &scratch.final_rows,
                        Some((&state.hidden_norm_weight, &state.hidden_norm_bias)),
                        &scratch.norm_rows,
                        n,
                        dims.hidden,
                        dims.head_eps,
                    )?;
                    batch.linear_f32_ordered(
                        &scratch.norm_rows,
                        &state.memory_projection,
                        &request.memory,
                        first * dims.width,
                        n,
                        dims.width,
                        dims.hidden,
                    )?;
                    batch.span_sums(&scratch.norm_rows, &span_buffer, &span_sums, spans.len(), dims.hidden, first, n)?;
                    batch.commit_wait()?;
                    if let Some(observer) = final_observer.as_mut() {
                        let rows = scratch.final_rows.read_f32(0, n * dims.hidden)?;
                        observer(state.layers.len(), first, &rows);
                    }
                    if first + n == length {
                        last = scratch.norm_rows.read_f32((n - 1) * dims.hidden, dims.hidden)?;
                    }
                    Ok(())
                })();
                (device, next.map(|handle| handle.join()))
            });
            device?;
            device_time += device_began.elapsed().as_secs_f64();
            if let Some(next) = next {
                prepared = Some(next.map_err(|_| String::from("the embedding thread panicked"))??);
            }
            first += n;
        }
        if profile {
            eprintln!(
                "clef metal prefill: {length} tokens chunk {chunk}: total {:.1} ms, device {:.1} ms",
                began.elapsed().as_secs_f64() * 1e3,
                device_time * 1e3,
            );
        }
        let sums = if spans.is_empty() {
            Vec::new()
        } else {
            span_sums.read_f32(0, spans.len() * dims.hidden)?
        };
        let scratch_bytes = state.scratch.as_ref().map_or(0, |scratch| scratch.bytes);
        let request_device_bytes = request.bytes + scratch_bytes;
        state.request = Some(request);
        Ok(ClefDevicePrefill {
            span_sums: sums,
            last,
            request_device_bytes,
        })
    }

    /// Memory attention against the last prefill's memory rows (see
    /// [`super::ClefCudaTrunk::attend_memory`]).
    pub fn attend_memory(
        &self,
        evidence: Option<usize>,
        queries: &[f32],
        rows: usize,
        scale: f32,
    ) -> Result<Vec<f32>, String> {
        let w = self.dims.width;
        if queries.len() != rows * w {
            return Err(String::from("memory attention: query width mismatch"));
        }
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        ensure_view(state, self.dims, evidence)?;
        let length = state.request.as_ref().ok_or("no prefill on the device")?.tokens;
        ensure_head_attention(state, rows, length, w);
        let scratch = state.head_attention.as_ref().ok_or("head attention scratch")?;
        scratch.side.write_f32(0, queries)?;
        let request = state.request.as_ref().ok_or("no prefill")?;
        let memory = memory_view(request, evidence)?;
        let batch = state.metal.batch();
        batch.gemm_f32(&scratch.side, memory, &scratch.scores, rows, length, w)?;
        batch.softmax_rows_f32(&scratch.scores, rows, length, scale)?;
        batch.gemm_f32_nn(&scratch.scores, memory, &scratch.mixed, rows, w, length)?;
        batch.commit_wait()?;
        scratch.mixed.read_f32(0, rows * w)
    }

    /// The memory attention between the head's query and output projections
    /// on the device (see [`super::ClefCudaTrunk::attend_memory_projected`]).
    #[allow(clippy::too_many_arguments)]
    pub fn attend_memory_projected(
        &self,
        evidence: Option<usize>,
        key_matrix: &[f32],
        value_matrix: &[f32],
        value_bias: &[f32],
        projected: &[f32],
        rows: usize,
        heads: usize,
        scale: f32,
    ) -> Result<Option<Vec<f32>>, String> {
        let w = self.dims.width;
        if projected.len() != rows * w || value_bias.len() != w || heads == 0 || w % heads != 0 {
            return Err(String::from("memory attention: shape mismatch"));
        }
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        let key = key_matrix.as_ptr() as usize;
        let value = value_matrix.as_ptr() as usize;
        if !state.head_matrices.contains_key(&key) || !state.head_matrices.contains_key(&value) {
            return Ok(None);
        }
        ensure_view(state, self.dims, evidence)?;
        let length = state.request.as_ref().ok_or("no prefill on the device")?.tokens;
        let side_rows = rows * heads;
        ensure_head_attention(state, side_rows.max(rows), length, w);
        let scratch = state.head_attention.as_ref().ok_or("head attention scratch")?;
        scratch.query.write_f32(0, projected)?;
        scratch.bias.write_f32(0, value_bias)?;
        let request = state.request.as_ref().ok_or("no prefill")?;
        let memory = memory_view(request, evidence)?;
        let (wk, _, _) = state.head_matrices.get(&key).ok_or("W_k")?;
        let (wv, _, _) = state.head_matrices.get(&value).ok_or("W_v")?;
        let batch = state.metal.batch();
        batch.head_side(&scratch.query, wk, &scratch.side, rows, heads, w)?;
        batch.gemm_f32(&scratch.side, memory, &scratch.scores, side_rows, length, w)?;
        batch.softmax_rows_f32(&scratch.scores, side_rows, length, scale)?;
        batch.gemm_f32_nn(&scratch.scores, memory, &scratch.mixed, side_rows, w, length)?;
        batch.head_context(&scratch.mixed, wv, &scratch.bias, &scratch.context, rows, heads, w)?;
        batch.commit_wait()?;
        scratch.context.read_f32(0, rows * w).map(Some)
    }

    /// `X W^T` for `n` rows against a head matrix uploaded at load; `None`
    /// when the matrix is not resident.
    pub fn head_linear(&self, values: &[f32], input: &[f32], n: usize) -> Result<Option<Vec<f32>>, String> {
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        let Some((_, rows, columns)) = state.head_matrices.get(&(values.as_ptr() as usize)) else {
            return Ok(None);
        };
        let (rows, columns) = (*rows, *columns);
        if input.len() != n * columns {
            return Err(String::from("clef metal head: input width mismatch"));
        }
        let grow = state
            .head_io
            .as_ref()
            .is_none_or(|(cap_in, _, cap_out, _)| *cap_in < n * columns || *cap_out < n * rows);
        if grow {
            let cap_in = (n * columns).next_power_of_two().max(1 << 16);
            let cap_out = (n * rows).next_power_of_two().max(1 << 16);
            state.head_io = Some((cap_in, state.metal.buffer(cap_in * 4), cap_out, state.metal.buffer(cap_out * 4)));
        }
        let (_, input_buffer, _, output_buffer) = state.head_io.as_ref().ok_or("head io")?;
        input_buffer.write_f32(0, input)?;
        let matrix = &state.head_matrices.get(&(values.as_ptr() as usize)).ok_or("head matrix")?.0;
        let batch = state.metal.batch();
        batch.gemm_f32(input_buffer, matrix, output_buffer, n, rows, columns)?;
        batch.commit_wait()?;
        output_buffer.read_f32(0, n * rows).map(Some)
    }
}

fn rope_table(rope: &RopeTable, first: usize, n: usize, rotary: usize) -> Vec<f32> {
    rope.table(first, n, rotary)
}

fn memory_view(request: &Request, evidence: Option<usize>) -> Result<&ClefMetalBuffer, String> {
    match evidence {
        Some(layer) => request
            .normalized_memory
            .get(layer)
            .and_then(Option::as_ref)
            .ok_or_else(|| String::from("normalized memory")),
        None => Ok(&request.memory),
    }
}

/// Builds an evidence layer's `LN_m(M)` view once per request.
fn ensure_view(state: &mut DeviceState, dims: Dims, evidence: Option<usize>) -> Result<(), String> {
    let Some(layer) = evidence else {
        return Ok(());
    };
    let w = dims.width;
    let length = state.request.as_ref().ok_or("no prefill on the device")?.tokens;
    let request = state.request.as_mut().ok_or("no prefill")?;
    if request.normalized_memory.len() <= layer {
        request.normalized_memory.resize_with(layer + 1, || None);
    }
    if request.normalized_memory[layer].is_none() {
        let buffer = state.metal.buffer(length * w * 4);
        let (nw, nb) = state.evidence_norms.get(layer).ok_or("no evidence layer")?;
        let batch = state.metal.batch();
        batch.layer_norm_f32(&request.memory, Some((nw, nb)), &buffer, length, w, dims.head_eps)?;
        batch.commit_wait()?;
        request.bytes += buffer.byte_len() as u64;
        request.normalized_memory[layer] = Some(buffer);
    }
    Ok(())
}

fn ensure_head_attention(state: &mut DeviceState, rows: usize, length: usize, w: usize) {
    let grow = state
        .head_attention
        .as_ref()
        .is_none_or(|scratch| scratch.rows < rows || scratch.length < length);
    if grow {
        let capacity = rows.next_power_of_two().max(64);
        let metal = &state.metal;
        state.head_attention = Some(HeadAttention {
            rows: capacity,
            length,
            query: metal.buffer(capacity * w * 4),
            side: metal.buffer(capacity * w * 4),
            scores: metal.buffer(capacity * length * 4),
            mixed: metal.buffer(capacity * w * 4),
            context: metal.buffer(capacity * w * 4),
            bias: metal.buffer(w * 4),
        });
    }
}

fn ensure_scratch(state: &mut DeviceState, dims: Dims, chunk: usize) {
    if state.scratch.as_ref().is_some_and(|scratch| scratch.capacity >= chunk) {
        return;
    }
    state.scratch = None;
    let metal = &state.metal;
    let c = chunk;
    let wide = dims.ffn.max(dims.conv_channels).max(dims.heads * dims.head_dim * 2);
    let mid = dims.hidden.max(dims.value_heads * dims.state).max(dims.heads * dims.head_dim);
    let small = dims.value_heads.max(dims.kv_heads * dims.head_dim).max(1);
    let mut bytes = 0u64;
    let mut f32b = |len: usize| {
        let buffer = metal.buffer(len.max(1) * 4);
        bytes += buffer.byte_len() as u64;
        buffer
    };
    let x = f32b(c * dims.hidden);
    let big_a = f32b(c * wide);
    let big_b = f32b(c * wide);
    let conv_out = f32b(c * dims.conv_channels.max(1));
    let mid_a = f32b(c * mid);
    let mid_b = f32b(c * mid);
    let small_a = f32b(c * small);
    let small_b = f32b(c * small);
    let small_c = f32b(c * small);
    let small_d = f32b(c * small);
    let qn = f32b(c * (dims.key_heads * dims.state).max(1));
    let kn = f32b(c * (dims.key_heads * dims.state).max(1));
    let key_dot_query = f32b(c * dims.key_heads.max(1));
    let cos_sin = f32b(c * dims.rotary.max(2));
    let final_rows = f32b(c * dims.hidden);
    let norm_rows = f32b(c * dims.hidden);
    let act16 = metal.buffer(c * wide.max(mid) * 2);
    let query16 = metal.buffer(c * (dims.heads * dims.head_dim).max(1) * 2);
    bytes += act16.byte_len() as u64 + query16.byte_len() as u64;
    state.scratch = Some(Scratch {
        capacity: c,
        x,
        act16,
        big_a,
        big_b,
        conv_out,
        mid_a,
        mid_b,
        small_a,
        small_b,
        small_c,
        small_d,
        qn,
        kn,
        key_dot_query,
        query16,
        cos_sin,
        final_rows,
        norm_rows,
        bytes,
    });
}

fn new_request(state: &DeviceState, dims: Dims, length: usize, chunk: usize) -> Request {
    let layers = state.layers.len();
    let metal = &state.metal;
    let scores = metal.buffer(chunk * length * 4);
    let probs = metal.buffer(chunk * length * 2);
    let memory = metal.buffer(length * dims.width * 4);
    let mut request = Request {
        tokens: length,
        conv: (0..layers).map(|_| None).collect(),
        delta: (0..layers).map(|_| None).collect(),
        key_cache: (0..layers).map(|_| None).collect(),
        value_cache: (0..layers).map(|_| None).collect(),
        bytes: (scores.byte_len() + probs.byte_len() + memory.byte_len()) as u64,
        memory,
        normalized_memory: Vec::new(),
        scores,
        probs,
    };
    for (index, layer) in state.layers.iter().enumerate() {
        match &layer.mixer {
            DeviceMixer::Hybrid { .. } => {
                let conv = metal.buffer(dims.conv_channels * dims.conv_kernel.saturating_sub(1).max(1) * 4);
                let delta = metal.buffer(dims.value_heads * dims.state * dims.state * 4);
                request.bytes += (conv.byte_len() + delta.byte_len()) as u64;
                request.conv[index] = Some(conv);
                request.delta[index] = Some(delta);
            }
            DeviceMixer::Attention { .. } => {
                let len = length * dims.kv_heads * dims.head_dim;
                let key = metal.buffer(len * 2);
                let value = metal.buffer(len * 2);
                request.bytes += (key.byte_len() + value.byte_len()) as u64;
                request.key_cache[index] = Some(key);
                request.value_cache[index] = Some(value);
            }
        }
    }
    request
}

fn linear(
    batch: &ClefMetalBatch<'_>,
    weight: &DeviceWeight,
    x16: &ClefMetalBuffer,
    out: &ClefMetalBuffer,
    n: usize,
    accumulate: bool,
) -> Result<(), String> {
    batch.gemm(x16, 0, &weight.buffer, out, 0, n, weight.rows, weight.columns, accumulate)
}

#[allow(clippy::too_many_arguments)]
fn encode_layer(
    batch: &ClefMetalBatch<'_>,
    layer: &DeviceLayer,
    s: &Scratch,
    request: &Request,
    layer_index: usize,
    dims: Dims,
    first: usize,
    n: usize,
) -> Result<(), String> {
    batch.rms_norm_to_f16(&s.x, &layer.attention_norm, &s.act16, n, dims.hidden, dims.eps)?;
    match &layer.mixer {
        DeviceMixer::Hybrid { qkv, z, alpha, beta, out, conv, ssm_a, ssm_dt, ssm_norm } => {
            let conv_state = request.conv[layer_index].as_ref().ok_or("conv state")?;
            let delta_state = request.delta[layer_index].as_ref().ok_or("delta state")?;
            linear(batch, qkv, &s.act16, &s.big_a, n, false)?;
            linear(batch, z, &s.act16, &s.mid_a, n, false)?;
            linear(batch, alpha, &s.act16, &s.small_a, n, false)?;
            linear(batch, beta, &s.act16, &s.small_b, n, false)?;
            batch.conv1d_seq_silu(&s.big_a, conv_state, conv, &s.conv_out, n, dims.conv_channels, dims.conv_kernel)?;
            batch.delta_prep(
                &s.conv_out,
                &s.small_a,
                &s.small_b,
                ssm_a,
                ssm_dt,
                &s.qn,
                &s.kn,
                &s.small_c,
                &s.small_d,
                &s.key_dot_query,
                n,
                dims.key_heads,
                dims.value_heads,
                dims.state,
                dims.conv_channels,
            )?;
            batch.delta_scan(
                &s.qn,
                &s.kn,
                &s.conv_out,
                &s.small_c,
                &s.small_d,
                &s.key_dot_query,
                delta_state,
                &s.mid_b,
                n,
                dims.key_heads,
                dims.value_heads,
                dims.state,
                dims.v_head_reordered,
                dims.conv_channels,
                2 * dims.key_heads * dims.state,
            )?;
            batch.gated_norm_to_f16(&s.mid_b, &s.mid_a, ssm_norm, &s.act16, n, dims.value_heads, dims.state, dims.eps)?;
            linear(batch, out, &s.act16, &s.x, n, true)?;
        }
        DeviceMixer::Attention { query_gate, key, value, out, query_norm, key_norm } => {
            let key_cache = request.key_cache[layer_index].as_ref().ok_or("key cache")?;
            let value_cache = request.value_cache[layer_index].as_ref().ok_or("value cache")?;
            linear(batch, query_gate, &s.act16, &s.big_a, n, false)?;
            linear(batch, key, &s.act16, &s.small_a, n, false)?;
            linear(batch, value, &s.act16, &s.small_b, n, false)?;
            batch.attention_prep(
                &s.big_a,
                &s.small_a,
                &s.small_b,
                query_norm,
                key_norm,
                &s.cos_sin,
                &s.query16,
                &s.mid_a,
                key_cache,
                value_cache,
                n,
                dims.heads,
                dims.kv_heads,
                dims.head_dim,
                dims.rotary,
                first,
                dims.attention_scale,
                dims.eps,
            )?;
            batch.attention(
                &s.query16,
                key_cache,
                value_cache,
                &s.mid_b,
                &request.scores,
                &request.probs,
                n,
                dims.heads,
                dims.kv_heads,
                dims.head_dim,
                first,
            )?;
            batch.sigmoid_gate_to_f16(&s.mid_b, &s.mid_a, &s.act16, n * dims.heads * dims.head_dim)?;
            linear(batch, out, &s.act16, &s.x, n, true)?;
        }
    }
    batch.rms_norm_to_f16(&s.x, &layer.post_attention_norm, &s.act16, n, dims.hidden, dims.eps)?;
    linear(batch, &layer.gate, &s.act16, &s.big_a, n, false)?;
    linear(batch, &layer.up, &s.act16, &s.big_b, n, false)?;
    batch.silu_mul_to_f16(&s.big_a, &s.big_b, &s.act16, n * dims.ffn)?;
    linear(batch, &layer.down, &s.act16, &s.x, n, true)?;
    Ok(())
}
