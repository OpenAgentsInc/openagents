use super::{
    OpenAiCompatConfig, OpenAiCompatServer, OpenAiCompatServerError, PromptMessage,
    PromptMessageRole, ResolvedToolCall, ResponsesInput, ResponsesRequest, ToolDefinitionEnvelope,
    ToolDefinitionRequest, assistant_prompt_message_for_tool_loop, handle_generic_responses,
    tool_loop_tool_call_from_resolved,
};
use axum::{body::to_bytes, response::Response};
use psionic_models::{GgufMetadataValue, GgufTensorType};
use psionic_router::{
    ToolLoopController, ToolLoopError, ToolLoopModelRunner, ToolLoopModelTurn, ToolLoopOutcome,
    ToolLoopPolicy, ToolLoopRequest, tassadar_starter_plugin_router_tool_definitions,
    tassadar_starter_plugin_tool_loop_gateway,
};
use psionic_runtime::{
    FetchTextConfig, FetchTextSnapshotResponse, FetchTextSnapshotResult,
    StarterPluginProjectedToolResultEnvelope, StarterPluginToolBridgeConfig,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::tempdir;
use thiserror::Error;

pub const TASSADAR_POST_ARTICLE_ROUTER_PLUGIN_TOOL_LOOP_PILOT_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_router_plugin_tool_loop_pilot_v1/tassadar_post_article_router_plugin_tool_loop_pilot_bundle.json";
pub const TASSADAR_POST_ARTICLE_ROUTER_PLUGIN_TOOL_LOOP_PILOT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_router_plugin_tool_loop_pilot_v1";

const SCHEMA_VERSION: u16 = 1;
const SUCCESS_CASE_ID: &str = "router_plugin_tool_loop_success";
const REFUSAL_CASE_ID: &str = "router_plugin_tool_loop_fetch_refusal";
const SUCCESS_MODEL_NAME: &str = "tiny-router-plugin-tool-success-llama";
const REFUSAL_MODEL_NAME: &str = "tiny-router-plugin-tool-refusal-llama";
const SUCCESS_DIRECTIVE: &str =
    "Read https://snapshot.example/article and https://snapshot.example/feed.rss.";
const REFUSAL_DIRECTIVE: &str = "Read https://snapshot.example/binary.";
const CONTINUATION_PROMPT: &str = "Continue the same bounded plugin workflow.";
const SUCCESS_FINAL_MESSAGE: &str =
    "I extracted one article and one feed through the router-owned plugin tool loop.";
const REFUSAL_FINAL_MESSAGE: &str =
    "The router-owned plugin tool loop stopped after a typed fetch refusal.";
const SUCCESS_FEED_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>Snapshot Feed</title><link>https://snapshot.example/</link><description>Snapshot feed updates.</description><item><title>Feed Entry</title><link>/posts/feed-entry</link><description>Snapshot entry summary.</description></item></channel></rss>"#;
const SUCCESS_ARTICLE_HTML: &str = r#"<html lang="en"><head><title>Snapshot Article</title></head><body><main><p>Bounded starter plugin content.</p></main></body></html>"#;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRouterPluginToolDefinitionRow {
    pub tool_name: String,
    pub description: String,
    pub parameters_digest: String,
    pub stable_kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleServedSeedResponseRow {
    pub response_id: String,
    pub conversation_id: String,
    pub conversation_revision: u64,
    pub tool_call_names: Vec<String>,
    pub route_worker: String,
    pub route_strategy: String,
    pub stored: bool,
    pub replayed_prompt_messages: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRouterPluginReceiptRow {
    pub step_index: usize,
    pub tool_call_id: String,
    pub tool_name: String,
    pub plugin_id: String,
    pub plugin_receipt_id: String,
    pub plugin_receipt_digest: String,
    pub status: String,
    pub output_or_refusal_schema_id: String,
    pub replay_class_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPostArticleRouterPluginToolLoopCaseRow {
    pub case_id: String,
    pub directive: String,
    pub served_seed: TassadarPostArticleServedSeedResponseRow,
    pub tool_loop_outcome: ToolLoopOutcome,
    pub receipt_rows: Vec<TassadarPostArticleRouterPluginReceiptRow>,
    pub observed_history_lens: Vec<usize>,
    pub max_steps: usize,
    pub bounded_step_count_preserved: bool,
    pub structured_plugin_outputs_preserved: bool,
    pub typed_refusal_preserved: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRouterPluginToolLoopContinuationRow {
    pub previous_response_id: String,
    pub response_id: String,
    pub conversation_id: String,
    pub conversation_revision: u64,
    pub tool_call_names: Vec<String>,
    pub route_worker: String,
    pub route_strategy: String,
    pub stored: bool,
    pub replayed_prompt_messages: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPostArticleRouterPluginToolLoopPilotBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub tool_definition_rows: Vec<TassadarPostArticleRouterPluginToolDefinitionRow>,
    pub case_rows: Vec<TassadarPostArticleRouterPluginToolLoopCaseRow>,
    pub continuation_row: TassadarPostArticleRouterPluginToolLoopContinuationRow,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleRouterPluginToolLoopPilotError {
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
    #[error("failed to initialize router pilot server: {0}")]
    Server(String),
    #[error("router pilot request failed: {0}")]
    Request(String),
    #[error("router pilot bundle violated an expected machine-readable field: {0}")]
    Bundle(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn tassadar_post_article_router_plugin_tool_loop_pilot_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_ROUTER_PLUGIN_TOOL_LOOP_PILOT_BUNDLE_REF)
}

pub fn build_tassadar_post_article_router_plugin_tool_loop_pilot_bundle() -> Result<
    TassadarPostArticleRouterPluginToolLoopPilotBundle,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let tool_definition_rows = tool_definition_rows();
    let (success_case, continuation_row) = run_success_case()?;
    let refusal_case = run_refusal_case()?;
    let mut bundle = TassadarPostArticleRouterPluginToolLoopPilotBundle {
        schema_version: SCHEMA_VERSION,
        bundle_id: String::from("tassadar.post_article.router_plugin_tool_loop_pilot.bundle.v1"),
        tool_definition_rows,
        case_rows: vec![success_case, refusal_case],
        continuation_row,
        claim_boundary: String::from(
            "this pilot freezes one bounded router-owned plugin tool loop on `/v1/responses` with shared starter-plugin runtime execution, structured tool envelopes, typed refusals, response-state continuation, and explicit route truth. It does not claim weighted plugin closure, arbitrary model planning, or canonical Tassadar controller completion.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "router-owned plugin tool-loop pilot covers tool_definitions={}, case_rows={}, and one continuation receipt above the shared starter-plugin runtime bridge.",
        bundle.tool_definition_rows.len(),
        bundle.case_rows.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"tassadar_post_article_router_plugin_tool_loop_pilot_bundle|",
        &bundle,
    );
    Ok(bundle)
}

pub fn write_tassadar_post_article_router_plugin_tool_loop_pilot_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleRouterPluginToolLoopPilotBundle,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleRouterPluginToolLoopPilotError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_post_article_router_plugin_tool_loop_pilot_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_post_article_router_plugin_tool_loop_pilot_bundle(
    path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleRouterPluginToolLoopPilotBundle,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn run_success_case() -> Result<
    (
        TassadarPostArticleRouterPluginToolLoopCaseRow,
        TassadarPostArticleRouterPluginToolLoopContinuationRow,
    ),
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let config = StarterPluginToolBridgeConfig::default();
    let (server, seed_payload, seed_headers) =
        seeded_server_and_response(SUCCESS_MODEL_NAME, SUCCESS_DIRECTIVE)?;
    let seed_turn = turn_from_seed_payload(&seed_payload)?;
    let seed_row = seed_row_from_payload(&seed_payload, &seed_headers)?;
    let gateway = tassadar_starter_plugin_tool_loop_gateway(config.clone());
    let controller = ToolLoopController::new(&server.state.router, &gateway);
    let mut runner = ScriptedServeToolLoopRunner {
        turns: vec![
            seed_turn,
            scripted_tool_turn(
                "Fetching article HTML through the router-owned plugin gateway.",
                "tool-1",
                "plugin_http_fetch_text",
                serde_json::json!({"url": "https://snapshot.example/article"}),
            ),
            scripted_tool_turn(
                "Extracting readable article content.",
                "tool-2",
                "plugin_html_extract_readable",
                serde_json::json!({
                    "source_url": "https://snapshot.example/article",
                    "content_type": "text/html",
                    "body_text": SUCCESS_ARTICLE_HTML
                }),
            ),
            scripted_tool_turn(
                "Fetching feed XML through the same gateway.",
                "tool-3",
                "plugin_http_fetch_text",
                serde_json::json!({"url": "https://snapshot.example/feed.rss"}),
            ),
            scripted_tool_turn(
                "Parsing the feed as RSS or Atom.",
                "tool-4",
                "plugin_feed_rss_atom_parse",
                serde_json::json!({
                    "source_url": "https://snapshot.example/feed.rss",
                    "content_type": "application/rss+xml",
                    "feed_text": SUCCESS_FEED_XML
                }),
            ),
            scripted_final_turn(SUCCESS_FINAL_MESSAGE),
        ],
        observed_history_lens: Vec::new(),
    };
    let max_steps = 6;
    let mut outcome = controller
        .run(
            ToolLoopRequest::new(
                psionic_router::RoutingRequest::new(psionic_router::RoutingEndpoint::Responses)
                    .require_tool_calling(),
                vec![PromptMessage::new(
                    PromptMessageRole::User,
                    SUCCESS_DIRECTIVE,
                )],
            )
            .with_policy(ToolLoopPolicy { max_steps }),
            &mut runner,
        )
        .map_err(|error| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
        })?;
    stabilize_tool_loop_outcome(&mut outcome);
    let receipt_rows = receipt_rows_from_outcome(&outcome)?;
    let continuation_row = continuation_row_from_server(
        &server,
        seed_row.response_id.as_str(),
        seed_row.conversation_id.as_str(),
    )?;
    Ok((
        TassadarPostArticleRouterPluginToolLoopCaseRow {
            case_id: String::from(SUCCESS_CASE_ID),
            directive: String::from(SUCCESS_DIRECTIVE),
            served_seed: seed_row,
            bounded_step_count_preserved: outcome.steps.len() <= max_steps,
            structured_plugin_outputs_preserved: receipt_rows.len()
                == outcome
                    .steps
                    .iter()
                    .map(|step| step.tool_results.len())
                    .sum::<usize>(),
            typed_refusal_preserved: receipt_rows
                .iter()
                .any(|row| row.status.as_str() == "refusal"),
            tool_loop_outcome: outcome,
            receipt_rows,
            observed_history_lens: runner.observed_history_lens,
            max_steps,
            detail: String::from(
                "the first step comes from a real `/v1/responses` tool-call response, while the bounded follow-on turns stay router-owned and execute real starter plugins through the shared bridge.",
            ),
        },
        continuation_row,
    ))
}

fn run_refusal_case() -> Result<
    TassadarPostArticleRouterPluginToolLoopCaseRow,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let mut snapshots = BTreeMap::new();
    let _ = snapshots.insert(
        String::from("https://snapshot.example/binary"),
        FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
            final_url: String::from("https://snapshot.example/binary"),
            status_code: 200,
            content_type: String::from("image/png"),
            charset: None,
            body_bytes: vec![0x89, b'P', b'N', b'G'],
        }),
    );
    let config = StarterPluginToolBridgeConfig {
        fetch_text: FetchTextConfig::snapshot(snapshots),
        ..StarterPluginToolBridgeConfig::default()
    };
    let (server, seed_payload, seed_headers) =
        seeded_server_and_response(REFUSAL_MODEL_NAME, REFUSAL_DIRECTIVE)?;
    let seed_turn = turn_from_seed_payload(&seed_payload)?;
    let seed_row = seed_row_from_payload(&seed_payload, &seed_headers)?;
    let gateway = tassadar_starter_plugin_tool_loop_gateway(config);
    let controller = ToolLoopController::new(&server.state.router, &gateway);
    let mut runner = ScriptedServeToolLoopRunner {
        turns: vec![
            seed_turn,
            scripted_tool_turn(
                "Fetching the extracted binary URL.",
                "tool-1",
                "plugin_http_fetch_text",
                serde_json::json!({"url": "https://snapshot.example/binary"}),
            ),
            scripted_final_turn(REFUSAL_FINAL_MESSAGE),
        ],
        observed_history_lens: Vec::new(),
    };
    let max_steps = 3;
    let mut outcome = controller
        .run(
            ToolLoopRequest::new(
                psionic_router::RoutingRequest::new(psionic_router::RoutingEndpoint::Responses)
                    .require_tool_calling(),
                vec![PromptMessage::new(
                    PromptMessageRole::User,
                    REFUSAL_DIRECTIVE,
                )],
            )
            .with_policy(ToolLoopPolicy { max_steps }),
            &mut runner,
        )
        .map_err(|error| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
        })?;
    stabilize_tool_loop_outcome(&mut outcome);
    let receipt_rows = receipt_rows_from_outcome(&outcome)?;
    Ok(TassadarPostArticleRouterPluginToolLoopCaseRow {
        case_id: String::from(REFUSAL_CASE_ID),
        directive: String::from(REFUSAL_DIRECTIVE),
        served_seed: seed_row,
        bounded_step_count_preserved: outcome.steps.len() <= max_steps,
        structured_plugin_outputs_preserved: receipt_rows.len()
            == outcome
                .steps
                .iter()
                .map(|step| step.tool_results.len())
                .sum::<usize>(),
        typed_refusal_preserved: receipt_rows
            .iter()
            .any(|row| row.status.as_str() == "refusal"),
        tool_loop_outcome: outcome,
        receipt_rows,
        observed_history_lens: runner.observed_history_lens,
        max_steps,
        detail: String::from(
            "the refusal case proves typed fetch refusals remain structured and receipt-bound after crossing the router-owned tool loop.",
        ),
    })
}

