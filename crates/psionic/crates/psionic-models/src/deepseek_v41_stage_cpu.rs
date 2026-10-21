//! DeepSeek-V4.1 layer ranges on the CPU, for pipeline stages.
//!
//! [`crate::deepseek_v41_cpu::transformer::CpuTransformer`] runs all 40 layers
//! with request state that it keeps private. A stage runs only its own layers
//! and receives, in each step's [`StageMessage`], the state that earlier
//! layers produced: the residual streams and pre-mix, rows of the KV source
//! and indexer keys that its layers reuse, the latest index selection, and
//! layer 20's candidate blocks. [`LayerRangeRuntime`] is the same computation
//! as the full transformer, cut at the stage's boundaries, with that state
//! passed in and out explicitly. Splitting a model this way gives the same
//! logits as the unsplit transformer, bit for bit, in every arithmetic mode.
//!
//! [`V41CpuStageExecutor`] adapts a runtime to W9's
//! [`StageExecutor`](crate::stage_state::StageExecutor): it maps the sections
//! of an admitted message onto a [`RangeStep`], and the [`RangeOutput`] onto
//! the sections the next stage's contract requires, or onto logits on the
//! last stage. Every section travels as `F32`, so the wire adds no rounding.
//!
//! The math follows `deepseek_v41_cpu::shared_state` and `transformer`
//! (reimplemented from the public MIT-licensed reference at revision
//! `1bae0e1088993b9b82de08f59de5be5a34e42170`); keep the two in step.

use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
    sync::Arc,
};

use crate::{
    deepseek_v41_cpu::{
        CpuError,
        attention::{self, CompressionState},
        block::TextBlock,
        engram::{CompressedTokenMap, NgramHashState},
        finite, matvec, product,
        projection::{self, AttentionProjections, WeightSource, compress_ratio},
        quantization::{self, Arithmetic, Quantization},
        rope::{Rope, RopeConfig, group_position},
        shape,
        shared_state::{KV_SOURCES, LayerOwners},
        transformer::TransformerConfig,
    },
    deepseek_v41_stage::{
        CANDIDATE_SOURCE, ENGRAM_LAYERS, V41_LAYERS, V41StageGeometry, boundary_sections, kv_owner,
        stage_contract,
    },
    stage_state::{
        RowRule, Section, SectionKind, StageContract, StageExecutor, StageMessage, WireDtype,
    },
};

/// Candidate blocks layer 20 keeps per token, as the reference does.
pub const CANDIDATE_BLOCKS: usize = 2048;
/// Compressed rows in one candidate block.
pub const CANDIDATE_BLOCK_ROWS: usize = 8;
/// Pads an index selection row to its fixed width on the wire.
pub const SELECTION_PAD: i32 = i32::MIN;
/// Pads a candidate row and Engram history on the wire.
pub const ID_PAD: u32 = u32::MAX;

#[derive(Clone, Default)]
struct Window {
    rows: VecDeque<Arc<[f32]>>,
    next_position: usize,
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

/// A KV source this stage reads. A source layer in the stage's range owns
/// its compressor; an earlier source arrives as rows from the stage before.
#[derive(Clone)]
struct Source {
    compressor: Option<CompressionState>,
    kv: Arc<Vec<f32>>,
    index_keys: Arc<Vec<f32>>,
}

/// State that layers of one step share: index selections and layer 20's
/// candidates, one row per token.
#[derive(Clone, Default)]
struct Chunk {
    selections: BTreeMap<usize, Vec<Vec<i32>>>,
    candidates: Option<Vec<Vec<bool>>>,
}

/// Rows a step appended to one KV source.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceRows {
    /// Row index of the first KV row in `kv`.
    pub first_kv_row: u64,
    /// Compressed KV rows, `head_dim` values each.
    pub kv: Vec<f32>,
    /// Row index of the first key row in `index_keys`.
    pub first_key_row: u64,
    /// Indexer key rows, `index_dim` values each.
    pub index_keys: Vec<f32>,
}

/// One step's input to a layer range.
#[derive(Clone, Debug, Default)]
pub struct RangeStep {
    /// Position of the first token.
    pub position: usize,
    /// The step's tokens.
    pub tokens: Vec<u32>,
    /// Residual streams per token, `streams * hidden` values each. Empty on
    /// the stage that holds the embeddings.
    pub residuals: Vec<Vec<f32>>,
    /// Pre-mix weights per token, `streams` values each. Empty on the stage
    /// that holds the embeddings.
    pub pre_mixes: Vec<Vec<f32>>,
    /// New rows of KV sources owned by earlier stages.
    pub source_rows: BTreeMap<usize, SourceRows>,
    /// Index selections of earlier index-source layers, per token.
    pub selections: BTreeMap<usize, Vec<Vec<i32>>>,
    /// Layer 20's candidate masks per token, when an earlier stage ran it.
    pub candidates: Option<Vec<Vec<bool>>>,
}

