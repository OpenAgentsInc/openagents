//! Per-request prefill state and shared-weight FFN cohorts.

use super::*;

/// All mutable backbone state belongs to this request until its head starts.
pub(crate) struct MetalSession {
    request: Request,
    scratch: Scratch,
    tokens: Vec<TokenId>,
    spans: Vec<(usize, usize)>,
    span_buffer: ClefMetalBuffer,
    span_sums: ClefMetalBuffer,
    first: usize,
    chunk: usize,
    boundary: Option<usize>,
    prefix_ids: Vec<u32>,
    cached_tokens: usize,
    prefix_owner: Arc<()>,
    last: Vec<f32>,
}

/// The model owns at most one snapshot, whether pending or published.
pub(super) struct PendingPrefix {
    owner: std::sync::Weak<()>,
    snapshot: Snapshot,
}

pub(super) fn reap_pending(state: &mut DeviceState) {
    if state
        .batch_prefix
        .as_ref()
        .is_some_and(|entry| entry.owner.upgrade().is_none())
    {
        state.batch_prefix = None;
    }
}

impl MetalSession {
    pub(crate) fn remaining_tokens(&self) -> usize {
        self.tokens.len() - self.first
    }

    pub(crate) fn chunk_tokens(&self) -> usize {
        self.chunk
    }

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

/// Only the single lane executor may activate a completed request for its head.
/// Clearing the head's memory on every exit prevents it leaking into another job.
pub(crate) struct ActiveHead<'a> {
    trunk: &'a ClefMetalTrunk,
}

impl Drop for ActiveHead<'_> {
    fn drop(&mut self) {
        let mut state = self
            .trunk
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.request = None;
        state.head_attention = None;
        state.head_io = None;
    }
}

impl ClefMetalTrunk {
    #[cfg(test)]
    pub(crate) fn scratch_estimate_for_test(
        large: bool,
        tokens: usize,
        delta: ClefMetalDelta,
        low_memory: bool,
    ) -> Result<u64, String> {
        let mut d = tests::dims();
        if large {
            d.hidden = 5120;
            d.ffn = 17408;
            d.conv_channels = 10240;
            d.value_heads = 48;
            d.heads = 24;
        }
        let weight_bytes = if low_memory {
            ffn_conversion_bytes(d.ffn, d.hidden)?
        } else {
            0
        };
        scratch_bytes(d, tokens, delta, weight_bytes)
    }

