use std::collections::{BTreeMap, VecDeque};

use psionic_models::{
    TassadarExecutorContractError, TassadarExecutorFixture, TassadarExecutorModelDescriptor,
};
use psionic_runtime::{
    TassadarExecutionEvidenceBundle, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarExecutorExecutionReport, TassadarExecutorSelectionDiagnostic, TassadarProgramArtifact,
    TassadarRuntimeCapabilityReport, TassadarTraceEvent, TassadarTraceStep,
    tassadar_wasm_profile_for_id,
    build_tassadar_execution_evidence_bundle, execute_tassadar_executor_request,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Dedicated served product identifier for the Tassadar executor lane.
pub const EXECUTOR_TRACE_PRODUCT_ID: &str = "psionic.executor_trace";

/// Dedicated planner-owned routing product for exact executor delegation.
pub const PLANNER_EXECUTOR_ROUTE_PRODUCT_ID: &str = "psionic.planner_executor_route";

/// Explicit request contract for the served Tassadar executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.executor_trace`.
    pub product_id: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Digest-bound program artifact submitted to the executor.
    pub program_artifact: TassadarProgramArtifact,
    /// Requested decode mode for the execution.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Explicit environment refs carried into runtime-manifest lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarExecutorRequest {
    /// Creates a request for the explicit executor-trace product family.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        program_artifact: TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            requested_model_id: None,
            program_artifact,
            requested_decode_mode,
            environment_refs: Vec::new(),
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Carries environment refs into the served evidence bundle.
    #[must_use]
    pub fn with_environment_refs(mut self, environment_refs: Vec<String>) -> Self {
        let mut environment_refs = environment_refs;
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }

    /// Returns a stable digest for the request surface.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Tassadar executor request should serialize");
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_executor_request|");
        hasher.update(encoded);
        hex::encode(hasher.finalize())
    }
}

/// Explicit refusal response for unsupported ABI, profile, decode, or model pairings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorRefusalResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor that evaluated the request.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Contract error when model/program pairing failed before selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Runtime selection diagnostic when decode selection reached refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Completed served executor response carrying the exact runtime truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Direct/fallback selection plus realized execution.
    pub execution_report: TassadarExecutorExecutionReport,
    /// Runtime-manifest, trace, proof, and proof-bundle evidence.
    pub evidence_bundle: TassadarExecutionEvidenceBundle,
    /// Ordered environment refs carried into execution lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarExecutorResponse {
    /// Returns the final scalar outputs emitted by the execution.
    #[must_use]
    pub fn final_outputs(&self) -> &[i32] {
        &self.execution_report.execution.outputs
    }

    /// Returns the emitted trace artifact.
    #[must_use]
    pub fn trace_artifact(&self) -> &psionic_runtime::TassadarTraceArtifact {
        &self.evidence_bundle.trace_artifact
    }

    /// Returns the proof-bearing trace artifact.
    #[must_use]
    pub fn trace_proof(&self) -> &psionic_runtime::TassadarTraceProofArtifact {
        &self.evidence_bundle.trace_proof
    }
}

/// Served outcome for one executor request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarExecutorOutcome {
    /// The executor accepted and completed the request.
    Completed { response: TassadarExecutorResponse },
    /// The executor refused the request explicitly.
    Refused {
        refusal: TassadarExecutorRefusalResponse,
    },
}

/// Step event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTraceStepEvent {
    /// Ordinal step index.
    pub step_index: u64,
    /// Full append-only trace step.
    pub step: TassadarTraceStep,
}

/// Output event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorOutputEvent {
    /// Output ordinal in the final ordered output vector.
    pub ordinal: usize,
    /// Output scalar value.
    pub value: i32,
}

/// Terminal event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTerminalEvent {
    /// Final served outcome.
    pub outcome: TassadarExecutorOutcome,
}

/// Typed event emitted by the pull-driven executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorStreamEvent {
    /// Runtime capability report surfaced before execution.
    Capability {
        /// Runtime capability report visible to the caller.
        runtime_capability: TassadarRuntimeCapabilityReport,
    },
    /// Decode-selection diagnostic surfaced before the first trace step.
    Selection {
        /// Direct/fallback/refused selection diagnostic.
        selection: TassadarExecutorSelectionDiagnostic,
    },
    /// One append-only trace step.
    TraceStep {
        /// Step event payload.
        trace_step: TassadarExecutorTraceStepEvent,
    },
    /// One emitted output value.
    Output {
        /// Output event payload.
        output: TassadarExecutorOutputEvent,
    },
    /// Terminal completion or refusal.
    Terminal {
        /// Terminal payload.
        terminal: TassadarExecutorTerminalEvent,
    },
}

/// Typed request-validation or model-resolution error for the served executor lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorServiceError {
    /// The request targeted a different served product family.
    #[error("unsupported Tassadar served product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
    /// The request named an executor model that is not registered.
    #[error("unknown Tassadar executor model `{model_id}`")]
    UnknownModel {
        /// Requested model identifier.
        model_id: String,
    },
    /// The request reached execution even though runtime selection refused it.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
}

/// Pre-execution contract and selection report for one executor request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorPreflightReport {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor that evaluated the request.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Contract error when the request failed before decode selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Decode selection diagnostic when contract validation succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
}

/// Pull-driven local stream for explicit executor-trace products.
#[derive(Clone, Debug, Default)]
pub struct LocalTassadarExecutorStream {
    events: VecDeque<TassadarExecutorStreamEvent>,
}

impl LocalTassadarExecutorStream {
    fn from_events(events: Vec<TassadarExecutorStreamEvent>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }

