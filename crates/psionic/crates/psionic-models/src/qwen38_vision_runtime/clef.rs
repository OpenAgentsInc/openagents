//! Clef vision weights loaded from an explicitly selected Ollama manifest.
//!
//! This reuses the vision graph without changing the pinned Qwen3.8 loader.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::*;
use crate::{qwen35_text_observed_tensors_from_shards, qwen38_vision_expected_tensor_specs};

/// A complete Clef or Clef-Flash vision tower, running explicitly on the CPU.
pub struct ClefNativeVisionRuntime {
    model: VisionModel,
    digest: String,
}

#[derive(Deserialize)]
struct Manifest {
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
struct Layer {
    #[serde(default)]
    name: String,
    digest: String,
    size: u64,
}

impl ClefNativeVisionRuntime {
    /// Loads all 333 vision tensors after checking their content hashes,
    /// shapes, dtypes, processor configuration, and decoder output width.
    /// Blob files remain in place; the loader never downloads or copies them.
    pub fn from_ollama_manifest(
        manifest: &Path,
        blobs: &Path,
        hidden_size: usize,
    ) -> Result<Self, String> {
        let manifest_bytes = read_bounded(manifest, 4 * 1024 * 1024)?;
        let manifest: Manifest =
            serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
        let mut named = BTreeMap::new();
        for layer in &manifest.layers {
            if !layer.name.is_empty() && named.insert(layer.name.as_str(), layer).is_some() {
                return Err(format!("duplicate vision source entry: {}", layer.name));
            }
        }
        let config_layer = named
            .get("config.json")
            .ok_or("vision source has no config.json")?;
        let processor_layer = named
            .get("processor_config.json")
            .ok_or("vision source has no processor_config.json")?;
        let config_path = verify_blob(blobs, config_layer)?;
        let processor_path = verify_blob(blobs, processor_layer)?;
        let config: serde_json::Value =
            serde_json::from_slice(&read_bounded(&config_path, 1024 * 1024)?)
                .map_err(|error| error.to_string())?;
        let processor: serde_json::Value =
            serde_json::from_slice(&read_bounded(&processor_path, 1024 * 1024)?)
                .map_err(|error| error.to_string())?;
        let config = validate_config(&config, &processor, hidden_size)?;
        let mut specs = qwen38_vision_expected_tensor_specs();
        for spec in &mut specs {
            if matches!(
                spec.name.as_str(),
                "model.visual.merger.linear_fc2.weight" | "model.visual.merger.linear_fc2.bias"
            ) {
                spec.shape[0] = hidden_size;
            }
        }
        let expected: BTreeSet<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
        let observed: BTreeSet<&str> = named
            .keys()
            .copied()
            .filter(|name| name.starts_with("model.visual."))
            .collect();
        if expected != observed {
            return Err(String::from(
                "vision source must contain exactly the 333 supported vision tensors",
            ));
        }
        let mut digest = Sha256::new();
        digest.update(b"clef-vision-v1\0");
        digest.update(config_layer.digest.as_bytes());
        digest.update(processor_layer.digest.as_bytes());
        let paths: Vec<PathBuf> = specs
            .iter()
            .map(|spec| {
                let layer = named[spec.name.as_str()];
                digest.update(spec.name.as_bytes());
                digest.update(layer.digest.as_bytes());
                verify_blob(blobs, layer)
            })
            .collect::<Result<_, _>>()?;
        let headers =
            qwen35_text_observed_tensors_from_shards(&paths).map_err(|error| error.to_string())?;
        if headers.tensors.len() != specs.len() {
            return Err(String::from(
                "vision blobs contain an unexpected number of tensors",
            ));
        }
        for (actual, expected) in headers.tensors.iter().zip(&specs) {
            if actual.name != expected.name
                || actual.shape != expected.shape
                || actual.dtype != "BF16"
            {
                return Err(format!(
                    "vision tensor {} has an unsupported name, shape, or dtype",
                    actual.name
                ));
            }
        }
        // SAFETY: these are verified local safetensors files. As with the other
        // mmap loaders, their owners must not modify them while the model lives.
        let weights =
            unsafe { VarBuilder::from_mmaped_safetensors(&paths, DType::F32, &Device::Cpu) }
                .map_err(|error| error.to_string())?;
        let model = VisionModel::new(&config, weights.pp("model").pp("visual"))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            model,
            digest: format!("sha256:{}", hex::encode(digest.finalize())),
        })
    }

    /// Identity of the validated configuration and all vision tensor contents.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Projects one bounded, already-preprocessed image into decoder rows.
    pub fn encode(&self, input: &Qwen38VisionPreprocessedInput) -> Result<Vec<Vec<f32>>, String> {
        validate_qwen38_vision_preprocessed_input(input).map_err(|error| error.to_string())?;
        if input.receipt.media_kind != crate::Qwen38VisionMediaKind::Image {
            return Err(String::from("Clef vision accepts images only"));
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(input.receipt.limits.timeout_ms))
            .ok_or("vision timeout overflow")?;
        let values = Tensor::from_vec(
            input.pixel_values.clone(),
            (
                input.receipt.patch_count,
                self.model.config.patch_vector_size(),
            ),
            &Device::Cpu,
        )
        .map_err(|error| error.to_string())?;
        let output = self
            .model
            .forward(&values, input.receipt.grid_thw, deadline)
            .map_err(|error| error.to_string())?;
        if Instant::now() >= deadline {
            return Err(String::from("Clef image processing timed out"));
        }
        let rows = output
            .to_dtype(DType::F32)
            .and_then(|output| output.to_vec2::<f32>())
            .map_err(|error| error.to_string())?;
        if rows.len() != input.receipt.merged_token_count
            || rows.iter().any(|row| {
                row.len() != self.model.config.out_hidden_size || row.iter().any(|x| !x.is_finite())
            })
        {
            return Err(String::from("Clef image encoder returned invalid rows"));
        }
        Ok(rows)
    }
}

