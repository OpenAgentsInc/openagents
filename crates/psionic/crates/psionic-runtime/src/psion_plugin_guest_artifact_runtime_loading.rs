use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    record_psion_plugin_guest_artifact_identity, reference_psion_plugin_guest_artifact_bytes,
    reference_psion_plugin_guest_artifact_manifest,
    refresh_psion_plugin_guest_artifact_manifest_digest, PsionPluginGuestArtifactFormat,
    PsionPluginGuestArtifactManifest, PsionPluginGuestArtifactManifestError,
};

pub const PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_guest_artifact_runtime_loading.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_REF: &str =
    "fixtures/psion/plugins/guest_artifact/psion_plugin_guest_artifact_runtime_loading_v1.json";

pub const GUEST_ARTIFACT_HOST_OWNED_MOUNT_POLICY_ID: &str =
    "psion.plugin_guest_artifact.host_owned_mount_policy.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactLoadStatus {
    Loaded,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactLoadRefusalCode {
    DigestMismatch,
    MalformedArtifact,
    UnsupportedFormat,
    ExportIdentityMismatch,
    CapabilityMountDenied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactLoadedArtifact {
    pub loaded_artifact_id: String,
    pub manifest_id: String,
    pub identity_digest: String,
    pub artifact_digest: String,
    pub packet_abi_version: String,
    pub guest_export_name: String,
    pub host_owned_mount_policy_id: String,
    pub host_owned_capability_mediation: bool,
    pub publication_blocked: bool,
    pub wasm_header_hex: String,
    pub loaded_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactLoadRefusal {
    pub refusal_code: PsionPluginGuestArtifactLoadRefusalCode,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactLoadCase {
    pub case_id: String,
    pub status: PsionPluginGuestArtifactLoadStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_artifact: Option<PsionPluginGuestArtifactLoadedArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionPluginGuestArtifactLoadRefusal>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactRuntimeLoadingBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub manifest_id: String,
    pub manifest_digest: String,
    pub success_case: PsionPluginGuestArtifactLoadCase,
    pub refusal_cases: Vec<PsionPluginGuestArtifactLoadCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionPluginGuestArtifactRuntimeLoadingBundle {
    pub fn validate(
        &self,
        manifest: &PsionPluginGuestArtifactManifest,
    ) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
        manifest
            .validate()
            .map_err(PsionPluginGuestArtifactRuntimeLoadingError::Manifest)?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_SCHEMA_VERSION,
            "psion_plugin_guest_artifact_runtime_loading.schema_version",
        )?;
        check_string_match(
            self.manifest_id.as_str(),
            manifest.manifest_id.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.manifest_id",
        )?;
        check_string_match(
            self.manifest_digest.as_str(),
            manifest.manifest_digest.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.manifest_digest",
        )?;
        if self.refusal_cases.is_empty() {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: String::from("psion_plugin_guest_artifact_runtime_loading.refusal_cases"),
                expected: String::from("at least one explicit refusal case"),
                actual: String::from("0"),
            });
        }
        self.success_case.validate_loaded(manifest)?;
        for (index, refusal_case) in self.refusal_cases.iter().enumerate() {
            refusal_case.validate_refusal(
                manifest,
                format!("psion_plugin_guest_artifact_runtime_loading.refusal_cases[{index}]")
                    .as_str(),
            )?;
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.claim_boundary",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.summary",
        )?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(
                PsionPluginGuestArtifactRuntimeLoadingError::DigestMismatch {
                    kind: String::from("psion_plugin_guest_artifact_runtime_loading"),
                },
            );
        }
        Ok(())
    }

    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
        write_json_file(self, output_path)
    }
}

