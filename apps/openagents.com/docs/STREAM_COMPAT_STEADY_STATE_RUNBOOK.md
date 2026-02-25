# Stream Compatibility Lane Steady-State Runbook

Tracking: `OA-WEBPARITY-081`

## Purpose

Define the canonical steady-state operating policy for the compatibility stream lane:
- `POST /api/chat/stream`
- `POST /api/chats/{conversationId}/stream`

This runbook applies after one-shot cutover completion and keeps the lane stable without reopening phased cutover behavior.

## Canonical Operation Policy

1. Cutover is one-shot. Do not run recurring route-target toggles as a steady-state operation.
2. Codex app-server protocol remains the only chat authority; compatibility aliases are adapter-only output surfaces.
3. Compatibility windows are managed only through the compatibility policy:
   - `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
4. Retired header semantics stay removed from active success responses.

## Required Dashboards / Alerts

Monitor and alert on these error classes and SLO indicators:

1. Compatibility handshake rejections:
   - `compatibility.rejected.control.*`
   - breakout by code: `invalid_client_build`, `unsupported_protocol_version`, `unsupported_schema_version`, `upgrade_required`
2. Compatibility stream terminal errors:
   - `turn_failed`
   - `turn_aborted`
   - `turn_interrupted`
   - `codex_error`
   - `adapter_preview_unavailable`
3. Stream endpoint SLOs:
   - HTTP 5xx rate for `/api/chat/stream` and `/api/chats/{id}/stream`
   - p95 latency for `/api/chat/stream` and `/api/chats/{id}/stream`
4. Bridge acceptance baseline:
   - `legacy.chat.stream.bridge.accepted` throughput and sudden drop/spike anomalies

## Steady-State Verification Cadence

Run on each release candidate and at least weekly:

1. Stream contract smoke:

```bash
BASE_URL=https://openagents.com \
AUTH_TOKEN=<authenticated-user-token> \
DRY_RUN=0 \
apps/openagents.com/scripts/run-production-stream-contract-smoke.sh
```

2. Control surface smoke:

```bash
OPENAGENTS_BASE_URL=https://openagents.com \
OPENAGENTS_CONTROL_ACCESS_TOKEN=<authenticated-user-token> \
apps/openagents.com/deploy/smoke-control.sh
```

3. Cutover gate rehearsal (non-mutating):

```bash
DRY_RUN=1 \
APPLY_ROUTE_FLIP=0 \
BASE_URL=https://openagents.com \
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service \
SLO_ERROR_BUDGET_CONSUMED_PERCENT=0 \
apps/openagents.com/scripts/run-production-stream-cutover.sh \
  stable-revision canary-revision
```

## Incident / Rollback Procedure

If compatibility stream error classes breach SLO thresholds:

1. Execute route-level rollback + traffic rollback using the existing canary runbook:
   - `apps/openagents.com/docs/CANARY_ROLLBACK_RUNBOOK.md`
2. Capture artifacts from:
   - `apps/openagents.com/scripts/run-production-stream-cutover.sh`
   - `apps/openagents.com/scripts/run-production-rust-route-flip.sh`
   - `apps/openagents.com/deploy/run-canary-rollback-drill.sh`
3. Attach artifact paths and timeline in the incident issue.
