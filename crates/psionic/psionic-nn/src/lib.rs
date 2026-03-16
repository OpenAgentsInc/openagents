//! Reusable module, parameter, buffer, state-tree, and bounded layer semantics
//! for Psionic.

mod layers;
mod training;

use std::collections::BTreeMap;

use psionic_core::{
    DType, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, QuantizationMode, TensorData,
    TensorSpec,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use layers::*;
pub use training::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "module, parameter, buffer, state-tree, bounded layer, and training-helper semantics";

/// Error returned when a module tree or state entry violates Psionic-nn rules.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleStateError {
    /// The root module identifier was blank.
    #[error("module tree is missing a module_id")]
    MissingModuleId,
    /// The root module kind was blank.
    #[error("module tree is missing a module_kind")]
    MissingModuleKind,
    /// One local parameter, buffer, or submodule name was blank or path-like.
    #[error("module tree local name `{name}` must be non-empty and may not contain `.`")]
    InvalidLocalName {
        /// Invalid local name.
        name: String,
    },
    /// One local name would shadow a parameter, buffer, or submodule already present.
    #[error("module tree already contains local name `{name}`")]
    DuplicateLocalName {
        /// Duplicated local name.
        name: String,
    },
    /// A requested submodule path does not exist.
    #[error("module tree does not contain submodule path `{path}`")]
    UnknownSubmodulePath {
        /// Missing submodule path.
        path: String,
    },
    /// A requested parameter path does not exist.
    #[error("module tree does not contain parameter path `{path}`")]
    MissingParameter {
        /// Missing parameter path.
        path: String,
    },
    /// A requested buffer path does not exist.
    #[error("module tree does not contain buffer path `{path}`")]
    MissingBuffer {
        /// Missing buffer path.
        path: String,
    },
    /// A dense payload length does not match the tensor storage length.
    #[error("module tensor `{owner}` expected {expected_len} dense values but found {actual_len}")]
    DensePayloadLengthMismatch {
        /// State owner name.
        owner: String,
        /// Expected dense length.
        expected_len: usize,
        /// Actual dense length.
        actual_len: usize,
    },
    /// A quantized payload used a dtype that cannot act as the logical view.
    #[error("module tensor `{owner}` does not allow quantized payload for dtype {dtype:?}")]
    QuantizedPayloadUnsupportedDType {
        /// State owner name.
        owner: String,
        /// Logical dtype.
        dtype: DType,
    },
    /// A quantized payload element count does not match the logical tensor shape.
    #[error(
        "module tensor `{owner}` expected {expected_elements} logical elements but quantized payload surfaced {actual_elements}"
    )]
    QuantizedPayloadElementCountMismatch {
        /// State owner name.
        owner: String,
        /// Expected logical element count.
        expected_elements: usize,
        /// Actual logical element count.
        actual_elements: usize,
    },
    /// One externally supplied state-dict entry key did not match its embedded path.
    #[error(
        "module state dict map key `{map_key}` does not match embedded entry path `{entry_path}`"
    )]
    StateEntryPathMismatch {
        /// Stable state-dict map key.
        map_key: String,
        /// Embedded entry path.
        entry_path: String,
    },
}

/// One trainable parameter entry owned by a module tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleParameter {
    /// Tensor metadata.
    pub spec: TensorSpec,
    /// Stored tensor payload.
    pub data: TensorData,
    /// Whether the parameter participates in gradient-bearing update paths.
    pub requires_grad: bool,
}

impl ModuleParameter {
    /// Creates a validated parameter entry.
    pub fn new(
        spec: TensorSpec,
        data: TensorData,
        requires_grad: bool,
    ) -> Result<Self, ModuleStateError> {
        validate_tensor_payload("parameter", &spec, &data)?;
        Ok(Self {
            spec,
            data,
            requires_grad,
        })
    }

    /// Returns whether the parameter is currently frozen.
    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        !self.requires_grad
    }
}

/// One buffer entry owned by a module tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleBuffer {
    /// Tensor metadata.
    pub spec: TensorSpec,
    /// Stored tensor payload.
    pub data: TensorData,
    /// Whether the buffer should appear in persistent state-tree views.
    pub persistent: bool,
}

impl ModuleBuffer {
    /// Creates a validated buffer entry.
    pub fn new(
        spec: TensorSpec,
        data: TensorData,
        persistent: bool,
    ) -> Result<Self, ModuleStateError> {
        validate_tensor_payload("buffer", &spec, &data)?;
        Ok(Self {
            spec,
            data,
            persistent,
        })
    }
}

/// Declared state-tree view over module buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStateView {
    /// Parameters plus buffers marked persistent.
    PersistentOnly,
    /// Parameters plus all buffers, including non-persistent scratch or stats buffers.
    AllBuffers,
}

/// Flattened state-tree entry type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStateEntryKind {
    /// Trainable parameter entry.
    Parameter,
    /// Buffer entry.
    Buffer,
}

/// Declared parameter traversal posture over trainable versus frozen entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleParameterView {
    /// Traverse every parameter regardless of trainability.
    All,
    /// Traverse only parameters that still require gradients.
    TrainableOnly,
    /// Traverse only parameters that are currently frozen.
    FrozenOnly,
}

/// One flattened state-tree entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleStateEntry {
    /// Stable dot-separated path from the root module.
    pub path: String,
    /// Entry kind.
    pub kind: ModuleStateEntryKind,
    /// Tensor metadata.
    pub spec: TensorSpec,
    /// Tensor payload.
    pub data: TensorData,
    /// Whether gradients are expected for this entry.
    pub requires_grad: bool,
    /// Whether the entry is persistent in the module state tree.
    pub persistent: bool,
}

/// Flattened state-tree view for one module graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleStateTree {
    /// Stable root module identifier.
    pub root_module_id: String,
    /// Human-readable root module kind.
    pub root_module_kind: String,
    /// View used to build the flattened state tree.
    pub view: ModuleStateView,
    /// Flattened entries in deterministic path order.
    pub entries: Vec<ModuleStateEntry>,
    /// Stable digest over the state tree contents.
    pub state_tree_digest: String,
}

impl ModuleStateTree {
    fn new(
        root_module_id: String,
        root_module_kind: String,
        view: ModuleStateView,
        entries: Vec<ModuleStateEntry>,
    ) -> Self {
        let state_tree_digest = stable_module_state_tree_digest(
            root_module_id.as_str(),
            root_module_kind.as_str(),
            view,
            entries.as_slice(),
        );
        Self {
            root_module_id,
            root_module_kind,
            view,
            entries,
            state_tree_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("root_module_id={}", self.root_module_id),
            format!("root_module_kind={}", self.root_module_kind),
            format!("view={:?}", self.view),
            format!("state_tree_digest={}", self.state_tree_digest),
        ];
        for entry in &self.entries {
            lines.push(format!(
                "{}|{:?}|persistent={}|requires_grad={}",
                entry.path, entry.kind, entry.persistent, entry.requires_grad
            ));
        }
        lines
    }
}

/// Stable `state_dict` load mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStateLoadMode {
    /// Missing or unexpected keys refuse the load.
    Strict,
    /// Missing or unexpected keys are reported but compatible overlaps still load.
    NonStrict,
}

/// Keyed `state_dict` view for one module tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleStateDict {
    /// Stable root module identifier.
    pub root_module_id: String,
    /// Human-readable root module kind.
    pub root_module_kind: String,
    /// View used to build the keyed state dict.
    pub view: ModuleStateView,
    /// Flattened entries keyed by deterministic dot-separated path.
    pub entries: BTreeMap<String, ModuleStateEntry>,
    /// Stable digest over the keyed state dict contents.
    pub state_dict_digest: String,
}

impl ModuleStateDict {
    /// Creates a validated keyed state-dict view.
    pub fn new(
        root_module_id: impl Into<String>,
        root_module_kind: impl Into<String>,
        view: ModuleStateView,
        entries: BTreeMap<String, ModuleStateEntry>,
    ) -> Result<Self, ModuleStateError> {
        let root_module_id = root_module_id.into();
        if root_module_id.trim().is_empty() {
            return Err(ModuleStateError::MissingModuleId);
        }
        let root_module_kind = root_module_kind.into();
        if root_module_kind.trim().is_empty() {
            return Err(ModuleStateError::MissingModuleKind);
        }
        for (path, entry) in &entries {
            if path != &entry.path {
                return Err(ModuleStateError::StateEntryPathMismatch {
                    map_key: path.clone(),
                    entry_path: entry.path.clone(),
                });
            }
        }
        let state_dict_digest = stable_module_state_tree_digest(
            root_module_id.as_str(),
            root_module_kind.as_str(),
            view,
            &entries.values().cloned().collect::<Vec<_>>(),
        );
        Ok(Self {
            root_module_id,
            root_module_kind,
            view,
            entries,
            state_dict_digest,
        })
    }

