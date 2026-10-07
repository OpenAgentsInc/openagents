use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

use crate::{
    psion_plugin_guest_artifact_tool_projection, reference_psion_plugin_guest_artifact_manifest,
    TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION,
    PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_OUTPUT_SCHEMA_ID,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_ID,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_REPLAY_CLASS_ID,
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_TEXT_URL_EXTRACT_RUNTIME_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_plugin_text_url_extract_v1/tassadar_post_article_plugin_text_url_extract_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_TEXT_URL_EXTRACT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_text_url_extract_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_TEXT_STATS_RUNTIME_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_plugin_text_stats_v1/tassadar_post_article_plugin_text_stats_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_TEXT_STATS_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_text_stats_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HTTP_FETCH_TEXT_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_http_fetch_text_v1/tassadar_post_article_plugin_http_fetch_text_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HTTP_FETCH_TEXT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_http_fetch_text_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HTML_EXTRACT_READABLE_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_html_extract_readable_v1/tassadar_post_article_plugin_html_extract_readable_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_HTML_EXTRACT_READABLE_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_html_extract_readable_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_FEED_RSS_ATOM_PARSE_RUNTIME_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_feed_rss_atom_parse_v1/tassadar_post_article_plugin_feed_rss_atom_parse_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_FEED_RSS_ATOM_PARSE_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_feed_rss_atom_parse_v1";
pub const PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_RUN_ROOT_REF: &str =
    "fixtures/psion/plugins/guest_artifact";

pub const STARTER_PLUGIN_VERSION: &str = "v1";
pub const STARTER_PLUGIN_TEXT_URL_EXTRACT_ID: &str = "plugin.text.url_extract";
pub const STARTER_PLUGIN_TEXT_URL_EXTRACT_TOOL_NAME: &str = "plugin_text_url_extract";
pub const STARTER_PLUGIN_TEXT_URL_EXTRACT_INPUT_SCHEMA_ID: &str =
    "plugin.text.url_extract.input.v1";
pub const STARTER_PLUGIN_TEXT_URL_EXTRACT_OUTPUT_SCHEMA_ID: &str =
    "plugin.text.url_extract.output.v1";
pub const STARTER_PLUGIN_TEXT_STATS_ID: &str = "plugin.text.stats";
pub const STARTER_PLUGIN_TEXT_STATS_TOOL_NAME: &str = "plugin_text_stats";
pub const STARTER_PLUGIN_TEXT_STATS_INPUT_SCHEMA_ID: &str = "plugin.text.stats.input.v1";
pub const STARTER_PLUGIN_TEXT_STATS_OUTPUT_SCHEMA_ID: &str = "plugin.text.stats.output.v1";
pub const STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID: &str = "plugin.refusal.schema_invalid.v1";
pub const STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID: &str = "plugin.refusal.packet_too_large.v1";
pub const STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID: &str = "plugin.refusal.unsupported_codec.v1";
pub const STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID: &str =
    "plugin.refusal.runtime_resource_limit.v1";
pub const STARTER_PLUGIN_HTTP_FETCH_TEXT_ID: &str = "plugin.http.fetch_text";
pub const STARTER_PLUGIN_HTTP_FETCH_TEXT_TOOL_NAME: &str = "plugin_http_fetch_text";
pub const STARTER_PLUGIN_HTTP_FETCH_TEXT_INPUT_SCHEMA_ID: &str = "plugin.http.fetch_text.input.v1";
pub const STARTER_PLUGIN_HTTP_FETCH_TEXT_OUTPUT_SCHEMA_ID: &str =
    "plugin.http.fetch_text.output.v1";
pub const STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID: &str = "plugin.html.extract_readable";
pub const STARTER_PLUGIN_HTML_EXTRACT_READABLE_TOOL_NAME: &str = "plugin_html_extract_readable";
pub const STARTER_PLUGIN_HTML_EXTRACT_READABLE_INPUT_SCHEMA_ID: &str =
    "plugin.html.extract_readable.input.v1";
pub const STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID: &str =
    "plugin.html.extract_readable.output.v1";
pub const STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID: &str = "plugin.feed.rss_atom_parse";
pub const STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_TOOL_NAME: &str = "plugin_feed_rss_atom_parse";
pub const STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_INPUT_SCHEMA_ID: &str =
    "plugin.feed.rss_atom_parse.input.v1";
pub const STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID: &str =
    "plugin.feed.rss_atom_parse.output.v1";
pub const STARTER_PLUGIN_GUEST_ECHO_ID: &str = PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_ID;
pub const STARTER_PLUGIN_GUEST_ECHO_TOOL_NAME: &str =
    PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_TOOL_NAME;
pub const STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID: &str = "plugin.refusal.network_denied.v1";
pub const STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID: &str = "plugin.refusal.url_not_permitted.v1";
pub const STARTER_PLUGIN_REFUSAL_TIMEOUT_ID: &str = "plugin.refusal.timeout.v1";
pub const STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID: &str =
    "plugin.refusal.response_too_large.v1";
pub const STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID: &str =
    "plugin.refusal.content_type_unsupported.v1";
pub const STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID: &str = "plugin.refusal.decode_failed.v1";
pub const STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID: &str = "plugin.refusal.upstream_failure.v1";
pub const STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID: &str = "plugin.refusal.input_too_large.v1";
pub const STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID: &str =
    "plugin.refusal.unsupported_feed_format.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterPluginInvocationStatus {
    Success,
    Refusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginToolProjection {
    pub plugin_id: String,
    pub tool_name: String,
    pub description: String,
    pub arguments_schema: Value,
    pub result_schema_id: String,
    pub refusal_schema_ids: Vec<String>,
    pub replay_class_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginInvocationReceipt {
    pub receipt_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub tool_name: String,
    pub packet_abi_version: String,
    pub mount_envelope_id: String,
    pub capability_namespace_ids: Vec<String>,
    pub replay_class_id: String,
    pub status: StarterPluginInvocationStatus,
    pub input_schema_id: String,
    pub input_packet_digest: String,
    pub output_or_refusal_schema_id: String,
    pub output_or_refusal_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_class_id: Option<String>,
    pub detail: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginRefusal {
    pub schema_id: String,
    pub refusal_class_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlExtractRequest {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlExtractResponse {
    pub urls: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlExtractConfig {
    pub packet_size_limit_bytes: usize,
    pub max_urls: usize,
}

impl Default for UrlExtractConfig {
    fn default() -> Self {
        Self {
            packet_size_limit_bytes: 16 * 1024,
            max_urls: 128,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlExtractInvocationOutcome {
    pub receipt: StarterPluginInvocationReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<UrlExtractResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<StarterPluginRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlExtractRuntimeCaseStatus {
    ExactSuccess,
    TypedMalformedPacket,
    TypedRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlExtractRuntimeCase {
    pub case_id: String,
    pub status: UrlExtractRuntimeCaseStatus,
    pub codec_id: String,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub receipt: StarterPluginInvocationReceipt,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlExtractRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_id: String,
    pub artifact_id: String,
    pub packet_abi_version: String,
    pub mount_envelope_id: String,
    pub tool_projection: StarterPluginToolProjection,
    pub negative_claim_ids: Vec<String>,
    pub case_rows: Vec<UrlExtractRuntimeCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStatsRequest {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStatsResponse {
    pub byte_count: usize,
    pub unicode_scalar_count: usize,
    pub line_count: usize,
    pub non_empty_line_count: usize,
    pub word_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStatsConfig {
    pub packet_size_limit_bytes: usize,
}

impl Default for TextStatsConfig {
    fn default() -> Self {
        Self {
            packet_size_limit_bytes: 16 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStatsInvocationOutcome {
    pub receipt: StarterPluginInvocationReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<TextStatsResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<StarterPluginRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextStatsRuntimeCaseStatus {
    ExactSuccess,
    TypedMalformedPacket,
    TypedRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStatsRuntimeCase {
    pub case_id: String,
    pub status: TextStatsRuntimeCaseStatus,
    pub codec_id: String,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub receipt: StarterPluginInvocationReceipt,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStatsRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_id: String,
    pub artifact_id: String,
    pub packet_abi_version: String,
    pub mount_envelope_id: String,
    pub tool_projection: StarterPluginToolProjection,
    pub negative_claim_ids: Vec<String>,
    pub case_rows: Vec<TextStatsRuntimeCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchTextRequest {
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchTextResponse {
    pub final_url: String,
    pub status_code: u16,
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    pub body_text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchTextMountEnvelope {
    pub envelope_id: String,
    pub allowlisted_url_prefixes: Vec<String>,
    pub timeout_millis: u64,
    pub response_size_limit_bytes: usize,
    pub redirect_limit: usize,
    pub allowed_content_type_ids: Vec<String>,
    pub replay_class_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchTextSnapshotResponse {
    pub final_url: String,
    pub status_code: u16,
    pub content_type: String,
    pub charset: Option<String>,
    pub body_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchTextSnapshotResult {
    Success(FetchTextSnapshotResponse),
    Timeout,
    NetworkDenied { detail: String },
    UpstreamFailure { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchTextBackend {
    Snapshot(BTreeMap<String, FetchTextSnapshotResult>),
    Live,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchTextConfig {
    pub mount_envelope: FetchTextMountEnvelope,
    pub backend: FetchTextBackend,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchTextInvocationOutcome {
    pub receipt: StarterPluginInvocationReceipt,
    pub backend_id: String,
    pub logical_cpu_millis: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<FetchTextResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<StarterPluginRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchTextRuntimeCaseStatus {
    ExactSuccess,
    TypedMalformedPacket,
    TypedRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchTextRuntimeCase {
    pub case_id: String,
    pub status: FetchTextRuntimeCaseStatus,
    pub codec_id: String,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub backend_id: String,
    pub logical_cpu_millis: u32,
    pub receipt: StarterPluginInvocationReceipt,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchTextRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_id: String,
    pub artifact_id: String,
    pub packet_abi_version: String,
    pub sample_mount_envelope: FetchTextMountEnvelope,
    pub tool_projection: StarterPluginToolProjection,
    pub supported_replay_class_ids: Vec<String>,
    pub negative_claim_ids: Vec<String>,
    pub case_rows: Vec<FetchTextRuntimeCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableRequest {
    pub source_url: String,
    pub content_type: String,
    pub body_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub readable_text: String,
    pub harvested_links: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_language: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableConfig {
    pub input_size_limit_bytes: usize,
    pub max_links: usize,
}

impl Default for ExtractReadableConfig {
    fn default() -> Self {
        Self {
            input_size_limit_bytes: 64 * 1024,
            max_links: 128,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableInvocationOutcome {
    pub receipt: StarterPluginInvocationReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<ExtractReadableResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<StarterPluginRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractReadableRuntimeCaseStatus {
    ExactSuccess,
    TypedMalformedPacket,
    TypedRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableRuntimeCase {
    pub case_id: String,
    pub status: ExtractReadableRuntimeCaseStatus,
    pub codec_id: String,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub receipt: StarterPluginInvocationReceipt,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableCompositionCase {
    pub case_id: String,
    pub step_plugin_ids: Vec<String>,
    pub step_receipt_ids: Vec<String>,
    pub schema_repair_allowed: bool,
    pub hidden_host_extraction_allowed: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractReadableRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_id: String,
    pub artifact_id: String,
    pub packet_abi_version: String,
    pub mount_envelope_id: String,
    pub tool_projection: StarterPluginToolProjection,
    pub negative_claim_ids: Vec<String>,
    pub case_rows: Vec<ExtractReadableRuntimeCase>,
    pub composition_case: ExtractReadableCompositionCase,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseRequest {
    pub source_url: String,
    pub content_type: String,
    pub feed_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_excerpt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_homepage_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_description: Option<String>,
    pub entries: Vec<FeedParseEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseConfig {
    pub input_size_limit_bytes: usize,
    pub max_entries: usize,
}

impl Default for FeedParseConfig {
    fn default() -> Self {
        Self {
            input_size_limit_bytes: 64 * 1024,
            max_entries: 64,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseInvocationOutcome {
    pub receipt: StarterPluginInvocationReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<FeedParseResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<StarterPluginRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedParseRuntimeCaseStatus {
    ExactSuccess,
    TypedMalformedPacket,
    TypedRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseRuntimeCase {
    pub case_id: String,
    pub status: FeedParseRuntimeCaseStatus,
    pub codec_id: String,
    pub request_packet_digest: String,
    pub response_or_refusal_schema_id: String,
    pub response_or_refusal_digest: String,
    pub receipt: StarterPluginInvocationReceipt,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseCompositionCase {
    pub case_id: String,
    pub step_plugin_ids: Vec<String>,
    pub step_receipt_ids: Vec<String>,
    pub schema_repair_allowed: bool,
    pub hidden_host_parsing_allowed: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedParseRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub manifest_id: String,
    pub artifact_id: String,
    pub packet_abi_version: String,
    pub mount_envelope_id: String,
    pub tool_projection: StarterPluginToolProjection,
    pub negative_claim_ids: Vec<String>,
    pub case_rows: Vec<FeedParseRuntimeCase>,
    pub composition_case: FeedParseCompositionCase,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum StarterPluginRuntimeError {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterPluginCapabilityClass {
    LocalDeterministic,
    ReadOnlyNetwork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterPluginAuthoringClass {
    CapabilityFreeLocalDeterministic,
    NetworkedReadOnly,
    GuestArtifactDigestBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterPluginOriginClass {
    OperatorBuiltin,
    UserAdded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StarterPluginCatalogRegistration {
    pub catalog_entry_id: &'static str,
    pub trust_tier_id: &'static str,
    pub evidence_posture_id: &'static str,
    pub catalog_capability_namespace_ids: &'static [&'static str],
    pub descriptor_ref: &'static str,
    pub fixture_bundle_ref: &'static str,
    pub sample_mount_envelope_ref: &'static str,
    pub descriptor_detail: &'static str,
    pub capability_matrix_detail: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StarterPluginRegistration {
    pub plugin_id: &'static str,
    pub plugin_version: &'static str,
    pub tool_name: &'static str,
    pub input_schema_id: &'static str,
    pub success_output_schema_id: &'static str,
    pub refusal_schema_ids: &'static [&'static str],
    pub replay_class_id: &'static str,
    pub capability_class: StarterPluginCapabilityClass,
    pub authoring_class: StarterPluginAuthoringClass,
    pub origin_class: StarterPluginOriginClass,
    pub capability_namespace_ids: &'static [&'static str],
    pub negative_claim_ids: &'static [&'static str],
    pub mount_envelope_id: &'static str,
    pub manifest_id: &'static str,
    pub artifact_id: &'static str,
    pub runtime_bundle_id: &'static str,
    pub runtime_bundle_ref: &'static str,
    pub runtime_run_root_ref: &'static str,
    pub tool_description: &'static str,
    pub bridge_exposed: bool,
    pub catalog_exposed: bool,
    pub catalog: Option<StarterPluginCatalogRegistration>,
}

const URL_EXTRACT_REFUSAL_SCHEMA_IDS: &[&str] = &[
    STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
    STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
    STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
    STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID,
];
const TEXT_STATS_REFUSAL_SCHEMA_IDS: &[&str] = &[
    STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
    STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
    STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
];
const FETCH_TEXT_REFUSAL_SCHEMA_IDS: &[&str] = &[
    STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
    STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
    STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID,
    STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID,
    STARTER_PLUGIN_REFUSAL_TIMEOUT_ID,
    STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID,
    STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
    STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID,
    STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID,
];
const EXTRACT_READABLE_REFUSAL_SCHEMA_IDS: &[&str] = &[
    STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
    STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
    STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
];
const FEED_PARSE_REFUSAL_SCHEMA_IDS: &[&str] = &[
    STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
    STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
    STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID,
];
const GUEST_ECHO_REFUSAL_SCHEMA_IDS: &[&str] = &[
    STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
    STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
    STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID,
    "plugin.refusal.runtime_unavailable.v1",
];

const URL_EXTRACT_NEGATIVE_CLAIM_IDS: &[&str] = &[
    "url_validation_truth_not_claimed",
    "dns_resolution_not_claimed",
    "redirect_truth_not_claimed",
    "network_reachability_not_claimed",
];
const TEXT_STATS_NEGATIVE_CLAIM_IDS: &[&str] = &[
    "tokenizer_truth_not_claimed",
    "language_detection_not_claimed",
    "sentence_boundary_truth_not_claimed",
    "semantic_structure_not_claimed",
];
const FETCH_TEXT_NEGATIVE_CLAIM_IDS: &[&str] = &[
    "browser_execution_not_claimed",
    "javascript_execution_not_claimed",
    "cookie_or_auth_session_not_claimed",
    "arbitrary_headers_not_claimed",
    "general_unrestricted_web_access_not_claimed",
];
const EXTRACT_READABLE_NEGATIVE_CLAIM_IDS: &[&str] = &[
    "browser_rendering_not_claimed",
    "javascript_evaluation_not_claimed",
    "css_layout_truth_not_claimed",
    "full_dom_semantics_not_claimed",
];
const FEED_PARSE_NEGATIVE_CLAIM_IDS: &[&str] = &[
    "arbitrary_xml_support_not_claimed",
    "opml_support_not_claimed",
    "general_document_parsing_not_claimed",
];
const GUEST_ECHO_NEGATIVE_CLAIM_IDS: &[&str] = &[
    "ambient_host_sdk_not_claimed",
    "guest_artifact_publication_not_claimed",
    "arbitrary_wasm_plugin_support_not_claimed",
    "secret_or_stateful_guest_capability_not_claimed",
];

const NO_CAPABILITY_NAMESPACE_IDS: &[&str] = &[];
const FETCH_TEXT_CAPABILITY_NAMESPACE_IDS: &[&str] = &["capability.http.read_only.v1"];
const FETCH_TEXT_CATALOG_CAPABILITY_NAMESPACE_IDS: &[&str] = &["cap.http.read_only_text.v1"];

const STARTER_PLUGIN_REGISTRATIONS: &[StarterPluginRegistration] = &[
    StarterPluginRegistration {
        plugin_id: STARTER_PLUGIN_TEXT_URL_EXTRACT_ID,
        plugin_version: STARTER_PLUGIN_VERSION,
        tool_name: STARTER_PLUGIN_TEXT_URL_EXTRACT_TOOL_NAME,
        input_schema_id: STARTER_PLUGIN_TEXT_URL_EXTRACT_INPUT_SCHEMA_ID,
        success_output_schema_id: STARTER_PLUGIN_TEXT_URL_EXTRACT_OUTPUT_SCHEMA_ID,
        refusal_schema_ids: URL_EXTRACT_REFUSAL_SCHEMA_IDS,
        replay_class_id: "deterministic_replayable",
        capability_class: StarterPluginCapabilityClass::LocalDeterministic,
        authoring_class: StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic,
        origin_class: StarterPluginOriginClass::OperatorBuiltin,
        capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
        negative_claim_ids: URL_EXTRACT_NEGATIVE_CLAIM_IDS,
        mount_envelope_id: "mount.plugin.text.url_extract.no_capabilities.v1",
        manifest_id: "manifest.plugin.text.url_extract.v1",
        artifact_id: "artifact.plugin.text.url_extract.v1",
        runtime_bundle_id: "tassadar.post_article.plugin_text_url_extract.runtime_bundle.v1",
        runtime_bundle_ref: TASSADAR_POST_ARTICLE_PLUGIN_TEXT_URL_EXTRACT_RUNTIME_BUNDLE_REF,
        runtime_run_root_ref: TASSADAR_POST_ARTICLE_PLUGIN_TEXT_URL_EXTRACT_RUN_ROOT_REF,
        tool_description: "extract literal http:// and https:// strings from packet-local text without URL validation, DNS, or network reachability claims.",
        bridge_exposed: true,
        catalog_exposed: true,
        catalog: Some(StarterPluginCatalogRegistration {
            catalog_entry_id: "plugin.text.url_extract@v1",
            trust_tier_id: "operator_curated_local_deterministic",
            evidence_posture_id: "evidence.descriptor_fixture_receipt_bound.v1",
            catalog_capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
            descriptor_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_text_url_extract_descriptor.json",
            fixture_bundle_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_text_url_extract_fixture_bundle.json",
            sample_mount_envelope_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_text_url_extract_mount_envelope.json",
            descriptor_detail: "the first starter plugin freezes left-to-right `http://` and `https://` string extraction with no deduplication and no capability mounts.",
            capability_matrix_detail: "the url-extract starter plugin is purely local string processing and does not claim any external-world semantics.",
        }),
    },
    StarterPluginRegistration {
        plugin_id: STARTER_PLUGIN_TEXT_STATS_ID,
        plugin_version: STARTER_PLUGIN_VERSION,
        tool_name: STARTER_PLUGIN_TEXT_STATS_TOOL_NAME,
        input_schema_id: STARTER_PLUGIN_TEXT_STATS_INPUT_SCHEMA_ID,
        success_output_schema_id: STARTER_PLUGIN_TEXT_STATS_OUTPUT_SCHEMA_ID,
        refusal_schema_ids: TEXT_STATS_REFUSAL_SCHEMA_IDS,
        replay_class_id: "deterministic_replayable",
        capability_class: StarterPluginCapabilityClass::LocalDeterministic,
        authoring_class: StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic,
        origin_class: StarterPluginOriginClass::UserAdded,
        capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
        negative_claim_ids: TEXT_STATS_NEGATIVE_CLAIM_IDS,
        mount_envelope_id: "mount.plugin.text.stats.no_capabilities.v1",
        manifest_id: "manifest.plugin.text.stats.v1",
        artifact_id: "artifact.plugin.text.stats.v1",
        runtime_bundle_id: "tassadar.post_article.plugin_text_stats.runtime_bundle.v1",
        runtime_bundle_ref: TASSADAR_POST_ARTICLE_PLUGIN_TEXT_STATS_RUNTIME_BUNDLE_REF,
        runtime_run_root_ref: TASSADAR_POST_ARTICLE_PLUGIN_TEXT_STATS_RUN_ROOT_REF,
        tool_description: "count bytes, Unicode scalar values, lines, non-empty lines, and whitespace-delimited words from packet-local text without tokenizer, language, or semantic-structure claims.",
        bridge_exposed: true,
        catalog_exposed: true,
        catalog: Some(StarterPluginCatalogRegistration {
            catalog_entry_id: "plugin.text.stats@v1",
            trust_tier_id: "operator_curated_local_deterministic",
            evidence_posture_id: "evidence.descriptor_fixture_receipt_bound.v1",
            catalog_capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
            descriptor_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_text_stats_descriptor.json",
            fixture_bundle_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_text_stats_fixture_bundle.json",
            sample_mount_envelope_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_text_stats_mount_envelope.json",
            descriptor_detail: "the first user-added starter plugin stays local, deterministic, and capability-free while exposing bounded packet-local counting truth.",
            capability_matrix_detail: "the text-stats starter plugin is a capability-free deterministic local transform over packet-local text only.",
        }),
    },
    StarterPluginRegistration {
        plugin_id: STARTER_PLUGIN_HTTP_FETCH_TEXT_ID,
        plugin_version: STARTER_PLUGIN_VERSION,
        tool_name: STARTER_PLUGIN_HTTP_FETCH_TEXT_TOOL_NAME,
        input_schema_id: STARTER_PLUGIN_HTTP_FETCH_TEXT_INPUT_SCHEMA_ID,
        success_output_schema_id: STARTER_PLUGIN_HTTP_FETCH_TEXT_OUTPUT_SCHEMA_ID,
        refusal_schema_ids: FETCH_TEXT_REFUSAL_SCHEMA_IDS,
        replay_class_id: "replayable_with_snapshots",
        capability_class: StarterPluginCapabilityClass::ReadOnlyNetwork,
        authoring_class: StarterPluginAuthoringClass::NetworkedReadOnly,
        origin_class: StarterPluginOriginClass::UserAdded,
        capability_namespace_ids: FETCH_TEXT_CAPABILITY_NAMESPACE_IDS,
        negative_claim_ids: FETCH_TEXT_NEGATIVE_CLAIM_IDS,
        mount_envelope_id: "mount.plugin.http.fetch_text.read_only_http_allowlist.v1",
        manifest_id: "manifest.plugin.http.fetch_text.v1",
        artifact_id: "artifact.plugin.http.fetch_text.v1",
        runtime_bundle_id: "tassadar.post_article.plugin_http_fetch_text.runtime_bundle.v1",
        runtime_bundle_ref: TASSADAR_POST_ARTICLE_PLUGIN_HTTP_FETCH_TEXT_RUNTIME_BUNDLE_REF,
        runtime_run_root_ref: TASSADAR_POST_ARTICLE_PLUGIN_HTTP_FETCH_TEXT_RUN_ROOT_REF,
        tool_description: "fetch allowlisted text content through a host-mediated read-only HTTP mount without browser execution, cookies, auth sessions, or unrestricted network access.",
        bridge_exposed: true,
        catalog_exposed: true,
        catalog: Some(StarterPluginCatalogRegistration {
            catalog_entry_id: "plugin.http.fetch_text@v1",
            trust_tier_id: "operator_curated_network_read_only",
            evidence_posture_id: "evidence.descriptor_envelope_receipt_bound.v1",
            catalog_capability_namespace_ids: FETCH_TEXT_CATALOG_CAPABILITY_NAMESPACE_IDS,
            descriptor_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_http_fetch_text_descriptor.json",
            fixture_bundle_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_http_fetch_text_fixture_bundle.json",
            sample_mount_envelope_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_http_fetch_text_mount_envelope.json",
            descriptor_detail: "the first manual user-added network starter plugin freezes GET-only read-only text fetch behind host-mediated allowlist, timeout, redirect, and response-size policy.",
            capability_matrix_detail: "the fetch-text starter plugin is the only networked starter entry, remains bounded to host-mediated read-only HTTP, and now proves the first manual user-added networked lane.",
        }),
    },
    StarterPluginRegistration {
        plugin_id: STARTER_PLUGIN_GUEST_ECHO_ID,
        plugin_version: PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_PLUGIN_VERSION,
        tool_name: STARTER_PLUGIN_GUEST_ECHO_TOOL_NAME,
        input_schema_id: PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_INPUT_SCHEMA_ID,
        success_output_schema_id: PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_OUTPUT_SCHEMA_ID,
        refusal_schema_ids: GUEST_ECHO_REFUSAL_SCHEMA_IDS,
        replay_class_id: PSION_PLUGIN_GUEST_ARTIFACT_REFERENCE_REPLAY_CLASS_ID,
        capability_class: StarterPluginCapabilityClass::LocalDeterministic,
        authoring_class: StarterPluginAuthoringClass::GuestArtifactDigestBound,
        origin_class: StarterPluginOriginClass::UserAdded,
        capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
        negative_claim_ids: GUEST_ECHO_NEGATIVE_CLAIM_IDS,
        mount_envelope_id: "psion.plugin_guest_artifact.mount_envelope.reference.v1",
        manifest_id: "psion_plugin_guest_artifact_manifest_reference",
        artifact_id: "artifact.plugin.example.echo_guest.v1",
        runtime_bundle_id: "psion.plugin_guest_artifact.invocation.v1",
        runtime_bundle_ref: PSION_PLUGIN_GUEST_ARTIFACT_INVOCATION_REF,
        runtime_run_root_ref: PSION_PLUGIN_GUEST_ARTIFACT_RUNTIME_RUN_ROOT_REF,
        tool_description: "echo one bounded text field through the digest-bound guest-artifact packet lane while preserving the shared starter-plugin receipt envelope.",
        bridge_exposed: true,
        catalog_exposed: true,
        catalog: Some(StarterPluginCatalogRegistration {
            catalog_entry_id: "plugin.example.echo_guest@v1",
            trust_tier_id: "operator_reviewed_guest_artifact_digest_bound_internal_only",
            evidence_posture_id: "evidence.manifest_digest_invocation_receipt_bound.v1",
            catalog_capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
            descriptor_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_example_echo_guest_descriptor.json",
            fixture_bundle_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_example_echo_guest_fixture_bundle.json",
            sample_mount_envelope_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_example_echo_guest_mount_envelope.json",
            descriptor_detail: "the first user-provided guest-artifact starter plugin stays digest-bound, operator-reviewed, local deterministic at runtime, and receipt-equivalent to the host-native starter surface.",
            capability_matrix_detail: "the guest echo starter plugin is capability-free and deterministic at runtime but remains a separate digest-bound guest-artifact lane instead of collapsing into host-native plugin truth.",
        }),
    },
    StarterPluginRegistration {
        plugin_id: STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID,
        plugin_version: STARTER_PLUGIN_VERSION,
        tool_name: STARTER_PLUGIN_HTML_EXTRACT_READABLE_TOOL_NAME,
        input_schema_id: STARTER_PLUGIN_HTML_EXTRACT_READABLE_INPUT_SCHEMA_ID,
        success_output_schema_id: STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID,
        refusal_schema_ids: EXTRACT_READABLE_REFUSAL_SCHEMA_IDS,
        replay_class_id: "deterministic_replayable",
        capability_class: StarterPluginCapabilityClass::LocalDeterministic,
        authoring_class: StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic,
        origin_class: StarterPluginOriginClass::OperatorBuiltin,
        capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
        negative_claim_ids: EXTRACT_READABLE_NEGATIVE_CLAIM_IDS,
        mount_envelope_id: "mount.plugin.html.extract_readable.no_capabilities.v1",
        manifest_id: "manifest.plugin.html.extract_readable.v1",
        artifact_id: "artifact.plugin.html.extract_readable.v1",
        runtime_bundle_id: "tassadar.post_article.plugin_html_extract_readable.runtime_bundle.v1",
        runtime_bundle_ref: TASSADAR_POST_ARTICLE_PLUGIN_HTML_EXTRACT_READABLE_RUNTIME_BUNDLE_REF,
        runtime_run_root_ref: TASSADAR_POST_ARTICLE_PLUGIN_HTML_EXTRACT_READABLE_RUN_ROOT_REF,
        tool_description: "extract already-fetched HTML into bounded readable text, metadata, and harvested links without browser rendering, JavaScript, or full DOM claims.",
        bridge_exposed: true,
        catalog_exposed: true,
        catalog: Some(StarterPluginCatalogRegistration {
            catalog_entry_id: "plugin.html.extract_readable@v1",
            trust_tier_id: "operator_curated_local_transform",
            evidence_posture_id: "evidence.descriptor_fixture_receipt_bound.v1",
            catalog_capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
            descriptor_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_html_extract_readable_descriptor.json",
            fixture_bundle_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_html_extract_readable_fixture_bundle.json",
            sample_mount_envelope_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_html_extract_readable_mount_envelope.json",
            descriptor_detail: "the readable-html starter plugin stays local, deterministic, and packet-only while producing bounded readable text, metadata, and harvested links.",
            capability_matrix_detail: "the readability extractor is a local deterministic transform over already-fetched content.",
        }),
    },
    StarterPluginRegistration {
        plugin_id: STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID,
        plugin_version: STARTER_PLUGIN_VERSION,
        tool_name: STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_TOOL_NAME,
        input_schema_id: STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_INPUT_SCHEMA_ID,
        success_output_schema_id: STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID,
        refusal_schema_ids: FEED_PARSE_REFUSAL_SCHEMA_IDS,
        replay_class_id: "deterministic_replayable",
        capability_class: StarterPluginCapabilityClass::LocalDeterministic,
        authoring_class: StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic,
        origin_class: StarterPluginOriginClass::OperatorBuiltin,
        capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
        negative_claim_ids: FEED_PARSE_NEGATIVE_CLAIM_IDS,
        mount_envelope_id: "mount.plugin.feed.rss_atom_parse.no_capabilities.v1",
        manifest_id: "manifest.plugin.feed.rss_atom_parse.v1",
        artifact_id: "artifact.plugin.feed.rss_atom_parse.v1",
        runtime_bundle_id: "tassadar.post_article.plugin_feed_rss_atom_parse.runtime_bundle.v1",
        runtime_bundle_ref: TASSADAR_POST_ARTICLE_PLUGIN_FEED_RSS_ATOM_PARSE_RUNTIME_BUNDLE_REF,
        runtime_run_root_ref: TASSADAR_POST_ARTICLE_PLUGIN_FEED_RSS_ATOM_PARSE_RUN_ROOT_REF,
        tool_description: "parse already-fetched RSS 2.0 or Atom 1.0 documents into bounded feed metadata and entry rows without network access or general XML claims.",
        bridge_exposed: true,
        catalog_exposed: true,
        catalog: Some(StarterPluginCatalogRegistration {
            catalog_entry_id: "plugin.feed.rss_atom_parse@v1",
            trust_tier_id: "operator_curated_local_structured_ingest",
            evidence_posture_id: "evidence.descriptor_fixture_receipt_bound.v1",
            catalog_capability_namespace_ids: NO_CAPABILITY_NAMESPACE_IDS,
            descriptor_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_feed_rss_atom_parse_descriptor.json",
            fixture_bundle_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_feed_rss_atom_parse_fixture_bundle.json",
            sample_mount_envelope_ref: "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_catalog_v1/plugin_feed_rss_atom_parse_mount_envelope.json",
            descriptor_detail: "the feed parser starter plugin stays local and deterministic over already-fetched RSS 2.0 and Atom 1.0 content.",
            capability_matrix_detail: "the feed parser is a local deterministic structured-ingest transform over already-fetched content.",
        }),
    },
];

#[must_use]
pub fn starter_plugin_registrations() -> &'static [StarterPluginRegistration] {
    STARTER_PLUGIN_REGISTRATIONS
}

#[must_use]
pub fn starter_plugin_registration_by_plugin_id(
    plugin_id: &str,
) -> Option<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .find(|registration| registration.plugin_id == plugin_id)
}

#[must_use]
pub fn starter_plugin_registration_by_tool_name(
    tool_name: &str,
) -> Option<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .find(|registration| registration.tool_name == tool_name)
}

#[must_use]
pub fn bridge_exposed_starter_plugin_registrations() -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| registration.bridge_exposed)
        .collect()
}

#[must_use]
pub fn catalog_exposed_starter_plugin_registrations() -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| registration.catalog_exposed)
        .collect()
}

#[must_use]
pub fn capability_free_starter_plugin_registrations() -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| {
            registration.authoring_class
                == StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic
        })
        .collect()
}

#[must_use]
pub fn networked_starter_plugin_registrations() -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| {
            registration.authoring_class == StarterPluginAuthoringClass::NetworkedReadOnly
        })
        .collect()
}

#[must_use]
pub fn guest_artifact_starter_plugin_registrations() -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| {
            registration.authoring_class == StarterPluginAuthoringClass::GuestArtifactDigestBound
        })
        .collect()
}

#[must_use]
pub fn user_added_starter_plugin_registrations() -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| registration.origin_class == StarterPluginOriginClass::UserAdded)
        .collect()
}

#[must_use]
pub fn weighted_controller_admissible_user_added_starter_plugin_registrations(
) -> Vec<&'static StarterPluginRegistration> {
    STARTER_PLUGIN_REGISTRATIONS
        .iter()
        .filter(|registration| {
            registration.origin_class == StarterPluginOriginClass::UserAdded
                && matches!(
                    registration.authoring_class,
                    StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic
                        | StarterPluginAuthoringClass::NetworkedReadOnly
                        | StarterPluginAuthoringClass::GuestArtifactDigestBound
                )
                && registration.bridge_exposed
                && registration.catalog_exposed
                && registration.catalog.is_some()
        })
        .collect()
}

fn starter_plugin_registration(plugin_id: &str) -> &'static StarterPluginRegistration {
    starter_plugin_registration_by_plugin_id(plugin_id)
        .unwrap_or_else(|| panic!("missing starter-plugin registration for `{plugin_id}`"))
}

fn starter_plugin_tool_projection_from_registration(
    registration: &StarterPluginRegistration,
    arguments_schema: Value,
) -> StarterPluginToolProjection {
    StarterPluginToolProjection {
        plugin_id: String::from(registration.plugin_id),
        tool_name: String::from(registration.tool_name),
        description: String::from(registration.tool_description),
        arguments_schema,
        result_schema_id: String::from(registration.success_output_schema_id),
        refusal_schema_ids: registration
            .refusal_schema_ids
            .iter()
            .map(|schema_id| String::from(*schema_id))
            .collect(),
        replay_class_id: String::from(registration.replay_class_id),
    }
}

fn starter_plugin_string_ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| String::from(*id)).collect()
}

#[must_use]
pub fn starter_plugin_tool_projection_for_plugin_id(
    plugin_id: &str,
) -> Option<StarterPluginToolProjection> {
    match plugin_id {
        STARTER_PLUGIN_TEXT_URL_EXTRACT_ID => Some(url_extract_tool_projection()),
        STARTER_PLUGIN_TEXT_STATS_ID => Some(text_stats_tool_projection()),
        STARTER_PLUGIN_HTTP_FETCH_TEXT_ID => Some(fetch_text_tool_projection()),
        STARTER_PLUGIN_GUEST_ECHO_ID => Some(guest_echo_tool_projection()),
        STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID => Some(extract_readable_tool_projection()),
        STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID => Some(feed_parse_tool_projection()),
        _ => None,
    }
}

#[must_use]
pub fn starter_plugin_tool_projection_for_tool_name(
    tool_name: &str,
) -> Option<StarterPluginToolProjection> {
    starter_plugin_registration_by_tool_name(tool_name).and_then(|registration| {
        starter_plugin_tool_projection_for_plugin_id(registration.plugin_id)
    })
}

fn starter_plugin_receipt_from_registration(
    registration: &StarterPluginRegistration,
    mount_envelope_id: &str,
    replay_class_id: &str,
    status: StarterPluginInvocationStatus,
    input_packet_digest: String,
    output_or_refusal_schema_id: &str,
    output_or_refusal_digest: String,
    refusal_class_id: Option<&str>,
    detail: impl Into<String>,
    digest_prefix: &[u8],
) -> StarterPluginInvocationReceipt {
    let detail = detail.into();
    let mut receipt = StarterPluginInvocationReceipt {
        receipt_id: format!(
            "receipt.{}.{}.v1",
            registration.plugin_id,
            &input_packet_digest[..16]
        ),
        plugin_id: String::from(registration.plugin_id),
        plugin_version: String::from(registration.plugin_version),
        tool_name: String::from(registration.tool_name),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        mount_envelope_id: String::from(mount_envelope_id),
        capability_namespace_ids: starter_plugin_string_ids(registration.capability_namespace_ids),
        replay_class_id: String::from(replay_class_id),
        status,
        input_schema_id: String::from(registration.input_schema_id),
        input_packet_digest,
        output_or_refusal_schema_id: String::from(output_or_refusal_schema_id),
        output_or_refusal_digest,
        refusal_class_id: refusal_class_id.map(String::from),
        detail,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_json_digest(digest_prefix, &receipt);
    receipt
}

#[must_use]
pub fn guest_echo_tool_projection() -> StarterPluginToolProjection {
    psion_plugin_guest_artifact_tool_projection(&reference_psion_plugin_guest_artifact_manifest())
}

#[must_use]
pub fn url_extract_tool_projection() -> StarterPluginToolProjection {
    starter_plugin_tool_projection_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_TEXT_URL_EXTRACT_ID),
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["text"],
            "properties": {
                "text": {
                    "type": "string",
                    "description": "packet-local input text scanned with the bounded https?://[^\\\\s]+ rule."
                }
            }
        }),
    )
}

#[must_use]
pub fn invoke_url_extract_json_packet(
    codec_id: &str,
    packet_bytes: &[u8],
    config: &UrlExtractConfig,
) -> UrlExtractInvocationOutcome {
    let input_packet_digest = sha256_digest(packet_bytes);
    if codec_id != "json" {
        return url_extract_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
            "unsupported_codec",
            "url extract accepts only json packet input under packet.v1.",
        );
    }
    if packet_bytes.len() > config.packet_size_limit_bytes {
        return url_extract_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
            "packet_too_large",
            "url extract keeps packet size ceilings explicit instead of relying on ambient parser allocation behavior.",
        );
    }
    let request = match serde_json::from_slice::<UrlExtractRequest>(packet_bytes) {
        Ok(request) => request,
        Err(_) => {
            return url_extract_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "url extract refuses malformed packets without host-side schema repair.",
            );
        }
    };

    let urls = match extract_urls(&request.text, config.max_urls) {
        Ok(urls) => urls,
        Err(refusal) => {
            return url_extract_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID,
                "runtime_resource_limit",
                refusal,
            );
        }
    };
    let response = UrlExtractResponse { urls };
    let output_or_refusal_digest = stable_json_digest(b"url_extract_response|", &response);
    let receipt = starter_plugin_receipt_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_TEXT_URL_EXTRACT_ID),
        starter_plugin_registration(STARTER_PLUGIN_TEXT_URL_EXTRACT_ID).mount_envelope_id,
        starter_plugin_registration(STARTER_PLUGIN_TEXT_URL_EXTRACT_ID).replay_class_id,
        StarterPluginInvocationStatus::Success,
        input_packet_digest,
        STARTER_PLUGIN_TEXT_URL_EXTRACT_OUTPUT_SCHEMA_ID,
        output_or_refusal_digest,
        None,
        String::from(
            "url extract keeps the legacy left-to-right https?://[^\\s]+ rule with duplicate preservation and no network semantics.",
        ),
        b"url_extract_receipt|",
    );
    UrlExtractInvocationOutcome {
        receipt,
        response: Some(response),
        refusal: None,
    }
}

#[must_use]
pub fn build_url_extract_runtime_bundle() -> UrlExtractRuntimeBundle {
    let success_packet = br#"{"text":"Read https://alpha.example/a then http://beta.test/b and revisit https://alpha.example/a"}"#;
    let malformed_packet = br#"{"body":"missing text"}"#;
    let too_many_urls_packet = br#"{"text":"https://alpha.example/a https://beta.example/b"}"#;
    let oversized_packet = oversized_url_extract_packet(UrlExtractConfig::default());

    let success =
        invoke_url_extract_json_packet("json", success_packet, &UrlExtractConfig::default());
    let malformed =
        invoke_url_extract_json_packet("json", malformed_packet, &UrlExtractConfig::default());
    let packet_too_large =
        invoke_url_extract_json_packet("json", &oversized_packet, &UrlExtractConfig::default());
    let unsupported_codec =
        invoke_url_extract_json_packet("bytes", success_packet, &UrlExtractConfig::default());
    let runtime_resource_limit = invoke_url_extract_json_packet(
        "json",
        too_many_urls_packet,
        &UrlExtractConfig {
            packet_size_limit_bytes: UrlExtractConfig::default().packet_size_limit_bytes,
            max_urls: 1,
        },
    );

    let case_rows = vec![
        url_extract_case(
            "extract_urls_success",
            UrlExtractRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(success_packet),
            STARTER_PLUGIN_TEXT_URL_EXTRACT_OUTPUT_SCHEMA_ID,
            success.receipt.output_or_refusal_digest.clone(),
            success.receipt.clone(),
            "the runtime preserves left-to-right output order and duplicate posture for literal http(s) strings.",
        ),
        url_extract_case(
            "schema_invalid_missing_text",
            UrlExtractRuntimeCaseStatus::TypedMalformedPacket,
            "json",
            sha256_digest(malformed_packet),
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
            malformed.receipt.output_or_refusal_digest.clone(),
            malformed.receipt.clone(),
            "missing `text` fails closed into a typed schema-invalid refusal.",
        ),
        url_extract_case(
            "packet_too_large_refusal",
            UrlExtractRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(&oversized_packet),
            STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
            packet_too_large.receipt.output_or_refusal_digest.clone(),
            packet_too_large.receipt.clone(),
            "packet ceilings stay explicit rather than relying on ambient parser behavior.",
        ),
        url_extract_case(
            "unsupported_codec_refusal",
            UrlExtractRuntimeCaseStatus::TypedRefusal,
            "bytes",
            sha256_digest(success_packet),
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
            unsupported_codec.receipt.output_or_refusal_digest.clone(),
            unsupported_codec.receipt.clone(),
            "unsupported codecs remain typed refusal truth instead of host-side best effort decode.",
        ),
        url_extract_case(
            "runtime_resource_limit_refusal",
            UrlExtractRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(too_many_urls_packet),
            STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID,
            runtime_resource_limit
                .receipt
                .output_or_refusal_digest
                .clone(),
            runtime_resource_limit.receipt.clone(),
            "bounded output ceilings fail closed into one typed runtime-resource-limit refusal.",
        ),
    ];

    let registration = starter_plugin_registration(STARTER_PLUGIN_TEXT_URL_EXTRACT_ID);
    let mut bundle = UrlExtractRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from(registration.runtime_bundle_id),
        plugin_id: String::from(registration.plugin_id),
        plugin_version: String::from(registration.plugin_version),
        manifest_id: String::from(registration.manifest_id),
        artifact_id: String::from(registration.artifact_id),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        mount_envelope_id: String::from(registration.mount_envelope_id),
        tool_projection: url_extract_tool_projection(),
        negative_claim_ids: starter_plugin_string_ids(registration.negative_claim_ids),
        case_rows,
        claim_boundary: String::from(
            "this runtime bundle closes one capability-free starter plugin that extracts literal http:// and https:// substrings from packet-local text under one deterministic regex rule. It does not claim URL validation, DNS truth, redirect truth, or any network semantics.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "url extract runtime bundle covers {} cases across success={}, malformed={}, refusals={}.",
        bundle.case_rows.len(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == UrlExtractRuntimeCaseStatus::ExactSuccess)
            .count(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == UrlExtractRuntimeCaseStatus::TypedMalformedPacket)
            .count(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == UrlExtractRuntimeCaseStatus::TypedRefusal)
            .count(),
    );
    bundle.bundle_digest = stable_json_digest(b"url_extract_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_text_url_extract_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_TEXT_URL_EXTRACT_RUNTIME_BUNDLE_REF)
}

pub fn write_url_extract_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<UrlExtractRuntimeBundle, StarterPluginRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| StarterPluginRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = build_url_extract_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_url_extract_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<UrlExtractRuntimeBundle, StarterPluginRuntimeError> {
    read_json(path)
}

#[must_use]
pub fn text_stats_tool_projection() -> StarterPluginToolProjection {
    starter_plugin_tool_projection_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_TEXT_STATS_ID),
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["text"],
            "properties": {
                "text": {
                    "type": "string",
                    "description": "packet-local text counted with Rust byte length, chars(), lines(), and split_whitespace() semantics."
                }
            }
        }),
    )
}

#[must_use]
pub fn invoke_text_stats_json_packet(
    codec_id: &str,
    packet_bytes: &[u8],
    config: &TextStatsConfig,
) -> TextStatsInvocationOutcome {
    let input_packet_digest = sha256_digest(packet_bytes);
    if codec_id != "json" {
        return text_stats_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
            "unsupported_codec",
            "text-stats accepts only json packet input under packet.v1.",
        );
    }
    if packet_bytes.len() > config.packet_size_limit_bytes {
        return text_stats_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
            "packet_too_large",
            "text-stats keeps packet size ceilings explicit instead of relying on ambient parser allocation behavior.",
        );
    }
    let request = match serde_json::from_slice::<TextStatsRequest>(packet_bytes) {
        Ok(request) => request,
        Err(_) => {
            return text_stats_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "text-stats refuses malformed packets without host-side schema repair.",
            );
        }
    };

    let response = text_stats_response(&request.text);
    let output_or_refusal_digest = stable_json_digest(b"text_stats_response|", &response);
    let registration = starter_plugin_registration(STARTER_PLUGIN_TEXT_STATS_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Success,
        input_packet_digest,
        STARTER_PLUGIN_TEXT_STATS_OUTPUT_SCHEMA_ID,
        output_or_refusal_digest,
        None,
        String::from(
            "text-stats keeps byte, scalar, line, and whitespace-delimited word counts explicit and packet-local instead of borrowing tokenizer or semantic structure claims.",
        ),
        b"text_stats_receipt|",
    );
    TextStatsInvocationOutcome {
        receipt,
        response: Some(response),
        refusal: None,
    }
}

#[must_use]
pub fn build_text_stats_runtime_bundle() -> TextStatsRuntimeBundle {
    let success_packet = br#"{"text":"alpha beta\n\ngamma delta"}"#;
    let malformed_packet = br#"{"body":"missing text"}"#;
    let oversized_packet = oversized_text_stats_packet(TextStatsConfig::default());

    let success =
        invoke_text_stats_json_packet("json", success_packet, &TextStatsConfig::default());
    let malformed =
        invoke_text_stats_json_packet("json", malformed_packet, &TextStatsConfig::default());
    let packet_too_large =
        invoke_text_stats_json_packet("json", &oversized_packet, &TextStatsConfig::default());
    let unsupported_codec =
        invoke_text_stats_json_packet("bytes", success_packet, &TextStatsConfig::default());

    let case_rows = vec![
        text_stats_case(
            "text_stats_success",
            TextStatsRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(success_packet),
            STARTER_PLUGIN_TEXT_STATS_OUTPUT_SCHEMA_ID,
            success.receipt.output_or_refusal_digest.clone(),
            success.receipt.clone(),
            "the runtime counts bytes, Unicode scalar values, lines, non-empty lines, and split_whitespace() words deterministically from packet-local text.",
        ),
        text_stats_case(
            "schema_invalid_missing_text",
            TextStatsRuntimeCaseStatus::TypedMalformedPacket,
            "json",
            sha256_digest(malformed_packet),
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
            malformed.receipt.output_or_refusal_digest.clone(),
            malformed.receipt.clone(),
            "missing `text` fails closed into one typed schema-invalid refusal.",
        ),
        text_stats_case(
            "packet_too_large_refusal",
            TextStatsRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(&oversized_packet),
            STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
            packet_too_large.receipt.output_or_refusal_digest.clone(),
            packet_too_large.receipt.clone(),
            "packet ceilings remain explicit instead of relying on ambient parser allocation behavior.",
        ),
        text_stats_case(
            "unsupported_codec_refusal",
            TextStatsRuntimeCaseStatus::TypedRefusal,
            "bytes",
            sha256_digest(success_packet),
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
            unsupported_codec.receipt.output_or_refusal_digest.clone(),
            unsupported_codec.receipt.clone(),
            "unsupported codecs remain typed refusal truth instead of host-side best effort decode.",
        ),
    ];

    let registration = starter_plugin_registration(STARTER_PLUGIN_TEXT_STATS_ID);
    let mut bundle = TextStatsRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from(registration.runtime_bundle_id),
        plugin_id: String::from(registration.plugin_id),
        plugin_version: String::from(registration.plugin_version),
        manifest_id: String::from(registration.manifest_id),
        artifact_id: String::from(registration.artifact_id),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        mount_envelope_id: String::from(registration.mount_envelope_id),
        tool_projection: text_stats_tool_projection(),
        negative_claim_ids: starter_plugin_string_ids(registration.negative_claim_ids),
        case_rows,
        claim_boundary: String::from(
            "this runtime bundle closes one capability-free starter plugin that counts bytes, Unicode scalar values, lines, non-empty lines, and whitespace-delimited words from packet-local text. It does not claim tokenizer truth, language detection, sentence-boundary truth, or semantic structure.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "text-stats runtime bundle covers {} cases across success={}, malformed={}, refusals={}.",
        bundle.case_rows.len(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == TextStatsRuntimeCaseStatus::ExactSuccess)
            .count(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == TextStatsRuntimeCaseStatus::TypedMalformedPacket)
            .count(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == TextStatsRuntimeCaseStatus::TypedRefusal)
            .count(),
    );
    bundle.bundle_digest = stable_json_digest(b"text_stats_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_text_stats_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_TEXT_STATS_RUNTIME_BUNDLE_REF)
}

pub fn write_text_stats_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TextStatsRuntimeBundle, StarterPluginRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| StarterPluginRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = build_text_stats_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_text_stats_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<TextStatsRuntimeBundle, StarterPluginRuntimeError> {
    read_json(path)
}

impl FetchTextConfig {
    #[must_use]
    pub fn snapshot(entries: BTreeMap<String, FetchTextSnapshotResult>) -> Self {
        Self {
            mount_envelope: default_fetch_text_mount_envelope("replayable_with_snapshots"),
            backend: FetchTextBackend::Snapshot(entries),
        }
    }

    #[must_use]
    pub fn live(allowlisted_url_prefixes: Vec<String>) -> Self {
        let mut mount_envelope = default_fetch_text_mount_envelope("operator_replay_only");
        mount_envelope.allowlisted_url_prefixes = allowlisted_url_prefixes;
        Self {
            mount_envelope,
            backend: FetchTextBackend::Live,
        }
    }
}

#[must_use]
pub fn fetch_text_tool_projection() -> StarterPluginToolProjection {
    starter_plugin_tool_projection_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_HTTP_FETCH_TEXT_ID),
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["url"],
            "properties": {
                "url": {
                    "type": "string",
                    "description": "allowlisted absolute URL fetched with one bounded GET-only runtime path."
                }
            }
        }),
    )
}

pub fn invoke_fetch_text_json_packet(
    codec_id: &str,
    packet_bytes: &[u8],
    config: &FetchTextConfig,
) -> FetchTextInvocationOutcome {
    let input_packet_digest = sha256_digest(packet_bytes);
    if codec_id != "json" {
        return fetch_text_refusal_outcome(
            &input_packet_digest,
            backend_id(config),
            replay_class_id(config),
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
            "unsupported_codec",
            "fetch-text accepts only json packet input under packet.v1.",
        );
    }
    let request = match serde_json::from_slice::<FetchTextRequest>(packet_bytes) {
        Ok(request) => request,
        Err(_) => {
            return fetch_text_refusal_outcome(
                &input_packet_digest,
                backend_id(config),
                replay_class_id(config),
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "fetch-text refuses malformed packets without host-side schema repair.",
            );
        }
    };
    let url = match Url::parse(&request.url) {
        Ok(url) => url,
        Err(_) => {
            return fetch_text_refusal_outcome(
                &input_packet_digest,
                backend_id(config),
                replay_class_id(config),
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "fetch-text requires one absolute URL string.",
            );
        }
    };
    if !url_allowed(url.as_str(), &config.mount_envelope) {
        return fetch_text_refusal_outcome(
            &input_packet_digest,
            backend_id(config),
            replay_class_id(config),
            STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID,
            "url_not_permitted",
            "fetch-text keeps URL allowlists in the host-owned mount envelope instead of guest packets.",
        );
    }

    let result = match &config.backend {
        FetchTextBackend::Snapshot(entries) => invoke_snapshot_fetch(&url, entries),
        FetchTextBackend::Live => invoke_live_fetch(&url, &config.mount_envelope),
    };
    match result {
        Ok(response) => {
            let output_or_refusal_digest = stable_json_digest(b"fetch_text_response|", &response);
            let receipt = starter_plugin_receipt_from_registration(
                starter_plugin_registration(STARTER_PLUGIN_HTTP_FETCH_TEXT_ID),
                &config.mount_envelope.envelope_id,
                replay_class_id(config),
                StarterPluginInvocationStatus::Success,
                input_packet_digest,
                STARTER_PLUGIN_HTTP_FETCH_TEXT_OUTPUT_SCHEMA_ID,
                output_or_refusal_digest,
                None,
                String::from(
                    "fetch-text keeps GET-only host-mediated access explicit and returns structured text-fetch truth without browser semantics.",
                ),
                b"fetch_text_receipt|",
            );
            FetchTextInvocationOutcome {
                receipt,
                backend_id: String::from(backend_id(config)),
                logical_cpu_millis: 2,
                response: Some(response),
                refusal: None,
            }
        }
        Err((schema_id, refusal_class_id, detail)) => fetch_text_refusal_outcome(
            &input_packet_digest,
            backend_id(config),
            replay_class_id(config),
            schema_id,
            refusal_class_id,
            detail,
        ),
    }
}

#[must_use]
pub fn build_fetch_text_runtime_bundle() -> FetchTextRuntimeBundle {
    let snapshot_entries = BTreeMap::from([
        (
            String::from("https://snapshot.example/article"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/article"),
                status_code: 200,
                content_type: String::from("text/html"),
                charset: Some(String::from("utf-8")),
                body_bytes: b"<html><title>Snapshot Article</title><body><h1>Snapshot Article</h1><p>Bounded starter plugin content.</p></body></html>".to_vec(),
            }),
        ),
        (
            String::from("https://snapshot.example/slow"),
            FetchTextSnapshotResult::Timeout,
        ),
        (
            String::from("https://snapshot.example/binary"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/binary"),
                status_code: 200,
                content_type: String::from("image/png"),
                charset: None,
                body_bytes: vec![0x89, 0x50, 0x4e, 0x47],
            }),
        ),
        (
            String::from("https://snapshot.example/large"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/large"),
                status_code: 200,
                content_type: String::from("text/plain"),
                charset: Some(String::from("utf-8")),
                body_bytes: vec![b'x'; 16 * 1024 + 8],
            }),
        ),
        (
            String::from("https://snapshot.example/bad-utf8"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/bad-utf8"),
                status_code: 200,
                content_type: String::from("text/plain"),
                charset: Some(String::from("utf-8")),
                body_bytes: vec![0xff, 0xfe, 0xfd],
            }),
        ),
        (
            String::from("https://snapshot.example/broken"),
            FetchTextSnapshotResult::UpstreamFailure {
                detail: String::from("the mounted snapshot marks this URL as an upstream transport failure."),
            },
        ),
    ]);
    let snapshot_config = FetchTextConfig::snapshot(snapshot_entries);

    let success_packet = br#"{"url":"https://snapshot.example/article"}"#;
    let malformed_packet = br#"{"uri":"https://snapshot.example/article"}"#;
    let blocked_packet = br#"{"url":"https://blocked.example/article"}"#;
    let timeout_packet = br#"{"url":"https://snapshot.example/slow"}"#;
    let missing_packet = br#"{"url":"https://snapshot.example/missing"}"#;
    let large_packet = br#"{"url":"https://snapshot.example/large"}"#;
    let bad_type_packet = br#"{"url":"https://snapshot.example/binary"}"#;
    let bad_decode_packet = br#"{"url":"https://snapshot.example/bad-utf8"}"#;
    let upstream_failure_packet = br#"{"url":"https://snapshot.example/broken"}"#;

    let success = invoke_fetch_text_json_packet("json", success_packet, &snapshot_config);
    let malformed = invoke_fetch_text_json_packet("json", malformed_packet, &snapshot_config);
    let blocked = invoke_fetch_text_json_packet("json", blocked_packet, &snapshot_config);
    let timeout = invoke_fetch_text_json_packet("json", timeout_packet, &snapshot_config);
    let network_denied = invoke_fetch_text_json_packet("json", missing_packet, &snapshot_config);
    let response_too_large = invoke_fetch_text_json_packet("json", large_packet, &snapshot_config);
    let content_type_unsupported =
        invoke_fetch_text_json_packet("json", bad_type_packet, &snapshot_config);
    let decode_failed = invoke_fetch_text_json_packet("json", bad_decode_packet, &snapshot_config);
    let upstream_failure =
        invoke_fetch_text_json_packet("json", upstream_failure_packet, &snapshot_config);

    let case_rows = vec![
        fetch_text_case(
            "fetch_text_article_success",
            FetchTextRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(success_packet),
            STARTER_PLUGIN_HTTP_FETCH_TEXT_OUTPUT_SCHEMA_ID,
            success.receipt.output_or_refusal_digest.clone(),
            success.backend_id.clone(),
            success.logical_cpu_millis,
            success.receipt.clone(),
            "snapshot-backed fetch-text returns structured host-mediated text-fetch truth.",
        ),
        fetch_text_case(
            "schema_invalid_missing_url",
            FetchTextRuntimeCaseStatus::TypedMalformedPacket,
            "json",
            sha256_digest(malformed_packet),
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
            malformed.receipt.output_or_refusal_digest.clone(),
            malformed.backend_id.clone(),
            malformed.logical_cpu_millis,
            malformed.receipt.clone(),
            "missing `url` fails closed into a typed schema-invalid refusal.",
        ),
        fetch_text_case(
            "url_not_permitted_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(blocked_packet),
            STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID,
            blocked.receipt.output_or_refusal_digest.clone(),
            blocked.backend_id.clone(),
            blocked.logical_cpu_millis,
            blocked.receipt.clone(),
            "URL allowlists stay bound to the mount envelope instead of guest packets.",
        ),
        fetch_text_case(
            "timeout_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(timeout_packet),
            STARTER_PLUGIN_REFUSAL_TIMEOUT_ID,
            timeout.receipt.output_or_refusal_digest.clone(),
            timeout.backend_id.clone(),
            timeout.logical_cpu_millis,
            timeout.receipt.clone(),
            "timeouts stay machine-readable and do not degrade into generic tool text.",
        ),
        fetch_text_case(
            "network_denied_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(missing_packet),
            STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID,
            network_denied.receipt.output_or_refusal_digest.clone(),
            network_denied.backend_id.clone(),
            network_denied.logical_cpu_millis,
            network_denied.receipt.clone(),
            "missing mounted snapshot entries remain explicit network-denied truth.",
        ),
        fetch_text_case(
            "response_too_large_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(large_packet),
            STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID,
            response_too_large.receipt.output_or_refusal_digest.clone(),
            response_too_large.backend_id.clone(),
            response_too_large.logical_cpu_millis,
            response_too_large.receipt.clone(),
            "response-size ceilings remain explicit and fail closed.",
        ),
        fetch_text_case(
            "content_type_unsupported_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(bad_type_packet),
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
            content_type_unsupported
                .receipt
                .output_or_refusal_digest
                .clone(),
            content_type_unsupported.backend_id.clone(),
            content_type_unsupported.logical_cpu_millis,
            content_type_unsupported.receipt.clone(),
            "binary content types remain outside the bounded text-fetch window.",
        ),
        fetch_text_case(
            "decode_failed_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(bad_decode_packet),
            STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID,
            decode_failed.receipt.output_or_refusal_digest.clone(),
            decode_failed.backend_id.clone(),
            decode_failed.logical_cpu_millis,
            decode_failed.receipt.clone(),
            "decode failures remain explicit instead of silently replacing bytes.",
        ),
        fetch_text_case(
            "upstream_failure_refusal",
            FetchTextRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(upstream_failure_packet),
            STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID,
            upstream_failure.receipt.output_or_refusal_digest.clone(),
            upstream_failure.backend_id.clone(),
            upstream_failure.logical_cpu_millis,
            upstream_failure.receipt.clone(),
            "snapshot-declared upstream failures stay typed and machine-readable.",
        ),
    ];

    let registration = starter_plugin_registration(STARTER_PLUGIN_HTTP_FETCH_TEXT_ID);
    let mut bundle = FetchTextRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from(registration.runtime_bundle_id),
        plugin_id: String::from(registration.plugin_id),
        plugin_version: String::from(registration.plugin_version),
        manifest_id: String::from(registration.manifest_id),
        artifact_id: String::from(registration.artifact_id),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        sample_mount_envelope: snapshot_config.mount_envelope.clone(),
        tool_projection: fetch_text_tool_projection(),
        supported_replay_class_ids: vec![
            String::from("replayable_with_snapshots"),
            String::from("operator_replay_only"),
        ],
        negative_claim_ids: starter_plugin_string_ids(registration.negative_claim_ids),
        case_rows,
        claim_boundary: String::from(
            "this runtime bundle closes one read-only network starter plugin that fetches allowlisted text content through a host-mediated HTTP mount. It does not claim browser execution, JavaScript, cookies, auth sessions, arbitrary headers, or unrestricted network access.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "fetch-text runtime bundle covers {} cases across success={}, malformed={}, refusals={} with snapshot-backed replay truth explicit.",
        bundle.case_rows.len(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == FetchTextRuntimeCaseStatus::ExactSuccess)
            .count(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == FetchTextRuntimeCaseStatus::TypedMalformedPacket)
            .count(),
        bundle
            .case_rows
            .iter()
            .filter(|row| row.status == FetchTextRuntimeCaseStatus::TypedRefusal)
            .count(),
    );
    bundle.bundle_digest = stable_json_digest(b"fetch_text_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_http_fetch_text_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_HTTP_FETCH_TEXT_RUNTIME_BUNDLE_REF)
}

pub fn write_fetch_text_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<FetchTextRuntimeBundle, StarterPluginRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| StarterPluginRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = build_fetch_text_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_fetch_text_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<FetchTextRuntimeBundle, StarterPluginRuntimeError> {
    read_json(path)
}

#[must_use]
pub fn extract_readable_tool_projection() -> StarterPluginToolProjection {
    starter_plugin_tool_projection_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID),
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["source_url", "content_type", "body_text"],
            "properties": {
                "source_url": { "type": "string" },
                "content_type": { "type": "string" },
                "body_text": { "type": "string" }
            }
        }),
    )
}

#[must_use]
pub fn invoke_extract_readable_json_packet(
    codec_id: &str,
    packet_bytes: &[u8],
    config: &ExtractReadableConfig,
) -> ExtractReadableInvocationOutcome {
    let input_packet_digest = sha256_digest(packet_bytes);
    if codec_id != "json" {
        return extract_readable_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
            "unsupported_codec",
            "extract-readable accepts only json packet input under packet.v1.",
        );
    }
    if packet_bytes.len() > config.input_size_limit_bytes {
        return extract_readable_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
            "input_too_large",
            "extract-readable keeps input size ceilings explicit instead of relying on parser allocation behavior.",
        );
    }
    let request = match serde_json::from_slice::<ExtractReadableRequest>(packet_bytes) {
        Ok(request) => request,
        Err(_) => {
            return extract_readable_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "extract-readable refuses malformed packets without host-side schema repair.",
            );
        }
    };
    let source_url = match Url::parse(&request.source_url) {
        Ok(url) => url,
        Err(_) => {
            return extract_readable_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "extract-readable requires one absolute `source_url` for canonical-link and link-resolution rules.",
            );
        }
    };
    let (content_type, _) = parse_content_type(&request.content_type);
    if content_type != "text/html" && content_type != "application/xhtml+xml" {
        return extract_readable_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
            "content_type_unsupported",
            "extract-readable stays bounded to html and xhtml input content types.",
        );
    }

    let document = scraper::Html::parse_document(&request.body_text);
    let readable_text = readable_text_from_document(&document);
    let title = first_text_for_selector(&document, "title")
        .or_else(|| meta_content(&document, "property", "og:title"));
    let canonical_url = first_attr_for_selector(&document, r#"link[rel="canonical"]"#, "href")
        .map(|href| resolve_link(&source_url, href.as_str()));
    let site_name = meta_content(&document, "property", "og:site_name")
        .or_else(|| source_url.host_str().map(String::from));
    let excerpt = meta_content(&document, "name", "description")
        .or_else(|| excerpt_from_text(&readable_text));
    let content_language = first_attr_for_selector(&document, "html", "lang")
        .or_else(|| meta_content(&document, "http-equiv", "content-language"));
    let harvested_links = harvested_links_from_document(&document, &source_url, config.max_links);
    let response = ExtractReadableResponse {
        title,
        canonical_url,
        site_name,
        excerpt,
        readable_text,
        harvested_links,
        content_language,
    };
    let output_or_refusal_digest = stable_json_digest(b"extract_readable_response|", &response);
    let registration = starter_plugin_registration(STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Success,
        input_packet_digest,
        STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID,
        output_or_refusal_digest,
        None,
        String::from(
            "extract-readable keeps bounded readability rules explicit and local instead of hiding them in host glue or browser state.",
        ),
        b"extract_readable_receipt|",
    );
    ExtractReadableInvocationOutcome {
        receipt,
        response: Some(response),
        refusal: None,
    }
}

