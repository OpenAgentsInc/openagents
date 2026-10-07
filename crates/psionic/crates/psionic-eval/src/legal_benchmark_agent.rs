//! Legal benchmark agent loop and run-record writer.
//!
//! This module drives provider turns against the closed legal benchmark tool
//! surface, then writes replayable run artifacts for scoring and Autopilot
//! import.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    ArtifactKind, ArtifactManifest, ArtifactManifestError, BenchmarkIntegrityError,
    BenchmarkTaskSpec, DataClassification, LegalBenchmarkAnswerIntegrityReport,
    LegalBenchmarkPathRoot, LegalBenchmarkToolExecution, LegalBenchmarkToolFailureKind,
    LegalBenchmarkToolInput, LegalBenchmarkToolReceipt, LegalBenchmarkToolWorkspace, Metadata,
    ModelAdapter, ModelAdapterError, ModelAdapterFailureKind, ModelMessage, ModelMessageRole,
    ModelProviderRoute, ModelRequest, ModelResponse, ModelStopReason, ModelToolCall, RunConfig,
    RunMetrics, RunRecord, RunTerminalState, SourceArtifact, ToolCallRecord, ToolResultMessage,
    TranscriptEvent, TranscriptEventKind, agent_visible_checklist, artifact_from_file,
    artifact_manifest_digest, build_answer_integrity_report, build_coverage_snapshot,
    build_output_artifact_manifest, execute_legal_benchmark_tool, run_config_digest,
    run_record_digest, stable_json_digest, task_spec_digest, transcript_digest,
};

