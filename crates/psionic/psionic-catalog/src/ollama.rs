use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    BlobError, BlobReadPreference, LocalBlob, LocalBlobKind, LocalBlobOpenOptions,
    canonical_ollama_digest, ollama_blob_path,
};

/// Default Ollama host applied to bare local model references.
pub const OLLAMA_DEFAULT_HOST: &str = "registry.ollama.ai";

/// Default Ollama namespace applied to bare local model references.
pub const OLLAMA_DEFAULT_NAMESPACE: &str = "library";

/// Default Ollama tag applied when the caller omits one.
pub const OLLAMA_DEFAULT_TAG: &str = "latest";

/// Catalog-side failure for Ollama manifest discovery and model resolution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CatalogError {
    /// The caller supplied a model reference that cannot be normalized honestly.
    #[error("invalid ollama model reference `{reference}`: {message}")]
    InvalidModelName {
        /// Original model reference.
        reference: String,
        /// Validation failure summary.
        message: String,
    },
    /// A manifest-relative path could not be interpreted as an Ollama model name.
    #[error("invalid ollama manifest path `{path}`: {message}")]
    InvalidManifestPath {
        /// Manifest-relative or absolute path.
        path: String,
        /// Validation failure summary.
        message: String,
    },
    /// The requested manifest does not exist in the local Ollama store.
    #[error("ollama manifest for `{model}` does not exist at `{path}`")]
    MissingManifest {
        /// Canonical fully qualified model name.
        model: String,
        /// Expected manifest path.
        path: String,
    },
    /// Reading a manifest file failed.
    #[error("failed to read ollama manifest `{path}`: {message}")]
    ReadManifest {
        /// Manifest path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// Decoding a manifest JSON payload failed.
    #[error("failed to decode ollama manifest `{path}`: {message}")]
    DecodeManifest {
        /// Manifest path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// The manifest content is structurally invalid.
    #[error("invalid ollama manifest `{path}`: {message}")]
    InvalidManifest {
        /// Manifest path.
        path: String,
        /// Validation failure summary.
        message: String,
    },
    /// Decoding a manifest layer payload failed.
    #[error("failed to decode ollama layer `{path}`: {message}")]
    DecodeLayer {
        /// Layer blob path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// Blob access delegated to the lower-level blob substrate failed.
    #[error(transparent)]
    Blob(#[from] BlobError),
}

/// Stable warning emitted while scanning the local Ollama model store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaCatalogWarning {
    /// Path that triggered the warning.
    pub path: PathBuf,
    /// High-signal failure summary.
    pub message: String,
}

impl OllamaCatalogWarning {
    fn new(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }
}

/// Verification scope for one local-store integrity diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaIntegrityScope {
    /// The manifest file itself.
    Manifest,
    /// One manifest-referenced blob.
    Blob,
}

/// Suggested repair action for one integrity failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaRepairAction {
    /// Re-pull the full model into the local store.
    RePullModel,
    /// Re-pull the referenced blob into the local store.
    RePullBlob,
    /// Remove the corrupt blob and re-pull it.
    RemoveCorruptBlobAndRePull,
}

/// Provenance class for one resolved local Ollama manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaProvenanceKind {
    /// A normal locally resolved manifest without an explicit remote alias.
    LocalManifest,
    /// A locally resolved manifest that also declares an upstream remote alias.
    RemoteAlias,
}

/// Stable provenance facts for one locally discovered Ollama manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaProvenanceFacts {
    /// Provenance class for the manifest.
    pub kind: OllamaProvenanceKind,
    /// Canonical fully qualified Ollama model name.
    pub canonical_name: String,
    /// Shortest display form for the same model.
    pub short_name: String,
    /// Stable digest over the manifest bytes.
    pub manifest_sha256: String,
    /// Declared remote host when the manifest is a remote alias.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_host: Option<String>,
    /// Declared remote model when the manifest is a remote alias.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_model: Option<String>,
    /// Declared base model when present in config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
}

/// Explicit role the local Ollama catalog plays inside Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaCatalogSurface {
    /// Compatibility/migration substrate layered over Ollama manifests and blobs.
    OllamaCompatMigration,
}

/// One declared license payload from an Ollama manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaLicenseEntry {
    /// Stable digest over the license text.
    pub sha256: String,
    /// Exact declared license text.
    pub text: String,
}

/// Stable license facts for one locally discovered Ollama manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaLicenseFacts {
    /// Whether the manifest declared any license text layers.
    pub declared: bool,
    /// Declared license payloads in manifest order.
    pub entries: Vec<OllamaLicenseEntry>,
}

impl OllamaLicenseFacts {
    /// Returns declared license digests in manifest order.
    #[must_use]
    pub fn digests(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.sha256.clone())
            .collect()
    }
}

/// One explicit integrity diagnostic for the local Ollama store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaIntegrityDiagnostic {
    /// Whether this applies to the manifest file or a referenced blob.
    pub scope: OllamaIntegrityScope,
    /// Path that failed verification.
    pub path: PathBuf,
    /// Logical layer kind when the diagnostic applies to a manifest blob.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_kind: Option<OllamaLayerKind>,
    /// Referenced digest when the diagnostic applies to a manifest blob.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// High-signal integrity failure summary.
    pub message: String,
    /// Suggested repair action for restoring the local cache entry.
    pub repair: OllamaRepairAction,
}

impl OllamaIntegrityDiagnostic {
    fn manifest(path: PathBuf, message: impl Into<String>, repair: OllamaRepairAction) -> Self {
        Self {
            scope: OllamaIntegrityScope::Manifest,
            path,
            layer_kind: None,
            digest: None,
            message: message.into(),
            repair,
        }
    }

    fn blob(
        layer: &OllamaManifestLayer,
        message: impl Into<String>,
        repair: OllamaRepairAction,
    ) -> Self {
        Self {
            scope: OllamaIntegrityScope::Blob,
            path: layer.blob_path.clone(),
            layer_kind: Some(layer.kind),
            digest: Some(layer.digest.clone()),
            message: message.into(),
            repair,
        }
    }
}

/// Integrity verification result for one resolved local Ollama manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaManifestIntegrity {
    /// Whether the resolved manifest and the referenced local blobs verified cleanly.
    pub verified: bool,
    /// Explicit diagnostics for missing, corrupt, or mismatched local cache entries.
    pub diagnostics: Vec<OllamaIntegrityDiagnostic>,
}

/// Integrity verification report for one caller-facing local model reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaModelIntegrityReport {
    /// Original caller-facing reference.
    pub requested_reference: String,
    /// Canonical parsed name when the reference normalized successfully.
    pub canonical_name: OllamaModelName,
    /// Expected manifest path inside the local store.
    pub manifest_path: PathBuf,
    /// Whether the manifest plus all verified local blobs passed integrity checks.
    pub verified: bool,
    /// Resolved manifest when it could be parsed structurally.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<OllamaManifest>,
    /// Explicit cache-repair diagnostics.
    pub diagnostics: Vec<OllamaIntegrityDiagnostic>,
}

/// Normalized Ollama model name used for local manifest resolution.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OllamaModelName {
    /// Registry host.
    pub host: String,
    /// Namespace inside the registry host.
    pub namespace: String,
    /// Model identifier.
    pub model: String,
    /// Model tag.
    pub tag: String,
}

impl OllamaModelName {
    /// Parses a caller-facing model reference using Ollama-compatible defaults.
    pub fn parse(reference: &str) -> Result<Self, CatalogError> {
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(CatalogError::InvalidModelName {
                reference: String::new(),
                message: String::from("model reference is empty"),
            });
        }
        if reference.contains('@') {
            return Err(CatalogError::InvalidModelName {
                reference: reference.to_string(),
                message: String::from(
                    "digest-qualified model references are not supported by the local catalog",
                ),
            });
        }