#[must_use]
pub fn build_extract_readable_runtime_bundle() -> ExtractReadableRuntimeBundle {
    let article_html = "<html lang=\"en\"><head><title>Snapshot Article</title><meta name=\"description\" content=\"Bounded HTML extraction fixture.\" /><meta property=\"og:site_name\" content=\"Snapshot Example\" /><link rel=\"canonical\" href=\"/article\" /></head><body><main><h1>Snapshot Article</h1><p>Bounded starter plugin content.</p><a href=\"/alpha\">Alpha</a><a href=\"https://snapshot.example/beta\">Beta</a></main></body></html>";
    let malformed_html =
        "<html><head><title>Broken but readable<title></head><body><article><p>Recovered text<a href=\"/link\">Link";
    let success_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/article",
        "content_type": "text/html; charset=utf-8",
        "body_text": article_html
    }))
    .unwrap_or_else(|_| Vec::new());
    let malformed_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/article",
        "content_type": "text/html",
        "body_text": malformed_html
    }))
    .unwrap_or_else(|_| Vec::new());
    let missing_packet =
        br#"{"source_url":"https://snapshot.example/article","content_type":"text/html"}"#.to_vec();
    let wrong_type_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/article",
        "content_type": "application/json",
        "body_text": "{}"
    }))
    .unwrap_or_else(|_| Vec::new());
    let oversized_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/article",
        "content_type": "text/html",
        "body_text": "x".repeat(64 * 1024)
    }))
    .unwrap_or_else(|_| Vec::new());

    let success = invoke_extract_readable_json_packet(
        "json",
        &success_packet,
        &ExtractReadableConfig::default(),
    );
    let malformed = invoke_extract_readable_json_packet(
        "json",
        &malformed_packet,
        &ExtractReadableConfig::default(),
    );
    let schema_invalid = invoke_extract_readable_json_packet(
        "json",
        &missing_packet,
        &ExtractReadableConfig::default(),
    );
    let wrong_type = invoke_extract_readable_json_packet(
        "json",
        &wrong_type_packet,
        &ExtractReadableConfig::default(),
    );
    let oversized = invoke_extract_readable_json_packet(
        "json",
        &oversized_packet,
        &ExtractReadableConfig {
            input_size_limit_bytes: 1024,
            max_links: 64,
        },
    );

    let fetch_packet = br#"{"url":"https://snapshot.example/article"}"#;
    let fetch_result = invoke_fetch_text_json_packet(
        "json",
        fetch_packet,
        &FetchTextConfig::snapshot(sample_fetch_text_snapshot_entries()),
    );

    let case_rows = vec![
        extract_readable_case(
            "extract_readable_success",
            ExtractReadableRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(&success_packet),
            STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID,
            success.receipt.output_or_refusal_digest.clone(),
            success.receipt.clone(),
            "deterministic readability extraction returns metadata, readable text, and harvested links from fetched HTML.",
        ),
        extract_readable_case(
            "extract_readable_malformed_but_recoverable_success",
            ExtractReadableRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(&malformed_packet),
            STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID,
            malformed.receipt.output_or_refusal_digest.clone(),
            malformed.receipt.clone(),
            "malformed-but-recoverable HTML still resolves under the bounded parser rules without browser semantics.",
        ),
        extract_readable_case(
            "schema_invalid_missing_body_text",
            ExtractReadableRuntimeCaseStatus::TypedMalformedPacket,
            "json",
            sha256_digest(&missing_packet),
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
            schema_invalid.receipt.output_or_refusal_digest.clone(),
            schema_invalid.receipt.clone(),
            "missing `body_text` fails closed into a typed schema-invalid refusal.",
        ),
        extract_readable_case(
            "content_type_unsupported_refusal",
            ExtractReadableRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(&wrong_type_packet),
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
            wrong_type.receipt.output_or_refusal_digest.clone(),
            wrong_type.receipt.clone(),
            "non-html content types remain outside the bounded readability window.",
        ),
        extract_readable_case(
            "input_too_large_refusal",
            ExtractReadableRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(&oversized_packet),
            STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
            oversized.receipt.output_or_refusal_digest.clone(),
            oversized.receipt.clone(),
            "input ceilings remain explicit instead of letting parser recovery define semantics.",
        ),
    ];
    let composition_case = ExtractReadableCompositionCase {
        case_id: String::from("fetch_then_extract_readable"),
        step_plugin_ids: vec![
            String::from(STARTER_PLUGIN_HTTP_FETCH_TEXT_ID),
            String::from(STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID),
        ],
        step_receipt_ids: vec![
            fetch_result.receipt.receipt_id.clone(),
            success.receipt.receipt_id.clone(),
        ],
        schema_repair_allowed: false,
        hidden_host_extraction_allowed: false,
        green: fetch_result.response.is_some() && success.response.is_some(),
        detail: String::from(
            "the fetch-text response binds directly into extract-readable input fields without hidden host schema repair or host-side readability glue.",
        ),
    };

    let registration = starter_plugin_registration(STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID);
    let mut bundle = ExtractReadableRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from(registration.runtime_bundle_id),
        plugin_id: String::from(registration.plugin_id),
        plugin_version: String::from(registration.plugin_version),
        manifest_id: String::from(registration.manifest_id),
        artifact_id: String::from(registration.artifact_id),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        mount_envelope_id: String::from(registration.mount_envelope_id),
        tool_projection: extract_readable_tool_projection(),
        negative_claim_ids: starter_plugin_string_ids(registration.negative_claim_ids),
        case_rows,
        composition_case,
        claim_boundary: String::from(
            "this runtime bundle closes one local deterministic readability extractor over already-fetched HTML. It does not claim browser rendering, JavaScript evaluation, CSS layout truth, or full DOM semantics.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "extract-readable runtime bundle covers {} cases plus one green fetch-to-extract composition harness.",
        bundle.case_rows.len(),
    );
    bundle.bundle_digest = stable_json_digest(b"extract_readable_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_html_extract_readable_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_HTML_EXTRACT_READABLE_RUNTIME_BUNDLE_REF)
}

