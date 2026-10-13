//! Per-request CUDA prefill state and shared-weight FFN cohorts (#11441).
//!
//! The device-generic worker (`clef/batch.rs`) admits several text requests
//! and advances one chunk of each per step. Each request keeps its own conv
//! and DeltaNet state, f16 K/V caches, flash attention, `W_mem` rows and span
//! sums; only the three FFN projections of every layer run once over the
//! cohort's packed rows (the fused kernel up to its row limit, dequantize +
//! cuBLAS above it), and the rows are copied back before the next layer.
//!
//! A session may start from a restored prefix snapshot (#11442); one that
//! misses keeps a chunk end at its prefix and captures a pending snapshot
//! there, published when the request finishes and released if it is
//! cancelled ([`super::prefix`]).

use std::sync::Arc;

use super::memory_admission::{BackboneShape, Estimate, HEAD_RESERVE_BYTES, Inventory};
use super::*;

/// All mutable backbone state belongs to this request until its head starts.
pub(crate) struct CudaSession {
    request: Request,
    scratch: Scratch,
    tokens: Vec<TokenId>,
    spans: usize,
    span_buffer: CudaBuffer,
    span_sums: CudaBuffer,
    first: usize,
    chunk: usize,
    /// The reusable prefix end, kept as a chunk end when the snapshot is
    /// not taken inside a chunk ([`prefix::mid_chunk`]).
    boundary: Option<usize>,
    prefix_ids: Vec<u32>,
    cached_tokens: usize,
    /// Owns this session's pending snapshot; dropping it releases one.
    prefix_owner: Arc<()>,
    last: Vec<f32>,
    /// The whole prompt's embedding rows and rotary table, built by the
    /// request's own thread ([`ClefCudaTrunk::prepare_inputs`]) so the batch
    /// worker does not decode rows while the GPU waits.
    prepared: Option<PreparedInputs>,
}

/// Host inputs of one prompt: embedding rows `[tokens, hidden]` and the
/// rotary table `[tokens, rotary]`.
pub(crate) struct PreparedInputs {
    embedded: Vec<f32>,
    table: Vec<f32>,
}

/// Prompts whose rows take more host memory than this are decoded per chunk
/// on the worker instead.
const MAX_PREPARED_BYTES: usize = 512 << 20;

impl CudaSession {
    pub(crate) fn remaining_tokens(&self) -> usize {
        self.tokens.len() - self.first
    }

    pub(crate) fn chunk_tokens(&self) -> usize {
        self.chunk
    }

    /// Chunk ends the scheduler must keep, relative to the next token.
    pub(crate) fn boundaries(&self) -> Vec<usize> {
        self.boundary
            .filter(|&end| end > self.first)
            .map(|end| end - self.first)
            .into_iter()
            .collect()
    }

    pub(crate) fn device_bytes(&self) -> u64 {
        self.request.bytes + self.scratch.bytes
    }
}

/// The packed rows of one cohort's feed-forward step.
pub(crate) struct CohortScratch {
    capacity: usize,
    x: CudaBuffer,
    act16: CudaBuffer,
    gate: CudaBuffer,
    up: CudaBuffer,
    out16: CudaBuffer,
}

/// Cohort bytes per packed token: residual rows, f16 input, gate and up
/// outputs, and the f16 GEMM output slot.
fn cohort_token_bytes(dims: Dims) -> Result<u64, String> {
    let wide = dims.ffn.max(dims.hidden);
    memory_admission::sum(&[
        memory_admission::bytes(&[dims.hidden], 4)?,
        memory_admission::bytes(&[wide], 2)?,
        memory_admission::bytes(&[dims.ffn.max(1)], 4)?,
        memory_admission::bytes(&[dims.ffn.max(1)], 4)?,
        memory_admission::bytes(&[wide], 2)?,
    ])
}

fn ensure_cohort(state: &mut DeviceState, dims: Dims, tokens: usize) -> Result<(), String> {
    if state
        .cohort
        .as_ref()
        .is_some_and(|cohort| cohort.capacity >= tokens)
    {
        return Ok(());
    }
    state.cohort = None;
    let wide = dims.ffn.max(dims.hidden);
    let backend = &mut state.backend;
    state.cohort = Some(CohortScratch {
        capacity: tokens,
        x: backend.f32_buffer(tokens * dims.hidden).map_err(err)?,
        act16: backend.f16_buffer(tokens * wide).map_err(err)?,
        gate: backend.f32_buffer(tokens * dims.ffn.max(1)).map_err(err)?,
        up: backend.f32_buffer(tokens * dims.ffn.max(1)).map_err(err)?,
        out16: backend.f16_buffer(tokens * wide).map_err(err)?,
    });
    Ok(())
}