    fn from_state_tree(tree: ModuleStateTree) -> Self {
        let entries = tree
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        Self {
            root_module_id: tree.root_module_id,
            root_module_kind: tree.root_module_kind,
            view: tree.view,
            entries,
            state_dict_digest: tree.state_tree_digest,
        }
    }

    /// Returns the deterministic state-dict key order.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Returns one keyed entry.
    #[must_use]
    pub fn entry(&self, path: &str) -> Option<&ModuleStateEntry> {
        self.entries.get(path)
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("root_module_id={}", self.root_module_id),
            format!("root_module_kind={}", self.root_module_kind),
            format!("view={:?}", self.view),
            format!("state_dict_digest={}", self.state_dict_digest),
        ];
        for (path, entry) in &self.entries {
            lines.push(format!(
                "{path}|{:?}|persistent={}|requires_grad={}",
                entry.kind, entry.persistent, entry.requires_grad
            ));
        }
        lines
    }
}

/// Successful `state_dict` load summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStateLoadReport {
    /// Load mode used by the caller.
    pub mode: ModuleStateLoadMode,
    /// State-dict view that governed expected keys.
    pub view: ModuleStateView,
    /// Source keyed state-dict digest.
    pub source_state_dict_digest: String,
    /// Target keyed state-dict digest before mutation.
    pub target_before_digest: String,
    /// Target keyed state-dict digest after mutation.
    pub target_after_digest: String,
    /// Deterministic paths that were loaded into the target module.
    pub loaded_paths: Vec<String>,
    /// Missing target keys tolerated under non-strict load.
    pub missing_keys: Vec<String>,
    /// Extra source keys tolerated under non-strict load.
    pub unexpected_keys: Vec<String>,
}

impl ModuleStateLoadReport {
    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("mode={:?}", self.mode),
            format!("view={:?}", self.view),
            format!("source_state_dict_digest={}", self.source_state_dict_digest),
            format!("target_before_digest={}", self.target_before_digest),
            format!("target_after_digest={}", self.target_after_digest),
        ];
        for path in &self.loaded_paths {
            lines.push(format!("loaded={path}"));
        }
        for path in &self.missing_keys {
            lines.push(format!("missing={path}"));
        }
        for path in &self.unexpected_keys {
            lines.push(format!("unexpected={path}"));
        }
        lines
    }
}

/// State-dict incompatibility that refused a load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleStateLoadError {
    /// Strict load found missing or unexpected keys.
    StrictKeyMismatch {
        /// Missing target keys.
        missing_keys: Vec<String>,
        /// Extra source keys.
        unexpected_keys: Vec<String>,
    },
    /// The source and target disagree on parameter-vs-buffer identity.
    EntryKindMismatch {
        /// Stable state-dict key.
        path: String,
        /// Expected target kind.
        expected: ModuleStateEntryKind,
        /// Actual source kind.
        actual: ModuleStateEntryKind,
    },
    /// The source and target disagree on tensor shape.
    ShapeMismatch {
        /// Stable state-dict key.
        path: String,
        /// Expected target shape.
        expected: Vec<usize>,
        /// Actual source shape.
        actual: Vec<usize>,
    },
    /// The source and target disagree on logical dtype.
    DTypeMismatch {
        /// Stable state-dict key.
        path: String,
        /// Expected target dtype.
        expected: DType,
        /// Actual source dtype.
        actual: DType,
    },
    /// One incoming source payload was structurally malformed for the target spec.
    InvalidSourcePayload {
        /// Stable state-dict key.
        path: String,
        /// Lower-level payload validation error.
        source: ModuleStateError,
    },
    /// The validated target path disappeared before mutation could be applied.
    MissingTargetPath {
        /// Stable state-dict key.
        path: String,
        /// Lower-level missing-path error.
        source: ModuleStateError,
    },
}

impl std::fmt::Display for ModuleStateLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StrictKeyMismatch {
                missing_keys,
                unexpected_keys,
            } => write!(
                f,
                "strict module state load key mismatch: missing={missing_keys:?} unexpected={unexpected_keys:?}"
            ),
            Self::EntryKindMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "module state load entry kind mismatch for `{path}`: expected {expected:?}, found {actual:?}"
            ),
            Self::ShapeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "module state load shape mismatch for `{path}`: expected {expected:?}, found {actual:?}"
            ),
            Self::DTypeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "module state load dtype mismatch for `{path}`: expected {expected:?}, found {actual:?}"
            ),
            Self::InvalidSourcePayload { path, source } => {
                write!(
                    f,
                    "module state load payload for `{path}` is invalid: {source}"
                )
            }
            Self::MissingTargetPath { path, source } => {
                write!(
                    f,
                    "module state load target path `{path}` disappeared: {source}"
                )
            }
        }
    }
}

impl std::error::Error for ModuleStateLoadError {}

/// Outcome status for one module parity case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleParityStatus {
    /// The bounded module semantics matched the seeded expectation.
    Supported,
    /// The bounded module semantics refused explicitly.
    Refused,
}

/// One machine-readable seeded module parity case result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleParityCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable oracle family label.
    pub oracle_family: String,
    /// Stable module-kind label.
    pub module_kind: String,
    /// Stable capability-profile label.
    pub capability_profile: String,
    /// Buffer view used by this module parity case.
    pub view: ModuleStateView,
    /// Expected module paths under the bounded parity profile.
    pub expected_module_paths: Vec<String>,
    /// Actual module paths surfaced by the implementation.
    pub actual_module_paths: Vec<String>,
    /// Expected parameter paths under the bounded parity profile.
    pub expected_parameter_paths: Vec<String>,
    /// Actual parameter paths surfaced by the implementation.
    pub actual_parameter_paths: Vec<String>,
    /// Expected buffer paths under the bounded parity profile.
    pub expected_buffer_paths: Vec<String>,
    /// Actual buffer paths surfaced by the implementation.
    pub actual_buffer_paths: Vec<String>,
    /// Expected state-dict keys under the bounded parity profile.
    pub expected_state_dict_keys: Vec<String>,
    /// Actual state-dict keys surfaced by the implementation.
    pub actual_state_dict_keys: Vec<String>,
    /// Expected refusal when the case is intentionally unsupported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_refusal: Option<PsionicRefusal>,
    /// Actual refusal surfaced by the implementation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_refusal: Option<PsionicRefusal>,
    /// Stable parity outcome status.
    pub status: ModuleParityStatus,
}

/// Machine-readable seeded module parity matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleParityMatrixReport {
    /// Stable schema version for the parity matrix report.
    pub schema_version: u32,
    /// Stable oracle family window label.
    pub oracle_family_window: String,
    /// Seeded parity case results.
    pub cases: Vec<ModuleParityCaseResult>,
    /// Stable digest over the report contents.
    pub matrix_digest: String,
}

impl ModuleParityMatrixReport {
    fn new(oracle_family_window: impl Into<String>, cases: Vec<ModuleParityCaseResult>) -> Self {
        let oracle_family_window = oracle_family_window.into();
        let matrix_digest =
            stable_module_parity_matrix_digest(oracle_family_window.as_str(), cases.as_slice());
        Self {
            schema_version: 1,
            oracle_family_window,
            cases,
            matrix_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("oracle_family_window={}", self.oracle_family_window),
            format!("matrix_digest={}", self.matrix_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{}|{:?}",
                case.case_id, case.module_kind, case.status
            ));
        }
        lines
    }
}

/// One nested reusable module tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    /// Stable module identifier.
    pub module_id: String,
    /// Human-readable module kind such as `linear` or `transformer_block`.
    pub module_kind: String,
    /// Local parameters keyed by local name.
    pub parameters: BTreeMap<String, ModuleParameter>,
    /// Local buffers keyed by local name.
    pub buffers: BTreeMap<String, ModuleBuffer>,
    /// Local submodules keyed by local name.
    pub submodules: BTreeMap<String, Module>,
}

