use serde::{Deserialize, Serialize};
use serde_json::{Value, to_value};

use crate::{
    AgentRoute, CompiledModuleManifest, FirstGraphModuleHub, GraphAuthority, GroundedAnswerInput,
    IntentRouteInput, SelectedGraph, ShadowMode, ToolArgumentsInput, ToolArgumentsOutput,
    ToolCall, ToolPolicyInput, ToolResult, VerifyInput, VerifyVerdict,
};

/// Confidence floors for the first compiled-agent executor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceFallbackPolicy {
    /// Minimum route confidence.
    pub intent_route_floor: f32,
    /// Minimum tool policy confidence.
    pub tool_policy_floor: f32,
    /// Minimum tool argument confidence.
    pub tool_arguments_floor: f32,
    /// Minimum grounded answer confidence.
    pub grounded_answer_floor: f32,
    /// Minimum verify confidence.
    pub verify_floor: f32,
    /// User-visible fallback refusal used when confidence is too low.
    pub fallback_response: String,
}

impl Default for ConfidenceFallbackPolicy {
    fn default() -> Self {
        Self {
            intent_route_floor: 0.7,
            tool_policy_floor: 0.7,
            tool_arguments_floor: 0.7,
            grounded_answer_floor: 0.75,
            verify_floor: 0.75,
            fallback_response: "I do not have enough grounded confidence to answer that safely."
                .to_string(),
        }
    }
}

/// User-visible outcome class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicOutcomeKind {
    /// Grounded answer accepted.
    GroundedAnswer,
    /// Unsupported request refused cleanly.
    UnsupportedRefusal,
    /// Low-confidence execution fell back instead of bluffing.
    ConfidenceFallback,
}

/// User-visible response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicAgentResponse {
    /// User-visible outcome class.
    pub kind: PublicOutcomeKind,
    /// User-visible text only.
    pub response: String,
}

/// Retained per-phase trace entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseTraceEntry {
    /// Phase name.
    pub phase: String,
    /// Executed manifest.
    pub manifest: CompiledModuleManifest,
    /// Whether this phase came from promoted or candidate authority.
    pub authority: GraphAuthority,
    /// Candidate label when applicable.
    pub candidate_label: Option<String>,
    /// Serialized typed input.
    pub input: Value,
    /// Serialized typed output.
    pub output: Value,
    /// Claimed confidence.
    pub confidence: f32,
    /// Internal trace retained for replay and debugging.
    pub trace: Value,
}

/// Internal trace that stays separate from the user-visible answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InternalAgentTrace {
    /// Primary authority graph phases.
    pub primary_phases: Vec<PhaseTraceEntry>,
    /// Shadow graph phases, when enabled.
    pub shadow_phases: Vec<PhaseTraceEntry>,
}

/// Receipt-friendly lineage for downstream replay and training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentLineage {
    /// User request that triggered the run.
    pub user_request: String,
    /// Final route chosen by the authority graph.
    pub route: AgentRoute,
    /// Tool calls emitted by the authority graph.
    pub tool_calls: Vec<ToolCall>,
    /// Tool results executed by the authority graph.
    pub tool_results: Vec<ToolResult>,
    /// Final user-visible outcome.
    pub public_response: PublicAgentResponse,
    /// Manifest ids used by the authority graph.
    pub authority_manifest_ids: Vec<String>,
    /// Manifest ids used by the shadow graph, when enabled.
    pub shadow_manifest_ids: Vec<String>,
}

/// Full compiled-agent run outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRun {
    /// User-visible answer or refusal.
    pub public_response: PublicAgentResponse,
    /// Internal trace retained for replay/debug only.
    pub internal_trace: InternalAgentTrace,
    /// Downstream lineage seed for training receipts.
    pub lineage: CompiledAgentLineage,
}

/// Tool executor abstraction kept separate from module traces.
pub trait ToolExecutor {
    /// Execute a single tool call.
    fn execute(&self, call: &ToolCall) -> Option<ToolResult>;
}

