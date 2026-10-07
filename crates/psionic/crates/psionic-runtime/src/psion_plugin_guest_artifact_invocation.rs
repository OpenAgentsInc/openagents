use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wasmi::{Engine, Linker, Module, Store};

use crate::{
    load_psion_plugin_guest_artifact, reference_psion_plugin_guest_artifact_bytes,
    reference_psion_plugin_guest_artifact_manifest, PsionPluginGuestArtifactLoadStatus,
    PsionPluginGuestArtifactManifest, PsionPluginGuestArtifactManifestError,
    PsionPluginGuestArtifactRuntimeLoadingError, StarterPluginInvocationReceipt,
    StarterPluginInvocationStatus, StarterPluginProjectedToolResultEnvelope, StarterPluginRefusal,
    StarterPluginToolProjection, PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_ID,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION,
};

pub const PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_guest_artifact_invocation.v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF: &str =
    "fixtures/psion/plugins/guest_artifact/psion_plugin_guest_artifact_invocation_v1.json";
pub const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME: &str = "plugin_example_echo_guest";

const PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_LIMIT_BYTES: usize = 4096;
const PSION_PLUGIN_GUEST_ARTIFACT_INPUT_PTR: usize = 0;
const PSION_PLUGIN_GUEST_ARTIFACT_OUTPUT_PTR: usize = 65_536;
const PSION_PLUGIN_GUEST_ARTIFACT_MOUNT_ENVELOPE_ID: &str =
    "psion.plugin_guest_artifact.mount_envelope.reference.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactEchoGuestPacket {
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactInvocationCaseStatus {
    ExactSuccess,
    TypedRefusal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginGuestArtifactInvocationRefusalCode {
    SchemaInvalid,
    PacketTooLarge,
    LoadRefusal,
    RuntimeUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactInvocationCase {
    pub case_id: String,
    pub status: PsionPluginGuestArtifactInvocationCaseStatus,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub replay_class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_code: Option<PsionPluginGuestArtifactInvocationRefusalCode>,
    pub projected_result: StarterPluginProjectedToolResultEnvelope,
    pub receipt_binding_preserved: bool,
    pub typed_refusal_preserved: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginGuestArtifactInvocationBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub manifest_id: String,
    pub manifest_digest: String,
    pub tool_projection: StarterPluginToolProjection,
    pub success_case: PsionPluginGuestArtifactInvocationCase,
    pub refusal_cases: Vec<PsionPluginGuestArtifactInvocationCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl PsionPluginGuestArtifactInvocationBundle {
    pub fn validate(
        &self,
        manifest: &PsionPluginGuestArtifactManifest,
    ) -> Result<(), PsionPluginGuestArtifactInvocationError> {
        manifest.validate()?;
        check_string_match(
            self.schema_version.as_str(),
            PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_SCHEMA_VERSION,
            "psion_plugin_guest_artifact_invocation.schema_version",
        )?;
        check_string_match(
            self.manifest_id.as_str(),
            manifest.manifest_id.as_str(),
            "psion_plugin_guest_artifact_invocation.manifest_id",
        )?;
        check_string_match(
            self.manifest_digest.as_str(),
            manifest.manifest_digest.as_str(),
            "psion_plugin_guest_artifact_invocation.manifest_digest",
        )?;
        validate_tool_projection(&self.tool_projection, manifest)?;
        self.success_case.validate(
            manifest,
            "psion_plugin_guest_artifact_invocation.success_case",
            true,
        )?;
        if self.refusal_cases.is_empty() {
            return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
                field: String::from("psion_plugin_guest_artifact_invocation.refusal_cases"),
                expected: String::from("at least one explicit refusal case"),
                actual: String::from("0"),
            });
        }
        for (index, case) in self.refusal_cases.iter().enumerate() {
            case.validate(
                manifest,
                format!("psion_plugin_guest_artifact_invocation.refusal_cases[{index}]").as_str(),
                false,
            )?;
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "psion_plugin_guest_artifact_invocation.claim_boundary",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "psion_plugin_guest_artifact_invocation.summary",
        )?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionPluginGuestArtifactInvocationError::DigestMismatch {
                kind: String::from("psion_plugin_guest_artifact_invocation"),
            });
        }
        Ok(())
    }

    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginGuestArtifactInvocationError> {
        write_json_file(self, output_path)
    }
}

