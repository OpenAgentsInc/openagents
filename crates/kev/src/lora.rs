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
/// For source-precision merging into a low-precision model, use
/// [`crate::DecisionModel::load_with_dtype`]; an already cast backbone
/// cannot recover its original precision here.
///
/// # Errors
///
/// Returns [`Error::Artifact`] when the adapter is malformed, names a module
/// the backbone does not carry, or the pair for a module is incomplete.
pub fn apply_lora(backbone: &mut Backbone, dir: &Path, device: &Device) -> Result<()> {
    let adapter = LoadedLora::load(
        dir,
        device,
        &mut crate::artifacts::ArtifactReader::default(),
    )?;
    backbone.merge_lora(
        &adapter.tensors,
        &adapter.config.target_modules,
        adapter.scale,
    )
}

/// Adapter factors stay in fp32 while base shards are loaded and released.
pub(crate) struct LoadedLora {
    config: LoraConfig,
    tensors: HashMap<String, Tensor>,
    pub(crate) scale: f64,
}

impl LoadedLora {
    pub(crate) fn load(
        dir: &Path,
        device: &Device,
        reader: &mut crate::artifacts::ArtifactReader,
    ) -> Result<Self> {
        let config: LoraConfig = serde_json::from_slice(&reader.read(
            "adapter/adapter_config.json",
            &dir.join("adapter_config.json"),
        )?)?;
        if config.r == 0 || !config.lora_alpha.is_finite() {
            return Err(Error::Artifact(
                "LoRA requires a positive rank and finite alpha".to_string(),
            ));
        }
        let scale = config.lora_alpha / config.r as f64;
        let bytes = reader.read(
            "adapter/adapter_model.safetensors",
            &dir.join("adapter_model.safetensors"),
        )?;
        let loaded = candle_core::safetensors::load_buffer(&bytes, device)
            .map_err(|e| Error::Artifact(format!("load adapter_model.safetensors: {e}")))?;
        drop(bytes);
        let tensors = loaded
            .into_iter()
            .map(|(k, v)| {
                let name = k
                    .strip_prefix("base_model.model.model.")
                    .or_else(|| k.strip_prefix("base_model.model."))
                    .unwrap_or(&k)
                    .to_string();
                Ok((name, v.to_dtype(DType::F32)?))
            })
            .collect::<candle_core::Result<_>>()?;
        Ok(Self {
            config,
            tensors,
            scale,
        })
    }

    /// Validate every requested projection before loading base weights.
    pub(crate) fn pairs(&self, layers: usize) -> Result<HashMap<String, (&Tensor, &Tensor)>> {
        let mut pairs = HashMap::new();
        for target in &self.config.target_modules {
            let group = match target.as_str() {
                "q_proj" | "k_proj" | "v_proj" | "o_proj" => "self_attn",
                "gate_proj" | "up_proj" | "down_proj" => "mlp",
                other => {
                    return Err(Error::Artifact(format!(
                        "adapter targets unknown module {other}"
                    )));
                }
            };
            for layer in 0..layers {
                let name = format!("layers.{layer}.{group}.{target}");
                let get = |factor: &str| {
                    self.tensors
                        .get(&format!("{name}.lora_{factor}.weight"))
                        .ok_or_else(|| Error::Artifact(format!("missing lora_{factor} for {name}")))
                };
                pairs.insert(format!("{name}.weight"), (get("A")?, get("B")?));
            }
        }
        Ok(pairs)
    }
}

/// Add in fp32 using the original base tensor, then cast the combined weight.
pub(crate) fn merged_weight(
    base: &Tensor,
    a: &Tensor,
    b: &Tensor,
    scale: f64,
    dtype: DType,
) -> candle_core::Result<Tensor> {
    let delta = b
        .to_dtype(DType::F32)?
        .matmul(&a.to_dtype(DType::F32)?)?
        .affine(scale, 0.0)?;
    (base.to_dtype(DType::F32)? + delta)?.to_dtype(dtype)
}

/// Merge into an already loaded projection. This cannot recover precision
/// the caller discarded before this call; assembled models merge during load.
pub fn merge(linear: &mut Linear, a: &Tensor, b: &Tensor, scale: f64) -> candle_core::Result<()> {
    linear.weight = merged_weight(&linear.weight, a, b, scale, linear.weight.dtype())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_base_and_delta_are_added_before_rounding() {
        let base = Tensor::new(&[[1.003f32]], &Device::Cpu).unwrap();
        let a = Tensor::new(&[[0.003f32]], &Device::Cpu).unwrap();
        let b = Tensor::new(&[[1.0f32]], &Device::Cpu).unwrap();
        let read = |t: Tensor| t.to_dtype(DType::F32).unwrap().to_vec2::<f32>().unwrap()[0][0];
        let merged = merged_weight(&base, &a, &b, 1.0, DType::BF16).unwrap();
        let expected = (&base + b.matmul(&a).unwrap())
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        let premature =
            (base.to_dtype(DType::BF16).unwrap() + a.to_dtype(DType::BF16).unwrap()).unwrap();
        assert_eq!(read(merged), read(expected));
        assert_ne!(read(premature), 1.0078125);
        assert_eq!(
            read(merged_weight(&base, &a, &b, 1.0, DType::BF16).unwrap()),
            1.0078125
        );
    }
}
