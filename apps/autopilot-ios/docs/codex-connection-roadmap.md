# Autopilot iOS -> Codex Connection Roadmap

Status: Active roadmap
Date: 2026-02-19
Owner: iOS + Runtime + Web teams

## Purpose

Define a comprehensive, implementation-ordered roadmap for connecting `apps/autopilot-ios/` to Codex with production-safe authority boundaries.

This roadmap references working patterns from `~/code/inbox-autopilot/` (local API client + SSE event loop) and adapts them for OpenAgents, where iOS cannot assume localhost Codex execution.

## Executive Summary

1. iOS should be Codex-admin and observability first, not Codex execution authority.
2. Runtime remains source of truth for worker lifecycle/events/policy/spend.
3. iOS talks to Laravel public APIs only (`/api/runtime/codex/workers*`).
4. Transport/execution backend can vary (`desktop`, hosted sandbox, future direct bridge), but runtime contract stays fixed.
5. First production lane should be runtime-mediated; direct iOS-to-local-Codex is optional and later.

## Architecture Guardrails

1. Runtime + Postgres are authoritative for worker lifecycle/events.
2. Convex is read-model projection only.
3. Laravel is auth/session authority and public API facade.
4. iOS never calls runtime internal `/internal/v1/*` directly.
5. Proto definitions under `proto/` remain schema authority.

References:

- `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `proto/README.md`

## Current Baseline

OpenAgents already has the cross-surface Codex worker contract in place:

1. Runtime internal worker APIs and durable state.
2. Laravel user-scoped proxy APIs for list/snapshot/request/events/stream/stop.
3. Web and mobile admin surfaces consuming those APIs.
4. Desktop runtime sync that mirrors local Codex events into runtime.

Primary references:

- `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
- `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`
- `apps/mobile/app/services/runtimeCodexApi.ts`
- `apps/autopilot-desktop/src/main.rs`

## Lessons to Reuse from inbox-autopilot

From `~/code/inbox-autopilot/`, reuse the following patterns:

1. Thin API client with centralized auth/session handling.
2. SSE stream parser returning `AsyncThrowingStream` and yielding typed events.
3. App-level event task that continuously merges streamed events into observable state.
4. Keychain-backed secret/session token storage.
5. Append-only event mindset for auditability.

Useful reference files:

- `/Users/christopherdavid/code/inbox-autopilot/Inbox Autopilot/Inbox Autopilot/DaemonAPIClient.swift`
- `/Users/christopherdavid/code/inbox-autopilot/Inbox Autopilot/Inbox Autopilot/AppModel.swift`
- `/Users/christopherdavid/code/inbox-autopilot/docs/ipc-contract.md`
- `/Users/christopherdavid/code/inbox-autopilot/daemon/src/routes.rs`

## iOS Connection Options (and when to use each)

### Option A: Runtime/Laravel-mediated Codex (recommended default)

Path:

`iOS -> Laravel /api/runtime/codex/workers* -> runtime -> desktop/hosted adapters`

Pros:

1. Matches existing web/mobile path.
2. Reuses ownership/policy/security controls.
3. Works for both desktop-backed and hosted-backed workers.
4. Best for production support and auditability.

Cons:

1. Depends on backend availability.
2. Requires stable user auth and network connectivity.

### Option B: iOS -> desktop companion over Tailscale/private network (later)

Path:

`iOS -> authenticated desktop gateway -> local Codex on desktop`

Pros:

1. Enables personal-device local execution access.
2. Useful for power users and low-latency personal workflows.

Cons:

1. Harder security posture (device trust, revocation, NAT/network complexity).
2. Requires additional gateway protocol and lifecycle management.
3. Can drift from runtime authority unless carefully mirrored.

Constraint:

If implemented, this path must still mirror all authoritative events/state into runtime so web/mobile/admin remain consistent.

### Option C: iOS -> hosted sandbox backend via runtime (parallel lane)

Path:

`iOS -> Laravel -> runtime -> sandbox backend (Cloudflare/Daytona/OpenAgents GCP)`

Pros:

1. Device-independent execution.
2. Easier team/shared access patterns.

Cons:

1. Infra/ops complexity.
2. Requires backend selection policy and cost guardrails.

Reference:

- `docs/codex/webapp-sandbox-and-codex-auth-plan.md`