        let mut base = reference;
        let mut tag = None;
        if let Some(colon_index) = reference.rfind(':') {
            let slash_index = reference.rfind('/');
            if slash_index.is_none_or(|slash_index| colon_index > slash_index) {
                let (without_tag, tag_value) = reference.split_at(colon_index);
                base = without_tag;
                tag = Some(tag_value.trim_start_matches(':').to_string());
            }
        }

        let parts = base.split('/').collect::<Vec<_>>();
        let (host, namespace, model) = match parts.as_slice() {
            [model] => (OLLAMA_DEFAULT_HOST, OLLAMA_DEFAULT_NAMESPACE, *model),
            [namespace, model] => (OLLAMA_DEFAULT_HOST, *namespace, *model),
            [host, namespace, model] => (*host, *namespace, *model),
            _ => {
                return Err(CatalogError::InvalidModelName {
                    reference: reference.to_string(),
                    message: String::from(
                        "expected `model`, `namespace/model`, or `host/namespace/model`",
                    ),
                });
            }
        };

        let name = Self {
            host: host.to_string(),
            namespace: namespace.to_string(),
            model: model.to_string(),
            tag: tag.unwrap_or_else(|| String::from(OLLAMA_DEFAULT_TAG)),
        };
        name.validate(reference)?;
        Ok(name)
    }

    /// Parses a manifest-relative path in the shape `<host>/<namespace>/<model>/<tag>`.
    pub fn from_manifest_relpath(path: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let path = path.as_ref();
        let parts = path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let [host, namespace, model, tag] = parts.as_slice() else {
            return Err(CatalogError::InvalidManifestPath {
                path: path.display().to_string(),
                message: String::from("expected exactly four path segments"),
            });
        };

        let name = Self {
            host: host.clone(),
            namespace: namespace.clone(),
            model: model.clone(),
            tag: tag.clone(),
        };
        name.validate(path.display().to_string())?;
        Ok(name)
    }

    /// Returns the canonical fully qualified name.
    #[must_use]
    pub fn canonical_name(&self) -> String {
        format!(
            "{}/{}/{}:{}",
            self.host, self.namespace, self.model, self.tag
        )
    }

    /// Returns the shortest display form following Ollama's default-host/default-namespace rules.
    #[must_use]
    pub fn display_shortest(&self) -> String {
        if !self.host.eq_ignore_ascii_case(OLLAMA_DEFAULT_HOST) {
            self.canonical_name()
        } else if !self
            .namespace
            .eq_ignore_ascii_case(OLLAMA_DEFAULT_NAMESPACE)
        {
            format!("{}/{}:{}", self.namespace, self.model, self.tag)
        } else {
            format!("{}:{}", self.model, self.tag)
        }
    }

    /// Returns the relative manifest path under the Ollama `manifests` root.
    #[must_use]
    pub fn manifest_relpath(&self) -> PathBuf {
        PathBuf::from(self.host.as_str())
            .join(self.namespace.as_str())
            .join(self.model.as_str())
            .join(self.tag.as_str())
    }

    fn validate(&self, original: impl Into<String>) -> Result<(), CatalogError> {
        let original = original.into();
        validate_name_part(
            &self.host,
            NamePartKind::Host,
            &original,
            is_valid_host_char,
            350,
        )?;
        validate_name_part(
            &self.namespace,
            NamePartKind::Namespace,
            &original,
            is_valid_namespace_char,
            80,
        )?;
        validate_name_part(
            &self.model,
            NamePartKind::Model,
            &original,
            is_valid_model_char,
            80,
        )?;
        validate_name_part(
            &self.tag,
            NamePartKind::Tag,
            &original,
            is_valid_model_char,
            80,
        )?;
        Ok(())
    }
}

impl fmt::Display for OllamaModelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_name().as_str())
    }
}

/// Parsed Ollama layer media type with stable parameter extraction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaMediaType {
    /// Original media type string from the manifest.
    pub raw: String,
    /// Base media type before any `; key=value` parameters.
    pub base: String,
    /// Stable parameter map parsed from the media type.
    pub parameters: BTreeMap<String, String>,
}

impl OllamaMediaType {
    /// Parses an Ollama layer media type into base and stable parameter map components.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let mut parts = raw.split(';');
        let base = parts.next().unwrap_or_default().trim().to_string();
        let mut parameters = BTreeMap::new();
        for parameter in parts {
            let parameter = parameter.trim();
            let Some((key, value)) = parameter.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if !key.is_empty() && !value.is_empty() {
                parameters.insert(key.to_string(), value.to_string());
            }
        }
        Self {
            raw: raw.to_string(),
            base,
            parameters,
        }
    }

    /// Returns a parsed parameter value by key.
    #[must_use]
    pub fn parameter(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).map(String::as_str)
    }
}

/// Explicit logical role for one local Ollama manifest layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaLayerKind {
    /// Manifest config layer.
    Config,
    /// Primary model weight blob.
    Model,
    /// Deprecated embedding-only layer.
    Embed,
    /// Adapter layer.
    Adapter,
    /// Projector layer.
    Projector,
    /// Prompt layer.
    Prompt,
    /// Template layer.
    Template,
    /// System-prompt layer.
    System,
    /// Parameters/options layer.
    Parameters,
    /// Messages/history layer.
    Messages,
    /// License text layer.
    License,
    /// Tensor layer from the newer manifest format.
    Tensor,
    /// Tokenizer payload layer.
    Tokenizer,
    /// Tokenizer config layer.
    TokenizerConfig,
    /// Unknown or not-yet-modeled layer type.
    Unknown,
}

/// Stable manifest layer record with direct blob identity and presence truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaManifestLayer {
    /// Logical layer role.
    pub kind: OllamaLayerKind,
    /// Parsed media type.
    pub media_type: OllamaMediaType,
    /// Canonical blob digest.
    pub digest: String,
    /// Declared blob size in bytes.
    pub size_bytes: u64,
    /// Optional parent/source label carried by legacy manifests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Optional layer name carried by legacy manifests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Resolved local blob path.
    pub blob_path: PathBuf,
    /// Whether the referenced blob exists locally right now.
    pub blob_present: bool,
}

impl OllamaManifestLayer {
    /// Opens the layer blob through the shared local blob substrate.
    pub fn open_blob(&self, mut options: LocalBlobOpenOptions) -> Result<LocalBlob, CatalogError> {
        options.expected_sha256 = Some(self.digest.clone());
        Ok(LocalBlob::open_path(
            &self.blob_path,
            LocalBlobKind::OllamaBlob,
            options,
        )?)
    }

    /// Reads the full layer blob into a UTF-8 string.
    pub fn read_text(&self, options: LocalBlobOpenOptions) -> Result<String, CatalogError> {
        let blob = self.open_blob(options)?;
        String::from_utf8(blob.bytes().to_vec()).map_err(|error| CatalogError::DecodeLayer {
            path: self.blob_path.display().to_string(),
            message: error.to_string(),
        })
    }

    /// Decodes the full layer blob as JSON.
    pub fn decode_json<T: DeserializeOwned>(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<T, CatalogError> {
        let blob = self.open_blob(options)?;
        serde_json::from_slice(blob.bytes()).map_err(|error| CatalogError::DecodeLayer {
            path: self.blob_path.display().to_string(),
            message: error.to_string(),
        })
    }
}

/// Current Psionic policy for Ollama manifests that carry adapter layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaAdapterPolicy {
    /// Refuse manifest-backed loading when one or more adapter layers are present.
    RefuseManifestWithAdapters,
}

