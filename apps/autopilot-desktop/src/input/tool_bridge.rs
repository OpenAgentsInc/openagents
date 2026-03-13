use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use codex_client::{DynamicToolCallOutputContentItem, DynamicToolCallResponse};
use openagents_kernel_core::receipts::EvidenceRef;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app_state::{
    ActivityFeedFilter, AutopilotToolCallRequest, CadBuildFailureClass, PaneKind, RenderState,
    SkillRegistryDiscoveredSkill,
};
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::openagents_dynamic_tools::{
    OPENAGENTS_DYNAMIC_TOOL_NAMES, OPENAGENTS_TOOL_CAD_ACTION, OPENAGENTS_TOOL_CAD_INTENT,
    OPENAGENTS_TOOL_GOAL_SCHEDULER, OPENAGENTS_TOOL_LABOR_CLAIM_DENY,
    OPENAGENTS_TOOL_LABOR_CLAIM_OPEN, OPENAGENTS_TOOL_LABOR_CLAIM_REMEDY,
    OPENAGENTS_TOOL_LABOR_CLAIM_RESOLVE, OPENAGENTS_TOOL_LABOR_CLAIM_REVIEW,
    OPENAGENTS_TOOL_LABOR_CLAIM_STATUS, OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH,
    OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST, OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH,
    OPENAGENTS_TOOL_LABOR_REQUIREMENTS, OPENAGENTS_TOOL_LABOR_SCOPE,
    OPENAGENTS_TOOL_LABOR_SUBMISSION_READY, OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST,
    OPENAGENTS_TOOL_PANE_ACTION, OPENAGENTS_TOOL_PANE_CLOSE, OPENAGENTS_TOOL_PANE_FOCUS,
    OPENAGENTS_TOOL_PANE_LIST, OPENAGENTS_TOOL_PANE_OPEN, OPENAGENTS_TOOL_PANE_SET_INPUT,
    OPENAGENTS_TOOL_PROVIDER_CONTROL, OPENAGENTS_TOOL_SWAP_EXECUTE, OPENAGENTS_TOOL_SWAP_QUOTE,
    OPENAGENTS_TOOL_TREASURY_CONVERT, OPENAGENTS_TOOL_TREASURY_RECEIPT,
    OPENAGENTS_TOOL_TREASURY_TRANSFER, OPENAGENTS_TOOL_WALLET_CHECK,
};
use crate::pane_registry::{enabled_pane_specs, pane_spec, pane_spec_by_command_id};
use crate::pane_system::{
    ActiveJobPaneAction, ActivityFeedPaneAction, AgentProfileStatePaneAction,
    AgentScheduleTickPaneAction, AlertsRecoveryPaneAction, CadDemoPaneAction,
    CastControlPaneAction, CodexAccountPaneAction, CodexAppsPaneAction, CodexConfigPaneAction,
    CodexDiagnosticsPaneAction, CodexLabsPaneAction, CodexMcpPaneAction, CodexModelsPaneAction,
    CredentialsPaneAction, CreditDeskPaneAction, CreditSettlementLedgerPaneAction,
    EarningsScoreboardPaneAction, JobHistoryPaneAction, JobInboxPaneAction,
    LocalInferencePaneAction, NetworkRequestsPaneAction, PaneController, PaneHitAction,
    ProviderControlPaneAction, ReciprocalLoopPaneAction, RelayConnectionsPaneAction,
    SettingsPaneAction, SkillRegistryPaneAction, SkillTrustRevocationPaneAction,
    StarterJobsPaneAction, SyncHealthPaneAction, TrajectoryAuditPaneAction,
};
use crate::runtime_lanes::SaLifecycleCommand;
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::spark_wallet::SparkWalletCommand;
use crate::state::autopilot_goals::{
    GoalMissedRunPolicy, GoalRolloutHardeningChecklist, GoalRolloutRollbackPolicy, GoalRolloutStage,
};
use crate::state::os_scheduler::{OsSchedulerAdapterKind, preferred_adapter_for_host};
use crate::state::swap_contract::{
    SwapAmount, SwapAmountUnit, SwapCommandProvenance, SwapDirection, SwapExecutionStatus,
    SwapQuoteTerms,
};
use crate::state::swap_quote_adapter::SwapQuoteProvider;
use crate::state::wallet_reconciliation::reconcile_wallet_events_for_goal;

const LEGACY_OPENAGENTS_TOOL_PANE_LIST: &str = "openagents.pane.list";
const LEGACY_OPENAGENTS_TOOL_PANE_OPEN: &str = "openagents.pane.open";
const LEGACY_OPENAGENTS_TOOL_PANE_FOCUS: &str = "openagents.pane.focus";
const LEGACY_OPENAGENTS_TOOL_PANE_CLOSE: &str = "openagents.pane.close";
const LEGACY_OPENAGENTS_TOOL_PANE_SET_INPUT: &str = "openagents.pane.set_input";
const LEGACY_OPENAGENTS_TOOL_PANE_ACTION: &str = "openagents.pane.action";
const LEGACY_OPENAGENTS_TOOL_CAD_INTENT: &str = "openagents.cad.intent";
const LEGACY_OPENAGENTS_TOOL_CAD_ACTION: &str = "openagents.cad.action";
const LEGACY_OPENAGENTS_TOOL_SWAP_QUOTE: &str = "openagents.swap.quote";
const LEGACY_OPENAGENTS_TOOL_SWAP_EXECUTE: &str = "openagents.swap.execute";
const LEGACY_OPENAGENTS_TOOL_TREASURY_TRANSFER: &str = "openagents.treasury.transfer";
const LEGACY_OPENAGENTS_TOOL_TREASURY_CONVERT: &str = "openagents.treasury.convert";
const LEGACY_OPENAGENTS_TOOL_TREASURY_RECEIPT: &str = "openagents.treasury.receipt";
const LEGACY_OPENAGENTS_TOOL_GOAL_SCHEDULER: &str = "openagents.goal.scheduler";
const LEGACY_OPENAGENTS_TOOL_WALLET_CHECK: &str = "openagents.wallet.check";
const LEGACY_OPENAGENTS_TOOL_PROVIDER_CONTROL: &str = "openagents.provider.control";
const LEGACY_OPENAGENTS_TOOL_LABOR_SCOPE: &str = "openagents.labor.scope";
const LEGACY_OPENAGENTS_TOOL_LABOR_REQUIREMENTS: &str = "openagents.labor.requirements";
const LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST: &str = "openagents.labor.evidence_list";
const LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH: &str = "openagents.labor.evidence.attach";
const LEGACY_OPENAGENTS_TOOL_LABOR_SUBMISSION_READY: &str = "openagents.labor.submission.ready";
const LEGACY_OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST: &str = "openagents.labor.verifier.request";
const LEGACY_OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH: &str = "openagents.labor.incident.attach";
const LEGACY_OPENAGENTS_TOOL_NAMES: &[&str] = &[
    LEGACY_OPENAGENTS_TOOL_PANE_LIST,
    LEGACY_OPENAGENTS_TOOL_PANE_OPEN,
    LEGACY_OPENAGENTS_TOOL_PANE_FOCUS,
    LEGACY_OPENAGENTS_TOOL_PANE_CLOSE,
    LEGACY_OPENAGENTS_TOOL_PANE_SET_INPUT,
    LEGACY_OPENAGENTS_TOOL_PANE_ACTION,
    LEGACY_OPENAGENTS_TOOL_CAD_INTENT,
    LEGACY_OPENAGENTS_TOOL_CAD_ACTION,
    LEGACY_OPENAGENTS_TOOL_SWAP_QUOTE,
    LEGACY_OPENAGENTS_TOOL_SWAP_EXECUTE,
    LEGACY_OPENAGENTS_TOOL_TREASURY_TRANSFER,
    LEGACY_OPENAGENTS_TOOL_TREASURY_CONVERT,
    LEGACY_OPENAGENTS_TOOL_TREASURY_RECEIPT,
    LEGACY_OPENAGENTS_TOOL_GOAL_SCHEDULER,
    LEGACY_OPENAGENTS_TOOL_WALLET_CHECK,
    LEGACY_OPENAGENTS_TOOL_PROVIDER_CONTROL,
    LEGACY_OPENAGENTS_TOOL_LABOR_SCOPE,
    LEGACY_OPENAGENTS_TOOL_LABOR_REQUIREMENTS,
    LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST,
    LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH,
    LEGACY_OPENAGENTS_TOOL_LABOR_SUBMISSION_READY,
    LEGACY_OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST,
    LEGACY_OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH,
];
pub(super) const OPENAGENTS_TOOL_PREFIXES: &[&str] = &["openagents_", "openagents."];
pub(super) const OPENAGENTS_TOOL_NAMES: &[&str] = OPENAGENTS_DYNAMIC_TOOL_NAMES;
const CAD_TOOL_RESPONSE_SCHEMA_VERSION: &str = "oa.cad.tool_response.v1";
const CAD_CHECKPOINT_SCHEMA_VERSION: &str = "oa.cad.checkpoint.v1";
const CAD_INTENT_PARSE_RETRY_LIMIT: u8 = 1;
const CAD_INTENT_TOOL_ENABLED_ENV: &str = "OPENAGENTS_CAD_INTENT_TOOL_ENABLED";

fn parse_bool_env_override(raw: &str) -> Option<bool> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn env_flag_enabled(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => parse_bool_env_override(&raw).unwrap_or(default),
        Err(_) => default,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ToolBridgeRequest {
    pub tool: String,
    pub arguments: Value,
}

impl ToolBridgeRequest {
    pub(super) fn decode_arguments<T: DeserializeOwned>(
        &self,
    ) -> Result<T, ToolBridgeResultEnvelope> {
        serde_json::from_value::<T>(self.arguments.clone()).map_err(|error| {
            ToolBridgeResultEnvelope::error(
                "OA-TOOL-ARGS-INVALID-SHAPE",
                format!(
                    "Arguments for '{}' did not match expected shape: {}",
                    self.tool, error
                ),
                json!({
                    "tool": self.tool,
                    "arguments": self.arguments,
                }),
            )
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ToolBridgeResultEnvelope {
    pub success: bool,
    pub code: String,
    pub message: String,
    pub details: Value,
}

impl ToolBridgeResultEnvelope {
    pub(super) fn ok(code: &str, message: impl Into<String>, details: Value) -> Self {
        Self {
            success: true,
            code: code.to_string(),
            message: message.into(),
            details,
        }
    }

    pub(super) fn error(code: &str, message: impl Into<String>, details: Value) -> Self {
        Self {
            success: false,
            code: code.to_string(),
            message: message.into(),
            details,
        }
    }

    pub(super) fn to_response(&self) -> DynamicToolCallResponse {
        DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: serde_json::to_string(self)
                    .unwrap_or_else(|_| "{\"success\":false,\"code\":\"OA-TOOL-RESPONSE-SERIALIZE-FAILED\",\"message\":\"failed to serialize tool response\",\"details\":{}}".to_string()),
            }],
            success: self.success,
        }
    }
}

pub(super) fn decode_tool_call_request(
    request: &AutopilotToolCallRequest,
) -> Result<ToolBridgeRequest, ToolBridgeResultEnvelope> {
    let tool = request.tool.trim();
    if !is_openagents_tool_namespace(tool) || !is_supported_tool(tool) {
        return Err(ToolBridgeResultEnvelope::error(
            "OA-TOOL-UNSUPPORTED",
            format!(
                "Unsupported tool '{}'. Supported tools must be in OpenAgents namespace and allowlisted.",
                request.tool
            ),
            json!({
                "tool": request.tool,
                "supported_tools": OPENAGENTS_TOOL_NAMES,
                "legacy_supported_tools": LEGACY_OPENAGENTS_TOOL_NAMES,
            }),
        ));
    }

    let arguments = if request.arguments.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&request.arguments).map_err(|error| {
            ToolBridgeResultEnvelope::error(
                "OA-TOOL-ARGS-INVALID-JSON",
                format!("Failed to parse tool arguments JSON: {error}"),
                json!({
                    "tool": tool,
                    "arguments_raw": request.arguments,
                }),
            )
        })?
    };

    if !arguments.is_object() {
        return Err(ToolBridgeResultEnvelope::error(
            "OA-TOOL-ARGS-NOT-OBJECT",
            "Tool arguments must decode to a JSON object",
            json!({
                "tool": tool,
                "arguments": arguments,
            }),
        ));
    }

    Ok(ToolBridgeRequest {
        tool: tool.to_string(),
        arguments,
    })
}

#[derive(Clone, Debug, Deserialize)]
struct PaneRefArgs {
    pane: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PaneInputArgs {
    pane: String,
    field: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize)]
struct PaneActionArgs {
    pane: String,
    action: String,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
struct CadIntentArgs {
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    intent_json: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct CadActionArgs {
    action: String,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
struct SwapQuoteArgs {
    goal_id: String,
    request_id: String,
    direction: String,
    amount: u64,
    unit: String,
    #[serde(default)]
    immediate_execution: bool,
    #[serde(default)]
    quote_ttl_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
struct SwapExecuteArgs {
    goal_id: String,
    quote_id: String,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TreasuryTransferArgs {
    from_owner_id: String,
    to_owner_id: String,
    asset: String,
    amount: u64,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TreasuryConvertArgs {
    owner_id: String,
    direction: String,
    amount: u64,
    unit: String,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TreasuryReceiptArgs {
    worker_request_id: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct BlinkSwapQuoteEnvelope {
    event: String,
    quote: BlinkSwapQuoteTerms,
}

#[derive(Clone, Debug, Deserialize)]
struct BlinkSwapExecutionEnvelope {
    event: String,
    status: String,
    quote: BlinkSwapQuoteTerms,
    #[serde(default, rename = "executedAtEpochSeconds")]
    executed_at_epoch_seconds: Option<u64>,
    #[serde(default)]
    execution: Option<BlinkSwapExecutionMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
struct BlinkSwapExecutionMetadata {
    #[serde(default, rename = "transactionId")]
    transaction_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct BlinkSwapQuoteTerms {
    #[serde(rename = "quoteId")]
    quote_id: String,
    direction: String,
    #[serde(rename = "amountIn")]
    amount_in: BlinkSwapAmount,
    #[serde(rename = "amountOut")]
    amount_out: BlinkSwapAmount,
    #[serde(rename = "expiresAtEpochSeconds")]
    expires_at_epoch_seconds: u64,
    #[serde(rename = "immediateExecution")]
    immediate_execution: bool,
    #[serde(rename = "feeSats")]
    fee_sats: u64,
    #[serde(rename = "feeBps")]
    fee_bps: u32,
    #[serde(rename = "slippageBps")]
    slippage_bps: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct BlinkSwapAmount {
    value: u64,
    unit: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GoalSchedulerToolArgs {
    action: String,
    #[serde(default)]
    goal_id: Option<String>,
    #[serde(default)]
    missed_run_policy: Option<String>,
    #[serde(default)]
    kill_switch_active: Option<bool>,
    #[serde(default)]
    kill_switch_reason: Option<String>,
    #[serde(default)]
    rollout_enabled: Option<bool>,
    #[serde(default)]
    rollout_stage: Option<String>,
    #[serde(default)]
    rollout_cohorts: Option<Vec<String>>,
    #[serde(default)]
    max_false_success_rate_bps: Option<u32>,
    #[serde(default)]
    max_abort_rate_bps: Option<u32>,
    #[serde(default)]
    max_error_rate_bps: Option<u32>,
    #[serde(default)]
    max_avg_payout_confirm_latency_seconds: Option<u64>,
    #[serde(default)]
    hardening_authoritative_payout_gate_validated: Option<bool>,
    #[serde(default)]
    hardening_scheduler_recovery_drills_validated: Option<bool>,
    #[serde(default)]
    hardening_swap_risk_alerting_validated: Option<bool>,
    #[serde(default)]
    hardening_incident_runbook_validated: Option<bool>,
    #[serde(default)]
    hardening_test_matrix_gate_green: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct WalletCheckArgs {
    #[serde(default)]
    window_seconds: Option<u64>,
    #[serde(default)]
    include_payments: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct ProviderControlArgs {
    action: String,
}

#[derive(Clone, Debug, Deserialize)]
struct LaborScopeArgs {
    #[serde(default)]
    work_unit_id: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct LaborEvidenceAttachArgs {
    #[serde(default)]
    work_unit_id: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
    kind: String,
    uri: String,
    digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct LaborClaimOpenArgs {
    #[serde(default)]
    work_unit_id: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
    #[serde(default)]
    reason_code: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct LaborClaimTransitionArgs {
    #[serde(default)]
    work_unit_id: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct LaborClaimRemedyArgs {
    #[serde(default)]
    work_unit_id: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
    outcome: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct LaborClaimDenyArgs {
    #[serde(default)]
    work_unit_id: Option<String>,
    #[serde(default)]
    contract_id: Option<String>,
    #[serde(default)]
    reason_code: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

const BLINK_SKILL_NAME: &str = "blink";
const BLINK_SWAP_QUOTE_SCRIPT: &str = "swap_quote.js";
const BLINK_SWAP_EXECUTE_SCRIPT: &str = "swap_execute.js";
const BLINK_BALANCE_SCRIPT: &str = "balance.js";
const BLINK_CREATE_INVOICE_SCRIPT: &str = "create_invoice.js";
const BLINK_CREATE_INVOICE_USD_SCRIPT: &str = "create_invoice_usd.js";
const BLINK_FEE_PROBE_SCRIPT: &str = "fee_probe.js";
const BLINK_PAY_INVOICE_SCRIPT: &str = "pay_invoice.js";
const BLINK_SWAP_PARSE_VERSION: &str = "blink.swap.v1";

pub(super) fn execute_openagents_tool_request(
    state: &mut RenderState,
    request: &AutopilotToolCallRequest,
) -> ToolBridgeResultEnvelope {
    let decoded = match decode_tool_call_request(request) {
        Ok(value) => value,
        Err(error) => return error,
    };

    if let Some(policy_error) = enforce_active_goal_command_scope(state, decoded.tool.as_str()) {
        return policy_error;
    }

    if let Some(scope_error) = enforce_labor_tool_scope(state, request, &decoded) {
        return scope_error;
    }

    match decoded.tool.as_str() {
        OPENAGENTS_TOOL_LABOR_SCOPE | LEGACY_OPENAGENTS_TOOL_LABOR_SCOPE => {
            let args = match decoded.decode_arguments::<LaborScopeArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_scope_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_REQUIREMENTS | LEGACY_OPENAGENTS_TOOL_LABOR_REQUIREMENTS => {
            let args = match decoded.decode_arguments::<LaborScopeArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_requirements_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST | LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST => {
            let args = match decoded.decode_arguments::<LaborScopeArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_evidence_list_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH | LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH => {
            let args = match decoded.decode_arguments::<LaborEvidenceAttachArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_evidence_attach_tool(
                state,
                request.turn_id.as_str(),
                &args,
                false,
            );
        }
        OPENAGENTS_TOOL_LABOR_SUBMISSION_READY | LEGACY_OPENAGENTS_TOOL_LABOR_SUBMISSION_READY => {
            let args = match decoded.decode_arguments::<LaborScopeArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_submission_ready_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST | LEGACY_OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST => {
            let args = match decoded.decode_arguments::<LaborScopeArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_verifier_request_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH | LEGACY_OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH => {
            let args = match decoded.decode_arguments::<LaborEvidenceAttachArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_evidence_attach_tool(
                state,
                request.turn_id.as_str(),
                &args,
                true,
            );
        }
        OPENAGENTS_TOOL_LABOR_CLAIM_STATUS => {
            let args = match decoded.decode_arguments::<LaborScopeArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_claim_status_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_CLAIM_OPEN => {
            let args = match decoded.decode_arguments::<LaborClaimOpenArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_claim_open_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_CLAIM_REVIEW => {
            let args = match decoded.decode_arguments::<LaborClaimTransitionArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_claim_review_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_CLAIM_REMEDY => {
            let args = match decoded.decode_arguments::<LaborClaimRemedyArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_claim_remedy_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_CLAIM_DENY => {
            let args = match decoded.decode_arguments::<LaborClaimDenyArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_claim_deny_tool(state, request.turn_id.as_str(), &args);
        }
        OPENAGENTS_TOOL_LABOR_CLAIM_RESOLVE => {
            let args = match decoded.decode_arguments::<LaborClaimTransitionArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            return execute_labor_claim_resolve_tool(state, request.turn_id.as_str(), &args);
        }
        _ => {}
    }

    match decoded.tool.as_str() {
        OPENAGENTS_TOOL_PANE_LIST | LEGACY_OPENAGENTS_TOOL_PANE_LIST => execute_pane_list(state),
        OPENAGENTS_TOOL_PANE_OPEN | LEGACY_OPENAGENTS_TOOL_PANE_OPEN => {
            let args = match decoded.decode_arguments::<PaneRefArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_open(state, args.pane.trim())
        }
        OPENAGENTS_TOOL_PANE_FOCUS | LEGACY_OPENAGENTS_TOOL_PANE_FOCUS => {
            let args = match decoded.decode_arguments::<PaneRefArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_focus(state, args.pane.trim())
        }
        OPENAGENTS_TOOL_PANE_CLOSE | LEGACY_OPENAGENTS_TOOL_PANE_CLOSE => {
            let args = match decoded.decode_arguments::<PaneRefArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_close(state, args.pane.trim())
        }
        OPENAGENTS_TOOL_PANE_SET_INPUT | LEGACY_OPENAGENTS_TOOL_PANE_SET_INPUT => {
            let args = match decoded.decode_arguments::<PaneInputArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_set_input(state, &args)
        }
        OPENAGENTS_TOOL_PANE_ACTION | LEGACY_OPENAGENTS_TOOL_PANE_ACTION => {
            let args = match decoded.decode_arguments::<PaneActionArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_action(state, &args)
        }
        OPENAGENTS_TOOL_CAD_INTENT | LEGACY_OPENAGENTS_TOOL_CAD_INTENT => {
            let args = match decoded.decode_arguments::<CadIntentArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_cad_intent(state, &args)
        }
        OPENAGENTS_TOOL_CAD_ACTION | LEGACY_OPENAGENTS_TOOL_CAD_ACTION => {
            let args = match decoded.decode_arguments::<CadActionArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_cad_action(state, &args)
        }
        OPENAGENTS_TOOL_SWAP_QUOTE | LEGACY_OPENAGENTS_TOOL_SWAP_QUOTE => {
            let args = match decoded.decode_arguments::<SwapQuoteArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_swap_quote(state, &args)
        }
        OPENAGENTS_TOOL_SWAP_EXECUTE | LEGACY_OPENAGENTS_TOOL_SWAP_EXECUTE => {
            let args = match decoded.decode_arguments::<SwapExecuteArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_swap_execute(state, &args)
        }
        OPENAGENTS_TOOL_TREASURY_TRANSFER | LEGACY_OPENAGENTS_TOOL_TREASURY_TRANSFER => {
            let args = match decoded.decode_arguments::<TreasuryTransferArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_treasury_transfer(state, &args)
        }
        OPENAGENTS_TOOL_TREASURY_CONVERT | LEGACY_OPENAGENTS_TOOL_TREASURY_CONVERT => {
            let args = match decoded.decode_arguments::<TreasuryConvertArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_treasury_convert(state, &args)
        }
        OPENAGENTS_TOOL_TREASURY_RECEIPT | LEGACY_OPENAGENTS_TOOL_TREASURY_RECEIPT => {
            let args = match decoded.decode_arguments::<TreasuryReceiptArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_treasury_receipt(state, &args)
        }
        OPENAGENTS_TOOL_GOAL_SCHEDULER | LEGACY_OPENAGENTS_TOOL_GOAL_SCHEDULER => {
            let args = match decoded.decode_arguments::<GoalSchedulerToolArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_goal_scheduler_tool(state, &args)
        }
        OPENAGENTS_TOOL_WALLET_CHECK | LEGACY_OPENAGENTS_TOOL_WALLET_CHECK => {
            let args = match decoded.decode_arguments::<WalletCheckArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_wallet_check_tool(state, &args)
        }
        OPENAGENTS_TOOL_PROVIDER_CONTROL | LEGACY_OPENAGENTS_TOOL_PROVIDER_CONTROL => {
            let args = match decoded.decode_arguments::<ProviderControlArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_provider_control_tool(state, &args)
        }
        _ => ToolBridgeResultEnvelope::error(
            "OA-TOOL-NOT-IMPLEMENTED",
            format!("Tool '{}' is parsed but not yet implemented", decoded.tool),
            json!({
                "tool": decoded.tool,
            }),
        ),
    }
}

fn is_supported_tool(tool: &str) -> bool {
    OPENAGENTS_TOOL_NAMES
        .iter()
        .chain(LEGACY_OPENAGENTS_TOOL_NAMES.iter())
        .any(|entry| *entry == tool)
}

pub(super) fn is_openagents_tool_namespace(tool: &str) -> bool {
    let tool = tool.trim();
    OPENAGENTS_TOOL_PREFIXES
        .iter()
        .any(|prefix| tool.starts_with(prefix))
}

pub(super) fn is_openagents_cad_intent_tool(tool: &str) -> bool {
    let trimmed = tool.trim();
    trimmed.eq_ignore_ascii_case(OPENAGENTS_TOOL_CAD_INTENT)
        || trimmed.eq_ignore_ascii_case(LEGACY_OPENAGENTS_TOOL_CAD_INTENT)
}

fn enforce_active_goal_command_scope(
    state: &RenderState,
    tool_name: &str,
) -> Option<ToolBridgeResultEnvelope> {
    let active_run = state.goal_loop_executor.active_run.as_ref()?;
    let goal = state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .find(|goal| goal.goal_id == active_run.goal_id)?;
    let policy = &goal.constraints.autonomy_policy;

    if policy.kill_switch_active {
        return Some(ToolBridgeResultEnvelope::error(
            "OA-GOAL-POLICY-KILL-SWITCH",
            "Goal kill switch is engaged; tool command denied",
            json!({
                "goal_id": goal.goal_id,
                "tool": tool_name,
                "kill_switch_reason": policy.kill_switch_reason,
            }),
        ));
    }

    let allowlist = policy
        .allowed_command_prefixes
        .iter()
        .map(|prefix| normalize_key(prefix))
        .filter(|prefix| !prefix.is_empty())
        .collect::<Vec<_>>();
    if allowlist.is_empty() {
        return None;
    }

    let normalized_tool = normalize_key(tool_name);
    if allowlist
        .iter()
        .any(|prefix| normalized_tool.starts_with(prefix))
    {
        None
    } else {
        Some(ToolBridgeResultEnvelope::error(
            "OA-GOAL-POLICY-COMMAND-DENIED",
            "Tool command denied by goal command scope policy",
            json!({
                "goal_id": goal.goal_id,
                "tool": tool_name,
                "allowed_command_prefixes": policy.allowed_command_prefixes,
            }),
        ))
    }
}

fn enforce_labor_tool_scope(
    state: &RenderState,
    request: &AutopilotToolCallRequest,
    decoded: &ToolBridgeRequest,
) -> Option<ToolBridgeResultEnvelope> {
    if !is_labor_tool(decoded.tool.as_str()) {
        return None;
    }

    let Some(binding) = state
        .autopilot_chat
        .turn_labor_binding_for(request.turn_id.as_str())
    else {
        return Some(ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-NOT-ACTIVE",
            "Labor tool requested outside an active labor-bound turn",
            json!({
                "tool": decoded.tool,
                "turn_id": request.turn_id,
            }),
        ));
    };

    if let Some(error) = enforce_matching_labor_contract_scope(binding, decoded) {
        return Some(error);
    }

    if is_labor_evidence_attach_tool(decoded.tool.as_str())
        && let Some(error) = enforce_labor_evidence_uri_scope(binding, decoded)
    {
        let mut details = match error.details {
            Value::Object(map) => map,
            value => {
                let mut map = serde_json::Map::new();
                map.insert("error".to_string(), value);
                map
            }
        };
        details.insert("tool".to_string(), json!(decoded.tool));
        details.insert("turn_id".to_string(), json!(request.turn_id));
        return Some(ToolBridgeResultEnvelope::error(
            error.code.as_str(),
            error.message,
            Value::Object(details),
        ));
    }

    None
}

fn is_labor_tool(tool: &str) -> bool {
    matches!(
        tool,
        OPENAGENTS_TOOL_LABOR_SCOPE
            | LEGACY_OPENAGENTS_TOOL_LABOR_SCOPE
            | OPENAGENTS_TOOL_LABOR_REQUIREMENTS
            | LEGACY_OPENAGENTS_TOOL_LABOR_REQUIREMENTS
            | OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST
            | LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST
            | OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH
            | LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH
            | OPENAGENTS_TOOL_LABOR_SUBMISSION_READY
            | LEGACY_OPENAGENTS_TOOL_LABOR_SUBMISSION_READY
            | OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST
            | LEGACY_OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST
            | OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH
            | LEGACY_OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH
            | OPENAGENTS_TOOL_LABOR_CLAIM_STATUS
            | OPENAGENTS_TOOL_LABOR_CLAIM_OPEN
            | OPENAGENTS_TOOL_LABOR_CLAIM_REVIEW
            | OPENAGENTS_TOOL_LABOR_CLAIM_REMEDY
            | OPENAGENTS_TOOL_LABOR_CLAIM_DENY
            | OPENAGENTS_TOOL_LABOR_CLAIM_RESOLVE
    )
}

fn is_labor_evidence_attach_tool(tool: &str) -> bool {
    matches!(
        tool,
        OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH
            | LEGACY_OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH
            | OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH
            | LEGACY_OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH
    )
}

fn enforce_matching_labor_contract_scope(
    binding: &crate::labor_orchestrator::CodexLaborBinding,
    decoded: &ToolBridgeRequest,
) -> Option<ToolBridgeResultEnvelope> {
    let provided_work_unit_id = string_argument(decoded.arguments.as_object(), "work_unit_id");
    if let Some(provided_work_unit_id) = provided_work_unit_id
        && provided_work_unit_id != binding.work_unit_id
    {
        return Some(ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-MISMATCH",
            "Provided work_unit_id does not match the active labor binding",
            json!({
                "tool": decoded.tool,
                "expected_work_unit_id": binding.work_unit_id,
                "provided_work_unit_id": provided_work_unit_id,
            }),
        ));
    }

    let provided_contract_id = string_argument(decoded.arguments.as_object(), "contract_id");
    if let Some(provided_contract_id) = provided_contract_id
        && provided_contract_id != binding.contract_id
    {
        return Some(ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-MISMATCH",
            "Provided contract_id does not match the active labor binding",
            json!({
                "tool": decoded.tool,
                "expected_contract_id": binding.contract_id,
                "provided_contract_id": provided_contract_id,
            }),
        ));
    }

    None
}

fn enforce_labor_evidence_uri_scope(
    binding: &crate::labor_orchestrator::CodexLaborBinding,
    decoded: &ToolBridgeRequest,
) -> Option<ToolBridgeResultEnvelope> {
    let uri = string_argument(decoded.arguments.as_object(), "uri")?;
    let scope_root = binding.artifact_scope_root();
    if uri.starts_with(scope_root.as_str()) {
        return None;
    }
    Some(ToolBridgeResultEnvelope::error(
        "OA-LABOR-EVIDENCE-OUT-OF-SCOPE",
        "Evidence URI is outside the contract artifact scope",
        json!({
            "uri": uri,
            "artifact_scope_root": scope_root,
        }),
    ))
}

fn string_argument<'a>(
    arguments: Option<&'a serde_json::Map<String, Value>>,
    key: &str,
) -> Option<&'a str> {
    arguments?
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn sync_goal_attempt_labor_linkage(state: &mut RenderState, turn_id: &str) {
    let Some(labor) = state.autopilot_chat.turn_labor_linkage_for(turn_id) else {
        return;
    };
    state
        .goal_loop_executor
        .merge_attempt_labor_linkage(Some(turn_id), labor);
}

fn execute_labor_scope_tool(
    state: &RenderState,
    turn_id: &str,
    args: &LaborScopeArgs,
) -> ToolBridgeResultEnvelope {
    match labor_scope_details(state, turn_id) {
        Ok(scope) => ToolBridgeResultEnvelope::ok(
            "OA-LABOR-SCOPE-OK",
            "Fetched active labor scope",
            json!({
                "turn_id": turn_id,
                "requested_work_unit_id": args.work_unit_id,
                "requested_contract_id": args.contract_id,
                "scope": scope,
            }),
        ),
        Err(error) => error,
    }
}

fn execute_labor_requirements_tool(
    state: &RenderState,
    turn_id: &str,
    args: &LaborScopeArgs,
) -> ToolBridgeResultEnvelope {
    let Some(requirements) = state
        .autopilot_chat
        .turn_labor_requirements_payload(turn_id)
    else {
        return ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-NOT-ACTIVE",
            "Labor requirements requested outside an active labor-bound turn",
            json!({
                "turn_id": turn_id,
            }),
        );
    };

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-REQUIREMENTS-OK",
        "Fetched labor acceptance criteria and evidence requirements",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "requirements": requirements,
        }),
    )
}

fn execute_labor_evidence_list_tool(
    state: &RenderState,
    turn_id: &str,
    args: &LaborScopeArgs,
) -> ToolBridgeResultEnvelope {
    let Some(evidence) = state.autopilot_chat.turn_labor_evidence_payload(turn_id) else {
        return ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-NOT-ACTIVE",
            "Labor evidence requested outside an active labor-bound turn",
            json!({
                "turn_id": turn_id,
            }),
        );
    };

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-EVIDENCE-LIST-OK",
        "Fetched labor evidence and unresolved gaps",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "evidence": evidence,
        }),
    )
}

fn execute_labor_evidence_attach_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborEvidenceAttachArgs,
    incident: bool,
) -> ToolBridgeResultEnvelope {
    let payload = match state.autopilot_chat.attach_turn_labor_evidence(
        turn_id,
        EvidenceRef::new(
            args.kind.trim().to_string(),
            args.uri.trim().to_string(),
            args.digest.trim().to_string(),
        ),
        incident,
    ) {
        Ok(Some(payload)) => payload,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor evidence attach requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-EVIDENCE-ATTACH-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "incident": incident,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "kind": args.kind,
                    "uri": args.uri,
                }),
            );
        }
    };

    ToolBridgeResultEnvelope::ok(
        if incident {
            "OA-LABOR-INCIDENT-ATTACH-OK"
        } else {
            "OA-LABOR-EVIDENCE-ATTACH-OK"
        },
        if incident {
            "Attached incident evidence to the active labor contract"
        } else {
            "Attached evidence to the active labor contract"
        },
        json!({
            "turn_id": turn_id,
            "incident": incident,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "evidence": payload,
        }),
    )
}

fn execute_labor_submission_ready_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborScopeArgs,
) -> ToolBridgeResultEnvelope {
    let Some(binding) = state.autopilot_chat.turn_labor_binding_for(turn_id) else {
        return ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-NOT-ACTIVE",
            "Labor submission requested outside an active labor-bound turn",
            json!({
                "turn_id": turn_id,
            }),
        );
    };
    let evidence_gaps = binding.required_evidence_gaps();
    if !evidence_gaps.is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-LABOR-SUBMISSION-NOT-READY",
            "Labor submission cannot be marked ready until required evidence is present",
            json!({
                "turn_id": turn_id,
                "requested_work_unit_id": args.work_unit_id,
                "requested_contract_id": args.contract_id,
                "evidence_gaps": evidence_gaps,
                "requirements": state.autopilot_chat.turn_labor_requirements_payload(turn_id),
                "evidence": state.autopilot_chat.turn_labor_evidence_payload(turn_id),
            }),
        );
    }

    let created_at_epoch_ms = current_epoch_ms();
    let submission = match state
        .autopilot_chat
        .assemble_turn_labor_submission(turn_id, created_at_epoch_ms)
    {
        Ok(Some(submission)) => submission,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor submission requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SUBMISSION-ASSEMBLY-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "requirements": state.autopilot_chat.turn_labor_requirements_payload(turn_id),
                    "evidence": state.autopilot_chat.turn_labor_evidence_payload(turn_id),
                }),
            );
        }
    };

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-SUBMISSION-READY-OK",
        "Labor submission assembled and ready for verification",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "submission": submission,
            "requirements": state.autopilot_chat.turn_labor_requirements_payload(turn_id),
            "evidence": state.autopilot_chat.turn_labor_evidence_payload(turn_id),
        }),
    )
}

fn execute_labor_verifier_request_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborScopeArgs,
) -> ToolBridgeResultEnvelope {
    let readiness = execute_labor_submission_ready_tool(state, turn_id, args);
    if !readiness.success {
        return readiness;
    }

    let verified_at_epoch_ms = current_epoch_ms();
    let verdict = match state
        .autopilot_chat
        .finalize_turn_labor_verdict(turn_id, verified_at_epoch_ms)
    {
        Ok(Some(verdict)) => verdict,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Verifier requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            let _ = state.autopilot_chat.open_turn_labor_claim(
                turn_id,
                verified_at_epoch_ms,
                None,
                Some(error.as_str()),
            );
            sync_goal_attempt_labor_linkage(state, turn_id);
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-VERIFIER-BLOCKED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "submission": state.autopilot_chat.turn_labor_submission_for(turn_id),
                    "evidence": state.autopilot_chat.turn_labor_evidence_payload(turn_id),
                }),
            );
        }
    };

    if !verdict.settlement_ready {
        let _ = state.autopilot_chat.open_turn_labor_claim(
            turn_id,
            verified_at_epoch_ms,
            verdict.verdict.reason_code.as_deref(),
            verdict.settlement_withheld_reason.as_deref(),
        );
    }
    sync_goal_attempt_labor_linkage(state, turn_id);

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-VERIFIER-OK",
        "Verifier completed for the active labor submission",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "submission": state.autopilot_chat.turn_labor_submission_for(turn_id),
            "verdict": verdict,
            "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
            "evidence": state.autopilot_chat.turn_labor_evidence_payload(turn_id),
            "settlement_ready": state.autopilot_chat.turn_labor_settlement_ready(turn_id),
        }),
    )
}