## Decision for Initial Delivery

Use Option A as the default production lane:

1. build iOS on the existing Laravel runtime proxy APIs,
2. keep runtime authority intact,
3. add Option B/C later behind runtime contracts and policy gates.

## Minimum Viable Handshake (Real iPhone <-> Desktop Codex)

This is the minimum connection target for a real iOS device and a desktop Codex session.

### Handshake definition

Handshake is successful when all of the following happen:

1. iOS can discover or select a desktop-backed worker through Laravel runtime APIs.
2. iOS sends a handshake event into that worker stream.
3. Desktop receives that handshake event from runtime and emits an ack event.
4. iOS receives the ack on stream and marks the desktop session as connected.

### Proposed handshake event pair (MVP)

1. iOS -> runtime event ingest:
   - endpoint: `POST /api/runtime/codex/workers/{workerId}/events`
   - `event_type`: `worker.event`
   - payload:
     - `source: \"autopilot-ios\"`
     - `method: \"ios/handshake\"`
     - `handshake_id`
     - `device_id`
     - `occurred_at`
2. Desktop -> runtime ack event:
   - endpoint: `POST /api/runtime/codex/workers/{workerId}/events`
   - `event_type`: `worker.event`
   - payload:
     - `source: \"autopilot-desktop\"`
     - `method: \"desktop/handshake_ack\"`
     - `handshake_id`
     - `desktop_session_id`
     - `occurred_at`
3. iOS stream success condition:
   - `desktop/handshake_ack` received with matching `handshake_id` within timeout window (e.g. 30s).

### Why this MVP shape

1. Uses existing Laravel/runtime worker event APIs without introducing a new authority path.
2. Avoids forcing immediate runtime adapter redesign for request forwarding.
3. Creates a real bidirectional signal path between iOS and desktop through the authoritative runtime event ledger.
4. Keeps future Tailscale or hosted backend options compatible with the same handshake envelope.

### MVP prerequisites

1. Desktop launched with runtime sync env vars (`OPENAGENTS_RUNTIME_SYNC_*`).
2. iOS user authenticated against Laravel API.
3. Worker ownership is shared/principal-valid for both device actions.
4. iOS stream client supports reconnect + cursor continuity.

### GitHub execution issues

1. Tracker: `#1775`
2. iOS handshake client + ack matcher: `#1771`
3. Desktop stream listener + ack emitter: `#1772`
4. Runtime/Laravel contract docs + tests: `#1773`
5. Real-device runbook + checklist: `#1774`

## Roadmap Phases

### Phase 0: iOS Foundation and Contracts

Goal:

Create iOS app architecture that can consume Codex worker APIs cleanly.

Work:

1. Create iOS docs and module layout in `apps/autopilot-ios/docs/`.
2. Add a `RuntimeCodexClient` service layer in Swift.
3. Define typed worker/snapshot/stream/action models aligned with runtime/Laravel envelopes.
4. Implement auth token provider + Keychain storage.

Deliverables:

1. `apps/autopilot-ios/docs/ios-codex-first-structure.md`
2. `apps/autopilot-ios/docs/codex-connection-roadmap.md`
3. Swift API client skeleton.

### Phase 1: Codex Read Path (List/Snapshot/Stream)

Goal:

iOS can observe Codex worker state and stream events.

Work:

1. Worker list screen (`GET /api/runtime/codex/workers`).
2. Worker detail snapshot (`GET /api/runtime/codex/workers/{id}`).
3. SSE stream client (`GET /api/runtime/codex/workers/{id}/stream?cursor=&tail_ms=`).
4. Cursor management, reconnect strategy, duplicate suppression.
5. Event timeline UI with bounded in-memory retention.

Reference parity:

- `apps/mobile/app/services/runtimeCodexApi.ts`
- `/Users/christopherdavid/code/inbox-autopilot/Inbox Autopilot/Inbox Autopilot/DaemonAPIClient.swift`

Exit criteria:

1. signed-in user sees principal-scoped worker list.
2. stream reconnects and advances cursor without event loss/duplication.
3. snapshot and stream stay coherent under reconnect.

### Phase 2: Codex Admin Actions (Request/Stop)

Goal:

iOS can safely administer workers.

Work:

