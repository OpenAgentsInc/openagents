# Autopilot Unified ACP Stream

## Goal
Render Autopilot as a single long-running conversation that streams tokens end-to-end, while still surfacing planning/todo/execution phases. The UI should consume one ACP event stream instead of bespoke DSPy markers or phase-specific buffers.

## Current State (2026-01)
- **Adjutant Autopilot loop** supports both legacy DSPy stage markers (ChannelOutput) and ACP notifications (AcpChannelOutput).
- **Autopilot UI** consumes ACP notifications for Autopilot and can render DSPy stage cards via `_meta.openagents_dspy_stage`.
- **Autopilot CLI/service** has distinct phase streams (plan/exec/review/fix) and stores per-phase events.

## Options Considered
1) **Keep DSPy markers + custom parsing**  
   - Minimal change, but UI must keep special parsing logic forever.
   - Hard to reuse across shells/clients; doesn’t align with ACP ecosystem.

2) **Add native DSPy token streaming in dsrs**  
   - Cleanest from a modeling perspective, but requires non-trivial dsrs changes.
   - Doesn’t directly solve the UI unification problem.

3) **Emit ACP notifications from Autopilot (Chosen)**  
   - Aligns with existing ACP paths for Codex/Codex.
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

### UI Integration (Autopilot UI)
- Consume ACP notifications and convert to existing UI events:
  - Agent message/thought chunks → chat stream
  - Plan updates → inline plan text in the same conversation
  - Optional: `_meta.openagents_dspy_stage` → DSPy stage cards
  - Tool calls → tool cards (optional; can be added incrementally)
- Result: a single long-running conversation that includes planning + execution context.

## Implications
- **Phase visibility** remains, but is now embedded in the same stream.
- **ACP unification** enables reuse of ACP tooling (replay, telemetry, rlog) across Autopilot.
- **Streaming parity** with Codex/Codex for UIs and future shells.

## Follow-up Work
- Emit structured ACP `ToolCall`/`ToolCallUpdate` from Adjutant tool execution (local LM path).
- Update autopilot-service and autopilot-shell to consume ACP stream directly.
- Optionally render ACP plan updates with custom components instead of plain text.
- Introduce a single session buffer for plan/exec/review/fix in Autopilot checkpoints.

## Open Questions
- Should plan updates be shown as inline text or dedicated UI cards?
- How should ACP thought chunks be styled (same as assistant text vs. muted)?
- Do we want a stable session id for Autopilot ACP streams (resume/replay)?

---

## Addendum: Codebase Analysis & Recommendations (2026-01-10)

Based on a deep exploration of the codebase, here are recommendations for each open question:

### Q1: Plan updates — Inline text or dedicated UI cards?

**Recommendation: Dedicated UI cards**

**Evidence:**
- DSPy stage cards already exist with full rendering infrastructure (`crates/autopilot/src/app/ui/rendering/dspy.rs`)
- `DspyStage::TodoList` renders tasks with:
  - Green accent (120°) border
  - Status symbols: `□` pending, `◐` in-progress, `✓` complete, `✗` failed
  - Colored status indicators per task
- Current plan handling (`crates/autopilot/src/app/autopilot/handler.rs:408-428`) formats as plain text via `format_plan_update()` → `ResponseEvent::Chunk`, losing visual structure
- The `SessionUpdate::Plan` → `DspyStage::TodoList` mapping is natural and reuses existing components

**Implementation path:**
1. In `acp_notification_to_response()`, convert `SessionUpdate::Plan` to `ResponseEvent::DspyStage(DspyStage::TodoList {...})`
2. Map `acp::PlanEntryStatus` → `TodoStatus` (Pending/InProgress/Completed → Pending/InProgress/Complete)
3. Remove `format_plan_update()` plain-text fallback

### Q2: ACP thought chunks — Same as assistant or muted?

**Recommendation: Muted styling**

