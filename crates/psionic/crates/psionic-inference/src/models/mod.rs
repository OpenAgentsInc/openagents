//! Additional pinned model families over the same tensor execution interface.

use crate::{
    EngineKind, Error, ExecutionIdentity, ModelEngine, Result,
    artifacts::{Artifact, verify},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

mod mnist;
mod potion;
pub use mnist::{DigitPrediction, MnistEngine, MnistModel};
pub use potion::{PotionEngine, PotionModel, RankedText};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    Potion,
    Mnist,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceFile {
    pub url: String,
    #[serde(flatten)]
    pub artifact: Artifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NativeArtifact {
    pub file: String,
    pub converter: String,
    pub converter_sha256: String,
    #[serde(flatten)]
    pub artifact: Artifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelSpec {
    pub model: String,
    pub revision: String,
    pub license: String,
    pub onnx: String,
    pub files: BTreeMap<String, SourceFile>,
    pub native: Option<NativeArtifact>,
}

pub fn manifest(family: ModelFamily) -> Result<ModelSpec> {
    let text = match family {
        ModelFamily::Potion => include_str!("../../fixtures/potion-manifest.json"),
        ModelFamily::Mnist => include_str!("../../fixtures/mnist-manifest.json"),
    };
    Ok(serde_json::from_str(text)?)
}

impl ModelSpec {
    pub fn verify(&self, root: &Path, kind: EngineKind) -> Result<()> {
        let mut files: BTreeMap<String, Artifact> = self
            .files
            .iter()
            .map(|(name, f)| (name.clone(), f.artifact.clone()))
            .collect();
        if kind == EngineKind::Native {
            if let Some(native) = &self.native {
                files.insert(native.file.clone(), native.artifact.clone());
            }
        }
        if !files.contains_key(&self.onnx) {
            return Err(Error("The model description has no ONNX artifact".into()));
        }
        verify(root, &files)
    }
}

pub fn load_engine(
    family: ModelFamily,
    kind: EngineKind,
    root: &Path,
    threads: usize,
    library: Option<&Path>,
    checksum: Option<&str>,
) -> Result<Box<dyn ModelEngine>> {
    let spec = manifest(family)?;
    spec.verify(root, kind)?;
    match kind {
        EngineKind::Native => match family {
            ModelFamily::Potion => Ok(Box::new(PotionEngine::load_verified(root, threads)?)),
            ModelFamily::Mnist => Ok(Box::new(MnistEngine::load_verified(root, threads)?)),
        },
        EngineKind::Ort => {
            #[cfg(feature = "ort")]
            {
                let options = crate::ort_engine::OrtOptions {
                    library: library
                        .ok_or_else(|| Error("Choose an ONNX Runtime library".into()))?
                        .into(),
                    library_sha256: checksum
                        .ok_or_else(|| Error("Provide the ONNX Runtime library checksum".into()))?
                        .into(),
                    threads,
                };
                Ok(Box::new(crate::ort_engine::OrtEngine::load(
                    &root.join(spec.onnx),
                    &options,
                )?))
            }
            #[cfg(not(feature = "ort"))]
            {
                let _ = (library, checksum);
                Err(Error(
                    "ONNX Runtime support is not enabled in this build".into(),
                ))
            }
        }
    }
}

fn identity(provider: &str, threads: usize) -> ExecutionIdentity {
    ExecutionIdentity {
        engine: EngineKind::Native,
        provider: provider.into(),
        device: "cpu".into(),
        precision: "f32".into(),
        threads,
        runtime_version: env!("CARGO_PKG_VERSION").into(),
        runtime_sha256: None,
        cpu_fallback: Some(false),
    }
}

fn floats(tensors: &safetensors::SafeTensors<'_>, name: &str, shape: &[usize]) -> Result<Vec<f32>> {
    let tensor = tensors.tensor(name).map_err(|e| Error(e.to_string()))?;
    if tensor.dtype() != safetensors::Dtype::F32 || tensor.shape() != shape {
        return Err(Error(format!(
            "Unexpected weight type or dimensions: {name}"
        )));
    }
    let values: Vec<f32> = tensor
        .data()
        .chunks_exact(4)
        .map(|v| f32::from_le_bytes([v[0], v[1], v[2], v[3]]))
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err(Error(format!("Non-finite model weight: {name}")));
    }
    Ok(values)
}

fn pool(threads: usize) -> Result<rayon::ThreadPool> {
    if !(1..=256).contains(&threads) {
        return Err(Error("Choose between 1 and 256 CPU threads".into()));
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| Error(e.to_string()))
}
