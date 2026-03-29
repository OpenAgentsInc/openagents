use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_compiled_agent::{
    AgentRoute, CompiledAgentExecutor, CompiledAgentRun, CompiledModuleManifest,
    ConfidenceFallbackPolicy, FirstGraphModuleHub, GroundedAnswerInput, GroundedAnswerOutput,
    GraphAuthority, GroundedAnswerSignature, IntentRouteInput, IntentRouteOutput,
    IntentRouteSignature, ModuleFamilyHub, ModulePromotionState, ModuleRun, PublicOutcomeKind,
    ShadowMode, ToolArgumentsInput, ToolArgumentsOutput, ToolArgumentsSignature, ToolCall,
    ToolExecutor, ToolPolicyInput, ToolPolicyOutput, ToolPolicySignature, ToolResult, ToolSpec,
    TypedModule, VerifyInput, VerifyOutput, VerifySignature, VerifyVerdict,
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
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const FIRST_GRAPH_COMPATIBILITY_VERSION: &str = "openagents.compiled_agent.first_graph.v1";
const COMPILED_AGENT_SLICE_TASK_FAMILY: &str = "compiled_agent.first_graph.admitted_family.v1";
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

/// Authority path used for the user-visible answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentAuthorityPath {
    /// Promoted contract artifacts produced the final answer.
    Promoted,
    /// Confidence policy forced a safe fallback instead of bluffing.
    Fallback,
    /// A rollback-safe candidate graph produced the final answer.
    Rollback,
}

/// Human-readable confidence band for runtime observability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentConfidenceBand {
    Low,
    Medium,
    High,
}

/// Evidence class preserved by the current narrow runtime lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentReceiptEvidenceClass {
    LearnedLane,
}

/// Stable lineage identifiers that correlate runtime telemetry with Psionic-ledger truth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRuntimeLineage {
    /// Stable runtime receipt id for this exact answered request.
    pub receipt_id: String,
    /// Promoted-artifact contract row id consumed by the runtime.
    pub contract_row_id: String,
    /// Promoted-artifact contract digest consumed by the runtime.
    pub contract_digest: String,
    /// Manifest ids executed by the authoritative graph.
    pub authority_manifest_ids: Vec<String>,
    /// Manifest ids executed by the shadow graph, when enabled.
    pub shadow_manifest_ids: Vec<String>,
    /// Artifact ids executed by the authoritative graph.
    pub authority_artifact_ids: Vec<String>,
    /// Artifact ids executed by the shadow graph, when enabled.
    pub shadow_artifact_ids: Vec<String>,
    /// Artifact row ids executed by the authoritative graph.
    pub authority_row_ids: Vec<String>,
    /// Artifact row ids executed by the shadow graph, when enabled.
    pub shadow_row_ids: Vec<String>,
}

/// Runtime-visible artifact provenance for one phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentArtifactTelemetry {
    /// Executed phase name.
    pub phase: String,
    /// Whether the phase came from promoted or candidate authority.
    pub authority: GraphAuthority,
    /// Candidate label when applicable.
    pub candidate_label: Option<String>,
    /// Stable manifest id.
    pub manifest_id: String,
    /// Artifact id consumed by this phase when available.
    pub artifact_id: Option<String>,
    /// Artifact digest consumed by this phase when available.
    pub artifact_digest: Option<String>,
    /// Artifact row id consumed by this phase when available.
    pub artifact_row_id: Option<String>,
    /// Revision or version used for this phase.
    pub revision: String,
    /// Raw confidence value claimed by this phase.
    pub confidence: f32,
    /// Human-readable confidence band for normal telemetry.
    pub confidence_band: CompiledAgentConfidenceBand,
    /// Rollback-safe artifact id when one is retained.
    pub rollback_artifact_id: Option<String>,
}

/// First-class retained disagreement between promoted and candidate execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentShadowDisagreement {
    /// Phase that diverged between promoted and candidate execution.
    pub phase: String,
    /// Candidate label used for the shadow run.
    pub candidate_label: String,
    /// Promoted manifest id for the disagreement.
    pub promoted_manifest_id: String,
    /// Candidate manifest id for the disagreement.
    pub candidate_manifest_id: String,
    /// Promoted output retained for review and replay.
    pub promoted_output: Value,
    /// Candidate output retained for review and replay.
    pub candidate_output: Value,
    /// Promoted confidence value.
    pub promoted_confidence: f32,
    /// Candidate confidence value.
    pub candidate_confidence: f32,
    /// Promoted confidence band.
    pub promoted_confidence_band: CompiledAgentConfidenceBand,
    /// Candidate confidence band.
    pub candidate_confidence_band: CompiledAgentConfidenceBand,
    /// Whether this disagreement should trigger human review before promotion.
    pub review_required: bool,
}