pub fn write_extract_readable_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<ExtractReadableRuntimeBundle, StarterPluginRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| StarterPluginRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = build_extract_readable_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_extract_readable_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<ExtractReadableRuntimeBundle, StarterPluginRuntimeError> {
    read_json(path)
}

#[must_use]
pub fn feed_parse_tool_projection() -> StarterPluginToolProjection {
    starter_plugin_tool_projection_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID),
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["source_url", "content_type", "feed_text"],
            "properties": {
                "source_url": { "type": "string" },
                "content_type": { "type": "string" },
                "feed_text": { "type": "string" }
            }
        }),
    )
}

#[must_use]
pub fn invoke_feed_parse_json_packet(
    codec_id: &str,
    packet_bytes: &[u8],
    config: &FeedParseConfig,
) -> FeedParseInvocationOutcome {
    let input_packet_digest = sha256_digest(packet_bytes);
    if codec_id != "json" {
        return feed_parse_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
            "schema_invalid",
            "feed-parse accepts only json packet input under packet.v1.",
        );
    }
    if packet_bytes.len() > config.input_size_limit_bytes {
        return feed_parse_refusal_outcome(
            &input_packet_digest,
            STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
            "input_too_large",
            "feed-parse keeps input size ceilings explicit instead of relying on parser allocation behavior.",
        );
    }
    let request = match serde_json::from_slice::<FeedParseRequest>(packet_bytes) {
        Ok(request) => request,
        Err(_) => {
            return feed_parse_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "feed-parse refuses malformed packets without host-side schema repair.",
            );
        }
    };
    let source_url = match Url::parse(&request.source_url) {
        Ok(url) => url,
        Err(_) => {
            return feed_parse_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
                "schema_invalid",
                "feed-parse requires one absolute `source_url` for bounded link resolution.",
            );
        }
    };
    let response = match parse_feed_response(&source_url, &request.feed_text, config.max_entries) {
        Ok(response) => response,
        Err(detail) => {
            return feed_parse_refusal_outcome(
                &input_packet_digest,
                STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID,
                "unsupported_feed_format",
                detail,
            );
        }
    };
    let output_or_refusal_digest = stable_json_digest(b"feed_parse_response|", &response);
    let registration = starter_plugin_registration(STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Success,
        input_packet_digest,
        STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID,
        output_or_refusal_digest,
        None,
        String::from(
            "feed-parse keeps RSS 2.0 and Atom 1.0 normalization explicit and local instead of hiding broader XML interpretation in host glue.",
        ),
        b"feed_parse_receipt|",
    );
    FeedParseInvocationOutcome {
        receipt,
        response: Some(response),
        refusal: None,
    }
}

