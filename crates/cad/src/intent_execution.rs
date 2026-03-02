use serde::{Deserialize, Serialize};

use crate::chat_adapter::{CadIntentTranslationOutcome, translate_chat_to_cad_intent};
use crate::dispatch::{CadDispatchState, dispatch_cad_intent};
use crate::intent::{CadIntent, parse_cad_intent_json};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadIntentExecutionSource {
    #[serde(rename = "intent_json")]
    IntentJson,
    #[serde(rename = "natural_language")]
    NaturalLanguage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadIntentExecutionStage {
    #[serde(rename = "parsed")]
    Parsed,
    #[serde(rename = "inferred")]
    Inferred,
    #[serde(rename = "confirmation_required")]
    ConfirmationRequired,
    #[serde(rename = "applied")]
    Applied,
    #[serde(rename = "clarification_required")]
    ClarificationRequired,
    #[serde(rename = "dispatch_failed")]
    DispatchFailed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadIntentExecutionPlan {
    pub stage: CadIntentExecutionStage,
    pub source: CadIntentExecutionSource,
    pub intent_name: String,
    pub intent: CadIntent,
    pub summary: String,
    pub requires_confirmation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadIntentExecutionClarification {
    pub stage: CadIntentExecutionStage,
    pub code: String,
    pub message: String,
    pub recovery_prompt: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadIntentExecutionDispatchFailure {
    pub stage: CadIntentExecutionStage,
    pub intent_name: String,
    pub error: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadIntentExecutionReceipt {
    pub stage: CadIntentExecutionStage,
    pub source: CadIntentExecutionSource,
    pub intent_name: String,
    pub state_revision: u64,
    pub dispatch_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CadIntentExecutionDecision {
    NeedsConfirmation(CadIntentExecutionPlan),
    ClarificationRequired(CadIntentExecutionClarification),
    DispatchFailed(CadIntentExecutionDispatchFailure),
    Applied(CadIntentExecutionReceipt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadIntentExecutionPolicy {
    pub require_confirmation_for_natural_language: bool,
}

impl Default for CadIntentExecutionPolicy {
    fn default() -> Self {
        Self {
            require_confirmation_for_natural_language: true,
        }
    }
}

pub fn plan_intent_execution(
    input: &str,
) -> Result<CadIntentExecutionPlan, CadIntentExecutionClarification> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CadIntentExecutionClarification {
            stage: CadIntentExecutionStage::ClarificationRequired,
            code: "CAD-CHAT-EMPTY".to_string(),
            message: "chat message is empty".to_string(),
            recovery_prompt: "Send a CAD command phrase or a strict CadIntent JSON payload."
                .to_string(),
        });
    }

    if looks_like_strict_json(trimmed) {
        return match parse_cad_intent_json(trimmed) {
            Ok(intent) => Ok(CadIntentExecutionPlan {
                stage: CadIntentExecutionStage::Parsed,
                source: CadIntentExecutionSource::IntentJson,
                intent_name: intent.intent_name().to_string(),
                summary: format!(
                    "Apply {} from strict CadIntent JSON: {}",
                    intent.intent_name(),
                    intent_preview(&intent)
                ),
                intent,
                requires_confirmation: false,
            }),
            Err(error) => Err(CadIntentExecutionClarification {
                stage: CadIntentExecutionStage::ClarificationRequired,
                code: error.code,
                message: error.message,
                recovery_prompt:
                    "Fix the JSON to match CadIntent schema or send a supported CAD command phrase."
                        .to_string(),
            }),
        };
    }

    match translate_chat_to_cad_intent(trimmed) {
        CadIntentTranslationOutcome::Intent(intent) => Ok(CadIntentExecutionPlan {
            stage: CadIntentExecutionStage::Inferred,
            source: CadIntentExecutionSource::NaturalLanguage,
            intent_name: intent.intent_name().to_string(),
            summary: format!(
                "Proposed {} from natural language: {}",
                intent.intent_name(),
                intent_preview(&intent)
            ),
            intent,
            requires_confirmation: true,
        }),
        CadIntentTranslationOutcome::ParseFailure(error) => Err(CadIntentExecutionClarification {
            stage: CadIntentExecutionStage::ClarificationRequired,
            code: error.code,
            message: error.message,
            recovery_prompt: error.recovery_prompt,
        }),
    }
}

pub fn execute_intent_input(
    input: &str,
    state: &mut CadDispatchState,
    policy: CadIntentExecutionPolicy,
    confirmed: bool,
) -> CadIntentExecutionDecision {
    let plan = match plan_intent_execution(input) {
        Ok(plan) => plan,
        Err(clarification) => {
            return CadIntentExecutionDecision::ClarificationRequired(clarification);
        }
    };

    if plan.requires_confirmation && policy.require_confirmation_for_natural_language && !confirmed
    {
        let mut confirmation_plan = plan.clone();
        confirmation_plan.stage = CadIntentExecutionStage::ConfirmationRequired;
        return CadIntentExecutionDecision::NeedsConfirmation(confirmation_plan);
    }

    match dispatch_cad_intent(&plan.intent, state) {
        Ok(dispatch_receipt) => CadIntentExecutionDecision::Applied(CadIntentExecutionReceipt {
            stage: CadIntentExecutionStage::Applied,
            source: plan.source,
            intent_name: plan.intent_name,
            state_revision: dispatch_receipt.state_revision,
            dispatch_summary: dispatch_receipt.summary,
        }),
        Err(error) => {
            CadIntentExecutionDecision::DispatchFailed(CadIntentExecutionDispatchFailure {
                stage: CadIntentExecutionStage::DispatchFailed,
                intent_name: plan.intent_name,
                error: error.to_string(),
            })
        }
    }
}

fn looks_like_strict_json(input: &str) -> bool {
    input.starts_with('{') && input.ends_with('}')
}

fn intent_preview(intent: &CadIntent) -> String {
    match intent {
        CadIntent::CreateRackSpec(payload) => format!(
            "units={}, material={}, airflow={}, mount_type={}",
            payload.units, payload.material, payload.airflow, payload.mount_type
        ),
        CadIntent::GenerateVariants(payload) => {
            format!(
                "count={}, objective_set={}",
                payload.count, payload.objective_set
            )
        }
        CadIntent::SetObjective(payload) => format!("objective={}", payload.objective),
        CadIntent::AdjustParameter(payload) => format!(
            "parameter={}, operation={:?}, value={}",
            payload.parameter, payload.operation, payload.value
        ),
        CadIntent::SetMaterial(payload) => format!("material_id={}", payload.material_id),
        CadIntent::AddVentPattern(payload) => format!(
            "pattern={}, size_mm={}, density={}",
            payload.pattern, payload.size_mm, payload.density
        ),
        CadIntent::Select(payload) => format!("selector={}", payload.selector),
        CadIntent::CompareVariants(payload) => {
            format!("variant_ids={}", payload.variant_ids.join(","))
        }
        CadIntent::Export(payload) => {
            format!(
                "format={}, variant_id={}",
                payload.format, payload.variant_id
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadIntentExecutionDecision, CadIntentExecutionPolicy, CadIntentExecutionSource,
        CadIntentExecutionStage, execute_intent_input, plan_intent_execution,
    };
    use crate::dispatch::CadDispatchState;

    #[test]
    fn strict_json_path_executes_without_confirmation() {
        let payload = r#"{"intent":"SetMaterial","material_id":"al-6061-t6"}"#;
        let mut state = CadDispatchState::default();
        let decision = execute_intent_input(
            payload,
            &mut state,
            CadIntentExecutionPolicy::default(),
            false,
        );
        match decision {
            CadIntentExecutionDecision::Applied(receipt) => {
                assert_eq!(receipt.stage, CadIntentExecutionStage::Applied);
                assert_eq!(receipt.source, CadIntentExecutionSource::IntentJson);
                assert_eq!(receipt.intent_name, "SetMaterial");
                assert_eq!(receipt.state_revision, 1);
            }
            other => panic!("expected applied decision, got: {other:?}"),
        }
        assert_eq!(state.material_id.as_deref(), Some("al-6061-t6"));
    }

    #[test]
    fn natural_language_path_requires_confirmation_by_default() {
        let prompt = "Set material steel-1018";
        let mut state = CadDispatchState::default();
        let first = execute_intent_input(
            prompt,
            &mut state,
            CadIntentExecutionPolicy::default(),
            false,
        );
        match first {
            CadIntentExecutionDecision::NeedsConfirmation(plan) => {
                assert_eq!(plan.stage, CadIntentExecutionStage::ConfirmationRequired);
                assert_eq!(plan.source, CadIntentExecutionSource::NaturalLanguage);
                assert_eq!(plan.intent_name, "SetMaterial");
                assert!(plan.requires_confirmation);
            }
            other => panic!("expected confirmation gate, got: {other:?}"),
        }

        let second = execute_intent_input(
            prompt,
            &mut state,
            CadIntentExecutionPolicy::default(),
            true,
        );
        match second {
            CadIntentExecutionDecision::Applied(receipt) => {
                assert_eq!(receipt.source, CadIntentExecutionSource::NaturalLanguage);
                assert_eq!(receipt.intent_name, "SetMaterial");
                assert_eq!(receipt.state_revision, 1);
            }
            other => panic!("expected applied decision, got: {other:?}"),
        }
    }

    #[test]
    fn policy_can_disable_confirmation_gate_for_natural_language() {
        let mut state = CadDispatchState::default();
        let policy = CadIntentExecutionPolicy {
            require_confirmation_for_natural_language: false,
        };
        let decision = execute_intent_input("Set objective stiffness", &mut state, policy, false);
        match decision {
            CadIntentExecutionDecision::Applied(receipt) => {
                assert_eq!(receipt.intent_name, "SetObjective");
                assert_eq!(receipt.state_revision, 1);
            }
            other => panic!("expected applied decision, got: {other:?}"),
        }
    }

    #[test]
    fn ambiguous_prompt_returns_clarification_payload() {
        let mut state = CadDispatchState::default();
        let decision = execute_intent_input(
            "can you just make it better",
            &mut state,
            CadIntentExecutionPolicy::default(),
            false,
        );
        match decision {
            CadIntentExecutionDecision::ClarificationRequired(clarification) => {
                assert_eq!(
                    clarification.stage,
                    CadIntentExecutionStage::ClarificationRequired
                );
                assert_eq!(clarification.code, "CAD-CHAT-AMBIGUOUS");
                assert!(clarification.recovery_prompt.contains("Set material"));
            }
            other => panic!("expected clarification decision, got: {other:?}"),
        }
    }

    #[test]
    fn strict_json_validation_errors_return_machine_code() {
        let mut state = CadDispatchState::default();
        let decision = execute_intent_input(
            r#"{"intent":"SetMaterial","material_id":""}"#,
            &mut state,
            CadIntentExecutionPolicy::default(),
            false,
        );
        match decision {
            CadIntentExecutionDecision::ClarificationRequired(clarification) => {
                assert_eq!(clarification.code, "CAD-INTENT-INVALID-FIELD");
                assert!(clarification.message.contains("must not be empty"));
            }
            other => panic!("expected clarification decision, got: {other:?}"),
        }
    }

    #[test]
    fn planning_is_deterministic_for_same_input() {
        let prompt = "Build a wall rack in sheet metal with high airflow";
        let first = plan_intent_execution(prompt).expect("first plan should parse");
        let second = plan_intent_execution(prompt).expect("second plan should parse");
        assert_eq!(first, second);
    }
}
