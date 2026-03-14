//! Adapter and LoRA packaging plus hosted binding types for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use psionic_core::QuantizationMode;
use psionic_datastream::{DatastreamManifest, DatastreamManifestRef, DatastreamSubjectKind};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "adapter and lora packaging plus hosted binding types";

/// High-level adapter artifact kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterArtifactKind {
    /// Low-rank adaptation weights.
    Lora,
    /// Generic residual adapter weights.
    ResidualAdapter,
}

/// Concrete packaging format for adapter artifacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterArtifactFormat {
    /// One or more safetensors files.
    Safetensors,
    /// Tar archive packaging multiple adapter blobs.
    TarArchive,
}

/// High-level target family for the adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterTargetFamily {
    /// Decoder attention projections.
    DecoderAttention,
    /// Decoder feed-forward blocks.
    DecoderFeedForward,
    /// Decoder-wide adapter touching multiple subsystems.
    DecoderComposite,
}

/// Residency mode used when serving adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterResidencyMode {
    /// Adapter weights stay as a hot-swappable overlay.
    HotSwapOverlay,
    /// Adapter weights were merged into a resident derived artifact.
    MergedResident,
}

/// Stable identity for one adapter or LoRA artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterArtifactIdentity {
    /// Stable adapter identifier.
    pub adapter_id: String,
    /// Stable adapter revision.
    pub adapter_revision: String,
    /// High-level adapter kind.
    pub kind: AdapterArtifactKind,
    /// Packaging format.
    pub format: AdapterArtifactFormat,
    /// Base model identifier the adapter targets.
    pub base_model_id: String,
    /// Base model revision the adapter targets.
    pub base_model_revision: String,
    /// Stable digest for the base served artifact.
    pub base_served_artifact_digest: String,
    /// Stable digest of the adapter bytes.
    pub artifact_digest: String,
    /// Dominant quantization mode observed across adapter tensors.
    pub quantization: QuantizationMode,
    /// High-level target family.
    pub target_family: AdapterTargetFamily,
    /// Declared parameter count for the adapter.
    pub parameter_count: u64,
    /// Stable provenance digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    /// Stable governance digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance_digest: Option<String>,
}