fn execute_labor_claim_status_tool(
    state: &RenderState,
    turn_id: &str,
    args: &LaborScopeArgs,
) -> ToolBridgeResultEnvelope {
    let Some(claim) = state.autopilot_chat.turn_labor_claim_payload(turn_id) else {
        return ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-NOT-ACTIVE",
            "Labor claim status requested outside an active labor-bound turn",
            json!({
                "turn_id": turn_id,
            }),
        );
    };

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-CLAIM-STATUS-OK",
        "Fetched labor claim and remedy state",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "claim": claim,
        }),
    )
}

fn execute_labor_claim_open_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborClaimOpenArgs,
) -> ToolBridgeResultEnvelope {
    let claim = match state.autopilot_chat.open_turn_labor_claim(
        turn_id,
        current_epoch_ms(),
        args.reason_code.as_deref(),
        args.note.as_deref(),
    ) {
        Ok(Some(claim)) => claim,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor claim requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-CLAIM-OPEN-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "submission": state.autopilot_chat.turn_labor_submission_for(turn_id),
                    "verdict": state.autopilot_chat.turn_labor_verdict_for(turn_id),
                    "evidence": state.autopilot_chat.turn_labor_evidence_payload(turn_id),
                }),
            );
        }
    };
    sync_goal_attempt_labor_linkage(state, turn_id);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "labor claim opened: claim_id={} state={}",
        claim.claim.claim_id,
        claim.status_label()
    ));

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-CLAIM-OPEN-OK",
        "Opened a claim against the active labor contract",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
        }),
    )
}

fn execute_labor_claim_review_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborClaimTransitionArgs,
) -> ToolBridgeResultEnvelope {
    let claim = match state.autopilot_chat.review_turn_labor_claim(
        turn_id,
        current_epoch_ms(),
        args.note.as_deref(),
    ) {
        Ok(Some(claim)) => claim,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor claim review requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-CLAIM-REVIEW-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
                }),
            );
        }
    };
    sync_goal_attempt_labor_linkage(state, turn_id);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "labor claim under review: claim_id={} state={}",
        claim.claim.claim_id,
        claim.status_label()
    ));

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-CLAIM-REVIEW-OK",
        "Moved the active labor claim into review",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
        }),
    )
}

fn execute_labor_claim_remedy_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborClaimRemedyArgs,
) -> ToolBridgeResultEnvelope {
    let claim = match state.autopilot_chat.issue_turn_labor_remedy(
        turn_id,
        current_epoch_ms(),
        args.outcome.as_str(),
        args.note.as_deref(),
    ) {
        Ok(Some(claim)) => claim,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor claim remedy requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-CLAIM-REMEDY-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
                }),
            );
        }
    };
    sync_goal_attempt_labor_linkage(state, turn_id);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "labor remedy issued: claim_id={} outcome={}",
        claim.claim.claim_id,
        claim
            .remedy
            .as_ref()
            .map(|remedy| remedy.outcome.as_str())
            .unwrap_or("unknown")
    ));

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-CLAIM-REMEDY-OK",
        "Issued a remedy for the active labor claim",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
        }),
    )
}

fn execute_labor_claim_deny_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborClaimDenyArgs,
) -> ToolBridgeResultEnvelope {
    let claim = match state.autopilot_chat.deny_turn_labor_claim(
        turn_id,
        current_epoch_ms(),
        args.reason_code.as_deref(),
        args.note.as_deref(),
    ) {
        Ok(Some(claim)) => claim,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor claim denial requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-CLAIM-DENY-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
                }),
            );
        }
    };
    sync_goal_attempt_labor_linkage(state, turn_id);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "labor claim denied: claim_id={} reason={}",
        claim.claim.claim_id,
        claim
            .claim
            .reason_code
            .as_deref()
            .unwrap_or("codex.claim.denied")
    ));

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-CLAIM-DENY-OK",
        "Denied the active labor claim",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
        }),
    )
}

fn execute_labor_claim_resolve_tool(
    state: &mut RenderState,
    turn_id: &str,
    args: &LaborClaimTransitionArgs,
) -> ToolBridgeResultEnvelope {
    let claim = match state.autopilot_chat.resolve_turn_labor_claim(
        turn_id,
        current_epoch_ms(),
        args.note.as_deref(),
    ) {
        Ok(Some(claim)) => claim,
        Ok(None) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-SCOPE-NOT-ACTIVE",
                "Labor claim resolution requested outside an active labor-bound turn",
                json!({
                    "turn_id": turn_id,
                }),
            );
        }
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-LABOR-CLAIM-RESOLVE-FAILED",
                error,
                json!({
                    "turn_id": turn_id,
                    "requested_work_unit_id": args.work_unit_id,
                    "requested_contract_id": args.contract_id,
                    "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
                }),
            );
        }
    };
    sync_goal_attempt_labor_linkage(state, turn_id);
    state.autopilot_chat.record_turn_timeline_event(format!(
        "labor claim resolved: claim_id={} state={}",
        claim.claim.claim_id,
        claim.status_label()
    ));

    ToolBridgeResultEnvelope::ok(
        "OA-LABOR-CLAIM-RESOLVE-OK",
        "Resolved the active labor claim",
        json!({
            "turn_id": turn_id,
            "requested_work_unit_id": args.work_unit_id,
            "requested_contract_id": args.contract_id,
            "claim": state.autopilot_chat.turn_labor_claim_payload(turn_id),
        }),
    )
}

fn labor_scope_details(
    state: &RenderState,
    turn_id: &str,
) -> Result<Value, ToolBridgeResultEnvelope> {
    let Some(mut scope) = state.autopilot_chat.turn_labor_scope_payload(turn_id) else {
        return Err(ToolBridgeResultEnvelope::error(
            "OA-LABOR-SCOPE-NOT-ACTIVE",
            "Labor scope requested outside an active labor-bound turn",
            json!({
                "turn_id": turn_id,
            }),
        ));
    };

    let command_scope = if let Some(policy) = goal_policy_for_turn(state, turn_id) {
        json!({
            "source": "goal_policy",
            "allowed_command_prefixes": policy.allowed_command_prefixes,
            "allowed_file_roots": policy.allowed_file_roots,
            "kill_switch_active": policy.kill_switch_active,
            "kill_switch_reason": policy.kill_switch_reason,
        })
    } else {
        json!({
            "source": "default_openagents_namespace",
            "allowed_command_prefixes": OPENAGENTS_TOOL_PREFIXES,
            "allowed_file_roots": Vec::<String>::new(),
            "kill_switch_active": false,
            "kill_switch_reason": Option::<String>::None,
        })
    };

    if let Some(object) = scope.as_object_mut() {
        object.insert("turn_id".to_string(), json!(turn_id));
        object.insert("command_scope".to_string(), command_scope);
    }
    Ok(scope)
}

fn goal_policy_for_turn<'a>(
    state: &'a RenderState,
    turn_id: &str,
) -> Option<&'a crate::state::autopilot_goals::GoalAutonomyPolicy> {
    let metadata = state.autopilot_chat.turn_metadata_for(turn_id)?;
    let crate::labor_orchestrator::CodexRunClassification::AutonomousGoal { goal_id, .. } =
        &metadata.run_classification
    else {
        return None;
    };
    state
        .autopilot_goals
        .document
        .active_goals
        .iter()
        .find(|goal| goal.goal_id == *goal_id)
        .map(|goal| &goal.constraints.autonomy_policy)
}

fn execute_pane_list(state: &RenderState) -> ToolBridgeResultEnvelope {
    let registered = enabled_pane_specs()
        .filter(|spec| spec.kind != PaneKind::Empty)
        .map(|spec| {
            json!({
                "kind": pane_kind_key(spec.kind),
                "title": spec.title,
                "command_id": spec.command.map(|command| command.id),
                "singleton": spec.singleton,
                "startup": spec.startup,
            })
        })
        .collect::<Vec<_>>();

    let open = state
        .panes
        .iter()
        .map(|pane| {
            json!({
                "pane_id": pane.id,
                "kind": pane_kind_key(pane.kind),
                "title": pane.title,
                "z_index": pane.z_index,
            })
        })
        .collect::<Vec<_>>();

    let active = PaneController::active(state).and_then(|pane_id| {
        state
            .panes
            .iter()
            .find(|pane| pane.id == pane_id)
            .map(|pane| {
                json!({
                    "pane_id": pane.id,
                    "kind": pane_kind_key(pane.kind),
                    "title": pane.title,
                    "z_index": pane.z_index,
                })
            })
    });

    ToolBridgeResultEnvelope::ok(
        "OA-PANE-LIST-OK",
        "Listed registered and open panes",
        json!({
            "registered": registered,
            "open": open,
            "active": active,
        }),
    )
}

fn execute_pane_open(state: &mut RenderState, pane_ref: &str) -> ToolBridgeResultEnvelope {
    let Some(kind) = resolve_pane_kind(state, pane_ref) else {
        return pane_resolution_error("OA-PANE-OPEN-NOT-FOUND", pane_ref);
    };
    let pane_id = PaneController::create_for_kind(state, kind);
    ToolBridgeResultEnvelope::ok(
        "OA-PANE-OPEN-OK",
        format!("Opened pane '{}'", pane_kind_key(kind)),
        json!({
            "pane_id": pane_id,
            "kind": pane_kind_key(kind),
            "title": pane_spec(kind).title,
        }),
    )
}

fn execute_pane_focus(state: &mut RenderState, pane_ref: &str) -> ToolBridgeResultEnvelope {
    let Some(kind) = resolve_pane_kind(state, pane_ref) else {
        return pane_resolution_error("OA-PANE-FOCUS-NOT-FOUND", pane_ref);
    };
    let pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == kind)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| (pane.id, pane.title.clone()));
    let Some((pane_id, pane_title)) = pane else {
        return ToolBridgeResultEnvelope::error(
            "OA-PANE-FOCUS-NOT-OPEN",
            format!("Pane '{}' is not currently open", pane_kind_key(kind)),
            json!({
                "pane": pane_ref,
                "kind": pane_kind_key(kind),
            }),
        );
    };
    PaneController::bring_to_front(state, pane_id);
    ToolBridgeResultEnvelope::ok(
        "OA-PANE-FOCUS-OK",
        format!("Focused pane '{}'", pane_kind_key(kind)),
        json!({
            "pane_id": pane_id,
            "kind": pane_kind_key(kind),
            "title": pane_title,
        }),
    )
}

fn execute_pane_close(state: &mut RenderState, pane_ref: &str) -> ToolBridgeResultEnvelope {
    let Some(kind) = resolve_pane_kind(state, pane_ref) else {
        return pane_resolution_error("OA-PANE-CLOSE-NOT-FOUND", pane_ref);
    };
    let pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == kind)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| (pane.id, pane.title.clone()));
    let Some((pane_id, pane_title)) = pane else {
        return ToolBridgeResultEnvelope::error(
            "OA-PANE-CLOSE-NOT-OPEN",
            format!("Pane '{}' is not currently open", pane_kind_key(kind)),
            json!({
                "pane": pane_ref,
                "kind": pane_kind_key(kind),
            }),
        );
    };
    PaneController::close(state, pane_id);
    ToolBridgeResultEnvelope::ok(
        "OA-PANE-CLOSE-OK",
        format!("Closed pane '{}'", pane_kind_key(kind)),
        json!({
            "pane_id": pane_id,
            "kind": pane_kind_key(kind),
            "title": pane_title,
        }),
    )
}

fn execute_pane_set_input(
    state: &mut RenderState,
    args: &PaneInputArgs,
) -> ToolBridgeResultEnvelope {
    let Some(kind) = resolve_pane_kind(state, args.pane.trim()) else {
        return pane_resolution_error("OA-PANE-SET-INPUT-NOT-FOUND", args.pane.trim());
    };
    let _ = PaneController::create_for_kind(state, kind);
    let field = normalize_key(&args.field);
    let value = args.value.clone();

    let applied = match kind {
        PaneKind::AutopilotChat => apply_chat_input(state, &field, &value),
        PaneKind::RelayConnections => apply_relay_connections_input(state, &field, &value),
        PaneKind::LocalInference => apply_local_inference_input(state, &field, &value),
        PaneKind::AppleFmWorkbench => apply_apple_fm_workbench_input(state, &field, &value),
        PaneKind::NetworkRequests => apply_network_requests_input(state, &field, &value),
        PaneKind::Settings => apply_settings_input(state, &field, &value),
        PaneKind::Credentials => apply_credentials_input(state, &field, &value),
        PaneKind::JobHistory => apply_job_history_input(state, &field, &value),
        PaneKind::SparkWallet => apply_spark_wallet_input(state, &field, &value),
        PaneKind::SparkCreateInvoice => apply_create_invoice_input(state, &field, &value),
        PaneKind::SparkPayInvoice => apply_pay_invoice_input(state, &field, &value),
        _ => false,
    };

    if !applied {
        return ToolBridgeResultEnvelope::error(
            "OA-PANE-INPUT-UNSUPPORTED",
            format!(
                "Field '{}' is not a supported tool-writable input for pane '{}'",
                args.field,
                pane_kind_key(kind)
            ),
            json!({
                "pane": pane_kind_key(kind),
                "field": args.field,
            }),
        );
    }

    ToolBridgeResultEnvelope::ok(
        "OA-PANE-SET-INPUT-OK",
        format!(
            "Updated input '{}' on pane '{}'",
            args.field,
            pane_kind_key(kind)
        ),
        json!({
            "pane": pane_kind_key(kind),
            "field": args.field,
            "value_chars": args.value.chars().count(),
        }),
    )
}

fn execute_pane_action(state: &mut RenderState, args: &PaneActionArgs) -> ToolBridgeResultEnvelope {
    let Some(kind) = resolve_pane_kind(state, args.pane.trim()) else {
        return pane_resolution_error("OA-PANE-ACTION-NOT-FOUND", args.pane.trim());
    };
    let _ = PaneController::create_for_kind(state, kind);
    let action_key = normalize_key(&args.action);

    if action_key == "snapshot" || action_key == "status" {
        return ToolBridgeResultEnvelope::ok(
            "OA-PANE-SNAPSHOT-OK",
            format!("Captured pane snapshot for '{}'", pane_kind_key(kind)),
            pane_snapshot_details(state, kind),
        );
    }

    let hit_action = match pane_action_to_hit_action(kind, action_key.as_str(), args.index) {
        Ok(action) => action,
        Err(error) => return error,
    };

    let changed = super::run_pane_hit_action(state, hit_action);
    ToolBridgeResultEnvelope::ok(
        "OA-PANE-ACTION-OK",
        format!(
            "Executed action '{}' on pane '{}'",
            args.action,
            pane_kind_key(kind)
        ),
        json!({
            "pane": pane_kind_key(kind),
            "action": args.action,
            "index": args.index,
            "changed": changed,
            "active_pane_id": PaneController::active(state),
            "snapshot": pane_snapshot_details(state, kind),
        }),
    )
}

fn pane_snapshot_details(state: &RenderState, kind: PaneKind) -> Value {
    let open_instances = state
        .panes
        .iter()
        .filter(|pane| pane.kind == kind)
        .collect::<Vec<_>>();
    let top = open_instances.iter().max_by_key(|pane| pane.z_index);

    let mut payload = json!({
        "kind": pane_kind_key(kind),
        "title": pane_spec(kind).title,
        "open_instances": open_instances.len(),
        "top_pane_id": top.map(|pane| pane.id),
        "active_pane_id": PaneController::active(state),
    });

    if let Some(map) = payload.as_object_mut() {
        match kind {
            PaneKind::AutopilotChat => {
                map.insert(
                    "chat".to_string(),
                    json!({
                        "active_thread_id": state.autopilot_chat.active_thread_id,
                        "active_turn_id": state.autopilot_chat.active_turn_id,
                        "last_error": state.autopilot_chat.last_error,
                        "message_count": state.autopilot_chat.messages.len(),
                    }),
                );
            }
            PaneKind::LocalInference => {
                map.insert(
                    "local_inference".to_string(),
                    json!({
                        "pending_request_id": state.local_inference.pending_request_id,
                        "last_request_id": state.local_inference.last_request_id,
                        "last_model": state.local_inference.last_model,
                        "output_chars": state.local_inference.output_chars,
                        "runtime_reachable": state.gpt_oss_execution.reachable,
                        "configured_model": state.gpt_oss_execution.configured_model,
                        "ready_model": state.gpt_oss_execution.ready_model,
                        "last_error": state.local_inference.last_error,
                    }),
                );
            }
            PaneKind::AppleFmWorkbench => {
                map.insert(
                    "apple_fm_workbench".to_string(),
                    json!({
                        "pending_request_id": state.apple_fm_workbench.pending_request_id,
                        "last_request_id": state.apple_fm_workbench.last_request_id,
                        "last_operation": state.apple_fm_workbench.last_operation,
                        "active_session_id": state.apple_fm_workbench.active_session_id,
                        "last_model": state.apple_fm_workbench.last_model,
                        "output_chars": state.apple_fm_workbench.output_chars,
                        "tool_profile": state.apple_fm_workbench.tool_profile.label(),
                        "sampling_mode": state.apple_fm_workbench.sampling_mode.label(),
                        "bridge_reachable": state.apple_fm_execution.reachable,
                        "ready_model": state.apple_fm_execution.ready_model,
                        "last_error": state.apple_fm_workbench.last_error,
                    }),
                );
            }
            PaneKind::CadDemo => {
                let visible_variant_ids = state.cad_demo.visible_variant_ids();
                map.insert(
                    "cad".to_string(),
                    json!({
                        "session_id": state.cad_demo.session_id,
                        "document_revision": state.cad_demo.document_revision,
                        "design_profile": cad_design_profile_label(state.cad_demo.active_design_profile()),
                        "active_variant_id": state.cad_demo.active_variant_id,
                        "variant_materials": &state.cad_demo.variant_materials,
                        "viewport_layout": state.cad_demo.viewport_layout.label(),
                        "visible_variant_ids": visible_variant_ids,
                        "all_variants_visible": state.cad_demo.all_variants_visible(),
                        "drawing_view_mode": state.cad_demo.drawing_view_mode.label(),
                        "drawing_view_direction": state.cad_demo.drawing_view_direction.label(),
                        "drawing_hidden_lines": state.cad_demo.drawing_show_hidden_lines,
                        "drawing_dimensions": state.cad_demo.drawing_show_dimensions,
                        "drawing_zoom": state.cad_demo.drawing_zoom,
                        "drawing_pan": {
                            "x": state.cad_demo.drawing_pan_x,
                            "y": state.cad_demo.drawing_pan_y,
                        },
                        "drawing_detail_view_count": state.cad_demo.drawing_detail_views.len(),
                        "last_action": state.cad_demo.last_action,
                        "last_error": state.cad_demo.last_error,
                    }),
                );
            }
            PaneKind::SparkWallet | PaneKind::SparkCreateInvoice | PaneKind::SparkPayInvoice => {
                map.insert(
                    "wallet".to_string(),
                    json!({
                        "balance_sats": state
                            .spark_wallet
                            .balance
                            .as_ref()
                            .map(spark_total_balance_sats),
                        "last_error": state.spark_wallet.last_error,
                        "last_action": state.spark_wallet.last_action,
                    }),
                );
            }
            _ => {}
        }
    }

    payload
}

