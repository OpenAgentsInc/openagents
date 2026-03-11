# Codex Integration Docs

This folder is the primary reference set for Codex integration in desktop MVP, with emphasis on pane tooling and chat-driven CAD orchestration.

## Suggested Reading Order

1. [`CAD_CHAT_BUILD_IMPLEMENTATION.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md)
2. [`CODEX_PANE_CAD_TOOLING.md`](/Users/christopherdavid/code/openagents/docs/codex/CODEX_PANE_CAD_TOOLING.md)
3. [`CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_CHAT_BUILD_RELEASE_RUNBOOK.md)
4. [`CAD_PHASE2_DEMO_RUNBOOK.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_PHASE2_DEMO_RUNBOOK.md)
5. [`EXEC.md`](/Users/christopherdavid/code/openagents/docs/codex/EXEC.md)
6. [`REMOTE.md`](/Users/christopherdavid/code/openagents/docs/codex/REMOTE.md)

## Documents

### Implementation + Contracts

- [`CAD_CHAT_BUILD_IMPLEMENTATION.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md)
  - comprehensive shipped architecture, runtime flow, state model, retries/failure classes, tests, and operator controls.
- [`CODEX_PANE_CAD_TOOLING.md`](/Users/christopherdavid/code/openagents/docs/codex/CODEX_PANE_CAD_TOOLING.md)
  - runtime API contract for `openagents.pane.*` and `openagents.cad.*` tools.

### Operations + Release

- [`CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_CHAT_BUILD_RELEASE_RUNBOOK.md)
  - release readiness checklist, manual smoke script, failure signatures, rollback toggles, and escalation flow.
- [`CAD_PHASE2_DEMO_RUNBOOK.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_PHASE2_DEMO_RUNBOOK.md)
  - phase-2 robotic hand demo choreography, capture checklist, and deterministic checkpoints.
- [`LIVE_HARNESS.md`](/Users/christopherdavid/code/openagents/docs/codex/LIVE_HARNESS.md)
  - codex live harness probes and manual protocol debugging workflow.
- [`EXEC.md`](/Users/christopherdavid/code/openagents/docs/codex/EXEC.md)
  - app-owned non-interactive Codex execution contract for scripts and local
    automation.
- [`REMOTE.md`](/Users/christopherdavid/code/openagents/docs/codex/REMOTE.md)
  - local-first remote companion setup, safety model, and operator commands.

### Plans / Historical Backlog

- [`AUTOPILOT_CHAT_CAD_LIVE_BUILD_PLAN.md`](/Users/christopherdavid/code/openagents/docs/codex/AUTOPILOT_CHAT_CAD_LIVE_BUILD_PLAN.md)
  - staged rollout plan and issue-by-issue delivery sequence for chat->CAD live build.
- [`CODEX_PANE_CAD_TOOLING_PLAN.md`](/Users/christopherdavid/code/openagents/docs/codex/CODEX_PANE_CAD_TOOLING_PLAN.md)
  - original pane and CAD tooling expansion plan.

## Cross-References Outside This Folder

- [`docs/PANES.md`](/Users/christopherdavid/code/openagents/docs/PANES.md)
  - pane inventory and codex tool control surface summary.
- [`docs/CODEX_INTEGRATION_RELEASE_CHECKLIST.md`](/Users/christopherdavid/code/openagents/docs/CODEX_INTEGRATION_RELEASE_CHECKLIST.md)
  - repo-level codex release checks (includes chat-CAD gates).
- [`docs/CODEX_INTEGRATION_DEBUG_RUNBOOK.md`](/Users/christopherdavid/code/openagents/docs/CODEX_INTEGRATION_DEBUG_RUNBOOK.md)
  - repo-level codex incident triage and debug command set.
