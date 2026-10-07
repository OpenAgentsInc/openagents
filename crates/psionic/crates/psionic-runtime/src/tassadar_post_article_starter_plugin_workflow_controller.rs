use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    execute_starter_plugin_tool_call, ExtractReadableResponse, FetchTextConfig, FetchTextResponse,
    FetchTextSnapshotResponse, FetchTextSnapshotResult, StarterPluginInvocationStatus,
    StarterPluginProjectedToolResultEnvelope, StarterPluginToolBridgeConfig,
    StarterPluginToolBridgeError, TextStatsResponse,
};

pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_WORKFLOW_CONTROLLER_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_workflow_controller_v1/tassadar_post_article_starter_plugin_workflow_controller_bundle.json";
pub const TASSADAR_POST_ARTICLE_STARTER_PLUGIN_WORKFLOW_CONTROLLER_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_workflow_controller_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowDecisionRow {
    pub decision_index: u16,
    pub decision_kind: String,
    pub subject_id: String,
    pub chosen_path_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowStepRow {
    pub step_index: u16,
    pub subject_id: String,
    pub tool_name: String,
    pub plugin_id: String,
    pub projected_result: StarterPluginProjectedToolResultEnvelope,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowRefusalRow {
    pub step_index: u16,
    pub tool_name: String,
    pub plugin_receipt_id: String,
    pub refusal_class_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowTextStatsRow {
    pub subject_id: String,
    pub byte_count: usize,
    pub word_count: usize,
    pub non_empty_line_count: usize,
    pub receipt_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowFinalArtifact {
    pub extracted_urls: Vec<String>,
    pub article_titles: Vec<String>,
    pub feed_titles: Vec<String>,
    pub text_stats_rows: Vec<StarterPluginWorkflowTextStatsRow>,
    pub step_receipt_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowCase {
    pub case_id: String,
    pub workflow_graph_id: String,
    pub directive_text: String,
    pub decision_rows: Vec<StarterPluginWorkflowDecisionRow>,
    pub step_rows: Vec<StarterPluginWorkflowStepRow>,
    pub refusal_rows: Vec<StarterPluginWorkflowRefusalRow>,
    pub final_artifact: StarterPluginWorkflowFinalArtifact,
    pub stop_condition_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterPluginWorkflowControllerBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub workflow_graph_id: String,
    pub case_rows: Vec<StarterPluginWorkflowCase>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum StarterPluginWorkflowControllerError {
    #[error(transparent)]
    Bridge(#[from] StarterPluginToolBridgeError),
    #[error("failed to decode structured payload for `{tool_name}`: {error}")]
    DecodePayload {
        tool_name: String,
        error: serde_json::Error,
    },
}

#[derive(Debug, Error)]
pub enum StarterPluginWorkflowControllerArtifactError {
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
pub fn build_starter_plugin_workflow_controller_bundle() -> StarterPluginWorkflowControllerBundle {
    let workflow_graph_id = "starter_flow.web_content_intake.v1";
    let case_rows = vec![
        run_web_content_success_case(workflow_graph_id).expect("web content success workflow case"),
        run_web_content_refusal_case(workflow_graph_id).expect("web content refusal workflow case"),
        run_guest_echo_success_case(workflow_graph_id)
            .expect("guest-artifact success workflow case"),
    ];
    let mut bundle = StarterPluginWorkflowControllerBundle {
        schema_version: 1,
        bundle_id: String::from(
            "tassadar.post_article.starter_plugin_workflow_controller.bundle.v1",
        ),
        workflow_graph_id: String::from(workflow_graph_id),
        case_rows,
        claim_boundary: String::from(
            "this bundle freezes one host-owned deterministic starter-plugin workflow controller above the shared bridge. It keeps URL extraction, fetch, content-type branching, readable-html extraction, readable-text measurement, feed parsing, refusal capture, and stop conditions explicit without claiming open-ended planning or weighted controller closure.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "starter workflow controller bundle covers case_rows={} over one host-owned web-content intake graph.",
        bundle.case_rows.len(),
    );
    bundle.bundle_digest =
        stable_json_digest(b"starter_plugin_workflow_controller_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_post_article_starter_plugin_workflow_controller_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_STARTER_PLUGIN_WORKFLOW_CONTROLLER_BUNDLE_REF)
}

pub fn write_starter_plugin_workflow_controller_bundle(
    output_path: impl AsRef<Path>,
) -> Result<StarterPluginWorkflowControllerBundle, StarterPluginWorkflowControllerArtifactError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            StarterPluginWorkflowControllerArtifactError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_starter_plugin_workflow_controller_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        StarterPluginWorkflowControllerArtifactError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

pub fn load_starter_plugin_workflow_controller_bundle(
    path: impl AsRef<Path>,
) -> Result<StarterPluginWorkflowControllerBundle, StarterPluginWorkflowControllerArtifactError> {
    read_json(path)
}

fn run_web_content_success_case(
    workflow_graph_id: &str,
) -> Result<StarterPluginWorkflowCase, StarterPluginWorkflowControllerError> {
    let directive_text =
        "Review https://snapshot.example/article and https://snapshot.example/feed.rss";
    run_workflow_case(
        "web_content_intake_success",
        workflow_graph_id,
        directive_text,
        deterministic_workflow_bridge_config(),
    )
}

fn run_web_content_refusal_case(
    workflow_graph_id: &str,
) -> Result<StarterPluginWorkflowCase, StarterPluginWorkflowControllerError> {
    let directive_text = "Review https://snapshot.example/binary";
    run_workflow_case(
        "web_content_intake_fetch_refusal",
        workflow_graph_id,
        directive_text,
        deterministic_workflow_bridge_config(),
    )
}

fn run_guest_echo_success_case(
    workflow_graph_id: &str,
) -> Result<StarterPluginWorkflowCase, StarterPluginWorkflowControllerError> {
    let projected_result = execute_starter_plugin_tool_call(
        "plugin_example_echo_guest",
        serde_json::json!({
            "text": "controller-visible guest-artifact echo"
        }),
        &deterministic_workflow_bridge_config(),
    )?;
    let echoed_text = decode_guest_echo_text(projected_result.structured_payload.clone())?;
    Ok(StarterPluginWorkflowCase {
        case_id: String::from("guest_artifact_echo_success"),
        workflow_graph_id: String::from(workflow_graph_id),
        directive_text: String::from("Echo one bounded guest-artifact packet through the shared starter-plugin controller surface."),
        decision_rows: vec![
            decision_row(
                0,
                "guest_artifact_branch",
                "guest_artifact_echo_success",
                "branch.plugin_example_echo_guest",
                "the host-owned controller admits the digest-bound guest-artifact starter plugin on the same shared bridge surface as the host-native starter set.",
            ),
            decision_row(
                1,
                "stop_condition",
                "guest_artifact_echo_success",
                "controller_stop.guest_artifact_echo_complete",
                "the host-owned controller stops after the guest-artifact result is accepted without hidden chaining.",
            ),
        ],
        step_rows: vec![workflow_step_row(
            0,
            "guest_artifact_echo_success",
            projected_result.clone(),
            "execute the first digest-bound guest-artifact starter plugin through the shared starter-plugin controller surface.",
        )],
        refusal_rows: Vec::new(),
        final_artifact: StarterPluginWorkflowFinalArtifact {
            extracted_urls: Vec::new(),
            article_titles: vec![echoed_text],
            feed_titles: Vec::new(),
            text_stats_rows: Vec::new(),
            step_receipt_ids: vec![projected_result.plugin_receipt.receipt_id.clone()],
        },
        stop_condition_id: String::from("controller_stop.guest_artifact_echo_complete"),
        green: true,
        detail: String::from(
            "the workflow controller now proves one bounded guest-artifact starter plugin can execute through the same shared bridge and receipt envelope as the host-native starter surface.",
        ),
    })
}

fn run_workflow_case(
    case_id: &str,
    workflow_graph_id: &str,
    directive_text: &str,
    config: StarterPluginToolBridgeConfig,
) -> Result<StarterPluginWorkflowCase, StarterPluginWorkflowControllerError> {
    let mut decision_rows = Vec::new();
    let mut step_rows = Vec::new();
    let mut refusal_rows = Vec::new();
    let mut article_titles = Vec::new();
    let mut feed_titles = Vec::new();
    let mut text_stats_rows = Vec::new();
    let mut step_receipt_ids = Vec::new();
    let mut step_index = 0_u16;
    let mut decision_index = 0_u16;
    let mut stop_condition_id = String::from("controller_stop.all_urls_processed");

    let url_extract = execute_starter_plugin_tool_call(
        "plugin_text_url_extract",
        serde_json::json!({ "text": directive_text }),
        &config,
    )?;
    step_receipt_ids.push(url_extract.plugin_receipt.receipt_id.clone());
    step_rows.push(workflow_step_row(
        step_index,
        "directive_text",
        url_extract.clone(),
        "extract candidate URLs from the directive text with the shared bridge.",
    ));
    let urls = decode_urls(url_extract.structured_payload.clone())?;

    for url in &urls {
        step_index = step_index.saturating_add(1);
        let fetch = execute_starter_plugin_tool_call(
            "plugin_http_fetch_text",
            serde_json::json!({ "url": url }),
            &config,
        )?;
        step_receipt_ids.push(fetch.plugin_receipt.receipt_id.clone());
        step_rows.push(workflow_step_row(
            step_index,
            url.as_str(),
            fetch.clone(),
            "fetch the URL through the shared bridge before any content-type branch decision.",
        ));

        if matches!(fetch.status, StarterPluginInvocationStatus::Refusal) {
            refusal_rows.push(refusal_row(
                step_index,
                &fetch,
                "the controller stops on typed plugin refusal instead of hiding retry logic.",
            ));
            decision_rows.push(decision_row(
                decision_index,
                "stop_condition",
                url.as_str(),
                "controller_stop.typed_refusal",
                "the host-owned controller stops immediately when fetch returns a typed refusal.",
            ));
            stop_condition_id = String::from("controller_stop.typed_refusal");
            break;
        }

        let fetch_response = decode_fetch_text_response(fetch.structured_payload.clone())?;
        if content_type_is_html(fetch_response.content_type.as_str()) {
            decision_rows.push(decision_row(
                decision_index,
                "content_type_branch",
                url.as_str(),
                "branch.extract_readable",
                "the host-owned controller routes html content into the readability plugin.",
            ));
            decision_index = decision_index.saturating_add(1);
            step_index = step_index.saturating_add(1);
            let extract = execute_starter_plugin_tool_call(
                "plugin_html_extract_readable",
                serde_json::json!({
                    "source_url": fetch_response.final_url,
                    "content_type": fetch_response.content_type,
                    "body_text": fetch_response.body_text
                }),
                &config,
            )?;
            step_receipt_ids.push(extract.plugin_receipt.receipt_id.clone());
            step_rows.push(workflow_step_row(
                step_index,
                url.as_str(),
                extract.clone(),
                "run bounded readability extraction on fetched html through the shared bridge.",
            ));
            let extract_response = decode_extract_response(extract.structured_payload.clone())?;
            if let Some(title) = extract_response.title.clone() {
                article_titles.push(title);
            }
            decision_rows.push(decision_row(
                decision_index,
                "post_extract_enrichment",
                url.as_str(),
                "branch.text_stats",
                "the host-owned controller routes bounded readable text into the user-added text-stats plugin for explicit packet-local measurement.",
            ));
            decision_index = decision_index.saturating_add(1);
            step_index = step_index.saturating_add(1);
            let text_stats = execute_starter_plugin_tool_call(
                "plugin_text_stats",
                serde_json::json!({
                    "text": extract_response.readable_text
                }),
                &config,
            )?;
            step_receipt_ids.push(text_stats.plugin_receipt.receipt_id.clone());
            step_rows.push(workflow_step_row(
                step_index,
                url.as_str(),
                text_stats.clone(),
                "measure extracted readable text with the user-added capability-free text-stats plugin through the shared bridge.",
            ));
            if matches!(text_stats.status, StarterPluginInvocationStatus::Refusal) {
                refusal_rows.push(refusal_row(
                    step_index,
                    &text_stats,
                    "the controller stops if the user-added text-stats plugin returns a typed refusal.",
                ));
                decision_rows.push(decision_row(
                    decision_index,
                    "stop_condition",
                    url.as_str(),
                    "controller_stop.typed_refusal",
                    "the host-owned controller stops immediately when text-stats returns a typed refusal.",
                ));
                stop_condition_id = String::from("controller_stop.typed_refusal");
                break;
            }
            let text_stats_response = decode_text_stats_response(text_stats.structured_payload)?;
            text_stats_rows.push(StarterPluginWorkflowTextStatsRow {
                subject_id: String::from(url),
                byte_count: text_stats_response.byte_count,
                word_count: text_stats_response.word_count,
                non_empty_line_count: text_stats_response.non_empty_line_count,
                receipt_id: text_stats.plugin_receipt.receipt_id,
            });
            continue;
        }

        if content_type_is_feed(fetch_response.content_type.as_str()) {
            decision_rows.push(decision_row(
                decision_index,
                "content_type_branch",
                url.as_str(),
                "branch.feed_parse",
                "the host-owned controller routes rss-or-atom content into the feed parser.",
            ));
            decision_index = decision_index.saturating_add(1);
            step_index = step_index.saturating_add(1);
            let parse = execute_starter_plugin_tool_call(
                "plugin_feed_rss_atom_parse",
                serde_json::json!({
                    "source_url": fetch_response.final_url,
                    "content_type": fetch_response.content_type,
                    "feed_text": fetch_response.body_text
                }),
                &config,
            )?;
            step_receipt_ids.push(parse.plugin_receipt.receipt_id.clone());
            step_rows.push(workflow_step_row(
                step_index,
                url.as_str(),
                parse.clone(),
                "run bounded feed parsing on fetched rss-or-atom content through the shared bridge.",
            ));
            let feed_title =
                decode_feed_title(parse.structured_payload.clone(), parse.tool_name.as_str())?;
            if let Some(title) = feed_title {
                feed_titles.push(title);
            }
            continue;
        }

        decision_rows.push(decision_row(
            decision_index,
            "stop_condition",
            url.as_str(),
            "controller_stop.unsupported_content_type",
            "the host-owned controller stops when fetch succeeds but the content type is outside the explicit html-or-feed branch window.",
        ));
        stop_condition_id = String::from("controller_stop.unsupported_content_type");
        break;
    }

    if stop_condition_id == "controller_stop.all_urls_processed" {
        decision_rows.push(decision_row(
            decision_index,
            "stop_condition",
            case_id,
            "controller_stop.all_urls_processed",
            "the host-owned controller stops after the extracted URL set is exhausted.",
        ));
    }

    Ok(StarterPluginWorkflowCase {
        case_id: String::from(case_id),
        workflow_graph_id: String::from(workflow_graph_id),
        directive_text: String::from(directive_text),
        decision_rows,
        step_rows: step_rows.clone(),
        refusal_rows,
        final_artifact: StarterPluginWorkflowFinalArtifact {
            extracted_urls: urls,
            article_titles,
            feed_titles,
            text_stats_rows,
            step_receipt_ids,
        },
        stop_condition_id,
        green: case_id == "web_content_intake_success",
        detail: String::from(
            "the deterministic controller keeps extraction, fetch, branch, user-added text measurement, refusal, and stop semantics explicit above the shared starter-plugin bridge.",
        ),
    })
}

fn workflow_step_row(
    step_index: u16,
    subject_id: &str,
    projected_result: StarterPluginProjectedToolResultEnvelope,
    detail: &str,
) -> StarterPluginWorkflowStepRow {
    StarterPluginWorkflowStepRow {
        step_index,
        subject_id: String::from(subject_id),
        tool_name: projected_result.tool_name.clone(),
        plugin_id: projected_result.plugin_id.clone(),
        projected_result,
        detail: String::from(detail),
    }
}

fn decision_row(
    decision_index: u16,
    decision_kind: &str,
    subject_id: &str,
    chosen_path_id: &str,
    detail: &str,
) -> StarterPluginWorkflowDecisionRow {
    StarterPluginWorkflowDecisionRow {
        decision_index,
        decision_kind: String::from(decision_kind),
        subject_id: String::from(subject_id),
        chosen_path_id: String::from(chosen_path_id),
        detail: String::from(detail),
    }
}

fn refusal_row(
    step_index: u16,
    projected_result: &StarterPluginProjectedToolResultEnvelope,
    detail: &str,
) -> StarterPluginWorkflowRefusalRow {
    StarterPluginWorkflowRefusalRow {
        step_index,
        tool_name: projected_result.tool_name.clone(),
        plugin_receipt_id: projected_result.plugin_receipt.receipt_id.clone(),
        refusal_class_id: projected_result
            .plugin_receipt
            .refusal_class_id
            .clone()
            .unwrap_or_else(|| String::from("unknown_refusal")),
        detail: String::from(detail),
    }
}

fn decode_urls(
    payload: serde_json::Value,
) -> Result<Vec<String>, StarterPluginWorkflowControllerError> {
    #[derive(Deserialize)]
    struct UrlExtractPayload {
        urls: Vec<String>,
    }
    serde_json::from_value::<UrlExtractPayload>(payload)
        .map(|payload| payload.urls)
        .map_err(
            |error| StarterPluginWorkflowControllerError::DecodePayload {
                tool_name: String::from("plugin_text_url_extract"),
                error,
            },
        )
}

fn decode_fetch_text_response(
    payload: serde_json::Value,
) -> Result<FetchTextResponse, StarterPluginWorkflowControllerError> {
    serde_json::from_value::<FetchTextResponse>(payload).map_err(|error| {
        StarterPluginWorkflowControllerError::DecodePayload {
            tool_name: String::from("plugin_http_fetch_text"),
            error,
        }
    })
}

fn decode_extract_response(
    payload: serde_json::Value,
) -> Result<ExtractReadableResponse, StarterPluginWorkflowControllerError> {
    serde_json::from_value::<ExtractReadableResponse>(payload).map_err(|error| {
        StarterPluginWorkflowControllerError::DecodePayload {
            tool_name: String::from("plugin_html_extract_readable"),
            error,
        }
    })
}

fn decode_feed_title(
    payload: serde_json::Value,
    tool_name: &str,
) -> Result<Option<String>, StarterPluginWorkflowControllerError> {
    #[derive(Deserialize)]
    struct FeedPayload {
        feed_title: Option<String>,
    }
    serde_json::from_value::<FeedPayload>(payload)
        .map(|payload| payload.feed_title)
        .map_err(
            |error| StarterPluginWorkflowControllerError::DecodePayload {
                tool_name: String::from(tool_name),
                error,
            },
        )
}

fn decode_text_stats_response(
    payload: serde_json::Value,
) -> Result<TextStatsResponse, StarterPluginWorkflowControllerError> {
    serde_json::from_value::<TextStatsResponse>(payload).map_err(|error| {
        StarterPluginWorkflowControllerError::DecodePayload {
            tool_name: String::from("plugin_text_stats"),
            error,
        }
    })
}

fn decode_guest_echo_text(
    payload: serde_json::Value,
) -> Result<String, StarterPluginWorkflowControllerError> {
    #[derive(Deserialize)]
    struct GuestEchoPayload {
        text: String,
    }
    serde_json::from_value::<GuestEchoPayload>(payload)
        .map(|payload| payload.text)
        .map_err(|error| StarterPluginWorkflowControllerError::DecodePayload {
            tool_name: String::from("plugin_example_echo_guest"),
            error,
        })
}

fn content_type_is_html(content_type: &str) -> bool {
    matches!(content_type, "text/html" | "application/xhtml+xml")
}

fn content_type_is_feed(content_type: &str) -> bool {
    matches!(
        content_type,
        "application/rss+xml" | "application/atom+xml" | "application/xml" | "text/xml"
    )
}

fn deterministic_workflow_bridge_config() -> StarterPluginToolBridgeConfig {
    StarterPluginToolBridgeConfig {
        fetch_text: FetchTextConfig::snapshot(BTreeMap::from([
            (
                String::from("https://snapshot.example/article"),
                FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                    final_url: String::from("https://snapshot.example/article"),
                    status_code: 200,
                    content_type: String::from("text/html"),
                    charset: Some(String::from("utf-8")),
                    body_bytes: b"<html lang=\"en\"><head><title>Snapshot Article</title></head><body><main><h1>Snapshot Article</h1><p>Bounded starter plugin content.</p></main></body></html>".to_vec(),
                }),
            ),
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
                String::from("https://snapshot.example/binary"),
                FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                    final_url: String::from("https://snapshot.example/binary"),
                    status_code: 200,
                    content_type: String::from("image/png"),
                    charset: None,
                    body_bytes: vec![0x89, 0x50, 0x4e, 0x47],
                }),
            ),
        ])),
        ..StarterPluginToolBridgeConfig::default()
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, StarterPluginWorkflowControllerArtifactError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| StarterPluginWorkflowControllerArtifactError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        StarterPluginWorkflowControllerArtifactError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(not(test))]
fn read_json<T: for<'de> Deserialize<'de>>(
    path: impl AsRef<Path>,
) -> Result<T, StarterPluginWorkflowControllerArtifactError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| StarterPluginWorkflowControllerArtifactError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        StarterPluginWorkflowControllerArtifactError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_starter_plugin_workflow_controller_bundle, deterministic_workflow_bridge_config,
        load_starter_plugin_workflow_controller_bundle, run_guest_echo_success_case,
        run_web_content_refusal_case, run_web_content_success_case,
        tassadar_post_article_starter_plugin_workflow_controller_bundle_path,
        write_starter_plugin_workflow_controller_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn starter_plugin_workflow_success_case_runs_multiple_plugins_in_sequence() {
        let case = run_web_content_success_case("starter_flow.web_content_intake.v1")
            .expect("success workflow case");

        assert!(case.green);
        assert_eq!(case.step_rows.len(), 6);
        assert_eq!(
            case.final_artifact.article_titles,
            vec![String::from("Snapshot Article")]
        );
        assert_eq!(
            case.final_artifact.feed_titles,
            vec![String::from("Snapshot Feed")]
        );
        assert_eq!(case.final_artifact.text_stats_rows.len(), 1);
        assert_eq!(case.final_artifact.text_stats_rows[0].word_count, 6);
        assert!(case
            .step_rows
            .iter()
            .any(|row| row.tool_name == "plugin_text_stats"));
        assert_eq!(case.stop_condition_id, "controller_stop.all_urls_processed");
    }

    #[test]
    fn starter_plugin_workflow_refusal_case_keeps_refusal_rows_and_stop_condition() {
        let case = run_web_content_refusal_case("starter_flow.web_content_intake.v1")
            .expect("refusal workflow case");

        assert!(!case.green);
        assert_eq!(case.refusal_rows.len(), 1);
        assert_eq!(
            case.refusal_rows[0].refusal_class_id,
            "content_type_unsupported"
        );
        assert_eq!(case.stop_condition_id, "controller_stop.typed_refusal");
    }

    #[test]
    fn starter_plugin_workflow_guest_artifact_case_uses_shared_bridge_surface() {
        let case = run_guest_echo_success_case("starter_flow.web_content_intake.v1")
            .expect("guest workflow case");

        assert!(case.green);
        assert_eq!(case.step_rows.len(), 1);
        assert_eq!(case.step_rows[0].tool_name, "plugin_example_echo_guest");
        assert_eq!(
            case.final_artifact.article_titles,
            vec![String::from("controller-visible guest-artifact echo")]
        );
    }

    #[test]
    fn starter_plugin_workflow_bundle_covers_success_and_refusal_cases() {
        let bundle = build_starter_plugin_workflow_controller_bundle();

        assert_eq!(bundle.case_rows.len(), 3);
        assert!(bundle.case_rows.iter().any(|row| row.green));
        assert!(bundle.case_rows.iter().any(|row| !row.green));
        assert!(bundle
            .case_rows
            .iter()
            .any(|row| row.case_id == "guest_artifact_echo_success"));
    }

    #[test]
    fn starter_plugin_workflow_bundle_writes_and_loads() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("starter_plugin_workflow_controller_bundle.json");
        let written =
            write_starter_plugin_workflow_controller_bundle(&output_path).expect("write bundle");
        let loaded =
            load_starter_plugin_workflow_controller_bundle(&output_path).expect("load bundle");

        assert_eq!(written, loaded);
        assert!(output_path.exists());
    }

    #[test]
    fn starter_plugin_workflow_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_starter_plugin_workflow_controller_bundle_path();
        assert!(path.ends_with(
            "fixtures/tassadar/runs/tassadar_post_article_starter_plugin_workflow_controller_v1/tassadar_post_article_starter_plugin_workflow_controller_bundle.json"
        ));
    }

    #[test]
    fn starter_plugin_workflow_bridge_config_keeps_binary_refusal_fixture() {
        let config = deterministic_workflow_bridge_config();
        let fetch_result = crate::execute_starter_plugin_tool_call(
            "plugin_http_fetch_text",
            serde_json::json!({ "url": "https://snapshot.example/binary" }),
            &config,
        )
        .expect("fetch result");

        assert_eq!(
            fetch_result.plugin_receipt.refusal_class_id.as_deref(),
            Some("content_type_unsupported")
        );
    }
}
