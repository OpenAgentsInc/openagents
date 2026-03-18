use codex_client::DynamicToolSpec;
use serde_json::json;

pub(crate) const OPENAGENTS_TOOL_PANE_LIST: &str = "openagents_pane_list";
pub(crate) const OPENAGENTS_TOOL_PANE_OPEN: &str = "openagents_pane_open";
pub(crate) const OPENAGENTS_TOOL_PANE_FOCUS: &str = "openagents_pane_focus";
pub(crate) const OPENAGENTS_TOOL_PANE_CLOSE: &str = "openagents_pane_close";
pub(crate) const OPENAGENTS_TOOL_PANE_SET_INPUT: &str = "openagents_pane_set_input";
pub(crate) const OPENAGENTS_TOOL_PANE_ACTION: &str = "openagents_pane_action";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_SELLER_STATUS: &str =
    "openagents_data_market_seller_status";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_DRAFT_ASSET: &str =
    "openagents_data_market_draft_asset";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_PREVIEW_ASSET: &str =
    "openagents_data_market_preview_asset";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_ASSET: &str =
    "openagents_data_market_publish_asset";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_DRAFT_GRANT: &str =
    "openagents_data_market_draft_grant";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_PREVIEW_GRANT: &str =
    "openagents_data_market_preview_grant";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_GRANT: &str =
    "openagents_data_market_publish_grant";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_REQUEST_PAYMENT: &str =
    "openagents_data_market_request_payment";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_PREPARE_DELIVERY: &str =
    "openagents_data_market_prepare_delivery";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_ISSUE_DELIVERY: &str =
    "openagents_data_market_issue_delivery";
pub(crate) const OPENAGENTS_TOOL_DATA_MARKET_SNAPSHOT: &str = "openagents_data_market_snapshot";
pub(crate) const OPENAGENTS_TOOL_CAD_INTENT: &str = "openagents_cad_intent";
pub(crate) const OPENAGENTS_TOOL_CAD_ACTION: &str = "openagents_cad_action";
pub(crate) const OPENAGENTS_TOOL_SWAP_QUOTE: &str = "openagents_swap_quote";
pub(crate) const OPENAGENTS_TOOL_SWAP_EXECUTE: &str = "openagents_swap_execute";
pub(crate) const OPENAGENTS_TOOL_TREASURY_TRANSFER: &str = "openagents_treasury_transfer";
pub(crate) const OPENAGENTS_TOOL_TREASURY_CONVERT: &str = "openagents_treasury_convert";
pub(crate) const OPENAGENTS_TOOL_TREASURY_RECEIPT: &str = "openagents_treasury_receipt";
pub(crate) const OPENAGENTS_TOOL_GOAL_SCHEDULER: &str = "openagents_goal_scheduler";
pub(crate) const OPENAGENTS_TOOL_WALLET_CHECK: &str = "openagents_wallet_check";
pub(crate) const OPENAGENTS_TOOL_PROVIDER_CONTROL: &str = "openagents_provider_control";
pub(crate) const OPENAGENTS_TOOL_LABOR_SCOPE: &str = "openagents_labor_scope";
pub(crate) const OPENAGENTS_TOOL_LABOR_REQUIREMENTS: &str = "openagents_labor_requirements";
pub(crate) const OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST: &str = "openagents_labor_evidence_list";
pub(crate) const OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH: &str = "openagents_labor_evidence_attach";
pub(crate) const OPENAGENTS_TOOL_LABOR_SUBMISSION_READY: &str = "openagents_labor_submission_ready";
pub(crate) const OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST: &str = "openagents_labor_verifier_request";
pub(crate) const OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH: &str = "openagents_labor_incident_attach";
pub(crate) const OPENAGENTS_TOOL_LABOR_CLAIM_STATUS: &str = "openagents_labor_claim_status";
pub(crate) const OPENAGENTS_TOOL_LABOR_CLAIM_OPEN: &str = "openagents_labor_claim_open";
pub(crate) const OPENAGENTS_TOOL_LABOR_CLAIM_REVIEW: &str = "openagents_labor_claim_review";
pub(crate) const OPENAGENTS_TOOL_LABOR_CLAIM_REMEDY: &str = "openagents_labor_claim_remedy";
pub(crate) const OPENAGENTS_TOOL_LABOR_CLAIM_DENY: &str = "openagents_labor_claim_deny";
pub(crate) const OPENAGENTS_TOOL_LABOR_CLAIM_RESOLVE: &str = "openagents_labor_claim_resolve";

