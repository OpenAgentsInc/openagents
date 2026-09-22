//! The decision head the checkpoints train on top of the encoder: a
//! per-type embedding added to every token, a pre-norm Transformer encoder
//! of `head_layers` blocks, a scorer that reads each option's marker, and
//! the act head that summarizes the answer distribution.
//!
//! Mirrors `DecisionModel` in `rl_common.py`: `TransformerEncoderLayer`
//! defaults apply (`relu` activation, `norm_first`, `eps = 1e-5`), the
//! scorer is `LayerNorm → Linear → GELU → Linear`, and the act head sees
//! the pooled first token plus detached features of its own softmax.

use candle_core::{D, DType, IndexOp, Tensor};
use candle_nn::{Embedding, LayerNorm, Linear, Module, VarBuilder, embedding, layer_norm, linear};

use crate::error::Result;

/// The mask fill the reference writes over absent options: `-1e4`, not
/// `-inf`, so a fully-masked row still softmaxes to a finite distribution.
const MASKED: f64 = -1e4;
/// The option-count normalization inside the act head's features.
const OPTION_SCALE: f64 = 255.0;

/// One `TransformerEncoderLayer(norm_first=True)`: pre-norm
/// self-attention, then pre-norm `linear1 → relu → linear2`.
struct HeadLayer {
    norm1: LayerNorm,
    in_proj: Linear,
    out_proj: Linear,
    norm2: LayerNorm,
    linear1: Linear,
    linear2: Linear,
    heads: usize,
}

impl HeadLayer {
    fn load(vb: VarBuilder, hidden: usize, heads: usize) -> Result<Self> {
        // nn.MultiheadAttention stores `in_proj_weight`/`in_proj_bias`,
        // not the `weight`/`bias` pair `candle_nn::linear` looks up.
        let attn = vb.pp("self_attn");
        Ok(Self {
            norm1: layer_norm(hidden, 1e-5, vb.pp("norm1"))?,
            in_proj: Linear::new(
                attn.get((3 * hidden, hidden), "in_proj_weight")?,
                Some(attn.get(3 * hidden, "in_proj_bias")?),
            ),
            out_proj: linear(hidden, hidden, attn.pp("out_proj"))?,
            norm2: layer_norm(hidden, 1e-5, vb.pp("norm2"))?,
            linear1: linear(hidden, 4 * hidden, vb.pp("linear1"))?,
            linear2: linear(4 * hidden, hidden, vb.pp("linear2"))?,
            heads,
        })
    }

    /// `layer(h, src_key_padding_mask=pad)`: `pad` is `f32::MIN` at
    /// padding positions, `0` elsewhere, shaped `[b, 1, 1, seq]` for the
    /// broadcast over queries.
    fn forward(&self, xs: &Tensor, pad: &Tensor) -> Result<Tensor> {
        let (b, seq, hidden) = xs.dims3()?;
        let head_dim = hidden / self.heads;
        let h = xs.apply(&self.norm1)?;
        let qkv = h
            .apply(&self.in_proj)?
            .reshape((b, seq, 3, self.heads, head_dim))?
            .permute((2, 0, 3, 1, 4))?;
        let q = qkv.get(0)?;
        let k = qkv.get(1)?;
        let v = qkv.get(2)?;
        let scale = (head_dim as f64).powf(-0.5);
        let att = (q * scale)?.matmul(&k.transpose(D::Minus2, D::Minus1)?)?;
        let att = att.broadcast_add(pad)?;
        let att = candle_nn::ops::softmax(&att, D::Minus1)?;
        let att = att.matmul(&v)?.transpose(1, 2)?.reshape((b, seq, hidden))?;
        let xs = (xs + att.apply(&self.out_proj)?)?;
        let ffn = xs
            .apply(&self.norm2)?
            .apply(&self.linear1)?
            .relu()?
            .apply(&self.linear2)?;
        Ok((xs + ffn)?)
    }
}

/// `scorer = LayerNorm → Linear → GELU → Linear(d, 1)` read at each
/// marker.
struct Scorer {
    norm: LayerNorm,
    first: Linear,
    last: Linear,
}

impl Scorer {
    fn load(vb: VarBuilder, hidden: usize) -> Result<Self> {
        Ok(Self {
            norm: layer_norm(hidden, 1e-5, vb.pp("0"))?,
            first: linear(hidden, hidden, vb.pp("1"))?,
            last: linear(hidden, 1, vb.pp("3"))?,
        })
    }

    fn forward(&self, markers: &Tensor) -> Result<Tensor> {
        Ok(markers
            .apply(&self.norm)?
            .apply(&self.first)?
            .gelu_erf()?
            .apply(&self.last)?
            .squeeze(D::Minus1)?)
    }
}

/// `act_head = Linear(d + 4, 256) → GELU → Linear(256, n_act)`.
struct ActHead {
    first: Linear,
    last: Linear,
}

impl ActHead {
    fn load(vb: VarBuilder, hidden: usize, n_act: usize) -> Result<Self> {
        Ok(Self {
            first: linear(hidden + 4, 256, vb.pp("0"))?,
            last: linear(256, n_act, vb.pp("2"))?,
        })
    }

    fn forward(&self, pooled: &Tensor, feats: &Tensor) -> Result<Tensor> {
        Ok(Tensor::cat(&[pooled, feats], D::Minus1)?
            .apply(&self.first)?
            .gelu_erf()?
            .apply(&self.last)?)
    }
}

