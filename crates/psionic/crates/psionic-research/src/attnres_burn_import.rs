use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use burn::module::{Module, Param};
use burn::prelude::{Backend, Tensor, TensorData};
use burn::record::{
    BinFileRecorder, DefaultRecorder, FullPrecisionSettings, HalfPrecisionSettings,
    NamedMpkFileRecorder, Recorder,
};
use burn_core as burn;
use burn_ndarray::NdArray;
use burn_nn::{Dropout, DropoutConfig, Embedding, EmbeddingConfig, Gelu, Linear, LinearConfig};
use burn_store::ModuleSnapshot;
use psionic_models::{
    ATTN_RES_MODEL_FAMILY, AttnResConfig, AttnResCpuReferenceModel, AttnResModelError,
    AttnResParameterVector, ModelDescriptor, WeightArtifactMetadata, WeightFormat, WeightSource,
};
use safetensors::{Dtype as SafeTensorsDType, serialize, tensor::TensorView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use burn::prelude::Int;
#[cfg(test)]
use burn::record::CompactRecorder;
#[cfg(test)]
use burn::tensor::activation::softmax;

const ATTNRES_BURN_IMPORT_SCHEMA_VERSION: u16 = 1;
const ATTNRES_BURN_IMPORT_METADATA_KEY: &str = "psionic.attnres.burn_import_manifest";

/// Legacy Burn recorder format supported by the optional AttnRes migration tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResBurnArtifactFormat {
    /// Burn default recorder using named msgpack and full precision.
    DefaultNamedMpk,
    /// Burn compact recorder using named msgpack and half precision.
    CompactNamedMpk,
    /// Burn binary recorder using bincode.
    Binary,
}

impl AttnResBurnArtifactFormat {
    /// Returns the stable CLI label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DefaultNamedMpk => "default",
            Self::CompactNamedMpk => "compact",
            Self::Binary => "binary",
        }
    }

    fn file_extension(self) -> &'static str {
        match self {
            Self::DefaultNamedMpk | Self::CompactNamedMpk => "mpk",
            Self::Binary => "bin",
        }
    }
}

impl std::str::FromStr for AttnResBurnArtifactFormat {
    type Err = AttnResBurnImportError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "default" | "default_named_mpk" | "named_mpk" | "mpk" => Ok(Self::DefaultNamedMpk),
            "compact" | "compact_named_mpk" => Ok(Self::CompactNamedMpk),
            "binary" | "bin" => Ok(Self::Binary),
            other => Err(AttnResBurnImportError::InvalidFormat(other.to_string())),
        }
    }
}

/// Import-bound path remap mode for legacy Burn tensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttnResBurnPathRemapMode {
    /// Replace the whole path when it matches exactly.
    Exact,
    /// Rewrite one leading path prefix.
    Prefix,
    /// Rewrite one trailing path suffix.
    Suffix,
    /// Replace every occurrence of the source substring.
    Replace,
}

/// One import-bound path remap rule applied before canonical Burn-to-Psionic mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResBurnPathRemap {
    /// Matching mode.
    pub mode: AttnResBurnPathRemapMode,
    /// Source path fragment.
    pub from: String,
    /// Replacement path fragment.
    pub to: String,
}

impl AttnResBurnPathRemap {
    /// Creates one explicit path remap rule.
    #[must_use]
    pub fn new(
        mode: AttnResBurnPathRemapMode,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            mode,
            from: from.into(),
            to: to.into(),
        }
    }

    fn validate(&self) -> Result<(), AttnResBurnImportError> {
        if self.from.is_empty() {
            return Err(AttnResBurnImportError::InvalidRemap {
                detail: String::from("path remap `from` must not be empty"),
            });
        }
        Ok(())
    }

    fn apply(&self, path: &str) -> String {
        match self.mode {
            AttnResBurnPathRemapMode::Exact if path == self.from => self.to.clone(),
            AttnResBurnPathRemapMode::Prefix if path.starts_with(&self.from) => {
                format!("{}{}", self.to, &path[self.from.len()..])
            }
            AttnResBurnPathRemapMode::Suffix if path.ends_with(&self.from) => {
                format!("{}{}", &path[..path.len() - self.from.len()], self.to)
            }
            AttnResBurnPathRemapMode::Replace => path.replace(&self.from, &self.to),
            _ => path.to_string(),
        }
    }
}

/// One research-owned import request for a legacy Burn AttnRes artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResBurnImportRequest {
    /// Path to the legacy artifact, with or without the recorder extension.
    pub source_path: PathBuf,
    /// Legacy recorder family.
    pub source_format: AttnResBurnArtifactFormat,
    /// Canonical Psionic AttnRes config the import must honor.
    pub config: AttnResConfig,
    /// Stable Psionic model identifier for the imported descriptor.
    pub model_id: String,
    /// Stable Psionic model revision for the imported descriptor.
    pub model_revision: String,
    /// Whether missing or unmapped tensors are allowed to keep seeded Psionic defaults.
    pub allow_partial: bool,
    /// Optional path remaps applied only at this import boundary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_remaps: Vec<AttnResBurnPathRemap>,
}

