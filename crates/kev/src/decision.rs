//! The assembled decision model: backbone plus LoRA plus pointer head, and
//! the request-in/probabilities-out path serving runs.

use std::path::Path;

use candle_core::{DType, Device, Tensor};
use tokenizers::Tokenizer;

use crate::api::{Answer, Meta, Record, SystemOneRequest, to_answers, to_record};
use crate::artifacts::{ArtifactIdentity, ArtifactReader};
use crate::encode::{Encoding, branch_mask, encode};
use crate::error::{Error, Result};
use crate::head::PointerHead;
use crate::lora::apply_lora_tracked;
use crate::model::Backbone;

use indexmap::IndexMap;

/// The `head_meta.json` fields the loader reads; the generator writes one
/// beside `head.safetensors` from the reference's `head.pt` metadata.
#[derive(Debug, serde::Deserialize)]
struct HeadMeta {
    /// Whether the checkpoint was trained with every option span isolated.
    #[serde(default)]
    option_isolation: bool,
}

/// Backbone plus LoRA adapter plus pointer head, on one device.
pub struct DecisionModel {
    /// The decoder trunk, adapter merged in.
    pub backbone: Backbone,
    /// The pointer readout.
    pub head: PointerHead,
    /// The artifact bundle's tokenizer.
    pub tokenizer: Tokenizer,
    /// Every option span isolated as its own sub-branch.
    pub option_isolation: bool,
    /// The device the weights live on.
    pub device: Device,
    /// Content identity captured from the same bytes used to load the model.
    pub artifacts: ArtifactIdentity,
}

impl DecisionModel {
    /// Load the bundle in `f32`: `base_dir` holds the backbone checkpoint,
    /// `adapter_dir` holds `adapter_config.json` +
    /// `adapter_model.safetensors` + `head.safetensors` + `tokenizer.json`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] or [`Error::Tokenize`] for missing or
    /// malformed files.
    pub fn load(base_dir: &Path, adapter_dir: &Path, device: Device) -> Result<Self> {
        Self::load_with_dtype(base_dir, adapter_dir, device, DType::F32)
    }

    /// Load the bundle with an explicit compute dtype.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] or [`Error::Tokenize`] for missing or
    /// malformed files.
    pub fn load_with_dtype(
        base_dir: &Path,
        adapter_dir: &Path,
        device: Device,
        dtype: DType,
    ) -> Result<Self> {
        let mut reader = ArtifactReader::default();
        let mut backbone = Backbone::load_tracked(base_dir, &device, dtype, &mut reader)?;
        apply_lora_tracked(&mut backbone, adapter_dir, &device, &mut reader)?;
        let head = PointerHead::load_tracked(adapter_dir, &device, &mut reader)?;
        let tokenizer_bytes = reader.read(
            "adapter/tokenizer.json",
            &adapter_dir.join("tokenizer.json"),
        )?;
        let tokenizer =
            Tokenizer::from_bytes(&tokenizer_bytes).map_err(|e| Error::Tokenize(e.to_string()))?;
        let option_isolation = match reader.optional(
            "adapter/head_meta.json",
            &adapter_dir.join("head_meta.json"),
        )? {
            Some(bytes) => {
                serde_json::from_slice::<HeadMeta>(&bytes)
                    .map_err(|e| Error::Artifact(format!("head_meta.json: {e}")))?
                    .option_isolation
            }
            None => false,
        };
        Ok(Self {
            backbone,
            head,
            tokenizer,
            option_isolation,
            device,
            artifacts: reader.finish()?,
        })
    }

    /// Pack a rendered record the same way serving does.
    ///
    /// # Errors
    ///
    /// Propagates [`encode`] errors.
    pub fn encode(&self, record: &Record, max_state: usize, max_branch: usize) -> Result<Encoding> {
        encode(
            &self.tokenizer,
            record,
            max_state,
            max_branch,
            false,
            self.option_isolation,
        )
    }

    /// The additive `[len, len]` mask for one encoding, `f32::MIN` where a
    /// query may not attend.
    fn additive_mask(&self, enc: &Encoding) -> Result<Tensor> {
        let opts = enc.option_isolation.then(|| std::slice::from_ref(&enc.opt));
        let allow = branch_mask(std::slice::from_ref(&enc.seg), opts);
        let len = enc.ids.len();
        let flat: Vec<f32> = allow[0]
            .iter()
            .flat_map(|row| row.iter().map(|yes| if *yes { 0.0 } else { f32::MIN }))
            .collect();
        Ok(Tensor::from_vec(flat, (len, len), &self.device)?.to_dtype(self.backbone.dtype())?)
    }

    /// One option distribution per question for a packed encoding.
    ///
    /// # Errors
    ///
    /// Propagates candle errors from the forward pass.
    pub fn probs(&self, enc: &Encoding) -> Result<Vec<Vec<f64>>> {
        let mask = self.additive_mask(enc)?;
        let hidden = self.backbone.hidden(&enc.ids, &enc.pos, &mask)?;
        self.head.probs(&hidden.to_dtype(DType::F32)?, enc)
    }

    /// The full serving path: request in, typed answers out, in question-id
    /// order.
    ///
    /// # Errors
    ///
    /// Propagates validation, encoding, and forward errors.
    pub fn systemone(
        &self,
        request: &SystemOneRequest,
        max_state: usize,
        max_branch: usize,
    ) -> Result<IndexMap<String, Answer>> {
        let (record, meta): (Record, Vec<Meta>) = to_record(request)?;
        let enc = self.encode(&record, max_state, max_branch)?;
        let probs = self.probs(&enc)?;
        Ok(to_answers(&probs, &meta))
    }
}
