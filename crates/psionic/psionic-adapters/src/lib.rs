//! Adapter and LoRA packaging plus hosted binding types for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use psionic_core::QuantizationMode;
use psionic_datastream::{DatastreamManifest, DatastreamManifestRef, DatastreamSubjectKind};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
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
    /// Apple Foundation Models `.fmadapter` directory package.
    AppleFmPackage,
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

/// Generic file inventory entry inside an adapter package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterPackageFile {
    /// Stable package-relative file path.
    pub relative_path: String,
    /// Byte length of the file payload.
    pub byte_length: u64,
    /// Stable digest of the file payload.
    pub sha256: String,
}

impl AdapterPackageFile {
    /// Creates one file-inventory entry.
    #[must_use]
    pub fn new(
        relative_path: impl Into<String>,
        byte_length: u64,
        sha256: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
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
    /// Optional file inventory when the package format is file-based.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<AdapterPackageFile>,
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
            &[],
        );
        Ok(Self {
            manifest_id,
            package_digest,
            adapter,
            datastream,
            tensor_count,
            tensors: Vec::new(),
            files: Vec::new(),
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
            &self.files,
        );
        self
    }

    /// Appends one generic file-inventory entry and refreshes the package digest.
    #[must_use]
    pub fn with_file(mut self, file: AdapterPackageFile) -> Self {
        self.files.push(file);
        self.package_digest = stable_package_digest(
            self.manifest_id.clone(),
            self.adapter.stable_digest(),
            &self.datastream,
            self.tensor_count,
            &self.tensors,
            &self.files,
        );
        self
    }
}

/// Stable suffix for Apple Foundation Models adapter packages.
pub const APPLE_FM_ADAPTER_PACKAGE_EXTENSION: &str = "fmadapter";
/// Metadata file written inside one `.fmadapter` package.
pub const APPLE_FM_ADAPTER_METADATA_FILE: &str = "metadata.json";
/// Required adapter-weights payload inside one `.fmadapter` package.
pub const APPLE_FM_ADAPTER_WEIGHTS_FILE: &str = "adapter_weights.bin";
/// Optional draft-model graph payload inside one `.fmadapter` package.
pub const APPLE_FM_ADAPTER_DRAFT_MIL_FILE: &str = "draft.mil";
/// Optional draft-model weights payload inside one `.fmadapter` package.
pub const APPLE_FM_ADAPTER_DRAFT_WEIGHTS_FILE: &str = "draft_weights.bin";

/// Apple Foundation Models adapter metadata frozen from the reference exporter.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleFmAdapterPackageMetadata {
    /// Stable adapter identifier exposed to operators and compatibility checks.
    pub adapter_identifier: String,
    /// Optional author label.
    #[serde(default)]
    pub author: String,
    /// Stable base-model signature the package targets.
    pub base_model_signature: String,
    /// Developer-defined or OpenAgents-defined extension fields.
    #[serde(default)]
    pub creator_defined: BTreeMap<String, Value>,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// Optional license label.
    #[serde(default)]
    pub license: String,
    /// Low-rank adapter rank.
    pub lora_rank: u32,
    /// Optional speculative-decoding token count carried by the exporter.
    #[serde(default)]
    pub speculative_decoding_draft_token_count: u32,
}

impl AppleFmAdapterPackageMetadata {
    /// Validates the visible metadata contract.
    pub fn validate(&self) -> Result<(), AppleFmAdapterPackageError> {
        if !self.adapter_identifier.starts_with("fmadapter-") {
            return Err(AppleFmAdapterPackageError::InvalidAdapterIdentifier(
                self.adapter_identifier.clone(),
            ));
        }
        if self.base_model_signature.len() != 40
            || !self
                .base_model_signature
                .as_bytes()
                .iter()
                .all(u8::is_ascii_hexdigit)
            || self
                .base_model_signature
                .chars()
                .any(|character| character.is_ascii_uppercase())
        {
            return Err(AppleFmAdapterPackageError::InvalidBaseModelSignature(
                self.base_model_signature.clone(),
            ));
        }
        if self.lora_rank == 0 {
            return Err(AppleFmAdapterPackageError::InvalidLoraRank);
        }
        Ok(())
    }