pub(crate) const OPENAGENTS_DYNAMIC_TOOL_NAMES: &[&str] = &[
    OPENAGENTS_TOOL_PANE_LIST,
    OPENAGENTS_TOOL_PANE_OPEN,
    OPENAGENTS_TOOL_PANE_FOCUS,
    OPENAGENTS_TOOL_PANE_CLOSE,
    OPENAGENTS_TOOL_PANE_SET_INPUT,
    OPENAGENTS_TOOL_PANE_ACTION,
    OPENAGENTS_TOOL_DATA_MARKET_SELLER_STATUS,
    OPENAGENTS_TOOL_DATA_MARKET_DRAFT_ASSET,
    OPENAGENTS_TOOL_DATA_MARKET_PREVIEW_ASSET,
    OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_ASSET,
    OPENAGENTS_TOOL_DATA_MARKET_DRAFT_GRANT,
    OPENAGENTS_TOOL_DATA_MARKET_PREVIEW_GRANT,
    OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_GRANT,
    OPENAGENTS_TOOL_DATA_MARKET_REQUEST_PAYMENT,
    OPENAGENTS_TOOL_DATA_MARKET_PREPARE_DELIVERY,
    OPENAGENTS_TOOL_DATA_MARKET_ISSUE_DELIVERY,
    OPENAGENTS_TOOL_DATA_MARKET_SNAPSHOT,
    OPENAGENTS_TOOL_CAD_INTENT,
    OPENAGENTS_TOOL_CAD_ACTION,
    OPENAGENTS_TOOL_SWAP_QUOTE,
    OPENAGENTS_TOOL_SWAP_EXECUTE,
    OPENAGENTS_TOOL_TREASURY_TRANSFER,
    OPENAGENTS_TOOL_TREASURY_CONVERT,
    OPENAGENTS_TOOL_TREASURY_RECEIPT,
    OPENAGENTS_TOOL_GOAL_SCHEDULER,
    OPENAGENTS_TOOL_WALLET_CHECK,
    OPENAGENTS_TOOL_PROVIDER_CONTROL,
    OPENAGENTS_TOOL_LABOR_SCOPE,
    OPENAGENTS_TOOL_LABOR_REQUIREMENTS,
    OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST,
    OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH,
    OPENAGENTS_TOOL_LABOR_SUBMISSION_READY,
    OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST,
    OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH,
    OPENAGENTS_TOOL_LABOR_CLAIM_STATUS,
    OPENAGENTS_TOOL_LABOR_CLAIM_OPEN,
    OPENAGENTS_TOOL_LABOR_CLAIM_REVIEW,
    OPENAGENTS_TOOL_LABOR_CLAIM_REMEDY,
    OPENAGENTS_TOOL_LABOR_CLAIM_DENY,
    OPENAGENTS_TOOL_LABOR_CLAIM_RESOLVE,
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
            name: OPENAGENTS_TOOL_DATA_MARKET_SELLER_STATUS.to_string(),
            description: "Return the current Data Seller pane status, draft posture, and readiness blockers."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_DRAFT_ASSET.to_string(),
            description:
                "Update structured Data Seller draft asset fields without publishing authority state."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "asset_kind": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "content_digest": { "type": "string" },
                    "provenance_ref": { "type": "string" },
                    "default_policy": { "type": "string" },
                    "price_hint_sats": { "type": "integer", "minimum": 0 },
                    "delivery_modes": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "visibility_posture": {
                        "type": "string",
                        "enum": ["targeted_only", "operator_only", "public_catalog"]
                    },
                    "sensitivity_posture": {
                        "type": "string",
                        "enum": ["private", "restricted", "public"]
                    },
                    "metadata": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_PREVIEW_ASSET.to_string(),
            description: "Produce the current Data Seller asset preview payload and readiness state."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_ASSET.to_string(),
            description:
                "Attempt the Data Seller asset publish path after explicit confirmation."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "confirm": { "type": "boolean" }
                },
                "required": ["confirm"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_DRAFT_GRANT.to_string(),
            description:
                "Update the default grant posture derived by the Data Seller draft without publishing."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "default_policy": { "type": "string" },
                    "policy_template": {
                        "type": "string",
                        "enum": ["targeted_request", "evaluation_window", "licensed_bundle"]
                    },
                    "consumer_id": { "type": "string" },
                    "price_hint_sats": { "type": "integer", "minimum": 0 },
                    "delivery_modes": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "visibility_posture": {
                        "type": "string",
                        "enum": ["targeted_only", "operator_only", "public_catalog"]
                    },
                    "expires_in_hours": { "type": "integer", "minimum": 1 },
                    "warranty_window_hours": { "type": "integer", "minimum": 0 },
                    "metadata": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_PREVIEW_GRANT.to_string(),
            description:
                "Produce the current derived grant preview payload and readiness posture."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_GRANT.to_string(),
            description:
                "Attempt the Data Seller grant publish path after explicit confirmation."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "confirm": { "type": "boolean" }
                },
                "required": ["confirm"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_REQUEST_PAYMENT.to_string(),
            description:
                "Generate a seller invoice and publish payment-required feedback for a targeted data-access request."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "request_id": { "type": "string" }
                },
                "required": ["request_id"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_PREPARE_DELIVERY.to_string(),
            description:
                "Update the local seller delivery draft for a paid targeted data-access request."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "request_id": { "type": "string" },
                    "preview_text": { "type": "string" },
                    "delivery_ref": { "type": "string" },
                    "delivery_digest": { "type": "string" },
                    "manifest_refs": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "bundle_size_bytes": { "type": "integer", "minimum": 0 },
                    "expires_in_hours": { "type": "integer", "minimum": 1 }
                },
                "required": ["request_id"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_ISSUE_DELIVERY.to_string(),
            description:
                "Accept the matched grant if needed, issue the DeliveryBundle, and publish the linked NIP-90 result."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "request_id": { "type": "string" }
                },
                "required": ["request_id"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_DATA_MARKET_SNAPSHOT.to_string(),
            description:
                "Return a compact Data Market snapshot covering seller draft state and read-only market counts."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
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
                    "quote_ttl_seconds": { "type": "integer", "minimum": 1 }
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
                    "memo": { "type": "string" }
                },
                "required": ["goal_id", "quote_id"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_TREASURY_TRANSFER.to_string(),
            description: "Queue a real Blink BTC/USD wallet-to-wallet transfer across StableSats topology."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from_owner_id": { "type": "string" },
                    "to_owner_id": { "type": "string" },
                    "asset": { "type": "string", "enum": ["btc_sats", "usd_cents"] },
                    "amount": { "type": "integer", "minimum": 1 },
                    "memo": { "type": "string" }
                },
                "required": ["from_owner_id", "to_owner_id", "asset", "amount"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_TREASURY_CONVERT.to_string(),
            description: "Queue a real Blink BTC<->USD conversion for a specific wallet owner."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "owner_id": { "type": "string" },
                    "direction": { "type": "string", "enum": ["btc-to-usd", "usd-to-btc"] },
                    "amount": { "type": "integer", "minimum": 1 },
                    "unit": { "type": "string", "enum": ["sats", "cents"] },
                    "memo": { "type": "string" }
                },
                "required": ["owner_id", "direction", "amount", "unit"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_TREASURY_RECEIPT.to_string(),
            description: "Fetch machine-parseable treasury receipt/status for an async worker request."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "worker_request_id": { "type": "integer", "minimum": 1 }
                },
                "required": ["worker_request_id"],
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
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_SCOPE.to_string(),
            description: "Fetch the active labor contract scope, ids, and safe artifact boundaries for the current turn."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_REQUIREMENTS.to_string(),
            description: "Inspect labor acceptance criteria, output requirements, and current evidence gaps."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_EVIDENCE_LIST.to_string(),
            description: "List current labor evidence, submission state, verdict state, and unresolved gaps."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_EVIDENCE_ATTACH.to_string(),
            description: "Attach an in-scope evidence reference to the active labor contract."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "uri": { "type": "string" },
                    "digest": { "type": "string" }
                },
                "required": ["kind", "uri", "digest"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_SUBMISSION_READY.to_string(),
            description: "Mark the active labor submission ready once required evidence is present."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_VERIFIER_REQUEST.to_string(),
            description: "Request local verifier execution for the active labor submission."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_INCIDENT_ATTACH.to_string(),
            description: "Attach incident or dispute evidence to the active labor contract."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "kind": { "type": "string" },
                    "uri": { "type": "string" },
                    "digest": { "type": "string" }
                },
                "required": ["kind", "uri", "digest"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_CLAIM_STATUS.to_string(),
            description: "Inspect the active labor claim, dispute, and remedy state."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_CLAIM_OPEN.to_string(),
            description: "Open a claim against the active labor contract when settlement is disputed."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "reason_code": { "type": "string" },
                    "note": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_CLAIM_REVIEW.to_string(),
            description: "Move the active labor claim into under-review state."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "note": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_CLAIM_REMEDY.to_string(),
            description: "Issue a remedy proposal for the active labor claim."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "outcome": { "type": "string" },
                    "note": { "type": "string" }
                },
                "required": ["outcome"],
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_CLAIM_DENY.to_string(),
            description: "Deny the active labor claim with an explicit reason code."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "reason_code": { "type": "string" },
                    "note": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        DynamicToolSpec {
            name: OPENAGENTS_TOOL_LABOR_CLAIM_RESOLVE.to_string(),
            description: "Resolve the active labor claim after review or remedy issuance."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "work_unit_id": { "type": "string" },
                    "contract_id": { "type": "string" },
                    "note": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        OPENAGENTS_DYNAMIC_TOOL_NAMES, OPENAGENTS_TOOL_DATA_MARKET_DRAFT_ASSET,
        OPENAGENTS_TOOL_DATA_MARKET_DRAFT_GRANT, OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_ASSET,
        OPENAGENTS_TOOL_DATA_MARKET_PREPARE_DELIVERY,
        OPENAGENTS_TOOL_DATA_MARKET_REQUEST_PAYMENT, OPENAGENTS_TOOL_SWAP_EXECUTE,
        OPENAGENTS_TOOL_SWAP_QUOTE,
        openagents_dynamic_tool_specs,
    };
    use serde_json::json;
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

    #[test]
    fn swap_tool_schemas_do_not_allow_injected_quote_or_status_fields() {
        let specs = openagents_dynamic_tool_specs();
        let quote_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_SWAP_QUOTE)
            .expect("swap quote spec should exist");
        let execute_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_SWAP_EXECUTE)
            .expect("swap execute spec should exist");

        assert!(
            quote_spec
                .input_schema
                .pointer("/properties/stablesats_quote")
                .is_none()
        );
        assert!(
            quote_spec
                .input_schema
                .pointer("/properties/stablesats_error")
                .is_none()
        );
        assert!(
            quote_spec
                .input_schema
                .pointer("/properties/fallback_quote_id")
                .is_none()
        );
        assert!(
            execute_spec
                .input_schema
                .pointer("/properties/status")
                .is_none()
        );
    }

    #[test]
    fn data_market_tool_schemas_expose_expected_fields() {
        let specs = openagents_dynamic_tool_specs();
        let draft_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_DATA_MARKET_DRAFT_ASSET)
            .expect("data market draft asset spec should exist");
        let publish_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_DATA_MARKET_PUBLISH_ASSET)
            .expect("data market publish asset spec should exist");
        let grant_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_DATA_MARKET_DRAFT_GRANT)
            .expect("data market draft grant spec should exist");
        let request_payment_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_DATA_MARKET_REQUEST_PAYMENT)
            .expect("data market request payment spec should exist");
        let prepare_delivery_spec = specs
            .iter()
            .find(|spec| spec.name == OPENAGENTS_TOOL_DATA_MARKET_PREPARE_DELIVERY)
            .expect("data market prepare delivery spec should exist");

        assert!(
            draft_spec
                .input_schema
                .pointer("/properties/title")
                .is_some()
        );
        assert!(
            draft_spec
                .input_schema
                .pointer("/properties/metadata")
                .is_some()
        );
        assert!(
            grant_spec
                .input_schema
                .pointer("/properties/policy_template")
                .is_some()
        );
        assert_eq!(
            publish_spec.input_schema.pointer("/required/0"),
            Some(&json!("confirm"))
        );
        assert_eq!(
            request_payment_spec.input_schema.pointer("/required/0"),
            Some(&json!("request_id"))
        );
        assert_eq!(
            prepare_delivery_spec.input_schema.pointer("/required/0"),
            Some(&json!("request_id"))
        );
    }
}