/// User or operator disagreement retained alongside artifact lineage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentFeedbackSignal {
    /// Whether the user explicitly disagreed with the returned answer.
    pub disagreed: bool,
    /// Optional correction or preferred answer text.
    pub correction_text: Option<String>,
    /// Optional reason code for downstream review buckets.
    pub reason_code: Option<String>,
    /// Optional operator note preserved with the receipt.
    pub operator_note: Option<String>,
}

/// Operator-visible runtime telemetry for the admitted compiled-agent slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRuntimeTelemetry {
    /// Stable runtime family identifier for this vertical slice.
    pub runtime_family: String,
    /// Admitted task family identifier.
    pub admitted_task_family: String,
    /// Evidence class carried into the governed Psionic ledger.
    pub evidence_class: CompiledAgentReceiptEvidenceClass,
    /// Whether promoted, fallback, or rollback authority produced the answer.
    pub authority_path: CompiledAgentAuthorityPath,
    /// Final route produced by the authoritative graph.
    pub route: AgentRoute,
    /// User-visible outcome kind.
    pub public_outcome_kind: PublicOutcomeKind,
    /// Lowest retained primary-phase confidence.
    pub primary_confidence: f32,
    /// Human-readable band for the primary confidence.
    pub primary_confidence_band: CompiledAgentConfidenceBand,
    /// Per-phase promoted or rollback authority provenance.
    pub promoted_authority: Vec<CompiledAgentArtifactTelemetry>,
    /// Per-phase shadow provenance when comparison is enabled.
    pub shadow_candidates: Vec<CompiledAgentArtifactTelemetry>,
    /// Retained promoted-versus-candidate disagreements.
    pub shadow_disagreements: Vec<CompiledAgentShadowDisagreement>,
    /// Stable lineage ids that bridge runtime telemetry to Psionic governance.
    pub lineage: CompiledAgentRuntimeLineage,
    /// Optional disagreement or correction signal attached after the answer.
    pub feedback: Option<CompiledAgentFeedbackSignal>,
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
    /// Normal runtime telemetry retained without requiring raw trace inspection.
    pub telemetry: CompiledAgentRuntimeTelemetry,
}

impl CompiledAgentSliceReceipt {
    /// Attach a retained disagreement or correction signal to an existing receipt.
    #[must_use]
    pub fn with_feedback(mut self, feedback: CompiledAgentFeedbackSignal) -> Self {
        self.telemetry.feedback = Some(feedback);
        self
    }
}

/// Execute the first narrow app-owned compiled-agent path.
#[must_use]
pub fn run_compiled_agent_slice(
    prompt: &str,
    state: &CompiledAgentSliceState,
    shadow_mode: ShadowMode,
) -> CompiledAgentSliceReceipt {
    run_compiled_agent_slice_with_feedback(prompt, state, shadow_mode, None)
}