impl Module {
    /// Creates an empty module tree node.
    pub fn new(
        module_id: impl Into<String>,
        module_kind: impl Into<String>,
    ) -> Result<Self, ModuleStateError> {
        let module_id = module_id.into();
        if module_id.trim().is_empty() {
            return Err(ModuleStateError::MissingModuleId);
        }
        let module_kind = module_kind.into();
        if module_kind.trim().is_empty() {
            return Err(ModuleStateError::MissingModuleKind);
        }
        Ok(Self {
            module_id,
            module_kind,
            parameters: BTreeMap::new(),
            buffers: BTreeMap::new(),
            submodules: BTreeMap::new(),
        })
    }

    /// Inserts one local parameter.
    pub fn insert_parameter(
        &mut self,
        name: impl Into<String>,
        parameter: ModuleParameter,
    ) -> Result<(), ModuleStateError> {
        let name = name.into();
        validate_local_name(name.as_str())?;
        self.ensure_local_name_available(name.as_str())?;
        self.parameters.insert(name, parameter);
        Ok(())
    }

    /// Inserts one local buffer.
    pub fn insert_buffer(
        &mut self,
        name: impl Into<String>,
        buffer: ModuleBuffer,
    ) -> Result<(), ModuleStateError> {
        let name = name.into();
        validate_local_name(name.as_str())?;
        self.ensure_local_name_available(name.as_str())?;
        self.buffers.insert(name, buffer);
        Ok(())
    }

    /// Inserts one local submodule.
    pub fn insert_submodule(
        &mut self,
        name: impl Into<String>,
        submodule: Module,
    ) -> Result<(), ModuleStateError> {
        let name = name.into();
        validate_local_name(name.as_str())?;
        self.ensure_local_name_available(name.as_str())?;
        self.submodules.insert(name, submodule);
        Ok(())
    }

    /// Returns a nested submodule by dot-separated path. The empty path returns the current module.
    pub fn submodule(&self, path: &str) -> Result<&Self, ModuleStateError> {
        if path.trim().is_empty() {
            return Ok(self);
        }
        let mut current = self;
        for segment in path.split('.') {
            validate_local_name(segment)?;
            current = current.submodules.get(segment).ok_or_else(|| {
                ModuleStateError::UnknownSubmodulePath {
                    path: String::from(path),
                }
            })?;
        }
        Ok(current)
    }

    /// Returns a mutable nested submodule by dot-separated path. The empty path returns the current module.
    pub fn submodule_mut(&mut self, path: &str) -> Result<&mut Self, ModuleStateError> {
        if path.trim().is_empty() {
            return Ok(self);
        }
        let mut current = self;
        for segment in path.split('.') {
            validate_local_name(segment)?;
            current = current.submodules.get_mut(segment).ok_or_else(|| {
                ModuleStateError::UnknownSubmodulePath {
                    path: String::from(path),
                }
            })?;
        }
        Ok(current)
    }

    /// Returns a parameter by dot-separated path.
    pub fn parameter(&self, path: &str) -> Result<&ModuleParameter, ModuleStateError> {
        let (module_path, local_name) = split_state_path(path)?;
        let module = self.submodule(module_path.as_str())?;
        module.parameters.get(local_name.as_str()).ok_or_else(|| {
            ModuleStateError::MissingParameter {
                path: String::from(path),
            }
        })
    }

    /// Returns a buffer by dot-separated path.
    pub fn buffer(&self, path: &str) -> Result<&ModuleBuffer, ModuleStateError> {
        let (module_path, local_name) = split_state_path(path)?;
        let module = self.submodule(module_path.as_str())?;
        module
            .buffers
            .get(local_name.as_str())
            .ok_or_else(|| ModuleStateError::MissingBuffer {
                path: String::from(path),
            })
    }

    /// Returns deterministic named parameter traversal.
    #[must_use]
    pub fn named_parameters(&self) -> Vec<(String, &ModuleParameter)> {
        self.named_parameters_with_view(ModuleParameterView::All)
    }

    /// Returns deterministic named parameter traversal for one trainability view.
    #[must_use]
    pub fn named_parameters_with_view(
        &self,
        view: ModuleParameterView,
    ) -> Vec<(String, &ModuleParameter)> {
        let mut entries = Vec::new();
        self.collect_named_parameters("", view, &mut entries);
        entries
    }

    /// Sets one parameter's gradient posture by dot-separated path and returns
    /// whether the flag changed.
    pub fn set_parameter_requires_grad(
        &mut self,
        path: &str,
        requires_grad: bool,
    ) -> Result<bool, ModuleStateError> {
        let parameter = self.parameter_mut(path)?;
        let changed = parameter.requires_grad != requires_grad;
        parameter.requires_grad = requires_grad;
        Ok(changed)
    }

    /// Freezes every parameter in the current module tree and returns the
    /// number of parameters whose posture changed.
    pub fn freeze(&mut self) -> usize {
        self.set_requires_grad_recursive(false)
    }

    /// Unfreezes every parameter in the current module tree and returns the
    /// number of parameters whose posture changed.
    pub fn unfreeze(&mut self) -> usize {
        self.set_requires_grad_recursive(true)
    }

    /// Freezes one nested submodule by dot-separated path and returns the
    /// number of parameters whose posture changed.
    pub fn freeze_submodule(&mut self, path: &str) -> Result<usize, ModuleStateError> {
        Ok(self.submodule_mut(path)?.set_requires_grad_recursive(false))
    }

    /// Unfreezes one nested submodule by dot-separated path and returns the
    /// number of parameters whose posture changed.
    pub fn unfreeze_submodule(&mut self, path: &str) -> Result<usize, ModuleStateError> {
        Ok(self.submodule_mut(path)?.set_requires_grad_recursive(true))
    }

    /// Returns deterministic named buffer traversal for one state-tree view.
    #[must_use]
    pub fn named_buffers(&self, view: ModuleStateView) -> Vec<(String, &ModuleBuffer)> {
        let mut entries = Vec::new();
        self.collect_named_buffers("", view, &mut entries);
        entries
    }

    /// Returns deterministic named module traversal, including the current root at `\"\"`.
    #[must_use]
    pub fn named_modules(&self) -> Vec<(String, &Module)> {
        let mut entries = Vec::new();
        self.collect_named_modules("", &mut entries);
        entries
    }

    /// Returns a flattened state tree for one view.
    #[must_use]
    pub fn state_tree(&self, view: ModuleStateView) -> ModuleStateTree {
        let mut entries = Vec::new();
        self.collect_state_entries("", view, &mut entries);
        ModuleStateTree::new(
            self.module_id.clone(),
            self.module_kind.clone(),
            view,
            entries,
        )
    }

