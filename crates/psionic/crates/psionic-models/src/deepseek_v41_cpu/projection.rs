//! Named checkpoint projection wiring from the pinned `model.py` reference.

use super::quantization::{self, Arithmetic, Quantization};
use super::{
    CheckpointMatrix, CpuError, MatrixRows, finite, matvec, positive, product, rms_norm, shape,
};
use crate::deepseek_v41_weights::DeepSeekV41Checkpoint;

/// Weights are fetched by checkpoint name; matrices remain row-addressable.
pub trait WeightSource {
    fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError>;
    fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError>;
}

impl WeightSource for DeepSeekV41Checkpoint {
    fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
        Ok(Box::new(CheckpointMatrix::new(self, name)?))
    }
    fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
        if self.entry(name)?.shape.len() != 1 {
            return Err(CpuError::Invalid("expected a checkpoint vector"));
        }
        let values = self.dequantize_row(name, 0)?;
        finite("checkpoint vector", &values)?;
        Ok(values)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub hidden: usize,
    pub heads: usize,
    pub head_dim: usize,
    pub q_rank: usize,
    pub o_groups: usize,
    pub o_rank: usize,
    pub index_heads: usize,
    pub index_dim: usize,
    pub rope_dim: usize,
    pub window: usize,
    pub index_top_k: usize,
    pub norm_epsilon: f32,
}

impl Geometry {
    pub fn flash() -> Self {
        Self {
            hidden: 5120,
            heads: 64,
            head_dim: 512,
            q_rank: 1280,
            o_groups: 8,
            o_rank: 1024,
            index_heads: 32,
            index_dim: 128,
            rope_dim: 64,
            window: 128,
            index_top_k: 512,
            norm_epsilon: 1e-20,
        }
    }
    pub fn validate(&self) -> Result<(), CpuError> {
        if [
            self.hidden,
            self.heads,
            self.head_dim,
            self.q_rank,
            self.o_groups,
            self.o_rank,
            self.index_heads,
            self.index_dim,
            self.rope_dim,
            self.window,
            self.index_top_k,
        ]
        .contains(&0)
            || !self.heads.is_multiple_of(self.o_groups)
            || !self.rope_dim.is_multiple_of(2)
            || self.rope_dim > self.head_dim.min(self.index_dim)
            || !positive(self.norm_epsilon)
        {
            return Err(CpuError::Invalid("V4.1 attention geometry"));
        }
        product(self.heads, self.head_dim)?;
        product(self.o_groups, self.o_rank)?;
        product(self.index_heads, self.index_dim)?;
        Ok(())
    }
}

pub fn compress_ratio(layer: usize) -> Result<usize, CpuError> {
    match layer {
        0..=1 => Ok(0),
        2..=19 => Ok(2),
        20..=39 => Ok(1),
        _ => Err(CpuError::Invalid("backbone layer out of range")),
    }
}

/// Low-rank query kept separately because the indexer reuses it.
#[derive(Debug, Clone)]
pub struct Query {
    pub low_rank: Vec<f32>,
    pub heads: Vec<f32>,
}

pub struct AttentionProjections<'a> {
    pub(crate) weights: &'a dyn WeightSource,
    pub(crate) layer: usize,
    pub(crate) geometry: Geometry,
    pub(crate) arithmetic: Arithmetic,
    reference_rows: usize,
}