/// Thin executor for the first compiled-agent graph.
pub struct CompiledAgentExecutor {
    module_hub: FirstGraphModuleHub,
    fallback_policy: ConfidenceFallbackPolicy,
}

impl CompiledAgentExecutor {
    /// Create a new executor around the module hub.
    #[must_use]
    pub fn new(module_hub: FirstGraphModuleHub, fallback_policy: ConfidenceFallbackPolicy) -> Self {
        Self {
            module_hub,
            fallback_policy,
        }
    }

    /// Execute the compiled-agent graph with optional shadow evaluation.
    pub fn run(
        &self,
        user_request: &str,
        available_tools: &[crate::ToolSpec],
        tool_executor: &dyn ToolExecutor,
        shadow_mode: ShadowMode,
    ) -> CompiledAgentRun {
        let primary_graph = self.module_hub.primary_graph(&shadow_mode);
        let shadow_graph = self.module_hub.shadow_graph(&shadow_mode);

        let primary = self.run_graph(&primary_graph, user_request, available_tools, tool_executor);
        let shadow = shadow_graph
            .as_ref()
            .map(|graph| self.run_graph(graph, user_request, available_tools, tool_executor));

        let public_response = primary.public_response.clone();
        let authority_manifest_ids = primary
            .phase_manifests()
            .into_iter()
            .map(|manifest| manifest.manifest_id())
            .collect::<Vec<_>>();
        let shadow_manifest_ids = shadow
            .as_ref()
            .map(GraphRun::phase_manifests)
            .unwrap_or_default()
            .into_iter()
            .map(|manifest| manifest.manifest_id())
            .collect::<Vec<_>>();

        CompiledAgentRun {
            public_response: public_response.clone(),
            internal_trace: InternalAgentTrace {
                primary_phases: primary.phases.clone(),
                shadow_phases: shadow
                    .as_ref()
                    .map(|run| run.phases.clone())
                    .unwrap_or_default(),
            },
            lineage: CompiledAgentLineage {
                user_request: user_request.to_string(),
                route: primary.route,
                tool_calls: primary.tool_calls,
                tool_results: primary.tool_results,
                public_response,
                authority_manifest_ids,
                shadow_manifest_ids,
            },
        }
    }