/// One step's output from a layer range.
#[derive(Clone, Debug, Default)]
pub struct RangeOutput {
    /// Residual streams per token after the range's last layer.
    pub residuals: Vec<Vec<f32>>,
    /// Pre-mix weights per token after the range's last layer.
    pub pre_mixes: Vec<Vec<f32>>,
    /// Rows this step appended to each KV source the stage holds.
    pub source_rows: BTreeMap<usize, SourceRows>,
    /// Index selections per token of every index source the step used.
    pub selections: BTreeMap<usize, Vec<Vec<i32>>>,
    /// Layer 20's candidate masks per token, when the step had them.
    pub candidates: Option<Vec<Vec<bool>>>,
    /// Up to three tokens before the step, oldest first.
    pub history: Vec<u32>,
    /// Full-vocabulary logits of the step's last token, on the head stage.
    pub logits: Option<Vec<f32>>,
}

/// The layers a stage runs and the model pieces it holds.
#[derive(Clone, Debug)]
pub struct RangeConfig {
    /// The model's configuration.
    pub transformer: TransformerConfig,
    /// Decoder layers, end exclusive.
    pub layers: Range<usize>,
    /// Longest request, prompt included.
    pub max_context: usize,
    /// Numerical mode.
    pub arithmetic: Arithmetic,
}

impl RangeConfig {
    /// Whether the range embeds tokens.
    #[must_use]
    pub fn embeddings(&self) -> bool {
        self.layers.start == 0
    }

    /// Whether the range ends with the final norm and output head.
    #[must_use]
    pub fn head(&self) -> bool {
        self.layers.end == V41_LAYERS as usize
    }
}

/// One request's state over a contiguous range of decoder layers.
#[derive(Clone)]
pub struct LayerRangeRuntime<'a> {
    weights: &'a dyn WeightSource,
    config: RangeConfig,
    map: Arc<CompressedTokenMap>,
    windows: BTreeMap<usize, Window>,
    sources: BTreeMap<usize, Source>,
    hash: Option<NgramHashState>,
    recent: VecDeque<u32>,
    position: usize,
    /// The last forward's selected experts, per layer, six per token in
    /// token order (G1 v2 router checks, #12405).
    routes: BTreeMap<usize, Vec<u32>>,
}

impl<'a> LayerRangeRuntime<'a> {
    /// A runtime for `config.layers`, after checking every weight the range
    /// reads.
    ///
    /// # Errors
    ///
    /// Refuses an empty or out-of-model range, a context outside 1–1,048,576,
    /// and missing or misshapen weights.
    pub fn new(
        weights: &'a dyn WeightSource,
        config: RangeConfig,
        map: Arc<CompressedTokenMap>,
    ) -> Result<Self, CpuError> {
        let layers = config.layers.clone();
        if layers.is_empty() || layers.end > V41_LAYERS as usize {
            return Err(CpuError::Invalid("stage layer range"));
        }
        if config.max_context == 0 || config.max_context > 1_048_576 {
            return Err(CpuError::Invalid("stage context capacity"));
        }
        let c = &config.transformer;
        c.block.validate()?;
        if map.ids().len() != c.vocabulary || map.vocab_size() != c.engram.compressed_vocab_size {
            return Err(CpuError::Invalid("stage token map"));
        }
        let width = c.block.geometry.hidden;
        if config.embeddings()
            && weights.matrix("embed.weight")?.dimensions() != (c.vocabulary, width)
        {
            return Err(CpuError::Invalid("embedding checkpoint dimensions"));
        }
        if config.head() {
            if weights.matrix("head.weight")?.dimensions() != (c.vocabulary, width) {
                return Err(CpuError::Invalid("head checkpoint dimensions"));
            }
            shape("final norm", &weights.vector("norm.weight")?, width)?;
        }
        for layer in layers.clone() {
            TextBlock::new(weights, layer, c.block, config.arithmetic)?;
        }
        for layer in &c.engram.layers {
            if layers.contains(&(layer.layer_id as usize))
                && weights
                    .matrix(&format!("layers.{}.engram.embed.weight", layer.layer_id))?
                    .dimensions()
                    != (
                        usize::try_from(layer.table_rows)
                            .map_err(|_| CpuError::Invalid("Engram table row overflow"))?,
                        c.engram.head_dim,
                    )
            {
                return Err(CpuError::Invalid("stage Engram table dimensions"));
            }
        }
        let g = c.block.geometry;
        let mut sources = BTreeMap::new();
        for source in KV_SOURCES {
            let owned = layers.contains(&source);
            let imported = source < layers.start
                && kv_owner(layers.start as u32).is_some_and(|s| s as usize == source);
            if owned || imported {
                sources.insert(
                    source,
                    Source {
                        compressor: if owned {
                            Some(
                                CompressionState::new(compress_ratio(source)?, g.head_dim)?
                                    .with_arithmetic(config.arithmetic),
                            )
                        } else {
                            None
                        },
                        kv: Arc::new(Vec::new()),
                        index_keys: Arc::new(Vec::new()),
                    },
                );
            }
        }
        let hash = if ENGRAM_LAYERS
            .iter()
            .any(|l| layers.contains(&(*l as usize)))
        {
            Some(NgramHashState::new(
                c.engram.clone(),
                map.get(c.pad_token)?,
            )?)
        } else {
            None
        };
        Ok(Self {
            weights,
            windows: layers.map(|l| (l, Window::default())).collect(),
            config,
            map,
            sources,
            hash,
            recent: VecDeque::new(),
            position: 0,
            routes: BTreeMap::new(),
        })
    }