    fn canonical_json_bytes(&self) -> Result<Vec<u8>, AppleFmAdapterPackageError> {
        serde_json::to_vec_pretty(self).map_err(|error| {
            AppleFmAdapterPackageError::SerializeMetadata {
                message: error.to_string(),
            }
        })
    }
}

/// Typed lineage fields OpenAgents extracts from `creatorDefined`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleFmAdapterLineage {
    /// Optional benchmark refs declared in the package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benchmark_refs: Vec<String>,
    /// Optional dataset ref recorded by the exporter or wrapper.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_ref: Option<String>,
    /// Optional draft-metadata digest recorded in metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_mil_digest: Option<String>,
    /// Whether metadata claims a draft-model payload is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_model_present: Option<bool>,
    /// Optional draft-weights digest recorded in metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_weights_digest: Option<String>,
    /// Optional package-format version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_format_version: Option<String>,
    /// Optional template digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_digest: Option<String>,
    /// Optional tokenizer digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_digest: Option<String>,
    /// Optional environment ref.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_environment_ref: Option<String>,
    /// Optional validator policy ref.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_policy_ref: Option<String>,
    /// Remaining creator-defined fields not normalized yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

impl AppleFmAdapterLineage {
    fn from_creator_defined(
        creator_defined: &BTreeMap<String, Value>,
    ) -> Result<Self, AppleFmAdapterPackageError> {
        let mut extra = creator_defined.clone();
        Ok(Self {
            benchmark_refs: take_string_vec_field(&mut extra, "benchmarkRefs")?,
            dataset_ref: take_string_field(&mut extra, "datasetRef")?,
            draft_mil_digest: take_string_field(&mut extra, "draftMilDigest")?,
            draft_model_present: take_bool_field(&mut extra, "draftModelPresent")?,
            draft_weights_digest: take_string_field(&mut extra, "draftWeightsDigest")?,
            package_format_version: take_string_field(&mut extra, "packageFormatVersion")?,
            template_digest: take_string_field(&mut extra, "templateDigest")?,
            tokenizer_digest: take_string_field(&mut extra, "tokenizerDigest")?,
            training_environment_ref: take_string_field(&mut extra, "trainingEnvironmentRef")?,
            validator_policy_ref: take_string_field(&mut extra, "validatorPolicyRef")?,
            extra,
        })
    }
}

/// Fully parsed Apple Foundation Models package with raw payload bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct AppleFmAdapterPackage {
    /// Stable package directory name including the `.fmadapter` suffix.
    pub package_name: String,
    /// Stable metadata object.
    pub metadata: AppleFmAdapterPackageMetadata,
    /// Typed lineage extracted from `creatorDefined`.
    pub lineage: AppleFmAdapterLineage,
    /// Stable package digest derived from inventory and file digests.
    pub package_digest: String,
    /// Ordered file inventory for the package.
    pub inventory: Vec<AdapterPackageFile>,
    metadata_bytes: Vec<u8>,
    adapter_weights_bytes: Vec<u8>,
    draft_mil_bytes: Option<Vec<u8>>,
    draft_weights_bytes: Option<Vec<u8>>,
}

