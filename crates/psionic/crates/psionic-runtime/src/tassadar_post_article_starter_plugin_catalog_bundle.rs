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
    build_psion_plugin_guest_artifact_invocation_bundle,
    catalog_exposed_starter_plugin_registrations, starter_plugin_registration_by_plugin_id,
    StarterPluginCapabilityClass, StarterPluginRegistration,
    TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION,
};

pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/tassadar_post_article_starter_plugin_catalog_bundle.json";
pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1";

const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const AUTOPILOT_PORTING_NOTES_REF: &str =
    "~/code/alpha/autopilot/autopilot-extism-plugin-porting-into-rust-runtime.md";
const URL_EXTRACTOR_INVENTORY_REF: &str =
    "~/code/openagents-plugins/url-extractor-and-url-scraper.md";
const RSS_FEED_INVENTORY_REF: &str = "~/code/openagents-plugins/plugin-rss-feed/README.md";
const MULTI_PLUGIN_AUDIT_REF: &str =
    "docs/audits/2026-03-21-multi-plugin-real-run-orchestration-audit.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStarterPluginCapabilityClass {
    LocalDeterministic,
    ReadOnlyNetwork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleStarterPluginFixtureStatus {
    ExactSuccess,
    TypedMalformedPacket,
    TypedRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginPacketSchemaSet {
    pub input_schema_id: String,
    pub success_output_schema_id: String,
    pub refusal_schema_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginDescriptor {
    pub plugin_id: String,
    pub plugin_version: String,
    pub catalog_entry_id: String,
    pub manifest_id: String,
    pub artifact_id: String,
    pub artifact_digest: String,
    pub packet_abi_version: String,
    pub packet_schemas: TassadarPostArticleStarterPluginPacketSchemaSet,
    pub capability_class: TassadarPostArticleStarterPluginCapabilityClass,
    pub replay_class_id: String,
    pub trust_tier_id: String,
    pub evidence_posture_id: String,
    pub capability_namespace_ids: Vec<String>,
    pub negative_claim_ids: Vec<String>,
    pub descriptor_ref: String,
    pub fixture_bundle_ref: String,
    pub sample_mount_envelope_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginFixtureCase {
    pub case_id: String,
    pub status: TassadarPostArticleStarterPluginFixtureStatus,
    pub request_schema_id: String,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub replay_class_id: String,
    pub receipt_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_or_failure_class_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginFixtureBundle {
    pub plugin_id: String,
    pub plugin_version: String,
    pub fixture_bundle_id: String,
    pub packet_abi_version: String,
    pub success_case_ids: Vec<String>,
    pub malformed_case_ids: Vec<String>,
    pub refusal_case_ids: Vec<String>,
    pub negative_claim_ids: Vec<String>,
    pub cases: Vec<TassadarPostArticleStarterPluginFixtureCase>,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginMountEnvelope {
    pub envelope_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub world_mount_id: String,
    pub capability_namespace_ids: Vec<String>,
    pub allowlisted_url_prefixes: Vec<String>,
    pub allowed_method_ids: Vec<String>,
    pub timeout_millis: u32,
    pub response_size_limit_bytes: u64,
    pub redirect_limit: u8,
    pub replay_posture_id: String,
    pub green: bool,
    pub detail: String,
    pub envelope_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCapabilityMatrixRow {
    pub plugin_id: String,
    pub catalog_entry_id: String,
    pub capability_class: TassadarPostArticleStarterPluginCapabilityClass,
    pub reads_network: bool,
    pub deterministic_replayable: bool,
    pub snapshot_backed_replay: bool,
    pub filesystem_access: bool,
    pub secrets_access: bool,
    pub mount_required: bool,
    pub host_mediated_network_only: bool,
    pub schema_local_composition: bool,
    pub operator_only: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCompositionStepRow {
    pub step_index: u16,
    pub plugin_id: String,
    pub input_schema_id: String,
    pub output_schema_id: String,
    pub mount_envelope_id: String,
    pub receipt_id: String,
    pub replay_class_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCompositionCaseRow {
    pub case_id: String,
    pub flow_class_id: String,
    pub step_plugin_ids: Vec<String>,
    pub step_receipt_ids: Vec<String>,
    pub hidden_host_orchestration_allowed: bool,
    pub schema_repair_allowed: bool,
    pub capability_leakage_allowed: bool,
    pub green: bool,
    pub step_rows: Vec<TassadarPostArticleStarterPluginCompositionStepRow>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleStarterPluginCatalogBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub packet_abi_version: String,
    pub supporting_material_refs: Vec<String>,
    pub descriptor_refs: Vec<String>,
    pub fixture_bundle_refs: Vec<String>,
    pub sample_mount_envelope_refs: Vec<String>,
    pub descriptor_rows: Vec<TassadarPostArticleStarterPluginDescriptor>,
    pub capability_matrix_rows: Vec<TassadarPostArticleStarterPluginCapabilityMatrixRow>,
    pub composition_case_rows: Vec<TassadarPostArticleStarterPluginCompositionCaseRow>,
    pub plugin_count: u32,
    pub local_deterministic_plugin_count: u32,
    pub read_only_network_plugin_count: u32,
    pub bounded_flow_count: u32,
    pub operator_only_posture: bool,
    pub runtime_builtins_separate: bool,
    pub public_marketplace_implication_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleStarterPluginCatalogBundleError {
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

struct TassadarPostArticleStarterPluginCatalogArtifacts {
    bundle: TassadarPostArticleStarterPluginCatalogBundle,
    descriptors: Vec<TassadarPostArticleStarterPluginDescriptor>,
    fixture_bundles: Vec<TassadarPostArticleStarterPluginFixtureBundle>,
    mount_envelopes: Vec<TassadarPostArticleStarterPluginMountEnvelope>,
}

#[must_use]
pub fn build_tassadar_post_article_starter_plugin_catalog_bundle(
) -> TassadarPostArticleStarterPluginCatalogBundle {
    build_tassadar_post_article_starter_plugin_catalog_artifacts().bundle
}

fn catalog_registration(plugin_id: &str) -> &'static StarterPluginRegistration {
    let registration = starter_plugin_registration_by_plugin_id(plugin_id)
        .unwrap_or_else(|| panic!("missing starter-plugin registration for `{plugin_id}`"));
    assert!(
        registration.catalog_exposed && registration.catalog.is_some(),
        "starter-plugin `{plugin_id}` is not catalog-exposed"
    );
    registration
}

fn catalog_capability_class(
    capability_class: StarterPluginCapabilityClass,
) -> TassadarPostArticleStarterPluginCapabilityClass {
    match capability_class {
        StarterPluginCapabilityClass::LocalDeterministic => {
            TassadarPostArticleStarterPluginCapabilityClass::LocalDeterministic
        }
        StarterPluginCapabilityClass::ReadOnlyNetwork => {
            TassadarPostArticleStarterPluginCapabilityClass::ReadOnlyNetwork
        }
    }
}

fn starter_plugin_descriptor_from_registration(
    registration: &StarterPluginRegistration,
) -> TassadarPostArticleStarterPluginDescriptor {
    let catalog = registration
        .catalog
        .unwrap_or_else(|| panic!("missing catalog metadata for `{}`", registration.plugin_id));
    starter_plugin_descriptor(
        registration.plugin_id,
        registration.manifest_id,
        registration.artifact_id,
        catalog_capability_class(registration.capability_class),
        registration.replay_class_id,
        catalog.trust_tier_id,
        catalog.evidence_posture_id,
        registration.input_schema_id,
        registration.success_output_schema_id,
        registration.refusal_schema_ids,
        catalog.catalog_capability_namespace_ids,
        registration.negative_claim_ids,
        catalog.descriptor_ref,
        catalog.fixture_bundle_ref,
        catalog.sample_mount_envelope_ref,
        catalog.descriptor_detail,
    )
}

fn starter_fixture_bundle_from_registration(
    registration: &StarterPluginRegistration,
    cases: &[TassadarPostArticleStarterPluginFixtureCase],
) -> TassadarPostArticleStarterPluginFixtureBundle {
    starter_fixture_bundle(
        registration.plugin_id,
        cases,
        registration.negative_claim_ids,
    )
}

fn guest_artifact_fixture_bundle_from_registration(
    registration: &StarterPluginRegistration,
) -> TassadarPostArticleStarterPluginFixtureBundle {
    let invocation_bundle = build_psion_plugin_guest_artifact_invocation_bundle();
    let mut cases = vec![starter_fixture_case(
        invocation_bundle.success_case.case_id.as_str(),
        TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
        registration.input_schema_id,
        &serde_json::json!({
            "text": "guest-artifact echo proof"
        }),
        invocation_bundle
            .success_case
            .response_or_refusal_schema_id
            .as_str(),
        &invocation_bundle.success_case.projected_result.structured_payload,
        invocation_bundle.success_case.replay_class_id.as_str(),
        invocation_bundle
            .success_case
            .projected_result
            .plugin_receipt
            .receipt_id
            .as_str(),
        None,
        "the first digest-bound guest-artifact starter plugin now contributes real invocation evidence to the starter catalog instead of a synthetic host-native stand-in.",
    )];
    for refusal_case in &invocation_bundle.refusal_cases {
        let refusal_class_id = refusal_case
            .projected_result
            .plugin_receipt
            .refusal_class_id
            .as_deref();
        let status = match refusal_class_id {
            Some("schema_invalid") => {
                TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket
            }
            _ => TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
        };
        let request = match refusal_case.case_id.as_str() {
            "guest_artifact_schema_invalid" => serde_json::json!({
                "message": "wrong field"
            }),
            "guest_artifact_packet_too_large" => serde_json::json!({
                "text_digest": "input.exceeds.guest_artifact.packet_limit.v1"
            }),
            _ => serde_json::json!({
                "text": "guest-artifact echo proof"
            }),
        };
        cases.push(starter_fixture_case(
            refusal_case.case_id.as_str(),
            status,
            registration.input_schema_id,
            &request,
            refusal_case.response_or_refusal_schema_id.as_str(),
            &refusal_case.projected_result.structured_payload,
            refusal_case.replay_class_id.as_str(),
            refusal_case
                .projected_result
                .plugin_receipt
                .receipt_id
                .as_str(),
            refusal_class_id,
            refusal_case.detail.as_str(),
        ));
    }
    starter_fixture_bundle_from_registration(registration, cases.as_slice())
}

fn starter_mount_envelope_from_registration(
    registration: &StarterPluginRegistration,
    world_mount_id: &str,
    allowlisted_url_prefixes: &[&str],
    allowed_method_ids: &[&str],
    timeout_millis: u32,
    response_size_limit_bytes: u64,
    redirect_limit: u8,
    replay_posture_id: &str,
    detail: &str,
) -> TassadarPostArticleStarterPluginMountEnvelope {
    let catalog = registration
        .catalog
        .unwrap_or_else(|| panic!("missing catalog metadata for `{}`", registration.plugin_id));
    starter_mount_envelope(
        registration.mount_envelope_id,
        registration.plugin_id,
        world_mount_id,
        catalog.catalog_capability_namespace_ids,
        allowlisted_url_prefixes,
        allowed_method_ids,
        timeout_millis,
        response_size_limit_bytes,
        redirect_limit,
        replay_posture_id,
        detail,
    )
}

fn capability_matrix_row_from_registration(
    registration: &StarterPluginRegistration,
    reads_network: bool,
    deterministic_replayable: bool,
    snapshot_backed_replay: bool,
    filesystem_access: bool,
    secrets_access: bool,
    mount_required: bool,
    host_mediated_network_only: bool,
    schema_local_composition: bool,
    operator_only: bool,
) -> TassadarPostArticleStarterPluginCapabilityMatrixRow {
    let catalog = registration
        .catalog
        .unwrap_or_else(|| panic!("missing catalog metadata for `{}`", registration.plugin_id));
    capability_matrix_row(
        registration.plugin_id,
        catalog.catalog_entry_id,
        catalog_capability_class(registration.capability_class),
        reads_network,
        deterministic_replayable,
        snapshot_backed_replay,
        filesystem_access,
        secrets_access,
        mount_required,
        host_mediated_network_only,
        schema_local_composition,
        operator_only,
        catalog.capability_matrix_detail,
    )
}

fn build_tassadar_post_article_starter_plugin_catalog_artifacts(
) -> TassadarPostArticleStarterPluginCatalogArtifacts {
    let url_extract = catalog_registration("plugin.text.url_extract");
    let text_stats = catalog_registration("plugin.text.stats");
    let fetch_text = catalog_registration("plugin.http.fetch_text");
    let guest_echo = catalog_registration("plugin.example.echo_guest");
    let extract_readable = catalog_registration("plugin.html.extract_readable");
    let feed_parse = catalog_registration("plugin.feed.rss_atom_parse");

    let descriptors = catalog_exposed_starter_plugin_registrations()
        .into_iter()
        .map(starter_plugin_descriptor_from_registration)
        .collect::<Vec<_>>();

    let fixture_bundles = vec![
        starter_fixture_bundle_from_registration(
            url_extract,
            &[
                starter_fixture_case(
                    "extract_urls_success",
                    TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
                    "plugin.text.url_extract.input.v1",
                    &serde_json::json!({
                        "text": "notes: https://example.com/articles/one then http://example.org/two"
                    }),
                    "plugin.text.url_extract.output.v1",
                    &serde_json::json!({
                        "urls": [
                            "https://example.com/articles/one",
                            "http://example.org/two"
                        ]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.url_extract.extract_urls_success.v1",
                    None,
                    "url extraction keeps left-to-right order, preserves duplicates posture, and stays bounded to literal http(s) string matches.",
                ),
                starter_fixture_case(
                    "schema_invalid_missing_text",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket,
                    "plugin.text.url_extract.input.v1",
                    &serde_json::json!({
                        "body": "missing the required field"
                    }),
                    "plugin.refusal.schema_invalid.v1",
                    &serde_json::json!({
                        "reason_id": "schema_invalid",
                        "missing_field_ids": ["text"]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.url_extract.schema_invalid_missing_text.v1",
                    Some("schema_invalid"),
                    "malformed packets fail closed into one typed schema-invalid refusal instead of guest panic or host repair.",
                ),
                starter_fixture_case(
                    "packet_too_large_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.text.url_extract.input.v1",
                    &serde_json::json!({
                        "text_digest": "input.exceeds.url_extract.packet_limit.v1"
                    }),
                    "plugin.refusal.packet_too_large.v1",
                    &serde_json::json!({
                        "reason_id": "packet_too_large",
                        "ceiling_bytes": 16384
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.url_extract.packet_too_large_refusal.v1",
                    Some("packet_too_large"),
                    "the deterministic plugin keeps packet ceilings explicit rather than relying on ambient parser allocation behavior.",
                ),
            ],
        ),
        starter_fixture_bundle_from_registration(
            text_stats,
            &[
                starter_fixture_case(
                    "text_stats_success",
                    TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
                    "plugin.text.stats.input.v1",
                    &serde_json::json!({
                        "text": "alpha beta\n\ngamma delta"
                    }),
                    "plugin.text.stats.output.v1",
                    &serde_json::json!({
                        "byte_count": 23,
                        "unicode_scalar_count": 23,
                        "line_count": 3,
                        "non_empty_line_count": 2,
                        "word_count": 4
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.stats.text_stats_success.v1",
                    None,
                    "the bounded user-added starter plugin publishes explicit packet-local counting truth without tokenizer or semantic claims.",
                ),
                starter_fixture_case(
                    "schema_invalid_missing_text",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket,
                    "plugin.text.stats.input.v1",
                    &serde_json::json!({
                        "body": "missing the required field"
                    }),
                    "plugin.refusal.schema_invalid.v1",
                    &serde_json::json!({
                        "reason_id": "schema_invalid",
                        "missing_field_ids": ["text"]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.stats.schema_invalid_missing_text.v1",
                    Some("schema_invalid"),
                    "missing text stays a typed malformed-packet refusal instead of ambient host repair.",
                ),
                starter_fixture_case(
                    "packet_too_large_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.text.stats.input.v1",
                    &serde_json::json!({
                        "text_digest": "input.exceeds.text_stats.packet_limit.v1"
                    }),
                    "plugin.refusal.packet_too_large.v1",
                    &serde_json::json!({
                        "reason_id": "packet_too_large",
                        "ceiling_bytes": 16384
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.stats.packet_too_large_refusal.v1",
                    Some("packet_too_large"),
                    "packet ceilings remain typed and explicit for the user-added plugin path as well.",
                ),
                starter_fixture_case(
                    "unsupported_codec_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.text.stats.input.v1",
                    &serde_json::json!({
                        "text": "alpha beta"
                    }),
                    "plugin.refusal.unsupported_codec.v1",
                    &serde_json::json!({
                        "reason_id": "unsupported_codec",
                        "supported_codec_ids": ["json"]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.text.stats.unsupported_codec_refusal.v1",
                    Some("unsupported_codec"),
                    "unsupported codecs remain typed refusals instead of hidden alternate decoding paths.",
                ),
            ],
        ),
        starter_fixture_bundle_from_registration(
            fetch_text,
            &[
                starter_fixture_case(
                    "fetch_text_article_success",
                    TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
                    "plugin.http.fetch_text.input.v1",
                    &serde_json::json!({
                        "url": "https://example.com/articles/one"
                    }),
                    "plugin.http.fetch_text.output.v1",
                    &serde_json::json!({
                        "final_url": "https://example.com/articles/one",
                        "status_code": 200,
                        "content_type": "text/html",
                        "charset": "utf-8",
                        "body_text": "<html><title>Example</title><body><article><p>Alpha page.</p></article><a href=\"https://example.com/about\">About</a></body></html>",
                        "truncated": false
                    }),
                    "replayable_with_snapshots",
                    "receipt.plugin.http.fetch_text.fetch_text_article_success.v1",
                    None,
                    "success receipts keep final URL, content type, charset, and truncation posture explicit for read-only network fetches.",
                ),
                starter_fixture_case(
                    "schema_invalid_missing_url",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket,
                    "plugin.http.fetch_text.input.v1",
                    &serde_json::json!({
                        "href": "https://example.com/articles/one"
                    }),
                    "plugin.refusal.schema_invalid.v1",
                    &serde_json::json!({
                        "reason_id": "schema_invalid",
                        "missing_field_ids": ["url"]
                    }),
                    "replayable_with_snapshots",
                    "receipt.plugin.http.fetch_text.schema_invalid_missing_url.v1",
                    Some("schema_invalid"),
                    "fetch-text packet validation fails closed before any capability-bound HTTP call is attempted.",
                ),
                starter_fixture_case(
                    "url_not_permitted_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.http.fetch_text.input.v1",
                    &serde_json::json!({
                        "url": "https://not-allowlisted.example/blocked"
                    }),
                    "plugin.refusal.url_not_permitted.v1",
                    &serde_json::json!({
                        "reason_id": "url_not_permitted",
                        "matched_allowlist": false
                    }),
                    "replayable_with_snapshots",
                    "receipt.plugin.http.fetch_text.url_not_permitted_refusal.v1",
                    Some("url_not_permitted"),
                    "allowlist refusal keeps the network policy in the mount envelope rather than silently widening to ambient host HTTP.",
                ),
                starter_fixture_case(
                    "timeout_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.http.fetch_text.input.v1",
                    &serde_json::json!({
                        "url": "https://feeds.example.com/slow.xml"
                    }),
                    "plugin.refusal.timeout.v1",
                    &serde_json::json!({
                        "reason_id": "timeout",
                        "timeout_millis": 2500
                    }),
                    "replayable_with_snapshots",
                    "receipt.plugin.http.fetch_text.timeout_refusal.v1",
                    Some("timeout"),
                    "network timeout remains a typed refusal or failure surface and never degenerates into hidden retry or host-side repair.",
                ),
            ],
        ),
        guest_artifact_fixture_bundle_from_registration(guest_echo),
        starter_fixture_bundle_from_registration(
            extract_readable,
            &[
                starter_fixture_case(
                    "extract_readable_success",
                    TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
                    "plugin.html.extract_readable.input.v1",
                    &serde_json::json!({
                        "source_url": "https://example.com/articles/one",
                        "content_type": "text/html",
                        "body_text": "<html><head><title>Example</title></head><body><article><p>Alpha page.</p></article><a href=\"https://example.com/about\">About</a></body></html>"
                    }),
                    "plugin.html.extract_readable.output.v1",
                    &serde_json::json!({
                        "title": "Example",
                        "canonical_url": "https://example.com/articles/one",
                        "site_name": "example.com",
                        "excerpt": "Alpha page.",
                        "readable_text": "Alpha page.",
                        "harvested_links": ["https://example.com/about"],
                        "content_language": "en"
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.html.extract_readable.extract_readable_success.v1",
                    None,
                    "the local readability extractor keeps title, excerpt, readable text, and harvested links typed and replayable.",
                ),
                starter_fixture_case(
                    "schema_invalid_missing_body_text",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket,
                    "plugin.html.extract_readable.input.v1",
                    &serde_json::json!({
                        "source_url": "https://example.com/articles/one",
                        "content_type": "text/html"
                    }),
                    "plugin.refusal.schema_invalid.v1",
                    &serde_json::json!({
                        "reason_id": "schema_invalid",
                        "missing_field_ids": ["body_text"]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.html.extract_readable.schema_invalid_missing_body_text.v1",
                    Some("schema_invalid"),
                    "missing fetched document content becomes a typed malformed-packet refusal rather than hidden host-side extraction.",
                ),
                starter_fixture_case(
                    "content_type_unsupported_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.html.extract_readable.input.v1",
                    &serde_json::json!({
                        "source_url": "https://example.com/manual.pdf",
                        "content_type": "application/pdf",
                        "body_text": "%PDF-1.7..."
                    }),
                    "plugin.refusal.content_type_unsupported.v1",
                    &serde_json::json!({
                        "reason_id": "content_type_unsupported",
                        "content_type": "application/pdf"
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.html.extract_readable.content_type_unsupported_refusal.v1",
                    Some("content_type_unsupported"),
                    "non-HTML content stays outside the bounded extractor claim surface.",
                ),
            ],
        ),
        starter_fixture_bundle_from_registration(
            feed_parse,
            &[
                starter_fixture_case(
                    "rss_parse_success",
                    TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
                    "plugin.feed.rss_atom_parse.input.v1",
                    &serde_json::json!({
                        "source_url": "https://feeds.example.com/example.xml",
                        "content_type": "application/rss+xml",
                        "feed_text": "<rss version=\"2.0\"><channel><title>Example Feed</title><link>https://example.com/</link><description>Updates</description><item><title>Post One</title><link>https://example.com/post-1</link><description>Hello</description></item></channel></rss>"
                    }),
                    "plugin.feed.rss_atom_parse.output.v1",
                    &serde_json::json!({
                        "feed_title": "Example Feed",
                        "feed_homepage_url": "https://example.com/",
                        "feed_description": "Updates",
                        "entries": [
                            {
                                "title": "Post One",
                                "link": "https://example.com/post-1",
                                "published_time": null,
                                "summary": "Hello",
                                "content_excerpt": "Hello"
                            }
                        ]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.feed.rss_atom_parse.rss_parse_success.v1",
                    None,
                    "rss v2 parsing stays deterministic and packet-local over already-fetched content.",
                ),
                starter_fixture_case(
                    "atom_parse_success",
                    TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess,
                    "plugin.feed.rss_atom_parse.input.v1",
                    &serde_json::json!({
                        "source_url": "https://feeds.example.com/atom.xml",
                        "content_type": "application/atom+xml",
                        "feed_text": "<feed xmlns=\"http://www.w3.org/2005/Atom\"><title>Example Atom</title><link href=\"https://example.com/\"/><entry><title>Atom Post</title><link href=\"https://example.com/atom-post\"/><summary>World</summary></entry></feed>"
                    }),
                    "plugin.feed.rss_atom_parse.output.v1",
                    &serde_json::json!({
                        "feed_title": "Example Atom",
                        "feed_homepage_url": "https://example.com/",
                        "feed_description": null,
                        "entries": [
                            {
                                "title": "Atom Post",
                                "link": "https://example.com/atom-post",
                                "published_time": null,
                                "summary": "World",
                                "content_excerpt": "World"
                            }
                        ]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.feed.rss_atom_parse.atom_parse_success.v1",
                    None,
                    "atom parsing stays inside the same deterministic bounded format window.",
                ),
                starter_fixture_case(
                    "schema_invalid_missing_feed_text",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket,
                    "plugin.feed.rss_atom_parse.input.v1",
                    &serde_json::json!({
                        "source_url": "https://feeds.example.com/example.xml",
                        "content_type": "application/rss+xml"
                    }),
                    "plugin.refusal.schema_invalid.v1",
                    &serde_json::json!({
                        "reason_id": "schema_invalid",
                        "missing_field_ids": ["feed_text"]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.feed.rss_atom_parse.schema_invalid_missing_feed_text.v1",
                    Some("schema_invalid"),
                    "missing feed text remains a typed packet refusal instead of host-hidden fetch or parse fallback.",
                ),
                starter_fixture_case(
                    "unsupported_feed_format_refusal",
                    TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal,
                    "plugin.feed.rss_atom_parse.input.v1",
                    &serde_json::json!({
                        "source_url": "https://feeds.example.com/opml.xml",
                        "content_type": "text/xml",
                        "feed_text": "<opml version=\"2.0\"></opml>"
                    }),
                    "plugin.refusal.unsupported_feed_format.v1",
                    &serde_json::json!({
                        "reason_id": "unsupported_feed_format",
                        "supported_format_ids": ["rss2", "atom1"]
                    }),
                    "deterministic_replayable",
                    "receipt.plugin.feed.rss_atom_parse.unsupported_feed_format_refusal.v1",
                    Some("unsupported_feed_format"),
                    "opml and arbitrary xml stay outside the starter feed parser boundary.",
                ),
            ],
        ),
    ];

    let mount_envelopes = vec![
        starter_mount_envelope_from_registration(
            url_extract,
            "world_mount.starter.local_no_capabilities.v1",
            &[],
            &[],
            1000,
            0,
            0,
            "deterministic_replayable",
            "the URL extractor proves that the starter catalog can ship a capability-free deterministic plugin with an explicit zero-namespace mount envelope.",
        ),
        starter_mount_envelope_from_registration(
            text_stats,
            "world_mount.starter.local_no_capabilities.v1",
            &[],
            &[],
            1000,
            0,
            0,
            "deterministic_replayable",
            "the user-added text-stats starter plugin remains capability-free under the same explicit no-import mount class as the built-in local deterministic entries.",
        ),
        starter_mount_envelope_from_registration(
            fetch_text,
            "world_mount.starter.http_read_only_text_allowlist.v1",
            &[
                "https://example.com/articles/",
                "https://feeds.example.com/",
            ],
            &["GET"],
            2500,
            131072,
            2,
            "replayable_with_snapshots",
            "the fetch-text starter plugin binds allowlist, timeout, response-size, and redirect posture to one explicit host-mediated mount envelope.",
        ),
        starter_mount_envelope_from_registration(
            guest_echo,
            "world_mount.starter.guest_artifact_digest_bound.v1",
            &[],
            &[],
            1000,
            4096,
            0,
            "guest_artifact_digest_replay_only.v1",
            "the first guest-artifact starter plugin binds one explicit no-capability, digest-bound, packet-size-limited mount envelope instead of inheriting host-native policy by accident.",
        ),
        starter_mount_envelope_from_registration(
            extract_readable,
            "world_mount.starter.local_no_capabilities.v1",
            &[],
            &[],
            1000,
            0,
            0,
            "deterministic_replayable",
            "the readability extractor stays local and capability-free under one explicit no-import mount envelope.",
        ),
        starter_mount_envelope_from_registration(
            feed_parse,
            "world_mount.starter.local_no_capabilities.v1",
            &[],
            &[],
            1000,
            0,
            0,
            "deterministic_replayable",
            "the feed parser stays local and capability-free because fetching remains outside the plugin family.",
        ),
    ];

    let capability_matrix_rows = vec![
        capability_matrix_row_from_registration(
            url_extract,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
        ),
        capability_matrix_row_from_registration(
            text_stats, false, true, false, false, false, false, false, true, true,
        ),
        capability_matrix_row_from_registration(
            fetch_text, true, false, true, false, false, true, true, true, true,
        ),
        capability_matrix_row_from_registration(
            guest_echo, false, true, false, false, false, true, false, true, true,
        ),
        capability_matrix_row_from_registration(
            extract_readable,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            true,
        ),
        capability_matrix_row_from_registration(
            feed_parse, false, true, false, false, false, false, false, true, true,
        ),
    ];

    let composition_case_rows = vec![
        composition_case(
            "discover_fetch_extract_page",
            "starter_flow.url_discovery_fetch_extract.v1",
            vec![
                composition_step(
                    0,
                    "plugin.text.url_extract",
                    "plugin.text.url_extract.input.v1",
                    "plugin.text.url_extract.output.v1",
                    "mount.plugin.text.url_extract.no_capabilities.v1",
                    "receipt.plugin.text.url_extract.extract_urls_success.v1",
                    "deterministic_replayable",
                    "discover candidate URLs from unstructured text without hidden host preprocessing.",
                ),
                composition_step(
                    1,
                    "plugin.http.fetch_text",
                    "plugin.http.fetch_text.input.v1",
                    "plugin.http.fetch_text.output.v1",
                    "mount.plugin.http.fetch_text.read_only_http_allowlist.v1",
                    "receipt.plugin.http.fetch_text.fetch_text_article_success.v1",
                    "replayable_with_snapshots",
                    "fetch the allowlisted URL through the bounded read-only network mount.",
                ),
                composition_step(
                    2,
                    "plugin.html.extract_readable",
                    "plugin.html.extract_readable.input.v1",
                    "plugin.html.extract_readable.output.v1",
                    "mount.plugin.html.extract_readable.no_capabilities.v1",
                    "receipt.plugin.html.extract_readable.extract_readable_success.v1",
                    "deterministic_replayable",
                    "extract readable text and harvested links without browser or host-hidden readability glue.",
                ),
            ],
            "the first starter flow proves that URL discovery, bounded fetch, and readability extraction can compose without hidden host orchestration or schema repair.",
        ),
        composition_case(
            "fetch_feed_then_parse",
            "starter_flow.fetch_feed_parse.v1",
            vec![
                composition_step(
                    0,
                    "plugin.http.fetch_text",
                    "plugin.http.fetch_text.input.v1",
                    "plugin.http.fetch_text.output.v1",
                    "mount.plugin.http.fetch_text.read_only_http_allowlist.v1",
                    "receipt.plugin.http.fetch_text.fetch_text_article_success.v1",
                    "replayable_with_snapshots",
                    "fetch the feed document through the same bounded read-only network mount.",
                ),
                composition_step(
                    1,
                    "plugin.feed.rss_atom_parse",
                    "plugin.feed.rss_atom_parse.input.v1",
                    "plugin.feed.rss_atom_parse.output.v1",
                    "mount.plugin.feed.rss_atom_parse.no_capabilities.v1",
                    "receipt.plugin.feed.rss_atom_parse.rss_parse_success.v1",
                    "deterministic_replayable",
                    "parse feed metadata and entries locally without host-hidden XML interpretation.",
                ),
            ],
            "the second starter flow proves that fetched feed content can enter a deterministic local parser without hidden schema repair or host-side feed parsing.",
        ),
    ];

    let descriptor_refs = descriptors
        .iter()
        .map(|row| row.descriptor_ref.clone())
        .collect::<Vec<_>>();
    let fixture_bundle_refs = descriptors
        .iter()
        .map(|row| row.fixture_bundle_ref.clone())
        .collect::<Vec<_>>();
    let sample_mount_envelope_refs = descriptors
        .iter()
        .map(|row| row.sample_mount_envelope_ref.clone())
        .collect::<Vec<_>>();

    let local_deterministic_plugin_count = capability_matrix_rows
        .iter()
        .filter(|row| {
            row.capability_class
                == TassadarPostArticleStarterPluginCapabilityClass::LocalDeterministic
        })
        .count() as u32;
    let read_only_network_plugin_count = capability_matrix_rows
        .iter()
        .filter(|row| {
            row.capability_class == TassadarPostArticleStarterPluginCapabilityClass::ReadOnlyNetwork
        })
        .count() as u32;

    let mut bundle = TassadarPostArticleStarterPluginCatalogBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.post_article.starter_plugin_catalog.runtime_bundle.v1"),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        supporting_material_refs: vec![
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            String::from(AUTOPILOT_PORTING_NOTES_REF),
            String::from(URL_EXTRACTOR_INVENTORY_REF),
            String::from(RSS_FEED_INVENTORY_REF),
            String::from(MULTI_PLUGIN_AUDIT_REF),
        ],
        descriptor_refs,
        fixture_bundle_refs,
        sample_mount_envelope_refs,
        descriptor_rows: descriptors.clone(),
        capability_matrix_rows,
        composition_case_rows,
        plugin_count: descriptors.len() as u32,
        local_deterministic_plugin_count,
        read_only_network_plugin_count,
        bounded_flow_count: 2,
        operator_only_posture: true,
        runtime_builtins_separate: true,
        public_marketplace_implication_allowed: false,
        claim_boundary: String::from(
            "this runtime-owned starter catalog freezes one small operator-curated plugin set above the canonical post-article machine closure bundle without implying served or public plugin rights, arbitrary public Wasm execution, arbitrary public tool use, or a public plugin marketplace.",
        ),
        summary: String::from(
            "starter catalog bundle publishes six operator-curated starter plugins, including one digest-bound guest-artifact entry, with per-plugin descriptor or fixture or mount sidecars and two bounded composition flows across local deterministic and read-only network capability classes.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_catalog_bundle|",
        &bundle,
    );

    TassadarPostArticleStarterPluginCatalogArtifacts {
        bundle,
        descriptors,
        fixture_bundles,
        mount_envelopes,
    }
}

#[must_use]
pub fn tassadar_post_article_starter_plugin_catalog_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_BUNDLE_REF)
}

pub fn write_tassadar_post_article_starter_plugin_catalog_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleStarterPluginCatalogBundle,
    TassadarPostArticleStarterPluginCatalogBundleError,
> {
    let output_path = output_path.as_ref();
    let artifacts = build_tassadar_post_article_starter_plugin_catalog_artifacts();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleStarterPluginCatalogBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
        for descriptor in &artifacts.descriptors {
            write_json(
                sibling_output_path(parent, &descriptor.descriptor_ref),
                descriptor,
            )?;
        }
        for fixture_bundle in &artifacts.fixture_bundles {
            let output = sibling_output_path(
                parent,
                catalog_registration(&fixture_bundle.plugin_id)
                    .catalog
                    .expect("catalog metadata")
                    .fixture_bundle_ref,
            );
            write_json(output, fixture_bundle)?;
        }
        for mount_envelope in &artifacts.mount_envelopes {
            let output = sibling_output_path(
                parent,
                catalog_registration(&mount_envelope.plugin_id)
                    .catalog
                    .expect("catalog metadata")
                    .sample_mount_envelope_ref,
            );
            write_json(output, mount_envelope)?;
        }
    }
    write_json(output_path, &artifacts.bundle)?;
    Ok(artifacts.bundle)
}

fn starter_plugin_descriptor(
    plugin_id: &str,
    manifest_id: &str,
    artifact_id: &str,
    capability_class: TassadarPostArticleStarterPluginCapabilityClass,
    replay_class_id: &str,
    trust_tier_id: &str,
    evidence_posture_id: &str,
    input_schema_id: &str,
    success_output_schema_id: &str,
    refusal_schema_ids: &[&str],
    capability_namespace_ids: &[&str],
    negative_claim_ids: &[&str],
    descriptor_ref: &str,
    fixture_bundle_ref: &str,
    sample_mount_envelope_ref: &str,
    detail: &str,
) -> TassadarPostArticleStarterPluginDescriptor {
    let mut descriptor = TassadarPostArticleStarterPluginDescriptor {
        plugin_id: String::from(plugin_id),
        plugin_version: String::from("v1"),
        catalog_entry_id: format!("{plugin_id}@v1"),
        manifest_id: String::from(manifest_id),
        artifact_id: String::from(artifact_id),
        artifact_digest: String::new(),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        packet_schemas: TassadarPostArticleStarterPluginPacketSchemaSet {
            input_schema_id: String::from(input_schema_id),
            success_output_schema_id: String::from(success_output_schema_id),
            refusal_schema_ids: refusal_schema_ids
                .iter()
                .map(|id| String::from(*id))
                .collect(),
        },
        capability_class,
        replay_class_id: String::from(replay_class_id),
        trust_tier_id: String::from(trust_tier_id),
        evidence_posture_id: String::from(evidence_posture_id),
        capability_namespace_ids: capability_namespace_ids
            .iter()
            .map(|id| String::from(*id))
            .collect(),
        negative_claim_ids: negative_claim_ids
            .iter()
            .map(|id| String::from(*id))
            .collect(),
        descriptor_ref: String::from(descriptor_ref),
        fixture_bundle_ref: String::from(fixture_bundle_ref),
        sample_mount_envelope_ref: String::from(sample_mount_envelope_ref),
        detail: String::from(detail),
    };
    descriptor.artifact_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_descriptor|",
        &descriptor,
    );
    descriptor
}

fn starter_fixture_bundle(
    plugin_id: &str,
    cases: &[TassadarPostArticleStarterPluginFixtureCase],
    negative_claim_ids: &[&str],
) -> TassadarPostArticleStarterPluginFixtureBundle {
    let success_case_ids = cases
        .iter()
        .filter(|case| case.status == TassadarPostArticleStarterPluginFixtureStatus::ExactSuccess)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let malformed_case_ids = cases
        .iter()
        .filter(|case| {
            case.status == TassadarPostArticleStarterPluginFixtureStatus::TypedMalformedPacket
        })
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let refusal_case_ids = cases
        .iter()
        .filter(|case| case.status == TassadarPostArticleStarterPluginFixtureStatus::TypedRefusal)
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut bundle = TassadarPostArticleStarterPluginFixtureBundle {
        plugin_id: String::from(plugin_id),
        plugin_version: String::from("v1"),
        fixture_bundle_id: format!("fixture_bundle.{plugin_id}.v1"),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        success_case_ids,
        malformed_case_ids,
        refusal_case_ids,
        negative_claim_ids: negative_claim_ids
            .iter()
            .map(|id| String::from(*id))
            .collect(),
        cases: cases.to_vec(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_fixture_bundle|",
        &bundle,
    );
    bundle
}

fn starter_fixture_case(
    case_id: &str,
    status: TassadarPostArticleStarterPluginFixtureStatus,
    request_schema_id: &str,
    request: &serde_json::Value,
    response_or_refusal_schema_id: &str,
    response_or_refusal: &serde_json::Value,
    replay_class_id: &str,
    receipt_id: &str,
    refusal_or_failure_class_id: Option<&str>,
    detail: &str,
) -> TassadarPostArticleStarterPluginFixtureCase {
    TassadarPostArticleStarterPluginFixtureCase {
        case_id: String::from(case_id),
        status,
        request_schema_id: String::from(request_schema_id),
        request_packet_digest: stable_digest(
            b"psionic_tassadar_post_article_starter_plugin_request|",
            request,
        ),
        response_or_refusal_schema_id: String::from(response_or_refusal_schema_id),
        response_or_refusal_digest: stable_digest(
            b"psionic_tassadar_post_article_starter_plugin_response|",
            response_or_refusal,
        ),
        replay_class_id: String::from(replay_class_id),
        receipt_id: String::from(receipt_id),
        refusal_or_failure_class_id: refusal_or_failure_class_id.map(String::from),
        detail: String::from(detail),
    }
}

fn starter_mount_envelope(
    envelope_id: &str,
    plugin_id: &str,
    world_mount_id: &str,
    capability_namespace_ids: &[&str],
    allowlisted_url_prefixes: &[&str],
    allowed_method_ids: &[&str],
    timeout_millis: u32,
    response_size_limit_bytes: u64,
    redirect_limit: u8,
    replay_posture_id: &str,
    detail: &str,
) -> TassadarPostArticleStarterPluginMountEnvelope {
    let mut envelope = TassadarPostArticleStarterPluginMountEnvelope {
        envelope_id: String::from(envelope_id),
        plugin_id: String::from(plugin_id),
        plugin_version: String::from("v1"),
        world_mount_id: String::from(world_mount_id),
        capability_namespace_ids: capability_namespace_ids
            .iter()
            .map(|id| String::from(*id))
            .collect(),
        allowlisted_url_prefixes: allowlisted_url_prefixes
            .iter()
            .map(|prefix| String::from(*prefix))
            .collect(),
        allowed_method_ids: allowed_method_ids
            .iter()
            .map(|method| String::from(*method))
            .collect(),
        timeout_millis,
        response_size_limit_bytes,
        redirect_limit,
        replay_posture_id: String::from(replay_posture_id),
        green: true,
        detail: String::from(detail),
        envelope_digest: String::new(),
    };
    envelope.envelope_digest = stable_digest(
        b"psionic_tassadar_post_article_starter_plugin_mount_envelope|",
        &envelope,
    );
    envelope
}

fn capability_matrix_row(
    plugin_id: &str,
    catalog_entry_id: &str,
    capability_class: TassadarPostArticleStarterPluginCapabilityClass,
    reads_network: bool,
    deterministic_replayable: bool,
    snapshot_backed_replay: bool,
    filesystem_access: bool,
    secrets_access: bool,
    mount_required: bool,
    host_mediated_network_only: bool,
    schema_local_composition: bool,
    operator_only: bool,
    detail: &str,
) -> TassadarPostArticleStarterPluginCapabilityMatrixRow {
    TassadarPostArticleStarterPluginCapabilityMatrixRow {
        plugin_id: String::from(plugin_id),
        catalog_entry_id: String::from(catalog_entry_id),
        capability_class,
        reads_network,
        deterministic_replayable,
        snapshot_backed_replay,
        filesystem_access,
        secrets_access,
        mount_required,
        host_mediated_network_only,
        schema_local_composition,
        operator_only,
        detail: String::from(detail),
    }
}

fn composition_step(
    step_index: u16,
    plugin_id: &str,
    input_schema_id: &str,
    output_schema_id: &str,
    mount_envelope_id: &str,
    receipt_id: &str,
    replay_class_id: &str,
    detail: &str,
) -> TassadarPostArticleStarterPluginCompositionStepRow {
    TassadarPostArticleStarterPluginCompositionStepRow {
        step_index,
        plugin_id: String::from(plugin_id),
        input_schema_id: String::from(input_schema_id),
        output_schema_id: String::from(output_schema_id),
        mount_envelope_id: String::from(mount_envelope_id),
        receipt_id: String::from(receipt_id),
        replay_class_id: String::from(replay_class_id),
        detail: String::from(detail),
    }
}

fn composition_case(
    case_id: &str,
    flow_class_id: &str,
    step_rows: Vec<TassadarPostArticleStarterPluginCompositionStepRow>,
    detail: &str,
) -> TassadarPostArticleStarterPluginCompositionCaseRow {
    TassadarPostArticleStarterPluginCompositionCaseRow {
        case_id: String::from(case_id),
        flow_class_id: String::from(flow_class_id),
        step_plugin_ids: step_rows.iter().map(|row| row.plugin_id.clone()).collect(),
        step_receipt_ids: step_rows.iter().map(|row| row.receipt_id.clone()).collect(),
        hidden_host_orchestration_allowed: false,
        schema_repair_allowed: false,
        capability_leakage_allowed: false,
        green: true,
        step_rows,
        detail: String::from(detail),
    }
}

fn write_json<T: Serialize>(
    output_path: impl AsRef<Path>,
    value: &T,
) -> Result<(), TassadarPostArticleStarterPluginCatalogBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleStarterPluginCatalogBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn sibling_output_path(output_dir: &Path, ref_path: &str) -> PathBuf {
    output_dir.join(
        Path::new(ref_path)
            .file_name()
            .expect("starter plugin sidecar refs must have filenames"),
    )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleStarterPluginCatalogBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleStarterPluginCatalogBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_starter_plugin_catalog_artifacts,
        build_tassadar_post_article_starter_plugin_catalog_bundle, read_json,
        tassadar_post_article_starter_plugin_catalog_bundle_path,
        write_tassadar_post_article_starter_plugin_catalog_bundle,
        TassadarPostArticleStarterPluginCatalogBundle, TassadarPostArticleStarterPluginDescriptor,
        TassadarPostArticleStarterPluginFixtureBundle,
        TassadarPostArticleStarterPluginMountEnvelope,
        TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_BUNDLE_REF,
    };

    #[test]
    fn starter_plugin_catalog_bundle_covers_declared_plugin_set() {
        let bundle = build_tassadar_post_article_starter_plugin_catalog_bundle();

        assert_eq!(
            bundle.bundle_id,
            "tassadar.post_article.starter_plugin_catalog.runtime_bundle.v1"
        );
        assert_eq!(bundle.plugin_count, 6);
        assert_eq!(bundle.local_deterministic_plugin_count, 5);
        assert_eq!(bundle.read_only_network_plugin_count, 1);
        assert_eq!(bundle.bounded_flow_count, 2);
        assert!(bundle.operator_only_posture);
        assert!(bundle.runtime_builtins_separate);
        assert!(!bundle.public_marketplace_implication_allowed);
        assert!(bundle
            .descriptor_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.text.url_extract"));
        assert!(bundle
            .descriptor_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.text.stats"));
        assert!(bundle
            .descriptor_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.http.fetch_text"));
        assert!(bundle
            .descriptor_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.example.echo_guest"));
        assert!(bundle
            .descriptor_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.html.extract_readable"));
        assert!(bundle
            .descriptor_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.feed.rss_atom_parse"));
    }

    #[test]
    fn starter_plugin_catalog_sidecars_match_committed_truth() {
        let artifacts = build_tassadar_post_article_starter_plugin_catalog_artifacts();
        let output_dir = tassadar_post_article_starter_plugin_catalog_bundle_path()
            .parent()
            .expect("run root")
            .to_path_buf();

        let committed_descriptors = [
            read_json::<TassadarPostArticleStarterPluginDescriptor>(
                output_dir.join("plugin_text_url_extract_descriptor.json"),
            )
            .expect("url extract descriptor"),
            read_json::<TassadarPostArticleStarterPluginDescriptor>(
                output_dir.join("plugin_text_stats_descriptor.json"),
            )
            .expect("text-stats descriptor"),
            read_json::<TassadarPostArticleStarterPluginDescriptor>(
                output_dir.join("plugin_http_fetch_text_descriptor.json"),
            )
            .expect("fetch text descriptor"),
            read_json::<TassadarPostArticleStarterPluginDescriptor>(
                output_dir.join("plugin_example_echo_guest_descriptor.json"),
            )
            .expect("guest echo descriptor"),
            read_json::<TassadarPostArticleStarterPluginDescriptor>(
                output_dir.join("plugin_html_extract_readable_descriptor.json"),
            )
            .expect("extract readable descriptor"),
            read_json::<TassadarPostArticleStarterPluginDescriptor>(
                output_dir.join("plugin_feed_rss_atom_parse_descriptor.json"),
            )
            .expect("rss atom descriptor"),
        ];
        let committed_fixture_bundles = [
            read_json::<TassadarPostArticleStarterPluginFixtureBundle>(
                output_dir.join("plugin_text_url_extract_fixture_bundle.json"),
            )
            .expect("url extract fixture bundle"),
            read_json::<TassadarPostArticleStarterPluginFixtureBundle>(
                output_dir.join("plugin_text_stats_fixture_bundle.json"),
            )
            .expect("text-stats fixture bundle"),
            read_json::<TassadarPostArticleStarterPluginFixtureBundle>(
                output_dir.join("plugin_http_fetch_text_fixture_bundle.json"),
            )
            .expect("fetch text fixture bundle"),
            read_json::<TassadarPostArticleStarterPluginFixtureBundle>(
                output_dir.join("plugin_example_echo_guest_fixture_bundle.json"),
            )
            .expect("guest echo fixture bundle"),
            read_json::<TassadarPostArticleStarterPluginFixtureBundle>(
                output_dir.join("plugin_html_extract_readable_fixture_bundle.json"),
            )
            .expect("extract readable fixture bundle"),
            read_json::<TassadarPostArticleStarterPluginFixtureBundle>(
                output_dir.join("plugin_feed_rss_atom_parse_fixture_bundle.json"),
            )
            .expect("rss atom fixture bundle"),
        ];
        let committed_mount_envelopes = [
            read_json::<TassadarPostArticleStarterPluginMountEnvelope>(
                output_dir.join("plugin_text_url_extract_mount_envelope.json"),
            )
            .expect("url extract mount"),
            read_json::<TassadarPostArticleStarterPluginMountEnvelope>(
                output_dir.join("plugin_text_stats_mount_envelope.json"),
            )
            .expect("text-stats mount"),
            read_json::<TassadarPostArticleStarterPluginMountEnvelope>(
                output_dir.join("plugin_http_fetch_text_mount_envelope.json"),
            )
            .expect("fetch text mount"),
            read_json::<TassadarPostArticleStarterPluginMountEnvelope>(
                output_dir.join("plugin_example_echo_guest_mount_envelope.json"),
            )
            .expect("guest echo mount"),
            read_json::<TassadarPostArticleStarterPluginMountEnvelope>(
                output_dir.join("plugin_html_extract_readable_mount_envelope.json"),
            )
            .expect("extract readable mount"),
            read_json::<TassadarPostArticleStarterPluginMountEnvelope>(
                output_dir.join("plugin_feed_rss_atom_parse_mount_envelope.json"),
            )
            .expect("rss atom mount"),
        ];

        assert_eq!(
            artifacts.descriptors.as_slice(),
            committed_descriptors.as_slice()
        );
        assert_eq!(
            artifacts.fixture_bundles.as_slice(),
            committed_fixture_bundles.as_slice()
        );
        assert_eq!(
            artifacts.mount_envelopes.as_slice(),
            committed_mount_envelopes.as_slice()
        );
    }

    #[test]
    fn starter_plugin_catalog_bundle_matches_committed_truth() {
        let generated = build_tassadar_post_article_starter_plugin_catalog_bundle();
        let committed: TassadarPostArticleStarterPluginCatalogBundle =
            read_json(tassadar_post_article_starter_plugin_catalog_bundle_path())
                .expect("committed starter catalog bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_starter_plugin_catalog_bundle_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_starter_plugin_catalog_bundle.json");
        let written =
            write_tassadar_post_article_starter_plugin_catalog_bundle(&output_path).expect("write");
        let roundtrip: TassadarPostArticleStarterPluginCatalogBundle =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_post_article_starter_plugin_catalog_bundle.json")
        );
        assert!(tempdir
            .path()
            .join("plugin_http_fetch_text_descriptor.json")
            .exists());
        assert!(tempdir
            .path()
            .join("plugin_example_echo_guest_descriptor.json")
            .exists());
        assert!(tempdir
            .path()
            .join("plugin_text_stats_mount_envelope.json")
            .exists());
        assert!(tempdir
            .path()
            .join("plugin_feed_rss_atom_parse_mount_envelope.json")
            .exists());
        assert_eq!(
            tassadar_post_article_starter_plugin_catalog_bundle_path()
                .strip_prefix(super::repo_root())
                .expect("starter catalog bundle should live under repo root")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_STARTER_PLUGIN_CATALOG_BUNDLE_REF
        );
    }
}
