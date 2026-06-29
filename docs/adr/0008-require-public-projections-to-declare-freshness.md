---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root INVARIANTS.md, apps/openagents.com/INVARIANTS.md, apps/openagents.com/scripts/check-zero-debt-architecture.mjs, apps/openagents.com/workers/api/src/public-projection-staleness.ts, docs/systems/README.md
informed: OpenAgents contributors and agents
---

# Require public projections to declare freshness

## Context and Problem Statement

OpenAgents exposes many public and agent-readable projections from D1-backed
source rows, Worker route composition, and stored snapshots. Those surfaces are
used as product evidence, counters, status pages, and agent-readable truth. A
public projection that silently lags behind its source of truth can cause the
platform to assert stale or opposite state on the exact routes users and agents
are told to trust.

## Decision Drivers

* Make public projection freshness explicit in every payload.
* Keep one shared staleness vocabulary across public Worker routes.
* Ensure new `/api/public/...` routes are added to a projection-surface ledger.
* Preserve D1 source rows as authority while public routes expose only
  public-safe projection payloads.

## Considered Options

* Public projections with `generatedAt` and a declared staleness contract
* Ad hoc freshness fields per route
* Public projections without explicit freshness metadata

## Decision Outcome

Chosen option: "Public projections with `generatedAt` and a declared staleness
contract", because it matches the existing invariant, shared schema module, and
zero-debt architecture guard for public projection surfaces.

### Consequences

* Good, because users, agents, and tests can distinguish live-at-read,
  rebuilt-on-transition, and stored-snapshot projections.
* Good, because new public routes fail the architecture guard until they are
  recorded in the projection-surface ledger and declare the shared contract.
* Good, because aggregate surfaces such as the Khala tokens-served counter can
  reconcile back to canonical D1 ledgers without exposing private row detail.
* Bad, because every new public projection needs explicit freshness modeling and
  ledger maintenance.

### Confirmation

Compliance is confirmed by app invariants, the shared
`PublicProjectionStalenessContract`, route tests, and
`apps/openagents.com/scripts/check-zero-debt-architecture.mjs`, which is run by
`bun run --cwd apps/openagents.com check:deploy`.

## Pros and Cons of the Options

### Public projections with `generatedAt` and a declared staleness contract

* Good, because every projection states when it was generated, how stale it may
  be, and which source transitions rebuild or compose it.
* Good, because a single vocabulary avoids incompatible route-local freshness
  conventions.
* Bad, because legacy projections must be ratcheted into compliance rather than
  being accepted silently.

### Ad hoc freshness fields per route

* Good, because route authors can choose local names quickly.
* Bad, because clients and agents cannot reliably interpret freshness across
  surfaces.
* Bad, because architecture checks cannot enforce a common contract.

### Public projections without explicit freshness metadata

* Good, because payloads are smaller.
* Bad, because consumers cannot tell whether a projection is current, stale, or
  missing a rebuild.
* Bad, because it conflicts with the current public projection invariant.

## More Information

* `INVARIANTS.md` ("Public Projection Staleness")
* `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness Declaration")
* `apps/openagents.com/workers/api/src/public-projection-staleness.ts`
* `apps/openagents.com/scripts/check-zero-debt-architecture.mjs`
* `apps/openagents.com/workers/api/src/public-khala-tokens-served-routes.ts`
* `apps/openagents.com/workers/api/migrations/0137_token_usage_events.sql`
* `docs/systems/README.md` ("D1 / projections / projection-surface ledger")
