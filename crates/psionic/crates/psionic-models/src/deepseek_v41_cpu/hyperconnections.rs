//! Hyper-connection mixing, following `model.py:950-995` and `kernel.py:407-459`.

use super::{CpuError, MatrixRows, finite, matvec, positive, product, shape, sigmoid};

#[derive(Clone, Copy, Debug)]
pub struct HyperConnections {
    pub streams: usize,
    pub width: usize,
    pub norm_epsilon: f32,
    pub mix_epsilon: f32,
    pub sinkhorn_iterations: usize,
}

impl HyperConnections {
    pub fn validate(&self) -> Result<(), CpuError> {
        self.dimensions().map(|_| ())
    }
    pub fn flash() -> Self {
        Self {
            streams: 4,
            width: 5120,
            norm_epsilon: 1e-20,
            mix_epsilon: 1e-6,
            sinkhorn_iterations: 20,
        }
    }

    fn dimensions(&self) -> Result<(usize, usize), CpuError> {
        if self.streams == 0
            || self.width == 0
            || self.sinkhorn_iterations == 0
            || !positive(self.norm_epsilon)
            || !positive(self.mix_epsilon)
        {
            return Err(CpuError::Invalid("hyper-connection configuration"));
        }
        let mixes = self
            .streams
            .checked_add(2)
            .ok_or(CpuError::Invalid("shape overflow"))?;
        Ok((
            product(self.streams, self.width)?,
            product(mixes, self.streams)?,
        ))
    }

    /// Normalize over all streams jointly before projecting to mix coefficients.
    pub fn mixes(
        &self,
        residual: &[f32],
        projection: &dyn MatrixRows,
        scale: &[f32],
        base: &[f32],
    ) -> Result<Mixes, CpuError> {
        self.mixes_impl(residual, projection, scale, base, None)
    }
    pub fn mixes_reference(
        &self,
        residual: &[f32],
        projection: &dyn MatrixRows,
        scale: &[f32],
        base: &[f32],
        rows: usize,
    ) -> Result<Mixes, CpuError> {
        self.mixes_impl(residual, projection, scale, base, Some(rows))
    }
    fn mixes_impl(
        &self,
        residual: &[f32],
        projection: &dyn MatrixRows,
        scale: &[f32],
        base: &[f32],
        rows: Option<usize>,
    ) -> Result<Mixes, CpuError> {
        let (elements, count) = self.dimensions()?;
        shape("hyper-connection residual", residual, elements)?;
        if projection.dimensions() != (count, elements) {
            return Err(CpuError::Invalid("hyper-connection projection shape"));
        }
        let variance = if let Some(rows) = rows {
            super::reference_row_mean(&residual.iter().map(|x| x * x).collect::<Vec<_>>(), rows)?
        } else {
            residual.iter().map(|x| x * x).sum::<f32>() / elements as f32
        };
        if !variance.is_finite() {
            return Err(CpuError::NonFinite("hyper-connection variance"));
        }
        let inv_rms = (variance + self.norm_epsilon).sqrt().recip();
        let projected: Vec<_> = matvec(projection, residual)?
            .into_iter()
            .map(|x| x * inv_rms)
            .collect();
        self.split_sinkhorn(&projected, scale, base)
    }

    pub fn split_sinkhorn(
        &self,
        projected: &[f32],
        scale: &[f32],
        base: &[f32],
    ) -> Result<Mixes, CpuError> {
        let (_, count) = self.dimensions()?;
        shape("projected mixes", projected, count)?;
        shape("mix base", base, count)?;
        shape("mix scale", scale, 3)?;
        let h = self.streams;
        let logits: Vec<_> = projected
            .iter()
            .enumerate()
            .map(|(i, x)| {
                x * scale[if i < h {
                    0
                } else if i < 2 * h {
                    1
                } else {
                    2
                }] + base[i]
            })
            .collect();
        finite("mix logits", &logits)?;
        let pre = logits[..h]
            .iter()
            .map(|x| sigmoid(*x) + self.mix_epsilon)
            .collect();
        let post = logits[h..2 * h].iter().map(|x| 2.0 * sigmoid(*x)).collect();
        let mut comb = logits[2 * h..].to_vec();
        for row in comb.chunks_exact_mut(h) {
            let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            for x in row.iter_mut() {
                *x = (*x - max).exp();
            }
            let sum: f32 = row.iter().sum();
            for x in row {
                *x = *x / sum + self.mix_epsilon;
            }
        }
        for iteration in 0..self.sinkhorn_iterations {
            if iteration > 0 {
                for row in comb.chunks_exact_mut(h) {
                    let sum = row.iter().sum::<f32>() + self.mix_epsilon;
                    for x in row {
                        *x /= sum;
                    }
                }
            }
            for k in 0..h {
                let sum = (0..h).map(|j| comb[j * h + k]).sum::<f32>() + self.mix_epsilon;
                for j in 0..h {
                    comb[j * h + k] /= sum;
                }
            }
        }
        Ok(Mixes { pre, post, comb })
    }

