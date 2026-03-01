use codex_client::{DynamicToolCallOutputContentItem, DynamicToolCallResponse};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app_state::{ActivityFeedFilter, AutopilotToolCallRequest, PaneKind, RenderState};
use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::pane_registry::{pane_spec, pane_spec_by_command_id, pane_specs};
use crate::pane_system::{
    ActiveJobPaneAction, ActivityFeedPaneAction, AgentNetworkSimulationPaneAction,
    AgentProfileStatePaneAction, AgentScheduleTickPaneAction, AlertsRecoveryPaneAction,
    CadDemoPaneAction, CodexAccountPaneAction, CodexAppsPaneAction, CodexConfigPaneAction,
    CodexDiagnosticsPaneAction, CodexLabsPaneAction, CodexMcpPaneAction, CodexModelsPaneAction,
    CreditDeskPaneAction, CreditSettlementLedgerPaneAction, CredentialsPaneAction,
    EarningsScoreboardPaneAction, JobHistoryPaneAction, JobInboxPaneAction,
    NetworkRequestsPaneAction, PaneController, PaneHitAction, RelayConnectionsPaneAction,
    RelaySecuritySimulationPaneAction, SettingsPaneAction, SkillRegistryPaneAction,
    SkillTrustRevocationPaneAction, StableSatsSimulationPaneAction, StarterJobsPaneAction,
    SyncHealthPaneAction, TrajectoryAuditPaneAction, TreasuryExchangeSimulationPaneAction,
};
use crate::spark_pane::{CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction};

