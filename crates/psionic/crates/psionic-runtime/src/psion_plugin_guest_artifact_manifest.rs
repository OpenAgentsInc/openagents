use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};

pub const PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_guest_artifact_manifest.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_guest_artifact_identity.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_REF: &str =
    "fixtures/psion/plugins/guest_artifact/psion_plugin_guest_artifact_manifest_v1.json";
pub const PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_REF: &str =
    "fixtures/psion/plugins/guest_artifact/psion_plugin_guest_artifact_identity_v1.json";

pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_ABI_VERSION: &str = "packet.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_GUEST_EXPORT_NAME: &str = "handle_packet";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_REPLAY_CLASS_ID: &str =
    "guest_artifact_digest_replay_only.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_ID: &str = "plugin.example.echo_guest";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION: &str = "v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_ARTIFACT_ID: &str =
    "artifact.plugin.example.echo_guest.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID: &str =
    "plugin.example.echo_guest.input.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_OUTPUT_SCHEMA_ID: &str =
    "plugin.example.echo_guest.output.v1";
const REFERENCE_DECLARED_ARTIFACT_REF: &str =
    "artifacts/operator_internal/plugin_example_echo_guest.wasm";
const REFERENCE_GUEST_MEMORY_PAGES: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactFormat {
    WasmModulePacketV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactTrustTier {
    OperatorReviewedDigestBoundInternalOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactPublicationPosture {
    OperatorInternalOnlyPublicationBlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactSourceKind {
    OperatorProvidedWasmBundle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactProvenance {
    pub source_kind: PsionPluginGuestArtifactSourceKind,
    pub source_bundle_digest: String,
    pub build_recipe_digest: String,
    pub declared_artifact_ref: String,
    pub detail: String,
}

impl PsionPluginGuestArtifactProvenance {
    fn validate(&self) -> Result<(), PsionPluginGuestArtifactManifestError> {
        ensure_digest(
            self.source_bundle_digest.as_str(),
            "psion_plugin_guest_artifact_manifest.provenance.source_bundle_digest",
        )?;
        ensure_digest(
            self.build_recipe_digest.as_str(),
            "psion_plugin_guest_artifact_manifest.provenance.build_recipe_digest",
        )?;
        ensure_nonempty(
            self.declared_artifact_ref.as_str(),
            "psion_plugin_guest_artifact_manifest.provenance.declared_artifact_ref",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_plugin_guest_artifact_manifest.provenance.detail",
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactEvidenceSettings {
    pub input_digest_required: bool,
    pub output_digest_required: bool,
    pub receipt_emission_required: bool,
}

impl PsionPluginGuestArtifactEvidenceSettings {
    fn validate(&self) -> Result<(), PsionPluginGuestArtifactManifestError> {
        ensure_bool_true(
            self.input_digest_required,
            "psion_plugin_guest_artifact_manifest.evidence_settings.input_digest_required",
        )?;
        ensure_bool_true(
            self.output_digest_required,
            "psion_plugin_guest_artifact_manifest.evidence_settings.output_digest_required",
        )?;
        ensure_bool_true(
            self.receipt_emission_required,
            "psion_plugin_guest_artifact_manifest.evidence_settings.receipt_emission_required",
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactManifest {
    pub schema_version: String,
    pub manifest_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub artifact_id: String,
    pub artifact_format: PsionPluginGuestArtifactFormat,
    pub artifact_digest: String,
    pub artifact_byte_len: u64,
    pub packet_abi_version: String,
    pub guest_export_name: String,
    pub input_schema_id: String,
    pub success_output_schema_id: String,
    pub refusal_schema_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_namespace_ids: Vec<String>,
    pub replay_class_id: String,
    pub trust_tier: PsionPluginGuestArtifactTrustTier,
    pub publication_posture: PsionPluginGuestArtifactPublicationPosture,
    pub provenance: PsionPluginGuestArtifactProvenance,
    pub evidence_settings: PsionPluginGuestArtifactEvidenceSettings,
    pub detail: String,
    pub manifest_digest: String,
}

impl PsionPluginGuestArtifactManifest {
    pub fn validate(&self) -> Result<(), PsionPluginGuestArtifactManifestError> {
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_SCHEMA_VERSION,
            "psion_plugin_guest_artifact_manifest.schema_version",
        )?;
        ensure_nonempty(
            self.manifest_id.as_str(),
            "psion_plugin_guest_artifact_manifest.manifest_id",
        )?;
        ensure_plugin_id(
            self.plugin_id.as_str(),
            "psion_plugin_guest_artifact_manifest.plugin_id",
        )?;
        ensure_version_id(
            self.plugin_version.as_str(),
            "psion_plugin_guest_artifact_manifest.plugin_version",
        )?;
        ensure_prefixed_id(
            self.artifact_id.as_str(),
            "artifact.",
            "psion_plugin_guest_artifact_manifest.artifact_id",
        )?;
        ensure_digest(
            self.artifact_digest.as_str(),
            "psion_plugin_guest_artifact_manifest.artifact_digest",
        )?;
        if self.artifact_byte_len == 0 {
            return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
                field: String::from("psion_plugin_guest_artifact_manifest.artifact_byte_len"),
                expected: String::from("> 0"),
                actual: self.artifact_byte_len.to_string(),
            });
        }
        check_string_match(
            self.packet_abi_version.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_ABI_VERSION,
            "psion_plugin_guest_artifact_manifest.packet_abi_version",
        )?;
        check_string_match(
            self.guest_export_name.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_GUEST_EXPORT_NAME,
            "psion_plugin_guest_artifact_manifest.guest_export_name",
        )?;
        ensure_prefixed_id(
            self.input_schema_id.as_str(),
            format!("{plugin_id}.input", plugin_id = self.plugin_id).as_str(),
            "psion_plugin_guest_artifact_manifest.input_schema_id",
        )?;
        ensure_prefixed_id(
            self.success_output_schema_id.as_str(),
            format!("{plugin_id}.output", plugin_id = self.plugin_id).as_str(),
            "psion_plugin_guest_artifact_manifest.success_output_schema_id",
        )?;
        reject_duplicate_strings(
            self.refusal_schema_ids.as_slice(),
            "psion_plugin_guest_artifact_manifest.refusal_schema_ids",
        )?;
        if self.refusal_schema_ids.is_empty() {
            return Err(PsionPluginGuestArtifactManifestError::MissingField {
                field: String::from("psion_plugin_guest_artifact_manifest.refusal_schema_ids"),
            });
        }
        for (index, refusal_schema_id) in self.refusal_schema_ids.iter().enumerate() {
            ensure_prefixed_id(
                refusal_schema_id.as_str(),
                "plugin.refusal.",
                format!("psion_plugin_guest_artifact_manifest.refusal_schema_ids[{index}]")
                    .as_str(),
            )?;
        }
        reject_duplicate_strings(
            self.capability_namespace_ids.as_slice(),
            "psion_plugin_guest_artifact_manifest.capability_namespace_ids",
        )?;
        for (index, capability_namespace_id) in self.capability_namespace_ids.iter().enumerate() {
            ensure_prefixed_id(
                capability_namespace_id.as_str(),
                "capability.",
                format!("psion_plugin_guest_artifact_manifest.capability_namespace_ids[{index}]")
                    .as_str(),
            )?;
        }
        check_string_match(
            self.replay_class_id.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_REPLAY_CLASS_ID,
            "psion_plugin_guest_artifact_manifest.replay_class_id",
        )?;
        self.provenance.validate()?;
        self.evidence_settings.validate()?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_plugin_guest_artifact_manifest.detail",
        )?;
        if self.manifest_digest != stable_manifest_digest(self) {
            return Err(PsionPluginGuestArtifactManifestError::DigestMismatch {
                kind: String::from("psion_plugin_guest_artifact_manifest"),
            });
        }
        Ok(())
    }

    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginGuestArtifactManifestError> {
        write_json_file(self, output_path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactIdentity {
    pub schema_version: String,
    pub identity_id: String,
    pub manifest_id: String,
    pub manifest_digest: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub artifact_id: String,
    pub artifact_format: PsionPluginGuestArtifactFormat,
    pub artifact_digest: String,
    pub packet_abi_version: String,
    pub guest_export_name: String,
    pub trust_tier: PsionPluginGuestArtifactTrustTier,
    pub publication_posture: PsionPluginGuestArtifactPublicationPosture,
    pub identity_digest: String,
}

impl PsionPluginGuestArtifactIdentity {
    pub fn validate_against_manifest(
        &self,
        manifest: &PsionPluginGuestArtifactManifest,
    ) -> Result<(), PsionPluginGuestArtifactManifestError> {
        manifest.validate()?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_SCHEMA_VERSION,
            "psion_plugin_guest_artifact_identity.schema_version",
        )?;
        check_string_match(
            self.identity_id.as_str(),
            format!(
                "psion_plugin_guest_artifact_identity.{}.{}",
                manifest.plugin_id, manifest.plugin_version
            )
            .as_str(),
            "psion_plugin_guest_artifact_identity.identity_id",
        )?;
        check_string_match(
            self.manifest_id.as_str(),
            manifest.manifest_id.as_str(),
            "psion_plugin_guest_artifact_identity.manifest_id",
        )?;
        check_string_match(
            self.manifest_digest.as_str(),
            manifest.manifest_digest.as_str(),
            "psion_plugin_guest_artifact_identity.manifest_digest",
        )?;
        check_string_match(
            self.plugin_id.as_str(),
            manifest.plugin_id.as_str(),
            "psion_plugin_guest_artifact_identity.plugin_id",
        )?;
        check_string_match(
            self.plugin_version.as_str(),
            manifest.plugin_version.as_str(),
            "psion_plugin_guest_artifact_identity.plugin_version",
        )?;
        check_string_match(
            self.artifact_id.as_str(),
            manifest.artifact_id.as_str(),
            "psion_plugin_guest_artifact_identity.artifact_id",
        )?;
        if self.artifact_format != manifest.artifact_format {
            return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
                field: String::from("psion_plugin_guest_artifact_identity.artifact_format"),
                expected: format!("{:?}", manifest.artifact_format),
                actual: format!("{:?}", self.artifact_format),
            });
        }
        check_string_match(
            self.artifact_digest.as_str(),
            manifest.artifact_digest.as_str(),
            "psion_plugin_guest_artifact_identity.artifact_digest",
        )?;
        check_string_match(
            self.packet_abi_version.as_str(),
            manifest.packet_abi_version.as_str(),
            "psion_plugin_guest_artifact_identity.packet_abi_version",
        )?;
        check_string_match(
            self.guest_export_name.as_str(),
            manifest.guest_export_name.as_str(),
            "psion_plugin_guest_artifact_identity.guest_export_name",
        )?;
        if self.trust_tier != manifest.trust_tier {
            return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
                field: String::from("psion_plugin_guest_artifact_identity.trust_tier"),
                expected: format!("{:?}", manifest.trust_tier),
                actual: format!("{:?}", self.trust_tier),
            });
        }
        if self.publication_posture != manifest.publication_posture {
            return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
                field: String::from("psion_plugin_guest_artifact_identity.publication_posture"),
                expected: format!("{:?}", manifest.publication_posture),
                actual: format!("{:?}", self.publication_posture),
            });
        }
        if self.identity_digest != stable_identity_digest(self) {
            return Err(PsionPluginGuestArtifactManifestError::DigestMismatch {
                kind: String::from("psion_plugin_guest_artifact_identity"),
            });
        }
        Ok(())
    }

    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginGuestArtifactManifestError> {
        write_json_file(self, output_path)
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginGuestArtifactManifestError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("repeated value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("digest drifted for `{kind}`")]
    DigestMismatch { kind: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn reference_psion_plugin_guest_artifact_manifest() -> PsionPluginGuestArtifactManifest {
    let artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
    let artifact_digest = sha256_hex(artifact_bytes.as_slice());
    let mut manifest = PsionPluginGuestArtifactManifest {
        schema_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_SCHEMA_VERSION),
        manifest_id: String::from("psion_plugin_guest_artifact_manifest_reference"),
        plugin_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_ID),
        plugin_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION),
        artifact_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_ARTIFACT_ID),
        artifact_format: PsionPluginGuestArtifactFormat::WasmModulePacketV1,
        artifact_digest,
        artifact_byte_len: artifact_bytes.len() as u64,
        packet_abi_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_ABI_VERSION),
        guest_export_name: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_GUEST_EXPORT_NAME),
        input_schema_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID),
        success_output_schema_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_OUTPUT_SCHEMA_ID),
        refusal_schema_ids: vec![
            String::from("plugin.refusal.schema_invalid.v1"),
            String::from("plugin.refusal.packet_too_large.v1"),
            String::from("plugin.refusal.runtime_resource_limit.v1"),
            String::from("plugin.refusal.runtime_unavailable.v1"),
        ],
        capability_namespace_ids: vec![],
        replay_class_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_REPLAY_CLASS_ID),
        trust_tier: PsionPluginGuestArtifactTrustTier::OperatorReviewedDigestBoundInternalOnly,
        publication_posture:
            PsionPluginGuestArtifactPublicationPosture::OperatorInternalOnlyPublicationBlocked,
        provenance: PsionPluginGuestArtifactProvenance {
            source_kind: PsionPluginGuestArtifactSourceKind::OperatorProvidedWasmBundle,
            source_bundle_digest: sha256_hex(b"psion_plugin_guest_artifact_reference_source_bundle_v1"),
            build_recipe_digest: sha256_hex(b"psion_plugin_guest_artifact_reference_build_recipe_v1"),
            declared_artifact_ref: String::from(REFERENCE_DECLARED_ARTIFACT_REF),
            detail: String::from(
                "Reference guest-artifact provenance stays digest-bound to one operator-provided source bundle and one explicit build recipe without claiming that the artifact is already runnable.",
            ),
        },
        evidence_settings: PsionPluginGuestArtifactEvidenceSettings {
            input_digest_required: true,
            output_digest_required: true,
            receipt_emission_required: true,
        },
        detail: String::from(
            "This reference manifest freezes the minimum digest-bound guest-artifact contract for one later user-provided Wasm lane while keeping bridge admission, controller admission, and publication support closed until the later guest-artifact issues land.",
        ),
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = stable_manifest_digest(&manifest);
    manifest
}

