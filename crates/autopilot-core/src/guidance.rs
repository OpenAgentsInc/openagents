//! Guidance Modules for Full Auto runs.
//!
//! Provides a demo-mode decision pipeline backed by local Ollama, plus shared
//! guardrail enforcement and decision parsing utilities.

use dsrs::signatures::GuidanceDecisionSignature;
use dsrs::{example, Example, LM, Predict, Predictor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

pub const ENV_GUIDANCE_MODE: &str = "OPENAGENTS_GUIDANCE_MODE";
pub const ENV_GUIDANCE_MODEL: &str = "OPENAGENTS_GUIDANCE_MODEL";

const DEFAULT_GUIDANCE_MODEL: &str = "ollama:llama3.2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidanceMode {
    Legacy,
    Demo,
}

impl GuidanceMode {
    pub fn from_env() -> Self {
        let value = env::var(ENV_GUIDANCE_MODE)
            .ok()
            .map(|value| value.trim().to_lowercase());
        match value.as_deref() {
            Some("demo") | Some("ollama") | Some("local") => GuidanceMode::Demo,
            _ => GuidanceMode::Legacy,
        }
    }
}

pub fn guidance_demo_model() -> String {
    let raw = env::var(ENV_GUIDANCE_MODEL)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_GUIDANCE_MODEL.to_string());
    if raw.contains(':') {
        raw
    } else {
        format!("ollama:{}", raw)
    }
}

