//! CUDA sequence prefill for the Clef decision lane (OpenAgentsInc/openagents
//! #11195).
//!
//! A decision is all prefill, so this path runs a chunk of tokens through
//! one layer at a time instead of one token through every layer:
//!
//! - **Projections.** Each weight stays on the device in its GGUF layout
//!   (Q8_0 / Q4_K / f32) and is dequantized to f16 into one scratch matrix
//!   right before a cuBLAS GEMM against the chunk's f16 activations (f32
//!   accumulate). The residual stream is f32 and the output projections
//!   accumulate into it in the GEMM (`beta = 1`).
//! - **Gated DeltaNet.** A sequence causal conv1d carries its last three
//!   inputs; the delta rule runs one warp per (value head, value row) over
//!   the chunk's tokens in order, with the 128-wide state row in registers.
//! - **Full attention.** q/k RMSNorm, rotary and the q scale in one kernel
//!   that appends k and v to f16 caches; then scores and `P V` as strided
//!   batched cuBLAS GEMMs per KV group with a causal softmax between.
//! - **Head inputs.** After the output RMSNorm the chunk's rows go through
//!   the head's `hidden_norm` LayerNorm and `W_mem` on the device; the
//!   1024-wide memory rows stay there, span sums of `LN(H)` accumulate there,
//!   and only the span sums and the last row come back. The head's memory
//!   attention reads the device rows ([`ClefCudaTrunk::attend_memory`]).
//!
//! `token_embd` and `output` stay on the host (only rows are gathered).

use std::sync::Mutex;

use psionic_backend_cuda::clef_prefill::{ClefEvent, ClefOperand, ClefStream, ClefWeightFormat};
use psionic_backend_cuda::{CudaBackend, CudaBuffer, CudaCommandWait, CudaSubmission};
use rayon::prelude::*;

use super::{
    CpuGgufQwen35TextGenerationService, CpuQwen35LayerKind, HostMatrix, HostMatrixKind,
    QuantizationMode, TokenId, qwen35_attention_scale, qwen35_mrope_interleaved,
    qwen35_mrope_sections, qwen35_mrope_theta_base, qwen35_rope_runtime_parameters, rope_yarn,
};

/// The head parameters the device needs: `hidden_norm`, `W_mem` and the
/// evidence layers' memory norms.
pub struct ClefCudaHeadParams<'a> {
    pub hidden_norm_weight: &'a [f32],
    pub hidden_norm_bias: &'a [f32],
    /// `[width, hidden]`, row-major (`torch.nn.Linear.weight`).
    pub memory_projection: &'a [f32],
    pub width: usize,
    pub evidence_norms: Vec<(&'a [f32], &'a [f32])>,
    pub norm_epsilon: f32,
    /// The head's dense matrices `(values, rows, columns)`, uploaded once
    /// and named by their host address for [`ClefCudaTrunk::head_linear`].
    pub linear_matrices: Vec<(&'a [f32], usize, usize)>,
}

/// What a prefill returns to the head.
#[derive(Debug)]
pub struct ClefCudaPrefill {
    /// `[spans, hidden]` sums of `LN(H)` over each requested span.
    pub span_sums: Vec<f32>,
    /// `LN(H)` of the last token.
    pub last: Vec<f32>,
    /// Peak bytes this request allocated on the device (state, caches,
    /// memory rows, chunk scratch), on top of the resident weights.
    pub request_device_bytes: u64,
}

/// A per-layer observer for parity dumps: `(layer, first_token, rows)`;
/// `layer == layers` is the final normalized rows (after the output norm).
pub type ClefLayerObserver<'a> = &'a mut dyn FnMut(usize, usize, &[f32]);

struct DeviceWeight {
    buffer: CudaBuffer,
    format: ClefWeightFormat,
    rows: usize,
    columns: usize,
}

impl DeviceWeight {
    fn elements(&self) -> usize {
        self.rows * self.columns
    }
}

enum DeviceMixer {
    Hybrid {
        qkv: DeviceWeight,
        z: DeviceWeight,
        alpha: DeviceWeight,
        beta: DeviceWeight,
        out: DeviceWeight,
        conv: CudaBuffer,
        ssm_a: CudaBuffer,
        ssm_dt: CudaBuffer,
        ssm_norm: CudaBuffer,
    },
    Attention {
        query_gate: DeviceWeight,
        key: DeviceWeight,
        value: DeviceWeight,
        out: DeviceWeight,
        query_norm: CudaBuffer,
        key_norm: CudaBuffer,
    },
}

struct DeviceLayer {
    attention_norm: CudaBuffer,
    post_attention_norm: CudaBuffer,
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
    // hybrid
    conv_channels: usize,
    conv_kernel: usize,
    key_heads: usize,
    value_heads: usize,
    state: usize,
    v_head_reordered: bool,
    // attention
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    rotary: usize,
    attention_scale: f32,
    // head
    width: usize,
    head_eps: f32,
}

#[derive(Default)]
struct ProfileTimings {
    embed: f64,
    hybrid: f64,
    attention: f64,
}

/// Two f16 weight slots filled by a side stream: the next projection's
/// weights dequantize while the current GEMM runs.
struct WeightPipeline {
    stream: ClefStream,
    slots: [CudaBuffer; 2],
    dequantized: [ClefEvent; 2],
    consumed: [ClefEvent; 2],
    next: std::cell::Cell<usize>,
}

/// Activations for one chunk of up to `capacity` tokens.
struct Scratch {
    capacity: usize,
    x: CudaBuffer,
    act16: CudaBuffer,
    big_a: CudaBuffer,
    big_b: CudaBuffer,
    conv_out: CudaBuffer,
    mid_a: CudaBuffer,
    mid_b: CudaBuffer,
    small_a: CudaBuffer,
    small_b: CudaBuffer,
    small_c: CudaBuffer,
    small_d: CudaBuffer,
    qn: CudaBuffer,
    kn: CudaBuffer,
    key_dot_query: CudaBuffer,
    query16: CudaBuffer,
    /// f16 GEMM outputs when the projections accumulate in f16.
    out16: CudaBuffer,
    cos_sin: CudaBuffer,
    final_rows: CudaBuffer,
    norm_rows: CudaBuffer,
    bytes: u64,
}

/// Per-request device state.
struct Request {
    tokens: usize,
    conv: Vec<Option<CudaBuffer>>,
    delta: Vec<Option<CudaBuffer>>,
    key_cache: Vec<Option<CudaBuffer>>,
    value_cache: Vec<Option<CudaBuffer>>,
    memory: CudaBuffer,
    normalized_memory: Vec<Option<CudaBuffer>>,
    /// Attention scores (f32) and probabilities (f16) for up to
    /// `score_capacity` elements: a KV group's heads at once when they fit
    /// [`ATTENTION_SCORE_BUDGET_BYTES`], fewer otherwise.
    scores: CudaBuffer,
    probs: CudaBuffer,
    score_capacity: usize,
    bytes: u64,
}

