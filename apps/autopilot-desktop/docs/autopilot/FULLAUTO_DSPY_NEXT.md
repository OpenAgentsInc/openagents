# Autopilot Desktop Full Auto + DSPy: Next Implementation Steps

- Status: Draft
- Source of truth: code + `docs/FULLAUTO.md`
- If docs conflict with code, code wins. If terms conflict, `GLOSSARY.md` wins.

## Purpose

Translate the Full Auto design in `docs/FULLAUTO.md` into concrete, Autopilot
Desktop work items. This is focused on what must happen next to make DSPy
(DSRS) drive the Full Auto loop in the desktop app.

## Current Desktop Reality (constraints to respect)

- Autopilot Desktop already runs the **Codex app-server** process and streams
  its events to the frontend (`app-server-event`).
- Full Auto today is tied to app-server `turn/completed` and auto-continues with
  a fixed prompt.
- **Adjutant** is DSPy-native and emits UI events (UITree + patches), but Full
  Auto must be codex app-server only for now.

Implication: Full Auto needs an event source that can drive turn-level
summaries. We must enrich and consume app-server `turn/*` signals directly.

## Decision to lock first

1. **Event source for Full Auto (codex app-server only)**
   - Add a new app-server connection and map `turn/completed`,
     `turn/plan/updated`, `turn/diff/updated`, and approval/tool-input requests
     into the backend event stream used by Full Auto.
   - Full Auto uses app-server signals as the authoritative turn boundary.

2. **Decision signature placement**
   - Add `FullAutoDecisionSignature` to `crates/dsrs/src/signatures/` (not
     app-local duplicates). Expose via `crates/dsrs/src/signature_registry.rs`.

3. **Full Auto UI event contract**
   - Decide whether Full Auto status/decisions are emitted through `ui-event`
     (preferred, signature-driven) or as a new `UnifiedEvent` variant. This is
     about which frontend listener handles Full Auto updates.

## Backend work (Autopilot Desktop backend)

### 1) Full Auto controller + state (app-server driven)

Add a controller under `apps/autopilot-desktop/src-tauri/src/agent/`:

- `full_auto/mod.rs`: `FullAutoController`, `FullAutoSessionState`.
- `full_auto/aggregator.rs`: builds `TurnSummary` from events.
- `full_auto/decision.rs`: runs `FullAutoDecisionSignature`.
- `full_auto/actions.rs`: issues follow-up `turn/start` or `send_message`.

Required behaviors:
- Trigger on authoritative end-of-turn event.
- Enforce hard stops (user stop, budget exceeded, failed turn, pending approvals
  timeout).
- Track soft-stop signals (no progress, repeated errors, compaction churn).
- Persist decisions with inputs/outputs for replay.

### 2) App-server event plumbing

- Use the existing app-server session and map its `turn/*` events into the
  backend event stream used by Full Auto.
- Ensure the event stream provides the fields needed for `TurnSummary` (plan,
  diff snapshots, approvals, token usage).
- Ensure only one active turn per session at a time (guard against overlap).

### 3) DSPy signatures

- Implement `FullAutoDecisionSignature` in dsrs with inputs/outputs described in
  `docs/FULLAUTO.md`.
- Optional: add a `ToolInputSignature` for auto-responding to
  `item/tool/requestUserInput` when safe.
- Add tests for schema + parsing (required for signatures).

### 4) Policy gating + budgets

- Add guardrails in the controller: budget thresholds, backoff, cooldown,
  max-turns per session, and no-progress detection.
- Confidence gating: low-confidence decisions should pause and surface in UI.

### 5) Logging and replay

- Emit a decision record per turn with: summary inputs, decision output,
  confidence, and whether it was executed or overridden.
- Store alongside session history so it can be exported later.

## Frontend work (Autopilot Desktop UI)

### 1) Full Auto UI controls

- Add a Full Auto toggle and status badge to the Autopilot canvas.
- Display the last decision + reason + confidence.
- Provide Stop / Pause controls with explicit stop reason.

### 2) Full Auto event rendering

- Render `TurnSummary` and decision outcomes inside the canvas (panel or node).
- If using `ui-event`, add a catalog component for Full Auto status.
- If using `unified-event`, add a new event renderer in
  `apps/autopilot-desktop/src/components/autopilot-canvas/`.

### 3) Approvals and tool inputs

- If Full Auto pauses for approvals, surface a blocking UI panel with
  approve/deny actions.
- If auto-approving, still log the decision and show it in the timeline.

## Suggested phase plan (app-server first)

1. **App-server connection + mapping**
   - Add app-server stream, map `turn/*` events into backend event pipeline.
2. **Full Auto controller + DSPy decision**
   - Implement controller, `FullAutoDecisionSignature`, policy gating.
3. **UI integration**
   - Toggle, status, decision summaries, and stop reasons in the canvas.
4. **Guardrails + replay**
   - Budget limits, cooldown, loop detection, decision logging.

## Dependencies and references

- `docs/FULLAUTO.md`
- `apps/autopilot-desktop/docs/autopilot/ARCHITECTURE.md`
- `apps/autopilot-desktop/docs/autopilot/IMPLEMENTATION.md`
- `apps/autopilot-desktop/docs/adjutant-agent.md`
- `docs/dsrs-effuse-ui-plan.md`
- `crates/dsrs/docs/SIGNATURES.md`