impl fmt::Display for OllamaAdapterPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RefuseManifestWithAdapters => f.write_str("refuse_manifest_with_adapters"),
        }
    }
}

/// Machine-checkable adapter support status for one local Ollama manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaAdapterPolicyStatus {
    /// Current Psionic adapter policy for Ollama manifests.
    pub policy: OllamaAdapterPolicy,
    /// Number of adapter layers carried by the manifest.
    pub adapter_layer_count: usize,
    /// Whether the manifest is admissible under the current policy.
    pub supported: bool,
}

/// One resolved local Ollama manifest plus layer identity and size truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaManifest {
    /// Canonical fully qualified model name.
    pub name: OllamaModelName,
    /// Short display name following Ollama defaults.
    pub short_name: String,
    /// Manifest file path in the local store.
    pub manifest_path: PathBuf,
    /// Manifest schema version.
    pub schema_version: u32,
    /// Manifest media type.
    pub media_type: String,
    /// Stable SHA-256 digest of the raw manifest bytes, without the `sha256:` prefix to match Ollama's API.
    pub manifest_sha256: String,
    /// Manifest file size in bytes.
    pub manifest_byte_length: u64,
    /// Sum of config and layer declared blob sizes.
    pub total_blob_size_bytes: u64,
    /// Config layer when the manifest carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<OllamaManifestLayer>,
    /// Ordered data layers.
    pub layers: Vec<OllamaManifestLayer>,
}

impl OllamaManifest {
    /// Returns the explicit catalog role this manifest belongs to inside Psionic.
    #[must_use]
    pub const fn catalog_surface(&self) -> OllamaCatalogSurface {
        OllamaCatalogSurface::OllamaCompatMigration
    }

    /// Returns the first primary model layer when present.
    #[must_use]
    pub fn primary_model_layer(&self) -> Option<&OllamaManifestLayer> {
        self.layers
            .iter()
            .find(|layer| layer.kind == OllamaLayerKind::Model)
    }

    /// Returns the first layer matching the requested logical role.
    #[must_use]
    pub fn first_layer_of_kind(&self, kind: OllamaLayerKind) -> Option<&OllamaManifestLayer> {
        self.layers.iter().find(|layer| layer.kind == kind)
    }

    /// Returns every adapter layer in manifest order.
    pub fn adapter_layers(&self) -> impl Iterator<Item = &OllamaManifestLayer> {
        self.layers
            .iter()
            .filter(|layer| layer.kind == OllamaLayerKind::Adapter)
    }

    /// Returns the number of adapter layers carried by the manifest.
    #[must_use]
    pub fn adapter_layer_count(&self) -> usize {
        self.adapter_layers().count()
    }

    /// Returns the current Psionic adapter support status for the manifest.
    #[must_use]
    pub fn adapter_policy_status(&self) -> OllamaAdapterPolicyStatus {
        let adapter_layer_count = self.adapter_layer_count();
        OllamaAdapterPolicyStatus {
            policy: OllamaAdapterPolicy::RefuseManifestWithAdapters,
            adapter_layer_count,
            supported: adapter_layer_count == 0,
        }
    }

    /// Returns the manifest file modification time.
    pub fn modified_at(&self) -> Result<SystemTime, CatalogError> {
        fs::metadata(&self.manifest_path)
            .map_err(|error| CatalogError::ReadManifest {
                path: self.manifest_path.display().to_string(),
                message: error.to_string(),
            })?
            .modified()
            .map_err(|error| CatalogError::ReadManifest {
                path: self.manifest_path.display().to_string(),
                message: error.to_string(),
            })
    }