pub const LEGAL_BENCHMARK_AGENT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug)]
pub struct LegalBenchmarkAgentRunRequest {
    pub task_spec: BenchmarkTaskSpec,
    pub input_artifact_manifest: ArtifactManifest,
    pub run_config: RunConfig,
    pub tool_workspace: LegalBenchmarkToolWorkspace,
    pub run_root: PathBuf,
    pub module_instructions: Vec<String>,
    pub extraction_receipt_refs: Vec<String>,
    pub run_nonce: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkRunArtifactPaths {
    pub run_root: PathBuf,
    pub config_json: PathBuf,
    pub transcript_jsonl: PathBuf,
    pub metrics_json: PathBuf,
    pub output_artifact_manifest_json: PathBuf,
    pub extraction_receipts_json: PathBuf,
    pub tool_receipts_json: PathBuf,
    pub run_record_json: PathBuf,
    pub run_receipt_json: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkRunReceipt {
    pub schema_version: u16,
    pub run_id: String,
    pub task_id: String,
    pub task_version: String,
    pub terminal_state: RunTerminalState,
    pub task_spec_hash: String,
    pub input_artifact_manifest_hash: String,
    pub run_config_hash: String,
    pub output_artifact_manifest_hash: String,
    pub transcript_hash: String,
    pub metrics_hash: String,
    pub tool_receipts_hash: String,
    pub run_record_hash: String,
    pub output_artifact_count: u64,
    pub tool_receipt_count: u64,
    #[serde(default)]
    pub answer_integrity: LegalBenchmarkAnswerIntegrityReport,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkAgentRunResult {
    pub run_id: String,
    pub terminal_state: RunTerminalState,
    pub run_record: RunRecord,
    pub output_artifact_manifest: ArtifactManifest,
    pub run_receipt: LegalBenchmarkRunReceipt,
    pub tool_receipts: Vec<LegalBenchmarkToolReceipt>,
    pub paths: LegalBenchmarkRunArtifactPaths,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSubmission {
    pub action: String,
    pub deliverables: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreSubmitProtocolReport {
    satisfied: bool,
    missing_steps: Vec<String>,
}

#[derive(Debug, Error)]
pub enum LegalBenchmarkAgentRunError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("artifact manifest error: {0}")]
    ArtifactManifest(#[from] ArtifactManifestError),
    #[error("benchmark integrity error: {0}")]
    Integrity(#[from] BenchmarkIntegrityError),
    #[error("provider error: {0:?}")]
    Provider(ModelAdapterError),
    #[error("invalid model tool call {tool_call_id}: {detail}")]
    ToolInput {
        tool_call_id: String,
        detail: String,
    },
}

pub fn run_legal_benchmark_agent<A>(
    request: LegalBenchmarkAgentRunRequest,
    adapter: &mut A,
) -> Result<LegalBenchmarkAgentRunResult, LegalBenchmarkAgentRunError>
where
    A: ModelAdapter,
{
    let started = Instant::now();
    let run_config_hash = run_config_digest(&request.run_config)?;
    let input_manifest_hash = artifact_manifest_digest(&request.input_artifact_manifest)?;
    let task_spec_hash = task_spec_digest(&request.task_spec)?;
    let run_id = legal_benchmark_run_id(
        &request.task_spec,
        &run_config_hash,
        request.run_nonce.as_deref(),
    );
    let paths = run_artifact_paths(&request.run_root);

    let mut transcript = Vec::new();
    let mut tool_calls = Vec::new();
    let mut tool_receipts = Vec::new();
    let mut metrics = RunMetrics {
        model_turns: 0,
        tool_call_count: 0,
        input_tokens: 0,
        output_tokens: 0,
        wall_time_ms: 0,
        estimated_cost_micro_usd: 0,
    };
    let mut messages = vec![
        ModelMessage::new(
            ModelMessageRole::System,
            legal_benchmark_system_prompt(&request),
        ),
        ModelMessage::new(
            ModelMessageRole::User,
            legal_benchmark_user_prompt(&request),
        ),
    ];
    push_transcript_event(
        &mut transcript,
        TranscriptEventKind::System,
        Some("system"),
        messages[0].content.clone(),
        Some(json!({"run_id": run_id, "run_config_hash": run_config_hash})),
    );
    push_transcript_event(
        &mut transcript,
        TranscriptEventKind::User,
        Some("user"),
        messages[1].content.clone(),
        Some(json!({"task_id": request.task_spec.task_id})),
    );

    let mut terminal_state = None;
    for turn_index in 0..request.run_config.tool_policy.max_turns {
        let effective_allowed_tools = effective_allowed_tools_for_turn(&request, &tool_calls);
        let forced_tool_choice =
            forced_tool_choice_for_turn(&request, &tool_calls, &effective_allowed_tools);
        let model_request = model_request_for_tool_policy(
            format!("model_request.{run_id}.{turn_index}"),
            messages.clone(),
            &effective_allowed_tools,
            &request.run_config.metadata,
            forced_tool_choice.as_deref(),
        );
        let response = match adapter.complete(&model_request) {
            Ok(response) => response,
            Err(error) => {
                terminal_state = Some(provider_terminal_state(error.kind));
                push_transcript_event(
                    &mut transcript,
                    TranscriptEventKind::Runner,
                    Some("runner"),
                    Some(String::from("provider failure")),
                    Some(json!({"error": error})),
                );
                break;
            }
        };
        apply_model_metrics(&mut metrics, &response);
        push_model_response_event(&mut transcript, &response);
        messages.push(ModelMessage::assistant_response(
            response.final_text.clone(),
            response.tool_calls.clone(),
        ));
        let plain_text_tool_call = if response.tool_calls.is_empty()
            && plain_text_tool_protocol_enabled(&request.run_config.metadata)
        {
            response
                .final_text
                .as_deref()
                .and_then(parse_plain_text_tool_call)
        } else {
            None
        };

        match response.stop_reason {
            ModelStopReason::MaxTokens if plain_text_tool_call.is_none() => {
                terminal_state = Some(RunTerminalState::MaxTokens);
                break;
            }
            ModelStopReason::SafetyRefusal => {
                terminal_state = Some(RunTerminalState::PolicyFailure);
                break;
            }
            ModelStopReason::ProviderError => {
                terminal_state = Some(RunTerminalState::ProviderFailure);
                break;
            }
            ModelStopReason::MaxTokens | ModelStopReason::Stop | ModelStopReason::ToolCalls => {}
        }

        if response.tool_calls.is_empty() {
            if let Some(tool_call) = plain_text_tool_call {
                push_transcript_event(
                    &mut transcript,
                    TranscriptEventKind::Runner,
                    Some("runner"),
                    Some(String::from("model-authored plain-text tool call accepted")),
                    Some(json!({
                        "tool_call_id": tool_call.tool_call_id.clone(),
                        "tool_name": tool_call.tool_name.clone(),
                    })),
                );
                let execution = execute_model_tool_call(&tool_call, &request.tool_workspace)
                    .map_err(|detail| LegalBenchmarkAgentRunError::ToolInput {
                        tool_call_id: tool_call.tool_call_id.clone(),
                        detail,
                    })?;
                let tool_result_message = append_tool_execution(
                    &mut transcript,
                    &mut tool_calls,
                    &mut tool_receipts,
                    &tool_call,
                    execution,
                )?;
                metrics.tool_call_count = metrics.tool_call_count.saturating_add(1);
                messages.push(ModelMessage::new(
                    ModelMessageRole::User,
                    format!(
                        "Tool result for {}: {}",
                        tool_result_message.tool_name, tool_result_message.content
                    ),
                ));
                continue;
            }
            match response.final_text.as_deref().and_then(parse_submission) {
                Some(submission) if is_submit_action(&submission.action) => {
                    let protocol =
                        pre_submit_protocol_report(&request, &tool_calls, &transcript, &submission);
                    if protocol.satisfied {
                        push_transcript_event(
                            &mut transcript,
                            TranscriptEventKind::Runner,
                            Some("runner"),
                            Some(String::from("submission accepted")),
                            Some(json!({"submission": submission})),
                        );
                        terminal_state = Some(RunTerminalState::Submitted);
                        break;
                    }
                    push_transcript_event(
                        &mut transcript,
                        TranscriptEventKind::Runner,
                        Some("runner"),
                        Some(String::from("pre-submit protocol incomplete")),
                        Some(json!({
                            "submission": submission,
                            "missing_steps": protocol.missing_steps.clone(),
                        })),
                    );
                    if turn_index + 1 >= request.run_config.tool_policy.max_turns {
                        terminal_state = Some(RunTerminalState::PolicyFailure);
                        break;
                    }
                    let feedback = format!(
                        "Pre-submit protocol incomplete. Complete these steps before submitting: {}",
                        protocol.missing_steps.join("; ")
                    );
                    messages.push(ModelMessage::new(ModelMessageRole::User, feedback));
                }
                _ => {
                    terminal_state = Some(RunTerminalState::NoToolCalls);
                    break;
                }
            }
            continue;
        }

        let mut terminal_from_tool = None;
        for tool_call in &response.tool_calls {
            if let Some(submission) = submission_from_submit_tool_call(tool_call, &request) {
                push_transcript_event(
                    &mut transcript,
                    TranscriptEventKind::ToolCall,
                    Some("assistant"),
                    None,
                    Some(json!({
                        "tool_call_id": tool_call.tool_call_id.clone(),
                        "tool_name": tool_call.tool_name.clone(),
                        "input": tool_call.arguments.clone(),
                        "interpreted_as": "terminal_submission",
                    })),
                );
                let protocol =
                    pre_submit_protocol_report(&request, &tool_calls, &transcript, &submission);
                if protocol.satisfied {
                    push_transcript_event(
                        &mut transcript,
                        TranscriptEventKind::Runner,
                        Some("runner"),
                        Some(String::from("submission accepted")),
                        Some(json!({"submission": submission, "source": "submit_tool_call"})),
                    );
                    terminal_from_tool = Some(RunTerminalState::Submitted);
                    break;
                }
                push_transcript_event(
                    &mut transcript,
                    TranscriptEventKind::Runner,
                    Some("runner"),
                    Some(String::from("pre-submit protocol incomplete")),
                    Some(json!({
                        "submission": submission,
                        "missing_steps": protocol.missing_steps.clone(),
                        "source": "submit_tool_call",
                    })),
                );
                if turn_index + 1 >= request.run_config.tool_policy.max_turns {
                    terminal_from_tool = Some(RunTerminalState::PolicyFailure);
                    break;
                }
                let feedback = format!(
                    "Pre-submit protocol incomplete. Complete these steps before submitting: {}",
                    protocol.missing_steps.join("; ")
                );
                messages.push(ModelMessage::new(ModelMessageRole::User, feedback));
                break;
            }
            let execution =
                execute_model_tool_call(tool_call, &request.tool_workspace).map_err(|detail| {
                    LegalBenchmarkAgentRunError::ToolInput {
                        tool_call_id: tool_call.tool_call_id.clone(),
                        detail,
                    }
                })?;
            let tool_result_message = append_tool_execution(
                &mut transcript,
                &mut tool_calls,
                &mut tool_receipts,
                tool_call,
                execution,
            )?;
            metrics.tool_call_count = metrics.tool_call_count.saturating_add(1);
            messages.push(ModelMessage::tool_result(tool_result_message));

            if let Some(receipt) = tool_receipts.last()
                && matches!(
                    receipt.failure_kind,
                    Some(
                        LegalBenchmarkToolFailureKind::SandboxUnavailable
                            | LegalBenchmarkToolFailureKind::SandboxFailed
                    )
                )
            {
                terminal_from_tool = Some(RunTerminalState::SandboxFailure);
                break;
            }
        }
        if terminal_from_tool.is_some() {
            terminal_state = terminal_from_tool;
            break;
        }
    }

    let terminal_state = terminal_state.unwrap_or(RunTerminalState::MaxTurns);
    metrics.wall_time_ms = elapsed_ms(started);
    let output_artifacts = collect_output_artifacts(&request.tool_workspace.output_root)?;
    let output_manifest = build_output_artifact_manifest(
        request.task_spec.task_id.clone(),
        request.task_spec.task_version.clone(),
        run_id.clone(),
        output_artifacts,
    );
    let answer_integrity = build_answer_integrity_report(
        &request.task_spec,
        &output_manifest,
        &request.tool_workspace.output_root,
        &tool_calls,
    )?;
    let output_manifest_hash = artifact_manifest_digest(&output_manifest)?;
    let coverage_snapshot = build_coverage_snapshot(
        &request.task_spec,
        &request.run_config,
        &tool_calls,
        &transcript,
        &output_manifest,
    )?;
    let mut provider_metadata = provider_route_run_metadata(adapter.route())?;
    provider_metadata.insert(
        String::from("answer_integrity"),
        serde_json::to_value(&answer_integrity)?,
    );
    let run_record = RunRecord {
        schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
        run_id: run_id.clone(),
        task_id: request.task_spec.task_id.clone(),
        task_version: request.task_spec.task_version.clone(),
        input_artifact_manifest_hash: input_manifest_hash.clone(),
        run_config_hash: run_config_hash.clone(),
        output_artifact_manifest_hash: output_manifest_hash.clone(),
        terminal_state,
        transcript,
        tool_calls,
        metrics,
        extraction_receipt_refs: request.extraction_receipt_refs.clone(),
        coverage_snapshot: Some(coverage_snapshot),
        metadata: provider_metadata.clone(),
    };
    let run_receipt = LegalBenchmarkRunReceipt {
        schema_version: LEGAL_BENCHMARK_AGENT_SCHEMA_VERSION,
        run_id: run_id.clone(),
        task_id: request.task_spec.task_id.clone(),
        task_version: request.task_spec.task_version.clone(),
        terminal_state,
        task_spec_hash,
        input_artifact_manifest_hash: input_manifest_hash,
        run_config_hash,
        output_artifact_manifest_hash: output_manifest_hash,
        transcript_hash: transcript_digest(&run_record.transcript)?,
        metrics_hash: stable_json_digest(
            "psionic.legal_benchmark.run_metrics.v1",
            &run_record.metrics,
        )?,
        tool_receipts_hash: stable_json_digest(
            "psionic.legal_benchmark.tool_receipts.v1",
            &tool_receipts,
        )?,
        run_record_hash: run_record_digest(&run_record)?,
        output_artifact_count: u64::try_from(output_manifest.artifacts.len()).unwrap_or(u64::MAX),
        tool_receipt_count: u64::try_from(tool_receipts.len()).unwrap_or(u64::MAX),
        answer_integrity,
        created_at_ms: now_ms(),
        metadata: provider_metadata,
    };

    write_run_artifacts(
        &paths,
        &request.run_config,
        &run_record,
        &output_manifest,
        &request.extraction_receipt_refs,
        &tool_receipts,
        &run_receipt,
    )?;

    Ok(LegalBenchmarkAgentRunResult {
        run_id,
        terminal_state,
        run_record,
        output_artifact_manifest: output_manifest,
        run_receipt,
        tool_receipts,
        paths,
    })
}

fn model_request_for_tool_policy(
    request_id: impl Into<String>,
    messages: Vec<ModelMessage>,
    allowed_tools: &[String],
    run_metadata: &Metadata,
    forced_tool_choice: Option<&str>,
) -> ModelRequest {
    let mut request = ModelRequest::new(request_id, messages);
    request
        .tools
        .retain(|tool| allowed_tools.iter().any(|allowed| allowed == &tool.name));
    if let Some(max_output_tokens) = run_metadata
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
    {
        request.sampling.max_output_tokens = max_output_tokens;
    }
    if let Some(tool_name) = forced_tool_choice {
        request.metadata.insert(
            String::from("tool_choice"),
            Value::String(tool_name.to_owned()),
        );
    }
    if plain_text_tool_protocol_enabled(run_metadata) {
        request.tools.clear();
        request
            .metadata
            .insert(String::from("plain_text_tool_protocol"), Value::Bool(true));
    }
    request
}

fn plain_text_tool_protocol_enabled(metadata: &Metadata) -> bool {
    metadata
        .get("plain_text_tool_protocol")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn parse_plain_text_tool_call(text: &str) -> Option<ModelToolCall> {
    let value = parse_json_value_from_text(text)?;
    let tool_name = value
        .get("tool")
        .and_then(Value::as_str)
        .or_else(|| value.get("action").and_then(Value::as_str))?
        .to_owned();
    if is_submit_action(tool_name.as_str()) {
        return None;
    }
    let arguments = normalize_plain_text_tool_arguments(
        tool_name.as_str(),
        value
            .get("input")
            .cloned()
            .or_else(|| value.get("arguments").cloned())?,
    )?;
    let tool_call_hash =
        stable_json_digest("psionic.legal_benchmark.plain_text_tool_call.v1", &value).ok()?;
    Some(ModelToolCall {
        tool_call_id: format!(
            "plain_text_tool.{}",
            tool_call_hash.chars().take(16).collect::<String>()
        ),
        tool_name,
        arguments,
    })
}

fn normalize_plain_text_tool_arguments(tool_name: &str, arguments: Value) -> Option<Value> {
    match tool_name {
        "write" => {
            if arguments.get("root").is_some()
                && arguments.get("relative_path").is_some()
                && arguments.get("content").is_some()
            {
                return Some(arguments);
            }
            let path = arguments
                .get("relative_path")
                .or_else(|| arguments.get("path"))?
                .as_str()?;
            let content = arguments.get("content")?.as_str()?;
            Some(json!({
                "root": arguments
                    .get("root")
                    .and_then(Value::as_str)
                    .unwrap_or("output"),
                "relative_path": normalize_output_path(path),
                "content": content,
                "overwrite": arguments
                    .get("overwrite")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            }))
        }
        "validate_deliverables" => {
            if arguments.get("root").is_some() && arguments.get("required_paths").is_some() {
                return Some(arguments);
            }
            let path = arguments
                .get("path")
                .or_else(|| arguments.get("relative_path"))?
                .as_str()?;
            Some(json!({
                "root": arguments
                    .get("root")
                    .and_then(Value::as_str)
                    .unwrap_or("output"),
                "required_paths": [normalize_output_path(path)],
                "max_results": arguments
                    .get("max_results")
                    .and_then(Value::as_u64)
                    .unwrap_or(5),
            }))
        }
        _ => Some(arguments),
    }
}

fn parse_json_value_from_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim);
    if let Some(candidate) = without_fence
        && let Ok(value) = serde_json::from_str::<Value>(candidate)
    {
        return Some(value);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&trimmed[start..=end]).ok()
}

fn effective_allowed_tools_for_turn(
    request: &LegalBenchmarkAgentRunRequest,
    tool_calls: &[ToolCallRecord],
) -> Vec<String> {
    let mut allowed_tools = request.run_config.tool_policy.allowed_tools.clone();
    if allowed_tools.is_empty()
        || allowed_tools
            .iter()
            .any(|tool| tool == "validate_deliverables")
        || !request.task_spec.source_artifacts.is_empty()
    {
        return allowed_tools;
    }
    if required_deliverables_written(request, tool_calls) {
        allowed_tools.clear();
    }
    allowed_tools
}

fn required_deliverables_written(
    request: &LegalBenchmarkAgentRunRequest,
    tool_calls: &[ToolCallRecord],
) -> bool {
    request
        .task_spec
        .deliverables
        .iter()
        .filter(|deliverable| deliverable.required)
        .all(|deliverable| {
            tool_calls.iter().any(|call| {
                call.tool_name == "write"
                    && call.error_kind.is_none()
                    && input_relative_path(call).is_some_and(|path| {
                        output_path_matches(deliverable.required_path.as_str(), path)
                    })
            })
        })
}

fn required_deliverables_validated(
    request: &LegalBenchmarkAgentRunRequest,
    tool_calls: &[ToolCallRecord],
) -> bool {
    request
        .task_spec
        .deliverables
        .iter()
        .filter(|deliverable| deliverable.required)
        .all(|deliverable| deliverable_validation_passed(tool_calls, &deliverable.required_path))
}

fn forced_tool_choice_for_turn(
    request: &LegalBenchmarkAgentRunRequest,
    tool_calls: &[ToolCallRecord],
    allowed_tools: &[String],
) -> Option<String> {
    let metadata = &request.run_config.metadata;
    let allowed = |tool_name: &str| allowed_tools.iter().any(|tool| tool == tool_name);
    if metadata
        .get("force_write_until_required_deliverables")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && allowed("write")
        && !required_deliverables_written(request, tool_calls)
    {
        return Some(String::from("write"));
    }
    if metadata
        .get("force_validate_after_write")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && allowed("validate_deliverables")
        && required_deliverables_written(request, tool_calls)
        && !required_deliverables_validated(request, tool_calls)
    {
        return Some(String::from("validate_deliverables"));
    }
    None
}

pub fn legal_benchmark_system_prompt(request: &LegalBenchmarkAgentRunRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are running a legal benchmark task under Psionic control.\n");
    prompt.push_str("Use only the listed tools and produce outputs intentionally.\n\n");
    prompt.push_str("Tool policy:\n");
    let allowed_tool_label = if request.run_config.tool_policy.allowed_tools.is_empty() {
        String::from("none")
    } else {
        request.run_config.tool_policy.allowed_tools.join(", ")
    };
    prompt.push_str(&format!("- allowed tools: {}\n", allowed_tool_label));
    prompt.push_str(&format!(
        "- network allowed: {}\n",
        request.run_config.tool_policy.network_allowed
    ));
    prompt.push_str(&format!(
        "- source artifacts read-only: {}\n",
        request.run_config.tool_policy.source_artifacts_read_only
    ));
    prompt.push_str(
        "- submit by returning JSON: {\"action\":\"submit\",\"deliverables\":[\"path\"]}.\n\n",
    );
    if request.run_config.tool_policy.allowed_tools.is_empty() {
        prompt.push_str("Tool descriptions:\n- No model tools are available for this task.\n");
    } else {
        prompt.push_str("Tool descriptions:\n");
        for tool in crate::legal_benchmark_model_tool_specs() {
            if request
                .run_config
                .tool_policy
                .allowed_tools
                .iter()
                .any(|allowed| allowed == &tool.name)
            {
                prompt.push_str(&format!("- {}: {}\n", tool.name, tool.description));
            }
        }
    }
    append_benchmark_operating_protocol(&mut prompt, request);
    if !request.module_instructions.is_empty() {
        prompt.push_str("\nModule instructions:\n");
        for instruction in &request.module_instructions {
            prompt.push_str("- ");
            prompt.push_str(instruction);
            prompt.push('\n');
        }
    }
    prompt
}

fn append_benchmark_operating_protocol(
    prompt: &mut String,
    request: &LegalBenchmarkAgentRunRequest,
) {
    let allows = |tool: &str| {
        request
            .run_config
            .tool_policy
            .allowed_tools
            .iter()
            .any(|allowed| allowed == tool)
    };
    let mut lines = Vec::new();
    if allows("inventory") {
        lines.push(String::from(
            "Start document-heavy tasks with inventory on the documents root.",
        ));
    }
    let inspection_tools = [
        "read",
        "email_summary",
        "spreadsheet_summary",
        "pdf_search",
        "grep",
    ]
    .into_iter()
    .filter(|tool| allows(tool))
    .collect::<Vec<_>>();
    if !inspection_tools.is_empty() {
        lines.push(format!(
            "Inspect source content with {} before drafting; use targeted searches for quoted facts.",
            inspection_tools.join(", ")
        ));
    }
    if allows("evidence_table") {
        lines.push(String::from(
            "Capture cited support with evidence_table before relying on a source claim.",
        ));
    }
    if allows("validate_deliverables") {
        lines.push(String::from(
            "Validate required output paths with validate_deliverables before submitting.",
        ));
    }
    if lines.is_empty() {
        return;
    }
    prompt.push_str("\nBenchmark operating protocol:\n");
    for line in lines {
        prompt.push_str("- ");
        prompt.push_str(&line);
        prompt.push('\n');
    }
    prompt.push_str(
        "- Final submission may be rejected until required inventory, source inspection, evidence rows, deliverable validation, and self-check notes are present.\n",
    );
}

pub fn legal_benchmark_user_prompt(request: &LegalBenchmarkAgentRunRequest) -> String {
    let mut prompt = String::new();
    prompt.push_str(&request.task_spec.instructions);
    if !request.task_spec.source_artifacts.is_empty() {
        prompt.push_str("\n\nSource artifact tool paths:\n");
        for artifact in &request.task_spec.source_artifacts {
            prompt.push_str(&format!(
                "- {}: {} ({})\n",
                artifact.artifact_id,
                source_tool_relative_path(artifact),
                artifact.media_type
            ));
        }
    }
    let issue_checklist = legal_issue_checklist(&request.task_spec);
    if !issue_checklist.is_empty() {
        prompt.push_str("\n\nPractice-area issue checklist:\n");
        for item in issue_checklist {
            prompt.push_str("- ");
            prompt.push_str(&item);
            prompt.push('\n');
        }
    }
    prompt.push_str("\n\nDeliverables:\n");
    for deliverable in &request.task_spec.deliverables {
        prompt.push_str(&format!(
            "- {} at {}: {}\n",
            deliverable.deliverable_id, deliverable.required_path, deliverable.description
        ));
    }
    let checklist = agent_visible_checklist(&request.run_config);
    if !checklist.items.is_empty() {
        prompt.push_str("\nApproved checklist:\n");
        for item in checklist.items {
            prompt.push_str(&format!("- {}: {}\n", item.item_id, item.prompt));
        }
    }
    prompt
}

pub fn parse_submission(text: &str) -> Option<LegalBenchmarkSubmission> {
    serde_json::from_str::<LegalBenchmarkSubmission>(text.trim()).ok()
}

fn is_submit_action(action: &str) -> bool {
    matches!(action, "submit" | "finalize")
}

fn submission_from_submit_tool_call(
    tool_call: &ModelToolCall,
    request: &LegalBenchmarkAgentRunRequest,
) -> Option<LegalBenchmarkSubmission> {
    if !is_submit_action(tool_call.tool_name.as_str()) {
        return None;
    }
    let arguments = if tool_call
        .arguments
        .get("deliverables")
        .and_then(Value::as_array)
        .is_some()
    {
        &tool_call.arguments
    } else {
        tool_call
            .arguments
            .get("input")
            .unwrap_or(&tool_call.arguments)
    };
    let deliverables = arguments
        .get("deliverables")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|deliverables| !deliverables.is_empty())
        .unwrap_or_else(|| {
            request
                .task_spec
                .deliverables
                .iter()
                .map(|deliverable| deliverable.required_path.clone())
                .collect()
        });
    let action = tool_call
        .arguments
        .get("action")
        .and_then(Value::as_str)
        .or_else(|| {
            tool_call
                .arguments
                .get("input")
                .and_then(|input| input.get("action"))
                .and_then(Value::as_str)
        })
        .unwrap_or(tool_call.tool_name.as_str())
        .to_owned();
    let note = tool_call
        .arguments
        .get("note")
        .and_then(Value::as_str)
        .or_else(|| {
            tool_call
                .arguments
                .get("input")
                .and_then(|input| input.get("note"))
                .and_then(Value::as_str)
        })
        .map(str::to_owned);
    Some(LegalBenchmarkSubmission {
        action,
        deliverables,
        note,
    })
}

fn legal_issue_checklist(task_spec: &BenchmarkTaskSpec) -> Vec<String> {
    let mut checklist = Vec::new();
    push_unique(
        &mut checklist,
        "Identify parties, dates, governing documents, requested output, and assumptions.",
    );
    push_unique(
        &mut checklist,
        "Separate document-backed facts from legal conclusions and cite each material fact.",
    );
    push_unique(
        &mut checklist,
        "Track ambiguities, missing documents, and conflicts between source materials.",
    );

    let normalized = normalized_task_terms(task_spec);
    if normalized.contains("review")
        || normalized.contains("analyze")
        || normalized.contains("compare")
        || normalized.contains("issue")
    {
        push_unique(
            &mut checklist,
            "Compare draft terms against instructions, term sheets, prior versions, and controlling source documents.",
        );
    }
    if normalized.contains("draft") {
        push_unique(
            &mut checklist,
            "Mirror defined terms, party names, dates, cross-references, exhibits, and governing-law style from source documents.",
        );
    }

    if normalized.contains("emerging")
        || normalized.contains("venture")
        || normalized.contains("startup")
        || normalized.contains("stock")
        || normalized.contains("financing")
    {
        push_unique(
            &mut checklist,
            "Check economics, conversion, liquidation, pro rata rights, voting/protective provisions, and approvals.",
        );
        push_unique(
            &mut checklist,
            "Check registration, information, transfer, ROFR/co-sale, board, stockholder, and securities-law issues.",
        );
    } else if normalized.contains("contract") || normalized.contains("commercial") {
        push_unique(
            &mut checklist,
            "Check obligations, conditions, deadlines, termination, remedies, assignment, confidentiality, warranties, and indemnities.",
        );
        push_unique(
            &mut checklist,
            "Check governing law, dispute resolution, notice mechanics, attachments, and inconsistent defined terms.",
        );
    } else if normalized.contains("employment") || normalized.contains("labor") {
        push_unique(
            &mut checklist,
            "Check classification, compensation, equity, termination, restrictive covenants, wage/hour, leave, and accommodations.",
        );
        push_unique(
            &mut checklist,
            "Check confidentiality, invention assignment, non-solicit, non-compete, and state-law enforceability limits.",
        );
    } else if normalized.contains("privacy")
        || normalized.contains("cyber")
        || normalized.contains("data")
    {
        push_unique(
            &mut checklist,
            "Check data categories, controller/processor roles, consent, retention, security, cross-border transfer, and breach notice.",
        );
        push_unique(
            &mut checklist,
            "Check vendor, subprocessor, regulator, consumer-rights, and data-processing-agreement obligations.",
        );
    } else if normalized.contains("intellectual")
        || normalized.contains("ip")
        || normalized.contains("patent")
        || normalized.contains("copyright")
        || normalized.contains("trademark")
    {
        push_unique(
            &mut checklist,
            "Check ownership, license scope, prosecution, infringement, open-source use, confidentiality, assignments, and royalties.",
        );
    } else if normalized.contains("litigation")
        || normalized.contains("dispute")
        || normalized.contains("court")
    {
        push_unique(
            &mut checklist,
            "Check procedural posture, jurisdiction, claims, elements, burdens, evidence, deadlines, defenses, and remedies.",
        );
    } else if normalized.contains("real estate") || normalized.contains("lease") {
        push_unique(
            &mut checklist,
            "Check property description, title, diligence, covenants, closing, leases, zoning, environmental, and default remedies.",
        );
    } else if normalized.contains("finance")
        || normalized.contains("banking")
        || normalized.contains("credit")
        || normalized.contains("loan")
    {
        push_unique(
            &mut checklist,
            "Check debt economics, collateral, guarantees, covenants, defaults, intercreditor terms, usury, and regulatory limits.",
        );
    } else if normalized.contains("tax") {
        push_unique(
            &mut checklist,
            "Check taxpayer, transaction steps, characterization, timing, basis, withholding, reporting, and authority level.",
        );
    } else {
        push_unique(
            &mut checklist,
            "Apply the practice-area standard issue-spotting checklist before drafting the final deliverable.",
        );
    }

    checklist.truncate(8);
    checklist
}

fn push_unique(checklist: &mut Vec<String>, item: &str) {
    if !checklist.iter().any(|existing| existing == item) {
        checklist.push(item.to_string());
    }
}

fn normalized_task_terms(task_spec: &BenchmarkTaskSpec) -> String {
    let mut terms = vec![
        task_spec.practice_area.as_str(),
        task_spec.workflow.as_str(),
        task_spec.work_type.as_str(),
        task_spec.title.as_str(),
    ];
    terms.extend(task_spec.tags.iter().map(String::as_str));
    terms.join(" ").to_ascii_lowercase().replace('_', " ")
}

fn pre_submit_protocol_report(
    request: &LegalBenchmarkAgentRunRequest,
    tool_calls: &[ToolCallRecord],
    transcript: &[TranscriptEvent],
    submission: &LegalBenchmarkSubmission,
) -> PreSubmitProtocolReport {
    let mut missing_steps = Vec::new();
    let source_artifacts = request.task_spec.source_artifacts.as_slice();
    let required_deliverables = request
        .task_spec
        .deliverables
        .iter()
        .filter(|deliverable| deliverable.required)
        .collect::<Vec<_>>();

    let missing_submitted = required_deliverables
        .iter()
        .filter(|deliverable| {
            !submission.deliverables.iter().any(|submitted| {
                output_path_matches(deliverable.required_path.as_str(), submitted.as_str())
            })
        })
        .map(|deliverable| deliverable.required_path.clone())
        .collect::<Vec<_>>();
    if !missing_submitted.is_empty() {
        missing_steps.push(format!(
            "submit every required deliverable path: {}",
            missing_submitted.join(", ")
        ));
    }

    if allowed_tool(request, "validate_deliverables") && !required_deliverables.is_empty() {
        let missing_validations = required_deliverables
            .iter()
            .filter(|deliverable| {
                !deliverable_validation_passed(tool_calls, deliverable.required_path.as_str())
            })
            .map(|deliverable| deliverable.required_path.clone())
            .collect::<Vec<_>>();
        if !missing_validations.is_empty() {
            missing_steps.push(format!(
                "validate required output paths with validate_deliverables: {}",
                missing_validations.join(", ")
            ));
        }
    }

    if !source_artifacts.is_empty() {
        if allowed_tool(request, "inventory")
            && !inventory_discovers_all(tool_calls, source_artifacts)
        {
            missing_steps.push(String::from(
                "run inventory on the documents root and discover every source artifact",
            ));
        }

        if source_inspection_tools_available(request) {
            let missing_inspections = source_artifacts
                .iter()
                .filter(|artifact| !source_artifact_inspected(request, tool_calls, artifact))
                .map(|artifact| source_tool_relative_path(artifact))
                .collect::<Vec<_>>();
            if !missing_inspections.is_empty() {
                missing_steps.push(format!(
                    "inspect each source with the strongest available document tool: {}",
                    missing_inspections.join(", ")
                ));
            }
        }

        if allowed_tool(request, "evidence_table") {
            let missing_evidence = source_artifacts
                .iter()
                .filter(|artifact| !evidence_table_covers_source(tool_calls, artifact))
                .map(|artifact| source_tool_relative_path(artifact))
                .collect::<Vec<_>>();
            if !missing_evidence.is_empty() {
                missing_steps.push(format!(
                    "capture evidence_table rows for source-backed claims: {}",
                    missing_evidence.join(", ")
                ));
            }
        }

        if !submission_note_self_checks(submission.note.as_deref())
            && !transcript_self_check_recorded(transcript)
        {
            missing_steps.push(String::from(
                "include a final self-check note covering evidence, deliverables, and uncited or unsupported claims",
            ));
        }
    }

    PreSubmitProtocolReport {
        satisfied: missing_steps.is_empty(),
        missing_steps,
    }
}

fn allowed_tool(request: &LegalBenchmarkAgentRunRequest, tool_name: &str) -> bool {
    request
        .run_config
        .tool_policy
        .allowed_tools
        .iter()
        .any(|allowed| allowed == tool_name)
}

fn source_inspection_tools_available(request: &LegalBenchmarkAgentRunRequest) -> bool {
    [
        "read",
        "grep",
        "pdf_search",
        "email_summary",
        "spreadsheet_summary",
    ]
    .iter()
    .any(|tool| allowed_tool(request, tool))
}

fn inventory_discovers_all(
    tool_calls: &[ToolCallRecord],
    source_artifacts: &[SourceArtifact],
) -> bool {
    source_artifacts.iter().all(|artifact| {
        tool_calls.iter().any(|call| {
            if call.tool_name != "inventory" || call.error_kind.is_some() {
                return false;
            }
            if input_root(call) != Some(LegalBenchmarkPathRoot::Documents) {
                return false;
            }
            output_payload(call, "inventory")
                .and_then(|output| output.get("artifacts"))
                .and_then(Value::as_array)
                .is_some_and(|artifacts| {
                    artifacts.iter().any(|row| {
                        row.get("relative_path")
                            .and_then(Value::as_str)
                            .is_some_and(|path| source_path_matches(artifact, path))
                    })
                })
        })
    })
}

fn source_artifact_inspected(
    request: &LegalBenchmarkAgentRunRequest,
    tool_calls: &[ToolCallRecord],
    artifact: &SourceArtifact,
) -> bool {
    if is_spreadsheet_source(artifact) && allowed_tool(request, "spreadsheet_summary") {
        return source_path_tool_called(tool_calls, "spreadsheet_summary", artifact);
    }
    if is_email_source(artifact) && allowed_tool(request, "email_summary") {
        return source_path_tool_called(tool_calls, "email_summary", artifact);
    }
    if is_pdf_source(artifact) && allowed_tool(request, "pdf_search") {
        return pdf_search_covers_source(tool_calls, artifact);
    }
    if allowed_tool(request, "read") && source_path_tool_called(tool_calls, "read", artifact) {
        return true;
    }
    if allowed_tool(request, "grep") && grep_covers_source(tool_calls, artifact) {
        return true;
    }
    if allowed_tool(request, "pdf_search") && pdf_search_covers_source(tool_calls, artifact) {
        return true;
    }
    false
}

fn source_path_tool_called(
    tool_calls: &[ToolCallRecord],
    tool_name: &str,
    artifact: &SourceArtifact,
) -> bool {
    tool_calls.iter().any(|call| {
        call.tool_name == tool_name
            && call.error_kind.is_none()
            && input_root(call) == Some(LegalBenchmarkPathRoot::Documents)
            && input_relative_path(call).is_some_and(|path| source_path_matches(artifact, path))
            && output_payload(call, tool_name).is_some()
    })
}

fn grep_covers_source(tool_calls: &[ToolCallRecord], artifact: &SourceArtifact) -> bool {
    tool_calls.iter().any(|call| {
        call.tool_name == "grep"
            && call.error_kind.is_none()
            && input_root(call) == Some(LegalBenchmarkPathRoot::Documents)
            && output_payload(call, "grep")
                .and_then(|output| output.get("matches"))
                .and_then(Value::as_array)
                .is_some_and(|matches| {
                    matches.iter().any(|row| {
                        row.get("relative_path")
                            .and_then(Value::as_str)
                            .is_some_and(|path| source_path_matches(artifact, path))
                    })
                })
    })
}

fn pdf_search_covers_source(tool_calls: &[ToolCallRecord], artifact: &SourceArtifact) -> bool {
    tool_calls.iter().any(|call| {
        if call.tool_name != "pdf_search"
            || call.error_kind.is_some()
            || input_root(call) != Some(LegalBenchmarkPathRoot::Documents)
        {
            return false;
        }
        if input_relative_path(call).is_some_and(|path| source_path_matches(artifact, path))
            && output_payload(call, "pdf_search").is_some()
        {
            return true;
        }
        output_payload(call, "pdf_search")
            .and_then(|output| output.get("matches"))
            .and_then(Value::as_array)
            .is_some_and(|matches| {
                matches.iter().any(|row| {
                    row.get("relative_path")
                        .and_then(Value::as_str)
                        .is_some_and(|path| source_path_matches(artifact, path))
                })
            })
    })
}

fn evidence_table_covers_source(tool_calls: &[ToolCallRecord], artifact: &SourceArtifact) -> bool {
    tool_calls.iter().any(|call| {
        call.tool_name == "evidence_table"
            && call.error_kind.is_none()
            && output_payload(call, "evidence_table")
                .and_then(|output| output.get("rows"))
                .and_then(Value::as_array)
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row.get("source_ref")
                            .and_then(Value::as_str)
                            .is_some_and(|source_ref| source_ref_matches(artifact, source_ref))
                    })
                })
    })
}

