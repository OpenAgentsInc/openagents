use codex_client::{DynamicToolCallOutputContentItem, DynamicToolCallResponse};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app_state::{
    ActivityFeedFilter, AutopilotToolCallRequest, CadBuildFailureClass, PaneKind, RenderState,
};
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::openagents_dynamic_tools::{
    OPENAGENTS_DYNAMIC_TOOL_NAMES, OPENAGENTS_TOOL_CAD_ACTION, OPENAGENTS_TOOL_CAD_INTENT,
    OPENAGENTS_TOOL_PANE_ACTION, OPENAGENTS_TOOL_PANE_CLOSE, OPENAGENTS_TOOL_PANE_FOCUS,
    OPENAGENTS_TOOL_PANE_LIST, OPENAGENTS_TOOL_PANE_OPEN, OPENAGENTS_TOOL_PANE_SET_INPUT,
    OPENAGENTS_TOOL_SWAP_EXECUTE, OPENAGENTS_TOOL_SWAP_QUOTE,
};
use crate::pane_registry::{pane_spec, pane_spec_by_command_id, pane_specs};
use crate::pane_system::{
    ActiveJobPaneAction, ActivityFeedPaneAction, AgentNetworkSimulationPaneAction,
    AgentProfileStatePaneAction, AgentScheduleTickPaneAction, AlertsRecoveryPaneAction,
    CadDemoPaneAction, CodexAccountPaneAction, CodexAppsPaneAction, CodexConfigPaneAction,
    CodexDiagnosticsPaneAction, CodexLabsPaneAction, CodexMcpPaneAction, CodexModelsPaneAction,
    CredentialsPaneAction, CreditDeskPaneAction, CreditSettlementLedgerPaneAction,
    EarningsScoreboardPaneAction, JobHistoryPaneAction, JobInboxPaneAction,
    NetworkRequestsPaneAction, PaneController, PaneHitAction, RelayConnectionsPaneAction,
    RelaySecuritySimulationPaneAction, SettingsPaneAction, SkillRegistryPaneAction,
    SkillTrustRevocationPaneAction, StableSatsSimulationPaneAction, StarterJobsPaneAction,
    SyncHealthPaneAction, TrajectoryAuditPaneAction, TreasuryExchangeSimulationPaneAction,
};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};
use crate::state::swap_contract::{SwapAmount, SwapAmountUnit, SwapDirection, SwapQuoteTerms};
use crate::state::swap_quote_adapter::{
    StablesatsQuoteClient, StablesatsQuoteFor, StablesatsQuoteResponse, SwapQuoteAdapterRequest,
    SwapQuoteProvider,
};

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
    #[serde(default)]
    fallback_quote_id: Option<String>,
    #[serde(default)]
    fallback_amount_out: Option<u64>,
    #[serde(default)]
    stablesats_error: Option<String>,
    #[serde(default)]
    stablesats_quote: Option<SwapStablesatsQuoteArgs>,
}

#[derive(Clone, Debug, Deserialize)]
struct SwapStablesatsQuoteArgs {
    quote_id: String,
    #[serde(default)]
    amount_to_sell_in_sats: Option<u64>,
    #[serde(default)]
    amount_to_buy_in_cents: Option<u64>,
    #[serde(default)]
    amount_to_buy_in_sats: Option<u64>,
    #[serde(default)]
    amount_to_sell_in_cents: Option<u64>,
    expires_at_epoch_seconds: u64,
    executed: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct SwapExecuteArgs {
    goal_id: String,
    quote_id: String,
    status: String,
    #[serde(default)]
    transaction_id: Option<String>,
    #[serde(default)]
    failure_reason: Option<String>,
}

struct ToolBridgeStablesatsClient {
    quote: Option<StablesatsQuoteResponse>,
    unavailable_reason: Option<String>,
    fail_accept_reason: Option<String>,
    accepted_quote_ids: Vec<String>,
}

impl ToolBridgeStablesatsClient {
    fn from_args(args: &SwapQuoteArgs) -> Self {
        let quote = args
            .stablesats_quote
            .as_ref()
            .map(|value| StablesatsQuoteResponse {
                quote_id: value.quote_id.clone(),
                amount_to_sell_in_sats: value.amount_to_sell_in_sats,
                amount_to_buy_in_cents: value.amount_to_buy_in_cents,
                amount_to_buy_in_sats: value.amount_to_buy_in_sats,
                amount_to_sell_in_cents: value.amount_to_sell_in_cents,
                expires_at_epoch_seconds: value.expires_at_epoch_seconds,
                executed: value.executed,
            });
        Self {
            quote,
            unavailable_reason: args.stablesats_error.clone(),
            fail_accept_reason: None,
            accepted_quote_ids: Vec::new(),
        }
    }
}

impl StablesatsQuoteClient for ToolBridgeStablesatsClient {
    fn get_quote_to_buy_usd(
        &mut self,
        _quote_for: StablesatsQuoteFor,
        _immediate_execution: bool,
    ) -> Result<StablesatsQuoteResponse, String> {
        if let Some(reason) = self.unavailable_reason.clone() {
            return Err(reason);
        }
        self.quote
            .clone()
            .ok_or_else(|| "stablesats quote unavailable".to_string())
    }