    /// The range's configuration.
    #[must_use]
    pub fn config(&self) -> &RangeConfig {
        &self.config
    }

    /// The next position this runtime expects.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// The KV sources this range holds, with their row counts.
    #[must_use]
    pub fn source_rows(&self) -> BTreeMap<usize, (usize, usize)> {
        let g = self.config.transformer.block.geometry;
        self.sources
            .iter()
            .map(|(l, s)| {
                (
                    *l,
                    (s.kv.len() / g.head_dim, s.index_keys.len() / g.index_dim),
                )
            })
            .collect()
    }

    /// Drops the request's state.
    pub fn reset(&mut self) {
        self.windows
            .values_mut()
            .for_each(|w| *w = Window::default());
        for source in self.sources.values_mut() {
            if let Some(c) = &mut source.compressor {
                c.reset();
            }
            source.kv = Arc::new(Vec::new());
            source.index_keys = Arc::new(Vec::new());
        }
        if let Some(hash) = &mut self.hash {
            hash.reset();
        }
        self.recent.clear();
        self.position = 0;
    }

    /// Runs one step through the range. A failure changes no state.
    ///
    /// # Errors
    ///
    /// Refuses tokens, positions, or state that don't fit the range, and any
    /// failure of the model's components.
    pub fn forward(&mut self, step: RangeStep) -> Result<RangeOutput, CpuError> {
        let mut next = self.clone();
        let output = next.run(step)?;
        *self = next;
        Ok(output)
    }

    fn finish(&self, values: Vec<f32>) -> Result<Vec<f32>, CpuError> {
        if self.config.arithmetic.is_bf16() {
            quantization::round_bf16(&values)
        } else {
            Ok(values)
        }
    }

    fn run(&mut self, step: RangeStep) -> Result<RangeOutput, CpuError> {
        let c = self.config.transformer.clone();
        let g = c.block.geometry;
        let width = g.hidden;
        let streams = c.block.hyper.streams;
        let tokens = &step.tokens;
        let count = tokens.len();
        if count == 0
            || tokens.iter().any(|t| *t as usize >= c.vocabulary)
            || step.position != self.position
            || self
                .position
                .checked_add(count)
                .is_none_or(|n| n > self.config.max_context)
        {
            return Err(CpuError::Invalid("stage tokens, position, or context"));
        }
        let history: Vec<u32> = self.recent.iter().copied().collect();
        let hashes = match &mut self.hash {
            Some(hash) => Some(hash.push_tokens(self.position, tokens, None, &self.map)?),
            None => None,
        };
        let (mut residuals, mut pre_mixes) = if self.config.embeddings() {
            let mut residuals = Vec::with_capacity(count);
            for token in tokens {
                let embedded =
                    self.finish(projection::embedding(self.weights, *token as usize, width)?)?;
                residuals.push(embedded.repeat(streams));
            }
            let mut pre = vec![0.0; streams];
            pre[0] = 1.0;
            (residuals, vec![pre; count])
        } else {
            if step.residuals.len() != count || step.pre_mixes.len() != count {
                return Err(CpuError::Invalid("stage residuals or pre-mixes per token"));
            }
            for (r, p) in step.residuals.iter().zip(&step.pre_mixes) {
                shape("stage residual", r, product(streams, width)?)?;
                shape("stage pre-mix", p, streams)?;
            }
            (step.residuals, step.pre_mixes)
        };
        let before: BTreeMap<usize, (usize, usize)> = self
            .sources
            .iter()
            .map(|(l, s)| (*l, (s.kv.len(), s.index_keys.len())))
            .collect();
        for (layer, rows) in step.source_rows {
            let source = self
                .sources
                .get_mut(&layer)
                .filter(|s| s.compressor.is_none())
                .ok_or(CpuError::Invalid(
                    "rows for a KV source this stage doesn't import",
                ))?;
            if rows.first_kv_row as usize * g.head_dim != source.kv.len()
                || rows.first_key_row as usize * g.index_dim != source.index_keys.len()
                || !rows.kv.len().is_multiple_of(g.head_dim)
                || !rows.index_keys.len().is_multiple_of(g.index_dim)
            {
                return Err(CpuError::Invalid("KV source rows out of order"));
            }
            finite("imported KV rows", &rows.kv)?;
            finite("imported index keys", &rows.index_keys)?;
            Arc::make_mut(&mut source.kv).extend(rows.kv);
            Arc::make_mut(&mut source.index_keys).extend(rows.index_keys);
        }
        let mut chunk = Chunk {
            selections: step.selections,
            candidates: step.candidates,
        };
        for rows in chunk.selections.values() {
            if rows.len() != count {
                return Err(CpuError::Invalid("index selections per token"));
            }
        }
        if chunk.candidates.as_ref().is_some_and(|r| r.len() != count) {
            return Err(CpuError::Invalid("candidate masks per token"));
        }
        self.routes.clear();
        for layer in self.config.layers.clone() {
            let block = TextBlock::new(self.weights, layer, c.block, self.config.arithmetic)?
                .with_reference_rows(count);
            if let Some(slot) = c
                .engram
                .layers
                .iter()
                .position(|l| l.layer_id as usize == layer)
            {
                let hashes = hashes
                    .as_ref()
                    .ok_or(CpuError::Invalid("missing Engram hashes"))?;
                for (i, residual) in residuals.iter_mut().enumerate() {
                    *residual = block.apply_engram(
                        residual,
                        &hashes[i].layers[slot],
                        c.engram.layers[slot].table_rows,
                        c.engram.head_dim,
                    )?;
                }
            }
            let mut mixes = Vec::with_capacity(count);
            let mut normalized = Vec::with_capacity(product(count, width)?);
            for (residual, pre) in residuals.iter().zip(&pre_mixes) {
                let input = block.begin_attention(residual, pre)?;
                normalized.extend(input.normalized);
                mixes.push(input.mixes);
            }
            let attention = self.attend(layer, self.position, &normalized, &mut chunk)?;
            for i in 0..count {
                let attended = block.finish_attention(
                    &attention[i * width..(i + 1) * width],
                    &residuals[i],
                    &mixes[i],
                )?;
                let output = block.feed_forward(&attended, &mixes[i].pre)?;
                self.routes
                    .entry(layer)
                    .or_default()
                    .extend(output.routing.indices.iter().map(|id| *id as u32));
                residuals[i] = output.residual;
                pre_mixes[i] = output.next_pre;
            }
        }
        let logits = if self.config.head() {
            let last = count - 1;
            let (_, normalized) = projection::final_input_with_reference_rows(
                self.weights,
                &residuals[last],
                &pre_mixes[last],
                width,
                g.norm_epsilon,
                self.config.arithmetic,
                count,
            )?;
            let row = matvec(self.weights.matrix("head.weight")?.as_ref(), &normalized)?;
            shape("stage logits", &row, c.vocabulary)?;
            Some(row)
        } else {
            None
        };
        let mut source_rows = BTreeMap::new();
        for (layer, source) in &self.sources {
            let (kv, keys) = before.get(layer).copied().unwrap_or_default();
            source_rows.insert(
                *layer,
                SourceRows {
                    first_kv_row: (kv / g.head_dim) as u64,
                    kv: source.kv[kv..].to_vec(),
                    first_key_row: (keys / g.index_dim) as u64,
                    index_keys: source.index_keys[keys..].to_vec(),
                },
            );
        }
        for token in tokens {
            if self.recent.len() + 1 == c.engram.max_ngram_size.max(1) {
                self.recent.pop_front();
            }
            if c.engram.max_ngram_size > 1 {
                self.recent.push_back(*token);
            }
        }
        self.position += count;
        Ok(RangeOutput {
            residuals,
            pre_mixes,
            source_rows,
            selections: chunk.selections,
            candidates: chunk.candidates,
            history,
            logits,
        })
    }

