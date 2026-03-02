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