    fn get_quote_to_sell_usd(
        &mut self,
        _quote_for: StablesatsQuoteFor,
        _immediate_execution: bool,
    ) -> Result<StablesatsQuoteResponse, String> {
        if let Some(reason) = self.unavailable_reason.clone() {
            return Err(reason);
        }
        self.quote
            .clone()
            .ok_or_else(|| "stablesats quote unavailable".to_string())
    }

    fn accept_quote(&mut self, quote_id: &str) -> Result<(), String> {
        if let Some(reason) = self.fail_accept_reason.clone() {
            return Err(reason);
        }
        self.accepted_quote_ids.push(quote_id.to_string());
        Ok(())
    }
}

pub(super) fn execute_openagents_tool_request(
    state: &mut RenderState,
    request: &AutopilotToolCallRequest,
) -> ToolBridgeResultEnvelope {
    let decoded = match decode_tool_call_request(request) {
        Ok(value) => value,
        Err(error) => return error,
    };

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

fn execute_pane_list(state: &RenderState) -> ToolBridgeResultEnvelope {
    let registered = pane_specs()
        .iter()
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
    let Some(kind) = resolve_pane_kind(pane_ref) else {
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
    let Some(kind) = resolve_pane_kind(pane_ref) else {
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
    let Some(kind) = resolve_pane_kind(pane_ref) else {
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
    let Some(kind) = resolve_pane_kind(args.pane.trim()) else {
        return pane_resolution_error("OA-PANE-SET-INPUT-NOT-FOUND", args.pane.trim());
    };
    let _ = PaneController::create_for_kind(state, kind);
    let field = normalize_key(&args.field);
    let value = args.value.clone();

    let applied = match kind {
        PaneKind::AutopilotChat => apply_chat_input(state, &field, &value),
        PaneKind::RelayConnections => apply_relay_connections_input(state, &field, &value),
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
    let Some(kind) = resolve_pane_kind(args.pane.trim()) else {
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
            PaneKind::CadDemo => {
                map.insert(
                    "cad".to_string(),
                    json!({
                        "session_id": state.cad_demo.session_id,
                        "document_revision": state.cad_demo.document_revision,
                        "active_variant_id": state.cad_demo.active_variant_id,
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
        PaneKind::GoOnline => match action {
            "toggle" | "set_online" => Ok(PaneHitAction::GoOnlineToggle),
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
        PaneKind::NetworkRequests => match action {
            "submit" | "submit_request" => Ok(PaneHitAction::NetworkRequests(
                NetworkRequestsPaneAction::SubmitRequest,
            )),
            _ => unsupported(),
        },
        PaneKind::StarterJobs => match action {
            "complete_selected" => Ok(PaneHitAction::StarterJobs(
                StarterJobsPaneAction::CompleteSelected,
            )),
            "select_row" => Ok(PaneHitAction::StarterJobs(
                StarterJobsPaneAction::SelectRow(require_index(action)?),
            )),
            _ => unsupported(),
        },
        PaneKind::ActivityFeed => match action {
            "refresh" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::Refresh)),
            "select_row" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SelectRow(require_index(action)?),
            )),
            "filter_all" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::All),
            )),
            "filter_chat" => Ok(PaneHitAction::ActivityFeed(
                ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Chat),
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
        PaneKind::AgentNetworkSimulation => match action {
            "run_round" => Ok(PaneHitAction::AgentNetworkSimulation(
                AgentNetworkSimulationPaneAction::RunRound,
            )),
            "reset" => Ok(PaneHitAction::AgentNetworkSimulation(
                AgentNetworkSimulationPaneAction::Reset,
            )),
            _ => unsupported(),
        },
        PaneKind::TreasuryExchangeSimulation => match action {
            "run_round" => Ok(PaneHitAction::TreasuryExchangeSimulation(
                TreasuryExchangeSimulationPaneAction::RunRound,
            )),
            "reset" => Ok(PaneHitAction::TreasuryExchangeSimulation(
                TreasuryExchangeSimulationPaneAction::Reset,
            )),
            _ => unsupported(),
        },
        PaneKind::RelaySecuritySimulation => match action {
            "run_round" => Ok(PaneHitAction::RelaySecuritySimulation(
                RelaySecuritySimulationPaneAction::RunRound,
            )),
            "reset" => Ok(PaneHitAction::RelaySecuritySimulation(
                RelaySecuritySimulationPaneAction::Reset,
            )),
            _ => unsupported(),
        },
        PaneKind::StableSatsSimulation => match action {
            "run_round" => Ok(PaneHitAction::StableSatsSimulation(
                StableSatsSimulationPaneAction::RunRound,
            )),
            "reset" => Ok(PaneHitAction::StableSatsSimulation(
                StableSatsSimulationPaneAction::Reset,
            )),
            _ => unsupported(),
        },
        PaneKind::CadDemo => match action {
            "bootstrap" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::BootstrapDemo)),
            "cycle_variant" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleVariant)),
            "reset_camera" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ResetCamera)),
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

fn apply_network_requests_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "request_type" => state
            .network_requests_inputs
            .request_type
            .set_value(value.to_string()),
        "payload" => state
            .network_requests_inputs
            .payload
            .set_value(value.to_string()),
        "skill_scope_id" => state
            .network_requests_inputs
            .skill_scope_id
            .set_value(value.to_string()),
        "credit_envelope_ref" => state
            .network_requests_inputs
            .credit_envelope_ref
            .set_value(value.to_string()),
        "budget_sats" => state
            .network_requests_inputs
            .budget_sats
            .set_value(value.to_string()),
        "timeout_seconds" => state
            .network_requests_inputs
            .timeout_seconds
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
        },
        "context": {
            "session_id": cad_demo.session_id,
            "active_chat_session_id": cad_demo.active_chat_session_id,
            "dispatch_session_count": cad_demo.dispatch_sessions.len(),
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
        "build_session": cad_build_session_checkpoint(cad_demo),
        "last_rebuild_receipt": last_receipt,
        "last_action": cad_demo.last_action,
        "last_error": cad_demo.last_error,
    })
}

