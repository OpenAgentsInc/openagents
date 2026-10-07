use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_packet_abi_and_rust_pdk_v1/tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_packet_abi_and_rust_pdk_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION: &str = "packet.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID: &str =
    "tassadar.plugin.rust_first_pdk.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID: &str =
    "tassadar.plugin_host.packet.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginPacketAbiCaseStatus {
    ExactOutputPacket,
    ExactTypedRefusal,
    ExactHostError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketRecord {
    pub schema_id: String,
    pub codec_id: String,
    pub payload_sha256: String,
    pub payload_bytes_len: u32,
    pub metadata_field_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginHostImportBinding {
    pub import_id: String,
    pub namespace_id: String,
    pub capability_scope: String,
    pub deterministic_surface: bool,
    pub ambient_authority_allowed: bool,
    pub out_of_band_data_allowed: bool,
    pub receipt_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginTypedRefusalDescriptor {
    pub refusal_id: String,
    pub retry_posture: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiCaseReceipt {
    pub case_id: String,
    pub handler_export: String,
    pub status: TassadarPostArticlePluginPacketAbiCaseStatus,
    pub input_packet: TassadarPostArticlePluginPacketRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_packet: Option<TassadarPostArticlePluginPacketRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_refusal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_error_id: Option<String>,
    pub invoked_host_import_ids: Vec<String>,
    pub emitted_receipt_field_ids: Vec<String>,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub packet_abi_version: String,
    pub rust_first_pdk_id: String,
    pub host_import_namespace_id: String,
    pub packet_field_ids: Vec<String>,
    pub host_imports: Vec<TassadarPostArticlePluginHostImportBinding>,
    pub typed_refusals: Vec<TassadarPostArticlePluginTypedRefusalDescriptor>,
    pub host_error_channel_ids: Vec<String>,
    pub receipt_field_ids: Vec<String>,
    pub case_receipts: Vec<TassadarPostArticlePluginPacketAbiCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginPacketAbiAndRustPdkBundleError {
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
pub fn build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle(
) -> TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle {
    let host_imports = vec![
        host_import(
            "host.read_invocation_context_v1",
            "context_read_only",
            true,
            false,
            false,
            true,
        ),
        host_import(
            "host.request_capability_packet_v1",
            "capability_mediated_packet_call",
            true,
            false,
            false,
            true,
        ),
        host_import(
            "host.emit_receipt_fact_v1",
            "receipt_annotation",
            true,
            false,
            false,
            true,
        ),
    ];
    let typed_refusals = vec![
        refusal(
            "schema_invalid",
            "fail_closed_no_retry",
            "schema mismatch remains a guest-visible typed refusal.",
        ),
        refusal(
            "codec_unsupported",
            "fail_closed_no_retry",
            "unsupported codecs remain a typed refusal instead of host-side best effort.",
        ),
    ];
    let host_error_channel_ids = vec![String::from("capability_namespace_unmounted")];
    let receipt_field_ids = vec![
        String::from("invocation_identity_digest"),
        String::from("input_packet_digest"),
        String::from("output_packet_digest"),
        String::from("typed_refusal_id"),
        String::from("host_error_id"),
    ];
    let case_receipts = vec![
        output_case(
            "json_echo_success",
            packet(
                "plugin.echo.input.v1",
                "json",
                41,
                &["mount_envelope_identity", "invocation_id"],
            ),
            packet(
                "plugin.echo.output.v1",
                "json",
                29,
                &["invocation_id"],
            ),
            &["host.read_invocation_context_v1"],
            &[
                "invocation_identity_digest",
                "input_packet_digest",
                "output_packet_digest",
            ],
            "the first ABI admits one JSON packet roundtrip without widening the host boundary.",
        ),
        output_case(
            "artifact_probe_success",
            packet(
                "plugin.artifact_probe.input.v1",
                "bytes",
                64,
                &["mount_envelope_identity", "capability_namespace_id"],
            ),
            packet(
                "plugin.artifact_probe.output.v1",
                "bytes",
                32,
                &["invocation_id", "artifact_ref"],
            ),
            &[
                "host.read_invocation_context_v1",
                "host.request_capability_packet_v1",
                "host.emit_receipt_fact_v1",
            ],
            &[
                "invocation_identity_digest",
                "input_packet_digest",
                "output_packet_digest",
            ],
            "capability-mediated artifact probing stays packet-shaped and receipt-bound.",
        ),
        refusal_case(
            "schema_invalid_typed_refusal",
            packet(
                "plugin.echo.input.v1",
                "json",
                17,
                &["mount_envelope_identity"],
            ),
            "schema_invalid",
            &["host.read_invocation_context_v1"],
            &["invocation_identity_digest", "input_packet_digest", "typed_refusal_id"],
            "schema drift remains explicit typed refusal truth.",
        ),
        refusal_case(
            "codec_unsupported_typed_refusal",
            packet(
                "plugin.echo.input.v1",
                "cbor",
                19,
                &["mount_envelope_identity"],
            ),
            "codec_unsupported",
            &["host.read_invocation_context_v1"],
            &["invocation_identity_digest", "input_packet_digest", "typed_refusal_id"],
            "codec drift remains explicit typed refusal truth.",
        ),
        host_error_case(
            "capability_unmounted_host_error",
            packet(
                "plugin.artifact_probe.input.v1",
                "bytes",
                24,
                &["mount_envelope_identity", "capability_namespace_id"],
            ),
            "capability_namespace_unmounted",
            &[
                "host.read_invocation_context_v1",
                "host.request_capability_packet_v1",
            ],
            &["invocation_identity_digest", "input_packet_digest", "host_error_id"],
            "unmounted capability namespaces remain an explicit host error channel instead of a guest refusal.",
        ),
    ];
    let exact_output_packet_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactOutputPacket)
        .count();
    let exact_typed_refusal_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactTypedRefusal)
        .count();
    let exact_host_error_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactHostError)
        .count();
    let mut bundle = TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from(
            "tassadar.post_article_plugin_packet_abi_and_rust_pdk.runtime_bundle.v1",
        ),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        rust_first_pdk_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID),
        host_import_namespace_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID,
        ),
        packet_field_ids: vec![
            String::from("schema_id"),
            String::from("codec_id"),
            String::from("payload_bytes"),
            String::from("metadata_envelope"),
        ],
        host_imports,
        typed_refusals,
        host_error_channel_ids,
        receipt_field_ids,
        case_receipts,
        claim_boundary: String::from(
            "this runtime bundle freezes one bounded packet.v1 plugin invocation ABI and Rust-first PDK surface with explicit output-packet, typed-refusal, and host-error cases. It does not claim weighted plugin control, public plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Post-article plugin packet ABI runtime bundle covers {} cases across {} output, {} refusal, and {} host-error receipts.",
        bundle.case_receipts.len(),
        exact_output_packet_count,
        exact_typed_refusal_count,
        exact_host_error_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_packet_abi_and_rust_pdk_runtime_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF)
}