1. Request action UI (`POST /requests`) with JSON parameter editor.
2. Stop action UI (`POST /stop`) with reason field.
3. Error mapping for `401/403/409/422` to explicit UX states.
4. Action receipts in timeline/notifications.

Reference parity:

- `apps/openagents.com/resources/js/pages/admin/index.tsx`
- `apps/mobile/app/screens/CodexWorkersScreen.tsx`

Exit criteria:

1. request and stop actions work with runtime policy semantics.
2. forbidden/conflict states are shown clearly and non-destructively.

### Phase 3: Reliability and Observability Hardening

Goal:

Make iOS connection path stable in real-world network conditions.

Work:

1. exponential backoff with jitter for stream reconnects.
2. periodic list/snapshot refresh reconciliation.
3. `x-request-id` generation/propagation in iOS client.
4. local structured logs for failed API/stream operations.
5. optional local event cache for recent timeline continuity.

Reuse pattern:

- event loop/task model from `/Users/christopherdavid/code/inbox-autopilot/Inbox Autopilot/Inbox Autopilot/AppModel.swift`

Exit criteria:

1. network flap tests recover automatically.
2. action outcomes remain traceable via request IDs.

### Phase 4: Backend Expansion Lanes

Goal:

Support more execution topologies without changing iOS contract.

Workstream A (desktop remote bridge via Tailscale):

1. define desktop companion gateway auth model (device registration, short-lived tokens).
2. require runtime mirror for all worker lifecycle/events.
3. add policy switch to enable per-user/per-workspace.

Workstream B (hosted sandbox backends):

1. consume runtime-selected backend metadata in worker snapshot.
2. keep iOS action surface unchanged.
3. add backend health/status badges in UI (informational).

Exit criteria:

1. iOS app behavior remains contract-stable while backend varies.

### Phase 5: Convex Read-Model Enhancements (Optional)

Goal:

Improve responsiveness for summaries while preserving runtime authority.

Work:

1. mint Convex token via Laravel (`POST /api/convex/token`).
2. subscribe to worker summary projections for fast list/status updates.
3. continue using runtime/Laravel APIs for all control actions.

Exit criteria:

1. projection lag/status visible in iOS.
2. no control-path authority moved to Convex.

## Security and Trust Model

1. Keychain for user tokens and sensitive local state.
2. Do not store long-lived backend admin secrets in app.
3. Treat device as untrusted for authority; server enforces ownership/policy.
4. If Tailscale lane is added, require explicit device enrollment and revocation flow.
5. Follow least-privilege and replay-safe request patterns.

## Testing and Verification Matrix

### Unit

1. request/response decoding and error mapping.
2. SSE parser and cursor advancement.
3. retry/backoff logic and cancellation behavior.

### Integration (staging)

1. list -> snapshot -> stream lifecycle.
2. request -> response event chain.
3. stop behavior and conflict semantics.
4. auth expiry and re-auth behavior.

### End-to-end

1. desktop-originated worker events visible in iOS stream.
2. iOS-admin action visible in web admin timeline.
3. correlation IDs traceable across Laravel/runtime logs.

## Dependencies and Sequencing

1. Runtime/Laravel Codex APIs must remain stable and versioned.
2. iOS phase 1 and 2 can run in parallel with UI polish, but not with contract churn.
3. Any Tailscale/hosted expansion requires threat-model review before implementation.

## Risks and Mitigations

1. Risk: transport fragmentation across desktop/local/hosted lanes.
   Mitigation: single runtime worker contract, backend hidden behind runtime adapters.
2. Risk: stream reliability issues on mobile networks.
   Mitigation: robust SSE reconnection, cursor replay, periodic snapshot reconciliation.
3. Risk: authority creep into client.
   Mitigation: enforce server-side ownership and policy checks only.
4. Risk: backend-specific behavior leaks into iOS UX.
   Mitigation: strict client contract abstraction and typed model boundaries.

## Definition of Done (Codex Connection)

1. iOS can observe and administer Codex workers through Laravel/runtime APIs with parity to web/mobile.
2. stream reliability meets reconnect and cursor continuity requirements.
3. request/stop actions honor runtime policy/ownership semantics.
4. tracing/correlation headers and logs are operationally useful.
5. backend expansion (desktop bridge or hosted sandbox) does not require iOS contract rewrite.