impl PsionPluginGuestArtifactInvocationCase {
    fn validate(
        &self,
        manifest: &PsionPluginGuestArtifactManifest,
        field: &str,
        expect_success: bool,
    ) -> Result<(), PsionPluginGuestArtifactInvocationError> {
        let expected_status = if expect_success {
            PsionPluginGuestArtifactInvocationCaseStatus::ExactSuccess
        } else {
            PsionPluginGuestArtifactInvocationCaseStatus::TypedRefusal
        };
        if self.status != expected_status {
            return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
                field: format!("{field}.status"),
                expected: format!("{expected_status:?}"),
                actual: format!("{:?}", self.status),
            });
        }
        ensure_digest(
            self.request_packet_digest.as_str(),
            format!("{field}.request_packet_digest").as_str(),
        )?;
        check_string_match(
            self.replay_class_id.as_str(),
            manifest.replay_class_id.as_str(),
            format!("{field}.replay_class_id").as_str(),
        )?;
        validate_projected_result(
            &self.projected_result,
            manifest,
            self.request_packet_digest.as_str(),
            self.response_or_refusal_schema_id.as_str(),
            self.response_or_refusal_digest.as_str(),
            expect_success,
            field,
        )?;
        if !self.receipt_binding_preserved {
            return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
                field: format!("{field}.receipt_binding_preserved"),
                expected: String::from("true"),
                actual: String::from("false"),
            });
        }
        if self.typed_refusal_preserved != !expect_success {
            return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
                field: format!("{field}.typed_refusal_preserved"),
                expected: format!("{}", !expect_success),
                actual: format!("{}", self.typed_refusal_preserved),
            });
        }
        if expect_success {
            if self.refusal_code.is_some() {
                return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
                    field: format!("{field}.refusal_code"),
                    expected: String::from("none"),
                    actual: String::from("some"),
                });
            }
            check_string_match(
                self.response_or_refusal_schema_id.as_str(),
                manifest.success_output_schema_id.as_str(),
                format!("{field}.response_or_refusal_schema_id").as_str(),
            )?;
            let packet: PsionPluginGuestArtifactEchoGuestPacket =
                serde_json::from_value(self.projected_result.structured_payload.clone()).map_err(
                    |error| PsionPluginGuestArtifactInvocationError::StructuredPayloadDecode {
                        field: format!("{field}.projected_result.structured_payload"),
                        error,
                    },
                )?;
            ensure_nonempty(
                packet.text.as_str(),
                format!("{field}.projected_result.structured_payload.text").as_str(),
            )?;
        } else {
            let refusal_code = self.refusal_code.ok_or_else(|| {
                PsionPluginGuestArtifactInvocationError::FieldMismatch {
                    field: format!("{field}.refusal_code"),
                    expected: String::from("some"),
                    actual: String::from("none"),
                }
            })?;
            let refusal: StarterPluginRefusal =
                serde_json::from_value(self.projected_result.structured_payload.clone()).map_err(
                    |error| PsionPluginGuestArtifactInvocationError::StructuredPayloadDecode {
                        field: format!("{field}.projected_result.structured_payload"),
                        error,
                    },
                )?;
            ensure_nonempty(
                refusal.refusal_class_id.as_str(),
                format!("{field}.projected_result.structured_payload.refusal_class_id").as_str(),
            )?;
            let expected_schema = refusal_schema_for_code(refusal_code);
            check_string_match(
                self.response_or_refusal_schema_id.as_str(),
                expected_schema,
                format!("{field}.response_or_refusal_schema_id").as_str(),
            )?;
        }
        ensure_digest(
            self.response_or_refusal_digest.as_str(),
            format!("{field}.response_or_refusal_digest").as_str(),
        )?;
        ensure_nonempty(self.detail.as_str(), format!("{field}.detail").as_str())?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginGuestArtifactInvocationError {
    #[error(transparent)]
    Manifest(#[from] PsionPluginGuestArtifactManifestError),
    #[error(transparent)]
    Loading(#[from] PsionPluginGuestArtifactRuntimeLoadingError),
    #[error("field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("digest drifted for `{kind}`")]
    DigestMismatch { kind: String },
    #[error("failed to encode projected payload for `{field}`: {error}")]
    StructuredPayloadEncode {
        field: String,
        error: serde_json::Error,
    },
    #[error("failed to decode projected payload for `{field}`: {error}")]
    StructuredPayloadDecode {
        field: String,
        error: serde_json::Error,
    },
    #[error("failed to compile the guest-artifact module: {detail}")]
    CompileModule { detail: String },
    #[error("failed to instantiate the guest-artifact module: {detail}")]
    InstantiateModule { detail: String },
    #[error("guest-artifact module omitted exported memory")]
    MissingMemory,
    #[error("guest-artifact module omitted exported `handle_packet`")]
    MissingHandlePacket,
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
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
pub fn psion_plugin_guest_artifact_tool_projection(
    manifest: &PsionPluginGuestArtifactManifest,
) -> StarterPluginToolProjection {
    StarterPluginToolProjection {
        plugin_id: manifest.plugin_id.clone(),
        tool_name: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME),
        description: String::from(
            "Echo one text field through the bounded guest-artifact packet path while preserving the host-native receipt and replay envelope.",
        ),
        arguments_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["text"],
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text echoed by the bounded guest-artifact reference plugin."
                }
            }
        }),
        result_schema_id: manifest.success_output_schema_id.clone(),
        refusal_schema_ids: manifest.refusal_schema_ids.clone(),
        replay_class_id: manifest.replay_class_id.clone(),
    }
}

