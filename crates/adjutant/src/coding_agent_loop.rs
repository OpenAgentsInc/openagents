//! CODING_AGENT_LOOP runtime implementation.
//!
//! This module executes a deterministic tool loop driven by DSPy signatures.

use crate::autopilot_loop::{AcpEventSender, DspyStage, DSPY_META_KEY, SESSION_ID_META_KEY};
use crate::planner::TaskPlan;
use crate::tools::{SideEffect, ToolExecutionResult, ToolRegistry};
use crate::{AdjutantError, Task};
use agent_client_protocol_schema as acp;
use anyhow::Result;
use chrono::{DateTime, Utc};
use dsrs::ir::{Complexity as PlanComplexity, PlanIR, PlanStep, StepIntent, VerificationStrategy};
use dsrs::signatures::{
    ContextSelectionSignature, PlanningSignature, ToolCallSignature, ToolResultSignature,
};
use dsrs::{Example, LM, Predict, Prediction, example};
use protocol::hash::canonical_hash;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs::{File, create_dir_all};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CodingAgentConfig {
    pub max_step_iterations: u8,
    pub max_tool_calls: usize,
    pub context_char_limit: usize,
    pub verify_completion: bool,
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            max_step_iterations: 3,
            max_tool_calls: 64,
            context_char_limit: 16_000,
            verify_completion: true,
        }
    }
}