/// Finished batched requests leave this many chunk scratches for reuse.
const POOLED_SCRATCH: usize = 4;

/// Only the lane executor may activate a completed request for its head.
/// Clearing the head's memory on every exit prevents it leaking into another job.
pub(crate) struct ActiveHead<'a> {
    trunk: &'a ClefCudaTrunk,
}

impl Drop for ActiveHead<'_> {
    fn drop(&mut self) {
        let mut state = self
            .trunk
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        retire_request(&mut state);
    }
}

fn embed_rows(
    cpu: &CpuGgufQwen35TextGenerationService,
    tokens: &[TokenId],
    hidden: usize,
) -> Result<Vec<f32>, String> {
    let model = &cpu.model;
    let mut embedded = vec![0.0f32; tokens.len() * hidden];
    embedded
        .par_chunks_mut(hidden)
        .zip(tokens.par_iter())
        .try_for_each(|(slot, token)| -> Result<(), String> {
            let index = token.as_u32() as usize;
            if index >= model.descriptor.config.vocab_size {
                return Err(format!("token {index} is outside the vocabulary"));
            }
            slot.copy_from_slice(&model.token_embedding.decode_row(index).map_err(err)?);
            Ok(())
        })?;
    Ok(embedded)
}

impl ClefCudaTrunk {
    /// Decodes a prompt's embedding rows and rotary table on the calling
    /// thread; `None` for prompts above [`MAX_PREPARED_BYTES`].
    pub(crate) fn prepare_inputs(
        &self,
        cpu: &CpuGgufQwen35TextGenerationService,
        tokens: &[TokenId],
    ) -> Result<Option<PreparedInputs>, String> {
        let dims = self.dims;
        if tokens.len().saturating_mul(dims.hidden + dims.rotary) * 4 > MAX_PREPARED_BYTES {
            return Ok(None);
        }
        let rope = rope_table_params(&cpu.model);
        Ok(Some(PreparedInputs {
            embedded: embed_rows(cpu, tokens, dims.hidden)?,
            table: rope.table(0, tokens.len(), dims.rotary),
        }))
    }

    fn shape(&self, state: &DeviceState) -> BackboneShape {
        let d = self.dims;
        let hybrid_layers = state
            .layers
            .iter()
            .filter(|layer| matches!(layer.mixer, DeviceMixer::Hybrid { .. }))
            .count();
        BackboneShape {
            hidden: d.hidden,
            ffn: d.ffn,
            conv_channels: d.conv_channels,
            conv_kernel: d.conv_kernel,
            key_heads: d.key_heads,
            value_heads: d.value_heads,
            state: d.state,
            heads: d.heads,
            kv_heads: d.kv_heads,
            head_dim: d.head_dim,
            rotary: d.rotary,
            width: d.width,
            hybrid_layers,
            attention_layers: state.layers.len() - hybrid_layers,
            flash: knobs().flash,
        }
    }

    /// Bytes of the shared packed FFN rows for a cohort of `tokens` rows.
    pub(crate) fn cohort_bytes(&self, tokens: usize) -> Result<u64, String> {
        cohort_token_bytes(self.dims)?
            .checked_mul(tokens as u64)
            .ok_or_else(|| String::from("CUDA memory estimate overflow"))
    }

    /// One request's backbone allocation (chunk scratch, recurrent state,
    /// K/V caches, memory rows, spans) plus its evidence-normalized memory
    /// views and a conservative allowance for the head's attention buffers.
    pub(crate) fn session_bytes(
        &self,
        length: usize,
        chunk: usize,
        spans: usize,
        heads: usize,
    ) -> Result<u64, String> {
        if length == 0 || chunk == 0 {
            return Err(String::from("empty batch request"));
        }
        let state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let estimate = Estimate::new(&Inventory::default(), self.shape(&state), length, chunk)?;
        let d = self.dims;
        let rows = spans
            .checked_mul(heads)
            .and_then(usize::checked_next_power_of_two)
            .ok_or("batch head rows overflow")?
            .max(64);
        memory_admission::sum(&[
            estimate.backbone_scratch_bytes,
            estimate.backbone_request_bytes,
            memory_admission::bytes(&[spans.max(1), 2], 4)?,
            memory_admission::bytes(&[spans.max(1), d.hidden], 4)?,
            memory_admission::bytes(&[state.evidence_norms.len(), length, d.width], 4)?,
            memory_admission::bytes(&[rows, length], 4)?,
            memory_admission::bytes(&[rows, d.width, 4], 4)?,
            HEAD_RESERVE_BYTES,
        ])
    }

