//! The loaded checkpoint: config, tokenizer, specials, and backbone, plus
//! `system_one`, the whole `state + questions → typed answers` call.

use std::path::Path;

use candle_core::Device;
use serde_json::{Value, json};
use tokenizers::Tokenizer;

use crate::api::{Meta, SystemOneRequest, to_answers};
use crate::artifacts::{ArtifactIdentity, ArtifactReader};
use crate::config::{EncoderConfig, HeadConfig, Specials};
use crate::encode::{Batch, Item, build_sequence, collate, option_count, serialize_state};
use crate::error::{Error, Result};
use crate::model::Backbone;

/// The id the reference stamps on every answer envelope — a constant in
/// `rl_agent_api.system_one`, not the checkpoint's `model_name`, so
/// typed-decisions answers still read `rl-agent`.
const WIRE_MODEL: &str = "rl-agent";

/// One loaded laya checkpoint: everything `system_one` needs that the
/// request does not carry.
pub struct DecisionModel {
    backbone: Backbone,
    tokenizer: Tokenizer,
    specials: Specials,
    config: HeadConfig,
    hidden_size: usize,
    attention_heads: usize,
    /// Identity of the bytes this model loaded, reported on the model
    /// card.
    pub artifacts: ArtifactIdentity,
    /// The device forwards run on.
    pub device: Device,
    /// The checkpoint's declared model id, published on the model card.
    /// Answer envelopes carry [`WIRE_MODEL`] instead — the reference
    /// never reads this field for them.
    pub model_name: String,
    /// The base encoder the checkpoint declares, reported on the card.
    pub encoder_id: String,
}

impl DecisionModel {
    /// Load a checkpoint directory: `rl_agent_config.json`,
    /// `encoder/config.json`, `tokenizer/{tokenizer,tokenizer_config}.json`,
    /// and `model.safetensors`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] for missing or malformed files and
    /// propagates the backbone's load errors.
    pub fn load(dir: &Path, device: Device) -> Result<Self> {
        let mut reader = ArtifactReader::default();
        reader.read("config", &dir.join("rl_agent_config.json"))?;
        reader.read("encoder/config", &dir.join("encoder/config.json"))?;
        reader.read("tokenizer/tokenizer", &dir.join("tokenizer/tokenizer.json"))?;
        reader.read(
            "tokenizer/tokenizer_config",
            &dir.join("tokenizer/tokenizer_config.json"),
        )?;
        reader.read("weights", &dir.join("model.safetensors"))?;
        let artifacts = reader.finish()?;

        let config = HeadConfig::load(&dir.join("rl_agent_config.json"))?;
        let encoder = EncoderConfig::load(&dir.join("encoder/config.json"))?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer/tokenizer.json"))
            .map_err(|e| Error::Tokenize(e.to_string()))?;
        let specials = Specials::load(&tokenizer, &dir.join("tokenizer/tokenizer_config.json"))?;
        let backbone = Backbone::load(
            dir,
            &encoder,
            config.head_layers,
            config.act_costs.len() + 1,
            &device,
        )?;
        Ok(Self {
            backbone,
            tokenizer,
            specials,
            hidden_size: encoder.hidden_size,
            attention_heads: encoder.num_attention_heads,
            model_name: config
                .model_name
                .clone()
                .unwrap_or_else(|| "rl-agent".to_string()),
            encoder_id: config.encoder.clone(),
            config,
            artifacts,
            device,
        })
    }

    /// The sequence-length bound this checkpoint's sequences obey.
    #[must_use]
    pub fn max_len(&self) -> usize {
        self.config.max_len
    }

    /// The question/options-half token budget.
    #[must_use]
    pub fn head_max_len(&self) -> usize {
        self.config.head_max_len
    }

    /// The encoder's residual width, for memory accounting.
    #[must_use]
    pub fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    /// The encoder's attention-head count, for memory accounting.
    #[must_use]
    pub fn attention_heads(&self) -> usize {
        self.attention_heads
    }

    /// `system_one(state, questions)`: encode one sequence per question,
    /// batch, forward, temperature-scale, and shape the typed answers the
    /// reference returns.
    ///
    /// # Errors
    ///
    /// Returns the validation [`Error`] for a request outside the
    /// contract, [`Error::OptionsDoNotFit`] when a question's options
    /// cannot each keep a marker inside `head_max_len`, and inference
    /// failures from the tokenizer or the tensor runtime.
    pub fn system_one(&self, request: &SystemOneRequest) -> Result<Value> {
        let (batch, metas, counts) = self.encode(request)?;
        let (logits, acts) = self.forward(&batch)?;
        self.answer(&batch, &metas, &counts, &logits, &acts)
    }