    /// Returns the next typed stream event.
    pub fn next_event(&mut self) -> Option<TassadarExecutorStreamEvent> {
        self.events.pop_front()
    }
}

/// Local reference implementation of the explicit Tassadar served product.
#[derive(Clone, Debug)]
pub struct LocalTassadarExecutorService {
    fixtures: BTreeMap<String, TassadarExecutorFixture>,
    default_model_id: String,
}

impl Default for LocalTassadarExecutorService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTassadarExecutorService {
    /// Creates the default in-process Tassadar executor service.
    #[must_use]
    pub fn new() -> Self {
        let fixture = TassadarExecutorFixture::new();
        let model_id = fixture.descriptor().model.model_id.clone();
        let mut fixtures = BTreeMap::new();
        fixtures.insert(model_id.clone(), fixture);
        Self {
            fixtures,
            default_model_id: model_id,
        }
    }

    /// Registers one additional executor fixture.
    #[must_use]
    pub fn with_fixture(mut self, fixture: TassadarExecutorFixture) -> Self {
        let model_id = fixture.descriptor().model.model_id.clone();
        self.fixtures.insert(model_id.clone(), fixture);
        if self.fixtures.len() == 1 {
            self.default_model_id = model_id;
        }
        self
    }

    /// Executes one request through the explicit executor-trace surface.
    pub fn execute(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<TassadarExecutorOutcome, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let fixture = self.resolve_fixture(request)?;
        Ok(self.execute_with_fixture(fixture, request))
    }

    /// Starts a pull-driven explicit executor-trace stream.
    pub fn execute_stream(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<LocalTassadarExecutorStream, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let fixture = self.resolve_fixture(request)?;
        let outcome = self.execute_with_fixture(fixture, request);
        Ok(LocalTassadarExecutorStream::from_events(
            stream_events_for_outcome(fixture, outcome),
        ))
    }

    /// Resolves the explicit model and selection truth without executing the program.
    pub fn preflight(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<TassadarExecutorPreflightReport, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let fixture = self.resolve_fixture(request)?;
        let descriptor = fixture.descriptor().clone();
        let runtime_capability = fixture.runtime_capability_report();
        let contract_error = descriptor
            .validate_program_artifact(&request.program_artifact, request.requested_decode_mode)
            .err();
        let selection = if contract_error.is_none() {
            Some(fixture.runtime_selection_diagnostic(
                &request.program_artifact.validated_program,
                request.requested_decode_mode,
            ))
        } else {
            None
        };
        Ok(TassadarExecutorPreflightReport {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            model_descriptor: descriptor,
            runtime_capability,
            contract_error,
            selection,
        })
    }

    fn validate_product(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<(), TassadarExecutorServiceError> {
        if request.product_id == EXECUTOR_TRACE_PRODUCT_ID {
            Ok(())
        } else {
            Err(TassadarExecutorServiceError::UnsupportedProduct {
                product_id: request.product_id.clone(),
            })
        }
    }

    fn resolve_fixture(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<&TassadarExecutorFixture, TassadarExecutorServiceError> {
        let requested_model_id = request
            .requested_model_id
            .as_deref()
            .unwrap_or(self.default_model_id.as_str());
        self.fixtures.get(requested_model_id).ok_or_else(|| {
            TassadarExecutorServiceError::UnknownModel {
                model_id: requested_model_id.to_string(),
            }
        })
    }

    fn execute_with_fixture(
        &self,
        fixture: &TassadarExecutorFixture,
        request: &TassadarExecutorRequest,
    ) -> TassadarExecutorOutcome {
        let descriptor = fixture.descriptor().clone();
        let runtime_capability = fixture.runtime_capability_report();
        match descriptor
            .validate_program_artifact(&request.program_artifact, request.requested_decode_mode)
        {
            Ok(()) => {}
            Err(contract_error) => {
                return TassadarExecutorOutcome::Refused {
                    refusal: TassadarExecutorRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        model_descriptor: descriptor,
                        runtime_capability,
                        detail: contract_error.to_string(),
                        contract_error: Some(contract_error),
                        selection: None,
                    },
                };
            }
        }

        let selection = fixture.runtime_selection_diagnostic(
            &request.program_artifact.validated_program,
            request.requested_decode_mode,
        );
        if selection.effective_decode_mode.is_none() {
            return TassadarExecutorOutcome::Refused {
                refusal: TassadarExecutorRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: descriptor,
                    runtime_capability,
                    detail: selection.detail.clone(),
                    contract_error: None,
                    selection: Some(selection),
                },
            };
        }

        match execute_tassadar_executor_request(
            &request.program_artifact.validated_program,
            request.requested_decode_mode,
            request.program_artifact.trace_abi_version,
            Some(descriptor.compatibility.supported_decode_modes.as_slice()),
        ) {
            Ok(execution_report) => {
                let evidence_bundle = build_tassadar_execution_evidence_bundle(
                    request.request_id.clone(),
                    request.stable_digest(),
                    request.product_id.clone(),
                    descriptor.model.model_id.clone(),
                    descriptor.stable_digest(),
                    request.environment_refs.clone(),
                    &request.program_artifact,
                    execution_report
                        .selection
                        .effective_decode_mode
                        .expect("completed execution should surface an effective decode mode"),
                    &execution_report.execution,
                );
                TassadarExecutorOutcome::Completed {
                    response: TassadarExecutorResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        model_descriptor: descriptor,
                        runtime_capability,
                        execution_report,
                        evidence_bundle,
                        environment_refs: request.environment_refs.clone(),
                    },
                }
            }
            Err(selection) => TassadarExecutorOutcome::Refused {
                refusal: TassadarExecutorRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: descriptor,
                    runtime_capability,
                    detail: selection.detail.clone(),
                    contract_error: None,
                    selection: Some(selection),
                },
            },
        }
    }
}

