# ADR-0001: Spacetime Domain Authority Matrix

Date: 2026-03-04
Status: Accepted
Owners: desktop, runtime, contracts/docs
Related: `docs/MVP.md`, `docs/OWNERSHIP.md`

## Context

MVP language retained a strict statement that sync is delivery/replay only. Current product direction requires Spacetime reintroduction for selected app-db domains (especially provider online presence), while preserving hard money/policy authority boundaries.

Without a domain matrix, "authority" becomes ambiguous and review decisions diverge.

## Decision

Authority is scoped by domain. Spacetime is authoritative only for explicitly listed app-state domains. Money, policy, and trust remain command-authoritative.

### Domain Matrix

| Domain | Authority Owner | Mutations Allowed via Spacetime Reducers | Notes |
| --- | --- | --- | --- |
| Wallet balances, sends, receives, settlement truth | Wallet/control/runtime command lanes | No | Wallet-confirmed and receipt-backed truth only. |
| Credit, underwriting, trust, policy, security verdicts | Runtime/control command lanes | No | Must remain authenticated command mutations with receipts. |
| Provider/device online registration and liveness | Spacetime (presence tables/reducers) | Yes | Device/session presence authority with TTL heartbeat semantics. |
| Sync replay checkpoints and stream cursor continuity | Spacetime (checkpoint/sync tables/reducers) | Yes | Ordered replay and idempotent apply remain mandatory. |
| Non-monetary UI/event projections (activity/job-state projections) | Spacetime (projection streams) | Yes | Projection-only; cannot redefine money truth. |
| Aggregated counters derived from authoritative presence/projections | Spacetime query/subscription | Yes (derived) | Counter contracts must name counting unit and staleness policy. |

## Invariants

1. Money mutation authority remains authenticated command lanes.
2. Spacetime authority is restricted to explicitly approved app-db domains.
3. Any new Spacetime-authoritative domain requires ADR update before implementation.
4. Replay/apply must stay deterministic and idempotent per `(stream_id, seq)`.
5. UI labels must match data source truth (no proxy fields labeled as Spacetime).
6. Presence counter cardinality/TTL semantics are governed by `ADR-0002-provider-presence-cardinality-and-ttl-policy.md`.

## Consequences

1. Spacetime can be used for online/presence truth without collapsing money authority boundaries.
2. MVP docs must describe domain-scoped authority instead of blanket "never authority."
3. Reviewers can evaluate changes against a concrete matrix rather than implied doctrine.
