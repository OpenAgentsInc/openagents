use serde::{Deserialize, Serialize};
use serde_json::json;

pub const BENCHMARK_TASK_SCHEMA_REF: &str = "openagents.benchmark_task.v1";
pub const BENCHMARK_RESULT_SCHEMA_REF: &str = "openagents.benchmark_result.v1";
pub const BENCHMARK_EVENT_SCHEMA_REF: &str = "openagents.benchmark_event.v1";
pub const BENCHMARK_ARTIFACT_MANIFEST_SCHEMA_REF: &str =
    "openagents.benchmark_artifact_manifest.v1";
pub const BENCHMARK_PROOF_BUNDLE_SCHEMA_REF: &str = "openagents.benchmark_proof_bundle.v1";
pub const RESOURCE_USAGE_RECEIPT_SCHEMA_REF: &str = "openagents.resource_usage_receipt.v1";
pub const BENCHMARK_SPLIT_MANIFEST_SCHEMA_REF: &str = "openagents.benchmark_split_manifest.v1";
pub const BENCHMARK_RUN_MANIFEST_SCHEMA_REF: &str = "openagents.benchmark_run_manifest.v1";
pub const BENCHMARK_CAMPAIGN_SPLIT_MANIFEST_SCHEMA_REF: &str =
    "openagents.benchmark_campaign_split_manifest.v1";
pub const PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF: &str = "probe.benchmark_assignment.v1";
pub const PYLON_BENCHMARK_WORKER_CAPABILITY_SCHEMA_REF: &str =
    "openagents.pylon_benchmark_worker_capability.v1";
pub const PYLON_BENCHMARK_WORK_REQUIREMENT_SCHEMA_REF: &str =
    "openagents.pylon_benchmark_work_requirement.v1";
pub const PROBE_GEPA_STAGE0_SMOKE_CAMPAIGN_SCHEMA_REF: &str =
    "openagents.probe_gepa_stage0_smoke_campaign.v1";
pub const PROBE_GEPA_STAGE1_RETAINED_SPRINT_SCHEMA_REF: &str =
    "openagents.probe_gepa_stage1_retained_sprint.v1";
pub const PROBE_GEPA_VALIDATION_SWEEP_SCHEMA_REF: &str =
    "openagents.probe_gepa_validation_sweep.v1";