#[derive(Clone)]
pub struct CodingAgentUi {
    pub token_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    pub acp_sender: Option<AcpEventSender>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolHistoryEntry {
    step_id: String,
    tool: String,
    success: bool,
    summary: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ContextPlan {
    include_paths: Option<Vec<String>>,
    exclude_paths: Option<Vec<String>>,
    include_summaries: Option<Vec<String>>,
    notes: Option<String>,
}

#[derive(Debug, Clone)]
struct ToolCallDecision {
    tool: String,
    params: Value,
    expected_outcome: String,
    progress_estimate: f32,
    needs_user_input: bool,
    user_question: String,
    confidence: f32,
}

#[derive(Debug, Clone)]
struct ToolResultInterpretation {
    success: String,
    extracted_facts: Vec<String>,
    should_continue: bool,
    step_utility: f32,
    confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
struct ToolReceipt {
    id: String,
    tool: String,
    params_hash: String,
    output_hash: String,
    step_utility: f32,
    latency_ms: u64,
    side_effects: Vec<SideEffect>,
}

#[derive(Debug, Clone, Serialize)]
struct VerificationReceipt {
    commands_run: Vec<String>,
    exit_codes: Vec<i32>,
    verification_delta: i32,
}

#[derive(Debug, Clone, Serialize)]
struct SessionReceipt {
    session_id: String,
    started_at: String,
    completed_at: String,
    issue_number: Option<i64>,
    plan_hash: String,
    tool_calls: Vec<ToolReceipt>,
    verification: VerificationReceipt,
    final_confidence: f32,
    policy_bundle_id: String,
    signature: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "event")]
enum ReplayEvent {
    ReplayHeader {
        replay_version: u8,
        producer: String,
        created_at: DateTime<Utc>,
    },
    SessionStart {
        t: DateTime<Utc>,
        session_id: String,
        issue_number: Option<i64>,
        policy_bundle_id: String,
    },
    PlanStart {
        t: DateTime<Utc>,
        plan_hash: String,
        step_count: usize,
    },
    ToolCall {
        t: DateTime<Utc>,
        id: String,
        tool: String,
        params: Value,
        params_hash: String,
        step_id: String,
    },
    ToolResult {
        t: DateTime<Utc>,
        id: String,
        output_hash: String,
        exit_code: Option<i32>,
        step_utility: f32,
        latency_ms: u64,
    },
    StepComplete {
        t: DateTime<Utc>,
        step_id: String,
        status: StepStatus,
        iterations: u8,
    },
    Verification {
        t: DateTime<Utc>,
        commands: Vec<String>,
        exit_codes: Vec<i32>,
        verification_delta: i32,
    },
    SessionEnd {
        t: DateTime<Utc>,
        status: SessionStatus,
        confidence: f32,
        total_tool_calls: usize,
        total_latency_ms: u64,
    },
}

#[derive(Serialize, Debug, Clone, Copy)]
enum StepStatus {
    Success,
    Failed,
    Skipped,
    MaxIterationsReached,
}

#[derive(Serialize, Debug, Clone, Copy)]
enum SessionStatus {
    Success,
    Failed,
    Cancelled,
    Timeout,
}

struct ReplayWriter {
    writer: BufWriter<File>,
}

impl ReplayWriter {
    fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    fn emit(&mut self, event: &ReplayEvent) -> Result<()> {
        let value = serde_json::to_value(event)?;
        let canonical = protocol::hash::canonical_json(&value)?;
        writeln!(self.writer, "{}", canonical)?;
        self.writer.flush()?;
        Ok(())
    }
}

pub async fn execute_coding_agent_loop(
    tools: &mut ToolRegistry,
    workspace_root: &Path,
    task: &Task,
    plan: &TaskPlan,
    decision_lm: Option<Arc<LM>>,
    ui: Option<CodingAgentUi>,
    config: CodingAgentConfig,
) -> Result<crate::executor::TaskResult, AdjutantError> {
    let session_id = ui
        .as_ref()
        .and_then(|ui| ui.acp_sender.as_ref())
        .map(|sender| sender.session_id.to_string())
        .unwrap_or_else(crate::autopilot_loop::generate_session_id);
    let policy_bundle_id = resolve_policy_bundle_id();
    let started_at = Utc::now();

    let session_dir = autopilot_core::paths::OpenAgentsPaths::default()
        .session_dir(&session_id);
    create_dir_all(&session_dir).map_err(AdjutantError::IoError)?;

    let mut replay = ReplayWriter::new(&session_dir.join("REPLAY.jsonl"))
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

    replay
        .emit(&ReplayEvent::ReplayHeader {
            replay_version: 1,
            producer: "adjutant".to_string(),
            created_at: started_at,
        })
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

    replay
        .emit(&ReplayEvent::SessionStart {
            t: started_at,
            session_id: session_id.clone(),
            issue_number: parse_issue_number(&task.id),
            policy_bundle_id: policy_bundle_id.clone(),
        })
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

    let tool_schemas = tools.tool_schemas();
    let tool_schema_json = serde_json::to_string(&tool_schemas)
        .unwrap_or_else(|_| "[]".to_string());

    let (context_plan, context_summary) = select_context(
        task,
        plan,
        tools,
        workspace_root,
        decision_lm.clone(),
        &ui,
        config.context_char_limit,
    )
    .await?;

    let per_step_mode = task.title.starts_with("Step ");
    let mut plan_ir = if per_step_mode {
        fallback_plan_ir(task, plan, config.max_step_iterations)
    } else {
        plan_from_signature(task, plan, &context_summary, decision_lm.clone()).await
    };

    if plan_ir.steps.is_empty() {
        plan_ir = fallback_plan_ir(task, plan, config.max_step_iterations);
    }

    let plan_hash = format!(
        "sha256:{}",
        canonical_hash(&plan_ir).map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?
    );

    replay
        .emit(&ReplayEvent::PlanStart {
            t: Utc::now(),
            plan_hash: plan_hash.clone(),
            step_count: plan_ir.steps.len(),
        })
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

    if !per_step_mode {
        emit_planning_stage(&ui, &plan_ir);
        emit_plan_update(&ui, &plan_ir, None);
    }

    let mut tool_history: Vec<ToolHistoryEntry> = Vec::new();
    let mut receipts: Vec<ToolReceipt> = Vec::new();
    let mut modified_files: HashSet<String> = HashSet::new();
    let mut total_latency_ms = 0u64;
    let mut tool_call_count = 0usize;
    let mut step_outcomes: Vec<String> = Vec::new();
    let mut success = true;

    for (step_index, step) in plan_ir.steps.iter().enumerate() {
        if !per_step_mode {
            emit_plan_update(&ui, &plan_ir, Some(step_index));
        }
        emit_step_start(&ui, step_index + 1, plan_ir.steps.len(), step.description.clone());

        let mut iterations: u8 = 0;
        let mut step_done = false;
        let mut step_status = StepStatus::Success;

        while !step_done {
            iterations = iterations.saturating_add(1);
            if iterations > step.max_iterations.max(1) || tool_call_count >= config.max_tool_calls {
                step_status = StepStatus::MaxIterationsReached;
                success = false;
                break;
            }

            let decision = tool_call_decision(
                step,
                &tool_schema_json,
                &context_summary,
                &tool_history,
                decision_lm.clone(),
            )
            .await
            .unwrap_or_else(|_| fallback_tool_call(step, plan, &context_plan));

            if decision.needs_user_input {
                emit_user_input_request(&ui, &decision.user_question);
                success = false;
                step_status = StepStatus::Failed;
                break;
            }

            let tool_id = format!("tc_{}", Uuid::new_v4());
            let params_hash = hash_value(&decision.params)?;

            emit_tool_call(&ui, &tool_id, &decision.tool, &decision.params);
            replay
                .emit(&ReplayEvent::ToolCall {
                    t: Utc::now(),
                    id: tool_id.clone(),
                    tool: decision.tool.clone(),
                    params: decision.params.clone(),
                    params_hash: params_hash.clone(),
                    step_id: step.id.clone(),
                })
                .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

            let start = Instant::now();
            let exec_result = tools.execute_named(&decision.tool, &decision.params).await;
            let latency_ms = start.elapsed().as_millis() as u64;
            total_latency_ms = total_latency_ms.saturating_add(latency_ms);
            tool_call_count = tool_call_count.saturating_add(1);

            let exec_result = match exec_result {
                Ok(result) => result,
                Err(e) => ToolExecutionResult {
                    output: crate::tools::ToolOutput::failure(e.to_string()),
                    exit_code: None,
                    side_effects: Vec::new(),
                },
            };

            for effect in &exec_result.side_effects {
                if let Some(path) = &effect.path {
                    modified_files.insert(path.clone());
                }
            }

            let output_hash = hash_tool_output(&exec_result.output)?;
            let step_utility = tool_result_interpretation(
                step,
                &decision,
                &exec_result,
                decision_lm.clone(),
            )
            .await
            .unwrap_or_else(|| fallback_tool_result(step, &decision.tool, &exec_result));

            let step_utility_clamped = step_utility
                .step_utility
                .clamp(-1.0, 1.0);

            receipts.push(ToolReceipt {
                id: tool_id.clone(),
                tool: decision.tool.clone(),
                params_hash: params_hash.clone(),
                output_hash: output_hash.clone(),
                step_utility: step_utility_clamped,
                latency_ms,
                side_effects: exec_result.side_effects.clone(),
            });

            replay
                .emit(&ReplayEvent::ToolResult {
                    t: Utc::now(),
                    id: tool_id.clone(),
                    output_hash: output_hash.clone(),
                    exit_code: exec_result.exit_code,
                    step_utility: step_utility_clamped,
                    latency_ms,
                })
                .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

            emit_tool_result(&ui, &tool_id, &exec_result);

            tool_history.push(ToolHistoryEntry {
                step_id: step.id.clone(),
                tool: decision.tool.clone(),
                success: exec_result.output.success,
                summary: truncate_for_display(&exec_result.output.content, 280),
            });

            if step_utility.should_continue {
                step_done = false;
            } else {
                step_done = true;
                if !exec_result.output.success && step_utility.success == "no" {
                    step_status = StepStatus::Failed;
                    success = false;
                }
            }
        }

        replay
            .emit(&ReplayEvent::StepComplete {
                t: Utc::now(),
                step_id: step.id.clone(),
                status: step_status,
                iterations,
            })
            .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

        emit_step_complete(&ui, step_index + 1, step_status_is_success(&step_status));
        if !per_step_mode {
            let next_active = if step_index + 1 < plan_ir.steps.len() {
                Some(step_index + 1)
            } else {
                Some(plan_ir.steps.len())
            };
            emit_plan_update(&ui, &plan_ir, next_active);
        }
        step_outcomes.push(format!(
            "{}: {:?} (iterations {})",
            step.description, step_status, iterations
        ));
    }

    let verification = if config.verify_completion {
        run_verification(tools, &plan_ir.verification_strategy).await
    } else {
        VerificationReceipt {
            commands_run: Vec::new(),
            exit_codes: Vec::new(),
            verification_delta: 0,
        }
    };

    replay
        .emit(&ReplayEvent::Verification {
            t: Utc::now(),
            commands: verification.commands_run.clone(),
            exit_codes: verification.exit_codes.clone(),
            verification_delta: verification.verification_delta,
        })
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

    let verification_passed = verification.exit_codes.iter().all(|code| *code == 0);
    if !verification_passed {
        success = false;
    }

    let completed_at = Utc::now();
    replay
        .emit(&ReplayEvent::SessionEnd {
            t: completed_at,
            status: if success {
                SessionStatus::Success
            } else {
                SessionStatus::Failed
            },
            confidence: plan_ir.confidence,
            total_tool_calls: tool_call_count,
            total_latency_ms,
        })
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;

    let summary = build_pr_summary(task, &plan_ir, &step_outcomes, &modified_files, &verification);
    let summary_path = session_dir.join("PR_SUMMARY.md");
    std::fs::write(&summary_path, summary.clone()).map_err(AdjutantError::IoError)?;

    let receipt = SessionReceipt {
        session_id: session_id.clone(),
        started_at: started_at.to_rfc3339(),
        completed_at: completed_at.to_rfc3339(),
        issue_number: parse_issue_number(&task.id),
        plan_hash,
        tool_calls: receipts,
        verification: verification.clone(),
        final_confidence: plan_ir.confidence,
        policy_bundle_id,
        signature: None,
    };

    let receipt_path = session_dir.join("RECEIPT.json");
    let receipt_json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?;
    std::fs::write(&receipt_path, receipt_json).map_err(AdjutantError::IoError)?;

    Ok(crate::executor::TaskResult {
        success,
        summary,
        modified_files: modified_files.into_iter().collect(),
        commit_hash: None,
        error: if success {
            None
        } else if !verification_passed {
            Some("Verification failed".to_string())
        } else {
            Some("Step failures detected".to_string())
        },
        session_id: Some(session_id),
    })
}

fn resolve_policy_bundle_id() -> String {
    std::env::var("OPENAGENTS_POLICY_BUNDLE_ID").unwrap_or_else(|_| "local-dev".to_string())
}

fn parse_issue_number(task_id: &str) -> Option<i64> {
    task_id.parse::<i64>().ok()
}

fn step_status_is_success(status: &StepStatus) -> bool {
    matches!(status, StepStatus::Success)
}

fn emit_planning_stage(ui: &Option<CodingAgentUi>, plan: &PlanIR) {
    let Some(ui) = ui else { return };
    let Some(sender) = &ui.acp_sender else { return };

    let files_to_modify = plan
        .steps
        .iter()
        .flat_map(|step| step.target_files.iter().cloned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let test_strategy = if plan.verification_strategy.commands.is_empty() {
        plan.verification_strategy.success_criteria.clone()
    } else {
        format!(
            "{} (commands: {})",
            plan.verification_strategy.success_criteria,
            plan.verification_strategy.commands.join(", ")
        )
    };

    let stage = DspyStage::Planning {
        analysis: plan.analysis.clone(),
        files_to_modify,
        implementation_steps: plan
            .steps
            .iter()
            .map(|step| step.description.clone())
            .collect(),
        test_strategy,
        complexity: format!("{:?}", plan.complexity),
        confidence: plan.confidence,
    };
    emit_stage(sender, stage);
}

fn emit_plan_update(ui: &Option<CodingAgentUi>, plan: &PlanIR, active_step: Option<usize>) {
    let Some(ui) = ui else { return };
    let Some(sender) = &ui.acp_sender else { return };

    let entries = plan
        .steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let status = match active_step {
                Some(active) if active == idx => acp::PlanEntryStatus::InProgress,
                Some(active) if active > idx => acp::PlanEntryStatus::Completed,
                _ => acp::PlanEntryStatus::Pending,
            };
            acp::PlanEntry::new(
                format!("{}. {}", idx + 1, step.description),
                acp::PlanEntryPriority::Medium,
                status,
            )
        })
        .collect::<Vec<_>>();

    let mut meta = acp::Meta::new();
    meta.insert(
        SESSION_ID_META_KEY.to_string(),
        serde_json::Value::String(sender.session_id.to_string()),
    );
    let plan_update = acp::Plan::new(entries).meta(meta);
    sender.send_update(acp::SessionUpdate::Plan(plan_update));
}

fn emit_step_start(ui: &Option<CodingAgentUi>, idx: usize, total: usize, desc: String) {
    let Some(ui) = ui else { return };
    if let Some(sender) = &ui.acp_sender {
        emit_stage(
            sender,
            DspyStage::ExecutingTask {
                task_index: idx,
                total_tasks: total,
                task_description: desc,
            },
        );
    }
}

fn emit_step_complete(ui: &Option<CodingAgentUi>, idx: usize, success: bool) {
    let Some(ui) = ui else { return };
    if let Some(sender) = &ui.acp_sender {
        emit_stage(sender, DspyStage::TaskComplete { task_index: idx, success });
    }
}

fn emit_user_input_request(ui: &Option<CodingAgentUi>, question: &str) {
    let Some(ui) = ui else { return };
    if let Some(tx) = &ui.token_tx {
        let _ = tx.send(format!("\n\nUser input required: {}\n", question));
    }
}

fn emit_stage(sender: &AcpEventSender, stage: DspyStage) {
    let Ok(stage_value) = serde_json::to_value(stage) else {
        return;
    };
    let mut meta = acp::Meta::new();
    meta.insert(DSPY_META_KEY.to_string(), stage_value);
    meta.insert(
        SESSION_ID_META_KEY.to_string(),
        serde_json::Value::String(sender.session_id.to_string()),
    );
    let content = acp::TextContent::new("").meta(meta);
    let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(content));
    sender.send_update(acp::SessionUpdate::AgentThoughtChunk(chunk));
}

