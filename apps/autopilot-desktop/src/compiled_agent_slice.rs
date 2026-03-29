use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_compiled_agent::{
    AgentRoute, CompiledAgentExecutor, CompiledAgentRun, CompiledModuleManifest,
    ConfidenceFallbackPolicy, FirstGraphModuleHub, GroundedAnswerInput, GroundedAnswerOutput,
    GroundedAnswerSignature, IntentRouteInput, IntentRouteOutput, IntentRouteSignature,
    ModuleFamilyHub, ModulePromotionState, ModuleRun, ShadowMode, ToolArgumentsInput,
    ToolArgumentsOutput, ToolArgumentsSignature, ToolCall, ToolExecutor, ToolPolicyInput,
    ToolPolicyOutput, ToolPolicySignature, ToolResult, ToolSpec, TypedModule, VerifyInput,
    VerifyOutput, VerifySignature, VerifyVerdict,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const PROVIDER_STATUS_TOOL: &str = "provider_status";
const WALLET_STATUS_TOOL: &str = "wallet_status";

/// Narrow runtime context for the first compiled-agent vertical slice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSliceState {
    /// Whether the provider is currently ready to go online.
    pub provider_ready: bool,
    /// Blocker codes explaining why the provider is not ready.
    pub provider_blockers: Vec<String>,
    /// Current wallet balance in sats.
    pub wallet_balance_sats: u64,
    /// Recent earned sats shown in the answer when useful.
    pub recent_earnings_sats: u64,
}

impl Default for CompiledAgentSliceState {
    fn default() -> Self {
        Self {
            provider_ready: true,
            provider_blockers: Vec::new(),
            wallet_balance_sats: 1_200,
            recent_earnings_sats: 240,
        }
    }
}

/// Receipt written by the harness for replay and downstream training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentSliceReceipt {
    /// Schema version for downstream replay conversion.
    pub schema_version: u32,
    /// Millisecond timestamp when the harness finished.
    pub captured_at_epoch_ms: u64,
    /// Harness runtime state used to answer the prompt.
    pub state: CompiledAgentSliceState,
    /// Full compiled-agent run outcome.
    pub run: CompiledAgentRun,
}

/// Execute the first narrow app-owned compiled-agent path.
#[must_use]
pub fn run_compiled_agent_slice(
    prompt: &str,
    state: &CompiledAgentSliceState,
    shadow_mode: ShadowMode,
) -> CompiledAgentSliceReceipt {
    let executor = CompiledAgentExecutor::new(build_module_hub(), fallback_policy());
    let runtime = SliceToolRuntime {
        state: state.clone(),
    };
    let run = executor.run(prompt, &supported_tools(), &runtime, shadow_mode);
    CompiledAgentSliceReceipt {
        schema_version: 1,
        captured_at_epoch_ms: now_epoch_ms(),
        state: state.clone(),
        run,
    }
}

/// Stable tools exposed by this narrow slice.
#[must_use]
pub fn supported_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: PROVIDER_STATUS_TOOL.to_string(),
            description: "Read provider readiness and blocker state.".to_string(),
        },
        ToolSpec {
            name: WALLET_STATUS_TOOL.to_string(),
            description: "Read wallet balance and recent earnings.".to_string(),
        },
    ]
}