pub async fn ensure_guidance_demo_lm() -> Result<LM, String> {
    let model = guidance_demo_model();
    LM::builder()
        .model(model)
        .temperature(0.2)
        .max_tokens(512)
        .build()
        .await
        .map_err(|e| format!("Failed to build guidance demo LM: {e}"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceGoal {
    pub intent: String,
    pub success_criteria: Vec<String>,
}

impl GuidanceGoal {
    pub fn new(intent: impl Into<String>) -> Self {
        Self {
            intent: intent.into(),
            success_criteria: Vec::new(),
        }
    }

    pub fn with_success(mut self, success_criteria: Vec<String>) -> Self {
        self.success_criteria = success_criteria;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GuidanceNetwork {
    None,
    Scoped,
    Full,
}

impl GuidanceNetwork {
    pub fn as_str(&self) -> &'static str {
        match self {
            GuidanceNetwork::None => "none",
            GuidanceNetwork::Scoped => "scoped",
            GuidanceNetwork::Full => "full",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidancePermissions {
    pub can_exec: bool,
    pub can_write: bool,
    pub network: GuidanceNetwork,
}

impl GuidancePermissions {
    pub fn new(can_exec: bool, can_write: bool, network: GuidanceNetwork) -> Self {
        Self {
            can_exec,
            can_write,
            network,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceState {
    pub turn_count: u64,
    pub no_progress_count: u32,
    pub tokens_used: Option<u64>,
    pub tokens_remaining: Option<u64>,
    pub time_remaining_ms: Option<u64>,
    pub permissions: GuidancePermissions,
}

impl GuidanceState {
    pub fn new(turn_count: u64, no_progress_count: u32, permissions: GuidancePermissions) -> Self {
        Self {
            turn_count,
            no_progress_count,
            tokens_used: None,
            tokens_remaining: None,
            time_remaining_ms: None,
            permissions,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceInputs {
    pub goal: GuidanceGoal,
    pub summary: Value,
    pub state: GuidanceState,
}

impl GuidanceInputs {
    pub fn to_example(&self) -> Example {
        let summary_json = serde_json::to_string_pretty(&self.summary)
            .unwrap_or_else(|_| self.summary.to_string());
        let state_json = serde_json::to_string_pretty(&self.state)
            .unwrap_or_else(|_| "{}".to_string());
        let success_json = serde_json::to_string_pretty(&self.goal.success_criteria)
            .unwrap_or_else(|_| "[]".to_string());
        example! {
            "goal_intent": "input" => self.goal.intent.clone(),
            "goal_success_criteria": "input" => success_json,
            "summary": "input" => summary_json,
            "state": "input" => state_json,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuidanceAction {
    Continue,
    Pause,
    Stop,
    Review,
}

impl GuidanceAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            GuidanceAction::Continue => "continue",
            GuidanceAction::Pause => "pause",
            GuidanceAction::Stop => "stop",
            GuidanceAction::Review => "review",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "continue" => GuidanceAction::Continue,
            "pause" => GuidanceAction::Pause,
            "stop" => GuidanceAction::Stop,
            "review" => GuidanceAction::Review,
            _ => GuidanceAction::Pause,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceGuardrailAudit {
    pub triggered: bool,
    pub rule: Option<String>,
    pub original_action: String,
    pub original_confidence: f32,
    pub enforced_action: String,
    pub enforced_confidence: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceDecision {
    pub action: GuidanceAction,
    pub next_input: Option<String>,
    pub reason: String,
    pub confidence: f32,
    pub guardrail: Option<GuidanceGuardrailAudit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceDecisionDiagnostics {
    pub raw_prediction: Value,
    pub action_raw: Option<String>,
    pub next_input_raw: Option<String>,
    pub reason_raw: Option<String>,
    pub confidence_raw: Option<Value>,
    pub action_parsed: String,
    pub next_input_parsed: String,
    pub reason_parsed: String,
    pub confidence_parsed: f32,
    pub parse_errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceDecisionResult {
    pub decision: GuidanceDecision,
    pub diagnostics: GuidanceDecisionDiagnostics,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceGuardrailConfig {
    pub min_confidence: f32,
    pub max_turns: u64,
    pub no_progress_limit: u32,
    pub max_tokens: Option<u64>,
}

impl Default for GuidanceGuardrailConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.55,
            max_turns: 200,
            no_progress_limit: 3,
            max_tokens: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceGuardrailContext {
    pub last_turn_status: String,
    pub turn_count: u64,
    pub no_progress_count: u32,
    pub tokens_used: Option<u64>,
}

pub async fn run_guidance_decision(
    inputs: &GuidanceInputs,
    lm: &LM,
) -> Result<GuidanceDecisionResult, String> {
    let predictor = Predict::new(GuidanceDecisionSignature::new());
    let example_inputs = inputs.to_example();
    let lm = std::sync::Arc::new(lm.clone());
    let prediction = predictor
        .forward_with_config(example_inputs, lm)
        .await
        .map_err(|e| format!("Guidance decision failed: {e}"))?;

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

    let decision = GuidanceDecision {
        action: GuidanceAction::from_str(&action_parsed),
        next_input: if next_input_parsed.trim().is_empty() {
            None
        } else {
            Some(next_input_parsed.clone())
        },
        reason: reason_parsed.clone(),
        confidence: confidence_parsed,
        guardrail: None,
    };

    let diagnostics = GuidanceDecisionDiagnostics {
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

    Ok(GuidanceDecisionResult {
        decision,
        diagnostics,
    })
}

pub fn apply_guidance_guardrails(
    context: &GuidanceGuardrailContext,
    config: &GuidanceGuardrailConfig,
    mut decision: GuidanceDecision,
) -> GuidanceDecision {
    let original_action = decision.action.as_str().to_string();
    let original_confidence = decision.confidence;
    let mut guardrail: Option<GuidanceGuardrailAudit> = None;

    if context.last_turn_status == "failed" {
        return GuidanceDecision {
            action: GuidanceAction::Stop,
            next_input: None,
            reason: "Turn failed; stopping Full Auto.".to_string(),
            confidence: 1.0,
            guardrail: Some(GuidanceGuardrailAudit {
                triggered: true,
                rule: Some("turn_failed".to_string()),
                original_action: original_action.clone(),
                original_confidence,
                enforced_action: "stop".to_string(),
                enforced_confidence: 1.0,
            }),
        };
    }

    if context.last_turn_status == "interrupted" {
        return GuidanceDecision {
            action: GuidanceAction::Pause,
            next_input: None,
            reason: "Turn interrupted; pausing Full Auto.".to_string(),
            confidence: 1.0,
            guardrail: Some(GuidanceGuardrailAudit {
                triggered: true,
                rule: Some("turn_interrupted".to_string()),
                original_action: original_action.clone(),
                original_confidence,
                enforced_action: "pause".to_string(),
                enforced_confidence: 1.0,
            }),
        };
    }

    if config.max_turns > 0 && context.turn_count >= config.max_turns {
        return GuidanceDecision {
            action: GuidanceAction::Stop,
            next_input: None,
            reason: "Reached Full Auto turn limit.".to_string(),
            confidence: 1.0,
            guardrail: Some(GuidanceGuardrailAudit {
                triggered: true,
                rule: Some("max_turns".to_string()),
                original_action: original_action.clone(),
                original_confidence,
                enforced_action: "stop".to_string(),
                enforced_confidence: 1.0,
            }),
        };
    }

    if config.no_progress_limit > 0 && context.no_progress_count >= config.no_progress_limit {
        return GuidanceDecision {
            action: GuidanceAction::Stop,
            next_input: None,
            reason: "No progress detected across multiple turns.".to_string(),
            confidence: 1.0,
            guardrail: Some(GuidanceGuardrailAudit {
                triggered: true,
                rule: Some("no_progress".to_string()),
                original_action: original_action.clone(),
                original_confidence,
                enforced_action: "stop".to_string(),
                enforced_confidence: 1.0,
            }),
        };
    }

    if let Some(max_tokens) = config.max_tokens {
        if let Some(total_tokens) = context.tokens_used {
            if total_tokens >= max_tokens {
                return GuidanceDecision {
                    action: GuidanceAction::Stop,
                    next_input: None,
                    reason: "Token budget exceeded; stopping Full Auto.".to_string(),
                    confidence: 1.0,
                    guardrail: Some(GuidanceGuardrailAudit {
                        triggered: true,
                        rule: Some("max_tokens".to_string()),
                        original_action: original_action.clone(),
                        original_confidence,
                        enforced_action: "stop".to_string(),
                        enforced_confidence: 1.0,
                    }),
                };
            }
        }
    }

    if decision.confidence < config.min_confidence {
        decision.action = GuidanceAction::Pause;
        decision.reason = format!(
            "Low confidence ({:.2}) decision; pausing Full Auto.",
            decision.confidence
        );
        guardrail = Some(GuidanceGuardrailAudit {
            triggered: true,
            rule: Some("low_confidence".to_string()),
            original_action: original_action.clone(),
            original_confidence,
            enforced_action: "pause".to_string(),
            enforced_confidence: decision.confidence,
        });
    }

    if decision.action == GuidanceAction::Review {
        decision.action = GuidanceAction::Pause;
        decision.reason = "Review requested; pausing Full Auto.".to_string();
        guardrail = Some(GuidanceGuardrailAudit {
            triggered: true,
            rule: Some("review_requested".to_string()),
            original_action: original_action.clone(),
            original_confidence,
            enforced_action: "pause".to_string(),
            enforced_confidence: decision.confidence,
        });
    }

    if guardrail.is_none() {
        decision.guardrail = Some(GuidanceGuardrailAudit {
            triggered: false,
            rule: None,
            original_action,
            original_confidence,
            enforced_action: decision.action.as_str().to_string(),
            enforced_confidence: decision.confidence,
        });
    } else {
        decision.guardrail = guardrail;
    }

    decision
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
        other => (
            0.0,
            Some(other),
            Some("confidence not numeric".to_string()),
        ),
    }
}