#[must_use]
pub fn build_feed_parse_runtime_bundle() -> FeedParseRuntimeBundle {
    let fetch_packet = br#"{"url":"https://snapshot.example/feed.rss"}"#;
    let fetch_result = invoke_fetch_text_json_packet(
        "json",
        fetch_packet,
        &FetchTextConfig::snapshot(sample_fetch_text_snapshot_entries()),
    );
    let rss_packet = fetch_result
        .response
        .as_ref()
        .and_then(|response| {
            serde_json::to_vec(&json!({
                "source_url": response.final_url,
                "content_type": response.content_type,
                "feed_text": response.body_text
            }))
            .ok()
        })
        .unwrap_or_default();
    let atom_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/feed.atom",
        "content_type": "application/atom+xml",
        "feed_text": r#"<?xml version="1.0" encoding="utf-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>Snapshot Atom Feed</title><subtitle>Atom updates from the snapshot fixture.</subtitle><link href="https://snapshot.example/" /><entry><title>Atom Entry</title><link href="/posts/atom-entry" /><updated>2026-03-22T00:00:00Z</updated><summary>Atom summary text.</summary></entry></feed>"#
    }))
    .unwrap_or_else(|_| Vec::new());
    let malformed_packet = br#"{"source_url":"https://snapshot.example/feed.rss","content_type":"application/rss+xml"}"#.to_vec();
    let unsupported_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/opml.xml",
        "content_type": "text/xml",
        "feed_text": "<opml version=\"2.0\"><body></body></opml>"
    }))
    .unwrap_or_else(|_| Vec::new());
    let oversized_packet = serde_json::to_vec(&json!({
        "source_url": "https://snapshot.example/feed.rss",
        "content_type": "application/rss+xml",
        "feed_text": "x".repeat(64 * 1024)
    }))
    .unwrap_or_else(|_| Vec::new());

    let rss_success =
        invoke_feed_parse_json_packet("json", &rss_packet, &FeedParseConfig::default());
    let atom_success =
        invoke_feed_parse_json_packet("json", &atom_packet, &FeedParseConfig::default());
    let schema_invalid =
        invoke_feed_parse_json_packet("json", &malformed_packet, &FeedParseConfig::default());
    let unsupported_format =
        invoke_feed_parse_json_packet("json", &unsupported_packet, &FeedParseConfig::default());
    let input_too_large = invoke_feed_parse_json_packet(
        "json",
        &oversized_packet,
        &FeedParseConfig {
            input_size_limit_bytes: 1024,
            max_entries: 64,
        },
    );

    let case_rows = vec![
        feed_parse_case(
            "rss_parse_success",
            FeedParseRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(&rss_packet),
            STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID,
            rss_success.receipt.output_or_refusal_digest.clone(),
            rss_success.receipt.clone(),
            "rss 2.0 parsing stays deterministic and packet-local over already-fetched content.",
        ),
        feed_parse_case(
            "atom_parse_success",
            FeedParseRuntimeCaseStatus::ExactSuccess,
            "json",
            sha256_digest(&atom_packet),
            STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID,
            atom_success.receipt.output_or_refusal_digest.clone(),
            atom_success.receipt.clone(),
            "atom 1.0 parsing stays inside the same deterministic bounded format window.",
        ),
        feed_parse_case(
            "schema_invalid_missing_feed_text",
            FeedParseRuntimeCaseStatus::TypedMalformedPacket,
            "json",
            sha256_digest(&malformed_packet),
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
            schema_invalid.receipt.output_or_refusal_digest.clone(),
            schema_invalid.receipt.clone(),
            "missing `feed_text` fails closed into a typed schema-invalid refusal.",
        ),
        feed_parse_case(
            "unsupported_feed_format_refusal",
            FeedParseRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(&unsupported_packet),
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID,
            unsupported_format.receipt.output_or_refusal_digest.clone(),
            unsupported_format.receipt.clone(),
            "opml and arbitrary xml stay outside the bounded RSS 2.0 and Atom 1.0 claim surface.",
        ),
        feed_parse_case(
            "input_too_large_refusal",
            FeedParseRuntimeCaseStatus::TypedRefusal,
            "json",
            sha256_digest(&oversized_packet),
            STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
            input_too_large.receipt.output_or_refusal_digest.clone(),
            input_too_large.receipt.clone(),
            "input ceilings stay explicit instead of letting XML parser allocation define semantics.",
        ),
    ];
    let composition_case = FeedParseCompositionCase {
        case_id: String::from("fetch_then_parse_feed"),
        step_plugin_ids: vec![
            String::from(STARTER_PLUGIN_HTTP_FETCH_TEXT_ID),
            String::from(STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID),
        ],
        step_receipt_ids: vec![
            fetch_result.receipt.receipt_id.clone(),
            rss_success.receipt.receipt_id.clone(),
        ],
        schema_repair_allowed: false,
        hidden_host_parsing_allowed: false,
        green: fetch_result.response.is_some() && rss_success.response.is_some(),
        detail: String::from(
            "the fetch-text response binds directly into feed-parse input fields without hidden host schema repair or host-side feed parsing.",
        ),
    };

    let registration = starter_plugin_registration(STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID);
    let mut bundle = FeedParseRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from(registration.runtime_bundle_id),
        plugin_id: String::from(registration.plugin_id),
        plugin_version: String::from(registration.plugin_version),
        manifest_id: String::from(registration.manifest_id),
        artifact_id: String::from(registration.artifact_id),
        packet_abi_version: String::from(TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_VERSION),
        mount_envelope_id: String::from(registration.mount_envelope_id),
        tool_projection: feed_parse_tool_projection(),
        negative_claim_ids: starter_plugin_string_ids(registration.negative_claim_ids),
        case_rows,
        composition_case,
        claim_boundary: String::from(
            "this runtime bundle closes one local deterministic starter plugin over already-fetched RSS 2.0 and Atom 1.0 content. It does not claim arbitrary XML support, OPML support, or general document parsing closure.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "feed-parse runtime bundle covers {} cases plus one green fetch-to-feed composition harness.",
        bundle.case_rows.len(),
    );
    bundle.bundle_digest = stable_json_digest(b"feed_parse_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_feed_rss_atom_parse_runtime_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_FEED_RSS_ATOM_PARSE_RUNTIME_BUNDLE_REF)
}

