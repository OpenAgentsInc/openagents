use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    sync::Arc,
};

use psionic_core::{DType, Device, QuantizedTensorData, TensorData, TensorSpec};
use psionic_data::{TokenizerDigest, TokenizerFamily};
use psionic_models::{
    GgufContent, GgufTokenizerMetadata, GgufTokenizerModel, GgufWeightBundleLoader,
    LocalWeightBundleLoader, ModelLoadError, WeightTensorStorage,
};
use safetensors::{
    Dtype as SafeTensorsDType, SafeTensorError, SafeTensors, serialize, tensor::TensorView,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    OptimizerStateResidency, TrainingOptimizerConfig, TrainingOptimizerKind,
    TrainingOptimizerResidencyPolicy, TrainingOptimizerState, TrainingParameterClass,
    TrainingParameterGroupSemantics, TrainingParameterGroupState, TrainingSchedulerBinding,
    TrainingTensorBuffer, core_loop::TrainingCoreError,
};

const SAFETENSORS_MANIFEST_KEY: &str = "psionic.model_io.bundle_manifest";

/// Error returned by the portable model-IO layer.
#[derive(Debug, Error)]
pub enum ModelIoError {
    /// A state-dict tensor key was repeated.
    #[error("state dict tensor `{state_key}` was defined more than once")]
    DuplicateTensor {
        /// Stable tensor key.
        state_key: String,
    },
    /// A state-dict group identifier was repeated.
    #[error("state dict group `{group_id}` was defined more than once")]
    DuplicateGroup {
        /// Stable group identifier.
        group_id: String,
    },
    /// A tensor manifest did not match the map key carrying it.
    #[error(
        "state dict tensor manifest key mismatch: map key `{map_key}` does not match manifest key `{manifest_key}`"
    )]
    TensorManifestKeyMismatch {
        /// Tensor key used by the map.
        map_key: String,
        /// Tensor key embedded in the manifest.
        manifest_key: String,
    },
    /// One tensor payload length mismatched its tensor spec.
    #[error(
        "state dict tensor `{state_key}` payload length mismatch: expected {expected_len}, found {actual_len}"
    )]
    TensorPayloadLengthMismatch {
        /// Stable tensor key.
        state_key: String,
        /// Expected logical or backing length.
        expected_len: usize,
        /// Actual payload length.
        actual_len: usize,
    },
    /// One quantized tensor layout mismatched its spec.
    #[error(
        "state dict tensor `{state_key}` quantized layout mismatch: expected {expected_len} logical elements, found {actual_len}"
    )]
    QuantizedTensorLayoutMismatch {
        /// Stable tensor key.
        state_key: String,
        /// Expected element count.
        expected_len: usize,
        /// Actual element count surfaced by the quantized layout.
        actual_len: usize,
    },
    /// One tensor data family could not be represented by the requested export.
    #[error("state dict tensor `{state_key}` cannot be exported as dense safetensors")]
    UnsupportedSafetensorsTensor {
        /// Stable tensor key.
        state_key: String,
    },
    /// One tensor used a non-contiguous layout that the current export cannot represent.
    #[error(
        "state dict tensor `{state_key}` uses a non-contiguous layout and cannot be exported as safetensors"
    )]
    NonContiguousSafetensorsTensor {
        /// Stable tensor key.
        state_key: String,
    },
    /// A required tensor key was missing from the state dict.
    #[error("state dict tensor `{state_key}` is missing")]
    MissingTensor {
        /// Stable tensor key.
        state_key: String,
    },
    /// A group referenced a tensor with the wrong role.
    #[error(
        "state dict group `{group_id}` expected tensor `{state_key}` to have role `{expected}`, found `{actual}`"
    )]
    TensorRoleMismatch {
        /// Stable group identifier.
        group_id: String,
        /// Stable tensor key.
        state_key: String,
        /// Expected tensor role.
        expected: &'static str,
        /// Actual tensor role.
        actual: &'static str,
    },
    /// A group-to-optimizer assignment was structurally inconsistent.
    #[error("state dict group `{group_id}` has invalid optimizer assignment: {message}")]
    InvalidGroupAssignment {
        /// Stable group identifier.
        group_id: String,
        /// Human-readable reason.
        message: String,
    },
    /// The safetensors artifact omitted the embedded Psionic manifest.
    #[error("safetensors artifact is missing embedded Psionic model-IO manifest")]
    MissingSafetensorsManifest,
    /// One tensor listed in the embedded manifest was missing from the safetensors payload.
    #[error("safetensors artifact is missing tensor `{state_key}` declared by the manifest")]
    MissingSafetensorsTensor {
        /// Stable tensor key.
        state_key: String,
    },
    /// One tensor shape in the safetensors payload mismatched the manifest spec.
    #[error(
        "safetensors tensor `{state_key}` shape mismatch: expected {expected:?}, found {actual:?}"
    )]
    SafetensorsShapeMismatch {
        /// Stable tensor key.
        state_key: String,
        /// Expected logical shape.
        expected: Vec<usize>,
        /// Actual shape surfaced by the artifact.
        actual: Vec<usize>,
    },
    /// One tensor dtype in the safetensors payload mismatched the manifest spec.
    #[error(
        "safetensors tensor `{state_key}` dtype mismatch: expected `{expected:?}`, found `{actual}`"
    )]
    SafetensorsDTypeMismatch {
        /// Stable tensor key.
        state_key: String,
        /// Expected dtype.
        expected: DType,
        /// Actual safetensors dtype.
        actual: String,
    },
    /// One explicit import include key did not exist in the source artifact.
    #[error("state-dict import include key `{state_key}` is missing from the source artifact")]
    MissingImportSelectionTensor {
        /// Stable source tensor key.
        state_key: String,
    },
    /// One key remap referenced a tensor that does not exist in the source artifact.
    #[error("state-dict import remap source `{state_key}` is missing from the source artifact")]
    MissingImportRemapSource {
        /// Stable source tensor key.
        state_key: String,
    },
    /// Two selected source tensors remapped to the same target key.
    #[error(
        "state-dict import remap collision at target `{target_state_key}` between source tensors `{first_source_state_key}` and `{second_source_state_key}`"
    )]
    ImportRemapCollision {
        /// Colliding target tensor key.
        target_state_key: String,
        /// First source tensor key.
        first_source_state_key: String,
        /// Second source tensor key.
        second_source_state_key: String,
    },
    /// One selection dropped part of a training-group assignment while keeping another part.
    #[error(
        "state-dict import selection for group `{group_id}` is incomplete; missing source tensors {missing_state_keys:?}"
    )]
    IncompleteImportGroupSelection {
        /// Stable training-group identifier.
        group_id: String,
        /// Source tensor keys required to keep the group structurally valid.
        missing_state_keys: Vec<String>,
    },
    /// The current adapter operation requires dense `f32` parameter tensors.
    #[error("state dict tensor `{state_key}` must be dense `f32` for this operation")]
    DenseF32Required {
        /// Stable tensor key.
        state_key: String,
    },
    /// The requested adapter was derived against a different base state dict.
    #[error("adapter `{adapter_id}` expects base state dict `{expected}`, found `{actual}`")]
    AdapterBaseDigestMismatch {
        /// Stable adapter identifier.
        adapter_id: String,
        /// Expected state-dict digest.
        expected: String,
        /// Actual state-dict digest.
        actual: String,
    },
    /// The requested adapter removal was attempted against the wrong target state dict.
    #[error("adapter `{adapter_id}` expects target state dict `{expected}`, found `{actual}`")]
    AdapterTargetDigestMismatch {
        /// Stable adapter identifier.
        adapter_id: String,
        /// Expected state-dict digest.
        expected: String,
        /// Actual state-dict digest.
        actual: String,
    },
    /// A state-dict pair used for adapter derivation did not expose the same parameter keys.
    #[error("adapter derivation requires matching parameter tensor keys")]
    AdapterKeySetMismatch,
    /// The serialized artifact body could not be encoded or decoded.
    #[error("{context}: {message}")]
    Serialization {
        /// Which serialization path failed.
        context: &'static str,
        /// Human-readable reason.
        message: String,
    },
    /// The GGUF import referenced a tensor that the loader did not return.
    #[error("GGUF import is missing tensor `{state_key}` from the loaded bundle")]
    MissingGgufTensor {
        /// Stable tensor key.
        state_key: String,
    },
    /// One numeric conversion required by the token binding overflowed.
    #[error("tokenizer field `{field}` does not fit in the current portable binding")]
    TokenValueOverflow {
        /// Stable field label.
        field: &'static str,
    },
    /// One lower-layer model-load operation failed.
    #[error(transparent)]
    ModelLoad(#[from] ModelLoadError),
    /// One lower-layer training-core operation failed.
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
}

/// Artifact or portability surface owned by the model-IO layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactFormat {
    /// Native in-memory Psionic state-dict ownership.
    PsionicStateDict,
    /// Dense safetensors payload with embedded Psionic manifest metadata.
    Safetensors,
    /// JSON-encoded torch-style state-dict compatibility artifact.
    TorchStateDictJson,
    /// Imported GGUF artifact surfaced as a typed portable bundle.
    Gguf,
}

/// Explicit interoperability surface tracked by the portable model-IO layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelInteropSurface {
    /// Psionic-native typed state ownership.
    PsionicStateDict,
    /// Dense safetensors carrying the embedded Psionic manifest metadata.
    PsionicManifestSafetensors,
    /// JSON torch-style state-dict compatibility artifact.
    TorchStateDictJson,
    /// GGUF import path into portable state.
    Gguf,
    /// Opaque Python pickle or `.pt` checkpoint payloads.
    TorchPickle,
    /// Other historical opaque checkpoint archives.
    LegacyOpaqueCheckpoint,
}

/// Boundary status for one interop surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelInteropStatus {
    /// The portable layer directly supports the boundary.
    Supported,
    /// The portable layer explicitly does not support the boundary.
    Unsupported,
}

/// Compatibility contract for one interop surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInteropSurfaceCompatibility {
    /// Surface being described.
    pub surface: ModelInteropSurface,
    /// Whether the surface supports import into Psionic portable state.
    pub import_status: ModelInteropStatus,
    /// Plain-language import boundary explanation.
    pub import_detail: String,
    /// Whether the surface supports export from Psionic portable state.
    pub export_status: ModelInteropStatus,
    /// Plain-language export boundary explanation.
    pub export_detail: String,
    /// Whether one bundle can roundtrip through the surface without ad hoc glue.
    pub roundtrip_status: ModelInteropStatus,
    /// Plain-language roundtrip boundary explanation.
    pub roundtrip_detail: String,
}

/// Machine-readable compatibility statement for one portable bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIoCompatibilityContract {
    /// Stable model family label.
    pub model_family: String,
    /// Stable model revision.
    pub revision: String,
    /// Stable state-dict digest this contract describes.
    pub state_dict_digest: String,
    /// Surface-by-surface compatibility boundary statements.
    pub surfaces: Vec<ModelInteropSurfaceCompatibility>,
    /// Stable digest over the compatibility contract contents.
    pub contract_digest: String,
}

impl ModelIoCompatibilityContract {
    fn new(
        model_family: impl Into<String>,
        revision: impl Into<String>,
        state_dict_digest: impl Into<String>,
        surfaces: Vec<ModelInteropSurfaceCompatibility>,
    ) -> Self {
        let model_family = model_family.into();
        let revision = revision.into();
        let state_dict_digest = state_dict_digest.into();
        let contract_digest = stable_model_io_compatibility_digest(
            model_family.as_str(),
            revision.as_str(),
            state_dict_digest.as_str(),
            surfaces.as_slice(),
        );
        Self {
            model_family,
            revision,
            state_dict_digest,
            surfaces,
            contract_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("model_family={}", self.model_family),
            format!("revision={}", self.revision),
            format!("state_dict_digest={}", self.state_dict_digest),
            format!("contract_digest={}", self.contract_digest),
        ];
        for surface in &self.surfaces {
            lines.push(format!(
                "{:?}|import={:?}|export={:?}|roundtrip={:?}",
                surface.surface,
                surface.import_status,
                surface.export_status,
                surface.roundtrip_status
            ));
        }
        lines
    }
}

/// Tokenizer asset family tracked by the portable bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortableTokenizerAssetFormat {
    /// A generic tokenizer JSON or vocabulary package.
    TokenizerJson,
    /// Tokenizer facts carried by a GGUF artifact.
    GgufMetadata,
    /// A SentencePiece or unigram model blob.
    SentencePieceModel,
    /// A Rust-native digest-only tokenizer record.
    PsionicDigest,
}

/// Tokenizer contract bound to one portable checkpoint or model bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableTokenizerBinding {
    /// Canonical tokenizer digest contract.
    pub digest: TokenizerDigest,
    /// How the tokenizer asset is packaged.
    pub asset_format: PortableTokenizerAssetFormat,
    /// Stable tokenizer asset version or revision binding.
    pub asset_version: String,
    /// Whether callers should inject BOS by default.
    pub add_bos: bool,
    /// Whether callers should inject EOS by default.
    pub add_eos: bool,
    /// Optional BOS token ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bos_token_id: Option<u32>,
    /// Ordered EOS token IDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eos_token_ids: Vec<u32>,
    /// Optional PAD token ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad_token_id: Option<u32>,
    /// Optional unknown-token ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown_token_id: Option<u32>,
}

/// Machine-readable promoted model profile contract carried by one portable
/// bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableModelProfileContract {
    /// Stable promoted profile id.
    pub profile_id: String,
    /// Stable shared family id.
    pub family_id: String,
    /// Stable baseline model id for the promoted family.
    pub baseline_model_id: String,
    /// Stable baseline revision for the promoted family.
    pub baseline_revision: String,
    /// Stable profile kind label such as `general_psion_small_decoder`.
    pub profile_kind: String,
    /// Shared capabilities admitted across all profiles for the family.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shared_capabilities: BTreeMap<String, bool>,
    /// Challenge-only overlay requirements for this profile.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overlay_requirements: BTreeMap<String, bool>,
}

impl PortableTokenizerBinding {
    /// Creates a tokenizer binding from an existing canonical tokenizer digest.
    #[must_use]
    pub fn new(
        digest: TokenizerDigest,
        asset_format: PortableTokenizerAssetFormat,
        asset_version: impl Into<String>,
    ) -> Self {
        Self {
            digest,
            asset_format,
            asset_version: asset_version.into(),
            add_bos: false,
            add_eos: false,
            bos_token_id: None,
            eos_token_ids: Vec::new(),
            pad_token_id: None,
            unknown_token_id: None,
        }
    }