pub const PROBE_RUNNER_REQUIRED_ARTIFACT_FILES: [&str; 8] = [
    "result.json",
    "events.jsonl",
    "metadata.json",
    "artifact_manifest.json",
    "proof_bundle.json",
    "resource_usage_receipt.json",
    "probe-run-record.json",
    "probe-closeout.json",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkEvidenceSplit {
    Retained,
    Validation,
    Holdout,
    Live,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    PolicyBlocked,
    Errored,
}

impl BenchmarkRunStatus {
    pub fn is_terminal_failure(&self) -> bool {
        matches!(
            self,
            Self::Failed | Self::TimedOut | Self::PolicyBlocked | Self::Errored
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkRedactionState {
    PublicSafe,
    Redacted,
    Withheld,
    UnsafeBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkEventKind {
    Assigned,
    Started,
    Progress,
    ArtifactRecorded,
    ProofRecorded,
    Completed,
    Failed,
    TimedOut,
    PolicyBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkPublicClaimLevel {
    None,
    RetainedSummary,
    ValidationSummary,
    HoldoutSummary,
    LiveClaim,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSplitLane {
    RetainedFixture,
    Validation,
    FrozenHoldout,
    LocalSmokeFixture,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkDatasetRef {
    pub slug: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScorerVerifierRef {
    pub scorer_ref: String,
    pub verifier_ref: String,
    pub verifier_public_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NoCheatMetadata {
    pub policy_ref: String,
    pub no_hidden_verifier_content: bool,
    pub no_private_repo_refs: bool,
    pub no_raw_logs: bool,
    pub no_task_solution_material: bool,
}

impl NoCheatMetadata {
    pub fn is_public_safe(&self) -> bool {
        self.no_hidden_verifier_content
            && self.no_private_repo_refs
            && self.no_raw_logs
            && self.no_task_solution_material
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkTask {
    pub schema_ref: String,
    pub task_ref: String,
    pub task_run_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub benchmark_suite_ref: String,
    pub split_ref: String,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub public_task_checksum: String,
    pub runner_ref: String,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkEvent {
    pub schema_ref: String,
    pub event_ref: String,
    pub run_ref: String,
    pub task_run_ref: String,
    pub event_kind: BenchmarkEventKind,
    pub observed_at: String,
    pub artifact_refs: Vec<String>,
    pub proof_bundle_refs: Vec<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkArtifactRef {
    pub artifact_ref: String,
    pub digest: String,
    pub media_type: String,
    pub public_url_ref: Option<String>,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkArtifactManifest {
    pub schema_ref: String,
    pub manifest_ref: String,
    pub run_ref: String,
    pub artifacts: Vec<BenchmarkArtifactRef>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkProofBundle {
    pub schema_ref: String,
    pub proof_bundle_ref: String,
    pub run_ref: String,
    pub artifact_manifest_refs: Vec<String>,
    pub scorer_verifier: ScorerVerifierRef,
    pub resource_usage_receipt_ref: Option<String>,
    pub probe_selected_signature_refs: Vec<String>,
    pub probe_tool_menu_ref: Option<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceTokenUsage {
    pub completion_tokens: Option<u64>,
    pub prompt_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub truth: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceUsageReceipt {
    pub schema_ref: String,
    pub receipt_ref: String,
    pub run_ref: String,
    pub worker_ref: String,
    pub duration_ms: Option<u64>,
    pub cpu_ms: Option<u64>,
    pub gpu_ms: Option<u64>,
    pub memory_peak_bytes: Option<u64>,
    pub token_usage: Option<ResourceTokenUsage>,
    pub cost_ref: Option<String>,
    pub unavailable_reason: Option<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSplitManifest {
    pub schema_ref: String,
    pub manifest_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub split_ref: String,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub task_refs: Vec<String>,
    pub frozen: bool,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRunManifest {
    pub schema_ref: String,
    pub run_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub split_manifest_refs: Vec<String>,
    pub agent_slug: String,
    pub candidate_hash: Option<String>,
    pub probe_commit: Option<String>,
    pub retry_policy_ref: String,
    pub scorer_verifier: ScorerVerifierRef,
    pub started_at: String,
    pub timeout_policy_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSplitTaskEntry {
    pub task_ref: String,
    pub task_id: String,
    pub public_task_checksum: String,
    pub lane: BenchmarkSplitLane,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub scorer_verifier: ScorerVerifierRef,
    pub allowed_claim_level: BenchmarkPublicClaimLevel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkCampaignSplitManifest {
    pub schema_ref: String,
    pub manifest_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub task_selector_version: String,
    pub task_order_ref: String,
    pub allowed_claim_state: BenchmarkPublicClaimLevel,
    pub retained_fixture_refs: Vec<String>,
    pub validation_task_refs: Vec<String>,
    pub frozen_holdout_task_refs: Vec<String>,
    pub local_smoke_fixture_refs: Vec<String>,
    pub scorer_verifier_versions: Vec<ScorerVerifierRef>,
    pub tasks: Vec<BenchmarkSplitTaskEntry>,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeBenchmarkAssignment {
    pub schema_ref: String,
    pub assignment_ref: String,
    pub benchmark_run_ref: String,
    pub task_run_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub split_ref: String,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub task_ref: String,
    pub public_task_checksum: String,
    pub probe_commit: String,
    pub runtime_ref: String,
    pub backend_profile_ref: String,
    pub backend_ref: String,
    pub model_backend_ref: String,
    pub provider_account_ref: Option<String>,
    pub auth_grant_ref: Option<String>,
    pub selected_blueprint_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub candidate_hash: String,
    pub timeout_policy_ref: String,
    pub budget_policy_ref: String,
    pub required_artifact_refs: Vec<String>,
    pub required_proof_bundle_refs: Vec<String>,
    pub callback_refs: Vec<String>,
    pub proof_sink_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeCommandInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub assignment_stdin_json: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeFakeRunnerOutcome {
    Pass,
    Timeout,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonBenchmarkIsolationProfile {
    HostLocal,
    Sandbox,
    ShcBox,
    Container,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonBenchmarkPayoutReadiness {
    NotReady,
    NoSpendOnly,
    CreditOnly,
    SettlementReady,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonBenchmarkWorkKind {
    BenchmarkEvaluation,
    GepaRolloutMetricCall,
    LoraFineTuning,
    ModelTraining,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GepaCandidateKind {
    Baseline,
    MutatedTextBundle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkCloseoutState {
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GepaCandidateDecisionState {
    OptimizerAccepted,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeValidationRouteKind {
    CurrentChampion,
    GepaCandidate,
    BaselineBackendRoute,
}

impl PylonBenchmarkWorkKind {
    pub fn requires_model_training(&self) -> bool {
        matches!(self, Self::LoraFineTuning | Self::ModelTraining)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonBenchmarkHardwareEnvelope {
    pub cpu_arch_ref: String,
    pub cpu_core_count: u32,
    pub disk_available_bytes: u64,
    pub gpu_memory_bytes: Option<u64>,
    pub gpu_refs: Vec<String>,
    pub ram_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonBenchmarkWorkerCapabilityEnvelope {
    pub schema_ref: String,
    pub capability_ref: String,
    pub worker_ref: String,
    pub pylon_version_ref: String,
    pub advertised_at: String,
    pub benchmark_runner_support: bool,
    pub harbor_terminal_bench_support: bool,
    pub probe_runtime_support: bool,
    pub local_model_support: bool,
    pub apple_fm_support: bool,
    pub qwen_adapter_support: bool,
    pub mlx_training_support: bool,
    pub rollout_eval_support: bool,
    pub model_training_support: bool,
    pub hardware: PylonBenchmarkHardwareEnvelope,
    pub max_wall_clock_ms: u64,
    pub max_cost_budget_ref: Option<String>,
    pub isolation_profile: PylonBenchmarkIsolationProfile,
    pub artifact_upload_support: bool,
    pub proof_receipt_support: bool,
    pub assignment_lease_support: bool,
    pub closeout_support: bool,
    pub payout_readiness: PylonBenchmarkPayoutReadiness,
    pub public_capability_refs: Vec<String>,
    pub caveat_refs: Vec<String>,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonBenchmarkWorkRequirement {
    pub schema_ref: String,
    pub work_ref: String,
    pub work_kind: PylonBenchmarkWorkKind,
    pub benchmark_suite_ref: String,
    pub task_ref: String,
    pub candidate_hash: Option<String>,
    pub requires_harbor_terminal_bench: bool,
    pub requires_probe_runtime: bool,
    pub requires_local_model: bool,
    pub requires_apple_fm: bool,
    pub requires_qwen_adapter: bool,
    pub requires_mlx_training: bool,
    pub min_cpu_core_count: u32,
    pub min_ram_bytes: u64,
    pub min_disk_available_bytes: u64,
    pub min_gpu_memory_bytes: Option<u64>,
    pub max_wall_clock_ms: u64,
    pub requires_artifact_upload: bool,
    pub requires_proof_receipts: bool,
    pub requires_assignment_lease: bool,
    pub requires_closeout: bool,
    pub accepted_payment_modes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonBenchmarkWorkerMatch {
    pub matched: bool,
    pub worker_ref: String,
    pub work_ref: String,
    pub work_kind: PylonBenchmarkWorkKind,
    pub blocker_refs: Vec<String>,
    pub payout_ready_for_paid_work: bool,
    pub admitted_for_assignment: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GepaTextBundleCandidate {
    pub candidate_ref: String,
    pub candidate_hash: String,
    pub parent_candidate_ref: Option<String>,
    pub candidate_kind: GepaCandidateKind,
    pub prompt_bundle_ref: String,
    pub blueprint_bundle_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeGepaMetricCallRecord {
    pub metric_call_ref: String,
    pub candidate_ref: String,
    pub candidate_hash: String,
    pub task_ref: String,
    pub task_id: String,
    pub verifier_ref: String,
    pub probe_assignment_ref: String,
    pub pylon_assignment_ref: Option<String>,
    pub payment_mode: String,
    pub probe_closeout_ref: String,
    pub probe_closeout_bundle_ref: String,
    pub benchmark_result_ref: String,
    pub artifact_manifest_ref: String,
    pub benchmark_cloud_proof_bundle_ref: String,
    pub resource_usage_receipt_ref: Option<String>,
    pub verifier_import_ref: String,
    pub verifier_result_ref: String,
    pub cost_ref: Option<String>,
    pub duration_ms: Option<u64>,
    pub artifact_available: bool,
    pub failure_classification_ref: Option<String>,
    pub closeout_state: BenchmarkCloseoutState,
    pub score_bps: Option<u32>,
    pub status: BenchmarkRunStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeGepaStage0SmokeCampaign {
    pub schema_ref: String,
    pub campaign_id: String,
    pub campaign_ref: String,
    pub split_manifest_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub retained_fixture_refs: Vec<String>,
    pub candidates: Vec<GepaTextBundleCandidate>,
    pub metric_calls: Vec<ProbeGepaMetricCallRecord>,
    pub pylon_worker_match_refs: Vec<String>,
    pub public_status_ref: String,
    pub public_status_label: String,
    pub promotion_state_ref: String,
    pub no_lora: bool,
    pub no_model_training: bool,
    pub no_public_leaderboard_claim: bool,
    pub automatic_promotion_enabled: bool,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateRetainedSummary {
    pub candidate_ref: String,
    pub candidate_hash: String,
    pub metric_call_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub mean_score_bps: u32,
    pub regression_count: usize,
    pub failure_classification_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeGepaStage1RetainedSprint {
    pub schema_ref: String,
    pub campaign_id: String,
    pub campaign_ref: String,
    pub split_manifest_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub retained_fixture_refs: Vec<String>,
    pub worker_assignment_refs: Vec<String>,
    pub candidates: Vec<GepaTextBundleCandidate>,
    pub metric_calls: Vec<ProbeGepaMetricCallRecord>,
    pub candidate_summaries: Vec<ProbeGepaCandidateRetainedSummary>,
    pub baseline_candidate_ref: String,
    pub champion_candidate_ref: String,
    pub selected_candidate_ref: String,
    pub selected_candidate_decision: GepaCandidateDecisionState,
    pub candidate_improves_or_preserves_retained_fixtures: bool,
    pub policy_gate_failure: bool,
    pub public_summary_ref: String,
    pub public_summary_label: String,
    pub public_summary_evidence_scope: String,
    pub no_lora: bool,
    pub no_model_training: bool,
    pub no_public_leaderboard_claim: bool,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeValidationRoute {
    pub route_ref: String,
    pub route_kind: ProbeValidationRouteKind,
    pub candidate_ref: Option<String>,
    pub candidate_hash: Option<String>,
    pub backend_route_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeGepaValidationSweep {
    pub schema_ref: String,
    pub campaign_id: String,
    pub sweep_ref: String,
    pub split_manifest_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub validation_task_refs: Vec<String>,
    pub probe_commit: String,
    pub gepa_candidate_hash: String,
    pub routes: Vec<ProbeValidationRoute>,
    pub rollout_records: Vec<ProbeGepaMetricCallRecord>,
    pub candidate_shadow_state_ref: String,
    pub candidate_may_move_to_shadow: bool,
    pub omega_blueprint_gate_refs: Vec<String>,
    pub public_claim_ref: String,
    pub public_claim_label: String,
    pub no_public_beats_terminal_bench_claim: bool,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeBenchmarkRunMetadata {
    pub runner_ref: String,
    pub benchmark_suite_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub task_ref: String,
    pub probe_command: ProbeCommandInvocation,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
    pub safe_account_grant_refs: bool,
    pub redaction_state: BenchmarkRedactionState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeBenchmarkRunnerArtifactSet {
    pub result_json: BenchmarkResult,
    pub events_jsonl: Vec<BenchmarkEvent>,
    pub metadata_json: ProbeBenchmarkRunMetadata,
    pub artifact_manifest_json: BenchmarkArtifactManifest,
    pub proof_bundle_json: BenchmarkProofBundle,
    pub resource_usage_receipt_json: ResourceUsageReceipt,
    pub probe_run_record_json: serde_json::Value,
    pub probe_closeout_json: serde_json::Value,
}

impl ProbeBenchmarkRunnerArtifactSet {
    pub fn file_names(&self) -> Vec<&'static str> {
        PROBE_RUNNER_REQUIRED_ARTIFACT_FILES.to_vec()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeCloseoutImport {
    pub probe_assignment_ref: String,
    pub probe_closeout_ref: String,
    pub probe_commit: String,
    pub probe_run_record_ref: String,
    pub probe_run_ref: String,
    pub candidate_hash: String,
    pub selected_signature_refs: Vec<String>,
    pub tool_menu_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkPublicClaimBoundary {
    pub claim_level: BenchmarkPublicClaimLevel,
    pub external_release_gate_refs: Vec<String>,
    pub public_claim_upgrade_authority: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub schema_ref: String,
    pub result_ref: String,
    pub run_ref: String,
    pub task_run_ref: String,
    pub dataset: BenchmarkDatasetRef,
    pub evidence_split: BenchmarkEvidenceSplit,
    pub status: BenchmarkRunStatus,
    pub scorer_verifier: ScorerVerifierRef,
    pub score_bps: Option<u32>,
    pub failure_classification_ref: Option<String>,
    pub artifact_manifest_refs: Vec<String>,
    pub proof_bundle_refs: Vec<String>,
    pub resource_usage_receipt_ref: Option<String>,
    pub resource_unavailable_reason: Option<String>,
    pub probe_closeout_import: Option<ProbeCloseoutImport>,
    pub no_cheat: NoCheatMetadata,
    pub redaction_state: BenchmarkRedactionState,
    pub public_claim_boundary: BenchmarkPublicClaimBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkContractError {
    MissingArtifactManifest {
        result_ref: String,
    },
    MissingProofBundle {
        result_ref: String,
    },
    MissingFailureClassification {
        result_ref: String,
    },
    MissingResourceUsage {
        result_ref: String,
    },
    UnsafeNoCheatMetadata {
        result_ref: String,
    },
    ProbeCloseoutImportIncomplete {
        result_ref: String,
    },
    PublicClaimUpgradeAuthority {
        result_ref: String,
    },
    NonLivePublicClaim {
        result_ref: String,
    },
    MissingPublicClaimReleaseGate {
        result_ref: String,
    },
    ManifestInvalid {
        manifest_ref: String,
        reason: String,
    },
    ProbeRunnerInvalid {
        run_ref: String,
        reason: String,
    },
    WorkerCapabilityInvalid {
        capability_ref: String,
        reason: String,
    },
    WorkerCapabilityNotMatched {
        capability_ref: String,
        work_ref: String,
        blocker_refs: Vec<String>,
    },
    Stage0SmokeInvalid {
        campaign_ref: String,
        reason: String,
    },
    Stage1SprintInvalid {
        campaign_ref: String,
        reason: String,
    },
    ValidationSweepInvalid {
        sweep_ref: String,
        reason: String,
    },
}

pub fn validate_benchmark_result(result: &BenchmarkResult) -> Result<(), BenchmarkContractError> {
    if result.artifact_manifest_refs.is_empty() {
        return Err(BenchmarkContractError::MissingArtifactManifest {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.proof_bundle_refs.is_empty() {
        return Err(BenchmarkContractError::MissingProofBundle {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.status.is_terminal_failure() && result.failure_classification_ref.is_none() {
        return Err(BenchmarkContractError::MissingFailureClassification {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.resource_usage_receipt_ref.is_none() && result.resource_unavailable_reason.is_none() {
        return Err(BenchmarkContractError::MissingResourceUsage {
            result_ref: result.result_ref.clone(),
        });
    }

    if !result.no_cheat.is_public_safe() {
        return Err(BenchmarkContractError::UnsafeNoCheatMetadata {
            result_ref: result.result_ref.clone(),
        });
    }

    if let Some(import) = &result.probe_closeout_import {
        if import.probe_assignment_ref.is_empty()
            || import.probe_closeout_ref.is_empty()
            || import.probe_run_record_ref.is_empty()
            || import.probe_run_ref.is_empty()
            || import.candidate_hash.is_empty()
        {
            return Err(BenchmarkContractError::ProbeCloseoutImportIncomplete {
                result_ref: result.result_ref.clone(),
            });
        }
    }

    if result.public_claim_boundary.public_claim_upgrade_authority {
        return Err(BenchmarkContractError::PublicClaimUpgradeAuthority {
            result_ref: result.result_ref.clone(),
        });
    }

    if result.public_claim_boundary.claim_level != BenchmarkPublicClaimLevel::None {
        if result.evidence_split != BenchmarkEvidenceSplit::Live {
            return Err(BenchmarkContractError::NonLivePublicClaim {
                result_ref: result.result_ref.clone(),
            });
        }

        if result
            .public_claim_boundary
            .external_release_gate_refs
            .is_empty()
        {
            return Err(BenchmarkContractError::MissingPublicClaimReleaseGate {
                result_ref: result.result_ref.clone(),
            });
        }
    }

    Ok(())
}

pub fn validate_campaign_split_manifest(
    manifest: &BenchmarkCampaignSplitManifest,
) -> Result<(), BenchmarkContractError> {
    if manifest.tasks.is_empty() {
        return manifest_error(
            manifest,
            "manifest must include at least one public task ref",
        );
    }

    if !manifest.no_cheat.is_public_safe() {
        return manifest_error(manifest, "no-cheat metadata must be public-safe");
    }

    if manifest.allowed_claim_state != BenchmarkPublicClaimLevel::None {
        return manifest_error(
            manifest,
            "Stage 0/1 Probe GEPA split manifests cannot authorize public claims",
        );
    }

    let mut task_refs = std::collections::BTreeSet::new();

    for task in &manifest.tasks {
        if !task_refs.insert(task.task_ref.clone()) {
            return manifest_error(manifest, "task refs must be unique");
        }

        if task.allowed_claim_level != BenchmarkPublicClaimLevel::None
            && task.evidence_split != BenchmarkEvidenceSplit::Live
        {
            return manifest_error(
                manifest,
                "non-live task entries cannot carry public claim levels",
            );
        }
    }

    require_lane_refs(
        manifest,
        &manifest.retained_fixture_refs,
        BenchmarkSplitLane::RetainedFixture,
        BenchmarkEvidenceSplit::Retained,
        "retained_fixture_refs",
    )?;
    require_lane_refs(
        manifest,
        &manifest.validation_task_refs,
        BenchmarkSplitLane::Validation,
        BenchmarkEvidenceSplit::Validation,
        "validation_task_refs",
    )?;
    require_lane_refs(
        manifest,
        &manifest.frozen_holdout_task_refs,
        BenchmarkSplitLane::FrozenHoldout,
        BenchmarkEvidenceSplit::Holdout,
        "frozen_holdout_task_refs",
    )?;
    require_lane_refs(
        manifest,
        &manifest.local_smoke_fixture_refs,
        BenchmarkSplitLane::LocalSmokeFixture,
        BenchmarkEvidenceSplit::Retained,
        "local_smoke_fixture_refs",
    )?;

    Ok(())
}

fn require_lane_refs(
    manifest: &BenchmarkCampaignSplitManifest,
    refs: &[String],
    lane: BenchmarkSplitLane,
    evidence_split: BenchmarkEvidenceSplit,
    list_name: &str,
) -> Result<(), BenchmarkContractError> {
    if refs.is_empty() {
        return manifest_error(manifest, format!("{list_name} must not be empty"));
    }

    for task_ref in refs {
        let Some(task) = manifest
            .tasks
            .iter()
            .find(|task| &task.task_ref == task_ref)
        else {
            return manifest_error(
                manifest,
                format!("{list_name} contains task ref not present in tasks: {task_ref}"),
            );
        };

        if task.lane != lane || task.evidence_split != evidence_split {
            return manifest_error(
                manifest,
                format!("{list_name} contains task ref with mismatched lane or split: {task_ref}"),
            );
        }
    }

    Ok(())
}

fn manifest_error(
    manifest: &BenchmarkCampaignSplitManifest,
    reason: impl Into<String>,
) -> Result<(), BenchmarkContractError> {
    Err(BenchmarkContractError::ManifestInvalid {
        manifest_ref: manifest.manifest_ref.clone(),
        reason: reason.into(),
    })
}

pub fn probe_assignment_from_split_task(
    manifest: &BenchmarkCampaignSplitManifest,
    task: &BenchmarkSplitTaskEntry,
    probe_commit: impl Into<String>,
    candidate_hash: impl Into<String>,
    selected_blueprint_signature_refs: Vec<String>,
    tool_menu_ref: impl Into<String>,
) -> ProbeBenchmarkAssignment {
    ProbeBenchmarkAssignment {
        schema_ref: String::from(PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF),
        assignment_ref: format!("probe_benchmark_assignment.{}", task.task_id),
        benchmark_run_ref: String::from("benchmark_run.terminal_bench_2.probe_gepa.stage_0_1"),
        task_run_ref: format!("task_run.{}", task.task_id),
        dataset: manifest.dataset.clone(),
        split_ref: manifest.manifest_ref.clone(),
        evidence_split: task.evidence_split.clone(),
        task_ref: task.task_ref.clone(),
        public_task_checksum: task.public_task_checksum.clone(),
        probe_commit: probe_commit.into(),
        runtime_ref: String::from("runtime.probe.v1"),
        backend_profile_ref: String::from("backend_profile.probe.default.v1"),
        backend_ref: String::from("probe.backend.runtime"),
        model_backend_ref: String::from("model_backend.probe.default"),
        provider_account_ref: Some(String::from("provider_account.probe.fixture")),
        auth_grant_ref: Some(String::from("provider_grant.probe.fixture")),
        selected_blueprint_signature_refs,
        tool_menu_ref: tool_menu_ref.into(),
        candidate_hash: candidate_hash.into(),
        timeout_policy_ref: String::from("timeout_policy.benchmark_cloud.probe.default.v1"),
        budget_policy_ref: String::from("budget_policy.benchmark_cloud.probe.no_spend.v1"),
        required_artifact_refs: vec![String::from("artifact_manifest.required.probe.closeout.v1")],
        required_proof_bundle_refs: vec![String::from("proof_bundle.required.probe.closeout.v1")],
        callback_refs: vec![String::from("callback.benchmark_cloud.probe.v1")],
        proof_sink_refs: vec![String::from("proof_sink.benchmark_cloud.probe.v1")],
    }
}

pub fn build_probe_command_invocation(
    assignment: &ProbeBenchmarkAssignment,
) -> Result<ProbeCommandInvocation, BenchmarkContractError> {
    validate_probe_assignment_public_refs(assignment)?;
    let assignment_stdin_json = serde_json::to_value(assignment).map_err(|error| {
        BenchmarkContractError::ProbeRunnerInvalid {
            run_ref: assignment.task_run_ref.clone(),
            reason: format!("failed to serialize Probe assignment JSON: {error}"),
        }
    })?;

    Ok(ProbeCommandInvocation {
        program: String::from("probe"),
        args: vec![
            String::from("benchmark"),
            String::from("run"),
            String::from("--assignment-json"),
            String::from("-"),
        ],
        assignment_stdin_json,
    })
}

pub fn validate_pylon_benchmark_worker_capability(
    capability: &PylonBenchmarkWorkerCapabilityEnvelope,
) -> Result<(), BenchmarkContractError> {
    if capability.schema_ref != PYLON_BENCHMARK_WORKER_CAPABILITY_SCHEMA_REF {
        return worker_capability_error(capability, "unsupported worker capability schema ref");
    }

    if capability.redaction_state != BenchmarkRedactionState::PublicSafe {
        return worker_capability_error(capability, "worker capability must be public-safe");
    }

    if capability.capability_ref.is_empty() || capability.worker_ref.is_empty() {
        return worker_capability_error(
            capability,
            "worker capability must include capability and worker refs",
        );
    }

    if capability.rollout_eval_support && !capability.benchmark_runner_support {
        return worker_capability_error(
            capability,
            "rollout/eval support requires benchmark runner support",
        );
    }

    if capability.probe_runtime_support && !capability.benchmark_runner_support {
        return worker_capability_error(
            capability,
            "Probe runtime support requires benchmark runner support",
        );
    }

    if capability.mlx_training_support && !capability.model_training_support {
        return worker_capability_error(
            capability,
            "MLX training support requires explicit model training support",
        );
    }

    if capability.model_training_support && !capability.local_model_support {
        return worker_capability_error(
            capability,
            "model training support requires local model support",
        );
    }

    if capability.max_wall_clock_ms == 0 {
        return worker_capability_error(
            capability,
            "worker capability must include a non-zero max wall-clock budget",
        );
    }

    if capability.hardware.cpu_core_count == 0 || capability.hardware.ram_bytes == 0 {
        return worker_capability_error(
            capability,
            "worker capability must include CPU and RAM capacity",
        );
    }

    if worker_capability_contains_unsafe_material(capability) {
        return worker_capability_error(
            capability,
            "worker capability contains private, credential, raw log, payment, payout, or private repo material",
        );
    }

    Ok(())
}

pub fn match_pylon_benchmark_worker(
    capability: &PylonBenchmarkWorkerCapabilityEnvelope,
    requirement: &PylonBenchmarkWorkRequirement,
) -> Result<PylonBenchmarkWorkerMatch, BenchmarkContractError> {
    validate_pylon_benchmark_worker_capability(capability)?;

    if requirement.schema_ref != PYLON_BENCHMARK_WORK_REQUIREMENT_SCHEMA_REF {
        return Err(BenchmarkContractError::WorkerCapabilityInvalid {
            capability_ref: capability.capability_ref.clone(),
            reason: String::from("unsupported benchmark work requirement schema ref"),
        });
    }

    if worker_requirement_contains_unsafe_material(requirement) {
        return Err(BenchmarkContractError::WorkerCapabilityInvalid {
            capability_ref: capability.capability_ref.clone(),
            reason: String::from("benchmark work requirement contains unsafe material"),
        });
    }

    let blocker_refs = pylon_benchmark_worker_blockers(capability, requirement);
    let matched = blocker_refs.is_empty();
    let payout_ready_for_paid_work = matches!(
        capability.payout_readiness,
        PylonBenchmarkPayoutReadiness::SettlementReady
    );

    Ok(PylonBenchmarkWorkerMatch {
        matched,
        worker_ref: capability.worker_ref.clone(),
        work_ref: requirement.work_ref.clone(),
        work_kind: requirement.work_kind.clone(),
        blocker_refs,
        payout_ready_for_paid_work,
        admitted_for_assignment: matched,
    })
}

pub fn build_probe_gepa_stage0_smoke_campaign(
    manifest: &BenchmarkCampaignSplitManifest,
    probe_commit: impl Into<String>,
) -> Result<ProbeGepaStage0SmokeCampaign, BenchmarkContractError> {
    validate_campaign_split_manifest(manifest)?;

    let probe_commit = probe_commit.into();
    let retained_tasks = manifest
        .tasks
        .iter()
        .filter(|task| {
            task.lane == BenchmarkSplitLane::RetainedFixture
                && task.evidence_split == BenchmarkEvidenceSplit::Retained
        })
        .take(5)
        .collect::<Vec<_>>();

    if retained_tasks.len() < 3 {
        return stage0_smoke_error(
            "campaign.probe_gepa.stage0.retained_smoke.2026_06_08",
            "Stage 0 smoke requires at least three retained fixtures",
        );
    }

    let candidates = probe_gepa_stage0_smoke_candidates();
    let pylon_worker_match_refs = vec![
        String::from("pylon_assignment_ref.public.stage0.shc_box_1"),
        String::from("pylon_assignment_ref.public.stage0.shc_box_2"),
    ];
    let mut metric_calls = Vec::new();

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        for (task_index, task) in retained_tasks.iter().enumerate() {
            let assignment = probe_assignment_from_split_task(
                manifest,
                task,
                probe_commit.clone(),
                candidate.candidate_hash.clone(),
                vec![String::from(
                    "program_signature.probe.benchmark.service_readiness.v1",
                )],
                "tool_menu.probe.terminal_bench.service_readiness.v1",
            );
            let outcome = stage0_smoke_outcome(candidate_index, task_index);
            let artifacts =
                run_fake_probe_benchmark_task(manifest, task, &assignment, outcome.clone())?;
            let metric_index = metric_calls.len() + 1;
            let closeout_state = if artifacts.result_json.status == BenchmarkRunStatus::Succeeded {
                BenchmarkCloseoutState::Accepted
            } else {
                BenchmarkCloseoutState::Rejected
            };

            metric_calls.push(ProbeGepaMetricCallRecord {
                metric_call_ref: format!("metric_call.probe_gepa.stage0.{:02}", metric_index),
                candidate_ref: candidate.candidate_ref.clone(),
                candidate_hash: candidate.candidate_hash.clone(),
                task_ref: task.task_ref.clone(),
                task_id: task.task_id.clone(),
                verifier_ref: task.scorer_verifier.verifier_ref.clone(),
                probe_assignment_ref: assignment.assignment_ref.clone(),
                pylon_assignment_ref: Some(format!(
                    "{}.{}",
                    pylon_worker_match_refs[metric_index % pylon_worker_match_refs.len()],
                    metric_index
                )),
                payment_mode: String::from("unpaid_smoke"),
                probe_closeout_ref: artifacts
                    .result_json
                    .probe_closeout_import
                    .as_ref()
                    .map(|import| import.probe_closeout_ref.clone())
                    .unwrap_or_default(),
                probe_closeout_bundle_ref: format!(
                    "probe_closeout_bundle.probe_gepa.stage0.{:02}",
                    metric_index
                ),
                benchmark_result_ref: artifacts.result_json.result_ref.clone(),
                artifact_manifest_ref: artifacts.artifact_manifest_json.manifest_ref.clone(),
                benchmark_cloud_proof_bundle_ref: artifacts.proof_bundle_json.proof_bundle_ref,
                resource_usage_receipt_ref: Some(
                    artifacts.resource_usage_receipt_json.receipt_ref.clone(),
                ),
                verifier_import_ref: format!(
                    "verifier_import.probe_gepa.stage0.{:02}",
                    metric_index
                ),
                verifier_result_ref: format!(
                    "verifier_result.probe_gepa.stage0.{:02}",
                    metric_index
                ),
                cost_ref: artifacts.resource_usage_receipt_json.cost_ref.clone(),
                duration_ms: artifacts.resource_usage_receipt_json.duration_ms,
                artifact_available: true,
                failure_classification_ref: artifacts.result_json.failure_classification_ref,
                closeout_state,
                score_bps: artifacts.result_json.score_bps,
                status: artifacts.result_json.status,
            });
        }
    }

    let campaign = ProbeGepaStage0SmokeCampaign {
        schema_ref: String::from(PROBE_GEPA_STAGE0_SMOKE_CAMPAIGN_SCHEMA_REF),
        campaign_id: String::from("probe-gepa-stage0-retained-smoke-2026-06-08"),
        campaign_ref: String::from("campaign.probe_gepa.stage0.retained_smoke.2026_06_08"),
        split_manifest_ref: manifest.manifest_ref.clone(),
        benchmark_suite_ref: manifest.benchmark_suite_ref.clone(),
        dataset: manifest.dataset.clone(),
        retained_fixture_refs: retained_tasks
            .iter()
            .map(|task| task.task_ref.clone())
            .collect(),
        candidates,
        metric_calls,
        pylon_worker_match_refs,
        public_status_ref: String::from("public_status.probe_gepa.measured_retained_smoke.v1"),
        public_status_label: String::from("measured retained smoke"),
        promotion_state_ref: String::from("promotion_state.probe_gepa.stage0.no_promotion.v1"),
        no_lora: true,
        no_model_training: true,
        no_public_leaderboard_claim: true,
        automatic_promotion_enabled: false,
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };

    validate_probe_gepa_stage0_smoke_campaign(&campaign, manifest)?;
    Ok(campaign)
}

pub fn validate_probe_gepa_stage0_smoke_campaign(
    campaign: &ProbeGepaStage0SmokeCampaign,
    manifest: &BenchmarkCampaignSplitManifest,
) -> Result<(), BenchmarkContractError> {
    validate_campaign_split_manifest(manifest)?;

    if campaign.schema_ref != PROBE_GEPA_STAGE0_SMOKE_CAMPAIGN_SCHEMA_REF {
        return stage0_smoke_error(&campaign.campaign_ref, "unsupported campaign schema ref");
    }

    if campaign.campaign_id.is_empty() || campaign.campaign_ref.is_empty() {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Stage 0 smoke campaign must include id and ref",
        );
    }

    if campaign.split_manifest_ref != manifest.manifest_ref {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "campaign split manifest ref must match validated manifest",
        );
    }

    if campaign.retained_fixture_refs.len() < 3 || campaign.retained_fixture_refs.len() > 6 {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Stage 0 smoke must use three to six retained fixtures",
        );
    }

    if campaign.metric_calls.len() < 20 || campaign.metric_calls.len() > 40 {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Stage 0 smoke must contain twenty to forty metric calls",
        );
    }

    if campaign.candidates.len() < 2
        || !campaign
            .candidates
            .iter()
            .any(|candidate| candidate.candidate_kind == GepaCandidateKind::Baseline)
        || !campaign
            .candidates
            .iter()
            .any(|candidate| candidate.candidate_kind == GepaCandidateKind::MutatedTextBundle)
    {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "campaign must include a baseline and at least one mutated text-bundle candidate",
        );
    }

    if !campaign.no_lora
        || !campaign.no_model_training
        || !campaign.no_public_leaderboard_claim
        || campaign.automatic_promotion_enabled
    {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Stage 0 smoke cannot enable LoRA, model training, public leaderboard claims, or automatic promotion",
        );
    }

    if campaign.public_status_ref != "public_status.probe_gepa.measured_retained_smoke.v1"
        || campaign.public_status_label != "measured retained smoke"
    {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "campaign public status must be measured retained smoke",
        );
    }

    if campaign.redaction_state != BenchmarkRedactionState::PublicSafe {
        return stage0_smoke_error(&campaign.campaign_ref, "campaign must be public-safe");
    }

    let retained_refs = campaign
        .retained_fixture_refs
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let candidate_hashes = campaign
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_hash.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    if candidate_hashes.len() != campaign.candidates.len() {
        return stage0_smoke_error(&campaign.campaign_ref, "candidate hashes must be unique");
    }

    for candidate in &campaign.candidates {
        if candidate.candidate_ref.is_empty()
            || candidate.candidate_hash.is_empty()
            || candidate.prompt_bundle_ref.is_empty()
            || candidate.blueprint_bundle_ref.is_empty()
        {
            return stage0_smoke_error(
                &campaign.campaign_ref,
                "candidate refs, hashes, prompt bundle refs, and Blueprint bundle refs are required",
            );
        }
    }

    let mut metric_refs = std::collections::BTreeSet::new();
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for metric_call in &campaign.metric_calls {
        if !metric_refs.insert(&metric_call.metric_call_ref) {
            return stage0_smoke_error(&campaign.campaign_ref, "metric call refs must be unique");
        }

        if !retained_refs.contains(&metric_call.task_ref) {
            return stage0_smoke_error(
                &campaign.campaign_ref,
                "Stage 0 metric calls must use retained fixtures only",
            );
        }

        if !candidate_hashes.contains(metric_call.candidate_hash.as_str()) {
            return stage0_smoke_error(
                &campaign.campaign_ref,
                "metric call candidate hash must reference campaign candidate",
            );
        }

        if metric_call.probe_assignment_ref.is_empty()
            || metric_call.probe_closeout_ref.is_empty()
            || metric_call.probe_closeout_bundle_ref.is_empty()
            || metric_call.benchmark_result_ref.is_empty()
            || metric_call.artifact_manifest_ref.is_empty()
            || metric_call.benchmark_cloud_proof_bundle_ref.is_empty()
            || metric_call.verifier_import_ref.is_empty()
        {
            return stage0_smoke_error(
                &campaign.campaign_ref,
                "metric calls must carry Probe closeout, Benchmark Cloud proof, result, artifact, and verifier import refs",
            );
        }

        match metric_call.closeout_state {
            BenchmarkCloseoutState::Accepted => accepted += 1,
            BenchmarkCloseoutState::Rejected => rejected += 1,
        }
    }

    if accepted == 0 || rejected == 0 {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Stage 0 smoke must include accepted and rejected closeout refs",
        );
    }

    if campaign.metric_calls.iter().any(|metric_call| {
        metric_call.pylon_assignment_ref.is_some() && campaign.pylon_worker_match_refs.is_empty()
    }) {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Pylon assignment refs require campaign worker match refs",
        );
    }

    if serde_json::to_value(campaign)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
    {
        return stage0_smoke_error(
            &campaign.campaign_ref,
            "Stage 0 smoke campaign contains unsafe material",
        );
    }

    Ok(())
}

pub fn build_probe_gepa_stage1_retained_sprint(
    manifest: &BenchmarkCampaignSplitManifest,
    probe_commit: impl Into<String>,
) -> Result<ProbeGepaStage1RetainedSprint, BenchmarkContractError> {
    validate_campaign_split_manifest(manifest)?;

    let probe_commit = probe_commit.into();
    let retained_tasks = manifest
        .tasks
        .iter()
        .filter(|task| {
            task.lane == BenchmarkSplitLane::RetainedFixture
                && task.evidence_split == BenchmarkEvidenceSplit::Retained
        })
        .collect::<Vec<_>>();
    let candidates = probe_gepa_stage1_sprint_candidates();
    let worker_assignment_refs = (1..=8)
        .map(|worker_index| format!("pylon_assignment_ref.public.stage1.worker_{worker_index:02}"))
        .collect::<Vec<_>>();
    let mut metric_calls = Vec::new();

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        for repeat_index in 0..3 {
            for (task_index, task) in retained_tasks.iter().enumerate() {
                let assignment = probe_assignment_from_split_task(
                    manifest,
                    task,
                    probe_commit.clone(),
                    candidate.candidate_hash.clone(),
                    vec![String::from(
                        "program_signature.probe.benchmark.service_readiness.v1",
                    )],
                    "tool_menu.probe.terminal_bench.service_readiness.v1",
                );
                let outcome = stage1_sprint_outcome(candidate_index, task_index, repeat_index);
                let artifacts =
                    run_fake_probe_benchmark_task(manifest, task, &assignment, outcome.clone())?;
                let metric_index = metric_calls.len() + 1;
                let closeout_state =
                    if artifacts.result_json.status == BenchmarkRunStatus::Succeeded {
                        BenchmarkCloseoutState::Accepted
                    } else {
                        BenchmarkCloseoutState::Rejected
                    };

                metric_calls.push(ProbeGepaMetricCallRecord {
                    metric_call_ref: format!("metric_call.probe_gepa.stage1.{:03}", metric_index),
                    candidate_ref: candidate.candidate_ref.clone(),
                    candidate_hash: candidate.candidate_hash.clone(),
                    task_ref: task.task_ref.clone(),
                    task_id: task.task_id.clone(),
                    verifier_ref: task.scorer_verifier.verifier_ref.clone(),
                    probe_assignment_ref: assignment.assignment_ref.clone(),
                    pylon_assignment_ref: Some(format!(
                        "{}.{}",
                        worker_assignment_refs[(metric_index - 1) % worker_assignment_refs.len()],
                        metric_index
                    )),
                    payment_mode: String::from("unpaid_smoke"),
                    probe_closeout_ref: artifacts
                        .result_json
                        .probe_closeout_import
                        .as_ref()
                        .map(|import| import.probe_closeout_ref.clone())
                        .unwrap_or_default(),
                    probe_closeout_bundle_ref: format!(
                        "probe_closeout_bundle.probe_gepa.stage1.{:03}",
                        metric_index
                    ),
                    benchmark_result_ref: format!(
                        "{}.stage1.{:03}",
                        artifacts.result_json.result_ref, metric_index
                    ),
                    artifact_manifest_ref: format!(
                        "{}.stage1.{:03}",
                        artifacts.artifact_manifest_json.manifest_ref, metric_index
                    ),
                    benchmark_cloud_proof_bundle_ref: format!(
                        "{}.stage1.{:03}",
                        artifacts.proof_bundle_json.proof_bundle_ref, metric_index
                    ),
                    resource_usage_receipt_ref: Some(format!(
                        "{}.stage1.{:03}",
                        artifacts.resource_usage_receipt_json.receipt_ref, metric_index
                    )),
                    verifier_import_ref: format!(
                        "verifier_import.probe_gepa.stage1.{:03}",
                        metric_index
                    ),
                    verifier_result_ref: format!(
                        "verifier_result.probe_gepa.stage1.{:03}",
                        metric_index
                    ),
                    cost_ref: artifacts.resource_usage_receipt_json.cost_ref.clone(),
                    duration_ms: artifacts.resource_usage_receipt_json.duration_ms,
                    artifact_available: true,
                    failure_classification_ref: artifacts.result_json.failure_classification_ref,
                    closeout_state,
                    score_bps: artifacts.result_json.score_bps,
                    status: artifacts.result_json.status,
                });
            }
        }
    }

    let candidate_summaries = retained_summaries_for_candidates(&candidates, &metric_calls);
    let selected_candidate_ref = String::from("candidate.probe_gepa.stage1.mutation_08");
    let campaign = ProbeGepaStage1RetainedSprint {
        schema_ref: String::from(PROBE_GEPA_STAGE1_RETAINED_SPRINT_SCHEMA_REF),
        campaign_id: String::from("probe-gepa-stage1-retained-failure-sprint-2026-06-08"),
        campaign_ref: String::from("campaign.probe_gepa.stage1.retained_failure_sprint.2026_06_08"),
        split_manifest_ref: manifest.manifest_ref.clone(),
        benchmark_suite_ref: manifest.benchmark_suite_ref.clone(),
        dataset: manifest.dataset.clone(),
        retained_fixture_refs: retained_tasks
            .iter()
            .map(|task| task.task_ref.clone())
            .collect(),
        worker_assignment_refs,
        candidates,
        metric_calls,
        candidate_summaries,
        baseline_candidate_ref: String::from("candidate.probe_gepa.stage1.baseline"),
        champion_candidate_ref: String::from("candidate.probe_gepa.stage1.champion"),
        selected_candidate_ref,
        selected_candidate_decision: GepaCandidateDecisionState::OptimizerAccepted,
        candidate_improves_or_preserves_retained_fixtures: true,
        policy_gate_failure: false,
        public_summary_ref: String::from("public_summary.probe_gepa.stage1.retained_only.v1"),
        public_summary_label: String::from("retained evidence only"),
        public_summary_evidence_scope: String::from("retained"),
        no_lora: true,
        no_model_training: true,
        no_public_leaderboard_claim: true,
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };

    validate_probe_gepa_stage1_retained_sprint(&campaign, manifest)?;
    Ok(campaign)
}

pub fn validate_probe_gepa_stage1_retained_sprint(
    campaign: &ProbeGepaStage1RetainedSprint,
    manifest: &BenchmarkCampaignSplitManifest,
) -> Result<(), BenchmarkContractError> {
    validate_campaign_split_manifest(manifest)?;

    if campaign.schema_ref != PROBE_GEPA_STAGE1_RETAINED_SPRINT_SCHEMA_REF {
        return stage1_sprint_error(&campaign.campaign_ref, "unsupported campaign schema ref");
    }

    if campaign.campaign_id.is_empty() || campaign.campaign_ref.is_empty() {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 sprint campaign must include id and ref",
        );
    }

    if campaign.split_manifest_ref != manifest.manifest_ref {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "campaign split manifest ref must match validated manifest",
        );
    }

    if campaign.worker_assignment_refs.len() < 8 || campaign.worker_assignment_refs.len() > 16 {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 retained sprint must use eight to sixteen worker assignment refs",
        );
    }

    if campaign.metric_calls.len() < 200 || campaign.metric_calls.len() > 400 {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 retained sprint must contain two hundred to four hundred metric calls",
        );
    }

    if !campaign.no_lora
        || !campaign.no_model_training
        || !campaign.no_public_leaderboard_claim
        || campaign.public_summary_evidence_scope != "retained"
        || campaign.public_summary_label != "retained evidence only"
    {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 sprint can publish retained evidence only and cannot claim LoRA, model training, or leaderboard standing",
        );
    }

    if campaign.selected_candidate_decision == GepaCandidateDecisionState::OptimizerAccepted
        && (!campaign.candidate_improves_or_preserves_retained_fixtures
            || campaign.policy_gate_failure)
    {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "optimizer accepted candidate must improve or preserve retained fixtures without policy-gate failure",
        );
    }

    if campaign.selected_candidate_ref == "active" {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 selected candidate cannot enter active state",
        );
    }

    let retained_refs = campaign
        .retained_fixture_refs
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let candidate_hashes = campaign
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_hash.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    if campaign.candidate_summaries.len() != campaign.candidates.len() {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "candidate summaries must cover every sprint candidate",
        );
    }

    let mut metric_refs = std::collections::BTreeSet::new();
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for metric_call in &campaign.metric_calls {
        if !metric_refs.insert(&metric_call.metric_call_ref) {
            return stage1_sprint_error(&campaign.campaign_ref, "metric call refs must be unique");
        }

        if !retained_refs.contains(&metric_call.task_ref) {
            return stage1_sprint_error(
                &campaign.campaign_ref,
                "Stage 1 sprint metric calls must use retained fixtures only",
            );
        }

        if !candidate_hashes.contains(metric_call.candidate_hash.as_str()) {
            return stage1_sprint_error(
                &campaign.campaign_ref,
                "metric call candidate hash must reference campaign candidate",
            );
        }

        if metric_call.verifier_ref.is_empty()
            || metric_call.artifact_manifest_ref.is_empty()
            || metric_call.benchmark_cloud_proof_bundle_ref.is_empty()
            || metric_call.resource_usage_receipt_ref.is_none()
            || metric_call.payment_mode.is_empty()
            || metric_call.pylon_assignment_ref.is_none()
        {
            return stage1_sprint_error(
                &campaign.campaign_ref,
                "each rollout must include candidate hash, task ref, verifier ref, artifact/proof ref, resource ref, Pylon assignment ref, and explicit payment mode",
            );
        }

        match metric_call.closeout_state {
            BenchmarkCloseoutState::Accepted => accepted += 1,
            BenchmarkCloseoutState::Rejected => rejected += 1,
        }
    }

    if accepted == 0 || rejected == 0 {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 sprint must include accepted and rejected Pylon closeouts",
        );
    }

    if serde_json::to_value(campaign)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
    {
        return stage1_sprint_error(
            &campaign.campaign_ref,
            "Stage 1 sprint campaign contains unsafe material",
        );
    }

    Ok(())
}

pub fn build_probe_gepa_validation_sweep(
    manifest: &BenchmarkCampaignSplitManifest,
    probe_commit: impl Into<String>,
    gepa_candidate_hash: impl Into<String>,
) -> Result<ProbeGepaValidationSweep, BenchmarkContractError> {
    validate_campaign_split_manifest(manifest)?;

    let probe_commit = probe_commit.into();
    let gepa_candidate_hash = gepa_candidate_hash.into();
    let validation_tasks = manifest
        .tasks
        .iter()
        .filter(|task| {
            task.lane == BenchmarkSplitLane::Validation
                && task.evidence_split == BenchmarkEvidenceSplit::Validation
        })
        .collect::<Vec<_>>();
    let routes = probe_gepa_validation_routes(&gepa_candidate_hash);
    let mut rollout_records = Vec::new();

    for (route_index, route) in routes.iter().enumerate() {
        for (task_index, task) in validation_tasks.iter().enumerate() {
            let candidate_hash = route
                .candidate_hash
                .clone()
                .unwrap_or_else(|| format!("sha256:{:064x}", 0x3000 + route_index));
            let assignment = probe_assignment_from_split_task(
                manifest,
                task,
                probe_commit.clone(),
                candidate_hash.clone(),
                vec![String::from(
                    "program_signature.probe.validation.terminal_bench.v1",
                )],
                "tool_menu.probe.terminal_bench.validation.v1",
            );
            let outcome = validation_sweep_outcome(route_index, task_index);
            let artifacts =
                run_fake_probe_benchmark_task(manifest, task, &assignment, outcome.clone())?;
            let rollout_index = rollout_records.len() + 1;
            let closeout_state = if artifacts.result_json.status == BenchmarkRunStatus::Succeeded {
                BenchmarkCloseoutState::Accepted
            } else {
                BenchmarkCloseoutState::Rejected
            };

            rollout_records.push(ProbeGepaMetricCallRecord {
                metric_call_ref: format!("metric_call.probe_gepa.validation.{:03}", rollout_index),
                candidate_ref: route
                    .candidate_ref
                    .clone()
                    .unwrap_or_else(|| route.route_ref.clone()),
                candidate_hash,
                task_ref: task.task_ref.clone(),
                task_id: task.task_id.clone(),
                verifier_ref: task.scorer_verifier.verifier_ref.clone(),
                probe_assignment_ref: assignment.assignment_ref.clone(),
                pylon_assignment_ref: Some(format!(
                    "shc_assignment_ref.public.validation.{rollout_index:03}"
                )),
                payment_mode: String::from("unpaid_smoke"),
                probe_closeout_ref: artifacts
                    .result_json
                    .probe_closeout_import
                    .as_ref()
                    .map(|import| import.probe_closeout_ref.clone())
                    .unwrap_or_default(),
                probe_closeout_bundle_ref: format!(
                    "probe_closeout_bundle.probe_gepa.validation.{:03}",
                    rollout_index
                ),
                benchmark_result_ref: format!(
                    "{}.validation.{:03}",
                    artifacts.result_json.result_ref, rollout_index
                ),
                artifact_manifest_ref: format!(
                    "{}.validation.{:03}",
                    artifacts.artifact_manifest_json.manifest_ref, rollout_index
                ),
                benchmark_cloud_proof_bundle_ref: format!(
                    "{}.validation.{:03}",
                    artifacts.proof_bundle_json.proof_bundle_ref, rollout_index
                ),
                resource_usage_receipt_ref: Some(format!(
                    "{}.validation.{:03}",
                    artifacts.resource_usage_receipt_json.receipt_ref, rollout_index
                )),
                verifier_import_ref: format!(
                    "verifier_import.probe_gepa.validation.{:03}",
                    rollout_index
                ),
                verifier_result_ref: format!(
                    "verifier_result.probe_gepa.validation.{:03}",
                    rollout_index
                ),
                cost_ref: Some(
                    artifacts
                        .resource_usage_receipt_json
                        .cost_ref
                        .clone()
                        .unwrap_or_else(|| String::from("cost.probe.validation.zero")),
                ),
                duration_ms: Some(
                    artifacts
                        .resource_usage_receipt_json
                        .duration_ms
                        .unwrap_or(1_500),
                ),
                artifact_available: true,
                failure_classification_ref: artifacts.result_json.failure_classification_ref,
                closeout_state,
                score_bps: artifacts.result_json.score_bps,
                status: artifacts.result_json.status,
            });
        }
    }

    let sweep = ProbeGepaValidationSweep {
        schema_ref: String::from(PROBE_GEPA_VALIDATION_SWEEP_SCHEMA_REF),
        campaign_id: String::from("probe-gepa-validation-sweep-2026-06-08"),
        sweep_ref: String::from("sweep.probe_gepa.validation.shc_terminal_bench.2026_06_08"),
        split_manifest_ref: manifest.manifest_ref.clone(),
        benchmark_suite_ref: manifest.benchmark_suite_ref.clone(),
        dataset: manifest.dataset.clone(),
        validation_task_refs: validation_tasks
            .iter()
            .map(|task| task.task_ref.clone())
            .collect(),
        probe_commit,
        gepa_candidate_hash,
        routes,
        rollout_records,
        candidate_shadow_state_ref: String::from(
            "shadow_state.probe_gepa.candidate.allowed_by_omega_blueprint_gates.v1",
        ),
        candidate_may_move_to_shadow: true,
        omega_blueprint_gate_refs: vec![
            String::from("omega_gate.probe_gepa.validation_shadow_approval.v1"),
            String::from("blueprint_gate.probe_gepa.validation_shadow_approval.v1"),
        ],
        public_claim_ref: String::from("public_claim.probe_gepa.validation_measured_only.v1"),
        public_claim_label: String::from("validation measured only"),
        no_public_beats_terminal_bench_claim: true,
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };

    validate_probe_gepa_validation_sweep(&sweep, manifest)?;
    Ok(sweep)
}

pub fn validate_probe_gepa_validation_sweep(
    sweep: &ProbeGepaValidationSweep,
    manifest: &BenchmarkCampaignSplitManifest,
) -> Result<(), BenchmarkContractError> {
    validate_campaign_split_manifest(manifest)?;

    if sweep.schema_ref != PROBE_GEPA_VALIDATION_SWEEP_SCHEMA_REF {
        return validation_sweep_error(&sweep.sweep_ref, "unsupported validation sweep schema ref");
    }

    if sweep.campaign_id.is_empty()
        || sweep.sweep_ref.is_empty()
        || sweep.probe_commit.is_empty()
        || sweep.gepa_candidate_hash.is_empty()
    {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "validation sweep must include campaign id, sweep ref, Probe commit, and candidate hash",
        );
    }

    if sweep.split_manifest_ref != manifest.manifest_ref {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "validation sweep split manifest ref must match validated manifest",
        );
    }

    let validation_refs = sweep
        .validation_task_refs
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if validation_refs.is_empty() {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "validation sweep must include validation task refs",
        );
    }

    for task_ref in &sweep.validation_task_refs {
        let Some(task) = manifest
            .tasks
            .iter()
            .find(|task| &task.task_ref == task_ref)
        else {
            return validation_sweep_error(
                &sweep.sweep_ref,
                "validation task ref must be present in split manifest",
            );
        };
        if task.evidence_split != BenchmarkEvidenceSplit::Validation
            || task.lane != BenchmarkSplitLane::Validation
        {
            return validation_sweep_error(
                &sweep.sweep_ref,
                "validation sweep cannot use retained or holdout tasks",
            );
        }
    }

    if sweep.routes.len() != 3
        || !sweep
            .routes
            .iter()
            .any(|route| route.route_kind == ProbeValidationRouteKind::CurrentChampion)
        || !sweep
            .routes
            .iter()
            .any(|route| route.route_kind == ProbeValidationRouteKind::GepaCandidate)
        || !sweep
            .routes
            .iter()
            .any(|route| route.route_kind == ProbeValidationRouteKind::BaselineBackendRoute)
    {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "validation sweep must compare champion, GEPA candidate, and baseline backend routes",
        );
    }

    if !sweep.no_public_beats_terminal_bench_claim
        || sweep.public_claim_label != "validation measured only"
    {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "validation sweep cannot claim Probe beats Terminal-Bench",
        );
    }

    if sweep.candidate_may_move_to_shadow
        && !(sweep
            .omega_blueprint_gate_refs
            .iter()
            .any(|gate| gate.starts_with("omega_gate."))
            && sweep
                .omega_blueprint_gate_refs
                .iter()
                .any(|gate| gate.starts_with("blueprint_gate.")))
    {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "shadow movement requires Omega and Blueprint gate refs",
        );
    }

    let mut rollout_refs = std::collections::BTreeSet::new();
    for rollout in &sweep.rollout_records {
        if !rollout_refs.insert(&rollout.metric_call_ref) {
            return validation_sweep_error(&sweep.sweep_ref, "rollout refs must be unique");
        }

        if !validation_refs.contains(&rollout.task_ref) {
            return validation_sweep_error(
                &sweep.sweep_ref,
                "all rollout records must use validation split tasks",
            );
        }

        if rollout.probe_closeout_bundle_ref.is_empty()
            || rollout.candidate_hash.is_empty()
            || rollout.verifier_ref.is_empty()
            || rollout.verifier_result_ref.is_empty()
            || rollout.artifact_manifest_ref.is_empty()
            || rollout.benchmark_cloud_proof_bundle_ref.is_empty()
            || rollout.resource_usage_receipt_ref.is_none()
            || rollout.cost_ref.is_none()
            || rollout.duration_ms.is_none()
            || !rollout.artifact_available
        {
            return validation_sweep_error(
                &sweep.sweep_ref,
                "validation rollouts must preserve closeout, candidate, verifier, artifact, proof, resource, cost, duration, and artifact availability records",
            );
        }
    }

    if serde_json::to_value(sweep)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
    {
        return validation_sweep_error(
            &sweep.sweep_ref,
            "validation sweep contains unsafe material",
        );
    }

    Ok(())
}

fn probe_gepa_stage0_smoke_candidates() -> Vec<GepaTextBundleCandidate> {
    vec![
        GepaTextBundleCandidate {
            candidate_ref: String::from("candidate.probe_gepa.stage0.baseline"),
            candidate_hash: String::from(
                "sha256:1000000000000000000000000000000000000000000000000000000000000001",
            ),
            parent_candidate_ref: None,
            candidate_kind: GepaCandidateKind::Baseline,
            prompt_bundle_ref: String::from("prompt_bundle.probe_gepa.stage0.baseline.v1"),
            blueprint_bundle_ref: String::from("blueprint_bundle.probe_gepa.stage0.baseline.v1"),
        },
        GepaTextBundleCandidate {
            candidate_ref: String::from("candidate.probe_gepa.stage0.mutation_1"),
            candidate_hash: String::from(
                "sha256:1000000000000000000000000000000000000000000000000000000000000002",
            ),
            parent_candidate_ref: Some(String::from("candidate.probe_gepa.stage0.baseline")),
            candidate_kind: GepaCandidateKind::MutatedTextBundle,
            prompt_bundle_ref: String::from("prompt_bundle.probe_gepa.stage0.mutation_1.v1"),
            blueprint_bundle_ref: String::from("blueprint_bundle.probe_gepa.stage0.mutation_1.v1"),
        },
        GepaTextBundleCandidate {
            candidate_ref: String::from("candidate.probe_gepa.stage0.mutation_2"),
            candidate_hash: String::from(
                "sha256:1000000000000000000000000000000000000000000000000000000000000003",
            ),
            parent_candidate_ref: Some(String::from("candidate.probe_gepa.stage0.baseline")),
            candidate_kind: GepaCandidateKind::MutatedTextBundle,
            prompt_bundle_ref: String::from("prompt_bundle.probe_gepa.stage0.mutation_2.v1"),
            blueprint_bundle_ref: String::from("blueprint_bundle.probe_gepa.stage0.mutation_2.v1"),
        },
        GepaTextBundleCandidate {
            candidate_ref: String::from("candidate.probe_gepa.stage0.mutation_3"),
            candidate_hash: String::from(
                "sha256:1000000000000000000000000000000000000000000000000000000000000004",
            ),
            parent_candidate_ref: Some(String::from("candidate.probe_gepa.stage0.baseline")),
            candidate_kind: GepaCandidateKind::MutatedTextBundle,
            prompt_bundle_ref: String::from("prompt_bundle.probe_gepa.stage0.mutation_3.v1"),
            blueprint_bundle_ref: String::from("blueprint_bundle.probe_gepa.stage0.mutation_3.v1"),
        },
    ]
}

fn probe_gepa_stage1_sprint_candidates() -> Vec<GepaTextBundleCandidate> {
    let mut candidates = Vec::new();
    for index in 0..10 {
        let (candidate_ref, parent_candidate_ref, candidate_kind) = match index {
            0 => (
                String::from("candidate.probe_gepa.stage1.baseline"),
                None,
                GepaCandidateKind::Baseline,
            ),
            1 => (
                String::from("candidate.probe_gepa.stage1.champion"),
                Some(String::from("candidate.probe_gepa.stage1.baseline")),
                GepaCandidateKind::MutatedTextBundle,
            ),
            _ => (
                format!("candidate.probe_gepa.stage1.mutation_{index:02}"),
                Some(String::from("candidate.probe_gepa.stage1.champion")),
                GepaCandidateKind::MutatedTextBundle,
            ),
        };

        candidates.push(GepaTextBundleCandidate {
            candidate_ref,
            candidate_hash: format!("sha256:{:064x}", 0x2000 + index),
            parent_candidate_ref,
            candidate_kind,
            prompt_bundle_ref: format!("prompt_bundle.probe_gepa.stage1.candidate_{index:02}.v1"),
            blueprint_bundle_ref: format!(
                "blueprint_bundle.probe_gepa.stage1.candidate_{index:02}.v1"
            ),
        });
    }

    candidates
}

fn probe_gepa_validation_routes(gepa_candidate_hash: &str) -> Vec<ProbeValidationRoute> {
    vec![
        ProbeValidationRoute {
            route_ref: String::from("route.probe.validation.current_champion"),
            route_kind: ProbeValidationRouteKind::CurrentChampion,
            candidate_ref: Some(String::from("candidate.probe_gepa.stage1.champion")),
            candidate_hash: Some(String::from(
                "sha256:0000000000000000000000000000000000000000000000000000000000002001",
            )),
            backend_route_ref: String::from("backend_route.probe.current_champion.v1"),
        },
        ProbeValidationRoute {
            route_ref: String::from("route.probe.validation.gepa_candidate"),
            route_kind: ProbeValidationRouteKind::GepaCandidate,
            candidate_ref: Some(String::from("candidate.probe_gepa.stage1.mutation_08")),
            candidate_hash: Some(gepa_candidate_hash.to_string()),
            backend_route_ref: String::from("backend_route.probe.gepa_candidate.v1"),
        },
        ProbeValidationRoute {
            route_ref: String::from("route.probe.validation.baseline_backend"),
            route_kind: ProbeValidationRouteKind::BaselineBackendRoute,
            candidate_ref: None,
            candidate_hash: None,
            backend_route_ref: String::from("backend_route.probe.baseline_backend.v1"),
        },
    ]
}

fn retained_summaries_for_candidates(
    candidates: &[GepaTextBundleCandidate],
    metric_calls: &[ProbeGepaMetricCallRecord],
) -> Vec<ProbeGepaCandidateRetainedSummary> {
    candidates
        .iter()
        .map(|candidate| {
            let calls = metric_calls
                .iter()
                .filter(|call| call.candidate_ref == candidate.candidate_ref)
                .collect::<Vec<_>>();
            let accepted_count = calls
                .iter()
                .filter(|call| call.closeout_state == BenchmarkCloseoutState::Accepted)
                .count();
            let rejected_count = calls.len().saturating_sub(accepted_count);
            let score_sum = calls
                .iter()
                .map(|call| call.score_bps.unwrap_or(0) as usize)
                .sum::<usize>();
            let failure_classification_refs = calls
                .iter()
                .filter_map(|call| call.failure_classification_ref.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            ProbeGepaCandidateRetainedSummary {
                candidate_ref: candidate.candidate_ref.clone(),
                candidate_hash: candidate.candidate_hash.clone(),
                metric_call_count: calls.len(),
                accepted_count,
                rejected_count,
                mean_score_bps: if calls.is_empty() {
                    0
                } else {
                    (score_sum / calls.len()) as u32
                },
                regression_count: rejected_count.saturating_sub(9),
                failure_classification_refs,
            }
        })
        .collect()
}

fn stage0_smoke_outcome(candidate_index: usize, task_index: usize) -> ProbeFakeRunnerOutcome {
    match (candidate_index + task_index) % 5 {
        0 | 1 | 3 => ProbeFakeRunnerOutcome::Pass,
        2 => ProbeFakeRunnerOutcome::Error,
        _ => ProbeFakeRunnerOutcome::Timeout,
    }
}

fn stage1_sprint_outcome(
    candidate_index: usize,
    task_index: usize,
    repeat_index: usize,
) -> ProbeFakeRunnerOutcome {
    if candidate_index >= 8 {
        if (task_index + repeat_index) % 11 == 0 {
            ProbeFakeRunnerOutcome::Error
        } else {
            ProbeFakeRunnerOutcome::Pass
        }
    } else if candidate_index >= 4 {
        match (task_index + repeat_index + candidate_index) % 6 {
            0 => ProbeFakeRunnerOutcome::Timeout,
            1 => ProbeFakeRunnerOutcome::Error,
            _ => ProbeFakeRunnerOutcome::Pass,
        }
    } else {
        match (task_index + repeat_index + candidate_index) % 4 {
            0 => ProbeFakeRunnerOutcome::Pass,
            1 => ProbeFakeRunnerOutcome::Error,
            _ => ProbeFakeRunnerOutcome::Timeout,
        }
    }
}

fn validation_sweep_outcome(route_index: usize, task_index: usize) -> ProbeFakeRunnerOutcome {
    match route_index {
        0 => {
            if task_index % 3 == 0 {
                ProbeFakeRunnerOutcome::Timeout
            } else {
                ProbeFakeRunnerOutcome::Pass
            }
        }
        1 => {
            if task_index == 5 {
                ProbeFakeRunnerOutcome::Error
            } else {
                ProbeFakeRunnerOutcome::Pass
            }
        }
        _ => {
            if task_index % 2 == 0 {
                ProbeFakeRunnerOutcome::Pass
            } else {
                ProbeFakeRunnerOutcome::Error
            }
        }
    }
}

fn stage0_smoke_error<T>(
    campaign_ref: impl Into<String>,
    reason: impl Into<String>,
) -> Result<T, BenchmarkContractError> {
    Err(BenchmarkContractError::Stage0SmokeInvalid {
        campaign_ref: campaign_ref.into(),
        reason: reason.into(),
    })
}

fn stage1_sprint_error<T>(
    campaign_ref: impl Into<String>,
    reason: impl Into<String>,
) -> Result<T, BenchmarkContractError> {
    Err(BenchmarkContractError::Stage1SprintInvalid {
        campaign_ref: campaign_ref.into(),
        reason: reason.into(),
    })
}

fn validation_sweep_error<T>(
    sweep_ref: impl Into<String>,
    reason: impl Into<String>,
) -> Result<T, BenchmarkContractError> {
    Err(BenchmarkContractError::ValidationSweepInvalid {
        sweep_ref: sweep_ref.into(),
        reason: reason.into(),
    })
}

fn pylon_benchmark_worker_blockers(
    capability: &PylonBenchmarkWorkerCapabilityEnvelope,
    requirement: &PylonBenchmarkWorkRequirement,
) -> Vec<String> {
    let mut blockers = Vec::new();

    if !capability.benchmark_runner_support {
        blockers.push(String::from(
            "blocker.pylon.capability.benchmark_runner_missing",
        ));
    }

    if requirement.requires_harbor_terminal_bench && !capability.harbor_terminal_bench_support {
        blockers.push(String::from(
            "blocker.pylon.capability.harbor_terminal_bench_missing",
        ));
    }

    if requirement.requires_probe_runtime && !capability.probe_runtime_support {
        blockers.push(String::from(
            "blocker.pylon.capability.probe_runtime_missing",
        ));
    }

    if requirement.requires_local_model && !capability.local_model_support {
        blockers.push(String::from("blocker.pylon.capability.local_model_missing"));
    }

    if requirement.requires_apple_fm && !capability.apple_fm_support {
        blockers.push(String::from("blocker.pylon.capability.apple_fm_missing"));
    }

    if requirement.requires_qwen_adapter && !capability.qwen_adapter_support {
        blockers.push(String::from(
            "blocker.pylon.capability.qwen_adapter_missing",
        ));
    }

    if requirement.requires_mlx_training && !capability.mlx_training_support {
        blockers.push(String::from(
            "blocker.pylon.capability.mlx_training_missing",
        ));
    }

    if requirement.work_kind.requires_model_training() && !capability.model_training_support {
        blockers.push(String::from(
            "blocker.pylon.capability.model_training_missing",
        ));
    }

    if !requirement.work_kind.requires_model_training() && !capability.rollout_eval_support {
        blockers.push(String::from(
            "blocker.pylon.capability.rollout_eval_missing",
        ));
    }

    if capability.hardware.cpu_core_count < requirement.min_cpu_core_count {
        blockers.push(String::from(
            "blocker.pylon.capacity.cpu_cores_insufficient",
        ));
    }

    if capability.hardware.ram_bytes < requirement.min_ram_bytes {
        blockers.push(String::from("blocker.pylon.capacity.ram_insufficient"));
    }

    if capability.hardware.disk_available_bytes < requirement.min_disk_available_bytes {
        blockers.push(String::from("blocker.pylon.capacity.disk_insufficient"));
    }

    if let Some(required_gpu_memory) = requirement.min_gpu_memory_bytes {
        if capability.hardware.gpu_memory_bytes.unwrap_or(0) < required_gpu_memory {
            blockers.push(String::from(
                "blocker.pylon.capacity.gpu_memory_insufficient",
            ));
        }
    }

    if capability.max_wall_clock_ms < requirement.max_wall_clock_ms {
        blockers.push(String::from(
            "blocker.pylon.capacity.wall_clock_budget_insufficient",
        ));
    }

    if requirement.requires_artifact_upload && !capability.artifact_upload_support {
        blockers.push(String::from(
            "blocker.pylon.capability.artifact_upload_missing",
        ));
    }

    if requirement.requires_proof_receipts && !capability.proof_receipt_support {
        blockers.push(String::from(
            "blocker.pylon.capability.proof_receipts_missing",
        ));
    }

    if requirement.requires_assignment_lease && !capability.assignment_lease_support {
        blockers.push(String::from(
            "blocker.pylon.capability.assignment_lease_missing",
        ));
    }

    if requirement.requires_closeout && !capability.closeout_support {
        blockers.push(String::from("blocker.pylon.capability.closeout_missing"));
    }

    blockers.sort();
    blockers.dedup();
    blockers
}

fn worker_capability_error(
    capability: &PylonBenchmarkWorkerCapabilityEnvelope,
    reason: impl Into<String>,
) -> Result<(), BenchmarkContractError> {
    Err(BenchmarkContractError::WorkerCapabilityInvalid {
        capability_ref: capability.capability_ref.clone(),
        reason: reason.into(),
    })
}

pub fn run_fake_probe_benchmark_task(
    manifest: &BenchmarkCampaignSplitManifest,
    task: &BenchmarkSplitTaskEntry,
    assignment: &ProbeBenchmarkAssignment,
    outcome: ProbeFakeRunnerOutcome,
) -> Result<ProbeBenchmarkRunnerArtifactSet, BenchmarkContractError> {
    validate_probe_assignment_public_refs(assignment)?;
    let probe_command = build_probe_command_invocation(assignment)?;
    let status = status_for_fake_outcome(&outcome);
    let run_ref = format!("benchmark_run.probe.{}", task.task_id);
    let artifact_manifest_ref = format!("artifact_manifest.probe.{}", task.task_id);
    let proof_bundle_ref = format!("proof_bundle.probe.{}", task.task_id);
    let resource_usage_receipt_ref = format!("resource_usage.probe.{}", task.task_id);
    let probe_closeout_ref = format!("probe_closeout.{}", run_ref);
    let failure_classification_ref = status.is_terminal_failure().then(|| {
        format!(
            "failure_classification.probe.{}.{}",
            task.task_id,
            failure_family_for_fake_outcome(&outcome)
        )
    });
    let resource_usage_ref =
        matches!(outcome, ProbeFakeRunnerOutcome::Pass).then(|| resource_usage_receipt_ref.clone());
    let resource_unavailable_reason =
        (!matches!(outcome, ProbeFakeRunnerOutcome::Pass)).then(|| {
            format!(
                "{}_before_resource_meter_flush",
                failure_family_for_fake_outcome(&outcome)
            )
        });
    let events = events_for_fake_outcome(&run_ref, &assignment.task_run_ref, &outcome);
    let artifact_manifest = BenchmarkArtifactManifest {
        schema_ref: String::from(BENCHMARK_ARTIFACT_MANIFEST_SCHEMA_REF),
        manifest_ref: artifact_manifest_ref.clone(),
        run_ref: run_ref.clone(),
        artifacts: PROBE_RUNNER_REQUIRED_ARTIFACT_FILES
            .iter()
            .map(|file_name| BenchmarkArtifactRef {
                artifact_ref: format!(
                    "artifact.probe.{}.{}",
                    task.task_id,
                    file_name.replace('.', "_")
                ),
                digest: format!("sha256:{}", "0".repeat(64)),
                media_type: media_type_for_file(file_name).to_string(),
                public_url_ref: None,
                size_bytes: 1,
            })
            .collect(),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };
    let resource_usage_receipt = ResourceUsageReceipt {
        schema_ref: String::from(RESOURCE_USAGE_RECEIPT_SCHEMA_REF),
        receipt_ref: resource_usage_receipt_ref.clone(),
        run_ref: run_ref.clone(),
        worker_ref: String::from("worker.shc.probe.fake"),
        duration_ms: matches!(outcome, ProbeFakeRunnerOutcome::Pass).then_some(1000),
        cpu_ms: matches!(outcome, ProbeFakeRunnerOutcome::Pass).then_some(500),
        gpu_ms: None,
        memory_peak_bytes: Some(1024),
        token_usage: None,
        cost_ref: Some(String::from("cost.probe.fake.zero")),
        unavailable_reason: resource_unavailable_reason.clone(),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };
    let proof_bundle = BenchmarkProofBundle {
        schema_ref: String::from(BENCHMARK_PROOF_BUNDLE_SCHEMA_REF),
        proof_bundle_ref: proof_bundle_ref.clone(),
        run_ref: run_ref.clone(),
        artifact_manifest_refs: vec![artifact_manifest_ref.clone()],
        scorer_verifier: task.scorer_verifier.clone(),
        resource_usage_receipt_ref: Some(resource_usage_receipt_ref.clone()),
        probe_selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
        probe_tool_menu_ref: Some(assignment.tool_menu_ref.clone()),
        redaction_state: BenchmarkRedactionState::PublicSafe,
    };
    let result = BenchmarkResult {
        schema_ref: String::from(BENCHMARK_RESULT_SCHEMA_REF),
        result_ref: format!("benchmark_result.probe.{}", task.task_id),
        run_ref: run_ref.clone(),
        task_run_ref: assignment.task_run_ref.clone(),
        dataset: assignment.dataset.clone(),
        evidence_split: assignment.evidence_split.clone(),
        status,
        scorer_verifier: task.scorer_verifier.clone(),
        score_bps: matches!(outcome, ProbeFakeRunnerOutcome::Pass).then_some(10_000),
        failure_classification_ref,
        artifact_manifest_refs: vec![artifact_manifest_ref],
        proof_bundle_refs: vec![proof_bundle_ref],
        resource_usage_receipt_ref: resource_usage_ref,
        resource_unavailable_reason,
        probe_closeout_import: Some(ProbeCloseoutImport {
            probe_assignment_ref: assignment.assignment_ref.clone(),
            probe_closeout_ref: probe_closeout_ref.clone(),
            probe_commit: assignment.probe_commit.clone(),
            probe_run_record_ref: String::from("probe-run-record.json"),
            probe_run_ref: run_ref.clone(),
            candidate_hash: assignment.candidate_hash.clone(),
            selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
            tool_menu_ref: assignment.tool_menu_ref.clone(),
        }),
        no_cheat: manifest.no_cheat.clone(),
        redaction_state: BenchmarkRedactionState::PublicSafe,
        public_claim_boundary: BenchmarkPublicClaimBoundary {
            claim_level: BenchmarkPublicClaimLevel::None,
            external_release_gate_refs: Vec::new(),
            public_claim_upgrade_authority: false,
        },
    };

    validate_benchmark_result(&result)?;

    let artifact_set = ProbeBenchmarkRunnerArtifactSet {
        result_json: result,
        events_jsonl: events,
        metadata_json: ProbeBenchmarkRunMetadata {
            runner_ref: String::from("runner.openagents.harbor.shc.probe.fake"),
            benchmark_suite_ref: manifest.benchmark_suite_ref.clone(),
            dataset: manifest.dataset.clone(),
            task_ref: task.task_ref.clone(),
            probe_command,
            selected_signature_refs: assignment.selected_blueprint_signature_refs.clone(),
            tool_menu_ref: assignment.tool_menu_ref.clone(),
            safe_account_grant_refs: true,
            redaction_state: BenchmarkRedactionState::PublicSafe,
        },
        artifact_manifest_json: artifact_manifest,
        proof_bundle_json: proof_bundle,
        resource_usage_receipt_json: resource_usage_receipt,
        probe_run_record_json: json!({
            "schema_ref": "probe.benchmark_run.v1",
            "run_ref": run_ref,
            "assignment_ref": assignment.assignment_ref,
            "candidate_hash": assignment.candidate_hash,
            "evidence_split": assignment.evidence_split,
            "status": assignment_status_for_probe_record(&outcome),
        }),
        probe_closeout_json: json!({
            "schema_ref": "probe.benchmark_closeout.v1",
            "closeout_ref": probe_closeout_ref,
            "assignment_ref": assignment.assignment_ref,
            "candidate_hash": assignment.candidate_hash,
            "selected_signature_refs": assignment.selected_blueprint_signature_refs,
            "tool_menu_ref": assignment.tool_menu_ref,
            "artifact_manifest_refs": assignment.required_artifact_refs,
            "proof_bundle_refs": assignment.required_proof_bundle_refs,
            "redaction_state": "public_safe",
        }),
    };

    if artifact_set_contains_unsafe_material(&artifact_set) {
        return Err(BenchmarkContractError::ProbeRunnerInvalid {
            run_ref: artifact_set.result_json.run_ref,
            reason: String::from("Probe runner artifact set contains unsafe material"),
        });
    }

    Ok(artifact_set)
}

pub fn artifact_set_contains_unsafe_material(
    artifact_set: &ProbeBenchmarkRunnerArtifactSet,
) -> bool {
    serde_json::to_value(artifact_set)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
}

pub fn worker_capability_contains_unsafe_material(
    capability: &PylonBenchmarkWorkerCapabilityEnvelope,
) -> bool {
    serde_json::to_value(capability)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
}

pub fn worker_requirement_contains_unsafe_material(
    requirement: &PylonBenchmarkWorkRequirement,
) -> bool {
    serde_json::to_value(requirement)
        .map(|value| json_contains_unsafe_material(&value))
        .unwrap_or(true)
}

fn validate_probe_assignment_public_refs(
    assignment: &ProbeBenchmarkAssignment,
) -> Result<(), BenchmarkContractError> {
    let fields = [
        assignment.provider_account_ref.as_deref(),
        assignment.auth_grant_ref.as_deref(),
        Some(assignment.task_ref.as_str()),
        Some(assignment.public_task_checksum.as_str()),
        Some(assignment.tool_menu_ref.as_str()),
        Some(assignment.candidate_hash.as_str()),
    ];

    for value in fields.into_iter().flatten() {
        if string_contains_unsafe_material(value) {
            return Err(BenchmarkContractError::ProbeRunnerInvalid {
                run_ref: assignment.task_run_ref.clone(),
                reason: String::from(
                    "Probe assignment contains raw credential or private trace material",
                ),
            });
        }
    }

    if assignment.selected_blueprint_signature_refs.is_empty() {
        return Err(BenchmarkContractError::ProbeRunnerInvalid {
            run_ref: assignment.task_run_ref.clone(),
            reason: String::from("Probe assignment must include selected Blueprint signature refs"),
        });
    }

    Ok(())
}

fn events_for_fake_outcome(
    run_ref: &str,
    task_run_ref: &str,
    outcome: &ProbeFakeRunnerOutcome,
) -> Vec<BenchmarkEvent> {
    let terminal_kind = match outcome {
        ProbeFakeRunnerOutcome::Pass => BenchmarkEventKind::Completed,
        ProbeFakeRunnerOutcome::Timeout => BenchmarkEventKind::TimedOut,
        ProbeFakeRunnerOutcome::Error => BenchmarkEventKind::Failed,
    };

    vec![
        BenchmarkEvent {
            schema_ref: String::from(BENCHMARK_EVENT_SCHEMA_REF),
            event_ref: format!("event.{run_ref}.started"),
            run_ref: run_ref.to_string(),
            task_run_ref: task_run_ref.to_string(),
            event_kind: BenchmarkEventKind::Started,
            observed_at: String::from("2026-06-08T00:00:00.000Z"),
            artifact_refs: Vec::new(),
            proof_bundle_refs: Vec::new(),
            redaction_state: BenchmarkRedactionState::PublicSafe,
        },
        BenchmarkEvent {
            schema_ref: String::from(BENCHMARK_EVENT_SCHEMA_REF),
            event_ref: format!("event.{run_ref}.terminal"),
            run_ref: run_ref.to_string(),
            task_run_ref: task_run_ref.to_string(),
            event_kind: terminal_kind,
            observed_at: String::from("2026-06-08T00:00:01.000Z"),
            artifact_refs: vec![format!("artifact_manifest.probe.{task_run_ref}")],
            proof_bundle_refs: vec![format!("proof_bundle.probe.{task_run_ref}")],
            redaction_state: BenchmarkRedactionState::PublicSafe,
        },
    ]
}

fn status_for_fake_outcome(outcome: &ProbeFakeRunnerOutcome) -> BenchmarkRunStatus {
    match outcome {
        ProbeFakeRunnerOutcome::Pass => BenchmarkRunStatus::Succeeded,
        ProbeFakeRunnerOutcome::Timeout => BenchmarkRunStatus::TimedOut,
        ProbeFakeRunnerOutcome::Error => BenchmarkRunStatus::Errored,
    }
}

fn assignment_status_for_probe_record(outcome: &ProbeFakeRunnerOutcome) -> &'static str {
    match outcome {
        ProbeFakeRunnerOutcome::Pass => "succeeded",
        ProbeFakeRunnerOutcome::Timeout => "timed_out",
        ProbeFakeRunnerOutcome::Error => "errored",
    }
}

fn failure_family_for_fake_outcome(outcome: &ProbeFakeRunnerOutcome) -> &'static str {
    match outcome {
        ProbeFakeRunnerOutcome::Pass => "none",
        ProbeFakeRunnerOutcome::Timeout => "timeout",
        ProbeFakeRunnerOutcome::Error => "runtime_error",
    }
}

fn media_type_for_file(file_name: &str) -> &'static str {
    if file_name.ends_with(".jsonl") {
        "application/x-ndjson"
    } else {
        "application/json"
    }
}

fn json_contains_unsafe_material(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => string_contains_unsafe_material(value),
        serde_json::Value::Array(values) => values.iter().any(json_contains_unsafe_material),
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            string_contains_unsafe_material(key) || json_contains_unsafe_material(value)
        }),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

fn string_contains_unsafe_material(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    let raw_logs_unsafe = normalized.contains("raw_logs") && !normalized.contains("no_raw_logs");

    normalized.contains("sk-")
        || normalized.contains("bearer ")
        || normalized.contains("refresh_token")
        || normalized.contains("access_token")
        || normalized.contains("auth.json")
        || normalized.contains("private_harbor_trace")
        || normalized.contains("private-repo://")
        || normalized.contains("raw_runner_log")
        || normalized.contains("raw_run_log")
        || normalized.contains("runner_log")
        || normalized.contains("wallet_mnemonic")
        || raw_logs_unsafe
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMINAL_BENCH_PROBE_SMOKE: &str =
        include_str!("../../../fixtures/benchmarks/terminal_bench_probe_contract_smoke.json");
    const TERMINAL_BENCH_PROBE_GEPA_SPLITS: &str = include_str!(
        "../../../fixtures/benchmarks/terminal_bench_probe_gepa_stage_0_1_splits.json"
    );

    fn no_cheat() -> NoCheatMetadata {
        NoCheatMetadata {
            policy_ref: String::from("policy.openagents.benchmark.no_cheat.public_refs_only.v1"),
            no_hidden_verifier_content: true,
            no_private_repo_refs: true,
            no_raw_logs: true,
            no_task_solution_material: true,
        }
    }

    fn scorer_verifier() -> ScorerVerifierRef {
        ScorerVerifierRef {
            scorer_ref: String::from("scorer.terminal_bench.binary.v1"),
            verifier_ref: String::from("verifier.terminal_bench.configure_git_webserver.v1"),
            verifier_public_ref: String::from("verifier_public.terminal_bench.binary_outcome.v1"),
        }
    }

    fn pylon_rollout_capability() -> PylonBenchmarkWorkerCapabilityEnvelope {
        PylonBenchmarkWorkerCapabilityEnvelope {
            schema_ref: String::from(PYLON_BENCHMARK_WORKER_CAPABILITY_SCHEMA_REF),
            capability_ref: String::from("capability.public.pylon.shc_box_1.probe_gepa.v1"),
            worker_ref: String::from("pylon.public.shc_box_1"),
            pylon_version_ref: String::from("release.public.pylon_v0_2_0"),
            advertised_at: String::from("2026-06-08T00:00:00.000Z"),
            benchmark_runner_support: true,
            harbor_terminal_bench_support: true,
            probe_runtime_support: true,
            local_model_support: true,
            apple_fm_support: true,
            qwen_adapter_support: false,
            mlx_training_support: false,
            rollout_eval_support: true,
            model_training_support: false,
            hardware: PylonBenchmarkHardwareEnvelope {
                cpu_arch_ref: String::from("cpu_arch.apple_silicon.arm64"),
                cpu_core_count: 12,
                disk_available_bytes: 500_000_000_000,
                gpu_memory_bytes: Some(32_000_000_000),
                gpu_refs: vec![String::from("gpu.apple.integrated_m_series")],
                ram_bytes: 64_000_000_000,
            },
            max_wall_clock_ms: 1_800_000,
            max_cost_budget_ref: Some(String::from("budget.pylon.no_spend.stage_0")),
            isolation_profile: PylonBenchmarkIsolationProfile::ShcBox,
            artifact_upload_support: true,
            proof_receipt_support: true,
            assignment_lease_support: true,
            closeout_support: true,
            payout_readiness: PylonBenchmarkPayoutReadiness::NoSpendOnly,
            public_capability_refs: vec![
                String::from("capability.public.pylon.benchmark_runner"),
                String::from("capability.public.pylon.probe_runtime"),
                String::from("capability.public.pylon.gepa_rollout_eval"),
            ],
            caveat_refs: vec![String::from(
                "caveat.public.pylon.worker_admission_not_payout_readiness",
            )],
            redaction_state: BenchmarkRedactionState::PublicSafe,
        }
    }

    fn probe_gepa_rollout_requirement() -> PylonBenchmarkWorkRequirement {
        PylonBenchmarkWorkRequirement {
            schema_ref: String::from(PYLON_BENCHMARK_WORK_REQUIREMENT_SCHEMA_REF),
            work_ref: String::from("work.public.probe_gepa.configure_git_webserver.1"),
            work_kind: PylonBenchmarkWorkKind::GepaRolloutMetricCall,
            benchmark_suite_ref: String::from(
                "benchmark_suite.terminal_bench_2.harbor.retained.v1",
            ),
            task_ref: String::from("task.terminal_bench.configure-git-webserver.v1"),
            candidate_hash: Some(String::from("sha256:candidate-1")),
            requires_harbor_terminal_bench: true,
            requires_probe_runtime: true,
            requires_local_model: true,
            requires_apple_fm: false,
            requires_qwen_adapter: false,
            requires_mlx_training: false,
            min_cpu_core_count: 4,
            min_ram_bytes: 16_000_000_000,
            min_disk_available_bytes: 50_000_000_000,
            min_gpu_memory_bytes: None,
            max_wall_clock_ms: 900_000,
            requires_artifact_upload: true,
            requires_proof_receipts: true,
            requires_assignment_lease: true,
            requires_closeout: true,
            accepted_payment_modes: vec![String::from("no_spend")],
        }
    }

    fn result_for(status: BenchmarkRunStatus) -> BenchmarkResult {
        BenchmarkResult {
            schema_ref: String::from(BENCHMARK_RESULT_SCHEMA_REF),
            result_ref: String::from("benchmark_result.terminal_bench.configure_git_webserver.1"),
            run_ref: String::from("benchmark_run.terminal_bench_2.harbor.stage_0.1"),
            task_run_ref: String::from("task_run.configure_git_webserver.1"),
            dataset: BenchmarkDatasetRef {
                slug: String::from("terminal-bench-2-harbor"),
                version: String::from("2026-06-08"),
            },
            evidence_split: BenchmarkEvidenceSplit::Retained,
            status,
            scorer_verifier: scorer_verifier(),
            score_bps: None,
            failure_classification_ref: Some(String::from(
                "failure_classification.configure_git_webserver.service_readiness",
            )),
            artifact_manifest_refs: vec![String::from(
                "artifact_manifest.probe.configure_git_webserver.1",
            )],
            proof_bundle_refs: vec![String::from("proof_bundle.probe.configure_git_webserver.1")],
            resource_usage_receipt_ref: None,
            resource_unavailable_reason: Some(String::from("meter_unavailable_in_smoke")),
            probe_closeout_import: Some(ProbeCloseoutImport {
                probe_assignment_ref: String::from(
                    "probe_benchmark_assignment.configure_git_webserver.1",
                ),
                probe_closeout_ref: String::from(
                    "probe_closeout.probe_run.configure_git_webserver.1",
                ),
                probe_commit: String::from("abc1234"),
                probe_run_record_ref: String::from("probe-run-record.json"),
                probe_run_ref: String::from("probe_run.configure_git_webserver.1"),
                candidate_hash: String::from("sha256:candidate-1"),
                selected_signature_refs: vec![String::from(
                    "program_signature.probe.benchmark.service_readiness.v1",
                )],
                tool_menu_ref: String::from("tool_menu.probe.terminal_bench.service_readiness.v1"),
            }),
            no_cheat: no_cheat(),
            redaction_state: BenchmarkRedactionState::PublicSafe,
            public_claim_boundary: BenchmarkPublicClaimBoundary {
                claim_level: BenchmarkPublicClaimLevel::None,
                external_release_gate_refs: Vec::new(),
                public_claim_upgrade_authority: false,
            },
        }
    }

    #[test]
    fn terminal_bench_harbor_probe_fixture_imports_probe_closeout_refs()
    -> Result<(), Box<dyn std::error::Error>> {
        let result: BenchmarkResult = serde_json::from_str(TERMINAL_BENCH_PROBE_SMOKE)?;

        assert_eq!(result.dataset.slug, "terminal-bench-2-harbor");
        assert_eq!(result.evidence_split, BenchmarkEvidenceSplit::Retained);
        assert!(result.probe_closeout_import.is_some());
        validate_benchmark_result(&result)
            .map_err(|error| format!("unexpected validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn result_contract_represents_all_evidence_splits() {
        let splits = [
            BenchmarkEvidenceSplit::Retained,
            BenchmarkEvidenceSplit::Validation,
            BenchmarkEvidenceSplit::Holdout,
            BenchmarkEvidenceSplit::Live,
        ];

        for split in splits {
            let mut result = result_for(BenchmarkRunStatus::Succeeded);
            result.evidence_split = split;
            result.failure_classification_ref = None;
            assert!(validate_benchmark_result(&result).is_ok());
        }
    }

    #[test]
    fn failed_timed_out_and_errored_results_require_artifact_and_proof_refs() {
        for status in [
            BenchmarkRunStatus::Failed,
            BenchmarkRunStatus::TimedOut,
            BenchmarkRunStatus::Errored,
        ] {
            let mut missing_artifact = result_for(status.clone());
            missing_artifact.artifact_manifest_refs.clear();
            assert!(matches!(
                validate_benchmark_result(&missing_artifact),
                Err(BenchmarkContractError::MissingArtifactManifest { .. })
            ));

            let mut missing_proof = result_for(status);
            missing_proof.proof_bundle_refs.clear();
            assert!(matches!(
                validate_benchmark_result(&missing_proof),
                Err(BenchmarkContractError::MissingProofBundle { .. })
            ));
        }
    }

    #[test]
    fn public_claims_require_live_evidence_and_external_gates() {
        let mut retained_result = result_for(BenchmarkRunStatus::Succeeded);
        retained_result.failure_classification_ref = None;
        retained_result.public_claim_boundary.claim_level =
            BenchmarkPublicClaimLevel::ValidationSummary;
        assert!(matches!(
            validate_benchmark_result(&retained_result),
            Err(BenchmarkContractError::NonLivePublicClaim { .. })
        ));

        let mut live_result = retained_result;
        live_result.evidence_split = BenchmarkEvidenceSplit::Live;
        assert!(matches!(
            validate_benchmark_result(&live_result),
            Err(BenchmarkContractError::MissingPublicClaimReleaseGate { .. })
        ));

        live_result
            .public_claim_boundary
            .external_release_gate_refs
            .push(String::from(
                "release_gate.openagents.public_benchmark_claim_review.v1",
            ));
        assert!(validate_benchmark_result(&live_result).is_ok());
    }

    #[test]
    fn terminal_bench_probe_gepa_split_manifest_loads_and_locks_task_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;

        assert_eq!(
            manifest.manifest_ref,
            "benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1"
        );
        assert_eq!(
            manifest.task_selector_version,
            "task_selector.terminal_bench_2.probe_gepa.stage_0_1.v1"
        );
        assert_eq!(
            manifest.allowed_claim_state,
            BenchmarkPublicClaimLevel::None
        );
        assert_eq!(
            manifest
                .tasks
                .iter()
                .map(|task| task.task_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "configure-git-webserver",
                "db-wal-recovery",
                "filter-js-from-html",
                "gcode-to-text",
                "pypi-server",
                "query-optimize",
                "runner-stall-supervision",
                "validation.db-wal-recovery",
                "validation.configure-git-webserver",
                "validation.pypi-server",
                "validation.filter-js-from-html",
                "validation.gcode-to-text",
                "validation.query-optimize",
                "holdout.openssl-selfsigned-cert",
                "holdout.vulnerable-secret",
                "local-smoke.probe-closeout-writer",
                "local-smoke.apple-fm-tool-stream",
            ]
        );
        validate_campaign_split_manifest(&manifest)
            .map_err(|error| format!("unexpected manifest validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn fake_probe_runner_pass_timeout_and_error_emit_required_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == "configure-git-webserver")
            .ok_or("missing retained configure-git-webserver task")?;
        let assignment = probe_assignment_from_split_task(
            &manifest,
            task,
            "abc1234",
            "sha256:candidate-1",
            vec![String::from(
                "program_signature.probe.benchmark.service_readiness.v1",
            )],
            "tool_menu.probe.terminal_bench.service_readiness.v1",
        );

        for outcome in [
            ProbeFakeRunnerOutcome::Pass,
            ProbeFakeRunnerOutcome::Timeout,
            ProbeFakeRunnerOutcome::Error,
        ] {
            let artifacts = run_fake_probe_benchmark_task(&manifest, task, &assignment, outcome)
                .map_err(|error| format!("unexpected fake Probe runner error: {error:?}"))?;
            assert_eq!(artifacts.file_names(), PROBE_RUNNER_REQUIRED_ARTIFACT_FILES);
            assert_eq!(
                artifacts.proof_bundle_json.probe_selected_signature_refs,
                assignment.selected_blueprint_signature_refs
            );
            assert_eq!(
                artifacts.proof_bundle_json.probe_tool_menu_ref.as_deref(),
                Some(assignment.tool_menu_ref.as_str())
            );
            assert!(!artifacts.artifact_manifest_json.artifacts.is_empty());
            assert!(!artifacts.result_json.artifact_manifest_refs.is_empty());
            assert!(!artifacts.result_json.proof_bundle_refs.is_empty());
            assert!(!artifact_set_contains_unsafe_material(&artifacts));
            validate_benchmark_result(&artifacts.result_json)
                .map_err(|error| format!("unexpected validation error: {error:?}"))?;
        }

        Ok(())
    }

    #[test]
    fn fake_probe_runner_rejects_raw_auth_material() -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let task = manifest
            .tasks
            .iter()
            .find(|task| task.task_id == "configure-git-webserver")
            .ok_or("missing retained configure-git-webserver task")?;
        let mut assignment = probe_assignment_from_split_task(
            &manifest,
            task,
            "abc1234",
            "sha256:candidate-1",
            vec![String::from(
                "program_signature.probe.benchmark.service_readiness.v1",
            )],
            "tool_menu.probe.terminal_bench.service_readiness.v1",
        );
        assignment.auth_grant_ref = Some(String::from("Bearer raw-token"));

        assert!(matches!(
            build_probe_command_invocation(&assignment),
            Err(BenchmarkContractError::ProbeRunnerInvalid { .. })
        ));
        Ok(())
    }

    #[test]
    fn probe_gepa_stage0_smoke_campaign_runs_retained_metric_calls()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let campaign = build_probe_gepa_stage0_smoke_campaign(&manifest, "abc1234")
            .map_err(|error| format!("unexpected Stage 0 smoke error: {error:?}"))?;

        assert_eq!(
            campaign.campaign_id,
            "probe-gepa-stage0-retained-smoke-2026-06-08"
        );
        assert_eq!(campaign.retained_fixture_refs.len(), 5);
        assert_eq!(campaign.candidates.len(), 4);
        assert_eq!(campaign.metric_calls.len(), 20);
        assert!(campaign.metric_calls.iter().all(|call| {
            call.pylon_assignment_ref
                .as_ref()
                .is_some_and(|assignment_ref| {
                    assignment_ref.starts_with("pylon_assignment_ref.public.stage0.shc_box_")
                })
        }));
        assert!(
            campaign
                .metric_calls
                .iter()
                .any(|call| call.closeout_state == BenchmarkCloseoutState::Accepted)
        );
        assert!(
            campaign
                .metric_calls
                .iter()
                .any(|call| call.closeout_state == BenchmarkCloseoutState::Rejected)
        );
        assert!(campaign.metric_calls.iter().all(
            |call| !call.probe_closeout_bundle_ref.is_empty()
                && !call.benchmark_cloud_proof_bundle_ref.is_empty()
                && !call.verifier_import_ref.is_empty()
        ));
        assert_eq!(campaign.public_status_label, "measured retained smoke");
        assert!(!campaign.automatic_promotion_enabled);
        assert!(campaign.no_lora);
        assert!(campaign.no_model_training);
        assert!(campaign.no_public_leaderboard_claim);
        validate_probe_gepa_stage0_smoke_campaign(&campaign, &manifest)
            .map_err(|error| format!("unexpected campaign validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn probe_gepa_stage0_smoke_rejects_public_claim_or_training_overclaim()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let mut campaign = build_probe_gepa_stage0_smoke_campaign(&manifest, "abc1234")
            .map_err(|error| format!("unexpected Stage 0 smoke error: {error:?}"))?;

        campaign.no_model_training = false;
        assert!(matches!(
            validate_probe_gepa_stage0_smoke_campaign(&campaign, &manifest),
            Err(BenchmarkContractError::Stage0SmokeInvalid { .. })
        ));

        campaign.no_model_training = true;
        campaign.public_status_label = String::from("public leaderboard winner");
        assert!(matches!(
            validate_probe_gepa_stage0_smoke_campaign(&campaign, &manifest),
            Err(BenchmarkContractError::Stage0SmokeInvalid { .. })
        ));
        Ok(())
    }

    #[test]
    fn probe_gepa_stage1_retained_sprint_runs_pylon_metric_call_batch()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let campaign = build_probe_gepa_stage1_retained_sprint(&manifest, "abc1234")
            .map_err(|error| format!("unexpected Stage 1 sprint error: {error:?}"))?;

        assert_eq!(
            campaign.campaign_id,
            "probe-gepa-stage1-retained-failure-sprint-2026-06-08"
        );
        assert_eq!(campaign.retained_fixture_refs.len(), 7);
        assert_eq!(campaign.worker_assignment_refs.len(), 8);
        assert_eq!(campaign.candidates.len(), 10);
        assert_eq!(campaign.metric_calls.len(), 210);
        assert!(
            campaign
                .metric_calls
                .iter()
                .all(|call| call.payment_mode == "unpaid_smoke"
                    && call.pylon_assignment_ref.is_some()
                    && !call.verifier_ref.is_empty()
                    && !call.artifact_manifest_ref.is_empty()
                    && !call.benchmark_cloud_proof_bundle_ref.is_empty()
                    && call.resource_usage_receipt_ref.is_some())
        );
        assert!(
            campaign
                .metric_calls
                .iter()
                .any(|call| call.closeout_state == BenchmarkCloseoutState::Accepted)
        );
        assert!(
            campaign
                .metric_calls
                .iter()
                .any(|call| call.closeout_state == BenchmarkCloseoutState::Rejected)
        );
        assert_eq!(
            campaign.selected_candidate_decision,
            GepaCandidateDecisionState::OptimizerAccepted
        );
        assert!(campaign.candidate_improves_or_preserves_retained_fixtures);
        assert!(!campaign.policy_gate_failure);
        assert_eq!(campaign.public_summary_evidence_scope, "retained");
        assert_eq!(campaign.public_summary_label, "retained evidence only");
        assert!(campaign.no_lora);
        assert!(campaign.no_model_training);
        assert!(campaign.no_public_leaderboard_claim);
        validate_probe_gepa_stage1_retained_sprint(&campaign, &manifest)
            .map_err(|error| format!("unexpected campaign validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn probe_gepa_stage1_retained_sprint_rejects_missing_payment_or_overclaim()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let mut campaign = build_probe_gepa_stage1_retained_sprint(&manifest, "abc1234")
            .map_err(|error| format!("unexpected Stage 1 sprint error: {error:?}"))?;

        campaign.metric_calls[0].payment_mode.clear();
        assert!(matches!(
            validate_probe_gepa_stage1_retained_sprint(&campaign, &manifest),
            Err(BenchmarkContractError::Stage1SprintInvalid { .. })
        ));

        campaign.metric_calls[0].payment_mode = String::from("unpaid_smoke");
        campaign.public_summary_evidence_scope = String::from("validation");
        assert!(matches!(
            validate_probe_gepa_stage1_retained_sprint(&campaign, &manifest),
            Err(BenchmarkContractError::Stage1SprintInvalid { .. })
        ));

        campaign.public_summary_evidence_scope = String::from("retained");
        campaign.policy_gate_failure = true;
        assert!(matches!(
            validate_probe_gepa_stage1_retained_sprint(&campaign, &manifest),
            Err(BenchmarkContractError::Stage1SprintInvalid { .. })
        ));
        Ok(())
    }

    #[test]
    fn probe_gepa_validation_sweep_runs_selected_shc_tasks_without_holdout()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let sweep = build_probe_gepa_validation_sweep(
            &manifest,
            "abc1234",
            "sha256:0000000000000000000000000000000000000000000000000000000000002008",
        )
        .map_err(|error| format!("unexpected validation sweep error: {error:?}"))?;

        assert_eq!(sweep.campaign_id, "probe-gepa-validation-sweep-2026-06-08");
        assert_eq!(sweep.validation_task_refs.len(), 6);
        assert_eq!(sweep.routes.len(), 3);
        assert_eq!(sweep.rollout_records.len(), 18);
        assert!(sweep.rollout_records.iter().all(|rollout| {
            rollout.task_id.starts_with("validation.")
                && !rollout.probe_closeout_bundle_ref.is_empty()
                && !rollout.candidate_hash.is_empty()
                && !rollout.verifier_ref.is_empty()
                && !rollout.verifier_result_ref.is_empty()
                && rollout.cost_ref.is_some()
                && rollout.duration_ms.is_some()
                && rollout.artifact_available
        }));
        assert!(sweep.no_public_beats_terminal_bench_claim);
        assert_eq!(sweep.public_claim_label, "validation measured only");
        assert!(sweep.candidate_may_move_to_shadow);
        assert!(
            sweep
                .omega_blueprint_gate_refs
                .iter()
                .any(|gate| gate.starts_with("omega_gate."))
        );
        assert!(
            sweep
                .omega_blueprint_gate_refs
                .iter()
                .any(|gate| gate.starts_with("blueprint_gate."))
        );
        validate_probe_gepa_validation_sweep(&sweep, &manifest)
            .map_err(|error| format!("unexpected sweep validation error: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn probe_gepa_validation_sweep_rejects_holdout_public_claim_or_missing_shadow_gate()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest: BenchmarkCampaignSplitManifest =
            serde_json::from_str(TERMINAL_BENCH_PROBE_GEPA_SPLITS)?;
        let mut sweep = build_probe_gepa_validation_sweep(
            &manifest,
            "abc1234",
            "sha256:0000000000000000000000000000000000000000000000000000000000002008",
        )
        .map_err(|error| format!("unexpected validation sweep error: {error:?}"))?;

        sweep.validation_task_refs[0] =
            String::from("benchmark_task.terminal_bench.holdout.vulnerable_secret.v1");
        assert!(matches!(
            validate_probe_gepa_validation_sweep(&sweep, &manifest),
            Err(BenchmarkContractError::ValidationSweepInvalid { .. })
        ));

        sweep = build_probe_gepa_validation_sweep(
            &manifest,
            "abc1234",
            "sha256:0000000000000000000000000000000000000000000000000000000000002008",
        )
        .map_err(|error| format!("unexpected validation sweep error: {error:?}"))?;
        sweep.no_public_beats_terminal_bench_claim = false;
        assert!(matches!(
            validate_probe_gepa_validation_sweep(&sweep, &manifest),
            Err(BenchmarkContractError::ValidationSweepInvalid { .. })
        ));

        sweep.no_public_beats_terminal_bench_claim = true;
        sweep.omega_blueprint_gate_refs = vec![String::from("omega_gate.only")];
        assert!(matches!(
            validate_probe_gepa_validation_sweep(&sweep, &manifest),
            Err(BenchmarkContractError::ValidationSweepInvalid { .. })
        ));
        Ok(())
    }

    #[test]
    fn pylon_worker_capability_matches_probe_gepa_rollout_without_payout_readiness()
    -> Result<(), Box<dyn std::error::Error>> {
        let capability = pylon_rollout_capability();
        let requirement = probe_gepa_rollout_requirement();
        validate_pylon_benchmark_worker_capability(&capability)
            .map_err(|error| format!("unexpected capability validation error: {error:?}"))?;
        let matched = match_pylon_benchmark_worker(&capability, &requirement)
            .map_err(|error| format!("unexpected match error: {error:?}"))?;

        assert!(matched.matched);
        assert!(matched.admitted_for_assignment);
        assert!(!matched.payout_ready_for_paid_work);
        assert_eq!(matched.worker_ref, "pylon.public.shc_box_1");
        assert_eq!(matched.blocker_refs, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn pylon_worker_capability_distinguishes_rollout_eval_from_model_training() {
        let capability = pylon_rollout_capability();
        let mut training_requirement = probe_gepa_rollout_requirement();
        training_requirement.work_kind = PylonBenchmarkWorkKind::LoraFineTuning;
        training_requirement.requires_mlx_training = true;
        training_requirement.min_gpu_memory_bytes = Some(64_000_000_000);

        let matched = match_pylon_benchmark_worker(&capability, &training_requirement)
            .expect("valid capability should return non-match blockers");

        assert!(!matched.matched);
        assert!(matched.blocker_refs.contains(&String::from(
            "blocker.pylon.capability.model_training_missing"
        )));
        assert!(matched.blocker_refs.contains(&String::from(
            "blocker.pylon.capability.mlx_training_missing"
        )));
        assert!(matched.blocker_refs.contains(&String::from(
            "blocker.pylon.capacity.gpu_memory_insufficient"
        )));
    }

    #[test]
    fn pylon_worker_capability_rejects_overclaiming_and_unsafe_refs() {
        let mut overclaim = pylon_rollout_capability();
        overclaim.mlx_training_support = true;
        assert!(matches!(
            validate_pylon_benchmark_worker_capability(&overclaim),
            Err(BenchmarkContractError::WorkerCapabilityInvalid { .. })
        ));

        let mut unsafe_capability = pylon_rollout_capability();
        unsafe_capability.public_capability_refs = vec![String::from("raw_runner_log.private")];
        assert!(matches!(
            validate_pylon_benchmark_worker_capability(&unsafe_capability),
            Err(BenchmarkContractError::WorkerCapabilityInvalid { .. })
        ));
    }

    #[test]
    fn pylon_worker_capability_admission_does_not_imply_settlement_readiness() {
        let mut settlement_capability = pylon_rollout_capability();
        settlement_capability.payout_readiness = PylonBenchmarkPayoutReadiness::SettlementReady;
        let matched =
            match_pylon_benchmark_worker(&settlement_capability, &probe_gepa_rollout_requirement())
                .expect("settlement-ready worker should still match rollout work");

        assert!(matched.matched);
        assert!(matched.payout_ready_for_paid_work);

        let no_spend_match = match_pylon_benchmark_worker(
            &pylon_rollout_capability(),
            &probe_gepa_rollout_requirement(),
        )
        .expect("no-spend worker should match rollout work");
        assert!(no_spend_match.matched);
        assert!(!no_spend_match.payout_ready_for_paid_work);
    }
}