fn pane_action_to_hit_action(
    kind: PaneKind,
    action: &str,
    index: Option<usize>,
) -> Result<PaneHitAction, ToolBridgeResultEnvelope> {
    let require_index = |action_label: &str| -> Result<usize, ToolBridgeResultEnvelope> {
        index.ok_or_else(|| {
            ToolBridgeResultEnvelope::error(
                "OA-PANE-ACTION-MISSING-INDEX",
                format!(
                    "Action '{}' for pane '{}' requires an 'index' argument",
                    action_label,
                    pane_kind_key(kind)
                ),
                json!({
                    "pane": pane_kind_key(kind),
                    "action": action_label,
                }),
            )
        })
    };

    let unsupported = || {
        Err(ToolBridgeResultEnvelope::error(
            "OA-PANE-ACTION-UNSUPPORTED",
            format!(
                "Unsupported action '{}' for pane '{}'",
                action,
                pane_kind_key(kind)
            ),
            json!({
                "pane": pane_kind_key(kind),
                "action": action,
                "index": index,
            }),
        ))
    };

    match kind {
        PaneKind::ProjectOps => unsupported(),
        PaneKind::PsionicViz => unsupported(),
        PaneKind::BuyerRaceMatrix => unsupported(),
        PaneKind::LogStream => match action {
            "copy" | "copy_all" | "copy_logs" => Ok(PaneHitAction::LogStream(
                crate::pane_system::LogStreamPaneAction::CopyAll,
            )),
            _ => unsupported(),
        },
        PaneKind::BuyModePayments => match action {
            "toggle" | "arm" => Ok(PaneHitAction::BuyModePayments(
                crate::pane_system::BuyModePaymentsPaneAction::ToggleLoop,
            )),
            "copy" | "copy_all" | "copy_ledger" => Ok(PaneHitAction::BuyModePayments(
                crate::pane_system::BuyModePaymentsPaneAction::CopyAll,
            )),
            _ => unsupported(),
        },
        PaneKind::AutopilotChat => match action {
            "send" | "submit" => Ok(PaneHitAction::ChatSend),
            "refresh_threads" => Ok(PaneHitAction::ChatRefreshThreads),
            "new_thread" => Ok(PaneHitAction::ChatNewThread),
            "cycle_model" => Ok(PaneHitAction::ChatCycleModel),
            "interrupt" => Ok(PaneHitAction::ChatInterruptTurn),
            "select_thread" | "select_row" => {
                Ok(PaneHitAction::ChatSelectThread(require_index(action)?))
            }
            _ => unsupported(),
        },
        PaneKind::Calculator => unsupported(),
        PaneKind::GoOnline | PaneKind::ProviderControl => match action {
            "toggle" | "set_online" | "set_offline" => Ok(PaneHitAction::GoOnlineToggle),
            "buy_mode_test_job" | "buy_test_job" | "submit_buy_mode_request" => {
                Ok(PaneHitAction::BuyModePayments(
                    crate::pane_system::BuyModePaymentsPaneAction::ToggleLoop,
                ))
            }
            "open_local_model" | "open_workbench" | "warm_model" | "download_model"
            | "local_runtime" => Ok(PaneHitAction::ProviderControl(
                ProviderControlPaneAction::TriggerLocalRuntimeAction,
            )),
            "test_local_fm" | "run_local_fm_test" | "summarize_local_fm" => Ok(
                PaneHitAction::ProviderControl(ProviderControlPaneAction::RunLocalFmSummaryTest),
            ),
            "toggle_inventory" | "toggle_inventory_row" | "toggle_product" => {
                let index = require_index(action)?;
                let Some(target) = crate::app_state::ProviderInventoryProductToggleTarget::all()
                    .get(index)
                    .copied()
                else {
                    return unsupported();
                };
                Ok(PaneHitAction::ProviderControl(
                    ProviderControlPaneAction::ToggleInventory(target),
                ))
            }
            _ => unsupported(),
        },
        PaneKind::CodexAccount => match action {
            "refresh" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::Refresh)),
            "login_chatgpt" | "login" => Ok(PaneHitAction::CodexAccount(
                CodexAccountPaneAction::LoginChatgpt,
            )),
            "cancel_login" => Ok(PaneHitAction::CodexAccount(
                CodexAccountPaneAction::CancelLogin,
            )),
            "logout" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::Logout)),
            "rate_limits" => Ok(PaneHitAction::CodexAccount(
                CodexAccountPaneAction::RateLimits,
            )),
            _ => unsupported(),
        },
        PaneKind::CodexModels => match action {
            "refresh" => Ok(PaneHitAction::CodexModels(CodexModelsPaneAction::Refresh)),
            "toggle_hidden" => Ok(PaneHitAction::CodexModels(
                CodexModelsPaneAction::ToggleHidden,
            )),
            _ => unsupported(),
        },
        PaneKind::CodexConfig => match action {
            "read" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::Read)),
            "requirements" => Ok(PaneHitAction::CodexConfig(
                CodexConfigPaneAction::Requirements,
            )),
            "write_sample" => Ok(PaneHitAction::CodexConfig(
                CodexConfigPaneAction::WriteSample,
            )),
            "batch_write_sample" => Ok(PaneHitAction::CodexConfig(
                CodexConfigPaneAction::BatchWriteSample,
            )),
            "detect_external" => Ok(PaneHitAction::CodexConfig(
                CodexConfigPaneAction::DetectExternal,
            )),
            "import_external" => Ok(PaneHitAction::CodexConfig(
                CodexConfigPaneAction::ImportExternal,
            )),
            _ => unsupported(),
        },
        PaneKind::CodexMcp => match action {
            "refresh" => Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::Refresh)),
            "login_selected" | "login" => {
                Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::LoginSelected))
            }
            "reload" => Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::Reload)),
            "select_row" | "select_server" => Ok(PaneHitAction::CodexMcp(
                CodexMcpPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::CodexApps => match action {
            "refresh" => Ok(PaneHitAction::CodexApps(CodexAppsPaneAction::Refresh)),
            "select_row" | "select_app" => Ok(PaneHitAction::CodexApps(
                CodexAppsPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::CodexLabs => match action {
            "review_inline" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::ReviewInline)),
            "review_detached" => Ok(PaneHitAction::CodexLabs(
                CodexLabsPaneAction::ReviewDetached,
            )),
            "command_exec" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::CommandExec)),
            "collaboration_modes" => Ok(PaneHitAction::CodexLabs(
                CodexLabsPaneAction::CollaborationModes,
            )),
            "experimental_features" => Ok(PaneHitAction::CodexLabs(
                CodexLabsPaneAction::ExperimentalFeatures,
            )),
            "toggle_experimental" => Ok(PaneHitAction::CodexLabs(
                CodexLabsPaneAction::ToggleExperimental,
            )),
            "realtime_start" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeStart)),
            "realtime_append_text" => Ok(PaneHitAction::CodexLabs(
                CodexLabsPaneAction::RealtimeAppendText,
            )),
            "realtime_stop" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeStop)),
            "windows_sandbox_setup" => Ok(PaneHitAction::CodexLabs(
                CodexLabsPaneAction::WindowsSandboxSetup,
            )),
            "fuzzy_start" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyStart)),
            "fuzzy_update" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyUpdate)),
            "fuzzy_stop" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyStop)),
            _ => unsupported(),
        },
        PaneKind::CodexDiagnostics => match action {
            "enable_wire_log" => Ok(PaneHitAction::CodexDiagnostics(
                CodexDiagnosticsPaneAction::EnableWireLog,
            )),
            "disable_wire_log" => Ok(PaneHitAction::CodexDiagnostics(
                CodexDiagnosticsPaneAction::DisableWireLog,
            )),
            "clear_events" => Ok(PaneHitAction::CodexDiagnostics(
                CodexDiagnosticsPaneAction::ClearEvents,
            )),
            _ => unsupported(),
        },
        PaneKind::EarningsScoreboard => match action {
            "refresh" => Ok(PaneHitAction::EarningsScoreboard(
                EarningsScoreboardPaneAction::Refresh,
            )),
            "open_job_inbox" | "open_inbox" => Ok(PaneHitAction::EarningsScoreboard(
                EarningsScoreboardPaneAction::OpenJobInbox,
            )),
            "open_active_job" => Ok(PaneHitAction::EarningsScoreboard(
                EarningsScoreboardPaneAction::OpenActiveJob,
            )),
            "open_job_history" | "open_history" => Ok(PaneHitAction::EarningsScoreboard(
                EarningsScoreboardPaneAction::OpenJobHistory,
            )),
            _ => unsupported(),
        },
        PaneKind::RelayConnections => match action {
            "add_relay" => Ok(PaneHitAction::RelayConnections(
                RelayConnectionsPaneAction::AddRelay,
            )),
            "remove_selected" => Ok(PaneHitAction::RelayConnections(
                RelayConnectionsPaneAction::RemoveSelected,
            )),
            "retry_selected" => Ok(PaneHitAction::RelayConnections(
                RelayConnectionsPaneAction::RetrySelected,
            )),
            "select_row" => Ok(PaneHitAction::RelayConnections(
                RelayConnectionsPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::SyncHealth => match action {
            "rebootstrap" => Ok(PaneHitAction::SyncHealth(SyncHealthPaneAction::Rebootstrap)),
            _ => unsupported(),
        },
        PaneKind::LocalInference => match action {
            "refresh" | "refresh_runtime" => Ok(PaneHitAction::LocalInference(
                LocalInferencePaneAction::RefreshRuntime,
            )),
            "warm" | "warm_model" | "load_model" => Ok(PaneHitAction::LocalInference(
                LocalInferencePaneAction::WarmModel,
            )),
            "unload" | "unload_model" => Ok(PaneHitAction::LocalInference(
                LocalInferencePaneAction::UnloadModel,
            )),
            "run" | "run_prompt" | "submit" => Ok(PaneHitAction::LocalInference(
                LocalInferencePaneAction::RunPrompt,
            )),
            _ => unsupported(),
        },
        PaneKind::AppleFmWorkbench => match action {
            "refresh" | "refresh_bridge" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RefreshBridge,
            )),
            "start" | "start_bridge" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::StartBridge,
            )),
            "create_session" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::CreateSession,
            )),
            "inspect_session" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::InspectSession,
            )),
            "reset_session" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::ResetSession,
            )),
            "delete_session" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::DeleteSession,
            )),
            "run_text" | "text" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RunText,
            )),
            "run_chat" | "chat" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RunChat,
            )),
            "run_session" | "session" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RunSession,
            )),
            "run_stream" | "stream" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RunStream,
            )),
            "run_structured" | "structured" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RunStructured,
            )),
            "export_transcript" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::ExportTranscript,
            )),
            "restore_transcript" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RestoreTranscript,
            )),
            "cycle_tool_profile" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::CycleToolProfile,
            )),
            "cycle_sampling_mode" => Ok(PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::CycleSamplingMode,
            )),
            _ => unsupported(),
        },
        PaneKind::NetworkRequests => match action {
            "submit" | "submit_request" => Ok(PaneHitAction::NetworkRequests(
                NetworkRequestsPaneAction::RequestQuotes,
            )),
            "accept_quote" | "accept_selected_quote" => Ok(PaneHitAction::NetworkRequests(
                NetworkRequestsPaneAction::AcceptSelectedQuote,
            )),
            "select_quote" => Ok(PaneHitAction::NetworkRequests(
                NetworkRequestsPaneAction::SelectQuote(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::StarterJobs => match action {
            "complete_selected" => Ok(PaneHitAction::StarterJobs(
                StarterJobsPaneAction::CompleteSelected,
            )),
            "toggle_kill_switch" => Ok(PaneHitAction::StarterJobs(
                StarterJobsPaneAction::ToggleKillSwitch,
            )),
            "select_row" => Ok(PaneHitAction::StarterJobs(
                StarterJobsPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::ReciprocalLoop => match action {
            "start" => Ok(PaneHitAction::ReciprocalLoop(
                ReciprocalLoopPaneAction::Start,
            )),
            "stop" => Ok(PaneHitAction::ReciprocalLoop(
                ReciprocalLoopPaneAction::Stop,
            )),
            "reset" => Ok(PaneHitAction::ReciprocalLoop(
                ReciprocalLoopPaneAction::Reset,
            )),
            _ => unsupported(),
        },
        PaneKind::ActivityFeed => match action {
            "refresh" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::Refresh)),
            "previous_page" | "prev_page" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::PreviousPage,
            )),
            "next_page" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::NextPage,
            )),
            "select_row" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SelectRow(require_index(action)?),
            )),
            "filter_all" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::All),
            )),
            "filter_chat" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Chat),
            )),
            "filter_cad" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Cad),
            )),
            "filter_job" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Job),
            )),
            "filter_wallet" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Wallet),
            )),
            "filter_network" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Network),
            )),
            "filter_sync" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Sync),
            )),
            "filter_sa" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Sa),
            )),
            "filter_skl" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Skl),
            )),
            "filter_ac" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Ac),
            )),
            "filter_nip90" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Nip90),
            )),
            _ => unsupported(),
        },
        PaneKind::AlertsRecovery => match action {
            "recover_selected" => Ok(PaneHitAction::AlertsRecovery(
                AlertsRecoveryPaneAction::RecoverSelected,
            )),
            "acknowledge_selected" => Ok(PaneHitAction::AlertsRecovery(
                AlertsRecoveryPaneAction::AcknowledgeSelected,
            )),
            "resolve_selected" => Ok(PaneHitAction::AlertsRecovery(
                AlertsRecoveryPaneAction::ResolveSelected,
            )),
            "select_row" => Ok(PaneHitAction::AlertsRecovery(
                AlertsRecoveryPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::Settings => match action {
            "save" => Ok(PaneHitAction::Settings(SettingsPaneAction::Save)),
            "reset_defaults" => Ok(PaneHitAction::Settings(SettingsPaneAction::ResetDefaults)),
            _ => unsupported(),
        },
        PaneKind::Credentials => match action {
            "add_custom" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::AddCustom)),
            "save_value" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::SaveValue)),
            "delete_or_clear" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::DeleteOrClear,
            )),
            "toggle_enabled" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::ToggleEnabled,
            )),
            "toggle_scope_codex" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::ToggleScopeCodex,
            )),
            "toggle_scope_spark" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::ToggleScopeSpark,
            )),
            "toggle_scope_skills" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::ToggleScopeSkills,
            )),
            "toggle_scope_global" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::ToggleScopeGlobal,
            )),
            "import_from_env" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::ImportFromEnv,
            )),
            "reload" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::Reload)),
            "select_row" => Ok(PaneHitAction::Credentials(
                CredentialsPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::JobInbox => match action {
            "accept_selected" => Ok(PaneHitAction::JobInbox(JobInboxPaneAction::AcceptSelected)),
            "reject_selected" => Ok(PaneHitAction::JobInbox(JobInboxPaneAction::RejectSelected)),
            "select_row" => Ok(PaneHitAction::JobInbox(JobInboxPaneAction::SelectRow(
                require_index(action)?,
            ))),
            _ => unsupported(),
        },
        PaneKind::ActiveJob => match action {
            "advance_stage" => Ok(PaneHitAction::ActiveJob(ActiveJobPaneAction::AdvanceStage)),
            "abort_job" => Ok(PaneHitAction::ActiveJob(ActiveJobPaneAction::AbortJob)),
            _ => unsupported(),
        },
        PaneKind::JobHistory => match action {
            "cycle_status_filter" => Ok(PaneHitAction::JobHistory(
                JobHistoryPaneAction::CycleStatusFilter,
            )),
            "cycle_time_range" => Ok(PaneHitAction::JobHistory(
                JobHistoryPaneAction::CycleTimeRange,
            )),
            "previous_page" => Ok(PaneHitAction::JobHistory(
                JobHistoryPaneAction::PreviousPage,
            )),
            "next_page" => Ok(PaneHitAction::JobHistory(JobHistoryPaneAction::NextPage)),
            _ => unsupported(),
        },
        PaneKind::NostrIdentity => match action {
            "regenerate" => Ok(PaneHitAction::NostrRegenerate),
            "reveal" => Ok(PaneHitAction::NostrReveal),
            "copy_secret" => Ok(PaneHitAction::NostrCopySecret),
            _ => unsupported(),
        },
        PaneKind::SparkWallet => match action {
            "refresh" => Ok(PaneHitAction::Spark(SparkPaneAction::Refresh)),
            "generate_spark_address" => {
                Ok(PaneHitAction::Spark(SparkPaneAction::GenerateSparkAddress))
            }
            "generate_bitcoin_address" => Ok(PaneHitAction::Spark(
                SparkPaneAction::GenerateBitcoinAddress,
            )),
            "copy_spark_address" => Ok(PaneHitAction::Spark(SparkPaneAction::CopySparkAddress)),
            "create_invoice" => Ok(PaneHitAction::Spark(SparkPaneAction::CreateInvoice)),
            "send_payment" => Ok(PaneHitAction::Spark(SparkPaneAction::SendPayment)),
            _ => unsupported(),
        },
        PaneKind::SparkCreateInvoice => match action {
            "create_invoice" => Ok(PaneHitAction::SparkCreateInvoice(
                CreateInvoicePaneAction::CreateInvoice,
            )),
            "copy_invoice" => Ok(PaneHitAction::SparkCreateInvoice(
                CreateInvoicePaneAction::CopyInvoice,
            )),
            _ => unsupported(),
        },
        PaneKind::SparkPayInvoice => match action {
            "send_payment" => Ok(PaneHitAction::SparkPayInvoice(
                PayInvoicePaneAction::SendPayment,
            )),
            _ => unsupported(),
        },
        PaneKind::AgentProfileState => match action {
            "publish_profile" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::PublishProfile,
            )),
            "publish_state" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::PublishState,
            )),
            "update_goals" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::UpdateGoals,
            )),
            "create_goal" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::CreateGoal,
            )),
            "start_goal" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::StartGoal,
            )),
            "abort_goal" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::AbortGoal,
            )),
            "inspect_goal_receipt" => Ok(PaneHitAction::AgentProfileState(
                AgentProfileStatePaneAction::InspectGoalReceipt,
            )),
            _ => unsupported(),
        },
        PaneKind::AgentScheduleTick => match action {
            "apply_schedule" => Ok(PaneHitAction::AgentScheduleTick(
                AgentScheduleTickPaneAction::ApplySchedule,
            )),
            "publish_manual_tick" => Ok(PaneHitAction::AgentScheduleTick(
                AgentScheduleTickPaneAction::PublishManualTick,
            )),
            "inspect_last_result" => Ok(PaneHitAction::AgentScheduleTick(
                AgentScheduleTickPaneAction::InspectLastResult,
            )),
            "toggle_os_scheduler_adapter" => Ok(PaneHitAction::AgentScheduleTick(
                AgentScheduleTickPaneAction::ToggleOsSchedulerAdapter,
            )),
            _ => unsupported(),
        },
        PaneKind::TrajectoryAudit => match action {
            "open_session" => Ok(PaneHitAction::TrajectoryAudit(
                TrajectoryAuditPaneAction::OpenSession,
            )),
            "cycle_step_filter" => Ok(PaneHitAction::TrajectoryAudit(
                TrajectoryAuditPaneAction::CycleStepFilter,
            )),
            "verify_trajectory_hash" => Ok(PaneHitAction::TrajectoryAudit(
                TrajectoryAuditPaneAction::VerifyTrajectoryHash,
            )),
            _ => unsupported(),
        },
        PaneKind::CastControl => match action {
            "refresh" | "refresh_status" => Ok(PaneHitAction::CastControl(
                CastControlPaneAction::RefreshStatus,
            )),
            "run_check" | "check" => {
                Ok(PaneHitAction::CastControl(CastControlPaneAction::RunCheck))
            }
            "run_prove" | "prove" => {
                Ok(PaneHitAction::CastControl(CastControlPaneAction::RunProve))
            }
            "run_sign" | "sign_broadcast" => Ok(PaneHitAction::CastControl(
                CastControlPaneAction::RunSignBroadcast,
            )),
            "run_inspect" | "inspect" => Ok(PaneHitAction::CastControl(
                CastControlPaneAction::RunInspect,
            )),
            "run_loop_once" | "loop_once" => Ok(PaneHitAction::CastControl(
                CastControlPaneAction::RunLoopOnce,
            )),
            "toggle_auto_loop" | "start_loop" | "stop_loop" => Ok(PaneHitAction::CastControl(
                CastControlPaneAction::ToggleAutoLoop,
            )),
            "toggle_broadcast_armed" | "toggle_broadcast" => Ok(PaneHitAction::CastControl(
                CastControlPaneAction::ToggleBroadcastArmed,
            )),
            _ => unsupported(),
        },
        PaneKind::SkillRegistry => match action {
            "discover_skills" => Ok(PaneHitAction::SkillRegistry(
                SkillRegistryPaneAction::DiscoverSkills,
            )),
            "inspect_manifest" => Ok(PaneHitAction::SkillRegistry(
                SkillRegistryPaneAction::InspectManifest,
            )),
            "install_selected_skill" => Ok(PaneHitAction::SkillRegistry(
                SkillRegistryPaneAction::InstallSelectedSkill,
            )),
            "select_row" => Ok(PaneHitAction::SkillRegistry(
                SkillRegistryPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::SkillTrustRevocation => match action {
            "refresh_trust" => Ok(PaneHitAction::SkillTrustRevocation(
                SkillTrustRevocationPaneAction::RefreshTrust,
            )),
            "inspect_attestations" => Ok(PaneHitAction::SkillTrustRevocation(
                SkillTrustRevocationPaneAction::InspectAttestations,
            )),
            "toggle_kill_switch" => Ok(PaneHitAction::SkillTrustRevocation(
                SkillTrustRevocationPaneAction::ToggleKillSwitch,
            )),
            "revoke_skill" => Ok(PaneHitAction::SkillTrustRevocation(
                SkillTrustRevocationPaneAction::RevokeSkill,
            )),
            _ => unsupported(),
        },
        PaneKind::CreditDesk => match action {
            "publish_intent" => Ok(PaneHitAction::CreditDesk(
                CreditDeskPaneAction::PublishIntent,
            )),
            "publish_offer" => Ok(PaneHitAction::CreditDesk(
                CreditDeskPaneAction::PublishOffer,
            )),
            "publish_envelope" => Ok(PaneHitAction::CreditDesk(
                CreditDeskPaneAction::PublishEnvelope,
            )),
            "authorize_spend" => Ok(PaneHitAction::CreditDesk(
                CreditDeskPaneAction::AuthorizeSpend,
            )),
            _ => unsupported(),
        },
        PaneKind::CreditSettlementLedger => match action {
            "verify_settlement" => Ok(PaneHitAction::CreditSettlementLedger(
                CreditSettlementLedgerPaneAction::VerifySettlement,
            )),
            "emit_default_notice" => Ok(PaneHitAction::CreditSettlementLedger(
                CreditSettlementLedgerPaneAction::EmitDefaultNotice,
            )),
            "emit_reputation_label" => Ok(PaneHitAction::CreditSettlementLedger(
                CreditSettlementLedgerPaneAction::EmitReputationLabel,
            )),
            _ => unsupported(),
        },
        PaneKind::CadDemo => match action {
            "bootstrap" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::BootstrapDemo)),
            "cycle_variant" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleVariant)),
            "toggle_gripper_jaw" | "animate_jaw" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleGripperJawAnimation,
            )),
            "toggle_viewport_layout" | "toggle_layout" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleViewportLayout,
            )),
            "reset_camera" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ResetCamera)),
            "toggle_drawing_mode" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleDrawingViewMode,
            )),
            "cycle_drawing_direction" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::CycleDrawingViewDirection,
            )),
            "toggle_drawing_hidden_lines" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleDrawingHiddenLines,
            )),
            "toggle_drawing_dimensions" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleDrawingDimensions,
            )),
            "reset_drawing_view" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ResetDrawingView)),
            "add_drawing_detail" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::AddDrawingDetailView,
            )),
            "clear_drawing_details" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ClearDrawingDetailViews,
            )),
            "toggle_projection" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleProjectionMode,
            )),
            "cycle_hidden_line_mode" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::CycleHiddenLineMode,
            )),
            "cycle_section_plane" => {
                Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleSectionPlane))
            }
            "step_section_offset" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::StepSectionPlaneOffset,
            )),
            "cycle_material" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::CycleMaterialPreset,
            )),
            "toggle_snap_grid" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapGrid)),
            "toggle_snap_origin" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapOrigin)),
            "toggle_snap_endpoint" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleSnapEndpoint,
            )),
            "toggle_snap_midpoint" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::ToggleSnapMidpoint,
            )),
            "timeline_select_prev" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::TimelineSelectPrev,
            )),
            "timeline_select_next" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::TimelineSelectNext,
            )),
            "select_timeline_row" => Ok(PaneHitAction::CadDemo(
                CadDemoPaneAction::SelectTimelineRow(require_index(action)?),
            )),
            "select_warning" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::SelectWarning(
                require_index(action)?,
            ))),
            _ => unsupported(),
        },
        PaneKind::ProviderStatus | PaneKind::Empty => unsupported(),
    }
}

fn apply_chat_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    if matches!(field, "composer" | "message" | "prompt") {
        state.chat_inputs.composer.set_value(value.to_string());
        return true;
    }
    false
}

fn apply_relay_connections_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    if field == "relay_url" {
        state
            .relay_connections_inputs
            .relay_url
            .set_value(value.to_string());
        return true;
    }
    false
}