impl PsionPluginGuestArtifactLoadCase {
    fn validate_loaded(
        &self,
        manifest: &PsionPluginGuestArtifactManifest,
    ) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
        if self.status != PsionPluginGuestArtifactLoadStatus::Loaded {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: String::from(
                    "psion_plugin_guest_artifact_runtime_loading.success_case.status",
                ),
                expected: String::from("loaded"),
                actual: format!("{:?}", self.status),
            });
        }
        let loaded_artifact = self.loaded_artifact.as_ref().ok_or_else(|| {
            PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: String::from(
                    "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact",
                ),
                expected: String::from("some"),
                actual: String::from("none"),
            }
        })?;
        if self.refusal.is_some() {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: String::from(
                    "psion_plugin_guest_artifact_runtime_loading.success_case.refusal",
                ),
                expected: String::from("none"),
                actual: String::from("some"),
            });
        }
        ensure_nonempty(
            loaded_artifact.loaded_artifact_id.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.loaded_artifact_id",
        )?;
        check_string_match(
            loaded_artifact.manifest_id.as_str(),
            manifest.manifest_id.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.manifest_id",
        )?;
        check_string_match(
            loaded_artifact.artifact_digest.as_str(),
            manifest.artifact_digest.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.artifact_digest",
        )?;
        check_string_match(
            loaded_artifact.packet_abi_version.as_str(),
            manifest.packet_abi_version.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.packet_abi_version",
        )?;
        check_string_match(
            loaded_artifact.guest_export_name.as_str(),
            manifest.guest_export_name.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.guest_export_name",
        )?;
        check_string_match(
            loaded_artifact.host_owned_mount_policy_id.as_str(),
            GUEST_ARTIFACT_HOST_OWNED_MOUNT_POLICY_ID,
            "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.host_owned_mount_policy_id",
        )?;
        if !loaded_artifact.host_owned_capability_mediation || !loaded_artifact.publication_blocked
        {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: String::from(
                    "psion_plugin_guest_artifact_runtime_loading.success_case.loaded_artifact.host_owned_posture",
                ),
                expected: String::from("host_owned_capability_mediation=true and publication_blocked=true"),
                actual: format!(
                    "host_owned_capability_mediation={} publication_blocked={}",
                    loaded_artifact.host_owned_capability_mediation,
                    loaded_artifact.publication_blocked
                ),
            });
        }
        if loaded_artifact.loaded_digest != stable_loaded_artifact_digest(loaded_artifact) {
            return Err(
                PsionPluginGuestArtifactRuntimeLoadingError::DigestMismatch {
                    kind: String::from("psion_plugin_guest_artifact_loaded_artifact"),
                },
            );
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_plugin_guest_artifact_runtime_loading.success_case.detail",
        )?;
        Ok(())
    }

    fn validate_refusal(
        &self,
        manifest: &PsionPluginGuestArtifactManifest,
        field: &str,
    ) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
        if self.status != PsionPluginGuestArtifactLoadStatus::Refused {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: format!("{field}.status"),
                expected: String::from("refused"),
                actual: format!("{:?}", self.status),
            });
        }
        if self.loaded_artifact.is_some() {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: format!("{field}.loaded_artifact"),
                expected: String::from("none"),
                actual: String::from("some"),
            });
        }
        let refusal = self.refusal.as_ref().ok_or_else(|| {
            PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: format!("{field}.refusal"),
                expected: String::from("some"),
                actual: String::from("none"),
            }
        })?;
        ensure_nonempty(
            refusal.detail.as_str(),
            format!("{field}.refusal.detail").as_str(),
        )?;
        ensure_nonempty(self.detail.as_str(), format!("{field}.detail").as_str())?;
        if matches!(
            refusal.refusal_code,
            PsionPluginGuestArtifactLoadRefusalCode::ExportIdentityMismatch
        ) && manifest.guest_export_name != "handle_packet"
        {
            return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
                field: format!("{field}.refusal.refusal_code"),
                expected: String::from("manifest export should still be canonical handle_packet"),
                actual: manifest.guest_export_name.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginGuestArtifactRuntimeLoadingError {
    #[error(transparent)]
    Manifest(#[from] PsionPluginGuestArtifactManifestError),
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
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

pub fn load_psion_plugin_guest_artifact(
    manifest: &PsionPluginGuestArtifactManifest,
    artifact_bytes: &[u8],
) -> Result<PsionPluginGuestArtifactLoadCase, PsionPluginGuestArtifactRuntimeLoadingError> {
    manifest
        .validate()
        .map_err(PsionPluginGuestArtifactRuntimeLoadingError::Manifest)?;
    if manifest.artifact_format != PsionPluginGuestArtifactFormat::WasmModulePacketV1 {
        return Ok(refusal_case(
            "unsupported_format",
            PsionPluginGuestArtifactLoadRefusalCode::UnsupportedFormat,
            "only the bounded WasmModulePacketV1 guest-artifact format is admitted by the current loader.",
        ));
    }
    if manifest.guest_export_name != "handle_packet" || manifest.packet_abi_version != "packet.v1" {
        return Ok(refusal_case(
            "export_identity_mismatch",
            PsionPluginGuestArtifactLoadRefusalCode::ExportIdentityMismatch,
            "guest-artifact loading fails closed unless the manifest stays on the canonical handle_packet + packet.v1 boundary.",
        ));
    }
    if !manifest.capability_namespace_ids.is_empty() {
        return Ok(refusal_case(
            "capability_mount_denied",
            PsionPluginGuestArtifactLoadRefusalCode::CapabilityMountDenied,
            "the bounded guest-artifact loader keeps capability mediation host-owned and refuses manifests that request mounted capability namespaces before the later mount-admission tranche lands.",
        ));
    }
    if sha256_hex(artifact_bytes) != manifest.artifact_digest {
        return Ok(refusal_case(
            "digest_mismatch",
            PsionPluginGuestArtifactLoadRefusalCode::DigestMismatch,
            "guest-artifact loading fails closed when artifact bytes do not match the manifest-bound digest.",
        ));
    }
    if artifact_bytes.len() < 8
        || &artifact_bytes[..4] != b"\0asm"
        || artifact_bytes[4..8] != [1, 0, 0, 0]
    {
        return Ok(refusal_case(
            "malformed_artifact",
            PsionPluginGuestArtifactLoadRefusalCode::MalformedArtifact,
            "guest-artifact loading requires the admitted Wasm magic and version header before any later runtime loading or invocation work may proceed.",
        ));
    }

    let identity = record_psion_plugin_guest_artifact_identity(manifest)
        .map_err(PsionPluginGuestArtifactRuntimeLoadingError::Manifest)?;
    let mut loaded_artifact = PsionPluginGuestArtifactLoadedArtifact {
        loaded_artifact_id: format!("loaded.{}", manifest.artifact_id),
        manifest_id: manifest.manifest_id.clone(),
        identity_digest: identity.identity_digest,
        artifact_digest: manifest.artifact_digest.clone(),
        packet_abi_version: manifest.packet_abi_version.clone(),
        guest_export_name: manifest.guest_export_name.clone(),
        host_owned_mount_policy_id: String::from(GUEST_ARTIFACT_HOST_OWNED_MOUNT_POLICY_ID),
        host_owned_capability_mediation: true,
        publication_blocked: true,
        wasm_header_hex: artifact_bytes[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        loaded_digest: String::new(),
    };
    loaded_artifact.loaded_digest = stable_loaded_artifact_digest(&loaded_artifact);
    Ok(PsionPluginGuestArtifactLoadCase {
        case_id: String::from("guest_artifact_reference_load_success"),
        status: PsionPluginGuestArtifactLoadStatus::Loaded,
        loaded_artifact: Some(loaded_artifact),
        refusal: None,
        detail: String::from(
            "the bounded guest-artifact loader admits one digest-matched Wasm packet artifact while keeping capability mediation host-owned and publication blocked.",
        ),
    })
}

#[must_use]
pub fn build_psion_plugin_guest_artifact_runtime_loading_bundle(
) -> PsionPluginGuestArtifactRuntimeLoadingBundle {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let reference_artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
    let success_case =
        load_psion_plugin_guest_artifact(&manifest, reference_artifact_bytes.as_slice())
            .expect("reference guest artifact should load");
    let mut malformed_bytes = reference_artifact_bytes.clone();
    malformed_bytes[0] = 0xff;
    let refusal_cases = vec![
        load_psion_plugin_guest_artifact(&manifest, b"definitely-not-the-declared-wasm")
            .expect("digest mismatch should refuse"),
        {
            let mut malformed_manifest = manifest.clone();
            malformed_manifest.artifact_digest = sha256_hex(malformed_bytes.as_slice());
            refresh_psion_plugin_guest_artifact_manifest_digest(&mut malformed_manifest);
            load_psion_plugin_guest_artifact(&malformed_manifest, malformed_bytes.as_slice())
                .expect("malformed artifact should refuse")
        },
        {
            let mut capability_mount = manifest.clone();
            capability_mount.capability_namespace_ids =
                vec![String::from("capability.http.read_only.v1")];
            refresh_psion_plugin_guest_artifact_manifest_digest(&mut capability_mount);
            load_psion_plugin_guest_artifact(&capability_mount, reference_artifact_bytes.as_slice())
                .expect("capability mount request should refuse")
        },
    ];
    let mut bundle = PsionPluginGuestArtifactRuntimeLoadingBundle {
        schema_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_SCHEMA_VERSION),
        bundle_id: String::from("psion_plugin_guest_artifact_runtime_loading"),
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: manifest.manifest_digest.clone(),
        success_case,
        refusal_cases,
        claim_boundary: String::from(
            "this bundle closes only the bounded guest-artifact runtime-loading path. It proves manifest-bound byte admission, digest verification, minimal Wasm header validation, host-owned capability mediation, and explicit typed load refusal while keeping invocation, bridge exposure, controller admission, publication, and arbitrary binary loading blocked.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Guest-artifact runtime loading bundle covers refusal_cases={} and keeps host_owned_capability_mediation=true with publication blocked.",
        bundle.refusal_cases.len()
    );
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle
}

#[must_use]
pub fn psion_plugin_guest_artifact_runtime_loading_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_REF)
}

pub fn write_psion_plugin_guest_artifact_runtime_loading_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginGuestArtifactRuntimeLoadingBundle, PsionPluginGuestArtifactRuntimeLoadingError>
{
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let bundle = build_psion_plugin_guest_artifact_runtime_loading_bundle();
    bundle.validate(&manifest)?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

fn refusal_case(
    case_id: &str,
    refusal_code: PsionPluginGuestArtifactLoadRefusalCode,
    detail: &str,
) -> PsionPluginGuestArtifactLoadCase {
    PsionPluginGuestArtifactLoadCase {
        case_id: String::from(case_id),
        status: PsionPluginGuestArtifactLoadStatus::Refused,
        loaded_artifact: None,
        refusal: Some(PsionPluginGuestArtifactLoadRefusal {
            refusal_code,
            detail: String::from(detail),
        }),
        detail: String::from(detail),
    }
}

fn stable_loaded_artifact_digest(
    loaded_artifact: &PsionPluginGuestArtifactLoadedArtifact,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(loaded_artifact.loaded_artifact_id.as_bytes());
    hasher.update(loaded_artifact.manifest_id.as_bytes());
    hasher.update(loaded_artifact.identity_digest.as_bytes());
    hasher.update(loaded_artifact.artifact_digest.as_bytes());
    hasher.update(loaded_artifact.packet_abi_version.as_bytes());
    hasher.update(loaded_artifact.guest_export_name.as_bytes());
    hasher.update(loaded_artifact.host_owned_mount_policy_id.as_bytes());
    hasher.update([loaded_artifact.host_owned_capability_mediation as u8]);
    hasher.update([loaded_artifact.publication_blocked as u8]);
    hasher.update(loaded_artifact.wasm_header_hex.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_bundle_digest(bundle: &PsionPluginGuestArtifactRuntimeLoadingBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(bundle.manifest_id.as_bytes());
    hasher.update(bundle.manifest_digest.as_bytes());
    update_case_digest(&mut hasher, &bundle.success_case);
    for refusal_case in &bundle.refusal_cases {
        update_case_digest(&mut hasher, refusal_case);
    }
    hasher.update(bundle.claim_boundary.as_bytes());
    hasher.update(bundle.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn update_case_digest(hasher: &mut Sha256, case: &PsionPluginGuestArtifactLoadCase) {
    hasher.update(case.case_id.as_bytes());
    hasher.update(format!("{:?}", case.status).as_bytes());
    if let Some(loaded_artifact) = &case.loaded_artifact {
        hasher.update(stable_loaded_artifact_digest(loaded_artifact).as_bytes());
    }
    if let Some(refusal) = &case.refusal {
        hasher.update(format!("{:?}", refusal.refusal_code).as_bytes());
        hasher.update(refusal.detail.as_bytes());
    }
    hasher.update(case.detail.as_bytes());
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
    if value.trim().is_empty() {
        return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
            field: String::from(field),
            expected: String::from("non-empty"),
            actual: String::from("empty"),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
    if actual != expected {
        return Err(PsionPluginGuestArtifactRuntimeLoadingError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
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
) -> Result<(), PsionPluginGuestArtifactRuntimeLoadingError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionPluginGuestArtifactRuntimeLoadingError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(output_path, bytes).map_err(|error| {
        PsionPluginGuestArtifactRuntimeLoadingError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, PsionPluginGuestArtifactRuntimeLoadingError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| PsionPluginGuestArtifactRuntimeLoadingError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionPluginGuestArtifactRuntimeLoadingError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        build_psion_plugin_guest_artifact_runtime_loading_bundle, load_psion_plugin_guest_artifact,
        psion_plugin_guest_artifact_runtime_loading_path, read_json,
        write_psion_plugin_guest_artifact_runtime_loading_bundle,
        PsionPluginGuestArtifactLoadRefusalCode, PsionPluginGuestArtifactLoadStatus,
        PsionPluginGuestArtifactRuntimeLoadingBundle,
        PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_REF,
    };
    use crate::{
        reference_psion_plugin_guest_artifact_bytes, reference_psion_plugin_guest_artifact_manifest,
    };

    #[test]
    fn guest_artifact_runtime_loading_bundle_validates() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let bundle = build_psion_plugin_guest_artifact_runtime_loading_bundle();
        bundle.validate(&manifest)?;
        Ok(())
    }

    #[test]
    fn guest_artifact_loader_refuses_digest_mismatch() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let refusal = load_psion_plugin_guest_artifact(&manifest, b"not-the-declared-wasm")?;
        assert_eq!(refusal.status, PsionPluginGuestArtifactLoadStatus::Refused);
        assert_eq!(
            refusal.refusal.expect("refusal").refusal_code,
            PsionPluginGuestArtifactLoadRefusalCode::DigestMismatch
        );
        Ok(())
    }

    #[test]
    fn guest_artifact_loader_loads_reference_wasm() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let loaded = load_psion_plugin_guest_artifact(
            &manifest,
            reference_psion_plugin_guest_artifact_bytes().as_slice(),
        )?;
        assert_eq!(loaded.status, PsionPluginGuestArtifactLoadStatus::Loaded);
        assert!(loaded.loaded_artifact.is_some());
        Ok(())
    }

    #[test]
    fn committed_guest_artifact_runtime_loading_fixture_validates() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let bundle: PsionPluginGuestArtifactRuntimeLoadingBundle =
            read_json(psion_plugin_guest_artifact_runtime_loading_path())?;
        bundle.validate(&manifest)?;
        Ok(())
    }

    #[test]
    fn write_guest_artifact_runtime_loading_fixture_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        let tempdir = tempfile::tempdir()?;
        let output_path = tempdir
            .path()
            .join("psion_plugin_guest_artifact_runtime_loading_v1.json");
        let written = write_psion_plugin_guest_artifact_runtime_loading_bundle(&output_path)?;
        let persisted: PsionPluginGuestArtifactRuntimeLoadingBundle = read_json(&output_path)?;
        assert_eq!(written, persisted);
        Ok(())
    }

    #[test]
    fn runtime_loading_fixture_ref_matches_committed_path() {
        assert!(psion_plugin_guest_artifact_runtime_loading_path()
            .display()
            .to_string()
            .ends_with(PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_LOADING_REF));
    }
}
