//! Attention primitives from `model.py:410-611` and `kernel.py:311-389`.
//!
//! Inputs are projected, rotated, and dequantized float32 vectors. This module
//! does not implement projection wiring, RoPE/YaRN, quantization, or ownership
//! of shared cross-layer caches. The sparse attention weights here stay f32;
//! the reference rounds them to BF16 before its value product.

use super::{CpuError, finite, positive, product, rms_norm, shape};

/// Prefill addresses a contiguous KV array; decode addresses a window ring.
/// Matches the reference's full prefill followed by single-token decode.
pub fn window_indices(
    window: usize,
    sequence_len: usize,
    start_position: usize,
) -> Result<Vec<Vec<i32>>, CpuError> {
    if window == 0
        || sequence_len == 0
        || window > i32::MAX as usize
        || sequence_len > i32::MAX as usize
        || (start_position > 0 && sequence_len != 1)
    {
        return Err(CpuError::Invalid("window dimensions or decode chunk"));
    }
    if start_position == 0 {
        let count = sequence_len.min(window);
        Ok((0..sequence_len)
            .map(|end| {
                let first = (end + 1).saturating_sub(window);
                (first..first + count)
                    .map(|i| if i <= end { i as i32 } else { -1 })
                    .collect()
            })
            .collect())
    } else {
        let oldest = start_position % window + 1;
        Ok(vec![
            (oldest..window)
                .chain(0..oldest)
                .map(|slot| {
                    if slot <= start_position {
                        slot as i32
                    } else {
                        -1
                    }
                })
                .collect(),
        ])
    }
}

/// One token's latent multi-head attention. K and V are the same shared vector.
/// The sink adds probability mass to the denominator with a zero value vector.
pub fn sparse_attention(
    query: &[f32],
    heads: usize,
    width: usize,
    kv: &[f32],
    indices: &[i32],
    sinks: &[f32],
    scale: f32,
) -> Result<Vec<f32>, CpuError> {
    if heads == 0 || width == 0 || !kv.len().is_multiple_of(width) || !positive(scale) {
        return Err(CpuError::Invalid("sparse attention dimensions or scale"));
    }
    shape("attention query", query, product(heads, width)?)?;
    shape("attention sinks", sinks, heads)?;
    finite("attention KV", kv)?;
    let positions = kv.len() / width;
    if indices
        .iter()
        .any(|i| *i < -1 || (*i >= 0 && *i as usize >= positions))
    {
        return Err(CpuError::Invalid("sparse attention index"));
    }
    let mut out = vec![0.0; query.len()];
    for h in 0..heads {
        let q = &query[h * width..(h + 1) * width];
        let mut selected = Vec::new();
        let mut max = sinks[h];
        for i in indices.iter().copied().filter(|i| *i >= 0) {
            let i = i as usize;
            let score = q
                .iter()
                .zip(&kv[i * width..(i + 1) * width])
                .map(|(q, k)| q * k)
                .sum::<f32>()
                * scale;
            if !score.is_finite() {
                return Err(CpuError::NonFinite("attention dot product"));
            }
            max = max.max(score);
            selected.push((i, score));
        }
        let mut denominator = (sinks[h] - max).exp();
        for (i, score) in selected {
            let weight = (score - max).exp();
            denominator += weight;
            for d in 0..width {
                out[h * width + d] += weight * kv[i * width + d];
            }
        }
        for d in 0..width {
            out[h * width + d] /= denominator;
        }
    }
    finite("attention output", &out)?;
    Ok(out)
}

/// Sequential learned compression over projected per-channel values and gates.
/// A completed group is RMS-normalized; an incomplete group produces no latent.
#[derive(Debug, Clone)]
pub struct CompressionState {
    ratio: usize,
    width: usize,
    next_position: usize,
    values: Vec<f32>,
    gates: Vec<f32>,
    arithmetic: super::quantization::Arithmetic,
    reference_rows: usize,
}

impl CompressionState {
    pub fn new(ratio: usize, width: usize) -> Result<Self, CpuError> {
        if ratio == 0 || width == 0 {
            return Err(CpuError::Invalid("compression dimensions"));
        }
        product(ratio, width)?;
        Ok(Self {
            ratio,
            width,
            next_position: 0,
            values: Vec::new(),
            gates: Vec::new(),
            arithmetic: super::quantization::Arithmetic::Float32,
            reference_rows: 1,
        })
    }