    /// Use the previous sub-block's pre-mix, not the one just computed here.
    pub fn collapse(&self, residual: &[f32], previous_pre: &[f32]) -> Result<Vec<f32>, CpuError> {
        shape("hyper-connection residual", residual, self.dimensions()?.0)?;
        shape("previous pre-mix", previous_pre, self.streams)?;
        let out: Vec<_> = (0..self.width)
            .map(|d| {
                (0..self.streams)
                    .map(|j| previous_pre[j] * residual[j * self.width + d])
                    .sum()
            })
            .collect();
        finite("collapsed residual", &out)?;
        Ok(out)
    }

    /// `comb[source, target]` mixes the residual into each target stream.
    pub fn expand(
        &self,
        output: &[f32],
        residual: &[f32],
        mixes: &Mixes,
    ) -> Result<Vec<f32>, CpuError> {
        let (elements, _) = self.dimensions()?;
        shape("sublayer output", output, self.width)?;
        shape("hyper-connection residual", residual, elements)?;
        shape("post-mix", &mixes.post, self.streams)?;
        shape(
            "residual mix",
            &mixes.comb,
            product(self.streams, self.streams)?,
        )?;
        let mut out = vec![0.0; elements];
        for k in 0..self.streams {
            for d in 0..self.width {
                out[k * self.width + d] = mixes.post[k] * output[d]
                    + (0..self.streams)
                        .map(|j| mixes.comb[j * self.streams + k] * residual[j * self.width + d])
                        .sum::<f32>();
            }
        }
        finite("expanded residual", &out)?;
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct Mixes {
    pub pre: Vec<f32>,
    pub post: Vec<f32>,
    /// Row-major `[source stream, target stream]`.
    pub comb: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek_v41_cpu::{MatrixRef, rms_norm};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        streams: usize,
        width: usize,
        residual: Vec<f32>,
        projection: Vec<f32>,
        scale: Vec<f32>,
        base: Vec<f32>,
        previous_pre: Vec<f32>,
        sublayer: Vec<f32>,
        pre: Vec<f32>,
        post: Vec<f32>,
        comb: Vec<f32>,
        collapsed: Vec<f32>,
        expanded: Vec<f32>,
    }

    fn near(a: &[f32], b: &[f32]) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert!((x - y).abs() < 3e-6, "{x} != {y}");
        }
    }

    #[test]
    fn collapse_and_expand_match_original_weight_oracle_activations()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::deepseek_v41_oracle::V41OracleFixtures;
        use crate::deepseek_v41_weights::decode_bf16;
        let oracle = V41OracleFixtures::load_checked_in()?;
        let values = |site: &str| -> Result<Vec<f32>, Box<dyn std::error::Error>> {
            let name = format!("p1-chat-thinking.decode.first.{site}");
            let raw = oracle
                .full_bytes(&name)
                .ok_or("whole oracle tensor missing")?;
            assert!(oracle.matches(&name, raw)?);
            Ok(match oracle.tensor(&name)?.dtype.as_str() {
                "bfloat16" => raw
                    .chunks_exact(2)
                    .map(|x| decode_bf16([x[0], x[1]]))
                    .collect(),
                "float32" => raw
                    .chunks_exact(4)
                    .map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]))
                    .collect(),
                _ => return Err("unexpected oracle dtype".into()),
            })
        };
        let check = |a: Vec<f32>, b: Vec<f32>| -> Result<(), CpuError> {
            let a = super::super::quantization::round_bf16(&a)?;
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert!((a - b).abs() <= 0.015625 + 0.015625 * b.abs(), "{a} != {b}");
            }
            Ok(())
        };
        let hc = HyperConnections::flash();
        let residual = values("layers.0.hc.in")?;
        check(
            hc.collapse(&residual, &values("layers.0.hc.pre_mix_in")?)?,
            values("layers.0.hc.attn.collapsed")?,
        )?;
        let mixes = Mixes {
            pre: values("layers.0.hc.attn.pre")?,
            post: values("layers.0.hc.attn.post")?,
            comb: values("layers.0.hc.attn.comb")?,
        };
        check(
            hc.expand(&values("layers.0.attn.out")?, &residual, &mixes)?,
            values("layers.0.hc.attn.expanded")?,
        )?;
        let residual = values("layers.0.hc.attn.expanded")?;
        check(
            hc.collapse(&residual, &mixes.pre)?,
            values("layers.0.hc.ffn.collapsed")?,
        )?;
        let mixes = Mixes {
            pre: values("layers.0.hc.ffn.pre")?,
            post: values("layers.0.hc.ffn.post")?,
            comb: values("layers.0.hc.ffn.comb")?,
        };
        check(
            hc.expand(&values("layers.0.ffn.out")?, &residual, &mixes)?,
            values("layers.0.hc.ffn.expanded")?,
        )?;
        Ok(())
    }
    #[test]
    fn matches_nonuniform_numpy_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let f: Fixture = serde_json::from_str(include_str!(
            "../testdata/deepseek_v41_cpu/hyperconnections.json"
        ))?;
        let hc = HyperConnections {
            streams: f.streams,
            width: f.width,
            ..HyperConnections::flash()
        };
        let matrix = MatrixRef::new(
            (hc.streams + 2) * hc.streams,
            hc.streams * hc.width,
            &f.projection,
        )?;
        let mixes = hc.mixes(&f.residual, &matrix, &f.scale, &f.base)?;
        near(&mixes.pre, &f.pre);
        near(&mixes.post, &f.post);
        near(&mixes.comb, &f.comb);
        near(&hc.collapse(&f.residual, &f.previous_pre)?, &f.collapsed);
        near(&hc.expand(&f.sublayer, &f.residual, &mixes)?, &f.expanded);
        for j in 0..hc.streams {
            assert!(
                (mixes.comb[j * hc.streams..(j + 1) * hc.streams]
                    .iter()
                    .sum::<f32>()
                    - 1.0)
                    .abs()
                    < 2e-5
            );
        }
        Ok(())
    }

    #[test]
    fn residual_matrix_is_source_then_target() -> Result<(), CpuError> {
        let hc = HyperConnections {
            streams: 2,
            width: 1,
            ..HyperConnections::flash()
        };
        let m = Mixes {
            pre: vec![],
            post: vec![2.0, 3.0],
            comb: vec![0.1, 0.2, 0.3, 0.4],
        };
        near(&hc.expand(&[5.0], &[10.0, 20.0], &m)?, &[17.0, 25.0]);
        Ok(())
    }

    #[test]
    fn rejects_invalid_shapes_and_nonfinite_inputs() {
        let hc = HyperConnections {
            streams: 2,
            width: 1,
            ..HyperConnections::flash()
        };
        assert!(hc.collapse(&[1.0], &[1.0, 0.0]).is_err());
        assert!(hc.collapse(&[f32::NAN, 1.0], &[1.0, 0.0]).is_err());
        assert!(
            hc.split_sinkhorn(&[0.0; 8], &[1.0; 3], &[f32::INFINITY; 8])
                .is_err()
        );
        assert!(
            HyperConnections {
                sinkhorn_iterations: 0,
                ..hc
            }
            .collapse(&[1.0, 2.0], &[1.0, 0.0])
            .is_err()
        );
        assert!(rms_norm(&[f32::MAX], &[1.0], 1e-20).is_err());
    }

    #[test]
    fn stable_extreme_logits_and_rms_norm() -> Result<(), CpuError> {
        let hc = HyperConnections {
            streams: 2,
            width: 1,
            ..HyperConnections::flash()
        };
        let m = hc.split_sinkhorn(
            &[
                1000.0, -1000.0, 1000.0, -1000.0, 1000.0, -1000.0, -1000.0, 1000.0,
            ],
            &[1.0; 3],
            &[0.0; 8],
        )?;
        assert_eq!(m.post, vec![2.0, 0.0]);
        assert!(m.comb.iter().all(|x| x.is_finite() && *x >= 0.0));
        near(
            &rms_norm(&[3.0, 4.0], &[1.0, 2.0], 1e-20)?,
            &[0.84852815, 2.2627417],
        );
        Ok(())
    }
}