struct DeviceState {
    backend: CudaBackend,
    layers: Vec<DeviceLayer>,
    output_norm: CudaBuffer,
    hidden_norm_weight: CudaBuffer,
    hidden_norm_bias: CudaBuffer,
    memory_projection: CudaBuffer,
    evidence_norms: Vec<(CudaBuffer, CudaBuffer)>,
    weight_scratch: WeightPipeline,
    scratch: Option<Scratch>,
    request: Option<Request>,
    attention_scratch: Option<(usize, CudaBuffer, CudaBuffer)>,
    /// Head matrices by host address: `(buffer, rows, columns)`.
    head_matrices: std::collections::HashMap<usize, (CudaBuffer, usize, usize)>,
    /// Staging for head products: input and output capacity in floats.
    head_io: Option<(usize, CudaBuffer, usize, CudaBuffer)>,
    /// Buffers for [`ClefCudaTrunk::attend_memory_projected`].
    projected_attention: Option<ProjectedAttention>,
    weight_bytes: u64,
}

/// Device buffers for the head's memory attention on projected queries:
/// capacity in side rows (queries x heads) and memory length.
struct ProjectedAttention {
    rows: usize,
    length: usize,
    query: CudaBuffer,
    side: CudaBuffer,
    scores: CudaBuffer,
    mixed: CudaBuffer,
    context: CudaBuffer,
    bias: CudaBuffer,
}

// SAFETY: the CUDA handles inside are process-wide runtime objects (device
// pointers, streams, the cuBLAS handle) that any host thread may use once it
// sets the device, which every launch does. All access goes through the
// `Mutex` in `ClefCudaTrunk`, so no two threads touch them at once.
unsafe impl Send for DeviceState {}

/// The Clef trunk on one CUDA device.
pub struct ClefCudaTrunk {
    device: Mutex<DeviceState>,
    dims: Dims,
    compute_16f: bool,
    device_name: String,
}

impl std::fmt::Debug for ClefCudaTrunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClefCudaTrunk")
            .field("device", &self.device_name)
            .field("compute_16f", &self.compute_16f)
            .finish_non_exhaustive()
    }
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The bytes of an f32 slice (native endianness, as the device reads it).
fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and any byte pattern is a valid u8; the
    // slice covers exactly the values' memory.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) }
}

fn upload_f32(backend: &mut CudaBackend, values: &[f32]) -> Result<CudaBuffer, String> {
    let mut buffer = backend.f32_buffer(values.len().max(1)).map_err(err)?;
    if !values.is_empty() {
        buffer.write_bytes_at_offset(0, f32_bytes(values)).map_err(err)?;
    }
    Ok(buffer)
}

fn upload_matrix(backend: &mut CudaBackend, matrix: &HostMatrix, name: &str) -> Result<DeviceWeight, String> {
    match &matrix.kind {
        HostMatrixKind::Dense(dense) => Ok(DeviceWeight {
            buffer: upload_f32(backend, &dense.values)?,
            format: ClefWeightFormat::F32,
            rows: dense.rows,
            columns: dense.columns,
        }),
        HostMatrixKind::Quantized(quantized) => {
            let format = match quantized.mode {
                QuantizationMode::GgmlQ8_0 => ClefWeightFormat::Q8_0,
                QuantizationMode::GgmlQ4K => ClefWeightFormat::Q4K,
                other => {
                    return Err(format!(
                        "clef cuda prefill: `{name}` is {other:?}; this lane dequantizes Q8_0, Q4_K and f32"
                    ));
                }
            };
            let elements = quantized.rows * quantized.columns;
            let bytes_len = format
                .byte_len(elements)
                .filter(|len| *len == quantized.rows * quantized.row_byte_len)
                .ok_or_else(|| format!("clef cuda prefill: `{name}` has an unexpected {format:?} size"))?;
            let bytes = quantized.storage.read_range(0, bytes_len).map_err(err)?;
            Ok(DeviceWeight {
                buffer: backend.byte_buffer(bytes).map_err(err)?,
                format,
                rows: quantized.rows,
                columns: quantized.columns,
            })
        }
    }
}

fn weight_bytes(weight: &DeviceWeight) -> u64 {
    weight.buffer.byte_len() as u64
}

