# Khala Sync Roadmap (Rust-Era, WS-Only)

Date: 2026-02-21  
Status: Active program  
Owner lanes: Runtime, Web, Desktop, iOS, Onyx, Infra, Protocol  
Authority ADRs: `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`, `docs/adr/ADR-0002-proto-first-contract-governance.md`

Khala is the runtime-owned sync/replay delivery system for Rust-era surfaces.

## Program Goal

Deliver and operate a production-grade WS-only sync lane with deterministic replay/resume semantics for:
- `apps/openagents.com/web-shell`
- `apps/autopilot-desktop`
- `apps/autopilot-ios`
- `apps/onyx` (explicit limited integration scope)

## Non-Negotiable Constraints

1. Runtime/Postgres remains authority for execution state.
2. Khala is projection/replay delivery only (no authority writes).
3. Live transport is WebSocket-only (no SSE/poll fallback lanes).
4. Proto contracts remain schema authority.
5. Commands are HTTP-only; subscriptions are WS-only.

## Current Baseline (2026-02-21)

Completed baseline capabilities:
1. Runtime fanout/replay endpoints and topic ACL checks exist in Rust runtime.
2. Sync token minting exists at `POST /api/sync/token`.
3. Rust-era docs now define canonical surface matrix and runbook gates.

Remaining migration work:
1. Remove residual legacy references from historical status artifacts when touched.
2. Keep cross-surface contract harness coverage aligned to active consumer set.
3. Close final rollout issues tied to OA-RUST WS-only gates.

## Surface Matrix (Authoritative)

| Surface | Primary Topic Scope | Integration Status |
|---|---|---|
| `apps/openagents.com/web-shell` | codex summaries + run summaries | Active (feature-flag and rollout staged) |
| `apps/autopilot-desktop` | codex worker events + optional summaries | Active primary operator lane |
| `apps/autopilot-ios` | codex worker events + optional summaries | Active follow/monitor lane |
| `apps/onyx` | optional run summaries only | Scoped/limited integration lane |

## Workstreams

## Workstream A: Protocol and Contract Stability

### SYNC-001: Keep proto envelope and error taxonomy stable
Status: In Progress

Done when:
- `proto/openagents/sync/v1/*` remains backward compatible for active clients.
- `stale_cursor`, auth denial, and payload limit errors remain deterministic.

Verification:
- `./scripts/local-ci.sh proto`

### SYNC-002: Keep command-vs-subscription contract explicit
Status: Completed

Done when:
- Sync docs consistently state HTTP-only commands and WS-only subscriptions.
- No sync doc prescribes RPC-style command transport over WS.

Verification:
- `./scripts/local-ci.sh docs`

## Workstream B: Runtime Delivery Correctness

### SYNC-003: Replay/resume correctness gates
Status: In Progress

Done when:
- replay, stale-cursor, and duplicate-frame behavior are covered by runtime tests.
- docs and runbooks match implemented behavior.

Verification:
- `cargo test -p openagents-runtime-service server::tests::khala_topic_messages -- --nocapture`
- `cargo test -p openagents-runtime-service projectors::tests -- --nocapture`

### SYNC-004: Fanout/backpressure operational gates
Status: In Progress

Done when:
- slow-consumer and fairness metrics are visible in rollout checks.
- runbook rollback procedures reference current runtime controls.

Verification:
- `curl -sS "$RUNTIME_BASE_URL/internal/v1/khala/fanout/metrics?topic_limit=20" -H "Authorization: Bearer $RUNTIME_ADMIN_TOKEN" | jq`

## Workstream C: Surface Rollout and Cutover

### SYNC-005: Web shell lane signoff
Status: In Progress

Done when:
- web-shell lane is duplicate-free, monotonic by `(topic, seq)`, and WS-only in rollout cohort.

Verification:
- `./scripts/local-ci.sh all-rust`

### SYNC-006: Desktop lane signoff
Status: In Progress

Done when:
- desktop receives codex worker events via WS-only lane and applies deterministic replay on reconnect.

Verification:
- `cargo run -p autopilot-desktop`
- runtime fanout endpoint checks from Workstream B

### SYNC-007: iOS lane signoff
Status: In Progress

Done when:
- iOS handshake, token minting, WS connect, and replay/resume behavior are stable.

Verification:
- `xcodebuild -project apps/autopilot-ios/Autopilot/Autopilot.xcodeproj -scheme Autopilot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test -only-testing:AutopilotTests`

### SYNC-008: Onyx scoped integration declaration
Status: Completed

Done when:
- docs clearly limit Onyx to selected read/admin scope (no full codex-worker event lane in v1).

Verification:
- `./scripts/local-ci.sh docs`

## Workstream D: Operations and Runbooks

### SYNC-009: WS-only rollout runbook alignment
Status: Completed

Done when:
- `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md` reflects Rust-era consumer set and rollback gates.

Verification:
- `./scripts/local-ci.sh docs`

### SYNC-010: Historical status handling policy
Status: In Progress

Done when:
- `docs/sync/status/*` is explicitly treated as historical snapshots.
- active guidance points operators to canonical surface docs/runbooks.

Verification:
- `./scripts/local-ci.sh docs`

## Rollout Gates

Do not advance to broader cohorts unless all are green:
1. WS auth/topic error rates remain within SLO.
2. Replay bootstrap latency remains in budget.
3. Reconnect storms remain bounded.
4. Slow-consumer evictions do not trend upward.
5. Surface UX remains duplicate-free and near-real-time.

## References

- `docs/sync/SURFACES.md`
- `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
