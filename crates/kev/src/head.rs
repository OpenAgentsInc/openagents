//! The pointer head: a query from the decision token's hidden state scored
//! against a key from each option's closing delimiter, softmax over options.
//!
//! Mirrors `kev/model.py::PointerHead`: two `896 → 256` linear maps and a
//! `1/sqrt(256)` scale on the dot product.

use std::path::Path;

use candle_core::{Device, Tensor};

use crate::encode::Encoding;
use crate::error::{Error, Result};
use crate::model::Linear;

/// Two linear maps projecting hidden states into pointer space.
pub struct PointerHead {
    /// Query map applied to the decision token.
    pub q: Linear,
    /// Key map applied to each option's closing token.
    pub k: Linear,
    /// Pointer dimension; the scale is `1/sqrt(dp)`.
    pub dp: usize,
}

impl PointerHead {
    /// Load `head.safetensors`: `q.weight`, `q.bias`, `k.weight`, `k.bias`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] when a tensor is missing or mis-shaped.
    pub fn load(dir: &Path, device: &Device) -> Result<Self> {
        let tensors = candle_core::safetensors::load(dir.join("head.safetensors"), device)
            .map_err(|e| Error::Artifact(format!("load head.safetensors: {e}")))?;
        let get = |name: &str| {
            tensors
                .get(name)
                .cloned()
                .ok_or_else(|| Error::Artifact(format!("missing tensor {name}")))
        };
        let q = Linear {
            weight: get("q.weight")?,
            bias: Some(get("q.bias")?),
        };
        let k = Linear {
            weight: get("k.weight")?,
            bias: Some(get("k.bias")?),
        };
        let dp = q.weight.dim(0)?;
        Ok(Self { q, k, dp })
    }

    /// Option logits for one question: `[K]` from the decision hidden state
    /// `h_decide` `[1, d]` and the option-close hidden states `h_opts` `[K, d]`.
    ///
    /// # Errors
    ///
    /// Propagates candle errors.
    pub fn logits(&self, h_decide: &Tensor, h_opts: &Tensor) -> candle_core::Result<Tensor> {
        let key = self.k.forward(h_opts)?; // [K, dp]
        let query = self.q.forward(h_decide)?; // [1, dp]
        key.matmul(&query.t()?.contiguous()?)?
            .squeeze(1)?
            .affine(1.0 / (self.dp as f64).sqrt(), 0.0)
    }

    /// One distribution per question for an encoding's hidden states.
    ///
    /// # Errors
    ///
    /// Propagates candle errors.
    pub fn probs(&self, hidden: &Tensor, enc: &Encoding) -> Result<Vec<Vec<f64>>> {
        let mut out = Vec::with_capacity(enc.decide_idx.len());
        for (decide, opts) in enc.decide_idx.iter().zip(&enc.opt_idx) {
            let h_decide = hidden.narrow(0, *decide, 1)?; // [1, d]
            let opt_ids = Tensor::from_vec(
                opts.iter().map(|i| *i as u32).collect::<Vec<_>>(),
                opts.len(),
                hidden.device(),
            )?;
            let h_opts = hidden.index_select(&opt_ids, 0)?;
            let logits = self.logits(&h_decide, &h_opts)?;
            let probs = candle_nn::ops::softmax_last_dim(&logits)?;
            out.push(probs.to_vec1::<f32>()?.into_iter().map(f64::from).collect());
        }
        Ok(out)
    }
}