pub fn write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
    TassadarPostArticlePluginPacketAbiAndRustPdkBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginPacketAbiAndRustPdkBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn packet(
    schema_id: &str,
    codec_id: &str,
    payload_bytes_len: u32,
    metadata_field_ids: &[&str],
) -> TassadarPostArticlePluginPacketRecord {
    let payload_seed = format!("{schema_id}|{codec_id}|{payload_bytes_len}");
    TassadarPostArticlePluginPacketRecord {
        schema_id: String::from(schema_id),
        codec_id: String::from(codec_id),
        payload_sha256: stable_digest(
            b"psionic_tassadar_post_article_plugin_packet_abi_payload|",
            &payload_seed,
        ),
        payload_bytes_len,
        metadata_field_ids: metadata_field_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
    }
}

fn host_import(
    import_id: &str,
    capability_scope: &str,
    deterministic_surface: bool,
    ambient_authority_allowed: bool,
    out_of_band_data_allowed: bool,
    receipt_required: bool,
) -> TassadarPostArticlePluginHostImportBinding {
    TassadarPostArticlePluginHostImportBinding {
        import_id: String::from(import_id),
        namespace_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID),
        capability_scope: String::from(capability_scope),
        deterministic_surface,
        ambient_authority_allowed,
        out_of_band_data_allowed,
        receipt_required,
    }
}

fn refusal(
    refusal_id: &str,
    retry_posture: &str,
    detail: &str,
) -> TassadarPostArticlePluginTypedRefusalDescriptor {
    TassadarPostArticlePluginTypedRefusalDescriptor {
        refusal_id: String::from(refusal_id),
        retry_posture: String::from(retry_posture),
        detail: String::from(detail),
    }
}