    /// Returns a stable digest over the full module contents.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        self.state_tree(ModuleStateView::AllBuffers)
            .state_tree_digest
    }

    /// Returns the default persistent-only keyed state dict.
    #[must_use]
    pub fn state_dict(&self) -> ModuleStateDict {
        self.state_dict_with_view(ModuleStateView::PersistentOnly)
    }

    /// Returns a keyed state dict for one buffer view.
    #[must_use]
    pub fn state_dict_with_view(&self, view: ModuleStateView) -> ModuleStateDict {
        ModuleStateDict::from_state_tree(self.state_tree(view))
    }

    /// Saves the default persistent-only weights view for the current module
    /// tree.
    #[must_use]
    pub fn save_weights(&self) -> ModuleStateDict {
        self.state_dict()
    }

    /// Saves one explicit weights view for the current module tree.
    #[must_use]
    pub fn save_weights_with_view(&self, view: ModuleStateView) -> ModuleStateDict {
        self.state_dict_with_view(view)
    }

    /// Loads one saved weights view using strict matching by default.
    pub fn load_weights(
        &mut self,
        weights: &ModuleStateDict,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        self.load_weights_with_mode(weights, ModuleStateLoadMode::Strict)
    }

    /// Loads one saved weights view using the requested strict or non-strict
    /// posture.
    pub fn load_weights_with_mode(
        &mut self,
        weights: &ModuleStateDict,
        mode: ModuleStateLoadMode,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        self.load_state_dict(weights, mode)
    }

    /// Loads one keyed state dict into the current module tree.
    pub fn load_state_dict(
        &mut self,
        state_dict: &ModuleStateDict,
        mode: ModuleStateLoadMode,
    ) -> Result<ModuleStateLoadReport, ModuleStateLoadError> {
        let target_before = self.state_dict_with_view(state_dict.view);
        let missing_keys = target_before
            .entries
            .keys()
            .filter(|path| !state_dict.entries.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        let unexpected_keys = state_dict
            .entries
            .keys()
            .filter(|path| !target_before.entries.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        if mode == ModuleStateLoadMode::Strict
            && (!missing_keys.is_empty() || !unexpected_keys.is_empty())
        {
            return Err(ModuleStateLoadError::StrictKeyMismatch {
                missing_keys,
                unexpected_keys,
            });
        }

        let mut updates = Vec::new();
        for (path, source_entry) in &state_dict.entries {
            let Some(target_entry) = target_before.entries.get(path) else {
                continue;
            };
            if target_entry.kind != source_entry.kind {
                return Err(ModuleStateLoadError::EntryKindMismatch {
                    path: path.clone(),
                    expected: target_entry.kind,
                    actual: source_entry.kind,
                });
            }
            if target_entry.spec.shape().dims() != source_entry.spec.shape().dims() {
                return Err(ModuleStateLoadError::ShapeMismatch {
                    path: path.clone(),
                    expected: target_entry.spec.shape().dims().to_vec(),
                    actual: source_entry.spec.shape().dims().to_vec(),
                });
            }
            if target_entry.spec.dtype() != source_entry.spec.dtype() {
                return Err(ModuleStateLoadError::DTypeMismatch {
                    path: path.clone(),
                    expected: target_entry.spec.dtype(),
                    actual: source_entry.spec.dtype(),
                });
            }
            validate_tensor_payload(path.as_str(), &target_entry.spec, &source_entry.data)
                .map_err(|source| ModuleStateLoadError::InvalidSourcePayload {
                    path: path.clone(),
                    source,
                })?;
            updates.push((path.clone(), target_entry.kind, source_entry.data.clone()));
        }

        for (path, kind, data) in &updates {
            match kind {
                ModuleStateEntryKind::Parameter => {
                    self.parameter_mut(path.as_str())
                        .map_err(|source| ModuleStateLoadError::MissingTargetPath {
                            path: path.clone(),
                            source,
                        })?
                        .data = data.clone();
                }
                ModuleStateEntryKind::Buffer => {
                    self.buffer_mut(path.as_str())
                        .map_err(|source| ModuleStateLoadError::MissingTargetPath {
                            path: path.clone(),
                            source,
                        })?
                        .data = data.clone();
                }
            }
        }

        let target_after = self.state_dict_with_view(state_dict.view);
        Ok(ModuleStateLoadReport {
            mode,
            view: state_dict.view,
            source_state_dict_digest: state_dict.state_dict_digest.clone(),
            target_before_digest: target_before.state_dict_digest,
            target_after_digest: target_after.state_dict_digest,
            loaded_paths: updates.into_iter().map(|(path, _, _)| path).collect(),
            missing_keys,
            unexpected_keys,
        })
    }

    fn ensure_local_name_available(&self, name: &str) -> Result<(), ModuleStateError> {
        if self.parameters.contains_key(name)
            || self.buffers.contains_key(name)
            || self.submodules.contains_key(name)
        {
            return Err(ModuleStateError::DuplicateLocalName {
                name: String::from(name),
            });
        }
        Ok(())
    }

    fn collect_named_parameters<'a>(
        &'a self,
        prefix: &str,
        view: ModuleParameterView,
        entries: &mut Vec<(String, &'a ModuleParameter)>,
    ) {
        for (name, parameter) in &self.parameters {
            let include = match view {
                ModuleParameterView::All => true,
                ModuleParameterView::TrainableOnly => parameter.requires_grad,
                ModuleParameterView::FrozenOnly => parameter.is_frozen(),
            };
            if !include {
                continue;
            }
            entries.push((join_path(prefix, name.as_str()), parameter));
        }
        for (name, submodule) in &self.submodules {
            submodule.collect_named_parameters(
                join_path(prefix, name.as_str()).as_str(),
                view,
                entries,
            );
        }
    }

    fn collect_named_buffers<'a>(
        &'a self,
        prefix: &str,
        view: ModuleStateView,
        entries: &mut Vec<(String, &'a ModuleBuffer)>,
    ) {
        for (name, buffer) in &self.buffers {
            if view == ModuleStateView::AllBuffers || buffer.persistent {
                entries.push((join_path(prefix, name.as_str()), buffer));
            }
        }
        for (name, submodule) in &self.submodules {
            submodule.collect_named_buffers(
                join_path(prefix, name.as_str()).as_str(),
                view,
                entries,
            );
        }
    }

    fn collect_named_modules<'a>(&'a self, prefix: &str, entries: &mut Vec<(String, &'a Module)>) {
        entries.push((String::from(prefix), self));
        for (name, submodule) in &self.submodules {
            submodule.collect_named_modules(join_path(prefix, name.as_str()).as_str(), entries);
        }
    }

    fn set_requires_grad_recursive(&mut self, requires_grad: bool) -> usize {
        let mut changed = 0_usize;
        for parameter in self.parameters.values_mut() {
            if parameter.requires_grad != requires_grad {
                parameter.requires_grad = requires_grad;
                changed += 1;
            }
        }
        for submodule in self.submodules.values_mut() {
            changed += submodule.set_requires_grad_recursive(requires_grad);
        }
        changed
    }

    fn collect_state_entries(
        &self,
        prefix: &str,
        view: ModuleStateView,
        entries: &mut Vec<ModuleStateEntry>,
    ) {
        for (name, parameter) in &self.parameters {
            entries.push(ModuleStateEntry {
                path: join_path(prefix, name.as_str()),
                kind: ModuleStateEntryKind::Parameter,
                spec: parameter.spec.clone(),
                data: parameter.data.clone(),
                requires_grad: parameter.requires_grad,
                persistent: true,
            });
        }
        for (name, buffer) in &self.buffers {
            if view == ModuleStateView::AllBuffers || buffer.persistent {
                entries.push(ModuleStateEntry {
                    path: join_path(prefix, name.as_str()),
                    kind: ModuleStateEntryKind::Buffer,
                    spec: buffer.spec.clone(),
                    data: buffer.data.clone(),
                    requires_grad: false,
                    persistent: buffer.persistent,
                });
            }
        }
        for (name, submodule) in &self.submodules {
            submodule.collect_state_entries(
                join_path(prefix, name.as_str()).as_str(),
                view,
                entries,
            );
        }
    }

    fn parameter_mut(&mut self, path: &str) -> Result<&mut ModuleParameter, ModuleStateError> {
        let (module_path, local_name) = split_state_path(path)?;
        let module = self.submodule_mut(module_path.as_str())?;
        module
            .parameters
            .get_mut(local_name.as_str())
            .ok_or_else(|| ModuleStateError::MissingParameter {
                path: String::from(path),
            })
    }

    fn buffer_mut(&mut self, path: &str) -> Result<&mut ModuleBuffer, ModuleStateError> {
        let (module_path, local_name) = split_state_path(path)?;
        let module = self.submodule_mut(module_path.as_str())?;
        module
            .buffers
            .get_mut(local_name.as_str())
            .ok_or_else(|| ModuleStateError::MissingBuffer {
                path: String::from(path),
            })
    }
}

#[derive(Clone, Debug)]
struct ModuleParityExpectations {
    module_paths: Vec<String>,
    parameter_paths: Vec<String>,
    buffer_paths: Vec<String>,
    state_dict_keys: Vec<String>,
}

/// Returns the seeded module parity matrix report for the current built-in
/// Psionic module tree surface.
pub fn builtin_module_parity_matrix_report() -> Result<ModuleParityMatrixReport, ModuleStateError> {
    let mut cases = Vec::new();

    cases.push(run_module_parity_supported_case(
        "pytorch.linear.normalized_state_dict",
        "linear",
        "normalized_state_dict_set",
        ModuleStateView::PersistentOnly,
        build_linear_module,
        ModuleParityExpectations {
            module_paths: normalized_paths([""]),
            parameter_paths: normalized_paths(["bias", "weight"]),
            buffer_paths: Vec::new(),
            state_dict_keys: normalized_paths(["bias", "weight"]),
        },
    )?);
    cases.push(run_module_parity_supported_case(
        "pytorch.batch_norm1d.persistent_buffers",
        "batch_norm1d",
        "persistent_buffer_contract",
        ModuleStateView::PersistentOnly,
        build_batch_norm1d_module,
        ModuleParityExpectations {
            module_paths: normalized_paths([""]),
            parameter_paths: normalized_paths(["bias", "weight"]),
            buffer_paths: normalized_paths(["running_mean", "running_var"]),
            state_dict_keys: normalized_paths(["bias", "running_mean", "running_var", "weight"]),
        },
    )?);
    cases.push(run_module_parity_supported_case(
        "pytorch.transformer_encoder_layer.normalized_nested_paths",
        "transformer_encoder_layer",
        "normalized_nested_all_buffers",
        ModuleStateView::AllBuffers,
        build_transformer_encoder_layer_module,
        ModuleParityExpectations {
            module_paths: normalized_paths(["", "norm1", "self_attn", "self_attn.out_proj"]),
            parameter_paths: normalized_paths([
                "norm1.bias",
                "norm1.weight",
                "self_attn.in_proj_weight",
                "self_attn.out_proj.bias",
                "self_attn.out_proj.weight",
            ]),
            buffer_paths: normalized_paths(["self_attn.cached_mask"]),
            state_dict_keys: normalized_paths([
                "norm1.bias",
                "norm1.weight",
                "self_attn.cached_mask",
                "self_attn.in_proj_weight",
                "self_attn.out_proj.bias",
                "self_attn.out_proj.weight",
            ]),
        },
    )?);
    cases.push(run_module_parity_refusal_case(
        "pytorch.linear.registration_order_preservation",
        "linear",
        "registration_order_preserving_state_dict",
        ModuleStateView::PersistentOnly,
        build_linear_module,
        ModuleParityExpectations {
            module_paths: normalized_paths([""]),
            parameter_paths: normalized_paths(["bias", "weight"]),
            buffer_paths: Vec::new(),
            state_dict_keys: vec![String::from("weight"), String::from("bias")],
        },
        PsionicRefusal::new(
            PsionicRefusalCode::SerializationIncompatibility,
            PsionicRefusalScope::Runtime,
            "module parity case requires PyTorch registration-order-preserving state_dict keys, but psionic-nn currently emits deterministic lexical key order",
        )
        .with_subject("state_dict_registration_order"),
    )?);

    Ok(ModuleParityMatrixReport::new(
        "pytorch_module_db_seed_v0",
        cases,
    ))
}

fn validate_local_name(name: &str) -> Result<(), ModuleStateError> {
    if name.trim().is_empty() || name.contains('.') {
        return Err(ModuleStateError::InvalidLocalName {
            name: String::from(name),
        });
    }
    Ok(())
}

fn run_module_parity_supported_case(
    case_id: &str,
    module_kind: &str,
    capability_profile: &str,
    view: ModuleStateView,
    build: impl FnOnce() -> Result<Module, ModuleStateError>,
    expected: ModuleParityExpectations,
) -> Result<ModuleParityCaseResult, ModuleStateError> {
    let module = build()?;
    Ok(ModuleParityCaseResult {
        case_id: String::from(case_id),
        oracle_family: String::from("pytorch_module_db_seed"),
        module_kind: String::from(module_kind),
        capability_profile: String::from(capability_profile),
        view,
        expected_module_paths: expected.module_paths,
        actual_module_paths: module
            .named_modules()
            .into_iter()
            .map(|(path, _)| path)
            .collect(),
        expected_parameter_paths: expected.parameter_paths,
        actual_parameter_paths: module
            .named_parameters()
            .into_iter()
            .map(|(path, _)| path)
            .collect(),
        expected_buffer_paths: expected.buffer_paths,
        actual_buffer_paths: module
            .named_buffers(view)
            .into_iter()
            .map(|(path, _)| path)
            .collect(),
        expected_state_dict_keys: expected.state_dict_keys,
        actual_state_dict_keys: module.state_dict_with_view(view).keys(),
        expected_refusal: None,
        actual_refusal: None,
        status: ModuleParityStatus::Supported,
    })
}

fn run_module_parity_refusal_case(
    case_id: &str,
    module_kind: &str,
    capability_profile: &str,
    view: ModuleStateView,
    build: impl FnOnce() -> Result<Module, ModuleStateError>,
    expected: ModuleParityExpectations,
    expected_refusal: PsionicRefusal,
) -> Result<ModuleParityCaseResult, ModuleStateError> {
    let module = build()?;
    let actual_module_paths = module
        .named_modules()
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    let actual_parameter_paths = module
        .named_parameters()
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    let actual_buffer_paths = module
        .named_buffers(view)
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    let actual_state_dict_keys = module.state_dict_with_view(view).keys();
    let actual_refusal = if actual_state_dict_keys == expected.state_dict_keys {
        None
    } else {
        Some(
            PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Runtime,
                "module parity case requires PyTorch registration-order-preserving state_dict keys, but psionic-nn currently emits deterministic lexical key order",
            )
            .with_subject("state_dict_registration_order"),
        )
    };
    let status = if actual_refusal.is_some() {
        ModuleParityStatus::Refused
    } else {
        ModuleParityStatus::Supported
    };
    Ok(ModuleParityCaseResult {
        case_id: String::from(case_id),
        oracle_family: String::from("pytorch_module_db_seed"),
        module_kind: String::from(module_kind),
        capability_profile: String::from(capability_profile),
        view,
        expected_module_paths: expected.module_paths,
        actual_module_paths,
        expected_parameter_paths: expected.parameter_paths,
        actual_parameter_paths,
        expected_buffer_paths: expected.buffer_paths,
        actual_buffer_paths,
        expected_state_dict_keys: expected.state_dict_keys,
        actual_state_dict_keys,
        expected_refusal: Some(expected_refusal),
        actual_refusal,
        status,
    })
}

