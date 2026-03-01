# Codex Tooling Plan: CAD + Full Pane Manipulation

## Objective

Enable Codex to reliably operate OpenAgents desktop UI state through structured tool calls so a user can ask for a workflow and the agent can complete it end-to-end.

Required outcome:

- CAD is controllable via Codex using structured intents/actions.
- Every pane can be opened/focused/closed and have at least one meaningful action path reachable through Codex tool calls.
- Tool-call handling is automatic (no manual approve button required for OpenAgents-owned tools).
- Behavior is documented and testable.

## Current Baseline (Repo Reality)

- Codex app-server requests are already surfaced via `item/tool/call` notifications.
- Desktop currently queues those requests and only sends a placeholder response when user clicks `Respond Tool Call`.
- Pane actions already exist as typed Rust enums in `pane_system.rs` and typed action reducers in `input/actions.rs` and `input/reducers/*`.
- CAD already supports:
  - typed `CadIntent` parsing/dispatch
  - rich `CadDemoPaneAction` controls
  - chat phrase -> CAD translation path

This means we do not need a new lane; we need a **tool-call execution bridge** on top of existing typed actions.

## Architecture

### 1) Tool Bridge Layer

Add a dedicated bridge in `apps/autopilot-desktop/src/input/tool_bridge.rs`:

- Parse Codex tool call requests (`tool` + JSON `arguments`).
- Resolve pane identifiers (`command id`, title, aliases).
- Execute:
  - pane lifecycle operations (`open`, `focus`, `close`, `list`)
  - pane actions (`refresh`, `select`, `submit`, etc.)
  - CAD operations (`intent`, `action`)
- Return deterministic structured result payloads.

### 2) Auto-Response Policy

When a Codex tool call request arrives:

- If `tool` is `openagents.*`, execute immediately and respond with `item/tool/call:respond`.
- If parse/execution fails, respond `success=false` with explicit error code/message.
- Keep manual response action as fallback for non-OpenAgents tools.

### 3) Security / Scope

- Only execute allowlisted OpenAgents tool names.
- Never shell out from this bridge.
- No implicit destructive behavior (for example, no blanket “close all panes” command in v1).

### 4) Skill Layer

Add a first-party skill (`skills/autopilot-pane-control`) that tells Codex when/how to call these tools.

This provides a reliable “skill path” while the “tool path” remains primary runtime behavior.

## Tool Contract (v1)

Canonical tool names:

- `openagents.pane.list`
- `openagents.pane.open`
- `openagents.pane.focus`
- `openagents.pane.close`
- `openagents.pane.set_input`
- `openagents.pane.action`
- `openagents.cad.intent`
- `openagents.cad.action`

All return structured JSON rendered into `DynamicToolCallResponse` content.

## Pane Coverage Requirement

For all registered pane kinds, support:

- lifecycle controls (`open/focus/close`)
- at least one action path in `openagents.pane.action`

For panes with row selection actions, support `index` argument.
For panes with text inputs, support `openagents.pane.set_input` field updates.

## Test Strategy

- Unit tests:
  - tool name parsing
  - pane identifier resolution
  - CAD action command resolution
- Integration-style reducer tests:
  - auto-respond path on `ToolCallRequested`
  - failure response payload shape
- Regression test:
  - tool request queue drains correctly and does not leave stale pending entries.

## Documentation Deliverables

- `docs/codex/CODEX_PANE_CAD_TOOLING.md` (runtime contract and examples)
- `docs/codex/CODEX_PANE_CAD_TOOLING_PLAN.md` (this plan)
- `docs/PANES.md` update to reference Codex toolability
- skill docs under `skills/autopilot-pane-control/`

## Sequenced GitHub Backlog

### Backlog 1: Add Codex desktop tool bridge skeleton + tool schema

Title: `Codex: add OpenAgents desktop tool bridge for pane/CAD calls`

Description:

- Add `input/tool_bridge.rs` with tool request parser, allowlist, and typed argument structs.
- Implement result envelope format (success, code, message, details).
- Add unit tests for parsing and invalid payload handling.

Definition of done:

- New bridge module compiles and is wired into input module.
- Unsupported tool names return deterministic error response payloads.
- Tests cover valid + invalid request decoding.

### Backlog 2: Implement pane lifecycle tools for all panes

Title: `Codex: implement pane lifecycle tools (list/open/focus/close) across all pane kinds`

Description:

- Implement `openagents.pane.list/open/focus/close`.
- Resolve panes by command id, title, enum alias, and common shorthand.
- Return open pane IDs and active pane state in response details.

Definition of done:

- Any pane in `pane_registry` can be opened/focused/closed through tool calls.
- Lifecycle tool responses include canonical pane key + title.

### Backlog 3: Implement pane action + input mutation tool coverage

Title: `Codex: add pane action dispatcher and input setter for all pane domains`

Description:

- Implement `openagents.pane.set_input` for supported pane inputs.
- Implement `openagents.pane.action` mapping to typed pane actions.
- Cover all pane domains with at least one meaningful action.

Definition of done:

- Codex can programmatically execute representative actions in every pane.
- Row/index-based panes support deterministic selection by index.

### Backlog 4: Implement CAD-specific tool endpoints

Title: `Codex: add CAD intent/action tools for deterministic CAD manipulation`

Description:

- Implement `openagents.cad.intent` (JSON intent or phrase bridge) and `openagents.cad.action` (CadDemoPaneAction).
- Ensure CAD pane auto-opens if needed.
- Return CAD revision/session/action summaries in response payload.

Definition of done:

- Codex can mutate CAD state via explicit intent/action tools.
- CAD tool responses include revision and active variant context.

### Backlog 5: Auto-execute OpenAgents tools on Codex tool-call requests

Title: `Codex: auto-handle OpenAgents tool calls and respond without manual click`

Description:

- On `ToolCallRequested`, auto-execute for `openagents.*` tools.
- Keep existing manual response path for unknown/non-OpenAgents tools.
- Emit diagnostics timeline events for success/failure.

Definition of done:

- OpenAgents tool calls complete end-to-end without user pressing `Respond Tool Call`.
- Failure path responds with `success=false` and actionable error text.

### Backlog 6: Ship skill + docs + verification runbook

Title: `Codex: document pane/CAD tooling and add autopilot-pane-control skill`

Description:

- Add skill files under `skills/autopilot-pane-control/` describing when/how to call tool contract.
- Add comprehensive docs in `docs/codex/` with schemas and examples.
- Update PANES docs with Codex toolability note.

Definition of done:

- Skill is discoverable via `skills/list` in repo root.
- Docs include copy-paste request examples for each tool.
- Verification steps documented for live harness and desktop UX.

## Execution Order

Implement strictly in backlog order 1 -> 6.
Each item must end with:

- focused tests run
- one commit pushed to `main`
- issue comment summarizing what shipped
- issue closed