fn output_case(
    case_id: &str,
    input_packet: TassadarPostArticlePluginPacketRecord,
    output_packet: TassadarPostArticlePluginPacketRecord,
    invoked_host_import_ids: &[&str],
    emitted_receipt_field_ids: &[&str],
    note: &str,
) -> TassadarPostArticlePluginPacketAbiCaseReceipt {
    let mut receipt = TassadarPostArticlePluginPacketAbiCaseReceipt {
        case_id: String::from(case_id),
        handler_export: String::from("handle_packet"),
        status: TassadarPostArticlePluginPacketAbiCaseStatus::ExactOutputPacket,
        input_packet,
        output_packet: Some(output_packet),
        typed_refusal_id: None,
        host_error_id: None,
        invoked_host_import_ids: invoked_host_import_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        emitted_receipt_field_ids: emitted_receipt_field_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_packet_abi_case_receipt|",
        &receipt,
    );
    receipt
}

fn refusal_case(
    case_id: &str,
    input_packet: TassadarPostArticlePluginPacketRecord,
    typed_refusal_id: &str,
    invoked_host_import_ids: &[&str],
    emitted_receipt_field_ids: &[&str],
    note: &str,
) -> TassadarPostArticlePluginPacketAbiCaseReceipt {
    let mut receipt = TassadarPostArticlePluginPacketAbiCaseReceipt {
        case_id: String::from(case_id),
        handler_export: String::from("handle_packet"),
        status: TassadarPostArticlePluginPacketAbiCaseStatus::ExactTypedRefusal,
        input_packet,
        output_packet: None,
        typed_refusal_id: Some(String::from(typed_refusal_id)),
        host_error_id: None,
        invoked_host_import_ids: invoked_host_import_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        emitted_receipt_field_ids: emitted_receipt_field_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_packet_abi_case_receipt|",
        &receipt,
    );
    receipt
}

fn host_error_case(
    case_id: &str,
    input_packet: TassadarPostArticlePluginPacketRecord,
    host_error_id: &str,
    invoked_host_import_ids: &[&str],
    emitted_receipt_field_ids: &[&str],
    note: &str,
) -> TassadarPostArticlePluginPacketAbiCaseReceipt {
    let mut receipt = TassadarPostArticlePluginPacketAbiCaseReceipt {
        case_id: String::from(case_id),
        handler_export: String::from("handle_packet"),
        status: TassadarPostArticlePluginPacketAbiCaseStatus::ExactHostError,
        input_packet,
        output_packet: None,
        typed_refusal_id: None,
        host_error_id: Some(String::from(host_error_id)),
        invoked_host_import_ids: invoked_host_import_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        emitted_receipt_field_ids: emitted_receipt_field_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_packet_abi_case_receipt|",
        &receipt,
    );
    receipt
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginPacketAbiAndRustPdkBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION,
        TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID,
        TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
        TassadarPostArticlePluginPacketAbiCaseStatus,
        build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle, read_json,
        tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle_path,
        write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle,
    };

    #[test]
    fn post_article_plugin_packet_abi_bundle_keeps_channels_explicit() {
        let bundle = build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle();

        assert_eq!(
            bundle.packet_abi_version,
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION
        );
        assert_eq!(bundle.rust_first_pdk_id, TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID);
        assert_eq!(bundle.packet_field_ids.len(), 4);
        assert_eq!(bundle.host_imports.len(), 3);
        assert_eq!(bundle.typed_refusals.len(), 2);
        assert_eq!(bundle.host_error_channel_ids, vec![String::from("capability_namespace_unmounted")]);
        assert_eq!(bundle.case_receipts.len(), 5);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactHostError
                && case.host_error_id.as_deref() == Some("capability_namespace_unmounted")
        }));
    }

    #[test]
    fn post_article_plugin_packet_abi_bundle_matches_committed_truth() {
        let generated = build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle();
        let committed: TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle =
            read_json(tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle_path())
                .expect("committed bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_post_article_plugin_packet_abi_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle.json");
        let written = write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle(
            &output_path,
        )
        .expect("write bundle");
        let persisted: TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle =
            read_json(&output_path).expect("persisted bundle");
        assert_eq!(written, persisted);
    }
}