impl AppleFmAdapterPackage {
    /// Builds one Apple package from explicit metadata and payload bytes.
    pub fn new(
        package_name: impl Into<String>,
        metadata: AppleFmAdapterPackageMetadata,
        adapter_weights_bytes: Vec<u8>,
        draft_mil_bytes: Option<Vec<u8>>,
        draft_weights_bytes: Option<Vec<u8>>,
    ) -> Result<Self, AppleFmAdapterPackageError> {
        let package_name = package_name.into();
        if !is_apple_fm_package_name(package_name.as_str()) {
            return Err(AppleFmAdapterPackageError::InvalidPackageRoot { path: package_name });
        }
        metadata.validate()?;
        validate_draft_payload_pair(
            &package_name,
            draft_mil_bytes.is_some(),
            draft_weights_bytes.is_some(),
        )?;
        let lineage = AppleFmAdapterLineage::from_creator_defined(&metadata.creator_defined)?;
        if let Some(draft_present) = lineage.draft_model_present {
            let payload_present = draft_mil_bytes.is_some() && draft_weights_bytes.is_some();
            if draft_present != payload_present {
                return Err(AppleFmAdapterPackageError::DraftPresenceMismatch {
                    path: package_name,
                    metadata_flag: draft_present,
                    payload_present,
                });
            }
        }
        let metadata_bytes = metadata.canonical_json_bytes()?;
        let mut inventory = vec![
            inventory_entry(APPLE_FM_ADAPTER_METADATA_FILE, metadata_bytes.as_slice()),
            inventory_entry(
                APPLE_FM_ADAPTER_WEIGHTS_FILE,
                adapter_weights_bytes.as_slice(),
            ),
        ];
        if let Some(bytes) = draft_mil_bytes.as_deref() {
            inventory.push(inventory_entry(APPLE_FM_ADAPTER_DRAFT_MIL_FILE, bytes));
        }
        if let Some(bytes) = draft_weights_bytes.as_deref() {
            inventory.push(inventory_entry(APPLE_FM_ADAPTER_DRAFT_WEIGHTS_FILE, bytes));
        }
        inventory.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let package_digest = stable_apple_fm_package_digest(package_name.as_str(), &inventory);
        Ok(Self {
            package_name,
            metadata,
            lineage,
            package_digest,
            inventory,
            metadata_bytes,
            adapter_weights_bytes,
            draft_mil_bytes,
            draft_weights_bytes,
        })
    }

    /// Reads and validates one Apple package from disk.
    pub fn read_from_directory(path: impl AsRef<Path>) -> Result<Self, AppleFmAdapterPackageError> {
        let path = path.as_ref();
        let package_name = path
            .file_name()
            .and_then(|component| component.to_str())
            .ok_or_else(|| AppleFmAdapterPackageError::InvalidPackageRoot {
                path: path.display().to_string(),
            })?;
        if !path.is_dir() || !is_apple_fm_package_name(package_name) {
            return Err(AppleFmAdapterPackageError::InvalidPackageRoot {
                path: path.display().to_string(),
            });
        }
        let metadata_bytes = read_required_package_file(path, APPLE_FM_ADAPTER_METADATA_FILE)?;
        let metadata: AppleFmAdapterPackageMetadata = serde_json::from_slice(&metadata_bytes)
            .map_err(|error| AppleFmAdapterPackageError::InvalidMetadataJson {
                path: path
                    .join(APPLE_FM_ADAPTER_METADATA_FILE)
                    .display()
                    .to_string(),
                message: error.to_string(),
            })?;
        let adapter_weights_bytes =
            read_required_package_file(path, APPLE_FM_ADAPTER_WEIGHTS_FILE)?;
        let draft_mil_path = path.join(APPLE_FM_ADAPTER_DRAFT_MIL_FILE);
        let draft_weights_path = path.join(APPLE_FM_ADAPTER_DRAFT_WEIGHTS_FILE);
        let draft_mil_bytes = read_optional_package_file(&draft_mil_path)?;
        let draft_weights_bytes = read_optional_package_file(&draft_weights_path)?;
        let mut package = Self::new(
            package_name.to_string(),
            metadata,
            adapter_weights_bytes,
            draft_mil_bytes,
            draft_weights_bytes,
        )?;
        package.metadata_bytes = metadata_bytes;
        package.inventory = build_apple_fm_inventory(
            package.metadata_bytes.as_slice(),
            package.adapter_weights_bytes.as_slice(),
            package.draft_mil_bytes.as_deref(),
            package.draft_weights_bytes.as_deref(),
        );
        package.package_digest =
            stable_apple_fm_package_digest(package.package_name.as_str(), &package.inventory);
        Ok(package)
    }