pub fn execute_reference_psion_plugin_guest_artifact_tool_call(
    packet: &[u8],
) -> Result<StarterPluginProjectedToolResultEnvelope, PsionPluginGuestArtifactInvocationError> {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
    let case_id = format!(
        "guest_artifact_tool_call_{}",
        &sha256_hex(packet)[..12]
    );
    let case = invoke_psion_plugin_guest_artifact_json_packet(
        case_id.as_str(),
        &manifest,
        artifact_bytes.as_slice(),
        packet,
    )?;
    Ok(case.projected_result)
}

pub fn invoke_psion_plugin_guest_artifact_json_packet(
    case_id: &str,
    manifest: &PsionPluginGuestArtifactManifest,
    artifact_bytes: &[u8],
    packet: &[u8],
) -> Result<PsionPluginGuestArtifactInvocationCase, PsionPluginGuestArtifactInvocationError> {
    manifest.validate()?;
    let input_packet_digest = sha256_hex(packet);
    let loaded = load_psion_plugin_guest_artifact(manifest, artifact_bytes)?;
    if loaded.status != PsionPluginGuestArtifactLoadStatus::Loaded {
        let detail = loaded.refusal.as_ref().map_or_else(
            || String::from("guest-artifact load refused"),
            |refusal| {
                format!(
                    "guest-artifact invocation refused because load failed closed: {}",
                    refusal.detail
                )
            },
        );
        return refusal_case(
            case_id,
            manifest,
            input_packet_digest,
            PsionPluginGuestArtifactInvocationRefusalCode::LoadRefusal,
            detail,
        );
    }
    if packet.len() > PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_LIMIT_BYTES {
        return refusal_case(
            case_id,
            manifest,
            input_packet_digest,
            PsionPluginGuestArtifactInvocationRefusalCode::PacketTooLarge,
            format!(
                "guest-artifact invocation refuses packets above {} bytes to keep the first guest lane bounded.",
                PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_LIMIT_BYTES
            ),
        );
    }
    let request: PsionPluginGuestArtifactEchoGuestPacket = match serde_json::from_slice::<
        PsionPluginGuestArtifactEchoGuestPacket,
    >(packet)
    {
        Ok(request) if !request.text.trim().is_empty() => request,
        Ok(_) => {
            return refusal_case(
                case_id,
                manifest,
                input_packet_digest,
                PsionPluginGuestArtifactInvocationRefusalCode::SchemaInvalid,
                String::from(
                    "guest-artifact invocation refuses empty or schema-drifted JSON packets instead of inventing host-side repairs.",
                ),
            );
        }
        Err(_) => {
            return refusal_case(
                case_id,
                manifest,
                input_packet_digest,
                PsionPluginGuestArtifactInvocationRefusalCode::SchemaInvalid,
                String::from(
                    "guest-artifact invocation accepts only the bounded JSON packet form for the reference echo plugin.",
                ),
            );
        }
    };

    let output_bytes = match execute_reference_guest_packet(artifact_bytes, packet) {
        Ok(output_bytes) => output_bytes,
        Err(detail) => {
            return refusal_case(
                case_id,
                manifest,
                input_packet_digest,
                PsionPluginGuestArtifactInvocationRefusalCode::RuntimeUnavailable,
                detail,
            );
        }
    };
    let response: PsionPluginGuestArtifactEchoGuestPacket = match serde_json::from_slice::<
        PsionPluginGuestArtifactEchoGuestPacket,
    >(&output_bytes)
    {
        Ok(response) if response.text == request.text => response,
        Ok(_) => {
            return refusal_case(
                case_id,
                manifest,
                input_packet_digest,
                PsionPluginGuestArtifactInvocationRefusalCode::RuntimeUnavailable,
                String::from(
                    "guest-artifact invocation refuses guest output that does not preserve the bounded echo contract.",
                ),
            );
        }
        Err(_) => {
            return refusal_case(
                case_id,
                manifest,
                input_packet_digest,
                PsionPluginGuestArtifactInvocationRefusalCode::RuntimeUnavailable,
                String::from(
                    "guest-artifact invocation refuses guest output that does not decode under the declared response schema.",
                ),
            );
        }
    };
    let response_digest = sha256_hex(output_bytes.as_slice());
    let receipt = guest_artifact_receipt(
        manifest,
        StarterPluginInvocationStatus::Success,
        input_packet_digest.as_str(),
        manifest.success_output_schema_id.as_str(),
        response_digest.as_str(),
        None,
        "guest-artifact invocation emits the same receipt class as host-native starter plugins while keeping the guest lane digest-bound and replay-explicit.",
    );
    let projected_result = StarterPluginProjectedToolResultEnvelope {
        tool_name: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME),
        plugin_id: manifest.plugin_id.clone(),
        plugin_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION),
        status: StarterPluginInvocationStatus::Success,
        output_or_refusal_schema_id: manifest.success_output_schema_id.clone(),
        replay_class_id: manifest.replay_class_id.clone(),
        structured_payload: serde_json::to_value(&response).map_err(|error| {
            PsionPluginGuestArtifactInvocationError::StructuredPayloadEncode {
                field: String::from("success.projected_result.structured_payload"),
                error,
            }
        })?,
        plugin_receipt: receipt,
    };
    Ok(PsionPluginGuestArtifactInvocationCase {
        case_id: String::from(case_id),
        status: PsionPluginGuestArtifactInvocationCaseStatus::ExactSuccess,
        request_packet_digest: input_packet_digest,
        response_or_refusal_schema_id: manifest.success_output_schema_id.clone(),
        response_or_refusal_digest: response_digest.clone(),
        replay_class_id: manifest.replay_class_id.clone(),
        refusal_code: None,
        receipt_binding_preserved: projected_result.plugin_receipt.output_or_refusal_schema_id
            == manifest.success_output_schema_id
            && projected_result.plugin_receipt.output_or_refusal_digest
                == response_digest,
        typed_refusal_preserved: false,
        projected_result,
        detail: String::from(
            "the bounded guest-artifact invocation path executes one digest-bound Wasm echo handler and emits a host-native-equivalent receipt-bound tool result.",
        ),
    })
}

