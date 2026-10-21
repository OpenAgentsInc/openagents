//! Complete text-block wiring from the pinned `model.py:328-366,794-995`.
//! Attention computation is supplied by the request-owned attention runtime.

use super::{
    CpuError,
    engram::EngramGate,
    finite,
    hyperconnections::{HyperConnections, Mixes},
    moe::{self, ExpertProvider, Router, Routing},
    positive, product,
    projection::{Geometry, WeightSource, compress_ratio},
    quantization::{self, Arithmetic},
    rms_norm, shape,
};

#[derive(Clone, Copy, Debug)]
pub struct BlockConfig {
    pub geometry: Geometry,
    pub hyper: HyperConnections,
    pub router: Router,
    pub expert_count: usize,
    pub expert_width: usize,
    pub swiglu_limit: f32,
}
impl BlockConfig {
    pub fn flash() -> Self {
        Self {
            geometry: Geometry::flash(),
            hyper: HyperConnections::flash(),
            router: Router::flash(),
            expert_count: 384,
            expert_width: 2304,
            swiglu_limit: 10.0,
        }
    }
    pub fn validate(&self) -> Result<(), CpuError> {
        self.geometry.validate()?;
        self.hyper.validate()?;
        if self.hyper.width != self.geometry.hidden
            || self.expert_width == 0
            || self.expert_count == 0
            || self.router.top_k == 0
            || self.router.top_k > self.expert_count
            || !positive(self.router.temperature)
            || !positive(self.router.route_scale)
            || !self.swiglu_limit.is_finite()
            || self.swiglu_limit < 0.0
        {
            return Err(CpuError::Invalid("V4.1 block configuration"));
        }
        product(self.expert_width, self.geometry.hidden)?;
        Ok(())
    }
}

pub struct AttentionInput {
    pub normalized: Vec<f32>,
    pub mixes: Mixes,
}
pub struct FeedForwardOutput {
    pub residual: Vec<f32>,
    pub next_pre: Vec<f32>,
    pub routing: Routing,
    pub normalized_input: Vec<f32>,
    pub collapsed_input: Vec<f32>,
    pub moe_output: Vec<f32>,
    pub mixes: Mixes,
}

