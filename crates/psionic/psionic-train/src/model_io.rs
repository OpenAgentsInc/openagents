use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use psionic_core::{DType, Device, QuantizedTensorData, TensorData, TensorSpec};
use psionic_data::{TokenizerDigest, TokenizerFamily};
use psionic_models::{
    GgufContent, GgufMetadataValue, GgufTensorType, GgufTokenizerMetadata, GgufTokenizerModel,
    GgufVersion, GgufWeightBundleLoader, LocalWeightBundleLoader, ModelLoadError,
    WeightTensorStorage,
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
    TrainingParameterGroupState, TrainingTensorBuffer, core_loop::TrainingCoreError,
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
    /// SGD momentum buffer.
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
                optimizer_residency_policy: group.optimizer_residency_policy,
                optimizer_residency: group.optimizer_residency,
                applied_steps: group.applied_steps,
                parameter_key,
                momentum_buffer_key: None,
                first_moment_key: None,
                second_moment_key: None,
            };

            match &group.optimizer_state {
                TrainingOptimizerState::Sgd { momentum_buffer } => {
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
                TrainingOptimizerState::AdamW {
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

            let optimizer_state = match assignment.optimizer.kind {
                TrainingOptimizerKind::Sgd => {
                    let momentum_buffer = assignment
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
                    TrainingOptimizerState::Sgd { momentum_buffer }
                }
                TrainingOptimizerKind::AdamW => {
                    let first_moment_key =
                        assignment.first_moment_key.as_ref().ok_or_else(|| {
                            ModelIoError::InvalidGroupAssignment {
                                group_id: assignment.group_id.clone(),
                                message: String::from("AdamW group is missing `first_moment_key`"),
                            }
                        })?;
                    let second_moment_key =
                        assignment.second_moment_key.as_ref().ok_or_else(|| {
                            ModelIoError::InvalidGroupAssignment {
                                group_id: assignment.group_id.clone(),
                                message: String::from("AdamW group is missing `second_moment_key`"),
                            }
                        })?;
                    TrainingOptimizerState::AdamW {
                        first_moment: dense_f32_values(
                            first_moment_key.as_str(),
                            self.tensors.get(first_moment_key.as_str()).ok_or_else(|| {
                                ModelIoError::MissingTensor {
                                    state_key: first_moment_key.clone(),
                                }
                            })?,
                        )?
                        .to_vec(),
                        second_moment: dense_f32_values(
                            second_moment_key.as_str(),
                            self.tensors
                                .get(second_moment_key.as_str())
                                .ok_or_else(|| ModelIoError::MissingTensor {
                                    state_key: second_moment_key.clone(),
                                })?,
                        )?
                        .to_vec(),
                    }
                }
            };

            groups.push(TrainingParameterGroupState {
                group_id: assignment.group_id.clone(),
                class: assignment.class,
                parameter,
                optimizer: assignment.optimizer.clone(),
                optimizer_state,
                optimizer_residency_policy: assignment.optimizer_residency_policy,
                optimizer_residency: assignment.optimizer_residency,
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
}

/// Metadata-only form embedded inside safetensors artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableModelBundleManifest {
    /// State-dict manifest without raw payloads.
    pub state_dict: PortableModelStateDictManifest,
    /// Bound tokenizer portability contract.
    pub tokenizer: PortableTokenizerBinding,
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
            chat_template_digest: self.chat_template_digest.clone(),
            preferred_serving_formats: self.preferred_serving_formats.clone(),
        }
    }

    /// Reconstructs training-core groups from the bundle.
    pub fn to_training_groups(&self) -> Result<Vec<TrainingParameterGroupState>, ModelIoError> {
        self.state_dict.to_training_groups()
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
        Ok(Self {
            state_dict,
            tokenizer: artifact.tokenizer,
            chat_template_digest: artifact.chat_template_digest,
            preferred_serving_formats: artifact.preferred_serving_formats,
        })
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
            let spec = &entry.manifest.spec;
            if spec.storage_size() != spec.element_count() {
                return Err(ModelIoError::NonContiguousSafetensorsTensor {
                    state_key: state_key.clone(),
                });
            }
            let values = match &entry.data {
                TensorData::F32(values) if spec.dtype() == DType::F32 => values,
                _ => {
                    return Err(ModelIoError::UnsupportedSafetensorsTensor {
                        state_key: state_key.clone(),
                    });
                }
            };
            raw_buffers.push((
                state_key.clone(),
                encode_f32_bytes(values),
                spec.shape().dims().to_vec(),
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

        let safetensors = SafeTensors::deserialize(bytes).map_err(safetensors_error)?;
        let mut tensors = BTreeMap::new();
        for (state_key, tensor_manifest) in &manifest.state_dict.tensors {
            let view = safetensors
                .tensor(state_key.as_str())
                .map_err(safetensors_error)?;
            if view.shape() != tensor_manifest.spec.shape().dims() {
                return Err(ModelIoError::SafetensorsShapeMismatch {
                    state_key: state_key.clone(),
                    expected: tensor_manifest.spec.shape().dims().to_vec(),
                    actual: view.shape().to_vec(),
                });
            }
            if view.dtype() != SafeTensorsDType::F32 || tensor_manifest.spec.dtype() != DType::F32 {
                return Err(ModelIoError::SafetensorsDTypeMismatch {
                    state_key: state_key.clone(),
                    expected: tensor_manifest.spec.dtype(),
                    actual: view.dtype().to_string(),
                });
            }
            let values = decode_f32_bytes(state_key.as_str(), view.data().as_ref())?;
            tensors.insert(
                state_key.clone(),
                ModelStateTensorEntry {
                    manifest: tensor_manifest.clone(),
                    data: TensorData::F32(values),
                },
            );
        }

        let state_dict = PortableModelStateDict::new(
            manifest.state_dict.model_family,
            manifest.state_dict.revision,
            manifest.state_dict.checkpoint_family,
            manifest.state_dict.checkpoint_ref,
            ModelArtifactFormat::Safetensors,
            manifest.state_dict.groups,
            tensors,
        )?;

        Ok(Self {
            state_dict,
            tokenizer: manifest.tokenizer,
            chat_template_digest: manifest.chat_template_digest,
            preferred_serving_formats: manifest.preferred_serving_formats,
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
            TrainingOptimizerKind::Sgd => {
                if group.first_moment_key.is_some() || group.second_moment_key.is_some() {
                    return Err(ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: String::from(
                            "SGD group may not carry Adam first/second moment keys",
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
            TrainingOptimizerKind::AdamW => {
                if group.momentum_buffer_key.is_some() {
                    return Err(ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: String::from(
                            "AdamW group may not carry an SGD momentum buffer key",
                        ),
                    });
                }
                let first_moment_key = group.first_moment_key.as_ref().ok_or_else(|| {
                    ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: String::from("AdamW group is missing `first_moment_key`"),
                    }
                })?;
                let second_moment_key = group.second_moment_key.as_ref().ok_or_else(|| {
                    ModelIoError::InvalidGroupAssignment {
                        group_id: group.group_id.clone(),
                        message: String::from("AdamW group is missing `second_moment_key`"),
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
        TensorData::F32(values) => Ok(values.as_slice()),
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
        TensorData::F32(values) => Ok(values.as_mut_slice()),
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
        TensorData::QuantizedBlocks(quantized) => {
            hasher.update(b"quantized|");
            hasher.update(stable_json_bytes(quantized));
        }
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

    use psionic_core::Shape;
    use tempfile::tempdir;

    use super::*;

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
        )?;

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
        )?;

        let (bytes, receipt) = bundle.export_safetensors()?;
        assert_eq!(receipt.format, ModelArtifactFormat::Safetensors);

        let imported = PortableModelBundle::import_safetensors(bytes.as_slice())?;
        assert_eq!(
            imported.state_dict.source_format,
            ModelArtifactFormat::Safetensors
        );
        assert_eq!(imported.to_training_groups()?, groups);
        assert_eq!(imported.tokenizer, bundle.tokenizer);
        assert_eq!(imported.chat_template_digest, bundle.chat_template_digest);
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