    /// Writes the package back to disk using the preserved payload bytes.
    pub fn write_to_directory(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), AppleFmAdapterPackageError> {
        let path = path.as_ref();
        let package_name = path
            .file_name()
            .and_then(|component| component.to_str())
            .ok_or_else(|| AppleFmAdapterPackageError::InvalidPackageRoot {
                path: path.display().to_string(),
            })?;
        if !is_apple_fm_package_name(package_name) {
            return Err(AppleFmAdapterPackageError::InvalidPackageRoot {
                path: path.display().to_string(),
            });
        }
        fs::create_dir_all(path).map_err(|error| AppleFmAdapterPackageError::CreateDirectory {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        write_package_file(
            path.join(APPLE_FM_ADAPTER_METADATA_FILE),
            self.metadata_bytes.as_slice(),
        )?;
        write_package_file(
            path.join(APPLE_FM_ADAPTER_WEIGHTS_FILE),
            self.adapter_weights_bytes.as_slice(),
        )?;
        if let Some(bytes) = &self.draft_mil_bytes {
            write_package_file(path.join(APPLE_FM_ADAPTER_DRAFT_MIL_FILE), bytes.as_slice())?;
        }
        if let Some(bytes) = &self.draft_weights_bytes {
            write_package_file(
                path.join(APPLE_FM_ADAPTER_DRAFT_WEIGHTS_FILE),
                bytes.as_slice(),
            )?;
        }
        Ok(())
    }

    /// Returns whether the package includes the optional draft payload.
    #[must_use]
    pub fn has_draft_payload(&self) -> bool {
        self.draft_mil_bytes.is_some() && self.draft_weights_bytes.is_some()
    }

    /// Returns the stable digest for one package-relative file when present.
    #[must_use]
    pub fn file_digest(&self, relative_path: &str) -> Option<&str> {
        self.inventory
            .iter()
            .find(|entry| entry.relative_path == relative_path)
            .map(|entry| entry.sha256.as_str())
    }

    /// Converts the Apple package into the generic adapter identity surface.
    #[must_use]
    pub fn to_adapter_identity(
        &self,
        base_model_id: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        parameter_count: u64,
    ) -> AdapterArtifactIdentity {
        AdapterArtifactIdentity::new(
            self.metadata.adapter_identifier.clone(),
            self.package_digest.clone(),
            AdapterArtifactKind::Lora,
            AdapterArtifactFormat::AppleFmPackage,
            base_model_id,
            self.metadata.base_model_signature.clone(),
            base_served_artifact_digest,
            self.package_digest.clone(),
            QuantizationMode::None,
            AdapterTargetFamily::DecoderComposite,
            parameter_count,
        )
    }

    /// Binds the Apple package to the existing datastream-backed manifest family.
    pub fn to_manifest(
        &self,
        manifest_id: impl Into<String>,
        datastream: &DatastreamManifest,
        base_model_id: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        parameter_count: u64,
    ) -> Result<AdapterPackageManifest, AdapterPackageError> {
        let mut manifest = AdapterPackageManifest::from_datastream(
            manifest_id,
            self.to_adapter_identity(base_model_id, base_served_artifact_digest, parameter_count),
            datastream,
            self.inventory.len(),
        )?;
        for file in &self.inventory {
            manifest = manifest.with_file(file.clone());
        }
        Ok(manifest)
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

/// Errors returned while reading or writing Apple Foundation Models packages.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleFmAdapterPackageError {
    /// The supplied path does not point at a `.fmadapter` directory.
    #[error("invalid Apple FM package root `{path}`")]
    InvalidPackageRoot {
        /// Visible path that failed validation.
        path: String,
    },
    /// One required file was missing.
    #[error("missing Apple FM package file `{relative_path}` under `{path}`")]
    MissingFile {
        /// Package root path.
        path: String,
        /// Missing package-relative file.
        relative_path: String,
    },
    /// One package file could not be read.
    #[error("failed to read Apple FM package file `{path}`: {message}")]
    ReadFile {
        /// Full local file path.
        path: String,
        /// Plain-text failure reason.
        message: String,
    },
    /// The metadata payload could not be parsed as JSON.
    #[error("invalid Apple FM metadata JSON `{path}`: {message}")]
    InvalidMetadataJson {
        /// Full local metadata path.
        path: String,
        /// Parser failure.
        message: String,
    },
    /// One recognized creator-defined field used the wrong type.
    #[error("invalid Apple FM creatorDefined field `{field}`: {message}")]
    InvalidCreatorDefinedField {
        /// Offending field name.
        field: String,
        /// Plain-text reason.
        message: String,
    },
    /// The base-model signature was malformed.
    #[error("invalid Apple FM base model signature `{0}`")]
    InvalidBaseModelSignature(String),
    /// The adapter identifier was malformed.
    #[error("invalid Apple FM adapter identifier `{0}`")]
    InvalidAdapterIdentifier(String),
    /// The LoRA rank was invalid.
    #[error("invalid Apple FM LoRA rank: must be positive")]
    InvalidLoraRank,
    /// The optional draft payload was present only partially.
    #[error(
        "Apple FM package `{path}` has incomplete draft payload: draft.mil={draft_mil_present} draft_weights.bin={draft_weights_present}"
    )]
    IncompleteDraftPayload {
        /// Package root path.
        path: String,
        /// Whether `draft.mil` was present.
        draft_mil_present: bool,
        /// Whether `draft_weights.bin` was present.
        draft_weights_present: bool,
    },
    /// Metadata claimed draft presence that did not match inventory.
    #[error(
        "Apple FM package `{path}` metadata draftModelPresent={metadata_flag} does not match payload presence={payload_present}"
    )]
    DraftPresenceMismatch {
        /// Package root path.
        path: String,
        /// `draftModelPresent` value declared in metadata.
        metadata_flag: bool,
        /// Whether both draft payload files were present.
        payload_present: bool,
    },
    /// Metadata serialization failed while writing a new package.
    #[error("failed to serialize Apple FM metadata: {message}")]
    SerializeMetadata {
        /// Plain-text failure reason.
        message: String,
    },
    /// Package directory creation failed while writing.
    #[error("failed to create Apple FM package directory `{path}`: {message}")]
    CreateDirectory {
        /// Directory path.
        path: String,
        /// Plain-text failure reason.
        message: String,
    },
    /// Package file write failed.
    #[error("failed to write Apple FM package file `{path}`: {message}")]
    WriteFile {
        /// Full file path.
        path: String,
        /// Plain-text failure reason.
        message: String,
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
    files: &[AdapterPackageFile],
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
    for file in files {
        hasher.update(b"|file|");
        hasher.update(file.relative_path.as_bytes());
        hasher.update(b"|");
        hasher.update(file.byte_length.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(file.sha256.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_apple_fm_package_digest(package_name: &str, inventory: &[AdapterPackageFile]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_apple_fm_package|");
    hasher.update(package_name.as_bytes());
    for file in inventory {
        hasher.update(b"|file|");
        hasher.update(file.relative_path.as_bytes());
        hasher.update(b"|");
        hasher.update(file.byte_length.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(file.sha256.as_bytes());
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
        AdapterArtifactFormat::AppleFmPackage => b"apple_fm_package",
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

fn inventory_entry(relative_path: &str, bytes: &[u8]) -> AdapterPackageFile {
    AdapterPackageFile::new(relative_path, bytes.len() as u64, sha256_hex(bytes))
}

fn build_apple_fm_inventory(
    metadata_bytes: &[u8],
    adapter_weights_bytes: &[u8],
    draft_mil_bytes: Option<&[u8]>,
    draft_weights_bytes: Option<&[u8]>,
) -> Vec<AdapterPackageFile> {
    let mut inventory = vec![
        inventory_entry(APPLE_FM_ADAPTER_METADATA_FILE, metadata_bytes),
        inventory_entry(APPLE_FM_ADAPTER_WEIGHTS_FILE, adapter_weights_bytes),
    ];
    if let Some(bytes) = draft_mil_bytes {
        inventory.push(inventory_entry(APPLE_FM_ADAPTER_DRAFT_MIL_FILE, bytes));
    }
    if let Some(bytes) = draft_weights_bytes {
        inventory.push(inventory_entry(APPLE_FM_ADAPTER_DRAFT_WEIGHTS_FILE, bytes));
    }
    inventory.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    inventory
}

fn is_apple_fm_package_name(package_name: &str) -> bool {
    Path::new(package_name)
        .extension()
        .and_then(|extension| extension.to_str())
        == Some(APPLE_FM_ADAPTER_PACKAGE_EXTENSION)
}

fn validate_draft_payload_pair(
    package_path: &str,
    draft_mil_present: bool,
    draft_weights_present: bool,
) -> Result<(), AppleFmAdapterPackageError> {
    if draft_mil_present == draft_weights_present {
        return Ok(());
    }
    Err(AppleFmAdapterPackageError::IncompleteDraftPayload {
        path: package_path.to_string(),
        draft_mil_present,
        draft_weights_present,
    })
}

fn read_required_package_file(
    package_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, AppleFmAdapterPackageError> {
    let path = package_root.join(relative_path);
    if !path.is_file() {
        return Err(AppleFmAdapterPackageError::MissingFile {
            path: package_root.display().to_string(),
            relative_path: relative_path.to_string(),
        });
    }
    read_package_file(&path)
}

fn read_optional_package_file(path: &Path) -> Result<Option<Vec<u8>>, AppleFmAdapterPackageError> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_package_file(path)?))
}

fn read_package_file(path: &Path) -> Result<Vec<u8>, AppleFmAdapterPackageError> {
    fs::read(path).map_err(|error| AppleFmAdapterPackageError::ReadFile {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_package_file(path: PathBuf, bytes: &[u8]) -> Result<(), AppleFmAdapterPackageError> {
    fs::write(&path, bytes).map_err(|error| AppleFmAdapterPackageError::WriteFile {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn take_string_field(
    map: &mut BTreeMap<String, Value>,
    field: &str,
) -> Result<Option<String>, AppleFmAdapterPackageError> {
    match map.remove(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(other) => Err(AppleFmAdapterPackageError::InvalidCreatorDefinedField {
            field: field.to_string(),
            message: format!("expected string, found {other}"),
        }),
    }
}

fn take_bool_field(
    map: &mut BTreeMap<String, Value>,
    field: &str,
) -> Result<Option<bool>, AppleFmAdapterPackageError> {
    match map.remove(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(other) => Err(AppleFmAdapterPackageError::InvalidCreatorDefinedField {
            field: field.to_string(),
            message: format!("expected bool, found {other}"),
        }),
    }
}

fn take_string_vec_field(
    map: &mut BTreeMap<String, Value>,
    field: &str,
) -> Result<Vec<String>, AppleFmAdapterPackageError> {
    match map.remove(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .into_iter()
            .map(|value| match value {
                Value::String(value) => Ok(value),
                other => Err(AppleFmAdapterPackageError::InvalidCreatorDefinedField {
                    field: field.to_string(),
                    message: format!("expected array of strings, found {other}"),
                }),
            })
            .collect(),
        Some(other) => Err(AppleFmAdapterPackageError::InvalidCreatorDefinedField {
            field: field.to_string(),
            message: format!("expected array of strings, found {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use psionic_core::QuantizationMode;
    use psionic_datastream::{DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind};
    use safetensors::{Dtype as SafeTensorsDType, serialize_to_file, tensor::TensorView};
    use serde::Deserialize;
    use tempfile::tempdir;

    use super::{
        AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterPackageError,
        AdapterPackageManifest, AdapterPackageTensor, AdapterResidencyMode, AdapterServingBinding,
        AdapterTargetFamily, AppleFmAdapterPackage, AppleFmAdapterPackageError,
        LmHeadLoraAdapterArtifact,
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PackageExpectation {
        adapter_identifier: String,
        base_model_signature: String,
        draft_files_present: bool,
        file_digests: BTreeMap<String, String>,
        package_path: String,
        required_files: Vec<String>,
        lora_rank: u32,
    }

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

    #[test]
    fn apple_fm_package_reads_positive_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let package = AppleFmAdapterPackage::read_from_directory(
            apple_fixture_root().join("packages/minimal_chat_adapter.fmadapter"),
        )?;
        let expected = read_expectation("packages/minimal_chat_adapter.expected.json")?;

        assert_eq!(package.package_name, expected.package_path);
        assert_eq!(
            package.metadata.adapter_identifier,
            expected.adapter_identifier
        );
        assert_eq!(
            package.metadata.base_model_signature,
            expected.base_model_signature
        );
        assert_eq!(package.metadata.lora_rank, expected.lora_rank);
        assert_eq!(package.has_draft_payload(), expected.draft_files_present);
        for relative_path in &expected.required_files {
            assert_eq!(
                package.file_digest(relative_path.as_str()),
                expected.file_digests.get(relative_path).map(String::as_str)
            );
        }
        assert_eq!(
            package.lineage.package_format_version.as_deref(),
            Some("openagents.apple-fmadapter.v1")
        );
        assert_eq!(
            package.lineage.training_environment_ref.as_deref(),
            Some("apple_adapter_sft@2026-03-14")
        );
        Ok(())
    }

    #[test]
    fn apple_fm_package_roundtrips_and_binds_to_manifest() -> Result<(), Box<dyn std::error::Error>>
    {
        let package = AppleFmAdapterPackage::read_from_directory(
            apple_fixture_root().join("packages/draft_chat_adapter.fmadapter"),
        )?;
        let temp = tempdir()?;
        let out = temp.path().join("draft_chat_adapter.fmadapter");
        package.write_to_directory(&out)?;
        let reread = AppleFmAdapterPackage::read_from_directory(&out)?;

        assert_eq!(package, reread);
        assert!(reread.has_draft_payload());

        let datastream = DatastreamManifest::from_bytes(
            "apple-adapter-stream",
            DatastreamSubjectKind::AdapterPackage,
            b"apple-adapter-package",
            4,
            DatastreamEncoding::TarArchive,
        );
        let manifest = reread.to_manifest(
            "apple-adapter-manifest",
            &datastream,
            "apple-foundation-model",
            "served-artifact-digest",
            0,
        )?;

        assert_eq!(
            manifest.adapter.format,
            AdapterArtifactFormat::AppleFmPackage
        );
        assert_eq!(manifest.files.len(), 4);
        assert_eq!(
            manifest.adapter.base_model_revision,
            "9799725ff8e851184037110b422d891ad3b92ec1"
        );
        Ok(())
    }

    #[test]
    fn apple_fm_package_rejects_bad_signature_fixture() {
        let result = AppleFmAdapterPackage::read_from_directory(
            apple_fixture_root().join("packages/invalid_bad_base_signature.fmadapter"),
        );

        assert!(matches!(
            result,
            Err(AppleFmAdapterPackageError::InvalidBaseModelSignature(_))
        ));
    }

    #[test]
    fn apple_fm_package_rejects_missing_metadata_fixture() {
        let result = AppleFmAdapterPackage::read_from_directory(
            apple_fixture_root().join("packages/invalid_missing_metadata.fmadapter"),
        );

        assert!(matches!(
            result,
            Err(AppleFmAdapterPackageError::MissingFile { .. })
        ));
    }

    #[test]
    fn apple_fm_package_rejects_incomplete_draft_fixture() {
        let result = AppleFmAdapterPackage::read_from_directory(
            apple_fixture_root().join("packages/invalid_draft_pairing.fmadapter"),
        );

        assert!(matches!(
            result,
            Err(AppleFmAdapterPackageError::IncompleteDraftPayload { .. })
        ));
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

    fn apple_fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/apple_adapter")
    }

    fn read_expectation(
        relative_path: &str,
    ) -> Result<PackageExpectation, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(apple_fixture_root().join(relative_path))?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