pub fn write_feed_parse_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<FeedParseRuntimeBundle, StarterPluginRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| StarterPluginRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = build_feed_parse_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_feed_parse_runtime_bundle(
    path: impl AsRef<Path>,
) -> Result<FeedParseRuntimeBundle, StarterPluginRuntimeError> {
    read_json(path)
}

fn url_extract_refusal_outcome(
    input_packet_digest: &str,
    schema_id: &str,
    refusal_class_id: &str,
    detail: impl Into<String>,
) -> UrlExtractInvocationOutcome {
    let refusal = StarterPluginRefusal {
        schema_id: String::from(schema_id),
        refusal_class_id: String::from(refusal_class_id),
        detail: detail.into(),
    };
    let output_or_refusal_digest = stable_json_digest(b"url_extract_refusal|", &refusal);
    let registration = starter_plugin_registration(STARTER_PLUGIN_TEXT_URL_EXTRACT_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Refusal,
        String::from(input_packet_digest),
        schema_id,
        output_or_refusal_digest,
        Some(refusal_class_id),
        refusal.detail.clone(),
        b"url_extract_receipt|",
    );
    UrlExtractInvocationOutcome {
        receipt,
        response: None,
        refusal: Some(refusal),
    }
}

fn extract_urls(text: &str, max_urls: usize) -> Result<Vec<String>, String> {
    let regex = regex::Regex::new(r"https?://[^\s]+").map_err(|error| error.to_string())?;
    let mut urls = Vec::new();
    for matched in regex.find_iter(text) {
        if urls.len() == max_urls {
            return Err(String::from(
                "url extract exceeded the configured output ceiling before completing the left-to-right scan.",
            ));
        }
        urls.push(matched.as_str().to_string());
    }
    Ok(urls)
}

