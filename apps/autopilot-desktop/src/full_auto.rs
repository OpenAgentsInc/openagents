use autopilot_core::guidance::{
    GuidanceAction, GuidanceDecision, GuidanceDecisionDiagnostics, GuidanceDecisionResult,
    GuidanceGoal, GuidanceGuardrailConfig, GuidanceGuardrailContext, GuidanceInputs, GuidanceMode,
    GuidanceNetwork, GuidancePermissions, GuidanceState, apply_guidance_guardrails,
    guidance_demo_model,
};
use chrono::Utc;
use dsrs::signatures::FullAutoDecisionSignature;
use dsrs::{LM, Predict, Predictor, example};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const DEFAULT_CONTINUE_PROMPT: &str = "Continue immediately. Do not ask for confirmation or pause. If errors occur, recover and keep going.";

const ENV_DECISION_MODEL: &str = "OPENAGENTS_FULL_AUTO_DECISION_MODEL";
const ENV_MAX_TOKENS: &str = "OPENAGENTS_FULL_AUTO_MAX_TOKENS";
const ENV_MAX_TURNS: &str = "OPENAGENTS_FULL_AUTO_MAX_TURNS";
const ENV_NO_PROGRESS_LIMIT: &str = "OPENAGENTS_FULL_AUTO_NO_PROGRESS_LIMIT";
const ENV_MIN_CONFIDENCE: &str = "OPENAGENTS_FULL_AUTO_MIN_CONFIDENCE";
const ENV_GUIDANCE_GOAL: &str = "OPENAGENTS_GUIDANCE_GOAL";
const DEFAULT_GUIDANCE_GOAL_INTENT: &str =
    "Keep making progress on the current task using the latest plan and diff.";

#[derive(Clone, Debug)]
pub struct FullAutoConfig {
    pub min_confidence: f32,
    pub max_turns: u64,
    pub no_progress_limit: u32,
    pub max_tokens: Option<u64>,
}

impl Default for FullAutoConfig {
    fn default() -> Self {
        Self {
            min_confidence: read_env_f32(ENV_MIN_CONFIDENCE).unwrap_or(0.55),
            max_turns: read_env_u64(ENV_MAX_TURNS).unwrap_or(200),
            no_progress_limit: read_env_u32(ENV_NO_PROGRESS_LIMIT).unwrap_or(3),
            max_tokens: read_env_u64(ENV_MAX_TOKENS),
        }
    }
}