fn seeded_server_and_response(
    model_name: &str,
    directive: &str,
) -> Result<
    (
        OpenAiCompatServer,
        serde_json::Value,
        BTreeMap<String, String>,
    ),
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let tempdir = tempdir().map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
    })?;
    let model_path = tempdir.path().join("tiny-router-plugin-tool.gguf");
    write_router_plugin_tool_call_gguf(&model_path, model_name, directive)?;
    let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&model_path))
        .map_err(server_error)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
    })?;
    let response = runtime
        .block_on(handle_generic_responses(
            Arc::clone(&server.state),
            ResponsesRequest {
                model: None,
                instructions: None,
                conversation: None,
                input: ResponsesInput::Text(String::from(directive)),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: router_tool_definitions(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
                ..Default::default()
            },
        ))
        .map_err(|error| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
        })?;
    let headers = response_header_map(&response);
    let payload = runtime.block_on(response_json(response))?;
    Ok((server, payload, headers))
}

fn continuation_row_from_server(
    server: &OpenAiCompatServer,
    previous_response_id: &str,
    conversation_id: &str,
) -> Result<
    TassadarPostArticleRouterPluginToolLoopContinuationRow,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let runtime = tokio::runtime::Runtime::new().map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
    })?;
    let response = runtime
        .block_on(handle_generic_responses(
            Arc::clone(&server.state),
            ResponsesRequest {
                model: None,
                instructions: None,
                conversation: Some(String::from(conversation_id)),
                input: ResponsesInput::Text(String::from(CONTINUATION_PROMPT)),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: router_tool_definitions(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
                ..Default::default()
            },
        ))
        .map_err(|error| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
        })?;
    let headers = response_header_map(&response);
    let payload = runtime.block_on(response_json(response))?;
    Ok(TassadarPostArticleRouterPluginToolLoopContinuationRow {
        previous_response_id: String::from(previous_response_id),
        response_id: required_string(&payload, &["id"])?,
        conversation_id: required_string(&payload, &["conversation", "id"])?,
        conversation_revision: required_u64(&payload, &["conversation", "revision"])?,
        tool_call_names: required_tool_call_names(&payload)?,
        route_worker: required_header(&headers, "x-psionic-route-worker")?,
        route_strategy: required_header(&headers, "x-psionic-route-strategy")?,
        stored: required_bool(&payload, &["psionic_response_state", "stored"])?,
        replayed_prompt_messages: required_u64(
            &payload,
            &["psionic_response_state", "replayed_prompt_messages"],
        )?,
    })
}