impl AttnResBurnImportRequest {
    /// Creates a request with the default imported-model identity.
    #[must_use]
    pub fn new(
        source_path: impl Into<PathBuf>,
        source_format: AttnResBurnArtifactFormat,
        config: AttnResConfig,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            source_format,
            config,
            model_id: String::from("attnres-legacy-import"),
            model_revision: String::from("burn-import-v1"),
            allow_partial: false,
            path_remaps: Vec::new(),
        }
    }

    /// Returns a copy with an explicit imported-model identity.
    #[must_use]
    pub fn with_model_identity(
        mut self,
        model_id: impl Into<String>,
        model_revision: impl Into<String>,
    ) -> Self {
        self.model_id = model_id.into();
        self.model_revision = model_revision.into();
        self
    }

    /// Returns a copy with explicit import-bound partial-load posture.
    #[must_use]
    pub const fn with_allow_partial(mut self, allow_partial: bool) -> Self {
        self.allow_partial = allow_partial;
        self
    }

    /// Returns a copy with one additional path remap.
    #[must_use]
    pub fn with_path_remap(mut self, remap: AttnResBurnPathRemap) -> Self {
        self.path_remaps.push(remap);
        self
    }

    fn validate(&self) -> Result<(), AttnResBurnImportError> {
        self.config.validate().map_err(AttnResModelError::from)?;
        if self.model_id.trim().is_empty() {
            return Err(AttnResBurnImportError::InvalidRequest(String::from(
                "model_id must not be empty",
            )));
        }
        if self.model_revision.trim().is_empty() {
            return Err(AttnResBurnImportError::InvalidRequest(String::from(
                "model_revision must not be empty",
            )));
        }
        for remap in &self.path_remaps {
            remap.validate()?;
        }
        Ok(())
    }
}

/// One imported source tensor that was matched onto a canonical Psionic parameter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResBurnImportedTensor {
    /// Raw tensor path from the legacy Burn artifact.
    pub source_path: String,
    /// Source path after import-bound remaps.
    pub canonical_source_path: String,
    /// Canonical Psionic parameter id.
    pub target_parameter_id: String,
    /// Logical tensor shape.
    pub shape: Vec<usize>,
    /// Stable digest over the imported dense values.
    pub value_digest: String,
}

/// One legacy source tensor that remained unmapped after the import boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttnResBurnUnmappedTensor {
    /// Raw tensor path from the legacy Burn artifact.
    pub source_path: String,
    /// Source path after import-bound remaps.
    pub canonical_source_path: String,
}

/// Machine-readable manifest for one legacy Burn AttnRes import.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttnResBurnImportManifest {
    /// Stable schema version for the import manifest.
    pub schema_version: u16,
    /// Imported Psionic model id.
    pub model_id: String,
    /// Imported Psionic model revision.
    pub model_revision: String,
    /// Concrete legacy artifact path consumed by the import.
    pub source_path: String,
    /// Legacy recorder family that produced the source artifact.
    pub source_format: AttnResBurnArtifactFormat,
    /// SHA-256 digest of the legacy source artifact bytes.
    pub source_sha256: String,
    /// Canonical Psionic AttnRes config the import targeted.
    pub config: AttnResConfig,
    /// Whether import-bound partial load was enabled.
    pub allow_partial: bool,
    /// Explicit import-bound remap rules that were applied before canonical mapping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_remaps: Vec<AttnResBurnPathRemap>,
    /// Imported source tensors that mapped onto canonical Psionic parameters.
    pub imported_tensors: Vec<AttnResBurnImportedTensor>,
    /// Source tensors that stayed outside the current mapping contract.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmapped_tensors: Vec<AttnResBurnUnmappedTensor>,
    /// Canonical Psionic parameters that were not present in the legacy artifact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_parameter_ids: Vec<String>,
    /// Weight artifact emitted by the import in canonical Psionic safetensors form.
    pub weights_artifact: WeightArtifactMetadata,
    /// Stable digest of the resulting Psionic weight bundle.
    pub weight_bundle_digest: String,
    /// Stable digest of the resulting Psionic descriptor.
    pub descriptor_digest: String,
}

impl AttnResBurnImportManifest {
    /// Returns a stable digest over the manifest payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_json_digest(b"psionic_attnres_burn_import_manifest|", self)
    }
}

/// In-memory canonical Psionic bundle produced by a legacy Burn import.
#[derive(Clone, Debug, PartialEq)]
pub struct AttnResBurnImportBundle {
    /// Imported Psionic model descriptor plus CPU-reference weight bundle.
    pub model: AttnResCpuReferenceModel,
    /// Canonical Psionic safetensors payload emitted by the import.
    pub weights_safetensors: Vec<u8>,
    /// Machine-readable import manifest.
    pub manifest: AttnResBurnImportManifest,
}

/// Concrete file outputs written by the import persistence helper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttnResBurnImportOutput {
    /// Written safetensors file path.
    pub weights_path: PathBuf,
    /// Written manifest path.
    pub manifest_path: PathBuf,
}