impl ClefCudaTrunk {
    /// Uploads the trunk of a loaded CPU qwen35 model (the CPU copy keeps
    /// `token_embd` and `output`) and the head's device-side parameters.
    pub fn load(
        cpu: &CpuGgufQwen35TextGenerationService,
        head: &ClefCudaHeadParams<'_>,
        compute_16f: bool,
    ) -> Result<Self, String> {
        let mut backend = CudaBackend::new();
        let device_name = backend
            .selected_device()
            .map(|device| device.device_name.clone().unwrap_or_default())
            .ok_or_else(|| String::from("no CUDA device is available"))?;
        let model = &cpu.model;
        let config = &model.descriptor.config;
        let metadata = &model.family_metadata;
        let mut layers = Vec::with_capacity(model.layers.len());
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
        let mut weight_total = 0u64;
        let mut largest = 0usize;
        for (index, layer) in model.layers.iter().enumerate() {
            let name = |part: &str| format!("blk.{index}.{part}");
            let gate = upload_matrix(&mut backend, &layer.ffn_gate_up.parts[0], &name("ffn_gate"))?;
            let up = upload_matrix(&mut backend, &layer.ffn_gate_up.parts[1], &name("ffn_up"))?;
            let down = upload_matrix(&mut backend, &layer.ffn_down, &name("ffn_down"))?;
            dims.ffn = gate.rows;
            let mixer = match &layer.kind {
                CpuQwen35LayerKind::Hybrid(hybrid) => {
                    let parts = &hybrid.qkv_gate_alpha_beta.parts;
                    let qkv = upload_matrix(&mut backend, &parts[0], &name("attn_qkv"))?;
                    let z = upload_matrix(&mut backend, &parts[1], &name("attn_gate"))?;
                    let alpha = upload_matrix(&mut backend, &parts[2], &name("ssm_alpha"))?;
                    let beta = upload_matrix(&mut backend, &parts[3], &name("ssm_beta"))?;
                    let out = upload_matrix(&mut backend, &hybrid.ssm_out, &name("ssm_out"))?;
                    dims.conv_channels = qkv.rows;
                    dims.conv_kernel = hybrid.conv_kernel;
                    dims.key_heads = hybrid.group_count;
                    dims.value_heads = hybrid.time_step_rank;
                    dims.state = hybrid.state_size;
                    dims.v_head_reordered = hybrid.v_head_reordered;
                    if hybrid.ssm_conv1d.rows != qkv.rows || hybrid.ssm_conv1d.columns != hybrid.conv_kernel {
                        return Err(format!("{}: conv1d shape does not match attn_qkv", name("ssm_conv1d")));
                    }
                    if hybrid.conv_kernel > 4 || !matches!(hybrid.state_size, 64 | 128 | 256) {
                        return Err(format!(
                            "{}: conv kernel {} / state {} are outside this lane",
                            name("ssm"),
                            hybrid.conv_kernel,
                            hybrid.state_size
                        ));
                    }
                    DeviceMixer::Hybrid {
                        conv: upload_f32(&mut backend, &hybrid.ssm_conv1d.values)?,
                        ssm_a: upload_f32(&mut backend, &hybrid.ssm_a)?,
                        ssm_dt: upload_f32(&mut backend, &hybrid.ssm_dt)?,
                        ssm_norm: upload_f32(&mut backend, &hybrid.ssm_norm)?,
                        qkv,
                        z,
                        alpha,
                        beta,
                        out,
                    }
                }
                CpuQwen35LayerKind::FullAttention(attention) => {
                    let parts = &attention.qkv.parts;
                    let query_gate = upload_matrix(&mut backend, &parts[0], &name("attn_q"))?;
                    let key = upload_matrix(&mut backend, &parts[1], &name("attn_k"))?;
                    let value = upload_matrix(&mut backend, &parts[2], &name("attn_v"))?;
                    let out = upload_matrix(&mut backend, &attention.output, &name("attn_output"))?;
                    dims.kv_heads = attention.kv_width / dims.head_dim.max(1);
                    if query_gate.rows != dims.heads * dims.head_dim * 2 {
                        return Err(format!("{}: expected an interleaved query/gate projection", name("attn_q")));
                    }
                    DeviceMixer::Attention {
                        query_norm: upload_f32(&mut backend, &attention.query_norm)?,
                        key_norm: upload_f32(&mut backend, &attention.key_norm)?,
                        query_gate,
                        key,
                        value,
                        out,
                    }
                }
            };
            let mut layer_weights: Vec<&DeviceWeight> = vec![&gate, &up, &down];
            match &mixer {
                DeviceMixer::Hybrid { qkv, z, alpha, beta, out, .. } => {
                    layer_weights.extend([qkv, z, alpha, beta, out]);
                }
                DeviceMixer::Attention { query_gate, key, value, out, .. } => {
                    layer_weights.extend([query_gate, key, value, out]);
                }
            }
            for weight in &layer_weights {
                weight_total += weight_bytes(weight);
                // the f16 slots only serve weights the fused kernel does not take
                let fused = knobs().fused == Fused::Always
                    && match weight.format {
                        ClefWeightFormat::Q8_0 => weight.columns % 64 == 0,
                        ClefWeightFormat::Q4K => weight.columns % 256 == 0,
                        ClefWeightFormat::F32 => false,
                    }
                    && weight.rows % 2 == 0;
                if !fused {
                    largest = largest.max(weight.elements());
                }
            }
            layers.push(DeviceLayer {
                attention_norm: upload_f32(&mut backend, &layer.attention_norm)?,
                post_attention_norm: upload_f32(&mut backend, &layer.post_attention_norm)?,
                gate,
                up,
                down,
                mixer,
            });
        }
        if dims.heads * dims.head_dim > 1024 * 64 || dims.head_dim > 1024 {
            return Err(String::from("clef cuda prefill: attention head dim above 1024"));
        }
        if head.memory_projection.len() != head.width * dims.hidden {
            return Err(String::from("clef cuda prefill: W_mem does not match the hidden size"));
        }
        let weight_scratch = WeightPipeline {
            stream: ClefStream::new().map_err(err)?,
            slots: [
                backend.f16_buffer(largest.max(1)).map_err(err)?,
                backend.f16_buffer(largest.max(1)).map_err(err)?,
            ],
            dequantized: [ClefEvent::new().map_err(err)?, ClefEvent::new().map_err(err)?],
            consumed: [ClefEvent::new().map_err(err)?, ClefEvent::new().map_err(err)?],
            next: std::cell::Cell::new(0),
        };
        let mut head_matrices = std::collections::HashMap::new();
        let mut head_bytes = 0u64;
        for (values, rows, columns) in &head.linear_matrices {
            if values.len() != rows * columns {
                return Err(String::from("clef cuda head: matrix shape mismatch"));
            }
            let buffer = upload_f32(&mut backend, values)?;
            head_bytes += buffer.byte_len() as u64;
            head_matrices.insert(values.as_ptr() as usize, (buffer, *rows, *columns));
        }
        let state = DeviceState {
            output_norm: upload_f32(&mut backend, &model.output_norm)?,
            hidden_norm_weight: upload_f32(&mut backend, head.hidden_norm_weight)?,
            hidden_norm_bias: upload_f32(&mut backend, head.hidden_norm_bias)?,
            memory_projection: upload_f32(&mut backend, head.memory_projection)?,
            evidence_norms: head
                .evidence_norms
                .iter()
                .map(|(w, b)| Ok((upload_f32(&mut backend, w)?, upload_f32(&mut backend, b)?)))
                .collect::<Result<_, String>>()?,
            weight_bytes: weight_total + (largest as u64) * 4 + head_bytes,
            head_matrices,
            head_io: None,
            projected_attention: None,
            weight_scratch,
            layers,
            scratch: None,
            request: None,
            attention_scratch: None,
            backend,
        };
        Ok(Self {
            device: Mutex::new(state),
            dims,
            compute_16f,
            device_name,
        })
    }

    /// The CUDA device name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Bytes of resident trunk weights (plus the f16 weight scratch).
    #[must_use]
    pub fn weight_device_bytes(&self) -> u64 {
        self.device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .weight_bytes
    }