#[must_use]
pub fn build_psion_plugin_guest_artifact_invocation_bundle(
) -> PsionPluginGuestArtifactInvocationBundle {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
    let tool_projection = psion_plugin_guest_artifact_tool_projection(&manifest);
    let success_case = invoke_psion_plugin_guest_artifact_json_packet(
        "guest_artifact_echo_success",
        &manifest,
        artifact_bytes.as_slice(),
        br#"{"text":"guest-artifact echo proof"}"#,
    )
    .expect("reference guest invocation should succeed");
    let refusal_cases = vec![
        invoke_psion_plugin_guest_artifact_json_packet(
            "guest_artifact_schema_invalid",
            &manifest,
            artifact_bytes.as_slice(),
            br#"{"message":"wrong field"}"#,
        )
        .expect("schema invalid should remain typed refusal"),
        invoke_psion_plugin_guest_artifact_json_packet(
            "guest_artifact_packet_too_large",
            &manifest,
            artifact_bytes.as_slice(),
            &serde_json::to_vec(&PsionPluginGuestArtifactEchoGuestPacket {
                text: "x".repeat(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PACKET_LIMIT_BYTES + 1),
            })
            .expect("oversized packet should serialize"),
        )
        .expect("packet too large should remain typed refusal"),
        invoke_psion_plugin_guest_artifact_json_packet(
            "guest_artifact_runtime_unavailable",
            &manifest,
            b"not-the-declared-artifact",
            br#"{"text":"guest-artifact echo proof"}"#,
        )
        .expect("load refusal should remain typed invocation refusal"),
    ];
    let mut bundle = PsionPluginGuestArtifactInvocationBundle {
        schema_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_SCHEMA_VERSION),
        bundle_id: String::from("psion_plugin_guest_artifact_invocation"),
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: manifest.manifest_digest.clone(),
        tool_projection,
        success_case,
        refusal_cases,
        claim_boundary: String::from(
            "this bundle closes only the bounded receipt-equivalent guest-artifact invocation path. It proves one digest-bound Wasm guest can execute one packet echo call under host-owned bounds while emitting the same receipt class, replay class, typed refusal shape, and projected tool-result envelope as the host-native starter-plugin lane. It still does not claim guest-artifact catalog admission, controller breadth, publication, or arbitrary Wasm plugin support.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Guest-artifact invocation bundle covers refusal_cases={}, tool_name=`{}`, and preserves receipt_binding_preserved=true across every case.",
        bundle.refusal_cases.len(),
        bundle.tool_projection.tool_name
    );
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle
}

#[must_use]
pub fn psion_plugin_guest_artifact_invocation_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF)
}