fn fallback_policy() -> ConfidenceFallbackPolicy {
    ConfidenceFallbackPolicy {
        fallback_response: "I could not produce a grounded answer for that request safely."
            .to_string(),
        ..ConfidenceFallbackPolicy::default()
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn build_module_hub() -> FirstGraphModuleHub {
    let promoted_route = Arc::new(KeywordIntentRouteModule::promoted());
    let candidate_route = Arc::new(KeywordIntentRouteModule::candidate());
    let promoted_policy = Arc::new(NarrowToolPolicyModule::promoted());
    let candidate_policy = Arc::new(NarrowToolPolicyModule::candidate());
    let promoted_arguments = Arc::new(NarrowToolArgumentsModule::promoted());
    let candidate_arguments = Arc::new(NarrowToolArgumentsModule::candidate());
    let promoted_grounded = Arc::new(NarrowGroundedAnswerModule::promoted());
    let candidate_grounded = Arc::new(NarrowGroundedAnswerModule::candidate());
    let promoted_verify = Arc::new(NarrowVerifyModule::promoted());
    let candidate_verify = Arc::new(NarrowVerifyModule::candidate());

    let mut intent_route = ModuleFamilyHub::new(promoted_route);
    intent_route.insert_candidate("compact", candidate_route);
    let mut tool_policy = ModuleFamilyHub::new(promoted_policy);
    tool_policy.insert_candidate("compact", candidate_policy);
    let mut tool_arguments = ModuleFamilyHub::new(promoted_arguments);
    tool_arguments.insert_candidate("compact", candidate_arguments);
    let mut grounded_answer = ModuleFamilyHub::new(promoted_grounded);
    grounded_answer.insert_candidate("compact", candidate_grounded);
    let mut verify = ModuleFamilyHub::new(promoted_verify);
    verify.insert_candidate("compact", candidate_verify);

    FirstGraphModuleHub {
        intent_route,
        tool_policy,
        tool_arguments,
        grounded_answer,
        verify,
    }
}

fn manifest(
    module_name: &str,
    implementation_label: &str,
    promotion_state: ModulePromotionState,
    confidence_floor: f32,
) -> CompiledModuleManifest {
    CompiledModuleManifest {
        module_name: module_name.to_string(),
        signature_name: module_name.to_string(),
        implementation_family: "rule_v1".to_string(),
        implementation_label: implementation_label.to_string(),
        version: "2026-03-28".to_string(),
        promotion_state,
        confidence_floor,
    }
}

fn normalized_tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn contains_any(tokens: &[String], needle_set: &[&str]) -> bool {
    tokens
        .iter()
        .any(|token| needle_set.iter().any(|needle| token == needle))
}

struct KeywordIntentRouteModule {
    manifest: CompiledModuleManifest,
}

impl KeywordIntentRouteModule {
    fn promoted() -> Self {
        Self {
            manifest: manifest(
                "intent_route",
                "promoted",
                ModulePromotionState::Promoted,
                0.8,
            ),
        }
    }

    fn candidate() -> Self {
        Self {
            manifest: manifest(
                "intent_route",
                "compact",
                ModulePromotionState::Candidate,
                0.8,
            ),
        }
    }
}

impl TypedModule<IntentRouteSignature> for KeywordIntentRouteModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &IntentRouteInput) -> ModuleRun<IntentRouteOutput> {
        let tokens = normalized_tokens(&input.user_request);
        let asks_provider = contains_any(&tokens, &["provider", "online", "ready", "readiness"]);
        let asks_wallet = contains_any(&tokens, &["wallet", "balance", "sats", "earnings"]);
        let (route, rationale, confidence) = match (asks_provider, asks_wallet) {
            (true, false) => (
                AgentRoute::ProviderStatus,
                "prompt asked about provider readiness".to_string(),
                0.94,
            ),
            (false, true) => (
                AgentRoute::WalletStatus,
                "prompt asked about wallet state".to_string(),
                0.94,
            ),
            (false, false) => (
                AgentRoute::Unsupported,
                "prompt is outside the narrow compiled-agent slice".to_string(),
                0.93,
            ),
            (true, true) => (
                AgentRoute::Unsupported,
                "prompt mixed multiple intents outside the narrow slice".to_string(),
                0.82,
            ),
        };
        ModuleRun::new(
            IntentRouteOutput { route, rationale },
            confidence,
            json!({
                "tokens": tokens,
                "implementation_label": self.manifest.implementation_label,
            }),
        )
    }
}

struct NarrowToolPolicyModule {
    manifest: CompiledModuleManifest,
}

impl NarrowToolPolicyModule {
    fn promoted() -> Self {
        Self {
            manifest: manifest(
                "tool_policy",
                "promoted",
                ModulePromotionState::Promoted,
                0.8,
            ),
        }
    }

    fn candidate() -> Self {
        Self {
            manifest: manifest(
                "tool_policy",
                "compact",
                ModulePromotionState::Candidate,
                0.8,
            ),
        }
    }
}