impl<'a> AttentionProjections<'a> {
    pub(crate) fn with_reference_rows(mut self, rows: usize) -> Self {
        self.reference_rows = rows;
        self
    }
    pub fn with_arithmetic(mut self, arithmetic: Arithmetic) -> Result<Self, CpuError> {
        let g = self.geometry;
        if arithmetic.is_bf16()
            && [
                g.hidden,
                g.q_rank,
                g.head_dim,
                g.heads * g.head_dim / g.o_groups,
                g.o_groups * g.o_rank,
                g.index_dim,
            ]
            .iter()
            .any(|n| !n.is_multiple_of(32))
        {
            return Err(CpuError::Invalid(
                "reference activation widths must be multiples of 32",
            ));
        }
        self.arithmetic = arithmetic;
        Ok(self)
    }
    pub(crate) fn finish(&self, values: Vec<f32>) -> Result<Vec<f32>, CpuError> {
        if self.arithmetic.is_bf16() {
            quantization::round_bf16(&values)
        } else {
            Ok(values)
        }
    }
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn new(
        weights: &'a dyn WeightSource,
        layer: usize,
        geometry: Geometry,
    ) -> Result<Self, CpuError> {
        geometry.validate()?;
        compress_ratio(layer)?;
        Ok(Self {
            weights,
            layer,
            geometry,
            arithmetic: Arithmetic::Float32,
            reference_rows: 1,
        })
    }
    fn name(&self, part: &str) -> String {
        format!("layers.{}.attn.{part}", self.layer)
    }
    fn project(
        &self,
        part: &str,
        input: &[f32],
        rows: usize,
        cols: usize,
    ) -> Result<Vec<f32>, CpuError> {
        let weight = self.weights.matrix(&self.name(part))?;
        if weight.dimensions() != (rows, cols) {
            return Err(CpuError::Invalid("attention projection shape"));
        }
        if self.arithmetic.is_bf16()
            && compress_ratio(self.layer)? > 1
            && part.starts_with("compressor.")
        {
            // convert.py promotes these BF16 checkpoint weights to F32.
            matvec(weight.as_ref(), input)
        } else {
            quantization::linear(weight.as_ref(), input, self.arithmetic)
        }
    }
    fn norm(&self, part: &str, input: &[f32]) -> Result<Vec<f32>, CpuError> {
        let weight = self.weights.vector(&self.name(part))?;
        let weight = if self.arithmetic.is_bf16() {
            quantization::round_bf16(&weight)?
        } else {
            weight
        };
        self.finish(if self.arithmetic == Arithmetic::ReferenceBf16Tp8 {
            super::rms_norm_reference(
                input,
                &weight,
                self.geometry.norm_epsilon,
                self.reference_rows,
            )?
        } else {
            rms_norm(input, &weight, self.geometry.norm_epsilon)?
        })
    }
    pub fn query(&self, input: &[f32]) -> Result<Query, CpuError> {
        let g = self.geometry;
        let low_rank = self.norm(
            "q_norm.weight",
            &self.project("wq_a.weight", input, g.q_rank, g.hidden)?,
        )?;
        let heads = self.project(
            "wq_b.weight",
            &low_rank,
            product(g.heads, g.head_dim)?,
            g.q_rank,
        )?;
        Ok(Query { low_rank, heads })
    }
    pub fn window_kv(&self, input: &[f32]) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        self.norm(
            "kv_norm.weight",
            &self.project("wkv.weight", input, g.head_dim, g.hidden)?,
        )
    }
    pub fn compressor_inputs(
        &self,
        input: &[f32],
    ) -> Result<(Vec<f32>, Option<Vec<f32>>), CpuError> {
        let ratio = compress_ratio(self.layer)?;
        let g = self.geometry;
        if ratio == 0 {
            return Err(CpuError::Invalid("window-only layer has no compressor"));
        }
        let value = self.project("compressor.wkv.weight", input, g.head_dim, g.hidden)?;
        let gate = if ratio > 1 {
            Some(self.project("compressor.wgate.weight", input, g.head_dim, g.hidden)?)
        } else {
            None
        };
        Ok((value, gate))
    }
    pub fn compressor_norm(&self) -> Result<Vec<f32>, CpuError> {
        let values = self.weights.vector(&self.name("compressor.norm.weight"))?;
        shape("compressor norm", &values, self.geometry.head_dim)?;
        Ok(values)
    }
    pub fn index_key(&self, latent: &[f32]) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        self.norm(
            "indexer.k_norm.weight",
            &self.project("indexer.wk.weight", latent, g.index_dim, g.head_dim)?,
        )
    }
    pub fn index_query(&self, low_rank: &[f32]) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        self.project(
            "indexer.wq_b.weight",
            low_rank,
            product(g.index_heads, g.index_dim)?,
            g.q_rank,
        )
    }
    pub fn index_weights(&self, input: &[f32]) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        self.project(
            "indexer.weights_proj.weight",
            input,
            g.index_heads,
            g.hidden,
        )
    }
    pub fn sinks(&self) -> Result<Vec<f32>, CpuError> {
        let values = self.weights.vector(&self.name("attn_sink"))?;
        shape("attention sinks", &values, self.geometry.heads)?;
        Ok(values)
    }
    /// wo_a is block-diagonal: each group sees only its own attention heads.
    pub fn output(&self, heads: &[f32]) -> Result<Vec<f32>, CpuError> {
        let low_rank = self.output_low_rank(heads)?;
        self.output_from_low_rank(&low_rank)
    }
    /// Grouped projection boundary before the TP8 row-parallel projection.
    pub fn output_low_rank(&self, heads: &[f32]) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        let width = product(g.heads, g.head_dim)? / g.o_groups;
        shape("attention heads", heads, product(g.heads, g.head_dim)?)?;
        let heads = if self.arithmetic.is_bf16() {
            quantization::inplace_bf16(heads, Quantization::Fp8)?
        } else {
            heads.to_vec()
        };
        let weight = self.weights.matrix(&self.name("wo_a.weight"))?;
        if weight.dimensions() != (product(g.o_groups, g.o_rank)?, width) {
            return Err(CpuError::Invalid("grouped output projection shape"));
        }
        let low_rank = weight.multiply_grouped(&heads, g.o_groups, self.arithmetic.is_bf16())?;
        finite("grouped output", &low_rank)?;
        self.finish(low_rank)
    }
    pub fn output_from_low_rank(&self, low_rank: &[f32]) -> Result<Vec<f32>, CpuError> {
        let g = self.geometry;
        shape(
            "attention output low rank",
            low_rank,
            product(g.o_groups, g.o_rank)?,
        )?;
        let weight = self.weights.matrix(&self.name("wo_b.weight"))?;
        if weight.dimensions() != (g.hidden, product(g.o_groups, g.o_rank)?) {
            return Err(CpuError::Invalid("wo_b dimensions"));
        }
        quantization::linear_row_parallel(weight.as_ref(), low_rank, self.arithmetic)
    }
}