pub struct TextBlock<'a> {
    weights: &'a dyn WeightSource,
    layer: usize,
    config: BlockConfig,
    arithmetic: Arithmetic,
}
impl<'a> TextBlock<'a> {
    pub fn new(
        weights: &'a dyn WeightSource,
        layer: usize,
        config: BlockConfig,
        arithmetic: Arithmetic,
    ) -> Result<Self, CpuError> {
        config.validate()?;
        compress_ratio(layer)?;
        super::projection::AttentionProjections::new(weights, layer, config.geometry)?
            .with_arithmetic(arithmetic)?;
        if arithmetic.is_bf16() && !config.expert_width.is_multiple_of(32) {
            return Err(CpuError::Invalid(
                "reference expert width must be a multiple of 32",
            ));
        }
        Ok(Self {
            weights,
            layer,
            config,
            arithmetic,
        })
    }
    fn name(&self, part: &str) -> String {
        format!("layers.{}.{part}", self.layer)
    }
    fn finish(&self, values: Vec<f32>) -> Result<Vec<f32>, CpuError> {
        finite("block output", &values)?;
        if self.arithmetic.is_bf16() {
            quantization::round_bf16(&values)
        } else {
            Ok(values)
        }
    }
    fn norm(&self, part: &str, values: &[f32]) -> Result<Vec<f32>, CpuError> {
        let weight = self.weights.vector(&self.name(part))?;
        let weight = if self.arithmetic.is_bf16() {
            quantization::round_bf16(&weight)?
        } else {
            weight
        };
        self.finish(rms_norm(
            values,
            &weight,
            self.config.geometry.norm_epsilon,
        )?)
    }
    fn mixes(&self, part: &str, residual: &[f32]) -> Result<Mixes, CpuError> {
        self.config.hyper.mixes(
            residual,
            self.weights
                .matrix(&self.name(&format!("hc_{part}_fn")))?
                .as_ref(),
            &self
                .weights
                .vector(&self.name(&format!("hc_{part}_scale")))?,
            &self
                .weights
                .vector(&self.name(&format!("hc_{part}_base")))?,
        )
    }
    /// Mixes computed here belong to attention's post step and the next FFN input.
    pub fn begin_attention(
        &self,
        residual: &[f32],
        previous_pre: &[f32],
    ) -> Result<AttentionInput, CpuError> {
        let mixes = self.mixes("attn", residual)?;
        let collapsed = self.finish(self.config.hyper.collapse(residual, previous_pre)?)?;
        Ok(AttentionInput {
            normalized: self.norm("attn_norm.weight", &collapsed)?,
            mixes,
        })
    }
    pub fn finish_attention(
        &self,
        output: &[f32],
        residual: &[f32],
        mixes: &Mixes,
    ) -> Result<Vec<f32>, CpuError> {
        self.finish(self.config.hyper.expand(output, residual, mixes)?)
    }
    pub fn feed_forward(
        &self,
        residual: &[f32],
        attention_pre: &[f32],
    ) -> Result<FeedForwardOutput, CpuError> {
        let mixes = self.mixes("ffn", residual)?;
        let collapsed = self.finish(self.config.hyper.collapse(residual, attention_pre)?)?;
        let input = self.norm("ffn_norm.weight", &collapsed)?;
        let gate = self.weights.matrix(&self.name("ffn.gate.weight"))?;
        if gate.dimensions() != (self.config.expert_count, self.config.geometry.hidden) {
            return Err(CpuError::Invalid("router matrix dimensions"));
        }
        let bias = self.weights.vector(&self.name("ffn.gate.bias"))?;
        // Gate.forward promotes both input and BF16 router weights to F32.
        let (routing, output) = moe::forward(
            &input,
            gate.as_ref(),
            &bias,
            self.config.router,
            &NamedExperts { block: self },
            self.config.swiglu_limit,
        )?;
        let output = self.finish(output)?;
        let residual = self.finish(self.config.hyper.expand(&output, residual, &mixes)?)?;
        Ok(FeedForwardOutput {
            residual,
            next_pre: mixes.pre.clone(),
            routing,
            normalized_input: input,
            collapsed_input: collapsed,
            moe_output: output,
            mixes,
        })
    }
    pub fn apply_engram(
        &self,
        residual: &[f32],
        hash_ids: &[u64],
        table_rows: u64,
        head_dim: usize,
    ) -> Result<Vec<f32>, CpuError> {
        if ![1, 14].contains(&self.layer) || hash_ids.len() != 24 {
            return Err(CpuError::Invalid("Engram layer or hash columns"));
        }
        let table = self.weights.matrix(&self.name("engram.embed.weight"))?;
        if table.dimensions()
            != (
                usize::try_from(table_rows)
                    .map_err(|_| CpuError::Invalid("Engram row count overflow"))?,
                head_dim,
            )
        {
            return Err(CpuError::Invalid("Engram table dimensions"));
        }
        let projection = self.weights.matrix(&self.name("engram.wkv.weight"))?;
        let h = self.config.hyper;
        let read_flat = |part: &str| -> Result<Vec<f32>, CpuError> {
            let matrix = self.weights.matrix(&self.name(part))?;
            if matrix.dimensions() != (h.streams, h.width) {
                return Err(CpuError::Invalid("Engram gate parameter dimensions"));
            }
            let mut values = Vec::with_capacity(product(h.streams, h.width)?);
            for i in 0..h.streams {
                let row = matrix.row(i)?;
                shape("Engram gate row", &row, h.width)?;
                values.extend(row);
            }
            Ok(values)
        };
        EngramGate {
            streams: h.streams,
            width: h.width,
            epsilon: self.config.geometry.norm_epsilon,
        }
        .lookup_and_apply_with_arithmetic(
            residual,
            hash_ids,
            table.as_ref(),
            projection.as_ref(),
            &read_flat("engram.q_weight")?,
            &read_flat("engram.k_weight")?,
            true,
            self.arithmetic,
        )
    }
}
struct NamedExperts<'a, 'b> {
    block: &'a TextBlock<'b>,
}
impl ExpertProvider for NamedExperts<'_, '_> {
    fn forward(
        &self,
        id: Option<usize>,
        input: &[f32],
        route_weight: f32,
        limit: f32,
    ) -> Result<Vec<f32>, CpuError> {
        let b = self.block;
        let c = b.config;
        if id.is_some_and(|i| i >= c.expert_count) {
            return Err(CpuError::Invalid("expert ID out of range"));
        }
        let stem = match id {
            Some(i) => format!("ffn.experts.{i}"),
            None => "ffn.shared_experts".to_owned(),
        };
        let w1 = b.weights.matrix(&b.name(&format!("{stem}.w1.weight")))?;
        let w2 = b.weights.matrix(&b.name(&format!("{stem}.w2.weight")))?;
        let w3 = b.weights.matrix(&b.name(&format!("{stem}.w3.weight")))?;
        if w1.dimensions() != (c.expert_width, c.geometry.hidden)
            || w2.dimensions() != (c.geometry.hidden, c.expert_width)
            || w3.dimensions() != w1.dimensions()
        {
            return Err(CpuError::Invalid("expert checkpoint dimensions"));
        }
        moe::expert_with_arithmetic(
            input,
            w1.as_ref(),
            w2.as_ref(),
            w3.as_ref(),
            route_weight,
            limit,
            b.arithmetic,
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::projection::tests::{Store, Tensor, tiny};
    use super::*;
    use serde::Deserialize;
    use std::{cell::RefCell, collections::BTreeMap};
    #[derive(Deserialize)]
    pub struct Fixture {
        pub tensors: BTreeMap<String, Tensor>,
        pub residual: Vec<f32>,
        pub previous_pre: Vec<f32>,
        pub attention_output: Vec<f32>,
        pub hashes: Vec<u64>,
        pub expected: BTreeMap<String, Vec<f32>>,
        pub expert_ids: Vec<usize>,
    }
    pub fn config() -> BlockConfig {
        BlockConfig {
            geometry: tiny(),
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
        }
    }
    pub fn load() -> Result<(Store, Fixture), Box<dyn std::error::Error>> {
        let f: Fixture =
            serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/block.json"))?;
        let tensors = f
            .tensors
            .iter()
            .map(|(n, t)| {
                (
                    n.clone(),
                    Tensor {
                        shape: t.shape.clone(),
                        values: t.values.clone(),
                    },
                )
            })
            .collect();
        Ok((
            Store {
                tensors,
                reads: RefCell::new(Vec::new()),
            },
            f,
        ))
    }
    fn near(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert!((a - b).abs() < 5e-6, "{a} != {b}");
        }
    }
    #[test]
    fn complete_named_block_matches_numpy_intermediates() -> Result<(), Box<dyn std::error::Error>>
    {
        let (s, f) = load()?;
        let b = TextBlock::new(&s, 1, config(), Arithmetic::Float32)?;
        let residual = b.apply_engram(&f.residual, &f.hashes, 64, 2)?;
        near(&residual, &f.expected["engram"]);
        let a = b.begin_attention(&residual, &f.previous_pre)?;
        near(&a.normalized, &f.expected["attention_input"]);
        near(&a.mixes.pre, &f.expected["attn_pre"]);
        near(&a.mixes.comb, &f.expected["attn_comb"]);
        let residual = b.finish_attention(&f.attention_output, &residual, &a.mixes)?;
        near(&residual, &f.expected["after_attention"]);
        let out = b.feed_forward(&residual, &a.mixes.pre)?;
        near(&out.residual, &f.expected["output"]);
        near(&out.next_pre, &f.expected["next_pre"]);
        near(&out.routing.weights, &f.expected["route_weights"]);
        assert_eq!(out.routing.indices, f.expert_ids);
        let reads = s.reads.borrow();
        for (name, _) in reads.iter().filter(|(n, _)| n.contains("ffn.experts.")) {
            let id: usize = name.split('.').nth(4).ok_or("expert name")?.parse()?;
            assert!(f.expert_ids.contains(&id));
        }
        let rows: Vec<_> = reads
            .iter()
            .filter(|(n, _)| n.ends_with("engram.embed.weight"))
            .map(|(_, i)| *i as u64)
            .collect();
        assert_eq!(rows, f.hashes);
        Ok(())
    }
    #[test]
    fn wrong_premix_missing_weights_and_shapes_are_refused()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut s, f) = load()?;
        assert!(
            TextBlock::new(&s, 1, config(), Arithmetic::Float32)?
                .begin_attention(&f.residual, &[1.0])
                .is_err()
        );
        s.tensors.remove("layers.1.ffn.shared_experts.w2.weight");
        let b = TextBlock::new(&s, 1, config(), Arithmetic::Float32)?;
        assert!(b.feed_forward(&f.residual, &f.previous_pre).is_err());
        assert!(TextBlock::new(&s, 40, config(), Arithmetic::Float32).is_err());
        assert!(
            TextBlock::new(
                &s,
                1,
                BlockConfig {
                    expert_count: 1,
                    ..config()
                },
                Arithmetic::Float32
            )
            .is_err()
        );
        Ok(())
    }
}