    pub fn next_position(&self) -> usize {
        self.next_position
    }
    pub fn with_arithmetic(mut self, arithmetic: super::quantization::Arithmetic) -> Self {
        self.arithmetic = arithmetic;
        self
    }
    pub(crate) fn set_reference_rows(&mut self, rows: usize) {
        self.reference_rows = rows;
    }
    pub fn pending_tokens(&self) -> usize {
        self.values.len() / self.width
    }
    pub fn reset(&mut self) {
        self.next_position = 0;
        self.values.clear();
        self.gates.clear();
    }

    /// Process a prefill chunk, committing its state only if every token passes.
    pub fn push_chunk(
        &mut self,
        start_position: usize,
        values: &[f32],
        gates: Option<&[f32]>,
        norm_weight: &[f32],
        epsilon: f32,
    ) -> Result<Vec<Vec<f32>>, CpuError> {
        if !values.len().is_multiple_of(self.width) || start_position != self.next_position {
            return Err(CpuError::Invalid(
                "compression chunk dimensions or position",
            ));
        }
        if let Some(gates) = gates {
            shape("compression chunk gates", gates, values.len())?;
        }
        let mut next = self.clone();
        let mut outputs = Vec::new();
        for (i, value) in values.chunks_exact(self.width).enumerate() {
            let gate = gates.map(|g| &g[i * self.width..(i + 1) * self.width]);
            if let Some(output) =
                next.push(next.next_position, value, gate, norm_weight, epsilon)?
            {
                outputs.push(output);
            }
        }
        *self = next;
        Ok(outputs)
    }

    /// A failed push leaves both the history and expected position unchanged.
    pub fn push(
        &mut self,
        position: usize,
        value: &[f32],
        gate: Option<&[f32]>,
        norm_weight: &[f32],
        epsilon: f32,
    ) -> Result<Option<Vec<f32>>, CpuError> {
        if position != self.next_position || !positive(epsilon) {
            return Err(CpuError::Invalid("compression position or epsilon"));
        }
        let next = position
            .checked_add(1)
            .ok_or(CpuError::Invalid("position overflow"))?;
        shape("compression value", value, self.width)?;
        shape("compression norm", norm_weight, self.width)?;
        match (self.ratio, gate) {
            (1, None) => {}
            (1, Some(_)) => return Err(CpuError::Invalid("ratio-one compression has no gate")),
            (_, Some(g)) => shape("compression gate", g, self.width)?,
            _ => return Err(CpuError::Invalid("missing compression gate")),
        }
        let mut values = self.values.clone();
        values.extend_from_slice(value);
        let mut gates = self.gates.clone();
        if let Some(g) = gate {
            gates.extend_from_slice(g);
        }
        let complete = values.len() / self.width == self.ratio;
        let output = if complete {
            let mut pooled = vec![0.0; self.width];
            if self.ratio == 1 {
                pooled.copy_from_slice(value);
            } else {
                for d in 0..self.width {
                    let max = (0..self.ratio)
                        .map(|r| gates[r * self.width + d])
                        .fold(f32::NEG_INFINITY, f32::max);
                    let denom: f32 = (0..self.ratio)
                        .map(|r| (gates[r * self.width + d] - max).exp())
                        .sum();
                    pooled[d] = (0..self.ratio)
                        .map(|r| {
                            values[r * self.width + d]
                                * ((gates[r * self.width + d] - max).exp() / denom)
                        })
                        .sum();
                }
            }
            let reference = self.arithmetic.is_bf16();
            if reference {
                pooled = super::quantization::round_bf16(&pooled)?;
            }
            let norm = if reference {
                super::quantization::round_bf16(norm_weight)?
            } else {
                norm_weight.to_vec()
            };
            let output = if self.arithmetic == super::quantization::Arithmetic::ReferenceBf16Tp8 {
                super::rms_norm_reference(&pooled, &norm, epsilon, self.reference_rows)?
            } else {
                rms_norm(&pooled, &norm, epsilon)?
            };
            Some(if reference {
                super::quantization::round_bf16(&output)?
            } else {
                output
            })
        } else {
            None
        };
        self.next_position = next;
        if complete {
            self.values.clear();
            self.gates.clear();
        } else {
            self.values = values;
            self.gates = gates;
        }
        Ok(output)
    }
}

