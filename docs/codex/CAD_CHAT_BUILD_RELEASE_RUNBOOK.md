# CAD Chat-Build Release Runbook

## Purpose

This runbook is the release and operations guide for the chat-driven CAD workflow:

`Autopilot Chat -> Codex tool call -> openagents.cad.intent -> CAD rebuild -> chat progress + CAD pane update`

Use this document as the single-source checklist for launch readiness, regression triage, and safe rollback.

## Feature Readiness Checklist

Release is blocked until all items are green:

- [ ] CAD release gates pass via `scripts/cad/release-gate-checklist.sh`.
- [ ] Chat-build e2e success/failure harness passes.
- [ ] CAD runbook and contract docs are current and linked from [`docs/codex/README.md`](/Users/christopherdavid/code/openagents/docs/codex/README.md).
- [ ] Tool response codes and failure-class contract are unchanged or intentionally versioned.
- [ ] Manual smoke script (below) is executed on current `main`.

## Required Gate Commands

Run from repo root:

```bash
scripts/cad/release-gate-checklist.sh
```

The gate output must include explicit pass lines for chat-build flow:

- `CAD release gate pass (E): chat-build e2e harness (success + failure)`
- `CAD release gate pass (E): chat-build runbook present`

For focused local verification:

```bash
scripts/cad/headless-script-ci.sh
cargo test -p autopilot-desktop cad_chat_build_e2e_harness -- --nocapture
```

## Manual Smoke Script (Operator)

1. Start desktop app on current `main`.
2. Ensure Codex is connected and `Autopilot Chat` pane is open.
3. Send: `Design a lightweight wall-mount rack for 2 Mac Studio units in 6061 aluminum.`
4. Confirm live behavior:
   - CAD pane auto-opens/focuses.
   - Chat transcript shows CAD progress rows (phase/tool/rebuild/events).
   - CAD pane state revision increments and mesh updates.
5. Send a follow-up edit: `Increase vent hole size by 20% and reduce weight.`
6. Confirm second rebuild receipt commits and progress reaches `done`.
7. Validate failure path with malformed tool intent in harness only:
   - `cargo test -p autopilot-desktop cad_chat_build_e2e_harness_failure_matches_golden -- --nocapture`
8. Confirm no overflow/flicker regressions in CAD pane rendering.

## Known Failure Signatures and Triage

### Failure Class: `intent_parse_validation`

Typical codes/messages:

- `OA-CAD-INTENT-PARSE-FAILED`
- `OA-CAD-INTENT-MISSING-PAYLOAD`
- chat progress rows show `failure_class=intent_parse_validation`

Action:

1. Retry with strict `intent_json` payload matching `CadIntent`.
2. Confirm parser retry budget was not exhausted unexpectedly.
3. Validate prompt/intent extraction in `apply_chat_prompt_to_cad_session_with_trigger_outcome`.

### Failure Class: `dispatch_rebuild`

Typical codes/messages:

- `OA-CAD-INTENT-DISPATCH-FAILED`
- `OA-CAD-INTENT-REBUILD-ENQUEUE-FAILED`
- `cad.build.rebuild.failed` events in progress rows

Action:

1. Inspect CAD reducer rebuild queue health and pending request handling.
2. Confirm `ai-intent:*` rebuild trigger was enqueued or safely aborted.
3. Verify last checkpoint includes `build_session`, `failure_metrics`, and `last_rebuild_receipt`.

### Failure Class: `tool_transport`

Typical signals:

- tool response submit retries exhausted
- `cad.build.response.submit_failed`

Action:

1. Confirm Codex lane connectivity and request/response sequencing.
2. Check retry counters in checkpoint `failure_metrics`.
3. Re-run with wire logging (`OPENAGENTS_CODEX_WIRE_LOG_PATH`).

## Escalation Decision Tree

1. Parse/validation failures only:
   - classify as prompt-contract issue, keep feature enabled, patch schema prompts/tests.
2. Rebuild enqueue/dispatch failures recurring:
   - enable rollback toggle (below), preserve non-CAD chat functionality, investigate CAD reducer path.
3. Tool transport instability:
   - treat as Codex lane reliability issue, keep CAD tool disabled until transport is stable.
4. Any silent mismatch between chat summary and CAD checkpoint:
   - block release; this violates deterministic contract.

## Safe Rollback Toggles

### Toggle 1: Disable CAD intent tool execution (hard stop)

Set before launch/runtime:

```bash
export OPENAGENTS_CAD_INTENT_TOOL_ENABLED=0
```

Effect:

- `openagents.cad.intent` returns `OA-CAD-INTENT-DISABLED`.
- no CAD mutation is executed through tool calls.
- other pane tools and non-CAD chat remain operational.

Re-enable:

```bash
export OPENAGENTS_CAD_INTENT_TOOL_ENABLED=1
```

### Toggle 2: Disable required CAD skills in Skills pane (soft stop)

Disable `autopilot-cad-builder` / `autopilot-pane-control` in the Skills pane.

Effect:

- CAD turns are blocked by required-skill policy and fail explicitly.
- non-CAD turns continue.

## Logging and Metrics Checkpoints

Inspect these checkpoints for every incident:

- Chat transcript CAD progress block:
  - `phase`
  - `tool`
  - `rebuild`
  - `failure_class`
  - `retries`
- Tool response `details.checkpoint` fields:
  - `document.revision`
  - `pending_rebuild`
  - `failure_metrics`
  - `build_session`
  - `last_rebuild_receipt`
- Gate/harness artifacts:
  - `apps/autopilot-desktop/tests/scripts/cad_chat_build_e2e_*`
  - `apps/autopilot-desktop/tests/goldens/cad_chat_build_e2e_*`

## Dry-Run Evidence (Scripted)

Use this command set as reproducible dry-run evidence:

```bash
scripts/cad/headless-script-ci.sh
cargo test -p autopilot-desktop cad_chat_build_e2e_harness -- --nocapture
scripts/cad/release-gate-checklist.sh
```

All three commands must pass before shipping chat-driven CAD build changes.