pub fn write_psion_plugin_guest_artifact_invocation_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginGuestArtifactInvocationBundle, PsionPluginGuestArtifactInvocationError> {
    let manifest = reference_psion_plugin_guest_artifact_manifest();
    let bundle = build_psion_plugin_guest_artifact_invocation_bundle();
    bundle.validate(&manifest)?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

fn refusal_case(
    case_id: &str,
    manifest: &PsionPluginGuestArtifactManifest,
    input_packet_digest: String,
    refusal_code: PsionPluginGuestArtifactInvocationRefusalCode,
    detail: String,
) -> Result<PsionPluginGuestArtifactInvocationCase, PsionPluginGuestArtifactInvocationError> {
    let refusal = StarterPluginRefusal {
        schema_id: String::from(refusal_schema_for_code(refusal_code)),
        refusal_class_id: String::from(refusal_class_for_code(refusal_code)),
        detail: detail.clone(),
    };
    let refusal_digest = stable_serialized_digest(&refusal)?;
    let receipt = guest_artifact_receipt(
        manifest,
        StarterPluginInvocationStatus::Refusal,
        input_packet_digest.as_str(),
        refusal.schema_id.as_str(),
        refusal_digest.as_str(),
        Some(refusal.refusal_class_id.as_str()),
        detail.as_str(),
    );
    let projected_result = StarterPluginProjectedToolResultEnvelope {
        tool_name: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME),
        plugin_id: manifest.plugin_id.clone(),
        plugin_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION),
        status: StarterPluginInvocationStatus::Refusal,
        output_or_refusal_schema_id: refusal.schema_id.clone(),
        replay_class_id: manifest.replay_class_id.clone(),
        structured_payload: serde_json::to_value(&refusal).map_err(|error| {
            PsionPluginGuestArtifactInvocationError::StructuredPayloadEncode {
                field: String::from("refusal.projected_result.structured_payload"),
                error,
            }
        })?,
        plugin_receipt: receipt,
    };
    Ok(PsionPluginGuestArtifactInvocationCase {
        case_id: String::from(case_id),
        status: PsionPluginGuestArtifactInvocationCaseStatus::TypedRefusal,
        request_packet_digest: input_packet_digest,
        response_or_refusal_schema_id: refusal.schema_id,
        response_or_refusal_digest: refusal_digest.clone(),
        replay_class_id: manifest.replay_class_id.clone(),
        refusal_code: Some(refusal_code),
        receipt_binding_preserved: projected_result.plugin_receipt.output_or_refusal_schema_id
            == projected_result.output_or_refusal_schema_id
            && projected_result.plugin_receipt.output_or_refusal_digest == refusal_digest,
        typed_refusal_preserved: true,
        projected_result,
        detail,
    })
}

fn guest_artifact_receipt(
    manifest: &PsionPluginGuestArtifactManifest,
    status: StarterPluginInvocationStatus,
    input_packet_digest: &str,
    output_or_refusal_schema_id: &str,
    output_or_refusal_digest: &str,
    refusal_class_id: Option<&str>,
    detail: &str,
) -> StarterPluginInvocationReceipt {
    let mut receipt = StarterPluginInvocationReceipt {
        receipt_id: format!(
            "receipt.{}.{}.v1",
            manifest.plugin_id,
            &input_packet_digest[..16]
        ),
        plugin_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_ID),
        plugin_version: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION),
        tool_name: String::from(PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME),
        packet_abi_version: manifest.packet_abi_version.clone(),
        mount_envelope_id: String::from(PSION_PLUGIN_GUEST_ARTIFACT_MOUNT_ENVELOPE_ID),
        capability_namespace_ids: manifest.capability_namespace_ids.clone(),
        replay_class_id: manifest.replay_class_id.clone(),
        status,
        input_schema_id: manifest.input_schema_id.clone(),
        input_packet_digest: String::from(input_packet_digest),
        output_or_refusal_schema_id: String::from(output_or_refusal_schema_id),
        output_or_refusal_digest: String::from(output_or_refusal_digest),
        refusal_class_id: refusal_class_id.map(String::from),
        detail: String::from(detail),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_receipt_digest(&receipt);
    receipt
}

fn execute_reference_guest_packet(artifact_bytes: &[u8], packet: &[u8]) -> Result<Vec<u8>, String> {
    let engine = Engine::default();
    let module = Module::new(&engine, artifact_bytes)
        .map_err(|error| format!("guest-artifact module compilation failed: {error}"))?;
    let mut store = Store::new(&engine, ());
    let linker = Linker::<()>::new(&engine);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|error| format!("guest-artifact module instantiation failed: {error}"))?;
    let memory = instance
        .get_memory(&store, "memory")
        .ok_or_else(|| String::from("guest-artifact module omitted exported memory"))?;
    let handle_packet = instance
        .get_typed_func::<(i32, i32, i32), i32>(&store, "handle_packet")
        .map_err(|_| String::from("guest-artifact module omitted exported handle_packet"))?;
    memory
        .write(&mut store, PSION_PLUGIN_GUEST_ARTIFACT_INPUT_PTR, packet)
        .map_err(|error| format!("guest-artifact memory write failed: {error}"))?;
    let output_len = handle_packet
        .call(
            &mut store,
            (
                PSION_PLUGIN_GUEST_ARTIFACT_INPUT_PTR as i32,
                packet.len() as i32,
                PSION_PLUGIN_GUEST_ARTIFACT_OUTPUT_PTR as i32,
            ),
        )
        .map_err(|error| format!("guest-artifact handle_packet call failed: {error}"))?;
    if output_len < 0 || output_len as usize > packet.len() {
        return Err(String::from(
            "guest-artifact handle_packet returned an out-of-bounds output length",
        ));
    }
    let mut output = vec![0_u8; output_len as usize];
    memory
        .read(
            &store,
            PSION_PLUGIN_GUEST_ARTIFACT_OUTPUT_PTR,
            output.as_mut_slice(),
        )
        .map_err(|error| format!("guest-artifact memory read failed: {error}"))?;
    Ok(output)
}

