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

### PM Collaboration Candidates (Not Yet Spacetime-Authoritative)

The native `Project Ops` Step 0 and early Phase 2 work in `apps/autopilot-desktop` use canonical PM stream ids, replay-safe local projection caches, and shared checkpoint discipline. Those PM streams are intentionally shaped like the existing Spacetime stack, but they are not granted live remote authority by this ADR yet.

Before any PM reducer or subscription becomes authoritative, the exact domain must be promoted from this candidate table into the accepted domain matrix above.

| PM domain candidate | Current Phase 1 owner | Candidate live authority owner after follow-up ADR | Required gate before promotion |
| --- | --- | --- | --- |
| Work-item and cycle collaboration projections (`stream.pm.work_items.v1`, `stream.pm.cycles.v1`) | Desktop-local PM command/event/projection loop | Spacetime reducers/subscriptions | Update this ADR, freeze PM stream/grant/checkpoint contract, and land parity evidence for replay/resume. |
| PM activity and saved-view projections (`stream.pm.activity_projection.v1`, `stream.pm.saved_views.v1`) | Desktop-local PM projection streams | Spacetime projection streams | Ratify source-badge truth, reducer scope, and projection rebuild parity before cutover. |
| PM comments, mentions, and notifications | Not implemented yet | Spacetime projection streams | Add explicit stream ids, grants, moderation/error rules, and parity evidence before implementation depends on live subscriptions. |
| PM team/project coordination views | Not implemented yet | Spacetime reducers/subscriptions | Add explicit entity contract, multi-team scope rules, and ADR approval before collaboration cutover. |
| PM reporting and derived counters | Local or projection-derived only | Spacetime query/subscription | Counting unit, staleness semantics, and rebuild parity must be documented before any badge claims live authority. |
| PM agent-task coordination metadata | Not implemented yet | Spacetime coordination/projection streams | Keep runtime/work execution authority separate and ratify coordination-only scope before any live queue truth is introduced. |

### Explicit PM Exclusions

The following remain out of PM authority scope even if later PM panes reference them:

- wallet balances, payment receipts, settlement truth, payout finality
- trust, underwriting, policy, or security verdicts
- any money-moving mutation lane

## Invariants

1. Money mutation authority remains authenticated command lanes.
2. Spacetime authority is restricted to explicitly approved app-db domains.
3. Any new Spacetime-authoritative domain requires ADR update before implementation.
4. Replay/apply must stay deterministic and idempotent per `(stream_id, seq)`.
5. UI labels must match data source truth (no proxy fields labeled as Spacetime).
6. Presence counter cardinality/TTL semantics are governed by `ADR-0002-provider-presence-cardinality-and-ttl-policy.md`.
7. PM collaboration domains stay local/mirror-only until a candidate row is explicitly promoted into the accepted domain matrix and parity evidence exists for that cutover.

## Consequences

1. Spacetime can be used for online/presence truth without collapsing money authority boundaries.
2. MVP docs must describe domain-scoped authority instead of blanket "never authority."
3. Reviewers can evaluate changes against a concrete matrix rather than implied doctrine.
4. PM work can reuse stream ids, checkpoints, and replay discipline now without implying that live multi-user PM authority has already been approved.