fn apply_local_inference_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "prompt" => state
            .local_inference_inputs
            .prompt
            .set_value(value.to_string()),
        "requested_model" | "model" => state
            .local_inference_inputs
            .requested_model
            .set_value(value.to_string()),
        "max_tokens" => state
            .local_inference_inputs
            .max_tokens
            .set_value(value.to_string()),
        "temperature" => state
            .local_inference_inputs
            .temperature
            .set_value(value.to_string()),
        "top_k" => state
            .local_inference_inputs
            .top_k
            .set_value(value.to_string()),
        "top_p" => state
            .local_inference_inputs
            .top_p
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_apple_fm_workbench_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "instructions" => state
            .apple_fm_workbench_inputs
            .instructions
            .set_value(value.to_string()),
        "prompt" => state
            .apple_fm_workbench_inputs
            .prompt
            .set_value(value.to_string()),
        "model" | "requested_model" => state
            .apple_fm_workbench_inputs
            .model
            .set_value(value.to_string()),
        "session" | "session_id" => state
            .apple_fm_workbench_inputs
            .session_id
            .set_value(value.to_string()),
        "max_tokens" => state
            .apple_fm_workbench_inputs
            .max_tokens
            .set_value(value.to_string()),
        "temperature" => state
            .apple_fm_workbench_inputs
            .temperature
            .set_value(value.to_string()),
        "top" | "top_k" => state
            .apple_fm_workbench_inputs
            .top
            .set_value(value.to_string()),
        "probability_threshold" | "top_p" => state
            .apple_fm_workbench_inputs
            .probability_threshold
            .set_value(value.to_string()),
        "seed" => state
            .apple_fm_workbench_inputs
            .seed
            .set_value(value.to_string()),
        "schema" | "schema_json" => state
            .apple_fm_workbench_inputs
            .schema_json
            .set_value(value.to_string()),
        "transcript" | "transcript_json" => state
            .apple_fm_workbench_inputs
            .transcript_json
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_network_requests_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "compute_family" | "request_type" => state
            .network_requests_inputs
            .compute_family
            .set_value(value.to_string()),
        "preferred_backend" | "payload" => state
            .network_requests_inputs
            .preferred_backend
            .set_value(value.to_string()),
        "capability_constraints" | "skill_scope_id" => state
            .network_requests_inputs
            .capability_constraints
            .set_value(value.to_string()),
        "quantity" | "credit_envelope_ref" => state
            .network_requests_inputs
            .quantity
            .set_value(value.to_string()),
        "delivery_start_minutes" | "delivery_start" => state
            .network_requests_inputs
            .delivery_start_minutes
            .set_value(value.to_string()),
        "window_minutes" | "budget_sats" => state
            .network_requests_inputs
            .window_minutes
            .set_value(value.to_string()),
        "max_price_sats" | "timeout_seconds" => state
            .network_requests_inputs
            .max_price_sats
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_settings_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "relay_url" => state.settings_inputs.relay_url.set_value(value.to_string()),
        "wallet_default_send_sats" => state
            .settings_inputs
            .wallet_default_send_sats
            .set_value(value.to_string()),
        "provider_max_queue_depth" => state
            .settings_inputs
            .provider_max_queue_depth
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_credentials_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "variable_name" => state
            .credentials_inputs
            .variable_name
            .set_value(value.to_string()),
        "variable_value" => state
            .credentials_inputs
            .variable_value
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_job_history_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    if field == "search_job_id" {
        state
            .job_history_inputs
            .search_job_id
            .set_value(value.to_string());
        return true;
    }
    false
}

fn apply_spark_wallet_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "invoice_amount" => state
            .spark_inputs
            .invoice_amount
            .set_value(value.to_string()),
        "send_request" => state.spark_inputs.send_request.set_value(value.to_string()),
        "send_amount" => state.spark_inputs.send_amount.set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_create_invoice_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "amount_sats" => state
            .create_invoice_inputs
            .amount_sats
            .set_value(value.to_string()),
        "description" => state
            .create_invoice_inputs
            .description
            .set_value(value.to_string()),
        "expiry_seconds" => state
            .create_invoice_inputs
            .expiry_seconds
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_pay_invoice_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "payment_request" => state
            .pay_invoice_inputs
            .payment_request
            .set_value(value.to_string()),
        "amount_sats" => state
            .pay_invoice_inputs
            .amount_sats
            .set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn cad_warning_counts(cad_demo: &crate::app_state::CadDemoPaneState) -> (Value, Value) {
    let mut by_severity = std::collections::BTreeMap::<String, u64>::new();
    let mut by_code = std::collections::BTreeMap::<String, u64>::new();
    for warning in &cad_demo.warnings {
        *by_severity.entry(warning.severity.clone()).or_insert(0) += 1;
        *by_code.entry(warning.code.clone()).or_insert(0) += 1;
    }
    (json!(by_severity), json!(by_code))
}

fn metadata_bool(metadata: &std::collections::BTreeMap<String, String>, key: &str) -> Option<bool> {
    metadata.get(key).and_then(|value| match value.as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    })
}

fn metadata_f64(metadata: &std::collections::BTreeMap<String, String>, key: &str) -> Option<f64> {
    metadata
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
}

fn cad_kinematic_checkpoint(cad_demo: &crate::app_state::CadDemoPaneState) -> Value {
    let metadata = &cad_demo.analysis_snapshot.estimator_metadata;
    let profile = metadata
        .get("kinematic.profile")
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    json!({
        "profile": profile,
        "joint_limits_deg": {
            "min": metadata_f64(metadata, "kinematic.joint_min_deg"),
            "max": metadata_f64(metadata, "kinematic.joint_max_deg"),
            "span": metadata_f64(metadata, "kinematic.travel_span_deg"),
        },
        "nominal_pose_deg": metadata_f64(metadata, "kinematic.nominal_pose_deg"),
        "finger_spacing_mm": metadata_f64(metadata, "kinematic.finger_spacing_mm"),
        "route_clearance_margin_mm": metadata_f64(metadata, "kinematic.route_clearance_margin_mm"),
        "bend_radius_margin_mm": metadata_f64(metadata, "kinematic.bend_radius_margin_mm"),
        "joint_range_violation": metadata_bool(metadata, "kinematic.joint_range_violation").unwrap_or(false),
        "travel_limit_violation": metadata_bool(metadata, "kinematic.travel_limit_violation").unwrap_or(false),
        "routing_collision": metadata_bool(metadata, "kinematic.routing_collision").unwrap_or(false),
        "nominal_self_intersection": metadata_bool(metadata, "kinematic.nominal_self_intersection").unwrap_or(false),
        "nominal_range_valid": metadata_bool(metadata, "kinematic.nominal_range_valid").unwrap_or(true),
    })
}

fn cad_sensor_feedback_checkpoint(cad_demo: &crate::app_state::CadDemoPaneState) -> Value {
    let latest_readings = cad_demo
        .sensor_feedback_readings
        .iter()
        .map(|reading| {
            json!({
                "digit_id": reading.digit_id,
                "pressure_ratio": reading.pressure_ratio,
                "proximity_mm": reading.proximity_mm,
                "contact": reading.contact,
            })
        })
        .collect::<Vec<_>>();
    let trace = cad_demo
        .sensor_feedback_trace
        .iter()
        .rev()
        .take(8)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|sample| {
            json!({
                "document_revision": sample.document_revision,
                "pose_preset": sample.pose_preset,
                "average_pressure_ratio": sample.average_pressure_ratio,
                "minimum_proximity_mm": sample.minimum_proximity_mm,
                "contact_count": sample.contact_count,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "visualization_mode": cad_demo.sensor_visualization_mode.label(),
        "last_updated_revision": cad_demo.sensor_feedback_last_updated_revision,
        "latest_readings": latest_readings,
        "trace": trace,
    })
}

fn cad_build_session_checkpoint(cad_demo: &crate::app_state::CadDemoPaneState) -> Value {
    if cad_demo.build_session.phase != crate::app_state::CadBuildSessionPhase::Idle {
        return json!({
            "phase": cad_demo.build_session.phase.label(),
            "failure_class": cad_demo.build_session.failure_class.map(|class| class.label().to_string()),
            "retry_attempts": cad_demo.build_session.retry_attempts,
            "retry_limit": cad_demo.build_session.retry_limit,
            "thread_id": cad_demo.build_session.thread_id,
            "turn_id": cad_demo.build_session.turn_id,
            "latest_tool_result": cad_demo.build_session.latest_tool_result,
            "latest_rebuild_result": cad_demo.build_session.latest_rebuild_result,
            "failure_reason": cad_demo.build_session.failure_reason,
            "remediation_hint": cad_demo.build_session.remediation_hint,
        });
    }
    if let Some(last) = cad_demo.last_build_session.as_ref() {
        return json!({
            "phase": last.terminal_phase.label(),
            "failure_class": last.failure_class.map(|class| class.label().to_string()),
            "retry_attempts": last.retry_attempts,
            "retry_limit": last.retry_limit,
            "thread_id": last.thread_id,
            "turn_id": last.turn_id,
            "latest_tool_result": last.latest_tool_result,
            "latest_rebuild_result": last.latest_rebuild_result,
            "failure_reason": last.failure_reason,
            "remediation_hint": last.remediation_hint,
        });
    }
    json!({
        "phase": crate::app_state::CadBuildSessionPhase::Idle.label(),
        "failure_class": null,
        "retry_attempts": 0,
        "retry_limit": 0,
        "thread_id": null,
        "turn_id": null,
        "latest_tool_result": null,
        "latest_rebuild_result": null,
        "failure_reason": null,
        "remediation_hint": null,
    })
}

fn cad_checkpoint_payload(
    cad_demo: &crate::app_state::CadDemoPaneState,
    thread_id: Option<&str>,
    source: &str,
) -> Value {
    let (warnings_by_severity, warnings_by_code) = cad_warning_counts(cad_demo);
    let analysis = &cad_demo.analysis_snapshot;
    let design_profile = cad_demo.active_design_profile();
    let visible_variant_ids = cad_demo.visible_variant_ids();
    let last_receipt = cad_demo.last_rebuild_receipt.as_ref().map_or_else(
        || {
            json!({
                "event_id": null,
                "document_revision": null,
                "variant_id": null,
                "rebuild_hash": null,
                "mesh_hash": null,
                "duration_ms": null,
            })
        },
        |receipt| {
            json!({
                "event_id": receipt.event_id,
                "document_revision": receipt.document_revision,
                "variant_id": receipt.variant_id,
                "rebuild_hash": receipt.rebuild_hash,
                "mesh_hash": receipt.mesh_hash,
                "duration_ms": receipt.duration_ms,
            })
        },
    );

    json!({
        "schema_version": CAD_CHECKPOINT_SCHEMA_VERSION,
        "source": source,
        "thread_id": thread_id,
        "document": {
            "id": cad_demo.document_id,
            "revision": cad_demo.document_revision,
        },
        "variant": {
            "active_id": cad_demo.active_variant_id,
            "active_tile_index": cad_demo.active_variant_tile_index,
            "all_ids": cad_demo.variant_ids,
            "visible_ids": visible_variant_ids,
            "all_visible": cad_demo.all_variants_visible(),
            "materials": &cad_demo.variant_materials,
        },
        "design_profile": cad_design_profile_label(design_profile),
        "context": {
            "session_id": cad_demo.session_id,
            "active_chat_session_id": cad_demo.active_chat_session_id,
            "dispatch_session_count": cad_demo.dispatch_sessions.len(),
            "viewport_layout": cad_demo.viewport_layout.label(),
        },
        "pending_rebuild": {
            "is_pending": cad_demo.pending_rebuild_request_id.is_some(),
            "request_id": cad_demo.pending_rebuild_request_id,
        },
        "warnings": {
            "total": cad_demo.warnings.len(),
            "by_severity": warnings_by_severity,
            "by_code": warnings_by_code,
        },
        "failure_metrics": {
            "tool_transport_failures": cad_demo.build_failure_metrics.tool_transport_failures,
            "intent_parse_failures": cad_demo.build_failure_metrics.intent_parse_failures,
            "dispatch_rebuild_failures": cad_demo.build_failure_metrics.dispatch_rebuild_failures,
            "tool_transport_retries": cad_demo.build_failure_metrics.tool_transport_retries,
            "intent_parse_retries": cad_demo.build_failure_metrics.intent_parse_retries,
            "dispatch_rebuild_retries": cad_demo.build_failure_metrics.dispatch_rebuild_retries,
            "terminal_failures": cad_demo.build_failure_metrics.terminal_failures,
        },
        "analysis": {
            "variant_id": analysis.variant_id,
            "material_id": analysis.material_id,
            "volume_mm3": analysis.volume_mm3,
            "mass_kg": analysis.mass_kg,
            "estimated_cost_usd": analysis.estimated_cost_usd,
            "max_deflection_mm": analysis.max_deflection_mm,
            "center_of_gravity_mm": analysis.center_of_gravity_mm,
        },
        "kinematics": cad_kinematic_checkpoint(cad_demo),
        "sensor_feedback": cad_sensor_feedback_checkpoint(cad_demo),
        "gripper_parameters": if matches!(
            design_profile,
            openagents_cad::dispatch::CadDesignProfile::ParallelJawGripper
                | openagents_cad::dispatch::CadDesignProfile::ParallelJawGripperUnderactuated
                | openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb
                | openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1
        ) {
            cad_gripper_parameter_summary(cad_demo, design_profile)
        } else {
            json!({})
        },
        "build_session": cad_build_session_checkpoint(cad_demo),
        "last_rebuild_receipt": last_receipt,
        "last_action": cad_demo.last_action,
        "last_error": cad_demo.last_error,
    })
}

fn cad_failure_class_label(class: CadBuildFailureClass) -> &'static str {
    class.label()
}

fn cad_design_profile_label(profile: openagents_cad::dispatch::CadDesignProfile) -> &'static str {
    match profile {
        openagents_cad::dispatch::CadDesignProfile::Rack => "rack",
        openagents_cad::dispatch::CadDesignProfile::ParallelJawGripper => "parallel_jaw_gripper",
        openagents_cad::dispatch::CadDesignProfile::ParallelJawGripperUnderactuated => {
            "parallel_jaw_gripper_underactuated"
        }
        openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb => "three_finger_thumb",
        openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1 => "humanoid_hand_v1",
    }
}

