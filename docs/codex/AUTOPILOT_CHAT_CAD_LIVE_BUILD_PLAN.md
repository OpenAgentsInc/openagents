# Autopilot Chat -> Live CAD Build Plan

## Objective

Enable a user to ask for a design in the main `Autopilot Chat` pane and have Codex execute a fully structured CAD workflow end-to-end:

1. detect CAD design intent in chat,
2. open/focus the CAD pane automatically,
3. build/update CAD state through tool calls,
4. stream visible build progress while the turn is running,
5. leave deterministic CAD state, rebuild receipts, and transcript trace after completion.

This plan is scoped to MVP architecture boundaries:

- `apps/autopilot-desktop` owns chat UX, pane orchestration, Codex lane plumbing, and tool-call execution.
- `crates/cad` owns typed CAD intents, dispatch, rebuild determinism, validity/events, and export contracts.

## Current State (Codebase Reality)

### What already exists

- Main chat turn path is in [`apps/autopilot-desktop/src/input/actions.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/actions.rs).
  - `run_chat_submit_action` sends `turn/start` through Codex lane.
  - It can attach one selected skill via `UserInput::Skill`.
- OpenAgents tool bridge already exists in [`apps/autopilot-desktop/src/input/tool_bridge.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/tool_bridge.rs).
  - Supports `openagents.pane.*`, `openagents.cad.intent`, and `openagents.cad.action`.
  - Auto-exec policy for `openagents.*` is wired in [`apps/autopilot-desktop/src/input/reducers/codex.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/codex.rs).
- CAD intent translation and typed dispatch exist in `crates/cad`:
  - [`crates/cad/src/chat_adapter.rs`](/Users/christopherdavid/code/openagents/crates/cad/src/chat_adapter.rs)
  - [`crates/cad/src/intent.rs`](/Users/christopherdavid/code/openagents/crates/cad/src/intent.rs)
  - [`crates/cad/src/dispatch.rs`](/Users/christopherdavid/code/openagents/crates/cad/src/dispatch.rs)
- CAD pane rendering and rebuild worker already exist:
  - [`apps/autopilot-desktop/src/panes/cad.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/panes/cad.rs)
  - [`apps/autopilot-desktop/src/input/reducers/cad.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/cad.rs)
  - [`apps/autopilot-desktop/src/cad_rebuild_worker.rs`](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/cad_rebuild_worker.rs)
- Codex tooling docs/skill already exist:
  - [`docs/codex/CODEX_PANE_CAD_TOOLING.md`](/Users/christopherdavid/code/openagents/docs/codex/CODEX_PANE_CAD_TOOLING.md)
  - [`skills/autopilot-pane-control/SKILL.md`](/Users/christopherdavid/code/openagents/skills/autopilot-pane-control/SKILL.md)

### Gaps blocking the requested UX

- CAD design requests in chat are not routed into a guaranteed "agent tool workflow" mode.
  - Current skill attachment is single-select and not CAD-aware.
- `openagents.cad.intent` updates dispatch/session state but does not yet guarantee a rebuild cycle for each intent mutation, so "being built in realtime" is incomplete.
- Main chat transcript does not surface tool-call execution/progress in a user-visible way.
  - Timeline events are recorded, but not rendered in the simple chat pane.
- There is no explicit CAD orchestration contract for multi-step design sessions (plan -> intent sequence -> rebuild checkpoints -> completion summary).
- There is no dedicated "CAD builder skill" tuned for this workflow.
- There is no deterministic test harness that asserts full chat -> tool-call -> CAD rebuild behavior in one flow.

## Target User Experience

When a user types a request like:

`Design a wall-mount rack for 2 Mac Studio units in aluminum with high airflow`

the app should:

1. keep the interaction in the main `Autopilot Chat` pane,
2. run Codex with CAD-capable skill/tool context,
3. auto-open and focus the CAD pane when CAD work starts,
4. show live "Autopilot is building CAD..." progress in chat while tool calls execute,
5. show the CAD pane updating as intent steps trigger rebuilds,
6. return a final assistant message summarizing what was built (variants/material/major params/export status).

## Architecture Changes (High Level)

### 1) Chat CAD routing

Add a deterministic CAD request classifier in desktop input layer to decide whether a turn should run in CAD orchestration mode. This classifier should be explicit and traceable (e.g., keyword + structured intent signals + optional JSON detection), and it should set turn metadata used by skill/tool injection and UI progress state.

### 2) Skill/tool injection policy

Move from "one manually selected skill only" to deterministic per-turn skill set composition for CAD turns:

- preserve user-selected skill behavior for generic turns,
- inject CAD control skill(s) for CAD turns,
- enforce predictable order and conflict handling,
- emit diagnostics when required skill/tool context is unavailable.

### 3) CAD intent -> rebuild coupling

Every state-mutating CAD intent from tool calls must enqueue a rebuild cycle with explicit trigger provenance (`ai-intent:*`) so the viewport and receipts evolve in lockstep with tool execution.

### 4) Live progress surfacing in main chat

Expose tool-call + CAD rebuild progress in the visible chat transcript. This should not dump raw protocol JSON; it should render concise state updates (opened CAD pane, applied intent, rebuild queued, rebuild committed, warnings summary, etc.).

### 5) Orchestration and safeguards

Define a strict CAD orchestration contract:

- no free-text geometry mutation at authority boundary,
- all CAD mutations via typed `CadIntent`,
- bounded retries for transient tool failures,
- final reconciliation snapshot between chat summary and CAD pane state.

## Staged Delivery

### Stage A: Routing + Turn Context

Introduce CAD turn detection and metadata so chat turns can opt into CAD build orchestration deterministically.

### Stage B: Skill/Tool Context

Guarantee that CAD turns have the right skill/tool context without manual pane interaction.

### Stage C: Real CAD Mutation Loop

Ensure `openagents.cad.intent` and related tool flows trigger real rebuilds and update visible CAD state incrementally.

### Stage D: Realtime UX + Observability

Render live build progress in the main chat and improve diagnostics for tool/build phases.

### Stage E: Reliability + Gates

Add integrated tests and release checks proving end-to-end chat -> CAD build functionality.

## Sequential GitHub Issue Backlog

Implement in order. Each issue below includes scope, acceptance, and validation commands/tests.

---

### Issue 1: Add CAD turn classifier and turn metadata in chat submit path

**Title**
`Autopilot Chat: classify CAD design turns and attach turn orchestration metadata`

**Description**
Add deterministic CAD-turn classification in `run_chat_submit_action` path and persist per-turn metadata in chat state (e.g., `is_cad_turn`, classifier reason, timestamps). This is routing-only; no UI behavior changes yet.

**Scope**

- Add classifier module near input layer (`apps/autopilot-desktop/src/input/`).
- Populate per-turn metadata at submission time.
- Emit diagnostics timeline entries for classifier decisions.

**Acceptance Criteria**

- CAD-looking prompts classify true with reason string.
- Non-CAD prompts classify false.
- Classifier result is queryable for the active turn.

**Validation**

- New unit tests for classifier positives/negatives.
- Reducer/action tests asserting metadata is attached on submit.

---

### Issue 2: Add multi-skill per-turn input assembly with deterministic ordering

**Title**
`Autopilot Chat: support deterministic multi-skill attachment per turn`

**Description**
Extend `assemble_chat_turn_input` from one optional selected skill to a deterministic list for the turn. Preserve backward compatibility for user-selected skill, and allow CAD-turn policy to append required skills.

**Scope**

- Refactor `assemble_chat_turn_input` signature and call sites.
- Add skill resolution utilities by skill name/path.
- Preserve current non-CAD behavior.

**Acceptance Criteria**

- Turn input can include multiple `UserInput::Skill` entries.
- Ordering is deterministic and tested.
- Existing non-CAD chat flow remains unchanged.

**Validation**

- Unit tests on assembly order/dedupe/disabled-skill handling.
- Existing chat submit tests updated and passing.

---

### Issue 3: Add CAD orchestration skill and default CAD-turn skill policy

**Title**
`Skills: add autopilot-cad-builder skill and auto-attach for CAD turns`

**Description**
Create a first-party CAD builder skill that explicitly instructs Codex to use `openagents.pane.*` + `openagents.cad.*` tools for design requests, and wire CAD-turn policy to attach it automatically.

**Scope**

- Add new skill under `skills/` (name + docs + examples).
- CAD-turn policy injects `autopilot-cad-builder` and `autopilot-pane-control` (or consolidated single skill) as required.
- Add failure diagnostics when required skill is missing/disabled.

**Acceptance Criteria**

- CAD turns include required skill(s) without manual selection.
- If skill missing/disabled, user-visible actionable error appears.
- Non-CAD turns do not auto-attach CAD skill unless configured.

**Validation**

- Skill discovery tests with simulated enabled/disabled states.
- Action-layer tests asserting correct skill set attached for CAD turns.

---

### Issue 4: Make `openagents.cad.intent` enqueue deterministic rebuild cycles

**Title**
`CAD Tool Bridge: couple intent dispatch to rebuild queue with ai-intent provenance`

**Description**
After successful CAD intent dispatch via tool bridge, enqueue CAD rebuild cycle immediately with trigger metadata (`ai-intent:<intent_name>`), so CAD pane updates as the agent builds.

**Scope**

- Update `execute_cad_intent` + CAD reducer integration.
- Ensure rebuild queue + receipt pipeline runs for intent mutations.
- Ensure pane auto-opens/focuses for CAD turns.

**Acceptance Criteria**

- Each successful mutating CAD intent produces a rebuild receipt.
- `last_good_mesh_payload` updates after each committed intent cycle.
- Rebuild provenance identifies AI-origin trigger.

**Validation**

- Integration tests for tool call -> dispatch -> rebuild receipt.
- Snapshot assertions for revision increments + receipt trail.

---

### Issue 5: Add CAD orchestration state machine for tool-driven build sessions

**Title**
`Autopilot CAD: add build-session orchestrator state for agent-driven design turns`

**Description**
Add explicit state machine for CAD agent runs (`idle -> planning -> applying -> rebuilding -> summarizing -> done/failed`) so chat and CAD panes can show coherent progress and recover from partial failures.

**Scope**

- Add orchestration state struct in app state.
- Track per-turn CAD build phases and latest tool/rebuild outcomes.
- Persist bounded event history for current/last CAD turn.

**Acceptance Criteria**

- Each CAD turn transitions through valid states only.
- Failures capture phase + remediation hint.
- Session clears or archives cleanly after turn completion.

**Validation**

- State-machine unit tests (valid/invalid transitions).
- Reducer tests for success and failure paths.

---

### Issue 6: Surface tool-call and rebuild progress in main chat transcript

**Title**
`Autopilot Chat UI: render live CAD build progress during agent tool execution`

**Description**
Expose human-readable tool/rebuild progress inside the main chat transcript (not only diagnostics). Add concise status rows tied to active assistant message and CAD build state.

**Scope**

- Extend chat message model to include progress events.
- Render progress blocks in `panes/chat.rs`.
- Wire tool-call auto-response and rebuild commits into progress feed.

**Acceptance Criteria**

- User sees live progress while CAD turn runs.
- Progress survives streaming and ends in clear done/failed state.
- No overflow/clipping regressions in pane layout.

**Validation**

- UI snapshot tests for in-progress + completed + failed CAD turns.
- Manual verification script in runbook.

---

### Issue 7: Expand CAD tool contract for snapshot/progress checkpoints

**Title**
`Codex Tooling: add explicit CAD checkpoint/snapshot tool responses for orchestration`

**Description**
Strengthen tool contract with explicit checkpoint data (revision, variant, pending rebuild, warning counts, analysis summary) so the agent can reason about next steps deterministically.

**Scope**

- Enhance `openagents.cad.intent`/`openagents.cad.action` response `details`.
- Add/standardize snapshot action usage pattern (existing `snapshot` path via pane actions).
- Document contract updates.

**Acceptance Criteria**

- CAD tool responses contain enough state for stepwise planning.
- Agent can query and branch based on snapshot without UI scraping.
- Contract documented with examples.

**Validation**

- Tool bridge unit tests for response shape.
- Docs updated in `docs/codex/`.

---

### Issue 8: Add CAD-turn failure handling and fallback behavior

**Title**
`Autopilot CAD: robust error handling for tool failures, parse failures, and rebuild failures`

**Description**
Implement bounded retry + fallback policy for CAD turns. Ensure clear chat-visible errors and safe termination when tools fail or CAD rebuild cannot commit.

**Scope**

- Define error classes and retry policy by failure type.
- Wire fallbacks (e.g., request clarification, switch to stricter `intent_json`, abort with summary).
- Add metrics counters for failure classes.

**Acceptance Criteria**

- No silent CAD-turn failures.
- Terminal error message includes next action for user.
- Retry policy is bounded and deterministic.

**Validation**

- Reducer tests for representative failure classes.
- Harness script covering failure injections.

---

### Issue 9: Add end-to-end chat->CAD live-build integration harness

**Title**
`Testing: add deterministic e2e harness for chat-driven CAD build with tool calls`

**Description**
Create an end-to-end test harness that simulates a CAD design chat turn, verifies tool-call execution, checks rebuild receipts, and asserts transcript progress output.

**Scope**

- Add fixture scripts and expected snapshots.
- Verify final CAD revision + mesh receipt + chat progress transcript.
- Include failure scenario fixtures.

**Acceptance Criteria**

- E2E harness passes for success flow.
- E2E harness catches regressions in tool routing/rebuild coupling/progress UI.

**Validation**

- New test target under `apps/autopilot-desktop` integration tests.
- Add CI invocation from existing CAD reliability lanes.

---

### Issue 10: Release gates and operator runbook for chat-driven CAD

**Title**
`Release: add CAD chat-build gate checklist and operational debug runbook`

**Description**
Document launch criteria and debugging workflow for this feature. Include required checks, known failure signatures, and safe rollback toggles.

**Scope**

- Add docs under `docs/codex/` and reference from existing runbooks.
- Add gate script updates if needed.
- Include manual smoke sequence.

**Acceptance Criteria**

- Team can run and verify feature without tribal knowledge.
- Gate checklist blocks release if core flow fails.

**Validation**

- Runbook dry-run by another engineer.
- Gate script output includes pass/fail for this flow.

## Cross-Cutting Constraints

- Keep CAD authority in typed contracts (`CadIntent` + dispatch state), not free text mutation.
- Do not add app-specific logic into `crates/wgpui`.
- Preserve existing non-CAD chat flow and startup behavior.
- Maintain pane no-overflow guarantees while adding progress UI.
- Keep tool bridge namespace allowlisted (`openagents.*` only).

## Rollout Strategy

1. Ship routing + skill policy behind feature flag if needed.
2. Enable rebuild coupling for CAD intents.
3. Enable transcript progress rendering.
4. Promote to default after harness + gate pass.

## Done Definition (Program Level)

This plan is complete when:

- A user can request CAD work in the main chat,
- Codex reliably uses tool/skill pathways to operate CAD,
- CAD pane opens and updates in realtime during the turn,
- chat transcript shows live build progress,
- and e2e gates prove deterministic behavior across success/failure paths.
