# ADR-0003: Khala WS-Only Replay Transport

## Status

Accepted

## Date

2026-02-21

## Owner Lane

`owner:contracts-docs`

## Context

Khala is the runtime-owned sync lane for replayable read-model delivery. Prior architecture iterations mixed SSE, polling, and websocket behavior across surfaces, causing ambiguity in replay semantics and operational ownership.

OpenAgents needs a locked transport contract for:

1. live update delivery,
2. replay/resume behavior,
3. stale-cursor recovery,
4. ordering and duplicate handling guarantees.

## Decision

Khala transport is WS-only for live sync delivery, with replay bootstrap and stale-cursor recovery semantics defined as part of the websocket subscription protocol.

Normative transport rules:

1. WebSocket is the only live subscription transport for Khala topics.
2. Authority mutations never travel over Khala WebSocket topics.
3. Commands/mutations remain authenticated HTTP APIs.
4. Client subscriptions include resume watermark context per topic.
5. Server replay runs first, then live tail, using `(topic, seq)` as logical ordering key.
6. If requested cursor is below retention floor, server returns `stale_cursor` and client must reset watermark + resnapshot.
7. Delivery is at-least-once; clients must discard duplicate frames where `seq <= last_applied`.

Disallowed patterns:

1. New SSE lane for Khala live updates.
2. Polling transport as primary live sync path.
3. Bidirectional RPC/command semantics over Khala topic streams.

## Rust-Era Boundary Impact

- Control-plane boundary: remains HTTP for commands and token minting.
- Runtime authority boundary: remains source of ordered event truth.
- Khala delivery boundary: explicitly limited to replay/live delivery.
- Client/runtime contract boundary: replay/resume/stale-cursor behavior is protocol contract.
- Deployment/runtime ops boundary: reconnect/replay alarms and runbooks are required.

## Invariant Gate Mapping

Source: `docs/plans/active/rust-migration-invariant-gates.md`

1. Invariants affected:
   - `INV-02` (HTTP command authority only)
   - `INV-03` (WS-only live sync transport)
   - `INV-06` (Khala not authority write path)
   - `INV-07` (ordering/idempotent apply semantics)
2. Preservation/change:
   - Locks WS-only policy and no-RPC-over-WS boundary.
   - Locks stale-cursor + replay bootstrap behavior for compatibility.
3. Follow-up gate requirements:
   - Reconnect/replay tests and drift alarms must remain in release evidence.
   - New sync docs and surface changes must cite this ADR.

## Compatibility and Migration Plan

1. Backward/forward compatibility:
   - Transitional SSE/poll endpoints may exist only for non-Khala legacy paths during migration.
   - New Khala features must use WS semantics defined here.
2. Rollout sequence:
   - Keep HTTP bootstrap endpoints authoritative for initial state.
   - Shift surfaces to WS subscriptions with per-topic watermarks.
   - Remove legacy live transports as closure issues complete.
3. Migration requirements:
   - Ensure stale-cursor handling and replay bootstrap are implemented consistently across web/desktop/iOS clients.

## Rollback and Failure Impact

1. Rollback triggers:
   - Replay gaps or duplicate application regressions.
   - Widespread stale-cursor loops without recovery.
   - WS auth/reconnect failure spikes breaching SLO thresholds.
2. Rollback procedure:
   - Revert transport changes causing replay regression.
   - Preserve runtime authority writes; pause promotion until replay correctness is restored.
3. Residual risk:
   - During migration, mixed client maturity can produce stale-cursor bursts; runbooks and alarms are mandatory mitigations.

## Verification

Required operational/test evidence:

```bash
./scripts/run-cross-surface-contract-harness.sh
apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh
cd apps/runtime && mix test test/openagents_runtime_web/channels/sync_channel_test.exs
```

## Consequences

### Positive

- Single unambiguous transport doctrine for Khala live delivery.
- Clear separation between command APIs and subscription streams.
- Stronger replay correctness and incident response consistency.

### Negative

- Surfaces relying on legacy SSE/poll live behavior must migrate or be retired.
- WS reconnect behavior becomes critical path and must be rigorously tested.

### Neutral

- HTTP bootstrap remains unchanged and continues to provide initial hydration.

## Alternatives Considered

1. Keep dual-transport model (SSE + WS) indefinitely.
   - Rejected: duplicate semantics and operator complexity.
2. Polling-first with optional websocket acceleration.
   - Rejected: weaker replay semantics and higher latency/cost profile.
3. WS with command RPC multiplexing.
   - Rejected: violates authority mutation boundary and increases blast radius.

## References

- `docs/ARCHITECTURE-RUST.md`
- `docs/sync/thoughts.md`
- `docs/sync/ROADMAP.md`
- `docs/sync/SURFACES.md`
- `apps/runtime/docs/RESTART_RECONNECT_CHAOS.md`
- `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
- Related issue: `OA-RUST-076` / `#1891`