fn cad_gripper_parameter_summary(
    cad_demo: &crate::app_state::CadDemoPaneState,
    design_profile: openagents_cad::dispatch::CadDesignProfile,
) -> Value {
    let keys = [
        "jaw_open_mm",
        "finger_length_mm",
        "finger_thickness_mm",
        "base_width_mm",
        "base_depth_mm",
        "base_thickness_mm",
        "servo_mount_hole_diameter_mm",
        "print_fit_mm",
        "print_clearance_mm",
        "compliant_joint_count",
        "flexure_thickness_mm",
        "finger_count",
        "thumb_base_angle_deg",
        "tendon_channel_diameter_mm",
        "joint_min_deg",
        "joint_max_deg",
        "tendon_route_clearance_mm",
        "tendon_bend_radius_mm",
        "servo_envelope_length_mm",
        "servo_envelope_width_mm",
        "servo_envelope_height_mm",
        "servo_shaft_axis_offset_mm",
        "servo_mount_pattern_pitch_mm",
        "servo_bracket_thickness_mm",
        "servo_housing_wall_mm",
        "servo_standoff_diameter_mm",
        "gearbox_ratio",
        "gearbox_stage_diameter_mm",
        "gearbox_stage_length_mm",
        "wiring_channel_diameter_mm",
        "wiring_bend_radius_mm",
        "wiring_clearance_mm",
        "force_sensor_pad_diameter_mm",
        "proximity_sensor_port_diameter_mm",
        "control_board_mount_width_mm",
        "control_board_mount_depth_mm",
        "control_board_mount_height_mm",
        "modular_mount_slot_pitch_mm",
        "modular_mount_slot_count",
        "electrical_clearance_mm",
    ];
    let mut summary = serde_json::Map::new();
    for key in keys {
        if let Some(value) = cad_demo.dimension_value_mm(key) {
            summary.insert(key.to_string(), json!(value));
        }
    }
    let underactuated_mode = cad_demo
        .active_dispatch_state()
        .map(|dispatch| dispatch.underactuated_mode)
        .unwrap_or(matches!(
            design_profile,
            openagents_cad::dispatch::CadDesignProfile::ParallelJawGripperUnderactuated
                | openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb
                | openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1
        ));
    summary.insert("underactuated_mode".to_string(), json!(underactuated_mode));
    let single_servo_drive = cad_demo
        .active_dispatch_state()
        .map(|dispatch| dispatch.single_servo_drive)
        .unwrap_or(true);
    summary.insert("single_servo_drive".to_string(), json!(single_servo_drive));
    let opposable_thumb = cad_demo
        .active_dispatch_state()
        .map(|dispatch| dispatch.opposable_thumb)
        .unwrap_or(matches!(
            design_profile,
            openagents_cad::dispatch::CadDesignProfile::ThreeFingerThumb
                | openagents_cad::dispatch::CadDesignProfile::HumanoidHandV1
        ));
    summary.insert("opposable_thumb".to_string(), json!(opposable_thumb));
    let servo_integration_enabled = cad_demo
        .active_dispatch_state()
        .map(|dispatch| dispatch.servo_integration_enabled)
        .unwrap_or(false);
    let compact_servo_layout = cad_demo
        .active_dispatch_state()
        .map(|dispatch| dispatch.compact_servo_layout)
        .unwrap_or(false);
    summary.insert(
        "servo_integration_enabled".to_string(),
        json!(servo_integration_enabled),
    );
    summary.insert(
        "compact_servo_layout".to_string(),
        json!(compact_servo_layout),
    );
    if let Some(pose_preset) = cad_demo
        .active_dispatch_state()
        .and_then(|dispatch| dispatch.pose_preset.as_deref())
    {
        summary.insert("pose_preset".to_string(), json!(pose_preset));
    }
    if let Some(dispatch) = cad_demo.active_dispatch_state() {
        if let Some(value) = dispatch.compliant_joint_count {
            summary.insert("compliant_joint_count".to_string(), json!(value));
        }
        if let Some(value) = dispatch.flexure_thickness_mm {
            summary.insert("flexure_thickness_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.joint_min_deg {
            summary.insert("joint_min_deg".to_string(), json!(value));
        }
        if let Some(value) = dispatch.joint_max_deg {
            summary.insert("joint_max_deg".to_string(), json!(value));
        }
        if let Some(value) = dispatch.tendon_route_clearance_mm {
            summary.insert("tendon_route_clearance_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.tendon_bend_radius_mm {
            summary.insert("tendon_bend_radius_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_envelope_length_mm {
            summary.insert("servo_envelope_length_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_envelope_width_mm {
            summary.insert("servo_envelope_width_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_envelope_height_mm {
            summary.insert("servo_envelope_height_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_shaft_axis_offset_mm {
            summary.insert("servo_shaft_axis_offset_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_mount_pattern_pitch_mm {
            summary.insert("servo_mount_pattern_pitch_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_bracket_thickness_mm {
            summary.insert("servo_bracket_thickness_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_housing_wall_mm {
            summary.insert("servo_housing_wall_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.servo_standoff_diameter_mm {
            summary.insert("servo_standoff_diameter_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch.parameter_values.get("gearbox_ratio").copied() {
            summary.insert("gearbox_ratio".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("gearbox_stage_diameter_mm")
            .copied()
        {
            summary.insert("gearbox_stage_diameter_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("gearbox_stage_length_mm")
            .copied()
        {
            summary.insert("gearbox_stage_length_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("wiring_channel_diameter_mm")
            .copied()
        {
            summary.insert("wiring_channel_diameter_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("wiring_bend_radius_mm")
            .copied()
        {
            summary.insert("wiring_bend_radius_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("wiring_clearance_mm")
            .copied()
        {
            summary.insert("wiring_clearance_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("force_sensor_pad_diameter_mm")
            .copied()
        {
            summary.insert("force_sensor_pad_diameter_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("proximity_sensor_port_diameter_mm")
            .copied()
        {
            summary.insert(
                "proximity_sensor_port_diameter_mm".to_string(),
                json!(value),
            );
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("control_board_mount_width_mm")
            .copied()
        {
            summary.insert("control_board_mount_width_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("control_board_mount_depth_mm")
            .copied()
        {
            summary.insert("control_board_mount_depth_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("control_board_mount_height_mm")
            .copied()
        {
            summary.insert("control_board_mount_height_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("modular_mount_slot_pitch_mm")
            .copied()
        {
            summary.insert("modular_mount_slot_pitch_mm".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("modular_mount_slot_count")
            .copied()
        {
            summary.insert("modular_mount_slot_count".to_string(), json!(value));
        }
        if let Some(value) = dispatch
            .parameter_values
            .get("electrical_clearance_mm")
            .copied()
        {
            summary.insert("electrical_clearance_mm".to_string(), json!(value));
        }
    }
    Value::Object(summary)
}

fn cad_parse_retry_prompt(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first = trimmed.find('{')?;
    let last = trimmed.rfind('}')?;
    if first >= last {
        return None;
    }
    let candidate = &trimmed[first..=last];
    let value = serde_json::from_str::<Value>(candidate).ok()?;
    if !value.is_object() {
        return None;
    }
    serde_json::to_string(&value).ok()
}

fn execute_cad_intent(state: &mut RenderState, args: &CadIntentArgs) -> ToolBridgeResultEnvelope {
    let thread_id = args
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| state.autopilot_chat.active_thread_id.clone())
        .unwrap_or_else(|| "autopilot.tool.local".to_string());

    if !env_flag_enabled(CAD_INTENT_TOOL_ENABLED_ENV, true) {
        return ToolBridgeResultEnvelope::error(
            "OA-CAD-INTENT-DISABLED",
            "CAD intent tool is disabled by operator flag",
            json!({
                "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                "failure_class": cad_failure_class_label(CadBuildFailureClass::DispatchRebuild),
                "thread_id": thread_id,
                "flag": {
                    "name": CAD_INTENT_TOOL_ENABLED_ENV,
                    "value": false,
                },
                "fallback": {
                    "strategy": "safe_abort",
                    "remediation_hint": "set OPENAGENTS_CAD_INTENT_TOOL_ENABLED=1 to re-enable CAD intent tool execution",
                },
                "checkpoint": cad_checkpoint_payload(
                    &state.cad_demo,
                    Some(thread_id.as_str()),
                    OPENAGENTS_TOOL_CAD_INTENT,
                ),
            }),
        );
    }

    let mut prompt = if let Some(intent_json) = args.intent_json.as_ref() {
        match serde_json::to_string(intent_json) {
            Ok(value) => value,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-CAD-INTENT-JSON-SERIALIZE-FAILED",
                    format!("Failed to serialize intent_json: {error}"),
                    json!({
                        "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                        "failure_class": cad_failure_class_label(CadBuildFailureClass::IntentParseValidation),
                        "thread_id": thread_id,
                        "checkpoint": cad_checkpoint_payload(
                            &state.cad_demo,
                            Some(thread_id.as_str()),
                            OPENAGENTS_TOOL_CAD_INTENT,
                        ),
                    }),
                );
            }
        }
    } else if let Some(prompt) = args.prompt.as_ref() {
        prompt.trim().to_string()
    } else {
        String::new()
    };

    if prompt.is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-CAD-INTENT-MISSING-PAYLOAD",
            format!(
                "Provide either non-empty `prompt` or `intent_json` for {}",
                OPENAGENTS_TOOL_CAD_INTENT
            ),
            json!({
                "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                "failure_class": cad_failure_class_label(CadBuildFailureClass::IntentParseValidation),
                "thread_id": thread_id,
                "fallback": {
                    "strategy": "request_clarification",
                    "strict_intent_json_required": true,
                    "remediation_hint": format!(
                        "retry {} with explicit intent_json payload",
                        OPENAGENTS_TOOL_CAD_INTENT
                    ),
                },
                "checkpoint": cad_checkpoint_payload(
                    &state.cad_demo,
                    Some(thread_id.as_str()),
                    OPENAGENTS_TOOL_CAD_INTENT,
                ),
            }),
        );
    }

    let _ = PaneController::create_for_kind(state, PaneKind::CadDemo);
    let mut parse_retry_count = 0u8;
    loop {
        let outcome = super::reducers::apply_chat_prompt_to_cad_session_with_trigger_outcome(
            state,
            &thread_id,
            &prompt,
            Some("ai-intent"),
        );
        match outcome {
            super::reducers::CadChatPromptApplyOutcome::Applied {
                intent_name,
                rebuild_trigger,
            } => {
                return ToolBridgeResultEnvelope::ok(
                    "OA-CAD-INTENT-OK",
                    "Applied CAD intent prompt through CAD chat adapter",
                    json!({
                        "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                        "thread_id": thread_id,
                        "intent_name": intent_name,
                        "rebuild_trigger": rebuild_trigger,
                        "rebuild_trigger_prefix": "ai-intent",
                        "retries": {
                            "parse_retry_count": parse_retry_count,
                            "parse_retry_limit": CAD_INTENT_PARSE_RETRY_LIMIT,
                        },
                        "checkpoint": cad_checkpoint_payload(
                            &state.cad_demo,
                            Some(thread_id.as_str()),
                            OPENAGENTS_TOOL_CAD_INTENT,
                        ),
                    }),
                );
            }
            super::reducers::CadChatPromptApplyOutcome::ParseFailure {
                error_code,
                error_message,
                recovery_prompt,
            } => {
                if parse_retry_count < CAD_INTENT_PARSE_RETRY_LIMIT
                    && let Some(retry_prompt) = cad_parse_retry_prompt(&prompt)
                {
                    parse_retry_count = parse_retry_count.saturating_add(1);
                    state.cad_demo.record_agent_build_retry_metric(
                        CadBuildFailureClass::IntentParseValidation,
                    );
                    prompt = retry_prompt;
                    continue;
                }
                return ToolBridgeResultEnvelope::error(
                    "OA-CAD-INTENT-PARSE-FAILED",
                    format!("CAD intent parse failed: {error_message}"),
                    json!({
                        "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                        "failure_class": cad_failure_class_label(CadBuildFailureClass::IntentParseValidation),
                        "thread_id": thread_id,
                        "prompt": prompt,
                        "error_code": error_code,
                        "error_message": error_message,
                        "retries": {
                            "parse_retry_count": parse_retry_count,
                            "parse_retry_limit": CAD_INTENT_PARSE_RETRY_LIMIT,
                        },
                        "fallback": {
                            "strategy": "request_clarification",
                            "strict_intent_json_required": true,
                            "clarification_prompt": recovery_prompt,
                            "remediation_hint": format!(
                                "retry {} with explicit intent_json payload matching CadIntent schema",
                                OPENAGENTS_TOOL_CAD_INTENT
                            ),
                        },
                        "checkpoint": cad_checkpoint_payload(
                            &state.cad_demo,
                            Some(thread_id.as_str()),
                            OPENAGENTS_TOOL_CAD_INTENT,
                        ),
                    }),
                );
            }
            super::reducers::CadChatPromptApplyOutcome::DispatchFailure { intent_name, error } => {
                return ToolBridgeResultEnvelope::error(
                    "OA-CAD-INTENT-DISPATCH-FAILED",
                    "CAD dispatch failed; aborting this CAD turn safely",
                    json!({
                        "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                        "failure_class": cad_failure_class_label(CadBuildFailureClass::DispatchRebuild),
                        "thread_id": thread_id,
                        "intent_name": intent_name,
                        "error": error,
                        "fallback": {
                            "strategy": "safe_abort",
                            "remediation_hint": "retry with stricter intent_json or narrower parameter changes",
                        },
                        "checkpoint": cad_checkpoint_payload(
                            &state.cad_demo,
                            Some(thread_id.as_str()),
                            OPENAGENTS_TOOL_CAD_INTENT,
                        ),
                    }),
                );
            }
            super::reducers::CadChatPromptApplyOutcome::RebuildEnqueueFailure {
                intent_name,
                trigger,
                error,
                retry_attempts,
                retry_limit,
            } => {
                return ToolBridgeResultEnvelope::error(
                    "OA-CAD-INTENT-REBUILD-ENQUEUE-FAILED",
                    "CAD rebuild enqueue failed after bounded retries",
                    json!({
                        "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                        "failure_class": cad_failure_class_label(CadBuildFailureClass::DispatchRebuild),
                        "thread_id": thread_id,
                        "intent_name": intent_name,
                        "rebuild_trigger": trigger,
                        "error": error,
                        "retries": {
                            "dispatch_retry_count": retry_attempts,
                            "dispatch_retry_limit": retry_limit,
                        },
                        "fallback": {
                            "strategy": "safe_abort",
                            "remediation_hint": "retry when CAD rebuild worker is healthy or simplify the model mutation",
                        },
                        "checkpoint": cad_checkpoint_payload(
                            &state.cad_demo,
                            Some(thread_id.as_str()),
                            OPENAGENTS_TOOL_CAD_INTENT,
                        ),
                    }),
                );
            }
            super::reducers::CadChatPromptApplyOutcome::IgnoredNonCadPrompt => {
                return ToolBridgeResultEnvelope::error(
                    "OA-CAD-INTENT-NO-CHANGE",
                    "CAD intent prompt did not produce a CAD mutation",
                    json!({
                        "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
                        "failure_class": cad_failure_class_label(CadBuildFailureClass::IntentParseValidation),
                        "thread_id": thread_id,
                        "prompt": prompt,
                        "fallback": {
                            "strategy": "request_clarification",
                            "strict_intent_json_required": true,
                            "remediation_hint": "use an explicit CAD instruction or pass intent_json",
                        },
                        "checkpoint": cad_checkpoint_payload(
                            &state.cad_demo,
                            Some(thread_id.as_str()),
                            OPENAGENTS_TOOL_CAD_INTENT,
                        ),
                    }),
                );
            }
        }
    }
}

fn execute_cad_action(state: &mut RenderState, args: &CadActionArgs) -> ToolBridgeResultEnvelope {
    let action_key = normalize_key(&args.action);
    let is_snapshot_request = matches!(action_key.as_str(), "snapshot" | "status");
    let action = match cad_action_from_key(action_key.as_str(), args.index) {
        Ok(value) => value,
        Err(error) => return error,
    };

    let _ = PaneController::create_for_kind(state, PaneKind::CadDemo);
    let changed = super::reducers::run_cad_demo_action(state, action);
    ToolBridgeResultEnvelope::ok(
        "OA-CAD-ACTION-OK",
        if is_snapshot_request {
            "Collected CAD checkpoint snapshot".to_string()
        } else {
            format!("Executed CAD action '{}'", args.action)
        },
        json!({
            "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
            "action": args.action,
            "action_key": action_key,
            "index": args.index,
            "changed": changed,
            "checkpoint": cad_checkpoint_payload(
                &state.cad_demo,
                state.autopilot_chat.active_thread_id.as_deref(),
                OPENAGENTS_TOOL_CAD_ACTION,
            ),
        }),
    )
}

fn execute_swap_quote(state: &mut RenderState, args: &SwapQuoteArgs) -> ToolBridgeResultEnvelope {
    if args.goal_id.trim().is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-MISSING-GOAL",
            "goal_id is required",
            json!({ "goal_id": args.goal_id }),
        );
    }
    if args.request_id.trim().is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-MISSING-REQUEST",
            "request_id is required",
            json!({ "request_id": args.request_id }),
        );
    }

    let direction = match parse_swap_direction(&args.direction) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let unit = match parse_swap_unit(&args.unit) {
        Ok(value) => value,
        Err(error) => return error,
    };
    if let Some(error) = validate_direction_unit(direction, unit) {
        return error;
    }

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let quote_ttl_seconds = args.quote_ttl_seconds.unwrap_or(60).max(1);
    let goal_id = args.goal_id.trim();
    let request_id = args.request_id.trim();

    let (daily_converted_sats, daily_converted_cents) = state
        .autopilot_goals
        .document
        .swap_execution_receipts
        .iter()
        .filter(|receipt| receipt.goal_id == goal_id)
        .fold((0_u64, 0_u64), |(sats, cents), receipt| {
            match receipt.amount_in.unit {
                SwapAmountUnit::Sats => (sats.saturating_add(receipt.amount_in.amount), cents),
                SwapAmountUnit::Cents => (sats, cents.saturating_add(receipt.amount_in.amount)),
            }
        });

    let policy_request = crate::state::swap_contract::SwapExecutionRequest {
        request_id: request_id.to_string(),
        direction,
        amount: SwapAmount {
            amount: args.amount,
            unit,
        },
        quote_ttl_seconds,
        immediate_execution: args.immediate_execution,
        max_fee_sats_override: None,
        max_slippage_bps_override: None,
    };
    if let Err(error) = state.autopilot_goals.validate_swap_request_policy(
        goal_id,
        &policy_request,
        daily_converted_sats,
        daily_converted_cents,
    ) {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-POLICY-REJECTED",
            format!("Swap quote policy rejected request: {error}"),
            json!({
                "goal_id": goal_id,
                "request_id": request_id,
            }),
        );
    }

    let script_path = match resolve_blink_swap_script_path(state, BLINK_SWAP_QUOTE_SCRIPT) {
        Ok(path) => path,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-SWAP-QUOTE-BLINK-SCRIPT-MISSING",
                error,
                json!({
                    "goal_id": goal_id,
                    "request_id": request_id,
                    "script": BLINK_SWAP_QUOTE_SCRIPT,
                }),
            );
        }
    };

    let mut script_args = vec![
        swap_direction_script_value(direction).to_string(),
        args.amount.to_string(),
        "--unit".to_string(),
        swap_unit_script_value(unit).to_string(),
        "--ttl-seconds".to_string(),
        quote_ttl_seconds.to_string(),
    ];
    if args.immediate_execution {
        script_args.push("--immediate".to_string());
    }
    let command_provenance = SwapCommandProvenance {
        script_path: script_path.display().to_string(),
        args: script_args.clone(),
        executed_at_epoch_seconds: now_epoch_seconds,
        parse_version: BLINK_SWAP_PARSE_VERSION.to_string(),
    };
    let worker_request_id = state.stable_sats_simulation.reserve_worker_request_id();
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            worker_request_id,
            crate::app_state::StableSatsTreasuryOperationKind::SwapQuote,
            now_epoch_seconds,
            format!("queued swap quote goal={} request={}", goal_id, request_id),
        );
    let env_overrides = resolve_swap_runtime_env_overrides(state);
    let worker_request = crate::stablesats_blink_worker::StableSatsBlinkSwapQuoteRequest {
        request_id: worker_request_id,
        now_epoch_seconds,
        goal_id: goal_id.to_string(),
        adapter_request_id: request_id.to_string(),
        script_path,
        script_args: script_args.clone(),
        env_overrides,
    };
    if let Err(error) = state
        .stable_sats_blink_worker
        .enqueue_swap_quote(worker_request)
    {
        state
            .stable_sats_simulation
            .record_treasury_operation_finished(
                worker_request_id,
                crate::app_state::StableSatsTreasuryOperationKind::SwapQuote,
                crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                now_epoch_seconds,
                format!("swap quote enqueue failed: {error}"),
            );
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-WORKER-UNAVAILABLE",
            format!("Swap quote worker unavailable: {error}"),
            json!({
                "goal_id": goal_id,
                "request_id": request_id,
                "worker_request_id": worker_request_id,
            }),
        );
    }
    state.autopilot_chat.record_turn_timeline_event(format!(
        "swap quote queued goal={} request={} worker_request={}",
        goal_id, request_id, worker_request_id
    ));
    ToolBridgeResultEnvelope::ok(
        "OA-SWAP-QUOTE-QUEUED",
        "Swap quote queued on off-thread treasury worker",
        json!({
            "goal_id": goal_id,
            "request_id": request_id,
            "worker_request_id": worker_request_id,
            "status": "queued",
            "async": true,
            "provider": "blink_infrastructure",
            "command_provenance": {
                "script_path": command_provenance.script_path,
                "args": command_provenance.args,
                "executed_at_epoch_seconds": command_provenance.executed_at_epoch_seconds,
                "parse_version": command_provenance.parse_version,
            },
        }),
    )
}

fn execute_swap_execute(
    state: &mut RenderState,
    args: &SwapExecuteArgs,
) -> ToolBridgeResultEnvelope {
    if args.goal_id.trim().is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-MISSING-GOAL",
            "goal_id is required",
            json!({ "goal_id": args.goal_id }),
        );
    }
    if args.quote_id.trim().is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-MISSING-QUOTE",
            "quote_id is required",
            json!({ "quote_id": args.quote_id }),
        );
    }

    let goal_id = args.goal_id.trim();
    let quote_id = args.quote_id.trim();

    let audit = state
        .autopilot_goals
        .document
        .swap_quote_audits
        .iter()
        .rev()
        .find(|entry| entry.goal_id == goal_id && entry.quote_id == quote_id)
        .cloned();
    let Some(audit) = audit else {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-QUOTE-NOT-FOUND",
            format!(
                "No swap quote audit found for goal '{}' and quote '{}'",
                goal_id, quote_id
            ),
            json!({
                "goal_id": goal_id,
                "quote_id": quote_id,
            }),
        );
    };

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let script_path = match resolve_blink_swap_script_path(state, BLINK_SWAP_EXECUTE_SCRIPT) {
        Ok(path) => path,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-SWAP-EXECUTE-BLINK-SCRIPT-MISSING",
                error,
                json!({
                    "goal_id": goal_id,
                    "quote_id": quote_id,
                    "script": BLINK_SWAP_EXECUTE_SCRIPT,
                }),
            );
        }
    };

    let direction = swap_direction_script_value(audit.direction).to_string();
    let mut script_args = vec![
        direction,
        audit.amount_in.amount.to_string(),
        "--unit".to_string(),
        swap_unit_script_value(audit.amount_in.unit).to_string(),
    ];
    if let Some(memo) = args
        .memo
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        script_args.push("--memo".to_string());
        script_args.push(memo.to_string());
    }
    let mut command_provenance = SwapCommandProvenance {
        script_path: script_path.display().to_string(),
        args: script_args.clone(),
        executed_at_epoch_seconds: now_epoch_seconds,
        parse_version: BLINK_SWAP_PARSE_VERSION.to_string(),
    };
    let worker_request_id = state.stable_sats_simulation.reserve_worker_request_id();
    let caller_identity = state
        .nostr_identity
        .as_ref()
        .map(|identity| identity.npub.as_str())
        .unwrap_or("autopilot-desktop");
    let now_epoch_ms = now_epoch_seconds.saturating_mul(1_000) as i64;
    if let Err(error) = state.earn_kernel_receipts.record_swap_execute_attempt(
        caller_identity,
        goal_id,
        quote_id,
        worker_request_id,
        now_epoch_ms,
        "tool_bridge.swap_execute",
    ) {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-IDEMPOTENCY-CONFLICT",
            format!("Swap execute rejected by idempotency policy: {error}"),
            json!({
                "goal_id": goal_id,
                "quote_id": quote_id,
                "worker_request_id": worker_request_id,
                "reason_code": "IDEMPOTENCY_CONFLICT",
            }),
        );
    }
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            worker_request_id,
            crate::app_state::StableSatsTreasuryOperationKind::SwapExecute,
            now_epoch_seconds,
            format!("queued swap execute goal={} quote={}", goal_id, quote_id),
        );
    let env_overrides = resolve_swap_runtime_env_overrides(state);
    let worker_request = crate::stablesats_blink_worker::StableSatsBlinkSwapExecuteRequest {
        request_id: worker_request_id,
        now_epoch_seconds,
        goal_id: goal_id.to_string(),
        quote_id: quote_id.to_string(),
        script_path,
        script_args: script_args.clone(),
        env_overrides,
    };
    if let Err(error) = state
        .stable_sats_blink_worker
        .enqueue_swap_execute(worker_request)
    {
        state
            .stable_sats_simulation
            .record_treasury_operation_finished(
                worker_request_id,
                crate::app_state::StableSatsTreasuryOperationKind::SwapExecute,
                crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                now_epoch_seconds,
                format!("swap execute enqueue failed: {error}"),
            );
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-WORKER-UNAVAILABLE",
            format!("Swap execute worker unavailable: {error}"),
            json!({
                "goal_id": goal_id,
                "quote_id": quote_id,
                "worker_request_id": worker_request_id,
            }),
        );
    }
    command_provenance.executed_at_epoch_seconds = now_epoch_seconds;
    state.autopilot_chat.record_turn_timeline_event(format!(
        "swap execute queued goal={} quote={} worker_request={}",
        goal_id, quote_id, worker_request_id
    ));
    ToolBridgeResultEnvelope::ok(
        "OA-SWAP-EXECUTE-QUEUED",
        "Swap execution queued on off-thread treasury worker",
        json!({
            "goal_id": goal_id,
            "quote_id": quote_id,
            "worker_request_id": worker_request_id,
            "status": "queued",
            "async": true,
            "audit": {
                "provider": match audit.provider {
                    SwapQuoteProvider::BlinkInfrastructure => "blink_infrastructure",
                    SwapQuoteProvider::StablesatsQuoteService => "stablesats_quote_service",
                    SwapQuoteProvider::BlinkFallback => "blink_fallback",
                },
                "direction": format!("{:?}", audit.direction),
                "amount_in": {
                    "amount": audit.amount_in.amount,
                    "unit": match audit.amount_in.unit {
                        SwapAmountUnit::Sats => "sats",
                        SwapAmountUnit::Cents => "cents",
                    },
                },
                "amount_out": {
                    "amount": audit.amount_out.amount,
                    "unit": match audit.amount_out.unit {
                        SwapAmountUnit::Sats => "sats",
                        SwapAmountUnit::Cents => "cents",
                    },
                },
                "expires_at_epoch_seconds": audit.expires_at_epoch_seconds,
            },
            "command_provenance": {
                "script_path": command_provenance.script_path,
                "args": command_provenance.args,
                "executed_at_epoch_seconds": command_provenance.executed_at_epoch_seconds,
                "parse_version": command_provenance.parse_version,
            },
        }),
    )
}

fn execute_treasury_transfer(
    state: &mut RenderState,
    args: &TreasuryTransferArgs,
) -> ToolBridgeResultEnvelope {
    if state.stable_sats_simulation.mode != crate::app_state::StableSatsSimulationMode::RealBlink {
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-TRANSFER-REAL-MODE-REQUIRED",
            "StableSats treasury transfer requires real mode",
            json!({
                "mode": state.stable_sats_simulation.mode.label(),
            }),
        );
    }
    let from_owner_id = args.from_owner_id.trim();
    if from_owner_id.is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-TRANSFER-MISSING-FROM",
            "from_owner_id is required",
            json!({ "from_owner_id": args.from_owner_id }),
        );
    }
    let to_owner_id = args.to_owner_id.trim();
    if to_owner_id.is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-TRANSFER-MISSING-TO",
            "to_owner_id is required",
            json!({ "to_owner_id": args.to_owner_id }),
        );
    }
    if from_owner_id == to_owner_id {
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-TRANSFER-SAME-OWNER",
            "from_owner_id and to_owner_id must be different wallets",
            json!({
                "from_owner_id": from_owner_id,
                "to_owner_id": to_owner_id,
            }),
        );
    }

    let asset = match parse_treasury_transfer_asset(args.asset.as_str()) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let from_wallet = match resolve_stablesats_wallet_by_owner(
        state,
        from_owner_id,
        "OA-TREASURY-TRANSFER-FROM-NOT-FOUND",
    ) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let to_wallet = match resolve_stablesats_wallet_by_owner(
        state,
        to_owner_id,
        "OA-TREASURY-TRANSFER-TO-NOT-FOUND",
    ) {
        Ok(value) => value,
        Err(error) => return error,
    };

    let source_env_overrides = match resolve_wallet_blink_env_overrides(state, &from_wallet) {
        Ok(value) => value,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-TREASURY-TRANSFER-FROM-CREDENTIAL-ERROR",
                error,
                json!({
                    "owner_id": from_owner_id,
                    "wallet_name": from_wallet.agent_name,
                    "credential_key_name": from_wallet.credential_key_name,
                    "credential_url_name": from_wallet.credential_url_name,
                }),
            );
        }
    };
    let destination_env_overrides = match resolve_wallet_blink_env_overrides(state, &to_wallet) {
        Ok(value) => value,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-TREASURY-TRANSFER-TO-CREDENTIAL-ERROR",
                error,
                json!({
                    "owner_id": to_owner_id,
                    "wallet_name": to_wallet.agent_name,
                    "credential_key_name": to_wallet.credential_key_name,
                    "credential_url_name": to_wallet.credential_url_name,
                }),
            );
        }
    };

    let balance_script_path = match resolve_blink_swap_script_path(state, BLINK_BALANCE_SCRIPT) {
        Ok(path) => path,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-TREASURY-TRANSFER-BLINK-SCRIPT-MISSING",
                error,
                json!({ "script": BLINK_BALANCE_SCRIPT }),
            );
        }
    };
    let create_invoice_script_path =
        match resolve_blink_swap_script_path(state, BLINK_CREATE_INVOICE_SCRIPT) {
            Ok(path) => path,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-TREASURY-TRANSFER-BLINK-SCRIPT-MISSING",
                    error,
                    json!({ "script": BLINK_CREATE_INVOICE_SCRIPT }),
                );
            }
        };
    let create_invoice_usd_script_path =
        match resolve_blink_swap_script_path(state, BLINK_CREATE_INVOICE_USD_SCRIPT) {
            Ok(path) => path,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-TREASURY-TRANSFER-BLINK-SCRIPT-MISSING",
                    error,
                    json!({ "script": BLINK_CREATE_INVOICE_USD_SCRIPT }),
                );
            }
        };
    let fee_probe_script_path = match resolve_blink_swap_script_path(state, BLINK_FEE_PROBE_SCRIPT)
    {
        Ok(path) => path,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-TREASURY-TRANSFER-BLINK-SCRIPT-MISSING",
                error,
                json!({ "script": BLINK_FEE_PROBE_SCRIPT }),
            );
        }
    };
    let pay_invoice_script_path =
        match resolve_blink_swap_script_path(state, BLINK_PAY_INVOICE_SCRIPT) {
            Ok(path) => path,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-TREASURY-TRANSFER-BLINK-SCRIPT-MISSING",
                    error,
                    json!({ "script": BLINK_PAY_INVOICE_SCRIPT }),
                );
            }
        };

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let worker_request_id = state.stable_sats_simulation.reserve_worker_request_id();
    let operation_kind = match asset {
        crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::BtcSats => {
            crate::app_state::StableSatsTreasuryOperationKind::TransferBtc
        }
        crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::UsdCents => {
            crate::app_state::StableSatsTreasuryOperationKind::TransferUsd
        }
    };
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            worker_request_id,
            operation_kind,
            now_epoch_seconds,
            format!(
                "queued treasury transfer {} {} {} -> {}",
                args.amount,
                asset.label(),
                from_owner_id,
                to_owner_id
            ),
        );
    let request = crate::stablesats_blink_worker::StableSatsBlinkTransferRequest {
        request_id: worker_request_id,
        now_epoch_seconds,
        from_owner_id: from_owner_id.to_string(),
        from_wallet_name: from_wallet.agent_name.clone(),
        to_owner_id: to_owner_id.to_string(),
        to_wallet_name: to_wallet.agent_name.clone(),
        asset,
        amount: args.amount,
        memo: args
            .memo
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        source_env_overrides,
        destination_env_overrides,
        balance_script_path,
        create_invoice_script_path,
        create_invoice_usd_script_path,
        fee_probe_script_path,
        pay_invoice_script_path,
    };
    if let Err(error) = state.stable_sats_blink_worker.enqueue_transfer(request) {
        state
            .stable_sats_simulation
            .record_treasury_operation_finished(
                worker_request_id,
                operation_kind,
                crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                now_epoch_seconds,
                format!("treasury transfer enqueue failed: {error}"),
            );
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-TRANSFER-WORKER-UNAVAILABLE",
            format!("Treasury transfer worker unavailable: {error}"),
            json!({
                "worker_request_id": worker_request_id,
                "from_owner_id": from_owner_id,
                "to_owner_id": to_owner_id,
                "asset": asset.label(),
                "amount": args.amount,
            }),
        );
    }
    state.autopilot_chat.record_turn_timeline_event(format!(
        "treasury transfer queued from={} to={} asset={} amount={} worker_request={}",
        from_owner_id,
        to_owner_id,
        asset.label(),
        args.amount,
        worker_request_id
    ));
    ToolBridgeResultEnvelope::ok(
        "OA-TREASURY-TRANSFER-QUEUED",
        "Treasury transfer queued on off-thread worker",
        json!({
            "worker_request_id": worker_request_id,
            "status": "queued",
            "async": true,
            "provider": "blink_infrastructure",
            "from_owner_id": from_owner_id,
            "from_wallet_name": from_wallet.agent_name,
            "to_owner_id": to_owner_id,
            "to_wallet_name": to_wallet.agent_name,
            "asset": asset.label(),
            "amount": args.amount,
        }),
    )
}

fn execute_treasury_convert(
    state: &mut RenderState,
    args: &TreasuryConvertArgs,
) -> ToolBridgeResultEnvelope {
    if state.stable_sats_simulation.mode != crate::app_state::StableSatsSimulationMode::RealBlink {
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-CONVERT-REAL-MODE-REQUIRED",
            "StableSats treasury convert requires real mode",
            json!({
                "mode": state.stable_sats_simulation.mode.label(),
            }),
        );
    }
    let owner_id = args.owner_id.trim();
    if owner_id.is_empty() {
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-CONVERT-MISSING-OWNER",
            "owner_id is required",
            json!({ "owner_id": args.owner_id }),
        );
    }
    let direction = match parse_swap_direction(args.direction.as_str()) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let unit = match parse_swap_unit(args.unit.as_str()) {
        Ok(value) => value,
        Err(error) => return error,
    };
    if let Some(error) = validate_direction_unit(direction, unit) {
        return error;
    }

    let wallet = match resolve_stablesats_wallet_by_owner(
        state,
        owner_id,
        "OA-TREASURY-CONVERT-OWNER-NOT-FOUND",
    ) {
        Ok(value) => value,
        Err(error) => return error,
    };
    let env_overrides = match resolve_wallet_blink_env_overrides(state, &wallet) {
        Ok(value) => value,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-TREASURY-CONVERT-CREDENTIAL-ERROR",
                error,
                json!({
                    "owner_id": owner_id,
                    "wallet_name": wallet.agent_name,
                    "credential_key_name": wallet.credential_key_name,
                    "credential_url_name": wallet.credential_url_name,
                }),
            );
        }
    };
    let swap_quote_script_path =
        match resolve_blink_swap_script_path(state, BLINK_SWAP_QUOTE_SCRIPT) {
            Ok(path) => path,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-TREASURY-CONVERT-BLINK-SCRIPT-MISSING",
                    error,
                    json!({ "script": BLINK_SWAP_QUOTE_SCRIPT }),
                );
            }
        };
    let swap_execute_script_path =
        match resolve_blink_swap_script_path(state, BLINK_SWAP_EXECUTE_SCRIPT) {
            Ok(path) => path,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-TREASURY-CONVERT-BLINK-SCRIPT-MISSING",
                    error,
                    json!({ "script": BLINK_SWAP_EXECUTE_SCRIPT }),
                );
            }
        };

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let worker_request_id = state.stable_sats_simulation.reserve_worker_request_id();
    state
        .stable_sats_simulation
        .record_treasury_operation_queued(
            worker_request_id,
            crate::app_state::StableSatsTreasuryOperationKind::Convert,
            now_epoch_seconds,
            format!(
                "queued treasury convert owner={} direction={} amount={} {}",
                owner_id,
                swap_direction_script_value(direction),
                args.amount,
                swap_unit_script_value(unit)
            ),
        );
    let request = crate::stablesats_blink_worker::StableSatsBlinkConvertRequest {
        request_id: worker_request_id,
        now_epoch_seconds,
        owner_id: owner_id.to_string(),
        wallet_name: wallet.agent_name.clone(),
        direction: swap_direction_script_value(direction).to_string(),
        amount: args.amount,
        unit: swap_unit_script_value(unit).to_string(),
        memo: args
            .memo
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        env_overrides,
        swap_execute_script_path,
        swap_quote_script_path,
    };
    if let Err(error) = state.stable_sats_blink_worker.enqueue_convert(request) {
        state
            .stable_sats_simulation
            .record_treasury_operation_finished(
                worker_request_id,
                crate::app_state::StableSatsTreasuryOperationKind::Convert,
                crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                now_epoch_seconds,
                format!("treasury convert enqueue failed: {error}"),
            );
        return ToolBridgeResultEnvelope::error(
            "OA-TREASURY-CONVERT-WORKER-UNAVAILABLE",
            format!("Treasury convert worker unavailable: {error}"),
            json!({
                "worker_request_id": worker_request_id,
                "owner_id": owner_id,
                "direction": swap_direction_script_value(direction),
                "amount": args.amount,
                "unit": swap_unit_script_value(unit),
            }),
        );
    }
    state.autopilot_chat.record_turn_timeline_event(format!(
        "treasury convert queued owner={} direction={} amount={} {} worker_request={}",
        owner_id,
        swap_direction_script_value(direction),
        args.amount,
        swap_unit_script_value(unit),
        worker_request_id
    ));
    ToolBridgeResultEnvelope::ok(
        "OA-TREASURY-CONVERT-QUEUED",
        "Treasury convert queued on off-thread worker",
        json!({
            "worker_request_id": worker_request_id,
            "status": "queued",
            "async": true,
            "provider": "blink_infrastructure",
            "owner_id": owner_id,
            "wallet_name": wallet.agent_name,
            "direction": swap_direction_script_value(direction),
            "amount": args.amount,
            "unit": swap_unit_script_value(unit),
        }),
    )
}

fn execute_treasury_receipt(
    state: &RenderState,
    args: &TreasuryReceiptArgs,
) -> ToolBridgeResultEnvelope {
    if let Some(receipt) = state
        .stable_sats_simulation
        .treasury_receipts
        .iter()
        .rev()
        .find(|entry| entry.request_id == args.worker_request_id)
    {
        return ToolBridgeResultEnvelope::ok(
            "OA-TREASURY-RECEIPT-FOUND",
            "Treasury receipt resolved",
            json!({
                "worker_request_id": args.worker_request_id,
                "status": "completed",
                "kind": receipt.kind.label(),
                "occurred_at_epoch_seconds": receipt.occurred_at_epoch_seconds,
                "receipt": receipt.payload,
            }),
        );
    }

    if let Some(operation) = state
        .stable_sats_simulation
        .treasury_operations
        .iter()
        .rev()
        .find(|entry| entry.request_id == args.worker_request_id)
    {
        return ToolBridgeResultEnvelope::ok(
            "OA-TREASURY-RECEIPT-PENDING",
            "Treasury operation is known but has no receipt payload yet",
            json!({
                "worker_request_id": args.worker_request_id,
                "status": operation.status.label(),
                "kind": operation.kind.label(),
                "detail": operation.detail,
                "updated_at_epoch_seconds": operation.updated_at_epoch_seconds,
            }),
        );
    }

    ToolBridgeResultEnvelope::error(
        "OA-TREASURY-RECEIPT-NOT-FOUND",
        format!(
            "No treasury operation or receipt found for worker request {}",
            args.worker_request_id
        ),
        json!({
            "worker_request_id": args.worker_request_id,
            "known_operation_count": state.stable_sats_simulation.treasury_operations.len(),
            "known_receipt_count": state.stable_sats_simulation.treasury_receipts.len(),
        }),
    )
}

