//! Adapter and LoRA packaging plus hosted binding types for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use psionic_core::QuantizationMode;
use psionic_datastream::{DatastreamManifest, DatastreamManifestRef, DatastreamSubjectKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

    use super::{
        AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterPackageError,
        AdapterPackageManifest, AdapterPackageTensor, AdapterResidencyMode, AdapterServingBinding,
        AdapterTargetFamily,
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
}
