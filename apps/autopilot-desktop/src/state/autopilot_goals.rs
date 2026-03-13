//! App-owned autonomous goal specification and persistence model.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_kernel_core::receipts::EvidenceRef;
use serde::{Deserialize, Serialize};

use crate::app_state::{JobHistoryState, SkillRegistryDiscoveredSkill};
use crate::spark_wallet::SparkPaneState;
use crate::state::cron_schedule::{next_cron_run_epoch_seconds, parse_cron_expression};
use crate::state::earnings_gate::{EarningsVerificationReport, verify_authoritative_earnings};
use crate::state::goal_conditions::{
    ConditionEvaluation, GoalProgressSnapshot, evaluate_conditions,
};
use crate::state::goal_skill_resolver::{GoalSkillResolution, resolve_goal_skill_candidates};
use crate::state::os_scheduler::{
    OsSchedulerAdapterConfig, OsSchedulerAdapterKind, OsSchedulerReconcileResult,
    OsSchedulerScheduleSpec, detect_adapter_capability, preferred_adapter_for_host,
    reconcile_os_scheduler_descriptor,
};
use crate::state::swap_contract::{
    GoalSwapExecutionReceipt, SwapExecutionRequest, SwapPolicy, SwapQuoteTerms,
};
use crate::state::swap_quote_adapter::{
    StablesatsQuoteClient, SwapQuoteAdapterOutcome, SwapQuoteAdapterRequest, SwapQuoteAuditReceipt,
    build_swap_quote_audit_receipt, request_quote_with_fallback,
};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalObjective {
    EarnBitcoin {
        min_wallet_delta_sats: u64,
        note: Option<String>,
    },
    SwapBtcToUsd {
        sell_sats: u64,
        note: Option<String>,
    },
    SwapUsdToBtc {
        sell_cents: u64,
        note: Option<String>,
    },
    Custom {
        instruction: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalAutonomyPolicy {
    #[serde(default)]
    pub allowed_command_prefixes: Vec<String>,
    #[serde(default)]
    pub allowed_file_roots: Vec<String>,
    #[serde(default)]
    pub kill_switch_active: bool,
    #[serde(default)]
    pub kill_switch_reason: Option<String>,
}

impl Default for GoalAutonomyPolicy {
    fn default() -> Self {
        Self {
            allowed_command_prefixes: vec!["openagents_".to_string(), "openagents.".to_string()],
            allowed_file_roots: Vec::new(),
            kill_switch_active: false,
            kill_switch_reason: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalConstraints {
    pub max_runtime_seconds: u64,
    pub max_attempts: u32,
    pub max_total_spend_sats: Option<u64>,
    pub max_total_swap_cents: Option<u64>,
    #[serde(default)]
    pub swap_policy: SwapPolicy,
    #[serde(default)]
    pub autonomy_policy: GoalAutonomyPolicy,
}

impl Default for GoalConstraints {
    fn default() -> Self {
        Self {
            max_runtime_seconds: 3_600,
            max_attempts: 12,
            max_total_spend_sats: None,
            max_total_swap_cents: None,
            swap_policy: SwapPolicy::default(),
            autonomy_policy: GoalAutonomyPolicy::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalPolicySnapshot {
    pub max_runtime_seconds: u64,
    pub max_attempts: u32,
    pub max_total_spend_sats: Option<u64>,
    pub max_total_swap_cents: Option<u64>,
    pub max_per_swap_sats: u64,
    pub max_per_swap_cents: u64,
    pub max_daily_converted_sats: u64,
    pub max_daily_converted_cents: u64,
    pub max_fee_sats: u64,
    pub max_slippage_bps: u32,
    pub require_quote_confirmation: bool,
    pub allowed_command_prefixes: Vec<String>,
    pub allowed_file_roots: Vec<String>,
    pub kill_switch_active: bool,
    pub kill_switch_reason: Option<String>,
}

impl Default for GoalPolicySnapshot {
    fn default() -> Self {
        let constraints = GoalConstraints::default();
        constraints.policy_snapshot()
    }
}

impl GoalConstraints {
    pub fn policy_snapshot(&self) -> GoalPolicySnapshot {
        GoalPolicySnapshot {
            max_runtime_seconds: self.max_runtime_seconds,
            max_attempts: self.max_attempts,
            max_total_spend_sats: self.max_total_spend_sats,
            max_total_swap_cents: self.max_total_swap_cents,
            max_per_swap_sats: self.swap_policy.max_per_swap_sats,
            max_per_swap_cents: self.swap_policy.max_per_swap_cents,
            max_daily_converted_sats: self.swap_policy.max_daily_converted_sats,
            max_daily_converted_cents: self.swap_policy.max_daily_converted_cents,
            max_fee_sats: self.swap_policy.max_fee_sats,
            max_slippage_bps: self.swap_policy.max_slippage_bps,
            require_quote_confirmation: self.swap_policy.require_quote_confirmation,
            allowed_command_prefixes: self.autonomy_policy.allowed_command_prefixes.clone(),
            allowed_file_roots: self.autonomy_policy.allowed_file_roots.clone(),
            kill_switch_active: self.autonomy_policy.kill_switch_active,
            kill_switch_reason: self.autonomy_policy.kill_switch_reason.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalStopCondition {
    WalletDeltaSatsAtLeast { sats: u64 },
    JobCountAtLeast { count: u32 },
    SuccessCountAtLeast { count: u32 },
    DeadlineEpochSeconds { epoch_seconds: u64 },
    ErrorBudgetExceeded { max_errors: u32 },
    ExternalSignal { key: String, expected: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRetryPolicy {
    pub max_retries: u32,
    pub backoff_seconds: u64,
    pub exponential_backoff: bool,
}

impl Default for GoalRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_seconds: 10,
            exponential_backoff: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalScheduleKind {
    Manual,
    IntervalSeconds {
        seconds: u64,
    },
    Cron {
        expression: String,
        timezone: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalMissedRunPolicy {
    CatchUp,
    Skip,
    SingleReplay,
}

impl Default for GoalMissedRunPolicy {
    fn default() -> Self {
        Self::SingleReplay
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalScheduleConfig {
    pub enabled: bool,
    pub kind: GoalScheduleKind,
    pub next_run_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub last_run_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub missed_run_policy: GoalMissedRunPolicy,
    #[serde(default)]
    pub pending_catchup_runs: u32,
    #[serde(default)]
    pub last_recovery_epoch_seconds: Option<u64>,
    #[serde(default)]
    pub os_adapter: OsSchedulerAdapterConfig,
}

impl Default for GoalScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: GoalScheduleKind::Manual,
            next_run_epoch_seconds: None,
            last_run_epoch_seconds: None,
            missed_run_policy: GoalMissedRunPolicy::default(),
            pending_catchup_runs: 0,
            last_recovery_epoch_seconds: None,
            os_adapter: OsSchedulerAdapterConfig::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalLifecycleStatus {
    Draft,
    Queued,
    Running,
    Paused,
    Succeeded,
    Failed,
    Aborted,
}

impl GoalLifecycleStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Aborted)
    }
}

impl Default for GoalLifecycleStatus {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRecord {
    pub goal_id: String,
    pub title: String,
    pub objective: GoalObjective,
    pub constraints: GoalConstraints,
    pub stop_conditions: Vec<GoalStopCondition>,
    pub retry_policy: GoalRetryPolicy,
    pub schedule: GoalScheduleConfig,
    #[serde(default)]
    pub lifecycle_status: GoalLifecycleStatus,
    pub created_at_epoch_seconds: u64,
    pub updated_at_epoch_seconds: u64,
    #[serde(default)]
    pub attempt_count: u32,
    #[serde(default)]
    pub last_failure_reason: Option<String>,
    #[serde(default)]
    pub terminal_reason: Option<String>,
    pub last_receipt_id: Option<String>,
    #[serde(default)]
    pub recovery_replay_pending: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalExecutionReceipt {
    pub receipt_id: String,
    pub goal_id: String,
    pub attempt_index: u32,
    pub started_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: u64,
    pub lifecycle_status: GoalLifecycleStatus,
    pub wallet_delta_sats: i64,
    pub jobs_completed: u32,
    pub successes: u32,
    pub errors: u32,
    pub notes: Option<String>,
    #[serde(default)]
    pub recovered_from_restart: bool,
    #[serde(default)]
    pub policy_snapshot: GoalPolicySnapshot,
    #[serde(default)]
    pub terminal_labor: GoalLaborLinkage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalLaborLinkage {
    #[serde(default)]
    pub work_unit_id: Option<String>,
    #[serde(default)]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub submission_id: Option<String>,
    #[serde(default)]
    pub verdict_id: Option<String>,
    #[serde(default)]
    pub claim_id: Option<String>,
    #[serde(default)]
    pub claim_state: Option<String>,
    #[serde(default)]
    pub remedy_kind: Option<String>,
    #[serde(default)]
    pub settlement_id: Option<String>,
    #[serde(default)]
    pub settlement_ready: Option<bool>,
    #[serde(default)]
    pub tool_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub submission_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub verdict_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub claim_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub incident_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub remedy_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub settlement_evidence_refs: Vec<EvidenceRef>,
}

impl GoalLaborLinkage {
    pub fn is_empty(&self) -> bool {
        self.work_unit_id.is_none()
            && self.contract_id.is_none()
            && self.submission_id.is_none()
            && self.verdict_id.is_none()
            && self.claim_id.is_none()
            && self.claim_state.is_none()
            && self.remedy_kind.is_none()
            && self.settlement_id.is_none()
            && self.settlement_ready.is_none()
            && self.tool_evidence_refs.is_empty()
            && self.submission_evidence_refs.is_empty()
            && self.verdict_evidence_refs.is_empty()
            && self.claim_evidence_refs.is_empty()
            && self.incident_evidence_refs.is_empty()
            && self.remedy_evidence_refs.is_empty()
            && self.settlement_evidence_refs.is_empty()
    }

    pub fn merge_from(&mut self, other: &Self) {
        if other.work_unit_id.is_some() {
            self.work_unit_id = other.work_unit_id.clone();
        }
        if other.contract_id.is_some() {
            self.contract_id = other.contract_id.clone();
        }
        if other.submission_id.is_some() {
            self.submission_id = other.submission_id.clone();
        }
        if other.verdict_id.is_some() {
            self.verdict_id = other.verdict_id.clone();
        }
        if other.claim_id.is_some() {
            self.claim_id = other.claim_id.clone();
        }
        if other.claim_state.is_some() {
            self.claim_state = other.claim_state.clone();
        }
        if other.remedy_kind.is_some() {
            self.remedy_kind = other.remedy_kind.clone();
        }
        if other.settlement_id.is_some() {
            self.settlement_id = other.settlement_id.clone();
        }
        if other.settlement_ready.is_some() {
            self.settlement_ready = other.settlement_ready;
        }
        merge_evidence_refs(&mut self.tool_evidence_refs, &other.tool_evidence_refs);
        merge_evidence_refs(
            &mut self.submission_evidence_refs,
            &other.submission_evidence_refs,
        );
        merge_evidence_refs(
            &mut self.verdict_evidence_refs,
            &other.verdict_evidence_refs,
        );
        merge_evidence_refs(&mut self.claim_evidence_refs, &other.claim_evidence_refs);
        merge_evidence_refs(
            &mut self.incident_evidence_refs,
            &other.incident_evidence_refs,
        );
        merge_evidence_refs(&mut self.remedy_evidence_refs, &other.remedy_evidence_refs);
        merge_evidence_refs(
            &mut self.settlement_evidence_refs,
            &other.settlement_evidence_refs,
        );
    }
}

fn merge_evidence_refs(target: &mut Vec<EvidenceRef>, incoming: &[EvidenceRef]) {
    for evidence in incoming {
        if target.iter().any(|existing| existing == evidence) {
            continue;
        }
        target.push(evidence.clone());
    }
    target.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.uri.cmp(&right.uri))
            .then_with(|| left.digest.cmp(&right.digest))
    });
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalToolInvocationAudit {
    pub request_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub response_code: String,
    pub success: bool,
    pub response_message: String,
    pub recorded_at_epoch_seconds: u64,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalAttemptAuditReceipt {
    pub attempt_index: u32,
    pub submitted_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: Option<u64>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub selected_skills: Vec<String>,
    pub turn_status: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub condition_goal_complete: Option<bool>,
    #[serde(default)]
    pub condition_should_continue: Option<bool>,
    #[serde(default)]
    pub condition_completion_reasons: Vec<String>,
    #[serde(default)]
    pub condition_stop_reasons: Vec<String>,
    #[serde(default)]
    pub labor: GoalLaborLinkage,
    #[serde(default)]
    pub tool_invocations: Vec<GoalToolInvocationAudit>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalPayoutEvidence {
    pub event_id: String,
    pub occurred_at_epoch_seconds: u64,
    pub job_id: String,
    pub payment_pointer: String,
    pub payout_sats: u64,
    #[serde(default)]
    pub attempt_index: Option<u32>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub labor: GoalLaborLinkage,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRunAuditReceipt {
    pub audit_id: String,
    pub receipt_id: String,
    pub goal_id: String,
    pub run_id: String,
    pub started_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: u64,
    pub lifecycle_status: GoalLifecycleStatus,
    pub terminal_status_reason: String,
    pub selected_skills: Vec<String>,
    pub attempts: Vec<GoalAttemptAuditReceipt>,
    #[serde(default)]
    pub terminal_labor: GoalLaborLinkage,
    pub condition_goal_complete: Option<bool>,
    pub condition_should_continue: Option<bool>,
    #[serde(default)]
    pub condition_completion_reasons: Vec<String>,
    #[serde(default)]
    pub condition_stop_reasons: Vec<String>,
    #[serde(default)]
    pub payout_evidence: Vec<GoalPayoutEvidence>,
    #[serde(default)]
    pub swap_quote_evidence: Vec<SwapQuoteAuditReceipt>,
    #[serde(default)]
    pub swap_execution_evidence: Vec<GoalSwapExecutionReceipt>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalRolloutStage {
    Disabled,
    InternalDogfood,
    Canary,
    GeneralAvailability,
}

impl GoalRolloutStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::InternalDogfood => "internal_dogfood",
            Self::Canary => "canary",
            Self::GeneralAvailability => "general_availability",
        }
    }
}

impl Default for GoalRolloutStage {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRolloutRollbackPolicy {
    pub max_false_success_rate_bps: u32,
    pub max_abort_rate_bps: u32,
    pub max_error_rate_bps: u32,
    pub max_avg_payout_confirm_latency_seconds: u64,
}

impl Default for GoalRolloutRollbackPolicy {
    fn default() -> Self {
        Self {
            max_false_success_rate_bps: 50,
            max_abort_rate_bps: 2_000,
            max_error_rate_bps: 1_500,
            max_avg_payout_confirm_latency_seconds: 300,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct GoalRolloutHardeningChecklist {
    #[serde(default)]
    pub authoritative_payout_gate_validated: bool,
    #[serde(default)]
    pub scheduler_recovery_drills_validated: bool,
    #[serde(default)]
    pub swap_risk_alerting_validated: bool,
    #[serde(default)]
    pub incident_runbook_validated: bool,
    #[serde(default)]
    pub test_matrix_gate_green: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRolloutConfig {
    #[serde(default)]
    pub feature_flag_enabled: bool,
    #[serde(default)]
    pub stage: GoalRolloutStage,
    #[serde(default)]
    pub allowed_cohorts: Vec<String>,
    #[serde(default)]
    pub rollback_policy: GoalRolloutRollbackPolicy,
    #[serde(default)]
    pub hardening_checklist: GoalRolloutHardeningChecklist,
    #[serde(default)]
    pub last_updated_epoch_seconds: Option<u64>,
}

impl Default for GoalRolloutConfig {
    fn default() -> Self {
        Self {
            feature_flag_enabled: false,
            stage: GoalRolloutStage::Disabled,
            allowed_cohorts: Vec::new(),
            rollback_policy: GoalRolloutRollbackPolicy::default(),
            hardening_checklist: GoalRolloutHardeningChecklist::default(),
            last_updated_epoch_seconds: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRolloutGateDecision {
    pub enabled: bool,
    pub reason: String,
    pub stage: GoalRolloutStage,
    pub local_cohort: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRolloutMetricsSnapshot {
    pub total_runs: u64,
    pub succeeded_runs: u64,
    pub completion_rate_bps: u32,
    pub false_success_runs: u64,
    pub false_success_rate_bps: u32,
    pub payout_confirmed_success_runs: u64,
    pub avg_payout_confirm_latency_seconds: Option<u64>,
    pub aborted_runs: u64,
    pub failed_runs: u64,
    pub error_attempts: u64,
    #[serde(default)]
    pub abort_error_distribution: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalRolloutHealthReport {
    pub healthy: bool,
    pub violations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalRestartRecoveryReport {
    pub recovered_running_goals: Vec<String>,
    pub replay_queued_goals: Vec<String>,
    pub skipped_goals: Vec<String>,
    pub catchup_backlog: Vec<(String, u32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GoalLifecycleEvent {
    Queue,
    StartRun,
    Pause,
    Resume,
    Succeed { reason: Option<String> },
    Fail { reason: String },
    Abort { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GoalStateTransition {
    pub goal_id: String,
    pub from: GoalLifecycleStatus,
    pub to: GoalLifecycleStatus,
    pub event: GoalLifecycleEvent,
    pub attempt_count: u32,
    pub reason: Option<String>,
    pub transitioned_at_epoch_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct AutopilotGoalsDocumentV1 {
    pub schema_version: u16,
    pub active_goals: Vec<GoalRecord>,
    pub historical_goals: Vec<GoalRecord>,
    pub receipts: Vec<GoalExecutionReceipt>,
    #[serde(default)]
    pub swap_quote_audits: Vec<SwapQuoteAuditReceipt>,
    #[serde(default)]
    pub swap_execution_receipts: Vec<GoalSwapExecutionReceipt>,
    #[serde(default)]
    pub run_audit_receipts: Vec<GoalRunAuditReceipt>,
    #[serde(default)]
    pub rollout_config: GoalRolloutConfig,
}

impl Default for AutopilotGoalsDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_goals: Vec::new(),
            historical_goals: Vec::new(),
            receipts: Vec::new(),
            swap_quote_audits: Vec::new(),
            swap_execution_receipts: Vec::new(),
            run_audit_receipts: Vec::new(),
            rollout_config: GoalRolloutConfig::default(),
        }
    }
}

pub struct AutopilotGoalsState {
    pub document: AutopilotGoalsDocumentV1,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    file_path: PathBuf,
}

impl Default for AutopilotGoalsState {
    fn default() -> Self {
        Self::load_from_disk()
    }
}

impl AutopilotGoalsState {
    pub fn load_from_disk() -> Self {
        Self::load_from_path(default_goals_file_path())
    }

    pub fn load_from_path(path: PathBuf) -> Self {
        let mut state = Self {
            document: AutopilotGoalsDocumentV1::default(),
            last_error: None,
            last_action: Some("Goal store ready".to_string()),
            file_path: path.clone(),
        };

        match std::fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<AutopilotGoalsDocumentV1>(&raw) {
                Ok(document) if document.schema_version == 1 => {
                    state.document = document;
                    state.last_action = Some(format!("Loaded goals from {}", path.display()));
                }
                Ok(document) => {
                    state.last_error = Some(format!(
                        "Unsupported goals schema version {}, expected 1",
                        document.schema_version
                    ));
                    state.last_action = Some("Using empty in-memory goal store".to_string());
                }
                Err(error) => {
                    state.last_error = Some(format!("Goals parse error: {error}"));
                    state.last_action = Some("Using empty in-memory goal store".to_string());
                }
            },
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    state.last_error = Some(format!("Goals read error: {error}"));
                }
            }
        }

        state
    }

    pub fn upsert_active_goal(&mut self, mut goal: GoalRecord) -> Result<(), String> {
        goal.updated_at_epoch_seconds = now_epoch_seconds();
        if goal.stop_conditions.is_empty() {
            return Err("Goal must define at least one stop condition".to_string());
        }
        if goal.goal_id.trim().is_empty() {
            return Err("Goal id cannot be empty".to_string());
        }
        if goal.title.trim().is_empty() {
            return Err("Goal title cannot be empty".to_string());
        }
        if goal.lifecycle_status.is_terminal() {
            return Err("Active goal cannot be in a terminal lifecycle status".to_string());
        }

        if let Some(index) = self
            .document
            .active_goals
            .iter()
            .position(|existing| existing.goal_id == goal.goal_id)
        {
            self.document.active_goals[index] = goal.clone();
        } else {
            self.document.active_goals.push(goal.clone());
        }

        self.document
            .historical_goals
            .retain(|existing| existing.goal_id != goal.goal_id);
        self.persist_to_disk()?;
        self.last_action = Some(format!("Upserted active goal {}", goal.goal_id));
        self.last_error = None;
        Ok(())
    }

    pub fn transition_goal(
        &mut self,
        goal_id: &str,
        event: GoalLifecycleEvent,
    ) -> Result<GoalStateTransition, String> {
        let Some(index) = self
            .document
            .active_goals
            .iter()
            .position(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        let current = self.document.active_goals[index].lifecycle_status;
        let (next_status, reason, increment_attempt) = transition_target(current, &event)?;
        let now = now_epoch_seconds();
        let attempt_count_after: u32;

        if next_status.is_terminal() {
            let mut goal = self.document.active_goals.remove(index);
            goal.lifecycle_status = next_status;
            goal.updated_at_epoch_seconds = now;
            if increment_attempt {
                goal.attempt_count = goal.attempt_count.saturating_add(1);
            }
            attempt_count_after = goal.attempt_count;
            match next_status {
                GoalLifecycleStatus::Failed => {
                    goal.last_failure_reason = reason.clone();
                    goal.terminal_reason = reason.clone();
                }
                GoalLifecycleStatus::Aborted | GoalLifecycleStatus::Succeeded => {
                    goal.terminal_reason = reason.clone();
                }
                _ => {}
            }
            self.document
                .historical_goals
                .retain(|existing| existing.goal_id != goal.goal_id);
            self.document.historical_goals.push(goal);
            if self.document.historical_goals.len() > 512 {
                let remove_count = self.document.historical_goals.len().saturating_sub(512);
                self.document.historical_goals.drain(0..remove_count);
            }
        } else {
            let goal = &mut self.document.active_goals[index];
            goal.lifecycle_status = next_status;
            goal.updated_at_epoch_seconds = now;
            if increment_attempt {
                goal.attempt_count = goal.attempt_count.saturating_add(1);
            }
            if next_status != GoalLifecycleStatus::Failed {
                goal.last_failure_reason = None;
            }
            attempt_count_after = goal.attempt_count;
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!(
            "Transitioned goal {goal_id}: {:?} -> {:?}",
            current, next_status
        ));
        self.last_error = None;

        Ok(GoalStateTransition {
            goal_id: goal_id.to_string(),
            from: current,
            to: next_status,
            event,
            attempt_count: attempt_count_after,
            reason,
            transitioned_at_epoch_seconds: now,
        })
    }

    pub fn archive_goal(
        &mut self,
        goal_id: &str,
        terminal_status: GoalLifecycleStatus,
    ) -> Result<(), String> {
        if !terminal_status.is_terminal() {
            return Err("Archived goal status must be terminal".to_string());
        }
        let Some(index) = self
            .document
            .active_goals
            .iter()
            .position(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        let mut goal = self.document.active_goals.remove(index);
        goal.lifecycle_status = terminal_status;
        goal.updated_at_epoch_seconds = now_epoch_seconds();
        if goal.terminal_reason.is_none() {
            goal.terminal_reason = Some(match terminal_status {
                GoalLifecycleStatus::Succeeded => "archived as succeeded".to_string(),
                GoalLifecycleStatus::Failed => "archived as failed".to_string(),
                GoalLifecycleStatus::Aborted => "archived as aborted".to_string(),
                _ => "archived".to_string(),
            });
        }
        if terminal_status == GoalLifecycleStatus::Failed && goal.last_failure_reason.is_none() {
            goal.last_failure_reason = goal.terminal_reason.clone();
        }
        self.document
            .historical_goals
            .retain(|existing| existing.goal_id != goal.goal_id);
        self.document.historical_goals.push(goal);
        if self.document.historical_goals.len() > 512 {
            let remove_count = self.document.historical_goals.len().saturating_sub(512);
            self.document.historical_goals.drain(0..remove_count);
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!("Archived goal {goal_id}"));
        self.last_error = None;
        Ok(())
    }

    pub fn record_receipt(&mut self, receipt: GoalExecutionReceipt) -> Result<(), String> {
        if receipt.receipt_id.trim().is_empty() {
            return Err("Receipt id cannot be empty".to_string());
        }
        if receipt.goal_id.trim().is_empty() {
            return Err("Receipt goal id cannot be empty".to_string());
        }
        if receipt.finished_at_epoch_seconds < receipt.started_at_epoch_seconds {
            return Err(
                "Receipt finished_at_epoch_seconds cannot be before started_at".to_string(),
            );
        }

        let receipt_id = receipt.receipt_id.clone();
        let goal_id = receipt.goal_id.clone();
        self.document.receipts.push(receipt);
        if self.document.receipts.len() > 2_048 {
            let remove_count = self.document.receipts.len().saturating_sub(2_048);
            self.document.receipts.drain(0..remove_count);
        }

        if let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        {
            goal.last_receipt_id = Some(receipt_id.clone());
            goal.updated_at_epoch_seconds = now_epoch_seconds();
        }
        if let Some(goal) = self
            .document
            .historical_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        {
            goal.last_receipt_id = Some(receipt_id);
            goal.updated_at_epoch_seconds = now_epoch_seconds();
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!("Recorded receipt for goal {}", goal_id));
        self.last_error = None;
        Ok(())
    }

    pub fn record_swap_quote_audit(&mut self, audit: SwapQuoteAuditReceipt) -> Result<(), String> {
        if audit.goal_id.trim().is_empty() {
            return Err("Swap quote audit goal id cannot be empty".to_string());
        }
        if audit.request_id.trim().is_empty() {
            return Err("Swap quote audit request id cannot be empty".to_string());
        }
        if audit.quote_id.trim().is_empty() {
            return Err("Swap quote audit quote id cannot be empty".to_string());
        }
        if audit.expires_at_epoch_seconds <= audit.created_at_epoch_seconds {
            return Err("Swap quote audit expiration must be after created_at".to_string());
        }
        if !self
            .document
            .active_goals
            .iter()
            .any(|goal| goal.goal_id == audit.goal_id)
            && !self
                .document
                .historical_goals
                .iter()
                .any(|goal| goal.goal_id == audit.goal_id)
        {
            return Err(format!(
                "Swap quote audit goal {} not found in active or historical goals",
                audit.goal_id
            ));
        }

        self.document.swap_quote_audits.push(audit.clone());
        if self.document.swap_quote_audits.len() > 4_096 {
            let remove_count = self.document.swap_quote_audits.len().saturating_sub(4_096);
            self.document.swap_quote_audits.drain(0..remove_count);
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!(
            "Recorded swap quote audit {} for goal {}",
            audit.quote_id, audit.goal_id
        ));
        self.last_error = None;
        Ok(())
    }

    pub fn record_swap_execution_receipt(
        &mut self,
        receipt: GoalSwapExecutionReceipt,
    ) -> Result<(), String> {
        if receipt.receipt_id.trim().is_empty() {
            return Err("Swap execution receipt id cannot be empty".to_string());
        }
        if receipt.goal_id.trim().is_empty() {
            return Err("Swap execution goal id cannot be empty".to_string());
        }
        if receipt.quote_id.trim().is_empty() {
            return Err("Swap execution quote id cannot be empty".to_string());
        }
        if receipt.finished_at_epoch_seconds < receipt.started_at_epoch_seconds {
            return Err("Swap execution receipt finish cannot be before start".to_string());
        }
        if !self
            .document
            .active_goals
            .iter()
            .any(|goal| goal.goal_id == receipt.goal_id)
            && !self
                .document
                .historical_goals
                .iter()
                .any(|goal| goal.goal_id == receipt.goal_id)
        {
            return Err(format!(
                "Swap execution goal {} not found in active or historical goals",
                receipt.goal_id
            ));
        }

        if let Some(existing) = self
            .document
            .swap_execution_receipts
            .iter_mut()
            .find(|existing| existing.receipt_id == receipt.receipt_id)
        {
            *existing = receipt.clone();
        } else {
            self.document.swap_execution_receipts.push(receipt.clone());
        }
        if self.document.swap_execution_receipts.len() > 4_096 {
            let remove_count = self
                .document
                .swap_execution_receipts
                .len()
                .saturating_sub(4_096);
            self.document.swap_execution_receipts.drain(0..remove_count);
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!(
            "Recorded swap execution receipt {} for goal {}",
            receipt.receipt_id, receipt.goal_id
        ));
        self.last_error = None;
        Ok(())
    }

    pub fn record_run_audit_receipt(&mut self, audit: GoalRunAuditReceipt) -> Result<(), String> {
        if audit.audit_id.trim().is_empty() {
            return Err("Run audit id cannot be empty".to_string());
        }
        if audit.goal_id.trim().is_empty() {
            return Err("Run audit goal id cannot be empty".to_string());
        }
        if audit.run_id.trim().is_empty() {
            return Err("Run audit run id cannot be empty".to_string());
        }
        if audit.receipt_id.trim().is_empty() {
            return Err("Run audit receipt id cannot be empty".to_string());
        }
        if audit.finished_at_epoch_seconds < audit.started_at_epoch_seconds {
            return Err("Run audit finish cannot be before start".to_string());
        }
        if !self
            .document
            .active_goals
            .iter()
            .any(|goal| goal.goal_id == audit.goal_id)
            && !self
                .document
                .historical_goals
                .iter()
                .any(|goal| goal.goal_id == audit.goal_id)
        {
            return Err(format!(
                "Run audit goal {} not found in active or historical goals",
                audit.goal_id
            ));
        }

        if let Some(existing) = self
            .document
            .run_audit_receipts
            .iter_mut()
            .find(|existing| existing.audit_id == audit.audit_id)
        {
            *existing = audit.clone();
        } else {
            self.document.run_audit_receipts.push(audit.clone());
        }
        if self.document.run_audit_receipts.len() > 2_048 {
            let remove_count = self.document.run_audit_receipts.len().saturating_sub(2_048);
            self.document.run_audit_receipts.drain(0..remove_count);
        }

        self.persist_to_disk()?;
        self.last_action = Some(format!(
            "Recorded run audit {} for goal {}",
            audit.audit_id, audit.goal_id
        ));
        self.last_error = None;
        Ok(())
    }

    pub fn update_rollout_config(
        &mut self,
        feature_flag_enabled: Option<bool>,
        stage: Option<GoalRolloutStage>,
        allowed_cohorts: Option<Vec<String>>,
        rollback_policy: Option<GoalRolloutRollbackPolicy>,
        hardening_checklist: Option<GoalRolloutHardeningChecklist>,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        if let Some(enabled) = feature_flag_enabled {
            self.document.rollout_config.feature_flag_enabled = enabled;
        }
        if let Some(stage) = stage {
            self.document.rollout_config.stage = stage;
        }
        if let Some(allowed_cohorts) = allowed_cohorts {
            let mut normalized = allowed_cohorts
                .into_iter()
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            normalized.sort();
            normalized.dedup();
            self.document.rollout_config.allowed_cohorts = normalized;
        }
        if let Some(rollback_policy) = rollback_policy {
            if rollback_policy.max_false_success_rate_bps > 10_000 {
                return Err("max_false_success_rate_bps must be <= 10000".to_string());
            }
            if rollback_policy.max_abort_rate_bps > 10_000 {
                return Err("max_abort_rate_bps must be <= 10000".to_string());
            }
            if rollback_policy.max_error_rate_bps > 10_000 {
                return Err("max_error_rate_bps must be <= 10000".to_string());
            }
            self.document.rollout_config.rollback_policy = rollback_policy;
        }
        if let Some(hardening_checklist) = hardening_checklist {
            self.document.rollout_config.hardening_checklist = hardening_checklist;
        }
        self.document.rollout_config.last_updated_epoch_seconds = Some(now_epoch_seconds);
        self.persist_to_disk()?;
        self.last_action = Some(format!(
            "Updated rollout config: enabled={} stage={}",
            self.document.rollout_config.feature_flag_enabled,
            self.document.rollout_config.stage.as_str()
        ));
        self.last_error = None;
        Ok(())
    }

    pub fn rollout_gate_decision(&self) -> GoalRolloutGateDecision {
        let config = &self.document.rollout_config;
        let local_cohort = local_rollout_cohort();

        if !config.feature_flag_enabled {
            return GoalRolloutGateDecision {
                enabled: false,
                reason: "feature flag disabled".to_string(),
                stage: config.stage,
                local_cohort,
            };
        }

        match config.stage {
            GoalRolloutStage::Disabled => GoalRolloutGateDecision {
                enabled: false,
                reason: "rollout stage disabled".to_string(),
                stage: config.stage,
                local_cohort,
            },
            GoalRolloutStage::GeneralAvailability => GoalRolloutGateDecision {
                enabled: true,
                reason: "general availability enabled".to_string(),
                stage: config.stage,
                local_cohort,
            },
            GoalRolloutStage::InternalDogfood | GoalRolloutStage::Canary => {
                if config.allowed_cohorts.is_empty() {
                    return GoalRolloutGateDecision {
                        enabled: false,
                        reason: "staged rollout has no allowed cohorts".to_string(),
                        stage: config.stage,
                        local_cohort,
                    };
                }
                if config.allowed_cohorts.iter().any(|cohort| cohort == "*") {
                    return GoalRolloutGateDecision {
                        enabled: true,
                        reason: format!(
                            "staged rollout wildcard cohort allowed for {}",
                            config.stage.as_str()
                        ),
                        stage: config.stage,
                        local_cohort,
                    };
                }
                let Some(local) = local_cohort.clone() else {
                    return GoalRolloutGateDecision {
                        enabled: false,
                        reason: "no local rollout cohort configured".to_string(),
                        stage: config.stage,
                        local_cohort,
                    };
                };
                let local_normalized = local.trim().to_ascii_lowercase();
                let matched = config
                    .allowed_cohorts
                    .iter()
                    .any(|cohort| cohort.eq_ignore_ascii_case(&local_normalized));
                if matched {
                    GoalRolloutGateDecision {
                        enabled: true,
                        reason: format!(
                            "staged rollout cohort '{}' allowed for {}",
                            local_normalized,
                            config.stage.as_str()
                        ),
                        stage: config.stage,
                        local_cohort: Some(local_normalized),
                    }
                } else {
                    GoalRolloutGateDecision {
                        enabled: false,
                        reason: format!(
                            "cohort '{}' not allowlisted for {}",
                            local_normalized,
                            config.stage.as_str()
                        ),
                        stage: config.stage,
                        local_cohort: Some(local_normalized),
                    }
                }
            }
        }
    }

    pub fn rollout_metrics_snapshot(&self) -> GoalRolloutMetricsSnapshot {
        let mut total_runs = 0u64;
        let mut succeeded_runs = 0u64;
        let mut false_success_runs = 0u64;
        let mut payout_confirmed_success_runs = 0u64;
        let mut aborted_runs = 0u64;
        let mut failed_runs = 0u64;
        let mut error_attempts = 0u64;
        let mut latency_samples = 0u64;
        let mut latency_total = 0u64;
        let mut abort_error_distribution = BTreeMap::<String, u64>::new();

        for audit in &self.document.run_audit_receipts {
            total_runs = total_runs.saturating_add(1);
            match audit.lifecycle_status {
                GoalLifecycleStatus::Succeeded => {
                    succeeded_runs = succeeded_runs.saturating_add(1);
                    if audit.payout_evidence.is_empty() {
                        false_success_runs = false_success_runs.saturating_add(1);
                        *abort_error_distribution
                            .entry("false_success_without_payout_evidence".to_string())
                            .or_insert(0) += 1;
                    } else {
                        payout_confirmed_success_runs =
                            payout_confirmed_success_runs.saturating_add(1);
                        if let Some(first_payout) = audit
                            .payout_evidence
                            .iter()
                            .map(|entry| entry.occurred_at_epoch_seconds)
                            .min()
                        {
                            latency_samples = latency_samples.saturating_add(1);
                            latency_total = latency_total.saturating_add(
                                first_payout.saturating_sub(audit.started_at_epoch_seconds),
                            );
                        }
                    }
                }
                GoalLifecycleStatus::Aborted => {
                    aborted_runs = aborted_runs.saturating_add(1);
                    *abort_error_distribution
                        .entry("aborted".to_string())
                        .or_insert(0) += 1;
                }
                GoalLifecycleStatus::Failed => {
                    failed_runs = failed_runs.saturating_add(1);
                    *abort_error_distribution
                        .entry("failed".to_string())
                        .or_insert(0) += 1;
                }
                _ => {}
            }

            for attempt in &audit.attempts {
                if attempt
                    .error
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                {
                    error_attempts = error_attempts.saturating_add(1);
                    *abort_error_distribution
                        .entry("attempt_error".to_string())
                        .or_insert(0) += 1;
                }
            }
        }

        GoalRolloutMetricsSnapshot {
            total_runs,
            succeeded_runs,
            completion_rate_bps: rate_bps(succeeded_runs, total_runs),
            false_success_runs,
            false_success_rate_bps: rate_bps(false_success_runs, succeeded_runs),
            payout_confirmed_success_runs,
            avg_payout_confirm_latency_seconds: if latency_samples > 0 {
                Some(latency_total / latency_samples)
            } else {
                None
            },
            aborted_runs,
            failed_runs,
            error_attempts,
            abort_error_distribution,
        }
    }

    pub fn rollout_health_report(&self) -> GoalRolloutHealthReport {
        let metrics = self.rollout_metrics_snapshot();
        let policy = &self.document.rollout_config.rollback_policy;
        let mut violations = Vec::<String>::new();

        if metrics.false_success_rate_bps > policy.max_false_success_rate_bps {
            violations.push(format!(
                "false_success_rate_bps {} > {}",
                metrics.false_success_rate_bps, policy.max_false_success_rate_bps
            ));
        }

        let abort_rate_bps = rate_bps(metrics.aborted_runs, metrics.total_runs);
        if abort_rate_bps > policy.max_abort_rate_bps {
            violations.push(format!(
                "abort_rate_bps {} > {}",
                abort_rate_bps, policy.max_abort_rate_bps
            ));
        }

        let error_rate_bps = rate_bps(metrics.failed_runs, metrics.total_runs);
        if error_rate_bps > policy.max_error_rate_bps {
            violations.push(format!(
                "error_rate_bps {} > {}",
                error_rate_bps, policy.max_error_rate_bps
            ));
        }

        if let Some(avg_latency) = metrics.avg_payout_confirm_latency_seconds
            && avg_latency > policy.max_avg_payout_confirm_latency_seconds
        {
            violations.push(format!(
                "avg_payout_confirm_latency_seconds {} > {}",
                avg_latency, policy.max_avg_payout_confirm_latency_seconds
            ));
        }

        GoalRolloutHealthReport {
            healthy: violations.is_empty(),
            violations,
        }
    }

    pub fn evaluate_active_goal_conditions(
        &self,
        goal_id: &str,
        progress: &GoalProgressSnapshot,
    ) -> Result<ConditionEvaluation, String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };
        Ok(evaluate_conditions(goal, progress))
    }

    pub fn verify_authoritative_earnings_gate(
        &self,
        goal_id: &str,
        progress: &GoalProgressSnapshot,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
    ) -> Result<EarningsVerificationReport, String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };
        Ok(verify_authoritative_earnings(
            goal,
            progress,
            job_history,
            spark_wallet,
        ))
    }

    pub fn validate_swap_request_policy(
        &self,
        goal_id: &str,
        request: &SwapExecutionRequest,
        daily_converted_sats: u64,
        daily_converted_cents: u64,
    ) -> Result<(), String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };
        goal.constraints.swap_policy.validate_request(
            request,
            daily_converted_sats,
            daily_converted_cents,
        )
    }

    pub fn validate_swap_quote_policy(
        &self,
        goal_id: &str,
        quote: &SwapQuoteTerms,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };
        goal.constraints
            .swap_policy
            .validate_quote(quote, now_epoch_seconds)
    }

    pub fn request_swap_quote_with_adapter<C: StablesatsQuoteClient>(
        &mut self,
        goal_id: &str,
        request: &SwapQuoteAdapterRequest,
        stablesats_client: &mut C,
        fallback_quote: SwapQuoteTerms,
    ) -> Result<SwapQuoteAdapterOutcome, String> {
        if !self
            .document
            .active_goals
            .iter()
            .any(|goal| goal.goal_id == goal_id)
        {
            return Err(format!("Active goal {goal_id} not found"));
        }

        let outcome = request_quote_with_fallback(stablesats_client, request, fallback_quote);
        let audit = build_swap_quote_audit_receipt(goal_id, request, &outcome);
        self.record_swap_quote_audit(audit)?;
        Ok(outcome)
    }

    pub fn resolve_skill_candidates_for_goal(
        &self,
        goal_id: &str,
        discovered_skills: &[SkillRegistryDiscoveredSkill],
    ) -> Result<GoalSkillResolution, String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };
        Ok(resolve_goal_skill_candidates(goal, discovered_skills))
    }

    pub fn recover_after_restart(
        &mut self,
        now_epoch_seconds: u64,
    ) -> Result<GoalRestartRecoveryReport, String> {
        let mut recovered_running_goals = Vec::new();
        let mut replay_queued_goals = Vec::new();
        let mut skipped_goals = Vec::new();
        let mut catchup_backlog = Vec::new();
        let mut touched = false;

        for goal in &mut self.document.active_goals {
            if goal.lifecycle_status == GoalLifecycleStatus::Running {
                goal.lifecycle_status = GoalLifecycleStatus::Queued;
                goal.recovery_replay_pending = true;
                goal.schedule.last_recovery_epoch_seconds = Some(now_epoch_seconds);
                goal.updated_at_epoch_seconds = now_epoch_seconds;
                recovered_running_goals.push(goal.goal_id.clone());
                touched = true;
            }

            if !goal.schedule.enabled {
                continue;
            }
            let Some(next_run_epoch_seconds) = goal.schedule.next_run_epoch_seconds else {
                continue;
            };
            if next_run_epoch_seconds > now_epoch_seconds {
                continue;
            }

            match goal.schedule.missed_run_policy {
                GoalMissedRunPolicy::Skip => {
                    goal.schedule.next_run_epoch_seconds =
                        Some(next_schedule_after(&goal.schedule.kind, now_epoch_seconds)?);
                    goal.schedule.last_recovery_epoch_seconds = Some(now_epoch_seconds);
                    goal.updated_at_epoch_seconds = now_epoch_seconds;
                    skipped_goals.push(goal.goal_id.clone());
                    touched = true;
                }
                GoalMissedRunPolicy::SingleReplay => {
                    goal.recovery_replay_pending = true;
                    if matches!(
                        goal.lifecycle_status,
                        GoalLifecycleStatus::Draft | GoalLifecycleStatus::Paused
                    ) {
                        goal.lifecycle_status = GoalLifecycleStatus::Queued;
                    }
                    goal.schedule.next_run_epoch_seconds =
                        Some(next_schedule_after(&goal.schedule.kind, now_epoch_seconds)?);
                    goal.schedule.last_recovery_epoch_seconds = Some(now_epoch_seconds);
                    goal.updated_at_epoch_seconds = now_epoch_seconds;
                    replay_queued_goals.push(goal.goal_id.clone());
                    touched = true;
                }
                GoalMissedRunPolicy::CatchUp => {
                    let mut missed_runs = 0u32;
                    let mut cursor = next_run_epoch_seconds;
                    while cursor <= now_epoch_seconds && missed_runs < 128 {
                        missed_runs = missed_runs.saturating_add(1);
                        cursor = next_schedule_after(&goal.schedule.kind, cursor)?;
                    }
                    if missed_runs > 0 {
                        goal.recovery_replay_pending = true;
                        goal.schedule.pending_catchup_runs = goal
                            .schedule
                            .pending_catchup_runs
                            .saturating_add(missed_runs.saturating_sub(1));
                        if matches!(
                            goal.lifecycle_status,
                            GoalLifecycleStatus::Draft | GoalLifecycleStatus::Paused
                        ) {
                            goal.lifecycle_status = GoalLifecycleStatus::Queued;
                        }
                        goal.schedule.next_run_epoch_seconds = Some(cursor);
                        goal.schedule.last_recovery_epoch_seconds = Some(now_epoch_seconds);
                        goal.updated_at_epoch_seconds = now_epoch_seconds;
                        replay_queued_goals.push(goal.goal_id.clone());
                        catchup_backlog.push((goal.goal_id.clone(), missed_runs));
                        touched = true;
                    }
                }
            }
        }

        if touched {
            self.persist_to_disk()?;
            self.last_error = None;
            self.last_action = Some(format!(
                "Recovered goals on startup running={} replay={} skipped={}",
                recovered_running_goals.len(),
                replay_queued_goals.len(),
                skipped_goals.len()
            ));
        }

        Ok(GoalRestartRecoveryReport {
            recovered_running_goals,
            replay_queued_goals,
            skipped_goals,
            catchup_backlog,
        })
    }

    pub fn consume_recovery_replay_flag(&mut self, goal_id: &str) -> Result<bool, String> {
        let Some(index) = self
            .document
            .active_goals
            .iter()
            .position(|goal| goal.goal_id == goal_id)
        else {
            return Ok(false);
        };
        let was_pending = self.document.active_goals[index].recovery_replay_pending;
        if was_pending {
            self.document.active_goals[index].recovery_replay_pending = false;
            self.document.active_goals[index].updated_at_epoch_seconds = now_epoch_seconds();
            if let Err(error) = self.persist_to_disk() {
                self.document.active_goals[index].recovery_replay_pending = true;
                return Err(error);
            }
        }
        Ok(was_pending)
    }

    pub fn set_goal_interval_schedule(
        &mut self,
        goal_id: &str,
        interval_seconds: u64,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        if interval_seconds == 0 {
            return Err("Interval schedule seconds must be greater than zero".to_string());
        }
        let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        goal.schedule.enabled = true;
        goal.schedule.kind = GoalScheduleKind::IntervalSeconds {
            seconds: interval_seconds,
        };
        goal.schedule.next_run_epoch_seconds =
            Some(now_epoch_seconds.saturating_add(interval_seconds));
        goal.schedule.pending_catchup_runs = 0;
        if goal.schedule.os_adapter.enabled {
            reconcile_goal_os_scheduler(goal, now_epoch_seconds)?;
        }
        goal.updated_at_epoch_seconds = now_epoch_seconds;
        self.persist_to_disk()?;
        self.last_error = None;
        self.last_action = Some(format!(
            "Set interval schedule for goal {} to {}s",
            goal_id, interval_seconds
        ));
        Ok(())
    }

    pub fn set_goal_cron_schedule(
        &mut self,
        goal_id: &str,
        expression: &str,
        timezone: &str,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        let normalized_expression = expression.trim();
        if normalized_expression.is_empty() {
            return Err("Cron expression cannot be empty".to_string());
        }
        let normalized_timezone = if timezone.trim().is_empty() {
            "UTC".to_string()
        } else {
            timezone.trim().to_string()
        };

        let spec = parse_cron_expression(normalized_expression)?;
        let next_run = next_cron_run_epoch_seconds(&spec, &normalized_timezone, now_epoch_seconds)?;

        let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        goal.schedule.enabled = true;
        goal.schedule.kind = GoalScheduleKind::Cron {
            expression: normalized_expression.to_string(),
            timezone: Some(normalized_timezone.clone()),
        };
        goal.schedule.next_run_epoch_seconds = Some(next_run);
        goal.schedule.pending_catchup_runs = 0;
        if goal.schedule.os_adapter.enabled {
            reconcile_goal_os_scheduler(goal, now_epoch_seconds)?;
        }
        goal.updated_at_epoch_seconds = now_epoch_seconds;
        self.persist_to_disk()?;
        self.last_error = None;
        self.last_action = Some(format!(
            "Set cron schedule for goal {} to '{}' tz={}",
            goal_id, normalized_expression, normalized_timezone
        ));
        Ok(())
    }

    pub fn set_goal_os_scheduler_adapter(
        &mut self,
        goal_id: &str,
        enabled: bool,
        adapter: Option<OsSchedulerAdapterKind>,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        if enabled {
            let resolved_adapter = adapter
                .or(goal.schedule.os_adapter.adapter)
                .or_else(preferred_adapter_for_host)
                .ok_or_else(|| {
                    "No supported OS-backed scheduler adapter available on this host".to_string()
                })?;
            let capability = detect_adapter_capability(resolved_adapter);
            if !capability.available {
                return Err(capability.reason.unwrap_or_else(|| {
                    format!(
                        "{} adapter unavailable on current host",
                        resolved_adapter.as_str()
                    )
                }));
            }
            goal.schedule.os_adapter.enabled = true;
            goal.schedule.os_adapter.adapter = Some(resolved_adapter);

            if goal.schedule.enabled {
                let reconcile = reconcile_goal_os_scheduler(goal, now_epoch_seconds)?;
                goal.schedule.os_adapter.last_reconcile_result = Some(format!(
                    "reconciled via {} ({})",
                    resolved_adapter.as_str(),
                    reconcile.install_command_preview
                ));
            } else {
                goal.schedule.os_adapter.last_reconciled_epoch_seconds = Some(now_epoch_seconds);
                goal.schedule.os_adapter.last_reconcile_result =
                    Some("adapter enabled (awaiting schedule apply)".to_string());
            }
        } else {
            goal.schedule.os_adapter.enabled = false;
            goal.schedule.os_adapter.adapter = adapter.or(goal.schedule.os_adapter.adapter);
            goal.schedule.os_adapter.adapter_job_id = None;
            goal.schedule.os_adapter.descriptor_path = None;
            goal.schedule.os_adapter.reconciliation_marker = None;
            goal.schedule.os_adapter.last_reconciled_epoch_seconds = Some(now_epoch_seconds);
            goal.schedule.os_adapter.last_reconcile_result = Some("adapter disabled".to_string());
        }

        goal.updated_at_epoch_seconds = now_epoch_seconds;
        self.persist_to_disk()?;
        self.last_error = None;
        self.last_action = Some(format!(
            "{} OS scheduler adapter for goal {}",
            if enabled { "Enabled" } else { "Disabled" },
            goal_id
        ));
        Ok(())
    }

    pub fn set_goal_missed_run_policy(
        &mut self,
        goal_id: &str,
        policy: GoalMissedRunPolicy,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };
        goal.schedule.missed_run_policy = policy;
        goal.schedule.last_recovery_epoch_seconds = Some(now_epoch_seconds);
        goal.updated_at_epoch_seconds = now_epoch_seconds;
        self.persist_to_disk()?;
        self.last_error = None;
        self.last_action = Some(format!(
            "Set missed-run policy for goal {} to {:?}",
            goal_id, policy
        ));
        Ok(())
    }

    pub fn set_goal_kill_switch(
        &mut self,
        goal_id: &str,
        active: bool,
        reason: Option<&str>,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        let Some(goal) = self
            .document
            .active_goals
            .iter_mut()
            .find(|goal| goal.goal_id == goal_id)
        else {
            return Err(format!("Active goal {goal_id} not found"));
        };

        goal.constraints.autonomy_policy.kill_switch_active = active;
        goal.constraints.autonomy_policy.kill_switch_reason = if active {
            reason
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .or_else(|| Some("kill switch engaged".to_string()))
        } else {
            None
        };
        goal.updated_at_epoch_seconds = now_epoch_seconds;
        self.persist_to_disk()?;
        self.last_error = None;
        self.last_action = Some(format!(
            "{} kill switch for goal {}",
            if active { "Enabled" } else { "Disabled" },
            goal_id
        ));
        Ok(())
    }

    pub fn reconcile_os_scheduler_adapters(
        &mut self,
        now_epoch_seconds: u64,
    ) -> Result<Vec<String>, String> {
        let mut touched = false;
        let mut reconciled_goal_ids = Vec::new();
        let mut errors = Vec::new();

        for goal in &mut self.document.active_goals {
            if !goal.schedule.enabled || !goal.schedule.os_adapter.enabled {
                continue;
            }
            match reconcile_goal_os_scheduler(goal, now_epoch_seconds) {
                Ok(reconcile) => {
                    goal.schedule.os_adapter.last_reconcile_result = Some(format!(
                        "reconciled via {} ({})",
                        goal.schedule
                            .os_adapter
                            .adapter
                            .map(|kind| kind.as_str().to_string())
                            .unwrap_or_else(|| "auto".to_string()),
                        reconcile.install_command_preview
                    ));
                    goal.updated_at_epoch_seconds = now_epoch_seconds;
                    touched = true;
                    reconciled_goal_ids.push(goal.goal_id.clone());
                }
                Err(error) => {
                    goal.schedule.os_adapter.last_reconciled_epoch_seconds =
                        Some(now_epoch_seconds);
                    goal.schedule.os_adapter.last_reconcile_result =
                        Some(format!("reconcile failed: {error}"));
                    goal.updated_at_epoch_seconds = now_epoch_seconds;
                    touched = true;
                    errors.push(format!("goal {}: {}", goal.goal_id, error));
                }
            }
        }

        if touched {
            self.persist_to_disk()?;
        }
        if !reconciled_goal_ids.is_empty() {
            self.last_action = Some(format!(
                "Reconciled OS scheduler adapters for goals: {}",
                reconciled_goal_ids.join(",")
            ));
            self.last_error = None;
        }
        if !errors.is_empty() {
            self.last_error = Some(errors.join(" | "));
        }
        Ok(reconciled_goal_ids)
    }

    pub fn schedule_goal_run_now(
        &mut self,
        goal_id: &str,
        now_epoch_seconds: u64,
    ) -> Result<(), String> {
        let lifecycle_status = {
            let Some(goal) = self
                .document
                .active_goals
                .iter_mut()
                .find(|goal| goal.goal_id == goal_id)
            else {
                return Err(format!("Active goal {goal_id} not found"));
            };

            goal.schedule.last_run_epoch_seconds = Some(now_epoch_seconds);
            match goal.schedule.kind {
                GoalScheduleKind::IntervalSeconds { seconds } => {
                    goal.schedule.next_run_epoch_seconds =
                        Some(now_epoch_seconds.saturating_add(seconds.max(1)));
                }
                GoalScheduleKind::Cron {
                    ref expression,
                    ref timezone,
                } => {
                    let timezone = timezone.as_deref().unwrap_or("UTC");
                    let spec = parse_cron_expression(expression)?;
                    goal.schedule.next_run_epoch_seconds = Some(next_cron_run_epoch_seconds(
                        &spec,
                        timezone,
                        now_epoch_seconds,
                    )?);
                }
                GoalScheduleKind::Manual => {
                    goal.schedule.next_run_epoch_seconds = None;
                }
            }
            goal.updated_at_epoch_seconds = now_epoch_seconds;
            if goal.schedule.os_adapter.enabled && goal.schedule.enabled {
                reconcile_goal_os_scheduler(goal, now_epoch_seconds)?;
            }
            goal.lifecycle_status
        };
        self.persist_to_disk()?;

        match lifecycle_status {
            GoalLifecycleStatus::Draft => {
                self.transition_goal(goal_id, GoalLifecycleEvent::Queue)?;
            }
            GoalLifecycleStatus::Paused => {
                self.transition_goal(goal_id, GoalLifecycleEvent::Resume)?;
            }
            GoalLifecycleStatus::Queued | GoalLifecycleStatus::Running => {}
            status => {
                return Err(format!("Cannot schedule run now from {status:?}"));
            }
        }

        self.last_error = None;
        self.last_action = Some(format!("Scheduled immediate run for goal {}", goal_id));
        Ok(())
    }

    pub fn run_scheduler_tick(&mut self, now_epoch_seconds: u64) -> Result<Vec<String>, String> {
        let mut scheduler_errors = Vec::new();
        let mut triggered_goal_ids = Vec::new();
        let mut queue_goal_ids = Vec::new();
        let mut touched = false;

        for goal in &mut self.document.active_goals {
            if !goal.schedule.enabled {
                continue;
            }

            match goal.schedule.kind {
                GoalScheduleKind::IntervalSeconds { seconds } => {
                    let interval_seconds = seconds.max(1);
                    let next_run = match goal.schedule.next_run_epoch_seconds {
                        Some(value) => value,
                        None => {
                            let next = now_epoch_seconds.saturating_add(interval_seconds);
                            goal.schedule.next_run_epoch_seconds = Some(next);
                            goal.updated_at_epoch_seconds = now_epoch_seconds;
                            touched = true;
                            continue;
                        }
                    };
                    if now_epoch_seconds < next_run {
                        continue;
                    }

                    goal.schedule.last_run_epoch_seconds = Some(now_epoch_seconds);
                    goal.schedule.next_run_epoch_seconds =
                        Some(now_epoch_seconds.saturating_add(interval_seconds));
                    goal.updated_at_epoch_seconds = now_epoch_seconds;
                    touched = true;
                    triggered_goal_ids.push(goal.goal_id.clone());
                }
                GoalScheduleKind::Cron {
                    ref expression,
                    ref timezone,
                } => {
                    let timezone = timezone.as_deref().unwrap_or("UTC");
                    let spec = match parse_cron_expression(expression) {
                        Ok(value) => value,
                        Err(error) => {
                            scheduler_errors
                                .push(format!("goal {} cron parse error: {}", goal.goal_id, error));
                            continue;
                        }
                    };
                    let next_preview =
                        match next_cron_run_epoch_seconds(&spec, timezone, now_epoch_seconds) {
                            Ok(value) => value,
                            Err(error) => {
                                scheduler_errors.push(format!(
                                    "goal {} cron preview error: {}",
                                    goal.goal_id, error
                                ));
                                continue;
                            }
                        };

                    let next_run = goal.schedule.next_run_epoch_seconds.unwrap_or(next_preview);
                    if goal.schedule.next_run_epoch_seconds.is_none() {
                        goal.schedule.next_run_epoch_seconds = Some(next_run);
                        goal.updated_at_epoch_seconds = now_epoch_seconds;
                        touched = true;
                    }

                    if now_epoch_seconds < next_run {
                        continue;
                    }

                    goal.schedule.last_run_epoch_seconds = Some(now_epoch_seconds);
                    goal.schedule.next_run_epoch_seconds = Some(next_preview.max(next_run + 60));
                    goal.updated_at_epoch_seconds = now_epoch_seconds;
                    touched = true;
                    triggered_goal_ids.push(goal.goal_id.clone());
                }
                GoalScheduleKind::Manual => continue,
            }

            if matches!(
                goal.lifecycle_status,
                GoalLifecycleStatus::Draft | GoalLifecycleStatus::Paused
            ) {
                queue_goal_ids.push(goal.goal_id.clone());
            }
        }

        if touched {
            self.persist_to_disk()?;
        }

        for goal_id in queue_goal_ids {
            let lifecycle_status = self
                .document
                .active_goals
                .iter()
                .find(|goal| goal.goal_id == goal_id)
                .map(|goal| goal.lifecycle_status);
            match lifecycle_status {
                Some(GoalLifecycleStatus::Draft) => {
                    self.transition_goal(&goal_id, GoalLifecycleEvent::Queue)?;
                }
                Some(GoalLifecycleStatus::Paused) => {
                    self.transition_goal(&goal_id, GoalLifecycleEvent::Resume)?;
                }
                _ => {}
            }
        }

        if !triggered_goal_ids.is_empty() {
            self.last_action = Some(format!(
                "Scheduler triggered goals: {}",
                triggered_goal_ids.join(",")
            ));
            self.last_error = None;
        }
        if !scheduler_errors.is_empty() {
            self.last_error = Some(scheduler_errors.join(" | "));
        }
        Ok(triggered_goal_ids)
    }

    pub fn run_interval_scheduler_tick(
        &mut self,
        now_epoch_seconds: u64,
    ) -> Result<Vec<String>, String> {
        self.run_scheduler_tick(now_epoch_seconds)
    }

    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    fn persist_to_disk(&mut self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create goals dir: {error}"))?;
        }
        let payload = serde_json::to_string_pretty(&self.document)
            .map_err(|error| format!("Failed to serialize goals document: {error}"))?;
        std::fs::write(&self.file_path, payload)
            .map_err(|error| format!("Failed to persist goals document: {error}"))?;
        Ok(())
    }
}

fn transition_target(
    current: GoalLifecycleStatus,
    event: &GoalLifecycleEvent,
) -> Result<(GoalLifecycleStatus, Option<String>, bool), String> {
    match event {
        GoalLifecycleEvent::Queue => match current {
            GoalLifecycleStatus::Draft | GoalLifecycleStatus::Paused => {
                Ok((GoalLifecycleStatus::Queued, None, false))
            }
            _ => Err(format!("Cannot queue goal from {current:?}")),
        },
        GoalLifecycleEvent::StartRun => match current {
            GoalLifecycleStatus::Queued => Ok((GoalLifecycleStatus::Running, None, true)),
            _ => Err(format!("Cannot start run from {current:?}")),
        },
        GoalLifecycleEvent::Pause => match current {
            GoalLifecycleStatus::Queued | GoalLifecycleStatus::Running => {
                Ok((GoalLifecycleStatus::Paused, None, false))
            }
            _ => Err(format!("Cannot pause goal from {current:?}")),
        },
        GoalLifecycleEvent::Resume => match current {
            GoalLifecycleStatus::Paused => Ok((GoalLifecycleStatus::Queued, None, false)),
            _ => Err(format!("Cannot resume goal from {current:?}")),
        },
        GoalLifecycleEvent::Succeed { reason } => match current {
            GoalLifecycleStatus::Queued
            | GoalLifecycleStatus::Running
            | GoalLifecycleStatus::Paused => Ok((
                GoalLifecycleStatus::Succeeded,
                normalize_reason(reason),
                false,
            )),
            _ => Err(format!("Cannot mark success from {current:?}")),
        },
        GoalLifecycleEvent::Fail { reason } => match current {
            GoalLifecycleStatus::Queued
            | GoalLifecycleStatus::Running
            | GoalLifecycleStatus::Paused => Ok((
                GoalLifecycleStatus::Failed,
                Some(require_reason(reason)?),
                false,
            )),
            _ => Err(format!("Cannot fail goal from {current:?}")),
        },
        GoalLifecycleEvent::Abort { reason } => match current {
            GoalLifecycleStatus::Draft
            | GoalLifecycleStatus::Queued
            | GoalLifecycleStatus::Running
            | GoalLifecycleStatus::Paused => Ok((
                GoalLifecycleStatus::Aborted,
                Some(require_reason(reason)?),
                false,
            )),
            _ => Err(format!("Cannot abort goal from {current:?}")),
        },
    }
}

fn require_reason(reason: &str) -> Result<String, String> {
    let normalized = reason.trim();
    if normalized.is_empty() {
        return Err("Lifecycle transition reason cannot be empty".to_string());
    }
    Ok(normalized.to_string())
}

fn normalize_reason(reason: &Option<String>) -> Option<String> {
    reason.as_ref().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn default_goals_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-goals-v1.json")
}

fn local_rollout_cohort() -> Option<String> {
    let explicit = std::env::var("OPENAGENTS_EARNINGS_ROLLOUT_COHORT")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if explicit.is_some() {
        return explicit;
    }

    std::env::var("USER")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn rate_bps(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    numerator
        .saturating_mul(10_000)
        .checked_div(denominator)
        .unwrap_or(0)
        .min(10_000) as u32
}

fn next_schedule_after(kind: &GoalScheduleKind, from_epoch_seconds: u64) -> Result<u64, String> {
    match kind {
        GoalScheduleKind::Manual => Ok(from_epoch_seconds.saturating_add(60)),
        GoalScheduleKind::IntervalSeconds { seconds } => {
            Ok(from_epoch_seconds.saturating_add((*seconds).max(1)))
        }
        GoalScheduleKind::Cron {
            expression,
            timezone,
        } => {
            let timezone = timezone.as_deref().unwrap_or("UTC");
            let spec = parse_cron_expression(expression)?;
            next_cron_run_epoch_seconds(&spec, timezone, from_epoch_seconds)
        }
    }
}

fn os_schedule_spec_from_kind(kind: &GoalScheduleKind) -> OsSchedulerScheduleSpec {
    match kind {
        GoalScheduleKind::Manual => OsSchedulerScheduleSpec::Manual,
        GoalScheduleKind::IntervalSeconds { seconds } => {
            OsSchedulerScheduleSpec::IntervalSeconds { seconds: *seconds }
        }
        GoalScheduleKind::Cron {
            expression,
            timezone,
        } => OsSchedulerScheduleSpec::Cron {
            expression: expression.clone(),
            timezone: timezone.clone(),
        },
    }
}

fn reconcile_goal_os_scheduler(
    goal: &mut GoalRecord,
    now_epoch_seconds: u64,
) -> Result<OsSchedulerReconcileResult, String> {
    let adapter = goal
        .schedule
        .os_adapter
        .adapter
        .or_else(preferred_adapter_for_host)
        .ok_or_else(|| "No supported OS scheduler adapter available on this host".to_string())?;
    let schedule = os_schedule_spec_from_kind(&goal.schedule.kind);
    let reconcile = reconcile_os_scheduler_descriptor(&goal.goal_id, &schedule, adapter)?;
    goal.schedule.os_adapter.adapter = Some(adapter);
    goal.schedule.os_adapter.adapter_job_id = Some(reconcile.adapter_job_id.clone());
    goal.schedule.os_adapter.descriptor_path =
        Some(reconcile.descriptor_path.display().to_string());
    goal.schedule.os_adapter.last_reconciled_epoch_seconds = Some(now_epoch_seconds);
    goal.schedule.os_adapter.reconciliation_marker = Some(reconcile.reconciliation_marker.clone());
    Ok(reconcile)
}

fn now_epoch_seconds() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use openagents_spark::{Balance, PaymentSummary};

    use super::{
        AutopilotGoalsState, GoalAttemptAuditReceipt, GoalConstraints, GoalExecutionReceipt,
        GoalLaborLinkage, GoalLifecycleEvent, GoalLifecycleStatus, GoalMissedRunPolicy,
        GoalObjective, GoalPayoutEvidence, GoalPolicySnapshot, GoalRecord, GoalRetryPolicy,
        GoalRolloutConfig, GoalRolloutHardeningChecklist, GoalRolloutRollbackPolicy,
        GoalRolloutStage, GoalRunAuditReceipt, GoalScheduleConfig, GoalScheduleKind,
        GoalStopCondition, GoalToolInvocationAudit,
    };
    use crate::app_state::{
        JobHistoryReceiptRow, JobHistoryState, JobHistoryStatus, JobHistoryStatusFilter,
        JobHistoryTimeRange, PaneLoadState, SkillRegistryDiscoveredSkill,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::goal_conditions::GoalProgressSnapshot;
    use crate::state::os_scheduler::OsSchedulerAdapterKind;
    use crate::state::swap_contract::{
        GoalSwapExecutionReceipt, SwapAmount, SwapAmountUnit, SwapDirection, SwapExecutionRequest,
        SwapQuoteTerms,
    };
    use crate::state::swap_quote_adapter::{
        StablesatsQuoteClient, StablesatsQuoteFor, StablesatsQuoteResponse,
        SwapQuoteAdapterRequest, SwapQuoteAuditReceipt, SwapQuoteProvider,
    };

    fn sample_goal(id: &str) -> GoalRecord {
        GoalRecord {
            goal_id: id.to_string(),
            title: "Earn +₿1000".to_string(),
            objective: GoalObjective::EarnBitcoin {
                min_wallet_delta_sats: 1_000,
                note: None,
            },
            constraints: GoalConstraints::default(),
            stop_conditions: vec![GoalStopCondition::WalletDeltaSatsAtLeast { sats: 1_000 }],
            retry_policy: GoalRetryPolicy::default(),
            schedule: GoalScheduleConfig::default(),
            lifecycle_status: GoalLifecycleStatus::Queued,
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
            attempt_count: 0,
            last_failure_reason: None,
            terminal_reason: None,
            last_receipt_id: None,
            recovery_replay_pending: false,
        }
    }

    #[test]
    fn goals_state_persists_active_historical_and_receipts() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-{now_nanos}.json"));

        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        let goal = sample_goal("goal-01");
        state
            .upsert_active_goal(goal)
            .expect("upsert active goal should persist");
        state
            .record_receipt(GoalExecutionReceipt {
                receipt_id: "receipt-01".to_string(),
                goal_id: "goal-01".to_string(),
                attempt_index: 1,
                started_at_epoch_seconds: 1_700_000_010,
                finished_at_epoch_seconds: 1_700_000_020,
                lifecycle_status: GoalLifecycleStatus::Running,
                wallet_delta_sats: 500,
                jobs_completed: 1,
                successes: 0,
                errors: 0,
                notes: Some("partial progress".to_string()),
                recovered_from_restart: false,
                policy_snapshot: GoalPolicySnapshot::default(),
                terminal_labor: GoalLaborLinkage::default(),
            })
            .expect("record receipt should persist");
        state
            .archive_goal("goal-01", GoalLifecycleStatus::Succeeded)
            .expect("archive goal should persist");

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert!(reloaded.document.active_goals.is_empty());
        assert_eq!(reloaded.document.historical_goals.len(), 1);
        assert_eq!(
            reloaded.document.historical_goals[0].lifecycle_status,
            GoalLifecycleStatus::Succeeded
        );
        assert_eq!(reloaded.document.receipts.len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn goal_state_machine_enforces_transitions_and_attempt_counter() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-transition-{now_nanos}.json"));

        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-state-machine"))
            .expect("upsert active goal should succeed");

        let running = state
            .transition_goal("goal-state-machine", GoalLifecycleEvent::StartRun)
            .expect("queued->running should be valid");
        assert_eq!(running.to, GoalLifecycleStatus::Running);
        assert_eq!(running.attempt_count, 1);

        let paused = state
            .transition_goal("goal-state-machine", GoalLifecycleEvent::Pause)
            .expect("running->paused should be valid");
        assert_eq!(paused.to, GoalLifecycleStatus::Paused);
        assert_eq!(paused.attempt_count, 1);

        let queued = state
            .transition_goal("goal-state-machine", GoalLifecycleEvent::Resume)
            .expect("paused->queued should be valid");
        assert_eq!(queued.to, GoalLifecycleStatus::Queued);

        let resumed = state
            .transition_goal("goal-state-machine", GoalLifecycleEvent::StartRun)
            .expect("queued->running should be valid again");
        assert_eq!(resumed.attempt_count, 2);

        let failed = state
            .transition_goal(
                "goal-state-machine",
                GoalLifecycleEvent::Fail {
                    reason: "network timeout".to_string(),
                },
            )
            .expect("running->failed should be valid");
        assert_eq!(failed.to, GoalLifecycleStatus::Failed);
        assert_eq!(failed.reason.as_deref(), Some("network timeout"));

        assert!(state.document.active_goals.is_empty());
        assert_eq!(state.document.historical_goals.len(), 1);
        assert_eq!(
            state.document.historical_goals[0].attempt_count, 2,
            "attempt counter should persist in historical record"
        );
        assert_eq!(
            state.document.historical_goals[0]
                .last_failure_reason
                .as_deref(),
            Some("network timeout")
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn goal_state_machine_rejects_invalid_transition() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autopilot-goals-invalid-transition-{now_nanos}.json"
        ));

        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-invalid"))
            .expect("upsert active goal should succeed");
        let error = state
            .transition_goal("goal-invalid", GoalLifecycleEvent::Resume)
            .expect_err("queued->resume should be invalid");
        assert!(
            error.contains("Cannot resume goal from"),
            "unexpected transition error: {error}"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn evaluate_active_goal_conditions_by_goal_id() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-eval-{now_nanos}.json"));

        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-eval-id"))
            .expect("upsert active goal should succeed");

        let evaluation = state
            .evaluate_active_goal_conditions(
                "goal-eval-id",
                &GoalProgressSnapshot {
                    started_at_epoch_seconds: 1_700_000_000,
                    now_epoch_seconds: 1_700_000_010,
                    attempt_count: 1,
                    wallet_delta_sats: 1_100,
                    earned_wallet_delta_sats: 1_100,
                    jobs_completed: 0,
                    successes: 0,
                    errors: 0,
                    total_spend_sats: 0,
                    total_swap_cents: 0,
                    external_signals: std::collections::BTreeMap::new(),
                },
            )
            .expect("goal evaluator should succeed");
        assert!(evaluation.goal_complete);
        assert!(!evaluation.should_continue);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn authoritative_earnings_gate_rejects_synthetic_history_payout() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-gate-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-gate"))
            .expect("upsert active goal should succeed");

        let progress = GoalProgressSnapshot {
            started_at_epoch_seconds: 1_700_000_000,
            now_epoch_seconds: 1_700_000_010,
            attempt_count: 1,
            wallet_delta_sats: 1_100,
            earned_wallet_delta_sats: 1_100,
            jobs_completed: 1,
            successes: 1,
            errors: 0,
            total_spend_sats: 0,
            total_swap_cents: 0,
            external_signals: std::collections::BTreeMap::new(),
        };
        let history = JobHistoryState {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: None,
            rows: vec![JobHistoryReceiptRow {
                job_id: "job-1".to_string(),
                status: JobHistoryStatus::Succeeded,
                demand_source: crate::app_state::JobDemandSource::OpenNetwork,
                completed_at_epoch_seconds: 1_700_000_008,
                requester_nostr_pubkey: Some("npub1buyer".to_string()),
                provider_nostr_pubkey: Some("npub1provider".to_string()),
                skill_scope_id: None,
                skl_manifest_a: None,
                skl_manifest_event_id: None,
                sa_tick_result_event_id: None,
                sa_trajectory_session_id: None,
                ac_envelope_event_id: None,
                ac_settlement_event_id: None,
                ac_default_event_id: None,
                delivery_proof_id: None,
                delivery_metering_rule_id: None,
                delivery_proof_status_label: None,
                delivery_metered_quantity: None,
                delivery_accepted_quantity: None,
                delivery_variance_reason_label: None,
                delivery_rejection_reason_label: None,
                payout_sats: 1_000,
                result_hash: "hash-1".to_string(),
                payment_pointer: "pay:starter-job-1".to_string(),
                failure_reason: None,
                execution_provenance: None,
            }],
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 6,
            search_job_id: String::new(),
            reference_epoch_seconds: 1_700_000_010,
        };
        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(Balance {
            spark_sats: 2_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        wallet.recent_payments.push(PaymentSummary {
            id: "wallet-payment-1".to_string(),
            direction: "receive".to_string(),
            status: "succeeded".to_string(),
            amount_sats: 1_000,
            timestamp: 1_700_000_009,
            ..Default::default()
        });

        let report = state
            .verify_authoritative_earnings_gate("goal-gate", &progress, &history, &wallet)
            .expect("earnings gate should evaluate");
        assert!(!report.authoritative_goal_complete);
        assert!(
            report
                .mismatches
                .iter()
                .any(|mismatch| mismatch.contains("synthetic payout pointer"))
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn validate_swap_request_policy_by_goal_id() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-swap-request-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-swap-request"))
            .expect("upsert active goal should succeed");

        let valid_request = SwapExecutionRequest {
            request_id: "swap-request-1".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            quote_ttl_seconds: 30,
            immediate_execution: true,
            max_fee_sats_override: Some(500),
            max_slippage_bps_override: Some(50),
        };
        state
            .validate_swap_request_policy("goal-swap-request", &valid_request, 10_000, 0)
            .expect("valid swap request should pass policy");

        let invalid_request = SwapExecutionRequest {
            request_id: "swap-request-2".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: GoalConstraints::default()
                    .swap_policy
                    .max_per_swap_sats
                    .saturating_add(1),
                unit: SwapAmountUnit::Sats,
            },
            quote_ttl_seconds: 30,
            immediate_execution: true,
            max_fee_sats_override: None,
            max_slippage_bps_override: None,
        };
        let error = state
            .validate_swap_request_policy("goal-swap-request", &invalid_request, 0, 0)
            .expect_err("request above per-swap limit should fail");
        assert!(error.contains("exceeds per-swap limit"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn validate_swap_quote_policy_by_goal_id() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-swap-quote-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-swap-quote"))
            .expect("upsert active goal should succeed");

        let quote = SwapQuoteTerms {
            quote_id: "quote-1".to_string(),
            direction: SwapDirection::UsdToBtc,
            amount_in: SwapAmount {
                amount: 100_00,
                unit: SwapAmountUnit::Cents,
            },
            amount_out: SwapAmount {
                amount: 3_200,
                unit: SwapAmountUnit::Sats,
            },
            expires_at_epoch_seconds: 500,
            immediate_execution: false,
            fee_sats: 100,
            fee_bps: 15,
            slippage_bps: 20,
        };
        state
            .validate_swap_quote_policy("goal-swap-quote", &quote, 400)
            .expect("valid quote should pass policy");

        let expired_error = state
            .validate_swap_quote_policy("goal-swap-quote", &quote, 500)
            .expect_err("expired quote should fail");
        assert!(expired_error.contains("expired"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn record_swap_quote_audit_persists_quote_metadata_for_replay() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-swap-audit-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-swap-audit"))
            .expect("upsert active goal should succeed");

        state
            .record_swap_quote_audit(SwapQuoteAuditReceipt {
                audit_id: "swap-quote-audit-1".to_string(),
                goal_id: "goal-swap-audit".to_string(),
                request_id: "req-1".to_string(),
                provider: SwapQuoteProvider::StablesatsQuoteService,
                quote_id: "quote-1".to_string(),
                direction: SwapDirection::BtcToUsd,
                amount_in: SwapAmount {
                    amount: 5_000,
                    unit: SwapAmountUnit::Sats,
                },
                amount_out: SwapAmount {
                    amount: 330,
                    unit: SwapAmountUnit::Cents,
                },
                fee_sats: 25,
                expires_at_epoch_seconds: 1_700_000_100,
                immediate_execution: false,
                executed: false,
                accepted_via_adapter: false,
                fallback_reason: None,
                created_at_epoch_seconds: 1_700_000_000,
                command_provenance: None,
            })
            .expect("swap quote audit should persist");

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert_eq!(reloaded.document.swap_quote_audits.len(), 1);
        assert_eq!(reloaded.document.swap_quote_audits[0].quote_id, "quote-1");
        assert_eq!(reloaded.document.swap_quote_audits[0].fee_sats, 25);
        assert_eq!(
            reloaded.document.swap_quote_audits[0].expires_at_epoch_seconds,
            1_700_000_100
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn record_swap_execution_receipt_persists_goal_scoped_swap_evidence() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-swap-exec-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-swap-exec"))
            .expect("upsert active goal should succeed");

        state
            .record_swap_execution_receipt(GoalSwapExecutionReceipt {
                receipt_id: "swap-exec-1".to_string(),
                goal_id: "goal-swap-exec".to_string(),
                quote_id: "quote-exec-1".to_string(),
                direction: SwapDirection::BtcToUsd,
                amount_in: SwapAmount {
                    amount: 2_500,
                    unit: SwapAmountUnit::Sats,
                },
                amount_out: SwapAmount {
                    amount: 180,
                    unit: SwapAmountUnit::Cents,
                },
                fee_sats: 7,
                status: crate::state::swap_contract::SwapExecutionStatus::Success,
                transaction_id: Some("swap-tx-1".to_string()),
                failure_reason: None,
                started_at_epoch_seconds: 1_700_000_000,
                finished_at_epoch_seconds: 1_700_000_030,
                command_provenance: None,
            })
            .expect("swap execution receipt should persist");

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert_eq!(reloaded.document.swap_execution_receipts.len(), 1);
        assert_eq!(
            reloaded.document.swap_execution_receipts[0].quote_id,
            "quote-exec-1"
        );
        assert_eq!(reloaded.document.swap_execution_receipts[0].fee_sats, 7);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn record_run_audit_receipt_persists_attempts_and_evidence() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-run-audit-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-run-audit"))
            .expect("upsert active goal should succeed");

        state
            .record_run_audit_receipt(GoalRunAuditReceipt {
                audit_id: "run-audit-1".to_string(),
                receipt_id: "goal-loop-receipt-goal-run-audit-1".to_string(),
                goal_id: "goal-run-audit".to_string(),
                run_id: "goal-run-goal-run-audit-1".to_string(),
                started_at_epoch_seconds: 1_700_000_000,
                finished_at_epoch_seconds: 1_700_000_120,
                lifecycle_status: GoalLifecycleStatus::Succeeded,
                terminal_status_reason: "goal conditions satisfied".to_string(),
                selected_skills: vec!["blink".to_string(), "l402".to_string()],
                terminal_labor: GoalLaborLinkage {
                    work_unit_id: Some("work-unit-1".to_string()),
                    contract_id: Some("contract-1".to_string()),
                    verdict_id: Some("verdict-1".to_string()),
                    ..GoalLaborLinkage::default()
                },
                attempts: vec![GoalAttemptAuditReceipt {
                    attempt_index: 1,
                    submitted_at_epoch_seconds: 1_700_000_005,
                    finished_at_epoch_seconds: Some(1_700_000_040),
                    thread_id: Some("thread-1".to_string()),
                    turn_id: Some("turn-1".to_string()),
                    selected_skills: vec!["blink".to_string()],
                    turn_status: Some("completed".to_string()),
                    error: None,
                    condition_goal_complete: Some(true),
                    condition_should_continue: Some(false),
                    condition_completion_reasons: vec!["wallet delta reached ₿1000".to_string()],
                    condition_stop_reasons: Vec::new(),
                    labor: GoalLaborLinkage {
                        work_unit_id: Some("work-unit-1".to_string()),
                        contract_id: Some("contract-1".to_string()),
                        submission_id: Some("submission-1".to_string()),
                        verdict_id: Some("verdict-1".to_string()),
                        settlement_ready: Some(true),
                        ..GoalLaborLinkage::default()
                    },
                    tool_invocations: vec![GoalToolInvocationAudit {
                        request_id: "req-1".to_string(),
                        call_id: "call-1".to_string(),
                        tool_name: "openagents_wallet_check".to_string(),
                        response_code: "OA-WALLET-CHECK-OK".to_string(),
                        success: true,
                        response_message: "wallet snapshot collected".to_string(),
                        recorded_at_epoch_seconds: 1_700_000_030,
                        evidence_refs: Vec::new(),
                    }],
                }],
                condition_goal_complete: Some(true),
                condition_should_continue: Some(false),
                condition_completion_reasons: vec!["wallet delta reached ₿1000".to_string()],
                condition_stop_reasons: Vec::new(),
                payout_evidence: vec![GoalPayoutEvidence {
                    event_id: "earn:job-1:wallet:pay:job-1".to_string(),
                    occurred_at_epoch_seconds: 1_700_000_090,
                    job_id: "job-1".to_string(),
                    payment_pointer: "wallet:pay:job-1".to_string(),
                    payout_sats: 1_000,
                    attempt_index: Some(1),
                    turn_id: Some("turn-1".to_string()),
                    labor: GoalLaborLinkage {
                        work_unit_id: Some("work-unit-1".to_string()),
                        contract_id: Some("contract-1".to_string()),
                        verdict_id: Some("verdict-1".to_string()),
                        settlement_id: Some("settlement-1".to_string()),
                        ..GoalLaborLinkage::default()
                    },
                }],
                swap_quote_evidence: Vec::new(),
                swap_execution_evidence: Vec::new(),
            })
            .expect("run audit receipt should persist");

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert_eq!(reloaded.document.run_audit_receipts.len(), 1);
        assert_eq!(reloaded.document.run_audit_receipts[0].attempts.len(), 1);
        assert_eq!(
            reloaded.document.run_audit_receipts[0].selected_skills,
            vec!["blink".to_string(), "l402".to_string()]
        );
        assert_eq!(
            reloaded.document.run_audit_receipts[0]
                .payout_evidence
                .len(),
            1
        );

        let _ = std::fs::remove_file(path);
    }

    #[derive(Default)]
    struct GoalsTestStablesatsClient {
        buy_quote: Option<Result<StablesatsQuoteResponse, String>>,
    }

    impl StablesatsQuoteClient for GoalsTestStablesatsClient {
        fn get_quote_to_buy_usd(
            &mut self,
            _quote_for: StablesatsQuoteFor,
            _immediate_execution: bool,
        ) -> Result<StablesatsQuoteResponse, String> {
            self.buy_quote
                .clone()
                .unwrap_or_else(|| Err("missing buy quote".to_string()))
        }

        fn get_quote_to_sell_usd(
            &mut self,
            _quote_for: StablesatsQuoteFor,
            _immediate_execution: bool,
        ) -> Result<StablesatsQuoteResponse, String> {
            Err("unused in this test".to_string())
        }

        fn accept_quote(&mut self, _quote_id: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn request_swap_quote_with_adapter_records_audit_and_fallback() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autopilot-goals-swap-adapter-request-{now_nanos}.json"
        ));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-swap-adapter"))
            .expect("upsert active goal should succeed");

        let request = SwapQuoteAdapterRequest {
            request_id: "req-swap-adapter".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            immediate_execution: false,
            now_epoch_seconds: 1_700_000_000,
        };
        let fallback_quote = SwapQuoteTerms {
            quote_id: "fallback-quote-1".to_string(),
            direction: SwapDirection::BtcToUsd,
            amount_in: SwapAmount {
                amount: 5_000,
                unit: SwapAmountUnit::Sats,
            },
            amount_out: SwapAmount {
                amount: 330,
                unit: SwapAmountUnit::Cents,
            },
            expires_at_epoch_seconds: 1_700_000_100,
            immediate_execution: false,
            fee_sats: 0,
            fee_bps: 0,
            slippage_bps: 0,
        };
        let mut stablesats_client = GoalsTestStablesatsClient {
            buy_quote: Some(Err("stablesats path unavailable".to_string())),
        };

        let outcome = state
            .request_swap_quote_with_adapter(
                "goal-swap-adapter",
                &request,
                &mut stablesats_client,
                fallback_quote.clone(),
            )
            .expect("adapter call should complete with fallback");
        assert_eq!(outcome.quote.quote_id, fallback_quote.quote_id);
        assert_eq!(outcome.provider, SwapQuoteProvider::BlinkFallback);

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert_eq!(reloaded.document.swap_quote_audits.len(), 1);
        assert_eq!(
            reloaded.document.swap_quote_audits[0].quote_id,
            fallback_quote.quote_id
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolve_skill_candidates_for_goal_is_ranked_and_reasoned() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-skill-resolve-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-skill-resolve"))
            .expect("upsert active goal should succeed");

        let discovered = vec![
            SkillRegistryDiscoveredSkill {
                name: "moneydevkit".to_string(),
                path: "/repo/skills/moneydevkit/SKILL.md".to_string(),
                scope: "user".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
            SkillRegistryDiscoveredSkill {
                name: "blink".to_string(),
                path: "/repo/skills/blink/SKILL.md".to_string(),
                scope: "user".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
            SkillRegistryDiscoveredSkill {
                name: "l402".to_string(),
                path: "/repo/skills/l402/SKILL.md".to_string(),
                scope: "user".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
            SkillRegistryDiscoveredSkill {
                name: "neutronpay".to_string(),
                path: "/repo/skills/neutronpay/SKILL.md".to_string(),
                scope: "user".to_string(),
                enabled: true,
                interface_display_name: None,
                dependency_count: 0,
            },
        ];

        let resolution = state
            .resolve_skill_candidates_for_goal("goal-skill-resolve", &discovered)
            .expect("goal skill resolver should succeed");
        let ordered = resolution
            .candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["blink", "l402", "moneydevkit", "neutronpay"]);
        assert!(
            resolution
                .candidates
                .iter()
                .all(|candidate| !candidate.reason.trim().is_empty())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn interval_schedule_persists_and_triggers_queue_transition() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autopilot-goals-interval-schedule-{now_nanos}.json"
        ));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-interval"))
            .expect("upsert active goal should succeed");

        state
            .set_goal_interval_schedule("goal-interval", 15, 1_700_000_000)
            .expect("setting interval schedule should succeed");
        let configured_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-interval")
            .expect("goal must exist");
        assert!(configured_goal.schedule.enabled);
        assert_eq!(
            configured_goal.schedule.next_run_epoch_seconds,
            Some(1_700_000_015)
        );

        let triggered = state
            .run_interval_scheduler_tick(1_700_000_020)
            .expect("scheduler tick should succeed");
        assert_eq!(triggered, vec!["goal-interval".to_string()]);
        let queued_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-interval")
            .expect("goal must still be active");
        assert_eq!(queued_goal.lifecycle_status, GoalLifecycleStatus::Queued);
        assert_eq!(
            queued_goal.schedule.last_run_epoch_seconds,
            Some(1_700_000_020)
        );
        assert_eq!(
            queued_goal.schedule.next_run_epoch_seconds,
            Some(1_700_000_035)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn schedule_goal_run_now_updates_last_run_and_queues_draft_goal() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-schedule-now-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-manual-now"))
            .expect("upsert active goal should succeed");

        state
            .schedule_goal_run_now("goal-manual-now", 1_700_000_100)
            .expect("manual run scheduling should succeed");
        let queued_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-manual-now")
            .expect("goal must exist");
        assert_eq!(queued_goal.lifecycle_status, GoalLifecycleStatus::Queued);
        assert_eq!(
            queued_goal.schedule.last_run_epoch_seconds,
            Some(1_700_000_100)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn set_goal_cron_schedule_persists_preview_and_timezone() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("autopilot-goals-cron-set-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-cron-set"))
            .expect("upsert active goal should succeed");

        state
            .set_goal_cron_schedule("goal-cron-set", "*/5 * * * *", "UTC", 1_700_000_000)
            .expect("cron schedule should be accepted");
        let goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-cron-set")
            .expect("goal should exist");
        assert!(goal.schedule.enabled);
        assert!(matches!(
            goal.schedule.kind,
            GoalScheduleKind::Cron { ref expression, .. } if expression == "*/5 * * * *"
        ));
        assert!(
            goal.schedule
                .next_run_epoch_seconds
                .is_some_and(|next| next > 1_700_000_000),
            "next cron run should be computed in the future"
        );

        let cron_error = state
            .set_goal_cron_schedule("goal-cron-set", "invalid cron", "UTC", 1_700_000_000)
            .expect_err("invalid cron must fail");
        assert!(cron_error.contains("must have 5 fields"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn scheduler_tick_triggers_due_cron_goal_and_sets_next_preview() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-cron-trigger-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-cron-trigger"))
            .expect("upsert active goal should succeed");
        state
            .set_goal_cron_schedule("goal-cron-trigger", "* * * * *", "UTC", 1_700_000_000)
            .expect("cron schedule should be accepted");

        {
            let goal = state
                .document
                .active_goals
                .iter_mut()
                .find(|goal| goal.goal_id == "goal-cron-trigger")
                .expect("goal should exist");
            goal.schedule.next_run_epoch_seconds = Some(1_700_000_060);
        }

        let triggered = state
            .run_scheduler_tick(1_700_000_060)
            .expect("scheduler tick should succeed");
        assert_eq!(triggered, vec!["goal-cron-trigger".to_string()]);

        let goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-cron-trigger")
            .expect("goal should remain active");
        assert_eq!(goal.lifecycle_status, GoalLifecycleStatus::Queued);
        assert_eq!(goal.schedule.last_run_epoch_seconds, Some(1_700_000_060));
        assert!(
            goal.schedule
                .next_run_epoch_seconds
                .is_some_and(|next| next > 1_700_000_060)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn recover_after_restart_applies_running_and_missed_run_semantics() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-restart-recovery-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());

        let mut running = sample_goal("goal-running-recover");
        running.lifecycle_status = GoalLifecycleStatus::Running;
        running.schedule.enabled = true;
        running.schedule.kind = GoalScheduleKind::IntervalSeconds { seconds: 300 };
        running.schedule.next_run_epoch_seconds = Some(1_700_001_000);
        state
            .upsert_active_goal(running)
            .expect("running goal should persist");

        let mut skip = sample_goal("goal-skip-missed");
        skip.lifecycle_status = GoalLifecycleStatus::Draft;
        skip.schedule.enabled = true;
        skip.schedule.kind = GoalScheduleKind::IntervalSeconds { seconds: 120 };
        skip.schedule.next_run_epoch_seconds = Some(1_700_000_000);
        skip.schedule.missed_run_policy = GoalMissedRunPolicy::Skip;
        state
            .upsert_active_goal(skip)
            .expect("skip goal should persist");

        let mut replay = sample_goal("goal-single-replay");
        replay.lifecycle_status = GoalLifecycleStatus::Draft;
        replay.schedule.enabled = true;
        replay.schedule.kind = GoalScheduleKind::IntervalSeconds { seconds: 120 };
        replay.schedule.next_run_epoch_seconds = Some(1_700_000_000);
        replay.schedule.missed_run_policy = GoalMissedRunPolicy::SingleReplay;
        state
            .upsert_active_goal(replay)
            .expect("single replay goal should persist");

        let mut catchup = sample_goal("goal-catchup");
        catchup.lifecycle_status = GoalLifecycleStatus::Paused;
        catchup.schedule.enabled = true;
        catchup.schedule.kind = GoalScheduleKind::IntervalSeconds { seconds: 60 };
        catchup.schedule.next_run_epoch_seconds = Some(1_700_000_000);
        catchup.schedule.missed_run_policy = GoalMissedRunPolicy::CatchUp;
        state
            .upsert_active_goal(catchup)
            .expect("catchup goal should persist");

        let report = state
            .recover_after_restart(1_700_000_300)
            .expect("restart recovery should succeed");
        assert!(
            report
                .recovered_running_goals
                .contains(&"goal-running-recover".to_string())
        );
        assert!(
            report
                .replay_queued_goals
                .contains(&"goal-single-replay".to_string())
        );
        assert!(
            report
                .skipped_goals
                .contains(&"goal-skip-missed".to_string())
        );
        assert!(
            report
                .catchup_backlog
                .iter()
                .any(|(goal_id, missed)| goal_id == "goal-catchup" && *missed > 1)
        );

        let running_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-running-recover")
            .expect("running goal should still exist");
        assert_eq!(running_goal.lifecycle_status, GoalLifecycleStatus::Queued);
        assert!(running_goal.recovery_replay_pending);

        let skip_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-skip-missed")
            .expect("skip goal should still exist");
        assert_eq!(skip_goal.lifecycle_status, GoalLifecycleStatus::Draft);
        assert!(!skip_goal.recovery_replay_pending);
        assert!(
            skip_goal
                .schedule
                .next_run_epoch_seconds
                .is_some_and(|next| next > 1_700_000_300)
        );

        let replay_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-single-replay")
            .expect("replay goal should still exist");
        assert_eq!(replay_goal.lifecycle_status, GoalLifecycleStatus::Queued);
        assert!(replay_goal.recovery_replay_pending);

        let catchup_goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-catchup")
            .expect("catchup goal should still exist");
        assert!(catchup_goal.recovery_replay_pending);
        assert!(catchup_goal.schedule.pending_catchup_runs > 0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn consume_recovery_replay_flag_clears_pending_marker() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autopilot-goals-consume-recovery-flag-{now_nanos}.json"
        ));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        let mut goal = sample_goal("goal-consume-recovery");
        goal.recovery_replay_pending = true;
        state.upsert_active_goal(goal).expect("goal should persist");

        assert!(
            state
                .consume_recovery_replay_flag("goal-consume-recovery")
                .expect("first consume should succeed")
        );
        assert!(
            !state
                .consume_recovery_replay_flag("goal-consume-recovery")
                .expect("second consume should succeed")
        );

        let refreshed = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-consume-recovery")
            .expect("goal should remain active");
        assert!(!refreshed.recovery_replay_pending);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn set_goal_os_scheduler_adapter_disables_and_records_marker() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autopilot-goals-os-adapter-disable-{now_nanos}.json"
        ));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-os-disable"))
            .expect("upsert active goal should succeed");

        state
            .set_goal_os_scheduler_adapter(
                "goal-os-disable",
                false,
                Some(OsSchedulerAdapterKind::Cron),
                1_700_000_000,
            )
            .expect("disabling adapter should succeed");
        let goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-os-disable")
            .expect("goal should exist");
        assert!(!goal.schedule.os_adapter.enabled);
        assert_eq!(
            goal.schedule.os_adapter.adapter,
            Some(OsSchedulerAdapterKind::Cron)
        );
        assert_eq!(
            goal.schedule.os_adapter.last_reconciled_epoch_seconds,
            Some(1_700_000_000)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn set_goal_os_scheduler_adapter_rejects_unsupported_adapter() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autopilot-goals-os-adapter-unsupported-{now_nanos}.json"
        ));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-os-unsupported"))
            .expect("upsert active goal should succeed");

        let unsupported_adapter = if cfg!(target_os = "macos") {
            OsSchedulerAdapterKind::Systemd
        } else {
            OsSchedulerAdapterKind::Launchd
        };
        let error = state
            .set_goal_os_scheduler_adapter(
                "goal-os-unsupported",
                true,
                Some(unsupported_adapter),
                1_700_000_000,
            )
            .expect_err("unsupported adapter should fail");
        assert!(
            error.contains("only available") || error.contains("not found"),
            "unexpected error: {error}"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn policy_snapshot_captures_budget_scope_and_swap_limits() {
        let mut goal = sample_goal("goal-policy-snapshot");
        goal.constraints.max_runtime_seconds = 90;
        goal.constraints.max_attempts = 4;
        goal.constraints.max_total_spend_sats = Some(3_000);
        goal.constraints.max_total_swap_cents = Some(42_000);
        goal.constraints.swap_policy.max_per_swap_sats = 15_000;
        goal.constraints.swap_policy.max_per_swap_cents = 200_000;
        goal.constraints.swap_policy.max_daily_converted_sats = 120_000;
        goal.constraints.swap_policy.max_daily_converted_cents = 1_500_000;
        goal.constraints.swap_policy.max_fee_sats = 120;
        goal.constraints.swap_policy.max_slippage_bps = 25;
        goal.constraints.swap_policy.require_quote_confirmation = false;
        goal.constraints.autonomy_policy.allowed_command_prefixes =
            vec!["openagents_goal_".to_string()];
        goal.constraints.autonomy_policy.allowed_file_roots =
            vec!["/tmp/openagents-goal".to_string()];
        goal.constraints.autonomy_policy.kill_switch_active = true;
        goal.constraints.autonomy_policy.kill_switch_reason = Some("operator stop".to_string());

        let snapshot = goal.constraints.policy_snapshot();
        assert_eq!(snapshot.max_runtime_seconds, 90);
        assert_eq!(snapshot.max_attempts, 4);
        assert_eq!(snapshot.max_total_spend_sats, Some(3_000));
        assert_eq!(snapshot.max_total_swap_cents, Some(42_000));
        assert_eq!(snapshot.max_per_swap_sats, 15_000);
        assert_eq!(snapshot.max_per_swap_cents, 200_000);
        assert_eq!(snapshot.max_daily_converted_sats, 120_000);
        assert_eq!(snapshot.max_daily_converted_cents, 1_500_000);
        assert_eq!(snapshot.max_fee_sats, 120);
        assert_eq!(snapshot.max_slippage_bps, 25);
        assert!(!snapshot.require_quote_confirmation);
        assert_eq!(
            snapshot.allowed_command_prefixes,
            vec!["openagents_goal_".to_string()]
        );
        assert_eq!(
            snapshot.allowed_file_roots,
            vec!["/tmp/openagents-goal".to_string()]
        );
        assert!(snapshot.kill_switch_active);
        assert_eq!(
            snapshot.kill_switch_reason.as_deref(),
            Some("operator stop")
        );
    }

    #[test]
    fn set_goal_kill_switch_updates_and_clears_reason() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-kill-switch-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-kill-switch"))
            .expect("upsert active goal should succeed");

        state
            .set_goal_kill_switch(
                "goal-kill-switch",
                true,
                Some("manual safety stop"),
                1_700_000_100,
            )
            .expect("enable kill switch should succeed");
        let goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-kill-switch")
            .expect("goal should exist");
        assert!(goal.constraints.autonomy_policy.kill_switch_active);
        assert_eq!(
            goal.constraints
                .autonomy_policy
                .kill_switch_reason
                .as_deref(),
            Some("manual safety stop")
        );

        state
            .set_goal_kill_switch("goal-kill-switch", false, None, 1_700_000_200)
            .expect("disable kill switch should succeed");
        let goal = state
            .document
            .active_goals
            .iter()
            .find(|goal| goal.goal_id == "goal-kill-switch")
            .expect("goal should exist");
        assert!(!goal.constraints.autonomy_policy.kill_switch_active);
        assert!(
            goal.constraints
                .autonomy_policy
                .kill_switch_reason
                .is_none()
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rollout_gate_respects_feature_flag_stage_and_cohorts() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-rollout-gate-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());

        let disabled = state.rollout_gate_decision();
        assert!(!disabled.enabled);
        assert!(disabled.reason.contains("feature flag disabled"));

        state
            .update_rollout_config(
                Some(true),
                Some(GoalRolloutStage::InternalDogfood),
                Some(vec!["*".to_string()]),
                None,
                None,
                1_700_000_100,
            )
            .expect("rollout update should persist");
        let wildcard = state.rollout_gate_decision();
        assert!(wildcard.enabled);
        assert_eq!(wildcard.stage, GoalRolloutStage::InternalDogfood);

        state
            .update_rollout_config(
                Some(true),
                Some(GoalRolloutStage::Canary),
                Some(vec!["cohort-not-present".to_string()]),
                None,
                None,
                1_700_000_200,
            )
            .expect("rollout update should persist");
        let restricted = state.rollout_gate_decision();
        assert!(!restricted.enabled);
        assert!(restricted.reason.contains("not allowlisted"));

        state
            .update_rollout_config(
                Some(true),
                Some(GoalRolloutStage::GeneralAvailability),
                None,
                None,
                None,
                1_700_000_300,
            )
            .expect("rollout update should persist");
        let ga = state.rollout_gate_decision();
        assert!(ga.enabled);
        assert_eq!(ga.stage, GoalRolloutStage::GeneralAvailability);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rollout_metrics_and_health_capture_false_success_latency_and_abort_distribution() {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("autopilot-goals-rollout-metrics-{now_nanos}.json"));
        let mut state = AutopilotGoalsState::load_from_path(path.clone());
        state
            .upsert_active_goal(sample_goal("goal-rollout-metrics"))
            .expect("upsert active goal should succeed");

        state
            .record_run_audit_receipt(GoalRunAuditReceipt {
                audit_id: "rollout-audit-1".to_string(),
                receipt_id: "goal-loop-receipt-goal-rollout-metrics-1".to_string(),
                goal_id: "goal-rollout-metrics".to_string(),
                run_id: "run-1".to_string(),
                started_at_epoch_seconds: 1_700_000_000,
                finished_at_epoch_seconds: 1_700_000_050,
                lifecycle_status: GoalLifecycleStatus::Succeeded,
                terminal_status_reason: "goal conditions satisfied".to_string(),
                selected_skills: vec!["blink".to_string()],
                terminal_labor: GoalLaborLinkage::default(),
                attempts: vec![GoalAttemptAuditReceipt {
                    attempt_index: 1,
                    submitted_at_epoch_seconds: 1_700_000_001,
                    finished_at_epoch_seconds: Some(1_700_000_040),
                    thread_id: Some("thread-1".to_string()),
                    turn_id: Some("turn-1".to_string()),
                    selected_skills: vec!["blink".to_string()],
                    turn_status: Some("completed".to_string()),
                    error: None,
                    condition_goal_complete: Some(true),
                    condition_should_continue: Some(false),
                    condition_completion_reasons: vec!["wallet delta reached ₿1000".to_string()],
                    condition_stop_reasons: Vec::new(),
                    labor: GoalLaborLinkage::default(),
                    tool_invocations: Vec::new(),
                }],
                condition_goal_complete: Some(true),
                condition_should_continue: Some(false),
                condition_completion_reasons: vec!["wallet delta reached ₿1000".to_string()],
                condition_stop_reasons: Vec::new(),
                payout_evidence: Vec::new(),
                swap_quote_evidence: Vec::new(),
                swap_execution_evidence: Vec::new(),
            })
            .expect("false-success run should persist");

        state
            .record_run_audit_receipt(GoalRunAuditReceipt {
                audit_id: "rollout-audit-2".to_string(),
                receipt_id: "goal-loop-receipt-goal-rollout-metrics-2".to_string(),
                goal_id: "goal-rollout-metrics".to_string(),
                run_id: "run-2".to_string(),
                started_at_epoch_seconds: 1_700_000_100,
                finished_at_epoch_seconds: 1_700_000_180,
                lifecycle_status: GoalLifecycleStatus::Succeeded,
                terminal_status_reason: "goal conditions satisfied".to_string(),
                selected_skills: vec!["blink".to_string(), "l402".to_string()],
                terminal_labor: GoalLaborLinkage::default(),
                attempts: vec![GoalAttemptAuditReceipt {
                    attempt_index: 1,
                    submitted_at_epoch_seconds: 1_700_000_101,
                    finished_at_epoch_seconds: Some(1_700_000_170),
                    thread_id: Some("thread-2".to_string()),
                    turn_id: Some("turn-2".to_string()),
                    selected_skills: vec!["blink".to_string()],
                    turn_status: Some("completed".to_string()),
                    error: Some("transient quote timeout".to_string()),
                    condition_goal_complete: Some(true),
                    condition_should_continue: Some(false),
                    condition_completion_reasons: vec!["wallet delta reached ₿1000".to_string()],
                    condition_stop_reasons: Vec::new(),
                    labor: GoalLaborLinkage::default(),
                    tool_invocations: Vec::new(),
                }],
                condition_goal_complete: Some(true),
                condition_should_continue: Some(false),
                condition_completion_reasons: vec!["wallet delta reached ₿1000".to_string()],
                condition_stop_reasons: Vec::new(),
                payout_evidence: vec![GoalPayoutEvidence {
                    event_id: "earn:job-2:wallet:pay:job-2".to_string(),
                    occurred_at_epoch_seconds: 1_700_000_130,
                    job_id: "job-2".to_string(),
                    payment_pointer: "wallet:pay:job-2".to_string(),
                    payout_sats: 1_000,
                    attempt_index: Some(1),
                    turn_id: Some("turn-2".to_string()),
                    labor: GoalLaborLinkage::default(),
                }],
                swap_quote_evidence: Vec::new(),
                swap_execution_evidence: Vec::new(),
            })
            .expect("payout-confirmed success run should persist");

        state
            .record_run_audit_receipt(GoalRunAuditReceipt {
                audit_id: "rollout-audit-3".to_string(),
                receipt_id: "goal-loop-receipt-goal-rollout-metrics-3".to_string(),
                goal_id: "goal-rollout-metrics".to_string(),
                run_id: "run-3".to_string(),
                started_at_epoch_seconds: 1_700_000_200,
                finished_at_epoch_seconds: 1_700_000_220,
                lifecycle_status: GoalLifecycleStatus::Aborted,
                terminal_status_reason: "kill switch engaged".to_string(),
                selected_skills: vec!["blink".to_string()],
                terminal_labor: GoalLaborLinkage::default(),
                attempts: Vec::new(),
                condition_goal_complete: Some(false),
                condition_should_continue: Some(false),
                condition_completion_reasons: Vec::new(),
                condition_stop_reasons: vec!["kill switch engaged".to_string()],
                payout_evidence: Vec::new(),
                swap_quote_evidence: Vec::new(),
                swap_execution_evidence: Vec::new(),
            })
            .expect("aborted run should persist");

        let metrics = state.rollout_metrics_snapshot();
        assert_eq!(metrics.total_runs, 3);
        assert_eq!(metrics.succeeded_runs, 2);
        assert_eq!(metrics.completion_rate_bps, 6_666);
        assert_eq!(metrics.false_success_runs, 1);
        assert_eq!(metrics.false_success_rate_bps, 5_000);
        assert_eq!(metrics.payout_confirmed_success_runs, 1);
        assert_eq!(metrics.avg_payout_confirm_latency_seconds, Some(30));
        assert_eq!(metrics.aborted_runs, 1);
        assert_eq!(metrics.error_attempts, 1);
        assert!(
            metrics
                .abort_error_distribution
                .contains_key("attempt_error")
        );

        let unhealthy = state.rollout_health_report();
        assert!(!unhealthy.healthy);
        assert!(
            unhealthy
                .violations
                .iter()
                .any(|violation| violation.contains("false_success_rate_bps"))
        );

        state
            .update_rollout_config(
                None,
                None,
                None,
                Some(GoalRolloutRollbackPolicy {
                    max_false_success_rate_bps: 6_000,
                    max_abort_rate_bps: 4_000,
                    max_error_rate_bps: 4_000,
                    max_avg_payout_confirm_latency_seconds: 60,
                }),
                Some(GoalRolloutHardeningChecklist {
                    authoritative_payout_gate_validated: true,
                    scheduler_recovery_drills_validated: true,
                    swap_risk_alerting_validated: true,
                    incident_runbook_validated: true,
                    test_matrix_gate_green: true,
                }),
                1_700_000_500,
            )
            .expect("rollout policy update should persist");
        let healthy = state.rollout_health_report();
        assert!(healthy.healthy);

        let reloaded = AutopilotGoalsState::load_from_path(path.clone());
        assert_eq!(
            reloaded.document.rollout_config,
            GoalRolloutConfig {
                feature_flag_enabled: false,
                stage: GoalRolloutStage::Disabled,
                allowed_cohorts: Vec::new(),
                rollback_policy: GoalRolloutRollbackPolicy {
                    max_false_success_rate_bps: 6_000,
                    max_abort_rate_bps: 4_000,
                    max_error_rate_bps: 4_000,
                    max_avg_payout_confirm_latency_seconds: 60,
                },
                hardening_checklist: GoalRolloutHardeningChecklist {
                    authoritative_payout_gate_validated: true,
                    scheduler_recovery_drills_validated: true,
                    swap_risk_alerting_validated: true,
                    incident_runbook_validated: true,
                    test_matrix_gate_green: true,
                },
                last_updated_epoch_seconds: Some(1_700_000_500),
            }
        );

        let _ = std::fs::remove_file(path);
    }
}