pub type FullAutoAction = GuidanceAction;
pub type FullAutoDecision = GuidanceDecision;
pub type FullAutoDecisionDiagnostics = GuidanceDecisionDiagnostics;
pub type FullAutoDecisionResult = GuidanceDecisionResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FullAutoDecisionRecord {
    pub turn_id: Option<String>,
    pub action: String,
    pub reason: String,
    pub confidence: f32,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct FullAutoThreadState {
    pub last_turn_id: Option<String>,
    pub last_turn_status: Option<String>,
    pub last_turn_error: Option<String>,
    pub last_agent_message: Option<String>,
    pub last_guidance_directive: Option<String>,
    pub plan_snapshot: Option<Value>,
    pub diff_snapshot: Option<Value>,
    pub token_usage: Option<Value>,
    pub pending_approvals: u32,
    pub pending_tool_inputs: u32,
    pub compaction_events: u32,
    pub no_progress_count: u32,
    pub last_progress_signature: Option<String>,
    pub turn_count: u64,
    pub recent_actions: Vec<FullAutoDecisionRecord>,
}

impl Default for FullAutoThreadState {
    fn default() -> Self {
        Self {
            last_turn_id: None,
            last_turn_status: None,
            last_turn_error: None,
            last_agent_message: None,
            last_guidance_directive: None,
            plan_snapshot: None,
            diff_snapshot: None,
            token_usage: None,
            pending_approvals: 0,
            pending_tool_inputs: 0,
            compaction_events: 0,
            no_progress_count: 0,
            last_progress_signature: None,
            turn_count: 0,
            recent_actions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FullAutoTurnSummary {
    pub thread_id: String,
    pub turn_id: String,
    pub last_turn_status: String,
    pub turn_error: String,
    pub last_agent_message: String,
    pub last_guidance_directive: String,
    pub turn_plan: String,
    pub diff_summary: String,
    pub token_usage: String,
    pub pending_approvals: String,
    pub pending_tool_inputs: String,
    pub recent_actions: String,
    pub compaction_events: String,
    pub turn_count: u64,
    pub no_progress_count: u32,
}

impl FullAutoTurnSummary {
    pub fn to_example(&self) -> dsrs::Example {
        example! {
            "thread_id": "input" => self.thread_id.clone(),
            "turn_id": "input" => self.turn_id.clone(),
            "last_turn_status": "input" => self.last_turn_status.clone(),
            "turn_error": "input" => self.turn_error.clone(),
            "turn_plan": "input" => self.turn_plan.clone(),
            "diff_summary": "input" => self.diff_summary.clone(),
            "token_usage": "input" => self.token_usage.clone(),
            "pending_approvals": "input" => self.pending_approvals.clone(),
            "pending_tool_inputs": "input" => self.pending_tool_inputs.clone(),
            "recent_actions": "input" => self.recent_actions.clone(),
            "compaction_events": "input" => self.compaction_events.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FullAutoDecisionRequest {
    pub summary: FullAutoTurnSummary,
    pub fallback_prompt: String,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Clone)]
pub struct FullAutoState {
    pub enabled: bool,
    pub thread_id: Option<String>,
    pub continue_prompt: String,
    pub config: FullAutoConfig,
    pub guidance_mode: GuidanceMode,
    pub guidance_goal: GuidanceGoal,
    pub guidance_activated: bool,
    decision_lm: Option<LM>,
    threads: HashMap<String, FullAutoThreadState>,
    pub run_id: String,
    pub decision_seq: u64,
}

impl FullAutoState {
    pub fn new(
        workspace_id: &str,
        thread_id: Option<String>,
        continue_prompt: Option<String>,
    ) -> Self {
        let config = FullAutoConfig::default();
        let run_id = format!("fullauto-{}", Uuid::new_v4());
        let started_at = Utc::now();
        let guidance_mode = GuidanceMode::from_env();
        let guidance_goal = guidance_goal_from_env();
        let guidance_activated = false;
        let _ = json!({
            "runId": run_id,
            "workspaceId": workspace_id,
            "threadId": thread_id,
            "startedAt": started_at.to_rfc3339(),
            "decisionModel": decision_model(),
            "guidanceMode": format!("{:?}", guidance_mode),
            "guidanceModel": if guidance_mode == GuidanceMode::Demo { guidance_demo_model() } else { String::new() },
            "minConfidence": config.min_confidence,
            "maxTurns": config.max_turns,
            "noProgressLimit": config.no_progress_limit,
            "maxTokens": config.max_tokens,
        });
        Self {
            enabled: true,
            thread_id,
            continue_prompt: normalize_prompt(continue_prompt),
            config,
            guidance_mode,
            guidance_goal,
            guidance_activated,
            decision_lm: None,
            threads: HashMap::new(),
            run_id,
            decision_seq: 0,
        }
    }

    pub fn matches_thread(&self, thread_id: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }
        match (&self.thread_id, thread_id) {
            (Some(expected), Some(actual)) => expected == actual,
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    pub fn adopt_thread(&mut self, thread_id: &str) {
        if self.thread_id.is_none() {
            self.thread_id = Some(thread_id.to_string());
        }
        self.threads.entry(thread_id.to_string()).or_default();
    }

    pub fn next_decision_sequence(&mut self) -> u64 {
        self.decision_seq = self.decision_seq.saturating_add(1);
        self.decision_seq
    }

    pub fn set_continue_prompt(&mut self, prompt: Option<String>) {
        if prompt.is_some() {
            self.continue_prompt = normalize_prompt(prompt);
        }
    }

    pub fn record_event(
        &mut self,
        method: &str,
        params: Option<&Value>,
        thread_id: Option<&str>,
        _turn_id: Option<&str>,
    ) {
        let thread_id = match thread_id {
            Some(thread_id) => thread_id,
            None => return,
        };
        self.adopt_thread(thread_id);
        let state = self.threads.entry(thread_id.to_string()).or_default();

        match method {
            "thread/compacted" => {
                state.compaction_events = state.compaction_events.saturating_add(1);
            }
            "turn/started" => {
                state.pending_approvals = 0;
                state.pending_tool_inputs = 0;
            }
            "turn/plan/updated" => {
                if let Some(params) = params {
                    state.plan_snapshot = Some(params.clone());
                }
            }
            "turn/diff/updated" => {
                if let Some(params) = params {
                    state.diff_snapshot = Some(params.clone());
                }
            }
            "thread/tokenUsage/updated" => {
                if let Some(params) = params {
                    state.token_usage = Some(params.clone());
                }
            }
            "turn/error" => {
                state.last_turn_error = params
                    .and_then(read_error)
                    .or_else(|| Some("turn error".to_string()));
            }
            "item/agentMessage/completed" | "item/assistantMessage/completed" => {
                if let Some(message) = params.and_then(read_agent_message) {
                    state.last_agent_message = Some(message);
                }
            }
            "item/completed" => {
                if let Some(message) = params.and_then(read_agent_message_from_item) {
                    state.last_agent_message = Some(message);
                }
            }
            "turn/completed" => {
                state.last_turn_status = params.and_then(read_turn_status);
                state.last_turn_error = params.and_then(read_turn_error);
            }
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                state.pending_approvals = state.pending_approvals.saturating_add(1);
            }
            "item/tool/requestUserInput" => {
                state.pending_tool_inputs = state.pending_tool_inputs.saturating_add(1);
            }
            _ => {}
        }
    }

    pub fn prepare_decision(
        &mut self,
        thread_id: Option<&str>,
        turn_id: Option<&str>,
    ) -> Option<FullAutoDecisionRequest> {
        if !self.enabled {
            return None;
        }
        if !self.matches_thread(thread_id) {
            return None;
        }
        let thread_id = thread_id?;
        let turn_id = turn_id?;

        let state = self.threads.entry(thread_id.to_string()).or_default();
        if state.last_turn_id.as_deref() == Some(turn_id) {
            return None;
        }

        state.turn_count = state.turn_count.saturating_add(1);
        state.last_turn_id = Some(turn_id.to_string());
        let progress_signature = build_progress_signature(state);
        if let Some(signature) = progress_signature {
            if state.last_progress_signature.as_deref() == Some(signature.as_str()) {
                state.no_progress_count = state.no_progress_count.saturating_add(1);
            } else {
                state.no_progress_count = 0;
                state.last_progress_signature = Some(signature);
            }
        }

        let summary = FullAutoTurnSummary {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            last_turn_status: state
                .last_turn_status
                .clone()
                .unwrap_or_else(|| "completed".to_string()),
            turn_error: state.last_turn_error.clone().unwrap_or_else(String::new),
            last_agent_message: state
                .last_agent_message
                .clone()
                .unwrap_or_else(String::new),
            last_guidance_directive: state
                .last_guidance_directive
                .clone()
                .unwrap_or_else(String::new),
            turn_plan: value_to_string(state.plan_snapshot.as_ref()),
            diff_summary: value_to_string(state.diff_snapshot.as_ref()),
            token_usage: value_to_string(state.token_usage.as_ref()),
            pending_approvals: format!("{}", state.pending_approvals),
            pending_tool_inputs: format!("{}", state.pending_tool_inputs),
            recent_actions: serialize_recent_actions(&state.recent_actions),
            compaction_events: format!("{}", state.compaction_events),
            turn_count: state.turn_count,
            no_progress_count: state.no_progress_count,
        };

        Some(FullAutoDecisionRequest {
            summary,
            fallback_prompt: self.continue_prompt.clone(),
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
        })
    }

    pub fn decision_lm(&self) -> Option<LM> {
        self.decision_lm.clone()
    }

    pub fn set_decision_lm(&mut self, lm: LM) {
        self.decision_lm = Some(lm);
    }

    pub fn record_guidance_directive(&mut self, thread_id: &str, directive: String) {
        let state = self.threads.entry(thread_id.to_string()).or_default();
        state.last_guidance_directive = Some(directive);
    }

    pub fn guidance_mode(&self) -> GuidanceMode {
        self.guidance_mode
    }

    pub fn guidance_goal_intent(&self) -> String {
        self.guidance_goal.intent.clone()
    }

    pub fn activate_guidance_mode(&mut self) -> bool {
        if self.guidance_activated {
            return false;
        }
        self.guidance_activated = true;
        self.guidance_mode = GuidanceMode::Demo;
        self.decision_lm = None;
        true
    }

    #[allow(dead_code)]
    pub fn build_guidance_inputs(&self, summary: &FullAutoTurnSummary) -> GuidanceInputs {
        let tokens_used = parse_total_tokens(&summary.token_usage);
        let tokens_remaining = self
            .config
            .max_tokens
            .and_then(|max| tokens_used.map(|used| max.saturating_sub(used)));
        let permissions = GuidancePermissions::new(true, true, GuidanceNetwork::Full);
        let mut state =
            GuidanceState::new(summary.turn_count, summary.no_progress_count, permissions);
        state.tokens_used = tokens_used;
        state.tokens_remaining = tokens_remaining;
        GuidanceInputs {
            goal: self.guidance_goal.clone(),
            summary: serde_json::to_value(summary).unwrap_or(Value::Null),
            state,
        }
    }

    pub fn apply_decision(&mut self, thread_id: &str, decision: &FullAutoDecision) {
        if let Some(state) = self.threads.get_mut(thread_id) {
            let record = FullAutoDecisionRecord {
                turn_id: state.last_turn_id.clone(),
                action: decision.action.as_str().to_string(),
                reason: decision.reason.clone(),
                confidence: decision.confidence,
                timestamp: now_unix_secs(),
            };
            state.recent_actions.push(record);
            if state.recent_actions.len() > 5 {
                let excess = state.recent_actions.len() - 5;
                state.recent_actions.drain(0..excess);
            }
        }
    }

    pub fn enforce_guardrails(
        &self,
        thread_id: &str,
        summary: &FullAutoTurnSummary,
        decision: FullAutoDecision,
    ) -> FullAutoDecision {
        let state = self.threads.get(thread_id);
        let turn_count = state.map(|s| s.turn_count).unwrap_or(summary.turn_count);
        let no_progress = state
            .map(|s| s.no_progress_count)
            .unwrap_or(summary.no_progress_count);
        let tokens_used = parse_total_tokens(&summary.token_usage);
        let config = GuidanceGuardrailConfig {
            min_confidence: self.config.min_confidence,
            max_turns: self.config.max_turns,
            no_progress_limit: self.config.no_progress_limit,
            max_tokens: self.config.max_tokens,
        };
        let context = GuidanceGuardrailContext {
            last_turn_status: summary.last_turn_status.clone(),
            turn_count,
            no_progress_count: no_progress,
            tokens_used,
        };

        apply_guidance_guardrails(&context, &config, decision)
    }
}

pub async fn run_full_auto_decision(
    summary: &FullAutoTurnSummary,
    lm: &LM,
) -> Result<FullAutoDecisionResult, String> {
    let predictor = Predict::new(FullAutoDecisionSignature::new());
    let inputs = summary.to_example();
    let lm = std::sync::Arc::new(lm.clone());
    let prediction = predictor
        .forward_with_config(inputs, lm)
        .await
        .map_err(|e| format!("Full Auto decision failed: {}", e))?;

    let raw_prediction = serde_json::to_value(&prediction).unwrap_or(Value::Null);
    let mut parse_errors = Vec::new();

    let (action_parsed, action_raw) = read_prediction_string_value(&prediction, "action");
    if action_raw.is_none() {
        parse_errors.push("action missing or not a string".to_string());
    }

    let (next_input_parsed, next_input_raw) =
        read_prediction_string_value(&prediction, "next_input");
    let (reason_parsed, reason_raw) = read_prediction_string_value(&prediction, "reason");
    if reason_raw.is_none() {
        parse_errors.push("reason missing or not a string".to_string());
    }

    let (confidence_parsed, confidence_raw, confidence_error) =
        read_prediction_confidence(&prediction);
    if let Some(error) = confidence_error {
        parse_errors.push(error);
    }

    let decision = FullAutoDecision {
        action: FullAutoAction::from_str(&action_parsed),
        next_input: if next_input_parsed.trim().is_empty() {
            None
        } else {
            Some(next_input_parsed.clone())
        },
        reason: reason_parsed.clone(),
        confidence: confidence_parsed,
        guardrail: None,
    };

    let diagnostics = FullAutoDecisionDiagnostics {
        raw_prediction,
        action_raw,
        next_input_raw,
        reason_raw,
        confidence_raw,
        action_parsed,
        next_input_parsed,
        reason_parsed,
        confidence_parsed,
        parse_errors,
    };

    Ok(FullAutoDecisionResult {
        decision,
        diagnostics,
    })
}

pub async fn ensure_codex_lm(model: &str) -> Result<LM, String> {
    LM::builder()
        .model(model.to_string())
        .temperature(0.2)
        .max_tokens(512)
        .build()
        .await
        .map_err(|e| format!("Failed to build Codex LM: {}", e))
}

pub fn decision_model() -> String {
    env::var(ENV_DECISION_MODEL)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "codex:gpt-5.1-codex-mini".to_string())
}

fn serialize_recent_actions(actions: &[FullAutoDecisionRecord]) -> String {
    if actions.is_empty() {
        return "[]".to_string();
    }
    serde_json::to_string_pretty(actions).unwrap_or_else(|_| "[]".to_string())
}

fn value_to_string(value: Option<&Value>) -> String {
    match value {
        Some(value) => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
        None => "".to_string(),
    }
}

fn read_agent_message(params: &Value) -> Option<String> {
    params
        .get("message")
        .and_then(extract_message_text)
        .or_else(|| params.get("item").and_then(extract_message_text))
}

fn read_agent_message_from_item(params: &Value) -> Option<String> {
    let item = params.get("item")?;
    let item_type = item.get("type").and_then(|value| value.as_str())?;
    if !item_type.contains("agent") && !item_type.contains("assistant") {
        return None;
    }
    extract_message_text(item)
}

fn extract_message_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("content").and_then(|value| value.as_str()) {
        if !text.trim().is_empty() {
            return Some(text.trim().to_string());
        }
    }
    if let Some(text) = value.get("text").and_then(|value| value.as_str()) {
        if !text.trim().is_empty() {
            return Some(text.trim().to_string());
        }
    }
    if let Some(array) = value.get("content").and_then(|value| value.as_array()) {
        let mut parts = Vec::new();
        for item in array {
            if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
                if !text.trim().is_empty() {
                    parts.push(text.trim().to_string());
                }
            }
        }
        if !parts.is_empty() {
            return Some(parts.join("\n"));
        }
    }
    None
}

fn read_prediction_string_value(
    prediction: &dsrs::Prediction,
    key: &str,
) -> (String, Option<String>) {
    let value = prediction.get(key, None);
    match value {
        Value::String(text) => (text.clone(), Some(text)),
        Value::Number(number) => {
            let text = number.to_string();
            (text.clone(), Some(text))
        }
        Value::Bool(value) => {
            let text = value.to_string();
            (text.clone(), Some(text))
        }
        Value::Null => ("".to_string(), None),
        other => (other.to_string(), None),
    }
}

fn read_prediction_confidence(
    prediction: &dsrs::Prediction,
) -> (f32, Option<Value>, Option<String>) {
    let value = prediction.get("confidence", None);
    match value {
        Value::Number(number) => {
            let parsed = number.as_f64().unwrap_or(0.0) as f32;
            (parsed, Some(Value::Number(number)), None)
        }
        Value::String(text) => match text.parse::<f32>() {
            Ok(parsed) => (parsed, Some(Value::String(text)), None),
            Err(_) => (
                0.0,
                Some(Value::String(text)),
                Some("confidence not parseable".to_string()),
            ),
        },
        Value::Null => (0.0, None, Some("confidence missing".to_string())),
        other => (0.0, Some(other), Some("confidence not numeric".to_string())),
    }
}

fn build_progress_signature(state: &FullAutoThreadState) -> Option<String> {
    let plan = value_to_string(state.plan_snapshot.as_ref());
    let diff = value_to_string(state.diff_snapshot.as_ref());
    if plan.trim().is_empty() && diff.trim().is_empty() {
        return None;
    }
    Some(format!("plan:{}\ndiff:{}", plan.trim(), diff.trim()))
}

fn read_turn_status(params: &Value) -> Option<String> {
    if let Some(turn) = params.get("turn") {
        if let Some(status) = turn.get("status").and_then(|v| v.as_str()) {
            return Some(status.to_string());
        }
    }
    params
        .get("status")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn read_turn_error(params: &Value) -> Option<String> {
    params
        .get("error")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            params
                .get("error")
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

fn read_error(params: &Value) -> Option<String> {
    params
        .get("message")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn parse_total_tokens(token_usage: &str) -> Option<u64> {
    let parsed: Value = serde_json::from_str(token_usage).ok()?;
    parsed
        .get("totalTokens")
        .or_else(|| parsed.get("total_tokens"))
        .and_then(|value| value.as_u64())
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn normalize_prompt(prompt: Option<String>) -> String {
    let trimmed = prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    trimmed
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_CONTINUE_PROMPT.to_string())
}

fn guidance_goal_from_env() -> GuidanceGoal {
    let intent = env::var(ENV_GUIDANCE_GOAL)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_GUIDANCE_GOAL_INTENT.to_string());
    GuidanceGoal::new(intent)
}

fn read_env_u64(key: &str) -> Option<u64> {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn read_env_u32(key: &str) -> Option<u32> {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
}

fn read_env_f32(key: &str) -> Option<f32> {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
}
