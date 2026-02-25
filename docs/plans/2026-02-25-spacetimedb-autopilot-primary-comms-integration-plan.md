# SpacetimeDB Autopilot Primary Comms Plan (Full Spacetime Replacement Track)

Status: active execution plan
Date: 2026-02-25
Owner lanes: `owner:autopilot`, `owner:runtime`, `owner:protocol`

This plan is the Autopilot-focused execution slice of:

- `docs/plans/spacetimedb-full-integration.md`

## 1) Scope

Deliver Spacetime-first Autopilot comms with explicit path to remove Spacetime dependencies from retained app surfaces.

In scope:

1. Presence, peer session comms, and collaboration state over Spacetime subscriptions.
2. Desktop checkpoint/replay/resume integration.
3. Runtime/control token + auth claim integration for Spacetime websocket sessions.
4. NIP-90/Nostr supplemental bridge behavior.

Out of scope:

1. Replacing Hydra/Aegis money/trust authority decisions.
2. Reintroducing long-term dual-primary sync lanes.

## 2) Spacetime Mechanics To Implement Explicitly

1. Connect to `/v1/database/:name_or_identity/subscribe` with supported WS subprotocol.
2. Use short-lived websocket token minting and scoped claims.
3. Handle `SubscribeApplied` snapshots then ordered `TransactionUpdate` batches.
4. Persist per-stream checkpoints and resume on reconnect.
5. Use confirmed-read mode for streams where durable visibility is required.
6. Use event tables for transient presence/progress signals.

## 3) Desktop Integration Requirements

1. Add Spacetime session lifecycle manager (connect, reconnect, token refresh, backoff).
2. Add deterministic apply engine keyed by `(stream_id, seq)`.
3. Add local checkpoint persistence.
4. Add health UI: connected state, lag, replay progress, token expiry.
5. Keep local Codex command/control as primary local execution lane.

## 4) Runtime/Control Integration Requirements

1. Control endpoint(s) for Spacetime claim/token issuance.
2. Runtime projection writer path into Spacetime reducers.
3. Compatibility negotiation payloads for sync protocol/version gating.
4. Audit metrics: lag, stale cursor frequency, duplicate suppression count.

## 5) Cutover Stages

### Stage 0: hidden validation lane

1. Spacetime connected but read-only in desktop.
2. Compare parity vs Spacetime outputs for equivalent streams.

### Stage 1: feature-flag primary lane

1. `OA_SPACETIME_PRIMARY_COMMS=1` routes desktop comms to Spacetime.
2. Keep rollback toggle to Spacetime for bounded cohorts only.

### Stage 2: default primary lane

1. Spacetime default-on for retained surfaces.
2. Spacetime only allowed in explicit emergency fallback mode.

### Stage 3: removal

1. Delete Spacetime client dependencies from desktop/runtime control paths.
2. Remove fallback toggles.

## 6) Verification Gates

1. Replay determinism tests across reconnect and restart.
2. Duplicate-delivery idempotency tests.
3. Stale-cursor recovery tests.
4. Token expiry/refresh tests.
5. Nostr bridge isolation tests (bridge failure cannot block Spacetime progression).

## 7) Completion Criteria

1. Desktop comms and sync run Spacetime-only in default and enforced policy modes.
2. Spacetime transport dependencies are removed from retained app code paths.
3. Incident runbooks and docs reference Spacetime as canonical sync lane.