impl AdapterArtifactIdentity {
    /// Creates a stable adapter identity.
    #[must_use]
    pub fn new(
        adapter_id: impl Into<String>,
        adapter_revision: impl Into<String>,
        kind: AdapterArtifactKind,
        format: AdapterArtifactFormat,
        base_model_id: impl Into<String>,
        base_model_revision: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        artifact_digest: impl Into<String>,
        quantization: QuantizationMode,
        target_family: AdapterTargetFamily,
        parameter_count: u64,
    ) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            adapter_revision: adapter_revision.into(),
            kind,
            format,
            base_model_id: base_model_id.into(),
            base_model_revision: base_model_revision.into(),
            base_served_artifact_digest: base_served_artifact_digest.into(),
            artifact_digest: artifact_digest.into(),
            quantization,
            target_family,
            parameter_count,
            provenance_digest: None,
            governance_digest: None,
        }
    }

    /// Attaches a provenance digest.
    #[must_use]
    pub fn with_provenance_digest(mut self, provenance_digest: impl Into<String>) -> Self {
        self.provenance_digest = Some(provenance_digest.into());
        self
    }

    /// Attaches a governance digest.
    #[must_use]
    pub fn with_governance_digest(mut self, governance_digest: impl Into<String>) -> Self {
        self.governance_digest = Some(governance_digest.into());
        self
    }

    /// Returns a stable digest for the adapter identity.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_adapter_identity|");
        hasher.update(self.adapter_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.adapter_revision.as_bytes());
        hasher.update(b"|");
        hasher.update(adapter_kind_label(self.kind));
        hasher.update(b"|");
        hasher.update(adapter_format_label(self.format));
        hasher.update(b"|");
        hasher.update(self.base_model_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.base_model_revision.as_bytes());
        hasher.update(b"|");
        hasher.update(self.base_served_artifact_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(self.artifact_digest.as_bytes());
        hasher.update(b"|");
        hasher.update(quantization_label(self.quantization));
        hasher.update(b"|");
        hasher.update(adapter_target_label(self.target_family));
        hasher.update(b"|");
        hasher.update(self.parameter_count.to_string().as_bytes());
        if let Some(provenance_digest) = &self.provenance_digest {
            hasher.update(b"|provenance|");
            hasher.update(provenance_digest.as_bytes());
        }
        if let Some(governance_digest) = &self.governance_digest {
            hasher.update(b"|governance|");
            hasher.update(governance_digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Optional tensor-level declaration inside an adapter package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPackageTensor {
    /// Stable tensor name.
    pub name: String,
    /// Byte length of the tensor payload.
    pub byte_length: u64,
    /// Stable digest of the tensor payload.
    pub sha256: String,
}

impl AdapterPackageTensor {
    /// Creates one tensor declaration.
    #[must_use]
    pub fn new(name: impl Into<String>, byte_length: u64, sha256: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            byte_length,
            sha256: sha256.into(),
        }
    }
}

/// Packaged adapter or LoRA bundle tied to one adapter-package datastream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPackageManifest {
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// Stable digest covering adapter identity and package stream.
    pub package_digest: String,
    /// Stable adapter identity.
    pub adapter: AdapterArtifactIdentity,
    /// Datastream reference for the package bytes.
    pub datastream: DatastreamManifestRef,
    /// Declared logical tensor count.
    pub tensor_count: usize,
    /// Optional tensor declarations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tensors: Vec<AdapterPackageTensor>,
}

impl AdapterPackageManifest {
    /// Builds an adapter package manifest from one adapter-package datastream.
    pub fn from_datastream(
        manifest_id: impl Into<String>,
        adapter: AdapterArtifactIdentity,
        datastream: &DatastreamManifest,
        tensor_count: usize,
    ) -> Result<Self, AdapterPackageError> {
        let manifest_id = manifest_id.into();
        if datastream.subject != DatastreamSubjectKind::AdapterPackage {
            return Err(AdapterPackageError::UnexpectedDatastreamSubject {
                stream_id: datastream.stream_id.clone(),
                actual: datastream.subject,
            });
        }
        let datastream = datastream.manifest_ref();
        let package_digest = stable_package_digest(
            manifest_id.clone(),
            adapter.stable_digest(),
            &datastream,
            tensor_count,
            &[],
        );
        Ok(Self {
            manifest_id,
            package_digest,
            adapter,
            datastream,
            tensor_count,
            tensors: Vec::new(),
        })
    }

    /// Appends one tensor declaration and refreshes the package digest.
    #[must_use]
    pub fn with_tensor(mut self, tensor: AdapterPackageTensor) -> Self {
        self.tensors.push(tensor);
        self.package_digest = stable_package_digest(
            self.manifest_id.clone(),
            self.adapter.stable_digest(),
            &self.datastream,
            self.tensor_count,
            &self.tensors,
        );
        self
    }
}

/// Hosted adapter-serving binding over one base served artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterServingBinding {
    /// Stable binding identifier.
    pub binding_id: String,
    /// Base model identifier.
    pub base_model_id: String,
    /// Base model revision.
    pub base_model_revision: String,
    /// Stable base served-artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable digest over the bound adapter set and residency mode.
    pub served_adapter_digest: String,
    /// Residency mode used by the hosted product.
    pub residency_mode: AdapterResidencyMode,
    /// Ordered adapter identities participating in the hosted product.
    pub adapters: Vec<AdapterArtifactIdentity>,
}

impl AdapterServingBinding {
    /// Creates a hosted adapter-serving binding and computes its stable digest.
    #[must_use]
    pub fn new(
        binding_id: impl Into<String>,
        base_model_id: impl Into<String>,
        base_model_revision: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        residency_mode: AdapterResidencyMode,
        mut adapters: Vec<AdapterArtifactIdentity>,
    ) -> Self {
        adapters.sort_by(|left, right| left.stable_digest().cmp(&right.stable_digest()));
        let binding_id = binding_id.into();
        let base_model_id = base_model_id.into();
        let base_model_revision = base_model_revision.into();
        let base_served_artifact_digest = base_served_artifact_digest.into();
        let served_adapter_digest = stable_binding_digest(
            binding_id.as_str(),
            base_model_id.as_str(),
            base_model_revision.as_str(),
            base_served_artifact_digest.as_str(),
            residency_mode,
            &adapters,
        );
        Self {
            binding_id,
            base_model_id,
            base_model_revision,
            base_served_artifact_digest,
            served_adapter_digest,
            residency_mode,
            adapters,
        }
    }
}

