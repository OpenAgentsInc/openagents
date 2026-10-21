//! Request-owned serial attention state. No global mutable cache or transport.
//!
//! Runs attention only, with already normalized layer inputs. This is not the
//! 40-layer transformer: Engram, mHC, MoE, and final decoding compose separately.

use super::{
    CpuError,
    attention::{self, CompressionState},
    finite, product,
    projection::{AttentionProjections, Geometry, WeightSource, compress_ratio},
    quantization::{self, Arithmetic, Quantization},
    rope::{Rope, RopeConfig, group_position},
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

pub const KV_SOURCES: [usize; 4] = [2, 8, 14, 20];
pub const INDEX_SOURCES: [usize; 8] = [2, 8, 14, 20, 24, 28, 32, 36];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerOwners {
    pub kv: Option<usize>,
    pub index: Option<usize>,
    pub candidates: Option<usize>,
}
impl LayerOwners {
    pub fn for_layer(layer: usize) -> Result<Self, CpuError> {
        compress_ratio(layer)?;
        Ok(Self {
            kv: KV_SOURCES
                .iter()
                .copied()
                .filter(|i| *i <= layer)
                .next_back(),
            index: INDEX_SOURCES
                .iter()
                .copied()
                .filter(|i| *i <= layer)
                .next_back(),
            candidates: if layer >= 20 { Some(20) } else { None },
        })
    }
}

#[derive(Clone, Default)]
struct Window {
    rows: VecDeque<Arc<[f32]>>,
    next_position: usize,
}

/// Keep masked slots in the reference's index order: they determine the
/// 64-index online-softmax blocks even though they contribute no value.
fn reference_window_indices(
    start: usize,
    count: usize,
    position: usize,
    window: usize,
) -> Vec<i32> {
    let rows = (position + 1).min(window);
    if start == 0 {
        (0..count.min(window))
            .map(|i| if i < rows { i as i32 } else { -1 })
            .collect()
    } else {
        let oldest_slot = (position + 1) % window;
        (0..window)
            .map(|i| {
                let slot = (oldest_slot + i) % window;
                if position + 1 < window {
                    if slot <= position { slot as i32 } else { -1 }
                } else {
                    // The CPU stores rows oldest first, rather than in a ring.
                    i as i32
                }
            })
            .collect()
    }
}
impl Window {
    fn push(&mut self, position: usize, value: Vec<f32>, limit: usize) -> Result<(), CpuError> {
        if position != self.next_position {
            return Err(CpuError::Invalid("window position"));
        }
        if self.rows.len() == limit {
            self.rows.pop_front();
        }
        self.rows.push_back(value.into());
        self.next_position += 1;
        Ok(())
    }
}
#[derive(Clone)]
struct Source {
    compressor: CompressionState,
    kv: Arc<Vec<f32>>,
    index_keys: Arc<Vec<f32>>,
}
#[derive(Clone)]
struct Chunk {
    start: usize,
    count: usize,
    next_layer: usize,
    selections: BTreeMap<usize, Arc<Vec<Vec<i32>>>>,
    candidates: Option<Arc<Vec<Vec<bool>>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSummary {
    pub next_position: usize,
    pub next_layer: usize,
    /// `(KV owner, compressed rows, index-key rows, pending compression tokens)`.
    pub sources: Vec<(usize, usize, usize, usize)>,
    pub window_positions: Vec<usize>,
}

/// Weights and numerical mode are fixed for this runtime's entire lifetime.
/// Rust's exclusive mutable borrow serializes updates. Clones share immutable
/// rows; copy-on-write source buffers make a failed layer update transactional.
#[derive(Clone)]
pub struct AttentionRuntime<'a> {
    weights: &'a dyn WeightSource,
    geometry: Geometry,
    arithmetic: Arithmetic,
    max_context: usize,
    windows: Vec<Window>,
    sources: BTreeMap<usize, Source>,
    chunk: Option<Chunk>,
    next_position: usize,
}
impl<'a> AttentionRuntime<'a> {
    pub fn new(
        weights: &'a dyn WeightSource,
        geometry: Geometry,
        max_context: usize,
        arithmetic: Arithmetic,
    ) -> Result<Self, CpuError> {
        geometry.validate()?;
        AttentionProjections::new(weights, 0, geometry)?.with_arithmetic(arithmetic)?;
        if max_context == 0 || max_context > 1_048_576 || geometry.window > 1_048_576 {
            return Err(CpuError::Invalid("attention context capacity"));
        }
        let mut sources = BTreeMap::new();
        for layer in KV_SOURCES {
            sources.insert(
                layer,
                Source {
                    compressor: CompressionState::new(compress_ratio(layer)?, geometry.head_dim)?
                        .with_arithmetic(arithmetic),
                    kv: Arc::new(Vec::new()),
                    index_keys: Arc::new(Vec::new()),
                },
            );
        }
        Ok(Self {
            weights,
            geometry,
            arithmetic,
            max_context,
            windows: vec![Window::default(); 40],
            sources,
            chunk: None,
            next_position: 0,
        })
    }
    pub fn summary(&self) -> StateSummary {
        StateSummary {
            next_position: self.next_position,
            next_layer: self.chunk.as_ref().map_or(0, |c| c.next_layer),
            sources: self
                .sources
                .iter()
                .map(|(id, s)| {
                    (
                        *id,
                        s.kv.len() / self.geometry.head_dim,
                        s.index_keys.len() / self.geometry.index_dim,
                        s.compressor.pending_tokens(),
                    )
                })
                .collect(),
            window_positions: self.windows.iter().map(|w| w.next_position).collect(),
        }
    }
    pub fn reset(&mut self) {
        self.windows.fill(Window::default());
        self.next_position = 0;
        self.chunk = None;
        for s in self.sources.values_mut() {
            s.compressor.reset();
            s.kv = Arc::new(Vec::new());
            s.index_keys = Arc::new(Vec::new());
        }
    }
    /// Feed each chunk through layers 0..39 in order. Chunk sizes may vary
    /// across calls after layer 39. A failure changes no persistent state.
    pub fn forward_chunk(
        &mut self,
        layer: usize,
        start: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, CpuError> {
        self.forward_chunk_observed(layer, start, input, None)
    }

    /// Observes intermediate values without committing state after an observer error.
    pub fn forward_chunk_observed(
        &mut self,
        layer: usize,
        start: usize,
        input: &[f32],
        mut observer: Option<&mut dyn super::transformer::ModelObserver>,
    ) -> Result<Vec<f32>, CpuError> {
        if layer >= 40 || input.is_empty() || !input.len().is_multiple_of(self.geometry.hidden) {
            return Err(CpuError::Invalid("attention layer or input dimensions"));
        }
        finite("attention layer input", input)?;
        let count = input.len() / self.geometry.hidden;
        let end = start
            .checked_add(count)
            .ok_or(CpuError::Invalid("attention position overflow"))?;
        if end > self.max_context {
            return Err(CpuError::Invalid("attention context capacity exceeded"));
        }
        if let Some(c) = &self.chunk {
            if (layer, start, count) != (c.next_layer, c.start, c.count) {
                return Err(CpuError::Invalid(
                    "attention layer order or chunk positions",
                ));
            }
        } else if layer != 0 || start != self.next_position {
            return Err(CpuError::Invalid(
                "attention chunk must start at the next position on layer zero",
            ));
        }
        let mut next = self.clone();
        if next.chunk.is_none() {
            next.chunk = Some(Chunk {
                start,
                count,
                next_layer: 0,
                selections: BTreeMap::new(),
                candidates: None,
            });
        }
        let output = next.run_layer(layer, start, input, &mut observer)?;
        if layer == 39 {
            next.chunk = None;
            next.next_position = end;
        } else {
            next.chunk
                .as_mut()
                .ok_or(CpuError::Invalid("missing attention chunk"))?
                .next_layer = layer + 1;
        }
        *self = next;
        Ok(output)
    }
    fn cache_form(&self, value: &[f32], kind: Quantization) -> Result<Vec<f32>, CpuError> {
        if self.arithmetic.is_bf16() {
            quantization::inplace_bf16(value, kind)
        } else {
            Ok(value.to_vec())
        }
    }
    fn run_layer(
        &mut self,
        layer: usize,
        start: usize,
        input: &[f32],
        observer: &mut Option<&mut dyn super::transformer::ModelObserver>,
    ) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        let ratio = compress_ratio(layer)?;
        let owners = LayerOwners::for_layer(layer)?;
        let p = AttentionProjections::new(self.weights, layer, g)?
            .with_arithmetic(self.arithmetic)?
            .with_reference_rows(input.len() / g.hidden);
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
        if let Some(source) = self.sources.get_mut(&layer) {
            source
                .compressor
                .set_reference_rows((input.len() / g.hidden / ratio.max(1)).max(1));
        }
        let mut selected_rows = Vec::new();
        let mut candidate_rows = Vec::new();
        let mut outputs = Vec::with_capacity(input.len());
        for (i, x) in input.chunks_exact(g.hidden).enumerate() {
            let position = start + i;
            let x = if self.arithmetic.is_bf16() {
                quantization::round_bf16(x)?
            } else {
                x.to_vec()
            };
            let mut q = p.query(&x)?;
            let mut window = p.window_kv(&x)?;
            let mut emit = |site: &str, values: &[f32]| -> Result<(), CpuError> {
                if let Some(o) = observer {
                    o.floats(&format!("layers.{layer}.attn.{site}"), position, values)?;
                }
                Ok(())
            };
            emit("q.normalized", &q.low_rank)?;
            emit("q.projected", &q.heads)?;
            emit("kv.normalized", &window)?;
            rope.apply_tail(&mut q.heads, g.head_dim, position, false)?;
            q.heads = p.finish(q.heads)?;
            rope.apply_tail(&mut window, g.head_dim, position, false)?;
            window = p.finish(window)?;
            emit("q.rotated", &q.heads)?;
            emit("kv.rotated", &window)?;
            let window = self.cache_form(&window, Quantization::Fp8)?;
            emit("kv.quantized", &window)?;
            self.windows[layer].push(position, window, g.window)?;
            if let Some(norm) = &norm {
                let (value, gate) = p.compressor_inputs(&x)?;
                let latent = self
                    .sources
                    .get_mut(&layer)
                    .ok_or(CpuError::Invalid("missing KV source"))?
                    .compressor
                    .push(position, &value, gate.as_deref(), norm, g.norm_epsilon)?;
                if let Some(mut latent) = latent {
                    emit(
                        &if start == 0 {
                            format!("compressor.group{}", position / ratio)
                        } else {
                            "compressor.latent".to_string()
                        },
                        &latent,
                    )?;
                    // Index projection consumes the unrotated latent first.
                    let group_pos = group_position(position, ratio)?;
                    let mut key = p.index_key(&latent)?;
                    rope.apply_tail(&mut key, g.index_dim, group_pos, false)?;
                    let key = self.cache_form(&p.finish(key)?, Quantization::IndexFp4)?;
                    rope.apply_tail(&mut latent, g.head_dim, group_pos, false)?;
                    let kv = self.cache_form(&p.finish(latent)?, Quantization::CompressedFp4)?;
                    let source = self
                        .sources
                        .get_mut(&layer)
                        .ok_or(CpuError::Invalid("missing KV source"))?;
                    Arc::make_mut(&mut source.index_keys).extend(key);
                    Arc::make_mut(&mut source.kv).extend(kv);
                }
            }
            let mut kv: Vec<f32> = self.windows[layer]
                .rows
                .iter()
                .flat_map(|r| r.iter().copied())
                .collect();
            let window_rows = kv.len() / g.head_dim;
            let mut indices: Vec<i32> = if self.arithmetic == Arithmetic::ReferenceBf16Tp8 {
                reference_window_indices(start, input.len() / g.hidden, position, g.window)
            } else {
                (0..window_rows).map(|i| i as i32).collect()
            };
            if let Some(owner) = owners.kv {
                let visible = (position + 1) / ratio;
                let source = self
                    .sources
                    .get(&owner)
                    .ok_or(CpuError::Invalid("missing shared KV owner"))?;
                let kv_len = product(visible, g.head_dim)?;
                let key_len = product(visible, g.index_dim)?;
                if source.kv.len() < kv_len || source.index_keys.len() < key_len {
                    return Err(CpuError::Invalid(
                        "shared KV source has not produced this position",
                    ));
                }
                let mut selected = if owners.index == Some(layer) {
                    let mut query = p.index_query(&q.low_rank)?;
                    rope.apply_tail(&mut query, g.index_dim, position, false)?;
                    let query = self.cache_form(&p.finish(query)?, Quantization::IndexFp4)?;
                    let weights = p.index_weights(&x)?;
                    let scores = if self.arithmetic == Arithmetic::Float32 {
                        attention::index_scores(
                            &query,
                            g.index_heads,
                            g.index_dim,
                            &source.index_keys[..key_len],
                            &weights,
                        )?
                    } else {
                        reference_index_scores(
                            &query,
                            g.index_heads,
                            g.index_dim,
                            &source.index_keys[..key_len],
                            &weights,
                        )?
                    };
                    let mut observed_scores = scores.clone();
                    observed_scores
                        .resize((start + input.len() / g.hidden) / ratio, f32::NEG_INFINITY);
                    emit("indexer.score", &observed_scores)?;
                    if layer == 20 {
                        let mask = attention::candidate_mask(&scores, visible, 2048, 8)?;
                        emit(
                            "indexer.candidate_blocks",
                            &mask
                                .chunks(8)
                                .enumerate()
                                .filter(|(_, block)| block[0])
                                .map(|(i, _)| i as f32)
                                .collect::<Vec<_>>(),
                        )?;
                        candidate_rows.push(mask);
                    }
                    let candidates = if layer > 20 {
                        Some(
                            self.chunk
                                .as_ref()
                                .and_then(|c| c.candidates.as_ref())
                                .and_then(|rows| rows.get(i))
                                .ok_or(CpuError::Invalid("missing layer-20 candidates"))?
                                .as_slice(),
                        )
                    } else {
                        None
                    };
                    let selected =
                        attention::select_indices(&scores, visible, g.index_top_k, 0, candidates)?;
                    selected_rows.push(selected.clone());
                    selected
                } else {
                    self.chunk
                        .as_ref()
                        .and_then(|c| c.selections.get(&owners.index.unwrap_or(40)))
                        .and_then(|rows| rows.get(i))
                        .ok_or(CpuError::Invalid("missing shared index selection"))?
                        .clone()
                };
                if self.arithmetic == Arithmetic::ReferenceBf16Tp8 {
                    selected.resize(
                        ((start + input.len() / g.hidden) / ratio).min(g.index_top_k),
                        -1,
                    );
                }
                if owners.index == Some(layer) {
                    let offset = if start == 0 {
                        input.len() / g.hidden
                    } else {
                        g.window
                    };
                    emit(
                        "indexer.topk_idxs",
                        &selected
                            .iter()
                            .map(|id| {
                                if *id < 0 {
                                    -1.0
                                } else {
                                    *id as f32 + offset as f32
                                }
                            })
                            .collect::<Vec<_>>(),
                    )?;
                }
                for id in selected {
                    if id < -1 || (id >= 0 && id as usize >= visible) {
                        return Err(CpuError::Invalid("shared selection points past causal KV"));
                    }
                    indices.push(if id < 0 {
                        -1
                    } else {
                        i32::try_from(window_rows + id as usize)
                            .map_err(|_| CpuError::Invalid("attention address overflow"))?
                    });
                }
                kv.extend_from_slice(&source.kv[..kv_len]);
            }
            let heads = if self.arithmetic == Arithmetic::Float32 {
                attention::sparse_attention(
                    &q.heads,
                    g.heads,
                    g.head_dim,
                    &kv,
                    &indices,
                    &sinks,
                    (g.head_dim as f32).sqrt().recip(),
                )?
            } else if self.arithmetic == Arithmetic::ReferenceBf16Tp8 {
                quantization::sparse_attention_bf16_tp8(
                    &q.heads,
                    g.heads,
                    g.head_dim,
                    &kv,
                    &indices,
                    &sinks,
                    (g.head_dim as f64).powf(-0.5) as f32,
                )?
            } else {
                quantization::sparse_attention_bf16(
                    &q.heads,
                    g.heads,
                    g.head_dim,
                    &kv,
                    &indices,
                    &sinks,
                    (g.head_dim as f64).powf(-0.5) as f32,
                )?
            };
            emit("heads", &heads)?;
            let mut heads = heads;
            rope.apply_tail(&mut heads, g.head_dim, position, true)?;
            let heads = p.finish(heads)?;
            emit("heads.unrotated", &heads)?;
            let low_rank = p.output_low_rank(&heads)?;
            emit("output.low_rank", &low_rank)?;
            outputs.extend(p.output_from_low_rank(&low_rank)?);
        }
        let chunk = self
            .chunk
            .as_mut()
            .ok_or(CpuError::Invalid("missing attention chunk"))?;
        if owners.index == Some(layer) {
            chunk.selections.insert(layer, Arc::new(selected_rows));
        }
        if layer == 20 {
            chunk.candidates = Some(Arc::new(candidate_rows));
        }
        Ok(outputs)
    }
}

fn reference_index_scores(
    query: &[f32],
    heads: usize,
    width: usize,
    keys: &[f32],
    weights: &[f32],
) -> Result<Vec<f32>, CpuError> {
    // Python computes these scalar factors in f64 before applying them to the
    // BF16 weights. Casting each square root early can change BF16 ties.
    let scale = ((width as f64).powf(-0.5) * (heads as f64).powf(-0.5)) as f32;
    let weights = quantization::round_bf16(&weights.iter().map(|w| w * scale).collect::<Vec<_>>())?;
    let mut out = Vec::new();
    for key in keys.chunks_exact(width) {
        let mut score = 0.0;
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
    finite("reference index scores", &out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::projection::tests::{Store, Tensor, load, tiny};
    use super::super::{MatrixRows, quantization::MatrixPrecision};
    use super::*;
    use serde::Deserialize;
    use std::cell::{Cell, RefCell};

    #[test]
    fn reference_window_masks_keep_prefill_and_decode_block_boundaries() {
        assert_eq!(reference_window_indices(0, 41, 2, 128), {
            let mut expected = vec![0, 1, 2];
            expected.resize(41, -1);
            expected
        });
        let decode = reference_window_indices(41, 1, 41, 128);
        assert_eq!(decode.len(), 128);
        assert_eq!(&decode[..86], &[-1; 86]);
        assert_eq!(&decode[86..], &(0..42).collect::<Vec<_>>());
        assert_eq!(
            reference_window_indices(256, 1, 256, 128),
            (0..128).collect::<Vec<_>>()
        );
        assert_eq!(
            reference_window_indices(0, 300, 256, 128),
            (0..128).collect::<Vec<_>>()
        );
    }

    fn weights() -> Result<Store, Box<dyn std::error::Error>> {
        let (base, _, _) = load()?;
        let mut tensors = BTreeMap::new();
        for layer in 0..40 {
            for (name, tensor) in &base.tensors {
                if name.starts_with("layers.2.") {
                    tensors.insert(
                        name.replacen("layers.2.", &format!("layers.{layer}."), 1),
                        Tensor {
                            shape: tensor.shape.clone(),
                            values: tensor.values.clone(),
                        },
                    );
                }
            }
        }
        Ok(Store {
            tensors,
            reads: RefCell::new(Vec::new()),
        })
    }
    fn inputs(layer: usize, start: usize, count: usize) -> Vec<f32> {
        (start..start + count)
            .flat_map(|pos| {
                [0.25, -0.5, 1.0, 2.0]
                    .map(|v| v + layer as f32 * 0.03 + (pos as f32 * 0.4).sin() * 0.2)
            })
            .collect()
    }
    fn run(
        runtime: &mut AttentionRuntime<'_>,
        chunks: &[usize],
    ) -> Result<BTreeMap<usize, Vec<f32>>, CpuError> {
        let mut start = 0;
        let mut outputs = BTreeMap::<usize, Vec<f32>>::new();
        for count in chunks {
            for layer in 0..40 {
                outputs
                    .entry(layer)
                    .or_default()
                    .extend(runtime.forward_chunk(layer, start, &inputs(layer, start, *count))?);
            }
            start += count;
        }
        Ok(outputs)
    }
    #[derive(Deserialize)]
    struct Fixture {
        outputs: BTreeMap<usize, Vec<f32>>,
    }
    #[test]
    fn integrated_40_layer_attention_schedule_matches_numpy()
    -> Result<(), Box<dyn std::error::Error>> {
        let w = weights()?;
        let mut runtime = AttentionRuntime::new(&w, tiny(), 16, Arithmetic::Float32)?;
        let got = run(&mut runtime, &[7])?;
        let expected: Fixture = serde_json::from_str(include_str!(
            "../testdata/deepseek_v41_cpu/shared_state.json"
        ))?;
        for (layer, ys) in expected.outputs {
            for (a, b) in got[&layer].iter().zip(ys) {
                assert!((a - b).abs() < 2e-5, "layer {layer}: {a} != {b}");
            }
        }
        assert_eq!(
            runtime.summary().sources,
            vec![(2, 3, 3, 1), (8, 3, 3, 1), (14, 3, 3, 1), (20, 7, 7, 0)]
        );
        assert_eq!(runtime.windows[0].rows.len(), 4);
        Ok(())
    }
    #[test]
    fn chunk_decode_reuse_and_reset_are_equivalent() -> Result<(), Box<dyn std::error::Error>> {
        let w = weights()?;
        let mut whole = AttentionRuntime::new(&w, tiny(), 16, Arithmetic::Float32)?;
        let expected = run(&mut whole, &[7])?;
        let mut chunked = AttentionRuntime::new(&w, tiny(), 16, Arithmetic::Float32)?;
        let got = run(&mut chunked, &[3, 1, 2, 1])?;
        for layer in 0..40 {
            for (a, b) in expected[&layer].iter().zip(&got[&layer]) {
                assert!((a - b).abs() < 1e-6);
            }
        }
        assert_eq!(whole.summary(), chunked.summary());
        chunked.reset();
        assert_eq!(chunked.summary().next_position, 0);
        assert_eq!(run(&mut chunked, &[7])?, expected);
        let fresh = AttentionRuntime::new(&w, tiny(), 16, Arithmetic::Float32)?;
        assert!(fresh.summary().sources.iter().all(|x| x.1 == 0));
        Ok(())
    }
    #[test]
    fn exact_owners_and_source_only_projection_reads() -> Result<(), Box<dyn std::error::Error>> {
        for (lo, hi, kv, index) in [
            (2, 7, 2, 2),
            (8, 13, 8, 8),
            (14, 19, 14, 14),
            (20, 23, 20, 20),
            (24, 27, 20, 24),
            (28, 31, 20, 28),
            (32, 35, 20, 32),
            (36, 39, 20, 36),
        ] {
            for layer in lo..=hi {
                let o = LayerOwners::for_layer(layer)?;
                assert_eq!(o.kv, Some(kv));
                assert_eq!(o.index, Some(index));
            }
        }
        let w = weights()?;
        let mut runtime = AttentionRuntime::new(&w, tiny(), 16, Arithmetic::Float32)?;
        run(&mut runtime, &[3])?;
        for (name, _) in w.reads.borrow().iter() {
            let layer: usize = name.split('.').nth(1).ok_or("layer name")?.parse()?;
            if name.contains("compressor.") || name.contains("indexer.wk.") {
                assert!(KV_SOURCES.contains(&layer));
            }
            if name.contains("indexer.wq_b.") {
                assert!(INDEX_SOURCES.contains(&layer));
            }
        }
        Ok(())
    }

    #[test]
    fn later_indexer_consumes_the_layer_20_candidate_mask() -> Result<(), Box<dyn std::error::Error>>
    {
        let w = weights()?;
        let mut r = AttentionRuntime::new(&w, tiny(), 8, Arithmetic::Float32)?;
        for l in 0..=20 {
            r.forward_chunk(l, 0, &inputs(l, 0, 7))?;
        }
        let chunk = r.chunk.as_mut().ok_or("chunk")?;
        let masks = Arc::make_mut(chunk.candidates.as_mut().ok_or("candidate batch")?);
        for mask in masks {
            mask.fill(false);
            mask[0] = true;
        }
        for l in 21..=24 {
            r.forward_chunk(l, 0, &inputs(l, 0, 7))?;
        }
        let rows = r
            .chunk
            .as_ref()
            .and_then(|c| c.selections.get(&24))
            .ok_or("layer 24 selections")?;
        for row in rows.iter() {
            assert_eq!(
                row.iter().copied().filter(|i| *i >= 0).collect::<Vec<_>>(),
                vec![0]
            );
        }
        Ok(())
    }
    struct Fail<'a> {
        inner: &'a Store,
        enabled: Cell<bool>,
    }
    impl WeightSource for Fail<'_> {
        fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            if self.enabled.get() && name == "layers.2.attn.wo_b.weight" {
                return Err(CpuError::Invalid("injected output projection failure"));
            }
            self.inner.matrix(name)
        }
        fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
            self.inner.vector(name)
        }
    }
    #[test]
    fn layer_failure_order_and_capacity_leave_state_unchanged()
    -> Result<(), Box<dyn std::error::Error>> {
        let w = weights()?;
        let source = Fail {
            inner: &w,
            enabled: Cell::new(false),
        };
        let mut r = AttentionRuntime::new(&source, tiny(), 7, Arithmetic::Float32)?;
        assert!(r.forward_chunk(1, 0, &inputs(1, 0, 3)).is_err());
        r.forward_chunk(0, 0, &inputs(0, 0, 3))?;
        r.forward_chunk(1, 0, &inputs(1, 0, 3))?;
        let before = r.summary();
        source.enabled.set(true);
        assert!(r.forward_chunk(2, 0, &inputs(2, 0, 3)).is_err());
        assert_eq!(r.summary(), before);
        source.enabled.set(false);
        r.forward_chunk(2, 0, &inputs(2, 0, 3))?;
        assert!(r.forward_chunk(3, 1, &inputs(3, 1, 3)).is_err());
        r.reset();
        let before = r.summary();
        assert!(r.forward_chunk(0, 0, &inputs(0, 0, 8)).is_err());
        assert_eq!(r.summary(), before);
        Ok(())
    }
    struct ReferenceWeights;
    struct ReferenceMatrix {
        rows: usize,
        cols: usize,
        precision: MatrixPrecision,
    }
    impl MatrixRows for ReferenceMatrix {
        fn dimensions(&self) -> (usize, usize) {
            (self.rows, self.cols)
        }
        fn precision(&self) -> MatrixPrecision {
            self.precision
        }
        fn row(&self, i: usize) -> Result<Vec<f32>, CpuError> {
            Ok((0..self.cols)
                .map(|j| if j == i % self.cols { 0.5 } else { 0.015625 })
                .collect())
        }
    }
    impl WeightSource for ReferenceWeights {
        fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
            Ok(if name.ends_with("attn_sink") {
                vec![0.25; 2]
            } else {
                vec![1.0; 32]
            })
        }
        fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            let (rows, cols) =
                if name.ends_with("attn.wq_b.weight") || name.ends_with("indexer.wq_b.weight") {
                    (64, 32)
                } else if name.ends_with("weights_proj.weight") {
                    (2, 32)
                } else if name.ends_with("wo_a.weight") {
                    (64, 32)
                } else if name.ends_with("wo_b.weight") {
                    (32, 64)
                } else {
                    (32, 32)
                };
            let precision = if name.contains("compressor.")
                || name.contains("indexer.wk.")
                || name.contains("weights_proj.")
            {
                MatrixPrecision::Bf16
            } else {
                MatrixPrecision::Fp8
            };
            Ok(Box::new(ReferenceMatrix {
                rows,
                cols,
                precision,
            }))
        }
    }
    #[test]
    fn bf16_path_runs_all_owners_and_matches_chunk_replay() -> Result<(), CpuError> {
        let g = Geometry {
            hidden: 32,
            heads: 2,
            head_dim: 32,
            q_rank: 32,
            o_groups: 2,
            o_rank: 32,
            index_heads: 2,
            index_dim: 32,
            rope_dim: 8,
            window: 4,
            index_top_k: 3,
            ..Geometry::flash()
        };
        let mut whole = AttentionRuntime::new(&ReferenceWeights, g, 8, Arithmetic::ReferenceBf16)?;
        let mut chunked = whole.clone();
        let x: Vec<_> = (0..160).map(|i| (i % 32) as f32 * 0.125 + 0.25).collect();
        let mut outputs = Vec::new();
        for l in 0..40 {
            outputs.push(whole.forward_chunk(l, 0, &x)?);
        }
        let mut actual = vec![Vec::new(); 40];
        for (start, count) in [(0, 3), (3, 1), (4, 1)] {
            for (l, out) in actual.iter_mut().enumerate() {
                out.extend(chunked.forward_chunk(
                    l,
                    start,
                    &x[start * 32..(start + count) * 32],
                )?);
            }
        }
        assert_eq!(outputs, actual);
        assert_eq!(whole.summary(), chunked.summary());
        assert_eq!(
            whole.summary().sources,
            vec![(2, 2, 2, 1), (8, 2, 2, 1), (14, 2, 2, 1), (20, 5, 5, 0)]
        );
        Ok(())
    }
}