/// Planner-visible fallback behavior when executor routing is not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerFallbackPolicy {
    /// Refuse the planner request rather than silently degrading it.
    Refuse,
    /// Return a typed planner fallback summary while preserving executor truth.
    PlannerSummary,
}

/// Planner-visible budget for one exact-computation subproblem.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingBudget {
    /// Maximum accepted validated-program length.
    pub max_program_len: usize,
    /// Maximum accepted trace-step budget inferred from the targeted profile.
    pub max_trace_steps: usize,
    /// Maximum accepted environment refs carried into lineage.
    pub max_environment_refs: usize,
}

impl TassadarPlannerRoutingBudget {
    /// Creates one explicit routing budget.
    #[must_use]
    pub fn new(max_program_len: usize, max_trace_steps: usize, max_environment_refs: usize) -> Self {
        Self {
            max_program_len,
            max_trace_steps,
            max_environment_refs,
        }
    }
}

impl Default for TassadarPlannerRoutingBudget {
    fn default() -> Self {
        Self::new(128, 512, 8)
    }
}

/// Planner-visible policy for exact executor delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingPolicy {
    /// Whether exact executor delegation is enabled at all.
    pub allow_executor_delegation: bool,
    /// Whether planner routing may accept runtime decode fallback.
    pub allow_runtime_decode_fallback: bool,
    /// Whether routing requires the requested decode path to remain direct.
    pub require_direct_decode: bool,
    /// Whether proof-bearing executor evidence must remain present on success.
    pub require_proof_bundle: bool,
    /// Typed behavior when executor routing is skipped or refused.
    pub fallback_policy: TassadarPlannerFallbackPolicy,
}

impl TassadarPlannerRoutingPolicy {
    /// Returns the canonical truthful planner routing policy.
    #[must_use]
    pub fn exact_executor_default() -> Self {
        Self {
            allow_executor_delegation: true,
            allow_runtime_decode_fallback: true,
            require_direct_decode: false,
            require_proof_bundle: true,
            fallback_policy: TassadarPlannerFallbackPolicy::Refuse,
        }
    }

    /// Replaces the fallback behavior.
    #[must_use]
    pub fn with_fallback_policy(mut self, fallback_policy: TassadarPlannerFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }
}

impl Default for TassadarPlannerRoutingPolicy {
    fn default() -> Self {
        Self::exact_executor_default()
    }
}

/// Planner-owned exact-computation subproblem routed into Tassadar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorSubproblem {
    /// Stable subproblem identifier.
    pub subproblem_id: String,
    /// Human-readable planner objective for the exact executor call.
    pub objective: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Digest-bound program artifact submitted to the executor.
    pub program_artifact: TassadarProgramArtifact,
    /// Requested decode mode for the exact executor path.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Ordered environment refs carried into executor lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarPlannerExecutorSubproblem {
    /// Creates one exact-computation subproblem.
    #[must_use]
    pub fn new(
        subproblem_id: impl Into<String>,
        objective: impl Into<String>,
        program_artifact: TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            subproblem_id: subproblem_id.into(),
            objective: objective.into(),
            requested_model_id: None,
            program_artifact,
            requested_decode_mode,
            environment_refs: Vec::new(),
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Carries environment refs into the executor lineage.
    #[must_use]
    pub fn with_environment_refs(mut self, mut environment_refs: Vec<String>) -> Self {
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }
}

/// Stable planner-owned request for exact executor delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingRequest {
    /// Stable planner request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.planner_executor_route`.
    pub product_id: String,
    /// Stable planner session identifier.
    pub planner_session_id: String,
    /// Planner model identifier making the exact-computation request.
    pub planner_model_id: String,
    /// Exact-computation subproblem to delegate.
    pub subproblem: TassadarPlannerExecutorSubproblem,
    /// Planner routing policy.
    pub routing_policy: TassadarPlannerRoutingPolicy,
    /// Planner routing budget.
    pub routing_budget: TassadarPlannerRoutingBudget,
}

impl TassadarPlannerRoutingRequest {
    /// Creates one planner-owned exact routing request.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        planner_session_id: impl Into<String>,
        planner_model_id: impl Into<String>,
        subproblem: TassadarPlannerExecutorSubproblem,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(PLANNER_EXECUTOR_ROUTE_PRODUCT_ID),
            planner_session_id: planner_session_id.into(),
            planner_model_id: planner_model_id.into(),
            subproblem,
            routing_policy: TassadarPlannerRoutingPolicy::default(),
            routing_budget: TassadarPlannerRoutingBudget::default(),
        }
    }

    /// Replaces the routing policy.
    #[must_use]
    pub fn with_routing_policy(mut self, routing_policy: TassadarPlannerRoutingPolicy) -> Self {
        self.routing_policy = routing_policy;
        self
    }

    /// Replaces the routing budget.
    #[must_use]
    pub fn with_routing_budget(mut self, routing_budget: TassadarPlannerRoutingBudget) -> Self {
        self.routing_budget = routing_budget;
        self
    }

    /// Returns a stable digest over the planner routing request.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_planner_routing_request|", self)
    }
}

/// Planner-visible route state after policy and executor resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerRouteState {
    /// The request delegated successfully into Tassadar.
    Delegated,
    /// The planner received an explicit typed fallback.
    PlannerFallback,
    /// The routing contract refused the request.
    Refused,
}

