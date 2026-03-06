# ADR-0002: Provider Presence Cardinality + TTL Policy

- Status: Accepted
- Date: 2026-03-05
- Owners: desktop + contracts-docs
- Supersedes: none
- Related: `ADR-0001-spacetime-domain-authority-matrix.md`

## Context

`providers_online` is now sourced from Spacetime presence reducers, but cardinality semantics were ambiguous across:
- device rows,
- worker lanes,
- identity-level counts.

TTL/heartbeat semantics also require a fixed policy to avoid counter drift and inconsistent offline cleanup behavior.

## Decision

### 1) Canonical counting unit for `providers_online`

`providers_online` MUST mean **identity cardinality**.

Definition:
- Count unique `nostr_pubkey_hex` values among rows where `status = online` and row is not stale/expired.

Rationale:
- User-facing Mission Control should answer "how many provider identities are online," not "how many processes/devices are online".
- Prevents over-counting when one provider identity runs multiple devices.

### 2) Presence row unit

Presence registration remains **device/session-scoped**:
- key fields: `node_id`, `session_id`, optional `worker_id`, `nostr_pubkey_hex`.

This preserves operational detail while keeping `providers_online` identity-stable.

### 3) TTL policy

Policy values (current retained defaults):
- `heartbeat_interval_ms = 5_000`
- `stale_after_ms = 30_000`
- `challenge_ttl_ms = 300_000`

Offline transitions:
- explicit offline: `offline_reason = explicit_offline`
- TTL expiry sweep: `offline_reason = ttl_expired`

### 4) Multi-device / worker semantics

- Same identity on multiple devices:
  - `Device` cardinality increments per online device row.
  - `Identity` cardinality remains one.
- `Worker` cardinality key is `<nostr_pubkey_hex>:<worker_id_or_node_id>`.
  - This supports internal worker-lane telemetry without changing canonical `providers_online` meaning.

## Query Contract

`providers_online` contract is:
- source: `spacetime.presence`
- cardinality: `identity`
- selection: `DISTINCT nostr_pubkey_hex WHERE status='online'`
- stale handling: rows beyond TTL are swept to offline before being counted.

Any query/UI field using non-identity cardinality MUST be explicitly labeled (`device` or `worker`) and must not reuse `providers_online` name.

## Test Scenarios

Required scenario coverage:

1. Multi-device same identity:
- Expect `Device=2`, `Worker=2` (different worker ids), `Identity=1`.

2. TTL expiry cleanup:
- Online row exceeds `stale_after_ms` and is marked offline with `ttl_expired`.

3. Explicit offline:
- Row goes offline immediately with `explicit_offline`.

4. Worker dedupe semantics:
- Same identity + same worker id across multiple device rows yields `Worker=1`.

Current implemented coverage:
- `ttl_expiry_marks_online_rows_offline`
- `multi_device_same_identity_cardinality_is_stable`
- `online_heartbeat_offline_lifecycle_is_deterministic`
- `worker_cardinality_dedupes_same_identity_worker_id`

## Consequences

- Mission Control `providers_online` remains stable under multi-device usage.
- Device/process-level telemetry remains available but explicitly labeled.
- Presence cleanup behavior is deterministic and testable.
