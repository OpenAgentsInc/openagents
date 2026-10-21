//! The CPU side of W7's stage parity (#12394): layers `0..end` of the CPU
//! path (`psionic_models::deepseek_v41_cpu`) in its default `ReferenceBf16`
//! order, with the same observations the GPU stage keeps.
//!
//! The CPU transformer runs all 40 layers and keeps its attention state
//! private, so this composes the CPU path's public components the way
//! `transformer.rs` and `shared_state.rs` compose them: `TextBlock` for the
//! Engram gate, the mixes, the norms, the expansion, and the MoE, and
//! `AttentionProjections`, `Rope`, `CompressionState`, the index scores,
//! `candidate_mask`, `select_indices`, and `sparse_attention_bf16` for
//! attention. The stage tests check it against the CPU transformer on a
//! synthetic 40-layer model, bit for bit.

use super::V41Site;
use psionic_models::deepseek_v41_cpu::{
    CpuError,
    attention::{CompressionState, candidate_mask, select_indices},
    block::{BlockConfig, TextBlock},
    engram::{EngramLayout, NgramHashState},
    matvec,
    projection::{self, AttentionProjections, WeightSource, compress_ratio},
    quantization::{Arithmetic, Quantization, inplace_bf16, round_bf16, sparse_attention_bf16},
    rope::{Rope, RopeConfig, group_position},
    shared_state::LayerOwners,
};
use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;
use std::sync::Arc;

const ARITHMETIC: Arithmetic = Arithmetic::ReferenceBf16;

/// One step's observations: per layer, per site, the values of every token
/// of the step one after another (integers as exact `f64`).
pub type Observations = BTreeMap<usize, BTreeMap<V41Site, Vec<f64>>>;

struct Source {
    compressor: CompressionState,
    kv: Vec<f32>,
    keys: Vec<f32>,
}

/// What one CPU step returns.
pub struct CpuStepOutput {
    pub observations: Observations,
    /// Per token: the output residual streams and pre-mix.
    pub residuals: Vec<Vec<f32>>,
    pub pre_mixes: Vec<Vec<f32>>,
    /// From a stage that ends at layer 40: the last token's final norm and
    /// logits.
    pub final_norm: Option<Vec<f32>>,
    pub logits: Option<Vec<f32>>,
}

/// Layers `0..end` on the CPU.
pub struct CpuStage<'a> {
    weights: &'a dyn WeightSource,
    block: BlockConfig,
    layers: Range<usize>,
    layout: Arc<EngramLayout>,
    hash: NgramHashState,
    compressed_ids: Vec<u32>,
    candidate_blocks: usize,
    candidate_block_size: usize,
    windows: Vec<VecDeque<Vec<f32>>>,
    sources: BTreeMap<usize, Source>,
    position: usize,
}

fn push_site(observations: &mut Observations, layer: usize, site: V41Site, values: &[f32]) {
    observations
        .entry(layer)
        .or_default()
        .entry(site)
        .or_default()
        .extend(values.iter().map(|v| f64::from(*v)));
}

fn push_ints(observations: &mut Observations, layer: usize, site: V41Site, values: &[i64]) {
    observations
        .entry(layer)
        .or_default()
        .entry(site)
        .or_default()
        .extend(values.iter().map(|v| *v as f64));
}

/// The reference index scores of `shared_state::reference_index_scores`.
fn reference_index_scores(
    query: &[f32],
    heads: usize,
    width: usize,
    keys: &[f32],
    weights: &[f32],
) -> Result<Vec<f32>, CpuError> {
    let scale = ((width as f64).powf(-0.5) * (heads as f64).powf(-0.5)) as f32;
    let weights = round_bf16(&weights.iter().map(|w| w * scale).collect::<Vec<_>>())?;
    let mut out = Vec::new();
    for key in keys.chunks_exact(width) {
        let mut score = 0.0_f32;
        for h in 0..heads {
            let dot = query[h * width..(h + 1) * width]
                .iter()
                .zip(key)
                .map(|(q, k)| q * k)
                .sum::<f32>();
            if !dot.is_finite() {
                return Err(CpuError::NonFinite("reference index dot product"));
            }
            score += dot.max(0.0) * weights[h];
        }
        out.push(score);
    }
    Ok(out)
}