/// Planner-visible reason for a route decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerRouteReason {
    /// Planner policy disabled exact executor delegation.
    PlannerPolicyDisabled,
    /// Validated-program length exceeded planner budget.
    ProgramLengthBudgetExceeded,
    /// Profile-backed trace-step budget exceeded planner budget.
    TraceStepBudgetExceeded,
    /// Environment refs exceeded planner budget.
    EnvironmentRefBudgetExceeded,
    /// Planner policy disallowed runtime decode fallback.
    ExecutorDecodeFallbackDisallowed,
    /// Planner policy required a direct decode path.
    ExecutorDirectPathRequired,
    /// Executor model/program contract was invalid.
    ExecutorContractRejected,
    /// Executor selection refused the request before execution.
    ExecutorSelectionRefused,
    /// Executor service rejected the request before delegation.
    ExecutorServiceRejected,
}

/// Replay-stable planner routing decision with executor truth attached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingDecision {
    /// Stable digest for the planner request.
    pub planner_request_digest: String,
    /// Stable digest for the routing decision itself.
    pub routing_digest: String,
    /// Stable planner request identifier.
    pub planner_request_id: String,
    /// Stable planner session identifier.
    pub planner_session_id: String,
    /// Planner model identifier that requested the route.
    pub planner_model_id: String,
    /// Planner-owned product identifier.
    pub planner_product_id: String,
    /// Executor-trace product delegated to by the router.
    pub executor_product_id: String,
    /// Stable subproblem identifier.
    pub subproblem_id: String,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode after runtime selection, when execution remained viable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Stable digest for the executor request when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_request_digest: Option<String>,
    /// Route state exposed back to the planner.
    pub route_state: TassadarPlannerRouteState,
    /// Route reason when routing did not delegate directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_reason: Option<TassadarPlannerRouteReason>,
    /// Budget policy consulted during route selection.
    pub routing_budget: TassadarPlannerRoutingBudget,
    /// Fallback and decode policy consulted during route selection.
    pub routing_policy: TassadarPlannerRoutingPolicy,
    /// Runtime capability report preserved across the planner boundary.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Executor selection diagnostic preserved across the planner boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
    /// Contract error preserved when model/program pairing failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Human-readable route summary safe for logs or UI.
    pub detail: String,
}

impl TassadarPlannerRoutingDecision {
    fn new(
        request: &TassadarPlannerRoutingRequest,
        executor_request_digest: Option<String>,
        runtime_capability: TassadarRuntimeCapabilityReport,
        selection: Option<TassadarExecutorSelectionDiagnostic>,
        contract_error: Option<TassadarExecutorContractError>,
        route_state: TassadarPlannerRouteState,
        route_reason: Option<TassadarPlannerRouteReason>,
        detail: String,
    ) -> Self {
        #[derive(Serialize)]
        struct RoutingDigestInput<'a> {
            planner_request_digest: &'a str,
            planner_request_id: &'a str,
            planner_session_id: &'a str,
            planner_model_id: &'a str,
            planner_product_id: &'a str,
            subproblem_id: &'a str,
            requested_decode_mode: TassadarExecutorDecodeMode,
            effective_decode_mode: Option<TassadarExecutorDecodeMode>,
            executor_request_digest: Option<&'a str>,
            route_state: TassadarPlannerRouteState,
            route_reason: Option<TassadarPlannerRouteReason>,
            routing_budget: &'a TassadarPlannerRoutingBudget,
            routing_policy: &'a TassadarPlannerRoutingPolicy,
            runtime_capability: &'a TassadarRuntimeCapabilityReport,
            selection: &'a Option<TassadarExecutorSelectionDiagnostic>,
            contract_error: &'a Option<TassadarExecutorContractError>,
            detail: &'a str,
        }

        let planner_request_digest = request.stable_digest();
        let effective_decode_mode = selection.as_ref().and_then(|value| value.effective_decode_mode);
        let routing_digest = stable_digest(
            b"tassadar_planner_routing_decision|",
            &RoutingDigestInput {
                planner_request_digest: planner_request_digest.as_str(),
                planner_request_id: request.request_id.as_str(),
                planner_session_id: request.planner_session_id.as_str(),
                planner_model_id: request.planner_model_id.as_str(),
                planner_product_id: request.product_id.as_str(),
                subproblem_id: request.subproblem.subproblem_id.as_str(),
                requested_decode_mode: request.subproblem.requested_decode_mode,
                effective_decode_mode,
                executor_request_digest: executor_request_digest.as_deref(),
                route_state,
                route_reason,
                routing_budget: &request.routing_budget,
                routing_policy: &request.routing_policy,
                runtime_capability: &runtime_capability,
                selection: &selection,
                contract_error: &contract_error,
                detail: detail.as_str(),
            },
        );
        Self {
            planner_request_digest,
            routing_digest,
            planner_request_id: request.request_id.clone(),
            planner_session_id: request.planner_session_id.clone(),
            planner_model_id: request.planner_model_id.clone(),
            planner_product_id: request.product_id.clone(),
            executor_product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            subproblem_id: request.subproblem.subproblem_id.clone(),
            requested_decode_mode: request.subproblem.requested_decode_mode,
            effective_decode_mode,
            executor_request_digest,
            route_state,
            route_reason,
            routing_budget: request.routing_budget.clone(),
            routing_policy: request.routing_policy.clone(),
            runtime_capability,
            selection,
            contract_error,
            detail,
        }
    }
}