    /// Run the batch through the backbone, public so the serving layer can
    /// attribute work without owning the encode step.
    ///
    /// # Errors
    ///
    /// Propagates candle errors.
    pub fn forward(&self, batch: &Batch) -> Result<(candle_core::Tensor, candle_core::Tensor)> {
        self.backbone.forward(batch)
    }

    /// The encoder's hidden states for a batch, before the decision head.
    ///
    /// # Errors
    ///
    /// Propagates candle errors.
    pub fn hidden(&self, batch: &Batch) -> Result<candle_core::Tensor> {
        self.backbone.hidden(batch)
    }

    /// Encode a request into the batch a forward consumes, for callers
    /// that want the token accounting before evaluating. Question ids in
    /// errors are the caller's keys. Also returns each row's option
    /// count, which [`DecisionModel::answer`] needs for the temperature
    /// bucket.
    ///
    /// # Errors
    ///
    /// As [`DecisionModel::system_one`], plus
    /// [`Error::InvalidRequest`] when every question carries fewer than
    /// two options — the reference's act head reads top-2 of the
    /// batch-wide option grid and cannot run on one column.
    pub fn encode(&self, request: &SystemOneRequest) -> Result<(Batch, Vec<Meta>, Vec<usize>)> {
        let internals = request.to_internal()?;
        let state = serialize_state(&request.state);
        let mut items = Vec::with_capacity(internals.len());
        let mut metas = Vec::with_capacity(internals.len());
        let mut counts = Vec::with_capacity(internals.len());
        for (q, meta) in internals {
            let encoding = build_sequence(
                &self.tokenizer,
                &self.specials,
                &state,
                &q,
                None,
                self.config.max_len,
                self.config.head_max_len,
                false,
            )?;
            if encoding.markers.len() != option_count(&q) {
                return Err(Error::OptionsDoNotFit {
                    id: meta.id,
                    max: self.config.head_max_len,
                });
            }
            counts.push(encoding.markers.len());
            items.push(Item {
                encoding,
                qtype: q.qtype,
            });
            metas.push(meta);
        }
        let batch = collate(&items, self.specials.pad_id);
        if batch.marker_pos.first().map_or(0, Vec::len) < 2 {
            return Err(Error::InvalidRequest(
                "every question carries fewer than two options; at least one question needs two"
                    .to_string(),
            ));
        }
        Ok((batch, metas, counts))
    }

    /// Finish `system_one` over an [`DecisionModel::encode`]d batch and
    /// its forward output.
    ///
    /// # Errors
    ///
    /// Propagates candle-to-host readback errors.
    pub fn answer(
        &self,
        batch: &Batch,
        metas: &[Meta],
        counts: &[usize],
        logits: &candle_core::Tensor,
        acts: &candle_core::Tensor,
    ) -> Result<Value> {
        let logits: Vec<Vec<f32>> = logits.to_vec2()?;
        let acts: Vec<Vec<f32>> = acts.to_vec2()?;
        let mut probs = Vec::with_capacity(counts.len());
        let mut act_probs = Vec::with_capacity(counts.len());
        for (i, meta) in metas.iter().enumerate() {
            let k = counts[i];
            let temperature = self.config.temperature_for(meta.qtype, k);
            probs.push(softmax(&logits[i][..k], temperature));
            act_probs.push(softmax_row(&acts[i])[0]);
        }
        let answers = to_answers(&probs, &act_probs, metas);
        Ok(json!({
            "model": WIRE_MODEL,
            "answers": answers,
            "usage": {"input_tokens": batch.n_tokens, "output_tokens": 0},
        }))
    }
}

/// `np.exp(z - z.max()) / sum` over the temperature-scaled logits, in
/// `f64` like the reference's numpy path.
fn softmax(logits: &[f32], temperature: f64) -> Vec<f64> {
    let z: Vec<f64> = logits.iter().map(|&l| l as f64 / temperature).collect();
    let max = z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = z.iter().map(|v| (v - max).exp()).collect();
    let sum: f64 = exps.iter().sum();
    exps.iter().map(|e| e / sum).collect()
}

/// `torch.softmax(act.float(), -1)` for one row.
fn softmax_row(logits: &[f32]) -> Vec<f64> {
    softmax(logits, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softmax_normalizes_after_temperature() {
        let p = softmax(&[2.0, 0.0], 2.0);
        assert!((p[0] - (1.0 / (1.0 + (-1f64).exp()))).abs() < 1e-9);
        let total: f64 = p.iter().sum();
        assert!((total - 1.0).abs() < 1e-9);
        // Zero temperature path: division by zero is upstream's problem
        // too; a finite temperature is a config contract.
        let p = softmax(&[0.0, 0.0, 0.0], 1.0);
        assert!((p[0] - 1.0 / 3.0).abs() < 1e-9);
    }
}