    /// Runs `tokens` through the trunk in chunks of `chunk`, leaving the
    /// memory rows on the device for [`Self::attend_memory`] and returning
    /// the span sums of `LN(H)` over `spans` (`[start, end)`) and the last
    /// `LN(H)` row. The observers (parity dumps; each forces a device sync)
    /// see every layer's residual rows and the final normalized rows.
    pub fn prefill(
        &self,
        cpu: &CpuGgufQwen35TextGenerationService,
        tokens: &[TokenId],
        spans: &[(usize, usize)],
        chunk: usize,
        mut layer_observer: Option<ClefLayerObserver<'_>>,
        mut final_observer: Option<ClefLayerObserver<'_>>,
    ) -> Result<ClefCudaPrefill, String> {
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
        state.attention_scratch = None;
        ensure_scratch(state, dims, chunk)?;
        let mut request = new_request(state, dims, length, chunk)?;

        // spans and their sums
        let span_values: Vec<i32> = spans
            .iter()
            .flat_map(|(start, end)| [*start as i32, *end as i32])
            .collect();
        let mut span_buffer = state.backend.i32_buffer(span_values.len().max(2)).map_err(err)?;
        if !span_values.is_empty() {
            let mut padded = span_values.clone();
            padded.resize(span_values.len().max(2), 0);
            span_buffer.write_i32(&padded).map_err(err)?;
        }
        let span_sums = state.backend.f32_buffer((spans.len() * dims.hidden).max(1)).map_err(err)?;
        {
            let mut submission = state.backend.begin_submission().map_err(err)?;
            submission.fill_buffer(&span_sums, 0).map_err(err)?;
            submission.commit(CudaCommandWait::Completed).map_err(err)?;
        }
        request.bytes += span_sums.byte_len() as u64 + span_buffer.byte_len() as u64;

        let rope = rope_table_params(model);
        let profile = std::env::var_os("PSIONIC_CLEF_PROFILE").is_some();
        let mut timings = ProfileTimings::default();
        let began = std::time::Instant::now();
        let mut last = Vec::new();
        // The host builds a chunk's embedding rows and rotary table (token
        // rows decoded from the host `token_embd`) on a second thread while
        // the device runs the chunk before it.
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
            Ok((embedded, rope.table(first, n, dims.rotary)))
        };
        let mut prepared = Some(build_inputs(0, chunk.min(length))?);
        let mut first = 0usize;
        while first < length {
            let chunk_began = std::time::Instant::now();
            let n = chunk.min(length - first);
            let (embedded, table) = prepared.take().ok_or("chunk inputs")?;
            let scratch = state.scratch.as_mut().ok_or("scratch")?;
            scratch.x.write_bytes_at_offset(0, f32_bytes(&embedded)).map_err(err)?;
            scratch.cos_sin.write_bytes_at_offset(0, f32_bytes(&table)).map_err(err)?;
            timings.embed += chunk_began.elapsed().as_secs_f64();
            let next_first = first + n;
            let next_n = chunk.min(length.saturating_sub(next_first));
            let (device, next) = std::thread::scope(|scope| {
                let next = (next_first < length)
                    .then(|| scope.spawn(|| build_inputs(next_first, next_n)));
                let device = (|| -> Result<(), String> {
                    if layer_observer.is_none() && !profile {
                        // one submission for the whole chunk
                        let mut submission = state.backend.begin_submission().map_err(err)?;
                        for layer_index in 0..state.layers.len() {
                            encode_layer(
                                &mut submission,
                                &state.layers[layer_index],
                                state.scratch.as_ref().ok_or("scratch")?,
                                &state.weight_scratch,
                                &request,
                                layer_index,
                                dims,
                                first,
                                n,
                                self.compute_16f,
                            )?;
                        }
                        submission.commit(CudaCommandWait::Completed).map_err(err)?;
                    }
                    for layer_index in 0..state.layers.len() {
                        if layer_observer.is_none() && !profile {
                            break;
                        }
                        let layer_began = std::time::Instant::now();
                        {
                            let mut submission = state.backend.begin_submission().map_err(err)?;
                            encode_layer(
                                &mut submission,
                                state.layers.get(layer_index).ok_or("layer")?,
                                state.scratch.as_ref().ok_or("scratch")?,
                                &state.weight_scratch,
                                &request,
                                layer_index,
                                dims,
                                first,
                                n,
                                self.compute_16f,
                            )?;
                            submission.commit(CudaCommandWait::Completed).map_err(err)?;
                        }
                        if profile {
                            let seconds = layer_began.elapsed().as_secs_f64();
                            match state.layers[layer_index].mixer {
                                DeviceMixer::Hybrid { .. } => timings.hybrid += seconds,
                                DeviceMixer::Attention { .. } => timings.attention += seconds,
                            }
                        }
                        if let Some(observer) = layer_observer.as_mut() {
                            let rows = state
                                .scratch
                                .as_ref()
                                .ok_or("scratch")?
                                .x
                                .read_f32_at_offset(0, n * dims.hidden)
                                .map_err(err)?;
                            observer(layer_index, first, &rows);
                        }
                    }

                    // output norm, hidden LayerNorm, memory rows, span sums
                    {
                        let scratch = state.scratch.as_ref().ok_or("scratch")?;
                        let mut submission = state.backend.begin_submission().map_err(err)?;
                        submission
                            .clef_rms_norm_f32(&scratch.x, &state.output_norm, &scratch.final_rows, n, dims.hidden, dims.eps)
                            .map_err(err)?;
                        submission
                            .clef_layer_norm_f32(
                                &scratch.final_rows,
                                Some((&state.hidden_norm_weight, &state.hidden_norm_bias)),
                                &scratch.norm_rows,
                                n,
                                dims.hidden,
                                dims.head_eps,
                            )
                            .map_err(err)?;
                        // fixed order: a memory row does not depend on its chunk
                        submission
                            .clef_linear_f32_ordered(
                                &scratch.norm_rows,
                                &state.memory_projection,
                                &request.memory,
                                first * dims.width,
                                n,
                                dims.width,
                                dims.hidden,
                            )
                            .map_err(err)?;
                        submission
                            .clef_span_sums(&scratch.norm_rows, &span_buffer, &span_sums, spans.len(), dims.hidden, first, n)
                            .map_err(err)?;
                        submission.commit(CudaCommandWait::Completed).map_err(err)?;
                        if let Some(observer) = final_observer.as_mut() {
                            let rows = scratch.final_rows.read_f32_at_offset(0, n * dims.hidden).map_err(err)?;
                            observer(state.layers.len(), first, &rows);
                        }
                        if first + n == length {
                            last = scratch
                                .norm_rows
                                .read_f32_at_offset((n - 1) * dims.hidden, dims.hidden)
                                .map_err(err)?;
                        }
                    }
                    Ok(())
                })();
                (device, next.map(|handle| handle.join()))
            });
            device?;
            if let Some(next) = next {
                prepared = Some(next.map_err(|_| String::from("the embedding thread panicked"))??);
            }
            first += n;
        }
        if profile {
            eprintln!(
                "clef cuda prefill: {length} tokens chunk {chunk}: total {:.1} ms, embed+rope {:.1} ms, hybrid layers {:.1} ms, attention layers {:.1} ms",
                began.elapsed().as_secs_f64() * 1e3,
                timings.embed * 1e3,
                timings.hybrid * 1e3,
                timings.attention * 1e3,
            );
        }
        let sums = if spans.is_empty() {
            Vec::new()
        } else {
            span_sums.read_f32_at_offset(0, spans.len() * dims.hidden).map_err(err)?
        };
        let scratch_bytes = state.scratch.as_ref().map_or(0, |scratch| scratch.bytes);
        let request_device_bytes = request.bytes + scratch_bytes;
        state.request = Some(request);
        Ok(ClefCudaPrefill {
            span_sums: sums,
            last,
            request_device_bytes,
        })
    }

    /// Memory attention against the last prefill's memory rows: for each of
    /// `rows` query vectors (`rows x width`), `sum_t softmax_t(scale * u .
    /// m_t) m_t`, where `m` is the raw memory (`evidence = None`) or an
    /// evidence layer's `LN_m(M)`.
    pub fn attend_memory(
        &self,
        evidence: Option<usize>,
        queries: &[f32],
        rows: usize,
        scale: f32,
    ) -> Result<Vec<f32>, String> {
        let dims = self.dims;
        let w = dims.width;
        if queries.len() != rows * w {
            return Err(String::from("memory attention: query width mismatch"));
        }
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        let length = state.request.as_ref().ok_or("no prefill on the device")?.tokens;
        // the view
        if let Some(layer) = evidence {
            let request = state.request.as_mut().ok_or("no prefill")?;
            if request.normalized_memory.len() <= layer {
                request.normalized_memory.resize_with(layer + 1, || None);
            }
            if request.normalized_memory[layer].is_none() {
                let buffer = state.backend.f32_buffer(length * w).map_err(err)?;
                let (nw, nb) = state.evidence_norms.get(layer).ok_or("no evidence layer")?;
                let mut submission = state.backend.begin_submission().map_err(err)?;
                submission
                    .clef_layer_norm_f32(&request.memory, Some((nw, nb)), &buffer, length, w, dims.head_eps)
                    .map_err(err)?;
                submission.commit(CudaCommandWait::Completed).map_err(err)?;
                request.bytes += buffer.byte_len() as u64;
                request.normalized_memory[layer] = Some(buffer);
            }
        }
        // query and score buffers
        let need = rows.max(1);
        let grow = state
            .attention_scratch
            .as_ref()
            .is_none_or(|(capacity, _, _)| *capacity < need);
        if grow {
            let capacity = need.next_power_of_two().max(64);
            let query_buffer = state.backend.f32_buffer(capacity * w).map_err(err)?;
            let score_buffer = state.backend.f32_buffer(capacity * length).map_err(err)?;
            state.attention_scratch = Some((capacity, query_buffer, score_buffer));
        }
        let (_, query_buffer, score_buffer) = state.attention_scratch.as_mut().ok_or("attention scratch")?;
        query_buffer.write_bytes_at_offset(0, f32_bytes(queries)).map_err(err)?;
        let request = state.request.as_ref().ok_or("no prefill")?;
        let memory = match evidence {
            Some(layer) => request.normalized_memory[layer].as_ref().ok_or("normalized memory")?,
            None => &request.memory,
        };
        let mixed = state.backend.f32_buffer(rows * w).map_err(err)?;
        let mut submission = state.backend.begin_submission().map_err(err)?;
        submission
            .clef_linear(
                ClefOperand::f32(query_buffer, 0),
                ClefOperand::f32(memory, 0),
                ClefOperand::f32(score_buffer, 0),
                rows,
                length,
                w,
                false,
                false,
            )
            .map_err(err)?;
        submission
            .clef_softmax_rows_f32(score_buffer, rows, length, scale)
            .map_err(err)?;
        // mixed^T [w, rows] = M^T [w, L] . P^T [L, rows]
        submission
            .clef_gemm_strided_batched(
                false,
                false,
                w,
                rows,
                length,
                ClefOperand::f32(memory, 0),
                w,
                0,
                length * w,
                ClefOperand::f32(score_buffer, 0),
                length,
                0,
                rows * length,
                ClefOperand::f32(&mixed, 0),
                w,
                0,
                rows * w,
                1,
            )
            .map_err(err)?;
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        mixed.read_f32_at_offset(0, rows * w).map_err(err)
    }
}