/// Rectified index-head dot products weighted by the projected per-head weights.
/// This includes `head_dim^-0.5 * n_heads^-0.5`, before causal masking/top-k.
pub fn index_scores(
    query: &[f32],
    heads: usize,
    width: usize,
    keys: &[f32],
    weights: &[f32],
) -> Result<Vec<f32>, CpuError> {
    if heads == 0 || width == 0 || !keys.len().is_multiple_of(width) {
        return Err(CpuError::Invalid("indexer dimensions"));
    }
    shape("indexer query", query, product(heads, width)?)?;
    shape("indexer head weights", weights, heads)?;
    finite("indexer keys", keys)?;
    let scale = (width as f32).sqrt().recip() * (heads as f32).sqrt().recip();
    let mut scores = Vec::with_capacity(keys.len() / width);
    for key in keys.chunks_exact(width) {
        let mut score = 0.0;
        for h in 0..heads {
            let dot: f32 = query[h * width..(h + 1) * width]
                .iter()
                .zip(key)
                .map(|(q, k)| q * k)
                .sum();
            if !dot.is_finite() {
                return Err(CpuError::NonFinite("indexer dot product"));
            }
            score += dot.max(0.0) * (weights[h] * scale);
        }
        scores.push(score);
    }
    finite("indexer scores", &scores)?;
    Ok(scores)
}

fn score_input(scores: &[f32], visible: usize) -> Result<(), CpuError> {
    if visible > scores.len() || scores.iter().any(|x| x.is_nan() || *x == f32::INFINITY) {
        return Err(CpuError::Invalid("index scores or visible length"));
    }
    Ok(())
}

/// Candidate blocks use their maximum score; the newest block is pinned.
/// Like the reference, the mask covers a whole partial block. Consumers must
/// still intersect it with the causal visible length.
pub fn candidate_mask(
    scores: &[f32],
    visible: usize,
    top_blocks: usize,
    block_size: usize,
) -> Result<Vec<bool>, CpuError> {
    score_input(scores, visible)?;
    if top_blocks == 0 || block_size == 0 {
        return Err(CpuError::Invalid("candidate dimensions"));
    }
    let count = scores.len().div_ceil(block_size);
    let mut block_scores = vec![f32::NEG_INFINITY; count];
    for (i, score) in scores[..visible].iter().enumerate() {
        block_scores[i / block_size] = block_scores[i / block_size].max(*score);
    }
    if visible > 0 {
        block_scores[(visible - 1) / block_size] = f32::INFINITY;
    }
    let mut blocks: Vec<_> = (0..count).collect();
    blocks.sort_unstable_by(|a, b| {
        block_scores[*b]
            .total_cmp(&block_scores[*a])
            .then_with(|| a.cmp(b))
    });
    let mut keep = vec![false; count];
    for b in blocks.into_iter().take(top_blocks) {
        keep[b] = block_scores[b] > f32::NEG_INFINITY;
    }
    Ok((0..scores.len()).map(|i| keep[i / block_size]).collect())
}