fn validate_tool_projection(
    projection: &StarterPluginToolProjection,
    manifest: &PsionPluginGuestArtifactManifest,
) -> Result<(), PsionPluginGuestArtifactInvocationError> {
    check_string_match(
        projection.plugin_id.as_str(),
        manifest.plugin_id.as_str(),
        "psion_plugin_guest_artifact_invocation.tool_projection.plugin_id",
    )?;
    check_string_match(
        projection.tool_name.as_str(),
        PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME,
        "psion_plugin_guest_artifact_invocation.tool_projection.tool_name",
    )?;
    check_string_match(
        projection.result_schema_id.as_str(),
        manifest.success_output_schema_id.as_str(),
        "psion_plugin_guest_artifact_invocation.tool_projection.result_schema_id",
    )?;
    check_string_match(
        projection.replay_class_id.as_str(),
        manifest.replay_class_id.as_str(),
        "psion_plugin_guest_artifact_invocation.tool_projection.replay_class_id",
    )?;
    if projection.refusal_schema_ids != manifest.refusal_schema_ids {
        return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
            field: String::from(
                "psion_plugin_guest_artifact_invocation.tool_projection.refusal_schema_ids",
            ),
            expected: format!("{:?}", manifest.refusal_schema_ids),
            actual: format!("{:?}", projection.refusal_schema_ids),
        });
    }
    Ok(())
}

fn validate_projected_result(
    projected_result: &StarterPluginProjectedToolResultEnvelope,
    manifest: &PsionPluginGuestArtifactManifest,
    request_packet_digest: &str,
    response_or_refusal_schema_id: &str,
    response_or_refusal_digest: &str,
    expect_success: bool,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactInvocationError> {
    check_string_match(
        projected_result.tool_name.as_str(),
        PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME,
        format!("{field}.projected_result.tool_name").as_str(),
    )?;
    check_string_match(
        projected_result.plugin_id.as_str(),
        manifest.plugin_id.as_str(),
        format!("{field}.projected_result.plugin_id").as_str(),
    )?;
    check_string_match(
        projected_result.plugin_version.as_str(),
        manifest.plugin_version.as_str(),
        format!("{field}.projected_result.plugin_version").as_str(),
    )?;
    check_string_match(
        projected_result.replay_class_id.as_str(),
        manifest.replay_class_id.as_str(),
        format!("{field}.projected_result.replay_class_id").as_str(),
    )?;
    check_string_match(
        projected_result.output_or_refusal_schema_id.as_str(),
        response_or_refusal_schema_id,
        format!("{field}.projected_result.output_or_refusal_schema_id").as_str(),
    )?;
    let expected_status = if expect_success {
        StarterPluginInvocationStatus::Success
    } else {
        StarterPluginInvocationStatus::Refusal
    };
    if projected_result.status != expected_status {
        return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
            field: format!("{field}.projected_result.status"),
            expected: format!("{expected_status:?}"),
            actual: format!("{:?}", projected_result.status),
        });
    }
    let receipt = &projected_result.plugin_receipt;
    check_string_match(
        receipt.plugin_id.as_str(),
        manifest.plugin_id.as_str(),
        format!("{field}.projected_result.plugin_receipt.plugin_id").as_str(),
    )?;
    check_string_match(
        receipt.tool_name.as_str(),
        PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME,
        format!("{field}.projected_result.plugin_receipt.tool_name").as_str(),
    )?;
    check_string_match(
        receipt.packet_abi_version.as_str(),
        manifest.packet_abi_version.as_str(),
        format!("{field}.projected_result.plugin_receipt.packet_abi_version").as_str(),
    )?;
    check_string_match(
        receipt.mount_envelope_id.as_str(),
        PSION_PLUGIN_GUEST_ARTIFACT_MOUNT_ENVELOPE_ID,
        format!("{field}.projected_result.plugin_receipt.mount_envelope_id").as_str(),
    )?;
    check_string_match(
        receipt.input_packet_digest.as_str(),
        request_packet_digest,
        format!("{field}.projected_result.plugin_receipt.input_packet_digest").as_str(),
    )?;
    check_string_match(
        receipt.output_or_refusal_schema_id.as_str(),
        response_or_refusal_schema_id,
        format!("{field}.projected_result.plugin_receipt.output_or_refusal_schema_id").as_str(),
    )?;
    check_string_match(
        receipt.output_or_refusal_digest.as_str(),
        response_or_refusal_digest,
        format!("{field}.projected_result.plugin_receipt.output_or_refusal_digest").as_str(),
    )?;
    if receipt.receipt_digest != stable_receipt_digest(receipt) {
        return Err(PsionPluginGuestArtifactInvocationError::DigestMismatch {
            kind: String::from("psion_plugin_guest_artifact_invocation.receipt"),
        });
    }
    Ok(())
}