    fn cache_form(&self, value: &[f32], kind: Quantization) -> Result<Vec<f32>, CpuError> {
        if self.config.arithmetic.is_bf16() {
            quantization::inplace_bf16(value, kind)
        } else {
            Ok(value.to_vec())
        }
    }

    /// One layer's attention for every token of the step, as
    /// `AttentionRuntime::run_layer` computes it.
    fn attend(
        &mut self,
        layer: usize,
        start: usize,
        input: &[f32],
        chunk: &mut Chunk,
    ) -> Result<Vec<f32>, CpuError> {
        let g = self.config.transformer.block.geometry;
        let arithmetic = self.config.arithmetic;
        finite("attention layer input", input)?;
        let ratio = compress_ratio(layer)?;
        let owners = LayerOwners::for_layer(layer)?;
        let p = AttentionProjections::new(self.weights, layer, g)?
            .with_arithmetic(arithmetic)?
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
        if let Some(compressor) = self
            .sources
            .get_mut(&layer)
            .and_then(|s| s.compressor.as_mut())
        {
            compressor.set_reference_rows((input.len() / g.hidden / ratio.max(1)).max(1));
        }
        let mut selected_rows = Vec::new();
        let mut candidate_rows = Vec::new();
        let mut outputs = Vec::with_capacity(input.len());
        for (i, x) in input.chunks_exact(g.hidden).enumerate() {
            let position = start + i;
            let x = if arithmetic.is_bf16() {
                quantization::round_bf16(x)?
            } else {
                x.to_vec()
            };
            let mut q = p.query(&x)?;
            let mut window = p.window_kv(&x)?;
            rope.apply_tail(&mut q.heads, g.head_dim, position, false)?;
            q.heads = p.finish(q.heads)?;
            rope.apply_tail(&mut window, g.head_dim, position, false)?;
            window = p.finish(window)?;
            let window = self.cache_form(&window, Quantization::Fp8)?;
            self.windows
                .get_mut(&layer)
                .ok_or(CpuError::Invalid("layer outside the stage"))?
                .push(position, window, g.window)?;
            if let Some(norm) = &norm {
                let (value, gate) = p.compressor_inputs(&x)?;
                let latent = self
                    .sources
                    .get_mut(&layer)
                    .and_then(|s| s.compressor.as_mut())
                    .ok_or(CpuError::Invalid("missing KV source"))?
                    .push(position, &value, gate.as_deref(), norm, g.norm_epsilon)?;
                if let Some(mut latent) = latent {
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
            let mut kv: Vec<f32> = self.windows[&layer]
                .rows
                .iter()
                .flat_map(|r| r.iter().copied())
                .collect();
            let window_rows = kv.len() / g.head_dim;
            let mut indices: Vec<i32> = if arithmetic == Arithmetic::ReferenceBf16Tp8 {
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
                let scores_here = owners.index == Some(layer);
                if source.kv.len() < kv_len || (scores_here && source.index_keys.len() < key_len) {
                    return Err(CpuError::Invalid(
                        "shared KV source has not produced this position",
                    ));
                }
                let mut selected = if scores_here {
                    let mut query = p.index_query(&q.low_rank)?;
                    rope.apply_tail(&mut query, g.index_dim, position, false)?;
                    let query = self.cache_form(&p.finish(query)?, Quantization::IndexFp4)?;
                    let weights = p.index_weights(&x)?;
                    let scores = if arithmetic == Arithmetic::Float32 {
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
                    if layer == CANDIDATE_SOURCE as usize {
                        candidate_rows.push(attention::candidate_mask(
                            &scores,
                            visible,
                            CANDIDATE_BLOCKS,
                            CANDIDATE_BLOCK_ROWS,
                        )?);
                    }
                    let candidates = if layer > CANDIDATE_SOURCE as usize {
                        Some(
                            chunk
                                .candidates
                                .as_ref()
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
                    chunk
                        .selections
                        .get(&owners.index.unwrap_or(40))
                        .and_then(|rows| rows.get(i))
                        .ok_or(CpuError::Invalid("missing shared index selection"))?
                        .clone()
                };
                if arithmetic == Arithmetic::ReferenceBf16Tp8 {
                    selected.resize(
                        ((start + input.len() / g.hidden) / ratio).min(g.index_top_k),
                        -1,
                    );
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
            let heads = if arithmetic == Arithmetic::Float32 {
                attention::sparse_attention(
                    &q.heads,
                    g.heads,
                    g.head_dim,
                    &kv,
                    &indices,
                    &sinks,
                    (g.head_dim as f32).sqrt().recip(),
                )?
            } else if arithmetic == Arithmetic::ReferenceBf16Tp8 {
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
            let mut heads = heads;
            rope.apply_tail(&mut heads, g.head_dim, position, true)?;
            let heads = p.finish(heads)?;
            let low_rank = p.output_low_rank(&heads)?;
            outputs.extend(p.output_from_low_rank(&low_rank)?);
        }
        if owners.index == Some(layer) {
            chunk.selections.insert(layer, selected_rows);
        }
        if layer == CANDIDATE_SOURCE as usize {
            chunk.candidates = Some(candidate_rows);
        }
        Ok(outputs)
    }
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

// ---------------------------------------------------------------------------
// Stage executor
// ---------------------------------------------------------------------------

/// The wire geometry of a model configuration: every section as `F32`.
#[must_use]
pub fn wire_geometry(config: &TransformerConfig) -> V41StageGeometry {
    let g = config.block.geometry;
    V41StageGeometry {
        streams: config.block.hyper.streams as u32,
        hidden: g.hidden as u32,
        head_dim: g.head_dim as u32,
        index_dim: g.index_dim as u32,
        index_top_k: g.index_top_k as u32,
        candidate_blocks: CANDIDATE_BLOCKS as u32,
        candidate_block_rows: CANDIDATE_BLOCK_ROWS as u32,
        max_ngram: config.engram.max_ngram_size as u32,
        vocabulary: config.vocabulary as u32,
        residual_dtype: WireDtype::F32,
        cache_dtype: WireDtype::F32,
    }
}

/// Why the CPU stage executor failed a step.
#[derive(Debug, thiserror::Error)]
pub enum StageCpuError {
    /// The model refused the step.
    #[error(transparent)]
    Model(#[from] CpuError),
    /// A section couldn't be read or built.
    #[error("stage section: {0}")]
    Section(String),
}

/// W9's [`StageExecutor`] over a [`LayerRangeRuntime`]: one stage of a
/// V4.1 group on the CPU.
pub struct V41CpuStageExecutor<'a> {
    runtime: LayerRangeRuntime<'a>,
    geometry: V41StageGeometry,
    stage: u16,
    next: Option<StageContract>,
    request: Option<(String, u32)>,
}

impl<'a> V41CpuStageExecutor<'a> {
    /// An executor for stage `stage` running `runtime`'s layers.
    #[must_use]
    pub fn new(runtime: LayerRangeRuntime<'a>, stage: u16) -> Self {
        let geometry = wire_geometry(&runtime.config.transformer);
        let layers = runtime.config.layers.clone();
        let next = (!runtime.config.head()).then(|| {
            stage_contract(
                &geometry,
                stage + 1,
                layers.end as u32..layers.end as u32 + 1,
            )
        });
        Self {
            runtime,
            geometry,
            stage,
            next,
            request: None,
        }
    }

    /// This stage's inbound contract.
    #[must_use]
    pub fn contract(&self) -> StageContract {
        let layers = &self.runtime.config.layers;
        stage_contract(
            &self.geometry,
            self.stage,
            layers.start as u32..layers.end as u32,
        )
    }

    /// The wire geometry.
    #[must_use]
    pub fn geometry(&self) -> V41StageGeometry {
        self.geometry
    }

    /// The last step's selected experts per layer, six per token.
    #[must_use]
    pub fn routes(&self) -> &BTreeMap<usize, Vec<u32>> {
        &self.runtime.routes
    }

    /// The runtime.
    #[must_use]
    pub fn runtime(&self) -> &LayerRangeRuntime<'a> {
        &self.runtime
    }

    fn input(&self, message: &StageMessage) -> Result<RangeStep, StageCpuError> {
        let bad = |e: crate::stage_state::StageRefusal| StageCpuError::Section(e.to_string());
        let g = self.geometry;
        let count = message.tokens.len();
        let mut step = RangeStep {
            position: usize::try_from(message.position)
                .map_err(|_| StageCpuError::Section("position".into()))?,
            tokens: message.tokens.clone(),
            ..RangeStep::default()
        };
        for section in &message.sections {
            match section.kind {
                SectionKind::Residual => {
                    let values = section.to_f32().map_err(bad)?;
                    let row = (g.streams * g.hidden) as usize;
                    step.residuals = values.chunks(row).map(<[f32]>::to_vec).collect();
                }
                SectionKind::PreMix => {
                    let values = section.to_f32().map_err(bad)?;
                    step.pre_mixes = values
                        .chunks(g.streams as usize)
                        .map(<[f32]>::to_vec)
                        .collect();
                }
                SectionKind::SourceKv { layer } => {
                    let rows = step.source_rows.entry(layer as usize).or_default();
                    rows.first_kv_row = section.first_row;
                    rows.kv = section.to_f32().map_err(bad)?;
                }
                SectionKind::SourceIndexKeys { layer } => {
                    let rows = step.source_rows.entry(layer as usize).or_default();
                    rows.first_key_row = section.first_row;
                    rows.index_keys = section.to_f32().map_err(bad)?;
                }
                SectionKind::IndexSelection { layer } => {
                    let values = section.to_i32().map_err(bad)?;
                    step.selections.insert(
                        layer as usize,
                        values
                            .chunks(g.index_top_k as usize)
                            .map(|row| {
                                row.iter()
                                    .copied()
                                    .filter(|id| *id != SELECTION_PAD)
                                    .collect()
                            })
                            .collect(),
                    );
                }
                SectionKind::Candidates { .. } => {
                    let values = section.to_u32().map_err(bad)?;
                    let mut masks = Vec::with_capacity(count);
                    for (i, row) in values.chunks(g.candidate_blocks as usize).enumerate() {
                        // Layer 20 scores every compressed row up to the token.
                        let visible = step.position + i + 1;
                        let mut mask = vec![false; visible];
                        for block in row.iter().filter(|b| **b != ID_PAD) {
                            let first = *block as usize * CANDIDATE_BLOCK_ROWS;
                            for slot in mask.iter_mut().skip(first).take(CANDIDATE_BLOCK_ROWS) {
                                *slot = true;
                            }
                        }
                        masks.push(mask);
                    }
                    step.candidates = Some(masks);
                }
                // The range keeps its own n-gram history; the section lets a
                // stage check it.
                SectionKind::EngramHistory
                | SectionKind::SourceValue { .. }
                | SectionKind::Logits => {}
            }
        }
        Ok(step)
    }

    fn output(&self, output: RangeOutput, count: usize) -> Result<Vec<Section>, StageCpuError> {
        let bad = |e: crate::stage_state::StageRefusal| StageCpuError::Section(e.to_string());
        let g = self.geometry;
        let tokens = count as u32;
        let Some(next) = &self.next else {
            let logits = output
                .logits
                .ok_or_else(|| StageCpuError::Section("the head stage made no logits".into()))?;
            return Ok(vec![
                Section::floats(
                    SectionKind::Logits,
                    WireDtype::F32,
                    vec![1, g.vocabulary],
                    0,
                    &logits,
                )
                .map_err(bad)?,
            ]);
        };
        let mut sections = Vec::with_capacity(next.sections.len());
        for spec in &next.sections {
            let section = match spec.kind {
                SectionKind::Residual => Section::floats(
                    spec.kind,
                    spec.dtype,
                    vec![tokens, g.streams, g.hidden],
                    0,
                    &output.residuals.concat(),
                ),
                SectionKind::PreMix => Section::floats(
                    spec.kind,
                    spec.dtype,
                    vec![tokens, g.streams],
                    0,
                    &output.pre_mixes.concat(),
                ),
                SectionKind::SourceKv { layer } | SectionKind::SourceIndexKeys { layer } => {
                    let rows = output.source_rows.get(&(layer as usize)).ok_or_else(|| {
                        StageCpuError::Section(format!("no rows of KV source {layer}"))
                    })?;
                    let (first, values, width) =
                        if matches!(spec.kind, SectionKind::SourceKv { .. }) {
                            (rows.first_kv_row, &rows.kv, g.head_dim)
                        } else {
                            (rows.first_key_row, &rows.index_keys, g.index_dim)
                        };
                    Section::floats(
                        spec.kind,
                        spec.dtype,
                        vec![(values.len() / width as usize) as u32, width],
                        first,
                        values,
                    )
                }
                SectionKind::IndexSelection { layer } => {
                    let rows = output.selections.get(&(layer as usize)).ok_or_else(|| {
                        StageCpuError::Section(format!("no selection of layer {layer}"))
                    })?;
                    let top_k = g.index_top_k as usize;
                    let mut values = Vec::with_capacity(count * top_k);
                    for row in rows {
                        if row.len() > top_k {
                            return Err(StageCpuError::Section("selection too wide".into()));
                        }
                        values.extend_from_slice(row);
                        values.resize(values.len() + top_k - row.len(), SELECTION_PAD);
                    }
                    Section::i32s(spec.kind, vec![tokens, g.index_top_k], &values)
                }
                SectionKind::Candidates { .. } => {
                    let masks = output.candidates.as_ref().ok_or_else(|| {
                        StageCpuError::Section("no layer-20 candidates to forward".into())
                    })?;
                    let width = g.candidate_blocks as usize;
                    let mut values = Vec::with_capacity(count * width);
                    for mask in masks {
                        let blocks: Vec<u32> = mask
                            .chunks(CANDIDATE_BLOCK_ROWS)
                            .enumerate()
                            .filter(|(_, block)| block[0])
                            .map(|(b, _)| b as u32)
                            .collect();
                        if blocks.len() > width {
                            return Err(StageCpuError::Section("too many candidates".into()));
                        }
                        values.extend_from_slice(&blocks);
                        values.resize(values.len() + width - blocks.len(), ID_PAD);
                    }
                    Section::u32s(spec.kind, vec![tokens, g.candidate_blocks], &values)
                }
                SectionKind::EngramHistory => {
                    let rows = match spec.rows {
                        RowRule::Fixed(rows) => rows as usize,
                        _ => g.max_ngram.saturating_sub(1) as usize,
                    };
                    let mut values = vec![ID_PAD; rows.saturating_sub(output.history.len())];
                    values.extend(
                        output
                            .history
                            .iter()
                            .skip(output.history.len().saturating_sub(rows)),
                    );
                    Section::u32s(spec.kind, vec![rows as u32], &values)
                }
                SectionKind::SourceValue { .. } | SectionKind::Logits => {
                    return Err(StageCpuError::Section(format!(
                        "V4.1 stages don't send {}",
                        spec.kind
                    )));
                }
            };
            sections.push(section.map_err(bad)?);
        }
        Ok(sections)
    }
}

impl StageExecutor for V41CpuStageExecutor<'_> {
    type Error = StageCpuError;

    fn step(&mut self, message: &StageMessage) -> Result<Vec<Section>, Self::Error> {
        let key = (message.request_id.clone(), message.attempt);
        if self.request.as_ref() != Some(&key) {
            self.runtime.reset();
            self.request = Some(key);
        }
        let step = self.input(message)?;
        let output = self.runtime.forward(step)?;
        self.output(output, message.tokens.len())
    }

    fn reset(&mut self, request_id: &str) {
        if self.request.as_ref().is_none_or(|(id, _)| id == request_id) {
            self.runtime.reset();
            self.request = None;
        }
    }
}

/// The section kinds a boundary at `layer` carries, for status pages.
#[must_use]
pub fn boundary_kinds(config: &TransformerConfig, layer: u32) -> Vec<SectionKind> {
    boundary_sections(&wire_geometry(config), layer)
        .into_iter()
        .map(|s| s.kind)
        .collect()
}

// ---------------------------------------------------------------------------
// Synthetic model
// ---------------------------------------------------------------------------

/// A tiny V4.1-shaped model for tests and local groups: the 40-layer
/// synthetic transformer that `deepseek_v41_cpu`'s tests check against
/// NumPy, with an eight-token vocabulary.
#[cfg(any(test, feature = "deepseek-v41-fixtures"))]
pub mod synthetic {
    use std::{collections::BTreeMap, sync::Arc};

    use serde::Deserialize;

    use crate::deepseek_v41_cpu::{
        CpuError, MatrixRef, MatrixRows,
        block::BlockConfig,
        engram::{CompressedTokenMap, EngramLayout},
        hyperconnections::HyperConnections,
        moe::Router,
        projection::{Geometry, WeightSource},
        transformer::TransformerConfig,
    };

    /// Tokens in the synthetic vocabulary.
    pub const VOCABULARY: usize = 8;
    /// The synthetic end-of-answer token.
    pub const EOS: u32 = 1;

    #[derive(Deserialize)]
    struct Tensor {
        shape: Vec<usize>,
        values: Vec<f32>,
    }

    #[derive(Deserialize)]
    struct Tensors {
        #[serde(alias = "global_tensors")]
        tensors: BTreeMap<String, Tensor>,
    }

    /// The synthetic weights, held in memory.
    pub struct SyntheticWeights {
        tensors: BTreeMap<String, Tensor>,
    }

    impl WeightSource for SyntheticWeights {
        fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            let tensor = self
                .tensors
                .get(name)
                .ok_or(CpuError::Invalid("missing synthetic weight"))?;
            if tensor.shape.len() != 2 {
                return Err(CpuError::Invalid("synthetic matrix shape"));
            }
            Ok(Box::new(MatrixRef::new(
                tensor.shape[0],
                tensor.shape[1],
                &tensor.values,
            )?))
        }

        fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
            Ok(self
                .tensors
                .get(name)
                .ok_or(CpuError::Invalid("missing synthetic vector"))?
                .values
                .clone())
        }
    }

    fn parse(json: &str) -> Result<BTreeMap<String, Tensor>, CpuError> {
        serde_json::from_str::<Tensors>(json)
            .map(|t| t.tensors)
            .map_err(|_| CpuError::Invalid("synthetic weight fixture"))
    }

    /// The synthetic weights for all 40 layers.
    ///
    /// # Errors
    ///
    /// Only if the embedded fixtures are corrupt.
    pub fn weights() -> Result<SyntheticWeights, CpuError> {
        let block = parse(include_str!("testdata/deepseek_v41_cpu/block.json"))?;
        let attention = parse(include_str!("testdata/deepseek_v41_cpu/projection.json"))?;
        let mut tensors = BTreeMap::new();
        for layer in 0..40 {
            for (name, t) in &block {
                if name.contains("engram.") && ![1, 14].contains(&layer) {
                    continue;
                }
                let mut values = t.values.clone();
                let mut shape = t.shape.clone();
                if name.ends_with("engram.embed.weight") {
                    shape[0] = 48;
                    values.truncate(48 * 2);
                }
                tensors.insert(
                    name.replacen("layers.1.", &format!("layers.{layer}."), 1),
                    Tensor { shape, values },
                );
            }
            for (name, t) in &attention {
                if name.starts_with("layers.2.") {
                    tensors.insert(
                        name.replacen("layers.2.", &format!("layers.{layer}."), 1),
                        Tensor {
                            shape: t.shape.clone(),
                            values: t.values.clone(),
                        },
                    );
                }
            }
        }
        tensors.extend(parse(include_str!(
            "testdata/deepseek_v41_cpu/transformer.json"
        ))?);
        Ok(SyntheticWeights { tensors })
    }

    /// The synthetic model's configuration.
    ///
    /// # Errors
    ///
    /// Only if the Flash Engram layout is invalid.
    pub fn config() -> Result<TransformerConfig, CpuError> {
        let mut layout = EngramLayout::flash()?;
        layout.head_dim = 2;
        layout.compressed_vocab_size = VOCABULARY;
        for layer in &mut layout.layers {
            layer.primes = vec![2; 24];
            layer.offsets = (0..24).map(|i| i * 2).collect();
            layer.table_rows = 48;
        }
        Ok(TransformerConfig {
            block: BlockConfig {
                geometry: Geometry {
                    hidden: 4,
                    heads: 4,
                    head_dim: 4,
                    q_rank: 3,
                    o_groups: 2,
                    o_rank: 2,
                    index_heads: 2,
                    index_dim: 4,
                    rope_dim: 2,
                    window: 4,
                    index_top_k: 3,
                    norm_epsilon: 1e-20,
                },
                hyper: HyperConnections {
                    width: 4,
                    ..HyperConnections::flash()
                },
                router: Router {
                    top_k: 2,
                    ..Router::flash()
                },
                expert_count: 8,
                expert_width: 6,
                swiglu_limit: 10.0,
            },
            vocabulary: VOCABULARY,
            pad_token: 2,
            eos_token: EOS,
            engram: Arc::new(layout),
        })
    }

    /// The synthetic word-level tokenizer: `tok0` through `tok7`.
    ///
    /// # Errors
    ///
    /// Only if the tokenizer can't be built.
    pub fn tokenizer() -> Result<tokenizers::Tokenizer, CpuError> {
        let vocab: BTreeMap<_, _> = (0..VOCABULARY).map(|i| (format!("tok{i}"), i)).collect();
        let json = serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":vocab,"unk_token":"tok0"}});
        tokenizers::Tokenizer::from_bytes(json.to_string())
            .map_err(|e| CpuError::Tokenization(e.to_string()))
    }

    /// The Engram token map of the synthetic tokenizer.
    ///
    /// # Errors
    ///
    /// Only if the map can't be built.
    pub fn map() -> Result<Arc<CompressedTokenMap>, CpuError> {
        Ok(Arc::new(CompressedTokenMap::build(
            &tokenizer()?,
            VOCABULARY,
        )?))
    }
}

#[cfg(test)]
#[path = "deepseek_v41_stage_cpu_tests.rs"]
mod tests;