fn deliverable_validation_passed(tool_calls: &[ToolCallRecord], required_path: &str) -> bool {
    tool_calls.iter().any(|call| {
        call.tool_name == "validate_deliverables"
            && call.error_kind.is_none()
            && matches!(
                input_root(call),
                Some(LegalBenchmarkPathRoot::Output | LegalBenchmarkPathRoot::Workspace)
            )
            && output_payload(call, "validate_deliverables")
                .and_then(|output| output.get("validations"))
                .and_then(Value::as_array)
                .is_some_and(|validations| {
                    validations.iter().any(|validation| {
                        let path_matches = validation
                            .get("relative_path")
                            .and_then(Value::as_str)
                            .is_some_and(|path| output_path_matches(required_path, path));
                        let exists = validation
                            .get("exists")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let readable = validation
                            .get("readable")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        path_matches && exists && readable
                    })
                })
    })
}

fn submission_note_self_checks(note: Option<&str>) -> bool {
    let Some(note) = note else {
        return false;
    };
    let normalized = note.to_ascii_lowercase();
    (normalized.contains("self-check") || normalized.contains("self check"))
        && (normalized.contains("evidence")
            || normalized.contains("citation")
            || normalized.contains("cited"))
        && (normalized.contains("deliverable") || normalized.contains("output"))
        && (normalized.contains("uncited")
            || normalized.contains("unsupported")
            || normalized.contains("unsubstantiated"))
}

