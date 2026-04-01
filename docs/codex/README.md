# Codex Integration Docs

This folder is the primary reference set for Codex integration in desktop MVP, with emphasis on pane tooling and chat-driven CAD orchestration.

## Suggested Reading Order

1. [`ROADMAP_CODEX.md`](./ROADMAP_CODEX.md)
2. [`AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md`](./AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md)
3. [`PROBE_LOCAL_SIDECAR_ADOPTION.md`](./PROBE_LOCAL_SIDECAR_ADOPTION.md)
4. [`PROBE_SHELL_PROJECTION.md`](./PROBE_SHELL_PROJECTION.md)
5. [`PROBE_OPERATOR_CONTROLS.md`](./PROBE_OPERATOR_CONTROLS.md)
6. [`CAD_CHAT_BUILD_IMPLEMENTATION.md`](./CAD_CHAT_BUILD_IMPLEMENTATION.md)
7. [`CODEX_PANE_CAD_TOOLING.md`](./CODEX_PANE_CAD_TOOLING.md)
8. [`CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`](./CAD_CHAT_BUILD_RELEASE_RUNBOOK.md)
9. [`CAD_PHASE2_DEMO_RUNBOOK.md`](./CAD_PHASE2_DEMO_RUNBOOK.md)
10. [`EXEC.md`](./EXEC.md)
11. [`REMOTE.md`](./REMOTE.md)

## Documents

### Implementation + Contracts

- [`ROADMAP_CODEX.md`](./ROADMAP_CODEX.md)
  - live roadmap for the current Codex-backed Autopilot lane, including the
    explicit boundary between the app-owned shell and a future Probe runtime.
- [`AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md`](./AUTOPILOT_CODING_SHELL_AND_PROBE_DIRECTION.md)
  - architecture explanation of how Autopilot currently embeds Codex and how
    the same product shell should transition to Probe.
- [`PROBE_LOCAL_SIDECAR_ADOPTION.md`](./PROBE_LOCAL_SIDECAR_ADOPTION.md)
  - the shipped daemon-first local consumer boundary for Autopilot's Probe
    lane, including runtime selection, fallback, reconnect, and failure
    posture.
- [`PROBE_SHELL_PROJECTION.md`](./PROBE_SHELL_PROJECTION.md)
  - the current app-owned mapping from Probe sessions and turn-control truth
    into Autopilot thread, transcript, workspace, artifact, and attach-resume
    state.
- [`PROBE_OPERATOR_CONTROLS.md`](./PROBE_OPERATOR_CONTROLS.md)
  - the current desktop operator loop above Probe sessions, including queued
    follow-ups, interrupt, approval resolution, and queued-turn cancel.
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