    /// Checks `bytes` against a fresh device memory reading (`not enough
    /// CUDA memory: ...` when it does not fit). Nothing is reserved.
    pub(crate) fn admit_bytes(&self, stage: &str, bytes: u64) -> Result<(), String> {
        let state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        admit_allocation(&state.backend, stage, &[bytes])
    }

    /// Creates isolated text-only state. The caller holds the lane execution
    /// lock and admits only a bounded number of sessions.
    pub(crate) fn begin_session(
        &self,
        tokens: Vec<TokenId>,
        spans: Vec<(usize, usize)>,
        chunk: usize,
        prefix_tokens: usize,
        prepared: Option<PreparedInputs>,
    ) -> Result<CudaSession, String> {
        let length = tokens.len();
        if prepared.as_ref().is_some_and(|inputs| {
            inputs.embedded.len() != length * self.dims.hidden
                || inputs.table.len() != length * self.dims.rotary
        }) {
            return Err(String::from("prepared inputs do not match the prompt"));
        }
        if length == 0 {
            return Err(String::from("empty prompt"));
        }
        if length > i32::MAX as usize
            || spans
                .iter()
                .any(|&(start, end)| start > end || end > length)
        {
            return Err(String::from("invalid prompt span"));
        }
        let dims = self.dims;
        let chunk = chunk.clamp(1, length);
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        // The execution mutex excludes a serial prefill or an active head;
        // retire their request (its buffers may serve this one).
        retire_request(state);
        state.attention_scratch = None;
        let scratch = match state
            .session_scratch
            .iter()
            .position(|scratch| scratch.capacity == chunk)
        {
            Some(index) => state.session_scratch.swap_remove(index),
            None => alloc_scratch(&mut state.backend, dims, chunk)?,
        };
        let mut request = new_request(state, dims, length, chunk)?;
        let span_values: Vec<i32> = spans
            .iter()
            .flat_map(|&(start, end)| [start as i32, end as i32])
            .collect();
        let mut span_buffer = state
            .backend
            .i32_buffer(span_values.len().max(2))
            .map_err(err)?;
        if !span_values.is_empty() {
            let mut padded = span_values;
            let len = padded.len().max(2);
            padded.resize(len, 0);
            span_buffer.write_i32(&padded).map_err(err)?;
        }
        let span_sums = state
            .backend
            .f32_buffer((spans.len() * dims.hidden).max(1))
            .map_err(err)?;
        {
            let mut submission = state.backend.begin_submission().map_err(err)?;
            submission.fill_buffer(&span_sums, 0).map_err(err)?;
            submission.commit(CudaCommandWait::Completed).map_err(err)?;
        }
        request.bytes += span_sums.byte_len() as u64 + span_buffer.byte_len() as u64;
        prefix::reap_pending(state);
        let boundary = aligned_boundary(prefix_tokens, length, &spans, PREFIX_ALIGN)
            .filter(|_| state.prefix_explicit || prefix::mid_chunk(chunk))
            .filter(|&prefix| {
                prefix::Snapshot::byte_len(&request, dims, prefix)
                    .is_ok_and(|bytes| state.prefix.admits(prefix, bytes))
            });
        let prefix_ids: Vec<u32> = boundary.map_or_else(Vec::new, |prefix| {
            tokens[..prefix]
                .iter()
                .map(|token| token.as_u32())
                .collect()
        });
        let cached_tokens = match state.prefix.get(&prefix_ids, chunk) {
            Some(saved) => {
                saved.restore(&mut state.backend, &request, dims)?;
                prefix_ids.len()
            }
            None => 0,
        };
        // On the blocked scan's grid a miss snapshots inside its chunk (the
        // capture is pending, and counted, from now on) and keeps the cold
        // chunking; elsewhere it keeps a chunk end at the prefix.
        let prefix_owner = Arc::new(());
        let mid_chunk = prefix::mid_chunk(chunk);
        if let Some(at) = boundary.filter(|_| mid_chunk && cached_tokens == 0)
            && let Some(target) = prefix::arm_within_budget(state, &request, dims, at)?
        {
            state.pending_prefix.push(prefix::PendingPrefix {
                owner: Arc::downgrade(&prefix_owner),
                bytes: target.bytes,
                snapshot: target.snapshot.clone(),
            });
            request.snap = Some(target);
        }
        let boundary = boundary.filter(|_| !mid_chunk);
        Ok(CudaSession {
            request,
            scratch,
            tokens,
            spans: spans.len(),
            span_buffer,
            span_sums,
            first: cached_tokens,
            chunk,
            boundary,
            prefix_ids,
            cached_tokens,
            prefix_owner,
            last: Vec::new(),
            prepared,
        })
    }

