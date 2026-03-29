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
use psionic_eval::{
    CompiledAgentModuleKind, CompiledAgentModuleRevisionSet, CompiledAgentRoute,
    CompiledAgentToolCall, CompiledAgentToolResult, CompiledAgentToolSpec,
    CompiledAgentVerifyVerdict, evaluate_compiled_agent_grounded_answer,
    evaluate_compiled_agent_route, evaluate_compiled_agent_tool_arguments,
    evaluate_compiled_agent_tool_policy, evaluate_compiled_agent_verify,
    predict_compiled_agent_route,
};
use psionic_train::{
    CompiledAgentArtifactContractEntry, CompiledAgentArtifactLifecycleState,
    CompiledAgentArtifactPayload, CompiledAgentPromotedArtifactContract,
    canonical_compiled_agent_promoted_artifact_contract,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

const FIRST_GRAPH_COMPATIBILITY_VERSION: &str = "openagents.compiled_agent.first_graph.v1";
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
    let contract = canonical_compiled_agent_promoted_artifact_contract()
        .expect("psionic compiled-agent promoted-artifact contract should load");
    build_module_hub_from_contract(&contract).expect(
        "compiled-agent promoted-artifact contract should be compatible with the first graph",
    )
}

fn build_module_hub_from_contract(
    contract: &CompiledAgentPromotedArtifactContract,
) -> Result<FirstGraphModuleHub, String> {
    let promoted_route = Arc::new(PsionicIntentRouteModule::from_entry(
        contract
            .promoted_entry(CompiledAgentModuleKind::Route)
            .ok_or_else(|| String::from("missing promoted route artifact"))?,
    )?);
    let promoted_policy = Arc::new(PsionicToolPolicyModule::from_entry(
        contract
            .promoted_entry(CompiledAgentModuleKind::ToolPolicy)
            .ok_or_else(|| String::from("missing promoted tool-policy artifact"))?,
    )?);
    let promoted_arguments = Arc::new(PsionicToolArgumentsModule::from_entry(
        contract
            .promoted_entry(CompiledAgentModuleKind::ToolArguments)
            .ok_or_else(|| String::from("missing promoted tool-arguments artifact"))?,
    )?);
    let promoted_grounded = Arc::new(PsionicGroundedAnswerModule::from_entry(
        contract
            .promoted_entry(CompiledAgentModuleKind::GroundedAnswer)
            .ok_or_else(|| String::from("missing promoted grounded-answer artifact"))?,
    )?);
    let promoted_verify = Arc::new(PsionicVerifyModule::from_entry(
        contract
            .promoted_entry(CompiledAgentModuleKind::Verify)
            .ok_or_else(|| String::from("missing promoted verify artifact"))?,
    )?);

    let mut intent_route = ModuleFamilyHub::new(promoted_route);
    for entry in contract.artifacts.iter().filter(|entry| {
        entry.module == CompiledAgentModuleKind::Route
            && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Candidate
    }) {
        if let Some(label) = entry.candidate_label.as_deref() {
            intent_route.insert_candidate(
                label,
                Arc::new(PsionicIntentRouteModule::from_entry(entry)?),
            );
        }
    }

    let tool_policy = ModuleFamilyHub::new(promoted_policy);
    let tool_arguments = ModuleFamilyHub::new(promoted_arguments);

    let mut grounded_answer = ModuleFamilyHub::new(promoted_grounded);
    for entry in contract.artifacts.iter().filter(|entry| {
        entry.module == CompiledAgentModuleKind::GroundedAnswer
            && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Candidate
    }) {
        if let Some(label) = entry.candidate_label.as_deref() {
            grounded_answer.insert_candidate(
                label,
                Arc::new(PsionicGroundedAnswerModule::from_entry(entry)?),
            );
        }
    }

    let mut verify = ModuleFamilyHub::new(promoted_verify);
    for entry in contract.artifacts.iter().filter(|entry| {
        entry.module == CompiledAgentModuleKind::Verify
            && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Candidate
    }) {
        if let Some(label) = entry.candidate_label.as_deref() {
            verify.insert_candidate(label, Arc::new(PsionicVerifyModule::from_entry(entry)?));
        }
    }

    Ok(FirstGraphModuleHub {
        intent_route,
        tool_policy,
        tool_arguments,
        grounded_answer,
        verify,
    })
}