fn validate_config(
    config: &serde_json::Value,
    processor: &serde_json::Value,
    width: usize,
) -> Result<VisionConfig, String> {
    if !matches!(width, 4096 | 5120) {
        return Err(String::from(
            "Clef vision requires a 4096- or 5120-wide decoder",
        ));
    }
    let vision = &config["vision_config"];
    for (name, expected) in [
        ("depth", 27),
        ("hidden_size", 1152),
        ("intermediate_size", 4304),
        ("num_heads", 16),
        ("in_channels", 3),
        ("patch_size", 16),
        ("spatial_merge_size", 2),
        ("temporal_patch_size", 2),
        ("num_position_embeddings", 2304),
        ("out_hidden_size", width),
    ] {
        if vision[name].as_u64() != Some(expected as u64) {
            return Err(format!("unsupported Clef vision configuration: {name}"));
        }
    }
    if vision["hidden_act"].as_str() != Some("gelu_pytorch_tanh")
        || !vision["deepstack_visual_indexes"]
            .as_array()
            .is_some_and(Vec::is_empty)
        || config["text_config"]["hidden_size"].as_u64() != Some(width as u64)
        || config["image_token_id"].as_u64() != Some(248056)
        || config["vision_start_token_id"].as_u64() != Some(248053)
        || config["vision_end_token_id"].as_u64() != Some(248054)
    {
        return Err(String::from(
            "Clef vision configuration does not match the decoder",
        ));
    }
    let image = &processor["image_processor"];
    for (name, expected) in [
        ("patch_size", 16),
        ("merge_size", 2),
        ("temporal_patch_size", 2),
    ] {
        if image[name].as_u64() != Some(expected) {
            return Err(format!("unsupported Clef image processor: {name}"));
        }
    }
    if ["do_convert_rgb", "do_normalize", "do_rescale", "do_resize"]
        .iter()
        .any(|name| image[name].as_bool() != Some(true))
        || image["image_mean"] != serde_json::json!([0.5, 0.5, 0.5])
        || image["image_std"] != serde_json::json!([0.5, 0.5, 0.5])
        || image["rescale_factor"].as_f64() != Some(1.0 / 255.0)
        || image["size"]["shortest_edge"].as_u64() != Some(65536)
        || image["size"]["longest_edge"].as_u64() != Some(16_777_216)
        || image["resample"].as_u64() != Some(3)
        || image["image_processor_type"].as_str() != Some("Qwen2VLImageProcessor")
        || processor["processor_class"].as_str() != Some("Qwen3VLProcessor")
    {
        return Err(String::from(
            "unsupported Clef image normalization or pixel bounds",
        ));
    }
    let mut result = VisionConfig::official();
    result.out_hidden_size = width;
    Ok(result)
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > maximum {
        return Err(String::from("vision metadata exceeds its size limit"));
    }
    Ok(bytes)
}