pub fn embedding(
    weights: &dyn WeightSource,
    token: usize,
    width: usize,
) -> Result<Vec<f32>, CpuError> {
    let matrix = weights.matrix("embed.weight")?;
    let (rows, cols) = matrix.dimensions();
    if token >= rows || width == 0 || cols != width {
        return Err(CpuError::Invalid("embedding token or width"));
    }
    let value = matrix.row(token)?;
    shape("embedding row", &value, width)?;
    Ok(value)
}

/// Final collapse uses the last block's pre-mix, then RMS norm and a float32 head.
pub fn final_logits(
    weights: &dyn WeightSource,
    residual: &[f32],
    pre_mix: &[f32],
    width: usize,
    epsilon: f32,
) -> Result<Vec<f32>, CpuError> {
    final_logits_with_arithmetic(
        weights,
        residual,
        pre_mix,
        width,
        epsilon,
        Arithmetic::Float32,
    )
}

pub fn final_logits_with_arithmetic(
    weights: &dyn WeightSource,
    residual: &[f32],
    pre_mix: &[f32],
    width: usize,
    epsilon: f32,
    arithmetic: Arithmetic,
) -> Result<Vec<f32>, CpuError> {
    let (_, normalized) =
        final_input_with_arithmetic(weights, residual, pre_mix, width, epsilon, arithmetic)?;
    matvec(weights.matrix("head.weight")?.as_ref(), &normalized)
}