#[must_use]
pub fn reference_psion_plugin_guest_artifact_bytes() -> Vec<u8> {
    build_reference_psion_plugin_guest_artifact_wasm()
}

pub fn refresh_psion_plugin_guest_artifact_manifest_digest(
    manifest: &mut PsionPluginGuestArtifactManifest,
) {
    manifest.manifest_digest = stable_manifest_digest(manifest);
}

pub fn record_psion_plugin_guest_artifact_identity(
    manifest: &PsionPluginGuestArtifactManifest,
) -> Result<PsionPluginGuestArtifactIdentity, PsionPluginGuestArtifactManifestError> {
    manifest.validate()?;
    let mut identity = PsionPluginGuestArtifactIdentity {
        schema_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_SCHEMA_VERSION),
        identity_id: format!(
            "psion_plugin_guest_artifact_identity.{}.{}",
            manifest.plugin_id, manifest.plugin_version
        ),
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: manifest.manifest_digest.clone(),
        plugin_id: manifest.plugin_id.clone(),
        plugin_version: manifest.plugin_version.clone(),
        artifact_id: manifest.artifact_id.clone(),
        artifact_format: manifest.artifact_format,
        artifact_digest: manifest.artifact_digest.clone(),
        packet_abi_version: manifest.packet_abi_version.clone(),
        guest_export_name: manifest.guest_export_name.clone(),
        trust_tier: manifest.trust_tier,
        publication_posture: manifest.publication_posture,
        identity_digest: String::new(),
    };
    identity.identity_digest = stable_identity_digest(&identity);
    identity.validate_against_manifest(manifest)?;
    Ok(identity)
}