fn refusal_schema_for_code(code: PsionPluginGuestArtifactInvocationRefusalCode) -> &'static str {
    match code {
        PsionPluginGuestArtifactInvocationRefusalCode::SchemaInvalid => {
            "plugin.refusal.schema_invalid.v1"
        }
        PsionPluginGuestArtifactInvocationRefusalCode::PacketTooLarge => {
            "plugin.refusal.packet_too_large.v1"
        }
        PsionPluginGuestArtifactInvocationRefusalCode::LoadRefusal
        | PsionPluginGuestArtifactInvocationRefusalCode::RuntimeUnavailable => {
            "plugin.refusal.runtime_unavailable.v1"
        }
    }
}

fn refusal_class_for_code(code: PsionPluginGuestArtifactInvocationRefusalCode) -> &'static str {
    match code {
        PsionPluginGuestArtifactInvocationRefusalCode::SchemaInvalid => "schema_invalid",
        PsionPluginGuestArtifactInvocationRefusalCode::PacketTooLarge => "packet_too_large",
        PsionPluginGuestArtifactInvocationRefusalCode::LoadRefusal => "guest_artifact_load_refusal",
        PsionPluginGuestArtifactInvocationRefusalCode::RuntimeUnavailable => {
            "guest_artifact_runtime_unavailable"
        }
    }
}