/// Execute the compiled-agent path and optionally retain user disagreement signals.
#[must_use]
pub fn run_compiled_agent_slice_with_feedback(
    prompt: &str,
    state: &CompiledAgentSliceState,
    shadow_mode: ShadowMode,
    feedback: Option<CompiledAgentFeedbackSignal>,
) -> CompiledAgentSliceReceipt {
    let contract = canonical_compiled_agent_promoted_artifact_contract()
        .expect("psionic compiled-agent promoted-artifact contract should load");
    let executor = CompiledAgentExecutor::new(
        build_module_hub_from_contract(&contract).expect(
            "compiled-agent promoted-artifact contract should be compatible with the first graph",
        ),
        fallback_policy(),
    );
    let runtime = SliceToolRuntime {
        state: state.clone(),
    };
    let run = executor.run(prompt, &supported_tools(), &runtime, shadow_mode);
    let captured_at_epoch_ms = now_epoch_ms();
    CompiledAgentSliceReceipt {
        schema_version: 1,
        captured_at_epoch_ms,
        state: state.clone(),
        telemetry: build_runtime_telemetry(
            &contract,
            captured_at_epoch_ms,
            state,
            &run,
            feedback.clone(),
        ),
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

fn build_runtime_telemetry(
    contract: &CompiledAgentPromotedArtifactContract,
    captured_at_epoch_ms: u64,
    state: &CompiledAgentSliceState,
    run: &CompiledAgentRun,
    feedback: Option<CompiledAgentFeedbackSignal>,
) -> CompiledAgentRuntimeTelemetry {
    let promoted_authority = run
        .internal_trace
        .primary_phases
        .iter()
        .map(phase_telemetry_from_entry)
        .collect::<Vec<_>>();
    let shadow_candidates = run
        .internal_trace
        .shadow_phases
        .iter()
        .map(phase_telemetry_from_entry)
        .collect::<Vec<_>>();
    let authority_path = classify_authority_path(run);
    let lineage = build_runtime_lineage(contract, captured_at_epoch_ms, state, run);

    CompiledAgentRuntimeTelemetry {
        runtime_family: FIRST_GRAPH_COMPATIBILITY_VERSION.to_string(),
        admitted_task_family: COMPILED_AGENT_SLICE_TASK_FAMILY.to_string(),
        evidence_class: CompiledAgentReceiptEvidenceClass::LearnedLane,
        authority_path,
        route: run.lineage.route,
        public_outcome_kind: run.public_response.kind,
        primary_confidence: primary_confidence(run),
        primary_confidence_band: confidence_band(primary_confidence(run)),
        promoted_authority,
        shadow_candidates,
        shadow_disagreements: shadow_disagreements(run),
        lineage,
        feedback,
    }
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

fn build_runtime_lineage(
    contract: &CompiledAgentPromotedArtifactContract,
    captured_at_epoch_ms: u64,
    state: &CompiledAgentSliceState,
    run: &CompiledAgentRun,
) -> CompiledAgentRuntimeLineage {
    let authority_artifact_ids = run
        .lineage
        .authority_manifests
        .iter()
        .filter_map(|manifest| manifest.artifact_id.clone())
        .collect::<Vec<_>>();
    let shadow_artifact_ids = run
        .lineage
        .shadow_manifests
        .iter()
        .filter_map(|manifest| manifest.artifact_id.clone())
        .collect::<Vec<_>>();
    let authority_row_ids = run
        .lineage
        .authority_manifests
        .iter()
        .filter_map(|manifest| manifest.row_id.clone())
        .collect::<Vec<_>>();
    let shadow_row_ids = run
        .lineage
        .shadow_manifests
        .iter()
        .filter_map(|manifest| manifest.row_id.clone())
        .collect::<Vec<_>>();

    CompiledAgentRuntimeLineage {
        receipt_id: runtime_receipt_id(contract, captured_at_epoch_ms, state, run),
        contract_row_id: contract.row_id.clone(),
        contract_digest: contract.contract_digest.clone(),
        authority_manifest_ids: run.lineage.authority_manifest_ids.clone(),
        shadow_manifest_ids: run.lineage.shadow_manifest_ids.clone(),
        authority_artifact_ids,
        shadow_artifact_ids,
        authority_row_ids,
        shadow_row_ids,
    }
}

fn runtime_receipt_id(
    contract: &CompiledAgentPromotedArtifactContract,
    captured_at_epoch_ms: u64,
    state: &CompiledAgentSliceState,
    run: &CompiledAgentRun,
) -> String {
    let payload = json!({
        "runtime_family": FIRST_GRAPH_COMPATIBILITY_VERSION,
        "task_family": COMPILED_AGENT_SLICE_TASK_FAMILY,
        "contract_row_id": contract.row_id,
        "contract_digest": contract.contract_digest,
        "captured_at_epoch_ms": captured_at_epoch_ms,
        "state": state,
        "user_request": run.lineage.user_request,
        "public_response": run.public_response,
        "authority_manifest_ids": run.lineage.authority_manifest_ids,
        "shadow_manifest_ids": run.lineage.shadow_manifest_ids,
    });
    let encoded = serde_json::to_vec(&payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    format!("oa-ca-{}", hex::encode(hasher.finalize()))
}

fn classify_authority_path(run: &CompiledAgentRun) -> CompiledAgentAuthorityPath {
    if run.public_response.kind == PublicOutcomeKind::ConfidenceFallback {
        CompiledAgentAuthorityPath::Fallback
    } else if run
        .internal_trace
        .primary_phases
        .iter()
        .any(|phase| phase.authority == GraphAuthority::Candidate)
    {
        CompiledAgentAuthorityPath::Rollback
    } else {
        CompiledAgentAuthorityPath::Promoted
    }
}

fn phase_telemetry_from_entry(entry: &openagents_compiled_agent::PhaseTraceEntry) -> CompiledAgentArtifactTelemetry {
    CompiledAgentArtifactTelemetry {
        phase: entry.phase.clone(),
        authority: entry.authority,
        candidate_label: entry.candidate_label.clone(),
        manifest_id: entry.manifest.manifest_id(),
        artifact_id: entry.manifest.artifact_id.clone(),
        artifact_digest: entry.manifest.artifact_digest.clone(),
        artifact_row_id: entry.manifest.row_id.clone(),
        revision: revision_for_phase(entry),
        confidence: entry.confidence,
        confidence_band: confidence_band(entry.confidence),
        rollback_artifact_id: entry.manifest.rollback_artifact_id.clone(),
    }
}

fn revision_for_phase(entry: &openagents_compiled_agent::PhaseTraceEntry) -> String {
    entry.trace
        .get("revision_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| entry.manifest.version.clone())
}

fn confidence_band(confidence: f32) -> CompiledAgentConfidenceBand {
    if confidence < 0.8 {
        CompiledAgentConfidenceBand::Low
    } else if confidence < 0.9 {
        CompiledAgentConfidenceBand::Medium
    } else {
        CompiledAgentConfidenceBand::High
    }
}

fn primary_confidence(run: &CompiledAgentRun) -> f32 {
    run.internal_trace
        .primary_phases
        .iter()
        .map(|phase| phase.confidence)
        .reduce(f32::min)
        .unwrap_or(0.0)
}

fn shadow_disagreements(run: &CompiledAgentRun) -> Vec<CompiledAgentShadowDisagreement> {
    run.internal_trace
        .primary_phases
        .iter()
        .filter_map(|primary| {
            let candidate = run
                .internal_trace
                .shadow_phases
                .iter()
                .find(|shadow| shadow.phase == primary.phase)?;
            if primary.output == candidate.output
                && confidence_band(primary.confidence) == confidence_band(candidate.confidence)
            {
                return None;
            }

            Some(CompiledAgentShadowDisagreement {
                phase: primary.phase.clone(),
                candidate_label: candidate
                    .candidate_label
                    .clone()
                    .unwrap_or_else(|| String::from("shadow_candidate")),
                promoted_manifest_id: primary.manifest.manifest_id(),
                candidate_manifest_id: candidate.manifest.manifest_id(),
                promoted_output: primary.output.clone(),
                candidate_output: candidate.output.clone(),
                promoted_confidence: primary.confidence,
                candidate_confidence: candidate.confidence,
                promoted_confidence_band: confidence_band(primary.confidence),
                candidate_confidence_band: confidence_band(candidate.confidence),
                review_required: primary.output != candidate.output,
            })
        })
        .collect()
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

    use super::{
        CompiledAgentAuthorityPath, CompiledAgentFeedbackSignal,
        CompiledAgentReceiptEvidenceClass, CompiledAgentSliceState, run_compiled_agent_slice,
    };

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
        assert_eq!(
            receipt.telemetry.evidence_class,
            CompiledAgentReceiptEvidenceClass::LearnedLane
        );
        assert_eq!(
            receipt.telemetry.authority_path,
            CompiledAgentAuthorityPath::Promoted
        );
        assert!(receipt.telemetry.lineage.receipt_id.starts_with("oa-ca-"));
        assert!(!receipt.telemetry.lineage.contract_digest.is_empty());
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
        assert!(
            receipt
                .telemetry
                .promoted_authority
                .iter()
                .all(|phase| phase.artifact_id.is_some())
        );
        assert!(!receipt.telemetry.lineage.authority_row_ids.is_empty());
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
        assert_eq!(
            receipt.telemetry.authority_path,
            CompiledAgentAuthorityPath::Promoted
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
        assert!(receipt
            .telemetry
            .shadow_disagreements
            .iter()
            .any(|entry| entry.phase == "grounded_answer" && entry.review_required));
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
        assert_eq!(
            receipt.telemetry.authority_path,
            CompiledAgentAuthorityPath::Rollback
        );
    }

    #[test]
    fn feedback_signal_stays_correlated_to_runtime_lineage() {
        let receipt = run_compiled_agent_slice(
            "How many sats are in the wallet?",
            &CompiledAgentSliceState::default(),
            ShadowMode::Disabled,
        )
        .with_feedback(CompiledAgentFeedbackSignal {
            disagreed: true,
            correction_text: Some("Use the wallet balance plus recent earnings phrasing.".to_string()),
            reason_code: Some("grounded_synthesis_drift".to_string()),
            operator_note: Some("Retain this for governed runtime ingestion.".to_string()),
        });

        let feedback = receipt
            .telemetry
            .feedback
            .as_ref()
            .expect("feedback should be retained");
        assert!(feedback.disagreed);
        assert_eq!(
            feedback.reason_code.as_deref(),
            Some("grounded_synthesis_drift")
        );
        assert!(receipt.telemetry.lineage.receipt_id.starts_with("oa-ca-"));
    }
}
