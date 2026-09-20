//! Applying the peft LoRA adapter to the backbone at load time.
//!
//! `adapter_model.safetensors` carries `lora_A`/`lora_B` pairs per targeted
//! projection; the effective weight is `W + (alpha / r) * B @ A`, exactly
//! what peft computes for an unmerged inference adapter. The served model
//! keeps the merged form — the reference does the same through peft's
//! inference path.

use std::collections::HashMap;
use std::path::Path;

use candle_core::{DType, Device, Tensor};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::{Backbone, Linear};

/// The `adapter_config.json` fields the merge reads.
#[derive(Debug, Clone, Deserialize)]
pub struct LoraConfig {
    /// Adapter rank.
    pub r: usize,
    /// Adapter alpha; the merge scale is `lora_alpha / r`.
    pub lora_alpha: f64,
    /// Projection names the adapter targets, such as `q_proj`.
    pub target_modules: Vec<String>,
}

/// Fold `adapter_model.safetensors` into the backbone's projection weights.
///
/// # Errors
///
/// Returns [`Error::Artifact`] when the adapter is malformed, names a module
/// the backbone does not carry, or the pair for a module is incomplete.
pub fn apply_lora(backbone: &mut Backbone, dir: &Path, device: &Device) -> Result<()> {
    apply_lora_tracked(
        backbone,
        dir,
        device,
        &mut crate::artifacts::ArtifactReader::default(),
    )
}

pub(crate) fn apply_lora_tracked(
    backbone: &mut Backbone,
    dir: &Path,
    device: &Device,
    reader: &mut crate::artifacts::ArtifactReader,
) -> Result<()> {
    let config: LoraConfig = serde_json::from_slice(&reader.read(
        "adapter/adapter_config.json",
        &dir.join("adapter_config.json"),
    )?)?;
    let scale = config.lora_alpha / config.r as f64;
    let bytes = reader.read(
        "adapter/adapter_model.safetensors",
        &dir.join("adapter_model.safetensors"),
    )?;
    let loaded = candle_core::safetensors::load_buffer(&bytes, device)
        .map_err(|e| Error::Artifact(format!("load adapter_model.safetensors: {e}")))?;
    drop(bytes);
    let tensors: HashMap<String, Tensor> = loaded
        .into_iter()
        .map(|(k, v)| {
            // peft prefixes `base_model.model.`; the weights themselves
            // then match the backbone's `model.` layout.
            let name = k
                .strip_prefix("base_model.model.model.")
                .or_else(|| k.strip_prefix("base_model.model."))
                .unwrap_or(&k)
                .to_string();
            Ok((name, v.to_dtype(DType::F32)?))
        })
        .collect::<candle_core::Result<_>>()?;
    backbone.merge_lora(&tensors, &config.target_modules, scale)
}

/// `W + scale * B @ A` for one targeted projection, applied in place.
pub fn merge(linear: &mut Linear, a: &Tensor, b: &Tensor, scale: f64) -> candle_core::Result<()> {
    // A: [r, in], B: [out, r]; delta: [out, in].
    let delta = b.matmul(a)?.affine(scale, 0.0)?;
    linear.weight = (&linear.weight + delta.to_dtype(linear.weight.dtype())?)?;
    Ok(())
}
