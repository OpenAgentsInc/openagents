# Autopilot Unified ACP Stream

## Goal
Render Autopilot as a single long-running conversation that streams tokens end-to-end, while still surfacing planning/todo/execution phases. The UI should consume one ACP event stream instead of bespoke DSPy markers or phase-specific buffers.

## Current State (2026-01)
- **Adjutant Autopilot loop** emits plain text chunks plus DSPy stage markers (`<<DSPY_STAGE:...>>`) for UI cards.
- **Coder** parses these markers and renders DSPy stage cards alongside chat output.
- **Autopilot CLI/service** has distinct phase streams (plan/exec/review/fix) and stores per-phase events.

## Options Considered
1) **Keep DSPy markers + custom parsing**  
   - Minimal change, but UI must keep special parsing logic forever.
   - Hard to reuse across shells/clients; doesn’t align with ACP ecosystem.

2) **Add native DSPy token streaming in dsrs**  
   - Cleanest from a modeling perspective, but requires non-trivial dsrs changes.
   - Doesn’t directly solve the UI unification problem.

3) **Emit ACP notifications from Autopilot (Chosen)**  
   - Aligns with existing ACP paths for Claude/Codex.
   - Lets UI consume one stream (AgentMessageChunk/AgentThoughtChunk/Plan/ToolCall*).
   - Keeps phase metadata without separate panes or buffers.

## Proposed Design
### ACP Output for Autopilot
- Add an `AcpChannelOutput` to `adjutant::autopilot_loop` that implements `AutopilotOutput`.
- Map Autopilot signals to ACP:
  - Tokens → `SessionUpdate::AgentMessageChunk`
  - Iteration/verification/progress notes → `SessionUpdate::AgentThoughtChunk`
  - Todo list → `SessionUpdate::Plan` (PlanEntry list)
  - Task status changes → plan updates (same Plan entries with updated status)
- Embed structured DSPy stage payloads in ACP text `_meta` under `openagents_dspy_stage` so UI can render optional stage cards without parsing markers.
- This removes the need for DSPy marker parsing in UIs.

### UI Integration (Coder)
- Consume ACP notifications and convert to existing UI events:
  - Agent message/thought chunks → chat stream
  - Plan updates → inline plan text in the same conversation
  - Optional: `_meta.openagents_dspy_stage` → DSPy stage cards
  - Tool calls → tool cards (optional; can be added incrementally)
- Result: a single long-running conversation that includes planning + execution context.

## Implications
- **Phase visibility** remains, but is now embedded in the same stream.
- **ACP unification** enables reuse of ACP tooling (replay, telemetry, rlog) across Autopilot.
- **Streaming parity** with Claude/Codex for UIs and future shells.

## Follow-up Work
- Emit structured ACP `ToolCall`/`ToolCallUpdate` from Adjutant tool execution (local LM path).
- Update autopilot-service and autopilot-shell to consume ACP stream directly.
- Optionally render ACP plan updates with custom components instead of plain text.
- Introduce a single session buffer for plan/exec/review/fix in Autopilot checkpoints.

## Open Questions
- Should plan updates be shown as inline text or dedicated UI cards?
- How should ACP thought chunks be styled (same as assistant text vs. muted)?
- Do we want a stable session id for Autopilot ACP streams (resume/replay)?