/// Planner-visible successful exact delegation result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerCompletedResponse {
    /// Replay-stable routing decision.
    pub routing_decision: TassadarPlannerRoutingDecision,
    /// Completed exact executor response with proof-bearing evidence.
    pub executor_response: TassadarExecutorResponse,
}

/// Planner-visible typed fallback preserving executor truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerFallbackResponse {
    /// Replay-stable routing decision.
    pub routing_decision: TassadarPlannerRoutingDecision,
    /// Human-readable planner fallback summary.
    pub fallback_summary: String,
    /// Executor refusal preserved when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_refusal: Option<TassadarExecutorRefusalResponse>,
}

/// Planner-visible typed refusal preserving executor truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRefusalResponse {
    /// Replay-stable routing decision.
    pub routing_decision: TassadarPlannerRoutingDecision,
    /// Human-readable refusal detail.
    pub detail: String,
    /// Executor refusal preserved when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_refusal: Option<TassadarExecutorRefusalResponse>,
}

/// Planner-visible outcome for one exact executor routing request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarPlannerRoutingOutcome {
    /// The planner delegated successfully into Tassadar.
    Completed {
        /// Completed exact routing response.
        response: TassadarPlannerCompletedResponse,
    },
    /// The planner received an explicit typed fallback instead of exact execution.
    Fallback {
        /// Typed fallback response.
        fallback: TassadarPlannerFallbackResponse,
    },
    /// The routing contract refused the request.
    Refused {
        /// Typed refusal response.
        refusal: TassadarPlannerRefusalResponse,
    },
}

/// Planner-owned request validation errors for hybrid exact routing.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarPlannerRouterError {
    /// The request targeted a different planner-owned product family.
    #[error("unsupported Tassadar planner routing product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
}

/// Local planner router that delegates exact subproblems into Tassadar.
#[derive(Clone, Debug, Default)]
pub struct LocalTassadarPlannerRouter {
    executor_service: LocalTassadarExecutorService,
}