fn stable_receipt_digest(receipt: &StarterPluginInvocationReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_plugin_guest_artifact_invocation_receipt_v1");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(receipt.plugin_id.as_bytes());
    hasher.update(receipt.plugin_version.as_bytes());
    hasher.update(receipt.tool_name.as_bytes());
    hasher.update(receipt.packet_abi_version.as_bytes());
    hasher.update(receipt.mount_envelope_id.as_bytes());
    for capability_namespace_id in &receipt.capability_namespace_ids {
        hasher.update(capability_namespace_id.as_bytes());
    }
    hasher.update(receipt.replay_class_id.as_bytes());
    hasher.update(format!("{:?}", receipt.status).as_bytes());
    hasher.update(receipt.input_schema_id.as_bytes());
    hasher.update(receipt.input_packet_digest.as_bytes());
    hasher.update(receipt.output_or_refusal_schema_id.as_bytes());
    hasher.update(receipt.output_or_refusal_digest.as_bytes());
    if let Some(refusal_class_id) = &receipt.refusal_class_id {
        hasher.update(refusal_class_id.as_bytes());
    }
    hasher.update(receipt.detail.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_bundle_digest(bundle: &PsionPluginGuestArtifactInvocationBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(bundle.manifest_id.as_bytes());
    hasher.update(bundle.manifest_digest.as_bytes());
    hasher.update(bundle.tool_projection.plugin_id.as_bytes());
    hasher.update(bundle.tool_projection.tool_name.as_bytes());
    hasher.update(bundle.tool_projection.result_schema_id.as_bytes());
    for refusal_schema_id in &bundle.tool_projection.refusal_schema_ids {
        hasher.update(refusal_schema_id.as_bytes());
    }
    hasher.update(bundle.tool_projection.replay_class_id.as_bytes());
    hasher.update(stable_case_digest(&bundle.success_case).as_bytes());
    for refusal_case in &bundle.refusal_cases {
        hasher.update(stable_case_digest(refusal_case).as_bytes());
    }
    hasher.update(bundle.claim_boundary.as_bytes());
    hasher.update(bundle.summary.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_case_digest(case: &PsionPluginGuestArtifactInvocationCase) -> String {
    let mut hasher = Sha256::new();
    hasher.update(case.case_id.as_bytes());
    hasher.update(format!("{:?}", case.status).as_bytes());
    hasher.update(case.request_packet_digest.as_bytes());
    hasher.update(case.response_or_refusal_schema_id.as_bytes());
    hasher.update(case.response_or_refusal_digest.as_bytes());
    hasher.update(case.replay_class_id.as_bytes());
    if let Some(refusal_code) = case.refusal_code {
        hasher.update(format!("{:?}", refusal_code).as_bytes());
    }
    hasher.update(stable_projected_result_digest(&case.projected_result).as_bytes());
    hasher.update([case.receipt_binding_preserved as u8]);
    hasher.update([case.typed_refusal_preserved as u8]);
    hasher.update(case.detail.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_projected_result_digest(result: &StarterPluginProjectedToolResultEnvelope) -> String {
    let mut hasher = Sha256::new();
    hasher.update(result.tool_name.as_bytes());
    hasher.update(result.plugin_id.as_bytes());
    hasher.update(result.plugin_version.as_bytes());
    hasher.update(format!("{:?}", result.status).as_bytes());
    hasher.update(result.output_or_refusal_schema_id.as_bytes());
    hasher.update(result.replay_class_id.as_bytes());
    hasher.update(
        serde_json::to_vec(&result.structured_payload)
            .expect("projected payload should always serialize"),
    );
    hasher.update(result.plugin_receipt.receipt_digest.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn stable_serialized_digest<T: Serialize>(
    value: &T,
) -> Result<String, PsionPluginGuestArtifactInvocationError> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactInvocationError> {
    if value.trim().is_empty() {
        return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
            field: String::from(field),
            expected: String::from("non-empty"),
            actual: String::from("empty"),
        });
    }
    Ok(())
}

fn ensure_digest(value: &str, field: &str) -> Result<(), PsionPluginGuestArtifactInvocationError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
            field: String::from(field),
            expected: String::from("64 lowercase hex characters"),
            actual: String::from(value),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPluginGuestArtifactInvocationError> {
    if actual != expected {
        return Err(PsionPluginGuestArtifactInvocationError::FieldMismatch {
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
) -> Result<(), PsionPluginGuestArtifactInvocationError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionPluginGuestArtifactInvocationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(output_path, bytes).map_err(|error| PsionPluginGuestArtifactInvocationError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, PsionPluginGuestArtifactInvocationError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| PsionPluginGuestArtifactInvocationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionPluginGuestArtifactInvocationError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{
        build_psion_plugin_guest_artifact_invocation_bundle,
        invoke_psion_plugin_guest_artifact_json_packet,
        psion_plugin_guest_artifact_invocation_path, read_json,
        write_psion_plugin_guest_artifact_invocation_bundle,
        PsionPluginGuestArtifactInvocationBundle, PsionPluginGuestArtifactInvocationCaseStatus,
        PsionPluginGuestArtifactInvocationRefusalCode, PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF,
    };
    use crate::{
        reference_psion_plugin_guest_artifact_bytes, reference_psion_plugin_guest_artifact_manifest,
    };

    #[test]
    fn guest_artifact_invocation_bundle_validates() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let bundle = build_psion_plugin_guest_artifact_invocation_bundle();
        bundle.validate(&manifest)?;
        Ok(())
    }

    #[test]
    fn guest_artifact_invocation_executes_reference_echo() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
        let case = invoke_psion_plugin_guest_artifact_json_packet(
            "reference_echo",
            &manifest,
            artifact_bytes.as_slice(),
            br#"{"text":"echo me"}"#,
        )?;
        assert_eq!(
            case.status,
            PsionPluginGuestArtifactInvocationCaseStatus::ExactSuccess
        );
        assert!(case.receipt_binding_preserved);
        Ok(())
    }

    #[test]
    fn guest_artifact_invocation_refuses_schema_drift() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let artifact_bytes = reference_psion_plugin_guest_artifact_bytes();
        let case = invoke_psion_plugin_guest_artifact_json_packet(
            "schema_invalid",
            &manifest,
            artifact_bytes.as_slice(),
            br#"{"message":"wrong"}"#,
        )?;
        assert_eq!(
            case.refusal_code,
            Some(PsionPluginGuestArtifactInvocationRefusalCode::SchemaInvalid)
        );
        Ok(())
    }

    #[test]
    fn committed_guest_artifact_invocation_fixture_validates() -> Result<(), Box<dyn Error>> {
        let manifest = reference_psion_plugin_guest_artifact_manifest();
        let bundle: PsionPluginGuestArtifactInvocationBundle =
            read_json(psion_plugin_guest_artifact_invocation_path())?;
        bundle.validate(&manifest)?;
        Ok(())
    }

    #[test]
    fn write_guest_artifact_invocation_fixture_persists_current_truth() -> Result<(), Box<dyn Error>>
    {
        let tempdir = tempfile::tempdir()?;
        let output_path = tempdir
            .path()
            .join("psion_plugin_guest_artifact_invocation_v1.json");
        let written = write_psion_plugin_guest_artifact_invocation_bundle(&output_path)?;
        let persisted: PsionPluginGuestArtifactInvocationBundle = read_json(&output_path)?;
        assert_eq!(written, persisted);
        Ok(())
    }

    #[test]
    fn invocation_fixture_ref_matches_committed_path() {
        assert!(psion_plugin_guest_artifact_invocation_path()
            .display()
            .to_string()
            .ends_with(PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF));
    }
}
