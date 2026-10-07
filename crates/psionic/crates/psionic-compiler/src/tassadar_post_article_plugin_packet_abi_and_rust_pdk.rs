use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION: &str = "packet.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID: &str =
    "tassadar.plugin.rust_first_pdk.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID: &str =
    "tassadar.plugin_host.packet.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_REFUSAL_TYPE_ID: &str =
    "tassadar.plugin.refusal.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginPacketAbiExpectedStatus {
    ExactOutputPacket,
    ExactTypedRefusal,
    ExactHostError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketFieldSpec {
    pub field_id: String,
    pub required: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRustPdkHostImportSpec {
    pub import_id: String,
    pub namespace_id: String,
    pub capability_scope: String,
    pub deterministic_surface: bool,
    pub ambient_authority_allowed: bool,
    pub out_of_band_data_allowed: bool,
    pub receipt_required: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRustGuestSignature {
    pub guest_authoring_posture: String,
    pub handler_export: String,
    pub input_type: String,
    pub output_type: String,
    pub refusal_type_id: String,
    pub host_import_namespace_id: String,
    pub raw_wasm_admission_posture: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiCaseSpec {
    pub case_id: String,
    pub input_schema_id: String,
    pub input_codec_id: String,
    pub expected_status: TassadarPostArticlePluginPacketAbiExpectedStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output_schema_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_refusal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_host_error_id: Option<String>,
    pub required_host_import_ids: Vec<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiRustPdkCompilationContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub packet_abi_version: String,
    pub rust_first_pdk_id: String,
    pub packet_field_specs: Vec<TassadarPostArticlePluginPacketFieldSpec>,
    pub host_import_specs: Vec<TassadarPostArticlePluginRustPdkHostImportSpec>,
    pub guest_signature: TassadarPostArticlePluginRustGuestSignature,
    pub case_specs: Vec<TassadarPostArticlePluginPacketAbiCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarPostArticlePluginPacketAbiRustPdkCompilationContract {
    fn new(
        packet_field_specs: Vec<TassadarPostArticlePluginPacketFieldSpec>,
        host_import_specs: Vec<TassadarPostArticlePluginRustPdkHostImportSpec>,
        guest_signature: TassadarPostArticlePluginRustGuestSignature,
        case_specs: Vec<TassadarPostArticlePluginPacketAbiCaseSpec>,
    ) -> Self {
        let exact_output_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_status == TassadarPostArticlePluginPacketAbiExpectedStatus::ExactOutputPacket
            })
            .count();
        let exact_refusal_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_status == TassadarPostArticlePluginPacketAbiExpectedStatus::ExactTypedRefusal
            })
            .count();
        let exact_host_error_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_status == TassadarPostArticlePluginPacketAbiExpectedStatus::ExactHostError
            })
            .count();
        let mut contract = Self {
            schema_version: 1,
            contract_id: String::from(
                "tassadar.post_article_plugin_packet_abi_and_rust_pdk.compilation_contract.v1",
            ),
            packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
            rust_first_pdk_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID),
            packet_field_specs,
            host_import_specs,
            guest_signature,
            case_specs,
            claim_boundary: String::from(
                "this compilation contract freezes one bounded post-article plugin packet ABI and Rust-first guest PDK surface. It defines a single packet-shaped invocation contract, one typed refusal family, one explicit host-error channel, and one narrow host-import namespace for operator/internal plugin work. It does not claim weighted plugin control, public plugin publication, served/public universality, or arbitrary software capability.",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Post-article plugin packet ABI compilation contract freezes {} packet fields, {} host imports, and {} cases across {} output, {} refusal, and {} host-error expectations.",
            contract.packet_field_specs.len(),
            contract.host_import_specs.len(),
            contract.case_specs.len(),
            exact_output_case_count,
            exact_refusal_case_count,
            exact_host_error_case_count,
        );
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_post_article_plugin_packet_abi_and_rust_pdk_compilation_contract|",
            &contract,
        );
        contract
    }
}