#[must_use]
pub fn psion_plugin_guest_artifact_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_REF)
}

#[must_use]
pub fn psion_plugin_guest_artifact_identity_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_REF)
}

pub fn write_reference_psion_plugin_guest_artifact_manifest(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginGuestArtifactManifest, PsionPluginGuestArtifactManifestError> {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    manifest.validate()?;
    manifest.write_to_path(output_path)?;
    Ok(manifest)
}

pub fn write_reference_psion_plugin_guest_artifact_identity(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginGuestArtifactIdentity, PsionPluginGuestArtifactManifestError> {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let identity = record_psion_plugin_guest_artifact_identity(&manifest)?;
    identity.write_to_path(output_path)?;
    Ok(identity)
}

fn stable_manifest_digest(manifest: &PsionPluginGuestArtifactManifest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(manifest.schema_version.as_bytes());
    hasher.update(manifest.manifest_id.as_bytes());
    hasher.update(manifest.plugin_id.as_bytes());
    hasher.update(manifest.plugin_version.as_bytes());
    hasher.update(manifest.artifact_id.as_bytes());
    hasher.update(format!("{:?}", manifest.artifact_format).as_bytes());
    hasher.update(manifest.artifact_digest.as_bytes());
    hasher.update(manifest.artifact_byte_len.to_le_bytes());
    hasher.update(manifest.packet_abi_version.as_bytes());
    hasher.update(manifest.guest_export_name.as_bytes());
    hasher.update(manifest.input_schema_id.as_bytes());
    hasher.update(manifest.success_output_schema_id.as_bytes());
    for refusal_schema_id in &manifest.refusal_schema_ids {
        hasher.update(refusal_schema_id.as_bytes());
    }
    for capability_namespace_id in &manifest.capability_namespace_ids {
        hasher.update(capability_namespace_id.as_bytes());
    }
    hasher.update(manifest.replay_class_id.as_bytes());
    hasher.update(format!("{:?}", manifest.trust_tier).as_bytes());
    hasher.update(format!("{:?}", manifest.publication_posture).as_bytes());
    hasher.update(format!("{:?}", manifest.provenance.source_kind).as_bytes());
    hasher.update(manifest.provenance.source_bundle_digest.as_bytes());
    hasher.update(manifest.provenance.build_recipe_digest.as_bytes());
    hasher.update(manifest.provenance.declared_artifact_ref.as_bytes());
    hasher.update(manifest.provenance.detail.as_bytes());
    hasher.update([manifest.evidence_settings.input_digest_required as u8]);
    hasher.update([manifest.evidence_settings.output_digest_required as u8]);
    hasher.update([manifest.evidence_settings.receipt_emission_required as u8]);
    hasher.update(manifest.detail.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn build_reference_psion_plugin_guest_artifact_wasm() -> Vec<u8> {
    let mut module = Module::new();

    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32, ValType::I32], [ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: REFERENCE_GUEST_MEMORY_PAGES,
        maximum: Some(REFERENCE_GUEST_MEMORY_PAGES),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    module.section(&memories);

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_GUEST_EXPORT_NAME, ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new(Vec::<(u32, ValType)>::new());
    function.instruction(&Instruction::LocalGet(2));
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::LocalGet(1));
    function.instruction(&Instruction::MemoryCopy {
        dst_mem: 0,
        src_mem: 0,
    });
    function.instruction(&Instruction::LocalGet(1));
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);

    module.finish()
}

fn stable_identity_digest(identity: &PsionPluginGuestArtifactIdentity) -> String {
    let mut hasher = Sha256::new();
    hasher.update(identity.schema_version.as_bytes());
    hasher.update(identity.identity_id.as_bytes());
    hasher.update(identity.manifest_id.as_bytes());
    hasher.update(identity.manifest_digest.as_bytes());
    hasher.update(identity.plugin_id.as_bytes());
    hasher.update(identity.plugin_version.as_bytes());
    hasher.update(identity.artifact_id.as_bytes());
    hasher.update(format!("{:?}", identity.artifact_format).as_bytes());
    hasher.update(identity.artifact_digest.as_bytes());
    hasher.update(identity.packet_abi_version.as_bytes());
    hasher.update(identity.guest_export_name.as_bytes());
    hasher.update(format!("{:?}", identity.trust_tier).as_bytes());
    hasher.update(format!("{:?}", identity.publication_posture).as_bytes());
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginGuestArtifactManifestError> {
    if value.trim().is_empty() {
        return Err(PsionPluginGuestArtifactManifestError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactManifestError> {
    if actual != expected {
        return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_prefixed_id(
    value: &str,
    prefix: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactManifestError> {
    ensure_nonempty(value, field)?;
    if !value.starts_with(prefix) {
        return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
            field: String::from(field),
            expected: format!("prefix `{prefix}`"),
            actual: String::from(value),
        });
    }
    Ok(())
}

fn ensure_plugin_id(value: &str, field: &str) -> Result<(), PsionPluginGuestArtifactManifestError> {
    ensure_prefixed_id(value, "plugin.", field)?;
    if value.chars().any(char::is_whitespace) {
        return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
            field: String::from(field),
            expected: String::from("no whitespace"),
            actual: String::from(value),
        });
    }
    Ok(())
}

fn ensure_version_id(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactManifestError> {
    ensure_prefixed_id(value, "v", field)
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), PsionPluginGuestArtifactManifestError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(PsionPluginGuestArtifactManifestError::DuplicateValue {
                field: String::from(field),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn ensure_bool_true(value: bool, field: &str) -> Result<(), PsionPluginGuestArtifactManifestError> {
    if !value {
        return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
            field: String::from(field),
            expected: String::from("true"),
            actual: String::from("false"),
        });
    }
    Ok(())
}

fn ensure_digest(value: &str, field: &str) -> Result<(), PsionPluginGuestArtifactManifestError> {
    ensure_nonempty(value, field)?;
    if value.len() != 64
        || !value
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(PsionPluginGuestArtifactManifestError::FieldMismatch {
            field: String::from(field),
            expected: String::from("64 lowercase hexadecimal characters"),
            actual: String::from(value),
        });
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn write_json_file<T: Serialize>(
    value: &T,
    output_path: impl AsRef<Path>,
) -> Result<(), PsionPluginGuestArtifactManifestError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionPluginGuestArtifactManifestError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(output_path, bytes).map_err(|error| PsionPluginGuestArtifactManifestError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, PsionPluginGuestArtifactManifestError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| PsionPluginGuestArtifactManifestError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionPluginGuestArtifactManifestError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        psion_plugin_guest_artifact_identity_path, psion_plugin_guest_artifact_manifest_path,
        read_json, record_psion_plugin_guest_artifact_identity,
        reference_psion_plugin_guest_artifact_manifest,
        write_reference_psion_plugin_guest_artifact_identity,
        write_reference_psion_plugin_guest_artifact_manifest, PsionPluginGuestArtifactIdentity,
        PsionPluginGuestArtifactManifest, PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_REF,
        PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_REF,
    };

    #[test]
    fn reference_guest_artifact_manifest_validates() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        manifest.validate()?;
        Ok(())
    }

    #[test]
    fn reference_guest_artifact_identity_matches_manifest() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let identity = record_psion_plugin_guest_artifact_identity(&manifest)?;
        identity.validate_against_manifest(&manifest)?;
        Ok(())
    }

    #[test]
    fn guest_artifact_manifest_rejects_missing_digest() {
        let mut manifest = reference_psion_plugin_guest_artifact_manifest();
        manifest.artifact_digest.clear();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn guest_artifact_manifest_rejects_missing_receipt_evidence() {
        let mut manifest = reference_psion_plugin_guest_artifact_manifest();
        manifest.evidence_settings.receipt_emission_required = false;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn committed_guest_artifact_contract_fixtures_validate() -> Result<(), Box<dyn Error>> {
        let manifest: PsionPluginGuestArtifactManifest =
            read_json(psion_plugin_guest_artifact_manifest_path())?;
        let identity: PsionPluginGuestArtifactIdentity =
            read_json(psion_plugin_guest_artifact_identity_path())?;
        manifest.validate()?;
        identity.validate_against_manifest(&manifest)?;
        Ok(())
    }

    #[test]
    fn write_reference_guest_artifact_contract_fixtures_persist_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let manifest_path = tempdir
            .path()
            .join("psion_plugin_guest_artifact_manifest_v1.json");
        let identity_path = tempdir
            .path()
            .join("psion_plugin_guest_artifact_identity_v1.json");
        let written_manifest =
            write_reference_psion_plugin_guest_artifact_manifest(&manifest_path)?;
        let written_identity =
            write_reference_psion_plugin_guest_artifact_identity(&identity_path)?;
        let persisted_manifest: PsionPluginGuestArtifactManifest = read_json(&manifest_path)?;
        let persisted_identity: PsionPluginGuestArtifactIdentity = read_json(&identity_path)?;
        assert_eq!(written_manifest, persisted_manifest);
        assert_eq!(written_identity, persisted_identity);
        Ok(())
    }

    #[test]
    fn fixture_refs_match_committed_paths() {
        assert!(psion_plugin_guest_artifact_manifest_path()
            .display()
            .to_string()
            .ends_with(PSION_PLUGIN_GUEST_ARTIFACT_MANIFEST_REF));
        assert!(psion_plugin_guest_artifact_identity_path()
            .display()
            .to_string()
            .ends_with(PSION_PLUGIN_GUEST_ARTIFACT_IDENTITY_REF));
    }
}