fn emit_tool_call(ui: &Option<CodingAgentUi>, tool_id: &str, tool: &str, params: &Value) {
    let Some(ui) = ui else { return };
    let Some(sender) = &ui.acp_sender else { return };

    let mut meta = acp::Meta::new();
    meta.insert(
        SESSION_ID_META_KEY.to_string(),
        serde_json::Value::String(sender.session_id.to_string()),
    );
    let tool_event = acp::ToolCall::new(acp::ToolCallId::new(tool_id), tool)
        .kind(tool_kind_for_name(tool))
        .status(acp::ToolCallStatus::InProgress)
        .raw_input(params.clone())
        .meta(meta);
    sender.send_update(acp::SessionUpdate::ToolCall(tool_event));
}

fn emit_tool_result(ui: &Option<CodingAgentUi>, tool_id: &str, result: &ToolExecutionResult) {
    let Some(ui) = ui else { return };
    let Some(sender) = &ui.acp_sender else { return };

    let status = if result.output.success {
        acp::ToolCallStatus::Completed
    } else {
        acp::ToolCallStatus::Failed
    };
    let raw_output = if result.output.success {
        json!({
            "content": truncate_for_display(&result.output.content, 4000),
            "success": true
        })
    } else {
        json!({
            "error": result.output.error.clone().unwrap_or_default(),
            "stdout": truncate_for_display(&result.output.content, 4000),
            "success": false
        })
    };

    let mut fields = acp::ToolCallUpdateFields::new();
    fields = fields.status(status).raw_output(raw_output);

    let mut meta = acp::Meta::new();
    meta.insert(
        SESSION_ID_META_KEY.to_string(),
        serde_json::Value::String(sender.session_id.to_string()),
    );
    let update = acp::ToolCallUpdate::new(acp::ToolCallId::new(tool_id), fields).meta(meta);
    sender.send_update(acp::SessionUpdate::ToolCallUpdate(update));
}

