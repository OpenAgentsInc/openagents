use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    bridge_exposed_starter_plugin_registrations,
    execute_reference_psion_plugin_guest_artifact_tool_call, invoke_extract_readable_json_packet,
    invoke_feed_parse_json_packet, invoke_fetch_text_json_packet, invoke_text_stats_json_packet,
    invoke_url_extract_json_packet, starter_plugin_registration_by_tool_name,
    starter_plugin_tool_projection_for_plugin_id, ExtractReadableConfig, FeedParseConfig,
    FetchTextConfig, FetchTextSnapshotResponse, FetchTextSnapshotResult,
    StarterPluginInvocationReceipt, StarterPluginInvocationStatus, StarterPluginRefusal,
    StarterPluginToolProjection, TextStatsConfig, UrlExtractConfig, STARTER_PLUGIN_GUEST_ECHO_ID,
};

pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_TOOL_BRIDGE_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_tool_bridge_v1/tassadar_post_article_starter_plugin_tool_bridge_bundle.json";
pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_TOOL_BRIDGE_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_tool_bridge_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarterPluginToolBridgeSurface {
    DeterministicWorkflow,
    RouterResponses,
    AppleFmSession,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginRouterToolDefinition {
    pub kind: String,
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginAppleFmToolDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub arguments_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginToolProjectionBridgeRow {
    pub plugin_id: String,
    pub tool_name: String,
    pub result_schema_id: String,
    pub refusal_schema_ids: Vec<String>,
    pub replay_class_id: String,
    pub deterministic_projection: StarterPluginToolProjection,
    pub router_projection: StarterPluginRouterToolDefinition,
    pub apple_fm_projection: StarterPluginAppleFmToolDefinition,
    pub arguments_schema_digest: String,
    pub router_parameters_digest: String,
    pub apple_fm_schema_digest: String,
    pub stable_across_surfaces: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginProjectedToolResultEnvelope {
    pub tool_name: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub status: StarterPluginInvocationStatus,
    pub output_or_refusal_schema_id: String,
    pub replay_class_id: String,
    pub structured_payload: Value,
    pub plugin_receipt: StarterPluginInvocationReceipt,
}

impl StarterPluginProjectedToolResultEnvelope {
    pub fn rendered_output(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginToolBridgeExecutionCase {
    pub case_id: String,
    pub tool_name: String,
    pub plugin_id: String,
    pub status: StarterPluginInvocationStatus,
    pub output_or_refusal_schema_id: String,
    pub structured_payload_digest: String,
    pub rendered_output_digest: String,
    pub plugin_receipt_id: String,
    pub plugin_receipt_digest: String,
    pub receipt_binding_preserved: bool,
    pub typed_refusal_preserved: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginToolBridgeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub surface_ids: Vec<StarterPluginToolBridgeSurface>,
    pub projection_rows: Vec<StarterPluginToolProjectionBridgeRow>,
    pub execution_cases: Vec<StarterPluginToolBridgeExecutionCase>,
    pub plugin_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StarterPluginToolBridgeConfig {
    pub url_extract: UrlExtractConfig,
    pub text_stats: TextStatsConfig,
    pub fetch_text: FetchTextConfig,
    pub extract_readable: ExtractReadableConfig,
    pub feed_parse: FeedParseConfig,
}

impl Default for StarterPluginToolBridgeConfig {
    fn default() -> Self {
        Self {
            url_extract: UrlExtractConfig::default(),
            text_stats: TextStatsConfig::default(),
            fetch_text: FetchTextConfig::snapshot(bridge_snapshot_entries()),
            extract_readable: ExtractReadableConfig::default(),
            feed_parse: FeedParseConfig::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum StarterPluginToolBridgeError {
    #[error("unknown starter-plugin tool `{tool_name}`")]
    UnknownTool { tool_name: String },
    #[error("failed to serialize tool arguments for `{tool_name}`: {error}")]
    EncodeArguments {
        tool_name: String,
        error: serde_json::Error,
    },
    #[error("failed to serialize projected tool result for `{tool_name}`: {error}")]
    EncodeProjectedResult {
        tool_name: String,
        error: serde_json::Error,
    },
    #[error("guest-artifact invocation failed for `{tool_name}`: {detail}")]
    GuestArtifactInvocation { tool_name: String, detail: String },
}

#[derive(Debug, Error)]
pub enum StarterPluginToolBridgeArtifactError {
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
pub fn starter_plugin_tool_bridge_projections() -> Vec<StarterPluginToolProjection> {
    bridge_exposed_starter_plugin_registrations()
        .into_iter()
        .filter_map(|registration| {
            starter_plugin_tool_projection_for_plugin_id(registration.plugin_id)
        })
        .collect()
}

#[must_use]
pub fn project_router_tool_definition(
    projection: &StarterPluginToolProjection,
) -> StarterPluginRouterToolDefinition {
    StarterPluginRouterToolDefinition {
        kind: String::from("function"),
        name: projection.tool_name.clone(),
        description: projection.description.clone(),
        parameters: projection.arguments_schema.clone(),
    }
}

#[must_use]
pub fn project_apple_fm_tool_definition(
    projection: &StarterPluginToolProjection,
) -> StarterPluginAppleFmToolDefinition {
    StarterPluginAppleFmToolDefinition {
        name: projection.tool_name.clone(),
        description: Some(projection.description.clone()),
        arguments_schema: projection.arguments_schema.clone(),
    }
}

pub fn execute_starter_plugin_tool_call(
    tool_name: &str,
    arguments: Value,
    config: &StarterPluginToolBridgeConfig,
) -> Result<StarterPluginProjectedToolResultEnvelope, StarterPluginToolBridgeError> {
    let packet = serde_json::to_vec(&arguments).map_err(|error| {
        StarterPluginToolBridgeError::EncodeArguments {
            tool_name: String::from(tool_name),
            error,
        }
    })?;
    let registration = starter_plugin_registration_by_tool_name(tool_name).ok_or_else(|| {
        StarterPluginToolBridgeError::UnknownTool {
            tool_name: String::from(tool_name),
        }
    })?;
    if !registration.bridge_exposed {
        return Err(StarterPluginToolBridgeError::UnknownTool {
            tool_name: String::from(tool_name),
        });
    }
    let envelope = match registration.plugin_id {
        "plugin.text.url_extract" => {
            let outcome = invoke_url_extract_json_packet("json", &packet, &config.url_extract);
            projected_tool_result_from_outcome(
                tool_name,
                outcome.receipt,
                outcome.response,
                outcome.refusal,
            )
        }
        "plugin.text.stats" => {
            let outcome = invoke_text_stats_json_packet("json", &packet, &config.text_stats);
            projected_tool_result_from_outcome(
                tool_name,
                outcome.receipt,
                outcome.response,
                outcome.refusal,
            )
        }
        "plugin.http.fetch_text" => {
            let outcome = invoke_fetch_text_json_packet("json", &packet, &config.fetch_text);
            projected_tool_result_from_outcome(
                tool_name,
                outcome.receipt,
                outcome.response,
                outcome.refusal,
            )
        }
        STARTER_PLUGIN_GUEST_ECHO_ID => execute_reference_psion_plugin_guest_artifact_tool_call(
            packet.as_slice(),
        )
        .map_err(|error| StarterPluginToolBridgeError::GuestArtifactInvocation {
            tool_name: String::from(tool_name),
            detail: error.to_string(),
        })?,
        "plugin.html.extract_readable" => {
            let outcome =
                invoke_extract_readable_json_packet("json", &packet, &config.extract_readable);
            projected_tool_result_from_outcome(
                tool_name,
                outcome.receipt,
                outcome.response,
                outcome.refusal,
            )
        }
        "plugin.feed.rss_atom_parse" => {
            let outcome = invoke_feed_parse_json_packet("json", &packet, &config.feed_parse);
            projected_tool_result_from_outcome(
                tool_name,
                outcome.receipt,
                outcome.response,
                outcome.refusal,
            )
        }
        _ => {
            return Err(StarterPluginToolBridgeError::UnknownTool {
                tool_name: String::from(tool_name),
            });
        }
    };
    let _ = envelope.rendered_output().map_err(|error| {
        StarterPluginToolBridgeError::EncodeProjectedResult {
            tool_name: String::from(tool_name),
            error,
        }
    })?;
    Ok(envelope)
}

#[must_use]
pub fn build_starter_plugin_tool_bridge_bundle() -> StarterPluginToolBridgeBundle {
    let projection_rows = starter_plugin_tool_bridge_projections()
        .into_iter()
        .map(|projection| {
            let router_projection = project_router_tool_definition(&projection);
            let apple_fm_projection = project_apple_fm_tool_definition(&projection);
            let arguments_schema_digest =
                stable_json_digest(b"starter_tool_bridge_schema|", &projection.arguments_schema);
            let router_parameters_digest = stable_json_digest(
                b"starter_tool_bridge_schema|",
                &router_projection.parameters,
            );
            let apple_fm_schema_digest = stable_json_digest(
                b"starter_tool_bridge_schema|",
                &apple_fm_projection.arguments_schema,
            );
            StarterPluginToolProjectionBridgeRow {
                plugin_id: projection.plugin_id.clone(),
                tool_name: projection.tool_name.clone(),
                result_schema_id: projection.result_schema_id.clone(),
                refusal_schema_ids: projection.refusal_schema_ids.clone(),
                replay_class_id: projection.replay_class_id.clone(),
                deterministic_projection: projection.clone(),
                router_projection,
                apple_fm_projection,
                stable_across_surfaces: projection.tool_name
                    == project_router_tool_definition(&projection).name
                    && projection.tool_name == project_apple_fm_tool_definition(&projection).name
                    && arguments_schema_digest == router_parameters_digest
                    && arguments_schema_digest == apple_fm_schema_digest,
                arguments_schema_digest,
                router_parameters_digest,
                apple_fm_schema_digest,
                detail: String::from(
                    "the shared bridge derives deterministic, router-owned, and Apple FM tool definitions from one starter-plugin projection without inventing a second schema vocabulary.",
                ),
            }
        })
        .collect::<Vec<_>>();

    let config = StarterPluginToolBridgeConfig::default();
    let execution_cases = vec![
        bridge_execution_case(
            "url_extract_success_bridge",
            execute_starter_plugin_tool_call(
                "plugin_text_url_extract",
                serde_json::json!({
                    "text": "see https://snapshot.example/feed.rss and https://snapshot.example/article"
                }),
                &config,
            )
            .expect("url extract bridge result"),
            "success results stay receipt-bound and structured at the shared bridge boundary.",
        ),
        bridge_execution_case(
            "text_stats_success_bridge",
            execute_starter_plugin_tool_call(
                "plugin_text_stats",
                serde_json::json!({
                    "text": "alpha beta\n\ngamma delta"
                }),
                &config,
            )
            .expect("text-stats bridge result"),
            "user-added capability-free plugins project through the shared bridge with the same receipt-bound structured envelope as the original starter set.",
        ),
        bridge_execution_case(
            "fetch_text_success_bridge",
            execute_starter_plugin_tool_call(
                "plugin_http_fetch_text",
                serde_json::json!({
                    "url": "https://snapshot.example/feed.rss"
                }),
                &config,
            )
            .expect("fetch-text bridge result"),
            "read-only network results stay structured and receipt-bound at the same bridge boundary.",
        ),
        bridge_execution_case(
            "guest_echo_success_bridge",
            execute_starter_plugin_tool_call(
                "plugin_example_echo_guest",
                serde_json::json!({
                    "text": "guest-artifact bridge proof"
                }),
                &config,
            )
            .expect("guest-artifact bridge result"),
            "the first digest-bound guest-artifact starter plugin projects through the same shared bridge and preserves the same receipt-bound structured envelope as the host-native starter surface.",
        ),
        bridge_execution_case(
            "extract_readable_success_bridge",
            execute_starter_plugin_tool_call(
                "plugin_html_extract_readable",
                serde_json::json!({
                    "source_url": "https://snapshot.example/article",
                    "content_type": "text/html",
                    "body_text": "<html lang=\"en\"><head><title>Snapshot Article</title></head><body><main><p>Bounded starter plugin content.</p></main></body></html>"
                }),
                &config,
            )
            .expect("extract-readable bridge result"),
            "local deterministic readability results carry the underlying plugin receipt across the tool boundary.",
        ),
        bridge_execution_case(
            "feed_parse_refusal_bridge",
            execute_starter_plugin_tool_call(
                "plugin_feed_rss_atom_parse",
                serde_json::json!({
                    "source_url": "https://snapshot.example/opml.xml",
                    "content_type": "text/xml",
                    "feed_text": "<opml version=\"2.0\"></opml>"
                }),
                &config,
            )
            .expect("feed-parse bridge result"),
            "typed plugin refusals stay typed and receipt-bound instead of degrading into free-form tool text.",
        ),
    ];

    let mut bundle = StarterPluginToolBridgeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.post_article.starter_plugin_tool_bridge.bundle.v1"),
        surface_ids: vec![
            StarterPluginToolBridgeSurface::DeterministicWorkflow,
            StarterPluginToolBridgeSurface::RouterResponses,
            StarterPluginToolBridgeSurface::AppleFmSession,
        ],
        projection_rows,
        execution_cases,
        plugin_count: 0,
        claim_boundary: String::from(
            "this bundle freezes one shared starter-plugin projection and receipt bridge above the plugin runtime and below deterministic, router-owned, and Apple FM controller lanes. It keeps tool-schema derivation, structured outputs, typed refusals, and plugin receipt identity explicit without claiming weighted controller closure.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "starter plugin tool bridge covers projection_rows={} and execution_cases={} across deterministic, router-owned, and Apple FM controller surfaces.",
        bundle.projection_rows.len(),
        bundle.execution_cases.len(),
    );
    bundle.plugin_count = bundle.projection_rows.len() as u32;
    bundle.bundle_digest = stable_json_digest(b"starter_plugin_tool_bridge_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_starter_plugin_tool_bridge_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_TOOL_BRIDGE_BUNDLE_REF)
}

pub fn write_starter_plugin_tool_bridge_bundle(
    output_path: impl AsRef<Path>,
) -> Result<StarterPluginToolBridgeBundle, StarterPluginToolBridgeArtifactError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            StarterPluginToolBridgeArtifactError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_starter_plugin_tool_bridge_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginToolBridgeArtifactError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_starter_plugin_tool_bridge_bundle(
    path: impl AsRef<Path>,
) -> Result<StarterPluginToolBridgeBundle, StarterPluginToolBridgeArtifactError> {
    read_json(path)
}

fn projected_tool_result_from_outcome<T: Serialize>(
    tool_name: &str,
    receipt: StarterPluginInvocationReceipt,
    response: Option<T>,
    refusal: Option<StarterPluginRefusal>,
) -> StarterPluginProjectedToolResultEnvelope {
    let structured_payload = response
        .map(|value| serde_json::to_value(value).unwrap_or_else(|_| Value::Null))
        .or_else(|| {
            refusal.map(|value| serde_json::to_value(value).unwrap_or_else(|_| Value::Null))
        })
        .unwrap_or(Value::Null);
    StarterPluginProjectedToolResultEnvelope {
        tool_name: String::from(tool_name),
        plugin_id: receipt.plugin_id.clone(),
        plugin_version: receipt.plugin_version.clone(),
        status: receipt.status,
        output_or_refusal_schema_id: receipt.output_or_refusal_schema_id.clone(),
        replay_class_id: receipt.replay_class_id.clone(),
        structured_payload,
        plugin_receipt: receipt,
    }
}

fn bridge_execution_case(
    case_id: &str,
    envelope: StarterPluginProjectedToolResultEnvelope,
    detail: &str,
) -> StarterPluginToolBridgeExecutionCase {
    let rendered_output = envelope
        .rendered_output()
        .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"));
    StarterPluginToolBridgeExecutionCase {
        case_id: String::from(case_id),
        tool_name: envelope.tool_name.clone(),
        plugin_id: envelope.plugin_id.clone(),
        status: envelope.status,
        output_or_refusal_schema_id: envelope.output_or_refusal_schema_id.clone(),
        structured_payload_digest: stable_json_digest(
            b"starter_plugin_tool_bridge_structured_payload|",
            &envelope.structured_payload,
        ),
        rendered_output_digest: stable_digest(
            b"starter_plugin_tool_bridge_rendered_output|",
            rendered_output.as_bytes(),
        ),
        plugin_receipt_id: envelope.plugin_receipt.receipt_id.clone(),
        plugin_receipt_digest: envelope.plugin_receipt.receipt_digest.clone(),
        receipt_binding_preserved: !envelope.plugin_receipt.receipt_id.is_empty()
            && !envelope.plugin_receipt.receipt_digest.is_empty(),
        typed_refusal_preserved: matches!(envelope.status, StarterPluginInvocationStatus::Refusal),
        detail: String::from(detail),
    }
}

fn stable_json_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value)
        .unwrap_or_else(|error| format!("serialization_error:{error}").into_bytes());
    stable_digest(prefix, &encoded)
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

fn bridge_snapshot_entries() -> std::collections::BTreeMap<String, FetchTextSnapshotResult> {
    std::collections::BTreeMap::from([
        (
            String::from("https://snapshot.example/feed.rss"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/feed.rss"),
                status_code: 200,
                content_type: String::from("application/rss+xml"),
                charset: Some(String::from("utf-8")),
                body_bytes: br#"<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>Snapshot Feed</title><link>https://snapshot.example/</link><description>Snapshot feed updates.</description><item><title>Feed Entry</title><link>/posts/feed-entry</link><description>Snapshot entry summary.</description></item></channel></rss>"#.to_vec(),
            }),
        ),
        (
            String::from("https://snapshot.example/article"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/article"),
                status_code: 200,
                content_type: String::from("text/html"),
                charset: Some(String::from("utf-8")),
                body_bytes: b"<html lang=\"en\"><head><title>Snapshot Article</title></head><body><main><p>Bounded starter plugin content.</p></main></body></html>".to_vec(),
            }),
        ),
    ])
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, StarterPluginToolBridgeArtifactError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| StarterPluginToolBridgeArtifactError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| StarterPluginToolBridgeArtifactError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(not(test))]
fn read_json<T: for<'de> Deserialize<'de>>(
    path: impl AsRef<Path>,
) -> Result<T, StarterPluginToolBridgeArtifactError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| StarterPluginToolBridgeArtifactError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| StarterPluginToolBridgeArtifactError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_starter_plugin_tool_bridge_bundle, execute_starter_plugin_tool_call,
        load_starter_plugin_tool_bridge_bundle, starter_plugin_tool_bridge_projections,
        tassadar_post_article_starter_plugin_tool_bridge_bundle_path,
        write_starter_plugin_tool_bridge_bundle, StarterPluginToolBridgeConfig,
        StarterPluginToolBridgeSurface,
    };
    use crate::StarterPluginInvocationStatus;
    use tempfile::tempdir;

    #[test]
    fn starter_plugin_tool_bridge_keeps_projection_rows_stable() {
        let bundle = build_starter_plugin_tool_bridge_bundle();

        assert_eq!(bundle.projection_rows.len(), 6);
        assert_eq!(
            bundle.surface_ids,
            vec![
                StarterPluginToolBridgeSurface::DeterministicWorkflow,
                StarterPluginToolBridgeSurface::RouterResponses,
                StarterPluginToolBridgeSurface::AppleFmSession,
            ]
        );
        assert!(bundle
            .projection_rows
            .iter()
            .all(|row| row.stable_across_surfaces));
        assert!(bundle
            .projection_rows
            .iter()
            .any(|row| row.tool_name == "plugin_text_stats"));
        assert!(bundle
            .projection_rows
            .iter()
            .any(|row| row.tool_name == "plugin_example_echo_guest"));
    }

    #[test]
    fn starter_plugin_tool_bridge_success_result_stays_receipt_bound() {
        let envelope = execute_starter_plugin_tool_call(
            "plugin_text_url_extract",
            serde_json::json!({
                "text": "https://snapshot.example/feed.rss"
            }),
            &StarterPluginToolBridgeConfig::default(),
        )
        .expect("bridge result");

        assert_eq!(envelope.plugin_id, "plugin.text.url_extract");
        assert_eq!(envelope.status, StarterPluginInvocationStatus::Success);
        assert!(!envelope.plugin_receipt.receipt_id.is_empty());
        assert!(envelope
            .rendered_output()
            .expect("rendered output")
            .contains("plugin_receipt"));
    }

    #[test]
    fn starter_plugin_tool_bridge_preserves_typed_refusals() {
        let envelope = execute_starter_plugin_tool_call(
            "plugin_feed_rss_atom_parse",
            serde_json::json!({
                "source_url": "https://snapshot.example/opml.xml",
                "content_type": "text/xml",
                "feed_text": "<opml version=\"2.0\"></opml>"
            }),
            &StarterPluginToolBridgeConfig::default(),
        )
        .expect("bridge result");

        assert_eq!(envelope.status, StarterPluginInvocationStatus::Refusal);
        assert_eq!(
            envelope.output_or_refusal_schema_id,
            "plugin.refusal.unsupported_feed_format.v1"
        );
        assert_eq!(
            envelope.plugin_receipt.refusal_class_id.as_deref(),
            Some("unsupported_feed_format")
        );
    }

    #[test]
    fn starter_plugin_tool_bridge_bundle_covers_projection_and_execution_rows() {
        let bundle = build_starter_plugin_tool_bridge_bundle();

        assert_eq!(starter_plugin_tool_bridge_projections().len(), 6);
        assert_eq!(bundle.execution_cases.len(), 6);
        assert!(bundle
            .execution_cases
            .iter()
            .any(|row| row.status == StarterPluginInvocationStatus::Refusal));
        assert!(bundle
            .execution_cases
            .iter()
            .any(|row| row.plugin_id == "plugin.text.stats"));
        assert!(bundle
            .execution_cases
            .iter()
            .any(|row| row.plugin_id == "plugin.example.echo_guest"));
        assert!(bundle
            .execution_cases
            .iter()
            .all(|row| row.receipt_binding_preserved));
    }

    #[test]
    fn starter_plugin_tool_bridge_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("starter_plugin_tool_bridge_bundle.json");
        let written = write_starter_plugin_tool_bridge_bundle(&output_path).expect("write bundle");
        let loaded = load_starter_plugin_tool_bridge_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn starter_plugin_tool_bridge_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_starter_plugin_tool_bridge_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_tool_bridge_v1/tassadar_post_article_starter_plugin_tool_bridge_bundle.json"
        ));
    }
}