    /// Attaches special-token behavior facts to the tokenizer binding.
    #[must_use]
    pub fn with_special_tokens(
        mut self,
        bos_token_id: Option<u32>,
        eos_token_ids: Vec<u32>,
        pad_token_id: Option<u32>,
        unknown_token_id: Option<u32>,
        add_bos: bool,
        add_eos: bool,
    ) -> Self {
        self.bos_token_id = bos_token_id;
        self.eos_token_ids = eos_token_ids;
        self.pad_token_id = pad_token_id;
        self.unknown_token_id = unknown_token_id;
        self.add_bos = add_bos;
        self.add_eos = add_eos;
        self
    }

    /// Returns the stable digest over the bound tokenizer contract.
    #[must_use]
    pub fn contract_digest(&self) -> String {
        self.digest.stable_digest()
    }

    /// Builds a tokenizer binding from GGUF tokenizer metadata and an asset version.
    pub fn from_gguf(
        tokenizer: &GgufTokenizerMetadata,
        asset_version: impl Into<String>,
        template_digest: Option<String>,
    ) -> Result<Self, ModelIoError> {
        let family = tokenizer_family_from_gguf(tokenizer.model);
        let vocab_size = u32::try_from(tokenizer.vocabulary.tokens().len()).map_err(|_| {
            ModelIoError::TokenValueOverflow {
                field: "tokenizer.vocab_size",
            }
        })?;
        let mut digest = TokenizerDigest::new(family, tokenizer.digest().to_string(), vocab_size)
            .with_special_tokens_digest(digest_tokenizer_specials(tokenizer));
        if let Some(template_digest) = template_digest.clone() {
            digest = digest.with_template_digest(template_digest);
        }
        Ok(Self {
            digest,
            asset_format: PortableTokenizerAssetFormat::GgufMetadata,
            asset_version: asset_version.into(),
            add_bos: tokenizer.add_bos,
            add_eos: tokenizer.add_eos,
            bos_token_id: tokenizer
                .vocabulary
                .bos_token_id()
                .map(|value| value.as_u32()),
            eos_token_ids: tokenizer
                .vocabulary
                .eos_token_ids()
                .iter()
                .map(|value| value.as_u32())
                .collect(),
            pad_token_id: tokenizer
                .vocabulary
                .pad_token_id()
                .map(|value| value.as_u32()),
            unknown_token_id: tokenizer
                .vocabulary
                .unknown_token_id()
                .map(|value| value.as_u32()),
        })
    }
}

/// Meaning of one state-dict tensor entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStateTensorRole {
    /// Train-visible model parameter tensor.
    Parameter,
    /// Momentum buffer for momentum-bearing local optimizers.
    SgdMomentumBuffer,
    /// Adam-family first moment.
    AdamFirstMoment,
    /// Adam-family second moment.
    AdamSecondMoment,
}

impl ModelStateTensorRole {
    const fn label(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::SgdMomentumBuffer => "sgd_momentum_buffer",
            Self::AdamFirstMoment => "adam_first_moment",
            Self::AdamSecondMoment => "adam_second_moment",
        }
    }
}

/// Traversal and assignment facts for one tensor key inside a portable state dict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelStateTensorManifest {
    /// Stable state-dict key.
    pub state_key: String,
    /// Logical Rust model-tree path targeted by the state assignment.
    pub model_tree_path: Vec<String>,
    /// Meaning of the tensor inside train or serve flows.
    pub role: ModelStateTensorRole,
    /// Tensor spec required by the state assignment.
    pub spec: TensorSpec,
}

impl ModelStateTensorManifest {
    fn new(
        state_key: impl Into<String>,
        model_tree_path: Vec<String>,
        role: ModelStateTensorRole,
        spec: TensorSpec,
    ) -> Self {
        Self {
            state_key: state_key.into(),
            model_tree_path,
            role,
            spec,
        }
    }
}

/// Group-level assignment back into the training core's parameter-group model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelStateGroupAssignment {
    /// Stable group identifier from the training core.
    pub group_id: String,
    /// High-level parameter-group family.
    pub class: TrainingParameterClass,
    /// Logical Rust model-tree path for the group.
    pub model_tree_path: Vec<String>,
    /// Full optimizer configuration for the group.
    pub optimizer: TrainingOptimizerConfig,
    /// Group-level learning-rate and weight-decay scaling semantics.
    #[serde(default)]
    pub parameter_semantics: TrainingParameterGroupSemantics,
    /// Optional scheduler config plus current state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<TrainingSchedulerBinding>,
    /// Preferred residency posture for the optimizer state.
    pub optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
    /// Current residency posture.
    pub optimizer_residency: OptimizerStateResidency,
    /// Number of updates already applied to the group.
    pub applied_steps: u64,
    /// Tensor key carrying the train-visible parameter values.
    pub parameter_key: String,
    /// Tensor key carrying the SGD momentum buffer when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub momentum_buffer_key: Option<String>,
    /// Tensor key carrying the Adam-family first moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_moment_key: Option<String>,
    /// Tensor key carrying the Adam-family second moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second_moment_key: Option<String>,
}

/// One portable state-dict tensor with both manifest and payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelStateTensorEntry {
    /// Traversal and assignment metadata.
    pub manifest: ModelStateTensorManifest,
    /// Typed tensor payload.
    pub data: TensorData,
}

/// Tensor-only manifest form embedded in safetensors metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableModelStateDictManifest {
    /// Stable model family label.
    pub model_family: String,
    /// Stable model revision.
    pub revision: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Optional checkpoint reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    /// Artifact surface that introduced the state dict into portability space.
    pub source_format: ModelArtifactFormat,
    /// Group-level assignment contracts.
    pub groups: Vec<ModelStateGroupAssignment>,
    /// Tensor-level traversal records.
    pub tensors: BTreeMap<String, ModelStateTensorManifest>,
    /// Stable digest over model-tree assignment and tensor payloads.
    pub digest: String,
}

/// Portable, inspectable model or checkpoint state dict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableModelStateDict {
    /// Stable model family label.
    pub model_family: String,
    /// Stable model revision.
    pub revision: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Optional checkpoint reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    /// Artifact surface that introduced the state dict into portability space.
    pub source_format: ModelArtifactFormat,
    /// Group-level assignment contracts for the training core.
    pub groups: Vec<ModelStateGroupAssignment>,
    /// Tensor payloads keyed by stable state-dict name.
    pub tensors: BTreeMap<String, ModelStateTensorEntry>,
    /// Stable digest over model-tree assignment and tensor payloads.
    pub digest: String,
}

impl PortableModelStateDict {
    /// Creates and validates a portable state dict.
    pub fn new(
        model_family: impl Into<String>,
        revision: impl Into<String>,
        checkpoint_family: impl Into<String>,
        checkpoint_ref: Option<String>,
        source_format: ModelArtifactFormat,
        groups: Vec<ModelStateGroupAssignment>,
        tensors: BTreeMap<String, ModelStateTensorEntry>,
    ) -> Result<Self, ModelIoError> {
        let model_family = model_family.into();
        let revision = revision.into();
        let checkpoint_family = checkpoint_family.into();
        validate_state_tensors(&tensors)?;
        validate_state_groups(&groups, &tensors)?;
        let digest = digest_state_dict(
            model_family.as_str(),
            revision.as_str(),
            checkpoint_family.as_str(),
            checkpoint_ref.as_deref(),
            &groups,
            &tensors,
        );
        Ok(Self {
            model_family,
            revision,
            checkpoint_family,
            checkpoint_ref,
            source_format,
            groups,
            tensors,
            digest,
        })
    }

    /// Builds a portable state dict directly from training-core group state.
    pub fn from_training_groups(
        model_family: impl Into<String>,
        revision: impl Into<String>,
        checkpoint_family: impl Into<String>,
        checkpoint_ref: Option<String>,
        groups: &[TrainingParameterGroupState],
    ) -> Result<Self, ModelIoError> {
        let mut tensors = BTreeMap::new();
        let mut assignments = Vec::with_capacity(groups.len());

        for group in groups {
            let group_path = split_model_tree_path(group.group_id.as_str());
            let parameter_key = format!("model.{}.parameter", group.group_id);
            insert_state_entry(
                &mut tensors,
                parameter_key.clone(),
                ModelStateTensorEntry {
                    manifest: ModelStateTensorManifest::new(
                        parameter_key.clone(),
                        extend_tree_path(group_path.clone(), "parameter"),
                        ModelStateTensorRole::Parameter,
                        group.parameter.spec.clone(),
                    ),
                    data: group.parameter.data.clone(),
                },
            )?;

            let mut assignment = ModelStateGroupAssignment {
                group_id: group.group_id.clone(),
                class: group.class,
                model_tree_path: group_path,
                optimizer: group.optimizer.clone(),
                parameter_semantics: group.parameter_semantics,
                scheduler: group.scheduler.clone(),
                optimizer_residency_policy: group.optimizer_residency_policy,
                optimizer_residency: group.optimizer_residency,
                applied_steps: group.applied_steps,
                parameter_key,
                momentum_buffer_key: None,
                first_moment_key: None,
                second_moment_key: None,
            };

            match &group.optimizer_state {
                TrainingOptimizerState::Sgd { momentum_buffer }
                | TrainingOptimizerState::Lars { momentum_buffer } => {
                    if let Some(momentum_buffer) = momentum_buffer {
                        let state_key = format!("optimizer.{}.momentum_buffer", group.group_id);
                        insert_state_entry(
                            &mut tensors,
                            state_key.clone(),
                            ModelStateTensorEntry {
                                manifest: ModelStateTensorManifest::new(
                                    state_key.clone(),
                                    extend_tree_path(
                                        assignment.model_tree_path.clone(),
                                        "momentum_buffer",
                                    ),
                                    ModelStateTensorRole::SgdMomentumBuffer,
                                    group.parameter.spec.clone(),
                                ),
                                data: TensorData::F32(momentum_buffer.clone()),
                            },
                        )?;
                        assignment.momentum_buffer_key = Some(state_key);
                    }
                }
                TrainingOptimizerState::Adam {
                    first_moment,
                    second_moment,
                }
                | TrainingOptimizerState::AdamW {
                    first_moment,
                    second_moment,
                }
                | TrainingOptimizerState::Lamb {
                    first_moment,
                    second_moment,
                } => {
                    let first_moment_key = format!("optimizer.{}.first_moment", group.group_id);
                    insert_state_entry(
                        &mut tensors,
                        first_moment_key.clone(),
                        ModelStateTensorEntry {
                            manifest: ModelStateTensorManifest::new(
                                first_moment_key.clone(),
                                extend_tree_path(
                                    assignment.model_tree_path.clone(),
                                    "first_moment",
                                ),
                                ModelStateTensorRole::AdamFirstMoment,
                                group.parameter.spec.clone(),
                            ),
                            data: TensorData::F32(first_moment.clone()),
                        },
                    )?;

                    let second_moment_key = format!("optimizer.{}.second_moment", group.group_id);
                    insert_state_entry(
                        &mut tensors,
                        second_moment_key.clone(),
                        ModelStateTensorEntry {
                            manifest: ModelStateTensorManifest::new(
                                second_moment_key.clone(),
                                extend_tree_path(
                                    assignment.model_tree_path.clone(),
                                    "second_moment",
                                ),
                                ModelStateTensorRole::AdamSecondMoment,
                                group.parameter.spec.clone(),
                            ),
                            data: TensorData::F32(second_moment.clone()),
                        },
                    )?;

                    assignment.first_moment_key = Some(first_moment_key);
                    assignment.second_moment_key = Some(second_moment_key);
                }
            }

            assignments.push(assignment);
        }

        Self::new(
            model_family,
            revision,
            checkpoint_family,
            checkpoint_ref,
            ModelArtifactFormat::PsionicStateDict,
            assignments,
            tensors,
        )
    }

    /// Returns a manifest form that omits raw tensor payloads.
    #[must_use]
    pub fn manifest(&self) -> PortableModelStateDictManifest {
        PortableModelStateDictManifest {
            model_family: self.model_family.clone(),
            revision: self.revision.clone(),
            checkpoint_family: self.checkpoint_family.clone(),
            checkpoint_ref: self.checkpoint_ref.clone(),
            source_format: self.source_format,
            groups: self.groups.clone(),
            tensors: self
                .tensors
                .iter()
                .map(|(key, entry)| (key.clone(), entry.manifest.clone()))
                .collect(),
            digest: self.digest.clone(),
        }
    }

    /// Returns ordered traversal records for all tensor entries.
    #[must_use]
    pub fn traversal_records(&self) -> Vec<ModelStateTensorManifest> {
        self.tensors
            .values()
            .map(|entry| entry.manifest.clone())
            .collect()
    }

