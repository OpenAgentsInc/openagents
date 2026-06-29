---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, apps/openagents.com/INVARIANTS.md, docs/khala/2026-06-26-khala-open-issues-master-roadmap.md, docs/systems/README.md
informed: OpenAgents contributors, agents, and operators
---

# Count Khala served tokens from exact usage events

## Context and Problem Statement

The Khala roadmap optimizes for an honest public tokens-served counter while
also using demand attribution to distinguish external, internal stress, and
owner-capacity usage. The root Khala/Pylon runbook requires exact downstream
Codex token rows before treating a delegation as proven, and it explicitly says
counter movement alone is not proof. The OpenAgents.com invariant ledger records
`token_usage_events` as the canonical token usage ledger.

## Decision Drivers

* Public Khala counters must reconcile to durable exact usage rows.
* Internal stress and owner-capacity demand should count in the global scalar
  while remaining attributable for routing and trace review.
* Raw prompts, completions, provider payloads, tool args, credentials, local
  paths, and private source material must never be persisted in token rows.
* Reasoning tokens and provider/model families need consistent aggregate
  treatment for `/khala` and `/stats`.

## Considered Options

* Exact `token_usage_events` as the counter source of truth
* Synthetic counter increments from request attempts or streamed chunks
* Per-route ad hoc counters

## Decision Outcome

Chosen option: "Exact `token_usage_events` as the counter source of truth",
because it preserves exact accounting, idempotency, demand attribution, privacy
boundaries, and public projection reconciliation.

### Consequences

* Good, because public counters and model-mix stats can be audited back to exact
  rows.
* Good, because demand labels affect routing and review without hiding real
  usage from the public scalar.
* Bad, because assignment closeout is incomplete until exact usage rows exist.

### Confirmation

Compliance is confirmed by token ledger tests, public counter route tests,
Pylon/Codex turn-ingest tests, Khala closeout proof, and reconciliation of
`GET /api/public/khala-tokens-served` to exact `token_usage_events` rows.

## Pros and Cons of the Options

### Exact `token_usage_events` as the counter source of truth

* Good, because it makes usage durable, idempotent, and attribution-aware.
* Good, because privacy rules can reject unsafe fields before persistence.
* Bad, because producers must report complete usage rather than relying on best
  effort request observations.

### Synthetic counter increments from request attempts or streamed chunks

* Good, because it can show rough activity earlier.
* Bad, because it can overcount, double-count, or count failed requests as
  served usage.

### Per-route ad hoc counters

* Good, because each route could evolve independently.
* Bad, because global stats, model mix, and trace review would fragment across
  incompatible accounting systems.

## More Information

* `apps/openagents.com/INVARIANTS.md` ("Canonical Token Usage Ledger")
* `apps/openagents.com/INVARIANTS.md` ("Public Projection Staleness Declaration")
* `AGENTS.md` ("Khala -> Pylon -> Codex Coding Delegation Runbook")
* `docs/khala/2026-06-26-khala-open-issues-master-roadmap.md`
* `apps/openagents.com/workers/api/src/token-usage-ledger.ts`
* `apps/openagents.com/workers/api/src/public-khala-tokens-served-routes.ts`
* `apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts`
* `apps/openagents.com/workers/api/migrations/0137_token_usage_events.sql`