/// Loaded LM-head LoRA adapter artifact for hosted text-generation serving.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LmHeadLoraAdapterArtifact {
    /// Stable adapter identity.
    pub identity: AdapterArtifactIdentity,
    /// LoRA rank.
    pub rank: usize,
    /// Scaling factor applied as `alpha / rank`.
    pub alpha: f32,
    /// Hidden width the adapter targets.
    pub hidden_size: usize,
    /// Vocabulary width the adapter targets.
    pub vocab_size: usize,
    lora_a: Vec<f32>,
    lora_b: Vec<f32>,
}

impl LmHeadLoraAdapterArtifact {
    /// Loads an LM-head LoRA adapter from a local safetensors artifact.
    pub fn from_safetensors_path(
        path: impl AsRef<Path>,
        identity: AdapterArtifactIdentity,
        alpha: f32,
    ) -> Result<Self, LmHeadLoraLoadError> {
        let bytes = std::fs::read(path.as_ref()).map_err(|error| LmHeadLoraLoadError::Read {
            path: path.as_ref().display().to_string(),
            message: error.to_string(),
        })?;
        Self::from_safetensors_bytes(&bytes, identity, alpha)
    }

    /// Loads an LM-head LoRA adapter from raw safetensors bytes.
    pub fn from_safetensors_bytes(
        bytes: &[u8],
        identity: AdapterArtifactIdentity,
        alpha: f32,
    ) -> Result<Self, LmHeadLoraLoadError> {
        if identity.kind != AdapterArtifactKind::Lora {
            return Err(LmHeadLoraLoadError::UnsupportedIdentity(format!(
                "expected adapter kind `lora`, found `{:?}`",
                identity.kind
            )));
        }
        if identity.format != AdapterArtifactFormat::Safetensors {
            return Err(LmHeadLoraLoadError::UnsupportedIdentity(format!(
                "expected adapter format `safetensors`, found `{:?}`",
                identity.format
            )));
        }
        let tensors = SafeTensors::deserialize(bytes)
            .map_err(|error| LmHeadLoraLoadError::Format(error.to_string()))?;
        let lora_a =
            find_lora_tensor(&tensors, &["lm_head.lora_A.weight", "output.lora_A.weight"])?;
        let lora_b =
            find_lora_tensor(&tensors, &["lm_head.lora_B.weight", "output.lora_B.weight"])?;
        let [rank, hidden_size] = lora_a.shape.as_slice() else {
            return Err(LmHeadLoraLoadError::InvalidShape {
                tensor: lora_a.name,
                expected: vec![0, 0],
                actual: lora_a.shape.clone(),
            });
        };
        let [vocab_size, lora_b_rank] = lora_b.shape.as_slice() else {
            return Err(LmHeadLoraLoadError::InvalidShape {
                tensor: lora_b.name,
                expected: vec![0, 0],
                actual: lora_b.shape.clone(),
            });
        };
        if rank != lora_b_rank {
            return Err(LmHeadLoraLoadError::RankMismatch {
                lora_a_rank: *rank,
                lora_b_rank: *lora_b_rank,
            });
        }
        Ok(Self {
            identity,
            rank: *rank,
            alpha,
            hidden_size: *hidden_size,
            vocab_size: *vocab_size,
            lora_a: decode_f32_tensor(&lora_a)?,
            lora_b: decode_f32_tensor(&lora_b)?,
        })
    }

    /// Returns the adapter scaling factor applied to the low-rank update.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.alpha / (self.rank.max(1) as f32)
    }

    /// Applies the adapter directly to a logits vector for one final hidden state.
    pub fn apply_to_logits(
        &self,
        hidden: &[f32],
        logits: &mut [f32],
    ) -> Result<(), LmHeadLoraRuntimeError> {
        if hidden.len() != self.hidden_size {
            return Err(LmHeadLoraRuntimeError::HiddenWidth {
                expected: self.hidden_size,
                actual: hidden.len(),
            });
        }
        if logits.len() != self.vocab_size {
            return Err(LmHeadLoraRuntimeError::LogitWidth {
                expected: self.vocab_size,
                actual: logits.len(),
            });
        }
        let mut intermediate = vec![0.0_f32; self.rank];
        for (rank_index, row) in self.lora_a.chunks_exact(self.hidden_size).enumerate() {
            intermediate[rank_index] = dot(row, hidden);
        }
        let scale = self.scale();
        for (vocab_index, row) in self.lora_b.chunks_exact(self.rank).enumerate() {
            logits[vocab_index] += dot(row, intermediate.as_slice()) * scale;
        }
        Ok(())
    }

    /// Materializes the merged dense output delta matrix as `vocab x hidden`.
    #[must_use]
    pub fn merged_output_delta(&self) -> Vec<f32> {
        let scale = self.scale();
        let mut merged = vec![0.0_f32; self.vocab_size.saturating_mul(self.hidden_size)];
        for vocab_index in 0..self.vocab_size {
            let b_row = &self.lora_b[vocab_index * self.rank..(vocab_index + 1) * self.rank];
            let merged_row =
                &mut merged[vocab_index * self.hidden_size..(vocab_index + 1) * self.hidden_size];
            for rank_index in 0..self.rank {
                let weight = b_row[rank_index] * scale;
                if weight == 0.0 {
                    continue;
                }
                let a_row = &self.lora_a
                    [rank_index * self.hidden_size..(rank_index + 1) * self.hidden_size];
                for (index, value) in a_row.iter().enumerate() {
                    merged_row[index] += value * weight;
                }
            }
        }
        merged
    }
}