    pub(crate) fn reap_pending_prefix(&self) {
        reap_pending(
            &mut self
                .device
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
    pub(crate) fn cohort_bytes(&self, tokens: usize) -> Result<u64, String> {
        let state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        scratch_bytes(
            self.dims,
            tokens,
            state.metal.delta_mode(),
            state.weight_workspace_bytes,
        )
    }

    /// Includes the request's backbone buffers and a conservative allowance
    /// for the head's normalized memory, attention, and temporary projections.
    /// The separate cohort budget covers the shared FFN scratch allocation.
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
        let chunk = chunk.min(length);
        let d = self.dims;
        let state = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut sizes = vec![
            scratch_bytes(
                d,
                chunk,
                state.metal.delta_mode(),
                state.weight_workspace_bytes,
            )?,
            allocation(&[length, d.width, 4])?,
            allocation(&[spans, 2, 4])?,
            allocation(&[spans, d.hidden, 4])?,
        ];
        let mut has_attention = false;
        for layer in &state.layers {
            match &layer.mixer {
                DeviceMixer::Hybrid { .. } => {
                    sizes.push(allocation(&[
                        d.conv_channels,
                        d.conv_kernel.saturating_sub(1).max(1),
                        4,
                    ])?);
                    sizes.push(allocation(&[d.value_heads, d.state, d.state, 4])?);
                }
                DeviceMixer::Attention { .. } => {
                    has_attention = true;
                    sizes.extend([allocation(&[length, d.kv_heads, d.head_dim, 2])?; 2]);
                }
            }
        }
        if has_attention && state.metal.attention_mode() == ClefMetalAttention::Staged {
            let plan =
                ClefAttentionPlan::new(chunk, d.heads, d.kv_heads, d.head_dim, length - chunk)?;
            sizes.extend([
                plan.score_bytes.max(16) as u64,
                plan.probability_bytes.max(16) as u64,
            ]);
        }
        for _ in &state.evidence_norms {
            sizes.push(allocation(&[length, d.width, 4])?);
        }
        let rows = spans
            .checked_mul(heads)
            .and_then(usize::checked_next_power_of_two)
            .ok_or("batch head rows overflow")?
            .max(64);
        sizes.push(allocation(&[rows, length, 4])?);
        sizes.extend([allocation(&[rows, d.width, 4])?; 4]);
        sizes.push(allocation(&[d.width, 4])?);
        if let Some(fields) = &state.head_fields {
            // Each active request owns its field workspace until the final wait.
            // There is at least one span per question; the public limit is 64.
            sizes.push(fields.workspace_bytes(spans.clamp(1, 64))? as u64);
        }
        // Head linears execute serially. Six times the largest input/output
        // geometry covers the batched option projections and FFN temporaries.
        let matrix_width = state
            .head_matrices
            .values()
            .map(|(_, rows, columns)| (*rows).max(*columns))
            .max()
            .unwrap_or(d.hidden)
            .max(d.hidden);
        sizes.push(allocation(&[6, spans.max(1), matrix_width, 4])?);
        sum_sizes(&sizes)
    }

    /// Creates isolated text-only state. The caller holds the lane execution
    /// lock and admits only a bounded number of sessions.
    pub(crate) fn begin_session(
        &self,
        tokens: Vec<TokenId>,
        spans: Vec<(usize, usize)>,
        chunk: usize,
        prefix_tokens: usize,
    ) -> Result<MetalSession, String> {
        let length = tokens.len();
        validate_prefill_spans(length, &spans)?;
        let dims = self.dims;
        let chunk = chunk.clamp(1, length);
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        // The execution mutex excludes a serial prefill or an active head.
        // Drop their cached buffers before allocating this request's allowance.
        state.request = None;
        state.head_attention = None;
        state.head_io = None;
        state.scratch = None;
        ensure_scratch(state, dims, chunk)?;
        let scratch = state.scratch.take().ok_or("missing session scratch")?;
        let mut request = new_request(state, dims, length, chunk)?;
        let span_values: Vec<i32> = spans
            .iter()
            .flat_map(|&(start, end)| [start as i32, end as i32])
            .collect();
        let span_buffer = state.metal.buffer(span_values.len().max(2) * 4);
        span_buffer.write_i32(0, &span_values)?;
        let span_sums = state.metal.buffer((spans.len() * dims.hidden).max(1) * 4);
        request.bytes += (span_buffer.byte_len() + span_sums.byte_len()) as u64;
        let boundary = reusable_boundary(prefix_tokens, length, &spans).filter(|&prefix| {
            Snapshot::byte_len(&request, dims, prefix)
                .is_ok_and(|bytes| state.prefix.admits(prefix, bytes))
        });
        let prefix_ids: Vec<u32> = boundary.map_or_else(Vec::new, |prefix| {
            tokens[..prefix]
                .iter()
                .map(|token| token.as_u32())
                .collect()
        });
        let cached_tokens = if let Some(saved) = state.prefix.get(&prefix_ids, chunk) {
            saved.restore(&request)?;
            prefix_ids.len()
        } else {
            0
        };
        Ok(MetalSession {
            request,
            scratch,
            tokens,
            spans,
            span_buffer,
            span_sums,
            first: cached_tokens,
            chunk,
            boundary,
            prefix_ids,
            cached_tokens,
            prefix_owner: Arc::new(()),
            last: Vec::new(),
        })
    }

    /// Runs one chunk of each request. Mixer kernels remain separate; every
    /// layer's three FFN projections operate on the cohort's concatenated rows.
    pub(crate) fn advance_sessions(
        &self,
        cpu: &CpuGgufQwen35TextGenerationService,
        sessions: &mut [(&mut MetalSession, usize)],
        capture_prefix: bool,
    ) -> Result<Option<usize>, String> {
        if sessions.is_empty() {
            return Ok(None);
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
        let model = &cpu.model;
        let rope = rope_table_params(model);
        for (session, count) in sessions.iter_mut() {
            let mut embedded = vec![0.0f32; *count * dims.hidden];
            embedded
                .par_chunks_mut(dims.hidden)
                .zip(session.tokens[session.first..session.first + *count].par_iter())
                .try_for_each(|(slot, token)| -> Result<(), String> {
                    let index = token.as_u32() as usize;
                    if index >= model.descriptor.config.vocab_size {
                        return Err(format!("token {index} is outside the vocabulary"));
                    }
                    slot.copy_from_slice(&model.token_embedding.decode_row(index).map_err(err)?);
                    Ok(())
                })?;
            session.scratch.x.write_f32(0, &embedded)?;
            session
                .scratch
                .cos_sin
                .write_f32(0, &rope_table(&rope, session.first, *count, dims.rotary))?;
        }
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        if sessions.len() > 1 {
            if state
                .scratch
                .as_ref()
                .is_some_and(|scratch| scratch.capacity > total)
            {
                state.scratch = None;
            }
            ensure_scratch(state, dims, total)?;
        }
        let batch = state.metal.batch();
        for (index, layer) in state.layers.iter().enumerate() {
            if sessions.len() == 1 {
                let (session, count) = &sessions[0];
                encode_layer(
                    &batch,
                    layer,
                    &session.scratch,
                    &session.request,
                    index,
                    dims,
                    session.first,
                    *count,
                )?;
                continue;
            }
            let packed = state.scratch.as_ref().ok_or("missing cohort scratch")?;
            for (session, count) in sessions.iter() {
                encode_mixer(
                    &batch,
                    layer,
                    &session.scratch,
                    &session.request,
                    index,
                    dims,
                    session.first,
                    *count,
                )?;
            }
            let mut row = 0;
            for (session, count) in sessions.iter() {
                batch.copy_f32(
                    &session.scratch.x,
                    0,
                    &packed.x,
                    row * dims.hidden,
                    count * dims.hidden,
                )?;
                row += count;
            }
            encode_ffn(&batch, layer, packed, dims, total)?;
            row = 0;
            for (session, count) in sessions.iter() {
                batch.copy_f32(
                    &packed.x,
                    row * dims.hidden,
                    &session.scratch.x,
                    0,
                    count * dims.hidden,
                )?;
                row += count;
            }
        }
        for (session, count) in sessions.iter() {
            let scratch = &session.scratch;
            batch.rms_norm_f32(
                &scratch.x,
                &state.output_norm,
                &scratch.final_rows,
                *count,
                dims.hidden,
                dims.eps,
            )?;
            batch.layer_norm_f32(
                &scratch.final_rows,
                Some((&state.hidden_norm_weight, &state.hidden_norm_bias)),
                &scratch.norm_rows,
                *count,
                dims.hidden,
                dims.head_eps,
            )?;
            batch.linear_f32_ordered(
                &scratch.norm_rows,
                &state.memory_projection,
                &session.request.memory,
                session.first * dims.width,
                *count,
                dims.width,
                dims.hidden,
            )?;
            batch.span_sums(
                &scratch.norm_rows,
                &session.span_buffer,
                &session.span_sums,
                session.spans.len(),
                dims.hidden,
                session.first,
                *count,
            )?;
        }
        batch.commit_wait()?;
        let mut captured = None;
        for (index, (session, count)) in sessions.iter_mut().enumerate() {
            session.first += *count;
            if session.first == session.tokens.len() {
                session.last = session
                    .scratch
                    .norm_rows
                    .read_f32((*count - 1) * dims.hidden, dims.hidden)?;
            }
            reap_pending(state);
            if capture_prefix
                && captured.is_none()
                && state.batch_prefix.is_none()
                && session.cached_tokens == 0
                && session.boundary == Some(session.first)
            {
                // The executor permits only one pending snapshot across all
                // sessions, sharing the existing model cache's byte budget.
                state.prefix.clear();
                state.batch_prefix = Some(PendingPrefix {
                    owner: Arc::downgrade(&session.prefix_owner),
                    snapshot: Snapshot::capture(&session.request, dims, session.first)?,
                });
                captured = Some(index);
            }
        }
        Ok(captured)
    }

    /// Activates exactly this completed session while the executor runs its head.
    pub(crate) fn activate_session(
        &self,
        session: MetalSession,
    ) -> Result<(ClefDevicePrefill, usize, ActiveHead<'_>), String> {
        if session.first != session.tokens.len() {
            return Err(String::from("cannot run the head before prefill completes"));
        }
        let sums = if session.spans.is_empty() {
            Vec::new()
        } else {
            session
                .span_sums
                .read_f32(0, session.spans.len() * self.dims.hidden)?
        };
        let prefill = ClefDevicePrefill {
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
        if state
            .batch_prefix
            .as_ref()
            .is_some_and(|pending| pending.owner.ptr_eq(&Arc::downgrade(&session.prefix_owner)))
        {
            let snapshot = state
                .batch_prefix
                .take()
                .expect("matching pending prefix")
                .snapshot;
            let bytes = Snapshot::byte_len(&session.request, self.dims, session.prefix_ids.len())?;
            state
                .prefix
                .insert(&session.prefix_ids, session.chunk, bytes, snapshot);
        }
        state.request = Some(session.request);
        Ok((prefill, session.cached_tokens, ActiveHead { trunk: self }))
    }
}

fn allocation(factors: &[usize]) -> Result<u64, String> {
    factors
        .iter()
        .try_fold(1u64, |value, &factor| {
            value
                .checked_mul(factor as u64)
                .ok_or_else(|| String::from("batch allocation size overflow"))
        })
        .map(|bytes| bytes.max(16))
}

fn sum_sizes(sizes: &[u64]) -> Result<u64, String> {
    sizes.iter().try_fold(0u64, |value, &bytes| {
        value
            .checked_add(bytes)
            .ok_or_else(|| String::from("batch allocation total overflow"))
    })
}

fn scratch_bytes(
    d: Dims,
    c: usize,
    delta: ClefMetalDelta,
    weight_workspace_bytes: usize,
) -> Result<u64, String> {
    let wide = d.ffn.max(d.conv_channels).max(d.heads * d.head_dim * 2);
    let mid = d
        .hidden
        .max(d.value_heads * d.state)
        .max(d.heads * d.head_dim);
    let small = d.value_heads.max(d.kv_heads * d.head_dim).max(1);
    let widths = [
        d.hidden,
        wide,
        wide,
        d.conv_channels.max(1),
        mid,
        mid,
        small,
        small,
        small,
        small,
        (d.key_heads * d.state).max(1),
        (d.key_heads * d.state).max(1),
        d.key_heads.max(1),
        d.rotary.max(2),
        d.hidden,
        d.hidden,
    ];
    let mut sizes = widths
        .iter()
        .map(|&width| allocation(&[c, width, 4]))
        .collect::<Result<Vec<_>, _>>()?;
    sizes.push(allocation(&[c, wide.max(mid), 2])?);
    sizes.push(allocation(&[c, (d.heads * d.head_dim).max(1), 2])?);
    sizes.push(delta_workspace_bytes(d, c, delta)? as u64);
    if weight_workspace_bytes > MAX_FFN_CONVERSION_BYTES {
        return Err(String::from("clef FFN weight conversion exceeds 256 MiB"));
    }
    sizes.push(weight_workspace_bytes as u64);
    sum_sizes(&sizes)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn dims() -> Dims {
        Dims {
            hidden: 4096,
            ffn: 12288,
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
            width: 256,
            head_eps: 1e-5,
        }
    }

    #[test]
    fn delta_workspace_is_counted_once_per_session_or_cohort_scratch() {
        for tokens in [1, 31, 32, 33, 1024, 2048] {
            let sequential = scratch_bytes(dims(), tokens, ClefMetalDelta::Sequential, 0).unwrap();
            let block = scratch_bytes(dims(), tokens, ClefMetalDelta::BlockTensorOps, 0).unwrap();
            let workspace = ClefDeltaPlan::workspace_bytes(tokens, 32).unwrap() as u64;
            assert_eq!(block - sequential, workspace);
        }
    }

    #[test]
    fn oversized_delta_scratch_is_refused_and_sequential_needs_none() {
        assert!(scratch_bytes(dims(), 7274, ClefMetalDelta::BlockTensorOps, 0).is_err());
        assert!(scratch_bytes(dims(), 7274, ClefMetalDelta::Sequential, 0).is_ok());
        let mut attention_only = dims();
        attention_only.key_heads = 0;
        attention_only.value_heads = 0;
        attention_only.state = 0;
        assert_eq!(
            scratch_bytes(attention_only, 33, ClefMetalDelta::Sequential, 0),
            scratch_bytes(attention_only, 33, ClefMetalDelta::BlockTensorOps, 0),
        );
    }

    #[test]
    fn weight_conversion_slot_is_counted_once_and_does_not_scale_with_chunk() {
        let slot = ffn_conversion_bytes(17408, 5120).unwrap();
        assert_eq!(slot, 178_257_920);
        for tokens in [1, 33, 1024, 2048] {
            let base = scratch_bytes(dims(), tokens, ClefMetalDelta::Sequential, 0).unwrap();
            let low_memory =
                scratch_bytes(dims(), tokens, ClefMetalDelta::Sequential, slot).unwrap();
            assert_eq!(low_memory - base, slot as u64);
        }
        assert!(
            scratch_bytes(
                dims(),
                1,
                ClefMetalDelta::Sequential,
                MAX_FFN_CONVERSION_BYTES + 1
            )
            .is_err()
        );
    }
}
