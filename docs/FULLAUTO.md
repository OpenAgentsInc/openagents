# Full Auto Mode Design (v1)

## Summary

Design a Full Auto mode that keeps running Codex turns until explicit stop
conditions are met. The controller should be event-driven off the Codex
app-server stream, using `turn/completed` as the authoritative end-of-turn
signal, and a decision policy (DSRS signature) to decide whether to continue,
pause, or stop after each turn.

## Goals

- Run autonomously across multiple turns without manual user prompts.
- Use app-server notifications as the single source of truth for turn state.
- Decide continuation via typed DSRS signatures and policy gating.
- Enforce safety, budget, and stop conditions deterministically.
- Persist replayable decisions and inputs for auditing and recovery.

## Non-goals (v1)

- Multi-agent orchestration across different runtimes/providers.
- New UI patterns beyond showing Full Auto status, decisions, and stop reasons.
- Replacing the app-server or the existing turn/item model.

## Codex app-server lifecycle (observed)

Sources: `/Users/christopherdavid/code/codex/codex-rs/app-server/README.md`,
`/Users/christopherdavid/code/codex/codex-rs/app-server-protocol/src/protocol/common.rs`,
`/Users/christopherdavid/code/codex/codex-rs/app-server-protocol/src/protocol/v2.rs`,
`/Users/christopherdavid/code/codex/codex-rs/docs/protocol_v1.md`,
`/Users/christopherdavid/code/codex/codex-rs/app-server/tests/suite/v2/*.rs`.

Key signals and ordering:

- Initialize first: `initialize` request, then client sends `initialized`.
- Start or resume a thread: `thread/start` (emits `thread/started`) or
  `thread/resume` (no notification).
- Start a turn: `turn/start` returns a Turn and emits `turn/started`.
- Items stream as the turn runs:
  - `item/started` -> zero or more deltas -> `item/completed`.
  - Deltas include `item/agentMessage/delta`, `item/reasoning/*`,
    `item/commandExecution/outputDelta`, etc.
- End of turn: `turn/completed` is the authoritative "done" hook.
  - `turn.status` is `completed`, `interrupted`, or `failed`.
  - `turn.error` includes failure details when failed.
  - `turn/completed` is emitted after `turn/interrupt`.
- Errors can arrive mid-turn via `error` and still be followed by
  `turn/completed` (see stream error tests).
- `turn/plan/updated` events provide plan steps and statuses.
- `turn/diff/updated` provides aggregated diff snapshots for file changes.
- `thread/tokenUsage/updated` delivers ongoing token usage and model limits.
- `thread/compacted` notifies that Codex compacted history.
- Server-initiated requests that must be answered:
  - `item/commandExecution/requestApproval`
  - `item/fileChange/requestApproval`
  - `item/tool/requestUserInput`

Takeaway: Full Auto should treat `turn/completed` as the only end-of-turn hook,
and must respond to approval and tool-input requests to avoid stalls.

## Full Auto orchestration model

### Core components

- `FullAutoController`: owns the run loop and policy for each thread.
- `FullAutoSessionState`: per-thread state (latest turn id, items, plan,
  token usage, errors, approvals pending, last decision).
- `EventAggregator`: ingests app-server notifications and updates state.
- `DecisionEngine` (DSRS signature): decides what to do after each turn.
- `ActionExecutor`: executes decisions via app-server requests.

### Event-driven flow (single thread)

1. Initialize app-server and start or resume a thread.
2. Issue the first `turn/start` using the user's initial prompt.
3. Stream events, updating in-memory state:
   - Items, deltas, plan updates, token usage, errors.
4. On `turn/completed`:
   - Compile a `TurnSummary` from items + plan + diff + errors + usage.
   - Run `FullAutoDecisionSignature`.
   - Execute the decision:
     - `continue` -> send next `turn/start` with synthesized input.
     - `pause` -> stop and wait for user.
     - `stop` -> end Full Auto with a reason.
     - `review` -> optionally `review/start` then re-evaluate.

### Decision signature (draft)

`FullAutoDecisionSignature` should be a typed DSRS signature:

Inputs:
- `thread_id`, `turn_id`
- `last_turn_status` + `turn_error` (if any)
- `turn_plan` (from `turn/plan/updated`)
- `diff_summary` (from `turn/diff/updated`)
- `token_usage` (from `thread/tokenUsage/updated`)
- `pending_approvals` / `pending_tool_inputs`
- `recent_actions` (last N decisions)
- `compaction_events` (recent)

Outputs:
- `action`: `continue | pause | stop | review`
- `next_input`: synthesized user message for `turn/start` (if continue)
- `reason`: brief justification (for UI + logs)
- `confidence`: for gating fallback behavior