fn tool_kind_for_name(name: &str) -> acp::ToolKind {
    match name {
        "read_file" | "glob" | "grep" => acp::ToolKind::Read,
        "edit_file" | "write_file" => acp::ToolKind::Edit,
        "bash" => acp::ToolKind::Execute,
        _ => acp::ToolKind::Other,
    }
}

async fn select_context(
    task: &Task,
    plan: &TaskPlan,
    tools: &mut ToolRegistry,
    workspace_root: &Path,
    decision_lm: Option<Arc<LM>>,
    ui: &Option<CodingAgentUi>,
    context_char_limit: usize,
) -> Result<(ContextPlan, String), AdjutantError> {
    let signature = ContextSelectionSignature::new();
    let inputs = example! {
        "session_summary": "input" => task.description.clone(),
        "recent_turns": "input" => "".to_string(),
        "file_history": "input" => plan.summary.clone(),
        "token_budget": "input" => context_char_limit.to_string(),
        "privacy_mode": "input" => "local".to_string(),
        "lane_constraints": "input" => "default".to_string(),
    };

    let prediction = run_signature(signature, inputs, decision_lm.clone()).await.ok();
    let raw_plan = prediction
        .as_ref()
        .map(|pred| prediction_string(pred, "context_plan"))
        .unwrap_or_default();

    let context_plan: ContextPlan = serde_json::from_str(&raw_plan).unwrap_or(ContextPlan {
        include_paths: None,
        exclude_paths: None,
        include_summaries: None,
        notes: None,
    });

    let include_paths = context_plan
        .include_paths
        .clone()
        .unwrap_or_else(|| {
            plan.files
                .iter()
                .take(10)
                .map(|path| path.to_string_lossy().to_string())
                .collect()
        });
    let exclude = context_plan
        .exclude_paths
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();

    let mut context = String::new();
    for path_str in include_paths {
        if exclude.contains(&path_str) {
            continue;
        }
        let path = workspace_root.join(&path_str);
        let output = tools.read(&path).await?;
        if output.success {
            context.push_str(&format!("\n--- {} ---\n{}\n", path_str, output.content));
        }
        if context.len() > context_char_limit {
            context = truncate_for_display(&context, context_char_limit);
            break;
        }
    }

    if let Some(ui) = ui {
        if let Some(tx) = &ui.token_tx {
            let note = context_plan
                .notes
                .clone()
                .unwrap_or_else(|| "context selection".to_string());
            let _ = tx.send(format!("\n\nContext selection: {}\n", note));
        }
    }

    Ok((context_plan, context))
}