/// Errors returned while packaging adapter manifests.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AdapterPackageError {
    /// The provided datastream subject was not adapter-package scoped.
    #[error("datastream `{stream_id}` is not an adapter package: found `{actual:?}`")]
    UnexpectedDatastreamSubject {
        /// Stable stream identifier.
        stream_id: String,
        /// Actual datastream subject.
        actual: DatastreamSubjectKind,
    },
}

/// Errors returned while loading an LM-head LoRA adapter artifact.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LmHeadLoraLoadError {
    /// The local safetensors artifact could not be read.
    #[error("failed to read LM-head LoRA adapter `{path}`: {message}")]
    Read {
        /// Local file path.
        path: String,
        /// Plain-text failure reason.
        message: String,
    },
    /// The safetensors payload was malformed.
    #[error("invalid LM-head LoRA safetensors artifact: {0}")]
    Format(String),
    /// The adapter identity does not match the supported import path.
    #[error("unsupported LM-head LoRA identity: {0}")]
    UnsupportedIdentity(String),
    /// One expected tensor was missing.
    #[error("missing LM-head LoRA tensor `{0}`")]
    MissingTensor(String),
    /// One tensor used the wrong dtype.
    #[error("LM-head LoRA tensor `{tensor}` must be `f32`, found `{actual}`")]
    UnsupportedDType {
        /// Tensor name.
        tensor: String,
        /// Actual dtype label.
        actual: String,
    },
    /// One tensor used an invalid shape.
    #[error(
        "LM-head LoRA tensor `{tensor}` has invalid shape: expected {expected:?}, actual {actual:?}"
    )]
    InvalidShape {
        /// Tensor name.
        tensor: String,
        /// Expected shape pattern.
        expected: Vec<usize>,
        /// Actual shape.
        actual: Vec<usize>,
    },
    /// The two LoRA matrices disagree on rank.
    #[error("LM-head LoRA rank mismatch: lora_A={lora_a_rank} lora_B={lora_b_rank}")]
    RankMismatch {
        /// Rank declared by `lora_A`.
        lora_a_rank: usize,
        /// Rank declared by `lora_B`.
        lora_b_rank: usize,
    },
}

/// Runtime failures returned while applying an LM-head LoRA artifact.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LmHeadLoraRuntimeError {
    /// The supplied hidden state used the wrong width.
    #[error("LM-head LoRA hidden width mismatch: expected {expected}, actual {actual}")]
    HiddenWidth {
        /// Expected hidden width.
        expected: usize,
        /// Actual hidden width.
        actual: usize,
    },
    /// The supplied logits vector used the wrong width.
    #[error("LM-head LoRA logits width mismatch: expected {expected}, actual {actual}")]
    LogitWidth {
        /// Expected logits width.
        expected: usize,
        /// Actual logits width.
        actual: usize,
    },
}