fn tool_definition_rows() -> Vec<TassadarPostArticleRouterPluginToolDefinitionRow> {
    tassadar_starter_plugin_router_tool_definitions()
        .into_iter()
        .map(
            |definition| TassadarPostArticleRouterPluginToolDefinitionRow {
                tool_name: definition.name,
                description: definition.description,
                parameters_digest: stable_digest(
                    b"tassadar_post_article_router_plugin_tool_definition|",
                    &definition.parameters,
                ),
                stable_kind: definition.kind,
            },
        )
        .collect()
}

fn router_tool_definitions() -> Vec<ToolDefinitionEnvelope> {
    tassadar_starter_plugin_router_tool_definitions()
        .into_iter()
        .map(|definition| ToolDefinitionEnvelope {
            kind: definition.kind,
            function: ToolDefinitionRequest {
                name: definition.name,
                description: Some(definition.description),
                parameters: Some(definition.parameters),
            },
        })
        .collect()
}

fn turn_from_seed_payload(
    payload: &serde_json::Value,
) -> Result<
    (Option<String>, Vec<ResolvedToolCall>),
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let content = payload
        .get("output_text")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(String::from);
    Ok((content, resolved_tool_calls_from_payload(payload)?))
}

fn resolved_tool_calls_from_payload(
    payload: &serde_json::Value,
) -> Result<Vec<ResolvedToolCall>, TassadarPostArticleRouterPluginToolLoopPilotError> {
    let tool_calls = payload
        .get("psionic_tool_calls")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(String::from(
                "missing `psionic_tool_calls` array",
            ))
        })?;
    tool_calls
        .iter()
        .map(|tool_call| {
            Ok(ResolvedToolCall {
                id: required_string(tool_call, &["id"])?,
                name: required_string(tool_call, &["name"])?,
                arguments: tool_call.get("arguments").cloned().ok_or_else(|| {
                    TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(String::from(
                        "missing tool-call arguments",
                    ))
                })?,
            })
        })
        .collect()
}