fn transcript_self_check_recorded(transcript: &[TranscriptEvent]) -> bool {
    transcript.iter().any(|event| {
        event
            .content
            .as_deref()
            .is_some_and(|content| submission_note_self_checks(Some(content)))
    })
}

fn input_payload(call: &ToolCallRecord) -> Option<&Value> {
    call.input.get("input")
}

fn output_payload<'a>(call: &'a ToolCallRecord, tool_name: &str) -> Option<&'a Value> {
    let output = call.output.as_ref()?;
    if output.get("tool").and_then(Value::as_str) != Some(tool_name) {
        return None;
    }
    output.get("output")
}

fn input_root(call: &ToolCallRecord) -> Option<LegalBenchmarkPathRoot> {
    match input_payload(call)?.get("root")?.as_str()? {
        "documents" => Some(LegalBenchmarkPathRoot::Documents),
        "workspace" => Some(LegalBenchmarkPathRoot::Workspace),
        "output" => Some(LegalBenchmarkPathRoot::Output),
        _ => None,
    }
}

fn input_relative_path(call: &ToolCallRecord) -> Option<&str> {
    input_payload(call)?.get("relative_path")?.as_str()
}

fn is_spreadsheet_source(artifact: &SourceArtifact) -> bool {
    let media_type = artifact.media_type.to_ascii_lowercase();
    let path = artifact.relative_path.to_ascii_lowercase();
    media_type.contains("spreadsheet")
        || media_type.contains("excel")
        || media_type == "text/csv"
        || path.ends_with(".xlsx")
        || path.ends_with(".xls")
        || path.ends_with(".csv")
        || path.ends_with(".tsv")
}