fn execute_goal_scheduler_tool(
    state: &mut RenderState,
    args: &GoalSchedulerToolArgs,
) -> ToolBridgeResultEnvelope {
    let action = normalize_key(&args.action);
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    match action.as_str() {
        "status" => {
            if let Some(goal_id) = args.goal_id.as_deref() {
                let Some(goal) = state
                    .autopilot_goals
                    .document
                    .active_goals
                    .iter()
                    .find(|goal| goal.goal_id == goal_id)
                else {
                    return ToolBridgeResultEnvelope::error(
                        "OA-GOAL-SCHEDULER-GOAL-NOT-FOUND",
                        format!("Goal '{}' not found", goal_id),
                        json!({ "goal_id": goal_id }),
                    );
                };
                let reconciliation = state
                    .goal_loop_executor
                    .active_run
                    .as_ref()
                    .filter(|run| run.goal_id == goal.goal_id)
                    .map(|run| {
                        let wallet_total_sats = state
                            .spark_wallet
                            .balance
                            .as_ref()
                            .map_or(0, spark_total_balance_sats);
                        reconcile_wallet_events_for_goal(
                            run.started_at_epoch_seconds,
                            run.initial_wallet_sats,
                            wallet_total_sats,
                            goal.goal_id.as_str(),
                            &state.job_history,
                            &state.spark_wallet,
                            &state.autopilot_goals.document.swap_execution_receipts,
                        )
                    });
                let latest_run_audit = state
                    .autopilot_goals
                    .document
                    .run_audit_receipts
                    .iter()
                    .rev()
                    .find(|audit| audit.goal_id == goal.goal_id)
                    .cloned();
                let rollout_gate = state.autopilot_goals.rollout_gate_decision();
                let rollout_metrics = state.autopilot_goals.rollout_metrics_snapshot();
                let rollout_health = state.autopilot_goals.rollout_health_report();
                return ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-STATUS-OK",
                    "Goal scheduler status read",
                    json!({
                        "goal_id": goal.goal_id,
                        "lifecycle_status": format!("{:?}", goal.lifecycle_status),
                        "policy": goal.constraints.policy_snapshot(),
                        "schedule": {
                            "enabled": goal.schedule.enabled,
                            "kind": format!("{:?}", goal.schedule.kind),
                            "next_run_epoch_seconds": goal.schedule.next_run_epoch_seconds,
                            "last_run_epoch_seconds": goal.schedule.last_run_epoch_seconds,
                            "missed_run_policy": format!("{:?}", goal.schedule.missed_run_policy),
                            "pending_catchup_runs": goal.schedule.pending_catchup_runs,
                            "last_recovery_epoch_seconds": goal.schedule.last_recovery_epoch_seconds,
                            "os_adapter": {
                                "enabled": goal.schedule.os_adapter.enabled,
                                "adapter": goal
                                    .schedule
                                    .os_adapter
                                    .adapter
                                    .map(|kind| kind.as_str().to_string()),
                                "descriptor_path": goal.schedule.os_adapter.descriptor_path,
                                "last_reconciled_epoch_seconds": goal
                                    .schedule
                                    .os_adapter
                                    .last_reconciled_epoch_seconds,
                                "last_reconcile_result": goal
                                    .schedule
                                    .os_adapter
                                    .last_reconcile_result,
                            },
                        },
                        "reconciliation": reconciliation.as_ref().map(|report| json!({
                            "wallet_delta_sats_raw": report.wallet_delta_sats_raw,
                            "wallet_delta_excluding_swaps_sats": report.wallet_delta_excluding_swaps_sats,
                            "earned_wallet_delta_sats": report.earned_wallet_delta_sats,
                            "swap_converted_out_sats": report.swap_converted_out_sats,
                            "swap_converted_in_sats": report.swap_converted_in_sats,
                            "swap_fee_sats": report.swap_fee_sats,
                            "non_swap_spend_sats": report.non_swap_spend_sats,
                            "unattributed_receive_sats": report.unattributed_receive_sats,
                            "total_swap_cents": report.total_swap_cents,
                            "events": report.events,
                        })),
                        "latest_run_audit": latest_run_audit,
                        "rollout": {
                            "config": state.autopilot_goals.document.rollout_config.clone(),
                            "gate": rollout_gate,
                            "metrics": rollout_metrics,
                            "health": rollout_health,
                        },
                    }),
                );
            }

            let active = state.autopilot_goals.document.active_goals.len();
            let running = state
                .autopilot_goals
                .document
                .active_goals
                .iter()
                .filter(|goal| {
                    matches!(
                        goal.lifecycle_status,
                        crate::state::autopilot_goals::GoalLifecycleStatus::Running
                    )
                })
                .count();
            let queued = state
                .autopilot_goals
                .document
                .active_goals
                .iter()
                .filter(|goal| {
                    matches!(
                        goal.lifecycle_status,
                        crate::state::autopilot_goals::GoalLifecycleStatus::Queued
                    )
                })
                .count();
            ToolBridgeResultEnvelope::ok(
                "OA-GOAL-SCHEDULER-STATUS-OK",
                "Goal scheduler summary read",
                json!({
                    "active_goals": active,
                    "running_goals": running,
                    "queued_goals": queued,
                    "last_action": state.autopilot_goals.last_action,
                    "last_error": state.autopilot_goals.last_error,
                    "rollout": {
                        "config": state.autopilot_goals.document.rollout_config.clone(),
                        "gate": state.autopilot_goals.rollout_gate_decision(),
                        "metrics": state.autopilot_goals.rollout_metrics_snapshot(),
                        "health": state.autopilot_goals.rollout_health_report(),
                    },
                }),
            )
        }
        "recover_startup" => match state
            .autopilot_goals
            .recover_after_restart(now_epoch_seconds)
        {
            Ok(report) => {
                state.goal_restart_recovery_ran = true;
                let _ = state
                    .autopilot_goals
                    .reconcile_os_scheduler_adapters(now_epoch_seconds);
                ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-RECOVER-OK",
                    "Executed startup goal recovery sequence",
                    json!({
                        "recovered_running_goals": report.recovered_running_goals,
                        "replay_queued_goals": report.replay_queued_goals,
                        "skipped_goals": report.skipped_goals,
                        "catchup_backlog": report.catchup_backlog,
                    }),
                )
            }
            Err(error) => ToolBridgeResultEnvelope::error(
                "OA-GOAL-SCHEDULER-RECOVER-FAILED",
                format!("Goal restart recovery failed: {}", error),
                json!({ "error": error }),
            ),
        },
        "run_now" => {
            let Some(goal_id) = args
                .goal_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-MISSING-GOAL",
                    "run_now requires a non-empty goal_id",
                    json!({ "action": args.action }),
                );
            };
            match state
                .autopilot_goals
                .schedule_goal_run_now(goal_id, now_epoch_seconds)
            {
                Ok(()) => ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-RUN-NOW-OK",
                    "Scheduled immediate goal run",
                    json!({ "goal_id": goal_id, "scheduled_at_epoch_seconds": now_epoch_seconds }),
                ),
                Err(error) => ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-RUN-NOW-FAILED",
                    format!("Failed to schedule immediate run: {}", error),
                    json!({ "goal_id": goal_id, "error": error }),
                ),
            }
        }
        "set_missed_policy" => {
            let Some(goal_id) = args
                .goal_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-MISSING-GOAL",
                    "set_missed_policy requires a non-empty goal_id",
                    json!({ "action": args.action }),
                );
            };
            let Some(policy_raw) = args.missed_run_policy.as_deref() else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-MISSING-POLICY",
                    "set_missed_policy requires missed_run_policy",
                    json!({ "goal_id": goal_id }),
                );
            };
            let Some(policy) = parse_goal_missed_run_policy(policy_raw) else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-INVALID-POLICY",
                    "missed_run_policy must be catch_up, skip, or single_replay",
                    json!({ "goal_id": goal_id, "missed_run_policy": policy_raw }),
                );
            };
            match state.autopilot_goals.set_goal_missed_run_policy(
                goal_id,
                policy,
                now_epoch_seconds,
            ) {
                Ok(()) => ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-POLICY-OK",
                    "Updated missed-run policy",
                    json!({
                        "goal_id": goal_id,
                        "missed_run_policy": policy_raw,
                    }),
                ),
                Err(error) => ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-POLICY-FAILED",
                    format!("Failed to set missed-run policy: {}", error),
                    json!({ "goal_id": goal_id, "error": error }),
                ),
            }
        }
        "set_kill_switch" => {
            let Some(goal_id) = args
                .goal_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-MISSING-GOAL",
                    "set_kill_switch requires a non-empty goal_id",
                    json!({ "action": args.action }),
                );
            };
            let Some(active) = args.kill_switch_active else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-MISSING-KILL-SWITCH",
                    "set_kill_switch requires kill_switch_active=true|false",
                    json!({ "goal_id": goal_id }),
                );
            };
            match state.autopilot_goals.set_goal_kill_switch(
                goal_id,
                active,
                args.kill_switch_reason.as_deref(),
                now_epoch_seconds,
            ) {
                Ok(()) => ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-KILL-SWITCH-OK",
                    if active {
                        "Enabled goal kill switch"
                    } else {
                        "Disabled goal kill switch"
                    },
                    json!({
                        "goal_id": goal_id,
                        "kill_switch_active": active,
                        "kill_switch_reason": args.kill_switch_reason.clone(),
                    }),
                ),
                Err(error) => ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-KILL-SWITCH-FAILED",
                    format!("Failed updating goal kill switch: {}", error),
                    json!({ "goal_id": goal_id, "error": error }),
                ),
            }
        }
        "set_rollout" => {
            let stage = args
                .rollout_stage
                .as_deref()
                .map(parse_goal_rollout_stage)
                .transpose();
            let stage = match stage {
                Ok(value) => value,
                Err(error) => {
                    return ToolBridgeResultEnvelope::error(
                        "OA-GOAL-SCHEDULER-INVALID-ROLLOUT-STAGE",
                        error,
                        json!({
                            "rollout_stage": args.rollout_stage,
                        }),
                    );
                }
            };

            let rollback_policy = if args.max_false_success_rate_bps.is_some()
                || args.max_abort_rate_bps.is_some()
                || args.max_error_rate_bps.is_some()
                || args.max_avg_payout_confirm_latency_seconds.is_some()
            {
                let existing = state
                    .autopilot_goals
                    .document
                    .rollout_config
                    .rollback_policy
                    .clone();
                Some(GoalRolloutRollbackPolicy {
                    max_false_success_rate_bps: args
                        .max_false_success_rate_bps
                        .unwrap_or(existing.max_false_success_rate_bps),
                    max_abort_rate_bps: args
                        .max_abort_rate_bps
                        .unwrap_or(existing.max_abort_rate_bps),
                    max_error_rate_bps: args
                        .max_error_rate_bps
                        .unwrap_or(existing.max_error_rate_bps),
                    max_avg_payout_confirm_latency_seconds: args
                        .max_avg_payout_confirm_latency_seconds
                        .unwrap_or(existing.max_avg_payout_confirm_latency_seconds),
                })
            } else {
                None
            };

            let hardening_checklist =
                if args.hardening_authoritative_payout_gate_validated.is_some()
                    || args.hardening_scheduler_recovery_drills_validated.is_some()
                    || args.hardening_swap_risk_alerting_validated.is_some()
                    || args.hardening_incident_runbook_validated.is_some()
                    || args.hardening_test_matrix_gate_green.is_some()
                {
                    let existing = state
                        .autopilot_goals
                        .document
                        .rollout_config
                        .hardening_checklist
                        .clone();
                    Some(GoalRolloutHardeningChecklist {
                        authoritative_payout_gate_validated: args
                            .hardening_authoritative_payout_gate_validated
                            .unwrap_or(existing.authoritative_payout_gate_validated),
                        scheduler_recovery_drills_validated: args
                            .hardening_scheduler_recovery_drills_validated
                            .unwrap_or(existing.scheduler_recovery_drills_validated),
                        swap_risk_alerting_validated: args
                            .hardening_swap_risk_alerting_validated
                            .unwrap_or(existing.swap_risk_alerting_validated),
                        incident_runbook_validated: args
                            .hardening_incident_runbook_validated
                            .unwrap_or(existing.incident_runbook_validated),
                        test_matrix_gate_green: args
                            .hardening_test_matrix_gate_green
                            .unwrap_or(existing.test_matrix_gate_green),
                    })
                } else {
                    None
                };

            match state.autopilot_goals.update_rollout_config(
                args.rollout_enabled,
                stage,
                args.rollout_cohorts.clone(),
                rollback_policy,
                hardening_checklist,
                now_epoch_seconds,
            ) {
                Ok(()) => ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-ROLLOUT-OK",
                    "Updated goal rollout configuration",
                    json!({
                        "rollout_config": state.autopilot_goals.document.rollout_config.clone(),
                        "rollout_gate": state.autopilot_goals.rollout_gate_decision(),
                        "rollout_metrics": state.autopilot_goals.rollout_metrics_snapshot(),
                        "rollout_health": state.autopilot_goals.rollout_health_report(),
                    }),
                ),
                Err(error) => ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-ROLLOUT-FAILED",
                    format!("Failed updating rollout configuration: {}", error),
                    json!({ "error": error }),
                ),
            }
        }
        "toggle_os_adapter" => {
            let Some(goal_id) = args
                .goal_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-MISSING-GOAL",
                    "toggle_os_adapter requires a non-empty goal_id",
                    json!({ "action": args.action }),
                );
            };
            let goal = state
                .autopilot_goals
                .document
                .active_goals
                .iter()
                .find(|goal| goal.goal_id == goal_id);
            let Some(goal) = goal else {
                return ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-GOAL-NOT-FOUND",
                    format!("Goal '{}' not found", goal_id),
                    json!({ "goal_id": goal_id }),
                );
            };
            let enable = !goal.schedule.os_adapter.enabled;
            let adapter = if enable {
                goal.schedule
                    .os_adapter
                    .adapter
                    .or_else(preferred_adapter_for_host)
                    .or(Some(OsSchedulerAdapterKind::Cron))
            } else {
                goal.schedule
                    .os_adapter
                    .adapter
                    .or(Some(OsSchedulerAdapterKind::Cron))
            };
            match state.autopilot_goals.set_goal_os_scheduler_adapter(
                goal_id,
                enable,
                adapter,
                now_epoch_seconds,
            ) {
                Ok(()) => ToolBridgeResultEnvelope::ok(
                    "OA-GOAL-SCHEDULER-OS-ADAPTER-OK",
                    if enable {
                        "Enabled OS scheduler adapter"
                    } else {
                        "Disabled OS scheduler adapter"
                    },
                    json!({
                        "goal_id": goal_id,
                        "enabled": enable,
                        "adapter": adapter.map(|kind| kind.as_str().to_string()),
                    }),
                ),
                Err(error) => ToolBridgeResultEnvelope::error(
                    "OA-GOAL-SCHEDULER-OS-ADAPTER-FAILED",
                    format!("Failed to toggle OS scheduler adapter: {}", error),
                    json!({ "goal_id": goal_id, "error": error }),
                ),
            }
        }
        "reconcile_os_adapters" => match state
            .autopilot_goals
            .reconcile_os_scheduler_adapters(now_epoch_seconds)
        {
            Ok(reconciled_goal_ids) => ToolBridgeResultEnvelope::ok(
                "OA-GOAL-SCHEDULER-RECONCILE-OK",
                "Reconciled enabled OS scheduler adapters",
                json!({
                    "reconciled_goal_ids": reconciled_goal_ids,
                }),
            ),
            Err(error) => ToolBridgeResultEnvelope::error(
                "OA-GOAL-SCHEDULER-RECONCILE-FAILED",
                format!("OS scheduler reconcile failed: {}", error),
                json!({ "error": error }),
            ),
        },
        _ => ToolBridgeResultEnvelope::error(
            "OA-GOAL-SCHEDULER-ACTION-UNSUPPORTED",
            format!(
                "Unsupported goal scheduler action '{}'. Allowed: status, recover_startup, run_now, set_missed_policy, set_kill_switch, set_rollout, toggle_os_adapter, reconcile_os_adapters",
                args.action
            ),
            json!({ "action": args.action }),
        ),
    }
}

fn execute_wallet_check_tool(
    state: &RenderState,
    args: &WalletCheckArgs,
) -> ToolBridgeResultEnvelope {
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let window_seconds = args.window_seconds.unwrap_or(86_400).clamp(60, 2_592_000);
    let cutoff = now_epoch_seconds.saturating_sub(window_seconds);

    let payments = state
        .spark_wallet
        .recent_payments
        .iter()
        .filter(|payment| {
            payment.timestamp >= cutoff && payment.status.eq_ignore_ascii_case("succeeded")
        })
        .collect::<Vec<_>>();
    let sent_sats = payments
        .iter()
        .filter(|payment| payment.direction.eq_ignore_ascii_case("send"))
        .fold(0u64, |acc, payment| acc.saturating_add(payment.amount_sats));
    let sent_fee_sats = payments
        .iter()
        .filter(|payment| payment.direction.eq_ignore_ascii_case("send"))
        .fold(0u64, |acc, payment| acc.saturating_add(payment.fees_sats));
    let sent_total_debit_sats = payments
        .iter()
        .filter(|payment| payment.direction.eq_ignore_ascii_case("send"))
        .fold(0u64, |acc, payment| {
            acc.saturating_add(crate::spark_wallet::wallet_payment_total_debit_sats(
                payment,
            ))
        });
    let received_sats = payments
        .iter()
        .filter(|payment| payment.direction.eq_ignore_ascii_case("receive"))
        .fold(0u64, |acc, payment| acc.saturating_add(payment.amount_sats));

    let payment_rows = if args.include_payments {
        payments
            .iter()
            .take(50)
            .map(|payment| {
                json!({
                    "id": payment.id,
                    "direction": payment.direction,
                    "status": payment.status,
                    "amount_sats": payment.amount_sats,
                    "fees_sats": payment.fees_sats,
                    "total_debit_sats": crate::spark_wallet::wallet_payment_total_debit_sats(payment),
                    "timestamp": payment.timestamp,
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    ToolBridgeResultEnvelope::ok(
        "OA-WALLET-CHECK-OK",
        "Read wallet status and bounded payment summary",
        json!({
            "network": format!("{:?}", state.spark_wallet.network),
            "network_status_present": state.spark_wallet.network_status.is_some(),
            "balance_sats": state
                .spark_wallet
                .balance
                .as_ref()
                .map(spark_total_balance_sats),
            "window_seconds": window_seconds,
            "payment_summary": {
                "count": payments.len(),
                "sent_sats": sent_sats,
                "sent_fee_sats": sent_fee_sats,
                "sent_total_debit_sats": sent_total_debit_sats,
                "received_sats": received_sats,
            },
            "payments": payment_rows,
            "last_error": state.spark_wallet.last_error,
            "last_action": state.spark_wallet.last_action,
        }),
    )
}

fn execute_provider_control_tool(
    state: &mut RenderState,
    args: &ProviderControlArgs,
) -> ToolBridgeResultEnvelope {
    let action = normalize_key(&args.action);
    match action.as_str() {
        "status" => ToolBridgeResultEnvelope::ok(
            "OA-PROVIDER-CONTROL-STATUS-OK",
            "Read provider runtime status",
            json!({
                "mode": state.provider_runtime.mode.label(),
                "degraded_reason_code": state.provider_runtime.degraded_reason_code,
                "queue_depth": state.provider_runtime.queue_depth,
                "runner_online": matches!(state.provider_runtime.mode, crate::app_state::ProviderMode::Online),
                "heartbeat_seconds": state.sa_lane.heartbeat_seconds,
                "last_result": state.provider_runtime.last_result,
                "last_error_detail": state.provider_runtime.last_error_detail,
            }),
        ),
        "set_online" | "set_offline" => {
            let online = action == "set_online";
            if online {
                let _ = state.spark_worker.enqueue(SparkWalletCommand::Reload);
            }
            match state.queue_sa_command(SaLifecycleCommand::SetRunnerOnline { online }) {
                Ok(command_seq) => ToolBridgeResultEnvelope::ok(
                    "OA-PROVIDER-CONTROL-SET-ONLINE-QUEUED",
                    if online {
                        "Queued provider online transition"
                    } else {
                        "Queued provider offline transition"
                    },
                    json!({
                        "online": online,
                        "command_seq": command_seq,
                    }),
                ),
                Err(error) => ToolBridgeResultEnvelope::error(
                    "OA-PROVIDER-CONTROL-SET-ONLINE-FAILED",
                    format!("Failed queuing provider state transition: {}", error),
                    json!({
                        "online": online,
                        "error": error,
                    }),
                ),
            }
        }
        "refresh_wallet" => match state.spark_worker.enqueue(SparkWalletCommand::Reload) {
            Ok(()) => ToolBridgeResultEnvelope::ok(
                "OA-PROVIDER-CONTROL-WALLET-REFRESH-QUEUED",
                "Queued wallet refresh",
                json!({ "queued": true }),
            ),
            Err(error) => ToolBridgeResultEnvelope::error(
                "OA-PROVIDER-CONTROL-WALLET-REFRESH-FAILED",
                format!("Failed queuing wallet refresh: {}", error),
                json!({ "queued": false, "error": error }),
            ),
        },
        _ => ToolBridgeResultEnvelope::error(
            "OA-PROVIDER-CONTROL-ACTION-UNSUPPORTED",
            format!(
                "Unsupported provider control action '{}'. Allowed: status, set_online, set_offline, refresh_wallet",
                args.action
            ),
            json!({ "action": args.action }),
        ),
    }
}

fn parse_treasury_transfer_asset(
    raw: &str,
) -> Result<crate::stablesats_blink_worker::StableSatsBlinkTransferAsset, ToolBridgeResultEnvelope>
{
    match normalize_key(raw).as_str() {
        "btc_sats" | "btc" | "sats" | "sat" => {
            Ok(crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::BtcSats)
        }
        "usd_cents" | "usd" | "cents" | "cent" => {
            Ok(crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::UsdCents)
        }
        _ => Err(ToolBridgeResultEnvelope::error(
            "OA-TREASURY-TRANSFER-INVALID-ASSET",
            format!("Unsupported treasury transfer asset '{}'", raw),
            json!({
                "asset": raw,
                "supported": ["btc_sats", "usd_cents"],
            }),
        )),
    }
}

fn resolve_stablesats_wallet_by_owner(
    state: &RenderState,
    owner_id: &str,
    error_code: &str,
) -> Result<crate::app_state::StableSatsAgentWalletState, ToolBridgeResultEnvelope> {
    state
        .stable_sats_simulation
        .agents
        .iter()
        .find(|wallet| wallet.owner_id == owner_id)
        .cloned()
        .ok_or_else(|| {
            ToolBridgeResultEnvelope::error(
                error_code,
                format!("StableSats wallet owner '{}' not configured", owner_id),
                json!({
                    "owner_id": owner_id,
                    "known_owner_ids": state
                        .stable_sats_simulation
                        .agents
                        .iter()
                        .map(|wallet| wallet.owner_id.clone())
                        .collect::<Vec<_>>(),
                }),
            )
        })
}

fn ensure_wallet_credential_slot_enabled(
    entries: &[crate::credentials::CredentialRecord],
    credential_name: &str,
) -> Result<(), String> {
    let normalized = crate::credentials::normalize_env_var_name(credential_name);
    let Some(entry) = entries.iter().find(|entry| entry.name == normalized) else {
        return Err(format!(
            "Credential slot {} is missing from credential manager",
            normalized
        ));
    };
    if !entry.enabled {
        return Err(format!("Credential slot {} is disabled", normalized));
    }
    Ok(())
}

fn has_enabled_credential_slot(
    entries: &[crate::credentials::CredentialRecord],
    credential_name: &str,
) -> bool {
    let normalized = crate::credentials::normalize_env_var_name(credential_name);
    entries
        .iter()
        .any(|entry| entry.name == normalized && entry.enabled)
}

fn resolve_wallet_blink_env_overrides(
    state: &RenderState,
    wallet: &crate::app_state::StableSatsAgentWalletState,
) -> Result<Vec<(String, String)>, String> {
    let entries = state.credentials.entries.as_slice();
    ensure_wallet_credential_slot_enabled(entries, wallet.credential_key_name.as_str())?;

    let api_key = state
        .credentials
        .read_secure_value(wallet.credential_key_name.as_str())
        .map_err(|error| {
            format!(
                "{} secure credential read failed for {}: {error}",
                wallet.agent_name, wallet.credential_key_name
            )
        })?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "{} missing secure value for {}",
                wallet.agent_name, wallet.credential_key_name
            )
        })?;
    let mut env_overrides = vec![("BLINK_API_KEY".to_string(), api_key)];

    if let Some(url_name) = wallet.credential_url_name.as_deref()
        && has_enabled_credential_slot(entries, url_name)
        && let Some(url) = state
            .credentials
            .read_secure_value(url_name)
            .map_err(|error| {
                format!(
                    "{} secure credential read failed for {}: {error}",
                    wallet.agent_name, url_name
                )
            })?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    {
        env_overrides.push(("BLINK_API_URL".to_string(), url));
    }

    Ok(env_overrides)
}

fn parse_goal_missed_run_policy(raw: &str) -> Option<GoalMissedRunPolicy> {
    match normalize_key(raw).as_str() {
        "catch_up" | "catchup" => Some(GoalMissedRunPolicy::CatchUp),
        "skip" => Some(GoalMissedRunPolicy::Skip),
        "single_replay" | "single" => Some(GoalMissedRunPolicy::SingleReplay),
        _ => None,
    }
}

fn parse_goal_rollout_stage(raw: &str) -> Result<GoalRolloutStage, String> {
    match normalize_key(raw).as_str() {
        "disabled" => Ok(GoalRolloutStage::Disabled),
        "internal_dogfood" | "internal" | "dogfood" => Ok(GoalRolloutStage::InternalDogfood),
        "canary" => Ok(GoalRolloutStage::Canary),
        "general_availability" | "ga" | "general" => Ok(GoalRolloutStage::GeneralAvailability),
        _ => Err(
            "rollout_stage must be disabled, internal_dogfood, canary, or general_availability"
                .to_string(),
        ),
    }
}

fn parse_swap_direction(direction: &str) -> Result<SwapDirection, ToolBridgeResultEnvelope> {
    match normalize_key(direction).as_str() {
        "btc_to_usd" | "sell_btc" | "buy_usd" => Ok(SwapDirection::BtcToUsd),
        "usd_to_btc" | "sell_usd" | "buy_btc" => Ok(SwapDirection::UsdToBtc),
        _ => Err(ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-INVALID-DIRECTION",
            format!("Unsupported swap direction '{}'", direction),
            json!({
                "direction": direction,
                "supported": ["btc_to_usd", "usd_to_btc"],
            }),
        )),
    }
}

fn parse_swap_unit(unit: &str) -> Result<SwapAmountUnit, ToolBridgeResultEnvelope> {
    match normalize_key(unit).as_str() {
        "sats" | "sat" => Ok(SwapAmountUnit::Sats),
        "cents" | "cent" | "usd_cents" => Ok(SwapAmountUnit::Cents),
        _ => Err(ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-INVALID-UNIT",
            format!("Unsupported swap unit '{}'", unit),
            json!({
                "unit": unit,
                "supported": ["sats", "cents"],
            }),
        )),
    }
}

fn validate_direction_unit(
    direction: SwapDirection,
    unit: SwapAmountUnit,
) -> Option<ToolBridgeResultEnvelope> {
    match (direction, unit) {
        (SwapDirection::BtcToUsd, SwapAmountUnit::Sats)
        | (SwapDirection::UsdToBtc, SwapAmountUnit::Cents) => None,
        _ => Some(ToolBridgeResultEnvelope::error(
            "OA-SWAP-QUOTE-UNIT-MISMATCH",
            "For controlled tool path, BTC->USD requires sats and USD->BTC requires cents",
            json!({
                "direction": format!("{:?}", direction),
                "unit": match unit {
                    SwapAmountUnit::Sats => "sats",
                    SwapAmountUnit::Cents => "cents",
                },
            }),
        )),
    }
}