fn build_linear_module() -> Result<Module, ModuleStateError> {
    let mut module = Module::new("linear0", "linear")?;
    module.insert_parameter(
        "weight",
        f32_parameter_entry(&[2, 2], &[1.0, 2.0, 3.0, 4.0])?,
    )?;
    module.insert_parameter("bias", f32_parameter_entry(&[2], &[0.1, 0.2])?)?;
    Ok(module)
}

fn build_batch_norm1d_module() -> Result<Module, ModuleStateError> {
    let mut module = Module::new("bn0", "batch_norm1d")?;
    module.insert_parameter("weight", f32_parameter_entry(&[2], &[1.0, 1.0])?)?;
    module.insert_parameter("bias", f32_parameter_entry(&[2], &[0.0, 0.0])?)?;
    module.insert_buffer("running_mean", f32_buffer_entry(&[2], &[0.4, -0.1], true)?)?;
    module.insert_buffer("running_var", f32_buffer_entry(&[2], &[1.2, 0.8], true)?)?;
    Ok(module)
}

fn build_transformer_encoder_layer_module() -> Result<Module, ModuleStateError> {
    let mut root = Module::new("layer0", "transformer_encoder_layer")?;

    let mut self_attn = Module::new("attn0", "multihead_attention")?;
    self_attn.insert_parameter(
        "in_proj_weight",
        f32_parameter_entry(
            &[6, 2],
            &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2],
        )?,
    )?;
    self_attn.insert_buffer("cached_mask", f32_buffer_entry(&[2], &[0.0, 1.0], false)?)?;

    let mut out_proj = Module::new("out_proj0", "linear")?;
    out_proj.insert_parameter(
        "weight",
        f32_parameter_entry(&[2, 2], &[0.9, 0.1, 0.3, 0.7])?,
    )?;
    out_proj.insert_parameter("bias", f32_parameter_entry(&[2], &[0.0, 0.1])?)?;
    self_attn.insert_submodule("out_proj", out_proj)?;

    let mut norm1 = Module::new("norm10", "layer_norm")?;
    norm1.insert_parameter("weight", f32_parameter_entry(&[2], &[1.0, 1.0])?)?;
    norm1.insert_parameter("bias", f32_parameter_entry(&[2], &[0.0, 0.0])?)?;

    root.insert_submodule("self_attn", self_attn)?;
    root.insert_submodule("norm1", norm1)?;
    Ok(root)
}

fn f32_parameter_entry(
    shape: &[usize],
    values: &[f32],
) -> Result<ModuleParameter, ModuleStateError> {
    ModuleParameter::new(
        TensorSpec::new(
            psionic_core::Shape::new(shape.to_vec()),
            DType::F32,
            psionic_core::Device::cpu(),
        ),
        TensorData::F32(values.to_vec()),
        true,
    )
}