fn cad_failure_class_label(class: CadBuildFailureClass) -> &'static str {
    class.label()
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

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let quote_ttl_seconds = args.quote_ttl_seconds.unwrap_or(60).max(1);

    let request = SwapQuoteAdapterRequest {
        request_id: args.request_id.clone(),
        direction,
        amount: SwapAmount {
            amount: args.amount,
            unit,
        },
        immediate_execution: args.immediate_execution,
        now_epoch_seconds: now,
    };

    let fallback_quote = build_fallback_quote(args, direction, unit, now, quote_ttl_seconds);

    let mut stablesats_client = ToolBridgeStablesatsClient::from_args(args);
    let outcome = match state.autopilot_goals.request_swap_quote_with_adapter(
        args.goal_id.trim(),
        &request,
        &mut stablesats_client,
        fallback_quote,
    ) {
        Ok(value) => value,
        Err(error) => {
            return ToolBridgeResultEnvelope::error(
                "OA-SWAP-QUOTE-FAILED",
                format!("Swap quote request failed: {error}"),
                json!({
                    "goal_id": args.goal_id,
                    "request_id": args.request_id,
                }),
            );
        }
    };

    state.autopilot_chat.record_turn_timeline_event(format!(
        "swap quote requested goal={} request={} direction={:?}",
        args.goal_id, args.request_id, direction
    ));
    if outcome.provider == SwapQuoteProvider::StablesatsQuoteService && outcome.accepted_via_adapter
    {
        state.autopilot_chat.record_turn_timeline_event(format!(
            "swap quote accepted goal={} quote={}",
            args.goal_id, outcome.quote.quote_id
        ));
    }

    let mut events = vec![json!({
        "event": "swap_quote_requested",
        "goal_id": args.goal_id,
        "request_id": args.request_id,
        "direction": format!("{:?}", direction),
    })];
    if outcome.provider == SwapQuoteProvider::StablesatsQuoteService && outcome.accepted_via_adapter
    {
        events.push(json!({
            "event": "swap_quote_accepted",
            "goal_id": args.goal_id,
            "quote_id": outcome.quote.quote_id,
        }));
    }

    ToolBridgeResultEnvelope::ok(
        "OA-SWAP-QUOTE-OK",
        "Swap quote requested through controlled adapter",
        json!({
            "goal_id": args.goal_id,
            "request_id": args.request_id,
            "provider": match outcome.provider {
                SwapQuoteProvider::StablesatsQuoteService => "stablesats_quote_service",
                SwapQuoteProvider::BlinkFallback => "blink_fallback",
            },
            "quote": {
                "quote_id": outcome.quote.quote_id,
                "direction": format!("{:?}", outcome.quote.direction),
                "amount_in": {
                    "amount": outcome.quote.amount_in.amount,
                    "unit": match outcome.quote.amount_in.unit {
                        SwapAmountUnit::Sats => "sats",
                        SwapAmountUnit::Cents => "cents",
                    },
                },
                "amount_out": {
                    "amount": outcome.quote.amount_out.amount,
                    "unit": match outcome.quote.amount_out.unit {
                        SwapAmountUnit::Sats => "sats",
                        SwapAmountUnit::Cents => "cents",
                    },
                },
                "expires_at_epoch_seconds": outcome.quote.expires_at_epoch_seconds,
                "immediate_execution": outcome.quote.immediate_execution,
            },
            "accepted_via_adapter": outcome.accepted_via_adapter,
            "fallback_reason": outcome.fallback_reason,
            "events": events,
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

    let status = args.status.trim().to_ascii_uppercase();
    if !matches!(
        status.as_str(),
        "SUCCESS" | "FAILURE" | "PENDING" | "ALREADY_PAID"
    ) {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-INVALID-STATUS",
            "status must be SUCCESS, FAILURE, PENDING, or ALREADY_PAID",
            json!({ "status": args.status }),
        );
    }

    let audit = state
        .autopilot_goals
        .document
        .swap_quote_audits
        .iter()
        .rev()
        .find(|entry| entry.goal_id == args.goal_id && entry.quote_id == args.quote_id)
        .cloned();
    let Some(audit) = audit else {
        return ToolBridgeResultEnvelope::error(
            "OA-SWAP-EXECUTE-QUOTE-NOT-FOUND",
            format!(
                "No swap quote audit found for goal '{}' and quote '{}'",
                args.goal_id, args.quote_id
            ),
            json!({
                "goal_id": args.goal_id,
                "quote_id": args.quote_id,
            }),
        );
    };

    let mut events = Vec::<Value>::new();
    match status.as_str() {
        "SUCCESS" => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "swap settled goal={} quote={} tx={}",
                args.goal_id,
                args.quote_id,
                args.transaction_id
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .unwrap_or("unknown")
            ));
            events.push(json!({
                "event": "swap_settled",
                "goal_id": args.goal_id,
                "quote_id": args.quote_id,
                "transaction_id": args.transaction_id,
            }));
        }
        "FAILURE" => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "swap failed goal={} quote={} reason={}",
                args.goal_id,
                args.quote_id,
                args.failure_reason
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("unknown")
            ));
            events.push(json!({
                "event": "swap_failed",
                "goal_id": args.goal_id,
                "quote_id": args.quote_id,
                "reason": args.failure_reason,
            }));
        }
        _ => {}
    }

    ToolBridgeResultEnvelope::ok(
        "OA-SWAP-EXECUTE-OK",
        "Swap execution status recorded",
        json!({
            "goal_id": args.goal_id,
            "quote_id": args.quote_id,
            "status": status,
            "transaction_id": args.transaction_id,
            "failure_reason": args.failure_reason,
            "audit": {
                "provider": match audit.provider {
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
            "events": events,
        }),
    )
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

fn build_fallback_quote(
    args: &SwapQuoteArgs,
    direction: SwapDirection,
    unit: SwapAmountUnit,
    now_epoch_seconds: u64,
    quote_ttl_seconds: u64,
) -> SwapQuoteTerms {
    let quote_id = args
        .fallback_quote_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("blink-fallback-quote")
        .to_string();
    let amount_out = args.fallback_amount_out.unwrap_or(match direction {
        SwapDirection::BtcToUsd => std::cmp::max(1, args.amount / 15),
        SwapDirection::UsdToBtc => args.amount.saturating_mul(15),
    });

    let (amount_in_value, amount_in_unit, amount_out_unit) = match (direction, unit) {
        (SwapDirection::BtcToUsd, SwapAmountUnit::Sats) => {
            (args.amount, SwapAmountUnit::Sats, SwapAmountUnit::Cents)
        }
        (SwapDirection::UsdToBtc, SwapAmountUnit::Cents) => {
            (args.amount, SwapAmountUnit::Cents, SwapAmountUnit::Sats)
        }
        _ => (args.amount, unit, unit),
    };

    SwapQuoteTerms {
        quote_id,
        direction,
        amount_in: SwapAmount {
            amount: amount_in_value,
            unit: amount_in_unit,
        },
        amount_out: SwapAmount {
            amount: amount_out,
            unit: amount_out_unit,
        },
        expires_at_epoch_seconds: now_epoch_seconds.saturating_add(quote_ttl_seconds),
        immediate_execution: args.immediate_execution,
        fee_sats: 0,
        fee_bps: 0,
        slippage_bps: 0,
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
        "reset_camera" => CadDemoPaneAction::ResetCamera,
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
            "supported_panes": pane_specs()
                .iter()
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

fn resolve_pane_kind(raw: &str) -> Option<PaneKind> {
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

    pane_specs()
        .iter()
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
        PaneKind::SparkWallet => &["wallet", "spark_wallet"],
        PaneKind::SparkCreateInvoice => &["create_invoice", "invoice_create"],
        PaneKind::SparkPayInvoice => &["pay_invoice", "invoice_pay"],
        PaneKind::NostrIdentity => &["identity", "identity_keys", "nostr"],
        PaneKind::CadDemo => &["cad", "cad_demo"],
        _ => &[],
    }
}

fn pane_kind_key(kind: PaneKind) -> &'static str {
    match kind {
        PaneKind::Empty => "pane",
        PaneKind::AutopilotChat => "autopilot_chat",
        PaneKind::CodexAccount => "codex_account",
        PaneKind::CodexModels => "codex_models",
        PaneKind::CodexConfig => "codex_config",
        PaneKind::CodexMcp => "codex_mcp",
        PaneKind::CodexApps => "codex_apps",
        PaneKind::CodexLabs => "codex_labs",
        PaneKind::CodexDiagnostics => "codex_diagnostics",
        PaneKind::GoOnline => "go_online",
        PaneKind::ProviderStatus => "provider_status",
        PaneKind::EarningsScoreboard => "earnings_scoreboard",
        PaneKind::RelayConnections => "relay_connections",
        PaneKind::SyncHealth => "sync_health",
        PaneKind::NetworkRequests => "network_requests",
        PaneKind::StarterJobs => "starter_jobs",
        PaneKind::ActivityFeed => "activity_feed",
        PaneKind::AlertsRecovery => "alerts_recovery",
        PaneKind::Settings => "settings",
        PaneKind::Credentials => "credentials",
        PaneKind::JobInbox => "job_inbox",
        PaneKind::ActiveJob => "active_job",
        PaneKind::JobHistory => "job_history",
        PaneKind::NostrIdentity => "nostr_identity",
        PaneKind::SparkWallet => "spark_wallet",
        PaneKind::SparkCreateInvoice => "spark_create_invoice",
        PaneKind::SparkPayInvoice => "spark_pay_invoice",
        PaneKind::AgentProfileState => "agent_profile_state",
        PaneKind::AgentScheduleTick => "agent_schedule_tick",
        PaneKind::TrajectoryAudit => "trajectory_audit",
        PaneKind::SkillRegistry => "skill_registry",
        PaneKind::SkillTrustRevocation => "skill_trust_revocation",
        PaneKind::CreditDesk => "credit_desk",
        PaneKind::CreditSettlementLedger => "credit_settlement_ledger",
        PaneKind::AgentNetworkSimulation => "agent_network_simulation",
        PaneKind::TreasuryExchangeSimulation => "treasury_exchange_simulation",
        PaneKind::RelaySecuritySimulation => "relay_security_simulation",
        PaneKind::StableSatsSimulation => "stable_sats_simulation",
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
        LEGACY_OPENAGENTS_TOOL_PANE_OPEN, OPENAGENTS_TOOL_PANE_OPEN, OPENAGENTS_TOOL_SWAP_QUOTE,
        ToolBridgeResultEnvelope, build_fallback_quote, cad_action_from_key,
        cad_checkpoint_payload, cad_parse_retry_prompt, decode_tool_call_request, normalize_key,
        pane_action_to_hit_action, pane_kind_key, parse_bool_env_override, parse_swap_direction,
        parse_swap_unit, resolve_pane_kind, validate_direction_unit,
    };
    use crate::app_state::{
        AutopilotToolCallRequest, CadDemoPaneState, CadDemoWarningState, PaneKind,
    };
    use crate::pane_system::{
        CadDemoPaneAction, PaneHitAction, RelayConnectionsPaneAction, SettingsPaneAction,
    };
    use crate::spark_pane::SparkPaneAction;
    use crate::state::swap_contract::{SwapAmountUnit, SwapDirection};
    use codex_client::AppServerRequestId;
    use serde::Deserialize;

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
    fn resolve_pane_kind_accepts_command_id_title_and_alias() {
        assert_eq!(
            resolve_pane_kind("pane.wallet"),
            Some(PaneKind::SparkWallet)
        );
        assert_eq!(
            resolve_pane_kind("Spark Lightning Wallet"),
            Some(PaneKind::SparkWallet)
        );
        assert_eq!(resolve_pane_kind("wallet"), Some(PaneKind::SparkWallet));
    }

    #[test]
    fn resolve_pane_kind_rejects_unknown_reference() {
        assert_eq!(resolve_pane_kind("not-a-real-pane"), None);
    }

    #[test]
    fn pane_key_normalization_is_stable() {
        assert_eq!(pane_kind_key(PaneKind::AutopilotChat), "autopilot_chat");
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
            pane_action_to_hit_action(PaneKind::Settings, "save", None).expect("settings save"),
            PaneHitAction::Settings(SettingsPaneAction::Save)
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
            cad_action_from_key("status", None).expect("status action"),
            CadDemoPaneAction::Noop
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
    fn swap_quote_tool_is_allowlisted_and_machine_parseable() {
        let decoded = decode_tool_call_request(&request(
            OPENAGENTS_TOOL_SWAP_QUOTE,
            r#"{"goal_id":"goal-swap-quote","request_id":"req-1","direction":"btc_to_usd","amount":2000,"unit":"sats"}"#,
        ))
        .expect("swap quote tool should decode");
        assert_eq!(decoded.tool, OPENAGENTS_TOOL_SWAP_QUOTE);
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
    fn fallback_quote_builder_uses_stable_shape() {
        let args = serde_json::from_str::<super::SwapQuoteArgs>(
            r#"{
                "goal_id":"goal-1",
                "request_id":"req-1",
                "direction":"btc_to_usd",
                "amount":3000,
                "unit":"sats",
                "fallback_quote_id":"quote-xyz",
                "fallback_amount_out":220
            }"#,
        )
        .expect("quote args");
        let quote = build_fallback_quote(
            &args,
            SwapDirection::BtcToUsd,
            SwapAmountUnit::Sats,
            1_700_000_000,
            60,
        );
        assert_eq!(quote.quote_id, "quote-xyz");
        assert_eq!(quote.amount_in.amount, 3_000);
        assert_eq!(quote.amount_out.amount, 220);
        assert_eq!(quote.expires_at_epoch_seconds, 1_700_000_060);
    }
}