async fn plan_from_signature(
    task: &Task,
    plan: &TaskPlan,
    context_summary: &str,
    decision_lm: Option<Arc<LM>>,
) -> PlanIR {
    let signature = PlanningSignature::new();
    let file_tree = plan
        .files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    let inputs = example! {
        "task_description": "input" => task.description.clone(),
        "repo_context": "input" => plan.summary.clone(),
        "file_tree": "input" => file_tree,
        "context_summary": "input" => truncate_for_display(context_summary, 4000),
        "constraints": "input" => "default".to_string(),
    };

    let prediction = match run_signature(signature, inputs, decision_lm).await {
        Ok(prediction) => prediction,
        Err(_) => return PlanIR {
            analysis: plan.summary.clone(),
            steps: Vec::new(),
            verification_strategy: VerificationStrategy {
                commands: Vec::new(),
                success_criteria: "No verification strategy provided".to_string(),
                max_retries: 0,
            },
            complexity: map_complexity(plan.complexity),
            confidence: 0.5,
        },
    };

    parse_plan_ir(&prediction).unwrap_or_else(|| PlanIR {
        analysis: plan.summary.clone(),
        steps: Vec::new(),
        verification_strategy: VerificationStrategy {
            commands: Vec::new(),
            success_criteria: "No verification strategy provided".to_string(),
            max_retries: 0,
        },
        complexity: map_complexity(plan.complexity),
        confidence: 0.5,
    })
}