impl ClefCudaTrunk {
    /// The whole memory attention between the query and output
    /// projections, on the device: `u[i, h] = W_k,h^T q[i, h]`, the
    /// attention of each `u` over the memory view, and `W_v,h z[i, h] + b_v`
    /// for `rows` projected queries (`rows x width`). `W_k` and `W_v` are
    /// head matrices uploaded at load, named by their host values; `None`
    /// when they are not resident (the caller runs the host path).
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
        let dims = self.dims;
        let w = dims.width;
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
        let length = state.request.as_ref().ok_or("no prefill on the device")?.tokens;
        if let Some(layer) = evidence {
            let request = state.request.as_mut().ok_or("no prefill")?;
            if request.normalized_memory.len() <= layer {
                request.normalized_memory.resize_with(layer + 1, || None);
            }
            if request.normalized_memory[layer].is_none() {
                let buffer = state.backend.f32_buffer(length * w).map_err(err)?;
                let (nw, nb) = state.evidence_norms.get(layer).ok_or("no evidence layer")?;
                let mut submission = state.backend.begin_submission().map_err(err)?;
                submission
                    .clef_layer_norm_f32(&request.memory, Some((nw, nb)), &buffer, length, w, dims.head_eps)
                    .map_err(err)?;
                submission.commit(CudaCommandWait::Completed).map_err(err)?;
                request.bytes += buffer.byte_len() as u64;
                request.normalized_memory[layer] = Some(buffer);
            }
        }
        let side_rows = rows * heads;
        let grow = state
            .projected_attention
            .as_ref()
            .is_none_or(|scratch| scratch.rows < side_rows || scratch.length < length);
        if grow {
            let capacity = side_rows.next_power_of_two().max(64);
            let query_capacity = rows.next_power_of_two().max(16);
            state.projected_attention = Some(ProjectedAttention {
                rows: capacity,
                length,
                query: state.backend.f32_buffer(query_capacity.max(capacity / heads) * w).map_err(err)?,
                side: state.backend.f32_buffer(capacity * w).map_err(err)?,
                scores: state.backend.f32_buffer(capacity * length).map_err(err)?,
                mixed: state.backend.f32_buffer(capacity * w).map_err(err)?,
                context: state.backend.f32_buffer(query_capacity.max(capacity / heads) * w).map_err(err)?,
                bias: state.backend.f32_buffer(w).map_err(err)?,
            });
        }
        let scratch = state.projected_attention.as_mut().ok_or("projected attention scratch")?;
        scratch.query.write_bytes_at_offset(0, f32_bytes(projected)).map_err(err)?;
        scratch.bias.write_bytes_at_offset(0, f32_bytes(value_bias)).map_err(err)?;
        let scratch = state.projected_attention.as_ref().ok_or("projected attention scratch")?;
        let request = state.request.as_ref().ok_or("no prefill")?;
        let memory = match evidence {
            Some(layer) => request.normalized_memory[layer].as_ref().ok_or("normalized memory")?,
            None => &request.memory,
        };
        let (wk, _, _) = state.head_matrices.get(&key).ok_or("W_k")?;
        let (wv, _, _) = state.head_matrices.get(&value).ok_or("W_v")?;
        let mut submission = state.backend.begin_submission().map_err(err)?;
        submission
            .clef_head_side(&scratch.query, wk, &scratch.side, rows, heads, w)
            .map_err(err)?;
        submission
            .clef_linear(
                ClefOperand::f32(&scratch.side, 0),
                ClefOperand::f32(memory, 0),
                ClefOperand::f32(&scratch.scores, 0),
                side_rows,
                length,
                w,
                false,
                false,
            )
            .map_err(err)?;
        submission
            .clef_softmax_rows_f32(&scratch.scores, side_rows, length, scale)
            .map_err(err)?;
        // mixed^T [w, side_rows] = M^T [w, L] . P^T [L, side_rows]
        submission
            .clef_gemm_strided_batched(
                false,
                false,
                w,
                side_rows,
                length,
                ClefOperand::f32(memory, 0),
                w,
                0,
                length * w,
                ClefOperand::f32(&scratch.scores, 0),
                length,
                0,
                side_rows * length,
                ClefOperand::f32(&scratch.mixed, 0),
                w,
                0,
                side_rows * w,
                1,
            )
            .map_err(err)?;
        submission
            .clef_head_context(&scratch.mixed, wv, &scratch.bias, &scratch.context, rows, heads, w)
            .map_err(err)?;
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        scratch.context.read_f32_at_offset(0, rows * w).map(Some).map_err(err)
    }

    /// `X W^T` for `n` rows of `X` against a head matrix uploaded at load
    /// (named by the address of its host values), in f32. `None` when the
    /// matrix is not resident (the caller multiplies on the CPU).
    pub fn head_linear(&self, values: &[f32], input: &[f32], n: usize) -> Result<Option<Vec<f32>>, String> {
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        let Some((matrix, rows, columns)) = state.head_matrices.get(&(values.as_ptr() as usize)) else {
            return Ok(None);
        };
        let (rows, columns) = (*rows, *columns);
        if input.len() != n * columns {
            return Err(String::from("clef cuda head: input width mismatch"));
        }
        let grow = state
            .head_io
            .as_ref()
            .is_none_or(|(cap_in, _, cap_out, _)| *cap_in < n * columns || *cap_out < n * rows);
        if grow {
            let cap_in = (n * columns).next_power_of_two().max(1 << 16);
            let cap_out = (n * rows).next_power_of_two().max(1 << 16);
            let input_buffer = state.backend.f32_buffer(cap_in).map_err(err)?;
            let output_buffer = state.backend.f32_buffer(cap_out).map_err(err)?;
            state.head_io = Some((cap_in, input_buffer, cap_out, output_buffer));
        }
        let (_, input_buffer, _, output_buffer) = state.head_io.as_mut().ok_or("head io")?;
        input_buffer.write_bytes_at_offset(0, f32_bytes(input)).map_err(err)?;
        let matrix = &state.head_matrices.get(&(values.as_ptr() as usize)).ok_or("head matrix")?.0;
        let mut submission = state.backend.begin_submission().map_err(err)?;
        submission
            .clef_linear(
                ClefOperand::f32(input_buffer, 0),
                ClefOperand::f32(matrix, 0),
                ClefOperand::f32(output_buffer, 0),
                n,
                rows,
                columns,
                false,
                false,
            )
            .map_err(err)?;
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        output_buffer.read_f32_at_offset(0, n * rows).map(Some).map_err(err)
    }
}