fn stable_package_digest(
    manifest_id: String,
    adapter_digest: String,
    datastream: &DatastreamManifestRef,
    tensor_count: usize,
    tensors: &[AdapterPackageTensor],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_adapter_package|");
    hasher.update(manifest_id.as_bytes());
    hasher.update(b"|");
    hasher.update(adapter_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(datastream.stream_id.as_bytes());
    hasher.update(b"|");
    hasher.update(datastream.manifest_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(datastream.object_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(tensor_count.to_string().as_bytes());
    for tensor in tensors {
        hasher.update(b"|tensor|");
        hasher.update(tensor.name.as_bytes());
        hasher.update(b"|");
        hasher.update(tensor.byte_length.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(tensor.sha256.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_binding_digest(
    binding_id: &str,
    base_model_id: &str,
    base_model_revision: &str,
    base_served_artifact_digest: &str,
    residency_mode: AdapterResidencyMode,
    adapters: &[AdapterArtifactIdentity],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_adapter_binding|");
    hasher.update(binding_id.as_bytes());
    hasher.update(b"|");
    hasher.update(base_model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(base_model_revision.as_bytes());
    hasher.update(b"|");
    hasher.update(base_served_artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(adapter_residency_label(residency_mode));
    for adapter in adapters {
        hasher.update(b"|adapter|");
        hasher.update(adapter.stable_digest().as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug)]
struct LoraTensorRef<'a> {
    name: String,
    shape: Vec<usize>,
    data: &'a [u8],
}

fn find_lora_tensor<'a>(
    tensors: &'a SafeTensors<'a>,
    names: &[&str],
) -> Result<LoraTensorRef<'a>, LmHeadLoraLoadError> {
    for name in names {
        if let Ok(tensor) = tensors.tensor(name) {
            if tensor.dtype() != SafeTensorsDType::F32 {
                return Err(LmHeadLoraLoadError::UnsupportedDType {
                    tensor: (*name).to_string(),
                    actual: format!("{:?}", tensor.dtype()),
                });
            }
            return Ok(LoraTensorRef {
                name: (*name).to_string(),
                shape: tensor.shape().to_vec(),
                data: tensor.data(),
            });
        }
    }
    Err(LmHeadLoraLoadError::MissingTensor(
        names.first().copied().unwrap_or("unknown").to_string(),
    ))
}

fn decode_f32_tensor(tensor: &LoraTensorRef<'_>) -> Result<Vec<f32>, LmHeadLoraLoadError> {
    let chunks = tensor.data.chunks_exact(std::mem::size_of::<f32>());
    if !chunks.remainder().is_empty() {
        return Err(LmHeadLoraLoadError::Format(format!(
            "tensor `{}` length {} is not aligned to `f32`",
            tensor.name,
            tensor.data.len()
        )));
    }
    Ok(chunks
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("f32 chunk")))
        .collect())
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .fold(0.0_f32, |accumulator, (lhs, rhs)| accumulator + (lhs * rhs))
}

fn adapter_kind_label(kind: AdapterArtifactKind) -> &'static [u8] {
    match kind {
        AdapterArtifactKind::Lora => b"lora",
        AdapterArtifactKind::ResidualAdapter => b"residual_adapter",
    }
}

fn adapter_format_label(format: AdapterArtifactFormat) -> &'static [u8] {
    match format {
        AdapterArtifactFormat::Safetensors => b"safetensors",
        AdapterArtifactFormat::TarArchive => b"tar_archive",
    }
}

fn adapter_target_label(target: AdapterTargetFamily) -> &'static [u8] {
    match target {
        AdapterTargetFamily::DecoderAttention => b"decoder_attention",
        AdapterTargetFamily::DecoderFeedForward => b"decoder_feed_forward",
        AdapterTargetFamily::DecoderComposite => b"decoder_composite",
    }
}

fn adapter_residency_label(mode: AdapterResidencyMode) -> &'static [u8] {
    match mode {
        AdapterResidencyMode::HotSwapOverlay => b"hot_swap_overlay",
        AdapterResidencyMode::MergedResident => b"merged_resident",
    }
}

fn quantization_label(mode: QuantizationMode) -> &'static [u8] {
    match mode {
        QuantizationMode::None => b"none",
        QuantizationMode::Int8Symmetric => b"int8_symmetric",
        QuantizationMode::GgmlMxfp4 => b"ggml_mxfp4",
        QuantizationMode::GgmlQ4_0 => b"ggml_q4_0",
        QuantizationMode::GgmlQ4_1 => b"ggml_q4_1",
        QuantizationMode::GgmlQ8_0 => b"ggml_q8_0",
    }
}

#[cfg(test)]
mod tests {
    use psionic_core::QuantizationMode;
    use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
    use safetensors::{Dtype as SafeTensorsDType, serialize_to_file, tensor::TensorView};
    use tempfile::tempdir;

