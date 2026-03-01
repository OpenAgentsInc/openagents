---
name: autopilot-pane-control
description: OpenAgents desktop pane and CAD control for Codex via openagents.* tool calls.
metadata:
  oa:
    project: openagents
    identifier: autopilot-pane-control
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - codex:tool-call
      - desktop:pane-control
      - cad:intent-control
---

# Autopilot Pane Control

Use this skill when the user asks for operations that require desktop pane manipulation and/or CAD state changes in OpenAgents.

## When To Use

- Open/focus/close panes to prepare UI state.
- Fill pane inputs and trigger pane actions.
- Apply CAD intents/actions.

## Tool Contract

Use only these tools:

- `openagents.pane.list`
- `openagents.pane.open`
- `openagents.pane.focus`
- `openagents.pane.close`
- `openagents.pane.set_input`
- `openagents.pane.action`
- `openagents.cad.intent`
- `openagents.cad.action`

Detailed schemas and examples live in:

- `docs/codex/CODEX_PANE_CAD_TOOLING.md`
- `references/tool-cheatsheet.md`

## Operating Rules

1. Start with `openagents.pane.list` if pane state is unknown.
2. Open/focus required pane before setting inputs.
3. Use deterministic action names and provide `index` when selecting rows.
4. For CAD edits, prefer structured `intent_json` over ambiguous prompt text.
5. After mutating state, read back via `openagents.pane.action` with `snapshot` to confirm.

## Minimal Sequences

### Wallet invoice sequence

1. `openagents.pane.open` for wallet
2. `openagents.pane.set_input` -> `invoice_amount`
3. `openagents.pane.action` -> `create_invoice`

### CAD sequence

1. `openagents.pane.open` for CAD
2. `openagents.cad.intent` with `intent_json`
3. `openagents.cad.action` for view/render/timeline ops