impl TypedModule<ToolPolicySignature> for NarrowToolPolicyModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &ToolPolicyInput) -> ModuleRun<ToolPolicyOutput> {
        let selected_tools = match input.route {
            AgentRoute::ProviderStatus => input
                .available_tools
                .iter()
                .filter(|tool| tool.name == PROVIDER_STATUS_TOOL)
                .cloned()
                .collect(),
            AgentRoute::WalletStatus => input
                .available_tools
                .iter()
                .filter(|tool| tool.name == WALLET_STATUS_TOOL)
                .cloned()
                .collect(),
            AgentRoute::Unsupported => Vec::new(),
        };
        ModuleRun::new(
            ToolPolicyOutput {
                selected_tools,
                rationale: "narrow slice exposes only the tool family required by the route"
                    .to_string(),
            },
            0.92,
            json!({
                "route": input.route,
                "implementation_label": self.manifest.implementation_label,
            }),
        )
    }
}

struct NarrowToolArgumentsModule {
    manifest: CompiledModuleManifest,
}

impl NarrowToolArgumentsModule {
    fn promoted() -> Self {
        Self {
            manifest: manifest(
                "tool_arguments",
                "promoted",
                ModulePromotionState::Promoted,
                0.8,
            ),
        }
    }

    fn candidate() -> Self {
        Self {
            manifest: manifest(
                "tool_arguments",
                "compact",
                ModulePromotionState::Candidate,
                0.8,
            ),
        }
    }
}

impl TypedModule<ToolArgumentsSignature> for NarrowToolArgumentsModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &ToolArgumentsInput) -> ModuleRun<ToolArgumentsOutput> {
        let calls = input
            .selected_tools
            .iter()
            .map(|tool| ToolCall {
                tool_name: tool.name.clone(),
                arguments: json!({}),
            })
            .collect::<Vec<_>>();
        ModuleRun::new(
            ToolArgumentsOutput { calls },
            0.96,
            json!({
                "route": input.route,
                "tool_count": input.selected_tools.len(),
                "implementation_label": self.manifest.implementation_label,
            }),
        )
    }
}

struct NarrowGroundedAnswerModule {
    manifest: CompiledModuleManifest,
}

impl NarrowGroundedAnswerModule {
    fn promoted() -> Self {
        Self {
            manifest: manifest(
                "grounded_answer",
                "promoted",
                ModulePromotionState::Promoted,
                0.82,
            ),
        }
    }

    fn candidate() -> Self {
        Self {
            manifest: manifest(
                "grounded_answer",
                "compact",
                ModulePromotionState::Candidate,
                0.82,
            ),
        }
    }
}

impl TypedModule<GroundedAnswerSignature> for NarrowGroundedAnswerModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &GroundedAnswerInput) -> ModuleRun<GroundedAnswerOutput> {
        let output = match input.route {
            AgentRoute::ProviderStatus => provider_grounded_answer(
                input.tool_results.as_slice(),
                self.manifest.implementation_label.as_str(),
            ),
            AgentRoute::WalletStatus => wallet_grounded_answer(
                input.tool_results.as_slice(),
                self.manifest.implementation_label.as_str(),
            ),
            AgentRoute::Unsupported => ModuleRun::new(
                GroundedAnswerOutput {
                    answer:
                        "I can currently answer only provider readiness and wallet balance questions."
                            .to_string(),
                    grounded_tool_names: Vec::new(),
                },
                0.95,
                json!({"reason":"unsupported_route"}),
            ),
        };
        output
    }
}