impl<'a> CpuStage<'a> {
    /// A stage of layers `0..end`. `compressed_ids` maps each vocabulary
    /// token to its Engram id, and `pad` is the compressed pad id.
    pub fn new(
        weights: &'a dyn WeightSource,
        block: BlockConfig,
        end: usize,
        layout: Arc<EngramLayout>,
        compressed_ids: Vec<u32>,
        pad: u32,
        candidates: (usize, usize),
    ) -> Result<Self, CpuError> {
        if end == 0 || end > 40 {
            return Err(CpuError::Invalid("CPU stage layers"));
        }
        let mut sources = BTreeMap::new();
        for layer in [2_usize, 8, 14, 20].into_iter().filter(|l| *l < end) {
            sources.insert(
                layer,
                Source {
                    compressor: CompressionState::new(
                        compress_ratio(layer)?,
                        block.geometry.head_dim,
                    )?
                    .with_arithmetic(ARITHMETIC),
                    kv: Vec::new(),
                    keys: Vec::new(),
                },
            );
        }
        Ok(Self {
            weights,
            block,
            layers: 0..end,
            hash: NgramHashState::new(layout.clone(), pad)?,
            layout,
            compressed_ids,
            candidate_blocks: candidates.0,
            candidate_block_size: candidates.1,
            windows: vec![VecDeque::new(); 40],
            sources,
            position: 0,
        })
    }

    /// Runs `tokens` at the next position.
    #[allow(clippy::too_many_lines)]
    pub fn step(&mut self, tokens: &[u32]) -> Result<CpuStepOutput, CpuError> {
        let geometry = self.block.geometry;
        let width = geometry.hidden;
        let streams = self.block.hyper.streams;
        let start = self.position;
        let compressed: Vec<Option<u32>> = tokens
            .iter()
            .map(|t| {
                self.compressed_ids
                    .get(*t as usize)
                    .copied()
                    .map(Some)
                    .ok_or(CpuError::Invalid("token outside the map"))
            })
            .collect::<Result<_, _>>()?;
        let hashes = self.hash.push_compressed(start, &compressed)?;
        let mut observations = Observations::new();
        let mut residuals = Vec::with_capacity(tokens.len());
        let mut pre_mixes = Vec::with_capacity(tokens.len());
        for token in tokens {
            let embedded = round_bf16(&projection::embedding(
                self.weights,
                *token as usize,
                width,
            )?)?;
            residuals.push(embedded.repeat(streams));
            let mut pre = vec![0.0; streams];
            pre[0] = 1.0;
            pre_mixes.push(pre);
        }
        let mut chunk_selections: BTreeMap<usize, Vec<Vec<i32>>> = BTreeMap::new();
        let mut chunk_candidates: Option<Vec<Vec<bool>>> = None;
        for layer in self.layers.clone() {
            let block = TextBlock::new(self.weights, layer, self.block, ARITHMETIC)?;
            let slot = self
                .layout
                .layers
                .iter()
                .position(|l| l.layer_id as usize == layer);
            if let Some(slot) = slot {
                let table_rows = self.layout.layers[slot].table_rows;
                for (i, residual) in residuals.iter_mut().enumerate() {
                    let ids = &hashes[i].layers[slot];
                    push_ints(
                        &mut observations,
                        layer,
                        V41Site::EngramIds,
                        &ids.iter().map(|x| *x as i64).collect::<Vec<_>>(),
                    );
                    let mut rows = Vec::new();
                    *residual = block.apply_engram_observed(
                        residual,
                        ids,
                        table_rows,
                        self.layout.head_dim,
                        &mut |r| {
                            rows = r.to_vec();
                            Ok(())
                        },
                    )?;
                    push_site(&mut observations, layer, V41Site::EngramRows, &rows);
                    push_site(&mut observations, layer, V41Site::EngramOut, residual);
                }
            }
            let mut normalized = Vec::with_capacity(tokens.len() * width);
            let mut mixes = Vec::with_capacity(tokens.len());
            for (residual, pre) in residuals.iter().zip(&pre_mixes) {
                push_site(&mut observations, layer, V41Site::HcIn, residual);
                let input = block.begin_attention(residual, pre)?;
                let mut all = input.mixes.pre.clone();
                all.extend(&input.mixes.post);
                all.extend(&input.mixes.comb);
                push_site(&mut observations, layer, V41Site::AttnMixes, &all);
                push_site(&mut observations, layer, V41Site::AttnIn, &input.normalized);
                normalized.extend(input.normalized);
                mixes.push(input.mixes);
            }
            let (attention, selections, candidates) = self.attention(
                layer,
                start,
                &normalized,
                &chunk_selections,
                chunk_candidates.as_deref(),
            )?;
            if let Some(rows) = selections {
                for row in &rows {
                    let mut padded = row.clone();
                    padded.resize(geometry.index_top_k, -1);
                    push_ints(
                        &mut observations,
                        layer,
                        V41Site::Selection,
                        &padded.iter().map(|x| i64::from(*x)).collect::<Vec<_>>(),
                    );
                }
                chunk_selections.insert(layer, rows);
            }
            if let Some(rows) = candidates {
                chunk_candidates = Some(rows);
            }
            for i in 0..tokens.len() {
                let out = &attention[i * width..(i + 1) * width];
                push_site(&mut observations, layer, V41Site::AttnOut, out);
                let attended = block.finish_attention(out, &residuals[i], &mixes[i])?;
                push_site(&mut observations, layer, V41Site::AttnExpanded, &attended);
                let output = block.feed_forward(&attended, &mixes[i].pre)?;
                let mut all = output.mixes.pre.clone();
                all.extend(&output.mixes.post);
                all.extend(&output.mixes.comb);
                push_site(&mut observations, layer, V41Site::FfnMixes, &all);
                push_site(
                    &mut observations,
                    layer,
                    V41Site::FfnIn,
                    &output.normalized_input,
                );
                // The GPU writes ids in ascending order with their weights.
                let mut routed: Vec<(usize, f32)> = output
                    .routing
                    .indices
                    .iter()
                    .copied()
                    .zip(output.routing.weights.iter().copied())
                    .collect();
                routed.sort_by_key(|(id, _)| *id);
                push_ints(
                    &mut observations,
                    layer,
                    V41Site::ExpertIds,
                    &routed.iter().map(|(id, _)| *id as i64).collect::<Vec<_>>(),
                );
                push_site(
                    &mut observations,
                    layer,
                    V41Site::RouteWeights,
                    &routed.iter().map(|(_, w)| *w).collect::<Vec<_>>(),
                );
                push_site(
                    &mut observations,
                    layer,
                    V41Site::FfnOut,
                    &output.moe_output,
                );
                push_site(&mut observations, layer, V41Site::HcOut, &output.residual);
                residuals[i] = output.residual;
                pre_mixes[i] = output.next_pre;
            }
        }
        let (mut final_norm, mut logits) = (None, None);
        if self.layers.end == 40 {
            let last = tokens.len() - 1;
            let (_, normalized) = projection::final_input_with_arithmetic(
                self.weights,
                &residuals[last],
                &pre_mixes[last],
                width,
                geometry.norm_epsilon,
                ARITHMETIC,
            )?;
            logits = Some(matvec(
                self.weights.matrix("head.weight")?.as_ref(),
                &normalized,
            )?);
            final_norm = Some(normalized);
        }
        self.position += tokens.len();
        Ok(CpuStepOutput {
            observations,
            residuals,
            pre_mixes,
            final_norm,
            logits,
        })
    }

