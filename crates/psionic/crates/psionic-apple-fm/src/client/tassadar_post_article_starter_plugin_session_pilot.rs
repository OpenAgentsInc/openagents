use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    FetchTextConfig, FetchTextSnapshotResponse, FetchTextSnapshotResult,
    StarterPluginInvocationStatus, StarterPluginProjectedToolResultEnvelope,
    StarterPluginToolBridgeConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{AppleFmToolCallbackRuntime, dispatch_tool_call};
use crate::{
    APPLE_FM_TRANSCRIPT_TYPE, APPLE_FM_TRANSCRIPT_VERSION, AppleFmGeneratedContent,
    AppleFmToolCallRequest, AppleFmTranscript, AppleFmTranscriptContent, AppleFmTranscriptEntry,
    AppleFmTranscriptPayload, DEFAULT_APPLE_FM_MODEL_ID, TassadarStarterPluginAppleFmToolError,
    tassadar_starter_plugin_apple_fm_tool_definitions, tassadar_starter_plugin_apple_fm_tools,
};

pub const TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_post_article_apple_fm_plugin_session_pilot_v1/tassadar_post_article_apple_fm_plugin_session_pilot_bundle.json";
pub const TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_post_article_apple_fm_plugin_session_pilot_v1";

const SUCCESS_CASE_ID: &str = "apple_fm_plugin_session_success";
const REFUSAL_CASE_ID: &str = "apple_fm_plugin_session_fetch_refusal";
const SUCCESS_SESSION_ID: &str = "apple-fm-local-pilot-success";
const REFUSAL_SESSION_ID: &str = "apple-fm-local-pilot-refusal";
const SUCCESS_DIRECTIVE: &str =
    "Read https://snapshot.example/article and https://snapshot.example/feed.rss.";