fn url_extract_case(
    case_id: &str,
    status: UrlExtractRuntimeCaseStatus,
    codec_id: &str,
    request_packet_digest: String,
    response_or_refusal_schema_id: &str,
    response_or_refusal_digest: String,
    receipt: StarterPluginInvocationReceipt,
    detail: &str,
) -> UrlExtractRuntimeCase {
    UrlExtractRuntimeCase {
        case_id: String::from(case_id),
        status,
        codec_id: String::from(codec_id),
        request_packet_digest,
        response_or_refusal_schema_id: String::from(response_or_refusal_schema_id),
        response_or_refusal_digest,
        receipt,
        detail: String::from(detail),
    }
}

fn oversized_url_extract_packet(config: UrlExtractConfig) -> Vec<u8> {
    let oversized = "x".repeat(config.packet_size_limit_bytes.saturating_add(1));
    serde_json::to_vec(&json!({ "text": oversized }))
        .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}").into_bytes())
}

fn text_stats_refusal_outcome(
    input_packet_digest: &str,
    schema_id: &str,
    refusal_class_id: &str,
    detail: impl Into<String>,
) -> TextStatsInvocationOutcome {
    let refusal = StarterPluginRefusal {
        schema_id: String::from(schema_id),
        refusal_class_id: String::from(refusal_class_id),
        detail: detail.into(),
    };
    let output_or_refusal_digest = stable_json_digest(b"text_stats_refusal|", &refusal);
    let registration = starter_plugin_registration(STARTER_PLUGIN_TEXT_STATS_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Refusal,
        String::from(input_packet_digest),
        schema_id,
        output_or_refusal_digest,
        Some(refusal_class_id),
        refusal.detail.clone(),
        b"text_stats_receipt|",
    );
    TextStatsInvocationOutcome {
        receipt,
        response: None,
        refusal: Some(refusal),
    }
}

fn text_stats_response(text: &str) -> TextStatsResponse {
    let line_count = text.lines().count();
    let non_empty_line_count = text.lines().filter(|line| !line.trim().is_empty()).count();
    let word_count = text.split_whitespace().count();
    TextStatsResponse {
        byte_count: text.len(),
        unicode_scalar_count: text.chars().count(),
        line_count,
        non_empty_line_count,
        word_count,
    }
}

fn text_stats_case(
    case_id: &str,
    status: TextStatsRuntimeCaseStatus,
    codec_id: &str,
    request_packet_digest: String,
    response_or_refusal_schema_id: &str,
    response_or_refusal_digest: String,
    receipt: StarterPluginInvocationReceipt,
    detail: &str,
) -> TextStatsRuntimeCase {
    TextStatsRuntimeCase {
        case_id: String::from(case_id),
        status,
        codec_id: String::from(codec_id),
        request_packet_digest,
        response_or_refusal_schema_id: String::from(response_or_refusal_schema_id),
        response_or_refusal_digest,
        receipt,
        detail: String::from(detail),
    }
}

fn oversized_text_stats_packet(config: TextStatsConfig) -> Vec<u8> {
    let oversized = "x".repeat(config.packet_size_limit_bytes.saturating_add(1));
    serde_json::to_vec(&json!({ "text": oversized }))
        .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}").into_bytes())
}

fn default_fetch_text_mount_envelope(replay_class_id: &str) -> FetchTextMountEnvelope {
    FetchTextMountEnvelope {
        envelope_id: String::from("mount.plugin.http.fetch_text.read_only_http_allowlist.v1"),
        allowlisted_url_prefixes: vec![String::from("https://snapshot.example/")],
        timeout_millis: 500,
        response_size_limit_bytes: 16 * 1024,
        redirect_limit: 3,
        allowed_content_type_ids: vec![
            String::from("text/"),
            String::from("application/xhtml+xml"),
            String::from("application/xml"),
            String::from("text/xml"),
            String::from("application/rss+xml"),
            String::from("application/atom+xml"),
        ],
        replay_class_id: String::from(replay_class_id),
        detail: String::from(
            "the sample fetch-text mount envelope binds allowlist, timeout, redirect, response-size, and content-type policy to one host-mediated read-only HTTP capability.",
        ),
    }
}

fn backend_id(config: &FetchTextConfig) -> &'static str {
    match &config.backend {
        FetchTextBackend::Snapshot(_) => "snapshot_http_mount",
        FetchTextBackend::Live => "host_http_client",
    }
}

fn replay_class_id(config: &FetchTextConfig) -> &'static str {
    match &config.backend {
        FetchTextBackend::Snapshot(_) => "replayable_with_snapshots",
        FetchTextBackend::Live => "operator_replay_only",
    }
}

fn url_allowed(url: &str, mount_envelope: &FetchTextMountEnvelope) -> bool {
    mount_envelope
        .allowlisted_url_prefixes
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

fn invoke_snapshot_fetch(
    url: &Url,
    entries: &BTreeMap<String, FetchTextSnapshotResult>,
) -> Result<FetchTextResponse, (&'static str, &'static str, String)> {
    let Some(result) = entries.get(url.as_str()) else {
        return Err((
            STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID,
            "network_denied",
            String::from("the mounted snapshot did not admit this allowlisted URL."),
        ));
    };
    match result {
        FetchTextSnapshotResult::Success(response) => materialize_fetch_text_response(response),
        FetchTextSnapshotResult::Timeout => Err((
            STARTER_PLUGIN_REFUSAL_TIMEOUT_ID,
            "timeout",
            String::from("the mounted snapshot records this fetch as a timeout."),
        )),
        FetchTextSnapshotResult::NetworkDenied { detail } => Err((
            STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID,
            "network_denied",
            detail.clone(),
        )),
        FetchTextSnapshotResult::UpstreamFailure { detail } => Err((
            STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID,
            "upstream_failure",
            detail.clone(),
        )),
    }
}

fn invoke_live_fetch(
    url: &Url,
    mount_envelope: &FetchTextMountEnvelope,
) -> Result<FetchTextResponse, (&'static str, &'static str, String)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(mount_envelope.timeout_millis))
        .redirect(reqwest::redirect::Policy::limited(
            mount_envelope.redirect_limit,
        ))
        .build()
        .map_err(|error| {
            (
                STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID,
                "network_denied",
                format!("failed to build host HTTP client: {error}"),
            )
        })?;
    let response = client.get(url.clone()).send().map_err(|error| {
        if error.is_timeout() {
            (
                STARTER_PLUGIN_REFUSAL_TIMEOUT_ID,
                "timeout",
                String::from("the host HTTP client hit the mounted timeout ceiling."),
            )
        } else {
            (
                STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID,
                "upstream_failure",
                format!("host HTTP fetch failed: {error}"),
            )
        }
    })?;
    let header_value = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream");
    let (content_type, charset) = parse_content_type(header_value);
    if !content_type_supported(&content_type, mount_envelope) {
        return Err((
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
            "content_type_unsupported",
            format!("content type `{content_type}` is outside the bounded text-fetch window."),
        ));
    }
    if !response.status().is_success() {
        return Err((
            STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID,
            "upstream_failure",
            format!(
                "upstream returned HTTP status {}.",
                response.status().as_u16()
            ),
        ));
    }
    let final_url = response.url().to_string();
    let status_code = response.status().as_u16();
    let mut limited = response.take((mount_envelope.response_size_limit_bytes + 1) as u64);
    let mut body_bytes = Vec::new();
    limited.read_to_end(&mut body_bytes).map_err(|error| {
        (
            STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID,
            "upstream_failure",
            format!("failed to read upstream response: {error}"),
        )
    })?;
    if body_bytes.len() > mount_envelope.response_size_limit_bytes {
        return Err((
            STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID,
            "response_too_large",
            String::from("the response exceeded the mounted response-size ceiling."),
        ));
    }
    let body_text = decode_text_body(&body_bytes, charset.as_deref()).map_err(|detail| {
        (
            STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID,
            "decode_failed",
            detail,
        )
    })?;
    Ok(FetchTextResponse {
        final_url,
        status_code,
        content_type,
        charset,
        body_text,
        truncated: false,
    })
}

fn materialize_fetch_text_response(
    response: &FetchTextSnapshotResponse,
) -> Result<FetchTextResponse, (&'static str, &'static str, String)> {
    let mount_envelope = default_fetch_text_mount_envelope("replayable_with_snapshots");
    if !content_type_supported(&response.content_type, &mount_envelope) {
        return Err((
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
            "content_type_unsupported",
            format!(
                "content type `{}` is outside the bounded text-fetch window.",
                response.content_type
            ),
        ));
    }
    if response.body_bytes.len() > mount_envelope.response_size_limit_bytes {
        return Err((
            STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID,
            "response_too_large",
            String::from("the response exceeded the mounted response-size ceiling."),
        ));
    }
    let body_text =
        decode_text_body(&response.body_bytes, response.charset.as_deref()).map_err(|detail| {
            (
                STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID,
                "decode_failed",
                detail,
            )
        })?;
    Ok(FetchTextResponse {
        final_url: response.final_url.clone(),
        status_code: response.status_code,
        content_type: response.content_type.clone(),
        charset: response.charset.clone(),
        body_text,
        truncated: false,
    })
}

fn fetch_text_refusal_outcome(
    input_packet_digest: &str,
    backend_id: &str,
    replay_class_id: &str,
    schema_id: &str,
    refusal_class_id: &str,
    detail: impl Into<String>,
) -> FetchTextInvocationOutcome {
    let refusal = StarterPluginRefusal {
        schema_id: String::from(schema_id),
        refusal_class_id: String::from(refusal_class_id),
        detail: detail.into(),
    };
    let output_or_refusal_digest = stable_json_digest(b"fetch_text_refusal|", &refusal);
    let receipt = starter_plugin_receipt_from_registration(
        starter_plugin_registration(STARTER_PLUGIN_HTTP_FETCH_TEXT_ID),
        "mount.plugin.http.fetch_text.read_only_http_allowlist.v1",
        replay_class_id,
        StarterPluginInvocationStatus::Refusal,
        String::from(input_packet_digest),
        schema_id,
        output_or_refusal_digest,
        Some(refusal_class_id),
        refusal.detail.clone(),
        b"fetch_text_receipt|",
    );
    FetchTextInvocationOutcome {
        receipt,
        backend_id: String::from(backend_id),
        logical_cpu_millis: 1,
        response: None,
        refusal: Some(refusal),
    }
}

fn fetch_text_case(
    case_id: &str,
    status: FetchTextRuntimeCaseStatus,
    codec_id: &str,
    request_packet_digest: String,
    response_or_refusal_schema_id: &str,
    response_or_refusal_digest: String,
    backend_id: String,
    logical_cpu_millis: u32,
    receipt: StarterPluginInvocationReceipt,
    detail: &str,
) -> FetchTextRuntimeCase {
    FetchTextRuntimeCase {
        case_id: String::from(case_id),
        status,
        codec_id: String::from(codec_id),
        request_packet_digest,
        response_or_refusal_schema_id: String::from(response_or_refusal_schema_id),
        response_or_refusal_digest,
        backend_id,
        logical_cpu_millis,
        receipt,
        detail: String::from(detail),
    }
}

fn parse_content_type(header_value: &str) -> (String, Option<String>) {
    let mut parts = header_value.split(';');
    let content_type = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream")
        .to_ascii_lowercase();
    let charset = parts.find_map(|part| {
        let part = part.trim();
        part.strip_prefix("charset=")
            .map(|value| value.trim_matches('"').to_ascii_lowercase())
    });
    (content_type, charset)
}

fn content_type_supported(content_type: &str, mount_envelope: &FetchTextMountEnvelope) -> bool {
    mount_envelope
        .allowed_content_type_ids
        .iter()
        .any(|allowed| {
            if allowed.ends_with('/') {
                content_type.starts_with(allowed)
            } else {
                content_type == allowed
            }
        })
}

fn decode_text_body(bytes: &[u8], charset: Option<&str>) -> Result<String, String> {
    match charset.unwrap_or("utf-8") {
        "utf-8" | "utf8" => String::from_utf8(bytes.to_vec()).map_err(|_| {
            String::from("the response body failed utf-8 decoding under the declared charset.")
        }),
        "us-ascii" | "ascii" => {
            if bytes.iter().all(|byte| byte.is_ascii()) {
                String::from_utf8(bytes.to_vec()).map_err(|_| {
                    String::from(
                        "the response body failed ascii decoding under the declared charset.",
                    )
                })
            } else {
                Err(String::from(
                    "the response body contained non-ascii bytes under an ascii charset declaration.",
                ))
            }
        }
        unsupported => Err(format!(
            "the declared charset `{unsupported}` is outside the bounded fetch-text decoder window."
        )),
    }
}

fn sample_fetch_text_snapshot_entries() -> BTreeMap<String, FetchTextSnapshotResult> {
    BTreeMap::from([
        (
            String::from("https://snapshot.example/article"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/article"),
                status_code: 200,
                content_type: String::from("text/html"),
                charset: Some(String::from("utf-8")),
                body_bytes: b"<html lang=\"en\"><head><title>Snapshot Article</title><meta name=\"description\" content=\"Bounded HTML extraction fixture.\" /><meta property=\"og:site_name\" content=\"Snapshot Example\" /><link rel=\"canonical\" href=\"/article\" /></head><body><main><h1>Snapshot Article</h1><p>Bounded starter plugin content.</p><a href=\"/alpha\">Alpha</a><a href=\"https://snapshot.example/beta\">Beta</a></main></body></html>".to_vec(),
            }),
        ),
        (
            String::from("https://snapshot.example/slow"),
            FetchTextSnapshotResult::Timeout,
        ),
        (
            String::from("https://snapshot.example/binary"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/binary"),
                status_code: 200,
                content_type: String::from("image/png"),
                charset: None,
                body_bytes: vec![0x89, 0x50, 0x4e, 0x47],
            }),
        ),
        (
            String::from("https://snapshot.example/large"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/large"),
                status_code: 200,
                content_type: String::from("text/plain"),
                charset: Some(String::from("utf-8")),
                body_bytes: vec![b'x'; 16 * 1024 + 8],
            }),
        ),
        (
            String::from("https://snapshot.example/bad-utf8"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/bad-utf8"),
                status_code: 200,
                content_type: String::from("text/plain"),
                charset: Some(String::from("utf-8")),
                body_bytes: vec![0xff, 0xfe, 0xfd],
            }),
        ),
        (
            String::from("https://snapshot.example/broken"),
            FetchTextSnapshotResult::UpstreamFailure {
                detail: String::from(
                    "the mounted snapshot marks this URL as an upstream transport failure.",
                ),
            },
        ),
        (
            String::from("https://snapshot.example/feed.rss"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/feed.rss"),
                status_code: 200,
                content_type: String::from("application/rss+xml"),
                charset: Some(String::from("utf-8")),
                body_bytes: br#"<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>Snapshot Feed</title><link>https://snapshot.example/</link><description>Snapshot feed updates.</description><item><title>Feed Entry</title><link>/posts/feed-entry</link><pubDate>Sat, 22 Mar 2026 00:00:00 GMT</pubDate><description>Snapshot entry summary.</description></item></channel></rss>"#.to_vec(),
            }),
        ),
        (
            String::from("https://snapshot.example/feed.atom"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/feed.atom"),
                status_code: 200,
                content_type: String::from("application/atom+xml"),
                charset: Some(String::from("utf-8")),
                body_bytes: br#"<?xml version="1.0" encoding="utf-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>Snapshot Atom Feed</title><subtitle>Atom updates from the snapshot fixture.</subtitle><link href="https://snapshot.example/" /><entry><title>Atom Entry</title><link href="/posts/atom-entry" /><updated>2026-03-22T00:00:00Z</updated><summary>Atom summary text.</summary></entry></feed>"#.to_vec(),
            }),
        ),
    ])
}