fn f32_buffer_entry(
    shape: &[usize],
    values: &[f32],
    persistent: bool,
) -> Result<ModuleBuffer, ModuleStateError> {
    ModuleBuffer::new(
        TensorSpec::new(
            psionic_core::Shape::new(shape.to_vec()),
            DType::F32,
            psionic_core::Device::cpu(),
        ),
        TensorData::F32(values.to_vec()),
        persistent,
    )
}

fn normalized_paths<const N: usize>(paths: [&str; N]) -> Vec<String> {
    let mut normalized = paths.into_iter().map(String::from).collect::<Vec<_>>();
    normalized.sort();
    normalized
}

fn split_state_path(path: &str) -> Result<(String, String), ModuleStateError> {
    if path.trim().is_empty() {
        return Err(ModuleStateError::InvalidLocalName {
            name: String::from(path),
        });
    }
    let mut segments = path.rsplitn(2, '.');
    let local_name = segments.next().unwrap_or_default();
    validate_local_name(local_name)?;
    let module_path = segments.next().unwrap_or_default();
    if !module_path.is_empty() {
        for segment in module_path.split('.') {
            validate_local_name(segment)?;
        }
    }
    Ok((String::from(module_path), String::from(local_name)))
}

fn join_path(prefix: &str, local_name: &str) -> String {
    if prefix.is_empty() {
        String::from(local_name)
    } else {
        format!("{prefix}.{local_name}")
    }
}

fn validate_tensor_payload(
    owner: &str,
    spec: &TensorSpec,
    data: &TensorData,
) -> Result<(), ModuleStateError> {
    match data {
        TensorData::F32(values) => {
            let expected_len = spec.storage_size();
            let actual_len = values.len();
            if actual_len != expected_len {
                return Err(ModuleStateError::DensePayloadLengthMismatch {
                    owner: String::from(owner),
                    expected_len,
                    actual_len,
                });
            }
        }
        TensorData::QuantizedBlocks(quantized) => {
            if !spec.dtype().supports_quantized_logical_storage() {
                return Err(ModuleStateError::QuantizedPayloadUnsupportedDType {
                    owner: String::from(owner),
                    dtype: spec.dtype(),
                });
            }
            let expected_elements = spec.shape().element_count();
            let actual_elements = quantized.layout.element_count();
            if actual_elements != expected_elements {
                return Err(ModuleStateError::QuantizedPayloadElementCountMismatch {
                    owner: String::from(owner),
                    expected_elements,
                    actual_elements,
                });
            }
        }
    }
    Ok(())
}