fn parse_plan_ir(prediction: &Prediction) -> Option<PlanIR> {
    let analysis = prediction_string(prediction, "analysis");
    let steps_raw = prediction_string(prediction, "steps");
    let verification_raw = prediction_string(prediction, "verification_strategy");
    let complexity_raw = prediction_string(prediction, "complexity");
    let confidence = prediction_f32(prediction, "confidence", 0.5);

    let steps = parse_steps(&steps_raw);
    let verification_strategy =
        serde_json::from_str::<VerificationStrategy>(&verification_raw).unwrap_or(
            VerificationStrategy {
                commands: Vec::new(),
                success_criteria: "No verification strategy provided".to_string(),
                max_retries: 0,
            },
        );

    Some(PlanIR {
        analysis,
        steps,
        verification_strategy,
        complexity: parse_complexity(&complexity_raw),
        confidence,
    })
}

fn parse_steps(raw: &str) -> Vec<PlanStep> {
    let parsed = serde_json::from_str::<Value>(raw).unwrap_or(Value::Null);
    if let Value::Array(items) = parsed {
        let mut steps = Vec::new();
        for (idx, item) in items.iter().enumerate() {
            if let Value::Object(map) = item {
                let id = map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&format!("step-{}", idx + 1))
                    .to_string();
                let description = map
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Step")
                    .to_string();
                let intent = map
                    .get("intent")
                    .and_then(|v| v.as_str())
                    .map(parse_step_intent)
                    .unwrap_or(StepIntent::Modify);
                let target_files = map
                    .get("target_files")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let depends_on = map
                    .get("depends_on")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let max_iterations = map
                    .get("max_iterations")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.min(10) as u8)
                    .unwrap_or(3);
                steps.push(PlanStep {
                    id,
                    description,
                    intent,
                    target_files,
                    depends_on,
                    max_iterations,
                });
            }
        }
        return steps;
    }

    raw.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(PlanStep {
                    id: format!("step-{}", idx + 1),
                    description: trimmed.to_string(),
                    intent: StepIntent::Modify,
                    target_files: Vec::new(),
                    depends_on: Vec::new(),
                    max_iterations: 3,
                })
            }
        })
        .collect()
}

fn parse_complexity(raw: &str) -> PlanComplexity {
    match raw.trim().to_lowercase().as_str() {
        "low" => PlanComplexity::Low,
        "high" => PlanComplexity::High,
        "veryhigh" | "very_high" | "very-high" => PlanComplexity::VeryHigh,
        _ => PlanComplexity::Medium,
    }
}

fn parse_step_intent(raw: &str) -> StepIntent {
    match raw.trim().to_lowercase().as_str() {
        "investigate" => StepIntent::Investigate,
        "verify" => StepIntent::Verify,
        "synthesize" => StepIntent::Synthesize,
        _ => StepIntent::Modify,
    }
}