fn extract_readable_refusal_outcome(
    input_packet_digest: &str,
    schema_id: &str,
    refusal_class_id: &str,
    detail: impl Into<String>,
) -> ExtractReadableInvocationOutcome {
    let refusal = StarterPluginRefusal {
        schema_id: String::from(schema_id),
        refusal_class_id: String::from(refusal_class_id),
        detail: detail.into(),
    };
    let output_or_refusal_digest = stable_json_digest(b"extract_readable_refusal|", &refusal);
    let registration = starter_plugin_registration(STARTER_PLUGIN_HTML_EXTRACT_READABLE_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Refusal,
        String::from(input_packet_digest),
        schema_id,
        output_or_refusal_digest,
        Some(refusal_class_id),
        refusal.detail.clone(),
        b"extract_readable_receipt|",
    );
    ExtractReadableInvocationOutcome {
        receipt,
        response: None,
        refusal: Some(refusal),
    }
}

fn extract_readable_case(
    case_id: &str,
    status: ExtractReadableRuntimeCaseStatus,
    codec_id: &str,
    request_packet_digest: String,
    response_or_refusal_schema_id: &str,
    response_or_refusal_digest: String,
    receipt: StarterPluginInvocationReceipt,
    detail: &str,
) -> ExtractReadableRuntimeCase {
    ExtractReadableRuntimeCase {
        case_id: String::from(case_id),
        status,
        codec_id: String::from(codec_id),
        request_packet_digest,
        response_or_refusal_schema_id: String::from(response_or_refusal_schema_id),
        response_or_refusal_digest,
        receipt,
        detail: String::from(detail),
    }
}

fn feed_parse_refusal_outcome(
    input_packet_digest: &str,
    schema_id: &str,
    refusal_class_id: &str,
    detail: impl Into<String>,
) -> FeedParseInvocationOutcome {
    let refusal = StarterPluginRefusal {
        schema_id: String::from(schema_id),
        refusal_class_id: String::from(refusal_class_id),
        detail: detail.into(),
    };
    let output_or_refusal_digest = stable_json_digest(b"feed_parse_refusal|", &refusal);
    let registration = starter_plugin_registration(STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_ID);
    let receipt = starter_plugin_receipt_from_registration(
        registration,
        registration.mount_envelope_id,
        registration.replay_class_id,
        StarterPluginInvocationStatus::Refusal,
        String::from(input_packet_digest),
        schema_id,
        output_or_refusal_digest,
        Some(refusal_class_id),
        refusal.detail.clone(),
        b"feed_parse_receipt|",
    );
    FeedParseInvocationOutcome {
        receipt,
        response: None,
        refusal: Some(refusal),
    }
}

fn feed_parse_case(
    case_id: &str,
    status: FeedParseRuntimeCaseStatus,
    codec_id: &str,
    request_packet_digest: String,
    response_or_refusal_schema_id: &str,
    response_or_refusal_digest: String,
    receipt: StarterPluginInvocationReceipt,
    detail: &str,
) -> FeedParseRuntimeCase {
    FeedParseRuntimeCase {
        case_id: String::from(case_id),
        status,
        codec_id: String::from(codec_id),
        request_packet_digest,
        response_or_refusal_schema_id: String::from(response_or_refusal_schema_id),
        response_or_refusal_digest,
        receipt,
        detail: String::from(detail),
    }
}

fn parse_feed_response(
    source_url: &Url,
    feed_text: &str,
    max_entries: usize,
) -> Result<FeedParseResponse, String> {
    let document = roxmltree::Document::parse(feed_text).map_err(|_| {
        String::from("feed-parse stays bounded to well-formed RSS 2.0 and Atom 1.0 XML documents.")
    })?;
    let root = document.root_element();
    if root.tag_name().name() == "rss" && root.attribute("version") == Some("2.0") {
        return parse_rss_feed(root, source_url, max_entries);
    }
    if root.tag_name().name() == "feed"
        && root.tag_name().namespace() == Some("http://www.w3.org/2005/Atom")
    {
        return parse_atom_feed(root, source_url, max_entries);
    }
    Err(String::from(
        "feed-parse stays bounded to RSS 2.0 and Atom 1.0 and refuses OPML or arbitrary XML documents.",
    ))
}

fn parse_rss_feed(
    root: roxmltree::Node<'_, '_>,
    source_url: &Url,
    max_entries: usize,
) -> Result<FeedParseResponse, String> {
    let channel = first_xml_child(root, "channel").ok_or_else(|| {
        String::from("rss parsing requires one `<channel>` element under an RSS 2.0 root.")
    })?;
    let entries = channel
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .take(max_entries)
        .map(|item| {
            let summary = first_xml_child_text(item, "description");
            let content_excerpt = first_xml_child_text(item, "encoded")
                .as_deref()
                .and_then(excerpt_from_text)
                .or_else(|| summary.as_deref().and_then(excerpt_from_text));
            FeedParseEntry {
                title: first_xml_child_text(item, "title"),
                link: first_xml_child_text(item, "link")
                    .map(|href| resolve_link(source_url, href.as_str())),
                published_time: first_xml_child_text(item, "pubDate")
                    .or_else(|| first_xml_child_text(item, "date")),
                summary,
                content_excerpt,
            }
        })
        .collect();
    Ok(FeedParseResponse {
        feed_title: first_xml_child_text(channel, "title"),
        feed_homepage_url: first_xml_child_text(channel, "link")
            .map(|href| resolve_link(source_url, href.as_str())),
        feed_description: first_xml_child_text(channel, "description"),
        entries,
    })
}

fn parse_atom_feed(
    root: roxmltree::Node<'_, '_>,
    source_url: &Url,
    max_entries: usize,
) -> Result<FeedParseResponse, String> {
    let entries = root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "entry")
        .take(max_entries)
        .map(|entry| {
            let summary = first_xml_child_text(entry, "summary");
            let content_excerpt = first_xml_child_text(entry, "content")
                .as_deref()
                .and_then(excerpt_from_text)
                .or_else(|| summary.as_deref().and_then(excerpt_from_text));
            FeedParseEntry {
                title: first_xml_child_text(entry, "title"),
                link: atom_link_href(entry, source_url),
                published_time: first_xml_child_text(entry, "published")
                    .or_else(|| first_xml_child_text(entry, "updated")),
                summary,
                content_excerpt,
            }
        })
        .collect();
    Ok(FeedParseResponse {
        feed_title: first_xml_child_text(root, "title"),
        feed_homepage_url: atom_link_href(root, source_url),
        feed_description: first_xml_child_text(root, "subtitle"),
        entries,
    })
}

fn first_xml_child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    local_name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == local_name)
}

fn first_xml_child_text(node: roxmltree::Node<'_, '_>, local_name: &str) -> Option<String> {
    first_xml_child(node, local_name).and_then(xml_node_text)
}

fn xml_node_text(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let value = node
        .descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = normalize_text(&value);
    (!normalized.is_empty()).then_some(normalized)
}

fn atom_link_href(node: roxmltree::Node<'_, '_>, base_url: &Url) -> Option<String> {
    let mut fallback = None;
    for child in node.children() {
        if !child.is_element() || child.tag_name().name() != "link" {
            continue;
        }
        let Some(href) = child.attribute("href") else {
            continue;
        };
        let resolved = resolve_link(base_url, href);
        let rel = child.attribute("rel").unwrap_or("alternate");
        if rel == "alternate" {
            return Some(resolved);
        }
        if fallback.is_none() {
            fallback = Some(resolved);
        }
    }
    fallback
}

fn readable_text_from_document(document: &scraper::Html) -> String {
    let candidate = text_for_selector(document, "article")
        .filter(|text| !text.is_empty())
        .or_else(|| text_for_selector(document, "main").filter(|text| !text.is_empty()))
        .or_else(|| text_for_selector(document, "body").filter(|text| !text.is_empty()))
        .unwrap_or_default();
    normalize_text(&candidate)
}

fn harvested_links_from_document(
    document: &scraper::Html,
    base_url: &Url,
    max_links: usize,
) -> Vec<String> {
    let Some(selector) = selector("a") else {
        return Vec::new();
    };
    document
        .select(&selector)
        .filter_map(|node| node.value().attr("href"))
        .take(max_links)
        .map(|href| resolve_link(base_url, href))
        .collect()
}

fn excerpt_from_text(text: &str) -> Option<String> {
    let excerpt = text.chars().take(240).collect::<String>();
    (!excerpt.is_empty()).then_some(excerpt)
}

fn resolve_link(base_url: &Url, href: &str) -> String {
    match base_url.join(href) {
        Ok(url) => url.to_string(),
        Err(_) => String::from(href),
    }
}