    /// Loads the manifest config blob when one exists.
    pub fn load_config(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<Option<OllamaModelConfig>, CatalogError> {
        self.config
            .as_ref()
            .map(|layer| layer.decode_json(options))
            .transpose()
    }

    /// Loads the effective prompt/template layer text when present.
    pub fn load_template(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<Option<String>, CatalogError> {
        self.load_last_text_layer(
            &[OllamaLayerKind::Prompt, OllamaLayerKind::Template],
            options,
        )
    }

    /// Loads the effective system-prompt layer text when present.
    pub fn load_system_prompt(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<Option<String>, CatalogError> {
        self.load_last_text_layer(&[OllamaLayerKind::System], options)
    }

    /// Loads the effective parameters/options layer when present.
    pub fn load_parameters(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<Option<BTreeMap<String, Value>>, CatalogError> {
        self.load_last_json_layer(OllamaLayerKind::Parameters, options)
    }

    /// Loads the effective message-history layer when present.
    pub fn load_messages(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<Vec<OllamaStoredMessage>, CatalogError> {
        Ok(self
            .load_last_json_layer::<Vec<OllamaStoredMessage>>(OllamaLayerKind::Messages, options)?
            .unwrap_or_default())
    }

    /// Loads all license text blobs in manifest order.
    pub fn load_licenses(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<Vec<String>, CatalogError> {
        let mut licenses = Vec::new();
        for layer in self
            .layers
            .iter()
            .filter(|layer| layer.kind == OllamaLayerKind::License)
        {
            licenses.push(layer.read_text(options.clone())?);
        }
        Ok(licenses)
    }

    /// Returns stable provenance facts for the resolved manifest, separate from integrity checks.
    #[must_use]
    pub fn provenance_facts(&self, config: Option<&OllamaModelConfig>) -> OllamaProvenanceFacts {
        let remote_host = config
            .and_then(OllamaModelConfig::remote_host)
            .map(str::to_string);
        let remote_model = config
            .and_then(OllamaModelConfig::remote_model)
            .map(str::to_string);

        OllamaProvenanceFacts {
            kind: if remote_host.is_some() || remote_model.is_some() {
                OllamaProvenanceKind::RemoteAlias
            } else {
                OllamaProvenanceKind::LocalManifest
            },
            canonical_name: self.name.canonical_name(),
            short_name: self.short_name.clone(),
            manifest_sha256: self.manifest_sha256.clone(),
            remote_host,
            remote_model,
            base_model: config
                .and_then(OllamaModelConfig::base_name)
                .map(str::to_string),
        }
    }

    /// Loads stable license facts from the manifest, separate from integrity checks.
    pub fn load_license_facts(
        &self,
        options: LocalBlobOpenOptions,
    ) -> Result<OllamaLicenseFacts, CatalogError> {
        let entries = self
            .load_licenses(options)?
            .into_iter()
            .map(|text| OllamaLicenseEntry {
                sha256: hex::encode(Sha256::digest(text.as_bytes())),
                text,
            })
            .collect::<Vec<_>>();

        Ok(OllamaLicenseFacts {
            declared: !entries.is_empty(),
            entries,
        })
    }

    fn load_last_text_layer(
        &self,
        kinds: &[OllamaLayerKind],
        options: LocalBlobOpenOptions,
    ) -> Result<Option<String>, CatalogError> {
        let mut value = None;
        for layer in self
            .layers
            .iter()
            .filter(|layer| kinds.contains(&layer.kind))
        {
            value = Some(layer.read_text(options.clone())?);
        }
        Ok(value)
    }

    fn load_last_json_layer<T: DeserializeOwned>(
        &self,
        kind: OllamaLayerKind,
        options: LocalBlobOpenOptions,
    ) -> Result<Option<T>, CatalogError> {
        let mut value = None;
        for layer in self.layers.iter().filter(|layer| layer.kind == kind) {
            value = Some(layer.decode_json(options.clone())?);
        }
        Ok(value)
    }

    /// Verifies that every referenced local blob exists and matches the manifest's digest and size declarations.
    #[must_use]
    pub fn verify_integrity(&self, options: LocalBlobOpenOptions) -> OllamaManifestIntegrity {
        let mut diagnostics = Vec::new();
        let verification_options = options.with_read_preference(BlobReadPreference::PreferBuffered);

        if let Some(diagnostic) = self
            .config
            .as_ref()
            .and_then(|config| verify_manifest_layer_blob(config, &verification_options))
        {
            diagnostics.push(diagnostic);
        }
        for layer in &self.layers {
            if let Some(diagnostic) = verify_manifest_layer_blob(layer, &verification_options) {
                diagnostics.push(diagnostic);
            }
        }

        OllamaManifestIntegrity {
            verified: diagnostics.is_empty(),
            diagnostics,
        }
    }
}

fn verify_manifest_layer_blob(
    layer: &OllamaManifestLayer,
    options: &LocalBlobOpenOptions,
) -> Option<OllamaIntegrityDiagnostic> {
    let blob = match layer.open_blob(options.clone()) {
        Ok(blob) => blob,
        Err(CatalogError::Blob(BlobError::MissingFile { .. })) => {
            return Some(OllamaIntegrityDiagnostic::blob(
                layer,
                format!("missing local blob `{}`", layer.digest),
                OllamaRepairAction::RePullBlob,
            ));
        }
        Err(CatalogError::Blob(BlobError::DigestMismatch {
            expected, actual, ..
        })) => {
            return Some(OllamaIntegrityDiagnostic::blob(
                layer,
                format!("blob digest mismatch: expected `{expected}`, actual `{actual}`"),
                OllamaRepairAction::RemoveCorruptBlobAndRePull,
            ));
        }
        Err(CatalogError::Blob(BlobError::Read { message, .. }))
        | Err(CatalogError::Blob(BlobError::MemoryMap { message, .. })) => {
            return Some(OllamaIntegrityDiagnostic::blob(
                layer,
                format!("failed to read local blob: {message}"),
                OllamaRepairAction::RemoveCorruptBlobAndRePull,
            ));
        }
        Err(CatalogError::Blob(BlobError::InvalidPageSize { page_size })) => {
            return Some(OllamaIntegrityDiagnostic::blob(
                layer,
                format!("invalid verification page size `{page_size}`"),
                OllamaRepairAction::RePullBlob,
            ));
        }
        Err(CatalogError::Blob(BlobError::InvalidDigestFormat { digest })) => {
            return Some(OllamaIntegrityDiagnostic::blob(
                layer,
                format!("invalid blob digest `{digest}`"),
                OllamaRepairAction::RePullBlob,
            ));
        }
        Err(CatalogError::Blob(BlobError::RangeOutOfBounds { .. }))
        | Err(CatalogError::Blob(BlobError::PageOutOfBounds { .. }))
        | Err(CatalogError::InvalidManifest { .. })
        | Err(CatalogError::InvalidManifestPath { .. })
        | Err(CatalogError::InvalidModelName { .. })
        | Err(CatalogError::MissingManifest { .. })
        | Err(CatalogError::ReadManifest { .. })
        | Err(CatalogError::DecodeManifest { .. })
        | Err(CatalogError::DecodeLayer { .. }) => {
            return Some(OllamaIntegrityDiagnostic::blob(
                layer,
                "failed to verify local blob",
                OllamaRepairAction::RemoveCorruptBlobAndRePull,
            ));
        }
    };

    if blob.metadata().byte_length != layer.size_bytes {
        return Some(OllamaIntegrityDiagnostic::blob(
            layer,
            format!(
                "blob size mismatch: expected {} bytes, actual {} bytes",
                layer.size_bytes,
                blob.metadata().byte_length
            ),
            OllamaRepairAction::RemoveCorruptBlobAndRePull,
        ));
    }

    None
}

/// Decoded Ollama config layer with the fields needed by local list/show parity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaModelConfig {
    /// User-visible weight format label.
    #[serde(default)]
    pub model_format: String,
    /// User-visible model family label.
    #[serde(default)]
    pub model_family: String,
    /// Additional family aliases.
    #[serde(default)]
    pub model_families: Vec<String>,
    /// User-visible parameter-size label.
    #[serde(default)]
    pub model_type: String,
    /// User-visible quantization label.
    #[serde(default)]
    pub file_type: String,
    /// Optional renderer label.
    #[serde(default)]
    pub renderer: String,
    /// Optional parser label.
    #[serde(default)]
    pub parser: String,
    /// Optional runtime requirement string.
    #[serde(default)]
    pub requires: String,
    /// Optional upstream host for remote aliases.
    #[serde(default)]
    pub remote_host: String,
    /// Optional upstream model for remote aliases.
    #[serde(default)]
    pub remote_model: String,
    /// Explicit capability list for non-GGUF-backed models.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Remote-model context window when explicitly declared.
    #[serde(default)]
    pub context_length: usize,
    /// Remote-model embedding width when explicitly declared.
    #[serde(default)]
    pub embedding_length: usize,
    /// Optional base model name.
    #[serde(default)]
    pub base_name: String,
}

impl OllamaModelConfig {
    /// Returns the non-empty model format.
    #[must_use]
    pub fn format(&self) -> Option<&str> {
        non_empty(self.model_format.as_str())
    }

    /// Returns the non-empty primary family label.
    #[must_use]
    pub fn family(&self) -> Option<&str> {
        non_empty(self.model_family.as_str())
    }

    /// Returns stable family aliases with empties removed.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        self.model_families
            .iter()
            .filter_map(|value| non_empty(value.as_str()).map(str::to_string))
            .collect()
    }

    /// Returns the non-empty parameter-size label.
    #[must_use]
    pub fn parameter_size(&self) -> Option<&str> {
        non_empty(self.model_type.as_str())
    }

    /// Returns the non-empty quantization label.
    #[must_use]
    pub fn quantization_level(&self) -> Option<&str> {
        non_empty(self.file_type.as_str())
    }

    /// Returns the non-empty parser label.
    #[must_use]
    pub fn parser(&self) -> Option<&str> {
        non_empty(self.parser.as_str())
    }

    /// Returns the non-empty requirement string.
    #[must_use]
    pub fn requires(&self) -> Option<&str> {
        non_empty(self.requires.as_str())
    }

    /// Returns the non-empty remote host.
    #[must_use]
    pub fn remote_host(&self) -> Option<&str> {
        non_empty(self.remote_host.as_str())
    }

    /// Returns the non-empty remote model.
    #[must_use]
    pub fn remote_model(&self) -> Option<&str> {
        non_empty(self.remote_model.as_str())
    }

    /// Returns the declared non-empty capabilities in stable file order.
    #[must_use]
    pub fn capabilities(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .filter_map(|value| non_empty(value.as_str()).map(str::to_string))
            .collect()
    }

    /// Returns the declared context length when non-zero.
    #[must_use]
    pub fn context_length(&self) -> Option<usize> {
        (self.context_length > 0).then_some(self.context_length)
    }

    /// Returns the declared embedding length when non-zero.
    #[must_use]
    pub fn embedding_length(&self) -> Option<usize> {
        (self.embedding_length > 0).then_some(self.embedding_length)
    }

    /// Returns the non-empty base model name.
    #[must_use]
    pub fn base_name(&self) -> Option<&str> {
        non_empty(self.base_name.as_str())
    }
}

/// Decoded message-history row from an Ollama manifest layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaStoredMessage {
    /// Message role label.
    pub role: String,
    /// Message content.
    pub content: String,
}

/// Discovery result over the local Ollama manifest tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaCatalogDiscovery {
    /// Successfully decoded manifests in stable canonical-name order.
    pub manifests: Vec<OllamaManifest>,
    /// Explicit warnings for invalid or unreadable manifest entries that were skipped.
    pub warnings: Vec<OllamaCatalogWarning>,
}

/// Non-mutating local Ollama catalog view rooted at one `OLLAMA_MODELS` directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaModelCatalog {
    models_root: PathBuf,
}

impl OllamaModelCatalog {
    /// Creates a local catalog rooted at an Ollama models directory.
    #[must_use]
    pub fn new(models_root: impl AsRef<Path>) -> Self {
        Self {
            models_root: models_root.as_ref().to_path_buf(),
        }
    }