impl LocalTassadarPlannerRouter {
    /// Creates the default local planner router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            executor_service: LocalTassadarExecutorService::new(),
        }
    }

    /// Replaces the backing local executor service.
    #[must_use]
    pub fn with_executor_service(
        mut self,
        executor_service: LocalTassadarExecutorService,
    ) -> Self {
        self.executor_service = executor_service;
        self
    }

    /// Routes one planner-owned exact subproblem into Tassadar when policy allows.
    pub fn route(
        &self,
        request: &TassadarPlannerRoutingRequest,
    ) -> Result<TassadarPlannerRoutingOutcome, TassadarPlannerRouterError> {
        self.validate_product(request)?;
        let executor_request = self.executor_request_for(request);
        let executor_request_digest = executor_request.stable_digest();
        let preflight = match self.executor_service.preflight(&executor_request) {
            Ok(preflight) => preflight,
            Err(TassadarExecutorServiceError::UnknownModel { model_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!("planner requested unknown Tassadar executor model `{model_id}`");
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ));
            }
            Err(TassadarExecutorServiceError::UnsupportedProduct { product_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail =
                    format!("planner delegated to unsupported Tassadar executor product `{product_id}`");
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ));
            }
            Err(TassadarExecutorServiceError::ExecutionRefusal(refusal)) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!(
                    "executor service rejected planner request before delegation: {refusal}"
                );
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ));
            }
        };

        if !request.routing_policy.allow_executor_delegation {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::PlannerPolicyDisabled,
                String::from("planner policy disabled exact Tassadar delegation"),
                None,
            ));
        }

        let program_len = request.subproblem.program_artifact.validated_program.instructions.len();
        if program_len > request.routing_budget.max_program_len {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::ProgramLengthBudgetExceeded,
                format!(
                    "validated program uses {} instructions which exceeds planner budget {}",
                    program_len, request.routing_budget.max_program_len
                ),
                None,
            ));
        }

        let conservative_trace_steps = route_trace_step_budget(&request.subproblem.program_artifact);
        if conservative_trace_steps > request.routing_budget.max_trace_steps {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::TraceStepBudgetExceeded,
                format!(
                    "profile-backed trace-step budget {} exceeds planner budget {}",
                    conservative_trace_steps, request.routing_budget.max_trace_steps
                ),
                None,
            ));
        }

        if request.subproblem.environment_refs.len() > request.routing_budget.max_environment_refs {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::EnvironmentRefBudgetExceeded,
                format!(
                    "environment ref count {} exceeds planner budget {}",
                    request.subproblem.environment_refs.len(),
                    request.routing_budget.max_environment_refs
                ),
                None,
            ));
        }

        if let Some(contract_error) = preflight.contract_error {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                Some(contract_error.clone()),
                TassadarPlannerRouteReason::ExecutorContractRejected,
                contract_error.to_string(),
                None,
            ));
        }

        let selection = preflight
            .selection
            .expect("preflight should include selection when contract validation succeeds");
        if selection.effective_decode_mode.is_none() {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                Some(selection.clone()),
                None,
                TassadarPlannerRouteReason::ExecutorSelectionRefused,
                selection.detail.clone(),
                None,
            ));
        }
        if !request.routing_policy.allow_runtime_decode_fallback && selection.is_fallback() {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                Some(selection.clone()),
                None,
                TassadarPlannerRouteReason::ExecutorDecodeFallbackDisallowed,
                format!(
                    "planner policy disallowed runtime decode fallback: {}",
                    selection.detail
                ),
                None,
            ));
        }
        if request.routing_policy.require_direct_decode && selection.is_fallback() {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                Some(selection.clone()),
                None,
                TassadarPlannerRouteReason::ExecutorDirectPathRequired,
                format!(
                    "planner policy required a direct decode path: {}",
                    selection.detail
                ),
                None,
            ));
        }

        match self.executor_service.execute(&executor_request) {
            Ok(TassadarExecutorOutcome::Completed { response }) => {
                if request.routing_policy.require_proof_bundle
                    && response.evidence_bundle.proof_bundle.product_id != EXECUTOR_TRACE_PRODUCT_ID
                {
                    let capability = response.runtime_capability.clone();
                    let selection = Some(response.execution_report.selection.clone());
                    return Ok(self.policy_terminal_outcome(
                        request,
                        Some(executor_request_digest),
                        capability,
                        selection,
                        None,
                        TassadarPlannerRouteReason::ExecutorServiceRejected,
                        String::from(
                            "completed executor response was missing the required proof-bearing product identity",
                        ),
                        None,
                    ));
                }
                let detail = format!(
                    "planner delegated exact subproblem `{}` into Tassadar via `{}`",
                    request.subproblem.subproblem_id, EXECUTOR_TRACE_PRODUCT_ID
                );
                let routing_decision = TassadarPlannerRoutingDecision::new(
                    request,
                    Some(executor_request_digest),
                    response.runtime_capability.clone(),
                    Some(response.execution_report.selection.clone()),
                    None,
                    TassadarPlannerRouteState::Delegated,
                    None,
                    detail,
                );
                Ok(TassadarPlannerRoutingOutcome::Completed {
                    response: TassadarPlannerCompletedResponse {
                        routing_decision,
                        executor_response: response,
                    },
                })
            }
            Ok(TassadarExecutorOutcome::Refused { refusal }) => Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                refusal.runtime_capability.clone(),
                refusal.selection.clone(),
                refusal.contract_error.clone(),
                TassadarPlannerRouteReason::ExecutorSelectionRefused,
                refusal.detail.clone(),
                Some(refusal),
            )),
            Err(TassadarExecutorServiceError::UnknownModel { model_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!("planner requested unknown Tassadar executor model `{model_id}`");
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ))
            }
            Err(TassadarExecutorServiceError::UnsupportedProduct { product_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail =
                    format!("planner delegated to unsupported Tassadar executor product `{product_id}`");
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ))
            }
            Err(TassadarExecutorServiceError::ExecutionRefusal(refusal)) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!(
                    "executor service rejected planner request before delegation: {refusal}"
                );
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ))
            }
        }
    }

    fn validate_product(
        &self,
        request: &TassadarPlannerRoutingRequest,
    ) -> Result<(), TassadarPlannerRouterError> {
        if request.product_id == PLANNER_EXECUTOR_ROUTE_PRODUCT_ID {
            Ok(())
        } else {
            Err(TassadarPlannerRouterError::UnsupportedProduct {
                product_id: request.product_id.clone(),
            })
        }
    }

    fn executor_request_for(
        &self,
        request: &TassadarPlannerRoutingRequest,
    ) -> TassadarExecutorRequest {
        let mut executor_request = TassadarExecutorRequest::new(
            format!("{}::{}", request.request_id, request.subproblem.subproblem_id),
            request.subproblem.program_artifact.clone(),
            request.subproblem.requested_decode_mode,
        )
        .with_environment_refs(request.subproblem.environment_refs.clone());
        if let Some(requested_model_id) = request.subproblem.requested_model_id.as_deref() {
            executor_request = executor_request.with_requested_model_id(requested_model_id);
        }
        executor_request
    }

    fn policy_terminal_outcome(
        &self,
        request: &TassadarPlannerRoutingRequest,
        executor_request_digest: Option<String>,
        runtime_capability: TassadarRuntimeCapabilityReport,
        selection: Option<TassadarExecutorSelectionDiagnostic>,
        contract_error: Option<TassadarExecutorContractError>,
        route_reason: TassadarPlannerRouteReason,
        detail: String,
        executor_refusal: Option<TassadarExecutorRefusalResponse>,
    ) -> TassadarPlannerRoutingOutcome {
        let route_state = match request.routing_policy.fallback_policy {
            TassadarPlannerFallbackPolicy::Refuse => TassadarPlannerRouteState::Refused,
            TassadarPlannerFallbackPolicy::PlannerSummary => TassadarPlannerRouteState::PlannerFallback,
        };
        let routing_decision = TassadarPlannerRoutingDecision::new(
            request,
            executor_request_digest,
            runtime_capability,
            selection,
            contract_error,
            route_state,
            Some(route_reason),
            detail.clone(),
        );
        match request.routing_policy.fallback_policy {
            TassadarPlannerFallbackPolicy::Refuse => TassadarPlannerRoutingOutcome::Refused {
                refusal: TassadarPlannerRefusalResponse {
                    routing_decision,
                    detail,
                    executor_refusal,
                },
            },
            TassadarPlannerFallbackPolicy::PlannerSummary => {
                TassadarPlannerRoutingOutcome::Fallback {
                    fallback: TassadarPlannerFallbackResponse {
                        routing_decision,
                        fallback_summary: detail,
                        executor_refusal,
                    },
                }
            }
        }
    }
}