    fn run_graph(
        &self,
        graph: &SelectedGraph,
        user_request: &str,
        available_tools: &[crate::ToolSpec],
        tool_executor: &dyn ToolExecutor,
    ) -> GraphRun {
        let mut phases = Vec::new();

        let route_input = IntentRouteInput {
            user_request: user_request.to_string(),
        };
        let route_run = graph.intent_route.module.run(&route_input);
        phases.push(self.phase_entry(
            "intent_route",
            &graph.intent_route.manifest,
            graph.authority,
            graph.candidate_label.clone(),
            &route_input,
            &route_run.output,
            route_run.confidence,
            route_run.trace.clone(),
        ));
        if self.below_floor(route_run.confidence, graph.intent_route.manifest.confidence_floor, self.fallback_policy.intent_route_floor)
        {
            return GraphRun::fallback(phases, route_run.output.route, self.fallback_policy.fallback_response.clone());
        }

        let tool_policy_input = ToolPolicyInput {
            user_request: user_request.to_string(),
            route: route_run.output.route,
            available_tools: available_tools.to_vec(),
        };
        let tool_policy_run = graph.tool_policy.module.run(&tool_policy_input);
        phases.push(self.phase_entry(
            "tool_policy",
            &graph.tool_policy.manifest,
            graph.authority,
            graph.candidate_label.clone(),
            &tool_policy_input,
            &tool_policy_run.output,
            tool_policy_run.confidence,
            tool_policy_run.trace.clone(),
        ));
        if self.below_floor(tool_policy_run.confidence, graph.tool_policy.manifest.confidence_floor, self.fallback_policy.tool_policy_floor)
        {
            return GraphRun::fallback(
                phases,
                route_run.output.route,
                self.fallback_policy.fallback_response.clone(),
            );
        }

        let tool_arguments_input = ToolArgumentsInput {
            user_request: user_request.to_string(),
            route: route_run.output.route,
            selected_tools: tool_policy_run.output.selected_tools.clone(),
        };
        let tool_arguments_run = graph.tool_arguments.module.run(&tool_arguments_input);
        phases.push(self.phase_entry(
            "tool_arguments",
            &graph.tool_arguments.manifest,
            graph.authority,
            graph.candidate_label.clone(),
            &tool_arguments_input,
            &tool_arguments_run.output,
            tool_arguments_run.confidence,
            tool_arguments_run.trace.clone(),
        ));
        if self.below_floor(tool_arguments_run.confidence, graph.tool_arguments.manifest.confidence_floor, self.fallback_policy.tool_arguments_floor)
        {
            return GraphRun::fallback(
                phases,
                route_run.output.route,
                self.fallback_policy.fallback_response.clone(),
            );
        }

        let tool_results = self.execute_tools(&tool_arguments_run.output, tool_executor);
        let grounded_input = GroundedAnswerInput {
            user_request: user_request.to_string(),
            route: route_run.output.route,
            tool_results: tool_results.clone(),
        };
        let grounded_run = graph.grounded_answer.module.run(&grounded_input);
        phases.push(self.phase_entry(
            "grounded_answer",
            &graph.grounded_answer.manifest,
            graph.authority,
            graph.candidate_label.clone(),
            &grounded_input,
            &grounded_run.output,
            grounded_run.confidence,
            grounded_run.trace.clone(),
        ));
        if self.below_floor(grounded_run.confidence, graph.grounded_answer.manifest.confidence_floor, self.fallback_policy.grounded_answer_floor)
        {
            return GraphRun::fallback(
                phases,
                route_run.output.route,
                self.fallback_policy.fallback_response.clone(),
            );
        }

        let verify_input = VerifyInput {
            user_request: user_request.to_string(),
            route: route_run.output.route,
            tool_calls: tool_arguments_run.output.calls.clone(),
            tool_results: tool_results.clone(),
            candidate_answer: grounded_run.output.answer.clone(),
        };
        let verify_run = graph.verify.module.run(&verify_input);
        phases.push(self.phase_entry(
            "verify",
            &graph.verify.manifest,
            graph.authority,
            graph.candidate_label.clone(),
            &verify_input,
            &verify_run.output,
            verify_run.confidence,
            verify_run.trace.clone(),
        ));
        if self.below_floor(verify_run.confidence, graph.verify.manifest.confidence_floor, self.fallback_policy.verify_floor)
        {
            return GraphRun::fallback(
                phases,
                route_run.output.route,
                self.fallback_policy.fallback_response.clone(),
            );
        }

        let public_response = match verify_run.output.verdict {
            VerifyVerdict::AcceptGroundedAnswer => PublicAgentResponse {
                kind: PublicOutcomeKind::GroundedAnswer,
                response: grounded_run.output.answer.clone(),
            },
            VerifyVerdict::UnsupportedRefusal => PublicAgentResponse {
                kind: PublicOutcomeKind::UnsupportedRefusal,
                response: grounded_run.output.answer.clone(),
            },
            VerifyVerdict::NeedsFallback => PublicAgentResponse {
                kind: PublicOutcomeKind::ConfidenceFallback,
                response: self.fallback_policy.fallback_response.clone(),
            },
        };

        GraphRun {
            phases,
            route: route_run.output.route,
            tool_calls: tool_arguments_run.output.calls,
            tool_results,
            public_response,
        }
    }

    fn phase_entry<I: Serialize, O: Serialize>(
        &self,
        phase: &str,
        manifest: &CompiledModuleManifest,
        authority: GraphAuthority,
        candidate_label: Option<String>,
        input: &I,
        output: &O,
        confidence: f32,
        trace: Value,
    ) -> PhaseTraceEntry {
        PhaseTraceEntry {
            phase: phase.to_string(),
            manifest: manifest.clone(),
            authority,
            candidate_label,
            input: to_value(input).unwrap_or(Value::Null),
            output: to_value(output).unwrap_or(Value::Null),
            confidence,
            trace,
        }
    }