pub(super) const OPENAGENTS_TOOL_PREFIX: &str = "openagents.";
pub(super) const OPENAGENTS_TOOL_NAMES: &[&str] = &[
    "openagents.pane.list",
    "openagents.pane.open",
    "openagents.pane.focus",
    "openagents.pane.close",
    "openagents.pane.set_input",
    "openagents.pane.action",
    "openagents.cad.intent",
    "openagents.cad.action",
];

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
    if !tool.starts_with(OPENAGENTS_TOOL_PREFIX) || !is_supported_tool(tool) {
        return Err(ToolBridgeResultEnvelope::error(
            "OA-TOOL-UNSUPPORTED",
            format!(
                "Unsupported tool '{}'. Supported tools must be in '{}' namespace and allowlisted.",
                request.tool, OPENAGENTS_TOOL_PREFIX
            ),
            json!({
                "tool": request.tool,
                "supported_tools": OPENAGENTS_TOOL_NAMES,
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

pub(super) fn execute_openagents_tool_request(
    state: &mut RenderState,
    request: &AutopilotToolCallRequest,
) -> ToolBridgeResultEnvelope {
    let decoded = match decode_tool_call_request(request) {
        Ok(value) => value,
        Err(error) => return error,
    };

    match decoded.tool.as_str() {
        "openagents.pane.list" => execute_pane_list(state),
        "openagents.pane.open" => {
            let args = match decoded.decode_arguments::<PaneRefArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_open(state, args.pane.trim())
        }
        "openagents.pane.focus" => {
            let args = match decoded.decode_arguments::<PaneRefArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_focus(state, args.pane.trim())
        }
        "openagents.pane.close" => {
            let args = match decoded.decode_arguments::<PaneRefArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_close(state, args.pane.trim())
        }
        "openagents.pane.set_input" => {
            let args = match decoded.decode_arguments::<PaneInputArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_set_input(state, &args)
        }
        "openagents.pane.action" => {
            let args = match decoded.decode_arguments::<PaneActionArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_pane_action(state, &args)
        }
        "openagents.cad.intent" => {
            let args = match decoded.decode_arguments::<CadIntentArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_cad_intent(state, &args)
        }
        "openagents.cad.action" => {
            let args = match decoded.decode_arguments::<CadActionArgs>() {
                Ok(value) => value,
                Err(error) => return error,
            };
            execute_cad_action(state, &args)
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
    OPENAGENTS_TOOL_NAMES.iter().any(|entry| *entry == tool)
}

pub(super) fn is_openagents_tool_namespace(tool: &str) -> bool {
    tool.trim().starts_with(OPENAGENTS_TOOL_PREFIX)
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

fn execute_pane_set_input(state: &mut RenderState, args: &PaneInputArgs) -> ToolBridgeResultEnvelope {
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
            "select_thread" | "select_row" => Ok(PaneHitAction::ChatSelectThread(require_index(action)?)),
            _ => unsupported(),
        },
        PaneKind::GoOnline => match action {
            "toggle" | "set_online" => Ok(PaneHitAction::GoOnlineToggle),
            _ => unsupported(),
        },
        PaneKind::CodexAccount => match action {
            "refresh" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::Refresh)),
            "login_chatgpt" | "login" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::LoginChatgpt)),
            "cancel_login" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::CancelLogin)),
            "logout" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::Logout)),
            "rate_limits" => Ok(PaneHitAction::CodexAccount(CodexAccountPaneAction::RateLimits)),
            _ => unsupported(),
        },
        PaneKind::CodexModels => match action {
            "refresh" => Ok(PaneHitAction::CodexModels(CodexModelsPaneAction::Refresh)),
            "toggle_hidden" => Ok(PaneHitAction::CodexModels(CodexModelsPaneAction::ToggleHidden)),
            _ => unsupported(),
        },
        PaneKind::CodexConfig => match action {
            "read" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::Read)),
            "requirements" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::Requirements)),
            "write_sample" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::WriteSample)),
            "batch_write_sample" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::BatchWriteSample)),
            "detect_external" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::DetectExternal)),
            "import_external" => Ok(PaneHitAction::CodexConfig(CodexConfigPaneAction::ImportExternal)),
            _ => unsupported(),
        },
        PaneKind::CodexMcp => match action {
            "refresh" => Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::Refresh)),
            "login_selected" | "login" => Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::LoginSelected)),
            "reload" => Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::Reload)),
            "select_row" | "select_server" => Ok(PaneHitAction::CodexMcp(CodexMcpPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::CodexApps => match action {
            "refresh" => Ok(PaneHitAction::CodexApps(CodexAppsPaneAction::Refresh)),
            "select_row" | "select_app" => Ok(PaneHitAction::CodexApps(CodexAppsPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::CodexLabs => match action {
            "review_inline" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::ReviewInline)),
            "review_detached" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::ReviewDetached)),
            "command_exec" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::CommandExec)),
            "collaboration_modes" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::CollaborationModes)),
            "experimental_features" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::ExperimentalFeatures)),
            "toggle_experimental" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::ToggleExperimental)),
            "realtime_start" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeStart)),
            "realtime_append_text" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeAppendText)),
            "realtime_stop" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::RealtimeStop)),
            "windows_sandbox_setup" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::WindowsSandboxSetup)),
            "fuzzy_start" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyStart)),
            "fuzzy_update" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyUpdate)),
            "fuzzy_stop" => Ok(PaneHitAction::CodexLabs(CodexLabsPaneAction::FuzzyStop)),
            _ => unsupported(),
        },
        PaneKind::CodexDiagnostics => match action {
            "enable_wire_log" => Ok(PaneHitAction::CodexDiagnostics(CodexDiagnosticsPaneAction::EnableWireLog)),
            "disable_wire_log" => Ok(PaneHitAction::CodexDiagnostics(CodexDiagnosticsPaneAction::DisableWireLog)),
            "clear_events" => Ok(PaneHitAction::CodexDiagnostics(CodexDiagnosticsPaneAction::ClearEvents)),
            _ => unsupported(),
        },
        PaneKind::EarningsScoreboard => match action {
            "refresh" => Ok(PaneHitAction::EarningsScoreboard(EarningsScoreboardPaneAction::Refresh)),
            _ => unsupported(),
        },
        PaneKind::RelayConnections => match action {
            "add_relay" => Ok(PaneHitAction::RelayConnections(RelayConnectionsPaneAction::AddRelay)),
            "remove_selected" => Ok(PaneHitAction::RelayConnections(RelayConnectionsPaneAction::RemoveSelected)),
            "retry_selected" => Ok(PaneHitAction::RelayConnections(RelayConnectionsPaneAction::RetrySelected)),
            "select_row" => Ok(PaneHitAction::RelayConnections(RelayConnectionsPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::SyncHealth => match action {
            "rebootstrap" => Ok(PaneHitAction::SyncHealth(SyncHealthPaneAction::Rebootstrap)),
            _ => unsupported(),
        },
        PaneKind::NetworkRequests => match action {
            "submit" | "submit_request" => Ok(PaneHitAction::NetworkRequests(NetworkRequestsPaneAction::SubmitRequest)),
            _ => unsupported(),
        },
        PaneKind::StarterJobs => match action {
            "complete_selected" => Ok(PaneHitAction::StarterJobs(StarterJobsPaneAction::CompleteSelected)),
            "select_row" => Ok(PaneHitAction::StarterJobs(StarterJobsPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::ActivityFeed => match action {
            "refresh" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::Refresh)),
            "select_row" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SelectRow(require_index(action)?))),
            "filter_all" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::All))),
            "filter_chat" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Chat))),
            "filter_job" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Job))),
            "filter_wallet" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Wallet))),
            "filter_network" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Network))),
            "filter_sync" => Ok(PaneHitAction::ActivityFeed(ActivityFeedPaneAction::SetFilter(ActivityFeedFilter::Sync))),
            _ => unsupported(),
        },
        PaneKind::AlertsRecovery => match action {
            "recover_selected" => Ok(PaneHitAction::AlertsRecovery(AlertsRecoveryPaneAction::RecoverSelected)),
            "acknowledge_selected" => Ok(PaneHitAction::AlertsRecovery(AlertsRecoveryPaneAction::AcknowledgeSelected)),
            "resolve_selected" => Ok(PaneHitAction::AlertsRecovery(AlertsRecoveryPaneAction::ResolveSelected)),
            "select_row" => Ok(PaneHitAction::AlertsRecovery(AlertsRecoveryPaneAction::SelectRow(require_index(action)?))),
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
            "delete_or_clear" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::DeleteOrClear)),
            "toggle_enabled" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::ToggleEnabled)),
            "toggle_scope_codex" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::ToggleScopeCodex)),
            "toggle_scope_spark" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::ToggleScopeSpark)),
            "toggle_scope_skills" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::ToggleScopeSkills)),
            "toggle_scope_global" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::ToggleScopeGlobal)),
            "import_from_env" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::ImportFromEnv)),
            "reload" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::Reload)),
            "select_row" => Ok(PaneHitAction::Credentials(CredentialsPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::JobInbox => match action {
            "accept_selected" => Ok(PaneHitAction::JobInbox(JobInboxPaneAction::AcceptSelected)),
            "reject_selected" => Ok(PaneHitAction::JobInbox(JobInboxPaneAction::RejectSelected)),
            "select_row" => Ok(PaneHitAction::JobInbox(JobInboxPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::ActiveJob => match action {
            "advance_stage" => Ok(PaneHitAction::ActiveJob(ActiveJobPaneAction::AdvanceStage)),
            "abort_job" => Ok(PaneHitAction::ActiveJob(ActiveJobPaneAction::AbortJob)),
            _ => unsupported(),
        },
        PaneKind::JobHistory => match action {
            "cycle_status_filter" => Ok(PaneHitAction::JobHistory(JobHistoryPaneAction::CycleStatusFilter)),
            "cycle_time_range" => Ok(PaneHitAction::JobHistory(JobHistoryPaneAction::CycleTimeRange)),
            "previous_page" => Ok(PaneHitAction::JobHistory(JobHistoryPaneAction::PreviousPage)),
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
            "generate_spark_address" => Ok(PaneHitAction::Spark(SparkPaneAction::GenerateSparkAddress)),
            "generate_bitcoin_address" => Ok(PaneHitAction::Spark(SparkPaneAction::GenerateBitcoinAddress)),
            "copy_spark_address" => Ok(PaneHitAction::Spark(SparkPaneAction::CopySparkAddress)),
            "create_invoice" => Ok(PaneHitAction::Spark(SparkPaneAction::CreateInvoice)),
            "send_payment" => Ok(PaneHitAction::Spark(SparkPaneAction::SendPayment)),
            _ => unsupported(),
        },
        PaneKind::SparkCreateInvoice => match action {
            "create_invoice" => Ok(PaneHitAction::SparkCreateInvoice(CreateInvoicePaneAction::CreateInvoice)),
            "copy_invoice" => Ok(PaneHitAction::SparkCreateInvoice(CreateInvoicePaneAction::CopyInvoice)),
            _ => unsupported(),
        },
        PaneKind::SparkPayInvoice => match action {
            "send_payment" => Ok(PaneHitAction::SparkPayInvoice(PayInvoicePaneAction::SendPayment)),
            _ => unsupported(),
        },
        PaneKind::AgentProfileState => match action {
            "publish_profile" => Ok(PaneHitAction::AgentProfileState(AgentProfileStatePaneAction::PublishProfile)),
            "publish_state" => Ok(PaneHitAction::AgentProfileState(AgentProfileStatePaneAction::PublishState)),
            "update_goals" => Ok(PaneHitAction::AgentProfileState(AgentProfileStatePaneAction::UpdateGoals)),
            _ => unsupported(),
        },
        PaneKind::AgentScheduleTick => match action {
            "apply_schedule" => Ok(PaneHitAction::AgentScheduleTick(AgentScheduleTickPaneAction::ApplySchedule)),
            "publish_manual_tick" => Ok(PaneHitAction::AgentScheduleTick(AgentScheduleTickPaneAction::PublishManualTick)),
            "inspect_last_result" => Ok(PaneHitAction::AgentScheduleTick(AgentScheduleTickPaneAction::InspectLastResult)),
            _ => unsupported(),
        },
        PaneKind::TrajectoryAudit => match action {
            "open_session" => Ok(PaneHitAction::TrajectoryAudit(TrajectoryAuditPaneAction::OpenSession)),
            "cycle_step_filter" => Ok(PaneHitAction::TrajectoryAudit(TrajectoryAuditPaneAction::CycleStepFilter)),
            "verify_trajectory_hash" => Ok(PaneHitAction::TrajectoryAudit(TrajectoryAuditPaneAction::VerifyTrajectoryHash)),
            _ => unsupported(),
        },
        PaneKind::SkillRegistry => match action {
            "discover_skills" => Ok(PaneHitAction::SkillRegistry(SkillRegistryPaneAction::DiscoverSkills)),
            "inspect_manifest" => Ok(PaneHitAction::SkillRegistry(SkillRegistryPaneAction::InspectManifest)),
            "install_selected_skill" => Ok(PaneHitAction::SkillRegistry(SkillRegistryPaneAction::InstallSelectedSkill)),
            "select_row" => Ok(PaneHitAction::SkillRegistry(SkillRegistryPaneAction::SelectRow(require_index(action)?))),
            _ => unsupported(),
        },
        PaneKind::SkillTrustRevocation => match action {
            "refresh_trust" => Ok(PaneHitAction::SkillTrustRevocation(SkillTrustRevocationPaneAction::RefreshTrust)),
            "inspect_attestations" => Ok(PaneHitAction::SkillTrustRevocation(SkillTrustRevocationPaneAction::InspectAttestations)),
            "toggle_kill_switch" => Ok(PaneHitAction::SkillTrustRevocation(SkillTrustRevocationPaneAction::ToggleKillSwitch)),
            "revoke_skill" => Ok(PaneHitAction::SkillTrustRevocation(SkillTrustRevocationPaneAction::RevokeSkill)),
            _ => unsupported(),
        },
        PaneKind::CreditDesk => match action {
            "publish_intent" => Ok(PaneHitAction::CreditDesk(CreditDeskPaneAction::PublishIntent)),
            "publish_offer" => Ok(PaneHitAction::CreditDesk(CreditDeskPaneAction::PublishOffer)),
            "publish_envelope" => Ok(PaneHitAction::CreditDesk(CreditDeskPaneAction::PublishEnvelope)),
            "authorize_spend" => Ok(PaneHitAction::CreditDesk(CreditDeskPaneAction::AuthorizeSpend)),
            _ => unsupported(),
        },
        PaneKind::CreditSettlementLedger => match action {
            "verify_settlement" => Ok(PaneHitAction::CreditSettlementLedger(CreditSettlementLedgerPaneAction::VerifySettlement)),
            "emit_default_notice" => Ok(PaneHitAction::CreditSettlementLedger(CreditSettlementLedgerPaneAction::EmitDefaultNotice)),
            "emit_reputation_label" => Ok(PaneHitAction::CreditSettlementLedger(CreditSettlementLedgerPaneAction::EmitReputationLabel)),
            _ => unsupported(),
        },
        PaneKind::AgentNetworkSimulation => match action {
            "run_round" => Ok(PaneHitAction::AgentNetworkSimulation(AgentNetworkSimulationPaneAction::RunRound)),
            "reset" => Ok(PaneHitAction::AgentNetworkSimulation(AgentNetworkSimulationPaneAction::Reset)),
            _ => unsupported(),
        },
        PaneKind::TreasuryExchangeSimulation => match action {
            "run_round" => Ok(PaneHitAction::TreasuryExchangeSimulation(TreasuryExchangeSimulationPaneAction::RunRound)),
            "reset" => Ok(PaneHitAction::TreasuryExchangeSimulation(TreasuryExchangeSimulationPaneAction::Reset)),
            _ => unsupported(),
        },
        PaneKind::RelaySecuritySimulation => match action {
            "run_round" => Ok(PaneHitAction::RelaySecuritySimulation(RelaySecuritySimulationPaneAction::RunRound)),
            "reset" => Ok(PaneHitAction::RelaySecuritySimulation(RelaySecuritySimulationPaneAction::Reset)),
            _ => unsupported(),
        },
        PaneKind::StableSatsSimulation => match action {
            "run_round" => Ok(PaneHitAction::StableSatsSimulation(StableSatsSimulationPaneAction::RunRound)),
            "reset" => Ok(PaneHitAction::StableSatsSimulation(StableSatsSimulationPaneAction::Reset)),
            _ => unsupported(),
        },
        PaneKind::CadDemo => match action {
            "bootstrap" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::BootstrapDemo)),
            "cycle_variant" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleVariant)),
            "reset_camera" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ResetCamera)),
            "toggle_projection" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleProjectionMode)),
            "cycle_hidden_line_mode" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleHiddenLineMode)),
            "cycle_section_plane" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleSectionPlane)),
            "step_section_offset" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::StepSectionPlaneOffset)),
            "cycle_material" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::CycleMaterialPreset)),
            "toggle_snap_grid" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapGrid)),
            "toggle_snap_origin" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapOrigin)),
            "toggle_snap_endpoint" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapEndpoint)),
            "toggle_snap_midpoint" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::ToggleSnapMidpoint)),
            "timeline_select_prev" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::TimelineSelectPrev)),
            "timeline_select_next" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::TimelineSelectNext)),
            "select_timeline_row" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::SelectTimelineRow(require_index(action)?))),
            "select_warning" => Ok(PaneHitAction::CadDemo(CadDemoPaneAction::SelectWarning(require_index(action)?))),
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
        state.relay_connections_inputs.relay_url.set_value(value.to_string());
        return true;
    }
    false
}

