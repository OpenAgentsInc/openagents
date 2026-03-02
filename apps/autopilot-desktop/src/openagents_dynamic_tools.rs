use codex_client::DynamicToolSpec;
use serde_json::json;

pub(crate) const OPENAGENTS_TOOL_PANE_LIST: &str = "openagents_pane_list";
pub(crate) const OPENAGENTS_TOOL_PANE_OPEN: &str = "openagents_pane_open";
pub(crate) const OPENAGENTS_TOOL_PANE_FOCUS: &str = "openagents_pane_focus";
pub(crate) const OPENAGENTS_TOOL_PANE_CLOSE: &str = "openagents_pane_close";
pub(crate) const OPENAGENTS_TOOL_PANE_SET_INPUT: &str = "openagents_pane_set_input";
pub(crate) const OPENAGENTS_TOOL_PANE_ACTION: &str = "openagents_pane_action";
pub(crate) const OPENAGENTS_TOOL_CAD_INTENT: &str = "openagents_cad_intent";
pub(crate) const OPENAGENTS_TOOL_CAD_ACTION: &str = "openagents_cad_action";
pub(crate) const OPENAGENTS_TOOL_SWAP_QUOTE: &str = "openagents_swap_quote";
pub(crate) const OPENAGENTS_TOOL_SWAP_EXECUTE: &str = "openagents_swap_execute";
pub(crate) const OPENAGENTS_TOOL_GOAL_SCHEDULER: &str = "openagents_goal_scheduler";
pub(crate) const OPENAGENTS_TOOL_WALLET_CHECK: &str = "openagents_wallet_check";
pub(crate) const OPENAGENTS_TOOL_PROVIDER_CONTROL: &str = "openagents_provider_control";

pub(crate) const OPENAGENTS_DYNAMIC_TOOL_NAMES: &[&str] = &[
    OPENAGENTS_TOOL_PANE_LIST,
    OPENAGENTS_TOOL_PANE_OPEN,
    OPENAGENTS_TOOL_PANE_FOCUS,
    OPENAGENTS_TOOL_PANE_CLOSE,
    OPENAGENTS_TOOL_PANE_SET_INPUT,
    OPENAGENTS_TOOL_PANE_ACTION,
    OPENAGENTS_TOOL_CAD_INTENT,
    OPENAGENTS_TOOL_CAD_ACTION,
    OPENAGENTS_TOOL_SWAP_QUOTE,
    OPENAGENTS_TOOL_SWAP_EXECUTE,
    OPENAGENTS_TOOL_GOAL_SCHEDULER,
    OPENAGENTS_TOOL_WALLET_CHECK,
    OPENAGENTS_TOOL_PROVIDER_CONTROL,
];