    /// Returns the explicit catalog role this substrate plays inside Psionic.
    #[must_use]
    pub const fn surface(&self) -> OllamaCatalogSurface {
        OllamaCatalogSurface::OllamaCompatMigration
    }

    /// Returns the catalog's models root.
    #[must_use]
    pub fn models_root(&self) -> &Path {
        &self.models_root
    }

    /// Returns the local `manifests` root path without mutating the filesystem.
    #[must_use]
    pub fn manifests_root(&self) -> PathBuf {
        ollama_manifests_root(&self.models_root)
    }

    /// Discovers every local manifest reachable under the `manifests` tree.
    pub fn discover_models(&self) -> Result<OllamaCatalogDiscovery, CatalogError> {
        let manifests_root = self.manifests_root();
        if !manifests_root.exists() {
            return Ok(OllamaCatalogDiscovery {
                manifests: Vec::new(),
                warnings: Vec::new(),
            });
        }

        let mut manifests = Vec::new();
        let mut warnings = Vec::new();
        scan_manifest_tree(
            &self.models_root,
            &manifests_root,
            &mut manifests,
            &mut warnings,
        )?;
        manifests.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(OllamaCatalogDiscovery {
            manifests,
            warnings,
        })
    }

    /// Resolves one caller-facing model reference to a local manifest.
    pub fn resolve_model(&self, reference: &str) -> Result<OllamaManifest, CatalogError> {
        let name = OllamaModelName::parse(reference)?;
        self.resolve_name(&name)
    }

    /// Resolves one already-normalized model name to a local manifest.
    pub fn resolve_name(&self, name: &OllamaModelName) -> Result<OllamaManifest, CatalogError> {
        let path = ollama_manifest_path(&self.models_root, name);
        if !path.exists() {
            return Err(CatalogError::MissingManifest {
                model: name.canonical_name(),
                path: path.display().to_string(),
            });
        }
        parse_manifest_file(&path, name.clone(), &self.models_root)
    }

    /// Verifies manifest and blob integrity for one caller-facing local model reference.
    pub fn verify_model_integrity(
        &self,
        reference: &str,
        options: LocalBlobOpenOptions,
    ) -> Result<OllamaModelIntegrityReport, CatalogError> {
        let name = OllamaModelName::parse(reference)?;
        let manifest_path = ollama_manifest_path(&self.models_root, &name);

        let manifest = match fs::read(&manifest_path) {
            Ok(bytes) => {
                match parse_manifest_bytes(&bytes, &manifest_path, name.clone(), &self.models_root)
                {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        return Ok(OllamaModelIntegrityReport {
                            requested_reference: reference.to_string(),
                            canonical_name: name,
                            manifest_path: manifest_path.clone(),
                            verified: false,
                            manifest: None,
                            diagnostics: vec![OllamaIntegrityDiagnostic::manifest(
                                manifest_path.clone(),
                                error.to_string(),
                                OllamaRepairAction::RePullModel,
                            )],
                        });
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(OllamaModelIntegrityReport {
                    requested_reference: reference.to_string(),
                    canonical_name: name,
                    manifest_path: manifest_path.clone(),
                    verified: false,
                    manifest: None,
                    diagnostics: vec![OllamaIntegrityDiagnostic::manifest(
                        manifest_path,
                        format!("missing local manifest for `{reference}`"),
                        OllamaRepairAction::RePullModel,
                    )],
                });
            }
            Err(error) => {
                return Ok(OllamaModelIntegrityReport {
                    requested_reference: reference.to_string(),
                    canonical_name: name,
                    manifest_path: manifest_path.clone(),
                    verified: false,
                    manifest: None,
                    diagnostics: vec![OllamaIntegrityDiagnostic::manifest(
                        manifest_path,
                        error.to_string(),
                        OllamaRepairAction::RePullModel,
                    )],
                });
            }
        };

        let integrity = manifest.verify_integrity(options);
        Ok(OllamaModelIntegrityReport {
            requested_reference: reference.to_string(),
            canonical_name: name,
            manifest_path,
            verified: integrity.verified,
            manifest: Some(manifest),
            diagnostics: integrity.diagnostics,
        })
    }
}

/// Returns the local Ollama manifests root for the provided models directory.
#[must_use]
pub fn ollama_manifests_root(models_root: impl AsRef<Path>) -> PathBuf {
    models_root.as_ref().join("manifests")
}

/// Returns the manifest path for one normalized local Ollama model name.
#[must_use]
pub fn ollama_manifest_path(models_root: impl AsRef<Path>, name: &OllamaModelName) -> PathBuf {
    ollama_manifests_root(models_root).join(name.manifest_relpath())
}

#[derive(Clone, Copy)]
enum NamePartKind {
    Host,
    Namespace,
    Model,
    Tag,
}

impl NamePartKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Namespace => "namespace",
            Self::Model => "model",
            Self::Tag => "tag",
        }
    }
}

fn validate_name_part(
    value: &str,
    kind: NamePartKind,
    original: &str,
    is_valid_rest: fn(char) -> bool,
    max_len: usize,
) -> Result<(), CatalogError> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(CatalogError::InvalidModelName {
            reference: original.to_string(),
            message: format!("missing {} segment", kind.as_str()),
        });
    };
    if value.len() > max_len {
        return Err(CatalogError::InvalidModelName {
            reference: original.to_string(),
            message: format!("{} segment exceeds {max_len} bytes", kind.as_str()),
        });
    }
    if !(first.is_ascii_alphanumeric() || first == '_') {
        return Err(CatalogError::InvalidModelName {
            reference: original.to_string(),
            message: format!(
                "{} segment `{value}` starts with an invalid character",
                kind.as_str()
            ),
        });
    }
    if !chars.all(is_valid_rest) {
        return Err(CatalogError::InvalidModelName {
            reference: original.to_string(),
            message: format!(
                "{} segment `{value}` contains invalid characters",
                kind.as_str()
            ),
        });
    }
    Ok(())
}

fn is_valid_host_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
}

fn is_valid_namespace_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
}