fn ensure_scratch(state: &mut DeviceState, dims: Dims, chunk: usize) -> Result<(), String> {
    if state.scratch.as_ref().is_some_and(|scratch| scratch.capacity >= chunk) {
        return Ok(());
    }
    state.scratch = None;
    let backend = &mut state.backend;
    let c = chunk;
    let wide = dims.ffn.max(dims.conv_channels).max(dims.heads * dims.head_dim * 2);
    let mid = dims.hidden.max(dims.value_heads * dims.state).max(dims.heads * dims.head_dim);
    let small = dims.value_heads.max(dims.kv_heads * dims.head_dim).max(1);
    let mut bytes = 0u64;
    let mut f32b = |backend: &mut CudaBackend, len: usize| -> Result<CudaBuffer, String> {
        let buffer = backend.f32_buffer(len.max(1)).map_err(err)?;
        bytes += buffer.byte_len() as u64;
        Ok(buffer)
    };
    let x = f32b(backend, c * dims.hidden)?;
    let big_a = f32b(backend, c * wide)?;
    let big_b = f32b(backend, c * wide)?;
    let conv_out = f32b(backend, c * dims.conv_channels.max(1))?;
    let mid_a = f32b(backend, c * mid)?;
    let mid_b = f32b(backend, c * mid)?;
    let small_a = f32b(backend, c * small)?;
    let small_b = f32b(backend, c * small)?;
    let small_c = f32b(backend, c * small)?;
    let small_d = f32b(backend, c * small)?;
    let qn = f32b(backend, c * (dims.key_heads * dims.state).max(1))?;
    let kn = f32b(backend, c * (dims.key_heads * dims.state).max(1))?;
    let key_dot_query = f32b(backend, c * dims.key_heads.max(1))?;
    let cos_sin = f32b(backend, c * dims.rotary.max(2))?;
    let final_rows = f32b(backend, c * dims.hidden)?;
    let norm_rows = f32b(backend, c * dims.hidden)?;
    let act16 = backend.f16_buffer(c * wide.max(mid)).map_err(err)?;
    let query16 = backend.f16_buffer(c * (dims.heads * dims.head_dim).max(1)).map_err(err)?;
    let out16 = backend.f16_buffer(c * wide.max(mid)).map_err(err)?;
    bytes += act16.byte_len() as u64 + query16.byte_len() as u64 + out16.byte_len() as u64;
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
        out16,
        cos_sin,
        final_rows,
        norm_rows,
        bytes,
    });
    Ok(())
}

fn new_request(
    state: &mut DeviceState,
    dims: Dims,
    length: usize,
    chunk: usize,
) -> Result<Request, String> {
    let layers = state.layers.len();
    let group = dims.heads / dims.kv_heads.max(1);
    let widest = chunk * length;
    let heads_at_once = (ATTENTION_SCORE_BUDGET_BYTES / (widest * 6).max(1)).clamp(1, group.max(1));
    let score_capacity = widest * heads_at_once;
    let scores = state.backend.f32_buffer(score_capacity.max(1)).map_err(err)?;
    let probs = state.backend.f16_buffer(score_capacity.max(1)).map_err(err)?;
    let mut request = Request {
        tokens: length,
        conv: (0..layers).map(|_| None).collect(),
        delta: (0..layers).map(|_| None).collect(),
        key_cache: (0..layers).map(|_| None).collect(),
        value_cache: (0..layers).map(|_| None).collect(),
        memory: state.backend.f32_buffer(length * dims.width).map_err(err)?,
        normalized_memory: Vec::new(),
        bytes: scores.byte_len() as u64 + probs.byte_len() as u64,
        scores,
        probs,
        score_capacity,
    };
    request.bytes += request.memory.byte_len() as u64;
    let mut submission = state.backend.begin_submission().map_err(err)?;
    for (index, layer) in state.layers.iter().enumerate() {
        match &layer.mixer {
            DeviceMixer::Hybrid { .. } => {
                let conv = state
                    .backend
                    .f32_buffer(dims.conv_channels * dims.conv_kernel.saturating_sub(1).max(1))
                    .map_err(err)?;
                let delta = state
                    .backend
                    .f32_buffer(dims.value_heads * dims.state * dims.state)
                    .map_err(err)?;
                submission.fill_buffer(&conv, 0).map_err(err)?;
                submission.fill_buffer(&delta, 0).map_err(err)?;
                request.bytes += conv.byte_len() as u64 + delta.byte_len() as u64;
                request.conv[index] = Some(conv);
                request.delta[index] = Some(delta);
            }
            DeviceMixer::Attention { .. } => {
                let len = length * dims.kv_heads * dims.head_dim;
                let key = state.backend.f16_buffer(len).map_err(err)?;
                let value = state.backend.f16_buffer(len).map_err(err)?;
                request.bytes += key.byte_len() as u64 + value.byte_len() as u64;
                request.key_cache[index] = Some(key);
                request.value_cache[index] = Some(value);
            }
        }
    }
    submission.commit(CudaCommandWait::Completed).map_err(err)?;
    Ok(request)
}