**Evidence:**
- Palette already defines distinct colors (`crates/autopilot/src/app/ui/theme.rs:66-68`):
  - `assistant_text`: Cyan HSL(180°, 50%, 70%) — bright, primary
  - `thinking_text`: Gray HSL(0°, 0%, 50%) — significantly muted
- Current implementation treats `AgentThoughtChunk` identically to `AgentMessageChunk` (both → `ResponseEvent::Chunk`)
- The "thinking" indicator uses muted styling with "..." but doesn't render thought content distinctly

**Rationale:**
- Thoughts = internal reasoning users may want visibility into, but shouldn't compete with primary output
- Matches Codex/Codex convention where extended thinking is visually subordinate
- Allows power users to follow reasoning without overwhelming casual users

**Implementation path:**
1. Add `ResponseEvent::ThoughtChunk(String)` variant
2. In `acp_notification_to_response()`, map `AgentThoughtChunk` → `ThoughtChunk` (not `Chunk`)
3. In chat rendering, use `palette.thinking_text` for thought chunks
4. Consider collapsible UI for long thought sequences

### Q3: Stable session ID for resume/replay?

**Recommendation: Yes — use existing session ID infrastructure**

**Evidence from existing session architecture:**

| Layer | ID Format | Location | Purpose |
|-------|-----------|----------|---------|
| Main session | `HHMMSS-{8-hex}` | `crates/autopilot-core/src/logger.rs:141-158` | Top-level identifier |
| Per-phase SDK | Codex API tokens | `StartupState.{plan,exec,review,fix}_session_id` | API resume |
| Adjutant DSPy | UUID v4 | `crates/adjutant/src/dspy/sessions.rs:159` | Decision tracking |
| rlog replay | `replay_{session_id}` | `crates/autopilot-core/src/replay.rs:99` | Trajectory export |

**Checkpoint system already supports resume:**
- `SessionCheckpoint` (`crates/autopilot-core/src/checkpoint.rs:20-113`) stores:
  - `session_id`, `phase`, `iteration`
  - Per-phase event cursors (`plan_cursor`, `exec_cursor`, etc.)
  - Per-phase SDK session IDs for API continuation
- Storage: `~/.openagents/sessions/{session_id}/checkpoint.json`

**Implementation path:**
1. Use main `session_id` (HHMMSS-hex) as ACP stream `conversation_id`
2. Embed session ID in ACP `_meta` for every notification
3. Store ACP event cursor in checkpoint alongside phase cursors
4. For replay: rlog header `id:` field already captures session ID
5. For resume: checkpoint restores SDK session IDs for API continuation

**Session ID lifecycle:**
```
autopilot run "task"
    → generate_session_id() → "153045-a1b2c3d4"
    → ACP stream with conversation_id = "153045-a1b2c3d4"
    → checkpoint.json saves cursor + SDK session IDs
    → [crash/interrupt]
    → resume from checkpoint
    → continue ACP stream with same conversation_id
    → [complete]
    → rlog export with id: "153045-a1b2c3d4"
```

### Summary Table

| Question | Recommendation | Key Files |
|----------|----------------|-----------|
| Plan rendering | Dedicated cards (reuse `DspyStage::TodoList`) | `dspy.rs`, `handler.rs` |
| Thought styling | Muted (`thinking_text` color) | `theme.rs`, `chat.rs` |
| Session ID | Yes, use `HHMMSS-{8-hex}` format | `logger.rs`, `checkpoint.rs` |

### References

- ACP adapter converters: `crates/acp-adapter/src/converters/rlog.rs`
- Autopilot UI event handling: `crates/autopilot/src/app/autopilot/handler.rs`
- DSPy stage rendering: `crates/autopilot/src/app/ui/rendering/dspy.rs`
- Theme/palette: `crates/autopilot/src/app/ui/theme.rs`
- Session checkpoints: `crates/autopilot-core/src/checkpoint.rs`
- Session ID generation: `crates/autopilot-core/src/logger.rs`
- Session documentation: `crates/autopilot-core/docs/SESSIONS.md`