fn manifest_from_entry(
    entry: &CompiledAgentArtifactContractEntry,
) -> Result<CompiledModuleManifest, String> {
    if entry.compatibility_version != FIRST_GRAPH_COMPATIBILITY_VERSION {
        return Err(format!(
            "artifact {} is incompatible with {}",
            entry.artifact_id, FIRST_GRAPH_COMPATIBILITY_VERSION
        ));
    }
    Ok(CompiledModuleManifest {
        module_name: entry.module_name.clone(),
        signature_name: entry.signature_name.clone(),
        implementation_family: entry.implementation_family.clone(),
        implementation_label: entry.implementation_label.clone(),
        version: entry.version.clone(),
        promotion_state: match entry.lifecycle_state {
            CompiledAgentArtifactLifecycleState::Promoted => ModulePromotionState::Promoted,
            CompiledAgentArtifactLifecycleState::Candidate => ModulePromotionState::Candidate,
        },
        confidence_floor: entry.confidence_floor,
        artifact_id: Some(entry.artifact_id.clone()),
        artifact_digest: Some(entry.artifact_digest.clone()),
        compatibility_version: Some(entry.compatibility_version.clone()),
        row_id: Some(entry.row_id.clone()),
        rollback_artifact_id: entry.rollback_artifact_id.clone(),
    })
}

fn revision_payload(
    entry: &CompiledAgentArtifactContractEntry,
) -> Result<CompiledAgentModuleRevisionSet, String> {
    match &entry.payload {
        CompiledAgentArtifactPayload::RevisionSet { revision } => Ok(revision.clone()),
        CompiledAgentArtifactPayload::RouteModel { .. } => Err(format!(
            "module {} expected a revision-set payload but got a route-model payload",
            entry.module_name
        )),
    }
}

fn openagents_route_to_psionic(route: AgentRoute) -> CompiledAgentRoute {
    match route {
        AgentRoute::ProviderStatus => CompiledAgentRoute::ProviderStatus,
        AgentRoute::WalletStatus => CompiledAgentRoute::WalletStatus,
        AgentRoute::Unsupported => CompiledAgentRoute::Unsupported,
    }
}

fn psionic_route_to_openagents(route: CompiledAgentRoute) -> AgentRoute {
    match route {
        CompiledAgentRoute::ProviderStatus => AgentRoute::ProviderStatus,
        CompiledAgentRoute::WalletStatus => AgentRoute::WalletStatus,
        CompiledAgentRoute::Unsupported => AgentRoute::Unsupported,
    }
}

fn psionic_tool_specs(tools: &[ToolSpec]) -> Vec<CompiledAgentToolSpec> {
    tools
        .iter()
        .map(|tool| CompiledAgentToolSpec {
            name: tool.name.clone(),
            description: tool.description.clone(),
        })
        .collect()
}

fn psionic_tool_results(results: &[ToolResult]) -> Vec<CompiledAgentToolResult> {
    results
        .iter()
        .map(|result| CompiledAgentToolResult {
            tool_name: result.tool_name.clone(),
            payload: result.payload.clone(),
        })
        .collect()
}

fn psionic_tool_calls_to_openagents(calls: Vec<CompiledAgentToolCall>) -> Vec<ToolCall> {
    calls
        .into_iter()
        .map(|call| ToolCall {
            tool_name: call.tool_name,
            arguments: call.arguments,
        })
        .collect()
}

fn verify_verdict_to_openagents(verdict: CompiledAgentVerifyVerdict) -> VerifyVerdict {
    match verdict {
        CompiledAgentVerifyVerdict::AcceptGroundedAnswer => VerifyVerdict::AcceptGroundedAnswer,
        CompiledAgentVerifyVerdict::UnsupportedRefusal => VerifyVerdict::UnsupportedRefusal,
        CompiledAgentVerifyVerdict::NeedsFallback => VerifyVerdict::NeedsFallback,
    }
}

fn grounded_confidence(route: AgentRoute, tool_results: &[ToolResult]) -> f32 {
    match route {
        AgentRoute::Unsupported => 0.95,
        AgentRoute::ProviderStatus | AgentRoute::WalletStatus if tool_results.is_empty() => 0.4,
        AgentRoute::ProviderStatus | AgentRoute::WalletStatus => 0.94,
    }
}

