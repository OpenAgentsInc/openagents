//! Complete 40-layer text transformer, `model.py:1186-1275`.
//! No MTP, vision encoder, sampling, network service, or eager weight expansion.

use super::{
    CpuError,
    block::{BlockConfig, TextBlock},
    engram::{CompressedTokenMap, EngramLayout, NgramHashState},
    product,
    projection::{self, WeightSource},
    quantization::{self, Arithmetic},
    shape,
    shared_state::{AttentionRuntime, StateSummary},
};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug)]
pub struct TransformerConfig {
    pub block: BlockConfig,
    pub vocabulary: usize,
    pub pad_token: u32,
    pub eos_token: u32,
    pub engram: Arc<EngramLayout>,
}
impl TransformerConfig {
    pub fn flash() -> Result<Self, CpuError> {
        Ok(Self {
            block: BlockConfig::flash(),
            vocabulary: 129280,
            pad_token: 2,
            eos_token: 1,
            engram: Arc::new(EngramLayout::flash()?),
        })
    }
    fn validate(&self, map: &CompressedTokenMap) -> Result<(), CpuError> {
        self.block.validate()?;
        if self.vocabulary == 0
            || self.vocabulary > u32::MAX as usize
            || map.ids().len() != self.vocabulary
            || self.pad_token as usize >= self.vocabulary
            || self.eos_token as usize >= self.vocabulary
            || map.vocab_size() != self.engram.compressed_vocab_size
            || self
                .engram
                .layers
                .iter()
                .map(|l| l.layer_id)
                .collect::<Vec<_>>()
                != vec![1, 14]
            || self.engram.heads != 8
            || self.engram.max_ngram_size != 4
        {
            return Err(CpuError::Invalid(
                "text transformer vocabulary or Engram configuration",
            ));
        }
        Ok(())
    }
}