#[must_use]
pub fn compile_tassadar_post_article_plugin_packet_abi_and_rust_pdk_contract(
) -> TassadarPostArticlePluginPacketAbiRustPdkCompilationContract {
    TassadarPostArticlePluginPacketAbiRustPdkCompilationContract::new(
        vec![
            packet_field_spec(
                "schema_id",
                "every packet must carry one schema id so guest and host shape drift fail closed instead of hiding translation in adapters.",
            ),
            packet_field_spec(
                "codec_id",
                "every packet must carry one codec id so content framing remains machine-readable and versioned.",
            ),
            packet_field_spec(
                "payload_bytes",
                "payload bytes remain the core transport form; typed schemas layer above bytes instead of replacing them with hidden host-owned structs.",
            ),
            packet_field_spec(
                "metadata_envelope",
                "packet metadata remains an explicit envelope rather than an ambient side channel.",
            ),
        ],
        vec![
            host_import_spec(
                "host.read_invocation_context_v1",
                "context_read_only",
                true,
                false,
                false,
                true,
                "the guest may read invocation identity and mount-envelope context through one explicit read-only import instead of ambient process globals.",
            ),
            host_import_spec(
                "host.request_capability_packet_v1",
                "capability_mediated_packet_call",
                true,
                false,
                false,
                true,
                "all guest-visible external capability use remains packet-shaped and mediated by the host instead of direct ambient IO or host-specific SDK calls.",
            ),
            host_import_spec(
                "host.emit_receipt_fact_v1",
                "receipt_annotation",
                true,
                false,
                false,
                true,
                "the guest may emit bounded receipt annotations through one explicit import instead of hidden logging side channels.",
            ),
        ],
        TassadarPostArticlePluginRustGuestSignature {
            guest_authoring_posture: String::from("rust_first_supported"),
            handler_export: String::from("handle_packet"),
            input_type: String::from("&[u8]"),
            output_type: String::from("Result<Vec<u8>, PluginRefusalV1>"),
            refusal_type_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_REFUSAL_TYPE_ID),
            host_import_namespace_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID,
            ),
            raw_wasm_admission_posture: String::from(
                "later_only_if_packet_abi_conformant_and_profile_admitted",
            ),
            detail: String::from(
                "the first admitted guest-authoring surface is one Rust crate exporting `handle_packet`, taking packet bytes, returning output bytes or `PluginRefusalV1`, and using only the narrow packet-host namespace.",
            ),
        },
        vec![
            case_spec(
                "json_echo_success",
                "plugin.echo.input.v1",
                "json",
                TassadarPostArticlePluginPacketAbiExpectedStatus::ExactOutputPacket,
                Some("plugin.echo.output.v1"),
                None,
                None,
                &["host.read_invocation_context_v1"],
                &[
                    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json",
                    "fixtures/tassadar/reports/tassadar_internal_compute_package_manager_report.json",
                ],
                "the bounded packet ABI admits one pure JSON packet roundtrip without widening to ambient host orchestration.",
            ),
            case_spec(
                "artifact_probe_success",
                "plugin.artifact_probe.input.v1",
                "bytes",
                TassadarPostArticlePluginPacketAbiExpectedStatus::ExactOutputPacket,
                Some("plugin.artifact_probe.output.v1"),
                None,
                None,
                &[
                    "host.read_invocation_context_v1",
                    "host.request_capability_packet_v1",
                    "host.emit_receipt_fact_v1",
                ],
                &[
                    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json",
                    "fixtures/tassadar/reports/tassadar_internal_component_abi_report.json",
                ],
                "the bounded packet ABI admits one capability-mediated bytes packet path with explicit receipt annotation instead of a raw host SDK call surface.",
            ),
            case_spec(
                "schema_invalid_typed_refusal",
                "plugin.echo.input.v1",
                "json",
                TassadarPostArticlePluginPacketAbiExpectedStatus::ExactTypedRefusal,
                None,
                Some("schema_invalid"),
                None,
                &["host.read_invocation_context_v1"],
                &[
                    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json",
                ],
                "schema mismatch stays in the guest-visible typed-refusal family rather than falling through to host exceptions.",
            ),
            case_spec(
                "codec_unsupported_typed_refusal",
                "plugin.echo.input.v1",
                "cbor",
                TassadarPostArticlePluginPacketAbiExpectedStatus::ExactTypedRefusal,
                None,
                Some("codec_unsupported"),
                None,
                &["host.read_invocation_context_v1"],
                &[
                    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json",
                ],
                "unsupported codecs remain a typed refusal instead of an implicit best-effort decode path.",
            ),
            case_spec(
                "capability_unmounted_host_error",
                "plugin.artifact_probe.input.v1",
                "bytes",
                TassadarPostArticlePluginPacketAbiExpectedStatus::ExactHostError,
                None,
                None,
                Some("capability_namespace_unmounted"),
                &[
                    "host.read_invocation_context_v1",
                    "host.request_capability_packet_v1",
                ],
                &[
                    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json",
                    "fixtures/tassadar/reports/tassadar_internal_component_abi_report.json",
                ],
                "unmounted capability namespaces stay in the explicit host-error channel instead of masquerading as a guest-owned typed refusal.",
            ),
        ],
    )
}