struct PsionicIntentRouteModule {
    manifest: CompiledModuleManifest,
    payload: CompiledAgentArtifactPayload,
}

impl PsionicIntentRouteModule {
    fn from_entry(entry: &CompiledAgentArtifactContractEntry) -> Result<Self, String> {
        Ok(Self {
            manifest: manifest_from_entry(entry)?,
            payload: entry.payload.clone(),
        })
    }
}

impl TypedModule<IntentRouteSignature> for PsionicIntentRouteModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &IntentRouteInput) -> ModuleRun<IntentRouteOutput> {
        match &self.payload {
            CompiledAgentArtifactPayload::RouteModel { artifact } => {
                let prediction =
                    predict_compiled_agent_route(artifact, input.user_request.as_str());
                let route = psionic_route_to_openagents(prediction.route);
                ModuleRun::new(
                    IntentRouteOutput {
                        route,
                        rationale: format!(
                            "route selected by promoted psionic artifact {}",
                            artifact.artifact_id
                        ),
                    },
                    prediction.confidence,
                    json!({
                        "artifact_id": artifact.artifact_id,
                        "artifact_digest": artifact.artifact_digest,
                        "active_features": prediction.active_features,
                        "score_margin": prediction.score_margin,
                        "route_scores": prediction.route_scores,
                    }),
                )
            }
            CompiledAgentArtifactPayload::RevisionSet { revision } => {
                let route = evaluate_compiled_agent_route(input.user_request.as_str(), revision);
                ModuleRun::new(
                    IntentRouteOutput {
                        route: psionic_route_to_openagents(route),
                        rationale: format!("route selected by revision {}", revision.revision_id),
                    },
                    0.93,
                    json!({
                        "revision_id": revision.revision_id,
                        "artifact_id": self.manifest.artifact_id,
                        "artifact_digest": self.manifest.artifact_digest,
                    }),
                )
            }
        }
    }
}

struct PsionicToolPolicyModule {
    manifest: CompiledModuleManifest,
}

impl PsionicToolPolicyModule {
    fn from_entry(entry: &CompiledAgentArtifactContractEntry) -> Result<Self, String> {
        Ok(Self {
            manifest: manifest_from_entry(entry)?,
        })
    }
}

impl TypedModule<ToolPolicySignature> for PsionicToolPolicyModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &ToolPolicyInput) -> ModuleRun<ToolPolicyOutput> {
        let selected_tools = evaluate_compiled_agent_tool_policy(
            openagents_route_to_psionic(input.route),
            &psionic_tool_specs(&input.available_tools),
        );
        ModuleRun::new(
            ToolPolicyOutput {
                selected_tools: selected_tools
                    .into_iter()
                    .map(|tool| ToolSpec {
                        name: tool.name,
                        description: tool.description,
                    })
                    .collect(),
                rationale: format!(
                    "tool policy selected by artifact {}",
                    self.manifest
                        .artifact_id
                        .as_deref()
                        .unwrap_or(self.manifest.implementation_label.as_str())
                ),
            },
            0.92,
            json!({
                "route": input.route,
                "artifact_id": self.manifest.artifact_id,
            }),
        )
    }
}

struct PsionicToolArgumentsModule {
    manifest: CompiledModuleManifest,
}

impl PsionicToolArgumentsModule {
    fn from_entry(entry: &CompiledAgentArtifactContractEntry) -> Result<Self, String> {
        Ok(Self {
            manifest: manifest_from_entry(entry)?,
        })
    }
}

impl TypedModule<ToolArgumentsSignature> for PsionicToolArgumentsModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &ToolArgumentsInput) -> ModuleRun<ToolArgumentsOutput> {
        let selected_tools = input
            .selected_tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<Vec<_>>();
        let calls = psionic_tool_calls_to_openagents(evaluate_compiled_agent_tool_arguments(
            &selected_tools,
        ));
        ModuleRun::new(
            ToolArgumentsOutput { calls },
            0.96,
            json!({
                "route": input.route,
                "tool_count": input.selected_tools.len(),
                "artifact_id": self.manifest.artifact_id,
            }),
        )
    }
}

struct PsionicGroundedAnswerModule {
    manifest: CompiledModuleManifest,
    revision: CompiledAgentModuleRevisionSet,
}