/// Optional caller-requested traces contain model activations, never just hashes.
#[derive(Debug, Clone)]
pub struct LayerTrace {
    pub layer: usize,
    pub residuals: Vec<Vec<f32>>,
    pub pre_mixes: Vec<Vec<f32>>,
    pub expert_ids: Vec<Vec<usize>>,
    pub route_weights: Vec<Vec<f32>>,
    pub engram_hashes: Option<Vec<Vec<u64>>>,
}
#[derive(Debug, Clone)]
pub struct ForwardOutput {
    /// Full vocabulary logits for the last token of the supplied chunk.
    pub logits: Vec<f32>,
    pub position: usize,
    pub layers: Vec<LayerTrace>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformerSummary {
    pub position: usize,
    pub hash_position: usize,
    pub attention: StateSummary,
}

/// Synchronous qualification observations. Values are borrowed for the call only.
/// An observer error aborts the forward without committing model state. External
/// observer output is diagnostic and is not rolled back with model state.
pub trait ModelObserver {
    /// Requests final collapse/norm observations at this position.
    fn final_position(&self, _position: usize) -> bool {
        false
    }
    /// Requests a full vocabulary projection in addition to final collapse/norm.
    fn logits_position(&self, _position: usize) -> bool {
        false
    }
    fn floats(&mut self, site: &str, position: usize, values: &[f32]) -> Result<(), CpuError>;
    fn integers(&mut self, _site: &str, _position: usize, _values: &[u64]) -> Result<(), CpuError> {
        Ok(())
    }
}
fn observe(
    observer: &mut Option<&mut dyn ModelObserver>,
    site: &str,
    position: usize,
    values: &[f32],
) -> Result<(), CpuError> {
    if let Some(o) = observer {
        o.floats(site, position, values)?;
    }
    Ok(())
}

#[derive(Clone)]
pub struct CpuTransformer<'a> {
    weights: &'a dyn WeightSource,
    config: TransformerConfig,
    map: Arc<CompressedTokenMap>,
    attention: AttentionRuntime<'a>,
    hash: NgramHashState,
    arithmetic: Arithmetic,
    max_context: usize,
    position: usize,
}
impl<'a> CpuTransformer<'a> {
    pub fn new(
        weights: &'a dyn WeightSource,
        config: TransformerConfig,
        map: Arc<CompressedTokenMap>,
        max_context: usize,
        arithmetic: Arithmetic,
    ) -> Result<Self, CpuError> {
        config.validate(&map)?;
        let width = config.block.geometry.hidden;
        for name in ["embed.weight", "head.weight"] {
            if weights.matrix(name)?.dimensions() != (config.vocabulary, width) {
                return Err(CpuError::Invalid("embedding/head checkpoint dimensions"));
            }
        }
        shape("final norm", &weights.vector("norm.weight")?, width)?;
        for layer in &config.engram.layers {
            if weights
                .matrix(&format!("layers.{}.engram.embed.weight", layer.layer_id))?
                .dimensions()
                != (
                    usize::try_from(layer.table_rows)
                        .map_err(|_| CpuError::Invalid("Engram table row overflow"))?,
                    config.engram.head_dim,
                )
            {
                return Err(CpuError::Invalid("transformer Engram table dimensions"));
            }
        }
        let hash = NgramHashState::new(config.engram.clone(), map.get(config.pad_token)?)?;
        TextBlock::new(weights, 0, config.block, arithmetic)?;
        let attention =
            AttentionRuntime::new(weights, config.block.geometry, max_context, arithmetic)?;
        Ok(Self {
            weights,
            config,
            map,
            attention,
            hash,
            arithmetic,
            max_context,
            position: 0,
        })
    }
    pub fn config(&self) -> &TransformerConfig {
        &self.config
    }
    pub fn arithmetic(&self) -> Arithmetic {
        self.arithmetic
    }
    pub fn max_context(&self) -> usize {
        self.max_context
    }
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn summary(&self) -> TransformerSummary {
        TransformerSummary {
            position: self.position,
            hash_position: self.hash.next_position(),
            attention: self.attention.summary(),
        }
    }
    pub fn reset(&mut self) {
        self.attention.reset();
        self.hash.reset();
        self.position = 0;
    }
    pub fn forward_tokens(&mut self, tokens: &[u32]) -> Result<ForwardOutput, CpuError> {
        self.forward_with_trace(tokens, &[])
    }
    /// Updates all blocks, attention caches, and n-gram history atomically.
    /// Tracing is explicit because it retains every selected layer's residuals.
    pub fn forward_with_trace(
        &mut self,
        tokens: &[u32],
        trace_layers: &[usize],
    ) -> Result<ForwardOutput, CpuError> {
        self.forward_impl(tokens, trace_layers, None)
    }
    /// Observes all blocks without retaining their full activation tensors.
    pub fn forward_observed(
        &mut self,
        tokens: &[u32],
        observer: &mut dyn ModelObserver,
    ) -> Result<ForwardOutput, CpuError> {
        self.forward_impl(tokens, &[], Some(observer))
    }
    fn forward_impl(
        &mut self,
        tokens: &[u32],
        trace_layers: &[usize],
        observer: Option<&mut dyn ModelObserver>,
    ) -> Result<ForwardOutput, CpuError> {
        if tokens.is_empty()
            || tokens.iter().any(|t| *t as usize >= self.config.vocabulary)
            || self
                .position
                .checked_add(tokens.len())
                .is_none_or(|n| n > self.max_context)
        {
            return Err(CpuError::Invalid("transformer tokens or context capacity"));
        }
        let trace: BTreeSet<_> = trace_layers.iter().copied().collect();
        if trace.len() != trace_layers.len() || trace.iter().any(|l| *l >= 40) {
            return Err(CpuError::Invalid("transformer trace layers"));
        }
        let mut next = self.clone();
        let output = next.run(tokens, &trace, observer)?;
        *self = next;
        Ok(output)
    }
    fn finish(&self, values: Vec<f32>) -> Result<Vec<f32>, CpuError> {
        if self.arithmetic.is_bf16() {
            quantization::round_bf16(&values)
        } else {
            Ok(values)
        }
    }
    fn run(
        &mut self,
        tokens: &[u32],
        trace: &BTreeSet<usize>,
        mut observer: Option<&mut dyn ModelObserver>,
    ) -> Result<ForwardOutput, CpuError> {
        let c = &self.config;
        let width = c.block.geometry.hidden;
        let streams = c.block.hyper.streams;
        let hashes = self
            .hash
            .push_tokens(self.position, tokens, None, &self.map)?;
        let mut residuals = Vec::with_capacity(tokens.len());
        let mut pre_mixes = Vec::with_capacity(tokens.len());
        for (i, token) in tokens.iter().enumerate() {
            let embedded =
                self.finish(projection::embedding(self.weights, *token as usize, width)?)?;
            observe(&mut observer, "embed.out", self.position + i, &embedded)?;
            residuals.push(embedded.repeat(streams));
            let mut pre = vec![0.0; streams];
            pre[0] = 1.0;
            pre_mixes.push(pre);
        }
        let mut layers = Vec::new();
        for layer in 0..40 {
            let block = TextBlock::new(self.weights, layer, c.block, self.arithmetic)?
                .with_reference_rows(tokens.len());
            let engram = c
                .engram
                .layers
                .iter()
                .position(|l| l.layer_id as usize == layer);
            if let Some(slot) = engram {
                for (i, residual) in residuals.iter_mut().enumerate() {
                    observe(
                        &mut observer,
                        &format!("layers.{layer}.engram.in"),
                        self.position + i,
                        residual,
                    )?;
                    if let Some(o) = &mut observer {
                        o.integers(
                            &format!("layers.{layer}.engram.hash_ids"),
                            self.position + i,
                            &hashes[i].layers[slot],
                        )?;
                    }
                    *residual = block.apply_engram_observed(
                        residual,
                        &hashes[i].layers[slot],
                        c.engram.layers[slot].table_rows,
                        c.engram.head_dim,
                        &mut |rows| {
                            observe(
                                &mut observer,
                                &format!("layers.{layer}.engram.rows"),
                                self.position + i,
                                rows,
                            )
                        },
                    )?;
                    observe(
                        &mut observer,
                        &format!("layers.{layer}.engram.out"),
                        self.position + i,
                        residual,
                    )?;
                }
            }
            for i in 0..tokens.len() {
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.in"),
                    self.position + i,
                    &residuals[i],
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.pre_mix_in"),
                    self.position + i,
                    &pre_mixes[i],
                )?;
            }
            let mut mixes = Vec::with_capacity(tokens.len());
            let mut normalized = Vec::with_capacity(product(tokens.len(), width)?);
            for (i, (residual, pre)) in residuals.iter().zip(&pre_mixes).enumerate() {
                let input = block.begin_attention(residual, pre)?;
                let pos = self.position + i;
                if observer.is_some() {
                    observe(
                        &mut observer,
                        &format!("layers.{layer}.hc.attn.collapsed"),
                        pos,
                        &self.finish(c.block.hyper.collapse(residual, pre)?)?,
                    )?;
                }
                observe(
                    &mut observer,
                    &format!("layers.{layer}.attn.in"),
                    pos,
                    &input.normalized,
                )?;
                for (name, values) in [
                    ("pre", &input.mixes.pre),
                    ("post", &input.mixes.post),
                    ("comb", &input.mixes.comb),
                ] {
                    observe(
                        &mut observer,
                        &format!("layers.{layer}.hc.attn.{name}"),
                        pos,
                        values,
                    )?;
                }
                normalized.extend(input.normalized);
                mixes.push(input.mixes);
            }
            let attention = match &mut observer {
                Some(o) => self.attention.forward_chunk_observed(
                    layer,
                    self.position,
                    &normalized,
                    Some(&mut **o),
                )?,
                None => self
                    .attention
                    .forward_chunk(layer, self.position, &normalized)?,
            };
            let mut expert_ids = Vec::new();
            let mut route_weights = Vec::new();
            for i in 0..tokens.len() {
                let pos = self.position + i;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.attn.out"),
                    pos,
                    &attention[i * width..(i + 1) * width],
                )?;
                let attended = block.finish_attention(
                    &attention[i * width..(i + 1) * width],
                    &residuals[i],
                    &mixes[i],
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.attn.expanded"),
                    pos,
                    &attended,
                )?;
                let output = block.feed_forward_observed(
                    &attended,
                    &mixes[i].pre,
                    &mut |site, values| {
                        observe(
                            &mut observer,
                            &format!("layers.{layer}.ffn.{site}"),
                            pos,
                            values,
                        )
                    },
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.ffn.gate.in"),
                    pos,
                    &output.normalized_input,
                )?;
                for (site, values) in [
                    ("scores", &output.routing.scores),
                    ("biased_scores", &output.routing.biased_scores),
                ] {
                    observe(
                        &mut observer,
                        &format!("layers.{layer}.ffn.gate.{site}"),
                        pos,
                        values,
                    )?;
                }
                observe(
                    &mut observer,
                    &format!("layers.{layer}.ffn.out"),
                    pos,
                    &output.moe_output,
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.ffn.collapsed"),
                    pos,
                    &output.collapsed_input,
                )?;
                for (name, values) in [
                    ("pre", &output.mixes.pre),
                    ("post", &output.mixes.post),
                    ("comb", &output.mixes.comb),
                ] {
                    observe(
                        &mut observer,
                        &format!("layers.{layer}.hc.ffn.{name}"),
                        pos,
                        values,
                    )?;
                }
                if let Some(o) = &mut observer {
                    o.integers(
                        &format!("layers.{layer}.ffn.gate.ids"),
                        pos,
                        &output
                            .routing
                            .indices
                            .iter()
                            .map(|id| *id as u64)
                            .collect::<Vec<_>>(),
                    )?;
                }
                observe(
                    &mut observer,
                    &format!("layers.{layer}.ffn.gate.weights"),
                    pos,
                    &output.routing.weights,
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.ffn.expanded"),
                    pos,
                    &output.residual,
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.out"),
                    pos,
                    &output.residual,
                )?;
                observe(
                    &mut observer,
                    &format!("layers.{layer}.hc.pre_mix_out"),
                    pos,
                    &output.next_pre,
                )?;
                residuals[i] = output.residual;
                pre_mixes[i] = output.next_pre;
                if trace.contains(&layer) {
                    expert_ids.push(output.routing.indices);
                    route_weights.push(output.routing.weights);
                }
            }
            if trace.contains(&layer) {
                layers.push(LayerTrace {
                    layer,
                    residuals: residuals.clone(),
                    pre_mixes: pre_mixes.clone(),
                    expert_ids,
                    route_weights,
                    engram_hashes: engram
                        .map(|slot| hashes.iter().map(|h| h.layers[slot].clone()).collect()),
                });
            }
        }
        let last = tokens.len() - 1;
        let mut logits = Vec::new();
        for i in 0..tokens.len() {
            let pos = self.position + i;
            let final_position = observer.as_ref().is_some_and(|o| o.final_position(pos));
            let want_logits = observer.as_ref().is_some_and(|o| o.logits_position(pos));
            if i != last && !final_position && !want_logits {
                continue;
            }
            let (collapsed, normalized) = projection::final_input_with_reference_rows(
                self.weights,
                &residuals[i],
                &pre_mixes[i],
                width,
                c.block.geometry.norm_epsilon,
                self.arithmetic,
                tokens.len(),
            )?;
            observe(
                &mut observer,
                "layers.39.hc.final.collapsed",
                pos,
                &collapsed,
            )?;
            observe(&mut observer, "norm.out", pos, &normalized)?;
            if i == last || want_logits {
                let row = super::matvec(self.weights.matrix("head.weight")?.as_ref(), &normalized)?;
                shape("transformer logits", &row, c.vocabulary)?;
                observe(&mut observer, "logits", pos, &row)?;
                if i == last {
                    logits = row;
                }
            }
        }
        self.position += tokens.len();
        if self.hash.next_position() != self.position
            || self.attention.summary().next_position != self.position
        {
            return Err(CpuError::Invalid(
                "transformer component positions disagree",
            ));
        }
        Ok(ForwardOutput {
            logits,
            position: self.position,
            layers,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::{
        MatrixRows,
        block::tests as block_fixture,
        projection::tests::{Store, Tensor},
        quantization::MatrixPrecision,
    };
    use super::*;
    use serde::Deserialize;
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
    };
    #[derive(Deserialize)]
    pub struct ExpectedLayer {
        pub residuals: Vec<Vec<f32>>,
        pub pre_mixes: Vec<Vec<f32>>,
        pub expert_ids: Vec<Vec<usize>>,
        pub route_weights: Vec<Vec<f32>>,
    }
    #[derive(Deserialize)]
    pub struct Fixture {
        pub global_tensors: BTreeMap<String, Tensor>,
        pub tokens: Vec<u32>,
        pub logits: Vec<f32>,
        pub layers: BTreeMap<usize, ExpectedLayer>,
    }
    pub fn fixture() -> Result<Fixture, serde_json::Error> {
        serde_json::from_str(include_str!(
            "../testdata/deepseek_v41_cpu/transformer.json"
        ))
    }
    pub fn map() -> Result<Arc<CompressedTokenMap>, CpuError> {
        let vocab: BTreeMap<_, _> = (0..8).map(|i| (format!("tok{i}"), i)).collect();
        let json = serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":vocab,"unk_token":"tok0"}});
        let tokenizer = tokenizers::Tokenizer::from_bytes(json.to_string())
            .map_err(|e| CpuError::Tokenization(e.to_string()))?;
        Ok(Arc::new(CompressedTokenMap::build(&tokenizer, 8)?))
    }
    pub fn config(reference: bool) -> Result<TransformerConfig, CpuError> {
        let mut layout = EngramLayout::flash()?;
        layout.head_dim = if reference { 4 } else { 2 };
        layout.compressed_vocab_size = 8;
        for layer in &mut layout.layers {
            layer.primes = vec![2; 24];
            layer.offsets = (0..24).map(|i| i * 2).collect();
            layer.table_rows = 48;
        }
        let mut block = block_fixture::config();
        if reference {
            block.geometry = super::super::projection::Geometry {
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
                ..super::super::projection::Geometry::flash()
            };
            block.hyper.width = 32;
            block.expert_width = 32;
        }
        Ok(TransformerConfig {
            block,
            vocabulary: 8,
            pad_token: 2,
            eos_token: 1,
            engram: Arc::new(layout),
        })
    }
    pub fn weights() -> Result<Store, Box<dyn std::error::Error>> {
        let (block, _) = block_fixture::load()?;
        let (attention, _, _) = super::super::projection::tests::load()?;
        let mut tensors = BTreeMap::new();
        for layer in 0..40 {
            for (name, t) in &block.tensors {
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
            for (name, t) in &attention.tensors {
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
        tensors.extend(fixture()?.global_tensors);
        Ok(Store {
            tensors,
            reads: RefCell::new(Vec::new()),
        })
    }
    fn near(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert!((a - b).abs() < 2e-4 + 3e-5 * b.abs(), "{a} != {b}");
        }
    }
    #[test]
    fn full_40_layer_transformer_matches_numpy() -> Result<(), Box<dyn std::error::Error>> {
        let w = weights()?;
        let f = fixture()?;
        let mut model = CpuTransformer::new(&w, config(false)?, map()?, 16, Arithmetic::Float32)?;
        let layers: Vec<_> = f.layers.keys().copied().collect();
        let output = model.forward_with_trace(&f.tokens, &layers)?;
        near(&output.logits, &f.logits);
        for trace in output.layers {
            let expected = &f.layers[&trace.layer];
            for (a, b) in trace.residuals.iter().zip(&expected.residuals) {
                near(a, b);
            }
            for (a, b) in trace.pre_mixes.iter().zip(&expected.pre_mixes) {
                near(a, b);
            }
            assert_eq!(trace.expert_ids, expected.expert_ids);
            for (a, b) in trace.route_weights.iter().zip(&expected.route_weights) {
                near(a, b);
            }
        }
        assert_eq!(model.position(), f.tokens.len());
        assert_eq!(model.summary().hash_position, f.tokens.len());
        Ok(())
    }
    #[test]
    fn full_model_chunk_replay_reset_and_input_refusals() -> Result<(), Box<dyn std::error::Error>>
    {
        let w = weights()?;
        let f = fixture()?;
        let mut whole = CpuTransformer::new(&w, config(false)?, map()?, 8, Arithmetic::Float32)?;
        let expected = whole.forward_tokens(&f.tokens)?;
        let mut replay = CpuTransformer::new(&w, config(false)?, map()?, 8, Arithmetic::Float32)?;
        let mut last = None;
        for token in &f.tokens {
            last = Some(replay.forward_tokens(&[*token])?);
        }
        near(&last.ok_or("last output")?.logits, &expected.logits);
        assert_eq!(whole.summary(), replay.summary());
        let before = replay.summary();
        assert!(replay.forward_tokens(&[8]).is_err());
        assert_eq!(before, replay.summary());
        assert!(replay.forward_tokens(&[0; 8]).is_err());
        assert!(replay.forward_with_trace(&[0], &[40]).is_err());
        replay.reset();
        assert_eq!(replay.forward_tokens(&f.tokens)?.logits, expected.logits);
        Ok(())
    }
    #[test]
    fn borrowed_observations_preserve_outputs_and_observer_errors_roll_back()
    -> Result<(), Box<dyn std::error::Error>> {
        struct Observe {
            count: usize,
            fail: Option<&'static str>,
            logits: Vec<Vec<f32>>,
            attention_sites: BTreeSet<String>,
            expert_suffixes: BTreeSet<String>,
        }
        impl ModelObserver for Observe {
            fn final_position(&self, _: usize) -> bool {
                true
            }
            fn logits_position(&self, _: usize) -> bool {
                true
            }
            fn floats(&mut self, site: &str, _: usize, values: &[f32]) -> Result<(), CpuError> {
                self.count += 1;
                if self.fail == Some(site) {
                    return Err(CpuError::Invalid("injected observer failure"));
                }
                if site == "logits" {
                    self.logits.push(values.to_vec());
                }
                if site.starts_with("layers.0.attn.") {
                    self.attention_sites.insert(site.to_string());
                }
                if let Some((_, tail)) = site.split_once(".ffn.expert.") {
                    let (_, suffix) = tail
                        .split_once('.')
                        .ok_or(CpuError::Invalid("expert trace name"))?;
                    self.expert_suffixes.insert(suffix.to_string());
                }
                Ok(())
            }
        }
        let w = weights()?;
        let mut a = CpuTransformer::new(&w, config(false)?, map()?, 16, Arithmetic::Float32)?;
        let mut b = a.clone();
        let expected = a.forward_tokens(&[0, 3, 4])?;
        let mut observer = Observe {
            count: 0,
            fail: None,
            logits: Vec::new(),
            attention_sites: BTreeSet::new(),
            expert_suffixes: BTreeSet::new(),
        };
        let observed = b.forward_observed(&[0, 3, 4], &mut observer)?;
        assert_eq!(expected.logits, observed.logits);
        assert_eq!(a.summary(), b.summary());
        assert_eq!(observer.logits.len(), 3);
        assert!(observer.count > 40 * 3);
        // These names match #12371's retained expert hook contract.
        for suffix in [
            "in",
            "combine_weight",
            "w1_out",
            "w3_out",
            "w2_in",
            "w2_out",
            "out",
        ] {
            assert!(observer.expert_suffixes.contains(suffix));
        }
        for site in [
            "q.normalized",
            "q.projected",
            "q.rotated",
            "kv.normalized",
            "kv.rotated",
            "kv.quantized",
            "heads",
            "heads.unrotated",
            "output.low_rank",
        ] {
            assert!(
                observer
                    .attention_sites
                    .contains(&format!("layers.0.attn.{site}"))
            );
        }
        let before = b.summary();
        for site in [
            "layers.0.attn.q.normalized",
            "layers.1.engram.rows",
            "layers.2.attn.heads",
            "layers.2.attn.compressor.latent",
            "layers.20.attn.indexer.topk_idxs",
            "layers.2.ffn.shared.out",
            "layers.39.hc.out",
        ] {
            observer.fail = Some(site);
            assert!(b.forward_observed(&[5], &mut observer).is_err());
            assert_eq!(b.summary(), before);
        }
        observer.fail = None;
        assert_eq!(
            a.forward_tokens(&[5])?.logits,
            b.forward_observed(&[5], &mut observer)?.logits
        );
        assert_eq!(a.summary(), b.summary());
        Ok(())
    }
    struct Fail<'a> {
        source: &'a Store,
        fail: Cell<bool>,
    }
    impl WeightSource for Fail<'_> {
        fn vector(&self, n: &str) -> Result<Vec<f32>, CpuError> {
            self.source.vector(n)
        }
        fn matrix(&self, n: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            if self.fail.get() && n == "layers.39.ffn.shared_experts.w2.weight" {
                return Err(CpuError::Invalid("injected final block failure"));
            }
            self.source.matrix(n)
        }
    }
    #[test]
    fn late_failure_rolls_back_all_layers_and_hash_history()
    -> Result<(), Box<dyn std::error::Error>> {
        let w = weights()?;
        let source = Fail {
            source: &w,
            fail: Cell::new(true),
        };
        let mut model =
            CpuTransformer::new(&source, config(false)?, map()?, 8, Arithmetic::Float32)?;
        let before = model.summary();
        assert!(model.forward_tokens(&[0, 3, 4]).is_err());
        assert_eq!(before, model.summary());
        source.fail.set(false);
        let out = model.forward_tokens(&[0, 3, 4])?;
        let mut fresh = CpuTransformer::new(&w, config(false)?, map()?, 8, Arithmetic::Float32)?;
        assert_eq!(out.logits, fresh.forward_tokens(&[0, 3, 4])?.logits);
        Ok(())
    }
    pub struct ReferenceWeights;
    struct Rows {
        rows: usize,
        cols: usize,
        precision: MatrixPrecision,
        gate: bool,
        embedding: bool,
    }
    impl MatrixRows for Rows {
        fn dimensions(&self) -> (usize, usize) {
            (self.rows, self.cols)
        }
        fn precision(&self) -> MatrixPrecision {
            self.precision
        }
        fn row(&self, i: usize) -> Result<Vec<f32>, CpuError> {
            if i >= self.rows {
                return Err(CpuError::Invalid("synthetic row out of range"));
            }
            if self.gate {
                return Ok(vec![1.0; self.cols]);
            }
            Ok((0..self.cols)
                .map(|j| {
                    if self.embedding {
                        ((i + j) % 8 + 1) as f32 * 0.03125
                    } else if j == i % self.cols {
                        0.0625
                    } else {
                        0.015625
                    }
                })
                .collect())
        }
    }
    impl WeightSource for ReferenceWeights {
        fn vector(&self, n: &str) -> Result<Vec<f32>, CpuError> {
            Ok(if n.ends_with("_base") {
                (0..24).map(|i| if i < 4 { -1.0 } else { -0.5 }).collect()
            } else if n.ends_with("_scale") {
                vec![0.05; 3]
            } else if n.ends_with("gate.bias") {
                (0..8).map(|i| i as f32 * 0.01).collect()
            } else if n.ends_with("attn_sink") {
                vec![0.25; 2]
            } else if n.ends_with("norm.weight") {
                vec![1.0; 32]
            } else {
                return Err(CpuError::Invalid("unknown synthetic vector"));
            })
        }
        fn matrix(&self, n: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            let (rows, cols) = if n == "embed.weight" || n == "head.weight" {
                (8, 32)
            } else if n.ends_with("_fn") {
                (24, 128)
            } else if n.ends_with("engram.embed.weight") {
                (48, 4)
            } else if n.ends_with("engram.wkv.weight") {
                (160, 96)
            } else if n.ends_with("engram.q_weight") || n.ends_with("engram.k_weight") {
                (4, 32)
            } else if n.ends_with("ffn.gate.weight") {
                (8, 32)
            } else if n.ends_with("attn.wq_b.weight") || n.ends_with("indexer.wq_b.weight") {
                (64, 32)
            } else if n.ends_with("weights_proj.weight") {
                (2, 32)
            } else if n.ends_with("wo_a.weight") {
                (64, 32)
            } else if n.ends_with("wo_b.weight") {
                (32, 64)
            } else if n.ends_with(".weight") {
                (32, 32)
            } else {
                return Err(CpuError::Invalid("unknown synthetic matrix"));
            };
            let precision = if n.ends_with("_fn") {
                MatrixPrecision::F32
            } else if n.contains("ffn.experts.") {
                MatrixPrecision::Fp4
            } else if n.contains("compressor.")
                || n.contains("indexer.wk.")
                || n.contains("weights_proj.")
                || n == "embed.weight"
                || n == "head.weight"
                || n.contains("ffn.gate.")
                || n.ends_with("_weight")
            {
                MatrixPrecision::Bf16
            } else {
                MatrixPrecision::Fp8
            };
            Ok(Box::new(Rows {
                rows,
                cols,
                precision,
                gate: n.ends_with("_weight"),
                embedding: n == "embed.weight" || n == "head.weight",
            }))
        }
    }
    #[test]
    fn complete_bf16_model_runs_all_parts_and_replays_exactly() -> Result<(), CpuError> {
        let mut model = CpuTransformer::new(
            &ReferenceWeights,
            config(true)?,
            map()?,
            8,
            Arithmetic::ReferenceBf16,
        )?;
        let out = model.forward_with_trace(&[0, 3, 4], &[1, 14, 39])?;
        assert_eq!(out.layers.len(), 3);
        assert!(out.layers[0].engram_hashes.is_some());
        assert!(out.layers[1].engram_hashes.is_some());
        let mut split = CpuTransformer::new(
            &ReferenceWeights,
            config(true)?,
            map()?,
            8,
            Arithmetic::ReferenceBf16,
        )?;
        split.forward_tokens(&[0, 3])?;
        let last = split.forward_tokens(&[4])?;
        assert_eq!(last.logits, out.logits);
        assert_eq!(model.summary(), split.summary());
        Ok(())
    }
}
