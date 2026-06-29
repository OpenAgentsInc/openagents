---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root INVARIANTS.md, apps/openagents.com/INVARIANTS.md, docs/systems/README.md
informed: OpenAgents contributors and agents
---

# Declare public projection staleness

## Context and Problem Statement

OpenAgents exposes public product, proof, stats, promise, Forum, Site, and
world projection surfaces. Many of those surfaces are backed by D1 rows or
composed live from D1 state, while others are generated records. Contributors
and agents need a settled rule for whether a public projection is current,
generated, stale, or intentionally live at read.

## Decision Drivers

* Prevent stale public data from being presented as current.
* Keep public surfaces explicit about freshness and rebuild behavior.
* Preserve Worker/D1 authority for public product truth while allowing
  Cloudflare world projections to own only public-safe world state.
* Make missing staleness contracts reviewable and checkable.

## Considered Options

* Public projection staleness declarations
* Implicit freshness based on route behavior
* Ad hoc comments near individual routes

## Decision Outcome

Chosen option: "Public projection staleness declarations", because the root
and app invariant ledgers already require each public projection to carry a
`generatedAt` value or equivalent rebuild timestamp plus a declared staleness
contract, and the OpenAgents.com architecture guard tracks route coverage in a
projection-surface ledger.

### Consequences

* Good, because consumers can distinguish live-at-read projections from
  generated or potentially stale records.
* Good, because public projection changes have a shared review vocabulary.
* Bad, because new public routes must be added to the projection ledger instead
  of relying only on local route tests.

### Confirmation

Compliance is confirmed by invariant review, the public projection inventory in
`apps/openagents.com/INVARIANTS.md`, route and projection tests, and
`apps/openagents.com/scripts/check-zero-debt-architecture.mjs`, which fails
missing projection-surface declarations as part of `check:deploy`.

## Pros and Cons of the Options

### Public projection staleness declarations

* Good, because the freshness contract is visible outside individual route
  implementations.
* Good, because stale or generated data can be described honestly in API
  payloads and docs.
* Bad, because the ledger must stay synchronized with route additions.

### Implicit freshness based on route behavior

* Good, because it avoids a separate inventory.
* Bad, because readers and agents would need to infer freshness from code.

### Ad hoc comments near individual routes

* Good, because the comment can be close to the implementation.
* Bad, because comments are hard to audit across all public routes.

## More Information

* `INVARIANTS.md` ("Public Projection Staleness")
* `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness Declaration")
* `apps/openagents.com/scripts/check-zero-debt-architecture.mjs`
* `docs/systems/README.md` ("D1 / projections / projection-surface ledger")