    /// Runs one chunk of each request in one submission. Mixer kernels stay
    /// separate; every layer's three FFN projections operate on the cohort's
    /// concatenated rows.
    pub(crate) fn advance_sessions(
        &self,
        cpu: &CpuGgufQwen35TextGenerationService,
        sessions: &mut [(&mut CudaSession, usize)],
    ) -> Result<(), String> {
        if sessions.is_empty() {
            return Ok(());
        }
        let dims = self.dims;
        let mut total = 0usize;
        for (session, count) in sessions.iter() {
            let allowed = chunk_len(
                session.first,
                session.tokens.len(),
                session.chunk,
                session.boundary,
            );
            if *count == 0 || *count > allowed {
                return Err(String::from("scheduled chunk crosses a request boundary"));
            }
            total = total
                .checked_add(*count)
                .ok_or("cohort token count overflow")?;
        }
        let rope = rope_table_params(&cpu.model);
        for (session, count) in sessions.iter_mut() {
            let (first, n) = (session.first, *count);
            let built;
            let (embedded, table) = match &session.prepared {
                Some(inputs) => (
                    &inputs.embedded[first * dims.hidden..(first + n) * dims.hidden],
                    &inputs.table[first * dims.rotary..(first + n) * dims.rotary],
                ),
                None => {
                    built = (
                        embed_rows(cpu, &session.tokens[first..first + n], dims.hidden)?,
                        rope.table(first, n, dims.rotary),
                    );
                    (built.0.as_slice(), built.1.as_slice())
                }
            };
            session
                .scratch
                .x
                .write_bytes_at_offset(0, f32_bytes(embedded))
                .map_err(err)?;
            session
                .scratch
                .cos_sin
                .write_bytes_at_offset(0, f32_bytes(table))
                .map_err(err)?;
        }
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        if sessions.len() > 1 {
            ensure_cohort(state, dims, total)?;
        }
        let row_bytes = dims.hidden * 4;
        let mut submission = state.backend.begin_submission().map_err(err)?;
        for (index, layer) in state.layers.iter().enumerate() {
            if sessions.len() == 1 {
                let (session, count) = &sessions[0];
                encode_layer(
                    &mut submission,
                    layer,
                    &session.scratch,
                    &state.weight_scratch,
                    &session.request,
                    index,
                    dims,
                    session.first,
                    *count,
                    self.compute_16f,
                )?;
                continue;
            }
            for (session, count) in sessions.iter() {
                encode_mixer(
                    &mut submission,
                    layer,
                    &session.scratch,
                    &state.weight_scratch,
                    &session.request,
                    index,
                    dims,
                    session.first,
                    *count,
                    self.compute_16f,
                )?;
            }
            // Each member's rows run through the kernel its serial prefill
            // would use for that chunk: fused members together (row-exact),
            // then dequantize + cuBLAS members together.
            let packed = state.cohort.as_ref().ok_or("missing cohort scratch")?;
            for fused in [true, false] {
                let group: Vec<_> = sessions
                    .iter()
                    .filter(|(_, count)| knobs().fused.takes(*count) == fused)
                    .collect();
                if group.is_empty() {
                    continue;
                }
                let mut rows = 0;
                for (session, count) in &group {
                    submission
                        .copy_buffer_region(
                            &session.scratch.x,
                            0,
                            &packed.x,
                            rows * row_bytes,
                            count * row_bytes,
                        )
                        .map_err(err)?;
                    rows += count;
                }
                encode_ffn(
                    &mut submission,
                    layer,
                    FfnRows {
                        x: &packed.x,
                        act16: &packed.act16,
                        gate: &packed.gate,
                        up: &packed.up,
                        out16: &packed.out16,
                    },
                    &state.weight_scratch,
                    dims,
                    rows,
                    self.compute_16f,
                    fused,
                )?;
                rows = 0;
                for (session, count) in &group {
                    submission
                        .copy_buffer_region(
                            &packed.x,
                            rows * row_bytes,
                            &session.scratch.x,
                            0,
                            count * row_bytes,
                        )
                        .map_err(err)?;
                    rows += count;
                }
            }
        }
        // output norm, hidden LayerNorm, memory rows, span sums per request
        for (session, count) in sessions.iter() {
            let s = &session.scratch;
            submission
                .clef_rms_norm_f32(
                    &s.x,
                    &state.output_norm,
                    &s.final_rows,
                    *count,
                    dims.hidden,
                    dims.eps,
                )
                .map_err(err)?;
            submission
                .clef_layer_norm_f32(
                    &s.final_rows,
                    Some((&state.hidden_norm_weight, &state.hidden_norm_bias)),
                    &s.norm_rows,
                    *count,
                    dims.hidden,
                    dims.head_eps,
                )
                .map_err(err)?;
            submission
                .clef_linear_f32_ordered(
                    &s.norm_rows,
                    &state.memory_projection,
                    &session.request.memory,
                    session.first * dims.width,
                    *count,
                    dims.width,
                    dims.hidden,
                )
                .map_err(err)?;
            submission
                .clef_span_sums(
                    &s.norm_rows,
                    &session.span_buffer,
                    &session.span_sums,
                    session.spans,
                    dims.hidden,
                    session.first,
                    *count,
                )
                .map_err(err)?;
        }
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        for (session, count) in sessions.iter_mut() {
            session.first += *count;
            if session.first == session.tokens.len() {
                session.last = session
                    .scratch
                    .norm_rows
                    .read_f32_at_offset((*count - 1) * dims.hidden, dims.hidden)
                    .map_err(err)?;
            }
            if session
                .request
                .snap
                .as_ref()
                .is_some_and(|target| target.at <= session.first)
            {
                // the chunk that contains the prefix end has completed
                let target = session.request.snap.take().ok_or("snapshot")?;
                if target.failed.get() {
                    prefix::drop_pending(state, &session.prefix_owner);
                } else {
                    target
                        .snapshot
                        .copy_positional(&mut state.backend, &session.request, dims)?;
                }
            } else if session.cached_tokens == 0 && session.boundary == Some(session.first) {
                // Several sessions may hold pending captures; together with
                // the published snapshots they stay within the budget.
                if let Some((bytes, snapshot)) =
                    prefix::capture_within_budget(state, &session.request, dims, session.first)?
                {
                    state.pending_prefix.push(prefix::PendingPrefix {
                        owner: Arc::downgrade(&session.prefix_owner),
                        bytes,
                        snapshot,
                    });
                }
            }
        }
        Ok(())
    }