    /// Reconstructs training-core group state from the portable state dict.
    pub fn to_training_groups(&self) -> Result<Vec<TrainingParameterGroupState>, ModelIoError> {
        let mut groups = Vec::with_capacity(self.groups.len());
        for assignment in &self.groups {
            let parameter = training_buffer_from_state_entry(
                assignment.parameter_key.as_str(),
                self.tensors
                    .get(assignment.parameter_key.as_str())
                    .ok_or_else(|| ModelIoError::MissingTensor {
                        state_key: assignment.parameter_key.clone(),
                    })?,
            )?;

            let mut optimizer_state = assignment
                .optimizer
                .initialize_state(parameter.spec.storage_size());
            match &mut optimizer_state {
                TrainingOptimizerState::Sgd { momentum_buffer }
                | TrainingOptimizerState::Lars { momentum_buffer } => {
                    *momentum_buffer = assignment
                        .momentum_buffer_key
                        .as_ref()
                        .map(|state_key| {
                            dense_f32_values(
                                state_key.as_str(),
                                self.tensors.get(state_key.as_str()).ok_or_else(|| {
                                    ModelIoError::MissingTensor {
                                        state_key: state_key.clone(),
                                    }
                                })?,
                            )
                            .map(ToOwned::to_owned)
                        })
                        .transpose()?;
                }
                TrainingOptimizerState::Adam {
                    first_moment,
                    second_moment,
                }
                | TrainingOptimizerState::AdamW {
                    first_moment,
                    second_moment,
                }
                | TrainingOptimizerState::Lamb {
                    first_moment,
                    second_moment,
                } => {
                    let first_moment_key =
                        assignment.first_moment_key.as_ref().ok_or_else(|| {
                            ModelIoError::InvalidGroupAssignment {
                                group_id: assignment.group_id.clone(),
                                message: format!(
                                    "{:?} group is missing `first_moment_key`",
                                    assignment.optimizer.kind
                                ),
                            }
                        })?;
                    let second_moment_key =
                        assignment.second_moment_key.as_ref().ok_or_else(|| {
                            ModelIoError::InvalidGroupAssignment {
                                group_id: assignment.group_id.clone(),
                                message: format!(
                                    "{:?} group is missing `second_moment_key`",
                                    assignment.optimizer.kind
                                ),
                            }
                        })?;
                    *first_moment = dense_f32_values(
                        first_moment_key.as_str(),
                        self.tensors.get(first_moment_key.as_str()).ok_or_else(|| {
                            ModelIoError::MissingTensor {
                                state_key: first_moment_key.clone(),
                            }
                        })?,
                    )?
                    .to_vec();
                    *second_moment = dense_f32_values(
                        second_moment_key.as_str(),
                        self.tensors
                            .get(second_moment_key.as_str())
                            .ok_or_else(|| ModelIoError::MissingTensor {
                                state_key: second_moment_key.clone(),
                            })?,
                    )?
                    .to_vec();
                }
            }

            groups.push(TrainingParameterGroupState {
                group_id: assignment.group_id.clone(),
                class: assignment.class,
                parameter,
                optimizer: assignment.optimizer.clone(),
                parameter_semantics: assignment.parameter_semantics,
                scheduler: assignment.scheduler.clone(),
                optimizer_state,
                optimizer_residency_policy: assignment.optimizer_residency_policy,
                optimizer_residency: assignment.optimizer_residency,
                accelerated_master_weights: None,
                applied_steps: assignment.applied_steps,
            });
        }
        Ok(groups)
    }

    /// Derives a portable additive adapter from one base and one tuned state dict.
    pub fn derive_adapter_delta(
        base: &Self,
        tuned: &Self,
        adapter_id: impl Into<String>,
    ) -> Result<ModelAdapterDelta, ModelIoError> {
        let base_keys = base.parameter_keys();
        let tuned_keys = tuned.parameter_keys();
        if base_keys != tuned_keys {
            return Err(ModelIoError::AdapterKeySetMismatch);
        }

        let mut tensors = BTreeMap::new();
        for key in base_keys {
            let base_entry =
                base.tensors
                    .get(key.as_str())
                    .ok_or_else(|| ModelIoError::MissingTensor {
                        state_key: key.clone(),
                    })?;
            let tuned_entry =
                tuned
                    .tensors
                    .get(key.as_str())
                    .ok_or_else(|| ModelIoError::MissingTensor {
                        state_key: key.clone(),
                    })?;
            if base_entry.manifest.spec != tuned_entry.manifest.spec {
                return Err(ModelIoError::InvalidGroupAssignment {
                    group_id: key.clone(),
                    message: String::from(
                        "parameter tensor specs changed during adapter derivation",
                    ),
                });
            }
            let base_values = dense_f32_values(key.as_str(), base_entry)?;
            let tuned_values = dense_f32_values(key.as_str(), tuned_entry)?;
            let delta_values = base_values
                .iter()
                .zip(tuned_values)
                .map(|(base_value, tuned_value)| tuned_value - base_value)
                .collect::<Vec<_>>();
            tensors.insert(
                key.clone(),
                ModelAdapterDeltaTensor {
                    state_key: key.clone(),
                    model_tree_path: base_entry.manifest.model_tree_path.clone(),
                    spec: base_entry.manifest.spec.clone(),
                    delta_values,
                },
            );
        }

        Ok(ModelAdapterDelta {
            adapter_id: adapter_id.into(),
            base_state_dict_digest: base.digest.clone(),
            target_state_dict_digest: tuned.digest.clone(),
            tensors,
        })
    }

    /// Applies a previously derived adapter delta to the current state dict.
    pub fn apply_adapter_delta(&self, delta: &ModelAdapterDelta) -> Result<Self, ModelIoError> {
        if self.digest != delta.base_state_dict_digest {
            return Err(ModelIoError::AdapterBaseDigestMismatch {
                adapter_id: delta.adapter_id.clone(),
                expected: delta.base_state_dict_digest.clone(),
                actual: self.digest.clone(),
            });
        }

        let mut tensors = self.tensors.clone();
        for (state_key, adapter_tensor) in &delta.tensors {
            let entry =
                tensors
                    .get_mut(state_key.as_str())
                    .ok_or_else(|| ModelIoError::MissingTensor {
                        state_key: state_key.clone(),
                    })?;
            let values = dense_f32_values_mut(state_key.as_str(), entry)?;
            if values.len() != adapter_tensor.delta_values.len() {
                return Err(ModelIoError::TensorPayloadLengthMismatch {
                    state_key: state_key.clone(),
                    expected_len: values.len(),
                    actual_len: adapter_tensor.delta_values.len(),
                });
            }
            for (value, delta_value) in values.iter_mut().zip(&adapter_tensor.delta_values) {
                *value += delta_value;
            }
        }

        let state_dict = Self::new(
            self.model_family.clone(),
            self.revision.clone(),
            self.checkpoint_family.clone(),
            self.checkpoint_ref.clone(),
            self.source_format,
            self.groups.clone(),
            tensors,
        )?;
        if state_dict.digest != delta.target_state_dict_digest {
            return Err(ModelIoError::AdapterTargetDigestMismatch {
                adapter_id: delta.adapter_id.clone(),
                expected: delta.target_state_dict_digest.clone(),
                actual: state_dict.digest,
            });
        }
        Ok(state_dict)
    }

    /// Removes a previously derived adapter delta from the current state dict.
    pub fn remove_adapter_delta(&self, delta: &ModelAdapterDelta) -> Result<Self, ModelIoError> {
        if self.digest != delta.target_state_dict_digest {
            return Err(ModelIoError::AdapterTargetDigestMismatch {
                adapter_id: delta.adapter_id.clone(),
                expected: delta.target_state_dict_digest.clone(),
                actual: self.digest.clone(),
            });
        }

        let mut tensors = self.tensors.clone();
        for (state_key, adapter_tensor) in &delta.tensors {
            let entry =
                tensors
                    .get_mut(state_key.as_str())
                    .ok_or_else(|| ModelIoError::MissingTensor {
                        state_key: state_key.clone(),
                    })?;
            let values = dense_f32_values_mut(state_key.as_str(), entry)?;
            if values.len() != adapter_tensor.delta_values.len() {
                return Err(ModelIoError::TensorPayloadLengthMismatch {
                    state_key: state_key.clone(),
                    expected_len: values.len(),
                    actual_len: adapter_tensor.delta_values.len(),
                });
            }
            for (value, delta_value) in values.iter_mut().zip(&adapter_tensor.delta_values) {
                *value -= delta_value;
            }
        }

        let state_dict = Self::new(
            self.model_family.clone(),
            self.revision.clone(),
            self.checkpoint_family.clone(),
            self.checkpoint_ref.clone(),
            self.source_format,
            self.groups.clone(),
            tensors,
        )?;
        if state_dict.digest != delta.base_state_dict_digest {
            return Err(ModelIoError::AdapterBaseDigestMismatch {
                adapter_id: delta.adapter_id.clone(),
                expected: delta.base_state_dict_digest.clone(),
                actual: state_dict.digest,
            });
        }
        Ok(state_dict)
    }

    fn parameter_keys(&self) -> BTreeSet<String> {
        self.tensors
            .iter()
            .filter_map(|(key, entry)| {
                (entry.manifest.role == ModelStateTensorRole::Parameter).then_some(key.clone())
            })
            .collect()
    }

    /// Builds a filtered and optionally remapped clone of the state dict.
    pub fn select_and_remap(
        &self,
        request: &PortableModelImportRequest,
    ) -> Result<Self, ModelIoError> {
        let source_tensors = self
            .tensors
            .iter()
            .map(|(key, entry)| (key.clone(), entry.manifest.clone()))
            .collect::<BTreeMap<_, _>>();
        let selection =
            select_and_remap_state_dict_manifest(&self.groups, &source_tensors, request)?;

        let mut tensors = BTreeMap::new();
        for binding in selection.tensor_bindings.values() {
            let entry = self
                .tensors
                .get(binding.source_state_key.as_str())
                .ok_or_else(|| ModelIoError::MissingTensor {
                    state_key: binding.source_state_key.clone(),
                })?;
            tensors.insert(
                binding.target_state_key.clone(),
                ModelStateTensorEntry {
                    manifest: binding.manifest.clone(),
                    data: entry.data.clone(),
                },
            );
        }

        Self::new(
            self.model_family.clone(),
            self.revision.clone(),
            self.checkpoint_family.clone(),
            self.checkpoint_ref.clone(),
            self.source_format,
            selection.groups,
            tensors,
        )
    }
}

/// Whether import should eagerly materialize tensor payloads or defer that work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TensorMaterializationPolicy {
    /// Decode tensor payloads immediately during import.
    #[default]
    Eager,
    /// Keep supported tensor payloads deferred until the caller materializes them.
    Deferred,
}

/// Explicit subset-selection policy for state-dict import.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableTensorImportSelection {
    /// Optional explicit include set over source state keys.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub include_state_keys: BTreeSet<String>,
    /// Explicit exclude set over source state keys.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub exclude_state_keys: BTreeSet<String>,
}

impl PortableTensorImportSelection {
    /// Creates an empty selection, which means "include everything".
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one source state key to the include set.
    #[must_use]
    pub fn include(mut self, state_key: impl Into<String>) -> Self {
        self.include_state_keys.insert(state_key.into());
        self
    }

    /// Adds one source state key to the exclude set.
    #[must_use]
    pub fn exclude(mut self, state_key: impl Into<String>) -> Self {
        self.exclude_state_keys.insert(state_key.into());
        self
    }
}

/// Explicit source-to-target state-key remapping for import.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableTensorKeyRemap {
    /// Source-to-target state-key remap table.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_to_target: BTreeMap<String, String>,
}

impl PortableTensorKeyRemap {
    /// Creates an empty remap table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one source-to-target remap entry.
    #[must_use]
    pub fn map(
        mut self,
        source_state_key: impl Into<String>,
        target_state_key: impl Into<String>,
    ) -> Self {
        self.source_to_target
            .insert(source_state_key.into(), target_state_key.into());
        self
    }

    fn target_for<'a>(&'a self, source_state_key: &'a str) -> &'a str {
        self.source_to_target
            .get(source_state_key)
            .map(String::as_str)
            .unwrap_or(source_state_key)
    }
}

/// Complete request for one bounded model-IO import operation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableModelImportRequest {
    /// Tensor-subset selection.
    pub selection: PortableTensorImportSelection,
    /// Optional source-to-target state-key remap.
    pub key_remap: PortableTensorKeyRemap,
    /// Eager versus deferred tensor materialization policy.
    pub materialization_policy: TensorMaterializationPolicy,
}

impl PortableModelImportRequest {
    /// Creates the default "import everything eagerly with no remap" request.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the tensor selection.
    #[must_use]
    pub fn with_selection(mut self, selection: PortableTensorImportSelection) -> Self {
        self.selection = selection;
        self
    }

    /// Replaces the key-remap table.
    #[must_use]
    pub fn with_key_remap(mut self, key_remap: PortableTensorKeyRemap) -> Self {
        self.key_remap = key_remap;
        self
    }

    /// Sets the tensor materialization policy.
    #[must_use]
    pub fn with_materialization_policy(
        mut self,
        materialization_policy: TensorMaterializationPolicy,
    ) -> Self {
        self.materialization_policy = materialization_policy;
        self
    }
}

/// Deferred tensor payload owned by an import plan.
#[derive(Clone, Debug)]
pub enum DeferredTensorData {
    /// Dense `f32` bytes stored as little-endian raw payload.
    DenseF32Bytes {
        /// Owned artifact bytes retained by the import plan.
        bytes: Arc<[u8]>,
        /// Byte range carrying the tensor payload.
        byte_range: std::ops::Range<usize>,
    },
}

impl DeferredTensorData {
    fn materialize(&self, state_key: &str) -> Result<TensorData, ModelIoError> {
        match self {
            Self::DenseF32Bytes { bytes, byte_range } => {
                let values = decode_f32_bytes(state_key, &bytes[byte_range.clone()])?;
                Ok(TensorData::F32(values))
            }
        }
    }

    fn payload_digest(&self) -> String {
        match self {
            Self::DenseF32Bytes { bytes, byte_range } => {
                tensor_payload_digest_from_raw_f32_bytes(&bytes[byte_range.clone()])
            }
        }
    }
}

/// Materialized or deferred tensor payload tracked by an import plan.
#[derive(Clone, Debug)]
pub enum PortableImportedTensorPayload {
    /// Tensor payload is already materialized.
    Materialized(TensorData),
    /// Tensor payload is deferred until the caller requests materialization.
    Deferred(DeferredTensorData),
}

impl PortableImportedTensorPayload {
    /// Returns whether the payload is currently deferred.
    #[must_use]
    pub const fn is_deferred(&self) -> bool {
        matches!(self, Self::Deferred(_))
    }

    fn materialize(&self, state_key: &str) -> Result<TensorData, ModelIoError> {
        match self {
            Self::Materialized(data) => Ok(data.clone()),
            Self::Deferred(data) => data.materialize(state_key),
        }
    }

    fn payload_digest(&self) -> String {
        match self {
            Self::Materialized(data) => tensor_payload_digest(data),
            Self::Deferred(data) => data.payload_digest(),
        }
    }
}

/// One admitted tensor inside a model import plan.
#[derive(Clone, Debug)]
pub struct PortableImportedTensorEntry {
    /// Source state key in the origin artifact.
    pub source_state_key: String,
    /// Target manifest inside the admitted plan.
    pub manifest: ModelStateTensorManifest,
    /// Materialized or deferred payload.
    pub payload: PortableImportedTensorPayload,
}