/// The loaded decision head: type embedding, Transformer layers, scorer,
/// and act head.
pub struct Head {
    type_emb: Embedding,
    layers: Vec<HeadLayer>,
    scorer: Scorer,
    act: ActHead,
}

impl Head {
    /// Load `type_emb`, `head.layers.*`, `scorer.*`, and `act_head.*` from
    /// the safetensors map; `n_act` is `len(act_costs) + 1`.
    ///
    /// # Errors
    ///
    /// Returns a candle error when a tensor is absent or mis-shaped.
    pub fn load(vb: VarBuilder, hidden: usize, head_layers: usize, n_act: usize) -> Result<Self> {
        let type_emb = embedding(3, hidden, vb.pp("type_emb"))?;
        let heads = (hidden / 64).max(1);
        let mut layers = Vec::with_capacity(head_layers);
        for i in 0..head_layers {
            layers.push(HeadLayer::load(
                vb.pp(format!("head.layers.{i}")),
                hidden,
                heads,
            )?);
        }
        Ok(Self {
            type_emb,
            layers,
            scorer: Scorer::load(vb.pp("scorer"), hidden)?,
            act: ActHead::load(vb.pp("act_head"), hidden, n_act)?,
        })
    }

    /// `forward` over the encoder's hidden states: add the per-type
    /// embedding, run the Transformer layers under the padding mask, score
    /// the markers, mask absent options, then feed the act head the pooled
    /// first token and the detached distribution features.
    ///
    /// Returns `(logits [b, kmax], act_logits [b, n_act])`.
    ///
    /// # Errors
    ///
    /// Propagates candle errors from the tensor ops.
    pub fn forward(
        &self,
        hidden: &Tensor,
        attention: &Tensor,
        marker_pos: &Tensor,
        marker_mask: &Tensor,
        qtype: &Tensor,
    ) -> Result<(Tensor, Tensor)> {
        let (b, _seq, _hidden) = hidden.dims3()?;
        // `pad = ~attention_mask.bool()`: f32::MIN at padded keys.
        let pad = attention
            .to_dtype(DType::F32)?
            .unsqueeze(1)?
            .unsqueeze(2)?
            .affine(-1.0, 1.0)?
            .affine(f32::MIN as f64, 0.0)?;
        let types = self.type_emb.forward(qtype)?.unsqueeze(1)?;
        let mut h = hidden.broadcast_add(&types)?;
        for layer in &self.layers {
            h = layer.forward(&h, &pad)?;
        }
        // `marker_pos.clamp(min=0)` in the reference; positions are u32,
        // already non-negative.
        let idx = marker_pos
            .unsqueeze(D::Minus1)?
            .expand((b, marker_pos.dim(1)?, h.dim(2)?))?
            .contiguous()?;
        let markers = h.gather(&idx, 1)?;
        let mut logits = self.scorer.forward(&markers)?.to_dtype(DType::F32)?;
        // `masked_fill(~marker_mask, -1e4)`: 0 on real options, MASKED off.
        let absent = marker_mask.to_dtype(DType::F32)?.affine(1e4, MASKED)?;
        logits = logits.broadcast_add(&absent)?;
        let p = candle_nn::ops::softmax(&logits, D::Minus1)?;
        // k = marker_mask.sum(-1).clamp(min=2)
        let k = marker_mask
            .to_dtype(DType::F32)?
            .sum(D::Minus1)?
            .clamp(2.0, f64::INFINITY)?;
        // ent = -(p * log(clamp(p, 1e-9))).sum(-1) / log(k)
        let ent = ((&p * &p.clamp(1e-9, f64::INFINITY)?.log()?)?
            .neg()?
            .sum(D::Minus1)?
            .broadcast_div(&k.log()?))?;
        // top2 = p.topk(2): the max, then the max with the argmax zeroed.
        // With one column the second read is 0 — the reference's
        // `topk(2)` cannot run there; serving refuses single-option
        // requests before this point, so the fallback is unreachable.
        let top1 = p.max_keepdim(D::Minus1)?;
        let argmax = p.argmax_keepdim(D::Minus1)?;
        let zeros = Tensor::zeros(argmax.shape(), DType::F32, p.device())?;
        let zeroed = p.scatter(&argmax, &zeros, D::Minus1)?;
        let second = zeroed.max_keepdim(D::Minus1)?;
        let gap = (&top1 - &second)?.squeeze(D::Minus1)?;
        let feats = Tensor::stack(
            &[top1.squeeze(D::Minus1)?, gap, ent, (k / OPTION_SCALE)?],
            D::Minus1,
        )?;
        let pooled = h.i((.., 0, ..))?.to_dtype(DType::F32)?;
        let act = self.act.forward(&pooled, &feats)?;
        Ok((logits, act))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    // The head needs real weights to exercise end-to-end; the conformance
    // fixtures cover the numerical path. These tests pin the shapes the
    // safetensors keys imply without loading a checkpoint.

    #[test]
    fn masked_fill_marks_absent_options_finite() {
        let mask = Tensor::from_vec(vec![1f32, 1f32, 0f32, 0f32], (1, 4), &Device::Cpu).unwrap();
        let absent = mask.affine(1e4, MASKED).unwrap();
        let fill: Vec<f32> = absent.flatten_all().unwrap().to_vec1().unwrap();
        assert_eq!(fill, vec![0.0, 0.0, -1e4, -1e4]);
    }
}
