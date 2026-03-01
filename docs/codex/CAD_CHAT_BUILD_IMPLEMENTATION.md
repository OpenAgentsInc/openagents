# CAD Chat-Build Implementation (Shipped)

## Scope

This document describes the shipped implementation that lets a user ask for CAD design work in `Autopilot Chat` and have Codex execute deterministic CAD mutations through OpenAgents tools while CAD progress is visible and auditable.

It covers:

- chat turn routing and CAD classification
- skill attachment policy for CAD turns
- tool bridge execution contract for pane + CAD mutation tools
- CAD orchestration state machine and failure accounting
- rebuild coupling and deterministic checkpointing
- e2e reliability harness and release gate integration
- operator rollback and triage controls

Primary ownership:

- `apps/autopilot-desktop`: orchestration, chat UX, pane control, tool execution, progress surfacing.
- `crates/cad`: typed intent translation/dispatch/eval/validity/material analysis primitives.

## Delivered Capabilities

1. CAD-turn routing in chat submit path with explicit classifier reason.
2. Deterministic multi-skill turn assembly (user-selected + policy-required).
3. Required CAD skills auto-attachment for CAD turns (`autopilot-cad-builder`, `autopilot-pane-control`).
4. OpenAgents tool bridge support for:
   - pane listing/open/focus/close/input/action
   - CAD intent/action execution.
5. CAD intent mutation path coupled to deterministic rebuild enqueue/receipt pipeline.
6. CAD build-session state machine (`idle -> planning -> applying -> rebuilding -> summarizing -> done|failed`).
7. Failure classes + retry counters in checkpoint contract and chat progress rows.
8. Deterministic e2e harness for success + failure chat-driven CAD tool-call flows.
9. Release-gate enforcement for chat-build reliability.
10. Operator rollback flag for disabling CAD intent tool execution safely.

## Code Map

### Chat turn routing + skill policy

- `apps/autopilot-desktop/src/input/actions.rs`
  - `run_chat_submit_action`
  - `assemble_chat_turn_input`
  - `cad_policy_skill_candidates_for_turn`
- `apps/autopilot-desktop/src/input/cad_turn_classifier.rs`
  - `classify_chat_prompt`
- `apps/autopilot-desktop/src/app_state.rs`
  - `AutopilotTurnMetadata`
  - `record_turn_submission_metadata`

### Tool bridge and CAD tool responses

- `apps/autopilot-desktop/src/input/tool_bridge.rs`
  - `OPENAGENTS_TOOL_NAMES`
  - `execute_cad_intent`
  - `execute_cad_action`
  - `cad_checkpoint_payload`
  - rollback flag: `OPENAGENTS_CAD_INTENT_TOOL_ENABLED`

### CAD reducer orchestration + rebuild coupling

- `apps/autopilot-desktop/src/input/reducers/cad.rs`
  - `apply_chat_prompt_to_cad_session_with_trigger_outcome`
  - `CadChatPromptApplyOutcome`
  - `enqueue_rebuild_cycle_with_retry`
  - `sync_cad_build_progress_to_chat`

### Tool-call auto execution and transport retries

- `apps/autopilot-desktop/src/input/reducers/codex.rs`
  - `queue_cad_tool_response_with_retry`
  - failure class extraction + remediation mapping from tool responses
  - CAD build-session lifecycle integration during tool-call handling

### CAD build session + failure metrics state

- `apps/autopilot-desktop/src/app_state_domains.rs`
  - `CadBuildSessionPhase`
  - `CadBuildFailureClass`
  - `CadBuildFailureMetricsState`
  - `CadBuildSessionState`
  - `CadBuildSessionArchiveState`

## End-to-End Runtime Flow

1. User submits prompt in `Autopilot Chat`.
2. Chat submit path classifies turn as CAD or non-CAD.
3. Turn metadata is recorded (`is_cad_turn`, classifier reason, submission sequence, timestamp).
4. CAD turns get required policy skills merged into turn input deterministically.
5. Codex lane receives `turn/start`.
6. Codex emits tool-call request for `openagents.*` tool.
7. Desktop auto-executes `openagents.*` tool call.
8. For `openagents.cad.intent`:
   - tool bridge resolves intent payload
   - CAD pane is ensured open
   - prompt/intent is applied via CAD chat adapter + typed intent dispatch
   - mutating intents enqueue rebuild trigger (`ai-intent:<intent>`)
   - checkpoint response includes CAD revision/variant/warnings/build-session/failure metrics.