pub(crate) fn openagents_dynamic_tool_specs() -> Vec<DynamicToolSpec> {
    vec![
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PANE_LIST.to_string(),
            description: "List panes and active state in OpenAgents desktop.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PANE_OPEN.to_string(),
            description: "Open a pane by pane identifier.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pane": { "type": "string" }
                },
                "required": ["pane"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PANE_FOCUS.to_string(),
            description: "Focus an already-open pane by pane identifier.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pane": { "type": "string" }
                },
                "required": ["pane"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PANE_CLOSE.to_string(),
            description: "Close a pane by pane identifier.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pane": { "type": "string" }
                },
                "required": ["pane"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PANE_SET_INPUT.to_string(),
            description: "Set a pane input field value.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pane": { "type": "string" },
                    "field": { "type": "string" },
                    "value": { "type": "string" }
                },
                "required": ["pane", "field", "value"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PANE_ACTION.to_string(),
            description: "Execute a pane action, optionally with a row index.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pane": { "type": "string" },
                    "action": { "type": "string" },
                    "index": { "type": "integer", "minimum": 0 }
                },
                "required": ["pane", "action"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_CAD_INTENT.to_string(),
            description: "Apply a CAD intent via prompt or typed intent_json payload.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "thread_id": { "type": "string" },
                    "prompt": { "type": "string" },
                    "intent_json": {
                        "type": "object",
                        "additionalProperties": true
                    }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_CAD_ACTION.to_string(),
            description: "Run a deterministic CAD pane action.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string" },
                    "index": { "type": "integer", "minimum": 0 }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_SWAP_QUOTE.to_string(),
            description: "Request a controlled BTC<->USD swap quote for an autonomous goal run."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "goal_id": { "type": "string" },
                    "request_id": { "type": "string" },
                    "direction": { "type": "string", "enum": ["btc_to_usd", "usd_to_btc"] },
                    "amount": { "type": "integer", "minimum": 1 },
                    "unit": { "type": "string", "enum": ["sats", "cents"] },
                    "immediate_execution": { "type": "boolean" },
                    "quote_ttl_seconds": { "type": "integer", "minimum": 1 },
                    "fallback_quote_id": { "type": "string" },
                    "fallback_amount_out": { "type": "integer", "minimum": 1 },
                    "stablesats_error": { "type": "string" },
                    "stablesats_quote": {
                        "type": "object",
                        "properties": {
                            "quote_id": { "type": "string" },
                            "amount_to_sell_in_sats": { "type": "integer", "minimum": 1 },
                            "amount_to_buy_in_cents": { "type": "integer", "minimum": 1 },
                            "amount_to_buy_in_sats": { "type": "integer", "minimum": 1 },
                            "amount_to_sell_in_cents": { "type": "integer", "minimum": 1 },
                            "expires_at_epoch_seconds": { "type": "integer", "minimum": 1 },
                            "executed": { "type": "boolean" }
                        },
                        "required": ["quote_id", "expires_at_epoch_seconds", "executed"],
                        "additionalProperties": false
                    }
                },
                "required": ["goal_id", "request_id", "direction", "amount", "unit"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_SWAP_EXECUTE.to_string(),
            description:
                "Record controlled swap settlement/failure for a quoted autonomous goal swap."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "goal_id": { "type": "string" },
                    "quote_id": { "type": "string" },
                    "status": { "type": "string", "enum": ["SUCCESS", "FAILURE", "PENDING", "ALREADY_PAID"] },
                    "transaction_id": { "type": "string" },
                    "failure_reason": { "type": "string" }
                },
                "required": ["goal_id", "quote_id", "status"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_GOAL_SCHEDULER.to_string(),
            description:
                "Run allowlisted goal scheduler operations (status, recovery, run-now, policy, rollout, OS adapter reconcile)."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [
                            "status",
                            "recover_startup",
                            "run_now",
                            "set_missed_policy",
                            "set_kill_switch",
                            "set_rollout",
                            "toggle_os_adapter",
                            "reconcile_os_adapters"
                        ]
                    },
                    "goal_id": { "type": "string" },
                    "missed_run_policy": {
                        "type": "string",
                        "enum": ["catch_up", "skip", "single_replay"]
                    },
                    "kill_switch_active": { "type": "boolean" },
                    "kill_switch_reason": { "type": "string" },
                    "rollout_enabled": { "type": "boolean" },
                    "rollout_stage": {
                        "type": "string",
                        "enum": [
                            "disabled",
                            "internal_dogfood",
                            "canary",
                            "general_availability"
                        ]
                    },
                    "rollout_cohorts": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "max_false_success_rate_bps": { "type": "integer", "minimum": 0, "maximum": 10000 },
                    "max_abort_rate_bps": { "type": "integer", "minimum": 0, "maximum": 10000 },
                    "max_error_rate_bps": { "type": "integer", "minimum": 0, "maximum": 10000 },
                    "max_avg_payout_confirm_latency_seconds": { "type": "integer", "minimum": 1 },
                    "hardening_authoritative_payout_gate_validated": { "type": "boolean" },
                    "hardening_scheduler_recovery_drills_validated": { "type": "boolean" },
                    "hardening_swap_risk_alerting_validated": { "type": "boolean" },
                    "hardening_incident_runbook_validated": { "type": "boolean" },
                    "hardening_test_matrix_gate_green": { "type": "boolean" }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_WALLET_CHECK.to_string(),
            description:
                "Read-only wallet status check with optional bounded recent payment summary."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "window_seconds": { "type": "integer", "minimum": 60 },
                    "include_payments": { "type": "boolean" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_PROVIDER_CONTROL.to_string(),
            description:
                "Controlled provider runtime actions (status, set online/offline, queue wallet refresh)."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["status", "set_online", "set_offline", "refresh_wallet"]
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{OPENAGENTS_DYNAMIC_TOOL_NAMES, openagents_dynamic_tool_specs};
    use std::collections::HashSet;

    fn matches_server_name_pattern(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    }

    #[test]
    fn specs_have_unique_non_empty_names() {
        let specs = openagents_dynamic_tool_specs();
        let mut seen = HashSet::new();
        for spec in specs {
            assert!(!spec.name.trim().is_empty());
            assert_eq!(spec.name.trim(), spec.name);
            assert!(seen.insert(spec.name));
        }
    }

    #[test]
    fn specs_names_match_server_string_pattern() {
        for name in OPENAGENTS_DYNAMIC_TOOL_NAMES {
            assert!(
                matches_server_name_pattern(name),
                "tool name should match server pattern: {name}"
            );
        }
    }
}
