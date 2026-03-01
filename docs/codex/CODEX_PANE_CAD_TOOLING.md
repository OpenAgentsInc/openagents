# Codex Pane + CAD Tooling Contract

## Purpose

This document defines how Codex can control OpenAgents desktop panes and CAD state through structured tool calls.

The runtime bridge lives in:

- `apps/autopilot-desktop/src/input/tool_bridge.rs`

The bridge executes tool calls received through Codex app-server `item/tool/call` server requests.

## Auto-Execution Policy

- Tools in `openagents.*` namespace are auto-executed when requested by Codex.
- Non-`openagents.*` tool calls are left in pending queue for manual response.
- All responses are sent via `item/tool/call:respond` with structured JSON payload in `content_items[0].text`.

## Response Envelope

Every OpenAgents tool response serializes this shape:

```json
{
  "success": true,
  "code": "OA-PANE-ACTION-OK",
  "message": "Executed action 'refresh' on pane 'codex_models'",
  "details": {}
}
```

Common failure codes:

- `OA-TOOL-UNSUPPORTED`
- `OA-TOOL-ARGS-INVALID-JSON`
- `OA-TOOL-ARGS-NOT-OBJECT`
- `OA-TOOL-ARGS-INVALID-SHAPE`
- `OA-PANE-ACTION-UNSUPPORTED`
- `OA-PANE-ACTION-MISSING-INDEX`
- `OA-PANE-INPUT-UNSUPPORTED`
- `OA-CAD-ACTION-UNSUPPORTED`
- `OA-CAD-ACTION-MISSING-INDEX`
- `OA-CAD-INTENT-MISSING-PAYLOAD`

## Tool Surface

## `openagents.pane.list`

Args:

```json
{}
```

Returns:

- registered pane catalog
- open pane instances
- active pane

## `openagents.pane.open`

Args:

```json
{ "pane": "pane.wallet" }
```

`pane` may be:

- command id (`pane.wallet`)
- pane title (`Spark Lightning Wallet`)
- key/alias (`spark_wallet`, `wallet`)

## `openagents.pane.focus`

Args:

```json
{ "pane": "cad" }
```

Focuses top-most open pane for resolved kind.

## `openagents.pane.close`

Args:

```json
{ "pane": "autopilot_chat" }
```

Closes top-most open instance for resolved kind.

## `openagents.pane.set_input`

Args:

```json
{
  "pane": "network_requests",
  "field": "payload",
  "value": "{\"task\":\"summarize\"}"
}
```

Supported writable input groups:

- chat composer
- relay connections URL
- network requests form fields
- settings form fields
- credentials form fields
- wallet / create-invoice / pay-invoice fields
- job history search field

## `openagents.pane.action`

Args:

```json
{
  "pane": "relay_connections",
  "action": "select_row",
  "index": 0
}
```

Notes:

- `index` is required for row-selection actions.
- `action: "snapshot"` (or `"status"`) returns pane state summary without mutation.
- Dispatcher routes into existing typed pane actions (`PaneHitAction`).

Representative actions:

- Chat: `send`, `new_thread`, `refresh_threads`, `select_thread`
- Codex panes: `refresh`, `toggle_hidden`, `read`, `requirements`, etc.
- Runtime panes: `submit_request`, `rebootstrap`, `recover_selected`, etc.
- Wallet panes: `refresh`, `create_invoice`, `send_payment`
- Sim panes: `run_round`, `reset`

## `openagents.cad.intent`

Args:

```json
{
  "thread_id": "optional-thread-id",
  "intent_json": {
    "intent": "SetMaterial",
    "material_id": "al-6061-t6"
  }
}
```

or:

```json
{
  "prompt": "Make vent holes 20% larger"
}
```

Behavior:

- Ensures CAD pane is open.
- Routes through existing CAD chat-intent adapter path.
- Returns CAD session/revision/variant summary.

## `openagents.cad.action`

Args:

```json
{
  "action": "select_warning",
  "index": 1
}
```

Representative actions:

- `bootstrap`
- `cycle_variant`
- `reset_camera`
- `toggle_projection`
- `cycle_section_plane`
- `step_section_offset`
- `cycle_material`
- `snap_top` / `snap_front` / `snap_right` / `snap_isometric`
- `cycle_hidden_line_mode`
- `select_warning` / `select_warning_marker` / `select_timeline_row`

## Workflow Examples

## Example 1: Open wallet and create invoice

1. `openagents.pane.open {"pane":"wallet"}`
2. `openagents.pane.set_input {"pane":"wallet","field":"invoice_amount","value":"2500"}`
3. `openagents.pane.action {"pane":"wallet","action":"create_invoice"}`

## Example 2: CAD mutation via intent

1. `openagents.pane.open {"pane":"cad"}`
2. `openagents.cad.intent {"prompt":"Set material al-6061-t6"}`
3. `openagents.cad.action {"action":"cycle_hidden_line_mode"}`

## Example 3: Alerts recovery triage

1. `openagents.pane.open {"pane":"alerts_recovery"}`
2. `openagents.pane.action {"pane":"alerts_recovery","action":"select_row","index":0}`
3. `openagents.pane.action {"pane":"alerts_recovery","action":"recover_selected"}`

## Troubleshooting

- If auto execution does not happen, inspect Codex Diagnostics pane for `tool call auto-response` timeline events.
- If a request remains pending, use manual `Respond Tool Call` to retry queueing.
- For `OA-PANE-ACTION-UNSUPPORTED`, call `snapshot` first to validate pane resolution and state.
- For CAD no-op errors, provide strict `intent_json` rather than phrase prompts.