fn swap_direction_script_value(direction: SwapDirection) -> &'static str {
    match direction {
        SwapDirection::BtcToUsd => "btc-to-usd",
        SwapDirection::UsdToBtc => "usd-to-btc",
    }
}

fn swap_unit_script_value(unit: SwapAmountUnit) -> &'static str {
    match unit {
        SwapAmountUnit::Sats => "sats",
        SwapAmountUnit::Cents => "cents",
    }
}

fn resolve_blink_swap_script_path(
    state: &RenderState,
    script_name: &str,
) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().ok();
    let candidates = blink_swap_script_candidates(
        &state.skill_registry.discovered_skills,
        state.skill_registry.repo_skills_root.as_deref(),
        cwd.as_deref(),
        script_name,
    );
    select_existing_blink_swap_script_path(candidates, script_name)
}

fn resolve_swap_runtime_env_overrides(state: &RenderState) -> Vec<(String, String)> {
    if let Some(operator_wallet) =
        state.stable_sats_simulation.agents.iter().find(|wallet| {
            wallet.owner_kind == crate::app_state::StableSatsWalletOwnerKind::Operator
        })
    {
        let mut overrides = Vec::<(String, String)>::new();
        if let Ok(Some(api_key)) = state
            .credentials
            .read_secure_value(operator_wallet.credential_key_name.as_str())
        {
            let trimmed = api_key.trim();
            if !trimmed.is_empty() {
                overrides.push(("BLINK_API_KEY".to_string(), trimmed.to_string()));
            }
        }
        if let Some(url_name) = operator_wallet.credential_url_name.as_deref()
            && let Ok(Some(api_url)) = state.credentials.read_secure_value(url_name)
        {
            let trimmed = api_url.trim();
            if !trimmed.is_empty() {
                overrides.push(("BLINK_API_URL".to_string(), trimmed.to_string()));
            }
        }
        if !overrides.is_empty() {
            return overrides;
        }
    }

    let scope = crate::credentials::CREDENTIAL_SCOPE_SKILLS
        | crate::credentials::CREDENTIAL_SCOPE_CODEX
        | crate::credentials::CREDENTIAL_SCOPE_GLOBAL;
    state
        .credentials
        .resolve_env_for_scope(scope)
        .unwrap_or_default()
        .into_iter()
        .filter(|(name, value)| {
            !value.trim().is_empty()
                && (name.eq_ignore_ascii_case("BLINK_API_KEY")
                    || name.eq_ignore_ascii_case("BLINK_API_URL"))
        })
        .collect()
}

fn blink_swap_script_candidates(
    discovered_skills: &[SkillRegistryDiscoveredSkill],
    repo_skills_root: Option<&str>,
    cwd: Option<&Path>,
    script_name: &str,
) -> BTreeSet<PathBuf> {
    let mut candidates = BTreeSet::<PathBuf>::new();
    for skill in discovered_skills
        .iter()
        .filter(|skill| skill.enabled && skill.name.eq_ignore_ascii_case(BLINK_SKILL_NAME))
    {
        let skill_root = normalize_skill_root_path(skill.path.as_str());
        if let Some(skill_root) = skill_root {
            candidates.insert(skill_root.join("scripts").join(script_name));
        }
    }
    if let Some(repo_skills_root) = repo_skills_root {
        candidates.insert(
            PathBuf::from(repo_skills_root)
                .join(BLINK_SKILL_NAME)
                .join("scripts")
                .join(script_name),
        );
    }
    if let Some(cwd) = cwd {
        candidates.insert(
            cwd.join("skills")
                .join(BLINK_SKILL_NAME)
                .join("scripts")
                .join(script_name),
        );
    }
    candidates
}

fn normalize_skill_root_path(raw_skill_path: &str) -> Option<PathBuf> {
    let skill_path = PathBuf::from(raw_skill_path.trim());
    if skill_path
        .file_name()
        .map(|file_name| file_name.to_string_lossy().eq_ignore_ascii_case("SKILL.md"))
        .unwrap_or(false)
    {
        return skill_path.parent().map(Path::to_path_buf);
    }
    if skill_path.is_dir() {
        return Some(skill_path);
    }
    skill_path.parent().map(Path::to_path_buf)
}

fn select_existing_blink_swap_script_path(
    candidates: BTreeSet<PathBuf>,
    script_name: &str,
) -> Result<PathBuf, String> {
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            format!(
                "Unable to locate Blink swap script '{}'. Ensure skills/blink is discovered and available.",
                script_name
            )
        })
}

fn run_blink_swap_script_json(
    script_path: &Path,
    args: &[String],
) -> Result<serde_json::Value, String> {
    let mut command = Command::new("node");
    command.arg(script_path);
    command.args(args);
    let output = command.output().map_err(|error| {
        format!(
            "Failed launching node for Blink script {}: {error}",
            script_path.display()
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let status = output.status.code().map_or_else(
            || "signal".to_string(),
            |value| format!("exit_code={value}"),
        );
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output".to_string()
        };
        return Err(format!(
            "Blink script {} failed ({status}): {details}",
            script_path.display()
        ));
    }
    if stdout.is_empty() {
        return Err(format!(
            "Blink script {} returned empty stdout",
            script_path.display()
        ));
    }
    serde_json::from_str::<serde_json::Value>(&stdout).map_err(|error| {
        format!(
            "Blink script {} returned non-JSON stdout: {error}",
            script_path.display()
        )
    })
}

fn parse_blink_swap_direction(raw: &str) -> Result<SwapDirection, String> {
    match normalize_key(raw).as_str() {
        "btc_to_usd" => Ok(SwapDirection::BtcToUsd),
        "usd_to_btc" => Ok(SwapDirection::UsdToBtc),
        _ => Err(format!("Unsupported Blink swap direction '{raw}'")),
    }
}

fn parse_blink_swap_unit(raw: &str) -> Result<SwapAmountUnit, String> {
    match normalize_key(raw).as_str() {
        "sats" | "sat" => Ok(SwapAmountUnit::Sats),
        "cents" | "cent" => Ok(SwapAmountUnit::Cents),
        _ => Err(format!("Unsupported Blink swap unit '{raw}'")),
    }
}

fn map_blink_quote_terms(quote: BlinkSwapQuoteTerms) -> Result<SwapQuoteTerms, String> {
    let direction = parse_blink_swap_direction(&quote.direction)?;
    let amount_in_unit = parse_blink_swap_unit(&quote.amount_in.unit)?;
    let amount_out_unit = parse_blink_swap_unit(&quote.amount_out.unit)?;
    Ok(SwapQuoteTerms {
        quote_id: quote.quote_id,
        direction,
        amount_in: SwapAmount {
            amount: quote.amount_in.value,
            unit: amount_in_unit,
        },
        amount_out: SwapAmount {
            amount: quote.amount_out.value,
            unit: amount_out_unit,
        },
        expires_at_epoch_seconds: quote.expires_at_epoch_seconds,
        immediate_execution: quote.immediate_execution,
        fee_sats: quote.fee_sats,
        fee_bps: quote.fee_bps,
        slippage_bps: quote.slippage_bps,
    })
}

fn parse_blink_quote_terms_from_json(raw: serde_json::Value) -> Result<SwapQuoteTerms, String> {
    let envelope = serde_json::from_value::<BlinkSwapQuoteEnvelope>(raw)
        .map_err(|error| format!("Invalid Blink quote payload: {error}"))?;
    if !envelope.event.eq_ignore_ascii_case("swap_quote") {
        return Err(format!("Unexpected Blink quote event '{}'", envelope.event));
    }
    map_blink_quote_terms(envelope.quote)
}

fn parse_blink_execution_payload_from_json(
    raw: serde_json::Value,
) -> Result<BlinkSwapExecutionEnvelope, String> {
    let envelope = serde_json::from_value::<BlinkSwapExecutionEnvelope>(raw)
        .map_err(|error| format!("Invalid Blink execution payload: {error}"))?;
    if !envelope.event.eq_ignore_ascii_case("swap_execution") {
        return Err(format!(
            "Unexpected Blink execution event '{}'",
            envelope.event
        ));
    }
    Ok(envelope)
}

fn map_blink_execution_status(status: &str) -> Result<SwapExecutionStatus, String> {
    match status.trim().to_ascii_uppercase().as_str() {
        "SUCCESS" => Ok(SwapExecutionStatus::Success),
        "FAILURE" => Ok(SwapExecutionStatus::Failure),
        "PENDING" => Ok(SwapExecutionStatus::Pending),
        "ALREADY_PAID" => Ok(SwapExecutionStatus::AlreadyPaid),
        value => Err(format!("Unsupported Blink execution status '{value}'")),
    }
}

fn cad_action_from_key(
    action: &str,
    index: Option<usize>,
) -> Result<CadDemoPaneAction, ToolBridgeResultEnvelope> {
    let require_index = |action_label: &str| -> Result<usize, ToolBridgeResultEnvelope> {
        index.ok_or_else(|| {
            ToolBridgeResultEnvelope::error(
                "OA-CAD-ACTION-MISSING-INDEX",
                format!("CAD action '{}' requires index", action_label),
                json!({
                    "action": action_label,
                }),
            )
        })
    };

    let value = match action {
        "bootstrap" | "bootstrap_demo" => CadDemoPaneAction::BootstrapDemo,
        "reset_session" => CadDemoPaneAction::ResetSession,
        "cycle_variant" => CadDemoPaneAction::CycleVariant,
        "toggle_gripper_jaw" | "animate_jaw" => CadDemoPaneAction::ToggleGripperJawAnimation,
        "toggle_viewport_layout" | "toggle_layout" => CadDemoPaneAction::ToggleViewportLayout,
        "reset_camera" => CadDemoPaneAction::ResetCamera,
        "toggle_drawing_mode" => CadDemoPaneAction::ToggleDrawingViewMode,
        "cycle_drawing_direction" => CadDemoPaneAction::CycleDrawingViewDirection,
        "toggle_drawing_hidden_lines" => CadDemoPaneAction::ToggleDrawingHiddenLines,
        "toggle_drawing_dimensions" => CadDemoPaneAction::ToggleDrawingDimensions,
        "reset_drawing_view" => CadDemoPaneAction::ResetDrawingView,
        "add_drawing_detail" => CadDemoPaneAction::AddDrawingDetailView,
        "clear_drawing_details" => CadDemoPaneAction::ClearDrawingDetailViews,
        "toggle_projection" => CadDemoPaneAction::ToggleProjectionMode,
        "cycle_section_plane" => CadDemoPaneAction::CycleSectionPlane,
        "step_section_offset" => CadDemoPaneAction::StepSectionPlaneOffset,
        "cycle_material" => CadDemoPaneAction::CycleMaterialPreset,
        "toggle_snap_grid" => CadDemoPaneAction::ToggleSnapGrid,
        "toggle_snap_origin" => CadDemoPaneAction::ToggleSnapOrigin,
        "toggle_snap_endpoint" => CadDemoPaneAction::ToggleSnapEndpoint,
        "toggle_snap_midpoint" => CadDemoPaneAction::ToggleSnapMidpoint,
        "snap_top" => CadDemoPaneAction::SnapViewTop,
        "snap_front" => CadDemoPaneAction::SnapViewFront,
        "snap_right" => CadDemoPaneAction::SnapViewRight,
        "snap_isometric" => CadDemoPaneAction::SnapViewIsometric,
        "cycle_hidden_line_mode" => CadDemoPaneAction::CycleHiddenLineMode,
        "cycle_sensor_mode" | "cycle_sensor_visualization_mode" => {
            CadDemoPaneAction::CycleSensorVisualizationMode
        }
        "cycle_warning_severity_filter" => CadDemoPaneAction::CycleWarningSeverityFilter,
        "cycle_warning_code_filter" => CadDemoPaneAction::CycleWarningCodeFilter,
        "select_warning" => CadDemoPaneAction::SelectWarning(require_index(action)?),
        "select_warning_marker" => CadDemoPaneAction::SelectWarningMarker(require_index(action)?),
        "select_timeline_row" => CadDemoPaneAction::SelectTimelineRow(require_index(action)?),
        "timeline_select_prev" => CadDemoPaneAction::TimelineSelectPrev,
        "timeline_select_next" => CadDemoPaneAction::TimelineSelectNext,
        "snapshot" | "status" => CadDemoPaneAction::Noop,
        _ => {
            return Err(ToolBridgeResultEnvelope::error(
                "OA-CAD-ACTION-UNSUPPORTED",
                format!("Unsupported CAD action '{}'", action),
                json!({
                    "action": action,
                    "index": index,
                }),
            ));
        }
    };
    Ok(value)
}

fn pane_resolution_error(code: &str, pane_ref: &str) -> ToolBridgeResultEnvelope {
    ToolBridgeResultEnvelope::error(
        code,
        format!("Could not resolve pane reference '{}'", pane_ref),
        json!({
            "pane": pane_ref,
            "supported_panes": enabled_pane_specs()
                .filter(|spec| spec.kind != PaneKind::Empty)
                .map(|spec| {
                    json!({
                        "kind": pane_kind_key(spec.kind),
                        "title": spec.title,
                        "command_id": spec.command.map(|command| command.id),
                    })
                })
                .collect::<Vec<_>>(),
        }),
    )
}

fn resolve_pane_kind(state: &RenderState, raw: &str) -> Option<PaneKind> {
    let _ = state;
    resolve_pane_kind_for_runtime(raw)
}

fn resolve_pane_kind_for_runtime(raw: &str) -> Option<PaneKind> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(spec) = pane_spec_by_command_id(trimmed) {
        return Some(spec.kind);
    }

    let normalized = normalize_key(trimmed);
    if normalized.is_empty() {
        return None;
    }

    enabled_pane_specs()
        .filter(|spec| spec.kind != PaneKind::Empty)
        .find_map(|spec| {
            let title_key = normalize_key(spec.title);
            let kind_key = normalize_key(pane_kind_key(spec.kind));
            let command_key = spec.command.map(|command| normalize_key(command.id));
            if normalized == title_key
                || normalized == kind_key
                || command_key
                    .as_ref()
                    .is_some_and(|value| *value == normalized)
                || pane_aliases(spec.kind)
                    .iter()
                    .any(|alias| *alias == normalized)
            {
                Some(spec.kind)
            } else {
                None
            }
        })
}

fn pane_aliases(kind: PaneKind) -> &'static [&'static str] {
    match kind {
        PaneKind::AutopilotChat => &["chat", "autopilot_chat", "autopilot", "codex"],
        PaneKind::ProjectOps => &["project_ops", "projectops", "pm", "project_management"],
        PaneKind::Calculator => &["calculator", "calc"],
        PaneKind::LocalInference => &[
            "local_inference",
            "local_runtime",
            "gpt_oss",
            "gptoss",
            "workbench",
        ],
        PaneKind::PsionicViz => &[
            "psionic_viz",
            "psionic_mesh",
            "psionic",
            "gpt_oss_viz",
            "decode_field",
        ],
        PaneKind::AppleFmWorkbench => &[
            "apple_fm",
            "apple_fm_workbench",
            "fm_workbench",
            "foundation_models",
        ],
        PaneKind::ProviderControl => &[
            "provider_control",
            "provider",
            "provider_runtime",
            "runtime_control",
            "mission_control",
            "go_online",
        ],
        PaneKind::SparkWallet => &["wallet", "spark_wallet"],
        PaneKind::SparkCreateInvoice => &["create_invoice", "invoice_create"],
        PaneKind::SparkPayInvoice => &["pay_invoice", "invoice_pay"],
        PaneKind::NostrIdentity => &["identity", "identity_keys", "nostr"],
        PaneKind::ReciprocalLoop => &["reciprocal_loop", "earn_loop", "pingpong_loop"],
        PaneKind::LogStream => &["log_stream", "logs", "runtime_logs"],
        PaneKind::BuyModePayments => &[
            "buy_mode",
            "buy_mode_payments",
            "buy_payments",
            "payment_history",
        ],
        PaneKind::BuyerRaceMatrix => &[
            "buyer_race_matrix",
            "race_matrix",
            "buyer_race",
            "nip90_race",
        ],
        PaneKind::CadDemo => &["cad", "cad_demo"],
        PaneKind::CastControl => &["cast", "cast_control"],
        _ => &[],
    }
}

fn pane_kind_key(kind: PaneKind) -> &'static str {
    match kind {
        PaneKind::Empty => "pane",
        PaneKind::AutopilotChat => "autopilot_chat",
        PaneKind::ProjectOps => "project_ops",
        PaneKind::CodexAccount => "codex_account",
        PaneKind::CodexModels => "codex_models",
        PaneKind::CodexConfig => "codex_config",
        PaneKind::CodexMcp => "codex_mcp",
        PaneKind::CodexApps => "codex_apps",
        PaneKind::CodexLabs => "codex_labs",
        PaneKind::CodexDiagnostics => "codex_diagnostics",
        PaneKind::GoOnline => "go_online",
        PaneKind::ProviderControl => "provider_control",
        PaneKind::ProviderStatus => "provider_status",
        PaneKind::LocalInference => "local_inference",
        PaneKind::PsionicViz => "psionic_viz",
        PaneKind::AppleFmWorkbench => "apple_fm_workbench",
        PaneKind::EarningsScoreboard => "earnings_scoreboard",
        PaneKind::RelayConnections => "relay_connections",
        PaneKind::SyncHealth => "sync_health",
        PaneKind::NetworkRequests => "network_requests",
        PaneKind::StarterJobs => "starter_jobs",
        PaneKind::ReciprocalLoop => "reciprocal_loop",
        PaneKind::ActivityFeed => "activity_feed",
        PaneKind::AlertsRecovery => "alerts_recovery",
        PaneKind::Settings => "settings",
        PaneKind::Credentials => "credentials",
        PaneKind::Calculator => "calculator",
        PaneKind::JobInbox => "job_inbox",
        PaneKind::ActiveJob => "active_job",
        PaneKind::JobHistory => "job_history",
        PaneKind::LogStream => "log_stream",
        PaneKind::BuyModePayments => "buy_mode",
        PaneKind::BuyerRaceMatrix => "buyer_race_matrix",
        PaneKind::NostrIdentity => "nostr_identity",
        PaneKind::SparkWallet => "spark_wallet",
        PaneKind::SparkCreateInvoice => "spark_create_invoice",
        PaneKind::SparkPayInvoice => "spark_pay_invoice",
        PaneKind::AgentProfileState => "agent_profile_state",
        PaneKind::AgentScheduleTick => "agent_schedule_tick",
        PaneKind::TrajectoryAudit => "trajectory_audit",
        PaneKind::CastControl => "cast_control",
        PaneKind::SkillRegistry => "skill_registry",
        PaneKind::SkillTrustRevocation => "skill_trust_revocation",
        PaneKind::CreditDesk => "credit_desk",
        PaneKind::CreditSettlementLedger => "credit_settlement_ledger",
        PaneKind::CadDemo => "cad_demo",
    }
}