fn provider_grounded_answer(
    tool_results: &[ToolResult],
    implementation_label: &str,
) -> ModuleRun<GroundedAnswerOutput> {
    let provider_result = tool_results
        .iter()
        .find(|result| result.tool_name == PROVIDER_STATUS_TOOL);
    let Some(provider_result) = provider_result else {
        return ModuleRun::new(
            GroundedAnswerOutput {
                answer: "Provider status was unavailable.".to_string(),
                grounded_tool_names: Vec::new(),
            },
            0.4,
            json!({"reason":"missing_provider_status"}),
        );
    };

    let ready = provider_result
        .payload
        .get("ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let blockers = provider_result
        .payload
        .get("blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();

    let answer = match (ready, implementation_label) {
        (true, "compact") => "Provider ready.".to_string(),
        (true, _) => "Provider is ready to go online.".to_string(),
        (false, "compact") if blockers.is_empty() => "Provider not ready.".to_string(),
        (false, "compact") => format!("Provider not ready. Blockers: {}.", blockers.join(", ")),
        (false, _) if blockers.is_empty() => "Provider is not ready to go online.".to_string(),
        (false, _) => format!(
            "Provider is not ready to go online. Blockers: {}.",
            blockers.join(", ")
        ),
    };

    ModuleRun::new(
        GroundedAnswerOutput {
            answer,
            grounded_tool_names: vec![PROVIDER_STATUS_TOOL.to_string()],
        },
        0.94,
        json!({
            "ready": ready,
            "blockers": blockers,
            "implementation_label": implementation_label,
        }),
    )
}

fn wallet_grounded_answer(
    tool_results: &[ToolResult],
    implementation_label: &str,
) -> ModuleRun<GroundedAnswerOutput> {
    let wallet_result = tool_results
        .iter()
        .find(|result| result.tool_name == WALLET_STATUS_TOOL);
    let Some(wallet_result) = wallet_result else {
        return ModuleRun::new(
            GroundedAnswerOutput {
                answer: "Wallet status was unavailable.".to_string(),
                grounded_tool_names: Vec::new(),
            },
            0.4,
            json!({"reason":"missing_wallet_status"}),
        );
    };

    let balance_sats = wallet_result
        .payload
        .get("balance_sats")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let recent_earnings_sats = wallet_result
        .payload
        .get("recent_earnings_sats")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let answer = if implementation_label == "compact" {
        format!("Wallet: {balance_sats} sats.")
    } else {
        format!(
            "Wallet balance is {balance_sats} sats, with {recent_earnings_sats} sats of recent earnings."
        )
    };

    ModuleRun::new(
        GroundedAnswerOutput {
            answer,
            grounded_tool_names: vec![WALLET_STATUS_TOOL.to_string()],
        },
        0.94,
        json!({
            "balance_sats": balance_sats,
            "recent_earnings_sats": recent_earnings_sats,
            "implementation_label": implementation_label,
        }),
    )
}

struct NarrowVerifyModule {
    manifest: CompiledModuleManifest,
}

impl NarrowVerifyModule {
    fn promoted() -> Self {
        Self {
            manifest: manifest("verify", "promoted", ModulePromotionState::Promoted, 0.82),
        }
    }

    fn candidate() -> Self {
        Self {
            manifest: manifest("verify", "compact", ModulePromotionState::Candidate, 0.82),
        }
    }
}

impl TypedModule<VerifySignature> for NarrowVerifyModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &VerifyInput) -> ModuleRun<VerifyOutput> {
        let (verdict, rationale) = match input.route {
            AgentRoute::ProviderStatus => verify_provider_answer(input),
            AgentRoute::WalletStatus => verify_wallet_answer(input),
            AgentRoute::Unsupported => {
                let refusal = input.tool_calls.is_empty()
                    && input
                        .candidate_answer
                        .contains("provider readiness and wallet balance");
                if refusal {
                    (
                        VerifyVerdict::UnsupportedRefusal,
                        "unsupported request refused cleanly".to_string(),
                    )
                } else {
                    (
                        VerifyVerdict::NeedsFallback,
                        "unsupported route did not stay inside the narrow refusal contract".to_string(),
                    )
                }
            }
        };
        ModuleRun::new(
            VerifyOutput { verdict, rationale },
            0.94,
            json!({"implementation_label": self.manifest.implementation_label}),
        )
    }
}

