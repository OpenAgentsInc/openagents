use codex_client::{DynamicToolCallOutputContentItem, DynamicToolCallResponse};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app_state::{AutopilotToolCallRequest, PaneKind, RenderState};
use crate::pane_registry::{pane_spec, pane_spec_by_command_id, pane_specs};
use crate::pane_system::PaneController;

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
        ToolBridgeResultEnvelope, decode_tool_call_request, normalize_key, pane_kind_key,
        resolve_pane_kind,
    };
    use crate::app_state::AutopilotToolCallRequest;
    use crate::app_state::PaneKind;
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
}