/// Failure while importing one legacy Burn AttnRes artifact.
#[derive(Debug, Error)]
pub enum AttnResBurnImportError {
    /// The requested recorder format label was invalid.
    #[error("unknown attnres burn import format `{0}`")]
    InvalidFormat(String),
    /// The request was structurally invalid.
    #[error("invalid attnres burn import request: {0}")]
    InvalidRequest(String),
    /// One path remap was invalid.
    #[error("invalid attnres burn import remap: {detail}")]
    InvalidRemap { detail: String },
    /// The source artifact could not be read.
    #[error("failed to read legacy attnres artifact at {path}: {detail}")]
    SourceRead { path: String, detail: String },
    /// The legacy recorder payload could not be loaded into the old Burn model.
    #[error("failed to load legacy attnres artifact at {path}: {detail}")]
    LegacyLoad { path: String, detail: String },
    /// One legacy tensor snapshot could not be materialized.
    #[error("failed to materialize legacy tensor `{path}`: {detail}")]
    SnapshotLoad { path: String, detail: String },
    /// The importer saw the same Psionic parameter twice.
    #[error(
        "legacy attnres import mapped both `{first_source_path}` and `{second_source_path}` to `{parameter_id}`"
    )]
    DuplicateParameterTarget {
        /// Canonical Psionic parameter id.
        parameter_id: String,
        /// First source path that claimed the parameter.
        first_source_path: String,
        /// Second source path that claimed the parameter.
        second_source_path: String,
    },
    /// One imported source tensor shape did not match the canonical Psionic parameter.
    #[error(
        "legacy attnres tensor `{source_path}` shape {actual:?} does not match `{parameter_id}` shape {expected:?}"
    )]
    ShapeMismatch {
        /// Legacy source path.
        source_path: String,
        /// Canonical Psionic parameter id.
        parameter_id: String,
        /// Expected logical tensor shape.
        expected: Vec<usize>,
        /// Actual imported tensor shape.
        actual: Vec<usize>,
    },
    /// The legacy tensor used a non-float dtype.
    #[error("legacy attnres tensor `{path}` used unsupported dtype `{dtype}`")]
    UnsupportedDType {
        /// Legacy source path.
        path: String,
        /// Rendered dtype label.
        dtype: String,
    },
    /// The import stayed incomplete while strict posture was requested.
    #[error(
        "legacy attnres import remained incomplete with {missing_parameter_count} missing parameter(s) and {unmapped_tensor_count} unmapped tensor(s); enable allow_partial to keep seeded Psionic defaults only at the import boundary"
    )]
    IncompleteImport {
        /// Canonical Psionic parameter ids that were missing from the source.
        missing_parameter_ids: Vec<String>,
        /// Source tensors that remained unmapped.
        unmapped_tensors: Vec<AttnResBurnUnmappedTensor>,
        /// Missing parameter count.
        missing_parameter_count: usize,
        /// Unmapped source-tensor count.
        unmapped_tensor_count: usize,
    },
    /// Psionic AttnRes model construction failed.
    #[error(transparent)]
    Model(#[from] AttnResModelError),
    /// SafeTensors export failed.
    #[error("failed to export attnres safetensors: {0}")]
    SafeTensors(String),
    /// Persisting the canonical import bundle failed.
    #[error("failed to persist attnres burn import output at {path}: {detail}")]
    PersistFailure { path: String, detail: String },
}

/// Imports one legacy Burn AttnRes artifact into canonical Psionic safetensors plus manifest form.
pub fn import_attnres_burn_artifact(
    request: &AttnResBurnImportRequest,
) -> Result<AttnResBurnImportBundle, AttnResBurnImportError> {
    request.validate()?;
    let source_file = artifact_file_path(&request.source_path, request.source_format);
    let source_bytes = fs::read(&source_file).map_err(|error| AttnResBurnImportError::SourceRead {
        path: source_file.display().to_string(),
        detail: error.to_string(),
    })?;
    let source_sha256 = hex::encode(Sha256::digest(source_bytes.as_slice()));
    let legacy_model = load_legacy_model(request)?;

    let base_bundle = psionic_models::AttnResWeightBundle::seeded_reference(&request.config)?;
    let expected_parameters = expected_parameter_inventory(&base_bundle);
    let mut overrides = BTreeMap::new();
    let mut imported_tensors = Vec::new();
    let mut parameter_sources = BTreeMap::<String, String>::new();
    let mut unmapped_tensors = Vec::new();

    for snapshot in legacy_model.collect(None, None, false) {
        let source_path = snapshot.full_path();
        let canonical_source_path = request
            .path_remaps
            .iter()
            .fold(source_path.clone(), |path, remap| remap.apply(path.as_str()));
        let Some(parameter_id) = map_legacy_path_to_psionic(&canonical_source_path) else {
            unmapped_tensors.push(AttnResBurnUnmappedTensor {
                source_path,
                canonical_source_path,
            });
            continue;
        };

        if let Some(first_source_path) = parameter_sources.get(&parameter_id) {
            return Err(AttnResBurnImportError::DuplicateParameterTarget {
                parameter_id,
                first_source_path: first_source_path.clone(),
                second_source_path: source_path,
            });
        }

        let data = snapshot
            .to_data()
            .map_err(|error| AttnResBurnImportError::SnapshotLoad {
                path: source_path.clone(),
                detail: error.to_string(),
            })?;
        let values = tensor_data_to_f32(&source_path, &data)?;
        let actual_shape = data.shape.clone();
        let Some(expected) = expected_parameters.get(&parameter_id) else {
            return Err(AttnResBurnImportError::InvalidRequest(format!(
                "psionic attnres parameter inventory is missing `{parameter_id}`"
            )));
        };
        if expected.shape != actual_shape {
            return Err(AttnResBurnImportError::ShapeMismatch {
                source_path: source_path.clone(),
                parameter_id,
                expected: expected.shape.clone(),
                actual: actual_shape,
            });
        }

        parameter_sources.insert(parameter_id.clone(), source_path.clone());
        imported_tensors.push(AttnResBurnImportedTensor {
            source_path,
            canonical_source_path,
            target_parameter_id: parameter_id.clone(),
            shape: expected.shape.clone(),
            value_digest: stable_bytes_digest(
                b"psionic_attnres_imported_tensor|",
                &encode_f32_bytes(&values),
            ),
        });
        overrides.insert(parameter_id, values);
    }

    imported_tensors.sort_by(|left, right| left.target_parameter_id.cmp(&right.target_parameter_id));
    unmapped_tensors.sort_by(|left, right| left.source_path.cmp(&right.source_path));

    let mut missing_parameter_ids = expected_parameters
        .keys()
        .filter(|parameter_id| !overrides.contains_key(*parameter_id))
        .cloned()
        .collect::<Vec<_>>();
    missing_parameter_ids.sort();

    if !request.allow_partial && (!missing_parameter_ids.is_empty() || !unmapped_tensors.is_empty()) {
        return Err(AttnResBurnImportError::IncompleteImport {
            missing_parameter_count: missing_parameter_ids.len(),
            unmapped_tensor_count: unmapped_tensors.len(),
            missing_parameter_ids,
            unmapped_tensors,
        });
    }

    let imported_bundle = base_bundle.with_parameter_overrides(&request.config, &overrides)?;
    let weights_safetensors = export_bundle_safetensors(&imported_bundle.parameter_vectors())?;
    let weights_artifact = WeightArtifactMetadata::new(
        "weights.safetensors",
        weights_safetensors.len() as u64,
        hex::encode(Sha256::digest(weights_safetensors.as_slice())),
    );
    let imported_bundle = imported_bundle.with_artifact_metadata(
        WeightFormat::SafeTensors,
        WeightSource::ExternalArtifact,
        vec![weights_artifact.clone()],
    );
    let model = AttnResCpuReferenceModel::with_weights(
        ModelDescriptor::new(
            request.model_id.clone(),
            ATTN_RES_MODEL_FAMILY,
            request.model_revision.clone(),
        ),
        request.config.clone(),
        imported_bundle,
    )?;
    let manifest = AttnResBurnImportManifest {
        schema_version: ATTNRES_BURN_IMPORT_SCHEMA_VERSION,
        model_id: request.model_id.clone(),
        model_revision: request.model_revision.clone(),
        source_path: source_file.display().to_string(),
        source_format: request.source_format,
        source_sha256,
        config: request.config.clone(),
        allow_partial: request.allow_partial,
        path_remaps: request.path_remaps.clone(),
        imported_tensors,
        unmapped_tensors,
        missing_parameter_ids,
        weights_artifact,
        weight_bundle_digest: model.weights().metadata().digest.clone(),
        descriptor_digest: model.descriptor().stable_digest(),
    };
    Ok(AttnResBurnImportBundle {
        model,
        weights_safetensors,
        manifest,
    })
}