fn map_complexity(complexity: crate::planner::Complexity) -> PlanComplexity {
    match complexity {
        crate::planner::Complexity::Low => PlanComplexity::Low,
        crate::planner::Complexity::Medium => PlanComplexity::Medium,
        crate::planner::Complexity::High => PlanComplexity::High,
        crate::planner::Complexity::VeryHigh => PlanComplexity::VeryHigh,
    }
}

fn fallback_plan_ir(task: &Task, plan: &TaskPlan, max_iterations: u8) -> PlanIR {
    PlanIR {
        analysis: plan.summary.clone(),
        steps: vec![PlanStep {
            id: "step-1".to_string(),
            description: task.description.clone(),
            intent: StepIntent::Modify,
            target_files: plan
                .files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            depends_on: Vec::new(),
            max_iterations,
        }],
        verification_strategy: VerificationStrategy {
            commands: Vec::new(),
            success_criteria: "No verification commands provided".to_string(),
            max_retries: 0,
        },
        complexity: map_complexity(plan.complexity),
        confidence: 0.5,
    }
}

async fn tool_call_decision(
    step: &PlanStep,
    tool_schemas: &str,
    context_summary: &str,
    history: &[ToolHistoryEntry],
    decision_lm: Option<Arc<LM>>,
) -> Result<ToolCallDecision> {
    let signature = ToolCallSignature::new();
    let history_json = serde_json::to_string(history).unwrap_or_else(|_| "[]".to_string());
    let inputs = example! {
        "step_id": "input" => step.id.clone(),
        "step_description": "input" => step.description.clone(),
        "step_intent": "input" => format!("{:?}", step.intent),
        "tool_schemas": "input" => tool_schemas.to_string(),
        "context_summary": "input" => truncate_for_display(context_summary, 4000),
        "execution_history": "input" => history_json,
    };

    let prediction = run_signature(signature, inputs, decision_lm).await?;

    let tool = prediction_string(&prediction, "tool");
    let params_raw = prediction_string(&prediction, "params");
    let expected_outcome = prediction_string(&prediction, "expected_outcome");
    let progress_estimate = prediction_f32(&prediction, "progress_estimate", 0.0);
    let needs_user_input = prediction_bool(&prediction, "needs_user_input", false);
    let user_question = prediction_string(&prediction, "user_question");
    let confidence = prediction_f32(&prediction, "confidence", 0.5);

    let params = serde_json::from_str::<Value>(&params_raw).unwrap_or_else(|_| json!({}));

    Ok(ToolCallDecision {
        tool,
        params,
        expected_outcome,
        progress_estimate,
        needs_user_input,
        user_question,
        confidence,
    })
}

fn fallback_tool_call(step: &PlanStep, plan: &TaskPlan, _context_plan: &ContextPlan) -> ToolCallDecision {
    let tool = match step.intent {
        StepIntent::Verify => "bash".to_string(),
        _ => "read_file".to_string(),
    };

    let params = if tool == "bash" {
        json!({"command": "cargo test --no-fail-fast"})
    } else {
        let target = step
            .target_files
            .get(0)
            .cloned()
            .or_else(|| plan.files.get(0).map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|| "README.md".to_string());
        json!({"path": target})
    };

    ToolCallDecision {
        tool,
        params,
        expected_outcome: "Gather more context".to_string(),
        progress_estimate: 0.1,
        needs_user_input: false,
        user_question: String::new(),
        confidence: 0.2,
    }
}

async fn tool_result_interpretation(
    step: &PlanStep,
    decision: &ToolCallDecision,
    exec_result: &ToolExecutionResult,
    decision_lm: Option<Arc<LM>>,
) -> Option<ToolResultInterpretation> {
    let signature = ToolResultSignature::new();
    let inputs = example! {
        "step_id": "input" => step.id.clone(),
        "step_description": "input" => step.description.clone(),
        "expected_outcome": "input" => decision.expected_outcome.clone(),
        "tool_name": "input" => decision.tool.clone(),
        "tool_params": "input" => decision.params.to_string(),
        "tool_output": "input" => exec_result.output.content.clone(),
        "tool_error": "input" => exec_result.output.error.clone().unwrap_or_default(),
    };

    let prediction = run_signature(signature, inputs, decision_lm).await.ok()?;

    Some(ToolResultInterpretation {
        success: prediction_string(&prediction, "success"),
        extracted_facts: prediction_string_vec(&prediction, "extracted_facts"),
        should_continue: prediction_bool(&prediction, "should_continue", false),
        step_utility: prediction_f32(&prediction, "step_utility", 0.0),
        confidence: prediction_f32(&prediction, "confidence", 0.5),
    })
}