fn stable_module_parity_matrix_digest(
    oracle_family_window: &str,
    cases: &[ModuleParityCaseResult],
) -> String {
    let mut lines = vec![format!("oracle_family_window={oracle_family_window}")];
    for case in cases {
        lines.push(format!(
            "{}|{}|{}|{:?}|{:?}",
            case.case_id, case.module_kind, case.capability_profile, case.view, case.status
        ));
        for path in &case.expected_module_paths {
            lines.push(format!("expected_module={path}"));
        }
        for path in &case.actual_module_paths {
            lines.push(format!("actual_module={path}"));
        }
        for path in &case.expected_parameter_paths {
            lines.push(format!("expected_parameter={path}"));
        }
        for path in &case.actual_parameter_paths {
            lines.push(format!("actual_parameter={path}"));
        }
        for path in &case.expected_buffer_paths {
            lines.push(format!("expected_buffer={path}"));
        }
        for path in &case.actual_buffer_paths {
            lines.push(format!("actual_buffer={path}"));
        }
        for key in &case.expected_state_dict_keys {
            lines.push(format!("expected_state_dict_key={key}"));
        }
        for key in &case.actual_state_dict_keys {
            lines.push(format!("actual_state_dict_key={key}"));
        }
        if let Some(refusal) = &case.expected_refusal {
            lines.push(format!(
                "expected_refusal={:?}|{:?}|{}|{}",
                refusal.code,
                refusal.scope,
                refusal.subject.as_deref().unwrap_or_default(),
                refusal.detail
            ));
        }
        if let Some(refusal) = &case.actual_refusal {
            lines.push(format!(
                "actual_refusal={:?}|{:?}|{}|{}",
                refusal.code,
                refusal.scope,
                refusal.subject.as_deref().unwrap_or_default(),
                refusal.detail
            ));
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn stable_module_state_tree_digest(
    root_module_id: &str,
    root_module_kind: &str,
    view: ModuleStateView,
    entries: &[ModuleStateEntry],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"module_state_tree|");
    hasher.update(root_module_id.as_bytes());
    hasher.update(b"|");
    hasher.update(root_module_kind.as_bytes());
    hasher.update(b"|view|");
    hasher.update(match view {
        ModuleStateView::PersistentOnly => b"persistent_only".as_slice(),
        ModuleStateView::AllBuffers => b"all_buffers".as_slice(),
    });
    for entry in entries {
        hasher.update(b"|entry|");
        hasher.update(entry.path.as_bytes());
        hasher.update(b"|");
        hasher.update(match entry.kind {
            ModuleStateEntryKind::Parameter => b"parameter".as_slice(),
            ModuleStateEntryKind::Buffer => b"buffer".as_slice(),
        });
        hasher.update(b"|");
        hasher.update(if entry.requires_grad {
            b"requires_grad".as_slice()
        } else {
            b"no_grad".as_slice()
        });
        hasher.update(b"|");
        hasher.update(if entry.persistent {
            b"persistent".as_slice()
        } else {
            b"ephemeral".as_slice()
        });
        hasher.update(b"|");
        hasher.update(stable_tensor_spec_digest(&entry.spec).as_bytes());
        hasher.update(b"|");
        hasher.update(stable_tensor_data_digest(&entry.data).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_tensor_spec_digest(spec: &TensorSpec) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tensor_spec|");
    for dim in spec.shape().dims() {
        hasher.update(b"dim|");
        hasher.update(dim.to_string().as_bytes());
    }
    hasher.update(b"|dtype|");
    hasher.update(match spec.dtype() {
        DType::F32 => b"f32".as_slice(),
        DType::F16 => b"f16".as_slice(),
        DType::BF16 => b"bf16".as_slice(),
        DType::I8 => b"i8".as_slice(),
    });
    hasher.update(b"|device_kind|");
    hasher.update(spec.device().kind().to_string().as_bytes());
    hasher.update(b"|device_ordinal|");
    hasher.update(spec.device().ordinal().to_string().as_bytes());
    if let Some(label) = spec.device().label() {
        hasher.update(b"|device_label|");
        hasher.update(label.as_bytes());
    }
    hasher.update(b"|storage_size|");
    hasher.update(spec.storage_size().to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_tensor_data_digest(data: &TensorData) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tensor_data|");
    match data {
        TensorData::F32(values) => {
            hasher.update(b"f32");
            for value in values {
                hasher.update(value.to_bits().to_le_bytes());
            }
        }
        TensorData::QuantizedBlocks(quantized) => {
            hasher.update(b"quantized_blocks|");
            hasher.update(quantization_mode_label(quantized.mode).as_bytes());
            hasher.update(b"|");
            hasher.update(quantized.layout.elements_per_block.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(quantized.layout.bytes_per_block.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(quantized.layout.block_count.to_string().as_bytes());
            hasher.update(b"|bytes|");
            hasher.update(quantized.bytes.as_slice());
        }
    }
    hex::encode(hasher.finalize())
}

const fn quantization_mode_label(mode: QuantizationMode) -> &'static str {
    match mode {
        QuantizationMode::None => "none",
        QuantizationMode::Int8Symmetric => "int8_symmetric",
        QuantizationMode::GgmlMxfp4 => "ggml_mxfp4",
        QuantizationMode::GgmlQ4_0 => "ggml_q4_0",
        QuantizationMode::GgmlQ4_1 => "ggml_q4_1",
        QuantizationMode::GgmlQ8_0 => "ggml_q8_0",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use std::collections::BTreeMap;

    use psionic_core::{Device, Shape, TensorData, TensorSpec};

    use super::{
        DType, Module, ModuleBuffer, ModuleParameter, ModuleParameterView, ModuleParityStatus,
        ModuleStateDict, ModuleStateEntry, ModuleStateEntryKind, ModuleStateError,
        ModuleStateLoadError, ModuleStateLoadMode, ModuleStateView,
        builtin_module_parity_matrix_report,
    };

    fn f32_parameter(shape: &[usize], values: &[f32]) -> Result<ModuleParameter, ModuleStateError> {
        f32_parameter_with_grad(shape, values, true)
    }

    fn f32_parameter_with_grad(
        shape: &[usize],
        values: &[f32],
        requires_grad: bool,
    ) -> Result<ModuleParameter, ModuleStateError> {
        ModuleParameter::new(
            TensorSpec::new(Shape::new(shape.to_vec()), DType::F32, Device::cpu()),
            TensorData::F32(values.to_vec()),
            requires_grad,
        )
    }

    fn f32_buffer(
        shape: &[usize],
        values: &[f32],
        persistent: bool,
    ) -> Result<ModuleBuffer, ModuleStateError> {
        ModuleBuffer::new(
            TensorSpec::new(Shape::new(shape.to_vec()), DType::F32, Device::cpu()),
            TensorData::F32(values.to_vec()),
            persistent,
        )
    }

    #[test]
    fn module_tree_traversal_surfaces_named_parameters_buffers_and_modules()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut root = Module::new("toy-transformer", "transformer")?;
        root.insert_parameter("embedding", f32_parameter(&[2, 2], &[0.1, 0.2, 0.3, 0.4])?)?;
        root.insert_buffer("running_scale", f32_buffer(&[2], &[1.0, 1.0], true)?)?;

        let mut block = Module::new("block0", "transformer_block")?;
        block.insert_parameter("weight", f32_parameter(&[2, 2], &[1.0, 2.0, 3.0, 4.0])?)?;
        block.insert_buffer("dropout_mask", f32_buffer(&[2], &[0.0, 1.0], false)?)?;
        root.insert_submodule("encoder", block)?;

        let parameter_paths = root
            .named_parameters()
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(
            parameter_paths,
            vec![String::from("embedding"), String::from("encoder.weight")]
        );

        let persistent_buffers = root
            .named_buffers(ModuleStateView::PersistentOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(persistent_buffers, vec![String::from("running_scale")]);

        let all_buffers = root
            .named_buffers(ModuleStateView::AllBuffers)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(
            all_buffers,
            vec![
                String::from("running_scale"),
                String::from("encoder.dropout_mask")
            ]
        );

        let module_paths = root
            .named_modules()
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(module_paths, vec![String::new(), String::from("encoder")]);
        assert_eq!(
            root.parameter("encoder.weight")?.spec.shape().dims(),
            &[2, 2]
        );
        assert!(root.buffer("encoder.dropout_mask").is_ok());
        Ok(())
    }

    #[test]
    fn module_freeze_semantics_and_filtered_parameter_discovery_stay_recursive()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut root = Module::new("toy-transformer", "transformer")?;
        root.insert_parameter(
            "embedding",
            f32_parameter_with_grad(&[2, 2], &[0.1, 0.2, 0.3, 0.4], true)?,
        )?;

        let mut block = Module::new("block0", "transformer_block")?;
        block.insert_parameter(
            "weight",
            f32_parameter_with_grad(&[2, 2], &[1.0, 2.0, 3.0, 4.0], true)?,
        )?;
        block.insert_parameter(
            "norm_bias",
            f32_parameter_with_grad(&[2], &[0.0, 0.0], false)?,
        )?;
        root.insert_submodule("encoder", block)?;

        let trainable_before = root
            .named_parameters_with_view(ModuleParameterView::TrainableOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(
            trainable_before,
            vec![String::from("embedding"), String::from("encoder.weight")]
        );
        let frozen_before = root
            .named_parameters_with_view(ModuleParameterView::FrozenOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(frozen_before, vec![String::from("encoder.norm_bias")]);

        assert_eq!(root.freeze_submodule("encoder")?, 1);
        let trainable_after_submodule = root
            .named_parameters_with_view(ModuleParameterView::TrainableOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(trainable_after_submodule, vec![String::from("embedding")]);
        let frozen_after_submodule = root
            .named_parameters_with_view(ModuleParameterView::FrozenOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(
            frozen_after_submodule,
            vec![
                String::from("encoder.norm_bias"),
                String::from("encoder.weight")
            ]
        );

        assert!(root.set_parameter_requires_grad("embedding", false)?);
        assert!(root.parameter("embedding")?.is_frozen());
        assert_eq!(root.freeze(), 0);
        assert_eq!(root.unfreeze_submodule("encoder")?, 2);
        assert_eq!(root.unfreeze(), 1);

        let trainable_after = root
            .named_parameters_with_view(ModuleParameterView::TrainableOnly)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert_eq!(
            trainable_after,
            vec![
                String::from("embedding"),
                String::from("encoder.norm_bias"),
                String::from("encoder.weight")
            ]
        );

        Ok(())
    }

    #[test]
    fn module_state_tree_persistent_view_omits_nonpersistent_buffers_and_stays_stable()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut root = Module::new("toy-module", "normed_linear")?;
        root.insert_parameter("weight", f32_parameter(&[2, 2], &[1.0, 2.0, 3.0, 4.0])?)?;
        root.insert_buffer("running_mean", f32_buffer(&[2], &[0.0, 0.1], true)?)?;
        root.insert_buffer("scratch", f32_buffer(&[2], &[9.0, 9.0], false)?)?;

        let persistent = root.state_tree(ModuleStateView::PersistentOnly);
        let all = root.state_tree(ModuleStateView::AllBuffers);

        assert_eq!(persistent.entries.len(), 2);
        assert_eq!(all.entries.len(), 3);
        assert_ne!(persistent.state_tree_digest, all.state_tree_digest);
        assert_eq!(persistent.entries[0].kind, ModuleStateEntryKind::Parameter);
        assert_eq!(persistent.entries[1].kind, ModuleStateEntryKind::Buffer);
        assert!(persistent.entries.iter().all(|entry| entry.persistent));
        assert!(
            persistent
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("state_tree_digest="))
        );
        assert_eq!(root.stable_digest(), all.state_tree_digest);
        Ok(())
    }

    #[test]
    fn module_tree_refuses_shadowing_invalid_names_and_missing_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut root = Module::new("toy", "linear")?;
        root.insert_parameter("weight", f32_parameter(&[1], &[1.0])?)?;

        let duplicate = root
            .insert_buffer("weight", f32_buffer(&[1], &[0.0], true)?)
            .expect_err("shadowed local name should refuse");
        assert_eq!(
            duplicate,
            ModuleStateError::DuplicateLocalName {
                name: String::from("weight"),
            }
        );

        let invalid = root
            .insert_parameter("bad.name", f32_parameter(&[1], &[2.0])?)
            .expect_err("path-like local name should refuse");
        assert_eq!(
            invalid,
            ModuleStateError::InvalidLocalName {
                name: String::from("bad.name"),
            }
        );

        let mut child = Module::new("child", "leaf")?;
        child.insert_parameter("bias", f32_parameter(&[1], &[3.0])?)?;
        root.insert_submodule("child", child)?;

        let missing_parameter = root
            .parameter("child.weight")
            .expect_err("missing parameter path should refuse");
        assert_eq!(
            missing_parameter,
            ModuleStateError::MissingParameter {
                path: String::from("child.weight"),
            }
        );

        let missing_submodule = root
            .submodule("child.inner")
            .expect_err("missing submodule path should refuse");
        assert_eq!(
            missing_submodule,
            ModuleStateError::UnknownSubmodulePath {
                path: String::from("child.inner"),
            }
        );
        Ok(())
    }

    #[test]
    fn module_state_dict_is_deterministic_and_persistent_only_by_default()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut root = Module::new("toy", "encoder")?;
        root.insert_parameter("weight", f32_parameter(&[2], &[1.0, 2.0])?)?;
        root.insert_buffer("running_mean", f32_buffer(&[2], &[0.0, 0.1], true)?)?;
        root.insert_buffer("scratch", f32_buffer(&[2], &[9.0, 9.0], false)?)?;

        let state_dict = root.state_dict();
        assert_eq!(
            state_dict.keys(),
            vec![String::from("running_mean"), String::from("weight")]
        );
        assert_eq!(state_dict.view, ModuleStateView::PersistentOnly);
        assert!(
            state_dict
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("state_dict_digest="))
        );
        assert!(state_dict.entry("scratch").is_none());

        let all_buffers = root.state_dict_with_view(ModuleStateView::AllBuffers);
        assert!(all_buffers.entry("scratch").is_some());
        assert_ne!(state_dict.state_dict_digest, all_buffers.state_dict_digest);
        Ok(())
    }

    #[test]
    fn save_weights_defaults_to_persistent_view_and_load_weights_defaults_to_strict()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut target = Module::new("target", "linear")?;
        target.insert_parameter("weight", f32_parameter(&[1], &[1.0])?)?;
        target.insert_buffer("running_mean", f32_buffer(&[1], &[0.0], true)?)?;

        let mut source = Module::new("source", "linear")?;
        source.insert_parameter("weight", f32_parameter(&[1], &[5.0])?)?;
        source.insert_buffer("scratch", f32_buffer(&[1], &[9.0], false)?)?;

        let saved = source.save_weights();
        assert_eq!(saved.view, ModuleStateView::PersistentOnly);
        assert_eq!(saved.keys(), vec![String::from("weight")]);

        let refusal = target
            .load_weights(&saved)
            .expect_err("missing persistent buffer should refuse strict load");
        assert_eq!(
            refusal,
            ModuleStateLoadError::StrictKeyMismatch {
                missing_keys: vec![String::from("running_mean")],
                unexpected_keys: Vec::new(),
            }
        );
        assert_eq!(target.parameter("weight")?.data, TensorData::F32(vec![1.0]));
        Ok(())
    }

    #[test]
    fn load_weights_with_mode_surfaces_non_strict_behavior_explicitly()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut target = Module::new("target", "linear")?;
        target.insert_parameter("weight", f32_parameter(&[1], &[1.0])?)?;
        target.insert_buffer("running_mean", f32_buffer(&[1], &[0.0], true)?)?;

        let mut source = Module::new("source", "linear")?;
        source.insert_parameter("weight", f32_parameter(&[1], &[7.0])?)?;
        source.insert_parameter("bias", f32_parameter(&[1], &[9.0])?)?;

        let report = target
            .load_weights_with_mode(&source.save_weights(), ModuleStateLoadMode::NonStrict)?;
        assert_eq!(report.mode, ModuleStateLoadMode::NonStrict);
        assert_eq!(report.loaded_paths, vec![String::from("weight")]);
        assert_eq!(report.missing_keys, vec![String::from("running_mean")]);
        assert_eq!(report.unexpected_keys, vec![String::from("bias")]);
        assert_eq!(target.parameter("weight")?.data, TensorData::F32(vec![7.0]));
        assert_eq!(
            target.buffer("running_mean")?.data,
            TensorData::F32(vec![0.0])
        );
        Ok(())
    }

    #[test]
    fn strict_module_state_load_requires_exact_key_match() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut target = Module::new("target", "linear")?;
        target.insert_parameter("weight", f32_parameter(&[1], &[1.0])?)?;
        target.insert_buffer("running_mean", f32_buffer(&[1], &[0.0], true)?)?;

        let mut source = Module::new("source", "linear")?;
        source.insert_parameter("weight", f32_parameter(&[1], &[5.0])?)?;

        let refusal = target
            .load_state_dict(&source.state_dict(), ModuleStateLoadMode::Strict)
            .expect_err("missing persistent buffer should refuse strict load");
        assert_eq!(
            refusal,
            ModuleStateLoadError::StrictKeyMismatch {
                missing_keys: vec![String::from("running_mean")],
                unexpected_keys: Vec::new(),
            }
        );
        assert_eq!(target.parameter("weight")?.data, TensorData::F32(vec![1.0]));
        Ok(())
    }

    #[test]
    fn non_strict_module_state_load_reports_missing_and_unexpected_keys()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut target = Module::new("target", "linear")?;
        target.insert_parameter("weight", f32_parameter(&[1], &[1.0])?)?;
        target.insert_buffer("running_mean", f32_buffer(&[1], &[0.0], true)?)?;

        let mut source = Module::new("source", "linear")?;
        source.insert_parameter("weight", f32_parameter(&[1], &[7.0])?)?;
        source.insert_parameter("bias", f32_parameter(&[1], &[9.0])?)?;

        let report =
            target.load_state_dict(&source.state_dict(), ModuleStateLoadMode::NonStrict)?;
        assert_eq!(report.loaded_paths, vec![String::from("weight")]);
        assert_eq!(report.missing_keys, vec![String::from("running_mean")]);
        assert_eq!(report.unexpected_keys, vec![String::from("bias")]);
        assert_eq!(target.parameter("weight")?.data, TensorData::F32(vec![7.0]));
        assert_eq!(
            target.buffer("running_mean")?.data,
            TensorData::F32(vec![0.0])
        );
        Ok(())
    }

    #[test]
    fn module_state_load_refuses_shape_and_kind_mismatches_even_non_strict()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut target = Module::new("target", "block")?;
        target.insert_parameter("weight", f32_parameter(&[2], &[1.0, 2.0])?)?;

        let mut wrong_shape = Module::new("source", "block")?;
        wrong_shape.insert_parameter("weight", f32_parameter(&[3], &[3.0, 4.0, 5.0])?)?;
        let shape_refusal = target
            .load_state_dict(&wrong_shape.state_dict(), ModuleStateLoadMode::NonStrict)
            .expect_err("shape mismatch should refuse");
        assert_eq!(
            shape_refusal,
            ModuleStateLoadError::ShapeMismatch {
                path: String::from("weight"),
                expected: vec![2],
                actual: vec![3],
            }
        );

        let mut wrong_kind_entries = BTreeMap::new();
        wrong_kind_entries.insert(
            String::from("weight"),
            ModuleStateEntry {
                path: String::from("weight"),
                kind: ModuleStateEntryKind::Buffer,
                spec: TensorSpec::new(Shape::new(vec![2]), DType::F32, Device::cpu()),
                data: TensorData::F32(vec![8.0, 9.0]),
                requires_grad: false,
                persistent: true,
            },
        );
        let wrong_kind = ModuleStateDict::new(
            "source",
            "block",
            ModuleStateView::PersistentOnly,
            wrong_kind_entries,
        )?;
        let kind_refusal = target
            .load_state_dict(&wrong_kind, ModuleStateLoadMode::NonStrict)
            .expect_err("kind mismatch should refuse");
        assert_eq!(
            kind_refusal,
            ModuleStateLoadError::EntryKindMismatch {
                path: String::from("weight"),
                expected: ModuleStateEntryKind::Parameter,
                actual: ModuleStateEntryKind::Buffer,
            }
        );

        assert_eq!(
            target.parameter("weight")?.data,
            TensorData::F32(vec![1.0, 2.0])
        );
        Ok(())
    }

    #[test]
    fn module_parity_matrix_report_tracks_seeded_supported_and_refusal_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_module_parity_matrix_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.oracle_family_window, "pytorch_module_db_seed_v0");
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("matrix_digest="))
        );

        let linear_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "pytorch.linear.normalized_state_dict")
            .expect("missing linear parity case");
        assert_eq!(linear_case.status, ModuleParityStatus::Supported);
        assert_eq!(
            linear_case.expected_module_paths,
            linear_case.actual_module_paths
        );
        assert_eq!(
            linear_case.expected_parameter_paths,
            linear_case.actual_parameter_paths
        );
        assert_eq!(
            linear_case.expected_buffer_paths,
            linear_case.actual_buffer_paths
        );
        assert_eq!(
            linear_case.expected_state_dict_keys,
            linear_case.actual_state_dict_keys
        );

        let refusal_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "pytorch.linear.registration_order_preservation")
            .expect("missing registration-order refusal case");
        assert_eq!(refusal_case.status, ModuleParityStatus::Refused);
        assert_eq!(
            refusal_case
                .expected_refusal
                .as_ref()
                .map(|refusal| refusal.code),
            refusal_case
                .actual_refusal
                .as_ref()
                .map(|refusal| refusal.code)
        );
        assert_eq!(
            refusal_case
                .expected_refusal
                .as_ref()
                .and_then(|refusal| refusal.subject.as_deref()),
            Some("state_dict_registration_order")
        );
        assert_ne!(
            refusal_case.expected_state_dict_keys,
            refusal_case.actual_state_dict_keys
        );
        Ok(())
    }
}