fn seed_row_from_payload(
    payload: &serde_json::Value,
    headers: &BTreeMap<String, String>,
) -> Result<
    TassadarPostArticleServedSeedResponseRow,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    Ok(TassadarPostArticleServedSeedResponseRow {
        response_id: required_string(payload, &["id"])?,
        conversation_id: required_string(payload, &["conversation", "id"])?,
        conversation_revision: required_u64(payload, &["conversation", "revision"])?,
        tool_call_names: required_tool_call_names(payload)?,
        route_worker: required_header(headers, "x-psionic-route-worker")?,
        route_strategy: required_header(headers, "x-psionic-route-strategy")?,
        stored: required_bool(payload, &["psionic_response_state", "stored"])?,
        replayed_prompt_messages: required_u64(
            payload,
            &["psionic_response_state", "replayed_prompt_messages"],
        )?,
    })
}

fn receipt_rows_from_outcome(
    outcome: &ToolLoopOutcome,
) -> Result<
    Vec<TassadarPostArticleRouterPluginReceiptRow>,
    TassadarPostArticleRouterPluginToolLoopPilotError,
> {
    let mut rows = Vec::new();
    for step in &outcome.steps {
        for tool_result in &step.tool_results {
            let structured = tool_result.structured.clone().ok_or_else(|| {
                TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(format!(
                    "tool result `{}` is missing the structured plugin envelope",
                    tool_result.tool_name
                ))
            })?;
            let envelope: StarterPluginProjectedToolResultEnvelope =
                serde_json::from_value(structured)?;
            rows.push(TassadarPostArticleRouterPluginReceiptRow {
                step_index: step.step_index,
                tool_call_id: tool_result.tool_call_id.clone(),
                tool_name: tool_result.tool_name.clone(),
                plugin_id: envelope.plugin_id,
                plugin_receipt_id: envelope.plugin_receipt.receipt_id,
                plugin_receipt_digest: envelope.plugin_receipt.receipt_digest,
                status: serde_status(&envelope.status),
                output_or_refusal_schema_id: envelope.output_or_refusal_schema_id,
                replay_class_id: envelope.replay_class_id,
            });
        }
    }
    Ok(rows)
}