fn apply_network_requests_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "request_type" => state.network_requests_inputs.request_type.set_value(value.to_string()),
        "payload" => state.network_requests_inputs.payload.set_value(value.to_string()),
        "skill_scope_id" => state.network_requests_inputs.skill_scope_id.set_value(value.to_string()),
        "credit_envelope_ref" => state.network_requests_inputs.credit_envelope_ref.set_value(value.to_string()),
        "budget_sats" => state.network_requests_inputs.budget_sats.set_value(value.to_string()),
        "timeout_seconds" => state.network_requests_inputs.timeout_seconds.set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_settings_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "relay_url" => state.settings_inputs.relay_url.set_value(value.to_string()),
        "wallet_default_send_sats" => state.settings_inputs.wallet_default_send_sats.set_value(value.to_string()),
        "provider_max_queue_depth" => state.settings_inputs.provider_max_queue_depth.set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_credentials_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "variable_name" => state.credentials_inputs.variable_name.set_value(value.to_string()),
        "variable_value" => state.credentials_inputs.variable_value.set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_job_history_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    if field == "search_job_id" {
        state.job_history_inputs.search_job_id.set_value(value.to_string());
        return true;
    }
    false
}

fn apply_spark_wallet_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "invoice_amount" => state.spark_inputs.invoice_amount.set_value(value.to_string()),
        "send_request" => state.spark_inputs.send_request.set_value(value.to_string()),
        "send_amount" => state.spark_inputs.send_amount.set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_create_invoice_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "amount_sats" => state.create_invoice_inputs.amount_sats.set_value(value.to_string()),
        "description" => state.create_invoice_inputs.description.set_value(value.to_string()),
        "expiry_seconds" => state.create_invoice_inputs.expiry_seconds.set_value(value.to_string()),
        _ => return false,
    }
    true
}