/// Dequantize a weight into the f16 scratch and multiply:
/// `out (+)= x16 · W^T`. With `compute_16f` the GEMM accumulates in f16
/// into `out16` and a conversion adds it to `out`.
#[allow(clippy::too_many_arguments)]
fn linear(
    submission: &mut CudaSubmission,
    weight: &DeviceWeight,
    pipeline: &WeightPipeline,
    x16: &CudaBuffer,
    out: &CudaBuffer,
    out16: &CudaBuffer,
    n: usize,
    accumulate: bool,
    compute_16f: bool,
) -> Result<(), String> {
    let knob = knobs();
    if knob.fused.takes(n) && !knob.skip_gemm {
        // straight from the GGUF layout; f16 accumulation in short spans
        // promoted to f32, or f32 tensor accumulation in strict mode
        let segment = if compute_16f { knob.segment } else { 0 };
        if submission
            .clef_fused_linear(x16, &weight.buffer, weight.format, out, n, weight.rows, weight.columns, segment, accumulate)
            .map_err(err)?
        {
            return Ok(());
        }
    }
    let slot = pipeline.next.get();
    pipeline.next.set(slot ^ 1);
    let weight_scratch = &pipeline.slots[slot];
    if !knobs().skip_dequant {
        // dequantize on the side stream once the GEMM that last read this
        // slot is done; the GEMM below waits for the dequantization
        pipeline.stream.wait(&pipeline.consumed[slot]).map_err(err)?;
        pipeline
            .stream
            .dequantize_to_f16(weight.format, &weight.buffer, weight_scratch, weight.elements())
            .map_err(err)?;
        pipeline.stream.record(&pipeline.dequantized[slot]).map_err(err)?;
        submission.clef_wait(&pipeline.dequantized[slot]).map_err(err)?;
    }
    if knobs().skip_gemm {
        return Ok(());
    }
    let result = gemm(submission, weight, weight_scratch, x16, out, out16, n, accumulate, compute_16f);
    submission.clef_record(&pipeline.consumed[slot]).map_err(err)?;
    result
}

#[allow(clippy::too_many_arguments)]
fn gemm(
    submission: &mut CudaSubmission,
    weight: &DeviceWeight,
    weight_scratch: &CudaBuffer,
    x16: &CudaBuffer,
    out: &CudaBuffer,
    out16: &CudaBuffer,
    n: usize,
    accumulate: bool,
    compute_16f: bool,
) -> Result<(), String> {
    if compute_16f {
        submission
            .clef_linear(
                ClefOperand::f16(x16, 0),
                ClefOperand::f16(weight_scratch, 0),
                ClefOperand::f16(out16, 0),
                n,
                weight.rows,
                weight.columns,
                false,
                true,
            )
            .map_err(err)?;
        return submission
            .clef_f16_to_f32(out16, out, n * weight.rows, accumulate)
            .map_err(err);
    }
    submission
        .clef_linear(
            ClefOperand::f16(x16, 0),
            ClefOperand::f16(weight_scratch, 0),
            ClefOperand::f32(out, 0),
            n,
            weight.rows,
            weight.columns,
            accumulate,
            false,
        )
        .map_err(err)
}

/// Experiment knobs. `PSIONIC_CLEF_SKIP=dequant,gemm,delta,attention` is
/// for profiling only (skipping makes the answers wrong).
/// `PSIONIC_CLEF_FUSED` picks the fused dequantize + tensor-core kernel
/// (bitwise chunk-invariant, f32-promoted accumulation): `1` for every
/// projection, `0` for none, `N` for chunks of at most N tokens (default
/// [`FUSED_UP_TO`]; above that dequantize + cuBLAS is faster on the 4080); `PSIONIC_CLEF_SEGMENT=1|2|4|8|16` sets its f16
/// accumulation span in 16-wide k steps (default 16); `PSIONIC_CLEF_SCAN=0`
/// runs the per-warp delta scan instead of the shared-memory-staged one;
/// `PSIONIC_CLEF_FLASH=0` runs attention as cuBLAS score and `P V` GEMMs
/// instead of the fixed-order flash kernel.
/// When a projection runs through the fused kernel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Fused {
    /// Never (dequantize + cuBLAS).
    #[default]
    Never,
    /// Every projection.
    Always,
    /// Chunks of at most this many tokens: there the projections are bound
    /// by reading the weights, and the fused kernel reads them quantized
    /// once instead of writing and re-reading an f16 copy.
    UpTo(usize),
}