fn stabilize_tool_loop_outcome(outcome: &mut ToolLoopOutcome) {
    for step in &mut outcome.steps {
        let stable_model_key = format!("{}@synthetic_local", step.route_selection.canonical_name);
        step.route_selection.model_key = stable_model_key.clone();
        for note in &mut step.route_selection.routing_notes {
            if note.starts_with("resolved target `default:") {
                *note = format!(
                    "resolved target `default:{}` to model `{}` on worker `{}`",
                    stable_model_key,
                    step.route_selection.canonical_name,
                    step.route_selection.worker_id,
                );
            }
        }
    }
}

fn scripted_tool_turn(
    content: &str,
    tool_call_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> (Option<String>, Vec<ResolvedToolCall>) {
    (
        Some(String::from(content)),
        vec![ResolvedToolCall {
            id: String::from(tool_call_id),
            name: String::from(tool_name),
            arguments,
        }],
    )
}

fn scripted_final_turn(content: &str) -> (Option<String>, Vec<ResolvedToolCall>) {
    (Some(String::from(content)), Vec::new())
}

fn required_header(
    headers: &BTreeMap<String, String>,
    key: &str,
) -> Result<String, TassadarPostArticleRouterPluginToolLoopPilotError> {
    headers.get(key).cloned().ok_or_else(|| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(format!(
            "missing response header `{key}`",
        ))
    })
}

fn required_tool_call_names(
    payload: &serde_json::Value,
) -> Result<Vec<String>, TassadarPostArticleRouterPluginToolLoopPilotError> {
    let tool_calls = payload
        .get("psionic_tool_calls")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(String::from(
                "missing `psionic_tool_calls` array",
            ))
        })?;
    tool_calls
        .iter()
        .map(|tool_call| required_string(tool_call, &["name"]))
        .collect()
}

fn required_string(
    value: &serde_json::Value,
    path: &[&str],
) -> Result<String, TassadarPostArticleRouterPluginToolLoopPilotError> {
    value
        .pointer(&pointer(path))
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .ok_or_else(|| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(format!(
                "missing string at `{}`",
                pointer(path)
            ))
        })
}

fn required_u64(
    value: &serde_json::Value,
    path: &[&str],
) -> Result<u64, TassadarPostArticleRouterPluginToolLoopPilotError> {
    value
        .pointer(&pointer(path))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(format!(
                "missing integer at `{}`",
                pointer(path)
            ))
        })
}

fn required_bool(
    value: &serde_json::Value,
    path: &[&str],
) -> Result<bool, TassadarPostArticleRouterPluginToolLoopPilotError> {
    value
        .pointer(&pointer(path))
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(format!(
                "missing bool at `{}`",
                pointer(path)
            ))
        })
}

fn pointer(path: &[&str]) -> String {
    let mut pointer = String::new();
    for segment in path {
        pointer.push('/');
        pointer.push_str(segment);
    }
    pointer
}

fn response_header_map(response: &Response) -> BTreeMap<String, String> {
    response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect()
}

async fn response_json(
    response: Response,
) -> Result<serde_json::Value, TassadarPostArticleRouterPluginToolLoopPilotError> {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| {
            TassadarPostArticleRouterPluginToolLoopPilotError::Request(error.to_string())
        })?;
    serde_json::from_slice(body.as_ref()).map_err(Into::into)
}

fn server_error(
    error: OpenAiCompatServerError,
) -> TassadarPostArticleRouterPluginToolLoopPilotError {
    TassadarPostArticleRouterPluginToolLoopPilotError::Server(error.to_string())
}