/// One admitted bundle import plan, optionally carrying deferred tensor payloads.
#[derive(Clone, Debug)]
pub struct PortableModelBundleImportPlan {
    /// Artifact surface the plan was built from.
    pub source_format: ModelArtifactFormat,
    /// Materialization policy used by the plan.
    pub materialization_policy: TensorMaterializationPolicy,
    /// Metadata-only manifest for the admitted bundle.
    pub manifest: PortableModelBundleManifest,
    /// Admitted tensor entries keyed by target state key.
    pub tensors: BTreeMap<String, PortableImportedTensorEntry>,
    /// Stable digest over the admitted import plan.
    pub plan_digest: String,
}

impl PortableModelBundleImportPlan {
    /// Returns the number of admitted tensors.
    #[must_use]
    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }

    /// Returns the number of deferred tensors still owned by the plan.
    #[must_use]
    pub fn deferred_tensor_count(&self) -> usize {
        self.tensors
            .values()
            .filter(|entry| entry.payload.is_deferred())
            .count()
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("source_format={:?}", self.source_format),
            format!("materialization_policy={:?}", self.materialization_policy),
            format!("plan_digest={}", self.plan_digest),
            format!("state_dict_digest={}", self.manifest.state_dict.digest),
        ];
        for (state_key, entry) in &self.tensors {
            lines.push(format!(
                "{state_key}|source={}|deferred={}",
                entry.source_state_key,
                entry.payload.is_deferred()
            ));
        }
        lines
    }

    /// Materializes one admitted tensor by target state key.
    pub fn materialize_tensor(&self, state_key: &str) -> Result<TensorData, ModelIoError> {
        let entry = self
            .tensors
            .get(state_key)
            .ok_or_else(|| ModelIoError::MissingTensor {
                state_key: String::from(state_key),
            })?;
        entry.payload.materialize(state_key)
    }

    /// Materializes the full admitted bundle.
    pub fn materialize_bundle(&self) -> Result<PortableModelBundle, ModelIoError> {
        let mut tensors = BTreeMap::new();
        for (state_key, entry) in &self.tensors {
            tensors.insert(
                state_key.clone(),
                ModelStateTensorEntry {
                    manifest: entry.manifest.clone(),
                    data: entry.payload.materialize(state_key.as_str())?,
                },
            );
        }

        let state_dict = PortableModelStateDict::new(
            self.manifest.state_dict.model_family.clone(),
            self.manifest.state_dict.revision.clone(),
            self.manifest.state_dict.checkpoint_family.clone(),
            self.manifest.state_dict.checkpoint_ref.clone(),
            self.source_format,
            self.manifest.state_dict.groups.clone(),
            tensors,
        )?;

        Ok(PortableModelBundle {
            state_dict,
            tokenizer: self.manifest.tokenizer.clone(),
            profile_contract: self.manifest.profile_contract.clone(),
            chat_template_digest: self.manifest.chat_template_digest.clone(),
            preferred_serving_formats: self.manifest.preferred_serving_formats.clone(),
        })
    }
}

/// Metadata-only form embedded inside safetensors artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableModelBundleManifest {
    /// State-dict manifest without raw payloads.
    pub state_dict: PortableModelStateDictManifest,
    /// Bound tokenizer portability contract.
    pub tokenizer: PortableTokenizerBinding,
    /// Optional promoted-family profile contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_contract: Option<PortableModelProfileContract>,
    /// Optional chat-template digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_digest: Option<String>,
    /// Preferred downstream serving or portability surfaces.
    pub preferred_serving_formats: Vec<ModelArtifactFormat>,
}

/// Complete portable bundle carrying state, tokenizer, and preferred serve surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableModelBundle {
    /// Portable state dict.
    pub state_dict: PortableModelStateDict,
    /// Bound tokenizer contract.
    pub tokenizer: PortableTokenizerBinding,
    /// Optional promoted-family profile contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_contract: Option<PortableModelProfileContract>,
    /// Optional chat-template digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_digest: Option<String>,
    /// Preferred downstream serving or portability surfaces.
    pub preferred_serving_formats: Vec<ModelArtifactFormat>,
}

impl PortableModelBundle {
    /// Creates a portable bundle from training-core state.
    pub fn from_training_groups(
        model_family: impl Into<String>,
        revision: impl Into<String>,
        checkpoint_family: impl Into<String>,
        checkpoint_ref: Option<String>,
        groups: &[TrainingParameterGroupState],
        tokenizer: PortableTokenizerBinding,
        chat_template_digest: Option<String>,
    ) -> Result<Self, ModelIoError> {
        Ok(Self {
            state_dict: PortableModelStateDict::from_training_groups(
                model_family,
                revision,
                checkpoint_family,
                checkpoint_ref,
                groups,
            )?,
            tokenizer,
            profile_contract: None,
            chat_template_digest,
            preferred_serving_formats: vec![
                ModelArtifactFormat::Safetensors,
                ModelArtifactFormat::TorchStateDictJson,
            ],
        })
    }

    /// Returns the metadata-only manifest used by safetensors export.
    #[must_use]
    pub fn manifest(&self) -> PortableModelBundleManifest {
        PortableModelBundleManifest {
            state_dict: self.state_dict.manifest(),
            tokenizer: self.tokenizer.clone(),
            profile_contract: self.profile_contract.clone(),
            chat_template_digest: self.chat_template_digest.clone(),
            preferred_serving_formats: self.preferred_serving_formats.clone(),
        }
    }

    /// Attaches one promoted-family profile contract to the bundle.
    #[must_use]
    pub fn with_profile_contract(mut self, profile_contract: PortableModelProfileContract) -> Self {
        self.profile_contract = Some(profile_contract);
        self
    }

    /// Reconstructs training-core groups from the bundle.
    pub fn to_training_groups(&self) -> Result<Vec<TrainingParameterGroupState>, ModelIoError> {
        self.state_dict.to_training_groups()
    }

    /// Returns the explicit compatibility boundary contract for this bundle.
    #[must_use]
    pub fn compatibility_contract(&self) -> ModelIoCompatibilityContract {
        ModelIoCompatibilityContract::new(
            self.state_dict.model_family.clone(),
            self.state_dict.revision.clone(),
            self.state_dict.digest.clone(),
            vec![
                ModelInteropSurfaceCompatibility {
                    surface: ModelInteropSurface::PsionicStateDict,
                    import_status: ModelInteropStatus::Supported,
                    import_detail: String::from(
                        "native typed Psionic state-dict ownership is the canonical import surface",
                    ),
                    export_status: ModelInteropStatus::Supported,
                    export_detail: String::from(
                        "native typed Psionic state-dict ownership is the canonical export surface",
                    ),
                    roundtrip_status: ModelInteropStatus::Supported,
                    roundtrip_detail: String::from(
                        "Psionic-native state dicts roundtrip without format conversion",
                    ),
                },
                safetensors_surface_compatibility(&self.state_dict),
                ModelInteropSurfaceCompatibility {
                    surface: ModelInteropSurface::TorchStateDictJson,
                    import_status: ModelInteropStatus::Supported,
                    import_detail: String::from(
                        "supported as a typed JSON state-dict compatibility shell rather than Python pickle",
                    ),
                    export_status: ModelInteropStatus::Supported,
                    export_detail: String::from(
                        "supported as a typed JSON state-dict compatibility shell rather than Python pickle",
                    ),
                    roundtrip_status: ModelInteropStatus::Supported,
                    roundtrip_detail: String::from(
                        "typed JSON compatibility artifacts can roundtrip through the portable bundle",
                    ),
                },
                ModelInteropSurfaceCompatibility {
                    surface: ModelInteropSurface::Gguf,
                    import_status: ModelInteropStatus::Supported,
                    import_detail: String::from(
                        "supported through the explicit GGUF import path into portable Psionic state",
                    ),
                    export_status: ModelInteropStatus::Unsupported,
                    export_detail: String::from(
                        "GGUF export is intentionally unsupported in the current boundary",
                    ),
                    roundtrip_status: ModelInteropStatus::Unsupported,
                    roundtrip_detail: String::from(
                        "GGUF roundtrip is unsupported because export is intentionally absent",
                    ),
                },
                ModelInteropSurfaceCompatibility {
                    surface: ModelInteropSurface::TorchPickle,
                    import_status: ModelInteropStatus::Unsupported,
                    import_detail: String::from(
                        "opaque Python pickle or `.pt` checkpoint decoding is intentionally unsupported",
                    ),
                    export_status: ModelInteropStatus::Unsupported,
                    export_detail: String::from(
                        "Psionic does not emit opaque Python pickle checkpoints",
                    ),
                    roundtrip_status: ModelInteropStatus::Unsupported,
                    roundtrip_detail: String::from(
                        "opaque Python pickle roundtrip is intentionally unsupported",
                    ),
                },
                ModelInteropSurfaceCompatibility {
                    surface: ModelInteropSurface::LegacyOpaqueCheckpoint,
                    import_status: ModelInteropStatus::Unsupported,
                    import_detail: String::from(
                        "legacy opaque checkpoint archives are outside the current bounded compatibility contract",
                    ),
                    export_status: ModelInteropStatus::Unsupported,
                    export_detail: String::from(
                        "legacy opaque checkpoint archives are not a Psionic export target",
                    ),
                    roundtrip_status: ModelInteropStatus::Unsupported,
                    roundtrip_detail: String::from(
                        "legacy opaque checkpoint roundtrip is intentionally unsupported",
                    ),
                },
            ],
        )
    }

    /// Builds a filtered and optionally remapped clone of the bundle.
    pub fn select_and_remap(
        &self,
        request: &PortableModelImportRequest,
    ) -> Result<Self, ModelIoError> {
        Ok(Self {
            state_dict: self.state_dict.select_and_remap(request)?,
            tokenizer: self.tokenizer.clone(),
            profile_contract: self.profile_contract.clone(),
            chat_template_digest: self.chat_template_digest.clone(),
            preferred_serving_formats: self.preferred_serving_formats.clone(),
        })
    }

    /// Exports the bundle as a JSON torch-style state-dict compatibility artifact.
    pub fn export_torch_state_dict_json(
        &self,
    ) -> Result<(Vec<u8>, ModelIoArtifactReceipt), ModelIoError> {
        let mut artifact = self.clone();
        artifact.state_dict.source_format = ModelArtifactFormat::TorchStateDictJson;
        let bytes = serde_json::to_vec_pretty(&artifact).map_err(|error| {
            serialization_error("torch state-dict json export", error.to_string())
        })?;
        Ok((
            bytes.clone(),
            ModelIoArtifactReceipt::new(
                ModelArtifactFormat::TorchStateDictJson,
                hex::encode(Sha256::digest(bytes)),
                artifact.state_dict.digest.clone(),
                artifact.tokenizer.contract_digest(),
                artifact.state_dict.tensors.len(),
            ),
        ))
    }

    /// Imports the bundle from a JSON torch-style state-dict compatibility artifact.
    pub fn import_torch_state_dict_json(bytes: &[u8]) -> Result<Self, ModelIoError> {
        Self::import_torch_state_dict_json_with_request(bytes, &PortableModelImportRequest::new())
    }

    /// Imports the bundle from a JSON torch-style state-dict compatibility artifact
    /// with explicit selection and remap control.
    pub fn import_torch_state_dict_json_with_request(
        bytes: &[u8],
        request: &PortableModelImportRequest,
    ) -> Result<Self, ModelIoError> {
        let artifact: Self = serde_json::from_slice(bytes).map_err(|error| {
            serialization_error("torch state-dict json import", error.to_string())
        })?;
        let state_dict = PortableModelStateDict::new(
            artifact.state_dict.model_family,
            artifact.state_dict.revision,
            artifact.state_dict.checkpoint_family,
            artifact.state_dict.checkpoint_ref,
            ModelArtifactFormat::TorchStateDictJson,
            artifact.state_dict.groups,
            artifact.state_dict.tensors,
        )?;
        Self {
            state_dict,
            tokenizer: artifact.tokenizer,
            profile_contract: artifact.profile_contract,
            chat_template_digest: artifact.chat_template_digest,
            preferred_serving_formats: artifact.preferred_serving_formats,
        }
        .select_and_remap(request)
    }

    /// Exports the bundle as dense safetensors with embedded Psionic metadata.
    pub fn export_safetensors(&self) -> Result<(Vec<u8>, ModelIoArtifactReceipt), ModelIoError> {
        let manifest_json = serde_json::to_string(&self.manifest()).map_err(|error| {
            serialization_error("safetensors manifest export", error.to_string())
        })?;
        let mut metadata = HashMap::new();
        metadata.insert(String::from(SAFETENSORS_MANIFEST_KEY), manifest_json);

        let mut raw_buffers = Vec::with_capacity(self.state_dict.tensors.len());
        for (state_key, entry) in &self.state_dict.tensors {
            validate_safetensors_export_entry(state_key.as_str(), entry)?;
            let TensorData::F32(values) = &entry.data else {
                unreachable!("validated safetensors export entry must be dense f32");
            };
            raw_buffers.push((
                state_key.clone(),
                encode_f32_bytes(values),
                entry.manifest.spec.shape().dims().to_vec(),
            ));
        }

        let mut views = Vec::with_capacity(raw_buffers.len());
        for (state_key, raw_bytes, shape) in &raw_buffers {
            let view = TensorView::new(SafeTensorsDType::F32, shape.clone(), raw_bytes.as_slice())
                .map_err(safetensors_error)?;
            views.push((state_key.clone(), view));
        }

        let bytes = serialize(
            views
                .iter()
                .map(|(state_key, view)| (state_key.as_str(), view.clone())),
            Some(metadata),
        )
        .map_err(safetensors_error)?;

        Ok((
            bytes.clone(),
            ModelIoArtifactReceipt::new(
                ModelArtifactFormat::Safetensors,
                hex::encode(Sha256::digest(bytes)),
                self.state_dict.digest.clone(),
                self.tokenizer.contract_digest(),
                self.state_dict.tensors.len(),
            ),
        ))
    }

    /// Imports a bundle from a safetensors artifact emitted by this crate.
    pub fn import_safetensors(bytes: &[u8]) -> Result<Self, ModelIoError> {
        Self::import_safetensors_with_request(bytes, &PortableModelImportRequest::new())
    }

    /// Imports a bundle from a safetensors artifact emitted by this crate with
    /// explicit selection, remap, and materialization control.
    pub fn import_safetensors_with_request(
        bytes: &[u8],
        request: &PortableModelImportRequest,
    ) -> Result<Self, ModelIoError> {
        Self::plan_safetensors_import(bytes, request)?.materialize_bundle()
    }

