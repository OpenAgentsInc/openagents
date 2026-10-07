use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    validate_probe_gepa_candidate_manifest, ProbeGepaCandidateManifest,
    ProbeGepaCandidateManifestError, ProbeGepaCandidatePromotionState, ProbeGepaPolicyGateState,
};

pub const PROBE_GEPA_COORDINATOR_STATE_SCHEMA_VERSION: &str =
    "psionic.probe_gepa_rollout_coordinator_state.v1";
pub const PROBE_GEPA_ROLLOUT_ASSIGNMENT_SCHEMA_VERSION: &str =
    "psionic.probe_gepa_rollout_assignment.v1";
pub const PROBE_GEPA_ROLLOUT_RESULT_SCHEMA_VERSION: &str = "psionic.probe_gepa_rollout_result.v1";
pub const PROBE_GEPA_LIVE_CLOSEOUT_IMPORT_SCHEMA_VERSION: &str =
    "psionic.probe_gepa_live_closeout_import.v1";
pub const PROBE_GEPA_LIVE_CLOSEOUT_IMPORT_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.probe_gepa_live_closeout_import_receipt.v1";
pub const PROBE_GEPA_STAGE_0_CAMPAIGN_ID: &str = "probe_gepa.terminal_bench.stage_0_1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaEvaluatorBackend {
    Local,
    PylonPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaVerifierStatus {
    Passed,
    Failed,
    TimedOut,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaRolloutStatus {
    Succeeded,
    AgentFailed,
    InfrastructureFailed,
    PolicyBlocked,
    TimedOut,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaFailureClass {
    None,
    ServiceReadiness,
    ParserCorrectness,
    RunnerSupervision,
    AgentRegression,
    InfrastructureUnavailable,
    PolicyViolation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaLivePaymentMode {
    UnpaidSmoke,
    OperatorCredit,
    PayablePendingSettlement,
    SettledBitcoin,
    RejectedNoPay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaLiveCloseoutState {
    Accepted,
    Rejected,
    InfrastructureFailure,
    AgentFailure,
    TimedOut,
    PolicyBlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaLivePublicClaim {
    None,
    RetainedSmoke,
    RetainedSummary,
    ValidationMeasured,
    PublicBenchmarkScore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaRolloutTask {
    pub task_id: String,
    pub dataset: String,
    pub split_ref: String,
    pub split: String,
    pub expected_failure_family: ProbeGepaFailureClass,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub scorer_verifier_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaRolloutAssignment {
    pub schema_version: String,
    pub rollout_ref: String,
    pub campaign_id: String,
    pub task_id: String,
    pub dataset: String,
    pub split_ref: String,
    pub split: String,
    pub probe_commit: String,
    pub agent_slug: String,
    pub backend_model_ref: String,
    pub candidate_hash: String,
    pub candidate_id: String,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub evaluator_backend: ProbeGepaEvaluatorBackend,
    pub expected_artifact_refs: Vec<String>,
    pub timeout_policy_ref: String,
    pub budget_policy_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaRolloutResult {
    pub schema_version: String,
    pub rollout_ref: String,
    pub task_id: String,
    pub dataset: String,
    pub split_ref: String,
    pub split: String,
    pub probe_commit: String,
    pub agent_slug: String,
    pub backend_model_ref: String,
    pub candidate_hash: String,
    pub candidate_id: String,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub verifier_status: ProbeGepaVerifierStatus,
    pub rollout_status: ProbeGepaRolloutStatus,
    pub scalar_score_bps: u32,
    pub failure_family: ProbeGepaFailureClass,
    pub artifact_manifest_ref: Option<String>,
    pub proof_bundle_ref: Option<String>,
    pub resource_usage_ref: Option<String>,
    pub policy_findings: Vec<String>,
    pub duration_ms: u64,
    pub cost_micros: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaLiveCloseoutImport {
    pub schema_version: String,
    pub assignment_id: String,
    pub closeout_ref: String,
    pub campaign_id: String,
    pub task_id: String,
    pub dataset: String,
    pub split_ref: String,
    pub split: String,
    pub probe_commit: String,
    pub agent_slug: String,
    pub backend_model_ref: String,
    pub candidate_hash: String,
    pub candidate_id: String,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub verifier_refs: Vec<String>,
    pub closeout_state: ProbeGepaLiveCloseoutState,
    pub payment_mode: ProbeGepaLivePaymentMode,
    pub settlement_receipt_refs: Vec<String>,
    pub scalar_score_bps: u32,
    pub failure_family: ProbeGepaFailureClass,
    pub artifact_manifest_ref: String,
    pub proof_bundle_ref: String,
    pub resource_usage_ref: String,
    pub route_scorecard_ref: String,
    pub policy_findings: Vec<String>,
    pub duration_ms: u64,
    pub cost_micros: u64,
    pub public_claim: ProbeGepaLivePublicClaim,
    pub runtime_promotion_claim: ProbeGepaCandidatePromotionState,
    pub model_training_authority_claimed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaLiveCloseoutImportReceipt {
    pub schema_version: String,
    pub assignment_id: String,
    pub rollout_ref: String,
    pub closeout_ref: String,
    pub candidate_hash: String,
    pub closeout_state: ProbeGepaLiveCloseoutState,
    pub payment_mode: ProbeGepaLivePaymentMode,
    pub imported_result_status: ProbeGepaRolloutStatus,
    pub frontier_candidate_count: usize,
    pub completed_rollout_count: usize,
    pub accepted_for_reflection: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaLiveCloseoutImportRecord {
    pub assignment_id: String,
    pub closeout_ref: String,
    pub rollout_ref: String,
    pub closeout_state: ProbeGepaLiveCloseoutState,
    pub payment_mode: ProbeGepaLivePaymentMode,
    pub verifier_refs: Vec<String>,
    pub settlement_receipt_refs: Vec<String>,
    pub route_scorecard_ref: String,
    pub public_claim: ProbeGepaLivePublicClaim,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaEvaluationCache {
    pub results_by_cache_key: BTreeMap<String, ProbeGepaRolloutResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateFrontierEntry {
    pub candidate_id: String,
    pub candidate_hash: String,
    pub parent_candidate_id: Option<String>,
    pub mean_score_bps: u32,
    pub successful_rollouts: u32,
    pub agent_failures: u32,
    pub infrastructure_failures: u32,
    pub policy_blocks: u32,
    pub timeout_rollouts: u32,
    pub rejected_rollouts: u32,
    pub accepted_for_reflection: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaLineageEdge {
    pub parent_candidate_id: Option<String>,
    pub child_candidate_id: String,
    pub optimizer_run_id: String,
    pub reflection_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaReflectionProposal {
    pub proposal_ref: String,
    pub source_candidate_id: String,
    pub source_candidate_hash: String,
    pub next_candidate_parent_id: String,
    pub failure_families_to_mutate: Vec<ProbeGepaFailureClass>,
    pub accepted: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCoordinatorExports {
    pub probe_candidate_refs: Vec<String>,
    pub omega_candidate_refs: Vec<String>,
    pub artanis_candidate_refs: Vec<String>,
    pub benchmark_cloud_candidate_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaIterationMetrics {
    pub iteration_index: u32,
    pub metric_call_count: u32,
    pub cache_hit_count: u32,
    pub local_rollout_count: u32,
    pub pylon_assignment_count: u32,
    pub infrastructure_failure_count: u32,
    pub agent_failure_count: u32,
    pub policy_block_count: u32,
    pub timeout_count: u32,
    pub rejected_rollout_count: u32,
    pub total_duration_ms: u64,
    pub total_cost_micros: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCoordinatorState {
    pub schema_version: String,
    pub campaign_id: String,
    pub iteration_index: u32,
    pub candidate_frontier: Vec<ProbeGepaCandidateFrontierEntry>,
    pub lineage: Vec<ProbeGepaLineageEdge>,
    pub completed_rollouts: Vec<ProbeGepaRolloutResult>,
    pub live_closeout_imports: Vec<ProbeGepaLiveCloseoutImportRecord>,
    pub pending_pylon_assignments: Vec<ProbeGepaRolloutAssignment>,
    pub evaluation_cache: ProbeGepaEvaluationCache,
    pub reflection_proposals: Vec<ProbeGepaReflectionProposal>,
    pub exports: ProbeGepaCoordinatorExports,
    pub last_iteration_metrics: Option<ProbeGepaIterationMetrics>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCoordinatorConfig {
    pub campaign_id: String,
    pub probe_commit: String,
    pub agent_slug: String,
    pub backend_model_ref: String,
    pub local_first: bool,
    pub pylon_enabled: bool,
    pub target_metric_calls: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeGepaCoordinatorError {
    #[error("candidate manifest invalid: {0}")]
    CandidateManifestInvalid(#[from] ProbeGepaCandidateManifestError),
    #[error("candidate `{candidate_id}` cannot advance because policy gate state is `{policy_gate_state:?}`")]
    CandidatePolicyBlocked {
        candidate_id: String,
        policy_gate_state: ProbeGepaPolicyGateState,
    },
    #[error(
        "candidate `{candidate_id}` cannot advance from promotion state `{promotion_state:?}`"
    )]
    CandidatePromotionBlocked {
        candidate_id: String,
        promotion_state: ProbeGepaCandidatePromotionState,
    },
    #[error("stage 0 requires 20 to 40 metric calls, found {target_metric_calls}")]
    InvalidStage0MetricCallCount { target_metric_calls: u32 },
    #[error(
        "live closeout import `{assignment_id}` has invalid schema version `{schema_version}`"
    )]
    LiveImportInvalidSchema {
        assignment_id: String,
        schema_version: String,
    },
    #[error("live closeout import `{assignment_id}` is missing required field `{field}`")]
    LiveImportMissingRequiredRef {
        assignment_id: String,
        field: String,
    },
    #[error("live closeout import `{assignment_id}` campaign `{campaign_id}` does not match candidate campaign `{expected_campaign_id}`")]
    LiveImportCampaignMismatch {
        assignment_id: String,
        campaign_id: String,
        expected_campaign_id: String,
    },
    #[error("live closeout import `{assignment_id}` candidate `{candidate_id}` / `{candidate_hash}` does not match expected `{expected_candidate_id}` / `{expected_candidate_hash}`")]
    LiveImportCandidateMismatch {
        assignment_id: String,
        candidate_id: String,
        candidate_hash: String,
        expected_candidate_id: String,
        expected_candidate_hash: String,
    },
    #[error("live closeout import `{assignment_id}` split ref `{split_ref}` is not admitted by the candidate")]
    LiveImportUnknownSplitRef {
        assignment_id: String,
        split_ref: String,
    },
    #[error("live closeout import `{assignment_id}` payment mode `{payment_mode:?}` is invalid for closeout state `{closeout_state:?}`")]
    LiveImportPaymentStateMismatch {
        assignment_id: String,
        payment_mode: ProbeGepaLivePaymentMode,
        closeout_state: ProbeGepaLiveCloseoutState,
    },
    #[error("live closeout import `{assignment_id}` claims settled bitcoin without settlement receipt refs")]
    LiveImportMissingSettlementReceipt { assignment_id: String },
    #[error("live closeout import `{assignment_id}` overclaims authority in `{field}`")]
    LiveImportAuthorityOverclaim {
        assignment_id: String,
        field: String,
    },
}

impl Default for ProbeGepaCoordinatorState {
    fn default() -> Self {
        Self {
            schema_version: PROBE_GEPA_COORDINATOR_STATE_SCHEMA_VERSION.to_string(),
            campaign_id: PROBE_GEPA_STAGE_0_CAMPAIGN_ID.to_string(),
            iteration_index: 0,
            candidate_frontier: Vec::new(),
            lineage: Vec::new(),
            completed_rollouts: Vec::new(),
            live_closeout_imports: Vec::new(),
            pending_pylon_assignments: Vec::new(),
            evaluation_cache: ProbeGepaEvaluationCache::default(),
            reflection_proposals: Vec::new(),
            exports: ProbeGepaCoordinatorExports {
                probe_candidate_refs: Vec::new(),
                omega_candidate_refs: Vec::new(),
                artanis_candidate_refs: Vec::new(),
                benchmark_cloud_candidate_refs: Vec::new(),
            },
            last_iteration_metrics: None,
        }
    }
}

impl Default for ProbeGepaCoordinatorConfig {
    fn default() -> Self {
        Self {
            campaign_id: PROBE_GEPA_STAGE_0_CAMPAIGN_ID.to_string(),
            probe_commit: "probe.commit.local-stage0".to_string(),
            agent_slug: "probe".to_string(),
            backend_model_ref: "backend.local.probe.fake_metric_call.v1".to_string(),
            local_first: true,
            pylon_enabled: false,
            target_metric_calls: 20,
        }
    }
}

pub fn run_probe_gepa_stage0_iteration(
    state: &mut ProbeGepaCoordinatorState,
    candidate: &ProbeGepaCandidateManifest,
    config: &ProbeGepaCoordinatorConfig,
) -> Result<ProbeGepaIterationMetrics, ProbeGepaCoordinatorError> {
    validate_stage0_config(config)?;
    validate_candidate_can_advance(candidate)?;

    let tasks = canonical_probe_gepa_stage0_tasks(config.target_metric_calls);
    let mut metrics = ProbeGepaIterationMetrics {
        iteration_index: state.iteration_index,
        metric_call_count: 0,
        cache_hit_count: 0,
        local_rollout_count: 0,
        pylon_assignment_count: 0,
        infrastructure_failure_count: 0,
        agent_failure_count: 0,
        policy_block_count: 0,
        timeout_count: 0,
        rejected_rollout_count: 0,
        total_duration_ms: 0,
        total_cost_micros: 0,
    };

    for task in tasks {
        let assignment = build_probe_gepa_rollout_assignment(candidate, config, &task);
        let cache_key = rollout_cache_key(&assignment);
        let result =
            if let Some(cached) = state.evaluation_cache.results_by_cache_key.get(&cache_key) {
                metrics.cache_hit_count += 1;
                cached.clone()
            } else if config.local_first {
                metrics.local_rollout_count += 1;
                let result = run_local_probe_gepa_rollout(&assignment, &task);
                state
                    .evaluation_cache
                    .results_by_cache_key
                    .insert(cache_key, result.clone());
                result
            } else {
                metrics.pylon_assignment_count += 1;
                state.pending_pylon_assignments.push(assignment);
                continue;
            };

        metrics.metric_call_count += 1;
        metrics.total_duration_ms += result.duration_ms;
        metrics.total_cost_micros += result.cost_micros;
        match result.rollout_status {
            ProbeGepaRolloutStatus::InfrastructureFailed => {
                metrics.infrastructure_failure_count += 1
            }
            ProbeGepaRolloutStatus::AgentFailed => metrics.agent_failure_count += 1,
            ProbeGepaRolloutStatus::PolicyBlocked => metrics.policy_block_count += 1,
            ProbeGepaRolloutStatus::TimedOut => metrics.timeout_count += 1,
            ProbeGepaRolloutStatus::Rejected => metrics.rejected_rollout_count += 1,
            ProbeGepaRolloutStatus::Succeeded => {}
        }
        state.completed_rollouts.push(result);
    }

    let frontier_entry = summarize_candidate_frontier_entry(candidate, &state.completed_rollouts);
    upsert_frontier_entry(&mut state.candidate_frontier, frontier_entry.clone());
    let reflection = build_reflection_proposal(candidate, &state.completed_rollouts);
    state.lineage.push(ProbeGepaLineageEdge {
        parent_candidate_id: candidate.parent_candidate_id.clone(),
        child_candidate_id: candidate.candidate_id.clone(),
        optimizer_run_id: candidate.optimizer_run_id.clone(),
        reflection_ref: reflection.proposal_ref.clone(),
    });
    state.reflection_proposals.push(reflection);
    state.exports = build_candidate_exports(candidate);
    state.iteration_index += 1;
    state.last_iteration_metrics = Some(metrics.clone());
    Ok(metrics)
}

pub fn import_probe_gepa_live_closeout(
    state: &mut ProbeGepaCoordinatorState,
    candidate: &ProbeGepaCandidateManifest,
    import: ProbeGepaLiveCloseoutImport,
) -> Result<ProbeGepaLiveCloseoutImportReceipt, ProbeGepaCoordinatorError> {
    validate_candidate_can_advance(candidate)?;
    validate_probe_gepa_live_closeout_import(state, candidate, &import)?;

    let rollout_ref = stable_rollout_ref(import.candidate_hash.as_str(), import.task_id.as_str());
    let result = live_closeout_import_to_rollout_result(&import, rollout_ref.as_str());
    let cache_key = live_closeout_cache_key(&import);
    state
        .evaluation_cache
        .results_by_cache_key
        .insert(cache_key, result.clone());
    state.completed_rollouts.push(result.clone());
    state
        .pending_pylon_assignments
        .retain(|assignment| assignment.rollout_ref != rollout_ref);

    state
        .live_closeout_imports
        .push(ProbeGepaLiveCloseoutImportRecord {
            assignment_id: import.assignment_id.clone(),
            closeout_ref: import.closeout_ref.clone(),
            rollout_ref: rollout_ref.clone(),
            closeout_state: import.closeout_state,
            payment_mode: import.payment_mode,
            verifier_refs: import.verifier_refs.clone(),
            settlement_receipt_refs: import.settlement_receipt_refs.clone(),
            route_scorecard_ref: import.route_scorecard_ref.clone(),
            public_claim: import.public_claim,
        });

    let frontier_entry = summarize_candidate_frontier_entry(candidate, &state.completed_rollouts);
    let accepted_for_reflection = frontier_entry.accepted_for_reflection;
    upsert_frontier_entry(&mut state.candidate_frontier, frontier_entry);
    state.exports = build_candidate_exports(candidate);
    state.last_iteration_metrics = Some(summarize_live_import_iteration_metrics(state));

    Ok(ProbeGepaLiveCloseoutImportReceipt {
        schema_version: PROBE_GEPA_LIVE_CLOSEOUT_IMPORT_RECEIPT_SCHEMA_VERSION.to_string(),
        assignment_id: import.assignment_id,
        rollout_ref,
        closeout_ref: import.closeout_ref,
        candidate_hash: import.candidate_hash,
        closeout_state: import.closeout_state,
        payment_mode: import.payment_mode,
        imported_result_status: result.rollout_status,
        frontier_candidate_count: state.candidate_frontier.len(),
        completed_rollout_count: state.completed_rollouts.len(),
        accepted_for_reflection,
    })
}

pub fn canonical_probe_gepa_terminal_bench_pylon_canary_import(
    candidate: &ProbeGepaCandidateManifest,
) -> ProbeGepaLiveCloseoutImport {
    let run_ref = "probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057";
    ProbeGepaLiveCloseoutImport {
        schema_version: PROBE_GEPA_LIVE_CLOSEOUT_IMPORT_SCHEMA_VERSION.to_string(),
        assignment_id: "assignment.public.probe_gepa.terminal_bench_2.canary.20260608151057"
            .to_string(),
        closeout_ref: format!("probe_closeout.{run_ref}"),
        campaign_id: candidate.campaign_id.clone(),
        task_id: "configure-git-webserver".to_string(),
        dataset: "terminal_bench_2".to_string(),
        split_ref: candidate.split_refs[0].clone(),
        split: "retained".to_string(),
        probe_commit: "d58cc03".to_string(),
        agent_slug: "probe".to_string(),
        backend_model_ref: "pylon.artanis.gepa_stats_canary.20260608150415".to_string(),
        candidate_hash: candidate.candidate_hash.clone(),
        candidate_id: candidate.candidate_id.clone(),
        selected_signature_refs: vec![
            "program_signature.probe.benchmark.service_readiness.v1".to_string(),
        ],
        tool_menu_ref: "tool_menu.probe.terminal_bench_2.canary.unpaid_smoke.v1".to_string(),
        verifier_refs: vec![
            format!("verifier_result.public.{run_ref}.initial_retained.service_readiness"),
            "accepted_work.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057.probe_gepa_canary".to_string(),
            "pylon_event.heartbeat.cbfba02f-8ce8-4161-954a-109f403a56c1".to_string(),
            "pylon_event.wallet_readiness.ae6b9e8c-58d8-4dde-a1a0-d666d162087a"
                .to_string(),
            "pylon_event.assignment_acceptance.ab63a3aa-771e-47cd-8cf6-1d29f945021e".to_string(),
            "pylon_event.assignment_progress.27bf8d2d-6979-4597-8e85-2da994990710".to_string(),
            "pylon_event.artifact_proof_metadata.82e04c95-8084-4b95-bbd2-1183b760c7ae".to_string(),
        ],
        closeout_state: ProbeGepaLiveCloseoutState::AgentFailure,
        payment_mode: ProbeGepaLivePaymentMode::UnpaidSmoke,
        settlement_receipt_refs: Vec::new(),
        scalar_score_bps: 0,
        failure_family: ProbeGepaFailureClass::ServiceReadiness,
        artifact_manifest_ref: format!("artifact.public.{run_ref}.probe_closeout_bundle"),
        proof_bundle_ref: format!("proof.public.{run_ref}.no_spend_assignment_lifecycle"),
        resource_usage_ref: format!("resource_usage.public.{run_ref}.unpaid_smoke.no_spend"),
        route_scorecard_ref: format!("route_scorecard.public.{run_ref}"),
        policy_findings: vec![
            "no_public_benchmark_score_claimed".to_string(),
            "worker_closeout_accepted_as_evidence_not_benchmark_pass".to_string(),
            "no_paid_work_or_settlement_claimed".to_string(),
        ],
        duration_ms: 90_000,
        cost_micros: 0,
        public_claim: ProbeGepaLivePublicClaim::None,
        runtime_promotion_claim: ProbeGepaCandidatePromotionState::Draft,
        model_training_authority_claimed: false,
    }
}

pub fn build_probe_gepa_rollout_assignment(
    candidate: &ProbeGepaCandidateManifest,
    config: &ProbeGepaCoordinatorConfig,
    task: &ProbeGepaRolloutTask,
) -> ProbeGepaRolloutAssignment {
    let rollout_ref = stable_rollout_ref(candidate.candidate_hash.as_str(), task.task_id.as_str());
    ProbeGepaRolloutAssignment {
        schema_version: PROBE_GEPA_ROLLOUT_ASSIGNMENT_SCHEMA_VERSION.to_string(),
        rollout_ref,
        campaign_id: config.campaign_id.clone(),
        task_id: task.task_id.clone(),
        dataset: task.dataset.clone(),
        split_ref: task.split_ref.clone(),
        split: task.split.clone(),
        probe_commit: config.probe_commit.clone(),
        agent_slug: config.agent_slug.clone(),
        backend_model_ref: config.backend_model_ref.clone(),
        candidate_hash: candidate.candidate_hash.clone(),
        candidate_id: candidate.candidate_id.clone(),
        selected_signature_refs: task.selected_signature_refs.clone(),
        tool_menu_ref: task.tool_menu_ref.clone(),
        evaluator_backend: if config.pylon_enabled && !config.local_first {
            ProbeGepaEvaluatorBackend::PylonPending
        } else {
            ProbeGepaEvaluatorBackend::Local
        },
        expected_artifact_refs: vec![
            "artifact_manifest.probe_gepa_rollout.required.v1".to_string(),
            "proof_bundle.probe_gepa_rollout.required.v1".to_string(),
            "resource_usage.probe_gepa_rollout.required.v1".to_string(),
        ],
        timeout_policy_ref: "timeout_policy.probe_gepa.stage0.v1".to_string(),
        budget_policy_ref: "budget_policy.probe_gepa.local_zero_cost.v1".to_string(),
    }
}

fn validate_probe_gepa_live_closeout_import(
    state: &ProbeGepaCoordinatorState,
    candidate: &ProbeGepaCandidateManifest,
    import: &ProbeGepaLiveCloseoutImport,
) -> Result<(), ProbeGepaCoordinatorError> {
    if import.schema_version != PROBE_GEPA_LIVE_CLOSEOUT_IMPORT_SCHEMA_VERSION {
        return Err(ProbeGepaCoordinatorError::LiveImportInvalidSchema {
            assignment_id: import.assignment_id.clone(),
            schema_version: import.schema_version.clone(),
        });
    }
    for (field, value) in [
        ("assignment_id", import.assignment_id.as_str()),
        ("closeout_ref", import.closeout_ref.as_str()),
        ("task_id", import.task_id.as_str()),
        ("dataset", import.dataset.as_str()),
        ("split_ref", import.split_ref.as_str()),
        ("split", import.split.as_str()),
        ("probe_commit", import.probe_commit.as_str()),
        ("agent_slug", import.agent_slug.as_str()),
        ("backend_model_ref", import.backend_model_ref.as_str()),
        ("candidate_hash", import.candidate_hash.as_str()),
        ("candidate_id", import.candidate_id.as_str()),
        ("tool_menu_ref", import.tool_menu_ref.as_str()),
        (
            "artifact_manifest_ref",
            import.artifact_manifest_ref.as_str(),
        ),
        ("proof_bundle_ref", import.proof_bundle_ref.as_str()),
        ("resource_usage_ref", import.resource_usage_ref.as_str()),
        ("route_scorecard_ref", import.route_scorecard_ref.as_str()),
    ] {
        require_live_import_ref(import.assignment_id.as_str(), field, value)?;
    }
    require_live_import_vec(
        import.assignment_id.as_str(),
        "selected_signature_refs",
        &import.selected_signature_refs,
    )?;
    require_live_import_vec(
        import.assignment_id.as_str(),
        "verifier_refs",
        &import.verifier_refs,
    )?;

    if import.campaign_id != candidate.campaign_id || import.campaign_id != state.campaign_id {
        return Err(ProbeGepaCoordinatorError::LiveImportCampaignMismatch {
            assignment_id: import.assignment_id.clone(),
            campaign_id: import.campaign_id.clone(),
            expected_campaign_id: candidate.campaign_id.clone(),
        });
    }
    if import.candidate_id != candidate.candidate_id
        || import.candidate_hash != candidate.candidate_hash
    {
        return Err(ProbeGepaCoordinatorError::LiveImportCandidateMismatch {
            assignment_id: import.assignment_id.clone(),
            candidate_id: import.candidate_id.clone(),
            candidate_hash: import.candidate_hash.clone(),
            expected_candidate_id: candidate.candidate_id.clone(),
            expected_candidate_hash: candidate.candidate_hash.clone(),
        });
    }
    if !candidate.split_refs.contains(&import.split_ref) {
        return Err(ProbeGepaCoordinatorError::LiveImportUnknownSplitRef {
            assignment_id: import.assignment_id.clone(),
            split_ref: import.split_ref.clone(),
        });
    }
    if import.payment_mode == ProbeGepaLivePaymentMode::SettledBitcoin
        && import.settlement_receipt_refs.is_empty()
    {
        return Err(
            ProbeGepaCoordinatorError::LiveImportMissingSettlementReceipt {
                assignment_id: import.assignment_id.clone(),
            },
        );
    }
    if import.closeout_state == ProbeGepaLiveCloseoutState::Rejected
        && import.payment_mode != ProbeGepaLivePaymentMode::RejectedNoPay
    {
        return Err(ProbeGepaCoordinatorError::LiveImportPaymentStateMismatch {
            assignment_id: import.assignment_id.clone(),
            payment_mode: import.payment_mode,
            closeout_state: import.closeout_state,
        });
    }
    if import.payment_mode == ProbeGepaLivePaymentMode::RejectedNoPay
        && import.closeout_state != ProbeGepaLiveCloseoutState::Rejected
    {
        return Err(ProbeGepaCoordinatorError::LiveImportPaymentStateMismatch {
            assignment_id: import.assignment_id.clone(),
            payment_mode: import.payment_mode,
            closeout_state: import.closeout_state,
        });
    }
    if import.public_claim == ProbeGepaLivePublicClaim::PublicBenchmarkScore {
        return Err(ProbeGepaCoordinatorError::LiveImportAuthorityOverclaim {
            assignment_id: import.assignment_id.clone(),
            field: "public_claim".to_string(),
        });
    }
    if matches!(
        import.runtime_promotion_claim,
        ProbeGepaCandidatePromotionState::Active
            | ProbeGepaCandidatePromotionState::ReleaseCandidate
    ) {
        return Err(ProbeGepaCoordinatorError::LiveImportAuthorityOverclaim {
            assignment_id: import.assignment_id.clone(),
            field: "runtime_promotion_claim".to_string(),
        });
    }
    if import.model_training_authority_claimed {
        return Err(ProbeGepaCoordinatorError::LiveImportAuthorityOverclaim {
            assignment_id: import.assignment_id.clone(),
            field: "model_training_authority_claimed".to_string(),
        });
    }
    Ok(())
}

fn live_closeout_import_to_rollout_result(
    import: &ProbeGepaLiveCloseoutImport,
    rollout_ref: &str,
) -> ProbeGepaRolloutResult {
    let rollout_status = match import.closeout_state {
        ProbeGepaLiveCloseoutState::Accepted => ProbeGepaRolloutStatus::Succeeded,
        ProbeGepaLiveCloseoutState::Rejected => ProbeGepaRolloutStatus::Rejected,
        ProbeGepaLiveCloseoutState::InfrastructureFailure => {
            ProbeGepaRolloutStatus::InfrastructureFailed
        }
        ProbeGepaLiveCloseoutState::AgentFailure => ProbeGepaRolloutStatus::AgentFailed,
        ProbeGepaLiveCloseoutState::TimedOut => ProbeGepaRolloutStatus::TimedOut,
        ProbeGepaLiveCloseoutState::PolicyBlocked => ProbeGepaRolloutStatus::PolicyBlocked,
    };
    let verifier_status = match import.closeout_state {
        ProbeGepaLiveCloseoutState::Accepted => ProbeGepaVerifierStatus::Passed,
        ProbeGepaLiveCloseoutState::Rejected => ProbeGepaVerifierStatus::Error,
        ProbeGepaLiveCloseoutState::InfrastructureFailure => ProbeGepaVerifierStatus::Error,
        ProbeGepaLiveCloseoutState::AgentFailure => ProbeGepaVerifierStatus::Failed,
        ProbeGepaLiveCloseoutState::TimedOut => ProbeGepaVerifierStatus::TimedOut,
        ProbeGepaLiveCloseoutState::PolicyBlocked => ProbeGepaVerifierStatus::Error,
    };

    ProbeGepaRolloutResult {
        schema_version: PROBE_GEPA_ROLLOUT_RESULT_SCHEMA_VERSION.to_string(),
        rollout_ref: rollout_ref.to_string(),
        task_id: import.task_id.clone(),
        dataset: import.dataset.clone(),
        split_ref: import.split_ref.clone(),
        split: import.split.clone(),
        probe_commit: import.probe_commit.clone(),
        agent_slug: import.agent_slug.clone(),
        backend_model_ref: import.backend_model_ref.clone(),
        candidate_hash: import.candidate_hash.clone(),
        candidate_id: import.candidate_id.clone(),
        selected_signature_refs: import.selected_signature_refs.clone(),
        tool_menu_ref: import.tool_menu_ref.clone(),
        verifier_status,
        rollout_status,
        scalar_score_bps: import.scalar_score_bps,
        failure_family: import.failure_family,
        artifact_manifest_ref: Some(import.artifact_manifest_ref.clone()),
        proof_bundle_ref: Some(import.proof_bundle_ref.clone()),
        resource_usage_ref: Some(import.resource_usage_ref.clone()),
        policy_findings: import.policy_findings.clone(),
        duration_ms: import.duration_ms,
        cost_micros: import.cost_micros,
    }
}

fn summarize_live_import_iteration_metrics(
    state: &ProbeGepaCoordinatorState,
) -> ProbeGepaIterationMetrics {
    let mut metrics = ProbeGepaIterationMetrics {
        iteration_index: state.iteration_index,
        metric_call_count: state.completed_rollouts.len() as u32,
        cache_hit_count: 0,
        local_rollout_count: 0,
        pylon_assignment_count: state.live_closeout_imports.len() as u32,
        infrastructure_failure_count: 0,
        agent_failure_count: 0,
        policy_block_count: 0,
        timeout_count: 0,
        rejected_rollout_count: 0,
        total_duration_ms: 0,
        total_cost_micros: 0,
    };
    for result in &state.completed_rollouts {
        metrics.total_duration_ms += result.duration_ms;
        metrics.total_cost_micros += result.cost_micros;
        match result.rollout_status {
            ProbeGepaRolloutStatus::Succeeded => {}
            ProbeGepaRolloutStatus::AgentFailed => metrics.agent_failure_count += 1,
            ProbeGepaRolloutStatus::InfrastructureFailed => {
                metrics.infrastructure_failure_count += 1
            }
            ProbeGepaRolloutStatus::PolicyBlocked => metrics.policy_block_count += 1,
            ProbeGepaRolloutStatus::TimedOut => metrics.timeout_count += 1,
            ProbeGepaRolloutStatus::Rejected => metrics.rejected_rollout_count += 1,
        }
    }
    metrics
}

fn require_live_import_ref(
    assignment_id: &str,
    field: &str,
    value: &str,
) -> Result<(), ProbeGepaCoordinatorError> {
    if value.trim().is_empty() {
        return Err(ProbeGepaCoordinatorError::LiveImportMissingRequiredRef {
            assignment_id: assignment_id.to_string(),
            field: field.to_string(),
        });
    }
    Ok(())
}

fn require_live_import_vec(
    assignment_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), ProbeGepaCoordinatorError> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(ProbeGepaCoordinatorError::LiveImportMissingRequiredRef {
            assignment_id: assignment_id.to_string(),
            field: field.to_string(),
        });
    }
    Ok(())
}

pub fn run_local_probe_gepa_rollout(
    assignment: &ProbeGepaRolloutAssignment,
    task: &ProbeGepaRolloutTask,
) -> ProbeGepaRolloutResult {
    let infrastructure_failure = task.task_id.contains("infra-unavailable");
    let agent_failure = task.task_id.contains("agent-regression");
    let policy_blocked = task.task_id.contains("policy-blocked");
    let rollout_status = if infrastructure_failure {
        ProbeGepaRolloutStatus::InfrastructureFailed
    } else if policy_blocked {
        ProbeGepaRolloutStatus::PolicyBlocked
    } else if agent_failure {
        ProbeGepaRolloutStatus::AgentFailed
    } else {
        ProbeGepaRolloutStatus::Succeeded
    };
    let verifier_status = match rollout_status {
        ProbeGepaRolloutStatus::Succeeded => ProbeGepaVerifierStatus::Passed,
        ProbeGepaRolloutStatus::AgentFailed => ProbeGepaVerifierStatus::Failed,
        ProbeGepaRolloutStatus::InfrastructureFailed => ProbeGepaVerifierStatus::Error,
        ProbeGepaRolloutStatus::PolicyBlocked => ProbeGepaVerifierStatus::Error,
        ProbeGepaRolloutStatus::TimedOut => ProbeGepaVerifierStatus::TimedOut,
        ProbeGepaRolloutStatus::Rejected => ProbeGepaVerifierStatus::Error,
    };
    let failure_family = match rollout_status {
        ProbeGepaRolloutStatus::Succeeded => ProbeGepaFailureClass::None,
        ProbeGepaRolloutStatus::AgentFailed => task.expected_failure_family,
        ProbeGepaRolloutStatus::InfrastructureFailed => {
            ProbeGepaFailureClass::InfrastructureUnavailable
        }
        ProbeGepaRolloutStatus::PolicyBlocked => ProbeGepaFailureClass::PolicyViolation,
        ProbeGepaRolloutStatus::TimedOut => ProbeGepaFailureClass::RunnerSupervision,
        ProbeGepaRolloutStatus::Rejected => ProbeGepaFailureClass::InfrastructureUnavailable,
    };

    ProbeGepaRolloutResult {
        schema_version: PROBE_GEPA_ROLLOUT_RESULT_SCHEMA_VERSION.to_string(),
        rollout_ref: assignment.rollout_ref.clone(),
        task_id: assignment.task_id.clone(),
        dataset: assignment.dataset.clone(),
        split_ref: assignment.split_ref.clone(),
        split: assignment.split.clone(),
        probe_commit: assignment.probe_commit.clone(),
        agent_slug: assignment.agent_slug.clone(),
        backend_model_ref: assignment.backend_model_ref.clone(),
        candidate_hash: assignment.candidate_hash.clone(),
        candidate_id: assignment.candidate_id.clone(),
        selected_signature_refs: assignment.selected_signature_refs.clone(),
        tool_menu_ref: assignment.tool_menu_ref.clone(),
        verifier_status,
        rollout_status,
        scalar_score_bps: if rollout_status == ProbeGepaRolloutStatus::Succeeded {
            10_000
        } else {
            0
        },
        failure_family,
        artifact_manifest_ref: Some(format!("artifact_manifest.{}", assignment.rollout_ref)),
        proof_bundle_ref: Some(format!("proof_bundle.{}", assignment.rollout_ref)),
        resource_usage_ref: Some(format!("resource_usage.{}", assignment.rollout_ref)),
        policy_findings: if policy_blocked {
            vec!["candidate_or_assignment_policy_blocked".to_string()]
        } else {
            Vec::new()
        },
        duration_ms: if infrastructure_failure { 250 } else { 1_000 },
        cost_micros: 0,
    }
}

pub fn canonical_probe_gepa_stage0_tasks(target_metric_calls: u32) -> Vec<ProbeGepaRolloutTask> {
    let families = [
        (
            "configure-git-webserver",
            ProbeGepaFailureClass::ServiceReadiness,
        ),
        (
            "filter-js-from-html",
            ProbeGepaFailureClass::ParserCorrectness,
        ),
        ("gcode-to-text", ProbeGepaFailureClass::ParserCorrectness),
        (
            "runner-stall-supervision",
            ProbeGepaFailureClass::RunnerSupervision,
        ),
    ];
    (0..target_metric_calls)
        .map(|index| {
            let (base_task, family) = families[index as usize % families.len()];
            let split = if index < target_metric_calls / 2 {
                "retained"
            } else {
                "validation"
            };
            ProbeGepaRolloutTask {
                task_id: format!("{base_task}.metric_call_{index:02}"),
                dataset: "terminal_bench_2".to_string(),
                split_ref: "benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1"
                    .to_string(),
                split: split.to_string(),
                expected_failure_family: family,
                selected_signature_refs: vec![format!(
                    "program_signature.probe.benchmark.{family:?}.v1"
                )],
                tool_menu_ref: format!("tool_menu.probe.terminal_bench.{base_task}.v1"),
                scorer_verifier_ref: format!("verifier.terminal_bench.{base_task}.v1"),
            }
        })
        .collect()
}

fn validate_stage0_config(
    config: &ProbeGepaCoordinatorConfig,
) -> Result<(), ProbeGepaCoordinatorError> {
    if !(20..=40).contains(&config.target_metric_calls) {
        return Err(ProbeGepaCoordinatorError::InvalidStage0MetricCallCount {
            target_metric_calls: config.target_metric_calls,
        });
    }
    Ok(())
}

fn validate_candidate_can_advance(
    candidate: &ProbeGepaCandidateManifest,
) -> Result<(), ProbeGepaCoordinatorError> {
    validate_probe_gepa_candidate_manifest(candidate)?;
    if candidate.safety_boundary.public_claim_upgrade_authority
        || !candidate.safety_boundary.no_new_runtime_authority
    {
        return Err(ProbeGepaCoordinatorError::CandidatePolicyBlocked {
            candidate_id: candidate.candidate_id.clone(),
            policy_gate_state: candidate.policy_gate_state,
        });
    }
    if matches!(
        candidate.policy_gate_state,
        ProbeGepaPolicyGateState::Failed | ProbeGepaPolicyGateState::Blocked
    ) {
        return Err(ProbeGepaCoordinatorError::CandidatePolicyBlocked {
            candidate_id: candidate.candidate_id.clone(),
            policy_gate_state: candidate.policy_gate_state,
        });
    }
    if matches!(
        candidate.promotion_state,
        ProbeGepaCandidatePromotionState::Active | ProbeGepaCandidatePromotionState::Reverted
    ) {
        return Err(ProbeGepaCoordinatorError::CandidatePromotionBlocked {
            candidate_id: candidate.candidate_id.clone(),
            promotion_state: candidate.promotion_state,
        });
    }
    Ok(())
}

fn summarize_candidate_frontier_entry(
    candidate: &ProbeGepaCandidateManifest,
    completed_rollouts: &[ProbeGepaRolloutResult],
) -> ProbeGepaCandidateFrontierEntry {
    let candidate_results = completed_rollouts
        .iter()
        .filter(|result| result.candidate_hash == candidate.candidate_hash)
        .collect::<Vec<_>>();
    let count = candidate_results.len().max(1) as u32;
    let total_score = candidate_results
        .iter()
        .map(|result| result.scalar_score_bps)
        .sum::<u32>();
    let infrastructure_failures = candidate_results
        .iter()
        .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::InfrastructureFailed)
        .count() as u32;
    let agent_failures = candidate_results
        .iter()
        .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::AgentFailed)
        .count() as u32;
    let policy_blocks = candidate_results
        .iter()
        .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::PolicyBlocked)
        .count() as u32;
    let timeout_rollouts = candidate_results
        .iter()
        .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::TimedOut)
        .count() as u32;
    let rejected_rollouts = candidate_results
        .iter()
        .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::Rejected)
        .count() as u32;

    ProbeGepaCandidateFrontierEntry {
        candidate_id: candidate.candidate_id.clone(),
        candidate_hash: candidate.candidate_hash.clone(),
        parent_candidate_id: candidate.parent_candidate_id.clone(),
        mean_score_bps: total_score / count,
        successful_rollouts: candidate_results
            .iter()
            .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::Succeeded)
            .count() as u32,
        agent_failures,
        infrastructure_failures,
        policy_blocks,
        timeout_rollouts,
        rejected_rollouts,
        accepted_for_reflection: policy_blocks == 0,
    }
}

fn build_reflection_proposal(
    candidate: &ProbeGepaCandidateManifest,
    completed_rollouts: &[ProbeGepaRolloutResult],
) -> ProbeGepaReflectionProposal {
    let mut families = BTreeSet::new();
    for result in completed_rollouts
        .iter()
        .filter(|result| result.candidate_hash == candidate.candidate_hash)
        .filter(|result| result.rollout_status == ProbeGepaRolloutStatus::AgentFailed)
    {
        families.insert(result.failure_family);
    }
    ProbeGepaReflectionProposal {
        proposal_ref: format!(
            "gepa_reflection.{}",
            short_hash(candidate.candidate_hash.as_str())
        ),
        source_candidate_id: candidate.candidate_id.clone(),
        source_candidate_hash: candidate.candidate_hash.clone(),
        next_candidate_parent_id: candidate.candidate_id.clone(),
        failure_families_to_mutate: families.into_iter().collect(),
        accepted: true,
        detail: "Central GEPA reflection/proposal step retained for the next candidate mutation."
            .to_string(),
    }
}

fn build_candidate_exports(candidate: &ProbeGepaCandidateManifest) -> ProbeGepaCoordinatorExports {
    ProbeGepaCoordinatorExports {
        probe_candidate_refs: vec![
            candidate.probe_import.prompt_candidate_ref.clone(),
            candidate.probe_import.blueprint_candidate_ref.clone(),
            candidate.probe_import.tool_menu_candidate_ref.clone(),
            candidate.probe_import.loop_policy_candidate_ref.clone(),
        ],
        omega_candidate_refs: vec![format!(
            "omega.probe_candidate_projection.{}",
            candidate.candidate_id
        )],
        artanis_candidate_refs: vec![format!(
            "artanis.probe_candidate_projection.{}",
            candidate.candidate_id
        )],
        benchmark_cloud_candidate_refs: candidate
            .benchmark_cloud_import
            .benchmark_run_manifest_refs
            .clone(),
    }
}

fn upsert_frontier_entry(
    frontier: &mut Vec<ProbeGepaCandidateFrontierEntry>,
    entry: ProbeGepaCandidateFrontierEntry,
) {
    if let Some(existing) = frontier
        .iter_mut()
        .find(|existing| existing.candidate_hash == entry.candidate_hash)
    {
        *existing = entry;
    } else {
        frontier.push(entry);
    }
}

fn rollout_cache_key(assignment: &ProbeGepaRolloutAssignment) -> String {
    stable_sha256(
        format!(
            "{}|{}|{}|{}|{}",
            assignment.candidate_hash,
            assignment.task_id,
            assignment.probe_commit,
            assignment.backend_model_ref,
            assignment.evaluator_backend as u8
        )
        .as_bytes(),
    )
}

fn live_closeout_cache_key(import: &ProbeGepaLiveCloseoutImport) -> String {
    stable_sha256(
        format!(
            "{}|{}|{}|{}|live_closeout|{:?}|{:?}",
            import.candidate_hash,
            import.task_id,
            import.probe_commit,
            import.backend_model_ref,
            import.closeout_state,
            import.payment_mode
        )
        .as_bytes(),
    )
}

fn stable_rollout_ref(candidate_hash: &str, task_id: &str) -> String {
    format!(
        "probe_gepa_rollout.{}",
        short_hash(stable_sha256(format!("{candidate_hash}|{task_id}").as_bytes()).as_str())
    )
}

fn stable_sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn short_hash(hash: &str) -> &str {
    hash.strip_prefix("sha256:")
        .and_then(|suffix| suffix.get(..16))
        .unwrap_or(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_probe_gepa_stage_0_1_candidate_manifest;

    #[test]
    fn local_evaluator_runs_stage0_metric_calls_before_pylon() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let config = ProbeGepaCoordinatorConfig::default();

        let metrics = run_probe_gepa_stage0_iteration(&mut state, &candidate, &config).unwrap();

        assert_eq!(metrics.metric_call_count, 20);
        assert_eq!(metrics.local_rollout_count, 20);
        assert_eq!(metrics.pylon_assignment_count, 0);
        assert_eq!(state.completed_rollouts.len(), 20);
        assert_eq!(state.evaluation_cache.results_by_cache_key.len(), 20);
        assert_eq!(state.candidate_frontier.len(), 1);
        assert_eq!(
            state.candidate_frontier[0].candidate_hash,
            candidate.candidate_hash
        );
        assert!(state.pending_pylon_assignments.is_empty());
        assert!(state
            .exports
            .probe_candidate_refs
            .contains(&candidate.probe_import.prompt_candidate_ref));
    }

    #[test]
    fn coordinator_resumes_from_evaluation_cache() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let config = ProbeGepaCoordinatorConfig::default();

        run_probe_gepa_stage0_iteration(&mut state, &candidate, &config).unwrap();
        state.completed_rollouts.clear();
        state.iteration_index = 1;
        let metrics = run_probe_gepa_stage0_iteration(&mut state, &candidate, &config).unwrap();

        assert_eq!(metrics.metric_call_count, 20);
        assert_eq!(metrics.cache_hit_count, 20);
        assert_eq!(metrics.local_rollout_count, 0);
        assert_eq!(state.completed_rollouts.len(), 20);
        assert_eq!(state.iteration_index, 2);
    }

    #[test]
    fn rollout_status_distinguishes_infrastructure_from_agent_failure() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let config = ProbeGepaCoordinatorConfig::default();
        let infra_task = ProbeGepaRolloutTask {
            task_id: "infra-unavailable.metric_call_00".to_string(),
            dataset: "terminal_bench_2".to_string(),
            split_ref: "split.ref".to_string(),
            split: "retained".to_string(),
            expected_failure_family: ProbeGepaFailureClass::ServiceReadiness,
            selected_signature_refs: vec!["signature.ref".to_string()],
            tool_menu_ref: "tool.ref".to_string(),
            scorer_verifier_ref: "verifier.ref".to_string(),
        };
        let agent_task = ProbeGepaRolloutTask {
            task_id: "agent-regression.metric_call_01".to_string(),
            ..infra_task.clone()
        };

        let infra = run_local_probe_gepa_rollout(
            &build_probe_gepa_rollout_assignment(&candidate, &config, &infra_task),
            &infra_task,
        );
        let agent = run_local_probe_gepa_rollout(
            &build_probe_gepa_rollout_assignment(&candidate, &config, &agent_task),
            &agent_task,
        );

        assert_eq!(
            infra.rollout_status,
            ProbeGepaRolloutStatus::InfrastructureFailed
        );
        assert_eq!(
            infra.failure_family,
            ProbeGepaFailureClass::InfrastructureUnavailable
        );
        assert_eq!(agent.rollout_status, ProbeGepaRolloutStatus::AgentFailed);
        assert_eq!(
            agent.failure_family,
            ProbeGepaFailureClass::ServiceReadiness
        );
    }

    #[test]
    fn policy_violating_candidates_cannot_advance() {
        let mut candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        candidate.safety_boundary.public_claim_upgrade_authority = true;
        let mut state = ProbeGepaCoordinatorState::default();
        let config = ProbeGepaCoordinatorConfig::default();

        assert!(matches!(
            run_probe_gepa_stage0_iteration(&mut state, &candidate, &config),
            Err(ProbeGepaCoordinatorError::CandidateManifestInvalid(_))
        ));
        assert!(state.completed_rollouts.is_empty());
        assert!(state.candidate_frontier.is_empty());
    }

    #[test]
    fn live_imports_update_frontier_for_accepted_and_rejected_closeouts() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();

        let accepted = live_import_for_candidate(
            &candidate,
            "task.accepted",
            ProbeGepaLiveCloseoutState::Accepted,
        );
        let mut rejected = live_import_for_candidate(
            &candidate,
            "task.rejected",
            ProbeGepaLiveCloseoutState::Rejected,
        );
        rejected.payment_mode = ProbeGepaLivePaymentMode::RejectedNoPay;

        let accepted_receipt =
            import_probe_gepa_live_closeout(&mut state, &candidate, accepted).unwrap();
        let rejected_receipt =
            import_probe_gepa_live_closeout(&mut state, &candidate, rejected).unwrap();

        assert_eq!(
            accepted_receipt.imported_result_status,
            ProbeGepaRolloutStatus::Succeeded
        );
        assert_eq!(
            rejected_receipt.imported_result_status,
            ProbeGepaRolloutStatus::Rejected
        );
        assert_eq!(state.live_closeout_imports.len(), 2);
        assert_eq!(state.candidate_frontier.len(), 1);
        assert_eq!(state.candidate_frontier[0].successful_rollouts, 1);
        assert_eq!(state.candidate_frontier[0].rejected_rollouts, 1);
        assert_eq!(
            candidate.promotion_state,
            ProbeGepaCandidatePromotionState::Draft
        );
        assert_eq!(
            state
                .last_iteration_metrics
                .as_ref()
                .unwrap()
                .pylon_assignment_count,
            2
        );
    }

    #[test]
    fn live_import_rejects_missing_artifact_proof_resource_and_verifier_refs() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let state = ProbeGepaCoordinatorState::default();

        let mut missing_artifact = live_import_for_candidate(
            &candidate,
            "task.missing_artifact",
            ProbeGepaLiveCloseoutState::Accepted,
        );
        missing_artifact.artifact_manifest_ref.clear();
        assert!(matches!(
            validate_probe_gepa_live_closeout_import(&state, &candidate, &missing_artifact),
            Err(ProbeGepaCoordinatorError::LiveImportMissingRequiredRef { field, .. })
                if field == "artifact_manifest_ref"
        ));

        let mut missing_proof = live_import_for_candidate(
            &candidate,
            "task.missing_proof",
            ProbeGepaLiveCloseoutState::Accepted,
        );
        missing_proof.proof_bundle_ref.clear();
        assert!(matches!(
            validate_probe_gepa_live_closeout_import(&state, &candidate, &missing_proof),
            Err(ProbeGepaCoordinatorError::LiveImportMissingRequiredRef { field, .. })
                if field == "proof_bundle_ref"
        ));

        let mut missing_resource = live_import_for_candidate(
            &candidate,
            "task.missing_resource",
            ProbeGepaLiveCloseoutState::Accepted,
        );
        missing_resource.resource_usage_ref.clear();
        assert!(matches!(
            validate_probe_gepa_live_closeout_import(&state, &candidate, &missing_resource),
            Err(ProbeGepaCoordinatorError::LiveImportMissingRequiredRef { field, .. })
                if field == "resource_usage_ref"
        ));

        let mut missing_verifier = live_import_for_candidate(
            &candidate,
            "task.missing_verifier",
            ProbeGepaLiveCloseoutState::Accepted,
        );
        missing_verifier.verifier_refs.clear();
        assert!(matches!(
            validate_probe_gepa_live_closeout_import(&state, &candidate, &missing_verifier),
            Err(ProbeGepaCoordinatorError::LiveImportMissingRequiredRef { field, .. })
                if field == "verifier_refs"
        ));
    }

    #[test]
    fn settled_bitcoin_requires_settlement_receipts() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let mut import = live_import_for_candidate(
            &candidate,
            "task.settled",
            ProbeGepaLiveCloseoutState::Accepted,
        );
        import.payment_mode = ProbeGepaLivePaymentMode::SettledBitcoin;

        assert!(matches!(
            import_probe_gepa_live_closeout(&mut state, &candidate, import),
            Err(ProbeGepaCoordinatorError::LiveImportMissingSettlementReceipt { .. })
        ));
        assert!(state.completed_rollouts.is_empty());
    }

    #[test]
    fn stage0_shc_harbor_live_smoke_imports_as_agent_failure() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let mut import = live_import_for_candidate(
            &candidate,
            "terminal-bench/db-wal-recovery",
            ProbeGepaLiveCloseoutState::AgentFailure,
        );
        import.assignment_id =
            "benchmark_run.probe.shc_harbor.db_wal_recovery.20260608".to_string();
        import.closeout_ref =
            "probe_closeout.probe.shc_harbor.db_wal_recovery.20260608".to_string();
        import.probe_commit = "probe.main.archived-refactor".to_string();
        import.backend_model_ref = "backend.codex.gpt-5.5.harbor.v1".to_string();
        import.scalar_score_bps = 0;
        import.failure_family = ProbeGepaFailureClass::RunnerSupervision;
        import.artifact_manifest_ref =
            "artifact_manifest.probe.shc_harbor.db_wal_recovery.20260608".to_string();
        import.proof_bundle_ref =
            "proof_bundle.probe.shc_harbor.db_wal_recovery.20260608".to_string();
        import.resource_usage_ref =
            "resource_usage_unavailable.probe.shc_harbor.db_wal_recovery.20260608".to_string();
        import.route_scorecard_ref =
            "route_scorecard.probe.shc_harbor.db_wal_recovery.20260608".to_string();
        import.verifier_refs = vec![
            "harbor_job.e487217a-715e-448c-8d45-e528b76980e7".to_string(),
            "harbor_trial.a6c6c245-b9c0-44a8-a8c0-0c7fe5cc3383".to_string(),
            "harbor_result.sha256:1a5bba9286f507a5e6923c0a67d94a0a9d8801cd016f50fec84ff49c08629528"
                .to_string(),
        ];
        import.policy_findings = vec!["no_public_benchmark_score_claimed".to_string()];
        import.public_claim = ProbeGepaLivePublicClaim::None;

        let receipt = import_probe_gepa_live_closeout(&mut state, &candidate, import).unwrap();

        assert_eq!(
            receipt.closeout_state,
            ProbeGepaLiveCloseoutState::AgentFailure
        );
        assert_eq!(
            receipt.imported_result_status,
            ProbeGepaRolloutStatus::AgentFailed
        );
        assert_eq!(state.candidate_frontier[0].agent_failures, 1);
        assert_eq!(state.candidate_frontier[0].mean_score_bps, 0);
    }

    #[test]
    fn terminal_bench_pylon_canary_import_updates_frontier_without_overclaim() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let import = canonical_probe_gepa_terminal_bench_pylon_canary_import(&candidate);

        let receipt = import_probe_gepa_live_closeout(&mut state, &candidate, import).unwrap();

        assert_eq!(
            receipt.assignment_id,
            "assignment.public.probe_gepa.terminal_bench_2.canary.20260608151057"
        );
        assert_eq!(
            receipt.imported_result_status,
            ProbeGepaRolloutStatus::AgentFailed
        );
        assert_eq!(receipt.payment_mode, ProbeGepaLivePaymentMode::UnpaidSmoke);
        assert_eq!(
            receipt.closeout_state,
            ProbeGepaLiveCloseoutState::AgentFailure
        );
        assert_eq!(state.live_closeout_imports.len(), 1);
        assert_eq!(state.candidate_frontier.len(), 1);
        assert_eq!(state.candidate_frontier[0].agent_failures, 1);
        assert_eq!(state.candidate_frontier[0].successful_rollouts, 0);
        assert_eq!(state.candidate_frontier[0].mean_score_bps, 0);
        assert_eq!(
            state.live_closeout_imports[0].route_scorecard_ref,
            "route_scorecard.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057"
        );
        assert_eq!(
            state.live_closeout_imports[0].settlement_receipt_refs.len(),
            0
        );
        assert_eq!(
            state.live_closeout_imports[0].public_claim,
            ProbeGepaLivePublicClaim::None
        );
        assert_eq!(
            state
                .last_iteration_metrics
                .as_ref()
                .unwrap()
                .pylon_assignment_count,
            1
        );
        assert_eq!(
            state.completed_rollouts[0].artifact_manifest_ref.as_deref(),
            Some("artifact.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057.probe_closeout_bundle")
        );
        assert_eq!(
            state.completed_rollouts[0].proof_bundle_ref.as_deref(),
            Some("proof.public.probe_run.public.probe_gepa.terminal_bench_2.canary.20260608151057.no_spend_assignment_lifecycle")
        );
    }

    #[test]
    fn terminal_bench_pylon_canary_import_rejects_score_training_or_settlement_overclaim() {
        let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        let mut state = ProbeGepaCoordinatorState::default();
        let mut public_score = canonical_probe_gepa_terminal_bench_pylon_canary_import(&candidate);
        public_score.public_claim = ProbeGepaLivePublicClaim::PublicBenchmarkScore;
        assert!(matches!(
            import_probe_gepa_live_closeout(&mut state, &candidate, public_score),
            Err(ProbeGepaCoordinatorError::LiveImportAuthorityOverclaim { field, .. })
                if field == "public_claim"
        ));

        let mut training = canonical_probe_gepa_terminal_bench_pylon_canary_import(&candidate);
        training.model_training_authority_claimed = true;
        assert!(matches!(
            import_probe_gepa_live_closeout(&mut state, &candidate, training),
            Err(ProbeGepaCoordinatorError::LiveImportAuthorityOverclaim { field, .. })
                if field == "model_training_authority_claimed"
        ));

        let mut settled = canonical_probe_gepa_terminal_bench_pylon_canary_import(&candidate);
        settled.payment_mode = ProbeGepaLivePaymentMode::SettledBitcoin;
        assert!(matches!(
            import_probe_gepa_live_closeout(&mut state, &candidate, settled),
            Err(ProbeGepaCoordinatorError::LiveImportMissingSettlementReceipt { .. })
        ));

        assert!(state.completed_rollouts.is_empty());
        assert!(state.candidate_frontier.is_empty());
    }

    fn live_import_for_candidate(
        candidate: &ProbeGepaCandidateManifest,
        task_id: &str,
        closeout_state: ProbeGepaLiveCloseoutState,
    ) -> ProbeGepaLiveCloseoutImport {
        ProbeGepaLiveCloseoutImport {
            schema_version: PROBE_GEPA_LIVE_CLOSEOUT_IMPORT_SCHEMA_VERSION.to_string(),
            assignment_id: format!("omega.assignment.{task_id}"),
            closeout_ref: format!("probe_closeout.{task_id}"),
            campaign_id: candidate.campaign_id.clone(),
            task_id: task_id.to_string(),
            dataset: "terminal_bench_2".to_string(),
            split_ref: candidate.split_refs[0].clone(),
            split: "retained".to_string(),
            probe_commit: "probe.commit.live-import-test".to_string(),
            agent_slug: "probe".to_string(),
            backend_model_ref: "backend.test.probe.live-import.v1".to_string(),
            candidate_hash: candidate.candidate_hash.clone(),
            candidate_id: candidate.candidate_id.clone(),
            selected_signature_refs: vec!["program_signature.probe.benchmark.test.v1".to_string()],
            tool_menu_ref: "tool_menu.probe.terminal_bench.test.v1".to_string(),
            verifier_refs: vec!["verifier.terminal_bench.test.v1".to_string()],
            closeout_state,
            payment_mode: ProbeGepaLivePaymentMode::UnpaidSmoke,
            settlement_receipt_refs: Vec::new(),
            scalar_score_bps: if closeout_state == ProbeGepaLiveCloseoutState::Accepted {
                10_000
            } else {
                0
            },
            failure_family: if closeout_state == ProbeGepaLiveCloseoutState::Accepted {
                ProbeGepaFailureClass::None
            } else {
                ProbeGepaFailureClass::AgentRegression
            },
            artifact_manifest_ref: format!("artifact_manifest.{task_id}"),
            proof_bundle_ref: format!("proof_bundle.{task_id}"),
            resource_usage_ref: format!("resource_usage.{task_id}"),
            route_scorecard_ref: format!("route_scorecard.{task_id}"),
            policy_findings: Vec::new(),
            duration_ms: 1_000,
            cost_micros: 0,
            public_claim: ProbeGepaLivePublicClaim::None,
            runtime_promotion_claim: ProbeGepaCandidatePromotionState::Draft,
            model_training_authority_claimed: false,
        }
    }
}