    use super::{
        AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterPackageError,
        AdapterPackageManifest, AdapterPackageTensor, AdapterResidencyMode, AdapterServingBinding,
        AdapterTargetFamily, LmHeadLoraAdapterArtifact,
    };

    fn sample_adapter() -> AdapterArtifactIdentity {
        AdapterArtifactIdentity::new(
            "adapter-qna",
            "r1",
            AdapterArtifactKind::Lora,
            AdapterArtifactFormat::Safetensors,
            "gpt-oss-20b",
            "2026-03",
            "base-served-artifact",
            "adapter-digest",
            QuantizationMode::GgmlQ8_0,
            AdapterTargetFamily::DecoderAttention,
            1_024_000,
        )
        .with_provenance_digest("prov-digest")
        .with_governance_digest("gov-digest")
    }

    #[test]
    fn adapter_package_manifest_requires_adapter_package_stream()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest = DatastreamManifest::from_bytes(
            "adapter-stream",
            DatastreamSubjectKind::Checkpoint,
            b"adapter-bytes",
            4,
            DatastreamEncoding::TarArchive,
        );

        let result = AdapterPackageManifest::from_datastream(
            "adapter-manifest",
            sample_adapter(),
            &manifest,
            3,
        );

        assert!(matches!(
            result,
            Err(AdapterPackageError::UnexpectedDatastreamSubject { .. })
        ));
        Ok(())
    }

    #[test]
    fn adapter_serving_binding_has_stable_digest() -> Result<(), Box<dyn std::error::Error>> {
        let datastream = DatastreamManifest::from_bytes(
            "adapter-stream",
            DatastreamSubjectKind::AdapterPackage,
            b"adapter-bytes",
            4,
            DatastreamEncoding::TarArchive,
        );
        let manifest = AdapterPackageManifest::from_datastream(
            "adapter-manifest",
            sample_adapter(),
            &datastream,
            2,
        )?
        .with_tensor(AdapterPackageTensor::new(
            "layers.0.attn.lora_a",
            4_096,
            "tensor-a",
        ))
        .with_tensor(AdapterPackageTensor::new(
            "layers.0.attn.lora_b",
            4_096,
            "tensor-b",
        ));
        assert!(!manifest.package_digest.is_empty());

        let binding = AdapterServingBinding::new(
            "binding-qna",
            "gpt-oss-20b",
            "2026-03",
            "base-served-artifact",
            AdapterResidencyMode::HotSwapOverlay,
            vec![manifest.adapter.clone()],
        );
        let second = AdapterServingBinding::new(
            "binding-qna",
            "gpt-oss-20b",
            "2026-03",
            "base-served-artifact",
            AdapterResidencyMode::HotSwapOverlay,
            vec![manifest.adapter],
        );

        assert_eq!(binding.served_adapter_digest, second.served_adapter_digest);
        Ok(())
    }

    #[test]
    fn lm_head_lora_adapter_loads_and_applies_overlay() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("lm_head_adapter.safetensors");
        write_lora_safetensors(&path, &[1.0, 0.0, 0.0, 0.0], &[0.0, 0.0, 8.0])?;

        let adapter =
            LmHeadLoraAdapterArtifact::from_safetensors_path(&path, sample_adapter(), 1.0)?;
        let mut logits = vec![0.0_f32, 0.0, 0.0];
        adapter.apply_to_logits(&[1.0, 0.0, 0.0, 0.0], logits.as_mut_slice())?;

        assert_eq!(adapter.rank, 1);
        assert_eq!(adapter.hidden_size, 4);
        assert_eq!(adapter.vocab_size, 3);
        assert_eq!(logits, vec![0.0, 0.0, 8.0]);
        Ok(())
    }

    fn write_lora_safetensors(
        path: &std::path::Path,
        lora_a: &[f32],
        lora_b: &[f32],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lora_a_bytes = encode_f32_bytes(lora_a);
        let lora_b_bytes = encode_f32_bytes(lora_b);
        let mut tensors = std::collections::BTreeMap::new();
        tensors.insert(
            "lm_head.lora_A.weight".to_string(),
            TensorView::new(SafeTensorsDType::F32, vec![1, lora_a.len()], &lora_a_bytes)?,
        );
        tensors.insert(
            "lm_head.lora_B.weight".to_string(),
            TensorView::new(SafeTensorsDType::F32, vec![lora_b.len(), 1], &lora_b_bytes)?,
        );
        serialize_to_file(tensors, None, path)?;
        Ok(())
    }

    fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
}