fn verify_blob(blobs: &Path, layer: &Layer) -> Result<PathBuf, String> {
    let digest = layer
        .digest
        .strip_prefix("sha256:")
        .filter(|value| value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or("invalid vision blob digest")?;
    let path = blobs.join(format!("sha256-{digest}"));
    let mut file = File::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if file.metadata().map_err(|error| error.to_string())?.len() != layer.size {
        return Err(format!("vision blob size mismatch: {}", layer.name));
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if hex::encode(hash.finalize()) != digest {
        return Err(format!("vision blob hash mismatch: {}", layer.name));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(width: usize) -> (serde_json::Value, serde_json::Value) {
        (
            serde_json::json!({
                "text_config": {"hidden_size": width}, "image_token_id": 248056,
                "vision_start_token_id": 248053, "vision_end_token_id": 248054,
                "vision_config": {"depth": 27, "hidden_size": 1152, "intermediate_size": 4304,
                    "num_heads": 16, "in_channels": 3, "patch_size": 16, "spatial_merge_size": 2,
                    "temporal_patch_size": 2, "num_position_embeddings": 2304, "out_hidden_size": width,
                    "hidden_act": "gelu_pytorch_tanh", "deepstack_visual_indexes": []}
            }),
            serde_json::json!({"processor_class": "Qwen3VLProcessor", "image_processor": {
                "image_processor_type": "Qwen2VLImageProcessor", "patch_size": 16, "merge_size": 2,
                "temporal_patch_size": 2, "do_convert_rgb": true, "do_normalize": true,
                "do_rescale": true, "do_resize": true, "image_mean": [0.5, 0.5, 0.5],
                "image_std": [0.5, 0.5, 0.5], "rescale_factor": 1.0 / 255.0, "resample": 3,
                "size": {"shortest_edge": 65536, "longest_edge": 16777216}
            }}),
        )
    }

    #[test]
    fn clef_vision_requires_matching_decoder_and_exact_processor() {
        for width in [4096, 5120] {
            let (config, processor) = configuration(width);
            assert_eq!(
                validate_config(&config, &processor, width)
                    .unwrap()
                    .out_hidden_size,
                width
            );
            assert!(
                validate_config(&config, &processor, if width == 4096 { 5120 } else { 4096 })
                    .is_err()
            );
            let mut changed = processor.clone();
            changed["image_processor"]["resample"] = serde_json::json!(2);
            assert!(validate_config(&config, &changed, width).is_err());
            let mut changed = processor.clone();
            changed["image_processor"]["size"]["longest_edge"] = serde_json::json!(262144);
            assert!(validate_config(&config, &changed, width).is_err());
            let mut changed = config.clone();
            changed["vision_config"]["deepstack_visual_indexes"] = serde_json::json!([8, 16, 24]);
            assert!(validate_config(&changed, &processor, width).is_err());
        }
    }

    #[test]
    fn vision_blob_names_cannot_escape_the_selected_directory() {
        for digest in [
            "../escape",
            "sha256:../../escape",
            "sha256:",
            "sha256:012345",
        ] {
            let layer = Layer {
                name: String::from("test"),
                digest: digest.into(),
                size: 0,
            };
            assert_eq!(
                verify_blob(Path::new("does-not-exist"), &layer).unwrap_err(),
                "invalid vision blob digest"
            );
        }
    }

    #[test]
    fn vision_blob_contents_must_match_the_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let content = b"public synthetic vision fixture";
        let digest = hex::encode(Sha256::digest(content));
        std::fs::write(directory.path().join(format!("sha256-{digest}")), content).unwrap();
        let mut layer = Layer {
            name: String::from("test"),
            digest: format!("sha256:{digest}"),
            size: content.len() as u64,
        };
        assert!(verify_blob(directory.path(), &layer).is_ok());
        layer.size += 1;
        assert!(
            verify_blob(directory.path(), &layer)
                .unwrap_err()
                .contains("size mismatch")
        );
        layer.size -= 1;
        std::fs::write(
            directory.path().join(format!("sha256-{digest}")),
            vec![0; content.len()],
        )
        .unwrap();
        assert!(
            verify_blob(directory.path(), &layer)
                .unwrap_err()
                .contains("hash mismatch")
        );
    }
}