fn packet_field_spec(
    field_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginPacketFieldSpec {
    TassadarPostArticlePluginPacketFieldSpec {
        field_id: String::from(field_id),
        required: true,
        detail: String::from(detail),
    }
}

fn host_import_spec(
    import_id: &str,
    capability_scope: &str,
    deterministic_surface: bool,
    ambient_authority_allowed: bool,
    out_of_band_data_allowed: bool,
    receipt_required: bool,
    detail: &str,
) -> TassadarPostArticlePluginRustPdkHostImportSpec {
    TassadarPostArticlePluginRustPdkHostImportSpec {
        import_id: String::from(import_id),
        namespace_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID),
        capability_scope: String::from(capability_scope),
        deterministic_surface,
        ambient_authority_allowed,
        out_of_band_data_allowed,
        receipt_required,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn case_spec(
    case_id: &str,
    input_schema_id: &str,
    input_codec_id: &str,
    expected_status: TassadarPostArticlePluginPacketAbiExpectedStatus,
    expected_output_schema_id: Option<&str>,
    expected_refusal_id: Option<&str>,
    expected_host_error_id: Option<&str>,
    required_host_import_ids: &[&str],
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarPostArticlePluginPacketAbiCaseSpec {
    TassadarPostArticlePluginPacketAbiCaseSpec {
        case_id: String::from(case_id),
        input_schema_id: String::from(input_schema_id),
        input_codec_id: String::from(input_codec_id),
        expected_status,
        expected_output_schema_id: expected_output_schema_id.map(String::from),
        expected_refusal_id: expected_refusal_id.map(String::from),
        expected_host_error_id: expected_host_error_id.map(String::from),
        required_host_import_ids: required_host_import_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarPostArticlePluginPacketAbiExpectedStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION,
        TASSADAR_POST_ARTICLE_PLUGIN_REFUSAL_TYPE_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID,
        compile_tassadar_post_article_plugin_packet_abi_and_rust_pdk_contract,
    };

    #[test]
    fn post_article_plugin_packet_abi_contract_is_machine_legible() {
        let contract = compile_tassadar_post_article_plugin_packet_abi_and_rust_pdk_contract();

        assert_eq!(
            contract.packet_abi_version,
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION
        );
        assert_eq!(
            contract.rust_first_pdk_id,
            TASSADAR_POST_ARTICLE_PLUGIN_RUST_FIRST_PDK_ID
        );
        assert_eq!(contract.packet_field_specs.len(), 4);
        assert_eq!(contract.host_import_specs.len(), 3);
        assert_eq!(
            contract.guest_signature.host_import_namespace_id,
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID
        );
        assert_eq!(
            contract.guest_signature.refusal_type_id,
            TASSADAR_POST_ARTICLE_PLUGIN_REFUSAL_TYPE_ID
        );
        assert_eq!(contract.case_specs.len(), 5);
        assert!(contract.case_specs.iter().any(|case| {
            case.case_id == "capability_unmounted_host_error"
                && case.expected_status
                    == TassadarPostArticlePluginPacketAbiExpectedStatus::ExactHostError
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.case_id == "schema_invalid_typed_refusal"
                && case.expected_refusal_id.as_deref() == Some("schema_invalid")
        }));
    }
}