fn apply_pay_invoice_input(state: &mut RenderState, field: &str, value: &str) -> bool {
    match field {
        "payment_request" => state.pay_invoice_inputs.payment_request.set_value(value.to_string()),
        "amount_sats" => state.pay_invoice_inputs.amount_sats.set_value(value.to_string()),
        _ => return false,
    }
    true
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

    let prompt = if let Some(intent_json) = args.intent_json.as_ref() {
        match serde_json::to_string(intent_json) {
            Ok(value) => value,
            Err(error) => {
                return ToolBridgeResultEnvelope::error(
                    "OA-CAD-INTENT-JSON-SERIALIZE-FAILED",
                    format!("Failed to serialize intent_json: {error}"),
                    json!({ "thread_id": thread_id }),
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
            "Provide either non-empty `prompt` or `intent_json` for openagents.cad.intent",
            json!({
                "thread_id": thread_id,
            }),
        );
    }

    let _ = PaneController::create_for_kind(state, PaneKind::CadDemo);
    let changed = super::reducers::apply_chat_prompt_to_cad_session(state, &thread_id, &prompt);
    if !changed {
        return ToolBridgeResultEnvelope::error(
            "OA-CAD-INTENT-NO-CHANGE",
            "CAD intent prompt did not produce a CAD mutation",
            json!({
                "thread_id": thread_id,
                "prompt": prompt,
                "last_error": state.cad_demo.last_error,
            }),
        );
    }

    ToolBridgeResultEnvelope::ok(
        "OA-CAD-INTENT-OK",
        "Applied CAD intent prompt through CAD chat adapter",
        json!({
            "thread_id": thread_id,
            "session_id": state.cad_demo.session_id,
            "document_revision": state.cad_demo.document_revision,
            "active_variant_id": state.cad_demo.active_variant_id,
            "last_action": state.cad_demo.last_action,
            "last_error": state.cad_demo.last_error,
        }),
    )
}

fn execute_cad_action(state: &mut RenderState, args: &CadActionArgs) -> ToolBridgeResultEnvelope {
    let action_key = normalize_key(&args.action);
    let action = match cad_action_from_key(action_key.as_str(), args.index) {
        Ok(value) => value,
        Err(error) => return error,
    };

    let _ = PaneController::create_for_kind(state, PaneKind::CadDemo);
    let changed = super::reducers::run_cad_demo_action(state, action);
    ToolBridgeResultEnvelope::ok(
        "OA-CAD-ACTION-OK",
        format!("Executed CAD action '{}'", args.action),
        json!({
            "action": args.action,
            "index": args.index,
            "changed": changed,
            "session_id": state.cad_demo.session_id,
            "document_revision": state.cad_demo.document_revision,
            "active_variant_id": state.cad_demo.active_variant_id,
            "last_action": state.cad_demo.last_action,
            "last_error": state.cad_demo.last_error,
        }),
    )
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
                || command_key.as_ref().is_some_and(|value| *value == normalized)
                || pane_aliases(spec.kind).iter().any(|alias| *alias == normalized)
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
        ToolBridgeResultEnvelope, cad_action_from_key, decode_tool_call_request, normalize_key,
        pane_action_to_hit_action, pane_kind_key, resolve_pane_kind,
    };
    use crate::app_state::AutopilotToolCallRequest;
    use crate::app_state::PaneKind;
    use crate::pane_system::{
        CadDemoPaneAction, PaneHitAction, RelayConnectionsPaneAction, SettingsPaneAction,
    };
    use crate::spark_pane::SparkPaneAction;
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
            "openagents.pane.open",
            r#"{"pane":"Spark Wallet"}"#,
        ))
        .expect("decode should succeed");
        assert_eq!(decoded.tool, "openagents.pane.open");

        let pane_args: PaneArgs = decoded.decode_arguments().expect("pane args decode");
        assert_eq!(pane_args.pane, "Spark Wallet");
    }

    #[test]
    fn decode_rejects_unsupported_tool_name() {
        let code = assert_error(decode_tool_call_request(&request("openagents.not_real", "{}")));
        assert_eq!(code, "OA-TOOL-UNSUPPORTED");
    }

    #[test]
    fn decode_rejects_malformed_json_arguments() {
        let code = assert_error(decode_tool_call_request(&request("openagents.pane.open", "{")));
        assert_eq!(code, "OA-TOOL-ARGS-INVALID-JSON");
    }

    #[test]
    fn decode_rejects_non_object_arguments() {
        let code = assert_error(decode_tool_call_request(&request("openagents.pane.open", "[]")));
        assert_eq!(code, "OA-TOOL-ARGS-NOT-OBJECT");
    }

    #[test]
    fn decode_arguments_reports_missing_required_field() {
        let decoded =
            decode_tool_call_request(&request("openagents.pane.open", r#"{"wrong":"field"}"#))
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
        assert_eq!(normalize_key("Spark Lightning Wallet"), "spark_lightning_wallet");
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
            cad_action_from_key("select_warning", Some(3)).expect("indexed action"),
            CadDemoPaneAction::SelectWarning(3)
        );
    }

    #[test]
    fn cad_action_mapping_requires_index_when_needed() {
        let err = cad_action_from_key("select_warning", None)
            .expect_err("missing index should fail");
        assert_eq!(err.code, "OA-CAD-ACTION-MISSING-INDEX");
    }
}