impl PsionicGroundedAnswerModule {
    fn from_entry(entry: &CompiledAgentArtifactContractEntry) -> Result<Self, String> {
        Ok(Self {
            manifest: manifest_from_entry(entry)?,
            revision: revision_payload(entry)?,
        })
    }
}

impl TypedModule<GroundedAnswerSignature> for PsionicGroundedAnswerModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &GroundedAnswerInput) -> ModuleRun<GroundedAnswerOutput> {
        let answer = evaluate_compiled_agent_grounded_answer(
            openagents_route_to_psionic(input.route),
            &psionic_tool_results(&input.tool_results),
            &self.revision,
        );
        ModuleRun::new(
            GroundedAnswerOutput {
                answer,
                grounded_tool_names: input
                    .tool_results
                    .iter()
                    .map(|result| result.tool_name.clone())
                    .collect(),
            },
            grounded_confidence(input.route, &input.tool_results),
            json!({
                "revision_id": self.revision.revision_id,
                "artifact_id": self.manifest.artifact_id,
                "artifact_digest": self.manifest.artifact_digest,
            }),
        )
    }
}

struct PsionicVerifyModule {
    manifest: CompiledModuleManifest,
    revision: CompiledAgentModuleRevisionSet,
}

impl PsionicVerifyModule {
    fn from_entry(entry: &CompiledAgentArtifactContractEntry) -> Result<Self, String> {
        Ok(Self {
            manifest: manifest_from_entry(entry)?,
            revision: revision_payload(entry)?,
        })
    }
}

impl TypedModule<VerifySignature> for PsionicVerifyModule {
    fn manifest(&self) -> &CompiledModuleManifest {
        &self.manifest
    }

    fn run(&self, input: &VerifyInput) -> ModuleRun<VerifyOutput> {
        let selected_tools = input
            .tool_calls
            .iter()
            .map(|call| call.tool_name.clone())
            .collect::<Vec<_>>();
        let verdict = evaluate_compiled_agent_verify(
            openagents_route_to_psionic(input.route),
            &selected_tools,
            &psionic_tool_results(&input.tool_results),
            input.candidate_answer.as_str(),
            &self.revision,
        );
        ModuleRun::new(
            VerifyOutput {
                verdict: verify_verdict_to_openagents(verdict),
                rationale: format!(
                    "verify decision produced by artifact {}",
                    self.manifest
                        .artifact_id
                        .as_deref()
                        .unwrap_or(self.manifest.implementation_label.as_str())
                ),
            },
            0.94,
            json!({
                "revision_id": self.revision.revision_id,
                "artifact_id": self.manifest.artifact_id,
                "artifact_digest": self.manifest.artifact_digest,
            }),
        )
    }
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
        assert_eq!(
            receipt.run.lineage.tool_calls[0].tool_name,
            "provider_status"
        );
        assert_eq!(
            receipt.run.lineage.authority_manifests[0]
                .artifact_id
                .as_deref(),
            Some("compiled_agent.route.multinomial_nb_v1")
        );
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
                label: "psionic_candidate".to_string(),
            },
        );

        assert!(!receipt.run.internal_trace.shadow_phases.is_empty());
        assert!(
            receipt
                .run
                .lineage
                .shadow_manifest_ids
                .iter()
                .any(|manifest_id| manifest_id.contains("psionic_candidate"))
        );
        assert!(receipt.run.lineage.shadow_manifests.iter().any(|manifest| {
            manifest.artifact_id.as_deref()
                == Some("compiled_agent.grounded_answer.rule_v2.recent_earnings")
        }));
        assert_eq!(
            receipt.run.public_response.response,
            "The wallet contains 1200 sats.".to_string()
        );
    }

    #[test]
    fn rollback_candidate_authority_uses_last_known_good_route_artifact() {
        let receipt = run_compiled_agent_slice(
            "Can I go online right now?",
            &CompiledAgentSliceState::default(),
            ShadowMode::CandidateAuthority {
                label: "last_known_good".to_string(),
            },
        );

        assert_eq!(
            receipt.run.lineage.authority_manifests[0]
                .artifact_id
                .as_deref(),
            Some("compiled_agent.baseline.rule_v1.route")
        );
        assert_eq!(
            receipt.run.public_response.response,
            "Provider is ready to go online.".to_string()
        );
    }
}