impl Fused {
    fn takes(self, n: usize) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::UpTo(limit) => n <= limit,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Knobs {
    skip_dequant: bool,
    skip_gemm: bool,
    skip_delta: bool,
    skip_attention: bool,
    fused: Fused,
    segment: usize,
    staged_scan: bool,
    flash: bool,
}

fn knobs() -> Knobs {
    static KNOBS: std::sync::OnceLock<Knobs> = std::sync::OnceLock::new();
    *KNOBS.get_or_init(|| {
        let value = std::env::var("PSIONIC_CLEF_SKIP").unwrap_or_default();
        Knobs {
            skip_dequant: value.contains("dequant"),
            skip_gemm: value.contains("gemm"),
            skip_delta: value.contains("delta"),
            skip_attention: value.contains("attention"),
            fused: match std::env::var("PSIONIC_CLEF_FUSED").as_deref() {
                Ok("1") => Fused::Always,
                Ok("0") => Fused::Never,
                Ok(other) => other.parse().map_or(Fused::UpTo(FUSED_UP_TO), Fused::UpTo),
                Err(_) => Fused::UpTo(FUSED_UP_TO),
            },
            staged_scan: std::env::var("PSIONIC_CLEF_SCAN").map_or(true, |v| v != "0"),
            flash: std::env::var("PSIONIC_CLEF_FLASH").map_or(true, |v| v != "0"),
            segment: std::env::var("PSIONIC_CLEF_SEGMENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v| matches!(v, 1 | 2 | 4 | 8 | 16))
                .unwrap_or(16),
        }
    })
}

/// The default largest chunk the fused kernel takes (`PSIONIC_CLEF_FUSED`).
const FUSED_UP_TO: usize = 1024;

/// Score plus probability bytes (6 per element) one batched attention call
/// may use; above it a KV group's heads run in smaller batches.
const ATTENTION_SCORE_BUDGET_BYTES: usize = 512 << 20;

#[allow(clippy::too_many_arguments)]
fn encode_layer(
    submission: &mut CudaSubmission,
    layer: &DeviceLayer,
    scratch: &Scratch,
    weight_scratch: &WeightPipeline,
    request: &Request,
    layer_index: usize,
    dims: Dims,
    first: usize,
    n: usize,
    compute_16f: bool,
) -> Result<(), String> {
    let s = scratch;
    submission
        .clef_rms_norm_to_f16(&s.x, &layer.attention_norm, &s.act16, n, dims.hidden, dims.eps)
        .map_err(err)?;
    match &layer.mixer {
        DeviceMixer::Hybrid { qkv, z, alpha, beta, out, conv, ssm_a, ssm_dt, ssm_norm } => {
            let conv_state = request.conv[layer_index].as_ref().ok_or("conv state")?;
            let delta_state = request.delta[layer_index].as_ref().ok_or("delta state")?;
            linear(submission, qkv, weight_scratch, &s.act16, &s.big_a, &s.out16, n, false, compute_16f)?;
            linear(submission, z, weight_scratch, &s.act16, &s.mid_a, &s.out16, n, false, compute_16f)?;
            linear(submission, alpha, weight_scratch, &s.act16, &s.small_a, &s.out16, n, false, compute_16f)?;
            linear(submission, beta, weight_scratch, &s.act16, &s.small_b, &s.out16, n, false, compute_16f)?;
            submission
                .clef_conv1d_seq_silu(&s.big_a, conv_state, conv, &s.conv_out, n, dims.conv_channels, dims.conv_kernel)
                .map_err(err)?;
            submission
                .clef_delta_prep(
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
                )
                .map_err(err)?;
            if !knobs().skip_delta {
            submission
                .clef_delta_seq(
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
                    knobs().staged_scan,
                )
                .map_err(err)?;
            }
            submission
                .clef_gated_norm_to_f16(&s.mid_b, &s.mid_a, ssm_norm, &s.act16, n, dims.value_heads, dims.state, dims.eps)
                .map_err(err)?;
            linear(submission, out, weight_scratch, &s.act16, &s.x, &s.out16, n, true, compute_16f)?;
        }
        DeviceMixer::Attention { query_gate, key, value, out, query_norm, key_norm } => {
            let key_cache = request.key_cache[layer_index].as_ref().ok_or("key cache")?;
            let value_cache = request.value_cache[layer_index].as_ref().ok_or("value cache")?;
            linear(submission, query_gate, weight_scratch, &s.act16, &s.big_a, &s.out16, n, false, compute_16f)?;
            linear(submission, key, weight_scratch, &s.act16, &s.small_a, &s.out16, n, false, compute_16f)?;
            linear(submission, value, weight_scratch, &s.act16, &s.small_b, &s.out16, n, false, compute_16f)?;
            submission
                .clef_attention_prep(
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
                )
                .map_err(err)?;
            if !knobs().skip_attention {
                attention(submission, s, request, key_cache, value_cache, dims, first, n)?;
            }
            submission
                .clef_sigmoid_gate_to_f16(&s.mid_b, &s.mid_a, &s.act16, n * dims.heads * dims.head_dim)
                .map_err(err)?;
            linear(submission, out, weight_scratch, &s.act16, &s.x, &s.out16, n, true, compute_16f)?;
        }
    }
    submission
        .clef_rms_norm_to_f16(&s.x, &layer.post_attention_norm, &s.act16, n, dims.hidden, dims.eps)
        .map_err(err)?;
    linear(submission, &layer.gate, weight_scratch, &s.act16, &s.big_a, &s.out16, n, false, compute_16f)?;
    linear(submission, &layer.up, weight_scratch, &s.act16, &s.big_b, &s.out16, n, false, compute_16f)?;
    submission
        .clef_silu_mul_to_f16(&s.big_a, &s.big_b, &s.act16, n * dims.ffn)
        .map_err(err)?;
    linear(submission, &layer.down, weight_scratch, &s.act16, &s.x, &s.out16, n, true, compute_16f)?;
    Ok(())
}

/// Causal attention of the chunk's queries (f16 in `scratch.query16`)
/// against the cache `[0, first + n)`, written f32 `[n, heads, dim]` to
/// `scratch.mid_b`. Scores and probabilities live in a per-request buffer.
#[allow(clippy::too_many_arguments)]
fn attention(
    submission: &mut CudaSubmission,
    s: &Scratch,
    request: &Request,
    key_cache: &CudaBuffer,
    value_cache: &CudaBuffer,
    dims: Dims,
    first: usize,
    n: usize,
) -> Result<(), String> {
    if knobs().flash
        && submission
            .clef_flash_attention(
                &s.query16,
                key_cache,
                value_cache,
                &s.mid_b,
                n,
                dims.heads,
                dims.kv_heads,
                dims.head_dim,
                first,
            )
            .map_err(err)?
    {
        return Ok(());
    }
    let keys = first + n;
    let group = dims.heads / dims.kv_heads.max(1);
    let batch = (request.score_capacity / (n * keys).max(1)).clamp(1, group);
    let (scores, probs) = (&request.scores, &request.probs);
    let kv_stride = dims.kv_heads * dims.head_dim;
    let q_stride = dims.heads * dims.head_dim;
    for kv in 0..dims.kv_heads {
        let mut head0 = 0;
        while head0 < group {
            let count = batch.min(group - head0);
            let head = kv * group + head0;
            // scores^T [keys, n] = K_kv [keys, dim] . Q_h^T [dim, n]
            submission
                .clef_gemm_strided_batched(
                    true,
                    false,
                    keys,
                    n,
                    dims.head_dim,
                    ClefOperand::f16(key_cache, kv * dims.head_dim),
                    kv_stride,
                    0,
                    (keys - 1) * kv_stride + dims.head_dim,
                    ClefOperand::f16(&s.query16, head * dims.head_dim),
                    q_stride,
                    dims.head_dim,
                    (n - 1) * q_stride + count * dims.head_dim,
                    ClefOperand::f32(scores, 0),
                    keys,
                    n * keys,
                    count * n * keys,
                    count,
                )
                .map_err(err)?;
            submission
                .clef_causal_softmax_to_f16(scores, probs, count * n, n, keys, first)
                .map_err(err)?;
            // out^T [dim, n] = V_kv^T [dim, keys] . P^T [keys, n]
            submission
                .clef_gemm_strided_batched(
                    false,
                    false,
                    dims.head_dim,
                    n,
                    keys,
                    ClefOperand::f16(value_cache, kv * dims.head_dim),
                    kv_stride,
                    0,
                    (keys - 1) * kv_stride + dims.head_dim,
                    ClefOperand::f16(probs, 0),
                    keys,
                    n * keys,
                    count * n * keys,
                    ClefOperand::f32(&s.mid_b, head * dims.head_dim),
                    q_stride,
                    dims.head_dim,
                    (n - 1) * q_stride + count * dims.head_dim,
                    count,
                )
                .map_err(err)?;
            head0 += count;
        }
    }
    Ok(())
}

/// Rotary parameters of the trunk, for the per-chunk cos/sin table.
struct RopeTable {
    freq_scale: f32,
    ext_factor: f32,
    corr_dims: [f32; 2],
    theta_scale: f32,
    sections: Option<[usize; 4]>,
    interleaved: bool,
}

fn rope_table_params(model: &super::CpuQwen35Model) -> RopeTable {
    let rotary = model.descriptor.config.block.attention.rotary_dim;
    let rotary = rotary.min(model.descriptor.config.block.attention.head_dim).max(2);
    let (freq_scale, ext_factor, corr_dims, theta_scale) =
        qwen35_rope_runtime_parameters(rotary, &model.family_metadata);
    RopeTable {
        freq_scale,
        ext_factor,
        corr_dims,
        theta_scale,
        sections: qwen35_mrope_sections(&model.family_metadata),
        interleaved: qwen35_mrope_interleaved(&model.family_metadata),
    }
}

impl RopeTable {
    /// `[n, rotary/2, (cos, sin)]` for text positions `first..first + n`
    /// (all three MRoPE positions equal), as the CPU lane computes them.
    fn table(&self, first: usize, n: usize, rotary: usize) -> Vec<f32> {
        let pairs = rotary / 2;
        let mut out = Vec::with_capacity(n * pairs * 2);
        for position in first..first + n {
            let p = position as f32;
            for pair in 0..pairs {
                let theta_base = if let Some(sections) = self.sections {
                    qwen35_mrope_theta_base(pair, sections, self.interleaved, [p, p, p, 0.0])
                        * self.theta_scale.powf(pair as f32)
                } else {
                    p * self.theta_scale.powf(pair as f32)
                };
                let (cos, sin) =
                    rope_yarn(theta_base, self.freq_scale, self.corr_dims, pair * 2, self.ext_factor, 1.0);
                out.push(cos);
                out.push(sin);
            }
        }
        out
    }
}