const REFUSAL_DIRECTIVE: &str = "Read https://snapshot.example/binary.";
const SUCCESS_ARTICLE_HTML: &str = r#"<html lang="en"><head><title>Snapshot Article</title></head><body><main><p>Bounded starter plugin content.</p></main></body></html>"#;
const SUCCESS_FEED_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?><rss version="2.0"><channel><title>Snapshot Feed</title><link>https://snapshot.example/</link><description>Snapshot feed updates.</description><item><title>Feed Entry</title><link>/posts/feed-entry</link><description>Snapshot entry summary.</description></item></channel></rss>"#;
const SUCCESS_FINAL_MESSAGE: &str = "The local Apple FM pilot extracted one article and one feed through the shared starter-plugin runtime.";
const REFUSAL_FINAL_MESSAGE: &str = "The local Apple FM pilot stopped after a typed fetch refusal.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAppleFmToolDefinitionRow {
    pub tool_name: String,
    pub description: Option<String>,
    pub arguments_schema_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPostArticleAppleFmSessionStepRow {
    pub step_index: usize,
    pub tool_name: String,
    pub arguments: Value,
    pub projected_result: StarterPluginProjectedToolResultEnvelope,
    pub transcript_call_entry_id: String,
    pub transcript_tool_entry_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPostArticleAppleFmSessionCaseRow {
    pub case_id: String,
    pub session_id: String,
    pub model_id: String,
    pub directive: String,
    pub transcript: AppleFmTranscript,
    pub step_rows: Vec<TassadarPostArticleAppleFmSessionStepRow>,
    pub session_token_binding_preserved: bool,
    pub typed_refusal_preserved: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPostArticleAppleFmPluginSessionPilotBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub tool_definition_rows: Vec<TassadarPostArticleAppleFmToolDefinitionRow>,
    pub case_rows: Vec<TassadarPostArticleAppleFmSessionCaseRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleAppleFmPluginSessionPilotError {
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
    Tool(#[from] TassadarStarterPluginAppleFmToolError),
    #[error("apple fm tool runtime failed: {0}")]
    ToolRuntime(String),
    #[error("apple fm dispatch failed for `{tool_name}`: {error}")]
    Dispatch { tool_name: String, error: String },
    #[error("pilot transcript error: {0}")]
    Transcript(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn tassadar_post_article_apple_fm_plugin_session_pilot_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_BUNDLE_REF)
}

pub fn build_tassadar_post_article_apple_fm_plugin_session_pilot_bundle() -> Result<
    TassadarPostArticleAppleFmPluginSessionPilotBundle,
    TassadarPostArticleAppleFmPluginSessionPilotError,
> {
    let tool_definition_rows = tool_definition_rows()?;
    let success_case = build_case(
        SUCCESS_CASE_ID,
        SUCCESS_SESSION_ID,
        SUCCESS_DIRECTIVE,
        StarterPluginToolBridgeConfig::default(),
        &[
            scripted_step(
                "plugin_text_url_extract",
                json!({"text": SUCCESS_DIRECTIVE}),
                "Calling the starter URL extractor from the Apple FM local lane.",
            ),
            scripted_step(
                "plugin_http_fetch_text",
                json!({"url": "https://snapshot.example/article"}),
                "Fetching the article HTML through the same plugin runtime.",
            ),
            scripted_step(
                "plugin_html_extract_readable",
                json!({
                    "source_url": "https://snapshot.example/article",
                    "content_type": "text/html",
                    "body_text": SUCCESS_ARTICLE_HTML
                }),
                "Extracting readable article content.",
            ),
            scripted_step(
                "plugin_http_fetch_text",
                json!({"url": "https://snapshot.example/feed.rss"}),
                "Fetching the feed document.",
            ),
            scripted_step(
                "plugin_feed_rss_atom_parse",
                json!({
                    "source_url": "https://snapshot.example/feed.rss",
                    "content_type": "application/rss+xml",
                    "feed_text": SUCCESS_FEED_XML
                }),
                "Parsing the feed through the shared starter-plugin bridge.",
            ),
        ],
        SUCCESS_FINAL_MESSAGE,
    )?;
    let refusal_case = build_case(
        REFUSAL_CASE_ID,
        REFUSAL_SESSION_ID,
        REFUSAL_DIRECTIVE,
        StarterPluginToolBridgeConfig {
            fetch_text: FetchTextConfig::snapshot(BTreeMap::from([(
                String::from("https://snapshot.example/binary"),
                FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                    final_url: String::from("https://snapshot.example/binary"),
                    status_code: 200,
                    content_type: String::from("image/png"),
                    charset: None,
                    body_bytes: vec![0x89, b'P', b'N', b'G'],
                }),
            )])),
            ..StarterPluginToolBridgeConfig::default()
        },
        &[
            scripted_step(
                "plugin_text_url_extract",
                json!({"text": REFUSAL_DIRECTIVE}),
                "Calling the starter URL extractor from the Apple FM local lane.",
            ),
            scripted_step(
                "plugin_http_fetch_text",
                json!({"url": "https://snapshot.example/binary"}),
                "Fetching the extracted binary URL.",
            ),
        ],
        REFUSAL_FINAL_MESSAGE,
    )?;

    let mut bundle = TassadarPostArticleAppleFmPluginSessionPilotBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.post_article.apple_fm_plugin_session_pilot.bundle.v1"),
        tool_definition_rows,
        case_rows: vec![success_case, refusal_case],
        claim_boundary: String::from(
            "this bundle freezes one local Apple FM plugin orchestration lane over the shared starter-plugin runtime and transcript envelope. It does not claim served-model closure, cross-platform serving completion, or canonical weighted-plugin control.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "apple fm plugin session pilot covers tool_definitions={} and case_rows={} with transcript truth plus plugin receipt truth.",
        bundle.tool_definition_rows.len(),
        bundle.case_rows.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"tassadar_post_article_apple_fm_plugin_session_pilot|",
        &bundle,
    );
    Ok(bundle)
}

pub fn write_tassadar_post_article_apple_fm_plugin_session_pilot_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleAppleFmPluginSessionPilotBundle,
    TassadarPostArticleAppleFmPluginSessionPilotError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleAppleFmPluginSessionPilotError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_post_article_apple_fm_plugin_session_pilot_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleAppleFmPluginSessionPilotError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

#[cfg(test)]
pub fn load_tassadar_post_article_apple_fm_plugin_session_pilot_bundle(
    path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleAppleFmPluginSessionPilotBundle,
    TassadarPostArticleAppleFmPluginSessionPilotError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleAppleFmPluginSessionPilotError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleAppleFmPluginSessionPilotError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn build_case(
    case_id: &str,
    session_id: &str,
    directive: &str,
    config: StarterPluginToolBridgeConfig,
    steps: &[ScriptedStep],
    final_message: &str,
) -> Result<
    TassadarPostArticleAppleFmSessionCaseRow,
    TassadarPostArticleAppleFmPluginSessionPilotError,
> {
    let runtime = AppleFmToolCallbackRuntime::default();
    let tools = tassadar_starter_plugin_apple_fm_tools(config)?;
    let (_definitions, _callback, session_token) = runtime
        .register_tools(tools)
        .map_err(TassadarPostArticleAppleFmPluginSessionPilotError::ToolRuntime)?;

    let mut transcript_entries = vec![transcript_text_entry(
        format!("{case_id}-user-0"),
        "user",
        directive,
        BTreeMap::new(),
    )];
    let mut step_rows = Vec::with_capacity(steps.len());
    let mut session_tokens = Vec::with_capacity(steps.len());
    for (step_index, step) in steps.iter().enumerate() {
        let call_entry_id = format!("{case_id}-assistant-{}", step_index + 1);
        let tool_entry_id = format!("{case_id}-tool-{}", step_index + 1);
        let mut assistant_extra = BTreeMap::new();
        let _ = assistant_extra.insert(
            String::from("toolCalls"),
            json!([{
                "name": step.tool_name,
                "arguments": step.arguments
            }]),
        );
        transcript_entries.push(transcript_text_entry(
            call_entry_id.clone(),
            "assistant",
            step.narration.as_str(),
            assistant_extra,
        ));
        session_tokens.push(session_token.clone());
        let output = dispatch_tool_call(
            &runtime.state,
            AppleFmToolCallRequest {
                session_token: session_token.clone(),
                tool_name: step.tool_name.clone(),
                arguments: AppleFmGeneratedContent::new(step.arguments.clone()),
            },
        )
        .map_err(
            |error| TassadarPostArticleAppleFmPluginSessionPilotError::Dispatch {
                tool_name: step.tool_name.clone(),
                error: error.underlying_error,
            },
        )?;
        let projected_result: StarterPluginProjectedToolResultEnvelope =
            serde_json::from_str(output.as_str())?;
        let mut tool_extra = BTreeMap::new();
        let _ = tool_extra.insert(
            String::from("toolName"),
            Value::String(step.tool_name.clone()),
        );
        let _ = tool_extra.insert(
            String::from("pluginReceiptId"),
            Value::String(projected_result.plugin_receipt.receipt_id.clone()),
        );
        transcript_entries.push(transcript_text_entry(
            tool_entry_id.clone(),
            "tool",
            output.as_str(),
            tool_extra,
        ));
        step_rows.push(TassadarPostArticleAppleFmSessionStepRow {
            step_index,
            tool_name: step.tool_name.clone(),
            arguments: step.arguments.clone(),
            projected_result,
            transcript_call_entry_id: call_entry_id,
            transcript_tool_entry_id: tool_entry_id,
        });
    }
    transcript_entries.push(transcript_text_entry(
        format!("{case_id}-assistant-final"),
        "assistant",
        final_message,
        BTreeMap::new(),
    ));
    runtime.remove_session_token(session_token.as_str());

    let transcript = AppleFmTranscript {
        version: APPLE_FM_TRANSCRIPT_VERSION,
        transcript_type: APPLE_FM_TRANSCRIPT_TYPE.to_string(),
        transcript: AppleFmTranscriptPayload {
            entries: transcript_entries,
        },
    };
    transcript.validate().map_err(|error| {
        TassadarPostArticleAppleFmPluginSessionPilotError::Transcript(error.to_string())
    })?;
    Ok(TassadarPostArticleAppleFmSessionCaseRow {
        case_id: String::from(case_id),
        session_id: String::from(session_id),
        model_id: String::from(DEFAULT_APPLE_FM_MODEL_ID),
        directive: String::from(directive),
        transcript,
        session_token_binding_preserved: session_tokens
            .first()
            .is_some_and(|first| session_tokens.iter().all(|value| value == first)),
        typed_refusal_preserved: step_rows
            .iter()
            .any(|row| row.projected_result.status == StarterPluginInvocationStatus::Refusal),
        step_rows,
        detail: String::from(
            "the local Apple FM pilot keeps one session-local callback registry, one transcript envelope, and one shared starter-plugin runtime surface explicit instead of inventing Apple-only plugin semantics.",
        ),
    })
}

fn tool_definition_rows() -> Result<
    Vec<TassadarPostArticleAppleFmToolDefinitionRow>,
    TassadarPostArticleAppleFmPluginSessionPilotError,
> {
    Ok(tassadar_starter_plugin_apple_fm_tool_definitions()?
        .into_iter()
        .map(|definition| TassadarPostArticleAppleFmToolDefinitionRow {
            arguments_schema_digest: stable_digest(
                b"tassadar_post_article_apple_fm_tool_definition|",
                definition.arguments_schema.as_json_value(),
            ),
            tool_name: definition.name,
            description: definition.description,
        })
        .collect())
}

fn transcript_text_entry(
    id: String,
    role: &str,
    text: &str,
    extra: BTreeMap<String, Value>,
) -> AppleFmTranscriptEntry {
    AppleFmTranscriptEntry {
        id: Some(id.clone()),
        role: String::from(role),
        contents: vec![AppleFmTranscriptContent {
            content_type: String::from("text"),
            id: Some(format!("{id}-content")),
            extra: BTreeMap::from([(String::from("text"), Value::String(String::from(text)))]),
        }],
        extra,
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
        .expect("repo root should resolve from psionic-apple-fm crate dir")
}

#[derive(Clone, Debug)]
struct ScriptedStep {
    tool_name: String,
    arguments: Value,
    narration: String,
}

fn scripted_step(tool_name: &str, arguments: Value, narration: &str) -> ScriptedStep {
    ScriptedStep {
        tool_name: String::from(tool_name),
        arguments,
        narration: String::from(narration),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SUCCESS_CASE_ID, TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_BUNDLE_REF,
        build_tassadar_post_article_apple_fm_plugin_session_pilot_bundle,
        load_tassadar_post_article_apple_fm_plugin_session_pilot_bundle,
        tassadar_post_article_apple_fm_plugin_session_pilot_bundle_path,
        write_tassadar_post_article_apple_fm_plugin_session_pilot_bundle,
    };

    #[test]
    fn apple_fm_plugin_session_pilot_bundle_covers_success_and_refusal_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_post_article_apple_fm_plugin_session_pilot_bundle()?;

        assert_eq!(bundle.tool_definition_rows.len(), 4);
        assert_eq!(bundle.case_rows.len(), 2);
        assert_eq!(bundle.case_rows[0].case_id, SUCCESS_CASE_ID);
        assert!(bundle.case_rows[0].session_token_binding_preserved);
        assert_eq!(bundle.case_rows[0].step_rows.len(), 5);
        assert!(
            bundle
                .case_rows
                .iter()
                .any(|row| row.typed_refusal_preserved)
        );
        assert!(
            bundle
                .case_rows
                .iter()
                .all(|row| row.transcript.entry_count() >= row.step_rows.len() * 2)
        );
        Ok(())
    }

    #[test]
    fn apple_fm_plugin_session_pilot_bundle_writes_and_loads()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempfile::tempdir()?;
        let output_path = tempdir
            .path()
            .join("apple_fm_plugin_session_pilot_bundle.json");
        let written =
            write_tassadar_post_article_apple_fm_plugin_session_pilot_bundle(&output_path)?;
        let loaded = load_tassadar_post_article_apple_fm_plugin_session_pilot_bundle(&output_path)?;

        assert_eq!(written, loaded);
        Ok(())
    }

    #[test]
    fn apple_fm_plugin_session_pilot_bundle_repo_path_is_under_fixtures() {
        let path = tassadar_post_article_apple_fm_plugin_session_pilot_bundle_path();
        assert!(path.ends_with(TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_BUNDLE_REF));
    }
}