fn meta_content(document: &scraper::Html, attr_name: &str, attr_value: &str) -> Option<String> {
    let selector = selector(&format!(r#"meta[{attr_name}="{attr_value}"]"#))?;
    document
        .select(&selector)
        .find_map(|node| node.value().attr("content").map(normalize_text))
        .filter(|value| !value.is_empty())
}

fn first_attr_for_selector(
    document: &scraper::Html,
    selector_text: &str,
    attr_name: &str,
) -> Option<String> {
    let selector = selector(selector_text)?;
    document
        .select(&selector)
        .find_map(|node| node.value().attr(attr_name).map(String::from))
}

fn first_text_for_selector(document: &scraper::Html, selector_text: &str) -> Option<String> {
    text_for_selector(document, selector_text).filter(|value| !value.is_empty())
}

fn text_for_selector(document: &scraper::Html, selector_text: &str) -> Option<String> {
    let selector = selector(selector_text)?;
    document
        .select(&selector)
        .next()
        .map(|node| normalize_text(&node.text().collect::<Vec<_>>().join(" ")))
}

fn selector(selector_text: &str) -> Option<scraper::Selector> {
    scraper::Selector::parse(selector_text).ok()
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn stable_json_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value)
        .unwrap_or_else(|error| format!("serialization_error:{error}").into_bytes());
    stable_digest(prefix, &encoded)
}

fn sha256_digest(bytes: &[u8]) -> String {
    stable_digest(b"sha256|", bytes)
}

fn stable_digest(prefix: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, StarterPluginRuntimeError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| StarterPluginRuntimeError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| StarterPluginRuntimeError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(not(test))]
fn read_json<T: for<'de> Deserialize<'de>>(
    path: impl AsRef<Path>,
) -> Result<T, StarterPluginRuntimeError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| StarterPluginRuntimeError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| StarterPluginRuntimeError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        bridge_exposed_starter_plugin_registrations, build_extract_readable_runtime_bundle,
        build_feed_parse_runtime_bundle, build_fetch_text_runtime_bundle,
        build_text_stats_runtime_bundle, build_url_extract_runtime_bundle,
        capability_free_starter_plugin_registrations, catalog_exposed_starter_plugin_registrations,
        guest_artifact_starter_plugin_registrations,
        invoke_extract_readable_json_packet, invoke_feed_parse_json_packet,
        invoke_fetch_text_json_packet, invoke_text_stats_json_packet,
        invoke_url_extract_json_packet, networked_starter_plugin_registrations,
        starter_plugin_registration_by_plugin_id, starter_plugin_registrations,
        tassadar_post_article_plugin_feed_rss_atom_parse_runtime_bundle_path,
        tassadar_post_article_plugin_html_extract_readable_runtime_bundle_path,
        tassadar_post_article_plugin_http_fetch_text_runtime_bundle_path,
        tassadar_post_article_plugin_text_stats_runtime_bundle_path,
        tassadar_post_article_plugin_text_url_extract_runtime_bundle_path,
        user_added_starter_plugin_registrations,
        weighted_controller_admissible_user_added_starter_plugin_registrations,
        write_extract_readable_runtime_bundle, write_feed_parse_runtime_bundle,
        write_fetch_text_runtime_bundle, write_text_stats_runtime_bundle,
        write_url_extract_runtime_bundle, ExtractReadableConfig, ExtractReadableRuntimeCaseStatus,
        FeedParseConfig, FeedParseRuntimeCaseStatus, FetchTextConfig, FetchTextRuntimeCaseStatus,
        FetchTextSnapshotResponse, FetchTextSnapshotResult, StarterPluginAuthoringClass,
        StarterPluginOriginClass, TextStatsConfig, TextStatsRuntimeCaseStatus, UrlExtractConfig,
        UrlExtractRuntimeCaseStatus, STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID,
        STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID,
        STARTER_PLUGIN_HTTP_FETCH_TEXT_OUTPUT_SCHEMA_ID,
        STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID,
        STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID, STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID,
        STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID, STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID,
        STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID,
        STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID, STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID,
        STARTER_PLUGIN_REFUSAL_TIMEOUT_ID, STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID,
        STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID,
        STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID, STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID,
        STARTER_PLUGIN_TEXT_STATS_OUTPUT_SCHEMA_ID,
        STARTER_PLUGIN_TEXT_URL_EXTRACT_OUTPUT_SCHEMA_ID,
    };
    use tempfile::tempdir;

    #[test]
    fn starter_plugin_registry_keeps_user_plugin_visible_across_bridge_and_catalog() {
        let text_stats = starter_plugin_registration_by_plugin_id("plugin.text.stats")
            .expect("text-stats registration");
        let fetch_text = starter_plugin_registration_by_plugin_id("plugin.http.fetch_text")
            .expect("fetch-text registration");
        let guest_echo = starter_plugin_registration_by_plugin_id("plugin.example.echo_guest")
            .expect("guest echo registration");

        assert_eq!(starter_plugin_registrations().len(), 6);
        assert_eq!(bridge_exposed_starter_plugin_registrations().len(), 6);
        assert_eq!(catalog_exposed_starter_plugin_registrations().len(), 6);
        assert_eq!(capability_free_starter_plugin_registrations().len(), 4);
        assert_eq!(networked_starter_plugin_registrations().len(), 1);
        assert_eq!(guest_artifact_starter_plugin_registrations().len(), 1);
        assert_eq!(user_added_starter_plugin_registrations().len(), 3);
        assert_eq!(
            weighted_controller_admissible_user_added_starter_plugin_registrations().len(),
            3
        );
        assert_eq!(text_stats.tool_name, "plugin_text_stats");
        assert_eq!(
            text_stats.authoring_class,
            StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic
        );
        assert_eq!(text_stats.origin_class, StarterPluginOriginClass::UserAdded);
        assert!(text_stats.bridge_exposed);
        assert!(text_stats.catalog_exposed);
        assert_eq!(fetch_text.tool_name, "plugin_http_fetch_text");
        assert_eq!(
            fetch_text.authoring_class,
            StarterPluginAuthoringClass::NetworkedReadOnly
        );
        assert_eq!(fetch_text.origin_class, StarterPluginOriginClass::UserAdded);
        assert!(fetch_text.bridge_exposed);
        assert!(fetch_text.catalog_exposed);
        assert_eq!(guest_echo.tool_name, "plugin_example_echo_guest");
        assert_eq!(
            guest_echo.authoring_class,
            StarterPluginAuthoringClass::GuestArtifactDigestBound
        );
        assert_eq!(guest_echo.origin_class, StarterPluginOriginClass::UserAdded);
        assert!(guest_echo.bridge_exposed);
        assert!(guest_echo.catalog_exposed);
    }

    #[test]
    fn url_extract_success_preserves_order_and_duplicates() {
        let packet =
            br#"{"text":"https://alpha.example/a http://beta.test/b https://alpha.example/a"}"#;
        let outcome = invoke_url_extract_json_packet("json", packet, &UrlExtractConfig::default());

        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_TEXT_URL_EXTRACT_OUTPUT_SCHEMA_ID
        );
        assert_eq!(
            outcome.response.expect("response").urls,
            vec![
                String::from("https://alpha.example/a"),
                String::from("http://beta.test/b"),
                String::from("https://alpha.example/a"),
            ]
        );
    }

    #[test]
    fn url_extract_refuses_schema_invalid_packets() {
        let outcome = invoke_url_extract_json_packet(
            "json",
            br#"{"body":"missing"}"#,
            &UrlExtractConfig::default(),
        );
        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        );
    }

    #[test]
    fn url_extract_refuses_oversized_packets() {
        let outcome = invoke_url_extract_json_packet(
            "json",
            br#"{"text":"0123456789"}"#,
            &UrlExtractConfig {
                packet_size_limit_bytes: 8,
                max_urls: 8,
            },
        );
        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID
        );
    }

    #[test]
    fn url_extract_refuses_unsupported_codecs() {
        let outcome = invoke_url_extract_json_packet(
            "bytes",
            br#"{"text":"https://alpha.example/a"}"#,
            &UrlExtractConfig::default(),
        );
        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID
        );
    }

    #[test]
    fn url_extract_refuses_runtime_resource_limit_overflow() {
        let outcome = invoke_url_extract_json_packet(
            "json",
            br#"{"text":"https://alpha.example/a https://beta.example/b"}"#,
            &UrlExtractConfig {
                packet_size_limit_bytes: 1024,
                max_urls: 1,
            },
        );
        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID
        );
    }

    #[test]
    fn url_extract_runtime_bundle_covers_declared_cases() {
        let bundle = build_url_extract_runtime_bundle();

        assert_eq!(bundle.case_rows.len(), 5);
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| row.status == UrlExtractRuntimeCaseStatus::ExactSuccess));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == UrlExtractRuntimeCaseStatus::TypedMalformedPacket
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == UrlExtractRuntimeCaseStatus::TypedRefusal
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == UrlExtractRuntimeCaseStatus::TypedRefusal
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == UrlExtractRuntimeCaseStatus::TypedRefusal
                && row.response_or_refusal_schema_id
                    == STARTER_PLUGIN_REFUSAL_RUNTIME_RESOURCE_LIMIT_ID
        }));
    }

    #[test]
    fn url_extract_runtime_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("url_extract_bundle.json");
        let written = write_url_extract_runtime_bundle(&output_path).expect("write bundle");
        let loaded: super::UrlExtractRuntimeBundle =
            super::load_url_extract_runtime_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn url_extract_runtime_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_plugin_text_url_extract_runtime_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_plugin_text_url_extract_v1/tassadar_post_article_plugin_text_url_extract_bundle.json"
        ));
    }

    #[test]
    fn text_stats_success_returns_deterministic_counts() {
        let packet = br#"{"text":"alpha beta\n\ngamma delta"}"#;
        let outcome = invoke_text_stats_json_packet("json", packet, &TextStatsConfig::default());

        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_TEXT_STATS_OUTPUT_SCHEMA_ID
        );
        let response = outcome.response.expect("response");
        assert_eq!(response.byte_count, 23);
        assert_eq!(response.unicode_scalar_count, 23);
        assert_eq!(response.line_count, 3);
        assert_eq!(response.non_empty_line_count, 2);
        assert_eq!(response.word_count, 4);
    }

    #[test]
    fn text_stats_refuses_schema_invalid_packets() {
        let outcome = invoke_text_stats_json_packet(
            "json",
            br#"{"body":"missing"}"#,
            &TextStatsConfig::default(),
        );
        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        );
    }

    #[test]
    fn text_stats_refuses_oversized_packets_and_unsupported_codecs() {
        let oversized = invoke_text_stats_json_packet(
            "json",
            br#"{"text":"0123456789"}"#,
            &TextStatsConfig {
                packet_size_limit_bytes: 8,
            },
        );
        assert_eq!(
            oversized.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID
        );

        let wrong_codec = invoke_text_stats_json_packet(
            "bytes",
            br#"{"text":"alpha beta"}"#,
            &TextStatsConfig::default(),
        );
        assert_eq!(
            wrong_codec.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID
        );
    }

    #[test]
    fn text_stats_runtime_bundle_covers_declared_cases() {
        let bundle = build_text_stats_runtime_bundle();

        assert_eq!(bundle.case_rows.len(), 4);
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| row.status == TextStatsRuntimeCaseStatus::ExactSuccess));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == TextStatsRuntimeCaseStatus::TypedMalformedPacket
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == TextStatsRuntimeCaseStatus::TypedRefusal
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_PACKET_TOO_LARGE_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == TextStatsRuntimeCaseStatus::TypedRefusal
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_UNSUPPORTED_CODEC_ID
        }));
    }

    #[test]
    fn text_stats_runtime_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("text_stats_bundle.json");
        let written = write_text_stats_runtime_bundle(&output_path).expect("write bundle");
        let loaded: super::TextStatsRuntimeBundle =
            super::load_text_stats_runtime_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn text_stats_runtime_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_plugin_text_stats_runtime_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_plugin_text_stats_v1/tassadar_post_article_plugin_text_stats_bundle.json"
        ));
    }

    #[test]
    fn fetch_text_snapshot_success_returns_structured_fields() {
        let config = FetchTextConfig::snapshot(std::collections::BTreeMap::from([(
            String::from("https://snapshot.example/article"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/article"),
                status_code: 200,
                content_type: String::from("text/plain"),
                charset: Some(String::from("utf-8")),
                body_bytes: b"snapshot body".to_vec(),
            }),
        )]));
        let outcome = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/article"}"#,
            &config,
        );

        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_HTTP_FETCH_TEXT_OUTPUT_SCHEMA_ID
        );
        let response = outcome.response.expect("response");
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body_text, "snapshot body");
    }

    #[test]
    fn fetch_text_refuses_blocked_urls() {
        let config = FetchTextConfig::snapshot(std::collections::BTreeMap::new());
        let outcome = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://blocked.example/article"}"#,
            &config,
        );
        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID
        );
    }

    #[test]
    fn fetch_text_refuses_timeout_and_missing_snapshot_rows() {
        let config = FetchTextConfig::snapshot(std::collections::BTreeMap::from([(
            String::from("https://snapshot.example/slow"),
            FetchTextSnapshotResult::Timeout,
        )]));
        let timeout = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/slow"}"#,
            &config,
        );
        assert_eq!(
            timeout.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_TIMEOUT_ID
        );

        let missing = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/missing"}"#,
            &config,
        );
        assert_eq!(
            missing.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID
        );
    }

    #[test]
    fn fetch_text_refuses_large_binary_decode_and_upstream_cases() {
        let config = FetchTextConfig::snapshot(std::collections::BTreeMap::from([
            (
                String::from("https://snapshot.example/large"),
                FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                    final_url: String::from("https://snapshot.example/large"),
                    status_code: 200,
                    content_type: String::from("text/plain"),
                    charset: Some(String::from("utf-8")),
                    body_bytes: vec![b'x'; 16 * 1024 + 1],
                }),
            ),
            (
                String::from("https://snapshot.example/binary"),
                FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                    final_url: String::from("https://snapshot.example/binary"),
                    status_code: 200,
                    content_type: String::from("image/png"),
                    charset: None,
                    body_bytes: vec![0x89, 0x50, 0x4e, 0x47],
                }),
            ),
            (
                String::from("https://snapshot.example/bad-utf8"),
                FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                    final_url: String::from("https://snapshot.example/bad-utf8"),
                    status_code: 200,
                    content_type: String::from("text/plain"),
                    charset: Some(String::from("utf-8")),
                    body_bytes: vec![0xff, 0xfe],
                }),
            ),
            (
                String::from("https://snapshot.example/broken"),
                FetchTextSnapshotResult::UpstreamFailure {
                    detail: String::from("broken"),
                },
            ),
        ]));

        let large = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/large"}"#,
            &config,
        );
        assert_eq!(
            large.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID
        );

        let bad_type = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/binary"}"#,
            &config,
        );
        assert_eq!(
            bad_type.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID
        );

        let bad_decode = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/bad-utf8"}"#,
            &config,
        );
        assert_eq!(
            bad_decode.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID
        );

        let upstream = invoke_fetch_text_json_packet(
            "json",
            br#"{"url":"https://snapshot.example/broken"}"#,
            &config,
        );
        assert_eq!(
            upstream.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID
        );
    }

    #[test]
    fn fetch_text_runtime_bundle_covers_declared_cases() {
        let bundle = build_fetch_text_runtime_bundle();

        assert_eq!(bundle.case_rows.len(), 9);
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| row.status == FetchTextRuntimeCaseStatus::ExactSuccess));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == FetchTextRuntimeCaseStatus::TypedMalformedPacket
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_URL_NOT_PERMITTED_ID
        }));
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| { row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_TIMEOUT_ID }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_NETWORK_DENIED_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_RESPONSE_TOO_LARGE_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_DECODE_FAILED_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_UPSTREAM_FAILURE_ID
        }));
    }

    #[test]
    fn fetch_text_runtime_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("fetch_text_bundle.json");
        let written = write_fetch_text_runtime_bundle(&output_path).expect("write bundle");
        let loaded: super::FetchTextRuntimeBundle =
            super::load_fetch_text_runtime_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn fetch_text_runtime_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_plugin_http_fetch_text_runtime_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_plugin_http_fetch_text_v1/tassadar_post_article_plugin_http_fetch_text_bundle.json"
        ));
    }

    #[test]
    fn extract_readable_success_returns_metadata_and_links() {
        let packet = serde_json::to_vec(&serde_json::json!({
            "source_url": "https://snapshot.example/article",
            "content_type": "text/html",
            "body_text": "<html lang=\"en\"><head><title>Example</title><meta name=\"description\" content=\"summary\" /><link rel=\"canonical\" href=\"/article\" /></head><body><main><h1>Example</h1><p>Readable body.</p><a href=\"/alpha\">Alpha</a></main></body></html>"
        }))
        .expect("packet");
        let outcome =
            invoke_extract_readable_json_packet("json", &packet, &ExtractReadableConfig::default());

        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_HTML_EXTRACT_READABLE_OUTPUT_SCHEMA_ID
        );
        let response = outcome.response.expect("response");
        assert_eq!(response.title.as_deref(), Some("Example"));
        assert_eq!(
            response.canonical_url.as_deref(),
            Some("https://snapshot.example/article")
        );
        assert_eq!(response.content_language.as_deref(), Some("en"));
        assert_eq!(
            response.harvested_links,
            vec![String::from("https://snapshot.example/alpha")]
        );
    }

    #[test]
    fn extract_readable_refuses_schema_invalid_content_type_and_size() {
        let schema_invalid = invoke_extract_readable_json_packet(
            "json",
            br#"{"source_url":"https://snapshot.example/article","content_type":"text/html"}"#,
            &ExtractReadableConfig::default(),
        );
        assert_eq!(
            schema_invalid.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        );

        let wrong_type = invoke_extract_readable_json_packet(
            "json",
            br#"{"source_url":"https://snapshot.example/article","content_type":"application/json","body_text":"{}"}"#,
            &ExtractReadableConfig::default(),
        );
        assert_eq!(
            wrong_type.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID
        );

        let oversized = invoke_extract_readable_json_packet(
            "json",
            br#"{"source_url":"https://snapshot.example/article","content_type":"text/html","body_text":"xxxxxxxxxxxxxxxx"}"#,
            &ExtractReadableConfig {
                input_size_limit_bytes: 8,
                max_links: 8,
            },
        );
        assert_eq!(
            oversized.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID
        );
    }

    #[test]
    fn extract_readable_runtime_bundle_covers_declared_cases() {
        let bundle = build_extract_readable_runtime_bundle();

        assert_eq!(bundle.case_rows.len(), 5);
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| row.status == ExtractReadableRuntimeCaseStatus::ExactSuccess));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == ExtractReadableRuntimeCaseStatus::TypedMalformedPacket
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_CONTENT_TYPE_UNSUPPORTED_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID
        }));
        assert!(bundle.composition_case.green);
        assert_eq!(
            bundle.composition_case.step_plugin_ids,
            vec![
                String::from("plugin.http.fetch_text"),
                String::from("plugin.html.extract_readable"),
            ]
        );
    }

    #[test]
    fn extract_readable_runtime_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("extract_readable_bundle.json");
        let written = write_extract_readable_runtime_bundle(&output_path).expect("write bundle");
        let loaded: super::ExtractReadableRuntimeBundle =
            super::load_extract_readable_runtime_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn extract_readable_runtime_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_plugin_html_extract_readable_runtime_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_plugin_html_extract_readable_v1/tassadar_post_article_plugin_html_extract_readable_bundle.json"
        ));
    }

    #[test]
    fn feed_parse_success_returns_feed_metadata_and_entries() {
        let packet = serde_json::to_vec(&serde_json::json!({
            "source_url": "https://snapshot.example/feed.rss",
            "content_type": "application/rss+xml",
            "feed_text": r#"<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>Snapshot Feed</title><link>https://snapshot.example/</link><description>Snapshot feed updates.</description><item><title>Feed Entry</title><link>/posts/feed-entry</link><pubDate>Sat, 22 Mar 2026 00:00:00 GMT</pubDate><description>Snapshot entry summary.</description></item></channel></rss>"#
        }))
        .expect("packet");
        let outcome = invoke_feed_parse_json_packet("json", &packet, &FeedParseConfig::default());

        assert_eq!(
            outcome.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_FEED_RSS_ATOM_PARSE_OUTPUT_SCHEMA_ID
        );
        let response = outcome.response.expect("response");
        assert_eq!(response.feed_title.as_deref(), Some("Snapshot Feed"));
        assert_eq!(
            response.feed_homepage_url.as_deref(),
            Some("https://snapshot.example/")
        );
        assert_eq!(response.entries.len(), 1);
        assert_eq!(
            response.entries[0].link.as_deref(),
            Some("https://snapshot.example/posts/feed-entry")
        );
    }

    #[test]
    fn feed_parse_atom_success_resolves_links() {
        let packet = serde_json::to_vec(&serde_json::json!({
            "source_url": "https://snapshot.example/feed.atom",
            "content_type": "application/atom+xml",
            "feed_text": r#"<?xml version="1.0" encoding="utf-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>Snapshot Atom Feed</title><subtitle>Atom updates from the snapshot fixture.</subtitle><link href="https://snapshot.example/" /><entry><title>Atom Entry</title><link href="/posts/atom-entry" /><updated>2026-03-22T00:00:00Z</updated><summary>Atom summary text.</summary></entry></feed>"#
        }))
        .expect("packet");
        let outcome = invoke_feed_parse_json_packet("json", &packet, &FeedParseConfig::default());

        let response = outcome.response.expect("response");
        assert_eq!(response.feed_title.as_deref(), Some("Snapshot Atom Feed"));
        assert_eq!(response.entries.len(), 1);
        assert_eq!(
            response.entries[0].link.as_deref(),
            Some("https://snapshot.example/posts/atom-entry")
        );
        assert_eq!(
            response.entries[0].published_time.as_deref(),
            Some("2026-03-22T00:00:00Z")
        );
    }

    #[test]
    fn feed_parse_refuses_schema_invalid_unsupported_and_large_inputs() {
        let schema_invalid = invoke_feed_parse_json_packet(
            "json",
            br#"{"source_url":"https://snapshot.example/feed.rss","content_type":"application/rss+xml"}"#,
            &FeedParseConfig::default(),
        );
        assert_eq!(
            schema_invalid.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        );

        let unsupported = invoke_feed_parse_json_packet(
            "json",
            br#"{"source_url":"https://snapshot.example/opml.xml","content_type":"text/xml","feed_text":"<opml version=\"2.0\"></opml>"}"#,
            &FeedParseConfig::default(),
        );
        assert_eq!(
            unsupported.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID
        );

        let oversized = invoke_feed_parse_json_packet(
            "json",
            br#"{"source_url":"https://snapshot.example/feed.rss","content_type":"application/rss+xml","feed_text":"xxxxxxxxxxxxxxxx"}"#,
            &FeedParseConfig {
                input_size_limit_bytes: 8,
                max_entries: 8,
            },
        );
        assert_eq!(
            oversized.receipt.output_or_refusal_schema_id,
            STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID
        );
    }

    #[test]
    fn feed_parse_runtime_bundle_covers_declared_cases() {
        let bundle = build_feed_parse_runtime_bundle();

        assert_eq!(bundle.case_rows.len(), 5);
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| row.status == FeedParseRuntimeCaseStatus::ExactSuccess));
        assert!(bundle.case_rows.iter().any(|row| {
            row.status == FeedParseRuntimeCaseStatus::TypedMalformedPacket
                && row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_SCHEMA_INVALID_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_UNSUPPORTED_FEED_FORMAT_ID
        }));
        assert!(bundle.case_rows.iter().any(|row| {
            row.response_or_refusal_schema_id == STARTER_PLUGIN_REFUSAL_INPUT_TOO_LARGE_ID
        }));
        assert!(bundle.composition_case.green);
        assert_eq!(
            bundle.composition_case.step_plugin_ids,
            vec![
                String::from("plugin.http.fetch_text"),
                String::from("plugin.feed.rss_atom_parse"),
            ]
        );
    }

    #[test]
    fn feed_parse_runtime_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("feed_parse_bundle.json");
        let written = write_feed_parse_runtime_bundle(&output_path).expect("write bundle");
        let loaded: super::FeedParseRuntimeBundle =
            super::load_feed_parse_runtime_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn feed_parse_runtime_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_plugin_feed_rss_atom_parse_runtime_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_plugin_feed_rss_atom_parse_v1/tassadar_post_article_plugin_feed_rss_atom_parse_bundle.json"
        ));
    }
}
