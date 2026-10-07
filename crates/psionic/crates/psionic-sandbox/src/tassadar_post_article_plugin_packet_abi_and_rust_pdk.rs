use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{
    compile_tassadar_post_article_plugin_packet_abi_and_rust_pdk_contract,
    TassadarPostArticlePluginPacketAbiRustPdkCompilationContract,
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_REFUSAL_TYPE_ID,
};
use psionic_runtime::{
    build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle,
    TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
    TassadarPostArticlePluginPacketAbiCaseStatus,
    TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_packet_abi_and_rust_pdk_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-packet-abi-and-rust-pdk.sh";

const PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json";
const INTERNAL_COMPONENT_ABI_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_component_abi_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const POST_TAS_102_FINAL_AUDIT_REF: &str =
    "docs/audits/2026-03-18-tassadar-post-tas-102-final-audit.md";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginPacketAbiAndRustPdkStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginPacketAbiDependencyClass {
    ProofCarrying,
    RuntimePrecedent,
    DesignInput,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub plugin_manifest_report_id: String,
    pub plugin_manifest_report_digest: String,
    pub packet_abi_version: String,
    pub rust_first_pdk_id: String,
    pub compiler_contract_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginPacketAbiDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiRow {
    pub abi_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRustPdkRow {
    pub pdk_id: String,
    pub current_posture: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginPacketAbiAndRustPdkReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub plugin_manifest_identity_contract_report_ref: String,
    pub internal_component_abi_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub post_tas_102_final_audit_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginPacketAbiMachineIdentityBinding,
    pub compiler_contract: TassadarPostArticlePluginPacketAbiRustPdkCompilationContract,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
    pub dependency_rows: Vec<TassadarPostArticlePluginPacketAbiDependencyRow>,
    pub abi_rows: Vec<TassadarPostArticlePluginPacketAbiRow>,
    pub pdk_rows: Vec<TassadarPostArticlePluginRustPdkRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginPacketAbiValidationRow>,
    pub contract_status: TassadarPostArticlePluginPacketAbiAndRustPdkStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub packet_abi_frozen: bool,
    pub rust_first_pdk_frozen: bool,
    pub typed_refusal_channel_frozen: bool,
    pub explicit_host_error_channel_frozen: bool,
    pub explicit_receipt_channel_required: bool,
    pub narrow_host_import_surface_frozen: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginPacketAbiAndRustPdkReportError {
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

pub fn build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report() -> Result<
    TassadarPostArticlePluginPacketAbiAndRustPdkReport,
    TassadarPostArticlePluginPacketAbiAndRustPdkReportError,
> {
    let manifest: PluginManifestIdentityContractFixture =
        read_repo_json(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF)?;
    let internal_component_abi: InternalComponentAbiFixture =
        read_repo_json(INTERNAL_COMPONENT_ABI_REPORT_REF)?;
    let compiler_contract = compile_tassadar_post_article_plugin_packet_abi_and_rust_pdk_contract();
    let runtime_bundle = build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle();

    let operator_internal_only_posture = manifest.operator_internal_only_posture
        && manifest.deferred_issue_ids.is_empty()
        && !manifest.plugin_publication_allowed;
    let typed_refusal_channel_frozen = runtime_bundle
        .typed_refusals
        .iter()
        .any(|refusal| refusal.refusal_id == "schema_invalid")
        && runtime_bundle
            .typed_refusals
            .iter()
            .any(|refusal| refusal.refusal_id == "codec_unsupported")
        && runtime_bundle.case_receipts.iter().any(|case| {
            case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactTypedRefusal
        });
    let explicit_host_error_channel_frozen = runtime_bundle.host_error_channel_ids
        == vec![String::from("capability_namespace_unmounted")]
        && runtime_bundle.case_receipts.iter().any(|case| {
            case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactHostError
        });
    let explicit_receipt_channel_required = runtime_bundle
        .receipt_field_ids
        .iter()
        .any(|field| field == "invocation_identity_digest")
        && runtime_bundle
            .receipt_field_ids
            .iter()
            .any(|field| field == "input_packet_digest")
        && runtime_bundle
            .receipt_field_ids
            .iter()
            .any(|field| field == "output_packet_digest")
        && runtime_bundle
            .receipt_field_ids
            .iter()
            .any(|field| field == "typed_refusal_id")
        && runtime_bundle
            .receipt_field_ids
            .iter()
            .any(|field| field == "host_error_id");
    let narrow_host_import_surface_frozen = compiler_contract.host_import_specs.len() == 3
        && compiler_contract
            .host_import_specs
            .iter()
            .all(|import| !import.ambient_authority_allowed && !import.out_of_band_data_allowed);

    let machine_identity_binding = TassadarPostArticlePluginPacketAbiMachineIdentityBinding {
        machine_identity_id: manifest
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: manifest.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: manifest.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: manifest
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: manifest
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: manifest
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: manifest
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: manifest
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: manifest
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        plugin_manifest_report_id: manifest.report_id.clone(),
        plugin_manifest_report_digest: manifest.report_digest.clone(),
        packet_abi_version: compiler_contract.packet_abi_version.clone(),
        rust_first_pdk_id: compiler_contract.rust_first_pdk_id.clone(),
        compiler_contract_id: compiler_contract.contract_id.clone(),
        runtime_bundle_id: runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF,
        ),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` plugin_manifest_report_id=`{}` compiler_contract_id=`{}` and runtime_bundle_id=`{}` remain bound to `{}`.",
            manifest.machine_identity_binding.machine_identity_id,
            manifest.machine_identity_binding.canonical_route_id,
            manifest.report_id,
            compiler_contract.contract_id,
            runtime_bundle.bundle_id,
            CANONICAL_ARCHITECTURE_ANCHOR_CRATE,
        ),
    };

    let dependency_rows = build_dependency_rows(
        &manifest,
        &internal_component_abi,
        operator_internal_only_posture,
    );
    let abi_rows = build_abi_rows(
        &compiler_contract,
        &runtime_bundle,
        explicit_receipt_channel_required,
    );
    let pdk_rows = build_pdk_rows(
        &compiler_contract,
        &runtime_bundle,
        typed_refusal_channel_frozen,
        narrow_host_import_surface_frozen,
    );
    let packet_abi_frozen = abi_rows.iter().all(|row| row.green);
    let rust_first_pdk_frozen = pdk_rows.iter().all(|row| row.green);
    let rebase_claim_allowed = manifest.rebase_claim_allowed;
    let plugin_capability_claim_allowed = false;
    let weighted_plugin_control_allowed = false;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;
    let validation_rows = build_validation_rows(
        &manifest,
        &internal_component_abi,
        &compiler_contract,
        &runtime_bundle,
        typed_refusal_channel_frozen,
        explicit_host_error_channel_frozen,
        narrow_host_import_surface_frozen,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );
    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && packet_abi_frozen
        && rust_first_pdk_frozen
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture
        && typed_refusal_channel_frozen
        && explicit_host_error_channel_frozen
        && explicit_receipt_channel_required
        && narrow_host_import_surface_frozen
        && rebase_claim_allowed
        && !plugin_capability_claim_allowed
        && !weighted_plugin_control_allowed
        && !plugin_publication_allowed
        && !served_public_universality_allowed
        && !arbitrary_software_capability_allowed;
    let contract_status = if contract_green {
        TassadarPostArticlePluginPacketAbiAndRustPdkStatus::Green
    } else {
        TassadarPostArticlePluginPacketAbiAndRustPdkStatus::Incomplete
    };

    let mut report = TassadarPostArticlePluginPacketAbiAndRustPdkReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_packet_abi_and_rust_pdk.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_CHECKER_REF,
        ),
        plugin_manifest_identity_contract_report_ref: String::from(
            PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF,
        ),
        internal_component_abi_report_ref: String::from(INTERNAL_COMPONENT_ABI_REPORT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        post_tas_102_final_audit_ref: String::from(POST_TAS_102_FINAL_AUDIT_REF),
        supporting_material_refs: vec![
            String::from(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF),
            String::from(INTERNAL_COMPONENT_ABI_REPORT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            String::from(POST_TAS_102_FINAL_AUDIT_REF),
            String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        ],
        machine_identity_binding,
        compiler_contract,
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF,
        ),
        runtime_bundle,
        dependency_rows,
        abi_rows,
        pdk_rows,
        validation_rows,
        contract_status,
        contract_green,
        operator_internal_only_posture,
        packet_abi_frozen,
        rust_first_pdk_frozen,
        typed_refusal_channel_frozen,
        explicit_host_error_channel_frozen,
        explicit_receipt_channel_required,
        narrow_host_import_surface_frozen,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        deferred_issue_ids: Vec::new(),
        claim_boundary: String::from(
            "this report freezes the first post-article plugin packet ABI and Rust-first PDK above the rebased machine without widening the current claim surface. It defines a single packet-shaped invocation contract, explicit typed refusal and host-error channels, a host-owned receipt channel, and a narrow Rust-first guest import surface while keeping weighted plugin control, plugin publication, served/public universality, and arbitrary software capability blocked until the later runtime API, engine-abstraction, and controller tranches land.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "post-article plugin packet ABI report binds machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, abi_rows={}, pdk_rows={}, validation_rows={}, and deferred_issue_ids={}.",
        report.machine_identity_binding.machine_identity_id,
        report.machine_identity_binding.canonical_route_id,
        report.contract_status,
        report.abi_rows.len(),
        report.pdk_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report|",
        &report,
    );
    Ok(report)
}

fn build_dependency_rows(
    manifest: &PluginManifestIdentityContractFixture,
    internal_component_abi: &InternalComponentAbiFixture,
    operator_internal_only_posture: bool,
) -> Vec<TassadarPostArticlePluginPacketAbiDependencyRow> {
    vec![
        TassadarPostArticlePluginPacketAbiDependencyRow {
            dependency_id: String::from("plugin_manifest_identity_contract"),
            dependency_class: TassadarPostArticlePluginPacketAbiDependencyClass::ProofCarrying,
            satisfied: manifest.contract_green
                && manifest.deferred_issue_ids.is_empty()
                && operator_internal_only_posture
                && manifest.manifest_fields_frozen
                && manifest.canonical_invocation_identity_frozen
                && manifest.hot_swap_rules_frozen,
            source_ref: String::from(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF),
            bound_report_id: Some(manifest.report_id.clone()),
            bound_report_digest: Some(manifest.report_digest.clone()),
            detail: String::from(
                "the packet ABI inherits the closed manifest contract so packet identity and hot-swap posture are not redefined in the runtime layer.",
            ),
        },
        TassadarPostArticlePluginPacketAbiDependencyRow {
            dependency_id: String::from("internal_component_abi_precedent"),
            dependency_class: TassadarPostArticlePluginPacketAbiDependencyClass::RuntimePrecedent,
            satisfied: internal_component_abi.overall_green
                && !internal_component_abi.served_publication_allowed
                && internal_component_abi.green_component_graph_ids.len() == 3
                && internal_component_abi.interface_manifest_case_ids.len() == 3,
            source_ref: String::from(INTERNAL_COMPONENT_ABI_REPORT_REF),
            bound_report_id: Some(internal_component_abi.report_id.clone()),
            bound_report_digest: Some(internal_component_abi.report_digest.clone()),
            detail: String::from(
                "the earlier internal component-ABI lane remains the narrow precedent for explicit interface, refusal, and publication discipline rather than arbitrary plugin composition.",
            ),
        },
        TassadarPostArticlePluginPacketAbiDependencyRow {
            dependency_id: String::from("local_plugin_system_spec"),
            dependency_class: TassadarPostArticlePluginPacketAbiDependencyClass::DesignInput,
            satisfied: true,
            source_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the local plugin-system draft defines the packet ABI, refusal channel, and Rust-first guest-authoring direction this report freezes.",
            ),
        },
        TassadarPostArticlePluginPacketAbiDependencyRow {
            dependency_id: String::from("post_tas_102_final_audit"),
            dependency_class: TassadarPostArticlePluginPacketAbiDependencyClass::ObservationalContext,
            satisfied: true,
            source_ref: String::from(POST_TAS_102_FINAL_AUDIT_REF),
            bound_report_id: None,
            bound_report_digest: None,
            detail: String::from(
                "the broader internal-compute audit remains the disclosure-safe posture reminder that named ABI surfaces do not imply public broad-family closure.",
            ),
        },
    ]
}

fn build_abi_rows(
    compiler_contract: &TassadarPostArticlePluginPacketAbiRustPdkCompilationContract,
    runtime_bundle: &TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
    explicit_receipt_channel_required: bool,
) -> Vec<TassadarPostArticlePluginPacketAbiRow> {
    vec![
        abi_row(
            "single_input_packet_required",
            "one_packet_input_per_invocation",
            runtime_bundle
                .case_receipts
                .iter()
                .all(|case| !case.input_packet.schema_id.is_empty()),
            vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            "every invocation stays packet-shaped with one explicit input packet instead of ambient guest arguments.",
        ),
        abi_row(
            "single_output_packet_or_typed_refusal_required",
            "one_of_output_refusal_or_host_error",
            runtime_bundle.case_receipts.iter().all(|case| {
                usize::from(case.output_packet.is_some())
                    + usize::from(case.typed_refusal_id.is_some())
                    + usize::from(case.host_error_id.is_some())
                    == 1
            }),
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "each invocation ends in one output packet, one typed refusal, or one host error, never an ambiguous mixed channel.",
        ),
        abi_row(
            "explicit_host_error_channel_required",
            "host_error_channel_separate_from_guest_refusal",
            runtime_bundle.host_error_channel_ids == vec![String::from("capability_namespace_unmounted")],
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "host runtime failures stay on one explicit host-error channel rather than masquerading as guest-authored refusals.",
        ),
        abi_row(
            "explicit_receipt_channel_required",
            "host_receipt_fields_required",
            explicit_receipt_channel_required,
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "input, output, refusal, host-error, and invocation identity digests remain explicit host receipt fields.",
        ),
        abi_row(
            "schema_ids_required",
            "schema_ids_bound_on_every_packet",
            compiler_contract
                .packet_field_specs
                .iter()
                .any(|field| field.field_id == "schema_id")
                && runtime_bundle.case_receipts.iter().all(|case| {
                    !case.input_packet.schema_id.is_empty()
                        && case
                            .output_packet
                            .as_ref()
                            .map_or(true, |packet| !packet.schema_id.is_empty())
                }),
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF),
            ],
            "schema ids remain mandatory so packet shape drift fails closed.",
        ),
        abi_row(
            "codec_ids_required",
            "codec_ids_bound_on_every_packet",
            compiler_contract
                .packet_field_specs
                .iter()
                .any(|field| field.field_id == "codec_id")
                && runtime_bundle.case_receipts.iter().all(|case| {
                    !case.input_packet.codec_id.is_empty()
                        && case
                            .output_packet
                            .as_ref()
                            .map_or(true, |packet| !packet.codec_id.is_empty())
                }),
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF),
            ],
            "codec ids remain mandatory so payload framing is versioned and challengeable.",
        ),
        abi_row(
            "payload_bytes_core_form",
            "bytes_are_the_core_payload_carrier",
            compiler_contract
                .packet_field_specs
                .iter()
                .any(|field| field.field_id == "payload_bytes")
                && runtime_bundle.case_receipts.iter().all(|case| {
                    case.input_packet.payload_bytes_len > 0
                        && case
                            .output_packet
                            .as_ref()
                            .map_or(true, |packet| packet.payload_bytes_len > 0)
                }),
            vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            "payload bytes remain the core ABI carrier; typed schemas layer above bytes instead of replacing them.",
        ),
        abi_row(
            "metadata_envelope_shape_frozen",
            "metadata_envelope_optional_and_typed",
            compiler_contract
                .packet_field_specs
                .iter()
                .any(|field| field.field_id == "metadata_envelope")
                && runtime_bundle
                    .case_receipts
                    .iter()
                    .all(|case| !case.input_packet.metadata_field_ids.is_empty()),
            vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            "metadata remains an explicit envelope instead of a hidden side channel.",
        ),
    ]
}

fn abi_row(
    abi_id: &str,
    current_posture: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginPacketAbiRow {
    TassadarPostArticlePluginPacketAbiRow {
        abi_id: String::from(abi_id),
        current_posture: String::from(current_posture),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn build_pdk_rows(
    compiler_contract: &TassadarPostArticlePluginPacketAbiRustPdkCompilationContract,
    runtime_bundle: &TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
    typed_refusal_channel_frozen: bool,
    narrow_host_import_surface_frozen: bool,
) -> Vec<TassadarPostArticlePluginRustPdkRow> {
    vec![
        pdk_row(
            "rust_guest_crate_required",
            "rust_first_supported",
            compiler_contract.guest_signature.guest_authoring_posture == "rust_first_supported",
            vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            "the admitted guest-authoring path remains Rust-first rather than an open-ended guest-language promise.",
        ),
        pdk_row(
            "handle_packet_export_required",
            "single_handler_export_required",
            compiler_contract.guest_signature.handler_export == "handle_packet",
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "the guest surface remains one `handle_packet` export instead of a broad host callback API.",
        ),
        pdk_row(
            "explicit_plugin_refusal_type_required",
            "plugin_refusal_v1_required",
            compiler_contract.guest_signature.refusal_type_id
                == TASSADAR_POST_ARTICLE_PLUGIN_REFUSAL_TYPE_ID
                && typed_refusal_channel_frozen
                && runtime_bundle.typed_refusals.len() == 2,
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "guest-authored refusals remain typed and bounded instead of collapsing into opaque guest error strings.",
        ),
        pdk_row(
            "narrow_host_import_namespace_required",
            "single_packet_host_namespace",
            compiler_contract.guest_signature.host_import_namespace_id
                == TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID
                && narrow_host_import_surface_frozen,
            vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            "the guest may only touch one narrow packet-host namespace instead of a broad engine-specific SDK surface.",
        ),
        pdk_row(
            "no_ambient_authority_required",
            "ambient_authority_forbidden",
            compiler_contract.host_import_specs.iter().all(|import| {
                !import.ambient_authority_allowed && !import.out_of_band_data_allowed
            }),
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "all external guest-visible access remains explicit host imports with no ambient authority or hidden side channels.",
        ),
        pdk_row(
            "raw_wasm_admission_later_only",
            "later_profile_gated_admission",
            compiler_contract
                .guest_signature
                .raw_wasm_admission_posture
                .contains("later"),
            vec![String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF)],
            "raw Wasm admission remains a later profile-gated follow-on rather than part of the first Rust-first support promise.",
        ),
    ]
}

fn pdk_row(
    pdk_id: &str,
    current_posture: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginRustPdkRow {
    TassadarPostArticlePluginRustPdkRow {
        pdk_id: String::from(pdk_id),
        current_posture: String::from(current_posture),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    manifest: &PluginManifestIdentityContractFixture,
    internal_component_abi: &InternalComponentAbiFixture,
    compiler_contract: &TassadarPostArticlePluginPacketAbiRustPdkCompilationContract,
    runtime_bundle: &TassadarPostArticlePluginPacketAbiAndRustPdkRuntimeBundle,
    typed_refusal_channel_frozen: bool,
    explicit_host_error_channel_frozen: bool,
    narrow_host_import_surface_frozen: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticlePluginPacketAbiValidationRow> {
    let exact_typed_refusal_count = runtime_bundle
        .case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactTypedRefusal
        })
        .count();
    let exact_host_error_count = runtime_bundle
        .case_receipts
        .iter()
        .filter(|case| case.status == TassadarPostArticlePluginPacketAbiCaseStatus::ExactHostError)
        .count();
    vec![
        validation_row(
            "hidden_host_orchestration_blocked",
            manifest.canonical_invocation_identity_frozen
                && compiler_contract.guest_signature.handler_export == "handle_packet",
            vec![
                String::from(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            "the host may carry packet execution and receipt emission, but it may not absorb guest decision logic behind a broader API.",
        ),
        validation_row(
            "schema_drift_posture_blocked",
            typed_refusal_channel_frozen,
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "schema and codec drift remain typed refusals rather than implicit best-effort translation.",
        ),
        validation_row(
            "envelope_leakage_posture_blocked",
            internal_component_abi.overall_green
                && !internal_component_abi.served_publication_allowed
                && compiler_contract.host_import_specs.iter().all(|import| {
                    !import.ambient_authority_allowed
                        && import.namespace_id
                            == TASSADAR_POST_ARTICLE_PLUGIN_HOST_IMPORT_NAMESPACE_ID
                }),
            vec![
                String::from(INTERNAL_COMPONENT_ABI_REPORT_REF),
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            ],
            "mount-envelope authority stays explicit and capability-mediated instead of leaking ambient access into guest code.",
        ),
        validation_row(
            "side_channel_posture_blocked",
            compiler_contract
                .host_import_specs
                .iter()
                .all(|import| import.deterministic_surface && !import.out_of_band_data_allowed)
                && runtime_bundle
                    .receipt_field_ids
                    .iter()
                    .any(|field| field == "invocation_identity_digest"),
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "guest-visible host data remains explicit, deterministic, and receipt-bound instead of using hidden side channels.",
        ),
        validation_row(
            "overclaim_posture_blocked",
            rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !weighted_plugin_control_allowed
                && !plugin_publication_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![String::from(PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF)],
            "the packet ABI and Rust-first PDK do not themselves imply weighted plugin control, public plugin publication, served/public universality, or arbitrary software capability.",
        ),
        validation_row(
            "typed_fail_closed_posture_explicit",
            exact_typed_refusal_count == 2 && exact_host_error_count == 1,
            vec![String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF)],
            "typed refusals and host errors remain explicit fail-closed outcomes when schemas, codecs, or capability mounts do not line up.",
        ),
        validation_row(
            "rust_first_surface_narrow",
            narrow_host_import_surface_frozen
                && compiler_contract.packet_field_specs.len() == 4
                && compiler_contract.guest_signature.handler_export == "handle_packet",
            vec![
                String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF),
            ],
            "the first guest-authoring surface stays narrow enough to test and audit.",
        ),
        validation_row(
            "error_channel_separate_from_guest_refusal",
            explicit_host_error_channel_frozen
                && runtime_bundle.host_error_channel_ids.iter().all(|host_error| {
                    runtime_bundle
                        .typed_refusals
                        .iter()
                        .all(|refusal| refusal.refusal_id != *host_error)
                }),
            vec![String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_BUNDLE_REF)],
            "host-error channels remain separate from guest refusals instead of flattening engine/runtime failures into the guest taxonomy.",
        ),
    ]
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticlePluginPacketAbiValidationRow {
    TassadarPostArticlePluginPacketAbiValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_plugin_packet_abi_and_rust_pdk_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginPacketAbiAndRustPdkReport,
    TassadarPostArticlePluginPacketAbiAndRustPdkReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginPacketAbiAndRustPdkReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
        .expect("repo root should resolve from psionic-sandbox crate dir")
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginPacketAbiAndRustPdkReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PluginManifestIdentityContractFixture {
    report_id: String,
    report_digest: String,
    machine_identity_binding: PluginManifestMachineIdentityFixture,
    contract_green: bool,
    operator_internal_only_posture: bool,
    manifest_fields_frozen: bool,
    canonical_invocation_identity_frozen: bool,
    hot_swap_rules_frozen: bool,
    rebase_claim_allowed: bool,
    plugin_publication_allowed: bool,
    deferred_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PluginManifestMachineIdentityFixture {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    computational_model_statement_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct InternalComponentAbiFixture {
    report_id: String,
    report_digest: String,
    green_component_graph_ids: Vec<String>,
    interface_manifest_case_ids: Vec<String>,
    portability_envelope_ids: Vec<String>,
    served_publication_allowed: bool,
    overall_green: bool,
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginPacketAbiAndRustPdkReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginPacketAbiAndRustPdkReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report, read_json,
        tassadar_post_article_plugin_packet_abi_and_rust_pdk_report_path,
        write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report,
        TassadarPostArticlePluginPacketAbiAndRustPdkReport,
        TassadarPostArticlePluginPacketAbiAndRustPdkStatus,
    };

    #[test]
    fn post_article_plugin_packet_abi_report_freezes_contract_without_widening_claims() {
        let report =
            build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report().expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginPacketAbiAndRustPdkStatus::Green
        );
        assert!(report.contract_green);
        assert!(report.operator_internal_only_posture);
        assert!(report.packet_abi_frozen);
        assert!(report.rust_first_pdk_frozen);
        assert!(report.typed_refusal_channel_frozen);
        assert!(report.explicit_host_error_channel_frozen);
        assert!(report.explicit_receipt_channel_required);
        assert!(report.narrow_host_import_surface_frozen);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert_eq!(report.dependency_rows.len(), 4);
        assert_eq!(report.abi_rows.len(), 8);
        assert_eq!(report.pdk_rows.len(), 6);
        assert_eq!(report.validation_rows.len(), 8);
        assert!(report.deferred_issue_ids.is_empty());
    }

    #[test]
    fn post_article_plugin_packet_abi_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report().expect("report");
        let committed: TassadarPostArticlePluginPacketAbiAndRustPdkReport =
            read_json(tassadar_post_article_plugin_packet_abi_and_rust_pdk_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_post_article_plugin_packet_abi_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_packet_abi_and_rust_pdk_report.json");
        let written =
            write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report(&output_path)
                .expect("write report");
        let persisted: TassadarPostArticlePluginPacketAbiAndRustPdkReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(written, persisted);
    }
}