fn route_trace_step_budget(program_artifact: &TassadarProgramArtifact) -> usize {
    tassadar_wasm_profile_for_id(program_artifact.wasm_profile_id.as_str())
        .map_or_else(
            || program_artifact.validated_program.instructions.len(),
            |profile| profile.max_steps,
        )
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar planner routing value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn stream_events_for_outcome(
    fixture: &TassadarExecutorFixture,
    outcome: TassadarExecutorOutcome,
) -> Vec<TassadarExecutorStreamEvent> {
    let mut events = vec![TassadarExecutorStreamEvent::Capability {
        runtime_capability: fixture.runtime_capability_report(),
    }];
    match &outcome {
        TassadarExecutorOutcome::Completed { response } => {
            events.push(TassadarExecutorStreamEvent::Selection {
                selection: response.execution_report.selection.clone(),
            });
            let mut output_ordinal = 0usize;
            for step in &response.execution_report.execution.steps {
                events.push(TassadarExecutorStreamEvent::TraceStep {
                    trace_step: TassadarExecutorTraceStepEvent {
                        step_index: step.step_index as u64,
                        step: step.clone(),
                    },
                });
                if let TassadarTraceEvent::Output { value } = step.event {
                    events.push(TassadarExecutorStreamEvent::Output {
                        output: TassadarExecutorOutputEvent {
                            ordinal: output_ordinal,
                            value,
                        },
                    });
                    output_ordinal += 1;
                }
            }
        }
        TassadarExecutorOutcome::Refused { refusal } => {
            if let Some(selection) = &refusal.selection {
                events.push(TassadarExecutorStreamEvent::Selection {
                    selection: selection.clone(),
                });
            }
        }
    }
    events.push(TassadarExecutorStreamEvent::Terminal {
        terminal: TassadarExecutorTerminalEvent { outcome },
    });
    events
}

#[cfg(test)]
mod tests {
    use psionic_models::TassadarExecutorFixture;
    use super::{
        EXECUTOR_TRACE_PRODUCT_ID, LocalTassadarExecutorService, LocalTassadarPlannerRouter,
        PLANNER_EXECUTOR_ROUTE_PRODUCT_ID, TassadarExecutorOutcome, TassadarExecutorRequest,
        TassadarExecutorServiceError, TassadarExecutorStreamEvent,
        TassadarPlannerExecutorSubproblem, TassadarPlannerFallbackPolicy,
        TassadarPlannerRouteReason, TassadarPlannerRouterError, TassadarPlannerRoutingBudget,
        TassadarPlannerRoutingOutcome, TassadarPlannerRoutingPolicy, TassadarPlannerRoutingRequest,
    };
    use psionic_runtime::{
        TassadarExecutorDecodeMode, TassadarInstruction, TassadarProgram, TassadarProgramArtifact,
        TassadarTraceAbi, TassadarWasmProfile, tassadar_validation_corpus,
    };

    fn request_for_case(case_id: &str) -> TassadarExecutorRequest {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .find(|case| case.case_id == case_id)
            .expect("requested validation case should exist");
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("artifact://tassadar/{case_id}"),
            &profile,
            &trace_abi,
            case.program,
        )
        .expect("fixture artifact should build");
        TassadarExecutorRequest::new(
            format!("request-{case_id}"),
            artifact,
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_environment_refs(vec![String::from("env.openagents.tassadar.benchmark")])
    }

    #[test]
    fn executor_service_executes_completed_request_with_explicit_product_semantics() {
        let service = LocalTassadarExecutorService::new();
        let request = request_for_case("locals_add");
        let outcome = service.execute(&request).expect("request should execute");

        match outcome {
            TassadarExecutorOutcome::Completed { response } => {
                assert_eq!(response.product_id, EXECUTOR_TRACE_PRODUCT_ID);
                assert_eq!(response.final_outputs(), &[12]);
                assert_eq!(
                    response.execution_report.selection.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert_eq!(
                    response.evidence_bundle.proof_bundle.product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response.trace_artifact().program_id,
                    "tassadar.locals_add.v1"
                );
            }
            TassadarExecutorOutcome::Refused { refusal } => {
                panic!("request should not be refused: {}", refusal.detail);
            }
        }
    }

    #[test]
    fn executor_service_returns_explicit_refusal_for_contract_mismatch() {
        let service = LocalTassadarExecutorService::new();
        let mut request = request_for_case("locals_add");
        request.program_artifact.trace_abi_version += 1;

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarExecutorOutcome::Completed { .. } => {
                panic!("mismatched ABI should not complete");
            }
            TassadarExecutorOutcome::Refused { refusal } => {
                assert!(refusal.contract_error.is_some());
                assert!(refusal.selection.is_none());
            }
        }
    }

    #[test]
    fn executor_stream_surfaces_capability_selection_trace_and_terminal() {
        let service = LocalTassadarExecutorService::new();
        let request = request_for_case("memory_roundtrip");
        let mut stream = service
            .execute_stream(&request)
            .expect("stream should be created");

        let first = stream.next_event().expect("capability event");
        assert!(matches!(
            first,
            TassadarExecutorStreamEvent::Capability { .. }
        ));
        let second = stream.next_event().expect("selection event");
        assert!(matches!(
            second,
            TassadarExecutorStreamEvent::Selection { .. }
        ));

        let mut saw_trace = false;
        let mut saw_terminal = false;
        while let Some(event) = stream.next_event() {
            match event {
                TassadarExecutorStreamEvent::TraceStep { .. } => saw_trace = true,
                TassadarExecutorStreamEvent::Terminal { .. } => {
                    saw_terminal = true;
                    break;
                }
                TassadarExecutorStreamEvent::Output { .. }
                | TassadarExecutorStreamEvent::Capability { .. }
                | TassadarExecutorStreamEvent::Selection { .. } => {}
            }
        }

        assert!(saw_trace);
        assert!(saw_terminal);
    }

    #[test]
    fn executor_service_rejects_non_executor_product_id() {
        let service = LocalTassadarExecutorService::new();
        let mut request = request_for_case("branch_guard");
        request.product_id = String::from("psionic.text_generation");

        let error = service
            .execute(&request)
            .expect_err("wrong product should fail before execution");
        assert_eq!(
            error,
            TassadarExecutorServiceError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }

    fn planner_request_for_case(case_id: &str) -> TassadarPlannerRoutingRequest {
        TassadarPlannerRoutingRequest::new(
            format!("planner-request-{case_id}"),
            "session-alpha",
            "planner-fixture-v0",
            TassadarPlannerExecutorSubproblem::new(
                format!("subproblem-{case_id}"),
                "exact arithmetic subproblem",
                request_for_case(case_id).program_artifact,
                TassadarExecutorDecodeMode::HullCache,
            )
            .with_environment_refs(vec![String::from("env.openagents.tassadar.benchmark")]),
        )
    }

    fn backward_branch_sparse_artifact() -> TassadarProgramArtifact {
        let profile = TassadarWasmProfile::core_i32_v2();
        let trace_abi = TassadarTraceAbi::core_i32_v2();
        let program = TassadarProgram::new(
            "tassadar.backward_branch_sparse.v1",
            &profile,
            1,
            0,
            vec![
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::BrIf { target_pc: 2 },
                TassadarInstruction::I32Const { value: 9 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        );
        TassadarProgramArtifact::fixture_reference(
            "artifact://tassadar/backward_branch_sparse",
            &profile,
            &trace_abi,
            program,
        )
        .expect("backward branch fixture should build")
    }

    #[test]
    fn planner_router_delegates_completed_exact_request_with_full_executor_truth() {
        let router = LocalTassadarPlannerRouter::new();
        let request = planner_request_for_case("locals_add");

        let outcome = router.route(&request).expect("planner route should succeed");
        match outcome {
            TassadarPlannerRoutingOutcome::Completed { response } => {
                assert_eq!(
                    response.routing_decision.planner_product_id,
                    PLANNER_EXECUTOR_ROUTE_PRODUCT_ID
                );
                assert_eq!(
                    response.routing_decision.executor_product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(response.executor_response.final_outputs(), &[12]);
                assert_eq!(
                    response.executor_response.evidence_bundle.proof_bundle.product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response.routing_decision.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert!(response.routing_decision.selection.is_some());
                assert!(!response.routing_decision.routing_digest.is_empty());
            }
            other => panic!("expected delegated completion, got {other:?}"),
        }
    }

    #[test]
    fn planner_router_can_return_typed_fallback_when_policy_disallows_runtime_decode_fallback() {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new().with_fixture(TassadarExecutorFixture::core_i32_v2()),
        );
        let request = TassadarPlannerRoutingRequest::new(
            "planner-request-sparse-fallback",
            "session-beta",
            "planner-fixture-v0",
            TassadarPlannerExecutorSubproblem::new(
                "subproblem-sparse-fallback",
                "exact sparse top-k subproblem",
                backward_branch_sparse_artifact(),
                TassadarExecutorDecodeMode::SparseTopK,
            ),
        )
        .with_routing_budget(TassadarPlannerRoutingBudget::new(128, 512, 8))
        .with_routing_policy(
            TassadarPlannerRoutingPolicy::exact_executor_default()
                .with_fallback_policy(TassadarPlannerFallbackPolicy::PlannerSummary),
        );
        let request = TassadarPlannerRoutingRequest {
            subproblem: request
                .subproblem
                .with_requested_model_id(TassadarExecutorFixture::ARTICLE_CLASS_MODEL_ID),
            routing_policy: TassadarPlannerRoutingPolicy {
                allow_runtime_decode_fallback: false,
                ..request.routing_policy
            },
            ..request
        };

        let outcome = router.route(&request).expect("planner route should be typed");
        match outcome {
            TassadarPlannerRoutingOutcome::Fallback { fallback } => {
                assert_eq!(
                    fallback.routing_decision.route_reason,
                    Some(TassadarPlannerRouteReason::ExecutorDecodeFallbackDisallowed)
                );
                assert!(fallback
                    .routing_decision
                    .selection
                    .as_ref()
                    .is_some_and(|selection| selection.is_fallback()));
                assert!(fallback.fallback_summary.contains("disallowed"));
            }
            other => panic!("expected typed fallback, got {other:?}"),
        }
    }

    #[test]
    fn planner_router_refuses_when_program_exceeds_budget() {
        let router = LocalTassadarPlannerRouter::new();
        let request = planner_request_for_case("memory_roundtrip").with_routing_budget(
            TassadarPlannerRoutingBudget::new(4, 512, 8),
        );

        let outcome = router.route(&request).expect("planner route should be typed");
        match outcome {
            TassadarPlannerRoutingOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.routing_decision.route_reason,
                    Some(TassadarPlannerRouteReason::ProgramLengthBudgetExceeded)
                );
                assert!(refusal.detail.contains("exceeds planner budget"));
            }
            other => panic!("expected budget refusal, got {other:?}"),
        }
    }

    #[test]
    fn planner_router_rejects_non_planner_product_id() {
        let router = LocalTassadarPlannerRouter::new();
        let mut request = planner_request_for_case("locals_add");
        request.product_id = String::from("psionic.text_generation");

        let error = router
            .route(&request)
            .expect_err("wrong planner product should fail before routing");
        assert_eq!(
            error,
            TassadarPlannerRouterError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }
}