/// Top-k positions sorted by position, then padded with -1 for unavailable
/// slots. Exact score ties choose the lower position, independently of torch's
/// unspecified device tie order. No masked candidate can become a KV address.
pub fn select_indices(
    scores: &[f32],
    visible: usize,
    top_k: usize,
    offset: usize,
    candidates: Option<&[bool]>,
) -> Result<Vec<i32>, CpuError> {
    score_input(scores, visible)?;
    if top_k == 0 || candidates.is_some_and(|x| x.len() != scores.len()) {
        return Err(CpuError::Invalid("index selection dimensions"));
    }
    let count = top_k.min(scores.len());
    let mut indices: Vec<_> = (0..visible)
        .filter(|i| scores[*i].is_finite() && candidates.is_none_or(|mask| mask[*i]))
        .collect();
    indices.sort_unstable_by(|a, b| scores[*b].total_cmp(&scores[*a]).then_with(|| a.cmp(b)));
    indices.truncate(count);
    indices.sort_unstable();
    let mut selected = Vec::with_capacity(count);
    for i in indices {
        selected.push(
            i.checked_add(offset)
                .and_then(|i| i32::try_from(i).ok())
                .ok_or(CpuError::Invalid("KV index overflow"))?,
        );
    }
    selected.resize(count, -1);
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        query: Vec<f32>,
        kv: Vec<f32>,
        sinks: Vec<f32>,
        indices: Vec<i32>,
        attention: Vec<f32>,
        index_weights: Vec<f32>,
        scores: Vec<f32>,
        values: Vec<f32>,
        gates: Vec<f32>,
        norm: Vec<f32>,
        compressed: Vec<f32>,
    }
    fn near(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 2e-6, "{x} != {y}");
        }
    }

    #[test]
    fn matches_numpy_attention_compression_and_indexer() -> Result<(), Box<dyn std::error::Error>> {
        let f: Fixture =
            serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/attention.json"))?;
        near(
            &sparse_attention(
                &f.query,
                2,
                3,
                &f.kv,
                &f.indices,
                &f.sinks,
                3.0_f32.sqrt().recip(),
            )?,
            &f.attention,
        );
        near(
            &index_scores(&f.query, 2, 3, &f.kv, &f.index_weights)?,
            &f.scores,
        );
        let mut c = CompressionState::new(2, 3)?;
        assert!(
            c.push(0, &f.values[..3], Some(&f.gates[..3]), &f.norm, 1e-20)?
                .is_none()
        );
        let out = c.push(1, &f.values[3..], Some(&f.gates[3..]), &f.norm, 1e-20)?;
        assert!(out.is_some());
        if let Some(out) = out {
            near(&out, &f.compressed);
        }
        Ok(())
    }

    #[test]
    fn window_prefill_and_ring_boundary_127_128_129() -> Result<(), CpuError> {
        let pre = window_indices(128, 130, 0)?;
        assert_eq!(pre[0][0], 0);
        assert!(pre[0][1..].iter().all(|i| *i == -1));
        assert_eq!(pre[127], (0..128).collect::<Vec<i32>>());
        assert_eq!(pre[128], (1..129).collect::<Vec<i32>>());
        assert_eq!(pre[129], (2..130).collect::<Vec<i32>>());
        for position in [127, 128, 129] {
            let ring = window_indices(128, 1, position)?;
            let absolute: Vec<_> = pre[position].iter().map(|i| i % 128).collect();
            assert_eq!(ring[0], absolute);
        }
        Ok(())
    }

    #[test]
    fn sink_has_zero_value_and_empty_histories_are_zero() -> Result<(), CpuError> {
        near(
            &sparse_attention(&[0.0], 1, 1, &[4.0], &[0], &[0.0], 1.0)?,
            &[2.0],
        );
        near(
            &sparse_attention(&[0.0], 1, 1, &[], &[-1, -1], &[1000.0], 1.0)?,
            &[0.0],
        );
        near(
            &sparse_attention(&[0.0], 1, 1, &[4.0], &[0], &[1000.0], 1.0)?,
            &[0.0],
        );
        Ok(())
    }

    #[test]
    fn compression_odd_tail_chunking_reset_and_atomic_errors() -> Result<(), CpuError> {
        let mut c = CompressionState::new(2, 2)?;
        assert!(
            c.push(0, &[1.0, 2.0], Some(&[0.0, 2.0]), &[1.0, 1.0], 1e-20)?
                .is_none()
        );
        assert_eq!(c.pending_tokens(), 1);
        assert!(
            c.push(2, &[3.0, 4.0], Some(&[2.0, 0.0]), &[1.0, 1.0], 1e-20)
                .is_err()
        );
        assert!(
            c.push(1, &[f32::NAN, 4.0], Some(&[2.0, 0.0]), &[1.0, 1.0], 1e-20)
                .is_err()
        );
        assert_eq!(c.next_position(), 1);
        let mut resumed = c.clone();
        let a = c.push(1, &[3.0, 4.0], Some(&[2.0, 0.0]), &[1.0, 1.0], 1e-20)?;
        assert_eq!(
            a,
            resumed.push(1, &[3.0, 4.0], Some(&[2.0, 0.0]), &[1.0, 1.0], 1e-20)?
        );
        assert_eq!(c.pending_tokens(), 0);
        assert!(
            c.push(2, &[5.0, 6.0], Some(&[1.0, 1.0]), &[1.0, 1.0], 1e-20)?
                .is_none()
        );
        c.reset();
        assert_eq!(c.next_position(), 0);
        assert_eq!(c.pending_tokens(), 0);
        let mut one = CompressionState::new(1, 2)?;
        assert_eq!(
            one.push(0, &[3.0, 4.0], None, &[1.0, 1.0], 1e-20)?,
            Some(rms_norm(&[3.0, 4.0], &[1.0, 1.0], 1e-20)?)
        );
        Ok(())
    }

    #[test]
    fn candidates_pin_newest_partial_block_and_exclude_unreachable_picks() -> Result<(), CpuError> {
        let scores = [9.0, 8.0, 7.0, 6.0, -10.0, -11.0, 1000.0];
        let mask = candidate_mask(&scores, 6, 2, 2)?;
        assert_eq!(mask, vec![true, true, false, false, true, true, false]);
        assert_eq!(
            select_indices(&scores, 6, 5, 10, Some(&mask))?,
            vec![10, 11, 14, 15, -1]
        );
        assert_eq!(candidate_mask(&scores, 0, 2, 2)?, vec![false; 7]);
        assert_eq!(select_indices(&scores, 0, 5, 0, None)?, vec![-1; 5]);
        Ok(())
    }

    #[test]
    fn beyond_candidate_budget_ties_and_short_rows() -> Result<(), CpuError> {
        let scores: Vec<_> = (0..20001).map(|i| -(i as f32)).collect();
        let mask = candidate_mask(&scores, scores.len(), 2048, 8)?;
        assert!(mask[20000]);
        assert!(mask.iter().filter(|b| **b).count() <= 16384);
        assert!(!mask[16376]);
        assert_eq!(select_indices(&[1.0, 1.0, 1.0], 3, 2, 0, None)?, vec![0, 1]);
        assert_eq!(
            select_indices(&[1.0, 2.0], 1, 512, 128, None)?,
            vec![128, -1]
        );
        Ok(())
    }

    #[test]
    fn compression_prefill_chunks_match_token_replay_with_odd_tail() -> Result<(), CpuError> {
        let values: Vec<_> = (1..=10).map(|i| i as f32).collect();
        let gates: Vec<_> = (0..10).map(|i| (i as f32).sin()).collect();
        let mut whole = CompressionState::new(2, 2)?;
        let expected = whole.push_chunk(0, &values, Some(&gates), &[1.0, 2.0], 1e-20)?;
        let mut chunked = CompressionState::new(2, 2)?;
        let mut actual =
            chunked.push_chunk(0, &values[..6], Some(&gates[..6]), &[1.0, 2.0], 1e-20)?;
        assert_eq!(chunked.pending_tokens(), 1);
        actual.extend(chunked.push_chunk(
            3,
            &values[6..],
            Some(&gates[6..]),
            &[1.0, 2.0],
            1e-20,
        )?);
        assert_eq!(expected, actual);
        assert_eq!(whole.pending_tokens(), 1);
        assert_eq!(chunked.pending_tokens(), 1);
        let mut bad = values.clone();
        bad[9] = f32::NAN;
        let mut empty = CompressionState::new(2, 2)?;
        assert!(
            empty
                .push_chunk(0, &bad, Some(&gates), &[1.0, 2.0], 1e-20)
                .is_err()
        );
        assert_eq!(empty.next_position(), 0);
        assert_eq!(empty.pending_tokens(), 0);
        Ok(())
    }

    #[test]
    fn invalid_shapes_scores_and_addresses_are_refused() {
        assert!(window_indices(0, 1, 0).is_err());
        assert!(window_indices(128, 2, 1).is_err());
        assert!(sparse_attention(&[1.0], 1, 1, &[1.0], &[1], &[0.0], 1.0).is_err());
        assert!(index_scores(&[f32::NAN], 1, 1, &[1.0], &[1.0]).is_err());
        assert!(select_indices(&[f32::NAN], 1, 1, 0, None).is_err());
        assert!(select_indices(&[1.0], 1, 1, usize::MAX, None).is_err());
        assert!(candidate_mask(&[1.0], 2, 1, 8).is_err());
    }
}