fn fallback_tool_result(
    step: &PlanStep,
    tool_name: &str,
    exec_result: &ToolExecutionResult,
) -> ToolResultInterpretation {
    let success = if exec_result.output.success {
        "yes".to_string()
    } else {
        "no".to_string()
    };
    let should_continue = exec_result.output.success
        && matches!(step.intent, StepIntent::Investigate)
        && matches!(tool_name, "read_file" | "grep" | "glob");
    let step_utility = if exec_result.output.success { 0.2 } else { -0.4 };

    ToolResultInterpretation {
        success,
        extracted_facts: Vec::new(),
        should_continue,
        step_utility,
        confidence: 0.3,
    }
}

async fn run_verification(
    tools: &ToolRegistry,
    strategy: &VerificationStrategy,
) -> VerificationReceipt {
    let mut commands_run = Vec::new();
    let mut exit_codes = Vec::new();

    for command in &strategy.commands {
        let output = tools.bash(command).await;
        commands_run.push(command.clone());
        match output {
            Ok(result) => {
                exit_codes.push(if result.success { 0 } else { 1 });
            }
            Err(_) => exit_codes.push(1),
        }
    }

    VerificationReceipt {
        commands_run,
        exit_codes,
        verification_delta: 0,
    }
}

fn build_pr_summary(
    task: &Task,
    plan: &PlanIR,
    outcomes: &[String],
    modified_files: &HashSet<String>,
    verification: &VerificationReceipt,
) -> String {
    let mut summary = String::new();
    summary.push_str("# Summary\n\n");
    summary.push_str(&format!("Task: {}\n\n", task.title));
    summary.push_str("## Plan\n");
    summary.push_str(&format!("{}\n\n", plan.analysis));
    for (idx, step) in plan.steps.iter().enumerate() {
        summary.push_str(&format!("{}. {}\n", idx + 1, step.description));
    }
    summary.push_str("\n## Outcomes\n");
    for outcome in outcomes {
        summary.push_str(&format!("- {}\n", outcome));
    }
    summary.push_str("\n## Verification\n");
    if verification.commands_run.is_empty() {
        summary.push_str("- No verification commands run\n");
    } else {
        for (cmd, code) in verification.commands_run.iter().zip(verification.exit_codes.iter()) {
            summary.push_str(&format!("- `{}` -> exit {}\n", cmd, code));
        }
    }
    summary.push_str("\n## Modified Files\n");
    if modified_files.is_empty() {
        summary.push_str("- None\n");
    } else {
        for file in modified_files.iter() {
            summary.push_str(&format!("- {}\n", file));
        }
    }
    summary
}

fn hash_value(value: &Value) -> Result<String, AdjutantError> {
    Ok(format!(
        "sha256:{}",
        canonical_hash(value).map_err(|e| AdjutantError::ExecutionFailed(e.to_string()))?
    ))
}

fn hash_tool_output(output: &crate::tools::ToolOutput) -> Result<String, AdjutantError> {
    let value = json!({
        "success": output.success,
        "content": output.content,
        "error": output.error,
    });
    hash_value(&value)
}

fn truncate_for_display(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let head = &text[..limit / 2];
    let tail = &text[text.len() - (limit / 2)..];
    format!("{}...{}", head, tail)
}

async fn run_signature<S: dsrs::core::signature::MetaSignature + 'static>(
    signature: S,
    input: Example,
    lm: Option<Arc<LM>>,
) -> Result<Prediction> {
    let predictor = Predict::new(signature);
    if let Some(lm) = lm {
        predictor.forward_with_config(input, lm).await
    } else {
        predictor.forward(input).await
    }
}

fn prediction_string(prediction: &Prediction, key: &str) -> String {
    let value = prediction.get(key, None);
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string().trim_matches('"').to_string()
    }
}

fn prediction_string_vec(prediction: &Prediction, key: &str) -> Vec<String> {
    let value = prediction.get(key, None);
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }
    if let Some(s) = value.as_str() {
        return serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| {
            s.lines()
                .map(|line| line.trim().trim_start_matches('-').to_string())
                .filter(|line| !line.is_empty())
                .collect()
        });
    }
    Vec::new()
}

fn prediction_bool(prediction: &Prediction, key: &str, default: bool) -> bool {
    let value = prediction.get(key, None);
    value.as_bool().unwrap_or(default)
}

fn prediction_f32(prediction: &Prediction, key: &str, default: f32) -> f32 {
    let value = prediction.get(key, None);
    value.as_f64().map(|v| v as f32).unwrap_or(default)
}