fn normalize_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{
        CAD_CHECKPOINT_SCHEMA_VERSION, CAD_TOOL_RESPONSE_SCHEMA_VERSION,
        LEGACY_OPENAGENTS_TOOL_PANE_OPEN, OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH,
        OPENAGENTS_TOOL_LABOR_SCOPE, OPENAGENTS_TOOL_PANE_OPEN, OPENAGENTS_TOOL_SWAP_QUOTE,
        OPENAGENTS_TOOL_TREASURY_CONVERT, OPENAGENTS_TOOL_TREASURY_RECEIPT,
        OPENAGENTS_TOOL_TREASURY_TRANSFER, ToolBridgeResultEnvelope, cad_action_from_key,
        cad_checkpoint_payload, cad_parse_retry_prompt, decode_tool_call_request,
        enforce_labor_evidence_uri_scope, enforce_matching_labor_contract_scope, normalize_key,
        pane_action_to_hit_action, pane_kind_key, parse_blink_execution_payload_from_json,
        parse_blink_quote_terms_from_json, parse_bool_env_override, parse_goal_rollout_stage,
        parse_swap_direction, parse_swap_unit, parse_treasury_transfer_asset,
        resolve_pane_kind_for_runtime, run_blink_swap_script_json,
        select_existing_blink_swap_script_path, validate_direction_unit,
    };
    use crate::app_state::{
        AutopilotToolCallRequest, CadDemoPaneState, CadDemoWarningState, CadViewportLayout,
        PaneKind,
    };
    use crate::pane_system::{
        CadDemoPaneAction, PaneHitAction, ProviderControlPaneAction, RelayConnectionsPaneAction,
        SettingsPaneAction,
    };
    use crate::spark_pane::SparkPaneAction;
    use crate::state::autopilot_goals::GoalRolloutStage;
    use crate::state::swap_contract::{SwapAmountUnit, SwapDirection};
    use codex_client::AppServerRequestId;
    use serde::Deserialize;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn request(tool: &str, arguments: &str) -> AutopilotToolCallRequest {
        AutopilotToolCallRequest {
            request_id: AppServerRequestId::String("test-request-id".to_string()),
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
            call_id: "call".to_string(),
            tool: tool.to_string(),
            arguments: arguments.to_string(),
        }
    }

    #[derive(Debug, Deserialize)]
    struct PaneArgs {
        pane: String,
    }

    fn assert_error(result: Result<super::ToolBridgeRequest, ToolBridgeResultEnvelope>) -> String {
        let error = result.expect_err("expected decode failure");
        error.code
    }

    fn temp_js_path(test_name: &str) -> PathBuf {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch time available")
            .as_nanos();
        std::env::temp_dir().join(format!("openagents-tool-bridge-{test_name}-{now_nanos}.js"))
    }

    fn fixture_labor_binding() -> crate::labor_orchestrator::CodexLaborBinding {
        crate::labor_orchestrator::orchestrate_codex_turn(
            crate::labor_orchestrator::CodexTurnExecutionRequest {
                trigger: crate::labor_orchestrator::CodexRunTrigger::AutonomousGoal {
                    goal_id: "goal-earn".to_string(),
                    goal_title: "Earn bitcoin".to_string(),
                },
                submitted_at_epoch_ms: 1_000,
                thread_id: "thread".to_string(),
                input: vec![codex_client::UserInput::Text {
                    text: "earn bitcoin".to_string(),
                    text_elements: Vec::new(),
                }],
                cwd: Some(PathBuf::from("/repo")),
                approval_policy: Some(codex_client::AskForApproval::Never),
                sandbox_policy: Some(codex_client::SandboxPolicy::DangerFullAccess),
                model: Some("gpt-5.2-codex".to_string()),
                service_tier: None,
                effort: None,
                personality: None,
                collaboration_mode: None,
            },
        )
        .labor_binding
        .expect("economically meaningful turns should create labor bindings")
    }

    #[test]
    fn decode_accepts_supported_tool_and_object_arguments() {
        let decoded = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_PANE_OPEN,
            r#"{"pane":"Spark Wallet"}"#,
        ))
        .expect("decode should succeed");
        assert_eq!(decoded.tool, OPENAGENTS_TOOL_PANE_OPEN);

        let pane_args: PaneArgs = decoded.decode_arguments().expect("pane args decode");
        assert_eq!(pane_args.pane, "Spark Wallet");
    }

    #[test]
    fn decode_accepts_legacy_supported_tool_name() {
        let decoded = decode_tool_call_request(&request(
            LEGACY_OPENAGENTS_TOOL_PANE_OPEN,
            r#"{"pane":"Spark Wallet"}"#,
        ))
        .expect("legacy decode should succeed");
        assert_eq!(decoded.tool, LEGACY_OPENAGENTS_TOOL_PANE_OPEN);
    }

    #[test]
    fn decode_rejects_unsupported_tool_name() {
        let code = assert_error(decode_tool_call_request(&request(
            "openagents.not_real",
            "{}",
        )));
        assert_eq!(code, "OA-TOOL-UNSUPPORTED");
    }

    #[test]
    fn decode_rejects_malformed_json_arguments() {
        let code = assert_error(decode_tool_call_request(&request(
            OPENAGENTS_TOOL_PANE_OPEN,
            "{",
        )));
        assert_eq!(code, "OA-TOOL-ARGS-INVALID-JSON");
    }

    #[test]
    fn decode_rejects_non_object_arguments() {
        let code = assert_error(decode_tool_call_request(&request(
            OPENAGENTS_TOOL_PANE_OPEN,
            "[]",
        )));
        assert_eq!(code, "OA-TOOL-ARGS-NOT-OBJECT");
    }

    #[test]
    fn decode_arguments_reports_missing_required_field() {
        let decoded =
            decode_tool_call_request(&request(OPENAGENTS_TOOL_PANE_OPEN, r#"{"wrong":"field"}"#))
                .expect("decode should succeed");
        let error = decoded
            .decode_arguments::<PaneArgs>()
            .expect_err("missing required field should fail");
        assert_eq!(error.code, "OA-TOOL-ARGS-INVALID-SHAPE");
    }

    #[test]
    fn decode_accepts_labor_tools_and_contract_identifiers() {
        let binding = fixture_labor_binding();
        let decoded = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_LABOR_SCOPE,
            format!(
                "{{\"work_unit_id\":\"{}\",\"contract_id\":\"{}\"}}",
                binding.work_unit_id, binding.contract_id
            )
            .as_str(),
        ))
        .expect("labor scope tool should decode");
        assert_eq!(decoded.tool, OPENAGENTS_TOOL_LABOR_SCOPE);
    }

    #[test]
    fn labor_scope_enforcement_rejects_mismatched_contract_identifiers() {
        let binding = fixture_labor_binding();
        let decoded = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_LABOR_SCOPE,
            r#"{"work_unit_id":"work_unit.wrong","contract_id":"contract.wrong"}"#,
        ))
        .expect("labor scope decode should succeed");

        let error = enforce_matching_labor_contract_scope(&binding, &decoded)
            .expect("mismatched ids should be rejected");
        assert_eq!(error.code, "OA-LABOR-SCOPE-MISMATCH");
    }

    #[test]
    fn labor_evidence_attach_flow_enforces_uri_scope_and_accepts_in_scope_artifacts() {
        let mut binding = fixture_labor_binding();
        let scope_root = binding.artifact_scope_root();
        let in_scope_uri = format!("{scope_root}artifacts/tool-log");
        let out_of_scope_uri = "oa://autopilot/codex/other-work-unit/artifacts/tool-log";

        let in_scope = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH,
            format!(
                "{{\"work_unit_id\":\"{}\",\"contract_id\":\"{}\",\"kind\":\"tool_log\",\"uri\":\"{}\",\"digest\":\"sha256:tool-log\"}}",
                binding.work_unit_id, binding.contract_id, in_scope_uri
            )
            .as_str(),
        ))
        .expect("in-scope attach decode should succeed");
        assert!(enforce_matching_labor_contract_scope(&binding, &in_scope).is_none());
        assert!(enforce_labor_evidence_uri_scope(&binding, &in_scope).is_none());
        let args: super::LaborEvidenceAttachArgs = in_scope
            .decode_arguments()
            .expect("attach args should decode");
        binding
            .attach_evidence_ref(
                openagents_kernel_core::receipts::EvidenceRef::new(
                    args.kind,
                    args.uri,
                    args.digest,
                ),
                false,
            )
            .expect("in-scope evidence should attach");
        assert_eq!(binding.attached_evidence_refs.len(), 1);

        let out_of_scope = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH,
            format!(
                "{{\"work_unit_id\":\"{}\",\"contract_id\":\"{}\",\"kind\":\"tool_log\",\"uri\":\"{}\",\"digest\":\"sha256:tool-log\"}}",
                binding.work_unit_id, binding.contract_id, out_of_scope_uri
            )
            .as_str(),
        ))
        .expect("out-of-scope attach decode should succeed");
        let error = enforce_labor_evidence_uri_scope(&binding, &out_of_scope)
            .expect("out-of-scope URI should be rejected");
        assert_eq!(error.code, "OA-LABOR-EVIDENCE-OUT-OF-SCOPE");
    }

    #[test]
    fn resolve_pane_kind_accepts_command_id_title_and_alias() {
        assert_eq!(
            resolve_pane_kind_for_runtime("pane.wallet"),
            Some(PaneKind::SparkWallet)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("Spark Lightning Wallet"),
            Some(PaneKind::SparkWallet)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("wallet"),
            Some(PaneKind::SparkWallet)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("pane.calculator"),
            Some(PaneKind::Calculator)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("calc"),
            Some(PaneKind::Calculator)
        );
        if cfg!(target_os = "macos") {
            assert_eq!(resolve_pane_kind_for_runtime("pane.local_inference"), None);
            assert_eq!(resolve_pane_kind_for_runtime("GPT-OSS Workbench"), None);
            assert_eq!(resolve_pane_kind_for_runtime("gptoss"), None);
        } else {
            assert_eq!(
                resolve_pane_kind_for_runtime("pane.local_inference"),
                Some(PaneKind::LocalInference)
            );
            assert_eq!(
                resolve_pane_kind_for_runtime("GPT-OSS Workbench"),
                Some(PaneKind::LocalInference)
            );
            assert_eq!(
                resolve_pane_kind_for_runtime("gptoss"),
                Some(PaneKind::LocalInference)
            );
        }
        assert_eq!(
            resolve_pane_kind_for_runtime("pane.apple_fm_workbench"),
            Some(PaneKind::AppleFmWorkbench)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("foundation_models"),
            Some(PaneKind::AppleFmWorkbench)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("mission_control"),
            Some(PaneKind::ProviderControl)
        );
        assert_eq!(
            resolve_pane_kind_for_runtime("go_online"),
            Some(PaneKind::ProviderControl)
        );
    }

    #[test]
    fn resolve_pane_kind_rejects_unknown_reference() {
        assert_eq!(resolve_pane_kind_for_runtime("not-a-real-pane"), None);
    }

    #[test]
    fn pane_key_normalization_is_stable() {
        assert_eq!(pane_kind_key(PaneKind::AutopilotChat), "autopilot_chat");
        assert_eq!(pane_kind_key(PaneKind::Calculator), "calculator");
        assert_eq!(
            normalize_key("Spark Lightning Wallet"),
            "spark_lightning_wallet"
        );
        assert_eq!(normalize_key("pane.wallet"), "pane_wallet");
    }

    #[test]
    fn pane_action_mapping_covers_multiple_domains() {
        assert_eq!(
            pane_action_to_hit_action(PaneKind::AutopilotChat, "send", None).expect("chat send"),
            PaneHitAction::ChatSend
        );
        assert_eq!(
            pane_action_to_hit_action(PaneKind::ProviderControl, "open_local_model", None)
                .expect("provider control local model"),
            PaneHitAction::ProviderControl(ProviderControlPaneAction::TriggerLocalRuntimeAction)
        );
        assert_eq!(
            pane_action_to_hit_action(PaneKind::ProviderControl, "test_local_fm", None)
                .expect("provider control local fm test"),
            PaneHitAction::ProviderControl(ProviderControlPaneAction::RunLocalFmSummaryTest)
        );
        assert_eq!(
            pane_action_to_hit_action(PaneKind::ProviderControl, "toggle_product", Some(0))
                .expect("provider inventory toggle"),
            PaneHitAction::ProviderControl(ProviderControlPaneAction::ToggleInventory(
                crate::app_state::ProviderInventoryProductToggleTarget::all()[0],
            ))
        );
        assert_eq!(
            pane_action_to_hit_action(PaneKind::Settings, "save", None).expect("settings save"),
            PaneHitAction::Settings(SettingsPaneAction::Save)
        );
        assert_eq!(
            pane_action_to_hit_action(PaneKind::AppleFmWorkbench, "run_stream", None)
                .expect("apple fm run stream"),
            PaneHitAction::AppleFmWorkbench(
                crate::pane_system::AppleFmWorkbenchPaneAction::RunStream
            )
        );
        assert_eq!(
            pane_action_to_hit_action(PaneKind::SparkWallet, "refresh", None)
                .expect("wallet refresh"),
            PaneHitAction::Spark(SparkPaneAction::Refresh)
        );
    }

    #[test]
    fn pane_action_mapping_requires_index_for_row_actions() {
        let err = pane_action_to_hit_action(PaneKind::RelayConnections, "select_row", None)
            .expect_err("missing index should fail");
        assert_eq!(err.code, "OA-PANE-ACTION-MISSING-INDEX");
        assert_eq!(
            pane_action_to_hit_action(PaneKind::RelayConnections, "select_row", Some(2))
                .expect("with index"),
            PaneHitAction::RelayConnections(RelayConnectionsPaneAction::SelectRow(2))
        );
    }

    #[test]
    fn cad_action_mapping_supports_indexed_actions() {
        assert_eq!(
            cad_action_from_key("bootstrap", None).expect("bootstrap action"),
            CadDemoPaneAction::BootstrapDemo
        );
        assert_eq!(
            cad_action_from_key("snapshot", None).expect("snapshot action"),
            CadDemoPaneAction::Noop
        );
        assert_eq!(
            cad_action_from_key("toggle_viewport_layout", None).expect("toggle layout action"),
            CadDemoPaneAction::ToggleViewportLayout
        );
        assert_eq!(
            cad_action_from_key("toggle_gripper_jaw", None).expect("toggle jaw action"),
            CadDemoPaneAction::ToggleGripperJawAnimation
        );
        assert_eq!(
            cad_action_from_key("status", None).expect("status action"),
            CadDemoPaneAction::Noop
        );
        assert_eq!(
            cad_action_from_key("cycle_sensor_mode", None).expect("sensor mode action"),
            CadDemoPaneAction::CycleSensorVisualizationMode
        );
        assert_eq!(
            cad_action_from_key("select_warning", Some(3)).expect("indexed action"),
            CadDemoPaneAction::SelectWarning(3)
        );
    }

    #[test]
    fn cad_action_mapping_requires_index_when_needed() {
        let err =
            cad_action_from_key("select_warning", None).expect_err("missing index should fail");
        assert_eq!(err.code, "OA-CAD-ACTION-MISSING-INDEX");
    }

    #[test]
    fn cad_checkpoint_payload_contains_required_contract_fields() {
        let mut cad_demo = CadDemoPaneState::default();
        cad_demo.document_revision = 9;
        cad_demo.pending_rebuild_request_id = Some(41);
        cad_demo.warnings = vec![
            CadDemoWarningState {
                warning_id: "warn-1".to_string(),
                code: "W001".to_string(),
                severity: "warning".to_string(),
                message: "first warning".to_string(),
                remediation_hint: "do thing".to_string(),
                semantic_refs: vec!["rack_outer_face".to_string()],
                deep_link: None,
                feature_id: "feature.base".to_string(),
                entity_id: "face.1".to_string(),
            },
            CadDemoWarningState {
                warning_id: "warn-2".to_string(),
                code: "E007".to_string(),
                severity: "error".to_string(),
                message: "second warning".to_string(),
                remediation_hint: "do other thing".to_string(),
                semantic_refs: vec!["vent_face_set".to_string()],
                deep_link: None,
                feature_id: "feature.vents".to_string(),
                entity_id: "face.2".to_string(),
            },
        ];
        let payload = cad_checkpoint_payload(&cad_demo, Some("thread-1"), "test-source");

        assert_eq!(
            payload.get("schema_version"),
            Some(&serde_json::Value::String(
                CAD_CHECKPOINT_SCHEMA_VERSION.to_string()
            ))
        );
        assert_eq!(
            payload.pointer("/source"),
            Some(&serde_json::json!("test-source"))
        );
        assert_eq!(
            payload.pointer("/thread_id"),
            Some(&serde_json::json!("thread-1"))
        );
        assert_eq!(
            payload.pointer("/document/revision"),
            Some(&serde_json::json!(9))
        );
        assert_eq!(
            payload.pointer("/pending_rebuild/is_pending"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            payload.pointer("/pending_rebuild/request_id"),
            Some(&serde_json::json!(41))
        );
        assert_eq!(
            payload.pointer("/warnings/total"),
            Some(&serde_json::json!(2))
        );
        assert_eq!(
            payload.pointer("/warnings/by_severity/error"),
            Some(&serde_json::json!(1))
        );
        assert_eq!(
            payload.pointer("/warnings/by_code/W001"),
            Some(&serde_json::json!(1))
        );
        assert!(
            payload
                .pointer("/analysis/material_id")
                .is_some_and(|value| !value.is_null())
        );
        assert!(payload.pointer("/kinematics/profile").is_some());
        assert!(
            payload
                .pointer("/failure_metrics/intent_parse_failures")
                .is_some()
        );
        assert!(
            payload
                .pointer("/failure_metrics/dispatch_rebuild_failures")
                .is_some()
        );
        assert!(
            payload
                .pointer("/sensor_feedback/visualization_mode")
                .is_some()
        );
        assert!(payload.get("build_session").is_some());
        assert!(payload.get("last_rebuild_receipt").is_some());

        let tool_response = serde_json::json!({
            "schema_version": CAD_TOOL_RESPONSE_SCHEMA_VERSION,
            "checkpoint": payload,
        });
        assert_eq!(
            tool_response.pointer("/schema_version"),
            Some(&serde_json::json!(CAD_TOOL_RESPONSE_SCHEMA_VERSION))
        );
        assert!(
            tool_response
                .pointer("/checkpoint/schema_version")
                .is_some()
        );
    }

    #[test]
    fn cad_checkpoint_payload_reports_single_vs_quad_visibility_truth() {
        let mut cad_demo = CadDemoPaneState::default();
        cad_demo.viewport_layout = CadViewportLayout::Single;
        let single_payload = cad_checkpoint_payload(&cad_demo, Some("thread-1"), "test-source");
        assert_eq!(
            single_payload.pointer("/variant/visible_ids"),
            Some(&serde_json::json!(vec!["variant.baseline"]))
        );
        assert_eq!(
            single_payload.pointer("/variant/all_visible"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            single_payload.pointer("/context/viewport_layout"),
            Some(&serde_json::json!("single"))
        );

        cad_demo.viewport_layout = CadViewportLayout::Quad;
        let quad_payload = cad_checkpoint_payload(&cad_demo, Some("thread-1"), "test-source");
        let visible_ids = quad_payload
            .pointer("/variant/visible_ids")
            .and_then(serde_json::Value::as_array)
            .expect("visible ids should be array");
        assert_eq!(visible_ids.len(), 4);
        assert_eq!(
            quad_payload.pointer("/variant/all_visible"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            quad_payload.pointer("/context/viewport_layout"),
            Some(&serde_json::json!("quad"))
        );
    }

    #[test]
    fn cad_checkpoint_payload_includes_viewport_visibility_truth_fields() {
        let mut cad_demo = CadDemoPaneState::default();
        cad_demo.viewport_layout = CadViewportLayout::Single;
        let single_payload = cad_checkpoint_payload(&cad_demo, Some("thread-1"), "test-source");
        assert_eq!(
            single_payload.pointer("/context/viewport_layout"),
            Some(&serde_json::json!("single"))
        );
        assert_eq!(
            single_payload.pointer("/variant/visible_ids"),
            Some(&serde_json::json!(vec!["variant.baseline"]))
        );
        assert_eq!(
            single_payload.pointer("/variant/all_visible"),
            Some(&serde_json::json!(false))
        );

        cad_demo.viewport_layout = CadViewportLayout::Quad;
        let quad_payload = cad_checkpoint_payload(&cad_demo, Some("thread-1"), "test-source");
        let visible_ids = quad_payload
            .pointer("/variant/visible_ids")
            .and_then(serde_json::Value::as_array)
            .expect("quad visible ids should be array");
        assert_eq!(visible_ids.len(), 4);
        assert_eq!(
            quad_payload.pointer("/variant/all_visible"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            quad_payload.pointer("/context/viewport_layout"),
            Some(&serde_json::json!("quad"))
        );
    }

    #[test]
    fn cad_checkpoint_payload_exposes_underactuation_state_and_parameters() {
        let mut cad_demo = CadDemoPaneState::default();
        cad_demo
            .apply_chat_intent_for_thread(
                "thread-underactuated",
                &openagents_cad::intent::CadIntent::CreateParallelJawGripperSpec(
                    openagents_cad::intent::CreateParallelJawGripperSpecIntent {
                        jaw_open_mm: 36.0,
                        finger_length_mm: 66.0,
                        finger_thickness_mm: 7.5,
                        base_width_mm: 82.0,
                        base_depth_mm: 54.0,
                        base_thickness_mm: 8.5,
                        servo_mount_hole_diameter_mm: 2.9,
                        print_fit_mm: 0.15,
                        print_clearance_mm: 0.35,
                        underactuated_mode: true,
                        compliant_joint_count: 3,
                        flexure_thickness_mm: 1.2,
                        single_servo_drive: true,
                        finger_count: 2,
                        opposable_thumb: false,
                        thumb_base_angle_deg: 42.0,
                        tendon_channel_diameter_mm: 1.8,
                        joint_min_deg: 12.0,
                        joint_max_deg: 82.0,
                        tendon_route_clearance_mm: 1.4,
                        tendon_bend_radius_mm: 3.2,
                        servo_integration_enabled: false,
                        compact_servo_layout: false,
                        servo_envelope_length_mm: 23.0,
                        servo_envelope_width_mm: 12.0,
                        servo_envelope_height_mm: 24.0,
                        servo_shaft_axis_offset_mm: 5.0,
                        servo_mount_pattern_pitch_mm: 16.0,
                        servo_bracket_thickness_mm: 2.6,
                        servo_housing_wall_mm: 2.0,
                        servo_standoff_diameter_mm: 4.2,
                        pose_preset: "open".to_string(),
                    },
                ),
            )
            .expect("underactuated intent should apply");
        let payload =
            cad_checkpoint_payload(&cad_demo, Some("thread-underactuated"), "test-source");
        assert_eq!(
            payload.pointer("/design_profile"),
            Some(&serde_json::json!("parallel_jaw_gripper_underactuated"))
        );
        assert_eq!(
            payload.pointer("/gripper_parameters/underactuated_mode"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            payload.pointer("/gripper_parameters/compliant_joint_count"),
            Some(&serde_json::json!(3))
        );
        assert_eq!(
            payload.pointer("/gripper_parameters/flexure_thickness_mm"),
            Some(&serde_json::json!(1.2))
        );
        assert_eq!(
            payload.pointer("/gripper_parameters/single_servo_drive"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn cad_checkpoint_payload_exposes_kinematic_diagnostics() {
        let mut cad_demo = CadDemoPaneState::default();
        cad_demo
            .analysis_snapshot
            .estimator_metadata
            .extend(std::collections::BTreeMap::from([
                (
                    "kinematic.profile".to_string(),
                    "three_finger_thumb".to_string(),
                ),
                ("kinematic.joint_min_deg".to_string(), "14.000".to_string()),
                ("kinematic.joint_max_deg".to_string(), "86.000".to_string()),
                (
                    "kinematic.travel_span_deg".to_string(),
                    "72.000".to_string(),
                ),
                (
                    "kinematic.nominal_pose_deg".to_string(),
                    "48.500".to_string(),
                ),
                (
                    "kinematic.finger_spacing_mm".to_string(),
                    "17.200".to_string(),
                ),
                (
                    "kinematic.route_clearance_margin_mm".to_string(),
                    "0.820".to_string(),
                ),
                (
                    "kinematic.bend_radius_margin_mm".to_string(),
                    "1.640".to_string(),
                ),
                (
                    "kinematic.nominal_self_intersection".to_string(),
                    "false".to_string(),
                ),
                (
                    "kinematic.nominal_range_valid".to_string(),
                    "true".to_string(),
                ),
            ]));
        let payload = cad_checkpoint_payload(&cad_demo, Some("thread-kinematics"), "test-source");
        assert_eq!(
            payload.pointer("/kinematics/profile"),
            Some(&serde_json::json!("three_finger_thumb"))
        );
        assert_eq!(
            payload.pointer("/kinematics/joint_limits_deg/min"),
            Some(&serde_json::json!(14.0))
        );
        assert_eq!(
            payload.pointer("/kinematics/joint_limits_deg/max"),
            Some(&serde_json::json!(86.0))
        );
        assert_eq!(
            payload.pointer("/kinematics/routing_collision"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            payload.pointer("/kinematics/nominal_range_valid"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn cad_checkpoint_payload_exposes_sensor_feedback_mode_and_latest_readings() {
        let mut cad_demo = CadDemoPaneState::default();
        cad_demo.sensor_visualization_mode = crate::app_state::CadSensorVisualizationMode::Combined;
        cad_demo.sensor_feedback_last_updated_revision = 23;
        cad_demo.sensor_feedback_readings = vec![
            crate::app_state::CadSensorFeedbackReading {
                digit_id: "index".to_string(),
                pressure_ratio: 0.64,
                proximity_mm: 0.92,
                contact: true,
            },
            crate::app_state::CadSensorFeedbackReading {
                digit_id: "thumb".to_string(),
                pressure_ratio: 0.58,
                proximity_mm: 1.14,
                contact: true,
            },
        ];
        cad_demo.sensor_feedback_trace = vec![crate::app_state::CadSensorFeedbackTracePoint {
            document_revision: 23,
            pose_preset: "tripod".to_string(),
            average_pressure_ratio: 0.61,
            minimum_proximity_mm: 0.92,
            contact_count: 2,
        }];

        let payload = cad_checkpoint_payload(&cad_demo, Some("thread-sensor"), "test-source");
        assert_eq!(
            payload.pointer("/sensor_feedback/visualization_mode"),
            Some(&serde_json::json!("combined"))
        );
        assert_eq!(
            payload.pointer("/sensor_feedback/last_updated_revision"),
            Some(&serde_json::json!(23))
        );
        assert_eq!(
            payload.pointer("/sensor_feedback/latest_readings/0/digit_id"),
            Some(&serde_json::json!("index"))
        );
        assert_eq!(
            payload.pointer("/sensor_feedback/latest_readings/0/contact"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            payload.pointer("/sensor_feedback/trace/0/pose_preset"),
            Some(&serde_json::json!("tripod"))
        );
    }

    #[test]
    fn cad_parse_retry_prompt_extracts_embedded_json_object() {
        let prompt = "Please apply this intent:\n```json\n{\"intent\":\"SetMaterial\",\"material_id\":\"al-6061-t6\"}\n```";
        let retry_prompt =
            cad_parse_retry_prompt(prompt).expect("retry helper should extract object payload");
        assert_eq!(
            retry_prompt,
            "{\"intent\":\"SetMaterial\",\"material_id\":\"al-6061-t6\"}"
        );
    }

    #[test]
    fn cad_parse_retry_prompt_rejects_non_object_payloads() {
        assert!(cad_parse_retry_prompt("[1,2,3]").is_none());
        assert!(cad_parse_retry_prompt("no json here").is_none());
    }

    #[test]
    fn parse_bool_env_override_supports_common_values() {
        assert_eq!(parse_bool_env_override("1"), Some(true));
        assert_eq!(parse_bool_env_override("true"), Some(true));
        assert_eq!(parse_bool_env_override("YES"), Some(true));
        assert_eq!(parse_bool_env_override("on"), Some(true));
        assert_eq!(parse_bool_env_override("0"), Some(false));
        assert_eq!(parse_bool_env_override("false"), Some(false));
        assert_eq!(parse_bool_env_override("No"), Some(false));
        assert_eq!(parse_bool_env_override("off"), Some(false));
        assert_eq!(parse_bool_env_override("unexpected"), None);
    }

    #[test]
    fn parse_goal_rollout_stage_accepts_supported_values() {
        assert_eq!(
            parse_goal_rollout_stage("disabled").expect("disabled stage"),
            GoalRolloutStage::Disabled
        );
        assert_eq!(
            parse_goal_rollout_stage("internal_dogfood").expect("internal stage"),
            GoalRolloutStage::InternalDogfood
        );
        assert_eq!(
            parse_goal_rollout_stage("canary").expect("canary stage"),
            GoalRolloutStage::Canary
        );
        assert_eq!(
            parse_goal_rollout_stage("ga").expect("ga stage"),
            GoalRolloutStage::GeneralAvailability
        );
        assert!(parse_goal_rollout_stage("invalid").is_err());
    }

    #[test]
    fn swap_quote_tool_is_allowlisted_and_machine_parseable() {
        let decoded = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_SWAP_QUOTE,
            r#"{"goal_id":"goal-swap-quote","request_id":"req-1","direction":"btc_to_usd","amount":2000,"unit":"sats"}"#,
        ))
        .expect("swap quote tool should decode");
        assert_eq!(decoded.tool, OPENAGENTS_TOOL_SWAP_QUOTE);
    }

    #[test]
    fn treasury_tools_are_allowlisted_and_machine_parseable() {
        let transfer = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_TREASURY_TRANSFER,
            r#"{"from_owner_id":"operator:autopilot","to_owner_id":"sa:wallet-1","asset":"btc_sats","amount":1500}"#,
        ))
        .expect("treasury transfer should decode");
        assert_eq!(transfer.tool, OPENAGENTS_TOOL_TREASURY_TRANSFER);

        let convert = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_TREASURY_CONVERT,
            r#"{"owner_id":"operator:autopilot","direction":"btc-to-usd","amount":2200,"unit":"sats"}"#,
        ))
        .expect("treasury convert should decode");
        assert_eq!(convert.tool, OPENAGENTS_TOOL_TREASURY_CONVERT);

        let receipt = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_TREASURY_RECEIPT,
            r#"{"worker_request_id":12}"#,
        ))
        .expect("treasury receipt should decode");
        assert_eq!(receipt.tool, OPENAGENTS_TOOL_TREASURY_RECEIPT);
    }

    #[test]
    fn treasury_transfer_asset_parser_accepts_supported_values() {
        assert_eq!(
            parse_treasury_transfer_asset("btc_sats").expect("btc asset"),
            crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::BtcSats
        );
        assert_eq!(
            parse_treasury_transfer_asset("usd_cents").expect("usd asset"),
            crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::UsdCents
        );
        assert!(parse_treasury_transfer_asset("invalid").is_err());
    }

    #[test]
    fn swap_direction_and_unit_parsing_is_deterministic() {
        assert_eq!(
            parse_swap_direction("btc_to_usd").expect("direction"),
            SwapDirection::BtcToUsd
        );
        assert_eq!(
            parse_swap_direction("usd-to-btc").expect("direction"),
            SwapDirection::UsdToBtc
        );
        assert_eq!(parse_swap_unit("sats").expect("unit"), SwapAmountUnit::Sats);
        assert_eq!(
            parse_swap_unit("cents").expect("unit"),
            SwapAmountUnit::Cents
        );
        assert!(parse_swap_direction("invalid").is_err());
        assert!(parse_swap_unit("invalid").is_err());

        assert!(validate_direction_unit(SwapDirection::BtcToUsd, SwapAmountUnit::Sats).is_none());
        assert!(validate_direction_unit(SwapDirection::UsdToBtc, SwapAmountUnit::Cents).is_none());
        assert!(validate_direction_unit(SwapDirection::BtcToUsd, SwapAmountUnit::Cents).is_some());
    }

    #[test]
    fn blink_quote_payload_parsing_maps_swap_terms() {
        let raw = serde_json::json!({
            "event": "swap_quote",
            "quote": {
                "quoteId": "blink-swap-123",
                "direction": "BTC_TO_USD",
                "amountIn": { "value": 2000, "unit": "sats" },
                "amountOut": { "value": 130, "unit": "cents" },
                "expiresAtEpochSeconds": 1_800_000_000_u64,
                "immediateExecution": false,
                "feeSats": 0,
                "feeBps": 0,
                "slippageBps": 0
            }
        });
        let quote = parse_blink_quote_terms_from_json(raw).expect("quote parse should succeed");
        assert_eq!(quote.quote_id, "blink-swap-123");
        assert_eq!(quote.direction, SwapDirection::BtcToUsd);
        assert_eq!(quote.amount_in.amount, 2_000);
        assert_eq!(quote.amount_in.unit, SwapAmountUnit::Sats);
        assert_eq!(quote.amount_out.amount, 130);
        assert_eq!(quote.amount_out.unit, SwapAmountUnit::Cents);
    }

    #[test]
    fn blink_execution_payload_parser_rejects_wrong_event() {
        let raw = serde_json::json!({
            "event": "not_swap_execution",
            "status": "SUCCESS",
            "quote": {
                "quoteId": "blink-swap-123",
                "direction": "USD_TO_BTC",
                "amountIn": { "value": 500, "unit": "cents" },
                "amountOut": { "value": 7400, "unit": "sats" },
                "expiresAtEpochSeconds": 1_800_000_000_u64,
                "immediateExecution": false,
                "feeSats": 0,
                "feeBps": 0,
                "slippageBps": 0
            }
        });
        let error = parse_blink_execution_payload_from_json(raw)
            .expect_err("non-execution event should be rejected");
        assert!(error.contains("Unexpected Blink execution event"));
    }

    #[test]
    fn blink_script_selection_errors_when_no_candidate_exists() {
        let error = select_existing_blink_swap_script_path(BTreeSet::new(), "swap_quote.js")
            .expect_err("missing script should fail");
        assert!(error.contains("Unable to locate Blink swap script"));
    }

    #[test]
    fn blink_script_runner_reports_script_failure() {
        let path = temp_js_path("script-failure");
        std::fs::write(
            &path,
            r#"
process.stderr.write("boom\n");
process.exit(7);
"#,
        )
        .expect("write script");

        let error = run_blink_swap_script_json(&path, &[]).expect_err("script should fail");
        assert!(error.contains("exit_code=7"));
        assert!(error.contains("boom"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn blink_script_runner_rejects_non_json_stdout() {
        let path = temp_js_path("non-json");
        std::fs::write(&path, r#"console.log("not-json");"#).expect("write script");

        let error = run_blink_swap_script_json(&path, &[]).expect_err("non-json should fail");
        assert!(error.contains("non-JSON stdout"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn blink_quote_payload_parser_rejects_missing_required_fields() {
        let raw = serde_json::json!({
            "event": "swap_quote",
            "quote": {
                "direction": "BTC_TO_USD",
                "amountIn": { "value": 2000, "unit": "sats" },
                "amountOut": { "value": 130, "unit": "cents" },
                "expiresAtEpochSeconds": 1_800_000_000_u64,
                "immediateExecution": false,
                "feeSats": 0,
                "feeBps": 0,
                "slippageBps": 0
            }
        });
        let error =
            parse_blink_quote_terms_from_json(raw).expect_err("missing quoteId should be rejected");
        assert!(error.contains("Invalid Blink quote payload"));
        assert!(error.contains("quoteId"));
    }

    #[test]
    fn blink_execution_payload_parser_rejects_missing_status() {
        let raw = serde_json::json!({
            "event": "swap_execution",
            "quote": {
                "quoteId": "blink-swap-123",
                "direction": "USD_TO_BTC",
                "amountIn": { "value": 500, "unit": "cents" },
                "amountOut": { "value": 7400, "unit": "sats" },
                "expiresAtEpochSeconds": 1_800_000_000_u64,
                "immediateExecution": false,
                "feeSats": 0,
                "feeBps": 0,
                "slippageBps": 0
            }
        });
        let error = parse_blink_execution_payload_from_json(raw)
            .expect_err("missing status should be rejected");
        assert!(error.contains("Invalid Blink execution payload"));
        assert!(error.contains("status"));
    }
}