    /// One layer's attention over the chunk, as `AttentionRuntime::run_layer`
    /// in the `ReferenceBf16` order. Returns the outputs, this layer's
    /// selections when it's an index source, and layer 20's candidates.
    #[allow(clippy::type_complexity, clippy::too_many_lines)]
    fn attention(
        &mut self,
        layer: usize,
        start: usize,
        input: &[f32],
        selections: &BTreeMap<usize, Vec<Vec<i32>>>,
        candidates: Option<&[Vec<bool>]>,
    ) -> Result<(Vec<f32>, Option<Vec<Vec<i32>>>, Option<Vec<Vec<bool>>>), CpuError> {
        let g = self.block.geometry;
        let ratio = compress_ratio(layer)?;
        let owners = LayerOwners::for_layer(layer)?;
        let p = AttentionProjections::new(self.weights, layer, g)?.with_arithmetic(ARITHMETIC)?;
        let rope = Rope::new(if ratio == 0 {
            RopeConfig::window(g.rope_dim)
        } else {
            RopeConfig::compressed(g.rope_dim)
        })?;
        let sinks = p.sinks()?;
        let norm = if owners.kv == Some(layer) {
            Some(p.compressor_norm()?)
        } else {
            None
        };
        let mut selected_rows = Vec::new();
        let mut candidate_rows = Vec::new();
        let mut outputs = Vec::with_capacity(input.len());
        for (i, x) in input.chunks_exact(g.hidden).enumerate() {
            let position = start + i;
            let x = round_bf16(x)?;
            let mut q = p.query(&x)?;
            let mut window = p.window_kv(&x)?;
            rope.apply_tail(&mut q.heads, g.head_dim, position, false)?;
            q.heads = round_bf16(&q.heads)?;
            rope.apply_tail(&mut window, g.head_dim, position, false)?;
            let window = inplace_bf16(&round_bf16(&window)?, Quantization::Fp8)?;
            let rows = &mut self.windows[layer];
            if rows.len() == g.window {
                rows.pop_front();
            }
            rows.push_back(window);
            if let Some(norm) = &norm {
                let (value, gate) = p.compressor_inputs(&x)?;
                let source = self
                    .sources
                    .get_mut(&layer)
                    .ok_or(CpuError::Invalid("missing KV source"))?;
                if let Some(mut latent) = source.compressor.push(
                    position,
                    &value,
                    gate.as_deref(),
                    norm,
                    g.norm_epsilon,
                )? {
                    let group = group_position(position, ratio)?;
                    let mut key = p.index_key(&latent)?;
                    rope.apply_tail(&mut key, g.index_dim, group, false)?;
                    let key = inplace_bf16(&round_bf16(&key)?, Quantization::IndexFp4)?;
                    rope.apply_tail(&mut latent, g.head_dim, group, false)?;
                    let kv = inplace_bf16(&round_bf16(&latent)?, Quantization::CompressedFp4)?;
                    source.keys.extend(key);
                    source.kv.extend(kv);
                }
            }
            let mut kv: Vec<f32> = self.windows[layer].iter().flatten().copied().collect();
            let window_rows = kv.len() / g.head_dim;
            let mut indices: Vec<i32> = (0..window_rows as i32).collect();
            if let Some(owner) = owners.kv {
                let visible = (position + 1) / ratio;
                let source = self
                    .sources
                    .get(&owner)
                    .ok_or(CpuError::Invalid("missing shared KV owner"))?;
                let kv_len = visible * g.head_dim;
                let key_len = visible * g.index_dim;
                if source.kv.len() < kv_len || source.keys.len() < key_len {
                    return Err(CpuError::Invalid("source has not produced this position"));
                }
                let selected = if owners.index == Some(layer) {
                    let mut query = p.index_query(&q.low_rank)?;
                    rope.apply_tail(&mut query, g.index_dim, position, false)?;
                    let query = inplace_bf16(&round_bf16(&query)?, Quantization::IndexFp4)?;
                    let weights = p.index_weights(&x)?;
                    let scores = reference_index_scores(
                        &query,
                        g.index_heads,
                        g.index_dim,
                        &source.keys[..key_len],
                        &weights,
                    )?;
                    if layer == 20 {
                        candidate_rows.push(candidate_mask(
                            &scores,
                            visible,
                            self.candidate_blocks,
                            self.candidate_block_size,
                        )?);
                    }
                    let mask = if layer > 20 {
                        Some(
                            candidates
                                .and_then(|rows| rows.get(i))
                                .ok_or(CpuError::Invalid("missing layer-20 candidates"))?
                                .as_slice(),
                        )
                    } else {
                        None
                    };
                    let selected = select_indices(&scores, visible, g.index_top_k, 0, mask)?;
                    selected_rows.push(selected.clone());
                    selected
                } else {
                    selections
                        .get(&owners.index.unwrap_or(40))
                        .and_then(|rows| rows.get(i))
                        .ok_or(CpuError::Invalid("missing shared index selection"))?
                        .clone()
                };
                for id in selected {
                    indices.push(if id < 0 { -1 } else { window_rows as i32 + id });
                }
                kv.extend_from_slice(&source.kv[..kv_len]);
            }
            let mut heads = sparse_attention_bf16(
                &q.heads,
                g.heads,
                g.head_dim,
                &kv,
                &indices,
                &sinks,
                (g.head_dim as f64).powf(-0.5) as f32,
            )?;
            rope.apply_tail(&mut heads, g.head_dim, position, true)?;
            let heads = round_bf16(&heads)?;
            let low_rank = p.output_low_rank(&heads)?;
            outputs.extend(p.output_from_low_rank(&low_rank)?);
        }
        let selections =
            (owners.index == Some(layer) && owners.kv.is_some()).then_some(selected_rows);
        let candidates = (layer == 20).then_some(candidate_rows);
        Ok((outputs, selections, candidates))
    }
}
