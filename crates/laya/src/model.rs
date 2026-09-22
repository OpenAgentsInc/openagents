//! The tensor half of the port: the ModernBERT encoder, loaded by
//! renaming the checkpoint's `encoder.*` keys to candle's `model.*`
//! layout, plus the decision [`Head`].

use std::collections::HashMap;
use std::path::Path;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::modernbert::{Config as BertConfig, ModernBert};

use crate::config::EncoderConfig;
use crate::encode::Batch;
use crate::error::{Error, Result};
use crate::head::Head;

/// The encoder plus the decision head, loaded in `F32` — the reference
/// keeps its modules in fp32 and only autocasts on CUDA, so fp32 is the
/// parity path on every device this port serves.
pub struct Backbone {
    encoder: ModernBert,
    head: Head,
    /// The device every forward runs on.
    pub device: Device,
}

impl Backbone {
    /// Load `model.safetensors` under `dir` for an encoder described by
    /// `encoder` and a head of `head_layers` Transformer blocks and
    /// `n_act` act outputs.
    ///
    /// The checkpoint's `encoder.` prefix becomes candle's `model.`
    /// prefix; `head.`, `scorer.`, `act_head.`, `type_emb.`, and
    /// `temperature` pass through. Tensors the port does not read
    /// (`temperature`, the buffer the config table supersedes) are
    /// tolerated rather than required.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] for an unreadable safetensors file and
    /// candle errors for absent or mis-shaped tensors.
    pub fn load(
        dir: &Path,
        encoder: &EncoderConfig,
        head_layers: usize,
        n_act: usize,
        device: &Device,
    ) -> Result<Self> {
        encoder.check_layer_pattern()?;
        let path = dir.join("model.safetensors");
        let tensors = candle_core::safetensors::load(&path, device)
            .map_err(|e| Error::Artifact(format!("load {}: {e}", path.display())))?;
        let renamed: HashMap<String, Tensor> = tensors
            .into_iter()
            .map(|(name, tensor)| {
                let name = name
                    .strip_prefix("encoder.")
                    .map_or(name.clone(), |rest| format!("model.{rest}"));
                (name, tensor)
            })
            .collect();
        let vb = VarBuilder::from_tensors(renamed, DType::F32, device);
        let bert = ModernBert::load(vb.clone(), &candle_config(encoder)?)?;
        let head = Head::load(vb, encoder.hidden_size, head_layers, n_act)?;
        Ok(Self {
            encoder: bert,
            head,
            device: device.clone(),
        })
    }

    /// The encoder's hidden states for a batch, before the decision head —
    /// exposed for parity diagnosis and per-layer conformance fixtures.
    ///
    /// # Errors
    ///
    /// Propagates candle errors.
    pub fn hidden(&self, batch: &Batch) -> Result<Tensor> {
        let n = batch.input_ids.len();
        let longest = batch.input_ids.first().map_or(0, Vec::len);
        let ids = Tensor::from_vec(batch.input_ids.concat(), (n, longest), &self.device)?;
        let attention =
            Tensor::from_vec(batch.attention_mask.concat(), (n, longest), &self.device)?;
        Ok(self.encoder.forward(&ids, &attention)?)
    }

    /// One forward over a padded batch: encoder, then head.
    ///
    /// Returns `(logits [b, kmax], act_logits [b, n_act])` as `f32`.
    ///
    /// # Errors
    ///
    /// Propagates candle errors from the tensor ops.
    pub fn forward(&self, batch: &Batch) -> Result<(Tensor, Tensor)> {
        let n = batch.input_ids.len();
        let longest = batch.input_ids.first().map_or(0, Vec::len);
        let kmax = batch.marker_pos.first().map_or(0, Vec::len);
        let ids = Tensor::from_vec(batch.input_ids.concat(), (n, longest), &self.device)?;
        let attention =
            Tensor::from_vec(batch.attention_mask.concat(), (n, longest), &self.device)?;
        let hidden = self.encoder.forward(&ids, &attention)?;
        let marker_pos = Tensor::from_vec(batch.marker_pos.concat(), (n, kmax), &self.device)?;
        let marker_mask = Tensor::from_vec(
            batch
                .marker_mask
                .concat()
                .into_iter()
                .map(u32::from)
                .collect::<Vec<_>>(),
            (n, kmax),
            &self.device,
        )?;
        let qtype = Tensor::from_vec(
            batch.qtype.iter().map(|&t| t as u32).collect::<Vec<_>>(),
            n,
            &self.device,
        )?;
        self.head
            .forward(&hidden, &attention, &marker_pos, &marker_mask, &qtype)
    }
}

/// Map the Hugging Face encoder config onto candle's `modernbert::Config`.
fn candle_config(e: &EncoderConfig) -> Result<BertConfig> {
    Ok(BertConfig {
        vocab_size: e.vocab_size,
        hidden_size: e.hidden_size,
        num_hidden_layers: e.num_hidden_layers,
        num_attention_heads: e.num_attention_heads,
        intermediate_size: e.intermediate_size,
        max_position_embeddings: e.max_position_embeddings,
        layer_norm_eps: e.layer_norm_eps,
        pad_token_id: e.pad_token_id,
        global_attn_every_n_layers: e.global_attn_every_n_layers,
        global_rope_theta: e.global_theta(),
        local_attention: e.local_attention,
        local_rope_theta: e.local_theta(),
        classifier_config: None,
    })
}