Policy gating:
- Low confidence or unsafe contexts should pause or request user input.
- If a decision conflicts with hard stop conditions, stop wins.

### Guidance Modules (demo mode)

Full Auto now supports a **Guidance Modules** pipeline that intercepts the
*first* user message in Full Auto and routes it through typed DSRS signatures.
The guidance pipeline prefers the configured **Codex decision model** when
available, and falls back to the **local Ollama** demo model.

Enable demo mode with:

```
OPENAGENTS_GUIDANCE_MODE=demo
OPENAGENTS_GUIDANCE_MODEL=ollama:llama3.2  # optional override
```

In demo mode:
- Full Auto builds `GuidanceInputs` (goal + summary + state + permissions).
- The first message runs `GuidanceRouterSignature` (route: respond | understand | plan).
- Routes can call `TaskUnderstandingSignature` or `PlanningSignature` for a
  richer initial response.
- If the user sends a minimal “go/just do it” prompt, **super mode** runs
  repo research plus `TaskUnderstandingSignature → PlanningSignature →
  GuidanceDirectiveSignature`, emitting each step to the UI and dispatching the
  directive to Codex.
- After each Codex turn completes in demo mode, Full Auto re-enters the guidance
  loop (repo intel + directive) and auto-continues until guardrails stop it.
- Guardrails are enforced the same way as legacy Full Auto decisions.

CLI demo:

```
autopilot guidance demo --summary path/to/summary.json
```

## Stop conditions and guardrails

Hard stops:
- User presses Stop / Full Auto toggle off.
- `turn.status == failed` with non-recoverable errors.
- Budget exceeded (tokens/time/cost thresholds).
- Unanswered approval or tool-input request after timeout.

Soft stops (policy-based):
- No progress across N turns (same plan + diff unchanged).
- Repeated errors of the same type.
- Context compaction triggered too frequently (signal of loop).
- Agent explicitly indicates completion in its final message.

## Approvals and tool-input handling

Approvals are blocking requests. Full Auto should pick one of these strategies:

1. Avoid approvals entirely by using `approvalPolicy: "never"` on
   `thread/start` or `turn/start`. This keeps the turn moving but removes a
   human approval gate.
2. Auto-approve based on a local policy:
   - Safe commands only, or safe paths only, etc.
   - Still surface actions in the UI with an audit trail.
3. Pause Full Auto and ask the user for approval.

`item/tool/requestUserInput` requires a response to continue. Options:

- Auto-answer via a dedicated `ToolInputSignature`.
- Pause and request user input if the signature is low confidence.

## Multi-session scheduling

Full Auto should manage multiple threads with an event-driven scheduler:

- Only one active turn per thread at a time.
- `turn/completed` marks a thread as ready for decision.
- A scheduler processes ready threads in a fair round-robin order.
- Backoff and cooldown to avoid rapid, wasteful loops.

## UI and surfaces

Minimal UI additions:

- Full Auto toggle with current state (`running`, `paused`, `stopped`).
- Latest decision + reason (from `FullAutoDecisionSignature`).
- Active stop condition and manual override controls.

Use existing item/plan rendering:

- `turn/plan/updated` -> plan panel
- `turn/diff/updated` -> diff snapshot
- Item summaries for tool calls and file changes

## Logging and replay

Every decision should emit a receipt entry with:

- Inputs (turn summary, plan, usage, approvals)
- Decision output and confidence
- Whether the decision was executed or overridden
- Any stop condition hit

This aligns with replay and audit expectations in `SYNTHESIS_EXECUTION.md`.

## Issue and plan organization

Recommended structure:

- Treat app-server events as the single plan source for per-turn work.
- Introduce a separate "Full Auto run" plan for the orchestrator itself.
- Track engineering milestones in `ROADMAP.md` and link to this doc.
- Use `docs/WORK_LOG.md` for execution notes; keep "Full Auto" specific notes
  in a new section there (avoid new scattered docs).

Proposed milestones:

1. Event aggregation: stable per-thread state, item store, plan store.
2. Decision signature: DSRS signature + policy gating + tests.
3. Full Auto loop: trigger on `turn/completed`, issue next `turn/start`.
4. Guardrails: budgets, cooldowns, loop detection, approvals.
5. UI: toggle, status, decision summaries, stop reason.

## Open questions

- Should Full Auto synthesize its own "continue" prompt or reuse a fixed
  template? How do we avoid instruction drift?
- How should reviews (`review/start`) fit into the loop (always at the end,
  or on error only)?
- What is the minimal safe auto-approval policy for v1?
- Do we need explicit "pause until user input" states for all blocking
  requests, or can the controller auto-answer for some tool calls?