9. CAD reducer rebuild worker commits receipt and last-good mesh.
10. Chat progress rows update with phase/tool/rebuild/failure/retry details.
11. Build-session state archives terminal result for post-turn debugging.

## Determinism and Reliability Contract

### Deterministic trigger provenance

- Rebuild triggers for AI tool mutations are prefixed as `ai-intent:<intent_name_lower>`.
- Trigger provenance is visible in progress rows and rebuild receipts.

### Bounded retries

- Intent parse canonicalization retry in tool bridge: max 1.
- Rebuild enqueue retry in CAD reducer: max 1.
- Tool-response submit retry in Codex reducer: max 2.

### Stable checkpoint contract

Tool responses include:

- schema versions (`oa.cad.tool_response.v1`, `oa.cad.checkpoint.v1`)
- document and variant context
- pending rebuild state
- warnings summary
- failure metrics (failures + retries)
- current/archived build-session state
- last rebuild receipt summary

### Golden snapshot policy

E2E harness snapshots normalize non-deterministic timing fields (`duration_ms`, `total_duration_ms`) while keeping deterministic CAD outputs (hashes, revisions, warnings, progress transitions) strict.

## Failure Model

### Failure classes

- `tool_transport`
- `intent_parse_validation`
- `dispatch_rebuild`

### Core CAD intent failure codes

- `OA-CAD-INTENT-MISSING-PAYLOAD`
- `OA-CAD-INTENT-DISABLED`
- `OA-CAD-INTENT-PARSE-FAILED`
- `OA-CAD-INTENT-DISPATCH-FAILED`
- `OA-CAD-INTENT-REBUILD-ENQUEUE-FAILED`

### Fallback semantics

- Parse/validation class -> `request_clarification` with strict intent-json hint.
- Dispatch/rebuild class -> `safe_abort` with remediation hint.

## Test and Harness Coverage

### Unit/integration test anchors

- `apps/autopilot-desktop/src/input/tool_bridge.rs` tests:
  - decode/shape validation
  - cad checkpoint contract fields
  - parse retry extraction
  - env flag parser behavior
- `apps/autopilot-desktop/src/input/reducers/cad.rs` tests:
  - `cad_chat_build_e2e_harness_success_matches_golden`
  - `cad_chat_build_e2e_harness_failure_matches_golden`
  - release-gate canonical script reliability

### Script fixtures

- `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_success_script.json`
- `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_failure_script.json`

### Golden snapshots

- `apps/autopilot-desktop/tests/goldens/cad_chat_build_e2e_success_snapshot.json`
- `apps/autopilot-desktop/tests/goldens/cad_chat_build_e2e_failure_snapshot.json`

### CI/release scripts

- `scripts/cad/headless-script-ci.sh`
- `scripts/cad/release-gate-checklist.sh`

Gate E now includes explicit pass/fail checks for chat-build harness and runbook presence.

## Operator Controls

### Rollback flag

- `OPENAGENTS_CAD_INTENT_TOOL_ENABLED=0`
  - disables `openagents.cad.intent`
  - returns explicit `OA-CAD-INTENT-DISABLED`
  - preserves non-CAD and pane tool behavior

### Skill-level soft stop

Disable `autopilot-cad-builder` / `autopilot-pane-control` in Skills pane to block CAD turns without disabling tool bridge globally.

## Observability Surfaces

1. Chat progress block rows:
   - phase
   - tool result
   - rebuild result
   - failure class
   - retries
   - event timeline
2. CAD checkpoint payload in tool response `details`.
3. CAD pane state:
   - revision
   - active variant
   - warnings
   - last rebuild receipt
4. Codex diagnostics and lane logs for transport-level failures.

## Known Operational Limits

1. CAD-turn classification is heuristic keyword/marker based; false positives/negatives are possible.
2. CAD mutation authority remains schema-driven; unsupported free-form operations must be converted into typed intents.
3. Deterministic snapshots intentionally ignore wall-clock timing jitter to prevent flaky tests.

## Change Checklist for Future Edits

When changing chat->CAD behavior:

1. Update tool contract docs (`CODEX_PANE_CAD_TOOLING.md`).
2. Update runbook (`CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`) if failures/gates/flags change.
3. Re-run chat-build e2e harness.
4. Re-run release gate checklist.
5. If contract changed intentionally, refresh and review golden snapshots.