    /// Releases the pending snapshots of cancelled or failed sessions.
    pub(crate) fn reap_pending_prefix(&self) {
        prefix::reap_pending(
            &mut self
                .device
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    /// Activates exactly this completed session while the executor runs its head.
    pub(crate) fn activate_session(
        &self,
        session: CudaSession,
    ) -> Result<(ClefCudaPrefill, usize, ActiveHead<'_>), String> {
        if session.first != session.tokens.len() {
            return Err(String::from("cannot run the head before prefill completes"));
        }
        let sums = if session.spans == 0 {
            Vec::new()
        } else {
            session
                .span_sums
                .read_f32_at_offset(0, session.spans * self.dims.hidden)
                .map_err(err)?
        };
        let prefill = ClefCudaPrefill {
            span_sums: sums,
            last: session.last,
            request_device_bytes: session.request.bytes + session.scratch.bytes,
        };
        let mut state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.request.is_some() {
            return Err(String::from("another request still owns head memory"));
        }
        if state.session_scratch.len() >= POOLED_SCRATCH {
            state.session_scratch.remove(0);
        }
        prefix::publish_pending(
            &mut state,
            &session.prefix_owner,
            &session.prefix_ids,
            session.chunk,
        );
        state.session_scratch.push(session.scratch);
        state.request = Some(session.request);
        Ok((prefill, session.cached_tokens, ActiveHead { trunk: self }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dims(large: bool) -> Dims {
        let (hidden, ffn) = if large { (5120, 17408) } else { (4096, 12288) };
        Dims {
            hidden,
            ffn,
            eps: 1e-6,
            conv_channels: 8192,
            conv_kernel: 4,
            key_heads: 16,
            value_heads: 32,
            state: 128,
            v_head_reordered: false,
            heads: 16,
            kv_heads: 4,
            head_dim: 256,
            rotary: 64,
            attention_scale: 0.0625,
            width: 1024,
            head_eps: 1e-5,
        }
    }

    #[test]
    fn default_cohort_fits_the_default_budget_for_flash_and_27b() {
        for large in [false, true] {
            let per_token = cohort_token_bytes(dims(large)).unwrap();
            let d = dims(large);
            assert_eq!(per_token, (d.hidden * 4 + d.ffn * 12) as u64);
            // PSIONIC_CLEF_BATCH_TOKENS=2048 against the 1 GiB default.
            assert!(per_token * 2048 <= 1 << 30);
        }
    }
}