fn is_email_source(artifact: &SourceArtifact) -> bool {
    let media_type = artifact.media_type.to_ascii_lowercase();
    let path = artifact.relative_path.to_ascii_lowercase();
    media_type == "message/rfc822" || path.ends_with(".eml")
}

fn is_pdf_source(artifact: &SourceArtifact) -> bool {
    artifact.media_type.eq_ignore_ascii_case("application/pdf")
        || artifact
            .relative_path
            .to_ascii_lowercase()
            .ends_with(".pdf")
}

fn source_ref_matches(artifact: &SourceArtifact, source_ref: &str) -> bool {
    source_ref == artifact.artifact_id || source_path_matches(artifact, source_ref)
}

fn source_path_matches(artifact: &SourceArtifact, observed_path: &str) -> bool {
    let observed = normalize_source_path(observed_path);
    observed == normalize_source_path(&artifact.relative_path)
        || observed == normalize_source_path(&source_tool_relative_path(artifact))
}

fn source_tool_relative_path(artifact: &SourceArtifact) -> String {
    let normalized = strip_relative_prefixes(&artifact.relative_path);
    if let Some((_, suffix)) = normalized.rsplit_once("/documents/") {
        return suffix.to_string();
    }
    normalized
        .strip_prefix("documents/")
        .unwrap_or(normalized.as_str())
        .to_string()
}

fn normalize_source_path(path: &str) -> String {
    strip_relative_prefixes(path).to_ascii_lowercase()
}

fn output_path_matches(expected_path: &str, observed_path: &str) -> bool {
    normalize_output_path(expected_path) == normalize_output_path(observed_path)
}

fn normalize_output_path(path: &str) -> String {
    let stripped = strip_relative_prefixes(path).to_ascii_lowercase();
    stripped
        .strip_prefix("output/")
        .or_else(|| stripped.strip_prefix("outputs/"))
        .unwrap_or(stripped.as_str())
        .to_string()
}

fn strip_relative_prefixes(path: &str) -> String {
    path.trim_start_matches("./")
        .trim_start_matches('/')
        .replace('\\', "/")
}

fn execute_model_tool_call(
    tool_call: &ModelToolCall,
    workspace: &LegalBenchmarkToolWorkspace,
) -> Result<LegalBenchmarkToolExecution, String> {
    let input: LegalBenchmarkToolInput = serde_json::from_value(json!({
        "tool": tool_call.tool_name.clone(),
        "input": tool_call.arguments.clone(),
    }))
    .map_err(|err| err.to_string())?;
    Ok(execute_legal_benchmark_tool(workspace, input))
}

fn append_tool_execution(
    transcript: &mut Vec<TranscriptEvent>,
    tool_calls: &mut Vec<ToolCallRecord>,
    tool_receipts: &mut Vec<LegalBenchmarkToolReceipt>,
    provider_tool_call: &ModelToolCall,
    mut execution: LegalBenchmarkToolExecution,
) -> Result<ToolResultMessage, LegalBenchmarkAgentRunError> {
    execution.receipt.tool_call_id = provider_tool_call.tool_call_id.clone();
    execution.receipt.metadata.insert(
        String::from("provider_tool_call_id"),
        Value::String(provider_tool_call.tool_call_id.clone()),
    );
    execution.tool_call_record.tool_call_id = provider_tool_call.tool_call_id.clone();
    execution.tool_call_record.call_event_index = next_event_index(transcript);
    execution.tool_call_record.result_event_index = Some(next_event_index(transcript) + 1);
    let output_value = execution
        .output
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    execution.tool_call_record.output = output_value.clone();
    execution.tool_call_record.error_kind = execution
        .receipt
        .failure_kind
        .map(|kind| format!("{kind:?}"));
    push_transcript_event(
        transcript,
        TranscriptEventKind::ToolCall,
        Some("assistant"),
        None,
        Some(json!({
            "tool_call_id": provider_tool_call.tool_call_id.clone(),
            "tool_name": provider_tool_call.tool_name.clone(),
            "input": provider_tool_call.arguments.clone(),
        })),
    );
    push_transcript_event(
        transcript,
        TranscriptEventKind::ToolResult,
        Some("tool"),
        None,
        Some(json!({
            "tool_call_id": provider_tool_call.tool_call_id.clone(),
            "output": output_value,
            "receipt": execution.receipt.clone(),
        })),
    );
    let result_payload = json!({
        "output": execution.output.clone(),
        "failure_kind": execution.receipt.failure_kind,
        "failure_detail": execution.receipt.failure_detail.clone(),
        "receipt_ref": execution.receipt.tool_call_id,
    });
    let result_content = serde_json::to_string(&result_payload)?;
    tool_calls.push(execution.tool_call_record);
    tool_receipts.push(execution.receipt);
    Ok(ToolResultMessage {
        tool_call_id: provider_tool_call.tool_call_id.clone(),
        tool_name: provider_tool_call.tool_name.clone(),
        content: result_content,
        is_error: tool_receipts
            .last()
            .and_then(|receipt| receipt.failure_kind)
            .is_some(),
        metadata: Metadata::new(),
    })
}