    /// Builds a bounded safetensors import plan without forcing eager tensor
    /// materialization when the request allows a deferred path.
    pub fn plan_safetensors_import(
        bytes: &[u8],
        request: &PortableModelImportRequest,
    ) -> Result<PortableModelBundleImportPlan, ModelIoError> {
        let (_, metadata) = SafeTensors::read_metadata(bytes).map_err(safetensors_error)?;
        let metadata = metadata
            .metadata()
            .as_ref()
            .ok_or(ModelIoError::MissingSafetensorsManifest)?;
        let manifest_json = metadata
            .get(SAFETENSORS_MANIFEST_KEY)
            .ok_or(ModelIoError::MissingSafetensorsManifest)?;
        let manifest: PortableModelBundleManifest =
            serde_json::from_str(manifest_json).map_err(|error| {
                serialization_error("safetensors manifest import", error.to_string())
            })?;

        let selection = select_and_remap_state_dict_manifest(
            &manifest.state_dict.groups,
            &manifest.state_dict.tensors,
            request,
        )?;
        let owned_bytes: Arc<[u8]> = Arc::from(bytes.to_vec());
        let safetensors =
            SafeTensors::deserialize(owned_bytes.as_ref()).map_err(safetensors_error)?;
        let buffer_start = owned_bytes.as_ptr() as usize;
        let mut tensors = BTreeMap::new();
        for binding in selection.tensor_bindings.values() {
            let view = safetensors
                .tensor(binding.source_state_key.as_str())
                .map_err(safetensors_error)?;
            if view.shape() != binding.manifest.spec.shape().dims() {
                return Err(ModelIoError::SafetensorsShapeMismatch {
                    state_key: binding.target_state_key.clone(),
                    expected: binding.manifest.spec.shape().dims().to_vec(),
                    actual: view.shape().to_vec(),
                });
            }
            if view.dtype() != SafeTensorsDType::F32 || binding.manifest.spec.dtype() != DType::F32
            {
                return Err(ModelIoError::SafetensorsDTypeMismatch {
                    state_key: binding.target_state_key.clone(),
                    expected: binding.manifest.spec.dtype(),
                    actual: view.dtype().to_string(),
                });
            }
            let payload = match request.materialization_policy {
                TensorMaterializationPolicy::Eager => {
                    PortableImportedTensorPayload::Materialized(TensorData::F32(decode_f32_bytes(
                        binding.target_state_key.as_str(),
                        view.data().as_ref(),
                    )?))
                }
                TensorMaterializationPolicy::Deferred => {
                    let start = (view.data().as_ptr() as usize).saturating_sub(buffer_start);
                    let end = start + view.data().len();
                    PortableImportedTensorPayload::Deferred(DeferredTensorData::DenseF32Bytes {
                        bytes: owned_bytes.clone(),
                        byte_range: start..end,
                    })
                }
            };
            tensors.insert(
                binding.target_state_key.clone(),
                PortableImportedTensorEntry {
                    source_state_key: binding.source_state_key.clone(),
                    manifest: binding.manifest.clone(),
                    payload,
                },
            );
        }

        let state_dict_digest = digest_selected_state_dict_payloads(
            manifest.state_dict.model_family.as_str(),
            manifest.state_dict.revision.as_str(),
            manifest.state_dict.checkpoint_family.as_str(),
            manifest.state_dict.checkpoint_ref.as_deref(),
            selection.groups.as_slice(),
            tensors
                .iter()
                .map(|(state_key, entry)| {
                    (
                        state_key.as_str(),
                        entry.manifest.clone(),
                        entry.payload.payload_digest(),
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let state_dict_manifest = PortableModelStateDictManifest {
            model_family: manifest.state_dict.model_family,
            revision: manifest.state_dict.revision,
            checkpoint_family: manifest.state_dict.checkpoint_family,
            checkpoint_ref: manifest.state_dict.checkpoint_ref,
            source_format: ModelArtifactFormat::Safetensors,
            groups: selection.groups,
            tensors: tensors
                .iter()
                .map(|(state_key, entry)| (state_key.clone(), entry.manifest.clone()))
                .collect(),
            digest: state_dict_digest,
        };
        let plan_manifest = PortableModelBundleManifest {
            state_dict: state_dict_manifest,
            tokenizer: manifest.tokenizer,
            profile_contract: manifest.profile_contract,
            chat_template_digest: manifest.chat_template_digest,
            preferred_serving_formats: manifest.preferred_serving_formats,
        };
        let plan_digest = stable_import_plan_digest(
            ModelArtifactFormat::Safetensors,
            request.materialization_policy,
            &plan_manifest.state_dict.digest,
            tensors
                .iter()
                .map(|(state_key, entry)| {
                    (
                        state_key.as_str(),
                        entry.source_state_key.as_str(),
                        entry.payload.payload_digest(),
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
        );

        Ok(PortableModelBundleImportPlan {
            source_format: ModelArtifactFormat::Safetensors,
            materialization_policy: request.materialization_policy,
            manifest: plan_manifest,
            tensors,
            plan_digest,
        })
    }

    /// Imports a typed portable bundle from a GGUF artifact path.
    pub fn import_gguf_path(
        path: impl AsRef<Path>,
        model_family: impl Into<String>,
        revision: impl Into<String>,
        checkpoint_family: impl Into<String>,
        asset_version: impl Into<String>,
    ) -> Result<(Self, ModelIoArtifactReceipt), ModelIoError> {
        let path = path.as_ref();
        let loader = GgufWeightBundleLoader;
        let weights = loader.load_path(path)?;
        let content = GgufContent::read_path(path)?;
        let tokenizer_metadata = content.load_tokenizer()?;
        let chat_templates = content.load_chat_templates()?;
        let chat_template_digest =
            (!chat_templates.is_empty()).then(|| chat_templates.digest().to_string());
        let tokenizer = PortableTokenizerBinding::from_gguf(
            &tokenizer_metadata,
            asset_version,
            chat_template_digest.clone(),
        )?;

        let mut tensors = BTreeMap::new();
        for tensor_metadata in &weights.metadata().tensors {
            let loaded_tensor = weights
                .tensor(tensor_metadata.name.as_str())
                .ok_or_else(|| ModelIoError::MissingGgufTensor {
                    state_key: tensor_metadata.name.clone(),
                })?;
            let spec = TensorSpec::new(
                tensor_metadata.shape.clone(),
                tensor_metadata.dtype,
                Device::cpu(),
            );
            let data = match loaded_tensor.storage() {
                WeightTensorStorage::DequantizedF32(values) => TensorData::F32(values.clone()),
                WeightTensorStorage::QuantizedBlocks(storage) => {
                    TensorData::QuantizedBlocks(QuantizedTensorData::new(
                        storage.quantization(),
                        storage.layout(),
                        storage.bytes().to_vec(),
                    ))
                }
            };
            let state_key = tensor_metadata.name.clone();
            insert_state_entry(
                &mut tensors,
                state_key.clone(),
                ModelStateTensorEntry {
                    manifest: ModelStateTensorManifest::new(
                        state_key.clone(),
                        split_model_tree_path(state_key.as_str()),
                        ModelStateTensorRole::Parameter,
                        spec,
                    ),
                    data,
                },
            )?;
        }

        let state_dict = PortableModelStateDict::new(
            model_family,
            revision,
            checkpoint_family,
            None,
            ModelArtifactFormat::Gguf,
            Vec::new(),
            tensors,
        )?;
        let bundle = Self {
            state_dict,
            tokenizer,
            profile_contract: None,
            chat_template_digest,
            preferred_serving_formats: vec![
                ModelArtifactFormat::Gguf,
                ModelArtifactFormat::TorchStateDictJson,
            ],
        };
        let artifact_digest = weights
            .metadata()
            .artifacts
            .first()
            .map(|artifact| artifact.sha256.clone())
            .unwrap_or_else(|| bundle.state_dict.digest.clone());
        let receipt = ModelIoArtifactReceipt::new(
            ModelArtifactFormat::Gguf,
            artifact_digest,
            bundle.state_dict.digest.clone(),
            bundle.tokenizer.contract_digest(),
            bundle.state_dict.tensors.len(),
        );
        Ok((bundle, receipt))
    }
}

/// Machine-legible receipt for one portable model artifact export or import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIoArtifactReceipt {
    /// Artifact surface that produced the receipt.
    pub format: ModelArtifactFormat,
    /// Stable artifact digest or identity digest for the produced artifact.
    pub artifact_digest: String,
    /// Stable state-dict digest carried by the artifact.
    pub state_dict_digest: String,
    /// Stable tokenizer-contract digest carried by the artifact.
    pub tokenizer_contract_digest: String,
    /// Number of named tensors carried by the artifact.
    pub tensor_count: usize,
}

impl ModelIoArtifactReceipt {
    fn new(
        format: ModelArtifactFormat,
        artifact_digest: String,
        state_dict_digest: String,
        tokenizer_contract_digest: String,
        tensor_count: usize,
    ) -> Self {
        Self {
            format,
            artifact_digest,
            state_dict_digest,
            tokenizer_contract_digest,
            tensor_count,
        }
    }
}

/// One additive parameter delta derived between two state dicts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelAdapterDeltaTensor {
    /// Stable parameter tensor key.
    pub state_key: String,
    /// Logical model-tree path for the parameter.
    pub model_tree_path: Vec<String>,
    /// Tensor spec required by the adapter.
    pub spec: TensorSpec,
    /// Per-element additive delta values.
    pub delta_values: Vec<f32>,
}

/// Typed adapter merge or unmerge artifact derived from two portable state dicts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelAdapterDelta {
    /// Stable adapter identifier.
    pub adapter_id: String,
    /// Expected base state-dict digest.
    pub base_state_dict_digest: String,
    /// Expected tuned state-dict digest after applying the adapter.
    pub target_state_dict_digest: String,
    /// Parameter deltas keyed by stable tensor name.
    pub tensors: BTreeMap<String, ModelAdapterDeltaTensor>,
}

fn validate_state_tensors(
    tensors: &BTreeMap<String, ModelStateTensorEntry>,
) -> Result<(), ModelIoError> {
    for (map_key, entry) in tensors {
        if entry.manifest.state_key != *map_key {
            return Err(ModelIoError::TensorManifestKeyMismatch {
                map_key: map_key.clone(),
                manifest_key: entry.manifest.state_key.clone(),
            });
        }
        validate_tensor_payload(map_key.as_str(), &entry.manifest.spec, &entry.data)?;
    }
    Ok(())
}

fn validate_state_groups(
    groups: &[ModelStateGroupAssignment],
    tensors: &BTreeMap<String, ModelStateTensorEntry>,
) -> Result<(), ModelIoError> {
    let mut group_ids = BTreeSet::new();
    for group in groups {
        if !group_ids.insert(group.group_id.clone()) {
            return Err(ModelIoError::DuplicateGroup {
                group_id: group.group_id.clone(),
            });
        }

        validate_assignment_tensor(
            group.group_id.as_str(),
            group.parameter_key.as_str(),
            ModelStateTensorRole::Parameter,
            tensors,
        )?;

        match group.optimizer.kind {
            TrainingOptimizerKind::Sgd | TrainingOptimizerKind::Lars => {
                if group.first_moment_key.is_some() || group.second_moment_key.is_some() {
                    return Err(ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: String::from(
                            "momentum-buffer optimizer group may not carry Adam-family first/second moment keys",
                        ),
                    });
                }
                if let Some(momentum_buffer_key) = &group.momentum_buffer_key {
                    validate_assignment_tensor(
                        group.group_id.as_str(),
                        momentum_buffer_key.as_str(),
                        ModelStateTensorRole::SgdMomentumBuffer,
                        tensors,
                    )?;
                }
            }
            TrainingOptimizerKind::Adam
            | TrainingOptimizerKind::AdamW
            | TrainingOptimizerKind::Lamb => {
                if group.momentum_buffer_key.is_some() {
                    return Err(ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: String::from(
                            "Adam-family optimizer group may not carry a momentum buffer key",
                        ),
                    });
                }
                let first_moment_key = group.first_moment_key.as_ref().ok_or_else(|| {
                    ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: format!(
                            "{:?} group is missing `first_moment_key`",
                            group.optimizer.kind
                        ),
                    }
                })?;
                let second_moment_key = group.second_moment_key.as_ref().ok_or_else(|| {
                    ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: format!(
                            "{:?} group is missing `second_moment_key`",
                            group.optimizer.kind
                        ),
                    }
                })?;
                validate_assignment_tensor(
                    group.group_id.as_str(),
                    first_moment_key.as_str(),
                    ModelStateTensorRole::AdamFirstMoment,
                    tensors,
                )?;
                validate_assignment_tensor(
                    group.group_id.as_str(),
                    second_moment_key.as_str(),
                    ModelStateTensorRole::AdamSecondMoment,
                    tensors,
                )?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct SelectedTensorBinding {
    source_state_key: String,
    target_state_key: String,
    manifest: ModelStateTensorManifest,
}

#[derive(Clone, Debug)]
struct StateDictImportSelectionPlan {
    groups: Vec<ModelStateGroupAssignment>,
    tensor_bindings: BTreeMap<String, SelectedTensorBinding>,
}

fn select_and_remap_state_dict_manifest(
    groups: &[ModelStateGroupAssignment],
    tensors: &BTreeMap<String, ModelStateTensorManifest>,
    request: &PortableModelImportRequest,
) -> Result<StateDictImportSelectionPlan, ModelIoError> {
    for state_key in &request.selection.include_state_keys {
        if !tensors.contains_key(state_key.as_str()) {
            return Err(ModelIoError::MissingImportSelectionTensor {
                state_key: state_key.clone(),
            });
        }
    }
    for state_key in request.key_remap.source_to_target.keys() {
        if !tensors.contains_key(state_key.as_str()) {
            return Err(ModelIoError::MissingImportRemapSource {
                state_key: state_key.clone(),
            });
        }
    }

    let selected_sources = if request.selection.include_state_keys.is_empty() {
        tensors.keys().cloned().collect::<BTreeSet<_>>()
    } else {
        request.selection.include_state_keys.clone()
    };

    let mut selected_sources = selected_sources
        .into_iter()
        .filter(|state_key| !request.selection.exclude_state_keys.contains(state_key))
        .collect::<BTreeSet<_>>();

    let mut tensor_bindings = BTreeMap::new();
    let mut remap_targets = BTreeMap::new();
    for source_state_key in selected_sources.iter() {
        let target_state_key = request
            .key_remap
            .target_for(source_state_key.as_str())
            .to_string();
        if let Some(previous) =
            remap_targets.insert(target_state_key.clone(), source_state_key.clone())
        {
            return Err(ModelIoError::ImportRemapCollision {
                target_state_key,
                first_source_state_key: previous,
                second_source_state_key: source_state_key.clone(),
            });
        }
        let mut manifest = tensors
            .get(source_state_key.as_str())
            .cloned()
            .ok_or_else(|| ModelIoError::MissingTensor {
                state_key: source_state_key.clone(),
            })?;
        manifest.state_key = target_state_key.clone();
        tensor_bindings.insert(
            target_state_key.clone(),
            SelectedTensorBinding {
                source_state_key: source_state_key.clone(),
                target_state_key,
                manifest,
            },
        );
    }

    let mut selected_groups = Vec::new();
    for group in groups {
        let group_keys = group_state_keys(group);
        let missing = group_keys
            .iter()
            .filter(|state_key| !selected_sources.contains((*state_key).as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if missing.len() == group_keys.len() {
            continue;
        }
        if !missing.is_empty() {
            return Err(ModelIoError::IncompleteImportGroupSelection {
                group_id: group.group_id.clone(),
                missing_state_keys: missing,
            });
        }

        let mut mapped = group.clone();
        mapped.parameter_key = request
            .key_remap
            .target_for(group.parameter_key.as_str())
            .to_string();
        mapped.momentum_buffer_key = group
            .momentum_buffer_key
            .as_ref()
            .map(|state_key| request.key_remap.target_for(state_key.as_str()).to_string());
        mapped.first_moment_key = group
            .first_moment_key
            .as_ref()
            .map(|state_key| request.key_remap.target_for(state_key.as_str()).to_string());
        mapped.second_moment_key = group
            .second_moment_key
            .as_ref()
            .map(|state_key| request.key_remap.target_for(state_key.as_str()).to_string());
        selected_groups.push(mapped);
    }

    selected_sources.clear();
    Ok(StateDictImportSelectionPlan {
        groups: selected_groups,
        tensor_bindings,
    })
}

fn group_state_keys(group: &ModelStateGroupAssignment) -> Vec<String> {
    let mut keys = vec![group.parameter_key.clone()];
    if let Some(state_key) = &group.momentum_buffer_key {
        keys.push(state_key.clone());
    }
    if let Some(state_key) = &group.first_moment_key {
        keys.push(state_key.clone());
    }
    if let Some(state_key) = &group.second_moment_key {
        keys.push(state_key.clone());
    }
    keys
}

fn validate_assignment_tensor(
    group_id: &str,
    state_key: &str,
    expected_role: ModelStateTensorRole,
    tensors: &BTreeMap<String, ModelStateTensorEntry>,
) -> Result<(), ModelIoError> {
    let entry = tensors
        .get(state_key)
        .ok_or_else(|| ModelIoError::MissingTensor {
            state_key: String::from(state_key),
        })?;
    if entry.manifest.role != expected_role {
        return Err(ModelIoError::TensorRoleMismatch {
            group_id: String::from(group_id),
            state_key: String::from(state_key),
            expected: expected_role.label(),
            actual: entry.manifest.role.label(),
        });
    }
    Ok(())
}

fn validate_tensor_payload(
    state_key: &str,
    spec: &TensorSpec,
    data: &TensorData,
) -> Result<(), ModelIoError> {
    match data {
        TensorData::F32(values) => {
            if spec.dtype() != DType::F32 {
                return Err(ModelIoError::DenseF32Required {
                    state_key: String::from(state_key),
                });
            }
            if values.len() != spec.storage_size() {
                return Err(ModelIoError::TensorPayloadLengthMismatch {
                    state_key: String::from(state_key),
                    expected_len: spec.storage_size(),
                    actual_len: values.len(),
                });
            }
        }
        TensorData::BF16(values) => {
            if spec.dtype() != DType::BF16 {
                return Err(ModelIoError::DenseF32Required {
                    state_key: String::from(state_key),
                });
            }
            if values.len() != spec.storage_size() {
                return Err(ModelIoError::TensorPayloadLengthMismatch {
                    state_key: String::from(state_key),
                    expected_len: spec.storage_size(),
                    actual_len: values.len(),
                });
            }
        }
        TensorData::I32(_) => {
            return Err(ModelIoError::DenseF32Required {
                state_key: String::from(state_key),
            });
        }
        TensorData::QuantizedBlocks(quantized) => {
            if spec.dtype() != DType::F32 {
                return Err(ModelIoError::DenseF32Required {
                    state_key: String::from(state_key),
                });
            }
            if quantized.layout.element_count() != spec.element_count() {
                return Err(ModelIoError::QuantizedTensorLayoutMismatch {
                    state_key: String::from(state_key),
                    expected_len: spec.element_count(),
                    actual_len: quantized.layout.element_count(),
                });
            }
        }
    }
    Ok(())
}

fn insert_state_entry(
    tensors: &mut BTreeMap<String, ModelStateTensorEntry>,
    state_key: String,
    entry: ModelStateTensorEntry,
) -> Result<(), ModelIoError> {
    if tensors.insert(state_key.clone(), entry).is_some() {
        return Err(ModelIoError::DuplicateTensor { state_key });
    }
    Ok(())
}

fn training_buffer_from_state_entry(
    state_key: &str,
    entry: &ModelStateTensorEntry,
) -> Result<TrainingTensorBuffer, ModelIoError> {
    validate_tensor_payload(state_key, &entry.manifest.spec, &entry.data)?;
    Ok(TrainingTensorBuffer {
        spec: entry.manifest.spec.clone(),
        data: entry.data.clone(),
    })
}

fn dense_f32_values<'a>(
    state_key: &str,
    entry: &'a ModelStateTensorEntry,
) -> Result<&'a [f32], ModelIoError> {
    match &entry.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        TensorData::I32(_) => Err(ModelIoError::DenseF32Required {
            state_key: String::from(state_key),
        }),
        TensorData::QuantizedBlocks(_) => Err(ModelIoError::DenseF32Required {
            state_key: String::from(state_key),
        }),
    }
}

fn dense_f32_values_mut<'a>(
    state_key: &str,
    entry: &'a mut ModelStateTensorEntry,
) -> Result<&'a mut [f32], ModelIoError> {
    match &mut entry.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_mut_slice()),
        TensorData::I32(_) => Err(ModelIoError::DenseF32Required {
            state_key: String::from(state_key),
        }),
        TensorData::QuantizedBlocks(_) => Err(ModelIoError::DenseF32Required {
            state_key: String::from(state_key),
        }),
    }
}