fn is_valid_model_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn scan_manifest_tree(
    models_root: &Path,
    manifests_root: &Path,
    manifests: &mut Vec<OllamaManifest>,
    warnings: &mut Vec<OllamaCatalogWarning>,
) -> Result<(), CatalogError> {
    for host_entry in read_directory(manifests_root)? {
        let Ok(host_entry) = host_entry else {
            continue;
        };
        let host_path = host_entry.path();
        if !is_directory(&host_entry, warnings)? {
            continue;
        }
        for namespace_entry in read_directory(&host_path)? {
            let Ok(namespace_entry) = namespace_entry else {
                continue;
            };
            let namespace_path = namespace_entry.path();
            if !is_directory(&namespace_entry, warnings)? {
                continue;
            }
            for model_entry in read_directory(&namespace_path)? {
                let Ok(model_entry) = model_entry else {
                    continue;
                };
                let model_path = model_entry.path();
                if !is_directory(&model_entry, warnings)? {
                    continue;
                }
                for tag_entry in read_directory(&model_path)? {
                    let Ok(tag_entry) = tag_entry else {
                        continue;
                    };
                    let tag_path = tag_entry.path();
                    if is_directory(&tag_entry, warnings)? {
                        warnings.push(OllamaCatalogWarning::new(
                            tag_path,
                            "expected manifest file at tag path but found a directory",
                        ));
                        continue;
                    }
                    let relative = match tag_path.strip_prefix(manifests_root) {
                        Ok(relative) => relative,
                        Err(error) => {
                            warnings.push(OllamaCatalogWarning::new(
                                tag_path,
                                format!("failed to compute manifest-relative path: {error}"),
                            ));
                            continue;
                        }
                    };
                    let name = match OllamaModelName::from_manifest_relpath(relative) {
                        Ok(name) => name,
                        Err(error) => {
                            warnings.push(warning_from_error(relative.to_path_buf(), error));
                            continue;
                        }
                    };
                    match parse_manifest_file(&tag_path, name, models_root) {
                        Ok(manifest) => manifests.push(manifest),
                        Err(error) => warnings.push(warning_from_error(tag_path, error)),
                    }
                }
            }
        }
    }
    Ok(())
}