fn push_model_response_event(transcript: &mut Vec<TranscriptEvent>, response: &ModelResponse) {
    push_transcript_event(
        transcript,
        TranscriptEventKind::Assistant,
        Some("assistant"),
        response.final_text.clone(),
        Some(json!({
            "response_id": response.response_id.clone(),
            "route_id": response.route_id.clone(),
            "model_id": response.model_id.clone(),
            "stop_reason": response.stop_reason,
            "tool_calls": response.tool_calls.clone(),
            "usage": response.usage.clone(),
            "elapsed_ms": response.elapsed_ms,
            "retry_count": response.retry_count,
            "raw_response_hash": response.raw_response_hash.clone(),
            "metadata": response.metadata.clone(),
        })),
    );
}

fn provider_route_run_metadata(
    route: &ModelProviderRoute,
) -> Result<Metadata, LegalBenchmarkAgentRunError> {
    let mut metadata = route.metadata.clone();
    metadata.insert(
        String::from("route_id"),
        Value::String(route.route_id.clone()),
    );
    metadata.insert(
        String::from("route_model_id"),
        Value::String(route.model_id.clone()),
    );
    metadata.insert(
        String::from("route_config_hash"),
        Value::String(
            route
                .config_hash()
                .map_err(LegalBenchmarkAgentRunError::Provider)?,
        ),
    );
    Ok(metadata)
}

fn push_transcript_event(
    transcript: &mut Vec<TranscriptEvent>,
    kind: TranscriptEventKind,
    role: Option<&str>,
    content: Option<String>,
    payload: Option<Value>,
) {
    transcript.push(TranscriptEvent {
        event_index: next_event_index(transcript),
        event_kind: kind,
        role: role.map(ToOwned::to_owned),
        content,
        payload,
        timestamp_ms: now_ms(),
    });
}

fn next_event_index(transcript: &[TranscriptEvent]) -> u64 {
    u64::try_from(transcript.len()).unwrap_or(u64::MAX)
}

fn apply_model_metrics(metrics: &mut RunMetrics, response: &ModelResponse) {
    metrics.model_turns = metrics.model_turns.saturating_add(1);
    metrics.input_tokens = metrics
        .input_tokens
        .saturating_add(response.usage.input_tokens);
    metrics.output_tokens = metrics
        .output_tokens
        .saturating_add(response.usage.output_tokens);
    metrics.estimated_cost_micro_usd = metrics
        .estimated_cost_micro_usd
        .saturating_add(response.usage.estimated_cost_micro_usd);
}

fn provider_terminal_state(kind: ModelAdapterFailureKind) -> RunTerminalState {
    match kind {
        ModelAdapterFailureKind::ContextOverflow => RunTerminalState::ContextOverflow,
        ModelAdapterFailureKind::SafetyRefusal => RunTerminalState::PolicyFailure,
        _ => RunTerminalState::ProviderFailure,
    }
}

fn collect_output_artifacts(
    root: &Path,
) -> Result<Vec<SourceArtifact>, LegalBenchmarkAgentRunError> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    let mut artifacts = Vec::new();
    for (index, file) in files.iter().enumerate() {
        artifacts.push(artifact_from_file(
            format!("artifact.output.{index}"),
            ArtifactKind::GeneratedDeliverable,
            root,
            file,
            DataClassification::BenchmarkConfidential,
            Some(String::from("legal_benchmark_agent_run")),
        )?);
    }
    Ok(artifacts)
}

fn collect_files(
    root: &Path,
    path: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), LegalBenchmarkAgentRunError> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|source| LegalBenchmarkAgentRunError::Io {
        path: path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| LegalBenchmarkAgentRunError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| LegalBenchmarkAgentRunError::Io {
                path: entry_path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_files(root, &entry_path, files)?;
        } else if file_type.is_file() && entry_path.strip_prefix(root).is_ok() {
            files.push(entry_path);
        }
    }
    Ok(())
}

fn write_run_artifacts(
    paths: &LegalBenchmarkRunArtifactPaths,
    run_config: &RunConfig,
    run_record: &RunRecord,
    output_manifest: &ArtifactManifest,
    extraction_receipt_refs: &[String],
    tool_receipts: &[LegalBenchmarkToolReceipt],
    run_receipt: &LegalBenchmarkRunReceipt,
) -> Result<(), LegalBenchmarkAgentRunError> {
    fs::create_dir_all(&paths.run_root).map_err(|source| LegalBenchmarkAgentRunError::Io {
        path: paths.run_root.clone(),
        source,
    })?;
    write_json(&paths.config_json, run_config)?;
    write_transcript_jsonl(&paths.transcript_jsonl, &run_record.transcript)?;
    write_json(&paths.metrics_json, &run_record.metrics)?;
    write_json(&paths.output_artifact_manifest_json, output_manifest)?;
    write_json(&paths.extraction_receipts_json, &extraction_receipt_refs)?;
    write_json(&paths.tool_receipts_json, &tool_receipts)?;
    write_json(&paths.run_record_json, run_record)?;
    write_json(&paths.run_receipt_json, run_receipt)?;
    Ok(())
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), LegalBenchmarkAgentRunError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| LegalBenchmarkAgentRunError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_transcript_jsonl(
    path: &Path,
    transcript: &[TranscriptEvent],
) -> Result<(), LegalBenchmarkAgentRunError> {
    let mut file = fs::File::create(path).map_err(|source| LegalBenchmarkAgentRunError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    for event in transcript {
        serde_json::to_writer(&mut file, event)?;
        file.write_all(b"\n")
            .map_err(|source| LegalBenchmarkAgentRunError::Io {
                path: path.to_path_buf(),
                source,
            })?;
    }
    Ok(())
}

fn run_artifact_paths(run_root: &Path) -> LegalBenchmarkRunArtifactPaths {
    LegalBenchmarkRunArtifactPaths {
        run_root: run_root.to_path_buf(),
        config_json: run_root.join("config.json"),
        transcript_jsonl: run_root.join("transcript.jsonl"),
        metrics_json: run_root.join("metrics.json"),
        output_artifact_manifest_json: run_root.join("output_artifact_manifest.json"),
        extraction_receipts_json: run_root.join("extraction_receipts.json"),
        tool_receipts_json: run_root.join("tool_receipts.json"),
        run_record_json: run_root.join("run_record.json"),
        run_receipt_json: run_root.join("run_receipt.json"),
    }
}

fn legal_benchmark_run_id(
    task_spec: &BenchmarkTaskSpec,
    run_config_hash: &str,
    nonce: Option<&str>,
) -> String {
    let nonce = nonce
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| now_ms().to_string());
    let hash_prefix = run_config_hash.get(..12).unwrap_or(run_config_hash);
    format!(
        "run.{}.{}.{}",
        stable_id_part(&task_spec.task_id),
        hash_prefix,
        stable_id_part(&nonce)
    )
}