fn split_model_tree_path(value: &str) -> Vec<String> {
    value
        .split(['.', '/'])
        .filter(|part| !part.is_empty())
        .map(String::from)
        .collect()
}

fn extend_tree_path(mut path: Vec<String>, leaf: impl Into<String>) -> Vec<String> {
    path.push(leaf.into());
    path
}

fn digest_state_dict(
    model_family: &str,
    revision: &str,
    checkpoint_family: &str,
    checkpoint_ref: Option<&str>,
    groups: &[ModelStateGroupAssignment],
    tensors: &BTreeMap<String, ModelStateTensorEntry>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_model_state_dict|");
    hasher.update(model_family.as_bytes());
    hasher.update(b"|");
    hasher.update(revision.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    if let Some(checkpoint_ref) = checkpoint_ref {
        hasher.update(b"|checkpoint_ref|");
        hasher.update(checkpoint_ref.as_bytes());
    }
    for group in groups {
        hasher.update(stable_json_bytes(group));
    }
    for (state_key, entry) in tensors {
        hasher.update(state_key.as_bytes());
        hasher.update(stable_json_bytes(&entry.manifest));
        hasher.update(tensor_payload_digest(&entry.data).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn tensor_payload_digest(data: &TensorData) -> String {
    let mut hasher = Sha256::new();
    match data {
        TensorData::F32(values) => {
            hasher.update(b"f32|");
            for value in values {
                hasher.update(value.to_le_bytes());
            }
        }
        TensorData::BF16(values) => {
            hasher.update(b"bf16|");
            for value in values {
                hasher.update(value.to_le_bytes());
            }
        }
        TensorData::I32(values) => {
            hasher.update(b"i32|");
            for value in values {
                hasher.update(value.to_le_bytes());
            }
        }
        TensorData::QuantizedBlocks(quantized) => {
            hasher.update(b"quantized|");
            hasher.update(stable_json_bytes(quantized));
        }
    }
    hex::encode(hasher.finalize())
}

fn tensor_payload_digest_from_raw_f32_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"f32|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn digest_selected_state_dict_payloads(
    model_family: &str,
    revision: &str,
    checkpoint_family: &str,
    checkpoint_ref: Option<&str>,
    groups: &[ModelStateGroupAssignment],
    tensors: &[(&str, ModelStateTensorManifest, String)],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_model_state_dict|");
    hasher.update(model_family.as_bytes());
    hasher.update(b"|");
    hasher.update(revision.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    if let Some(checkpoint_ref) = checkpoint_ref {
        hasher.update(b"|checkpoint_ref|");
        hasher.update(checkpoint_ref.as_bytes());
    }
    for group in groups {
        hasher.update(stable_json_bytes(group));
    }
    for (state_key, manifest, payload_digest) in tensors {
        hasher.update(state_key.as_bytes());
        hasher.update(stable_json_bytes(manifest));
        hasher.update(payload_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_import_plan_digest(
    source_format: ModelArtifactFormat,
    materialization_policy: TensorMaterializationPolicy,
    state_dict_digest: &str,
    tensors: &[(&str, &str, String)],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_model_import_plan|");
    hasher.update(format!("{source_format:?}").as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{materialization_policy:?}").as_bytes());
    hasher.update(b"|");
    hasher.update(state_dict_digest.as_bytes());
    for (target_state_key, source_state_key, payload_digest) in tensors {
        hasher.update(target_state_key.as_bytes());
        hasher.update(b"|");
        hasher.update(source_state_key.as_bytes());
        hasher.update(b"|");
        hasher.update(payload_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn safetensors_surface_compatibility(
    state_dict: &PortableModelStateDict,
) -> ModelInteropSurfaceCompatibility {
    match state_dict
        .tensors
        .iter()
        .try_for_each(|(state_key, entry)| validate_safetensors_export_entry(state_key, entry))
    {
        Ok(()) => ModelInteropSurfaceCompatibility {
            surface: ModelInteropSurface::PsionicManifestSafetensors,
            import_status: ModelInteropStatus::Supported,
            import_detail: String::from(
                "supported for dense `f32` safetensors artifacts that carry the embedded Psionic manifest",
            ),
            export_status: ModelInteropStatus::Supported,
            export_detail: String::from(
                "supported for dense `f32` contiguous state dicts with embedded Psionic manifest metadata",
            ),
            roundtrip_status: ModelInteropStatus::Supported,
            roundtrip_detail: String::from(
                "dense `f32` manifest-carrying safetensors can roundtrip through the portable bundle",
            ),
        },
        Err(error) => {
            let detail = format!(
                "unsupported for this bundle because dense `f32` manifest-carrying safetensors require current tensor constraints: {error}"
            );
            ModelInteropSurfaceCompatibility {
                surface: ModelInteropSurface::PsionicManifestSafetensors,
                import_status: ModelInteropStatus::Unsupported,
                import_detail: detail.clone(),
                export_status: ModelInteropStatus::Unsupported,
                export_detail: detail.clone(),
                roundtrip_status: ModelInteropStatus::Unsupported,
                roundtrip_detail: detail,
            }
        }
    }
}

fn validate_safetensors_export_entry(
    state_key: &str,
    entry: &ModelStateTensorEntry,
) -> Result<(), ModelIoError> {
    let spec = &entry.manifest.spec;
    if spec.storage_size() != spec.element_count() {
        return Err(ModelIoError::NonContiguousSafetensorsTensor {
            state_key: String::from(state_key),
        });
    }
    match &entry.data {
        TensorData::F32(_) if spec.dtype() == DType::F32 => Ok(()),
        _ => Err(ModelIoError::UnsupportedSafetensorsTensor {
            state_key: String::from(state_key),
        }),
    }
}

fn stable_model_io_compatibility_digest(
    model_family: &str,
    revision: &str,
    state_dict_digest: &str,
    surfaces: &[ModelInteropSurfaceCompatibility],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_model_io_compatibility|");
    hasher.update(model_family.as_bytes());
    hasher.update(b"|");
    hasher.update(revision.as_bytes());
    hasher.update(b"|");
    hasher.update(state_dict_digest.as_bytes());
    for surface in surfaces {
        hasher.update(stable_json_bytes(surface));
    }
    hex::encode(hasher.finalize())
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>()
}

fn decode_f32_bytes(state_key: &str, bytes: &[u8]) -> Result<Vec<f32>, ModelIoError> {
    if bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(ModelIoError::Serialization {
            context: "f32 tensor decode",
            message: format!(
                "tensor `{state_key}` byte length {} is not divisible by 4",
                bytes.len()
            ),
        });
    }
    Ok(bytes
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| {
            let mut array = [0_u8; 4];
            array.copy_from_slice(chunk);
            f32::from_le_bytes(array)
        })
        .collect())
}

fn tokenizer_family_from_gguf(model: GgufTokenizerModel) -> TokenizerFamily {
    match model {
        GgufTokenizerModel::SentencePiece => TokenizerFamily::SentencePiece,
        GgufTokenizerModel::Gpt2Bpe => TokenizerFamily::BytePairEncoding,
        GgufTokenizerModel::Gemma4Bpe => TokenizerFamily::BytePairEncoding,
        GgufTokenizerModel::BertWordPiece => TokenizerFamily::WordPiece,
    }
}

fn digest_tokenizer_specials(tokenizer: &GgufTokenizerMetadata) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_tokenizer_specials|");
    hasher.update(u8::from(tokenizer.add_bos).to_be_bytes());
    hasher.update(u8::from(tokenizer.add_eos).to_be_bytes());
    if let Some(id) = tokenizer.vocabulary.bos_token_id() {
        hasher.update(b"|bos|");
        hasher.update(id.as_u32().to_be_bytes());
    }
    for id in tokenizer.vocabulary.eos_token_ids() {
        hasher.update(b"|eos|");
        hasher.update(id.as_u32().to_be_bytes());
    }
    if let Some(id) = tokenizer.vocabulary.pad_token_id() {
        hasher.update(b"|pad|");
        hasher.update(id.as_u32().to_be_bytes());
    }
    if let Some(id) = tokenizer.vocabulary.unknown_token_id() {
        hasher.update(b"|unk|");
        hasher.update(id.as_u32().to_be_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_json_bytes(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("stable JSON serialization failed")
}

fn safetensors_error(error: SafeTensorError) -> ModelIoError {
    ModelIoError::Serialization {
        context: "safetensors",
        message: error.to_string(),
    }
}

fn serialization_error(context: &'static str, message: String) -> ModelIoError {
    ModelIoError::Serialization { context, message }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use psionic_core::{QuantizationMode, QuantizedBlockLayout, Shape};
    use psionic_models::{GgufMetadataValue, GgufTensorType, GgufVersion};
    use tempfile::tempdir;

    use super::*;
    use crate::TrainingSchedulerConfig;

    #[test]
    fn portable_model_bundle_roundtrips_training_groups_through_torch_json()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?
        .with_profile_contract(sample_profile_contract());

        let traversal = bundle.state_dict.traversal_records();
        assert_eq!(traversal.len(), 5);
        let first_moment = traversal
            .iter()
            .find(|record| record.state_key == "optimizer.decoder.head.first_moment")
            .expect("missing first-moment traversal record");
        assert_eq!(
            first_moment.model_tree_path,
            vec![
                String::from("decoder"),
                String::from("head"),
                String::from("first_moment")
            ]
        );

        let (bytes, receipt) = bundle.export_torch_state_dict_json()?;
        assert_eq!(receipt.format, ModelArtifactFormat::TorchStateDictJson);
        assert_eq!(receipt.tensor_count, 5);

        let imported = PortableModelBundle::import_torch_state_dict_json(bytes.as_slice())?;
        assert_eq!(
            imported.state_dict.source_format,
            ModelArtifactFormat::TorchStateDictJson
        );
        assert_eq!(imported.to_training_groups()?, groups);
        assert_eq!(imported.tokenizer, bundle.tokenizer);
        assert_eq!(imported.profile_contract, bundle.profile_contract);
        Ok(())
    }

    #[test]
    fn portable_model_bundle_roundtrips_through_safetensors_manifest() -> Result<(), Box<dyn Error>>
    {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?
        .with_profile_contract(sample_profile_contract());

        let (bytes, receipt) = bundle.export_safetensors()?;
        assert_eq!(receipt.format, ModelArtifactFormat::Safetensors);

        let imported = PortableModelBundle::import_safetensors(bytes.as_slice())?;
        assert_eq!(
            imported.state_dict.source_format,
            ModelArtifactFormat::Safetensors
        );
        assert_eq!(imported.to_training_groups()?, groups);
        assert_eq!(imported.tokenizer, bundle.tokenizer);
        assert_eq!(imported.profile_contract, bundle.profile_contract);
        assert_eq!(imported.chat_template_digest, bundle.chat_template_digest);
        Ok(())
    }

    #[test]
    fn portable_model_bundle_select_and_remap_keeps_complete_groups_only()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;

        let request = PortableModelImportRequest::new()
            .with_selection(
                PortableTensorImportSelection::new()
                    .include("model.decoder.head.parameter")
                    .include("optimizer.decoder.head.first_moment")
                    .include("optimizer.decoder.head.second_moment"),
            )
            .with_key_remap(
                PortableTensorKeyRemap::new()
                    .map(
                        "model.decoder.head.parameter",
                        "model.decoder.output.parameter",
                    )
                    .map(
                        "optimizer.decoder.head.first_moment",
                        "optimizer.decoder.output.first_moment",
                    )
                    .map(
                        "optimizer.decoder.head.second_moment",
                        "optimizer.decoder.output.second_moment",
                    ),
            );

        let selected = bundle.select_and_remap(&request)?;
        assert_eq!(selected.state_dict.tensors.len(), 3);
        assert_eq!(selected.state_dict.groups.len(), 1);
        assert!(
            selected
                .state_dict
                .tensors
                .contains_key("model.decoder.output.parameter")
        );
        assert!(
            selected
                .state_dict
                .tensors
                .contains_key("optimizer.decoder.output.first_moment")
        );
        assert!(
            selected
                .state_dict
                .tensors
                .contains_key("optimizer.decoder.output.second_moment")
        );
        assert_eq!(selected.to_training_groups()?.len(), 1);
        assert_eq!(selected.to_training_groups()?[0].group_id, "decoder.head");
        Ok(())
    }

    #[test]
    fn portable_model_bundle_import_selection_refuses_incomplete_group()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;
        let (bytes, _) = bundle.export_safetensors()?;
        let request = PortableModelImportRequest::new().with_selection(
            PortableTensorImportSelection::new().include("model.embedding.parameter"),
        );

        let error = PortableModelBundle::plan_safetensors_import(bytes.as_slice(), &request)
            .expect_err("incomplete embedding group should refuse");
        match error {
            ModelIoError::IncompleteImportGroupSelection {
                group_id,
                missing_state_keys,
            } => {
                assert_eq!(group_id, "embedding");
                assert_eq!(
                    missing_state_keys,
                    vec![String::from("optimizer.embedding.momentum_buffer")]
                );
            }
            other => panic!("unexpected error: {other}"),
        }
        Ok(())
    }

    #[test]
    fn portable_model_bundle_import_selection_refuses_remap_collisions()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;
        let request = PortableModelImportRequest::new()
            .with_selection(
                PortableTensorImportSelection::new()
                    .include("model.decoder.head.parameter")
                    .include("optimizer.decoder.head.first_moment")
                    .include("optimizer.decoder.head.second_moment"),
            )
            .with_key_remap(
                PortableTensorKeyRemap::new()
                    .map("model.decoder.head.parameter", "model.decoder.shared")
                    .map(
                        "optimizer.decoder.head.first_moment",
                        "model.decoder.shared",
                    ),
            );

        let error = bundle
            .select_and_remap(&request)
            .expect_err("collision should refuse");
        match error {
            ModelIoError::ImportRemapCollision {
                target_state_key,
                first_source_state_key,
                second_source_state_key,
            } => {
                assert_eq!(target_state_key, "model.decoder.shared");
                assert_eq!(first_source_state_key, "model.decoder.head.parameter");
                assert_eq!(
                    second_source_state_key,
                    "optimizer.decoder.head.first_moment"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
        Ok(())
    }

    #[test]
    fn portable_model_bundle_plan_safetensors_import_can_defer_materialization()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;
        let (bytes, _) = bundle.export_safetensors()?;
        let request = PortableModelImportRequest::new()
            .with_selection(
                PortableTensorImportSelection::new()
                    .include("model.decoder.head.parameter")
                    .include("optimizer.decoder.head.first_moment")
                    .include("optimizer.decoder.head.second_moment"),
            )
            .with_materialization_policy(TensorMaterializationPolicy::Deferred);

        let plan = PortableModelBundle::plan_safetensors_import(bytes.as_slice(), &request)?;
        assert_eq!(plan.tensor_count(), 3);
        assert_eq!(plan.deferred_tensor_count(), 3);
        assert!(
            plan.stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("plan_digest="))
        );

        let TensorData::F32(values) = plan.materialize_tensor("model.decoder.head.parameter")?
        else {
            panic!("expected dense f32 decoder parameter");
        };
        assert_eq!(values, vec![0.5, -0.5]);

        let materialized = plan.materialize_bundle()?;
        let eager = bundle.select_and_remap(&request)?;
        assert_eq!(materialized.state_dict.digest, eager.state_dict.digest);
        assert_eq!(
            materialized.to_training_groups()?,
            eager.to_training_groups()?
        );
        Ok(())
    }

    #[test]
    fn portable_model_bundle_import_torch_json_with_request_matches_select_and_remap()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;
        let request = PortableModelImportRequest::new().with_selection(
            PortableTensorImportSelection::new()
                .include("model.decoder.head.parameter")
                .include("optimizer.decoder.head.first_moment")
                .include("optimizer.decoder.head.second_moment"),
        );
        let (bytes, _) = bundle.export_torch_state_dict_json()?;

        let imported = PortableModelBundle::import_torch_state_dict_json_with_request(
            bytes.as_slice(),
            &request,
        )?;
        let expected = bundle.select_and_remap(&request)?;
        assert_eq!(imported.state_dict.digest, expected.state_dict.digest);
        assert_eq!(
            imported.to_training_groups()?,
            expected.to_training_groups()?
        );
        Ok(())
    }

    #[test]
    fn portable_model_bundle_publishes_explicit_compatibility_boundaries()
    -> Result<(), Box<dyn Error>> {
        let groups = sample_training_groups()?;
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "compat-r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/compat")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;

        let contract = bundle.compatibility_contract();
        assert_eq!(contract.model_family, "weather-agent");
        assert_eq!(contract.revision, "compat-r1");
        assert_eq!(contract.state_dict_digest, bundle.state_dict.digest);
        assert!(
            contract
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("contract_digest="))
        );

        let safetensors = contract
            .surfaces
            .iter()
            .find(|surface| surface.surface == ModelInteropSurface::PsionicManifestSafetensors)
            .expect("missing safetensors surface");
        assert_eq!(safetensors.export_status, ModelInteropStatus::Supported);
        assert_eq!(safetensors.roundtrip_status, ModelInteropStatus::Supported);

        let gguf = contract
            .surfaces
            .iter()
            .find(|surface| surface.surface == ModelInteropSurface::Gguf)
            .expect("missing gguf surface");
        assert_eq!(gguf.import_status, ModelInteropStatus::Supported);
        assert_eq!(gguf.export_status, ModelInteropStatus::Unsupported);

        let pickle = contract
            .surfaces
            .iter()
            .find(|surface| surface.surface == ModelInteropSurface::TorchPickle)
            .expect("missing pickle surface");
        assert_eq!(pickle.import_status, ModelInteropStatus::Unsupported);
        assert!(pickle.import_detail.contains("opaque Python pickle"));
        Ok(())
    }

    #[test]
    fn quantized_portable_bundle_marks_safetensors_boundary_unsupported()
    -> Result<(), Box<dyn Error>> {
        let state_key = String::from("model.quantized.weight");
        let spec = TensorSpec::new(Shape::new(vec![32]), DType::F32, Device::cpu());
        let quantized = QuantizedTensorData::new(
            QuantizationMode::GgmlQ4_0,
            QuantizedBlockLayout::new(32, 18, 1),
            vec![0_u8; 18],
        );
        let tensors = BTreeMap::from([(
            state_key.clone(),
            ModelStateTensorEntry {
                manifest: ModelStateTensorManifest::new(
                    state_key.clone(),
                    vec![
                        String::from("model"),
                        String::from("quantized"),
                        String::from("weight"),
                    ],
                    ModelStateTensorRole::Parameter,
                    spec,
                ),
                data: TensorData::QuantizedBlocks(quantized),
            },
        )]);
        let state_dict = PortableModelStateDict::new(
            "quantized-agent",
            "q1",
            "quant-checkpoints",
            None,
            ModelArtifactFormat::PsionicStateDict,
            Vec::new(),
            tensors,
        )?;
        let bundle = PortableModelBundle {
            state_dict,
            tokenizer: sample_tokenizer_binding(),
            profile_contract: None,
            chat_template_digest: None,
            preferred_serving_formats: vec![ModelArtifactFormat::TorchStateDictJson],
        };

        let contract = bundle.compatibility_contract();
        let safetensors = contract
            .surfaces
            .iter()
            .find(|surface| surface.surface == ModelInteropSurface::PsionicManifestSafetensors)
            .expect("missing safetensors surface");
        assert_eq!(safetensors.import_status, ModelInteropStatus::Unsupported);
        assert_eq!(safetensors.export_status, ModelInteropStatus::Unsupported);
        assert!(safetensors.export_detail.contains("dense `f32`"));
        Ok(())
    }

    #[test]
    fn portable_model_bundle_roundtrips_new_optimizer_family_variants_through_safetensors()
    -> Result<(), Box<dyn Error>> {
        let mut adam = TrainingParameterGroupState::new(
            "adam.block",
            TrainingParameterClass::Matrix,
            TrainingTensorBuffer::from_f32(
                "adam.block",
                TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                vec![1.0, -1.0],
            )?,
            TrainingOptimizerConfig::adam(0.01, 0.9, 0.999, 1e-8),
            TrainingOptimizerResidencyPolicy::new(
                OptimizerStateResidency::DeviceResident,
                OptimizerStateResidency::HostResident,
            ),
        )?
        .with_parameter_semantics(TrainingParameterGroupSemantics::new(0.5, 1.0))
        .with_scheduler(TrainingSchedulerConfig::linear_warmup(4, 0.25));
        adam.optimizer_state = TrainingOptimizerState::Adam {
            first_moment: vec![0.01, -0.02],
            second_moment: vec![0.03, 0.04],
        };
        adam.applied_steps = 2;
        adam.scheduler.as_mut().expect("scheduler").state.last_step = 2;
        adam.scheduler
            .as_mut()
            .expect("scheduler")
            .state
            .last_learning_rate = Some(0.00375);

        let mut lars = TrainingParameterGroupState::new(
            "lars.block",
            TrainingParameterClass::Matrix,
            TrainingTensorBuffer::from_f32(
                "lars.block",
                TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                vec![0.25, -0.5],
            )?,
            TrainingOptimizerConfig::lars(0.02, 0.9, 0.001, 1e-8).with_weight_decay(0.01),
            TrainingOptimizerResidencyPolicy::new(
                OptimizerStateResidency::DeviceResident,
                OptimizerStateResidency::HostResident,
            ),
        )?
        .with_parameter_semantics(TrainingParameterGroupSemantics::new(1.25, 0.5))
        .with_scheduler(TrainingSchedulerConfig::step_lr(2, 0.5));
        lars.optimizer_state = TrainingOptimizerState::Lars {
            momentum_buffer: Some(vec![0.001, -0.002]),
        };
        lars.optimizer_residency = OptimizerStateResidency::HostResident;
        lars.applied_steps = 3;
        lars.scheduler.as_mut().expect("scheduler").state.last_step = 3;
        lars.scheduler
            .as_mut()
            .expect("scheduler")
            .state
            .last_learning_rate = Some(0.0125);

        let mut lamb = TrainingParameterGroupState::new(
            "lamb.head",
            TrainingParameterClass::Head,
            TrainingTensorBuffer::from_f32(
                "lamb.head",
                TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                vec![0.75, -0.25],
            )?,
            TrainingOptimizerConfig::lamb(0.015, 0.9, 0.999, 1e-6).with_weight_decay(0.02),
            TrainingOptimizerResidencyPolicy::new(
                OptimizerStateResidency::DeviceResident,
                OptimizerStateResidency::Offloaded,
            ),
        )?
        .with_parameter_semantics(TrainingParameterGroupSemantics::new(0.75, 1.5))
        .with_scheduler(TrainingSchedulerConfig::cosine_annealing(8, 0.001));
        lamb.optimizer_state = TrainingOptimizerState::Lamb {
            first_moment: vec![0.02, -0.03],
            second_moment: vec![0.04, 0.05],
        };
        lamb.optimizer_residency = OptimizerStateResidency::Offloaded;
        lamb.applied_steps = 4;
        lamb.scheduler.as_mut().expect("scheduler").state.last_step = 4;
        lamb.scheduler
            .as_mut()
            .expect("scheduler")
            .state
            .last_learning_rate = Some(0.008);

        let groups = vec![adam, lars, lamb];
        let bundle = PortableModelBundle::from_training_groups(
            "weather-agent",
            "optimizer-r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/optimizer")),
            groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;

        let (bytes, receipt) = bundle.export_safetensors()?;
        assert_eq!(receipt.format, ModelArtifactFormat::Safetensors);
        assert_eq!(receipt.tensor_count, 8);

        let imported = PortableModelBundle::import_safetensors(bytes.as_slice())?;
        assert_eq!(imported.to_training_groups()?, groups);
        Ok(())
    }

    #[test]
    fn portable_model_bundle_can_derive_and_remove_adapter_delta() -> Result<(), Box<dyn Error>> {
        let base_groups = sample_training_groups()?;
        let mut tuned_groups = sample_training_groups()?;

        for group in &mut tuned_groups {
            if group.group_id == "decoder.head" {
                let TensorData::F32(values) = &mut group.parameter.data else {
                    panic!("expected dense parameter tensor");
                };
                values[0] += 0.5;
                values[1] -= 0.25;
            }
        }

        let base = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            base_groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;
        let tuned = PortableModelBundle::from_training_groups(
            "weather-agent",
            "r1",
            "weather-checkpoints",
            Some(String::from("checkpoint://weather/7")),
            tuned_groups.as_slice(),
            sample_tokenizer_binding(),
            Some(String::from("chat-template-digest")),
        )?;

        let delta = PortableModelStateDict::derive_adapter_delta(
            &base.state_dict,
            &tuned.state_dict,
            "weather-head-delta",
        )?;
        let merged = base.state_dict.apply_adapter_delta(&delta)?;
        assert_eq!(merged.digest, tuned.state_dict.digest);

        let unmerged = merged.remove_adapter_delta(&delta)?;
        assert_eq!(unmerged.digest, base.state_dict.digest);
        assert_eq!(
            PortableModelBundle {
                state_dict: unmerged,
                tokenizer: base.tokenizer.clone(),
                profile_contract: base.profile_contract.clone(),
                chat_template_digest: base.chat_template_digest.clone(),
                preferred_serving_formats: base.preferred_serving_formats.clone(),
            }
            .to_training_groups()?,
            base_groups
        );
        Ok(())
    }

    #[test]
    fn gguf_import_surfaces_tokenizer_binding_and_tensor_inventory() -> Result<(), Box<dyn Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("weather.gguf");
        fs::write(
            &path,
            build_test_gguf(&sample_gguf_metadata(), &sample_gguf_tensors())?,
        )?;

        let (bundle, receipt) = PortableModelBundle::import_gguf_path(
            &path,
            "weather-agent",
            "gguf-r1",
            "weather-serve",
            "gguf-2026-03-14",
        )?;

        assert_eq!(receipt.format, ModelArtifactFormat::Gguf);
        assert_eq!(bundle.state_dict.source_format, ModelArtifactFormat::Gguf);
        assert_eq!(
            bundle.tokenizer.asset_format,
            PortableTokenizerAssetFormat::GgufMetadata
        );
        assert_eq!(
            bundle.tokenizer.digest.family,
            TokenizerFamily::BytePairEncoding
        );
        assert!(bundle.chat_template_digest.is_some());
        assert_eq!(
            bundle.chat_template_digest,
            bundle.tokenizer.digest.template_digest
        );
        assert_eq!(bundle.state_dict.tensors.len(), 1);
        let dense = bundle
            .state_dict
            .tensors
            .get("output.weight")
            .expect("missing output.weight");
        assert_eq!(dense.manifest.role, ModelStateTensorRole::Parameter);
        assert_eq!(
            dense.manifest.model_tree_path,
            vec![String::from("output"), String::from("weight")]
        );
        let TensorData::F32(values) = &dense.data else {
            panic!("expected dense GGUF tensor");
        };
        assert_eq!(values, &vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    fn sample_training_groups() -> Result<Vec<TrainingParameterGroupState>, Box<dyn Error>> {
        let mut embedding = TrainingParameterGroupState::new(
            "embedding",
            TrainingParameterClass::Embedding,
            TrainingTensorBuffer::from_f32(
                "embedding",
                TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                vec![1.0, 2.0, 3.0, 4.0],
            )?,
            TrainingOptimizerConfig::sgd(0.1).with_momentum(0.9),
            TrainingOptimizerResidencyPolicy::new(
                OptimizerStateResidency::DeviceResident,
                OptimizerStateResidency::HostResident,
            ),
        )?;
        embedding.optimizer_state = TrainingOptimizerState::Sgd {
            momentum_buffer: Some(vec![0.1, 0.2, 0.3, 0.4]),
        };
        embedding.optimizer_residency = OptimizerStateResidency::DeviceResident;
        embedding.applied_steps = 3;

        let mut decoder_head = TrainingParameterGroupState::new(
            "decoder.head",
            TrainingParameterClass::Head,
            TrainingTensorBuffer::from_f32(
                "decoder.head",
                TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                vec![0.5, -0.5],
            )?,
            TrainingOptimizerConfig::adamw(0.01, 0.9, 0.999, 1e-8).with_weight_decay(0.01),
            TrainingOptimizerResidencyPolicy::new(
                OptimizerStateResidency::DeviceResident,
                OptimizerStateResidency::Offloaded,
            ),
        )?;
        decoder_head.optimizer_state = TrainingOptimizerState::AdamW {
            first_moment: vec![0.01, -0.02],
            second_moment: vec![0.03, 0.04],
        };
        decoder_head.optimizer_residency = OptimizerStateResidency::Offloaded;
        decoder_head.applied_steps = 7;

        Ok(vec![embedding, decoder_head])
    }

    fn sample_tokenizer_binding() -> PortableTokenizerBinding {
        PortableTokenizerBinding::new(
            TokenizerDigest::new(
                TokenizerFamily::BytePairEncoding,
                "weather-tokenizer-digest",
                32_000,
            )
            .with_special_tokens_digest("weather-tokenizer-specials")
            .with_template_digest("chat-template-digest"),
            PortableTokenizerAssetFormat::TokenizerJson,
            "2026-03-14",
        )
        .with_special_tokens(Some(1), vec![2], Some(0), Some(3), true, false)
    }

    fn sample_profile_contract() -> PortableModelProfileContract {
        let mut shared_capabilities = BTreeMap::new();
        shared_capabilities.insert(String::from("grouped_query_attention"), true);
        shared_capabilities.insert(String::from("partial_rope"), true);

        let mut overlay_requirements = BTreeMap::new();
        overlay_requirements.insert(String::from("exact_sp1024_tokenizer"), false);
        overlay_requirements.insert(String::from("score_first_ttt"), false);

        PortableModelProfileContract {
            profile_id: String::from("psion_small_decoder_pgolf_core_v0"),
            family_id: String::from("parameter_golf_decoder"),
            baseline_model_id: String::from("parameter-golf-sp1024-9x512"),
            baseline_revision: String::from("public-2026-03-18"),
            profile_kind: String::from("general_psion_small_decoder"),
            shared_capabilities,
            overlay_requirements,
        }
    }

    fn sample_gguf_metadata() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("llama")),
            ),
            (
                String::from("general.alignment"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<pad>")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("<eos>")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.merges"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("h e")),
                    GgufMetadataValue::String(String::from("he llo")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(true),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("tokenizer.ggml.padding_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.chat_template"),
                GgufMetadataValue::String(String::from(
                    "{{ bos_token }}{{ messages[0]['content'] }}{{ eos_token }}",
                )),
            ),
        ]
    }

    fn sample_gguf_tensors() -> Vec<TestGgufTensor> {
        vec![TestGgufTensor::new(
            "output.weight",
            vec![2, 2],
            GgufTensorType::F32,
            encode_f32_bytes(&[1.0, 2.0, 3.0, 4.0]),
        )]
    }

    #[derive(Clone, Debug)]
    struct TestGgufTensor {
        name: String,
        shape: Vec<usize>,
        tensor_type: GgufTensorType,
        bytes: Vec<u8>,
    }

    impl TestGgufTensor {
        fn new(
            name: impl Into<String>,
            shape: Vec<usize>,
            tensor_type: GgufTensorType,
            bytes: Vec<u8>,
        ) -> Self {
            Self {
                name: name.into(),
                shape,
                tensor_type,
                bytes,
            }
        }
    }

    fn build_test_gguf(
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let alignment = metadata
            .iter()
            .find(|(key, _)| key == "general.alignment")
            .and_then(|(_, value)| value.as_u64())
            .unwrap_or(32)
            .max(1);

        let mut bytes = Vec::new();
        bytes.extend(b"GGUF");
        push_u32(&mut bytes, gguf_version_code(GgufVersion::V3));
        push_u64(&mut bytes, u64::try_from(tensors.len())?);
        push_u64(&mut bytes, u64::try_from(metadata.len())?);

        for (key, value) in metadata {
            push_gguf_string(&mut bytes, key)?;
            push_u32(&mut bytes, gguf_metadata_value_type(value));
            push_gguf_value(&mut bytes, value)?;
        }

        let mut next_offset = 0_usize;
        let mut tensor_offsets = Vec::with_capacity(tensors.len());
        for tensor in tensors {
            tensor_offsets.push(next_offset);
            next_offset = align_usize(next_offset + tensor.bytes.len(), alignment as usize);
        }

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            push_gguf_string(&mut bytes, tensor.name.as_str())?;
            push_u32(&mut bytes, u32::try_from(tensor.shape.len())?);
            for dimension in tensor.shape.iter().rev() {
                push_u64(&mut bytes, u64::try_from(*dimension)?);
            }
            push_u32(&mut bytes, gguf_tensor_type_code(tensor.tensor_type));
            push_u64(&mut bytes, u64::try_from(*offset)?);
        }

        let tensor_data_offset = align_u64(bytes.len() as u64, alignment);
        bytes.resize(tensor_data_offset as usize, 0);
        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            let start = tensor_data_offset as usize + offset;
            if bytes.len() < start {
                bytes.resize(start, 0);
            }
            bytes.extend_from_slice(&tensor.bytes);
            bytes.resize(align_usize(bytes.len(), alignment as usize), 0);
        }
        Ok(bytes)
    }

    fn push_gguf_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Box<dyn Error>> {
        push_u64(bytes, u64::try_from(value.len())?);
        bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn push_gguf_value(
        bytes: &mut Vec<u8>,
        value: &GgufMetadataValue,
    ) -> Result<(), Box<dyn Error>> {
        match value {
            GgufMetadataValue::U8(value) => bytes.push(*value),
            GgufMetadataValue::I8(value) => bytes.push(value.to_le_bytes()[0]),
            GgufMetadataValue::U16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::Bool(value) => bytes.push(u8::from(*value)),
            GgufMetadataValue::String(value) => push_gguf_string(bytes, value)?,
            GgufMetadataValue::Array(values) => {
                let value_type = values.first().map_or(4, gguf_metadata_value_type);
                push_u32(bytes, value_type);
                push_u64(bytes, u64::try_from(values.len())?);
                for value in values {
                    push_gguf_value(bytes, value)?;
                }
            }
        }
        Ok(())
    }

    fn gguf_metadata_value_type(value: &GgufMetadataValue) -> u32 {
        match value {
            GgufMetadataValue::U8(_) => 0,
            GgufMetadataValue::I8(_) => 1,
            GgufMetadataValue::U16(_) => 2,
            GgufMetadataValue::I16(_) => 3,
            GgufMetadataValue::U32(_) => 4,
            GgufMetadataValue::I32(_) => 5,
            GgufMetadataValue::F32(_) => 6,
            GgufMetadataValue::Bool(_) => 7,
            GgufMetadataValue::String(_) => 8,
            GgufMetadataValue::Array(_) => 9,
            GgufMetadataValue::U64(_) => 10,
            GgufMetadataValue::I64(_) => 11,
            GgufMetadataValue::F64(_) => 12,
        }
    }

    fn gguf_tensor_type_code(tensor_type: GgufTensorType) -> u32 {
        match tensor_type {
            GgufTensorType::F32 => 0,
            GgufTensorType::F16 => 1,
            GgufTensorType::Q4_0 => 2,
            GgufTensorType::Q4_1 => 3,
            GgufTensorType::Q5_0 => 6,
            GgufTensorType::Q5_1 => 7,
            GgufTensorType::Q8_0 => 8,
            GgufTensorType::Q8_1 => 9,
            GgufTensorType::Q2K => 10,
            GgufTensorType::Q3K => 11,
            GgufTensorType::Q4K => 12,
            GgufTensorType::Q5K => 13,
            GgufTensorType::Q6K => 14,
            GgufTensorType::Q8K => 15,
            GgufTensorType::IQ3S => 21,
            GgufTensorType::IQ4XS => 23,
            GgufTensorType::BF16 => 30,
            GgufTensorType::MXFP4 => 39,
            GgufTensorType::Unknown(value) => value,
        }
    }

    fn gguf_version_code(version: GgufVersion) -> u32 {
        match version {
            GgufVersion::V1 => 1,
            GgufVersion::V2 => 2,
            GgufVersion::V3 => 3,
        }
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }

    fn align_u64(value: u64, alignment: u64) -> u64 {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + (alignment - remainder)
        }
    }

    fn align_usize(value: usize, alignment: usize) -> usize {
        align_u64(value as u64, alignment as u64) as usize
    }
}