fn verify_provider_answer(input: &VerifyInput) -> (VerifyVerdict, String) {
    let ready = input
        .tool_results
        .iter()
        .find(|result| result.tool_name == PROVIDER_STATUS_TOOL)
        .and_then(|result| result.payload.get("ready"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let answer = input.candidate_answer.to_ascii_lowercase();
    if ready && answer.contains("ready") {
        return (
            VerifyVerdict::AcceptGroundedAnswer,
            "provider answer reflects readiness".to_string(),
        );
    }
    if !ready && answer.contains("not ready") {
        return (
            VerifyVerdict::AcceptGroundedAnswer,
            "provider answer reflects blockers".to_string(),
        );
    }
    (
        VerifyVerdict::NeedsFallback,
        "provider answer did not reflect the tool result".to_string(),
    )
}

fn verify_wallet_answer(input: &VerifyInput) -> (VerifyVerdict, String) {
    let balance_sats = input
        .tool_results
        .iter()
        .find(|result| result.tool_name == WALLET_STATUS_TOOL)
        .and_then(|result| result.payload.get("balance_sats"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if input.candidate_answer.contains(balance_sats.to_string().as_str()) {
        return (
            VerifyVerdict::AcceptGroundedAnswer,
            "wallet answer reflects the balance".to_string(),
        );
    }
    (
        VerifyVerdict::NeedsFallback,
        "wallet answer omitted the returned balance".to_string(),
    )
}

struct SliceToolRuntime {
    state: CompiledAgentSliceState,
}

impl ToolExecutor for SliceToolRuntime {
    fn execute(&self, call: &ToolCall) -> Option<ToolResult> {
        let payload = match call.tool_name.as_str() {
            PROVIDER_STATUS_TOOL => json!({
                "ready": self.state.provider_ready,
                "blockers": self.state.provider_blockers,
            }),
            WALLET_STATUS_TOOL => json!({
                "balance_sats": self.state.wallet_balance_sats,
                "recent_earnings_sats": self.state.recent_earnings_sats,
            }),
            _ => return None,
        };
        Some(ToolResult {
            tool_name: call.tool_name.clone(),
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use openagents_compiled_agent::ShadowMode;

    use super::{CompiledAgentSliceState, run_compiled_agent_slice};

    #[test]
    fn provider_prompt_uses_provider_tool_only() {
        let receipt = run_compiled_agent_slice(
            "Can I go online right now?",
            &CompiledAgentSliceState::default(),
            ShadowMode::Disabled,
        );

        assert_eq!(receipt.run.lineage.tool_calls.len(), 1);
        assert_eq!(receipt.run.lineage.tool_calls[0].tool_name, "provider_status");
        assert_eq!(
            receipt.run.public_response.response,
            "Provider is ready to go online.".to_string()
        );
    }

    #[test]
    fn wallet_prompt_uses_wallet_tool_only() {
        let receipt = run_compiled_agent_slice(
            "How many sats are in the wallet?",
            &CompiledAgentSliceState::default(),
            ShadowMode::Disabled,
        );

        assert_eq!(receipt.run.lineage.tool_calls.len(), 1);
        assert_eq!(receipt.run.lineage.tool_calls[0].tool_name, "wallet_status");
        assert!(receipt.run.public_response.response.contains("1200 sats"));
    }

    #[test]
    fn unsupported_prompt_refuses_without_tool_calls() {
        let receipt = run_compiled_agent_slice(
            "Write a poem about GPUs.",
            &CompiledAgentSliceState::default(),
            ShadowMode::Disabled,
        );

        assert!(receipt.run.lineage.tool_calls.is_empty());
        assert_eq!(
            receipt.run.public_response.kind,
            openagents_compiled_agent::PublicOutcomeKind::UnsupportedRefusal
        );
    }

    #[test]
    fn shadow_candidate_keeps_primary_authority() {
        let receipt = run_compiled_agent_slice(
            "How many sats are in the wallet?",
            &CompiledAgentSliceState::default(),
            ShadowMode::EvaluateCandidate {
                label: "compact".to_string(),
            },
        );

        assert!(!receipt.run.internal_trace.shadow_phases.is_empty());
        assert!(receipt
            .run
            .lineage
            .shadow_manifest_ids
            .iter()
            .any(|manifest_id| manifest_id.contains("compact")));
        assert_eq!(
            receipt.run.public_response.response,
            "Wallet balance is 1200 sats, with 240 sats of recent earnings.".to_string()
        );
    }
}