/// The collapsed BF16 stream and normalized head input, exposed for qualification.
pub fn final_input_with_arithmetic(
    weights: &dyn WeightSource,
    residual: &[f32],
    pre_mix: &[f32],
    width: usize,
    epsilon: f32,
    arithmetic: Arithmetic,
) -> Result<(Vec<f32>, Vec<f32>), CpuError> {
    final_input_with_reference_rows(weights, residual, pre_mix, width, epsilon, arithmetic, 1)
}
pub fn final_input_with_reference_rows(
    weights: &dyn WeightSource,
    residual: &[f32],
    pre_mix: &[f32],
    width: usize,
    epsilon: f32,
    arithmetic: Arithmetic,
    rows: usize,
) -> Result<(Vec<f32>, Vec<f32>), CpuError> {
    use super::hyperconnections::HyperConnections;
    let hc = HyperConnections {
        streams: pre_mix.len(),
        width,
        norm_epsilon: epsilon,
        ..HyperConnections::flash()
    };
    let hidden = hc.collapse(residual, pre_mix)?;
    let hidden = if arithmetic.is_bf16() {
        quantization::round_bf16(&hidden)?
    } else {
        hidden
    };
    let norm = weights.vector("norm.weight")?;
    let normalized = if arithmetic == Arithmetic::ReferenceBf16Tp8 {
        super::rms_norm_reference(&hidden, &norm, epsilon, rows)?
    } else {
        rms_norm(&hidden, &norm, epsilon)?
    };
    let normalized = if arithmetic.is_bf16() {
        quantization::round_bf16(&normalized)?
    } else {
        normalized
    };
    Ok((hidden, normalized))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::MatrixRef;
    use super::*;
    use serde::Deserialize;
    use std::{cell::RefCell, collections::BTreeMap};
    #[derive(Deserialize)]
    pub struct Tensor {
        pub shape: Vec<usize>,
        pub values: Vec<f32>,
    }
    #[derive(Deserialize)]
    pub struct Fixture {
        pub tensors: BTreeMap<String, Tensor>,
        pub input: Vec<f32>,
        pub expected: BTreeMap<String, Vec<f32>>,
    }
    pub struct Store {
        pub tensors: BTreeMap<String, Tensor>,
        pub reads: RefCell<Vec<(String, usize)>>,
    }
    struct Rows<'a> {
        name: &'a str,
        tensor: &'a Tensor,
        reads: &'a RefCell<Vec<(String, usize)>>,
    }
    impl MatrixRows for Rows<'_> {
        fn dimensions(&self) -> (usize, usize) {
            (self.tensor.shape[0], self.tensor.shape[1])
        }
        fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
            self.reads.borrow_mut().push((self.name.to_owned(), index));
            MatrixRef::new(
                self.tensor.shape[0],
                self.tensor.shape[1],
                &self.tensor.values,
            )?
            .row(index)
        }
    }
    impl WeightSource for Store {
        fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
            let (key, tensor) = self
                .tensors
                .get_key_value(name)
                .ok_or(CpuError::Invalid("missing fixture weight"))?;
            if tensor.shape.len() != 2 {
                return Err(CpuError::Invalid("fixture matrix shape"));
            }
            Ok(Box::new(Rows {
                name: key,
                tensor,
                reads: &self.reads,
            }))
        }
        fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
            Ok(self
                .tensors
                .get(name)
                .ok_or(CpuError::Invalid("missing fixture vector"))?
                .values
                .clone())
        }
    }
    pub fn tiny() -> Geometry {
        Geometry {
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
        }
    }
    pub fn load()
    -> Result<(Store, Vec<f32>, BTreeMap<String, Vec<f32>>), Box<dyn std::error::Error>> {
        let f: Fixture =
            serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/projection.json"))?;
        Ok((
            Store {
                tensors: f.tensors,
                reads: RefCell::new(Vec::new()),
            },
            f.input,
            f.expected,
        ))
    }
    fn near(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 3e-6, "{x} != {y}");
        }
    }
    #[test]
    fn named_projections_match_numpy() -> Result<(), Box<dyn std::error::Error>> {
        let (s, x, e) = load()?;
        let p = AttentionProjections::new(&s, 2, tiny())?;
        let q = p.query(&x)?;
        near(&q.low_rank, &e["qr"]);
        near(&q.heads, &e["q"]);
        near(&p.window_kv(&x)?, &e["kv"]);
        let (v, g) = p.compressor_inputs(&x)?;
        near(&v, &e["compress_value"]);
        assert!(g.is_some());
        if let Some(g) = g {
            near(&g, &e["compress_gate"]);
        }
        near(&p.index_key(&v)?, &e["index_key"]);
        near(&p.index_query(&q.low_rank)?, &e["index_query"]);
        near(&p.index_weights(&x)?, &e["index_weights"]);
        near(&p.output(&q.heads)?, &e["output"]);
        near(
            &final_logits(&s, &e["residual"], &[0.25, 0.75], 4, 1e-20)?,
            &e["logits"],
        );
        Ok(())
    }
    #[test]
    fn embedding_reads_one_row_and_grouped_projection_rejects_bad_shapes()
    -> Result<(), Box<dyn std::error::Error>> {
        let (s, _, e) = load()?;
        near(&embedding(&s, 2, 4)?, &e["embedding"]);
        assert_eq!(*s.reads.borrow(), vec![("embed.weight".into(), 2)]);
        assert!(embedding(&s, 99, 4).is_err());
        let p = AttentionProjections::new(&s, 2, tiny())?;
        assert!(p.output(&[0.0; 8]).is_err());
        assert!(p.query(&[f32::NAN; 4]).is_err());
        assert!(AttentionProjections::new(&s, 40, tiny()).is_err());
        assert!(
            AttentionProjections::new(
                &s,
                2,
                Geometry {
                    o_groups: 3,
                    ..tiny()
                }
            )
            .is_err()
        );
        assert!(
            AttentionProjections::new(&s, 3, tiny())?
                .query(&[1.0; 4])
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn output_groups_never_read_other_heads() -> Result<(), Box<dyn std::error::Error>> {
        let (mut s, _, _) = load()?;
        s.tensors.insert(
            "layers.2.attn.wo_b.weight".into(),
            Tensor {
                shape: vec![4, 4],
                values: vec![
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ],
            },
        );
        let p = AttentionProjections::new(&s, 2, tiny())?;
        let mut heads = vec![0.0; 16];
        heads[..8].fill(1.0);
        let out = p.output(&heads)?;
        assert!(out[..2].iter().any(|v| v.abs() > 1e-4));
        assert_eq!(&out[2..], &[0.0, 0.0]);
        Ok(())
    }
}