fn read_directory(path: &Path) -> Result<fs::ReadDir, CatalogError> {
    fs::read_dir(path).map_err(|error| CatalogError::ReadManifest {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn is_directory(
    entry: &fs::DirEntry,
    warnings: &mut Vec<OllamaCatalogWarning>,
) -> Result<bool, CatalogError> {
    match entry.file_type() {
        Ok(file_type) => Ok(file_type.is_dir()),
        Err(error) => {
            warnings.push(OllamaCatalogWarning::new(
                entry.path(),
                format!("failed to inspect entry type: {error}"),
            ));
            Ok(false)
        }
    }
}

fn warning_from_error(path: PathBuf, error: CatalogError) -> OllamaCatalogWarning {
    OllamaCatalogWarning::new(path, error.to_string())
}

fn parse_manifest_file(
    path: &Path,
    name: OllamaModelName,
    models_root: &Path,
) -> Result<OllamaManifest, CatalogError> {
    let bytes = fs::read(path).map_err(|error| CatalogError::ReadManifest {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    parse_manifest_bytes(&bytes, path, name, models_root)
}

pub(crate) fn parse_manifest_bytes(
    bytes: &[u8],
    path: &Path,
    name: OllamaModelName,
    models_root: &Path,
) -> Result<OllamaManifest, CatalogError> {
    let raw = serde_json::from_slice::<RawManifest>(bytes).map_err(|error| {
        CatalogError::DecodeManifest {
            path: path.display().to_string(),
            message: error.to_string(),
        }
    })?;

    let config = raw
        .config
        .map(|config| parse_layer(path, models_root, config, OllamaLayerKind::Config))
        .transpose()?;
    let layers = raw
        .layers
        .into_iter()
        .map(|layer| {
            let media_type = OllamaMediaType::parse(layer.media_type.as_str());
            let kind = classify_layer_kind(media_type.base.as_str());
            parse_layer(path, models_root, layer, kind)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let total_blob_size_bytes = config.as_ref().map_or(0, |config| config.size_bytes)
        + layers.iter().map(|layer| layer.size_bytes).sum::<u64>();

    Ok(OllamaManifest {
        short_name: name.display_shortest(),
        name,
        manifest_path: path.to_path_buf(),
        schema_version: raw.schema_version,
        media_type: raw.media_type,
        manifest_sha256: hex::encode(Sha256::digest(bytes)),
        manifest_byte_length: bytes.len() as u64,
        total_blob_size_bytes,
        config,
        layers,
    })
}

fn parse_layer(
    manifest_path: &Path,
    models_root: &Path,
    raw: RawManifestLayer,
    kind: OllamaLayerKind,
) -> Result<OllamaManifestLayer, CatalogError> {
    if raw.media_type.trim().is_empty() {
        return Err(CatalogError::InvalidManifest {
            path: manifest_path.display().to_string(),
            message: String::from("manifest layer is missing a media type"),
        });
    }
    if raw.digest.trim().is_empty() {
        return Err(CatalogError::InvalidManifest {
            path: manifest_path.display().to_string(),
            message: String::from("manifest layer is missing a digest"),
        });
    }

    let digest = canonical_ollama_digest(raw.digest.as_str()).map_err(|error| {
        CatalogError::InvalidManifest {
            path: manifest_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let blob_path = ollama_blob_path(models_root, digest.as_str())?;

    Ok(OllamaManifestLayer {
        kind,
        media_type: OllamaMediaType::parse(raw.media_type.as_str()),
        digest,
        size_bytes: raw.size,
        from: raw.from,
        name: raw.name,
        blob_present: blob_path.exists(),
        blob_path,
    })
}

fn classify_layer_kind(base_media_type: &str) -> OllamaLayerKind {
    match base_media_type {
        "application/vnd.ollama.image.config"
        | "application/vnd.docker.container.image.v1+json" => OllamaLayerKind::Config,
        "application/vnd.ollama.image.model" => OllamaLayerKind::Model,
        "application/vnd.ollama.image.embed" => OllamaLayerKind::Embed,
        "application/vnd.ollama.image.adapter" => OllamaLayerKind::Adapter,
        "application/vnd.ollama.image.projector" => OllamaLayerKind::Projector,
        "application/vnd.ollama.image.prompt" => OllamaLayerKind::Prompt,
        "application/vnd.ollama.image.template" => OllamaLayerKind::Template,
        "application/vnd.ollama.image.system" => OllamaLayerKind::System,
        "application/vnd.ollama.image.params" => OllamaLayerKind::Parameters,
        "application/vnd.ollama.image.messages" => OllamaLayerKind::Messages,
        "application/vnd.ollama.image.license" => OllamaLayerKind::License,
        "application/vnd.ollama.image.tensor" => OllamaLayerKind::Tensor,
        "application/vnd.ollama.image.tokenizer" => OllamaLayerKind::Tokenizer,
        "application/vnd.ollama.image.tokenizer.config" => OllamaLayerKind::TokenizerConfig,
        _ => OllamaLayerKind::Unknown,
    }
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    #[serde(default, rename = "schemaVersion")]
    schema_version: u32,
    #[serde(default, rename = "mediaType")]
    media_type: String,
    #[serde(default)]
    config: Option<RawManifestLayer>,
    #[serde(default)]
    layers: Vec<RawManifestLayer>,
}

#[derive(Debug, Deserialize)]
struct RawManifestLayer {
    #[serde(default, rename = "mediaType")]
    media_type: String,
    #[serde(default)]
    digest: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic_in_result_fn)]

    use std::{collections::BTreeMap, fs, io::Write, path::Path};

    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use crate::{BlobReadPreference, LocalBlobOpenOptions};

    use super::{
        CatalogError, OLLAMA_DEFAULT_HOST, OLLAMA_DEFAULT_NAMESPACE, OLLAMA_DEFAULT_TAG,
        OllamaAdapterPolicy, OllamaCatalogSurface, OllamaIntegrityScope, OllamaLayerKind,
        OllamaMediaType, OllamaModelCatalog, OllamaModelConfig, OllamaModelName,
        OllamaProvenanceKind, OllamaRepairAction, OllamaStoredMessage,
    };

    #[test]
    fn ollama_model_name_applies_defaults_and_shortest_display()
    -> Result<(), Box<dyn std::error::Error>> {
        let bare = OllamaModelName::parse("qwen2")?;
        assert_eq!(bare.host, OLLAMA_DEFAULT_HOST);
        assert_eq!(bare.namespace, OLLAMA_DEFAULT_NAMESPACE);
        assert_eq!(bare.model, "qwen2");
        assert_eq!(bare.tag, OLLAMA_DEFAULT_TAG);
        assert_eq!(
            bare.canonical_name(),
            "registry.ollama.ai/library/qwen2:latest"
        );
        assert_eq!(bare.display_shortest(), "qwen2:latest");

        let namespaced = OllamaModelName::parse("acme/qwen2:4b")?;
        assert_eq!(namespaced.display_shortest(), "acme/qwen2:4b");

        let hosted = OllamaModelName::parse("localhost:11434/acme/qwen2:4b")?;
        assert_eq!(hosted.host, "localhost:11434");
        assert_eq!(hosted.display_shortest(), "localhost:11434/acme/qwen2:4b");
        Ok(())
    }

    #[test]
    fn ollama_model_name_rejects_invalid_manifest_hidden_tag() {
        let error = OllamaModelName::from_manifest_relpath(Path::new(
            "registry.ollama.ai/library/qwen2/.hidden",
        ))
        .expect_err("hidden tag should be rejected");

        assert!(matches!(error, CatalogError::InvalidModelName { .. }));
    }

    #[test]
    fn ollama_media_type_parses_stable_parameters() {
        let media_type = OllamaMediaType::parse(
            "application/vnd.ollama.image.tensor; name=input; dtype=F32; shape=1,2,3",
        );

        assert_eq!(media_type.base, "application/vnd.ollama.image.tensor");
        assert_eq!(media_type.parameter("name"), Some("input"));
        assert_eq!(media_type.parameter("dtype"), Some("F32"));
        assert_eq!(media_type.parameter("shape"), Some("1,2,3"));
    }

    #[test]
    fn ollama_catalog_discovers_local_manifests_and_warnings()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let config_digest = write_blob(temp.path(), br#"{"model_family":"qwen2"}"#)?;
        let model_digest = write_blob(temp.path(), b"gguf-model-bytes")?;
        let template_digest = write_blob(temp.path(), b"{{ prompt }}")?;

        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": config_digest,
                    "size": 24
                },
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 16,
                        "from": "qwen2"
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.template; name=default",
                        "digest": template_digest,
                        "size": 12
                    }
                ]
            }),
        )?;
        write_manifest_raw(temp.path(), "registry.ollama.ai/library/bad/.hidden", b"{}")?;

        let discovery = OllamaModelCatalog::new(temp.path()).discover_models()?;
        assert_eq!(discovery.manifests.len(), 1);
        assert_eq!(discovery.warnings.len(), 1);

        let manifest = &discovery.manifests[0];
        assert_eq!(manifest.short_name, "qwen2:latest");
        assert_eq!(
            manifest.name.canonical_name(),
            "registry.ollama.ai/library/qwen2:latest"
        );
        assert_eq!(manifest.total_blob_size_bytes, 52);
        assert_eq!(
            manifest.config.as_ref().map(|layer| layer.kind),
            Some(OllamaLayerKind::Config)
        );
        assert_eq!(
            manifest.primary_model_layer().map(|layer| layer.kind),
            Some(OllamaLayerKind::Model)
        );
        assert!(
            manifest
                .layers
                .iter()
                .any(|layer| layer.kind == OllamaLayerKind::Template)
        );
        Ok(())
    }

    #[test]
    fn ollama_catalog_resolves_model_and_opens_model_blob() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let model_digest = write_blob(temp.path(), b"qwen2-gguf")?;
        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 10
                    }
                ]
            }),
        )?;

        let catalog = OllamaModelCatalog::new(temp.path());
        let manifest = catalog.resolve_model("qwen2")?;
        let model_layer = manifest.primary_model_layer().expect("model layer");
        let blob = model_layer.open_blob(
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered),
        )?;

        assert_eq!(blob.bytes(), b"qwen2-gguf");
        assert!(model_layer.blob_present);
        assert_eq!(
            manifest.manifest_byte_length,
            fs::metadata(&manifest.manifest_path)?.len()
        );
        Ok(())
    }

    #[test]
    fn ollama_catalog_reports_missing_manifest() {
        let catalog = OllamaModelCatalog::new("/tmp/does-not-exist");
        let error = catalog
            .resolve_model("qwen2")
            .expect_err("missing manifest should fail");

        assert!(matches!(error, CatalogError::MissingManifest { .. }));
    }

    #[test]
    fn ollama_catalog_verifies_clean_local_store() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let model_digest = write_blob(temp.path(), b"qwen2-gguf")?;
        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 10
                    }
                ]
            }),
        )?;

        let report = OllamaModelCatalog::new(temp.path())
            .verify_model_integrity("qwen2", LocalBlobOpenOptions::default())?;

        assert!(report.verified);
        assert!(report.diagnostics.is_empty());
        assert!(report.manifest.is_some());
        Ok(())
    }

    #[test]
    fn ollama_catalog_integrity_reports_missing_manifest_with_repair_hint()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = OllamaModelCatalog::new("/tmp/does-not-exist")
            .verify_model_integrity("qwen2", LocalBlobOpenOptions::default())?;

        assert!(!report.verified);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].scope, OllamaIntegrityScope::Manifest);
        assert_eq!(
            report.diagnostics[0].repair,
            OllamaRepairAction::RePullModel
        );
        Ok(())
    }

    #[test]
    fn ollama_catalog_integrity_reports_missing_blob_with_repair_hint()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "size": 10
                    }
                ]
            }),
        )?;

        let report = OllamaModelCatalog::new(temp.path())
            .verify_model_integrity("qwen2", LocalBlobOpenOptions::default())?;

        assert!(!report.verified);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].scope, OllamaIntegrityScope::Blob);
        assert_eq!(
            report.diagnostics[0].layer_kind,
            Some(OllamaLayerKind::Model)
        );
        assert_eq!(report.diagnostics[0].repair, OllamaRepairAction::RePullBlob);
        Ok(())
    }

    #[test]
    fn ollama_catalog_integrity_reports_corrupt_blob_and_declared_size_mismatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let expected_bytes = b"qwen2-gguf";
        let digest = format!("sha256:{:x}", Sha256::digest(expected_bytes));
        let blob_path = temp.path().join("blobs").join(digest.replace(':', "-"));
        fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        fs::write(&blob_path, b"corrupt-model")?;
        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": digest,
                        "size": expected_bytes.len()
                    }
                ]
            }),
        )?;

        let report = OllamaModelCatalog::new(temp.path())
            .verify_model_integrity("qwen2", LocalBlobOpenOptions::default())?;

        assert!(!report.verified);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].scope, OllamaIntegrityScope::Blob);
        assert_eq!(
            report.diagnostics[0].repair,
            OllamaRepairAction::RemoveCorruptBlobAndRePull
        );
        assert!(
            report.diagnostics[0]
                .message
                .contains("blob digest mismatch")
        );

        fs::write(&blob_path, expected_bytes)?;
        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": format!("sha256:{:x}", Sha256::digest(expected_bytes)),
                        "size": expected_bytes.len() + 1
                    }
                ]
            }),
        )?;

        let report = OllamaModelCatalog::new(temp.path())
            .verify_model_integrity("qwen2", LocalBlobOpenOptions::default())?;
        assert!(!report.verified);
        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0].message.contains("blob size mismatch"));
        Ok(())
    }

    #[test]
    fn ollama_manifest_loads_config_and_optional_layers() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let config_digest = write_blob(
            temp.path(),
            br#"{
                "model_format":"gguf",
                "model_family":"qwen2",
                "model_families":["qwen2","qwen"],
                "model_type":"7B",
                "file_type":"Q4_0",
                "parser":"qwen2",
                "requires":"metal",
                "remote_host":"registry.ollama.ai",
                "remote_model":"library/qwen2"
            }"#,
        )?;
        let template_digest = write_blob(temp.path(), b"{{ prompt }}")?;
        let system_digest = write_blob(temp.path(), b"You are helpful.")?;
        let params_digest = write_blob(temp.path(), br#"{"seed":42,"stop":["<|im_end|>"]}"#)?;
        let messages_digest = write_blob(
            temp.path(),
            br#"[{"role":"user","content":"hello"},{"role":"assistant","content":"hi"}]"#,
        )?;
        let license_digest = write_blob(temp.path(), b"Apache-2.0")?;

        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": config_digest,
                    "size": 1
                },
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.template",
                        "digest": template_digest,
                        "size": 1
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.system",
                        "digest": system_digest,
                        "size": 1
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.params",
                        "digest": params_digest,
                        "size": 1
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.messages",
                        "digest": messages_digest,
                        "size": 1
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.license",
                        "digest": license_digest,
                        "size": 1
                    }
                ]
            }),
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2")?;
        let config = manifest
            .load_config(LocalBlobOpenOptions::default())?
            .expect("config layer");
        assert_eq!(
            config,
            OllamaModelConfig {
                model_format: String::from("gguf"),
                model_family: String::from("qwen2"),
                model_families: vec![String::from("qwen2"), String::from("qwen")],
                model_type: String::from("7B"),
                file_type: String::from("Q4_0"),
                renderer: String::new(),
                parser: String::from("qwen2"),
                requires: String::from("metal"),
                remote_host: String::from("registry.ollama.ai"),
                remote_model: String::from("library/qwen2"),
                capabilities: Vec::new(),
                context_length: 0,
                embedding_length: 0,
                base_name: String::new(),
            }
        );
        assert_eq!(
            manifest.load_template(LocalBlobOpenOptions::default())?,
            Some(String::from("{{ prompt }}"))
        );
        assert_eq!(
            manifest.load_system_prompt(LocalBlobOpenOptions::default())?,
            Some(String::from("You are helpful."))
        );
        assert_eq!(
            manifest.load_parameters(LocalBlobOpenOptions::default())?,
            Some(BTreeMap::from([
                (String::from("seed"), serde_json::json!(42)),
                (String::from("stop"), serde_json::json!(["<|im_end|>"])),
            ]))
        );
        assert_eq!(
            manifest.load_messages(LocalBlobOpenOptions::default())?,
            vec![
                OllamaStoredMessage {
                    role: String::from("user"),
                    content: String::from("hello"),
                },
                OllamaStoredMessage {
                    role: String::from("assistant"),
                    content: String::from("hi"),
                },
            ]
        );
        assert_eq!(
            manifest.load_licenses(LocalBlobOpenOptions::default())?,
            vec![String::from("Apache-2.0")]
        );
        assert!(manifest.modified_at()?.elapsed().is_ok());
        Ok(())
    }

    #[test]
    fn ollama_manifest_surfaces_provenance_and_license_facts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let config_digest = write_blob(
            temp.path(),
            br#"{
                "model_format":"gguf",
                "model_family":"qwen2",
                "remote_host":"cloud.example",
                "remote_model":"team/qwen2-licensed",
                "base_name":"qwen2-base"
            }"#,
        )?;
        let model_digest = write_blob(temp.path(), b"GGUF")?;
        let license_digest = write_blob(temp.path(), b"Apache-2.0")?;

        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": config_digest,
                    "size": 1
                },
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 4
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.license",
                        "digest": license_digest,
                        "size": 10
                    }
                ]
            }),
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2")?;
        let config = manifest.load_config(LocalBlobOpenOptions::default())?;
        let provenance = manifest.provenance_facts(config.as_ref());
        let licenses = manifest.load_license_facts(LocalBlobOpenOptions::default())?;

        assert_eq!(provenance.kind, OllamaProvenanceKind::RemoteAlias);
        assert_eq!(
            provenance.canonical_name,
            "registry.ollama.ai/library/qwen2:latest"
        );
        assert_eq!(provenance.short_name, "qwen2:latest");
        assert_eq!(provenance.remote_host.as_deref(), Some("cloud.example"));
        assert_eq!(
            provenance.remote_model.as_deref(),
            Some("team/qwen2-licensed")
        );
        assert_eq!(provenance.base_model.as_deref(), Some("qwen2-base"));

        assert!(licenses.declared);
        assert_eq!(licenses.entries.len(), 1);
        assert_eq!(licenses.entries[0].text, "Apache-2.0");
        assert_eq!(
            licenses.entries[0].sha256,
            hex::encode(Sha256::digest(b"Apache-2.0"))
        );
        Ok(())
    }

    #[test]
    fn ollama_catalog_and_manifest_surface_explicit_migration_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let digest = hex::encode(Sha256::digest(b"model"));
        let blob_path = temp.path().join("blobs").join(format!("sha256-{digest}"));
        std::fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        std::fs::write(&blob_path, b"model")?;

        let manifest_path = temp
            .path()
            .join("manifests/registry.ollama.ai/library/qwen2/latest");
        std::fs::create_dir_all(manifest_path.parent().ok_or("missing parent")?)?;
        std::fs::write(
            &manifest_path,
            format!(
                r#"{{"schemaVersion":2,"mediaType":"application/vnd.docker.distribution.manifest.v2+json","layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{digest}","size":5}}]}}"#
            ),
        )?;

        let catalog = OllamaModelCatalog::new(temp.path());
        assert_eq!(
            catalog.surface(),
            OllamaCatalogSurface::OllamaCompatMigration
        );

        let manifest = catalog.resolve_model("qwen2")?;
        assert_eq!(
            manifest.catalog_surface(),
            OllamaCatalogSurface::OllamaCompatMigration
        );
        Ok(())
    }

    #[test]
    fn ollama_manifest_reports_adapter_policy_status() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let model_digest = write_blob(temp.path(), b"qwen2-gguf")?;
        let adapter_digest = write_blob(temp.path(), b"adapter-gguf")?;

        write_manifest(
            temp.path(),
            "registry.ollama.ai/library/qwen2-adapter/latest",
            serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": model_digest,
                        "size": 10
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.adapter",
                        "digest": adapter_digest,
                        "size": 12
                    }
                ]
            }),
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2-adapter")?;
        let status = manifest.adapter_policy_status();

        assert_eq!(manifest.adapter_layer_count(), 1);
        assert_eq!(manifest.adapter_layers().count(), 1);
        assert_eq!(
            status.policy,
            OllamaAdapterPolicy::RefuseManifestWithAdapters
        );
        assert_eq!(status.adapter_layer_count, 1);
        assert!(!status.supported);
        Ok(())
    }

    fn write_manifest(
        models_root: &Path,
        relpath: &str,
        json: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = serde_json::to_vec(&json)?;
        write_manifest_raw(models_root, relpath, bytes.as_slice())
    }

    fn write_manifest_raw(
        models_root: &Path,
        relpath: &str,
        bytes: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = models_root.join("manifests").join(relpath);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        file.write_all(bytes)?;
        Ok(())
    }

    fn write_blob(models_root: &Path, bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let path = models_root.join("blobs").join(digest.replace(':', "-"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        file.write_all(bytes)?;
        Ok(digest)
    }
}