    fn execute_tools(
        &self,
        output: &ToolArgumentsOutput,
        tool_executor: &dyn ToolExecutor,
    ) -> Vec<ToolResult> {
        output
            .calls
            .iter()
            .filter_map(|call| tool_executor.execute(call))
            .collect()
    }

    fn below_floor(&self, confidence: f32, manifest_floor: f32, policy_floor: f32) -> bool {
        confidence < manifest_floor.max(policy_floor)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct GraphRun {
    phases: Vec<PhaseTraceEntry>,
    route: AgentRoute,
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<ToolResult>,
    public_response: PublicAgentResponse,
}

impl GraphRun {
    fn fallback(phases: Vec<PhaseTraceEntry>, route: AgentRoute, fallback_response: String) -> Self {
        Self {
            phases,
            route,
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            public_response: PublicAgentResponse {
                kind: PublicOutcomeKind::ConfidenceFallback,
                response: fallback_response,
            },
        }
    }

    fn phase_manifests(&self) -> Vec<CompiledModuleManifest> {
        self.phases
            .iter()
            .map(|entry| entry.manifest.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::{
        AgentRoute, CompiledModuleManifest, FirstGraphModuleHub, GroundedAnswerInput,
        GroundedAnswerOutput, GroundedAnswerSignature, IntentRouteInput, IntentRouteOutput,
        IntentRouteSignature, ModuleFamilyHub, ModulePromotionState, ModuleRun, ToolArgumentsInput,
        ToolArgumentsOutput, ToolArgumentsSignature, ToolCall, ToolPolicyInput, ToolPolicyOutput,
        ToolPolicySignature, ToolResult, ToolSpec, TypedModule, VerifyInput, VerifyOutput,
        VerifySignature, VerifyVerdict,
    };

    use super::{
        CompiledAgentExecutor, ConfidenceFallbackPolicy, PublicOutcomeKind, ShadowMode,
        ToolExecutor,
    };

    #[derive(Clone)]
    struct DemoRouteModule {
        manifest: CompiledModuleManifest,
        route: AgentRoute,
        confidence: f32,
    }

    impl TypedModule<IntentRouteSignature> for DemoRouteModule {
        fn manifest(&self) -> &CompiledModuleManifest {
            &self.manifest
        }

        fn run(&self, _input: &IntentRouteInput) -> ModuleRun<IntentRouteOutput> {
            ModuleRun::new(
                IntentRouteOutput {
                    route: self.route,
                    rationale: "demo".to_string(),
                },
                self.confidence,
                json!({"phase":"route"}),
            )
        }
    }

    #[derive(Clone)]
    struct DemoPolicyModule {
        manifest: CompiledModuleManifest,
        confidence: f32,
    }

    impl TypedModule<ToolPolicySignature> for DemoPolicyModule {
        fn manifest(&self) -> &CompiledModuleManifest {
            &self.manifest
        }

        fn run(&self, input: &ToolPolicyInput) -> ModuleRun<ToolPolicyOutput> {
            let selected_tools = match input.route {
                AgentRoute::ProviderStatus => input
                    .available_tools
                    .iter()
                    .filter(|tool| tool.name == "provider_status")
                    .cloned()
                    .collect(),
                AgentRoute::WalletStatus => input
                    .available_tools
                    .iter()
                    .filter(|tool| tool.name == "wallet_status")
                    .cloned()
                    .collect(),
                AgentRoute::Unsupported => Vec::new(),
            };
            ModuleRun::new(
                ToolPolicyOutput {
                    selected_tools,
                    rationale: "demo".to_string(),
                },
                self.confidence,
                json!({"phase":"tool_policy"}),
            )
        }
    }

    #[derive(Clone)]
    struct DemoArgumentsModule {
        manifest: CompiledModuleManifest,
        confidence: f32,
    }

    impl TypedModule<ToolArgumentsSignature> for DemoArgumentsModule {
        fn manifest(&self) -> &CompiledModuleManifest {
            &self.manifest
        }

        fn run(&self, input: &ToolArgumentsInput) -> ModuleRun<ToolArgumentsOutput> {
            ModuleRun::new(
                ToolArgumentsOutput {
                    calls: input
                        .selected_tools
                        .iter()
                        .map(|tool| ToolCall {
                            tool_name: tool.name.clone(),
                            arguments: json!({}),
                        })
                        .collect(),
                },
                self.confidence,
                json!({"phase":"tool_arguments"}),
            )
        }
    }

    #[derive(Clone)]
    struct DemoGroundedModule {
        manifest: CompiledModuleManifest,
        answer: String,
        confidence: f32,
    }

    impl TypedModule<GroundedAnswerSignature> for DemoGroundedModule {
        fn manifest(&self) -> &CompiledModuleManifest {
            &self.manifest
        }

        fn run(&self, input: &GroundedAnswerInput) -> ModuleRun<GroundedAnswerOutput> {
            ModuleRun::new(
                GroundedAnswerOutput {
                    answer: self.answer.clone(),
                    grounded_tool_names: input
                        .tool_results
                        .iter()
                        .map(|result| result.tool_name.clone())
                        .collect(),
                },
                self.confidence,
                json!({"phase":"grounded_answer"}),
            )
        }
    }

    #[derive(Clone)]
    struct DemoVerifyModule {
        manifest: CompiledModuleManifest,
        verdict: VerifyVerdict,
        confidence: f32,
    }

    impl TypedModule<VerifySignature> for DemoVerifyModule {
        fn manifest(&self) -> &CompiledModuleManifest {
            &self.manifest
        }

        fn run(&self, _input: &VerifyInput) -> ModuleRun<VerifyOutput> {
            ModuleRun::new(
                VerifyOutput {
                    verdict: self.verdict,
                    rationale: "demo".to_string(),
                },
                self.confidence,
                json!({"phase":"verify"}),
            )
        }
    }

    struct DemoToolExecutor;

    impl ToolExecutor for DemoToolExecutor {
        fn execute(&self, call: &ToolCall) -> Option<ToolResult> {
            let payload = match call.tool_name.as_str() {
                "provider_status" => json!({"ready": true, "label": "ready"}),
                "wallet_status" => json!({"balance_sats": 1200}),
                _ => return None,
            };
            Some(ToolResult {
                tool_name: call.tool_name.clone(),
                payload,
            })
        }
    }

    fn manifest(module_name: &str, label: &str, confidence_floor: f32, promotion_state: ModulePromotionState) -> CompiledModuleManifest {
        CompiledModuleManifest {
            module_name: module_name.to_string(),
            signature_name: module_name.to_string(),
            implementation_family: "rule_v1".to_string(),
            implementation_label: label.to_string(),
            version: "2026-03-28".to_string(),
            promotion_state,
            confidence_floor,
        }
    }

    fn build_hub(grounded_confidence: f32) -> FirstGraphModuleHub {
        let promoted_route = Arc::new(DemoRouteModule {
            manifest: manifest("intent_route", "promoted", 0.7, ModulePromotionState::Promoted),
            route: AgentRoute::ProviderStatus,
            confidence: 0.92,
        });
        let candidate_route = Arc::new(DemoRouteModule {
            manifest: manifest("intent_route", "candidate", 0.7, ModulePromotionState::Candidate),
            route: AgentRoute::WalletStatus,
            confidence: 0.91,
        });
        let promoted_policy = Arc::new(DemoPolicyModule {
            manifest: manifest("tool_policy", "promoted", 0.7, ModulePromotionState::Promoted),
            confidence: 0.88,
        });
        let candidate_policy = Arc::new(DemoPolicyModule {
            manifest: manifest("tool_policy", "candidate", 0.7, ModulePromotionState::Candidate),
            confidence: 0.87,
        });
        let promoted_args = Arc::new(DemoArgumentsModule {
            manifest: manifest("tool_arguments", "promoted", 0.7, ModulePromotionState::Promoted),
            confidence: 0.86,
        });
        let candidate_args = Arc::new(DemoArgumentsModule {
            manifest: manifest("tool_arguments", "candidate", 0.7, ModulePromotionState::Candidate),
            confidence: 0.85,
        });
        let promoted_grounded = Arc::new(DemoGroundedModule {
            manifest: manifest("grounded_answer", "promoted", 0.75, ModulePromotionState::Promoted),
            answer: "Provider is ready.".to_string(),
            confidence: grounded_confidence,
        });
        let candidate_grounded = Arc::new(DemoGroundedModule {
            manifest: manifest("grounded_answer", "candidate", 0.75, ModulePromotionState::Candidate),
            answer: "Wallet has 1200 sats.".to_string(),
            confidence: 0.9,
        });
        let promoted_verify = Arc::new(DemoVerifyModule {
            manifest: manifest("verify", "promoted", 0.75, ModulePromotionState::Promoted),
            verdict: VerifyVerdict::AcceptGroundedAnswer,
            confidence: 0.9,
        });
        let candidate_verify = Arc::new(DemoVerifyModule {
            manifest: manifest("verify", "candidate", 0.75, ModulePromotionState::Candidate),
            verdict: VerifyVerdict::AcceptGroundedAnswer,
            confidence: 0.9,
        });

        let mut intent_route = ModuleFamilyHub::new(promoted_route);
        intent_route.insert_candidate("candidate", candidate_route);
        let mut tool_policy = ModuleFamilyHub::new(promoted_policy);
        tool_policy.insert_candidate("candidate", candidate_policy);
        let mut tool_arguments = ModuleFamilyHub::new(promoted_args);
        tool_arguments.insert_candidate("candidate", candidate_args);
        let mut grounded_answer = ModuleFamilyHub::new(promoted_grounded);
        grounded_answer.insert_candidate("candidate", candidate_grounded);
        let mut verify = ModuleFamilyHub::new(promoted_verify);
        verify.insert_candidate("candidate", candidate_verify);

        FirstGraphModuleHub {
            intent_route,
            tool_policy,
            tool_arguments,
            grounded_answer,
            verify,
        }
    }

    fn tools() -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "provider_status".to_string(),
                description: "Provider readiness".to_string(),
            },
            ToolSpec {
                name: "wallet_status".to_string(),
                description: "Wallet balance".to_string(),
            },
        ]
    }

    #[test]
    fn shadow_candidate_trace_does_not_replace_promoted_authority() {
        let executor = CompiledAgentExecutor::new(
            build_hub(0.9),
            ConfidenceFallbackPolicy::default(),
        );
        let run = executor.run(
            "am I ready to go online?",
            &tools(),
            &DemoToolExecutor,
            ShadowMode::EvaluateCandidate {
                label: "candidate".to_string(),
            },
        );

        assert_eq!(run.public_response.kind, PublicOutcomeKind::GroundedAnswer);
        assert_eq!(run.public_response.response, "Provider is ready.".to_string());
        assert_eq!(run.internal_trace.primary_phases.len(), 5);
        assert_eq!(run.internal_trace.shadow_phases.len(), 5);
        assert_eq!(run.lineage.route, AgentRoute::ProviderStatus);
        assert!(run
            .lineage
            .shadow_manifest_ids
            .iter()
            .any(|manifest_id| manifest_id.contains("candidate")));
    }

    #[test]
    fn low_confidence_grounded_answer_falls_back_cleanly() {
        let executor = CompiledAgentExecutor::new(
            build_hub(0.5),
            ConfidenceFallbackPolicy::default(),
        );
        let run = executor.run(
            "am I ready to go online?",
            &tools(),
            &DemoToolExecutor,
            ShadowMode::Disabled,
        );

        assert_eq!(run.public_response.kind, PublicOutcomeKind::ConfidenceFallback);
        assert_eq!(
            run.public_response.response,
            "I do not have enough grounded confidence to answer that safely.".to_string()
        );
        assert!(run.internal_trace.primary_phases.len() >= 4);
        assert!(run.internal_trace.shadow_phases.is_empty());
    }
}
