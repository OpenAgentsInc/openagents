---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root INVARIANTS.md, apps/openagents.com/INVARIANTS.md, docs/systems/README.md, apps/openagents.com/scripts/check-zero-debt-architecture.mjs
informed: OpenAgents contributors and agents
---

# Declare public projection freshness at each surface

## Context and Problem Statement

OpenAgents exposes many public and agent-readable projections over D1-backed
state, Worker routes, product promises, Forum activity, proof surfaces, token
usage, and world state. A projection can be correct at write time but misleading
later if readers cannot tell when it was rebuilt or whether it is composed live.
The root and `openagents.com` invariant ledgers now require public projections
to carry `generatedAt` or an equivalent timestamp plus a declared staleness
contract.

## Decision Drivers

* Make freshness and rebuild behavior explicit on every public projection.
* Keep D1 rows, derived snapshots, and live-at-read routes from being mistaken
  for current state when their contract says otherwise.
* Fail new public routes through an architecture guard when they are missing
  from the projection-surface ledger.
* Preserve public-safe projection boundaries without granting authority.

## Considered Options

* Per-surface freshness declarations with a projection-surface ledger
* Implicit freshness based on route implementation
* One global cache policy for all public projections

## Decision Outcome

Chosen option: "Per-surface freshness declarations with a projection-surface
ledger", because each public surface has different rebuild triggers and safety
constraints, and the existing invariant plus `check-zero-debt-architecture.mjs`
already enforce explicit registration.

### Consequences

* Good, because public readers and agents can distinguish live-at-read,
  stored-snapshot, stale, degraded, and rebuilt projections.
* Good, because adding a new public projection requires declaring its freshness
  contract and source refs.
* Bad, because projection route changes must update both implementation and the
  ledger entry instead of relying on route code alone.

### Confirmation

Compliance is confirmed by the public projection staleness invariant, the
projection-surface ledger in `apps/openagents.com/INVARIANTS.md`, helpers in
`workers/api/src/public-projection-staleness.ts`, route tests that assert
`projection_staleness.v1`, and `check:deploy` through
`scripts/check-zero-debt-architecture.mjs`.

## Pros and Cons of the Options

### Per-surface freshness declarations with a projection-surface ledger

* Good, because the declaration lives beside the public authority and can name
  source stores such as `token_usage_events`.
* Good, because architecture checks catch missing declarations before deploy.
* Bad, because the ledger must be kept current as routes are added or renamed.

### Implicit freshness based on route implementation

* Good, because it avoids extra documentation.
* Bad, because consumers and agents cannot reliably infer rebuild semantics from
  a response body or URL.

### One global cache policy for all public projections

* Good, because it would be simple to state.
* Bad, because OpenAgents mixes live-at-read aggregates, stored snapshots,
  docs-backed registries, and event-derived projections with different
  contracts.

## More Information

* `INVARIANTS.md` ("Public Projection Staleness")
* `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness Declaration")
* `apps/openagents.com/scripts/check-zero-debt-architecture.mjs`
* `apps/openagents.com/workers/api/src/public-projection-staleness.ts`
* `apps/openagents.com/workers/api/src/public-khala-tokens-served-routes.ts`
* `docs/systems/README.md` ("D1 / projections / projection-surface ledger")