fn serde_status(status: &psionic_runtime::StarterPluginInvocationStatus) -> String {
    match status {
        psionic_runtime::StarterPluginInvocationStatus::Success => String::from("success"),
        psionic_runtime::StarterPluginInvocationStatus::Refusal => String::from("refusal"),
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value)
        .unwrap_or_else(|error| format!("serialization_error:{error}").into_bytes());
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    format!("{:x}", hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-serve crate dir")
}

#[derive(Clone, Debug)]
struct ScriptedServeToolLoopRunner {
    turns: Vec<(Option<String>, Vec<ResolvedToolCall>)>,
    observed_history_lens: Vec<usize>,
}

impl ToolLoopModelRunner for ScriptedServeToolLoopRunner {
    fn run_turn(
        &mut self,
        request: psionic_router::ToolLoopTurnRequest,
    ) -> Result<ToolLoopModelTurn, ToolLoopError> {
        self.observed_history_lens
            .push(request.prompt_history.len());
        let (content, tool_calls) = self
            .turns
            .get(request.step_index)
            .cloned()
            .ok_or_else(|| ToolLoopError::Execution(String::from("missing scripted turn")))?;
        Ok(ToolLoopModelTurn {
            assistant_message: assistant_prompt_message_for_tool_loop(content),
            tool_calls: tool_calls
                .into_iter()
                .map(tool_loop_tool_call_from_resolved)
                .collect(),
        })
    }
}

fn write_router_plugin_tool_call_gguf(
    path: &Path,
    model_name: &str,
    directive: &str,
) -> Result<(), TassadarPostArticleRouterPluginToolLoopPilotError> {
    let tool_call_json = serde_json::json!({
        "kind": "tool:plugin_text_url_extract",
        "text": directive,
    })
    .to_string();
    let metadata = starter_plugin_tool_call_llama_metadata(model_name, &tool_call_json);
    let tensors = dense_decoder_tensors_with_vocab(6, 3, 4);
    fs::write(path, build_test_gguf(&metadata, &tensors)?).map_err(|error| {
        TassadarPostArticleRouterPluginToolLoopPilotError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

#[derive(Clone, Debug)]
struct TestGgufTensor {
    name: String,
    shape: Vec<usize>,
    tensor_type: GgufTensorType,
    bytes: Vec<u8>,
}

impl TestGgufTensor {
    fn new(
        name: impl Into<String>,
        shape: Vec<usize>,
        tensor_type: GgufTensorType,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            name: name.into(),
            shape,
            tensor_type,
            bytes,
        }
    }
}

fn starter_plugin_tool_call_llama_metadata(
    name: &str,
    tool_call_json: &str,
) -> Vec<(String, GgufMetadataValue)> {
    let mut metadata = dense_family_header("llama", name);
    set_context_length(&mut metadata, "llama", 256);
    metadata.extend(sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
        String::from("<unk>"),
        String::from("<s>"),
        String::from("</s>"),
        String::from("hello"),
        String::from(tool_call_json),
        String::from("done"),
    ]));
    metadata
}

fn set_context_length(
    metadata: &mut [(String, GgufMetadataValue)],
    architecture: &str,
    context_length: u32,
) {
    let key = format!("{architecture}.context_length");
    if let Some((_, value)) = metadata.iter_mut().find(|(candidate, _)| candidate == &key) {
        *value = GgufMetadataValue::U32(context_length);
    }
}

fn dense_family_header(architecture: &str, name: &str) -> Vec<(String, GgufMetadataValue)> {
    vec![
        (
            String::from("general.architecture"),
            GgufMetadataValue::String(architecture.to_string()),
        ),
        (
            String::from("general.name"),
            GgufMetadataValue::String(name.to_string()),
        ),
        (
            format!("{architecture}.context_length"),
            GgufMetadataValue::U32(32),
        ),
        (
            format!("{architecture}.embedding_length"),
            GgufMetadataValue::U32(4),
        ),
        (
            format!("{architecture}.feed_forward_length"),
            GgufMetadataValue::U32(8),
        ),
        (
            format!("{architecture}.block_count"),
            GgufMetadataValue::U32(1),
        ),
        (
            format!("{architecture}.attention.head_count"),
            GgufMetadataValue::U32(2),
        ),
        (
            format!("{architecture}.attention.head_count_kv"),
            GgufMetadataValue::U32(1),
        ),
        (
            format!("{architecture}.attention.layer_norm_rms_epsilon"),
            GgufMetadataValue::F32(1e-5),
        ),
        (
            format!("{architecture}.rope.freq_base"),
            GgufMetadataValue::F32(10_000.0),
        ),
    ]
}

fn sentencepiece_tokenizer_metadata_entries_with_tokens(
    tokens: Vec<String>,
) -> Vec<(String, GgufMetadataValue)> {
    vec![
        (
            String::from("tokenizer.ggml.model"),
            GgufMetadataValue::String(String::from("llama")),
        ),
        (
            String::from("tokenizer.ggml.tokens"),
            GgufMetadataValue::Array(tokens.into_iter().map(GgufMetadataValue::String).collect()),
        ),
        (
            String::from("tokenizer.ggml.bos_token_id"),
            GgufMetadataValue::U32(1),
        ),
        (
            String::from("tokenizer.ggml.eos_token_id"),
            GgufMetadataValue::U32(2),
        ),
        (
            String::from("tokenizer.ggml.unknown_token_id"),
            GgufMetadataValue::U32(0),
        ),
        (
            String::from("tokenizer.ggml.add_bos_token"),
            GgufMetadataValue::Bool(false),
        ),
        (
            String::from("tokenizer.ggml.add_eos_token"),
            GgufMetadataValue::Bool(false),
        ),
    ]
}

fn dense_decoder_tensors_with_vocab(
    vocab_size: usize,
    hello_token_index: usize,
    output_token_index: usize,
) -> Vec<TestGgufTensor> {
    vec![
        dense_tensor(
            "token_embd.weight",
            vec![vocab_size, 4],
            token_embedding_values(vocab_size, hello_token_index),
        ),
        dense_tensor("output_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
        dense_tensor(
            "output.weight",
            vec![vocab_size, 4],
            output_values(vocab_size, output_token_index),
        ),
        dense_tensor("blk.0.attn_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
        dense_tensor("blk.0.attn_q.weight", vec![4, 4], vec![0.0; 16]),
        dense_tensor("blk.0.attn_k.weight", vec![2, 4], vec![0.0; 8]),
        dense_tensor("blk.0.attn_v.weight", vec![2, 4], vec![0.0; 8]),
        dense_tensor("blk.0.attn_output.weight", vec![4, 4], vec![0.0; 16]),
        dense_tensor("blk.0.ffn_gate.weight", vec![8, 4], vec![0.0; 32]),
        dense_tensor("blk.0.ffn_down.weight", vec![4, 8], vec![0.0; 32]),
        dense_tensor("blk.0.ffn_up.weight", vec![8, 4], vec![0.0; 32]),
        dense_tensor("blk.0.ffn_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
    ]
}

fn token_embedding_values(vocab_size: usize, hello_token_index: usize) -> Vec<f32> {
    let mut values = vec![0.0; vocab_size * 4];
    values[hello_token_index.saturating_mul(4)] = 2.0;
    values
}

fn output_values(vocab_size: usize, output_token_index: usize) -> Vec<f32> {
    let mut values = vec![0.0; vocab_size * 4];
    values[output_token_index.saturating_mul(4)] = 1.0;
    values
}

fn dense_tensor(name: &str, shape: Vec<usize>, values: Vec<f32>) -> TestGgufTensor {
    TestGgufTensor::new(
        name,
        shape,
        GgufTensorType::F32,
        encode_f32_bytes(values.as_slice()),
    )
}

fn build_test_gguf(
    metadata: &[(String, GgufMetadataValue)],
    tensors: &[TestGgufTensor],
) -> Result<Vec<u8>, TassadarPostArticleRouterPluginToolLoopPilotError> {
    let alignment = metadata
        .iter()
        .find(|(key, _)| key == "general.alignment")
        .and_then(|(_, value)| match value {
            GgufMetadataValue::U64(value) => Some(*value as usize),
            GgufMetadataValue::U32(value) => Some(*value as usize),
            _ => None,
        })
        .unwrap_or(32)
        .max(1);

    let mut bytes = Vec::new();
    bytes.extend(b"GGUF");
    push_u32(&mut bytes, 3);
    push_u64(
        &mut bytes,
        u64::try_from(tensors.len()).map_err(into_bundle_error)?,
    );
    push_u64(
        &mut bytes,
        u64::try_from(metadata.len()).map_err(into_bundle_error)?,
    );

    for (key, value) in metadata {
        push_gguf_string(&mut bytes, key)?;
        push_u32(&mut bytes, gguf_metadata_value_type(value));
        push_gguf_value(&mut bytes, value)?;
    }

    let mut next_offset = 0usize;
    let mut tensor_offsets = Vec::with_capacity(tensors.len());
    for tensor in tensors {
        tensor_offsets.push(next_offset);
        next_offset = align_usize(next_offset + tensor.bytes.len(), alignment);
    }

    for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
        push_gguf_string(&mut bytes, tensor.name.as_str())?;
        push_u32(
            &mut bytes,
            u32::try_from(tensor.shape.len()).map_err(into_bundle_error)?,
        );
        for dimension in tensor.shape.iter().rev() {
            push_u64(
                &mut bytes,
                u64::try_from(*dimension).map_err(into_bundle_error)?,
            );
        }
        push_u32(&mut bytes, gguf_tensor_type_code(tensor.tensor_type));
        push_u64(
            &mut bytes,
            u64::try_from(*offset).map_err(into_bundle_error)?,
        );
    }

    let tensor_data_offset = align_usize(bytes.len(), alignment);
    bytes.resize(tensor_data_offset, 0);

    for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
        let start = tensor_data_offset + offset;
        if bytes.len() < start {
            bytes.resize(start, 0);
        }
        bytes.extend_from_slice(tensor.bytes.as_slice());
        bytes.resize(align_usize(bytes.len(), alignment), 0);
    }

    Ok(bytes)
}

fn align_usize(value: usize, alignment: usize) -> usize {
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + alignment - remainder
    }
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>()
}

fn gguf_metadata_value_type(value: &GgufMetadataValue) -> u32 {
    match value {
        GgufMetadataValue::U8(_) => 0,
        GgufMetadataValue::I8(_) => 1,
        GgufMetadataValue::U16(_) => 2,
        GgufMetadataValue::I16(_) => 3,
        GgufMetadataValue::U32(_) => 4,
        GgufMetadataValue::I32(_) => 5,
        GgufMetadataValue::F32(_) => 6,
        GgufMetadataValue::Bool(_) => 7,
        GgufMetadataValue::String(_) => 8,
        GgufMetadataValue::Array(_) => 9,
        GgufMetadataValue::U64(_) => 10,
        GgufMetadataValue::I64(_) => 11,
        GgufMetadataValue::F64(_) => 12,
    }
}

fn gguf_tensor_type_code(tensor_type: GgufTensorType) -> u32 {
    match tensor_type {
        GgufTensorType::F32 => 0,
        other => panic!("unsupported synthetic gguf tensor type: {other:?}"),
    }
}

fn push_gguf_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), TassadarPostArticleRouterPluginToolLoopPilotError> {
    push_u64(
        bytes,
        u64::try_from(value.len()).map_err(into_bundle_error)?,
    );
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_gguf_value(
    bytes: &mut Vec<u8>,
    value: &GgufMetadataValue,
) -> Result<(), TassadarPostArticleRouterPluginToolLoopPilotError> {
    match value {
        GgufMetadataValue::U8(value) => bytes.push(*value),
        GgufMetadataValue::I8(value) => bytes.push(value.to_le_bytes()[0]),
        GgufMetadataValue::U16(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::I16(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::U32(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::I32(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::U64(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::I64(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::F32(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::F64(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::Bool(value) => bytes.push(u8::from(*value)),
        GgufMetadataValue::String(value) => push_gguf_string(bytes, value)?,
        GgufMetadataValue::Array(values) => {
            let value_type = values.first().map_or(4, gguf_metadata_value_type);
            push_u32(bytes, value_type);
            push_u64(
                bytes,
                u64::try_from(values.len()).map_err(into_bundle_error)?,
            );
            for value in values {
                push_gguf_value(bytes, value)?;
            }
        }
    }
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

fn into_bundle_error(
    error: impl std::fmt::Display,
) -> TassadarPostArticleRouterPluginToolLoopPilotError {
    TassadarPostArticleRouterPluginToolLoopPilotError::Bundle(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        SUCCESS_CASE_ID, TASSADAR_POST_ARTICLE_ROUTER_PLUGIN_TOOL_LOOP_PILOT_BUNDLE_REF,
        build_tassadar_post_article_router_plugin_tool_loop_pilot_bundle,
        load_tassadar_post_article_router_plugin_tool_loop_pilot_bundle,
        tassadar_post_article_router_plugin_tool_loop_pilot_bundle_path,
        write_tassadar_post_article_router_plugin_tool_loop_pilot_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn router_plugin_tool_loop_pilot_bundle_covers_success_refusal_and_continuation()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_post_article_router_plugin_tool_loop_pilot_bundle()?;

        assert_eq!(bundle.tool_definition_rows.len(), 4);
        assert_eq!(bundle.case_rows.len(), 2);
        assert_eq!(bundle.case_rows[0].case_id, SUCCESS_CASE_ID);
        assert!(bundle.case_rows[0].bounded_step_count_preserved);
        assert!(bundle.case_rows[0].structured_plugin_outputs_preserved);
        assert_eq!(bundle.case_rows[0].tool_loop_outcome.steps.len(), 6);
        assert!(
            bundle
                .case_rows
                .iter()
                .any(|row| row.typed_refusal_preserved)
        );
        assert_eq!(bundle.continuation_row.conversation_revision, 2);
        assert!(bundle.continuation_row.replayed_prompt_messages > 0);
        Ok(())
    }

    #[test]
    fn router_plugin_tool_loop_pilot_bundle_writes_and_loads()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempdir()?;
        let output_path = tempdir
            .path()
            .join("router_plugin_tool_loop_pilot_bundle.json");
        let written =
            write_tassadar_post_article_router_plugin_tool_loop_pilot_bundle(&output_path)?;
        let loaded = load_tassadar_post_article_router_plugin_tool_loop_pilot_bundle(&output_path)?;

        assert_eq!(written, loaded);
        Ok(())
    }

    #[test]
    fn router_plugin_tool_loop_pilot_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_router_plugin_tool_loop_pilot_bundle_path();
        assert!(path.ends_with(TASSADAR_POST_ARTICLE_ROUTER_PLUGIN_TOOL_LOOP_PILOT_BUNDLE_REF));
    }
}