fn stable_id_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '.'
            }
        })
        .collect()
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactManifestRole, CriterionKind, DeliverableKind, JudgeMode, JudgePolicy,
        MockModelAdapter, ModelProviderRoute, ModelUsage, RunActor, ToolPolicy,
        build_input_artifact_manifest,
    };

    fn task_spec() -> BenchmarkTaskSpec {
        BenchmarkTaskSpec {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            task_id: String::from("legal.mock_agent"),
            task_version: String::from("v1"),
            domain: String::from("legal"),
            practice_area: String::from("contracts"),
            workflow: String::from("draft"),
            title: String::from("Draft memo"),
            instructions: String::from("Read the case file and draft a short memo."),
            work_type: String::from("drafting"),
            tags: vec![String::from("mock")],
            source_artifacts: Vec::new(),
            deliverables: vec![crate::DeliverableSpec {
                deliverable_id: String::from("memo"),
                deliverable_kind: DeliverableKind::Markdown,
                required_path: String::from("memo.md"),
                description: String::from("Short legal memo"),
                required: true,
            }],
            criteria: vec![crate::CriterionSpec {
                criterion_id: String::from("criterion.memo.exists"),
                criterion_kind: CriterionKind::DeliverableValidation,
                description: String::from("The memo exists."),
                weight_bps: Some(10_000),
                deliverable_ids: vec![String::from("memo")],
                source_artifact_ids: Vec::new(),
            }],
            judge_policy: JudgePolicy {
                mode: JudgeMode::Deterministic,
                provider: String::from("mock"),
                model: String::from("mock-judge"),
                prompt_template_id: String::from("judge.mock"),
                prompt_template_hash: String::from("hash.judge.mock"),
                all_pass_required: true,
                sample_count: 1,
            },
            tool_policy: ToolPolicy {
                allowed_tools: vec![String::from("write")],
                network_allowed: false,
                source_artifacts_read_only: true,
                max_turns: 4,
                max_wall_time_seconds: 60,
            },
            source_compatibility: None,
            metadata: Metadata::new(),
        }
    }

    fn run_config(task: &BenchmarkTaskSpec) -> RunConfig {
        RunConfig {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            run_config_id: String::from("run_config.mock_agent"),
            provider: String::from("mock"),
            model: String::from("deterministic-legal-mock"),
            agent_protocol_version: String::from("legal-agent-loop.v1"),
            tool_policy: task.tool_policy.clone(),
            judge_policy: task.judge_policy.clone(),
            random_seed: Some(7),
            metadata: Metadata::new(),
        }
    }

    fn response_with_tool_call(route: &ModelProviderRoute) -> ModelResponse {
        ModelResponse {
            schema_version: crate::LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            response_id: String::from("mock.response.tool"),
            request_id: String::from("model_request"),
            route_id: route.route_id.clone(),
            provider_family: route.family,
            model_id: route.model_id.clone(),
            model_config_hash: String::from("mock-config"),
            secret_reference_id: None,
            final_text: None,
            tool_calls: vec![ModelToolCall {
                tool_call_id: String::from("call.write.memo"),
                tool_name: String::from("write"),
                arguments: json!({
                    "root": "output",
                    "relative_path": "memo.md",
                    "content": "# Memo\n\nContract risk is low.\n",
                    "overwrite": true
                }),
            }],
            stop_reason: ModelStopReason::ToolCalls,
            usage: ModelUsage {
                input_tokens: 12,
                output_tokens: 8,
                total_tokens: 20,
                cached_input_tokens: 0,
                estimated_cost_micro_usd: 0,
            },
            elapsed_ms: 5,
            retry_count: 0,
            raw_response_hash: String::from("mock-raw-tool"),
            created_at_ms: 1,
            metadata: Metadata::new(),
        }
    }

    fn response_write_validate_memo(
        route: &ModelProviderRoute,
        response_id: &str,
        content: &str,
    ) -> ModelResponse {
        let mut response = response_with_tool_call(route);
        response.response_id = response_id.to_owned();
        response.tool_calls = vec![
            ModelToolCall {
                tool_call_id: format!("{response_id}.write"),
                tool_name: String::from("write"),
                arguments: json!({
                    "root": "output",
                    "relative_path": "memo.md",
                    "content": content,
                    "overwrite": true
                }),
            },
            ModelToolCall {
                tool_call_id: format!("{response_id}.validate"),
                tool_name: String::from("validate_deliverables"),
                arguments: json!({
                    "root": "output",
                    "required_paths": ["memo.md"],
                    "max_results": 10
                }),
            },
        ];
        response
    }

    fn response_submit(route: &ModelProviderRoute) -> ModelResponse {
        ModelResponse {
            schema_version: crate::LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            response_id: String::from("mock.response.submit"),
            request_id: String::from("model_request"),
            route_id: route.route_id.clone(),
            provider_family: route.family,
            model_id: route.model_id.clone(),
            model_config_hash: String::from("mock-config"),
            secret_reference_id: None,
            final_text: Some(String::from(
                "{\"action\":\"submit\",\"deliverables\":[\"memo.md\"]}",
            )),
            tool_calls: Vec::new(),
            stop_reason: ModelStopReason::Stop,
            usage: ModelUsage {
                input_tokens: 6,
                output_tokens: 3,
                total_tokens: 9,
                cached_input_tokens: 0,
                estimated_cost_micro_usd: 0,
            },
            elapsed_ms: 3,
            retry_count: 0,
            raw_response_hash: String::from("mock-raw-submit"),
            created_at_ms: 2,
            metadata: Metadata::new(),
        }
    }

    fn response_submit_with_note(
        route: &ModelProviderRoute,
        deliverable_path: &str,
        note: &str,
    ) -> ModelResponse {
        let mut response = response_submit(route);
        response.response_id = String::from("mock.response.submit.note");
        response.final_text = Some(
            json!({
                "action": "submit",
                "deliverables": [deliverable_path],
                "note": note
            })
            .to_string(),
        );
        response
    }

    #[derive(Clone, Debug)]
    struct RecordingModelAdapter {
        route: ModelProviderRoute,
        responses: Vec<ModelResponse>,
        request_tool_names_by_turn: Vec<Vec<String>>,
    }

    impl RecordingModelAdapter {
        fn new(route: ModelProviderRoute, response: ModelResponse) -> Self {
            Self::new_sequence(route, vec![response])
        }

        fn new_sequence(route: ModelProviderRoute, responses: Vec<ModelResponse>) -> Self {
            Self {
                route,
                responses,
                request_tool_names_by_turn: Vec::new(),
            }
        }
    }

    impl ModelAdapter for RecordingModelAdapter {
        fn route(&self) -> &ModelProviderRoute {
            &self.route
        }

        fn complete(&mut self, request: &ModelRequest) -> Result<ModelResponse, ModelAdapterError> {
            self.request_tool_names_by_turn.push(
                request
                    .tools
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect::<Vec<_>>(),
            );
            self.responses.drain(..1).next().ok_or_else(|| {
                ModelAdapterError::new(
                    ModelAdapterFailureKind::ProviderError,
                    "recording adapter has no queued response",
                )
            })
        }
    }

    fn request_tool_names(adapter: &RecordingModelAdapter, turn_index: usize) -> Vec<String> {
        adapter
            .request_tool_names_by_turn
            .get(turn_index)
            .cloned()
            .unwrap_or_default()
    }

    fn response_with_protocol_tool_calls(route: &ModelProviderRoute) -> ModelResponse {
        let mut response = response_with_tool_call(route);
        response.response_id = String::from("mock.response.protocol.tools");
        response.tool_calls = vec![
            ModelToolCall {
                tool_call_id: String::from("call.inventory.documents"),
                tool_name: String::from("inventory"),
                arguments: json!({
                    "root": "documents",
                    "max_results": 20,
                    "include_hidden": false,
                    "include_hashes": true
                }),
            },
            ModelToolCall {
                tool_call_id: String::from("call.read.case"),
                tool_name: String::from("read"),
                arguments: json!({
                    "root": "documents",
                    "relative_path": "case.txt",
                    "prefer_extracted": false
                }),
            },
            ModelToolCall {
                tool_call_id: String::from("call.evidence.case"),
                tool_name: String::from("evidence_table"),
                arguments: json!({
                    "entries": [{
                        "source_ref": "artifact.case",
                        "locator": "case.txt",
                        "quote": "The contract requires notice within five days.",
                        "note": "deadline support for memo"
                    }]
                }),
            },
            ModelToolCall {
                tool_call_id: String::from("call.write.protocol.memo"),
                tool_name: String::from("write"),
                arguments: json!({
                    "root": "output",
                    "relative_path": "outputs/memo.md",
                    "content": "# Memo\n\nThe notice deadline is five days, supported by the case file.\n",
                    "overwrite": true
                }),
            },
            ModelToolCall {
                tool_call_id: String::from("call.validate.memo"),
                tool_name: String::from("validate_deliverables"),
                arguments: json!({
                    "root": "output",
                    "required_paths": ["outputs/memo.md"],
                    "max_results": 10
                }),
            },
        ];
        response
    }

    #[test]
    fn mock_agent_run_writes_outputs_and_run_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");

        let task = task_spec();
        let input_manifest = build_input_artifact_manifest(&task);
        assert_eq!(input_manifest.manifest_role, ArtifactManifestRole::Input);
        let config = run_config(&task);
        let route = ModelProviderRoute::mock("mock.agent", "deterministic-legal-mock");
        let mut adapter = MockModelAdapter::new(
            route.clone(),
            vec![
                Ok(response_with_tool_call(&route)),
                Ok(response_submit(&route)),
            ],
        );
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: vec![String::from("Keep the memo short.")],
            extraction_receipt_refs: vec![String::from("extract.mock.1")],
            run_nonce: Some(String::from("attempt.1")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");

        assert_eq!(result.terminal_state, RunTerminalState::Submitted);
        assert_eq!(result.run_record.metrics.model_turns, 2);
        assert_eq!(result.run_record.metrics.tool_call_count, 1);
        assert_eq!(result.output_artifact_manifest.artifacts.len(), 1);
        assert_eq!(
            fs::read_to_string(output_root.join("memo.md")).expect("memo"),
            "# Memo\n\nContract risk is low.\n"
        );
        assert!(result.paths.config_json.exists());
        assert!(result.paths.transcript_jsonl.exists());
        assert!(result.paths.metrics_json.exists());
        assert!(result.paths.output_artifact_manifest_json.exists());
        assert!(result.paths.tool_receipts_json.exists());
        assert!(result.paths.run_record_json.exists());
        assert!(result.paths.run_receipt_json.exists());

        let decoded: RunRecord = serde_json::from_slice(
            &fs::read(&result.paths.run_record_json).expect("run record bytes"),
        )
        .expect("run record json");
        assert_eq!(decoded.terminal_state, RunTerminalState::Submitted);
        assert_eq!(
            decoded.metadata.get("route_id").and_then(Value::as_str),
            Some("mock.agent")
        );
        assert_eq!(
            result
                .run_receipt
                .metadata
                .get("route_model_id")
                .and_then(Value::as_str),
            Some("deterministic-legal-mock")
        );
        assert_eq!(decoded.extraction_receipt_refs, vec!["extract.mock.1"]);
        let coverage = decoded.coverage_snapshot.expect("coverage snapshot");
        assert!(!coverage.hidden_criteria_visible);
        assert_eq!(coverage.deliverable_sections.len(), 2);
        let user_prompt = decoded
            .transcript
            .iter()
            .find(|event| event.event_kind == TranscriptEventKind::User)
            .and_then(|event| event.content.as_deref())
            .expect("user prompt");
        assert!(user_prompt.contains("Practice-area issue checklist"));
        assert!(user_prompt.contains("obligations, conditions, deadlines"));
        assert!(!user_prompt.contains("The memo exists."));
    }

    #[test]
    fn agent_request_advertises_only_policy_allowed_tools() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");

        let mut task = task_spec();
        task.tool_policy.allowed_tools = vec![String::from("write")];
        let input_manifest = build_input_artifact_manifest(&task);
        let config = run_config(&task);
        let route = ModelProviderRoute::mock("mock.agent.filtered", "deterministic-legal-mock");
        let mut adapter = RecordingModelAdapter::new(route.clone(), response_submit(&route));
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("tool-filter")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");

        assert_eq!(result.terminal_state, RunTerminalState::Submitted);
        assert_eq!(request_tool_names(&adapter, 0), vec![String::from("write")]);
    }

    #[test]
    fn agent_suppresses_tools_after_required_write_without_sources() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");

        let mut task = task_spec();
        task.tool_policy.allowed_tools = vec![String::from("write")];
        let input_manifest = build_input_artifact_manifest(&task);
        let config = run_config(&task);
        let route =
            ModelProviderRoute::mock("mock.agent.write_then_submit", "deterministic-legal-mock");
        let mut adapter = RecordingModelAdapter::new_sequence(
            route.clone(),
            vec![response_with_tool_call(&route), response_submit(&route)],
        );
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("tool-suppression")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");

        assert_eq!(result.terminal_state, RunTerminalState::Submitted);
        assert_eq!(request_tool_names(&adapter, 0), vec![String::from("write")]);
        assert!(request_tool_names(&adapter, 1).is_empty());
    }

    #[test]
    fn no_tool_calls_without_submit_is_terminal() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");
        let task = task_spec();
        let input_manifest = build_input_artifact_manifest(&task);
        let config = run_config(&task);
        let route = ModelProviderRoute::mock("mock.no_tools", "deterministic-legal-mock");
        let mut response = response_submit(&route);
        response.final_text = Some(String::from("I am done."));
        let mut adapter = MockModelAdapter::new(route, vec![Ok(response)]);
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("attempt.2")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");
        assert_eq!(result.terminal_state, RunTerminalState::NoToolCalls);
        assert_eq!(result.run_record.metrics.model_turns, 1);
    }

    #[test]
    fn runner_does_not_materialize_answer_text_from_final_response() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");
        let task = task_spec();
        let input_manifest = build_input_artifact_manifest(&task);
        let config = run_config(&task);
        let route = ModelProviderRoute::mock("mock.no_injection", "deterministic-legal-mock");
        let mut response = response_submit(&route);
        response.final_text = Some(
            json!({
                "action": "submit",
                "deliverables": ["memo.md"],
                "answer_text": "# Memo\n\nThis text must not be written by the runner."
            })
            .to_string(),
        );
        let mut adapter = MockModelAdapter::new(route, vec![Ok(response)]);
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("no-runner-answer-injection")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");

        assert_eq!(result.terminal_state, RunTerminalState::Submitted);
        assert!(!output_root.join("memo.md").exists());
        assert!(result.output_artifact_manifest.artifacts.is_empty());
        assert_eq!(result.run_record.metrics.tool_call_count, 0);
    }

    #[test]
    fn source_backed_submit_waits_for_protocol_coverage() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");
        let case_path = documents_root.join("case.txt");
        fs::write(
            &case_path,
            "The contract requires notice within five days.\nThe counterparty waived no rights.\n",
        )
        .expect("case file");

        let mut task = task_spec();
        task.source_artifacts = vec![
            artifact_from_file(
                String::from("artifact.case"),
                ArtifactKind::SourceDocument,
                &documents_root,
                &case_path,
                DataClassification::BenchmarkConfidential,
                Some(String::from("test fixture")),
            )
            .expect("source artifact"),
        ];
        task.deliverables[0].required_path = String::from("outputs/memo.md");
        task.criteria[0].source_artifact_ids = vec![String::from("artifact.case")];
        task.tool_policy.allowed_tools = vec![
            String::from("inventory"),
            String::from("read"),
            String::from("write"),
            String::from("evidence_table"),
            String::from("validate_deliverables"),
        ];
        task.tool_policy.max_turns = 4;
        let input_manifest = build_input_artifact_manifest(&task);
        let config = run_config(&task);
        let route = ModelProviderRoute::mock("mock.protocol", "deterministic-legal-mock");
        let mut early_submit = response_submit(&route);
        early_submit.final_text = Some(String::from(
            "{\"action\":\"submit\",\"deliverables\":[\"outputs/memo.md\"]}",
        ));
        let mut adapter = MockModelAdapter::new(
            route.clone(),
            vec![
                Ok(early_submit),
                Ok(response_with_protocol_tool_calls(&route)),
                Ok(response_submit_with_note(
                    &route,
                    "outputs/memo.md",
                    "Final self-check: evidence rows support each claim, deliverables validated, and no uncited or unsupported claims remain.",
                )),
            ],
        );
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("protocol.coverage")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");

        assert_eq!(result.terminal_state, RunTerminalState::Submitted);
        assert_eq!(result.run_record.metrics.model_turns, 3);
        assert_eq!(result.run_record.metrics.tool_call_count, 5);
        assert!(
            result.run_record.transcript.iter().any(|event| {
                event.content.as_deref() == Some("pre-submit protocol incomplete")
            })
        );
        assert!(
            output_root.join("outputs/memo.md").exists(),
            "memo should be written under the required output path"
        );
        let coverage = result
            .run_record
            .coverage_snapshot
            .as_ref()
            .expect("coverage snapshot");
        assert!(coverage.documents.iter().any(|document| {
            document.artifact_id == "artifact.case" && document.discovered && document.read
        }));
        assert!(!coverage.evidence_refs.is_empty());
        assert!(
            coverage.validations.iter().any(|validation| {
                validation.target_ref == "outputs/memo.md" && validation.passed
            })
        );
        assert!(!coverage.self_checks.is_empty());
    }

    #[test]
    fn legacy_output_marker_metadata_does_not_mutate_model_write() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");

        let mut task = task_spec();
        task.tool_policy.allowed_tools =
            vec![String::from("write"), String::from("validate_deliverables")];
        task.tool_policy.max_turns = 5;
        let input_manifest = build_input_artifact_manifest(&task);
        let mut config = run_config(&task);
        config.metadata.insert(
            String::from("required_output_markers"),
            json!([{
                "path": "memo.md",
                "label": "coverage line",
                "marker": "COVERAGE-OK"
            }]),
        );
        config.metadata.insert(
            String::from("apply_required_output_markers_on_write"),
            Value::Bool(true),
        );
        let route = ModelProviderRoute::mock("mock.output_markers", "deterministic-legal-mock");
        let mut adapter = MockModelAdapter::new(
            route.clone(),
            vec![
                Ok(response_write_validate_memo(
                    &route,
                    "mock.response.missing_marker",
                    "# Memo\n\nNo marker yet.\n",
                )),
                Ok(response_submit(&route)),
            ],
        );
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("output.marker.repair")),
        };
        let result = run_legal_benchmark_agent(request, &mut adapter).expect("agent run");

        assert_eq!(result.terminal_state, RunTerminalState::Submitted);
        assert_eq!(result.run_record.metrics.model_turns, 2);
        assert!(
            !fs::read_to_string(output_root.join("memo.md"))
                .expect("memo")
                .contains("COVERAGE-OK"),
            "runner must not add text that the model did not write"
        );
        assert!(!result.run_record.transcript.iter().any(|event| {
            event.content.as_deref() == Some("Blueprint output scaffold applied")
                || event
                    .payload
                    .as_ref()
                    .is_some_and(|payload| payload.to_string().contains("required text markers"))
        }));
        assert!(result.run_receipt.answer_integrity.valid);
        assert_eq!(
            result.run_receipt.answer_integrity.answer_files[0].last_modifying_actor,
            RunActor::Model
        );
    }

    #[test]
    fn system_prompt_includes_allowed_coverage_protocol_without_hidden_criteria() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.keep();
        let documents_root = root.join("documents");
        let workspace_root = root.join("workspace");
        let output_root = root.join("output");
        fs::create_dir_all(&documents_root).expect("documents");
        fs::create_dir_all(&workspace_root).expect("workspace");
        fs::create_dir_all(&output_root).expect("output");

        let mut task = task_spec();
        task.tool_policy.allowed_tools = vec![
            String::from("inventory"),
            String::from("read"),
            String::from("grep"),
            String::from("evidence_table"),
            String::from("validate_deliverables"),
            String::from("write"),
        ];
        let input_manifest = build_input_artifact_manifest(&task);
        let config = run_config(&task);
        let request = LegalBenchmarkAgentRunRequest {
            task_spec: task,
            input_artifact_manifest: input_manifest,
            run_config: config,
            tool_workspace: LegalBenchmarkToolWorkspace::new(
                &documents_root,
                &workspace_root,
                &output_root,
            ),
            run_root: root.join("run"),
            module_instructions: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            run_nonce: Some(String::from("prompt.protocol")),
        };
        let prompt = legal_benchmark_system_prompt(&request);

        assert!(prompt.contains("Benchmark operating protocol"));
        assert!(prompt.contains("inventory on the documents root"));
        assert!(prompt.contains("read, grep"));
        assert!(prompt.contains("evidence_table"));
        assert!(prompt.contains("validate_deliverables"));
        assert!(prompt.contains("Final submission may be rejected"));
        assert!(!prompt.contains("The memo exists."));
    }
}
