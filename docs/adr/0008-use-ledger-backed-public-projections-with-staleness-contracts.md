---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root INVARIANTS.md, apps/openagents.com/INVARIANTS.md, docs/systems/README.md, apps/openagents.com/scripts/check-zero-debt-architecture.mjs
informed: OpenAgents contributors and agents
---

# Use ledger-backed public projections with staleness contracts

## Context and Problem Statement

OpenAgents public surfaces expose product state, proof state, stats, Forum
activity, Khala counters, payouts, training truth, product promises, and world
projection rows. These surfaces must not look fresher or more authoritative than
their source ledgers. Root invariants require every public projection to carry a
freshness contract, and the OpenAgents.com invariant ledger defines the shared
`projection_staleness.v1` vocabulary and projection-surface inventory.

## Decision Drivers

* Public projections need explicit source authority and freshness semantics.
* D1-backed rows, generated rows, and public read models must stay auditably
  connected to source ledgers.
* Stale projections must declare staleness instead of serving old data as
  current.
* New `/api/public/...` routes need a reviewable inventory and deploy-time
  guard.

## Considered Options

* Ledger-backed projections with declared staleness contracts
* Ad hoc route payloads without a projection ledger
* UI-owned public state without Worker/D1 source authority

## Decision Outcome

Chosen option: "Ledger-backed projections with declared staleness contracts",
because it matches the current Worker/D1 projection model, the public projection
staleness invariant, and the architecture guard that inventories public
projection routes.

### Consequences

* Good, because readers can distinguish live-at-read, rebuilt-on-transition,
  and stored-snapshot surfaces.
* Good, because public state remains traceable to Worker/D1 ledgers, migrations,
  and typed projection contracts.
* Bad, because adding a public route also requires projection-ledger and
  staleness-contract work.

### Confirmation

Compliance is confirmed by app-specific invariant review, projection tests, D1
migrations, and `check:deploy`. The zero-debt architecture check scans public
route literals and fails routes missing from the projection-surface ledger or
the shared staleness vocabulary.

## Pros and Cons of the Options

### Ledger-backed projections with declared staleness contracts

* Good, because it preserves public authority and freshness boundaries.
* Good, because it gives operators and agents a stable inventory for projection
  audits.
* Bad, because it adds maintenance overhead for each public surface.

### Ad hoc route payloads without a projection ledger

* Good, because it is fast for one-off endpoints.
* Bad, because it makes freshness, source authority, and public safety hard to
  verify.

### UI-owned public state without Worker/D1 source authority

* Good, because it can be convenient for prototypes.
* Bad, because public product state would drift from the authoritative Worker
  and D1 ledgers.

## More Information

* `INVARIANTS.md` ("Public Projection Staleness")
* `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness Declaration")
* `docs/systems/README.md` ("D1 / projections / projection-surface ledger")
* `apps/openagents.com/workers/api/src/public-projection-staleness.ts`
* `apps/openagents.com/workers/api/src/public-projection-staleness.test.ts`
* `apps/openagents.com/scripts/check-zero-debt-architecture.mjs`