/// Persists one canonical import bundle under the supplied output directory.
pub fn persist_attnres_burn_import_bundle(
    bundle: &AttnResBurnImportBundle,
    output_dir: &Path,
) -> Result<AttnResBurnImportOutput, AttnResBurnImportError> {
    fs::create_dir_all(output_dir).map_err(|error| AttnResBurnImportError::PersistFailure {
        path: output_dir.display().to_string(),
        detail: error.to_string(),
    })?;

    let weights_path = output_dir.join("weights.safetensors");
    fs::write(&weights_path, &bundle.weights_safetensors).map_err(|error| {
        AttnResBurnImportError::PersistFailure {
            path: weights_path.display().to_string(),
            detail: error.to_string(),
        }
    })?;

    let manifest_path = output_dir.join("manifest.json");
    let manifest_bytes = serde_json::to_vec_pretty(&bundle.manifest).map_err(|error| {
        AttnResBurnImportError::PersistFailure {
            path: manifest_path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    fs::write(&manifest_path, manifest_bytes).map_err(|error| {
        AttnResBurnImportError::PersistFailure {
            path: manifest_path.display().to_string(),
            detail: error.to_string(),
        }
    })?;

    Ok(AttnResBurnImportOutput {
        weights_path,
        manifest_path,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParameterInventoryEntry {
    shape: Vec<usize>,
}

#[derive(Module, Debug)]
struct LegacyRmsNorm<B: Backend> {
    gamma: Param<Tensor<B, 1>>,
    eps: f64,
}

impl<B: Backend> LegacyRmsNorm<B> {
    #[cfg(test)]
    fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let variance = x.clone().powf_scalar(2.0).mean_dim(2);
        let rms = variance.add_scalar(self.eps).sqrt();
        x / rms
            * self
                .gamma
                .val()
                .unsqueeze_dim::<2>(0)
                .unsqueeze_dim::<3>(0)
    }

    #[cfg(test)]
    fn forward_4d(&self, x: Tensor<B, 4>) -> Tensor<B, 4> {
        let variance = x.clone().powf_scalar(2.0).mean_dim(3);
        let rms = variance.add_scalar(self.eps).sqrt();
        x / rms
            * self
                .gamma
                .val()
                .unsqueeze_dim::<2>(0)
                .unsqueeze_dim::<3>(0)
                .unsqueeze_dim::<4>(0)
    }
}

#[derive(Module, Debug)]
struct LegacyAttnResOp<B: Backend> {
    pseudo_query: Param<Tensor<B, 1>>,
    norm: LegacyRmsNorm<B>,
}

impl<B: Backend> LegacyAttnResOp<B> {
    #[cfg(test)]
    fn forward_optional_partial(
        &self,
        blocks: &[Tensor<B, 3>],
        partial_block: Option<&Tensor<B, 3>>,
    ) -> Tensor<B, 3> {
        let mut sources = blocks.to_vec();
        if let Some(partial_block) = partial_block {
            sources.push(partial_block.clone());
        }

        let values = Tensor::stack(sources, 0);
        let normed = self.norm.forward_4d(values.clone());
        let query = self
            .pseudo_query
            .val()
            .unsqueeze_dim::<2>(0)
            .unsqueeze_dim::<3>(0)
            .unsqueeze_dim::<4>(0);
        let logits = (normed * query).sum_dim(3).squeeze_dim::<3>(3);
        let routing = softmax(logits, 0);
        (values * routing.unsqueeze_dim::<4>(3))
            .sum_dim(0)
            .squeeze_dim::<3>(0)
    }
}

#[derive(Module, Debug)]
struct LegacyMultiHeadAttention<B: Backend> {
    q_proj: Linear<B>,
    k_proj: Linear<B>,
    v_proj: Linear<B>,
    o_proj: Linear<B>,
    dropout: Dropout,
    num_heads: usize,
    d_head: usize,
}

impl<B: Backend> LegacyMultiHeadAttention<B> {
    #[cfg(test)]
    fn forward(&self, x: Tensor<B, 3>, mask: Option<&Tensor<B, 3>>) -> Tensor<B, 3> {
        let [batch, seq_len, _] = x.dims();
        let q = self.q_proj.forward(x.clone());
        let k = self.k_proj.forward(x.clone());
        let v = self.v_proj.forward(x);
        let q = q
            .reshape([batch, seq_len, self.num_heads, self.d_head])
            .swap_dims(1, 2);
        let k = k
            .reshape([batch, seq_len, self.num_heads, self.d_head])
            .swap_dims(1, 2);
        let v = v
            .reshape([batch, seq_len, self.num_heads, self.d_head])
            .swap_dims(1, 2);
        let scores = q.matmul(k.swap_dims(2, 3)) / (self.d_head as f64).sqrt();
        let scores = match mask {
            Some(mask) => scores + mask.clone().unsqueeze_dim::<4>(1),
            None => scores,
        };
        let routing = self.dropout.forward(softmax(scores, 3));
        let merged = routing
            .matmul(v)
            .swap_dims(1, 2)
            .reshape([batch, seq_len, self.num_heads * self.d_head]);
        self.o_proj.forward(merged)
    }
}

#[derive(Module, Debug)]
struct LegacyFeedForward<B: Backend> {
    linear1: Linear<B>,
    linear2: Linear<B>,
    gelu: Gelu,
    dropout: Dropout,
}

impl<B: Backend> LegacyFeedForward<B> {
    #[cfg(test)]
    fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 3> {
        let x = self.linear1.forward(x);
        let x = self.gelu.forward(x);
        let x = self.dropout.forward(x);
        let x = self.linear2.forward(x);
        self.dropout.forward(x)
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct LegacyBlockState<B: Backend> {
    blocks: Vec<Tensor<B, 3>>,
    partial_block: Option<Tensor<B, 3>>,
}

#[cfg(test)]
impl<B: Backend> LegacyBlockState<B> {
    fn new(embeddings: Tensor<B, 3>) -> Self {
        Self {
            blocks: vec![embeddings],
            partial_block: None,
        }
    }
}

#[derive(Module, Debug)]
struct LegacyAttnResLayer<B: Backend> {
    layer_idx: usize,
    block_size: usize,
    attn_res: LegacyAttnResOp<B>,
    mlp_res: LegacyAttnResOp<B>,
    attn_norm: LegacyRmsNorm<B>,
    attn: LegacyMultiHeadAttention<B>,
    mlp_norm: LegacyRmsNorm<B>,
    mlp: LegacyFeedForward<B>,
}

impl<B: Backend> LegacyAttnResLayer<B> {
    #[cfg(test)]
    fn starts_new_block_before_sublayer(&self, sublayer_idx: usize) -> bool {
        sublayer_idx > 0 && sublayer_idx.is_multiple_of(self.block_size)
    }

    #[cfg(test)]
    fn forward(&self, mut state: LegacyBlockState<B>, mask: Option<&Tensor<B, 3>>) -> LegacyBlockState<B> {
        let attn_sublayer_idx = self.layer_idx * 2;
        let mlp_sublayer_idx = attn_sublayer_idx + 1;

        let current_partial = state.partial_block.take();
        let hidden = self
            .attn_res
            .forward_optional_partial(&state.blocks, current_partial.as_ref());
        let mut partial_for_attn =
            current_partial.unwrap_or_else(|| Tensor::zeros_like(state.blocks.last().expect("block")));
        if self.starts_new_block_before_sublayer(attn_sublayer_idx) {
            state.blocks.push(partial_for_attn.clone());
            partial_for_attn = Tensor::zeros_like(state.blocks.last().expect("block"));
        }
        let attn_out = self.attn.forward(self.attn_norm.forward(hidden), mask);
        let partial_after_attn = partial_for_attn + attn_out;

        let hidden = self
            .mlp_res
            .forward_optional_partial(&state.blocks, Some(&partial_after_attn));
        let mut partial_for_mlp = partial_after_attn;
        if self.starts_new_block_before_sublayer(mlp_sublayer_idx) {
            state.blocks.push(partial_for_mlp.clone());
            partial_for_mlp = Tensor::zeros_like(state.blocks.last().expect("block"));
        }
        let mlp_out = self.mlp.forward(self.mlp_norm.forward(hidden));
        state.partial_block = Some(partial_for_mlp + mlp_out);
        state
    }
}

#[derive(Module, Debug)]
struct LegacyAttnResTransformer<B: Backend> {
    embedding: Embedding<B>,
    layers: Vec<LegacyAttnResLayer<B>>,
    final_norm: LegacyRmsNorm<B>,
    lm_head: Linear<B>,
}

impl<B: Backend> LegacyAttnResTransformer<B> {
    fn init_from_config(config: &AttnResConfig, device: &B::Device) -> Result<Self, AttnResModelError> {
        config.validate()?;
        let block_size = config.block_size()?;
        let head_dim = config.head_dim()?;
        let d_ff = config.effective_d_ff()?;
        let layers = (0..config.num_transformer_layers())
            .map(|layer_idx| LegacyAttnResLayer {
                layer_idx,
                block_size,
                attn_res: LegacyAttnResOp {
                    pseudo_query: Param::from_tensor(Tensor::zeros([config.d_model], device)),
                    norm: LegacyRmsNorm {
                        gamma: Param::from_tensor(Tensor::ones([config.d_model], device)),
                        eps: f64::from(config.rms_norm_eps),
                    },
                },
                mlp_res: LegacyAttnResOp {
                    pseudo_query: Param::from_tensor(Tensor::zeros([config.d_model], device)),
                    norm: LegacyRmsNorm {
                        gamma: Param::from_tensor(Tensor::ones([config.d_model], device)),
                        eps: f64::from(config.rms_norm_eps),
                    },
                },
                attn_norm: LegacyRmsNorm {
                    gamma: Param::from_tensor(Tensor::ones([config.d_model], device)),
                    eps: f64::from(config.rms_norm_eps),
                },
                attn: LegacyMultiHeadAttention {
                    q_proj: LinearConfig::new(config.d_model, config.d_model).init(device),
                    k_proj: LinearConfig::new(config.d_model, config.d_model).init(device),
                    v_proj: LinearConfig::new(config.d_model, config.d_model).init(device),
                    o_proj: LinearConfig::new(config.d_model, config.d_model).init(device),
                    dropout: DropoutConfig::new(f64::from(config.dropout)).init(),
                    num_heads: config.num_heads,
                    d_head: head_dim,
                },
                mlp_norm: LegacyRmsNorm {
                    gamma: Param::from_tensor(Tensor::ones([config.d_model], device)),
                    eps: f64::from(config.rms_norm_eps),
                },
                mlp: LegacyFeedForward {
                    linear1: LinearConfig::new(config.d_model, d_ff).init(device),
                    linear2: LinearConfig::new(d_ff, config.d_model).init(device),
                    gelu: Gelu::new(),
                    dropout: DropoutConfig::new(f64::from(config.dropout)).init(),
                },
            })
            .collect();

        Ok(Self {
            embedding: EmbeddingConfig::new(config.vocab_size, config.d_model).init(device),
            layers,
            final_norm: LegacyRmsNorm {
                gamma: Param::from_tensor(Tensor::ones([config.d_model], device)),
                eps: f64::from(config.rms_norm_eps),
            },
            lm_head: LinearConfig::new(config.d_model, config.vocab_size).init(device),
        })
    }

    #[cfg(test)]
    fn forward(&self, input_ids: Tensor<B, 2, Int>, mask: Option<&Tensor<B, 3>>) -> Tensor<B, 3> {
        let embeddings = self.embedding.forward(input_ids);
        let mut state = LegacyBlockState::new(embeddings);
        for layer in &self.layers {
            state = layer.forward(state, mask);
        }
        let hidden = state.partial_block.expect("final partial block");
        self.lm_head.forward(self.final_norm.forward(hidden))
    }

    #[cfg(test)]
    fn save<P: AsRef<Path>>(&self, path: P, _device: &B::Device) -> Result<(), AttnResBurnImportError> {
        DefaultRecorder::default()
            .record(self.clone().into_record(), path.as_ref().to_path_buf())
            .map_err(|error| AttnResBurnImportError::LegacyLoad {
                path: artifact_file_path(path.as_ref(), AttnResBurnArtifactFormat::DefaultNamedMpk)
                    .display()
                    .to_string(),
                detail: error.to_string(),
            })
    }

    fn load<P: AsRef<Path>>(
        path: P,
        config: &AttnResConfig,
        device: &B::Device,
    ) -> Result<Self, AttnResBurnImportError> {
        let record = DefaultRecorder::default()
            .load(recorder_base_path(path.as_ref(), AttnResBurnArtifactFormat::DefaultNamedMpk), device)
            .map_err(|error| AttnResBurnImportError::LegacyLoad {
                path: artifact_file_path(path.as_ref(), AttnResBurnArtifactFormat::DefaultNamedMpk)
                    .display()
                    .to_string(),
                detail: error.to_string(),
            })?;
        Ok(Self::init_from_config(config, device)?.load_record(record))
    }

    #[cfg(test)]
    fn save_compact<P: AsRef<Path>>(&self, path: P) -> Result<(), AttnResBurnImportError> {
        CompactRecorder::default()
            .record(self.clone().into_record(), path.as_ref().to_path_buf())
            .map_err(|error| AttnResBurnImportError::LegacyLoad {
                path: artifact_file_path(path.as_ref(), AttnResBurnArtifactFormat::CompactNamedMpk)
                    .display()
                    .to_string(),
                detail: error.to_string(),
            })
    }

    fn load_compact<P: AsRef<Path>>(
        path: P,
        config: &AttnResConfig,
        device: &B::Device,
    ) -> Result<Self, AttnResBurnImportError> {
        let record = NamedMpkFileRecorder::<HalfPrecisionSettings>::default()
            .load(recorder_base_path(path.as_ref(), AttnResBurnArtifactFormat::CompactNamedMpk), device)
            .map_err(|error| AttnResBurnImportError::LegacyLoad {
                path: artifact_file_path(path.as_ref(), AttnResBurnArtifactFormat::CompactNamedMpk)
                    .display()
                    .to_string(),
                detail: error.to_string(),
            })?;
        Ok(Self::init_from_config(config, device)?.load_record(record))
    }

    #[cfg(test)]
    fn save_binary<P: AsRef<Path>>(&self, path: P) -> Result<(), AttnResBurnImportError> {
        BinFileRecorder::<FullPrecisionSettings>::default()
            .record(self.clone().into_record(), path.as_ref().to_path_buf())
            .map_err(|error| AttnResBurnImportError::LegacyLoad {
                path: artifact_file_path(path.as_ref(), AttnResBurnArtifactFormat::Binary)
                    .display()
                    .to_string(),
                detail: error.to_string(),
            })
    }

    fn load_binary<P: AsRef<Path>>(
        path: P,
        config: &AttnResConfig,
        device: &B::Device,
    ) -> Result<Self, AttnResBurnImportError> {
        let record = BinFileRecorder::<FullPrecisionSettings>::default()
            .load(recorder_base_path(path.as_ref(), AttnResBurnArtifactFormat::Binary), device)
            .map_err(|error| AttnResBurnImportError::LegacyLoad {
                path: artifact_file_path(path.as_ref(), AttnResBurnArtifactFormat::Binary)
                    .display()
                    .to_string(),
                detail: error.to_string(),
            })?;
        Ok(Self::init_from_config(config, device)?.load_record(record))
    }
}

fn artifact_file_path(source_path: &Path, format: AttnResBurnArtifactFormat) -> PathBuf {
    match source_path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension == format.file_extension() => source_path.to_path_buf(),
        _ => source_path.with_extension(format.file_extension()),
    }
}

fn recorder_base_path(source_path: &Path, format: AttnResBurnArtifactFormat) -> PathBuf {
    match source_path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension == format.file_extension() => {
            let stem = source_path
                .file_stem()
                .map(|stem| stem.to_os_string())
                .unwrap_or_default();
            source_path.with_file_name(stem)
        }
        _ => source_path.to_path_buf(),
    }
}

fn load_legacy_model(
    request: &AttnResBurnImportRequest,
) -> Result<LegacyAttnResTransformer<NdArray>, AttnResBurnImportError> {
    let device = Default::default();
    match request.source_format {
        AttnResBurnArtifactFormat::DefaultNamedMpk => {
            LegacyAttnResTransformer::<NdArray>::load(&request.source_path, &request.config, &device)
        }
        AttnResBurnArtifactFormat::CompactNamedMpk => LegacyAttnResTransformer::<NdArray>::load_compact(
            &request.source_path,
            &request.config,
            &device,
        ),
        AttnResBurnArtifactFormat::Binary => {
            LegacyAttnResTransformer::<NdArray>::load_binary(&request.source_path, &request.config, &device)
        }
    }
}

fn expected_parameter_inventory(
    bundle: &psionic_models::AttnResWeightBundle,
) -> BTreeMap<String, ParameterInventoryEntry> {
    bundle
        .parameter_vectors()
        .into_iter()
        .map(|parameter| {
            (
                parameter.parameter_id,
                ParameterInventoryEntry {
                    shape: parameter.shape.dims().to_vec(),
                },
            )
        })
        .collect()
}

fn map_legacy_path_to_psionic(path: &str) -> Option<String> {
    if path == "embedding.weight" {
        return Some(String::from("token_embeddings"));
    }
    if path == "final_norm.gamma" {
        return Some(String::from("final_norm_gamma"));
    }
    if path == "lm_head.weight" || path == "lm_head.bias" {
        return Some(path.to_string());
    }

    let parts = path.split('.').collect::<Vec<_>>();
    if parts.len() < 3 || parts[0] != "layers" {
        return None;
    }
    let layer_idx = parts[1].parse::<usize>().ok()?;
    let rest = &parts[2..];

    match rest {
        ["attn_res", "pseudo_query"] => Some(format!("layers.{layer_idx}.attn_res.pseudo_query")),
        ["attn_res", "norm", "gamma"] => Some(format!("layers.{layer_idx}.attn_res.norm_gamma")),
        ["mlp_res", "pseudo_query"] => Some(format!("layers.{layer_idx}.mlp_res.pseudo_query")),
        ["mlp_res", "norm", "gamma"] => Some(format!("layers.{layer_idx}.mlp_res.norm_gamma")),
        ["attn_norm", "gamma"] => Some(format!("layers.{layer_idx}.attn_norm_gamma")),
        ["mlp_norm", "gamma"] => Some(format!("layers.{layer_idx}.mlp_norm_gamma")),
        ["attn", projection, field @ ("weight" | "bias")] => {
            Some(format!("layers.{layer_idx}.attention.{projection}.{field}"))
        }
        ["mlp", linear, field @ ("weight" | "bias")] => {
            Some(format!("layers.{layer_idx}.feed_forward.{linear}.{field}"))
        }
        _ => None,
    }
}

fn tensor_data_to_f32(
    path: &str,
    data: &TensorData,
) -> Result<Vec<f32>, AttnResBurnImportError> {
    match data.dtype {
        burn::tensor::DType::F64
        | burn::tensor::DType::F32
        | burn::tensor::DType::F16
        | burn::tensor::DType::BF16 => data
            .clone()
            .convert::<f32>()
            .to_vec::<f32>()
            .map_err(|error| AttnResBurnImportError::SnapshotLoad {
                path: path.to_string(),
                detail: error.to_string(),
            }),
        dtype => Err(AttnResBurnImportError::UnsupportedDType {
            path: path.to_string(),
            dtype: format!("{dtype:?}"),
        }),
    }
}

fn export_bundle_safetensors(
    parameters: &[AttnResParameterVector],
) -> Result<Vec<u8>, AttnResBurnImportError> {
    let imported_parameter_ids = parameters
        .iter()
        .map(|parameter| parameter.parameter_id.as_str())
        .collect::<Vec<_>>();
    let metadata_json = serde_json::to_string(&imported_parameter_ids)
        .map_err(|error| AttnResBurnImportError::SafeTensors(error.to_string()))?;
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from(ATTNRES_BURN_IMPORT_METADATA_KEY),
        metadata_json,
    );

    let raw_buffers = parameters
        .iter()
        .map(|parameter| {
            (
                parameter.parameter_id.as_str(),
                encode_f32_bytes(&parameter.values),
                parameter.shape.dims().to_vec(),
            )
        })
        .collect::<Vec<_>>();

    let mut views = Vec::with_capacity(raw_buffers.len());
    for (parameter_id, raw_bytes, shape) in &raw_buffers {
        let view = TensorView::new(SafeTensorsDType::F32, shape.clone(), raw_bytes.as_slice())
            .map_err(|error| AttnResBurnImportError::SafeTensors(error.to_string()))?;
        views.push((*parameter_id, view));
    }
    serialize(
        views.iter().map(|(parameter_id, view)| (*parameter_id, view.clone())),
        Some(metadata),
    )
    .map_err(|error| AttnResBurnImportError::SafeTensors(error.to_string()))
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn stable_json_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    match serde_json::to_vec(value) {
        Ok(bytes) => stable_bytes_digest(prefix, &bytes),
        Err(error) => stable_bytes_digest(prefix, error.to_string().as_bytes()),
    }
}

fn stable_bytes_digest(prefix: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        AttnResBurnArtifactFormat, AttnResBurnImportRequest, AttnResBurnPathRemap,
        AttnResBurnPathRemapMode, LegacyAttnResTransformer, import_attnres_burn_artifact,
    };
    use std::error::Error;

    use burn_core as burn;
    use burn::prelude::{Int, Tensor};
    use burn_ndarray::NdArray;
    use psionic_models::{AttnResConfig, AttnResTensor3, TokenId, TokenSequence};
    use tempfile::tempdir;

    #[test]
    fn legacy_attnres_import_preserves_model_outputs_across_formats() -> Result<(), Box<dyn Error>>
    {
        for (format, tolerance) in [
            (AttnResBurnArtifactFormat::DefaultNamedMpk, 2.0e-4_f32),
            (AttnResBurnArtifactFormat::CompactNamedMpk, 6.0e-3_f32),
            (AttnResBurnArtifactFormat::Binary, 2.0e-4_f32),
        ] {
            let tempdir = tempdir()?;
            let source_root = tempdir.path().join(format!("legacy_attnres_{}", format.label()));
            let config = AttnResConfig::new(8, 4, 2)
                .with_num_heads(2)
                .with_vocab_size(8)
                .with_d_ff(16);
            let device = Default::default();
            let legacy_model = LegacyAttnResTransformer::<NdArray>::init_from_config(&config, &device)?;
            match format {
                AttnResBurnArtifactFormat::DefaultNamedMpk => {
                    legacy_model.save(&source_root, &device)?
                }
                AttnResBurnArtifactFormat::CompactNamedMpk => legacy_model.save_compact(&source_root)?,
                AttnResBurnArtifactFormat::Binary => legacy_model.save_binary(&source_root)?,
            }

            let request = AttnResBurnImportRequest::new(&source_root, format, config.clone())
                .with_model_identity("attnres-imported", format!("{}.v1", format.label()));
            let imported = import_attnres_burn_artifact(&request)?;

            assert!(imported.manifest.missing_parameter_ids.is_empty());
            assert!(imported.manifest.unmapped_tensors.is_empty());
            assert_eq!(
                imported.model.weights().metadata().format,
                psionic_models::WeightFormat::SafeTensors
            );
            assert_eq!(
                imported.model.weights().metadata().source,
                psionic_models::WeightSource::ExternalArtifact
            );

            let input_tokens = vec![TokenSequence::from(vec![
                TokenId(0),
                TokenId(1),
                TokenId(2),
                TokenId(3),
            ])];
            let legacy_input = Tensor::<NdArray, 2, Int>::from_ints([[0, 1, 2, 3]], &device);
            let legacy_mask = Tensor::<NdArray, 2>::ones([4, 4], &device)
                .triu(1)
                .mul_scalar(-1e9)
                .unsqueeze_dim::<3>(0);
            let legacy_output = legacy_model.forward(legacy_input, Some(&legacy_mask)).into_data();
            let imported_output = imported.model.forward(&input_tokens)?;
            let legacy_output = AttnResTensor3::new(
                [1, 4, config.vocab_size],
                legacy_output.convert::<f32>().to_vec::<f32>()?,
            )?;
            let diff = imported_output.max_abs_diff(&legacy_output)?;
            assert!(
                diff <= tolerance,
                "format {} diff {diff} exceeded tolerance {tolerance}",
                format.label()
            );
        }
        Ok(())
    }

    #[test]
    fn legacy_attnres_import_keeps_seeded_defaults_only_when_allow_partial_is_enabled(
    ) -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let source_root = tempdir.path().join("legacy_attnres_partial");
        let config = AttnResConfig::new(8, 4, 2)
            .with_num_heads(2)
            .with_vocab_size(8)
            .with_d_ff(16);
        let device = Default::default();
        let legacy_model = LegacyAttnResTransformer::<NdArray>::init_from_config(&config, &device)?;
        legacy_model.save(&source_root, &device)?;

        let request = AttnResBurnImportRequest::new(
            &source_root,
            AttnResBurnArtifactFormat::DefaultNamedMpk,
            config.clone(),
        )
        .with_allow_partial(true)
        .with_path_remap(AttnResBurnPathRemap::new(
            AttnResBurnPathRemapMode::Exact,
            "lm_head.bias",
            "legacy.lm_head.bias",
        ));
        let imported = import_attnres_burn_artifact(&request)?;
        assert_eq!(
            imported.manifest.missing_parameter_ids,
            vec![String::from("lm_head.bias")]
        );
        assert_eq!(imported.manifest.unmapped_tensors.len(), 1);

        let seeded = psionic_models::AttnResCpuReferenceModel::seeded(
            "attnres-seeded",
            "v0",
            config,
        )?;
        let imported_bias = imported
            .model
            .weights()
            .parameter_vector("lm_head.bias")
            .ok_or("missing imported lm_head.bias")?;
        let seeded_bias = seeded
            .weights()
            .parameter_vector("lm_head.bias")
            .ok_or("missing seeded lm_head.bias")?;
        assert_eq!(imported_bias.values, seeded_bias.values);
        Ok(())
    }
}
